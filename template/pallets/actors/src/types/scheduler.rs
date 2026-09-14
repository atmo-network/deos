use super::{
  contract::ActorContractCommitment,
  lifecycle::{ActorId, ActorRef, ServiceResidenceKind},
};
use frame::prelude::*;

pub type QueueTicket = u64;
pub type QueuePageId = u64;
pub type WakeupPageId = u64;
pub type WakeupSlot = u32;
pub type WakeupCursorIndex = u32;
pub type SchedulerTick = u64;

/// Inert storage shape for the future actor-keyed persistent service ring.
///
/// `cursor` is the next member to encounter; `count` is occupancy only. The
/// historical control cells remain scheduler authority until the whole ring is
/// populated and cut over atomically.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ServiceHeaderRecord<BlockNumber> {
  pub round_block: Option<BlockNumber>,
  pub cursor: Option<ActorRef>,
  pub count: u32,
}

impl<BlockNumber> Default for ServiceHeaderRecord<BlockNumber> {
  fn default() -> Self {
    Self {
      round_block: None,
      cursor: None,
      count: 0,
    }
  }
}

/// One generation-bound member of the future Live/Pending service ring.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ServiceNode<BlockNumber> {
  pub generation: u64,
  pub previous: ActorRef,
  pub next: ActorRef,
  pub kind: ServiceResidenceKind,
  pub eligible_from: BlockNumber,
  pub last_considered: BlockNumber,
}

/// Rejected transaction-local mutations of the inert canonical service ring.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceRingMutationError {
  TransactionRequired,
  LegacyAuthorityPresent,
  ProcessMissing,
  ProcessResidenceMismatch,
  MemberAlreadyExists,
  MemberMissing,
  StaleGeneration,
  CorruptRing,
  CapacityExceeded,
  BlockNumberOverflow,
}

/// Read-only classification of the current inert service-ring frontier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceRoundEncounter {
  Empty,
  Closed,
  AlreadyAttempted(ActorRef),
  Eligible(ActorRef),
}

/// Rejected transaction-local operations on the inert service-ring round frontier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceRoundError {
  TransactionRequired,
  RoundFromFuture,
  RoundNotStarted,
  CorruptRing,
  StaleGeneration,
  ProcessMissing,
  ProcessResidenceMismatch,
  FutureMemberUnmarked,
  AttemptFromFuture,
}

pub type DependencySourceId = u64;
pub type DependencyRevision = u64;
pub type PlanRevision = u64;

/// Checked monotone state owned by one event-complete dependency source.
#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Default,
  Encode,
  Eq,
  PartialEq,
  TypeInfo,
  MaxEncodedLen,
)]
pub struct DependencyRevisionState {
  pub revision: DependencyRevision,
  pub scan_target: Option<DependencyRevision>,
  pub scan_cursor: u64,
  pub scan_end: u64,
  pub exhausted: bool,
}

/// One coalesced activation-check obligation bound to exact semantic authority.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct PendingCheckOwner {
  pub actor: ActorRef,
  pub plan_revision: PlanRevision,
}

/// Exact reverse handle for one event-complete dependency registration.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct DependencyRegistrationHandle {
  pub actor: ActorRef,
  pub plan_revision: PlanRevision,
  pub acknowledged_revision: DependencyRevision,
}

/// Source-owned append position for one exact registration handle.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct DependencyRegistrationPosition {
  pub page: u64,
  pub slot: u8,
}

/// Bounded topology owner for one source's retained registration pages.
#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Default,
  Encode,
  Eq,
  PartialEq,
  TypeInfo,
  MaxEncodedLen,
)]
pub struct DependencyRegistrationHeader {
  pub next_index: u64,
  pub count: u32,
  pub free_count: u32,
}

