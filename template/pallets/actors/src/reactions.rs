use crate::pallet::*;
use crate::scheduler::{
  ActivationFailure, ActivationOutcome, ObservationActivationOutcome, ObservationPlacementCandidate,
};
use crate::types::{
  DirtyObservationList, DirtyObservationState, ObservationFanoutBranch, ObservationRevision,
};
use crate::weights::WeightInfo as _;
use alloc::vec;
use polkadot_sdk::frame_support::{ensure, storage::TransactionOutcome, traits::Get};
use polkadot_sdk::frame_system::{Pallet as System, pallet_prelude::BlockNumberFor};
use polkadot_sdk::sp_runtime::{
  DispatchError, DispatchResult,
  traits::{One, SaturatedConversion, Saturating},
};

impl<T: Config> Pallet<T> {
  fn append_crossing_pending_feed(feed: T::ObservationFeedId) -> DispatchResult {
    ensure!(
      !CrossingPendingFeeds::<T>::contains_key(feed),
      Error::<T>::CrossingTransitionInvariant
    );
    let mut list = CrossingPendingFeedListState::<T>::get();
    ensure!(
      list.count < T::MaxActiveActors::get(),
      Error::<T>::CrossingTransitionCapacityExceeded
    );
    if let Some(tail) = list.tail {
      CrossingPendingFeeds::<T>::try_mutate(tail, |maybe| -> DispatchResult {
        let state = maybe
          .as_mut()
          .ok_or(Error::<T>::CrossingTransitionInvariant)?;
        ensure!(
          state.next.is_none(),
          Error::<T>::CrossingTransitionInvariant
        );
        state.next = Some(feed);
        Ok(())
      })?;
    } else {
      ensure!(
        list.head.is_none() && list.cursor.is_none() && list.count == 0,
        Error::<T>::CrossingTransitionInvariant
      );
      list.head = Some(feed);
      list.cursor = Some(feed);
    }
    CrossingPendingFeeds::<T>::insert(
      feed,
      crate::CrossingPendingFeedState {
        previous: list.tail,
        next: None,
      },
    );
    list.tail = Some(feed);
    list.count = list
      .count
      .checked_add(1)
      .ok_or(Error::<T>::CrossingTransitionCapacityExceeded)?;
    CrossingPendingFeedListState::<T>::put(list);
    Ok(())
  }

  fn unlink_crossing_pending_feed(feed: T::ObservationFeedId) -> DispatchResult {
    let state =
      CrossingPendingFeeds::<T>::get(feed).ok_or(Error::<T>::CrossingTransitionInvariant)?;
    let mut list = CrossingPendingFeedListState::<T>::get();
    ensure!(list.count > 0, Error::<T>::CrossingTransitionInvariant);
    if let Some(previous) = state.previous {
      CrossingPendingFeeds::<T>::try_mutate(previous, |maybe| -> DispatchResult {
        let previous_state = maybe
          .as_mut()
          .ok_or(Error::<T>::CrossingTransitionInvariant)?;
        ensure!(
          previous_state.next == Some(feed),
          Error::<T>::CrossingTransitionInvariant
        );
        previous_state.next = state.next;
        Ok(())
      })?;
    } else {
      ensure!(
        list.head == Some(feed),
        Error::<T>::CrossingTransitionInvariant
      );
      list.head = state.next;
    }
    if let Some(next) = state.next {
      CrossingPendingFeeds::<T>::try_mutate(next, |maybe| -> DispatchResult {
        let next_state = maybe
          .as_mut()
          .ok_or(Error::<T>::CrossingTransitionInvariant)?;
        ensure!(
          next_state.previous == Some(feed),
          Error::<T>::CrossingTransitionInvariant
        );
        next_state.previous = state.previous;
        Ok(())
      })?;
    } else {
      ensure!(
        list.tail == Some(feed),
        Error::<T>::CrossingTransitionInvariant
      );
      list.tail = state.previous;
    }
    if list.cursor == Some(feed) {
      list.cursor = state.next.or(list.head);
    }
    list.count -= 1;
    CrossingPendingFeeds::<T>::remove(feed);
    if list.count == 0 {
      ensure!(
        list.head.is_none() && list.tail.is_none() && list.cursor.is_none(),
        Error::<T>::CrossingTransitionInvariant
      );
      CrossingPendingFeedListState::<T>::kill();
    } else {
      CrossingPendingFeedListState::<T>::put(list);
    }
    Ok(())
  }

