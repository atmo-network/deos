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

  let entries =
    BoundedVec::<Option<WakeupEntry>, <Test as crate::Config>::WakeupPageSize>::try_from(vec![
      Some(WakeupEntry { actor_id: 9 }),
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
  assert_eq!(page.entries[0], Some(WakeupEntry { actor_id: 9 }));
  assert_eq!(page.entries[1], None);
  assert_eq!(page.live_entries, 1);
  assert_eq!((page.previous_page, page.next_page), (Some(6), Some(8)));

  let bucket = WakeupBucketState {
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

    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
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

    assert_eq!(
      Actors::wakeup_substrate_invalidate(actor_id),
      Some(replacement)
    );
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
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    WakeupBuckets::<Test>::mutate(WakeupKey::Block(10), |maybe_bucket| {
      maybe_bucket.as_mut().expect("bucket").cursor_index = Some(1);
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
fn wakeup_pointer_corruption_matrix_fails_closed_and_is_detected() {
  for corruption in 0u8..8 {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      let other = create_system_with(BOB, manual_schedule(), None, inert_contract_steps());
      assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
      match corruption {
        0 => ActorHot::<Test>::mutate(actor_id, |maybe| {
          maybe
            .as_mut()
            .expect("active actor")
            .wakeup_pointer
            .as_mut()
            .expect("wakeup pointer")
            .block = WakeupKey::Block(11);
        }),
        1 => ActorHot::<Test>::mutate(actor_id, |maybe| {
          maybe
            .as_mut()
            .expect("active actor")
            .wakeup_pointer
            .as_mut()
            .expect("wakeup pointer")
            .page_id = 1;
        }),
        2 => ActorHot::<Test>::mutate(actor_id, |maybe| {
          maybe
            .as_mut()
            .expect("active actor")
            .wakeup_pointer
            .as_mut()
            .expect("wakeup pointer")
            .slot = 7;
        }),
        3 => WakeupPages::<Test>::mutate((WakeupKey::Block(10), 0), |maybe| {
          maybe.as_mut().expect("wakeup page").entries[0] =
            Some(crate::WakeupEntry { actor_id: other });
        }),
        4 => WakeupPages::<Test>::remove((WakeupKey::Block(10), 0)),
        5 => WakeupBuckets::<Test>::remove(WakeupKey::Block(10)),
        6 => WakeupBuckets::<Test>::mutate(WakeupKey::Block(10), |maybe| {
          maybe.as_mut().expect("wakeup bucket").cursor_index = None;
        }),
        7 => WakeupBuckets::<Test>::mutate(WakeupKey::Block(10), |maybe| {
          maybe.as_mut().expect("wakeup bucket").live_entries = 0;
        }),
        _ => unreachable!(),
      }
      let events_before = System::events();
      let identity_before = Actors::actor_identities(actor_id).expect("identity");
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

      assert_eq!(
        Actors::try_wakeup_substrate_schedule_inner(actor_id, 20),
        Err(crate::EnqueueOutcome::CorruptedTopology),
        "replacement case {corruption}"
      );
      assert_eq!(Actors::wakeup_substrate_invalidate(actor_id), None);
      assert_eq!(System::events(), events_before);
      assert_eq!(Actors::actor_identities(actor_id), Some(identity_before));
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

#[test]
fn wakeup_replacement_rolls_back_when_existing_page_or_slot_is_missing() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    WakeupPages::<Test>::remove((WakeupKey::Block(10), 0));
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
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    ActorHot::<Test>::mutate(actor_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("hot")
        .wakeup_pointer
        .as_mut()
        .expect("pointer")
        .slot = 7;
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
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    WakeupBuckets::<Test>::mutate(WakeupKey::Block(10), |maybe_bucket| {
      maybe_bucket.as_mut().expect("bucket").live_entries = 0;
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
fn wakeup_cursor_capacity_overflow_fails_closed_and_preserves_existing_path() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
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
    // Seed a bucket whose next_page_id is at the u64 ceiling; appending a new page
    // cannot advance the monotonic page index and must fail closed as
    // WakeupIndexExhausted (mapped to the SchedulerIndexExhausted public error).
    let block = 10u64;
    assert!(Actors::wakeup_substrate_schedule(actor_id, block));
    // Fill the tail page to force the page-append branch, then set the next page id
    // at the u64 ceiling so the monotonic index cannot advance.
    let page_size = <<Test as crate::Config>::WakeupPageSize as Get<u32>>::get();
    for _ in 0..page_size.saturating_sub(1) {
      let extra = create_system_with(
        BOB,
        manual_schedule(),
        None,
        transfer_contract_steps(CHARLIE, 1),
      );
      assert!(Actors::wakeup_substrate_schedule(extra, block));
    }
    crate::WakeupBuckets::<Test>::mutate(WakeupKey::Block(block), |bucket| {
      let bucket = bucket.as_mut().expect("bucket exists");
      bucket.next_page_id = u64::MAX;
    });
    let pointer_before = Actors::actor_hot(actor_id)
      .expect("hot")
      .wakeup_pointer
      .expect("existing pointer");
    // A different actor scheduling into the full bucket with the page index at the
    // u64 ceiling cannot advance the monotonic index and fails closed.
    let new_actor = create_system_with(
      BOB,
      manual_schedule(),
      None,
      transfer_contract_steps(CHARLIE, 1),
    );
    assert!(matches!(
      crate::Pallet::<Test>::try_wakeup_substrate_schedule_inner(new_actor, block),
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
    assert!(Actors::wakeup_substrate_schedule(actor_id, block));
    // Corrupt the TARGET bucket for a different actor: a bucket exists at block + 1 but
    // its cursor_index is missing, so the schedule path must fail closed as corrupted
    // topology instead of retrying as queue-full.
    let target = block + 1;
    crate::WakeupBuckets::<Test>::insert(
      WakeupKey::Block(target),
      crate::WakeupBucketState {
        head_page: 0,
        tail_page: 0,
        next_page_id: 1,
        live_entries: 1,
        cursor_index: None,
      },
    );
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
      crate::Pallet::<Test>::try_wakeup_substrate_schedule_inner(new_actor, target),
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
    assert_ok!(Actors::enqueue(actor_id));
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
    crate::WakeupBuckets::<Test>::insert(
      WakeupKey::Block(2),
      crate::WakeupBucketState {
        head_page: 0,
        tail_page: 0,
        next_page_id: 1,
        live_entries: 0,
        cursor_index: None,
      },
    );
    // The placement must fail closed: the caller learns the actor owns no path
    // instead of believing enqueue succeeded while readiness was lost.
    assert!(matches!(
      Actors::enqueue(actor_id),
      Err(crate::EnqueueOutcome::CorruptedTopology)
    ));
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
    crate::WakeupBuckets::<Test>::insert(
      WakeupKey::Block(2),
      crate::WakeupBucketState {
        head_page: 0,
        tail_page: 0,
        next_page_id: 1,
        live_entries: 0,
        cursor_index: None,
      },
    );
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
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    let pointer = Actors::actor_hot(actor_id)
      .expect("hot state")
      .wakeup_pointer
      .expect("wakeup pointer");
    crate::pallet::WakeupBuckets::<Test>::mutate(WakeupKey::Block(10), |maybe_bucket| {
      maybe_bucket.as_mut().expect("wakeup bucket").cursor_index = None;
    });

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
    let page_size: u32 = <Test as crate::Config>::WakeupPageSize::get();
    let count = page_size.saturating_mul(2).saturating_add(1);
    let mut actors = Vec::new();
    for _ in 0..count {
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
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
    for actor_id in &actors[page_size..page_size * 2] {
      assert!(Actors::wakeup_substrate_invalidate(*actor_id).is_some());
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

    for actor_id in actors {
      let _ = Actors::wakeup_substrate_invalidate(actor_id);
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
    let page_size: u32 = <Test as crate::Config>::WakeupPageSize::get();
    let count = page_size.saturating_mul(2).saturating_add(1);
    let mut actors = Vec::new();
    for _ in 0..count {
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
      actors.push(actor_id);
    }

    let first_limit = page_size / 2;
    let (first, first_stats) = Actors::wakeup_substrate_drain_block(10, first_limit);
    assert_eq!(first.as_slice(), &actors[..first_limit as usize]);
    assert_eq!(first_stats.entries_scanned, first_limit);
    assert_eq!(first_stats.ready_entries, first_limit);
    assert_eq!(first_stats.pages_touched, 1);
    assert_eq!(first_stats.pages_deleted, 0);
    let head = Actors::wakeup_pages((10, 0)).expect("partially drained head");
    assert_eq!(head.scan_slot, first_limit);
    assert_eq!(head.live_entries, page_size - first_limit);

    let (second, second_stats) = Actors::wakeup_substrate_drain_block(10, page_size);
    let second_end = first_limit.saturating_add(page_size) as usize;
    assert_eq!(second.as_slice(), &actors[first_limit as usize..second_end]);
    assert_eq!(second_stats.entries_scanned, page_size);
    assert_eq!(second_stats.ready_entries, page_size);
    assert_eq!(second_stats.pages_touched, 2);
    assert_eq!(second_stats.pages_deleted, 1);
    let bucket = Actors::wakeup_buckets(10).expect("remaining wakeup bucket");
    assert_eq!(bucket.head_page, 1);
    let head = Actors::wakeup_pages((10, 1)).expect("second partial head");
    assert_eq!(head.previous_page, None);
    assert_eq!(head.scan_slot, first_limit);

    let (final_ready, final_stats) = Actors::wakeup_substrate_drain_block(10, u32::MAX);
    assert_eq!(final_ready.as_slice(), &actors[second_end..]);
    assert_eq!(final_stats.ready_entries, count - first_limit - page_size);
    assert_eq!(final_stats.pages_touched, 2);
    assert_eq!(final_stats.pages_deleted, 2);
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
fn paged_wakeup_drain_discards_stale_only_bucket() {
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
      assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
      ActorHot::<Test>::mutate(actor_id, |maybe_hot| {
        maybe_hot.as_mut().expect("hot state").wakeup_pointer = None;
      });
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
    assert!(Actors::wakeup_substrate_schedule(due, 10));
    assert!(Actors::wakeup_substrate_schedule(future, 1_000_000));

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
      assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
      actors.push(actor_id);
    }
    let limit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_wakeup_cursor_worker_future()
        .saturating_add(Actors::wakeup_cursor_drain_unit_weight_upper(false));
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
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    let required =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_wakeup_cursor_worker_future()
        .saturating_add(Actors::wakeup_cursor_drain_unit_weight_upper(true));

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
      .saturating_add(Actors::wakeup_cursor_drain_unit_weight_upper(true));
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
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    crate::NextQueueTicket::<Test>::put(u64::MAX);
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
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    QueueTail::<Test>::put(1);
    QueueOccupancy::<Test>::put(0);
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let mut meter = WeightMeter::with_limit(Weight::MAX);

    let stats = Actors::drain_overdue_wakeups_cursor(10, &mut meter);

    assert_eq!(stats.entries_scanned, 0);
    assert_eq!(System::events(), events_before);
    assert_ne!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
    assert_eq!(scheduled_wakeup_block(actor_id), Some(10));
    assert_eq!(
      Actors::wakeup_worker_fault(),
      Some(crate::WakeupWorkerFault {
        key: WakeupKey::Block(10),
        page: 0,
        class: crate::CrossingWorkerFaultClass::Invariant,
      })
    );
    let mut halted = WeightMeter::with_limit(Weight::MAX);
    assert_eq!(
      Actors::drain_overdue_wakeups_cursor(10, &mut halted).entries_scanned,
      0
    );
    assert_noop!(
      Actors::clear_wakeup_worker_fault(RuntimeOrigin::signed(ALICE)),
      DispatchError::BadOrigin
    );
    QueueTail::<Test>::put(0);
    assert_ok!(Actors::clear_wakeup_worker_fault(RuntimeOrigin::root()));
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
      assert!(Actors::wakeup_substrate_schedule(actor_id, block));
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

    let removed = blocks[(count / 2) as usize];
    assert!(Actors::wakeup_cursor_remove(removed));
    assert!(!Actors::wakeup_cursor_remove(removed));
    assert_eq!(Actors::wakeup_cursor_len(), count.saturating_sub(1));
    assert_eq!(
      Actors::wakeup_buckets(removed)
        .expect("removed cursor bucket")
        .cursor_index,
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
      Actors::wakeup_buckets(*block)
        .expect("wakeup bucket")
        .cursor_index
        .is_none()
    }));

    for actor_id in actors {
      let _ = Actors::wakeup_substrate_invalidate(actor_id);
    }
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn creation_and_activation_before_cutoff_use_exact_next_block_wakeup() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    crate::NextQueueTicket::<Test>::put(u64::MAX);
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
    crate::NextQueueTicket::<Test>::put(u64::MAX);
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
    crate::NextQueueTicket::<Test>::put(u64::MAX);
    let bob_before = native_balance(&BOB);
    System::reset_events();

    run_idle(Weight::MAX);

    assert_eq!(native_balance(&BOB), bob_before);
    assert!(Actors::actor_identities(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_none());
    assert!(Actors::actor_funding(actor_id).is_none());
    assert_eq!(Actors::combined_queue_occupancy(), 0);
    assert!(!WakeupBuckets::<Test>::contains_key(WakeupKey::Block(2)));
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
fn post_attempt_wakeup_index_exhaustion_commits_then_closes() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      timer_schedule(1),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 100);
    let page_size = <Test as crate::Config>::WakeupPageSize::get();
    for _ in 0..page_size {
      create_system_with(ALICE, timer_schedule(2), None, inert_contract_steps());
    }
    WakeupBuckets::<Test>::mutate(WakeupKey::Tick(3), |maybe| {
      maybe
        .as_mut()
        .expect("saturated future bucket")
        .next_page_id = u64::MAX;
    });
    frame_system::Pallet::<Test>::set_block_number(2);
    let bob_before = native_balance(&BOB);
    System::reset_events();

    run_idle(Weight::MAX);

    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert!(Actors::actor_identities(actor_id).is_none());
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
      assert!(Actors::wakeup_substrate_schedule(actor_id, 1));
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
    assert!(Actors::wakeup_substrate_schedule(actor_id, 1));
    // The shared on_idle meter has far more than the wakeup worker's dedicated envelope; the
    // worker must stop at its own ceiling and leave the surplus for actor service (spec 8.4.5).
    let cursor_probe = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_wakeup_cursor_worker_future();
    let worker_unit = Actors::wakeup_cursor_drain_unit_weight_upper(true);
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
    assert!(Actors::wakeup_substrate_schedule(actor_id, 1));
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
    assert!(Actors::wakeup_substrate_schedule(actor_id, 1_000_000));
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
    assert!(WakeupBuckets::<Test>::get(WakeupKey::Tick(initial_block)).is_none());
    assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick), 1);
  });
}

#[test]
fn cadence_update_rolls_back_exactly_when_existing_wakeup_cursor_is_corrupt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(20), None, inert_contract_steps());
    let initial_block = scheduled_wakeup_block(actor_id).expect("initial wakeup");
    WakeupBuckets::<Test>::mutate(WakeupKey::Tick(initial_block), |maybe_bucket| {
      maybe_bucket.as_mut().expect("bucket").cursor_index = None;
    });
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
      ActorHot::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("hot").terminal_at = Some(50);
      });
      ActorContracts::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("contract").window = Some(ScheduleWindow { start: 1, end: 49 });
      });
      assert_eq!(
        crate::Pallet::<Test>::do_try_state().map_err(|error| format!("{error:?}")),
        Err("Other(\"ActorHot wakeup pointer exceeds its terminal membership\")".into())
      );
      ActorHot::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("hot").terminal_at = Some(102);
      });
      ActorContracts::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("contract").window = Some(ScheduleWindow { start: 1, end: 101 });
      });
      assert_ok!(crate::Pallet::<Test>::do_try_state());
    }
  });
}