/// One fixed-width source-owned registration page. Removed entries remain stable holes.
#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct DependencyRegistrationPage {
  pub entries: BoundedVec<Option<DependencyRegistrationHandle>, ConstU32<32>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyRegistrationMutation {
  Installed,
  Unchanged,
  Replaced,
  Removed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyRegistrationError {
  TransactionRequired,
  PendingOwnerMissing,
  PendingOwnerMismatch,
  SourceExhausted,
  RevisionFromFuture,
  RegistrationAlreadyExists,
  RegistrationMissing,
  CurrentRegistrationMismatch,
  PositionMissing,
  PositionMismatch,
  CorruptTopology,
  CapacityExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyRevisionMutation {
  Advanced(DependencyRevision),
  Exhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyRevisionError {
  TransactionRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyScanMutation {
  Begun(DependencyRevision),
  Advanced(u64),
  Completed,
  HandedOff(DependencyRevision),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyScanError {
  TransactionRequired,
  SourceExhausted,
  ScanAlreadyActive,
  ScanMissing,
  TargetMismatch,
  CursorMismatch,
  CursorExhausted,
  PendingAuthorityMissing,
  PendingAuthorityMismatch,
  CorruptRegistrationPosition,
  ScanComplete,
  CorruptTopology,
}

/// Bucket-level ownership for retained fixed-width deadline pages.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct DeadlineHeader {
  pub first_page: u64,
  pub last_page: u64,
  pub next_page: u64,
  pub page_count: u32,
  pub count: u32,
}

/// One retained C32 deadline page. Empty interior slots are reusable without moving members.
#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct DeadlinePage {
  pub previous_page: Option<u64>,
  pub next_page: Option<u64>,
  pub live_entries: u8,
  pub entries: BoundedVec<Option<ActorRef>, ConstU32<32>>,
}

/// Generation-bound reverse index for exact arbitrary deadline removal.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct DeadlineHandle<BlockNumber> {
  pub actor: ActorRef,
  pub key: WakeupKey<BlockNumber>,
  pub page: u64,
  pub slot: u8,
}

/// Rejected transaction-local mutations of the inert canonical deadline carrier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeadlineMutationError {
  TransactionRequired,
  LegacyAuthorityPresent,
  ProcessMissing,
  ProcessResidenceMismatch,
  MemberAlreadyExists,
  MemberMissing,
  StaleGeneration,
  InvalidDestination,
  PageFull,
  CorruptCarrier,
  CapacityExceeded,
}

/// Rejected transaction-local mutations of the inert deadline-key min-heaps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeadlineIndexMutationError {
  TransactionRequired,
  LegacyAuthorityPresent,
  HeaderMissing,
  KeyAlreadyExists,
  KeyMissing,
  StaleIndex,
  CorruptHeader,
  CorruptHeap,
  CapacityExceeded,
}

impl From<DeadlineIndexMutationError> for DeadlineMutationError {
  fn from(error: DeadlineIndexMutationError) -> Self {
    match error {
      DeadlineIndexMutationError::TransactionRequired => Self::TransactionRequired,
      DeadlineIndexMutationError::LegacyAuthorityPresent => Self::LegacyAuthorityPresent,
      DeadlineIndexMutationError::CapacityExceeded => Self::CapacityExceeded,
      DeadlineIndexMutationError::HeaderMissing
      | DeadlineIndexMutationError::KeyAlreadyExists
      | DeadlineIndexMutationError::KeyMissing
      | DeadlineIndexMutationError::StaleIndex
      | DeadlineIndexMutationError::CorruptHeader
      | DeadlineIndexMutationError::CorruptHeap => Self::CorruptCarrier,
    }
  }
}

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Default,
  Encode,
  Eq,
  Ord,
  PartialEq,
  PartialOrd,
  TypeInfo,
  MaxEncodedLen,
)]
pub enum WakeupClock {
  #[default]
  Block,
  Tick,
}

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  Ord,
  PartialEq,
  PartialOrd,
  TypeInfo,
  MaxEncodedLen,
)]
pub enum WakeupKey<BlockNumber> {
  Block(BlockNumber),
  Tick(SchedulerTick),
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct WakeupWorkerFault<BlockNumber> {
  pub key: WakeupKey<BlockNumber>,
  pub page: WakeupPageId,
  pub class: super::observation::CrossingWorkerFaultClass,
}

impl<BlockNumber> WakeupKey<BlockNumber> {
  pub fn clock(&self) -> WakeupClock {
    match self {
      Self::Block(_) => WakeupClock::Block,
      Self::Tick(_) => WakeupClock::Tick,
    }
  }
}

/// Returns the last complete scheduler tick visible at `timestamp_millis`.
pub fn scheduler_tick_floor(timestamp_millis: u64, tick_millis: u64) -> Option<SchedulerTick> {
  (tick_millis > 0).then(|| timestamp_millis / tick_millis)
}

/// Returns the first scheduler tick whose boundary is not earlier than `timestamp_millis`.
pub fn scheduler_tick_ceil(timestamp_millis: u64, tick_millis: u64) -> Option<SchedulerTick> {
  if tick_millis == 0 {
    return None;
  }
  let quotient = timestamp_millis / tick_millis;
  if timestamp_millis.is_multiple_of(tick_millis) {
    Some(quotient)
  } else {
    quotient.checked_add(1)
  }
}

/// Anchors a newly admitted cadence so its first deadline is never earlier than one full period.
pub fn first_cadence_due_tick(
  timestamp_millis: u64,
  tick_millis: u64,
  every_ticks: SchedulerTick,
) -> Option<SchedulerTick> {
  if every_ticks == 0 {
    return None;
  }
  scheduler_tick_ceil(timestamp_millis, tick_millis)?.checked_add(every_ticks)
}

/// Returns the first cadence point strictly after `now_tick`, coalescing every missed period.
pub fn next_cadence_due_tick(
  anchor_tick: SchedulerTick,
  every_ticks: SchedulerTick,
  now_tick: SchedulerTick,
) -> Option<SchedulerTick> {
  if every_ticks == 0 {
    return None;
  }
  let first_due = anchor_tick.checked_add(every_ticks)?;
  let lower = now_tick.checked_add(1)?;
  if lower <= first_due {
    return Some(first_due);
  }
  let delta = lower.checked_sub(anchor_tick)?;
  let periods = delta.div_ceil(every_ticks);
  anchor_tick.checked_add(periods.checked_mul(every_ticks)?)
}

#[cfg(test)]
mod cadence_tick_tests {
  use super::{
    first_cadence_due_tick, next_cadence_due_tick, scheduler_tick_ceil, scheduler_tick_floor,
  };

