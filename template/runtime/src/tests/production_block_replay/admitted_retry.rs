//! Genuine Temporary LP-output failure, economic prefix and paid Deadline continuation.

use super::admitted_priming::{input, retain_block};
use super::admitted_refusal::{advance, transfer};
use super::*;
use codec::DecodeAll;
use pallet_deos_actors::{Event, RetryClass, StepOutcome, SuspensionReason, WakeupKey, WeightInfo};
use polkadot_sdk::frame_support::traits::ExecuteBlock;
use polkadot_sdk::sp_consensus_aura::{digests::CompatibleDigestItem, sr25519::AuthoritySignature};
use polkadot_sdk::sp_runtime::DispatchError;
use primitives::AssetKind;

type W = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;

fn export_stage(
  output: &std::path::Path,
  name: &str,
  parent: &Header,
  block: &AuthoredBlock,
  wasm: &[u8],
) {
  let path = output.join(name);
  std::fs::create_dir(&path).unwrap();
  std::fs::write(path.join("parent.scale"), block.pre_state.top.encode()).unwrap();
  std::fs::write(path.join("post.scale"), block.post_state.top.encode()).unwrap();
  std::fs::write(path.join("parent-header.scale"), parent.encode()).unwrap();
  std::fs::create_dir(path.join("selected")).unwrap();
  pvf_fixture::export(
    &path.join("selected"),
    &polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into(),
    (
      block.pre_state.clone(),
      block.post_state.clone(),
      parent,
      block.block.clone(),
      block.inherent_data.clone(),
    ),
  )
  .unwrap();
  replay_complete_block_in_wasm(block, wasm, name);
}

fn fee_sink_distributions() -> u128 {
  System::events()
    .into_iter()
    .filter_map(|record| match record.event {
      RuntimeEvent::Actors(Event::TransferExecuted {
        actor_id,
        asset: AssetKind::Native,
        amount,
        ..
      }) if actor_id == primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID => Some(amount),
      _ => None,
    })
    .sum()
}

fn assert_temporary_attempt(storage: &Storage, attempt: u32, action_fee: u128) {
  externalities_with_bound_code(storage.clone()).execute_with(|| {
    let mut failed = 0; let mut actions = 0; let mut suspended = 0; let mut continued = 0;
    for record in System::events() {
      match record.event {
        RuntimeEvent::Actors(Event::StepFailed {actor_id: 89, cycle_nonce, step_index, retry_class, error}) => {
          assert_eq!((cycle_nonce, step_index), (1, 1)); assert_eq!(retry_class, RetryClass::Temporary);
          // SCALE intentionally does not retain Other's static diagnostic string.
          assert_eq!(error.encode(), DispatchError::Other("MinimumLpOutputNotMet").encode()); failed += 1;
        },
        RuntimeEvent::Actors(Event::ActionFeeCharged {actor_id: 89, cycle_nonce, step_index, fee, actual_effect_weight}) => {
          assert_eq!((cycle_nonce, step_index), (1, 1)); assert_eq!(fee, action_fee);
          assert_eq!(actual_effect_weight, W::task_add_liquidity()); actions += 1;
        },
        RuntimeEvent::Actors(Event::CycleSuspended {actor_id: 89, cycle_nonce, cursor, reason, ..}) => {
          assert_eq!((cycle_nonce, cursor), (1, 1)); assert_eq!(reason, SuspensionReason::Temporary); suspended += 1;
        },
        RuntimeEvent::Actors(Event::CycleContinued {actor_id: 89, cycle_nonce, cursor}) => {
          assert_eq!((cycle_nonce, cursor), (1, 1)); continued += 1;
        },
        RuntimeEvent::Actors(Event::PipelineFeeCharged {actor_id: 89, ..} | Event::CycleStarted {actor_id: 89, ..}
          | Event::StepSkipped {actor_id: 89, ..} | Event::LiquidityAdded {actor_id: 89, ..}
          | Event::CycleSummary {actor_id: 89, ..} | Event::ActorClosed {actor_id: 89, ..}) =>
          panic!("Temporary attempt preserves paid Pipeline and prefix: {:?}", record.event),
        _ => {},
      }
    }
    assert_eq!((failed, actions, suspended, continued), (1, 1, 1, u32::from(attempt > 1)));
    let actor = Actors::active_actor_state(89).unwrap();
    assert_eq!(actor.identity.cycle_nonce, 0); assert_eq!(actor.hot.cycle_state, CycleState::Suspended);
    assert!(!actor.hot.pending_signal);
    let run = actor.run_state.unwrap();
    assert_eq!((run.cycle_nonce, run.cursor, run.unsuccessful_attempts_at_cursor), (1, 1, attempt));
    assert_eq!(run.suspension, Some(SuspensionReason::Temporary));
    assert_eq!(run.last_attempt_block, System::block_number());
    assert!(matches!(run.last_step_outcome, Some(StepOutcome::Failed(ref failure)) if failure.retry == RetryClass::Temporary));
    let handle = Actors::deadline_handles(89).unwrap();
    assert_eq!(handle.actor.actor_id, 89);
    assert_eq!(handle.key, WakeupKey::Block(run.eligible_at));
    assert!(Actors::service_nodes(89).is_none());
    let page = Actors::deadline_pages(handle.key, handle.page).unwrap();
    assert_eq!(page.entries[usize::from(handle.slot)], Some(handle.actor));
    assert_eq!(Actors::actor_state_hold(89).unwrap().breakdown, Actors::actor_cost_quote(89).unwrap().actor_state_hold.breakdown);
    assert!(Actors::block_resource_state().is_none());
    assert!(!Actors::finalized_block_resource_telemetry().unwrap().optional_actor_work_halted());
  });
}

