use super::*;
use crate::weights::WeightInfo as _;
use crate::{
  ActorProcesses, ActorRef, BlockResourceDomain, BlockResourceLimits, BlockResourceState,
  ServiceHeader, ServiceHeaderRecord, ServiceNode, ServiceNodes, ServiceRoundEncounter,
  ServiceRoundError,
};

// A native declaration token, not a measured or accepted physical coefficient.
const COMPLETION: Weight = Weight::from_parts(701, 113);

fn root() -> Vec<u8> {
  polkadot_sdk::sp_io::storage::root(StateVersion::V1)
}

fn serve(now: u64) -> Result<ServiceRoundEncounter, ServiceRoundError> {
  Actors::service_canonical_round_head(&mut WeightMeter::with_limit(Weight::MAX), now)
}

fn create(class: ActorType, steps: crate::ContractSteps<Test>) -> ActorRef {
  let id = match class {
    ActorType::User => create_user_with(ALICE, Mutability::Mutable, manual_schedule(), None, steps),
    ActorType::System => create_system_with(ALICE, manual_schedule(), None, steps),
  };
  fund_native(id, 1_000_000_000_000);
  assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
  Actors::load_actor_ref(id).unwrap()
}

// All prefixes are established through real Opening/Task/typed-failure commits, without an
// injected Run, re-trigger, altered counter or scheduler carrier.
fn prepare(source: u8) -> ActorRef {
  System::set_block_number(1);
  let steps = if source == 0 {
    BoundedVec::default()
  } else {
    let mut step = transfer_contract_steps(BOB, 1)[0].clone();
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    BoundedVec::try_from(vec![step.clone(), step]).unwrap()
  };
  let actor = create(ActorType::User, steps);
  System::set_block_number(2);
  if source >= 2 {
    assert_eq!(serve(2), Ok(ServiceRoundEncounter::Eligible(actor)));
    System::set_block_number(3);
    assert_eq!(Actors::actor_run_state(actor.actor_id).unwrap().cursor, 1);
  }
  if source == 3 {
    set_temporary_transfer_failure_to(Some(BOB));
    assert_eq!(serve(3), Ok(ServiceRoundEncounter::Eligible(actor)));
    set_temporary_transfer_failure_to(None);
    System::set_block_number(4);
    let run = Actors::actor_run_state(actor.actor_id).unwrap();
    assert_eq!(run.unsuccessful_attempts_at_cursor, 1);
    assert_eq!(run.eligible_at, 4);
    assert!(ServiceNodes::<Test>::contains_key(actor.actor_id));
  }
  clear_fee_collections();
  actor
}

struct Before {
  root: Vec<u8>,
  header: ServiceHeaderRecord<u64>,
  nodes: Vec<(ActorRef, ServiceNode<u64>)>,
}

impl Before {
  fn capture(actors: &[ActorRef]) -> Self {
    Self {
      root: root(),
      header: ServiceHeader::<Test>::get(),
      nodes: actors
        .iter()
        .map(|actor| (*actor, ServiceNodes::<Test>::get(actor.actor_id).unwrap()))
        .collect(),
    }
  }

  fn assert_only_turns(&self, now: u64, cursor: ActorRef) {
    let mut expected_header = self.header;
    expected_header.round_block = Some(now);
    expected_header.cursor = Some(cursor);
    assert_eq!(ServiceHeader::<Test>::get(), expected_header);
    for (actor, node) in &self.nodes {
      let mut expected = *node;
      expected.last_considered = now;
      assert_eq!(ServiceNodes::<Test>::get(actor.actor_id), Some(expected));
    }
    // A raw rollback layer keeps FRAME's transaction-level key at the captured caller depth.
    polkadot_sdk::sp_io::storage::start_transaction();
    ServiceHeader::<Test>::put(self.header);
    for (actor, node) in &self.nodes {
      ServiceNodes::<Test>::insert(actor.actor_id, node);
    }
    let normalized = root();
    polkadot_sdk::sp_io::storage::rollback_transaction();
    assert_eq!(
      normalized, self.root,
      "no other consensus key or field may change"
    );
  }
}

