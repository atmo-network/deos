//! Public no-policy User workflow diagnostic; declared Weight is not a physical cost bound.

use super::{
  actors_design_comparison::with_isolated_block, actors_integration_tests as fixtures, common,
  h1_geometry, h1_storage_trace,
};
use crate::{Actors, Balances, BlockNumber, Runtime, RuntimeEvent, RuntimeOrigin, System};
use codec::Encode;
use pallet_deos_actors::{
  ActorContract, ActorType, FundingSourcePolicy, Mutability, OnUnfunded, StepControlWeightContext,
  StepControlWeightProvider, WeightInfo,
};
use polkadot_sdk::frame_support::{
  assert_ok,
  traits::{Currency, Get, Hooks},
  weights::Weight,
};
use serde_json::json;

const BLOCK: Weight = Weight::from_parts(2_000_000_000_000, 10_485_760);

#[derive(Debug, Default, PartialEq, Eq)]
struct ReplayWitness {
  roots: Vec<Vec<u8>>,
  weights: Vec<(Weight, Weight)>,
}

fn sha256(bytes: &[u8]) -> String {
  format!(
    "{:x}",
    polkadot_sdk::sp_core::H256::from(polkadot_sdk::sp_io::hashing::sha2_256(bytes))
  )
}

// Installed declared-owner arithmetic is independent of benchmark applicability/physical soundness.
fn singleton_owner_ledger(
  case: &str,
  now: BlockNumber,
  actual: Weight,
  inner: Vec<(&str, Weight)>,
  deadline_return: bool,
  coverage: &str,
) {
  type W = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
  let selector = W::service_round_begin_populated() + W::service_round_probe_eligible();
  let mut owners = vec![
    (
      "scheduler_on_initialize_cutoff",
      W::scheduler_on_initialize_cutoff(),
    ),
    (
      "classify_due_block_deadline",
      W::classify_due_block_deadline(),
    ),
    (
      "classify_due_tick_deadline",
      W::classify_due_tick_deadline(),
    ),
    (
      "dependency_scan_source_probe",
      W::dependency_scan_source_probe(),
    ),
    ("scheduler_on_idle_base", W::scheduler_on_idle_base()),
    ("block_resource_finalize", W::block_resource_finalize()),
    ("prepass_end_selector", selector),
    ("drain_end_selector", selector),
  ];
  if !inner.is_empty() {
    owners.push((
      "eligible_inspection",
      selector + W::scheduler_actor_state_probe(),
    ));
    owners.push((
      "outer_service_suffix",
      W::service_round_admit_eligible()
        .max(W::service_member_retire_interior())
        .max(W::service_member_retire_pair_cursor())
        .max(W::service_member_retire_singleton()),
    ));
    owners.extend(inner);
  }
  if deadline_return {
    owners.push((
      "deadline_return_union",
      W::return_due_block_deadline_to_service()
        .max(W::return_due_block_deadline_to_service_deep_index()),
    ));
  }
  let total = owners
    .iter()
    .try_fold(Weight::zero(), |sum, (_, weight)| sum.checked_add(weight))
    .unwrap();
  assert_eq!(
    actual, total,
    "complete declared singleton caller ledger: {case} block {now}"
  );
  eprintln!(
    "ACTORS_H1 {}",
    json!({
      "schema": 1, "row": "declared_owner_ledger", "case": case, "block": now,
      "coverage": coverage, "control": [total.ref_time(), total.proof_size()],
      "owners": owners.iter().map(|(name, weight)| json!({
        "name": name, "weight": [weight.ref_time(), weight.proof_size()],
      })).collect::<Vec<_>>(),
    })
  );
}

fn primary_inner_owners(length: u32, cursor: u32) -> Vec<(&'static str, Weight)> {
  type W = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
  let (name, weight) = if cursor == 0 {
    if length == 1 {
      (
        "opening_user_complete_header",
        W::scheduler_inner_opening_user_complete_header_max(),
      )
    } else {
      let tails = (length - 1).div_ceil(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK);
      (
        "opening_system_user_progress_union",
        W::scheduler_inner_opening_progress_min(tails)
          .max(W::scheduler_inner_opening_user_progress_min(tails)),
      )
    }
  } else {
    let chunk = pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK;
    let first = 1 + (cursor - 1) / chunk * chunk;
    let fragment = (length - first).min(chunk);
    if cursor + 1 == length {
      (
        "running_complete",
        W::scheduler_inner_running_complete(fragment, 0),
      )
    } else {
      (
        "running_progress",
        W::scheduler_inner_running_progress(fragment, 0),
      )
    }
  };
  vec![
    (name, weight),
    ("action_collection", W::fee_collection()),
    ("action_invocation_receipt", W::action_invocation_receipt()),
  ]
}

fn block(now: BlockNumber, case: &str) -> (Weight, Weight) {
  block_with(now, case, |_| {})
}

