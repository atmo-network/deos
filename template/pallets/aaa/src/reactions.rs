use crate::pallet::*;
use crate::types::{DirtyObservationList, DirtyObservationState, ObservationRevision};
use crate::weights::WeightInfo as _;
use polkadot_sdk::frame_support::{ensure, storage::TransactionOutcome, traits::Get};
use polkadot_sdk::frame_system::{Pallet as System, pallet_prelude::BlockNumberFor};
use polkadot_sdk::sp_runtime::{DispatchError, DispatchResult};

impl<T: Config> Pallet<T> {
  fn maximum_dirty_observation_feeds() -> Result<u32, DispatchError> {
    T::MaxActiveActors::get()
      .checked_mul(T::MaxTriggerSources::get())
      .ok_or(Error::<T>::DirtyObservationCapacityExceeded.into())
  }

  fn dirty_observation_links_are_valid(
    feed: T::ObservationFeedId,
    state: &DirtyObservationState<T::ObservationFeedId, BlockNumberFor<T>>,
    list: &DirtyObservationList<T::ObservationFeedId>,
  ) -> bool {
    let previous_valid = state
      .previous_dirty_feed
      .map_or(list.head == Some(feed), |previous| {
        DirtyObservationFeeds::<T>::get(previous)
          .is_some_and(|previous_state| previous_state.next_dirty_feed == Some(feed))
      });
    let next_valid = state
      .next_dirty_feed
      .map_or(list.tail == Some(feed), |next| {
        DirtyObservationFeeds::<T>::get(next)
          .is_some_and(|next_state| next_state.previous_dirty_feed == Some(feed))
      });
    previous_valid && next_valid
  }

  fn append_dirty_observation_feed(
    feed: T::ObservationFeedId,
    revision: ObservationRevision,
  ) -> DispatchResult {
    ensure!(
      !DirtyObservationFeeds::<T>::contains_key(feed),
      Error::<T>::DirtyObservationInvariant
    );
    let mut list = DirtyObservationListState::<T>::get();
    ensure!(
      list.count < Self::maximum_dirty_observation_feeds()?,
      Error::<T>::DirtyObservationCapacityExceeded
    );
    if let Some(tail) = list.tail {
      DirtyObservationFeeds::<T>::try_mutate(tail, |maybe| -> DispatchResult {
        let tail_state = maybe
          .as_mut()
          .ok_or(Error::<T>::DirtyObservationInvariant)?;
        ensure!(
          tail_state.next_dirty_feed.is_none(),
          Error::<T>::DirtyObservationInvariant
        );
        tail_state.next_dirty_feed = Some(feed);
        Ok(())
      })?;
    } else {
      ensure!(
        list.head.is_none() && list.cursor.is_none() && list.count == 0,
        Error::<T>::DirtyObservationInvariant
      );
      list.head = Some(feed);
      list.cursor = Some(feed);
    }
    DirtyObservationFeeds::<T>::insert(
      feed,
      DirtyObservationState {
        latest_revision: revision,
        fanout_revision: 0,
        dirty_since: System::<T>::block_number(),
        next_subscriber_page: None,
        previous_dirty_feed: list.tail,
        next_dirty_feed: None,
      },
    );
    list.tail = Some(feed);
    list.count = list
      .count
      .checked_add(1)
      .ok_or(Error::<T>::DirtyObservationCapacityExceeded)?;
    DirtyObservationListState::<T>::put(list);
    Ok(())
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
    Self::append_dirty_observation_feed(feed, revision)
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
    let list = DirtyObservationListState::<T>::get();
    for feed in feeds {
      if ObservationSubscriberCount::<T>::get(feed) != 1 {
        continue;
      }
      if let Some(state) = DirtyObservationFeeds::<T>::get(feed) {
        ensure!(
          Self::dirty_observation_links_are_valid(*feed, &state, &list),
          Error::<T>::DirtyObservationInvariant
        );
      }
    }
    Ok(())
  }

