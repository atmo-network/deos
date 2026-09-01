use super::*;

#[test]
fn loaded_step_returns_exact_persisted_successor_run() {
  use crate::AttemptDisposition;

  // Source states are reached through ordinary Opening/Q1 service. Only the final bounded
  // core atom is inspected before successor publication, inside a rolled-back transaction.
  for (name, count, initial_funding, advance, refill, retry, expected, cursor) in [
    (
      "opening progress",
      3,
      10,
      false,
      0,
      false,
      AttemptDisposition::Continued,
      Some(1),
    ),
    (
      "running progress",
      3,
      10,
      true,
      0,
      false,
      AttemptDisposition::Continued,
      Some(2),
    ),
    (
      "first retry",
      2,
      0,
      false,
      0,
      true,
      AttemptDisposition::Suspended,
      Some(0),
    ),
    (
      "repeat retry",
      2,
      0,
      true,
      0,
      true,
      AttemptDisposition::Suspended,
      Some(0),
    ),
    (
      "suspended progress",
      2,
      0,
      true,
      10,
      true,
      AttemptDisposition::Continued,
      Some(1),
    ),
    (
      "opening complete",
      1,
      10,
      false,
      0,
      false,
      AttemptDisposition::Completed,
      None,
    ),
    (
      "running complete",
      2,
      10,
      true,
      0,
      false,
      AttemptDisposition::Completed,
      None,
    ),
    (
      "suspended complete",
      1,
      0,
      true,
      10,
      true,
      AttemptDisposition::Completed,
      None,
    ),
    (
      "opening failed",
      1,
      10,
      false,
      0,
      false,
      AttemptDisposition::Failed,
      None,
    ),
    (
      "running failed",
      2,
      10,
      true,
      0,
      false,
      AttemptDisposition::Failed,
      None,
    ),
  ] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let mut step = make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Local(1),
        amount: AmountResolution::Fixed(1),
      });
      if retry {
        step.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
      }
      let steps = BoundedVec::try_from(vec![step; count]).expect("bounded test Contract");
      let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
      let sovereign = Actors::actor_identity(actor_id)
        .expect("admitted identity")
        .sovereign_account;
      if initial_funding > 0 {
        set_asset_balance(&sovereign, TestAsset::Local(1), initial_funding);
      }
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      if advance {
        Actors::on_initialize(1);
        run_prepass();
        Actors::on_idle(1, Weight::MAX);
        let run = Actors::actor_run_state(actor_id).expect("first ordinary attempt retains Run");
        System::set_block_number(run.eligible_at);
      }
      if refill > 0 {
        set_asset_balance(&sovereign, TestAsset::Local(1), refill);
      }
      if expected == AttemptDisposition::Failed {
        set_fail_transfer_to(Some(BOB));
      }
      let now = System::block_number();
      let (state, admission, loaded_step) = Actors::load_current_step_service_state(actor_id)
        .expect("real source has complete canonical authority");
      let instance = Actors::derive_active_actor_view(
        state.identity.clone(),
        state.hot.clone(),
        state.contract.clone(),
      );
      let ticket = Actors::build_actor_step_ticket(
        actor_id,
        state.hot.queue_ticket.expect("ordinary successor is Ready"),
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("real source ticket");
      let fee = Actors::maximum_current_action_fee(
        ActorType::System,
        &loaded_step.step,
        loaded_step.resources,
      )
      .expect("System fee envelope");
      let plan = Actors::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        fee,
      )
      .expect("source plan is coherent");
      let corruption_plan = plan.clone();
      let mut successor_parts = None;
      let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      assert_ok!(polkadot_sdk::frame_support::storage::with_transaction(
        || {
          assert_ok!(Actors::paged_consume_head_at(ticket.ticket));
          let (returned, _, disposition, _, _) =
            Actors::execute_loaded_single_step_core(actor_id, &instance, plan, now, count as u32)
              .unwrap_or_else(|error| panic!("{name}: core failed: {error:?}"));
          assert_eq!(disposition, expected, "{name}");
          assert_eq!(
            returned.run.as_ref().map(|run| run.cursor),
            cursor,
            "{name}"
          );
          assert_eq!(
            returned.run.as_ref().map(Encode::encode),
            Actors::actor_run_state(actor_id)
              .as_ref()
              .map(Encode::encode),
            "{name}: returned authority must be the persisted successor, including terminal None",
          );
          if name == "opening progress" {
            successor_parts = Some((
              crate::ActorRunHeads::<Test>::get(actor_id).expect("real successor head"),
              crate::ActorRunPayloads::<Test>::get(actor_id).expect("real successor payload"),
            ));
          }
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(
            Ok::<(), DispatchError>(()),
          )
        }
      ));
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        before
      );
      if let Some((head, payload)) = successor_parts {
        for head_only in [true, false] {
          if head_only {
            crate::ActorRunHeads::<Test>::insert(actor_id, head.clone());
          } else {
            crate::ActorRunPayloads::<Test>::insert(actor_id, &payload);
          }
          assert!(Actors::actor_run_state(actor_id).is_none());
          let corrupt_root =
            polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
          let custody = (
            asset_balance(&sovereign, TestAsset::Local(1)),
            asset_balance(&BOB, TestAsset::Local(1)),
          );
          assert_ok!(polkadot_sdk::frame_support::storage::with_transaction(
            || {
              assert_ok!(Actors::paged_consume_head_at(ticket.ticket));
              let events = System::events();
              let before_core =
                polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
              let result = Actors::execute_loaded_single_step_core(
                actor_id,
                &instance,
                corruption_plan.clone(),
                now,
                count as u32,
              );
              assert!(
                matches!(
                  result,
                  Err(crate::scheduler::AttemptTransactionError::Invariant)
                ),
                "Idle orphan partition must reject before effects; head_only={head_only}"
              );
              assert_eq!(
                polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
                before_core
              );
              assert_eq!(System::events(), events);
              assert_eq!(
                (
                  asset_balance(&sovereign, TestAsset::Local(1)),
                  asset_balance(&BOB, TestAsset::Local(1))
                ),
                custody
              );
              polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Ok::<
                (),
                DispatchError,
              >(()))
            }
          ));
          assert_eq!(
            polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
            corrupt_root
          );
          crate::ActorRunHeads::<Test>::remove(actor_id);
          crate::ActorRunPayloads::<Test>::remove(actor_id);
        }
      }
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn loaded_cancellation_missing_run_is_idle_only_and_never_resurrects_backing() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let mut state = Actors::active_actor_state(actor_id).expect("real suspended authority");
    let (_, cell) = Actors::actor_control_cell(actor_id).expect("canonical primary");
    let identity = state.identity.clone();
    let orphan_run = state.run_state.take().expect("Run remains persisted");
    let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_eq!(Actors::cancel_run_internal_loaded(actor_id, &identity, CancellationReason::Explicit, None,
      crate::execution::LoadedCancellationContext::RetainedFrame { admission: cell.admission, state }), Err(Error::<Test>::ActorRunInvariant.into()));
    assert_eq!(polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1), before);

    let idle_id = create_system_with(ALICE, manual_schedule(), None, BoundedVec::default());
    let idle = Actors::active_actor_state(idle_id).expect("real Idle authority");
    let (_, idle_cell) = Actors::actor_control_cell(idle_id).expect("Idle primary");
    assert!(idle.run_state.is_none());
    crate::ActorRunStateStore::<Test>::insert(idle_id, orphan_run);
    System::reset_events();
    let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_eq!(Actors::cancel_run_internal_loaded(idle_id, &idle.identity, CancellationReason::Explicit, None,
      crate::execution::LoadedCancellationContext::RetainedFrame { admission: idle_cell.admission.clone(), state: idle.clone() }), Ok(false));
    assert_eq!(polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1), before);
    assert_ok!(Actors::finalize_actor_from_retained_state(idle_id, idle, &idle_cell.admission, CloseReason::OwnerInitiated));
    assert!(!crate::ActorRunStateStore::<Test>::contains_key(idle_id));
    assert!(!has_actor_event(|event| matches!(event, Event::CycleCancelled { actor_id: id, .. } | Event::CycleSummary { actor_id: id, .. } if *id == idle_id)));
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn loaded_cancellation_uses_supplied_run_despite_missing_or_stale_backing() {
  for (consumed, waiting) in [(false, false), (true, false), (false, true)] {
    for missing in [false, true] {
      new_test_ext().execute_with(|| {
        let actor_id = create_suspended_system_retry(1);
        if waiting {
          let eligible_at = Actors::actor_run_state(actor_id).expect("first retry").eligible_at;
          System::set_block_number(eligible_at);
          Actors::on_initialize(eligible_at);
          run_prepass();
          Actors::on_idle(eligible_at, Weight::MAX);
        }
        let state = Actors::active_actor_state(actor_id).expect("real suspended authority");
        let (_, cell) = Actors::actor_control_cell(actor_id).expect("canonical primary");
        let identity = state.identity.clone();
        let supplied_run = state.run_state.clone().expect("supplied Run");
        assert_eq!(state.hot.cycle_state, CycleState::Suspended);
        assert_eq!(state.hot.wakeup_pointer.is_some(), waiting);
        if consumed {
          assert_ok!(Actors::remove_primary_control_cell_inner(actor_id));
        }
        if missing {
          crate::ActorRunStateStore::<Test>::remove(actor_id);
        } else {
          let mut stale = supplied_run.clone();
          stale.cycle_nonce = stale.cycle_nonce.checked_add(7).unwrap();
          crate::ActorRunStateStore::<Test>::insert(actor_id, stale);
        }
        let context = if consumed {
          crate::execution::LoadedCancellationContext::ConsumedFrame { admission: cell.admission, state }
        } else {
          crate::execution::LoadedCancellationContext::RetainedFrame { admission: cell.admission, state }
        };
        System::reset_events();
        assert_eq!(Actors::cancel_run_internal_loaded(actor_id, &identity, CancellationReason::Explicit, None, context), Ok(true));
        assert!(Actors::actor_run_state(actor_id).is_none());
        let successor = Actors::active_actor_state(actor_id).expect("Idle successor is canonical");
        assert_eq!(successor.identity.cycle_nonce, supplied_run.cycle_nonce);
        assert_eq!(successor.hot.cycle_state, CycleState::Idle);
        assert!(successor.hot.wakeup_pointer.is_none());
        assert!(has_actor_event(|event| matches!(event, Event::CycleCancelled { actor_id: id, cycle_nonce, reason: CancellationReason::Explicit } if *id == actor_id && *cycle_nonce == supplied_run.cycle_nonce)));
        assert!(has_actor_event(|event| matches!(event, Event::CycleSummary { actor_id: id, cycle_nonce, result: CycleResult::Cancelled, outcomes } if *id == actor_id && *cycle_nonce == supplied_run.cycle_nonce && *outcomes == supplied_run.cumulative_outcomes)));
        #[cfg(feature = "try-runtime")]
        assert_ok!(Actors::do_try_state());
      });
    }
  }
}

fn current_step_execution_weight(actor_id: ActorId) -> Weight {
  let (_, _, step) = Actors::load_current_step_service_state(actor_id)
    .expect("current Step has canonical resource authority");
  step.resources.control.saturating_add(step.resources.effect)
}

#[test]
fn frozen_cycle_snapshot_dependency_is_closed_over_amounts_tasks_and_preconditions() {
  assert!(!AmountResolution::Fixed(1u64).requires_frozen_cycle_snapshot());
  assert!(
    !AmountResolution::<u64>::PercentageOfCurrent(Perbill::one()).requires_frozen_cycle_snapshot()
  );
  assert!(!AmountResolution::<u64>::AllAvailable.requires_frozen_cycle_snapshot());
  assert!(
    AmountResolution::<u64>::PercentageAtOpening(Perbill::one()).requires_frozen_cycle_snapshot()
  );
  assert!(
    AmountResolution::<u64>::PercentageOfLastFunding(Perbill::one())
      .requires_frozen_cycle_snapshot()
  );

  let mut step = make_step(Task::StopCycle);
  assert!(!step.requires_frozen_cycle_snapshot());
  step.precondition = timed_all_conditions(
    ObservationTiming::Opening,
    vec![Predicate::BlockNumberAbove { threshold: 0 }],
  );
  assert!(step.requires_frozen_cycle_snapshot());

  let current = StepOf::<Test> {
    precondition: timed_all_conditions(
      ObservationTiming::Current,
      vec![Predicate::BlockNumberAbove { threshold: 0 }],
    ),
    task: Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfCurrent(Perbill::one()),
    },
    on_error: StepErrorPolicy::AbortCycle,
  };
  assert!(!current.requires_frozen_cycle_snapshot());

  let opening_amount = make_step(Task::Transfer {
    to: BOB,
    asset: TestAsset::Native,
    amount: AmountResolution::PercentageAtOpening(Perbill::one()),
  });
  assert!(opening_amount.requires_frozen_cycle_snapshot());
}

#[test]
fn every_trigger_family_round_trips_dormant_and_active_lifecycle() {
  new_test_ext().execute_with(|| {
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let triggers = [
      RuntimeTrigger::manual(),
      RuntimeTrigger::address_event(SourceFilter::OwnerOnly, AssetFilter::Any),
      RuntimeTrigger::observation_change(7),
      RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
      RuntimeTrigger::cadenced(10),
    ];
    for (index, trigger) in triggers.into_iter().enumerate() {
      let create_block = index as u64 * 3 + 1;
      frame_system::Pallet::<Test>::set_block_number(create_block);
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        20_000 + index as u64,
        Mutability::Mutable,
        None,
      ));
      let actor_id = index as u64;
      assert!(matches!(
        Actors::load_actor_state(actor_id),
        LoadedActorStateOf::Dormant(_)
      ));
      let contract = system_active_contract(
        Schedule {
          trigger,
          cooldown_blocks: 0,
        },
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      )
      .expect("active Trigger contract");
      frame_system::Pallet::<Test>::set_block_number(create_block + 1);
      assert_ok!(Actors::activate_actor(
        RuntimeOrigin::root(),
        actor_id,
        contract,
      ));
      let active = Actors::active_actor_state(actor_id).expect("active Actor state");
      assert!(
        active
          .hot
          .trigger_runtime_state
          .is_compatible_with(&active.contract.trigger)
      );

      frame_system::Pallet::<Test>::set_block_number(create_block + 2);
      assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), actor_id));
      assert!(matches!(
        Actors::load_actor_state(actor_id),
        LoadedActorStateOf::Dormant(_)
      ));
      assert!(Actors::actor_hot(actor_id).is_none());
      assert!(Actors::load_actor_contract(actor_id).is_none());
      assert!(ActorFunding::<Test>::get(actor_id).is_none());
      assert!(Actors::crossing_membership(actor_id).is_none());
      assert!(Actors::actor_observation_feeds(actor_id).is_none());
    }
  });
}

#[test]
fn actor_step_metadata_is_a_closed_linear_control_surface() {
  let info = RuntimeStep::type_info();
  let TypeDef::Composite(definition) = info.type_def else {
    panic!("Step must remain a SCALE composite");
  };
  assert_eq!(definition.fields.len(), 3);
  assert_eq!(definition.fields[0].name, Some("precondition"));
  assert_eq!(definition.fields[1].name, Some("task"));
  assert_eq!(definition.fields[2].name, Some("on_error"));
  assert!(
    definition.fields[0]
      .type_name
      .unwrap_or_default()
      .contains("Option")
  );
  assert!(
    definition.fields[1]
      .type_name
      .unwrap_or_default()
      .contains("Task")
  );
  assert!(
    definition.fields[2]
      .type_name
      .unwrap_or_default()
      .contains("StepErrorPolicy")
  );
}

#[test]
fn task_failure_defaults_unknown_errors_to_permanent() {
  let error = DispatchError::Other("UnclassifiedAdapterFailure");
  assert_eq!(TaskFailure::from(error), TaskFailure::permanent(error));
  assert_eq!(TaskFailure::temporary(error).retry, RetryClass::Temporary);
}

#[test]
fn actor_run_schema_round_trips_retry_position_and_typed_snapshot_surfaces() {
  new_test_ext().execute_with(|| {
    let mut contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageAtOpening(Perbill::one()),
      }),
      make_step(Task::Unstake {
        asset: TestAsset::Local(1),
        shares: AmountResolution::PercentageAtOpening(Perbill::one()),
      }),
    ])
    .expect("two-step plan fits");
    contract_steps[0].on_error = RETRY_LATER;
    let actor_id = create_system_with(ALICE, percentage_trigger_schedule(), None, contract_steps);
    let mut opening_snapshot: BoundedBTreeMap<
      OpeningSurface<TestAsset>,
      Balance,
      <Test as crate::Config>::MaxOpeningSnapshotEntries,
    > = Default::default();
    opening_snapshot
      .try_insert(OpeningSurface::PreservableAsset(TestAsset::Native), 100)
      .expect("asset snapshot fits");
    opening_snapshot
      .try_insert(OpeningSurface::StakingShares(TestAsset::Local(1)), 40)
      .expect("staking snapshot fits");
    let run_state = RuntimeActorRunState {
      contract_authority: run_contract_authority(actor_id),
      cycle_nonce: 1,
      cursor: 0,
      opening_predicate_cursor: 0,
      unsuccessful_attempts_at_cursor: 1,
      last_attempt_block: 1,
      last_committed_step_block: None,
      eligible_at: 2,
      opening_snapshot,
      opening_predicate_results: Default::default(),
      funding_snapshot: Default::default(),
      cumulative_outcomes: OutcomeTotals::default(),
      last_step_outcome: Some(StepOutcome::FundingUnavailable),
      suspension: Some(SuspensionReason::FundingUnavailable),
    };
    let encoded = run_state.encode();
    let decoded = RuntimeActorRunState::decode(&mut &encoded[..]).expect("Actor run decodes");
    assert_eq!(decoded.cycle_nonce, 1);
    assert_eq!(decoded.cursor, 0);
    assert_eq!(decoded.unsuccessful_attempts_at_cursor, 1);
    assert_eq!(decoded.last_attempt_block, 1);
    assert_eq!(decoded.eligible_at, 2);
    assert_eq!(decoded.opening_snapshot.len(), 2);
    assert_eq!(
      decoded.last_step_outcome,
      Some(StepOutcome::FundingUnavailable)
    );
    assert_eq!(
      decoded.suspension,
      Some(SuspensionReason::FundingUnavailable)
    );

    assert_ok!(Actors::write_run_state(actor_id, Some(run_state)));
    assert_eq!(
      Actors::actor_identity(actor_id)
        .expect("actor identity")
        .cycle_nonce,
      0
    );
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("Actor run exists")
        .unsuccessful_attempts_at_cursor,
      1
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn ordinary_one_attempt_run_keeps_run_state_sparse() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_eq!(
      Actors::actor_hot(actor_id)
        .expect("actor exists")
        .cycle_state,
      CycleState::Idle
    );
    assert!(Actors::actor_run_state(actor_id).is_none());
    fund_native(actor_id, 10);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::actor_hot(actor_id)
        .expect("actor exists")
        .cycle_state,
      CycleState::Idle
    );
    assert!(Actors::actor_run_state(actor_id).is_none());
  });
}