  pub(crate) fn clear_crossing_transition_queue(feed: T::ObservationFeedId) -> DispatchResult {
    let has_queue = CrossingTransitionQueues::<T>::contains_key(feed);
    let has_link = CrossingPendingFeeds::<T>::contains_key(feed);
    ensure!(
      has_queue == has_link,
      Error::<T>::CrossingTransitionInvariant
    );
    CrossingRangeCursors::<T>::remove(feed);
    if has_link {
      Self::unlink_crossing_pending_feed(feed)?;
      CrossingTransitionQueues::<T>::remove(feed);
    }
    Ok(())
  }

  pub(crate) fn advance_crossing_pending_feed(feed: T::ObservationFeedId) -> DispatchResult {
    let state =
      CrossingPendingFeeds::<T>::get(feed).ok_or(Error::<T>::CrossingTransitionInvariant)?;
    CrossingPendingFeedListState::<T>::try_mutate(|list| -> DispatchResult {
      ensure!(
        list.cursor == Some(feed),
        Error::<T>::CrossingTransitionInvariant
      );
      list.cursor = state.next.or(list.head);
      Ok(())
    })
  }

  pub(crate) fn complete_crossing_transition(
    feed: T::ObservationFeedId,
    revision: u64,
  ) -> DispatchResult {
    let mut queue =
      CrossingTransitionQueues::<T>::get(feed).ok_or(Error::<T>::CrossingTransitionInvariant)?;
    ensure!(
      queue.first().is_some_and(|item| item.revision == revision),
      Error::<T>::CrossingTransitionInvariant
    );
    queue.remove(0);
    CrossingRangeCursors::<T>::remove(feed);
    if queue.is_empty() {
      Self::unlink_crossing_pending_feed(feed)?;
      CrossingTransitionQueues::<T>::remove(feed);
    } else {
      CrossingTransitionQueues::<T>::insert(feed, queue);
      Self::advance_crossing_pending_feed(feed)?;
    }
    Ok(())
  }

  fn maximum_dirty_observation_feeds() -> Result<u32, DispatchError> {
    Ok(T::MaxActiveActors::get())
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
    cause_provenance: crate::TriggerCauseProvenance,
    cause_block: u64,
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
        latest_cause_provenance: cause_provenance,
        latest_cause_block: cause_block,
        fanout_revision: 0,
        fanout_cause_provenance: crate::TriggerCauseProvenance::Deferred,
        fanout_cause_block: 0,
        dirty_since: System::<T>::block_number(),
        next_subscriber_page: None,
        next_subscriber_position: 0,
        next_subscriber_branch: ObservationFanoutBranch::Ordinary,
        retry_after: None,
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
    cause_provenance: crate::TriggerCauseProvenance,
  ) -> DispatchResult {
    ensure!(revision > 0, Error::<T>::InvalidObservationRevision);
    let cause_block = System::<T>::block_number().saturated_into::<u64>();
    if ObservationSubscriberCount::<T>::get(feed) == 0 {
      ensure!(
        !ObservationIngressRevisions::<T>::contains_key(feed)
          && !DirtyObservationFeeds::<T>::contains_key(feed),
        Error::<T>::DirtyObservationInvariant
      );
      return Ok(());
    }
    let baseline = ObservationIngressRevisions::<T>::get(feed);
    if let Some(current) = baseline {
      ensure!(revision >= current, Error::<T>::InvalidObservationRevision);
      if revision == current {
        return Ok(());
      }
    }
    let dirty = DirtyObservationFeeds::<T>::get(feed);
    ensure!(
      match (&baseline, &dirty) {
        (Some(current), Some(state)) => state.latest_revision == *current,
        (Some(_), None) | (None, None) => true,
        (None, Some(_)) => false,
      },
      Error::<T>::DirtyObservationInvariant
    );
    ObservationIngressRevisions::<T>::insert(feed, revision);
    if let Some(mut state) = dirty {
      state.latest_revision = revision;
      state.latest_cause_provenance = cause_provenance;
      state.latest_cause_block = cause_block;
      DirtyObservationFeeds::<T>::insert(feed, state);
      return Ok(());
    }
    Self::append_dirty_observation_feed(feed, revision, cause_provenance, cause_block)
  }