fn block_with(
  now: BlockNumber,
  case: &str,
  before_drain: impl FnOnce(&h1_storage_trace::Sample),
) -> (Weight, Weight) {
  with_isolated_block(now, || {
    // Synthetic consensus inputs precede the measured Actor phases; no unrelated hooks run.
    System::set_block_number(now);
    System::reset_events();
    System::set_block_consumed_resources(Weight::zero(), 0);
    common::set_consensus_timestamp(
      u64::from(now) * primitives::ecosystem::params::ACTOR_CADENCE_TICK_MILLIS,
    );
    polkadot_sdk::cumulus_pallet_parachain_system::ValidationData::<Runtime>::put(
      polkadot_sdk::cumulus_primitives_core::PersistedValidationData {
        max_pov_size: BLOCK.proof_size() as u32,
        ..Default::default()
      },
    );
    h1_geometry::check(now);
    let sample = h1_storage_trace::Sample::new();
    assert_eq!(
      sample.phase("initialize", || Actors::on_initialize(now)),
      Weight::zero()
    );
    let post = sample.phase("prepass", || {
      Actors::actor_prepass(RuntimeOrigin::none()).unwrap()
    });
    let prepass = Actors::block_resource_state().unwrap();
    let budget = prepass.budget().unwrap();
    assert_eq!(budget.maximum_block(), BLOCK);
    assert_eq!(budget.fixed_envelope(), Weight::zero());
    assert_eq!(
      budget.limits().actor_control(),
      Weight::from_parts(666_666_666_666, 3_495_253)
    );
    assert_eq!(
      budget.limits().shared_economic(),
      Weight::from_parts(1_333_333_333_334, 6_990_507)
    );
    assert_eq!(
      budget.limits().actor_base_turn(),
      Weight::from_parts(666_666_666_667, 3_495_253)
    );
    assert_eq!(
      prepass.phase(),
      pallet_deos_actors::BlockResourcePhase::ExternalPhase
    );
    assert_eq!(prepass.outstanding_reservations(), 0);
    assert!(
      prepass
        .usage()
        .actor_effect_used()
        .all_lte(budget.limits().actor_base_turn())
    );
    let used = prepass
      .usage()
      .actor_control_used()
      .checked_add(&prepass.usage().actor_effect_used())
      .unwrap();
    assert_eq!(post.actual_weight, Some(used));
    // Counterfactual controls may settle explicit prior usage, never alter the frozen limits.
    before_drain(&sample);
    let seeded = Actors::block_resource_state().unwrap().usage();
    let seeded_control = seeded
      .actor_control_used()
      .checked_sub(&prepass.usage().actor_control_used())
      .unwrap();
    let consumed = seeded
      .actor_control_used()
      .checked_add(&seeded.actor_effect_used())
      .unwrap()
      .checked_add(&seeded.user_dispatch_used())
      .unwrap();
    sample.phase("drain", || {
      Actors::on_idle(now, BLOCK.checked_sub(&consumed).unwrap())
    });
    let final_state = Actors::block_resource_state().unwrap();
    assert_eq!(final_state.budget(), Ok(budget));
    assert_eq!(
      final_state.phase(),
      pallet_deos_actors::BlockResourcePhase::Finalizable
    );
    assert_eq!(final_state.outstanding_reservations(), 0);
    assert!(!final_state.optional_actor_work_halted());
    let usage = final_state.usage();
    assert_eq!(usage.user_dispatch_used(), seeded.user_dispatch_used());
    assert!(
      usage
        .actor_control_used()
        .all_lte(budget.limits().actor_control())
    );
    assert!(
      usage
        .actor_effect_used()
        .all_lte(budget.limits().shared_economic())
    );
    // Report performed Actor hook charges, not the counterfactual Control consumption.
    let result = (
      usage
        .actor_control_used()
        .checked_sub(&seeded_control)
        .unwrap(),
      usage.actor_effect_used(),
    );
    assert_eq!(
      Actors::finalized_block_resource_telemetry(),
      Some(final_state.finalized_snapshot().unwrap())
    );
    sample.phase("finalize", || Actors::on_finalize(now));
    assert!(Actors::block_resource_state().is_none());
    if let Some(storage) = sample.finish() {
      eprintln!(
        "ACTORS_H1 {}",
        json!({
          "schema": 1, "row": "storage", "case": case, "block": now, "storage": storage,
        })
      );
    }
    let geometry = h1_geometry::check(now);
    eprintln!(
      "ACTORS_H1 {}",
      json!({
        "schema": 1, "row": "geometry", "case": case, "block": now,
        "checkpoint": "after-finalize", "geometry": geometry,
      })
    );
    result
  })
}

#[test]
fn h1_public_user_lengths_report_declared_cost_and_q1() {
  h1_storage_trace::h1_storage_trace_scopes_validation_and_preserves_provisional_work();
  let plain = user_workflows();
  let traced = h1_storage_trace::recorded(user_workflows);
  assert_eq!(plain.roots.len(), 16);
  assert_eq!(plain.weights.len(), 77);
  assert_eq!(
    plain, traced,
    "observer must preserve roots and every block Weight vector"
  );
  eprintln!(
    "ACTORS_H1 {}",
    json!({
      "schema": 1, "row": "observer_equivalence", "cases": plain.roots.len(),
    "phase_blocks": plain.weights.len(), "equal_final_roots": true, "equal_phase_weights": true,
    "final_roots": plain.roots.iter().map(|root|
      format!("{:x}", polkadot_sdk::sp_core::H256::from_slice(root))).collect::<Vec<_>>(),
    })
  );
}

