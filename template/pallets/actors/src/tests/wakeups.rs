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
fn paged_wakeup_substrate_replaces_and_invalidates_exact_slots() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert!(schedule_latched_service_wakeup(actor_id, 10));
    let first = Actors::actor_hot(actor_id)
      .expect("hot state")
      .wakeup_pointer
      .expect("first wakeup pointer");
    assert_eq!(
      (first.block, first.page_id, first.slot),
      (WakeupKey::Block(10), 0, 0)
    );
    assert_eq!(
      Actors::wakeup_buckets(10)
        .expect("first bucket")
        .live_entries,
      1
    );
    assert_eq!(Actors::wakeup_cursor_len(), 1);
    assert_eq!(Actors::wakeup_cursor_peek(), Some(10));

    assert!(Actors::wakeup_substrate_schedule(actor_id, 20));
    let replacement = Actors::actor_hot(actor_id)
      .expect("hot state")
      .wakeup_pointer
      .expect("replacement wakeup pointer");
    assert_eq!(
      (replacement.block, replacement.page_id, replacement.slot),
      (WakeupKey::Block(20), 0, 0)
    );
    assert!(Actors::wakeup_buckets(10).is_none());
    assert!(Actors::wakeup_pages((10, 0)).is_none());
    assert_eq!(Actors::wakeup_cursor_len(), 1);
    assert_eq!(Actors::wakeup_cursor_peek(), Some(20));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());

    let (_, mut consumed) = Actors::actor_control_cell(actor_id).expect("Waiting authority");
    assert_eq!(
      Actors::wakeup_substrate_invalidate(actor_id),
      Some(replacement)
    );
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    consumed.hot.pending_signal = false;
    consumed.hot.wakeup_pointer = None;
    consumed.eligible_at = None;
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, consumed);
    crate::ActorControlLocators::<Test>::insert(actor_id, crate::ActorControlLocation::Unsignaled);
    assert!(
      Actors::actor_hot(actor_id)
        .expect("hot state")
        .wakeup_pointer
        .is_none()
    );
    assert!(Actors::wakeup_buckets(20).is_none());
    assert!(Actors::wakeup_pages((20, 0)).is_none());
    assert_eq!(Actors::wakeup_cursor_len(), 0);
    assert_eq!(Actors::wakeup_cursor_peek(), None);
  });
}

#[test]
fn wakeup_replacement_rolls_back_when_existing_cursor_is_corrupt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(schedule_latched_service_wakeup(actor_id, 10));
    crate::ActorWaitingCursorIndices::<Test>::insert(WakeupKey::Block(10), 1);
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    assert_eq!(
      Actors::try_wakeup_substrate_schedule_inner(actor_id, 20),
      Err(crate::EnqueueOutcome::CorruptedTopology),
    );

    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });
}

#[test]
fn wakeup_pointer_corruption_matrix_fails_closed_and_is_detected() {
  for corruption in 0u8..8 {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      let other = create_system_with(BOB, manual_schedule(), None, inert_contract_steps());
      assert!(schedule_latched_service_wakeup(actor_id, 10));
      match corruption {
        0 => mutate_primary_control_cell(actor_id, |cell| {
          cell
            .hot
            .wakeup_pointer
            .as_mut()
            .expect("wakeup pointer")
            .block = WakeupKey::Block(11);
        }),
        1 => mutate_primary_control_cell(actor_id, |cell| {
          cell
            .hot
            .wakeup_pointer
            .as_mut()
            .expect("wakeup pointer")
            .page_id = 1;
        }),
        2 => mutate_primary_control_cell(actor_id, |cell| {
          cell
            .hot
            .wakeup_pointer
            .as_mut()
            .expect("wakeup pointer")
            .slot = 7;
        }),
        3 => mutate_primary_control_cell(actor_id, |cell| cell.actor_id = other),
        4 => crate::ActorWaitingFrameChunks::<Test>::remove((WakeupKey::Block(10), 0)),
        5 => crate::ActorWaitingTails::<Test>::remove(WakeupKey::Block(10)),
        6 => crate::ActorWaitingCursorIndices::<Test>::remove(WakeupKey::Block(10)),
        7 => crate::ActorWaitingOccupancies::<Test>::insert(WakeupKey::Block(10), 0),
        _ => unreachable!(),
      }
      let events_before = System::events();
      let identity_before = Actors::actor_identity(actor_id);
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

      assert_eq!(
        Actors::try_wakeup_substrate_schedule_inner(actor_id, 20),
        Err(crate::EnqueueOutcome::CorruptedTopology),
        "replacement case {corruption}"
      );
      assert_eq!(Actors::wakeup_substrate_invalidate(actor_id), None);
      assert_eq!(System::events(), events_before);
      assert_eq!(Actors::actor_identity(actor_id), identity_before);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        root_before,
        "invalidation case {corruption}"
      );
      #[cfg(feature = "try-runtime")]
      assert!(
        crate::Pallet::<Test>::do_try_state().is_err(),
        "try-state case {corruption}"
      );
    });
  }
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
fn wakeup_replacement_rolls_back_when_existing_page_or_slot_is_missing() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(schedule_latched_service_wakeup(actor_id, 10));
    crate::ActorWaitingFrameChunks::<Test>::remove((WakeupKey::Block(10), 0));
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_eq!(
      Actors::try_wakeup_substrate_schedule_inner(actor_id, 20),
      Err(crate::EnqueueOutcome::CorruptedTopology),
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(schedule_latched_service_wakeup(actor_id, 10));
    mutate_primary_control_cell(actor_id, |cell| {
      cell.hot.wakeup_pointer.as_mut().expect("pointer").slot = 7;
    });
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_eq!(
      Actors::try_wakeup_substrate_schedule_inner(actor_id, 20),
      Err(crate::EnqueueOutcome::CorruptedTopology),
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });
}

#[test]
fn wakeup_replacement_rolls_back_on_live_count_underflow() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(schedule_latched_service_wakeup(actor_id, 10));
    crate::ActorWaitingOccupancies::<Test>::insert(WakeupKey::Block(10), 0);
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    assert_eq!(
      Actors::try_wakeup_substrate_schedule_inner(actor_id, 20),
      Err(crate::EnqueueOutcome::CorruptedTopology),
    );

    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });
}

