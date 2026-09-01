use super::*;
use crate::{
  ActorContractHeads, ActorContractTailChunks, ActorControlLocation, ActorControlLocators,
  ActorRunHeads, ActorRunPayloads, ActorUnsignaledControlCells,
};

#[test]
fn trigger_grammar_is_single_source_and_non_nested() {
  let manual = RuntimeTrigger::manual();
  let address = RuntimeTrigger::address_event(SourceFilter::OwnerOnly, AssetFilter::Any);
  let observation = RuntimeTrigger::observation_change(7);
  let crossing = RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80);
  let at_time = RuntimeTrigger::at_time(10);
  let cadence = RuntimeTrigger::cadenced(10);

  assert_eq!(observation.encode(), vec![2, 7, 0, 0, 0]);
  assert!(manual.manual_source_enabled());
  assert!(!cadence.manual_source_enabled());

  assert_eq!(crossing.encode()[0], 3);
  assert_eq!(at_time.encode()[0], 4);
  assert_eq!(cadence.encode()[0], 5);

  assert!(TriggerRuntimeState::Stateless.is_compatible_with(&manual));
  assert!(TriggerRuntimeState::Stateless.is_compatible_with(&address));
  assert!(TriggerRuntimeState::Stateless.is_compatible_with(&observation));
  assert!(
    TriggerRuntimeState::ObservationCrossing {
      phase: CrossingPhase::Armed,
      installed_at_revision: 1,
    }
    .is_compatible_with(&crossing)
  );
  assert!(
    TriggerRuntimeState::AtTime {
      anchor_tick: None,
      consumed: false,
    }
    .is_compatible_with(&at_time)
  );
  assert!(TriggerRuntimeState::Cadenced { anchor_tick: None }.is_compatible_with(&cadence));
  assert!(!TriggerRuntimeState::Stateless.is_compatible_with(&crossing));
  assert!(!TriggerRuntimeState::Cadenced { anchor_tick: None }.is_compatible_with(&manual));

  for trigger in [manual, address, observation, crossing, at_time, cadence] {
    assert!(trigger.has_canonical_filters());
    let encoded = trigger.encode();
    assert!(encoded.len() <= RuntimeTrigger::max_encoded_len());
    assert_eq!(RuntimeTrigger::decode(&mut &encoded[..]), Ok(trigger));
  }

  let non_canonical_filter = RuntimeTrigger::address_event(
    SourceFilter::Whitelist(BoundedVec::try_from(vec![2, 1]).expect("within whitelist bound")),
    AssetFilter::Any,
  );
  assert!(!non_canonical_filter.has_canonical_filters());
}

#[test]
fn active_trigger_replacement_matrix_preserves_one_canonical_family() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
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
      RuntimeTrigger::at_time(10),
      RuntimeTrigger::cadenced(10),
    ];
    let mut actor_id = 0;
    for old_trigger in &triggers {
      for new_trigger in &triggers {
        let create_block = actor_id * 2 + 1;
        frame_system::Pallet::<Test>::set_block_number(create_block);
        let current_id = create_system_with(
          10_000 + actor_id,
          Schedule {
            trigger: old_trigger.clone(),
            cooldown_blocks: 0,
          },
          None,
          contract_steps_with_step(make_step(Task::StopCycle)),
        );
        frame_system::Pallet::<Test>::set_block_number(create_block + 1);
        let replacement = update_contract_partial!(
          RuntimeOrigin::root(),
          current_id,
          Schedule {
            trigger: new_trigger.clone(),
            cooldown_blocks: 0,
          },
          None,
        );
        assert!(
          replacement.is_ok(),
          "trigger replacement failed: old={old_trigger:?}, new={new_trigger:?}, result={replacement:?}"
        );
        let contract = Actors::load_actor_contract(current_id).expect("active Actor Contract");
        let hot = Actors::actor_hot(current_id).expect("active Actor hot state");
        assert_eq!(&contract.trigger, new_trigger);
        assert!(
          hot
            .trigger_runtime_state
            .is_compatible_with(&contract.trigger)
        );
        assert_eq!(
          Actors::crossing_membership(current_id).is_some(),
          matches!(new_trigger, Trigger::ObservationCrossing { .. })
        );
        assert_eq!(
          Actors::actor_observation_feeds(current_id).is_some(),
          matches!(new_trigger, Trigger::ObservationChange { .. })
        );
        actor_id += 1;
      }
    }
  });
}

#[test]
fn late_trigger_transition_failure_rolls_back_canonical_and_derived_state() {
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
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    set_fail_create_checkpoint(true);
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::root(),
        actor_id,
        Schedule {
          trigger: RuntimeTrigger::observation_change(7),
          cooldown_blocks: 0,
        },
        None,
      ),
      DispatchError::Other("AtomicityCreateCheckpointFailed")
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
    let contract = Actors::load_actor_contract(actor_id).expect("active Actor Contract");
    let hot = Actors::actor_hot(actor_id).expect("active Actor hot state");
    assert!(matches!(
      contract.trigger,
      Trigger::ObservationCrossing { .. }
    ));
    assert!(matches!(
      hot.trigger_runtime_state,
      TriggerRuntimeState::ObservationCrossing { .. }
    ));
    assert!(Actors::crossing_membership(actor_id).is_some());
    assert!(Actors::actor_observation_feeds(actor_id).is_none());
  });
}

#[test]
fn trigger_transition_preflight_is_read_only() {
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
        trigger: RuntimeTrigger::Manual,
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let trigger = RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80);
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let _plan = Actors::preflight_trigger_transition(
      actor_id,
      &trigger,
      crate::TriggerTransitionIntent::ReplaceActive,
    )
    .expect("valid Trigger transition preflights");
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
    assert!(Actors::crossing_membership(actor_id).is_none());
  });
}

#[test]
fn percentage_at_opening_is_independent_from_trigger_kind_and_payload() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let plan = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageAtOpening(Perbill::from_percent(50)),
    }));
    for schedule in [manual_schedule(), observation_schedule(vec![1])] {
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(schedule, None, plan.clone()),
      ));
    }
    let actor_id = Actors::next_actor_id().saturating_sub(1);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      percentage_trigger_schedule(),
      None,
    ));
  });
}

#[test]
fn already_live_map_error_fails_closed_without_panicking() {
  new_test_ext().execute_with(|| {
    assert_eq!(
      Actors::placement_error(crate::EnqueueOutcome::AlreadyLive),
      Error::<Test>::QueueCapacityUnavailable.into()
    );
  });
}

#[test]
fn system_creation_accepts_absent_contract() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None,
    ));
    let identity = Actors::actor_identity(0).expect("dormant identity exists");
    assert_eq!(identity.actor_class, ActorClass::System { sovereign_id: 0 });
    assert!(Actors::active_actor_view(0).is_none());
    assert_eq!(Actors::actor_identity_count(), 1);
    assert_eq!(Actors::active_actor_count(), 0);
  });
}

#[test]
fn exact_slot_user_creation_accepts_absent_contract() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let owner_slot = 2;
    assert_ok!(Actors::create_user_actor_at_slot(
      RuntimeOrigin::signed(ALICE),
      owner_slot,
      Mutability::Mutable,
      None,
    ));
    let identity = Actors::actor_identity(0).expect("dormant identity exists");
    assert_eq!(identity.actor_class, ActorClass::User { owner_slot });
    assert!(Actors::active_actor_view(0).is_none());
    assert_eq!(Actors::actor_identity_count(), 1);
    assert_eq!(Actors::active_actor_count(), 0);
  });
}

#[test]
fn deactivation_removes_active_epoch_and_all_contract_fragments_without_orphans() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let inert_step = inert_contract_steps()[0].clone();
    let contract_steps = BoundedVec::try_from(vec![inert_step; 8]).expect("eight Steps fit");
    let user_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps.clone(),
    );
    let system_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);

    for actor_id in [user_id, system_id] {
      assert!(Actors::actor_hot(actor_id).is_some());
      assert_eq!(
        ActorControlLocators::<Test>::get(actor_id),
        Some(ActorControlLocation::Unsignaled)
      );
      assert!(ActorUnsignaledControlCells::<Test>::contains_key(actor_id));
      assert!(ActorContractHeads::<Test>::contains_key(actor_id));
      assert!(Actors::actor_control_cell(actor_id).is_some());
      assert!(ActorContractTailChunks::<Test>::contains_key(actor_id, 0));
      assert!(ActorContractTailChunks::<Test>::contains_key(actor_id, 1));
    }

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      user_id,
    ));
    assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), system_id));

    for actor_id in [user_id, system_id] {
      assert!(Actors::actor_identity(actor_id).is_some());
      assert!(Actors::active_actor_view(actor_id).is_none());
      assert!(!Actors::actor_hot(actor_id).is_some());
      assert!(!ActorControlLocators::<Test>::contains_key(actor_id));
      assert!(!ActorUnsignaledControlCells::<Test>::contains_key(actor_id));
      assert!(!ActorContractHeads::<Test>::contains_key(actor_id));
      assert!(!Actors::actor_control_cell(actor_id).is_some());
      assert!(!ActorContractTailChunks::<Test>::contains_key(actor_id, 0));
      assert!(!ActorContractTailChunks::<Test>::contains_key(actor_id, 1));
      assert!(!ActorRunHeads::<Test>::contains_key(actor_id));
      assert!(!ActorRunPayloads::<Test>::contains_key(actor_id));
      assert!(!ActorFunding::<Test>::contains_key(actor_id));
    }
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn deactivate_activate_preserves_nonce_but_resets_active_epoch_state_for_both_classes() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let user_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let system_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    fund_native(user_id, 1_000_000_000_000);
    fund_native(system_id, 100);
    for actor_id in [user_id, system_id] {
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
    }
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(user_id)
        .expect("User active")
        .cycle_nonce,
      1
    );
    assert_eq!(
      Actors::active_actor_view(system_id)
        .expect("System active")
        .cycle_nonce,
      1
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      user_id,
    ));
    assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), system_id));
    for actor_id in [user_id, system_id] {
      let dormant = Actors::actor_identity(actor_id).expect("durable identity");
      assert_eq!(dormant.cycle_nonce, 1);
      assert_eq!(ActorIdentities::<Test>::get(actor_id), Some(dormant));
      assert!(!ActorControlLocators::<Test>::contains_key(actor_id));
      assert!(Actors::actor_funding(actor_id).is_none());
      assert!(Actors::actor_run_state(actor_id).is_none());
    }

    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      user_id,
      user_active_contract(manual_schedule(), None, inert_contract_steps())
        .expect("direct Actor Contract"),
    ));
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::root(),
      system_id,
      system_active_contract(manual_schedule(), None, inert_contract_steps())
        .expect("direct Actor Contract"),
    ));
    for actor_id in [user_id, system_id] {
      let active = Actors::active_actor_view(actor_id).expect("reactivated actor");
      assert_eq!(active.cycle_nonce, 1);
      assert!(!ActorIdentities::<Test>::contains_key(actor_id));
      assert_eq!(
        ActorControlLocators::<Test>::get(actor_id),
        Some(ActorControlLocation::Unsignaled)
      );
      assert!(ActorUnsignaledControlCells::<Test>::contains_key(actor_id));
      assert_eq!(active.unsuccessful_attempt_streak, 0);
      assert!(!active.pending_signal);
      assert!(actor_funding(actor_id).funding_accumulated.is_empty());
    }

    frame_system::Pallet::<Test>::set_block_number(4);
    for actor_id in [user_id, system_id] {
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
    }
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(user_id)
        .expect("User active")
        .cycle_nonce,
      2
    );
    assert_eq!(
      Actors::active_actor_view(system_id)
        .expect("System active")
        .cycle_nonce,
      2
    );
    frame_system::Pallet::<Test>::set_block_number(5);
    for actor_id in [user_id, system_id] {
      let latest = Actors::actor_identity(actor_id).expect("latest active identity");
      assert_ok!(Actors::deactivate_actor(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      let dormant = ActorIdentities::<Test>::get(actor_id).expect("dormant registry restored");
      assert_eq!(dormant.cycle_nonce, latest.cycle_nonce);
      assert_eq!(dormant.sovereign_account, latest.sovereign_account);
      assert_eq!(dormant.owner, latest.owner);
      assert_eq!(dormant.actor_class, latest.actor_class);
      assert!(!ActorControlLocators::<Test>::contains_key(actor_id));
    }
  });
}

