use super::pallet::*;
use super::{AssetOps, FundingAuthority, weights::WeightInfo};
use alloc::vec::Vec;
use frame::prelude::*;
use polkadot_sdk::frame_support::storage::transactional::with_transaction_opaque_err;
use polkadot_sdk::sp_runtime::traits::{One, Zero};
use polkadot_sdk::sp_weights::WeightMeter;

#[derive(Clone, Copy)]
enum QueueMutation {
  Enqueue,
  Head,
}

struct QueueTopology {
  head: QueueTicket,
  tail: QueueTicket,
  occupancy: u32,
}

enum AdmissionDecision {
  Admit(Weight),
  Close { reason: CloseReason, weight: Weight },
  Defer(DeferReason),
  Skip,
}

/// Closed outcome of one canonical FIFO placement attempt. Queue capacity
/// exhaustion may preserve readiness through an exact later wakeup; monotonic
/// ticket/page namespace exhaustion and corruption are not retryable and fail
/// closed through the public error surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnqueueOutcome {
  Ok,
  AlreadyLive,
  CapacityUnavailable,
  TicketExhausted,
  SchedulerIndexExhausted,
  WakeupCapacityExhausted,
  WakeupIndexExhausted,
  CorruptedTopology,
}

const MAX_RETRY_BACKOFF_BLOCKS: u32 = 8;

#[cfg(test)]
std::thread_local! {
  static CORRUPT_QUEUE_BEFORE_CLOSE_CONSUME: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

/// Why the actor pass stopped at a queue boundary. Only a weight block over live FIFO work with
/// no admitted attempt drives `IdleStarvationState`; every other reason clears it once (spec 8.6).
#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockKind {
  Weight,
  NonWeight,
}

/// One bounded actor-service pass over the canonical FIFO: consumed weight plus the starve signal
/// derived from the terminal block reason (spec 8.6.3).
pub(crate) struct CyclePass {
  pub(crate) consumed: Weight,
  pub(crate) starved: bool,
}

enum FifoStepResult {
  NoWork,
  Progress { executed: bool },
  Blocked(BlockKind),
}

enum HeadDiscovery {
  Empty,
  Head(QueueTicket, QueueEntry),
  Blocked(BlockKind),
}

impl<T: Config> Pallet<T> {
  pub(crate) fn execute_cycle(remaining_weight: Weight) -> CyclePass {
    if remaining_weight.is_zero() {
      return CyclePass {
        consumed: Weight::zero(),
        starved: false,
      };
    }
    let mut cycle_meter = WeightMeter::with_limit(remaining_weight);
    let now = frame_system::Pallet::<T>::block_number();
    // The cutoff is captured after the on_idle wakeup and fanout phases (spec 8.2.1);
    // only tickets below the cutoff may execute in this actor-service pass.
    let cutoff = NextQueueTicket::<T>::get();
    let max_executions = T::MaxExecutionsPerBlock::get();
    let max_scanned = T::MaxQueueEntriesScannedPerBlock::get();
    let mut executed = 0u32;
    let mut scanned = 0u32;
    let mut starved = false;
    while executed < max_executions && scanned < max_scanned {
      let head = Self::live_queue_head(cutoff, &mut cycle_meter, &mut scanned, max_scanned);
      match head {
        HeadDiscovery::Empty => break,
        HeadDiscovery::Blocked(kind) => {
          starved = matches!(kind, BlockKind::Weight) && executed == 0;
          break;
        }
        HeadDiscovery::Head(position, entry) => {
          match Self::service_live_queue_entry((position, entry), now, &mut cycle_meter) {
            FifoStepResult::Progress {
              executed: did_execute,
            } => executed = executed.saturating_add(u32::from(did_execute)),
            FifoStepResult::NoWork => continue,
            FifoStepResult::Blocked(kind) => {
              starved = matches!(kind, BlockKind::Weight) && executed == 0;
              break;
            }
          }
        }
      }
    }
    CyclePass {
      consumed: cycle_meter.consumed(),
      starved,
    }
  }

  fn classify_current_queue(cutoff: QueueTicket) -> HeadDiscovery {
    if Self::queue_topology_preflight(QueueMutation::Head).is_err() {
      return HeadDiscovery::Blocked(BlockKind::NonWeight);
    }
    if QueueHead::<T>::get() >= QueueTail::<T>::get() {
      return HeadDiscovery::Empty;
    }
    match Self::paged_head_entry() {
      Some((_, entry)) if entry.ticket >= cutoff => HeadDiscovery::Empty,
      _ => HeadDiscovery::Blocked(BlockKind::NonWeight),
    }
  }

  /// True when the physical FIFO head is a live actor ticket below the pass cutoff. The worker uses
  /// this at the probe boundary to distinguish an empty/post-cutoff or tombstone-only queue (not
  /// starvation) from live work blocked by weight (spec 8.6.3).
  fn head_blocked_by_weight(cutoff: QueueTicket) -> bool {
    if QueueHead::<T>::get() >= QueueTail::<T>::get() {
      return false;
    }
    match Self::paged_head_entry() {
      Some((_, entry)) if entry.ticket < cutoff => {
        ActorHot::<T>::get(entry.aaa_id).is_some_and(|hot| hot.queue_ticket == Some(entry.ticket))
          && ActorIdentities::<T>::contains_key(entry.aaa_id)
      }
      _ => false,
    }
  }

  fn live_queue_head(
    cutoff: QueueTicket,
    cycle_meter: &mut WeightMeter,
    scanned: &mut u32,
    max_scanned: u32,
  ) -> HeadDiscovery {
    let scan_weight = T::WeightInfo::scheduler_paged_tombstone_drain(1);
    while *scanned < max_scanned {
      if Self::queue_topology_preflight(QueueMutation::Head).is_err() {
        return HeadDiscovery::Blocked(BlockKind::NonWeight);
      }
      if !cycle_meter.can_consume(scan_weight) {
        return if Self::head_blocked_by_weight(cutoff) {
          HeadDiscovery::Blocked(BlockKind::Weight)
        } else {
          HeadDiscovery::Empty
        };
      }
      cycle_meter.consume(scan_weight);
      let before = QueueHead::<T>::get();
      let stats = match Self::paged_drain_tombstones(cutoff, 1) {
        Ok(stats) => stats,
        Err(_) => return HeadDiscovery::Blocked(BlockKind::NonWeight),
      };
      if stats.entries_scanned == 0 {
        return Self::classify_current_queue(cutoff);
      }
      *scanned = scanned.saturating_add(stats.entries_scanned);
      if QueueHead::<T>::get() != before {
        continue;
      }
      return match Self::paged_head_entry() {
        Some((position, entry)) if entry.ticket < cutoff => HeadDiscovery::Head(position, entry),
        Some(_) => HeadDiscovery::Empty,
        None => Self::classify_current_queue(cutoff),
      };
    }
    HeadDiscovery::Blocked(BlockKind::NonWeight)
  }

  #[cfg(test)]
  pub(crate) fn test_head_discovery(
    cutoff: QueueTicket,
    scan_limit: u32,
    scanned_start: u32,
    weight: Weight,
  ) -> (u8, Option<QueueEntry>, u32) {
    let mut meter = WeightMeter::with_limit(weight);
    let mut scanned = scanned_start;
    let discovery = Self::live_queue_head(cutoff, &mut meter, &mut scanned, scan_limit);
    match discovery {
      HeadDiscovery::Empty => (0, None, scanned),
      HeadDiscovery::Head(_, entry) => (1, Some(entry), scanned),
      HeadDiscovery::Blocked(_) => (2, None, scanned),
    }
  }