#[test]
fn temporal_membership_try_state_accepts_lazy_wakeup_tombstones() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(20), None, inert_contract_steps());
    let scheduled_block = scheduled_wakeup_block(actor_id).expect("timer wakeup scheduled");
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
    // Lazy terminal cleanup leaves a stale physical wakeup entry behind; the entry carries no
    // membership authority and must not fail try_state (spec 5.1 stale-entry semantics).
    assert!(WakeupBuckets::<Test>::contains_key(WakeupKey::Tick(
      scheduled_block
    )));
    assert!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick) > 0);
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    // The bounded drain converges the tombstone at its due tick.
    frame_system::Pallet::<Test>::set_block_number(scheduled_block);
    run_idle(Weight::MAX);
    assert!(!WakeupBuckets::<Test>::contains_key(WakeupKey::Tick(
      scheduled_block
    )));
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
fn close_before_future_wakeup_leaves_harmless_lazy_tombstone() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(20), None, inert_contract_steps());
    let scheduled_block =
      scheduled_wakeup_block(actor_id).expect("timer wakeup should be scheduled");
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(scheduled_wakeup_block(actor_id).is_none());
    assert!(WakeupBuckets::<Test>::contains_key(WakeupKey::Tick(
      scheduled_block
    )));
    assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick), 1);
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

