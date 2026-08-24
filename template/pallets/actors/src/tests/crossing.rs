use super::*;
use crate::scheduler::ActivationOutcome;
use crate::weights::WeightInfo as _;

#[test]
fn observation_crossing_semantics_are_exact_and_hysteretic() {
  let rising = ObservationCrossing {
    feed: 7u32,
    direction: CrossingDirection::Rising,
    threshold: 100,
    rearm_threshold: 80,
  };
  let falling = ObservationCrossing {
    feed: 7u32,
    direction: CrossingDirection::Falling,
    threshold: 80,
    rearm_threshold: 100,
  };
  assert!(rising.has_valid_hysteresis());
  assert!(falling.has_valid_hysteresis());
  assert!(
    !ObservationCrossing {
      feed: 7u32,
      direction: CrossingDirection::Rising,
      threshold: 100,
      rearm_threshold: 100,
    }
    .has_valid_hysteresis()
  );
  assert!(
    !ObservationCrossing {
      feed: 7u32,
      direction: CrossingDirection::Falling,
      threshold: 100,
      rearm_threshold: 100,
    }
    .has_valid_hysteresis()
  );

  assert_eq!(rising.initial_phase(99), CrossingPhase::Armed);
  assert_eq!(rising.initial_phase(100), CrossingPhase::WaitingForRearm);
  assert_eq!(falling.initial_phase(81), CrossingPhase::Armed);
  assert_eq!(falling.initial_phase(80), CrossingPhase::WaitingForRearm);

  let cases = [
    (
      &rising,
      CrossingPhase::Armed,
      99,
      100,
      CrossingTransition::Fire,
    ),
    (
      &rising,
      CrossingPhase::Armed,
      100,
      101,
      CrossingTransition::None,
    ),
    (
      &rising,
      CrossingPhase::WaitingForRearm,
      81,
      80,
      CrossingTransition::Rearm,
    ),
    (
      &rising,
      CrossingPhase::WaitingForRearm,
      80,
      79,
      CrossingTransition::None,
    ),
    (
      &falling,
      CrossingPhase::Armed,
      81,
      80,
      CrossingTransition::Fire,
    ),
    (
      &falling,
      CrossingPhase::Armed,
      80,
      79,
      CrossingTransition::None,
    ),
    (
      &falling,
      CrossingPhase::WaitingForRearm,
      99,
      100,
      CrossingTransition::Rearm,
    ),
    (
      &falling,
      CrossingPhase::WaitingForRearm,
      100,
      101,
      CrossingTransition::None,
    ),
  ];
  for (crossing, phase, previous, current, expected) in cases {
    assert_eq!(crossing.transition(phase, previous, current), expected);
    assert_eq!(
      crossing.transition(phase, previous, previous),
      CrossingTransition::None
    );
  }
}

#[test]
fn observation_crossing_fire_charges_before_readiness() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      transfer_contract_steps(BOB, 1),
    );
    {
      let installed = CrossingMemberships::<Test>::get(actor_id).expect("Crossing membership");
      let installed_member = CrossingMemberPages::<Test>::get(installed.key, installed.page)
        .and_then(|page| page.entries.get(installed.offset as usize).copied())
        .expect("installed Crossing member");
      assert_eq!(installed_member.counterpart_threshold, 80);
      let compact = Actors::load_crossing_idle_activation_state(actor_id, 7)
        .expect("Idle Crossing activation authority");
      assert!(compact.run_head.is_none());
      assert!(compact.loaded_step.is_none());
    }

    let sovereign = sovereign_account(actor_id);
    let sovereign_before = native_balance(&sovereign);
    let sink_before = native_balance(&TestFeeSink::get());
    clear_fee_collections();

    assert_ok!(Actors::note_observation_transition_with_provenance(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
      crate::TriggerCauseProvenance::ExternalPhase,
    ));
    assert_eq!(
      Actors::crossing_transition_queue(7)
        .and_then(|queue| queue.first().copied())
        .map(|transition| (transition.cause_provenance, transition.cause_block)),
      Some((crate::TriggerCauseProvenance::ExternalPhase, 1))
    );
    drain_crossing_work();

    let fee = observation_crossing_trigger_fee();
    assert_eq!(fee_collections(), vec![fee]);
    assert_eq!(native_balance(&sovereign), sovereign_before - fee);
    assert_eq!(native_balance(&TestFeeSink::get()), sink_before + fee);
    let hot = ActorHot::<Test>::get(actor_id).expect("Crossing Actor remains active");
    assert!(hot.pending_signal);
    assert!(hot.queue_ticket.is_some() || hot.wakeup_pointer.is_some());
    assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
    {
      let rearmed = CrossingMemberships::<Test>::get(actor_id).expect("rearm membership");
      let rearmed_member = CrossingMemberPages::<Test>::get(rearmed.key, rearmed.page)
        .and_then(|page| page.entries.get(rearmed.offset as usize).copied())
        .expect("rearm Crossing member");
      assert_eq!(rearmed.key.threshold, 80);
      assert_eq!(rearmed_member.counterpart_threshold, 100);
    }
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed {
        actor_id: id,
        trigger_family: TriggerFamily::ObservationCrossing,
        fee: charged,
      } if *id == actor_id && *charged == fee
    )));
  });
}

#[test]
fn repeated_latched_crossing_fires_charge_only_the_useful_transition() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      transfer_contract_steps(BOB, 1),
    );
    clear_fee_collections();

    for (revision, previous, current) in [(2, 50, 150), (3, 150, 70), (4, 70, 150)] {
      assert_ok!(Actors::note_observation_transition(
        7,
        crate::ObservationTransition {
          revision,
          previous: Some(previous),
          current,
        },
      ));
      drain_crossing_work();
    }

    let fee = observation_crossing_trigger_fee();
    assert_eq!(fee_collections(), vec![fee]);
    let hot = ActorHot::<Test>::get(actor_id).expect("Crossing Actor remains active");
    assert!(hot.pending_signal);
    assert!(hot.queue_ticket.is_some() || hot.wakeup_pointer.is_some());
    assert_eq!(
      frame_system::Pallet::<Test>::events()
        .iter()
        .filter(|record| matches!(
          &record.event,
          RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
            actor_id: id,
            trigger_family: TriggerFamily::ObservationCrossing,
            ..
          }) if *id == actor_id
        ))
        .count(),
      1
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed {
        actor_id: id,
        trigger_family: TriggerFamily::ObservationCrossing,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn busy_crossing_fire_charges_and_latches_only_the_future_pipeline() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
    ])
    .expect("two-Step Contract fits");
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      steps,
    );
    fund_native(actor_id, 1_000_000);
    assert_eq!(
      Actors::request_activation(actor_id),
      Ok(ActivationOutcome::Latched)
    );
    Actors::on_idle(1, Weight::MAX);
    let run_before = ActorRunStateStore::<Test>::get(actor_id).expect("Pipeline is Running");
    assert_eq!(
      ActorHot::<Test>::get(actor_id).map(|hot| hot.cycle_state),
      Some(CycleState::Running)
    );
    clear_fee_collections();

    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    drain_crossing_work();

    assert_eq!(fee_collections(), vec![observation_crossing_trigger_fee()]);
    let hot = ActorHot::<Test>::get(actor_id).expect("busy Actor remains active");
    assert_eq!(hot.cycle_state, CycleState::Running);
    assert!(hot.pending_signal);
    let run_after = ActorRunStateStore::<Test>::get(actor_id).expect("Pipeline remains Running");
    assert_eq!(run_after.cursor, run_before.cursor);
    assert_eq!(run_after.cycle_nonce, run_before.cycle_nonce);
  });
}

#[test]
fn underfunded_crossing_fire_advances_without_fee_readiness_or_apoptosis() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      inert_contract_steps(),
    );
    let sovereign = sovereign_account(actor_id);
    let balance = native_balance(&sovereign);
    deplete_user_sovereign(actor_id, balance - TestMinUserBalance::get());
    clear_fee_collections();

    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    drain_crossing_work();

    assert!(fee_collections().is_empty());
    assert_eq!(native_balance(&sovereign), TestMinUserBalance::get());
    let hot = ActorHot::<Test>::get(actor_id).expect("process remains live");
    assert!(!hot.pending_signal);
    assert!(hot.queue_ticket.is_none());
    assert!(hot.wakeup_pointer.is_none());
    assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
    assert!(Actors::crossing_transition_queue(7).is_none());
    assert!(Actors::active_actor_view(actor_id).is_some());
  });
}

#[test]
fn crossing_batch_falls_back_to_scalar_progress_for_an_underfunded_member() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let schedule = Schedule {
      trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
      cooldown_blocks: 0,
    };
    let underfunded = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule.clone(),
      None,
      inert_contract_steps(),
    );
    let funded = create_user_with(
      BOB,
      Mutability::Mutable,
      schedule,
      None,
      inert_contract_steps(),
    );
    let balance = native_balance(&sovereign_account(underfunded));
    deplete_user_sovereign(underfunded, balance - TestMinUserBalance::get());
    clear_fee_collections();

    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    Actors::service_crossing_transitions(Weight::MAX);

    assert!(Actors::crossing_transition_queue(7).is_none());
    assert!(Actors::crossing_worker_fault().is_none());
    let underfunded_hot = ActorHot::<Test>::get(underfunded).expect("underfunded process remains");
    assert!(!underfunded_hot.pending_signal);
    assert!(underfunded_hot.queue_ticket.is_none());
    assert_eq!(crossing_phase(underfunded), CrossingPhase::WaitingForRearm);
    let funded_hot = ActorHot::<Test>::get(funded).expect("funded process remains");
    assert!(funded_hot.pending_signal);
    assert!(funded_hot.queue_ticket.is_some() || funded_hot.wakeup_pointer.is_some());
    assert_eq!(fee_collections(), vec![observation_crossing_trigger_fee()]);
  });
}

#[test]
fn crossing_fire_collection_failure_advances_without_readiness() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      inert_contract_steps(),
    );
    let sovereign = sovereign_account(actor_id);
    let before = native_balance(&sovereign);
    set_fail_fee_sink_transfer(true);

    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    drain_crossing_work();
    set_fail_fee_sink_transfer(false);

    assert_eq!(native_balance(&sovereign), before);
    let hot = ActorHot::<Test>::get(actor_id).expect("process remains live");
    assert!(!hot.pending_signal);
    assert!(hot.queue_ticket.is_none());
    assert!(hot.wakeup_pointer.is_none());
    assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
    assert!(Actors::crossing_transition_queue(7).is_none());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn observation_crossing_u128_boundaries_and_adjacent_hysteresis_are_exact() {
  let rising = ObservationCrossing {
    feed: 7u32,
    direction: CrossingDirection::Rising,
    threshold: u128::MAX,
    rearm_threshold: u128::MAX - 1,
  };
  assert!(rising.has_valid_hysteresis());
  assert_eq!(rising.initial_phase(u128::MAX - 1), CrossingPhase::Armed);
  assert_eq!(
    rising.initial_phase(u128::MAX),
    CrossingPhase::WaitingForRearm
  );
  assert_eq!(
    rising.transition(CrossingPhase::Armed, u128::MAX - 1, u128::MAX),
    CrossingTransition::Fire
  );
  assert_eq!(
    rising.transition(CrossingPhase::WaitingForRearm, u128::MAX, u128::MAX - 1),
    CrossingTransition::Rearm
  );

  let falling = ObservationCrossing {
    feed: 7u32,
    direction: CrossingDirection::Falling,
    threshold: 0,
    rearm_threshold: 1,
  };
  assert!(falling.has_valid_hysteresis());
  assert_eq!(falling.initial_phase(1), CrossingPhase::Armed);
  assert_eq!(falling.initial_phase(0), CrossingPhase::WaitingForRearm);
  assert_eq!(
    falling.transition(CrossingPhase::Armed, 1, 0),
    CrossingTransition::Fire
  );
  assert_eq!(
    falling.transition(CrossingPhase::WaitingForRearm, 0, 1),
    CrossingTransition::Rearm
  );

  for (direction, boundary) in [
    (CrossingDirection::Rising, u128::MAX),
    (CrossingDirection::Falling, 0),
  ] {
    assert!(
      !ObservationCrossing {
        feed: 7u32,
        direction,
        threshold: boundary,
        rearm_threshold: boundary,
      }
      .has_valid_hysteresis()
    );
  }
}

#[test]
fn per_feed_crossing_member_cap_rejects_before_active_installation_commits() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None,
    ));
    let feed_cap: u32 = <Test as crate::Config>::MaxCrossingMembersPerFeed::get();
    crate::CrossingFeedMembershipCount::<Test>::insert(7, feed_cap);
    frame_system::Pallet::<Test>::set_block_number(2);
    let contract = system_active_contract(
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      inert_contract_steps(),
    )
    .expect("active Crossing contract");
    assert_noop!(
      Actors::activate_actor(RuntimeOrigin::root(), 0, contract),
      Error::<Test>::CrossingIndexCapacityExceeded
    );
    assert!(matches!(
      Actors::load_actor_state(0),
      LoadedActorStateOf::Dormant(_)
    ));
    assert!(Actors::crossing_membership(0).is_none());
  });
}