#[test]
fn guaranteed_actor_service_rejects_housekeeping_underflow_in_each_dimension() {
  new_test_ext().execute_with(|| {
    let fixed = <TestWeightInfo as crate::WeightInfo>::scheduler_on_idle_base()
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::materialization_coordinator_base())
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1))
      .saturating_add(TestWakeupWeightLimit::get())
      .saturating_add(TestCrossingWorkerWeightLimit::get())
      .saturating_add(TestObservationFanoutWeightLimit::get());

    set_guaranteed_on_idle_weight(Weight::from_parts(
      fixed.ref_time().saturating_sub(1),
      u64::MAX,
    ));
    assert!(Actors::guaranteed_actor_service_weight().is_none());
    set_guaranteed_on_idle_weight(Weight::from_parts(
      u64::MAX,
      fixed.proof_size().saturating_sub(1),
    ));
    assert!(Actors::guaranteed_actor_service_weight().is_none());
  });
}

#[test]
fn owner_slot_capacity_is_enforced() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_slots = <<Test as crate::Config>::MaxOwnerSlots as Get<u8>>::get() as u64;
    for _ in 0..max_slots {
      let _ = create_user_with(
        ALICE,
        Mutability::Mutable,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
    }
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::OwnerSlotCapacityExceeded
    );
  });
}

#[test]
fn active_actor_capacity_is_enforced() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::set_active_actor_limit(RuntimeOrigin::root(), 1));
    let _first = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::ActiveActorCapacityExceeded
    );
  });
}

#[test]
fn actor_contract_installs_and_validates_auto_close_target() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      Some(ActorContract {
        auto_close_at_cycle_nonce: Some(2),
        ..system_active_contract(manual_schedule(), None, inert_contract_steps())
          .expect("direct Actor Contract")
      }),
    ));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("active actor exists")
        .auto_close_at_cycle_nonce,
      Some(2)
    );

    let next_id = Actors::next_actor_id();
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        Some(ActorContract {
          auto_close_at_cycle_nonce: Some(0),
          ..system_active_contract(manual_schedule(), None, inert_contract_steps())
            .expect("direct Actor Contract")
        }),
      ),
      Error::<Test>::InvalidAutoCloseNonce
    );
    assert_eq!(Actors::next_actor_id(), next_id);
  });
}

#[test]
fn system_actor_count_is_not_limited_by_owner_slots() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let attempts = <<Test as crate::Config>::MaxOwnerSlots as Get<u8>>::get() as u64 + 2;
    let mut sovereign_accounts: Vec<AccountId> = Vec::new();
    for _ in 0..attempts {
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      let inst = Actors::active_actor_view(actor_id).expect("Actors exists");
      assert_eq!(
        inst.actor_class,
        ActorClass::System {
          sovereign_id: actor_id,
        }
      );
      sovereign_accounts.push(inst.sovereign_account);
    }
    assert_eq!(OwnerSlotBitmaps::<Test>::get(ALICE), [0; 32]);
    for i in 0..sovereign_accounts.len() {
      for j in i + 1..sovereign_accounts.len() {
        assert_ne!(sovereign_accounts[i], sovereign_accounts[j]);
      }
    }
  });
}

#[test]
fn actor_class_separates_user_slots_from_system_actors() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let user_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    assert_eq!(
      Actors::active_actor_view(user_id)
        .expect("user Actors exists")
        .actor_class,
      ActorClass::User { owner_slot: 0 }
    );
    assert_eq!(OwnerSlotBitmaps::<Test>::get(ALICE)[0], 0b0000_0001);
    let system_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let system = Actors::active_actor_view(system_id).expect("system Actors exists");
    assert_eq!(
      system.actor_class,
      ActorClass::System {
        sovereign_id: system_id,
      }
    );
    assert_eq!(OwnerSlotBitmaps::<Test>::get(ALICE)[0], 0b0000_0001);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorCreated {
          actor_id,
          owner,
          actor_class,
          initial_lifecycle: InitialLifecycle::Active,
          ..
        } if *actor_id == system_id
          && *owner == ALICE
          && matches!(actor_class, ActorClass::System { .. })
      )
    }));
  });
}

#[test]
fn owner_slot_reused_after_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let id0 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let id1 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let slot0 = Actors::active_actor_view(id0)
      .expect("id0 exists")
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    let slot1 = Actors::active_actor_view(id1)
      .expect("id1 exists")
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    assert_eq!(slot0, 0);
    assert_eq!(slot1, 1);
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), id0));
    let id2 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let slot2 = Actors::active_actor_view(id2)
      .expect("id2 exists")
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    assert_eq!(slot2, slot0);
    assert!(Actors::active_actor_view(id0).is_none());
  });
}

#[test]
fn create_user_at_slot_fails_when_requested_slot_is_occupied() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let target_slot = 2;
    let _first = create_user_with_slot(
      ALICE,
      target_slot,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_noop!(
      Actors::create_user_actor_at_slot(
        RuntimeOrigin::signed(ALICE),
        target_slot,
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::OwnerSlotOccupied
    );
  });
}

#[test]
fn create_user_at_slot_fails_when_requested_slot_is_out_of_range() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let invalid_slot = <<Test as crate::Config>::MaxOwnerSlots as Get<u8>>::get();
    assert_noop!(
      Actors::create_user_actor_at_slot(
        RuntimeOrigin::signed(ALICE),
        invalid_slot,
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::InvalidOwnerSlot
    );
  });
}

#[test]
fn owner_slot_bitmap_covers_byte_boundaries_highest_slot_and_reuse() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    for slot in [7, 8, 15, 16] {
      create_user_with_slot(
        ALICE,
        slot,
        Mutability::Mutable,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
    }
    let highest = create_user_with_slot(
      ALICE,
      254,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let bitmap = Actors::owner_slot_bitmap(ALICE);
    assert_eq!(bitmap[0], 0b1000_0000);
    assert_eq!(bitmap[1], 0b1000_0001);
    assert_eq!(bitmap[2], 0b0000_0001);
    assert_eq!(bitmap[31], 0b0100_0000);
    assert_eq!(
      Actors::active_actor_view(highest)
        .unwrap()
        .actor_class
        .owner_slot(),
      Some(254)
    );
    assert_noop!(
      Actors::create_user_actor_at_slot(
        RuntimeOrigin::signed(ALICE),
        255,
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::InvalidOwnerSlot
    );
    let sovereign = Actors::active_actor_view(highest)
      .unwrap()
      .sovereign_account;
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), highest));
    assert_eq!(Actors::owner_slot_bitmap(ALICE)[31], 0);
    let replacement = create_user_with_slot(
      ALICE,
      254,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_eq!(
      Actors::active_actor_view(replacement)
        .unwrap()
        .sovereign_account,
      sovereign
    );
  });
}

#[test]
fn close_actor_emits_owner_initiated_reason() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let active_before = Actors::active_actor_count();
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_eq!(Actors::active_actor_count(), active_before + 1);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(Actors::active_actor_count(), active_before);
    let events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(events.len(), 1, "pure close emits only its close boundary");
    assert!(matches!(
      events[0],
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::OwnerInitiated,
      } if id == actor_id
    ));
  });
}

#[test]
fn address_ingress_reuses_existing_transaction_and_fails_closed_at_depth_limit() {
  fn run_nested(depth: u32, actor_id: ActorId) -> polkadot_sdk::sp_runtime::DispatchResult {
    if depth == 0 {
      return Actors::notify_address_event(actor_id, TestAsset::Native, 1, &ALICE);
    }
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      match run_nested(depth - 1, actor_id) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let available_depth =
      u32::from(polkadot_sdk::frame_support::storage::transactional::TRANSACTIONAL_LIMIT);

    assert_ok!(run_nested(available_depth - 2, actor_id));
    assert!(Actors::pending_signal(actor_id));
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();
    let available_depth =
      u32::from(polkadot_sdk::frame_support::storage::transactional::TRANSACTIONAL_LIMIT);

    assert!(run_nested(available_depth, actor_id).is_err());
    assert!(!Actors::pending_signal(actor_id));
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn control_permanent_placement_exhaustion_closes_through_the_unified_sink() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    crate::ActorReadyHead::<Test>::put(u64::MAX);
    crate::ActorReadyTail::<Test>::put(u64::MAX);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::SchedulerIndexExhausted,
      } if *id == actor_id
    )));
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      timer_schedule(1),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let trigger_deadline = Actors::actor_hot(actor_id)
      .and_then(|hot| hot.trigger_wakeup_pointer)
      .expect("Cadenced trigger deadline exists");
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(
      Actors::actor_hot(actor_id).and_then(|hot| hot.trigger_wakeup_pointer),
      Some(trigger_deadline)
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    crate::ActorReadyHead::<Test>::put(u64::MAX);
    crate::ActorReadyTail::<Test>::put(u64::MAX);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(
      Actors::actor_hot(actor_id).and_then(|hot| hot.trigger_wakeup_pointer),
      Some(trigger_deadline)
    );
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      transfer_contract_steps(BOB, 1),
    );
    let (_, mut consumed) = Actors::actor_control_cell(actor_id).expect("terminal primary");
    assert!(Actors::wakeup_substrate_invalidate(actor_id).is_some());
    assert!(!ActorControlLocators::<Test>::contains_key(actor_id));
    consumed.hot.wakeup_pointer = None;
    consumed.eligible_at = None;
    ActorUnsignaledControlCells::<Test>::insert(actor_id, consumed);
    ActorControlLocators::<Test>::insert(actor_id, ActorControlLocation::Unsignaled);
    crate::WakeupCursorLen::<Test>::insert(
      WakeupClock::Block,
      <<Test as crate::Config>::MaxActiveActors as Get<u32>>::get(),
    );
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();

    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::SchedulerIndexExhausted
    );
    assert!(
      !Actors::actor_hot(actor_id)
        .expect("actor remains active")
        .lifecycle
        .is_paused()
    );
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    crate::ActorReadyHead::<Test>::put(u64::MAX);
    crate::ActorReadyTail::<Test>::put(u64::MAX);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      timer_schedule(1),
      None
    ));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
  });
}

#[test]
fn on_address_event_owner_filter_is_enforced() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule,
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &BOB
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(10));
  });
}