fn envelope(actor: ActorRef) -> crate::ActorStepResourceEnvelope {
  type W = <Test as crate::Config>::WeightInfo;
  let crate::LoadedActorStateOf::<Test>::Active(state) = Actors::load_actor_state(actor.actor_id)
  else {
    panic!("prepared active source");
  };
  let (inner, step) = if state.contract.steps.is_empty() {
    (
      Actors::zero_step_resource_envelope(&state.contract).unwrap(),
      None,
    )
  } else {
    let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
    let loaded = Actors::load_current_step_from_storage(actor.actor_id, cursor).unwrap();
    (loaded.resources, Some(loaded.step))
  };
  let control = W::service_round_begin_populated()
    .saturating_add(W::service_round_probe_eligible())
    .saturating_add(W::scheduler_actor_state_probe())
    .saturating_add(inner.control)
    .saturating_add(
      W::service_round_admit_eligible()
        .max(W::service_member_retire_interior())
        .max(W::service_member_retire_pair_cursor())
        .max(W::service_member_retire_singleton()),
    )
    .saturating_add(Actors::service_terminal_control_upper(
      &state,
      step.as_ref(),
    ))
    .saturating_add(COMPLETION);
  crate::ActorStepResourceEnvelope {
    control,
    effect: inner.effect,
  }
}

fn resources(
  now: u64,
  control: Weight,
  effect: Weight,
) -> (BlockResourceLimits, BlockResourceState<u64>) {
  let limits = BlockResourceLimits::new(control, effect.saturating_mul(2), effect, effect).unwrap();
  let mut state = BlockResourceState::new(now);
  assert_ok!(state.begin_prepass(limits.into_budget().unwrap()));
  (limits, state)
}

#[test]
fn consumed_refusal_preserves_exact_source_and_paid_rights_for_all_four_boundaries() {
  for source in 0..4 {
    new_test_ext().execute_with(|| {
      let actor = prepare(source);
      let follower = create(ActorType::System, transfer_contract_steps(CHARLIE, 1));
      let now = System::block_number() + 1;
      System::set_block_number(now);
      set_service_refusal_completion_weight(Some(COMPLETION));
      let admitted = envelope(actor);
      let (limits, mut resources) = resources(now, admitted.control, admitted.effect);
      let before = Before::capture(&[actor]);
      let process = ActorProcesses::<Test>::get(actor.actor_id).unwrap();
      set_fail_fee_sink_transfer(true);
      let mut meter = WeightMeter::with_limit(Weight::MAX);
      assert_eq!(
        Actors::service_canonical_round_head_with_resources(
          &mut meter,
          now,
          &mut resources,
          limits,
          BlockResourceDomain::ActorBaseEffect,
        ),
        Ok(ServiceRoundEncounter::ConsumedRefusal(actor))
      );
      set_fail_fee_sink_transfer(false);
      before.assert_only_turns(now, follower);
      assert_eq!(ActorProcesses::<Test>::get(actor.actor_id), Some(process));
      assert_eq!(resources.usage().actor_control_used(), admitted.control);
      assert_eq!(resources.usage().actor_effect_used(), admitted.effect);
      assert_eq!(resources.outstanding_reservations(), 0);
      assert!(!resources.optional_actor_work_halted());
      assert_eq!(
        meter.consumed(),
        admitted.control.saturating_add(admitted.effect)
      );
      assert_eq!(serve(now), Ok(ServiceRoundEncounter::Eligible(follower)));
      assert_eq!(serve(now), Ok(ServiceRoundEncounter::Closed));
      let frozen = root();
      assert_eq!(serve(now), Ok(ServiceRoundEncounter::Closed));
      assert_eq!(root(), frozen, "Drain cannot issue a second turn");
      clear_fee_collections();
      System::set_block_number(now + 1);
      assert_eq!(serve(now + 1), Ok(ServiceRoundEncounter::Eligible(actor)));
      assert_eq!(
        fee_collections().len(),
        if source == 1 { 2 } else { 1 },
        "source {source}: only the owed Pipeline and/or Action is charged"
      );
      #[cfg(feature = "try-runtime")]
      Actors::do_try_state().unwrap();
    });
  }
}

