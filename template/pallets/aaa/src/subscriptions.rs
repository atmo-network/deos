use crate::pallet::*;
use crate::types::TriggerSource;
use polkadot_sdk::frame_support::{BoundedVec, ensure, traits::Get};
use polkadot_sdk::sp_runtime::{DispatchError, DispatchResult};

impl<T: Config> Pallet<T> {
  fn observation_page_size() -> Result<u32, DispatchError> {
    let size = T::QueuePageSize::get();
    ensure!(size > 0, Error::<T>::ObservationSubscriptionInvariant);
    Ok(size)
  }

  pub(crate) fn derive_observation_feeds(
    schedule: &ScheduleOf<T>,
  ) -> Result<ActorObservationFeedsOf<T>, DispatchError> {
    let mut feeds = alloc::vec::Vec::new();
    if let Some(sources) = schedule.trigger.sources() {
      for source in sources {
        if let TriggerSource::OnObservationChange { feed } = source {
          feeds.push(*feed);
        }
      }
    }
    BoundedVec::try_from(feeds)
      .map_err(|_| Error::<T>::ObservationSubscriptionCapacityExceeded.into())
  }

  fn preflight_observation_subscription_slot(aaa_id: AaaId) -> Result<u32, DispatchError> {
    if let Some(slot) = ObservationSubscriptionSlot::<T>::get(aaa_id) {
      ensure!(
        ObservationSubscriptionSlotOwner::<T>::get(slot) == Some(aaa_id),
        Error::<T>::ObservationSubscriptionInvariant
      );
      return Ok(slot);
    }
    let page_size = Self::observation_page_size()?;
    let free_len = ObservationFreeSlotLen::<T>::get();
    let slot = if free_len > 0 {
      let index = free_len - 1;
      let page_id = index / page_size;
      ObservationFreeSlotPages::<T>::get(page_id)
        .and_then(|page| page.last().copied())
        .ok_or(Error::<T>::ObservationSubscriptionInvariant)?
    } else {
      let slot = NextObservationSubscriptionSlot::<T>::get();
      ensure!(
        slot < T::MaxActiveActors::get(),
        Error::<T>::ObservationSubscriptionCapacityExceeded
      );
      slot
    };
    ensure!(
      !ObservationSubscriptionSlotOwner::<T>::contains_key(slot),
      Error::<T>::ObservationSubscriptionInvariant
    );
    Ok(slot)
  }

  pub(crate) fn preflight_observation_subscription_replace(
    aaa_id: AaaId,
    schedule: &ScheduleOf<T>,
  ) -> DispatchResult {
    let new_feeds = Self::derive_observation_feeds(schedule)?;
    let old_feeds = ActorObservationFeeds::<T>::get(aaa_id).unwrap_or_default();
    if old_feeds == new_feeds {
      return Ok(());
    }
    let removed_feeds = old_feeds
      .iter() // deos-bypass: bounded-iter — MaxTriggerSources bounds dirty-feed cleanup preflight.
      .filter(|feed| !new_feeds.contains(feed))
      .copied()
      .collect::<alloc::vec::Vec<_>>();
    Self::preflight_clear_dirty_observation_feeds(&removed_feeds)?;
    let page_size = Self::observation_page_size()?;
    let existing_slot = ObservationSubscriptionSlot::<T>::get(aaa_id);
    ensure!(
      old_feeds.is_empty() == existing_slot.is_none(),
      Error::<T>::ObservationSubscriptionInvariant
    );
    let candidate_slot = if new_feeds.is_empty() {
      existing_slot
    } else {
      Some(Self::preflight_observation_subscription_slot(aaa_id)?)
    };
    if let Some(slot) = candidate_slot {
      let page_id = slot / page_size;
      let offset = (slot % page_size) as usize;
      for feed in new_feeds
        .iter() // deos-bypass: bounded-iter — MaxTriggerSources bounds actor feed preflight.
        .filter(|feed| !old_feeds.contains(feed))
      {
        let page = ObservationSubscriberPages::<T>::get(feed, page_id).unwrap_or_default();
        ensure!(
          page.entries.get(offset).is_none_or(Option::is_none),
          Error::<T>::ObservationSubscriptionInvariant
        );
        ensure!(
          ObservationSubscriberCount::<T>::get(feed) < T::MaxActiveActors::get(),
          Error::<T>::ObservationSubscriptionCapacityExceeded
        );
      }
    }
    Ok(())
  }

