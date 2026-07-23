use super::pallet::*;
use super::{AssetOps, FundingAuthority, weights::WeightInfo};
use alloc::vec::Vec;
use frame::prelude::*;
use polkadot_sdk::sp_runtime::traits::{One, Saturating, Zero};
use polkadot_sdk::sp_weights::WeightMeter;

enum AdmissionDecision {
  Admit(Weight),
  Closed(Weight),
  CloseFailed(Weight),
  Defer(DeferReason),
  Skip,
}

impl<T: Config> Pallet<T> {
  pub(crate) fn execute_cycle(remaining_weight: Weight) -> Weight {
    if remaining_weight.is_zero() {
      return Weight::zero();
    }
    let mut cycle_meter = WeightMeter::with_limit(remaining_weight);
    let now = frame_system::Pallet::<T>::block_number();
    // Materialize already-due temporal readiness before taking the block-start
    // run-queue snapshot. Every later append, including actor self-enqueue during
    // execution, receives a ticket at or beyond this cutoff and waits one block.
    Self::drain_overdue_wakeups_paged(now, &mut cycle_meter);
    let cutoff = QueueTail::<T>::get();

    let max_executions = T::MaxExecutionsPerBlock::get();
    let max_scanned = T::MaxQueueEntriesScannedPerBlock::get();
    let scan_weight = T::WeightInfo::scheduler_paged_tombstone_drain(1);
    let consume_weight = T::WeightInfo::scheduler_paged_consume_preserve_page()
      .max(T::WeightInfo::scheduler_paged_consume_delete_page());
    let probe_weight = Self::scheduler_probe_weight_upper().saturating_add(consume_weight);
    let mut executed = 0u32;
    let mut scanned = 0u32;
    let mut periodic_continuations: Vec<AaaId> = Vec::new();

    while executed < max_executions && scanned < max_scanned {
      if QueueHead::<T>::get() >= cutoff || !cycle_meter.can_consume(scan_weight) {
        break;
      }
      let before = QueueHead::<T>::get();
      let drain = Self::paged_drain_tombstones(cutoff, 1);
      if drain.entries_scanned == 0 {
        break;
      }
      cycle_meter.consume(scan_weight);
      scanned = scanned.saturating_add(drain.entries_scanned);
      if QueueHead::<T>::get() != before {
        continue;
      }

      let Some((ticket, entry)) = Self::paged_head_entry() else {
        break;
      };
      if ticket >= cutoff || !cycle_meter.can_consume(probe_weight) {
        break;
      }
      let Some(hot) = ActorHot::<T>::get(entry.aaa_id) else {
        continue;
      };
      if hot.queue_ticket != Some(ticket) {
        continue;
      }
      cycle_meter.consume(probe_weight);
      let aaa_id = entry.aaa_id;
      if hot.cycle_nonce > 0 && hot.last_cycle_block == now {
        break;
      }
      if hot.lifecycle.is_paused() {
        if !Self::paged_consume_head(ticket) {
          break;
        }
        continue;
      }
      let Some(program) = ActorProgram::<T>::get(aaa_id) else {
        if !Self::paged_consume_head(ticket) {
          break;
        }
        continue;
      };
      let instance = Self::compose_active_actor(hot, program);
      let cycle_weight_upper = match Self::apply_admission(aaa_id, &instance, &cycle_meter) {
        AdmissionDecision::Admit(weight) => {
          if !Self::paged_consume_head(ticket) {
            break;
          }
          weight
        }
        AdmissionDecision::Closed(weight) => {
          let _ = Self::paged_consume_head(ticket);
          cycle_meter.consume(weight);
          continue;
        }
        AdmissionDecision::CloseFailed(weight) => {
          cycle_meter.consume(weight);
          Self::deposit_event(Event::CycleDeferred {
            aaa_id,
            reason: DeferReason::CloseTransitionFailed,
          });
          break;
        }
        AdmissionDecision::Defer(reason) => {
          Self::deposit_event(Event::CycleDeferred { aaa_id, reason });
          break;
        }
        AdmissionDecision::Skip => {
          if !Self::paged_consume_head(ticket) {
            break;
          }
          if let Some(updated) = Self::active_actor_snapshot(aaa_id) {
            Self::preserve_skipped_readiness(aaa_id, &updated, now, &mut periodic_continuations);
          }
          continue;
        }
      };
      let _actual = Self::execute_single_cycle(aaa_id);
      cycle_meter.consume(cycle_weight_upper);
      executed = executed.saturating_add(1);
      if let Some(updated) = Self::active_actor_snapshot(aaa_id) {
        Self::schedule_next_timer_wakeup_local(aaa_id, &updated, now, &mut periodic_continuations);
      }
    }
    for aaa_id in periodic_continuations {
      Self::enqueue(aaa_id);
    }
    cycle_meter.consumed()
  }

  pub(crate) fn enqueue(aaa_id: AaaId) {
    if !Self::paged_enqueue(aaa_id) {
      let next_block = frame_system::Pallet::<T>::block_number().saturating_add(One::one());
      Self::defer_wakeup(aaa_id, next_block);
    }
  }

  fn queue_page_size() -> u64 {
    u64::from(T::QueuePageSize::get())
  }

  fn queue_page_and_slot(ticket: QueueTicket) -> (QueuePageId, usize) {
    let page_size = Self::queue_page_size();
    ((ticket / page_size), (ticket % page_size) as usize)
  }

  /// Append one actor to the inactive paged-FIFO substrate.
  ///
  /// The active scheduler switches to this primitive only after its page
  /// operation weights and cutoff flow replace the double-buffer queue.
  pub fn paged_enqueue(aaa_id: AaaId) -> bool {
    let Some(hot) = ActorHot::<T>::get(aaa_id) else {
      return false;
    };
    if hot.queue_ticket.is_some() {
      return true;
    }
    let head = QueueHead::<T>::get();
    let tail = QueueTail::<T>::get();
    if tail < head || tail.saturating_sub(head) >= u64::from(T::MaxQueueLength::get()) {
      return false;
    }
    let Some(next_tail) = tail.checked_add(1) else {
      return false;
    };
    let (page_id, slot) = Self::queue_page_and_slot(tail);
    let mut page = QueuePages::<T>::get(page_id).unwrap_or_default();
    if page.len() != slot || page.try_push(QueueEntry { aaa_id }).is_err() {
      return false;
    }
    QueuePages::<T>::insert(page_id, page);
    QueueTail::<T>::put(next_tail);
    ActorHot::<T>::mutate(aaa_id, |maybe| {
      if let Some(hot) = maybe.as_mut() {
        hot.queue_ticket = Some(tail);
      }
    });
    true
  }

  pub fn paged_invalidate(aaa_id: AaaId) -> Option<QueueTicket> {
    ActorHot::<T>::mutate(aaa_id, |maybe| {
      maybe.as_mut().and_then(|hot| hot.queue_ticket.take())
    })
  }

  pub fn paged_head_entry() -> Option<(QueueTicket, QueueEntry)> {
    let head = QueueHead::<T>::get();
    if head >= QueueTail::<T>::get() {
      return None;
    }
    let (page_id, slot) = Self::queue_page_and_slot(head);
    QueuePages::<T>::get(page_id)
      .and_then(|page| page.get(slot).copied())
      .map(|entry| (head, entry))
  }

  /// Commit physical consumption only after the scheduler has decided that the
  /// current head may advance. Matching live membership clears actor-local state;
  /// mismatches remain distinguishable tombstones.
  pub fn paged_consume_head(ticket: QueueTicket) -> bool {
    let head = QueueHead::<T>::get();
    let tail = QueueTail::<T>::get();
    if ticket != head || head >= tail {
      return false;
    }
    let Some((_, entry)) = Self::paged_head_entry() else {
      return false;
    };
    let Some(next_head) = head.checked_add(1) else {
      return false;
    };
    let page_size = Self::queue_page_size();
    let (page_id, _) = Self::queue_page_and_slot(head);
    if next_head == tail {
      let remainder = next_head % page_size;
      let aligned = if remainder == 0 {
        next_head
      } else {
        let Some(aligned) = next_head.checked_add(page_size.saturating_sub(remainder)) else {
          return false;
        };
        aligned
      };
      QueuePages::<T>::remove(page_id);
      QueueHead::<T>::put(aligned);
      QueueTail::<T>::put(aligned);
    } else {
      QueueHead::<T>::put(next_head);
      if next_head % page_size == 0 {
        QueuePages::<T>::remove(page_id);
      }
    }
    ActorHot::<T>::mutate(entry.aaa_id, |maybe| {
      if let Some(hot) = maybe.as_mut()
        && hot.queue_ticket == Some(ticket)
      {
        hot.queue_ticket = None;
      }
    });
    true
  }