  pub fn observation_change_ingress_weight() -> polkadot_sdk::frame_support::weights::Weight {
    T::WeightInfo::observation_change_ingress()
  }

  /// Coalesces one changed feed revision without reading subscribers or executing actors.
  pub fn note_observation_changed(
    feed: T::ObservationFeedId,
    revision: ObservationRevision,
  ) -> DispatchResult {
    Self::with_reused_transaction(|| {
      Self::do_note_observation_changed(feed, revision, crate::TriggerCauseProvenance::Deferred)
    })
  }

  fn do_note_observation_transition(
    feed: T::ObservationFeedId,
    transition: crate::ObservationTransition,
    cause_provenance: crate::TriggerCauseProvenance,
  ) -> DispatchResult {
    ensure!(
      transition.revision > 0,
      Error::<T>::InvalidObservationRevision
    );
    if let Some(previous) = transition.previous {
      ensure!(
        previous != transition.current,
        Error::<T>::CrossingTransitionInvariant
      );
    } else {
      ensure!(
        transition.revision == 1,
        Error::<T>::CrossingTransitionInvariant
      );
    }
    let cause_block = System::<T>::block_number().saturated_into::<u64>();
    Self::do_note_observation_changed(feed, transition.revision, cause_provenance)?;
    if CrossingFeedMembershipCount::<T>::get(feed) == 0 || transition.previous.is_none() {
      return Ok(());
    }
    let was_empty = !CrossingTransitionQueues::<T>::contains_key(feed);
    ensure!(
      was_empty == !CrossingPendingFeeds::<T>::contains_key(feed),
      Error::<T>::CrossingTransitionInvariant
    );
    CrossingTransitionQueues::<T>::try_mutate(feed, |maybe| -> DispatchResult {
      let queue = maybe.get_or_insert_default();
      if let Some(last) = queue.last() {
        ensure!(
          transition.revision == last.revision.saturating_add(1)
            && transition.previous == Some(last.current),
          Error::<T>::CrossingTransitionInvariant
        );
      }
      queue
        .try_push(crate::CrossingTransitionObligation {
          revision: transition.revision,
          previous: transition
            .previous
            .ok_or(Error::<T>::CrossingTransitionInvariant)?,
          current: transition.current,
          cause_provenance,
          cause_block,
        })
        .map_err(|_| Error::<T>::CrossingTransitionCapacityExceeded)?;
      Ok(())
    })?;
    if was_empty {
      Self::append_crossing_pending_feed(feed)?;
    }
    Ok(())
  }

  pub fn note_observation_transition(
    feed: T::ObservationFeedId,
    transition: crate::ObservationTransition,
  ) -> DispatchResult {
    Self::note_observation_transition_with_provenance(
      feed,
      transition,
      crate::TriggerCauseProvenance::Deferred,
    )
  }