#[test]
fn wakeup_cursor_capacity_overflow_fails_closed_and_preserves_existing_path() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert!(schedule_latched_service_wakeup(actor_id, 10));
    let pointer = Actors::actor_hot(actor_id)
      .expect("hot state")
      .wakeup_pointer
      .expect("existing path");
    assert_eq!(
      (pointer.block, pointer.page_id, pointer.slot),
      (WakeupKey::Block(10), 0, 0)
    );

    // Saturate the wakeup cursor heap at its capacity bound; the worker's index insert fails.
    crate::WakeupCursorLen::<Test>::insert(
      WakeupClock::Block,
      <<Test as crate::Config>::MaxActiveActors as Get<u32>>::get(),
    );
    assert!(
      !Actors::wakeup_substrate_schedule(actor_id, 20),
      "cursor capacity overflow must fail closed"
    );
    // The transactional wrapper rolls back the attempted replacement; the actor keeps its exact
    // existing pointer and the original bucket/cursor entries stay intact.
    assert_eq!(
      Actors::actor_hot(actor_id)
        .expect("hot state")
        .wakeup_pointer,
      Some(pointer)
    );
    assert!(
      Actors::wakeup_buckets(10).is_some(),
      "original bucket survives"
    );
    assert!(
      Actors::wakeup_pages((10, 0)).is_some(),
      "original page survives"
    );
    assert!(
      Actors::wakeup_buckets(20).is_none(),
      "no partial replacement bucket"
    );
    assert_eq!(Actors::wakeup_cursor_peek(), Some(10));
    #[cfg(feature = "try-runtime")]
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn wakeup_page_index_overflow_fails_closed_as_namespace_exhaustion() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    // Seed a Waiting bucket whose tail position is at the u64 ceiling; appending
    // cannot advance the monotonic position and must fail closed as
    // WakeupIndexExhausted (mapped to the SchedulerIndexExhausted public error).
    let block = 10u64;
    assert!(schedule_latched_service_wakeup(actor_id, block));
    // Fill the tail page, then set the next Waiting position at the u64 ceiling
    // so the monotonic address cannot advance.
    let page_size = 32u32;
    for _ in 0..page_size.saturating_sub(1) {
      let extra = create_system_with(
        BOB,
        manual_schedule(),
        None,
        transfer_contract_steps(CHARLIE, 1),
      );
      assert!(schedule_latched_service_wakeup(extra, block));
    }
    crate::ActorWaitingTails::<Test>::insert(WakeupKey::Block(block), u64::MAX);
    let pointer_before = Actors::actor_hot(actor_id)
      .expect("hot")
      .wakeup_pointer
      .expect("existing pointer");
    // A different actor scheduling into the full bucket with the tail position at the
    // u64 ceiling cannot advance the monotonic index and fails closed.
    let new_actor = create_system_with(
      BOB,
      manual_schedule(),
      None,
      transfer_contract_steps(CHARLIE, 1),
    );
    assert!(matches!(
      try_schedule_latched_service_wakeup(new_actor, block),
      Err(crate::EnqueueOutcome::WakeupIndexExhausted)
    ));
    // The existing pointer is never cleared before the replacement path fits.
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot").wakeup_pointer,
      Some(pointer_before)
    );
  });
}

#[test]
fn wakeup_bucket_corruption_fails_closed_as_corrupted_topology() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let block = 10u64;
    assert!(schedule_latched_service_wakeup(actor_id, block));
    // Corrupt the TARGET bucket for a different actor: a bucket exists at block + 1 but
    // its cursor_index is missing, so the schedule path must fail closed as corrupted
    // topology instead of retrying as queue-full.
    let target = block + 1;
    crate::ActorWaitingOccupancies::<Test>::insert(WakeupKey::Block(target), 1);
    let pointer_before = Actors::actor_hot(actor_id)
      .expect("hot")
      .wakeup_pointer
      .expect("existing pointer");
    let new_actor = create_system_with(
      BOB,
      manual_schedule(),
      None,
      transfer_contract_steps(CHARLIE, 1),
    );
    assert!(matches!(
      try_schedule_latched_service_wakeup(new_actor, target),
      Err(crate::EnqueueOutcome::CorruptedTopology)
    ));
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot").wakeup_pointer,
      Some(pointer_before)
    );
  });
}

#[test]
fn saturated_enqueue_falls_back_to_exact_next_block_wakeup_not_silent_loss() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    // Saturate the canonical FIFO so ticket placement is impossible.
    seed_saturated_tombstone_queue();
    // Placement must not silently lose readiness: the fallback is an exact
    // next-block wakeup, so the actor owns a wakeup path even though the FIFO
    // rejected the ticket (spec 8.1.4).
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let hot = Actors::actor_hot(actor_id).expect("hot");
    assert!(
      hot.queue_ticket.is_none(),
      "saturated FIFO grants no ticket"
    );
    assert_eq!(
      hot.wakeup_pointer.expect("readiness wakeup").block,
      WakeupKey::Block(2),
      "exact next-block wakeup preserves readiness"
    );
    assert_eq!(Actors::wakeup_buckets(2).expect("bucket").live_entries, 1);
  });
}

#[test]
fn saturated_enqueue_fails_closed_when_wakeup_fallback_also_fails() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    // Saturate the FIFO so enqueue falls back to the next-block wakeup, then
    // corrupt the target wakeup bucket so that fallback placement also fails.
    seed_saturated_tombstone_queue();
    crate::ActorWaitingOccupancies::<Test>::insert(WakeupKey::Block(2), 1);
    // The placement must fail closed: the caller learns the actor owns no path
    // instead of believing enqueue succeeded while readiness was lost.
    // The public placement boundary maps CorruptedTopology to SchedulerIndexExhausted.
    assert_noop!(
      Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::SchedulerIndexExhausted
    );
    let hot = Actors::actor_hot(actor_id).expect("hot");
    assert!(hot.queue_ticket.is_none());
    assert!(hot.wakeup_pointer.is_none(), "no phantom wakeup on failure");
    assert!(Actors::wakeup_cursor_peek().is_none(), "no cursor residue");
  });
}

