use crate::{
  contract_types::{OpeningSurface, PredicateError, ScheduleWindow},
  scheduler_types::WakeupPointer,
};
use frame::prelude::*;

pub type ActorId = u64;

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum ActorType {
  User,
  System,
}

/// Lifecycle at the moment a fresh actor identity is created. Excludes Paused by
/// construction; a newly created actor is either Dormant or Active.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum InitialLifecycle {
  Dormant,
  Active,
}

pub type SystemSovereignId = u64;

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum SystemSovereignState {
  Vacant,
  Occupied(ActorId),
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum ActorClass {
  User { owner_slot: u8 },
  System { sovereign_id: SystemSovereignId },
}

impl ActorClass {
  pub fn actor_type(self) -> ActorType {
    match self {
      Self::User { .. } => ActorType::User,
      Self::System { .. } => ActorType::System,
    }
  }

  pub fn owner_slot(self) -> Option<u8> {
    match self {
      Self::User { owner_slot } => Some(owner_slot),
      Self::System { .. } => None,
    }
  }

  pub fn system_sovereign_id(self) -> Option<SystemSovereignId> {
    match self {
      Self::User { .. } => None,
      Self::System { sovereign_id } => Some(sovereign_id),
    }
  }
}

#[derive(
  Clone,
  Copy,
  Debug,
  Default,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  PartialEq,
  TypeInfo,
  MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum Mutability {
  #[default]
  Mutable,
  Immutable,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum ActiveLifecycle {
  Active,
  Paused,
}

impl ActiveLifecycle {
  pub fn is_paused(self) -> bool {
    matches!(self, Self::Paused)
  }
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum CloseReason {
  OwnerInitiated,
  BalanceExhausted,
  ConsecutiveFailures,
  WindowExpired,
  CycleNonceExhausted,
  FeeBudgetExhausted,
  AutoCloseNonceReached,
  RetryAttemptsExhausted,
  ProductiveCycleCompleted,
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
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum CompletionPolicy {
  #[default]
  Persistent,
  CloseAfterProductiveCycle,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum StepErrorPolicy {
  AbortCycle,
  ContinueNextStep,
  RetryLater { max_attempts: u32 },
}

impl StepErrorPolicy {
  pub fn retry_max_attempts(self) -> Option<u32> {
    match self {
      Self::RetryLater { max_attempts } => Some(max_attempts),
      Self::AbortCycle | Self::ContinueNextStep => None,
    }
  }
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum SuspensionReason {
  FundingUnavailable,
  Temporary,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum CycleResult {
  Completed,
  Failed,
  Cancelled,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum CancellationReason {
  Explicit,
  StepsChanged,
  CompletionChanged,
  FundingChanged,
  ScheduleChanged,
  Deactivated,
  Closing(CloseReason),
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum StepSkippedReason {
  PreconditionFalse,
  ResolutionSkipped,
  FundingUnavailable,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum SimulationMode {
  FreshCurrentPlan,
  CurrentContinuation,
}

/// Final disposition of one canonical production or simulated attempt.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum AttemptDisposition {
  Completed,
  Failed,
  Suspended,
  Closed(CloseReason),
}

/// Canonical result produced once for each visited Step before its error policy is interpreted.
#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum StepOutcome {
  Executed,
  Stopped,
  Skipped(StepSkippedReason),
  FundingUnavailable,
  Failed(crate::TaskFailure),
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub struct SimulationStepRecord {
  pub step_index: u32,
  pub outcome: StepOutcome,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum SimulationError {
  TransactionDepthExceeded,
  Classification(ActorClassificationError),
  ActorNotFound,
  TypeMismatch,
  MutabilityMismatch,
  ContractMismatch,
  ModeCycleStateMismatch,
  GlobalCircuitBreaker,
  Paused,
  NotReady,
  FeeCollectionFailed,
}

impl From<polkadot_sdk::sp_runtime::DispatchError> for SimulationError {
  fn from(_: polkadot_sdk::sp_runtime::DispatchError) -> Self {
    Self::TransactionDepthExceeded
  }
}

#[derive(
  polkadot_sdk::frame_support::CloneNoBound,
  polkadot_sdk::frame_support::DebugNoBound,
  polkadot_sdk::frame_support::PartialEqNoBound,
  polkadot_sdk::frame_support::EqNoBound,
  Decode,
  DecodeWithMemTracking,
  Encode,
  TypeInfo,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
#[scale_info(skip_type_params(MaxContractSteps))]
pub struct SimulationResult<MaxContractSteps: Get<u32>> {
  pub status: AttemptDisposition,
  pub cycle_nonce: u64,
  pub start_cursor: u32,
  pub continuation_cursor: Option<u32>,
  pub unsuccessful_attempts_at_cursor: Option<u32>,
  pub cumulative_outcomes: OutcomeTotals,
  pub steps: BoundedVec<SimulationStepRecord, MaxContractSteps>,
}

/// Internal execution-phase output of the canonical actor classifier.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum ActorExecutionPhase<BlockNumber> {
  GlobalCircuitBreaker,
  Paused,
  WaitingRetry(BlockNumber),
  WaitingTemporal(BlockNumber),
  WaitingSignal,
  Ready,
}

/// Internal canonical actor classification shared by runtime projections.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub struct ActorClassification<BlockNumber> {
  pub terminal_reason: Option<CloseReason>,
  pub execution_phase: ActorExecutionPhase<BlockNumber>,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum ActorClassificationError {
  ActorInvariant,
  ContinuationInvariant,
  ComputationOverflow,
}

/// Read-only scheduler-owned readiness phase for the eligibility projection
/// (spec 7.3). Clients read one runtime API instead of reimplementing cadence
/// phase, cooldown, window floor, retry backoff, breaker, and latch arithmetic.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum ActorEligibilityPhase {
  /// No identity is registered under the id.
  NotRegistered,
  /// A dormant identity exists without an Active Actor Contract.
  Dormant,
  /// The actor is temporally and trigger-ready for scheduler admission now.
  Ready,
  /// Manual pause blocks execution.
  Paused,
  /// The global circuit breaker blocks all execution.
  GlobalCircuitBreaker,
  /// Classification found terminal liveness or configured closure due.
  CloseDue(CloseReason),
  /// The temporal gate is open but the pending-signal latch is absent.
  WaitingSignal,
  /// A suspended run waits for retry backoff or cooldown before the next attempt.
  WaitingRetry,
  /// Cooldown, window floor, or cadence has not yet opened the temporal gate.
  WaitingTemporal,
}

/// One read-only eligibility projection (spec 7.3). `phase` owns the scheduler
/// verdict; `next_eligible_block` is the next block at which temporal eligibility
/// opens (`now` when ready), or `None` when no future temporal gate is computable.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub struct ActorEligibilityProjection<BlockNumber> {
  pub phase: ActorEligibilityPhase,
  pub next_eligible_block: Option<BlockNumber>,
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
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub enum CycleState {
  #[default]
  Idle,
  Suspended,
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
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub struct OutcomeTotals {
  pub executed_steps: u32,
  pub committed_effectful_tasks: u32,
  pub precondition_skips: u32,
  pub skipped_resolution: u32,
  pub skipped_funding_unavailable: u32,
  pub failed_steps: u32,
}

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
#[scale_info(skip_type_params(
  MaxSnapshotEntries,
  MaxFundingTrackedAssets,
  MaxOpeningPredicateResults
))]
pub struct ContinuationState<
  AssetId,
  Balance,
  BlockNumber,
  MaxSnapshotEntries: Get<u32>,
  MaxFundingTrackedAssets: Get<u32>,
  MaxOpeningPredicateResults: Get<u32>,
> {
  pub cursor: u32,
  pub unsuccessful_attempts_at_cursor: u32,
  pub last_attempt_block: BlockNumber,
  pub opening_snapshot: BoundedBTreeMap<OpeningSurface<AssetId>, Balance, MaxSnapshotEntries>,
  pub opening_predicate_results:
    BoundedVec<Result<bool, PredicateError>, MaxOpeningPredicateResults>,
  pub funding_snapshot: BoundedBTreeMap<AssetId, Balance, MaxFundingTrackedAssets>,
  pub cumulative_outcomes: OutcomeTotals,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub struct ActorIdentity<AccountId, BlockNumber> {
  pub sovereign_account: AccountId,
  pub owner: AccountId,
  pub actor_class: ActorClass,
  pub mutability: Mutability,
  pub cycle_nonce: u64,
  pub last_control_mutation_block: BlockNumber,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub struct ActorHotState<BlockNumber> {
  pub lifecycle: ActiveLifecycle,
  pub cycle_state: CycleState,
  pub auto_close_at_cycle_nonce: Option<u64>,
  pub unsuccessful_attempt_streak: u32,
  pub pending_signal: bool,
  pub queue_ticket: Option<u64>,
  pub wakeup_pointer: Option<WakeupPointer<BlockNumber>>,
  pub terminal_at: Option<BlockNumber>,
  pub schedule_anchor: BlockNumber,
  pub last_cycle_block: Option<BlockNumber>,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
#[scale_info(replace_segment("lifecycle_types", "types"))]
pub struct ActiveActorView<AccountId, BlockNumber, Schedule, Steps> {
  pub sovereign_account: AccountId,
  pub owner: AccountId,
  pub actor_class: ActorClass,
  pub mutability: Mutability,
  pub lifecycle: ActiveLifecycle,
  pub cycle_state: CycleState,
  pub schedule: Schedule,
  pub schedule_window: Option<ScheduleWindow<BlockNumber>>,
  pub steps: Steps,
  pub completion: CompletionPolicy,
  pub cycle_nonce: u64,
  pub auto_close_at_cycle_nonce: Option<u64>,
  pub unsuccessful_attempt_streak: u32,
  pub pending_signal: bool,
  pub queue_ticket: Option<u64>,
  pub last_control_mutation_block: BlockNumber,
  pub schedule_anchor: BlockNumber,
  pub last_cycle_block: Option<BlockNumber>,
}
