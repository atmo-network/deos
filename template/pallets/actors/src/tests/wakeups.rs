use super::*;

#[test]
fn paged_wakeup_primitives_encode_exact_pointer_and_bounded_page_ownership() {
  let pointer = WakeupPointer {
    block: WakeupKey::Block(42u64),
    page_id: 7,
    slot: 3,
  };
  assert_eq!(pointer.block, WakeupKey::Block(42));
  assert_eq!(pointer.page_id, 7);
  assert_eq!(pointer.slot, 3);

  let reference = crate::ActorWakeupReference {
    actor_id: 9,
    admission_identity: [1; 32],
  };
  let entries = crate::ActorWaitingChunkOf::<Test>::try_from(vec![
    Some(crate::ActorWaitingEntry::Reference(reference.clone())),
    None,
  ])
  .expect("wakeup page entries fit");
  let page = WakeupPage {
    entries,
    live_entries: 1,
    scan_slot: 0,
    previous_page: Some(6),
    next_page: Some(8),
  };
  assert_eq!(
    page.entries[0],
    Some(crate::ActorWaitingEntry::Reference(reference))
  );
  assert_eq!(page.entries[1], None);
  assert_eq!(page.live_entries, 1);
  assert_eq!((page.previous_page, page.next_page), (Some(6), Some(8)));

  let bucket = crate::WakeupBucketState {
    head_page: 6,
    tail_page: 8,
    next_page_id: 9,
    live_entries: 65,
    cursor_index: Some(3),
  };
  assert_eq!(bucket.head_page, 6);
  assert_eq!(bucket.tail_page, 8);
  assert_eq!(bucket.next_page_id, 9);
  assert_eq!(bucket.live_entries, 65);
}

#[test]
fn creation_and_activation_before_cutoff_use_exact_next_block_wakeup() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    crate::ActorReadyHead::<Test>::put(u64::MAX);
    crate::ActorReadyTail::<Test>::put(u64::MAX);
    prefund_active_user_creation(ALICE, &transfer_contract_steps(BOB, 1));
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(timer_schedule(1), None, transfer_contract_steps(BOB, 1)),
    ));
    assert_eq!(scheduled_wakeup_block(0), Some(2));
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      user_active_contract(timer_schedule(1), None, transfer_contract_steps(BOB, 1)),
    ));
    assert_eq!(scheduled_wakeup_block(1), Some(2));
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      None,
    ));
    let actor_id = Actors::next_actor_id() - 1;
    crate::ActorReadyHead::<Test>::put(u64::MAX);
    crate::ActorReadyTail::<Test>::put(u64::MAX);
    prefund_user_sovereign(ALICE, 0, &transfer_contract_steps(BOB, 1));
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      user_active_contract(timer_schedule(1), None, transfer_contract_steps(BOB, 1))
        .expect("direct Actor Contract"),
    ));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(3));
  });
}

#[test]
fn canonical_temporal_occurrence_ignores_the_legacy_ticket_namespace() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      timer_schedule(1),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 100);
    frame_system::Pallet::<Test>::set_block_number(2);
    // A canonically published Actor owns its temporal residence in the generation-bound
    // `TriggerDeadlineHandles`/Service ring carriers, so a saturated pre-cutover paged ticket
    // namespace cannot block or close its occurrence.
    crate::ActorReadyHead::<Test>::put(u64::MAX);
    crate::ActorReadyTail::<Test>::put(u64::MAX);
    let bob_before = native_balance(&BOB);
    System::reset_events();

    run_canonical_block_at(2, Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    run_canonical_block_at(3, Weight::MAX);

    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert!(Actors::active_actor_exists(actor_id));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleStarted {
        actor_id: id,
        ..
      } if *id == actor_id
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActorClosed { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn cadence_update_replaces_live_future_wakeup_instead_of_accumulating() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(20), None, inert_contract_steps());
    let initial_block = scheduled_wakeup_block(actor_id).expect("timer wakeup should be scheduled");
    assert_eq!(scheduled_wakeup_block(actor_id), Some(initial_block));
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      timer_schedule(5),
      None,
    ));
    let rescheduled_block = scheduled_wakeup_block(actor_id).expect("replacement wakeup");
    // The replacement re-anchors the cadence at the update block: exactly one canonical Trigger
    // deadline, at the first period point strictly after the replacement anchor, replaces the
    // superseded tick instead of accumulating a second live membership.
    assert_eq!(rescheduled_block, 7);
    assert_ne!(rescheduled_block, initial_block);
    assert_eq!(
      crate::TriggerDeadlineHandles::<Test>::get(actor_id).map(|handle| handle.key),
      Some(WakeupKey::Tick(rescheduled_block))
    );
    assert_eq!(
      Actors::actor_hot(actor_id)
        .and_then(|hot| hot.trigger_wakeup_pointer)
        .map(|pointer| pointer.tick),
      Some(rescheduled_block)
    );
    assert!(!crate::ActorWaitingOccupancies::<Test>::contains_key(
      WakeupKey::Tick(initial_block)
    ));
  });
}

