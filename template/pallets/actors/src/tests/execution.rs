use super::*;

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
      assert!(ActorContracts::<Test>::get(actor_id).is_none());
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
fn continuation_schema_round_trips_retry_position_and_typed_snapshot_surfaces() {
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
    let continuation = RuntimeContinuationState {
      cursor: 0,
      unsuccessful_attempts_at_cursor: 1,
      last_attempt_block: 1,
      opening_snapshot,
      opening_predicate_results: Default::default(),
      funding_snapshot: Default::default(),
      cumulative_outcomes: OutcomeTotals {
        executed_steps: 1,
        ..Default::default()
      },
    };
    let encoded = continuation.encode();
    let decoded =
      RuntimeContinuationState::decode(&mut &encoded[..]).expect("continuation decodes");
    assert_eq!(decoded.cursor, 0);
    assert_eq!(decoded.unsuccessful_attempts_at_cursor, 1);
    assert_eq!(decoded.last_attempt_block, 1);
    assert_eq!(decoded.opening_snapshot.len(), 2);
    assert_eq!(decoded.cumulative_outcomes.executed_steps, 1);

    ContinuationStateStore::<Test>::insert(actor_id, continuation);
    ActorHot::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("active actor").cycle_state = CycleState::Suspended;
    });
    ActorIdentities::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("actor identity").cycle_nonce = 1;
    });
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("continuation exists")
        .unsuccessful_attempts_at_cursor,
      1
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn ordinary_one_attempt_run_keeps_continuation_sparse() {
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
    assert!(Actors::continuation_state(actor_id).is_none());
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
    assert!(Actors::continuation_state(actor_id).is_none());
  });
}

#[test]
fn continuation_attempt_missing_state_fails_without_panicking_or_mutating() {
  new_test_ext().execute_with(|| {
    let mut plan = transfer_contract_steps(BOB, 1);
    plan[0].on_error = RETRY_LATER;
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    ContinuationStateStore::<Test>::insert(
      actor_id,
      RuntimeContinuationState {
        cursor: 0,
        unsuccessful_attempts_at_cursor: 1,
        last_attempt_block: 1,
        opening_snapshot: Default::default(),
        opening_predicate_results: Default::default(),
        funding_snapshot: Default::default(),
        cumulative_outcomes: Default::default(),
      },
    );
    ActorHot::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("actor exists").cycle_state = CycleState::Suspended;
    });
    ActorIdentities::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("identity exists").cycle_nonce = 1;
    });
    let instance = Actors::active_actor_view(actor_id).expect("coherent continuation exists");
    ContinuationStateStore::<Test>::remove(actor_id);
    let hot_before = Actors::actor_hot(actor_id).expect("hot state remains");
    System::reset_events();

    let _ = Actors::execute_single_cycle(actor_id, instance, 1);

    assert_eq!(Actors::actor_hot(actor_id), Some(hot_before));
    assert!(System::events().is_empty());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn continuation_try_state_rejects_marker_and_cursor_drift() {
  new_test_ext().execute_with(|| {
    let mut plan = transfer_contract_steps(BOB, 1);
    plan[0].on_error = RETRY_LATER;
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    ContinuationStateStore::<Test>::insert(
      actor_id,
      RuntimeContinuationState {
        cursor: 0,
        unsuccessful_attempts_at_cursor: 1,
        last_attempt_block: 1,
        opening_snapshot: Default::default(),
        opening_predicate_results: Default::default(),
        funding_snapshot: Default::default(),
        cumulative_outcomes: Default::default(),
      },
    );
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    ActorHot::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("actor exists").cycle_state = CycleState::Suspended;
    });
    ActorIdentities::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("identity exists").cycle_nonce = 1;
    });
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    ContinuationStateStore::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("continuation exists")
        .opening_snapshot
        .try_insert(OpeningSurface::PreservableAsset(TestAsset::Native), 10)
        .expect("snapshot entry fits");
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    ContinuationStateStore::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("continuation exists")
        .opening_snapshot
        .clear();
    });
    ContinuationStateStore::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("continuation exists").cursor = 1;
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    ContinuationStateStore::<Test>::remove(actor_id);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn user_dormant_fund_then_activate_is_the_unfunded_lifecycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // The unfunded lifecycle is create Dormant -> fund the deterministic sovereign ->
    // activate; activation of an unfunded sovereign fails InsufficientBalance.
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      None,
    ));
    let actor_id = Actors::next_actor_id() - 1;
    let plan = transfer_contract_steps(BOB, 1);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_noop!(
      Actors::activate_actor(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        user_active_contract(manual_schedule(), None, plan.clone()).expect("direct Actor Contract"),
      ),
      Error::<Test>::InsufficientBalance
    );
    assert!(Actors::actor_identities(actor_id).is_some());
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(Actors::active_actor_count(), 0);

    // Funding the deterministic sovereign account admits activation.
    fund_native_raw(
      &Actors::sovereign_account_id(&ALICE, 0),
      user_prefunding_requirement(&plan),
    );
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      user_active_contract(manual_schedule(), None, plan).expect("direct Actor Contract"),
    ));
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert_eq!(Actors::active_actor_count(), 1);
  });
}