fn user_workflows() -> ReplayWitness {
  let mut witness = ReplayWitness::default();
  let executable = std::env::current_exe().unwrap();
  eprintln!(
    "ACTORS_H1 {}",
    json!({
      "schema": 1, "row": "manifest", "workload": "H1-public-transfer-r1", "evidence": "native-diagnostic",
      "executor": "compiled-runtime-library-test", "binary_sha256": sha256(&std::fs::read(executable).unwrap()),
      "instrumented": h1_storage_trace::enabled(),
      "logical_storage": if h1_storage_trace::enabled() { "selected-sdk-state-events" } else { "not-collected" },
      "lengths": [1, 2, 4, 8, 12], "companions": ["recipient-retry", "recipient-retry-terminal"],
      "primary_population": 1, "seed": null,
      "controls": ["isolated-empty", "user-drain-resource-refusal"],
      "control_population": "reported-per-control",
      "unsupported": ["full-storage-api-coverage", "physical-proof", "pov", "cpu", "capacity-qualification"],
    })
  );
  for length in [1u32, 2, 4, 8, 12] {
    witness.roots.push(common::synthetic_actor_test_ext().execute_with(|| {
      assert_eq!(Actors::actor_identity_count(), 0);
      System::set_block_number(1);
      assert_ok!(Actors::set_user_creation_enabled(RuntimeOrigin::root(), true));
      let amount = crate::EXISTENTIAL_DEPOSIT;
      let template = fixtures::transfer_contract_steps(common::BOB, primitives::AssetKind::Native, amount);
      let contract = ActorContract {
        trigger: pallet_deos_actors::Trigger::manual(), cooldown_blocks: 0, window: None,
        steps: vec![template[0].clone(); length as usize].try_into().unwrap(),
        on_unfunded: OnUnfunded::Close, close_when: None, parked_balance_activation: None,
        funding: FundingSourcePolicy::OwnerOnly, auto_close_at_cycle_nonce: None,
      };
      let contract_hash = sha256(&contract.encode());
      let sovereign = Actors::sovereign_account_id(&common::ALICE, 0);
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sovereign, 1_000 * crate::UNIT);
      let sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
      let sink_before = Balances::free_balance(&sink);
      let actor = Actors::next_actor_id();
      assert_ok!(Actors::create_user_actor(RuntimeOrigin::signed(common::ALICE), Mutability::Mutable, Some(contract)));
      assert_eq!(Actors::actor_identity_count(), 1);
      assert_eq!(Actors::active_actor_count(), 1);
      let identity = Actors::actor_identity(actor).unwrap();
      assert_eq!(identity.actor_class.actor_type(), ActorType::User);
      assert_eq!(identity.sovereign_account, sovereign);
      let creation = Balances::free_balance(&sink) - sink_before;
      let quote = Actors::actor_cost_quote(actor).unwrap();
      assert_eq!(creation, quote.creation_fee);
      let hold = Actors::actor_state_hold(actor);
      assert!(hold.is_some());
      // Use a later public call, not test-only aging of the creation control clock.
      System::set_block_number(2);
      let source_before = Balances::free_balance(&sovereign);
      let sink_before = Balances::free_balance(&sink);
      assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(common::ALICE), actor));
      let trigger = Balances::free_balance(&sink) - sink_before;
      let trigger_quote = quote.prospective_trigger_fee.unwrap();
      assert_eq!(trigger, trigger_quote.fee);
      assert_eq!(source_before - Balances::free_balance(&sovereign), trigger);
      assert_eq!(Actors::service_nodes(actor).unwrap().eligible_from, 3);
      eprintln!("ACTORS_H1 {}", json!({
        "schema": 1, "row": "setup", "length": length, "contract_scale_sha256": contract_hash,
        "creation_fee": creation.to_string(), "trigger_fee": trigger.to_string(),
        "active_limit": Actors::configured_active_actor_limit(), "trigger_block": 2,
        "trigger_control": [trigger_quote.maximum_weight.ref_time(), trigger_quote.maximum_weight.proof_size()],
      }));
      let mut total_control = Weight::zero();
      let mut total_effect = Weight::zero();
      let mut total_pipeline = 0u128;
      let mut total_action = 0u128;
      for now in 2..=length + 2 {
        let source_before = Balances::free_balance(&sovereign);
        let sink_before = Balances::free_balance(&sink);
        let recipient_before = Balances::free_balance(common::BOB);
        let node_before = Actors::service_nodes(actor);
        let case = format!("length-{length}");
        let (control, effect) = block(now, &case);
        singleton_owner_ledger(&case, now, control,
          if now == 2 { vec![] } else { primary_inner_owners(length, now - 3) },
          false, "declared-arithmetic-only");
        witness.weights.push((control, effect));
        let mut steps = Vec::new();
        let mut pipeline = 0u128;
        let mut action = 0u128;
        for record in System::events() {
          match record.event {
            RuntimeEvent::Actors(pallet_deos_actors::Event::TransferExecuted { actor_id, step_index, amount: actual, .. }) => {
              assert_eq!(actor_id, actor);
              assert_eq!(actual, amount);
              steps.push(step_index);
            }
            RuntimeEvent::Actors(pallet_deos_actors::Event::PipelineFeeCharged { actor_id, fee }) => {
              assert_eq!(actor_id, actor);
              pipeline = pipeline.checked_add(fee).unwrap();
            }
            RuntimeEvent::Actors(pallet_deos_actors::Event::ActionFeeCharged { actor_id, step_index, actual_effect_weight, fee, .. }) => {
              assert_eq!(actor_id, actor);
              assert_eq!(Some(&step_index), steps.last());
              assert_eq!(actual_effect_weight, effect);
              action = action.checked_add(fee).unwrap();
            }
            RuntimeEvent::Actors(pallet_deos_actors::Event::StepFailed { .. } | pallet_deos_actors::Event::StepSkipped { .. }) => panic!("H1 requires successful economic Steps"),
            _ => (),
          }
        }
        let expected: Vec<u32> = if now == 2 { vec![] } else { vec![now - 3] };
        assert_eq!(steps, expected, "B+1 and Q1 under singleton conforming capacity");
        assert_eq!(pipeline, if now == 3 { quote.prospective_pipeline_fee.as_ref().unwrap().total_fee } else { 0 });
        assert_eq!(action, if steps.is_empty() { 0 } else { quote.maximum_next_action_fee.maximum_effect_fee });
        assert_eq!(Balances::free_balance(common::BOB) - recipient_before, amount * steps.len() as u128);
        assert_eq!(Balances::free_balance(&sink) - sink_before, pipeline + action);
        assert_eq!(source_before - Balances::free_balance(&sovereign), amount * steps.len() as u128 + pipeline + action);
        let node_after = Actors::service_nodes(actor);
        if now < length + 2 {
          let before = node_before.unwrap();
          let after = node_after.unwrap();
          assert_eq!((before.generation, before.previous, before.next), (after.generation, after.previous, after.next));
          if now > 2 { assert_eq!(Actors::actor_run_state(actor).unwrap().cursor, now - 2); }
        } else {
          assert!(node_after.is_none());
          assert!(Actors::actor_run_state(actor).is_none());
          assert_eq!(Actors::actor_identity(actor).unwrap().cycle_nonce, 1);
          let hot = Actors::actor_hot(actor).unwrap();
          assert_eq!(hot.cycle_state, pallet_deos_actors::CycleState::Idle);
          assert!(!hot.pending_signal);
        }
        assert_eq!(Actors::actor_state_hold(actor), hold);
        eprintln!("ACTORS_H1 {}", json!({
          "schema": 1, "row": "block", "length": length, "block": now, "steps": steps,
          "control": [control.ref_time(), control.proof_size()], "effect": [effect.ref_time(), effect.proof_size()],
          "pipeline_fee": pipeline.to_string(), "action_fee": action.to_string(),
          "resident_before": node_before.is_some(), "resident_after": node_after.is_some(),
        }));
        total_control = total_control.checked_add(&control).unwrap();
        total_effect = total_effect.checked_add(&effect).unwrap();
        total_pipeline = total_pipeline.checked_add(pipeline).unwrap();
        total_action = total_action.checked_add(action).unwrap();
      }
      eprintln!("ACTORS_H1 {}", json!({
        "schema": 1, "row": "workflow", "length": length, "committed_steps": length,
        "post_trigger_control": [total_control.ref_time(), total_control.proof_size()],
        "trigger_control": [trigger_quote.maximum_weight.ref_time(), trigger_quote.maximum_weight.proof_size()],
        "effect": [total_effect.ref_time(), total_effect.proof_size()],
        "pipeline_fee": total_pipeline.to_string(), "action_fee": total_action.to_string(),
        "first_step_latency_blocks": 1, "completion_latency_blocks": length, "disposition": "passive-idle",
      }));
      polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version())
    }));
  }
  for terminal in [false, true] {
    let mut retry = retry_after_recipient_recovery(terminal);
    witness.roots.append(&mut retry.roots);
    witness.weights.append(&mut retry.weights);
  }
  for mut control in
    std::iter::once(empty_block_control()).chain([false, true].into_iter().flat_map(|control| {
      [false, true].into_iter().flat_map(move |proof| {
        [false, true]
          .into_iter()
          .map(move |running| resource_refusal(control, proof, running))
      })
    }))
  {
    witness.roots.append(&mut control.roots);
    witness.weights.append(&mut control.weights);
  }
  witness
}