#[test]
fn on_address_event_without_source_is_ignored_for_owner_filter() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule,
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::notify_address_event_without_source(
      actor_id,
      TestAsset::Native,
      100
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn stable_hot_mutation_and_store_seams_update_primary_and_fail_closed_on_topology_fault() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_eq!(
      Actors::mutate_control_hot_or(actor_id, 99, |hot| {
        hot.unsuccessful_attempt_streak = 7;
        7
      }),
      7
    );
    let (_, _, frame_hot, _) =
      Actors::load_frame_control_authority(actor_id).expect("frame authority exists");
    let projected_hot = Actors::actor_hot(actor_id).expect("canonical projection exists");
    assert_eq!(frame_hot, projected_hot);
    assert_eq!(frame_hot.unsuccessful_attempt_streak, 7);

    let mut replacement = projected_hot;
    replacement.unsuccessful_attempt_streak = 6;
    assert_eq!(
      Actors::try_store_control_hot_with_authority(actor_id, replacement.clone(),),
      Ok(())
    );
    let (_, _, frame_hot, _) =
      Actors::load_frame_control_authority(actor_id).expect("stored frame authority exists");
    assert_eq!(frame_hot, replacement);
    assert_eq!(Actors::actor_hot(actor_id), Some(replacement.clone()));

    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert!(Actors::actor_hot(actor_id).is_none());
    assert_eq!(
      Actors::mutate_control_hot_or(actor_id, 99, |hot| {
        hot.unsuccessful_attempt_streak = 8;
        8
      }),
      99
    );
    assert!(Actors::actor_hot(actor_id).is_none());
    let mut rejected = replacement;
    rejected.unsuccessful_attempt_streak = 8;
    assert_eq!(
      Actors::try_store_control_hot_with_authority(actor_id, rejected,),
      Err(crate::scheduler::EnqueueOutcome::CorruptedTopology)
    );
    assert!(Actors::actor_hot(actor_id).is_none());
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn lifecycle_mutations_keep_frame_identity_and_hot_authority_in_lockstep() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    let (_, frame_identity, frame_hot, frame_admission) =
      Actors::load_frame_control_authority(actor_id).expect("paused frame authority exists");
    assert_eq!(
      frame_identity,
      Actors::actor_identity(actor_id).expect("canonical identity")
    );
    assert_eq!(
      frame_hot,
      Actors::actor_hot(actor_id).expect("canonical hot state")
    );
    assert_eq!(
      frame_admission,
      Actors::actor_control_cell(actor_id)
        .map(|(_, cell)| cell.admission)
        .expect("canonical admission")
    );
    assert!(frame_hot.lifecycle.is_paused());

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    let (_, frame_identity, frame_hot, _) =
      Actors::load_frame_control_authority(actor_id).expect("resumed frame authority exists");
    assert_eq!(
      frame_identity,
      Actors::actor_identity(actor_id).expect("canonical identity")
    );
    assert_eq!(
      frame_hot,
      Actors::actor_hot(actor_id).expect("canonical hot state")
    );
    assert_eq!(frame_hot.lifecycle, ActiveLifecycle::Active);
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn policy_only_contract_replacement_uses_frame_authority_with_canonical_control() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let crate::LoadedActorStateOf::Active(state) = Actors::load_frame_actor_state(actor_id) else {
      panic!("active frame state");
    };
    let mut replacement = state.contract;
    replacement.completion = crate::CompletionPolicy::CloseAfterProductiveCycle;

    assert_ok!(Actors::update_contract(
      RuntimeOrigin::root(),
      actor_id,
      replacement.clone(),
    ));
    let (_, _, hot, admission) =
      Actors::load_frame_control_authority(actor_id).expect("updated frame authority exists");
    assert_eq!(hot.lifecycle, ActiveLifecycle::Active);
    let crate::LoadedActorStateOf::Active(updated) = Actors::load_frame_actor_state(actor_id)
    else {
      panic!("updated frame state");
    };
    assert_eq!(updated.contract, replacement);
    assert!(admission.has_valid_identity());
    assert!(ActorIdentities::<Test>::get(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn user_contract_replacement_reconciles_state_hold_from_frame_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let hold_before = crate::ActorStateHolds::<Test>::get(actor_id).expect("User hold exists");
    let crate::LoadedActorStateOf::Active(state) = Actors::load_frame_actor_state(actor_id) else {
      panic!("active User frame state");
    };
    let mut replacement = state.contract;
    replacement.completion = crate::CompletionPolicy::CloseAfterProductiveCycle;
    frame_system::Pallet::<Test>::set_block_number(2);

    assert_ok!(Actors::update_contract(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      replacement,
    ));
    let hold_after = crate::ActorStateHolds::<Test>::get(actor_id).expect("User hold survives");
    assert_eq!(hold_after.owner, hold_before.owner);
    assert_eq!(hold_after.breakdown, hold_before.breakdown);
    assert!(ActorIdentities::<Test>::get(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn user_crossing_install_uses_frozen_class_with_canonical_control() {
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
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let crate::LoadedActorStateOf::Active(state) = Actors::load_frame_actor_state(actor_id) else {
      panic!("active User frame state");
    };
    let mut replacement = state.contract;
    replacement.trigger =
      RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80);
    frame_system::Pallet::<Test>::set_block_number(2);

    assert_ok!(Actors::update_contract(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      replacement,
    ));
    let locator =
      crate::CrossingMemberships::<Test>::get(actor_id).expect("User membership exists");
    assert_eq!(crate::CrossingUserFeedMembershipCount::<Test>::get(7), 1);
    let (_, _, hot, admission) =
      Actors::load_frame_control_authority(actor_id).expect("User Crossing frame authority exists");
    let page = crate::CrossingMemberPages::<Test>::get(locator.key, locator.page)
      .expect("User Crossing member page exists");
    let member = page
      .entries
      .get(locator.offset as usize)
      .expect("User Crossing member exists");
    assert_eq!(member.admission_identity, admission.admission_identity);
    assert_ne!(member.admission_identity, [0; 32]);
    assert!(matches!(
      hot.trigger_runtime_state,
      TriggerRuntimeState::ObservationCrossing {
        phase: CrossingPhase::Armed,
        ..
      }
    ));
    assert!(ActorIdentities::<Test>::get(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn crossing_schedule_replacement_preserves_frame_phase_with_canonical_control() {
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
      inert_contract_steps(),
    );
    let membership_before =
      crate::CrossingMemberships::<Test>::get(actor_id).expect("Crossing membership exists");
    let crate::LoadedActorStateOf::Active(state) = Actors::load_frame_actor_state(actor_id) else {
      panic!("active Crossing frame state");
    };
    let mut replacement = state.contract;
    replacement.cooldown_blocks = 1;
    frame_system::Pallet::<Test>::set_block_number(2);

    assert_ok!(Actors::update_contract(
      RuntimeOrigin::root(),
      actor_id,
      replacement,
    ));
    assert_eq!(
      crate::CrossingMemberships::<Test>::get(actor_id),
      Some(membership_before)
    );
    let (_, _, hot, _) =
      Actors::load_frame_control_authority(actor_id).expect("Crossing frame authority survives");
    assert!(matches!(
      hot.trigger_runtime_state,
      TriggerRuntimeState::ObservationCrossing {
        phase: CrossingPhase::Armed,
        ..
      }
    ));
    assert!(ActorIdentities::<Test>::get(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn temporal_schedule_replacement_invalidates_loaded_frame_reference_with_canonical_control() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(5), None, inert_contract_steps());
    let crate::LoadedActorStateOf::Active(state) = Actors::load_frame_actor_state(actor_id) else {
      panic!("active temporal frame state");
    };
    assert!(state.hot.trigger_wakeup_pointer.is_some());
    let mut replacement = state.contract;
    replacement.trigger = Trigger::manual();
    frame_system::Pallet::<Test>::set_block_number(2);

    assert_ok!(Actors::update_contract(
      RuntimeOrigin::root(),
      actor_id,
      replacement,
    ));
    let (location, _, hot, _) =
      Actors::load_frame_control_authority(actor_id).expect("updated frame authority exists");
    assert_eq!(location, crate::ActorControlLocation::Unsignaled);
    assert!(hot.trigger_wakeup_pointer.is_none());
    assert!(ActorIdentities::<Test>::get(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn pause_and_resume_use_frame_authority_with_canonical_control() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());

    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    let (_, _, paused_hot, _) =
      Actors::load_frame_control_authority(actor_id).expect("paused frame authority exists");
    assert!(paused_hot.lifecycle.is_paused());
    assert!(ActorIdentities::<Test>::get(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    let (_, _, resumed_hot, _) =
      Actors::load_frame_control_authority(actor_id).expect("resumed frame authority exists");
    assert_eq!(resumed_hot.lifecycle, ActiveLifecycle::Active);
    assert!(ActorIdentities::<Test>::get(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn pause_and_resume_fail_closed_without_primary_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    Actors::remove_primary_control_cell_inner(actor_id).expect("primary removal succeeds");
    let hot_before = Actors::actor_hot(actor_id);
    let events_before = System::events();
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ActorInvariant
    );
    assert_eq!(Actors::actor_hot(actor_id), hot_before);
    assert_eq!(System::events(), events_before);
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::remove_primary_control_cell_inner(actor_id).expect("paused primary removal succeeds");
    let hot_before = Actors::actor_hot(actor_id);
    let events_before = System::events();
    assert_noop!(
      Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ActorInvariant
    );
    assert_eq!(Actors::actor_hot(actor_id), hot_before);
    assert_eq!(System::events(), events_before);
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn permissionless_sweep_fails_closed_without_primary_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    Actors::remove_primary_control_cell_inner(actor_id).expect("primary removal succeeds");
    let identity_before = Actors::actor_identity(actor_id);
    let hot_before = Actors::actor_hot(actor_id);
    let events_before = System::events();
    assert_noop!(
      Actors::permissionless_sweep(RuntimeOrigin::signed(CHARLIE), actor_id),
      Error::<Test>::ActorInvariant
    );
    let sweep_ids: BoundedVec<u64, <Test as crate::Config>::MaxSweepBatch> =
      BoundedVec::try_from(vec![actor_id]).expect("batch fits");
    assert_noop!(
      Actors::permissionless_sweep_many(RuntimeOrigin::signed(CHARLIE), sweep_ids),
      Error::<Test>::ActorInvariant
    );
    assert_eq!(Actors::actor_identity(actor_id), identity_before);
    assert_eq!(Actors::actor_hot(actor_id), hot_before);
    assert_eq!(System::events(), events_before);
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn permissionless_sweep_many_rolls_back_an_earlier_close_on_missing_primary() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let terminal = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    set_actor_cycle_nonce_coherent(terminal, u64::MAX);
    let corrupt = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    Actors::remove_primary_control_cell_inner(corrupt).expect("primary removal succeeds");
    let corrupt_contract_before = ActorContractHeads::<Test>::get(corrupt);
    let corrupt_funding_before = ActorFunding::<Test>::get(corrupt);
    let events_before = System::events();
    let sweep_ids: BoundedVec<u64, <Test as crate::Config>::MaxSweepBatch> =
      BoundedVec::try_from(vec![terminal, corrupt]).expect("batch fits");

    assert_noop!(
      Actors::permissionless_sweep_many(RuntimeOrigin::signed(CHARLIE), sweep_ids),
      Error::<Test>::ActorInvariant
    );
    assert!(Actors::active_actor_state(terminal).is_some());
    assert_eq!(
      Actors::actor_identity(terminal)
        .expect("terminal identity rolls back")
        .cycle_nonce,
      u64::MAX
    );
    assert!(Actors::actor_identity(corrupt).is_none());
    assert!(Actors::actor_hot(corrupt).is_none());
    assert!(!ActorControlLocators::<Test>::contains_key(corrupt));
    assert!(!crate::ActorUnsignaledControlCells::<Test>::contains_key(
      corrupt
    ));
    assert_eq!(
      ActorContractHeads::<Test>::get(corrupt),
      corrupt_contract_before
    );
    assert_eq!(ActorFunding::<Test>::get(corrupt), corrupt_funding_before);
    assert_eq!(System::events(), events_before);
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn close_and_deactivate_fail_closed_without_primary_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    Actors::remove_primary_control_cell_inner(actor_id).expect("primary removal succeeds");
    let identity_before = Actors::actor_identity(actor_id);
    let hot_before = Actors::actor_hot(actor_id);
    let events_before = System::events();
    assert_noop!(
      Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ActorInvariant
    );
    assert_eq!(Actors::actor_identity(actor_id), identity_before);
    assert_eq!(Actors::actor_hot(actor_id), hot_before);
    assert_eq!(System::events(), events_before);
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    Actors::remove_primary_control_cell_inner(actor_id).expect("primary removal succeeds");
    let identity_before = Actors::actor_identity(actor_id);
    let hot_before = Actors::actor_hot(actor_id);
    let events_before = System::events();
    assert_noop!(
      Actors::deactivate_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ActorInvariant
    );
    assert_eq!(Actors::actor_identity(actor_id), identity_before);
    assert_eq!(Actors::actor_hot(actor_id), hot_before);
    assert_eq!(System::events(), events_before);
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn active_reactivation_fails_closed_without_primary_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let contract = Actors::load_actor_contract(actor_id).expect("active Contract exists");
    Actors::remove_primary_control_cell_inner(actor_id).expect("primary removal succeeds");
    let identity_before = Actors::actor_identity(actor_id);
    let hot_before = Actors::actor_hot(actor_id);
    let events_before = System::events();

    assert_noop!(
      Actors::activate_actor(RuntimeOrigin::signed(ALICE), actor_id, contract),
      Error::<Test>::ActorInvariant
    );
    assert_eq!(Actors::actor_identity(actor_id), identity_before);
    assert_eq!(Actors::actor_hot(actor_id), hot_before);
    assert_eq!(System::events(), events_before);
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn contract_update_fails_closed_without_primary_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let mut replacement = Actors::load_actor_contract(actor_id).expect("active Contract exists");
    replacement.steps = transfer_contract_steps(BOB, 1);
    replacement.completion = crate::CompletionPolicy::Persistent;
    Actors::remove_primary_control_cell_inner(actor_id).expect("primary removal succeeds");
    let contract_before = Actors::load_actor_contract(actor_id);
    let admission_before = Actors::actor_control_cell(actor_id).map(|(_, cell)| cell.admission);
    let hot_before = Actors::actor_hot(actor_id);
    let events_before = System::events();

    assert_noop!(
      Actors::update_contract(RuntimeOrigin::signed(ALICE), actor_id, replacement,),
      Error::<Test>::ActorInvariant
    );
    assert_eq!(Actors::load_actor_contract(actor_id), contract_before);
    assert_eq!(
      Actors::actor_control_cell(actor_id).map(|(_, cell)| cell.admission),
      admission_before
    );
    assert_eq!(Actors::actor_hot(actor_id), hot_before);
    assert_eq!(System::events(), events_before);
  });
}

#[test]
fn manual_trigger_persists_across_pause_resume() {
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
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .pending_signal
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .pending_signal
    );
    run_idle(Weight::MAX);
    let inst = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(!inst.pending_signal);
    assert_eq!(inst.cycle_nonce, 1);
  });
}

#[test]
fn system_pause_resume_churn_is_limited_for_governance_and_owner_origins() {
  new_test_ext().execute_with(|| {
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_ok!(Actors::pause_actor(RuntimeOrigin::root(), actor_id));
    assert_noop!(
      Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ControlMutationRateLimited
    );
    assert_eq!(
      Actors::actor_identity(actor_id)
        .expect("paused System actor identity")
        .last_control_mutation_block,
      1
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ControlMutationRateLimited
    );
    assert_eq!(
      Actors::actor_identity(actor_id)
        .expect("active System actor identity")
        .last_control_mutation_block,
      2
    );
  });
}

#[test]
fn paused_head_uses_complete_loaded_state_admission() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    let scan = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1);
    let consume = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
      .max(<<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page());
    let state_probe = Actors::scheduler_actor_state_probe_weight_upper();
    Actors::execute_cycle(scan.saturating_add(state_probe).saturating_add(consume));

    let paused = Actors::actor_hot(actor_id).expect("paused actor");
    assert!(paused.pending_signal);
    assert_eq!(Actors::actor_identity(actor_id).expect("identity").cycle_nonce, 0);
    assert!(paused.queue_ticket.is_none());
  });
}

#[test]
fn manual_trigger_waits_through_cooldown_without_second_signal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 5,
    };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule,
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 2_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      1
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .pending_signal
    );
    assert_eq!(scheduled_wakeup_block(actor_id), Some(6));
    frame_system::Pallet::<Test>::set_block_number(6);
    run_idle(Weight::MAX);
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(instance.cycle_nonce, 2);
    assert!(!instance.pending_signal);
  });
}

