use super::pallet::*;
use super::{AddressEvent, AssetOps, FundingAuthority, IngressFailure, weights::WeightInfo};
use alloc::{vec, vec::Vec};
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

pub(crate) struct QueueAppendPlan<T: Config> {
  actors: Vec<(ActorId, ActorHotStateOf<T>)>,
  pages: Vec<(QueuePageId, QueuePageOf<T>)>,
  next_ticket: QueueTicket,
  next_tail: QueueTicket,
  next_occupancy: u32,
}

enum AdmissionDecision {
  Admit {
    weight: Weight,
    terminal_cleanup_reserved: bool,
  },
  Close {
    reason: CloseReason,
    weight: Weight,
  },
  Defer,
  Skip,
  Invariant,
}

/// Closed outcome of one canonical FIFO placement attempt. Queue capacity
/// exhaustion may preserve readiness through an exact later wakeup; monotonic
/// ticket/page namespace exhaustion and corruption are not retryable and fail
/// closed through the public error surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnqueueOutcome {
  AlreadyLive,
  CapacityUnavailable,
  TicketExhausted,
  SchedulerIndexExhausted,
  WakeupCapacityExhausted,
  WakeupIndexExhausted,
  CorruptedTopology,
}

/// Semantic result of admitting one trigger activation through the canonical
/// pending latch and FIFO/wakeup substrate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActivationOutcome {
  IgnoredStale,
  Coalesced,
  Latched,
  Closed,
}

/// Typed activation failure. Temporary pressure preserves the producer's
/// retryable work; permanent corruption fails the enclosing transition closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActivationFailure {
  Temporary(DispatchError),
  Permanent(DispatchError),
}

impl From<DispatchError> for ActivationFailure {
  fn from(error: DispatchError) -> Self {
    Self::Permanent(error)
  }
}

pub(crate) enum PrimeSchedulePlan<BlockNumber> {
  None,
  Enqueue,
  BlockWakeup(BlockNumber),
}

pub(crate) enum ActivationAction<T: Config> {
  CloseWindowExpired,
  CoalesceLive,
  EnqueueCadenced(Result<QueueAppendPlan<T>, EnqueueOutcome>),
  PrimeSchedule(Result<PrimeSchedulePlan<BlockNumberFor<T>>, EnqueueOutcome>),
}

pub(crate) struct ActivationPlan<T: Config> {
  pub actor_id: ActorId,
  pub already_pending: bool,
  pub prospective_hot: ActorHotStateOf<T>,
  pub instance: ActiveActorViewOf<T>,
  pub terminal_reason: Option<CloseReason>,
  pub action: ActivationAction<T>,
}

const MAX_RETRY_BACKOFF_BLOCKS: u32 = 8;

#[cfg(test)]
std::thread_local! {
  static CORRUPT_QUEUE_BEFORE_CLOSE_CONSUME: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
  static FAIL_WAKEUP_PLACEMENT_WITH_CAPACITY: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
  static QUEUE_APPEND_COMMITS: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
  static CROSSING_CURSOR_COMMITS: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
  static FIRST_CROSSING_BRANCH_WEIGHT: core::cell::Cell<Option<Weight>> = const { core::cell::Cell::new(None) };
}

/// Why the actor pass stopped at a queue boundary. Only a weight block over live FIFO work with
/// no admitted attempt drives `IdleStarvationState`; every other reason clears it once (spec 8.6).
#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockKind {
  Weight,
  FeeCollection,
  NonWeight,
}

#[derive(Clone, Copy)]
enum AttemptTransactionError {
  FeeCollection,
  Invariant,
}

impl From<polkadot_sdk::sp_runtime::DispatchError> for AttemptTransactionError {
  fn from(_: polkadot_sdk::sp_runtime::DispatchError) -> Self {
    Self::Invariant
  }
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
  WeightStall,
  PassExhausted,
  InvariantStall,
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
        HeadDiscovery::WeightStall | HeadDiscovery::InvariantStall => {
          starved = executed == 0;
          break;
        }
        HeadDiscovery::PassExhausted => break,
        HeadDiscovery::Head(position, entry) => {
          match Self::service_live_queue_entry((position, entry), now, &mut cycle_meter) {
            FifoStepResult::Progress {
              executed: did_execute,
            } => executed = executed.saturating_add(u32::from(did_execute)),
            FifoStepResult::NoWork => continue,
            FifoStepResult::Blocked(_kind) => {
              starved = executed == 0;
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
      return HeadDiscovery::InvariantStall;
    }
    if QueueHead::<T>::get() >= QueueTail::<T>::get() {
      return HeadDiscovery::Empty;
    }
    match Self::paged_head_entry() {
      Some((_, entry)) if entry.ticket >= cutoff => HeadDiscovery::Empty,
      _ => HeadDiscovery::InvariantStall,
    }
  }

  /// Conservatively reports physical pre-cutoff work when the complete loaded-state probe cannot
  /// be admitted. This path performs no unmetered actor-partition reads; the next funded pass
  /// decides whether the entry is live, stale, or corrupt.
  fn head_blocked_by_weight(cutoff: QueueTicket) -> bool {
    QueueHead::<T>::get() < QueueTail::<T>::get()
      && Self::paged_head_entry().is_some_and(|(_, entry)| entry.ticket < cutoff)
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
        return HeadDiscovery::InvariantStall;
      }
      if !cycle_meter.can_consume(scan_weight) {
        return if Self::head_blocked_by_weight(cutoff) {
          HeadDiscovery::WeightStall
        } else {
          HeadDiscovery::Empty
        };
      }
      cycle_meter.consume(scan_weight);
      let before = QueueHead::<T>::get();
      let stats = match Self::paged_drain_tombstones(cutoff, 1) {
        Ok(stats) => stats,
        Err(_) => return HeadDiscovery::InvariantStall,
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
    HeadDiscovery::PassExhausted
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
      HeadDiscovery::WeightStall => (2, None, scanned),
      HeadDiscovery::InvariantStall => (3, None, scanned),
      HeadDiscovery::PassExhausted => (4, None, scanned),
    }
  }