  pub(crate) fn clear_dirty_observation_feed(feed: T::ObservationFeedId) -> DispatchResult {
    let Some(state) = DirtyObservationFeeds::<T>::get(feed) else {
      return Ok(());
    };
    let mut list = DirtyObservationListState::<T>::get();
    ensure!(
      list.count > 0 && Self::dirty_observation_links_are_valid(feed, &state, &list),
      Error::<T>::DirtyObservationInvariant
    );
    let next_count = list
      .count
      .checked_sub(1)
      .ok_or(Error::<T>::DirtyObservationInvariant)?;
    if let Some(previous) = state.previous_dirty_feed {
      DirtyObservationFeeds::<T>::try_mutate(previous, |maybe| -> DispatchResult {
        let previous_state = maybe
          .as_mut()
          .ok_or(Error::<T>::DirtyObservationInvariant)?;
        previous_state.next_dirty_feed = state.next_dirty_feed;
        Ok(())
      })?;
    } else {
      list.head = state.next_dirty_feed;
    }
    if let Some(next) = state.next_dirty_feed {
      DirtyObservationFeeds::<T>::try_mutate(next, |maybe| -> DispatchResult {
        let next_state = maybe
          .as_mut()
          .ok_or(Error::<T>::DirtyObservationInvariant)?;
        next_state.previous_dirty_feed = state.previous_dirty_feed;
        Ok(())
      })?;
    } else {
      list.tail = state.previous_dirty_feed;
    }
    if list.cursor == Some(feed) {
      list.cursor = state.next_dirty_feed.or(list.head);
    }
    list.count = next_count;
    DirtyObservationFeeds::<T>::remove(feed);
    if next_count == 0 {
      ensure!(
        list.head.is_none() && list.tail.is_none() && list.cursor.is_none(),
        Error::<T>::DirtyObservationInvariant
      );
      DirtyObservationListState::<T>::kill();
    } else {
      ensure!(
        list.head.is_some() && list.tail.is_some() && list.cursor.is_some(),
        Error::<T>::DirtyObservationInvariant
      );
      DirtyObservationListState::<T>::put(list);
    }
    Ok(())
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

  pub fn dirty_observation_feed_count() -> u32 {
    DirtyObservationListState::<T>::get().count
  }

  pub(crate) fn dirty_observation_fanout_base_probe() -> u32 {
    Self::dirty_observation_feed_count()
  }

  fn advance_dirty_observation_cursor(
    list: &mut DirtyObservationList<T::ObservationFeedId>,
    state: &DirtyObservationState<T::ObservationFeedId, BlockNumberFor<T>>,
  ) {
    list.cursor = state.next_dirty_feed.or(list.head);
  }

  pub(crate) fn do_fanout_dirty_observation_page() -> Result<bool, DispatchError> {
    let mut list = DirtyObservationListState::<T>::get();
    if list.count == 0 {
      ensure!(
        list.head.is_none() && list.tail.is_none() && list.cursor.is_none(),
        Error::<T>::DirtyObservationInvariant
      );
      return Ok(false);
    }
    let feed = list.cursor.ok_or(Error::<T>::DirtyObservationInvariant)?;
    let mut state =
      DirtyObservationFeeds::<T>::get(feed).ok_or(Error::<T>::DirtyObservationInvariant)?;
    ensure!(
      Self::dirty_observation_links_are_valid(feed, &state, &list),
      Error::<T>::DirtyObservationInvariant
    );
    let page_list = ObservationSubscriberPageLists::<T>::get(feed)
      .ok_or(Error::<T>::DirtyObservationInvariant)?;
    ensure!(page_list.count > 0, Error::<T>::DirtyObservationInvariant);
    if state.fanout_revision == 0 {
      state.fanout_revision = state.latest_revision;
      state.next_subscriber_page = Some(page_list.head);
    }
    let Some(page_id) = state.next_subscriber_page else {
      if state.latest_revision == state.fanout_revision {
        Self::clear_dirty_observation_feed(feed)?;
      } else {
        state.fanout_revision = state.latest_revision;
        state.next_subscriber_page = Some(page_list.head);
        Self::advance_dirty_observation_cursor(&mut list, &state);
        DirtyObservationFeeds::<T>::insert(feed, state);
        DirtyObservationListState::<T>::put(list);
      }
      return Ok(Self::dirty_observation_feed_count() > 0);
    };
    let page = ObservationSubscriberPages::<T>::get(feed, page_id)
      .ok_or(Error::<T>::DirtyObservationInvariant)?;
    let next_page = page.next;
    let mut page_complete = true;
    for aaa_id in page.entries.into_iter().flatten() {
      // deos-bypass: bounded-iter — QueuePageSize bounds one fanout unit.
      page_complete &= Self::signal_observation_subscriber(aaa_id);
    }
    if !page_complete {
      Self::advance_dirty_observation_cursor(&mut list, &state);
      DirtyObservationFeeds::<T>::insert(feed, state);
      DirtyObservationListState::<T>::put(list);
      return Ok(true);
    }
    state.next_subscriber_page = next_page;
    if next_page.is_none() {
      if state.latest_revision == state.fanout_revision {
        Self::clear_dirty_observation_feed(feed)?;
      } else {
        state.fanout_revision = state.latest_revision;
        state.next_subscriber_page = Some(page_list.head);
        Self::advance_dirty_observation_cursor(&mut list, &state);
        DirtyObservationFeeds::<T>::insert(feed, state);
        DirtyObservationListState::<T>::put(list);
      }
    } else {
      Self::advance_dirty_observation_cursor(&mut list, &state);
      DirtyObservationFeeds::<T>::insert(feed, state);
      DirtyObservationListState::<T>::put(list);
    }
    Ok(Self::dirty_observation_feed_count() > 0)
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
    let list = DirtyObservationListState::<T>::get();
    if list.count > maximum
      || (list.count == 0 && (list.head.is_some() || list.tail.is_some() || list.cursor.is_some()))
      || (list.count > 0 && (list.head.is_none() || list.tail.is_none() || list.cursor.is_none()))
    {
      return Err(TryRuntimeError::Other(
        "dirty observation list bounds are invalid",
      ));
    }
    let mut linked_feeds = BTreeSet::<T::ObservationFeedId>::new();
    let mut current = list.head;
    let mut previous = None;
    while let Some(feed) = current {
      // deos-bypass: bounded-iter — try-state-only walk is bounded by maximum dirty feeds.
      let state = DirtyObservationFeeds::<T>::get(feed).ok_or(TryRuntimeError::Other(
        "dirty observation list owner is missing",
      ))?;
      if state.previous_dirty_feed != previous || !linked_feeds.insert(feed) {
        return Err(TryRuntimeError::Other(
          "dirty observation reciprocal links disagree",
        ));
      }
      previous = Some(feed);
      current = state.next_dirty_feed;
    }
    if previous != list.tail
      || linked_feeds.len() as u32 != list.count
      || list
        .cursor
        .is_some_and(|cursor| !linked_feeds.contains(&cursor))
    {
      return Err(TryRuntimeError::Other(
        "dirty observation list accounting disagrees",
      ));
    }
    let mut stored_feeds = BTreeSet::<T::ObservationFeedId>::new();
    let dirty_feeds = DirtyObservationFeeds::<T>::iter(); // deos-bypass: bounded-iter — try-state-only bounded dirty-feed audit.
    for (feed, state) in dirty_feeds {
      if state.latest_revision == 0
        || state.fanout_revision > state.latest_revision
        || (state.fanout_revision == 0 && state.next_subscriber_page.is_some())
        || state
          .next_subscriber_page
          .is_some_and(|page_id| ObservationSubscriberPages::<T>::get(feed, page_id).is_none())
        || ObservationSubscriberCount::<T>::get(feed) == 0
        || ObservationSubscriberPageLists::<T>::get(feed).is_none()
        || !stored_feeds.insert(feed)
        || !linked_feeds.contains(&feed)
        || !Self::dirty_observation_links_are_valid(feed, &state, &list)
      {
        return Err(TryRuntimeError::Other(
          "dirty observation feed state disagrees",
        ));
      }
    }
    if stored_feeds != linked_feeds {
      return Err(TryRuntimeError::Other(
        "dirty observation map and list disagree",
      ));
    }
    Ok(())
  }
}