#[test]
fn normal_running_progress_persists_one_causal_successor_and_rejects_stale_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![make_step(Task::StopCycle), make_step(Task::StopCycle)])
        .expect("two Steps fit"),
    );
    let running = RuntimeActorRunState {
      contract_authority: run_contract_authority(actor_id),
      cycle_nonce: 1,
      cursor: 1,
      opening_predicate_cursor: 0,
      unsuccessful_attempts_at_cursor: 0,
      last_attempt_block: 1,
      last_committed_step_block: Some(1),
      eligible_at: 2,
      opening_snapshot: Default::default(),
      opening_predicate_results: Default::default(),
      funding_snapshot: Default::default(),
      cumulative_outcomes: OutcomeTotals {
        executed_steps: 1,
        committed_effectful_tasks: 0,
        ..Default::default()
      },
      last_step_outcome: Some(StepOutcome::Stopped),
      suspension: None,
    };
    assert_ok!(Actors::persist_run_progress(actor_id, running.clone()));
    let stored = ActorRunStateStore::<Test>::get(actor_id).expect("Running state persists");
    assert_eq!(stored.cycle_nonce, running.cycle_nonce);
    assert_eq!(stored.cursor, running.cursor);
    assert_eq!(
      stored.last_committed_step_block,
      running.last_committed_step_block
    );
    assert_eq!(stored.eligible_at, running.eligible_at);
    assert_eq!(stored.cumulative_outcomes, running.cumulative_outcomes);
    assert_eq!(
      Actors::actor_hot(actor_id).map(|hot| hot.cycle_state),
      Some(CycleState::Running)
    );
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Active(_)
    ));
    ActorRunStateStore::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Running state exists")
        .contract_authority
        .admission_identity[0] ^= 1;
    });
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
  });
}

#[test]
fn running_state_carries_the_exact_opening_predicate_cursor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![
        StepOf::<Test> {
          precondition: timed_all_conditions(
            ObservationTiming::Opening,
            vec![Predicate::BlockNumberAbove { threshold: 10 }],
          ),
          task: Task::StopCycle,
          on_error: StepErrorPolicy::AbortCycle,
        },
        StepOf::<Test> {
          precondition: timed_all_conditions(
            ObservationTiming::Opening,
            vec![Predicate::BlockNumberAbove { threshold: 0 }],
          ),
          task: Task::Transfer {
            to: BOB,
            asset: TestAsset::Native,
            amount: AmountResolution::Fixed(5),
          },
          on_error: StepErrorPolicy::AbortCycle,
        },
      ])
      .expect("two Steps fit"),
    );
    fund_native(actor_id, 10);
    let before = asset_balance(&BOB, TestAsset::Native);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    Actors::on_idle(1, Weight::MAX);
    let run = Actors::actor_run_state(actor_id).expect("Running state persists");
    assert_eq!(run.cursor, 1);
    assert_eq!(run.opening_predicate_cursor, 1);
    assert_eq!(
      run.opening_predicate_results.as_slice(),
      [Ok(false), Ok(true)]
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    Actors::on_idle(2, Weight::MAX);
    assert_eq!(asset_balance(&BOB, TestAsset::Native), before + 5);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(
      Actors::active_actor_view(actor_id).map(|actor| actor.cycle_nonce),
      Some(1)
    );
  });
}

#[test]
fn signal_during_running_is_retained_for_a_later_cycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut steps = inert_contract_steps();
    steps
      .try_push(make_step(Task::StopCycle))
      .expect("second Step fits");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    Actors::on_idle(1, Weight::MAX);
    assert_eq!(
      Actors::actor_hot(actor_id).map(|hot| hot.cycle_state),
      Some(CycleState::Running)
    );
    assert!(!Actors::pending_signal(actor_id));

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(Actors::pending_signal(actor_id));
    Actors::on_idle(1, Weight::MAX);
    assert_eq!(
      Actors::actor_hot(actor_id).map(|hot| hot.cycle_state),
      Some(CycleState::Running),
      "the successor cannot execute in the commit block"
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    Actors::on_idle(2, Weight::MAX);
    let after_first_cycle = Actors::active_actor_view(actor_id).expect("Actor remains active");
    assert_eq!(after_first_cycle.cycle_nonce, 1);
    assert_eq!(after_first_cycle.cycle_state, CycleState::Idle);
    assert!(after_first_cycle.pending_signal);

    Actors::on_idle(2, Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id).map(|actor| actor.cycle_nonce),
      Some(1),
      "the retained signal cannot open another cycle in the completion block"
    );
    frame_system::Pallet::<Test>::set_block_number(3);
    Actors::on_initialize(3);
    Actors::on_idle(3, Weight::MAX);
    let next_cycle = Actors::active_actor_view(actor_id).expect("next Cycle remains active");
    assert_eq!(next_cycle.cycle_nonce, 1);
    assert_eq!(next_cycle.cycle_state, CycleState::Running);
    assert!(!next_cycle.pending_signal);
    assert_eq!(
      Actors::actor_run_state(actor_id).map(|run| run.cycle_nonce),
      Some(2)
    );
  });
}

#[test]
fn run_attempt_missing_state_fails_without_panicking_or_mutating() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let due = Actors::actor_run_state(actor_id)
      .expect("coherent Actor run exists")
      .eligible_at;
    frame_system::Pallet::<Test>::set_block_number(due);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(due, &mut meter);
    assert_eq!(
      Actors::paged_head_entry().map(|(_, entry)| entry.actor_id),
      Some(actor_id)
    );
    ActorRunStateStore::<Test>::remove(actor_id);
    let hot_before = Actors::actor_hot(actor_id).expect("hot state remains");
    System::reset_events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    Actors::execute_cycle_to_cutoff(Weight::MAX, Actors::queue_tail());

    assert_eq!(Actors::actor_hot(actor_id), Some(hot_before));
    assert!(System::events().is_empty());
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn actor_run_try_state_rejects_marker_and_cursor_drift() {
  new_test_ext().execute_with(|| {
    let mut plan = transfer_contract_steps(BOB, 1);
    plan[0].on_error = RETRY_LATER;
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    ActorRunStateStore::<Test>::insert(
      actor_id,
      RuntimeActorRunState {
        contract_authority: run_contract_authority(actor_id),
        cycle_nonce: 1,
        cursor: 0,
        opening_predicate_cursor: 0,
        unsuccessful_attempts_at_cursor: 1,
        last_attempt_block: 1,
        last_committed_step_block: None,
        eligible_at: 2,
        opening_snapshot: Default::default(),
        opening_predicate_results: Default::default(),
        funding_snapshot: Default::default(),
        cumulative_outcomes: Default::default(),
        last_step_outcome: Some(StepOutcome::FundingUnavailable),
        suspension: Some(SuspensionReason::FundingUnavailable),
      },
    );
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    let run = ActorRunStateStore::<Test>::take(actor_id).expect("orphan Run is retained");
    assert_ok!(Actors::write_run_state(actor_id, Some(run)));
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    ActorRunStateStore::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("Actor run exists").suspension = Some(SuspensionReason::Temporary);
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    ActorRunStateStore::<Test>::mutate(actor_id, |maybe| {
      let run_state = maybe.as_mut().expect("Actor run exists");
      run_state.suspension = Some(SuspensionReason::FundingUnavailable);
      run_state.eligible_at = 3;
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    ActorRunStateStore::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("Actor run exists").eligible_at = 2;
    });
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    ActorRunStateStore::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Actor run exists")
        .opening_snapshot
        .try_insert(OpeningSurface::PreservableAsset(TestAsset::Native), 10)
        .expect("snapshot entry fits");
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    ActorRunStateStore::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Actor run exists")
        .opening_snapshot
        .clear();
    });
    ActorRunStateStore::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("Actor run exists").cursor = 1;
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    ActorRunStateStore::<Test>::remove(actor_id);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn user_dormant_activation_installs_process_without_service_prefunding() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      None,
    ));
    let actor_id = Actors::next_actor_id() - 1;
    let sovereign = Actors::sovereign_account_id(&ALICE, 0);
    assert_eq!(native_balance(&sovereign), 0);
    frame_system::Pallet::<Test>::set_block_number(2);

    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1))
        .expect("direct Actor Contract"),
    ));

    assert_eq!(native_balance(&sovereign), 0);
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert_eq!(Actors::active_actor_count(), 1);
  });
}

#[test]
fn generated_run_weights_cover_distinct_storage_paths() {
  new_test_ext().execute_with(|| {
    let suspend = <TestWeightInfo as crate::WeightInfo>::run_suspend();
    assert_eq!(suspend, Weight::from_parts(28_668_868, 4_178));
    assert_eq!(
      <TestWeightInfo as crate::WeightInfo>::run_complete(),
      Weight::from_parts(18_019_000, 4_030)
    );
    assert_eq!(
      <TestWeightInfo as crate::WeightInfo>::run_cancel(),
      Weight::from_parts(56_782_000, 8_120)
    );
  });
}

#[test]
fn generated_predicate_weight_scales_with_bounded_step_visits() {
  use crate::WeightInfo;

  let zero = <Test as crate::Config>::WeightInfo::predicate_set_evaluation(0);
  let one = <Test as crate::Config>::WeightInfo::predicate_set_evaluation(1);
  let maximum = <Test as crate::Config>::WeightInfo::predicate_set_evaluation(4);
  assert_eq!(zero, Weight::zero());
  assert!(one.ref_time() > 0 && one.proof_size() > 0);
  assert!(maximum.ref_time() >= one.ref_time());
  assert!(maximum.proof_size() >= one.proof_size());
  assert!(maximum.ref_time() >= 41_068_000);
  assert!(maximum.proof_size() >= 9_045);
}

#[test]
fn canonical_instance_readiness_state_tracks_lifecycle_and_schedule() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    let initial = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(initial.actor_class.actor_type(), ActorType::User);
    assert!(matches!(initial.trigger, Trigger::Manual));
    assert_eq!(initial.lifecycle, ActiveLifecycle::Active);
    assert!(!initial.pending_signal);
    assert_eq!(initial.cycle_nonce, 0);
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .pending_signal
    );
    run_idle(Weight::MAX);
    let after_cycle = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(!after_cycle.pending_signal);
    assert_eq!(after_cycle.cycle_nonce, 1);
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .lifecycle,
      ActiveLifecycle::Paused
    );
    let timer_schedule = Schedule {
      trigger: Trigger::cadenced(3),
      cooldown_blocks: 0,
    };
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      timer_schedule,
      None,
    ));
    let after_update = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(after_update.cooldown_blocks, 0);
    assert!(matches!(
      after_update.trigger,
      Trigger::Cadenced { every_ticks: 3 }
    ));
  });
}

#[test]
fn authored_abort_preserves_earlier_provisional_task_commit() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      }),
      StepOf::<Test> {
        precondition: None,
        task: Task::SwapIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::Fixed(10),
          slippage_tolerance: Perbill::one(),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
    ])
    .expect("two steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    run_idle(Weight::MAX);

    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("failed actor remains below cutoff")
        .unsuccessful_attempt_streak,
      1
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        result: CycleResult::Failed,
        ..
      } if *id == actor_id
    )));
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_abort_cycle_removes_transaction_local_scalar_bridges() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let plan = contract_steps_with_step(StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::AbortCycle,
    });
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        result: CycleResult::Failed,
        ..
      } if *id == actor_id
    )));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(Actors::load_frame_control_authority(actor_id).is_some());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_abort_cycle_closes_at_failure_threshold_without_scalar_hot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let plan = contract_steps_with_step(StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::AbortCycle,
    });
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 100);
    let threshold = <Test as crate::Config>::MaxConsecutiveFailures::get();
    mutate_actor_hot_coherent(actor_id, |hot| {
      hot.unsuccessful_attempt_streak = threshold.saturating_sub(1);
    });
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    set_temporary_dex_failure(true);

    Actors::on_idle(1, Weight::MAX);

    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ConsecutiveFailures,
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_running_abort_cycle_failure_never_materializes_scalar_hot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      }),
      StepOf::<Test> {
        precondition: None,
        task: Task::SwapIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::Fixed(10),
          slippage_tolerance: Perbill::one(),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
      make_step(Task::StopCycle),
    ])
    .expect("three Steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    Actors::on_idle(1, Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("Running prefix")
        .cursor,
      1
    );
    set_temporary_dex_failure(true);
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    run_prepass();
    Actors::on_idle(2, Weight::MAX);

    assert!(Actors::actor_run_state(actor_id).is_none());
    let authority = Actors::load_frame_control_authority(actor_id).expect("Idle authority");
    assert_eq!(authority.2.cycle_state, CycleState::Idle);
    assert_eq!(authority.2.unsuccessful_attempt_streak, 1);
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Unsignaled)
    ));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        result: CycleResult::Failed,
        outcomes: OutcomeTotals { executed_steps: 1, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStopped { actor_id: id, .. } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn run_control_placement_failures_roll_back_exactly() {
  fn exhaust_ticket_namespace(actor_id: ActorId) {
    let (location, cell) = Actors::actor_control_cell(actor_id).expect("suspended primary");
    let crate::ActorControlLocation::Ready { ticket } = location else {
      panic!("retry fixture retains its Ready ticket");
    };
    assert_eq!(Actors::queue_occupancy(), 1);
    let old_page =
      crate::ActorReadyFrameChunks::<Test>::take(ticket / 32).expect("sole Ready page");
    assert_eq!(old_page.iter().flatten().count(), 1);
    let final_ticket = u64::MAX - 1;
    let mut final_page = old_page;
    *final_page
      .get_mut((ticket % 32) as usize)
      .expect("old slot") = None;
    *final_page
      .get_mut((final_ticket % 32) as usize)
      .expect("final slot") = Some(cell);
    crate::ActorReadyFrameChunks::<Test>::insert(final_ticket / 32, final_page);
    crate::ActorControlLocators::<Test>::insert(
      actor_id,
      crate::ActorControlLocation::Ready {
        ticket: final_ticket,
      },
    );
    crate::ActorReadyHead::<Test>::put(final_ticket);
    crate::ActorReadyTail::<Test>::put(u64::MAX);
    assert!(Actors::active_actor_view(actor_id).is_some());
  }

  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    exhaust_ticket_namespace(actor_id);
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();

    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::root(),
        actor_id,
        FundingSourcePolicy::AnyVerifiedIngress,
      ),
      Error::<Test>::QueueTicketExhausted
    );
    assert!(Actors::actor_run_state(actor_id).is_some());
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });

  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    exhaust_ticket_namespace(actor_id);
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();

    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::root(),
        actor_id,
        inert_contract_steps(),
        crate::CompletionPolicy::Persistent,
      ),
      Error::<Test>::QueueTicketExhausted
    );
    assert!(Actors::actor_run_state(actor_id).is_some());
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });

  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    exhaust_ticket_namespace(actor_id);
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();

    assert_noop!(
      Actors::cancel_run(RuntimeOrigin::root(), actor_id),
      Error::<Test>::QueueTicketExhausted
    );
    assert!(Actors::actor_run_state(actor_id).is_some());
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn manual_trigger_clears_when_cycle_starts() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .pending_signal
    );
    run_idle(Weight::MAX);
    let inst = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(!inst.pending_signal);
    assert_eq!(inst.cycle_nonce, 1);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleStarted {
          actor_id: id,
          cycle_nonce: 1,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn completed_failed_and_suspended_attempts_update_failure_streak_once() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    mutate_actor_hot_coherent(actor_id, |hot| hot.unsuccessful_attempt_streak = 1);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("completed actor remains")
        .unsuccessful_attempt_streak,
      0
    );
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let plan = contract_steps_with_step(StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::AbortCycle,
    });
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("failed actor remains below cutoff")
        .unsuccessful_attempt_streak,
      1
    );
  });

  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("suspended actor remains")
        .unsuccessful_attempt_streak,
      1
    );
  });
}