#[test]
fn generated_continuation_weights_cover_distinct_storage_paths() {
  new_test_ext().execute_with(|| {
    let suspend_min = <TestWeightInfo as crate::WeightInfo>::continuation_suspend(0);
    let suspend_max = <TestWeightInfo as crate::WeightInfo>::continuation_suspend(20);
    assert_eq!(suspend_min, Weight::from_parts(27_920_348, 4_178));
    assert_eq!(suspend_max, Weight::from_parts(28_668_868, 4_178));
    assert_eq!(
      <TestWeightInfo as crate::WeightInfo>::continuation_retry(),
      Weight::from_parts(22_070_000, 4_266)
    );
    assert_eq!(
      <TestWeightInfo as crate::WeightInfo>::continuation_complete(),
      Weight::from_parts(18_019_000, 4_030)
    );
    assert_eq!(
      <TestWeightInfo as crate::WeightInfo>::continuation_cancel(),
      Weight::from_parts(56_782_000, 8_120)
    );
    let suffix_min = <TestWeightInfo as crate::WeightInfo>::continuation_suffix_admission(1);
    let suffix_max = <TestWeightInfo as crate::WeightInfo>::continuation_suffix_admission(10);
    assert_eq!(suffix_min, Weight::from_parts(1_439_006, 0));
    assert_eq!(suffix_max, Weight::from_parts(1_442_894, 0));
    assert!(suffix_min.ref_time() < suffix_max.ref_time());
    assert_eq!(suffix_min.proof_size(), suffix_max.proof_size());
  });
}

