use super::pallet::*;
use super::{AssetOps, FundingAuthority, weights::WeightInfo};
use alloc::vec::Vec;
use frame::prelude::*;
use polkadot_sdk::sp_runtime::{
  Perbill,
  traits::{One, Saturating, Zero},
};
use polkadot_sdk::sp_weights::WeightMeter;

enum AdmissionDecision {
  Admit(Weight),
  Closed(Weight),
  Defer(DeferReason),
  Skip,
}

const MAX_RETRY_BACKOFF_BLOCKS: u32 = 8;

enum LaneStepResult {
  NoWork,
  Progress { executed: bool },
  Blocked,
}

impl<T: Config> Pallet<T> {
  pub(crate) fn execute_cycle(remaining_weight: Weight) -> Weight {
    if remaining_weight.is_zero() {
      return Weight::zero();
    }
    let mut cycle_meter = WeightMeter::with_limit(remaining_weight);
    let now = frame_system::Pallet::<T>::block_number();
    Self::drain_overdue_wakeups_cursor(now, &mut cycle_meter);
    let cutoff = NextQueueTicket::<T>::get();
    let max_executions = T::MaxExecutionsPerBlock::get();
    let max_scanned = T::MaxQueueEntriesScannedPerBlock::get();
    let mut executed = 0u32;
    let mut scanned = 0u32;
    let mut work_requeues: Vec<AaaId> = Vec::new();
    // Typed phase meters consume admitted actor-execution Weight only. Discovery,
    // cleanup, wakeups, probes, admission, and page bookkeeping consume cycle_meter.
    let system_share = Self::effective_system_service_share();
    let mut system_meter =
      WeightMeter::with_limit(Self::service_budget(remaining_weight, system_share));
    let system_limit = Self::service_execution_limit(max_executions, system_share);
    let mut system_executed = 0u32;
    while executed < max_executions && system_executed < system_limit {
      match Self::service_lane_head(
        QueueGroup::System,
        cutoff,
        now,
        &mut cycle_meter,
        Some(&mut system_meter),
        &mut scanned,
        max_scanned,
        &mut work_requeues,
      ) {
        LaneStepResult::Progress {
          executed: did_execute,
        } => {
          let delta = u32::from(did_execute);
          executed = executed.saturating_add(delta);
          system_executed = system_executed.saturating_add(delta);
        }
        LaneStepResult::NoWork | LaneStepResult::Blocked => break,
      }
    }

    let user_share = T::UserExecutionGuarantee::get();
    let mut user_meter =
      WeightMeter::with_limit(Self::service_budget(remaining_weight, user_share));
    let user_limit = Self::service_execution_limit(max_executions, user_share);
    let mut user_executed = 0u32;
    while executed < max_executions && user_executed < user_limit {
      match Self::service_lane_head(
        QueueGroup::User,
        cutoff,
        now,
        &mut cycle_meter,
        Some(&mut user_meter),
        &mut scanned,
        max_scanned,
        &mut work_requeues,
      ) {
        LaneStepResult::Progress {
          executed: did_execute,
        } => {
          let delta = u32::from(did_execute);
          executed = executed.saturating_add(delta);
          user_executed = user_executed.saturating_add(delta);
        }
        LaneStepResult::NoWork | LaneStepResult::Blocked => break,
      }
    }

    while executed < max_executions && scanned < max_scanned {
      let shared_discovery_weight =
        T::WeightInfo::scheduler_paged_tombstone_drain(1).saturating_mul(2);
      if max_scanned.saturating_sub(scanned) < 2
        || !cycle_meter.can_consume(shared_discovery_weight)
      {
        break;
      }
      let system = Self::live_lane_head(
        QueueGroup::System,
        cutoff,
        &mut cycle_meter,
        &mut scanned,
        max_scanned,
      );
      let user = Self::live_lane_head(
        QueueGroup::User,
        cutoff,
        &mut cycle_meter,
        &mut scanned,
        max_scanned,
      );
      let (group, head) = match (system, user) {
        (Some(system_head), Some(user_head)) => {
          if system_head.1.ticket < user_head.1.ticket {
            (QueueGroup::System, system_head)
          } else {
            (QueueGroup::User, user_head)
          }
        }
        (Some(system_head), None) => (QueueGroup::System, system_head),
        (None, Some(user_head)) => (QueueGroup::User, user_head),
        (None, None) => break,
      };
      match Self::service_live_lane_entry(
        group,
        head,
        now,
        &mut cycle_meter,
        None,
        &mut work_requeues,
      ) {
        LaneStepResult::Progress {
          executed: did_execute,
        } => {
          executed = executed.saturating_add(u32::from(did_execute));
        }
        LaneStepResult::NoWork => continue,
        LaneStepResult::Blocked => break,
      }
    }

    for aaa_id in work_requeues {
      Self::enqueue(aaa_id);
    }
    cycle_meter.consumed()
  }

  pub(crate) fn effective_system_service_share() -> Perbill {
    T::SystemExecutionReserve::get().min(Perbill::one() - T::UserExecutionGuarantee::get())
  }