#[test]
fn unsuccessful_attempt_streak_closes_actor_at_inclusive_threshold() {
  new_test_ext().execute_with(|| {
    let threshold = <Test as crate::Config>::MaxConsecutiveFailures::get();
    frame_system::Pallet::<Test>::set_block_number(1);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(failing_step),
    );
    fund_native(actor_id, 100);
    for cycle in 1..=threshold {
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      run_idle(Weight::MAX);
      if cycle < threshold {
        let inst = Actors::active_actor_view(actor_id).expect("actor remains before threshold");
        assert_eq!(inst.unsuccessful_attempt_streak, cycle);
        frame_system::Pallet::<Test>::set_block_number((cycle + 1) as u64);
      }
    }
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::ConsecutiveFailures,
        } if *id == actor_id
      )
    }));
    let events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    let cycle_summary = events
      .iter()
      .position(|event| {
        matches!(
          event,
          Event::CycleSummary {
            actor_id: id,
            cycle_nonce,
            ..
          } if *id == actor_id && *cycle_nonce == u64::from(threshold)
        )
      })
      .expect("terminal cycle summary exists");
    let closed = events
      .iter()
      .position(|event| {
        matches!(
          event,
          Event::ActorClosed {
            actor_id: id,
            reason: CloseReason::ConsecutiveFailures,
          } if *id == actor_id
        )
      })
      .expect("terminal close exists");
    assert!(
      cycle_summary < closed,
      "the admitted cycle must summarize before the terminal event"
    );
  });
}

#[test]
fn system_immutable_actor_closes_internally_at_failure_threshold_without_tasks() {
  new_test_ext().execute_with(|| {
    let threshold = <Test as crate::Config>::MaxConsecutiveFailures::get();
    frame_system::Pallet::<Test>::set_block_number(1);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Immutable,
      system_active_contract(
        timer_schedule(1),
        None,
        contract_steps_with_step(failing_step)
      ),
    ));
    assert_noop!(
      Actors::close_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ImmutableActor
    );
    fund_native(actor_id, 100);
    let charlie_before = native_balance(&CHARLIE);
    for cycle in 1..=threshold {
      frame_system::Pallet::<Test>::set_block_number(u64::from(cycle) + 1);
      run_idle(Weight::MAX);
      if cycle < threshold {
        assert_eq!(
          Actors::active_actor_view(actor_id)
            .expect("actor remains before threshold")
            .unsuccessful_attempt_streak,
          cycle
        );
      }
    }
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(
      native_balance(&CHARLIE),
      charlie_before,
      "terminal cleanup must not execute hidden transfer work"
    );
    assert_eq!(
      Actors::system_sovereigns(actor_id),
      Some(SystemSovereignState::Vacant)
    );
    let fresh_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor_at_sovereign_id(
      RuntimeOrigin::root(),
      actor_id,
      ALICE,
      Mutability::Mutable,
      system_active_contract(timer_schedule(1), None, transfer_contract_steps(BOB, 1)),
    ));
    assert!(Actors::active_actor_view(fresh_id).is_some());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::ConsecutiveFailures,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn execute_cycle_respects_max_executions_per_block() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_exec: u32 = <Test as crate::Config>::MaxExecutionsPerBlock::get();
    let total = max_exec + 2;
    let mut ids = Vec::new();
    for _ in 0..total {
      let id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      ids.push(id);
      assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
    }
    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let started_block_2 = frame_system::Pallet::<Test>::events()
      .iter()
      .filter(|record| {
        matches!(
          record.event,
          RuntimeEvent::Actors(Event::CycleStarted { .. })
        )
      })
      .count() as u32;
    assert_eq!(started_block_2, max_exec);
    frame_system::Pallet::<Test>::set_block_number(3);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let started_block_3 = frame_system::Pallet::<Test>::events()
      .iter()
      .filter(|record| {
        matches!(
          record.event,
          RuntimeEvent::Actors(Event::CycleStarted { .. })
        )
      })
      .count() as u32;
    assert_eq!(started_block_3, total - max_exec);
  });
}

#[test]
fn retry_later_is_mutable_only_at_creation_and_update() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut retry_plan = transfer_contract_steps(BOB, 1);
    retry_plan[0].on_error = RETRY_LATER;

    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Immutable,
        user_active_contract(manual_schedule(), None, retry_plan.clone()),
      ),
      Error::<Test>::RetryLaterNotAllowedForImmutableActor
    );
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Immutable,
        system_active_contract(manual_schedule(), None, retry_plan.clone()),
      ),
      Error::<Test>::RetryLaterNotAllowedForImmutableActor
    );

    prefund_active_user_creation(ALICE, &retry_plan);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, retry_plan.clone()),
    ));

    let immutable_id = create_user_with(
      BOB,
      Mutability::Immutable,
      manual_schedule(),
      None,
      transfer_contract_steps(ALICE, 1),
    );
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::signed(BOB),
        immutable_id,
        retry_plan,
        crate::CompletionPolicy::Persistent,
      ),
      Error::<Test>::RetryLaterNotAllowedForImmutableActor
    );
  });
}

#[test]
fn retry_later_enforces_the_protocol_fixed_attempt_range() {
  new_test_ext().execute_with(|| {
    for max_attempts in [0, 1, 11] {
      let mut plan = transfer_contract_steps(BOB, 1);
      plan[0].on_error = StepErrorPolicy::RetryLater { max_attempts };
      assert_noop!(
        Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          Mutability::Mutable,
          user_active_contract(manual_schedule(), None, plan),
        ),
        Error::<Test>::InvalidRetryAttemptLimit
      );
    }
    for max_attempts in [2, 10] {
      let mut valid_plan = transfer_contract_steps(BOB, 1);
      valid_plan[0].on_error = StepErrorPolicy::RetryLater { max_attempts };
      prefund_active_user_creation(ALICE, &valid_plan);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, valid_plan),
      ));
    }
  });
}

#[test]
fn retry_later_aborts_permanent_failure_without_executing_suffix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: RETRY_LATER,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    });
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![failing_step, succeeding_step]).expect("two steps fit"),
    );
    fund_native(actor_id, 100);
    let charlie_before = native_balance(&CHARLIE);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    let instance = Actors::active_actor_view(actor_id).expect("actor remains active");
    assert_eq!(instance.cycle_nonce, 1);
    assert_eq!(instance.unsuccessful_attempt_streak, 1);
    assert_eq!(instance.cycle_state, CycleState::Idle);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          outcomes: OutcomeTotals { executed_steps: 0, failed_steps: 1, .. },
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_running_swap_retry_and_resume_never_materialize_scalar_hot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_pool(TestAsset::Native, TestAsset::Local(77), 10_000, 10_000);
    set_asset_balance(&u64::MAX, TestAsset::Local(77), 10_000);
    let steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      }),
      StepOf::<Test> {
        precondition: None,
        task: Task::SwapIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::Fixed(20),
          slippage_tolerance: Perbill::one(),
        },
        on_error: RETRY_LATER,
      },
      make_step(Task::StopCycle),
    ])
    .expect("three Steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    Actors::on_idle(1, Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("Running prefix")
        .cursor,
      1
    );
    set_temporary_dex_failure(true);
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    run_prepass();
    Actors::on_idle(2, Weight::MAX);
    let retry = Actors::actor_run_state(actor_id).expect("Running failure suspends");
    assert_eq!(retry.cursor, 1);
    assert_eq!(retry.eligible_at, 3);

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(3);
    Actors::on_initialize(3);
    run_prepass();
    Actors::on_idle(3, Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("resumed continuation")
        .cursor,
      2
    );
    frame_system::Pallet::<Test>::set_block_number(4);
    Actors::on_initialize(4);
    run_prepass();
    Actors::on_idle(4, Weight::MAX);

    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Unsignaled)
    ));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleStopped {
        actor_id: id,
        cycle_nonce: 1,
        step_index: 2,
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_opening_continue_next_step_failure_never_materializes_scalar_hot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_pool(TestAsset::Native, TestAsset::Local(77), 10_000, 10_000);
    set_asset_balance(&u64::MAX, TestAsset::Local(77), 10_000);
    let steps = BoundedVec::try_from(vec![
      StepOf::<Test> {
        precondition: None,
        task: Task::SwapIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::Fixed(20),
          slippage_tolerance: Perbill::one(),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      make_step(Task::StopCycle),
    ])
    .expect("two Steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    set_temporary_dex_failure(true);

    Actors::on_idle(1, Weight::MAX);
    let continuation = Actors::actor_run_state(actor_id).expect("failure advances cursor");
    assert_eq!(continuation.cursor, 1);
    assert!(continuation.suspension.is_none());
    assert_eq!(continuation.cumulative_outcomes.failed_steps, 1);

    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    run_prepass();
    Actors::on_idle(2, Weight::MAX);

    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Unsignaled)
    ));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_stake_continue_next_step_rolls_back_without_scalar_hot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Native;
    let steps = BoundedVec::try_from(vec![
      StepOf::<Test> {
        precondition: None,
        task: Task::Stake {
          asset,
          amount: AmountResolution::Fixed(40),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      make_step(Task::StopCycle),
    ])
    .expect("two Steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 120);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    set_fail_staking_after_burn(true);

    Actors::on_idle(1, Weight::MAX);
    let continuation = Actors::actor_run_state(actor_id).expect("failure advances cursor");
    assert_eq!(continuation.cursor, 1);
    assert_eq!(continuation.cumulative_outcomes.failed_steps, 1);
    assert_eq!(native_balance(&actor), 120);
    assert_eq!(staked_balance(actor, asset), 0);

    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    run_prepass();
    Actors::on_idle(2, Weight::MAX);

    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::StakeExecuted { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_stake_retry_later_permanent_failure_terminates_without_scalar_hot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Native;
    let plan = contract_steps_with_step(StepOf::<Test> {
      precondition: None,
      task: Task::Stake {
        asset,
        amount: AmountResolution::Fixed(40),
      },
      on_error: RETRY_LATER,
    });
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 120);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    set_fail_staking_after_burn(true);

    Actors::on_idle(1, Weight::MAX);

    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(native_balance(&actor), 120);
    assert_eq!(staked_balance(actor, asset), 0);
    let authority = Actors::load_frame_control_authority(actor_id).expect("Idle authority");
    assert_eq!(authority.2.cycle_state, CycleState::Idle);
    assert_eq!(authority.2.unsuccessful_attempt_streak, 1);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        result: CycleResult::Failed,
        outcomes: OutcomeTotals { failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_retry_attempt_exhaustion_closes_from_consumed_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let plan = contract_steps_with_step(StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 2 },
    });
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    set_temporary_dex_failure(true);

    Actors::on_idle(1, Weight::MAX);
    assert!(matches!(
      Actors::active_actor_view(actor_id).map(|instance| instance.cycle_state),
      Some(CycleState::Suspended)
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    run_prepass();
    Actors::on_idle(2, Weight::MAX);

    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::RetryAttemptsExhausted,
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_running_continue_next_step_failure_never_materializes_scalar_hot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_pool(TestAsset::Native, TestAsset::Local(77), 10_000, 10_000);
    set_asset_balance(&u64::MAX, TestAsset::Local(77), 10_000);
    let steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      }),
      StepOf::<Test> {
        precondition: None,
        task: Task::SwapIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::Fixed(20),
          slippage_tolerance: Perbill::one(),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      make_step(Task::StopCycle),
    ])
    .expect("three Steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    Actors::on_idle(1, Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("Running prefix")
        .cursor,
      1
    );
    set_temporary_dex_failure(true);
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    run_prepass();
    Actors::on_idle(2, Weight::MAX);
    let continuation = Actors::actor_run_state(actor_id).expect("failure advances cursor");
    assert_eq!(continuation.cursor, 2);
    assert!(continuation.suspension.is_none());
    assert_eq!(continuation.cumulative_outcomes.failed_steps, 1);

    frame_system::Pallet::<Test>::set_block_number(3);
    Actors::on_initialize(3);
    run_prepass();
    Actors::on_idle(3, Weight::MAX);

    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Unsignaled)
    ));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn retry_later_resumes_same_cursor_without_replaying_committed_prefix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_pool(TestAsset::Native, TestAsset::Local(77), 10_000, 10_000);
    set_asset_balance(&u64::MAX, TestAsset::Local(77), 10_000);
    let retry_step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(20),
        slippage_tolerance: Perbill::one(),
      },
      on_error: RETRY_LATER,
    };
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      }),
      retry_step,
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(5),
      }),
    ])
    .expect("three steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    let first = Actors::active_actor_view(actor_id).expect("suspended actor remains");
    let first_continuation = Actors::actor_run_state(actor_id).expect("Actor run exists");
    assert_eq!(first.cycle_state, CycleState::Suspended);
    assert_eq!(first.cycle_nonce, 0);
    assert_eq!(first.unsuccessful_attempt_streak, 1);
    assert_eq!(first_continuation.cycle_nonce, 1);
    assert_eq!(first_continuation.cursor, 1);
    assert_eq!(first_continuation.unsuccessful_attempts_at_cursor, 1);
    assert_eq!(first_continuation.last_attempt_block, 2);
    assert_eq!(first_continuation.eligible_at, 3);
    assert!(matches!(
      first_continuation.last_step_outcome,
      Some(StepOutcome::Failed(TaskFailure {
        retry: RetryClass::Temporary,
        ..
      }))
    ));
    assert_eq!(
      first_continuation.suspension,
      Some(SuspensionReason::Temporary)
    );
    assert_eq!(first_continuation.cumulative_outcomes.executed_steps, 1);
    assert_eq!(first_continuation.cumulative_outcomes.failed_steps, 1);
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert_eq!(native_balance(&first.sovereign_account), 90);
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    assert!(!has_actor_event(|event| {
      matches!(event, Event::CycleSummary { actor_id: id, .. } if *id == actor_id)
    }));

    frame_system::Pallet::<Test>::set_block_number(first_continuation.eligible_at);
    run_idle(Weight::MAX);
    run_next_idle(Weight::MAX);
    let second_continuation = Actors::actor_run_state(actor_id).expect("Actor run remains");
    assert_eq!(second_continuation.cycle_nonce, 1);
    assert_eq!(second_continuation.cursor, 1);
    assert_eq!(second_continuation.unsuccessful_attempts_at_cursor, 2);
    assert_eq!(second_continuation.last_attempt_block, 4);
    assert_eq!(second_continuation.eligible_at, 6);
    assert_eq!(second_continuation.cumulative_outcomes.executed_steps, 1);
    assert_eq!(second_continuation.cumulative_outcomes.failed_steps, 2);
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(native_balance(&CHARLIE), charlie_before);

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(second_continuation.eligible_at);
    Actors::on_initialize(second_continuation.eligible_at);
    run_prepass();
    run_idle(Weight::MAX);
    run_next_idle(Weight::MAX);

    let completed = Actors::active_actor_view(actor_id).expect("completed actor remains");
    assert_eq!(completed.cycle_state, CycleState::Idle);
    assert_eq!(completed.cycle_nonce, 1);
    assert_eq!(completed.unsuccessful_attempt_streak, 0);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 5);
    let starts = frame_system::Pallet::<Test>::events()
      .iter()
      .filter(|record| {
        matches!(
          record.event,
          RuntimeEvent::Actors(Event::CycleStarted { actor_id: id, .. }) if id == actor_id
        )
      })
      .count();
    assert_eq!(starts, 1);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          cycle_nonce: 1,
          outcomes: OutcomeTotals { executed_steps: 3, failed_steps: 2, .. },
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn stop_cycle_commits_prefix_and_completes_before_unreachable_suffix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      }),
      StepOf::<Test> {
        precondition: None,
        task: Task::StopCycle,
        on_error: RETRY_LATER,
      },
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(5),
      }),
    ])
    .expect("three steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    let actor = Actors::active_actor_view(actor_id).expect("actor remains active");
    assert_eq!(actor.cycle_state, CycleState::Idle);
    assert_eq!(actor.cycle_nonce, 1);
    assert_eq!(actor.unsuccessful_attempt_streak, 0);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleStopped {
        actor_id: id,
        cycle_nonce: 1,
        step_index: 1,
      } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 2, failed_steps: 0, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn skipped_stop_cycle_advances_to_the_suffix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let stop = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 1_000,
      }]),
      task: Task::StopCycle,
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = BoundedVec::try_from(vec![
      stop,
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(5),
      }),
    ])
    .expect("two steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(native_balance(&BOB), bob_before + 5);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStopped { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 1, precondition_skips: 1, failed_steps: 0, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn close_after_productive_cycle_ignores_false_cycles_then_closes_immutable_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 100,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Immutable,
      system_active_contract_with_completion(
        timer_schedule(1),
        None,
        contract_steps_with_step(step),
        crate::CompletionPolicy::CloseAfterProductiveCycle,
      ),
    ));
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 50);
    let bob_before = native_balance(&BOB);

    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);

    let retained = Actors::active_actor_view(actor_id).expect("false cycle remains active");
    assert_eq!(retained.cycle_nonce, 1);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&actor), 50);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { committed_effectful_tasks: 0, .. },
        ..
      } if *id == actor_id
    )));

    fund_native(actor_id, 100);
    frame_system::Pallet::<Test>::set_block_number(3);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(native_balance(&actor), 140);
    let events: Vec<_> = System::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    let summary = events
      .iter()
      .position(|event| matches!(event, Event::CycleSummary { actor_id: id, outcomes: OutcomeTotals { committed_effectful_tasks: 1, .. }, .. } if *id == actor_id))
      .expect("productive summary");
    let closed = events
      .iter()
      .position(|event| matches!(event, Event::ActorClosed { actor_id: id, reason: CloseReason::ProductiveCycleCompleted } if *id == actor_id))
      .expect("productive close");
    assert!(summary < closed);
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_productive_completion_closes_without_scalar_hot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract_with_completion(
        manual_schedule(),
        None,
        contract_steps_with_step(make_step(Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(10),
        })),
        crate::CompletionPolicy::CloseAfterProductiveCycle,
      ),
    ));
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));

    Actors::on_idle(1, Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ProductiveCycleCompleted,
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_effectful_auto_close_uses_terminal_nonce_without_scalar_hot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      })),
    );
    fund_native(actor_id, 100);
    assert_ok!(replace_auto_close(RuntimeOrigin::root(), actor_id, Some(1),));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));

    Actors::on_idle(1, Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::AutoCloseNonceReached,
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn close_after_productive_cycle_does_not_treat_stop_cycle_as_effectful() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract_with_completion(
        manual_schedule(),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
        crate::CompletionPolicy::CloseAfterProductiveCycle,
      ),
    ));

    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_some());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 1, committed_effectful_tasks: 0, .. },
        ..
      } if *id == actor_id
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ProductiveCycleCompleted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn close_after_productive_cycle_waits_for_retry_completion() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract_with_completion(
        manual_schedule(),
        None,
        temporary_retry_swap_plan(),
        crate::CompletionPolicy::CloseAfterProductiveCycle,
      ),
    ));
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);

    assert!(Actors::actor_run_state(actor_id).is_some());
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ProductiveCycleCompleted,
      } if *id == actor_id
    )));

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);

    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ProductiveCycleCompleted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn close_after_productive_cycle_keeps_retry_exhaustion_as_failure_terminal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let retry_step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(20),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 2 },
    };
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract_with_completion(
        manual_schedule(),
        None,
        contract_steps_with_step(retry_step),
        crate::CompletionPolicy::CloseAfterProductiveCycle,
      ),
    ));
    fund_native(actor_id, 100);
    let actor = sovereign_account(actor_id);
    let balance_before = native_balance(&actor);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_some());
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(native_balance(&actor), balance_before);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::RetryAttemptsExhausted,
      } if *id == actor_id
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ProductiveCycleCompleted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn stop_cycle_runs_normal_auto_close_after_the_summary() {
  for step_count in [1usize, 5, 9, 12] {
    new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![make_step(Task::StopCycle); step_count])
        .expect("Contract fits the twelve-Step host bound"),
    );
    let tail_count = (step_count - 1).div_ceil(4);
    assert_eq!(crate::ActorContractTailChunks::<Test>::iter_prefix(actor_id).count(), tail_count);
    assert_ok!(replace_auto_close(
      RuntimeOrigin::root(),
      actor_id,
      Some(1),
    ));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!crate::ActorContractHeads::<Test>::contains_key(actor_id));
    assert_eq!(crate::ActorContractTailChunks::<Test>::iter_prefix(actor_id).count(), 0);
    let events: Vec<_> = System::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    let stopped = events
      .iter()
      .position(|event| matches!(event, Event::CycleStopped { actor_id: id, .. } if *id == actor_id))
      .expect("stop event");
    let summary = events
      .iter()
      .position(|event| matches!(event, Event::CycleSummary { actor_id: id, .. } if *id == actor_id))
      .expect("summary event");
    let closed = events
      .iter()
      .position(|event| matches!(event, Event::ActorClosed { actor_id: id, reason: CloseReason::AutoCloseNonceReached } if *id == actor_id))
      .expect("auto-close event");
    assert!(stopped < summary && summary < closed);
    assert_eq!(
      last_step_control_execution().map(|execution| execution.outcome),
      Some(crate::StepControlOutcome::Completed),
      "auto-close retains completed control ownership while cleanup is metered separately",
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn continuation_can_complete_at_stop_cycle_without_replaying_prefix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_pool(TestAsset::Native, TestAsset::Local(77), 10_000, 10_000);
    set_asset_balance(&u64::MAX, TestAsset::Local(77), 10_000);
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      }),
      StepOf::<Test> {
        precondition: None,
        task: Task::SwapIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::Fixed(20),
          slippage_tolerance: Perbill::one(),
        },
        on_error: RETRY_LATER,
      },
      make_step(Task::StopCycle),
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(5),
      }),
    ])
    .expect("four steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id).expect("suspended").cursor,
      1
    );
    assert_eq!(native_balance(&BOB), bob_before + 10);

    set_temporary_dex_failure(false);
    let eligible_at = Actors::actor_run_state(actor_id)
      .expect("suspension remains")
      .eligible_at;
    frame_system::Pallet::<Test>::set_block_number(eligible_at);
    Actors::on_initialize(eligible_at);
    run_prepass();
    Actors::on_idle(eligible_at, Weight::MAX);
    #[cfg(not(feature = "runtime-benchmarks"))]
    {
      let locator = crate::ActorControlLocators::<Test>::get(actor_id);
      assert!(
        matches!(locator, Some(crate::ActorControlLocation::Ready { .. })),
        "successor locator: {locator:?}"
      );
    }
    run_next_idle(Weight::MAX);

    let crate::LoadedActorStateOf::Active(state) =
      Actors::load_actor_state_for_frame_control(actor_id)
    else {
      panic!("frame-owned actor remains active");
    };
    let actor = Actors::derive_active_actor_view(state.identity, state.hot, state.contract);
    #[cfg(not(feature = "runtime-benchmarks"))]
    assert_eq!(actor.cycle_state, CycleState::Idle);
    assert_eq!(actor.unsuccessful_attempt_streak, 0);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleStopped {
        actor_id: id,
        cycle_nonce: 1,
        step_index: 2,
      } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 3, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn temporary_failure_keeps_continue_next_step_semantics() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_pool(TestAsset::Native, TestAsset::Local(77), 10_000, 10_000);
    set_asset_balance(&u64::MAX, TestAsset::Local(77), 10_000);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![
        failing_step,
        make_step(Task::Transfer {
          to: CHARLIE,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(1),
        }),
      ])
      .expect("two steps fit"),
    );
    fund_native(actor_id, 100);
    let charlie_before = native_balance(&CHARLIE);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    let completed = Actors::active_actor_view(actor_id).expect("actor remains");
    assert_eq!(completed.cycle_state, CycleState::Idle);
    assert_eq!(completed.unsuccessful_attempt_streak, 0);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(native_balance(&CHARLIE), charlie_before + 1);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          outcomes: OutcomeTotals { executed_steps: 1, failed_steps: 1, .. },
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn retry_later_local_attempt_cutoff_closes_without_prefix_replay() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_max_consecutive_failures(10);
    setup_pool(TestAsset::Native, TestAsset::Local(77), 10_000, 10_000);
    set_asset_balance(&u64::MAX, TestAsset::Local(77), 10_000);
    let retry_step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![
        make_step(Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(1),
        }),
        retry_step,
      ])
      .expect("two steps fit"),
    );
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("first unsuccessful attempt persists")
        .unsuccessful_attempts_at_cursor,
      1
    );

    let first_eligible_at = Actors::actor_run_state(actor_id)
      .expect("first unsuccessful attempt persists")
      .eligible_at;
    frame_system::Pallet::<Test>::set_block_number(first_eligible_at);
    run_idle(Weight::MAX);
    run_next_idle(Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("second unsuccessful attempt persists")
        .unsuccessful_attempts_at_cursor,
      2
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(
      Actors::active_actor_view(actor_id)
        .expect("suspended actor remains")
        .pending_signal
    );

    let second_eligible_at = Actors::actor_run_state(actor_id)
      .expect("second unsuccessful attempt persists")
      .eligible_at;
    frame_system::Pallet::<Test>::set_block_number(second_eligible_at);
    Actors::on_initialize(second_eligible_at);
    run_prepass();
    run_idle(Weight::MAX);
    run_next_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before + 1);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::RetryAttemptsExhausted,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn retry_later_resets_local_attempt_count_after_cursor_advancement() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_max_consecutive_failures(10);
    setup_temporary_retry_pool();
    let plan = BoundedVec::try_from(vec![
      StepOf::<Test> {
        precondition: None,
        task: Task::SwapIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::Fixed(10),
          slippage_tolerance: Perbill::one(),
        },
        on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
      },
      StepOf::<Test> {
        precondition: None,
        task: Task::AddLiquidity {
          asset_a: TestAsset::Local(77),
          asset_b: TestAsset::Local(88),
          amount_a: AmountResolution::Fixed(1),
          amount_b: AmountResolution::Fixed(1),
          min_lp_out: 1,
        },
        on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
      },
    ])
    .expect("two retry steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 100);
    set_asset_balance(&actor, TestAsset::Local(88), 10);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let first = Actors::actor_run_state(actor_id).expect("first cursor suspends");
    assert_eq!(
      (first.cursor, first.unsuccessful_attempts_at_cursor),
      (0, 1)
    );

    set_temporary_dex_failure(false);
    set_temporary_add_liquidity_failure(true);
    frame_system::Pallet::<Test>::set_block_number(first.eligible_at);
    run_idle(Weight::MAX);
    let advanced = Actors::actor_run_state(actor_id).expect("later cursor suspends");
    assert_eq!(advanced.cursor, 1);
    assert_eq!(advanced.unsuccessful_attempts_at_cursor, 1);
  });
}