#[test]
fn user_crossing_cap_reserves_feed_capacity_for_system_actors() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let schedule = Schedule {
      trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
      cooldown_blocks: 0,
    };
    let steps = inert_contract_steps();
    let user_cap = <Test as crate::Config>::MaxUserCrossingMembersPerFeed::get();
    for _ in 0..user_cap {
      create_user_with(
        ALICE,
        Mutability::Mutable,
        schedule.clone(),
        None,
        steps.clone(),
      );
    }
    assert_eq!(Actors::crossing_user_feed_membership_count(7), user_cap);
    assert_eq!(Actors::crossing_feed_membership_count(7), user_cap);

    let rejected_owner = BOB;
    prefund_active_user_creation(rejected_owner, &steps);
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(rejected_owner),
        Mutability::Mutable,
        user_active_contract(schedule.clone(), None, steps.clone()),
      ),
      Error::<Test>::CrossingUserCapacityExceeded
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );

    for index in 0..2u64 {
      create_system_with(30_000 + index, schedule.clone(), None, steps.clone());
    }
    assert_eq!(Actors::crossing_user_feed_membership_count(7), user_cap);
    assert_eq!(Actors::crossing_feed_membership_count(7), user_cap + 2);
    #[cfg(feature = "try-runtime")]
    {
      assert_ok!(crate::Pallet::<Test>::do_try_state());
      crate::CrossingUserFeedMembershipCount::<Test>::insert(7, user_cap + 1);
      assert!(crate::Pallet::<Test>::do_try_state().is_err());
      crate::CrossingUserFeedMembershipCount::<Test>::insert(7, user_cap);
      assert_ok!(crate::Pallet::<Test>::do_try_state());
    }
  });
}

#[test]
fn dormant_crossing_initializes_only_during_active_installation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None,
    ));
    let actor_id = 0;
    frame_system::Pallet::<Test>::set_block_number(2);
    let contract = system_active_contract(
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    )
    .expect("active Crossing contract");
    let identity = Actors::actor_identities(actor_id).expect("dormant identity");
    assert_noop!(
      Actors::activate_actor(RuntimeOrigin::root(), actor_id, contract.clone()),
      Error::<Test>::ObservationUnavailable
    );
    assert_eq!(Actors::actor_identities(actor_id), Some(identity));
    assert!(Actors::actor_hot(actor_id).is_none());
    assert!(Actors::crossing_membership(actor_id).is_none());

    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 120,
        observed_at: 1,
      },
    );
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::root(),
      actor_id,
      contract,
    ));
    assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
    let hot = ActorHot::<Test>::get(actor_id).expect("active Crossing actor");
    assert!(!hot.pending_signal);
    assert!(hot.queue_ticket.is_none());
  });
}

#[test]
fn active_crossing_initializes_without_retrofire_and_cleans_exact_index_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 100,
        observed_at: 1,
      },
    );
    let schedule = Schedule {
      trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
      cooldown_blocks: 0,
    };
    let actor_id = create_system_with(
      ALICE,
      schedule,
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let locator = Actors::crossing_membership(actor_id).expect("Crossing membership exists");
    assert!(matches!(
      ActorHot::<Test>::get(actor_id).map(|hot| hot.trigger_runtime_state),
      Some(TriggerRuntimeState::ObservationCrossing {
        phase: CrossingPhase::WaitingForRearm,
        ..
      })
    ));
    assert_eq!(locator.key.feed, 7);
    assert_eq!(locator.key.traversal, crate::CrossingTraversal::Downward);
    assert_eq!(locator.key.threshold, 80);
    assert!(
      !ActorHot::<Test>::get(actor_id)
        .expect("hot state exists")
        .pending_signal
    );
    assert_eq!(Actors::crossing_feed_membership_count(7), 1);
    assert!(crate::CrossingLeafStates::<Test>::contains_key(locator.key));
    assert!(
      crate::CrossingRadixNodes::<Test>::iter_keys()
        .any(|key| key.feed == 7 && key.traversal == crate::CrossingTraversal::Downward)
    );

    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), actor_id));
    assert!(Actors::crossing_membership(actor_id).is_none());
    assert_eq!(Actors::crossing_feed_membership_count(7), 0);
    assert!(!crate::CrossingLeafStates::<Test>::contains_key(
      locator.key
    ));
    assert!(
      !crate::CrossingRadixNodes::<Test>::iter_keys()
        .any(|key| key.feed == 7 && key.traversal == crate::CrossingTraversal::Downward)
    );
  });
}

#[test]
fn exact_crossing_trigger_equality_preserves_canonical_and_physical_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 120,
        observed_at: 1,
      },
    );
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    let contract = Actors::load_actor_contract(actor_id).expect("active Actor Contract");
    let hot = ActorHot::<Test>::get(actor_id).expect("active Actor hot state");
    let locator = Actors::crossing_membership(actor_id).expect("Crossing membership");
    System::reset_events();
    assert_ok!(Actors::update_contract(
      RuntimeOrigin::root(),
      actor_id,
      contract.clone(),
    ));
    assert_eq!(Actors::load_actor_contract(actor_id), Some(contract));
    assert_eq!(ActorHot::<Test>::get(actor_id), Some(hot));
    assert_eq!(Actors::crossing_membership(actor_id), Some(locator));
    assert!(System::events().is_empty());
  });
}

#[test]
fn active_crossing_rejects_invalid_hysteresis_and_missing_current_observation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract = |rearm_threshold| {
      system_active_contract(
        Schedule {
          trigger: RuntimeTrigger::observation_crossing(
            7,
            CrossingDirection::Rising,
            100,
            rearm_threshold,
          ),
          cooldown_blocks: 0,
        },
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      )
    };
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        contract(100),
      ),
      Error::<Test>::InvalidTriggerConfiguration
    );
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        contract(80),
      ),
      Error::<Test>::ObservationUnavailable
    );
    set_observation(7, crate::ScalarObservationState::Uninitialized);
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        contract(80),
      ),
      Error::<Test>::ObservationUninitialized
    );
  });
}

#[test]
fn crossing_transition_queue_preserves_reversals_and_fails_atomically_at_capacity() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    for (revision, previous, current) in [(2, 50, 110), (3, 110, 70), (4, 70, 120), (5, 120, 60)] {
      assert_ok!(Actors::note_observation_transition(
        7,
        crate::ObservationTransition {
          revision,
          previous: Some(previous),
          current,
        },
      ));
    }
    let queue = Actors::crossing_transition_queue(7).expect("transition queue exists");
    assert_eq!(queue.len(), 4);
    assert_eq!(
      queue
        .iter()
        .map(|transition| (transition.revision, transition.previous, transition.current))
        .collect::<Vec<_>>(),
      vec![(2, 50, 110), (3, 110, 70), (4, 70, 120), (5, 120, 60)]
    );
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_noop!(
      Actors::note_observation_transition(
        7,
        crate::ObservationTransition {
          revision: 6,
          previous: Some(60),
          current: 130,
        },
      ),
      Error::<Test>::CrossingTransitionCapacityExceeded
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );

    for _ in 0..8 {
      if Actors::crossing_transition_queue(7)
        .and_then(|queued| queued.first().copied())
        .is_none_or(|head| head.revision != 2)
      {
        break;
      }
      Actors::crossing_work_unit().expect("head transition service must succeed");
    }
    assert_eq!(
      Actors::crossing_transition_queue(7)
        .and_then(|queued| queued.first().copied())
        .map(|head| head.revision),
      Some(3)
    );
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 6,
        previous: Some(60),
        current: 130,
      },
    ));
    assert_eq!(
      Actors::crossing_transition_queue(7)
        .expect("recovered transition queue")
        .len(),
      4
    );
    drain_crossing_work();
    assert!(Actors::crossing_transition_queue(7).is_none());
    assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
    assert!(Actors::pending_signal(actor_id));
  });
}

#[test]
fn resumed_crossing_worker_preserves_per_block_component_caps() {
  new_test_ext().execute_with(|| {
    let counters = crate::crossing::CrossingWorkCounters {
      candidates: <Test as crate::Config>::MaxCrossingActorsPerBlock::get(),
      ..Default::default()
    };
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let (consumed, resumed) = Actors::service_crossing_transitions_resuming(Weight::MAX, counters);
    assert_eq!(consumed, Weight::zero());
    assert_eq!(resumed, counters);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before,
      "a resumed Crossing family at one component cap must not probe or mutate again"
    );
  });
}