#[test]
fn manual_trigger_fails_closed_when_wakeup_fallback_cannot_preserve_readiness() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    // Saturate the FIFO so any ticket placement falls back to the next-block
    // wakeup, then corrupt that target bucket so the fallback also fails.
    seed_saturated_tombstone_queue();
    crate::ActorWaitingOccupancies::<Test>::insert(WakeupKey::Block(2), 1);
    // The extrinsic must fail closed (namespace/corruption is not retryable
    // queue-full), leaving the actor with its pre-trigger state: no ticket and
    // no wakeup, and the pending-signal mutation rolled back.
    // FRAME dispatch wraps the call transactionally, so the pending-signal
    // mutation rolls back together with the failed placement.
    let before_signal = Actors::actor_hot(actor_id).expect("hot").pending_signal;
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let result = Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id);
    let after_signal = Actors::actor_hot(actor_id).expect("hot").pending_signal;
    assert!(
      result.is_err(),
      "manual_trigger must fail closed: {result:?}"
    );
    assert_eq!(
      (before_signal, after_signal),
      (false, false),
      "signal mutation must roll back with the failed placement"
    );
    let hot = Actors::actor_hot(actor_id).expect("hot");
    assert!(hot.queue_ticket.is_none());
    assert!(hot.wakeup_pointer.is_none());
    assert!(
      !hot.pending_signal,
      "signal mutation rolled back with the call"
    );
    assert_eq!(System::events(), events_before, "trigger event rolls back");
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn paged_wakeup_substrate_invalidation_rolls_back_on_cursor_mismatch() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert!(schedule_latched_service_wakeup(actor_id, 10));
    let pointer = Actors::actor_hot(actor_id)
      .expect("hot state")
      .wakeup_pointer
      .expect("wakeup pointer");
    crate::ActorWaitingCursorIndices::<Test>::remove(WakeupKey::Block(10));

    assert_eq!(Actors::wakeup_substrate_invalidate(actor_id), None);
    assert_eq!(
      Actors::actor_hot(actor_id)
        .expect("hot state")
        .wakeup_pointer,
      Some(pointer)
    );
    assert!(Actors::wakeup_page_entry_matches(pointer, actor_id));
    assert_eq!(
      Actors::wakeup_buckets(10)
        .expect("wakeup bucket")
        .live_entries,
      1
    );
    assert_eq!(Actors::wakeup_cursor_len(), 1);
    assert_eq!(Actors::wakeup_cursor_peek(), Some(10));
  });
}

#[test]
fn paged_wakeup_substrate_links_and_unlinks_middle_pages() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size = 32u32;
    let count = page_size.saturating_mul(2).saturating_add(1);
    let mut actors = Vec::new();
    for _ in 0..count {
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      assert!(schedule_latched_service_wakeup(actor_id, 10));
      actors.push(actor_id);
    }

    let bucket = Actors::wakeup_buckets(10).expect("dense bucket");
    assert_eq!(bucket.live_entries, count);
    assert_eq!(
      (bucket.head_page, bucket.tail_page, bucket.next_page_id),
      (0, 2, 3)
    );
    assert_eq!(
      Actors::wakeup_pages((10, 0)).expect("head page").next_page,
      Some(1)
    );
    assert_eq!(
      Actors::wakeup_pages((10, 1))
        .expect("middle page")
        .next_page,
      Some(2)
    );
    assert_eq!(
      Actors::wakeup_pages((10, 2))
        .expect("tail page")
        .previous_page,
      Some(1)
    );

    let page_size = page_size as usize;
    let invalidate_service = |actor_id| {
      let (_, mut consumed) = Actors::actor_control_cell(actor_id).expect("Waiting authority");
      assert!(Actors::wakeup_substrate_invalidate(actor_id).is_some());
      assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
      consumed.hot.pending_signal = false;
      consumed.hot.wakeup_pointer = None;
      consumed.eligible_at = None;
      crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, consumed);
      crate::ActorControlLocators::<Test>::insert(
        actor_id,
        crate::ActorControlLocation::Unsignaled,
      );
    };
    for actor_id in &actors[page_size..page_size * 2] {
      invalidate_service(*actor_id);
    }
    let bucket = Actors::wakeup_buckets(10).expect("bucket after middle unlink");
    assert_eq!(bucket.live_entries, count.saturating_sub(page_size as u32));
    assert!(Actors::wakeup_pages((10, 1)).is_none());
    assert_eq!(
      Actors::wakeup_pages((10, 0)).expect("head page").next_page,
      Some(2)
    );
    assert_eq!(
      Actors::wakeup_pages((10, 2))
        .expect("tail page")
        .previous_page,
      Some(0)
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());

    for (index, actor_id) in actors.into_iter().enumerate() {
      if !(page_size..page_size * 2).contains(&index) {
        invalidate_service(actor_id);
      }
    }
    assert!(Actors::wakeup_buckets(10).is_none());
    assert!(Actors::wakeup_pages((10, 0)).is_none());
    assert!(Actors::wakeup_pages((10, 2)).is_none());
  });
}