  pub fn note_observation_transition_with_provenance(
    feed: T::ObservationFeedId,
    transition: crate::ObservationTransition,
    cause_provenance: crate::TriggerCauseProvenance,
  ) -> DispatchResult {
    Self::with_reused_transaction(|| {
      Self::do_note_observation_transition(feed, transition, cause_provenance)
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

  fn process_observation_change_occurrence(
    actor_id: ActorId,
    feed: T::ObservationFeedId,
    execute_terminal: bool,
    cause_provenance: crate::TriggerCauseProvenance,
    cause_block: u64,
  ) -> Result<ObservationActivationOutcome, ActivationFailure> {
    if IndexedTriggerDetectionDisabled::<T>::contains_key(actor_id) {
      return Ok(ObservationActivationOutcome::Ordinary(
        ActivationOutcome::IgnoredStale,
      ));
    }
    let Some(state) = Self::load_observation_activation_state(actor_id, feed) else {
      return if execute_terminal {
        Self::request_observation_activation_compact_with_cause(
          actor_id,
          feed,
          cause_provenance,
          cause_block,
        )
        .map(ObservationActivationOutcome::Ordinary)
      } else {
        Self::request_observation_activation_ordinary_with_cause(
          actor_id,
          feed,
          cause_provenance,
          cause_block,
        )
      };
    };
    if state.hot.pending_signal {
      return Ok(ObservationActivationOutcome::Ordinary(
        ActivationOutcome::IgnoredStale,
      ));
    }
    let classification =
      Self::classify_observation_activation_compact(&state).map_err(|error| {
        ActivationFailure::Permanent(Self::classification_dispatch_error(error).into())
      })?;
    if classification.terminal_reason.is_some() {
      return if execute_terminal {
        Self::request_observation_activation_compact_with_cause(
          actor_id,
          feed,
          cause_provenance,
          cause_block,
        )
        .map(ObservationActivationOutcome::Ordinary)
      } else {
        Self::request_observation_activation_ordinary_with_cause(
          actor_id,
          feed,
          cause_provenance,
          cause_block,
        )
      };
    }
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let actor_type = state.identity.actor_class.actor_type();
      let breakdown = Self::trigger_fee_for_weight(
        actor_type,
        TriggerFamily::ObservationChange,
        T::WeightInfo::observation_change_trigger_occurrence(),
      );
      let charged = match Self::try_charge_automatic_trigger_occurrence(
        actor_type,
        &state.identity.sovereign_account,
        breakdown,
      ) {
        Ok(charged) => charged,
        Err(error) => {
          return TransactionOutcome::Rollback(Err(ActivationFailure::Permanent(error)));
        }
      };
      if !charged {
        return TransactionOutcome::Commit(Ok(ObservationActivationOutcome::Ordinary(
          ActivationOutcome::IgnoredStale,
        )));
      }
      let outcome = if execute_terminal {
        Self::request_observation_activation_compact_with_cause(
          actor_id,
          feed,
          cause_provenance,
          cause_block,
        )
        .map(ObservationActivationOutcome::Ordinary)
      } else {
        Self::request_observation_activation_ordinary_with_cause(
          actor_id,
          feed,
          cause_provenance,
          cause_block,
        )
      };
      let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(error) => return TransactionOutcome::Rollback(Err(error)),
      };
      let ObservationActivationOutcome::Ordinary(activation) = outcome else {
        return TransactionOutcome::Rollback(Ok(outcome));
      };
      if matches!(
        activation,
        ActivationOutcome::Latched | ActivationOutcome::Coalesced
      ) {
        IndexedTriggerDetectionDisabled::<T>::insert(actor_id, ());
        Self::deposit_event(Event::TriggerOccurrenceProcessed {
          actor_id,
          trigger_family: breakdown.trigger_family,
          fee: breakdown.trigger_fee,
        });
        TransactionOutcome::Commit(Ok(outcome))
      } else {
        TransactionOutcome::Rollback(Ok(outcome))
      }
    })
  }

  pub(crate) fn signal_observation_subscriber(
    actor_id: ActorId,
    feed: T::ObservationFeedId,
    cause_provenance: crate::TriggerCauseProvenance,
    cause_block: u64,
  ) -> Result<bool, DispatchError> {
    match Self::process_observation_change_occurrence(
      actor_id,
      feed,
      true,
      cause_provenance,
      cause_block,
    ) {
      Ok(ObservationActivationOutcome::Ordinary(
        ActivationOutcome::IgnoredStale
        | ActivationOutcome::Coalesced
        | ActivationOutcome::Latched
        | ActivationOutcome::Closed,
      )) => Ok(true),
      Ok(ObservationActivationOutcome::TerminalDeferred) => Err(Error::<T>::ActorInvariant.into()),
      Err(ActivationFailure::Temporary(_)) => Ok(false),
      Err(error @ ActivationFailure::Permanent(_)) => Err(Self::activation_failure_error(error)),
    }
  }