#[test]
#[ignore = "exact paid-Running K checkpoint and public authored Temporary failure/Deadline/retry suffix"]
fn retained_successor_temporary_lp_paid_retry() {
  let prior = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_RETRY_SOURCE_PARENT",
        "b4702064d9831c8dd0c613fcdc0caf6142ab353d02ff69bf364a3de99e61399a",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let mut storage = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_RETRY_SOURCE_POST",
        "d20528728b140fca4dd16ba0ae44959d53bd5fc9d23870fba1f35a9c7620f615",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let sealed = Block::decode_all(
    &mut input(
      "DEOS_RETRY_SOURCE_BLOCK",
      "bbe1019879951635342d5a3a7cd0ed0fb4bccabeec72acc26d19d0c1e01a1bdc",
    )
    .as_slice(),
  )
  .unwrap();
  type Executor = polkadot_sdk::cumulus_pallet_aura_ext::BlockExecutor<Runtime, Executive>;
  let mut verified = sealed.clone().into();
  externalities_with_bound_code(prior)
    .execute_with(|| <Executor as ExecuteBlock<Block>>::verify_and_remove_seal(&mut verified));
  let mut parent = sealed.header.clone();
  let seal = parent.digest_mut().pop().unwrap();
  assert!(<DigestItem as CompatibleDigestItem<AuthoritySignature>>::as_aura_seal(&seal).is_some());
  assert_eq!(*parent.number(), 187_565);
  assert_eq!(
    format!("{:x}", parent.state_root()),
    "ac802f16d146fc9c61386b68513197c620c5c4b605552a5a40ed80f49761ddb1"
  );
  let wasm = storage.top[well_known_keys::CODE].clone();
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256
  );
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let (mut contract, custody, lp) =
    externalities_with_bound_code(storage.clone()).execute_with(|| {
      let actor = Actors::active_actor_state(89).unwrap();
      assert_eq!(actor.identity.owner, owner);
      assert_eq!(actor.identity.cycle_nonce, 0);
      assert!(actor.run_state.is_none() && !actor.hot.pending_signal);
      let lp = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((
        AssetKind::Native,
        AssetKind::Local(0),
      ))
      .unwrap()
      .lp_token;
      (actor.contract, actor.identity.sovereign_account, lp)
    });
  // First backoff alone is B+1 and can remain on Service; authored cooldown forces Deadline.
  contract.cooldown_blocks = 2;
  contract.steps = vec![
    StepOf::<Runtime> {
      precondition: None,
      task: Task::Transfer {
        to: owner.clone(),
        asset: AssetKind::Native,
        amount: AmountResolution::Fixed(1),
      },
      on_error: StepErrorPolicy::AbortCycle,
    },
    StepOf::<Runtime> {
      precondition: None,
      task: Task::AddLiquidity {
        asset_a: AssetKind::Native,
        asset_b: AssetKind::Local(0),
        amount_a: AmountResolution::Fixed(crate::UNIT),
        amount_b: AmountResolution::Fixed(crate::UNIT),
        min_lp_out: 2 * crate::UNIT,
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
    },
  ]
  .try_into()
  .unwrap();
  let output = std::path::PathBuf::from(
    std::env::var("DEOS_PAID_RETRY_OUTPUT").expect("new output directory"),
  );
  std::fs::create_dir(&output).unwrap();
  std::fs::create_dir(output.join("native-blocks")).unwrap();
  let mut history = Vec::new();
  let block = advance(
    &mut storage,
    &mut parent,
    &wasm,
    &signer,
    &[RuntimeCall::Actors(
      pallet_deos_actors::Call::update_contract {
        actor_id: 89,
        contract,
      },
    )],
    UserDemand::ActorOnly,
  );
  retain_block(&output, &block, &mut history);
  let lp_fee = <crate::WeightToFee as WeightToFee>::weight_to_fee(&W::task_add_liquidity());
  let prefix_fee = <crate::WeightToFee as WeightToFee>::weight_to_fee(&W::task_transfer());
  let (q, extra) = externalities_with_bound_code(storage.clone()).execute_with(|| {
    let q = Actors::actor_cost_quote(89).unwrap();
    let desired = <Runtime as pallet_deos_actors::Config>::MinUserBalance::get()
      .checked_add(crate::EXISTENTIAL_DEPOSIT)
      .unwrap()
      .checked_add(crate::UNIT)
      .unwrap()
      .checked_add(1)
      .unwrap()
      .checked_add(q.prospective_trigger_fee.unwrap().fee)
      .unwrap()
      .checked_add(q.prospective_pipeline_fee.unwrap().total_fee)
      .unwrap()
      .checked_add(prefix_fee)
      .unwrap()
      .checked_add(lp_fee.checked_mul(2).unwrap())
      .unwrap();
    (q, desired.saturating_sub(Balances::free_balance(&custody)))
  });
  let mut calls = Vec::new();
  if extra > 0 {
    calls.push(transfer(custody.clone(), extra));
  }
  calls.push(RuntimeCall::Actors(
    pallet_deos_actors::Call::manual_trigger { actor_id: 89 },
  ));
  let block = advance(
    &mut storage,
    &mut parent,
    &wasm,
    &signer,
    &calls,
    UserDemand::ActorOnly,
  );
  retain_block(&output, &block, &mut history);
  let (native_funded, paired_initial, lp_initial, reserves, supply, sink_funded) = externalities_with_bound_code(storage.clone()).execute_with(|| {
    let actor = Actors::active_actor_state(89).unwrap(); assert!(actor.hot.pending_signal && actor.run_state.is_none());
    let reserves = crate::AssetConversion::get_reserves(AssetKind::Native, AssetKind::Local(0)).unwrap();
    assert_eq!(reserves.0, reserves.1);
    assert_eq!(<Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<crate::AccountId>>::total_issuance(lp), reserves.0, "balanced pool makes actual LP output UNIT, below the authored 2*UNIT");
    (Balances::free_balance(&custody), Assets::balance(0, &custody), Assets::balance(lp, &custody), reserves, <Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<crate::AccountId>>::total_issuance(lp),
      Balances::free_balance(<Runtime as pallet_deos_actors::Config>::FeeSink::get()))
  });
  let block = advance(
    &mut storage,
    &mut parent,
    &wasm,
    &signer,
    &[],
    UserDemand::ActorOnly,
  );
  retain_block(&output, &block, &mut history);
  let prefix_run = externalities_with_bound_code(storage.clone()).execute_with(|| {
    let mut transfers = 0;
    let mut pipelines = 0;
    let mut actions = 0;
    for record in System::events() {
      match record.event {
        RuntimeEvent::Actors(Event::TransferExecuted {
          actor_id: 89,
          amount,
          asset,
          to,
          cycle_nonce,
          step_index,
        }) => {
          assert_eq!((cycle_nonce, step_index), (1, 0));
          assert_eq!(amount, 1);
          assert_eq!(asset, AssetKind::Native);
          assert_eq!(to, owner);
          transfers += 1;
        }
        RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id: 89, fee }) => {
          assert_eq!(fee, q.prospective_pipeline_fee.unwrap().total_fee);
          pipelines += 1;
        }
        RuntimeEvent::Actors(Event::ActionFeeCharged {
          actor_id: 89, fee, ..
        }) => {
          assert_eq!(fee, prefix_fee);
          actions += 1;
        }
        RuntimeEvent::Actors(
          Event::StepFailed { actor_id: 89, .. } | Event::StepSkipped { actor_id: 89, .. },
        ) => panic!("prefix must commit a real Transfer"),
        _ => {}
      }
    }
    assert_eq!((transfers, pipelines, actions), (1, 1, 1));
    assert_eq!(
      native_funded - Balances::free_balance(&custody),
      1 + prefix_fee + q.prospective_pipeline_fee.unwrap().total_fee
    );
    assert_eq!(
      Balances::free_balance(<Runtime as pallet_deos_actors::Config>::FeeSink::get()) - sink_funded,
      prefix_fee + q.prospective_pipeline_fee.unwrap().total_fee
    );
    let a = Actors::active_actor_state(89).unwrap();
    let run = a.run_state.unwrap();
    assert_eq!(a.identity.cycle_nonce, 0);
    assert_eq!(run.cursor, 1);
    assert_eq!(run.last_committed_step_block, Some(*parent.number()));
    run
  });
  let mut stages = Vec::new();
  for attempt in 1..=2 {
    let native_before = externalities_with_bound_code(storage.clone())
      .execute_with(|| Balances::free_balance(&custody));
    let sink_before = externalities_with_bound_code(storage.clone()).execute_with(|| {
      Balances::free_balance(<Runtime as pallet_deos_actors::Config>::FeeSink::get())
    });
    let stage_parent = parent.clone();
    let block = advance(
      &mut storage,
      &mut parent,
      &wasm,
      &signer,
      &[],
      UserDemand::ActorOnly,
    );
    retain_block(&output, &block, &mut history);
    assert_temporary_attempt(&storage, attempt, lp_fee);
    let run = externalities_with_bound_code(storage.clone()).execute_with(|| {
      let run = Actors::active_actor_state(89).unwrap().run_state.unwrap();
      assert_eq!(run.contract_authority, prefix_run.contract_authority);
      assert_eq!(run.last_committed_step_block, prefix_run.last_committed_step_block);
      assert_eq!(native_before - Balances::free_balance(&custody), lp_fee);
      assert_eq!(Balances::free_balance(<Runtime as pallet_deos_actors::Config>::FeeSink::get()) + fee_sink_distributions() - sink_before, lp_fee);
      assert_eq!(Assets::balance(0, &custody), paired_initial); assert_eq!(Assets::balance(lp, &custody), lp_initial);
      assert_eq!(crate::AssetConversion::get_reserves(AssetKind::Native, AssetKind::Local(0)).unwrap(), reserves);
      assert_eq!(<Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<crate::AccountId>>::total_issuance(lp), supply);
      run
    });
    let name = if attempt == 1 {
      "first-failure"
    } else {
      "retry-failure"
    };
    export_stage(&output, name, &stage_parent, &block, &wasm);
    stages.push(serde_json::json!({"stage": name, "block": parent.number(), "eligibleAt": run.eligible_at,
      "failuresAtCursor": run.unsuccessful_attempts_at_cursor, "lastCommitted": run.last_committed_step_block,
      "postRoot": format!("{:?}", parent.state_root())}));
    if attempt == 1 {
      assert_eq!(run.eligible_at, parent.number().checked_add(2).unwrap());
      let waiting = advance(
        &mut storage,
        &mut parent,
        &wasm,
        &signer,
        &[],
        UserDemand::ActorOnly,
      );
      retain_block(&output, &waiting, &mut history);
      externalities_with_bound_code(storage.clone()).execute_with(|| {
        assert_eq!(Actors::actor_run_state(89).unwrap().encode(), run.encode());
        assert_eq!(
          Actors::deadline_handles(89).unwrap().key,
          WakeupKey::Block(run.eligible_at)
        );
        assert!(Actors::service_nodes(89).is_none());
        assert_eq!(Balances::free_balance(&custody), native_before - lp_fee);
      });
      let wake_parent = parent.clone();
      let wake = advance(
        &mut storage,
        &mut parent,
        &wasm,
        &signer,
        &[],
        UserDemand::ActorOnly,
      );
      retain_block(&output, &wake, &mut history);
      externalities_with_bound_code(storage.clone()).execute_with(|| {
        assert_eq!(
          Actors::actor_run_state(89).unwrap().encode(),
          run.encode(),
          "Deadline return cannot rewrite Run or its paid authority"
        );
        assert!(Actors::deadline_handles(89).is_none());
        let service = Actors::service_nodes(89).unwrap();
        assert_eq!(
          service.eligible_from,
          parent.number().checked_add(1).unwrap()
        );
        assert_eq!(Balances::free_balance(&custody), native_before - lp_fee);
        assert!(!System::events().iter().any(|record| matches!(
          record.event,
          RuntimeEvent::Actors(
            Event::CycleContinued { actor_id: 89, .. }
              | Event::ActionFeeCharged { actor_id: 89, .. }
              | Event::PipelineFeeCharged { actor_id: 89, .. }
              | Event::StepFailed { actor_id: 89, .. }
          )
        )));
      });
      export_stage(&output, "wakeup", &wake_parent, &wake, &wasm);
    }
  }
  std::fs::write(output.join("result.json"), serde_json::to_vec_pretty(&serde_json::json!({
    "actor": 89, "lastBlock": parent.number(), "nativeBlocks": history.len(), "stages": stages,
    "prefixTransfer": "1", "pipelineFee": q.prospective_pipeline_fee.unwrap().total_fee.to_string(),
    "prefixActionFee": prefix_fee.to_string(), "lpActionFeePerAttempt": lp_fee.to_string(), "newPipelineFeesAfterPrefix": 0,
    "committedPrefixPreserved": true, "lpEconomicRollback": true, "actualRetryTransitionQualified": true,
    "inheritedSignedNativeAncestry": true, "physicalPricingQualified": false, "completeDomainMaximaQualified": false,
  })).unwrap()).unwrap();
}