  /// Drain only stale physical entries before `cutoff`, stopping at the first
  /// live actor ticket. Scanning and successful execution therefore remain
  /// independent resources, and a caller can snapshot `QueueTail` at block start.
  pub fn paged_drain_tombstones(cutoff: QueueTicket, scan_limit: u32) -> QueueDrainStats {
    let mut stats = QueueDrainStats::default();
    if scan_limit == 0 {
      return stats;
    }
    let original_head = QueueHead::<T>::get();
    let tail = QueueTail::<T>::get();
    let cutoff = cutoff.min(tail);
    let page_size = Self::queue_page_size();
    let mut head = original_head;
    let mut last_deleted_page = None;

    'pages: while head < cutoff && stats.entries_scanned < scan_limit {
      let (page_id, mut slot) = Self::queue_page_and_slot(head);
      let Some(page) = QueuePages::<T>::get(page_id) else {
        break;
      };
      stats.pages_touched = stats.pages_touched.saturating_add(1);
      while head < cutoff && stats.entries_scanned < scan_limit && slot < page.len() {
        let entry = page[slot];
        stats.entries_scanned = stats.entries_scanned.saturating_add(1);
        let is_live =
          ActorHot::<T>::get(entry.aaa_id).is_some_and(|hot| hot.queue_ticket == Some(head));
        if is_live {
          break 'pages;
        }
        stats.tombstones_skipped = stats.tombstones_skipped.saturating_add(1);
        head = head.saturating_add(1);
        slot = slot.saturating_add(1);
      }
      if slot == page.len() {
        QueuePages::<T>::remove(page_id);
        last_deleted_page = Some(page_id);
        stats.pages_deleted = stats.pages_deleted.saturating_add(1);
      } else if slot < page.len() {
        break;
      }
    }