#[test]
fn consumed_refusal_rolls_back_pipeline_task_callback_and_outer_service_transaction() {
  for outer_rollback in [false, true] {
    new_test_ext().execute_with(|| {
      let actor = prepare(1);
      set_service_refusal_completion_weight(Some(COMPLETION));
      let before = Before::capture(&[actor]);
      let now = System::block_number();
      set_fee_collection_hook(|| {
        polkadot_sdk::sp_io::storage::set(b"service-refusal-callback", b"tentative");
        set_fail_fee_sink_transfer(true);
      });
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let inside = Before::capture(&[actor]);
        assert_eq!(
          serve(now),
          Ok(ServiceRoundEncounter::ConsumedRefusal(actor))
        );
        assert_eq!(
          fee_collections().len(),
          2,
          "Pipeline then late Action collection attempted"
        );
        inside.assert_only_turns(now, actor);
        if outer_rollback {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
        } else {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
        }
      });
      set_fail_fee_sink_transfer(false);
      if outer_rollback {
        assert_eq!(
          root(),
          before.root,
          "outer rollback also reverts Service control"
        );
      } else {
        before.assert_only_turns(now, actor);
      }
      assert!(polkadot_sdk::sp_io::storage::get(b"service-refusal-callback").is_none());
    });
  }
}

#[test]
fn consumed_refusal_requires_full_meter_and_frozen_control_admission_on_both_axes() {
  for proof in [false, true] {
    for ledger_limited in [false, true] {
      new_test_ext().execute_with(|| {
        let actor = prepare(1);
        set_service_refusal_completion_weight(Some(COMPLETION));
        let full = envelope(actor);
        let shortage = if proof {
          Weight::from_parts(0, 1)
        } else {
          Weight::from_parts(1, 0)
        };
        let control = full.control.checked_sub(&shortage).unwrap();
        let now = System::block_number();
        let (limits, mut state) = resources(
          now,
          if ledger_limited {
            control
          } else {
            full.control
          },
          full.effect,
        );
        let mut meter = WeightMeter::with_limit(if ledger_limited {
          Weight::MAX
        } else {
          control.saturating_add(full.effect)
        });
        let before = root();
        assert_eq!(
          Actors::service_canonical_round_head_with_resources(
            &mut meter,
            now,
            &mut state,
            limits,
            BlockResourceDomain::ActorBaseEffect,
          ),
          Err(if ledger_limited {
            ServiceRoundError::ResourceUnavailable
          } else {
            ServiceRoundError::InsufficientWeight
          })
        );
        assert_eq!(root(), before);
        assert!(fee_collections().is_empty());
        assert_eq!(state.usage().actor_effect_used(), Weight::zero());
        assert_eq!(state.outstanding_reservations(), 0);
        assert!(!state.optional_actor_work_halted());
      });
    }
  }
}

#[test]
fn consumed_refusal_cannot_bypass_effect_capacity_on_either_axis() {
  for proof in [false, true] {
    new_test_ext().execute_with(|| {
      let actor = prepare(1);
      set_service_refusal_completion_weight(Some(COMPLETION));
      let full = envelope(actor);
      let shortage = if proof {
        Weight::from_parts(0, 1)
      } else {
        Weight::from_parts(1, 0)
      };
      let now = System::block_number();
      let (limits, mut state) = resources(
        now,
        full.control,
        full.effect.checked_sub(&shortage).unwrap(),
      );
      let before = root();
      assert_eq!(
        Actors::service_canonical_round_head_with_resources(
          &mut WeightMeter::with_limit(Weight::MAX),
          now,
          &mut state,
          limits,
          BlockResourceDomain::ActorBaseEffect,
        ),
        Err(ServiceRoundError::ResourceUnavailable)
      );
      assert_eq!(root(), before);
      assert!(fee_collections().is_empty());
      assert_eq!(state.usage().actor_effect_used(), Weight::zero());
      assert_eq!(state.outstanding_reservations(), 0);
    });
  }
}