#[test]
fn crossing_source_prefix_snapshot_grants_only_contiguous_validated_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actors = [ALICE, BOB, CHARLIE, 44]
      .into_iter()
      .map(|owner| {
        create_system_with(
          owner,
          Schedule {
            trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
            cooldown_blocks: 0,
          },
          None,
          contract_steps_with_step(make_step(Task::StopCycle)),
        )
      })
      .collect::<Vec<_>>();
    let locator = Actors::crossing_membership(actors[0]).expect("first locator");
    let first_contract = Actors::load_actor_contract(actors[0]).expect("first contract");
    let first_crossing =
      Actors::crossing_from_trigger(&first_contract.trigger).expect("first Crossing contract");
    let movement_root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_eq!(
      Actors::test_move_crossing_membership_without_hot(
        actors[0],
        first_crossing.clone(),
        CrossingPhase::WaitingForRearm,
        locator,
      ),
      Ok(true)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      movement_root,
      "movement-without-Hot proof must roll back every touched surface"
    );
    assert_eq!(
      Actors::test_derived_tail_locator_after_first_movement(
        actors[0],
        actors[3],
        first_crossing.clone(),
        locator,
      ),
      Ok(true)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      movement_root,
      "derived-tail proof must roll back every touched surface"
    );
    let mut split_tail_crossing = first_crossing.clone();
    split_tail_crossing.rearm_threshold = 70;
    assert_eq!(
      Actors::test_split_destination_pair_movements_without_hot(
        actors[0],
        actors[3],
        first_crossing.clone(),
        split_tail_crossing.clone(),
        locator,
      ),
      Ok(true)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      movement_root,
      "split-destination movement proof must roll back every touched surface"
    );
    assert_eq!(
      Actors::test_atomic_placed_pair_commit_prototype(actors[0], actors[3], locator),
      Ok(true)
    );
    assert_eq!(Actors::test_queue_append_commits(), 1);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      movement_root,
      "atomic pair prototype must roll back membership and queue surfaces"
    );
    let page = CrossingMemberPages::<Test>::get(locator.key, locator.page).expect("source page");
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_eq!(Actors::crossing_source_prefix_count(&page, 1, 2), 2);
    assert_eq!(Actors::crossing_source_prefix_count(&page, 1, u32::MAX), 3);
    assert_eq!(
      Actors::crossing_source_cohort_count(&page, 1, u32::MAX, Some(2)),
      2
    );
    assert_eq!(
      Actors::crossing_source_cohort_count(&page, 1, u32::MAX, Some(0)),
      0
    );
    assert_eq!(
      Actors::crossing_source_prefix_count(&page, page.entries.len() as u32, 2),
      0
    );
    assert_eq!(Actors::crossing_source_prefix_count(&page, 1, 0), 0);
    let stable_remainder = Actors::stable_crossing_source_remainder(&page, 1, 2)
      .expect("stable partial-page compaction");
    assert_eq!(
      stable_remainder
        .entries
        .iter()
        .map(|member| member.actor_id)
        .collect::<Vec<_>>(),
      vec![actors[0], actors[3]]
    );

    let prefix = Actors::snapshot_crossing_source_prefix(locator.key, locator.page, &page, 1, 2)
      .expect("contiguous prefix");
    assert_eq!(
      (
        prefix.key,
        prefix.page,
        prefix.start_offset,
        prefix.end_offset
      ),
      (locator.key, locator.page, 1, 3)
    );
    assert_eq!(
      prefix
        .candidates
        .iter()
        .map(|authority| authority.member.actor_id)
        .collect::<Vec<_>>(),
      actors[1..3]
    );
    let transition = crate::CrossingTransitionObligation {
      revision: 2,
      previous: 50,
      current: 150,
      cause_provenance: crate::TriggerCauseProvenance::Deferred,
      cause_block: 0,
    };
    let homogeneous = Actors::preflight_crossing_cohort(&prefix, transition, false, None)
      .expect("homogeneous preflight");
    assert_eq!(homogeneous.plan, crate::CrossingWorkPlan::FireCohortPending);
    assert_eq!(homogeneous.admitted_candidates, 2);
    assert_eq!(homogeneous.placed_immediate_fifo, None);
    let placed_homogeneous = Actors::preflight_crossing_cohort(&prefix, transition, true, None)
      .expect("homogeneous placed preflight");
    assert_eq!(
      placed_homogeneous.plan,
      crate::CrossingWorkPlan::FireCohortPlaced
    );
    assert_eq!(placed_homogeneous.admitted_candidates, 2);
    assert_eq!(placed_homogeneous.placed_immediate_fifo, Some(true));
    assert_eq!(placed_homogeneous.queue_candidates.len(), 2);
    assert!(
      placed_homogeneous
        .queue_candidates
        .iter(/* deos-bypass: bounded-iter */)
        .all(|(_, hot)| matches!(
          hot.trigger_runtime_state,
          TriggerRuntimeState::ObservationCrossing {
            phase: CrossingPhase::WaitingForRearm,
            ..
          }
        ))
    );
    let maximum_prefix =
      Actors::snapshot_crossing_source_prefix(locator.key, locator.page, &page, 0, 4)
        .expect("maximum production prefix");
    let maximum_preflight =
      Actors::preflight_crossing_cohort(&maximum_prefix, transition, true, None)
        .expect("maximum placed preflight");
    assert_eq!(maximum_preflight.admitted_candidates, 4);
    assert_eq!(maximum_preflight.placed_immediate_fifo, Some(true));
    assert_eq!(maximum_preflight.queue_candidates.len(), 4);
    let maximum_root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_eq!(
      Actors::test_placed_cohort_authority_count(&maximum_prefix, transition, false),
      Ok(4)
    );
    assert_eq!(Actors::test_queue_append_commits(), 1);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      maximum_root,
      "maximum cohort commit prototype must roll back membership and queue surfaces"
    );
    assert_eq!(
      Actors::test_placed_cohort_authority_count(&maximum_prefix, transition, true),
      Ok(4)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      maximum_root,
      "malformed later locator must roll back every cohort surface"
    );
    assert!(Actors::preflight_paged_enqueue_cohort(maximum_preflight.queue_candidates).is_ok());
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      maximum_root,
      "maximum placed prefix and aggregate FIFO authority must remain read-only"
    );

    let original_contract = Actors::load_actor_contract(actors[2]).expect("second prefix contract");
    let mut wakeup_contract = original_contract.clone();
    wakeup_contract.cooldown_blocks = 10;
    assert_ok!(Actors::store_actor_contract(actors[2], wakeup_contract));
    let mut wakeup_hot = ActorHot::<Test>::get(actors[2]).expect("second prefix actor");
    let original_wakeup_hot = wakeup_hot.clone();
    wakeup_hot.last_cycle_block = Some(1);
    ActorHot::<Test>::insert(actors[2], wakeup_hot);
    let placement_split =
      Actors::preflight_crossing_cohort(&maximum_prefix, transition, true, None)
        .expect("third-candidate placement split fallback");
    assert_eq!(
      placement_split.plan,
      crate::CrossingWorkPlan::FireCohortPlaced
    );
    assert_eq!(placement_split.admitted_candidates, 2);
    assert_eq!(placement_split.placed_immediate_fifo, Some(true));
    assert_eq!(placement_split.queue_candidates.len(), 2);
    assert!(matches!(
      placement_split.queue_candidates[0].1.trigger_runtime_state,
      TriggerRuntimeState::ObservationCrossing {
        phase: CrossingPhase::WaitingForRearm,
        ..
      }
    ));
    let wakeup_only =
      Actors::snapshot_crossing_source_prefix(locator.key, locator.page, &page, 2, 1)
        .expect("wakeup-only prefix");
    let wakeup_preflight = Actors::preflight_crossing_cohort(&wakeup_only, transition, true, None)
      .expect("wakeup-only preflight");
    assert_eq!(wakeup_preflight.admitted_candidates, 1);
    assert_eq!(wakeup_preflight.placed_immediate_fifo, Some(false));
    assert_ok!(Actors::store_actor_contract(actors[2], original_contract));
    ActorHot::<Test>::insert(actors[2], original_wakeup_hot);

    let mut heterogeneous_hot = ActorHot::<Test>::get(actors[2]).expect("second prefix actor");
    let original_hot = heterogeneous_hot.clone();
    let TriggerRuntimeState::ObservationCrossing { phase, .. } =
      heterogeneous_hot.trigger_runtime_state
    else {
      panic!("Crossing runtime state")
    };
    heterogeneous_hot.trigger_runtime_state = TriggerRuntimeState::ObservationCrossing {
      phase,
      installed_at_revision: transition.revision,
    };
    ActorHot::<Test>::insert(actors[2], heterogeneous_hot);
    let heterogeneous = Actors::preflight_crossing_cohort(&prefix, transition, false, None)
      .expect("heterogeneous fallback");
    assert_eq!(
      heterogeneous.plan,
      crate::CrossingWorkPlan::FireCohortPending
    );
    assert_eq!(heterogeneous.admitted_candidates, 1);
    ActorHot::<Test>::insert(actors[2], original_hot);

    let remainder =
      Actors::snapshot_crossing_source_prefix(locator.key, locator.page, &page, 1, u32::MAX)
        .expect("bounded remainder");
    assert_eq!(remainder.candidates.len(), 3);
    assert_eq!(remainder.end_offset, page.entries.len() as u32);
    let empty = Actors::snapshot_crossing_source_prefix(locator.key, locator.page, &page, 1, 0)
      .expect("zero authority");
    assert!(empty.candidates.is_empty());
    assert_eq!(empty.end_offset, empty.start_offset);
    assert!(matches!(
      Actors::snapshot_crossing_source_prefix(
        locator.key,
        locator.page,
        &page,
        page.entries.len() as u32 + 1,
        1,
      ),
      Err(error) if error == Error::<Test>::CrossingIndexInvariant.into()
    ));
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );

    CrossingMemberships::<Test>::mutate(actors[3], |maybe_locator| {
      maybe_locator.as_mut().expect("outside locator").offset = 99;
    });
    assert!(
      Actors::snapshot_crossing_source_prefix(locator.key, locator.page, &page, 1, 2).is_ok(),
      "corruption outside the selected prefix is not read"
    );
    CrossingMemberships::<Test>::mutate(actors[2], |maybe_locator| {
      maybe_locator.as_mut().expect("selected locator").offset = 98;
    });
    let corrupt_root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert!(matches!(
      Actors::snapshot_crossing_source_prefix(locator.key, locator.page, &page, 1, 2),
      Err(error) if error == Error::<Test>::CrossingIndexInvariant.into()
    ));
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      corrupt_root
    );
  });
}

#[test]
fn crossing_tail_suffix_snapshot_grants_exact_generation_checked_refill_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    for owner in 1_000..1_031 {
      let actor_id = create_system_with(
        owner,
        manual_schedule(),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      assert!(Actors::paged_enqueue(actor_id));
    }
    assert_eq!(Actors::queue_occupancy(), 31);
    let actors = (0..20)
      .map(|index| {
        create_system_with(
          100 + index,
          Schedule {
            trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
            cooldown_blocks: 0,
          },
          None,
          contract_steps_with_step(make_step(Task::StopCycle)),
        )
      })
      .collect::<Vec<_>>();
    let mut split_contract =
      Actors::load_actor_contract(actors[2]).expect("split-destination contract");
    split_contract.trigger =
      RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 70);
    assert_ok!(Actors::store_actor_contract(actors[2], split_contract));
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    let source = Actors::crossing_membership(actors[0]).expect("source locator");
    crate::CrossingRangeCursors::<Test>::insert(
      7,
      crate::CrossingRangeCursor {
        revision: 2,
        traversal: crate::CrossingTraversal::Upward,
        search_bound: 150,
        current_threshold: Some(100),
        page: source.page,
        offset: 0,
        exhausted: false,
      },
    );
    let root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let suffix = Actors::snapshot_crossing_tail_suffix(source.key, source.page, 4)
      .expect("bounded tail suffix");
    assert_eq!(suffix.page, 1);
    assert_eq!(suffix.start_offset, 0);
    let source_page =
      CrossingMemberPages::<Test>::get(source.key, source.page).expect("non-tail source page");
    let source_prefix =
      Actors::snapshot_crossing_source_prefix(source.key, source.page, &source_page, 0, 4)
        .expect("non-tail source prefix");
    assert_eq!(
      Actors::test_non_tail_placed_authority_count(
        &source_prefix,
        &suffix,
        crate::CrossingTransitionObligation {
          revision: 2,
          previous: 50,
          current: 150,
          cause_provenance: crate::TriggerCauseProvenance::Deferred,
          cause_block: 0,
        },
      ),
      Ok(4)
    );
    assert_eq!(
      Actors::rewrite_non_tail_source(&source_prefix, &suffix, false),
      Ok(true)
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), root);
    assert_eq!(
      suffix
        .candidates
        .iter()
        .map(|candidate| candidate.member.actor_id)
        .collect::<Vec<_>>(),
      actors[16..20]
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), root);
    let corrupted = polkadot_sdk::frame_support::storage::with_transaction(|| {
      CrossingMemberships::<Test>::mutate(actors[18], |locator| {
        locator.as_mut().expect("tail locator").generation += 1;
      });
      let rejected = Actors::snapshot_crossing_tail_suffix(source.key, source.page, 4).is_err()
        && Actors::rewrite_non_tail_source(&source_prefix, &suffix, false).is_err();
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Ok::<bool, DispatchError>(
        rejected,
      ))
    });
    assert_eq!(corrupted, Ok(true));
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), root);
    let corrupted_source = polkadot_sdk::frame_support::storage::with_transaction(|| {
      crate::CrossingMemberships::<Test>::mutate(
        source_prefix.candidates[1].member.actor_id,
        |locator| {
          locator.as_mut().expect("source locator").generation += 1;
        },
      );
      let rejected = Actors::rewrite_non_tail_source(&source_prefix, &suffix, false).is_err();
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Ok::<bool, DispatchError>(
        rejected,
      ))
    });
    assert_eq!(corrupted_source, Ok(true));
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), root);

    for owner in [120, 121] {
      create_system_with(
        owner,
        Schedule {
          trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
          cooldown_blocks: 0,
        },
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
    }
    let trimmed_root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let trimmed_suffix = Actors::snapshot_crossing_tail_suffix(source.key, source.page, 4)
      .expect("trimmed tail suffix");
    assert_eq!(trimmed_suffix.start_offset, 2);
    assert_eq!(
      Actors::rewrite_non_tail_source(&source_prefix, &trimmed_suffix, false),
      Ok(true)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      trimmed_root,
      "trimmed-tail source rewrite must roll back exactly"
    );

    Actors::test_reset_queue_append_commits();
    Actors::test_reset_crossing_cursor_commits();
    Actors::test_reset_first_crossing_branch_weight();
    let non_tail_weight =
      <TestWeightInfo as crate::WeightInfo>::crossing_placed_non_tail_trimmed_unit();
    let production_budget = <TestWeightInfo as crate::WeightInfo>::crossing_worker_base()
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::crossing_work_probe())
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::crossing_fire_pair_probe())
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::crossing_tail_refill_probe())
      .saturating_add(non_tail_weight)
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::record_crossing_worker_fault());
    Actors::service_crossing_transitions(production_budget);
    assert_eq!(
      Actors::test_first_crossing_branch_weight(),
      Some(non_tail_weight),
      "production must consume the admitted specialized non-tail branch owner"
    );
    assert!(Actors::test_queue_append_commits() >= 1);
    assert!(Actors::test_crossing_cursor_commits() >= 1);
    assert!(
      actors[..4].iter().all(|actor_id| {
        Actors::crossing_membership(*actor_id).is_some_and(|locator| locator.key != source.key)
      }),
      "production locators: {:?}",
      actors[..4]
        .iter()
        .map(|actor_id| Actors::crossing_membership(*actor_id))
        .collect::<Vec<_>>()
    );
    assert_eq!(
      actors[..4]
        .iter()
        .map(|actor_id| Actors::actor_hot(*actor_id).and_then(|hot| hot.queue_ticket))
        .collect::<Vec<_>>(),
      vec![Some(31), Some(32), Some(33), Some(34)],
      "non-tail aggregate queue authority must cross the queue-page boundary"
    );
    assert_ne!(
      Actors::crossing_membership(actors[0]).map(|locator| locator.key),
      Actors::crossing_membership(actors[2]).map(|locator| locator.key),
      "production non-tail commit must preserve split destinations"
    );
    assert_eq!(
      Actors::crossing_membership(actors[4]).map(|locator| (
        locator.key,
        locator.page,
        locator.offset
      )),
      Some((source.key, source.page, 0))
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::service_crossing_transitions(production_budget);
    assert!(
      Actors::crossing_membership(actors[4]).is_some_and(|locator| locator.key != source.key),
      "the retained source prefix must resume from the persisted cursor"
    );
  });
}

#[test]
fn crossing_plan_components_are_exhaustive_and_branch_exact() {
  use crate::CrossingWorkPlan::*;

  for plan in [Empty, StructuralFault] {
    assert_eq!(Actors::crossing_plan_components(plan), (0, 0, 0, 0));
  }
  for plan in [CompleteTransition, SeekMiss] {
    assert_eq!(Actors::crossing_plan_components(plan), (1, 0, 0, 0));
  }
  assert_eq!(Actors::crossing_plan_components(AdvanceLeaf), (1, 1, 0, 0));
  assert_eq!(Actors::crossing_plan_components(AdvancePage), (1, 1, 1, 0));
  for plan in [
    SkipPostInstallationPairPending,
    SkipPostInstallationPair,
    RearmCohortPairPending,
    RearmCohortPair,
    FireCohortPairPending,
    FireCohortPlacedBatch,
    FireCohortCoalescedPair,
  ] {
    assert_eq!(Actors::crossing_plan_components(plan), (2, 2, 2, 2));
  }
  assert_eq!(
    Actors::crossing_plan_components_for_admission(FireCohortPlacedBatch, 3),
    Some((3, 3, 3, 3))
  );
  assert_eq!(
    Actors::crossing_plan_components_for_admission(FireCohortPlacedBatch, 4),
    Some((4, 4, 4, 4))
  );
  assert_eq!(
    Actors::crossing_plan_components_for_admission(FireCohortPlacedBatch, 1),
    None
  );
  let pair_weight = Actors::crossing_plan_weight(FireCohortPlacedBatch);
  assert_eq!(
    Actors::crossing_plan_weight_for_admission(FireCohortPlacedBatch, 2),
    Some(pair_weight)
  );
  assert_eq!(
    Actors::crossing_plan_weight_for_admission(FireCohortPlacedBatch, 3),
    Some(<Test as crate::Config>::WeightInfo::crossing_placed_maximum_unit())
  );
  assert_eq!(
    Actors::crossing_plan_weight_for_admission(FireCohortPlacedBatch, 4),
    Some(<Test as crate::Config>::WeightInfo::crossing_placed_maximum_unit())
  );
  assert_eq!(
    Actors::crossing_plan_weight_for_admission(FireCohortPlacedBatch, 1),
    None
  );
  for plan in [
    OpenLeaf,
    SkipPostInstallationTransition,
    RearmCohort,
    FireCohortPending,
    FireCohortCoalesced,
    FireCohortPlaced,
    FireCohortClosed,
  ] {
    assert_eq!(Actors::crossing_plan_components(plan), (1, 1, 1, 1));
  }
  for (pair, single) in [
    (SkipPostInstallationPair, SkipPostInstallationTransition),
    (RearmCohortPair, RearmCohort),
    (FireCohortPlacedBatch, FireCohortPlaced),
    (FireCohortCoalescedPair, FireCohortCoalesced),
  ] {
    assert_eq!(Actors::crossing_single_candidate_plan(pair), Some(single));
  }
  assert_eq!(
    Actors::crossing_single_candidate_plan(FireCohortClosed),
    None
  );
}