  fn service_live_queue_entry(
    (position, entry): (QueueTicket, QueueEntry),
    now: BlockNumberFor<T>,
    cycle_meter: &mut WeightMeter,
  ) -> FifoStepResult {
    let consume_weight = T::WeightInfo::scheduler_paged_consume_preserve_page()
      .max(T::WeightInfo::scheduler_paged_consume_delete_page());
    let hot_probe_weight = Self::scheduler_actor_hot_probe_weight_upper();
    let program_probe_weight = Self::scheduler_actor_program_probe_weight_upper();
    if !cycle_meter.can_consume(hot_probe_weight.saturating_add(consume_weight)) {
      return FifoStepResult::Blocked(BlockKind::Weight);
    }
    let Some(hot) = ActorHot::<T>::get(entry.aaa_id) else {
      cycle_meter.consume(hot_probe_weight);
      return FifoStepResult::NoWork;
    };
    let Some(identity) = ActorIdentities::<T>::get(entry.aaa_id) else {
      cycle_meter.consume(hot_probe_weight);
      return FifoStepResult::NoWork;
    };
    cycle_meter.consume(hot_probe_weight);
    if hot.queue_ticket != Some(entry.ticket) {
      return FifoStepResult::NoWork;
    }
    if hot.run_state == RunState::Suspended {
      if ContinuationStateStore::<T>::get(entry.aaa_id)
        .is_some_and(|continuation| continuation.last_attempt_block == now)
      {
        return FifoStepResult::Blocked(BlockKind::NonWeight);
      }
    } else if identity.cycle_nonce > 0 && hot.last_cycle_block == Some(now) {
      return FifoStepResult::Blocked(BlockKind::NonWeight);
    }
    if hot.lifecycle.is_paused() && hot.terminal_at.is_none_or(|terminal_at| terminal_at > now) {
      if Self::paged_consume_head_at(position).is_err() {
        return FifoStepResult::Blocked(BlockKind::NonWeight);
      }
      cycle_meter.consume(consume_weight);
      return FifoStepResult::Progress { executed: false };
    }
    if !cycle_meter.can_consume(program_probe_weight.saturating_add(consume_weight)) {
      return FifoStepResult::Blocked(BlockKind::Weight);
    }
    let Some(program) = ActorProgram::<T>::get(entry.aaa_id) else {
      cycle_meter.consume(program_probe_weight);
      if Self::paged_consume_head_at(position).is_err() {
        return FifoStepResult::Blocked(BlockKind::NonWeight);
      }
      cycle_meter.consume(consume_weight);
      return FifoStepResult::Progress { executed: false };
    };
    cycle_meter.consume(program_probe_weight);
    let aaa_id = entry.aaa_id;
    let instance = Self::compose_active_actor(identity, hot, program);
    match Self::apply_admission(aaa_id, &instance, cycle_meter) {
      AdmissionDecision::Admit(weight) => {
        if !cycle_meter.can_consume(consume_weight.saturating_add(weight)) {
          let reason = Self::deferred_dimension(cycle_meter, consume_weight.saturating_add(weight));
          Self::try_emit_cycle_deferred(aaa_id, &instance, reason, cycle_meter);
          return FifoStepResult::Blocked(BlockKind::Weight);
        }
        let outcome = polkadot_sdk::frame_support::storage::with_transaction(|| {
          if Self::paged_consume_head_at(position).is_err() {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
              polkadot_sdk::sp_runtime::DispatchError::Other("scheduler queue topology changed"),
            ));
          }
          let _actual = Self::execute_single_cycle(aaa_id, instance, now);
          if let Some(updated) = Self::active_actor_snapshot(aaa_id) {
            if Self::schedule_next_work(aaa_id, &updated, now).is_err() {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                polkadot_sdk::sp_runtime::DispatchError::Other("post-attempt placement failed"),
              ));
            }
          }
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
        });
        cycle_meter.consume(consume_weight.saturating_add(weight));
        match outcome {
          Ok(()) => FifoStepResult::Progress { executed: true },
          Err(_) => FifoStepResult::Blocked(BlockKind::NonWeight),
        }
      }
      AdmissionDecision::Close { reason, weight } => {
        let atomic_weight = consume_weight.saturating_add(weight);
        if !cycle_meter.can_consume(atomic_weight) {
          return FifoStepResult::Blocked(BlockKind::Weight);
        }
        let outcome = polkadot_sdk::frame_support::storage::with_transaction(|| {
          if let Err(error) = Self::close_actor(aaa_id, &instance, reason) {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
          }
          Self::apply_test_close_queue_corruption();
          if Self::paged_consume_head_at(position).is_err() {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
              polkadot_sdk::sp_runtime::DispatchError::Other("scheduler queue head changed"),
            ));
          }
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
        });
        cycle_meter.consume(atomic_weight);
        match outcome {
          Ok(()) => FifoStepResult::Progress { executed: false },
          Err(_) => FifoStepResult::Blocked(BlockKind::NonWeight),
        }
      }
      AdmissionDecision::Defer(reason) => {
        Self::try_emit_cycle_deferred(aaa_id, &instance, reason, cycle_meter);
        FifoStepResult::Blocked(BlockKind::Weight)
      }
      AdmissionDecision::Skip => {
        let outcome = polkadot_sdk::frame_support::storage::with_transaction(|| {
          if Self::paged_consume_head_at(position).is_err() {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
              polkadot_sdk::sp_runtime::DispatchError::Other("scheduler queue topology changed"),
            ));
          }
          if let Some(updated) = Self::active_actor_snapshot(aaa_id) {
            if Self::schedule_next_work(aaa_id, &updated, now).is_err() {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                polkadot_sdk::sp_runtime::DispatchError::Other("post-skip placement failed"),
              ));
            }
          }
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
        });
        cycle_meter.consume(consume_weight);
        match outcome {
          Ok(()) => FifoStepResult::Progress { executed: false },
          Err(_) => FifoStepResult::Blocked(BlockKind::NonWeight),
        }
      }
    }
  }

  pub(crate) fn enqueue(aaa_id: AaaId) -> Result<(), EnqueueOutcome> {
    match Self::try_paged_enqueue(aaa_id) {
      Ok(()) => Ok(()),
      Err(EnqueueOutcome::AlreadyLive) => Ok(()),
      Err(EnqueueOutcome::CapacityUnavailable) => {
        // Queue saturation preserves readiness through an exact next-block wakeup
        // (spec 8.1.4). A failure to place that wakeup must fail closed rather than
        // silently leave the actor with neither a live ticket nor a wakeup.
        let next_block = frame_system::Pallet::<T>::block_number()
          .checked_add(&One::one())
          .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
        Self::defer_wakeup(aaa_id, next_block)
      }
      Err(other) => Err(other),
    }
  }

  #[cfg(test)]
  pub(crate) fn test_corrupt_queue_before_close_consume() {
    CORRUPT_QUEUE_BEFORE_CLOSE_CONSUME.with(|flag| flag.set(true));
  }

  #[cfg(test)]
  fn apply_test_close_queue_corruption() {
    CORRUPT_QUEUE_BEFORE_CLOSE_CONSUME.with(|flag| {
      if flag.replace(false) {
        QueueTail::<T>::mutate(|tail| *tail = tail.saturating_add(1));
      }
    });
  }

  #[cfg(not(test))]
  fn apply_test_close_queue_corruption() {}

  fn queue_page_size() -> u64 {
    u64::from(T::QueuePageSize::get())
  }

  fn queue_page_and_slot(position: QueueTicket) -> (QueuePageId, usize) {
    let page_size = Self::queue_page_size();
    ((position / page_size), (position % page_size) as usize)
  }

  fn queue_topology_preflight(mutation: QueueMutation) -> Result<QueueTopology, EnqueueOutcome> {
    let head = QueueHead::<T>::get();
    let tail = QueueTail::<T>::get();
    let occupancy = QueueOccupancy::<T>::get();
    let span = tail
      .checked_sub(head)
      .ok_or(EnqueueOutcome::CorruptedTopology)?;
    if span != u64::from(occupancy) || occupancy > T::MaxQueueLength::get() {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    let page_size = Self::queue_page_size();
    let page_size_usize = page_size as usize;
    if occupancy == 0 {
      if !head.is_multiple_of(page_size) {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
      if matches!(mutation, QueueMutation::Enqueue) {
        let (tail_page_id, tail_slot) = Self::queue_page_and_slot(tail);
        if tail_slot != 0 || QueuePages::<T>::contains_key(tail_page_id) {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
      }
      return Ok(QueueTopology {
        head,
        tail,
        occupancy,
      });
    }

    let (head_page_id, head_slot) = Self::queue_page_and_slot(head);
    let head_page = QueuePages::<T>::get(head_page_id).ok_or(EnqueueOutcome::CorruptedTopology)?;
    if head_page.is_empty() || head_page.len() > page_size_usize || head_slot >= head_page.len() {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    let (tail_page_id, tail_slot) = Self::queue_page_and_slot(tail);
    if head_page_id == tail_page_id {
      if tail_slot == 0 || head_page.len() != tail_slot {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
    } else {
      if head_page.len() != page_size_usize {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
      if tail_slot == 0 {
        if QueuePages::<T>::contains_key(tail_page_id) {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
      } else {
        let tail_page =
          QueuePages::<T>::get(tail_page_id).ok_or(EnqueueOutcome::CorruptedTopology)?;
        if tail_page.len() != tail_slot || tail_page.len() > page_size_usize {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
      }
    }
    Ok(QueueTopology {
      head,
      tail,
      occupancy,
    })
  }

  pub fn combined_queue_occupancy() -> u64 {
    u64::from(QueueOccupancy::<T>::get())
  }

  /// Appends one actor to the canonical FIFO using the global ticket allocator.
  pub fn paged_enqueue(aaa_id: AaaId) -> bool {
    matches!(
      Self::try_paged_enqueue(aaa_id),
      Ok(()) | Err(EnqueueOutcome::AlreadyLive)
    )
  }

  pub fn try_paged_enqueue(aaa_id: AaaId) -> Result<(), EnqueueOutcome> {
    with_transaction_opaque_err(|| {
      let transition = || -> Result<(), EnqueueOutcome> {
        let Some(mut hot) = ActorHot::<T>::get(aaa_id) else {
          return Err(EnqueueOutcome::CapacityUnavailable);
        };
        if hot.queue_ticket.is_some() || !ActorIdentities::<T>::contains_key(aaa_id) {
          return if hot.queue_ticket.is_some() {
            Err(EnqueueOutcome::AlreadyLive)
          } else {
            Err(EnqueueOutcome::CapacityUnavailable)
          };
        }
        let topology = Self::queue_topology_preflight(QueueMutation::Enqueue)?;
        if topology.occupancy >= T::MaxQueueLength::get() {
          return Err(EnqueueOutcome::CapacityUnavailable);
        }
        let ticket = NextQueueTicket::<T>::get();
        let next_ticket = ticket
          .checked_add(1)
          .ok_or(EnqueueOutcome::TicketExhausted)?;
        let next_tail = topology
          .tail
          .checked_add(1)
          .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
        let next_occupancy = topology
          .occupancy
          .checked_add(1)
          .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
        let (page_id, slot) = Self::queue_page_and_slot(topology.tail);
        let mut page = QueuePages::<T>::get(page_id).unwrap_or_default();
        if page.len() != slot || page.try_push(QueueEntry { ticket, aaa_id }).is_err() {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        hot.queue_ticket = Some(ticket);
        QueuePages::<T>::insert(page_id, page);
        QueueTail::<T>::put(next_tail);
        QueueOccupancy::<T>::put(next_occupancy);
        NextQueueTicket::<T>::put(next_ticket);
        ActorHot::<T>::insert(aaa_id, hot);
        Ok(())
      };
      match transition() {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  /// Maps a placement result to the public error surface for extrinsic boundaries.
  pub fn enqueue_outcome_error(outcome: Result<(), EnqueueOutcome>) -> Result<(), DispatchError> {
    match outcome {
      Ok(()) => Ok(()),
      Err(EnqueueOutcome::Ok) => Ok(()),
      Err(EnqueueOutcome::AlreadyLive) => Ok(()),
      Err(EnqueueOutcome::CapacityUnavailable) => Err(Error::<T>::QueueCapacityUnavailable.into()),
      Err(EnqueueOutcome::TicketExhausted) => Err(Error::<T>::QueueTicketExhausted.into()),
      Err(EnqueueOutcome::SchedulerIndexExhausted) => {
        Err(Error::<T>::SchedulerIndexExhausted.into())
      }
      Err(EnqueueOutcome::WakeupCapacityExhausted) => {
        Err(Error::<T>::QueueCapacityUnavailable.into())
      }
      Err(EnqueueOutcome::WakeupIndexExhausted) => Err(Error::<T>::SchedulerIndexExhausted.into()),
      Err(EnqueueOutcome::CorruptedTopology) => Err(Error::<T>::SchedulerIndexExhausted.into()),
    }
  }

  /// Extracts the public error from a failed placement outcome for `map_err` sites.
  pub fn placement_error(outcome: EnqueueOutcome) -> DispatchError {
    match Self::enqueue_outcome_error(Err(outcome)) {
      Ok(()) => unreachable!("placement error cannot map to Ok"),
      Err(error) => error,
    }
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

  pub(crate) fn paged_consume_head_at(position: QueueTicket) -> Result<(), EnqueueOutcome> {
    with_transaction_opaque_err(|| {
      let transition = || -> Result<(), EnqueueOutcome> {
        let topology = Self::queue_topology_preflight(QueueMutation::Head)?;
        if position != topology.head || topology.head >= topology.tail {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        let (page_id, slot) = Self::queue_page_and_slot(topology.head);
        let page = QueuePages::<T>::get(page_id).ok_or(EnqueueOutcome::CorruptedTopology)?;
        let entry = page
          .get(slot)
          .copied()
          .ok_or(EnqueueOutcome::CorruptedTopology)?;
        let mut actor_hot = ActorHot::<T>::get(entry.aaa_id);
        if actor_hot
          .as_ref()
          .is_some_and(|hot| hot.queue_ticket != Some(entry.ticket))
        {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        let next_head = topology
          .head
          .checked_add(1)
          .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
        let next_occupancy = topology
          .occupancy
          .checked_sub(1)
          .ok_or(EnqueueOutcome::CorruptedTopology)?;
        let page_size = Self::queue_page_size();
        if next_head == topology.tail {
          let remainder = next_head % page_size;
          let aligned = if remainder == 0 {
            next_head
          } else {
            let distance = page_size
              .checked_sub(remainder)
              .ok_or(EnqueueOutcome::CorruptedTopology)?;
            next_head
              .checked_add(distance)
              .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?
          };
          QueuePages::<T>::remove(page_id);
          QueueHead::<T>::put(aligned);
          QueueTail::<T>::put(aligned);
        } else {
          QueueHead::<T>::put(next_head);
          if next_head.is_multiple_of(page_size) {
            QueuePages::<T>::remove(page_id);
          }
        }
        QueueOccupancy::<T>::put(next_occupancy);
        if let Some(hot) = actor_hot.as_mut() {
          hot.queue_ticket = None;
          ActorHot::<T>::insert(entry.aaa_id, hot);
        }
        Ok(())
      };
      match transition() {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  pub fn paged_consume_head(ticket: QueueTicket) -> bool {
    let Some((position, entry)) = Self::paged_head_entry() else {
      return false;
    };
    entry.ticket == ticket && Self::paged_consume_head_at(position).is_ok()
  }

  pub fn paged_drain_tombstones(
    cutoff: QueueTicket,
    scan_limit: u32,
  ) -> Result<QueueDrainStats, EnqueueOutcome> {
    let outcome = with_transaction_opaque_err(|| {
      let corrupt = || {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          EnqueueOutcome::CorruptedTopology,
        ))
      };
      let mut stats = QueueDrainStats::default();
      if scan_limit == 0 {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(stats));
      }
      let Ok(topology) = Self::queue_topology_preflight(QueueMutation::Head) else {
        return corrupt();
      };
      let original_head = topology.head;
      let tail = topology.tail;
      let occupancy = topology.occupancy;
      let next_ticket = NextQueueTicket::<T>::get();
      let page_size = Self::queue_page_size();
      let page_size_usize = page_size as usize;
      let mut head = original_head;
      let mut last_ticket = None;
      let mut pages_to_delete: Vec<QueuePageId> = Vec::new();

      'pages: while head < tail && stats.entries_scanned < scan_limit {
        let (page_id, mut slot) = Self::queue_page_and_slot(head);
        let Some(page) = QueuePages::<T>::get(page_id) else {
          return corrupt();
        };
        if page.is_empty() || page.len() > page_size_usize || slot >= page.len() {
          return corrupt();
        }
        let Some(pages_touched) = stats.pages_touched.checked_add(1) else {
          return corrupt();
        };
        stats.pages_touched = pages_touched;
        while head < tail && stats.entries_scanned < scan_limit && slot < page.len() {
          let entry = page[slot];
          if entry.ticket >= next_ticket
            || last_ticket.is_some_and(|previous| entry.ticket <= previous)
          {
            return corrupt();
          }
          last_ticket = Some(entry.ticket);
          if entry.ticket >= cutoff {
            break 'pages;
          }
          let Some(entries_scanned) = stats.entries_scanned.checked_add(1) else {
            return corrupt();
          };
          stats.entries_scanned = entries_scanned;
          let is_live = ActorHot::<T>::get(entry.aaa_id)
            .is_some_and(|hot| hot.queue_ticket == Some(entry.ticket))
            && ActorIdentities::<T>::contains_key(entry.aaa_id);
          if is_live {
            break 'pages;
          }
          let Some(tombstones_skipped) = stats.tombstones_skipped.checked_add(1) else {
            return corrupt();
          };
          let Some(next_head) = head.checked_add(1) else {
            return corrupt();
          };
          let Some(next_slot) = slot.checked_add(1) else {
            return corrupt();
          };
          stats.tombstones_skipped = tombstones_skipped;
          head = next_head;
          slot = next_slot;
        }
        if slot == page.len() {
          if head < tail && (page.len() != page_size_usize || !head.is_multiple_of(page_size)) {
            return corrupt();
          }
          pages_to_delete.push(page_id);
          let Some(pages_deleted) = stats.pages_deleted.checked_add(1) else {
            return corrupt();
          };
          stats.pages_deleted = pages_deleted;
        } else {
          break;
        }
      }

      if head == original_head {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(stats));
      }
      let Some(next_occupancy) = occupancy.checked_sub(stats.tombstones_skipped) else {
        return corrupt();
      };
      let (next_head, next_tail) = if head == tail {
        let remainder = tail % page_size;
        let Some(aligned) = (if remainder == 0 {
          Some(tail)
        } else {
          tail.checked_add(page_size - remainder)
        }) else {
          return corrupt();
        };
        (aligned, aligned)
      } else {
        (head, tail)
      };
      for page_id in pages_to_delete {
        QueuePages::<T>::remove(page_id);
      }
      QueueHead::<T>::put(next_head);
      QueueTail::<T>::put(next_tail);
      QueueOccupancy::<T>::put(next_occupancy);
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(stats))
    });
    outcome
      .map_err(|_| EnqueueOutcome::CorruptedTopology)?
      .map_err(|_| EnqueueOutcome::CorruptedTopology)
  }

  pub(crate) fn wakeup_page_entry_matches(
    pointer: WakeupPointer<BlockNumberFor<T>>,
    aaa_id: AaaId,
  ) -> bool {
    WakeupPages::<T>::get((pointer.block, pointer.page_id))
      .and_then(|page| page.entries.get(pointer.slot as usize).copied().flatten())
      .is_some_and(|entry| entry.aaa_id == aaa_id)
  }

  fn wakeup_substrate_invalidate_inner(
    aaa_id: AaaId,
  ) -> Result<Option<WakeupPointer<BlockNumberFor<T>>>, EnqueueOutcome> {
    let Some(mut hot) = ActorHot::<T>::get(aaa_id) else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    let Some(pointer) = hot.wakeup_pointer else {
      return Ok(None);
    };
    let key = (pointer.block, pointer.page_id);
    let Some(mut page) = WakeupPages::<T>::get(key) else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    let Some(slot) = page.entries.get(pointer.slot as usize) else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    if !slot.is_some_and(|entry| entry.aaa_id == aaa_id) {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    let physical_live = page
      .entries
      .iter() // deos-bypass: bounded-iter — WakeupPageSize-bounded reciprocity check
      .filter(|entry| entry.is_some())
      .count();
    if physical_live != page.live_entries as usize {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    let Some(next_page_live) = page.live_entries.checked_sub(1) else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    let Some(mut bucket) = WakeupBuckets::<T>::get(pointer.block) else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    let Some(cursor_index) = bucket.cursor_index else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    if Self::wakeup_cursor_get(cursor_index) != Some(pointer.block) {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    let Some(next_bucket_live) = bucket.live_entries.checked_sub(1) else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    if page.previous_page.is_none() != (bucket.head_page == pointer.page_id)
      || page.next_page.is_none() != (bucket.tail_page == pointer.page_id)
    {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    let previous = if let Some(previous_page) = page.previous_page {
      let Some(previous) = WakeupPages::<T>::get((pointer.block, previous_page)) else {
        return Err(EnqueueOutcome::CorruptedTopology);
      };
      if previous.next_page != Some(pointer.page_id) {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
      Some((previous_page, previous))
    } else {
      None
    };
    let next = if let Some(next_page) = page.next_page {
      let Some(next) = WakeupPages::<T>::get((pointer.block, next_page)) else {
        return Err(EnqueueOutcome::CorruptedTopology);
      };
      if next.previous_page != Some(pointer.page_id) {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
      Some((next_page, next))
    } else {
      None
    };
    if next_page_live == 0 && ((next_bucket_live == 0) != (previous.is_none() && next.is_none())) {
      return Err(EnqueueOutcome::CorruptedTopology);
    }

    page.entries[pointer.slot as usize] = None;
    page.live_entries = next_page_live;
    bucket.live_entries = next_bucket_live;
    hot.wakeup_pointer = None;
    ActorHot::<T>::insert(aaa_id, hot);
    if page.live_entries > 0 {
      WakeupPages::<T>::insert(key, page);
      WakeupBuckets::<T>::insert(pointer.block, bucket);
      return Ok(Some(pointer));
    }

    if let Some((previous_page, mut previous)) = previous {
      previous.next_page = page.next_page;
      WakeupPages::<T>::insert((pointer.block, previous_page), previous);
    } else if let Some(next_page) = page.next_page {
      bucket.head_page = next_page;
    }
    if let Some((next_page, mut next)) = next {
      next.previous_page = page.previous_page;
      WakeupPages::<T>::insert((pointer.block, next_page), next);
    } else if let Some(previous_page) = page.previous_page {
      bucket.tail_page = previous_page;
    }
    WakeupPages::<T>::remove(key);
    if bucket.live_entries == 0 {
      if page.previous_page.is_some() || page.next_page.is_some() {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
      if !Self::wakeup_cursor_remove_inner(pointer.block) {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
      WakeupBuckets::<T>::remove(pointer.block);
    } else {
      WakeupBuckets::<T>::insert(pointer.block, bucket);
    }
    Ok(Some(pointer))
  }

  pub fn wakeup_substrate_invalidate(aaa_id: AaaId) -> Option<WakeupPointer<BlockNumberFor<T>>> {
    let result: Result<WakeupPointer<BlockNumberFor<T>>, DispatchError> =
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Self::wakeup_substrate_invalidate_inner(aaa_id) {
          Ok(Some(pointer)) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(pointer))
          }
          Ok(None) | Err(_) => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(
            Err(Error::<T>::AaaNotFound.into()),
          ),
        }
      });
    result.ok()
  }

  fn wakeup_substrate_schedule_inner(aaa_id: AaaId, wakeup_block: BlockNumberFor<T>) -> bool {
    matches!(
      Self::try_wakeup_substrate_schedule_inner(aaa_id, wakeup_block),
      Ok(()) | Err(EnqueueOutcome::AlreadyLive)
    )
  }

  pub(crate) fn try_wakeup_substrate_schedule_inner(
    aaa_id: AaaId,
    wakeup_block: BlockNumberFor<T>,
  ) -> Result<(), EnqueueOutcome> {
    with_transaction_opaque_err(|| {
      match Self::try_wakeup_substrate_schedule_transition(aaa_id, wakeup_block) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  fn try_wakeup_substrate_schedule_transition(
    aaa_id: AaaId,
    wakeup_block: BlockNumberFor<T>,
  ) -> Result<(), EnqueueOutcome> {
    let Some(hot) = ActorHot::<T>::get(aaa_id) else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    if let Some(pointer) = hot.wakeup_pointer {
      if pointer.block == wakeup_block && Self::wakeup_page_entry_matches(pointer, aaa_id) {
        return Err(EnqueueOutcome::AlreadyLive);
      }
      Self::wakeup_substrate_invalidate_inner(aaa_id)?;
    }

    let (page_id, slot) = if let Some(mut bucket) = WakeupBuckets::<T>::get(wakeup_block) {
      let Some(cursor_index) = bucket.cursor_index else {
        return Err(EnqueueOutcome::CorruptedTopology);
      };
      if Self::wakeup_cursor_get(cursor_index) != Some(wakeup_block) {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
      let tail_key = (wakeup_block, bucket.tail_page);
      let Some(mut tail_page) = WakeupPages::<T>::get(tail_key) else {
        return Err(EnqueueOutcome::CorruptedTopology);
      };
      if tail_page.next_page.is_some()
        || tail_page
          .entries
          .iter() // deos-bypass: bounded-iter — WakeupPageSize-bounded reciprocity check
          .filter(|entry| entry.is_some())
          .count()
          != tail_page.live_entries as usize
      {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
      let Some(next_bucket_live) = bucket.live_entries.checked_add(1) else {
        return Err(EnqueueOutcome::WakeupIndexExhausted);
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
          return Err(EnqueueOutcome::WakeupCapacityExhausted);
        }
        slot
      } else {
        let page_id = bucket.next_page_id;
        let Some(next_page_id) = page_id.checked_add(1) else {
          return Err(EnqueueOutcome::WakeupIndexExhausted);
        };
        let mut entries = WakeupPageEntriesOf::<T>::default();
        if entries.try_push(Some(WakeupEntry { aaa_id })).is_err() {
          return Err(EnqueueOutcome::WakeupCapacityExhausted);
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
        bucket.live_entries = next_bucket_live;
        WakeupBuckets::<T>::insert(wakeup_block, bucket);
        Self::set_wakeup_pointer(aaa_id, wakeup_block, page_id, 0)?;
        return Ok(());
      };
      tail_page.live_entries = tail_page
        .live_entries
        .checked_add(1)
        .ok_or(EnqueueOutcome::WakeupIndexExhausted)?;
      let page_id = bucket.tail_page;
      WakeupPages::<T>::insert(tail_key, tail_page);
      bucket.live_entries = next_bucket_live;
      WakeupBuckets::<T>::insert(wakeup_block, bucket);
      (page_id, slot as WakeupSlot)
    } else {
      let mut entries = WakeupPageEntriesOf::<T>::default();
      if entries.try_push(Some(WakeupEntry { aaa_id })).is_err() {
        return Err(EnqueueOutcome::WakeupCapacityExhausted);
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
        return Err(EnqueueOutcome::WakeupIndexExhausted);
      }
      (0, 0)
    };
    Self::set_wakeup_pointer(aaa_id, wakeup_block, page_id, slot)?;
    Ok(())
  }

  fn set_wakeup_pointer(
    aaa_id: AaaId,
    block: BlockNumberFor<T>,
    page_id: WakeupPageId,
    slot: WakeupSlot,
  ) -> Result<(), EnqueueOutcome> {
    let pointer = WakeupPointer {
      block,
      page_id,
      slot,
    };
    ActorHot::<T>::try_mutate(aaa_id, |maybe_hot| {
      let hot = maybe_hot
        .as_mut()
        .ok_or(EnqueueOutcome::CorruptedTopology)?;
      if hot.wakeup_pointer.is_some() {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
      hot.wakeup_pointer = Some(pointer);
      Ok(())
    })
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
    let Some(cursor_index) = bucket.cursor_index else {
      return None;
    };
    if Self::wakeup_cursor_get(cursor_index) != Some(wakeup_block) {
      return None;
    }
    let mut page_id = bucket.head_page;

    while stats.entries_scanned < scan_limit {
      let key = (wakeup_block, page_id);
      let Some(mut page) = WakeupPages::<T>::get(key) else {
        return None;
      };
      if page
        .entries
        .iter() // deos-bypass: bounded-iter — WakeupPageSize-bounded reciprocity check
        .filter(|entry| entry.is_some())
        .count()
        != page.live_entries as usize
      {
        return None;
      }
      stats.pages_touched = stats.pages_touched.saturating_add(1);
      let mut slot = page.scan_slot as usize;
      while slot < page.entries.len() && stats.entries_scanned < scan_limit {
        let entry = page.entries[slot].take();
        let Some(next_scan_slot) = (slot as WakeupSlot).checked_add(1) else {
          return None;
        };
        let Some(next_slot) = slot.checked_add(1) else {
          return None;
        };
        page.scan_slot = next_scan_slot;
        stats.entries_scanned = stats.entries_scanned.saturating_add(1);
        slot = next_slot;
        let Some(entry) = entry else {
          continue;
        };
        page.live_entries = page.live_entries.checked_sub(1)?;
        bucket.live_entries = bucket.live_entries.checked_sub(1)?;
        let pointer_slot = slot.checked_sub(1)?;
        let pointer = WakeupPointer {
          block: wakeup_block,
          page_id,
          slot: pointer_slot as WakeupSlot,
        };
        let is_live =
          ActorHot::<T>::get(entry.aaa_id).and_then(|hot| hot.wakeup_pointer) == Some(pointer);
        if !is_live {
          stats.stale_entries = stats.stale_entries.saturating_add(1);
          continue;
        }
        if ready.try_push(entry.aaa_id).is_err() {
          page.entries[pointer_slot] = Some(entry);
          page.live_entries = page.live_entries.checked_add(1)?;
          bucket.live_entries = bucket.live_entries.checked_add(1)?;
          page.scan_slot = pointer_slot as WakeupSlot;
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
    if slot.checked_add(1) != Some(page.len()) {
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
    let Some(mut left_bucket) = WakeupBuckets::<T>::get(left_block) else {
      return false;
    };
    let Some(mut right_bucket) = WakeupBuckets::<T>::get(right_block) else {
      return false;
    };
    if left_bucket.cursor_index != Some(left) || right_bucket.cursor_index != Some(right) {
      return false;
    }
    if !Self::wakeup_cursor_set(left, right_block) || !Self::wakeup_cursor_set(right, left_block) {
      return false;
    }
    right_bucket.cursor_index = Some(left);
    left_bucket.cursor_index = Some(right);
    WakeupBuckets::<T>::insert(right_block, right_bucket);
    WakeupBuckets::<T>::insert(left_block, left_bucket);
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
    let Some(next_len) = len.checked_add(1) else {
      return false;
    };
    if len >= T::MaxActiveActors::get() || !Self::wakeup_cursor_set(len, block) {
      return false;
    }
    bucket.cursor_index = Some(len);
    WakeupBuckets::<T>::insert(block, bucket);
    WakeupCursorLen::<T>::put(next_len);
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
    let Some(mut target_bucket) = WakeupBuckets::<T>::get(block) else {
      return false;
    };
    let Some(index) = target_bucket.cursor_index else {
      return false;
    };
    let len = WakeupCursorLen::<T>::get();
    if index >= len || Self::wakeup_cursor_get(index) != Some(block) {
      return false;
    }
    let Some(last_index) = len.checked_sub(1) else {
      return false;
    };
    let Some(last_block) = Self::wakeup_cursor_get(last_index) else {
      return false;
    };
    let mut last_bucket = if last_block == block {
      target_bucket
    } else {
      let Some(last_bucket) = WakeupBuckets::<T>::get(last_block) else {
        return false;
      };
      last_bucket
    };
    if last_bucket.cursor_index != Some(last_index) || !Self::wakeup_cursor_remove_tail(last_index)
    {
      return false;
    }
    target_bucket.cursor_index = None;
    WakeupBuckets::<T>::insert(block, target_bucket);
    WakeupCursorLen::<T>::put(last_index);
    if index == last_index {
      return true;
    }
    if !Self::wakeup_cursor_set(index, last_block) {
      return false;
    }
    last_bucket.cursor_index = Some(index);
    WakeupBuckets::<T>::insert(last_block, last_bucket);

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

  pub(crate) fn prime_actor_schedule(aaa_id: AaaId) -> Result<(), EnqueueOutcome> {
    let Some(instance) = Self::active_actor_snapshot(aaa_id) else {
      return Ok(());
    };
    let now = frame_system::Pallet::<T>::block_number();
    if instance.lifecycle.is_paused() {
      return Self::schedule_window_expiry(aaa_id, &instance);
    }
    Self::schedule_next_work(aaa_id, &instance, now)
  }

  fn window_expiry_wakeup(instance: &AaaInstanceOf<T>) -> Option<BlockNumberFor<T>> {
    instance
      .schedule_window
      .map(|window| Self::window_terminal_at(&window))
  }

  fn schedule_window_expiry(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    if let Some(expiry) = Self::window_expiry_wakeup(instance) {
      Self::defer_wakeup(aaa_id, expiry)
    } else {
      Ok(())
    }
  }

  fn defer_wakeup(aaa_id: AaaId, wakeup_block: BlockNumberFor<T>) -> Result<(), EnqueueOutcome> {
    let target = Self::active_actor_snapshot(aaa_id)
      .and_then(|instance| Self::window_expiry_wakeup(&instance))
      .map(|expiry| wakeup_block.min(expiry))
      .unwrap_or(wakeup_block);
    match Self::try_wakeup_substrate_schedule_inner(aaa_id, target) {
      Ok(()) => Ok(()),
      Err(EnqueueOutcome::AlreadyLive) => Ok(()),
      Err(other) => Err(other),
    }
  }

  /// Baseline scheduler envelope reserved ahead of one actor run plus pure cleanup.
  /// Explicit permissionless repair sweeps remain dispatch-owned and do not consume every block's
  /// guaranteed scheduler envelope.
  pub fn scheduler_admission_overhead() -> Weight {
    T::WeightInfo::scheduler_on_idle_base()
      .saturating_add(T::WeightInfo::scheduler_paged_tombstone_drain(1).saturating_mul(2))
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

  /// Generated envelope for the complete `CycleDeferred` candidate calculation
  /// and event write after the actor program probe has already been charged.
  pub fn deferred_event_weight_upper() -> Weight {
    T::WeightInfo::scheduler_cycle_deferral_dimension()
      .saturating_sub(T::WeightInfo::scheduler_actor_program_probe())
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_scheduler_actor_hot_probe(aaa_id: AaaId) {
    let hot = ActorHot::<T>::get(aaa_id).expect("benchmark actor hot state must exist");
    assert!(hot.lifecycle.is_paused());
    core::hint::black_box(hot);
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_scheduler_actor_program_probe(aaa_id: AaaId, hot: ActorHotStateOf<T>) {
    let identity = ActorIdentities::<T>::get(aaa_id).expect("benchmark actor identity must exist");
    let program = ActorProgram::<T>::get(aaa_id).expect("benchmark actor program state must exist");
    let instance = Self::compose_active_actor(identity, hot, program);
    let meter = WeightMeter::with_limit(Weight::zero());
    let AdmissionDecision::Defer(reason) = Self::apply_admission(aaa_id, &instance, &meter) else {
      panic!("benchmark actor must defer on an exhausted cycle budget");
    };
    Self::benchmark_cycle_deferral(aaa_id, &instance, reason);
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_scheduler_cycle_deferral_dimension(
    aaa_id: AaaId,
    hot: ActorHotStateOf<T>,
    dimension: DeferReason,
  ) {
    let identity = ActorIdentities::<T>::get(aaa_id).expect("benchmark actor identity must exist");
    let program = ActorProgram::<T>::get(aaa_id).expect("benchmark actor program state must exist");
    let instance = Self::compose_active_actor(identity, hot, program);
    let cycle_weight_upper = Self::cycle_admission_weight_upper(&instance, 0, None);
    // Force exactly one dimension exhausted: limit RefTime only, ProofSize only,
    // or both, so each DeferReason branch is measured.
    let limit = match dimension {
      DeferReason::RefTime => Weight::from_parts(
        cycle_weight_upper.ref_time().saturating_sub(1),
        Weight::MAX.proof_size(),
      ),
      DeferReason::ProofSize => Weight::from_parts(
        Weight::MAX.ref_time(),
        cycle_weight_upper.proof_size().saturating_sub(1),
      ),
      DeferReason::Both => Weight::from_parts(
        cycle_weight_upper.ref_time().saturating_sub(1),
        cycle_weight_upper.proof_size().saturating_sub(1),
      ),
    };
    let meter = WeightMeter::with_limit(limit);
    let AdmissionDecision::Defer(reason) = Self::apply_admission(aaa_id, &instance, &meter) else {
      panic!("benchmark actor must defer on an exhausted cycle budget");
    };
    debug_assert_eq!(reason, dimension);
    Self::benchmark_cycle_deferral(aaa_id, &instance, reason);
  }

  #[cfg(feature = "runtime-benchmarks")]
  fn benchmark_cycle_deferral(aaa_id: AaaId, instance: &AaaInstanceOf<T>, reason: DeferReason) {
    let (candidate_cycle_nonce, candidate_attempt, cursor) =
      Self::deferral_candidate(aaa_id, instance)
        .expect("benchmark candidate identity must remain within configured bounds");
    Self::deposit_event(Event::CycleDeferred {
      aaa_id,
      candidate_cycle_nonce,
      candidate_attempt,
      cursor,
      reason,
    });
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
    // Independent worker ceiling: the overdue wakeup worker may consume at most its configured
    // two-dimensional envelope from the shared on_idle meter, mirroring the fanout worker's
    // `ObservationFanoutWeightLimit`. Actor service receives the remaining budget; there is no
    // guarantee lending (spec 8.4.5).
    let mut worker_meter = WeightMeter::with_limit(T::WakeupWeightLimit::get());
    while total.entries_scanned < max_scans {
      let block_cursor = if let Some(block) = current_block {
        block
      } else {
        let cursor_weight = T::WeightInfo::scheduler_wakeup_cursor_worker_future();
        if !worker_meter.can_consume(cursor_weight) || !meter.can_consume(cursor_weight) {
          break;
        }
        worker_meter.consume(cursor_weight);
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
        || !worker_meter.can_consume(base_weight)
        || !meter.can_consume(base_weight)
      {
        break;
      }
      let Some(bucket) = WakeupBuckets::<T>::get(block_cursor) else {
        worker_meter.consume(base_weight);
        meter.consume(base_weight);
        break;
      };
      let unit_weight = Self::wakeup_cursor_drain_unit_weight_upper(bucket.live_entries <= 1);
      if !worker_meter.can_consume(unit_weight) || !meter.can_consume(unit_weight) {
        worker_meter.consume(base_weight);
        meter.consume(base_weight);
        break;
      }
      worker_meter.consume(unit_weight);
      meter.consume(unit_weight);
      let outcome = polkadot_sdk::frame_support::storage::with_transaction(|| {
        let (ready, stats) = Self::wakeup_substrate_drain_block(block_cursor, 1);
        if stats.entries_scanned == 0 {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(stats));
        }
        for aaa_id in ready {
          if Self::enqueue(aaa_id).is_err() {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
              polkadot_sdk::sp_runtime::DispatchError::Other("wakeup materialization failed"),
            ));
          }
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(stats))
      });
      let Ok(stats) = outcome else {
        break;
      };
      if stats.entries_scanned == 0 {
        break;
      }
      total.entries_scanned = total.entries_scanned.saturating_add(stats.entries_scanned);
      total.ready_entries = total.ready_entries.saturating_add(stats.ready_entries);
      total.stale_entries = total.stale_entries.saturating_add(stats.stale_entries);
      total.pages_touched = total.pages_touched.saturating_add(stats.pages_touched);
      total.pages_deleted = total.pages_deleted.saturating_add(stats.pages_deleted);
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
    let raw = u64::from_le_bytes([
      hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7],
    ]);
    (raw % u64::from(window)).saturated_into()
  }

  /// The Active-epoch clock anchor. Set to the current block (clamped to window
  /// start) at Active installation and schedule replacement; reactivation with
  /// `cycle_nonce > 0` uses it as the conservative cooldown anchor when no
  /// active-epoch `last_cycle_block` exists (spec 4.3).
  pub(crate) fn schedule_anchor_at(
    schedule_window: Option<ScheduleWindow<BlockNumberFor<T>>>,
    now: BlockNumberFor<T>,
  ) -> BlockNumberFor<T> {
    schedule_window
      .map(|window| now.max(window.start))
      .unwrap_or(now)
  }

  fn next_eligible_at(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    now: BlockNumberFor<T>,
    include_timer: bool,
  ) -> Result<BlockNumberFor<T>, EnqueueOutcome> {
    // Lifetime-first exemption exists only when `cycle_nonce == 0` and the Active
    // epoch has no recorded `last_cycle_block` (spec 4.3.1).
    let cooldown_anchor = instance
      .last_cycle_block
      .unwrap_or(instance.schedule_anchor);
    if instance.cycle_nonce == 0 && instance.last_cycle_block.is_none() {
      if include_timer {
        let first = instance.schedule_anchor;
        if now <= first {
          return Ok(first);
        }
        if let TriggerPolicy::Cadenced { every_blocks, .. } = instance.schedule.trigger {
          let jitter: u32 = Self::timer_jitter_blocks(aaa_id, every_blocks).saturated_into();
          let cadence_span = every_blocks
            .checked_add(jitter)
            .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
          let elapsed: u64 = now.saturating_sub(first).saturated_into();
          let span = u64::from(cadence_span.max(1));
          let periods = elapsed.div_ceil(span);
          let offset = periods
            .checked_mul(span)
            .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
          let offset_block: BlockNumberFor<T> = offset.saturated_into();
          let exact_offset: u64 = offset_block.saturated_into();
          if exact_offset != offset {
            return Err(EnqueueOutcome::SchedulerIndexExhausted);
          }
          return first
            .checked_add(&offset_block)
            .ok_or(EnqueueOutcome::SchedulerIndexExhausted);
        }
      }
      return Ok(
        instance
          .schedule_window
          .map(|window| now.max(window.start))
          .unwrap_or(now),
      );
    }
    let mut eligible_at = now;
    if let Some(window) = instance.schedule_window {
      eligible_at = eligible_at.max(window.start);
    }
    if instance.cycle_nonce < u64::MAX {
      let cooldown: BlockNumberFor<T> = instance.schedule.cooldown_blocks.into();
      let cooldown_target = cooldown_anchor
        .checked_add(&cooldown)
        .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
      eligible_at = eligible_at.max(cooldown_target);
    }
    if include_timer && instance.cycle_nonce < u64::MAX {
      if let TriggerPolicy::Cadenced { every_blocks, .. } = instance.schedule.trigger {
        let cadence: BlockNumberFor<T> = every_blocks.into();
        let jitter = Self::timer_jitter_blocks(aaa_id, every_blocks);
        let cadence_target = cooldown_anchor
          .checked_add(&cadence)
          .and_then(|target| target.checked_add(&jitter))
          .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
        eligible_at = eligible_at.max(cadence_target);
      }
    }
    Ok(eligible_at)
  }

  fn retry_backoff_blocks(attempt: u32) -> u32 {
    match attempt {
      0 => 1,
      1 => 2,
      2 => 4,
      _ => MAX_RETRY_BACKOFF_BLOCKS,
    }
  }

  pub(crate) fn retry_eligible_at(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
  ) -> Result<BlockNumberFor<T>, EnqueueOutcome> {
    let continuation =
      ContinuationStateStore::<T>::get(aaa_id).ok_or(EnqueueOutcome::CorruptedTopology)?;
    let cooldown: BlockNumberFor<T> = instance.schedule.cooldown_blocks.into();
    let backoff: BlockNumberFor<T> = Self::retry_backoff_blocks(continuation.attempt).into();
    let retry_delay = cooldown.max(backoff);
    let mut eligible_at = continuation
      .last_attempt_block
      .checked_add(&retry_delay)
      .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
    if let Some(window) = instance.schedule_window {
      eligible_at = eligible_at.max(window.start);
    }
    Ok(eligible_at)
  }

  fn schedule_next_work_local(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    now: BlockNumberFor<T>,
    requeues: &mut Vec<AaaId>,
  ) -> Result<(), EnqueueOutcome> {
    if instance.lifecycle.is_paused() {
      return Self::schedule_window_expiry(aaa_id, instance);
    }
    let eligible_at = if instance.run_state == RunState::Suspended {
      Self::retry_eligible_at(aaa_id, instance)?
    } else if instance.pending_signal {
      Self::next_eligible_at(
        aaa_id,
        instance,
        now,
        instance.schedule.trigger.cadence_blocks().is_some(),
      )?
    } else if matches!(instance.schedule.trigger, TriggerPolicy::Cadenced { .. }) {
      let exact_next_block = now
        .checked_add(&One::one())
        .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
      Self::next_eligible_at(aaa_id, instance, exact_next_block, true)?
    } else {
      return Self::schedule_window_expiry(aaa_id, instance);
    };
    let wakeup_at = instance.schedule_window.map_or(eligible_at, |window| {
      eligible_at.min(Self::window_terminal_at(&window))
    });
    let exact_next_block = now
      .checked_add(&One::one())
      .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
    if wakeup_at <= exact_next_block {
      requeues.push(aaa_id);
      Ok(())
    } else {
      Self::defer_wakeup(aaa_id, wakeup_at)
    }
  }

  fn schedule_next_work(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<(), EnqueueOutcome> {
    let mut requeues = Vec::new();
    Self::schedule_next_work_local(aaa_id, instance, now, &mut requeues)?;
    for aaa_id in requeues {
      Self::enqueue(aaa_id)?;
    }
    Ok(())
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
    if include_fee_budget {
      // The complete envelope must fit above MinUserBalance: charging it must not
      // cross the protected floor (spec 5.2.1).
      let available_fee_budget = native_balance
        .checked_sub(&T::MinUserBalance::get())
        .unwrap_or_default();
      if available_fee_budget < Self::cycle_fee_upper_bound(instance) {
        return Some(CloseReason::FeeBudgetExhausted);
      }
    }
    None
  }

  fn close_admission_decision(
    instance: &AaaInstanceOf<T>,
    reason: CloseReason,
    meter: &WeightMeter,
  ) -> AdmissionDecision {
    let weight = Self::close_cycle_weight_upper_bound(instance);
    if !meter.can_consume(weight) {
      return AdmissionDecision::Defer(Self::deferred_dimension(meter, weight));
    }
    AdmissionDecision::Close { reason, weight }
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

  fn deferred_dimension(meter: &WeightMeter, required: Weight) -> DeferReason {
    let after = meter.consumed().saturating_add(required);
    let limit = meter.limit();
    match (
      after.ref_time() > limit.ref_time(),
      after.proof_size() > limit.proof_size(),
    ) {
      (true, true) => DeferReason::Both,
      (true, false) => DeferReason::RefTime,
      (false, true) => DeferReason::ProofSize,
      (false, false) => DeferReason::RefTime,
    }
  }

  fn try_emit_cycle_deferred(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    reason: DeferReason,
    meter: &mut WeightMeter,
  ) -> bool {
    let event_weight = Self::deferred_event_weight_upper();
    if !meter.can_consume(event_weight) {
      return false;
    }
    let Some((candidate_cycle_nonce, candidate_attempt, cursor)) =
      Self::deferral_candidate(aaa_id, instance)
    else {
      return false;
    };
    Self::deposit_event(Event::CycleDeferred {
      aaa_id,
      candidate_cycle_nonce,
      candidate_attempt,
      cursor,
      reason,
    });
    meter.consume(event_weight);
    true
  }

  fn deferral_candidate(aaa_id: AaaId, instance: &AaaInstanceOf<T>) -> Option<(u64, u32, u32)> {
    if instance.run_state == RunState::Suspended {
      let continuation = ContinuationStateStore::<T>::get(aaa_id)?;
      return Some((
        instance.cycle_nonce,
        continuation.attempt.checked_add(1)?,
        continuation.cursor,
      ));
    }
    Some((instance.cycle_nonce.checked_add(1)?, 0, 0))
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
      return Self::close_admission_decision(instance, CloseReason::WindowExpired, meter);
    }
    if instance.lifecycle.is_paused() {
      return AdmissionDecision::Skip;
    }
    if instance.run_state == RunState::Idle && instance.cycle_nonce == u64::MAX {
      return Self::close_admission_decision(instance, CloseReason::CycleNonceExhausted, meter);
    }
    if let Some(reason) = Self::pending_post_cycle_close_reason(instance) {
      return Self::close_admission_decision(instance, reason, meter);
    }
    if !Self::is_ready_for_execution(aaa_id, instance) {
      return AdmissionDecision::Skip;
    }
    if let Some(reason) = Self::user_resource_close_reason(instance, false) {
      return Self::close_admission_decision(instance, reason, meter);
    }
    let continuation = if instance.run_state == RunState::Suspended {
      let Some(continuation) = ContinuationStateStore::<T>::get(aaa_id) else {
        return AdmissionDecision::Skip;
      };
      Some(continuation)
    } else {
      None
    };
    let start_cursor = continuation
      .as_ref()
      .map_or(0, |state| state.cursor as usize);
    if instance.actor_class.aaa_type() == AaaType::User {
      // Normative available budget: `fee_native_balance - MinUserBalance`; a User
      // attempt must not be admitted when charging the complete (suffix) envelope can
      // cross MinUserBalance, even if the raw balance covers the envelope (spec 5.2.1).
      let native_balance =
        T::AssetOps::balance(&instance.sovereign_account, T::NativeAssetId::get());
      let available_fee_budget = native_balance
        .checked_sub(&T::MinUserBalance::get())
        .unwrap_or_default();
      if available_fee_budget < Self::attempt_fee_upper_bound(instance, start_cursor) {
        return Self::close_admission_decision(instance, CloseReason::FeeBudgetExhausted, meter);
      }
    }
    let cycle_weight_upper = Self::cycle_admission_weight_upper(
      instance,
      start_cursor,
      continuation
        .as_ref()
        .map(|state| state.unsuccessful_attempts_at_cursor),
    );
    if !meter.can_consume(cycle_weight_upper) {
      return AdmissionDecision::Defer(Self::deferred_dimension(meter, cycle_weight_upper));
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
      let balance = T::AssetOps::balance(&instance.sovereign_account, T::NativeAssetId::get());
      if balance < T::MinUserBalance::get() {
        return Err(SimulationError::BalanceUnavailable);
      }
      let available_fee_budget = balance
        .checked_sub(&T::MinUserBalance::get())
        .unwrap_or_default();
      if available_fee_budget < Self::attempt_fee_upper_bound(instance, start_cursor) {
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
      return Self::retry_eligible_at(aaa_id, instance).is_ok_and(|eligible_at| eligible_at <= now);
    }
    let include_timer = instance.schedule.trigger.cadence_blocks().is_some();
    if !Self::next_eligible_at(aaa_id, instance, now, include_timer)
      .is_ok_and(|eligible_at| eligible_at <= now)
    {
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
    Self::next_eligible_at(aaa_id, instance, now, true).is_ok_and(|eligible_at| eligible_at <= now)
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
      (SourceFilter::Whitelist(list), Some(who)) => list
        .iter() // deos-bypass: bounded-iter — MaxWhitelistSize source filter
        .any(|a| a == who),
      (SourceFilter::Whitelist(_), None) => false,
    }
  }

  fn asset_matches_filter(filter: &AssetFilterOf<T>, asset: T::AssetId) -> bool {
    match filter {
      AssetFilter::Any => true,
      AssetFilter::Whitelist(list) => list
        .iter() // deos-bypass: bounded-iter — MaxWhitelistSize asset filter
        .any(|id| *id == asset),
    }
  }

  pub fn notify_address_event(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    source: &T::AccountId,
  ) -> DispatchResult {
    let provenance = FundingProvenance::Signed;
    Self::notify_address_event_with_context(aaa_id, asset, amount, Some(source), Some(&provenance))
  }

  pub fn notify_internal_address_event(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    source: &T::AccountId,
  ) -> DispatchResult {
    let provenance = FundingProvenance::InternalProtocol;
    Self::notify_address_event_with_context(aaa_id, asset, amount, Some(source), Some(&provenance))
  }

  pub fn notify_xcm_address_event(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    source: &T::AccountId,
  ) -> DispatchResult {
    let provenance = FundingProvenance::Xcm;
    Self::notify_address_event_with_context(aaa_id, asset, amount, Some(source), Some(&provenance))
  }

  pub fn notify_address_event_without_source(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
  ) -> DispatchResult {
    Self::notify_address_event_with_context(aaa_id, asset, amount, None, None)
  }

  fn funding_event_authorized(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    funding: &ActorFundingStateOf<T>,
    source: Option<&T::AccountId>,
    provenance: Option<&FundingProvenance>,
  ) -> bool {
    match &funding.funding_source_policy {
      FundingSourcePolicy::OwnerOnly => {
        provenance == Some(&FundingProvenance::Signed) && source == Some(&instance.owner)
      }
      FundingSourcePolicy::SignedAllowlist(allowed) => {
        provenance == Some(&FundingProvenance::Signed)
          && source.is_some_and(|source| allowed.contains(source))
      }
      FundingSourcePolicy::RuntimePolicy => {
        T::FundingAuthority::permits(aaa_id, &instance.owner, source, provenance)
      }
      FundingSourcePolicy::AnyVerifiedIngress => source.is_some() || provenance.is_some(),
    }
  }

  pub fn preflight_funding_event(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    source: Option<&T::AccountId>,
    provenance: Option<&FundingProvenance>,
  ) -> DispatchResult {
    let Some(instance) = Self::active_actor_snapshot(aaa_id) else {
      return Ok(());
    };
    if Self::is_window_expired(&instance) || amount.is_zero() {
      return Ok(());
    }
    let funding = ActorFunding::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
    if !Self::funding_event_authorized(aaa_id, &instance, &funding, source, provenance)
      || !funding.funding_tracked_assets.contains(&asset)
    {
      return Ok(());
    }
    if let Some(accumulated) = funding.funding_accumulated.get(&asset) {
      ensure!(
        accumulated.checked_add(&amount).is_some(),
        Error::<T>::FundingAccumulatorOverflow
      );
    }
    Ok(())
  }

  fn notify_address_event_with_context(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    source: Option<&T::AccountId>,
    provenance: Option<&FundingProvenance>,
  ) -> DispatchResult {
    Self::preflight_funding_event(aaa_id, asset, amount, source, provenance)?;
    polkadot_sdk::frame_support::storage::with_transaction(
      || match Self::apply_address_event_parts(
        aaa_id, asset, amount, source, provenance, true, true,
      ) {
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
    source: Option<&T::AccountId>,
    provenance: Option<&FundingProvenance>,
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
      for trigger_source in sources {
        // deos-bypass: bounded-iter — MaxTriggerSources bounds full source observation.
        if let TriggerSource::OnAddressEvent {
          source_filter,
          asset_filter,
        } = trigger_source
        {
          signal_matched |= Self::source_matches_filter(source_filter, &instance.owner, source)
            && Self::asset_matches_filter(asset_filter, asset);
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
      if Self::funding_event_authorized(aaa_id, &instance, &funding, source, provenance)
        && funding.funding_tracked_assets.contains(&asset)
      {
        let accumulated = if let Some(accumulated) = funding.funding_accumulated.get_mut(&asset) {
          *accumulated = accumulated
            .checked_add(&amount)
            .ok_or(Error::<T>::FundingAccumulatorOverflow)?;
          *accumulated
        } else {
          funding
            .funding_accumulated
            .try_insert(asset, amount)
            .map_err(|_| Error::<T>::FundingAccumulatorOverflow)?;
          amount
        };
        ActorFunding::<T>::insert(aaa_id, funding);
        Self::deposit_event(Event::FundingAccumulated {
          aaa_id,
          asset,
          added: amount,
          accumulated,
        });
      }
    }
    if signal_matched {
      // Queue capacity exhaustion preserves readiness through an exact later
      // wakeup (spec 8.1.4); monotonic ticket/page namespace exhaustion and wakeup
      // placement failure are not retryable queue-full and fail closed, rolling
      // back the producer movement in the same transaction.
      Self::enqueue_outcome_error(Self::enqueue(aaa_id))?;
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