#[test]
fn consumed_refusal_simulation_cannot_consume_a_real_turn() {
  new_test_ext().execute_with(|| {
    let actor = prepare(1);
    set_service_refusal_completion_weight(Some(COMPLETION));
    let crate::LoadedActorStateOf::<Test>::Active(state) = Actors::load_actor_state(actor.actor_id)
    else {
      panic!("active source");
    };
    let before = root();
    set_fail_fee_sink_transfer(true);
    assert_eq!(
      Actors::simulate_current_contract(
        actor.actor_id,
        ActorType::User,
        Mutability::Mutable,
        state.contract,
        SimulationMode::FreshCurrentPlan,
        ample_simulation_budget(),
      ),
      Err(SimulationError::FeeCollectionFailed)
    );
    set_fail_fee_sink_transfer(false);
    assert_eq!(root(), before);
  });
}

#[test]
fn consumed_refusal_is_unavailable_without_a_complete_host_bound() {
  new_test_ext().execute_with(|| {
    prepare(0);
    let before = root();
    set_fail_fee_sink_transfer(true);
    assert_eq!(
      serve(System::block_number()),
      Err(ServiceRoundError::FeeCollection)
    );
    set_fail_fee_sink_transfer(false);
    assert_eq!(root(), before);
    set_service_refusal_completion_weight(Some(COMPLETION));
  });
  new_test_ext().execute_with(|| {
    assert_eq!(<MockAdmissionCertificateAuthority as crate::AdmissionCertificateAuthorityProvider>::service_refusal_completion_weight(), None);
  });
}

#[test]
fn consumed_refusal_rejects_invalid_effect_control_and_ring_evidence() {
  for fault in 0..5 {
    new_test_ext().execute_with(|| {
      let actor = prepare(2);
      let _follower = create(ActorType::System, transfer_contract_steps(CHARLIE, 1));
      let now = System::block_number() + 1;
      System::set_block_number(now);
      set_service_refusal_completion_weight(Some(COMPLETION));
      match fault {
        0 => set_missing_task_effect_actual_weight(true),
        1 => set_task_effect_actual_weight_override(Some(Weight::MAX)),
        2 => set_missing_step_control_actual_weight(true),
        3 => set_step_control_actual_weight_override(Some(Weight::MAX)),
        _ => {
          ServiceNodes::<Test>::mutate(actor.actor_id, |node| {
            node.as_mut().unwrap().next.generation += 1;
          });
          set_fail_fee_sink_transfer(true);
        }
      }
      let before = root();
      assert!(
        serve(now).is_err(),
        "unsafe evidence must never consume a turn"
      );
      assert_eq!(root(), before);
      set_fail_fee_sink_transfer(false);
    });
  }
}

#[test]
fn consumed_refusal_does_not_report_an_all_refused_round_as_recovery() {
  new_test_ext().execute_with(|| {
    let actor = prepare(0);
    let peer = create(ActorType::User, BoundedVec::default());
    let now = System::block_number() + 1;
    System::set_block_number(now);
    set_service_refusal_completion_weight(Some(COMPLETION));
    let before = Before::capture(&[actor, peer]);
    clear_fee_collections();
    set_fail_fee_sink_transfer(true);
    let first = envelope(actor);
    let second = envelope(peer);
    let selector = TestWeightInfo::service_round_begin_populated()
      .saturating_add(TestWeightInfo::service_round_probe_eligible());
    let control = first
      .control
      .saturating_add(second.control)
      .saturating_add(selector);
    let (limits, mut state) = resources(now, control, Weight::zero());
    let pass = Actors::execute_cycle_to_cutoff_with_resources(
      control,
      0,
      &mut state,
      limits,
      BlockResourceDomain::ActorBaseEffect,
      control,
    );
    set_fail_fee_sink_transfer(false);
    assert_eq!(state.usage().actor_control_used(), pass.consumed);
    assert_eq!(state.outstanding_reservations(), 0);
    assert!(!state.optional_actor_work_halted());
    assert!(pass.starvation_observed);
    assert!(pass.starved);
    assert_eq!(
      fee_collections().len(),
      2,
      "one refused Opening per member, not a hot loop"
    );
    before.assert_only_turns(now, actor);
  });
}

