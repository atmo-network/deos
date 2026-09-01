use crate::ObservationProvider as _;
use crate::pallet::*;
use crate::scheduler::ActivationOutcome;
use crate::types::{
  CrossingDirection, CrossingLeafKey, CrossingLeafState, CrossingMember, CrossingMembershipLocator,
  CrossingMembershipRole, CrossingPhase, CrossingRadixNodeKey, CrossingTransition,
  CrossingTraversal, ObservationCrossing, TriggerFamily, TriggerRuntimeState,
};
use alloc::vec::Vec;
use polkadot_sdk::frame_support::{
  BoundedVec, ensure, storage::TransactionOutcome, traits::Get, weights::Weight,
};
use polkadot_sdk::sp_runtime::{DispatchError, DispatchResult};

const RADIX_DEPTH: u8 = 32;

pub(crate) enum CrossingMembershipTransition<FeedId> {
  Remove,
  Preserve {
    phase: CrossingPhase,
    installed_at_revision: u64,
  },
  Replace {
    crossing: ObservationCrossing<FeedId>,
    phase: CrossingPhase,
    generation: u64,
    installed_at_revision: u64,
  },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CrossingCandidateAuthority<FeedId> {
  pub member: CrossingMember,
  pub locator: CrossingMembershipLocator<FeedId>,
}

pub(crate) struct CrossingCohortSnapshot<FeedId, MaxCandidates: Get<u32>> {
  pub key: CrossingLeafKey<FeedId>,
  pub page: u32,
  pub start_offset: u32,
  pub end_offset: u32,
  pub candidates: BoundedVec<CrossingCandidateAuthority<FeedId>, MaxCandidates>,
}

pub(crate) struct CrossingCohortPreflight<T: Config> {
  pub plan: CrossingWorkPlan,
  pub admitted_candidates: u32,
  pub placed_immediate_fifo: Option<bool>,
  pub queue_candidates: Vec<(ActorId, ActorHotStateOf<T>)>,
}

/// `(tail_page, available_suffix_members)` granted after the bounded tail-page probe.
type CrossingTailRefillAuthority = (u32, u32);

struct CrossingPlacedCohortAuthority<T: Config> {
  feed: T::ObservationFeedId,
  transition: CrossingTransitionObligation,
  cursor: crate::CrossingRangeCursor,
  candidates:
    BoundedVec<CrossingCandidateAuthority<T::ObservationFeedId>, T::MaxCrossingActorsPerBlock>,
  crossings: BoundedVec<ObservationCrossing<T::ObservationFeedId>, T::MaxCrossingActorsPerBlock>,
  tail_refill: Option<CrossingCohortSnapshot<T::ObservationFeedId, T::CrossingPageSize>>,
  queue_plan: crate::scheduler::QueueAppendPlan<T>,
}

impl<T: Config> CrossingPlacedCohortAuthority<T> {
  fn is_coherent(&self) -> bool {
    let _queue_authority = &self.queue_plan;
    self.candidates.len() >= 2
      && self.candidates.len() == self.crossings.len()
      && self.cursor.revision == self.transition.revision
      && self
        .tail_refill
        .as_ref()
        .is_none_or(|snapshot| snapshot.key.feed == self.feed)
      && self
        .candidates
        .iter(/* deos-bypass: bounded-iter */)
        .zip(self.crossings.iter(/* deos-bypass: bounded-iter */))
        .all(
        |(candidate, crossing)| {
          candidate.locator.key.feed == self.feed && crossing.feed == self.feed
        },
      )
  }
}

#[derive(Clone, Copy)]
pub(crate) enum CrossingFireClassification {
  Deferred,
  Resolve,
}

#[derive(Clone, Copy)]
enum CrossingFeedQueueDisposition {
  Preserve,
  ClearIfFeedEmpty,
}

#[derive(Clone, Copy)]
pub(crate) enum CrossingRewriteDisposition {
  Commit,
  #[cfg(test)]
  Preview,
}

#[cfg(test)]
#[derive(Clone, Copy)]
pub(crate) enum PlacedCohortFixtureFault {
  None,
  MalformedLaterLocator,
}

#[cfg(test)]
impl PlacedCohortFixtureFault {
  fn malforms_later_locator(self) -> bool {
    matches!(self, Self::MalformedLaterLocator)
  }
}

impl CrossingFireClassification {
  fn resolves_fire(self) -> bool {
    matches!(self, Self::Resolve)
  }
}

struct CrossingWorkClassification {
  plan: CrossingWorkPlan,
  admitted_candidates: u32,
  tail_refill: Option<CrossingTailRefillAuthority>,
}

struct CrossingWorkOutcome {
  has_more: bool,
  transitions: u32,
  leaves: u32,
  pages: u32,
  actors: u32,
  canonical_probes: u32,
  activations: u32,
  closes: u32,
}

impl CrossingWorkOutcome {
  const fn new(has_more: bool, transitions: u32, leaves: u32, pages: u32, actors: u32) -> Self {
    Self {
      has_more,
      transitions,
      leaves,
      pages,
      actors,
      canonical_probes: 0,
      activations: 0,
      closes: 0,
    }
  }

  const fn with_activation(mut self, closed: bool) -> Self {
    self.canonical_probes = 1;
    self.activations = 1;
    self.closes = closed as u32;
    self
  }

  const fn with_canonical_probe(mut self) -> Self {
    self.canonical_probes = 1;
    self
  }

