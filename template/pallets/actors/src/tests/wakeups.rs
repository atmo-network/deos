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

#[cfg(all(feature = "try-runtime", not(feature = "runtime-benchmarks")))]
#[test]
fn try_state_reads_frame_wakeup_authority_without_scalar_hot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(1), None, inert_contract_steps());
    let Some(crate::ActorControlLocation::Waiting { key, page, slot }) =
      crate::ActorControlLocators::<Test>::get(actor_id)
    else {
      panic!("Cadenced actor owns one Waiting frame cell");
    };
    crate::ActorWaitingFrameChunks::<Test>::mutate((key, page), |maybe_chunk| {
      let cell = maybe_chunk
        .as_mut()
        .and_then(|page| page.entries.get_mut(slot as usize))
        .and_then(Option::as_mut)
        .and_then(crate::ActorWaitingEntry::primary_mut)
        .expect("Waiting frame cell exists");
      cell
        .hot
        .trigger_wakeup_pointer
        .as_mut()
        .expect("Cadenced pointer exists")
        .slot = u32::from(slot).saturating_add(1);
    });

    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn resumed_wakeup_worker_preserves_the_per_block_scan_cap() {
  new_test_ext().execute_with(|| {
    let stats = crate::WakeupDrainStats {
      entries_scanned: <Test as crate::Config>::MaxWakeupsPerBlock::get(),
      ..Default::default()
    };
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    assert_eq!(
      Actors::drain_overdue_wakeups_cursor_resuming(1, &mut meter, stats),
      stats
    );
    assert_eq!(meter.consumed(), Weight::zero());
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before,
      "a resumed wakeup family at its scan cap must not probe or mutate again"
    );
  });
}

#[test]
fn wakeup_fault_recording_admits_both_weight_dimensions_and_is_idempotent() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let fault = crate::WakeupWorkerFault {
      key: WakeupKey::Block(10),
      page: 0,
      class: crate::CrossingWorkerFaultClass::Invariant,
    };
    let required = <TestWeightInfo as crate::WeightInfo>::record_wakeup_worker_fault();

    let mut ref_time_short = WeightMeter::with_limit(Weight::from_parts(
      required.ref_time().saturating_sub(1),
      u64::MAX,
    ));
    assert!(!Actors::record_wakeup_worker_fault(
      &mut ref_time_short,
      fault
    ));
    assert!(crate::WakeupWorkerFaultState::<Test>::get().is_none());
    assert_eq!(System::events().len(), 0);

    let mut proof_short = WeightMeter::with_limit(Weight::from_parts(
      u64::MAX,
      required.proof_size().saturating_sub(1),
    ));
    assert!(!Actors::record_wakeup_worker_fault(&mut proof_short, fault));
    assert!(crate::WakeupWorkerFaultState::<Test>::get().is_none());
    assert_eq!(System::events().len(), 0);

    let mut admitted = WeightMeter::with_limit(required);
    assert!(Actors::record_wakeup_worker_fault(&mut admitted, fault));
    assert_eq!(admitted.consumed(), required);
    let events_after_first = System::events();

    let mut duplicate = WeightMeter::with_limit(Weight::MAX);
    assert!(!Actors::record_wakeup_worker_fault(
      &mut duplicate,
      crate::WakeupWorkerFault {
        class: crate::CrossingWorkerFaultClass::Other,
        ..fault
      },
    ));
    assert_eq!(duplicate.consumed(), Weight::zero());
    assert_eq!(crate::WakeupWorkerFaultState::<Test>::get(), Some(fault));
    assert_eq!(System::events(), events_after_first);
  });
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
fn wakeup_materialization_index_exhaustion_closes_without_an_attempt() {
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
    crate::ActorReadyHead::<Test>::put(u64::MAX);
    crate::ActorReadyTail::<Test>::put(u64::MAX);
    let bob_before = native_balance(&BOB);
    System::reset_events();

    run_idle(Weight::MAX);

    assert_eq!(native_balance(&BOB), bob_before);
    assert!(Actors::actor_identity(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_none());
    assert_eq!(Actors::combined_queue_occupancy(), 0);
    assert!(!crate::ActorWaitingOccupancies::<Test>::contains_key(
      WakeupKey::Block(2)
    ));
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
    assert_ne!(rescheduled_block, initial_block);
    assert!(!crate::ActorWaitingOccupancies::<Test>::contains_key(
      WakeupKey::Tick(initial_block)
    ));
    assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick), 1);
  });
}

