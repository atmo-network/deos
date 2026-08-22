use crate::ObservationProvider as _;
use crate::pallet::*;
use crate::scheduler::ActivationOutcome;
use crate::types::{
  CrossingDirection, CrossingLeafKey, CrossingLeafState, CrossingMember, CrossingMembershipLocator,
  CrossingMembershipRole, CrossingPhase, CrossingRadixNodeKey, CrossingTransition,
  CrossingTraversal, ObservationCrossing,
};
use polkadot_sdk::frame_support::{ensure, storage::TransactionOutcome, traits::Get};
use polkadot_sdk::sp_runtime::{DispatchError, DispatchResult};

const RADIX_DEPTH: u8 = 32;

struct CrossingWorkOutcome {
  has_more: bool,
  transitions: u32,
  leaves: u32,
  pages: u32,
  actors: u32,
}

impl CrossingWorkOutcome {
  const fn new(has_more: bool, transitions: u32, leaves: u32, pages: u32, actors: u32) -> Self {
    Self {
      has_more,
      transitions,
      leaves,
      pages,
      actors,
    }
  }
}

impl<T: Config> Pallet<T> {
  #[cfg(feature = "try-runtime")]
  pub(crate) fn do_try_state_crossing() -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
    use alloc::collections::{BTreeMap, BTreeSet};
    use polkadot_sdk::sp_runtime::TryRuntimeError;

    let invalid = || TryRuntimeError::Other("Crossing index topology is inconsistent");
    let mut expected_feed_counts = BTreeMap::new();
    let mut expected_radix = BTreeMap::<CrossingRadixNodeKeyOf<T>, u16>::new();
    let mut indexed_actors = BTreeSet::new();
    let page_size = T::ObservationPageSize::get();

    for (key, state) in CrossingLeafStates::<T>::iter(/* deos-bypass: bounded-iter */) {
      if page_size == 0
        || state.member_count == 0
        || state.page_count != state.tail_page.saturating_add(1)
      {
        return Err(invalid());
      }
      let mut leaf_count = 0u32;
      for page_id in 0..=state.tail_page {
        let page = CrossingMemberPages::<T>::get(key, page_id).ok_or_else(&invalid)?;
        if page.entries.is_empty()
          || (page_id < state.tail_page && page.entries.len() as u32 != page_size)
        {
          return Err(invalid());
        }
        for (offset, member) in page
          .entries
          .iter(/* deos-bypass: bounded-iter */)
          .enumerate()
        {
          let locator = CrossingMemberships::<T>::get(member.actor_id).ok_or_else(&invalid)?;
          if !indexed_actors.insert(member.actor_id)
            || locator.key != key
            || locator.page != page_id
            || locator.offset != offset as u32
            || locator.generation != member.generation
          {
            return Err(invalid());
          }
          let contract = ActorContracts::<T>::get(member.actor_id).ok_or_else(&invalid)?;
          let crossing = Self::crossing_from_trigger(&contract.trigger).ok_or_else(&invalid)?;
          let (expected_key, expected_role) = Self::crossing_obligation(&crossing, locator.phase);
          if !ActorHot::<T>::contains_key(member.actor_id)
            || expected_key != key
            || expected_role != locator.role
          {
            return Err(invalid());
          }
          leaf_count = leaf_count.checked_add(1).ok_or_else(&invalid)?;
        }
      }
      if leaf_count != state.member_count {
        return Err(invalid());
      }
      *expected_feed_counts.entry(key.feed).or_insert(0u32) = expected_feed_counts
        .get(&key.feed)
        .copied()
        .unwrap_or(0)
        .checked_add(leaf_count)
        .ok_or_else(&invalid)?;
      for depth in 0..RADIX_DEPTH {
        let node = Self::radix_node_key(&key, depth);
        let bit = 1u16 << Self::radix_child(&key, depth);
        expected_radix
          .entry(node)
          .and_modify(|bitmap| *bitmap |= bit)
          .or_insert(bit);
      }
    }
    if CrossingMemberPages::<T>::iter()
      .any(|(key, _, _)| !CrossingLeafStates::<T>::contains_key(key))
      || CrossingMemberships::<T>::iter_keys().any(|actor_id| !indexed_actors.contains(&actor_id))
    {
      return Err(invalid());
    }
    let actual_radix: BTreeMap<_, _> = CrossingRadixNodes::<T>::iter().collect();
    if actual_radix != expected_radix {
      return Err(invalid());
    }
    let actual_feed_counts: BTreeMap<_, _> = CrossingFeedMembershipCount::<T>::iter().collect();
    if actual_feed_counts != expected_feed_counts {
      return Err(invalid());
    }
    for actor_id in ActorHot::<T>::iter_keys() {
      let contract = ActorContracts::<T>::get(actor_id).ok_or_else(&invalid)?;
      if Self::crossing_from_trigger(&contract.trigger).is_some()
        != CrossingMemberships::<T>::contains_key(actor_id)
      {
        return Err(invalid());
      }
    }