#[test]
fn repeated_timer_close_churn_converges_lazy_wakeup_tombstones() {
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
    assert!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick) > 0);
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
      Actors::continuation_state(actor_id).is_some(),
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
    let page_size: u32 = <Test as crate::Config>::QueuePageSize::get();
    let capacity: u32 = <Test as crate::Config>::MaxQueueLength::get();
    for page_id in 0..capacity.div_ceil(page_size) {
      let first = page_id.saturating_mul(page_size);
      let len = page_size.min(capacity.saturating_sub(first));
      let entries = (0..len)
        .map(|offset| {
          let ticket = u64::from(first.saturating_add(offset));
          QueueEntry {
            ticket,
            actor_id: if ticket == existing_ticket {
              actor_id
            } else {
              30_000_000u64.saturating_add(ticket)
            },
          }
        })
        .collect::<Vec<_>>();
      QueuePages::<Test>::insert(
        u64::from(page_id),
        BoundedVec::try_from(entries).expect("saturated queue page fits"),
      );
    }
    QueueHead::<Test>::put(0);
    QueueTail::<Test>::put(u64::from(capacity));
    QueueOccupancy::<Test>::put(capacity);
    crate::NextQueueTicket::<Test>::put(u64::from(capacity));
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
    assert!(Actors::continuation_state(actor_id).is_none());
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
    crate::WakeupBuckets::<Test>::insert(
      WakeupKey::Block(next_retry),
      crate::WakeupBucketState {
        head_page: 0,
        tail_page: 0,
        next_page_id: 1,
        live_entries: 0,
        cursor_index: None,
      },
    );
    let actor_before = Actors::active_actor_view(actor_id).expect("queued continuation");
    let continuation_before = Actors::continuation_state(actor_id)
      .expect("continuation before corrupt retry placement")
      .encode();
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    let _ = Actors::execute_cycle(Weight::MAX);

    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("continuation survives failed placement")
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

    assert_ok!(Actors::cancel_continuation(RuntimeOrigin::root(), actor_id));
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
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
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
        assert!(Actors::wakeup_substrate_schedule(*actor_id, 10));
      }
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
        assert_eq!(Actors::actor_identities(*actor_id).unwrap().cycle_nonce, 0);
      }
      #[cfg(feature = "try-runtime")]
      assert!(crate::Pallet::<Test>::do_try_state().is_err());
    });
  }
}
