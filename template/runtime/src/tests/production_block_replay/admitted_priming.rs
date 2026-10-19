//! Public LP priming on the qualified native successor, never captured-state repair.

use super::admitted_refusal::{advance, transfer};
use super::*;
use codec::DecodeAll;
use pallet_deos_actors::{ActorCostQuote, ActorType, Event};
use polkadot_sdk::frame_support::traits::{ExecuteBlock, tokens::fungible::InspectHold};
use polkadot_sdk::sp_consensus_aura::{digests::CompatibleDigestItem, sr25519::AuthoritySignature};
use primitives::AssetKind;

pub(super) fn input(name: &str, expected: &str) -> Vec<u8> {
  let bytes = std::fs::read(std::env::var(name).expect("explicit qualified checkpoint")).unwrap();
  assert_eq!(
    format!(
      "{:x}",
      H256::from(polkadot_sdk::sp_io::hashing::sha2_256(&bytes))
    ),
    expected
  );
  bytes
}

pub(super) fn retain_block(
  output: &std::path::Path,
  authored: &AuthoredBlock,
  history: &mut Vec<serde_json::Value>,
) {
  let bytes = authored.block.encode();
  let number = *authored.block.header.number();
  std::fs::write(
    output.join("native-blocks").join(format!("{number}.scale")),
    &bytes,
  )
  .unwrap();
  history.push(serde_json::json!({
    "number": number, "nativeHash": format!("{:?}", authored.block.header.hash()),
    "nativeParentHash": format!("{:?}", authored.block.header.parent_hash()),
    "postRoot": format!("{:?}", authored.block.header.state_root()),
    "unsealedBlockSha256": format!("{:x}", H256::from(polkadot_sdk::sp_io::hashing::sha2_256(&bytes))),
  }));
  std::fs::write(
    output.join("native-history.json"),
    serde_json::to_vec_pretty(history).unwrap(),
  )
  .unwrap();
}

struct Pending {
  id: pallet_deos_actors::ActorId,
  custody: crate::AccountId,
  contract: pallet_deos_actors::ActorContractOf<Runtime>,
  native_before: u128,
  paired_before: u128,
  lp_before: u128,
  funded: u128,
  quote: ActorCostQuote<u128>,
}