#[test]
#[ignore = "exact retry checkpoint, exhausted Temporary attempt and public custody-neutral close"]
fn retained_retry_exhaustion_and_owner_close() {
  use pallet_deos_actors::{ActorClass, CloseReason, CycleResult};
  use polkadot_sdk::frame_support::traits::fungible::InspectHold;

  let prior = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_RETRY_CLOSE_PARENT",
        "1194aa7c3e221a01115d52ac74a5a049e0b83dcb6bbc4c0c0640b46cc850dd5a",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let mut storage = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_RETRY_CLOSE_POST",
        "e21988dc00b1dc52b3f8cb68d6fda8462ea78df357a413039f080e9ac394bda3",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let sealed = Block::decode_all(
    &mut input(
      "DEOS_RETRY_CLOSE_BLOCK",
      "2762b5a289f939bcd735b0a56263ac6b294dac9ee204cc11bca5f24801adbe8d",
    )
    .as_slice(),
  )
  .unwrap();
  type Executor = polkadot_sdk::cumulus_pallet_aura_ext::BlockExecutor<Runtime, Executive>;
  let mut verified = sealed.clone().into();
  externalities_with_bound_code(prior)
    .execute_with(|| <Executor as ExecuteBlock<Block>>::verify_and_remove_seal(&mut verified));
  let mut parent = sealed.header.clone();
  let seal = parent.digest_mut().pop().unwrap();
  assert!(<DigestItem as CompatibleDigestItem<AuthoritySignature>>::as_aura_seal(&seal).is_some());
  assert_eq!(*parent.number(), 187_572);
  assert_eq!(
    format!("{:x}", parent.state_root()),
    "6282d891729ea6cf8c4c6b11ac9a83a379101e5383f2066d7be09784da1c7901"
  );
  let wasm = storage.top[well_known_keys::CODE].clone();
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256
  );
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let reason: crate::RuntimeHoldReason = pallet_deos_actors::HoldReason::ActorState.into();
  let owner_held = || <Balances as InspectHold<crate::AccountId>>::balance_on_hold(&reason, &owner);
  let (
    before,
    custody,
    lp,
    funds,
    reserves,
    supply,
    identities,
    active,
    initial_hold,
    held_before,
    bitmap_before,
  ) = externalities_with_bound_code(storage.clone()).execute_with(|| {
    let before = Actors::active_actor_state(89).unwrap();
    assert_eq!(before.identity.owner, owner);
    assert_eq!(before.identity.cycle_nonce, 0);
    let run = before.run_state.as_ref().unwrap();
    assert_eq!(
      (
        run.cycle_nonce,
        run.cursor,
        run.unsuccessful_attempts_at_cursor
      ),
      (1, 1, 2)
    );
    assert_eq!(run.last_committed_step_block, Some(187_568));
    assert_eq!(run.eligible_at, 187_574);
    assert_eq!(run.cumulative_outcomes.executed_steps, 1);
    assert_eq!(run.cumulative_outcomes.committed_effectful_tasks, 1);
    assert_eq!(run.cumulative_outcomes.failed_steps, 2);
    assert_eq!(
      Actors::deadline_handles(89).unwrap().key,
      WakeupKey::Block(187_574)
    );
    assert!(Actors::service_nodes(89).is_none());
    let custody = before.identity.sovereign_account.clone();
    let lp = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((
      AssetKind::Native,
      AssetKind::Local(0),
    ))
    .unwrap()
    .lp_token;
    let funds = [
      Balances::free_balance(&custody),
      Assets::balance(0, &custody),
      Assets::balance(lp, &custody),
    ];
    let reserves =
      crate::AssetConversion::get_reserves(AssetKind::Native, AssetKind::Local(0)).unwrap();
    let supply = <Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<
      crate::AccountId,
    >>::total_issuance(lp);
    (
      before,
      custody,
      lp,
      funds,
      reserves,
      supply,
      Actors::actor_identity_count(),
      Actors::active_actor_count(),
      Actors::actor_state_hold(89).unwrap().breakdown,
      owner_held(),
      Actors::owner_slot_bitmap(&owner),
    )
  });
  let hold_total = |h: pallet_deos_actors::ActorStateHoldBreakdown<u128>| {
    h.identity + h.contract_head + h.contract_body + h.detector + h.run
  };
  let (cold, cold_funds, cold_hold) =
    externalities_with_bound_code(storage.clone()).execute_with(|| {
      let cold = Actors::active_actor_state(90).unwrap();
      assert_eq!(cold.identity.owner, owner);
      assert_eq!(cold.identity.cycle_nonce, 0);
      assert!(cold.run_state.is_none() && !cold.hot.pending_signal);
      assert_eq!(cold.contract.steps.len(), 1);
      let account = &cold.identity.sovereign_account;
      let funds = [
        Balances::free_balance(account),
        Assets::balance(0, account),
        Assets::balance(lp, account),
      ];
      (cold, funds, Actors::actor_state_hold(90).unwrap().breakdown)
    });
  let output = std::path::PathBuf::from(
    std::env::var("DEOS_RETRY_CLOSE_OUTPUT").expect("new lifecycle output"),
  );
  std::fs::create_dir(&output).unwrap();
  std::fs::create_dir(output.join("native-blocks")).unwrap();
  let mut history = Vec::new();
  for number in 187_573..=187_574 {
    let stage_parent = parent.clone();
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
      assert_eq!(System::block_number(), number);
      assert_eq!(
        Actors::actor_run_state(89).unwrap(),
        before.run_state.clone().unwrap()
      );
      if number == 187_573 {
        assert_eq!(
          Actors::deadline_handles(89).unwrap().key,
          WakeupKey::Block(187_574)
        );
        assert!(Actors::service_nodes(89).is_none());
      } else {
        assert!(Actors::deadline_handles(89).is_none());
        assert_eq!(Actors::service_nodes(89).unwrap().eligible_from, 187_575);
      }
      assert_eq!(Balances::free_balance(&custody), funds[0]);
      assert_eq!(Assets::balance(0, &custody), funds[1]);
      assert_eq!(Assets::balance(lp, &custody), funds[2]);
      assert!(!System::events().iter().any(|r| matches!(
        r.event,
        RuntimeEvent::Actors(
          Event::ActionFeeCharged { actor_id: 89, .. }
            | Event::PipelineFeeCharged { actor_id: 89, .. }
            | Event::CycleContinued { actor_id: 89, .. }
            | Event::CycleSummary { actor_id: 89, .. }
        )
      )));
    });
    if number == 187_574 {
      export_stage(&output, "terminal-wakeup", &stage_parent, &block, &wasm);
    }
  }
  let sink_before = externalities_with_bound_code(storage.clone()).execute_with(|| {
    Balances::free_balance(<Runtime as pallet_deos_actors::Config>::FeeSink::get())
  });
  let stage_parent = parent.clone();
  let terminal = advance(
    &mut storage,
    &mut parent,
    &wasm,
    &signer,
    &[],
    UserDemand::ActorOnly,
  );
  retain_block(&output, &terminal, &mut history);
  let fee = <crate::WeightToFee as WeightToFee>::weight_to_fee(&W::task_add_liquidity());
  let held_terminal = externalities_with_bound_code(storage.clone()).execute_with(|| {
    assert_eq!(System::block_number(), 187_575);
    assert!(Actors::actor_identity(89).is_none()); assert!(Actors::active_actor_state(89).is_none());
    assert!(Actors::actor_semantic_states(89).is_none()); assert!(Actors::actor_run_state(89).is_none());
    assert!(Actors::actor_state_hold(89).is_none());
    assert_eq!(Actors::actor_cost_quote(89).unwrap_err(), pallet_deos_actors::ActorCostQuoteError::ActorNotFound);
    assert!(Actors::deadline_handles(89).is_none()); assert!(Actors::service_nodes(89).is_none());
    let mut failed = 0; let mut action = 0; let mut continued = 0; let mut summary = 0; let mut closed = 0;
    for record in System::events() {
      match record.event {
        RuntimeEvent::Actors(Event::StepFailed {actor_id: 89, cycle_nonce: 1, step_index: 1, retry_class, error}) => {
          assert_eq!(retry_class, RetryClass::Temporary);
          assert_eq!(error.encode(), DispatchError::Other("MinimumLpOutputNotMet").encode()); failed += 1;
        },
        RuntimeEvent::Actors(Event::ActionFeeCharged {actor_id: 89, cycle_nonce: 1, step_index: 1, fee: actual, actual_effect_weight}) => {
          assert_eq!(actual, fee); assert_eq!(actual_effect_weight, W::task_add_liquidity()); action += 1;
        },
        RuntimeEvent::Actors(Event::CycleContinued {actor_id: 89, cycle_nonce: 1, ..}) => continued += 1,
        RuntimeEvent::Actors(Event::CycleSummary {actor_id: 89, cycle_nonce: 1, result, outcomes}) => {
          assert_eq!(result, CycleResult::Failed); assert_eq!(outcomes.failed_steps, 3);
          assert_eq!(outcomes.executed_steps, 1); assert_eq!(outcomes.committed_effectful_tasks, 1); summary += 1;
        },
        RuntimeEvent::Actors(Event::ActorClosed {actor_id: 89, reason}) => {assert_eq!(reason, CloseReason::RetryAttemptsExhausted); closed += 1;},
        RuntimeEvent::Actors(Event::PipelineFeeCharged {actor_id: 89, ..} | Event::CycleStarted {actor_id: 89, ..}
          | Event::CycleSuspended {actor_id: 89, ..} | Event::LiquidityAdded {actor_id: 89, ..}
          | Event::TransferExecuted {actor_id: 89, ..}) => panic!("terminal failure cannot reopen or compensate"),
        _ => {},
      }
    }
    assert_eq!((failed, action, continued, summary, closed), (1, 1, 1, 1, 1));
    assert_eq!(funds[0] - Balances::free_balance(&custody), fee);
    assert_eq!(Assets::balance(0, &custody), funds[1]); assert_eq!(Assets::balance(lp, &custody), funds[2]);
    assert_eq!(Balances::free_balance(<Runtime as pallet_deos_actors::Config>::FeeSink::get()) + fee_sink_distributions() - sink_before, fee);
    assert_eq!(crate::AssetConversion::get_reserves(AssetKind::Native, AssetKind::Local(0)).unwrap(), reserves);
    assert_eq!(<Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<crate::AccountId>>::total_issuance(lp), supply);
    assert_eq!(Actors::actor_identity_count(), identities - 1); assert_eq!(Actors::active_actor_count(), active - 1);
    let ActorClass::User {owner_slot} = before.identity.actor_class else {unreachable!()};
    let mut bitmap = bitmap_before; bitmap[usize::from(owner_slot / 8)] &= !(1 << (owner_slot % 8));
    assert_eq!(Actors::owner_slot_bitmap(&owner), bitmap); assert!(Actors::sovereign_index(&custody).is_none());
    assert!(initial_hold.run > 0); assert_eq!(held_before - owner_held(), hold_total(initial_hold));
    assert_eq!(Actors::active_actor_state(90).unwrap().encode(), cold.encode());
    assert_eq!(Actors::actor_state_hold(90).unwrap().breakdown, cold_hold);
    assert!(Actors::block_resource_state().is_none());
    assert!(!Actors::finalized_block_resource_telemetry().unwrap().optional_actor_work_halted());
    owner_held()
  });
  export_stage(&output, "terminal-failure", &stage_parent, &terminal, &wasm);
  let stage_parent = parent.clone();
  let close = advance(
    &mut storage,
    &mut parent,
    &wasm,
    &signer,
    &[RuntimeCall::Actors(
      pallet_deos_actors::Call::<Runtime>::close_actor { actor_id: 90 },
    )],
    UserDemand::ActorOnly,
  );
  retain_block(&output, &close, &mut history);
  let released = externalities_with_bound_code(storage.clone()).execute_with(|| {
    assert_eq!(System::block_number(), 187_576);
    assert!(Actors::actor_identity(90).is_none()); assert!(Actors::actor_semantic_states(90).is_none());
    assert!(Actors::active_actor_state(90).is_none()); assert!(Actors::actor_run_state(90).is_none());
    assert!(Actors::actor_state_hold(90).is_none());
    assert_eq!(Actors::actor_cost_quote(90).unwrap_err(), pallet_deos_actors::ActorCostQuoteError::ActorNotFound);
    assert!(Actors::service_nodes(90).is_none()); assert!(Actors::deadline_handles(90).is_none());
    assert!(Actors::sovereign_index(&cold.identity.sovereign_account).is_none());
    assert_eq!(Actors::actor_identity_count(), identities - 2); assert_eq!(Actors::active_actor_count(), active - 2);
    let mut bitmap = bitmap_before;
    for class in [before.identity.actor_class, cold.identity.actor_class] {
      let ActorClass::User {owner_slot} = class else {unreachable!()};
      bitmap[usize::from(owner_slot / 8)] &= !(1 << (owner_slot % 8));
    }
    assert_eq!(Actors::owner_slot_bitmap(&owner), bitmap);
    let released = hold_total(cold_hold); assert!(released > 0); assert_eq!(held_terminal - owner_held(), released);
    let ledger_total = pallet_deos_actors::ActorStateHolds::<Runtime>::iter().filter(|(_, h)| h.owner == owner)
      .map(|(_, h)| hold_total(h.breakdown)).sum::<u128>();
    assert_eq!(owner_held(), ledger_total);
    assert_eq!(Balances::free_balance(&custody), funds[0] - fee);
    assert_eq!(Assets::balance(0, &custody), funds[1]); assert_eq!(Assets::balance(lp, &custody), funds[2]);
    let account = &cold.identity.sovereign_account;
    assert_eq!(Balances::free_balance(account), cold_funds[0]);
    assert_eq!(Assets::balance(0, account), cold_funds[1]); assert_eq!(Assets::balance(lp, account), cold_funds[2]);
    assert_eq!(crate::AssetConversion::get_reserves(AssetKind::Native, AssetKind::Local(0)).unwrap(), reserves);
    assert_eq!(<Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<crate::AccountId>>::total_issuance(lp), supply);
    let mut closed = 0;
    for record in System::events() {match record.event {
      RuntimeEvent::Actors(Event::ActorClosed {actor_id: 90, reason}) => {assert_eq!(reason, CloseReason::OwnerInitiated); closed += 1;},
      RuntimeEvent::Actors(Event::ActionFeeCharged {actor_id: 90, ..} | Event::PipelineFeeCharged {actor_id: 90, ..}
        | Event::CycleStarted {actor_id: 90, ..} | Event::CycleSummary {actor_id: 90, ..}
        | Event::TransferExecuted {actor_id: 90, ..} | Event::LiquidityAdded {actor_id: 90, ..}) => panic!("close is custody-neutral process destruction"),
      _ => {},
    }}
    assert_eq!(closed, 1);
    assert!(Actors::block_resource_state().is_none());
    assert!(!Actors::finalized_block_resource_telemetry().unwrap().optional_actor_work_halted());
    released
  });
  export_stage(&output, "owner-close", &stage_parent, &close, &wasm);
  std::fs::write(output.join("result.json"), serde_json::to_vec_pretty(&serde_json::json!({
    "exhaustedActor": 89, "ownerClosedActor": 90, "firstBlock": 187573, "lastBlock": parent.number(), "nativeBlocks": history.len(),
    "terminalCycle": 1, "executedPrefix": 1, "failedAttempts": 3, "terminalActionFee": fee.to_string(),
    "newPipelineFees": 0, "closeActionFee": 0, "closePipelineFee": 0, "exhaustionReleasedHold": hold_total(initial_hold).to_string(), "ownerReleasedHold": released.to_string(),
    "custodyNeutralClose": true, "failedLpEconomicRollback": true, "ownerSlotReleased": true,
    "inheritedSignedNativeAncestry": true, "physicalPricingQualified": false, "completeDomainMaximaQualified": false,
  })).unwrap()).unwrap();
}