#[test]
fn consumed_refusal_pass_progress_reconciles_control_and_effects_without_halting_followers() {
  new_test_ext().execute_with(|| {
    let actor = prepare(2);
    let follower = create(ActorType::System, transfer_contract_steps(CHARLIE, 1));
    let now = System::block_number() + 1;
    System::set_block_number(now);
    set_service_refusal_completion_weight(Some(COMPLETION));
    let first = envelope(actor);
    let second = envelope(follower);
    let selector = TestWeightInfo::service_round_begin_populated()
      .saturating_add(TestWeightInfo::service_round_probe_eligible());
    let control = first
      .control
      .saturating_add(second.control)
      .saturating_add(selector);
    let effect = first.effect.saturating_add(second.effect);
    let (limits, mut state) = resources(now, control, effect);
    let before_run = Actors::actor_run_state(actor.actor_id).unwrap().encode();
    let before_recipient = native_balance(&CHARLIE);
    clear_fee_collections();
    set_fail_fee_sink_transfer(true);
    let pass = Actors::execute_cycle_to_cutoff_with_resources(
      control.saturating_add(effect),
      0,
      &mut state,
      limits,
      BlockResourceDomain::ActorBaseEffect,
      control,
    );
    set_fail_fee_sink_transfer(false);
    assert!(!pass.starved);
    assert!(!pass.effect_reconciliation_uncertain);
    assert_eq!(
      state.usage().actor_control_used(),
      pass.reconciled_domains().unwrap().0
    );
    assert_eq!(state.usage().actor_effect_used(), effect);
    assert_eq!(state.outstanding_reservations(), 0);
    assert!(!state.optional_actor_work_halted());
    assert_eq!(
      Actors::actor_run_state(actor.actor_id).unwrap().encode(),
      before_run
    );
    assert_eq!(native_balance(&CHARLIE), before_recipient + 1);
    assert_eq!(
      fee_collections().len(),
      1,
      "System follower is Actor-fee exempt"
    );
    let before = root();
    assert_eq!(serve(now), Ok(ServiceRoundEncounter::Closed));
    assert_eq!(root(), before);
    #[cfg(feature = "try-runtime")]
    Actors::do_try_state().unwrap();
  });
}

#[test]
fn consumed_refusal_keeps_public_reentry_rate_limit_and_replacement_b_plus_one() {
  new_test_ext().execute_with(|| {
    let actor = prepare(1);
    let now = System::block_number();
    set_service_refusal_completion_weight(Some(COMPLETION));
    set_fail_fee_sink_transfer(true);
    assert_eq!(
      serve(now),
      Ok(ServiceRoundEncounter::ConsumedRefusal(actor))
    );
    set_fail_fee_sink_transfer(false);
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      actor.actor_id
    ));
    let replacement_contract =
      user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)).unwrap();
    let disabled = root();
    assert_noop!(
      Actors::activate_actor(
        RuntimeOrigin::signed(ALICE),
        actor.actor_id,
        replacement_contract.clone()
      ),
      Error::<Test>::ControlMutationRateLimited
    );
    assert_eq!(root(), disabled);
    System::set_block_number(now + 1);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      actor.actor_id,
      replacement_contract
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor.actor_id
    ));
    let replacement = Actors::load_actor_ref(actor.actor_id).unwrap();
    assert!(replacement.generation > actor.generation);
    let node = ServiceNodes::<Test>::get(actor.actor_id).unwrap();
    assert_eq!(node.eligible_from, now + 2);
    let before = root();
    assert_eq!(serve(now + 1), Ok(ServiceRoundEncounter::Closed));
    assert_eq!(root(), before);
  });
}