  fn signal_observation_subscriber_ordinary(
    actor_id: ActorId,
    feed: T::ObservationFeedId,
    cause_provenance: crate::TriggerCauseProvenance,
    cause_block: u64,
  ) -> Result<Option<bool>, DispatchError> {
    match Self::process_observation_change_occurrence(
      actor_id,
      feed,
      false,
      cause_provenance,
      cause_block,
    ) {
      Ok(ObservationActivationOutcome::Ordinary(
        ActivationOutcome::IgnoredStale
        | ActivationOutcome::Coalesced
        | ActivationOutcome::Latched
        | ActivationOutcome::Closed,
      )) => Ok(Some(true)),
      Ok(ObservationActivationOutcome::TerminalDeferred) => Ok(None),
      Err(ActivationFailure::Temporary(_)) => Ok(Some(false)),
      Err(error @ ActivationFailure::Permanent(_)) => Err(Self::activation_failure_error(error)),
    }
  }

  pub fn dirty_observation_feed_count() -> u32 {
    DirtyObservationListState::<T>::get().count
  }

  pub(crate) fn dirty_observation_fanout_base_probe() -> u32 {
    Self::dirty_observation_feed_count()
  }

  pub(crate) fn observation_fanout_branch_probe() -> Option<ObservationFanoutBranch> {
    let list = DirtyObservationListState::<T>::get();
    if list.count == 0 {
      return None;
    }
    let feed = list.cursor.or(list.head)?;
    let state = DirtyObservationFeeds::<T>::get(feed)?;
    if state
      .retry_after
      .is_some_and(|retry_after| retry_after > System::<T>::block_number())
    {
      return None;
    }
    Some(state.next_subscriber_branch)
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
    if let Some(retry_after) = state.retry_after {
      if retry_after > System::<T>::block_number() {
        Self::advance_dirty_observation_cursor(&mut list, &state);
        DirtyObservationListState::<T>::put(list);
        return Ok(true);
      }
      state.retry_after = None;
    }
    ensure!(
      Self::dirty_observation_links_are_valid(feed, &state, &list),
      Error::<T>::DirtyObservationInvariant
    );
    let page_list = ObservationSubscriberPageLists::<T>::get(feed)
      .ok_or(Error::<T>::DirtyObservationInvariant)?;
    ensure!(page_list.count > 0, Error::<T>::DirtyObservationInvariant);
    if state.fanout_revision == 0 {
      state.fanout_revision = state.latest_revision;
      state.fanout_cause_provenance = state.latest_cause_provenance;
      state.fanout_cause_block = state.latest_cause_block;
      state.next_subscriber_page = Some(page_list.head);
      state.next_subscriber_position = 0;
      state.next_subscriber_branch = ObservationFanoutBranch::Ordinary;
    }
    let Some(page_id) = state.next_subscriber_page else {
      ensure!(
        state.next_subscriber_position == 0
          && state.next_subscriber_branch == ObservationFanoutBranch::Ordinary
          && state.retry_after.is_none(),
        Error::<T>::DirtyObservationInvariant
      );
      if state.latest_revision == state.fanout_revision {
        Self::clear_dirty_observation_feed(feed)?;
      } else {
        state.fanout_revision = state.latest_revision;
        state.fanout_cause_provenance = state.latest_cause_provenance;
        state.fanout_cause_block = state.latest_cause_block;
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
    let page_len =
      u32::try_from(page.entries.len()).map_err(|_| Error::<T>::DirtyObservationInvariant)?;
    ensure!(
      state.next_subscriber_position < page_len,
      Error::<T>::DirtyObservationInvariant
    );
    let mut page_complete = true;
    'page: while state.next_subscriber_position < page_len {
      if state.next_subscriber_branch == ObservationFanoutBranch::Ordinary {
        let cohort_start = state.next_subscriber_position as usize;
        if let Some(actor_id) = page.entries[cohort_start]
          && let Some(first) = Self::prepare_observation_placement_candidate(
            actor_id,
            feed,
            state.fanout_cause_provenance,
            state.fanout_cause_block,
          )
          .map_err(Self::activation_failure_error)?
        {
          let wakeup_key = first.wakeup_key();
          let mut cohort_end = cohort_start.saturating_add(1);
          let committed = match first {
            ObservationPlacementCandidate::Queue(first) => {
              let mut candidates = vec![first];
              while cohort_end < page.entries.len() {
                let Some(actor_id) = page.entries[cohort_end] else {
                  break;
                };
                let Some(ObservationPlacementCandidate::Queue(candidate)) =
                  Self::prepare_observation_placement_candidate(
                    actor_id,
                    feed,
                    state.fanout_cause_provenance,
                    state.fanout_cause_block,
                  )
                  .map_err(Self::activation_failure_error)?
                else {
                  break;
                };
                candidates.push(candidate);
                cohort_end = cohort_end.saturating_add(1);
              }
              Self::commit_observation_queue_cohort(candidates).is_ok()
            }
            ObservationPlacementCandidate::Wakeup(first) => {
              let mut candidates = vec![first];
              while cohort_end < page.entries.len() {
                let Some(actor_id) = page.entries[cohort_end] else {
                  break;
                };
                let Some(candidate) = Self::prepare_observation_placement_candidate(
                  actor_id,
                  feed,
                  state.fanout_cause_provenance,
                  state.fanout_cause_block,
                )
                .map_err(Self::activation_failure_error)?
                else {
                  break;
                };
                if candidate.wakeup_key() != wakeup_key {
                  break;
                }
                let ObservationPlacementCandidate::Wakeup(candidate) = candidate else {
                  break;
                };
                candidates.push(candidate);
                cohort_end = cohort_end.saturating_add(1);
              }
              match Self::commit_observation_wakeup_cohort(candidates) {
                Ok(()) => true,
                Err(
                  crate::scheduler::EnqueueOutcome::CapacityUnavailable
                  | crate::scheduler::EnqueueOutcome::WakeupCapacityExhausted,
                ) => {
                  state.retry_after = Some(System::<T>::block_number().saturating_add(One::one()));
                  page_complete = false;
                  break 'page;
                }
                Err(_) => return Err(Error::<T>::SchedulerIndexExhausted.into()),
              }
            }
          };
          if committed {
            state.next_subscriber_position =
              u32::try_from(cohort_end).map_err(|_| Error::<T>::DirtyObservationInvariant)?;
            continue;
          }
        }
      }

      let position = state.next_subscriber_position as usize;
      let maybe_actor_id = page.entries[position];
      let next_position = state
        .next_subscriber_position
        .checked_add(1)
        .ok_or(Error::<T>::DirtyObservationInvariant)?;
      match state.next_subscriber_branch {
        ObservationFanoutBranch::Ordinary => {
          let Some(actor_id) = maybe_actor_id else {
            state.next_subscriber_position = next_position;
            continue;
          };
          match Self::signal_observation_subscriber_ordinary(
            actor_id,
            feed,
            state.fanout_cause_provenance,
            state.fanout_cause_block,
          )? {
            Some(true) => state.next_subscriber_position = next_position,
            Some(false) => {
              state.retry_after = Some(System::<T>::block_number().saturating_add(One::one()));
              page_complete = false;
              break;
            }
            None => {
              state.next_subscriber_branch = ObservationFanoutBranch::Terminal;
              page_complete = false;
              break;
            }
          }
        }
        ObservationFanoutBranch::Terminal => {
          let actor_id = maybe_actor_id.ok_or(Error::<T>::DirtyObservationInvariant)?;
          if !Self::signal_observation_subscriber(
            actor_id,
            feed,
            state.fanout_cause_provenance,
            state.fanout_cause_block,
          )? {
            state.retry_after = Some(System::<T>::block_number().saturating_add(One::one()));
            page_complete = false;
            break;
          }
          if !DirtyObservationFeeds::<T>::contains_key(feed) {
            return Ok(Self::dirty_observation_feed_count() > 0);
          }
          state.next_subscriber_position = next_position;
          state.next_subscriber_branch = ObservationFanoutBranch::Ordinary;
          page_complete = next_position == page_len;
          break;
        }
      }
    }
    if !page_complete {
      Self::advance_dirty_observation_cursor(&mut list, &state);
      DirtyObservationFeeds::<T>::insert(feed, state);
      DirtyObservationListState::<T>::put(list);
      return Ok(true);
    }
    ensure!(
      state.next_subscriber_position == page_len,
      Error::<T>::DirtyObservationInvariant
    );
    state.next_subscriber_page = next_page;
    state.next_subscriber_position = 0;
    state.next_subscriber_branch = ObservationFanoutBranch::Ordinary;
    state.retry_after = None;
    if next_page.is_none() {
      if state.latest_revision == state.fanout_revision {
        Self::clear_dirty_observation_feed(feed)?;
      } else {
        state.fanout_revision = state.latest_revision;
        state.fanout_cause_provenance = state.latest_cause_provenance;
        state.fanout_cause_block = state.latest_cause_block;
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
    Self::fanout_dirty_observations_with_pages(remaining_weight, 0).0
  }

  pub(crate) fn fanout_dirty_observations_with_pages(
    remaining_weight: polkadot_sdk::frame_support::weights::Weight,
    pages_already_serviced: u32,
  ) -> (polkadot_sdk::frame_support::weights::Weight, u32) {
    use polkadot_sdk::frame_support::weights::Weight;
    use polkadot_sdk::sp_weights::WeightMeter;

    let mut meter = WeightMeter::with_limit(remaining_weight);
    let mut pages_serviced = pages_already_serviced;
    if pages_serviced >= T::MaxObservationFanoutPagesPerBlock::get() {
      return (Weight::zero(), pages_serviced);
    }
    let base_weight = T::WeightInfo::observation_fanout_base();
    if !meter.can_consume(base_weight) {
      return (Weight::zero(), pages_serviced);
    }
    meter.consume(base_weight);
    if ObservationFanoutWorkerFaultState::<T>::exists() {
      return (meter.consumed(), pages_serviced);
    }
    if Self::dirty_observation_fanout_base_probe() == 0 {
      return (meter.consumed(), pages_serviced);
    }
    let branch_probe_weight = T::WeightInfo::observation_fanout_branch_probe();
    let fault_weight = T::WeightInfo::record_observation_fanout_worker_fault();
    for _ in pages_serviced..T::MaxObservationFanoutPagesPerBlock::get() {
      if !meter.can_consume(branch_probe_weight) {
        break;
      }
      meter.consume(branch_probe_weight);
      let Some(branch) = Self::observation_fanout_branch_probe() else {
        break;
      };
      let unit_weight = match branch {
        ObservationFanoutBranch::Ordinary => Self::observation_fanout_ordinary_weight_upper(),
        ObservationFanoutBranch::Terminal => T::WeightInfo::observation_fanout_terminal(),
      };
      if !meter.can_consume(unit_weight.saturating_add(fault_weight)) {
        break;
      }
      let list = DirtyObservationListState::<T>::get();
      let fault_feed = list.cursor.or(list.head);
      let fault_state = fault_feed.and_then(DirtyObservationFeeds::<T>::get);
      let result = polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Self::do_fanout_dirty_observation_page() {
          Ok(has_more) => TransactionOutcome::Commit(Ok(has_more)),
          Err(error) => TransactionOutcome::Rollback(Err(error)),
        }
      });
      meter.consume(unit_weight);
      pages_serviced = pages_serviced.saturating_add(1);
      match result {
        Ok(true) => {}
        Ok(false) => break,
        Err(error) => {
          if let (Some(feed), Some(state)) = (fault_feed, fault_state) {
            let class = if error == Error::<T>::DirtyObservationInvariant.into()
              || error == Error::<T>::ActorInvariant.into()
              || error == Error::<T>::ObservationSubscriptionInvariant.into()
            {
              CrossingWorkerFaultClass::Invariant
            } else if error == Error::<T>::SchedulerIndexExhausted.into() {
              CrossingWorkerFaultClass::SchedulerExhausted
            } else {
              CrossingWorkerFaultClass::Other
            };
            let actor_id = state.next_subscriber_page.and_then(|page_id| {
              ObservationSubscriberPages::<T>::get(feed, page_id).and_then(|page| {
                page
                  .entries
                  .get(state.next_subscriber_position as usize)
                  .copied()
                  .flatten()
              })
            });
            let authority = actor_id.and_then(ActorActivationAuthorities::<T>::get);
            let recorded = Self::record_observation_fanout_worker_fault(
              &mut meter,
              ObservationFanoutWorkerFault {
                feed,
                revision: state.latest_revision,
                subscriber_page: state.next_subscriber_page,
                subscriber_position: state.next_subscriber_position,
                actor_id,
                semantic_contract_id: authority
                  .as_ref()
                  .map(|authority| authority.semantic_contract_id),
                body_commitment: authority
                  .as_ref()
                  .map(|authority| authority.body_commitment),
                admission_identity: authority.map(|authority| authority.admission_identity),
                branch: state.next_subscriber_branch,
                class,
              },
            );
            debug_assert!(recorded, "fault Weight was reserved before fanout mutation");
          }
          break;
        }
      }
    }
    (meter.consumed(), pages_serviced)
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
    for feed in DirtyObservationFeeds::<T>::iter_keys() {
      let state = DirtyObservationFeeds::<T>::get(feed)
        .ok_or(TryRuntimeError::Other("dirty observation key has no state"))?;
      if state.latest_revision == 0
        || state.fanout_revision > state.latest_revision
        || (state.fanout_revision == 0
          && (state.fanout_cause_provenance != crate::TriggerCauseProvenance::Deferred
            || state.fanout_cause_block != 0
            || state.next_subscriber_page.is_some()
            || state.next_subscriber_position != 0
            || state.next_subscriber_branch != ObservationFanoutBranch::Ordinary
            || state.retry_after.is_some()))
        || state.next_subscriber_page.map_or(
          state.next_subscriber_position != 0
            || state.next_subscriber_branch != ObservationFanoutBranch::Ordinary
            || state.retry_after.is_some(),
          |page_id| {
            ObservationSubscriberPages::<T>::get(feed, page_id).is_none_or(|page| {
              state.next_subscriber_position >= page.entries.len() as u32
                || (state.next_subscriber_branch == ObservationFanoutBranch::Terminal
                  && page.entries[state.next_subscriber_position as usize].is_none())
            })
          },
        )
        || ObservationSubscriberCount::<T>::get(feed) == 0
        || ObservationSubscriberPageLists::<T>::get(feed).is_none()
        || ObservationIngressRevisions::<T>::get(feed) != Some(state.latest_revision)
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
    for feed in ObservationIngressRevisions::<T>::iter_keys() {
      let revision = ObservationIngressRevisions::<T>::get(feed).ok_or(TryRuntimeError::Other(
        "observation revision key has no value",
      ))?;
      if revision == 0 || ObservationSubscriberCount::<T>::get(feed) == 0 {
        return Err(TryRuntimeError::Other(
          "observation ingress revision baseline disagrees",
        ));
      }
    }
    Ok(())
  }
}

impl<T: Config> crate::ObservationTransitionIngress<T::ObservationFeedId> for Pallet<T> {
  fn note_observation_transition(
    feed: T::ObservationFeedId,
    transition: crate::ObservationTransition,
    cause_provenance: crate::TriggerCauseProvenance,
  ) -> DispatchResult {
    Pallet::<T>::note_observation_transition_with_provenance(feed, transition, cause_provenance)
  }
}