#[test]
fn paged_wakeup_drain_preserves_partial_progress_and_crosses_page_boundaries() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size = 32u32;
    let count = page_size.saturating_mul(2).saturating_add(1);
    let mut actors = Vec::new();
    for _ in 0..count {
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      assert!(schedule_latched_service_wakeup(actor_id, 10));
      actors.push(actor_id);
    }

    let first_limit = page_size / 2;
    let (first, first_stats) =
      Actors::wakeup_substrate_drain_key(WakeupKey::Block(10), first_limit);
    assert_eq!(
      first.iter().map(|entry| entry.0).collect::<Vec<_>>(),
      actors[..first_limit as usize]
    );
    assert_eq!(first_stats.entries_scanned, first_limit);
    assert_eq!(first_stats.ready_entries, first_limit);
    assert_eq!(first_stats.pages_touched, 1);
    assert_eq!(first_stats.pages_deleted, 0);
    let head = Actors::wakeup_pages((10, 0)).expect("partially drained head");
    assert_eq!(head.scan_slot, first_limit);
    assert_eq!(head.live_entries, page_size - first_limit);

    let (second, second_stats) =
      Actors::wakeup_substrate_drain_key(WakeupKey::Block(10), page_size);
    let second_end = first_limit.saturating_add(page_size) as usize;
    assert_eq!(
      second.iter().map(|entry| entry.0).collect::<Vec<_>>(),
      actors[first_limit as usize..second_end]
    );
    assert_eq!(second_stats.entries_scanned, page_size);
    assert_eq!(second_stats.ready_entries, page_size);
    assert_eq!(second_stats.pages_touched, 2);
    assert_eq!(second_stats.pages_deleted, 1);
    let bucket = Actors::wakeup_buckets(10).expect("remaining wakeup bucket");
    assert_eq!(bucket.head_page, 1);
    let head = Actors::wakeup_pages((10, 1)).expect("second partial head");
    assert_eq!(head.previous_page, None);
    assert_eq!(head.scan_slot, first_limit);

    let (final_ready, final_stats) =
      Actors::wakeup_substrate_drain_key(WakeupKey::Block(10), u32::MAX);
    assert_eq!(
      final_ready.iter().map(|entry| entry.0).collect::<Vec<_>>(),
      actors[second_end..]
    );
    assert_eq!(final_stats.ready_entries, count - first_limit - page_size);
    assert_eq!(final_stats.pages_touched, 2);
    assert_eq!(final_stats.pages_deleted, 2);
    for (actor_id, state, admission, loaded_step) in
      first.into_iter().chain(second).chain(final_ready)
    {
      let loaded_step = loaded_step.expect("nonempty service Step");
      let cell = crate::ActorControlCellOf::<Test> {
        actor_id,
        identity: Actors::control_identity_from_scalar(state.identity).expect("canonical identity"),
        hot: Actors::control_hot_from_scalar(state.hot),
        admission,
        cursor: loaded_step.cursor,
        resources: loaded_step.resources,
        eligible_at: Some(10),
      };
      assert_ok!(Actors::control_append_ready(cell));
    }
    assert!(Actors::wakeup_buckets(10).is_none());
    assert!(Actors::wakeup_pages((10, 1)).is_none());
    assert!(Actors::wakeup_pages((10, 2)).is_none());
    assert_eq!(Actors::wakeup_cursor_len(), 0);
    assert_eq!(Actors::wakeup_cursor_peek(), None);
    assert!(actors.iter().all(|actor_id| {
      Actors::actor_hot(*actor_id)
        .expect("hot state")
        .wakeup_pointer
        .is_none()
    }));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn paged_wakeup_drain_discards_explicit_orphan_references() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut actors = Vec::new();
    let mut stale_cells = Vec::new();
    for _ in 0..3 {
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      assert!(schedule_latched_service_wakeup(actor_id, 10));
      let (location, mut cell) = Actors::actor_control_cell(actor_id).expect("scheduled primary");
      // Deliberate orphan corruption: real lifecycle removal deletes the entry as well.
      let crate::ActorControlLocation::Waiting { key, page, slot } = location else {
        panic!("scheduled primary occupies Waiting");
      };
      crate::ActorWaitingFrameChunks::<Test>::mutate((key, page), |stored| {
        stored.as_mut().expect("Waiting page").entries[slot as usize] = Some(
          crate::ActorWaitingEntry::Reference(crate::ActorWakeupReference {
            actor_id,
            admission_identity: cell.admission.admission_identity,
          }),
        );
      });
      crate::ActorControlLocators::<Test>::remove(actor_id);
      cell.hot.pending_signal = false;
      cell.hot.wakeup_pointer = None;
      cell.eligible_at = None;
      stale_cells.push((actor_id, cell));
      actors.push(actor_id);
    }

    let (ready, stats) = Actors::wakeup_substrate_drain_block(10, 3);
    assert!(ready.is_empty());
    assert_eq!(stats.entries_scanned, 3);
    assert_eq!(stats.ready_entries, 0);
    assert_eq!(stats.stale_entries, 3);
    assert_eq!(stats.pages_touched, 1);
    assert_eq!(stats.pages_deleted, 1);
    assert!(Actors::wakeup_buckets(10).is_none());
    assert!(Actors::wakeup_pages((10, 0)).is_none());
    assert!(
      actors
        .iter()
        .all(|actor_id| Actors::actor_hot(*actor_id).is_none())
    );
    for (actor_id, cell) in stale_cells {
      crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell);
      crate::ActorControlLocators::<Test>::insert(
        actor_id,
        crate::ActorControlLocation::Unsignaled,
      );
    }
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn cursor_wakeup_drain_recovers_sparse_overdue_blocks_without_scanning_gaps() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let due = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let future = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert!(schedule_latched_service_wakeup(due, 10));
    assert!(schedule_latched_service_wakeup(future, 1_000_000));

    let mut halted = WeightMeter::with_limit(Weight::zero());
    assert_eq!(
      Actors::drain_overdue_wakeups_cursor(100, &mut halted).entries_scanned,
      0
    );
    assert!(
      Actors::actor_hot(due)
        .expect("due actor")
        .queue_ticket
        .is_none()
    );

    let mut ample = WeightMeter::with_limit(Weight::from_parts(u64::MAX, u64::MAX));
    let stats = Actors::drain_overdue_wakeups_cursor(100, &mut ample);
    assert_eq!(stats.entries_scanned, 1);
    assert_eq!(stats.ready_entries, 1);
    assert!(
      Actors::actor_hot(due)
        .expect("due actor")
        .queue_ticket
        .is_some()
    );
    assert_eq!(Actors::wakeup_cursor_len(), 1);
    assert_eq!(Actors::wakeup_cursor_peek(), Some(1_000_000));
    assert!(
      Actors::actor_hot(future)
        .expect("future actor")
        .wakeup_pointer
        .is_some()
    );
  });
}

#[test]
fn cursor_wakeup_drain_halts_and_resumes_between_slot_units() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut actors = Vec::new();
    for _ in 0..3 {
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      assert!(schedule_latched_service_wakeup(actor_id, 10));
      actors.push(actor_id);
    }
    let limit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_wakeup_cursor_worker_future()
        .saturating_add(Actors::block_wakeup_cursor_drain_unit_weight_upper(
          crate::scheduler::WakeupBucketDisposition::Retain,
        ));
    let mut one_slot = WeightMeter::with_limit(limit);
    let first = Actors::drain_overdue_wakeups_cursor(10, &mut one_slot);
    assert_eq!(first.entries_scanned, 1);
    assert_eq!(first.ready_entries, 1);
    assert_eq!(
      Actors::wakeup_buckets(10)
        .expect("partial bucket")
        .live_entries,
      2
    );
    assert_eq!(Actors::wakeup_cursor_peek(), Some(10));

    let mut resume = WeightMeter::with_limit(Weight::from_parts(u64::MAX, u64::MAX));
    let second = Actors::drain_overdue_wakeups_cursor(10, &mut resume);
    assert_eq!(second.entries_scanned, 2);
    assert_eq!(second.ready_entries, 2);
    assert!(Actors::wakeup_buckets(10).is_none());
    assert_eq!(Actors::wakeup_cursor_len(), 0);
    assert!(actors.iter().all(|actor_id| {
      Actors::actor_hot(*actor_id)
        .expect("actor")
        .queue_ticket
        .is_some()
    }));
  });
}