  fn combine(self, next: Self) -> Self {
    Self {
      has_more: next.has_more,
      transitions: self.transitions.saturating_add(next.transitions),
      leaves: self.leaves.saturating_add(next.leaves),
      pages: self.pages.saturating_add(next.pages),
      actors: self.actors.saturating_add(next.actors),
      canonical_probes: self.canonical_probes.saturating_add(next.canonical_probes),
      activations: self.activations.saturating_add(next.activations),
      closes: self.closes.saturating_add(next.closes),
    }
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CrossingWorkCounters {
  pub transitions: u32,
  pub leaves: u32,
  pub pages: u32,
  pub candidates: u32,
  pub canonical_probes: u32,
  pub activations: u32,
  pub closes: u32,
  pub faults: u32,
}

impl<T: Config> Pallet<T> {
  #[cfg(feature = "try-runtime")]
  pub(crate) fn do_try_state_crossing() -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
    use alloc::collections::{BTreeMap, BTreeSet};
    use polkadot_sdk::sp_runtime::TryRuntimeError;

    let invalid = || TryRuntimeError::Other("Crossing index topology is inconsistent");
    let mut expected_feed_counts = BTreeMap::new();
    let mut expected_user_feed_counts = BTreeMap::new();
    let mut expected_radix = BTreeMap::<CrossingRadixNodeKeyOf<T>, u16>::new();
    let mut indexed_actors = BTreeSet::new();
    let page_size = T::CrossingPageSize::get();

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
          let crate::LoadedActorStateOf::Active(actor) =
            Self::load_frame_actor_state(member.actor_id)
          else {
            return Err(invalid());
          };
          if matches!(actor.identity.actor_class, ActorClass::User { .. }) {
            *expected_user_feed_counts.entry(key.feed).or_insert(0u32) = expected_user_feed_counts
              .get(&key.feed)
              .copied()
              .unwrap_or(0)
              .checked_add(1)
              .ok_or_else(&invalid)?;
          }
          let crossing =
            Self::crossing_from_trigger(&actor.contract.trigger).ok_or_else(&invalid)?;
          let TriggerRuntimeState::ObservationCrossing { phase, .. } =
            actor.hot.trigger_runtime_state
          else {
            return Err(invalid());
          };
          let (expected_key, _) = Self::crossing_obligation(&crossing, phase);
          if expected_key != key {
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
    if CrossingMemberPages::<T>::iter().any(|(key, page_id, _)| {
      CrossingLeafStates::<T>::get(key).is_none_or(|state| page_id > state.tail_page)
    }) || CrossingMemberships::<T>::iter_keys()
      .any(|actor_id| !indexed_actors.contains(&actor_id))
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
    let actual_user_feed_counts: BTreeMap<_, _> =
      CrossingUserFeedMembershipCount::<T>::iter().collect();
    if actual_user_feed_counts != expected_user_feed_counts {
      return Err(invalid());
    }
    let control_entries = Self::frame_control_entries().ok_or_else(&invalid)?;
    for (actor_id, _, _) in control_entries {
      let crate::LoadedActorStateOf::Active(state) = Self::load_frame_actor_state(actor_id) else {
        return Err(invalid());
      };
      let hot = state.hot;
      let contract_crossing = Self::crossing_from_trigger(&state.contract.trigger);
      let runtime_crossing = match hot.trigger_runtime_state {
        TriggerRuntimeState::ObservationCrossing { phase, .. } => Some(phase),
        TriggerRuntimeState::Stateless
        | TriggerRuntimeState::AtTime { .. }
        | TriggerRuntimeState::Cadenced { .. } => None,
      };
      if contract_crossing.is_some() != runtime_crossing.is_some()
        || runtime_crossing.is_some() != CrossingMemberships::<T>::contains_key(actor_id)
      {
        return Err(invalid());
      }
      if let (Some(crossing), Some(phase), Some(locator)) = (
        contract_crossing,
        runtime_crossing,
        CrossingMemberships::<T>::get(actor_id),
      ) {
        let (expected_key, _) = Self::crossing_obligation(&crossing, phase);
        if locator.key != expected_key || !indexed_actors.contains(&actor_id) {
          return Err(invalid());
        }
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

  #[cfg(test)]
  fn insert_crossing_member(
    actor_id: ActorId,
    crossing: ObservationCrossing<T::ObservationFeedId>,
    phase: CrossingPhase,
    generation: u64,
    actor_type: ActorType,
  ) -> DispatchResult {
    let admission_identity = Self::load_control_admission(actor_id)
      .map(|certificate| certificate.admission_identity)
      .unwrap_or([0; 32]);
    Self::insert_crossing_member_with_admission_identity(
      actor_id,
      crossing,
      phase,
      generation,
      actor_type,
      admission_identity,
    )
  }

  fn insert_crossing_member_with_admission_identity(
    actor_id: ActorId,
    crossing: ObservationCrossing<T::ObservationFeedId>,
    phase: CrossingPhase,
    generation: u64,
    actor_type: ActorType,
    admission_identity: [u8; 32],
  ) -> DispatchResult {
    let is_user = actor_type == ActorType::User;
    ensure!(
      !CrossingMemberships::<T>::contains_key(actor_id),
      Error::<T>::CrossingIndexInvariant
    );
    let page_size = T::CrossingPageSize::get();
    ensure!(page_size > 0, Error::<T>::CrossingIndexInvariant);
    let (key, _) = Self::crossing_obligation(&crossing, phase);
    let counterpart_threshold = match phase {
      CrossingPhase::Armed => crossing.rearm_threshold,
      CrossingPhase::WaitingForRearm => crossing.threshold,
    };
    ensure!(
      CrossingFeedMembershipCount::<T>::get(key.feed) < T::MaxCrossingMembersPerFeed::get(),
      Error::<T>::CrossingIndexCapacityExceeded
    );
    if is_user {
      ensure!(
        CrossingUserFeedMembershipCount::<T>::get(key.feed)
          < T::MaxUserCrossingMembersPerFeed::get(),
        Error::<T>::CrossingUserCapacityExceeded
      );
    }
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
        counterpart_threshold,
        admission_identity,
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
        page: page_id,
        offset,
        generation,
      },
    );
    CrossingFeedMembershipCount::<T>::try_mutate(key.feed, |count| -> DispatchResult {
      *count = count
        .checked_add(1)
        .ok_or(Error::<T>::CrossingIndexCapacityExceeded)?;
      Ok(())
    })?;
    if is_user {
      CrossingUserFeedMembershipCount::<T>::try_mutate(key.feed, |count| -> DispatchResult {
        *count = count
          .checked_add(1)
          .ok_or(Error::<T>::CrossingUserCapacityExceeded)?;
        Ok(())
      })?;
    }
    Ok(())
  }

  pub(crate) fn sync_crossing_compiled_authority(
    actor_id: ActorId,
    crossing: ObservationCrossing<T::ObservationFeedId>,
    phase: CrossingPhase,
    admission_identity: [u8; 32],
  ) -> DispatchResult {
    let locator =
      CrossingMemberships::<T>::get(actor_id).ok_or(Error::<T>::CrossingIndexInvariant)?;
    let counterpart_threshold = match phase {
      CrossingPhase::Armed => crossing.rearm_threshold,
      CrossingPhase::WaitingForRearm => crossing.threshold,
    };
    CrossingMemberPages::<T>::try_mutate(
      locator.key,
      locator.page,
      |maybe_page| -> DispatchResult {
        let member = maybe_page
          .as_mut()
          .and_then(|page| page.entries.get_mut(locator.offset as usize))
          .ok_or(Error::<T>::CrossingIndexInvariant)?;
        ensure!(
          member.actor_id == actor_id && member.generation == locator.generation,
          Error::<T>::CrossingIndexInvariant
        );
        member.counterpart_threshold = counterpart_threshold;
        member.admission_identity = admission_identity;
        Ok(())
      },
    )
  }

  fn remove_crossing_member(
    actor_id: ActorId,
    feed_queue: CrossingFeedQueueDisposition,
    actor_type: ActorType,
  ) -> DispatchResult {
    let is_user = actor_type == ActorType::User;
    let Some(locator) = CrossingMemberships::<T>::get(actor_id) else {
      return Ok(());
    };
    let mut state =
      CrossingLeafStates::<T>::get(locator.key).ok_or(Error::<T>::CrossingIndexInvariant)?;
    ensure!(state.member_count > 0, Error::<T>::CrossingIndexInvariant);
    let mut page = CrossingMemberPages::<T>::get(locator.key, locator.page)
      .ok_or(Error::<T>::CrossingIndexInvariant)?;
    ensure!(
      page
        .entries
        .get(locator.offset as usize)
        .is_some_and(|member| {
          member.actor_id == actor_id && member.generation == locator.generation
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
    if is_user {
      CrossingUserFeedMembershipCount::<T>::try_mutate_exists(
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
    }
    if matches!(feed_queue, CrossingFeedQueueDisposition::ClearIfFeedEmpty)
      && CrossingFeedMembershipCount::<T>::get(locator.key.feed) == 0
    {
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

  fn remove_crossing_member_preserving_feed_queue(
    actor_id: ActorId,
    actor_type: ActorType,
  ) -> DispatchResult {
    Self::remove_crossing_member(actor_id, CrossingFeedQueueDisposition::Preserve, actor_type)
  }

  pub(crate) fn remove_crossing_membership(
    actor_id: ActorId,
    actor_type: ActorType,
  ) -> DispatchResult {
    Self::remove_crossing_member(
      actor_id,
      CrossingFeedQueueDisposition::ClearIfFeedEmpty,
      actor_type,
    )
  }

  fn move_crossing_membership_with_authority(
    actor_id: ActorId,
    crossing: ObservationCrossing<T::ObservationFeedId>,
    next_phase: CrossingPhase,
    locator: CrossingMembershipLocator<T::ObservationFeedId>,
  ) -> DispatchResult {
    let (identity, _, admission) =
      Self::load_control_authority_with_authority(actor_id).ok_or(Error::<T>::ActorInvariant)?;
    Self::move_crossing_membership_index_with_authority(
      actor_id,
      crossing,
      next_phase,
      locator,
      identity.actor_class.actor_type(),
      admission.admission_identity,
    )?;
    Self::try_mutate_control_hot_with_authority(
      actor_id,
      Error::<T>::ActorInvariant,
      |hot| -> DispatchResult {
        let TriggerRuntimeState::ObservationCrossing {
          installed_at_revision,
          ..
        } = hot.trigger_runtime_state
        else {
          return Err(Error::<T>::ActorInvariant.into());
        };
        hot.trigger_runtime_state = TriggerRuntimeState::ObservationCrossing {
          phase: next_phase,
          installed_at_revision,
        };
        Ok(())
      },
    )
  }

  fn move_crossing_membership_index_with_authority(
    actor_id: ActorId,
    crossing: ObservationCrossing<T::ObservationFeedId>,
    next_phase: CrossingPhase,
    locator: CrossingMembershipLocator<T::ObservationFeedId>,
    actor_type: ActorType,
    admission_identity: [u8; 32],
  ) -> DispatchResult {
    Self::remove_crossing_member_preserving_feed_queue(actor_id, actor_type)?;
    Self::insert_crossing_member_with_admission_identity(
      actor_id,
      crossing,
      next_phase,
      locator.generation,
      actor_type,
      admission_identity,
    )
  }

  #[cfg(test)]
  fn move_crossing_membership_fixture_without_hot(
    actor_id: ActorId,
    crossing: ObservationCrossing<T::ObservationFeedId>,
    next_phase: CrossingPhase,
    locator: CrossingMembershipLocator<T::ObservationFeedId>,
  ) -> DispatchResult {
    let actor_type = Self::load_control_identity(actor_id)
      .ok_or(Error::<T>::ActorInvariant)?
      .actor_class
      .actor_type();
    Self::remove_crossing_member_preserving_feed_queue(actor_id, actor_type)?;
    Self::insert_crossing_member(
      actor_id,
      crossing,
      next_phase,
      locator.generation,
      actor_type,
    )
  }

  #[cfg(all(test, feature = "runtime-benchmarks"))]
  pub(crate) fn control_move_crossing_membership_without_hot(
    actor_id: ActorId,
    crossing: ObservationCrossing<T::ObservationFeedId>,
    next_phase: CrossingPhase,
    locator: CrossingMembershipLocator<T::ObservationFeedId>,
    actor_type: ActorType,
  ) -> DispatchResult {
    Self::remove_crossing_member_preserving_feed_queue(actor_id, actor_type)?;
    Self::insert_crossing_member(
      actor_id,
      crossing,
      next_phase,
      locator.generation,
      actor_type,
    )
  }

  #[cfg(test)]
  pub(crate) fn test_move_crossing_membership_without_hot(
    actor_id: ActorId,
    crossing: ObservationCrossing<T::ObservationFeedId>,
    next_phase: CrossingPhase,
    locator: CrossingMembershipLocator<T::ObservationFeedId>,
  ) -> Result<bool, DispatchError> {
    let original_hot = Self::load_frame_control_authority(actor_id)
      .map(|(_, _, hot, _)| hot)
      .ok_or(Error::<T>::ActorInvariant)?;
    let (expected_key, _) = Self::crossing_obligation(&crossing, next_phase);
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      if let Err(error) =
        Self::move_crossing_membership_fixture_without_hot(actor_id, crossing, next_phase, locator)
      {
        return TransactionOutcome::Rollback(Err(error));
      }
      let preserved = Self::load_frame_control_authority(actor_id)
        .is_some_and(|(_, _, hot, _)| hot == original_hot)
        && CrossingMemberships::<T>::get(actor_id).is_some_and(|moved| moved.key == expected_key);
      TransactionOutcome::Rollback(Ok(preserved))
    })
  }

  #[cfg(test)]
  pub(crate) fn test_derived_tail_locator_after_first_movement(
    first: ActorId,
    tail: ActorId,
    crossing: ObservationCrossing<T::ObservationFeedId>,
    first_locator: CrossingMembershipLocator<T::ObservationFeedId>,
  ) -> Result<bool, DispatchError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      if let Err(error) = Self::move_crossing_membership_fixture_without_hot(
        first,
        crossing,
        CrossingPhase::WaitingForRearm,
        first_locator,
      ) {
        return TransactionOutcome::Rollback(Err(error));
      }
      let derived = CrossingMemberships::<T>::get(tail).is_some_and(|locator| {
        locator.key == first_locator.key
          && locator.page == first_locator.page
          && locator.offset == first_locator.offset
      });
      TransactionOutcome::Rollback(Ok(derived))
    })
  }

  #[cfg(test)]
  pub(crate) fn test_split_destination_pair_movements_without_hot(
    first: ActorId,
    tail: ActorId,
    first_crossing: ObservationCrossing<T::ObservationFeedId>,
    tail_crossing: ObservationCrossing<T::ObservationFeedId>,
    first_locator: CrossingMembershipLocator<T::ObservationFeedId>,
  ) -> Result<bool, DispatchError> {
    let first_hot = Self::load_frame_control_authority(first)
      .map(|(_, _, hot, _)| hot)
      .ok_or(Error::<T>::ActorInvariant)?;
    let tail_hot = Self::load_frame_control_authority(tail)
      .map(|(_, _, hot, _)| hot)
      .ok_or(Error::<T>::ActorInvariant)?;
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      if let Err(error) = Self::move_crossing_membership_fixture_without_hot(
        first,
        first_crossing,
        CrossingPhase::WaitingForRearm,
        first_locator,
      ) {
        return TransactionOutcome::Rollback(Err(error));
      }
      let Some(derived_tail) = CrossingMemberships::<T>::get(tail) else {
        return TransactionOutcome::Rollback(Err(Error::<T>::CrossingIndexInvariant.into()));
      };
      if let Err(error) = Self::move_crossing_membership_fixture_without_hot(
        tail,
        tail_crossing,
        CrossingPhase::WaitingForRearm,
        derived_tail,
      ) {
        return TransactionOutcome::Rollback(Err(error));
      }
      let split = CrossingMemberships::<T>::get(first)
        .zip(CrossingMemberships::<T>::get(tail))
        .is_some_and(|(first_moved, tail_moved)| first_moved.key != tail_moved.key)
        && Self::load_frame_control_authority(first).is_some_and(|(_, _, hot, _)| hot == first_hot)
        && Self::load_frame_control_authority(tail).is_some_and(|(_, _, hot, _)| hot == tail_hot);
      TransactionOutcome::Rollback(Ok(split))
    })
  }

  fn commit_placed_pair_authority(authority: CrossingPlacedCohortAuthority<T>) -> DispatchResult {
    ensure!(
      authority.candidates.len() == 2,
      Error::<T>::CrossingIndexInvariant
    );
    let first = authority.candidates[0];
    let tail = authority.candidates[1];
    let first_crossing = authority.crossings[0].clone();
    let tail_crossing = authority.crossings[1].clone();
    Self::move_crossing_membership_with_authority(
      first.member.actor_id,
      first_crossing,
      CrossingPhase::WaitingForRearm,
      first.locator,
    )?;
    let derived_tail = CrossingMemberships::<T>::get(tail.member.actor_id)
      .ok_or(Error::<T>::CrossingIndexInvariant)?;
    ensure!(
      derived_tail.key == first.locator.key
        && derived_tail.page == first.locator.page
        && derived_tail.offset == first.locator.offset
        && derived_tail.generation == tail.member.generation,
      Error::<T>::CrossingIndexInvariant
    );
    Self::move_crossing_membership_with_authority(
      tail.member.actor_id,
      tail_crossing,
      CrossingPhase::WaitingForRearm,
      derived_tail,
    )?;
    Self::commit_paged_enqueue(authority.queue_plan).map_err(|_| Error::<T>::ActorInvariant)?;
    Ok(())
  }

  #[cfg(test)]
  pub(crate) fn test_atomic_placed_pair_commit_prototype(
    first: ActorId,
    tail: ActorId,
    first_locator: CrossingMembershipLocator<T::ObservationFeedId>,
  ) -> Result<bool, DispatchError> {
    let mut first_hot = Self::load_frame_control_authority(first)
      .map(|(_, _, hot, _)| hot)
      .ok_or(Error::<T>::ActorInvariant)?;
    let mut tail_hot = Self::load_frame_control_authority(tail)
      .map(|(_, _, hot, _)| hot)
      .ok_or(Error::<T>::ActorInvariant)?;
    for hot in [&mut first_hot, &mut tail_hot] {
      let TriggerRuntimeState::ObservationCrossing {
        installed_at_revision,
        ..
      } = hot.trigger_runtime_state
      else {
        return Err(Error::<T>::ActorInvariant.into());
      };
      hot.trigger_runtime_state = TriggerRuntimeState::ObservationCrossing {
        phase: CrossingPhase::WaitingForRearm,
        installed_at_revision,
      };
      hot.pending_signal = true;
    }
    let page = CrossingMemberPages::<T>::get(first_locator.key, first_locator.page)
      .ok_or(Error::<T>::CrossingIndexInvariant)?;
    let first_member = *page
      .entries
      .get(first_locator.offset as usize)
      .ok_or(Error::<T>::CrossingIndexInvariant)?;
    let tail_offset = page.entries.len().saturating_sub(1) as u32;
    let tail_member = *page
      .entries
      .get(tail_offset as usize)
      .ok_or(Error::<T>::CrossingIndexInvariant)?;
    ensure!(
      first_member.actor_id == first && tail_member.actor_id == tail,
      Error::<T>::CrossingIndexInvariant
    );
    let feed = first_locator.key.feed;
    let transition = CrossingTransitionObligation {
      revision: 1,
      previous: first_locator.key.threshold,
      current: first_locator.key.threshold,
      cause_provenance: crate::TriggerCauseProvenance::Deferred,
      cause_block: 0,
    };
    let cursor = CrossingRangeCursor {
      revision: transition.revision,
      traversal: first_locator.key.traversal,
      search_bound: first_locator.key.threshold,
      current_threshold: Some(first_locator.key.threshold),
      page: first_locator.page,
      offset: first_locator.offset,
      exhausted: false,
    };
    let authority = Self::build_placed_cohort_authority(
      feed,
      transition,
      cursor,
      alloc::vec![
        CrossingCandidateAuthority {
          member: first_member,
          locator: first_locator,
        },
        CrossingCandidateAuthority {
          member: tail_member,
          locator: CrossingMembershipLocator {
            key: first_locator.key,
            page: first_locator.page,
            offset: tail_offset,
            generation: tail_member.generation,
          },
        },
      ],
      alloc::vec![(first, first_hot), (tail, tail_hot)],
    )?;
    Self::test_reset_queue_append_commits();
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      if let Err(error) = Self::commit_placed_pair_authority(authority) {
        return TransactionOutcome::Rollback(Err(error));
      }
      let committed = [first, tail].into_iter().all(|actor_id| {
        Self::load_frame_control_authority(actor_id).is_some_and(|(_, _, hot, _)| {
          hot.pending_signal
            && hot.queue_ticket.is_some()
            && matches!(
              hot.trigger_runtime_state,
              TriggerRuntimeState::ObservationCrossing {
                phase: CrossingPhase::WaitingForRearm,
                ..
              }
            )
        })
      }) && Self::test_queue_append_commits() == 1;
      TransactionOutcome::Rollback(Ok(committed))
    })
  }

