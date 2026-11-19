//! Real paid Running LP target after the public priming successor.

use super::admitted_paired::lp_step;
use super::admitted_priming::{input, retain_block};
use super::admitted_refusal::{advance, transfer};
use super::*;
use codec::DecodeAll;
use pallet_deos_actors::{Event, StepSkippedReason, WeightInfo};
use polkadot_sdk::frame_support::traits::ExecuteBlock;
use polkadot_sdk::sp_consensus_aura::{digests::CompatibleDigestItem, sr25519::AuthoritySignature};
use primitives::AssetKind;

type W = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;

fn fund(storage: &Storage, ids: &[pallet_deos_actors::ActorId]) -> Vec<RuntimeCall> {
  externalities_with_bound_code(storage.clone()).execute_with(|| {
    let paired_min = <Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<
      crate::AccountId,
    >>::minimum_balance(0);
    let lp_fee = <crate::WeightToFee as WeightToFee>::weight_to_fee(&W::task_add_liquidity());
    let mut calls = Vec::new();
    for id in ids {
      let actor = Actors::active_actor_state(*id).unwrap();
      assert!(actor.run_state.is_none() && !actor.hot.pending_signal);
      let q = Actors::actor_cost_quote(*id).unwrap();
      let custody = actor.identity.sovereign_account;
      let native = <Runtime as pallet_deos_actors::Config>::MinUserBalance::get()
        .checked_add(crate::EXISTENTIAL_DEPOSIT)
        .unwrap()
        .checked_add(crate::UNIT)
        .unwrap()
        .checked_add(q.prospective_trigger_fee.unwrap().fee)
        .unwrap()
        .checked_add(q.prospective_pipeline_fee.unwrap().total_fee)
        .unwrap()
        .checked_add(lp_fee)
        .unwrap()
        .saturating_sub(Balances::free_balance(&custody));
      if native > 0 {
        calls.push(transfer(custody.clone(), native));
      }
      let paired = crate::UNIT
        .checked_add(paired_min)
        .unwrap()
        .saturating_sub(Assets::balance(0, &custody));
      if paired > 0 {
        calls.push(RuntimeCall::Assets(
          polkadot_sdk::pallet_assets::Call::transfer {
            id: 0,
            target: MultiAddress::Id(custody),
            amount: paired,
          },
        ));
      }
    }
    calls
  })
}