  fn service_live_queue_entry(
    (position, entry): (QueueTicket, QueueEntry),
    now: BlockNumberFor<T>,
    cycle_meter: &mut WeightMeter,
  ) -> FifoStepResult {
    let consume_weight = T::WeightInfo::scheduler_paged_consume_preserve_page()
      .max(T::WeightInfo::scheduler_paged_consume_delete_page());
    let state_probe_weight = Self::scheduler_actor_state_probe_weight_upper();
    if !cycle_meter.can_consume(state_probe_weight.saturating_add(consume_weight)) {
      return FifoStepResult::Blocked(BlockKind::Weight);
    }
    let LoadedActorStateOf::Active(state) = Self::load_actor_state(entry.actor_id) else {
      cycle_meter.consume(state_probe_weight);
      return FifoStepResult::Blocked(BlockKind::NonWeight);
    };
    cycle_meter.consume(state_probe_weight);
    if state.hot.queue_ticket != Some(entry.ticket) {
      return FifoStepResult::NoWork;
    }
    if state.hot.cycle_state == CycleState::Suspended {
      if state
        .continuation
        .as_ref()
        .is_some_and(|continuation| continuation.last_attempt_block == now)
      {
        return FifoStepResult::Blocked(BlockKind::NonWeight);
      }
    } else if state.identity.cycle_nonce > 0 && state.hot.last_cycle_block == Some(now) {
      return FifoStepResult::Blocked(BlockKind::NonWeight);
    }
    let queue_owner_hot = state.hot.clone();
    if state.hot.lifecycle.is_paused()
      && state
        .hot
        .terminal_at
        .is_none_or(|terminal_at| terminal_at > now)
    {
      let outcome: DispatchResult = polkadot_sdk::frame_support::storage::with_transaction(|| {
        if Self::paged_consume_loaded_head_at(
          position,
          entry.actor_id,
          entry.ticket,
          queue_owner_hot,
        )
        .is_err()
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            polkadot_sdk::sp_runtime::DispatchError::Other("paused queue consume failed"),
          ));
        }
        ActorHot::<T>::mutate(entry.actor_id, |maybe| {
          if let Some(current) = maybe {
            current.queue_ticket = None;
          }
        });
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      });
      if outcome.is_err() {
        return FifoStepResult::Blocked(BlockKind::NonWeight);
      }
      cycle_meter.consume(consume_weight);
      return FifoStepResult::Progress { executed: false };
    }
    let actor_id = entry.actor_id;
    let loaded_continuation = state.continuation;
    let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
    match Self::apply_admission_loaded(&instance, loaded_continuation.as_ref(), cycle_meter) {
      AdmissionDecision::Admit {
        weight,
        terminal_cleanup_reserved,
      } => {
        let attempt_weight = consume_weight.saturating_add(weight);
        let exhaustion_close_weight = if terminal_cleanup_reserved {
          Weight::zero()
        } else {
          Self::close_cleanup_weight_upper()
        };
        if !cycle_meter.can_consume(attempt_weight.saturating_add(exhaustion_close_weight)) {
          return FifoStepResult::Blocked(BlockKind::Weight);
        }
        let outcome = polkadot_sdk::frame_support::storage::with_transaction(|| {
          if Self::paged_consume_loaded_head_at(
            position,
            entry.actor_id,
            entry.ticket,
            queue_owner_hot,
          )
          .is_err()
          {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
              AttemptTransactionError::Invariant,
            ));
          }
          let (_actual, fee_collection_failed) =
            Self::execute_single_cycle(actor_id, instance, now);
          if fee_collection_failed {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
              AttemptTransactionError::FeeCollection,
            ));
          }
          match Self::loaded_active_view_for_placement(actor_id) {
            Ok(Some(updated)) => {
              if let Err(error) = Self::schedule_next_work(actor_id, &updated, now, true) {
                if !Self::scheduler_index_is_exhausted(error)
                  || Self::close_for_scheduler_index_exhaustion(actor_id).is_err()
                {
                  return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                    AttemptTransactionError::Invariant,
                  ));
                }
                return polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(true));
              }
            }
            Ok(None) => {}
            Err(_) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                AttemptTransactionError::Invariant,
              ));
            }
          }
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(false))
        });
        cycle_meter.consume(attempt_weight);
        match outcome {
          Ok(closed_for_exhaustion) => {
            if closed_for_exhaustion {
              cycle_meter.consume(exhaustion_close_weight);
            }
            FifoStepResult::Progress { executed: true }
          }
          Err(AttemptTransactionError::FeeCollection) => {
            FifoStepResult::Blocked(BlockKind::FeeCollection)
          }
          Err(AttemptTransactionError::Invariant) => FifoStepResult::Blocked(BlockKind::NonWeight),
        }
      }
      AdmissionDecision::Close { reason, weight } => {
        let atomic_weight = consume_weight.saturating_add(weight);
        if !cycle_meter.can_consume(atomic_weight) {
          return FifoStepResult::Blocked(BlockKind::Weight);
        }
        let outcome = polkadot_sdk::frame_support::storage::with_transaction(|| {
          if let Err(error) = Self::finalize_actor(actor_id, &instance, reason) {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
          }
          Self::apply_test_close_queue_corruption();
          if Self::paged_consume_closed_head_at(position).is_err() {
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
      AdmissionDecision::Defer => FifoStepResult::Blocked(BlockKind::Weight),
      AdmissionDecision::Invariant => FifoStepResult::Blocked(BlockKind::NonWeight),
      AdmissionDecision::Skip => {
        let exhaustion_close_weight = Self::close_cleanup_weight_upper();
        if !cycle_meter.can_consume(consume_weight.saturating_add(exhaustion_close_weight)) {
          return FifoStepResult::Blocked(BlockKind::Weight);
        }
        let outcome = polkadot_sdk::frame_support::storage::with_transaction(|| {
          if Self::paged_consume_loaded_head_at(position, actor_id, entry.ticket, queue_owner_hot)
            .is_err()
          {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
              polkadot_sdk::sp_runtime::DispatchError::Other("scheduler queue topology changed"),
            ));
          }
          match Self::loaded_active_view_for_placement(actor_id) {
            Ok(Some(updated)) => {
              if let Err(error) = Self::schedule_next_work(actor_id, &updated, now, true) {
                if !Self::scheduler_index_is_exhausted(error)
                  || Self::close_for_scheduler_index_exhaustion(actor_id).is_err()
                {
                  return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                    polkadot_sdk::sp_runtime::DispatchError::Other("post-skip placement failed"),
                  ));
                }
                return polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(true));
              }
            }
            Ok(None) => {}
            Err(_) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                polkadot_sdk::sp_runtime::DispatchError::Other("post-skip actor state corrupt"),
              ));
            }
          }
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(false))
        });
        cycle_meter.consume(consume_weight);
        match outcome {
          Ok(closed_for_exhaustion) => {
            if closed_for_exhaustion {
              cycle_meter.consume(exhaustion_close_weight);
            }
            FifoStepResult::Progress { executed: false }
          }
          Err(_) => FifoStepResult::Blocked(BlockKind::NonWeight),
        }
      }
    }
  }

  pub(crate) fn enqueue(actor_id: ActorId) -> Result<(), EnqueueOutcome> {
    match Self::try_paged_enqueue(actor_id) {
      Ok(()) => Ok(()),
      Err(EnqueueOutcome::AlreadyLive) => Ok(()),
      Err(EnqueueOutcome::CapacityUnavailable) => {
        // Queue saturation preserves readiness through an exact next-block wakeup
        // (spec 8.1.4). A failure to place that wakeup must fail closed rather than
        // silently leave the actor with neither a live ticket nor a wakeup.
        let next_block = frame_system::Pallet::<T>::block_number()
          .checked_add(&One::one())
          .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
        Self::defer_wakeup(actor_id, next_block)
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
  pub fn paged_enqueue(actor_id: ActorId) -> bool {
    matches!(
      Self::try_paged_enqueue(actor_id),
      Ok(()) | Err(EnqueueOutcome::AlreadyLive)
    )
  }

  pub(crate) fn preflight_paged_enqueue_cohort(
    actors: Vec<(ActorId, ActorHotStateOf<T>)>,
  ) -> Result<QueueAppendPlan<T>, EnqueueOutcome> {
    if actors.is_empty() || actors.len() > T::MaxCrossingActorsPerBlock::get() as usize {
      return Err(EnqueueOutcome::CapacityUnavailable);
    }
    let topology = Self::queue_topology_preflight(QueueMutation::Enqueue)?;
    let mut plan = QueueAppendPlan {
      actors: Vec::new(),
      pages: Vec::new(),
      next_ticket: NextQueueTicket::<T>::get(),
      next_tail: topology.tail,
      next_occupancy: topology.occupancy,
    };
    for (actor_id, hot) in actors.into_iter(/* deos-bypass: bounded-iter */) {
      Self::reserve_following_paged_enqueue(&mut plan, actor_id, hot)?;
    }
    Ok(plan)
  }

  fn preflight_paged_enqueue_loaded(
    actor_id: ActorId,
    hot: ActorHotStateOf<T>,
  ) -> Result<QueueAppendPlan<T>, EnqueueOutcome> {
    Self::preflight_paged_enqueue_cohort(vec![(actor_id, hot)])
  }

  pub(crate) fn reserve_following_paged_enqueue(
    plan: &mut QueueAppendPlan<T>,
    actor_id: ActorId,
    mut hot: ActorHotStateOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    if plan.actors.len() >= T::MaxCrossingActorsPerBlock::get() as usize {
      return Err(EnqueueOutcome::CapacityUnavailable);
    }
    if hot.queue_ticket.is_some()
      || plan
        .actors
        .iter(/* deos-bypass: bounded-iter */)
        .any(|(planned_actor, _)| *planned_actor == actor_id)
    {
      return Err(EnqueueOutcome::AlreadyLive);
    }
    if plan.next_occupancy >= T::MaxQueueLength::get() {
      return Err(EnqueueOutcome::CapacityUnavailable);
    }
    let ticket = plan.next_ticket;
    let next_ticket = ticket
      .checked_add(1)
      .ok_or(EnqueueOutcome::TicketExhausted)?;
    let position = plan.next_tail;
    let next_tail = position
      .checked_add(1)
      .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
    let next_occupancy = plan
      .next_occupancy
      .checked_add(1)
      .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
    let (page_id, slot) = Self::queue_page_and_slot(position);
    let page = if let Some((_, page)) = plan
      .pages
      .iter_mut(/* deos-bypass: bounded-iter */)
      .find(|(planned_page_id, _)| *planned_page_id == page_id)
    {
      page
    } else {
      plan
        .pages
        .push((page_id, QueuePages::<T>::get(page_id).unwrap_or_default()));
      let Some((_, page)) = plan.pages.last_mut() else {
        return Err(EnqueueOutcome::CorruptedTopology);
      };
      page
    };
    if page.len() != slot || page.try_push(QueueEntry { ticket, actor_id }).is_err() {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    hot.queue_ticket = Some(ticket);
    plan.actors.push((actor_id, hot));
    plan.next_ticket = next_ticket;
    plan.next_tail = next_tail;
    plan.next_occupancy = next_occupancy;
    Ok(())
  }

  fn preflight_paged_enqueue(actor_id: ActorId) -> Result<QueueAppendPlan<T>, EnqueueOutcome> {
    let hot = match Self::load_actor_state(actor_id) {
      LoadedActorStateOf::Active(state) => state.hot,
      LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => {
        return Err(EnqueueOutcome::CapacityUnavailable);
      }
      LoadedActorStateOf::Corrupt => return Err(EnqueueOutcome::CorruptedTopology),
    };
    Self::preflight_paged_enqueue_loaded(actor_id, hot)
  }

  #[cfg(test)]
  pub(crate) fn test_preflight_queue_pair(
    first: ActorId,
    second: ActorId,
  ) -> Result<[QueueTicket; 2], EnqueueOutcome> {
    let first_hot = match Self::load_actor_state(first) {
      LoadedActorStateOf::Active(state) => state.hot,
      _ => return Err(EnqueueOutcome::CorruptedTopology),
    };
    let second_hot = match Self::load_actor_state(second) {
      LoadedActorStateOf::Active(state) => state.hot,
      _ => return Err(EnqueueOutcome::CorruptedTopology),
    };
    let plan =
      Self::preflight_paged_enqueue_cohort(vec![(first, first_hot), (second, second_hot)])?;
    let first_ticket = plan.actors[0]
      .1
      .queue_ticket
      .ok_or(EnqueueOutcome::CorruptedTopology)?;
    let second_ticket = plan.actors[1]
      .1
      .queue_ticket
      .ok_or(EnqueueOutcome::CorruptedTopology)?;
    Ok([first_ticket, second_ticket])
  }

  #[cfg(test)]
  pub(crate) fn test_preflight_queue_quartet(
    actors: [ActorId; 4],
  ) -> Result<[QueueTicket; 4], EnqueueOutcome> {
    let first_hot = match Self::load_actor_state(actors[0]) {
      LoadedActorStateOf::Active(state) => state.hot,
      _ => return Err(EnqueueOutcome::CorruptedTopology),
    };
    let mut cohort = vec![(actors[0], first_hot)];
    for actor_id in actors.iter(/* deos-bypass: bounded-iter */).skip(1) {
      let hot = match Self::load_actor_state(*actor_id) {
        LoadedActorStateOf::Active(state) => state.hot,
        _ => return Err(EnqueueOutcome::CorruptedTopology),
      };
      cohort.push((*actor_id, hot));
    }
    let plan = Self::preflight_paged_enqueue_cohort(cohort)?;
    let mut tickets = [0; 4];
    for (index, (_, hot)) in plan
      .actors
      .iter(/* deos-bypass: bounded-iter */)
      .enumerate()
    {
      tickets[index] = hot.queue_ticket.ok_or(EnqueueOutcome::CorruptedTopology)?;
    }
    Ok(tickets)
  }

  #[cfg(test)]
  pub(crate) fn test_commit_queue_quartet(actors: [ActorId; 4]) -> Result<(), EnqueueOutcome> {
    let mut cohort = Vec::new();
    for actor_id in actors {
      let mut hot = match Self::load_actor_state(actor_id) {
        LoadedActorStateOf::Active(state) => state.hot,
        _ => return Err(EnqueueOutcome::CorruptedTopology),
      };
      hot.pending_signal = true;
      cohort.push((actor_id, hot));
    }
    let plan = Self::preflight_paged_enqueue_cohort(cohort)?;
    Self::commit_paged_enqueue(plan);
    Ok(())
  }

  #[cfg(test)]
  pub(crate) fn test_preflight_queue_over_cap(actors: Vec<ActorId>) -> Result<(), EnqueueOutcome> {
    let mut cohort = Vec::new();
    for actor_id in actors {
      let hot = match Self::load_actor_state(actor_id) {
        LoadedActorStateOf::Active(state) => state.hot,
        _ => return Err(EnqueueOutcome::CorruptedTopology),
      };
      cohort.push((actor_id, hot));
    }
    Self::preflight_paged_enqueue_cohort(cohort).map(|_| ())
  }

  #[cfg(test)]
  pub(crate) fn test_reset_crossing_cursor_commits() {
    CROSSING_CURSOR_COMMITS.with(|count| count.set(0));
  }

  #[cfg(test)]
  pub(crate) fn test_crossing_cursor_commits() -> u32 {
    CROSSING_CURSOR_COMMITS.with(core::cell::Cell::get)
  }

  #[cfg(test)]
  pub(crate) fn test_record_crossing_cursor_commit() {
    CROSSING_CURSOR_COMMITS.with(|count| count.set(count.get().saturating_add(1)));
  }

  #[cfg(test)]
  pub(crate) fn test_reset_first_crossing_branch_weight() {
    FIRST_CROSSING_BRANCH_WEIGHT.with(|weight| weight.set(None));
  }

  #[cfg(test)]
  pub(crate) fn test_first_crossing_branch_weight() -> Option<Weight> {
    FIRST_CROSSING_BRANCH_WEIGHT.with(core::cell::Cell::get)
  }

  #[cfg(test)]
  pub(crate) fn test_record_first_crossing_branch_weight(weight: Weight) {
    FIRST_CROSSING_BRANCH_WEIGHT.with(|recorded| {
      if recorded.get().is_none() {
        recorded.set(Some(weight));
      }
    });
  }

  #[cfg(test)]
  pub(crate) fn test_reset_queue_append_commits() {
    QUEUE_APPEND_COMMITS.with(|count| count.set(0));
  }

  #[cfg(test)]
  pub(crate) fn test_queue_append_commits() -> u32 {
    QUEUE_APPEND_COMMITS.with(core::cell::Cell::get)
  }

  pub(crate) fn commit_paged_enqueue(plan: QueueAppendPlan<T>) {
    #[cfg(test)]
    QUEUE_APPEND_COMMITS.with(|count| count.set(count.get().saturating_add(1)));
    for (page_id, page) in plan.pages {
      QueuePages::<T>::insert(page_id, page);
    }
    QueueTail::<T>::put(plan.next_tail);
    QueueOccupancy::<T>::put(plan.next_occupancy);
    NextQueueTicket::<T>::put(plan.next_ticket);
    for (actor_id, hot) in plan.actors {
      ActorHot::<T>::insert(actor_id, hot);
    }
  }

  pub fn try_paged_enqueue(actor_id: ActorId) -> Result<(), EnqueueOutcome> {
    with_transaction_opaque_err(|| match Self::preflight_paged_enqueue(actor_id) {
      Ok(plan) => {
        Self::commit_paged_enqueue(plan);
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      }
      Err(error) => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error)),
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  fn scheduler_index_is_exhausted(outcome: EnqueueOutcome) -> bool {
    matches!(
      outcome,
      EnqueueOutcome::TicketExhausted
        | EnqueueOutcome::SchedulerIndexExhausted
        | EnqueueOutcome::WakeupIndexExhausted
    )
  }

  fn close_for_scheduler_index_exhaustion(actor_id: ActorId) -> DispatchResult {
    let instance = Self::active_actor_view(actor_id).ok_or(Error::<T>::ActorInvariant)?;
    Self::finalize_actor(actor_id, &instance, CloseReason::SchedulerIndexExhausted)
  }

  pub(crate) fn request_activation(
    actor_id: ActorId,
  ) -> Result<ActivationOutcome, ActivationFailure> {
    if polkadot_sdk::frame_support::storage::transactional::is_transactional() {
      return Self::request_activation_inner(actor_id);
    }
    polkadot_sdk::frame_support::storage::with_transaction(|| match Self::request_activation_inner(
      actor_id,
    ) {
      Ok(outcome) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(outcome)),
      Err(error) => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error)),
    })
  }

  pub(crate) fn preflight_activation_loaded(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
  ) -> Result<ActivationPlan<T>, ActivationFailure> {
    let already_pending = state.hot.pending_signal;
    let continuation = state.continuation;
    let mut hot = state.hot;
    hot.pending_signal = true;
    let instance = Self::derive_active_actor_view(state.identity, hot.clone(), state.contract);
    let classification =
      Self::classify_actor_loaded(&instance, continuation.as_ref()).map_err(|error| {
        ActivationFailure::Permanent(Self::classification_dispatch_error(error).into())
      })?;
    let action = if classification.terminal_reason == Some(CloseReason::WindowExpired) {
      ActivationAction::CloseWindowExpired
    } else if instance.queue_ticket.is_some() {
      ActivationAction::CoalesceLive
    } else if matches!(instance.trigger, Trigger::Cadenced { .. }) {
      ActivationAction::EnqueueCadenced(Self::preflight_paged_enqueue_loaded(actor_id, hot.clone()))
    } else {
      ActivationAction::PrimeSchedule(Self::preflight_prime_schedule_loaded(
        &instance,
        continuation.as_ref(),
      ))
    };
    Ok(ActivationPlan {
      actor_id,
      already_pending,
      prospective_hot: hot,
      instance,
      terminal_reason: classification.terminal_reason,
      action,
    })
  }

  #[cfg(test)]
  pub(crate) fn test_activation_plan_kind(actor_id: ActorId) -> Result<u8, ActivationFailure> {
    let LoadedActorStateOf::Active(state) = Self::load_actor_state(actor_id) else {
      return Err(ActivationFailure::Permanent(
        Error::<T>::ActorInvariant.into(),
      ));
    };
    let plan = Self::preflight_activation_loaded(actor_id, state)?;
    Ok(match plan.action {
      ActivationAction::CloseWindowExpired => 0,
      ActivationAction::CoalesceLive => 1,
      ActivationAction::EnqueueCadenced(_) => 2,
      ActivationAction::PrimeSchedule(Ok(PrimeSchedulePlan::None)) => 3,
      ActivationAction::PrimeSchedule(Ok(PrimeSchedulePlan::Enqueue)) => 4,
      ActivationAction::PrimeSchedule(Ok(PrimeSchedulePlan::BlockWakeup(_))) => 5,
      ActivationAction::PrimeSchedule(Err(_)) => 6,
    })
  }

  pub(crate) fn commit_activation_plan(
    plan: ActivationPlan<T>,
  ) -> Result<ActivationOutcome, ActivationFailure> {
    let ActivationPlan {
      actor_id,
      already_pending,
      prospective_hot,
      instance,
      terminal_reason: _,
      action,
    } = plan;
    if !already_pending {
      ActorHot::<T>::insert(actor_id, prospective_hot);
    }
    match action {
      ActivationAction::CloseWindowExpired => {
        Self::finalize_actor(actor_id, &instance, CloseReason::WindowExpired)
          .map_err(ActivationFailure::Permanent)?;
        return Ok(ActivationOutcome::Closed);
      }
      ActivationAction::CoalesceLive => {
        return Ok(if already_pending {
          ActivationOutcome::Coalesced
        } else {
          ActivationOutcome::Latched
        });
      }
      ActivationAction::EnqueueCadenced(_) | ActivationAction::PrimeSchedule(_) => {}
    }

    let placement = match action {
      ActivationAction::EnqueueCadenced(Ok(queue_plan)) => {
        Self::commit_paged_enqueue(queue_plan);
        Ok(())
      }
      ActivationAction::EnqueueCadenced(Err(EnqueueOutcome::CapacityUnavailable)) => {
        Self::enqueue(actor_id)
      }
      ActivationAction::EnqueueCadenced(Err(error)) => Err(error),
      ActivationAction::PrimeSchedule(Ok(PrimeSchedulePlan::None)) => Ok(()),
      ActivationAction::PrimeSchedule(Ok(PrimeSchedulePlan::Enqueue)) => Self::enqueue(actor_id),
      ActivationAction::PrimeSchedule(Ok(PrimeSchedulePlan::BlockWakeup(block))) => {
        Self::defer_wakeup(actor_id, block)
      }
      ActivationAction::PrimeSchedule(Err(error)) => Err(error),
      ActivationAction::CloseWindowExpired | ActivationAction::CoalesceLive => {
        return Err(ActivationFailure::Permanent(
          Error::<T>::ActorInvariant.into(),
        ));
      }
    };
    match placement {
      Ok(()) | Err(EnqueueOutcome::AlreadyLive) => Ok(if already_pending {
        ActivationOutcome::Coalesced
      } else {
        ActivationOutcome::Latched
      }),
      Err(EnqueueOutcome::CapacityUnavailable | EnqueueOutcome::WakeupCapacityExhausted) => Err(
        ActivationFailure::Temporary(Error::<T>::QueueCapacityUnavailable.into()),
      ),
      Err(
        outcome @ (EnqueueOutcome::TicketExhausted
        | EnqueueOutcome::SchedulerIndexExhausted
        | EnqueueOutcome::WakeupIndexExhausted),
      ) => {
        let _ = outcome;
        Self::close_for_scheduler_index_exhaustion(actor_id)
          .map_err(ActivationFailure::Permanent)?;
        Ok(ActivationOutcome::Closed)
      }
      Err(EnqueueOutcome::CorruptedTopology) => Err(ActivationFailure::Permanent(
        Error::<T>::SchedulerIndexExhausted.into(),
      )),
    }
  }

  fn request_activation_inner(actor_id: ActorId) -> Result<ActivationOutcome, ActivationFailure> {
    let state = match Self::load_actor_state(actor_id) {
      LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => {
        return Ok(ActivationOutcome::IgnoredStale);
      }
      LoadedActorStateOf::Active(state) => state,
      LoadedActorStateOf::Corrupt => {
        return Err(ActivationFailure::Permanent(
          Error::<T>::ActorInvariant.into(),
        ));
      }
    };
    let plan = Self::preflight_activation_loaded(actor_id, state)?;
    Self::commit_activation_plan(plan)
  }

  pub(crate) fn activation_failure_error(error: ActivationFailure) -> DispatchError {
    match error {
      ActivationFailure::Temporary(error) | ActivationFailure::Permanent(error) => error,
    }
  }

  /// Maps a placement result to the public error surface for extrinsic boundaries.
  pub fn enqueue_outcome_error(outcome: Result<(), EnqueueOutcome>) -> Result<(), DispatchError> {
    match outcome {
      Ok(()) => Ok(()),
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
      // Placement owners normally normalize AlreadyLive to success before `map_err`.
      // A missed normalization fails closed instead of panicking in consensus execution.
      Ok(()) => Error::<T>::QueueCapacityUnavailable.into(),
      Err(error) => error,
    }
  }

  fn wakeup_page_physical_live(page: &WakeupPageOf<T>) -> usize {
    let mut live = 0usize;
    for entry in page.entries.as_slice() {
      if entry.is_some() {
        live = live.saturating_add(1);
      }
    }
    live
  }

  pub fn paged_invalidate(actor_id: ActorId) -> Option<QueueTicket> {
    Self::try_paged_invalidate(actor_id).ok().flatten()
  }

  pub(crate) fn try_paged_invalidate(
    actor_id: ActorId,
  ) -> Result<Option<QueueTicket>, EnqueueOutcome> {
    match Self::load_actor_state(actor_id) {
      LoadedActorStateOf::Active(mut state) => {
        let ticket = state.hot.queue_ticket.take();
        ActorHot::<T>::insert(actor_id, state.hot);
        Ok(ticket)
      }
      LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => Ok(None),
      LoadedActorStateOf::Corrupt => Err(EnqueueOutcome::CorruptedTopology),
    }
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
    Self::paged_consume_head_at_inner(position, false, None)
  }

  fn paged_consume_loaded_head_at(
    position: QueueTicket,
    actor_id: ActorId,
    ticket: QueueTicket,
    hot: ActorHotStateOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    Self::paged_consume_head_at_inner(position, false, Some((actor_id, ticket, hot)))
  }

  fn paged_consume_closed_head_at(position: QueueTicket) -> Result<(), EnqueueOutcome> {
    Self::paged_consume_head_at_inner(position, true, None)
  }

  fn paged_consume_head_at_inner(
    position: QueueTicket,
    owner_was_closed: bool,
    loaded_owner: Option<(ActorId, QueueTicket, ActorHotStateOf<T>)>,
  ) -> Result<(), EnqueueOutcome> {
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
        let actor_hot = if let Some((actor_id, ticket, hot)) = loaded_owner {
          if owner_was_closed
            || entry.actor_id != actor_id
            || entry.ticket != ticket
            || hot.queue_ticket != Some(ticket)
          {
            return Err(EnqueueOutcome::CorruptedTopology);
          }
          Some(hot)
        } else {
          match Self::load_actor_state(entry.actor_id) {
            LoadedActorStateOf::Active(state) if !owner_was_closed => {
              if state.hot.queue_ticket != Some(entry.ticket) {
                return Err(EnqueueOutcome::CorruptedTopology);
              }
              Some(state.hot)
            }
            LoadedActorStateOf::NotRegistered if owner_was_closed => None,
            _ => return Err(EnqueueOutcome::CorruptedTopology),
          }
        };
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
        if let Some(mut hot) = actor_hot {
          hot.queue_ticket = None;
          ActorHot::<T>::insert(entry.actor_id, hot);
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
          match Self::load_actor_state(entry.actor_id) {
            LoadedActorStateOf::Active(state) if state.hot.queue_ticket == Some(entry.ticket) => {
              break 'pages;
            }
            LoadedActorStateOf::Active(_)
            | LoadedActorStateOf::NotRegistered
            | LoadedActorStateOf::Dormant(_) => {}
            LoadedActorStateOf::Corrupt => return corrupt(),
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
    actor_id: ActorId,
  ) -> bool {
    WakeupPages::<T>::get((pointer.block, pointer.page_id))
      .and_then(|page| page.entries.get(pointer.slot as usize).copied().flatten())
      .is_some_and(|entry| entry.actor_id == actor_id)
  }

  pub(crate) fn wakeup_substrate_invalidate_inner(
    actor_id: ActorId,
  ) -> Result<Option<WakeupPointer<BlockNumberFor<T>>>, EnqueueOutcome> {
    let LoadedActorStateOf::Active(mut state) = Self::load_actor_state(actor_id) else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    let Some(pointer) = state.hot.wakeup_pointer else {
      return Ok(None);
    };
    let key = (pointer.block, pointer.page_id);
    let Some(mut page) = WakeupPages::<T>::get(key) else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    let Some(slot) = page.entries.get(pointer.slot as usize) else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    if !slot.is_some_and(|entry| entry.actor_id == actor_id) {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    let physical_live = Self::wakeup_page_physical_live(&page);
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
    if Self::wakeup_cursor_get(pointer.block.clock(), cursor_index) != Some(pointer.block) {
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
    state.hot.wakeup_pointer = None;
    ActorHot::<T>::insert(actor_id, state.hot);
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

  pub fn wakeup_substrate_invalidate(
    actor_id: ActorId,
  ) -> Option<WakeupPointer<BlockNumberFor<T>>> {
    let result: Result<WakeupPointer<BlockNumberFor<T>>, DispatchError> =
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Self::wakeup_substrate_invalidate_inner(actor_id) {
          Ok(Some(pointer)) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(pointer))
          }
          Ok(None) | Err(_) => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(
            Err(Error::<T>::ActorNotFound.into()),
          ),
        }
      });
    result.ok()
  }

  fn wakeup_substrate_schedule_inner(
    actor_id: ActorId,
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
  ) -> bool {
    matches!(
      Self::try_wakeup_substrate_schedule_key_inner(actor_id, wakeup_key),
      Ok(()) | Err(EnqueueOutcome::AlreadyLive)
    )
  }

  #[cfg(test)]
  pub(crate) fn try_wakeup_substrate_schedule_inner(
    actor_id: ActorId,
    wakeup_block: BlockNumberFor<T>,
  ) -> Result<(), EnqueueOutcome> {
    Self::try_wakeup_substrate_schedule_key_inner(actor_id, WakeupKey::Block(wakeup_block))
  }

  fn try_wakeup_substrate_schedule_key_inner(
    actor_id: ActorId,
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
  ) -> Result<(), EnqueueOutcome> {
    with_transaction_opaque_err(|| {
      match Self::try_wakeup_substrate_schedule_transition(actor_id, wakeup_key) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  fn try_wakeup_substrate_schedule_transition(
    actor_id: ActorId,
    wakeup_block: WakeupKey<BlockNumberFor<T>>,
  ) -> Result<(), EnqueueOutcome> {
    let LoadedActorStateOf::Active(state) = Self::load_actor_state(actor_id) else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    if let Some(pointer) = state.hot.wakeup_pointer {
      if pointer.block == wakeup_block && Self::wakeup_page_entry_matches(pointer, actor_id) {
        return Err(EnqueueOutcome::AlreadyLive);
      }
      Self::wakeup_substrate_invalidate_inner(actor_id)?;
    }

    let (page_id, slot) = if let Some(mut bucket) = WakeupBuckets::<T>::get(wakeup_block) {
      let Some(cursor_index) = bucket.cursor_index else {
        return Err(EnqueueOutcome::CorruptedTopology);
      };
      if Self::wakeup_cursor_get(wakeup_block.clock(), cursor_index) != Some(wakeup_block) {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
      let tail_key = (wakeup_block, bucket.tail_page);
      let Some(mut tail_page) = WakeupPages::<T>::get(tail_key) else {
        return Err(EnqueueOutcome::CorruptedTopology);
      };
      if tail_page.next_page.is_some()
        || Self::wakeup_page_physical_live(&tail_page) != tail_page.live_entries as usize
      {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
      let Some(next_bucket_live) = bucket.live_entries.checked_add(1) else {
        return Err(EnqueueOutcome::WakeupIndexExhausted);
      };
      let mut reusable_slot = None;
      for slot in tail_page.scan_slot as usize..tail_page.entries.len() {
        if tail_page.entries[slot].is_none() {
          reusable_slot = Some(slot);
          break;
        }
      }
      let slot = if let Some(slot) = reusable_slot {
        tail_page.entries[slot] = Some(WakeupEntry { actor_id });
        slot
      } else if tail_page.entries.len() < T::WakeupPageSize::get() as usize {
        let slot = tail_page.entries.len();
        if tail_page
          .entries
          .try_push(Some(WakeupEntry { actor_id }))
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
        if entries.try_push(Some(WakeupEntry { actor_id })).is_err() {
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
        Self::set_wakeup_pointer(actor_id, wakeup_block, page_id, 0)?;
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
      if entries.try_push(Some(WakeupEntry { actor_id })).is_err() {
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
    Self::set_wakeup_pointer(actor_id, wakeup_block, page_id, slot)?;
    Ok(())
  }

  fn set_wakeup_pointer(
    actor_id: ActorId,
    block: WakeupKey<BlockNumberFor<T>>,
    page_id: WakeupPageId,
    slot: WakeupSlot,
  ) -> Result<(), EnqueueOutcome> {
    let pointer = WakeupPointer {
      block,
      page_id,
      slot,
    };
    let LoadedActorStateOf::Active(mut state) = Self::load_actor_state(actor_id) else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    if state.hot.wakeup_pointer.is_some() {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    state.hot.wakeup_pointer = Some(pointer);
    ActorHot::<T>::insert(actor_id, state.hot);
    Ok(())
  }

  pub fn wakeup_substrate_schedule(actor_id: ActorId, wakeup_block: BlockNumberFor<T>) -> bool {
    let result: DispatchResult = polkadot_sdk::frame_support::storage::with_transaction(|| {
      if Self::wakeup_substrate_schedule_inner(actor_id, WakeupKey::Block(wakeup_block)) {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      } else {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          Error::<T>::ActorNotFound.into(),
        ))
      }
    });
    result.is_ok()
  }

  fn wakeup_substrate_drain_block_inner(
    wakeup_block: WakeupKey<BlockNumberFor<T>>,
    max_entries_scanned: u32,
  ) -> Option<(BoundedVec<ActorId, T::MaxWakeupsPerBlock>, WakeupDrainStats)> {
    let mut ready = BoundedVec::<ActorId, T::MaxWakeupsPerBlock>::default();
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
    if Self::wakeup_cursor_get(wakeup_block.clock(), cursor_index) != Some(wakeup_block) {
      return None;
    }
    let mut page_id = bucket.head_page;

    while stats.entries_scanned < scan_limit {
      let key = (wakeup_block, page_id);
      let Some(mut page) = WakeupPages::<T>::get(key) else {
        return None;
      };
      if Self::wakeup_page_physical_live(&page) != page.live_entries as usize {
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
        let mut actor_hot = match Self::load_actor_state(entry.actor_id) {
          LoadedActorStateOf::Active(state) if state.hot.wakeup_pointer == Some(pointer) => {
            state.hot
          }
          LoadedActorStateOf::Active(state) if state.hot.wakeup_pointer.is_none() => {
            stats.stale_entries = stats.stale_entries.saturating_add(1);
            continue;
          }
          LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => {
            stats.stale_entries = stats.stale_entries.saturating_add(1);
            continue;
          }
          LoadedActorStateOf::Active(_) | LoadedActorStateOf::Corrupt => return None,
        };
        if ready.try_push(entry.actor_id).is_err() {
          page.entries[pointer_slot] = Some(entry);
          page.live_entries = page.live_entries.checked_add(1)?;
          bucket.live_entries = bucket.live_entries.checked_add(1)?;
          page.scan_slot = pointer_slot as WakeupSlot;
          stats.entries_scanned = stats.entries_scanned.saturating_sub(1);
          WakeupPages::<T>::insert(key, page);
          WakeupBuckets::<T>::insert(wakeup_block, bucket);
          return Some((ready, stats));
        }
        actor_hot.wakeup_pointer = None;
        ActorHot::<T>::insert(entry.actor_id, actor_hot);
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

  fn wakeup_substrate_drain_key(
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
    max_entries_scanned: u32,
  ) -> (BoundedVec<ActorId, T::MaxWakeupsPerBlock>, WakeupDrainStats) {
    let result: Result<_, DispatchError> =
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Self::wakeup_substrate_drain_block_inner(wakeup_key, max_entries_scanned) {
          Some(result) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(result))
          }
          None => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorNotFound.into(),
          )),
        }
      });
    result.unwrap_or_default()
  }

  pub fn wakeup_substrate_drain_block(
    wakeup_block: BlockNumberFor<T>,
    max_entries_scanned: u32,
  ) -> (BoundedVec<ActorId, T::MaxWakeupsPerBlock>, WakeupDrainStats) {
    Self::wakeup_substrate_drain_key(WakeupKey::Block(wakeup_block), max_entries_scanned)
  }

  fn wakeup_cursor_page_and_slot(index: WakeupCursorIndex) -> (WakeupPageId, usize) {
    let page_size = T::WakeupPageSize::get().max(1);
    (u64::from(index / page_size), (index % page_size) as usize)
  }

  pub(crate) fn wakeup_cursor_get(
    clock: WakeupClock,
    index: WakeupCursorIndex,
  ) -> Option<WakeupKey<BlockNumberFor<T>>> {
    let (page_id, slot) = Self::wakeup_cursor_page_and_slot(index);
    WakeupCursorPages::<T>::get((clock, page_id)).and_then(|page| page.get(slot).copied())
  }

  fn wakeup_cursor_set(
    clock: WakeupClock,
    index: WakeupCursorIndex,
    block: WakeupKey<BlockNumberFor<T>>,
  ) -> bool {
    if block.clock() != clock {
      return false;
    }
    let (page_id, slot) = Self::wakeup_cursor_page_and_slot(index);
    let mut page = WakeupCursorPages::<T>::get((clock, page_id)).unwrap_or_default();
    if slot < page.len() {
      page[slot] = block;
    } else if slot == page.len() {
      if page.try_push(block).is_err() {
        return false;
      }
    } else {
      return false;
    }
    WakeupCursorPages::<T>::insert((clock, page_id), page);
    true
  }

  fn wakeup_cursor_remove_tail(clock: WakeupClock, index: WakeupCursorIndex) -> bool {
    let (page_id, slot) = Self::wakeup_cursor_page_and_slot(index);
    let Some(mut page) = WakeupCursorPages::<T>::get((clock, page_id)) else {
      return false;
    };
    if slot.checked_add(1) != Some(page.len()) {
      return false;
    }
    page.pop();
    if page.is_empty() {
      WakeupCursorPages::<T>::remove((clock, page_id));
    } else {
      WakeupCursorPages::<T>::insert((clock, page_id), page);
    }
    true
  }

  fn wakeup_cursor_swap(
    clock: WakeupClock,
    left: WakeupCursorIndex,
    right: WakeupCursorIndex,
  ) -> bool {
    let Some(left_block) = Self::wakeup_cursor_get(clock, left) else {
      return false;
    };
    let Some(right_block) = Self::wakeup_cursor_get(clock, right) else {
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
    if !Self::wakeup_cursor_set(clock, left, right_block)
      || !Self::wakeup_cursor_set(clock, right, left_block)
    {
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

  fn wakeup_cursor_insert_inner(block: WakeupKey<BlockNumberFor<T>>) -> bool {
    let clock = block.clock();
    let Some(mut bucket) = WakeupBuckets::<T>::get(block) else {
      return false;
    };
    if let Some(index) = bucket.cursor_index {
      return Self::wakeup_cursor_get(clock, index) == Some(block);
    }
    let len = WakeupCursorLen::<T>::get(clock);
    let Some(next_len) = len.checked_add(1) else {
      return false;
    };
    if len >= T::MaxActiveActors::get() || !Self::wakeup_cursor_set(clock, len, block) {
      return false;
    }
    bucket.cursor_index = Some(len);
    WakeupBuckets::<T>::insert(block, bucket);
    WakeupCursorLen::<T>::insert(clock, next_len);
    let mut current = len;
    for _ in 0..Self::wakeup_cursor_height_bound() {
      if current == 0 {
        break;
      }
      let parent = current.saturating_sub(1) / 2;
      let Some(parent_block) = Self::wakeup_cursor_get(clock, parent) else {
        return false;
      };
      let Some(current_block) = Self::wakeup_cursor_get(clock, current) else {
        return false;
      };
      if parent_block <= current_block {
        break;
      }
      if !Self::wakeup_cursor_swap(clock, parent, current) {
        return false;
      }
      current = parent;
    }
    true
  }

  pub fn wakeup_cursor_insert(block: BlockNumberFor<T>) -> bool {
    let result: DispatchResult = polkadot_sdk::frame_support::storage::with_transaction(|| {
      if Self::wakeup_cursor_insert_inner(WakeupKey::Block(block)) {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      } else {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          Error::<T>::ActorNotFound.into(),
        ))
      }
    });
    result.is_ok()
  }

  pub(crate) fn wakeup_cursor_peek_key(clock: WakeupClock) -> Option<WakeupKey<BlockNumberFor<T>>> {
    (WakeupCursorLen::<T>::get(clock) > 0)
      .then(|| Self::wakeup_cursor_get(clock, 0))
      .flatten()
  }

  pub fn wakeup_cursor_peek() -> Option<BlockNumberFor<T>> {
    match Self::wakeup_cursor_peek_key(WakeupClock::Block)? {
      WakeupKey::Block(block) => Some(block),
      WakeupKey::Tick(_) => None,
    }
  }

  fn wakeup_cursor_remove_inner(block: WakeupKey<BlockNumberFor<T>>) -> bool {
    let clock = block.clock();
    let Some(mut target_bucket) = WakeupBuckets::<T>::get(block) else {
      return false;
    };
    let Some(index) = target_bucket.cursor_index else {
      return false;
    };
    let len = WakeupCursorLen::<T>::get(clock);
    if index >= len || Self::wakeup_cursor_get(clock, index) != Some(block) {
      return false;
    }
    let Some(last_index) = len.checked_sub(1) else {
      return false;
    };
    let Some(last_block) = Self::wakeup_cursor_get(clock, last_index) else {
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
    if last_bucket.cursor_index != Some(last_index)
      || !Self::wakeup_cursor_remove_tail(clock, last_index)
    {
      return false;
    }
    target_bucket.cursor_index = None;
    WakeupBuckets::<T>::insert(block, target_bucket);
    WakeupCursorLen::<T>::insert(clock, last_index);
    if index == last_index {
      return true;
    }
    if !Self::wakeup_cursor_set(clock, index, last_block) {
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
      let Some(parent_block) = Self::wakeup_cursor_get(clock, parent) else {
        return false;
      };
      let Some(current_block) = Self::wakeup_cursor_get(clock, current) else {
        return false;
      };
      if parent_block <= current_block {
        break;
      }
      if !Self::wakeup_cursor_swap(clock, parent, current) {
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
      let Some(left_block) = Self::wakeup_cursor_get(clock, left) else {
        return false;
      };
      if right < last_index {
        let Some(right_block) = Self::wakeup_cursor_get(clock, right) else {
          return false;
        };
        if right_block < left_block {
          smallest = right;
        }
      }
      let Some(current_block) = Self::wakeup_cursor_get(clock, current) else {
        return false;
      };
      let Some(smallest_block) = Self::wakeup_cursor_get(clock, smallest) else {
        return false;
      };
      if current_block <= smallest_block {
        break;
      }
      if !Self::wakeup_cursor_swap(clock, current, smallest) {
        return false;
      }
      current = smallest;
    }
    true
  }

  pub fn wakeup_cursor_remove(block: BlockNumberFor<T>) -> bool {
    let result: DispatchResult = polkadot_sdk::frame_support::storage::with_transaction(|| {
      if Self::wakeup_cursor_remove_inner(WakeupKey::Block(block)) {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      } else {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          Error::<T>::ActorNotFound.into(),
        ))
      }
    });
    result.is_ok()
  }

  fn wakeup_cursor_pop_min_inner(clock: WakeupClock) -> Option<WakeupKey<BlockNumberFor<T>>> {
    let min_block = Self::wakeup_cursor_get(clock, 0)?;
    Self::wakeup_cursor_remove_inner(min_block).then_some(min_block)
  }

  pub fn wakeup_cursor_pop_min() -> Option<BlockNumberFor<T>> {
    let result: Result<BlockNumberFor<T>, DispatchError> =
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Self::wakeup_cursor_pop_min_inner(WakeupClock::Block) {
          Some(WakeupKey::Block(block)) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(block))
          }
          Some(WakeupKey::Tick(_)) | None => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
              Error::<T>::ActorNotFound.into(),
            ))
          }
        }
      });
    result.ok()
  }

  fn loaded_active_view_for_placement(
    actor_id: ActorId,
  ) -> Result<Option<ActiveActorViewOf<T>>, EnqueueOutcome> {
    match Self::load_actor_state(actor_id) {
      LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => Ok(None),
      LoadedActorStateOf::Active(state) => Ok(Some(Self::derive_active_actor_view(
        state.identity,
        state.hot,
        state.contract,
      ))),
      LoadedActorStateOf::Corrupt => Err(EnqueueOutcome::CorruptedTopology),
    }
  }

  pub(crate) fn prime_actor_schedule(actor_id: ActorId) -> Result<(), EnqueueOutcome> {
    let Some(instance) = Self::loaded_active_view_for_placement(actor_id)? else {
      return Ok(());
    };
    Self::prime_actor_schedule_loaded(actor_id, &instance)
  }

  fn preflight_prime_schedule_loaded(
    instance: &ActiveActorViewOf<T>,
    continuation: Option<&ContinuationStateOf<T>>,
  ) -> Result<PrimeSchedulePlan<BlockNumberFor<T>>, EnqueueOutcome> {
    if instance.lifecycle.is_paused() {
      return Ok(
        Self::window_expiry_wakeup(instance)
          .map_or(PrimeSchedulePlan::None, PrimeSchedulePlan::BlockWakeup),
      );
    }
    let now = frame_system::Pallet::<T>::block_number();
    let eligible_at = if instance.cycle_state == CycleState::Suspended {
      Self::retry_eligible_at_loaded(
        instance,
        continuation.ok_or(EnqueueOutcome::CorruptedTopology)?,
      )?
    } else if instance.pending_signal {
      Self::next_eligible_at(instance, now)?
    } else {
      return Ok(
        Self::window_expiry_wakeup(instance)
          .map_or(PrimeSchedulePlan::None, PrimeSchedulePlan::BlockWakeup),
      );
    };
    let wakeup_at = instance.window.map_or(eligible_at, |window| {
      eligible_at.min(Self::window_terminal_at(&window))
    });
    let exact_next_block = now
      .checked_add(&One::one())
      .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
    Ok(if wakeup_at < exact_next_block {
      PrimeSchedulePlan::Enqueue
    } else {
      PrimeSchedulePlan::BlockWakeup(wakeup_at)
    })
  }

  fn prime_actor_schedule_loaded(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    let now = frame_system::Pallet::<T>::block_number();
    if instance.lifecycle.is_paused() {
      return Self::schedule_window_expiry(actor_id, &instance);
    }
    if matches!(instance.trigger, Trigger::Cadenced { .. })
      && instance.cadence_anchor_tick.is_none()
    {
      return Self::defer_tick_wakeup(actor_id, 0);
    }
    Self::schedule_next_work(actor_id, &instance, now, false)
  }

  fn window_expiry_wakeup(instance: &ActiveActorViewOf<T>) -> Option<BlockNumberFor<T>> {
    instance
      .window
      .map(|window| Self::window_terminal_at(&window))
  }

  fn schedule_window_expiry(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    if let Some(expiry) = Self::window_expiry_wakeup(instance) {
      Self::defer_wakeup(actor_id, expiry)
    } else {
      Ok(())
    }
  }

  #[cfg(test)]
  pub(crate) fn test_fail_wakeup_placement_with_capacity() {
    FAIL_WAKEUP_PLACEMENT_WITH_CAPACITY.with(|flag| flag.set(true));
  }

  fn defer_tick_wakeup(
    actor_id: ActorId,
    wakeup_tick: SchedulerTick,
  ) -> Result<(), EnqueueOutcome> {
    #[cfg(test)]
    if FAIL_WAKEUP_PLACEMENT_WITH_CAPACITY.with(|flag| flag.replace(false)) {
      return Err(EnqueueOutcome::WakeupCapacityExhausted);
    }
    match Self::try_wakeup_substrate_schedule_key_inner(actor_id, WakeupKey::Tick(wakeup_tick)) {
      Ok(()) | Err(EnqueueOutcome::AlreadyLive) => Ok(()),
      Err(other) => Err(other),
    }
  }

  fn defer_wakeup(
    actor_id: ActorId,
    wakeup_block: BlockNumberFor<T>,
  ) -> Result<(), EnqueueOutcome> {
    #[cfg(test)]
    if FAIL_WAKEUP_PLACEMENT_WITH_CAPACITY.with(|flag| flag.replace(false)) {
      return Err(EnqueueOutcome::WakeupCapacityExhausted);
    }
    let target = Self::loaded_active_view_for_placement(actor_id)?
      .and_then(|instance| Self::window_expiry_wakeup(&instance))
      .map(|expiry| wakeup_block.min(expiry))
      .unwrap_or(wakeup_block);
    match Self::try_wakeup_substrate_schedule_key_inner(actor_id, WakeupKey::Block(target)) {
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
      .saturating_add(T::WeightInfo::scheduler_wakeup_cursor_worker_future().saturating_mul(2))
      .saturating_add(Self::scheduler_actor_state_probe_weight_upper())
  }

  /// Conservatively prices pure actor-local terminal deletion from the measured User close path.
  /// Shared queue and wakeup records become lazy tombstones.
  pub fn close_cleanup_weight_upper() -> Weight {
    T::WeightInfo::close_actor()
  }

  pub fn wakeup_registration_weight_upper() -> Weight {
    T::WeightInfo::scheduler_wakeup_append_new_page()
      .saturating_add(T::WeightInfo::scheduler_wakeup_cursor_insert())
      .saturating_add(T::WeightInfo::scheduler_wakeup_cursor_remove_exact())
  }

  pub fn scheduler_actor_probe_weight_upper() -> Weight {
    Self::scheduler_actor_state_probe_weight_upper()
  }

  pub fn scheduler_actor_state_probe_weight_upper() -> Weight {
    T::WeightInfo::scheduler_actor_state_probe()
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_defer_tick_wakeup(
    actor_id: ActorId,
    wakeup_tick: SchedulerTick,
  ) -> Result<(), EnqueueOutcome> {
    Self::defer_tick_wakeup(actor_id, wakeup_tick)
  }

  fn wakeup_cursor_drain_branch_weight(removes_bucket: bool) -> Weight {
    if removes_bucket {
      T::WeightInfo::scheduler_wakeup_cursor_worker_remove()
    } else {
      T::WeightInfo::scheduler_wakeup_cursor_worker_partial()
    }
  }

  pub fn wakeup_cursor_drain_unit_weight_upper(removes_bucket: bool) -> Weight {
    Self::wakeup_cursor_drain_branch_weight(removes_bucket)
      .saturating_add(Self::close_cleanup_weight_upper())
  }

  pub fn drain_overdue_wakeups_cursor(
    now: BlockNumberFor<T>,
    meter: &mut WeightMeter,
  ) -> WakeupDrainStats {
    Self::drain_overdue_wakeups_cursor_resuming(now, meter, WakeupDrainStats::default())
  }

  pub(crate) fn drain_overdue_wakeups_cursor_resuming(
    now: BlockNumberFor<T>,
    meter: &mut WeightMeter,
    mut total: WakeupDrainStats,
  ) -> WakeupDrainStats {
    let max_scans = T::MaxWakeupsPerBlock::get();
    if total.entries_scanned >= max_scans {
      return total;
    }
    let now_tick = match Self::current_scheduler_tick() {
      Ok(tick) => tick,
      Err(_) => return total,
    };
    while total.entries_scanned < max_scans {
      let first_clock = NextWakeupClock::<T>::get();
      let clocks = match first_clock {
        WakeupClock::Block => [WakeupClock::Block, WakeupClock::Tick],
        WakeupClock::Tick => [WakeupClock::Tick, WakeupClock::Block],
      };
      let mut selected = None;
      for clock in clocks {
        let cursor_weight = T::WeightInfo::scheduler_wakeup_cursor_worker_future();
        if !meter.can_consume(cursor_weight) {
          break;
        }
        meter.consume(cursor_weight);
        if WakeupWorkerFaultState::<T>::exists() {
          return total;
        }
        let Some(key) = Self::wakeup_cursor_peek_key(clock) else {
          continue;
        };
        let due = match key {
          WakeupKey::Block(block) => block <= now,
          WakeupKey::Tick(tick) => tick <= now_tick,
        };
        if due {
          selected = Some((
            key,
            match clock {
              WakeupClock::Block => WakeupClock::Tick,
              WakeupClock::Tick => WakeupClock::Block,
            },
          ));
          break;
        }
      }
      let Some((wakeup_key, next_clock_after_success)) = selected else {
        break;
      };
      let base_weight = Self::wakeup_cursor_drain_branch_weight(false);
      if Self::combined_queue_occupancy() >= u64::from(T::MaxActiveActors::get())
        || !meter.can_consume(base_weight)
      {
        break;
      }
      let Some(bucket) = WakeupBuckets::<T>::get(wakeup_key) else {
        meter.consume(base_weight);
        break;
      };
      let removes_bucket = bucket.live_entries <= 1;
      let unit_weight = Self::wakeup_cursor_drain_branch_weight(removes_bucket);
      let admission_weight = Self::wakeup_cursor_drain_unit_weight_upper(removes_bucket);
      if !meter.can_consume(admission_weight) {
        meter.consume(base_weight);
        break;
      }
      meter.consume(unit_weight);
      let outcome = polkadot_sdk::frame_support::storage::with_transaction(|| {
        let (ready, stats) = Self::wakeup_substrate_drain_key(wakeup_key, 1);
        if stats.entries_scanned == 0 {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok((
            stats, false,
          )));
        }
        let mut closed_for_exhaustion = false;
        for actor_id in ready {
          if matches!(wakeup_key, WakeupKey::Tick(_)) {
            let LoadedActorStateOf::Active(mut state) = Self::load_actor_state(actor_id) else {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                polkadot_sdk::sp_runtime::DispatchError::Other("tick wakeup owner is corrupt"),
              ));
            };
            let Trigger::Cadenced { every_ticks } = &state.contract.trigger else {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                polkadot_sdk::sp_runtime::DispatchError::Other("tick wakeup owner is not cadenced"),
              ));
            };
            if state
              .hot
              .trigger_runtime_state
              .cadence_anchor_tick()
              .is_none()
            {
              let Ok(Some(anchor_tick)) = Self::cadence_anchor_tick(&state.contract.trigger) else {
                return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                  polkadot_sdk::sp_runtime::DispatchError::Other("genesis cadence anchor failed"),
                ));
              };
              let Some(due_tick) = next_cadence_due_tick(anchor_tick, *every_ticks, now_tick)
              else {
                return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                  polkadot_sdk::sp_runtime::DispatchError::Other("genesis cadence deadline failed"),
                ));
              };
              state.hot.trigger_runtime_state = TriggerRuntimeState::Cadenced {
                anchor_tick: Some(anchor_tick),
              };
              ActorHot::<T>::insert(actor_id, state.hot);
              if let Err(error) = Self::defer_tick_wakeup(actor_id, due_tick) {
                if !Self::scheduler_index_is_exhausted(error)
                  || Self::close_for_scheduler_index_exhaustion(actor_id).is_err()
                {
                  return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                    polkadot_sdk::sp_runtime::DispatchError::Other("genesis cadence rearm failed"),
                  ));
                }
                closed_for_exhaustion = true;
              }
              continue;
            }
            match Self::request_activation(actor_id) {
              Ok(ActivationOutcome::IgnoredStale) => {
                return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                  polkadot_sdk::sp_runtime::DispatchError::Other("cadence owner is stale"),
                ));
              }
              Ok(ActivationOutcome::Closed) => closed_for_exhaustion = true,
              Ok(ActivationOutcome::Coalesced | ActivationOutcome::Latched) => {}
              Err(_) => {
                return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                  polkadot_sdk::sp_runtime::DispatchError::Other("cadence activation failed"),
                ));
              }
            }
            continue;
          }
          if let Err(error) = Self::enqueue(actor_id) {
            if !Self::scheduler_index_is_exhausted(error)
              || Self::close_for_scheduler_index_exhaustion(actor_id).is_err()
            {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                polkadot_sdk::sp_runtime::DispatchError::Other("wakeup materialization failed"),
              ));
            }
            closed_for_exhaustion = true;
          }
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok((
          stats,
          closed_for_exhaustion,
        )))
      });
      let (stats, closed_for_exhaustion) = match outcome {
        Ok(outcome) => outcome,
        Err(_) => {
          WakeupWorkerFaultState::<T>::put(WakeupWorkerFault {
            key: wakeup_key,
            page: bucket.head_page,
            class: CrossingWorkerFaultClass::Invariant,
          });
          break;
        }
      };
      if closed_for_exhaustion {
        meter.consume(Self::close_cleanup_weight_upper());
      }
      if stats.entries_scanned == 0 {
        break;
      }
      NextWakeupClock::<T>::put(next_clock_after_success);
      total.entries_scanned = total.entries_scanned.saturating_add(stats.entries_scanned);
      total.ready_entries = total.ready_entries.saturating_add(stats.ready_entries);
      total.stale_entries = total.stale_entries.saturating_add(stats.stale_entries);
      total.pages_touched = total.pages_touched.saturating_add(stats.pages_touched);
      total.pages_deleted = total.pages_deleted.saturating_add(stats.pages_deleted);
    }
    total
  }

  pub(crate) fn current_scheduler_tick() -> Result<SchedulerTick, EnqueueOutcome> {
    scheduler_tick_floor(
      <T::Time as polkadot_sdk::frame_support::traits::Time>::now(),
      T::CadenceTickMillis::get(),
    )
    .ok_or(EnqueueOutcome::SchedulerIndexExhausted)
  }

  pub(crate) fn cadence_anchor_tick(
    trigger: &TriggerOf<T>,
  ) -> Result<Option<SchedulerTick>, EnqueueOutcome> {
    if !matches!(trigger, Trigger::Cadenced { .. }) {
      return Ok(None);
    }
    scheduler_tick_ceil(
      <T::Time as polkadot_sdk::frame_support::traits::Time>::now(),
      T::CadenceTickMillis::get(),
    )
    .map(Some)
    .ok_or(EnqueueOutcome::SchedulerIndexExhausted)
  }

  /// The Active-epoch block anchor. Set to the current block (clamped to window
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
    instance: &ActiveActorViewOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<BlockNumberFor<T>, EnqueueOutcome> {
    let cooldown_anchor = instance
      .last_cycle_block
      .unwrap_or(instance.schedule_anchor);
    let cooldown_eligible_at = if instance.cycle_nonce == 0 && instance.last_cycle_block.is_none() {
      instance.schedule_anchor
    } else {
      cooldown_anchor
        .checked_add(&instance.cooldown_blocks.into())
        .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?
    };
    let window_floor = instance
      .window
      .map(|window| window.start)
      .unwrap_or_else(Zero::zero);
    Ok(now.max(cooldown_eligible_at).max(window_floor))
  }

  pub(crate) fn retry_backoff_blocks(cursor_local_attempt: u32) -> u32 {
    1u32
      .checked_shl(cursor_local_attempt)
      .unwrap_or(MAX_RETRY_BACKOFF_BLOCKS)
      .min(MAX_RETRY_BACKOFF_BLOCKS)
  }

  pub(crate) fn retry_eligible_at(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
  ) -> Result<BlockNumberFor<T>, EnqueueOutcome> {
    let continuation =
      ContinuationStateStore::<T>::get(actor_id).ok_or(EnqueueOutcome::CorruptedTopology)?;
    Self::retry_eligible_at_loaded(instance, &continuation)
  }

  fn retry_eligible_at_loaded(
    instance: &ActiveActorViewOf<T>,
    continuation: &ContinuationStateOf<T>,
  ) -> Result<BlockNumberFor<T>, EnqueueOutcome> {
    let cooldown: BlockNumberFor<T> = instance.cooldown_blocks.into();
    let cursor_local_attempt = continuation
      .unsuccessful_attempts_at_cursor
      .saturating_sub(1);
    let backoff: BlockNumberFor<T> = Self::retry_backoff_blocks(cursor_local_attempt).into();
    let retry_delay = cooldown.max(backoff);
    let mut eligible_at = continuation
      .last_attempt_block
      .checked_add(&retry_delay)
      .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
    if let Some(window) = instance.window {
      eligible_at = eligible_at.max(window.start);
    }
    Ok(eligible_at)
  }

  fn schedule_next_work_local(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
    now: BlockNumberFor<T>,
    cutoff_snapshotted: bool,
    requeues: &mut Vec<ActorId>,
  ) -> Result<(), EnqueueOutcome> {
    if instance.lifecycle.is_paused() {
      return Self::schedule_window_expiry(actor_id, instance);
    }
    if instance.cycle_state != CycleState::Suspended
      && let Trigger::Cadenced { every_ticks } = instance.trigger
    {
      let anchor_tick = instance
        .cadence_anchor_tick
        .ok_or(EnqueueOutcome::CorruptedTopology)?;
      let due_tick =
        next_cadence_due_tick(anchor_tick, every_ticks, Self::current_scheduler_tick()?)
          .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
      return Self::defer_tick_wakeup(actor_id, due_tick);
    }
    let eligible_at = if instance.cycle_state == CycleState::Suspended {
      Self::retry_eligible_at(actor_id, instance)?
    } else if instance.pending_signal {
      Self::next_eligible_at(instance, now)?
    } else {
      return Self::schedule_window_expiry(actor_id, instance);
    };
    let wakeup_at = instance.window.map_or(eligible_at, |window| {
      eligible_at.min(Self::window_terminal_at(&window))
    });
    let exact_next_block = now
      .checked_add(&One::one())
      .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
    if wakeup_at < exact_next_block || wakeup_at == exact_next_block && cutoff_snapshotted {
      requeues.push(actor_id);
      Ok(())
    } else {
      Self::defer_wakeup(actor_id, wakeup_at)
    }
  }

  fn schedule_next_work(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
    now: BlockNumberFor<T>,
    cutoff_snapshotted: bool,
  ) -> Result<(), EnqueueOutcome> {
    let mut requeues = Vec::new();
    Self::schedule_next_work_local(actor_id, instance, now, cutoff_snapshotted, &mut requeues)?;
    for actor_id in requeues {
      Self::enqueue(actor_id)?;
    }
    Ok(())
  }

  pub(crate) fn is_window_expired(instance: &ActiveActorViewOf<T>) -> bool {
    let now = frame_system::Pallet::<T>::block_number();
    instance
      .window
      .map(|window| now > window.end)
      .unwrap_or(false)
  }

  pub(crate) fn user_native_balance(instance: &ActiveActorViewOf<T>) -> T::Balance {
    let native = T::FeeNativeAssetId::get();
    T::AssetOps::balance(&instance.sovereign_account, native)
  }

  pub(crate) fn classification_dispatch_error(error: ActorClassificationError) -> Error<T> {
    match error {
      ActorClassificationError::ActorInvariant => Error::<T>::ActorInvariant,
      ActorClassificationError::ContinuationInvariant => Error::<T>::ContinuationInvariant,
      ActorClassificationError::ComputationOverflow => Error::<T>::ComputationOverflow,
    }
  }

  pub(crate) fn expiry_substitution_due_loaded(
    instance: &ActiveActorViewOf<T>,
    continuation: Option<&ContinuationStateOf<T>>,
  ) -> Result<bool, Error<T>> {
    Self::classify_actor_loaded(instance, continuation)
      .map(|classification| classification.terminal_reason == Some(CloseReason::WindowExpired))
      .map_err(Self::classification_dispatch_error)
  }

  // Deterministic User viability precedence is BalanceExhausted, then
  // FeeBudgetExhausted. The caller supplies the current contract cursor.
  pub(crate) fn user_viability_close_reason(
    instance: &ActiveActorViewOf<T>,
    start_cursor: usize,
  ) -> Option<CloseReason> {
    if instance.actor_class.actor_type() != ActorType::User {
      return None;
    }
    let native_balance = Self::user_native_balance(instance);
    if native_balance < T::MinUserBalance::get() {
      return Some(CloseReason::BalanceExhausted);
    }
    // The complete suffix envelope must fit above MinUserBalance: charging it
    // must not cross the protected floor (spec 5.2.1).
    let available_fee_budget = native_balance
      .checked_sub(&T::MinUserBalance::get())
      .unwrap_or_default();
    if available_fee_budget < Self::attempt_fee_upper_bound(instance, start_cursor) {
      return Some(CloseReason::FeeBudgetExhausted);
    }
    None
  }

  #[cfg(test)]
  pub(crate) fn classify_actor(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
  ) -> Result<ActorClassification<BlockNumberFor<T>>, ActorClassificationError> {
    let continuation = ContinuationStateStore::<T>::get(actor_id);
    Self::classify_actor_loaded(instance, continuation.as_ref())
  }

  pub(crate) fn classify_actor_loaded(
    instance: &ActiveActorViewOf<T>,
    continuation: Option<&ContinuationStateOf<T>>,
  ) -> Result<ActorClassification<BlockNumberFor<T>>, ActorClassificationError> {
    if (instance.cycle_state == CycleState::Suspended) != continuation.is_some() {
      return Err(ActorClassificationError::ContinuationInvariant);
    }
    let cursor = continuation
      .as_ref()
      .map_or(0, |state| state.cursor as usize);
    if let Some(state) = continuation.as_ref() {
      if cursor >= instance.steps.len()
        || state.unsuccessful_attempts_at_cursor == 0
        || instance.steps[cursor]
          .on_error
          .retry_max_attempts()
          .is_none()
      {
        return Err(ActorClassificationError::ContinuationInvariant);
      }
    }

    let terminal_reason = if Self::is_window_expired(instance) {
      Some(CloseReason::WindowExpired)
    } else if let Some(reason) = Self::user_viability_close_reason(instance, cursor) {
      Some(reason)
    } else if instance.cycle_state == CycleState::Idle && instance.cycle_nonce == u64::MAX {
      Some(CloseReason::CycleNonceExhausted)
    } else if continuation.as_ref().is_some_and(|state| {
      instance.steps[state.cursor as usize]
        .on_error
        .retry_max_attempts()
        .is_some_and(|max_attempts| state.unsuccessful_attempts_at_cursor >= max_attempts)
    }) {
      Some(CloseReason::RetryAttemptsExhausted)
    } else if Self::failure_limit_reached(instance.unsuccessful_attempt_streak) {
      Some(CloseReason::ConsecutiveFailures)
    } else if instance.cycle_state == CycleState::Idle
      && instance
        .auto_close_at_cycle_nonce
        .is_some_and(|target| instance.cycle_nonce >= target)
    {
      Some(CloseReason::AutoCloseNonceReached)
    } else {
      None
    };

    let execution_phase = if GlobalCircuitBreaker::<T>::get() {
      ActorExecutionPhase::GlobalCircuitBreaker
    } else if instance.lifecycle.is_paused() {
      ActorExecutionPhase::Paused
    } else if terminal_reason.is_some() {
      ActorExecutionPhase::Ready
    } else if instance.cycle_state == CycleState::Suspended {
      let eligible_at = Self::retry_eligible_at_loaded(
        instance,
        continuation.ok_or(ActorClassificationError::ContinuationInvariant)?,
      )
      .map_err(|outcome| match outcome {
        EnqueueOutcome::SchedulerIndexExhausted => ActorClassificationError::ComputationOverflow,
        _ => ActorClassificationError::ContinuationInvariant,
      })?;
      let now = frame_system::Pallet::<T>::block_number();
      if eligible_at > now {
        ActorExecutionPhase::WaitingRetry(eligible_at)
      } else {
        ActorExecutionPhase::Ready
      }
    } else if matches!(instance.trigger, Trigger::Cadenced { .. }) {
      if instance.pending_signal {
        ActorExecutionPhase::Ready
      } else {
        let Some(WakeupPointer {
          block: WakeupKey::Tick(due_tick),
          ..
        }) = instance.wakeup_pointer
        else {
          return Err(ActorClassificationError::ActorInvariant);
        };
        ActorExecutionPhase::WaitingCadenceTick(due_tick)
      }
    } else {
      let now = frame_system::Pallet::<T>::block_number();
      let eligible_at = Self::next_eligible_at(instance, now)
        .map_err(|_| ActorClassificationError::ComputationOverflow)?;
      if eligible_at > now {
        ActorExecutionPhase::WaitingBlock(eligible_at)
      } else if !instance.pending_signal {
        ActorExecutionPhase::WaitingSignal
      } else {
        ActorExecutionPhase::Ready
      }
    };
    Ok(ActorClassification {
      terminal_reason,
      execution_phase,
    })
  }

  fn close_admission_decision(
    instance: &ActiveActorViewOf<T>,
    reason: CloseReason,
    meter: &WeightMeter,
  ) -> AdmissionDecision {
    let weight = Self::close_cycle_weight_upper_bound(instance);
    if !meter.can_consume(weight) {
      return AdmissionDecision::Defer;
    }
    AdmissionDecision::Close { reason, weight }
  }

  fn cycle_may_close_on_failure(
    instance: &ActiveActorViewOf<T>,
    start_cursor: usize,
    prior_unsuccessful_attempts_at_cursor: Option<u32>,
  ) -> bool {
    if Self::failure_limit_reached(instance.unsuccessful_attempt_streak.saturating_add(1)) {
      return true;
    }
    for index in start_cursor..instance.steps.len() {
      let step = &instance.steps[index];
      if step.on_error.retry_max_attempts().is_some_and(|limit| {
        let next_attempt = if index == start_cursor {
          prior_unsuccessful_attempts_at_cursor
            .unwrap_or_default()
            .saturating_add(1)
        } else {
          1
        };
        next_attempt >= limit
      }) {
        return true;
      }
    }
    false
  }

  fn cycle_may_auto_close_on_success(instance: &ActiveActorViewOf<T>) -> bool {
    instance
      .auto_close_at_cycle_nonce
      .map(|target| instance.cycle_nonce.saturating_add(1) >= target)
      .unwrap_or(false)
  }

  fn cycle_requires_terminal_cleanup_budget(
    instance: &ActiveActorViewOf<T>,
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
    instance: &ActiveActorViewOf<T>,
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

  fn apply_admission_loaded(
    instance: &ActiveActorViewOf<T>,
    continuation: Option<&ContinuationStateOf<T>>,
    meter: &WeightMeter,
  ) -> AdmissionDecision {
    let Ok(classification) = Self::classify_actor_loaded(instance, continuation) else {
      return AdmissionDecision::Invariant;
    };
    if classification.execution_phase == ActorExecutionPhase::GlobalCircuitBreaker {
      return AdmissionDecision::Skip;
    }
    if let Some(reason) = classification.terminal_reason {
      return Self::close_admission_decision(instance, reason, meter);
    }
    if classification.execution_phase != ActorExecutionPhase::Ready {
      return AdmissionDecision::Skip;
    }
    let continuation = if instance.cycle_state == CycleState::Suspended {
      let Some(continuation) = continuation else {
        return AdmissionDecision::Skip;
      };
      Some(continuation)
    } else {
      None
    };
    let start_cursor = continuation
      .as_ref()
      .map_or(0, |state| state.cursor as usize);
    let prior_unsuccessful_attempts_at_cursor = continuation
      .as_ref()
      .map(|state| state.unsuccessful_attempts_at_cursor);
    let terminal_cleanup_reserved = Self::cycle_requires_terminal_cleanup_budget(
      instance,
      start_cursor,
      prior_unsuccessful_attempts_at_cursor,
    );
    let cycle_weight_upper = Self::cycle_admission_weight_upper(
      instance,
      start_cursor,
      prior_unsuccessful_attempts_at_cursor,
    );
    if !meter.can_consume(cycle_weight_upper) {
      return AdmissionDecision::Defer;
    }
    AdmissionDecision::Admit {
      weight: cycle_weight_upper,
      terminal_cleanup_reserved,
    }
  }

  /// Projects the canonical actor classifier without stripping temporal payloads.
  pub fn actor_eligibility(
    actor_id: ActorId,
  ) -> Result<ActorEligibility<T::ObservationFeedId, BlockNumberFor<T>>, ActorClassificationError>
  {
    let state = match Self::load_actor_state(actor_id) {
      LoadedActorStateOf::NotRegistered => return Ok(ActorEligibility::NotRegistered),
      LoadedActorStateOf::Dormant(_) => return Ok(ActorEligibility::Dormant),
      LoadedActorStateOf::Active(state) => state,
      LoadedActorStateOf::Corrupt => return Err(ActorClassificationError::ActorInvariant),
    };
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    if state
      .hot
      .wakeup_pointer
      .is_some_and(|pointer| !Self::wakeup_page_entry_matches(pointer, actor_id))
    {
      return Err(ActorClassificationError::ActorInvariant);
    }
    let placement = match (state.hot.queue_ticket, state.hot.wakeup_pointer) {
      (None, None) => ActorActivationPlacement::Unplaced,
      (Some(ticket), None) => ActorActivationPlacement::Queue(ticket),
      (None, Some(pointer)) => ActorActivationPlacement::Wakeup(pointer.block),
      // A live FIFO ticket may coexist with the actor's terminal window wakeup;
      // the queue ticket is the current activation placement.
      (Some(ticket), Some(_)) => ActorActivationPlacement::Queue(ticket),
    };
    let trigger = match &state.contract.trigger {
      Trigger::Manual => ActorTriggerActivation::Manual,
      Trigger::AddressEvent { .. } => ActorTriggerActivation::AddressEvent,
      Trigger::ObservationChange { feed } => {
        let feeds = ActorObservationFeeds::<T>::get(actor_id)
          .ok_or(ActorClassificationError::ActorInvariant)?;
        if feeds.as_slice() != [*feed] || !ObservationSubscriptionSlot::<T>::contains_key(actor_id)
        {
          return Err(ActorClassificationError::ActorInvariant);
        }
        ActorTriggerActivation::ObservationChange {
          feed: *feed,
          subscriber_count: ObservationSubscriberCount::<T>::get(feed),
          pending_revision: DirtyObservationFeeds::<T>::get(feed)
            .map(|dirty| dirty.latest_revision),
        }
      }
      Trigger::ObservationCrossing { .. } => {
        let crossing = Self::crossing_from_trigger(&state.contract.trigger)
          .ok_or(ActorClassificationError::ActorInvariant)?;
        let locator = CrossingMemberships::<T>::get(actor_id)
          .ok_or(ActorClassificationError::ActorInvariant)?;
        let TriggerRuntimeState::ObservationCrossing {
          phase,
          installed_at_revision,
        } = state.hot.trigger_runtime_state
        else {
          return Err(ActorClassificationError::ActorInvariant);
        };
        let (key, _) = Self::crossing_obligation(&crossing, phase);
        if locator.key != key {
          return Err(ActorClassificationError::ActorInvariant);
        }
        ActorTriggerActivation::ObservationCrossing {
          feed: crossing.feed,
          direction: crossing.direction,
          threshold: crossing.threshold,
          rearm_threshold: crossing.rearm_threshold,
          phase,
          installed_at_revision,
          pending_revisions: CrossingTransitionQueues::<T>::get(crossing.feed)
            .map_or(0, |queue| queue.len() as u32),
          processing_revision: CrossingRangeCursors::<T>::get(crossing.feed)
            .map(|cursor| cursor.revision),
        }
      }
      Trigger::Cadenced { every_ticks } => ActorTriggerActivation::Cadenced {
        every_ticks: *every_ticks,
      },
    };
    Ok(ActorEligibility::Active(ActiveActorActivation {
      trigger,
      pending_signal: state.hot.pending_signal,
      placement,
      eligibility: Self::classify_actor_loaded(&instance, state.continuation.as_ref())?,
    }))
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
      (SourceFilter::Whitelist(list), Some(who)) => list.contains(who),
      (SourceFilter::Whitelist(_), None) => false,
    }
  }

  fn asset_matches_filter(filter: &AssetFilterOf<T>, asset: T::AssetId) -> bool {
    match filter {
      AssetFilter::Any => true,
      AssetFilter::Whitelist(list) => list.contains(&asset),
    }
  }

  pub fn notify_address_event(
    actor_id: ActorId,
    asset: T::AssetId,
    amount: T::Balance,
    source: &T::AccountId,
  ) -> DispatchResult {
    let provenance = FundingProvenance::Signed;
    Self::notify_address_event_with_context(
      actor_id,
      asset,
      amount,
      Some(source),
      Some(&provenance),
    )
  }

  pub fn notify_internal_address_event(
    actor_id: ActorId,
    asset: T::AssetId,
    amount: T::Balance,
    source: &T::AccountId,
  ) -> DispatchResult {
    let provenance = FundingProvenance::InternalProtocol;
    Self::notify_address_event_with_context(
      actor_id,
      asset,
      amount,
      Some(source),
      Some(&provenance),
    )
  }

  pub fn notify_xcm_address_event(
    actor_id: ActorId,
    asset: T::AssetId,
    amount: T::Balance,
    source: &T::AccountId,
  ) -> DispatchResult {
    let provenance = FundingProvenance::Xcm;
    Self::notify_address_event_with_context(
      actor_id,
      asset,
      amount,
      Some(source),
      Some(&provenance),
    )
  }

  pub fn notify_address_event_without_source(
    actor_id: ActorId,
    asset: T::AssetId,
    amount: T::Balance,
  ) -> DispatchResult {
    Self::notify_address_event_with_context(actor_id, asset, amount, None, None)
  }

  fn funding_event_authorized(
    actor_id: ActorId,
    owner: &T::AccountId,
    policy: &FundingSourcePolicyOf<T>,
    source: Option<&T::AccountId>,
    provenance: Option<&FundingProvenance>,
  ) -> bool {
    match policy {
      FundingSourcePolicy::OwnerOnly => {
        provenance == Some(&FundingProvenance::Signed) && source == Some(owner)
      }
      FundingSourcePolicy::SignedAllowlist(allowed) => {
        provenance == Some(&FundingProvenance::Signed)
          && source.is_some_and(|source| allowed.contains(source))
      }
      FundingSourcePolicy::RuntimePolicy => {
        T::FundingAuthority::permits(actor_id, owner, source, provenance)
      }
      FundingSourcePolicy::AnyVerifiedIngress => source.is_some() || provenance.is_some(),
    }
  }

  pub fn preflight_funding_event(
    actor_id: ActorId,
    asset: T::AssetId,
    amount: T::Balance,
    source: Option<&T::AccountId>,
    provenance: Option<&FundingProvenance>,
  ) -> DispatchResult {
    let state = match Self::load_actor_state(actor_id) {
      LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => return Ok(()),
      LoadedActorStateOf::Active(state) => state,
      LoadedActorStateOf::Corrupt => return Err(Error::<T>::ActorInvariant.into()),
    };
    let authorized = Self::funding_event_authorized(
      actor_id,
      &state.identity.owner,
      &state.contract.funding,
      source,
      provenance,
    );
    let funding = state.funding;
    let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
    let classification = Self::classify_actor_loaded(&instance, state.continuation.as_ref())
      .map_err(Self::classification_dispatch_error)?;
    if classification.terminal_reason == Some(CloseReason::WindowExpired) || amount.is_zero() {
      return Ok(());
    }
    if !authorized || !funding.funding_tracked_assets.contains(&asset) {
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

  /// Typed certified-ingress preflight (spec 5.3, 6.2). Read-only and covers
  /// lifecycle, funding, trigger, and required placement. An absent or Dormant
  /// destination, a zero amount, and an expired window are balance-only.
  pub fn preflight_ingress(
    event: &AddressEvent<T::AccountId, T::AssetId, T::Balance>,
  ) -> Result<(), IngressFailure> {
    let Some(actor_id) = Self::sovereign_index(&event.destination) else {
      return Ok(());
    };
    Self::preflight_funding_event(
      actor_id,
      event.asset,
      event.amount,
      event.source.as_ref(),
      event.provenance.as_ref(),
    )
    .map_err(IngressFailure::permanent)
  }

  /// Typed certified-ingress consequence (spec 5.3, 6.2). Executes exactly once at
  /// the host protocol's declared notify or transactional-precommit phase and preserves
  /// the placement classification: recoverable queue/wakeup capacity or placement
  /// unavailability is Temporary; monotonic
  /// ticket/index exhaustion, topology corruption, and invariant failure are
  /// Permanent.
  pub fn notify_ingress(
    event: &AddressEvent<T::AccountId, T::AssetId, T::Balance>,
  ) -> Result<(), IngressFailure> {
    let Some(actor_id) = Self::sovereign_index(&event.destination) else {
      return Ok(());
    };
    Self::notify_address_event_with_context(
      actor_id,
      event.asset,
      event.amount,
      event.source.as_ref(),
      event.provenance.as_ref(),
    )
    .map_err(Self::classify_ingress_error)
  }

  /// Maps one certified-ingress error to its closed retry class.
  ///
  /// Recoverable queue/wakeup capacity or placement unavailability surfaces as
  /// `QueueCapacityUnavailable` (queue saturation and failed wakeup placement)
  /// and is Temporary. Monotonic ticket/index exhaustion, topology corruption,
  /// and invariant failure are Permanent.
  fn classify_ingress_error(error: DispatchError) -> IngressFailure {
    if error == Error::<T>::QueueCapacityUnavailable.into() {
      IngressFailure::temporary(error)
    } else {
      IngressFailure::permanent(error)
    }
  }

  fn notify_address_event_with_context(
    actor_id: ActorId,
    asset: T::AssetId,
    amount: T::Balance,
    source: Option<&T::AccountId>,
    provenance: Option<&FundingProvenance>,
  ) -> DispatchResult {
    // Zero or self/no-op movement creates no Actors ingress (spec 5.3).
    if amount.is_zero() {
      return Ok(());
    }
    Self::preflight_funding_event(actor_id, asset, amount, source, provenance)?;
    Self::with_reused_transaction(|| {
      Self::apply_address_event_parts(actor_id, asset, amount, source, provenance, true, true)
    })
  }

  fn apply_address_event_parts(
    actor_id: ActorId,
    asset: T::AssetId,
    amount: T::Balance,
    source: Option<&T::AccountId>,
    provenance: Option<&FundingProvenance>,
    apply_trigger: bool,
    apply_funding: bool,
  ) -> DispatchResult {
    let state = match Self::load_actor_state(actor_id) {
      LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => return Ok(()),
      LoadedActorStateOf::Active(state) => state,
      LoadedActorStateOf::Corrupt => return Err(Error::<T>::ActorInvariant.into()),
    };
    let funding_authorized = Self::funding_event_authorized(
      actor_id,
      &state.identity.owner,
      &state.contract.funding,
      source,
      provenance,
    );
    let mut funding = state.funding;
    let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
    let classification = Self::classify_actor_loaded(&instance, state.continuation.as_ref())
      .map_err(Self::classification_dispatch_error)?;
    if classification.terminal_reason == Some(CloseReason::WindowExpired) {
      return Self::finalize_actor(actor_id, &instance, CloseReason::WindowExpired);
    }
    let signal_matched = if apply_trigger
      && let Trigger::AddressEvent {
        source_filter,
        asset_filter,
      } = &instance.trigger
    {
      Self::source_matches_filter(source_filter, &instance.owner, source)
        && Self::asset_matches_filter(asset_filter, asset)
    } else {
      false
    };
    if apply_funding && amount > Zero::zero() {
      if funding_authorized && funding.funding_tracked_assets.contains(&asset) {
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
        ActorFunding::<T>::insert(actor_id, funding);
        Self::deposit_event(Event::FundingAccumulated {
          actor_id,
          asset,
          added: amount,
          accumulated,
        });
      }
    }
    if signal_matched {
      Self::request_activation(actor_id).map_err(Self::activation_failure_error)?;
    }
    Ok(())
  }

  pub(crate) fn evaluate_actor_liveness(actor_id: ActorId) -> DispatchResult {
    let state = match Self::load_actor_state(actor_id) {
      LoadedActorStateOf::Active(state) => state,
      LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => {
        return Err(Error::<T>::ActorNotFound.into());
      }
      LoadedActorStateOf::Corrupt => return Err(Error::<T>::ActorInvariant.into()),
    };
    let continuation = state.continuation;
    let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
    if let Some(reason) = Self::classify_actor_loaded(&instance, continuation.as_ref())
      .map_err(Self::classification_dispatch_error)?
      .terminal_reason
    {
      return Self::finalize_actor(actor_id, &instance, reason);
    }
    Ok(())
  }
}