#[test]
fn cursor_wakeup_drain_stops_independently_on_reftime_and_proof_size() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert!(schedule_latched_service_wakeup(actor_id, 10));
    let required =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_wakeup_cursor_worker_future()
        .saturating_add(Actors::block_wakeup_cursor_drain_unit_weight_upper(
          crate::scheduler::WakeupBucketDisposition::Remove,
        ));

    let mut reftime_short = WeightMeter::with_limit(Weight::from_parts(
      required.ref_time().saturating_sub(1),
      u64::MAX,
    ));
    assert_eq!(
      Actors::drain_overdue_wakeups_cursor(10, &mut reftime_short).entries_scanned,
      0
    );
    assert!(Actors::actor_hot(actor_id)
      .expect("actor after RefTime stop")
      .wakeup_pointer
      .is_some());

    let mut proof_short = WeightMeter::with_limit(Weight::from_parts(
      u64::MAX,
      required.proof_size().saturating_sub(1),
    ));
    assert_eq!(
      Actors::drain_overdue_wakeups_cursor(10, &mut proof_short).entries_scanned,
      0
    );
    assert!(Actors::actor_hot(actor_id)
      .expect("actor after ProofSize stop")
      .wakeup_pointer
      .is_some());
  });
}

#[test]
fn on_idle_wakeup_worker_respects_remaining_weight_in_each_dimension() {
  let required =
    <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_wakeup_cursor_worker_future()
      .saturating_add(Actors::block_wakeup_cursor_drain_unit_weight_upper(
        crate::scheduler::WakeupBucketDisposition::Remove,
      ));
  assert_on_idle_wakeup_insufficiency_preserves_state(Weight::from_parts(
    required.ref_time().saturating_sub(1),
    u64::MAX,
  ));
  assert_on_idle_wakeup_insufficiency_preserves_state(Weight::from_parts(
    u64::MAX,
    required.proof_size().saturating_sub(1),
  ));
  assert_on_idle_wakeup_insufficiency_preserves_state(Weight::from_parts(
    required.ref_time().saturating_sub(1),
    required.proof_size().saturating_sub(1),
  ));
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
fn wakeup_materialization_closes_when_queue_ticket_is_exhausted() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert!(schedule_latched_service_wakeup(actor_id, 10));
    crate::ActorReadyHead::<Test>::put(u64::MAX);
    crate::ActorReadyTail::<Test>::put(u64::MAX);
    System::reset_events();
    let mut meter = WeightMeter::with_limit(Weight::MAX);

    let stats = Actors::drain_overdue_wakeups_cursor(10, &mut meter);

    assert_eq!(stats.entries_scanned, 1);
    assert!(Actors::actor_hot(actor_id).is_none());
    assert!(Actors::wakeup_buckets(10).is_none());
    assert!(Actors::wakeup_pages((10, 0)).is_none());
    assert_eq!(Actors::wakeup_cursor_peek(), None);
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
fn wakeup_materialization_faults_and_requires_repair_on_fifo_topology_corruption() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(schedule_latched_service_wakeup(actor_id, 10));
    ActorReadyTail::<Test>::put(1);
    ActorReadyOccupancy::<Test>::put(0);
    let events_before = System::events().len();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let mut meter = WeightMeter::with_limit(Weight::MAX);

    let stats = Actors::drain_overdue_wakeups_cursor(10, &mut meter);

    assert_eq!(stats.entries_scanned, 0);
    assert_eq!(System::events().len(), events_before + 1);
    assert_ne!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
    assert_eq!(scheduled_wakeup_block(actor_id), Some(10));
    let fault = crate::WakeupWorkerFault {
      key: WakeupKey::Block(10),
      page: 0,
      class: crate::CrossingWorkerFaultClass::Invariant,
    };
    assert_eq!(Actors::wakeup_worker_fault(), Some(fault));
    assert_eq!(
      actor_event_count(|event| matches!(
        event,
        Event::ActorFaultRecorded {
          fault_id: crate::FaultId::WakeupWorker,
          kind: crate::ActorFaultKind::Wakeup,
          first_recorded_block: 1,
          context: crate::FaultContext::Wakeup(recorded),
        } if recorded == &fault
      )),
      1
    );
    let mut halted = WeightMeter::with_limit(Weight::MAX);
    assert_eq!(
      Actors::drain_overdue_wakeups_cursor(10, &mut halted).entries_scanned,
      0
    );
    assert_eq!(
      actor_event_count(|event| matches!(
        event,
        Event::ActorFaultRecorded {
          fault_id: crate::FaultId::WakeupWorker,
          ..
        }
      )),
      1,
      "an uncleared wakeup fault emits only its first-recorded event"
    );
    assert_noop!(
      Actors::clear_wakeup_worker_fault(RuntimeOrigin::signed(ALICE)),
      DispatchError::BadOrigin
    );
    ActorReadyTail::<Test>::put(0);
    assert_ok!(Actors::clear_wakeup_worker_fault(RuntimeOrigin::root()));
    let actor_events: Vec<_> = System::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    let recorded_index = actor_events
      .iter()
      .position(|event| matches!(event, Event::ActorFaultRecorded { .. }))
      .expect("first-recorded event exists");
    let cleared_index = actor_events
      .iter()
      .position(|event| matches!(event, Event::WakeupWorkerFaultCleared { .. }))
      .expect("clear event exists");
    assert!(recorded_index < cleared_index);
    let mut repaired = WeightMeter::with_limit(Weight::MAX);
    assert_eq!(
      Actors::drain_overdue_wakeups_cursor(10, &mut repaired).ready_entries,
      1
    );
    assert!(Actors::wakeup_worker_fault().is_none());
    assert_eq!(scheduled_wakeup_block(actor_id), None);
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
fn waiting_removal_repairs_upward_and_reclaims_heap_tail_page() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let size: u32 = <Test as crate::Config>::WakeupPageSize::get();
    assert_eq!(
      size, 32,
      "fixture straddles the reference heap-page boundary"
    );
    let count = size * 2 + 1;
    let mut actors = Vec::new();
    let mut blocks = Vec::new();
    for index in 0..count {
      let mut branch = index;
      while branch > 2 {
        branch = (branch - 1) / 2;
      }
      let block = if index == 0 {
        10_000
      } else if branch == 1 {
        100_000
      } else {
        1_000_000
      } + u64::from(index);
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      assert!(schedule_latched_service_wakeup(actor_id, block));
      assert_eq!(
        crate::ActorWaitingCursorIndices::<Test>::get(WakeupKey::Block(block)),
        Some(index)
      );
      actors.push(actor_id);
      blocks.push(block);
    }
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());

    // The final key belongs to the low left subtree. Replacing a deep right-subtree key
    // forces four upward swaps across a page boundary, then deletes the one-entry tail page.
    assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), actors[62]));
    assert!(!Actors::active_actor_exists(actors[62]));
    assert_eq!(Actors::wakeup_cursor_len(), 64);
    assert!(Actors::wakeup_cursor_pages(2).is_none());
    assert!(!crate::ActorWaitingCursorIndices::<Test>::contains_key(
      WakeupKey::Block(blocks[62])
    ));
    assert_eq!(
      crate::ActorWaitingCursorIndices::<Test>::get(WakeupKey::Block(blocks[64])),
      Some(2)
    );
    for (from, to) in [(2, 6), (6, 14), (14, 30), (30, 62)] {
      assert_eq!(
        crate::ActorWaitingCursorIndices::<Test>::get(WakeupKey::Block(blocks[from])),
        Some(to)
      );
    }
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());

    // Removing the current last key needs no repair; it truncates the retained page.
    assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), actors[63]));
    assert!(!Actors::active_actor_exists(actors[63]));
    assert_eq!(Actors::wakeup_cursor_len(), 63);
    assert_eq!(
      Actors::wakeup_cursor_pages(1).map(|page| page.len()),
      Some(31)
    );
    assert!(!crate::ActorWaitingCursorIndices::<Test>::contains_key(
      WakeupKey::Block(blocks[63])
    ));
    assert_eq!(Actors::wakeup_cursor_peek(), Some(blocks[0]));
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn paged_wakeup_cursor_orders_sparse_blocks_across_page_boundaries() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size: u32 = <Test as crate::Config>::WakeupPageSize::get();
    let count = page_size.saturating_add(3);
    let mut actors = Vec::new();
    let blocks: Vec<MockBlockNumber> = (0..count)
      .map(|index| 10_000u64.saturating_add(u64::from(index).saturating_mul(10_000)))
      .collect();

    for block in blocks.iter().rev().copied() {
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      assert!(schedule_latched_service_wakeup(actor_id, block));
      assert!(Actors::wakeup_cursor_insert(block));
      actors.push(actor_id);
    }

    assert_eq!(Actors::wakeup_cursor_len(), count);
    assert_eq!(Actors::wakeup_cursor_peek(), blocks.first().copied());
    assert_eq!(
      Actors::wakeup_cursor_pages(0).map(|page| page.len()),
      Some(page_size as usize)
    );
    assert_eq!(
      Actors::wakeup_cursor_pages(1).map(|page| page.len()),
      Some(3)
    );
    assert!(Actors::wakeup_cursor_insert(blocks[0]));
    assert_eq!(Actors::wakeup_cursor_len(), count);

    let mut consumed = Vec::new();
    for actor_id in actors {
      let (location, cell) = Actors::actor_control_cell(actor_id).expect("live Waiting owner");
      let crate::ActorControlLocation::Waiting { key, page, .. } = location else {
        panic!("service primary is Waiting");
      };
      // Model the atomic boundary after each sole entry is consumed, before heap repair.
      crate::ActorWaitingFrameChunks::<Test>::remove((key, page));
      crate::ActorWaitingHeads::<Test>::remove(key);
      crate::ActorWaitingTails::<Test>::remove(key);
      crate::ActorWaitingOccupancies::<Test>::remove(key);
      crate::ActorControlLocators::<Test>::remove(actor_id);
      consumed.push(cell);
    }
    let removed = blocks[(count / 2) as usize];
    assert!(Actors::wakeup_cursor_remove(removed));
    assert!(!Actors::wakeup_cursor_remove(removed));
    assert_eq!(Actors::wakeup_cursor_len(), count.saturating_sub(1));
    assert_eq!(
      crate::ActorWaitingCursorIndices::<Test>::get(WakeupKey::Block(removed)),
      None
    );
    let expected: Vec<_> = blocks
      .iter()
      .copied()
      .filter(|block| *block != removed)
      .collect();
    let mut popped = Vec::new();
    while let Some(block) = Actors::wakeup_cursor_pop_min() {
      popped.push(block);
    }
    assert_eq!(popped, expected);
    assert_eq!(Actors::wakeup_cursor_len(), 0);
    assert!(Actors::wakeup_cursor_pages(0).is_none());
    assert!(Actors::wakeup_cursor_pages(1).is_none());
    assert!(blocks.iter().all(|block| {
      !crate::ActorWaitingCursorIndices::<Test>::contains_key(WakeupKey::Block(*block))
    }));

    for mut cell in consumed {
      cell.hot.pending_signal = false;
      cell.hot.wakeup_pointer = None;
      cell.eligible_at = None;
      crate::ActorControlLocators::<Test>::insert(
        cell.actor_id,
        crate::ActorControlLocation::Unsignaled,
      );
      crate::ActorUnsignaledControlCells::<Test>::insert(cell.actor_id, cell);
    }
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
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
    assert!(Actors::actor_funding(actor_id).is_none());
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
fn trigger_rearm_index_exhaustion_closes_before_the_due_attempt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      timer_schedule(1),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 100);
    let page_size = 32u32;
    for _ in 0..page_size {
      create_system_with(ALICE, timer_schedule(2), None, inert_contract_steps());
    }
    crate::ActorWaitingTails::<Test>::insert(WakeupKey::Tick(3), u64::MAX);
    frame_system::Pallet::<Test>::set_block_number(2);
    let bob_before = native_balance(&BOB);
    System::reset_events();

    run_idle(Weight::MAX);

    assert_eq!(native_balance(&BOB), bob_before);
    assert!(Actors::actor_identity(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_none());
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
fn creation_wakeup_failures_roll_back_exactly() {
  for saturate_queue in [false, true] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      if saturate_queue {
        seed_saturated_tombstone_queue();
      }
      crate::WakeupCursorLen::<Test>::insert(
        WakeupClock::Tick,
        <<Test as crate::Config>::MaxActiveActors as Get<u32>>::get(),
      );
      let schedule = timer_schedule(if saturate_queue { 1 } else { 10 });
      prefund_active_user_creation(ALICE, &transfer_contract_steps(BOB, 1));
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
      let events_before = System::events();

      assert_noop!(
        Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          Mutability::Mutable,
          user_active_contract(schedule, None, transfer_contract_steps(BOB, 1)),
        ),
        Error::<Test>::SchedulerIndexExhausted
      );
      assert_eq!(Actors::next_actor_id(), 0);
      assert_eq!(System::events(), events_before);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        root_before
      );
    });
  }
}