#[test]
fn global_failure_limit_can_close_before_retry_later_local_limit() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_max_consecutive_failures(1);
    setup_temporary_retry_pool();
    let plan = contract_steps_with_step(StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 2 },
    });
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleCancelled { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary { actor_id: id, result: CycleResult::Failed, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ConsecutiveFailures,
      } if *id == actor_id
    )));
    assert_eq!(
      last_step_control_execution().map(|execution| execution.outcome),
      Some(crate::StepControlOutcome::Failed),
      "failure close retains failed control ownership while cleanup is metered separately",
    );
  });
}

#[test]
fn reached_global_failure_cutoff_closes_before_another_attempt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, temporary_retry_swap_plan());
    fund_native(actor_id, 100);
    let threshold = <Test as crate::Config>::MaxConsecutiveFailures::get();
    mutate_actor_hot_coherent(actor_id, |hot| {
      hot.unsuccessful_attempt_streak = threshold;
    });
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed { actor_id: id, reason: CloseReason::ConsecutiveFailures }
        if *id == actor_id
    )));
  });
}

#[test]
fn retry_local_limit_precedes_simultaneous_global_failure_limit() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_max_consecutive_failures(2);
    setup_temporary_retry_pool();
    let plan = contract_steps_with_step(StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 2 },
    });
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    let run = Actors::actor_run_state(actor_id).expect("first admitted failure suspends");
    assert_eq!(run.unsuccessful_attempts_at_cursor, 1);
    frame_system::Pallet::<Test>::set_block_number(run.eligible_at);
    Actors::on_initialize(run.eligible_at);
    run_prepass();
    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::RetryAttemptsExhausted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn temporary_retry_backoff_is_one_two_four_eight_then_capped() {
  assert_eq!(Actors::retry_backoff_blocks(0), 1);
  assert_eq!(Actors::retry_backoff_blocks(1), 2);
  assert_eq!(Actors::retry_backoff_blocks(2), 4);
  assert_eq!(Actors::retry_backoff_blocks(3), 8);
  assert_eq!(Actors::retry_backoff_blocks(31), 8);
  assert_eq!(Actors::retry_backoff_blocks(u32::MAX), 8);

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    set_max_consecutive_failures(10);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, temporary_retry_swap_plan());
    fund_native(actor_id, 100);
    let actor = sovereign_account(actor_id);
    let balance_before = native_balance(&actor);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let initial_hot = Actors::actor_hot(actor_id).expect("suspended actor stays schedulable");
    assert!(initial_hot.queue_ticket.is_some());
    assert!(initial_hot.wakeup_pointer.is_none());
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("attempt zero")
        .cursor,
      0
    );
    assert_eq!(native_balance(&actor), balance_before);

    for (due, expected_attempt, next_due) in [(2, 1, 4), (4, 2, 8), (8, 3, 16), (16, 4, 24)] {
      frame_system::Pallet::<Test>::set_block_number(due - 1);
      run_idle(Weight::MAX);
      assert_eq!(
        Actors::actor_run_state(actor_id)
          .expect("not eligible before due block")
          .unsuccessful_attempts_at_cursor,
        expected_attempt
      );
      frame_system::Pallet::<Test>::set_block_number(due);
      run_idle(Weight::MAX);
      let continuation = Actors::actor_run_state(actor_id).expect("temporary failure resuspends");
      assert_eq!(
        continuation.unsuccessful_attempts_at_cursor,
        expected_attempt + 1
      );
      assert_eq!(continuation.cursor, 0);
      assert_eq!(scheduled_wakeup_block(actor_id), Some(next_due));
      assert_eq!(native_balance(&actor), balance_before);
    }

    frame_system::Pallet::<Test>::set_block_number(23);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("capped delay holds")
        .unsuccessful_attempts_at_cursor,
      5
    );
    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(24);
    run_idle(Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(native_balance(&actor), balance_before - 10);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor completed")
        .cycle_state,
      CycleState::Idle
    );
  });
}

#[test]
fn run_weight_deferral_does_not_admit_attempt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      temporary_retry_swap_plan(),
    );
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    run_idle(Weight::MAX);
    let before = Actors::actor_run_state(actor_id).expect("suspended");
    let ticket_before = Actors::actor_hot(actor_id)
      .expect("suspended actor")
      .queue_ticket;

    frame_system::Pallet::<Test>::set_block_number(2);
    let queue_weight = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1)
      .saturating_add(Actors::scheduler_actor_probe_weight_upper())
      .saturating_add(
        <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
          .max(<<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page()),
      );
    let proof_limit = queue_weight
      .proof_size()
      .saturating_add(current_step_execution_weight(actor_id).proof_size())
      .saturating_sub(1);
    Actors::execute_cycle(Weight::from_parts(u64::MAX, proof_limit));

    let after = Actors::actor_run_state(actor_id).expect("deferral preserves Actor run");
    assert_eq!(
      after.unsuccessful_attempts_at_cursor,
      before.unsuccessful_attempts_at_cursor
    );
    assert_eq!(after.last_attempt_block, before.last_attempt_block);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor remains")
        .unsuccessful_attempt_streak,
      1
    );
    assert_eq!(
      Actors::actor_hot(actor_id)
        .expect("suspended actor")
        .queue_ticket,
      ticket_before
    );
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleContinued { actor_id: id, .. } | Event::CycleSummary { actor_id: id, .. }
        if *id == actor_id
    )));
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_exact_next_retry_completes_without_scalar_rehydration() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 1,
      },
      None,
      temporary_retry_swap_plan(),
    );
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    Actors::on_idle(1, Weight::MAX);
    let suspended = Actors::actor_run_state(actor_id).expect("suspended Run survives");
    assert_eq!(suspended.cursor, 0);
    assert_eq!(suspended.eligible_at, 2);
    assert_eq!(canonical_scheduled_wakeup_block(actor_id), None);
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Ready { .. })
    ));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    run_prepass();
    Actors::on_idle(2, Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Unsignaled)
    ));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        result: CycleResult::Completed,
        ..
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_user_retry_preserves_installed_hold_without_scalar_rehydration() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 1,
      },
      None,
      temporary_retry_swap_plan(),
    );
    fund_native(actor_id, 1_000_000_000_000_000_000);
    let installed_hold = crate::ActorStateHolds::<Test>::get(actor_id)
      .expect("active User owns its installed-lifetime hold");
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    Actors::on_idle(1, Weight::MAX);
    let suspended = Actors::actor_run_state(actor_id).expect("User retry suspends");
    assert_eq!(suspended.eligible_at, 2);
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Ready { .. })
    ));
    assert_eq!(
      crate::ActorStateHolds::<Test>::get(actor_id),
      Some(installed_hold.clone())
    );
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    run_prepass();
    Actors::on_idle(2, Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Unsignaled)
    ));
    assert_eq!(
      crate::ActorStateHolds::<Test>::get(actor_id),
      Some(installed_hold)
    );
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_user_later_retry_preserves_hold_through_block_waiting() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 1,
      },
      None,
      temporary_retry_swap_plan(),
    );
    fund_native(actor_id, 1_000_000_000_000_000_000);
    let installed_hold = crate::ActorStateHolds::<Test>::get(actor_id)
      .expect("active User owns its installed-lifetime hold");
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    Actors::on_idle(1, Weight::MAX);
    let first_retry = Actors::actor_run_state(actor_id).expect("first retry survives");
    assert_eq!(first_retry.unsuccessful_attempts_at_cursor, 1);
    assert_eq!(first_retry.eligible_at, 2);
    assert_eq!(canonical_scheduled_wakeup_block(actor_id), None);

    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    run_prepass();
    Actors::on_idle(2, Weight::MAX);
    let second_retry = Actors::actor_run_state(actor_id).expect("second retry survives");
    assert_eq!(second_retry.unsuccessful_attempts_at_cursor, 2);
    assert_eq!(second_retry.eligible_at, 4);
    assert_eq!(canonical_scheduled_wakeup_block(actor_id), Some(4));
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Waiting {
        key: WakeupKey::Block(4),
        ..
      })
    ));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert_eq!(
      crate::ActorStateHolds::<Test>::get(actor_id),
      Some(installed_hold.clone())
    );

    frame_system::Pallet::<Test>::set_block_number(4);
    Actors::on_initialize(4);
    run_prepass();
    Actors::on_idle(4, Weight::MAX);
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Ready { .. })
    ));
    assert_eq!(canonical_scheduled_wakeup_block(actor_id), None);
    assert!(Actors::actor_run_state(actor_id).is_some());
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert_eq!(
      crate::ActorStateHolds::<Test>::get(actor_id),
      Some(installed_hold.clone())
    );
    assert!(!crate::WakeupWorkerFaultState::<Test>::exists());

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(5);
    Actors::on_initialize(5);
    run_prepass();
    Actors::on_idle(5, Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Unsignaled)
    ));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert_eq!(
      crate::ActorStateHolds::<Test>::get(actor_id),
      Some(installed_hold)
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        result: CycleResult::Completed,
        ..
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn suspended_retry_wakes_for_window_expiry_before_cooldown() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 200,
    };
    let actor_id = create_system_with(
      ALICE,
      schedule,
      Some(ScheduleWindow { start: 1, end: 101 }),
      temporary_retry_swap_plan(),
    );
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(canonical_scheduled_wakeup_block(actor_id), Some(102));
    #[cfg(not(feature = "runtime-benchmarks"))]
    {
      assert!(!ActorIdentities::<Test>::contains_key(actor_id));
      let locator = crate::ActorControlLocators::<Test>::get(actor_id);
      assert!(
        matches!(
          locator,
          Some(crate::ActorControlLocation::Waiting {
            key: WakeupKey::Block(102),
            ..
          })
        ),
        "window-expiry locator: {locator:?}; hot={:?}",
        Actors::actor_hot(actor_id)
      );
      assert!(Actors::load_frame_control_authority(actor_id).is_some());
    }

    let ticket_frontier_before_expiry = crate::ActorReadyTail::<Test>::get();
    frame_system::Pallet::<Test>::set_block_number(102);
    run_idle(Weight::MAX);
    assert_eq!(
      crate::ActorReadyTail::<Test>::get(),
      ticket_frontier_before_expiry,
      "exact window expiry must not publish a transient Ready ticket",
    );
    assert!(
      Actors::active_actor_view(actor_id).is_none(),
      "actor={:?}; locator={:?}; hot={:?}; wakeup_fault={:?}",
      Actors::active_actor_view(actor_id),
      crate::ActorControlLocators::<Test>::get(actor_id),
      Actors::actor_hot(actor_id),
      crate::WakeupWorkerFaultState::<Test>::get(),
    );
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::WindowExpired,
        } if *id == actor_id
      )
    }));
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_user_window_expiry_releases_hold_from_consumed_waiting_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 200,
      },
      Some(ScheduleWindow { start: 1, end: 101 }),
      temporary_retry_swap_plan(),
    );
    fund_native(actor_id, 1_000_000_000_000_000_000);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    Actors::on_idle(1, Weight::MAX);
    assert_eq!(canonical_scheduled_wakeup_block(actor_id), Some(102));
    assert!(crate::ActorStateHolds::<Test>::contains_key(actor_id));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));

    let ticket_frontier_before_expiry = crate::ActorReadyTail::<Test>::get();
    frame_system::Pallet::<Test>::set_block_number(102);
    Actors::on_initialize(102);
    run_prepass();
    Actors::on_idle(102, Weight::MAX);
    assert_eq!(
      crate::ActorReadyTail::<Test>::get(),
      ticket_frontier_before_expiry,
      "User window expiry must not publish a transient Ready ticket",
    );
    assert!(!Actors::active_actor_exists(actor_id));
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!crate::ActorStateHolds::<Test>::contains_key(actor_id));
    assert!(!crate::ActorFunding::<Test>::contains_key(actor_id));
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::WindowExpired,
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn user_retry_admits_and_charges_only_each_current_step() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      }),
      StepOf::<Test> {
        precondition: None,
        task: Task::SwapIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::Fixed(10),
          slippage_tolerance: Perbill::one(),
        },
        on_error: RETRY_LATER,
      },
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      }),
    ])
    .expect("three-step User plan fits");
    let prefix_fee = user_step_fee(&plan[0]);
    let retry_fee = user_step_fee(&plan[1]);
    let tail_fee = user_step_fee(&plan[2]);
    let pipeline_fee = pipeline_opening_fee(&plan);
    let actor_id = create_user_with(ALICE, Mutability::Mutable, manual_schedule(), None, plan);
    fund_native(actor_id, 1_000_000_000_000_000_000);
    let sink = TestFeeSink::get();
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let sink_before = native_balance(&sink);
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&sink) - sink_before,
      pipeline_fee + prefix_fee + retry_fee
    );
    assert_eq!(
      Actors::actor_run_state(actor_id).expect("Actor run").cursor,
      1
    );
    let retry_weight = current_step_execution_weight(actor_id);

    let first_eligible_at = Actors::actor_run_state(actor_id)
      .expect("first retry remains")
      .eligible_at;
    frame_system::Pallet::<Test>::set_block_number(first_eligible_at);
    Actors::on_initialize(first_eligible_at);
    run_prepass();
    let retry_budget = Actors::scheduler_admission_overhead()
      .saturating_add(retry_weight)
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::scheduler_on_idle_base())
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::materialization_coordinator_base())
      .saturating_add(
        <TestWeightInfo as crate::WeightInfo>::scheduler_wakeup_cursor_worker_future()
          .saturating_mul(2),
      )
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::crossing_worker_base())
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::observation_fanout_base())
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::block_resource_finalize());
    run_idle(retry_budget);
    run_next_idle(retry_budget);
    assert_eq!(
      native_balance(&sink) - sink_before,
      pipeline_fee + prefix_fee + retry_fee.saturating_mul(2)
    );
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("retry remains")
        .unsuccessful_attempts_at_cursor,
      2
    );

    let second_eligible_at = Actors::actor_run_state(actor_id)
      .expect("second retry remains")
      .eligible_at;
    frame_system::Pallet::<Test>::set_block_number(second_eligible_at);
    set_temporary_dex_failure(false);
    Actors::on_initialize(second_eligible_at);
    run_prepass();
    run_idle(Weight::MAX);
    run_next_idle(Weight::MAX);
    assert_eq!(
      native_balance(&sink) - sink_before,
      pipeline_fee + prefix_fee + retry_fee.saturating_mul(3) + tail_fee
    );
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
  });
}