#[test]
#[ignore = "qualified successor inputs and real signed owner Contract/funding/priming blocks"]
fn retained_successor_public_lp_priming() {
  let prior = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_PRIMING_SOURCE_PARENT",
        "216baf777b9edaf3e9f10a1864daa8912815573d4dd5a183443d70cee907974e",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let mut storage = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_PRIMING_SOURCE_POST",
        "811ab7822799a0eb8d6eeeb5f349d232b92f65f562eb10b3e852b06830b15187",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let sealed = Block::decode_all(
    &mut input(
      "DEOS_PRIMING_SOURCE_BLOCK",
      "bdcc8d587704126705e13626971a6338f1e5d9a6a150247e90af3d3cfaddd666",
    )
    .as_slice(),
  )
  .unwrap();
  type Executor = polkadot_sdk::cumulus_pallet_aura_ext::BlockExecutor<Runtime, Executive>;
  let mut verified = sealed.clone().into();
  externalities_with_bound_code(prior).execute_with(|| {
    <Executor as ExecuteBlock<Block>>::verify_and_remove_seal(&mut verified);
  });
  let mut parent = sealed.header.clone();
  let seal = parent.digest_mut().pop().unwrap();
  assert!(<DigestItem as CompatibleDigestItem<AuthoritySignature>>::as_aura_seal(&seal).is_some());
  assert_eq!(*parent.number(), 187_466);
  assert_eq!(
    format!("{:x}", parent.state_root()),
    "4bc7de7fb379f3ddb6ff102badff32f4676b1800ca415400a0b3e6d925a7fd89"
  );
  let wasm = storage.top[well_known_keys::CODE].clone();
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256
  );
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let (lp, minimum, owner_lp_before, identities, untouched) =
    externalities_with_bound_code(storage.clone()).execute_with(|| {
      let lp = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((
        AssetKind::Native,
        AssetKind::Local(0),
      ))
      .unwrap()
      .lp_token;
      let minimum = <Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<
        crate::AccountId,
      >>::minimum_balance(lp);
      assert!(minimum > 0);
      let untouched = [87, 88].map(|id| Actors::active_actor_state(id).unwrap().encode());
      for id in 15..87 {
        let actor = Actors::active_actor_state(id).unwrap();
        assert_eq!(actor.identity.owner, owner);
        assert_eq!(actor.identity.cycle_nonce, 1);
        assert!(actor.run_state.is_none() && !actor.hot.pending_signal);
        assert_eq!(actor.hot.cycle_state, CycleState::Idle);
        assert_eq!(actor.contract.steps.len(), if id < 51 { 3 } else { 2 });
        assert_eq!(
          Assets::balance(lp, &actor.identity.sovereign_account),
          crate::UNIT
        );
      }
      (
        lp,
        minimum,
        Assets::balance(lp, &owner),
        Actors::actor_identity_count(),
        untouched,
      )
    });
  let output = std::path::PathBuf::from(
    std::env::var("DEOS_PUBLIC_PRIMING_OUTPUT").expect("new output directory"),
  );
  std::fs::create_dir(&output).unwrap();
  std::fs::create_dir(output.join("native-blocks")).unwrap();
  std::fs::write(output.join("source-post.scale"), storage.top.encode()).unwrap();
  std::fs::write(output.join("source-native-header.scale"), parent.encode()).unwrap();
  let mut history = Vec::new();
  let mut rows = Vec::new();
  let mut last = None;
  let ids = (15..87).collect::<Vec<_>>();
  for batch in ids.chunks(4) {
    let calls = externalities_with_bound_code(storage.clone()).execute_with(|| {
      batch
        .iter()
        .map(|id| {
          let mut contract = Actors::active_actor_state(*id).unwrap().contract;
          contract.steps = vec![StepOf::<Runtime> {
            precondition: None,
            task: Task::Transfer {
              to: owner.clone(),
              asset: AssetKind::Local(lp),
              amount: AmountResolution::Percent(Perbill::one()),
            },
            on_error: StepErrorPolicy::AbortCycle,
          }]
          .try_into()
          .unwrap();
          RuntimeCall::Actors(pallet_deos_actors::Call::update_contract {
            actor_id: *id,
            contract,
          })
        })
        .collect::<Vec<_>>()
    });
    let block = advance(
      &mut storage,
      &mut parent,
      &wasm,
      &signer,
      &calls,
      UserDemand::ActorOnly,
    );
    retain_block(&output, &block, &mut history);
    let pending = externalities_with_bound_code(storage.clone()).execute_with(|| {
      batch
        .iter()
        .map(|id| {
          let actor = Actors::active_actor_state(*id).unwrap();
          assert_eq!(actor.identity.cycle_nonce, 1);
          assert!(actor.run_state.is_none() && !actor.hot.pending_signal);
          assert_eq!(actor.contract.steps.len(), 1);
          let quote = Actors::actor_cost_quote(*id).unwrap();
          assert_eq!(quote.actor_type, ActorType::User);
          let custody = actor.identity.sovereign_account;
          let native_before = Balances::free_balance(&custody);
          let funded = <Runtime as pallet_deos_actors::Config>::MinUserBalance::get()
            .checked_add(crate::EXISTENTIAL_DEPOSIT)
            .unwrap()
            .checked_add(quote.prospective_trigger_fee.unwrap().fee)
            .unwrap()
            .checked_add(quote.prospective_pipeline_fee.unwrap().total_fee)
            .unwrap()
            .checked_add(quote.maximum_next_action_fee.maximum_effect_fee)
            .unwrap()
            .saturating_sub(native_before);
          Pending {
            id: *id,
            native_before,
            paired_before: Assets::balance(0, &custody),
            lp_before: Assets::balance(lp, &custody),
            custody,
            funded,
            quote,
            contract: actor.contract,
          }
        })
        .collect::<Vec<_>>()
    });
    let calls = pending
      .iter()
      .flat_map(|row| {
        let mut calls = Vec::new();
        if row.funded > 0 {
          calls.push(transfer(row.custody.clone(), row.funded));
        }
        calls.push(RuntimeCall::Actors(
          pallet_deos_actors::Call::manual_trigger { actor_id: row.id },
        ));
        calls
      })
      .collect::<Vec<_>>();
    let block = advance(
      &mut storage,
      &mut parent,
      &wasm,
      &signer,
      &calls,
      UserDemand::ActorOnly,
    );
    retain_block(&output, &block, &mut history);
    let triggers = externalities_with_bound_code(storage.clone()).execute_with(|| {
      pending
        .iter()
        .map(|row| {
          let fees =
            System::events()
              .iter()
              .filter_map(|event| match event.event {
                RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
                  actor_id, fee, ..
                }) if actor_id == row.id => Some(fee),
                _ => None,
              })
              .collect::<Vec<_>>();
          assert_eq!(fees.len(), 1);
          assert!(fees[0] > 0);
          let actor = Actors::active_actor_state(row.id).unwrap();
          assert!(actor.hot.pending_signal && actor.run_state.is_none());
          assert_eq!(actor.identity.cycle_nonce, 1);
          fees[0]
        })
        .collect::<Vec<_>>()
    });
    let selected_parent = parent.clone();
    let block = advance(
      &mut storage,
      &mut parent,
      &wasm,
      &signer,
      &[],
      UserDemand::ActorOnly,
    );
    retain_block(&output, &block, &mut history);
    externalities_with_bound_code(storage.clone()).execute_with(|| {
      for (row, trigger) in pending.iter().zip(triggers) {
        let mut pipelines = Vec::new(); let mut actions = Vec::new(); let mut transfers = Vec::new(); let mut summaries = Vec::new();
        for record in System::events() {
          match record.event {
            RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id, fee }) if actor_id == row.id => pipelines.push(fee),
            RuntimeEvent::Actors(Event::ActionFeeCharged { actor_id, fee, .. }) if actor_id == row.id => actions.push(fee),
            RuntimeEvent::Actors(Event::TransferExecuted { actor_id, asset, amount, to, .. }) if actor_id == row.id => {
              assert_eq!(asset, AssetKind::Local(lp)); assert_eq!(to, owner); transfers.push(amount);
            },
            RuntimeEvent::Actors(Event::CycleSummary { actor_id, cycle_nonce, .. }) if actor_id == row.id => summaries.push(cycle_nonce),
            RuntimeEvent::Actors(Event::StepFailed {actor_id, ..} | Event::StepSkipped {actor_id, ..} | Event::ActorClosed {actor_id, ..})
              if actor_id == row.id => panic!("priming must be an invoked paid Transfer: {:?}", record.event),
            _ => {},
          }
        }
        assert_eq!(pipelines, [row.quote.prospective_pipeline_fee.unwrap().total_fee]);
        assert_eq!(actions, [row.quote.maximum_next_action_fee.maximum_effect_fee]);
        assert!(pipelines[0] > 0 && actions[0] > 0);
        assert_eq!(transfers, [row.lp_before - minimum]);
        assert_eq!(summaries, [2]);
        let actor = Actors::active_actor_state(row.id).unwrap();
        assert_eq!(actor.identity.cycle_nonce, 2);
        assert_eq!(actor.hot.cycle_state, CycleState::Idle);
        assert_eq!(actor.hot.unsuccessful_attempt_streak, 0);
        assert!(actor.run_state.is_none() && !actor.hot.pending_signal);
        assert_eq!(actor.contract, row.contract);
        assert_eq!(Assets::balance(lp, &row.custody), minimum);
        assert_eq!(Assets::balance(0, &row.custody), row.paired_before);
        assert_eq!(Balances::free_balance(&row.custody), row.native_before + row.funded - trigger - pipelines[0] - actions[0]);
        let hold = Actors::actor_state_hold(row.id).unwrap();
        assert_eq!(hold.owner, owner);
        assert_eq!(hold.breakdown, Actors::actor_cost_quote(row.id).unwrap().actor_state_hold.breakdown);
        rows.push(serde_json::json!({ "actor": row.id, "nativeFunded": row.funded.to_string(),
          "triggerFee": trigger.to_string(), "pipelineFee": pipelines[0].to_string(), "actionFee": actions[0].to_string(),
          "transferredLp": transfers[0].to_string(), "remainingLp": minimum.to_string(), "cycleNonce": 2 }));
      }
    });
    std::fs::write(
      output.join("primed-actors.json"),
      serde_json::to_vec_pretty(&rows).unwrap(),
    )
    .unwrap();
    last = Some((selected_parent, block));
  }
  let (selected_parent, selected) = last.unwrap();
  assert_eq!(rows.len(), 72);
  externalities_with_bound_code(storage.clone()).execute_with(|| {
    assert_eq!(Actors::actor_identity_count(), identities);
    assert_eq!(
      Assets::balance(lp, &owner) - owner_lp_before,
      (crate::UNIT - minimum) * 72
    );
    for (index, id) in [87, 88].iter().enumerate() {
      assert_eq!(
        Actors::active_actor_state(*id).unwrap().encode(),
        untouched[index]
      );
    }
    // Test-only complete-owner reconciliation; MaxActorIdentities bounds this ledger.
    let total = pallet_deos_actors::ActorStateHolds::<Runtime>::iter()
      .filter(|(_, h)| h.owner == owner)
      .map(|(_, h)| {
        h.breakdown.identity
          + h.breakdown.contract_head
          + h.breakdown.contract_body
          + h.breakdown.detector
          + h.breakdown.run
      })
      .sum::<u128>();
    let reason: crate::RuntimeHoldReason = pallet_deos_actors::HoldReason::ActorState.into();
    assert_eq!(
      <Balances as InspectHold<crate::AccountId>>::balance_on_hold(&reason, &owner),
      total
    );
    assert!(Actors::block_resource_state().is_none());
    assert!(
      !Actors::finalized_block_resource_telemetry()
        .unwrap()
        .optional_actor_work_halted()
    );
  });
  std::fs::write(output.join("parent.scale"), selected.pre_state.top.encode()).unwrap();
  std::fs::write(output.join("parent-header.scale"), selected_parent.encode()).unwrap();
  std::fs::write(output.join("post.scale"), selected.post_state.top.encode()).unwrap();
  std::fs::create_dir(output.join("selected")).unwrap();
  pvf_fixture::export(
    &output.join("selected"),
    &polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into(),
    (
      selected.pre_state.clone(),
      selected.post_state.clone(),
      &selected_parent,
      selected.block.clone(),
      selected.inherent_data.clone(),
    ),
  )
  .unwrap();
  replay_complete_block_in_wasm(&selected, &wasm, "retained-successor-public-lp-priming");
  std::fs::write(output.join("result.json"), serde_json::to_vec_pretty(&serde_json::json!({
    "firstBlock": 187_467, "lastBlock": parent.number(), "nativeBlocks": history.len(), "primedActors": rows.len(),
    "lpMinimum": minimum.to_string(), "zeroPreservableLp": true, "rawLpZero": false,
    "postRoot": format!("{:?}", parent.state_root()), "nativeParentHash": format!("{:?}", parent.hash()),
    "inheritedSignedNativeAncestry": true, "runningTargetQualified": false, "physicalPricingQualified": false,
  })).unwrap()).unwrap();
}