#[test]
fn manual_trigger_waits_for_schedule_window_without_second_signal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(ScheduleWindow {
        start: 10,
        end: 110,
      }),
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .pending_signal
    );
    assert_eq!(scheduled_wakeup_block(actor_id), Some(10));
    frame_system::Pallet::<Test>::set_block_number(10);
    run_idle(Weight::MAX);
    #[cfg(not(feature = "runtime-benchmarks"))]
    {
      let locator = crate::ActorControlLocators::<Test>::get(actor_id);
      assert!(
        matches!(locator, Some(crate::ActorControlLocation::Waiting { .. })),
        "post-cycle locator: {locator:?}"
      );
    }
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      1
    );
  });
}

#[test]
fn manual_trigger_is_preserved_on_weight_defer() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Actors::scheduler_admission_overhead().saturating_add(Weight::from_parts(10, 0)));
    let inst = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(inst.pending_signal);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. } | Event::CycleSummary { actor_id: id, .. }
        if *id == actor_id
    )));
  });
}

#[test]
fn manual_trigger_is_preserved_on_proof_size_defer() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let task = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(task)),
    );
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    let (_, cell) = Actors::actor_control_cell(actor_id).expect("current control owner exists");
    let step = cell.resources.control.saturating_add(cell.resources.effect);
    let queue_weight = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1)
      .saturating_add(Actors::scheduler_actor_probe_weight_upper())
      .saturating_add(
        <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
          .max(<<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page()),
      );
    let proof_limit = queue_weight
      .proof_size()
      .saturating_add(step.proof_size())
      .saturating_sub(1);
    Actors::execute_cycle(Weight::from_parts(u64::MAX, proof_limit));
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(instance.pending_signal);
    assert_eq!(instance.cycle_nonce, 0);
  });
}

#[test]
fn typed_ingress_preflight_and_notify_close_permanent_exhaustion() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, 10),
    );
    let sovereign = sovereign_account(actor_id);
    let event = crate::AddressEvent {
      destination: sovereign,
      source: Some(ALICE),
      asset: TestAsset::Native,
      amount: 100,
      provenance: Some(crate::FundingProvenance::Signed),
    };
    // Preflight is read-only: it covers lifecycle and funding but performs no
    // placement, so monotonic namespace exhaustion cannot fail it.
    assert_ok!(Actors::preflight_ingress(&event));
    // Monotonic ticket namespace at the ceiling closes through the canonical
    // SchedulerIndexExhausted owner (spec 5.3, 6.2).
    crate::ActorReadyHead::<Test>::put(u64::MAX);
    crate::ActorReadyTail::<Test>::put(u64::MAX);
    let actor_before = native_balance(&sovereign);
    assert_ok!(Actors::notify_ingress(&event));
    assert_eq!(
      native_balance(&sovereign),
      actor_before,
      "the ingress adapter owns the already-certified balance movement"
    );
    assert!(Actors::active_actor_view(actor_id).is_none());
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
fn dormant_activation_anchors_first_eligibility_at_activation_time() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None,
    ));
    let actor_id = 0;
    frame_system::Pallet::<Test>::set_block_number(10);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      ActorContract {
        funding: FundingSourcePolicy::AnyVerifiedIngress,
        ..system_active_contract(timer_schedule(20), None, inert_contract_steps())
          .expect("direct Actor Contract")
      },
    ));
    let instance = Actors::active_actor_view(actor_id).expect("active Actors exists");
    // Activation anchors the fresh epoch at block 10; the first gate is one exact cadence later.
    assert_eq!(instance.schedule_anchor, 10);
    assert_eq!(instance.last_cycle_block, None);
    assert_eq!(scheduled_wakeup_block(actor_id), Some(30));
  });
}

#[test]
fn absent_schedule_window_never_expires() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    frame_system::Pallet::<Test>::set_block_number(u64::MAX);
    let instance = Actors::active_actor_view(actor_id).expect("actor remains live");
    assert!(!Actors::is_window_expired(&instance));
  });
}