#[test]
fn crossing_pair_downgrades_to_one_at_each_resumed_component_boundary() {
  let maximums: [u32; 4] = [
    <Test as crate::Config>::MaxCrossingTransitionsPerBlock::get(),
    <Test as crate::Config>::MaxCrossingLeavesPerBlock::get(),
    <Test as crate::Config>::MaxCrossingPagesPerBlock::get(),
    <Test as crate::Config>::MaxCrossingActorsPerBlock::get(),
  ];
  for component in 0..maximums.len() {
    new_test_ext().execute_with(|| {
      prepare_crossing_pair_after_sparse_open();
      let mut counters = crate::crossing::CrossingWorkCounters::default();
      let admitted_start = maximums[component].saturating_sub(1);
      match component {
        0 => counters.transitions = admitted_start,
        1 => counters.leaves = admitted_start,
        2 => counters.pages = admitted_start,
        _ => counters.candidates = admitted_start,
      }
      let (_, resumed) = Actors::service_crossing_transitions_resuming(Weight::MAX, counters);
      let observed = match component {
        0 => resumed.transitions,
        1 => resumed.leaves,
        2 => resumed.pages,
        _ => resumed.candidates,
      };
      assert_eq!(observed, maximums[component]);
      assert_eq!(resumed.candidates, counters.candidates.saturating_add(1));
      assert_eq!(resumed.faults, 0);
      assert_eq!(Actors::queue_occupancy(), 2);
    });
  }
}

#[test]
fn crossing_pair_downgrades_to_one_at_probe_weight_boundary() {
  use crate::weights::WeightInfo as _;

  new_test_ext().execute_with(|| {
    prepare_crossing_pair_after_sparse_open();
    let budget = <Test as crate::Config>::WeightInfo::crossing_worker_base()
      .saturating_add(<Test as crate::Config>::WeightInfo::crossing_work_probe())
      .saturating_add(<Test as crate::Config>::WeightInfo::crossing_fire_probe())
      .saturating_add(<Test as crate::Config>::WeightInfo::crossing_placed_unit())
      .saturating_add(<Test as crate::Config>::WeightInfo::record_crossing_worker_fault());
    let (_, counters) = Actors::service_crossing_transitions_resuming(
      budget,
      crate::crossing::CrossingWorkCounters::default(),
    );
    assert_eq!(counters.candidates, 1);
    assert_eq!(counters.faults, 0);
    assert_eq!(Actors::queue_occupancy(), 2);

    let (_, resumed) = Actors::service_crossing_transitions_resuming(Weight::MAX, counters);
    assert_eq!(resumed.candidates, 2);
    assert_eq!(resumed.faults, 0);
    assert_eq!(Actors::queue_occupancy(), 3);
  });
}

#[test]
fn crossing_pair_downgrades_to_one_at_branch_weight_boundary() {
  use crate::weights::WeightInfo as _;

  new_test_ext().execute_with(|| {
    prepare_crossing_pair_after_sparse_open();
    let budget = <Test as crate::Config>::WeightInfo::crossing_worker_base()
      .saturating_add(<Test as crate::Config>::WeightInfo::crossing_work_probe())
      .saturating_add(<Test as crate::Config>::WeightInfo::crossing_fire_pair_probe())
      .saturating_add(<Test as crate::Config>::WeightInfo::crossing_placed_unit())
      .saturating_add(<Test as crate::Config>::WeightInfo::record_crossing_worker_fault());
    let (_, counters) = Actors::service_crossing_transitions_resuming(
      budget,
      crate::crossing::CrossingWorkCounters::default(),
    );
    assert_eq!(counters.candidates, 1);
    assert_eq!(counters.faults, 0);
    assert_eq!(Actors::queue_occupancy(), 2);

    let (_, resumed) = Actors::service_crossing_transitions_resuming(Weight::MAX, counters);
    assert_eq!(resumed.candidates, 2);
    assert_eq!(resumed.faults, 0);
    assert_eq!(Actors::queue_occupancy(), 3);
  });
}

#[test]
fn crossing_pair_admits_at_exact_remaining_component_capacity() {
  let maximums: [u32; 4] = [
    <Test as crate::Config>::MaxCrossingTransitionsPerBlock::get(),
    <Test as crate::Config>::MaxCrossingLeavesPerBlock::get(),
    <Test as crate::Config>::MaxCrossingPagesPerBlock::get(),
    <Test as crate::Config>::MaxCrossingActorsPerBlock::get(),
  ];
  for component in 0..maximums.len() {
    new_test_ext().execute_with(|| {
      prepare_crossing_pair_after_sparse_open();
      let mut counters = crate::crossing::CrossingWorkCounters::default();
      let admitted_start = maximums[component].saturating_sub(2);
      match component {
        0 => counters.transitions = admitted_start,
        1 => counters.leaves = admitted_start,
        2 => counters.pages = admitted_start,
        _ => counters.candidates = admitted_start,
      }
      let (_, resumed) = Actors::service_crossing_transitions_resuming(Weight::MAX, counters);
      let observed = match component {
        0 => resumed.transitions,
        1 => resumed.leaves,
        2 => resumed.pages,
        _ => resumed.candidates,
      };
      assert_eq!(observed, maximums[component]);
      assert_eq!(resumed.candidates, counters.candidates.saturating_add(2));
      assert_eq!(resumed.faults, 0);
      assert_eq!(Actors::queue_occupancy(), 3);
    });
  }
}

#[test]
fn crossing_worker_fires_then_rearms_in_revision_order_without_retrofire() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    for (revision, previous, current) in [(2, 50, 110), (3, 110, 70)] {
      assert_ok!(Actors::note_observation_transition(
        7,
        crate::ObservationTransition {
          revision,
          previous: Some(previous),
          current,
        },
      ));
    }
    for _ in 0..8 {
      if !Actors::crossing_work_unit().expect("Crossing worker remains valid") {
        break;
      }
    }
    assert!(Actors::crossing_transition_queue(7).is_none());
    assert_eq!(Actors::crossing_pending_feed_list().count, 0);
    let locator = Actors::crossing_membership(actor_id).expect("Crossing membership remains");
    assert!(matches!(
      ActorHot::<Test>::get(actor_id).map(|hot| hot.trigger_runtime_state),
      Some(TriggerRuntimeState::ObservationCrossing {
        phase: CrossingPhase::WaitingForRearm,
        ..
      })
    ));
    assert_eq!(locator.key.traversal, crate::CrossingTraversal::Downward);
    assert_eq!(locator.key.threshold, 80);
    assert!(
      ActorHot::<Test>::get(actor_id)
        .expect("hot state exists")
        .pending_signal
    );
  });
}

#[test]
fn falling_crossing_fires_and_rearms_without_duplicate_activation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 120,
        observed_at: 1,
      },
    );
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Falling, 100, 120),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(120),
        current: 100,
      },
    ));
    drain_crossing_work();
    let fired = Actors::crossing_membership(actor_id).expect("membership remains");
    assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
    assert_eq!(fired.key.traversal, crate::CrossingTraversal::Upward);
    let ticket = ActorHot::<Test>::get(actor_id)
      .expect("hot state")
      .queue_ticket;
    assert!(ticket.is_some());

    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 3,
        previous: Some(100),
        current: 120,
      },
    ));
    drain_crossing_work();
    let rearmed = Actors::crossing_membership(actor_id).expect("membership remains");
    assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
    assert_eq!(rearmed.key.traversal, crate::CrossingTraversal::Upward);
    assert_eq!(
      ActorHot::<Test>::get(actor_id)
        .expect("hot state")
        .queue_ticket,
      ticket,
      "rearm changes detection state only and cannot duplicate activation"
    );
  });
}

#[test]
fn paused_and_breaker_crossings_preserve_state_evolution_and_coalescing() {
  for paused in [true, false] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      set_observation(
        7,
        crate::ScalarObservationState::Fresh {
          value: 50,
          observed_at: 1,
        },
      );
      let actor_id = create_system_with(
        ALICE,
        Schedule {
          trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
          cooldown_blocks: 0,
        },
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      frame_system::Pallet::<Test>::set_block_number(2);
      if paused {
        assert_ok!(Actors::pause_actor(RuntimeOrigin::root(), actor_id));
      } else {
        assert_ok!(Actors::set_global_circuit_breaker(
          RuntimeOrigin::root(),
          true
        ));
      }

      assert_ok!(Actors::note_observation_transition(
        7,
        crate::ObservationTransition {
          revision: 2,
          previous: Some(50),
          current: 120,
        },
      ));
      drain_crossing_work();
      assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
      let first = ActorHot::<Test>::get(actor_id).expect("active Crossing actor");
      assert!(first.pending_signal);
      assert_eq!(
        Actors::indexed_trigger_detection_disabled(actor_id),
        Some(())
      );
      let placement = (first.queue_ticket, first.wakeup_pointer);

      assert_ok!(Actors::note_observation_transition(
        7,
        crate::ObservationTransition {
          revision: 3,
          previous: Some(120),
          current: 70,
        },
      ));
      drain_crossing_work();
      assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
      let rearmed = ActorHot::<Test>::get(actor_id).expect("active Crossing actor");
      assert!(rearmed.pending_signal);
      assert_eq!((rearmed.queue_ticket, rearmed.wakeup_pointer), placement);

      assert_ok!(Actors::note_observation_transition(
        7,
        crate::ObservationTransition {
          revision: 4,
          previous: Some(70),
          current: 120,
        },
      ));
      drain_crossing_work();
      assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
      let fired_again = ActorHot::<Test>::get(actor_id).expect("active Crossing actor");
      assert!(fired_again.pending_signal);
      assert_eq!(
        (fired_again.queue_ticket, fired_again.wakeup_pointer),
        placement
      );

      frame_system::Pallet::<Test>::set_block_number(3);
      if paused {
        assert_ok!(Actors::resume_actor(RuntimeOrigin::root(), actor_id));
      } else {
        assert_ok!(Actors::set_global_circuit_breaker(
          RuntimeOrigin::root(),
          false
        ));
      }
      run_idle(Weight::MAX);
      let resumed = ActorHot::<Test>::get(actor_id).expect("Crossing actor survives StopCycle");
      assert_eq!(
        resumed.trigger_runtime_state,
        TriggerRuntimeState::ObservationCrossing {
          phase: CrossingPhase::Armed,
          installed_at_revision: 1,
        }
      );
      assert!(!resumed.pending_signal);
      assert_eq!(Actors::indexed_trigger_detection_disabled(actor_id), None);
      assert!(resumed.queue_ticket.is_none());
      assert!(resumed.wakeup_pointer.is_none());
    });
  }
}

#[test]
fn same_threshold_crossing_herd_spans_pages_without_loss_or_duplicate_ticket() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let count = <<Test as crate::Config>::CrossingPageSize as Get<u32>>::get() + 1;
    let mut actors = Vec::new();
    for owner in 0..count {
      actors.push(create_system_with(
        10_000 + u64::from(owner),
        Schedule {
          trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
          cooldown_blocks: 0,
        },
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      ));
    }
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    drain_crossing_work();
    let mut tickets = BTreeSet::new();
    for actor_id in actors {
      let hot = ActorHot::<Test>::get(actor_id).expect("hot state");
      assert!(hot.pending_signal);
      assert!(tickets.insert(hot.queue_ticket.expect("one live ticket")));
      assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
    }
    assert_eq!(tickets.len() as u32, count);
  });
}