#[test]
fn ticket_and_terminal_window_wakeup_coexist_under_one_pointer() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // Manual actor inside a bounded schedule window: the Manual trigger queues it (live FIFO
    // ticket); updating the schedule to a still-future window then installs the terminal-only
    // expiry wakeup, which must coexist with the live ticket (SCHED-MEMBERSHIP).
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
    let ticket = Actors::actor_hot(actor_id)
      .and_then(|hot| hot.queue_ticket)
      .expect("manual trigger queues the actor");
    assert_eq!(ticket, 0);

    // Re-schedule the same window; the terminal-only expiry wakeup is installed while the actor
    // keeps its live FIFO ticket.
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
    ));
    let terminal_wakeup =
      Actors::actor_hot(actor_id).and_then(|hot| hot.wakeup_pointer.map(|pointer| pointer.block));
    assert!(
      terminal_wakeup.is_some(),
      "terminal-only window wakeup must coexist with the live ticket"
    );
    assert_eq!(
      Actors::actor_hot(actor_id).and_then(|hot| hot.queue_ticket),
      Some(ticket),
      "the live FIFO ticket survives the schedule update"
    );
    assert_eq!(Actors::wakeup_cursor_len(), 1);
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
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
      // Drift the window and terminal membership together to a shorter terminal, leaving the
      // existing wakeup beyond it: the earlier-due service-point contract must fail try_state.
      let mut contract = Actors::load_actor_contract(actor_id).expect("contract");
      contract.window = Some(ScheduleWindow { start: 1, end: 49 });
      assert_ok!(Actors::store_actor_contract(actor_id, contract));
      mutate_primary_control_cell(actor_id, |cell| cell.hot.terminal_at = Some(50));
      assert_eq!(
        crate::Pallet::<Test>::do_try_state().map_err(|error| format!("{error:?}")),
        Err(
          "Other(\"ActorControl Pipeline wakeup pointer exceeds its terminal membership\")".into()
        )
      );
      mutate_primary_control_cell(actor_id, |cell| cell.hot.terminal_at = Some(102));
      let mut contract = Actors::load_actor_contract(actor_id).expect("contract");
      contract.window = Some(ScheduleWindow { start: 1, end: 101 });
      assert_ok!(Actors::store_actor_contract(actor_id, contract));
      assert_ok!(crate::Pallet::<Test>::do_try_state());
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
    frame_system::Pallet::<Test>::set_block_number(102);
    run_idle(Weight::MAX);
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
fn terminal_window_wakeup_survives_queue_saturation_and_continuation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    // Retryable swap inside a bounded window: the first attempt creates a Continuation whose
    // retry backoff would land far past the window end; the terminal expiry wakeup at end + 1
    // must win, and then close the actor even when the queue is fully saturated.
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      temporary_retry_swap_plan(),
    );
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    run_idle(Weight::MAX);
    assert!(
      Actors::actor_run_state(actor_id).is_some(),
      "retryable step leaves a Continuation"
    );
    assert_eq!(
      scheduled_wakeup_block(actor_id),
      Some(102),
      "terminal expiry at end + 1 wins over the retry backoff"
    );
    // Saturate the physical queue coherently while preserving the Continuation's live ticket.
    let existing_ticket = Actors::actor_hot(actor_id)
      .and_then(|hot| hot.queue_ticket)
      .expect("Continuation retains its live queue ticket");
    let (_, cell) = Actors::actor_control_cell(actor_id).expect("Continuation primary");
    seed_saturated_tombstone_queue();
    crate::ActorReadyFrameChunks::<Test>::mutate(existing_ticket / 32, |page| {
      page.as_mut().expect("saturated Ready page")[(existing_ticket % 32) as usize] = Some(cell);
    });
    crate::ActorReadyOccupancy::<Test>::put(1);
    frame_system::Pallet::<Test>::set_block_number(102);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(
      Actors::active_actor_view(actor_id).is_none(),
      "expiry closes the actor despite saturation and Continuation; head={} tail={} occupancy={} wakeup={:?} queue_ticket={:?}",
      Actors::queue_head(),
      Actors::queue_tail(),
      Actors::combined_queue_occupancy(),
      scheduled_wakeup_block(actor_id),
      Actors::actor_hot(actor_id).and_then(|hot| hot.queue_ticket),
    );
    assert!(Actors::actor_run_state(actor_id).is_none());
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
    frame_system::Pallet::<Test>::set_block_number(102);
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
  });
}