  pub(crate) fn service_execution_limit(max_executions: u32, share: Perbill) -> u32 {
    if share.is_zero() || max_executions == 0 {
      0
    } else {
      share.mul_floor(max_executions).max(1)
    }
  }

  pub(crate) fn service_budget(limit: Weight, share: Perbill) -> Weight {
    Weight::from_parts(
      share.mul_floor(limit.ref_time()),
      share.mul_floor(limit.proof_size()),
    )
  }

  fn live_lane_head(
    group: QueueGroup,
    cutoff: QueueTicket,
    cycle_meter: &mut WeightMeter,
    scanned: &mut u32,
    max_scanned: u32,
  ) -> Option<(QueueTicket, QueueEntry)> {
    let scan_weight = T::WeightInfo::scheduler_paged_tombstone_drain(1);
    while *scanned < max_scanned && cycle_meter.can_consume(scan_weight) {
      let before = Self::queue_group_head(group);
      let stats = Self::paged_drain_group_tombstones(group, cutoff, 1);
      if stats.entries_scanned == 0 {
        return None;
      }
      cycle_meter.consume(scan_weight);
      *scanned = scanned.saturating_add(stats.entries_scanned);
      if Self::queue_group_head(group) != before {
        continue;
      }
      let (position, entry) = Self::paged_group_head_entry(group)?;
      return (entry.ticket < cutoff).then_some((position, entry));
    }
    None
  }

  #[allow(clippy::too_many_arguments)]
  fn service_lane_head(
    group: QueueGroup,
    cutoff: QueueTicket,
    now: BlockNumberFor<T>,
    cycle_meter: &mut WeightMeter,
    phase_meter: Option<&mut WeightMeter>,
    scanned: &mut u32,
    max_scanned: u32,
    work_requeues: &mut Vec<AaaId>,
  ) -> LaneStepResult {
    let Some(head) = Self::live_lane_head(group, cutoff, cycle_meter, scanned, max_scanned) else {
      return LaneStepResult::NoWork;
    };
    Self::service_live_lane_entry(group, head, now, cycle_meter, phase_meter, work_requeues)
  }