fn retry_after_recipient_recovery(terminal: bool) -> ReplayWitness {
  use pallet_deos_actors::{
    AmountResolution, CloseReason, CycleState, Event, RetryClass, StepErrorPolicy, StepOutcome,
    SuspensionReason, Task, TaskFailure, WakeupKey,
  };
  let mut weights = Vec::new();
  let root = common::synthetic_actor_test_ext().execute_with(|| {
    const ASSET: u32 = 70_999;
    let scenario = if terminal { "retry-terminal" } else { "retry" };
    let recipient = crate::AccountId::new([173u8; 32]);
    System::set_block_number(1);
    assert_ok!(Actors::set_user_creation_enabled(RuntimeOrigin::root(), true));
    assert_ok!(crate::Assets::force_create(RuntimeOrigin::root(), ASSET, common::ALICE.into(), true, 2));
    let sovereign = Actors::sovereign_account_id(&common::ALICE, 0);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sovereign, 1_000 * crate::UNIT);
    assert_ok!(crate::Assets::mint(RuntimeOrigin::signed(common::ALICE), ASSET, sovereign.clone().into(), 3));
    let amount = crate::EXISTENTIAL_DEPOSIT;
    let template = fixtures::transfer_contract_steps(common::BOB, primitives::AssetKind::Native, amount);
    let mut middle = template[0].clone();
    middle.task = Task::Transfer {
      to: recipient.clone(), asset: primitives::AssetKind::Local(ASSET), amount: AmountResolution::Fixed(1),
    };
    middle.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let contract = ActorContract {
      trigger: pallet_deos_actors::Trigger::manual(), cooldown_blocks: 0, window: None,
      steps: vec![template[0].clone(), middle, template[0].clone()].try_into().unwrap(),
      on_unfunded: OnUnfunded::Close, close_when: None, parked_balance_activation: None,
      funding: FundingSourcePolicy::OwnerOnly, auto_close_at_cycle_nonce: terminal.then_some(1),
    };
    let contract_hash = sha256(&contract.encode());
    let sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
    let sink_before = Balances::free_balance(&sink);
    let actor = Actors::next_actor_id();
    let owner_before = Balances::free_balance(common::ALICE);
    assert_ok!(Actors::create_user_actor(RuntimeOrigin::signed(common::ALICE), Mutability::Mutable, Some(contract)));
    let creation = Balances::free_balance(&sink) - sink_before;
    let quote = Actors::actor_cost_quote(actor).unwrap();
    assert_eq!(creation, quote.creation_fee);
    let hold = Actors::actor_state_hold(actor);
    assert!(hold.is_some());
    let owner_after_creation = Balances::free_balance(common::ALICE);
    let held = quote.actor_state_hold.total;
    assert_eq!(owner_before - owner_after_creation, creation + held);
    System::set_block_number(2);
    let sink_before = Balances::free_balance(&sink);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(common::ALICE), actor));
    let trigger = Balances::free_balance(&sink) - sink_before;
    let trigger_quote = quote.prospective_trigger_fee.unwrap();
    assert_eq!(trigger, trigger_quote.fee);
    let bob_before = Balances::free_balance(common::BOB);
    let mut total_control = Weight::zero();
    let mut total_effect = Weight::zero();
    let mut total_pipeline = 0u128;
    let mut total_action = 0u128;
    let mut all_transfers = Vec::new();
    let mut all_attempts = Vec::new();
    eprintln!("ACTORS_H1 {}", json!({
      "schema": 1, "row": "setup", "scenario": scenario, "workload": "H1-public-recipient-retry-r1",
      "contract_scale_sha256": contract_hash, "length": 3, "creation_fee": creation.to_string(),
      "trigger_fee": trigger.to_string(), "trigger_block": 2,
      "trigger_control": [trigger_quote.maximum_weight.ref_time(), trigger_quote.maximum_weight.proof_size()],
      "active_limit": Actors::configured_active_actor_limit(),
    }));
    for now in 2..=9 {
      if now == 6 {
        // A real public host-ledger operation repairs the recipient, not fabricated retry state.
        // Its execution is outside the isolated Actor phases and is not charged as Actor work.
        System::set_block_number(now);
        assert_ok!(crate::Assets::mint(RuntimeOrigin::signed(common::ALICE), ASSET, recipient.clone().into(), 2));
        eprintln!("ACTORS_H1 {}", json!({
          "schema": 1, "row": "recovery", "scenario": scenario, "block": now,
          "operation": "public-assets-mint", "amount": "2", "external_dispatch_measured": false,
        }));
      }
      let source_before = Balances::free_balance(&sovereign);
      let sink_before = Balances::free_balance(&sink);
      let (control, effect) = block(now, scenario);
      type W = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
      let inner = match now {
        3 => primary_inner_owners(3, 0),
        4 => vec![
          ("running_tail_plan", W::current_step_plan_running_tail(2)),
          ("run_suspend", W::run_suspend()),
          ("queued_retry_placement", W::service_round_admit_eligible()),
          ("action_collection", W::fee_collection()),
          ("action_invocation_receipt", W::action_invocation_receipt()),
        ],
        5 | 8 => vec![
          (if now == 5 { "suspended_tail_retry_class_union" } else { "suspended_tail_progress_class_union" },
            if now == 5 { W::scheduler_inner_suspended_tail_retry(2, 0).max(W::scheduler_inner_suspended_user_tail_retry(2, 0)) }
            else { W::scheduler_inner_suspended_tail_progress(2, 0).max(W::scheduler_inner_suspended_user_tail_progress(2, 0)) }),
          ("action_collection", W::fee_collection()),
          ("action_invocation_receipt", W::action_invocation_receipt()),
        ],
        9 if terminal => {
          type Control = <Runtime as pallet_deos_actors::Config>::StepControlWeight;
          let context = StepControlWeightContext {
            cursor: 2, steps_in_fragment: 2, opening_tail_chunks: 0, predicate_evaluation_units: 0,
          };
          let step = fixtures::transfer_contract_steps(common::BOB, primitives::AssetKind::Native, amount)[0].clone();
          vec![
            ("retained_admitted_step_maximum", Control::maximum_control_weight(context, &step).unwrap()),
            ("retained_terminal_cleanup", Actors::close_dispatch_weight_upper()),
          ]
        },
        9 => primary_inner_owners(3, 2),
        _ => vec![],
      };
      singleton_owner_ledger(scenario, now, control, inner, now == 7,
        if now == 5 || now == 8 { "declared-user-system-union" }
        else { "declared-arithmetic-only" });
      weights.push((control, effect));
      let mut transfers = Vec::new();
      let mut attempts = Vec::new();
      let mut failures = Vec::new();
      let mut pipeline = Vec::new();
      let mut action = 0u128;
      let mut closed = Vec::new();
      for record in System::events() {
        match record.event {
          RuntimeEvent::Actors(Event::TransferExecuted { actor_id, cycle_nonce, step_index, .. }) => {
            assert_eq!((actor_id, cycle_nonce), (actor, 1));
            transfers.push(step_index);
          }
          RuntimeEvent::Actors(Event::StepFailed { actor_id, step_index, retry_class, error, .. }) => {
            assert_eq!(actor_id, actor);
            assert_eq!(retry_class, RetryClass::Temporary);
            assert_eq!(error, pallet_deos_actors::Error::<Runtime>::RecipientDepositUnavailable.into());
            failures.push(step_index);
          }
          RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id, fee }) => {
            assert_eq!(actor_id, actor);
            pipeline.push(fee);
          }
          RuntimeEvent::Actors(Event::ActionFeeCharged { actor_id, cycle_nonce, step_index, actual_effect_weight, fee }) => {
            assert_eq!((actor_id, cycle_nonce), (actor, 1));
            assert_eq!(actual_effect_weight, effect);
            assert_eq!(fee, quote.maximum_next_action_fee.maximum_effect_fee);
            attempts.push(step_index);
            action = action.checked_add(fee).unwrap();
          }
          RuntimeEvent::Actors(Event::ActorClosed { actor_id, reason }) => {
            assert_eq!(actor_id, actor);
            closed.push(reason);
          }
          RuntimeEvent::Actors(Event::StepSkipped { .. }) => panic!("retry witness must not skip its economic Steps"),
          _ => (),
        }
      }
      assert_eq!(transfers, match now { 3 => vec![0], 8 => vec![1], 9 => vec![2], _ => vec![] });
      assert_eq!(attempts, match now { 3 => vec![0], 4 | 5 | 8 => vec![1], 9 => vec![2], _ => vec![] });
      assert_eq!(failures, if now == 4 || now == 5 { vec![1] } else { vec![] });
      assert_eq!(effect == Weight::zero(), attempts.is_empty());
      assert_eq!(pipeline, if now == 3 { vec![quote.prospective_pipeline_fee.as_ref().unwrap().total_fee] } else { vec![] });
      let pipeline: u128 = pipeline.into_iter().sum();
      assert_eq!(Balances::free_balance(&sink) - sink_before, pipeline + action);
      let native_value = if now == 3 || now == 9 { amount } else { 0 };
      assert_eq!(source_before - Balances::free_balance(&sovereign), native_value + pipeline + action);
      assert_eq!(Balances::free_balance(common::BOB) - bob_before, if now == 9 { 2 * amount } else if now >= 3 { amount } else { 0 });
      assert_eq!(crate::Assets::balance(ASSET, &sovereign), if now >= 8 { 2 } else { 3 });
      assert_eq!(crate::Assets::balance(ASSET, &recipient), if now >= 8 { 3 } else if now >= 6 { 2 } else { 0 });
      if (4..=7).contains(&now) {
        let run = Actors::actor_run_state(actor).unwrap();
        assert_eq!(run.cursor, 1);
        assert_eq!(run.unsuccessful_attempts_at_cursor, if now == 4 { 1 } else { 2 });
        assert_eq!(run.last_committed_step_block, Some(3));
        assert_eq!(run.last_step_outcome, Some(StepOutcome::Failed(TaskFailure::temporary(pallet_deos_actors::Error::<Runtime>::RecipientDepositUnavailable))));
        assert_eq!(run.suspension, Some(SuspensionReason::Temporary));
        assert_eq!(Actors::actor_hot(actor).unwrap().cycle_state, CycleState::Suspended);
      }
      if now == 5 || now == 6 {
        assert!(Actors::service_nodes(actor).is_none());
        assert_eq!(Actors::deadline_handles(actor).unwrap().key, WakeupKey::Block(7));
      }
      if now == 7 {
        assert!(Actors::deadline_handles(actor).is_none());
        assert_eq!(Actors::service_nodes(actor).unwrap().eligible_from, 8);
        assert_eq!(effect, Weight::zero());
      }
      if now == 8 {
        let run = Actors::actor_run_state(actor).unwrap();
        assert_eq!((run.cursor, run.unsuccessful_attempts_at_cursor, run.last_committed_step_block), (2, 0, Some(8)));
      }
      assert_eq!(closed, if terminal && now == 9 { vec![CloseReason::AutoCloseNonceReached] } else { vec![] });
      if terminal && now == 9 {
        assert!(Actors::actor_identity(actor).is_none());
        assert!(Actors::actor_contract(actor).is_none());
        assert!(Actors::actor_processes(actor).is_none());
        assert!(Actors::actor_hot(actor).is_none());
        assert!(Actors::actor_state_hold(actor).is_none());
        assert_eq!(Balances::free_balance(common::ALICE), owner_after_creation + held);
      } else {
        assert_eq!(Actors::actor_state_hold(actor), hold);
        assert_eq!(Balances::free_balance(common::ALICE), owner_after_creation);
      }
      eprintln!("ACTORS_H1 {}", json!({
        "schema": 1, "row": "block", "scenario": scenario, "block": now,
        "steps": transfers, "attempts": attempts, "temporary_failures": failures,
        "control": [control.ref_time(), control.proof_size()], "effect": [effect.ref_time(), effect.proof_size()],
        "pipeline_fee": pipeline.to_string(), "action_fee": action.to_string(),
        "service": Actors::service_nodes(actor).is_some(), "closed": terminal && now == 9,
      }));
      all_transfers.extend(transfers);
      all_attempts.extend(attempts);
      total_control = total_control.checked_add(&control).unwrap();
      total_effect = total_effect.checked_add(&effect).unwrap();
      total_pipeline = total_pipeline.checked_add(pipeline).unwrap();
      total_action = total_action.checked_add(action).unwrap();
    }
    assert_eq!(all_transfers, vec![0, 1, 2]);
    assert_eq!(all_attempts, vec![0, 1, 1, 1, 2]);
    assert!(Actors::actor_run_state(actor).is_none());
    assert!(Actors::service_nodes(actor).is_none());
    eprintln!("ACTORS_H1 {}", json!({
      "schema": 1, "row": "workflow", "scenario": scenario, "length": 3,
      "successful_steps": 3, "typed_attempt_outcomes": 5, "action_attempts": 5, "temporary_failures": 2,
      "first_step_latency_blocks": 1, "completion_latency_blocks": 7,
      "post_trigger_control": [total_control.ref_time(), total_control.proof_size()],
      "trigger_control": [trigger_quote.maximum_weight.ref_time(), trigger_quote.maximum_weight.proof_size()],
      "effect": [total_effect.ref_time(), total_effect.proof_size()],
      "pipeline_fee": total_pipeline.to_string(), "action_fee": total_action.to_string(),
      "disposition": if terminal { "closed-auto-nonce" } else { "passive-idle" },
    }));
    polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version())
  });
  ReplayWitness {
    roots: vec![root],
    weights,
  }
}