  pub(crate) fn crossing_radix_min_ge(
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

  fn classify_fire_activation(
    actor_id: ActorId,
    hot: ActorHotStateOf<T>,
    contract: ActorContractOf<T>,
    _transition: CrossingTransitionObligation,
  ) -> Result<(CrossingWorkPlan, bool, Option<ActorHotStateOf<T>>), DispatchError> {
    let mut loaded = match Self::load_actor_state_with_authority(actor_id) {
      LoadedActorStateOf::Active(state) => state,
      _ => return Err(Error::<T>::ActorInvariant.into()),
    };
    ensure!(
      loaded.hot == hot && loaded.contract == contract,
      Error::<T>::ActorInvariant
    );
    let TriggerRuntimeState::ObservationCrossing {
      installed_at_revision,
      ..
    } = loaded.hot.trigger_runtime_state
    else {
      return Err(Error::<T>::ActorInvariant.into());
    };
    loaded.hot.trigger_runtime_state = TriggerRuntimeState::ObservationCrossing {
      phase: CrossingPhase::WaitingForRearm,
      installed_at_revision,
    };
    let activation = Self::preflight_activation_loaded(actor_id, loaded)
      .map_err(|_| Error::<T>::ActorInvariant)?;
    if activation.terminal_reason.is_some() || ActorReadyTail::<T>::get() == u64::MAX {
      Ok((CrossingWorkPlan::FireCohortClosed, false, None))
    } else if activation.already_pending {
      Ok((CrossingWorkPlan::FireCohortCoalesced, false, None))
    } else {
      let immediate_fifo = matches!(
        activation.action,
        crate::scheduler::ActivationAction::PrimeSchedule(Ok(
          crate::scheduler::PrimeSchedulePlan::Enqueue
        ))
      );
      let queue_hot = immediate_fifo.then_some(activation.prospective_hot);
      Ok((
        CrossingWorkPlan::FireCohortPlaced,
        immediate_fifo,
        queue_hot,
      ))
    }
  }

  fn classify_crossing_candidate(
    member: CrossingMember,
    transition: CrossingTransitionObligation,
    fire_classification: CrossingFireClassification,
  ) -> Result<(CrossingWorkPlan, bool, Option<ActorHotStateOf<T>>), DispatchError> {
    if IndexedTriggerDetectionDisabled::<T>::contains_key(member.actor_id) {
      return Ok((
        CrossingWorkPlan::SkipPostInstallationTransition,
        false,
        None,
      ));
    }
    let LoadedActorStateOf::Active(loaded) = Self::load_actor_state_with_authority(member.actor_id)
    else {
      return Err(Error::<T>::ActorInvariant.into());
    };
    let hot = loaded.hot;
    let contract = loaded.contract;
    let TriggerRuntimeState::ObservationCrossing {
      phase,
      installed_at_revision,
    } = hot.trigger_runtime_state
    else {
      return Err(Error::<T>::ActorInvariant.into());
    };
    if installed_at_revision >= transition.revision {
      return Ok((
        CrossingWorkPlan::SkipPostInstallationTransition,
        false,
        None,
      ));
    }
    let crossing =
      Self::crossing_from_trigger(&contract.trigger).ok_or(Error::<T>::CrossingIndexInvariant)?;
    let transition_kind = crossing.transition(phase, transition.previous, transition.current);
    let (_, role) = Self::crossing_obligation(&crossing, phase);
    ensure!(
      matches!(
        (role, transition_kind),
        (CrossingMembershipRole::Fire, CrossingTransition::Fire)
          | (CrossingMembershipRole::Rearm, CrossingTransition::Rearm)
      ),
      Error::<T>::CrossingIndexInvariant
    );
    if transition_kind == CrossingTransition::Rearm {
      return Ok((CrossingWorkPlan::RearmCohort, false, None));
    }
    if !fire_classification.resolves_fire() {
      return Ok((CrossingWorkPlan::FireCohortPending, false, None));
    }
    Self::classify_fire_activation(member.actor_id, hot, contract, transition)
  }

  fn classify_crossing_idle_candidate(
    candidate: &CrossingCandidateAuthority<T::ObservationFeedId>,
    transition: CrossingTransitionObligation,
    fire_classification: CrossingFireClassification,
  ) -> Result<Option<(CrossingWorkPlan, bool, Option<ActorHotStateOf<T>>)>, DispatchError> {
    let member = candidate.member;
    if IndexedTriggerDetectionDisabled::<T>::contains_key(member.actor_id) {
      return Ok(Some((
        CrossingWorkPlan::SkipPostInstallationTransition,
        false,
        None,
      )));
    }
    if candidate.locator.generation != member.generation
      || candidate.locator.key.traversal
        != if transition.current > transition.previous {
          CrossingTraversal::Upward
        } else {
          CrossingTraversal::Downward
        }
    {
      return Err(Error::<T>::CrossingIndexInvariant.into());
    }
    let Some(state) = Self::load_crossing_idle_activation_state_with_authority(
      member.actor_id,
      candidate.locator.key.feed,
    ) else {
      return Ok(None);
    };
    if member.admission_identity != state.authority.admission_identity {
      return Ok(None);
    }
    let TriggerRuntimeState::ObservationCrossing {
      phase,
      installed_at_revision,
    } = state.hot.trigger_runtime_state
    else {
      return Err(Error::<T>::CrossingIndexInvariant.into());
    };
    if installed_at_revision >= transition.revision {
      return Ok(Some((
        CrossingWorkPlan::SkipPostInstallationTransition,
        false,
        None,
      )));
    }
    if phase == CrossingPhase::WaitingForRearm {
      return Ok(Some((CrossingWorkPlan::RearmCohort, false, None)));
    }
    if !fire_classification.resolves_fire() {
      return Ok(Some((CrossingWorkPlan::FireCohortPending, false, None)));
    }
    let classification = Self::classify_observation_activation_compact(&state)
      .map_err(|_| Error::<T>::ActorInvariant)?;
    if classification.terminal_reason.is_some() || ActorReadyTail::<T>::get() == u64::MAX {
      return Ok(Some((CrossingWorkPlan::FireCohortClosed, false, None)));
    }
    if state.hot.pending_signal {
      return Ok(Some((CrossingWorkPlan::FireCohortCoalesced, false, None)));
    }
    if classification.execution_phase != crate::ActorExecutionPhase::Ready {
      return Ok(None);
    }
    let mut queue_hot = state.hot;
    queue_hot.pending_signal = true;
    queue_hot.trigger_runtime_state = TriggerRuntimeState::ObservationCrossing {
      phase: CrossingPhase::WaitingForRearm,
      installed_at_revision,
    };
    Ok(Some((
      CrossingWorkPlan::FireCohortPlaced,
      true,
      Some(queue_hot),
    )))
  }

  pub(crate) fn preflight_crossing_cohort(
    snapshot: &CrossingCohortSnapshot<T::ObservationFeedId, T::CrossingPageSize>,
    transition: CrossingTransitionObligation,
    fire_classification: CrossingFireClassification,
    expected_plan: Option<CrossingWorkPlan>,
  ) -> Result<CrossingCohortPreflight<T>, DispatchError> {
    let mut plan = expected_plan;
    let mut placed_immediate_fifo = None;
    let mut queue_candidates = Vec::new();
    let mut admitted_candidates = 0u32;
    for authority in snapshot.candidates.iter(/* deos-bypass: bounded-iter */) {
      let (candidate_immediate_fifo, queue_hot, truncate_after_candidate) = {
        let compact =
          Self::classify_crossing_idle_candidate(authority, transition, fire_classification)?;
        if compact.is_none() && admitted_candidates > 0 {
          break;
        }
        let truncate_after_candidate = compact.is_none();
        let (candidate_plan, immediate_fifo, queue_hot) = if let Some(compact) = compact {
          compact
        } else {
          Self::classify_crossing_candidate(authority.member, transition, fire_classification)?
        };
        if let Some(expected) = plan {
          if candidate_plan != expected {
            break;
          }
        } else {
          plan = Some(candidate_plan);
        }
        (immediate_fifo, queue_hot, truncate_after_candidate)
      };
      if plan == Some(CrossingWorkPlan::FireCohortPlaced) {
        if placed_immediate_fifo.is_some_and(|expected| expected != candidate_immediate_fifo) {
          break;
        }
        placed_immediate_fifo = Some(candidate_immediate_fifo);
        if let Some(hot) = queue_hot {
          queue_candidates.push((authority.member.actor_id, hot));
        }
      }
      admitted_candidates = admitted_candidates
        .checked_add(1)
        .ok_or(Error::<T>::CrossingIndexInvariant)?;
      if truncate_after_candidate {
        break;
      }
    }
    Ok(CrossingCohortPreflight {
      plan: plan.ok_or(Error::<T>::CrossingIndexInvariant)?,
      admitted_candidates,
      placed_immediate_fifo,
      queue_candidates,
    })
  }

  fn build_placed_cohort_authority(
    feed: T::ObservationFeedId,
    transition: CrossingTransitionObligation,
    cursor: CrossingRangeCursor,
    candidates: Vec<CrossingCandidateAuthority<T::ObservationFeedId>>,
    queue_candidates: Vec<(ActorId, ActorHotStateOf<T>)>,
  ) -> Result<CrossingPlacedCohortAuthority<T>, DispatchError> {
    ensure!(
      candidates.len() >= 2 && queue_candidates.len() == candidates.len(),
      Error::<T>::CrossingIndexInvariant
    );
    let candidates: BoundedVec<_, T::MaxCrossingActorsPerBlock> = candidates
      .try_into()
      .map_err(|_| Error::<T>::CrossingIndexInvariant)?;
    let mut crossings = BoundedVec::default();
    for candidate in &candidates {
      let crossing = ObservationCrossing {
        feed: candidate.locator.key.feed,
        direction: match candidate.locator.key.traversal {
          CrossingTraversal::Upward => CrossingDirection::Rising,
          CrossingTraversal::Downward => CrossingDirection::Falling,
        },
        threshold: candidate.locator.key.threshold,
        rearm_threshold: candidate.member.counterpart_threshold,
      };
      crossings
        .try_push(crossing)
        .map_err(|_| Error::<T>::CrossingIndexInvariant)?;
    }
    let queue_plan = Self::preflight_paged_enqueue_cohort_with_authority(queue_candidates)
      .map_err(|_| Error::<T>::CrossingIndexInvariant)?;
    let authority = CrossingPlacedCohortAuthority {
      feed,
      transition,
      cursor,
      candidates,
      crossings,
      tail_refill: None,
      queue_plan,
    };
    ensure!(authority.is_coherent(), Error::<T>::CrossingIndexInvariant);
    Ok(authority)
  }

  fn insert_crossing_destination_cohort(
    candidates: &BoundedVec<
      CrossingCandidateAuthority<T::ObservationFeedId>,
      T::MaxCrossingActorsPerBlock,
    >,
    crossings: &BoundedVec<ObservationCrossing<T::ObservationFeedId>, T::MaxCrossingActorsPerBlock>,
  ) -> DispatchResult {
    let page_size = T::CrossingPageSize::get();
    ensure!(page_size > 0, Error::<T>::CrossingIndexInvariant);
    let mut processed = alloc::vec![false; candidates.len()];
    let mut user_count = 0u32;
    for group_start in 0..candidates.len() {
      if processed[group_start] {
        continue;
      }
      let (key, _) =
        Self::crossing_obligation(&crossings[group_start], CrossingPhase::WaitingForRearm);
      let mut state = CrossingLeafStates::<T>::get(key).unwrap_or(CrossingLeafState {
        tail_page: 0,
        page_count: 1,
        member_count: 0,
      });
      let insert_radix = state.member_count == 0;
      let mut page = CrossingMemberPages::<T>::get(key, state.tail_page).unwrap_or_default();
      for index in group_start..candidates.len() {
        if processed[index]
          || Self::crossing_obligation(&crossings[index], CrossingPhase::WaitingForRearm).0 != key
        {
          continue;
        }
        if page.entries.len() as u32 == page_size {
          CrossingMemberPages::<T>::insert(key, state.tail_page, page);
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
        let candidate = candidates[index];
        let offset = page.entries.len() as u32;
        let destination_member = {
          let mut member = candidate.member;
          member.counterpart_threshold = crossings[index].threshold;
          member
        };
        page
          .entries
          .try_push(destination_member)
          .map_err(|_| Error::<T>::CrossingIndexCapacityExceeded)?;
        CrossingMemberships::<T>::insert(
          candidate.member.actor_id,
          CrossingMembershipLocator {
            key,
            page: state.tail_page,
            offset,
            generation: candidate.member.generation,
          },
        );
        Self::try_mutate_control_hot_with_authority(
          candidate.member.actor_id,
          Error::<T>::ActorInvariant,
          |hot| -> DispatchResult {
            let TriggerRuntimeState::ObservationCrossing {
              installed_at_revision,
              ..
            } = hot.trigger_runtime_state
            else {
              return Err(Error::<T>::ActorInvariant.into());
            };
            hot.trigger_runtime_state = TriggerRuntimeState::ObservationCrossing {
              phase: CrossingPhase::WaitingForRearm,
              installed_at_revision,
            };
            Ok(())
          },
        )?;
        state.member_count = state
          .member_count
          .checked_add(1)
          .ok_or(Error::<T>::CrossingIndexCapacityExceeded)?;
        user_count = user_count.saturating_add(u32::from(matches!(
          Self::load_control_authority_with_authority(candidate.member.actor_id,)
            .ok_or(Error::<T>::ActorInvariant)?
            .0
            .actor_class,
          ActorClass::User { .. }
        )));
        processed[index] = true;
      }
      if insert_radix {
        Self::insert_crossing_radix_path(&key)?;
      }
      CrossingMemberPages::<T>::insert(key, state.tail_page, page);
      CrossingLeafStates::<T>::insert(key, state);
    }
    let count = candidates.len() as u32;
    let feed = candidates[0].locator.key.feed;
    CrossingFeedMembershipCount::<T>::try_mutate(feed, |total| -> DispatchResult {
      *total = total
        .checked_add(count)
        .ok_or(Error::<T>::CrossingIndexCapacityExceeded)?;
      Ok(())
    })?;
    CrossingUserFeedMembershipCount::<T>::try_mutate(feed, |users| -> DispatchResult {
      *users = users
        .checked_add(user_count)
        .ok_or(Error::<T>::CrossingUserCapacityExceeded)?;
      Ok(())
    })?;
    Ok(())
  }

  fn commit_non_tail_placed_cohort_authority(
    mut authority: CrossingPlacedCohortAuthority<T>,
  ) -> DispatchResult {
    let tail_refill = authority
      .tail_refill
      .take()
      .ok_or(Error::<T>::CrossingIndexInvariant)?;
    let source: CrossingCohortSnapshot<T::ObservationFeedId, T::CrossingPageSize> =
      CrossingCohortSnapshot {
        key: authority.candidates[0].locator.key,
        page: authority.cursor.page,
        start_offset: authority.cursor.offset,
        end_offset: authority
          .cursor
          .offset
          .saturating_add(authority.candidates.len() as u32),
        candidates: authority
          .candidates
          .clone()
          .into_inner()
          .try_into()
          .map_err(|_| Error::<T>::CrossingIndexInvariant)?,
      };
    ensure!(
      Self::rewrite_non_tail_source(&source, &tail_refill, CrossingRewriteDisposition::Commit,)?,
      Error::<T>::CrossingIndexInvariant
    );
    let count = authority.candidates.len() as u32;
    CrossingLeafStates::<T>::try_mutate(source.key, |state| -> DispatchResult {
      let state = state.as_mut().ok_or(Error::<T>::CrossingIndexInvariant)?;
      state.member_count = state
        .member_count
        .checked_sub(count)
        .ok_or(Error::<T>::CrossingIndexInvariant)?;
      Ok(())
    })?;
    CrossingFeedMembershipCount::<T>::try_mutate(source.key.feed, |total| -> DispatchResult {
      *total = total
        .checked_sub(count)
        .ok_or(Error::<T>::CrossingIndexInvariant)?;
      Ok(())
    })?;
    let user_count = authority
      .candidates
      .iter(/* deos-bypass: bounded-iter */)
      .try_fold(0u32, |users, candidate| -> Result<u32, DispatchError> {
        let is_user = matches!(
          Self::load_control_authority_with_authority(
            candidate.member.actor_id,
          )
          .ok_or(Error::<T>::ActorInvariant)?
          .0
          .actor_class,
          ActorClass::User { .. }
        );
        Ok(users.saturating_add(u32::from(is_user)))
      })?;
    CrossingUserFeedMembershipCount::<T>::try_mutate(source.key.feed, |users| -> DispatchResult {
      *users = users
        .checked_sub(user_count)
        .ok_or(Error::<T>::CrossingIndexInvariant)?;
      Ok(())
    })?;
    Self::insert_crossing_destination_cohort(&authority.candidates, &authority.crossings)?;
    Self::commit_paged_enqueue(authority.queue_plan).map_err(|_| Error::<T>::ActorInvariant)?;
    Ok(())
  }

  fn commit_tail_page_placed_cohort_authority(
    authority: CrossingPlacedCohortAuthority<T>,
  ) -> DispatchResult {
    let key = authority.candidates[0].locator.key;
    let state = CrossingLeafStates::<T>::get(key).ok_or(Error::<T>::CrossingIndexInvariant)?;
    let page = CrossingMemberPages::<T>::get(key, authority.cursor.page)
      .ok_or(Error::<T>::CrossingIndexInvariant)?;
    ensure!(
      authority.cursor.page == state.tail_page,
      Error::<T>::CrossingIndexInvariant
    );
    let remainder = Self::stable_crossing_source_remainder(
      &page,
      authority.cursor.offset,
      authority.candidates.len() as u32,
    )?;
    for (index, candidate) in authority
      .candidates
      .iter(/* deos-bypass: bounded-iter */)
      .enumerate()
    {
      ensure!(
        candidate.locator.key == key
          && candidate.locator.page == authority.cursor.page
          && candidate.locator.offset == authority.cursor.offset.saturating_add(index as u32),
        Error::<T>::CrossingIndexInvariant
      );
    }
    for (candidate, crossing) in authority
      .candidates
      .iter(/* deos-bypass: bounded-iter */)
      .zip(authority.crossings.iter(/* deos-bypass: bounded-iter */))
      .rev()
    {
      let locator = CrossingMemberships::<T>::get(candidate.member.actor_id)
        .ok_or(Error::<T>::CrossingIndexInvariant)?;
      ensure!(
        locator == candidate.locator,
        Error::<T>::CrossingIndexInvariant
      );
      Self::move_crossing_membership_with_authority(
        candidate.member.actor_id,
        crossing.clone(),
        CrossingPhase::WaitingForRearm,
        locator,
      )?;
    }
    if !remainder.entries.is_empty() {
      for (offset, member) in remainder
        .entries
        .iter(/* deos-bypass: bounded-iter */)
        .enumerate()
      {
        CrossingMemberships::<T>::insert(
          member.actor_id,
          CrossingMembershipLocator {
            key,
            page: authority.cursor.page,
            offset: offset as u32,
            generation: member.generation,
          },
        );
      }
      CrossingMemberPages::<T>::insert(key, authority.cursor.page, remainder);
    }
    Self::commit_paged_enqueue(authority.queue_plan).map_err(|_| Error::<T>::ActorInvariant)?;
    Ok(())
  }

  #[cfg(test)]
  pub(crate) fn test_placed_cohort_authority_count(
    snapshot: &CrossingCohortSnapshot<T::ObservationFeedId, T::CrossingPageSize>,
    transition: CrossingTransitionObligation,
    fixture_fault: PlacedCohortFixtureFault,
  ) -> Result<usize, DispatchError> {
    let preflight = Self::preflight_crossing_cohort(
      snapshot,
      transition,
      CrossingFireClassification::Resolve,
      Some(CrossingWorkPlan::FireCohortPlaced),
    )?;
    ensure!(
      preflight.admitted_candidates as usize == snapshot.candidates.len()
        && preflight.placed_immediate_fifo == Some(true),
      Error::<T>::CrossingIndexInvariant
    );
    let authority = Self::build_placed_cohort_authority(
      snapshot.key.feed,
      transition,
      CrossingRangeCursor {
        revision: transition.revision,
        traversal: snapshot.key.traversal,
        search_bound: snapshot.key.threshold,
        current_threshold: Some(snapshot.key.threshold),
        page: snapshot.page,
        offset: snapshot.start_offset,
        exhausted: false,
      },
      snapshot.candidates.clone().into_inner(),
      preflight.queue_candidates,
    )?;
    let count = authority.candidates.len();
    let malformed_actor = authority
      .candidates
      .get(2)
      .map(|candidate| candidate.member.actor_id);
    Self::test_reset_queue_append_commits();
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      if fixture_fault.malforms_later_locator() {
        let Some(actor_id) = malformed_actor else {
          return TransactionOutcome::Rollback(Err(Error::<T>::CrossingIndexInvariant.into()));
        };
        CrossingMemberships::<T>::mutate(actor_id, |locator| {
          if let Some(locator) = locator {
            locator.generation = locator.generation.saturating_add(1);
          }
        });
      }
      match Self::commit_tail_page_placed_cohort_authority(authority) {
        Ok(())
          if Self::test_queue_append_commits() == 1 && !fixture_fault.malforms_later_locator() =>
        {
          TransactionOutcome::Rollback(Ok(count))
        }
        Ok(()) => TransactionOutcome::Rollback(Err(Error::<T>::CrossingIndexInvariant.into())),
        Err(_) if fixture_fault.malforms_later_locator() => TransactionOutcome::Rollback(Ok(count)),
        Err(error) => TransactionOutcome::Rollback(Err(error)),
      }
    })
  }

