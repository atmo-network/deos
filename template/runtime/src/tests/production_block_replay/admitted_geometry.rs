//! Genuine signed admission/encoding preflight; declarations are not physical maxima.

use super::admitted_priming::{input, retain_block};
use super::admitted_refusal::advance;
use super::*;
use codec::DecodeAll;
use pallet_deos_actors::{
  ActorContractHeads, ActorContractTailChunks, Config, FundingSourcePolicy, Precondition,
  Predicate, ScheduleWindow, StepControlWeightContext, StepControlWeightProvider,
};
use polkadot_sdk::frame_support::traits::ExecuteBlock;
use polkadot_sdk::sp_consensus_aura::{digests::CompatibleDigestItem, sr25519::AuthoritySignature};

type Control = crate::configs::actor_config::RuntimeStepControlWeight;

#[test]
#[ignore = "exact lifecycle checkpoint and one genuine signed larger-geometry admission block"]
fn retained_legal_geometry_admission_preflight() {
  let prior = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_GEOMETRY_SOURCE_PARENT",
        "48978e14e0bed484c04c37d90e481039304cf0f52c28caeae396162fe5a02143",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let mut storage = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_GEOMETRY_SOURCE_POST",
        "fe468faca5524cf814293780495e61fbc6e3cbacb2d5e86fc1e45e4946f70b9d",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let sealed = Block::decode_all(
    &mut input(
      "DEOS_GEOMETRY_SOURCE_BLOCK",
      "29fc4b2b3436afac8cf0b3740741e31bbeabaa4899bd1662458ce37136727db1",
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
  assert_eq!(*parent.number(), 187_576);
  assert_eq!(
    format!("{:x}", parent.hash()),
    "67112aeb064e1b4ed90e0294281aea400f61660815d2530d639be5aebeda5144"
  );
  let wasm = storage.top[well_known_keys::CODE].clone();
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256
  );
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let (before, old_head, old_hold) =
    externalities_with_bound_code(storage.clone()).execute_with(|| {
      let before = Actors::active_actor_state(91).unwrap();
      assert_eq!(before.identity.owner, owner);
      assert_eq!(before.identity.cycle_nonce, 0);
      assert!(before.run_state.is_none() && !before.hot.pending_signal);
      assert_eq!(before.contract.steps.len(), 1);
      assert_eq!(<Runtime as Config>::MaxContractSteps::get(), 12);
      assert_eq!(<Runtime as Config>::MaxPreconditionClauses::get(), 4);
      assert_eq!(<Runtime as Config>::MaxPredicatesPerClause::get(), 4);
      assert_eq!(<Runtime as Config>::MaxPredicatesPerStep::get(), 4);
      (
        before,
        ActorContractHeads::<Runtime>::get(91).unwrap(),
        Actors::actor_state_hold(91).unwrap().breakdown,
      )
    });
  let mut predicates = vec![
    Predicate::BalanceAbove {
      asset: primitives::AssetKind::Native,
      threshold: 0,
    },
    Predicate::BalanceAbove {
      asset: primitives::AssetKind::Local(0),
      threshold: 0,
    },
    Predicate::BlockNumberAbove { threshold: 0 },
    Predicate::BlockNumberBelow {
      threshold: u32::MAX,
    },
  ];
  predicates.sort_by_key(Encode::encode);
  let condition: pallet_deos_actors::PreconditionOf<Runtime> = Precondition {
    clauses: vec![predicates.try_into().unwrap()].try_into().unwrap(),
  };
  assert_eq!(condition.evaluation_units(), 4);
  let mut contract = before.contract.clone();
  let mut lp = super::admitted_paired::lp_step();
  lp.precondition = Some(condition.clone());
  lp.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
  if let Task::AddLiquidity { min_lp_out, .. } = &mut lp.task {
    *min_lp_out = 2 * crate::UNIT;
  } else {
    unreachable!()
  }
  let mut steps = vec![lp; <Runtime as Config>::MaxContractSteps::get() as usize];
  steps[0] = StepOf::<Runtime> {
    precondition: Some(condition),
    task: Task::Transfer {
      to: owner.clone(),
      asset: primitives::AssetKind::Native,
      amount: AmountResolution::Fixed(1),
    },
    on_error: StepErrorPolicy::AbortCycle,
  };
  contract.steps = steps.try_into().unwrap();
  contract.cooldown_blocks = 2;
  let start = parent.number().checked_add(1).unwrap();
  contract.window = Some(ScheduleWindow {
    start,
    end: start + <Runtime as Config>::MinWindowLength::get(),
  });
  let allowed = (1u8..16)
    .map(|i| crate::AccountId::new([i; 32]))
    .chain([owner.clone()])
    .collect::<BTreeSet<_>>();
  contract.funding = FundingSourcePolicy::SignedAllowlist(allowed.try_into().unwrap());
  assert_eq!(
    contract.trigger,
    pallet_deos_actors::TriggerOf::<Runtime>::Manual
  );
  let output =
    std::path::PathBuf::from(std::env::var("DEOS_GEOMETRY_OUTPUT").expect("new geometry output"));
  std::fs::create_dir(&output).unwrap();
  std::fs::create_dir(output.join("native-blocks")).unwrap();
  std::fs::write(output.join("candidate-contract.scale"), contract.encode()).unwrap();
  let stage_parent = parent.clone();
  let selected = advance(
    &mut storage,
    &mut parent,
    &wasm,
    &signer,
    &[RuntimeCall::Actors(
      pallet_deos_actors::Call::<Runtime>::update_contract {
        actor_id: 91,
        contract: contract.clone(),
      },
    )],
    UserDemand::ActorOnly,
  );
  retain_block(&output, &selected, &mut Vec::new());
  std::fs::write(output.join("parent.scale"), selected.pre_state.top.encode()).unwrap();
  std::fs::write(output.join("post.scale"), selected.post_state.top.encode()).unwrap();
  std::fs::write(output.join("parent-header.scale"), stage_parent.encode()).unwrap();
  std::fs::create_dir(output.join("selected")).unwrap();
  pvf_fixture::export(
    &output.join("selected"),
    &polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into(),
    (
      selected.pre_state.clone(),
      selected.post_state.clone(),
      &stage_parent,
      selected.block.clone(),
      selected.inherent_data.clone(),
    ),
  )
  .unwrap();
  externalities_with_bound_code(storage.clone()).execute_with(|| {
    let after = Actors::active_actor_state(91).unwrap();
    assert_eq!(after.contract, contract);
    assert_eq!(after.identity.cycle_nonce, 0); assert!(after.run_state.is_none() && !after.hot.pending_signal);
    let head = ActorContractHeads::<Runtime>::get(91).unwrap();
    assert_eq!(head.header.step_count, 12);
    assert_eq!(head.first_step.as_ref(), contract.steps.first());
    let chunks = ActorContractTailChunks::<Runtime>::iter_prefix(91).collect::<BTreeMap<_, _>>();
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks.values().map(|chunk| chunk.steps.len()).collect::<Vec<_>>(), [4, 4, 3]);
    assert_eq!(head.header.semantic_contract_id, contract.semantic_contract_id());
    assert_eq!(head.header.body_commitment, contract.body_commitment().unwrap());
    for step in &contract.steps {assert_eq!(step.precondition.as_ref().unwrap().evaluation_units(), 4);}
    let mut declarations = Vec::new();
    for cursor in [0u32, 1, 5, 9, 11] {
      let (fragment, resource) = if cursor == 0 {(1, head.first_step_resources.unwrap())} else {
        let chunk = &chunks[&((cursor - 1) / 4)];
        (chunk.steps.len() as u32, chunk.step_resources[(cursor - chunk.first_step_index) as usize])
      };
      let context = StepControlWeightContext {cursor, steps_in_fragment: fragment,
        opening_tail_chunks: if cursor == 0 {3} else {0}, predicate_evaluation_units: 4};
      assert_eq!(resource.control, Control::maximum_control_weight(context, &contract.steps[cursor as usize]).unwrap());
      declarations.push(serde_json::json!({"cursor": cursor, "fragmentSteps": fragment,
        "controlProvider": [resource.control.ref_time(), resource.control.proof_size()],
        "effectProvider": [resource.effect.ref_time(), resource.effect.proof_size()]}));
    }
    let hold = Actors::actor_state_hold(91).unwrap().breakdown;
    assert_eq!(hold, Actors::actor_cost_quote(91).unwrap().actor_state_hold.breakdown);
    assert!(hold.contract_head > old_hold.contract_head); assert!(hold.contract_body > old_hold.contract_body);
    assert!(System::events().iter().all(|r| !matches!(r.event, RuntimeEvent::Actors(
      pallet_deos_actors::Event::PipelineFeeCharged {actor_id: 91, ..} | pallet_deos_actors::Event::ActionFeeCharged {actor_id: 91, ..}
      | pallet_deos_actors::Event::CycleStarted {actor_id: 91, ..} | pallet_deos_actors::Event::StepFailed {actor_id: 91, ..}))));
    let encoded_chunks = chunks.iter().map(|(index, chunk)| serde_json::json!({
      "index": index, "steps": chunk.steps.len(), "bytes": chunk.encoded_size(),
    })).collect::<Vec<_>>();
    assert!(head.encoded_size() > old_head.encoded_size());
    std::fs::write(output.join("head.scale"), head.encode()).unwrap();
    std::fs::write(output.join("tail-chunks.scale"), chunks.encode()).unwrap();
    std::fs::write(output.join("result.json"), serde_json::to_vec_pretty(&serde_json::json!({
      "actor": 91, "block": parent.number(), "steps": contract.steps.len(), "predicatesPerStep": 4,
      "clausesPerStep": 1, "fundingSources": 16, "window": [start, start + <Runtime as Config>::MinWindowLength::get()],
      "contractBytes": contract.encoded_size(), "headBytes": head.encoded_size(), "previousHeadBytes": old_head.encoded_size(),
      "tailChunks": encoded_chunks, "declarations": declarations,
      "nextPipelineFee": Actors::actor_cost_quote(91).unwrap().prospective_pipeline_fee.unwrap().total_fee.to_string(),
      "signedAdmissionQualified": true, "largerTaskExecutionQualified": false, "cartesianMaximaQualified": false,
      "physicalPricingQualified": false, "completeDomainMaximaQualified": false,
    })).unwrap()).unwrap();
  });
  replay_complete_block_in_wasm(&selected, &wasm, "larger-geometry-admission-preflight");
}