  fn service_live_lane_entry(
    group: QueueGroup,
    (position, entry): (QueueTicket, QueueEntry),
    now: BlockNumberFor<T>,
    cycle_meter: &mut WeightMeter,
    mut phase_meter: Option<&mut WeightMeter>,
    work_requeues: &mut Vec<AaaId>,
  ) -> LaneStepResult {
    let consume_weight = T::WeightInfo::scheduler_paged_consume_preserve_page()
      .max(T::WeightInfo::scheduler_paged_consume_delete_page());
    let hot_probe_weight = Self::scheduler_actor_hot_probe_weight_upper();
    let program_probe_weight = Self::scheduler_actor_program_probe_weight_upper();
    if !cycle_meter.can_consume(hot_probe_weight.saturating_add(consume_weight)) {
      return LaneStepResult::Blocked;
    }
    let Some(hot) = ActorHot::<T>::get(entry.aaa_id) else {
      cycle_meter.consume(hot_probe_weight);
      return LaneStepResult::NoWork;
    };
    cycle_meter.consume(hot_probe_weight);
    if hot.queue_ticket != Some(entry.ticket)
      || Self::queue_group(hot.actor_class.aaa_type()) != group
    {
      return LaneStepResult::NoWork;
    }
    if hot.run_state == RunState::Suspended {
      if ContinuationStateStore::<T>::get(entry.aaa_id)
        .is_some_and(|continuation| continuation.last_attempt_block == now)
      {
        return LaneStepResult::Blocked;
      }
    } else if hot.cycle_nonce > 0 && hot.last_cycle_block == now {
      return LaneStepResult::Blocked;
    }
    if hot.lifecycle.is_paused() && hot.terminal_at.is_none_or(|terminal_at| terminal_at > now) {
      if !Self::paged_consume_group_head(group, position) {
        return LaneStepResult::Blocked;
      }
      cycle_meter.consume(consume_weight);
      return LaneStepResult::Progress { executed: false };
    }
    if !cycle_meter.can_consume(program_probe_weight.saturating_add(consume_weight)) {
      return LaneStepResult::Blocked;
    }
    let Some(program) = ActorProgram::<T>::get(entry.aaa_id) else {
      cycle_meter.consume(program_probe_weight);
      if !Self::paged_consume_group_head(group, position) {
        return LaneStepResult::Blocked;
      }
      cycle_meter.consume(consume_weight);
      return LaneStepResult::Progress { executed: false };
    };
    cycle_meter.consume(program_probe_weight);
    let aaa_id = entry.aaa_id;
    let instance = Self::compose_active_actor(hot, program);
    match Self::apply_admission(aaa_id, &instance, cycle_meter) {
      AdmissionDecision::Admit(weight) => {
        if !cycle_meter.can_consume(consume_weight.saturating_add(weight)) {
          Self::deposit_event(Event::CycleDeferred {
            aaa_id,
            reason: DeferReason::InsufficientWeightBudget,
          });
          return LaneStepResult::Blocked;
        }
        if phase_meter
          .as_ref()
          .is_some_and(|meter| !meter.can_consume(weight))
        {
          return LaneStepResult::Blocked;
        }
        if !Self::paged_consume_group_head(group, position) {
          return LaneStepResult::Blocked;
        }
        cycle_meter.consume(consume_weight);
        let _actual = Self::execute_single_cycle(aaa_id, instance, now);
        cycle_meter.consume(weight);
        if let Some(meter) = phase_meter.as_mut() {
          meter.consume(weight);
        }
        if let Some(updated) = Self::active_actor_snapshot(aaa_id) {
          Self::schedule_next_work_local(aaa_id, &updated, now, work_requeues);
        }
        LaneStepResult::Progress { executed: true }
      }
      AdmissionDecision::Closed(weight) => {
        if !cycle_meter.can_consume(consume_weight.saturating_add(weight)) {
          return LaneStepResult::Blocked;
        }
        let _ = Self::paged_consume_group_head(group, position);
        cycle_meter.consume(consume_weight.saturating_add(weight));
        LaneStepResult::Progress { executed: false }
      }
      AdmissionDecision::Defer(reason) => {
        Self::deposit_event(Event::CycleDeferred { aaa_id, reason });
        LaneStepResult::Blocked
      }
      AdmissionDecision::Skip => {
        if !Self::paged_consume_group_head(group, position) {
          return LaneStepResult::Blocked;
        }
        cycle_meter.consume(consume_weight);
        if let Some(updated) = Self::active_actor_snapshot(aaa_id) {
          Self::schedule_next_work_local(aaa_id, &updated, now, work_requeues);
        }
        LaneStepResult::Progress { executed: false }
      }
    }
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

  fn queue_page_and_slot(position: QueueTicket) -> (QueuePageId, usize) {
    let page_size = Self::queue_page_size();
    ((position / page_size), (position % page_size) as usize)
  }

  pub(crate) fn queue_group(aaa_type: AaaType) -> QueueGroup {
    match aaa_type {
      AaaType::System => QueueGroup::System,
      AaaType::User => QueueGroup::User,
    }
  }

  pub(crate) fn queue_group_head(group: QueueGroup) -> QueueTicket {
    match group {
      QueueGroup::System => SystemQueueHead::<T>::get(),
      QueueGroup::User => UserQueueHead::<T>::get(),
    }
  }

  pub(crate) fn queue_group_tail(group: QueueGroup) -> QueueTicket {
    match group {
      QueueGroup::System => SystemQueueTail::<T>::get(),
      QueueGroup::User => UserQueueTail::<T>::get(),
    }
  }

  fn set_queue_group_head(group: QueueGroup, position: QueueTicket) {
    match group {
      QueueGroup::System => SystemQueueHead::<T>::put(position),
      QueueGroup::User => UserQueueHead::<T>::put(position),
    }
  }

  fn set_queue_group_tail(group: QueueGroup, position: QueueTicket) {
    match group {
      QueueGroup::System => SystemQueueTail::<T>::put(position),
      QueueGroup::User => UserQueueTail::<T>::put(position),
    }
  }

  fn queue_group_page(group: QueueGroup, page_id: QueuePageId) -> Option<QueuePageOf<T>> {
    match group {
      QueueGroup::System => SystemQueuePages::<T>::get(page_id),
      QueueGroup::User => UserQueuePages::<T>::get(page_id),
    }
  }

  fn insert_queue_group_page(group: QueueGroup, page_id: QueuePageId, page: QueuePageOf<T>) {
    match group {
      QueueGroup::System => SystemQueuePages::<T>::insert(page_id, page),
      QueueGroup::User => UserQueuePages::<T>::insert(page_id, page),
    }
  }

  fn remove_queue_group_page(group: QueueGroup, page_id: QueuePageId) {
    match group {
      QueueGroup::System => SystemQueuePages::<T>::remove(page_id),
      QueueGroup::User => UserQueuePages::<T>::remove(page_id),
    }
  }

  pub fn combined_queue_occupancy() -> u64 {
    UserQueueTail::<T>::get()
      .saturating_sub(UserQueueHead::<T>::get())
      .saturating_add(SystemQueueTail::<T>::get().saturating_sub(SystemQueueHead::<T>::get()))
  }

  /// Append one actor to its immutable type-derived lane using one global ticket allocator.
  pub fn paged_enqueue(aaa_id: AaaId) -> bool {
    let Some(hot) = ActorHot::<T>::get(aaa_id) else {
      return false;
    };
    if hot.queue_ticket.is_some() {
      return true;
    }
    let group = Self::queue_group(hot.actor_class.aaa_type());
    let head = Self::queue_group_head(group);
    let tail = Self::queue_group_tail(group);
    if tail < head || Self::combined_queue_occupancy() >= u64::from(T::MaxQueueLength::get()) {
      return false;
    }
    let ticket = NextQueueTicket::<T>::get();
    let Some(next_ticket) = ticket.checked_add(1) else {
      return false;
    };
    let Some(next_tail) = tail.checked_add(1) else {
      return false;
    };
    let (page_id, slot) = Self::queue_page_and_slot(tail);
    let mut page = Self::queue_group_page(group, page_id).unwrap_or_default();
    if page.len() != slot || page.try_push(QueueEntry { ticket, aaa_id }).is_err() {
      return false;
    }
    Self::insert_queue_group_page(group, page_id, page);
    Self::set_queue_group_tail(group, next_tail);
    NextQueueTicket::<T>::put(next_ticket);
    ActorHot::<T>::mutate(aaa_id, |maybe| {
      if let Some(hot) = maybe.as_mut() {
        hot.queue_ticket = Some(ticket);
      }
    });
    true
  }

  pub fn paged_invalidate(aaa_id: AaaId) -> Option<QueueTicket> {
    ActorHot::<T>::mutate(aaa_id, |maybe| {
      maybe.as_mut().and_then(|hot| hot.queue_ticket.take())
    })
  }

  pub fn paged_group_head_entry(group: QueueGroup) -> Option<(QueueTicket, QueueEntry)> {
    let head = Self::queue_group_head(group);
    if head >= Self::queue_group_tail(group) {
      return None;
    }
    let (page_id, slot) = Self::queue_page_and_slot(head);
    Self::queue_group_page(group, page_id)
      .and_then(|page| page.get(slot).copied())
      .map(|entry| (head, entry))
  }

  pub fn queue_head() -> QueueTicket {
    if UserQueueHead::<T>::get() < UserQueueTail::<T>::get() {
      UserQueueHead::<T>::get()
    } else {
      SystemQueueHead::<T>::get()
    }
  }

  pub fn queue_tail() -> QueueTicket {
    if UserQueueHead::<T>::get() < UserQueueTail::<T>::get() {
      UserQueueTail::<T>::get()
    } else {
      SystemQueueTail::<T>::get()
    }
  }

  pub fn queue_pages(page_id: QueuePageId) -> Option<QueuePageOf<T>> {
    UserQueuePages::<T>::get(page_id).or_else(|| SystemQueuePages::<T>::get(page_id))
  }

  pub fn paged_head_entry() -> Option<(QueueTicket, QueueEntry)> {
    let system = Self::paged_group_head_entry(QueueGroup::System);
    let user = Self::paged_group_head_entry(QueueGroup::User);
    match (system, user) {
      (Some((_, system)), Some((_, user))) => Some(if system.ticket < user.ticket {
        (system.ticket, system)
      } else {
        (user.ticket, user)
      }),
      (Some((_, entry)), None) | (None, Some((_, entry))) => Some((entry.ticket, entry)),
      (None, None) => None,
    }
  }

  pub fn paged_consume_group_head(group: QueueGroup, position: QueueTicket) -> bool {
    let head = Self::queue_group_head(group);
    let tail = Self::queue_group_tail(group);
    if position != head || head >= tail {
      return false;
    }
    let Some((_, entry)) = Self::paged_group_head_entry(group) else {
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
      Self::remove_queue_group_page(group, page_id);
      Self::set_queue_group_head(group, aligned);
      Self::set_queue_group_tail(group, aligned);
    } else {
      Self::set_queue_group_head(group, next_head);
      if next_head % page_size == 0 {
        Self::remove_queue_group_page(group, page_id);
      }
    }
    ActorHot::<T>::mutate(entry.aaa_id, |maybe| {
      if let Some(hot) = maybe.as_mut()
        && hot.queue_ticket == Some(entry.ticket)
      {
        hot.queue_ticket = None;
      }
    });
    true
  }

  pub fn paged_consume_head(ticket: QueueTicket) -> bool {
    for group in [QueueGroup::System, QueueGroup::User] {
      if let Some((position, entry)) = Self::paged_group_head_entry(group)
        && entry.ticket == ticket
      {
        return Self::paged_consume_group_head(group, position);
      }
    }
    false
  }

  pub fn paged_drain_group_tombstones(
    group: QueueGroup,
    cutoff: QueueTicket,
    scan_limit: u32,
  ) -> QueueDrainStats {
    let mut stats = QueueDrainStats::default();
    if scan_limit == 0 {
      return stats;
    }
    let original_head = Self::queue_group_head(group);
    let tail = Self::queue_group_tail(group);
    let page_size = Self::queue_page_size();
    let mut head = original_head;
    let mut last_deleted_page = None;

    'pages: while head < tail && stats.entries_scanned < scan_limit {
      let (page_id, mut slot) = Self::queue_page_and_slot(head);
      let Some(page) = Self::queue_group_page(group, page_id) else {
        break;
      };
      stats.pages_touched = stats.pages_touched.saturating_add(1);
      while head < tail && stats.entries_scanned < scan_limit && slot < page.len() {
        let entry = page[slot];
        if entry.ticket >= cutoff {
          break 'pages;
        }
        stats.entries_scanned = stats.entries_scanned.saturating_add(1);
        let is_live = ActorHot::<T>::get(entry.aaa_id).is_some_and(|hot| {
          hot.queue_ticket == Some(entry.ticket)
            && Self::queue_group(hot.actor_class.aaa_type()) == group
        });
        if is_live {
          break 'pages;
        }
        stats.tombstones_skipped = stats.tombstones_skipped.saturating_add(1);
        head = head.saturating_add(1);
        slot = slot.saturating_add(1);
      }
      if slot == page.len() {
        Self::remove_queue_group_page(group, page_id);
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
          Self::remove_queue_group_page(group, page_id);
          stats.pages_deleted = stats.pages_deleted.saturating_add(1);
        }
      }
      Self::set_queue_group_head(group, aligned);
      Self::set_queue_group_tail(group, aligned);
    } else {
      Self::set_queue_group_head(group, head);
    }
    stats
  }

  pub fn paged_drain_tombstones(cutoff: QueueTicket, scan_limit: u32) -> QueueDrainStats {
    let group = match (
      Self::paged_group_head_entry(QueueGroup::System),
      Self::paged_group_head_entry(QueueGroup::User),
    ) {
      (Some((_, system)), Some((_, user))) if system.ticket < user.ticket => QueueGroup::System,
      (Some(_), Some(_)) | (None, Some(_)) => QueueGroup::User,
      (Some(_), None) => QueueGroup::System,
      (None, None) => return QueueDrainStats::default(),
    };
    Self::paged_drain_group_tombstones(group, cutoff, scan_limit)
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
    if instance.lifecycle.is_paused() {
      Self::schedule_window_expiry(aaa_id, &instance);
      return;
    }
    Self::schedule_next_work(aaa_id, &instance, now);
  }

  fn window_expiry_wakeup(instance: &AaaInstanceOf<T>) -> Option<BlockNumberFor<T>> {
    instance
      .schedule_window
      .map(|window| window.end.saturating_add(One::one()))
  }

  fn schedule_window_expiry(aaa_id: AaaId, instance: &AaaInstanceOf<T>) {
    if let Some(expiry) = Self::window_expiry_wakeup(instance) {
      let _ = Self::wakeup_substrate_schedule(aaa_id, expiry);
    }
  }

  fn defer_wakeup(aaa_id: AaaId, wakeup_block: BlockNumberFor<T>) -> bool {
    let target = Self::active_actor_snapshot(aaa_id)
      .and_then(|instance| Self::window_expiry_wakeup(&instance))
      .map(|expiry| wakeup_block.min(expiry))
      .unwrap_or(wakeup_block);
    Self::wakeup_substrate_schedule(aaa_id, target)
  }

  /// Baseline scheduler envelope reserved ahead of one actor run plus pure cleanup.
  /// Explicit permissionless repair sweeps remain dispatch-owned and do not consume every block's
  /// guaranteed scheduler envelope.
  pub fn scheduler_admission_overhead() -> Weight {
    T::WeightInfo::scheduler_on_idle_base()
      .saturating_add(T::WeightInfo::scheduler_paged_tombstone_drain(2))
      .saturating_add(
        T::WeightInfo::scheduler_paged_consume_preserve_page()
          .max(T::WeightInfo::scheduler_paged_consume_delete_page()),
      )
      .saturating_add(
        T::WeightInfo::scheduler_paged_append_existing_page()
          .max(T::WeightInfo::scheduler_paged_append_new_page()),
      )
      .saturating_add(T::WeightInfo::scheduler_wakeup_cursor_worker_future())
      .saturating_add(Self::scheduler_actor_hot_probe_weight_upper().saturating_mul(2))
      .saturating_add(Self::scheduler_actor_probe_weight_upper())
  }

  /// Conservatively prices pure actor-local terminal deletion from the measured User close path.
  /// Shared queue and wakeup records become lazy tombstones.
  pub(crate) fn close_cleanup_weight_upper() -> Weight {
    T::WeightInfo::close_aaa()
  }

  pub fn wakeup_registration_weight_upper() -> Weight {
    T::WeightInfo::scheduler_wakeup_append_new_page()
      .saturating_add(T::WeightInfo::scheduler_wakeup_cursor_insert())
      .saturating_add(T::WeightInfo::scheduler_wakeup_cursor_remove_exact())
  }

  pub fn scheduler_actor_hot_probe_weight_upper() -> Weight {
    T::WeightInfo::scheduler_actor_hot_probe()
  }

  pub fn scheduler_actor_program_probe_weight_upper() -> Weight {
    T::WeightInfo::scheduler_actor_program_probe()
  }

  pub fn scheduler_actor_probe_weight_upper() -> Weight {
    Self::scheduler_actor_hot_probe_weight_upper()
      .saturating_add(Self::scheduler_actor_program_probe_weight_upper())
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_scheduler_actor_hot_probe(aaa_id: AaaId) {
    let hot = ActorHot::<T>::get(aaa_id).expect("benchmark actor hot state must exist");
    assert!(hot.lifecycle.is_paused());
    core::hint::black_box(hot);
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_scheduler_actor_program_probe(aaa_id: AaaId, hot: ActorHotStateOf<T>) {
    let program = ActorProgram::<T>::get(aaa_id).expect("benchmark actor program state must exist");
    let instance = Self::compose_active_actor(hot, program);
    let meter = WeightMeter::with_limit(Weight::zero());
    let AdmissionDecision::Defer(reason) = Self::apply_admission(aaa_id, &instance, &meter) else {
      panic!("benchmark actor must defer on an exhausted cycle budget");
    };
    Self::deposit_event(Event::CycleDeferred { aaa_id, reason });
  }

  pub fn wakeup_cursor_drain_unit_weight_upper(removes_bucket: bool) -> Weight {
    if removes_bucket {
      T::WeightInfo::scheduler_wakeup_cursor_worker_remove()
    } else {
      T::WeightInfo::scheduler_wakeup_cursor_worker_partial()
    }
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
      if Self::combined_queue_occupancy() >= u64::from(T::MaxActiveActors::get())
        || !meter.can_consume(base_weight)
      {
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
    if let TriggerPolicy::Cadenced { every_blocks, .. } = schedule.trigger
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
        let first = instance.first_eligible_at;
        if now <= first {
          return first;
        }
        if let TriggerPolicy::Cadenced { every_blocks, .. } = instance.schedule.trigger {
          let cadence_span = every_blocks.saturating_add(
            Self::timer_jitter_blocks(aaa_id, every_blocks).saturated_into::<u32>(),
          );
          let elapsed: u64 = now.saturating_sub(first).saturated_into();
          let span = u64::from(cadence_span.max(1));
          let periods = elapsed.saturating_add(span.saturating_sub(1)) / span;
          return first.saturating_add(periods.saturating_mul(span).saturated_into());
        }
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
      if let TriggerPolicy::Cadenced { every_blocks, .. } = instance.schedule.trigger {
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

  fn retry_backoff_blocks(attempt: u32) -> u32 {
    match attempt {
      0 => 1,
      1 => 2,
      2 => 4,
      _ => MAX_RETRY_BACKOFF_BLOCKS,
    }
  }

  fn retry_eligible_at(aaa_id: AaaId, instance: &AaaInstanceOf<T>) -> BlockNumberFor<T> {
    let continuation = ContinuationStateStore::<T>::get(aaa_id)
      .expect("Suspended run_state requires ContinuationState");
    let cooldown: BlockNumberFor<T> = instance.schedule.cooldown_blocks.into();
    let backoff: BlockNumberFor<T> = Self::retry_backoff_blocks(continuation.attempt).into();
    let retry_delay = cooldown.max(backoff);
    let mut eligible_at = continuation.last_attempt_block.saturating_add(retry_delay);
    if let Some(window) = instance.schedule_window {
      eligible_at = eligible_at.max(window.start);
    }
    eligible_at
  }

  fn schedule_next_work_local(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    now: BlockNumberFor<T>,
    requeues: &mut Vec<AaaId>,
  ) {
    if instance.lifecycle.is_paused() {
      Self::schedule_window_expiry(aaa_id, instance);
      return;
    }
    let eligible_at = if instance.run_state == RunState::Suspended {
      Self::retry_eligible_at(aaa_id, instance)
    } else if instance.pending_signal {
      Self::next_eligible_at(
        aaa_id,
        instance,
        now,
        instance.schedule.trigger.cadence_blocks().is_some(),
      )
    } else if matches!(instance.schedule.trigger, TriggerPolicy::Cadenced { .. }) {
      Self::next_eligible_at(aaa_id, instance, now.saturating_add(One::one()), true)
    } else {
      Self::schedule_window_expiry(aaa_id, instance);
      return;
    };
    let wakeup_at = instance.schedule_window.map_or(eligible_at, |window| {
      eligible_at.min(window.end.saturating_add(One::one()))
    });
    if wakeup_at <= now.saturating_add(One::one()) {
      requeues.push(aaa_id);
    } else {
      Self::defer_wakeup(aaa_id, wakeup_at);
    }
  }

  fn schedule_next_work(aaa_id: AaaId, instance: &AaaInstanceOf<T>, now: BlockNumberFor<T>) {
    let mut requeues = Vec::new();
    Self::schedule_next_work_local(aaa_id, instance, now, &mut requeues);
    for aaa_id in requeues {
      Self::enqueue(aaa_id);
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
    Self::close_actor(aaa_id, instance, reason)
      .expect("fresh scheduler snapshot satisfies terminal preconditions");
    AdmissionDecision::Closed(close_weight_upper)
  }

  fn pending_post_cycle_close_reason(instance: &AaaInstanceOf<T>) -> Option<CloseReason> {
    if Self::failure_limit_reached(instance.consecutive_failures) {
      return Some(CloseReason::ConsecutiveFailures);
    }
    if instance.run_state == RunState::Suspended {
      return None;
    }
    instance
      .auto_close_at_cycle_nonce
      .filter(|target| instance.cycle_nonce >= *target)
      .map(|_| CloseReason::AutoCloseNonceReached)
  }

  fn cycle_may_close_on_failure(
    instance: &AaaInstanceOf<T>,
    start_cursor: usize,
    prior_unsuccessful_attempts_at_cursor: Option<u32>,
  ) -> bool {
    Self::failure_limit_reached(instance.consecutive_failures.saturating_add(1))
      || instance
        .execution_plan
        .iter() // deos-bypass: bounded-iter — plan length is bounded by runtime execution-plan constants
        .enumerate()
        .skip(start_cursor)
        .any(|(index, step)| {
          step.on_error.retry_max_attempts().is_some_and(|limit| {
            let next_attempt = if index == start_cursor {
              prior_unsuccessful_attempts_at_cursor
                .unwrap_or_default()
                .saturating_add(1)
            } else {
              1
            };
            next_attempt >= limit
          })
        })
  }

  fn cycle_may_auto_close_on_success(instance: &AaaInstanceOf<T>) -> bool {
    instance
      .auto_close_at_cycle_nonce
      .map(|target| instance.cycle_nonce.saturating_add(1) >= target)
      .unwrap_or(false)
  }

  fn cycle_requires_terminal_cleanup_budget(
    instance: &AaaInstanceOf<T>,
    start_cursor: usize,
    prior_unsuccessful_attempts_at_cursor: Option<u32>,
  ) -> bool {
    Self::cycle_may_close_on_failure(
      instance,
      start_cursor,
      prior_unsuccessful_attempts_at_cursor,
    ) || Self::cycle_may_auto_close_on_success(instance)
  }

  fn cycle_admission_weight_upper(
    instance: &AaaInstanceOf<T>,
    start_cursor: usize,
    prior_unsuccessful_attempts_at_cursor: Option<u32>,
  ) -> Weight {
    let mut weight = Self::attempt_weight_upper_bound(instance, start_cursor);
    if Self::cycle_requires_terminal_cleanup_budget(
      instance,
      start_cursor,
      prior_unsuccessful_attempts_at_cursor,
    ) {
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
    if instance.run_state == RunState::Idle
      && instance.actor_class.aaa_type() == AaaType::User
      && instance.cycle_nonce == u64::MAX
    {
      return Self::close_within_budget(aaa_id, instance, CloseReason::CycleNonceExhausted, meter);
    }
    if let Some(reason) = Self::pending_post_cycle_close_reason(instance) {
      return Self::close_within_budget(aaa_id, instance, reason, meter);
    }
    if !Self::is_ready_for_execution(aaa_id, instance) {
      return AdmissionDecision::Skip;
    }
    if let Some(reason) = Self::user_resource_close_reason(instance, false) {
      return Self::close_within_budget(aaa_id, instance, reason, meter);
    }
    let continuation = if instance.run_state == RunState::Suspended {
      Some(
        ContinuationStateStore::<T>::get(aaa_id)
          .expect("Suspended run_state requires ContinuationState"),
      )
    } else {
      None
    };
    let start_cursor = continuation
      .as_ref()
      .map_or(0, |state| state.cursor as usize);
    if instance.actor_class.aaa_type() == AaaType::User
      && T::AssetOps::balance(&instance.sovereign_account, T::NativeAssetId::get())
        < Self::attempt_fee_upper_bound(instance, start_cursor)
    {
      return Self::close_within_budget(aaa_id, instance, CloseReason::FeeBudgetExhausted, meter);
    }
    let cycle_weight_upper = Self::cycle_admission_weight_upper(
      instance,
      start_cursor,
      continuation
        .as_ref()
        .map(|state| state.unsuccessful_attempts_at_cursor),
    );
    if !meter.can_consume(cycle_weight_upper) {
      return AdmissionDecision::Defer(DeferReason::InsufficientWeightBudget);
    }
    AdmissionDecision::Admit(cycle_weight_upper)
  }

  pub(crate) fn ensure_simulation_ready(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    mode: SimulationMode,
  ) -> Result<(), SimulationError> {
    if GlobalCircuitBreaker::<T>::get() {
      return Err(SimulationError::GlobalCircuitBreaker);
    }
    if Self::is_window_expired(instance) {
      return Err(SimulationError::WindowExpired);
    }
    if instance.lifecycle.is_paused() {
      return Err(SimulationError::Paused);
    }
    if Self::failure_limit_reached(instance.consecutive_failures) {
      return Err(SimulationError::ConsecutiveFailures);
    }
    match mode {
      SimulationMode::FreshCurrentPlan if instance.run_state != RunState::Idle => {
        return Err(SimulationError::ModeRunStateMismatch);
      }
      SimulationMode::CurrentContinuation if instance.run_state != RunState::Suspended => {
        return Err(SimulationError::ModeRunStateMismatch);
      }
      _ => {}
    }
    if mode == SimulationMode::FreshCurrentPlan && instance.cycle_nonce == u64::MAX {
      return Err(SimulationError::CycleNonceExhausted);
    }
    let start_cursor = if mode == SimulationMode::CurrentContinuation {
      ContinuationStateStore::<T>::get(aaa_id)
        .ok_or(SimulationError::ContinuationInvariant)?
        .cursor as usize
    } else {
      0
    };
    if !Self::is_ready_for_execution(aaa_id, instance) {
      return Err(SimulationError::NotReady);
    }
    if instance.actor_class.aaa_type() == AaaType::User {
      let balance = Self::user_native_balance(instance);
      if balance < T::MinUserBalance::get() {
        return Err(SimulationError::BalanceUnavailable);
      }
      if balance < Self::attempt_fee_upper_bound(instance, start_cursor) {
        return Err(SimulationError::FeeBudgetUnavailable);
      }
    }
    Ok(())
  }

  fn is_ready_for_execution(aaa_id: AaaId, instance: &AaaInstanceOf<T>) -> bool {
    if instance.lifecycle.is_paused() {
      return false;
    }
    if GlobalCircuitBreaker::<T>::get() {
      return false;
    }
    let now = frame_system::Pallet::<T>::block_number();
    if instance.run_state == RunState::Suspended {
      return Self::retry_eligible_at(aaa_id, instance) <= now;
    }
    let include_timer = instance.schedule.trigger.cadence_blocks().is_some();
    if Self::next_eligible_at(aaa_id, instance, now, include_timer) > now {
      return false;
    }
    Self::evaluate_trigger(aaa_id, instance)
  }

  fn evaluate_trigger(aaa_id: AaaId, instance: &AaaInstanceOf<T>) -> bool {
    match instance.schedule.trigger {
      TriggerPolicy::Immediate { .. }
      | TriggerPolicy::Cadenced {
        mode: CadenceMode::WhenSignalled(_),
        ..
      } => instance.pending_signal,
      TriggerPolicy::Cadenced {
        mode: CadenceMode::Always,
        ..
      } => Self::evaluate_timer(aaa_id, instance),
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
    let mut signal_matched = false;
    if apply_trigger && let Some(sources) = instance.schedule.trigger.sources() {
      for source in sources {
        // deos-bypass: bounded-iter — MaxTriggerSources bounds full source observation.
        if let TriggerSource::OnAddressEvent {
          source_filter,
          asset_filter,
        } = source
        {
          signal_matched |= Self::source_matches_filter(
            source_filter,
            &instance.owner,
            provenance.map(FundingProvenance::account),
          ) && Self::asset_matches_filter(asset_filter, asset);
        }
      }
      if signal_matched && !instance.pending_signal {
        ActorHot::<T>::mutate(aaa_id, |maybe_hot| {
          if let Some(hot) = maybe_hot {
            hot.pending_signal = true;
          }
        });
      }
    }
    if apply_funding && amount > Zero::zero() {
      let mut funding = ActorFunding::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      if Self::funding_event_authorized(aaa_id, &instance, &funding, provenance)
        && funding.funding_tracked_assets.contains(&asset)
      {
        let mut new_pending_asset = false;
        if let Some(batch) = funding.funding_snapshots.get_mut(&asset) {
          new_pending_asset = batch.pending_amount.is_zero();
          let pending_amount = batch
            .pending_amount
            .checked_add(&amount)
            .ok_or(Error::<T>::FundingBatchOverflow)?;
          batch.pending_amount = pending_amount;
          Self::deposit_event(Event::FundingBatchPendingAccumulated {
            aaa_id,
            asset,
            added: amount,
            pending_amount,
          });
        } else if instance.run_state == RunState::Suspended {
          funding
            .funding_snapshots
            .try_insert(
              asset,
              FundingBatch {
                amount: Zero::zero(),
                pending_amount: amount,
              },
            )
            .map_err(|_| Error::<T>::FundingBatchOverflow)?;
          new_pending_asset = true;
          Self::deposit_event(Event::FundingBatchPendingAccumulated {
            aaa_id,
            asset,
            added: amount,
            pending_amount: amount,
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
        if new_pending_asset {
          ActorHot::<T>::mutate(aaa_id, |maybe_hot| {
            if let Some(hot) = maybe_hot {
              hot.pending_funding_count = hot
                .pending_funding_count
                .checked_add(1)
                .expect("pending funding count is bounded by tracked assets");
            }
          });
        }
      }
    }
    if signal_matched {
      Self::enqueue(aaa_id);
    }
    Ok(())
  }

  pub(crate) fn evaluate_actor_liveness(aaa_id: AaaId) -> DispatchResult {
    let instance = Self::active_actor_snapshot(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
    if let Some(reason) = Self::liveness_close_reason(&instance) {
      return Self::close_actor(aaa_id, &instance, reason);
    }
    Ok(())
  }
}