#[test]
fn wakeup_drain_respects_max_wakeups_per_block() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_wakeups: u32 = <Test as crate::Config>::MaxWakeupsPerBlock::get();
    let total = max_wakeups + 5;
    let mut ids = Vec::new();
    for _ in 0..total {
      let id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      ids.push(id);
    }
    for actor_id in ids {
      assert!(schedule_latched_service_wakeup(actor_id, 1));
    }
    run_idle(Weight::MAX);
    let remaining = Actors::wakeup_buckets(1)
      .map(|bucket| bucket.live_entries)
      .unwrap_or(0);
    assert_eq!(remaining, total - max_wakeups);
  });
}

#[test]
fn wakeup_worker_stops_at_its_own_weight_envelope_without_lending() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(schedule_latched_service_wakeup(actor_id, 1));
    // The shared on_idle meter has far more than the wakeup worker's dedicated envelope; the
    // worker must stop at its own ceiling and leave the surplus for actor service (spec 8.4.5).
    let cursor_probe = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_wakeup_cursor_worker_future();
    let worker_unit = Actors::block_wakeup_cursor_drain_unit_weight_upper(
      crate::scheduler::WakeupBucketDisposition::Remove,
    );
    let worker_envelope = cursor_probe.saturating_add(worker_unit);
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(
      worker_envelope.saturating_sub(Weight::from_parts(1, 0)),
    );
    let stats = Actors::drain_overdue_wakeups_cursor(1, &mut meter);
    assert_eq!(stats.entries_scanned, 0, "worker cannot afford one complete unit");
    assert_eq!(stats.ready_entries, 0);
    assert_eq!(
      Actors::wakeup_buckets(1).expect("preserved bucket").live_entries,
      1
    );
    // The probe charge stays below one complete unit and never reaches the actor envelope.
    assert!(meter.consumed().all_lt(worker_envelope));
    // A full envelope admits the unit.
    let mut full_meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(worker_envelope);
    let full_stats = Actors::drain_overdue_wakeups_cursor(1, &mut full_meter);
    assert_eq!(full_stats.ready_entries, 1);
    assert!(Actors::wakeup_buckets(1).is_none());
  });
}