    if head == original_head {
      return stats;
    }
    if head == tail {
      let remainder = tail % page_size;
      let aligned = if remainder == 0 {
        tail
      } else {
        tail.saturating_add(page_size.saturating_sub(remainder))
      };
      if remainder != 0 {
        let (page_id, _) = Self::queue_page_and_slot(head.saturating_sub(1));
        if last_deleted_page != Some(page_id) {
          QueuePages::<T>::remove(page_id);
          stats.pages_deleted = stats.pages_deleted.saturating_add(1);
        }
      }
      QueueHead::<T>::put(aligned);
      QueueTail::<T>::put(aligned);
    } else {
      QueueHead::<T>::put(head);
    }
    stats
  }

  pub(crate) fn wakeup_page_entry_matches(
    pointer: WakeupPointer<BlockNumberFor<T>>,
    aaa_id: AaaId,
  ) -> bool {
    WakeupPages::<T>::get((pointer.block, pointer.page_id))
      .and_then(|page| page.entries.get(pointer.slot as usize).copied().flatten())
      .is_some_and(|entry| entry.aaa_id == aaa_id)
  }

  fn wakeup_substrate_invalidate_inner(aaa_id: AaaId) -> Option<WakeupPointer<BlockNumberFor<T>>> {
    let pointer = ActorHot::<T>::get(aaa_id)?.wakeup_pointer?;
    ActorHot::<T>::mutate(aaa_id, |maybe_hot| {
      if let Some(hot) = maybe_hot
        && hot.wakeup_pointer == Some(pointer)
      {
        hot.wakeup_pointer = None;
      }
    });
    let key = (pointer.block, pointer.page_id);
    let Some(mut page) = WakeupPages::<T>::get(key) else {
      return Some(pointer);
    };
    let Some(slot) = page.entries.get_mut(pointer.slot as usize) else {
      return Some(pointer);
    };
    if !slot.is_some_and(|entry| entry.aaa_id == aaa_id) {
      return Some(pointer);
    }
    *slot = None;
    page.live_entries = page.live_entries.saturating_sub(1);
    let Some(mut bucket) = WakeupBuckets::<T>::get(pointer.block) else {
      WakeupPages::<T>::insert(key, page);
      return Some(pointer);
    };
    bucket.live_entries = bucket.live_entries.saturating_sub(1);
    if page.live_entries > 0 {
      WakeupPages::<T>::insert(key, page);
      WakeupBuckets::<T>::insert(pointer.block, bucket);
      return Some(pointer);
    }

    if let Some(previous_page) = page.previous_page {
      WakeupPages::<T>::mutate((pointer.block, previous_page), |maybe_previous| {
        if let Some(previous) = maybe_previous {
          previous.next_page = page.next_page;
        }
      });
    } else {
      bucket.head_page = page.next_page.unwrap_or(bucket.tail_page);
    }
    if let Some(next_page) = page.next_page {
      WakeupPages::<T>::mutate((pointer.block, next_page), |maybe_next| {
        if let Some(next) = maybe_next {
          next.previous_page = page.previous_page;
        }
      });
    } else {
      bucket.tail_page = page.previous_page.unwrap_or(bucket.head_page);
    }
    WakeupPages::<T>::remove(key);
    if bucket.live_entries == 0 {
      if !Self::wakeup_cursor_remove_inner(pointer.block) {
        return None;
      }
      WakeupBuckets::<T>::remove(pointer.block);
    } else {
      WakeupBuckets::<T>::insert(pointer.block, bucket);
    }
    Some(pointer)
  }

  pub fn wakeup_substrate_invalidate(aaa_id: AaaId) -> Option<WakeupPointer<BlockNumberFor<T>>> {
    let result: Result<WakeupPointer<BlockNumberFor<T>>, DispatchError> =
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Self::wakeup_substrate_invalidate_inner(aaa_id) {
          Some(pointer) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(pointer))
          }
          None => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::AaaNotFound.into(),
          )),
        }
      });
    result.ok()
  }

  fn wakeup_substrate_schedule_inner(aaa_id: AaaId, wakeup_block: BlockNumberFor<T>) -> bool {
    let Some(hot) = ActorHot::<T>::get(aaa_id) else {
      return false;
    };
    if let Some(pointer) = hot.wakeup_pointer {
      if pointer.block == wakeup_block && Self::wakeup_page_entry_matches(pointer, aaa_id) {
        return true;
      }
      Self::wakeup_substrate_invalidate_inner(aaa_id);
    }

    let (page_id, slot) = if let Some(mut bucket) = WakeupBuckets::<T>::get(wakeup_block) {
      if bucket.cursor_index.is_none() {
        return false;
      }
      let tail_key = (wakeup_block, bucket.tail_page);
      let Some(mut tail_page) = WakeupPages::<T>::get(tail_key) else {
        return false;
      };
      let reusable_slot = tail_page
        .entries
        .iter() // deos-bypass: bounded-iter — WakeupPageSize-bounded tail-page slot reuse
        .enumerate()
        .skip(tail_page.scan_slot as usize)
        .find_map(|(slot, entry)| entry.is_none().then_some(slot));
      let slot = if let Some(slot) = reusable_slot {
        tail_page.entries[slot] = Some(WakeupEntry { aaa_id });
        slot
      } else if tail_page.entries.len() < T::WakeupPageSize::get() as usize {
        let slot = tail_page.entries.len();
        if tail_page
          .entries
          .try_push(Some(WakeupEntry { aaa_id }))
          .is_err()
        {
          return false;
        }
        slot
      } else {
        let page_id = bucket.next_page_id;
        let Some(next_page_id) = page_id.checked_add(1) else {
          return false;
        };
        let mut entries = WakeupPageEntriesOf::<T>::default();
        if entries.try_push(Some(WakeupEntry { aaa_id })).is_err() {
          return false;
        }
        tail_page.next_page = Some(page_id);
        WakeupPages::<T>::insert(tail_key, tail_page);
        WakeupPages::<T>::insert(
          (wakeup_block, page_id),
          WakeupPage {
            entries,
            live_entries: 1,
            scan_slot: 0,
            previous_page: Some(bucket.tail_page),
            next_page: None,
          },
        );
        bucket.tail_page = page_id;
        bucket.next_page_id = next_page_id;
        bucket.live_entries = bucket.live_entries.saturating_add(1);
        WakeupBuckets::<T>::insert(wakeup_block, bucket);
        return Self::set_wakeup_pointer(aaa_id, wakeup_block, page_id, 0);
      };
      tail_page.live_entries = tail_page.live_entries.saturating_add(1);
      let page_id = bucket.tail_page;
      WakeupPages::<T>::insert(tail_key, tail_page);
      bucket.live_entries = bucket.live_entries.saturating_add(1);
      WakeupBuckets::<T>::insert(wakeup_block, bucket);
      (page_id, slot as WakeupSlot)
    } else {
      let mut entries = WakeupPageEntriesOf::<T>::default();
      if entries.try_push(Some(WakeupEntry { aaa_id })).is_err() {
        return false;
      }
      WakeupPages::<T>::insert(
        (wakeup_block, 0),
        WakeupPage {
          entries,
          live_entries: 1,
          scan_slot: 0,
          previous_page: None,
          next_page: None,
        },
      );
      WakeupBuckets::<T>::insert(
        wakeup_block,
        WakeupBucketState {
          head_page: 0,
          tail_page: 0,
          next_page_id: 1,
          live_entries: 1,
          cursor_index: None,
        },
      );
      if !Self::wakeup_cursor_insert_inner(wakeup_block) {
        return false;
      }
      (0, 0)
    };
    Self::set_wakeup_pointer(aaa_id, wakeup_block, page_id, slot)
  }

  fn set_wakeup_pointer(
    aaa_id: AaaId,
    block: BlockNumberFor<T>,
    page_id: WakeupPageId,
    slot: WakeupSlot,
  ) -> bool {
    let pointer = WakeupPointer {
      block,
      page_id,
      slot,
    };
    ActorHot::<T>::mutate(aaa_id, |maybe_hot| {
      if let Some(hot) = maybe_hot {
        hot.wakeup_pointer = Some(pointer);
      }
    });
    match MinWakeupBlock::<T>::get() {
      Some(current_min) if current_min <= block => {}
      _ => MinWakeupBlock::<T>::put(block),
    }
    true
  }

  pub fn wakeup_substrate_schedule(aaa_id: AaaId, wakeup_block: BlockNumberFor<T>) -> bool {
    let result: DispatchResult = polkadot_sdk::frame_support::storage::with_transaction(|| {
      if Self::wakeup_substrate_schedule_inner(aaa_id, wakeup_block) {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      } else {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          Error::<T>::AaaNotFound.into(),
        ))
      }
    });
    result.is_ok()
  }

  fn wakeup_substrate_drain_block_inner(
    wakeup_block: BlockNumberFor<T>,
    max_entries_scanned: u32,
  ) -> Option<(BoundedVec<AaaId, T::MaxWakeupsPerBlock>, WakeupDrainStats)> {
    let mut ready = BoundedVec::<AaaId, T::MaxWakeupsPerBlock>::default();
    let mut stats = WakeupDrainStats::default();
    let scan_limit = max_entries_scanned.min(T::MaxWakeupsPerBlock::get());
    if scan_limit == 0 {
      return Some((ready, stats));
    }
    let Some(mut bucket) = WakeupBuckets::<T>::get(wakeup_block) else {
      return Some((ready, stats));
    };
    if bucket.cursor_index.is_none() {
      return None;
    }
    let mut page_id = bucket.head_page;

    while stats.entries_scanned < scan_limit {
      let key = (wakeup_block, page_id);
      let Some(mut page) = WakeupPages::<T>::get(key) else {
        return None;
      };
      stats.pages_touched = stats.pages_touched.saturating_add(1);
      let mut slot = page.scan_slot as usize;
      while slot < page.entries.len() && stats.entries_scanned < scan_limit {
        let entry = page.entries[slot].take();
        page.scan_slot = (slot as WakeupSlot).saturating_add(1);
        stats.entries_scanned = stats.entries_scanned.saturating_add(1);
        slot = slot.saturating_add(1);
        let Some(entry) = entry else {
          continue;
        };
        page.live_entries = page.live_entries.saturating_sub(1);
        bucket.live_entries = bucket.live_entries.saturating_sub(1);
        let pointer = WakeupPointer {
          block: wakeup_block,
          page_id,
          slot: (slot - 1) as WakeupSlot,
        };
        let is_live =
          ActorHot::<T>::get(entry.aaa_id).and_then(|hot| hot.wakeup_pointer) == Some(pointer);
        if !is_live {
          stats.stale_entries = stats.stale_entries.saturating_add(1);
          continue;
        }
        if ready.try_push(entry.aaa_id).is_err() {
          page.entries[slot - 1] = Some(entry);
          page.live_entries = page.live_entries.saturating_add(1);
          bucket.live_entries = bucket.live_entries.saturating_add(1);
          page.scan_slot = (slot - 1) as WakeupSlot;
          stats.entries_scanned = stats.entries_scanned.saturating_sub(1);
          WakeupPages::<T>::insert(key, page);
          WakeupBuckets::<T>::insert(wakeup_block, bucket);
          return Some((ready, stats));
        }
        ActorHot::<T>::mutate(entry.aaa_id, |maybe_hot| {
          if let Some(hot) = maybe_hot
            && hot.wakeup_pointer == Some(pointer)
          {
            hot.wakeup_pointer = None;
          }
        });
        stats.ready_entries = stats.ready_entries.saturating_add(1);
      }

      if page.live_entries > 0 {
        WakeupPages::<T>::insert(key, page);
        WakeupBuckets::<T>::insert(wakeup_block, bucket);
        return Some((ready, stats));
      }

      let next_page = page.next_page;
      WakeupPages::<T>::remove(key);
      stats.pages_deleted = stats.pages_deleted.saturating_add(1);
      let Some(next_page) = next_page else {
        if !Self::wakeup_cursor_remove_inner(wakeup_block) {
          return None;
        }
        WakeupBuckets::<T>::remove(wakeup_block);
        return Some((ready, stats));
      };
      WakeupPages::<T>::mutate((wakeup_block, next_page), |maybe_next| {
        if let Some(next) = maybe_next {
          next.previous_page = None;
        }
      });
      bucket.head_page = next_page;
      WakeupBuckets::<T>::insert(wakeup_block, bucket);
      page_id = next_page;
    }
    Some((ready, stats))
  }

  pub fn wakeup_substrate_drain_block(
    wakeup_block: BlockNumberFor<T>,
    max_entries_scanned: u32,
  ) -> (BoundedVec<AaaId, T::MaxWakeupsPerBlock>, WakeupDrainStats) {
    let result: Result<_, DispatchError> =
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Self::wakeup_substrate_drain_block_inner(wakeup_block, max_entries_scanned) {
          Some(result) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(result))
          }
          None => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::AaaNotFound.into(),
          )),
        }
      });
    result.unwrap_or_default()
  }

  fn wakeup_cursor_page_and_slot(index: WakeupCursorIndex) -> (WakeupPageId, usize) {
    let page_size = T::WakeupPageSize::get().max(1);
    (u64::from(index / page_size), (index % page_size) as usize)
  }

  pub(crate) fn wakeup_cursor_get(index: WakeupCursorIndex) -> Option<BlockNumberFor<T>> {
    let (page_id, slot) = Self::wakeup_cursor_page_and_slot(index);
    WakeupCursorPages::<T>::get(page_id).and_then(|page| page.get(slot).copied())
  }

  fn wakeup_cursor_set(index: WakeupCursorIndex, block: BlockNumberFor<T>) -> bool {
    let (page_id, slot) = Self::wakeup_cursor_page_and_slot(index);
    let mut page = WakeupCursorPages::<T>::get(page_id).unwrap_or_default();
    if slot < page.len() {
      page[slot] = block;
    } else if slot == page.len() {
      if page.try_push(block).is_err() {
        return false;
      }
    } else {
      return false;
    }
    WakeupCursorPages::<T>::insert(page_id, page);
    true
  }

  fn wakeup_cursor_remove_tail(index: WakeupCursorIndex) -> bool {
    let (page_id, slot) = Self::wakeup_cursor_page_and_slot(index);
    let Some(mut page) = WakeupCursorPages::<T>::get(page_id) else {
      return false;
    };
    if slot.saturating_add(1) != page.len() {
      return false;
    }
    page.pop();
    if page.is_empty() {
      WakeupCursorPages::<T>::remove(page_id);
    } else {
      WakeupCursorPages::<T>::insert(page_id, page);
    }
    true
  }

  fn wakeup_cursor_swap(left: WakeupCursorIndex, right: WakeupCursorIndex) -> bool {
    let Some(left_block) = Self::wakeup_cursor_get(left) else {
      return false;
    };
    let Some(right_block) = Self::wakeup_cursor_get(right) else {
      return false;
    };
    if !Self::wakeup_cursor_set(left, right_block) || !Self::wakeup_cursor_set(right, left_block) {
      return false;
    }
    WakeupBuckets::<T>::mutate(right_block, |maybe_bucket| {
      if let Some(bucket) = maybe_bucket {
        bucket.cursor_index = Some(left);
      }
    });
    WakeupBuckets::<T>::mutate(left_block, |maybe_bucket| {
      if let Some(bucket) = maybe_bucket {
        bucket.cursor_index = Some(right);
      }
    });
    true
  }

  fn wakeup_cursor_height_bound() -> u32 {
    u32::BITS.saturating_sub(T::MaxActiveActors::get().max(1).leading_zeros())
  }

  fn wakeup_cursor_insert_inner(block: BlockNumberFor<T>) -> bool {
    let Some(mut bucket) = WakeupBuckets::<T>::get(block) else {
      return false;
    };
    if let Some(index) = bucket.cursor_index {
      return Self::wakeup_cursor_get(index) == Some(block);
    }
    let len = WakeupCursorLen::<T>::get();
    if len >= T::MaxActiveActors::get() || !Self::wakeup_cursor_set(len, block) {
      return false;
    }
    bucket.cursor_index = Some(len);
    WakeupBuckets::<T>::insert(block, bucket);
    WakeupCursorLen::<T>::put(len.saturating_add(1));
    let mut current = len;
    for _ in 0..Self::wakeup_cursor_height_bound() {
      if current == 0 {
        break;
      }
      let parent = current.saturating_sub(1) / 2;
      let Some(parent_block) = Self::wakeup_cursor_get(parent) else {
        return false;
      };
      let Some(current_block) = Self::wakeup_cursor_get(current) else {
        return false;
      };
      if parent_block <= current_block {
        break;
      }
      if !Self::wakeup_cursor_swap(parent, current) {
        return false;
      }
      current = parent;
    }
    true
  }

  pub fn wakeup_cursor_insert(block: BlockNumberFor<T>) -> bool {
    let result: DispatchResult = polkadot_sdk::frame_support::storage::with_transaction(|| {
      if Self::wakeup_cursor_insert_inner(block) {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      } else {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          Error::<T>::AaaNotFound.into(),
        ))
      }
    });
    result.is_ok()
  }

  pub fn wakeup_cursor_peek() -> Option<BlockNumberFor<T>> {
    (WakeupCursorLen::<T>::get() > 0)
      .then(|| Self::wakeup_cursor_get(0))
      .flatten()
  }

  fn wakeup_cursor_remove_inner(block: BlockNumberFor<T>) -> bool {
    let Some(index) = WakeupBuckets::<T>::get(block).and_then(|bucket| bucket.cursor_index) else {
      return false;
    };
    let len = WakeupCursorLen::<T>::get();
    if index >= len || Self::wakeup_cursor_get(index) != Some(block) {
      return false;
    }
    let last_index = len.saturating_sub(1);
    let Some(last_block) = Self::wakeup_cursor_get(last_index) else {
      return false;
    };
    if !Self::wakeup_cursor_remove_tail(last_index) {
      return false;
    }
    WakeupBuckets::<T>::mutate(block, |maybe_bucket| {
      if let Some(bucket) = maybe_bucket {
        bucket.cursor_index = None;
      }
    });
    WakeupCursorLen::<T>::put(last_index);
    if index == last_index {
      return true;
    }
    if !Self::wakeup_cursor_set(index, last_block) {
      return false;
    }
    WakeupBuckets::<T>::mutate(last_block, |maybe_bucket| {
      if let Some(bucket) = maybe_bucket {
        bucket.cursor_index = Some(index);
      }
    });

    let mut current = index;
    for _ in 0..Self::wakeup_cursor_height_bound() {
      if current == 0 {
        break;
      }
      let parent = current.saturating_sub(1) / 2;
      let Some(parent_block) = Self::wakeup_cursor_get(parent) else {
        return false;
      };
      let Some(current_block) = Self::wakeup_cursor_get(current) else {
        return false;
      };
      if parent_block <= current_block {
        break;
      }
      if !Self::wakeup_cursor_swap(parent, current) {
        return false;
      }
      current = parent;
    }
    if current != index {
      return true;
    }

    for _ in 0..Self::wakeup_cursor_height_bound() {
      let left = current.saturating_mul(2).saturating_add(1);
      if left >= last_index {
        break;
      }
      let right = left.saturating_add(1);
      let mut smallest = left;
      let Some(left_block) = Self::wakeup_cursor_get(left) else {
        return false;
      };
      if right < last_index {
        let Some(right_block) = Self::wakeup_cursor_get(right) else {
          return false;
        };
        if right_block < left_block {
          smallest = right;
        }
      }
      let Some(current_block) = Self::wakeup_cursor_get(current) else {
        return false;
      };
      let Some(smallest_block) = Self::wakeup_cursor_get(smallest) else {
        return false;
      };
      if current_block <= smallest_block {
        break;
      }
      if !Self::wakeup_cursor_swap(current, smallest) {
        return false;
      }
      current = smallest;
    }
    true
  }

  pub fn wakeup_cursor_remove(block: BlockNumberFor<T>) -> bool {
    let result: DispatchResult = polkadot_sdk::frame_support::storage::with_transaction(|| {
      if Self::wakeup_cursor_remove_inner(block) {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      } else {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          Error::<T>::AaaNotFound.into(),
        ))
      }
    });
    result.is_ok()
  }

  fn wakeup_cursor_pop_min_inner() -> Option<BlockNumberFor<T>> {
    let min_block = Self::wakeup_cursor_get(0)?;
    Self::wakeup_cursor_remove_inner(min_block).then_some(min_block)
  }

  pub fn wakeup_cursor_pop_min() -> Option<BlockNumberFor<T>> {
    let result: Result<BlockNumberFor<T>, DispatchError> =
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Self::wakeup_cursor_pop_min_inner() {
          Some(block) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(block))
          }
          None => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::AaaNotFound.into(),
          )),
        }
      });
    result.ok()
  }

  pub(crate) fn prime_actor_schedule(aaa_id: AaaId) {
    let Some(instance) = Self::active_actor_snapshot(aaa_id) else {
      return;
    };
    let now = frame_system::Pallet::<T>::block_number();
    match &instance.schedule.trigger {
      Trigger::Timer { .. } => Self::schedule_next_timer_wakeup(aaa_id, &instance, now),
      Trigger::Manual => {
        if instance.manual_trigger_pending {
          Self::enqueue(aaa_id);
        }
      }
      Trigger::OnAddressEvent { .. } => {
        if instance.manual_trigger_pending {
          Self::enqueue(aaa_id);
        }
      }
    }
  }

  fn defer_wakeup(aaa_id: AaaId, wakeup_block: BlockNumberFor<T>) -> bool {
    // Temporal wakeup layer: place future eligibility into bounded block buckets.
    let previous_block = ScheduledWakeupBlock::<T>::get(aaa_id);
    let mut target_block = wakeup_block;
    let mut scheduled_block: Option<BlockNumberFor<T>> = None;
    for _ in 0..=T::MaxSpilloverBlocks::get() {
      let inserted = WakeupIndex::<T>::mutate(target_block, |queue| {
        if queue.contains(&aaa_id) {
          return true;
        }
        queue.try_push(aaa_id).is_ok()
      });
      if inserted {
        scheduled_block = Some(target_block);
        break;
      }
      target_block = target_block.saturating_add(One::one());
    }
    let Some(scheduled_block) = scheduled_block else {
      WakeupRetryPending::<T>::insert(aaa_id, true);
      WakeupScheduleDrops::<T>::mutate(|drops| *drops = drops.saturating_add(1));
      Self::deposit_event(Event::WakeupScheduleDropped {
        aaa_id,
        requested_block: wakeup_block,
      });
      return false;
    };
    if previous_block != Some(scheduled_block) {
      if let Some(previous_block) = previous_block {
        Self::remove_wakeup_bucket_entry(previous_block, aaa_id);
      }
      ScheduledWakeupBlock::<T>::insert(aaa_id, scheduled_block);
    }
    if scheduled_block != wakeup_block {
      Self::deposit_event(Event::WakeupRescheduled {
        aaa_id,
        requested_block: wakeup_block,
        scheduled_block,
      });
    }
    WakeupRetryPending::<T>::remove(aaa_id);
    match MinWakeupBlock::<T>::get() {
      Some(current_min) if current_min <= scheduled_block => {}
      _ => MinWakeupBlock::<T>::put(scheduled_block),
    }
    true
  }

  pub(crate) fn remove_wakeup_bucket_entry(block: BlockNumberFor<T>, aaa_id: AaaId) {
    WakeupIndex::<T>::mutate_exists(block, |maybe_queue| {
      let Some(queue) = maybe_queue.as_mut() else {
        return;
      };
      queue.retain(|id| *id != aaa_id);
      if queue.is_empty() {
        *maybe_queue = None;
      }
    });
  }

  /// Baseline scheduler envelope reserved ahead of one actor run/close pair.
  ///
  /// Compatibility-ingress drains and heavyweight wakeup-retry or terminal
  /// sweep units remain independently metered durable carry-over work. They
  /// may defer actor execution in a saturated block and therefore do not form
  /// part of this same-block plan-admission guarantee.
  pub fn scheduler_admission_overhead() -> Weight {
    T::WeightInfo::scheduler_on_idle_base()
      .saturating_add(T::WeightInfo::compatibility_ingress_probe())
      .saturating_add(T::WeightInfo::scheduler_zombie_sweep_base())
      .saturating_add(
        T::WeightInfo::permissionless_sweep()
          .saturating_add(T::DbWeight::get().writes(1))
          .saturating_mul(u64::from(T::MaxSweepPerBlock::get())),
      )
      .saturating_add(T::WeightInfo::scheduler_paged_tombstone_drain(1))
      .saturating_add(
        T::WeightInfo::scheduler_paged_consume_preserve_page()
          .max(T::WeightInfo::scheduler_paged_consume_delete_page()),
      )
      .saturating_add(
        T::WeightInfo::scheduler_paged_append_existing_page()
          .max(T::WeightInfo::scheduler_paged_append_new_page()),
      )
      .saturating_add(Self::wakeup_cursor_weight())
      .saturating_add(Self::scheduler_probe_weight_upper())
  }

  /// Upper-bounds terminal state deletion after the close plan has run.
  ///
  /// Actor-local queue invalidation is O(1). The generated wakeup-bucket scan
  /// proxy covers removal from one full future bucket, while the fixed tail covers
  /// pointer/retry state, actor cardinality, reverse indexes, checkpoint, and event.
  pub(crate) fn close_cleanup_weight_upper() -> Weight {
    T::WeightInfo::scheduler_paged_consume_preserve_page()
      .saturating_add(Weight::from_parts(3_000_000_000, 170_000))
      .saturating_add(T::DbWeight::get().reads_writes(2, 2))
      .saturating_add(Weight::from_parts(10_000_000, 32_768))
      .saturating_add(T::DbWeight::get().reads_writes(9, 12))
  }

  pub fn wakeup_registration_weight_upper() -> Weight {
    Self::wakeup_retry_probe_weight_upper()
  }

  fn wakeup_retry_probe_weight_upper() -> Weight {
    T::WeightInfo::scheduler_wakeup_spillover_probe(T::MaxSpilloverBlocks::get().saturating_add(1))
  }

  pub fn scheduler_actor_probe_weight_upper() -> Weight {
    T::WeightInfo::scheduler_actor_probe()
  }

  fn scheduler_probe_weight_upper() -> Weight {
    Self::scheduler_actor_probe_weight_upper()
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_scheduler_actor_probe(aaa_id: AaaId) {
    let hot = ActorHot::<T>::get(aaa_id).expect("benchmark actor hot state must exist");
    let program = ActorProgram::<T>::get(aaa_id).expect("benchmark actor program state must exist");
    let instance = Self::compose_active_actor(hot, program);
    let meter = WeightMeter::with_limit(Weight::zero());
    let AdmissionDecision::Defer(reason) = Self::apply_admission(aaa_id, &instance, &meter) else {
      panic!("benchmark actor must defer on an exhausted cycle budget");
    };
    Self::deposit_event(Event::CycleDeferred { aaa_id, reason });
  }

  pub fn wakeup_drain_weight_upper(wakeups: u32) -> Weight {
    T::WeightInfo::scheduler_wakeup_dense_due_drain(wakeups)
  }

  fn wakeup_cursor_weight() -> Weight {
    Self::wakeup_drain_weight_upper(0)
  }

  fn wakeup_drain_actor_weight_upper() -> Weight {
    Self::wakeup_drain_weight_upper(1).saturating_add(Self::wakeup_retry_probe_weight_upper())
  }

  pub fn wakeup_cursor_drain_unit_weight_upper(removes_bucket: bool) -> Weight {
    let enqueue_or_reschedule = T::WeightInfo::scheduler_paged_append_new_page()
      .saturating_add(T::WeightInfo::scheduler_wakeup_append_new_page())
      .saturating_add(T::WeightInfo::scheduler_wakeup_cursor_insert());
    let mut weight =
      T::WeightInfo::scheduler_wakeup_drain_partial_page().saturating_add(enqueue_or_reschedule);
    if removes_bucket {
      weight = weight.saturating_add(T::WeightInfo::scheduler_wakeup_cursor_remove_exact());
    }
    weight
  }

  pub fn drain_overdue_wakeups_cursor(
    now: BlockNumberFor<T>,
    meter: &mut WeightMeter,
  ) -> WakeupDrainStats {
    let mut total = WakeupDrainStats::default();
    let mut current_block = None;
    let max_scans = T::MaxWakeupsPerBlock::get();
    while total.entries_scanned < max_scans {
      let block_cursor = if let Some(block) = current_block {
        block
      } else {
        let cursor_weight = T::WeightInfo::scheduler_wakeup_cursor_worker_future();
        if !meter.can_consume(cursor_weight) {
          break;
        }
        meter.consume(cursor_weight);
        let Some(block) = Self::wakeup_cursor_peek() else {
          break;
        };
        if block > now {
          break;
        }
        current_block = Some(block);
        block
      };
      let base_weight = Self::wakeup_cursor_drain_unit_weight_upper(false);
      if !meter.can_consume(base_weight) {
        break;
      }
      let Some(bucket) = WakeupBuckets::<T>::get(block_cursor) else {
        meter.consume(base_weight);
        break;
      };
      let unit_weight = Self::wakeup_cursor_drain_unit_weight_upper(bucket.live_entries <= 1);
      if !meter.can_consume(unit_weight) {
        meter.consume(base_weight);
        break;
      }
      meter.consume(unit_weight);
      let (ready, stats) = Self::wakeup_substrate_drain_block(block_cursor, 1);
      if stats.entries_scanned == 0 {
        break;
      }
      total.entries_scanned = total.entries_scanned.saturating_add(stats.entries_scanned);
      total.ready_entries = total.ready_entries.saturating_add(stats.ready_entries);
      total.stale_entries = total.stale_entries.saturating_add(stats.stale_entries);
      total.pages_touched = total.pages_touched.saturating_add(stats.pages_touched);
      total.pages_deleted = total.pages_deleted.saturating_add(stats.pages_deleted);
      for aaa_id in ready {
        Self::enqueue(aaa_id);
      }
      if !WakeupBuckets::<T>::contains_key(block_cursor) {
        current_block = None;
      }
    }
    total
  }

  fn drain_overdue_wakeups_paged(now: BlockNumberFor<T>, meter: &mut WeightMeter) {
    let cursor_weight = Self::wakeup_cursor_weight();
    if !meter.can_consume(cursor_weight) {
      return;
    }
    meter.consume(cursor_weight);
    let mut processed = 0u32;
    let max_wakeups = T::MaxWakeupsPerBlock::get();
    let mut scanned_blocks = 0u32;
    let mut cursor = MinWakeupBlock::<T>::get();
    while processed < max_wakeups && scanned_blocks < max_wakeups {
      let Some(block_cursor) = cursor else {
        break;
      };
      if block_cursor > now {
        break;
      }
      let bucket_weight = T::WeightInfo::scheduler_wakeup_dense_due_drain(0);
      if !meter.can_consume(bucket_weight) {
        break;
      }
      meter.consume(bucket_weight);
      scanned_blocks = scanned_blocks.saturating_add(1);
      let actors = WakeupIndex::<T>::take(block_cursor).into_inner();
      if actors.is_empty() {
        cursor = Some(block_cursor.saturating_add(One::one()));
        continue;
      }
      let actor_weight = Self::wakeup_drain_actor_weight_upper();
      let mut index = 0usize;
      while index < actors.len() && processed < max_wakeups {
        if !meter.can_consume(actor_weight) {
          break;
        }
        meter.consume(actor_weight);
        let aaa_id = actors[index];
        processed = processed.saturating_add(1);
        index = index.saturating_add(1);
        if ScheduledWakeupBlock::<T>::get(aaa_id) != Some(block_cursor) {
          continue;
        }
        ScheduledWakeupBlock::<T>::remove(aaa_id);
        Self::enqueue(aaa_id);
      }
      if index < actors.len() {
        WakeupIndex::<T>::insert(
          block_cursor,
          BoundedVec::<AaaId, T::MaxWakeupBucketSize>::truncate_from(actors[index..].to_vec()),
        );
        cursor = Some(block_cursor);
        break;
      }
      cursor = Some(block_cursor.saturating_add(One::one()));
    }
    match cursor {
      Some(block_cursor) => MinWakeupBlock::<T>::put(block_cursor),
      None => MinWakeupBlock::<T>::kill(),
    }
  }

  fn timer_jitter_blocks(aaa_id: AaaId, every_blocks: u32) -> BlockNumberFor<T> {
    let window = every_blocks
      .saturating_div(4)
      .min(T::MaxTimerJitterBlocks::get());
    if window == 0 {
      return Zero::zero();
    }
    let hash = frame::hashing::blake2_256(&aaa_id.encode());
    let raw = u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]);
    (raw % window).into()
  }

  pub(crate) fn initial_eligible_at(
    aaa_id: AaaId,
    schedule: &ScheduleOf<T>,
    schedule_window: Option<ScheduleWindow<BlockNumberFor<T>>>,
    now: BlockNumberFor<T>,
  ) -> BlockNumberFor<T> {
    let mut eligible_at = schedule_window
      .map(|window| now.max(window.start))
      .unwrap_or(now);
    if let Trigger::Timer { every_blocks } = schedule.trigger
      && every_blocks > 1
    {
      eligible_at = eligible_at.max(
        now
          .saturating_add(every_blocks.into())
          .saturating_add(Self::timer_jitter_blocks(aaa_id, every_blocks)),
      );
    }
    eligible_at
  }

  fn next_eligible_at(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    now: BlockNumberFor<T>,
    include_timer: bool,
  ) -> BlockNumberFor<T> {
    if instance.cycle_nonce == 0 {
      if include_timer {
        return now.max(instance.first_eligible_at);
      }
      return instance
        .schedule_window
        .map(|window| now.max(window.start))
        .unwrap_or(now);
    }
    let mut eligible_at = now;
    if let Some(window) = instance.schedule_window {
      eligible_at = eligible_at.max(window.start);
    }
    if instance.cycle_nonce < u64::MAX {
      let cooldown: BlockNumberFor<T> = instance.schedule.cooldown_blocks.into();
      eligible_at = eligible_at.max(instance.last_cycle_block.saturating_add(cooldown));
    }
    if include_timer && instance.cycle_nonce < u64::MAX {
      if let Trigger::Timer { every_blocks } = instance.schedule.trigger {
        let cadence: BlockNumberFor<T> = every_blocks.into();
        let jitter = Self::timer_jitter_blocks(aaa_id, every_blocks);
        eligible_at = eligible_at.max(
          instance
            .last_cycle_block
            .saturating_add(cadence)
            .saturating_add(jitter),
        );
      }
    }
    eligible_at
  }

  fn preserve_skipped_readiness(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    now: BlockNumberFor<T>,
    periodic_continuations: &mut Vec<AaaId>,
  ) {
    if instance.lifecycle.is_paused() {
      return;
    }
    if matches!(instance.schedule.trigger, Trigger::Timer { .. }) {
      Self::schedule_next_timer_wakeup_local(aaa_id, instance, now, periodic_continuations);
      return;
    }
    let pending = instance.manual_trigger_pending;
    if !pending {
      return;
    }
    let eligibility_block = Self::next_eligible_at(aaa_id, instance, now, false);
    if eligibility_block > now {
      Self::defer_wakeup(aaa_id, eligibility_block);
    } else {
      periodic_continuations.push(aaa_id);
    }
  }

  fn schedule_next_timer_wakeup_local(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    now: BlockNumberFor<T>,
    periodic_continuations: &mut Vec<AaaId>,
  ) {
    let Trigger::Timer { .. } = instance.schedule.trigger else {
      return;
    };
    let eligible_at = Self::next_eligible_at(aaa_id, instance, now, true);
    if eligible_at <= now.saturating_add(One::one()) {
      periodic_continuations.push(aaa_id);
    } else {
      Self::defer_wakeup(aaa_id, eligible_at);
    }
  }

  fn schedule_next_timer_wakeup(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    now: BlockNumberFor<T>,
  ) {
    let Trigger::Timer { .. } = instance.schedule.trigger else {
      return;
    };
    let eligible_at = Self::next_eligible_at(aaa_id, instance, now, true);
    if eligible_at <= now.saturating_add(One::one()) {
      Self::enqueue(aaa_id);
    } else {
      Self::defer_wakeup(aaa_id, eligible_at);
    }
  }

  pub(crate) fn is_window_expired(instance: &AaaInstanceOf<T>) -> bool {
    let now = frame_system::Pallet::<T>::block_number();
    instance
      .schedule_window
      .map(|window| now > window.end)
      .unwrap_or(false)
  }

  pub(crate) fn user_native_balance(instance: &AaaInstanceOf<T>) -> T::Balance {
    let native = T::NativeAssetId::get();
    T::AssetOps::balance(&instance.sovereign_account, native)
  }

  pub(crate) fn liveness_close_reason(instance: &AaaInstanceOf<T>) -> Option<CloseReason> {
    if Self::is_window_expired(instance) {
      return Some(CloseReason::WindowExpired);
    }
    if instance.lifecycle.is_paused() {
      return None;
    }
    Self::user_resource_close_reason(instance, false)
  }

  // Deterministic pre-cycle User precedence is BalanceExhausted, then
  // FeeBudgetExhausted. WindowExpired is handled by the caller first.
  fn user_resource_close_reason(
    instance: &AaaInstanceOf<T>,
    include_fee_budget: bool,
  ) -> Option<CloseReason> {
    if instance.actor_class.aaa_type() != AaaType::User {
      return None;
    }
    let native_balance = Self::user_native_balance(instance);
    if native_balance < T::MinUserBalance::get() {
      return Some(CloseReason::BalanceExhausted);
    }
    if include_fee_budget && native_balance < Self::cycle_fee_upper_bound(instance) {
      return Some(CloseReason::FeeBudgetExhausted);
    }
    None
  }

  fn close_within_budget(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    reason: CloseReason,
    meter: &WeightMeter,
  ) -> AdmissionDecision {
    let close_weight_upper = Self::close_cycle_weight_upper_bound(instance);
    if !meter.can_consume(close_weight_upper) {
      return AdmissionDecision::Defer(DeferReason::InsufficientWeightBudget);
    }
    match Self::close_actor(aaa_id, instance, reason) {
      Ok(()) => AdmissionDecision::Closed(close_weight_upper),
      Err(_) => AdmissionDecision::CloseFailed(close_weight_upper),
    }
  }

  fn pending_post_cycle_close_reason(instance: &AaaInstanceOf<T>) -> Option<CloseReason> {
    if Self::failure_limit_reached(instance.consecutive_failures) {
      return Some(CloseReason::ConsecutiveFailures);
    }
    instance
      .auto_close_at_cycle_nonce
      .filter(|target| instance.cycle_nonce >= *target)
      .map(|_| CloseReason::AutoCloseNonceReached)
  }

  fn cycle_may_close_on_failure(instance: &AaaInstanceOf<T>) -> bool {
    Self::failure_limit_reached(instance.consecutive_failures.saturating_add(1))
  }

  fn cycle_may_auto_close_on_success(instance: &AaaInstanceOf<T>) -> bool {
    instance
      .auto_close_at_cycle_nonce
      .map(|target| instance.cycle_nonce.saturating_add(1) >= target)
      .unwrap_or(false)
  }

  fn cycle_requires_close_tail_budget(instance: &AaaInstanceOf<T>) -> bool {
    Self::cycle_may_close_on_failure(instance) || Self::cycle_may_auto_close_on_success(instance)
  }

  fn cycle_admission_weight_upper(instance: &AaaInstanceOf<T>) -> Weight {
    let mut weight = Self::cycle_weight_upper_bound(instance);
    if Self::cycle_requires_close_tail_budget(instance) {
      weight = weight.saturating_add(Self::close_cycle_weight_upper_bound(instance));
    }
    weight
  }

  fn apply_admission(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    meter: &WeightMeter,
  ) -> AdmissionDecision {
    if GlobalCircuitBreaker::<T>::get() {
      return AdmissionDecision::Skip;
    }
    if Self::is_window_expired(instance) {
      return Self::close_within_budget(aaa_id, instance, CloseReason::WindowExpired, meter);
    }
    if instance.lifecycle.is_paused() {
      return AdmissionDecision::Skip;
    }
    if instance.actor_class.aaa_type() == AaaType::User && instance.cycle_nonce == u64::MAX {
      return Self::close_within_budget(aaa_id, instance, CloseReason::CycleNonceExhausted, meter);
    }
    if let Some(reason) = Self::pending_post_cycle_close_reason(instance) {
      return Self::close_within_budget(aaa_id, instance, reason, meter);
    }
    if !Self::is_ready_for_execution(aaa_id, instance) {
      return AdmissionDecision::Skip;
    }
    if let Some(reason) = Self::user_resource_close_reason(instance, true) {
      return Self::close_within_budget(aaa_id, instance, reason, meter);
    }
    let cycle_weight_upper = Self::cycle_admission_weight_upper(instance);
    if !meter.can_consume(cycle_weight_upper) {
      return AdmissionDecision::Defer(DeferReason::InsufficientWeightBudget);
    }
    AdmissionDecision::Admit(cycle_weight_upper)
  }

  fn is_ready_for_execution(aaa_id: AaaId, instance: &AaaInstanceOf<T>) -> bool {
    if instance.lifecycle.is_paused() {
      return false;
    }
    if GlobalCircuitBreaker::<T>::get() {
      return false;
    }
    let now = frame_system::Pallet::<T>::block_number();
    let include_timer = !instance.manual_trigger_pending
      && matches!(instance.schedule.trigger, Trigger::Timer { .. });
    if Self::next_eligible_at(aaa_id, instance, now, include_timer) > now {
      return false;
    }
    Self::evaluate_trigger(aaa_id, instance)
  }

  fn evaluate_trigger(aaa_id: AaaId, instance: &AaaInstanceOf<T>) -> bool {
    if instance.manual_trigger_pending {
      return true;
    }
    match instance.schedule.trigger {
      Trigger::Manual => false,
      Trigger::Timer { .. } => Self::evaluate_timer(aaa_id, instance),
      Trigger::OnAddressEvent { .. } => false,
    }
  }

  fn evaluate_timer(aaa_id: AaaId, instance: &AaaInstanceOf<T>) -> bool {
    let now = frame_system::Pallet::<T>::block_number();
    Self::next_eligible_at(aaa_id, instance, now, true) <= now
  }

  fn source_matches_filter(
    filter: &SourceFilterOf<T>,
    owner: &T::AccountId,
    source: Option<&T::AccountId>,
  ) -> bool {
    match (filter, source) {
      (SourceFilter::Any, _) => true,
      (SourceFilter::OwnerOnly, Some(who)) => who == owner,
      (SourceFilter::OwnerOnly, None) => false,
      (SourceFilter::Whitelist(list), Some(who)) => list.iter().any(|a| a == who),
      (SourceFilter::Whitelist(_), None) => false,
    }
  }

  fn asset_matches_filter(filter: &AssetFilterOf<T>, asset: T::AssetId) -> bool {
    match filter {
      AssetFilter::Any => true,
      AssetFilter::Whitelist(list) => list.iter().any(|id| *id == asset),
    }
  }

  pub fn notify_address_event(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    source: &T::AccountId,
  ) -> DispatchResult {
    let provenance = FundingProvenance::Signed(source.clone());
    Self::notify_address_event_with_provenance(aaa_id, asset, amount, Some(&provenance))
  }

  pub fn notify_internal_address_event(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    source: &T::AccountId,
  ) -> DispatchResult {
    let provenance = FundingProvenance::InternalProtocol(source.clone());
    Self::notify_address_event_with_provenance(aaa_id, asset, amount, Some(&provenance))
  }

  pub fn notify_xcm_address_event(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    source: &T::AccountId,
  ) -> DispatchResult {
    let provenance = FundingProvenance::Xcm(source.clone());
    Self::notify_address_event_with_provenance(aaa_id, asset, amount, Some(&provenance))
  }

  pub fn notify_address_event_without_source(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
  ) -> DispatchResult {
    Self::notify_address_event_with_provenance(aaa_id, asset, amount, None)
  }

  pub fn queue_address_event(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    provenance: Option<FundingProvenance<T::AccountId>>,
  ) -> bool {
    if amount == Zero::zero() {
      return true;
    }
    let Some(instance) = Self::active_actor_snapshot(aaa_id) else {
      return true;
    };
    if Self::is_window_expired(&instance) {
      return true;
    }
    let capacity = T::MaxIngressOverflowQueue::get();
    if capacity == 0 {
      return false;
    }
    let len = IngressOverflowLen::<T>::get();
    if len >= capacity
      || Self::preflight_funding_event(aaa_id, asset, amount, provenance.as_ref()).is_err()
    {
      return false;
    }
    let head = IngressOverflowHead::<T>::get() % capacity;
    let tail = (head.saturating_add(len)) % capacity;
    let event = IngressOverflowEvent {
      aaa_id,
      asset,
      amount,
      provenance,
    };
    if Self::apply_address_event_parts(
      aaa_id,
      asset,
      amount,
      event.provenance.as_ref(),
      false,
      true,
    )
    .is_err()
    {
      return false;
    }
    IngressOverflowSlots::<T>::insert(tail, event);
    IngressOverflowLen::<T>::put(len.saturating_add(1));
    true
  }

  pub fn drain_address_event_overflow(max_events: u32) -> u32 {
    if max_events == 0 {
      return 0;
    }
    let capacity = T::MaxIngressOverflowQueue::get();
    if capacity == 0 {
      return 0;
    }
    let mut drained = 0u32;
    let mut head = IngressOverflowHead::<T>::get() % capacity;
    let mut len = IngressOverflowLen::<T>::get();
    while drained < max_events && len > 0 {
      let Some(event) = IngressOverflowSlots::<T>::take(head) else {
        head = (head.saturating_add(1)) % capacity;
        len = len.saturating_sub(1);
        continue;
      };
      let _ = Self::apply_address_event_parts(
        event.aaa_id,
        event.asset,
        event.amount,
        event.provenance.as_ref(),
        true,
        false,
      );
      head = (head.saturating_add(1)) % capacity;
      len = len.saturating_sub(1);
      drained = drained.saturating_add(1);
    }
    IngressOverflowHead::<T>::put(head);
    IngressOverflowLen::<T>::put(len);
    drained
  }

  fn funding_event_authorized(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    funding: &ActorFundingStateOf<T>,
    provenance: Option<&FundingProvenance<T::AccountId>>,
  ) -> bool {
    provenance.is_some_and(|provenance| match &funding.funding_source_policy {
      FundingSourcePolicy::OwnerOnly => matches!(
        provenance,
        FundingProvenance::Signed(source) if source == &instance.owner
      ),
      FundingSourcePolicy::SignedAllowlist(allowed) => matches!(
        provenance,
        FundingProvenance::Signed(source) if allowed.contains(source)
      ),
      FundingSourcePolicy::RuntimePolicy => {
        T::FundingAuthority::allows(aaa_id, &instance.owner, provenance)
      }
      FundingSourcePolicy::AnySource => true,
    })
  }

  pub fn preflight_funding_event(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    provenance: Option<&FundingProvenance<T::AccountId>>,
  ) -> DispatchResult {
    let Some(instance) = Self::active_actor_snapshot(aaa_id) else {
      return Ok(());
    };
    if Self::is_window_expired(&instance) || amount.is_zero() {
      return Ok(());
    }
    let funding = ActorFunding::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
    if !Self::funding_event_authorized(aaa_id, &instance, &funding, provenance)
      || !funding.funding_tracked_assets.contains(&asset)
    {
      return Ok(());
    }
    if let Some(batch) = funding.funding_snapshots.get(&asset) {
      ensure!(
        batch.pending_amount.checked_add(&amount).is_some(),
        Error::<T>::FundingBatchOverflow
      );
    }
    Ok(())
  }

  fn notify_address_event_with_provenance(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    provenance: Option<&FundingProvenance<T::AccountId>>,
  ) -> DispatchResult {
    Self::preflight_funding_event(aaa_id, asset, amount, provenance)?;
    polkadot_sdk::frame_support::storage::with_transaction(
      || match Self::apply_address_event_parts(aaa_id, asset, amount, provenance, true, true) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      },
    )
  }

  fn apply_address_event_parts(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    provenance: Option<&FundingProvenance<T::AccountId>>,
    apply_trigger: bool,
    apply_funding: bool,
  ) -> DispatchResult {
    let instance = match Self::active_actor_snapshot(aaa_id) {
      Some(inst) => inst,
      None => return Ok(()),
    };
    if Self::is_window_expired(&instance) {
      return Ok(());
    }
    let mut inbox_matched = false;
    if apply_trigger
      && let Trigger::OnAddressEvent {
        source_filter,
        asset_filter,
      } = &instance.schedule.trigger
    {
      if Self::source_matches_filter(
        source_filter,
        &instance.owner,
        provenance.map(FundingProvenance::account),
      ) && Self::asset_matches_filter(asset_filter, asset)
      {
        inbox_matched = true;
        if !instance.manual_trigger_pending {
          ActorHot::<T>::mutate(aaa_id, |maybe_hot| {
            if let Some(hot) = maybe_hot {
              hot.pending_signal = true;
            }
          });
        }
      }
    }
    if apply_funding && amount > Zero::zero() {
      let mut funding = ActorFunding::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      if Self::funding_event_authorized(aaa_id, &instance, &funding, provenance)
        && funding.funding_tracked_assets.contains(&asset)
      {
        if let Some(batch) = funding.funding_snapshots.get_mut(&asset) {
          let pending_amount = batch
            .pending_amount
            .checked_add(&amount)
            .ok_or(Error::<T>::FundingBatchOverflow)?;
          batch.pending_amount = pending_amount;
          funding.has_pending_funding = true;
          Self::deposit_event(Event::FundingBatchPendingAccumulated {
            aaa_id,
            asset,
            added: amount,
            pending_amount,
          });
        } else {
          funding
            .funding_snapshots
            .try_insert(
              asset,
              FundingBatch {
                amount,
                pending_amount: Zero::zero(),
              },
            )
            .map_err(|_| Error::<T>::FundingBatchOverflow)?;
          Self::deposit_event(Event::FundingBatchActivated {
            aaa_id,
            asset,
            amount,
          });
        }
        ActorFunding::<T>::insert(aaa_id, funding);
      }
    }
    if inbox_matched {
      Self::enqueue(aaa_id);
    }
    Ok(())
  }

  pub(crate) fn execute_zombie_sweep(remaining_weight: Weight) -> Weight {
    let max_check = T::MaxSweepPerBlock::get();
    if remaining_weight.is_zero() {
      return Weight::zero();
    }
    let mut meter = WeightMeter::with_limit(remaining_weight);
    let base_weight = T::WeightInfo::scheduler_zombie_sweep_base();
    if !meter.can_consume(base_weight) {
      return Weight::zero();
    }
    meter.consume(base_weight);
    let max_id = NextAaaId::<T>::get();
    if max_id == 0 {
      return meter.consumed();
    }
    let mut cursor = SweepCursor::<T>::get();
    let iteration_weight =
      T::WeightInfo::permissionless_sweep().saturating_add(T::DbWeight::get().writes(1));
    let retry_weight =
      Self::scheduler_probe_weight_upper().saturating_add(Self::wakeup_retry_probe_weight_upper());
    let now = frame_system::Pallet::<T>::block_number();
    let mut retry_limit = 0u32;
    let mut admitted_retry_weight = Weight::zero();
    while retry_limit < max_check {
      let next_weight = admitted_retry_weight.saturating_add(retry_weight);
      if !meter.can_consume(next_weight) {
        break;
      }
      admitted_retry_weight = next_weight;
      retry_limit = retry_limit.saturating_add(1);
    }
    let retry_ids = WakeupRetryPending::<T>::iter_keys()
      .take(retry_limit as usize)
      .collect::<Vec<_>>();
    for aaa_id in &retry_ids {
      if ActorHot::<T>::contains_key(*aaa_id) {
        let _ = Self::defer_wakeup(*aaa_id, now.saturating_add(One::one()));
      } else {
        WakeupRetryPending::<T>::remove(aaa_id);
      }
      meter.consume(retry_weight);
    }
    let mut checked = retry_ids.len() as u32;
    while checked < max_check {
      if !meter.can_consume(iteration_weight) {
        break;
      }
      let next_cursor = (cursor + 1) % max_id;
      if let Some(instance) = Self::active_actor_snapshot(next_cursor) {
        if let Some(reason) = Self::liveness_close_reason(&instance) {
          let close_weight_upper = Self::close_cycle_weight_upper_bound(&instance);
          let required_weight = iteration_weight.saturating_add(close_weight_upper);
          if !meter.can_consume(required_weight) {
            break;
          }
          match Self::close_actor(next_cursor, &instance, reason) {
            Ok(()) => {
              cursor = next_cursor;
              SweepCursor::<T>::put(cursor);
              checked = checked.saturating_add(1);
              meter.consume(required_weight);
              continue;
            }
            Err(_) => {
              Self::deposit_event(Event::CycleDeferred {
                aaa_id: next_cursor,
                reason: DeferReason::CloseTransitionFailed,
              });
              meter.consume(required_weight);
              break;
            }
          }
        }
      }
      cursor = next_cursor;
      SweepCursor::<T>::put(cursor);
      checked = checked.saturating_add(1);
      meter.consume(iteration_weight);
    }
    meter.consumed()
  }

  pub(crate) fn evaluate_actor_liveness(aaa_id: AaaId) -> DispatchResult {
    let instance = Self::active_actor_snapshot(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
    if let Some(reason) = Self::liveness_close_reason(&instance) {
      return Self::close_actor(aaa_id, &instance, reason);
    }
    Ok(())
  }
}
