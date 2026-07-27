use crate::pallet::*;
use crate::types::{DirtyObservationState, ObservationRevision};
use crate::weights::WeightInfo as _;
use polkadot_sdk::frame_support::{ensure, storage::TransactionOutcome, traits::Get};
use polkadot_sdk::sp_runtime::{DispatchError, DispatchResult};

impl<T: Config> Pallet<T> {
  fn maximum_dirty_observation_feeds() -> Result<u32, DispatchError> {
    T::MaxActiveActors::get()
      .checked_mul(T::MaxTriggerSources::get())
      .ok_or(Error::<T>::DirtyObservationCapacityExceeded.into())
  }

  fn dirty_observation_page_size() -> Result<u32, DispatchError> {
    let size = T::QueuePageSize::get();
    ensure!(size > 0, Error::<T>::DirtyObservationInvariant);
    Ok(size)
  }

  fn allocate_dirty_observation_slot(feed: T::ObservationFeedId) -> Result<u32, DispatchError> {
    ensure!(
      !DirtyObservationFeeds::<T>::contains_key(feed),
      Error::<T>::DirtyObservationInvariant
    );
    let page_size = Self::dirty_observation_page_size()?;
    let free_len = DirtyObservationFreeSlotLen::<T>::get();
    let slot = if free_len > 0 {
      let index = free_len - 1;
      let page_id = index / page_size;
      let mut page = DirtyObservationFreeSlotPages::<T>::get(page_id)
        .ok_or(Error::<T>::DirtyObservationInvariant)?;
      let slot = page.pop().ok_or(Error::<T>::DirtyObservationInvariant)?;
      if page.is_empty() {
        DirtyObservationFreeSlotPages::<T>::remove(page_id);
      } else {
        DirtyObservationFreeSlotPages::<T>::insert(page_id, page);
      }
      DirtyObservationFreeSlotLen::<T>::put(index);
      slot
    } else {
      let slot = NextDirtyObservationSlot::<T>::get();
      ensure!(
        slot < Self::maximum_dirty_observation_feeds()?,
        Error::<T>::DirtyObservationCapacityExceeded
      );
      NextDirtyObservationSlot::<T>::put(
        slot
          .checked_add(1)
          .ok_or(Error::<T>::DirtyObservationCapacityExceeded)?,
      );
      slot
    };
    ensure!(
      !DirtyObservationSlotFeed::<T>::contains_key(slot),
      Error::<T>::DirtyObservationInvariant
    );
    DirtyObservationSlotFeed::<T>::insert(slot, feed);
    Ok(slot)
  }

  fn do_note_observation_changed(
    feed: T::ObservationFeedId,
    revision: ObservationRevision,
  ) -> DispatchResult {
    ensure!(revision > 0, Error::<T>::InvalidObservationRevision);
    if ObservationSubscriberCount::<T>::get(feed) == 0 {
      ensure!(
        !DirtyObservationFeeds::<T>::contains_key(feed),
        Error::<T>::DirtyObservationInvariant
      );
      return Ok(());
    }
    if let Some(mut state) = DirtyObservationFeeds::<T>::get(feed) {
      ensure!(
        revision >= state.latest_revision,
        Error::<T>::InvalidObservationRevision
      );
      if revision > state.latest_revision {
        state.latest_revision = revision;
        DirtyObservationFeeds::<T>::insert(feed, state);
      }
      return Ok(());
    }
    ensure!(
      DirtyObservationFeedCount::<T>::get() < Self::maximum_dirty_observation_feeds()?,
      Error::<T>::DirtyObservationCapacityExceeded
    );
    let slot = Self::allocate_dirty_observation_slot(feed)?;
    DirtyObservationFeeds::<T>::insert(
      feed,
      DirtyObservationState {
        slot,
        latest_revision: revision,
        fanout_revision: 0,
        next_subscriber_page: 0,
      },
    );
    DirtyObservationFeedCount::<T>::try_mutate(|count| -> DispatchResult {
      *count = count
        .checked_add(1)
        .ok_or(Error::<T>::DirtyObservationCapacityExceeded)?;
      Ok(())
    })
  }