#[test]
fn wakeup_drain_preserves_bucket_when_proof_budget_cannot_admit_it() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(schedule_latched_service_wakeup(actor_id, 1));
    run_idle(Weight::from_parts(u64::MAX, 300));
    assert_eq!(
      Actors::wakeup_buckets(1)
        .expect("preserved bucket")
        .live_entries,
      1
    );
    assert_eq!(scheduled_wakeup_block(actor_id), Some(1));
    assert_eq!(Actors::wakeup_cursor_peek(), Some(1));
  });
}

#[test]
fn wakeup_drain_stops_at_the_sparse_future_minimum() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(10_000);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(schedule_latched_service_wakeup(actor_id, 1_000_000));
    run_idle(Weight::MAX);
    assert_eq!(Actors::wakeup_cursor_peek(), Some(1_000_000));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(1_000_000));
  });
}

#[test]
fn typed_ingress_notify_classifies_wakeup_capacity_as_temporary() {
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
    // Saturate the FIFO so ticket placement falls back to an exact next-block
    // wakeup, then force that wakeup placement to fail with capacity exhaustion.
    seed_saturated_tombstone_queue();
    Actors::test_fail_wakeup_placement_with_capacity();
    let actor_before = native_balance(&sovereign);
    let failure = Actors::notify_ingress(&event).expect_err("wakeup capacity must reject");
    assert_eq!(
      failure.retry,
      crate::RetryClass::Temporary,
      "recoverable queue/wakeup capacity is Temporary"
    );
    assert_eq!(
      failure.error,
      Error::<Test>::QueueCapacityUnavailable.into(),
      "failed wakeup placement surfaces as queue capacity unavailability"
    );
    assert_eq!(native_balance(&sovereign), actor_before);
    let hot = Actors::actor_hot(actor_id).expect("hot state");
    assert!(hot.queue_ticket.is_none());
    assert!(hot.wakeup_pointer.is_none(), "no phantom wakeup on failure");
  });
}

#[test]
fn paged_wakeup_uses_the_exact_requested_block_without_spillover() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    seed_saturated_tombstone_queue();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    assert_eq!(
      Actors::wakeup_buckets(2)
        .expect("paged bucket")
        .live_entries,
      1
    );
    assert_eq!(Actors::wakeup_cursor_peek(), Some(2));
  });
}