#[test]
fn exact_update_noops_preserve_all_actor_state_and_emit_nothing() {
  new_test_ext().execute_with(|| {
    let encoded_actor_state = |actor_id| {
      (
        Actors::actor_hot(actor_id).encode(),
        Actors::load_actor_contract(actor_id).encode(),
        crate::ActorFunding::<Test>::get(actor_id).encode(),
        ActorRunStateStore::<Test>::get(actor_id).encode(),
      )
    };
    let plan_id = create_suspended_system_retry(1);
    let plan_before = encoded_actor_state(plan_id);
    let stored_contract = Actors::load_actor_contract(plan_id).expect("active Actor Contract");
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      plan_id,
      stored_contract.steps,
      stored_contract.completion,
    ));
    assert_eq!(encoded_actor_state(plan_id), plan_before);
    assert!(frame_system::Pallet::<Test>::events().is_empty());

    let policy_id = create_suspended_system_retry(2);
    let policy_before = encoded_actor_state(policy_id);
    let policy = Actors::load_actor_contract(policy_id)
      .expect("active Actor Contract")
      .funding;
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      policy_id,
      policy,
    ));
    assert_eq!(encoded_actor_state(policy_id), policy_before);
    assert!(frame_system::Pallet::<Test>::events().is_empty());

    let schedule_id = create_suspended_system_retry(3);
    let schedule_before = encoded_actor_state(schedule_id);
    let stored_schedule = Actors::load_actor_contract(schedule_id).expect("active Actor Contract");
    let schedule = RuntimeSchedule {
      trigger: stored_schedule.trigger,
      cooldown_blocks: stored_schedule.cooldown_blocks,
    };
    let schedule_window = stored_schedule.window;
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      schedule_id,
      schedule,
      schedule_window,
    ));
    assert_eq!(encoded_actor_state(schedule_id), schedule_before);
    assert!(frame_system::Pallet::<Test>::events().is_empty());

    let auto_close_id = create_suspended_system_retry(4);
    let continuation_before =
      ActorRunStateStore::<Test>::get(auto_close_id).expect("suspended run exists");
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(replace_auto_close(
      RuntimeOrigin::root(),
      auto_close_id,
      Some(2),
    ));
    assert!(ActorRunStateStore::<Test>::get(auto_close_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleCancelled {
        actor_id,
        cycle_nonce,
        reason: CancellationReason::ContractReplaced,
      } if *actor_id == auto_close_id && *cycle_nonce == continuation_before.cycle_nonce
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ContractUpdated { actor_id } if *actor_id == auto_close_id
    )));
    let auto_close_before = encoded_actor_state(auto_close_id);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(replace_auto_close(
      RuntimeOrigin::root(),
      auto_close_id,
      Some(2),
    ));
    assert_eq!(encoded_actor_state(auto_close_id), auto_close_before);
    assert!(frame_system::Pallet::<Test>::events().is_empty());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn authored_cancellation_restores_frame_authority_with_canonical_control() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(9);
    frame_system::Pallet::<Test>::set_block_number(10);
    run_idle(Weight::MAX);
    assert!(matches!(
      ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Waiting { .. })
    ));

    assert_ok!(Actors::cancel_run(RuntimeOrigin::signed(ALICE), actor_id,));
    assert!(ActorRunStateStore::<Test>::get(actor_id).is_none());
    assert!(ActorIdentities::<Test>::get(actor_id).is_none());
    assert!(Actors::actor_control_cell(actor_id).is_some());
    assert!(Actors::actor_hot(actor_id).is_some());
    assert_eq!(
      ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Unsignaled)
    );
    let (_, _, hot, _) =
      Actors::load_frame_control_authority(actor_id).expect("frame authority is restored");
    assert_eq!(hot.cycle_state, CycleState::Idle);
    assert!(hot.wakeup_pointer.is_none());
    assert!(hot.queue_ticket.is_none());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn deactivation_uses_frame_authority_with_canonical_control() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(9);
    frame_system::Pallet::<Test>::set_block_number(10);
    run_idle(Weight::MAX);

    assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), actor_id));
    assert!(ActorControlLocators::<Test>::get(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_none());
    assert!(
      Actors::actor_control_cell(actor_id)
        .map(|(_, cell)| cell.admission)
        .is_none()
    );
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Dormant(_)
    ));
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn authored_ready_close_removes_frame_authority_with_canonical_control() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(matches!(
      ActorControlLocators::<Test>::get(actor_id),
      Some(ActorControlLocation::Ready { .. })
    ));
    frame_system::Pallet::<Test>::set_block_number(2);

    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));

    assert!(ActorControlLocators::<Test>::get(actor_id).is_none());
    assert!(ActorIdentities::<Test>::get(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_none());
    assert!(
      Actors::actor_control_cell(actor_id)
        .map(|(_, cell)| cell.admission)
        .is_none()
    );
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::NotRegistered
    ));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn cadenced_waiting_deactivation_invalidates_frame_wakeup_with_canonical_control() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(1), None, inert_contract_steps());
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    assert!(matches!(
      ActorControlLocators::<Test>::get(actor_id),
      Some(ActorControlLocation::Waiting { .. })
    ));
    frame_system::Pallet::<Test>::set_block_number(2);

    assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), actor_id));

    assert!(ActorControlLocators::<Test>::get(actor_id).is_none());
    assert_eq!(scheduled_wakeup_block(actor_id), None);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Dormant(_)
    ));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn ready_deactivation_consumes_frame_ticket_with_canonical_control() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(matches!(
      ActorControlLocators::<Test>::get(actor_id),
      Some(ActorControlLocation::Ready { .. })
    ));
    frame_system::Pallet::<Test>::set_block_number(2);

    assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), actor_id));

    assert!(ActorControlLocators::<Test>::get(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_none());
    assert!(
      Actors::actor_control_cell(actor_id)
        .map(|(_, cell)| cell.admission)
        .is_none()
    );
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Dormant(_)
    ));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn crossing_deactivation_removes_membership_with_canonical_control() {
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
      inert_contract_steps(),
    );
    assert!(crate::CrossingMemberships::<Test>::contains_key(actor_id));

    assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), actor_id));
    assert!(!crate::CrossingMemberships::<Test>::contains_key(actor_id));
    assert!(ActorControlLocators::<Test>::get(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_none());
    assert!(
      Actors::actor_control_cell(actor_id)
        .map(|(_, cell)| cell.admission)
        .is_none()
    );
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Dormant(_)
    ));
  });
}

#[test]
fn contract_policy_schedule_deactivation_and_close_cancel_with_typed_reasons() {
  new_test_ext().execute_with(|| {
    let plan_id = create_suspended_system_retry(1);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      plan_id,
      inert_contract_steps(),
      crate::CompletionPolicy::Persistent,
    ));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleCancelled { actor_id, reason: CancellationReason::ContractReplaced, .. }
        if *actor_id == plan_id
    )));

    let policy_id = create_suspended_system_retry(2);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      policy_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleCancelled { actor_id, reason: CancellationReason::ContractReplaced, .. }
        if *actor_id == policy_id
    )));

    let schedule_id = create_suspended_system_retry(3);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      schedule_id,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 1,
      },
      None
    ));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleCancelled { actor_id, reason: CancellationReason::ContractReplaced, .. }
        if *actor_id == schedule_id
    )));

    let deactivate_id = create_suspended_system_retry(4);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), deactivate_id));
    let deactivate_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert!(matches!(deactivate_events[0], Event::CycleCancelled { actor_id, reason: CancellationReason::Deactivated, .. } if actor_id == deactivate_id));
    assert!(matches!(deactivate_events[1], Event::CycleSummary { actor_id, .. } if actor_id == deactivate_id));
    assert!(matches!(deactivate_events[2], Event::ActorDeactivated { actor_id } if actor_id == deactivate_id));

    let close_id = create_suspended_system_retry(5);
    let sovereign = sovereign_account(close_id);
    let balance_before = native_balance(&sovereign);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), close_id));
    assert_eq!(native_balance(&sovereign), balance_before);
    let close_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert!(matches!(close_events[0], Event::CycleCancelled { actor_id, reason: CancellationReason::Closing(CloseReason::OwnerInitiated), .. } if actor_id == close_id));
    assert!(matches!(close_events[1], Event::CycleSummary { actor_id, .. } if actor_id == close_id));
    assert!(matches!(close_events[2], Event::ActorClosed { actor_id, reason: CloseReason::OwnerInitiated } if actor_id == close_id));
  });
}

#[test]
fn window_expiry_cancels_while_failure_cutoff_finalizes_before_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let window_id = create_system_with(
      ALICE,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 200,
      },
      Some(ScheduleWindow { start: 1, end: 101 }),
      temporary_retry_swap_plan(),
    );
    fund_native(window_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), window_id));
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(102);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let expiry_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert!(matches!(expiry_events[0], Event::CycleCancelled { actor_id, reason: CancellationReason::Closing(CloseReason::WindowExpired), .. } if actor_id == window_id));
    assert!(matches!(expiry_events[1], Event::CycleSummary { actor_id, .. } if actor_id == window_id));
    assert!(matches!(expiry_events[2], Event::ActorClosed { actor_id, reason: CloseReason::WindowExpired } if actor_id == window_id));

    let cutoff_id = create_suspended_system_retry(103);
    frame_system::Pallet::<Test>::set_block_number(104);
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(106);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let cutoff_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert!(matches!(cutoff_events[0], Event::CycleContinued { actor_id, .. } if actor_id == cutoff_id));
    assert!(!cutoff_events.iter().any(|event| matches!(
      event,
      Event::CycleCancelled { actor_id, .. } if *actor_id == cutoff_id
    )));
    let summary_index = cutoff_events
      .iter()
      .position(|event| matches!(event, Event::CycleSummary { actor_id, result: CycleResult::Failed, .. } if *actor_id == cutoff_id))
      .expect("terminal summary");
    let close_index = cutoff_events
      .iter()
      .position(|event| matches!(event, Event::ActorClosed { actor_id, reason: CloseReason::ConsecutiveFailures } if *actor_id == cutoff_id))
      .expect("terminal close");
    assert!(summary_index < close_index);
  });
}

#[test]
fn user_immutable_manual_source_changes_readiness_without_mutating_contract() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Immutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    fund_native(actor_id, 1_000);
    let contract_before = Actors::load_actor_contract(actor_id).expect("immutable contract");
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(Actors::pending_signal(actor_id));
    assert_eq!(Actors::load_actor_contract(actor_id), Some(contract_before));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("immutable actor remains")
        .cycle_nonce,
      1
    );
  });
}

#[test]
fn user_immutable_rejects_every_owner_mutation_and_close_path() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Immutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ImmutableActor
    );
    assert_noop!(
      Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ImmutableActor
    );
    let replacement = transfer_contract_steps(CHARLIE, 1);
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        replacement,
        crate::CompletionPolicy::Persistent,
      ),
      Error::<Test>::ImmutableActor
    );
    assert_noop!(
      replace_auto_close(RuntimeOrigin::signed(ALICE), actor_id, Some(2)),
      Error::<Test>::ImmutableActor
    );
    assert_noop!(
      Actors::deactivate_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ImmutableActor
    );
    assert_noop!(
      Actors::cancel_run(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ImmutableActor
    );
    assert_noop!(
      Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ImmutableActor
    );
    let retained = Actors::active_actor_view(actor_id).expect("immutable actor remains active");
    assert_eq!(retained.mutability, Mutability::Immutable);
    assert_eq!(retained.cycle_nonce, 0);
  });
}

#[test]
fn update_contract_rejects_mint_for_user_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let replacement = contract_steps_with_step(make_step(Task::Mint {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    }));
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        replacement,
        crate::CompletionPolicy::Persistent,
      ),
      Error::<Test>::MintNotAllowedForUserActor
    );
  });
}

#[test]
fn permissionless_sweep_many_closes_multiple_and_reports_counts() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let user_a_prefunded = user_prefunding_requirement(&transfer_contract_steps(BOB, 1));
    let user_b_prefunded = user_prefunding_requirement(&transfer_contract_steps(ALICE, 1));
    let user_a = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let user_b = create_user_with(
      BOB,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(ALICE, 1),
    );
    let _ = (user_a_prefunded, user_b_prefunded);
    set_actor_cycle_nonce_coherent(user_a, u64::MAX);
    set_actor_cycle_nonce_coherent(user_b, u64::MAX);
    let system_alive = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let sweep_ids: BoundedVec<u64, <Test as crate::Config>::MaxSweepBatch> =
      BoundedVec::try_from(vec![user_a, user_b, system_alive]).expect("batch fits");
    assert_ok!(Actors::permissionless_sweep_many(
      RuntimeOrigin::signed(CHARLIE),
      sweep_ids,
    ));
    assert!(Actors::active_actor_view(user_a).is_none());
    assert!(Actors::active_actor_view(user_b).is_none());
    assert!(Actors::active_actor_view(system_alive).is_some());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::SweepBatchProcessed {
          requested: 3,
          closed: 2,
          alive: 1,
          missing: 0,
        }
      )
    }));
  });
}