  pub(crate) fn rewrite_non_tail_source(
    source: &CrossingCohortSnapshot<T::ObservationFeedId, T::CrossingPageSize>,
    tail_refill: &CrossingCohortSnapshot<T::ObservationFeedId, T::CrossingPageSize>,
    disposition: CrossingRewriteDisposition,
  ) -> Result<bool, DispatchError> {
    ensure!(
      source.key == tail_refill.key
        && source.page < tail_refill.page
        && source.candidates.len() == tail_refill.candidates.len(),
      Error::<T>::CrossingIndexInvariant
    );
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let result = (|| -> DispatchResult {
        for candidate in source
          .candidates
          .iter(/* deos-bypass: bounded-iter */)
          .chain(tail_refill.candidates.iter(/* deos-bypass: bounded-iter */))
        {
          ensure!(
            CrossingMemberships::<T>::get(candidate.member.actor_id) == Some(candidate.locator),
            Error::<T>::CrossingIndexInvariant
          );
        }
        let source_page = CrossingMemberPages::<T>::get(source.key, source.page)
          .ok_or(Error::<T>::CrossingIndexInvariant)?;
        let tail_page = CrossingMemberPages::<T>::get(tail_refill.key, tail_refill.page)
          .ok_or(Error::<T>::CrossingIndexInvariant)?;
        let mut rewritten = Self::stable_crossing_source_remainder(
          &source_page,
          source.start_offset,
          source.candidates.len() as u32,
        )?;
        for candidate in &tail_refill.candidates {
          rewritten
            .entries
            .try_push(candidate.member)
            .map_err(|_| Error::<T>::CrossingIndexInvariant)?;
        }
        let retained_tail = Self::stable_crossing_source_remainder(
          &tail_page,
          tail_refill.start_offset,
          tail_refill.candidates.len() as u32,
        )?;
        for candidate in &source.candidates {
          CrossingMemberships::<T>::remove(candidate.member.actor_id);
        }
        for (offset, member) in rewritten
          .entries
          .iter(/* deos-bypass: bounded-iter */)
          .enumerate()
        {
          CrossingMemberships::<T>::insert(
            member.actor_id,
            CrossingMembershipLocator {
              key: source.key,
              page: source.page,
              offset: offset as u32,
              generation: member.generation,
            },
          );
        }
        CrossingMemberPages::<T>::insert(source.key, source.page, rewritten);
        if retained_tail.entries.is_empty() {
          CrossingMemberPages::<T>::remove(tail_refill.key, tail_refill.page);
          CrossingLeafStates::<T>::try_mutate(source.key, |state| -> DispatchResult {
            let state = state.as_mut().ok_or(Error::<T>::CrossingIndexInvariant)?;
            ensure!(
              state.tail_page == tail_refill.page,
              Error::<T>::CrossingIndexInvariant
            );
            state.tail_page = state.tail_page.saturating_sub(1);
            state.page_count = state.page_count.saturating_sub(1);
            Ok(())
          })?;
        } else {
          for (offset, member) in retained_tail
            .entries
            .iter(/* deos-bypass: bounded-iter */)
            .enumerate()
          {
            CrossingMemberships::<T>::insert(
              member.actor_id,
              CrossingMembershipLocator {
                key: tail_refill.key,
                page: tail_refill.page,
                offset: offset as u32,
                generation: member.generation,
              },
            );
          }
          CrossingMemberPages::<T>::insert(tail_refill.key, tail_refill.page, retained_tail);
        }
        Ok(())
      })();
      match result {
        Ok(()) if matches!(disposition, CrossingRewriteDisposition::Commit) => {
          TransactionOutcome::Commit(Ok(true))
        }
        Ok(()) => TransactionOutcome::Rollback(Ok(true)),
        Err(error) => TransactionOutcome::Rollback(Err(error)),
      }
    })
  }

  #[cfg(test)]
  pub(crate) fn test_non_tail_placed_authority_count(
    source: &CrossingCohortSnapshot<T::ObservationFeedId, T::CrossingPageSize>,
    tail_refill: &CrossingCohortSnapshot<T::ObservationFeedId, T::CrossingPageSize>,
    transition: CrossingTransitionObligation,
  ) -> Result<usize, DispatchError> {
    ensure!(
      source.key == tail_refill.key
        && source.page < tail_refill.page
        && source.candidates.len() == tail_refill.candidates.len(),
      Error::<T>::CrossingIndexInvariant
    );
    let preflight = Self::preflight_crossing_cohort(
      source,
      transition,
      CrossingFireClassification::Resolve,
      Some(CrossingWorkPlan::FireCohortPlaced),
    )?;
    ensure!(
      preflight.admitted_candidates as usize == source.candidates.len()
        && preflight.placed_immediate_fifo == Some(true),
      Error::<T>::CrossingIndexInvariant
    );
    let mut authority = Self::build_placed_cohort_authority(
      source.key.feed,
      transition,
      CrossingRangeCursor {
        revision: transition.revision,
        traversal: source.key.traversal,
        search_bound: source.key.threshold,
        current_threshold: Some(source.key.threshold),
        page: source.page,
        offset: source.start_offset,
        exhausted: false,
      },
      source.candidates.clone().into_inner(),
      preflight.queue_candidates,
    )?;
    authority.tail_refill = Some(CrossingCohortSnapshot {
      key: tail_refill.key,
      page: tail_refill.page,
      start_offset: tail_refill.start_offset,
      end_offset: tail_refill.end_offset,
      candidates: tail_refill.candidates.clone(),
    });
    ensure!(authority.is_coherent(), Error::<T>::CrossingIndexInvariant);
    let count = authority.candidates.len();
    let feed = authority.feed;
    let cursor = authority.cursor;
    let threshold = source.key.threshold;
    Self::test_reset_queue_append_commits();
    Self::test_reset_crossing_cursor_commits();
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let result = Self::commit_non_tail_placed_cohort_authority(authority).and_then(|()| {
        Self::persist_crossing_cursor_after_movement(
          feed,
          &transition,
          cursor,
          source.key,
          threshold,
        )
      });
      match result {
        Ok(())
          if Self::test_queue_append_commits() == 1
            && Self::test_crossing_cursor_commits() == 1
            && source
              .candidates
              .iter(/* deos-bypass: bounded-iter */)
              .all(|candidate| {
              CrossingMemberships::<T>::get(candidate.member.actor_id)
                .is_some_and(|locator| locator.key != source.key)
                && Self::load_frame_control_authority(candidate.member.actor_id).is_some_and(
                  |(_, _, hot, _)| {
                    hot.pending_signal
                      && hot.queue_ticket.is_some()
                      && matches!(
                        hot.trigger_runtime_state,
                        TriggerRuntimeState::ObservationCrossing {
                          phase: CrossingPhase::WaitingForRearm,
                          ..
                        }
                      )
                  },
                )
            }) =>
        {
          TransactionOutcome::Rollback(Ok(count))
        }
        Ok(()) => TransactionOutcome::Rollback(Err(Error::<T>::CrossingIndexInvariant.into())),
        Err(error) => TransactionOutcome::Rollback(Err(error)),
      }
    })
  }

  fn do_classify_crossing_work(
    fire_classification: CrossingFireClassification,
    max_candidates: u32,
  ) -> Result<CrossingWorkPlan, DispatchError> {
    let list = CrossingPendingFeedListState::<T>::get();
    if list.count == 0 {
      ensure!(
        list.head.is_none() && list.tail.is_none() && list.cursor.is_none(),
        Error::<T>::CrossingTransitionInvariant
      );
      return Ok(CrossingWorkPlan::Empty);
    }
    let feed = list.cursor.ok_or(Error::<T>::CrossingTransitionInvariant)?;
    let queue =
      CrossingTransitionQueues::<T>::get(feed).ok_or(Error::<T>::CrossingTransitionInvariant)?;
    let transition = *queue
      .first()
      .ok_or(Error::<T>::CrossingTransitionInvariant)?;
    let cursor = CrossingRangeCursors::<T>::get(feed)
      .map_or_else(|| Self::initialize_crossing_cursor(transition), Ok)?;
    ensure!(
      cursor.revision == transition.revision,
      Error::<T>::CrossingTransitionInvariant
    );
    if cursor.exhausted {
      return Ok(CrossingWorkPlan::CompleteTransition);
    }
    let Some(threshold) = cursor.current_threshold else {
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
      return Ok(if found.is_some() {
        CrossingWorkPlan::OpenLeaf
      } else {
        CrossingWorkPlan::SeekMiss
      });
    };
    let key = CrossingLeafKey {
      feed,
      traversal: cursor.traversal,
      threshold,
    };
    let state = CrossingLeafStates::<T>::get(key).ok_or(Error::<T>::CrossingIndexInvariant)?;
    if cursor.page > state.tail_page {
      return Ok(CrossingWorkPlan::AdvanceLeaf);
    }
    let page =
      CrossingMemberPages::<T>::get(key, cursor.page).ok_or(Error::<T>::CrossingIndexInvariant)?;
    if cursor.offset as usize >= page.entries.len() {
      return Ok(CrossingWorkPlan::AdvancePage);
    }
    let snapshot =
      Self::snapshot_crossing_source_prefix(key, cursor.page, &page, cursor.offset, 1)?;
    ensure!(
      snapshot.key == key
        && snapshot.page == cursor.page
        && snapshot.start_offset == cursor.offset
        && snapshot.end_offset == cursor.offset.saturating_add(1),
      Error::<T>::CrossingIndexInvariant
    );
    let first_preflight =
      Self::preflight_crossing_cohort(&snapshot, transition, fire_classification, None)?;
    ensure!(
      first_preflight.admitted_candidates == 1,
      Error::<T>::CrossingIndexInvariant
    );
    let first_plan = first_preflight.plan;
    if first_plan == CrossingWorkPlan::SkipPostInstallationTransition {
      let pair_shape = cursor.page == state.tail_page
        && Self::crossing_source_prefix_count(&page, cursor.offset, max_candidates) >= 2;
      if !pair_shape {
        return Ok(CrossingWorkPlan::SkipPostInstallationTransition);
      }
      if !fire_classification.resolves_fire() {
        return Ok(CrossingWorkPlan::SkipPostInstallationPairPending);
      }
      let second_offset = page.entries.len().saturating_sub(1) as u32;
      let second_snapshot =
        Self::snapshot_crossing_source_prefix(key, cursor.page, &page, second_offset, 1)?;
      let second_preflight = Self::preflight_crossing_cohort(
        &second_snapshot,
        transition,
        fire_classification,
        Some(first_plan),
      )?;
      return Ok(if second_preflight.admitted_candidates == 1 {
        CrossingWorkPlan::SkipPostInstallationPair
      } else {
        CrossingWorkPlan::SkipPostInstallationTransition
      });
    }
    if first_plan == CrossingWorkPlan::RearmCohort {
      let pair_shape = cursor.page == state.tail_page
        && Self::crossing_source_prefix_count(&page, cursor.offset, max_candidates) >= 2;
      if !pair_shape {
        return Ok(CrossingWorkPlan::RearmCohort);
      }
      if !fire_classification.resolves_fire() {
        return Ok(CrossingWorkPlan::RearmCohortPairPending);
      }
      let second_offset = page.entries.len().saturating_sub(1) as u32;
      let second_snapshot =
        Self::snapshot_crossing_source_prefix(key, cursor.page, &page, second_offset, 1)?;
      let second_preflight = Self::preflight_crossing_cohort(
        &second_snapshot,
        transition,
        fire_classification,
        Some(first_plan),
      )?;
      return Ok(if second_preflight.admitted_candidates == 1 {
        CrossingWorkPlan::RearmCohortPair
      } else {
        CrossingWorkPlan::RearmCohort
      });
    }
    if !fire_classification.resolves_fire() {
      ensure!(
        first_plan == CrossingWorkPlan::FireCohortPending,
        Error::<T>::CrossingIndexInvariant
      );
      return Ok(
        if cursor.page == state.tail_page
          && Self::crossing_source_prefix_count(&page, cursor.offset, max_candidates) >= 2
        {
          CrossingWorkPlan::FireCohortPairPending
        } else {
          CrossingWorkPlan::FireCohortPending
        },
      );
    }
    if !matches!(
      first_plan,
      CrossingWorkPlan::FireCohortPlaced | CrossingWorkPlan::FireCohortCoalesced
    ) || cursor.page != state.tail_page
      || Self::crossing_source_prefix_count(&page, cursor.offset, max_candidates) < 2
    {
      return Ok(first_plan);
    }
    let second_offset = page.entries.len().saturating_sub(1) as u32;
    let second_snapshot =
      Self::snapshot_crossing_source_prefix(key, cursor.page, &page, second_offset, 1)?;
    let second_preflight = Self::preflight_crossing_cohort(
      &second_snapshot,
      transition,
      fire_classification,
      Some(first_plan),
    )?;
    if second_preflight.admitted_candidates == 0 {
      return Ok(first_plan);
    }
    ensure!(
      second_preflight.admitted_candidates == 1,
      Error::<T>::CrossingIndexInvariant
    );
    let second_plan = second_preflight.plan;
    let pair_has_homogeneous_immediate_fifo = first_preflight.placed_immediate_fifo == Some(true)
      && second_preflight.placed_immediate_fifo == Some(true);
    let mut queue_candidates = first_preflight.queue_candidates;
    queue_candidates.extend(second_preflight.queue_candidates);
    let pair_authority = Self::build_placed_cohort_authority(
      feed,
      transition,
      cursor,
      alloc::vec![snapshot.candidates[0], second_snapshot.candidates[0]],
      queue_candidates,
    )
    .ok();
    Ok(match (first_plan, second_plan) {
      (CrossingWorkPlan::FireCohortPlaced, CrossingWorkPlan::FireCohortPlaced)
        if pair_has_homogeneous_immediate_fifo && pair_authority.is_some() =>
      {
        CrossingWorkPlan::FireCohortPlacedBatch
      }
      (CrossingWorkPlan::FireCohortCoalesced, CrossingWorkPlan::FireCohortCoalesced) => {
        CrossingWorkPlan::FireCohortCoalescedPair
      }
      _ => first_plan,
    })
  }

  pub(crate) fn crossing_source_cohort_count(
    page: &CrossingMemberPageOf<T>,
    offset: u32,
    requested: u32,
    tail_refill_available: Option<u32>,
  ) -> u32 {
    let remaining = (page.entries.len() as u32).saturating_sub(offset);
    let refill_limit = tail_refill_available.unwrap_or(u32::MAX);
    core::cmp::min(
      core::cmp::min(core::cmp::min(requested, remaining), refill_limit),
      T::CrossingPageSize::get(),
    )
  }

  pub(crate) fn crossing_source_prefix_count(
    page: &CrossingMemberPageOf<T>,
    offset: u32,
    requested: u32,
  ) -> u32 {
    Self::crossing_source_cohort_count(page, offset, requested, None)
  }

  pub(crate) fn snapshot_crossing_tail_suffix(
    key: CrossingLeafKeyOf<T>,
    source_page: u32,
    count: u32,
  ) -> Result<CrossingCohortSnapshot<T::ObservationFeedId, T::CrossingPageSize>, DispatchError> {
    let state = CrossingLeafStates::<T>::get(key).ok_or(Error::<T>::CrossingIndexInvariant)?;
    ensure!(
      count > 0 && source_page < state.tail_page,
      Error::<T>::CrossingIndexInvariant
    );
    let tail = CrossingMemberPages::<T>::get(key, state.tail_page)
      .ok_or(Error::<T>::CrossingIndexInvariant)?;
    let offset = (tail.entries.len() as u32)
      .checked_sub(count)
      .ok_or(Error::<T>::CrossingIndexInvariant)?;
    Self::snapshot_crossing_source_prefix(key, state.tail_page, &tail, offset, count)
  }

  pub(crate) fn stable_crossing_source_remainder(
    page: &CrossingMemberPageOf<T>,
    offset: u32,
    count: u32,
  ) -> Result<CrossingMemberPageOf<T>, DispatchError> {
    let start = offset as usize;
    let end = start
      .checked_add(count as usize)
      .ok_or(Error::<T>::CrossingIndexInvariant)?;
    ensure!(
      end <= page.entries.len(),
      Error::<T>::CrossingIndexInvariant
    );
    let mut entries = BoundedVec::default();
    for member in page
      .entries
      .iter(/* deos-bypass: bounded-iter */)
      .take(start)
      .chain(page.entries.iter(/* deos-bypass: bounded-iter */).skip(end))
    {
      entries
        .try_push(*member)
        .map_err(|_| Error::<T>::CrossingIndexInvariant)?;
    }
    Ok(CrossingMemberPage { entries })
  }

  pub(crate) fn snapshot_crossing_source_prefix(
    key: CrossingLeafKeyOf<T>,
    page_id: u32,
    page: &CrossingMemberPageOf<T>,
    offset: u32,
    limit: u32,
  ) -> Result<CrossingCohortSnapshot<T::ObservationFeedId, T::CrossingPageSize>, DispatchError> {
    let start = offset as usize;
    ensure!(
      start <= page.entries.len(),
      Error::<T>::CrossingIndexInvariant
    );
    let count = Self::crossing_source_prefix_count(page, offset, limit) as usize;
    let mut authorities = BoundedVec::default();
    for (relative, member) in page
      .entries
      .iter(/* deos-bypass: bounded-iter */)
      .skip(start)
      .take(count)
      .enumerate()
    {
      let member_offset = offset
        .checked_add(relative as u32)
        .ok_or(Error::<T>::CrossingIndexInvariant)?;
      let locator =
        CrossingMemberships::<T>::get(member.actor_id).ok_or(Error::<T>::CrossingIndexInvariant)?;
      ensure!(
        locator.key == key
          && locator.page == page_id
          && locator.offset == member_offset
          && locator.generation == member.generation,
        Error::<T>::CrossingIndexInvariant
      );
      authorities
        .try_push(CrossingCandidateAuthority {
          member: *member,
          locator,
        })
        .map_err(|_| Error::<T>::CrossingIndexInvariant)?;
    }
    let end_offset = offset
      .checked_add(authorities.len() as u32)
      .ok_or(Error::<T>::CrossingIndexInvariant)?;
    Ok(CrossingCohortSnapshot {
      key,
      page: page_id,
      start_offset: offset,
      end_offset,
      candidates: authorities,
    })
  }

  pub fn classify_crossing_work() -> CrossingWorkPlan {
    Self::classify_crossing_work_with_limit(2).plan
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn classify_crossing_work_preflight() -> CrossingWorkPlan {
    Self::classify_crossing_work_preflight_with_limit(2).plan
  }

  fn crossing_work_classification(plan: CrossingWorkPlan) -> CrossingWorkClassification {
    CrossingWorkClassification {
      plan,
      admitted_candidates: Self::crossing_plan_components(plan).3,
      tail_refill: None,
    }
  }

  fn classify_crossing_work_with_limit(max_candidates: u32) -> CrossingWorkClassification {
    let plan = Self::do_classify_crossing_work(CrossingFireClassification::Resolve, max_candidates)
      .unwrap_or(CrossingWorkPlan::StructuralFault);
    let mut classified = Self::crossing_work_classification(plan);
    let source_is_non_tail = CrossingPendingFeedListState::<T>::get()
      .cursor
      .and_then(|feed| {
        CrossingRangeCursors::<T>::get(feed).and_then(|cursor| {
          cursor.current_threshold.and_then(|threshold| {
            CrossingLeafStates::<T>::get(CrossingLeafKey {
              feed,
              traversal: cursor.traversal,
              threshold,
            })
            .map(|state| cursor.page < state.tail_page)
          })
        })
      })
      .unwrap_or(false);
    if (plan == CrossingWorkPlan::FireCohortPlacedBatch
      || (plan == CrossingWorkPlan::FireCohortPlaced && source_is_non_tail))
      && max_candidates > 1
    {
      if let Ok(authority) = Self::preflight_current_placed_batch_authority(max_candidates) {
        classified.plan = CrossingWorkPlan::FireCohortPlacedBatch;
        classified.admitted_candidates = authority.candidates.len() as u32;
        classified.tail_refill = authority
          .tail_refill
          .as_ref()
          .map(|snapshot| (snapshot.page, snapshot.candidates.len() as u32));
      } else if plan == CrossingWorkPlan::FireCohortPlacedBatch {
        classified.admitted_candidates = 0;
      }
    }
    classified
  }

  fn classify_crossing_work_preflight_with_limit(
    max_candidates: u32,
  ) -> CrossingWorkClassification {
    let plan =
      Self::do_classify_crossing_work(CrossingFireClassification::Deferred, max_candidates)
        .unwrap_or(CrossingWorkPlan::StructuralFault);
    let mut classified = Self::crossing_work_classification(plan);
    if max_candidates > 2 {
      if let Some(feed) = CrossingPendingFeedListState::<T>::get().cursor {
        if let Some(cursor) = CrossingRangeCursors::<T>::get(feed) {
          if let Some(threshold) = cursor.current_threshold {
            let key = CrossingLeafKey {
              feed,
              traversal: cursor.traversal,
              threshold,
            };
            classified.tail_refill = CrossingLeafStates::<T>::get(key)
              .filter(|state| cursor.page < state.tail_page)
              .map(|state| (state.tail_page, 0));
          }
        }
      }
    }
    classified
  }

  fn persist_crossing_cursor_after_movement(
    feed: T::ObservationFeedId,
    transition: &CrossingTransitionObligation,
    mut cursor: CrossingRangeCursor,
    key: CrossingLeafKeyOf<T>,
    threshold: u128,
  ) -> DispatchResult {
    #[cfg(test)]
    Self::test_record_crossing_cursor_commit();
    if CrossingLeafStates::<T>::contains_key(key) {
      cursor.current_threshold = Some(threshold);
    } else {
      Self::advance_crossing_threshold(&mut cursor, transition, threshold);
    }
    CrossingRangeCursors::<T>::insert(feed, cursor);
    Self::advance_crossing_pending_feed(feed)
  }

  fn charge_crossing_fire_occurrence(
    actor_id: ActorId,
  ) -> Result<Option<crate::TriggerFeeBreakdown<T::Balance>>, DispatchError> {
    use crate::weights::WeightInfo as _;

    let (identity, _, _) =
      Self::load_control_authority_with_authority(actor_id).ok_or(Error::<T>::ActorInvariant)?;
    let actor_type = identity.actor_class.actor_type();
    let breakdown = Self::trigger_fee_for_weight(
      actor_type,
      TriggerFamily::ObservationCrossing,
      T::WeightInfo::observation_crossing_trigger_occurrence(),
    );
    Self::try_charge_automatic_trigger_occurrence(
      actor_type,
      &identity.sovereign_account,
      breakdown,
    )
    .map(|charged| charged.then_some(breakdown))
  }

  fn deposit_crossing_fire_occurrence(
    actor_id: ActorId,
    breakdown: crate::TriggerFeeBreakdown<T::Balance>,
  ) {
    Self::deposit_event(Event::TriggerOccurrenceProcessed {
      actor_id,
      trigger_family: breakdown.trigger_family,
      fee: breakdown.trigger_fee,
    });
  }

  fn charge_crossing_placed_cohort(authority: &CrossingPlacedCohortAuthority<T>) -> DispatchResult {
    let mut charged = Vec::with_capacity(authority.candidates.len());
    for candidate in &authority.candidates {
      let Some(breakdown) = Self::charge_crossing_fire_occurrence(candidate.member.actor_id)?
      else {
        return Err(Error::<T>::InsufficientFee.into());
      };
      charged.push((candidate.member.actor_id, breakdown));
    }
    for (actor_id, breakdown) in charged {
      IndexedTriggerDetectionDisabled::<T>::insert(actor_id, ());
      Self::deposit_crossing_fire_occurrence(actor_id, breakdown);
    }
    Ok(())
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
    if IndexedTriggerDetectionDisabled::<T>::contains_key(member.actor_id) {
      cursor.offset = cursor
        .offset
        .checked_add(1)
        .ok_or(Error::<T>::CrossingIndexInvariant)?;
      CrossingRangeCursors::<T>::insert(feed, cursor);
      Self::advance_crossing_pending_feed(feed)?;
      return Ok(CrossingWorkOutcome::new(true, 1, 1, 1, 1));
    }
    let LoadedActorStateOf::Active(loaded) =
      Self::load_actor_state_for_frame_control(member.actor_id)
    else {
      return Err(Error::<T>::ActorInvariant.into());
    };
    let hot = loaded.hot;
    let contract = loaded.contract;
    let TriggerRuntimeState::ObservationCrossing {
      phase,
      installed_at_revision,
    } = hot.trigger_runtime_state
    else {
      return Err(Error::<T>::ActorInvariant.into());
    };
    if installed_at_revision >= transition.revision {
      cursor.offset = cursor
        .offset
        .checked_add(1)
        .ok_or(Error::<T>::CrossingIndexInvariant)?;
      CrossingRangeCursors::<T>::insert(feed, cursor);
      Self::advance_crossing_pending_feed(feed)?;
      return Ok(CrossingWorkOutcome::new(true, 1, 1, 1, 1));
    }
    let crossing =
      Self::crossing_from_trigger(&contract.trigger).ok_or(Error::<T>::CrossingIndexInvariant)?;
    let transition_kind = crossing.transition(phase, transition.previous, transition.current);
    let (_, role) = Self::crossing_obligation(&crossing, phase);
    ensure!(
      matches!(
        (role, transition_kind),
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
    let fire_plan = if transition_kind == CrossingTransition::Fire {
      Some(Self::classify_fire_activation(member.actor_id, hot, contract, transition)?.0)
    } else {
      None
    };
    Self::move_crossing_membership_with_authority(member.actor_id, crossing, next_phase, locator)?;
    let mut activation = None;
    if let Some(fire_plan) = fire_plan {
      if fire_plan == CrossingWorkPlan::FireCohortClosed {
        activation =
          Some(Self::request_activation(member.actor_id).map_err(Self::activation_failure_error)?);
      } else if fire_plan != CrossingWorkPlan::FireCohortCoalesced
        && let Some(breakdown) = Self::charge_crossing_fire_occurrence(member.actor_id)?
      {
        let activated =
          Self::request_activation(member.actor_id).map_err(Self::activation_failure_error)?;
        ensure!(
          matches!(
            activated,
            ActivationOutcome::Latched | ActivationOutcome::Coalesced
          ),
          Error::<T>::CrossingIndexInvariant
        );
        IndexedTriggerDetectionDisabled::<T>::insert(member.actor_id, ());
        Self::deposit_crossing_fire_occurrence(member.actor_id, breakdown);
        activation = Some(activated);
      }
      if activation == Some(ActivationOutcome::Closed)
        && CrossingFeedMembershipCount::<T>::get(feed) == 0
      {
        // Closing the final member clears this feed's queue, cursor, and pending
        // link. Report possible outer work conservatively; the next bounded
        // unit observes the canonical pending-list state without recreating it.
        return Ok(CrossingWorkOutcome::new(true, 1, 1, 1, 1).with_activation(true));
      }
    }
    Self::persist_crossing_cursor_after_movement(feed, &transition, cursor, key, threshold)?;
    let outcome = CrossingWorkOutcome::new(true, 1, 1, 1, 1);
    Ok(match activation {
      Some(activated) => outcome.with_activation(activated == ActivationOutcome::Closed),
      None if transition_kind == CrossingTransition::Fire => outcome.with_canonical_probe(),
      None => outcome,
    })
  }

  pub(crate) fn crossing_single_candidate_plan(plan: CrossingWorkPlan) -> Option<CrossingWorkPlan> {
    match plan {
      CrossingWorkPlan::SkipPostInstallationPair => {
        Some(CrossingWorkPlan::SkipPostInstallationTransition)
      }
      CrossingWorkPlan::RearmCohortPair => Some(CrossingWorkPlan::RearmCohort),
      CrossingWorkPlan::FireCohortPlacedBatch => Some(CrossingWorkPlan::FireCohortPlaced),
      CrossingWorkPlan::FireCohortCoalescedPair => Some(CrossingWorkPlan::FireCohortCoalesced),
      _ => None,
    }
  }

  pub(crate) fn crossing_plan_components(plan: CrossingWorkPlan) -> (u32, u32, u32, u32) {
    match plan {
      CrossingWorkPlan::Empty | CrossingWorkPlan::StructuralFault => (0, 0, 0, 0),
      CrossingWorkPlan::CompleteTransition | CrossingWorkPlan::SeekMiss => (1, 0, 0, 0),
      CrossingWorkPlan::AdvanceLeaf => (1, 1, 0, 0),
      CrossingWorkPlan::AdvancePage => (1, 1, 1, 0),
      CrossingWorkPlan::SkipPostInstallationPairPending
      | CrossingWorkPlan::SkipPostInstallationPair
      | CrossingWorkPlan::RearmCohortPairPending
      | CrossingWorkPlan::RearmCohortPair
      | CrossingWorkPlan::FireCohortPairPending
      | CrossingWorkPlan::FireCohortPlacedBatch
      | CrossingWorkPlan::FireCohortCoalescedPair => (2, 2, 2, 2),
      CrossingWorkPlan::OpenLeaf
      | CrossingWorkPlan::SkipPostInstallationTransition
      | CrossingWorkPlan::RearmCohort
      | CrossingWorkPlan::FireCohortPending
      | CrossingWorkPlan::FireCohortCoalesced
      | CrossingWorkPlan::FireCohortPlaced
      | CrossingWorkPlan::FireCohortClosed => (1, 1, 1, 1),
    }
  }

  pub(crate) fn crossing_plan_components_for_admission(
    plan: CrossingWorkPlan,
    admitted_candidates: u32,
  ) -> Option<(u32, u32, u32, u32)> {
    if plan == CrossingWorkPlan::FireCohortPlacedBatch {
      if (2..=T::MaxCrossingActorsPerBlock::get()).contains(&admitted_candidates) {
        return Some((
          admitted_candidates,
          admitted_candidates,
          admitted_candidates,
          admitted_candidates,
        ));
      }
      return None;
    }
    let components = Self::crossing_plan_components(plan);
    (components.3 == admitted_candidates).then_some(components)
  }

  pub(crate) fn crossing_plan_weight(plan: CrossingWorkPlan) -> Weight {
    use crate::weights::WeightInfo as _;

    let transition = T::WeightInfo::crossing_transition_unit();
    let ordinary = T::WeightInfo::crossing_leaf_unit().max(T::WeightInfo::crossing_page_unit());
    let rearm = T::WeightInfo::crossing_rearm_unit();
    let rearm_pair = T::WeightInfo::crossing_rearm_pair_unit();
    let coalesced = T::WeightInfo::crossing_coalesced_unit();
    let coalesced_pair = T::WeightInfo::crossing_coalesced_pair_unit();
    let placed = T::WeightInfo::crossing_placed_unit();
    let placed_pair = T::WeightInfo::crossing_placed_pair_unit();
    let skip = T::WeightInfo::crossing_skip_unit();
    let skip_pair = T::WeightInfo::crossing_skip_pair_unit();
    let terminal = T::WeightInfo::crossing_actor_unit();
    match plan {
      CrossingWorkPlan::Empty => Weight::zero(),
      CrossingWorkPlan::CompleteTransition | CrossingWorkPlan::SeekMiss => transition,
      CrossingWorkPlan::OpenLeaf
      | CrossingWorkPlan::AdvanceLeaf
      | CrossingWorkPlan::AdvancePage
      | CrossingWorkPlan::FireCohortPending
      | CrossingWorkPlan::FireCohortPairPending
      | CrossingWorkPlan::RearmCohortPairPending
      | CrossingWorkPlan::SkipPostInstallationPairPending => ordinary,
      CrossingWorkPlan::SkipPostInstallationTransition => skip,
      CrossingWorkPlan::SkipPostInstallationPair => skip_pair,
      CrossingWorkPlan::RearmCohort => rearm,
      CrossingWorkPlan::RearmCohortPair => rearm_pair,
      CrossingWorkPlan::FireCohortPlaced => placed,
      CrossingWorkPlan::FireCohortPlacedBatch => placed_pair,
      CrossingWorkPlan::FireCohortCoalescedPair => coalesced_pair,
      CrossingWorkPlan::FireCohortCoalesced => coalesced,
      CrossingWorkPlan::FireCohortClosed | CrossingWorkPlan::StructuralFault => terminal,
    }
  }

  fn preflight_current_placed_pair_authority()
  -> Result<CrossingPlacedCohortAuthority<T>, DispatchError> {
    let feed = CrossingPendingFeedListState::<T>::get()
      .cursor
      .ok_or(Error::<T>::CrossingTransitionInvariant)?;
    let transition = CrossingTransitionQueues::<T>::get(feed)
      .and_then(|queue| queue.first().copied())
      .ok_or(Error::<T>::CrossingTransitionInvariant)?;
    let cursor =
      CrossingRangeCursors::<T>::get(feed).ok_or(Error::<T>::CrossingTransitionInvariant)?;
    let threshold = cursor
      .current_threshold
      .ok_or(Error::<T>::CrossingIndexInvariant)?;
    let key = CrossingLeafKey {
      feed,
      traversal: cursor.traversal,
      threshold,
    };
    let state = CrossingLeafStates::<T>::get(key).ok_or(Error::<T>::CrossingIndexInvariant)?;
    ensure!(
      cursor.page == state.tail_page,
      Error::<T>::CrossingIndexInvariant
    );
    let page =
      CrossingMemberPages::<T>::get(key, cursor.page).ok_or(Error::<T>::CrossingIndexInvariant)?;
    let first = Self::snapshot_crossing_source_prefix(key, cursor.page, &page, cursor.offset, 1)?;
    let tail_offset = page.entries.len().saturating_sub(1) as u32;
    ensure!(
      tail_offset != cursor.offset,
      Error::<T>::CrossingIndexInvariant
    );
    let tail = Self::snapshot_crossing_source_prefix(key, cursor.page, &page, tail_offset, 1)?;
    let first_preflight = Self::preflight_crossing_cohort(
      &first,
      transition,
      CrossingFireClassification::Resolve,
      Some(CrossingWorkPlan::FireCohortPlaced),
    )?;
    let tail_preflight = Self::preflight_crossing_cohort(
      &tail,
      transition,
      CrossingFireClassification::Resolve,
      Some(CrossingWorkPlan::FireCohortPlaced),
    )?;
    ensure!(
      first_preflight.admitted_candidates == 1
        && tail_preflight.admitted_candidates == 1
        && first_preflight.placed_immediate_fifo == Some(true)
        && tail_preflight.placed_immediate_fifo == Some(true),
      Error::<T>::CrossingIndexInvariant
    );
    let mut queue_candidates = first_preflight.queue_candidates;
    queue_candidates.extend(tail_preflight.queue_candidates);
    Self::build_placed_cohort_authority(
      feed,
      transition,
      cursor,
      alloc::vec![first.candidates[0], tail.candidates[0]],
      queue_candidates,
    )
  }

  fn preflight_current_placed_batch_authority(
    max_candidates: u32,
  ) -> Result<CrossingPlacedCohortAuthority<T>, DispatchError> {
    let feed = CrossingPendingFeedListState::<T>::get()
      .cursor
      .ok_or(Error::<T>::CrossingTransitionInvariant)?;
    let transition = CrossingTransitionQueues::<T>::get(feed)
      .and_then(|queue| queue.first().copied())
      .ok_or(Error::<T>::CrossingTransitionInvariant)?;
    let cursor =
      CrossingRangeCursors::<T>::get(feed).ok_or(Error::<T>::CrossingTransitionInvariant)?;
    if max_candidates > 2 {
      if let Some(threshold) = cursor.current_threshold {
        let key = CrossingLeafKey {
          feed,
          traversal: cursor.traversal,
          threshold,
        };
        if let (Some(state), Some(page)) = (
          CrossingLeafStates::<T>::get(key),
          CrossingMemberPages::<T>::get(key, cursor.page),
        ) {
          let branch_candidates = if cursor.page == state.tail_page {
            max_candidates
          } else {
            max_candidates.min(64)
          };
          let count = Self::crossing_source_prefix_count(&page, cursor.offset, branch_candidates);
          if count >= 3 {
            let snapshot =
              Self::snapshot_crossing_source_prefix(key, cursor.page, &page, cursor.offset, count)?;
            let preflight = Self::preflight_crossing_cohort(
              &snapshot,
              transition,
              CrossingFireClassification::Resolve,
              Some(CrossingWorkPlan::FireCohortPlaced),
            )?;
            if preflight.admitted_candidates == count
              && preflight.placed_immediate_fifo == Some(true)
            {
              let mut authority = Self::build_placed_cohort_authority(
                feed,
                transition,
                cursor,
                snapshot.candidates.into_inner(),
                preflight.queue_candidates,
              )?;
              if cursor.page != state.tail_page {
                if let Ok(tail_refill) =
                  Self::snapshot_crossing_tail_suffix(key, cursor.page, count)
                {
                  authority.tail_refill = Some(tail_refill);
                  return Ok(authority);
                }
              } else {
                return Ok(authority);
              }
            }
          }
        }
      }
    }
    Self::preflight_current_placed_pair_authority()
  }

  fn do_crossing_atomic_placed_batch_unit(
    admitted_candidates: u32,
  ) -> Result<CrossingWorkOutcome, DispatchError> {
    let authority = Self::preflight_current_placed_batch_authority(admitted_candidates)?;
    let feed = authority.feed;
    let transition = authority.transition;
    let cursor = authority.cursor;
    let count = authority.candidates.len() as u32;
    let threshold = cursor
      .current_threshold
      .ok_or(Error::<T>::CrossingIndexInvariant)?;
    let key = authority.candidates[0].locator.key;
    Self::charge_crossing_placed_cohort(&authority)?;
    if authority.tail_refill.is_some() {
      Self::commit_non_tail_placed_cohort_authority(authority)?;
    } else if count == 2 {
      Self::commit_placed_pair_authority(authority)?;
    } else {
      Self::commit_tail_page_placed_cohort_authority(authority)?;
    }
    Self::persist_crossing_cursor_after_movement(feed, &transition, cursor, key, threshold)?;
    Ok(CrossingWorkOutcome {
      has_more: true,
      transitions: count,
      leaves: count,
      pages: count,
      actors: count,
      canonical_probes: count,
      activations: count,
      closes: 0,
    })
  }

  fn do_crossing_placed_batch_or_single_unit(
    admitted_candidates: u32,
  ) -> Result<CrossingWorkOutcome, DispatchError> {
    let batch = polkadot_sdk::frame_support::storage::with_transaction(|| {
      match Self::do_crossing_atomic_placed_batch_unit(admitted_candidates) {
        Ok(outcome) => TransactionOutcome::Commit(Ok(outcome)),
        Err(error) => TransactionOutcome::Rollback(Err(error)),
      }
    });
    match batch {
      Err(error) if error == Error::<T>::InsufficientFee.into() => Self::do_crossing_work_unit(),
      result => result,
    }
  }

  pub(crate) fn crossing_plan_weight_for_admission(
    plan: CrossingWorkPlan,
    admitted_candidates: u32,
  ) -> Option<Weight> {
    use crate::weights::WeightInfo as _;

    if plan == CrossingWorkPlan::FireCohortPlacedBatch {
      if !(2..=T::MaxCrossingActorsPerBlock::get()).contains(&admitted_candidates) {
        return None;
      }
      return Some(if admitted_candidates == 2 {
        Self::crossing_plan_weight(plan)
      } else {
        T::WeightInfo::crossing_placed_maximum_unit()
      });
    }
    let components = Self::crossing_plan_components(plan);
    (components.3 == admitted_candidates).then(|| Self::crossing_plan_weight(plan))
  }

  fn do_crossing_placed_pair_unit() -> Result<CrossingWorkOutcome, DispatchError> {
    let original_feed = CrossingPendingFeedListState::<T>::get()
      .cursor
      .ok_or(Error::<T>::CrossingTransitionInvariant)?;
    let first = Self::do_crossing_work_unit()?;
    ensure!(
      CrossingTransitionQueues::<T>::contains_key(original_feed)
        && CrossingRangeCursors::<T>::contains_key(original_feed),
      Error::<T>::CrossingTransitionInvariant
    );
    CrossingPendingFeedListState::<T>::try_mutate(|list| -> DispatchResult {
      ensure!(list.count > 0, Error::<T>::CrossingTransitionInvariant);
      list.cursor = Some(original_feed);
      Ok(())
    })?;
    let second = Self::do_crossing_work_unit()?;
    Ok(first.combine(second))
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn crossing_placed_batch_work_unit(
    admitted_candidates: u32,
  ) -> Result<bool, DispatchError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      match Self::do_crossing_atomic_placed_batch_unit(admitted_candidates) {
        Ok(outcome) => TransactionOutcome::Commit(Ok(outcome.has_more)),
        Err(error) => TransactionOutcome::Rollback(Err(error)),
      }
    })
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn crossing_pair_work_unit() -> Result<bool, DispatchError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      match Self::do_crossing_placed_pair_unit() {
        Ok(outcome) => TransactionOutcome::Commit(Ok(outcome.has_more)),
        Err(error) => TransactionOutcome::Rollback(Err(error)),
      }
    })
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
    Self::service_crossing_transitions_with_counters(remaining_weight).0
  }

  pub(crate) fn service_crossing_transitions_with_counters(
    remaining_weight: polkadot_sdk::frame_support::weights::Weight,
  ) -> (
    polkadot_sdk::frame_support::weights::Weight,
    CrossingWorkCounters,
  ) {
    Self::service_crossing_transitions_resuming(remaining_weight, CrossingWorkCounters::default())
  }

  pub(crate) fn service_crossing_transitions_resuming(
    remaining_weight: polkadot_sdk::frame_support::weights::Weight,
    mut counters: CrossingWorkCounters,
  ) -> (
    polkadot_sdk::frame_support::weights::Weight,
    CrossingWorkCounters,
  ) {
    use crate::weights::WeightInfo as _;
    use polkadot_sdk::sp_weights::WeightMeter;

    let mut meter = WeightMeter::with_limit(remaining_weight);
    if counters.transitions >= T::MaxCrossingTransitionsPerBlock::get()
      || counters.leaves >= T::MaxCrossingLeavesPerBlock::get()
      || counters.pages >= T::MaxCrossingPagesPerBlock::get()
      || counters.candidates >= T::MaxCrossingActorsPerBlock::get()
    {
      return (meter.consumed(), counters);
    }
    let base = T::WeightInfo::crossing_worker_base();
    if !meter.can_consume(base) {
      return (meter.consumed(), counters);
    }
    meter.consume(base);
    if CrossingWorkerFaultState::<T>::exists() {
      return (meter.consumed(), counters);
    }
    if CrossingPendingFeedListState::<T>::get().count == 0 {
      return (meter.consumed(), counters);
    }
    let probe = T::WeightInfo::crossing_work_probe();
    loop {
      if counters.transitions >= T::MaxCrossingTransitionsPerBlock::get()
        || counters.leaves >= T::MaxCrossingLeavesPerBlock::get()
        || counters.pages >= T::MaxCrossingPagesPerBlock::get()
        || counters.candidates >= T::MaxCrossingActorsPerBlock::get()
      {
        break;
      }
      let search_probe = CrossingPendingFeedListState::<T>::get()
        .cursor
        .and_then(CrossingRangeCursors::<T>::get)
        .is_some_and(|cursor| cursor.current_threshold.is_none())
        .then(T::WeightInfo::crossing_search_probe);
      let required_probe = search_probe.map_or(probe, |search| probe.saturating_add(search));
      if !meter.can_consume(required_probe) {
        break;
      }
      meter.consume(probe);
      if let Some(search) = search_probe {
        meter.consume(search);
      }
      let max_candidates = T::MaxCrossingTransitionsPerBlock::get()
        .saturating_sub(counters.transitions)
        .min(T::MaxCrossingLeavesPerBlock::get().saturating_sub(counters.leaves))
        .min(T::MaxCrossingPagesPerBlock::get().saturating_sub(counters.pages))
        .min(T::MaxCrossingActorsPerBlock::get().saturating_sub(counters.candidates));
      let classified = Self::classify_crossing_work_preflight_with_limit(max_candidates);
      let mut plan = classified.plan;
      let mut admitted_candidates = classified.admitted_candidates;
      let mut tail_refill = classified.tail_refill.is_some();
      if matches!(
        plan,
        CrossingWorkPlan::FireCohortPending
          | CrossingWorkPlan::FireCohortPairPending
          | CrossingWorkPlan::RearmCohortPairPending
          | CrossingWorkPlan::SkipPostInstallationPairPending
      ) {
        let requested_probe = match plan {
          CrossingWorkPlan::FireCohortPairPending => T::WeightInfo::crossing_fire_pair_probe(),
          CrossingWorkPlan::RearmCohortPairPending => T::WeightInfo::crossing_rearm_pair_probe(),
          CrossingWorkPlan::SkipPostInstallationPairPending => {
            T::WeightInfo::crossing_skip_pair_probe()
          }
          _ => T::WeightInfo::crossing_fire_probe(),
        };
        let tail_refill_probe = T::WeightInfo::crossing_tail_refill_probe();
        let mut classification_limit = max_candidates;
        let classification_reserve = if tail_refill {
          requested_probe.saturating_add(tail_refill_probe)
        } else {
          requested_probe
        };
        if !meter.can_consume(classification_reserve) {
          plan = match plan {
            CrossingWorkPlan::FireCohortPairPending => CrossingWorkPlan::FireCohortPending,
            CrossingWorkPlan::RearmCohortPairPending => CrossingWorkPlan::RearmCohort,
            CrossingWorkPlan::SkipPostInstallationPairPending => {
              CrossingWorkPlan::SkipPostInstallationTransition
            }
            _ => break,
          };
          classification_limit = 1;
          admitted_candidates = 1;
        }
        if plan == CrossingWorkPlan::FireCohortPending {
          let fire_probe = T::WeightInfo::crossing_fire_probe();
          if !meter.can_consume(fire_probe) {
            break;
          }
          meter.consume(fire_probe);
          let classified = Self::classify_crossing_work_with_limit(classification_limit);
          plan = classified.plan;
          admitted_candidates = classified.admitted_candidates;
          tail_refill = classified.tail_refill.is_some();
        } else if classification_limit > 1 {
          meter.consume(requested_probe);
          let classified = Self::classify_crossing_work_with_limit(classification_limit);
          plan = classified.plan;
          admitted_candidates = classified.admitted_candidates;
          tail_refill = classified.tail_refill.is_some();
        }
      }
      if tail_refill {
        meter.consume(T::WeightInfo::crossing_tail_refill_probe());
      }
      let admitted_branch_weight = if tail_refill {
        let emptied = T::WeightInfo::crossing_placed_non_tail_emptied_unit();
        let trimmed = T::WeightInfo::crossing_placed_non_tail_trimmed_unit();
        Weight::from_parts(
          emptied.ref_time().max(trimmed.ref_time()),
          emptied.proof_size().max(trimmed.proof_size()),
        )
      } else {
        let Some(weight) = Self::crossing_plan_weight_for_admission(plan, admitted_candidates)
        else {
          break;
        };
        weight
      };
      let fault_weight = T::WeightInfo::record_crossing_worker_fault();
      let branch_weight = if meter.can_consume(admitted_branch_weight.saturating_add(fault_weight))
      {
        admitted_branch_weight
      } else if let Some(single_plan) = Self::crossing_single_candidate_plan(plan) {
        plan = single_plan;
        admitted_candidates = 1;
        let Some(single_weight) =
          Self::crossing_plan_weight_for_admission(plan, admitted_candidates)
        else {
          break;
        };
        if !meter.can_consume(single_weight.saturating_add(fault_weight)) {
          break;
        }
        single_weight
      } else {
        break;
      };
      let Some((transitions, leaves, pages, candidates)) =
        Self::crossing_plan_components_for_admission(plan, admitted_candidates)
      else {
        break;
      };
      if counters.transitions.saturating_add(transitions) > T::MaxCrossingTransitionsPerBlock::get()
        || counters.leaves.saturating_add(leaves) > T::MaxCrossingLeavesPerBlock::get()
        || counters.pages.saturating_add(pages) > T::MaxCrossingPagesPerBlock::get()
        || counters.candidates.saturating_add(candidates) > T::MaxCrossingActorsPerBlock::get()
      {
        break;
      }
      if plan == CrossingWorkPlan::Empty
        || !meter.can_consume(branch_weight.saturating_add(fault_weight))
      {
        break;
      }
      let list = CrossingPendingFeedListState::<T>::get();
      let fault_feed = list.cursor.or(list.head);
      let fault_cursor = fault_feed.and_then(CrossingRangeCursors::<T>::get);
      let fault_revision = fault_cursor.map(|cursor| cursor.revision).or_else(|| {
        fault_feed
          .and_then(CrossingTransitionQueues::<T>::get)
          .and_then(|queue| queue.first().map(|transition| transition.revision))
      });
      #[cfg(test)]
      Self::test_record_first_crossing_branch_weight(branch_weight);
      let result = polkadot_sdk::frame_support::storage::with_transaction(|| {
        if plan == CrossingWorkPlan::FireCohortPlacedBatch {
          match Self::do_crossing_placed_batch_or_single_unit(admitted_candidates) {
            Ok(outcome) => TransactionOutcome::Commit(Ok(outcome)),
            Err(error) => TransactionOutcome::Rollback(Err(error)),
          }
        } else if matches!(
          plan,
          CrossingWorkPlan::FireCohortCoalescedPair
            | CrossingWorkPlan::RearmCohortPair
            | CrossingWorkPlan::SkipPostInstallationPair
        ) {
          match Self::do_crossing_placed_pair_unit() {
            Ok(outcome) => TransactionOutcome::Commit(Ok(outcome)),
            Err(error) => TransactionOutcome::Rollback(Err(error)),
          }
        } else {
          match Self::do_crossing_work_unit() {
            Ok(outcome) => TransactionOutcome::Commit(Ok(outcome)),
            Err(error) => TransactionOutcome::Rollback(Err(error)),
          }
        }
      });
      meter.consume(branch_weight);
      let Ok(outcome) = result else {
        counters.faults = counters.faults.saturating_add(1);
        if let (Some(feed), Err(error)) = (fault_feed, result) {
          let class = if error == Error::<T>::CrossingIndexInvariant.into()
            || error == Error::<T>::CrossingTransitionInvariant.into()
            || error == Error::<T>::ActorInvariant.into()
          {
            CrossingWorkerFaultClass::Invariant
          } else if error == Error::<T>::CrossingTransitionCapacityExceeded.into() {
            CrossingWorkerFaultClass::Capacity
          } else if error == Error::<T>::SchedulerIndexExhausted.into() {
            CrossingWorkerFaultClass::SchedulerExhausted
          } else {
            CrossingWorkerFaultClass::Other
          };
          let recorded = Self::record_crossing_worker_fault(
            &mut meter,
            CrossingWorkerFault {
              feed,
              revision: fault_revision,
              threshold: fault_cursor.and_then(|cursor| cursor.current_threshold),
              class,
            },
          );
          debug_assert!(
            recorded,
            "fault Weight was reserved before Crossing mutation"
          );
        }
        break;
      };
      counters.transitions = counters.transitions.saturating_add(outcome.transitions);
      counters.leaves = counters.leaves.saturating_add(outcome.leaves);
      counters.pages = counters.pages.saturating_add(outcome.pages);
      counters.candidates = counters.candidates.saturating_add(outcome.actors);
      counters.canonical_probes = counters
        .canonical_probes
        .saturating_add(outcome.canonical_probes);
      counters.activations = counters.activations.saturating_add(outcome.activations);
      counters.closes = counters.closes.saturating_add(outcome.closes);
      if !outcome.has_more {
        break;
      }
    }
    (meter.consumed(), counters)
  }

  pub(crate) fn prepare_crossing_rearm_hot(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
  ) -> Result<Option<ActorHotStateOf<T>>, DispatchError> {
    if !IndexedTriggerDetectionDisabled::<T>::contains_key(actor_id) {
      return Ok(None);
    }
    let crossing =
      Self::crossing_from_trigger(&instance.trigger).ok_or(Error::<T>::ActorInvariant)?;
    let locator =
      CrossingMemberships::<T>::get(actor_id).ok_or(Error::<T>::CrossingIndexInvariant)?;
    let TriggerRuntimeState::ObservationCrossing { .. } = &instance.trigger_runtime_state else {
      return Err(Error::<T>::ActorInvariant.into());
    };
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
    let actor_type = instance.actor_class.actor_type();
    Self::remove_crossing_member_preserving_feed_queue(actor_id, actor_type)?;
    Self::insert_crossing_member_with_admission_identity(
      actor_id,
      crossing,
      phase,
      locator.generation,
      actor_type,
      admission.admission_identity,
    )?;
    IndexedTriggerDetectionDisabled::<T>::remove(actor_id);
    Ok(Some(ActorHotState {
      lifecycle: instance.lifecycle,
      cycle_state: instance.cycle_state,
      trigger_runtime_state: TriggerRuntimeState::ObservationCrossing {
        phase,
        installed_at_revision,
      },
      unsuccessful_attempt_streak: instance.unsuccessful_attempt_streak,
      pending_signal: false,
      queue_ticket: None,
      wakeup_pointer: instance.wakeup_pointer,
      trigger_wakeup_pointer: instance.trigger_wakeup_pointer,
      terminal_at: instance
        .window
        .map(|window| Self::window_terminal_at(&window)),
      schedule_anchor: instance.schedule_anchor,
      last_cycle_block: instance.last_cycle_block,
    }))
  }

  pub(crate) fn preflight_crossing_membership_with_authority(
    actor_id: ActorId,
    trigger: &TriggerOf<T>,
  ) -> Result<CrossingMembershipTransition<T::ObservationFeedId>, DispatchError> {
    let previous_generation =
      CrossingMemberships::<T>::get(actor_id).map_or(0, |locator| locator.generation);
    let Some(crossing) = trigger.observation_crossing_contract() else {
      return Ok(CrossingMembershipTransition::Remove);
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
    let preserved_hot = {
      match Self::load_actor_state_with_authority(actor_id) {
        LoadedActorStateOf::Active(state) if state.contract.trigger == *trigger => Some(state.hot),
        _ => None,
      }
    };
    if let Some(hot) = preserved_hot {
      let TriggerRuntimeState::ObservationCrossing {
        phase,
        installed_at_revision,
      } = hot.trigger_runtime_state
      else {
        return Err(Error::<T>::ActorInvariant.into());
      };
      return Ok(CrossingMembershipTransition::Preserve {
        phase,
        installed_at_revision,
      });
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
    Ok(CrossingMembershipTransition::Replace {
      crossing,
      phase,
      generation,
      installed_at_revision,
    })
  }

  pub(crate) fn commit_crossing_membership(
    actor_id: ActorId,
    transition: CrossingMembershipTransition<T::ObservationFeedId>,
    actor_type: ActorType,
    admission_identity: [u8; 32],
  ) -> Result<Option<(CrossingPhase, u64)>, DispatchError> {
    match transition {
      CrossingMembershipTransition::Remove => {
        Self::remove_crossing_membership(actor_id, actor_type)?;
        Ok(None)
      }
      CrossingMembershipTransition::Preserve {
        phase,
        installed_at_revision,
      } => Ok(Some((phase, installed_at_revision))),
      CrossingMembershipTransition::Replace {
        crossing,
        phase,
        generation,
        installed_at_revision,
      } => {
        Self::remove_crossing_membership(actor_id, actor_type)?;
        Self::insert_crossing_member_with_admission_identity(
          actor_id,
          crossing,
          phase,
          generation,
          actor_type,
          admission_identity,
        )?;
        Ok(Some((phase, installed_at_revision)))
      }
    }
  }
}