  fn allocate_observation_subscription_slot(aaa_id: AaaId) -> Result<u32, DispatchError> {
    ensure!(
      !ObservationSubscriptionSlot::<T>::contains_key(aaa_id),
      Error::<T>::ObservationSubscriptionInvariant
    );
    let page_size = Self::observation_page_size()?;
    let free_len = ObservationFreeSlotLen::<T>::get();
    let slot = if free_len > 0 {
      let index = free_len - 1;
      let page_id = index / page_size;
      let mut page = ObservationFreeSlotPages::<T>::get(page_id)
        .ok_or(Error::<T>::ObservationSubscriptionInvariant)?;
      let slot = page
        .pop()
        .ok_or(Error::<T>::ObservationSubscriptionInvariant)?;
      if page.is_empty() {
        ObservationFreeSlotPages::<T>::remove(page_id);
      } else {
        ObservationFreeSlotPages::<T>::insert(page_id, page);
      }
      ObservationFreeSlotLen::<T>::put(index);
      slot
    } else {
      let slot = NextObservationSubscriptionSlot::<T>::get();
      ensure!(
        slot < T::MaxActiveActors::get(),
        Error::<T>::ObservationSubscriptionCapacityExceeded
      );
      NextObservationSubscriptionSlot::<T>::put(
        slot
          .checked_add(1)
          .ok_or(Error::<T>::ObservationSubscriptionCapacityExceeded)?,
      );
      slot
    };
    ensure!(
      !ObservationSubscriptionSlotOwner::<T>::contains_key(slot),
      Error::<T>::ObservationSubscriptionInvariant
    );
    ObservationSubscriptionSlot::<T>::insert(aaa_id, slot);
    ObservationSubscriptionSlotOwner::<T>::insert(slot, aaa_id);
    Ok(slot)
  }

  fn release_observation_subscription_slot(aaa_id: AaaId) -> DispatchResult {
    let slot = ObservationSubscriptionSlot::<T>::take(aaa_id)
      .ok_or(Error::<T>::ObservationSubscriptionInvariant)?;
    ensure!(
      ObservationSubscriptionSlotOwner::<T>::take(slot) == Some(aaa_id),
      Error::<T>::ObservationSubscriptionInvariant
    );
    let page_size = Self::observation_page_size()?;
    let free_len = ObservationFreeSlotLen::<T>::get();
    ensure!(
      free_len < NextObservationSubscriptionSlot::<T>::get(),
      Error::<T>::ObservationSubscriptionInvariant
    );
    let page_id = free_len / page_size;
    let mut page = ObservationFreeSlotPages::<T>::get(page_id).unwrap_or_default();
    page
      .try_push(slot)
      .map_err(|_| Error::<T>::ObservationSubscriptionInvariant)?;
    ObservationFreeSlotPages::<T>::insert(page_id, page);
    ObservationFreeSlotLen::<T>::put(
      free_len
        .checked_add(1)
        .ok_or(Error::<T>::ObservationSubscriptionInvariant)?,
    );
    Ok(())
  }