#[test]
fn permissionless_sweep_many_rolls_back_prior_closes_on_late_failure() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let user_a_prefunded = user_prefunding_requirement(&transfer_contract_steps(BOB, 1));
    let user_b_prefunded = user_prefunding_requirement(&transfer_contract_steps(ALICE, 1));
    let user_a = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let user_b = create_user_with(
      BOB,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(ALICE, 1),
    );
    let _ = (user_a_prefunded, user_b_prefunded);
    set_actor_cycle_nonce_coherent(user_a, u64::MAX);
    set_actor_cycle_nonce_coherent(user_b, u64::MAX);
    OwnerSlotBitmaps::<Test>::remove(BOB);
    let sweep_ids: BoundedVec<u64, <Test as crate::Config>::MaxSweepBatch> =
      BoundedVec::try_from(vec![user_a, user_b]).expect("batch fits");
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();

    assert_noop!(
      Actors::permissionless_sweep_many(RuntimeOrigin::signed(CHARLIE), sweep_ids),
      Error::<Test>::InvalidOwnerSlot
    );
    assert!(Actors::active_actor_view(user_a).is_some());
    assert!(Actors::active_actor_view(user_b).is_some());
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn percentage_at_opening_uses_preservable_native_snapshot_for_user() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let task = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageAtOpening(Perbill::one()),
    };
    let contract_steps = contract_steps_with_step(make_step(task.clone()));
    let pipeline_fee = pipeline_opening_fee(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      percentage_trigger_schedule(),
      None,
      contract_steps,
    );
    let actor = sovereign_account(actor_id);
    let funding = 500;
    fund_native(actor_id, funding);
    let expected_action_fee =
      <TestWeightToFee as polkadot_sdk::sp_weights::WeightToFee>::weight_to_fee(
        &Actors::weight_upper_bound(&task),
      );
    let actor_before = native_balance(&actor);
    let expected_transfer = actor_before
      .saturating_sub(address_event_trigger_fee())
      .saturating_sub(pipeline_fee)
      .saturating_sub(expected_action_fee)
      .saturating_sub(TestMinUserBalance::get());
    let bob_before = native_balance(&BOB);
    signal_percentage_trigger(actor_id, TestAsset::Native);
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&BOB),
      bob_before.saturating_add(expected_transfer)
    );
    assert_eq!(native_balance(&actor), TestMinUserBalance::get());
    assert!(expected_transfer > 0);
  });
}

#[test]
fn notify_address_event_updates_accumulator_without_resuming_paused_system_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(100)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 500);
    mutate_actor_hot_coherent(actor_id, |hot| {
      hot.lifecycle = ActiveLifecycle::Paused;
    });
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      500,
      &CHARLIE
    ));
    let updated = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(updated.lifecycle, ActiveLifecycle::Paused);
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&500)
    );
    assert_eq!(native_balance(&actor), 500);
    assert!(!has_actor_event(|event| {
      matches!(event, Event::ActorResumed { actor_id: id } if *id == actor_id)
    }));
  });
}

#[test]
fn actor_not_found_on_nonexistent_id() {
  new_test_ext().execute_with(|| {
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::signed(ALICE), 999),
      Error::<Test>::ActorNotFound
    );
  });
}

#[test]
fn actor_id_overflow_at_max() {
  new_test_ext().execute_with(|| {
    NextActorId::<Test>::put(u64::MAX);
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 10)),
      ),
      Error::<Test>::ActorIdOverflow
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn actor_id_collision_check_uses_frame_authority_with_canonical_control() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::active_actor_exists(actor_id));

    let next_actor_id = NextActorId::<Test>::get();
    NextActorId::<Test>::put(actor_id);
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, inert_contract_steps()),
      ),
      Error::<Test>::ActorIdOccupied
    );
    NextActorId::<Test>::put(next_actor_id);
    assert!(Actors::active_actor_exists(actor_id));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn actor_id_collision_rejects_each_orphan_canonical_partition_without_writes() {
  for partition in 0..5 {
    for actor_type in [ActorType::User, ActorType::System] {
      new_test_ext().execute_with(|| {
        let source = create_suspended_system_retry(1);
        let target = NextActorId::<Test>::get();
        match partition {
          0 => {
            let (_, mut cell) = Actors::actor_control_cell(source).expect("source primary");
            cell.actor_id = target;
            ActorUnsignaledControlCells::<Test>::insert(target, cell);
          }
          1 => ActorContractHeads::<Test>::insert(
            target,
            ActorContractHeads::<Test>::get(source).expect("source Contract head"),
          ),
          2 => ActorFunding::<Test>::insert(
            target,
            ActorFunding::<Test>::get(source).expect("source funding"),
          ),
          3 => ActorRunHeads::<Test>::insert(
            target,
            ActorRunHeads::<Test>::get(source).expect("source Run head"),
          ),
          4 => ActorRunPayloads::<Test>::insert(
            target,
            ActorRunPayloads::<Test>::get(source).expect("source Run payload"),
          ),
          _ => unreachable!(),
        }
        assert!(!ActorControlLocators::<Test>::contains_key(target));
        assert!(!ActorIdentities::<Test>::contains_key(target));
        assert_noop!(
          match actor_type {
            ActorType::User =>
              Actors::create_user_actor(RuntimeOrigin::signed(ALICE), Mutability::Mutable, None),
            ActorType::System =>
              Actors::create_system_actor(RuntimeOrigin::root(), ALICE, Mutability::Mutable, None),
          },
          Error::<Test>::ActorIdOccupied
        );
      });
    }
  }
}

#[cfg(feature = "try-runtime")]
#[test]
fn dormant_immutable_system_audit_requires_genesis_authority() {
  new_test_ext().execute_with(|| {
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None
    ));
    assert_ok!(Actors::do_try_state());
    ActorIdentities::<Test>::mutate(actor_id, |identity| {
      identity
        .as_mut()
        .expect("public Dormant identity exists")
        .mutability = Mutability::Immutable;
    });
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_eq!(
      Actors::do_try_state(),
      Err(polkadot_sdk::sp_runtime::TryRuntimeError::Other(
        "Immutable Dormant System Actor is not declared by genesis",
      )),
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });
}

#[test]
fn duplicate_dormant_and_active_identity_rejects_eligibility_and_close() {
  new_test_ext().execute_with(|| {
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let identity = Actors::actor_identity(actor_id).expect("active identity");
    ActorIdentities::<Test>::insert(actor_id, identity);
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_eq!(
      Actors::actor_eligibility(actor_id),
      Err(ActorClassificationError::ActorInvariant)
    );
    assert!(Actors::actor_identity(actor_id).is_none());
    assert_noop!(
      Actors::close_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ActorInvariant
    );
    assert_noop!(
      Actors::resume_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ActorInvariant
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });
}

#[test]
fn eligibility_rejects_partial_or_mismatched_run_tiers_in_nonrunning_states() {
  for identity_state in 0..3 {
    for run_shape in 0..3 {
      new_test_ext().execute_with(|| {
        let source = create_suspended_system_retry(1);
        let target = NextActorId::<Test>::get();
        match identity_state {
          0 => assert_eq!(eligibility(target), ActorEligibility::NotRegistered),
          1 => {
            assert_ok!(Actors::create_system_actor(
              RuntimeOrigin::root(),
              BOB,
              Mutability::Mutable,
              None
            ));
            assert_eq!(eligibility(target), ActorEligibility::Dormant);
          }
          2 => {
            assert_eq!(
              create_system_with(BOB, manual_schedule(), None, inert_contract_steps()),
              target
            );
            assert!(matches!(eligibility(target), ActorEligibility::Active(_)));
          }
          _ => unreachable!(),
        }
        if run_shape != 0 {
          let mut head = ActorRunHeads::<Test>::get(source).expect("source Run head");
          if run_shape == 2 {
            head.payload_commitment[0] ^= 1;
          }
          ActorRunHeads::<Test>::insert(target, head);
        }
        if run_shape != 1 {
          ActorRunPayloads::<Test>::insert(
            target,
            ActorRunPayloads::<Test>::get(source).expect("source Run payload"),
          );
        }
        assert!(ActorRunStateStore::<Test>::get(target).is_none());
        let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
        assert_eq!(
          Actors::actor_eligibility(target),
          Err(ActorClassificationError::ActorInvariant)
        );
        assert_noop!(
          Actors::resume_actor(RuntimeOrigin::root(), target),
          Error::<Test>::ActorInvariant
        );
        assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
      });
    }
  }
}

#[test]
fn idle_close_removes_terminal_and_temporal_waiting_owners() {
  for cadenced in [false, true] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(
        ALICE,
        if cadenced {
          timer_schedule(20)
        } else {
          manual_schedule()
        },
        (!cadenced).then_some(ScheduleWindow { start: 1, end: 101 }),
        inert_contract_steps(),
      );
      let (location, cell) = Actors::actor_control_cell(actor_id).expect("idle primary");
      assert_eq!(cell.hot.cycle_state, CycleState::Idle);
      let ActorControlLocation::Waiting { key, page, slot } = location else {
        panic!("deadline owns the Idle primary");
      };
      if cadenced {
        let pointer = cell.hot.trigger_wakeup_pointer.expect("temporal pointer");
        assert_eq!(
          (key, page, u32::from(slot)),
          (WakeupKey::Tick(pointer.tick), pointer.page_id, pointer.slot)
        );
        assert!(cell.hot.wakeup_pointer.is_none());
      } else {
        let pointer = cell.hot.wakeup_pointer.expect("terminal pointer");
        assert_eq!(pointer.block, WakeupKey::Block(102));
        assert_eq!(
          (key, page, u32::from(slot)),
          (pointer.block, pointer.page_id, pointer.slot)
        );
      }
      let page = crate::ActorWaitingFrameChunks::<Test>::get((key, page)).expect("Waiting page");
      assert!(matches!(
        page.entries[slot as usize],
        Some(crate::ActorWaitingEntry::Primary(_))
      ));
      assert_eq!(cell.hot.trigger_wakeup_pointer.is_some(), cadenced);
      assert_ok!(Actors::close_actor(RuntimeOrigin::root(), actor_id));
      assert_eq!(Actors::active_actor_count(), 0);
      assert_eq!(Actors::actor_identity_count(), 0);
      assert_eq!(ActorControlLocators::<Test>::iter_keys().count(), 0);
      assert_eq!(
        crate::ActorWaitingFrameChunks::<Test>::iter_keys().count(),
        0
      );
      assert_eq!(crate::ActorWaitingHeads::<Test>::iter_keys().count(), 0);
      assert_eq!(crate::ActorWaitingTails::<Test>::iter_keys().count(), 0);
      assert_eq!(
        crate::ActorWaitingOccupancies::<Test>::iter_keys().count(),
        0
      );
      assert_eq!(
        crate::ActorWaitingCursorIndices::<Test>::iter_keys().count(),
        0
      );
      for clock in [WakeupClock::Block, WakeupClock::Tick] {
        assert_eq!(crate::WakeupCursorLen::<Test>::get(clock), 0);
      }
      assert_eq!(crate::WakeupCursorPages::<Test>::iter_keys().count(), 0);
    });
  }
}