#[test]
fn user_retry_insolvency_closes_before_effect_capacity_deferral() {
  for solvent in [false, true] {
    for effect_capacity in [
      Weight::from_parts(0, u64::MAX / 2),
      Weight::from_parts(u64::MAX / 2, 0),
    ] {
      new_test_ext().execute_with(|| {
        System::set_block_number(1);
        setup_temporary_retry_pool();
        let contract = user_active_contract(
          Schedule {
            trigger: Trigger::manual(),
            cooldown_blocks: 1,
          },
          None,
          temporary_retry_swap_plan(),
        )
        .expect("active retry Contract");
        prefund_active_user_creation(ALICE, &contract.steps);
        let actor_id = Actors::next_actor_id();
        assert_ok!(Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          Mutability::Mutable,
          Some(contract)
        ));
        fund_native(actor_id, 1_000_000_000_000_000_000);
        set_temporary_dex_failure(true);
        System::set_block_number(2);
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          actor_id
        ));
        Actors::on_idle(2, Weight::MAX);
        let run = Actors::actor_run_state(actor_id).expect("temporary failure suspends");
        assert_eq!(run.unsuccessful_attempts_at_cursor, 1);
        assert_eq!(run.eligible_at, 3);
        assert!(matches!(
          crate::ActorControlLocators::<Test>::get(actor_id),
          Some(crate::ActorControlLocation::Ready { .. })
        ));
        let sovereign = sovereign_account(actor_id);
        if !solvent {
          let excess = native_balance(&sovereign)
            .checked_sub(TestMinUserBalance::get())
            .expect("funded Actor covers the protected floor");
          deplete_user_sovereign(actor_id, excess);
        }
        let custody_before = native_balance(&sovereign);
        let actor_before = Actors::active_actor_view(actor_id).expect("Suspended User");
        clear_fee_collections();
        System::reset_events();
        System::set_block_number(run.eligible_at);
        let limits = crate::SimulationBudget {
          actor_control: Weight::from_parts(u64::MAX / 2, u64::MAX / 2),
          shared_economic: effect_capacity,
        }
        .checked_limits()
        .expect("independent component limits fit");
        let mut resources = crate::BlockResourceState::new(run.eligible_at);
        assert_ok!(resources.begin_prepass());
        assert_ok!(resources.open_external_phase());
        assert_ok!(resources.begin_drain());
        let pass = Actors::execute_cycle_to_cutoff_with_resources(
          Weight::MAX,
          Actors::queue_tail(),
          &mut resources,
          limits,
          crate::BlockResourceDomain::ActorDrainEffect,
          limits.actor_control(),
        );
        assert_eq!(resources.outstanding_reservations(), 0);
        assert_eq!(resources.usage().actor_effect_used(), Weight::zero());
        assert_eq!(
          pass.reconciled_domains(),
          Some((pass.consumed, Weight::zero()))
        );
        assert_eq!(native_balance(&sovereign), custody_before);
        assert!(fee_collections().is_empty());
        assert!(!has_actor_event(|event| matches!(
          event,
          Event::StepFailed { actor_id: id, .. }
            | Event::ActionFeeCharged { actor_id: id, .. }
            | Event::CycleStarted { actor_id: id, .. } if *id == actor_id
        )));
        if solvent {
          assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
          assert_eq!(
            Actors::actor_run_state(actor_id)
              .expect("deferred Run remains")
              .encode(),
            run.encode()
          );
          assert_eq!(
            Actors::paged_head_entry().map(|(_, entry)| entry.actor_id),
            Some(actor_id)
          );
          Actors::execute_cycle_to_cutoff(Weight::MAX, Actors::queue_tail());
          let retried = Actors::actor_run_state(actor_id).expect("funded retry suspends again");
          assert_eq!(retried.cycle_nonce, run.cycle_nonce);
          assert_eq!(retried.cursor, run.cursor);
          assert_eq!(retried.unsuccessful_attempts_at_cursor, 2);
          assert!(!fee_collections().is_empty());
        } else {
          assert!(!Actors::active_actor_exists(actor_id));
          assert!(Actors::actor_run_state(actor_id).is_none());
          assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
          assert!(!crate::ActorStateHolds::<Test>::contains_key(actor_id));
          assert!(has_actor_event(|event| matches!(
            event,
            Event::ActorClosed {
              actor_id: id,
              reason: CloseReason::CycleAdmissionInsufficient,
            } if *id == actor_id
          )));
        }
        #[cfg(feature = "try-runtime")]
        assert_ok!(Actors::do_try_state());
      });
    }
  }
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_underfunded_retry_apoptosis_closes_from_retained_ready_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 1,
      },
      None,
      temporary_retry_swap_plan(),
    );
    fund_native(actor_id, 1_000_000_000_000_000_000);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    Actors::on_idle(1, Weight::MAX);
    let run = Actors::actor_run_state(actor_id).expect("temporary failure suspends");
    assert_eq!(run.eligible_at, 2);
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Ready { .. })
    ));
    let sovereign = sovereign_account(actor_id);
    let balance = native_balance(&sovereign);
    deplete_user_sovereign(actor_id, balance - TestMinUserBalance::get());
    let custody_before = native_balance(&sovereign);
    clear_fee_collections();

    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    run_prepass();
    Actors::on_idle(2, Weight::MAX);

    assert!(!Actors::active_actor_exists(actor_id));
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!crate::ActorStateHolds::<Test>::contains_key(actor_id));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert_eq!(native_balance(&sovereign), custody_before);
    assert!(fee_collections().is_empty());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::CycleAdmissionInsufficient,
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn consumed_frame_finalizer_rejects_a_retained_primary() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let admission = Actors::actor_control_cell(actor_id)
      .expect("creation admits the Contract")
      .1
      .admission;
    let state = Actors::active_actor_state(actor_id)
      .expect("capture state while primary remains authoritative");

    assert_noop!(
      Actors::finalize_actor_from_consumed_state(
        actor_id,
        state,
        &admission,
        CloseReason::SchedulerIndexExhausted,
      ),
      Error::<Test>::ActorInvariant
    );
    assert!(Actors::active_actor_exists(actor_id));
    assert!(crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn run_opening_snapshot_is_complete_frozen_and_capacity_checked_live() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_a = TestAsset::Local(1);
    let asset_b = TestAsset::Local(2);
    let asset_c = TestAsset::Local(3);
    let asset_out = TestAsset::Local(4);
    setup_pool(asset_b, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: asset_a,
        amount: AmountResolution::PercentageAtOpening(Perbill::from_percent(10)),
      }),
      StepOf::<Test> {
        precondition: None,
        task: Task::SwapIn {
          asset_in: asset_b,
          asset_out,
          amount_in: AmountResolution::PercentageAtOpening(Perbill::from_percent(50)),
          slippage_tolerance: Perbill::one(),
        },
        on_error: RETRY_LATER,
      },
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: asset_c,
        amount: AmountResolution::PercentageAtOpening(Perbill::from_percent(10)),
      }),
    ])
    .expect("three-step plan fits");
    let actor_id = create_system_with(ALICE, percentage_trigger_schedule(), None, plan);
    let actor = sovereign_account(actor_id);
    for asset in [asset_a, asset_b, asset_c] {
      set_asset_balance(&actor, asset, 100);
    }
    set_temporary_dex_failure(true);
    signal_percentage_trigger(actor_id, asset_b);
    run_idle(Weight::MAX);

    let continuation = Actors::actor_run_state(actor_id).expect("suspended");
    assert_eq!(continuation.cursor, 1);
    assert_eq!(continuation.opening_snapshot.len(), 3);
    assert_eq!(
      continuation
        .opening_snapshot
        .get(&OpeningSurface::PreservableAsset(asset_a)),
      Some(&99)
    );
    assert_eq!(
      continuation
        .opening_snapshot
        .get(&OpeningSurface::PreservableAsset(asset_b)),
      Some(&99)
    );
    let mut mutated_snapshot = continuation.clone();
    *mutated_snapshot
      .opening_snapshot
      .get_mut(&OpeningSurface::PreservableAsset(asset_a))
      .expect("Opening asset remains present") = 98;
    assert_noop!(
      Actors::write_run_state(actor_id, Some(mutated_snapshot)),
      Error::<Test>::ActorRunInvariant
    );
    assert_eq!(
      Actors::actor_run_state(actor_id).map(|state| state.encode()),
      Some(continuation.encode())
    );
    assert_eq!(asset_balance(&BOB, asset_a), 9);

    assert_ok!(MockAssetOps::transfer(&actor, &BOB, asset_b, 20));
    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(continuation.eligible_at);
    run_idle(Weight::MAX);
    run_next_idle(Weight::MAX);
    let after_capacity_failure = Actors::actor_run_state(actor_id).expect("still suspended");
    assert_eq!(after_capacity_failure.cursor, 1);
    assert_eq!(after_capacity_failure.unsuccessful_attempts_at_cursor, 2);
    assert_eq!(
      after_capacity_failure
        .opening_snapshot
        .get(&OpeningSurface::PreservableAsset(asset_b)),
      Some(&99)
    );

    set_asset_balance(&actor, asset_b, 100);
    frame_system::Pallet::<Test>::set_block_number(after_capacity_failure.eligible_at);
    Actors::on_initialize(after_capacity_failure.eligible_at);
    run_prepass();
    run_idle(Weight::MAX);
    run_next_idle(Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(asset_balance(&actor, asset_b), 82);
    assert_eq!(asset_balance(&CHARLIE, asset_c), 9);
    assert_eq!(asset_balance(&BOB, asset_a), 9);
  });
}

#[test]
fn maximal_run_opening_snapshot_stays_bounded_to_full_contract_surfaces() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut steps = Vec::new();
    let max_steps = <<Test as crate::Config>::MaxContractSteps as Get<u32>>::get();
    for index in 0..max_steps {
      steps.push(StepOf::<Test> {
        precondition: None,
        task: Task::AddLiquidity {
          asset_a: TestAsset::Local(100 + index * 2),
          asset_b: TestAsset::Local(101 + index * 2),
          amount_a: AmountResolution::PercentageAtOpening(Perbill::from_percent(10)),
          amount_b: AmountResolution::PercentageAtOpening(Perbill::from_percent(10)),
          min_lp_out: 1,
        },
        on_error: if index == 0 {
          RETRY_LATER
        } else {
          StepErrorPolicy::AbortCycle
        },
      });
    }
    let plan = BoundedVec::try_from(steps).expect("maximal System plan fits");
    let actor_id = create_system_with(ALICE, percentage_trigger_schedule(), None, plan);
    let actor = sovereign_account(actor_id);
    for index in 0..max_steps.saturating_mul(2) {
      set_asset_balance(&actor, TestAsset::Local(100 + index), 100);
    }
    set_temporary_add_liquidity_failure(true);
    signal_percentage_trigger(actor_id, TestAsset::Local(100));
    run_idle(Weight::MAX);

    let continuation = Actors::actor_run_state(actor_id).expect("maximal Actor run");
    assert_eq!(continuation.cursor, 0);
    assert_eq!(
      continuation.opening_snapshot.len() as u32,
      <<Test as crate::Config>::MaxOpeningSnapshotEntries as Get<u32>>::get()
    );

    set_temporary_add_liquidity_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());
  });
}

#[test]
fn run_attempts_have_unique_chain_coordinates_without_the_stored_ordinal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      temporary_retry_swap_plan(),
    );
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    run_idle(Weight::MAX);
    let opening_event_index = frame_system::Pallet::<Test>::events()
      .iter()
      .position(|record| matches!(record.event, RuntimeEvent::Actors(Event::CycleStarted { actor_id: id, cycle_nonce: 1 }) if id == actor_id))
      .expect("opening attempt has a chain event coordinate");
    let opening: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    let cycle_started = opening
      .iter()
      .position(|event| matches!(event, Event::CycleStarted { actor_id: id, cycle_nonce: 1 } if *id == actor_id))
      .expect("Opening emits CycleStarted");
    assert!(matches!(opening[cycle_started + 1], Event::StepFailed { actor_id: id, cycle_nonce: 1, step_index: 0, .. } if id == actor_id));
    assert!(matches!(opening[cycle_started + 2], Event::CycleSuspended { actor_id: id, cycle_nonce: 1, cursor: 0, reason: SuspensionReason::Temporary, .. } if id == actor_id));

    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let retry_event_index = frame_system::Pallet::<Test>::events()
      .iter()
      .position(|record| matches!(record.event, RuntimeEvent::Actors(Event::CycleContinued { actor_id: id, cycle_nonce: 1, cursor: 0, .. }) if id == actor_id))
      .expect("retry attempt has a chain event coordinate");
    let retry: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert!(matches!(retry[0], Event::CycleContinued { actor_id: id, cycle_nonce: 1, cursor: 0 } if id == actor_id));
    assert!(matches!(retry[1], Event::StepFailed { actor_id: id, cycle_nonce: 1, step_index: 0, .. } if id == actor_id));
    assert!(matches!(retry[2], Event::CycleSuspended { actor_id: id, cycle_nonce: 1, cursor: 0, .. } if id == actor_id));

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(4);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let completion_event_index = frame_system::Pallet::<Test>::events()
      .iter()
      .position(|record| matches!(record.event, RuntimeEvent::Actors(Event::CycleContinued { actor_id: id, cycle_nonce: 1, cursor: 0, .. }) if id == actor_id))
      .expect("completion attempt has a chain event coordinate");
    let completion: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert!(matches!(completion[0], Event::CycleContinued { actor_id: id, cycle_nonce: 1, cursor: 0 } if id == actor_id));
    assert!(matches!(completion[1], Event::SwapExecuted { actor_id: id, .. } if id == actor_id));
    assert!(matches!(completion[2], Event::CycleSummary { actor_id: id, cycle_nonce: 1, outcomes: OutcomeTotals { failed_steps: 2, .. }, .. } if id == actor_id));

    let attempt_coordinates = [
      (1u64, 1u64, opening_event_index),
      (1u64, 2u64, retry_event_index),
      (1u64, 4u64, completion_event_index),
    ];
    assert_eq!(
      attempt_coordinates.into_iter().collect::<BTreeSet<_>>().len(),
      attempt_coordinates.len(),
      "cycle nonce plus block/event coordinates uniquely identify every attempt without its stored ordinal"
    );
  });
}