  fn link_observation_subscriber_page(
    feed: T::ObservationFeedId,
    page_id: u32,
    page: &mut ObservationSubscriberPageOf<T>,
  ) -> DispatchResult {
    ensure!(
      page.entries.is_empty() && page.previous.is_none() && page.next.is_none(),
      Error::<T>::ObservationSubscriptionInvariant
    );
    if let Some(mut list) = ObservationSubscriberPageLists::<T>::get(feed) {
      let mut tail = ObservationSubscriberPages::<T>::get(feed, list.tail)
        .ok_or(Error::<T>::ObservationSubscriptionInvariant)?;
      ensure!(
        tail.next.is_none(),
        Error::<T>::ObservationSubscriptionInvariant
      );
      tail.next = Some(page_id);
      ObservationSubscriberPages::<T>::insert(feed, list.tail, tail);
      page.previous = Some(list.tail);
      list.tail = page_id;
      list.count = list
        .count
        .checked_add(1)
        .ok_or(Error::<T>::ObservationSubscriptionCapacityExceeded)?;
      ensure!(
        list.count <= T::MaxActiveActors::get(),
        Error::<T>::ObservationSubscriptionCapacityExceeded
      );
      ObservationSubscriberPageLists::<T>::insert(feed, list);
    } else {
      ObservationSubscriberPageLists::<T>::insert(
        feed,
        ObservationSubscriberPageList {
          head: page_id,
          tail: page_id,
          count: 1,
        },
      );
    }
    Ok(())
  }

  fn unlink_observation_subscriber_page(
    feed: T::ObservationFeedId,
    page_id: u32,
    page: &ObservationSubscriberPageOf<T>,
  ) -> DispatchResult {
    ensure!(
      page.entries.is_empty(),
      Error::<T>::ObservationSubscriptionInvariant
    );
    let mut list = ObservationSubscriberPageLists::<T>::get(feed)
      .ok_or(Error::<T>::ObservationSubscriptionInvariant)?;
    ensure!(list.count > 0, Error::<T>::ObservationSubscriptionInvariant);
    if let Some(previous_id) = page.previous {
      ensure!(
        ObservationSubscriberPages::<T>::get(feed, previous_id)
          .is_some_and(|previous| previous.next == Some(page_id)),
        Error::<T>::ObservationSubscriptionInvariant
      );
    }
    if let Some(next_id) = page.next {
      ensure!(
        ObservationSubscriberPages::<T>::get(feed, next_id)
          .is_some_and(|next| next.previous == Some(page_id)),
        Error::<T>::ObservationSubscriptionInvariant
      );
    }
    DirtyObservationFeeds::<T>::mutate(feed, |maybe| {
      if let Some(state) = maybe
        && state.next_subscriber_page == Some(page_id)
      {
        state.next_subscriber_page = page.next;
      }
    });
    if let Some(previous_id) = page.previous {
      let mut previous = ObservationSubscriberPages::<T>::get(feed, previous_id)
        .ok_or(Error::<T>::ObservationSubscriptionInvariant)?;
      ensure!(
        previous.next == Some(page_id),
        Error::<T>::ObservationSubscriptionInvariant
      );
      previous.next = page.next;
      ObservationSubscriberPages::<T>::insert(feed, previous_id, previous);
    } else {
      ensure!(
        list.head == page_id,
        Error::<T>::ObservationSubscriptionInvariant
      );
      list.head = page.next.unwrap_or(page_id);
    }
    if let Some(next_id) = page.next {
      let mut next = ObservationSubscriberPages::<T>::get(feed, next_id)
        .ok_or(Error::<T>::ObservationSubscriptionInvariant)?;
      ensure!(
        next.previous == Some(page_id),
        Error::<T>::ObservationSubscriptionInvariant
      );
      next.previous = page.previous;
      ObservationSubscriberPages::<T>::insert(feed, next_id, next);
    } else {
      ensure!(
        list.tail == page_id,
        Error::<T>::ObservationSubscriptionInvariant
      );
      list.tail = page.previous.unwrap_or(page_id);
    }
    list.count -= 1;
    if list.count == 0 {
      ensure!(
        page.previous.is_none() && page.next.is_none(),
        Error::<T>::ObservationSubscriptionInvariant
      );
      ObservationSubscriberPageLists::<T>::remove(feed);
    } else {
      ObservationSubscriberPageLists::<T>::insert(feed, list);
    }
    Ok(())
  }