#[test]
fn idle_contract_replacement_rebinds_retained_waiting_admission_and_executes_due_work() {
  for cadenced in [false, true] {
    for policy_only in [false, true] {
      new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(1);
        let actor_id = create_system_with(
          ALICE,
          if cadenced {
            timer_schedule(20)
          } else {
            manual_schedule()
          },
          (!cadenced).then_some(ScheduleWindow { start: 1, end: 101 }),
          inert_contract_steps(),
        );
        let (_, before) = Actors::actor_control_cell(actor_id).expect("idle primary");
        let mut contract = Actors::load_actor_contract(actor_id).expect("Contract");
        if policy_only {
          contract.completion = crate::CompletionPolicy::CloseAfterProductiveCycle;
        } else {
          contract
            .steps
            .try_push(make_step(Task::StopCycle))
            .expect("extra Step fits");
        }
        frame_system::Pallet::<Test>::set_block_number(2);
        assert_ok!(Actors::update_contract(
          RuntimeOrigin::root(),
          actor_id,
          contract
        ));
        let (_, after) = Actors::actor_control_cell(actor_id).expect("updated primary");
        assert_ne!(
          after.admission.admission_identity,
          before.admission.admission_identity
        );
        assert_eq!(after.hot.wakeup_pointer, before.hot.wakeup_pointer);
        assert_eq!(
          after.hot.trigger_wakeup_pointer,
          before.hot.trigger_wakeup_pointer
        );
        let pointers = [
          after.hot.wakeup_pointer,
          after
            .hot
            .trigger_wakeup_pointer
            .map(|pointer| WakeupPointer {
              block: WakeupKey::Tick(pointer.tick),
              page_id: pointer.page_id,
              slot: pointer.slot,
            }),
        ];
        for pointer in pointers.into_iter().flatten() {
          let page = crate::ActorWaitingFrameChunks::<Test>::get((pointer.block, pointer.page_id))
            .expect("retained Waiting page");
          let (stored_actor, admission) = match page.entries[pointer.slot as usize]
            .as_ref()
            .expect("retained Waiting entry")
          {
            crate::ActorWaitingEntry::Primary(cell) => {
              (cell.actor_id, cell.admission.admission_identity)
            }
            crate::ActorWaitingEntry::Reference(reference) => {
              (reference.actor_id, reference.admission_identity)
            }
          };
          assert_eq!(stored_actor, actor_id);
          assert_eq!(admission, after.admission.admission_identity);
        }
        if !cadenced {
          assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
        }
        for block in 3..=25 {
          frame_system::Pallet::<Test>::set_block_number(block);
          run_idle(Weight::MAX);
          if Actors::actor_identity(actor_id).is_some_and(|identity| identity.cycle_nonce == 1) {
            break;
          }
        }
        assert_eq!(
          Actors::actor_identity(actor_id)
            .expect("first cycle completes")
            .cycle_nonce,
          1
        );
        if cadenced {
          assert_ok!(Actors::close_actor(RuntimeOrigin::root(), actor_id));
        } else {
          frame_system::Pallet::<Test>::set_block_number(102);
          Actors::on_initialize(102);
          run_prepass();
          run_idle(Weight::MAX);
        }
        assert!(
          Actors::actor_identity(actor_id).is_none(),
          "cadenced={cadenced} policy_only={policy_only}: locator={:?}, hot={:?}, fault={:?}",
          ActorControlLocators::<Test>::get(actor_id),
          Actors::actor_hot(actor_id),
          Actors::wakeup_worker_fault(),
        );
        assert_eq!(
          crate::ActorWaitingFrameChunks::<Test>::iter_keys().count(),
          0
        );
        assert!(Actors::wakeup_worker_fault().is_none());
      });
    }
  }
}

#[test]
fn control_authority_matrix_is_class_mutability_and_breaker_complete() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actors = [
      (
        create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          inert_contract_steps(),
        ),
        ActorType::User,
      ),
      (
        create_user_with(
          ALICE,
          Mutability::Immutable,
          timer_schedule(2),
          None,
          inert_contract_steps(),
        ),
        ActorType::User,
      ),
      (
        create_system_with_mutability(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          inert_contract_steps(),
        ),
        ActorType::System,
      ),
      (
        create_system_with_mutability(
          ALICE,
          Mutability::Immutable,
          timer_schedule(2),
          None,
          inert_contract_steps(),
        ),
        ActorType::System,
      ),
    ];

    for breaker in [false, true] {
      GlobalCircuitBreaker::<Test>::put(breaker);
      for (actor_id, actor_type) in actors {
        let actor = Actors::active_actor_view(actor_id).expect("matrix actor exists");
        assert_ok!(Actors::ensure_control_origin(
          RuntimeOrigin::signed(ALICE),
          &actor,
        ));
        assert_noop!(
          Actors::ensure_control_origin(RuntimeOrigin::signed(BOB), &actor),
          Error::<Test>::NotOwner
        );
        if actor_type == ActorType::System {
          assert_ok!(Actors::ensure_control_origin(RuntimeOrigin::root(), &actor));
        } else {
          assert_noop!(
            Actors::ensure_control_origin(RuntimeOrigin::root(), &actor),
            Error::<Test>::NotGovernance
          );
        }
      }
    }
  });
}

#[test]
fn not_owner_on_foreign_actor() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::signed(BOB), actor_id),
      Error::<Test>::NotOwner
    );
  });
}

#[test]
fn not_governance_on_user_actor_via_root() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::NotGovernance
    );
  });
}

#[test]
fn governance_can_manage_system_actor_control_surface() {
  new_test_ext().execute_with(|| {
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::pause_actor(RuntimeOrigin::root(), actor_id));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("system actor")
        .lifecycle,
      ActiveLifecycle::Paused
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::root(), actor_id));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("system actor")
        .lifecycle,
      ActiveLifecycle::Active
    );
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    assert!(
      Actors::active_actor_view(actor_id)
        .expect("system actor")
        .pending_signal
    );
    let updated_schedule = timer_schedule(3);
    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      updated_schedule.clone(),
      None,
    ));
    let updated_view = Actors::active_actor_view(actor_id).expect("system actor");
    assert_eq!(updated_view.trigger, updated_schedule.trigger);
    assert_eq!(
      updated_view.cooldown_blocks,
      updated_schedule.cooldown_blocks
    );
    mutate_actor_hot_coherent(actor_id, |hot| hot.unsuccessful_attempt_streak = 2);
    frame_system::Pallet::<Test>::set_block_number(4);
    let updated_plan = transfer_contract_steps(BOB, 1);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      updated_plan.clone(),
      crate::CompletionPolicy::Persistent,
    ));
    let updated = Actors::active_actor_view(actor_id).expect("system actor");
    assert_eq!(updated.steps, updated_plan);
    assert_eq!(updated.unsuccessful_attempt_streak, 0);
    frame_system::Pallet::<Test>::set_block_number(5);
    assert_ok!(replace_auto_close(RuntimeOrigin::root(), actor_id, Some(5),));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("system actor")
        .auto_close_at_cycle_nonce,
      Some(5)
    );
    frame_system::Pallet::<Test>::set_block_number(6);
    assert_ok!(replace_auto_close(RuntimeOrigin::root(), actor_id, Some(7),));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("system actor")
        .auto_close_at_cycle_nonce,
      Some(7)
    );
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), actor_id));
    assert!(Actors::active_actor_view(actor_id).is_none());
  });
}

#[test]
fn resume_on_active_is_an_exact_noop() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    let before = Actors::active_actor_view(actor_id).expect("actor exists");
    System::reset_events();
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(Actors::active_actor_view(actor_id), Some(before));
    assert!(System::events().is_empty());
  });
}

#[test]
fn pause_on_paused_is_an_exact_noop() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    frame_system::Pallet::<Test>::set_block_number(1);
    let before = Actors::active_actor_view(actor_id).expect("actor exists");
    System::reset_events();
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(Actors::active_actor_view(actor_id), Some(before));
    assert!(System::events().is_empty());
  });
}

#[test]
fn already_paused_on_manual_trigger() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_noop!(
      Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ActorPaused
    );
  });
}

#[test]
fn governance_updates_active_actor_limit_and_creation_respects_it() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let old_limit: u32 = <<Test as crate::Config>::MaxActiveActors as Get<u32>>::get()
      .min(<<Test as crate::Config>::MaxQueueLength as Get<u32>>::get());
    assert_eq!(Actors::configured_active_actor_limit(), old_limit);
    assert_ok!(Actors::set_active_actor_limit(RuntimeOrigin::root(), 2));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActiveActorLimitSet {
          old_limit: prev,
          new_limit: 2,
        } if *prev == old_limit
      )
    }));
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::set_active_actor_limit(RuntimeOrigin::root(), 2));
    assert!(frame_system::Pallet::<Test>::events().is_empty());
    let _ = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let _ = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::ActiveActorCapacityExceeded
    );
  });
}

#[test]
fn active_actor_limit_update_validates_bounds() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let _ = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let _ = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_noop!(
      Actors::set_active_actor_limit(RuntimeOrigin::root(), 1),
      Error::<Test>::ActiveActorLimitBelowCurrent
    );
    assert_noop!(
      Actors::set_active_actor_limit(RuntimeOrigin::root(), 0),
      Error::<Test>::ActiveActorLimitTooLow
    );
    assert_noop!(
      Actors::set_active_actor_limit(RuntimeOrigin::root(), u32::MAX),
      Error::<Test>::ActiveActorLimitTooHigh
    );
    assert_noop!(
      Actors::set_active_actor_limit(
        RuntimeOrigin::root(),
        <<Test as crate::Config>::MaxQueueLength as Get<u32>>::get().saturating_add(1),
      ),
      Error::<Test>::ActiveActorLimitExceedsQueueCapacity
    );
    crate::ActiveActorLimit::<Test>::put(0);
    assert_eq!(Actors::effective_active_actor_limit(), 0);
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::ActiveActorCapacityExceeded
    );
    #[cfg(feature = "try-runtime")]
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn invalid_schedule_window_end_before_start() {
  new_test_ext().execute_with(|| {
    let window = ScheduleWindow {
      start: 100,
      end: 50,
    };
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(
          manual_schedule(),
          Some(window),
          transfer_contract_steps(BOB, 10)
        ),
      ),
      Error::<Test>::InvalidScheduleWindow
    );
  });
}

#[test]
fn schedule_window_requires_an_exact_end_plus_one_terminal_block() {
  new_test_ext().execute_with(|| {
    let window = ScheduleWindow {
      start: u64::MAX - 100,
      end: u64::MAX,
    };
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(
          manual_schedule(),
          Some(window),
          transfer_contract_steps(BOB, 10),
        ),
      ),
      Error::<Test>::InvalidScheduleWindow
    );
  });
}

#[test]
fn in_progress_window_is_admissible_when_now_is_inside() {
  new_test_ext().execute_with(|| {
    // Section 7.3.3: an in-progress window (start <= now <= end) is admissible;
    // only an already-expired window (end < now) is rejected.
    frame_system::Pallet::<Test>::set_block_number(50);
    let window = ScheduleWindow {
      start: 10,
      end: 200,
    };
    prefund_active_user_creation(ALICE, &transfer_contract_steps(BOB, 10));
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(
        manual_schedule(),
        Some(window),
        transfer_contract_steps(BOB, 10),
      ),
    ));
  });
}

#[test]
fn already_expired_window_is_rejected() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(300);
    let window = ScheduleWindow {
      start: 10,
      end: 200,
    };
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(
          manual_schedule(),
          Some(window),
          transfer_contract_steps(BOB, 10),
        ),
      ),
      Error::<Test>::InvalidScheduleWindow
    );
  });
}