    let list = CrossingPendingFeedListState::<T>::get();
    if (list.count == 0) != (list.head.is_none() && list.tail.is_none() && list.cursor.is_none()) {
      return Err(invalid());
    }
    let mut linked_feeds = BTreeSet::new();
    let mut previous = None;
    let mut current = list.head;
    while let Some(feed) = current {
      let link = CrossingPendingFeeds::<T>::get(feed).ok_or_else(&invalid)?;
      if link.previous != previous || !linked_feeds.insert(feed) {
        return Err(invalid());
      }
      let queue = CrossingTransitionQueues::<T>::get(feed).ok_or_else(&invalid)?;
      if queue.is_empty() || CrossingFeedMembershipCount::<T>::get(feed) == 0 {
        return Err(invalid());
      }
      for pair in queue.windows(2) {
        if pair[1].revision != pair[0].revision.saturating_add(1)
          || pair[1].previous != pair[0].current
          || pair[1].previous == pair[1].current
        {
          return Err(invalid());
        }
      }
      if queue
        .iter(/* deos-bypass: bounded-iter */)
        .any(|transition| transition.previous == transition.current)
      {
        return Err(invalid());
      }
      if let Some(cursor) = CrossingRangeCursors::<T>::get(feed) {
        let head = queue.first().ok_or_else(&invalid)?;
        let traversal = if head.current > head.previous {
          CrossingTraversal::Upward
        } else {
          CrossingTraversal::Downward
        };
        if cursor.revision != head.revision || cursor.traversal != traversal {
          return Err(invalid());
        }
      }
      previous = Some(feed);
      current = link.next;
    }
    if linked_feeds.len() as u32 != list.count
      || previous != list.tail
      || list
        .cursor
        .is_some_and(|feed| !linked_feeds.contains(&feed))
      || CrossingPendingFeeds::<T>::iter_keys().any(|feed| !linked_feeds.contains(&feed))
      || CrossingTransitionQueues::<T>::iter_keys().any(|feed| !linked_feeds.contains(&feed))
      || CrossingRangeCursors::<T>::iter_keys().any(|feed| !linked_feeds.contains(&feed))
    {
      return Err(invalid());
    }
    Ok(())
  }

  pub(crate) fn crossing_obligation(
    crossing: &ObservationCrossing<T::ObservationFeedId>,
    phase: CrossingPhase,
  ) -> (CrossingLeafKeyOf<T>, CrossingMembershipRole) {
    match (crossing.direction, phase) {
      (CrossingDirection::Rising, CrossingPhase::Armed) => (
        CrossingLeafKey {
          feed: crossing.feed,
          traversal: CrossingTraversal::Upward,
          threshold: crossing.threshold,
        },
        CrossingMembershipRole::Fire,
      ),
      (CrossingDirection::Rising, CrossingPhase::WaitingForRearm) => (
        CrossingLeafKey {
          feed: crossing.feed,
          traversal: CrossingTraversal::Downward,
          threshold: crossing.rearm_threshold,
        },
        CrossingMembershipRole::Rearm,
      ),
      (CrossingDirection::Falling, CrossingPhase::Armed) => (
        CrossingLeafKey {
          feed: crossing.feed,
          traversal: CrossingTraversal::Downward,
          threshold: crossing.threshold,
        },
        CrossingMembershipRole::Fire,
      ),
      (CrossingDirection::Falling, CrossingPhase::WaitingForRearm) => (
        CrossingLeafKey {
          feed: crossing.feed,
          traversal: CrossingTraversal::Upward,
          threshold: crossing.rearm_threshold,
        },
        CrossingMembershipRole::Rearm,
      ),
    }
  }

  fn radix_node_key(key: &CrossingLeafKeyOf<T>, depth: u8) -> CrossingRadixNodeKeyOf<T> {
    let prefix = if depth == 0 {
      0
    } else {
      key.threshold >> (128 - u32::from(depth) * 4)
    };
    CrossingRadixNodeKey {
      feed: key.feed,
      traversal: key.traversal,
      depth,
      prefix,
    }
  }

  fn radix_child(key: &CrossingLeafKeyOf<T>, depth: u8) -> u8 {
    ((key.threshold >> (124 - u32::from(depth) * 4)) & 0x0f) as u8
  }

  fn insert_crossing_radix_path(key: &CrossingLeafKeyOf<T>) -> DispatchResult {
    for depth in 0..RADIX_DEPTH {
      let node = Self::radix_node_key(key, depth);
      let bit = 1u16 << Self::radix_child(key, depth);
      CrossingRadixNodes::<T>::mutate(node, |bitmap| {
        *bitmap = Some(bitmap.unwrap_or(0) | bit);
      });
    }
    Ok(())
  }

  fn remove_crossing_radix_path(key: &CrossingLeafKeyOf<T>) -> DispatchResult {
    for depth in (0..RADIX_DEPTH).rev() {
      let node = Self::radix_node_key(key, depth);
      let bit = 1u16 << Self::radix_child(key, depth);
      let bitmap = CrossingRadixNodes::<T>::get(node).ok_or(Error::<T>::CrossingIndexInvariant)?;
      ensure!(bitmap & bit != 0, Error::<T>::CrossingIndexInvariant);
      let next = bitmap & !bit;
      if next == 0 {
        CrossingRadixNodes::<T>::remove(node);
      } else {
        CrossingRadixNodes::<T>::insert(node, next);
        break;
      }
    }
    Ok(())
  }

  fn insert_crossing_member(
    actor_id: ActorId,
    crossing: ObservationCrossing<T::ObservationFeedId>,
    phase: CrossingPhase,
    generation: u64,
    installed_at_revision: u64,
  ) -> DispatchResult {
    ensure!(
      !CrossingMemberships::<T>::contains_key(actor_id),
      Error::<T>::CrossingIndexInvariant
    );
    let page_size = T::ObservationPageSize::get();
    ensure!(page_size > 0, Error::<T>::CrossingIndexInvariant);
    let (key, role) = Self::crossing_obligation(&crossing, phase);
    let mut state = CrossingLeafStates::<T>::get(key).unwrap_or(CrossingLeafState {
      tail_page: 0,
      page_count: 1,
      member_count: 0,
    });
    ensure!(
      state.member_count < T::MaxActiveActors::get(),
      Error::<T>::CrossingIndexCapacityExceeded
    );
    let mut page = CrossingMemberPages::<T>::get(key, state.tail_page).unwrap_or_default();
    if page.entries.len() as u32 == page_size {
      state.tail_page = state
        .tail_page
        .checked_add(1)
        .ok_or(Error::<T>::CrossingIndexCapacityExceeded)?;
      state.page_count = state
        .page_count
        .checked_add(1)
        .ok_or(Error::<T>::CrossingIndexCapacityExceeded)?;
      page = Default::default();
    }
    let page_id = state.tail_page;
    let offset = page.entries.len() as u32;
    page
      .entries
      .try_push(CrossingMember {
        actor_id,
        generation,
      })
      .map_err(|_| Error::<T>::CrossingIndexCapacityExceeded)?;
    if state.member_count == 0 {
      Self::insert_crossing_radix_path(&key)?;
    }
    state.member_count = state
      .member_count
      .checked_add(1)
      .ok_or(Error::<T>::CrossingIndexCapacityExceeded)?;
    CrossingMemberPages::<T>::insert(key, page_id, page);
    CrossingLeafStates::<T>::insert(key, state);
    CrossingMemberships::<T>::insert(
      actor_id,
      CrossingMembershipLocator {
        key,
        role,
        phase,
        page: page_id,
        offset,
        generation,
        installed_at_revision,
      },
    );
    CrossingFeedMembershipCount::<T>::try_mutate(key.feed, |count| -> DispatchResult {
      *count = count
        .checked_add(1)
        .ok_or(Error::<T>::CrossingIndexCapacityExceeded)?;
      ensure!(
        *count <= T::MaxActiveActors::get(),
        Error::<T>::CrossingIndexCapacityExceeded
      );
      Ok(())
    })
  }

  fn remove_crossing_member(actor_id: ActorId, clear_feed_queue: bool) -> DispatchResult {
    let Some(locator) = CrossingMemberships::<T>::get(actor_id) else {
      return Ok(());
    };
    let mut state =
      CrossingLeafStates::<T>::get(locator.key).ok_or(Error::<T>::CrossingIndexInvariant)?;
    ensure!(state.member_count > 0, Error::<T>::CrossingIndexInvariant);
    let mut page = CrossingMemberPages::<T>::get(locator.key, locator.page)
      .ok_or(Error::<T>::CrossingIndexInvariant)?;
    ensure!(
      page.entries.get(locator.offset as usize).copied()
        == Some(CrossingMember {
          actor_id,
          generation: locator.generation,
        }),
      Error::<T>::CrossingIndexInvariant
    );
    let tail_page = state.tail_page;
    let mut tail = CrossingMemberPages::<T>::get(locator.key, tail_page)
      .ok_or(Error::<T>::CrossingIndexInvariant)?;
    let tail_offset = tail
      .entries
      .len()
      .checked_sub(1)
      .ok_or(Error::<T>::CrossingIndexInvariant)? as u32;
    let moved = tail
      .entries
      .pop()
      .ok_or(Error::<T>::CrossingIndexInvariant)?;
    if locator.page != state.tail_page {
      page.entries[locator.offset as usize] = moved;
      CrossingMemberPages::<T>::insert(locator.key, locator.page, page);
      CrossingMemberships::<T>::try_mutate(moved.actor_id, |maybe| -> DispatchResult {
        let moved_locator = maybe.as_mut().ok_or(Error::<T>::CrossingIndexInvariant)?;
        ensure!(
          moved_locator.key == locator.key && moved_locator.generation == moved.generation,
          Error::<T>::CrossingIndexInvariant
        );
        moved_locator.page = locator.page;
        moved_locator.offset = locator.offset;
        Ok(())
      })?;
    } else if moved.actor_id != actor_id || moved.generation != locator.generation {
      tail.entries[locator.offset as usize] = moved;
      CrossingMemberships::<T>::try_mutate(moved.actor_id, |maybe| -> DispatchResult {
        let moved_locator = maybe.as_mut().ok_or(Error::<T>::CrossingIndexInvariant)?;
        ensure!(
          moved_locator.key == locator.key && moved_locator.generation == moved.generation,
          Error::<T>::CrossingIndexInvariant
        );
        moved_locator.offset = locator.offset;
        Ok(())
      })?;
    }
    if tail.entries.is_empty() {
      CrossingMemberPages::<T>::remove(locator.key, state.tail_page);
      if state.tail_page > 0 {
        state.tail_page -= 1;
        state.page_count = state
          .page_count
          .checked_sub(1)
          .ok_or(Error::<T>::CrossingIndexInvariant)?;
      }
    } else {
      CrossingMemberPages::<T>::insert(locator.key, state.tail_page, tail);
    }
    if moved.actor_id != actor_id
      && CrossingRangeCursors::<T>::get(locator.key.feed).is_some_and(|cursor| {
        cursor.current_threshold == Some(locator.key.threshold)
          && cursor.traversal == locator.key.traversal
          && (tail_page, tail_offset) >= (cursor.page, cursor.offset)
          && (locator.page, locator.offset) < (cursor.page, cursor.offset)
      })
    {
      CrossingRangeCursors::<T>::mutate(locator.key.feed, |maybe| {
        if let Some(cursor) = maybe {
          cursor.page = locator.page;
          cursor.offset = locator.offset;
        }
      });
    }
    state.member_count -= 1;
    CrossingMemberships::<T>::remove(actor_id);
    CrossingFeedMembershipCount::<T>::try_mutate_exists(
      locator.key.feed,
      |maybe| -> DispatchResult {
        let count = maybe.as_mut().ok_or(Error::<T>::CrossingIndexInvariant)?;
        *count = count
          .checked_sub(1)
          .ok_or(Error::<T>::CrossingIndexInvariant)?;
        if *count == 0 {
          *maybe = None;
        }
        Ok(())
      },
    )?;
    if clear_feed_queue && CrossingFeedMembershipCount::<T>::get(locator.key.feed) == 0 {
      Self::clear_crossing_transition_queue(locator.key.feed)?;
    }
    if state.member_count == 0 {
      CrossingLeafStates::<T>::remove(locator.key);
      Self::remove_crossing_radix_path(&locator.key)?;
    } else {
      CrossingLeafStates::<T>::insert(locator.key, state);
    }
    Ok(())
  }

  pub(crate) fn remove_crossing_membership(actor_id: ActorId) -> DispatchResult {
    Self::remove_crossing_member(actor_id, true)
  }

  fn move_crossing_membership(
    actor_id: ActorId,
    crossing: ObservationCrossing<T::ObservationFeedId>,
    next_phase: CrossingPhase,
    locator: CrossingMembershipLocator<T::ObservationFeedId>,
  ) -> DispatchResult {
    Self::remove_crossing_member(actor_id, false)?;
    Self::insert_crossing_member(
      actor_id,
      crossing,
      next_phase,
      locator.generation,
      locator.installed_at_revision,
    )
  }

  fn crossing_radix_min_ge(
    feed: T::ObservationFeedId,
    traversal: CrossingTraversal,
    depth: u8,
    prefix: u128,
    lower: u128,
    upper: u128,
  ) -> Result<Option<u128>, DispatchError> {
    let node = CrossingRadixNodeKey {
      feed,
      traversal,
      depth,
      prefix,
    };
    let Some(bitmap) = CrossingRadixNodes::<T>::get(node) else {
      return Ok(None);
    };
    ensure!(bitmap != 0, Error::<T>::CrossingIndexInvariant);
    for child in 0u8..16 {
      if bitmap & (1u16 << child) == 0 {
        continue;
      }
      let child_prefix = (prefix << 4) | u128::from(child);
      let child_depth = depth + 1;
      let remaining = 128 - u32::from(child_depth) * 4;
      let minimum = if remaining == 0 {
        child_prefix
      } else {
        child_prefix << remaining
      };
      let maximum = if remaining == 0 {
        child_prefix
      } else {
        minimum | ((1u128 << remaining) - 1)
      };
      if maximum < lower || minimum > upper {
        continue;
      }
      if child_depth == RADIX_DEPTH {
        let key = CrossingLeafKey {
          feed,
          traversal,
          threshold: child_prefix,
        };
        ensure!(
          CrossingLeafStates::<T>::contains_key(key),
          Error::<T>::CrossingIndexInvariant
        );
        return Ok(Some(child_prefix));
      }
      if let Some(found) =
        Self::crossing_radix_min_ge(feed, traversal, child_depth, child_prefix, lower, upper)?
      {
        return Ok(Some(found));
      }
    }
    Ok(None)
  }

  fn crossing_radix_max_le(
    feed: T::ObservationFeedId,
    traversal: CrossingTraversal,
    depth: u8,
    prefix: u128,
    lower: u128,
    upper: u128,
  ) -> Result<Option<u128>, DispatchError> {
    let node = CrossingRadixNodeKey {
      feed,
      traversal,
      depth,
      prefix,
    };
    let Some(bitmap) = CrossingRadixNodes::<T>::get(node) else {
      return Ok(None);
    };
    ensure!(bitmap != 0, Error::<T>::CrossingIndexInvariant);
    for child in (0u8..16).rev() {
      if bitmap & (1u16 << child) == 0 {
        continue;
      }
      let child_prefix = (prefix << 4) | u128::from(child);
      let child_depth = depth + 1;
      let remaining = 128 - u32::from(child_depth) * 4;
      let minimum = if remaining == 0 {
        child_prefix
      } else {
        child_prefix << remaining
      };
      let maximum = if remaining == 0 {
        child_prefix
      } else {
        minimum | ((1u128 << remaining) - 1)
      };
      if maximum < lower || minimum > upper {
        continue;
      }
      if child_depth == RADIX_DEPTH {
        let key = CrossingLeafKey {
          feed,
          traversal,
          threshold: child_prefix,
        };
        ensure!(
          CrossingLeafStates::<T>::contains_key(key),
          Error::<T>::CrossingIndexInvariant
        );
        return Ok(Some(child_prefix));
      }
      if let Some(found) =
        Self::crossing_radix_max_le(feed, traversal, child_depth, child_prefix, lower, upper)?
      {
        return Ok(Some(found));
      }
    }
    Ok(None)
  }

  fn initialize_crossing_cursor(
    transition: crate::CrossingTransitionObligation,
  ) -> Result<crate::CrossingRangeCursor, DispatchError> {
    if transition.current > transition.previous {
      let search_bound = transition
        .previous
        .checked_add(1)
        .ok_or(Error::<T>::CrossingTransitionInvariant)?;
      Ok(crate::CrossingRangeCursor {
        revision: transition.revision,
        traversal: CrossingTraversal::Upward,
        search_bound,
        current_threshold: None,
        page: 0,
        offset: 0,
        exhausted: false,
      })
    } else {
      let search_bound = transition
        .previous
        .checked_sub(1)
        .ok_or(Error::<T>::CrossingTransitionInvariant)?;
      Ok(crate::CrossingRangeCursor {
        revision: transition.revision,
        traversal: CrossingTraversal::Downward,
        search_bound,
        current_threshold: None,
        page: 0,
        offset: 0,
        exhausted: false,
      })
    }
  }

  fn advance_crossing_threshold(
    cursor: &mut crate::CrossingRangeCursor,
    transition: &crate::CrossingTransitionObligation,
    threshold: u128,
  ) {
    cursor.current_threshold = None;
    cursor.page = 0;
    cursor.offset = 0;
    match cursor.traversal {
      CrossingTraversal::Upward => {
        if threshold >= transition.current {
          cursor.exhausted = true;
        } else {
          cursor.search_bound = threshold + 1;
        }
      }
      CrossingTraversal::Downward => {
        if threshold <= transition.current {
          cursor.exhausted = true;
        } else {
          cursor.search_bound = threshold - 1;
        }
      }
    }
  }

  pub(crate) fn crossing_from_trigger(
    trigger: &TriggerOf<T>,
  ) -> Option<ObservationCrossing<T::ObservationFeedId>> {
    trigger
      .observation_crossing_contract()
      .map(|crossing| ObservationCrossing {
        feed: *crossing.feed,
        direction: crossing.direction,
        threshold: crossing.threshold,
        rearm_threshold: crossing.rearm_threshold,
      })
  }

  fn do_crossing_work_unit() -> Result<CrossingWorkOutcome, DispatchError> {
    let list = CrossingPendingFeedListState::<T>::get();
    if list.count == 0 {
      ensure!(
        list.head.is_none() && list.tail.is_none() && list.cursor.is_none(),
        Error::<T>::CrossingTransitionInvariant
      );
      return Ok(CrossingWorkOutcome::new(false, 0, 0, 0, 0));
    }
    let feed = list.cursor.ok_or(Error::<T>::CrossingTransitionInvariant)?;
    let queue =
      CrossingTransitionQueues::<T>::get(feed).ok_or(Error::<T>::CrossingTransitionInvariant)?;
    let transition = *queue
      .first()
      .ok_or(Error::<T>::CrossingTransitionInvariant)?;
    let mut cursor = CrossingRangeCursors::<T>::get(feed)
      .map_or_else(|| Self::initialize_crossing_cursor(transition), Ok)?;
    ensure!(
      cursor.revision == transition.revision,
      Error::<T>::CrossingTransitionInvariant
    );
    if cursor.exhausted {
      Self::complete_crossing_transition(feed, transition.revision)?;
      return Ok(CrossingWorkOutcome::new(
        CrossingPendingFeedListState::<T>::get().count > 0,
        1,
        0,
        0,
        0,
      ));
    }
    let threshold = if let Some(threshold) = cursor.current_threshold {
      threshold
    } else {
      let found = match cursor.traversal {
        CrossingTraversal::Upward => Self::crossing_radix_min_ge(
          feed,
          cursor.traversal,
          0,
          0,
          cursor.search_bound,
          transition.current,
        )?,
        CrossingTraversal::Downward => Self::crossing_radix_max_le(
          feed,
          cursor.traversal,
          0,
          0,
          transition.current,
          cursor.search_bound,
        )?,
      };
      let Some(threshold) = found else {
        cursor.exhausted = true;
        CrossingRangeCursors::<T>::insert(feed, cursor);
        Self::advance_crossing_pending_feed(feed)?;
        return Ok(CrossingWorkOutcome::new(true, 1, 0, 0, 0));
      };
      cursor.current_threshold = Some(threshold);
      cursor.page = 0;
      cursor.offset = 0;
      threshold
    };
    let key = CrossingLeafKey {
      feed,
      traversal: cursor.traversal,
      threshold,
    };
    let state = CrossingLeafStates::<T>::get(key).ok_or(Error::<T>::CrossingIndexInvariant)?;
    if cursor.page > state.tail_page {
      Self::advance_crossing_threshold(&mut cursor, &transition, threshold);
      CrossingRangeCursors::<T>::insert(feed, cursor);
      Self::advance_crossing_pending_feed(feed)?;
      return Ok(CrossingWorkOutcome::new(true, 1, 1, 0, 0));
    }
    let page =
      CrossingMemberPages::<T>::get(key, cursor.page).ok_or(Error::<T>::CrossingIndexInvariant)?;
    if cursor.offset as usize >= page.entries.len() {
      cursor.page = cursor
        .page
        .checked_add(1)
        .ok_or(Error::<T>::CrossingIndexInvariant)?;
      cursor.offset = 0;
      if cursor.page > state.tail_page {
        Self::advance_crossing_threshold(&mut cursor, &transition, threshold);
      }
      CrossingRangeCursors::<T>::insert(feed, cursor);
      Self::advance_crossing_pending_feed(feed)?;
      return Ok(CrossingWorkOutcome::new(true, 1, 1, 1, 0));
    }
    let member = page.entries[cursor.offset as usize];
    let locator =
      CrossingMemberships::<T>::get(member.actor_id).ok_or(Error::<T>::CrossingIndexInvariant)?;
    ensure!(
      locator.key == key
        && locator.page == cursor.page
        && locator.offset == cursor.offset
        && locator.generation == member.generation,
      Error::<T>::CrossingIndexInvariant
    );
    if locator.installed_at_revision >= transition.revision {
      cursor.offset += 1;
      CrossingRangeCursors::<T>::insert(feed, cursor);
      Self::advance_crossing_pending_feed(feed)?;
      return Ok(CrossingWorkOutcome::new(true, 1, 1, 1, 1));
    }
    let contract =
      ActorContracts::<T>::get(member.actor_id).ok_or(Error::<T>::CrossingIndexInvariant)?;
    let crossing =
      Self::crossing_from_trigger(&contract.trigger).ok_or(Error::<T>::CrossingIndexInvariant)?;
    let transition_kind =
      crossing.transition(locator.phase, transition.previous, transition.current);
    ensure!(
      matches!(
        (locator.role, transition_kind),
        (CrossingMembershipRole::Fire, CrossingTransition::Fire)
          | (CrossingMembershipRole::Rearm, CrossingTransition::Rearm)
      ),
      Error::<T>::CrossingIndexInvariant
    );
    let next_phase = match transition_kind {
      CrossingTransition::Fire => CrossingPhase::WaitingForRearm,
      CrossingTransition::Rearm => CrossingPhase::Armed,
      CrossingTransition::None => return Err(Error::<T>::CrossingIndexInvariant.into()),
    };
    Self::move_crossing_membership(member.actor_id, crossing, next_phase, locator)?;
    if transition_kind == CrossingTransition::Fire {
      let activated =
        Self::request_activation(member.actor_id).map_err(Self::activation_failure_error)?;
      ensure!(
        activated != ActivationOutcome::IgnoredStale,
        Error::<T>::CrossingIndexInvariant
      );
      if activated == ActivationOutcome::Closed && CrossingFeedMembershipCount::<T>::get(feed) == 0
      {
        // Closing the final member clears this feed's queue, cursor, and pending
        // link. Report possible outer work conservatively; the next bounded
        // unit observes the canonical pending-list state without recreating it.
        return Ok(CrossingWorkOutcome::new(true, 1, 1, 1, 1));
      }
    }
    if CrossingLeafStates::<T>::contains_key(key) {
      cursor.current_threshold = Some(threshold);
    } else {
      Self::advance_crossing_threshold(&mut cursor, &transition, threshold);
    }
    CrossingRangeCursors::<T>::insert(feed, cursor);
    Self::advance_crossing_pending_feed(feed)?;
    Ok(CrossingWorkOutcome::new(true, 1, 1, 1, 1))
  }

  pub fn crossing_work_unit() -> Result<bool, DispatchError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| match Self::do_crossing_work_unit() {
      Ok(outcome) => TransactionOutcome::Commit(Ok(outcome.has_more)),
      Err(error) => TransactionOutcome::Rollback(Err(error)),
    })
  }

  pub fn service_crossing_transitions(
    remaining_weight: polkadot_sdk::frame_support::weights::Weight,
  ) -> polkadot_sdk::frame_support::weights::Weight {
    use crate::weights::WeightInfo as _;
    use polkadot_sdk::sp_weights::WeightMeter;

    let configured = T::CrossingWorkerWeightLimit::get();
    let limit = polkadot_sdk::frame_support::weights::Weight::from_parts(
      remaining_weight.ref_time().min(configured.ref_time()),
      remaining_weight.proof_size().min(configured.proof_size()),
    );
    let mut meter = WeightMeter::with_limit(limit);
    let base = T::WeightInfo::crossing_worker_base();
    if !meter.can_consume(base) {
      return meter.consumed();
    }
    meter.consume(base);
    if CrossingPendingFeedListState::<T>::get().count == 0 {
      return meter.consumed();
    }
    let unit = T::WeightInfo::crossing_transition_unit()
      .saturating_add(T::WeightInfo::crossing_leaf_unit())
      .saturating_add(T::WeightInfo::crossing_page_unit())
      .saturating_add(T::WeightInfo::crossing_actor_unit());
    let mut transitions = 0u32;
    let mut leaves = 0u32;
    let mut pages = 0u32;
    let mut actors = 0u32;
    loop {
      if transitions >= T::MaxCrossingTransitionsPerBlock::get()
        || leaves >= T::MaxCrossingLeavesPerBlock::get()
        || pages >= T::MaxCrossingPagesPerBlock::get()
        || actors >= T::MaxCrossingActorsPerBlock::get()
        || !meter.can_consume(unit)
      {
        break;
      }
      let result =
        polkadot_sdk::frame_support::storage::with_transaction(
          || match Self::do_crossing_work_unit() {
            Ok(outcome) => TransactionOutcome::Commit(Ok(outcome)),
            Err(error) => TransactionOutcome::Rollback(Err(error)),
          },
        );
      meter.consume(unit);
      let Ok(outcome) = result else {
        break;
      };
      transitions = transitions.saturating_add(outcome.transitions);
      leaves = leaves.saturating_add(outcome.leaves);
      pages = pages.saturating_add(outcome.pages);
      actors = actors.saturating_add(outcome.actors);
      if !outcome.has_more {
        break;
      }
    }
    meter.consumed()
  }

  pub(crate) fn replace_crossing_membership(
    actor_id: ActorId,
    trigger: &TriggerOf<T>,
  ) -> DispatchResult {
    let previous_generation =
      CrossingMemberships::<T>::get(actor_id).map_or(0, |locator| locator.generation);
    let Some(crossing) = trigger.observation_crossing_contract() else {
      return Self::remove_crossing_membership(actor_id);
    };
    let crossing = ObservationCrossing {
      feed: *crossing.feed,
      direction: crossing.direction,
      threshold: crossing.threshold,
      rearm_threshold: crossing.rearm_threshold,
    };
    ensure!(
      crossing.has_valid_hysteresis(),
      Error::<T>::InvalidTriggerConfiguration
    );
    if ActorContracts::<T>::get(actor_id).is_some_and(|contract| contract.trigger == *trigger) {
      return Ok(());
    }
    let (current, installed_at_revision) = match T::ObservationProvider::current(&crossing.feed) {
      crate::CanonicalObservationState::Available { value, revision } => (value, revision),
      crate::CanonicalObservationState::Unavailable => {
        return Err(Error::<T>::ObservationUnavailable.into());
      }
      crate::CanonicalObservationState::Uninitialized => {
        return Err(Error::<T>::ObservationUninitialized.into());
      }
    };
    let phase = crossing.initial_phase(current);
    let generation = previous_generation
      .checked_add(1)
      .ok_or(Error::<T>::CrossingGenerationExhausted)?;
    Self::remove_crossing_membership(actor_id)?;
    Self::insert_crossing_member(actor_id, crossing, phase, generation, installed_at_revision)
  }
}