#[test]
fn completion_policy_only_replacement_cancels_contract_bound_run() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let before = Actors::active_actor_view(actor_id).expect("suspended actor");
    let continuation_before = Actors::actor_run_state(actor_id).expect("suspended Actor run");
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      before.steps.clone(),
      crate::CompletionPolicy::CloseAfterProductiveCycle,
    ));
    let after = Actors::active_actor_view(actor_id).expect("updated actor");
    assert_eq!(after.steps, before.steps);
    assert_eq!(
      after.completion,
      crate::CompletionPolicy::CloseAfterProductiveCycle
    );
    assert_eq!(after.cycle_state, CycleState::Idle);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(after.cycle_nonce, continuation_before.cycle_nonce);
    let events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(events.len(), 3);
    assert!(matches!(
      events.first(),
      Some(Event::CycleCancelled {
        actor_id: id,
        cycle_nonce,
        reason: CancellationReason::ContractReplaced,
      }) if *id == actor_id && *cycle_nonce == continuation_before.cycle_nonce
    ));
    assert!(matches!(
      events.get(1),
      Some(Event::CycleSummary {
        actor_id: id,
        cycle_nonce,
        result: CycleResult::Cancelled,
        ..
      }) if *id == actor_id && *cycle_nonce == continuation_before.cycle_nonce
    ));
    assert!(matches!(
      events.get(2),
      Some(Event::ContractUpdated { actor_id: id, .. }) if *id == actor_id
    ));
  });
}

#[test]
fn permissionless_sweep_is_lifecycle_touchpoint_only_under_breaker() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    assert_ok!(Actors::permissionless_sweep(
      RuntimeOrigin::signed(BOB),
      actor_id
    ));
    let instance = Actors::active_actor_view(actor_id).expect("system Actors remains alive");
    assert_eq!(instance.cycle_nonce, 0);
    assert_eq!(Actors::queue_head(), Actors::queue_tail());
  });
}

#[test]
fn tiny_percentage_amount_is_skipped_without_contract_steps_failure() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfCurrent(Perbill::from_parts(1)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let inst = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(inst.unsuccessful_attempt_streak, 0);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::ResolutionSkipped,
          ..
        } if *id == actor_id
      )
    }));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          cycle_nonce: 1,
          result: CycleResult::Completed,
          outcomes: OutcomeTotals {
            executed_steps: 0,
            committed_effectful_tasks: 0,
            precondition_skips: 0,
            skipped_resolution: 1,
            skipped_funding_unavailable: 0,
            failed_steps: 0,
          },
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn cycle_summary_tracks_step_outcomes() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step_conditions = all_conditions(vec![Predicate::BalanceAbove {
      asset: TestAsset::Native,
      threshold: 1_000,
    }]);
    let contract_steps = BoundedVec::try_from(vec![
      StepOf::<Test> {
        precondition: step_conditions,
        task: Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(10),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfCurrent(Perbill::from_parts(1)),
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      }),
      StepOf::<Test> {
        precondition: None,
        task: Task::SwapIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::Fixed(10),
          slippage_tolerance: Perbill::one(),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
    ])
    .expect("contract_steps must fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          cycle_nonce: 1,
          result: CycleResult::Completed,
          outcomes: OutcomeTotals {
            executed_steps: 1,
            committed_effectful_tasks: 1,
            precondition_skips: 1,
            skipped_resolution: 1,
            skipped_funding_unavailable: 0,
            failed_steps: 1,
          },
        } if *id == actor_id
      )
    }));
    let actor_events = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect::<Vec<_>>();
    let summary_index = actor_events
      .iter()
      .position(|event| {
        matches!(
          event,
          Event::CycleSummary { actor_id: id, cycle_nonce: 1, .. } if *id == actor_id
        )
      })
      .expect("Cycle summary exists");
    assert!(summary_index < actor_events.len() - 1);
    assert!(matches!(
      actor_events.last(),
      Some(Event::ActionFeeCharged {
        actor_id: id,
        cycle_nonce: 1,
        step_index: 3,
        fee: 0,
        ..
      }) if *id == actor_id
    ));
  });
}

#[test]
fn cycle_success_predicate_drives_failure_reset_auto_close_and_event_order() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let failing_step = |on_error| StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error,
    };
    let continue_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![
        failing_step(StepErrorPolicy::ContinueNextStep),
        failing_step(StepErrorPolicy::ContinueNextStep),
      ])
      .expect("two steps fit"),
    );
    fund_native(continue_id, 100);
    mutate_actor_hot_coherent(continue_id, |hot| hot.unsuccessful_attempt_streak = 2);
    assert_ok!(replace_auto_close(
      RuntimeOrigin::root(),
      continue_id,
      Some(2)
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      continue_id
    ));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let after_first = Actors::active_actor_view(continue_id).expect("successful actor remains active");
    assert_eq!(after_first.cycle_nonce, 1);
    assert_eq!(after_first.unsuccessful_attempt_streak, 0);
    frame_system::Pallet::<Test>::set_block_number(3);
    Actors::on_initialize(3);
    run_prepass();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      continue_id
    ));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(4);
    Actors::on_initialize(4);
    run_prepass();
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(continue_id).is_none());
    let continue_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(continue_events.len(), 7);
    assert!(matches!(continue_events[0], Event::CycleStarted { actor_id, .. } if actor_id == continue_id));
    assert!(matches!(continue_events[1], Event::StepFailed { actor_id, step_index: 0, .. } if actor_id == continue_id));
    assert!(matches!(continue_events[2], Event::ActionFeeCharged { actor_id, step_index: 0, fee: 0, .. } if actor_id == continue_id));
    assert!(matches!(continue_events[3], Event::StepFailed { actor_id, step_index: 1, .. } if actor_id == continue_id));
    assert!(matches!(continue_events[4], Event::CycleSummary { actor_id, result: CycleResult::Completed, outcomes: OutcomeTotals { failed_steps: 2, .. }, .. } if actor_id == continue_id));
    assert!(matches!(continue_events[5], Event::ActorClosed { actor_id, reason: CloseReason::AutoCloseNonceReached } if actor_id == continue_id));
    assert!(matches!(continue_events[6], Event::ActionFeeCharged { actor_id, step_index: 1, fee: 0, .. } if actor_id == continue_id));
    let skip_step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 1,
      }]),
      task: Task::Stake {
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let skip_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![skip_step]).expect("one step fits"),
    );
    assert_ok!(replace_auto_close(
      RuntimeOrigin::root(),
      skip_id,
      Some(1)
    ));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), skip_id));
    frame_system::Pallet::<Test>::reset_events();
    run_next_idle(Weight::MAX);
    assert!(Actors::active_actor_view(skip_id).is_none());
    let skip_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(skip_events.len(), 4);
    assert!(matches!(skip_events[0], Event::CycleStarted { actor_id, .. } if actor_id == skip_id));
    assert!(matches!(skip_events[1], Event::StepSkipped { actor_id, step_index: 0, .. } if actor_id == skip_id));
    assert!(matches!(skip_events[2], Event::CycleSummary { actor_id, result: CycleResult::Completed, outcomes: OutcomeTotals { precondition_skips: 1, failed_steps: 0, .. }, .. } if actor_id == skip_id));
    assert!(matches!(skip_events[3], Event::ActorClosed { actor_id, reason: CloseReason::AutoCloseNonceReached } if actor_id == skip_id));
    let abort_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![
        failing_step(StepErrorPolicy::AbortCycle),
        make_step(Task::Stake {
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(1),
        }),
      ])
      .expect("two steps fit"),
    );
    fund_native(abort_id, 100);
    assert_ok!(replace_auto_close(
      RuntimeOrigin::root(),
      abort_id,
      Some(1)
    ));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), abort_id));
    frame_system::Pallet::<Test>::reset_events();
    run_next_idle(Weight::MAX);
    let abort_instance = Actors::active_actor_view(abort_id).expect("aborted actor remains active");
    assert_eq!(abort_instance.unsuccessful_attempt_streak, 1);
    let abort_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(abort_events.len(), 4);
    assert!(matches!(abort_events[0], Event::CycleStarted { actor_id, .. } if actor_id == abort_id));
    assert!(matches!(abort_events[1], Event::StepFailed { actor_id, step_index: 0, .. } if actor_id == abort_id));
    assert!(matches!(abort_events[2], Event::CycleSummary { actor_id, result: CycleResult::Failed, outcomes: OutcomeTotals { executed_steps: 0, failed_steps: 1, .. }, .. } if actor_id == abort_id));
    assert!(matches!(abort_events[3], Event::ActionFeeCharged { actor_id, step_index: 0, fee: 0, .. } if actor_id == abort_id));
    let close_only_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), close_only_id));
    let close_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(close_events.len(), 1);
    assert!(matches!(close_events[0], Event::ActorClosed { actor_id, reason: CloseReason::OwnerInitiated } if actor_id == close_only_id));
  });
}

#[test]
fn percentage_at_opening_uses_cycle_start_snapshot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      }),
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageAtOpening(Perbill::from_percent(50)),
      }),
    ])
    .expect("contract_steps fits");
    let actor_id = create_system_with(ALICE, percentage_trigger_schedule(), None, contract_steps);
    fund_native(actor_id, 101);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    let actor = sovereign_account(actor_id);
    let actor_before = native_balance(&actor);
    signal_percentage_trigger(actor_id, TestAsset::Native);
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(native_balance(&CHARLIE), charlie_before.saturating_add(50));
    assert_eq!(native_balance(&actor), actor_before.saturating_sub(100));
  });
}

#[test]
fn notify_address_event_accumulates_without_pause_resume_cycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(100)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      100
    ));
    assert_eq!(native_balance(&actor), 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 1);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 2);
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(instance.lifecycle, ActiveLifecycle::Active);
    frame_system::Pallet::<Test>::set_block_number(3);
    fund_native(actor_id, 500);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      500,
      &CHARLIE
    ));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&500)
    );
    assert!(!has_actor_event(|event| {
      matches!(event, Event::ActorResumed { actor_id: id } if *id == actor_id)
    }));
  });
}

#[test]
fn zero_step_user_contract_is_first_class_geometry() {
  new_test_ext().execute_with(|| {
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, BoundedVec::default()),
    ));
    let head = crate::ActorContractHeads::<Test>::get(0).expect("zero-Step head exists");
    assert_eq!(head.header.step_count, 0);
    assert!(head.first_step.is_none());
    assert!(head.first_step_resources.is_none());
    assert_eq!(
      Actors::actor_contract(0)
        .expect("zero-Step Contract reconstructs")
        .steps
        .len(),
      0
    );
  });
}

#[test]
fn contract_steps_hard_ceiling_is_255_steps() {
  assert!(!crate::contract_steps_bound_is_valid(0));
  assert!(crate::contract_steps_bound_is_valid(1));
  assert!(crate::contract_steps_bound_is_valid(8));
  assert!(crate::contract_steps_bound_is_valid(255));
  assert!(!crate::contract_steps_bound_is_valid(256));
}

#[test]
fn contract_steps_bound_is_single_and_encoded_for_both_classes() {
  new_test_ext().execute_with(|| {
    assert_eq!(
      <<Test as crate::Config>::MaxContractSteps as Get<u32>>::get(),
      12
    );
    let steps: Vec<_> = (0..13)
      .map(|i| {
        make_step(Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(i + 1),
        })
      })
      .collect();
    assert!(crate::ContractSteps::<Test>::try_from(steps).is_err());
    let maximum = BoundedVec::try_from(
      (0..12)
        .map(|i| {
          make_step(Task::Transfer {
            to: BOB,
            asset: TestAsset::Native,
            amount: AmountResolution::Fixed(i + 1),
          })
        })
        .collect::<Vec<_>>(),
    )
    .expect("twelve steps fit the shared encoded bound");
    prefund_active_user_creation(ALICE, &maximum);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, maximum.clone()),
    ));
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract(manual_schedule(), None, maximum),
    ));
  });
}

#[test]
fn cycle_nonce_max_value_is_the_last_executable_cycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    set_actor_cycle_nonce_coherent(actor_id, u64::MAX - 1);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    run_idle(Weight::MAX);
    let instance = Actors::active_actor_view(actor_id).expect("system Actors remains");
    assert_eq!(instance.cycle_nonce, u64::MAX);
    assert_eq!(native_balance(&BOB), bob_before + 1);
    assert!(has_actor_event(|event| {
      matches!(event, Event::CycleStarted { actor_id: id, cycle_nonce } if *id == actor_id && *cycle_nonce == u64::MAX)
    }));
    assert!(has_actor_event(|event| {
      matches!(event, Event::CycleSummary { actor_id: id, cycle_nonce, .. } if *id == actor_id && *cycle_nonce == u64::MAX)
    }));
  });
}

#[test]
fn cycle_nonce_exhaustion_closes_user_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    fund_native(actor_id, 1_000);
    let bob_before = native_balance(&BOB);
    set_actor_cycle_nonce_coherent(actor_id, u64::MAX);
    let expected_contract = Actors::load_actor_contract(actor_id).expect("contract exists");
    let simulation = Actors::simulate_current_contract(
      actor_id,
      ActorType::User,
      Mutability::Mutable,
      expected_contract,
      SimulationMode::FreshCurrentPlan,
      ample_simulation_budget(),
    )
    .expect("nonce exhaustion is a terminal projection, not arithmetic failure");
    assert_eq!(
      simulation.status,
      AttemptDisposition::Closed(CloseReason::CycleNonceExhausted)
    );
    assert_eq!(simulation.cycle_nonce, u64::MAX);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before);
    assert!(!has_actor_event(|event| {
      matches!(event, Event::CycleStarted { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::CycleSummary { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::CycleNonceExhausted,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn cycle_nonce_exhaustion_closes_system_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let bob_before = native_balance(&BOB);
    set_actor_cycle_nonce_coherent(actor_id, u64::MAX);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before);
    assert!(!has_actor_event(|event| {
      matches!(event, Event::CycleStarted { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::CycleSummary { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::CycleNonceExhausted,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn schedule_cooldown_is_bounded_by_max_execution_delay() {
  new_test_ext().execute_with(|| {
    let too_long = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: u32::try_from(
        <Test as crate::Config>::MaxExecutionDelayBlocks::get().saturating_add(1),
      )
      .unwrap_or(u32::MAX),
    };
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(too_long, None, transfer_contract_steps(BOB, 10),),
      ),
      Error::<Test>::ExecutionDelayTooLong
    );
  });
}

#[test]
fn retry_target_uses_only_cursor_local_count_and_last_attempt_block() {
  new_test_ext().execute_with(|| {
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let instance = Actors::active_actor_view(actor_id).expect("actor");
    let run_state = |last_attempt_block, eligible_at| RuntimeActorRunState {
      contract_authority: run_contract_authority(actor_id),
      cycle_nonce: 1,
      cursor: 0,
      opening_predicate_cursor: 0,
      unsuccessful_attempts_at_cursor: 1,
      last_attempt_block,
      last_committed_step_block: None,
      eligible_at,
      opening_snapshot: Default::default(),
      opening_predicate_results: Default::default(),
      funding_snapshot: Default::default(),
      cumulative_outcomes: Default::default(),
      last_step_outcome: Some(StepOutcome::FundingUnavailable),
      suspension: Some(SuspensionReason::FundingUnavailable),
    };
    ActorRunStateStore::<Test>::insert(actor_id, run_state(u64::MAX - 1, u64::MAX));
    assert_eq!(Actors::retry_eligible_at(actor_id, &instance), Ok(u64::MAX));

    ActorRunStateStore::<Test>::insert(actor_id, run_state(u64::MAX, u64::MAX));
    assert_eq!(
      Actors::retry_eligible_at(actor_id, &instance),
      Err(crate::EnqueueOutcome::SchedulerIndexExhausted)
    );
  });
}

#[test]
fn predicate_evaluator_visits_every_atom_and_preserves_first_error() {
  let precondition = any_conditions(vec![
    Predicate::BlockNumberAbove { threshold: 0 },
    Predicate::BlockNumberAbove { threshold: 1 },
    Predicate::BlockNumberAbove { threshold: 2 },
    Predicate::BlockNumberAbove { threshold: 3 },
  ]);
  let mut visits = 0u32;
  let result = crate::execution::evaluate_precondition_with(
    precondition.as_ref().expect("precondition"),
    |_| {
      visits += 1;
      if visits == 1 { Err("first") } else { Ok(true) }
    },
  );
  assert_eq!(result, Err("first"));
  assert_eq!(visits, 4);
}

#[test]
fn opening_and_current_predicates_observe_distinct_step_state() {
  for (timing, second_step_executes) in [
    (ObservationTiming::Opening, true),
    (ObservationTiming::Current, false),
  ] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let plan = BoundedVec::try_from(vec![
        make_step(Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(60),
        }),
        StepOf::<Test> {
          precondition: timed_all_conditions(
            timing,
            vec![Predicate::BalanceAbove {
              asset: TestAsset::Native,
              threshold: 50,
            }],
          ),
          task: Task::Transfer {
            to: CHARLIE,
            asset: TestAsset::Native,
            amount: AmountResolution::Fixed(10),
          },
          on_error: StepErrorPolicy::AbortCycle,
        },
      ])
      .expect("two-step plan fits");
      let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
      fund_native(actor_id, 100);
      let charlie_before = native_balance(&CHARLIE);
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      run_idle(Weight::MAX);
      assert_eq!(
        native_balance(&CHARLIE) > charlie_before,
        second_step_executes
      );
      assert_eq!(
        has_actor_event(|event| matches!(
          event,
          Event::StepSkipped {
            actor_id: id,
            step_index: 1,
            reason: StepSkippedReason::PreconditionFalse,
            ..
          } if *id == actor_id
        )),
        !second_step_executes
      );
    });
  }
}

#[test]
fn opening_predicate_result_is_reused_by_run_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let plan = contract_steps_with_step(StepOf::<Test> {
      precondition: timed_all_conditions(
        ObservationTiming::Opening,
        vec![Predicate::BalanceAbove {
          asset: TestAsset::Native,
          threshold: 50,
        }],
      ),
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: RETRY_LATER,
    });
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 100);
    let actor = sovereign_account(actor_id);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let continuation = Actors::actor_run_state(actor_id).expect("Actor run exists");
    assert_eq!(
      continuation.opening_predicate_results.as_slice(),
      &[Ok(true)]
    );
    assert_ok!(MockAssetOps::transfer(&actor, &BOB, TestAsset::Native, 60));
    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::SwapExecuted { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn empty_outer_and_inner_precondition_forms_are_rejected() {
  new_test_ext().execute_with(|| {
    let empty_outer = Precondition {
      clauses: BoundedVec::default(),
    };
    let empty_inner = Precondition {
      clauses: BoundedVec::try_from(vec![BoundedVec::default()]).expect("empty clause fits"),
    };
    for precondition in [empty_outer, empty_inner] {
      let plan = contract_steps_with_step(StepOf::<Test> {
        precondition: Some(precondition),
        task: Task::StopCycle,
        on_error: StepErrorPolicy::AbortCycle,
      });
      assert_noop!(
        Actors::create_system_actor(
          RuntimeOrigin::root(),
          ALICE,
          Mutability::Mutable,
          system_active_contract(manual_schedule(), None, plan),
        ),
        Error::<Test>::EmptyPrecondition
      );
    }
  });
}

#[test]
fn any_with_multiple_true_atoms_executes_the_task_once() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(5);
    let step = StepOf::<Test> {
      precondition: any_conditions(vec![
        Predicate::BlockNumberAbove { threshold: 1 },
        Predicate::BlockNumberBelow { threshold: 10 },
      ]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(step),
    );
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(
      System::events()
        .iter()
        .filter(|record| matches!(
          record.event,
          RuntimeEvent::Actors(Event::TransferExecuted { actor_id: id, .. }) if id == actor_id
        ))
        .count(),
      1
    );
  });
}

#[test]
fn any_skip_cannot_create_continuation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(5);
    let step = StepOf::<Test> {
      precondition: any_conditions(vec![
        Predicate::BlockNumberBelow { threshold: 1 },
        Predicate::BlockNumberAbove { threshold: 10 },
      ]),
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: RETRY_LATER,
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(step),
    );
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor remains")
        .unsuccessful_attempt_streak,
      0
    );
  });
}