  fn add_observation_subscriber(
    aaa_id: AaaId,
    slot: u32,
    feed: T::ObservationFeedId,
  ) -> DispatchResult {
    let page_size = Self::observation_page_size()?;
    let page_id = slot / page_size;
    let offset = (slot % page_size) as usize;
    let mut page = ObservationSubscriberPages::<T>::get(feed, page_id).unwrap_or_default();
    let page_was_empty = page.entries.is_empty();
    if page_was_empty {
      Self::link_observation_subscriber_page(feed, page_id, &mut page)?;
    }
    while page.entries.len() <= offset {
      page
        .entries
        .try_push(None)
        .map_err(|_| Error::<T>::ObservationSubscriptionInvariant)?;
    }
    ensure!(
      page.entries[offset].is_none(),
      Error::<T>::ObservationSubscriptionInvariant
    );
    page.entries[offset] = Some(aaa_id);
    ObservationSubscriberPages::<T>::insert(feed, page_id, page);
    ObservationSubscriberCount::<T>::try_mutate(feed, |count| -> DispatchResult {
      ensure!(
        *count < T::MaxActiveActors::get(),
        Error::<T>::ObservationSubscriptionCapacityExceeded
      );
      *count = count
        .checked_add(1)
        .ok_or(Error::<T>::ObservationSubscriptionCapacityExceeded)?;
      Ok(())
    })?;
    ObservationSubscriptionCount::<T>::try_mutate(|count| -> DispatchResult {
      let maximum = T::MaxActiveActors::get()
        .checked_mul(T::MaxTriggerSources::get())
        .ok_or(Error::<T>::ObservationSubscriptionCapacityExceeded)?;
      ensure!(
        *count < maximum,
        Error::<T>::ObservationSubscriptionCapacityExceeded
      );
      *count = count
        .checked_add(1)
        .ok_or(Error::<T>::ObservationSubscriptionCapacityExceeded)?;
      Ok(())
    })
  }

  fn remove_observation_subscriber(
    aaa_id: AaaId,
    slot: u32,
    feed: T::ObservationFeedId,
  ) -> DispatchResult {
    let page_size = Self::observation_page_size()?;
    let page_id = slot / page_size;
    let offset = (slot % page_size) as usize;
    let mut page = ObservationSubscriberPages::<T>::get(feed, page_id)
      .ok_or(Error::<T>::ObservationSubscriptionInvariant)?;
    ensure!(
      page.entries.get(offset) == Some(&Some(aaa_id)),
      Error::<T>::ObservationSubscriptionInvariant
    );
    page.entries[offset] = None;
    while page.entries.last().is_some_and(Option::is_none) {
      page.entries.pop();
    }
    if page.entries.is_empty() {
      Self::unlink_observation_subscriber_page(feed, page_id, &page)?;
      ObservationSubscriberPages::<T>::remove(feed, page_id);
    } else {
      ObservationSubscriberPages::<T>::insert(feed, page_id, page);
    }
    let removed_last = ObservationSubscriberCount::<T>::try_mutate_exists(
      feed,
      |maybe| -> Result<bool, DispatchError> {
        let count = maybe
          .as_mut()
          .ok_or(Error::<T>::ObservationSubscriptionInvariant)?;
        ensure!(*count > 0, Error::<T>::ObservationSubscriptionInvariant);
        *count -= 1;
        let removed_last = *count == 0;
        if removed_last {
          *maybe = None;
        }
        Ok(removed_last)
      },
    )?;
    ObservationSubscriptionCount::<T>::try_mutate(|count| -> DispatchResult {
      ensure!(*count > 0, Error::<T>::ObservationSubscriptionInvariant);
      *count -= 1;
      Ok(())
    })?;
    if removed_last {
      Self::clear_dirty_observation_feed(feed)?;
    }
    Ok(())
  }