#[test]
fn inclusive_window_span_exact_minimum_is_admissible() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // Inclusive span: end - start + 1 >= MinWindowLength (100). start 1, end 100
    // has span 100 and is admissible; start 1, end 99 has span 99 and is rejected.
    prefund_active_user_creation(ALICE, &transfer_contract_steps(BOB, 10));
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(
        manual_schedule(),
        Some(ScheduleWindow { start: 1, end: 100 }),
        transfer_contract_steps(BOB, 10),
      ),
    ));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(BOB),
        Mutability::Mutable,
        user_active_contract(
          manual_schedule(),
          Some(ScheduleWindow { start: 1, end: 99 }),
          transfer_contract_steps(CHARLIE, 10),
        ),
      ),
      Error::<Test>::InvalidScheduleWindow
    );
  });
}

#[test]
fn future_window_terminal_and_exact_next_block_reject_overflow_without_mutation() {
  new_test_ext().execute_with(|| {
    let max = u64::MAX;
    frame_system::Pallet::<Test>::set_block_number(max - 101);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow {
        start: max - 100,
        end: max - 1,
      }),
      inert_contract_steps(),
    );
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot").terminal_at,
      Some(max)
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(u64::MAX);
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, inert_contract_steps()),
      ),
      Error::<Test>::SchedulerIndexExhausted
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });
}

#[test]
fn mint_works_for_system_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Mint {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(500),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    // Mint on empty account works — mint policy skips source-balance check
    let before = native_balance(&actor);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), before + 500);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::MintExecuted { actor_id: id, asset: TestAsset::Native, amount: 500, .. } if *id == actor_id
    )));
  });
}

#[test]
fn mint_percentage_at_opening_uses_target_not_preservable_surface() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(5);
    let contract_steps = contract_steps_with_step(make_step(Task::Mint {
      asset,
      amount: AmountResolution::PercentageAtOpening(Perbill::from_percent(50)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 100);

    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);

    assert_eq!(asset_balance(&actor, asset), 150);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::MintExecuted {
        actor_id: id,
        asset: minted_asset,
        amount: 50,
        ..
      } if *id == actor_id && *minted_asset == asset
    )));
  });
}

#[test]
fn invalid_schedule_window_too_short() {
  new_test_ext().execute_with(|| {
    // MinWindowLength = 100 in mock
    let window = ScheduleWindow { start: 10, end: 50 };
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(
          manual_schedule(),
          Some(window),
          transfer_contract_steps(BOB, 10)
        ),
      ),
      Error::<Test>::InvalidScheduleWindow
    );
  });
}

#[test]
fn manual_readiness_mutates_hot_state_without_rewriting_contract() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let contract_before = Actors::actor_contract(actor_id).expect("actor contract exists");

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    assert!(
      Actors::actor_hot(actor_id)
        .expect("actor hot state exists")
        .pending_signal
    );
    assert_eq!(Actors::actor_contract(actor_id), Some(contract_before));
  });
}

#[test]
fn active_actors_set_maintains_integrity() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let schedule = timer_schedule(1);
    let inert_plan = inert_contract_steps();
    for _ in 0..3 {
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(schedule.clone(), None, inert_plan.clone()),
      ));
    }
    assert_eq!(ActorControlLocators::<Test>::iter_keys().count(), 3);
    assert!(Actors::active_actor_view(0).is_some());
    assert!(Actors::active_actor_view(1).is_some());
    assert!(Actors::active_actor_view(2).is_some());
    let inst = Actors::active_actor_view(1).unwrap();
    let _ = Balances::deposit_creating(&inst.sovereign_account, 1_000_000);
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), 1));
    assert_eq!(ActorControlLocators::<Test>::iter_keys().count(), 2);
    assert!(Actors::active_actor_view(0).is_some());
    assert!(Actors::active_actor_view(2).is_some());
    assert!(Actors::active_actor_view(1).is_none());
  });
}

#[test]
fn auto_close_configuration_enforces_origin_mutability_and_target_rules() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let immutable_id = create_user_with(
      ALICE,
      Mutability::Immutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    assert_noop!(
      replace_auto_close(RuntimeOrigin::signed(ALICE), immutable_id, Some(2)),
      Error::<Test>::ImmutableActor
    );
    let mutable_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_noop!(
      replace_auto_close(RuntimeOrigin::signed(BOB), mutable_id, Some(2)),
      Error::<Test>::NotOwner
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      mutable_id
    ));
    run_idle(Weight::MAX);
    assert_noop!(
      replace_auto_close(RuntimeOrigin::signed(ALICE), mutable_id, Some(1)),
      Error::<Test>::InvalidAutoCloseNonce
    );
    let horizon = TestMaxAutoCloseNonceHorizon::get();
    assert_noop!(
      replace_auto_close(
        RuntimeOrigin::signed(ALICE),
        mutable_id,
        Some(1u64.saturating_add(horizon).saturating_add(1)),
      ),
      Error::<Test>::AutoCloseNonceHorizonExceeded
    );
    let boundary_target = 1u64.saturating_add(horizon);
    assert_ok!(replace_auto_close(
      RuntimeOrigin::signed(ALICE),
      mutable_id,
      Some(boundary_target),
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_noop!(
      replace_auto_close(
        RuntimeOrigin::signed(ALICE),
        mutable_id,
        Some(boundary_target.saturating_add(1)),
      ),
      Error::<Test>::AutoCloseNonceHorizonExceeded
    );
    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(replace_auto_close(
      RuntimeOrigin::signed(ALICE),
      mutable_id,
      None,
    ));
    assert_eq!(
      Actors::active_actor_view(mutable_id)
        .expect("system actor remains active")
        .auto_close_at_cycle_nonce,
      None
    );
  });
}

#[test]
fn system_immutable_creation_rejects_manual_but_allows_internal_window_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Immutable,
        system_active_contract(manual_schedule(), None, inert_contract_steps()),
      ),
      Error::<Test>::InvalidTriggerConfiguration
    );
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Immutable,
      system_active_contract(
        on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
        Some(ScheduleWindow { start: 1, end: 101 }),
        inert_contract_steps(),
      ),
    ));
    let actor_id = Actors::next_actor_id().saturating_sub(1);
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert_noop!(
      Actors::close_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ImmutableActor
    );
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());
    frame_system::Pallet::<Test>::set_block_number(102);
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed { actor_id: id, reason: CloseReason::WindowExpired } if *id == actor_id
    )));
  });
}

#[test]
fn close_cleanup_admission_uses_measured_pure_close_weight() {
  new_test_ext().execute_with(|| {
    let measured = <TestWeightInfo as crate::WeightInfo>::close_actor();
    assert!(measured.ref_time() > 0);
    assert!(measured.proof_size() > 0);
    assert_eq!(Actors::close_cleanup_weight_upper(), measured);
    assert_eq!(Actors::close_dispatch_weight_upper(), measured);
  });
}

#[test]
fn eligibility_projection_reports_not_registered_and_dormant() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let fresh_id = Actors::next_actor_id();
    assert_eq!(eligibility(fresh_id), ActorEligibility::NotRegistered);

    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None,
    ));
    assert_eq!(eligibility(fresh_id), ActorEligibility::Dormant);
  });
}

#[test]
fn classifier_projections_agree_on_breaker_terminal_and_paused_products() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let window = ScheduleWindow { start: 1, end: 101 };
    let contract_steps = inert_contract_steps();
    let expected_contract =
      system_active_contract(manual_schedule(), Some(window), contract_steps.clone())
        .expect("direct Actor Contract");
    let actor_id = create_system_with(ALICE, manual_schedule(), Some(window), contract_steps);
    frame_system::Pallet::<Test>::set_block_number(102);
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    let instance = Actors::active_actor_view(actor_id).expect("actor exists");
    let classification = Actors::classify_actor(actor_id, &instance).expect("classification");
    assert_eq!(
      classification.terminal_reason,
      Some(CloseReason::WindowExpired)
    );
    assert_eq!(
      classification.execution_phase,
      ActorExecutionPhase::GlobalCircuitBreaker
    );
    assert_eq!(active_eligibility(actor_id), classification);
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        expected_contract.clone(),
        SimulationMode::FreshCurrentPlan,
        ample_simulation_budget(),
      ),
      Err(SimulationError::GlobalCircuitBreaker)
    );

    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      false
    ));
    assert_eq!(
      active_eligibility(actor_id).terminal_reason,
      Some(CloseReason::WindowExpired)
    );
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        expected_contract,
        SimulationMode::FreshCurrentPlan,
        ample_simulation_budget(),
      )
      .expect("terminal simulation projects")
      .status,
      AttemptDisposition::Closed(CloseReason::WindowExpired)
    );
    assert_ok!(Actors::permissionless_sweep(
      RuntimeOrigin::signed(BOB),
      actor_id
    ));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::WindowExpired,
      } if *id == actor_id
    )));
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = inert_contract_steps();
    let expected_contract = system_active_contract(manual_schedule(), None, contract_steps.clone())
      .expect("direct Actor Contract");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    assert_ok!(Actors::pause_actor(RuntimeOrigin::root(), actor_id));
    let instance = Actors::active_actor_view(actor_id).expect("actor exists");
    assert_eq!(
      Actors::classify_actor(actor_id, &instance)
        .expect("classification")
        .execution_phase,
      ActorExecutionPhase::Paused
    );
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::Paused
    );
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        expected_contract,
        SimulationMode::FreshCurrentPlan,
        ample_simulation_budget(),
      ),
      Err(SimulationError::Paused)
    );
  });
}

#[test]
fn eligibility_projection_ready_after_signal_and_waits_without_latch() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingSignal
    );

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::Ready
    );
  });
}

#[test]
fn eligibility_projection_waits_for_cooldown_and_reports_next_block() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 5,
    };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule,
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 2_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      1
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingBlock(6)
    );
  });
}

#[test]
fn eligibility_projection_waits_for_schedule_window_until_gate() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow {
        start: 50,
        end: 150,
      }),
      inert_contract_steps(),
    );
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingBlock(50)
    );

    frame_system::Pallet::<Test>::set_block_number(50);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::Ready
    );
  });
}

#[test]
fn eligibility_projection_reports_paused_breaker_and_window_expired() {
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
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::Paused
    );
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::GlobalCircuitBreaker
    );
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow {
        start: 50,
        end: 150,
      }),
      inert_contract_steps(),
    );
    frame_system::Pallet::<Test>::set_block_number(151);
    assert_eq!(
      active_eligibility(actor_id).terminal_reason,
      Some(CloseReason::WindowExpired)
    );
  });
}

#[test]
fn eligibility_projection_reports_failure_limit_auto_close_and_nonce_exhaustion() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    mutate_actor_hot_coherent(actor_id, |hot| {
      hot.unsuccessful_attempt_streak = <Test as crate::Config>::MaxConsecutiveFailures::get();
    });
    assert_eq!(
      active_eligibility(actor_id).terminal_reason,
      Some(CloseReason::ConsecutiveFailures)
    );
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor")
        .cycle_nonce,
      1
    );
    // A failed terminal cycle leaves the incremented nonce at the target without
    // closing; the next admission closes before any further cycle (spec 2.4).
    let mut contract = Actors::load_actor_contract(actor_id).expect("active Actor Contract");
    contract.auto_close_at_cycle_nonce = Some(1);
    assert_ok!(Actors::store_actor_contract(actor_id, contract));
    assert_eq!(
      active_eligibility(actor_id).terminal_reason,
      Some(CloseReason::AutoCloseNonceReached)
    );
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    set_actor_cycle_nonce_coherent(actor_id, u64::MAX);
    assert_eq!(
      active_eligibility(actor_id).terminal_reason,
      Some(CloseReason::CycleNonceExhausted)
    );
  });
}