#[test]
fn temporal_membership_try_state_rejects_wakeup_pointer_beyond_terminal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_contract_steps(),
    );
    assert_eq!(scheduled_wakeup_block(actor_id), Some(102));
    #[cfg(feature = "try-runtime")]
    {
      assert_ok!(crate::Pallet::<Test>::do_try_state());
      // Move the complete canonical Pipeline deadline beyond the unchanged terminal. The
      // Deadline carrier remains internally coherent, so try_state must reject the semantic bound
      // rather than an obsolete Contract or legacy-frame fixture.
      let actor = Actors::load_actor_ref(actor_id).expect("canonical Actor reference");
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        Actors::remove_deadline_member(actor).expect("original terminal deadline is removable");
        let replacement = Actors::plan_deadline_destination(actor, WakeupKey::Block(103))
          .expect("replacement deadline destination");
        let mut process = crate::ActorProcesses::<Test>::get(actor_id).expect("canonical process");
        process.residence = Some(crate::ProcessResidence::Deadline {
          key: replacement.key,
          page: replacement.page,
          slot: replacement.slot,
        });
        crate::ActorProcesses::<Test>::insert(actor_id, process);
        Actors::insert_deadline_member(replacement).expect("replacement deadline is inserted");
        mutate_actor_hot_coherent(actor_id, |hot| {
          hot.wakeup_pointer = Some(crate::WakeupPointer {
            block: replacement.key,
            page_id: replacement.page,
            slot: replacement.slot.into(),
          });
        });
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
      assert_eq!(
        crate::Pallet::<Test>::do_try_state().map_err(|error| format!("{error:?}")),
        Err(
          "Other(\"ActorControl Pipeline wakeup pointer exceeds its terminal membership\")".into()
        )
      );
    }
  });
}

#[test]
fn temporal_membership_try_state_accepts_exact_close_cleanup() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(20), None, inert_contract_steps());
    let scheduled_block = scheduled_wakeup_block(actor_id).expect("timer wakeup scheduled");
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(!crate::ActorWaitingOccupancies::<Test>::contains_key(
      WakeupKey::Tick(scheduled_block)
    ));
    assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick), 0);
    assert_eq!(
      crate::ActorWaitingFrameChunks::<Test>::iter_keys().count(),
      0
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    frame_system::Pallet::<Test>::set_block_number(scheduled_block);
    run_idle(Weight::MAX);
    assert!(!crate::ActorWaitingOccupancies::<Test>::contains_key(
      WakeupKey::Tick(scheduled_block)
    ));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn close_before_future_wakeup_removes_exact_waiting_membership() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(20), None, inert_contract_steps());
    let scheduled_block =
      scheduled_wakeup_block(actor_id).expect("timer wakeup should be scheduled");
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(scheduled_wakeup_block(actor_id).is_none());
    assert!(!crate::ActorWaitingOccupancies::<Test>::contains_key(
      WakeupKey::Tick(scheduled_block)
    ));
    assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick), 0);
    assert_eq!(crate::ActorWaitingFrameChunks::<Test>::iter().count(), 0);
    frame_system::Pallet::<Test>::set_block_number(scheduled_block);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(Actors::wakeup_buckets(scheduled_block).is_none());
    assert_eq!(Actors::queue_head(), Actors::queue_tail());
    assert!(!has_actor_event(|event| {
      matches!(event, Event::CycleStarted { actor_id: id, .. } if *id == actor_id)
    }));
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn canonical_close_removes_future_tick_and_releases_state_hold() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      timer_schedule(20),
      None,
      inert_contract_steps(),
    );
    let scheduled_tick = scheduled_wakeup_block(actor_id).expect("Tick wakeup is scheduled");
    assert!(Actors::actor_state_hold(actor_id).is_some());

    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));

    assert!(crate::ActorControlLocators::<Test>::get(actor_id).is_none());
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(Actors::actor_state_hold(actor_id).is_none());
    assert!(!crate::ActorWaitingOccupancies::<Test>::contains_key(
      WakeupKey::Tick(scheduled_tick)
    ));
    assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick), 0);
    assert_eq!(crate::ActorWaitingFrameChunks::<Test>::iter().count(), 0);
    frame_system::Pallet::<Test>::set_block_number(scheduled_tick);
    run_idle(Weight::MAX);
    assert!(Actors::wakeup_buckets(scheduled_tick).is_none());
    assert_eq!(Actors::queue_head(), Actors::queue_tail());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn repeated_timer_close_churn_leaves_no_wakeup_tombstones() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let total = <<Test as crate::Config>::MaxWakeupsPerBlock as Get<u32>>::get() + 2;
    let mut actors = Vec::new();
    let mut latest_wakeup = 1u64;
    for _ in 0..total {
      let actor_id = create_system_with(ALICE, timer_schedule(4_000), None, inert_contract_steps());
      let wakeup = scheduled_wakeup_block(actor_id).expect("timer wakeup must be scheduled");
      latest_wakeup = latest_wakeup.max(wakeup);
      actors.push(actor_id);
    }

    for actor_id in actors {
      assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
      assert!(scheduled_wakeup_block(actor_id).is_none());
    }
    assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick), 0);
    assert_eq!(crate::ActorWaitingFrameChunks::<Test>::iter().count(), 0);
    frame_system::Pallet::<Test>::reset_events();
    for offset in 0..10 {
      frame_system::Pallet::<Test>::set_block_number(
        latest_wakeup.saturating_add(1_000).saturating_add(offset),
      );
      run_idle(Weight::MAX);
      if crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick) == 0 {
        break;
      }
    }
    assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick), 0);
    assert_eq!(Actors::queue_head(), Actors::queue_tail());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { .. }
    )));
  });
}