  pub(crate) fn replace_observation_subscriptions(
    aaa_id: AaaId,
    schedule: &ScheduleOf<T>,
  ) -> DispatchResult {
    Self::preflight_observation_subscription_replace(aaa_id, schedule)?;
    let new_feeds = Self::derive_observation_feeds(schedule)?;
    let old_feeds = ActorObservationFeeds::<T>::get(aaa_id).unwrap_or_default();
    if old_feeds == new_feeds {
      return Ok(());
    }
    let slot = match (
      ObservationSubscriptionSlot::<T>::get(aaa_id),
      new_feeds.is_empty(),
    ) {
      (Some(slot), _) => slot,
      (None, false) => Self::allocate_observation_subscription_slot(aaa_id)?,
      (None, true) => return Ok(()),
    };
    for feed in old_feeds
      .iter() // deos-bypass: bounded-iter — MaxTriggerSources bounds exact removals.
      .filter(|feed| !new_feeds.contains(feed))
    {
      Self::remove_observation_subscriber(aaa_id, slot, *feed)?;
    }
    for feed in new_feeds
      .iter() // deos-bypass: bounded-iter — MaxTriggerSources bounds exact additions.
      .filter(|feed| !old_feeds.contains(feed))
    {
      Self::add_observation_subscriber(aaa_id, slot, *feed)?;
    }
    if new_feeds.is_empty() {
      ActorObservationFeeds::<T>::remove(aaa_id);
      Self::release_observation_subscription_slot(aaa_id)?;
    } else {
      ActorObservationFeeds::<T>::insert(aaa_id, new_feeds);
    }
    Ok(())
  }

  pub(crate) fn preflight_remove_observation_subscriptions(aaa_id: AaaId) -> DispatchResult {
    let Some(feeds) = ActorObservationFeeds::<T>::get(aaa_id) else {
      ensure!(
        !ObservationSubscriptionSlot::<T>::contains_key(aaa_id),
        Error::<T>::ObservationSubscriptionInvariant
      );
      return Ok(());
    };
    let page_size = Self::observation_page_size()?;
    let slot = ObservationSubscriptionSlot::<T>::get(aaa_id)
      .ok_or(Error::<T>::ObservationSubscriptionInvariant)?;
    ensure!(
      ObservationSubscriptionSlotOwner::<T>::get(slot) == Some(aaa_id),
      Error::<T>::ObservationSubscriptionInvariant
    );
    let page_id = slot / page_size;
    let offset = (slot % page_size) as usize;
    for feed in &feeds {
      ensure!(
        ObservationSubscriberPages::<T>::get(feed, page_id)
          .and_then(|page| page.entries.get(offset).copied())
          == Some(Some(aaa_id))
          && ObservationSubscriberCount::<T>::get(feed) > 0,
        Error::<T>::ObservationSubscriptionInvariant
      );
    }
    ensure!(
      ObservationSubscriptionCount::<T>::get() >= feeds.len() as u32,
      Error::<T>::ObservationSubscriptionInvariant
    );
    let free_len = ObservationFreeSlotLen::<T>::get();
    ensure!(
      free_len < NextObservationSubscriptionSlot::<T>::get(),
      Error::<T>::ObservationSubscriptionInvariant
    );
    let free_page_id = free_len / page_size;
    let expected_free_page_len = free_len % page_size;
    ensure!(
      ObservationFreeSlotPages::<T>::get(free_page_id).map_or(0, |page| page.len() as u32)
        == expected_free_page_len,
      Error::<T>::ObservationSubscriptionInvariant
    );
    Self::preflight_clear_dirty_observation_feeds(feeds.as_slice())
  }

  pub(crate) fn remove_observation_subscriptions(aaa_id: AaaId) -> DispatchResult {
    Self::preflight_remove_observation_subscriptions(aaa_id)?;
    let Some(feeds) = ActorObservationFeeds::<T>::get(aaa_id) else {
      return Ok(());
    };
    let slot = ObservationSubscriptionSlot::<T>::get(aaa_id)
      .ok_or(Error::<T>::ObservationSubscriptionInvariant)?;
    for feed in feeds {
      Self::remove_observation_subscriber(aaa_id, slot, feed)?;
    }
    ActorObservationFeeds::<T>::remove(aaa_id);
    Self::release_observation_subscription_slot(aaa_id)
  }

  #[cfg(feature = "try-runtime")]
  pub(crate) fn do_try_state_observation_subscriptions()
  -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
    use alloc::collections::{BTreeMap, BTreeSet};
    use codec::Encode;
    use polkadot_sdk::sp_runtime::TryRuntimeError;