fn changed_authority() -> crate::AdmissionCertificateAuthority {
  use crate::AdmissionCertificateAuthorityProvider as _;
  let mut authority = MockAdmissionCertificateAuthority::current().unwrap();
  authority.production_weight_identity[0] ^= 1;
  authority
}

#[test]
fn fresh_cycle_authority_rejects_stale_idle_without_mutation() {
  for class in [ActorType::User, ActorType::System] {
    for zero_step in [false, true] {
      new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let steps = if zero_step {
          BoundedVec::default()
        } else {
          transfer_contract_steps(BOB, 1)
        };
        let actor = create(class, steps);
        System::set_block_number(2);
        let before = root();
        set_admission_authority_override(Some(changed_authority()));
        clear_fee_collections();
        let mut meter = WeightMeter::with_limit(Weight::MAX);
        assert!(Actors::service_canonical_round_head(&mut meter, 2).is_err());
        assert_eq!(
          root(),
          before,
          "stale classification changes no consensus state"
        );
        assert!(fee_collections().is_empty());
        assert!(meter.consumed().ref_time() > 0);
        assert!(meter.consumed().proof_size() > 0);
        set_admission_authority_override(None);
        assert_eq!(serve(2), Ok(ServiceRoundEncounter::Eligible(actor)));
        assert_eq!(
          Actors::actor_identity(actor.actor_id).unwrap().cycle_nonce,
          1
        );
      });
    }
  }
}

#[test]
fn fresh_cycle_authority_rejects_opening_and_late_action_callbacks() {
  for callback_at_action in [false, true] {
    for zero_step in [false, true] {
      if callback_at_action && zero_step {
        continue; // Zero-Step has no Action collector.
      }
      new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let steps = if zero_step {
          BoundedVec::default()
        } else {
          transfer_contract_steps(BOB, 1)
        };
        let actor = create(ActorType::User, steps);
        System::set_block_number(2);
        let changed = changed_authority();
        let before = root();
        let admitted = envelope(actor);
        let (limits, mut state) = resources(2, admitted.control, admitted.effect);
        clear_fee_collections();
        if callback_at_action {
          set_fee_collection_hook(move || {
            set_fee_collection_hook(move || set_admission_authority_override(Some(changed)));
          });
        } else {
          set_fee_collection_hook(move || set_admission_authority_override(Some(changed)));
        }
        let mut meter = WeightMeter::with_limit(Weight::MAX);
        assert!(
          Actors::service_canonical_round_head_with_resources(
            &mut meter,
            2,
            &mut state,
            limits,
            BlockResourceDomain::ActorBaseEffect,
          )
          .is_err()
        );
        assert_eq!(
          root(),
          before,
          "callbacks cannot commit a stale new Cycle, fee or effect"
        );
        assert_eq!(
          fee_collections().len(),
          if callback_at_action { 2 } else { 1 },
          "collector attempts are diagnostics, not committed fees"
        );
        assert!(state.usage().actor_control_used().ref_time() > 0);
        assert!(state.usage().actor_control_used().proof_size() > 0);
        assert_eq!(state.outstanding_reservations(), 0);
        assert_eq!(
          state.usage().actor_effect_used(),
          if callback_at_action {
            admitted.effect
          } else {
            Weight::zero()
          },
          "performed Task work survives rollback, uninvoked work is zero"
        );
        set_admission_authority_override(None);
        assert_eq!(serve(2), Ok(ServiceRoundEncounter::Eligible(actor)));
      });
    }
  }
}