#[test]
fn generated_predicate_weight_scales_and_chunks_opening_plus_step_visits() {
  use crate::WeightInfo;

  let zero = <Test as crate::Config>::WeightInfo::predicate_set_evaluation(0);
  let one = <Test as crate::Config>::WeightInfo::predicate_set_evaluation(1);
  let maximum = <Test as crate::Config>::WeightInfo::predicate_set_evaluation(4);
  assert_eq!(zero, Weight::zero());
  assert!(one.ref_time() > 0 && one.proof_size() > 0);
  assert!(maximum.ref_time() >= one.ref_time());
  assert!(maximum.proof_size() >= one.proof_size());
  let doubled = Actors::predicate_evaluation_weight(8);
  assert_eq!(doubled, maximum.saturating_mul(2));
  assert!(doubled.ref_time() > maximum.ref_time());
  assert!(doubled.proof_size() > maximum.proof_size());
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

#[test]
fn continuation_control_placement_failures_roll_back_exactly() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    crate::NextQueueTicket::<Test>::put(u64::MAX);
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
    assert!(Actors::continuation_state(actor_id).is_some());
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
    crate::NextQueueTicket::<Test>::put(u64::MAX);
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
    assert!(Actors::continuation_state(actor_id).is_some());
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
    crate::NextQueueTicket::<Test>::put(u64::MAX);
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();

    assert_noop!(
      Actors::cancel_continuation(RuntimeOrigin::root(), actor_id),
      Error::<Test>::QueueTicketExhausted
    );
    assert!(Actors::continuation_state(actor_id).is_some());
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
    ActorHot::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("active actor")
        .unsuccessful_attempt_streak = 1;
    });
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
    assert!(Actors::continuation_state(actor_id).is_none());
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
    let first_continuation = Actors::continuation_state(actor_id).expect("continuation exists");
    assert_eq!(first.cycle_state, CycleState::Suspended);
    assert_eq!(first.cycle_nonce, 1);
    assert_eq!(first.unsuccessful_attempt_streak, 1);
    assert_eq!(first_continuation.cursor, 1);
    assert_eq!(first_continuation.unsuccessful_attempts_at_cursor, 1);
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

    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    let second_continuation = Actors::continuation_state(actor_id).expect("continuation remains");
    assert_eq!(second_continuation.cursor, 1);
    assert_eq!(second_continuation.unsuccessful_attempts_at_cursor, 2);
    assert_eq!(second_continuation.cumulative_outcomes.executed_steps, 1);
    assert_eq!(second_continuation.cumulative_outcomes.failed_steps, 2);
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(native_balance(&CHARLIE), charlie_before);

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(4);
    run_idle(Weight::MAX);

    let completed = Actors::active_actor_view(actor_id).expect("completed actor remains");
    assert_eq!(completed.cycle_state, CycleState::Idle);
    assert_eq!(completed.cycle_nonce, 1);
    assert_eq!(completed.unsuccessful_attempt_streak, 0);
    assert!(Actors::continuation_state(actor_id).is_none());
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
    assert!(Actors::continuation_state(actor_id).is_none());
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

    assert!(Actors::continuation_state(actor_id).is_some());
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

    assert!(Actors::continuation_state(actor_id).is_none());
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
    assert!(Actors::continuation_state(actor_id).is_some());
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
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    assert_ok!(replace_auto_close(
      RuntimeOrigin::root(),
      actor_id,
      Some(1),
    ));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    frame_system::Pallet::<Test>::reset_events();

    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
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
  });
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
      Actors::continuation_state(actor_id)
        .expect("suspended")
        .cursor,
      1
    );
    assert_eq!(native_balance(&BOB), bob_before + 10);

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);

    let actor = Actors::active_actor_view(actor_id).expect("actor remains active");
    assert_eq!(actor.cycle_state, CycleState::Idle);
    assert_eq!(actor.unsuccessful_attempt_streak, 0);
    assert!(Actors::continuation_state(actor_id).is_none());
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
    assert!(Actors::continuation_state(actor_id).is_none());
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
      Actors::continuation_state(actor_id)
        .expect("first unsuccessful attempt persists")
        .unsuccessful_attempts_at_cursor,
      1
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::continuation_state(actor_id)
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

    frame_system::Pallet::<Test>::set_block_number(4);
    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(Actors::continuation_state(actor_id).is_none());
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
    let first = Actors::continuation_state(actor_id).expect("first cursor suspends");
    assert_eq!(
      (first.cursor, first.unsuccessful_attempts_at_cursor),
      (0, 1)
    );

    set_temporary_dex_failure(false);
    set_temporary_add_liquidity_failure(true);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    let advanced = Actors::continuation_state(actor_id).expect("later cursor suspends");
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
    assert!(Actors::continuation_state(actor_id).is_none());
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
  });
}