fn control_block(witness: &mut ReplayWitness, case: &str, now: u32, cost: (Weight, Weight)) {
  witness.weights.push(cost);
  eprintln!(
    "ACTORS_H1 {}",
    json!({
      "schema": 1, "row": "control_block", "case": case, "block": now,
      "performed_hook_control": [cost.0.ref_time(), cost.0.proof_size()],
      "performed_actor_effect": [cost.1.ref_time(), cost.1.proof_size()],
    })
  );
}

fn empty_block_control() -> ReplayWitness {
  let mut witness = ReplayWitness::default();
  let root = common::synthetic_actor_test_ext().execute_with(|| {
    assert_eq!(Actors::actor_identity_count(), 0);
    let sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
    let before = (
      Balances::free_balance(&sink),
      Balances::free_balance(common::ALICE),
    );
    let cost = block(2, "isolated-empty");
    assert_eq!(cost.1, Weight::zero());
    assert!(cost.0.ref_time() > 0 && cost.0.proof_size() > 0);
    assert_eq!(Actors::service_header().count, 0);
    assert_eq!(Actors::actor_identity_count(), 0);
    assert_eq!(
      before,
      (
        Balances::free_balance(&sink),
        Balances::free_balance(common::ALICE)
      )
    );
    assert!(System::events().is_empty());
    control_block(&mut witness, "isolated-empty", 2, cost);
    polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version())
  });
  witness.roots.push(root);
  witness
}