#[test]
fn fresh_cycle_authority_preserves_paid_running_and_suspended_prefixes() {
  for source in [2, 3] {
    new_test_ext().execute_with(|| {
      let actor = prepare(source);
      let now = System::block_number();
      let run = Actors::actor_run_state(actor.actor_id).unwrap();
      let recipient = native_balance(&BOB);
      let before = root();
      set_admission_authority_override(Some(changed_authority()));
      assert!(
        serve(now).is_err(),
        "stale admission refuses without cancelling paid rights"
      );
      assert_eq!(root(), before);
      assert_eq!(Actors::actor_run_state(actor.actor_id), Some(run));
      assert_eq!(
        native_balance(&BOB),
        recipient,
        "the earlier committed Step is durable"
      );
      assert!(fee_collections().is_empty());
      set_admission_authority_override(None);
      assert_eq!(serve(now), Ok(ServiceRoundEncounter::Eligible(actor)));
      assert_eq!(native_balance(&BOB), recipient + 1);
      assert_eq!(
        fee_collections(),
        vec![100],
        "only Action, no second Pipeline payment"
      );
    });
  }
}

#[test]
fn fresh_cycle_authority_admits_classification_on_both_axes_before_loading() {
  for proof in [false, true] {
    new_test_ext().execute_with(|| {
      let actor = prepare(0);
      let now = System::block_number();
      let inspection = TestWeightInfo::service_round_begin_populated()
        + TestWeightInfo::service_round_probe_eligible()
        + TestWeightInfo::scheduler_actor_state_probe();
      let limit = inspection
        .checked_sub(&if proof {
          Weight::from_parts(0, 1)
        } else {
          Weight::from_parts(1, 0)
        })
        .unwrap();
      let (limits, mut state) = resources(now, limit, Weight::zero());
      let before = root();
      clear_fee_collections();
      #[cfg(feature = "runtime-benchmarks")]
      take_admission_authority_calls();
      let mut meter = WeightMeter::with_limit(Weight::MAX);
      assert!(
        Actors::service_canonical_round_head_with_resources(
          &mut meter,
          now,
          &mut state,
          limits,
          BlockResourceDomain::ActorBaseEffect,
        )
        .is_err()
      );
      #[cfg(feature = "runtime-benchmarks")]
      assert_eq!(
        take_admission_authority_calls(),
        0,
        "no unpaid authority getter"
      );
      assert_eq!(root(), before);
      assert!(fee_collections().is_empty());
      assert_eq!(state.outstanding_reservations(), 0);
      assert!(meter.consumed().all_lte(limit));
      assert!(ServiceNodes::<Test>::contains_key(actor.actor_id));
    });
  }
}

#[test]
fn fresh_cycle_authority_authorized_replacement_restores_eligibility() {
  new_test_ext().execute_with(|| {
    use crate::AdmissionCertificateAuthorityProvider as _;
    let actor = prepare(1);
    let mut contract = Actors::actor_contract(actor.actor_id).unwrap();
    let certificate = Actors::load_control_admission(actor.actor_id).unwrap();
    let mut renewed = MockAdmissionCertificateAuthority::current().unwrap();
    renewed.runtime_actor_semantics_version += 1;
    set_admission_authority_override(Some(renewed));
    let now = System::block_number();
    assert!(serve(now).is_err());
    // Use the authorized replacement transaction, not an internal storage-only renewal seam.
    contract.cooldown_blocks = 1;
    assert_ok!(Actors::update_contract(
      RuntimeOrigin::signed(ALICE),
      actor.actor_id,
      contract,
    ));
    let current = Actors::load_control_admission(actor.actor_id).unwrap();
    assert_ne!(current.admission_identity, certificate.admission_identity);
    assert!(Actors::admission_matches_current_authority(&current));
    assert!(Actors::actor_run_state(actor.actor_id).is_none());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor.actor_id
    ));
    let replacement = Actors::load_actor_ref(actor.actor_id).unwrap();
    assert!(replacement.generation > actor.generation);
    assert_eq!(
      ServiceNodes::<Test>::get(actor.actor_id)
        .unwrap()
        .eligible_from,
      now + 1
    );
    System::set_block_number(now + 1);
    assert_eq!(
      serve(now + 1),
      Ok(ServiceRoundEncounter::Eligible(replacement))
    );
    assert_eq!(Actors::actor_run_state(actor.actor_id).unwrap().cursor, 1);
  });
}