#[test]
fn reached_global_failure_cutoff_closes_before_another_attempt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, temporary_retry_swap_plan());
    fund_native(actor_id, 100);
    let threshold = <Test as crate::Config>::MaxConsecutiveFailures::get();
    ActorHot::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("active actor")
        .unsuccessful_attempt_streak = threshold;
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
fn global_failure_limit_can_close_before_local_retry_limit() {
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
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ConsecutiveFailures,
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
      Actors::continuation_state(actor_id)
        .expect("attempt zero")
        .cursor,
      0
    );
    assert_eq!(native_balance(&actor), balance_before);

    for (due, expected_attempt, next_due) in [(2, 1, 4), (4, 2, 8), (8, 3, 16), (16, 4, 24)] {
      frame_system::Pallet::<Test>::set_block_number(due - 1);
      run_idle(Weight::MAX);
      assert_eq!(
        Actors::continuation_state(actor_id)
          .expect("not eligible before due block")
          .unsuccessful_attempts_at_cursor,
        expected_attempt
      );
      frame_system::Pallet::<Test>::set_block_number(due);
      run_idle(Weight::MAX);
      let continuation =
        Actors::continuation_state(actor_id).expect("temporary failure resuspends");
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
      Actors::continuation_state(actor_id)
        .expect("capped delay holds")
        .unsuccessful_attempts_at_cursor,
      5
    );
    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(24);
    run_idle(Weight::MAX);
    assert!(Actors::continuation_state(actor_id).is_none());
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
fn continuation_weight_deferral_does_not_admit_attempt() {
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
    let before = Actors::continuation_state(actor_id).expect("suspended");
    let ticket_before = Actors::actor_hot(actor_id)
      .expect("suspended actor")
      .queue_ticket;

    frame_system::Pallet::<Test>::set_block_number(2);
    let instance = Actors::active_actor_view(actor_id).expect("suspended actor");
    let queue_weight = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1)
      .saturating_add(Actors::scheduler_actor_probe_weight_upper())
      .saturating_add(
        <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
          .max(<<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page()),
      );
    let proof_limit = queue_weight
      .proof_size()
      .saturating_add(Actors::attempt_weight_upper_bound(&instance, 1).proof_size())
      .saturating_sub(1);
    Actors::execute_cycle(Weight::from_parts(u64::MAX, proof_limit));

    let after = Actors::continuation_state(actor_id).expect("deferral preserves continuation");
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
    assert_eq!(scheduled_wakeup_block(actor_id), Some(102));

    frame_system::Pallet::<Test>::set_block_number(102);
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(Actors::continuation_state(actor_id).is_none());
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

#[test]
fn user_retry_admits_and_charges_only_the_unresolved_suffix() {
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
    let actor_id = create_user_with(ALICE, Mutability::Mutable, manual_schedule(), None, plan);
    fund_native(actor_id, 1_000_000_000_000_000_000);
    let sink = TestFeeSink::get();
    let sink_before = native_balance(&sink);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&sink) - sink_before, prefix_fee + retry_fee);
    let suspended = Actors::active_actor_view(actor_id).expect("suspended User actor");
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("continuation")
        .cursor,
      1
    );
    let retry_weight = Actors::attempt_weight_upper_bound(&suspended, 1);
    let full_weight = Actors::attempt_weight_upper_bound(&suspended, 0);
    assert!(retry_weight.ref_time() < full_weight.ref_time());
    assert!(retry_weight.proof_size() < full_weight.proof_size());

    frame_system::Pallet::<Test>::set_block_number(2);
    let retry_budget = Actors::scheduler_admission_overhead()
      .saturating_add(retry_weight)
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::scheduler_on_idle_base())
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::materialization_coordinator_base())
      .saturating_add(
        <TestWeightInfo as crate::WeightInfo>::scheduler_wakeup_cursor_worker_future()
          .saturating_mul(2),
      )
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::crossing_worker_base())
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::observation_fanout_base());
    run_idle(retry_budget);
    assert_eq!(
      native_balance(&sink) - sink_before,
      prefix_fee + retry_fee.saturating_mul(2)
    );
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("retry remains")
        .unsuccessful_attempts_at_cursor,
      2
    );

    frame_system::Pallet::<Test>::set_block_number(4);
    set_temporary_dex_failure(false);
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&sink) - sink_before,
      prefix_fee + retry_fee.saturating_mul(3) + tail_fee
    );
    assert!(Actors::continuation_state(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
  });
}