#[test]
fn crossing_compaction_rewinds_cursor_for_an_unprocessed_tail_member() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let count = <<Test as crate::Config>::CrossingPageSize as Get<u32>>::get() + 1;
    let actors = (0..count)
      .map(|owner| {
        create_system_with(
          20_000 + u64::from(owner),
          Schedule {
            trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
            cooldown_blocks: 0,
          },
          None,
          contract_steps_with_step(make_step(Task::StopCycle)),
        )
      })
      .collect::<Vec<_>>();
    ActorHot::<Test>::mutate(actors[0], |maybe_hot| {
      let hot = maybe_hot.as_mut().expect("active Crossing actor");
      let TriggerRuntimeState::ObservationCrossing { phase, .. } = hot.trigger_runtime_state else {
        panic!("actor owns Crossing runtime state");
      };
      hot.trigger_runtime_state = TriggerRuntimeState::ObservationCrossing {
        phase,
        installed_at_revision: 2,
      };
    });
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    assert!(Actors::crossing_work_unit().expect("first skipped member is valid"));
    assert_eq!(
      Actors::crossing_range_cursor(7)
        .expect("cursor exists")
        .offset,
      1
    );
    let tail_actor = *actors.last().expect("tail actor exists");
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), actors[0]));
    let rewound = Actors::crossing_range_cursor(7).expect("cursor remains");
    assert_eq!((rewound.page, rewound.offset), (0, 0));
    drain_crossing_work();
    assert!(
      ActorHot::<Test>::get(tail_actor)
        .expect("tail actor remains")
        .pending_signal
    );
    assert_eq!(crossing_phase(tail_actor), CrossingPhase::WaitingForRearm);
  });
}

#[test]
fn large_crossing_jump_visits_each_distinct_occupied_threshold_once() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 10,
        observed_at: 1,
      },
    );
    let mut actors = Vec::new();
    for (offset, (threshold, rearm)) in [(25, 20), (100, 80), (175, 150)].into_iter().enumerate() {
      actors.push(create_system_with(
        20_000 + offset as u64,
        Schedule {
          trigger: RuntimeTrigger::observation_crossing(
            7,
            CrossingDirection::Rising,
            threshold,
            rearm,
          ),
          cooldown_blocks: 0,
        },
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      ));
    }
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(10),
        current: 200,
      },
    ));
    drain_crossing_work();
    let tickets = actors
      .iter()
      .map(|actor_id| {
        assert_eq!(crossing_phase(*actor_id), CrossingPhase::WaitingForRearm);
        ActorHot::<Test>::get(*actor_id)
          .expect("hot state")
          .queue_ticket
          .expect("one live ticket")
      })
      .collect::<BTreeSet<_>>();
    assert_eq!(tickets.len(), actors.len());
    assert!(Actors::crossing_transition_queue(7).is_none());
  });
}

#[test]
fn sparse_crossing_range_preserves_bounded_leaf_suffix_across_service_passes() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 0,
        observed_at: 1,
      },
    );
    let mut actors = Vec::new();
    for index in 0..17u32 {
      let threshold = 1u128 << (index * 7);
      actors.push(create_system_with(
        25_000 + u64::from(index),
        Schedule {
          trigger: RuntimeTrigger::observation_crossing(
            7,
            CrossingDirection::Rising,
            threshold,
            threshold - 1,
          ),
          cooldown_blocks: 0,
        },
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      ));
    }
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(0),
        current: u128::MAX,
      },
    ));

    let first_weight = Actors::service_crossing_transitions(Weight::MAX);
    assert!(first_weight.ref_time() > 0 && first_weight.proof_size() > 0);
    let first_fired = actors
      .iter()
      .filter(|actor_id| {
        ActorHot::<Test>::get(**actor_id)
          .expect("actor remains active")
          .pending_signal
      })
      .count();
    assert_eq!(
      first_fired, 4,
      "the transition component cap admits four sparse leaves"
    );
    assert!(
      first_fired
        <= <<Test as crate::Config>::MaxCrossingLeavesPerBlock as Get<u32>>::get() as usize
    );
    assert!(Actors::crossing_range_cursor(7).is_some());
    assert!(Actors::crossing_transition_queue(7).is_some());

    let mut passes = 1u32;
    while Actors::crossing_pending_feed_list().count > 0 {
      assert!(
        passes < 8,
        "sparse suffix must converge within eight passes"
      );
      Actors::service_crossing_transitions(Weight::MAX);
      passes = passes.saturating_add(1);
    }
    assert_eq!(passes, 5);
    let tickets = actors
      .iter()
      .map(|actor_id| {
        ActorHot::<Test>::get(*actor_id)
          .expect("actor remains active")
          .queue_ticket
          .expect("one live ticket")
      })
      .collect::<BTreeSet<_>>();
    assert_eq!(tickets.len(), actors.len());
    assert!(
      actors
        .iter()
        .all(|actor_id| crossing_phase(*actor_id) == CrossingPhase::WaitingForRearm)
    );
  });
}

#[test]
#[ignore] // Release load profile; run through scripts/actors-assurance.sh.
fn crossing_scale_10k_zero_match_small_cohort_and_maximum_herd() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 0,
        observed_at: 1,
      },
    );
    let actor_count = 10_000u32;
    let matched_count = 8u32;
    // This detector-only load fixture exceeds the mock's 1,024-ticket queue
    // configuration but never places more than the eight matched actors. The
    // production runtime configures both active and queue bounds to 10,000.
    crate::ActiveActorLimit::<Test>::put(actor_count);
    let mut actors = Vec::with_capacity(actor_count as usize);
    let mut matched = Vec::new();
    for index in 0..actor_count {
      let is_match = index < matched_count;
      let actor_id = create_system_with(
        30_000 + u64::from(index),
        Schedule {
          trigger: RuntimeTrigger::observation_crossing(
            7,
            CrossingDirection::Rising,
            if is_match { 50 } else { 100 },
            if is_match { 40 } else { 80 },
          ),
          cooldown_blocks: 0,
        },
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      if is_match {
        matched.push(actor_id);
      }
      actors.push(actor_id);
    }
    assert_eq!(Actors::crossing_feed_membership_count(7), actor_count);

    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(0),
        current: 1,
      },
    ));
    let (_, zero_match) = Actors::service_crossing_transitions_with_counters(Weight::MAX);
    assert_eq!(Actors::crossing_pending_feed_list().count, 0);
    assert_eq!(zero_match.candidates, 0);
    assert_eq!(zero_match.canonical_probes, 0);
    assert_eq!(zero_match.activations, 0);
    assert_eq!(zero_match.closes, 0);
    assert_eq!(zero_match.faults, 0);
    assert_eq!(Actors::combined_queue_occupancy(), 0);
    assert!(matched.iter().all(|actor_id| {
      !ActorHot::<Test>::get(*actor_id)
        .expect("matched actor remains active")
        .pending_signal
    }));

    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 3,
        previous: Some(1),
        current: 60,
      },
    ));
    let mut small_cohort = crate::crossing::CrossingWorkCounters::default();
    while Actors::crossing_pending_feed_list().count > 0 {
      let (_, pass) = Actors::service_crossing_transitions_with_counters(Weight::MAX);
      small_cohort.transitions = small_cohort.transitions.saturating_add(pass.transitions);
      small_cohort.leaves = small_cohort.leaves.saturating_add(pass.leaves);
      small_cohort.pages = small_cohort.pages.saturating_add(pass.pages);
      small_cohort.candidates = small_cohort.candidates.saturating_add(pass.candidates);
      small_cohort.canonical_probes = small_cohort
        .canonical_probes
        .saturating_add(pass.canonical_probes);
      small_cohort.activations = small_cohort.activations.saturating_add(pass.activations);
      small_cohort.closes = small_cohort.closes.saturating_add(pass.closes);
      small_cohort.faults = small_cohort.faults.saturating_add(pass.faults);
    }
    assert_eq!(small_cohort.candidates, matched_count);
    assert_eq!(small_cohort.canonical_probes, matched_count);
    assert_eq!(small_cohort.activations, matched_count);
    assert_eq!(small_cohort.closes, 0);
    assert_eq!(small_cohort.faults, 0);
    assert_eq!(Actors::combined_queue_occupancy(), u64::from(matched_count));
    let tickets = matched
      .iter()
      .map(|actor_id| {
        ActorHot::<Test>::get(*actor_id)
          .expect("matched actor remains active")
          .queue_ticket
          .expect("matched actor owns one ticket")
      })
      .collect::<BTreeSet<_>>();
    assert_eq!(tickets.len() as u32, matched_count);

    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 60,
        observed_at: 1,
      },
    );
    for (index, actor_id) in matched.iter().copied().enumerate() {
      assert_ok!(update_contract_partial!(
        RuntimeOrigin::signed(30_000 + index as u64),
        actor_id,
        Schedule {
          trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80,),
          cooldown_blocks: 0,
        },
        None,
      ));
    }
    assert_eq!(
      crate::CrossingLeafStates::<Test>::get(crate::CrossingLeafKey {
        feed: 7,
        traversal: crate::CrossingTraversal::Upward,
        threshold: 100,
      })
      .expect("maximum herd leaf exists")
      .member_count,
      actor_count
    );
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 4,
        previous: Some(60),
        current: 150,
      },
    ));
    let initial_herd_units = drain_crossing_work_with_limit(actor_count.saturating_mul(2));
    let mut queued = 0u32;
    let mut deferred = 0u32;
    let mut placements = Vec::with_capacity(actor_count as usize);
    for actor_id in actors.iter().copied() {
      let hot = ActorHot::<Test>::get(actor_id).expect("herd actor remains active");
      assert!(hot.pending_signal);
      placements.push((hot.queue_ticket, hot.wakeup_pointer));
      match (hot.queue_ticket, hot.wakeup_pointer) {
        (Some(_), None) => queued = queued.saturating_add(1),
        (None, Some(pointer)) if pointer.block == WakeupKey::Block(2) => {
          deferred = deferred.saturating_add(1)
        }
        placement => {
          panic!("actor must own exactly one queue or deferred placement: {placement:?}")
        }
      }
      assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
    }
    assert_eq!(queued, 1_024);
    assert_eq!(queued.saturating_add(deferred), actor_count);
    assert!(Actors::crossing_transition_queue(7).is_none());

    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 5,
        previous: Some(150),
        current: 0,
      },
    ));
    let rearm_herd_units = drain_crossing_work_with_limit(actor_count.saturating_mul(2));
    for (index, actor_id) in actors.iter().enumerate() {
      let hot = ActorHot::<Test>::get(*actor_id).expect("herd actor remains active");
      assert_eq!(
        (
          crossing_phase(*actor_id),
          hot.queue_ticket,
          hot.wakeup_pointer
        ),
        (
          if index < matched_count as usize {
            CrossingPhase::Armed
          } else {
            CrossingPhase::WaitingForRearm
          },
          placements[index].0,
          placements[index].1
        ),
        "rearm mismatch at {index}",
      );
    }

    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 6,
        previous: Some(0),
        current: 150,
      },
    ));
    let repeated_herd_units = drain_crossing_work_with_limit(actor_count.saturating_mul(2));
    assert!(actors.iter().enumerate().all(|(index, actor_id)| {
      let hot = ActorHot::<Test>::get(*actor_id).expect("herd actor remains active");
      crossing_phase(*actor_id) == CrossingPhase::WaitingForRearm
        && (hot.queue_ticket, hot.wakeup_pointer) == placements[index]
    }));
    assert_eq!(
      (initial_herd_units, rearm_herd_units, repeated_herd_units),
      (10_002, 10_627, 10),
      "latched members consume the rearm observation without re-enabling detection; only the eight explicitly replaced members remain armed for the repeated fire"
    );
  });
}

#[test]
#[ignore] // Release load profile; run through scripts/actors-assurance.sh.
fn breaker_materializes_maximum_mixed_wakeup_crossing_and_broad_fanout_without_execution_loss() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let total: u32 = <Test as crate::Config>::MaxActiveActors::get();
    let crossing_count = total / 2;
    let change_count = total.saturating_sub(crossing_count).saturating_sub(1);
    crate::ActiveActorLimit::<Test>::put(total);
    let steps = contract_steps_with_step(make_step(Task::StopCycle));
    let mut crossing_actors = Vec::with_capacity(crossing_count as usize);
    let mut change_actors = Vec::with_capacity(change_count as usize);
    for index in 0..crossing_count {
      crossing_actors.push(create_system_with(
        100_000 + u64::from(index),
        Schedule {
          trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
          cooldown_blocks: 0,
        },
        None,
        steps.clone(),
      ));
    }
    for index in 0..change_count {
      change_actors.push(create_system_with(
        200_000 + u64::from(index),
        Schedule {
          trigger: RuntimeTrigger::observation_change(8),
          cooldown_blocks: 0,
        },
        None,
        steps.clone(),
      ));
    }
    let cadence_actor = create_system_with(
      300_000,
      Schedule {
        trigger: RuntimeTrigger::cadenced(10),
        cooldown_blocks: 0,
      },
      None,
      steps.clone(),
    );
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    assert_ok!(Actors::note_observation_changed(8, 1));

    for block in 2..=total.saturating_mul(2) {
      frame_system::Pallet::<Test>::set_block_number(block.into());
      run_idle(Weight::MAX);
      if Actors::crossing_transition_queue(7).is_none()
        && Actors::dirty_observation_feeds(8).is_none()
      {
        break;
      }
    }
    assert!(Actors::crossing_transition_queue(7).is_none());
    assert!(Actors::dirty_observation_feeds(8).is_none());
    let cadence_identity = ActorIdentities::<Test>::get(cadence_actor).expect("cadence identity");
    let cadence_hot = ActorHot::<Test>::get(cadence_actor).expect("cadence hot state");
    assert_eq!(cadence_identity.cycle_nonce, 0);
    assert!(cadence_hot.pending_signal);
    assert!(
      cadence_hot.queue_ticket.is_some() || cadence_hot.wakeup_pointer.is_some(),
      "cadence must retain canonical queue or backpressure-retry placement under breaker"
    );
    for actor_id in crossing_actors {
      let identity = ActorIdentities::<Test>::get(actor_id).expect("Crossing actor identity");
      let hot = ActorHot::<Test>::get(actor_id).expect("Crossing actor hot state");
      assert_eq!(identity.cycle_nonce, 0);
      assert!(hot.pending_signal);
      assert_eq!(
        hot.trigger_runtime_state,
        TriggerRuntimeState::ObservationCrossing {
          phase: CrossingPhase::WaitingForRearm,
          installed_at_revision: 1,
        }
      );
    }
    for actor_id in change_actors {
      let identity = ActorIdentities::<Test>::get(actor_id).expect("Change actor identity");
      let hot = ActorHot::<Test>::get(actor_id).expect("Change actor hot state");
      assert_eq!(identity.cycle_nonce, 0);
      assert!(hot.pending_signal);
    }
  });
}