#[test]
fn retry_re_evaluates_live_any_conditions_at_the_same_cursor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let step = StepOf::<Test> {
      precondition: any_conditions(vec![
        Predicate::BlockNumberBelow { threshold: 2 },
        Predicate::BlockNumberAbove { threshold: 100 },
      ]),
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: RETRY_LATER,
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(step),
    );
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id).expect("suspended").cursor,
      0
    );

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { precondition_skips: 1, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn continue_next_step_error_policy_proceeds_after_failure() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let contract_steps = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(
      has_actor_event(|e| matches!(
        e,
        Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id
      )),
      "step 0 must fail"
    );
    assert_eq!(
      native_balance(&CHARLIE),
      charlie_before + 10,
      "step 1 must execute despite step 0 failure"
    );
  });
}

#[test]
fn staking_adapter_late_failure_rolls_back_partial_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Native;
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::Stake {
        asset,
        amount: AmountResolution::Fixed(40),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let contract_steps = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 120);
    set_fail_staking_after_burn(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 110);
    assert_eq!(staked_balance(actor, asset), 0);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(!has_actor_event(|e| matches!(
      e,
      Event::StakeExecuted { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 1, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn system_condition_respects_adapter_visible_native_lock() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 100,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(step),
    );
    fund_native(actor_id, 200);
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("system actor")
      .sovereign_account;
    set_native_transfer_lock(&sovereign, 150);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepSkipped { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
  });
}

#[test]
fn user_dca_complete_lifecycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // Step 1: Create User Actors with Timer trigger
    let schedule = timer_schedule(5);
    let foreign = TestAsset::Local(1);
    set_asset_balance(&ALICE, foreign, 10_000);
    let contract_steps = contract_steps_with_step(StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: foreign,
        threshold: 50,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: foreign,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    });
    let actor_id = Actors::next_actor_id();
    prefund_active_user_creation(ALICE, &contract_steps);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(schedule, None, contract_steps),
    ));
    // Verify creation
    assert!(has_actor_event(|e| matches!(
      e,
      Event::ActorCreated { actor_id: id, actor_class: ActorClass::User { .. }, .. } if *id == actor_id
    )));
    let instance = Actors::active_actor_view(actor_id).expect("instance exists");
    assert_eq!(instance.owner, ALICE);
    // Step 2: Fund sovereign
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, foreign, 500);
    fund_native(actor_id, 500); // For fees
    // Step 3-4: Advance blocks and verify execution
    for block in 2..8 {
      frame_system::Pallet::<Test>::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::MAX);
    }
    assert!(
      has_actor_event(|e| matches!(
        e,
        Event::TransferExecuted { actor_id: id, .. } if *id == actor_id
      )),
      "Should execute transfer on timer"
    );
    // Step 5: Multiple cycles
    let bob_before = asset_balance(&BOB, foreign);
    for block in 7..27 {
      frame_system::Pallet::<Test>::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::MAX);
    }
    let bob_after = asset_balance(&BOB, foreign);
    assert!(bob_after > bob_before, "Bob should receive transfers");
    // Step 6-7: Drain native below MinUserBalance; automatic cadence underfunding preserves process authority
    let actor_native = native_balance(&actor);
    let min_user = <Test as crate::Config>::MinUserBalance::get();
    let slash_amount = actor_native.saturating_sub(min_user / 2);
    let _ = <Balances as Currency<AccountId>>::slash(&actor, slash_amount);
    assert!(
      native_balance(&actor) < min_user,
      "Actor balance must be below MinUserBalance after slash"
    );
    // Later due occurrences advance without fee/readiness/apoptosis; lifecycle touchpoints do not predict solvency
    for block in 30..50 {
      frame_system::Pallet::<Test>::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::MAX);
      if Actors::active_actor_view(actor_id).is_none() {
        break;
      }
    }
    let retained = Actors::active_actor_view(actor_id)
      .expect("underfunded Cadenced User process remains active");
    assert!(!retained.pending_signal);
    assert!(retained.trigger_wakeup_pointer.is_some());
  });
}

#[test]
fn auto_close_threshold_reached_closes_actor_after_successful_cycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(replace_auto_close(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      Some(2),
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_some());
    #[cfg(not(feature = "runtime-benchmarks"))]
    assert!(matches!(
      Actors::load_frame_actor_state(actor_id),
      crate::LoadedActorStateOf::Active(_)
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    #[cfg(not(feature = "runtime-benchmarks"))]
    assert!(Actors::load_frame_control_authority(actor_id).is_none());
    #[cfg(not(feature = "runtime-benchmarks"))]
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::AutoCloseNonceReached,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn deferred_cycle_does_not_consume_auto_close_nonce_target() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(replace_auto_close(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      Some(1),
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(starvation_blocked_budget(actor_id));
    let inst = Actors::active_actor_view(actor_id).expect("Actors must exist");
    assert_eq!(inst.cycle_nonce, 0);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. } | Event::CycleSummary { actor_id: id, .. }
        if *id == actor_id
    )));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
  });
}

#[test]
fn pure_close_does_not_start_normal_cycle_or_execute_tasks() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let sovereign = sovereign_account(actor_id);
    fund_native_raw(&sovereign, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let before_close_nonce = Actors::active_actor_view(actor_id)
      .expect("Actors exists")
      .cycle_nonce;
    assert_eq!(before_close_nonce, 1);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(!has_actor_event(|event| {
      matches!(event, Event::CycleStarted { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::CycleSummary { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(has_actor_event(|event| {
      matches!(event, Event::ActorClosed { actor_id: id, .. } if *id == actor_id)
    }));
  });
}

#[test]
fn canonical_step_transition_matrix_has_production_simulation_parity() {
  let mut covered_rows = BTreeSet::new();
  let mut fresh_rows = BTreeSet::new();
  let mut current_rows = BTreeSet::new();
  let mut covered_policies = BTreeSet::new();
  let mut covered_mutability = BTreeSet::new();
  let mut covered_actor_types = BTreeSet::new();
  let mut covered_outcome_variants = BTreeSet::new();
  for case in STEP_TRANSITION_PARITY_MATRIX {
    covered_rows.insert(case.row);
    covered_policies.insert(case.policy.encode());
    covered_mutability.insert(case.mutability.encode());
    covered_actor_types.insert(case.actor_type.encode());
    let target_outcome = parity_expected_steps(*case)
      .into_iter()
      .find(|record| record.step_index == 1)
      .expect("matrix target outcome exists")
      .outcome;
    covered_outcome_variants.insert(
      *target_outcome
        .encode()
        .first()
        .expect("StepOutcome encoding carries a variant index"),
    );
    for current_run in [false, true] {
      if !current_run && case.bound == StepParityBound::LocalReached {
        continue;
      }
      if current_run {
        current_rows.insert(case.row);
      } else {
        fresh_rows.insert(case.row);
      }
      let target_index = u32::from(current_run);
      new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(10);
        set_max_consecutive_failures(if case.bound == StepParityBound::GlobalReached {
          1
        } else {
          10
        });
        set_observation(
          1,
          crate::ScalarObservationState::Fresh {
            value: 50,
            observed_at: u64::MAX,
          },
        );
        setup_temporary_retry_pool();
        set_temporary_dex_failure(case.stimulus == StepParityStimulus::TemporaryFailure);
        set_fail_dex_after_input_transfer(case.stimulus == StepParityStimulus::PermanentFailure);
        let steps = if current_run {
          parity_contract_steps(*case)
        } else {
          BoundedVec::try_from(
            parity_contract_steps(*case)
              .into_iter()
              .skip(1)
              .collect::<Vec<_>>(),
          )
          .expect("Fresh target and suffix fit")
        };
        let schedule = if case.stimulus == StepParityStimulus::PredicateError {
          observation_schedule(vec![1])
        } else if case.actor_type == ActorType::System && case.mutability == Mutability::Immutable {
          timer_schedule(1)
        } else {
          manual_schedule()
        };
        let expected_contract = match case.actor_type {
          ActorType::User => user_active_contract(schedule.clone(), None, steps.clone()),
          ActorType::System => system_active_contract(schedule.clone(), None, steps.clone()),
        }
        .expect("direct Actor Contract");
        let actor_id = match case.actor_type {
          ActorType::User => create_user_with(ALICE, case.mutability, schedule, None, steps),
          ActorType::System => {
            create_system_with_mutability(ALICE, case.mutability, schedule, None, steps)
          }
        };
        fund_native(
          actor_id,
          if case.actor_type == ActorType::User {
            1_000
          } else if case.stimulus == StepParityStimulus::FundingUnavailable {
            if current_run { 8 } else { 6 }
          } else {
            100
          },
        );
        if case.actor_type == ActorType::System && case.mutability == Mutability::Immutable {
          frame_system::Pallet::<Test>::set_block_number(11);
          assert!(enqueue_latched_actor(actor_id));
        } else if case.stimulus == StepParityStimulus::PredicateError {
          assert!(enqueue_latched_actor(actor_id));
        } else {
          assert_ok!(Actors::manual_trigger(
            RuntimeOrigin::signed(ALICE),
            actor_id
          ));
        }
        if current_run {
          // Commit the real prefix before comparing one target Step, never a whole-cycle runner.
          Actors::execute_cycle_to_cutoff(Weight::MAX, Actors::queue_tail());
          let prefix_run = Actors::actor_run_state(actor_id).expect("matrix prefix opens a Run");
          assert_eq!(prefix_run.cursor, 1, "{} prefix cursor", case.name);
          frame_system::Pallet::<Test>::set_block_number(prefix_run.eligible_at);
          if case.bound == StepParityBound::LocalReached {
            Actors::execute_cycle_to_cutoff(Weight::MAX, Actors::queue_tail());
            assert_eq!(
              Actors::actor_hot(actor_id).map(|hot| hot.cycle_state),
              Some(CycleState::Suspended)
            );
            assert_eq!(
              Actors::actor_run_state(actor_id).map(|state| state.cursor),
              Some(1)
            );
            let eligible_at = Actors::actor_run_state(actor_id)
              .expect("matrix suspension persists")
              .eligible_at;
            frame_system::Pallet::<Test>::set_block_number(eligible_at);
            let mut meter = WeightMeter::with_limit(Weight::MAX);
            Actors::drain_overdue_wakeups_cursor(eligible_at, &mut meter);
          }
        }
        let mode = if current_run {
          SimulationMode::CurrentRun
        } else {
          SimulationMode::FreshCurrentPlan
        };
        assert_eq!(
          Actors::paged_head_entry().map(|(_, entry)| entry.actor_id),
          Some(actor_id),
          "{} target is the real production FIFO head",
          case.name
        );
        System::reset_events();
        let actor = sovereign_account(actor_id);
        let streak_before = Actors::active_actor_view(actor_id)
          .expect("matrix actor remains active before production")
          .unsuccessful_attempt_streak;
        let before = (
          Actors::active_actor_view(actor_id).map(|view| view.encode()),
          Actors::actor_run_state(actor_id).map(|state| state.encode()),
          native_balance(&actor),
          native_balance(&BOB),
          native_balance(&CHARLIE),
          native_balance(&TestFeeSink::get()),
          native_balance(&u64::MAX),
          asset_balance(&actor, TestAsset::Local(77)),
          asset_balance(&u64::MAX, TestAsset::Local(77)),
          System::events(),
        );
        let simulation_root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
        let simulation = Actors::simulate_current_contract(
          actor_id,
          case.actor_type,
          case.mutability,
          expected_contract,
          mode,
          ample_simulation_budget(),
        )
        .unwrap_or_else(|error| panic!("{} simulation failed: {error:?}", case.name));
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(StateVersion::V1),
          simulation_root,
          "{} simulation changed whole storage",
          case.name
        );
        let after_simulation = (
          Actors::active_actor_view(actor_id).map(|view| view.encode()),
          Actors::actor_run_state(actor_id).map(|state| state.encode()),
          native_balance(&actor),
          native_balance(&BOB),
          native_balance(&CHARLIE),
          native_balance(&TestFeeSink::get()),
          native_balance(&u64::MAX),
          asset_balance(&actor, TestAsset::Local(77)),
          asset_balance(&u64::MAX, TestAsset::Local(77)),
          System::events(),
        );
        assert_eq!(
          after_simulation, before,
          "{} simulation persisted state or custody",
          case.name
        );
        assert_eq!(
          simulation.steps.as_slice(),
          vec![SimulationStepRecord {
            step_index: target_index,
            outcome: target_outcome.clone()
          }],
          "{} trace",
          case.name
        );

        clear_fee_collections();
        System::reset_events();
        let bob_before = native_balance(&BOB);
        let charlie_before = native_balance(&CHARLIE);
        let actor_before = native_balance(&actor);
        let sink_before = native_balance(&TestFeeSink::get());
        let pool_native_before = native_balance(&u64::MAX);
        Actors::execute_cycle_to_cutoff(Weight::MAX, Actors::queue_tail());
        let (status, outcomes, cursor, attempts) = observed_attempt_projection(actor_id);
        assert_eq!(status, simulation.status, "{} disposition", case.name);
        assert_eq!(
          outcomes, simulation.cumulative_outcomes,
          "{} outcomes",
          case.name
        );
        assert_eq!(cursor, simulation.run_cursor, "{} cursor", case.name);
        assert_eq!(
          attempts, simulation.unsuccessful_attempts_at_cursor,
          "{} local attempts",
          case.name
        );
        match status {
          AttemptDisposition::Completed | AttemptDisposition::Continued => assert_eq!(
            Actors::active_actor_view(actor_id)
              .expect("completed persistent matrix actor remains active")
              .unsuccessful_attempt_streak,
            0,
            "{} completed attempt resets the failure streak",
            case.name,
          ),
          AttemptDisposition::Failed | AttemptDisposition::Suspended => assert_eq!(
            Actors::active_actor_view(actor_id)
              .expect("failed or suspended matrix actor remains active")
              .unsuccessful_attempt_streak,
            streak_before
              .checked_add(1)
              .expect("matrix streak remains bounded"),
            "{} unsuccessful attempt increments the failure streak once",
            case.name,
          ),
          AttemptDisposition::Closed(_) => assert!(
            Actors::active_actor_view(actor_id).is_none(),
            "{} closed disposition removes active state",
            case.name,
          ),
        }

        let target_transfer = case.stimulus == StepParityStimulus::SuccessfulTask;
        let expected_bob: Balance = if target_transfer { 5 } else { 0 };
        let expected_charlie: Balance = 0;
        assert_eq!(
          native_balance(&BOB),
          bob_before + expected_bob,
          "{} committed prefix/target",
          case.name
        );
        assert_eq!(
          native_balance(&CHARLIE),
          charlie_before + expected_charlie,
          "{} suffix boundary",
          case.name
        );
        if matches!(
          case.stimulus,
          StepParityStimulus::TemporaryFailure | StepParityStimulus::PermanentFailure
        ) {
          assert_eq!(
            native_balance(&u64::MAX),
            pool_native_before,
            "{} failed task custody rollback",
            case.name
          );
        }
        let fee_delta = native_balance(&TestFeeSink::get()).saturating_sub(sink_before);
        match case.actor_type {
          ActorType::User => {
            // Fresh Opening pays Pipeline admission; a persisted Run pays only an
            // invoked target Action, since skipped/StopCycle control is prepaid.
            if !current_run
              || matches!(
                case.stimulus,
                StepParityStimulus::SuccessfulTask
                  | StepParityStimulus::TemporaryFailure
                  | StepParityStimulus::PermanentFailure
              )
            {
              assert!(
                fee_delta > 0,
                "{} invoked target Action is charged",
                case.name
              );
            } else {
              assert_eq!(fee_delta, 0, "{} target invokes no paid Action", case.name);
            }
            assert_eq!(
              fee_collections().iter().copied().sum::<Balance>(),
              fee_delta,
              "{} fee accounting",
              case.name
            );
          }
          ActorType::System => assert_eq!(fee_delta, 0, "{} System fee exemption", case.name),
        }
        assert_eq!(
          actor_before.saturating_sub(native_balance(&actor)),
          expected_bob + expected_charlie + fee_delta,
          "{} reservation, fee, and custody conservation",
          case.name,
        );
        set_temporary_dex_failure(false);
        set_fail_dex_after_input_transfer(false);
      });
    }
  }
  assert_eq!(
    fresh_rows, covered_rows,
    "every canonical row has a Fresh Opening witness"
  );
  assert_eq!(
    current_rows, covered_rows,
    "every canonical row has a persisted Run witness"
  );
  assert_eq!(
    covered_rows,
    BTreeSet::from([
      "ST-01", "ST-02", "ST-03", "ST-04", "ST-05", "ST-06", "ST-07", "ST-08", "ST-09", "ST-10",
      "ST-11", "ST-12", "ST-13"
    ]),
    "matrix inventory must remain closed against specification section 3.4",
  );
  assert_eq!(
    covered_policies.len(),
    variant_count::<StepErrorPolicy>(),
    "every StepErrorPolicy variant must be represented",
  );
  assert_eq!(
    covered_mutability.len(),
    variant_count::<Mutability>(),
    "every Mutability variant must be represented",
  );
  assert_eq!(
    covered_actor_types.len(),
    variant_count::<ActorType>(),
    "every ActorType variant must be represented",
  );
  assert_eq!(
    covered_outcome_variants.len(),
    variant_count::<StepOutcome>(),
    "every canonical StepOutcome variant must be represented",
  );
}

#[test]
fn simulation_zero_step_user_auto_close_preserves_budget_and_rollback_boundaries() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      BoundedVec::default(),
    );
    fund_native(actor_id, 100_000);
    assert_ok!(replace_auto_close(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      Some(1)
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let contract = Actors::load_actor_contract(actor_id).expect("zero-Step Contract exists");
    let root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let custody = native_balance(&sovereign_account(actor_id));
    let mut budget = ample_simulation_budget();
    budget.actor_control = Weight::zero();
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::User,
        Mutability::Mutable,
        contract.clone(),
        SimulationMode::FreshCurrentPlan,
        budget
      ),
      Err(SimulationError::ResourceDeferred)
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), root);
    let simulated = Actors::simulate_current_contract(
      actor_id,
      ActorType::User,
      Mutability::Mutable,
      contract,
      SimulationMode::FreshCurrentPlan,
      ample_simulation_budget(),
    )
    .expect("zero-Step terminal Opening simulates");
    assert_eq!(
      simulated.status,
      AttemptDisposition::Closed(CloseReason::AutoCloseNonceReached)
    );
    assert_eq!(simulated.cycle_nonce, 1);
    assert!(simulated.steps.is_empty());
    assert_eq!(simulated.cumulative_outcomes, OutcomeTotals::default());
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), root);
    assert_eq!(native_balance(&sovereign_account(actor_id)), custody);
    System::reset_events();
    Actors::execute_cycle_to_cutoff(Weight::MAX, Actors::queue_tail());
    assert!(Actors::actor_identity(actor_id).is_none());
    assert!(has_actor_event(
      |event| matches!(event, Event::ActorClosed { actor_id: id,
      reason: CloseReason::AutoCloseNonceReached } if *id == actor_id)
    ));
    assert!(has_actor_event(
      |event| matches!(event, Event::CycleSummary { actor_id: id,
      outcomes, .. } if *id == actor_id && *outcomes == OutcomeTotals::default())
    ));
  });
}