  pub fn observation_change_ingress_weight() -> polkadot_sdk::frame_support::weights::Weight {
    T::WeightInfo::observation_change_ingress()
  }

  /// Coalesces one changed feed revision without reading subscribers or executing actors.
  pub fn note_observation_changed(
    feed: T::ObservationFeedId,
    revision: ObservationRevision,
  ) -> DispatchResult {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      match Self::do_note_observation_changed(feed, revision) {
        Ok(()) => TransactionOutcome::Commit(Ok(())),
        Err(error) => TransactionOutcome::Rollback(Err(error)),
      }
    })
  }

  pub(crate) fn preflight_clear_dirty_observation_feeds(
    feeds: &[T::ObservationFeedId],
  ) -> DispatchResult {
    let mut release_count = 0u32;
    for feed in feeds {
      if ObservationSubscriberCount::<T>::get(feed) != 1 {
        continue;
      }
      if let Some(state) = DirtyObservationFeeds::<T>::get(feed) {
        ensure!(
          DirtyObservationSlotFeed::<T>::get(state.slot) == Some(*feed),
          Error::<T>::DirtyObservationInvariant
        );
        release_count = release_count
          .checked_add(1)
          .ok_or(Error::<T>::DirtyObservationInvariant)?;
      }
    }
    if release_count == 0 {
      return Ok(());
    }
    let page_size = Self::dirty_observation_page_size()?;
    let free_len = DirtyObservationFreeSlotLen::<T>::get();
    let next_slot = NextDirtyObservationSlot::<T>::get();
    let next_free_len = free_len
      .checked_add(release_count)
      .ok_or(Error::<T>::DirtyObservationInvariant)?;
    ensure!(
      next_free_len <= next_slot,
      Error::<T>::DirtyObservationInvariant
    );
    let first_page = free_len / page_size;
    let last_page = (next_free_len - 1) / page_size;
    for page_id in first_page..=last_page {
      let expected_len = if page_id == first_page {
        free_len % page_size
      } else {
        0
      };
      let actual_len =
        DirtyObservationFreeSlotPages::<T>::get(page_id).map_or(0, |page| page.len() as u32);
      ensure!(
        actual_len == expected_len,
        Error::<T>::DirtyObservationInvariant
      );
    }
    Ok(())
  }

  pub(crate) fn clear_dirty_observation_feed(feed: T::ObservationFeedId) -> DispatchResult {
    let Some(state) = DirtyObservationFeeds::<T>::take(feed) else {
      return Ok(());
    };
    ensure!(
      DirtyObservationSlotFeed::<T>::take(state.slot) == Some(feed),
      Error::<T>::DirtyObservationInvariant
    );
    let page_size = Self::dirty_observation_page_size()?;
    let free_len = DirtyObservationFreeSlotLen::<T>::get();
    ensure!(
      free_len < NextDirtyObservationSlot::<T>::get(),
      Error::<T>::DirtyObservationInvariant
    );
    let page_id = free_len / page_size;
    let mut page = DirtyObservationFreeSlotPages::<T>::get(page_id).unwrap_or_default();
    page
      .try_push(state.slot)
      .map_err(|_| Error::<T>::DirtyObservationInvariant)?;
    DirtyObservationFreeSlotPages::<T>::insert(page_id, page);
    DirtyObservationFreeSlotLen::<T>::put(
      free_len
        .checked_add(1)
        .ok_or(Error::<T>::DirtyObservationInvariant)?,
    );
    DirtyObservationFeedCount::<T>::try_mutate(|count| -> DispatchResult {
      ensure!(*count > 0, Error::<T>::DirtyObservationInvariant);
      *count -= 1;
      Ok(())
    })
  }

  fn signal_observation_subscriber(aaa_id: AaaId) -> bool {
    let mut exists = false;
    let mut has_admission_path = false;
    ActorHot::<T>::mutate(aaa_id, |maybe| {
      if let Some(hot) = maybe {
        hot.pending_signal = true;
        exists = true;
        has_admission_path = hot.queue_ticket.is_some() || hot.wakeup_pointer.is_some();
      }
    });
    !exists || has_admission_path || Self::paged_enqueue(aaa_id)
  }

  pub(crate) fn dirty_observation_fanout_base_probe() -> u32 {
    DirtyObservationFeedCount::<T>::get()
  }

  pub(crate) fn do_fanout_dirty_observation_page() -> Result<bool, DispatchError> {
    let dirty_count = DirtyObservationFeedCount::<T>::get();
    if dirty_count == 0 {
      return Ok(false);
    }
    let next_slot = NextDirtyObservationSlot::<T>::get();
    ensure!(next_slot > 0, Error::<T>::DirtyObservationInvariant);
    let cursor = DirtyObservationScanSlot::<T>::get();
    let slot = if cursor < next_slot { cursor } else { 0 };
    let next_cursor = slot.saturating_add(1) % next_slot;
    let Some(feed) = DirtyObservationSlotFeed::<T>::get(slot) else {
      DirtyObservationScanSlot::<T>::put(next_cursor);
      return Ok(true);
    };
    let mut state =
      DirtyObservationFeeds::<T>::get(feed).ok_or(Error::<T>::DirtyObservationInvariant)?;
    ensure!(state.slot == slot, Error::<T>::DirtyObservationInvariant);
    let page_size = Self::dirty_observation_page_size()?;
    let subscriber_page_count = NextObservationSubscriptionSlot::<T>::get().div_ceil(page_size);
    ensure!(
      subscriber_page_count > 0 && state.next_subscriber_page < subscriber_page_count,
      Error::<T>::DirtyObservationInvariant
    );
    if state.fanout_revision == 0 {
      state.fanout_revision = state.latest_revision;
    }
    let page_id = state.next_subscriber_page;
    let mut page_complete = true;
    if let Some(page) = ObservationSubscriberPages::<T>::get(feed, page_id) {
      for aaa_id in page.into_iter().flatten() {
        // deos-bypass: bounded-iter — QueuePageSize bounds one fanout unit.
        page_complete &= Self::signal_observation_subscriber(aaa_id);
      }
    }
    if !page_complete {
      DirtyObservationFeeds::<T>::insert(feed, state);
      DirtyObservationScanSlot::<T>::put(next_cursor);
      return Ok(true);
    }
    state.next_subscriber_page = state
      .next_subscriber_page
      .checked_add(1)
      .ok_or(Error::<T>::DirtyObservationInvariant)?;
    if state.next_subscriber_page == subscriber_page_count {
      if state.latest_revision == state.fanout_revision {
        Self::clear_dirty_observation_feed(feed)?;
      } else {
        state.fanout_revision = state.latest_revision;
        state.next_subscriber_page = 0;
        DirtyObservationFeeds::<T>::insert(feed, state);
      }
    } else {
      DirtyObservationFeeds::<T>::insert(feed, state);
    }
    DirtyObservationScanSlot::<T>::put(next_cursor);
    Ok(DirtyObservationFeedCount::<T>::get() > 0)
  }

  pub fn fanout_dirty_observations(
    remaining_weight: polkadot_sdk::frame_support::weights::Weight,
  ) -> polkadot_sdk::frame_support::weights::Weight {
    use polkadot_sdk::frame_support::weights::Weight;
    use polkadot_sdk::sp_weights::WeightMeter;

    let configured = T::ObservationFanoutWeightLimit::get();
    let limit = Weight::from_parts(
      remaining_weight.ref_time().min(configured.ref_time()),
      remaining_weight.proof_size().min(configured.proof_size()),
    );
    let mut meter = WeightMeter::with_limit(limit);
    let base_weight = T::WeightInfo::observation_fanout_base();
    if !meter.can_consume(base_weight) {
      return Weight::zero();
    }
    meter.consume(base_weight);
    if Self::dirty_observation_fanout_base_probe() == 0 {
      return meter.consumed();
    }
    let unit_weight = T::WeightInfo::observation_fanout_page();
    for _ in 0..T::MaxObservationFanoutPagesPerBlock::get() {
      if !meter.can_consume(unit_weight) {
        break;
      }
      let result = polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Self::do_fanout_dirty_observation_page() {
          Ok(has_more) => TransactionOutcome::Commit(Ok(has_more)),
          Err(error) => TransactionOutcome::Rollback(Err(error)),
        }
      });
      meter.consume(unit_weight);
      match result {
        Ok(true) => {}
        Ok(false) | Err(_) => break,
      }
    }
    meter.consumed()
  }

  #[cfg(feature = "try-runtime")]
  pub(crate) fn do_try_state_dirty_observations()
  -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
    use alloc::collections::BTreeSet;
    use polkadot_sdk::sp_runtime::TryRuntimeError;

    let maximum = Self::maximum_dirty_observation_feeds()
      .map_err(|_| TryRuntimeError::Other("dirty observation capacity is invalid"))?;
    let page_size = Self::dirty_observation_page_size()
      .map_err(|_| TryRuntimeError::Other("dirty observation page size is invalid"))?;
    let next_slot = NextDirtyObservationSlot::<T>::get();
    let free_len = DirtyObservationFreeSlotLen::<T>::get();
    let scan_slot = DirtyObservationScanSlot::<T>::get();
    if next_slot > maximum
      || free_len > next_slot
      || (next_slot == 0 && scan_slot != 0)
      || (next_slot > 0 && scan_slot >= next_slot)
    {
      return Err(TryRuntimeError::Other(
        "dirty observation slot bounds are invalid",
      ));
    }
    let subscriber_page_count = NextObservationSubscriptionSlot::<T>::get().div_ceil(page_size);
    let mut owned_slots = BTreeSet::<u32>::new();
    let dirty_feeds = DirtyObservationFeeds::<T>::iter(); // deos-bypass: bounded-iter — try-state-only bounded dirty-feed audit.
    for (feed, state) in dirty_feeds {
      if state.latest_revision == 0
        || state.fanout_revision > state.latest_revision
        || (state.fanout_revision == 0 && state.next_subscriber_page != 0)
        || (state.fanout_revision > 0
          && (subscriber_page_count == 0 || state.next_subscriber_page >= subscriber_page_count))
        || ObservationSubscriberCount::<T>::get(feed) == 0
        || !owned_slots.insert(state.slot)
        || DirtyObservationSlotFeed::<T>::get(state.slot) != Some(feed)
      {
        return Err(TryRuntimeError::Other(
          "dirty observation feed state disagrees",
        ));
      }
    }
    let slot_feeds = DirtyObservationSlotFeed::<T>::iter(); // deos-bypass: bounded-iter — try-state-only bounded dirty-slot reverse audit.
    for (slot, feed) in slot_feeds {
      if DirtyObservationFeeds::<T>::get(feed).map(|state| state.slot) != Some(slot) {
        return Err(TryRuntimeError::Other(
          "dirty observation slot reverse owner disagrees",
        ));
      }
    }
    let mut free_slots = BTreeSet::<u32>::new();
    let free_page_count = free_len.div_ceil(page_size);
    for page_id in 0..free_page_count {
      let page = DirtyObservationFreeSlotPages::<T>::get(page_id).ok_or(TryRuntimeError::Other(
        "dirty observation free-slot page is missing",
      ))?;
      for slot in page {
        if slot >= next_slot
          || !free_slots.insert(slot)
          || DirtyObservationSlotFeed::<T>::contains_key(slot)
        {
          return Err(TryRuntimeError::Other(
            "dirty observation free-slot ownership is invalid",
          ));
        }
      }
    }
    if free_slots.len() as u32 != free_len
      || owned_slots.len() as u32 + free_len != next_slot
      || DirtyObservationFreeSlotPages::<T>::iter_keys().count() as u32 != free_page_count
      || DirtyObservationFeedCount::<T>::get() != owned_slots.len() as u32
    {
      return Err(TryRuntimeError::Other(
        "dirty observation slot accounting disagrees",
      ));
    }
    Ok(())
  }
}