#[test]
fn continuation_snapshot_is_trimmed_frozen_and_capacity_checked_live() {
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

    let continuation = Actors::continuation_state(actor_id).expect("suspended");
    assert_eq!(continuation.cursor, 1);
    assert_eq!(continuation.opening_snapshot.len(), 2);
    let suspended = Actors::active_actor_view(actor_id).expect("suspended actor");
    let retry_weight = Actors::attempt_weight_upper_bound(&suspended, continuation.cursor as usize);
    let full_weight = Actors::attempt_weight_upper_bound(&suspended, 0);
    assert!(retry_weight.ref_time() < full_weight.ref_time());
    assert!(retry_weight.proof_size() < full_weight.proof_size());
    assert!(
      !continuation
        .opening_snapshot
        .contains_key(&OpeningSurface::PreservableAsset(asset_a))
    );
    assert_eq!(
      continuation
        .opening_snapshot
        .get(&OpeningSurface::PreservableAsset(asset_b)),
      Some(&99)
    );
    assert_eq!(asset_balance(&BOB, asset_a), 9);

    assert_ok!(MockAssetOps::transfer(&actor, &BOB, asset_b, 20));
    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    let after_capacity_failure = Actors::continuation_state(actor_id).expect("still suspended");
    assert_eq!(after_capacity_failure.cursor, 1);
    assert_eq!(after_capacity_failure.unsuccessful_attempts_at_cursor, 2);
    assert_eq!(
      after_capacity_failure
        .opening_snapshot
        .get(&OpeningSurface::PreservableAsset(asset_b)),
      Some(&99)
    );

    set_asset_balance(&actor, asset_b, 100);
    frame_system::Pallet::<Test>::set_block_number(4);
    run_idle(Weight::MAX);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert_eq!(asset_balance(&actor, asset_b), 82);
    assert_eq!(asset_balance(&CHARLIE, asset_c), 9);
    assert_eq!(asset_balance(&BOB, asset_a), 9);
  });
}

#[test]
fn maximal_continuation_snapshot_stays_bounded_to_unresolved_surfaces() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut steps = Vec::new();
    for index in 0..8u32 {
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
    for index in 0..16u32 {
      set_asset_balance(&actor, TestAsset::Local(100 + index), 100);
    }
    set_temporary_add_liquidity_failure(true);
    signal_percentage_trigger(actor_id, TestAsset::Local(100));
    run_idle(Weight::MAX);

    let continuation = Actors::continuation_state(actor_id).expect("maximal continuation");
    assert_eq!(continuation.cursor, 0);
    assert_eq!(
      continuation.opening_snapshot.len() as u32,
      <<Test as crate::Config>::MaxOpeningSnapshotEntries as Get<u32>>::get()
    );

    set_temporary_add_liquidity_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(Actors::continuation_state(actor_id).is_none());
  });
}