#[test]
fn simulation_terminal_ready_rejects_corrupt_fifo_span_without_cleanup() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_contract_steps(),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let contract = Actors::load_actor_contract(actor_id).expect("admitted windowed Contract");
    assert!(matches!(
      Actors::actor_control_cell(actor_id).map(|(location, _)| location),
      Some(crate::ActorControlLocation::Ready { .. })
    ));
    System::set_block_number(102);
    assert_eq!(
      active_eligibility(actor_id).terminal_reason,
      Some(CloseReason::WindowExpired)
    );
    let head = Actors::queue_head();
    crate::ActorReadyTail::<Test>::put(
      head + u64::from(<<Test as crate::Config>::MaxQueueLength as Get<u32>>::get()) + 1,
    );
    System::reset_events();
    let root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let account = sovereign_account(actor_id);
    let custody = native_balance(&account);
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        contract,
        SimulationMode::FreshCurrentPlan,
        ample_simulation_budget()
      ),
      Err(SimulationError::Classification(
        ActorClassificationError::ActorInvariant
      ))
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), root);
    Actors::execute_cycle_to_cutoff(Weight::MAX, Actors::queue_tail());
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), root);
    assert_eq!(native_balance(&account), custody);
    assert!(System::events().is_empty());
  });
}

#[test]
fn simulation_non_ready_terminal_cleanup_is_budgeted_and_custody_neutral() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_contract_steps(),
    );
    fund_native(actor_id, 100);
    let contract = Actors::load_actor_contract(actor_id).expect("windowed Contract exists");
    System::set_block_number(102);
    assert!(
      Actors::actor_hot(actor_id)
        .expect("active terminal actor")
        .queue_ticket
        .is_none()
    );
    let account = sovereign_account(actor_id);
    let custody = native_balance(&account);
    let root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let mut budget = ample_simulation_budget();
    budget.actor_control = Weight::zero();
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        contract.clone(),
        SimulationMode::FreshCurrentPlan,
        budget
      ),
      Err(SimulationError::ResourceDeferred)
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), root);
    let simulated = Actors::simulate_current_contract(
      actor_id,
      ActorType::System,
      Mutability::Mutable,
      contract,
      SimulationMode::FreshCurrentPlan,
      ample_simulation_budget(),
    )
    .expect("non-Ready terminal cleanup simulates");
    assert_eq!(
      simulated.status,
      AttemptDisposition::Closed(CloseReason::WindowExpired)
    );
    assert!(simulated.steps.is_empty());
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), root);
    assert_eq!(native_balance(&account), custody);
    assert_ok!(Actors::permissionless_sweep(
      RuntimeOrigin::signed(BOB),
      actor_id
    ));
    assert!(Actors::actor_identity(actor_id).is_none());
    assert_eq!(native_balance(&account), custody);
    assert!(has_actor_event(
      |event| matches!(event, Event::ActorClosed { actor_id: id,
      reason: CloseReason::WindowExpired } if *id == actor_id)
    ));
  });
}

#[test]
fn running_simulation_obeys_q1_and_returns_only_the_current_step() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(2),
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(5),
      }),
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(3),
      }),
    ])
    .expect("three Steps fit");
    let contract =
      system_active_contract(manual_schedule(), None, steps.clone()).expect("valid Contract");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    Actors::execute_cycle_to_cutoff(Weight::MAX, Actors::queue_tail());
    let run = Actors::actor_run_state(actor_id).expect("Opening commits only the prefix");
    assert_eq!(run.cursor, 1);
    assert!(run.eligible_at > System::block_number());
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        contract.clone(),
        SimulationMode::CurrentRun,
        ample_simulation_budget()
      ),
      Err(SimulationError::NotReady)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
    System::set_block_number(run.eligible_at);
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let balances = (
      native_balance(&BOB),
      native_balance(&CHARLIE),
      native_balance(&sovereign_account(actor_id)),
    );
    let result = Actors::simulate_current_contract(
      actor_id,
      ActorType::System,
      Mutability::Mutable,
      contract,
      SimulationMode::CurrentRun,
      ample_simulation_budget(),
    )
    .expect("eligible Running Step simulates");
    assert_eq!(result.status, AttemptDisposition::Continued);
    assert_eq!(result.start_cursor, 1);
    assert_eq!(result.run_cursor, Some(2));
    assert_eq!(
      result.steps.as_slice(),
      &[SimulationStepRecord {
        step_index: 1,
        outcome: StepOutcome::Executed
      }]
    );
    assert_eq!(result.cumulative_outcomes.executed_steps, 2);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
    assert_eq!(
      (
        native_balance(&BOB),
        native_balance(&CHARLIE),
        native_balance(&sovereign_account(actor_id))
      ),
      balances
    );
  });
}

#[test]
fn run_simulation_preserves_retry_position_and_committed_state() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let expected_contract =
      system_active_contract(manual_schedule(), None, temporary_retry_swap_plan())
        .expect("direct Actor Contract");
    let continuation_before = Actors::actor_run_state(actor_id).expect("Actor run exists");
    let actor_before = Actors::active_actor_view(actor_id).expect("actor exists");
    let events_before = frame_system::Pallet::<Test>::event_count();
    assert!(continuation_before.eligible_at > System::block_number());
    let before_due_root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        expected_contract.clone(),
        SimulationMode::CurrentRun,
        ample_simulation_budget()
      ),
      Err(SimulationError::NotReady)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      before_due_root
    );
    frame_system::Pallet::<Test>::set_block_number(continuation_before.eligible_at);
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    let result = Actors::simulate_current_contract(
      actor_id,
      ActorType::System,
      Mutability::Mutable,
      expected_contract,
      SimulationMode::CurrentRun,
      ample_simulation_budget(),
    )
    .expect("eligible Actor run simulates");

    assert_eq!(result.status, AttemptDisposition::Suspended);
    assert_eq!(result.cycle_nonce, continuation_before.cycle_nonce);
    assert_eq!(result.start_cursor, continuation_before.cursor);
    assert_eq!(result.run_cursor, Some(continuation_before.cursor));
    assert_eq!(
      result.unsuccessful_attempts_at_cursor,
      Some(
        continuation_before
          .unsuccessful_attempts_at_cursor
          .saturating_add(1)
      )
    );
    assert_eq!(result.cumulative_outcomes.failed_steps, 2);
    assert_eq!(
      result.steps.as_slice(),
      &[SimulationStepRecord {
        step_index: 0,
        outcome: StepOutcome::Failed(TaskFailure::temporary(DispatchError::Other(
          "TemporaryDexCapacity"
        ))),
      }]
    );
    assert_eq!(
      Actors::actor_run_state(actor_id).map(|state| state.encode()),
      Some(continuation_before.encode())
    );
    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(frame_system::Pallet::<Test>::event_count(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn simulation_projects_first_retry_suspension_and_rolls_back_actor_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let contract_steps = contract_steps_with_step(StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 2 },
    });
    let expected_contract = system_active_contract(manual_schedule(), None, contract_steps.clone())
      .expect("direct Actor Contract");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let actor_before = Actors::active_actor_view(actor_id).expect("actor exists");
    let events_before = frame_system::Pallet::<Test>::event_count();

    let result = Actors::simulate_current_contract(
      actor_id,
      ActorType::System,
      Mutability::Mutable,
      expected_contract,
      SimulationMode::FreshCurrentPlan,
      ample_simulation_budget(),
    )
    .expect("retry exhaustion simulates");

    assert_eq!(result.status, AttemptDisposition::Suspended);
    assert_eq!(result.run_cursor, Some(0));
    assert_eq!(result.unsuccessful_attempts_at_cursor, Some(1));
    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(frame_system::Pallet::<Test>::event_count(), events_before);
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn simulation_classification_uses_frame_authority_without_scalar_control_partitions() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = transfer_contract_steps(BOB, 10);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let expected_contract = Actors::load_actor_contract(actor_id).expect("Actor Contract exists");

    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        expected_contract,
        SimulationMode::FreshCurrentPlan,
        ample_simulation_budget(),
      )
      .err(),
      Some(SimulationError::NotReady)
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn simulation_rejects_contract_and_mode_mismatch_without_execution() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = transfer_contract_steps(BOB, 10);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps.clone());
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, contract_steps.clone())
          .expect("direct Actor Contract"),
        SimulationMode::FreshCurrentPlan,
        ample_simulation_budget(),
      )
      .err(),
      Some(SimulationError::NotReady)
    );
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let events_before = frame_system::Pallet::<Test>::event_count();

    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        ActorContract {
          trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 100,),
          ..system_active_contract(manual_schedule(), None, contract_steps.clone())
            .expect("direct Actor Contract")
        },
        SimulationMode::FreshCurrentPlan,
        ample_simulation_budget(),
      )
      .err(),
      Some(SimulationError::InvalidContract)
    );

    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        ActorContract {
          cooldown_blocks: 1,
          ..system_active_contract(manual_schedule(), None, contract_steps.clone())
            .expect("direct Actor Contract")
        },
        SimulationMode::FreshCurrentPlan,
        ample_simulation_budget(),
      )
      .err(),
      Some(SimulationError::ContractMismatch)
    );
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, contract_steps)
          .expect("direct Actor Contract"),
        SimulationMode::CurrentRun,
        ample_simulation_budget(),
      )
      .err(),
      Some(SimulationError::ModeCycleStateMismatch)
    );
    assert_eq!(frame_system::Pallet::<Test>::event_count(), events_before);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor exists")
        .cycle_nonce,
      0
    );
  });
}

#[test]
fn canonical_loader_requires_run_state_exactly_for_suspended_state() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let run_state = Actors::actor_run_state(actor_id).expect("suspended run exists");
    let (location, cell) = Actors::actor_control_cell(actor_id).expect("primary exists");
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Active(_)
    ));
    ActorRunStateStore::<Test>::remove(actor_id);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Corrupt
    ));
    ActorRunStateStore::<Test>::insert(actor_id, run_state);
    mutate_primary_control_cell(actor_id, |stored| stored.hot.cycle_state = CycleState::Idle);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Corrupt
    ));
    mutate_primary_control_cell(actor_id, |stored| *stored = cell.clone());
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Active(_)
    ));

    let contract = Actors::load_actor_contract(actor_id).expect("Contract exists");
    let admission = cell.admission.clone();
    for remove_partition in 0u8..4 {
      match remove_partition {
        0 => {
          crate::ActorControlLocators::<Test>::remove(actor_id);
          assert!(matches!(
            Actors::load_actor_state(actor_id),
            LoadedActorStateOf::Corrupt
          ));
          crate::ActorControlLocators::<Test>::insert(actor_id, location);
        }
        1 => {
          mutate_primary_control_cell(actor_id, |stored| stored.actor_id = u64::MAX);
          assert!(matches!(
            Actors::load_actor_state(actor_id),
            LoadedActorStateOf::Corrupt
          ));
          match location {
            crate::ActorControlLocation::Unsignaled => {
              crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell.clone());
            }
            crate::ActorControlLocation::Ready { ticket } => {
              crate::ActorReadyFrameChunks::<Test>::mutate(ticket / 32, |stored| {
                *stored
                  .as_mut()
                  .expect("Ready page")
                  .get_mut((ticket % 32) as usize)
                  .expect("Ready slot") = Some(cell.clone());
              });
            }
            crate::ActorControlLocation::Waiting { key, page, slot } => {
              crate::ActorWaitingFrameChunks::<Test>::mutate((key, page), |stored| {
                *stored
                  .as_mut()
                  .expect("Waiting page")
                  .entries
                  .get_mut(usize::from(slot))
                  .expect("Waiting slot") = Some(crate::ActorWaitingEntry::Primary(cell.clone()));
              });
            }
          }
        }
        2 => {
          assert!(Actors::remove_admitted_contract_geometry(actor_id).is_some());
          assert!(matches!(
            Actors::load_actor_state(actor_id),
            LoadedActorStateOf::Corrupt
          ));
          assert!(Actors::insert_admitted_contract_geometry(
            actor_id, &contract, &admission
          ));
        }
        3 => {
          let funding = ActorFunding::<Test>::take(actor_id).expect("funding exists");
          assert!(matches!(
            Actors::load_actor_state(actor_id),
            LoadedActorStateOf::Corrupt
          ));
          ActorFunding::<Test>::insert(actor_id, funding);
        }
        _ => unreachable!(),
      }
      assert!(matches!(
        Actors::load_actor_state(actor_id),
        LoadedActorStateOf::Active(_)
      ));
    }
  });
}

#[test]
fn eligibility_projection_reports_suspended_retry_then_ready_at_attempt_block() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingRetry(2)
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::Ready
    );
  });
}