fn resource_refusal(
  control_shortfall: bool,
  proof_shortfall: bool,
  running: bool,
) -> ReplayWitness {
  use pallet_deos_actors::{BlockResourceDomain as Domain, WeightInfo};
  type Weights = crate::weights::pallet_deos_actors::SubstrateWeight<Runtime>;
  let case = format!(
    "refusal-{}-{}-{}",
    if control_shortfall {
      "control"
    } else {
      "effect"
    },
    if proof_shortfall { "proof" } else { "time" },
    if running { "running" } else { "idle" },
  );
  let mut witness = ReplayWitness::default();
  let root = common::seeded_synthetic_actor_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(Actors::set_user_creation_enabled(RuntimeOrigin::root(), true));
    let amount = crate::EXISTENTIAL_DEPOSIT;
    let steps = fixtures::transfer_contract_steps(common::BOB, primitives::AssetKind::Native, amount);
    let contract = ActorContract {
      trigger: pallet_deos_actors::Trigger::manual(), cooldown_blocks: 0, window: None,
      steps: vec![steps[0].clone(); 3].try_into().unwrap(),
      on_unfunded: OnUnfunded::Close, close_when: None, parked_balance_activation: None,
      funding: FundingSourcePolicy::OwnerOnly, auto_close_at_cycle_nonce: None,
    };
    let ids = (0..2).map(|slot| {
      let sovereign = Actors::sovereign_account_id(&common::ALICE, slot);
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sovereign, 1_000 * crate::UNIT);
      let id = Actors::next_actor_id();
      assert_ok!(Actors::create_user_actor(RuntimeOrigin::signed(common::ALICE), Mutability::Mutable, Some(contract.clone())));
      assert_eq!(Actors::actor_identity(id).unwrap().sovereign_account, sovereign);
      id
    }).collect::<Vec<_>>();
    let quotes = ids.iter().map(|id| Actors::actor_cost_quote(*id).unwrap()).collect::<Vec<_>>();
    let holds = ids.iter().map(|id| Actors::actor_state_hold(*id)).collect::<Vec<_>>();
    let budget = with_isolated_block(3, || super::actors_design_comparison::prepass_budget(
      3, crate::configs::BlockResourceBudgetValue::get(),
    ).unwrap());
    let (prefix, prefix_effect) = fixtures::prepare_paired_drain_prefix(budget, running, |steps| {
      let id = Actors::next_actor_id();
      assert_ok!(Actors::create_system_actor(RuntimeOrigin::root(), common::ALICE, Mutability::Mutable,
        Some(ActorContract { steps, ..contract.clone() })));
      id
    });
    assert_eq!(Actors::active_actor_count() as usize, prefix.len() + ids.len());
    // Activation order, not Actor class, puts the real LP prefix before both public User heads.
    System::set_block_number(2);
    for id in &prefix { assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), *id)); }
    for (id, quote) in ids.iter().zip(&quotes) {
      let account = Actors::actor_identity(*id).unwrap().sovereign_account;
      let before = Balances::free_balance(&account);
      assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(common::ALICE), *id));
      assert_eq!(before - Balances::free_balance(&account), quote.prospective_trigger_fee.as_ref().unwrap().fee);
      assert_eq!(Actors::service_nodes(*id).unwrap().eligible_from, 3);
    }
    eprintln!("ACTORS_H1 {}", json!({
      "schema": 1, "row": "control_setup", "case": case,
      "workload": "H1-public-user-resource-refusal-r1", "prefix": "real-add-liquidity",
      "prefix_actors": prefix.len(), "user_actors": ids.len(), "user_steps": 3,
      "contract_scale_sha256": sha256(&contract.encode()), "clock_aging": false,
      "synthetic_prior_usage": true, "signed_block_or_capacity_acceptance": false,
    }));
    control_block(&mut witness, &case, 2, block(2, &case));
    for id in &ids { assert!(Actors::actor_run_state(*id).is_none()); }
    if running {
      control_block(&mut witness, &case, 3, block(3, &case));
      for id in &ids { assert_eq!(Actors::actor_run_state(*id).unwrap().cursor, 1); }
    }
    let now = if running { 4 } else { 3 };
    let sink = <Runtime as pallet_deos_actors::Config>::FeeSink::get();
    let snapshot = || (
      Actors::service_header(),
      ids.iter().map(|id| {
        let account = Actors::actor_identity(*id).unwrap().sovereign_account;
        (pallet_deos_actors::ActorSemanticStates::<Runtime>::get(id), pallet_deos_actors::ActorProcesses::<Runtime>::get(id),
          Actors::actor_run_state(*id), Actors::service_nodes(*id), Actors::actor_state_hold(*id),
          pallet_deos_actors::ActorContractHeads::<Runtime>::get(id),
          pallet_deos_actors::ActorContractTailChunks::<Runtime>::get(id, 0),
          Balances::free_balance(&account), Balances::reserved_balance(&account))
      }).collect::<Vec<_>>(),
      Balances::free_balance(common::ALICE), Balances::reserved_balance(common::ALICE),
      Balances::free_balance(common::BOB), Balances::free_balance(&sink), Balances::total_issuance(), System::events(),
    ).encode();
    let mut before = Vec::new();
    let mut input = None;
    let housekeeping = Weights::scheduler_on_idle_base().saturating_add(Weights::block_resource_finalize());
    let inspection = Weights::service_round_begin_populated()
      .saturating_add(Weights::service_round_probe_eligible())
      .saturating_add(Weights::scheduler_actor_state_probe());
    let cost = block_with(now, &case, |sample| {
      fixtures::close_paired_drain_head_with(&prefix, prefix_effect, |id| {
        assert_ok!(sample.phase("prefix-head-close", || Actors::close_actor(RuntimeOrigin::root(), id)));
      });
      assert_eq!(Actors::service_header().cursor.unwrap().actor_id, ids[0]);
      for id in &ids {
        assert_eq!(Actors::actor_run_state(*id).map(|run| run.cursor), running.then_some(1));
      }
      let mut state = Actors::block_resource_state().unwrap();
      assert_eq!(state.budget(), Ok(budget));
      let limits = budget.limits();
      let resources = if running {
        pallet_deos_actors::ActorContractTailChunks::<Runtime>::get(ids[0], 0).unwrap().step_resources[0]
      } else {
        pallet_deos_actors::ActorContractHeads::<Runtime>::get(ids[0]).unwrap().first_step_resources.unwrap()
      };
      let suffix = Weights::service_round_admit_eligible()
        .max(Weights::service_member_retire_interior())
        .max(Weights::service_member_retire_pair_cursor())
        .max(Weights::service_member_retire_singleton());
      let (domain, remaining, required) = if control_shortfall {
        (Domain::ActorControl,
          limits.actor_control().checked_sub(&state.usage().actor_control_used()).unwrap(),
          housekeeping.saturating_add(inspection).saturating_add(resources.control).saturating_add(suffix))
      } else {
        (Domain::UserDispatch,
          limits.shared_economic().checked_sub(&state.usage().actor_effect_used()
            .saturating_add(state.usage().user_dispatch_used())).unwrap(),
          resources.effect)
      };
      assert!(required.all_lte(remaining));
      let prior = if proof_shortfall {
        Weight::from_parts(0, remaining.proof_size() - required.proof_size() + 1)
      } else {
        Weight::from_parts(remaining.ref_time() - required.ref_time() + 1, 0)
      };
      let control_before = state.usage().actor_control_used();
      let setup_dispatch = state.usage().user_dispatch_used();
      let mut reservation = state.reserve(limits, domain, prior).unwrap();
      assert_ok!(state.settle(&mut reservation, prior));
      pallet_deos_actors::CurrentBlockResourceState::<Runtime>::put(state);
      before = snapshot();
      input = Some((prior, required, remaining, control_before, setup_dispatch));
    });
    assert_eq!(snapshot(), before, "paid capacity refusal preserves both User heads, fees and custody");
    let (prior, required, remaining, control_before, setup_dispatch) = input.unwrap();
    assert_eq!(cost.0, control_before.saturating_add(housekeeping).saturating_add(inspection));
    assert_eq!(cost.1, prefix_effect.saturating_mul((prefix.len() - 1) as u64));
    control_block(&mut witness, &case, now, cost);
    eprintln!("ACTORS_H1 {}", json!({
      "schema": 1, "row": "resource_refusal", "case": case, "block": now,
      "seeded_domain": if control_shortfall { "actor-control" } else { "user-dispatch-shared-economic" },
      "limiting_component": if proof_shortfall { "proof-size" } else { "ref-time" },
      "prior_usage": [prior.ref_time(), prior.proof_size()],
      "prior_reserved_equals_settled": true,
      "setup_dispatch_weight": [setup_dispatch.ref_time(), setup_dispatch.proof_size()],
      "remaining_before_seed": [remaining.ref_time(), remaining.proof_size()],
      "required": [required.ref_time(), required.proof_size()],
      "paid_refusal_control": [housekeeping.saturating_add(inspection).ref_time(), housekeeping.saturating_add(inspection).proof_size()],
      "user_attempts": 0, "user_pipeline_fee": "0", "user_action_fee": "0", "cursor_unchanged": true,
    }));
    let recipient_before = Balances::free_balance(common::BOB);
    let sink_before = Balances::free_balance(&sink);
    let accounts = ids.iter().map(|id| Actors::actor_identity(*id).unwrap().sovereign_account).collect::<Vec<_>>();
    let source_before = accounts.iter().map(Balances::free_balance).collect::<Vec<_>>();
    control_block(&mut witness, &case, now + 1, block(now + 1, &case));
    let transfers = System::events().into_iter().filter_map(|record| match record.event {
      RuntimeEvent::Actors(pallet_deos_actors::Event::TransferExecuted { actor_id, step_index, amount: actual, .. }) => {
        assert!(ids.contains(&actor_id));
        assert_eq!(actual, amount);
        Some((actor_id, step_index))
      }
      RuntimeEvent::Actors(pallet_deos_actors::Event::StepFailed { .. } | pallet_deos_actors::Event::StepSkipped { .. }) => panic!("recovery must execute useful Steps"),
      _ => None,
    }).collect::<Vec<_>>();
    assert_eq!(transfers, ids.iter().map(|id| (*id, u32::from(running))).collect::<Vec<_>>());
    let mut user_fees = 0u128;
    for (index, id) in ids.iter().enumerate() {
      let run = Actors::actor_run_state(*id).unwrap();
      assert_eq!(run.cursor, u32::from(running) + 1);
      assert_eq!(run.eligible_at, now + 2);
      assert_eq!(Actors::actor_state_hold(*id), holds[index]);
      let fee = quotes[index].maximum_next_action_fee.maximum_effect_fee
        + if running { 0 } else { quotes[index].prospective_pipeline_fee.as_ref().unwrap().total_fee };
      assert_eq!(source_before[index] - Balances::free_balance(&accounts[index]), amount + fee);
      user_fees += fee;
    }
    assert_eq!(Balances::free_balance(common::BOB) - recipient_before, 2 * amount);
    assert_eq!(Balances::free_balance(&sink) - sink_before, user_fees);
    eprintln!("ACTORS_H1 {}", json!({
      "schema": 1, "row": "resource_recovery", "case": case, "block": now + 1,
      "ordered_transfers": transfers, "user_fees": user_fees.to_string(),
      "next_eligible_block": now + 2, "retained_holds": true, "duplicate_steps": 0,
    }));
    polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version())
  });
  witness.roots.push(root);
  witness
}