#[test]
fn defer_wakeup_deduplicates_repeated_manual_trigger_for_same_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    seed_saturated_tombstone_queue();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    assert_eq!(
      Actors::wakeup_buckets(2)
        .expect("deduplicated bucket")
        .live_entries,
      1
    );
    assert_eq!(Actors::wakeup_cursor_len(), 1);
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
fn cadence_update_rolls_back_exactly_when_existing_wakeup_cursor_is_corrupt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(20), None, inert_contract_steps());
    let initial_block = scheduled_wakeup_block(actor_id).expect("initial wakeup");
    crate::ActorWaitingCursorIndices::<Test>::remove(WakeupKey::Tick(initial_block));
    frame_system::Pallet::<Test>::set_block_number(2);
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    assert!(
      update_contract_partial!(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        timer_schedule(5),
        None,
      )
      .is_err()
    );

    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
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
fn paged_wakeup_recovery_is_independent_of_sparse_actor_ids() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    seed_saturated_tombstone_queue();
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    let capacity = <Test as crate::Config>::MaxQueueLength::get();
    Actors::paged_drain_tombstones(Actors::next_queue_ticket(), capacity)
      .expect("saturated tombstones drain coherently");
    NextActorId::<Test>::put(10_000_000);
    let bob_before = native_balance(&BOB);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(10));
    assert!(!Actors::actor_hot(actor_id).is_some_and(|hot| hot.pending_signal));
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
fn temporal_service_preserves_exact_primary_wakeup_address() {
  for schedule in [timer_schedule(1), at_time_schedule(1)] {
    let recurrent = matches!(schedule.trigger, Trigger::Cadenced { .. });
    for steps in [inert_contract_steps(), BoundedVec::default()] {
      new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(1);
        let tombstone = create_system_with(ALICE, schedule.clone(), None, inert_contract_steps());
        assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), tombstone));
        let actor_id = create_system_with(BOB, schedule.clone(), None, steps);
        let pointer = Actors::actor_hot(actor_id)
          .and_then(|hot| hot.trigger_wakeup_pointer)
          .expect("temporal physical wakeup pointer exists");
        let Some(crate::ActorControlLocation::Waiting { page, slot, .. }) =
          crate::ActorControlLocators::<Test>::get(actor_id)
        else {
          panic!("temporal frame Waiting address exists");
        };
        assert_eq!(
          (pointer.page_id, pointer.slot),
          (page, u32::from(slot)),
          "the temporal pointer must identify its single Waiting primary",
        );

        frame_system::Pallet::<Test>::set_block_number(2);
        run_idle(Weight::MAX);

        assert!(has_actor_event(|event| matches!(
          event,
          Event::CycleSummary { actor_id: id, .. } if *id == actor_id
        )));
        assert!(crate::WakeupWorkerFaultState::<Test>::get().is_none());
        assert!(!ActorIdentities::<Test>::contains_key(actor_id));

        frame_system::Pallet::<Test>::set_block_number(3);
        run_idle(Weight::MAX);
        let summaries = System::events()
          .iter()
          .filter(|record| {
            matches!(
              record.event,
              RuntimeEvent::Actors(Event::CycleSummary { actor_id: id, .. }) if id == actor_id
            )
          })
          .count();
        assert_eq!(summaries, if recurrent { 2 } else { 1 });
        assert!(crate::WakeupWorkerFaultState::<Test>::get().is_none());
        assert!(!ActorIdentities::<Test>::contains_key(actor_id));
        #[cfg(feature = "try-runtime")]
        assert_ok!(crate::Pallet::<Test>::do_try_state());
      });
    }
  }
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
fn saturated_enqueue_defers_latched_actor_and_preserves_failed_placement() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(schedule_latched_service_wakeup(actor_id, 10));
    seed_saturated_tombstone_queue();
    assert!(
      Actors::load_current_step_service_state(actor_id).is_some(),
      "canonical latched source"
    );
    assert_eq!(
      Actors::try_paged_enqueue(actor_id),
      Err(crate::EnqueueOutcome::CapacityUnavailable)
    );
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    Actors::test_fail_wakeup_placement_with_capacity();
    assert_eq!(
      Actors::enqueue(actor_id),
      Err(crate::EnqueueOutcome::WakeupCapacityExhausted)
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
    assert_ok!(Actors::enqueue(actor_id));
    let hot = Actors::actor_hot(actor_id).expect("deferred actor");
    assert!(hot.pending_signal);
    assert!(hot.queue_ticket.is_none());
    assert_eq!(
      hot.wakeup_pointer.expect("exact fallback").block,
      WakeupKey::Block(2)
    );
    assert!(matches!(
      Actors::actor_control_cell(actor_id)
        .expect("sole primary")
        .0,
      crate::ActorControlLocation::Waiting { .. }
    ));
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
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

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn canonical_waiting_publication_shares_and_releases_the_temporal_cursor_atomically() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(20), None, inert_contract_steps());
    let key = WakeupKey::Tick(21);
    let waiting_index = crate::ActorWaitingCursorIndices::<Test>::get(key)
      .expect("canonical temporal primary owns the shared key");
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Waiting { key: stored, .. }) if stored == key
    ));
    assert_eq!(crate::ActorWaitingOccupancies::<Test>::get(key), 1);
    assert_eq!(
      crate::ActorWaitingCursorIndices::<Test>::get(key),
      Some(waiting_index)
    );
    assert_eq!(Actors::wakeup_cursor_peek_key(WakeupClock::Tick), Some(key));

    frame_system::Pallet::<Test>::set_block_number(21);
    run_idle(Weight::MAX);
    assert_eq!(crate::ActorWaitingOccupancies::<Test>::get(key), 0);
    assert!(!crate::ActorWaitingCursorIndices::<Test>::contains_key(key));
    assert!(!crate::ActorWaitingOccupancies::<Test>::contains_key(key));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(41));
    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Unsignaled),
      "completed cadence retains one stable primary while the next trigger is lightweight"
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

    frame_system::Pallet::<Test>::set_block_number(21);
    run_idle(Weight::MAX);
    assert_eq!(scheduled_wakeup_block(actor_id), Some(41));
  });
}

#[test]
fn wakeup_ownership_fails_closed_for_corrupt_actor_partitions() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(schedule_latched_service_wakeup(actor_id, 10));
    let pointer = Actors::actor_hot(actor_id)
      .and_then(|hot| hot.wakeup_pointer)
      .expect("scheduled actor owns a wakeup pointer");
    ActorFunding::<Test>::remove(actor_id);
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    assert_eq!(
      Actors::try_wakeup_substrate_schedule_inner(actor_id, 20),
      Err(crate::EnqueueOutcome::CorruptedTopology)
    );
    assert_eq!(Actors::wakeup_substrate_invalidate(actor_id), None);
    assert_eq!(
      Actors::wakeup_substrate_drain_block(10, 1)
        .1
        .entries_scanned,
      0
    );

    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
    assert_eq!(
      Actors::actor_hot(actor_id).and_then(|hot| hot.wakeup_pointer),
      Some(pointer)
    );
  });
}

#[test]
fn mixed_wakeup_bucket_rolls_back_valid_neighbors_around_corruption() {
  for corrupt_index in 0usize..3 {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actors = (0..3)
        .map(|_| create_system_with(ALICE, manual_schedule(), None, inert_contract_steps()))
        .collect::<Vec<_>>();
      for actor_id in &actors {
        assert!(schedule_latched_service_wakeup(*actor_id, 10));
      }
      let primaries = actors
        .iter()
        .map(|actor_id| {
          Actors::actor_control_cell(*actor_id).expect("canonical primary before corruption")
        })
        .collect::<Vec<_>>();
      ActorFunding::<Test>::remove(actors[corrupt_index]);
      let pointers = actors
        .iter()
        .map(|actor_id| {
          Actors::actor_hot(*actor_id)
            .and_then(|hot| hot.wakeup_pointer)
            .expect("wakeup pointer")
        })
        .collect::<Vec<_>>();
      let events_before = System::events();
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

      let (ready, stats) = Actors::wakeup_substrate_drain_block(10, 3);

      assert!(ready.is_empty());
      assert_eq!(stats, Default::default());
      assert_eq!(System::events(), events_before);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        root_before
      );
      assert_eq!(Actors::combined_queue_occupancy(), 0);
      for (index, actor_id) in actors.iter().enumerate() {
        assert_eq!(
          Actors::actor_hot(*actor_id).and_then(|hot| hot.wakeup_pointer),
          Some(pointers[index])
        );
        assert_eq!(
          Actors::actor_control_cell(*actor_id),
          Some(primaries[index].clone())
        );
        assert_eq!(primaries[index].1.identity.cycle_nonce, 0);
        if index == corrupt_index {
          assert!(Actors::actor_identity(*actor_id).is_none());
        } else {
          assert_eq!(
            Actors::actor_identity(*actor_id)
              .expect("healthy identity")
              .cycle_nonce,
            0
          );
        }
      }
      #[cfg(feature = "try-runtime")]
      assert!(crate::Pallet::<Test>::do_try_state().is_err());
    });
  }
}