  const TICK_MILLIS: u64 = 500;
  const FEE_SINK_PERIOD_TICKS: u64 = 120;

  #[test]
  fn tick_quantization_floors_readiness_and_ceils_activation() {
    for (timestamp, floor, ceil) in [(0, 0, 0), (1, 0, 1), (499, 0, 1), (500, 1, 1), (501, 1, 2)] {
      assert_eq!(scheduler_tick_floor(timestamp, TICK_MILLIS), Some(floor));
      assert_eq!(scheduler_tick_ceil(timestamp, TICK_MILLIS), Some(ceil));
    }
    assert_eq!(scheduler_tick_floor(1, 0), None);
    assert_eq!(scheduler_tick_ceil(1, 0), None);
  }

  #[test]
  fn fee_sink_first_deadline_never_shortens_sixty_seconds() {
    for timestamp in [0, 1, 499, 500, 501] {
      let due_tick = first_cadence_due_tick(timestamp, TICK_MILLIS, FEE_SINK_PERIOD_TICKS)
        .expect("valid cadence arithmetic");
      let due_millis = due_tick * TICK_MILLIS;
      assert!(due_millis - timestamp >= 60_000);
      assert!(
        scheduler_tick_floor(due_millis - 1, TICK_MILLIS).expect("nonzero tick duration")
          < due_tick
      );
      assert_eq!(
        scheduler_tick_floor(due_millis, TICK_MILLIS),
        Some(due_tick)
      );
    }
  }

  #[test]
  fn delayed_cadence_coalesces_missed_periods_without_catch_up() {
    assert_eq!(next_cadence_due_tick(1, 120, 1), Some(121));
    assert_eq!(next_cadence_due_tick(1, 120, 120), Some(121));
    assert_eq!(next_cadence_due_tick(1, 120, 121), Some(241));
    assert_eq!(next_cadence_due_tick(1, 120, 500), Some(601));
    assert_eq!(next_cadence_due_tick(1, 0, 1), None);
    assert_eq!(next_cadence_due_tick(u64::MAX, 1, u64::MAX), None);
  }
}

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Default,
  Encode,
  Eq,
  PartialEq,
  TypeInfo,
  MaxEncodedLen,
)]
pub enum IdleStarvationPhase {
  #[default]
  Healthy,
  Starving {
    consecutive_blocks: u32,
  },
  Alerted {
    consecutive_blocks: u32,
  },
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct WakeupPointer<BlockNumber> {
  pub block: WakeupKey<BlockNumber>,
  pub page_id: WakeupPageId,
  pub slot: WakeupSlot,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct TriggerWakeupPointer {
  pub tick: SchedulerTick,
  pub page_id: WakeupPageId,
  pub slot: WakeupSlot,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct WakeupEntry {
  pub actor_id: ActorId,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct WakeupPage<Entries> {
  pub entries: Entries,
  pub live_entries: u32,
  pub scan_slot: WakeupSlot,
  pub previous_page: Option<WakeupPageId>,
  pub next_page: Option<WakeupPageId>,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct WakeupBucketState {
  pub head_page: WakeupPageId,
  pub tail_page: WakeupPageId,
  pub next_page_id: WakeupPageId,
  pub live_entries: u32,
  pub cursor_index: Option<WakeupCursorIndex>,
}

pub type QueueEntry<BlockNumber> = ActorStepTicket<BlockNumber, ActorContractCommitment<[u8; 32]>>;

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorStepTicket<BlockNumber, ContractCommitment> {
  pub actor_id: ActorId,
  pub cycle_nonce: u64,
  pub cursor: u32,
  pub ticket: QueueTicket,
  pub eligible_at: BlockNumber,
  pub contract_commitment: ContractCommitment,
}

impl<BlockNumber: PartialEq, ContractCommitment: PartialEq>
  ActorStepTicket<BlockNumber, ContractCommitment>
{
  pub fn matches(
    &self,
    actor_id: ActorId,
    cycle_nonce: u64,
    cursor: u32,
    ticket: QueueTicket,
    eligible_at: &BlockNumber,
    contract_commitment: &ContractCommitment,
  ) -> bool {
    self.actor_id == actor_id
      && self.cycle_nonce == cycle_nonce
      && self.cursor == cursor
      && self.ticket == ticket
      && &self.eligible_at == eligible_at
      && &self.contract_commitment == contract_commitment
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QueueDrainStats {
  pub entries_scanned: u32,
  pub tombstones_skipped: u32,
  pub pages_touched: u32,
  pub pages_deleted: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WakeupDrainStats {
  pub entries_scanned: u32,
  pub ready_entries: u32,
  pub stale_entries: u32,
  pub pages_touched: u32,
  pub pages_deleted: u32,
}
