use super::pallet::*;
use super::{
  AddressEvent, AssetOps, BlockResourceDomain, BlockResourceLimits, BlockResourceState,
  FundingAuthority, IngressFailure, StepControlExecution, StepControlOutcome, StepControlPhase,
  StepControlPlacement, StepControlWeightProvider as _, TaskEffectWeightProvider as _,
  weights::WeightInfo,
};
#[cfg(test)]
use alloc::vec;
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

enum ReadyHeadOwner<T: Config> {
  #[cfg(any(test, feature = "runtime-benchmarks"))]
  DiscoverCanonical,
  Loaded {
    actor_id: ActorId,
    ticket: QueueTicket,
    hot: ActorHotStateOf<T>,
  },
  ClosedTombstone,
}

pub(crate) struct QueueAppendPlan<T: Config> {
  publications: Vec<PreparedReadyPublication<T>>,
  next_tail: QueueTicket,
  next_occupancy: u32,
}

struct PreparedReadyPublication<T: Config> {
  ticket: QueueTicket,
  cell: ActorControlCellOf<T>,
}

#[derive(Clone, Copy)]
enum TerminalCleanupReservation {
  Included,
  NotIncluded,
}

impl TerminalCleanupReservation {
  fn is_included(self) -> bool {
    matches!(self, Self::Included)
  }
}

#[derive(Clone, Copy)]
enum ServiceCutoff {
  Open,
  Snapshotted,
}

#[derive(Clone, Copy)]
pub enum WakeupBucketDisposition {
  Retain,
  Remove,
}

impl ServiceCutoff {
  fn is_snapshotted(self) -> bool {
    matches!(self, Self::Snapshotted)
  }
}