#[test]
#[ignore] // Release mixed-branch profile; run through scripts/actors-assurance.sh.
fn crossing_mixed_dense_sparse_directional_lifecycle_profile() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    for (feed, value) in [(7, 50), (8, 150)] {
      set_observation(
        feed,
        crate::ScalarObservationState::Fresh {
          value,
          observed_at: 1,
        },
      );
    }
    let steps = contract_steps_with_step(make_step(Task::StopCycle));
    let mut rising = Vec::new();
    for index in 0..3u128 {
      rising.push(create_system_with(
        400_000 + index as u64,
        Schedule {
          trigger: RuntimeTrigger::observation_crossing(
            7,
            CrossingDirection::Rising,
            100 + index.saturating_mul(10),
            70 + index,
          ),
          cooldown_blocks: 0,
        },
        None,
        steps.clone(),
      ));
    }
    let dense = (0..3u64)
      .map(|index| {
        create_system_with(
          410_000 + index,
          Schedule {
            trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
            cooldown_blocks: 0,
          },
          None,
          steps.clone(),
        )
      })
      .collect::<Vec<_>>();
    let falling = (0..3u128)
      .map(|index| {
        create_system_with(
          420_000 + index as u64,
          Schedule {
            trigger: RuntimeTrigger::observation_crossing(
              8,
              CrossingDirection::Falling,
              100 - index.saturating_mul(10),
              120 + index,
            ),
            cooldown_blocks: 0,
          },
          None,
          steps.clone(),
        )
      })
      .collect::<Vec<_>>();
    assert_ok!(Actors::pause_actor(
      RuntimeOrigin::signed(400_000),
      rising[0]
    ));
    Actors::request_activation(dense[0]).expect("fixture latch placement succeeds");

    let user_steps = transfer_contract_steps(BOB, 1);
    let insolvent = create_user_with(
      ALICE,
      Mutability::Mutable,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 130, 90),
        cooldown_blocks: 0,
      },
      None,
      user_steps,
    );
    let insolvent_balance = native_balance(&sovereign_account(insolvent));
    deplete_user_sovereign(insolvent, insolvent_balance);

    let event_count_before_materialization = frame_system::Pallet::<Test>::events().len();
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 200,
      },
    ));
    assert_ok!(Actors::note_observation_transition(
      8,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(150),
        current: 0,
      },
    ));
    let mut passes = 0u32;
    while Actors::crossing_pending_feed_list().count > 0 {
      passes = passes.saturating_add(1);
      assert!(
        passes <= 16,
        "mixed profile must drain under bounded passes"
      );
      Actors::service_crossing_transitions(Weight::MAX);
    }

    assert!(
      passes > 1,
      "mixed branch topology must span more than one component-limited pass"
    );
    let events = frame_system::Pallet::<Test>::events();
    let materialization_events = &events[event_count_before_materialization..];
    assert_eq!(materialization_events.len(), 8);
    assert!(materialization_events.iter().all(|record| matches!(
      record.event,
      RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
        trigger_family: TriggerFamily::ObservationCrossing,
        ..
      })
    )));
    assert!(
      rising
        .into_iter()
        .chain(dense)
        .chain(falling)
        .all(|actor_id| { ActorHot::<Test>::get(actor_id).is_some_and(|hot| hot.pending_signal) })
    );
    let insolvent_hot = ActorHot::<Test>::get(insolvent).expect("insolvent Actor remains active");
    assert!(!insolvent_hot.pending_signal);
    assert!(insolvent_hot.queue_ticket.is_none());
    assert_eq!(crossing_phase(insolvent), CrossingPhase::WaitingForRearm);
    assert!(Actors::active_actor_view(insolvent).is_some());
  });
}

#[test]
fn crossing_contract_replacement_removes_stale_pending_membership_before_worker_service() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      manual_schedule(),
      None,
    ));
    drain_crossing_work();
    assert!(Actors::crossing_membership(actor_id).is_none());
    let hot = ActorHot::<Test>::get(actor_id).expect("hot state");
    assert!(!hot.pending_signal);
    assert!(hot.queue_ticket.is_none());
  });
}

#[test]
fn semantic_crossing_replacement_reinitializes_from_current_observation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    drain_crossing_work();
    assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 150,
        observed_at: 1,
      },
    );
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 200, 180),
        cooldown_blocks: 0,
      },
      None,
    ));
    assert_eq!(crossing_phase(actor_id), CrossingPhase::Armed);
    let hot = ActorHot::<Test>::get(actor_id).expect("active Crossing actor");
    assert!(hot.pending_signal);
    assert!(hot.queue_ticket.is_some());
    let locator = Actors::crossing_membership(actor_id).expect("replacement membership");
    assert_eq!(locator.key.threshold, 200);
  });
}

#[test]
fn crossing_deactivation_and_close_remove_pending_members_without_stale_activation() {
  for deactivate in [true, false] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      set_observation(
        7,
        crate::ScalarObservationState::Fresh {
          value: 50,
          observed_at: 1,
        },
      );
      let actor_id = create_system_with(
        ALICE,
        Schedule {
          trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
          cooldown_blocks: 0,
        },
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      assert_ok!(Actors::note_observation_transition(
        7,
        crate::ObservationTransition {
          revision: 2,
          previous: Some(50),
          current: 150,
        },
      ));
      if deactivate {
        assert_ok!(Actors::deactivate_actor(
          RuntimeOrigin::signed(ALICE),
          actor_id
        ));
      } else {
        assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
      }
      drain_crossing_work();
      assert!(Actors::crossing_membership(actor_id).is_none());
      assert!(ActorHot::<Test>::get(actor_id).is_none());
      assert_eq!(Actors::combined_queue_occupancy(), 0);
    });
  }
}

#[test]
fn latched_crossing_rearm_transition_uses_disabled_skip_without_activation_probe() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    drain_crossing_work();
    assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 3,
        previous: Some(150),
        current: 70,
      },
    ));
    let (_, counters) = Actors::service_crossing_transitions_with_counters(Weight::MAX);
    assert_eq!(counters.candidates, 1);
    assert_eq!(counters.canonical_probes, 0);
    assert_eq!(counters.activations, 0);
    assert_eq!(counters.closes, 0);
    assert_eq!(counters.faults, 0);
    assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
  });
}

#[test]
fn crossing_compact_authority_mismatch_truncates_at_first_middle_and_final_boundaries() {
  for mismatch_index in [0usize, 1, 3] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      set_observation(
        7,
        crate::ScalarObservationState::Fresh {
          value: 50,
          observed_at: 1,
        },
      );
      let actors = [ALICE, BOB, CHARLIE, 44]
        .into_iter()
        .map(|owner| {
          create_system_with(
            owner,
            Schedule {
              trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
              cooldown_blocks: 0,
            },
            None,
            contract_steps_with_step(make_step(Task::StopCycle)),
          )
        })
        .collect::<Vec<_>>();
      let locator = Actors::crossing_membership(actors[0]).expect("first locator");
      let page = CrossingMemberPages::<Test>::get(locator.key, locator.page)
        .expect("compact-authority source page");
      let snapshot = Actors::snapshot_crossing_source_prefix(
        locator.key,
        locator.page,
        &page,
        locator.offset,
        4,
      )
      .expect("four-candidate compact snapshot");
      crate::ActorActivationAuthorities::<Test>::mutate(
        actors[mismatch_index],
        |maybe_authority| {
          maybe_authority
            .as_mut()
            .expect("activation authority")
            .admission_identity = [0xA5; 32];
        },
      );
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
      let preflight = Actors::preflight_crossing_cohort(
        &snapshot,
        crate::CrossingTransitionObligation {
          revision: 2,
          previous: 50,
          current: 150,
          cause_provenance: crate::TriggerCauseProvenance::Deferred,
          cause_block: 1,
        },
        true,
        Some(crate::CrossingWorkPlan::FireCohortPlaced),
      )
      .expect("mismatch must select an exact scalar fallback or compact prefix");
      assert_eq!(
        preflight.admitted_candidates,
        if mismatch_index == 0 {
          1
        } else {
          mismatch_index as u32
        },
      );
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        root_before,
        "preflight mismatch handling must remain read-only"
      );
    });
  }
}

#[test]
fn crossing_same_tail_page_places_two_candidates_in_one_admitted_cohort() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let schedule = Schedule {
      trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
      cooldown_blocks: 0,
    };
    let steps = contract_steps_with_step(make_step(Task::StopCycle));
    let first = create_system_with(ALICE, schedule.clone(), None, steps.clone());
    let second = create_system_with(BOB, schedule, None, steps);
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    crate::CrossingRangeCursors::<Test>::insert(
      7,
      crate::CrossingRangeCursor {
        revision: 2,
        traversal: crate::CrossingTraversal::Upward,
        search_bound: 150,
        current_threshold: Some(100),
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    let queue_capacity: u32 = <Test as crate::Config>::MaxQueueLength::get();
    QueueOccupancy::<Test>::put(queue_capacity - 1);
    assert_eq!(
      Actors::classify_crossing_work(),
      crate::CrossingWorkPlan::FireCohortPlaced,
      "a pair must downgrade before mutation when only one FIFO position remains"
    );
    QueueOccupancy::<Test>::put(0);
    crate::NextQueueTicket::<Test>::put(u64::MAX - 1);
    assert_eq!(
      Actors::classify_crossing_work(),
      crate::CrossingWorkPlan::FireCohortPlaced,
      "a pair must downgrade before its second ticket would exhaust the allocator"
    );
    crate::NextQueueTicket::<Test>::put(0);
    QueueTail::<Test>::put(1);
    let corrupt_root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_eq!(
      Actors::classify_crossing_work(),
      crate::CrossingWorkPlan::FireCohortPlaced,
      "corrupt aggregate FIFO authority must not admit a pair"
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      corrupt_root,
      "pair classification must retain no partial queue authority"
    );
    QueueTail::<Test>::put(0);
    assert_eq!(
      Actors::classify_crossing_work(),
      crate::CrossingWorkPlan::FireCohortPlacedBatch
    );
    Actors::test_reset_queue_append_commits();
    Actors::test_reset_crossing_cursor_commits();
    let (_, counters) = Actors::service_crossing_transitions_with_counters(Weight::MAX);
    assert_eq!(counters.candidates, 2);
    assert_eq!(counters.canonical_probes, 2);
    assert_eq!(counters.activations, 2);
    assert_eq!(Actors::test_queue_append_commits(), 1);
    assert_eq!(Actors::test_crossing_cursor_commits(), 1);
    assert_eq!(counters.closes, 0);
    assert_eq!(counters.faults, 0);
    for actor_id in [first, second] {
      let hot = ActorHot::<Test>::get(actor_id).expect("cohort actor remains active");
      assert!(hot.pending_signal);
      assert!(hot.queue_ticket.is_some());
      assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
    }

    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 3,
        previous: Some(150),
        current: 50,
      },
    ));
    crate::CrossingRangeCursors::<Test>::insert(
      7,
      crate::CrossingRangeCursor {
        revision: 3,
        traversal: crate::CrossingTraversal::Downward,
        search_bound: 50,
        current_threshold: Some(80),
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    assert_eq!(
      Actors::classify_crossing_work(),
      crate::CrossingWorkPlan::SkipPostInstallationPair
    );
    let (_, rearm_counters) = Actors::service_crossing_transitions_with_counters(Weight::MAX);
    assert_eq!(rearm_counters.candidates, 2);
    assert_eq!(rearm_counters.canonical_probes, 0);
    assert_eq!(rearm_counters.activations, 0);
    assert_eq!(rearm_counters.closes, 0);
    assert_eq!(rearm_counters.faults, 0);
    for actor_id in [first, second] {
      assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
    }
  });
}