#[test]
fn continuation_attempts_have_unique_chain_coordinates_without_the_stored_ordinal() {
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
    assert!(matches!(opening[1], Event::CycleStarted { actor_id: id, cycle_nonce: 1 } if id == actor_id));
    assert!(matches!(opening[2], Event::StepFailed { actor_id: id, cycle_nonce: 1, step_index: 0, .. } if id == actor_id));
    assert!(matches!(opening[3], Event::CycleSuspended { actor_id: id, cycle_nonce: 1, cursor: 0, reason: SuspensionReason::Temporary, .. } if id == actor_id));

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
fn completion_policy_only_replacement_preserves_continuation() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let before = Actors::active_actor_view(actor_id).expect("suspended actor");
    let continuation_before = Actors::continuation_state(actor_id).expect("suspended continuation");
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
    assert_eq!(after.cycle_state, CycleState::Suspended);
    assert_eq!(
      Actors::continuation_state(actor_id).map(|state| state.encode()),
      Some(continuation_before.encode())
    );
    let events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(events.len(), 1);
    assert!(matches!(
      events.first(),
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
    let last_actor_event = frame_system::Pallet::<Test>::events()
      .into_iter()
      .rev()
      .find_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .expect("Actors event stream must not be empty");
    assert!(matches!(
      last_actor_event,
      Event::CycleSummary { actor_id: id, cycle_nonce: 1, .. } if id == actor_id
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
    ActorHot::<Test>::mutate(continue_id, |maybe| {
      maybe.as_mut().expect("actor hot state exists").unsuccessful_attempt_streak = 2;
    });
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
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      continue_id
    ));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(continue_id).is_none());
    let continue_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(continue_events.len(), 5);
    assert!(matches!(continue_events[0], Event::CycleStarted { actor_id, .. } if actor_id == continue_id));
    assert!(matches!(continue_events[1], Event::StepFailed { actor_id, step_index: 0, .. } if actor_id == continue_id));
    assert!(matches!(continue_events[2], Event::StepFailed { actor_id, step_index: 1, .. } if actor_id == continue_id));
    assert!(matches!(continue_events[3], Event::CycleSummary { actor_id, result: CycleResult::Completed, outcomes: OutcomeTotals { failed_steps: 2, .. }, .. } if actor_id == continue_id));
    assert!(matches!(continue_events[4], Event::ActorClosed { actor_id, reason: CloseReason::AutoCloseNonceReached } if actor_id == continue_id));
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
    run_idle(Weight::MAX);
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
    run_idle(Weight::MAX);
    let abort_instance = Actors::active_actor_view(abort_id).expect("aborted actor remains active");
    assert_eq!(abort_instance.unsuccessful_attempt_streak, 1);
    let abort_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(abort_events.len(), 3);
    assert!(matches!(abort_events[0], Event::CycleStarted { actor_id, .. } if actor_id == abort_id));
    assert!(matches!(abort_events[1], Event::StepFailed { actor_id, step_index: 0, .. } if actor_id == abort_id));
    assert!(matches!(abort_events[2], Event::CycleSummary { actor_id, result: CycleResult::Failed, outcomes: OutcomeTotals { executed_steps: 0, failed_steps: 1, .. }, .. } if actor_id == abort_id));
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
fn empty_contract_steps_rejected() {
  new_test_ext().execute_with(|| {
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, BoundedVec::default()),
      ),
      Error::<Test>::EmptyContractSteps
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
      8
    );
    let steps: Vec<_> = (0..9)
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
      (0..8)
        .map(|i| {
          make_step(Task::Transfer {
            to: BOB,
            asset: TestAsset::Native,
            amount: AmountResolution::Fixed(i + 1),
          })
        })
        .collect::<Vec<_>>(),
    )
    .expect("eight steps fit the shared encoded bound");
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
    ActorIdentities::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("system Actors identity exists").cycle_nonce = u64::MAX - 1;
    });
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
    ActorIdentities::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("user Actors identity exists")
        .cycle_nonce = u64::MAX;
    });
    let expected_contract = ActorContracts::<Test>::get(actor_id).expect("contract exists");
    let simulation = Actors::simulate_current_contract(
      actor_id,
      ActorType::User,
      Mutability::Mutable,
      expected_contract,
      SimulationMode::FreshCurrentPlan,
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
    ActorIdentities::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("system Actors identity exists")
        .cycle_nonce = u64::MAX;
    });
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
    let continuation = |last_attempt_block| RuntimeContinuationState {
      cursor: 0,
      unsuccessful_attempts_at_cursor: 1,
      last_attempt_block,
      opening_snapshot: Default::default(),
      opening_predicate_results: Default::default(),
      funding_snapshot: Default::default(),
      cumulative_outcomes: Default::default(),
    };
    ContinuationStateStore::<Test>::insert(actor_id, continuation(u64::MAX - 1));
    assert_eq!(Actors::retry_eligible_at(actor_id, &instance), Ok(u64::MAX));

    ContinuationStateStore::<Test>::insert(actor_id, continuation(u64::MAX));
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
fn opening_predicate_result_is_reused_by_continuation() {
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
    let continuation = Actors::continuation_state(actor_id).expect("continuation exists");
    assert_eq!(
      continuation.opening_predicate_results.as_slice(),
      &[Ok(true)]
    );
    assert_ok!(MockAssetOps::transfer(&actor, &BOB, TestAsset::Native, 60));
    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(Actors::continuation_state(actor_id).is_none());
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
    assert!(Actors::continuation_state(actor_id).is_none());
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
      Actors::continuation_state(actor_id)
        .expect("suspended")
        .cursor,
      0
    );

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(Actors::continuation_state(actor_id).is_none());
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
    for block in 2..7 {
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
    // Step 6-7: Drain native below MinUserBalance to trigger sweep close
    let actor_native = native_balance(&actor);
    let min_user = <Test as crate::Config>::MinUserBalance::get();
    let slash_amount = actor_native.saturating_sub(min_user / 2);
    let _ = <Balances as Currency<AccountId>>::slash(&actor, slash_amount);
    assert!(
      native_balance(&actor) < min_user,
      "Actor balance must be below MinUserBalance after slash"
    );
    // Sweep cursor iterates MaxSweepBatch=3 IDs per block; run enough blocks
    for block in 30..50 {
      frame_system::Pallet::<Test>::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::MAX);
      if Actors::active_actor_view(actor_id).is_none() {
        break;
      }
    }
    assert!(
      Actors::active_actor_view(actor_id).is_none(),
      "User Actors must be destroyed by sweep when native < MinUserBalance"
    );
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
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
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
          observed_at: 11,
        },
      );
      setup_temporary_retry_pool();
      set_temporary_dex_failure(case.stimulus == StepParityStimulus::TemporaryFailure);
      set_fail_dex_after_input_transfer(case.stimulus == StepParityStimulus::PermanentFailure);
      let steps = parity_contract_steps(*case);
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
        if case.stimulus == StepParityStimulus::FundingUnavailable {
          8
        } else {
          100
        },
      );
      if case.actor_type == ActorType::System && case.mutability == Mutability::Immutable {
        frame_system::Pallet::<Test>::set_block_number(11);
        ActorHot::<Test>::mutate(actor_id, |maybe| {
          maybe
            .as_mut()
            .expect("parity actor hot state exists")
            .pending_signal = true;
        });
        Actors::enqueue(actor_id).expect("parity actor enqueues");
      } else if case.stimulus == StepParityStimulus::PredicateError {
        ActorHot::<Test>::mutate(actor_id, |maybe| {
          maybe
            .as_mut()
            .expect("parity actor hot state exists")
            .pending_signal = true;
        });
        Actors::enqueue(actor_id).expect("parity actor enqueues");
      } else {
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          actor_id
        ));
      }
      if case.bound == StepParityBound::LocalReached {
        run_idle(Weight::MAX);
        assert_eq!(
          Actors::continuation_state(actor_id).map(|state| state.cursor),
          Some(1)
        );
        frame_system::Pallet::<Test>::set_block_number(11);
      }
      let mode = if case.bound == StepParityBound::LocalReached {
        SimulationMode::CurrentContinuation
      } else {
        SimulationMode::FreshCurrentPlan
      };
      System::reset_events();
      let actor = sovereign_account(actor_id);
      let streak_before = Actors::active_actor_view(actor_id)
        .expect("matrix actor remains active before production")
        .unsuccessful_attempt_streak;
      let before = (
        Actors::active_actor_view(actor_id).map(|view| view.encode()),
        Actors::continuation_state(actor_id).map(|state| state.encode()),
        native_balance(&actor),
        native_balance(&BOB),
        native_balance(&CHARLIE),
        native_balance(&TestFeeSink::get()),
        native_balance(&u64::MAX),
        asset_balance(&actor, TestAsset::Local(77)),
        asset_balance(&u64::MAX, TestAsset::Local(77)),
        System::events(),
      );
      let simulation = Actors::simulate_current_contract(
        actor_id,
        case.actor_type,
        case.mutability,
        expected_contract,
        mode,
      )
      .unwrap_or_else(|error| panic!("{} simulation failed: {error:?}", case.name));
      let after_simulation = (
        Actors::active_actor_view(actor_id).map(|view| view.encode()),
        Actors::continuation_state(actor_id).map(|state| state.encode()),
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
        parity_expected_steps(*case),
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
      run_idle(Weight::MAX);
      let (status, outcomes, cursor, attempts) = observed_attempt_projection(actor_id);
      assert_eq!(status, simulation.status, "{} disposition", case.name);
      assert_eq!(
        outcomes, simulation.cumulative_outcomes,
        "{} outcomes",
        case.name
      );
      assert_eq!(
        cursor, simulation.continuation_cursor,
        "{} cursor",
        case.name
      );
      assert_eq!(
        attempts, simulation.unsuccessful_attempts_at_cursor,
        "{} local attempts",
        case.name
      );
      match status {
        AttemptDisposition::Completed => assert_eq!(
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

      let advances = parity_advances(*case);
      let prefix_runs = case.bound != StepParityBound::LocalReached;
      let target_transfer = case.stimulus == StepParityStimulus::SuccessfulTask;
      let expected_bob: Balance =
        (if prefix_runs { 2 } else { 0 }) + (if target_transfer { 5 } else { 0 });
      let expected_charlie: Balance = if advances { 3 } else { 0 };
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
          assert!(
            fee_delta > 0,
            "{} must charge its committed attempt",
            case.name
          );
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
fn continuation_simulation_preserves_retry_position_and_committed_state() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let expected_contract =
      system_active_contract(manual_schedule(), None, temporary_retry_swap_plan())
        .expect("direct Actor Contract");
    let continuation_before = Actors::continuation_state(actor_id).expect("continuation exists");
    let actor_before = Actors::active_actor_view(actor_id).expect("actor exists");
    let events_before = frame_system::Pallet::<Test>::event_count();
    frame_system::Pallet::<Test>::set_block_number(2);

    let result = Actors::simulate_current_contract(
      actor_id,
      ActorType::System,
      Mutability::Mutable,
      expected_contract,
      SimulationMode::CurrentContinuation,
    )
    .expect("eligible continuation simulates");

    assert_eq!(result.status, AttemptDisposition::Suspended);
    assert_eq!(result.cycle_nonce, actor_before.cycle_nonce);
    assert_eq!(result.start_cursor, continuation_before.cursor);
    assert_eq!(result.continuation_cursor, Some(continuation_before.cursor));
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
      Actors::continuation_state(actor_id).map(|state| state.encode()),
      Some(continuation_before.encode())
    );
    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(frame_system::Pallet::<Test>::event_count(), events_before);
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
    )
    .expect("retry exhaustion simulates");

    assert_eq!(result.status, AttemptDisposition::Suspended);
    assert_eq!(result.continuation_cursor, Some(0));
    assert_eq!(result.unsuccessful_attempts_at_cursor, Some(1));
    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(frame_system::Pallet::<Test>::event_count(), events_before);
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
        SimulationMode::CurrentContinuation,
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
fn canonical_loader_requires_continuation_exactly_for_suspended_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut plan = inert_contract_steps();
    plan[0].on_error = RETRY_LATER;
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    let continuation = RuntimeContinuationState {
      cursor: 0,
      unsuccessful_attempts_at_cursor: 1,
      last_attempt_block: 1,
      opening_snapshot: Default::default(),
      opening_predicate_results: Default::default(),
      funding_snapshot: Default::default(),
      cumulative_outcomes: Default::default(),
    };

    ContinuationStateStore::<Test>::insert(actor_id, continuation.clone());
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Corrupt
    ));
    ActorHot::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("active actor").cycle_state = CycleState::Suspended;
    });
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Active(_)
    ));
    ContinuationStateStore::<Test>::remove(actor_id);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Corrupt
    ));

    ContinuationStateStore::<Test>::insert(actor_id, continuation);
    for remove_partition in 0u8..4 {
      let identity = ActorIdentities::<Test>::take(actor_id);
      let hot = ActorHot::<Test>::take(actor_id);
      let contract = ActorContracts::<Test>::take(actor_id);
      let funding = ActorFunding::<Test>::take(actor_id);
      if remove_partition != 0 {
        ActorIdentities::<Test>::insert(actor_id, identity.clone().unwrap());
      }
      if remove_partition != 1 {
        ActorHot::<Test>::insert(actor_id, hot.clone().unwrap());
      }
      if remove_partition != 2 {
        ActorContracts::<Test>::insert(actor_id, contract.clone().unwrap());
      }
      if remove_partition != 3 {
        ActorFunding::<Test>::insert(actor_id, funding.clone().unwrap());
      }
      assert!(matches!(
        Actors::load_actor_state(actor_id),
        LoadedActorStateOf::Corrupt
      ));
      ActorIdentities::<Test>::remove(actor_id);
      ActorHot::<Test>::remove(actor_id);
      ActorContracts::<Test>::remove(actor_id);
      ActorFunding::<Test>::remove(actor_id);
      ActorIdentities::<Test>::insert(actor_id, identity.unwrap());
      ActorHot::<Test>::insert(actor_id, hot.unwrap());
      ActorContracts::<Test>::insert(actor_id, contract.unwrap());
      ActorFunding::<Test>::insert(actor_id, funding.unwrap());
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