enum AdmissionDecision {
  Admit {
    weight: Weight,
    terminal_cleanup: TerminalCleanupReservation,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ObservationActivationOutcome {
  Ordinary(ActivationOutcome),
  TerminalDeferred,
}

#[derive(Clone, Copy)]
enum ObservationTerminalHandling {
  Execute,
  Defer,
}

pub(crate) struct ObservationQueueCandidate<T: Config> {
  state: ObservationActivationState<T>,
  hot: ActorHotStateOf<T>,
}

pub(crate) struct ObservationWakeupCandidate<T: Config> {
  state: ObservationActivationState<T>,
  hot: ActorHotStateOf<T>,
  wakeup_key: WakeupKey<BlockNumberFor<T>>,
}

pub(crate) enum ObservationPlacementCandidate<T: Config> {
  Queue(ObservationQueueCandidate<T>),
  Wakeup(ObservationWakeupCandidate<T>),
}

impl<T: Config> ObservationPlacementCandidate<T> {
  pub(crate) fn wakeup_key(&self) -> Option<WakeupKey<BlockNumberFor<T>>> {
    match self {
      Self::Queue(_) => None,
      Self::Wakeup(candidate) => Some(candidate.wakeup_key),
    }
  }
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
  Close(CloseReason),
  CoalesceLive,
  EnqueueReady(Result<QueueAppendPlan<T>, EnqueueOutcome>),
  EnqueueTemporal(Result<QueueAppendPlan<T>, EnqueueOutcome>),
  PrimeSchedule(Result<PrimeSchedulePlan<BlockNumberFor<T>>, EnqueueOutcome>),
}

/// Synchronous transition authority: commit before any intervening Actor mutation.
pub(crate) struct ActivationPlan<T: Config> {
  pub actor_id: ActorId,

  pub frame_admission: ActorAdmissionCertificateOf<T>,
  pub frame_source_state: ActiveActorStateOf<T>,
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
  static OBSERVATION_WAKEUP_COHORT_COMMITS: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AttemptTransactionError {
  FeeCollection,
  StateHold,
  Invariant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActorControlTransitionError {
  Invariant,
  IndexExhausted,
}

impl From<polkadot_sdk::sp_runtime::DispatchError> for ActorControlTransitionError {
  fn from(_: polkadot_sdk::sp_runtime::DispatchError) -> Self {
    Self::Invariant
  }
}

#[derive(Clone, Copy)]
pub(crate) enum ActorWaitingAuthority {
  Trigger,
  Service,
}

pub(crate) struct StepCommitEvidence {
  pub(crate) closed_for_exhaustion: bool,
  pub(crate) actual_control_weight: Weight,
  pub(crate) actual_effect_weight: Weight,
  pub(crate) attempt: ActorAttemptEvidence,
}

pub(crate) struct ActorAttemptEvidence {
  status: AttemptDisposition,
  cycle_nonce: u64,
  start_cursor: u32,
  run_cursor: Option<u32>,
  unsuccessful_attempts_at_cursor: Option<u32>,
  cumulative_outcomes: OutcomeTotals,
  step: Option<SimulationStepRecord>,
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
  pub(crate) effect_consumed: Weight,
  pub(crate) effect_reconciliation_uncertain: bool,
  pub(crate) starved: bool,
}

impl CyclePass {
  pub(crate) fn reconciled_domains(&self) -> Option<(Weight, Weight)> {
    if self.effect_reconciliation_uncertain {
      return None;
    }
    self
      .consumed
      .checked_sub(&self.effect_consumed)
      .map(|control| (control, self.effect_consumed))
  }
}

enum FifoStepResult {
  NoWork,
  Progress {
    executed: bool,
    attempt: Option<ActorAttemptEvidence>,
  },
  Blocked(BlockKind),
}

#[derive(Clone, Copy)]
enum ActorServiceSource {
  FifoHead,
  SelectedReady,
}

enum HeadDiscovery<BlockNumber> {
  Empty,
  Head(QueueTicket, QueueEntry<BlockNumber>),
  WeightStall,
  PassExhausted,
  InvariantStall,
}

impl<T: Config> Pallet<T> {
  pub fn next_queue_ticket() -> QueueTicket {
    ActorReadyTail::<T>::get()
  }

  pub fn queue_head() -> QueueTicket {
    ActorReadyHead::<T>::get()
  }

  pub fn queue_tail() -> QueueTicket {
    ActorReadyTail::<T>::get()
  }

  pub fn queue_occupancy() -> u32 {
    ActorReadyOccupancy::<T>::get()
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub(crate) fn execute_cycle(remaining_weight: Weight) -> CyclePass {
    Self::execute_cycle_to_cutoff(remaining_weight, ActorReadyTail::<T>::get())
  }

  pub(crate) fn execute_cycle_to_cutoff(
    remaining_weight: Weight,
    cutoff: QueueTicket,
  ) -> CyclePass {
    Self::execute_cycle_to_cutoff_inner(remaining_weight, cutoff, None)
  }

  pub(crate) fn execute_cycle_to_cutoff_with_resources(
    remaining_weight: Weight,
    cutoff: QueueTicket,
    state: &mut BlockResourceState<BlockNumberFor<T>>,
    limits: BlockResourceLimits,
    effect_domain: BlockResourceDomain,
    control_maximum: Weight,
  ) -> CyclePass {
    Self::execute_cycle_to_cutoff_inner(
      remaining_weight,
      cutoff,
      Some((state, limits, effect_domain, control_maximum)),
    )
  }

  fn execute_cycle_to_cutoff_inner(
    remaining_weight: Weight,
    cutoff: QueueTicket,
    mut resources: Option<(
      &mut BlockResourceState<BlockNumberFor<T>>,
      BlockResourceLimits,
      BlockResourceDomain,
      Weight,
    )>,
  ) -> CyclePass {
    if remaining_weight.is_zero() {
      return CyclePass {
        consumed: Weight::zero(),
        effect_consumed: Weight::zero(),
        effect_reconciliation_uncertain: false,
        starved: false,
      };
    }
    let mut pass_control_reservation = match resources.as_mut() {
      Some((state, limits, _, control_maximum)) => {
        match state.reserve(*limits, BlockResourceDomain::ActorControl, *control_maximum) {
          Ok(reservation) => Some(reservation),
          Err(_) => {
            state.halt_optional_actor_work();
            return CyclePass {
              consumed: Weight::zero(),
              effect_consumed: Weight::zero(),
              effect_reconciliation_uncertain: false,
              starved: true,
            };
          }
        }
      }
      None => None,
    };
    let mut cycle_meter = WeightMeter::with_limit(remaining_weight);
    let mut control_meter = resources
      .as_ref()
      .map(|(_, _, _, control_maximum)| WeightMeter::with_limit(*control_maximum));
    let now = frame_system::Pallet::<T>::block_number();
    // Only tickets below the caller-owned stage cutoff may execute in this actor-service pass.
    let max_executions = T::MaxExecutionsPerBlock::get();
    let max_scanned = T::MaxQueueEntriesScannedPerBlock::get();
    let mut executed = 0u32;
    let mut scanned = 0u32;
    let mut effect_consumed = Weight::zero();
    let mut effect_reconciliation_uncertain = false;
    let mut starved = false;
    while executed < max_executions && scanned < max_scanned {
      let head = Self::live_queue_head(
        cutoff,
        &mut cycle_meter,
        control_meter.as_mut(),
        &mut scanned,
        max_scanned,
      );
      match head {
        HeadDiscovery::Empty => break,
        HeadDiscovery::WeightStall | HeadDiscovery::InvariantStall => {
          starved = executed == 0;
          break;
        }
        HeadDiscovery::PassExhausted => break,
        HeadDiscovery::Head(position, entry) => {
          match Self::service_live_queue_entry(
            (position, entry),
            now,
            &mut cycle_meter,
            control_meter.as_mut(),
            &mut effect_consumed,
            &mut effect_reconciliation_uncertain,
            resources
              .as_mut()
              .map(|(state, limits, domain, _)| (&mut **state, *limits, *domain)),
            ActorServiceSource::FifoHead,
          ) {
            FifoStepResult::Progress {
              executed: did_execute,
              ..
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
    let pass = CyclePass {
      consumed: cycle_meter.consumed(),
      effect_consumed,
      effect_reconciliation_uncertain,
      starved,
    };
    if let (Some((state, _, _, control_maximum)), Some(reservation)) =
      (resources.as_mut(), pass_control_reservation.as_mut())
    {
      let actual_control = pass
        .reconciled_domains()
        .map(|(actual, _)| actual)
        .unwrap_or(*control_maximum);
      if state.settle(reservation, actual_control).is_err() {
        // Preserve the admitted maximum while releasing transition authority. This second
        // settlement is deterministic because the reservation still owns that exact maximum.
        let _ = state.settle(reservation, *control_maximum);
        state.halt_optional_actor_work();
      } else if pass.reconciled_domains().is_none() {
        state.halt_optional_actor_work();
      }
    }
    pass
  }

  /// Sole cheap readiness-presence seam for block hooks and FIFO classification. frame replaces the
  /// backing counters; callers must not inspect scalar head/tail independently.
  pub(crate) fn ready_work_exists() -> bool {
    ActorReadyHead::<T>::get() < ActorReadyTail::<T>::get()
  }

  fn classify_current_queue(cutoff: QueueTicket) -> HeadDiscovery<BlockNumberFor<T>> {
    if Self::queue_topology_preflight(QueueMutation::Head).is_err() {
      return HeadDiscovery::InvariantStall;
    }
    if !Self::ready_work_exists() {
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
    Self::ready_work_exists()
      && Self::paged_head_entry().is_some_and(|(_, entry)| entry.ticket < cutoff)
  }

  fn live_queue_head(
    cutoff: QueueTicket,
    cycle_meter: &mut WeightMeter,
    mut scan_control_meter: Option<&mut WeightMeter>,
    scanned: &mut u32,
    max_scanned: u32,
  ) -> HeadDiscovery<BlockNumberFor<T>> {
    let scan_weight = T::WeightInfo::scheduler_paged_tombstone_drain(1);
    while *scanned < max_scanned {
      if Self::queue_topology_preflight(QueueMutation::Head).is_err() {
        return HeadDiscovery::InvariantStall;
      }
      if !cycle_meter.can_consume(scan_weight)
        || scan_control_meter
          .as_ref()
          .is_some_and(|meter| !meter.can_consume(scan_weight))
      {
        return if Self::head_blocked_by_weight(cutoff) {
          HeadDiscovery::WeightStall
        } else {
          HeadDiscovery::Empty
        };
      }
      cycle_meter.consume(scan_weight);
      if let Some(meter) = scan_control_meter.as_deref_mut() {
        meter.consume(scan_weight);
      }
      let before = ActorReadyHead::<T>::get();
      let stats = match Self::paged_drain_tombstones(cutoff, 1) {
        Ok(stats) => stats,
        Err(_) => return HeadDiscovery::InvariantStall,
      };
      if stats.entries_scanned == 0 {
        return Self::classify_current_queue(cutoff);
      }
      *scanned = scanned.saturating_add(stats.entries_scanned);
      if ActorReadyHead::<T>::get() != before {
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
  ) -> (u8, Option<QueueEntry<BlockNumberFor<T>>>, u32) {
    let mut meter = WeightMeter::with_limit(weight);
    let mut scanned = scanned_start;
    let discovery = Self::live_queue_head(cutoff, &mut meter, None, &mut scanned, scan_limit);
    match discovery {
      HeadDiscovery::Empty => (0, None, scanned),
      HeadDiscovery::Head(_, entry) => (1, Some(entry), scanned),
      HeadDiscovery::WeightStall => (2, None, scanned),
      HeadDiscovery::InvariantStall => (3, None, scanned),
      HeadDiscovery::PassExhausted => (4, None, scanned),
    }
  }

  fn charge_pipeline_opening(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
  ) -> Result<(), AttemptTransactionError> {
    if instance.cycle_state != CycleState::Idle
      || instance.actor_class.actor_type() != ActorType::User
    {
      return Ok(());
    }
    let breakdown =
      Self::collect_pipeline_fee(actor_id, ActorType::User, &instance.sovereign_account)
        .map_err(|_| AttemptTransactionError::FeeCollection)?;
    Self::deposit_event(Event::PipelineFeeCharged {
      actor_id,
      fee: breakdown.total_fee,
    });
    Ok(())
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn execute_zero_step_from_consumed_fixture(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<(), AttemptTransactionError> {
    Self::execute_zero_step_from_consumed_frame(actor_id, state, admission, now).map(|_| ())
  }

  fn prepare_cadenced_rearm_hot(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    mut hot: ActorHotStateOf<T>,
  ) -> Result<ActorHotStateOf<T>, AttemptTransactionError> {
    let Trigger::Cadenced { every_ticks } = &instance.trigger else {
      return Err(AttemptTransactionError::Invariant);
    };
    let anchor_tick = instance
      .temporal_anchor_tick
      .ok_or(AttemptTransactionError::Invariant)?;
    if let Some(pointer) = hot.trigger_wakeup_pointer {
      Self::invalidate_wakeup_reference(
        actor_id,
        WakeupPointer {
          block: WakeupKey::Tick(pointer.tick),
          page_id: pointer.page_id,
          slot: pointer.slot,
        },
        admission.admission_identity,
      )
      .map_err(|_| AttemptTransactionError::Invariant)?;
      hot.trigger_wakeup_pointer = None;
    }
    let now_tick =
      Self::current_scheduler_tick().map_err(|_| AttemptTransactionError::Invariant)?;
    let due_tick = next_cadence_due_tick(anchor_tick, *every_ticks, now_tick)
      .ok_or(AttemptTransactionError::Invariant)?;
    let (page_id, slot) = Self::schedule_fresh_wakeup_reference(
      actor_id,
      WakeupKey::Tick(due_tick),
      admission.admission_identity,
    )
    .map_err(|_| AttemptTransactionError::Invariant)?;
    Ok(Self::with_wakeup_pointer(
      hot,
      WakeupKey::Tick(due_tick),
      page_id,
      slot,
    ))
  }

  fn prepare_opening_rearm_hot(
    actor_id: ActorId,
    opening: &ActiveActorViewOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    hot: ActorHotStateOf<T>,
  ) -> Result<ActorHotStateOf<T>, AttemptTransactionError> {
    match &opening.trigger {
      Trigger::Cadenced { .. } => {
        Self::prepare_cadenced_rearm_hot(actor_id, opening, admission, hot)
      }
      Trigger::ObservationCrossing { .. } => {
        Self::prepare_crossing_rearm_hot(actor_id, opening, admission)
          .map_err(|_| AttemptTransactionError::Invariant)
          .map(|replacement| match replacement {
            Some(mut replacement) => {
              // Detector rearming precedes the common Opening core's latch consumption.
              replacement.pending_signal = hot.pending_signal;
              replacement
            }
            None => hot,
          })
      }
      Trigger::ObservationChange { .. } => {
        IndexedTriggerDetectionDisabled::<T>::remove(actor_id);
        Ok(hot)
      }
      Trigger::Manual | Trigger::AddressEvent { .. } | Trigger::AtTime { .. } => Ok(hot),
    }
  }

  fn execute_zero_step_from_consumed_frame(
    actor_id: ActorId,
    mut state: ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<ActorAttemptEvidence, AttemptTransactionError> {
    if !matches!(
      state.contract.trigger,
      Trigger::Manual
        | Trigger::AddressEvent { .. }
        | Trigger::ObservationChange { .. }
        | Trigger::ObservationCrossing { .. }
        | Trigger::AtTime { .. }
        | Trigger::Cadenced { .. }
    ) || !state.contract.steps.is_empty()
      || state.hot.cycle_state != CycleState::Idle
      || state.run_state.is_some()
      || ActorControlLocators::<T>::contains_key(actor_id)
    {
      return Err(AttemptTransactionError::Invariant);
    }
    state.hot.queue_ticket = None;
    let opening = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    state.hot = Self::prepare_opening_rearm_hot(actor_id, &opening, admission, state.hot)?;
    Self::charge_pipeline_opening(actor_id, &opening)?;
    let cycle_nonce = state
      .identity
      .cycle_nonce
      .checked_add(1)
      .ok_or(AttemptTransactionError::Invariant)?;
    state.identity.cycle_nonce = cycle_nonce;
    state.hot.pending_signal = false;
    state.hot.last_cycle_block = Some(now);
    state.hot.unsuccessful_attempt_streak = 0;
    state.funding.funding_accumulated.clear();
    ActorFunding::<T>::insert(actor_id, &state.funding);
    Self::deposit_event(Event::CycleStarted {
      actor_id,
      cycle_nonce,
    });
    Self::deposit_event(Event::CycleSummary {
      actor_id,
      cycle_nonce,
      result: CycleResult::Completed,
      outcomes: OutcomeTotals::default(),
    });
    if state
      .contract
      .auto_close_at_cycle_nonce
      .is_some_and(|target| cycle_nonce >= target)
    {
      Self::finalize_actor_from_consumed_state(
        actor_id,
        state,
        admission,
        CloseReason::AutoCloseNonceReached,
      )
      .map_err(|_| AttemptTransactionError::Invariant)?;
      return Ok(Self::step_simulation_evidence(
        cycle_nonce,
        0,
        AttemptDisposition::Closed(CloseReason::AutoCloseNonceReached),
        OutcomeTotals::default(),
        None,
        None,
      ));
    }
    let resources = ActorStepResourceEnvelope {
      control: T::WeightInfo::scheduler_inner_zero_step_complete(),
      effect: Weight::zero(),
    };
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    let unsignaled_hot = state.hot.clone();
    match Self::schedule_next_work_with_authority(
      actor_id,
      &instance,
      state.hot.clone(),
      &state.identity,
      None,
      admission,
      resources,
      now,
      ServiceCutoff::Snapshotted,
    ) {
      Ok(StepControlPlacement::None) => Self::restore_unsignaled_from_authority(
        actor_id,
        unsignaled_hot,
        &state.identity,
        None,
        admission,
        resources,
      )
      .map_err(|_| AttemptTransactionError::Invariant)?,
      Ok(_) => {}
      Err(_) => {
        Self::finalize_actor_from_consumed_state(
          actor_id,
          state,
          admission,
          CloseReason::SchedulerIndexExhausted,
        )
        .map_err(|_| AttemptTransactionError::Invariant)?;
        return Ok(Self::step_simulation_evidence(
          cycle_nonce,
          0,
          AttemptDisposition::Closed(CloseReason::SchedulerIndexExhausted),
          OutcomeTotals::default(),
          None,
          None,
        ));
      }
    }
    Self::reconcile_actor_state_hold_with_authority(actor_id)
      .map_err(|_| AttemptTransactionError::StateHold)?;
    Ok(Self::step_simulation_evidence(
      cycle_nonce,
      0,
      AttemptDisposition::Completed,
      OutcomeTotals::default(),
      None,
      None,
    ))
  }

  fn execute_stop_cycle_from_consumed_frame(
    actor_id: ActorId,
    mut state: ActiveActorStateOf<T>,
    commit_plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<StepCommitEvidence, AttemptTransactionError> {
    let step = commit_plan.loaded_step.step.clone();
    if state.hot.cycle_state != CycleState::Idle
      || state.run_state.is_some()
      || commit_plan.run.is_some()
      || ActorControlLocators::<T>::contains_key(actor_id)
      || commit_plan.ticket.actor_id != actor_id
      || commit_plan.ticket.cursor != 0
      || commit_plan.loaded_step.cursor != 0
      || commit_plan.ticket.cycle_nonce
        != state
          .identity
          .cycle_nonce
          .checked_add(1)
          .ok_or(AttemptTransactionError::Invariant)?
      || !matches!(step.task, super::types::Task::StopCycle)
      || state.contract.steps.first() != Some(&step)
      || commit_plan.identity != state.identity
      || commit_plan.hot != state.hot
      || commit_plan.funding != state.funding
      || commit_plan.admission != *admission
    {
      return Err(AttemptTransactionError::Invariant);
    }
    state.hot.queue_ticket = None;
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    let control_context =
      Self::execution_step_control_weight_context(&instance, None, &commit_plan.loaded_step)
        .ok_or(AttemptTransactionError::Invariant)?;
    let reserved_control_weight = commit_plan.loaded_step.resources.control;
    let reserved_effect_weight = commit_plan.loaded_step.resources.effect;
    if T::StepControlWeight::maximum_control_weight(control_context, &step)
      != Some(reserved_control_weight)
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let expected_fee = Self::maximum_current_action_fee(
      instance.actor_class.actor_type(),
      &step,
      commit_plan.loaded_step.resources,
    )
    .map_err(|_| AttemptTransactionError::Invariant)?;
    if commit_plan.maximum_fee != expected_fee {
      return Err(AttemptTransactionError::Invariant);
    }
    state.hot = Self::prepare_opening_rearm_hot(actor_id, &instance, admission, state.hot)?;
    Self::charge_pipeline_opening(actor_id, &instance)?;
    let opening_predicate_results = Self::capture_opening_predicate_results(
      &instance.sovereign_account,
      &instance.steps,
      commit_plan.maximum_fee.total_fee,
    );
    let mut opening_predicate_index = 0usize;
    let predicate_matches = Self::evaluate_step_precondition(
      step.precondition.as_ref(),
      &instance.sovereign_account,
      commit_plan.maximum_fee.total_fee,
      &opening_predicate_results,
      &mut opening_predicate_index,
    )
    .map_err(|_| AttemptTransactionError::Invariant)?;
    let cycle_nonce = commit_plan.ticket.cycle_nonce;
    state.identity.cycle_nonce = cycle_nonce;
    state.hot.cycle_state = CycleState::Idle;
    state.hot.pending_signal = false;
    state.hot.last_cycle_block = Some(now);
    state.hot.unsuccessful_attempt_streak = 0;
    state.funding.funding_accumulated.clear();
    ActorFunding::<T>::insert(actor_id, &state.funding);
    Self::deposit_event(Event::CycleStarted {
      actor_id,
      cycle_nonce,
    });
    let (effect_execution, outcomes) = if predicate_matches {
      Self::record_stop_cycle_event(actor_id, cycle_nonce, 0);
      (
        super::TaskEffectExecution::Invoked,
        OutcomeTotals {
          executed_steps: 1,
          ..Default::default()
        },
      )
    } else {
      Self::deposit_event(Event::StepSkipped {
        actor_id,
        cycle_nonce,
        step_index: 0,
        reason: StepSkippedReason::PreconditionFalse,
      });
      (
        super::TaskEffectExecution::NotInvoked,
        OutcomeTotals {
          precondition_skips: 1,
          ..Default::default()
        },
      )
    };
    Self::deposit_event(Event::CycleSummary {
      actor_id,
      cycle_nonce,
      result: CycleResult::Completed,
      outcomes,
    });

    let mut closed_for_exhaustion = false;
    let mut attempt_status = AttemptDisposition::Completed;
    let placement = if state
      .contract
      .auto_close_at_cycle_nonce
      .is_some_and(|target| cycle_nonce >= target)
    {
      Self::finalize_actor_from_consumed_state(
        actor_id,
        state.clone(),
        admission,
        CloseReason::AutoCloseNonceReached,
      )
      .map_err(|_| AttemptTransactionError::Invariant)?;
      attempt_status = AttemptDisposition::Closed(CloseReason::AutoCloseNonceReached);
      StepControlPlacement::None
    } else {
      let placement_instance = Self::derive_active_actor_view(
        state.identity.clone(),
        state.hot.clone(),
        state.contract.clone(),
      );
      let unsignaled_hot = state.hot.clone();
      match Self::schedule_next_work_with_authority(
        actor_id,
        &placement_instance,
        state.hot.clone(),
        &state.identity,
        None,
        admission,
        commit_plan.loaded_step.resources,
        now,
        ServiceCutoff::Snapshotted,
      ) {
        Ok(placement) => {
          if placement == StepControlPlacement::None {
            Self::restore_unsignaled_from_authority(
              actor_id,
              unsignaled_hot,
              &state.identity,
              None,
              admission,
              commit_plan.loaded_step.resources,
            )
            .map_err(|_| AttemptTransactionError::Invariant)?;
          }
          placement
        }
        Err(error) => {
          if !Self::scheduler_index_is_exhausted(error) {
            return Err(AttemptTransactionError::Invariant);
          }
          Self::finalize_actor_from_consumed_state(
            actor_id,
            state.clone(),
            admission,
            CloseReason::SchedulerIndexExhausted,
          )
          .map_err(|_| AttemptTransactionError::Invariant)?;
          closed_for_exhaustion = true;
          attempt_status = AttemptDisposition::Closed(CloseReason::SchedulerIndexExhausted);
          StepControlPlacement::None
        }
      }
    };
    let actual_effect_weight =
      T::TaskEffectWeight::actual_effect_weight(&step.task, effect_execution)
        .ok_or(AttemptTransactionError::Invariant)?;
    if !actual_effect_weight.all_lte(reserved_effect_weight) {
      return Err(AttemptTransactionError::Invariant);
    }
    let actual_control_weight = T::StepControlWeight::actual_control_weight(
      control_context,
      &step,
      reserved_control_weight,
      StepControlExecution {
        phase: StepControlPhase::Opening,
        outcome: StepControlOutcome::Completed,
        placement,
        action_fee_collected: false,
      },
    )
    .ok_or(AttemptTransactionError::Invariant)?;
    if !actual_control_weight.all_lte(reserved_control_weight) {
      return Err(AttemptTransactionError::Invariant);
    }
    let actual_fee = Self::maximum_current_action_fee(
      instance.actor_class.actor_type(),
      &step,
      ActorStepResourceEnvelope {
        control: Weight::zero(),
        effect: actual_effect_weight,
      },
    )
    .map_err(|_| AttemptTransactionError::Invariant)?;
    if instance.actor_class.actor_type() == ActorType::User && !actual_fee.total_fee.is_zero() {
      Self::collect_user_step_fee(&instance.sovereign_account, actual_fee.total_fee)
        .map_err(|_| AttemptTransactionError::FeeCollection)?;
    }
    if ActorControlLocators::<T>::contains_key(actor_id) {
      Self::reconcile_actor_state_hold_with_authority(actor_id)
        .map_err(|_| AttemptTransactionError::StateHold)?;
    }
    Ok(StepCommitEvidence {
      closed_for_exhaustion,
      actual_control_weight,
      actual_effect_weight,
      attempt: Self::step_simulation_evidence(
        cycle_nonce,
        0,
        attempt_status,
        outcomes,
        None,
        Some(if predicate_matches {
          StepOutcome::Stopped
        } else {
          StepOutcome::Skipped(StepSkippedReason::PreconditionFalse)
        }),
      ),
    })
  }

  fn execute_running_stop_cycle_from_consumed_frame(
    actor_id: ActorId,
    mut state: ActiveActorStateOf<T>,
    commit_plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<StepCommitEvidence, AttemptTransactionError> {
    let step = commit_plan.loaded_step.step.clone();
    let mut run = commit_plan
      .run
      .clone()
      .ok_or(AttemptTransactionError::Invariant)?;
    if state.hot.cycle_state != CycleState::Running
      || state.run_state.is_none()
      || !run.running_is_coherent()
      || ActorControlLocators::<T>::contains_key(actor_id)
      || commit_plan.ticket.actor_id != actor_id
      || commit_plan.ticket.cursor != run.cursor
      || commit_plan.ticket.cycle_nonce != run.cycle_nonce
      || commit_plan.loaded_step.cursor != run.cursor
      || !matches!(step.task, super::types::Task::StopCycle)
      || commit_plan.identity != state.identity
      || commit_plan.hot != state.hot
      || commit_plan.funding != state.funding
      || commit_plan.admission != *admission
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    let control_context =
      Self::execution_step_control_weight_context(&instance, Some(&run), &commit_plan.loaded_step)
        .ok_or(AttemptTransactionError::Invariant)?;
    let reserved_control_weight = commit_plan.loaded_step.resources.control;
    let reserved_effect_weight = commit_plan.loaded_step.resources.effect;
    let expected_fee = Self::maximum_current_action_fee(
      instance.actor_class.actor_type(),
      &step,
      commit_plan.loaded_step.resources,
    )
    .map_err(|_| AttemptTransactionError::Invariant)?;
    if commit_plan.maximum_fee != expected_fee {
      return Err(AttemptTransactionError::Invariant);
    }
    let mut predicate_index = run.opening_predicate_cursor as usize;
    let predicate_matches = Self::evaluate_step_precondition(
      step.precondition.as_ref(),
      &instance.sovereign_account,
      commit_plan.maximum_fee.total_fee,
      &run.opening_predicate_results,
      &mut predicate_index,
    )
    .map_err(|_| AttemptTransactionError::Invariant)?;
    let cycle_nonce = run.cycle_nonce;
    let cursor = run.cursor;
    let (effect_execution, outcomes) = if predicate_matches {
      run.cumulative_outcomes.executed_steps = run
        .cumulative_outcomes
        .executed_steps
        .checked_add(1)
        .ok_or(AttemptTransactionError::Invariant)?;
      Self::record_stop_cycle_event(actor_id, cycle_nonce, cursor);
      (super::TaskEffectExecution::Invoked, run.cumulative_outcomes)
    } else {
      run.cumulative_outcomes.precondition_skips = run
        .cumulative_outcomes
        .precondition_skips
        .checked_add(1)
        .ok_or(AttemptTransactionError::Invariant)?;
      Self::deposit_event(Event::StepSkipped {
        actor_id,
        cycle_nonce,
        step_index: cursor,
        reason: StepSkippedReason::PreconditionFalse,
      });
      (
        super::TaskEffectExecution::NotInvoked,
        run.cumulative_outcomes,
      )
    };
    ActorRunStateStore::<T>::remove(actor_id);
    state.run_state = None;
    state.identity.cycle_nonce = cycle_nonce;
    state.hot.cycle_state = CycleState::Idle;
    state.hot.queue_ticket = None;
    state.hot.last_cycle_block = Some(now);
    state.hot.unsuccessful_attempt_streak = 0;
    Self::deposit_event(Event::CycleSummary {
      actor_id,
      cycle_nonce,
      result: CycleResult::Completed,
      outcomes,
    });

    let next_resources = Self::load_current_step_with_admission(actor_id, 0, admission)
      .map(|loaded| loaded.resources)
      .ok_or(AttemptTransactionError::Invariant)?;
    let mut closed_for_exhaustion = false;
    let mut attempt_status = AttemptDisposition::Completed;
    let placement = if state
      .contract
      .auto_close_at_cycle_nonce
      .is_some_and(|target| cycle_nonce >= target)
    {
      Self::finalize_actor_from_consumed_state(
        actor_id,
        state.clone(),
        admission,
        CloseReason::AutoCloseNonceReached,
      )
      .map_err(|_| AttemptTransactionError::Invariant)?;
      attempt_status = AttemptDisposition::Closed(CloseReason::AutoCloseNonceReached);
      StepControlPlacement::None
    } else {
      let placement_instance = Self::derive_active_actor_view(
        state.identity.clone(),
        state.hot.clone(),
        state.contract.clone(),
      );
      let unsignaled_hot = state.hot.clone();
      match Self::schedule_next_work_with_authority(
        actor_id,
        &placement_instance,
        state.hot.clone(),
        &state.identity,
        None,
        admission,
        next_resources,
        now,
        ServiceCutoff::Snapshotted,
      ) {
        Ok(placement) => {
          if placement == StepControlPlacement::None {
            Self::restore_unsignaled_from_authority(
              actor_id,
              unsignaled_hot,
              &state.identity,
              None,
              admission,
              next_resources,
            )
            .map_err(|_| AttemptTransactionError::Invariant)?;
          }
          placement
        }
        Err(error) => {
          if !Self::scheduler_index_is_exhausted(error) {
            return Err(AttemptTransactionError::Invariant);
          }
          Self::finalize_actor_from_consumed_state(
            actor_id,
            state.clone(),
            admission,
            CloseReason::SchedulerIndexExhausted,
          )
          .map_err(|_| AttemptTransactionError::Invariant)?;
          closed_for_exhaustion = true;
          attempt_status = AttemptDisposition::Closed(CloseReason::SchedulerIndexExhausted);
          StepControlPlacement::None
        }
      }
    };
    let actual_effect_weight =
      T::TaskEffectWeight::actual_effect_weight(&step.task, effect_execution)
        .ok_or(AttemptTransactionError::Invariant)?;
    if !actual_effect_weight.all_lte(reserved_effect_weight) {
      return Err(AttemptTransactionError::Invariant);
    }
    let actual_control_weight = T::StepControlWeight::actual_control_weight(
      control_context,
      &step,
      reserved_control_weight,
      StepControlExecution {
        phase: StepControlPhase::Running,
        outcome: StepControlOutcome::Completed,
        placement,
        action_fee_collected: false,
      },
    )
    .ok_or(AttemptTransactionError::Invariant)?;
    if !actual_control_weight.all_lte(reserved_control_weight) {
      return Err(AttemptTransactionError::Invariant);
    }
    let actual_fee = Self::maximum_current_action_fee(
      instance.actor_class.actor_type(),
      &step,
      ActorStepResourceEnvelope {
        control: Weight::zero(),
        effect: actual_effect_weight,
      },
    )
    .map_err(|_| AttemptTransactionError::Invariant)?;
    if instance.actor_class.actor_type() == ActorType::User && !actual_fee.total_fee.is_zero() {
      Self::collect_user_step_fee(&instance.sovereign_account, actual_fee.total_fee)
        .map_err(|_| AttemptTransactionError::FeeCollection)?;
    }
    if ActorControlLocators::<T>::contains_key(actor_id) {
      Self::reconcile_actor_state_hold_with_authority(actor_id)
        .map_err(|_| AttemptTransactionError::StateHold)?;
    }
    Ok(StepCommitEvidence {
      closed_for_exhaustion,
      actual_control_weight,
      actual_effect_weight,
      attempt: Self::step_simulation_evidence(
        cycle_nonce,
        cursor,
        attempt_status,
        outcomes,
        None,
        Some(if predicate_matches {
          StepOutcome::Stopped
        } else {
          StepOutcome::Skipped(StepSkippedReason::PreconditionFalse)
        }),
      ),
    })
  }

  fn execute_effectful_step_from_consumed_frame(
    actor_id: ActorId,
    mut state: ActiveActorStateOf<T>,
    mut plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<StepCommitEvidence, AttemptTransactionError> {
    let header =
      ActorContractHeads::<T>::get(actor_id).ok_or(AttemptTransactionError::Invariant)?;
    if header.header.admission_identity != admission.admission_identity {
      return Err(AttemptTransactionError::Invariant);
    }
    let step_count = header.header.step_count;
    if state.hot.cycle_state == CycleState::Idle {
      state.contract = Self::load_contract_geometry_with_admission(actor_id, admission)
        .ok_or(AttemptTransactionError::Invariant)?;
    }
    let step = plan.loaded_step.step.clone();
    let direct_task_policy = matches!(
      step.on_error,
      StepErrorPolicy::ContinueNextStep
        | StepErrorPolicy::AbortCycle
        | StepErrorPolicy::RetryLater { .. }
    );
    if ActorControlLocators::<T>::contains_key(actor_id)
      || plan.identity != state.identity
      || plan.hot != state.hot
      || plan.funding != state.funding
      || plan.admission != *admission
      || !direct_task_policy
    {
      return Err(AttemptTransactionError::Invariant);
    }
    let execution_instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    let control_context = Self::execution_step_control_weight_context(
      &execution_instance,
      plan.run.as_ref(),
      &plan.loaded_step,
    )
    .ok_or(AttemptTransactionError::Invariant)?;
    let reserved_control_weight = plan.loaded_step.resources.control;
    let reserved_effect_weight = plan.loaded_step.resources.effect;
    let retry_attempt_limit_reached = if let Some(max_attempts) = step.on_error.retry_max_attempts()
    {
      plan
        .run
        .as_ref()
        .map_or(0, |run| run.unsuccessful_attempts_at_cursor)
        .checked_add(1)
        .ok_or(AttemptTransactionError::Invariant)?
        >= max_attempts
    } else {
      false
    };
    if execution_instance.cycle_state == CycleState::Idle {
      plan.hot.queue_ticket = None;
      plan.hot =
        Self::prepare_opening_rearm_hot(actor_id, &execution_instance, admission, plan.hot)?;
    }
    Self::charge_pipeline_opening(actor_id, &execution_instance)?;
    let (mut plan, effect_execution, disposition, outcomes, eligible_at) =
      Self::execute_loaded_single_step_core(actor_id, &execution_instance, plan, now, step_count)?;
    let actual_effect_weight =
      T::TaskEffectWeight::actual_effect_weight(&step.task, effect_execution)
        .ok_or(AttemptTransactionError::Invariant)?;
    if !actual_effect_weight.all_lte(reserved_effect_weight) {
      return Err(AttemptTransactionError::Invariant);
    }
    let placement_run = plan.run.take();
    let mut attempt = Self::step_simulation_evidence(
      plan.ticket.cycle_nonce,
      plan.loaded_step.cursor,
      disposition,
      outcomes,
      placement_run.as_ref(),
      Some(
        plan
          .last_step_outcome
          .take()
          .ok_or(AttemptTransactionError::Invariant)?,
      ),
    );
    let placement_cursor = placement_run.as_ref().map_or(0, |run| run.cursor);
    let placement_resources =
      Self::load_current_step_with_admission(actor_id, placement_cursor, admission)
        .map(|loaded| loaded.resources)
        .ok_or(AttemptTransactionError::Invariant)?;
    let placement_instance = Self::derive_active_actor_view(
      plan.identity.clone(),
      plan.hot.clone(),
      state.contract.clone(),
    );
    let unsignaled_hot = plan.hot.clone();
    let mut closed_for_exhaustion = false;
    let failure_close_reason = if disposition != AttemptDisposition::Failed {
      None
    } else if retry_attempt_limit_reached {
      Some(CloseReason::RetryAttemptsExhausted)
    } else if Self::failure_limit_reached(plan.hot.unsuccessful_attempt_streak) {
      Some(CloseReason::ConsecutiveFailures)
    } else {
      None
    };
    let successful_close_reason = if disposition != AttemptDisposition::Completed {
      None
    } else if state.contract.completion == CompletionPolicy::CloseAfterProductiveCycle
      && outcomes.committed_effectful_tasks > 0
    {
      Some(CloseReason::ProductiveCycleCompleted)
    } else {
      state
        .contract
        .auto_close_at_cycle_nonce
        .filter(|target_nonce| plan.identity.cycle_nonce >= *target_nonce)
        .map(|_| CloseReason::AutoCloseNonceReached)
    };
    let placement = if let Some(close_reason) = failure_close_reason.or(successful_close_reason) {
      state.identity = plan.identity.clone();
      state.hot = plan.hot.clone();
      state.run_state = placement_run;
      state.funding = plan.funding.clone();
      Self::finalize_actor_from_consumed_state(actor_id, state, admission, close_reason)
        .map_err(|_| AttemptTransactionError::Invariant)?;
      attempt.status = AttemptDisposition::Closed(close_reason);
      attempt.run_cursor = None;
      attempt.unsuccessful_attempts_at_cursor = None;
      StepControlPlacement::None
    } else {
      match Self::schedule_next_work_with_authority(
        actor_id,
        &placement_instance,
        plan.hot.clone(),
        &plan.identity,
        placement_run.as_ref(),
        admission,
        placement_resources,
        now,
        ServiceCutoff::Snapshotted,
      ) {
        Ok(placement) => {
          if placement == StepControlPlacement::None {
            Self::restore_unsignaled_from_authority(
              actor_id,
              unsignaled_hot,
              &plan.identity,
              placement_run.as_ref(),
              admission,
              placement_resources,
            )
            .map_err(|_| AttemptTransactionError::Invariant)?;
          }
          placement
        }
        Err(error) => {
          if !Self::scheduler_index_is_exhausted(error) {
            return Err(AttemptTransactionError::Invariant);
          }
          state.identity = plan.identity.clone();
          state.hot = plan.hot.clone();
          state.run_state = placement_run;
          state.funding = plan.funding.clone();
          Self::finalize_actor_from_consumed_state(
            actor_id,
            state,
            admission,
            CloseReason::SchedulerIndexExhausted,
          )
          .map_err(|_| AttemptTransactionError::Invariant)?;
          closed_for_exhaustion = true;
          attempt.status = AttemptDisposition::Closed(CloseReason::SchedulerIndexExhausted);
          attempt.run_cursor = None;
          attempt.unsuccessful_attempts_at_cursor = None;
          StepControlPlacement::None
        }
      }
    };
    let control_outcome = match disposition {
      AttemptDisposition::Completed => StepControlOutcome::Completed,
      AttemptDisposition::Continued => StepControlOutcome::Continued,
      AttemptDisposition::Suspended => StepControlOutcome::Suspended,
      AttemptDisposition::Failed => StepControlOutcome::Failed,
      _ => return Err(AttemptTransactionError::Invariant),
    };
    // Completion may leave a paid deferred activation or terminal deadline even
    // though the completed Run no longer has its own service eligibility.
    if eligible_at.is_some() && placement == StepControlPlacement::None && !closed_for_exhaustion {
      return Err(AttemptTransactionError::Invariant);
    }
    let actual_fee = Self::maximum_current_action_fee(
      execution_instance.actor_class.actor_type(),
      &step,
      ActorStepResourceEnvelope {
        control: Weight::zero(),
        effect: actual_effect_weight,
      },
    )
    .map_err(|_| AttemptTransactionError::Invariant)?;
    let action_fee_collected = execution_instance.actor_class.actor_type() == ActorType::User
      && !actual_fee.total_fee.is_zero();
    let actual_control_weight = T::StepControlWeight::actual_control_weight(
      control_context,
      &step,
      reserved_control_weight,
      StepControlExecution {
        phase: match execution_instance.cycle_state {
          CycleState::Idle => StepControlPhase::Opening,
          CycleState::Running => StepControlPhase::Running,
          CycleState::Suspended => StepControlPhase::Suspended,
        },
        outcome: control_outcome,
        placement,
        action_fee_collected,
      },
    )
    .ok_or(AttemptTransactionError::Invariant)?;
    if !actual_control_weight.all_lte(reserved_control_weight) {
      return Err(AttemptTransactionError::Invariant);
    }
    if action_fee_collected {
      Self::collect_user_step_fee(&execution_instance.sovereign_account, actual_fee.total_fee)
        .map_err(|_| AttemptTransactionError::FeeCollection)?;
    }
    if matches!(effect_execution, super::TaskEffectExecution::Invoked) {
      Self::deposit_event(Event::ActionFeeCharged {
        actor_id,
        cycle_nonce: plan.ticket.cycle_nonce,
        step_index: plan.loaded_step.cursor,
        actual_effect_weight,
        fee: actual_fee.effect_fee,
      });
    }
    if ActorControlLocators::<T>::contains_key(actor_id) {
      Self::reconcile_actor_state_hold_with_authority(actor_id)
        .map_err(|_| AttemptTransactionError::StateHold)?;
    }
    Ok(StepCommitEvidence {
      closed_for_exhaustion,
      actual_control_weight,
      actual_effect_weight,
      attempt,
    })
  }

  pub(crate) fn execute_current_step_and_place(
    actor_id: ActorId,
    state: &ActiveActorStateOf<T>,
    commit_plan: CurrentStepPlanOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
  ) -> Result<StepCommitEvidence, AttemptTransactionError> {
    if matches!(
      commit_plan.loaded_step.step.task,
      super::types::Task::StopCycle
    ) && commit_plan.loaded_step.step.precondition.is_none()
    {
      match state.hot.cycle_state {
        CycleState::Idle => Self::execute_stop_cycle_from_consumed_frame(
          actor_id,
          state.clone(),
          commit_plan,
          admission,
          now,
        ),
        CycleState::Running => Self::execute_running_stop_cycle_from_consumed_frame(
          actor_id,
          state.clone(),
          commit_plan,
          admission,
          now,
        ),
        CycleState::Suspended => Err(AttemptTransactionError::Invariant),
      }
    } else {
      Self::execute_effectful_step_from_consumed_frame(
        actor_id,
        state.clone(),
        commit_plan,
        admission,
        now,
      )
    }
  }

  fn control_blank_control_chunk() -> Result<ActorControlChunkOf<T>, ActorControlTransitionError> {
    BoundedVec::try_from(alloc::vec![None; 32]).map_err(|_| ActorControlTransitionError::Invariant)
  }

  fn append_waiting_entry(
    key: WakeupKey<BlockNumberFor<T>>,
    entry: impl FnOnce(WakeupPageId, WakeupSlot) -> ActorWaitingEntry<ActorControlCellOf<T>>,
  ) -> Result<(WakeupPageId, WakeupSlot), ActorControlTransitionError> {
    let occupancy = ActorWaitingOccupancies::<T>::get(key);
    let tail = ActorWaitingTails::<T>::get(key);
    Self::validate_waiting_directory(key, occupancy)?;
    let next_tail = tail
      .checked_add(1)
      .ok_or(ActorControlTransitionError::IndexExhausted)?;
    let next_occupancy = occupancy
      .checked_add(1)
      .filter(|value| *value <= T::MaxActiveActors::get())
      .ok_or(ActorControlTransitionError::IndexExhausted)?;
    let page_id = tail / 32;
    let slot = (tail % 32) as u32;
    let mut page = if slot == 0 {
      if ActorWaitingFrameChunks::<T>::contains_key((key, page_id)) {
        return Err(ActorControlTransitionError::Invariant);
      }
      let previous_page = if occupancy == 0 {
        None
      } else {
        tail.checked_sub(1).map(|tail| tail / 32)
      };
      if let Some(previous_id) = previous_page {
        let mut previous = ActorWaitingFrameChunks::<T>::get((key, previous_id))
          .ok_or(ActorControlTransitionError::Invariant)?;
        if previous.next_page.is_some() || previous.live_entries == 0 {
          return Err(ActorControlTransitionError::Invariant);
        }
        previous.next_page = Some(page_id);
        ActorWaitingFrameChunks::<T>::insert((key, previous_id), previous);
      }
      ActorWaitingPageOf::<T> {
        entries: BoundedVec::try_from(alloc::vec![None; 32])
          .map_err(|_| ActorControlTransitionError::Invariant)?,
        live_entries: 0,
        scan_slot: 0,
        previous_page,
        next_page: None,
      }
    } else {
      ActorWaitingFrameChunks::<T>::get((key, page_id))
        .ok_or(ActorControlTransitionError::Invariant)?
    };
    if page.next_page.is_some() || page.entries.get(slot as usize).is_none_or(Option::is_some) {
      return Err(ActorControlTransitionError::Invariant);
    }
    page.entries[slot as usize] = Some(entry(page_id, slot));
    page.live_entries = page
      .live_entries
      .checked_add(1)
      .ok_or(ActorControlTransitionError::Invariant)?;
    ActorWaitingFrameChunks::<T>::insert((key, page_id), page);
    if occupancy == 0 {
      ActorWaitingHeads::<T>::insert(key, tail);
    }
    ActorWaitingTails::<T>::insert(key, next_tail);
    ActorWaitingOccupancies::<T>::insert(key, next_occupancy);
    if occupancy == 0 && !Self::wakeup_cursor_insert_inner(key) {
      return Err(ActorControlTransitionError::Invariant);
    }
    Ok((page_id, slot))
  }

  fn remove_waiting_entry(
    pointer: WakeupPointer<BlockNumberFor<T>>,
  ) -> Result<ActorWaitingEntry<ActorControlCellOf<T>>, ActorControlTransitionError> {
    let key = pointer.block;
    Self::validate_waiting_directory(key, ActorWaitingOccupancies::<T>::get(key))?;
    let mut page = ActorWaitingFrameChunks::<T>::get((key, pointer.page_id))
      .ok_or(ActorControlTransitionError::Invariant)?;
    // ActorWaitingChunkOf is type-bounded to 32 slots, including malformed short pages.
    if page.entries.iter(/* deos-bypass: bounded-iter */).filter(|entry| entry.is_some()).count()
      != page.live_entries as usize
    {
      return Err(ActorControlTransitionError::Invariant);
    }
    let entry = page
      .entries
      .get_mut(pointer.slot as usize)
      .and_then(Option::take)
      .ok_or(ActorControlTransitionError::Invariant)?;
    page.live_entries = page
      .live_entries
      .checked_sub(1)
      .ok_or(ActorControlTransitionError::Invariant)?;
    let occupancy = ActorWaitingOccupancies::<T>::get(key)
      .checked_sub(1)
      .ok_or(ActorControlTransitionError::Invariant)?;
    if (occupancy == 0)
      != (page.live_entries == 0 && page.previous_page.is_none() && page.next_page.is_none())
      || occupancy < page.live_entries
    {
      return Err(ActorControlTransitionError::Invariant);
    }
    if page.live_entries > 0 {
      ActorWaitingFrameChunks::<T>::insert((key, pointer.page_id), page);
      ActorWaitingOccupancies::<T>::insert(key, occupancy);
      return Ok(entry);
    }
    if let Some(previous_id) = page.previous_page {
      let mut previous = ActorWaitingFrameChunks::<T>::get((key, previous_id))
        .ok_or(ActorControlTransitionError::Invariant)?;
      if previous.next_page != Some(pointer.page_id) {
        return Err(ActorControlTransitionError::Invariant);
      }
      previous.next_page = page.next_page;
      ActorWaitingFrameChunks::<T>::insert((key, previous_id), previous);
    }
    if let Some(next_id) = page.next_page {
      let mut next = ActorWaitingFrameChunks::<T>::get((key, next_id))
        .ok_or(ActorControlTransitionError::Invariant)?;
      if next.previous_page != Some(pointer.page_id) {
        return Err(ActorControlTransitionError::Invariant);
      }
      next.previous_page = page.previous_page;
      if page.previous_page.is_none() {
        ActorWaitingHeads::<T>::insert(
          key,
          next_id
            .checked_mul(32)
            .and_then(|start| start.checked_add(u64::from(next.scan_slot)))
            .ok_or(ActorControlTransitionError::IndexExhausted)?,
        );
      }
      ActorWaitingFrameChunks::<T>::insert((key, next_id), next);
    } else if let Some(previous_id) = page.previous_page {
      ActorWaitingTails::<T>::insert(
        key,
        previous_id
          .checked_add(1)
          .and_then(|next| next.checked_mul(32))
          .ok_or(ActorControlTransitionError::IndexExhausted)?,
      );
    }
    ActorWaitingFrameChunks::<T>::remove((key, pointer.page_id));
    if occupancy == 0 {
      if page.previous_page.is_some()
        || page.next_page.is_some()
        || !Self::control_wakeup_cursor_release(key)
      {
        return Err(ActorControlTransitionError::Invariant);
      }
      ActorWaitingHeads::<T>::remove(key);
      ActorWaitingTails::<T>::remove(key);
      ActorWaitingOccupancies::<T>::remove(key);
    } else {
      ActorWaitingOccupancies::<T>::insert(key, occupancy);
    }
    Ok(entry)
  }

  fn validate_waiting_directory(
    key: WakeupKey<BlockNumberFor<T>>,
    occupancy: u32,
  ) -> Result<(), ActorControlTransitionError> {
    // The draining owner advances head before consuming its last live slot, so
    // head == tail is a valid in-transaction intermediate, not a missing directory.
    let head_present = ActorWaitingHeads::<T>::contains_key(key);
    let tail_present = ActorWaitingTails::<T>::contains_key(key);
    if occupancy == 0 {
      if head_present || tail_present {
        return Err(ActorControlTransitionError::Invariant);
      }
    } else if !head_present
      || !tail_present
      || ActorWaitingHeads::<T>::get(key) > ActorWaitingTails::<T>::get(key)
      || ActorWaitingCursorIndices::<T>::get(key).is_none()
    {
      return Err(ActorControlTransitionError::Invariant);
    }
    Ok(())
  }

  pub(crate) fn control_append_waiting(
    mut cell: ActorControlCellOf<T>,
    key: WakeupKey<BlockNumberFor<T>>,
    authority: ActorWaitingAuthority,
  ) -> Result<ActorControlLocation<BlockNumberFor<T>>, ActorControlTransitionError> {
    let actor_id = cell.actor_id;
    if ActorControlLocators::<T>::contains_key(actor_id)
      || ActorUnsignaledControlCells::<T>::contains_key(actor_id)
    {
      return Err(ActorControlTransitionError::Invariant);
    }
    let pointer = match authority {
      ActorWaitingAuthority::Trigger => {
        if !matches!(key, WakeupKey::Tick(_))
          || cell.hot.cycle_state != CycleState::Idle
          || cell.hot.pending_signal
          || cell.eligible_at.is_some()
          || cell.hot.wakeup_pointer.is_some()
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        cell
          .hot
          .trigger_wakeup_pointer
          .map(|pointer| WakeupPointer {
            block: WakeupKey::Tick(pointer.tick),
            page_id: pointer.page_id,
            slot: pointer.slot,
          })
      }
      ActorWaitingAuthority::Service => {
        let terminal_idle = cell.hot.cycle_state == CycleState::Idle
          && !cell.hot.pending_signal
          && cell
            .hot
            .terminal_at
            .is_some_and(|terminal| cell.eligible_at.is_some_and(|at| at >= terminal));
        if !matches!(key, WakeupKey::Block(_))
          || cell.eligible_at.is_none()
          || !(matches!(
            cell.hot.cycle_state,
            CycleState::Running | CycleState::Suspended
          ) || (cell.hot.cycle_state == CycleState::Idle && cell.hot.pending_signal)
            || terminal_idle)
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        cell.hot.wakeup_pointer
      }
    };
    if let Some(pointer) = pointer {
      if pointer.block != key {
        return Err(ActorControlTransitionError::Invariant);
      }
      if let Some(mut page) = ActorWaitingFrameChunks::<T>::get((key, pointer.page_id)) {
        if let Some(Some(ActorWaitingEntry::Reference(reference))) =
          page.entries.get(pointer.slot as usize)
        {
          if reference.actor_id != actor_id
            || reference.admission_identity != cell.admission.admission_identity
          {
            return Err(ActorControlTransitionError::Invariant);
          }
          let slot =
            u8::try_from(pointer.slot).map_err(|_| ActorControlTransitionError::Invariant)?;
          page.entries[pointer.slot as usize] = Some(ActorWaitingEntry::Primary(cell));
          let location = ActorControlLocation::Waiting {
            key,
            page: pointer.page_id,
            slot,
          };
          ActorWaitingFrameChunks::<T>::insert((key, pointer.page_id), page);
          ActorControlLocators::<T>::insert(actor_id, location);
          return Ok(location);
        }
        if page
          .entries
          .get(pointer.slot as usize)
          .is_none_or(Option::is_some)
        {
          return Err(ActorControlTransitionError::Invariant);
        }
      }
    }
    let (page, slot) = Self::append_waiting_entry(key, |page_id, slot| {
      match key {
        WakeupKey::Block(_) => {
          cell.hot.wakeup_pointer = Some(WakeupPointer {
            block: key,
            page_id,
            slot,
          })
        }
        WakeupKey::Tick(tick) => {
          cell.hot.trigger_wakeup_pointer = Some(TriggerWakeupPointer {
            tick,
            page_id,
            slot,
          })
        }
      }
      ActorWaitingEntry::Primary(cell)
    })?;
    let slot = u8::try_from(slot).map_err(|_| ActorControlTransitionError::Invariant)?;
    let location = ActorControlLocation::Waiting { key, page, slot };
    ActorControlLocators::<T>::insert(actor_id, location);
    Ok(location)
  }

  pub(crate) fn control_append_ready(
    cell: ActorControlCellOf<T>,
  ) -> Result<(ActorId, QueueTicket), ActorControlTransitionError> {
    const CHUNK_SIZE: QueueTicket = 32;
    if cell.eligible_at.is_none() {
      return Err(ActorControlTransitionError::Invariant);
    }
    let actor_id = cell.actor_id;
    let tail = ActorReadyTail::<T>::get();
    let occupancy = ActorReadyOccupancy::<T>::get();
    if tail
      .checked_sub(ActorReadyHead::<T>::get())
      .is_none_or(|span| span >= u64::from(T::MaxQueueLength::get()))
      || ActorControlLocators::<T>::contains_key(actor_id)
    {
      return Err(ActorControlTransitionError::Invariant);
    }
    let next_tail = tail
      .checked_add(1)
      .ok_or(ActorControlTransitionError::IndexExhausted)?;
    let next_occupancy = occupancy
      .checked_add(1)
      .filter(|value| *value <= T::MaxActiveActors::get())
      .ok_or(ActorControlTransitionError::IndexExhausted)?;
    if Self::project_control_cell(&cell, ActorControlLocation::Ready { ticket: tail }).is_none() {
      return Err(ActorControlTransitionError::Invariant);
    }
    let page = tail / CHUNK_SIZE;
    let slot = (tail % CHUNK_SIZE) as usize;
    let mut chunk = match ActorReadyFrameChunks::<T>::get(page) {
      Some(chunk) => chunk,
      None => Self::control_blank_control_chunk()?,
    };
    let Some(target) = chunk.get_mut(slot) else {
      return Err(ActorControlTransitionError::Invariant);
    };
    if target.replace(cell).is_some() {
      return Err(ActorControlTransitionError::Invariant);
    }
    ActorReadyFrameChunks::<T>::insert(page, chunk);
    ActorReadyTail::<T>::put(next_tail);
    ActorReadyOccupancy::<T>::put(next_occupancy);
    ActorControlLocators::<T>::insert(actor_id, ActorControlLocation::Ready { ticket: tail });
    Ok((actor_id, tail))
  }

  #[cfg(all(test, feature = "runtime-benchmarks"))]
  pub(crate) fn control_remove_ready_primary(
    actor_id: ActorId,
  ) -> Result<QueueTicket, ActorControlTransitionError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let remove = || {
        let ActorControlLocation::Ready { ticket } =
          ActorControlLocators::<T>::get(actor_id).ok_or(ActorControlTransitionError::Invariant)?
        else {
          return Err(ActorControlTransitionError::Invariant);
        };
        let page = ticket / 32;
        let slot = (ticket % 32) as usize;
        let mut chunk =
          ActorReadyFrameChunks::<T>::get(page).ok_or(ActorControlTransitionError::Invariant)?;
        let stored = chunk
          .get_mut(slot)
          .ok_or(ActorControlTransitionError::Invariant)?;
        if stored.as_ref().map(|cell| cell.actor_id) != Some(actor_id) {
          return Err(ActorControlTransitionError::Invariant);
        }
        *stored = None;
        let occupancy = ActorReadyOccupancy::<T>::get()
          .checked_sub(1)
          .ok_or(ActorControlTransitionError::Invariant)?;
        ActorReadyFrameChunks::<T>::insert(page, chunk);
        ActorReadyOccupancy::<T>::put(occupancy);
        ActorControlLocators::<T>::remove(actor_id);
        Ok(ticket)
      };
      match remove() {
        Ok(ticket) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(ticket)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  #[cfg(all(test, feature = "runtime-benchmarks"))]
  pub(crate) fn control_normalize_ready_head(
    cutoff: QueueTicket,
    max_scans: u32,
  ) -> Result<(u32, Option<QueueTicket>), ActorControlTransitionError> {
    const CHUNK_SIZE: QueueTicket = 32;
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let normalize = || {
        let initial_head = ActorReadyHead::<T>::get();
        let tail = ActorReadyTail::<T>::get();
        let limit = cutoff.min(tail);
        let mut head = initial_head;
        let mut scans = 0u32;
        while head < limit && scans < max_scans {
          let page = head / CHUNK_SIZE;
          let slot = (head % CHUNK_SIZE) as usize;
          let chunk =
            ActorReadyFrameChunks::<T>::get(page).ok_or(ActorControlTransitionError::Invariant)?;
          let cell = chunk
            .get(slot)
            .ok_or(ActorControlTransitionError::Invariant)?;
          if cell.is_some() {
            break;
          }
          head = head
            .checked_add(1)
            .ok_or(ActorControlTransitionError::IndexExhausted)?;
          scans = scans.saturating_add(1);
          if head.is_multiple_of(CHUNK_SIZE) || head == tail {
            if chunk
              .iter(/* deos-bypass: bounded-iter */)
              .any(Option::is_some)
            {
              return Err(ActorControlTransitionError::Invariant);
            }
            ActorReadyFrameChunks::<T>::remove(page);
          }
        }
        if head != initial_head {
          ActorReadyHead::<T>::put(head);
        }
        let next_live = if head < limit {
          let page = head / CHUNK_SIZE;
          let slot = (head % CHUNK_SIZE) as usize;
          ActorReadyFrameChunks::<T>::get(page)
            .and_then(|chunk| chunk.get(slot).cloned().flatten())
            .map(|_| head)
        } else {
          None
        };
        Ok((scans, next_live))
      };
      match normalize() {
        Ok(result) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(result)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn control_stage_unsignaled_temporal(
    actor_id: ActorId,
    due_tick: SchedulerTick,
  ) -> Result<ActorControlLocation<BlockNumberFor<T>>, ActorControlTransitionError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let transition = || {
        if ActorControlLocators::<T>::get(actor_id) != Some(ActorControlLocation::Unsignaled) {
          return Err(ActorControlTransitionError::Invariant);
        }
        let cell = ActorUnsignaledControlCells::<T>::get(actor_id)
          .ok_or(ActorControlTransitionError::Invariant)?;
        if Self::project_control_cell(&cell, ActorControlLocation::Unsignaled).is_none() {
          return Err(ActorControlTransitionError::Invariant);
        }
        Self::remove_primary_control_cell_inner(actor_id)
          .map_err(|_| ActorControlTransitionError::Invariant)?;
        let location = Self::control_append_waiting(
          cell,
          WakeupKey::Tick(due_tick),
          ActorWaitingAuthority::Trigger,
        )?;
        Ok(location)
      };
      match transition() {
        Ok(location) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(location))
        }
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn control_latch_temporal_waiting_page(
    due_tick: SchedulerTick,
    page: u64,
    now: BlockNumberFor<T>,
    now_tick: SchedulerTick,
  ) -> Result<Vec<ActorId>, ActorControlTransitionError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let transition = || {
        const CHUNK_SIZE: u64 = 32;
        let source_key = WakeupKey::Tick(due_tick);
        if due_tick > now_tick
          || Self::wakeup_cursor_peek_key(WakeupClock::Tick) != Some(source_key)
          || ActorWaitingHeads::<T>::get(source_key) / CHUNK_SIZE != page
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        let mut chunk = ActorWaitingFrameChunks::<T>::get((source_key, page))
          .ok_or(ActorControlTransitionError::Invariant)?;
        let eligible_at = now
          .checked_add(&One::one())
          .ok_or(ActorControlTransitionError::IndexExhausted)?;
        let destination_key = WakeupKey::Block(eligible_at);
        let mut moved = Vec::new();
        for (slot, maybe_cell) in chunk
          .entries
          .iter_mut(/* deos-bypass: bounded-iter */)
          .enumerate()
        {
          let Some(entry) = maybe_cell.take() else {
            continue;
          };
          let mut cell = entry
            .into_primary()
            .ok_or(ActorControlTransitionError::Invariant)?;
          let slot = u8::try_from(slot).map_err(|_| ActorControlTransitionError::Invariant)?;
          let location = ActorControlLocation::Waiting {
            key: source_key,
            page,
            slot,
          };
          if Self::project_control_cell(&cell, location).is_none()
            || cell.hot.wakeup_pointer.is_some()
          {
            return Err(ActorControlTransitionError::Invariant);
          }
          cell.hot.trigger_wakeup_pointer = None;
          cell.hot.pending_signal = true;
          cell.eligible_at = Some(eligible_at);
          let actor_id = cell.actor_id;
          Self::remove_primary_control_cell_inner(actor_id)?;
          Self::control_append_waiting(cell, destination_key, ActorWaitingAuthority::Service)?;
          moved.push(actor_id);
        }
        Ok(moved)
      };
      match transition() {
        Ok(moved) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(moved)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn control_promote_due_waiting_page(
    eligible_at: BlockNumberFor<T>,
    page: u64,
    now: BlockNumberFor<T>,
  ) -> Result<Vec<(ActorId, QueueTicket)>, ActorControlTransitionError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let transition = || {
        const CHUNK_SIZE: u64 = 32;
        if eligible_at > now {
          return Err(ActorControlTransitionError::Invariant);
        }
        let source_key = WakeupKey::Block(eligible_at);
        if Self::wakeup_cursor_peek_key(WakeupClock::Block) != Some(source_key)
          || ActorWaitingHeads::<T>::get(source_key) / CHUNK_SIZE != page
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        let mut chunk = ActorWaitingFrameChunks::<T>::get((source_key, page))
          .ok_or(ActorControlTransitionError::Invariant)?;
        let mut moved = Vec::new();
        for (slot, maybe_cell) in chunk
          .entries
          .iter_mut(/* deos-bypass: bounded-iter */)
          .enumerate()
        {
          let Some(entry) = maybe_cell.take() else {
            continue;
          };
          let mut cell = entry
            .into_primary()
            .ok_or(ActorControlTransitionError::Invariant)?;
          let slot = u8::try_from(slot).map_err(|_| ActorControlTransitionError::Invariant)?;
          let location = ActorControlLocation::Waiting {
            key: source_key,
            page,
            slot,
          };
          if Self::project_control_cell(&cell, location).is_none()
            || cell.eligible_at.is_none_or(|value| value > now)
            || cell.hot.wakeup_pointer.is_none()
          {
            return Err(ActorControlTransitionError::Invariant);
          }
          cell.hot.wakeup_pointer = None;
          Self::remove_primary_control_cell_inner(cell.actor_id)?;
          moved.push(Self::control_append_ready(cell)?);
        }
        Ok(moved)
      };
      match transition() {
        Ok(moved) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(moved)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  #[cfg(all(test, feature = "runtime-benchmarks"))]
  pub(crate) fn control_service_next_due_waiting_unit(
    now: BlockNumberFor<T>,
    now_tick: SchedulerTick,
  ) -> Result<Option<(WakeupClock, u32)>, ActorControlTransitionError> {
    let preferred = NextWakeupClock::<T>::get();
    let peer = match preferred {
      WakeupClock::Block => WakeupClock::Tick,
      WakeupClock::Tick => WakeupClock::Block,
    };
    let due_key = |clock| {
      Self::wakeup_cursor_peek_key(clock).filter(|key| match key {
        WakeupKey::Block(block) => *block <= now,
        WakeupKey::Tick(tick) => *tick <= now_tick,
      })
    };
    let selected = due_key(preferred)
      .map(|key| (preferred, key))
      .or_else(|| due_key(peer).map(|key| (peer, key)));
    let Some((clock, key)) = selected else {
      return Ok(None);
    };
    let page = ActorWaitingHeads::<T>::get(key) / 32;
    let moved = match key {
      WakeupKey::Tick(due_tick) => u32::try_from(
        Self::control_latch_temporal_waiting_page(due_tick, page, now, now_tick)?.len(),
      )
      .map_err(|_| ActorControlTransitionError::Invariant)?,
      WakeupKey::Block(eligible_at) => {
        u32::try_from(Self::control_promote_due_waiting_page(eligible_at, page, now)?.len())
          .map_err(|_| ActorControlTransitionError::Invariant)?
      }
    };
    if moved == 0 {
      return Err(ActorControlTransitionError::Invariant);
    }
    NextWakeupClock::<T>::put(peer);
    Ok(Some((clock, moved)))
  }

  fn step_simulation_evidence(
    cycle_nonce: u64,
    start_cursor: u32,
    status: AttemptDisposition,
    cumulative_outcomes: OutcomeTotals,
    run: Option<&ActorRunStateOf<T>>,
    outcome: Option<StepOutcome>,
  ) -> ActorAttemptEvidence {
    ActorAttemptEvidence {
      status,
      cycle_nonce,
      start_cursor,
      run_cursor: run.map(|run| run.cursor),
      unsuccessful_attempts_at_cursor: run.map(|run| run.unsuccessful_attempts_at_cursor),
      cumulative_outcomes,
      step: outcome.map(|outcome| SimulationStepRecord {
        step_index: start_cursor,
        outcome,
      }),
    }
  }

  fn simulation_placement_error(error: EnqueueOutcome) -> SimulationError {
    match error {
      EnqueueOutcome::CapacityUnavailable | EnqueueOutcome::WakeupCapacityExhausted => {
        SimulationError::ResourceDeferred
      }
      EnqueueOutcome::TicketExhausted
      | EnqueueOutcome::SchedulerIndexExhausted
      | EnqueueOutcome::WakeupIndexExhausted => {
        SimulationError::Classification(ActorClassificationError::ComputationOverflow)
      }
      _ => SimulationError::Classification(ActorClassificationError::ActorInvariant),
    }
  }

  /// Called only within the simulation's outer rollback. Selects one real primary without
  /// advancing the FIFO Head or servicing any other Actor.
  pub(crate) fn simulate_actor_service(
    actor_id: ActorId,
    budget: SimulationBudget,
  ) -> Result<SimulationResult, SimulationError> {
    let limits = budget
      .checked_limits()
      .map_err(|_| SimulationError::InvalidBudget)?;
    let now = frame_system::Pallet::<T>::block_number();
    let mut resources = BlockResourceState::new(now);
    resources
      .begin_prepass()
      .map_err(|_| SimulationError::InvalidBudget)?;
    resources
      .open_external_phase()
      .map_err(|_| SimulationError::InvalidBudget)?;
    resources
      .begin_drain()
      .map_err(|_| SimulationError::InvalidBudget)?;
    let mut reservation = resources
      .reserve(
        limits,
        BlockResourceDomain::ActorControl,
        budget.actor_control,
      )
      .map_err(|_| SimulationError::ResourceDeferred)?;
    let mut control_meter = WeightMeter::with_limit(budget.actor_control);
    let mut cycle_meter = WeightMeter::with_limit(
      budget
        .actor_control
        .checked_add(&budget.shared_economic)
        .ok_or(SimulationError::InvalidBudget)?,
    );
    let (location, cell) = Self::load_primary_control_cell(actor_id)
      .map_err(|_| SimulationError::Classification(ActorClassificationError::ActorInvariant))?;
    let (terminal_state, terminal_admission, _) = Self::load_frame_actor_service_state(actor_id)
      .ok_or(SimulationError::Classification(
        ActorClassificationError::ActorInvariant,
      ))?;
    let terminal_instance = Self::derive_active_actor_view(
      terminal_state.identity.clone(),
      terminal_state.hot.clone(),
      terminal_state.contract.clone(),
    );
    let terminal_classification =
      Self::classify_actor_loaded(&terminal_instance, terminal_state.run_state.as_ref())
        .map_err(SimulationError::Classification)?;
    if terminal_classification.terminal_reason.is_some() {
      let probe = Self::scheduler_actor_state_probe_weight_upper();
      let consume = T::WeightInfo::scheduler_paged_consume_preserve_page()
        .max(T::WeightInfo::scheduler_paged_consume_delete_page());
      if !control_meter.can_consume(probe.saturating_add(consume))
        || !cycle_meter.can_consume(probe.saturating_add(consume))
      {
        return Err(SimulationError::ResourceDeferred);
      }
      control_meter.consume(probe);
      cycle_meter.consume(probe);
      let decision = Self::apply_admission_loaded(
        actor_id,
        &terminal_instance,
        terminal_state.run_state.as_ref(),
        None,
        None,
        &cycle_meter,
      );
      let AdmissionDecision::Close { reason, weight } = decision else {
        return Err(match decision {
          AdmissionDecision::Defer => SimulationError::ResourceDeferred,
          _ => SimulationError::Classification(ActorClassificationError::ActorInvariant),
        });
      };
      let source = match location {
        ActorControlLocation::Ready { ticket } => {
          Some((ActorServiceSource::SelectedReady, ticket, ticket))
        }
        _ => None,
      };
      let result = Self::service_admitted_close(
        actor_id,
        terminal_state,
        terminal_admission,
        reason,
        weight,
        consume,
        &mut cycle_meter,
        Some(&mut control_meter),
        source,
      );
      resources
        .settle(&mut reservation, cycle_meter.consumed())
        .map_err(|_| SimulationError::Classification(ActorClassificationError::ActorInvariant))?;
      return Self::simulation_service_result(result);
    }
    if let ActorControlLocation::Waiting { key, page, slot } = location {
      let WakeupKey::Block(due) = key else {
        return Err(SimulationError::NotReady);
      };
      if due > now
        || cell.hot.wakeup_pointer
          != Some(WakeupPointer {
            block: key,
            page_id: page,
            slot: u32::from(slot),
          })
      {
        return Err(SimulationError::Classification(
          ActorClassificationError::ActorInvariant,
        ));
      }
      let preparation = Self::wakeup_cursor_drain_unit_weight_for(
        WakeupBucketDisposition::Retain,
        WakeupClock::Block,
      )
      .max(Self::wakeup_cursor_drain_unit_weight_for(
        WakeupBucketDisposition::Remove,
        WakeupClock::Block,
      ));
      if !control_meter.can_consume(preparation) || !cycle_meter.can_consume(preparation) {
        return Err(SimulationError::ResourceDeferred);
      }
      control_meter.consume(preparation);
      cycle_meter.consume(preparation);
      let (mut state, admission, loaded_step) = Self::load_frame_actor_service_state(actor_id)
        .ok_or(SimulationError::Classification(
          ActorClassificationError::ActorInvariant,
        ))?;
      Self::remove_primary_control_cell_inner(actor_id)
        .map_err(|_| SimulationError::Classification(ActorClassificationError::ActorInvariant))?;
      state.hot.wakeup_pointer = None;
      let step_resources = if state.contract.steps.is_empty() {
        ActorStepResourceEnvelope {
          control: T::WeightInfo::scheduler_inner_zero_step_complete(),
          effect: Weight::zero(),
        }
      } else {
        loaded_step
          .ok_or(SimulationError::Classification(
            ActorClassificationError::ActorInvariant,
          ))?
          .resources
      };
      let plan = Self::preflight_paged_enqueue_authority(
        actor_id,
        state.hot,
        &state.identity,
        state.run_state.as_ref(),
        &admission,
        step_resources,
      )
      .map_err(Self::simulation_placement_error)?;
      Self::commit_paged_enqueue(plan).map_err(Self::simulation_placement_error)?;
    }
    let (location, cell) = Self::load_primary_control_cell(actor_id)
      .map_err(|_| SimulationError::Classification(ActorClassificationError::ActorInvariant))?;
    let ActorControlLocation::Ready { ticket } = location else {
      return Err(SimulationError::NotReady);
    };
    let eligible_at = cell.eligible_at.ok_or(SimulationError::Classification(
      ActorClassificationError::ActorInvariant,
    ))?;
    if eligible_at > now {
      return Err(SimulationError::NotReady);
    }
    let entry = ActorStepTicket {
      actor_id,
      cycle_nonce: cell.identity.cycle_nonce.saturating_add(1),
      cursor: cell.cursor,
      ticket,
      eligible_at,
      contract_commitment: ActorContractCommitment {
        semantic_contract_id: cell.admission.semantic_contract_id,
        body_commitment: cell.admission.body_commitment,
      },
    };
    let mut effect_consumed = Weight::zero();
    let mut uncertain = false;
    let result = Self::service_live_queue_entry(
      (ticket, entry),
      now,
      &mut cycle_meter,
      Some(&mut control_meter),
      &mut effect_consumed,
      &mut uncertain,
      Some((
        &mut resources,
        limits,
        BlockResourceDomain::ActorDrainEffect,
      )),
      ActorServiceSource::SelectedReady,
    );
    let actual_control = if uncertain {
      budget.actor_control
    } else {
      cycle_meter.consumed().checked_sub(&effect_consumed).ok_or(
        SimulationError::Classification(ActorClassificationError::ActorInvariant),
      )?
    };
    resources
      .settle(&mut reservation, actual_control)
      .map_err(|_| SimulationError::Classification(ActorClassificationError::ActorInvariant))?;
    Self::simulation_service_result(result)
  }

  fn simulation_service_result(
    result: FifoStepResult,
  ) -> Result<SimulationResult, SimulationError> {
    match result {
      FifoStepResult::Progress {
        attempt: Some(attempt),
        ..
      } => Ok(SimulationResult {
        status: attempt.status,
        cycle_nonce: attempt.cycle_nonce,
        start_cursor: attempt.start_cursor,
        run_cursor: attempt.run_cursor,
        unsuccessful_attempts_at_cursor: attempt.unsuccessful_attempts_at_cursor,
        cumulative_outcomes: attempt.cumulative_outcomes,
        steps: BoundedVec::truncate_from(attempt.step.into_iter().collect()),
      }),
      FifoStepResult::Blocked(BlockKind::Weight) => Err(SimulationError::ResourceDeferred),
      FifoStepResult::Blocked(BlockKind::FeeCollection) => {
        Err(SimulationError::FeeCollectionFailed)
      }
      FifoStepResult::Blocked(BlockKind::NonWeight) => Err(SimulationError::Classification(
        ActorClassificationError::ActorInvariant,
      )),
      _ => Err(SimulationError::NotReady),
    }
  }

  fn consume_actor_service_source(
    source: ActorServiceSource,
    position: QueueTicket,
    actor_id: ActorId,
    ticket: QueueTicket,
    hot: ActorHotStateOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    match source {
      ActorServiceSource::FifoHead => {
        Self::paged_consume_loaded_head_at(position, actor_id, ticket, hot)
      }
      ActorServiceSource::SelectedReady => {
        if hot.queue_ticket != Some(ticket) {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        Self::queue_topology_preflight(QueueMutation::Head)?;
        Self::consume_ready_primary(actor_id, ticket)
      }
    }
  }

  fn service_admitted_close(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    admission: ActorAdmissionCertificateOf<T>,
    reason: CloseReason,
    weight: Weight,
    consume_weight: Weight,
    cycle_meter: &mut WeightMeter,
    mut control_meter: Option<&mut WeightMeter>,
    source: Option<(ActorServiceSource, QueueTicket, QueueTicket)>,
  ) -> FifoStepResult {
    let atomic_weight = consume_weight.saturating_add(weight);
    if !cycle_meter.can_consume(atomic_weight)
      || control_meter
        .as_ref()
        .is_some_and(|meter| !meter.can_consume(atomic_weight))
    {
      return FifoStepResult::Blocked(BlockKind::Weight);
    }
    let outcome = polkadot_sdk::frame_support::storage::with_transaction(|| {
      if matches!(source, Some((ActorServiceSource::SelectedReady, _, _)))
        && Self::queue_topology_preflight(QueueMutation::Head).is_err()
      {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          polkadot_sdk::sp_runtime::DispatchError::Other("scheduler queue topology corrupted"),
        ));
      }
      let close_result =
        Self::finalize_actor_from_retained_state(actor_id, state.clone(), &admission, reason);
      if let Err(error) = close_result {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
      }
      Self::apply_test_close_queue_corruption();
      let source_closed = match source {
        None => {
          if ActorControlLocators::<T>::contains_key(actor_id) {
            Err(EnqueueOutcome::CorruptedTopology)
          } else {
            Ok(())
          }
        }
        Some((ActorServiceSource::FifoHead, position, _)) => {
          Self::paged_consume_closed_head_at(position)
        }
        Some((ActorServiceSource::SelectedReady, _, ticket)) => {
          if Self::queue_topology_preflight(QueueMutation::Head).is_err()
            || ActorControlLocators::<T>::contains_key(actor_id)
            || ActorReadyFrameChunks::<T>::get(ticket / 32)
              .and_then(|chunk| chunk.get((ticket % 32) as usize).cloned().flatten())
              .is_some()
          {
            Err(EnqueueOutcome::CorruptedTopology)
          } else {
            Ok(())
          }
        }
      };
      if source_closed.is_err() {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          polkadot_sdk::sp_runtime::DispatchError::Other("scheduler queue head changed"),
        ));
      }
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
    });
    cycle_meter.consume(atomic_weight);
    if let Some(meter) = control_meter.as_deref_mut() {
      meter.consume(atomic_weight);
    }
    match outcome {
      Ok(()) => FifoStepResult::Progress {
        executed: false,
        attempt: Some(Self::step_simulation_evidence(
          state
            .run_state
            .as_ref()
            .map_or(state.identity.cycle_nonce, |run| run.cycle_nonce),
          state.run_state.as_ref().map_or(0, |run| run.cursor),
          AttemptDisposition::Closed(reason),
          state
            .run_state
            .as_ref()
            .map_or(OutcomeTotals::default(), |run| run.cumulative_outcomes),
          None,
          None,
        )),
      },
      Err(_) => FifoStepResult::Blocked(BlockKind::NonWeight),
    }
  }

  fn service_live_queue_entry(
    (position, entry): (QueueTicket, QueueEntry<BlockNumberFor<T>>),
    now: BlockNumberFor<T>,
    cycle_meter: &mut WeightMeter,
    mut control_meter: Option<&mut WeightMeter>,
    effect_consumed: &mut Weight,
    effect_reconciliation_uncertain: &mut bool,
    mut resources: Option<(
      &mut BlockResourceState<BlockNumberFor<T>>,
      BlockResourceLimits,
      BlockResourceDomain,
    )>,
    source: ActorServiceSource,
  ) -> FifoStepResult {
    let consume_weight = T::WeightInfo::scheduler_paged_consume_preserve_page()
      .max(T::WeightInfo::scheduler_paged_consume_delete_page());
    let state_probe_weight = Self::scheduler_actor_state_probe_weight_upper();
    let probe_and_consume = state_probe_weight.saturating_add(consume_weight);
    if !cycle_meter.can_consume(probe_and_consume)
      || control_meter
        .as_ref()
        .is_some_and(|meter| !meter.can_consume(probe_and_consume))
    {
      return FifoStepResult::Blocked(BlockKind::Weight);
    }
    cycle_meter.consume(state_probe_weight);
    if let Some(meter) = control_meter.as_deref_mut() {
      meter.consume(state_probe_weight);
    }
    let Ok((location, cell)) = Self::load_primary_control_cell(entry.actor_id) else {
      return FifoStepResult::Blocked(BlockKind::NonWeight);
    };
    let Some((identity, hot, admission)) = Self::project_control_cell(&cell, location) else {
      return FifoStepResult::Blocked(BlockKind::NonWeight);
    };
    if hot.queue_ticket != Some(entry.ticket) {
      return FifoStepResult::NoWork;
    }
    if cell.eligible_at != Some(entry.eligible_at) {
      return FifoStepResult::Blocked(BlockKind::NonWeight);
    }
    let inline_terminal_due = hot.terminal_at.is_some_and(|terminal| terminal <= now)
      || identity.cycle_nonce == u64::MAX
      || Self::failure_limit_reached(hot.unsuccessful_attempt_streak);
    if entry.eligible_at > now
      && !inline_terminal_due
      && !hot.lifecycle.is_paused()
      && !GlobalCircuitBreaker::<T>::get()
    {
      return FifoStepResult::Blocked(BlockKind::NonWeight);
    }
    let opening_head = if hot.cycle_state == CycleState::Idle && hot.pending_signal {
      let Some(head) = ActorContractHeads::<T>::get(entry.actor_id) else {
        return FifoStepResult::Blocked(BlockKind::NonWeight);
      };
      Some(head)
    } else {
      None
    };
    let ordinary_opening = opening_head.as_ref().is_some_and(|head| {
      head.header.step_count > 0
        && head
          .header
          .auto_close_at_cycle_nonce
          .is_none_or(|target| identity.cycle_nonce < target)
    });
    let mut pipeline_capacity = None;
    if (hot.cycle_state == CycleState::Running || ordinary_opening)
      && !inline_terminal_due
      && !hot.lifecycle.is_paused()
      && let Some(required_control) = consume_weight
        .checked_add(&cell.resources.control)
        .and_then(|weight| weight.checked_add(&Self::close_cleanup_weight_upper()))
      && let Some(required_attempt) = required_control.checked_add(&cell.resources.effect)
      && (!cycle_meter.can_consume(required_attempt)
        || control_meter
          .as_ref()
          .is_some_and(|meter| !meter.can_consume(required_control))
        || resources.as_ref().is_some_and(|(state, limits, domain)| {
          state.capacity_exceeded(*limits, *domain, cell.resources.effect)
        }))
      && !GlobalCircuitBreaker::<T>::get()
    {
      if hot.cycle_state == CycleState::Running {
        return FifoStepResult::Blocked(BlockKind::Weight);
      }
      pipeline_capacity = opening_head.as_ref().map(|head| {
        Self::pipeline_capacity_sufficient_with_envelope(
          identity.actor_class.actor_type(),
          &identity.sovereign_account,
          head.header.pipeline_machine_envelope,
        )
      });
      if pipeline_capacity
        .as_ref()
        .is_some_and(|capacity| matches!(capacity, Ok(true)))
      {
        return FifoStepResult::Blocked(BlockKind::Weight);
      }
    }
    let service_state = match opening_head {
      Some(head) => {
        Self::load_actor_service_state_with_head(entry.actor_id, identity, hot, admission, head)
      }
      None => Self::load_actor_service_state_with_control(entry.actor_id, identity, hot, admission),
    };
    let Some((state, admission, loaded_step)) = service_state else {
      return FifoStepResult::Blocked(BlockKind::NonWeight);
    };
    let current_ticket = loaded_step.as_ref().and_then(|_| {
      Self::build_actor_step_ticket(
        entry.actor_id,
        entry.ticket,
        entry.eligible_at,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
    });
    let current_plan = loaded_step.as_ref().and_then(|loaded_step| {
      let ticket = current_ticket?;
      let maximum_fee = Self::maximum_current_action_fee(
        state.identity.actor_class.actor_type(),
        &loaded_step.step,
        loaded_step.resources,
      )
      .ok()?;
      Self::build_current_step_plan(
        entry.actor_id,
        state.identity.clone(),
        state.hot.clone(),
        state.run_state.clone(),
        state.funding.clone(),
        admission.clone(),
        ticket,
        loaded_step.clone(),
        maximum_fee,
      )
    });
    if state
      .run_state
      .as_ref()
      .is_some_and(|run_state| run_state.last_committed_step_block == Some(now))
    {
      return FifoStepResult::Blocked(BlockKind::NonWeight);
    }
    if state.hot.cycle_state == CycleState::Suspended {
      if state
        .run_state
        .as_ref()
        .is_some_and(|run_state| run_state.last_attempt_block == now)
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
      let mut post_source_hot = queue_owner_hot.clone();
      post_source_hot.queue_ticket = None;
      let placement_resources = loaded_step.as_ref().map_or(
        ActorStepResourceEnvelope {
          control: T::WeightInfo::scheduler_inner_zero_step_complete(),
          effect: Weight::zero(),
        },
        |loaded| loaded.resources,
      );
      let outcome: DispatchResult = polkadot_sdk::frame_support::storage::with_transaction(|| {
        if Self::consume_actor_service_source(
          source,
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
        let restoration = Self::restore_unsignaled_from_authority(
          entry.actor_id,
          post_source_hot,
          &state.identity,
          state.run_state.as_ref(),
          &admission,
          placement_resources,
        );
        if restoration.is_err()
          || (Self::reconcile_actor_state_hold_with_authority(entry.actor_id).is_err())
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            polkadot_sdk::sp_runtime::DispatchError::Other("paused frame restoration failed"),
          ));
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      });
      if outcome.is_err() {
        return FifoStepResult::Blocked(BlockKind::NonWeight);
      }
      cycle_meter.consume(consume_weight);
      if let Some(meter) = control_meter.as_deref_mut() {
        meter.consume(consume_weight);
      }
      return FifoStepResult::Progress {
        executed: false,
        attempt: None,
      };
    }
    let actor_id = entry.actor_id;
    let loaded_run_state = state.run_state.clone();
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    match Self::apply_admission_loaded(
      actor_id,
      &instance,
      loaded_run_state.as_ref(),
      current_plan.as_ref(),
      pipeline_capacity,
      cycle_meter,
    ) {
      AdmissionDecision::Admit {
        weight,
        terminal_cleanup,
      } => {
        if loaded_step.is_none() {
          if !instance.steps.is_empty()
            || instance.cycle_state != CycleState::Idle
            || state.run_state.is_some()
          {
            return FifoStepResult::Blocked(BlockKind::NonWeight);
          }
          let attempt_weight = consume_weight.saturating_add(weight);
          if !cycle_meter.can_consume(attempt_weight)
            || control_meter
              .as_ref()
              .is_some_and(|meter| !meter.can_consume(attempt_weight))
          {
            return FifoStepResult::Blocked(BlockKind::Weight);
          }
          let outcome = polkadot_sdk::frame_support::storage::with_transaction(|| {
            if Self::consume_actor_service_source(
              source,
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
            let execution =
              Self::execute_zero_step_from_consumed_frame(actor_id, state.clone(), &admission, now);
            match execution {
              Ok(attempt) => {
                polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(attempt))
              }
              Err(error) => {
                polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
              }
            }
          });
          cycle_meter.consume(attempt_weight);
          if let Some(meter) = control_meter.as_deref_mut() {
            meter.consume(attempt_weight);
          }
          return match outcome {
            Ok(attempt) => FifoStepResult::Progress {
              executed: true,
              attempt: Some(attempt),
            },
            Err(AttemptTransactionError::FeeCollection) => {
              FifoStepResult::Blocked(BlockKind::FeeCollection)
            }
            Err(AttemptTransactionError::StateHold | AttemptTransactionError::Invariant) => {
              FifoStepResult::Blocked(BlockKind::NonWeight)
            }
          };
        }
        if current_ticket != Some(entry) || current_plan.is_none() {
          return FifoStepResult::Blocked(BlockKind::NonWeight);
        }
        let Some(commit_plan) = current_plan else {
          return FifoStepResult::Blocked(BlockKind::NonWeight);
        };
        let attempt_weight = consume_weight.saturating_add(weight);
        let exhaustion_close_weight = if terminal_cleanup.is_included() {
          Weight::zero()
        } else {
          Self::close_cleanup_weight_upper()
        };
        let reserved_control_weight = commit_plan.loaded_step.resources.control;
        let reserved_effect_weight = commit_plan.loaded_step.resources.effect;
        let Some(reserved_attempt_control) = attempt_weight.checked_sub(&reserved_effect_weight)
        else {
          return FifoStepResult::Blocked(BlockKind::NonWeight);
        };
        let reserved_control_with_cleanup =
          reserved_attempt_control.saturating_add(exhaustion_close_weight);
        if !cycle_meter.can_consume(attempt_weight.saturating_add(exhaustion_close_weight))
          || control_meter
            .as_ref()
            .is_some_and(|meter| !meter.can_consume(reserved_control_with_cleanup))
        {
          return FifoStepResult::Blocked(BlockKind::Weight);
        }
        let mut resource_reservation = match resources.as_mut() {
          Some((state, limits, effect_domain)) => {
            match state.reserve(*limits, *effect_domain, reserved_effect_weight) {
              Ok(reservation) => Some(reservation),
              Err(_) => return FifoStepResult::Blocked(BlockKind::Weight),
            }
          }
          None => None,
        };
        let outcome = polkadot_sdk::frame_support::storage::with_transaction(|| {
          if Self::consume_actor_service_source(
            source,
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
          let execution =
            Self::execute_current_step_and_place(actor_id, &state, commit_plan, &admission, now);
          match execution {
            Ok(evidence) => {
              if let (Some((state, _, _)), Some(reservation)) =
                (resources.as_mut(), resource_reservation.as_mut())
                && state
                  .settle(reservation, evidence.actual_effect_weight)
                  .is_err()
              {
                state.halt_optional_actor_work();
                return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                  AttemptTransactionError::Invariant,
                ));
              }
              polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(evidence))
            }
            Err(error) => {
              polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
            }
          }
        });
        if outcome.is_err()
          && let (Some(reservation), Some((state, _, _))) =
            (resource_reservation.as_mut(), resources.as_mut())
        {
          // The storage attempt rolled back its settlement together with the effect. Retain the
          // admitted maximum, but consume this reservation's transition authority outside that
          // transaction so external user work and final reconciliation remain reachable.
          let _ = state.settle(reservation, reserved_effect_weight);
          state.halt_optional_actor_work();
        }
        match outcome {
          Ok(StepCommitEvidence {
            closed_for_exhaustion,
            actual_control_weight,
            actual_effect_weight,
            attempt,
          }) => {
            let Some(non_step_reservation) = attempt_weight
              .checked_sub(&reserved_control_weight)
              .and_then(|weight| weight.checked_sub(&reserved_effect_weight))
            else {
              cycle_meter.consume(attempt_weight);
              if let Some(meter) = control_meter.as_deref_mut() {
                meter.consume(reserved_attempt_control);
              }
              return FifoStepResult::Blocked(BlockKind::NonWeight);
            };
            let actual_attempt_weight = non_step_reservation
              .saturating_add(actual_control_weight)
              .saturating_add(actual_effect_weight);
            cycle_meter.consume(actual_attempt_weight);
            let actual_attempt_control = actual_attempt_weight
              .checked_sub(&actual_effect_weight)
              .unwrap_or(reserved_attempt_control);
            if let Some(meter) = control_meter.as_deref_mut() {
              meter.consume(actual_attempt_control);
            }
            effect_consumed.saturating_accrue(actual_effect_weight);
            if closed_for_exhaustion {
              cycle_meter.consume(exhaustion_close_weight);
              if let Some(meter) = control_meter.as_deref_mut() {
                meter.consume(exhaustion_close_weight);
              }
            }
            FifoStepResult::Progress {
              executed: true,
              attempt: Some(attempt),
            }
          }
          Err(AttemptTransactionError::FeeCollection) => {
            *effect_reconciliation_uncertain |= !reserved_effect_weight.is_zero();
            cycle_meter.consume(attempt_weight);
            if let Some(meter) = control_meter.as_deref_mut() {
              meter.consume(reserved_attempt_control);
            }
            FifoStepResult::Blocked(BlockKind::FeeCollection)
          }
          Err(AttemptTransactionError::StateHold | AttemptTransactionError::Invariant) => {
            *effect_reconciliation_uncertain |= !reserved_effect_weight.is_zero();
            cycle_meter.consume(attempt_weight);
            if let Some(meter) = control_meter.as_deref_mut() {
              meter.consume(reserved_attempt_control);
            }
            FifoStepResult::Blocked(BlockKind::NonWeight)
          }
        }
      }
      AdmissionDecision::Close { reason, weight } => Self::service_admitted_close(
        actor_id,
        state,
        admission,
        reason,
        weight,
        consume_weight,
        cycle_meter,
        control_meter,
        Some((source, position, entry.ticket)),
      ),
      AdmissionDecision::Defer => FifoStepResult::Blocked(BlockKind::Weight),
      AdmissionDecision::Invariant => FifoStepResult::Blocked(BlockKind::NonWeight),
      AdmissionDecision::Skip => {
        let exhaustion_close_weight = Self::close_cleanup_weight_upper();
        let skip_maximum = consume_weight.saturating_add(exhaustion_close_weight);
        if !cycle_meter.can_consume(skip_maximum)
          || control_meter
            .as_ref()
            .is_some_and(|meter| !meter.can_consume(skip_maximum))
        {
          return FifoStepResult::Blocked(BlockKind::Weight);
        }
        let mut post_source_instance = instance;
        post_source_instance.queue_ticket = None;
        let mut post_source_hot = queue_owner_hot.clone();
        post_source_hot.queue_ticket = None;
        let unsignaled_hot = post_source_hot.clone();
        let placement_identity = ActorIdentity {
          sovereign_account: post_source_instance.sovereign_account.clone(),
          owner: post_source_instance.owner.clone(),
          actor_class: post_source_instance.actor_class,
          mutability: post_source_instance.mutability,
          cycle_nonce: post_source_instance.cycle_nonce,
          last_control_mutation_block: post_source_instance.last_control_mutation_block,
        };
        let placement_resources = loaded_step.as_ref().map_or(
          ActorStepResourceEnvelope {
            control: T::WeightInfo::scheduler_inner_zero_step_complete(),
            effect: Weight::zero(),
          },
          |loaded| loaded.resources,
        );
        let outcome = polkadot_sdk::frame_support::storage::with_transaction(|| {
          if Self::consume_actor_service_source(
            source,
            position,
            actor_id,
            entry.ticket,
            queue_owner_hot,
          )
          .is_err()
          {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
              polkadot_sdk::sp_runtime::DispatchError::Other("scheduler queue topology changed"),
            ));
          }
          match Self::schedule_next_work_with_authority(
            actor_id,
            &post_source_instance,
            post_source_hot,
            &placement_identity,
            loaded_run_state.as_ref(),
            &admission,
            placement_resources,
            now,
            ServiceCutoff::Snapshotted,
          ) {
            Ok(StepControlPlacement::None) => {
              if Self::restore_unsignaled_from_authority(
                actor_id,
                unsignaled_hot,
                &placement_identity,
                loaded_run_state.as_ref(),
                &admission,
                placement_resources,
              )
              .is_err()
              {
                return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                  polkadot_sdk::sp_runtime::DispatchError::Other(
                    "post-skip frame restoration failed",
                  ),
                ));
              }
            }
            Ok(_) => {}
            Err(error) => {
              let close_result = Self::finalize_actor_from_consumed_state(
                actor_id,
                state.clone(),
                &admission,
                CloseReason::SchedulerIndexExhausted,
              );
              if !Self::scheduler_index_is_exhausted(error) || close_result.is_err() {
                return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                  polkadot_sdk::sp_runtime::DispatchError::Other("post-skip placement failed"),
                ));
              }
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(true));
            }
          }
          if Self::reconcile_actor_state_hold_with_authority(actor_id).is_err() {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
              polkadot_sdk::sp_runtime::DispatchError::Other(
                "post-skip hold reconciliation failed",
              ),
            ));
          }
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(false))
        });
        cycle_meter.consume(consume_weight);
        if let Some(meter) = control_meter.as_deref_mut() {
          meter.consume(consume_weight);
        }
        match outcome {
          Ok(closed_for_exhaustion) => {
            if closed_for_exhaustion {
              cycle_meter.consume(exhaustion_close_weight);
              if let Some(meter) = control_meter.as_deref_mut() {
                meter.consume(exhaustion_close_weight);
              }
            }
            FifoStepResult::Progress {
              executed: false,
              attempt: None,
            }
          }
          Err(_) => FifoStepResult::Blocked(BlockKind::NonWeight),
        }
      }
    }
  }

  #[cfg(test)]
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
        Self::defer_retained_wakeup(actor_id, next_block)
      }
      Err(other) => Err(other),
    }
  }

  fn enqueue_authority_loaded(
    actor_id: ActorId,
    hot: ActorHotStateOf<T>,
    identity: &ActorIdentityOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
  ) -> Result<(), EnqueueOutcome> {
    let result = with_transaction_opaque_err(|| {
      match Self::preflight_paged_enqueue_authority(
        actor_id, hot, identity, run_state, admission, resources,
      ) {
        Ok(plan) => match Self::commit_paged_enqueue(plan) {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        },
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
    match result {
      Ok(()) | Err(EnqueueOutcome::AlreadyLive) => Ok(()),
      Err(other) => Err(other),
    }
  }

  fn enqueue_actor_state_loaded(
    actor_id: ActorId,
    state: &ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    loaded_step: Option<&LoadedActorStepOf<T>>,
  ) -> Result<(), EnqueueOutcome> {
    let result = with_transaction_opaque_err(|| {
      match Self::preflight_paged_enqueue_actor_state(actor_id, state, admission, loaded_step) {
        Ok(plan) => match Self::commit_paged_enqueue(plan) {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        },
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
    match result {
      Ok(()) | Err(EnqueueOutcome::AlreadyLive) => Ok(()),
      Err(EnqueueOutcome::CapacityUnavailable) => {
        let next_block = frame_system::Pallet::<T>::block_number()
          .checked_add(&One::one())
          .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
        let instance = Self::derive_active_actor_view(
          state.identity.clone(),
          state.hot.clone(),
          state.contract.clone(),
        );
        {
          let resources = if state.contract.steps.is_empty() {
            ActorStepResourceEnvelope {
              control: T::WeightInfo::scheduler_inner_zero_step_complete(),
              effect: Weight::zero(),
            }
          } else {
            let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
            loaded_step
              .filter(|loaded| loaded.cursor == cursor)
              .map(|loaded| loaded.resources)
              .ok_or(EnqueueOutcome::CorruptedTopology)?
          };
          Self::defer_wakeup_with_authority(
            actor_id,
            next_block,
            &instance,
            state.hot.clone(),
            &state.identity,
            state.run_state.as_ref(),
            admission,
            resources,
          )
        }
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
        ActorReadyTail::<T>::put(
          ActorReadyHead::<T>::get().saturating_add(u64::from(T::MaxQueueLength::get()) + 1),
        );
      }
    });
  }

  #[cfg(not(test))]
  fn apply_test_close_queue_corruption() {}

  fn queue_topology_preflight(_mutation: QueueMutation) -> Result<QueueTopology, EnqueueOutcome> {
    let head = ActorReadyHead::<T>::get();
    let tail = ActorReadyTail::<T>::get();
    let occupancy = ActorReadyOccupancy::<T>::get();
    let span = tail
      .checked_sub(head)
      .ok_or(EnqueueOutcome::CorruptedTopology)?;
    if span > u64::from(T::MaxQueueLength::get())
      || u64::from(occupancy) > span
      || occupancy > T::MaxActiveActors::get()
    {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    if head < tail {
      let chunk =
        ActorReadyFrameChunks::<T>::get(head / 32).ok_or(EnqueueOutcome::CorruptedTopology)?;
      // ActorControlChunkOf has at most 32 slots; inspect only the consumed head prefix.
      if chunk.len() != 32
        || chunk.iter(/* deos-bypass: bounded-iter */).take((head % 32) as usize).any(Option::is_some)
      {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
    }
    if let Some(chunk) = ActorReadyFrameChunks::<T>::get(tail / 32) {
      // ActorControlChunkOf has at most 32 slots; inspect only the unused tail suffix.
      if chunk.len() != 32
        || chunk.iter(/* deos-bypass: bounded-iter */).skip((tail % 32) as usize).any(Option::is_some)
      {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
    } else if head < tail && !tail.is_multiple_of(32) {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    Ok(QueueTopology {
      head,
      tail,
      occupancy,
    })
  }

  pub fn combined_queue_occupancy() -> u64 {
    u64::from(ActorReadyOccupancy::<T>::get())
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  /// Appends one actor to the canonical FIFO using the global ticket allocator.
  pub fn paged_enqueue(actor_id: ActorId) -> bool {
    matches!(
      Self::try_paged_enqueue(actor_id),
      Ok(()) | Err(EnqueueOutcome::AlreadyLive)
    )
  }

  pub(crate) fn preflight_paged_enqueue_cohort_with_authority(
    actors: Vec<(ActorId, ActorHotStateOf<T>)>,
  ) -> Result<QueueAppendPlan<T>, EnqueueOutcome> {
    if actors.is_empty() || actors.len() > T::MaxCrossingActorsPerBlock::get() as usize {
      return Err(EnqueueOutcome::CapacityUnavailable);
    }
    let mut plan = Self::new_queue_append_plan()?;
    for (actor_id, hot) in actors.into_iter(/* deos-bypass: bounded-iter */) {
      let (state, admission, loaded_step) =
        Self::load_frame_actor_service_state(actor_id).ok_or(EnqueueOutcome::CorruptedTopology)?;
      let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
      let resources = if state.contract.steps.is_empty() {
        ActorStepResourceEnvelope {
          control: T::WeightInfo::scheduler_inner_zero_step_complete(),
          effect: Weight::zero(),
        }
      } else {
        loaded_step
          .filter(|loaded| loaded.cursor == cursor)
          .map(|loaded| loaded.resources)
          .ok_or(EnqueueOutcome::CorruptedTopology)?
      };
      Self::reserve_following_paged_enqueue_with_authority(
        &mut plan,
        actor_id,
        hot,
        &state.identity,
        state.run_state.as_ref(),
        &admission,
        resources,
      )?;
    }
    Ok(plan)
  }

  fn preflight_paged_enqueue_actor_state(
    actor_id: ActorId,
    state: &ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    loaded_step: Option<&LoadedActorStepOf<T>>,
  ) -> Result<QueueAppendPlan<T>, EnqueueOutcome> {
    let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
    let resources = if state.contract.steps.is_empty() {
      ActorStepResourceEnvelope {
        control: T::WeightInfo::scheduler_inner_zero_step_complete(),
        effect: Weight::zero(),
      }
    } else {
      loaded_step
        .filter(|loaded| loaded.cursor == cursor)
        .map(|loaded| loaded.resources)
        .ok_or(EnqueueOutcome::CorruptedTopology)?
    };
    Self::preflight_paged_enqueue_authority(
      actor_id,
      state.hot.clone(),
      &state.identity,
      state.run_state.as_ref(),
      admission,
      resources,
    )
  }

  fn new_queue_append_plan() -> Result<QueueAppendPlan<T>, EnqueueOutcome> {
    let topology = Self::queue_topology_preflight(QueueMutation::Enqueue)?;
    Ok(QueueAppendPlan {
      publications: Vec::new(),
      next_tail: topology.tail,
      next_occupancy: topology.occupancy,
    })
  }

  pub(crate) fn preflight_paged_enqueue_authority(
    actor_id: ActorId,
    hot: ActorHotStateOf<T>,
    identity: &ActorIdentityOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
  ) -> Result<QueueAppendPlan<T>, EnqueueOutcome> {
    let mut plan = Self::new_queue_append_plan()?;
    Self::reserve_following_paged_enqueue_with_authority(
      &mut plan, actor_id, hot, identity, run_state, admission, resources,
    )?;
    Ok(plan)
  }

  fn prepare_observation_ready_cell(
    state: &ObservationActivationState<T>,
    hot: &ActorHotStateOf<T>,
  ) -> Result<ActorControlCellOf<T>, EnqueueOutcome> {
    let (location, mut cell) = Self::load_primary_control_cell(state.actor_id)
      .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
    let expected_identity = Self::control_identity_from_scalar(state.identity.clone())
      .ok_or(EnqueueOutcome::CorruptedTopology)?;
    if matches!(location, ActorControlLocation::Ready { .. })
      || cell.identity != expected_identity
      || cell.hot != Self::control_hot_from_scalar(state.hot.clone())
    {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    cell.hot = Self::control_hot_from_scalar(hot.clone());
    cell.cursor = state.run_head.as_ref().map_or(0, |run| run.cursor);
    cell.eligible_at = Some(
      state
        .run_head
        .as_ref()
        .map_or_else(frame_system::Pallet::<T>::block_number, |run| {
          run.eligible_at
        }),
    );
    Ok(cell)
  }

  fn preflight_paged_enqueue_observation_loaded(
    state: &ObservationActivationState<T>,
    hot: ActorHotStateOf<T>,
  ) -> Result<QueueAppendPlan<T>, EnqueueOutcome> {
    let cell = Self::prepare_observation_ready_cell(state, &hot)?;
    let mut plan = Self::new_queue_append_plan()?;
    let mut planned_actors = alloc::collections::BTreeSet::new();
    Self::reserve_following_paged_enqueue_observation(
      &mut plan,
      &mut planned_actors,
      state,
      hot,
      cell,
    )?;
    Ok(plan)
  }

  fn reserve_following_paged_enqueue_observation(
    plan: &mut QueueAppendPlan<T>,
    planned_actors: &mut alloc::collections::BTreeSet<ActorId>,
    state: &ObservationActivationState<T>,
    hot: ActorHotStateOf<T>,
    cell: ActorControlCellOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    if hot.queue_ticket.is_some() {
      return Err(EnqueueOutcome::CapacityUnavailable);
    }
    Self::preflight_ready_publication_capacity(plan)?;
    match (hot.cycle_state, state.run_head.as_ref()) {
      (CycleState::Idle, None) => {
        state
          .identity
          .cycle_nonce
          .checked_add(1)
          .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
      }
      (CycleState::Running | CycleState::Suspended, Some(_)) => {}
      _ => return Err(EnqueueOutcome::CorruptedTopology),
    }
    if !planned_actors.insert(state.actor_id) {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    Self::reserve_ready_publication(plan, cell)
  }

  fn preflight_ready_publication_capacity(plan: &QueueAppendPlan<T>) -> Result<(), EnqueueOutcome> {
    if plan.next_tail.saturating_sub(ActorReadyHead::<T>::get())
      >= u64::from(T::MaxQueueLength::get())
      || plan.next_occupancy >= T::MaxActiveActors::get()
    {
      return Err(EnqueueOutcome::CapacityUnavailable);
    }
    plan
      .next_tail
      .checked_add(1)
      .ok_or(EnqueueOutcome::TicketExhausted)?;
    Ok(())
  }

  fn reserve_ready_publication(
    plan: &mut QueueAppendPlan<T>,
    cell: ActorControlCellOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    let ticket = plan.next_tail;
    let next_tail = ticket
      .checked_add(1)
      .ok_or(EnqueueOutcome::TicketExhausted)?;
    let next_occupancy = plan
      .next_occupancy
      .checked_add(1)
      .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
    plan
      .publications
      .push(PreparedReadyPublication { ticket, cell });
    plan.next_tail = next_tail;
    plan.next_occupancy = next_occupancy;
    Ok(())
  }

  fn reserve_following_paged_enqueue_with_authority(
    plan: &mut QueueAppendPlan<T>,
    actor_id: ActorId,
    mut hot: ActorHotStateOf<T>,
    identity: &ActorIdentityOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
  ) -> Result<(), EnqueueOutcome> {
    if plan.publications.len() >= T::MaxCrossingActorsPerBlock::get() as usize {
      return Err(EnqueueOutcome::CapacityUnavailable);
    }
    if hot.queue_ticket.is_some()
      || plan.publications.iter(/* deos-bypass: bounded-iter */)
        .any(|publication| publication.cell.actor_id == actor_id)
    {
      return Err(EnqueueOutcome::AlreadyLive);
    }
    Self::preflight_ready_publication_capacity(plan)?;
    let ticket = plan.next_tail;
    hot.queue_ticket = Some(ticket);
    let now = frame_system::Pallet::<T>::block_number();
    let eligible_at = match run_state {
      Some(run) => run.eligible_at,
      None if hot.last_cycle_block == Some(now) => now
        .checked_add(&One::one())
        .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?,
      None => now,
    };
    let step_ticket = Self::build_actor_step_ticket(
      actor_id,
      ticket,
      eligible_at,
      identity,
      &hot,
      run_state,
      admission,
    )
    .ok_or(EnqueueOutcome::CorruptedTopology)?;
    let cell = ActorControlCell {
      actor_id,
      identity: Self::control_identity_from_scalar(identity.clone())
        .ok_or(EnqueueOutcome::CorruptedTopology)?,
      hot: Self::control_hot_from_scalar(hot),
      cursor: step_ticket.cursor,
      eligible_at: Some(step_ticket.eligible_at),
      admission: admission.clone(),
      resources,
    };
    Self::reserve_ready_publication(plan, cell)
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  fn preflight_retained_paged_enqueue(
    actor_id: ActorId,
  ) -> Result<QueueAppendPlan<T>, EnqueueOutcome> {
    let Some((state, admission, loaded_step)) = Self::load_frame_actor_service_state(actor_id)
    else {
      return if Self::control_hot_exists(actor_id) {
        Err(EnqueueOutcome::CorruptedTopology)
      } else {
        Err(EnqueueOutcome::CapacityUnavailable)
      };
    };
    // Preserve the single-member cohort admission boundary before queue topology checks.
    if T::MaxCrossingActorsPerBlock::get() == 0 {
      return Err(EnqueueOutcome::CapacityUnavailable);
    }
    Self::preflight_paged_enqueue_actor_state(actor_id, &state, &admission, loaded_step.as_ref())
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
    let plan = Self::preflight_paged_enqueue_cohort_with_authority(vec![
      (first, first_hot),
      (second, second_hot),
    ])?;
    let first_ticket = plan.publications[0].ticket;
    let second_ticket = plan.publications[1].ticket;
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
    let plan = Self::preflight_paged_enqueue_cohort_with_authority(cohort)?;
    let mut tickets = [0; 4];
    for (index, publication) in plan
      .publications
      .iter(/* deos-bypass: bounded-iter */)
      .enumerate()
    {
      tickets[index] = publication.ticket;
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
    let plan = Self::preflight_paged_enqueue_cohort_with_authority(cohort)?;
    Self::commit_paged_enqueue(plan)
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
    Self::preflight_paged_enqueue_cohort_with_authority(cohort).map(|_| ())
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

  pub(crate) fn update_existing_frame_control_identity(
    actor_id: ActorId,
    identity: &ActorIdentityOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    let (location, mut cell) =
      Self::load_primary_control_cell(actor_id).map_err(|_| EnqueueOutcome::CorruptedTopology)?;
    cell.identity = Self::control_identity_from_scalar(identity.clone())
      .ok_or(EnqueueOutcome::CorruptedTopology)?;
    Self::store_primary_control_cell(location, cell).map_err(|_| EnqueueOutcome::CorruptedTopology)
  }

  pub(crate) fn update_existing_frame_control_hot(
    actor_id: ActorId,
    hot: &ActorHotStateOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    let (location, mut cell) =
      Self::load_primary_control_cell(actor_id).map_err(|_| EnqueueOutcome::CorruptedTopology)?;
    cell.hot = Self::control_hot_from_scalar(hot.clone());
    Self::store_primary_control_cell(location, cell).map_err(|_| EnqueueOutcome::CorruptedTopology)
  }

  pub(crate) fn restore_unsignaled_from_authority(
    actor_id: ActorId,
    hot: ActorHotStateOf<T>,
    identity: &ActorIdentityOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
  ) -> Result<(), EnqueueOutcome> {
    if ActorControlLocators::<T>::contains_key(actor_id) {
      Self::remove_primary_control_cell_inner(actor_id)
        .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
    }
    let cell = ActorControlCell {
      actor_id,
      identity: Self::control_identity_from_scalar(identity.clone())
        .ok_or(EnqueueOutcome::CorruptedTopology)?,
      hot: Self::control_hot_from_scalar(hot),
      cursor: run_state.map_or(0, |run| run.cursor),
      eligible_at: None,
      admission: admission.clone(),
      resources,
    };
    ActorUnsignaledControlCells::<T>::insert(actor_id, cell);
    ActorControlLocators::<T>::insert(actor_id, ActorControlLocation::Unsignaled);
    Ok(())
  }

  fn consume_ready_primary(actor_id: ActorId, ticket: QueueTicket) -> Result<(), EnqueueOutcome> {
    if ActorControlLocators::<T>::get(actor_id) != Some(ActorControlLocation::Ready { ticket }) {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    Self::remove_primary_control_cell_inner(actor_id)
      .map(|_| ())
      .map_err(|_| EnqueueOutcome::CorruptedTopology)
  }

  pub(crate) fn detach_primary_for_successor(
    actor_id: ActorId,
    successor_hot: &ActorHotStateOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    let (location, cell) =
      Self::load_primary_control_cell(actor_id).map_err(|_| EnqueueOutcome::CorruptedTopology)?;
    if let ActorControlLocation::Waiting { key, page, slot } = location {
      let pointer = WakeupPointer {
        block: key,
        page_id: page,
        slot: u32::from(slot),
      };
      if Self::wakeup_pointer_for_clock(successor_hot, key.clock()) == Some(pointer) {
        let mut stored = ActorWaitingFrameChunks::<T>::get((key, page))
          .ok_or(EnqueueOutcome::CorruptedTopology)?;
        let target = stored
          .entries
          .get_mut(slot as usize)
          .ok_or(EnqueueOutcome::CorruptedTopology)?;
        if target.as_ref().and_then(ActorWaitingEntry::primary) != Some(&cell) {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        *target = Some(ActorWaitingEntry::Reference(ActorWakeupReference {
          actor_id,
          admission_identity: cell.admission.admission_identity,
        }));
        ActorWaitingFrameChunks::<T>::insert((key, page), stored);
        ActorControlLocators::<T>::remove(actor_id);
        return Ok(());
      }
    }
    Self::remove_primary_control_cell_inner(actor_id)
      .map(|_| ())
      .map_err(|_| EnqueueOutcome::CorruptedTopology)
  }

  fn consume_waiting_from_supplied_authority(
    actor_id: ActorId,
    key: WakeupKey<BlockNumberFor<T>>,
    hot: &ActorHotStateOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    let Some(location) = ActorControlLocators::<T>::get(actor_id) else {
      return Ok(());
    };
    if let ActorControlLocation::Waiting {
      key: waiting_key, ..
    } = location
      && waiting_key == key
    {
      let mut cell = Self::remove_primary_control_cell_inner(actor_id)
        .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
      // The caller validates the exact physical pointer before consuming its primary.
      let consumed_trigger_source = matches!(key, WakeupKey::Tick(tick)
        if cell.hot.trigger_wakeup_pointer.is_some_and(|pointer| pointer.tick == tick));
      if consumed_trigger_source {
        if hot.cycle_state != CycleState::Idle
          || hot.pending_signal
          || hot.queue_ticket.is_some()
          || hot.trigger_wakeup_pointer.is_some()
        {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        cell.hot = Self::control_hot_from_scalar(hot.clone());
        cell.cursor = 0;
        cell.eligible_at = None;
        ActorUnsignaledControlCells::<T>::insert(actor_id, cell);
        ActorControlLocators::<T>::insert(actor_id, ActorControlLocation::Unsignaled);
      }
      return Ok(());
    }
    let (_, mut cell) =
      Self::load_primary_control_cell(actor_id).map_err(|_| EnqueueOutcome::CorruptedTopology)?;
    cell.hot = Self::control_hot_from_scalar(hot.clone());
    Self::store_primary_control_cell(location, cell).map_err(|_| EnqueueOutcome::CorruptedTopology)
  }

  fn commit_paged_enqueue_transactional(plan: QueueAppendPlan<T>) -> Result<(), EnqueueOutcome> {
    with_transaction_opaque_err(|| match Self::commit_paged_enqueue(plan) {
      Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
      Err(error) => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error)),
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  pub(crate) fn commit_paged_enqueue(plan: QueueAppendPlan<T>) -> Result<(), EnqueueOutcome> {
    #[cfg(test)]
    QUEUE_APPEND_COMMITS.with(|count| count.set(count.get().saturating_add(1)));
    for PreparedReadyPublication { ticket, cell } in plan.publications {
      let actor_id = cell.actor_id;
      let hot = Self::control_hot_to_scalar(&cell.hot, Some(ticket));
      if let Some(location) = ActorControlLocators::<T>::get(actor_id) {
        let (_, source) = Self::load_primary_control_cell(actor_id)
          .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
        let source_hot_matches = if source.hot == cell.hot {
          true
        } else {
          let mut pre_activation_hot = cell.hot.clone();
          pre_activation_hot.pending_signal = false;
          source.hot == pre_activation_hot
        };
        if matches!(location, ActorControlLocation::Ready { .. })
          || source.actor_id != cell.actor_id
          || source.identity != cell.identity
          || !source_hot_matches
          || source.cursor != cell.cursor
          || source.admission != cell.admission
          || source.resources != cell.resources
        {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        Self::detach_primary_for_successor(actor_id, &hot)?;
      }
      if ActorReadyTail::<T>::get() != ticket || cell.eligible_at.is_none() {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
      let (stored_actor, stored_ticket) =
        Self::control_append_ready(cell).map_err(|_| EnqueueOutcome::CorruptedTopology)?;
      if (stored_actor, stored_ticket) != (actor_id, ticket) {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
    }
    if ActorReadyTail::<T>::get() != plan.next_tail
      || ActorReadyOccupancy::<T>::get() != plan.next_occupancy
    {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    Ok(())
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub(crate) fn try_paged_enqueue(actor_id: ActorId) -> Result<(), EnqueueOutcome> {
    with_transaction_opaque_err(|| match Self::preflight_retained_paged_enqueue(actor_id) {
      Ok(plan) => match Self::commit_paged_enqueue(plan) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      },
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

  pub(crate) fn request_activation(
    actor_id: ActorId,
  ) -> Result<ActivationOutcome, ActivationFailure> {
    let activate = || Self::request_activation_inner(actor_id);
    if polkadot_sdk::frame_support::storage::transactional::is_transactional() {
      return activate();
    }
    polkadot_sdk::frame_support::storage::with_transaction(|| match activate() {
      Ok(outcome) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(outcome)),
      Err(error) => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error)),
    })
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub(crate) fn request_observation_activation_compact(
    actor_id: ActorId,
    feed: T::ObservationFeedId,
  ) -> Result<ActivationOutcome, ActivationFailure> {
    Self::request_observation_activation_compact_with_cause(
      actor_id,
      feed,
      TriggerCauseProvenance::Deferred,
      frame_system::Pallet::<T>::block_number().saturated_into::<u64>(),
    )
  }

  pub(crate) fn request_observation_activation_compact_with_cause(
    actor_id: ActorId,
    feed: T::ObservationFeedId,
    cause_provenance: TriggerCauseProvenance,
    cause_block: u64,
  ) -> Result<ActivationOutcome, ActivationFailure> {
    let activate = || match Self::request_observation_activation_compact_inner(
      actor_id,
      feed,
      ObservationTerminalHandling::Execute,
      cause_provenance,
      cause_block,
    )? {
      ObservationActivationOutcome::Ordinary(outcome) => Ok(outcome),
      ObservationActivationOutcome::TerminalDeferred => Err(ActivationFailure::Permanent(
        Error::<T>::ActorInvariant.into(),
      )),
    };
    if polkadot_sdk::frame_support::storage::transactional::is_transactional() {
      return activate();
    }
    polkadot_sdk::frame_support::storage::with_transaction(|| match activate() {
      Ok(outcome) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(outcome)),
      Err(error) => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error)),
    })
  }

  pub(crate) fn request_observation_activation_ordinary_with_cause(
    actor_id: ActorId,
    feed: T::ObservationFeedId,
    cause_provenance: TriggerCauseProvenance,
    cause_block: u64,
  ) -> Result<ObservationActivationOutcome, ActivationFailure> {
    Self::request_observation_activation_compact_inner(
      actor_id,
      feed,
      ObservationTerminalHandling::Defer,
      cause_provenance,
      cause_block,
    )
  }

  pub(crate) fn prepare_observation_placement_candidate(
    actor_id: ActorId,
    feed: T::ObservationFeedId,
    _cause_provenance: TriggerCauseProvenance,
    _cause_block: u64,
  ) -> Result<Option<ObservationPlacementCandidate<T>>, ActivationFailure> {
    if IndexedTriggerDetectionDisabled::<T>::contains_key(actor_id) {
      return Ok(None);
    }
    let Some(state) = Self::load_observation_activation_state(actor_id, feed) else {
      return Ok(None);
    };
    let classification =
      Self::classify_observation_activation_compact(&state).map_err(|error| {
        ActivationFailure::Permanent(Self::classification_dispatch_error(error).into())
      })?;
    if classification.terminal_reason.is_some()
      || state.hot.queue_ticket.is_some()
      || state.hot.wakeup_pointer.is_some()
    {
      return Ok(None);
    }
    let now = frame_system::Pallet::<T>::block_number();
    let wakeup_at = if state.hot.lifecycle.is_paused() {
      let Some(window) = state.authority.window else {
        return Ok(None);
      };
      Self::window_terminal_at(&window)
    } else {
      let eligible_at = if state.hot.cycle_state == CycleState::Suspended {
        state
          .run_head
          .as_ref()
          .ok_or(ActivationFailure::Permanent(
            Error::<T>::ActorRunInvariant.into(),
          ))?
          .eligible_at
      } else {
        let cooldown_anchor = state
          .hot
          .last_cycle_block
          .unwrap_or(state.hot.schedule_anchor);
        let cooldown_eligible_at =
          if state.identity.cycle_nonce == 0 && state.hot.last_cycle_block.is_none() {
            state.hot.schedule_anchor
          } else {
            cooldown_anchor
              .checked_add(&state.authority.cooldown_blocks.into())
              .ok_or(ActivationFailure::Permanent(
                Error::<T>::SchedulerIndexExhausted.into(),
              ))?
          };
        let window_floor = state
          .authority
          .window
          .map(|window| window.start)
          .unwrap_or_else(Zero::zero);
        now.max(cooldown_eligible_at).max(window_floor)
      };
      state.authority.window.map_or(eligible_at, |window| {
        eligible_at.min(Self::window_terminal_at(&window))
      })
    };
    let exact_next_block = now
      .checked_add(&One::one())
      .ok_or(ActivationFailure::Permanent(
        Error::<T>::SchedulerIndexExhausted.into(),
      ))?;
    let mut hot = state.hot.clone();
    hot.pending_signal = true;
    if wakeup_at >= exact_next_block {
      return Ok(Some(ObservationPlacementCandidate::Wakeup(
        ObservationWakeupCandidate {
          state,
          hot,
          wakeup_key: WakeupKey::Block(wakeup_at),
        },
      )));
    }
    Ok(Some(ObservationPlacementCandidate::Queue(
      ObservationQueueCandidate { state, hot },
    )))
  }

  #[cfg(test)]
  pub(crate) fn test_reset_observation_wakeup_cohort_commits() {
    OBSERVATION_WAKEUP_COHORT_COMMITS.with(|count| count.set(0));
  }

  #[cfg(test)]
  pub(crate) fn test_observation_wakeup_cohort_commits() -> u32 {
    OBSERVATION_WAKEUP_COHORT_COMMITS.with(core::cell::Cell::get)
  }

  pub(crate) fn commit_observation_queue_cohort(
    candidates: Vec<ObservationQueueCandidate<T>>,
  ) -> Result<(), EnqueueOutcome> {
    if candidates.is_empty() || candidates.len() > T::ObservationPageSize::get() as usize {
      return Err(EnqueueOutcome::CapacityUnavailable);
    }
    with_transaction_opaque_err(|| {
      let mut admitted = Vec::new();
      let mut occurrences = Vec::new();
      for candidate in candidates.into_iter(/* deos-bypass: bounded-iter */) {
        let actor_type = candidate.state.identity.actor_class.actor_type();
        let breakdown = Self::trigger_fee_for_weight(
          actor_type,
          TriggerFamily::ObservationChange,
          T::WeightInfo::observation_change_trigger_occurrence(),
        );
        let charged = match Self::try_charge_automatic_trigger_occurrence(
          actor_type,
          &candidate.state.identity.sovereign_account,
          breakdown,
        ) {
          Ok(charged) => charged,
          Err(_) => {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
              EnqueueOutcome::CorruptedTopology,
            ));
          }
        };
        if charged {
          occurrences.push((
            candidate.state.actor_id,
            breakdown,
            candidate.state.hot.pending_signal,
          ));
          admitted.push(candidate);
        }
      }
      if admitted.is_empty() {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()));
      }
      let topology = match Self::queue_topology_preflight(QueueMutation::Enqueue) {
        Ok(topology) => topology,
        Err(error) => {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
      };
      let mut plan = QueueAppendPlan {
        publications: Vec::new(),
        next_tail: topology.tail,
        next_occupancy: topology.occupancy,
      };
      // Identity-only membership is bounded by this observation page, not actor population.
      let mut planned_actors = alloc::collections::BTreeSet::new();
      for candidate in admitted.into_iter(/* deos-bypass: bounded-iter */) {
        let prepared_frame_cell =
          match Self::prepare_observation_ready_cell(&candidate.state, &candidate.hot) {
            Ok(cell) => cell,
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error,
              ));
            }
          };
        if let Err(error) = Self::reserve_following_paged_enqueue_observation(
          &mut plan,
          &mut planned_actors,
          &candidate.state,
          candidate.hot,
          prepared_frame_cell,
        ) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
      }
      if let Err(error) = Self::commit_paged_enqueue(plan) {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
      }
      for (actor_id, breakdown, _) in occurrences {
        IndexedTriggerDetectionDisabled::<T>::insert(actor_id, ());
        Self::deposit_event(Event::TriggerOccurrenceProcessed {
          actor_id,
          trigger_family: breakdown.trigger_family,
          fee: breakdown.trigger_fee,
        });
      }
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  pub(crate) fn commit_observation_wakeup_cohort(
    candidates: Vec<ObservationWakeupCandidate<T>>,
  ) -> Result<(), EnqueueOutcome> {
    if candidates.is_empty() || candidates.len() > T::ObservationPageSize::get() as usize {
      return Err(EnqueueOutcome::CapacityUnavailable);
    }
    let wakeup_key = candidates[0].wakeup_key;
    if candidates
      .iter(/* deos-bypass: bounded-iter */)
      .any(|candidate| candidate.wakeup_key != wakeup_key)
    {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    let result = with_transaction_opaque_err(|| {
      for candidate in candidates {
        let actor_id = candidate.state.actor_id;
        let actor_type = candidate.state.identity.actor_class.actor_type();
        let breakdown = Self::trigger_fee_for_weight(
          actor_type,
          TriggerFamily::ObservationChange,
          T::WeightInfo::observation_change_trigger_occurrence(),
        );
        let charged = match Self::try_charge_automatic_trigger_occurrence(
          actor_type,
          &candidate.state.identity.sovereign_account,
          breakdown,
        ) {
          Ok(charged) => charged,
          Err(_) => {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
              EnqueueOutcome::CorruptedTopology,
            ));
          }
        };
        if !charged {
          continue;
        }
        match Self::place_observation_wakeup(&candidate.state, candidate.hot, candidate.wakeup_key)
        {
          Ok(()) | Err(EnqueueOutcome::AlreadyLive) => {}
          Err(error) => {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
          }
        }
        IndexedTriggerDetectionDisabled::<T>::insert(actor_id, ());
        Self::deposit_event(Event::TriggerOccurrenceProcessed {
          actor_id,
          trigger_family: breakdown.trigger_family,
          fee: breakdown.trigger_fee,
        });
      }
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
    #[cfg(test)]
    if result.is_ok() {
      OBSERVATION_WAKEUP_COHORT_COMMITS.with(|count| count.set(count.get().saturating_add(1)));
    }
    result
  }

  fn place_observation_wakeup(
    state: &ObservationActivationState<T>,
    hot: ActorHotStateOf<T>,
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
  ) -> Result<(), EnqueueOutcome> {
    let admission = state
      .admission
      .as_ref()
      .ok_or(EnqueueOutcome::CorruptedTopology)?;
    let cursor = state.run_head.as_ref().map_or(0, |run| run.cursor);
    let resources = state.loaded_step.as_ref().map_or(
      ActorStepResourceEnvelope {
        control: T::WeightInfo::scheduler_inner_zero_step_complete(),
        effect: Weight::zero(),
      },
      |loaded| loaded.resources,
    );
    Self::try_store_control_hot_with_authority(state.actor_id, hot.clone())
      .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
    Self::try_wakeup_substrate_schedule_transition_with_authority(
      state.actor_id,
      wakeup_key,
      hot,
      &state.identity,
      cursor,
      admission,
      resources,
    )
  }

  fn request_observation_activation_compact_inner(
    actor_id: ActorId,
    feed: T::ObservationFeedId,
    terminal_handling: ObservationTerminalHandling,
    _cause_provenance: TriggerCauseProvenance,
    _cause_block: u64,
  ) -> Result<ObservationActivationOutcome, ActivationFailure> {
    let Some(state) = Self::load_observation_activation_state(actor_id, feed) else {
      return if Self::control_hot_exists(actor_id) {
        Err(ActivationFailure::Permanent(
          Error::<T>::ActorInvariant.into(),
        ))
      } else {
        Ok(ObservationActivationOutcome::Ordinary(
          ActivationOutcome::IgnoredStale,
        ))
      };
    };
    let classification =
      Self::classify_observation_activation_compact(&state).map_err(|error| {
        ActivationFailure::Permanent(Self::classification_dispatch_error(error).into())
      })?;
    if classification.terminal_reason.is_some() {
      return if matches!(terminal_handling, ObservationTerminalHandling::Execute) {
        Self::request_activation_inner(actor_id).map(ObservationActivationOutcome::Ordinary)
      } else {
        Ok(ObservationActivationOutcome::TerminalDeferred)
      };
    }

    let already_pending = state.hot.pending_signal;
    let mut hot = state.hot.clone();
    hot.pending_signal = true;
    if hot.queue_ticket.is_some() {
      if !already_pending {
        Self::try_store_control_hot_with_authority(actor_id, hot)
          .map_err(|_| ActivationFailure::Permanent(Error::<T>::ActorInvariant.into()))?;
      }
      return Ok(ObservationActivationOutcome::Ordinary(if already_pending {
        ActivationOutcome::Coalesced
      } else {
        ActivationOutcome::Latched
      }));
    }

    enum CompactPlacement<BlockNumber> {
      None,
      Queue,
      Wakeup(BlockNumber),
    }
    let placement = if hot.lifecycle.is_paused() {
      state
        .authority
        .window
        .map(|window| CompactPlacement::Wakeup(Self::window_terminal_at(&window)))
        .unwrap_or(CompactPlacement::None)
    } else {
      let now = frame_system::Pallet::<T>::block_number();
      let eligible_at = if hot.cycle_state == CycleState::Suspended {
        state
          .run_head
          .as_ref()
          .ok_or(ActivationFailure::Permanent(
            Error::<T>::ActorRunInvariant.into(),
          ))?
          .eligible_at
      } else {
        let cooldown_anchor = hot.last_cycle_block.unwrap_or(hot.schedule_anchor);
        let cooldown_eligible_at =
          if state.identity.cycle_nonce == 0 && hot.last_cycle_block.is_none() {
            hot.schedule_anchor
          } else {
            cooldown_anchor
              .checked_add(&state.authority.cooldown_blocks.into())
              .ok_or(ActivationFailure::Permanent(
                Error::<T>::SchedulerIndexExhausted.into(),
              ))?
          };
        let window_floor = state
          .authority
          .window
          .map(|window| window.start)
          .unwrap_or_else(Zero::zero);
        now.max(cooldown_eligible_at).max(window_floor)
      };
      let wakeup_at = state.authority.window.map_or(eligible_at, |window| {
        eligible_at.min(Self::window_terminal_at(&window))
      });
      let exact_next_block = now
        .checked_add(&One::one())
        .ok_or(ActivationFailure::Permanent(
          Error::<T>::SchedulerIndexExhausted.into(),
        ))?;
      if wakeup_at < exact_next_block {
        CompactPlacement::Queue
      } else {
        CompactPlacement::Wakeup(wakeup_at)
      }
    };

    let placement_result = match placement {
      CompactPlacement::None => Self::try_store_control_hot_with_authority(actor_id, hot),
      CompactPlacement::Queue => {
        match Self::preflight_paged_enqueue_observation_loaded(&state, hot.clone()) {
          Ok(plan) => Self::commit_paged_enqueue(plan),
          Err(EnqueueOutcome::CapacityUnavailable) => {
            let retry_at = frame_system::Pallet::<T>::block_number()
              .checked_add(&One::one())
              .ok_or(ActivationFailure::Permanent(
                Error::<T>::SchedulerIndexExhausted.into(),
              ))?;
            Self::place_observation_wakeup(&state, hot, WakeupKey::Block(retry_at))
          }
          Err(_) => {
            return Self::request_activation_inner(actor_id)
              .map(ObservationActivationOutcome::Ordinary);
          }
        }
      }
      CompactPlacement::Wakeup(block) => {
        Self::place_observation_wakeup(&state, hot, WakeupKey::Block(block))
      }
    };
    match placement_result {
      Ok(()) | Err(EnqueueOutcome::AlreadyLive) => {
        Ok(ObservationActivationOutcome::Ordinary(if already_pending {
          ActivationOutcome::Coalesced
        } else {
          ActivationOutcome::Latched
        }))
      }
      Err(EnqueueOutcome::CapacityUnavailable | EnqueueOutcome::WakeupCapacityExhausted) => Err(
        ActivationFailure::Temporary(Error::<T>::QueueCapacityUnavailable.into()),
      ),
      Err(_) => Err(ActivationFailure::Permanent(
        Error::<T>::SchedulerIndexExhausted.into(),
      )),
    }
  }

  fn preflight_activation_enqueue(
    actor_id: ActorId,
    state: &ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
  ) -> Result<QueueAppendPlan<T>, EnqueueOutcome> {
    let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
    let loaded_step = if state.contract.steps.is_empty() {
      None
    } else {
      Some(
        Self::load_current_step_with_admission(actor_id, cursor, admission)
          .ok_or(EnqueueOutcome::CorruptedTopology)?,
      )
    };
    Self::preflight_paged_enqueue_actor_state(actor_id, state, admission, loaded_step.as_ref())
  }

  pub(crate) fn preflight_activation_loaded(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
  ) -> Result<ActivationPlan<T>, ActivationFailure> {
    let frame_admission = {
      let (_, identity, _, admission) = Self::load_frame_control_authority(actor_id).ok_or(
        ActivationFailure::Permanent(Error::<T>::ActorInvariant.into()),
      )?;
      if identity != state.identity {
        return Err(ActivationFailure::Permanent(
          Error::<T>::ActorInvariant.into(),
        ));
      }
      admission
    };
    Self::preflight_activation_from_authority(actor_id, state, frame_admission)
  }

  fn preflight_activation_from_authority(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    frame_admission: ActorAdmissionCertificateOf<T>,
  ) -> Result<ActivationPlan<T>, ActivationFailure> {
    let frame_source_state = state.clone();
    let already_pending = state.hot.pending_signal;
    let mut queue_state = state;
    let run_state = queue_state.run_state.clone();
    let mut hot = queue_state.hot.clone();
    hot.pending_signal = true;
    queue_state.hot = hot.clone();
    let instance = Self::derive_active_actor_view(
      queue_state.identity.clone(),
      hot.clone(),
      queue_state.contract.clone(),
    );
    let classification =
      Self::classify_actor_loaded(&instance, run_state.as_ref()).map_err(|error| {
        ActivationFailure::Permanent(Self::classification_dispatch_error(error).into())
      })?;
    let action = if matches!(
      classification.terminal_reason,
      Some(CloseReason::WindowExpired | CloseReason::CycleNonceExhausted)
    ) {
      ActivationAction::Close(classification.terminal_reason.ok_or(
        ActivationFailure::Permanent(Error::<T>::ActorInvariant.into()),
      )?)
    } else if instance.queue_ticket.is_some()
      || (ActorControlLocators::<T>::contains_key(actor_id)
        && matches!(
          instance.cycle_state,
          CycleState::Running | CycleState::Suspended
        ))
    {
      ActivationAction::CoalesceLive
    } else if matches!(
      instance.trigger,
      Trigger::AtTime { .. } | Trigger::Cadenced { .. }
    ) {
      ActivationAction::EnqueueTemporal(Self::preflight_activation_enqueue(
        actor_id,
        &queue_state,
        &frame_admission,
      ))
    } else {
      match Self::preflight_prime_schedule_loaded(&instance, run_state.as_ref()) {
        Ok(PrimeSchedulePlan::Enqueue) => ActivationAction::EnqueueReady(
          Self::preflight_activation_enqueue(actor_id, &queue_state, &frame_admission),
        ),
        other => ActivationAction::PrimeSchedule(other),
      }
    };
    Ok(ActivationPlan {
      actor_id,

      frame_admission,
      frame_source_state,
      already_pending,
      prospective_hot: hot,
      instance,
      terminal_reason: classification.terminal_reason,
      action,
    })
  }

  #[cfg(test)]
  pub(crate) fn test_activation_plan_kind(actor_id: ActorId) -> Result<u8, ActivationFailure> {
    let loaded = if cfg!(feature = "runtime-benchmarks") {
      Self::load_actor_state(actor_id)
    } else {
      Self::load_frame_actor_state(actor_id)
    };
    let LoadedActorStateOf::Active(state) = loaded else {
      return Err(ActivationFailure::Permanent(
        Error::<T>::ActorInvariant.into(),
      ));
    };
    let plan = Self::preflight_activation_loaded(actor_id, state)?;
    Ok(match plan.action {
      ActivationAction::Close(_) => 0,
      ActivationAction::CoalesceLive => 1,
      ActivationAction::EnqueueTemporal(_) => 2,
      ActivationAction::PrimeSchedule(Ok(PrimeSchedulePlan::None)) => 3,
      ActivationAction::EnqueueReady(_) => 4,
      ActivationAction::PrimeSchedule(Ok(PrimeSchedulePlan::Enqueue)) => 4,
      ActivationAction::PrimeSchedule(Ok(PrimeSchedulePlan::BlockWakeup(_))) => 5,
      ActivationAction::PrimeSchedule(Err(_)) => 6,
    })
  }

  pub(crate) fn commit_activation_plan(
    plan: ActivationPlan<T>,
  ) -> Result<ActivationOutcome, ActivationFailure> {
    if polkadot_sdk::frame_support::storage::transactional::is_transactional() {
      return Self::commit_activation_plan_inner(plan);
    }
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      match Self::commit_activation_plan_inner(plan) {
        Ok(outcome) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(outcome))
        }
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  fn commit_activation_plan_inner(
    plan: ActivationPlan<T>,
  ) -> Result<ActivationOutcome, ActivationFailure> {
    let ActivationPlan {
      actor_id,

      frame_admission,
      frame_source_state,
      already_pending,
      prospective_hot,
      instance,
      terminal_reason: _,
      action,
    } = plan;
    let placement_hot = prospective_hot.clone();
    if !already_pending {
      // A live process latches the deferred occurrence by mutating its canonical primary in place.
      // Placement-changing activations instead publish prospective authority in the destination
      // cell; writing it into Unsignaled first would create a forbidden transient owner state.
      if matches!(
        &action,
        ActivationAction::CoalesceLive
          | ActivationAction::PrimeSchedule(Ok(PrimeSchedulePlan::None))
      ) {
        Self::try_store_control_hot_with_authority(actor_id, prospective_hot)
          .map_err(|_| ActivationFailure::Permanent(Error::<T>::ActorInvariant.into()))?;
      }
    }
    match action {
      ActivationAction::Close(reason) => {
        {
          Self::finalize_actor_from_retained_state(
            actor_id,
            frame_source_state,
            &frame_admission,
            reason,
          )
          .map_err(ActivationFailure::Permanent)?;
        }
        return Ok(ActivationOutcome::Closed);
      }
      ActivationAction::CoalesceLive => {
        return Ok(if already_pending {
          ActivationOutcome::Coalesced
        } else {
          ActivationOutcome::Latched
        });
      }
      ActivationAction::EnqueueReady(_)
      | ActivationAction::EnqueueTemporal(_)
      | ActivationAction::PrimeSchedule(_) => {}
    }

    let placement = match action {
      ActivationAction::EnqueueReady(Ok(queue_plan))
      | ActivationAction::EnqueueTemporal(Ok(queue_plan)) => {
        Self::commit_paged_enqueue_transactional(queue_plan)
      }
      ActivationAction::EnqueueReady(Err(EnqueueOutcome::CapacityUnavailable))
      | ActivationAction::EnqueueTemporal(Err(EnqueueOutcome::CapacityUnavailable)) => {
        match frame_system::Pallet::<T>::block_number().checked_add(&One::one()) {
          Some(next_block) => Self::defer_activation_wakeup(
            actor_id,
            next_block,
            &instance,
            placement_hot.clone(),
            &frame_source_state,
            &frame_admission,
          ),
          None => Err(EnqueueOutcome::SchedulerIndexExhausted),
        }
      }
      ActivationAction::EnqueueReady(Err(error))
      | ActivationAction::EnqueueTemporal(Err(error)) => Err(error),
      ActivationAction::PrimeSchedule(Ok(PrimeSchedulePlan::None)) => {
        if Self::load_frame_control_authority(actor_id).is_some() {
          Ok(())
        } else {
          Err(EnqueueOutcome::CorruptedTopology)
        }
      }
      ActivationAction::PrimeSchedule(Ok(PrimeSchedulePlan::Enqueue)) => {
        Err(EnqueueOutcome::CorruptedTopology)
      }
      ActivationAction::PrimeSchedule(Ok(PrimeSchedulePlan::BlockWakeup(block))) => {
        Self::defer_activation_wakeup(
          actor_id,
          block,
          &instance,
          placement_hot,
          &frame_source_state,
          &frame_admission,
        )
      }
      ActivationAction::PrimeSchedule(Err(error)) => Err(error),
      ActivationAction::Close(_) | ActivationAction::CoalesceLive => {
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
        EnqueueOutcome::TicketExhausted
        | EnqueueOutcome::SchedulerIndexExhausted
        | EnqueueOutcome::WakeupIndexExhausted,
      ) => {
        Self::finalize_actor_from_retained_state(
          actor_id,
          frame_source_state,
          &frame_admission,
          CloseReason::SchedulerIndexExhausted,
        )
        .map_err(ActivationFailure::Permanent)?;
        Ok(ActivationOutcome::Closed)
      }
      Err(EnqueueOutcome::CorruptedTopology) => Err(ActivationFailure::Permanent(
        Error::<T>::SchedulerIndexExhausted.into(),
      )),
    }
  }

  fn request_activation_inner(actor_id: ActorId) -> Result<ActivationOutcome, ActivationFailure> {
    let loaded = Self::load_actor_state_with_authority(actor_id);
    let state = match loaded {
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

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub fn paged_invalidate(actor_id: ActorId) -> Option<QueueTicket> {
    Self::try_invalidate_ready_to_unsignaled(actor_id)
      .ok()
      .flatten()
  }

  fn invalidate_ready_to_unsignaled_inner(
    actor_id: ActorId,
  ) -> Result<Option<QueueTicket>, EnqueueOutcome> {
    let Some((state, admission, loaded_step)) = Self::load_frame_actor_service_state(actor_id)
    else {
      return if Self::control_hot_exists(actor_id) {
        Err(EnqueueOutcome::CorruptedTopology)
      } else {
        Ok(None)
      };
    };
    let ticket = state.hot.queue_ticket;
    if ticket.is_some() {
      let resources = if state.contract.steps.is_empty() {
        ActorStepResourceEnvelope {
          control: T::WeightInfo::scheduler_inner_zero_step_complete(),
          effect: Weight::zero(),
        }
      } else {
        let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
        loaded_step
          .filter(|loaded| loaded.cursor == cursor)
          .map(|loaded| loaded.resources)
          .ok_or(EnqueueOutcome::CorruptedTopology)?
      };
      let mut hot = state.hot;
      hot.queue_ticket = None;
      Self::remove_primary_control_cell_inner(actor_id)
        .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
      Self::restore_unsignaled_from_authority(
        actor_id,
        hot,
        &state.identity,
        state.run_state.as_ref(),
        &admission,
        resources,
      )?;
    }
    Ok(ticket)
  }

  pub(crate) fn try_invalidate_ready_to_unsignaled(
    actor_id: ActorId,
  ) -> Result<Option<QueueTicket>, EnqueueOutcome> {
    with_transaction_opaque_err(
      || match Self::invalidate_ready_to_unsignaled_inner(actor_id) {
        Ok(ticket) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(ticket)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      },
    )
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  #[cfg(test)]
  pub(crate) fn try_paged_invalidate(
    actor_id: ActorId,
  ) -> Result<Option<QueueTicket>, EnqueueOutcome> {
    Self::try_invalidate_ready_to_unsignaled(actor_id)
  }

  pub(crate) fn invalidate_ready_to_unsignaled_with_authority(
    actor_id: ActorId,
  ) -> Result<(), EnqueueOutcome> {
    Self::try_invalidate_ready_to_unsignaled(actor_id)?
      .map(|_| ())
      .ok_or(EnqueueOutcome::CorruptedTopology)
  }

  pub fn paged_head_entry() -> Option<(QueueTicket, QueueEntry<BlockNumberFor<T>>)> {
    let head = ActorReadyHead::<T>::get();
    if head >= ActorReadyTail::<T>::get() {
      return None;
    }
    let chunk = ActorReadyFrameChunks::<T>::get(head / 32)?;
    let cell = chunk.get((head % 32) as usize)?.as_ref()?;
    let entry = ActorStepTicket {
      actor_id: cell.actor_id,
      // Discovery must retain an exhausted head for mandatory terminal cleanup.
      // Opening still requires a checked next nonce in the execution ticket builder.
      cycle_nonce: cell.identity.cycle_nonce.saturating_add(1),
      cursor: cell.cursor,
      ticket: head,
      eligible_at: cell.eligible_at?,
      contract_commitment: ActorContractCommitment {
        semantic_contract_id: cell.admission.semantic_contract_id,
        body_commitment: cell.admission.body_commitment,
      },
    };
    Some((head, entry))
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub(crate) fn paged_consume_head_at(position: QueueTicket) -> Result<(), EnqueueOutcome> {
    Self::paged_consume_head_at_inner(position, ReadyHeadOwner::DiscoverCanonical)
  }

  fn paged_consume_loaded_head_at(
    position: QueueTicket,
    actor_id: ActorId,
    ticket: QueueTicket,
    hot: ActorHotStateOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    Self::paged_consume_head_at_inner(
      position,
      ReadyHeadOwner::Loaded {
        actor_id,
        ticket,
        hot,
      },
    )
  }

  fn paged_consume_closed_head_at(position: QueueTicket) -> Result<(), EnqueueOutcome> {
    Self::paged_consume_head_at_inner(position, ReadyHeadOwner::ClosedTombstone)
  }

  fn paged_consume_head_at_inner(
    position: QueueTicket,
    owner: ReadyHeadOwner<T>,
  ) -> Result<(), EnqueueOutcome> {
    with_transaction_opaque_err(|| {
      let transition = || -> Result<(), EnqueueOutcome> {
        let topology = Self::queue_topology_preflight(QueueMutation::Head)?;
        if position != topology.head || position >= topology.tail {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        let entry = Self::paged_head_entry().map(|(_, entry)| entry);
        let loaded = match owner {
          ReadyHeadOwner::Loaded {
            actor_id,
            ticket,
            hot,
          } => {
            let entry = entry.ok_or(EnqueueOutcome::CorruptedTopology)?;
            if entry.actor_id != actor_id
              || entry.ticket != ticket
              || hot.queue_ticket != Some(ticket)
            {
              return Err(EnqueueOutcome::CorruptedTopology);
            }
            Some(entry)
          }
          ReadyHeadOwner::ClosedTombstone => {
            if entry.is_some() {
              return Err(EnqueueOutcome::CorruptedTopology);
            }
            None
          }
          #[cfg(any(test, feature = "runtime-benchmarks"))]
          ReadyHeadOwner::DiscoverCanonical => {
            let entry = entry.ok_or(EnqueueOutcome::CorruptedTopology)?;
            let (state, _, _) = Self::load_frame_actor_service_state(entry.actor_id)
              .ok_or(EnqueueOutcome::CorruptedTopology)?;
            if state.hot.queue_ticket != Some(entry.ticket) {
              return Err(EnqueueOutcome::CorruptedTopology);
            }
            Some(entry)
          }
        };
        if let Some(entry) = loaded {
          Self::consume_ready_primary(entry.actor_id, entry.ticket)?;
        }
        let next_head = position
          .checked_add(1)
          .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
        ActorReadyHead::<T>::put(next_head);
        if next_head.is_multiple_of(32) || next_head == topology.tail {
          ActorReadyFrameChunks::<T>::remove(position / 32);
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

  #[cfg(any(test, feature = "runtime-benchmarks"))]
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
    with_transaction_opaque_err(|| {
      let transition = || -> Result<QueueDrainStats, EnqueueOutcome> {
        let topology = Self::queue_topology_preflight(QueueMutation::Head)?;
        let mut stats = QueueDrainStats::default();
        let mut head = topology.head;
        let limit = scan_limit.min(T::MaxQueueEntriesScannedPerBlock::get());
        while head < topology.tail && head < cutoff && stats.entries_scanned < limit {
          let page_id = head / 32;
          let chunk =
            ActorReadyFrameChunks::<T>::get(page_id).ok_or(EnqueueOutcome::CorruptedTopology)?;
          if chunk.len() != 32 {
            return Err(EnqueueOutcome::CorruptedTopology);
          }
          stats.pages_touched = stats.pages_touched.saturating_add(1);
          while head < topology.tail
            && head < cutoff
            && head / 32 == page_id
            && stats.entries_scanned < limit
          {
            stats.entries_scanned = stats.entries_scanned.saturating_add(1);
            if let Some(cell) = &chunk[(head % 32) as usize] {
              if ActorControlLocators::<T>::get(cell.actor_id)
                != Some(ActorControlLocation::Ready { ticket: head })
                || Self::project_control_cell(cell, ActorControlLocation::Ready { ticket: head })
                  .is_none()
              {
                return Err(EnqueueOutcome::CorruptedTopology);
              }
              if head != topology.head {
                ActorReadyHead::<T>::put(head);
              }
              return Ok(stats);
            }
            stats.tombstones_skipped = stats.tombstones_skipped.saturating_add(1);
            head = head
              .checked_add(1)
              .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
          }
          if head.is_multiple_of(32) || head == topology.tail {
            ActorReadyFrameChunks::<T>::remove(page_id);
            stats.pages_deleted = stats.pages_deleted.saturating_add(1);
          }
        }
        if head != topology.head {
          ActorReadyHead::<T>::put(head);
        }
        Ok(stats)
      };
      match transition() {
        Ok(stats) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(stats)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  pub(crate) fn wakeup_page_entry_matches(
    pointer: WakeupPointer<BlockNumberFor<T>>,
    actor_id: ActorId,
  ) -> bool {
    ActorWaitingFrameChunks::<T>::get((pointer.block, pointer.page_id))
      .and_then(|page| page.entries.get(pointer.slot as usize).cloned().flatten())
      .is_some_and(|entry| match entry {
        ActorWaitingEntry::Primary(cell) => cell.actor_id == actor_id,
        ActorWaitingEntry::Reference(reference) => reference.actor_id == actor_id,
      })
  }

  fn wakeup_pointer_for_clock(
    hot: &ActorHotStateOf<T>,
    clock: WakeupClock,
  ) -> Option<WakeupPointer<BlockNumberFor<T>>> {
    match clock {
      WakeupClock::Block => hot.wakeup_pointer,
      WakeupClock::Tick => hot.trigger_wakeup_pointer.map(|pointer| WakeupPointer {
        block: WakeupKey::Tick(pointer.tick),
        page_id: pointer.page_id,
        slot: pointer.slot,
      }),
    }
  }

  fn clear_wakeup_pointer_for_clock(hot: &mut ActorHotStateOf<T>, clock: WakeupClock) {
    match clock {
      WakeupClock::Block => hot.wakeup_pointer = None,
      WakeupClock::Tick => hot.trigger_wakeup_pointer = None,
    }
  }

  pub(crate) fn wakeup_substrate_invalidate_loaded(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
  ) -> Result<Option<WakeupPointer<BlockNumberFor<T>>>, EnqueueOutcome> {
    Self::wakeup_substrate_invalidate_clock_loaded(actor_id, state, admission, WakeupClock::Block)
  }

  pub(crate) fn trigger_wakeup_substrate_invalidate_loaded(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
  ) -> Result<Option<WakeupPointer<BlockNumberFor<T>>>, EnqueueOutcome> {
    Self::wakeup_substrate_invalidate_clock_loaded(actor_id, state, admission, WakeupClock::Tick)
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  fn wakeup_substrate_invalidate_clock_inner(
    actor_id: ActorId,
    clock: WakeupClock,
  ) -> Result<Option<WakeupPointer<BlockNumberFor<T>>>, EnqueueOutcome> {
    let Some((state, admission, _)) = Self::load_frame_actor_service_state(actor_id) else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    Self::wakeup_substrate_invalidate_clock_loaded(actor_id, state, &admission, clock)
  }

  fn wakeup_substrate_invalidate_clock_loaded(
    actor_id: ActorId,
    mut state: ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    clock: WakeupClock,
  ) -> Result<Option<WakeupPointer<BlockNumberFor<T>>>, EnqueueOutcome> {
    let Some(pointer) = Self::wakeup_pointer_for_clock(&state.hot, clock) else {
      return Ok(None);
    };
    Self::invalidate_wakeup_reference(actor_id, pointer, admission.admission_identity)?;
    Self::clear_wakeup_pointer_for_clock(&mut state.hot, clock);
    {
      Self::consume_waiting_from_supplied_authority(actor_id, pointer.block, &state.hot)?;
    }
    Ok(Some(pointer))
  }

  pub(crate) fn invalidate_wakeup_reference(
    actor_id: ActorId,
    pointer: WakeupPointer<BlockNumberFor<T>>,
    admission_identity: [u8; 32],
  ) -> Result<(), EnqueueOutcome> {
    let page = ActorWaitingFrameChunks::<T>::get((pointer.block, pointer.page_id))
      .ok_or(EnqueueOutcome::CorruptedTopology)?;
    match page
      .entries
      .get(pointer.slot as usize)
      .and_then(Option::as_ref)
    {
      Some(ActorWaitingEntry::Primary(cell))
        if cell.actor_id == actor_id && cell.admission.admission_identity == admission_identity =>
      {
        // The caller transfers primary authority immediately after clearing this pointer.
        // Only that transfer may remove the primary slot and its locator.
        Ok(())
      }
      Some(ActorWaitingEntry::Reference(reference))
        if reference.actor_id == actor_id && reference.admission_identity == admission_identity =>
      {
        Self::remove_waiting_entry(pointer)
          .map(|_| ())
          .map_err(|_| EnqueueOutcome::CorruptedTopology)
      }
      _ => Err(EnqueueOutcome::CorruptedTopology),
    }
  }

  #[cfg(all(test, feature = "runtime-benchmarks"))]
  pub(crate) fn control_invalidate_wakeup_reference(
    mut cell: ActorControlCellOf<T>,
    clock: WakeupClock,
  ) -> Result<ActorControlCellOf<T>, ActorControlTransitionError> {
    let pointer = match clock {
      WakeupClock::Block => cell.hot.wakeup_pointer,
      WakeupClock::Tick => cell
        .hot
        .trigger_wakeup_pointer
        .map(|pointer| WakeupPointer {
          block: WakeupKey::Tick(pointer.tick),
          page_id: pointer.page_id,
          slot: pointer.slot,
        }),
    }
    .ok_or(ActorControlTransitionError::Invariant)?;
    Self::invalidate_wakeup_reference(cell.actor_id, pointer, cell.admission.admission_identity)
      .map_err(|_| ActorControlTransitionError::Invariant)?;
    match clock {
      WakeupClock::Block => cell.hot.wakeup_pointer = None,
      WakeupClock::Tick => cell.hot.trigger_wakeup_pointer = None,
    }
    Ok(cell)
  }

  #[cfg(all(test, feature = "runtime-benchmarks"))]
  pub(crate) fn control_due_wakeup_reference(
    key: WakeupKey<BlockNumberFor<T>>,
    now: BlockNumberFor<T>,
    now_tick: SchedulerTick,
  ) -> Result<(ActorId, WakeupPointer<BlockNumberFor<T>>), ActorControlTransitionError> {
    let due = match key {
      WakeupKey::Block(block) => block <= now,
      WakeupKey::Tick(tick) => tick <= now_tick,
    };
    if !due || Self::wakeup_cursor_peek_key(key.clock()) != Some(key) {
      return Err(ActorControlTransitionError::Invariant);
    }
    let cursor_index =
      ActorWaitingCursorIndices::<T>::get(key).ok_or(ActorControlTransitionError::Invariant)?;
    if ActorWaitingOccupancies::<T>::get(key) == 0
      || Self::wakeup_cursor_get(key.clock(), cursor_index) != Some(key)
    {
      return Err(ActorControlTransitionError::Invariant);
    }
    let page_id = ActorWaitingHeads::<T>::get(key) / 32;
    let page = ActorWaitingFrameChunks::<T>::get((key, page_id))
      .ok_or(ActorControlTransitionError::Invariant)?;
    if page.previous_page.is_some()
      || page.entries.iter(/* deos-bypass: bounded-iter */).filter(|entry| entry.is_some()).count()
        != page.live_entries as usize
      || page.live_entries == 0
    {
      return Err(ActorControlTransitionError::Invariant);
    }
    let scan_slot = page.scan_slot as usize;
    if page
      .entries
      .iter(/* deos-bypass: bounded-iter */)
      .take(scan_slot)
      .any(Option::is_some)
    {
      return Err(ActorControlTransitionError::Invariant);
    }
    let (slot, entry) = page
      .entries
      .iter(/* deos-bypass: bounded-iter */)
      .enumerate()
      .skip(scan_slot)
      .find_map(|(slot, entry)| entry.as_ref().map(|entry| (slot, entry)))
      .ok_or(ActorControlTransitionError::Invariant)?;
    let slot = WakeupSlot::try_from(slot).map_err(|_| ActorControlTransitionError::Invariant)?;
    Ok((
      match entry {
        ActorWaitingEntry::Primary(cell) => cell.actor_id,
        ActorWaitingEntry::Reference(reference) => reference.actor_id,
      },
      WakeupPointer {
        block: key,
        page_id,
        slot,
      },
    ))
  }

  #[cfg(all(test, feature = "runtime-benchmarks"))]
  pub(crate) fn control_due_wakeup_primary(
    key: WakeupKey<BlockNumberFor<T>>,
    now: BlockNumberFor<T>,
    now_tick: SchedulerTick,
  ) -> Result<
    (
      ActorControlLocation<BlockNumberFor<T>>,
      ActorControlCellOf<T>,
    ),
    ActorControlTransitionError,
  > {
    let (actor_id, pointer) = Self::control_due_wakeup_reference(key, now, now_tick)?;
    let location =
      ActorControlLocators::<T>::get(actor_id).ok_or(ActorControlTransitionError::Invariant)?;
    let cell = match location {
      ActorControlLocation::Unsignaled => ActorUnsignaledControlCells::<T>::get(actor_id),
      ActorControlLocation::Ready { ticket } => ActorReadyFrameChunks::<T>::get(ticket / 32)
        .and_then(|chunk| chunk.get((ticket % 32) as usize).cloned().flatten()),
      ActorControlLocation::Waiting { key, page, slot } => {
        ActorWaitingFrameChunks::<T>::get((key, page))
          .and_then(|page| page.entries.get(slot as usize).cloned().flatten())
          .and_then(ActorWaitingEntry::into_primary)
      }
    }
    .ok_or(ActorControlTransitionError::Invariant)?;
    if cell.actor_id != actor_id {
      return Err(ActorControlTransitionError::Invariant);
    }
    let reference_page = ActorWaitingFrameChunks::<T>::get((key, pointer.page_id))
      .ok_or(ActorControlTransitionError::Invariant)?;
    let entry = reference_page
      .entries
      .get(pointer.slot as usize)
      .and_then(Option::as_ref)
      .ok_or(ActorControlTransitionError::Invariant)?;
    let admission_identity = match entry {
      ActorWaitingEntry::Primary(primary) => primary.admission.admission_identity,
      ActorWaitingEntry::Reference(reference) => reference.admission_identity,
    };
    if admission_identity != cell.admission.admission_identity {
      return Err(ActorControlTransitionError::Invariant);
    }
    let pointer_matches = match key.clock() {
      WakeupClock::Block => cell.hot.wakeup_pointer == Some(pointer),
      WakeupClock::Tick => match key {
        WakeupKey::Tick(tick) => cell.hot.trigger_wakeup_pointer.is_some_and(|candidate| {
          candidate.tick == tick
            && candidate.page_id == pointer.page_id
            && candidate.slot == pointer.slot
        }),
        WakeupKey::Block(_) => false,
      },
    };
    if !pointer_matches {
      return Err(ActorControlTransitionError::Invariant);
    }
    Ok((location, cell))
  }

  #[cfg(all(test, feature = "runtime-benchmarks"))]
  pub(crate) fn control_consume_due_wakeup_reference(
    cell: ActorControlCellOf<T>,
    key: WakeupKey<BlockNumberFor<T>>,
    now: BlockNumberFor<T>,
    now_tick: SchedulerTick,
  ) -> Result<ActorControlCellOf<T>, ActorControlTransitionError> {
    let due = match key {
      WakeupKey::Block(block) => block <= now,
      WakeupKey::Tick(tick) => tick <= now_tick,
    };
    if !due || Self::wakeup_cursor_peek_key(key.clock()) != Some(key) {
      return Err(ActorControlTransitionError::Invariant);
    }
    let pointer_matches = match key {
      WakeupKey::Block(_) => cell
        .hot
        .wakeup_pointer
        .is_some_and(|pointer| pointer.block == key),
      WakeupKey::Tick(tick) => cell
        .hot
        .trigger_wakeup_pointer
        .is_some_and(|pointer| pointer.tick == tick),
    };
    if !pointer_matches {
      return Err(ActorControlTransitionError::Invariant);
    }
    Self::control_invalidate_wakeup_reference(cell, key.clock())
  }

  #[cfg(any(feature = "try-runtime", all(test, feature = "runtime-benchmarks")))]
  pub(crate) fn frame_control_entries() -> Option<
    Vec<(
      ActorId,
      ActorControlLocation<BlockNumberFor<T>>,
      ActorControlCellOf<T>,
    )>,
  > {
    let mut entries = alloc::collections::BTreeMap::<
      ActorId,
      (
        ActorControlLocation<BlockNumberFor<T>>,
        ActorControlCellOf<T>,
      ),
    >::new();
    let mut ready_count = 0u32;
    for (actor_id, cell) in ActorUnsignaledControlCells::<T>::iter(/* deos-bypass: bounded-iter */)
    {
      if cell.actor_id != actor_id
        || entries
          .insert(actor_id, (ActorControlLocation::Unsignaled, cell))
          .is_some()
      {
        return None;
      }
    }
    for (page, chunk) in ActorReadyFrameChunks::<T>::iter(/* deos-bypass: bounded-iter */) {
      for (slot, cell) in chunk.into_iter().enumerate() {
        let Some(cell) = cell else {
          continue;
        };
        let ticket = page.checked_mul(32)?.checked_add(slot as u64)?;
        if entries
          .insert(
            cell.actor_id,
            (ActorControlLocation::Ready { ticket }, cell),
          )
          .is_some()
        {
          return None;
        }
        ready_count = ready_count.checked_add(1)?;
      }
    }
    for ((key, page), chunk) in ActorWaitingFrameChunks::<T>::iter(/* deos-bypass: bounded-iter */)
    {
      for (slot, entry) in chunk.entries.into_iter().enumerate() {
        let Some(ActorWaitingEntry::Primary(cell)) = entry else {
          continue;
        };
        let slot = u8::try_from(slot).ok()?;
        if entries
          .insert(
            cell.actor_id,
            (ActorControlLocation::Waiting { key, page, slot }, cell),
          )
          .is_some()
        {
          return None;
        }
      }
    }
    let locators =
      ActorControlLocators::<T>::iter().collect::<alloc::collections::BTreeMap<_, _>>();
    if locators.len() != entries.len()
      || entries
        .iter(/* deos-bypass: bounded-iter */)
        .any(|(actor_id, (location, _))| locators.get(actor_id) != Some(location))
      || ActorReadyOccupancy::<T>::get() != ready_count
    {
      return None;
    }
    Some(
      entries
        .into_iter()
        .map(|(actor_id, (location, cell))| (actor_id, location, cell))
        .collect(),
    )
  }

  pub(crate) fn load_primary_control_cell(
    actor_id: ActorId,
  ) -> Result<
    (
      ActorControlLocation<BlockNumberFor<T>>,
      ActorControlCellOf<T>,
    ),
    ActorControlTransitionError,
  > {
    let location =
      ActorControlLocators::<T>::get(actor_id).ok_or(ActorControlTransitionError::Invariant)?;
    let cell = match location {
      ActorControlLocation::Unsignaled => ActorUnsignaledControlCells::<T>::get(actor_id),
      ActorControlLocation::Ready { ticket } => ActorReadyFrameChunks::<T>::get(ticket / 32)
        .and_then(|chunk| chunk.get((ticket % 32) as usize).cloned().flatten()),
      ActorControlLocation::Waiting { key, page, slot } => {
        ActorWaitingFrameChunks::<T>::get((key, page))
          .and_then(|page| page.entries.get(slot as usize).cloned().flatten())
          .and_then(ActorWaitingEntry::into_primary)
      }
    }
    .ok_or(ActorControlTransitionError::Invariant)?;
    if cell.actor_id != actor_id {
      return Err(ActorControlTransitionError::Invariant);
    }
    Ok((location, cell))
  }

  pub(crate) fn store_primary_control_cell(
    location: ActorControlLocation<BlockNumberFor<T>>,
    cell: ActorControlCellOf<T>,
  ) -> Result<(), ActorControlTransitionError> {
    let actor_id = cell.actor_id;
    if ActorControlLocators::<T>::get(actor_id) != Some(location) {
      return Err(ActorControlTransitionError::Invariant);
    }
    match location {
      ActorControlLocation::Unsignaled => {
        if ActorUnsignaledControlCells::<T>::get(actor_id)
          .is_none_or(|stored| stored.actor_id != actor_id)
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        ActorUnsignaledControlCells::<T>::insert(actor_id, cell);
      }
      ActorControlLocation::Ready { ticket } => {
        let page = ticket / 32;
        let slot = (ticket % 32) as usize;
        let mut chunk =
          ActorReadyFrameChunks::<T>::get(page).ok_or(ActorControlTransitionError::Invariant)?;
        let stored = chunk
          .get_mut(slot)
          .ok_or(ActorControlTransitionError::Invariant)?;
        if stored.as_ref().map(|stored| stored.actor_id) != Some(actor_id) {
          return Err(ActorControlTransitionError::Invariant);
        }
        *stored = Some(cell);
        ActorReadyFrameChunks::<T>::insert(page, chunk);
      }
      ActorControlLocation::Waiting { key, page, slot } => {
        let mut chunk = ActorWaitingFrameChunks::<T>::get((key, page))
          .ok_or(ActorControlTransitionError::Invariant)?;
        let stored = chunk
          .entries
          .get_mut(slot as usize)
          .ok_or(ActorControlTransitionError::Invariant)?;
        if stored
          .as_ref()
          .and_then(ActorWaitingEntry::primary)
          .map(|stored| stored.actor_id)
          != Some(actor_id)
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        *stored = Some(ActorWaitingEntry::Primary(cell));
        ActorWaitingFrameChunks::<T>::insert((key, page), chunk);
      }
    }
    Ok(())
  }

  pub(crate) fn remove_primary_control_cell_inner(
    actor_id: ActorId,
  ) -> Result<ActorControlCellOf<T>, ActorControlTransitionError> {
    let (location, cell) = Self::load_primary_control_cell(actor_id)?;
    match location {
      ActorControlLocation::Unsignaled => {
        ActorUnsignaledControlCells::<T>::remove(actor_id);
      }
      ActorControlLocation::Ready { ticket } => {
        let page = ticket / 32;
        let slot = (ticket % 32) as usize;
        let mut chunk =
          ActorReadyFrameChunks::<T>::get(page).ok_or(ActorControlTransitionError::Invariant)?;
        let stored = chunk
          .get_mut(slot)
          .ok_or(ActorControlTransitionError::Invariant)?;
        if stored.as_ref().map(|stored| stored.actor_id) != Some(actor_id) {
          return Err(ActorControlTransitionError::Invariant);
        }
        *stored = None;
        ActorReadyFrameChunks::<T>::insert(page, chunk);
        ActorReadyOccupancy::<T>::try_mutate(|occupancy| {
          *occupancy = occupancy
            .checked_sub(1)
            .ok_or(ActorControlTransitionError::Invariant)?;
          Ok::<(), ActorControlTransitionError>(())
        })?;
      }
      ActorControlLocation::Waiting { key, page, slot } => {
        let removed = Self::remove_waiting_entry(WakeupPointer {
          block: key,
          page_id: page,
          slot: u32::from(slot),
        })?;
        if removed.primary().map(|stored| stored.actor_id) != Some(actor_id) {
          return Err(ActorControlTransitionError::Invariant);
        }
      }
    }
    ActorControlLocators::<T>::remove(actor_id);
    Ok(cell)
  }

  #[cfg(all(test, feature = "runtime-benchmarks"))]
  fn control_finalize_underfunded_at_time(
    actor_id: ActorId,
    location: ActorControlLocation<BlockNumberFor<T>>,
    cell: ActorControlCellOf<T>,
    identity: ActorIdentityOf<T>,
    contract: &ActorContractOf<T>,
  ) -> Result<(), ActorControlTransitionError> {
    let ActorClass::User { owner_slot } = identity.actor_class else {
      return Err(ActorControlTransitionError::Invariant);
    };
    if location != ActorControlLocation::Unsignaled
      || cell.actor_id != actor_id
      || cell.hot.cycle_state != CycleState::Idle
      || cell.hot.pending_signal
      || cell.eligible_at.is_some()
      || cell.hot.wakeup_pointer.is_some()
      || cell.hot.trigger_wakeup_pointer.is_some()
      || ActorIdentities::<T>::contains_key(actor_id)
      || !ActorFunding::<T>::contains_key(actor_id)
      || ActiveActorCount::<T>::get() == 0
      || ActorIdentityCount::<T>::get() == 0
      || SovereignIndex::<T>::get(&identity.sovereign_account) != Some(actor_id)
      || !Self::owner_slot_is_set(&OwnerSlotBitmaps::<T>::get(&identity.owner), owner_slot)
    {
      return Err(ActorControlTransitionError::Invariant);
    }
    ActorUnsignaledControlCells::<T>::remove(actor_id);
    ActorControlLocators::<T>::remove(actor_id);
    if !Self::control_remove_frame_owned_contract_geometry(actor_id, contract) {
      return Err(ActorControlTransitionError::Invariant);
    }
    ActorRunStateStore::<T>::remove(actor_id);
    ActorFunding::<T>::remove(actor_id);
    let active_count = ActiveActorCount::<T>::get()
      .checked_sub(1)
      .ok_or(ActorControlTransitionError::Invariant)?;
    let identity_count = ActorIdentityCount::<T>::get()
      .checked_sub(1)
      .ok_or(ActorControlTransitionError::Invariant)?;
    ActiveActorCount::<T>::put(active_count);
    ActorIdentityCount::<T>::put(identity_count);
    Self::remove_owner_slot_binding(&identity.owner, owner_slot, &identity.sovereign_account);
    Self::reconcile_actor_state_hold_with_authority(actor_id)
      .map_err(|_| ActorControlTransitionError::Invariant)?;
    Self::deposit_event(Event::ActorClosed {
      actor_id,
      reason: CloseReason::TriggerAdmissionInsufficient,
    });
    Ok(())
  }

  #[cfg(all(test, feature = "runtime-benchmarks"))]
  pub(crate) fn control_latch_manual_occurrence(
    actor_id: ActorId,
    now: BlockNumberFor<T>,
  ) -> Result<Option<ActorControlLocation<BlockNumberFor<T>>>, ActorControlTransitionError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let transition = || {
        let (location, mut cell) = Self::load_primary_control_cell(actor_id)?;
        if cell.hot.pending_signal {
          return Ok(None);
        }
        if cell.hot.trigger_wakeup_pointer.is_some() {
          return Err(ActorControlTransitionError::Invariant);
        }
        let idle_activation = location == ActorControlLocation::Unsignaled
          && cell.hot.cycle_state == CycleState::Idle
          && cell.eligible_at.is_none()
          && cell.hot.wakeup_pointer.is_none();
        let busy_deferred = matches!(
          cell.hot.cycle_state,
          CycleState::Running | CycleState::Suspended
        ) && cell.eligible_at.is_some()
          && matches!(
            location,
            ActorControlLocation::Ready { .. } | ActorControlLocation::Waiting { .. }
          );
        if !idle_activation && !busy_deferred {
          return Err(ActorControlTransitionError::Invariant);
        }
        let (identity, _, admission) = Self::project_control_cell(&cell, location)
          .ok_or(ActorControlTransitionError::Invariant)?;
        let (contract, _, _) = Self::control_load_current_step_contract(actor_id, &admission, 0)
          .ok_or(ActorControlTransitionError::Invariant)?;
        if !matches!(contract.trigger, Trigger::Manual)
          || !matches!(
            cell.hot.trigger_runtime_state,
            TriggerRuntimeState::Stateless
          )
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        let actor_type = identity.actor_class.actor_type();
        let breakdown = Self::trigger_fee_for_weight(
          actor_type,
          TriggerFamily::Manual,
          T::WeightInfo::manual_trigger(),
        );
        if !Self::trigger_occurrence_capacity_sufficient(
          actor_type,
          &identity.sovereign_account,
          breakdown,
        )
        .map_err(|_| ActorControlTransitionError::Invariant)?
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        if !Self::try_charge_prechecked_automatic_trigger_occurrence(
          actor_type,
          &identity.sovereign_account,
          breakdown,
        )
        .map_err(|_| ActorControlTransitionError::Invariant)?
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        cell.hot.pending_signal = true;
        let destination = if busy_deferred {
          Self::store_primary_control_cell(location, cell)?;
          location
        } else {
          let eligible_at = now
            .checked_add(&One::one())
            .ok_or(ActorControlTransitionError::IndexExhausted)?;
          cell.eligible_at = Some(eligible_at);
          Self::remove_primary_control_cell_inner(actor_id)
            .map_err(|_| ActorControlTransitionError::Invariant)?;
          let destination = Self::control_append_waiting(
            cell,
            WakeupKey::Block(eligible_at),
            ActorWaitingAuthority::Service,
          )?;
          destination
        };
        Self::deposit_event(Event::TriggerOccurrenceProcessed {
          actor_id,
          trigger_family: breakdown.trigger_family,
          fee: breakdown.trigger_fee,
        });
        Self::deposit_event(Event::ManualTriggerSet { actor_id });
        Ok(Some(destination))
      };
      match transition() {
        Ok(output) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(output)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  #[cfg(all(test, feature = "runtime-benchmarks"))]
  pub(crate) fn control_apply_address_event(
    actor_id: ActorId,
    asset: T::AssetId,
    amount: T::Balance,
    source: Option<&T::AccountId>,
    provenance: Option<&FundingProvenance>,
    now: BlockNumberFor<T>,
  ) -> Result<Option<ActorControlLocation<BlockNumberFor<T>>>, ActorControlTransitionError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let transition = || {
        if amount.is_zero() {
          return Ok(None);
        }
        let (location, mut cell) = Self::load_primary_control_cell(actor_id)?;
        let (identity, _, admission) = Self::project_control_cell(&cell, location)
          .ok_or(ActorControlTransitionError::Invariant)?;
        let (contract, _, contract_head) =
          Self::control_load_current_step_contract(actor_id, &admission, 0)
            .ok_or(ActorControlTransitionError::Invariant)?;
        let mut funding =
          ActorFunding::<T>::get(actor_id).ok_or(ActorControlTransitionError::Invariant)?;
        let funding_authorized = Self::funding_event_authorized(
          actor_id,
          &identity.owner,
          &contract.funding,
          source,
          provenance,
        );
        if funding_authorized && funding.funding_tracked_assets.contains(&asset) {
          let accumulated = if let Some(accumulated) = funding.funding_accumulated.get_mut(&asset) {
            *accumulated = accumulated
              .checked_add(&amount)
              .ok_or(ActorControlTransitionError::Invariant)?;
            *accumulated
          } else {
            funding
              .funding_accumulated
              .try_insert(asset, amount)
              .map_err(|_| ActorControlTransitionError::Invariant)?;
            amount
          };
          Self::ensure_funding_state_hold_capacity(actor_id, &identity, &funding)
            .map_err(|_| ActorControlTransitionError::Invariant)?;
          ActorFunding::<T>::insert(actor_id, &funding);
          Self::control_reconcile_single_step_state_hold(actor_id, &cell, &contract_head, &funding)
            .map_err(|_| ActorControlTransitionError::Invariant)?;
          Self::deposit_event(Event::FundingAccumulated {
            actor_id,
            asset,
            added: amount,
            accumulated,
          });
        }
        let signal_matched = if !cell.hot.pending_signal
          && let Trigger::AddressEvent {
            source_filter,
            asset_filter,
          } = &contract.trigger
        {
          Self::source_matches_filter(source_filter, &identity.owner, source)
            && Self::asset_matches_filter(asset_filter, asset)
        } else {
          false
        };
        if !signal_matched {
          return Ok(None);
        }
        if cell.hot.trigger_wakeup_pointer.is_some() {
          return Err(ActorControlTransitionError::Invariant);
        }
        let idle_activation = location == ActorControlLocation::Unsignaled
          && cell.hot.cycle_state == CycleState::Idle
          && cell.eligible_at.is_none()
          && cell.hot.wakeup_pointer.is_none();
        let busy_deferred = matches!(
          cell.hot.cycle_state,
          CycleState::Running | CycleState::Suspended
        ) && cell.eligible_at.is_some()
          && matches!(
            location,
            ActorControlLocation::Ready { .. } | ActorControlLocation::Waiting { .. }
          );
        if !idle_activation && !busy_deferred {
          return Err(ActorControlTransitionError::Invariant);
        }
        let actor_type = identity.actor_class.actor_type();
        let breakdown = Self::trigger_fee_for_weight(
          actor_type,
          TriggerFamily::AddressEvent,
          T::WeightInfo::address_event_trigger_occurrence(),
        );
        if !Self::try_charge_automatic_trigger_occurrence(
          actor_type,
          &identity.sovereign_account,
          breakdown,
        )
        .map_err(|_| ActorControlTransitionError::Invariant)?
        {
          return Ok(None);
        }
        cell.hot.pending_signal = true;
        let destination = if busy_deferred {
          Self::store_primary_control_cell(location, cell)?;
          location
        } else {
          let eligible_at = now
            .checked_add(&One::one())
            .ok_or(ActorControlTransitionError::IndexExhausted)?;
          cell.eligible_at = Some(eligible_at);
          Self::remove_primary_control_cell_inner(actor_id)
            .map_err(|_| ActorControlTransitionError::Invariant)?;
          let destination = Self::control_append_waiting(
            cell,
            WakeupKey::Block(eligible_at),
            ActorWaitingAuthority::Service,
          )?;
          destination
        };
        Self::deposit_event(Event::TriggerOccurrenceProcessed {
          actor_id,
          trigger_family: breakdown.trigger_family,
          fee: breakdown.trigger_fee,
        });
        Ok(Some(destination))
      };
      match transition() {
        Ok(output) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(output)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  #[cfg(all(test, feature = "runtime-benchmarks"))]
  pub(crate) fn control_latch_observation_change_occurrence(
    actor_id: ActorId,
    feed: T::ObservationFeedId,
    now: BlockNumberFor<T>,
  ) -> Result<Option<ActorControlLocation<BlockNumberFor<T>>>, ActorControlTransitionError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let transition = || {
        let (location, mut cell) = Self::load_primary_control_cell(actor_id)?;
        let detector_disabled = IndexedTriggerDetectionDisabled::<T>::contains_key(actor_id);
        if cell.hot.pending_signal || detector_disabled {
          if cell.hot.pending_signal != detector_disabled {
            return Err(ActorControlTransitionError::Invariant);
          }
          return Ok(None);
        }
        if cell.hot.trigger_wakeup_pointer.is_some() {
          return Err(ActorControlTransitionError::Invariant);
        }
        let idle_activation = location == ActorControlLocation::Unsignaled
          && cell.hot.cycle_state == CycleState::Idle
          && cell.eligible_at.is_none()
          && cell.hot.wakeup_pointer.is_none();
        let busy_deferred = matches!(
          cell.hot.cycle_state,
          CycleState::Running | CycleState::Suspended
        ) && cell.eligible_at.is_some()
          && matches!(
            location,
            ActorControlLocation::Ready { .. } | ActorControlLocation::Waiting { .. }
          );
        if !idle_activation && !busy_deferred {
          return Err(ActorControlTransitionError::Invariant);
        }
        let (identity, _, admission) = Self::project_control_cell(&cell, location)
          .ok_or(ActorControlTransitionError::Invariant)?;
        let (contract, _, _) = Self::control_load_current_step_contract(actor_id, &admission, 0)
          .ok_or(ActorControlTransitionError::Invariant)?;
        if !matches!(
          contract.trigger,
          Trigger::ObservationChange { feed: contract_feed } if contract_feed == feed
        ) || !matches!(
          cell.hot.trigger_runtime_state,
          TriggerRuntimeState::Stateless
        ) || ActorObservationFeeds::<T>::get(actor_id)
          .is_none_or(|feeds| feeds.as_slice() != [feed])
          || !ObservationSubscriptionSlot::<T>::contains_key(actor_id)
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        let actor_type = identity.actor_class.actor_type();
        let breakdown = Self::trigger_fee_for_weight(
          actor_type,
          TriggerFamily::ObservationChange,
          T::WeightInfo::observation_change_trigger_occurrence(),
        );
        if !Self::try_charge_automatic_trigger_occurrence(
          actor_type,
          &identity.sovereign_account,
          breakdown,
        )
        .map_err(|_| ActorControlTransitionError::Invariant)?
        {
          return Ok(None);
        }
        cell.hot.pending_signal = true;
        let destination = if busy_deferred {
          Self::store_primary_control_cell(location, cell)?;
          location
        } else {
          let eligible_at = now
            .checked_add(&One::one())
            .ok_or(ActorControlTransitionError::IndexExhausted)?;
          cell.eligible_at = Some(eligible_at);
          Self::remove_primary_control_cell_inner(actor_id)
            .map_err(|_| ActorControlTransitionError::Invariant)?;
          let destination = Self::control_append_waiting(
            cell,
            WakeupKey::Block(eligible_at),
            ActorWaitingAuthority::Service,
          )?;
          destination
        };
        IndexedTriggerDetectionDisabled::<T>::insert(actor_id, ());
        Self::deposit_event(Event::TriggerOccurrenceProcessed {
          actor_id,
          trigger_family: breakdown.trigger_family,
          fee: breakdown.trigger_fee,
        });
        Ok(Some(destination))
      };
      match transition() {
        Ok(output) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(output)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  #[cfg(all(test, feature = "runtime-benchmarks"))]
  pub(crate) fn control_latch_observation_crossing_fire(
    actor_id: ActorId,
    transition: crate::ObservationTransition,
    now: BlockNumberFor<T>,
  ) -> Result<Option<ActorControlLocation<BlockNumberFor<T>>>, ActorControlTransitionError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let transition_result = || {
        let (location, mut cell) = Self::load_primary_control_cell(actor_id)?;
        let detector_disabled = IndexedTriggerDetectionDisabled::<T>::contains_key(actor_id);
        if cell.hot.pending_signal || detector_disabled {
          if cell.hot.pending_signal != detector_disabled {
            return Err(ActorControlTransitionError::Invariant);
          }
          return Ok(None);
        }
        if cell.hot.trigger_wakeup_pointer.is_some() {
          return Err(ActorControlTransitionError::Invariant);
        }
        let idle_activation = location == ActorControlLocation::Unsignaled
          && cell.hot.cycle_state == CycleState::Idle
          && cell.eligible_at.is_none()
          && cell.hot.wakeup_pointer.is_none();
        let busy_deferred = matches!(
          cell.hot.cycle_state,
          CycleState::Running | CycleState::Suspended
        ) && cell.eligible_at.is_some()
          && matches!(
            location,
            ActorControlLocation::Ready { .. } | ActorControlLocation::Waiting { .. }
          );
        if !idle_activation && !busy_deferred {
          return Err(ActorControlTransitionError::Invariant);
        }
        let (identity, _, admission) = Self::project_control_cell(&cell, location)
          .ok_or(ActorControlTransitionError::Invariant)?;
        let (contract, _, _) = Self::control_load_current_step_contract(actor_id, &admission, 0)
          .ok_or(ActorControlTransitionError::Invariant)?;
        let crossing = Self::crossing_from_trigger(&contract.trigger)
          .ok_or(ActorControlTransitionError::Invariant)?;
        let TriggerRuntimeState::ObservationCrossing {
          phase,
          installed_at_revision,
        } = cell.hot.trigger_runtime_state
        else {
          return Err(ActorControlTransitionError::Invariant);
        };
        let previous = transition
          .previous
          .ok_or(ActorControlTransitionError::Invariant)?;
        if installed_at_revision >= transition.revision
          || crossing.transition(phase, previous, transition.current)
            != crate::CrossingTransition::Fire
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        let locator =
          CrossingMemberships::<T>::get(actor_id).ok_or(ActorControlTransitionError::Invariant)?;
        Self::control_move_crossing_membership_without_hot(
          actor_id,
          crossing,
          CrossingPhase::WaitingForRearm,
          locator,
          identity.actor_class.actor_type(),
        )
        .map_err(|_| ActorControlTransitionError::Invariant)?;
        cell.hot.trigger_runtime_state = TriggerRuntimeState::ObservationCrossing {
          phase: CrossingPhase::WaitingForRearm,
          installed_at_revision,
        };
        let actor_type = identity.actor_class.actor_type();
        let breakdown = Self::trigger_fee_for_weight(
          actor_type,
          TriggerFamily::ObservationCrossing,
          T::WeightInfo::observation_crossing_trigger_occurrence(),
        );
        if !Self::try_charge_automatic_trigger_occurrence(
          actor_type,
          &identity.sovereign_account,
          breakdown,
        )
        .map_err(|_| ActorControlTransitionError::Invariant)?
        {
          Self::store_primary_control_cell(location, cell)?;
          return Ok(None);
        }
        cell.hot.pending_signal = true;
        let destination = if busy_deferred {
          Self::store_primary_control_cell(location, cell)?;
          location
        } else {
          let eligible_at = now
            .checked_add(&One::one())
            .ok_or(ActorControlTransitionError::IndexExhausted)?;
          cell.eligible_at = Some(eligible_at);
          Self::remove_primary_control_cell_inner(actor_id)
            .map_err(|_| ActorControlTransitionError::Invariant)?;
          let destination = Self::control_append_waiting(
            cell,
            WakeupKey::Block(eligible_at),
            ActorWaitingAuthority::Service,
          )?;
          destination
        };
        IndexedTriggerDetectionDisabled::<T>::insert(actor_id, ());
        Self::deposit_event(Event::TriggerOccurrenceProcessed {
          actor_id,
          trigger_family: breakdown.trigger_family,
          fee: breakdown.trigger_fee,
        });
        Ok(Some(destination))
      };
      match transition_result() {
        Ok(output) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(output)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  #[cfg(all(test, feature = "runtime-benchmarks"))]
  pub(crate) fn control_apply_observation_crossing_rearm(
    actor_id: ActorId,
    transition: crate::ObservationTransition,
  ) -> Result<(), ActorControlTransitionError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let transition_result = || {
        let (location, mut cell) = Self::load_primary_control_cell(actor_id)?;
        if cell.hot.pending_signal
          || IndexedTriggerDetectionDisabled::<T>::contains_key(actor_id)
          || cell.hot.trigger_wakeup_pointer.is_some()
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        let (identity, _, admission) = Self::project_control_cell(&cell, location)
          .ok_or(ActorControlTransitionError::Invariant)?;
        let (contract, _, _) = Self::control_load_current_step_contract(actor_id, &admission, 0)
          .ok_or(ActorControlTransitionError::Invariant)?;
        let crossing = Self::crossing_from_trigger(&contract.trigger)
          .ok_or(ActorControlTransitionError::Invariant)?;
        let TriggerRuntimeState::ObservationCrossing {
          phase,
          installed_at_revision,
        } = cell.hot.trigger_runtime_state
        else {
          return Err(ActorControlTransitionError::Invariant);
        };
        let previous = transition
          .previous
          .ok_or(ActorControlTransitionError::Invariant)?;
        if installed_at_revision >= transition.revision
          || crossing.transition(phase, previous, transition.current)
            != crate::CrossingTransition::Rearm
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        let locator =
          CrossingMemberships::<T>::get(actor_id).ok_or(ActorControlTransitionError::Invariant)?;
        Self::control_move_crossing_membership_without_hot(
          actor_id,
          crossing,
          CrossingPhase::Armed,
          locator,
          identity.actor_class.actor_type(),
        )
        .map_err(|_| ActorControlTransitionError::Invariant)?;
        cell.hot.trigger_runtime_state = TriggerRuntimeState::ObservationCrossing {
          phase: CrossingPhase::Armed,
          installed_at_revision,
        };
        Self::store_primary_control_cell(location, cell)
      };
      match transition_result() {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  #[cfg(all(test, feature = "runtime-benchmarks"))]
  pub(crate) fn control_latch_observation_change_page(
    feed: T::ObservationFeedId,
    page: u32,
    now: BlockNumberFor<T>,
  ) -> Result<
    Vec<(ActorId, Option<ActorControlLocation<BlockNumberFor<T>>>)>,
    ActorControlTransitionError,
  > {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let transition = || {
        let list = ObservationSubscriberPageLists::<T>::get(feed)
          .ok_or(ActorControlTransitionError::Invariant)?;
        let subscriber_page = ObservationSubscriberPages::<T>::get(feed, page)
          .ok_or(ActorControlTransitionError::Invariant)?;
        if list.count == 0
          || (subscriber_page.previous.is_none() && list.head != page)
          || (subscriber_page.next.is_none() && list.tail != page)
        {
          return Err(ActorControlTransitionError::Invariant);
        }
        let mut outcomes = Vec::new();
        for maybe_actor_id in subscriber_page.entries.iter(/* deos-bypass: bounded-iter */) {
          let Some(actor_id) = maybe_actor_id else {
            continue;
          };
          let outcome = Self::control_latch_observation_change_occurrence(*actor_id, feed, now)?;
          outcomes.push((*actor_id, outcome));
        }
        Ok(outcomes)
      };
      match transition() {
        Ok(output) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(output)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  #[cfg(all(test, feature = "runtime-benchmarks"))]
  pub(crate) fn control_latch_due_temporal_reference(
    key: WakeupKey<BlockNumberFor<T>>,
    now: BlockNumberFor<T>,
    now_tick: SchedulerTick,
  ) -> Result<(ActorId, ActorControlLocation<BlockNumberFor<T>>), ActorControlTransitionError> {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let transition = || {
        if !matches!(key, WakeupKey::Tick(_)) {
          return Err(ActorControlTransitionError::Invariant);
        }
        let (location, mut cell) = Self::control_due_wakeup_primary(key, now, now_tick)?;
        let idle_activation = location == ActorControlLocation::Unsignaled
          && cell.hot.cycle_state == CycleState::Idle
          && !cell.hot.pending_signal
          && cell.eligible_at.is_none()
          && cell.hot.wakeup_pointer.is_none();
        let busy_deferred = matches!(
          cell.hot.cycle_state,
          CycleState::Running | CycleState::Suspended
        ) && !cell.hot.pending_signal
          && cell.eligible_at.is_some()
          && matches!(
            location,
            ActorControlLocation::Ready { .. } | ActorControlLocation::Waiting { .. }
          );
        if !idle_activation && !busy_deferred {
          return Err(ActorControlTransitionError::Invariant);
        }
        let actor_id = cell.actor_id;
        let (identity, _, admission) = Self::project_control_cell(&cell, location)
          .ok_or(ActorControlTransitionError::Invariant)?;
        let (contract, _) = Self::control_load_frame_contract(actor_id, &admission)
          .ok_or(ActorControlTransitionError::Invariant)?;
        let (trigger_family, occurrence_weight) = match &contract.trigger {
          Trigger::AtTime { .. } => (
            TriggerFamily::AtTime,
            T::WeightInfo::at_time_trigger_occurrence(),
          ),
          Trigger::Cadenced { .. } => (
            TriggerFamily::Cadenced,
            T::WeightInfo::cadenced_trigger_occurrence(),
          ),
          _ => return Err(ActorControlTransitionError::Invariant),
        };
        cell.hot.trigger_runtime_state = match (trigger_family, cell.hot.trigger_runtime_state) {
          (
            TriggerFamily::AtTime,
            TriggerRuntimeState::AtTime {
              anchor_tick: Some(anchor_tick),
              consumed: false,
            },
          ) if idle_activation => TriggerRuntimeState::AtTime {
            anchor_tick: Some(anchor_tick),
            consumed: true,
          },
          (
            TriggerFamily::Cadenced,
            state @ TriggerRuntimeState::Cadenced {
              anchor_tick: Some(_),
            },
          ) => state,
          _ => return Err(ActorControlTransitionError::Invariant),
        };
        let actor_type = identity.actor_class.actor_type();
        let breakdown = Self::trigger_fee_for_weight(actor_type, trigger_family, occurrence_weight);
        let charged = if trigger_family == TriggerFamily::AtTime {
          if !Self::trigger_occurrence_capacity_sufficient(
            actor_type,
            &identity.sovereign_account,
            breakdown,
          )
          .map_err(|_| ActorControlTransitionError::Invariant)?
          {
            let cell = Self::control_consume_due_wakeup_reference(cell, key, now, now_tick)?;
            Self::control_finalize_underfunded_at_time(
              actor_id, location, cell, identity, &contract,
            )?;
            return Ok((actor_id, location));
          }
          Self::try_charge_prechecked_automatic_trigger_occurrence(
            actor_type,
            &identity.sovereign_account,
            breakdown,
          )
        } else {
          Self::try_charge_automatic_trigger_occurrence(
            actor_type,
            &identity.sovereign_account,
            breakdown,
          )
        }
        .map_err(|_| ActorControlTransitionError::Invariant)?;
        if !charged {
          if trigger_family != TriggerFamily::Cadenced {
            return Err(ActorControlTransitionError::Invariant);
          }
          let Trigger::Cadenced { every_ticks } = contract.trigger else {
            return Err(ActorControlTransitionError::Invariant);
          };
          let anchor_tick = cell
            .hot
            .trigger_runtime_state
            .temporal_anchor_tick()
            .ok_or(ActorControlTransitionError::Invariant)?;
          let next_due_tick = next_cadence_due_tick(anchor_tick, every_ticks, now_tick)
            .ok_or(ActorControlTransitionError::Invariant)?;
          let cell = Self::control_consume_due_wakeup_reference(cell, key, now, now_tick)?;
          let cell =
            Self::control_schedule_fresh_wakeup_reference(cell, WakeupKey::Tick(next_due_tick))?;
          Self::store_primary_control_cell(location, cell)?;
          return Ok((actor_id, location));
        }
        let mut cell = Self::control_consume_due_wakeup_reference(cell, key, now, now_tick)?;
        cell.hot.pending_signal = true;
        let destination = if busy_deferred {
          Self::store_primary_control_cell(location, cell)?;
          location
        } else {
          let eligible_at = now
            .checked_add(&One::one())
            .ok_or(ActorControlTransitionError::IndexExhausted)?;
          cell.eligible_at = Some(eligible_at);
          Self::remove_primary_control_cell_inner(actor_id)
            .map_err(|_| ActorControlTransitionError::Invariant)?;
          let destination = Self::control_append_waiting(
            cell,
            WakeupKey::Block(eligible_at),
            ActorWaitingAuthority::Service,
          )?;
          destination
        };
        Self::deposit_event(Event::TriggerOccurrenceProcessed {
          actor_id,
          trigger_family: breakdown.trigger_family,
          fee: breakdown.trigger_fee,
        });
        Ok((actor_id, destination))
      };
      match transition() {
        Ok(output) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(output)),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub(crate) fn trigger_wakeup_substrate_invalidate_inner(
    actor_id: ActorId,
  ) -> Result<Option<WakeupPointer<BlockNumberFor<T>>>, EnqueueOutcome> {
    Self::wakeup_substrate_invalidate_clock_inner(actor_id, WakeupClock::Tick)
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  pub fn wakeup_substrate_invalidate(
    actor_id: ActorId,
  ) -> Option<WakeupPointer<BlockNumberFor<T>>> {
    let result: Result<WakeupPointer<BlockNumberFor<T>>, DispatchError> =
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Self::wakeup_substrate_invalidate_clock_inner(actor_id, WakeupClock::Block) {
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

  #[cfg(any(test, feature = "runtime-benchmarks"))]
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

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  fn try_wakeup_substrate_schedule_key_inner(
    actor_id: ActorId,
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
  ) -> Result<(), EnqueueOutcome> {
    with_transaction_opaque_err(|| {
      match Self::schedule_retained_wakeup_transition(actor_id, wakeup_key) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
  fn schedule_retained_wakeup_transition(
    actor_id: ActorId,
    wakeup_block: WakeupKey<BlockNumberFor<T>>,
  ) -> Result<(), EnqueueOutcome> {
    let Some((state, admission, loaded_step)) = Self::load_frame_actor_service_state(actor_id)
    else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    let resources = if state.contract.steps.is_empty() {
      ActorStepResourceEnvelope {
        control: T::WeightInfo::scheduler_inner_zero_step_complete(),
        effect: Weight::zero(),
      }
    } else {
      loaded_step
        .ok_or(EnqueueOutcome::CorruptedTopology)?
        .resources
    };
    Self::try_wakeup_substrate_schedule_transition_with_authority(
      actor_id,
      wakeup_block,
      state.hot,
      &state.identity,
      state.run_state.as_ref().map_or(0, |run| run.cursor),
      &admission,
      resources,
    )
  }

  fn publish_waiting_from_authority(
    actor_id: ActorId,
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
    hot: ActorHotStateOf<T>,
    identity: &ActorIdentityOf<T>,
    cursor: u32,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
  ) -> Result<(), EnqueueOutcome> {
    if ActorControlLocators::<T>::contains_key(actor_id) {
      return Err(EnqueueOutcome::AlreadyLive);
    }
    let (authority, eligible_at) = match wakeup_key {
      WakeupKey::Block(block) => (ActorWaitingAuthority::Service, Some(block)),
      WakeupKey::Tick(_) => (ActorWaitingAuthority::Trigger, None),
    };
    let cell = ActorControlCell {
      actor_id,
      identity: Self::control_identity_from_scalar(identity.clone())
        .ok_or(EnqueueOutcome::CorruptedTopology)?,
      hot: Self::control_hot_from_scalar(hot),
      cursor,
      eligible_at,
      admission: admission.clone(),
      resources,
    };
    Self::control_append_waiting(cell, wakeup_key, authority)
      .map(|_| ())
      .map_err(|_| EnqueueOutcome::CorruptedTopology)
  }

  pub(crate) fn try_wakeup_substrate_schedule_transition_with_authority(
    actor_id: ActorId,
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
    hot: ActorHotStateOf<T>,
    identity: &ActorIdentityOf<T>,
    cursor: u32,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
  ) -> Result<(), EnqueueOutcome> {
    with_transaction_opaque_err(|| {
      match Self::schedule_wakeup_transition_with_authority_inner(
        actor_id, wakeup_key, hot, identity, cursor, admission, resources,
      ) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
  }

  fn schedule_wakeup_transition_with_authority_inner(
    actor_id: ActorId,
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
    mut hot: ActorHotStateOf<T>,
    identity: &ActorIdentityOf<T>,
    cursor: u32,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
  ) -> Result<(), EnqueueOutcome> {
    let clock = wakeup_key.clock();
    if let Some(pointer) = Self::wakeup_pointer_for_clock(&hot, clock) {
      if pointer.block == wakeup_key && Self::wakeup_page_entry_matches(pointer, actor_id) {
        if let Some(location) = ActorControlLocators::<T>::get(actor_id) {
          let (_, mut source) = Self::load_primary_control_cell(actor_id)
            .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
          if source.admission != *admission {
            return Err(EnqueueOutcome::CorruptedTopology);
          }
          source.identity = Self::control_identity_from_scalar(identity.clone())
            .ok_or(EnqueueOutcome::CorruptedTopology)?;
          source.hot = Self::control_hot_from_scalar(hot);
          source.cursor = cursor;
          source.resources = resources;
          return Self::store_primary_control_cell(location, source)
            .map_err(|_| EnqueueOutcome::CorruptedTopology);
        }
        return Self::publish_waiting_from_authority(
          actor_id, wakeup_key, hot, identity, cursor, admission, resources,
        );
      }
      Self::invalidate_wakeup_reference(actor_id, pointer, admission.admission_identity)?;
      Self::clear_wakeup_pointer_for_clock(&mut hot, clock);
    }
    let mut retained_primary = None;
    if let Some(location) = ActorControlLocators::<T>::get(actor_id) {
      let (_, source) =
        Self::load_primary_control_cell(actor_id).map_err(|_| EnqueueOutcome::CorruptedTopology)?;
      let mut expected_identity = Self::control_identity_from_scalar(identity.clone())
        .ok_or(EnqueueOutcome::CorruptedTopology)?;
      expected_identity.cycle_nonce = source.identity.cycle_nonce;
      if source.identity != expected_identity || source.admission != *admission {
        return Err(EnqueueOutcome::CorruptedTopology);
      }
      if location == ActorControlLocation::Unsignaled
        || matches!(location, ActorControlLocation::Waiting { key, .. } if key.clock() == clock)
      {
        Self::remove_primary_control_cell_inner(actor_id)
          .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
      } else {
        retained_primary = Some((location, source));
      }
    }
    let (page_id, slot) =
      Self::schedule_fresh_wakeup_reference(actor_id, wakeup_key, admission.admission_identity)?;
    hot = Self::with_wakeup_pointer(hot, wakeup_key, page_id, slot);
    if let Some((location, mut source)) = retained_primary {
      source.identity = Self::control_identity_from_scalar(identity.clone())
        .ok_or(EnqueueOutcome::CorruptedTopology)?;
      source.hot = Self::control_hot_from_scalar(hot);
      source.cursor = cursor;
      source.resources = resources;
      return Self::store_primary_control_cell(location, source)
        .map_err(|_| EnqueueOutcome::CorruptedTopology);
    }
    Self::publish_waiting_from_authority(
      actor_id, wakeup_key, hot, identity, cursor, admission, resources,
    )
  }

  fn schedule_fresh_wakeup_reference(
    actor_id: ActorId,
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
    admission_identity: [u8; 32],
  ) -> Result<(WakeupPageId, WakeupSlot), EnqueueOutcome> {
    #[cfg(test)]
    if FAIL_WAKEUP_PLACEMENT_WITH_CAPACITY.with(|flag| flag.replace(false)) {
      return Err(EnqueueOutcome::WakeupCapacityExhausted);
    }
    Self::append_waiting_entry(wakeup_key, |_, _| {
      ActorWaitingEntry::Reference(ActorWakeupReference {
        actor_id,
        admission_identity,
      })
    })
    .map_err(|error| match error {
      ActorControlTransitionError::IndexExhausted => EnqueueOutcome::WakeupIndexExhausted,
      _ => EnqueueOutcome::CorruptedTopology,
    })
  }

  #[cfg(all(test, feature = "runtime-benchmarks"))]
  pub(crate) fn control_schedule_fresh_wakeup_reference(
    mut cell: ActorControlCellOf<T>,
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
  ) -> Result<ActorControlCellOf<T>, ActorControlTransitionError> {
    let pointer_exists = match wakeup_key {
      WakeupKey::Block(_) => cell.hot.wakeup_pointer.is_some(),
      WakeupKey::Tick(_) => cell.hot.trigger_wakeup_pointer.is_some(),
    };
    if pointer_exists {
      return Err(ActorControlTransitionError::Invariant);
    }
    let (page_id, slot) = Self::schedule_fresh_wakeup_reference(
      cell.actor_id,
      wakeup_key,
      cell.admission.admission_identity,
    )
    .map_err(|error| match error {
      EnqueueOutcome::WakeupIndexExhausted | EnqueueOutcome::SchedulerIndexExhausted => {
        ActorControlTransitionError::IndexExhausted
      }
      _ => ActorControlTransitionError::Invariant,
    })?;
    match wakeup_key {
      WakeupKey::Block(_) => {
        cell.hot.wakeup_pointer = Some(WakeupPointer {
          block: wakeup_key,
          page_id,
          slot,
        });
      }
      WakeupKey::Tick(tick) => {
        cell.hot.trigger_wakeup_pointer = Some(TriggerWakeupPointer {
          tick,
          page_id,
          slot,
        });
      }
    }
    Ok(cell)
  }

  fn with_wakeup_pointer(
    mut hot: ActorHotStateOf<T>,
    block: WakeupKey<BlockNumberFor<T>>,
    page_id: WakeupPageId,
    slot: WakeupSlot,
  ) -> ActorHotStateOf<T> {
    match block {
      WakeupKey::Block(_) => {
        hot.wakeup_pointer = Some(WakeupPointer {
          block,
          page_id,
          slot,
        });
      }
      WakeupKey::Tick(tick) => {
        hot.trigger_wakeup_pointer = Some(TriggerWakeupPointer {
          tick,
          page_id,
          slot,
        });
      }
    }
    hot
  }

  #[cfg(any(test, feature = "runtime-benchmarks"))]
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
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
    max_entries_scanned: u32,
  ) -> Option<(
    BoundedVec<
      (
        ActorId,
        ActiveActorStateOf<T>,
        ActorAdmissionCertificateOf<T>,
        Option<LoadedActorStepOf<T>>,
      ),
      T::MaxWakeupsPerBlock,
    >,
    WakeupDrainStats,
  )> {
    let mut ready = BoundedVec::<
      (
        ActorId,
        ActiveActorStateOf<T>,
        ActorAdmissionCertificateOf<T>,
        Option<LoadedActorStepOf<T>>,
      ),
      T::MaxWakeupsPerBlock,
    >::default();
    let mut stats = WakeupDrainStats::default();
    let limit = max_entries_scanned.min(T::MaxWakeupsPerBlock::get());
    let mut last_page = None;
    while stats.entries_scanned < limit && ActorWaitingOccupancies::<T>::get(wakeup_key) > 0 {
      let cursor_index = ActorWaitingCursorIndices::<T>::get(wakeup_key)?;
      if Self::wakeup_cursor_get(wakeup_key.clock(), cursor_index) != Some(wakeup_key) {
        return None;
      }
      let head = ActorWaitingHeads::<T>::get(wakeup_key);
      let page_id = head / 32;
      let mut page = ActorWaitingFrameChunks::<T>::get((wakeup_key, page_id))?;
      if page.previous_page.is_some()
        || page.entries.len() != 32
        || page.live_entries == 0
        || page.entries.iter(/* deos-bypass: bounded-iter */).filter(|entry| entry.is_some()).count()
          != page.live_entries as usize
      {
        return None;
      }
      if last_page != Some(page_id) {
        stats.pages_touched = stats.pages_touched.saturating_add(1);
        last_page = Some(page_id);
      }
      let slot = page.scan_slot;
      if slot >= 32 || head % 32 != u64::from(slot) {
        return None;
      }
      let entry = page.entries.get(slot as usize)?.clone();
      let pointer = WakeupPointer {
        block: wakeup_key,
        page_id,
        slot,
      };
      let frozen = entry.as_ref().and_then(|entry| {
        let actor_id = match entry {
          ActorWaitingEntry::Primary(cell) => cell.actor_id,
          ActorWaitingEntry::Reference(reference) => reference.actor_id,
        };
        Self::load_frame_actor_service_state(actor_id)
      });
      page.scan_slot = slot.checked_add(1)?;
      stats.entries_scanned = stats.entries_scanned.saturating_add(1);
      ActorWaitingFrameChunks::<T>::insert((wakeup_key, page_id), page);
      ActorWaitingHeads::<T>::insert(wakeup_key, head.checked_add(1)?);
      let Some(entry) = entry else {
        continue;
      };
      let (actor_id, expected_admission, is_primary) = match entry {
        ActorWaitingEntry::Primary(cell) => {
          (cell.actor_id, cell.admission.admission_identity, true)
        }
        ActorWaitingEntry::Reference(reference) => {
          (reference.actor_id, reference.admission_identity, false)
        }
      };
      let (mut state, admission, loaded_step) = match frozen {
        Some((state, admission, loaded_step))
          if admission.admission_identity == expected_admission
            && Self::wakeup_pointer_for_clock(&state.hot, wakeup_key.clock()) == Some(pointer) =>
        {
          (state, admission, loaded_step)
        }
        None if !is_primary && !ActorControlLocators::<T>::contains_key(actor_id) => {
          Self::remove_waiting_entry(pointer).ok()?;
          stats.stale_entries = stats.stale_entries.saturating_add(1);
          if !ActorWaitingFrameChunks::<T>::contains_key((wakeup_key, page_id)) {
            stats.pages_deleted = stats.pages_deleted.saturating_add(1);
          }
          continue;
        }
        Some((state, _, _))
          if !is_primary
            && Self::wakeup_pointer_for_clock(&state.hot, wakeup_key.clock()).is_none() =>
        {
          Self::remove_waiting_entry(pointer).ok()?;
          stats.stale_entries = stats.stale_entries.saturating_add(1);
          if !ActorWaitingFrameChunks::<T>::contains_key((wakeup_key, page_id)) {
            stats.pages_deleted = stats.pages_deleted.saturating_add(1);
          }
          continue;
        }
        _ => return None,
      };
      Self::clear_wakeup_pointer_for_clock(&mut state.hot, wakeup_key.clock());
      if !is_primary {
        Self::remove_waiting_entry(pointer).ok()?;
      }
      Self::consume_waiting_from_supplied_authority(actor_id, wakeup_key, &state.hot).ok()?;
      ready
        .try_push((actor_id, state, admission, loaded_step))
        .ok()?;
      stats.ready_entries = stats.ready_entries.saturating_add(1);
      if !ActorWaitingFrameChunks::<T>::contains_key((wakeup_key, page_id)) {
        stats.pages_deleted = stats.pages_deleted.saturating_add(1);
      }
    }
    Some((ready, stats))
  }

  pub(crate) fn wakeup_substrate_drain_key(
    wakeup_key: WakeupKey<BlockNumberFor<T>>,
    max_entries_scanned: u32,
  ) -> (
    BoundedVec<
      (
        ActorId,
        ActiveActorStateOf<T>,
        ActorAdmissionCertificateOf<T>,
        Option<LoadedActorStepOf<T>>,
      ),
      T::MaxWakeupsPerBlock,
    >,
    WakeupDrainStats,
  ) {
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
    let (loaded, stats) =
      Self::wakeup_substrate_drain_key(WakeupKey::Block(wakeup_block), max_entries_scanned);
    let ready = BoundedVec::truncate_from(
      loaded
        .into_iter()
        .map(|(actor_id, _, _, _)| actor_id)
        .collect(),
    );
    (ready, stats)
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

  pub(crate) fn wakeup_cursor_owner_index(
    key: WakeupKey<BlockNumberFor<T>>,
  ) -> Option<WakeupCursorIndex> {
    ActorWaitingCursorIndices::<T>::get(key)
  }

  fn wakeup_cursor_has_owner(key: WakeupKey<BlockNumberFor<T>>) -> bool {
    ActorWaitingOccupancies::<T>::get(key) > 0 || ActorWaitingCursorIndices::<T>::contains_key(key)
  }

  fn wakeup_cursor_write_owner_index(
    key: WakeupKey<BlockNumberFor<T>>,
    index: Option<WakeupCursorIndex>,
  ) -> bool {
    let mut wrote = false;
    if ActorWaitingOccupancies::<T>::get(key) > 0
      || ActorWaitingCursorIndices::<T>::contains_key(key)
    {
      match index {
        Some(index) => ActorWaitingCursorIndices::<T>::insert(key, index),
        None => ActorWaitingCursorIndices::<T>::remove(key),
      }
      wrote = true;
    }
    wrote
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
    if Self::wakeup_cursor_owner_index(left_block) != Some(left)
      || Self::wakeup_cursor_owner_index(right_block) != Some(right)
    {
      return false;
    }
    if !Self::wakeup_cursor_set(clock, left, right_block)
      || !Self::wakeup_cursor_set(clock, right, left_block)
    {
      return false;
    }
    Self::wakeup_cursor_write_owner_index(right_block, Some(left))
      && Self::wakeup_cursor_write_owner_index(left_block, Some(right))
  }

  fn wakeup_cursor_height_bound() -> u32 {
    u32::BITS.saturating_sub(T::MaxActiveActors::get().max(1).leading_zeros())
  }

  fn wakeup_cursor_insert_inner(block: WakeupKey<BlockNumberFor<T>>) -> bool {
    let clock = block.clock();
    if !Self::wakeup_cursor_has_owner(block) {
      return false;
    }
    if let Some(index) = Self::wakeup_cursor_owner_index(block) {
      return Self::wakeup_cursor_get(clock, index) == Some(block)
        && Self::wakeup_cursor_write_owner_index(block, Some(index));
    }
    let len = WakeupCursorLen::<T>::get(clock);
    let Some(next_len) = len.checked_add(1) else {
      return false;
    };
    if len >= T::MaxActiveActors::get() || !Self::wakeup_cursor_set(clock, len, block) {
      return false;
    }
    if !Self::wakeup_cursor_write_owner_index(block, Some(len)) {
      return false;
    }
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
    let Some(index) = Self::wakeup_cursor_owner_index(block) else {
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
    if Self::wakeup_cursor_owner_index(last_block) != Some(last_index)
      || !Self::wakeup_cursor_remove_tail(clock, last_index)
      || !Self::wakeup_cursor_write_owner_index(block, None)
    {
      return false;
    }
    WakeupCursorLen::<T>::insert(clock, last_index);
    if index == last_index {
      return true;
    }
    if !Self::wakeup_cursor_set(clock, index, last_block)
      || !Self::wakeup_cursor_write_owner_index(last_block, Some(index))
    {
      return false;
    }

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

  pub(crate) fn control_wakeup_cursor_release(key: WakeupKey<BlockNumberFor<T>>) -> bool {
    let Some(index) = ActorWaitingCursorIndices::<T>::get(key) else {
      return false;
    };
    if Self::wakeup_cursor_get(key.clock(), index) != Some(key) {
      return false;
    }
    Self::wakeup_cursor_remove_inner(key)
  }

  pub fn wakeup_cursor_remove(block: BlockNumberFor<T>) -> bool {
    let result: DispatchResult = polkadot_sdk::frame_support::storage::with_transaction(|| {
      let key = WakeupKey::Block(block);
      if ActorWaitingOccupancies::<T>::get(key) > 0 {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          Error::<T>::ActorNotFound.into(),
        ));
      }
      if Self::wakeup_cursor_remove_inner(key) {
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
    if ActorWaitingOccupancies::<T>::get(min_block) > 0 {
      return None;
    }
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

  pub(crate) fn prime_initial_actor_schedule(actor_id: ActorId) -> Result<(), EnqueueOutcome> {
    let Some((mut state, admission, loaded_step)) = Self::load_frame_actor_service_state(actor_id)
    else {
      return match (
        Self::control_identity_exists(actor_id),
        Self::control_hot_exists(actor_id),
        ActorContractHeads::<T>::contains_key(actor_id),
        Self::control_admission_exists(actor_id),
        ActorFunding::<T>::contains_key(actor_id),
        ActorRunStateStore::<T>::contains_key(actor_id),
      ) {
        (false, false, false, false, false, false) | (true, false, false, false, false, false) => {
          Ok(())
        }
        _ => Err(EnqueueOutcome::CorruptedTopology),
      };
    };
    let mut instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    let resources = if state.contract.steps.is_empty() {
      ActorStepResourceEnvelope {
        control: T::WeightInfo::scheduler_inner_zero_step_complete(),
        effect: Weight::zero(),
      }
    } else {
      loaded_step
        .ok_or(EnqueueOutcome::CorruptedTopology)?
        .resources
    };
    let now = frame_system::Pallet::<T>::block_number();
    if !instance.lifecycle.is_paused() && instance.trigger_wakeup_pointer.is_none() {
      if let Some(due_tick) = Self::initial_trigger_wakeup_tick(&instance)? {
        #[cfg(test)]
        if FAIL_WAKEUP_PLACEMENT_WITH_CAPACITY.with(|flag| flag.replace(false)) {
          return Err(EnqueueOutcome::WakeupCapacityExhausted);
        }
        match Self::try_wakeup_substrate_schedule_transition_with_authority(
          actor_id,
          WakeupKey::Tick(due_tick),
          state.hot.clone(),
          &state.identity,
          state.run_state.as_ref().map_or(0, |run| run.cursor),
          &admission,
          resources,
        ) {
          Ok(()) | Err(EnqueueOutcome::AlreadyLive) => {}
          Err(error) => return Err(error),
        }
        let (_, _, hot, _) =
          Self::load_frame_control_authority(actor_id).ok_or(EnqueueOutcome::CorruptedTopology)?;
        state.hot = hot;
        instance = Self::derive_active_actor_view(
          state.identity.clone(),
          state.hot.clone(),
          state.contract.clone(),
        );
      }
    }
    let placement = Self::schedule_next_work_with_authority(
      actor_id,
      &instance,
      state.hot,
      &state.identity,
      state.run_state.as_ref(),
      &admission,
      resources,
      now,
      ServiceCutoff::Open,
    )?;
    if placement == StepControlPlacement::None
      && Self::load_frame_control_authority(actor_id).is_none()
    {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    Ok(())
  }

  pub(crate) fn demote_ready_frame_to_unsignaled(actor_id: ActorId) -> Result<(), EnqueueOutcome> {
    let (location, mut cell) =
      Self::load_primary_control_cell(actor_id).map_err(|_| EnqueueOutcome::CorruptedTopology)?;
    if !matches!(location, ActorControlLocation::Ready { .. })
      || cell.hot.cycle_state != CycleState::Idle
    {
      return Ok(());
    }
    Self::remove_primary_control_cell_inner(actor_id)
      .map_err(|_| EnqueueOutcome::CorruptedTopology)?;
    cell.eligible_at = None;
    ActorUnsignaledControlCells::<T>::insert(actor_id, cell);
    ActorControlLocators::<T>::insert(actor_id, ActorControlLocation::Unsignaled);
    Ok(())
  }

  pub(crate) fn prime_frame_actor_schedule(actor_id: ActorId) -> Result<(), EnqueueOutcome> {
    let Some((state, admission, loaded_step)) = Self::load_frame_actor_service_state(actor_id)
    else {
      return Err(EnqueueOutcome::CorruptedTopology);
    };
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    if instance.queue_ticket.is_some() {
      return Ok(());
    }
    let prime = Self::preflight_prime_schedule_loaded(&instance, state.run_state.as_ref());
    match prime? {
      PrimeSchedulePlan::Enqueue => {
        let plan = Self::preflight_activation_enqueue(actor_id, &state, &admission)?;
        Self::commit_paged_enqueue_transactional(plan)
      }
      PrimeSchedulePlan::BlockWakeup(block) => {
        let resources = if state.contract.steps.is_empty() {
          ActorStepResourceEnvelope {
            control: T::WeightInfo::scheduler_inner_zero_step_complete(),
            effect: Weight::zero(),
          }
        } else {
          loaded_step
            .as_ref()
            .map(|loaded| loaded.resources)
            .ok_or(EnqueueOutcome::CorruptedTopology)?
        };
        Self::defer_wakeup_with_authority(
          actor_id,
          block,
          &instance,
          state.hot,
          &state.identity,
          state.run_state.as_ref(),
          &admission,
          resources,
        )
      }
      PrimeSchedulePlan::None => {
        if state.hot.trigger_wakeup_pointer.is_some() {
          return Ok(());
        }
        let due_tick = Self::initial_trigger_wakeup_tick(&instance)?;
        if let Some(due_tick) = due_tick {
          let resources = if state.contract.steps.is_empty() {
            ActorStepResourceEnvelope {
              control: T::WeightInfo::scheduler_inner_zero_step_complete(),
              effect: Weight::zero(),
            }
          } else {
            loaded_step
              .as_ref()
              .map(|loaded| loaded.resources)
              .ok_or(EnqueueOutcome::CorruptedTopology)?
          };
          Self::try_wakeup_substrate_schedule_transition_with_authority(
            actor_id,
            WakeupKey::Tick(due_tick),
            state.hot,
            &state.identity,
            state.run_state.as_ref().map_or(0, |run| run.cursor),
            &admission,
            resources,
          )
        } else {
          Ok(())
        }
      }
    }
  }

  pub(crate) fn publish_resumed_frame(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    admission: ActorAdmissionCertificateOf<T>,
    loaded_step: Option<LoadedActorStepOf<T>>,
  ) -> Result<(), EnqueueOutcome> {
    let resources = if state.contract.steps.is_empty() {
      ActorStepResourceEnvelope {
        control: T::WeightInfo::scheduler_inner_zero_step_complete(),
        effect: Weight::zero(),
      }
    } else {
      loaded_step
        .as_ref()
        .map(|loaded| loaded.resources)
        .ok_or(EnqueueOutcome::CorruptedTopology)?
    };
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    let placement = Self::schedule_next_work_with_authority(
      actor_id,
      &instance,
      state.hot.clone(),
      &state.identity,
      state.run_state.as_ref(),
      &admission,
      resources,
      frame_system::Pallet::<T>::block_number(),
      ServiceCutoff::Open,
    )?;
    if placement == StepControlPlacement::None {
      Self::restore_unsignaled_from_authority(
        actor_id,
        state.hot,
        &state.identity,
        state.run_state.as_ref(),
        &admission,
        resources,
      )?;
      Self::prime_frame_actor_schedule(actor_id)?;
    }
    Ok(())
  }

  fn preflight_prime_schedule_loaded(
    instance: &ActiveActorViewOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
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
        run_state.ok_or(EnqueueOutcome::CorruptedTopology)?,
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

  fn initial_trigger_wakeup_tick(
    instance: &ActiveActorViewOf<T>,
  ) -> Result<Option<SchedulerTick>, EnqueueOutcome> {
    Ok(match instance.trigger {
      Trigger::AtTime { after_ticks } if !instance.temporal_occurrence_consumed => {
        Some(instance.temporal_anchor_tick.map_or(Ok(0), |anchor_tick| {
          anchor_tick
            .checked_add(after_ticks)
            .ok_or(EnqueueOutcome::SchedulerIndexExhausted)
        })?)
      }
      Trigger::Cadenced { every_ticks } => Some(
        instance
          .temporal_anchor_tick
          .map(|anchor_tick| {
            next_cadence_due_tick(anchor_tick, every_ticks, Self::current_scheduler_tick()?)
              .ok_or(EnqueueOutcome::SchedulerIndexExhausted)
          })
          .transpose()?
          .unwrap_or(0),
      ),
      Trigger::Manual
      | Trigger::AddressEvent { .. }
      | Trigger::ObservationChange { .. }
      | Trigger::ObservationCrossing { .. }
      | Trigger::AtTime { .. } => None,
    })
  }

  fn window_expiry_wakeup(instance: &ActiveActorViewOf<T>) -> Option<BlockNumberFor<T>> {
    instance
      .window
      .map(|window| Self::window_terminal_at(&window))
  }

  fn schedule_window_expiry_with_authority(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
    loaded_authority: (
      &ActorHotStateOf<T>,
      &ActorIdentityOf<T>,
      Option<&ActorRunStateOf<T>>,
      &ActorAdmissionCertificateOf<T>,
      ActorStepResourceEnvelope,
    ),
  ) -> Result<(), EnqueueOutcome> {
    let Some(expiry) = Self::window_expiry_wakeup(instance) else {
      return Ok(());
    };
    let (hot, identity, run_state, admission, resources) = loaded_authority;
    Self::defer_wakeup_with_authority(
      actor_id,
      expiry,
      instance,
      hot.clone(),
      identity,
      run_state,
      admission,
      resources,
    )
  }

  #[cfg(test)]
  pub(crate) fn test_fail_wakeup_placement_with_capacity() {
    FAIL_WAKEUP_PLACEMENT_WITH_CAPACITY.with(|flag| flag.set(true));
  }

  #[cfg(test)]
  fn defer_retained_wakeup(
    actor_id: ActorId,
    wakeup_block: BlockNumberFor<T>,
  ) -> Result<(), EnqueueOutcome> {
    let (state, admission, loaded_step) =
      Self::load_frame_actor_service_state(actor_id).ok_or(EnqueueOutcome::CorruptedTopology)?;
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    let resources = if state.contract.steps.is_empty() {
      ActorStepResourceEnvelope {
        control: T::WeightInfo::scheduler_inner_zero_step_complete(),
        effect: Weight::zero(),
      }
    } else {
      loaded_step
        .ok_or(EnqueueOutcome::CorruptedTopology)?
        .resources
    };
    Self::defer_wakeup_with_authority(
      actor_id,
      wakeup_block,
      &instance,
      state.hot,
      &state.identity,
      state.run_state.as_ref(),
      &admission,
      resources,
    )
  }

  fn defer_wakeup_with_authority(
    actor_id: ActorId,
    wakeup_block: BlockNumberFor<T>,
    instance: &ActiveActorViewOf<T>,
    hot: ActorHotStateOf<T>,
    identity: &ActorIdentityOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
  ) -> Result<(), EnqueueOutcome> {
    #[cfg(test)]
    if FAIL_WAKEUP_PLACEMENT_WITH_CAPACITY.with(|flag| flag.replace(false)) {
      return Err(EnqueueOutcome::WakeupCapacityExhausted);
    }
    let target = Self::window_expiry_wakeup(instance)
      .map(|expiry| wakeup_block.min(expiry))
      .unwrap_or(wakeup_block);
    match with_transaction_opaque_err(|| {
      match Self::try_wakeup_substrate_schedule_transition_with_authority(
        actor_id,
        WakeupKey::Block(target),
        hot,
        identity,
        run_state.map_or(0, |run| run.cursor),
        admission,
        resources,
      ) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
    {
      Ok(()) | Err(EnqueueOutcome::AlreadyLive) => Ok(()),
      Err(other) => Err(other),
    }
  }

  fn defer_activation_wakeup(
    actor_id: ActorId,
    wakeup_block: BlockNumberFor<T>,
    instance: &ActiveActorViewOf<T>,
    hot: ActorHotStateOf<T>,
    source: &ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
  ) -> Result<(), EnqueueOutcome> {
    #[cfg(test)]
    if FAIL_WAKEUP_PLACEMENT_WITH_CAPACITY.with(|flag| flag.replace(false)) {
      return Err(EnqueueOutcome::WakeupCapacityExhausted);
    }
    let target = Self::window_expiry_wakeup(instance)
      .map(|expiry| wakeup_block.min(expiry))
      .unwrap_or(wakeup_block);
    match with_transaction_opaque_err(|| {
      let transition = || {
        let cursor = source.run_state.as_ref().map_or(0, |run| run.cursor);
        let resources = if source.contract.steps.is_empty() {
          ActorStepResourceEnvelope {
            control: T::WeightInfo::scheduler_inner_zero_step_complete(),
            effect: Weight::zero(),
          }
        } else {
          Self::load_current_step_with_admission(actor_id, cursor, admission)
            .ok_or(EnqueueOutcome::CorruptedTopology)?
            .resources
        };
        Self::try_wakeup_substrate_schedule_transition_with_authority(
          actor_id,
          WakeupKey::Block(target),
          hot,
          &source.identity,
          cursor,
          admission,
          resources,
        )
      };
      match transition() {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .map_err(|_| EnqueueOutcome::CorruptedTopology)?
    {
      Ok(()) | Err(EnqueueOutcome::AlreadyLive) => Ok(()),
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

  /// Conservatively prices terminal deletion from the measured User close path.
  /// Ready slots become tombstones; Waiting release unlinks empty pages and repairs its directory.
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
    #[cfg(test)]
    if FAIL_WAKEUP_PLACEMENT_WITH_CAPACITY.with(|flag| flag.replace(false)) {
      return Err(EnqueueOutcome::WakeupCapacityExhausted);
    }
    match Self::try_wakeup_substrate_schedule_key_inner(actor_id, WakeupKey::Tick(wakeup_tick)) {
      Ok(()) | Err(EnqueueOutcome::AlreadyLive) => Ok(()),
      Err(other) => Err(other),
    }
  }

  fn wakeup_cursor_drain_branch_weight(
    bucket: WakeupBucketDisposition,
    clock: WakeupClock,
  ) -> Weight {
    let physical = if matches!(bucket, WakeupBucketDisposition::Remove) {
      T::WeightInfo::scheduler_wakeup_cursor_worker_remove()
    } else {
      T::WeightInfo::scheduler_wakeup_cursor_worker_partial()
    };
    match clock {
      WakeupClock::Block => physical,
      WakeupClock::Tick => {
        let at_time = T::WeightInfo::at_time_trigger_occurrence();
        let cadenced = T::WeightInfo::cadenced_trigger_occurrence();
        physical.saturating_add(Weight::from_parts(
          at_time.ref_time().max(cadenced.ref_time()),
          at_time.proof_size().max(cadenced.proof_size()),
        ))
      }
    }
  }

  fn wakeup_cursor_drain_unit_weight_for(
    bucket: WakeupBucketDisposition,
    clock: WakeupClock,
  ) -> Weight {
    Self::wakeup_cursor_drain_branch_weight(bucket, clock)
      .saturating_add(Self::close_cleanup_weight_upper())
      .saturating_add(T::WeightInfo::record_wakeup_worker_fault())
  }

  pub fn wakeup_cursor_drain_unit_weight_upper(bucket: WakeupBucketDisposition) -> Weight {
    Self::wakeup_cursor_drain_unit_weight_for(bucket, WakeupClock::Tick)
  }

  #[cfg(test)]
  pub(crate) fn block_wakeup_cursor_drain_unit_weight_upper(
    bucket: WakeupBucketDisposition,
  ) -> Weight {
    Self::wakeup_cursor_drain_unit_weight_for(bucket, WakeupClock::Block)
  }

  pub(crate) fn process_due_temporal_occurrence_loaded(
    actor_id: ActorId,
    mut state: ActiveActorStateOf<T>,
    admission: ActorAdmissionCertificateOf<T>,
    loaded_step: Option<LoadedActorStepOf<T>>,
    now_tick: SchedulerTick,
  ) -> Result<bool, DispatchError> {
    let resources = if state.contract.steps.is_empty() {
      ActorStepResourceEnvelope {
        control: T::WeightInfo::scheduler_inner_zero_step_complete(),
        effect: Weight::zero(),
      }
    } else {
      let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
      loaded_step
        .filter(|loaded| loaded.cursor == cursor)
        .map(|loaded| loaded.resources)
        .ok_or(DispatchError::Other("temporal loaded Step is corrupt"))?
    };
    if state.hot.pending_signal {
      return Ok(false);
    }
    let (delay_ticks, trigger_family, occurrence_weight) = match state.contract.trigger {
      Trigger::AtTime { after_ticks } => (
        after_ticks,
        TriggerFamily::AtTime,
        T::WeightInfo::at_time_trigger_occurrence(),
      ),
      Trigger::Cadenced { every_ticks } => (
        every_ticks,
        TriggerFamily::Cadenced,
        T::WeightInfo::cadenced_trigger_occurrence(),
      ),
      Trigger::Manual
      | Trigger::AddressEvent { .. }
      | Trigger::ObservationChange { .. }
      | Trigger::ObservationCrossing { .. } => {
        return Err(DispatchError::Other("tick wakeup owner is not temporal"));
      }
    };
    if state
      .hot
      .trigger_runtime_state
      .temporal_anchor_tick()
      .is_none()
    {
      let Some(anchor_tick) = Self::temporal_anchor_tick(&state.contract.trigger)
        .map_err(|_| DispatchError::Other("genesis temporal anchor failed"))?
      else {
        return Err(DispatchError::Other("genesis temporal anchor failed"));
      };
      let due_tick = anchor_tick
        .checked_add(delay_ticks)
        .ok_or(DispatchError::Other("genesis temporal deadline failed"))?;
      state.hot.trigger_runtime_state = match state.contract.trigger {
        Trigger::AtTime { .. } => TriggerRuntimeState::AtTime {
          anchor_tick: Some(anchor_tick),
          consumed: false,
        },
        Trigger::Cadenced { .. } => TriggerRuntimeState::Cadenced {
          anchor_tick: Some(anchor_tick),
        },
        _ => return Err(DispatchError::Other("tick wakeup owner changed trigger")),
      };
      Self::try_store_control_hot_with_authority(actor_id, state.hot.clone())
        .map_err(|_| DispatchError::Other("genesis temporal authority update failed"))?;
      let placement = {
        Self::try_wakeup_substrate_schedule_transition_with_authority(
          actor_id,
          WakeupKey::Tick(due_tick),
          state.hot.clone(),
          &state.identity,
          state.run_state.as_ref().map_or(0, |run| run.cursor),
          &admission,
          resources,
        )
      };
      if let Err(error) = placement {
        let close_result = Self::finalize_actor_from_retained_state(
          actor_id,
          state.clone(),
          &admission,
          CloseReason::SchedulerIndexExhausted,
        );
        if !Self::scheduler_index_is_exhausted(error) || close_result.is_err() {
          return Err(DispatchError::Other("genesis temporal placement failed"));
        }
        return Ok(true);
      }
      return Ok(false);
    }
    let anchor_tick = state
      .hot
      .trigger_runtime_state
      .temporal_anchor_tick()
      .ok_or(DispatchError::Other(
        "initialized temporal anchor is missing",
      ))?;
    match state.hot.trigger_runtime_state {
      TriggerRuntimeState::AtTime {
        anchor_tick,
        consumed: false,
      } => {
        state.hot.trigger_runtime_state = TriggerRuntimeState::AtTime {
          anchor_tick,
          consumed: true,
        };
        Self::try_store_control_hot_with_authority(actor_id, state.hot)
          .map_err(|_| DispatchError::Other("AtTime progression commit failed"))?;
      }
      TriggerRuntimeState::Cadenced { .. } => {
        let next_due_tick = next_cadence_due_tick(anchor_tick, delay_ticks, now_tick)
          .ok_or(DispatchError::Other("cadence deadline failed"))?;
        let placement = {
          Self::try_wakeup_substrate_schedule_transition_with_authority(
            actor_id,
            WakeupKey::Tick(next_due_tick),
            state.hot.clone(),
            &state.identity,
            state.run_state.as_ref().map_or(0, |run| run.cursor),
            &admission,
            resources,
          )
        };
        if let Err(error) = placement {
          let close_result = Self::finalize_actor_from_retained_state(
            actor_id,
            state.clone(),
            &admission,
            CloseReason::SchedulerIndexExhausted,
          );
          if !Self::scheduler_index_is_exhausted(error) || close_result.is_err() {
            return Err(DispatchError::Other("cadence rearm failed"));
          }
          return Ok(true);
        }
      }
      TriggerRuntimeState::AtTime { consumed: true, .. }
      | TriggerRuntimeState::Stateless
      | TriggerRuntimeState::ObservationCrossing { .. } => {
        return Err(DispatchError::Other(
          "temporal runtime state is incompatible",
        ));
      }
    }
    Self::reconcile_actor_state_hold_with_authority(actor_id)
      .map_err(|_| DispatchError::Other("temporal state hold reconciliation failed"))?;
    let loaded_state = Self::load_actor_service_state_with_authority(actor_id);
    let Some((state, admission, _)) = loaded_state else {
      return Err(DispatchError::Other(
        "temporal progression state is corrupt",
      ));
    };
    let activation =
      Self::preflight_activation_from_authority(actor_id, state.clone(), admission.clone())
        .map_err(|_| DispatchError::Other("temporal activation preflight failed"))?;
    let closes_without_occurrence = activation.terminal_reason.is_some()
      || matches!(
        activation.action,
        ActivationAction::Close(_)
          | ActivationAction::EnqueueTemporal(Err(
            EnqueueOutcome::TicketExhausted
              | EnqueueOutcome::SchedulerIndexExhausted
              | EnqueueOutcome::WakeupIndexExhausted
          ))
      );
    if closes_without_occurrence {
      let activation = Self::commit_activation_plan(activation);
      return match activation {
        Ok(ActivationOutcome::Closed) => Ok(true),
        Ok(ActivationOutcome::Latched | ActivationOutcome::Coalesced) => Ok(false),
        _ => Err(DispatchError::Other(
          "temporal terminal substitution failed",
        )),
      };
    }
    let actor_type = state.identity.actor_class.actor_type();
    let breakdown = Self::trigger_fee_for_weight(actor_type, trigger_family, occurrence_weight);
    if trigger_family == TriggerFamily::AtTime {
      let temporal_capacity = Self::trigger_occurrence_capacity_sufficient(
        actor_type,
        &state.identity.sovereign_account,
        breakdown,
      )
      .map_err(|_| DispatchError::Other("temporal capacity calculation failed"))?;
      if !temporal_capacity {
        let close_result = Self::finalize_actor_from_retained_state(
          actor_id,
          state,
          &admission,
          CloseReason::TriggerAdmissionInsufficient,
        );
        close_result.map_err(|_| DispatchError::Other("underfunded temporal apoptosis failed"))?;
        return Ok(true);
      }
      if !Self::try_charge_prechecked_automatic_trigger_occurrence(
        actor_type,
        &state.identity.sovereign_account,
        breakdown,
      )
      .map_err(|_| DispatchError::Other("temporal collection failed"))?
      {
        return Err(DispatchError::Other("temporal fee collection failed"));
      }
    } else if !Self::try_charge_automatic_trigger_occurrence(
      actor_type,
      &state.identity.sovereign_account,
      breakdown,
    )
    .map_err(|_| DispatchError::Other("temporal collection failed"))?
    {
      return Ok(false);
    }
    let activation = Self::request_activation(actor_id);
    match activation {
      Ok(ActivationOutcome::Coalesced | ActivationOutcome::Latched) => {
        if trigger_family == TriggerFamily::Cadenced {
          {
            let (state, _, _) = Self::load_frame_actor_service_state(actor_id)
              .ok_or(DispatchError::Other("cadence latch authority disappeared"))?;
            if state.hot.trigger_wakeup_pointer.is_some() {
              Self::trigger_wakeup_substrate_invalidate_loaded(actor_id, state, &admission)
                .map_err(|_| DispatchError::Other("cadence latch disable failed"))?;
            }
          }
        }
        let paused_hot =
          Self::load_control_authority_with_authority(actor_id).map(|(_, hot, _)| hot);
        if let Some(hot) = paused_hot
          && hot.lifecycle.is_paused()
          && hot.queue_ticket.is_some()
        {
          Self::invalidate_ready_to_unsignaled_with_authority(actor_id)
            .map_err(|_| DispatchError::Other("paused temporal queue invalidation failed"))?;
        }
        Self::reconcile_actor_state_hold_with_authority(actor_id)
          .map_err(|_| DispatchError::Other("temporal latch hold reconciliation failed"))?;
        Self::deposit_event(Event::TriggerOccurrenceProcessed {
          actor_id,
          trigger_family: breakdown.trigger_family,
          fee: breakdown.trigger_fee,
        });
        Ok(false)
      }
      _ => Err(DispatchError::Other("temporal activation failed")),
    }
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
      let base_weight = Self::wakeup_cursor_drain_branch_weight(
        WakeupBucketDisposition::Retain,
        wakeup_key.clock(),
      );
      if Self::combined_queue_occupancy() >= u64::from(T::MaxActiveActors::get())
        || !meter.can_consume(base_weight)
      {
        break;
      }
      let occupancy = ActorWaitingOccupancies::<T>::get(wakeup_key);
      if occupancy == 0 {
        meter.consume(base_weight);
        break;
      }
      let bucket_disposition = if occupancy <= 1 {
        WakeupBucketDisposition::Remove
      } else {
        WakeupBucketDisposition::Retain
      };
      let unit_weight =
        Self::wakeup_cursor_drain_branch_weight(bucket_disposition, wakeup_key.clock());
      let admission_weight =
        Self::wakeup_cursor_drain_unit_weight_for(bucket_disposition, wakeup_key.clock());
      if !meter.can_consume(admission_weight) {
        meter.consume(base_weight);
        break;
      }
      meter.consume(unit_weight);
      let source_page = ActorWaitingHeads::<T>::get(wakeup_key) / 32;
      let outcome = polkadot_sdk::frame_support::storage::with_transaction(|| {
        let (ready, stats) = Self::wakeup_substrate_drain_key(wakeup_key, 1);
        if stats.entries_scanned == 0 {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok((
            stats, false,
          )));
        }
        let mut closed_with_reserved_cleanup = false;
        for (actor_id, post_source_state, admission, loaded_step) in ready {
          if matches!(wakeup_key, WakeupKey::Tick(_)) {
            match Self::process_due_temporal_occurrence_loaded(
              actor_id,
              post_source_state,
              admission,
              loaded_step,
              now_tick,
            ) {
              Ok(closed) => closed_with_reserved_cleanup |= closed,
              Err(error) => {
                return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                  error,
                ));
              }
            }
            continue;
          }
          let close_instance = Self::derive_active_actor_view(
            post_source_state.identity.clone(),
            post_source_state.hot.clone(),
            post_source_state.contract.clone(),
          );
          match Self::expiry_substitution_due_loaded(
            &close_instance,
            post_source_state.run_state.as_ref(),
          ) {
            Ok(true) => {
              let close_result = Self::finalize_actor_from_consumed_state(
                actor_id,
                post_source_state.clone(),
                &admission,
                CloseReason::WindowExpired,
              );
              if let Err(error) = close_result {
                return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                  error,
                ));
              }
              closed_with_reserved_cleanup = true;
              continue;
            }
            Ok(false) => {}
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error.into(),
              ));
            }
          }
          if let Err(error) = Self::enqueue_actor_state_loaded(
            actor_id,
            &post_source_state,
            &admission,
            loaded_step.as_ref(),
          ) {
            let close_result = Self::finalize_actor_from_consumed_state(
              actor_id,
              post_source_state.clone(),
              &admission,
              CloseReason::SchedulerIndexExhausted,
            );
            if !Self::scheduler_index_is_exhausted(error) || close_result.is_err() {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                polkadot_sdk::sp_runtime::DispatchError::Other("wakeup materialization failed"),
              ));
            }
            closed_with_reserved_cleanup = true;
          }
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok((
          stats,
          closed_with_reserved_cleanup,
        )))
      });
      let (stats, closed_with_reserved_cleanup) = match outcome {
        Ok(outcome) => outcome,
        Err(_) => {
          let recorded = Self::record_wakeup_worker_fault(
            meter,
            WakeupWorkerFault {
              key: wakeup_key,
              page: source_page,
              class: CrossingWorkerFaultClass::Invariant,
            },
          );
          debug_assert!(recorded, "fault Weight was reserved before wakeup mutation");
          break;
        }
      };
      if closed_with_reserved_cleanup {
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

  pub(crate) fn temporal_anchor_tick(
    trigger: &TriggerOf<T>,
  ) -> Result<Option<SchedulerTick>, EnqueueOutcome> {
    if !matches!(trigger, Trigger::AtTime { .. } | Trigger::Cadenced { .. }) {
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

  #[cfg(test)]
  pub(crate) fn retry_eligible_at(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
  ) -> Result<BlockNumberFor<T>, EnqueueOutcome> {
    let run_state =
      ActorRunStateStore::<T>::get(actor_id).ok_or(EnqueueOutcome::CorruptedTopology)?;
    Self::retry_eligible_at_loaded(instance, &run_state)
  }

  pub(crate) fn suspension_eligible_at(
    cooldown_blocks: u32,
    window: Option<ScheduleWindow<BlockNumberFor<T>>>,
    last_attempt_block: BlockNumberFor<T>,
    unsuccessful_attempts_at_cursor: u32,
  ) -> Result<BlockNumberFor<T>, EnqueueOutcome> {
    let cooldown: BlockNumberFor<T> = cooldown_blocks.into();
    let cursor_local_attempt = unsuccessful_attempts_at_cursor.saturating_sub(1);
    let backoff: BlockNumberFor<T> = Self::retry_backoff_blocks(cursor_local_attempt).into();
    let retry_delay = cooldown.max(backoff);
    let mut eligible_at = last_attempt_block
      .checked_add(&retry_delay)
      .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
    if let Some(window) = window {
      eligible_at = eligible_at.max(window.start);
    }
    Ok(eligible_at)
  }

  fn retry_eligible_at_loaded(
    instance: &ActiveActorViewOf<T>,
    run_state: &ActorRunStateOf<T>,
  ) -> Result<BlockNumberFor<T>, EnqueueOutcome> {
    let expected = Self::suspension_eligible_at(
      instance.cooldown_blocks,
      instance.window,
      run_state.last_attempt_block,
      run_state.unsuccessful_attempts_at_cursor,
    )?;
    if run_state.eligible_at != expected {
      return Err(EnqueueOutcome::CorruptedTopology);
    }
    Ok(run_state.eligible_at)
  }

  fn schedule_next_work_loaded(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
    loaded_authority: (
      &ActorHotStateOf<T>,
      &ActorIdentityOf<T>,
      Option<&ActorRunStateOf<T>>,
      &ActorAdmissionCertificateOf<T>,
      ActorStepResourceEnvelope,
    ),
    now: BlockNumberFor<T>,
    cutoff: ServiceCutoff,
  ) -> Result<StepControlPlacement, EnqueueOutcome> {
    if instance.lifecycle.is_paused() {
      return Self::schedule_window_expiry_with_authority(actor_id, instance, loaded_authority)
        .map(|()| {
          if instance.window.is_some() {
            StepControlPlacement::Wakeup
          } else {
            StepControlPlacement::None
          }
        });
    }
    let eligible_at = if matches!(
      instance.cycle_state,
      CycleState::Running | CycleState::Suspended
    ) {
      let run = loaded_authority
        .2
        .ok_or(EnqueueOutcome::CorruptedTopology)?;
      if instance.cycle_state == CycleState::Running {
        if !run.running_is_coherent() {
          return Err(EnqueueOutcome::CorruptedTopology);
        }
        run.eligible_at
      } else {
        Self::retry_eligible_at_loaded(instance, run)?
      }
    } else if instance.pending_signal {
      Self::next_eligible_at(instance, now)?
    } else {
      return Self::schedule_window_expiry_with_authority(actor_id, instance, loaded_authority)
        .map(|()| {
          if instance.window.is_some() {
            StepControlPlacement::Wakeup
          } else {
            StepControlPlacement::None
          }
        });
    };
    let wakeup_at = instance.window.map_or(eligible_at, |window| {
      eligible_at.min(Self::window_terminal_at(&window))
    });
    let exact_next_block = now
      .checked_add(&One::one())
      .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
    if wakeup_at < exact_next_block || wakeup_at == exact_next_block && cutoff.is_snapshotted() {
      Ok(StepControlPlacement::Queue)
    } else {
      let (hot, identity, run_state, admission, resources) = loaded_authority;
      Self::defer_wakeup_with_authority(
        actor_id,
        wakeup_at,
        instance,
        hot.clone(),
        identity,
        run_state,
        admission,
        resources,
      )
      .map(|()| StepControlPlacement::Wakeup)
    }
  }

  #[cfg(test)]
  pub(crate) fn test_schedule_next_work_source(
    actor_id: ActorId,
    state: &ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
    supplied_run: Option<Option<&ActorRunStateOf<T>>>,
    now: BlockNumberFor<T>,
  ) -> Result<(StepControlPlacement, Vec<ActorId>), EnqueueOutcome> {
    let retained;
    let (state, admission, resources, run) = match supplied_run {
      Some(run) => (state, admission, resources, run),
      None => {
        retained = Self::load_frame_actor_service_state(actor_id)
          .ok_or(EnqueueOutcome::CorruptedTopology)?;
        let (state, admission, loaded_step) = &retained;
        let resources = if state.contract.steps.is_empty() {
          ActorStepResourceEnvelope {
            control: T::WeightInfo::scheduler_inner_zero_step_complete(),
            effect: Weight::zero(),
          }
        } else {
          loaded_step
            .as_ref()
            .ok_or(EnqueueOutcome::CorruptedTopology)?
            .resources
        };
        (state, admission, resources, state.run_state.as_ref())
      }
    };
    let instance = Self::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    let authority = (&state.hot, &state.identity, run, admission, resources);
    let placement = Self::schedule_next_work_loaded(
      actor_id,
      &instance,
      authority,
      now,
      ServiceCutoff::Snapshotted,
    )?;
    let requeues = if placement == StepControlPlacement::Queue {
      vec![actor_id]
    } else {
      Vec::new()
    };
    Ok((placement, requeues))
  }

  fn schedule_next_work_with_authority(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
    hot: ActorHotStateOf<T>,
    identity: &ActorIdentityOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
    admission: &ActorAdmissionCertificateOf<T>,
    resources: ActorStepResourceEnvelope,
    now: BlockNumberFor<T>,
    cutoff: ServiceCutoff,
  ) -> Result<StepControlPlacement, EnqueueOutcome> {
    let loaded_authority = (&hot, identity, run_state, admission, resources);
    let placement =
      Self::schedule_next_work_loaded(actor_id, instance, loaded_authority, now, cutoff)?;
    if placement == StepControlPlacement::Queue {
      match Self::enqueue_authority_loaded(
        actor_id,
        hot.clone(),
        identity,
        run_state,
        admission,
        resources,
      ) {
        Ok(()) => {}
        Err(EnqueueOutcome::CapacityUnavailable) => {
          let next_block = frame_system::Pallet::<T>::block_number()
            .checked_add(&One::one())
            .ok_or(EnqueueOutcome::SchedulerIndexExhausted)?;
          Self::defer_wakeup_with_authority(
            actor_id,
            next_block,
            instance,
            hot.clone(),
            identity,
            run_state,
            admission,
            resources,
          )?;
          return Ok(StepControlPlacement::Wakeup);
        }
        Err(error) => return Err(error),
      }
    }
    Ok(placement)
  }

  pub(crate) fn is_window_expired(instance: &ActiveActorViewOf<T>) -> bool {
    let now = frame_system::Pallet::<T>::block_number();
    instance
      .window
      .map(|window| now > window.end)
      .unwrap_or(false)
  }

  pub(crate) fn classification_dispatch_error(error: ActorClassificationError) -> Error<T> {
    match error {
      ActorClassificationError::ActorInvariant => Error::<T>::ActorInvariant,
      ActorClassificationError::RunInvariant => Error::<T>::ActorRunInvariant,
      ActorClassificationError::ComputationOverflow => Error::<T>::ComputationOverflow,
    }
  }

  pub(crate) fn expiry_substitution_due_loaded(
    instance: &ActiveActorViewOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
  ) -> Result<bool, Error<T>> {
    Self::classify_actor_loaded(instance, run_state)
      .map(|classification| classification.terminal_reason == Some(CloseReason::WindowExpired))
      .map_err(Self::classification_dispatch_error)
  }

  #[cfg(test)]
  pub(crate) fn classify_actor(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
  ) -> Result<ActorClassification<BlockNumberFor<T>>, ActorClassificationError> {
    let run_state = ActorRunStateStore::<T>::get(actor_id);
    Self::classify_actor_loaded(instance, run_state.as_ref())
  }

  pub(crate) fn classify_observation_activation_compact(
    state: &ObservationActivationState<T>,
  ) -> Result<ActorClassification<BlockNumberFor<T>>, ActorClassificationError> {
    let now = frame_system::Pallet::<T>::block_number();
    let run_head = state.run_head.as_ref();
    let terminal_reason = if state
      .authority
      .window
      .is_some_and(|window| now > window.end)
    {
      Some(CloseReason::WindowExpired)
    } else if state.hot.cycle_state == CycleState::Idle && state.identity.cycle_nonce == u64::MAX {
      Some(CloseReason::CycleNonceExhausted)
    } else if run_head.is_some_and(|run| {
      state.loaded_step.as_ref().is_some_and(|loaded_step| {
        loaded_step
          .step
          .on_error
          .retry_max_attempts()
          .is_some_and(|max_attempts| run.unsuccessful_attempts_at_cursor >= max_attempts)
      })
    }) {
      Some(CloseReason::RetryAttemptsExhausted)
    } else if Self::failure_limit_reached(state.hot.unsuccessful_attempt_streak) {
      Some(CloseReason::ConsecutiveFailures)
    } else if state.hot.cycle_state == CycleState::Idle
      && state
        .authority
        .auto_close_at_cycle_nonce
        .is_some_and(|target| state.identity.cycle_nonce >= target)
    {
      Some(CloseReason::AutoCloseNonceReached)
    } else {
      None
    };

    let execution_phase = if GlobalCircuitBreaker::<T>::get() {
      ActorExecutionPhase::GlobalCircuitBreaker
    } else if state.hot.lifecycle.is_paused() {
      ActorExecutionPhase::Paused
    } else if terminal_reason.is_some() {
      ActorExecutionPhase::Ready
    } else if state.hot.cycle_state == CycleState::Running {
      let run = run_head.ok_or(ActorClassificationError::RunInvariant)?;
      if run.eligible_at > now {
        ActorExecutionPhase::WaitingBlock(run.eligible_at)
      } else {
        ActorExecutionPhase::Ready
      }
    } else if state.hot.cycle_state == CycleState::Suspended {
      let run = run_head.ok_or(ActorClassificationError::RunInvariant)?;
      let expected = Self::suspension_eligible_at(
        state.authority.cooldown_blocks,
        state.authority.window,
        run.last_attempt_block,
        run.unsuccessful_attempts_at_cursor,
      )
      .map_err(|outcome| match outcome {
        EnqueueOutcome::SchedulerIndexExhausted => ActorClassificationError::ComputationOverflow,
        _ => ActorClassificationError::RunInvariant,
      })?;
      if expected != run.eligible_at {
        return Err(ActorClassificationError::RunInvariant);
      }
      if run.eligible_at > now {
        ActorExecutionPhase::WaitingRetry(run.eligible_at)
      } else {
        ActorExecutionPhase::Ready
      }
    } else {
      let cooldown_anchor = state
        .hot
        .last_cycle_block
        .unwrap_or(state.hot.schedule_anchor);
      let cooldown_eligible_at =
        if state.identity.cycle_nonce == 0 && state.hot.last_cycle_block.is_none() {
          state.hot.schedule_anchor
        } else {
          cooldown_anchor
            .checked_add(&state.authority.cooldown_blocks.into())
            .ok_or(ActorClassificationError::ComputationOverflow)?
        };
      let window_floor = state
        .authority
        .window
        .map(|window| window.start)
        .unwrap_or_else(Zero::zero);
      let eligible_at = now.max(cooldown_eligible_at).max(window_floor);
      if eligible_at > now {
        ActorExecutionPhase::WaitingBlock(eligible_at)
      } else {
        ActorExecutionPhase::Ready
      }
    };
    Ok(ActorClassification {
      terminal_reason,
      execution_phase,
    })
  }

  pub(crate) fn classify_actor_loaded(
    instance: &ActiveActorViewOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
  ) -> Result<ActorClassification<BlockNumberFor<T>>, ActorClassificationError> {
    let cursor = run_state.as_ref().map_or(0, |state| state.cursor as usize);
    Self::classify_actor_at_current_step(instance, run_state, instance.steps.get(cursor))
  }

  fn classify_actor_at_current_step(
    instance: &ActiveActorViewOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
    current_step: Option<&StepOf<T>>,
  ) -> Result<ActorClassification<BlockNumberFor<T>>, ActorClassificationError> {
    match (instance.cycle_state, run_state) {
      (CycleState::Idle, None) => {}
      (CycleState::Running | CycleState::Suspended, Some(state)) => {
        let expected_cycle_nonce = instance
          .cycle_nonce
          .checked_add(1)
          .ok_or(ActorClassificationError::RunInvariant)?;
        if current_step.is_none() || state.cycle_nonce != expected_cycle_nonce {
          return Err(ActorClassificationError::RunInvariant);
        }
        if instance.cycle_state == CycleState::Running {
          if state.unsuccessful_attempts_at_cursor != 0 || !state.running_is_coherent() {
            return Err(ActorClassificationError::RunInvariant);
          }
        } else if state.unsuccessful_attempts_at_cursor == 0
          || !state.suspension_is_coherent()
          || current_step
            .and_then(|step| step.on_error.retry_max_attempts())
            .is_none()
        {
          return Err(ActorClassificationError::RunInvariant);
        }
      }
      _ => return Err(ActorClassificationError::RunInvariant),
    }

    let terminal_reason = if Self::is_window_expired(instance) {
      Some(CloseReason::WindowExpired)
    } else if instance.cycle_state == CycleState::Idle && instance.cycle_nonce == u64::MAX {
      Some(CloseReason::CycleNonceExhausted)
    } else if run_state.as_ref().is_some_and(|state| {
      current_step
        .and_then(|step| step.on_error.retry_max_attempts())
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
    } else if instance.cycle_state == CycleState::Running {
      let state = run_state.ok_or(ActorClassificationError::RunInvariant)?;
      let now = frame_system::Pallet::<T>::block_number();
      if state.eligible_at > now {
        ActorExecutionPhase::WaitingBlock(state.eligible_at)
      } else {
        ActorExecutionPhase::Ready
      }
    } else if instance.cycle_state == CycleState::Suspended {
      let eligible_at = Self::retry_eligible_at_loaded(
        instance,
        run_state.ok_or(ActorClassificationError::RunInvariant)?,
      )
      .map_err(|outcome| match outcome {
        EnqueueOutcome::SchedulerIndexExhausted => ActorClassificationError::ComputationOverflow,
        _ => ActorClassificationError::RunInvariant,
      })?;
      let now = frame_system::Pallet::<T>::block_number();
      if eligible_at > now {
        ActorExecutionPhase::WaitingRetry(eligible_at)
      } else {
        ActorExecutionPhase::Ready
      }
    } else if matches!(
      instance.trigger,
      Trigger::AtTime { .. } | Trigger::Cadenced { .. }
    ) {
      if instance.pending_signal {
        ActorExecutionPhase::Ready
      } else if instance.temporal_occurrence_consumed {
        ActorExecutionPhase::WaitingSignal
      } else {
        let Some(TriggerWakeupPointer { tick: due_tick, .. }) = instance.trigger_wakeup_pointer
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

  fn close_admission_decision(reason: CloseReason, meter: &WeightMeter) -> AdmissionDecision {
    let weight = if matches!(
      reason,
      CloseReason::CycleAdmissionInsufficient | CloseReason::TriggerAdmissionInsufficient
    ) {
      T::WeightInfo::pipeline_admission_apoptosis()
    } else {
      Self::close_cleanup_weight_upper()
    };
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
    instance
      .steps
      .get(start_cursor)
      .and_then(|step| step.on_error.retry_max_attempts())
      .is_some_and(|limit| {
        prior_unsuccessful_attempts_at_cursor
          .unwrap_or_default()
          .saturating_add(1)
          >= limit
      })
  }

  fn cycle_may_close_on_success(instance: &ActiveActorViewOf<T>) -> bool {
    instance.completion == CompletionPolicy::CloseAfterProductiveCycle
      || instance
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
    ) || Self::cycle_may_close_on_success(instance)
  }

  fn apply_admission_loaded(
    actor_id: ActorId,
    instance: &ActiveActorViewOf<T>,
    run_state: Option<&ActorRunStateOf<T>>,
    step_plan: Option<&CurrentStepPlanOf<T>>,
    pipeline_capacity: Option<Result<bool, Error<T>>>,
    meter: &WeightMeter,
  ) -> AdmissionDecision {
    let current_step = step_plan.map(|plan| &plan.loaded_step.step).or_else(|| {
      instance
        .steps
        .get(run_state.map_or(0, |run| run.cursor as usize))
    });
    let Ok(classification) =
      Self::classify_actor_at_current_step(instance, run_state, current_step)
    else {
      return AdmissionDecision::Invariant;
    };
    if classification.execution_phase == ActorExecutionPhase::GlobalCircuitBreaker {
      return AdmissionDecision::Skip;
    }
    if let Some(reason) = classification.terminal_reason {
      return Self::close_admission_decision(reason, meter);
    }
    if instance.actor_class.actor_type() == ActorType::User
      && instance.cycle_state == CycleState::Idle
      && instance.pending_signal
    {
      match pipeline_capacity.unwrap_or_else(|| {
        Self::pipeline_capacity_sufficient(actor_id, ActorType::User, &instance.sovereign_account)
      }) {
        Ok(true) => {}
        Ok(false) => {
          return Self::close_admission_decision(CloseReason::CycleAdmissionInsufficient, meter);
        }
        Err(_) => return AdmissionDecision::Invariant,
      }
    }
    if classification.execution_phase != ActorExecutionPhase::Ready {
      return AdmissionDecision::Skip;
    }
    let run_state = if matches!(
      instance.cycle_state,
      CycleState::Running | CycleState::Suspended
    ) {
      let Some(run_state) = run_state else {
        return AdmissionDecision::Skip;
      };
      Some(run_state)
    } else {
      None
    };
    if instance.steps.is_empty() {
      if instance.cycle_state != CycleState::Idle || run_state.is_some() || step_plan.is_some() {
        return AdmissionDecision::Invariant;
      }
      let zero_step_weight = Self::contract_steps_admission_weight_upper(
        instance.actor_class.actor_type(),
        &instance.steps,
      );
      if !meter.can_consume(zero_step_weight) {
        return AdmissionDecision::Defer;
      }
      return AdmissionDecision::Admit {
        weight: zero_step_weight,
        terminal_cleanup: TerminalCleanupReservation::Included,
      };
    }
    let start_cursor = run_state.as_ref().map_or(0, |state| state.cursor as usize);
    let prior_unsuccessful_attempts_at_cursor = run_state
      .as_ref()
      .map(|state| state.unsuccessful_attempts_at_cursor);
    let terminal_cleanup = if Self::cycle_requires_terminal_cleanup_budget(
      instance,
      start_cursor,
      prior_unsuccessful_attempts_at_cursor,
    ) {
      TerminalCleanupReservation::Included
    } else {
      TerminalCleanupReservation::NotIncluded
    };
    let resources = step_plan
      .filter(|plan| {
        plan.ticket.actor_id == actor_id && plan.ticket.cursor as usize == start_cursor
      })
      .map(|plan| plan.loaded_step.resources);
    let Some(resources) = resources else {
      return AdmissionDecision::Invariant;
    };
    if instance.actor_class.actor_type() == ActorType::User
      && instance.cycle_state == CycleState::Suspended
    {
      let Some(step) = current_step else {
        return AdmissionDecision::Invariant;
      };
      match Self::action_capacity_sufficient(
        ActorType::User,
        &instance.sovereign_account,
        step,
        resources,
      ) {
        Ok(true) => {}
        Ok(false) => {
          return Self::close_admission_decision(CloseReason::CycleAdmissionInsufficient, meter);
        }
        Err(_) => return AdmissionDecision::Invariant,
      }
    }
    let mut current_step_weight = resources.control.saturating_add(resources.effect);
    if terminal_cleanup.is_included() {
      current_step_weight = current_step_weight.saturating_add(Self::close_cleanup_weight_upper());
    }
    if !meter.can_consume(current_step_weight) {
      return AdmissionDecision::Defer;
    }
    AdmissionDecision::Admit {
      weight: current_step_weight,
      terminal_cleanup,
    }
  }

  /// Projects the canonical actor classifier without stripping temporal payloads.
  pub fn actor_eligibility(
    actor_id: ActorId,
  ) -> Result<ActorEligibility<T::ObservationFeedId, BlockNumberFor<T>>, ActorClassificationError>
  {
    let state = match Self::load_actor_state_for_frame_control(actor_id) {
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
      Trigger::AtTime { after_ticks } => {
        let TriggerRuntimeState::AtTime { consumed, .. } = state.hot.trigger_runtime_state else {
          return Err(ActorClassificationError::ActorInvariant);
        };
        ActorTriggerActivation::AtTime {
          after_ticks: *after_ticks,
          consumed,
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
      eligibility: Self::classify_actor_loaded(&instance, state.run_state.as_ref())?,
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
      TriggerCauseProvenance::ExternalPhase,
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
      TriggerCauseProvenance::Deferred,
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
      TriggerCauseProvenance::Deferred,
    )
  }

  pub fn notify_address_event_without_source(
    actor_id: ActorId,
    asset: T::AssetId,
    amount: T::Balance,
  ) -> DispatchResult {
    Self::notify_address_event_with_context(
      actor_id,
      asset,
      amount,
      None,
      None,
      TriggerCauseProvenance::Deferred,
    )
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
    let state = match Self::load_actor_state_for_frame_control(actor_id) {
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
    let mut funding = state.funding;
    let run_state = state.run_state;
    let identity = state.identity.clone();
    let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
    let classification = Self::classify_actor_loaded(&instance, run_state.as_ref())
      .map_err(Self::classification_dispatch_error)?;
    if classification.terminal_reason == Some(CloseReason::WindowExpired) || amount.is_zero() {
      return Ok(());
    }
    if !authorized || !funding.funding_tracked_assets.contains(&asset) {
      return Ok(());
    }
    if let Some(accumulated) = funding.funding_accumulated.get_mut(&asset) {
      *accumulated = accumulated
        .checked_add(&amount)
        .ok_or(Error::<T>::FundingAccumulatorOverflow)?;
    } else {
      funding
        .funding_accumulated
        .try_insert(asset, amount)
        .map_err(|_| Error::<T>::FundingAccumulatorOverflow)?;
    }
    Self::ensure_funding_state_hold_capacity(actor_id, &identity, &funding)
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
    if T::AssetOps::balance(&event.destination, event.asset)
      .checked_add(&event.amount)
      .is_none()
    {
      return Err(IngressFailure::permanent(
        Error::<T>::FundingAccumulatorOverflow,
      ));
    }
    Self::preflight_funding_event(
      actor_id,
      event.asset,
      event.amount,
      event.source.as_ref(),
      event.provenance.as_ref(),
    )
    .map_err(Self::classify_ingress_error)
  }

  fn trigger_cause_provenance(provenance: Option<&FundingProvenance>) -> TriggerCauseProvenance {
    match provenance {
      Some(FundingProvenance::Signed) => TriggerCauseProvenance::ExternalPhase,
      Some(FundingProvenance::InternalProtocol | FundingProvenance::Xcm) | None => {
        TriggerCauseProvenance::Deferred
      }
    }
  }

  #[cfg(test)]
  pub(crate) fn test_trigger_cause_provenance(
    provenance: Option<&FundingProvenance>,
  ) -> TriggerCauseProvenance {
    Self::trigger_cause_provenance(provenance)
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
      Self::trigger_cause_provenance(event.provenance.as_ref()),
    )
    .map_err(Self::classify_ingress_error)
  }

  /// Maps one certified-ingress error to its closed retry class.
  ///
  /// Recoverable queue/wakeup capacity or placement unavailability surfaces as
  /// `QueueCapacityUnavailable` (queue saturation and failed wakeup placement) and
  /// `StateHoldUnavailable` (owner may fund the positive geometry delta) are Temporary.
  /// Monotonic ticket/index exhaustion, topology corruption, and invariant failure are Permanent.
  fn classify_ingress_error(error: DispatchError) -> IngressFailure {
    if error == Error::<T>::QueueCapacityUnavailable.into()
      || error == Error::<T>::StateHoldUnavailable.into()
    {
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
    cause_provenance: TriggerCauseProvenance,
  ) -> DispatchResult {
    // Zero or self/no-op movement creates no Actors ingress (spec 5.3).
    if amount.is_zero() {
      return Ok(());
    }
    Self::preflight_funding_event(actor_id, asset, amount, source, provenance)?;
    Self::with_reused_transaction(|| {
      Self::apply_address_event_parts(
        actor_id,
        asset,
        amount,
        source,
        provenance,
        cause_provenance,
      )
    })
  }

  fn apply_address_event_parts(
    actor_id: ActorId,
    asset: T::AssetId,
    amount: T::Balance,
    source: Option<&T::AccountId>,
    provenance: Option<&FundingProvenance>,
    cause_provenance: TriggerCauseProvenance,
  ) -> DispatchResult {
    let state = match Self::load_actor_state_for_frame_control(actor_id) {
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
    let run_state = state.run_state;
    let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
    let classification = Self::classify_actor_loaded(&instance, run_state.as_ref())
      .map_err(Self::classification_dispatch_error)?;
    if classification.terminal_reason == Some(CloseReason::WindowExpired) {
      return Self::finalize_actor(actor_id, &instance, CloseReason::WindowExpired);
    }
    let signal_matched = if !instance.pending_signal
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
    if amount > Zero::zero() {
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
        Self::reconcile_actor_state_hold_with_authority(actor_id)?;
        Self::deposit_event(Event::FundingAccumulated {
          actor_id,
          asset,
          added: amount,
          accumulated,
        });
      }
    }
    if signal_matched {
      let actor_type = instance.actor_class.actor_type();
      let breakdown = Self::trigger_fee_for_weight(
        actor_type,
        TriggerFamily::AddressEvent,
        T::WeightInfo::address_event_trigger_occurrence(),
      );
      let _ = Self::try_commit_frame_automatic_trigger_occurrence(
        actor_id,
        actor_type,
        &instance.sovereign_account,
        breakdown,
        cause_provenance,
      )?;
    }
    Ok(())
  }

  pub(crate) fn evaluate_actor_liveness(actor_id: ActorId) -> DispatchResult {
    let state = match Self::load_actor_state_for_frame_control(actor_id) {
      LoadedActorStateOf::Active(state) => state,
      LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => {
        return Err(Error::<T>::ActorNotFound.into());
      }
      LoadedActorStateOf::Corrupt => return Err(Error::<T>::ActorInvariant.into()),
    };
    let run_state = state.run_state;
    let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
    if let Some(reason) = Self::classify_actor_loaded(&instance, run_state.as_ref())
      .map_err(Self::classification_dispatch_error)?
      .terminal_reason
    {
      return Self::finalize_actor(actor_id, &instance, reason);
    }
    Ok(())
  }
}