#[test]
fn window_expiry_wakeup_closes_inactive_actor_without_identity_scan() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_contract_steps(),
    );
    assert_eq!(scheduled_wakeup_block(actor_id), Some(102));
    NextActorId::<Test>::put(10_000_000);
    run_canonical_block_at(102, Weight::MAX);
    run_canonical_block_at(103, Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::WindowExpired,
      } if *id == actor_id
    )));
  });
}

#[test]
fn paused_actor_retains_direct_window_expiry_wakeup() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_contract_steps(),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(102));
    run_canonical_block_at(102, Weight::MAX);
    run_canonical_block_at(103, Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
  });
}

#[test]
fn cancelled_continuation_exactly_invalidates_its_wakeup_before_reprime() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 10,
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
    run_idle(Weight::MAX);
    // The first canonical attempt is served at B+1 and the temporary failure suspends the retry at
    // the persisted cooldown deadline. Cancellation must release that exact process deadline.
    let retry_at = scheduled_wakeup_block(actor_id).expect("suspended retry deadline");
    assert_eq!(retry_at, 12);
    assert_eq!(
      crate::DeadlineHandles::<Test>::get(actor_id).map(|handle| handle.key),
      Some(WakeupKey::Block(retry_at))
    );

    assert_ok!(Actors::cancel_run(RuntimeOrigin::root(), actor_id));
    assert!(scheduled_wakeup_block(actor_id).is_none());
    assert!(!crate::DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(Actors::actor_run_state(actor_id).is_none());
    frame_system::Pallet::<Test>::set_block_number(retry_at);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor remains")
        .cycle_nonce,
      1
    );
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn queue_saturation_at_block_max_cannot_create_same_block_wakeup() {
  new_test_ext().execute_with(|| {
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    frame_system::Pallet::<Test>::set_block_number(u64::MAX);
    seed_saturated_tombstone_queue();
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    assert_eq!(
      Actors::enqueue(actor_id),
      Err(crate::EnqueueOutcome::SchedulerIndexExhausted)
    );

    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
    assert!(
      Actors::actor_hot(actor_id)
        .expect("hot")
        .wakeup_pointer
        .is_none()
    );
  });
}

#[test]
fn timer_wakeup_uses_exact_cadence_without_actor_phase() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let cadence = 20u32;
    let actor_id = create_system_with(ALICE, timer_schedule(cadence), None, inert_contract_steps());
    assert_eq!(scheduled_wakeup_block(actor_id), Some(21));

    run_canonical_block_at(21, Weight::MAX);
    assert_eq!(scheduled_wakeup_block(actor_id), None);
    run_canonical_block_at(22, Weight::MAX);
    assert_eq!(scheduled_wakeup_block(actor_id), Some(41));
  });
}