fn export_execution_stage(
  output: &std::path::Path,
  name: &str,
  parent: &Header,
  block: &AuthoredBlock,
  wasm: &[u8],
) {
  let stage = output.join(name);
  std::fs::create_dir(&stage).unwrap();
  std::fs::write(stage.join("parent.scale"), block.pre_state.top.encode()).unwrap();
  std::fs::write(stage.join("post.scale"), block.post_state.top.encode()).unwrap();
  std::fs::write(stage.join("parent-header.scale"), parent.encode()).unwrap();
  std::fs::create_dir(stage.join("selected")).unwrap();
  pvf_fixture::export(
    &stage.join("selected"),
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

fn distributed_native() -> u128 {
  System::events()
    .into_iter()
    .filter_map(|r| match r.event {
      RuntimeEvent::Actors(pallet_deos_actors::Event::TransferExecuted {
        actor_id,
        asset: primitives::AssetKind::Native,
        amount,
        ..
      }) if actor_id == primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID => Some(amount),
      _ => None,
    })
    .sum()
}

#[test]
#[ignore = "exact larger-geometry checkpoint and real predicate-heavy paid retry/exhaustion suffix"]
fn retained_geometry_predicate_retry_exhaustion() {
  use super::admitted_refusal::transfer;
  use pallet_deos_actors::{
    ActorClass, CloseReason, CycleResult, Event, RetryClass, SuspensionReason, WakeupKey,
    WeightInfo,
  };
  use polkadot_sdk::frame_support::traits::fungible::InspectHold;
  type W = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
  let prior = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_GEOMETRY_RUN_PARENT",
        "fe468faca5524cf814293780495e61fbc6e3cbacb2d5e86fc1e45e4946f70b9d",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let mut storage = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_GEOMETRY_RUN_POST",
        "892611a77475a89d5c606d363f6d0a2a9d0b85f86e1ce2aa9b1189be3e3b6645",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let sealed = Block::decode_all(
    &mut input(
      "DEOS_GEOMETRY_RUN_BLOCK",
      "fb8fdb7c40d2dee384c2f8892bf67380af59508c85ff714a42ca8cf14a9077e5",
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
  assert_eq!(*parent.number(), 187_577);
  assert_eq!(
    format!("{:x}", parent.hash()),
    "28479ddc73461cffca31fb76755ad36123eb6d14f0c767a2d392b9fac949e6a4"
  );
  let wasm = storage.top[well_known_keys::CODE].clone();
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(&wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256
  );
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let owner = crate::AccountId::from(signer.public());
  let sink = <Runtime as Config>::FeeSink::get();
  let fee = <crate::WeightToFee as WeightToFee>::weight_to_fee(&W::task_add_liquidity());
  let prefix_fee = <crate::WeightToFee as WeightToFee>::weight_to_fee(&W::task_transfer());
  let (before, q, custody, lp, extra_native, extra_paired) =
    externalities_with_bound_code(storage.clone()).execute_with(|| {
      let before = Actors::active_actor_state(91).unwrap();
      assert_eq!(before.identity.owner, owner);
      assert_eq!(before.identity.cycle_nonce, 0);
      assert!(before.run_state.is_none() && !before.hot.pending_signal);
      assert_eq!(before.contract.steps.len(), 12);
      assert_eq!(
        before.contract.window,
        Some(ScheduleWindow {
          start: 187577,
          end: 187677
        })
      );
      assert_eq!(before.contract.cooldown_blocks, 2);
      for step in &before.contract.steps {
        assert_eq!(step.precondition.as_ref().unwrap().evaluation_units(), 4);
      }
      let custody = before.identity.sovereign_account.clone();
      let q = Actors::actor_cost_quote(91).unwrap();
      let required = <Runtime as Config>::MinUserBalance::get()
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
        .checked_add(fee.checked_mul(3).unwrap())
        .unwrap();
      let minimum = <Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<
        crate::AccountId,
      >>::minimum_balance(0);
      let paired = crate::UNIT
        .checked_add(minimum)
        .unwrap()
        .saturating_sub(Assets::balance(0, &custody));
      let lp = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((
        primitives::AssetKind::Native,
        primitives::AssetKind::Local(0),
      ))
      .unwrap()
      .lp_token;
      (
        before,
        q,
        custody.clone(),
        lp,
        required.saturating_sub(Balances::free_balance(&custody)),
        paired,
      )
    });
  let output = std::path::PathBuf::from(
    std::env::var("DEOS_GEOMETRY_RUN_OUTPUT").expect("new execution directory"),
  );
  std::fs::create_dir(&output).unwrap();
  std::fs::create_dir(output.join("native-blocks")).unwrap();
  let mut history = Vec::new();
  let mut stages = Vec::new();
  let mut calls = Vec::new();
  if extra_native > 0 {
    calls.push(transfer(custody.clone(), extra_native));
  }
  if extra_paired > 0 {
    calls.push(RuntimeCall::Assets(
      polkadot_sdk::pallet_assets::Call::transfer {
        id: 0,
        target: MultiAddress::Id(custody.clone()),
        amount: extra_paired,
      },
    ));
  }
  calls.push(RuntimeCall::Actors(
    pallet_deos_actors::Call::manual_trigger { actor_id: 91 },
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
  let (funded, paired, lp_balance, reserves, supply, sink_prefix) =
    externalities_with_bound_code(storage.clone()).execute_with(|| {
      let actor = Actors::active_actor_state(91).unwrap();
      assert!(actor.hot.pending_signal && actor.run_state.is_none());
      assert_eq!(actor.contract, before.contract);
      assert_eq!(actor.identity.cycle_nonce, 0);
      let reserves = crate::AssetConversion::get_reserves(
        primitives::AssetKind::Native,
        primitives::AssetKind::Local(0),
      )
      .unwrap();
      let supply = <Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<
        crate::AccountId,
      >>::total_issuance(lp);
      assert_eq!(reserves.0, reserves.1);
      assert_eq!(
        supply, reserves.0,
        "actual minted UNIT is below authored 2UNIT"
      );
      (
        Balances::free_balance(&custody),
        Assets::balance(0, &custody),
        Assets::balance(lp, &custody),
        reserves,
        supply,
        Balances::free_balance(&sink),
      )
    });
  let stage_parent = parent.clone();
  let prefix = advance(
    &mut storage,
    &mut parent,
    &wasm,
    &signer,
    &[],
    UserDemand::ActorOnly,
  );
  retain_block(&output, &prefix, &mut history);
  let prefix_run = externalities_with_bound_code(storage.clone()).execute_with(|| {
    let mut transfers = 0;
    let mut pipelines = 0;
    let mut actions = 0;
    for record in System::events() {
      match record.event {
        RuntimeEvent::Actors(Event::TransferExecuted {
          actor_id: 91,
          amount,
          asset,
          to,
          cycle_nonce,
          step_index,
        }) => {
          assert_eq!((cycle_nonce, step_index, amount), (1, 0, 1));
          assert_eq!(asset, primitives::AssetKind::Native);
          assert_eq!(to, owner);
          transfers += 1;
        }
        RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id: 91, fee }) => {
          assert_eq!(fee, q.prospective_pipeline_fee.unwrap().total_fee);
          pipelines += 1;
        }
        RuntimeEvent::Actors(Event::ActionFeeCharged {
          actor_id: 91,
          fee,
          actual_effect_weight,
          ..
        }) => {
          assert_eq!(fee, prefix_fee);
          assert_eq!(actual_effect_weight, W::task_transfer());
          actions += 1;
        }
        RuntimeEvent::Actors(
          Event::StepFailed { actor_id: 91, .. } | Event::StepSkipped { actor_id: 91, .. },
        ) => panic!("all four prefix predicates must pass for the real Transfer"),
        _ => {}
      }
    }
    assert_eq!((transfers, pipelines, actions), (1, 1, 1));
    assert_eq!(
      funded - Balances::free_balance(&custody),
      1 + prefix_fee + q.prospective_pipeline_fee.unwrap().total_fee
    );
    assert_eq!(
      Balances::free_balance(&sink) + distributed_native() - sink_prefix,
      prefix_fee + q.prospective_pipeline_fee.unwrap().total_fee
    );
    let actor = Actors::active_actor_state(91).unwrap();
    assert_eq!(actor.identity.cycle_nonce, 0);
    let run = actor.run_state.unwrap();
    assert_eq!((run.cycle_nonce, run.cursor), (1, 1));
    assert_eq!(run.last_committed_step_block, Some(*parent.number()));
    assert_eq!(run.cumulative_outcomes.committed_effectful_tasks, 1);
    run
  });
  export_execution_stage(&output, "opening-prefix", &stage_parent, &prefix, &wasm);
  let prefix_block = *parent.number();
  let reason: crate::RuntimeHoldReason = pallet_deos_actors::HoldReason::ActorState.into();
  let mut released_hold = 0;
  for attempt in 1..=3 {
    let (native_before, sink_before, held_before, hold, count, active, bitmap) =
      externalities_with_bound_code(storage.clone()).execute_with(|| {
        (
          Balances::free_balance(&custody),
          Balances::free_balance(&sink),
          <Balances as InspectHold<crate::AccountId>>::balance_on_hold(&reason, &owner),
          Actors::actor_state_hold(91).unwrap().breakdown,
          Actors::actor_identity_count(),
          Actors::active_actor_count(),
          Actors::owner_slot_bitmap(&owner),
        )
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
    let next_run = externalities_with_bound_code(storage.clone()).execute_with(|| {
      let mut actions = 0; let mut failures = 0; let mut suspended = 0; let mut continued = 0; let mut summaries = 0; let mut closed = 0;
      for record in System::events() {match record.event {
        RuntimeEvent::Actors(Event::ActionFeeCharged {actor_id: 91, cycle_nonce, step_index, fee: actual, actual_effect_weight}) => {
          assert_eq!((cycle_nonce, step_index), (1, 1)); assert_eq!(actual, fee); assert_eq!(actual_effect_weight, W::task_add_liquidity()); actions += 1;
        },
        RuntimeEvent::Actors(Event::StepFailed {actor_id: 91, cycle_nonce, step_index, retry_class, error}) => {
          assert_eq!((cycle_nonce, step_index), (1, 1)); assert_eq!(retry_class, RetryClass::Temporary);
          assert_eq!(error.encode(), polkadot_sdk::sp_runtime::DispatchError::Other("MinimumLpOutputNotMet").encode()); failures += 1;
        },
        RuntimeEvent::Actors(Event::CycleSuspended {actor_id: 91, reason, ..}) => {assert_eq!(reason, SuspensionReason::Temporary); suspended += 1;},
        RuntimeEvent::Actors(Event::CycleContinued {actor_id: 91, cycle_nonce, ..}) => {assert_eq!(cycle_nonce, 1); continued += 1;},
        RuntimeEvent::Actors(Event::CycleSummary {actor_id: 91, cycle_nonce, result, outcomes}) => {
          assert_eq!(cycle_nonce, 1); assert_eq!(result, CycleResult::Failed); assert_eq!(outcomes.failed_steps, 3);
          assert_eq!(outcomes.executed_steps, 1); assert_eq!(outcomes.committed_effectful_tasks, 1); summaries += 1;
        },
        RuntimeEvent::Actors(Event::ActorClosed {actor_id: 91, reason}) => {assert_eq!(reason, CloseReason::RetryAttemptsExhausted); closed += 1;},
        RuntimeEvent::Actors(Event::PipelineFeeCharged {actor_id: 91, ..} | Event::CycleStarted {actor_id: 91, ..}
          | Event::StepSkipped {actor_id: 91, ..} | Event::LiquidityAdded {actor_id: 91, ..} | Event::TransferExecuted {actor_id: 91, ..}) => panic!("no new Opening, false predicate skip, LP output or prefix compensation"),
        _ => {},
      }}
      assert_eq!((actions, failures, continued), (1, 1, u32::from(attempt > 1)));
      assert_eq!((suspended, summaries, closed), if attempt < 3 {(1, 0, 0)} else {(0, 1, 1)});
      assert_eq!(native_before - Balances::free_balance(&custody), fee);
      assert_eq!(Balances::free_balance(&sink) + distributed_native() - sink_before, fee);
      assert_eq!(Assets::balance(0, &custody), paired); assert_eq!(Assets::balance(lp, &custody), lp_balance);
      assert_eq!(crate::AssetConversion::get_reserves(primitives::AssetKind::Native, primitives::AssetKind::Local(0)).unwrap(), reserves);
      assert_eq!(<Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<crate::AccountId>>::total_issuance(lp), supply);
      assert!(Actors::block_resource_state().is_none());
      assert!(!Actors::finalized_block_resource_telemetry().unwrap().optional_actor_work_halted());
      if attempt < 3 {
        let actor = Actors::active_actor_state(91).unwrap(); let run = actor.run_state.unwrap();
        assert_eq!(actor.contract, before.contract); assert_eq!(actor.identity.cycle_nonce, 0);
        assert_eq!((run.cycle_nonce, run.cursor, run.unsuccessful_attempts_at_cursor), (1, 1, attempt));
        assert_eq!(run.contract_authority, prefix_run.contract_authority);
        assert_eq!(run.last_committed_step_block, Some(prefix_block)); assert_eq!(run.eligible_at, parent.number() + 2);
        let handle = Actors::deadline_handles(91).unwrap();
        assert_eq!(handle.key, WakeupKey::Block(run.eligible_at)); assert!(Actors::service_nodes(91).is_none());
        assert_eq!(Actors::deadline_pages(handle.key, handle.page).unwrap().entries[handle.slot as usize], Some(handle.actor));
        assert_eq!(Actors::actor_state_hold(91).unwrap().breakdown, Actors::actor_cost_quote(91).unwrap().actor_state_hold.breakdown);
        Some(run)
      } else {
        assert!(Actors::actor_identity(91).is_none()); assert!(Actors::actor_semantic_states(91).is_none());
        assert!(Actors::actor_run_state(91).is_none()); assert!(Actors::actor_state_hold(91).is_none());
        assert!(ActorContractHeads::<Runtime>::get(91).is_none()); assert_eq!(ActorContractTailChunks::<Runtime>::iter_prefix(91).count(), 0);
        assert!(Actors::deadline_handles(91).is_none()); assert!(Actors::service_nodes(91).is_none()); assert!(Actors::sovereign_index(&custody).is_none());
        assert_eq!(Actors::actor_identity_count(), count - 1); assert_eq!(Actors::active_actor_count(), active - 1);
        let ActorClass::User {owner_slot} = before.identity.actor_class else {unreachable!()};
        let mut bitmap = bitmap; bitmap[usize::from(owner_slot / 8)] &= !(1 << (owner_slot % 8));
        assert_eq!(Actors::owner_slot_bitmap(&owner), bitmap);
        released_hold = hold.identity + hold.contract_head + hold.contract_body + hold.detector + hold.run;
        assert_eq!(held_before - <Balances as InspectHold<crate::AccountId>>::balance_on_hold(&reason, &owner), released_hold);
        let ledger = pallet_deos_actors::ActorStateHolds::<Runtime>::iter().filter(|(_, h)| h.owner == owner)
          .map(|(_, h)| h.breakdown.identity + h.breakdown.contract_head + h.breakdown.contract_body + h.breakdown.detector + h.breakdown.run).sum::<u128>();
        assert_eq!(<Balances as InspectHold<crate::AccountId>>::balance_on_hold(&reason, &owner), ledger);
        None
      }
    });
    let name = match attempt {
      1 => "first-failure",
      2 => "retry-failure",
      _ => "terminal-exhaustion",
    };
    export_execution_stage(&output, name, &stage_parent, &block, &wasm);
    stages.push(serde_json::json!({"stage": name, "block": parent.number(), "postRoot": format!("{:?}", parent.state_root())}));
    if let Some(run) = next_run {
      for waking in [false, true] {
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
          assert_eq!(Actors::actor_run_state(91).unwrap().encode(), run.encode());
          assert_eq!(Balances::free_balance(&custody), native_before - fee);
          assert_eq!(Assets::balance(0, &custody), paired);
          assert_eq!(Assets::balance(lp, &custody), lp_balance);
          if waking {
            assert!(Actors::deadline_handles(91).is_none());
            assert_eq!(
              Actors::service_nodes(91).unwrap().eligible_from,
              parent.number() + 1
            );
          } else {
            assert_eq!(
              Actors::deadline_handles(91).unwrap().key,
              WakeupKey::Block(run.eligible_at)
            );
            assert!(Actors::service_nodes(91).is_none());
          }
          assert!(!System::events().iter().any(|r| matches!(
            r.event,
            RuntimeEvent::Actors(
              Event::ActionFeeCharged { actor_id: 91, .. }
                | Event::PipelineFeeCharged { actor_id: 91, .. }
                | Event::CycleContinued { actor_id: 91, .. }
                | Event::StepFailed { actor_id: 91, .. }
                | Event::CycleSummary { actor_id: 91, .. }
            )
          )));
        });
        if waking {
          export_execution_stage(
            &output,
            if attempt == 1 {
              "retry-wakeup"
            } else {
              "terminal-wakeup"
            },
            &stage_parent,
            &block,
            &wasm,
          );
        }
      }
    }
  }
  std::fs::write(output.join("result.json"), serde_json::to_vec_pretty(&serde_json::json!({
    "actor": 91, "lastBlock": parent.number(), "nativeBlocks": history.len(), "steps": 12, "predicatesPerInvokedStep": 4,
    "pipelineFee": q.prospective_pipeline_fee.unwrap().total_fee.to_string(), "prefixTransfer": 1, "prefixActionFee": prefix_fee.to_string(),
    "lpActionFeePerAttempt": fee.to_string(), "attempts": 3, "newPipelineFeesAfterPrefix": 0, "releasedHold": released_hold.to_string(),
    "stages": stages, "largerTaskExecutionQualified": true, "economicRollback": true, "committedPrefixPreserved": true,
    "creationBackedDestructionQualified": true, "physicalPricingQualified": false, "completeDomainMaximaQualified": false,
  })).unwrap()).unwrap();
}