#[test]
fn crossing_maximum_partial_page_batch_preserves_remainder_and_next_feed() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    for feed in [7, 8] {
      set_observation(
        feed,
        crate::ScalarObservationState::Fresh {
          value: 50,
          observed_at: 1,
        },
      );
    }
    let schedule = |feed| Schedule {
      trigger: RuntimeTrigger::observation_crossing(feed, CrossingDirection::Rising, 100, 80),
      cooldown_blocks: 0,
    };
    let steps = contract_steps_with_step(make_step(Task::StopCycle));
    let first = create_system_with(ALICE, schedule(7), None, steps.clone());
    let second = create_system_with(BOB, schedule(7), None, steps.clone());
    let third = create_system_with(44, schedule(7), None, steps.clone());
    let fourth = create_system_with(55, schedule(7), None, steps.clone());
    let retained = create_system_with(66, schedule(7), None, steps.clone());
    let isolated = create_system_with(CHARLIE, schedule(8), None, steps);
    for feed in [7, 8] {
      assert_ok!(Actors::note_observation_transition(
        feed,
        crate::ObservationTransition {
          revision: 2,
          previous: Some(50),
          current: 150,
        },
      ));
    }
    crate::CrossingRangeCursors::<Test>::insert(
      7,
      crate::CrossingRangeCursor {
        revision: 2,
        traversal: crate::CrossingTraversal::Upward,
        search_bound: 150,
        current_threshold: Some(100),
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    assert_eq!(
      Actors::classify_crossing_work(),
      crate::CrossingWorkPlan::FireCohortPlacedBatch
    );
    let cap: u32 = <Test as crate::Config>::MaxCrossingActorsPerBlock::get();
    Actors::test_reset_queue_append_commits();
    Actors::test_reset_crossing_cursor_commits();
    let (_, counters) = Actors::service_crossing_transitions_resuming(
      Weight::MAX,
      crate::crossing::CrossingWorkCounters {
        candidates: cap - 4,
        ..Default::default()
      },
    );
    assert_eq!(counters.candidates, cap);
    assert_eq!(Actors::test_queue_append_commits(), 1);
    assert_eq!(Actors::test_crossing_cursor_commits(), 1);
    for actor_id in [first, second, third, fourth] {
      assert!(ActorHot::<Test>::get(actor_id).is_some_and(|hot| hot.pending_signal));
      assert_eq!(crossing_phase(actor_id), CrossingPhase::WaitingForRearm);
    }
    let retained_hot = ActorHot::<Test>::get(retained).expect("retained source actor");
    assert!(!retained_hot.pending_signal);
    assert!(retained_hot.queue_ticket.is_none());
    assert_eq!(crossing_phase(retained), CrossingPhase::Armed);
    assert_eq!(
      Actors::crossing_membership(retained).map(|locator| locator.offset),
      Some(0)
    );
    let isolated_hot = ActorHot::<Test>::get(isolated).expect("isolated feed actor");
    assert!(!isolated_hot.pending_signal);
    assert!(isolated_hot.queue_ticket.is_none());
    assert_eq!(crossing_phase(isolated), CrossingPhase::Armed);
    assert!(Actors::crossing_transition_queue(8).is_some());
  });
}

#[test]
fn crossing_pair_partial_cohort_preserves_second_actor_installation_barrier() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let schedule = Schedule {
      trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
      cooldown_blocks: 0,
    };
    let steps = contract_steps_with_step(make_step(Task::StopCycle));
    let first = create_system_with(ALICE, schedule.clone(), None, steps.clone());
    let second = create_system_with(BOB, schedule, None, steps);
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    crate::CrossingRangeCursors::<Test>::insert(
      7,
      crate::CrossingRangeCursor {
        revision: 2,
        traversal: crate::CrossingTraversal::Upward,
        search_bound: 150,
        current_threshold: Some(100),
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    ActorHot::<Test>::mutate(second, |maybe_hot| {
      let hot = maybe_hot.as_mut().expect("second cohort Actor exists");
      let TriggerRuntimeState::ObservationCrossing {
        installed_at_revision,
        ..
      } = &mut hot.trigger_runtime_state
      else {
        panic!("second cohort Actor uses Crossing state");
      };
      *installed_at_revision = 2;
    });

    assert_eq!(
      Actors::classify_crossing_work(),
      crate::CrossingWorkPlan::FireCohortPlaced,
      "the incompatible second candidate must not enter the admitted pair"
    );
    let (_, counters) = Actors::service_crossing_transitions_with_counters(Weight::MAX);
    assert_eq!(counters.candidates, 2);
    assert_eq!(counters.canonical_probes, 1);
    assert_eq!(counters.activations, 1);
    assert_eq!(counters.closes, 0);
    assert_eq!(counters.faults, 0);
    assert!(ActorHot::<Test>::get(first).is_some_and(|hot| hot.pending_signal));
    assert_eq!(crossing_phase(first), CrossingPhase::WaitingForRearm);
    assert!(
      ActorHot::<Test>::get(second)
        .is_some_and(|hot| { !hot.pending_signal && hot.queue_ticket.is_none() })
    );
    assert_eq!(crossing_phase(second), CrossingPhase::Armed);
  });
}

#[test]
fn crossing_pair_preflight_fault_cannot_activate_its_valid_first_candidate() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let schedule = Schedule {
      trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
      cooldown_blocks: 0,
    };
    let steps = contract_steps_with_step(make_step(Task::StopCycle));
    let first = create_system_with(ALICE, schedule.clone(), None, steps.clone());
    let second = create_system_with(BOB, schedule, None, steps);
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    crate::CrossingRangeCursors::<Test>::insert(
      7,
      crate::CrossingRangeCursor {
        revision: 2,
        traversal: crate::CrossingTraversal::Upward,
        search_bound: 150,
        current_threshold: Some(100),
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    let first_before = ActorHot::<Test>::get(first).expect("first cohort Actor exists");
    let second_before = ActorHot::<Test>::get(second).expect("second cohort Actor exists");
    crate::CrossingMemberships::<Test>::mutate(second, |maybe_locator| {
      let locator = maybe_locator
        .as_mut()
        .expect("second cohort locator exists");
      locator.generation = locator.generation.saturating_add(1);
    });

    let (_, counters) = Actors::service_crossing_transitions_with_counters(Weight::MAX);
    assert_eq!(counters.candidates, 0);
    assert_eq!(counters.canonical_probes, 0);
    assert_eq!(counters.activations, 0);
    assert_eq!(counters.closes, 0);
    assert_eq!(counters.faults, 1);
    assert_eq!(ActorHot::<Test>::get(first), Some(first_before));
    assert_eq!(ActorHot::<Test>::get(second), Some(second_before));
    assert_eq!(crate::QueueOccupancy::<Test>::get(), 0);
    let fault = crate::CrossingWorkerFaultState::<Test>::get().expect("Crossing fault recorded");
    assert_eq!(
      actor_event_count(|event| matches!(
        event,
        Event::ActorFaultRecorded {
          fault_id: crate::FaultId::CrossingWorker,
          kind: crate::ActorFaultKind::Detector,
          first_recorded_block: 1,
          context: crate::FaultContext::Crossing(recorded),
        } if recorded == &fault
      )),
      1
    );
    let _ = Actors::service_crossing_transitions_with_counters(Weight::MAX);
    assert_eq!(
      actor_event_count(|event| matches!(
        event,
        Event::ActorFaultRecorded {
          fault_id: crate::FaultId::CrossingWorker,
          ..
        }
      )),
      1,
      "an uncleared Crossing fault emits only its first-recorded event"
    );
  });
}

#[test]
fn crossing_fault_recording_admits_both_weight_dimensions_and_is_idempotent() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let fault = crate::CrossingWorkerFault {
      feed: 7,
      revision: Some(2),
      threshold: Some(100),
      class: crate::CrossingWorkerFaultClass::Invariant,
    };
    let required = <TestWeightInfo as crate::WeightInfo>::record_crossing_worker_fault();

    let mut ref_time_short = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::from_parts(
      required.ref_time().saturating_sub(1),
      u64::MAX,
    ));
    assert!(!Actors::record_crossing_worker_fault(
      &mut ref_time_short,
      fault
    ));
    assert!(crate::CrossingWorkerFaultState::<Test>::get().is_none());
    assert_eq!(System::events().len(), 0);

    let mut proof_short = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::from_parts(
      u64::MAX,
      required.proof_size().saturating_sub(1),
    ));
    assert!(!Actors::record_crossing_worker_fault(
      &mut proof_short,
      fault
    ));
    assert!(crate::CrossingWorkerFaultState::<Test>::get().is_none());
    assert_eq!(System::events().len(), 0);

    let mut admitted = polkadot_sdk::sp_weights::WeightMeter::with_limit(required);
    assert!(Actors::record_crossing_worker_fault(&mut admitted, fault));
    assert_eq!(admitted.consumed(), required);
    let events_after_first = System::events();

    let mut duplicate = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    assert!(!Actors::record_crossing_worker_fault(
      &mut duplicate,
      crate::CrossingWorkerFault {
        class: crate::CrossingWorkerFaultClass::Other,
        ..fault
      },
    ));
    assert_eq!(duplicate.consumed(), Weight::zero());
    assert_eq!(crate::CrossingWorkerFaultState::<Test>::get(), Some(fault));
    assert_eq!(System::events(), events_after_first);
  });
}

#[test]
fn crossing_terminal_weight_is_separate_from_ordinary_placement() {
  new_test_ext().execute_with(|| {
    let ordinary = <TestWeightInfo as crate::WeightInfo>::crossing_placed_unit();
    let terminal = <TestWeightInfo as crate::WeightInfo>::crossing_actor_unit();
    assert_eq!(
      Actors::crossing_plan_weight(crate::CrossingWorkPlan::FireCohortPlaced),
      ordinary
    );
    assert_eq!(
      Actors::crossing_plan_weight(crate::CrossingWorkPlan::FireCohortClosed),
      terminal
    );
    assert_ne!(ordinary, terminal);
  });
}

#[test]
fn crossing_work_plan_is_read_only_and_classifies_search_before_execution() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let base = <TestWeightInfo as crate::WeightInfo>::crossing_worker_base();
    let probe = <TestWeightInfo as crate::WeightInfo>::crossing_work_probe();
    let short = Weight::from_parts(
      base
        .ref_time()
        .saturating_add(probe.ref_time())
        .saturating_sub(1),
      u64::MAX,
    );
    assert_eq!(Actors::service_crossing_transitions(short), base);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
    assert!(Actors::crossing_worker_fault().is_none());
    assert_eq!(
      Actors::classify_crossing_work(),
      crate::CrossingWorkPlan::OpenLeaf
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
    let cheap_branch = base
      .saturating_add(probe)
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::crossing_transition_unit());
    assert_eq!(
      Actors::service_crossing_transitions(cheap_branch),
      base.saturating_add(probe)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
    let (_, counters) = Actors::service_crossing_transitions_with_counters(Weight::MAX);
    assert!(Actors::pending_signal(actor_id));
    assert_eq!(counters.candidates, 1);
    assert_eq!(counters.canonical_probes, 1);
    assert_eq!(counters.activations, 1);
    assert_eq!(counters.closes, 0);
    assert_eq!(counters.faults, 0);
  });
}

#[test]
fn crossing_seek_miss_uses_only_probe_and_transition_branch_weight() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 200, 180),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    assert_eq!(
      Actors::classify_crossing_work(),
      crate::CrossingWorkPlan::SeekMiss
    );
    let probe_expected = <TestWeightInfo as crate::WeightInfo>::crossing_worker_base()
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::crossing_work_probe());
    let consumed_expected = probe_expected
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::crossing_transition_unit());
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let (deferred, deferred_counters) =
      Actors::service_crossing_transitions_with_counters(consumed_expected);
    assert_eq!(deferred, probe_expected);
    assert_eq!(deferred_counters, Default::default());
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before,
      "the branch must not mutate without its first-fault reserve"
    );

    let budget = consumed_expected
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::record_crossing_worker_fault());
    let (consumed, counters) = Actors::service_crossing_transitions_with_counters(budget);
    assert_eq!(consumed, consumed_expected);
    assert_eq!(
      counters,
      crate::crossing::CrossingWorkCounters {
        transitions: 1,
        ..Default::default()
      }
    );
    assert!(Actors::crossing_range_cursor(7).is_some_and(|cursor| cursor.exhausted));
    assert!(!Actors::pending_signal(actor_id));
  });
}

#[test]
fn crossing_worker_rejects_stale_generation_without_partial_progress() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    let locator = Actors::crossing_membership(actor_id).expect("membership exists");
    crate::CrossingMemberPages::<Test>::mutate(locator.key, locator.page, |maybe| {
      maybe.as_mut().expect("member page exists").entries[locator.offset as usize].generation += 1;
    });
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_noop!(
      Actors::crossing_work_unit(),
      Error::<Test>::CrossingIndexInvariant
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn crossing_service_records_one_bounded_fault_and_halts_until_repair() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    let locator = Actors::crossing_membership(actor_id).expect("membership exists");
    let deferred_queue = Actors::crossing_transition_queue(7);
    Actors::service_crossing_transitions(
      <TestWeightInfo as crate::WeightInfo>::crossing_worker_base(),
    );
    assert_eq!(Actors::crossing_transition_queue(7), deferred_queue);
    assert!(Actors::crossing_worker_fault().is_none());
    crate::CrossingMemberPages::<Test>::mutate(locator.key, locator.page, |maybe| {
      maybe.as_mut().expect("member page exists").entries[locator.offset as usize].generation += 1;
    });
    let queue_before = Actors::crossing_transition_queue(7);
    let (first, fault_counters) = Actors::service_crossing_transitions_with_counters(Weight::MAX);
    assert!(first.ref_time() > 0 && first.proof_size() > 0);
    assert_eq!(fault_counters.faults, 1);
    assert_eq!(fault_counters.activations, 0);
    assert_eq!(fault_counters.closes, 0);
    assert!(Actors::actor_hot(actor_id).is_some_and(|hot| {
      !hot.pending_signal
        && hot.queue_ticket.is_none()
        && matches!(
          hot.trigger_runtime_state,
          TriggerRuntimeState::ObservationCrossing {
            phase: CrossingPhase::Armed,
            ..
          }
        )
    }));
    assert_eq!(Actors::crossing_transition_queue(7), queue_before);
    assert_eq!(
      Actors::crossing_worker_fault(),
      Some(crate::CrossingWorkerFault {
        feed: 7,
        revision: Some(2),
        threshold: None,
        class: crate::CrossingWorkerFaultClass::Invariant,
      })
    );
    let second = Actors::service_crossing_transitions(Weight::MAX);
    assert_eq!(
      second,
      <TestWeightInfo as crate::WeightInfo>::crossing_worker_base()
    );
    assert_eq!(Actors::crossing_transition_queue(7), queue_before);

    crate::CrossingMemberPages::<Test>::mutate(locator.key, locator.page, |maybe| {
      maybe.as_mut().expect("member page exists").entries[locator.offset as usize].generation -= 1;
    });
    assert_noop!(
      Actors::clear_crossing_worker_fault(RuntimeOrigin::signed(ALICE)),
      DispatchError::BadOrigin
    );
    assert_ok!(Actors::clear_crossing_worker_fault(RuntimeOrigin::root()));
    assert!(Actors::crossing_worker_fault().is_none());
    Actors::service_crossing_transitions(Weight::MAX);
    assert!(Actors::crossing_transition_queue(7).is_none());
    assert!(Actors::actor_hot(actor_id).is_some_and(|hot| hot.pending_signal));
  });
}