#[test]
fn continuation_attempt_rolls_back_when_retry_wakeup_topology_is_corrupt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    set_max_consecutive_failures(10);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, temporary_retry_swap_plan());
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let due = 2u64;
    frame_system::Pallet::<Test>::set_block_number(due);
    let next_retry = due.saturating_add(2);
    crate::ActorWaitingOccupancies::<Test>::insert(WakeupKey::Block(next_retry), 1);
    let actor_before = Actors::active_actor_view(actor_id).expect("queued continuation");
    let continuation_before = Actors::actor_run_state(actor_id)
      .expect("continuation before corrupt retry placement")
      .encode();
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    let _ = Actors::execute_cycle(Weight::MAX);

    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("Actor run survives failed placement")
        .encode(),
      continuation_before,
    );
    assert_eq!(System::events(), events_before, "attempt events roll back");
    assert!(
      Actors::actor_hot(actor_id)
        .expect("continuation remains queued")
        .queue_ticket
        .is_some()
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
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
    assert_eq!(scheduled_wakeup_block(actor_id), Some(11));
    assert!(Actors::wakeup_buckets(11).is_some());

    assert_ok!(Actors::cancel_run(RuntimeOrigin::root(), actor_id));
    assert!(scheduled_wakeup_block(actor_id).is_none());
    assert!(Actors::wakeup_buckets(11).is_none());
    frame_system::Pallet::<Test>::set_block_number(11);
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
fn pipeline_and_trigger_temporal_memberships_coexist_and_drain_independently() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = inert_contract_steps()[0].clone();
    let steps = BoundedVec::try_from(vec![step.clone(), step]).expect("two Steps fit");
    let actor_id = create_system_with(ALICE, timer_schedule(100), None, steps);
    let first_tick = Actors::actor_hot(actor_id)
      .and_then(|hot| hot.trigger_wakeup_pointer)
      .expect("initial Cadenced pointer")
      .tick;
    frame_system::Pallet::<Test>::set_block_number(first_tick);
    Actors::on_idle(first_tick, Weight::MAX);
    let run = Actors::actor_run_state(actor_id).expect("first Step leaves a Running suffix");
    assert_eq!(run.cursor, 1);
    let service_at = run.eligible_at;
    let (location, _) = Actors::actor_control_cell(actor_id).expect("Running primary");
    if matches!(location, crate::ActorControlLocation::Ready { .. }) {
      let cell =
        Actors::remove_primary_control_cell_inner(actor_id).expect("consume Ready placement");
      assert_ok!(Actors::control_append_waiting(
        cell,
        WakeupKey::Block(service_at),
        crate::scheduler::ActorWaitingAuthority::Service,
      ));
    }
    let trigger_pointer = Actors::actor_hot(actor_id)
      .and_then(|hot| hot.trigger_wakeup_pointer)
      .expect("Running cadence is rearmed");
    let hot = Actors::actor_hot(actor_id).expect("Actor owns both temporal memberships");
    assert_eq!(hot.cycle_state, CycleState::Running);
    assert!(!hot.pending_signal);
    assert!(hot.wakeup_pointer.is_some());
    assert_eq!(hot.trigger_wakeup_pointer, Some(trigger_pointer));
    assert!(matches!(
      crate::ActorWaitingFrameChunks::<Test>::get((
        WakeupKey::Tick(trigger_pointer.tick),
        trigger_pointer.page_id
      ))
      .expect("independent Trigger page")
      .entries[trigger_pointer.slot as usize],
      Some(crate::ActorWaitingEntry::Reference(_))
    ));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());

    let (mut ready, stats) = Actors::wakeup_substrate_drain_key(WakeupKey::Block(service_at), 1);
    assert_eq!(
      ready.iter().map(|entry| entry.0).collect::<Vec<_>>(),
      vec![actor_id]
    );
    assert_eq!(stats.ready_entries, 1);
    let (id, state, admission, loaded_step) = ready.pop().expect("consumed service authority");
    let loaded_step = loaded_step.expect("Running Step");
    let cell = crate::ActorControlCellOf::<Test> {
      actor_id: id,
      identity: Actors::control_identity_from_scalar(state.identity).expect("canonical identity"),
      hot: Actors::control_hot_from_scalar(state.hot),
      pipeline_service_identity: crate::pipeline_service_identity(admission.admission_identity),
      admission,
      cursor: loaded_step.cursor,
      resources: loaded_step.resources,
      eligible_at: Some(service_at),
    };
    assert_ok!(Actors::control_append_ready(cell));
    let hot = Actors::actor_hot(actor_id).expect("Actor remains active");
    assert!(hot.wakeup_pointer.is_none());
    assert_eq!(hot.trigger_wakeup_pointer, Some(trigger_pointer));
    #[cfg(feature = "try-runtime")]
    {
      assert_ok!(crate::Pallet::<Test>::do_try_state());
    }
  });
}

#[test]
fn timer_wakeup_uses_exact_cadence_without_actor_phase() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let cadence = 20u32;
    let actor_id = create_system_with(ALICE, timer_schedule(cadence), None, inert_contract_steps());
    assert_eq!(scheduled_wakeup_block(actor_id), Some(21));

    frame_system::Pallet::<Test>::set_block_number(21);
    run_idle(Weight::MAX);
    assert_eq!(scheduled_wakeup_block(actor_id), Some(41));
  });
}