#[test]
#[ignore = "exact primed successor and signed restored Contracts/funding/real paid Running frontier"]
fn retained_priming_paid_running_refusal_source() {
  let prior = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_RUNNING_SOURCE_PARENT",
        "ff428692f25ee5561994deef0077b2c487d476d956bd6f5cb94948b827c0a6e1",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let mut storage = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_RUNNING_SOURCE_POST",
        "b5a6bff0fc39f0359cdd9f6bbcade29e49943fd0de1477f038e27fc9e8c03b1c",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let sealed = Block::decode_all(
    &mut input(
      "DEOS_RUNNING_SOURCE_BLOCK",
      "0536afa45ea125a39d04281a61fe87dddeb4c9fef86f9fd5ce112db3e891fb4b",
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
  assert_eq!(*parent.number(), 187_520);
  assert_eq!(
    format!("{:x}", parent.hash()),
    "4044a44a2b58b2d00548c930f0ee009e5ec824c5103a836b341cccb486b140ca"
  );
  let wasm = storage.top[well_known_keys::CODE].clone();
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256
  );
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let ids = (15..87).collect::<Vec<pallet_deos_actors::ActorId>>();
  let target = 88;
  let (lp, minimum) = externalities_with_bound_code(storage.clone()).execute_with(|| {
    let lp = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((
      AssetKind::Native,
      AssetKind::Local(0),
    ))
    .unwrap()
    .lp_token;
    let minimum = <Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<
      crate::AccountId,
    >>::minimum_balance(lp);
    assert_eq!(minimum, 1);
    for id in &ids {
      let a = Actors::active_actor_state(*id).unwrap();
      assert_eq!(a.identity.owner, owner);
      assert_eq!(a.identity.cycle_nonce, 2);
      assert!(a.run_state.is_none() && !a.hot.pending_signal);
      assert_eq!(a.contract.steps.len(), 1);
      assert_eq!(Assets::balance(lp, &a.identity.sovereign_account), minimum);
    }
    let a = Actors::active_actor_state(target).unwrap();
    assert_eq!(a.identity.owner, owner);
    assert_eq!(a.identity.cycle_nonce, 0);
    assert!(a.run_state.is_none() && !a.hot.pending_signal);
    assert_eq!(Assets::balance(lp, &a.identity.sovereign_account), 0);
    (lp, minimum)
  });
  let output = std::path::PathBuf::from(
    std::env::var("DEOS_PAID_RUNNING_OUTPUT").expect("new source output directory"),
  );
  std::fs::create_dir(&output).unwrap();
  std::fs::create_dir(output.join("native-blocks")).unwrap();
  let mut history = Vec::new();
  for batch in ids.chunks(4) {
    let calls = externalities_with_bound_code(storage.clone()).execute_with(|| batch.iter().map(|id| {
      let mut contract = Actors::active_actor_state(*id).unwrap().contract;
      let skipped = contract.steps[0].clone();
      assert!(matches!(skipped.task, Task::Transfer {asset: AssetKind::Local(asset), ..} if asset == lp));
      contract.steps = std::iter::repeat_n(skipped, if *id < 51 {2} else {1})
        .chain(std::iter::once(lp_step())).collect::<Vec<_>>().try_into().unwrap();
      RuntimeCall::Actors(pallet_deos_actors::Call::update_contract { actor_id: *id, contract })
    }).collect::<Vec<_>>());
    let block = advance(
      &mut storage,
      &mut parent,
      &wasm,
      &signer,
      &calls,
      UserDemand::ActorOnly,
    );
    retain_block(&output, &block, &mut history);
    let calls = fund(&storage, batch);
    let block = advance(
      &mut storage,
      &mut parent,
      &wasm,
      &signer,
      &calls,
      UserDemand::ActorOnly,
    );
    retain_block(&output, &block, &mut history);
  }
  let contract = externalities_with_bound_code(storage.clone()).execute_with(|| {
    let mut contract = Actors::active_actor_state(target).unwrap().contract;
    let mut retry = lp_step();
    retry.on_error = StepErrorPolicy::RetryLater { max_attempts: 2 };
    contract.steps = vec![
      StepOf::<Runtime> {
        precondition: None,
        task: Task::Transfer {
          to: owner.clone(),
          asset: AssetKind::Local(lp),
          amount: AmountResolution::Percent(Perbill::one()),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
      retry,
    ]
    .try_into()
    .unwrap();
    contract
  });
  let block = advance(
    &mut storage,
    &mut parent,
    &wasm,
    &signer,
    &[RuntimeCall::Actors(
      pallet_deos_actors::Call::update_contract {
        actor_id: target,
        contract,
      },
    )],
    UserDemand::ActorOnly,
  );
  retain_block(&output, &block, &mut history);
  let calls = fund(&storage, &[target]);
  let block = advance(
    &mut storage,
    &mut parent,
    &wasm,
    &signer,
    &calls,
    UserDemand::ActorOnly,
  );
  retain_block(&output, &block, &mut history);
  // Keep the User observation after the ordinary two-leg Fee Sink distribution, not inside it.
  // These are full ordinary predecessors; no scheduler clock, System state or budget is patched.
  for _ in 0..2 {
    let block = advance(
      &mut storage,
      &mut parent,
      &wasm,
      &signer,
      &[],
      UserDemand::ActorOnly,
    );
    retain_block(&output, &block, &mut history);
  }
  let all = ids
    .iter()
    .copied()
    .chain(std::iter::once(target))
    .collect::<Vec<_>>();
  let mut opened = BTreeSet::new();
  let mut skipped = 0;
  for batch in [
    ids[..36].to_vec(),
    ids[36..]
      .iter()
      .copied()
      .chain(std::iter::once(target))
      .collect::<Vec<_>>(),
    Vec::new(),
  ] {
    let calls = batch
      .iter()
      .map(|id| RuntimeCall::Actors(pallet_deos_actors::Call::manual_trigger { actor_id: *id }))
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
    externalities_with_bound_code(storage.clone()).execute_with(|| {
      for record in System::events() {
        match record.event {
          RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id, fee })
            if all.contains(&actor_id) =>
          {
            assert!(fee > 0);
            assert!(opened.insert(actor_id));
          }
          RuntimeEvent::Actors(Event::StepSkipped {
            actor_id,
            reason: StepSkippedReason::ResolutionSkipped,
            ..
          }) if all.contains(&actor_id) => skipped += 1,
          RuntimeEvent::Actors(
            Event::StepSkipped { actor_id, .. }
            | Event::StepFailed { actor_id, .. }
            | Event::ActorClosed { actor_id, .. }
            | Event::ActionFeeCharged { actor_id, .. }
            | Event::TransferExecuted { actor_id, .. }
            | Event::LiquidityAdded { actor_id, .. },
          ) if all.contains(&actor_id) => panic!(
            "warmup must only resolve zero preservable LP: {:?}",
            record.event
          ),
          _ => {}
        }
      }
    });
  }
  std::fs::write(output.join("parent.scale"), storage.top.encode()).unwrap();
  std::fs::write(output.join("parent-header.scale"), parent.encode()).unwrap();
  let parent_rows = externalities_with_bound_code(storage.clone()).execute_with(|| all.iter().map(|id| {
    let a = Actors::active_actor_state(*id).unwrap();
    serde_json::json!({"actor": id, "completedNonce": a.identity.cycle_nonce, "run": format!("{:?}", a.run_state),
      "lpBalance": Assets::balance(lp, &a.identity.sovereign_account).to_string(), "pending": a.hot.pending_signal})
  }).collect::<Vec<_>>());
  std::fs::write(
    output.join("parent.json"),
    serde_json::to_vec_pretty(&serde_json::json!({
      "block": parent.number(), "skipped": skipped, "opened": opened, "actors": parent_rows,
      "runningTargetQualified": false, "physicalPricingQualified": false,
    }))
    .unwrap(),
  )
  .unwrap();
  assert_eq!(skipped, 109);
  assert_eq!(opened, all.iter().copied().collect());
  let target_before = externalities_with_bound_code(storage.clone()).execute_with(|| {
    for (index, id) in all.iter().enumerate() {
      let a = Actors::active_actor_state(*id).unwrap();
      let run = a.run_state.as_ref().unwrap();
      assert_eq!(a.hot.cycle_state, CycleState::Running);
      assert!(!a.hot.pending_signal);
      assert_eq!(a.hot.unsuccessful_attempt_streak, 0);
      assert_eq!(run.unsuccessful_attempts_at_cursor, 0);
      assert!(run.suspension.is_none());
      assert_eq!(run.cursor, if index < 36 { 2 } else { 1 });
      assert_eq!(a.identity.cycle_nonce, if *id == target { 0 } else { 2 });
      assert_eq!(run.cycle_nonce, if *id == target { 1 } else { 3 });
      assert_eq!(
        Assets::balance(lp, &a.identity.sovereign_account),
        if *id == target { 0 } else { minimum }
      );
    }
    let a = Actors::active_actor_state(target).unwrap();
    assert_eq!(a.contract.cooldown_blocks, 0);
    assert!(
      a.contract.window.is_none()
        && a.contract.close_when.is_none()
        && a.contract.auto_close_at_cycle_nonce.is_none()
    );
    assert_eq!(
      a.contract.steps[1].on_error,
      StepErrorPolicy::RetryLater { max_attempts: 2 }
    );
    (
      a.encode(),
      Balances::free_balance(&a.identity.sovereign_account),
      Assets::balance(0, &a.identity.sovereign_account),
      Assets::balance(lp, &a.identity.sovereign_account),
      Actors::actor_state_hold(target).unwrap().encode(),
    )
  });
  let selected_parent = parent.clone();
  let selected = advance(
    &mut storage,
    &mut parent,
    &wasm,
    &signer,
    &[],
    UserDemand::ContinuousShared,
  );
  retain_block(&output, &selected, &mut history);
  std::fs::write(output.join("post.scale"), storage.top.encode()).unwrap();
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
  let (actions, fees, summaries, cursor) = externalities_with_bound_code(storage.clone())
    .execute_with(|| {
      let mut actions = Vec::new();
      let mut fees = Vec::new();
      let mut summaries = Vec::new();
      for record in System::events() {
        match record.event {
          RuntimeEvent::Actors(Event::LiquidityAdded {
            actor_id,
            lp_minted,
            ..
          }) if all.contains(&actor_id) => {
            assert!(lp_minted > 0);
            actions.push(actor_id);
          }
          RuntimeEvent::Actors(Event::ActionFeeCharged { actor_id, fee, .. })
            if all.contains(&actor_id) =>
          {
            assert!(fee > 0);
            fees.push(actor_id);
          }
          RuntimeEvent::Actors(Event::CycleSummary {
            actor_id,
            cycle_nonce,
            ..
          }) if all.contains(&actor_id) => {
            assert_eq!(cycle_nonce, 3);
            summaries.push(actor_id);
          }
          RuntimeEvent::Actors(
            Event::StepSkipped { actor_id, .. }
            | Event::StepFailed { actor_id, .. }
            | Event::PipelineFeeCharged { actor_id, .. },
          ) if all.contains(&actor_id) => panic!(
            "paid prefixes cannot fail or repay Opening: {:?}",
            record.event
          ),
          _ => {}
        }
      }
      (actions, fees, summaries, Actors::service_header().cursor)
    });
  std::fs::write(output.join("result-before-assertions.json"), serde_json::to_vec_pretty(&serde_json::json!({
    "block": parent.number(), "actions": actions, "fees": fees, "summaries": summaries, "cursor": format!("{cursor:?}"), "acceptance": false,
  })).unwrap()).unwrap();
  assert_eq!(actions, ids);
  assert_eq!(fees, ids);
  assert_eq!(summaries, ids);
  assert_eq!(cursor.unwrap().actor_id, target);
  externalities_with_bound_code(storage.clone()).execute_with(|| {
    for id in &ids {
      let a = Actors::active_actor_state(*id).unwrap();
      assert_eq!(a.identity.cycle_nonce, 3);
      assert_eq!(a.hot.cycle_state, CycleState::Idle);
      assert!(a.run_state.is_none() && !a.hot.pending_signal);
      assert!(Assets::balance(lp, &a.identity.sovereign_account) > minimum);
      assert_eq!(
        Actors::actor_state_hold(*id).unwrap().breakdown,
        Actors::actor_cost_quote(*id)
          .unwrap()
          .actor_state_hold
          .breakdown
      );
    }
    let a = Actors::active_actor_state(target).unwrap();
    assert_eq!(
      (
        a.encode(),
        Balances::free_balance(&a.identity.sovereign_account),
        Assets::balance(0, &a.identity.sovereign_account),
        Assets::balance(lp, &a.identity.sovereign_account),
        Actors::actor_state_hold(target).unwrap().encode()
      ),
      target_before
    );
    assert!(Actors::block_resource_state().is_none());
    assert!(
      !Actors::finalized_block_resource_telemetry()
        .unwrap()
        .optional_actor_work_halted()
    );
  });
  replay_complete_block_in_wasm(&selected, &wasm, "public-priming-paid-running-lp-refusal");
  std::fs::write(output.join("result.json"), serde_json::to_vec_pretty(&serde_json::json!({
    "block": parent.number(), "target": target, "actions": actions, "prefixNonce": 3, "targetCompletedNonce": 0, "targetRunNonce": 1,
    "nativeBlocks": history.len(), "parentRoot": format!("{:?}", selected_parent.state_root()), "postRoot": format!("{:?}", parent.state_root()),
    "inheritedSignedNativeAncestry": true, "paidRunningFrontierQualified": true,
    "pairedBookingQualified": false, "actualRetryTransitionQualified": false, "physicalPricingQualified": false,
  })).unwrap()).unwrap();
}

#[test]
#[ignore = "exact paid Running refusal source and its genuine ordinary next block"]
fn retained_paid_running_successor_k() {
  let prior = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_RUNNING_K_SOURCE_PARENT",
        "a2a777adb5279351f990dded85da96acac1ffee9e7a22c80264d55980fec6b88",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let mut storage = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_RUNNING_K_SOURCE_POST",
        "b4702064d9831c8dd0c613fcdc0caf6142ab353d02ff69bf364a3de99e61399a",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let sealed = Block::decode_all(
    &mut input(
      "DEOS_RUNNING_K_SOURCE_BLOCK",
      "b1d447323ccc4ed14134ab41040137cc6f3ccaa79cecc95b37b0376b94dc06e8",
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
  assert_eq!(*parent.number(), 187_564);
  assert_eq!(
    format!("{:x}", parent.hash()),
    "dcb0674a01e208769169f06257a104dd505a406a10086788fe820aec540c60d9"
  );
  let wasm = storage.top[well_known_keys::CODE].clone();
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256
  );
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let (before, action_fee, lp, native_before, paired_before, lp_before, sink_before) =
    externalities_with_bound_code(storage.clone()).execute_with(|| {
      let actor = Actors::active_actor_state(88).unwrap();
      assert_eq!(actor.identity.cycle_nonce, 0);
      assert_eq!(actor.hot.cycle_state, CycleState::Running);
      assert!(!actor.hot.pending_signal);
      let run = actor.run_state.as_ref().unwrap();
      assert_eq!(run.cursor, 1);
      assert_eq!(run.cycle_nonce, 1);
      assert_eq!(run.unsuccessful_attempts_at_cursor, 0);
      assert!(run.suspension.is_none());
      assert_eq!(
        actor.contract.steps[1].on_error,
        StepErrorPolicy::RetryLater { max_attempts: 2 }
      );
      let lp = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((
        AssetKind::Native,
        AssetKind::Local(0),
      ))
      .unwrap()
      .lp_token;
      let custody = &actor.identity.sovereign_account;
      (
        actor.clone(),
        Actors::actor_cost_quote(88)
          .unwrap()
          .maximum_next_action_fee
          .maximum_effect_fee,
        lp,
        Balances::free_balance(custody),
        Assets::balance(0, custody),
        Assets::balance(lp, custody),
        Balances::free_balance(<Runtime as pallet_deos_actors::Config>::FeeSink::get()),
      )
    });
  let output =
    std::path::PathBuf::from(std::env::var("DEOS_RUNNING_K_OUTPUT").expect("new output directory"));
  std::fs::create_dir(&output).unwrap();
  std::fs::write(output.join("parent.scale"), storage.top.encode()).unwrap();
  std::fs::write(output.join("parent-header.scale"), parent.encode()).unwrap();
  let selected_parent = parent.clone();
  let selected = advance(
    &mut storage,
    &mut parent,
    &wasm,
    &signer,
    &[],
    UserDemand::ActorOnly,
  );
  std::fs::write(output.join("post.scale"), storage.top.encode()).unwrap();
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
  let minted = externalities_with_bound_code(storage.clone()).execute_with(|| {
    let mut actions = Vec::new();
    let mut outputs = Vec::new();
    let mut summaries = Vec::new();
    for record in System::events() {
      match record.event {
        RuntimeEvent::Actors(Event::ActionFeeCharged {
          actor_id: 88,
          cycle_nonce,
          step_index,
          fee,
          ..
        }) => {
          assert_eq!((cycle_nonce, step_index), (1, 1));
          actions.push(fee);
        }
        RuntimeEvent::Actors(Event::LiquidityAdded {
          actor_id: 88,
          lp_minted,
          ..
        }) => outputs.push(lp_minted),
        RuntimeEvent::Actors(Event::CycleSummary {
          actor_id: 88,
          cycle_nonce,
          ..
        }) => summaries.push(cycle_nonce),
        RuntimeEvent::Actors(
          Event::PipelineFeeCharged { actor_id: 88, .. }
          | Event::CycleStarted { actor_id: 88, .. }
          | Event::StepSkipped { actor_id: 88, .. }
          | Event::StepFailed { actor_id: 88, .. }
          | Event::CycleSuspended { actor_id: 88, .. }
          | Event::ActorClosed { actor_id: 88, .. },
        ) => panic!(
          "paid Running successor cannot reopen, retry, skip or close: {:?}",
          record.event
        ),
        _ => {}
      }
    }
    assert_eq!(actions, [action_fee]);
    assert!(action_fee > 0);
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0] > 0);
    assert_eq!(summaries, [1]);
    let actor = Actors::active_actor_state(88).unwrap();
    assert_eq!(actor.identity.cycle_nonce, 1);
    assert_eq!(actor.hot.cycle_state, CycleState::Idle);
    assert!(actor.run_state.is_none() && !actor.hot.pending_signal);
    assert_eq!(actor.contract, before.contract);
    assert_eq!(
      actor.identity.sovereign_account,
      before.identity.sovereign_account
    );
    assert_eq!(
      native_before - Balances::free_balance(&actor.identity.sovereign_account),
      crate::UNIT + action_fee
    );
    assert_eq!(
      paired_before - Assets::balance(0, &actor.identity.sovereign_account),
      crate::UNIT
    );
    assert_eq!(
      Assets::balance(lp, &actor.identity.sovereign_account) - lp_before,
      outputs[0]
    );
    assert_eq!(
      Balances::free_balance(<Runtime as pallet_deos_actors::Config>::FeeSink::get()) - sink_before,
      action_fee
    );
    assert_eq!(
      Actors::actor_state_hold(88).unwrap().breakdown,
      Actors::actor_cost_quote(88)
        .unwrap()
        .actor_state_hold
        .breakdown
    );
    let telemetry = Actors::finalized_block_resource_telemetry().unwrap();
    assert!(!telemetry.optional_actor_work_halted());
    assert!(Actors::block_resource_state().is_none());
    assert_eq!(
      telemetry.usage().actor_effect_used(),
      W::task_add_liquidity()
    );
    outputs[0]
  });
  replay_complete_block_in_wasm(&selected, &wasm, "paid-running-successor-k");
  std::fs::write(output.join("result.json"), serde_json::to_vec_pretty(&serde_json::json!({
    "block": parent.number(), "target": 88, "completedNonceBefore": 0, "runNonceBefore": 1, "completedNonceAfter": 1,
    "newPipelineFee": "0", "actionFee": action_fee.to_string(), "lpMinted": minted.to_string(),
    "parentRoot": format!("{:?}", selected_parent.state_root()), "postRoot": format!("{:?}", parent.state_root()),
    "inheritedSignedNativeAncestry": true, "actualRetryTransitionQualified": false,
    "phaseKQualified": false, "completeKMaximumQualified": false, "physicalPricingQualified": false,
  })).unwrap()).unwrap();
}