#[test]
fn crossing_generation_exhaustion_rejects_replacement_before_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let locator = Actors::crossing_membership(actor_id).expect("membership exists");
    crate::CrossingMemberships::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("membership exists").generation = u64::MAX;
    });
    crate::CrossingMemberPages::<Test>::mutate(locator.key, locator.page, |maybe| {
      maybe.as_mut().expect("member page exists").entries[locator.offset as usize].generation =
        u64::MAX;
    });
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        Schedule {
          trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 120, 90,),
          cooldown_blocks: 0,
        },
        None,
      ),
      Error::<Test>::CrossingGenerationExhausted
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn crossing_activation_under_saturated_fifo_latches_and_defers_exactly_once() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    seed_saturated_tombstone_queue();
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    drain_crossing_work();
    let hot = ActorHot::<Test>::get(actor_id).expect("hot state");
    assert!(hot.pending_signal);
    assert!(hot.queue_ticket.is_none());
    assert_eq!(
      hot.wakeup_pointer.map(|pointer| pointer.block),
      Some(WakeupKey::Block(2))
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    let recovered = ActorHot::<Test>::get(actor_id).expect("actor survives StopCycle");
    assert!(!recovered.pending_signal);
    assert!(recovered.queue_ticket.is_none());
    assert!(recovered.wakeup_pointer.is_none());
    assert_eq!(
      ActorIdentities::<Test>::get(actor_id)
        .expect("actor identity")
        .cycle_nonce,
      1
    );
  });
}

#[test]
fn crossing_ticket_exhaustion_closes_and_cleans_detection_membership() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    crate::NextQueueTicket::<Test>::put(u64::MAX);
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    let (_, counters) = Actors::service_crossing_transitions_with_counters(Weight::MAX);
    assert_eq!(counters.candidates, 1);
    assert_eq!(counters.canonical_probes, 1);
    assert_eq!(counters.activations, 1);
    assert_eq!(counters.closes, 1);
    assert_eq!(counters.faults, 0);
    assert!(Actors::active_actor_state(actor_id).is_none());
    assert!(Actors::crossing_membership(actor_id).is_none());
    assert_eq!(Actors::crossing_feed_membership_count(7), 0);
    assert_eq!(Actors::combined_queue_occupancy(), 0);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::SchedulerIndexExhausted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn crossing_queue_index_exhaustion_rolls_back_the_actor_unit_exactly() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    crate::QueueTail::<Test>::put(u64::MAX);
    crate::NextQueueTicket::<Test>::put(5);
    QueueOccupancy::<Test>::put(0);
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_noop!(
      Actors::crossing_work_unit(),
      Error::<Test>::SchedulerIndexExhausted
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
    let hot = ActorHot::<Test>::get(actor_id).expect("actor remains active");
    assert!(matches!(
      hot.trigger_runtime_state,
      TriggerRuntimeState::ObservationCrossing {
        phase: CrossingPhase::Armed,
        ..
      }
    ));
    assert!(!hot.pending_signal);
    assert!(hot.queue_ticket.is_none());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn crossing_try_state_rejects_canonical_locator_radix_and_pending_list_corruption() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    assert_ok!(crate::Pallet::<Test>::do_try_state());

    let locator = Actors::crossing_membership(actor_id).expect("membership exists");
    for corrupt in [
      crate::CrossingMembershipLocator {
        key: crate::CrossingLeafKey {
          threshold: locator.key.threshold.saturating_add(1),
          ..locator.key
        },
        ..locator
      },
      crate::CrossingMembershipLocator {
        page: locator.page.saturating_add(1),
        ..locator
      },
      crate::CrossingMembershipLocator {
        offset: locator.offset.saturating_add(1),
        ..locator
      },
      crate::CrossingMembershipLocator {
        generation: locator.generation.saturating_add(1),
        ..locator
      },
    ] {
      crate::CrossingMemberships::<Test>::insert(actor_id, corrupt);
      assert!(crate::Pallet::<Test>::do_try_state().is_err());
      crate::CrossingMemberships::<Test>::insert(actor_id, locator);
    }
    crate::CrossingMemberships::<Test>::remove(actor_id);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::CrossingMemberships::<Test>::insert(actor_id, locator);

    let other_actor_id = create_system_with(
      BOB,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let original_page = crate::CrossingMemberPages::<Test>::get(locator.key, locator.page)
      .expect("membership page exists");
    crate::CrossingMemberPages::<Test>::mutate(locator.key, locator.page, |maybe_page| {
      maybe_page
        .as_mut()
        .expect("membership page exists")
        .entries
        .remove(locator.offset as usize);
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::CrossingMemberPages::<Test>::insert(locator.key, locator.page, original_page.clone());

    crate::CrossingMemberPages::<Test>::mutate(locator.key, locator.page, |maybe_page| {
      let page = maybe_page.as_mut().expect("membership page exists");
      page
        .entries
        .try_push(page.entries[locator.offset as usize])
        .expect("test membership page has duplicate capacity");
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::CrossingMemberPages::<Test>::insert(locator.key, locator.page, original_page.clone());

    crate::CrossingMemberPages::<Test>::mutate(locator.key, locator.page, |maybe_page| {
      maybe_page.as_mut().expect("membership page exists").entries[locator.offset as usize]
        .actor_id = other_actor_id;
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::CrossingMemberPages::<Test>::insert(locator.key, locator.page, original_page);
    assert_ok!(crate::Pallet::<Test>::do_try_state());

    let identity = ActorIdentities::<Test>::get(actor_id).expect("actor identity exists");
    let hot = ActorHot::<Test>::get(actor_id).expect("actor hot state exists");
    let contract = Actors::load_actor_contract(actor_id).expect("actor contract exists");
    let funding = ActorFunding::<Test>::get(actor_id).expect("actor funding exists");
    ActorIdentities::<Test>::remove(actor_id);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    ActorIdentities::<Test>::insert(actor_id, identity);
    ActorHot::<Test>::remove(actor_id);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    ActorHot::<Test>::insert(actor_id, hot.clone());
    assert!(Actors::remove_admitted_contract_geometry(actor_id).is_some());
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    assert_ok!(Actors::store_actor_contract(actor_id, contract));
    ActorFunding::<Test>::remove(actor_id);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    ActorFunding::<Test>::insert(actor_id, funding);

    let runtime_state = hot.trigger_runtime_state;
    ActorHot::<Test>::mutate(actor_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("active Crossing actor")
        .trigger_runtime_state = TriggerRuntimeState::Stateless;
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    ActorHot::<Test>::mutate(actor_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("active Crossing actor")
        .trigger_runtime_state = runtime_state.clone();
    });
    assert_ok!(crate::Pallet::<Test>::do_try_state());

    ActorHot::<Test>::mutate(actor_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("active Crossing actor")
        .trigger_runtime_state = TriggerRuntimeState::ObservationCrossing {
        phase: CrossingPhase::WaitingForRearm,
        installed_at_revision: u64::MAX,
      };
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    ActorHot::<Test>::mutate(actor_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("active Crossing actor")
        .trigger_runtime_state = runtime_state;
    });
    assert_ok!(crate::Pallet::<Test>::do_try_state());

    let cadence_actor = create_system_with(
      CHARLIE,
      Schedule {
        trigger: RuntimeTrigger::cadenced(10),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    ActorHot::<Test>::mutate(cadence_actor, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("active cadence actor")
        .trigger_runtime_state = TriggerRuntimeState::Stateless;
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    ActorHot::<Test>::mutate(cadence_actor, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("active cadence actor")
        .trigger_runtime_state = TriggerRuntimeState::Cadenced { anchor_tick: None };
    });
    assert_ok!(crate::Pallet::<Test>::do_try_state());

    let (radix_key, bitmap) = crate::CrossingRadixNodes::<Test>::iter()
      .next()
      .expect("radix path exists");
    crate::CrossingRadixNodes::<Test>::remove(radix_key);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::CrossingRadixNodes::<Test>::insert(radix_key, bitmap);
    crate::CrossingRadixNodes::<Test>::insert(radix_key, 0);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::CrossingRadixNodes::<Test>::insert(radix_key, bitmap);
    crate::CrossingRadixNodes::<Test>::insert(radix_key, bitmap ^ 1);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::CrossingRadixNodes::<Test>::insert(radix_key, bitmap);

    let leaf_state =
      crate::CrossingLeafStates::<Test>::get(locator.key).expect("Crossing leaf state exists");
    crate::CrossingLeafStates::<Test>::remove(locator.key);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::CrossingLeafStates::<Test>::insert(locator.key, leaf_state);
    let extra_leaf_key = crate::CrossingLeafKey {
      threshold: locator.key.threshold.saturating_add(1),
      ..locator.key
    };
    crate::CrossingLeafStates::<Test>::insert(extra_leaf_key, leaf_state);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::CrossingLeafStates::<Test>::remove(extra_leaf_key);

    let feed_count = crate::CrossingFeedMembershipCount::<Test>::get(locator.key.feed);
    crate::CrossingFeedMembershipCount::<Test>::insert(
      locator.key.feed,
      feed_count.saturating_add(1),
    );
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::CrossingFeedMembershipCount::<Test>::insert(locator.key.feed, feed_count);

    let page = crate::CrossingMemberPages::<Test>::get(locator.key, locator.page)
      .expect("Crossing member page exists");
    let orphan_page = locator.page.saturating_add(100);
    crate::CrossingMemberPages::<Test>::insert(locator.key, orphan_page, page);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::CrossingMemberPages::<Test>::remove(locator.key, orphan_page);
    assert_ok!(crate::Pallet::<Test>::do_try_state());

    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 110,
      },
    ));
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 3,
        previous: Some(110),
        current: 50,
      },
    ));
    assert_ok!(crate::Pallet::<Test>::do_try_state());

    let list = crate::CrossingPendingFeedListState::<Test>::get();
    let pending = crate::CrossingPendingFeeds::<Test>::get(7).expect("pending feed exists");
    let queue = crate::CrossingTransitionQueues::<Test>::get(7).expect("transition queue exists");

    crate::CrossingPendingFeedListState::<Test>::mutate(|stored| stored.count += 1);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::CrossingPendingFeedListState::<Test>::put(list);

    crate::CrossingTransitionQueues::<Test>::insert(
      7,
      crate::CrossingTransitionQueueOf::<Test>::default(),
    );
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::CrossingTransitionQueues::<Test>::insert(7, queue.clone());

    crate::CrossingTransitionQueues::<Test>::mutate(7, |maybe_queue| {
      maybe_queue.as_mut().expect("transition queue exists")[1].previous = 999;
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::CrossingTransitionQueues::<Test>::insert(7, queue);

    crate::CrossingPendingFeeds::<Test>::insert(
      7,
      crate::CrossingPendingFeedState {
        previous: pending.previous,
        next: Some(7),
      },
    );
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::CrossingPendingFeeds::<Test>::insert(7, pending);

    crate::CrossingPendingFeedListState::<Test>::mutate(|stored| stored.cursor = Some(999));
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::CrossingPendingFeedListState::<Test>::put(list);

    crate::CrossingRangeCursors::<Test>::insert(
      7,
      crate::CrossingRangeCursor {
        revision: 999,
        traversal: crate::CrossingTraversal::Upward,
        search_bound: u128::MAX,
        current_threshold: None,
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::CrossingRangeCursors::<Test>::remove(7);
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn eligibility_projection_explains_crossing_phase_work_and_topology_failure() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 110,
      },
    ));
    let ActorEligibility::Active(activation) = eligibility(actor_id) else {
      panic!("Crossing actor must be active");
    };
    assert_eq!(activation.pending_signal, false);
    assert_eq!(activation.placement, ActorActivationPlacement::Unplaced);
    assert!(matches!(
      activation.trigger,
      ActorTriggerActivation::ObservationCrossing {
        feed: 7,
        direction: CrossingDirection::Rising,
        threshold: 100,
        rearm_threshold: 80,
        phase: CrossingPhase::Armed,
        installed_at_revision: 1,
        pending_revisions: 1,
        processing_revision: None,
      }
    ));

    crate::CrossingMemberships::<Test>::remove(actor_id);
    assert_eq!(
      Actors::actor_eligibility(actor_id),
      Err(ActorClassificationError::ActorInvariant)
    );
  });
}