    let page_size = Self::observation_page_size()
      .map_err(|_| TryRuntimeError::Other("observation page size is zero"))?;
    let next_slot = NextObservationSubscriptionSlot::<T>::get();
    let free_len = ObservationFreeSlotLen::<T>::get();
    if next_slot > T::MaxActiveActors::get() || free_len > next_slot {
      return Err(TryRuntimeError::Other(
        "observation subscription slot bounds are invalid",
      ));
    }
    let mut expected_by_feed = BTreeMap::<T::ObservationFeedId, u32>::new();
    let mut owned_slots = BTreeSet::<u32>::new();
    let mut total = 0u32;
    let actor_feeds = ActorObservationFeeds::<T>::iter(); // deos-bypass: bounded-iter — try-state-only active-actor subscription audit.
    for (aaa_id, feeds) in actor_feeds {
      if feeds.is_empty() || !Self::active_actor_exists(aaa_id) {
        return Err(TryRuntimeError::Other(
          "observation feed ownership has no active actor",
        ));
      }
      if !feeds
        .windows(2)
        .all(|pair| pair[0].encode() < pair[1].encode())
      {
        return Err(TryRuntimeError::Other(
          "actor observation feeds are not canonical",
        ));
      }
      let slot = ObservationSubscriptionSlot::<T>::get(aaa_id).ok_or(TryRuntimeError::Other(
        "observation feed owner has no subscription slot",
      ))?;
      if !owned_slots.insert(slot)
        || ObservationSubscriptionSlotOwner::<T>::get(slot) != Some(aaa_id)
      {
        return Err(TryRuntimeError::Other(
          "observation subscription slot ownership disagrees",
        ));
      }
      let page_id = slot / page_size;
      let offset = (slot % page_size) as usize;
      for feed in feeds {
        let page = ObservationSubscriberPages::<T>::get(feed, page_id).ok_or(
          TryRuntimeError::Other("observation subscriber page is missing"),
        )?;
        if page.entries.get(offset) != Some(&Some(aaa_id)) {
          return Err(TryRuntimeError::Other(
            "observation subscriber page disagrees with actor ownership",
          ));
        }
        let count = expected_by_feed.entry(feed).or_default();
        *count = count
          .checked_add(1)
          .ok_or(TryRuntimeError::Other("observation feed count overflow"))?;
        total = total.checked_add(1).ok_or(TryRuntimeError::Other(
          "observation subscription count overflow",
        ))?;
      }
    }
    let actor_slots = ObservationSubscriptionSlot::<T>::iter(); // deos-bypass: bounded-iter — try-state-only MaxActiveActors slot audit.
    for (aaa_id, slot) in actor_slots {
      if !ActorObservationFeeds::<T>::contains_key(aaa_id)
        || ObservationSubscriptionSlotOwner::<T>::get(slot) != Some(aaa_id)
      {
        return Err(TryRuntimeError::Other(
          "observation subscription slot has no feed owner",
        ));
      }
    }
    let slot_owners = ObservationSubscriptionSlotOwner::<T>::iter(); // deos-bypass: bounded-iter — try-state-only MaxActiveActors reverse-slot audit.
    for (slot, aaa_id) in slot_owners {
      if ObservationSubscriptionSlot::<T>::get(aaa_id) != Some(slot) {
        return Err(TryRuntimeError::Other(
          "observation subscription slot reverse owner disagrees",
        ));
      }
    }
    let mut free_slots = BTreeSet::<u32>::new();
    let free_page_count = free_len.div_ceil(page_size);
    for page_id in 0..free_page_count {
      let page = ObservationFreeSlotPages::<T>::get(page_id).ok_or(TryRuntimeError::Other(
        "observation free-slot page is missing",
      ))?;
      for slot in page {
        if slot >= next_slot
          || !free_slots.insert(slot)
          || ObservationSubscriptionSlotOwner::<T>::contains_key(slot)
        {
          return Err(TryRuntimeError::Other(
            "observation free-slot ownership is invalid",
          ));
        }
      }
    }
    if free_slots.len() as u32 != free_len
      || owned_slots.len() as u32 + free_len != next_slot
      || ObservationFreeSlotPages::<T>::iter_keys().count() as u32 != free_page_count
    {
      return Err(TryRuntimeError::Other(
        "observation subscription slot accounting disagrees",
      ));
    }
    let mut actual_by_feed = BTreeMap::<T::ObservationFeedId, u32>::new();
    let mut actual_pages_by_feed = BTreeMap::<T::ObservationFeedId, BTreeSet<u32>>::new();
    let subscriber_pages = ObservationSubscriberPages::<T>::iter(); // deos-bypass: bounded-iter — try-state-only bounded subscription-page audit.
    for (feed, page_id, page) in subscriber_pages {
      let list = ObservationSubscriberPageLists::<T>::get(feed).ok_or(TryRuntimeError::Other(
        "observation subscriber page has no occupied-page list",
      ))?;
      if page.previous.is_none() != (list.head == page_id)
        || page.next.is_none() != (list.tail == page_id)
        || page.previous.is_some_and(|previous_id| {
          ObservationSubscriberPages::<T>::get(feed, previous_id)
            .is_none_or(|previous| previous.next != Some(page_id))
        })
        || page.next.is_some_and(|next_id| {
          ObservationSubscriberPages::<T>::get(feed, next_id)
            .is_none_or(|next| next.previous != Some(page_id))
        })
        || !actual_pages_by_feed
          .entry(feed)
          .or_default()
          .insert(page_id)
      {
        return Err(TryRuntimeError::Other(
          "observation occupied-page links disagree",
        ));
      }
      if page.entries.is_empty() || page.entries.last().is_some_and(Option::is_none) {
        return Err(TryRuntimeError::Other(
          "observation subscriber page is empty or noncanonical",
        ));
      }
      for (offset, maybe_aaa_id) in page.entries.into_iter().enumerate() {
        let Some(aaa_id) = maybe_aaa_id else { continue };
        let slot = ObservationSubscriptionSlot::<T>::get(aaa_id)
          .ok_or(TryRuntimeError::Other("observation page actor has no slot"))?;
        if slot / page_size != page_id
          || slot % page_size != offset as u32
          || !ActorObservationFeeds::<T>::get(aaa_id).is_some_and(|feeds| feeds.contains(&feed))
        {
          return Err(TryRuntimeError::Other(
            "observation subscriber page entry disagrees",
          ));
        }
        let count = actual_by_feed.entry(feed).or_default();
        *count = count
          .checked_add(1)
          .ok_or(TryRuntimeError::Other("observation page count overflow"))?;
      }
    }
    let page_lists = ObservationSubscriberPageLists::<T>::iter(); // deos-bypass: bounded-iter — try-state-only bounded occupied-page audit.
    let mut page_list_count = 0usize;
    for (feed, list) in page_lists {
      let Some(pages) = actual_pages_by_feed.get(&feed) else {
        return Err(TryRuntimeError::Other(
          "observation occupied-page list has no pages",
        ));
      };
      if list.count == 0
        || list.count as usize != pages.len()
        || !pages.contains(&list.head)
        || !pages.contains(&list.tail)
      {
        return Err(TryRuntimeError::Other(
          "observation occupied-page list bounds disagree",
        ));
      }
      page_list_count += 1;
    }
    if page_list_count != actual_pages_by_feed.len() || actual_by_feed != expected_by_feed {
      return Err(TryRuntimeError::Other(
        "observation subscriber pages disagree with actor feeds",
      ));
    }
    for (feed, expected) in expected_by_feed {
      if ObservationSubscriberCount::<T>::get(feed) != expected {
        return Err(TryRuntimeError::Other(
          "observation subscriber count disagrees",
        ));
      }
    }
    if ObservationSubscriberCount::<T>::iter().count() != actual_by_feed.len()
      || ObservationSubscriptionCount::<T>::get() != total
    {
      return Err(TryRuntimeError::Other(
        "observation subscription aggregate count disagrees",
      ));
    }
    Ok(())
  }
}
