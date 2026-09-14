use super::{
  contract::{
    CrossingDirection, CrossingPhase, OpeningSurface, ScheduleWindow, Trigger, TriggerFamily,
  },
  scheduler::{TriggerWakeupPointer, WakeupKey, WakeupPointer},
};
use frame::prelude::*;

pub type ActorId = u64;
pub type ActorGeneration = u64;

/// Generation-bound identity used by every future process-residence index.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorRef {
  pub actor_id: ActorId,
  pub generation: ActorGeneration,
}

/// Service-ring role. Live continuations and Pending activation checks share one carrier.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ServiceResidenceKind {
  Live,
  Pending,
}

/// Why a current-state activation check may remain parked.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ParkNegativeReason {
  PredicateFalse,
  SourceUnavailable,
  MonotonicBoundaryPassed,
}

/// Process-owned identity for a negative check. Dependency registrations and their revisions remain
/// carrier-owned reverse handles; this header prevents a Park residence from being inferred from
/// missing scheduler membership.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ParkEvidence<BlockNumber> {
  pub plan_identity: [u8; 32],
  pub reason: ParkNegativeReason,
  pub review_at: Option<BlockNumber>,
}

/// Exact executable residence owned by one serving Actor-generation process.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ProcessResidence<BlockNumber> {
  Service(ServiceResidenceKind),
  Deadline {
    key: WakeupKey<BlockNumber>,
    page: u64,
    slot: u8,
  },
  Parked(ParkEvidence<BlockNumber>),
}

/// Typed reversible reason. Park is deliberately absent because negative current-state evidence is
/// a serving residence rather than lifecycle authority.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ProcessDisableCause {
  OwnerPaused,
  OwnerDeactivated,
  Protocol,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ProcessRevivalAuthority {
  Owner,
  SystemOrigin,
  Protocol,
}

/// Semantic basis retained while service authority is revoked. The canonical run record continues
/// to own committed counters, retry history, and the exact Step cursor.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum SuspendedProcessBasis<BlockNumber> {
  Idle,
  Running { eligible_at: BlockNumber },
  Suspended { not_before: BlockNumber },
  Dormant,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ProcessDisablement<BlockNumber> {
  pub cause: ProcessDisableCause,
  pub revival_authority: ProcessRevivalAuthority,
  pub basis: SuspendedProcessBasis<BlockNumber>,
}

/// Sole lifecycle authority for a stable process. Only Serving may carry an executable residence;
/// Retired is irreversible and leaves only future generation-bound cleanup authority.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ProcessStatus<BlockNumber> {
  Serving,
  Disabled(ProcessDisablement<BlockNumber>),
  Retired(CloseReason),
}

/// Minimal stable process owner introduced ahead of the atomic scheduler cutover.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorProcess<BlockNumber> {
  pub generation: ActorGeneration,
  pub last_attempted: Option<BlockNumber>,
  pub status: ProcessStatus<BlockNumber>,
  pub residence: Option<ProcessResidence<BlockNumber>>,
}

/// Legacy placement input for the storage-free process cutover compiler. Unsignaled intentionally
/// retains an explicit evidence hole instead of guessing Park or lifecycle disablement.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum LegacyProcessPlacement<BlockNumber> {
  Ready(ServiceResidenceKind),
  Waiting {
    key: WakeupKey<BlockNumber>,
    page: u64,
    slot: u8,
  },
  Unsignaled(Option<UnsignaledProcessEvidence<BlockNumber>>),
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum UnsignaledProcessEvidence<BlockNumber> {
  Parked(ParkEvidence<BlockNumber>),
  Disabled(ProcessDisablement<BlockNumber>),
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ProcessCompileError {
  AmbiguousUnsignaled,
  MalformedControlCell,
}

/// Cutover obligation assigned to every owner of legacy control-placement mutation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProcessTransitionObligation {
  PublishTypedResidence,
  PreserveProcess,
  AtomicSuccessorOrRemoval,
  RetireOrDisable,
  CarrierOnly,
}

/// Typed evidence supplied by a legacy mutation owner to the storage-free cutover planner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyProcessTransition<BlockNumber> {
  Publish(LegacyProcessPlacement<BlockNumber>),
  Preserve,
  Replace(Option<LegacyProcessPlacement<BlockNumber>>),
  Disable(ProcessDisablement<BlockNumber>),
  Retire(CloseReason),
  CarrierOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessTransitionError {
  InvalidCurrentProcess,
  ObligationMismatch,
  DetachWithoutSuccessor,
  Compile(ProcessCompileError),
}

/// Publication-boundary failures kept distinct from pure transition-planning failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessPublicationError {
  TransactionRequired,
  LegacyAuthorityPresent,
  ProcessAlreadyExists,
  ProcessMissing,
  CurrentProcessMismatch,
  Transition(ProcessTransitionError),
}

/// Pure compiler used to prove the legacy-to-process mapping before any storage authority moves.
pub fn compile_legacy_process<BlockNumber>(
  generation: ActorGeneration,
  last_attempted: Option<BlockNumber>,
  placement: LegacyProcessPlacement<BlockNumber>,
) -> Result<ActorProcess<BlockNumber>, ProcessCompileError> {
  let (status, residence) = match placement {
    LegacyProcessPlacement::Ready(kind) => (
      ProcessStatus::Serving,
      Some(ProcessResidence::Service(kind)),
    ),
    LegacyProcessPlacement::Waiting { key, page, slot } => (
      ProcessStatus::Serving,
      Some(ProcessResidence::Deadline { key, page, slot }),
    ),
    LegacyProcessPlacement::Unsignaled(Some(UnsignaledProcessEvidence::Parked(evidence))) => (
      ProcessStatus::Serving,
      Some(ProcessResidence::Parked(evidence)),
    ),
    LegacyProcessPlacement::Unsignaled(Some(UnsignaledProcessEvidence::Disabled(disablement))) => {
      (ProcessStatus::Disabled(disablement), None)
    }
    LegacyProcessPlacement::Unsignaled(None) => {
      return Err(ProcessCompileError::AmbiguousUnsignaled);
    }
  };
  Ok(ActorProcess {
    generation,
    last_attempted,
    status,
    residence,
  })
}

/// Plans one legacy control-owner mutation without publishing process storage. The obligation makes
/// the owner inventory exhaustive; typed evidence prevents detach-first and ambiguous Unsignaled
/// transitions from becoming a process state.
pub fn plan_legacy_process_transition<BlockNumber: Copy>(
  current: ActorProcess<BlockNumber>,
  obligation: ProcessTransitionObligation,
  transition: LegacyProcessTransition<BlockNumber>,
) -> Result<ActorProcess<BlockNumber>, ProcessTransitionError> {
  let coherent = matches!(
    (current.status, current.residence),
    (ProcessStatus::Serving, Some(_))
      | (ProcessStatus::Disabled(_), None)
      | (ProcessStatus::Retired(_), None)
  );
  if !coherent {
    return Err(ProcessTransitionError::InvalidCurrentProcess);
  }

  match (obligation, transition) {
    (
      ProcessTransitionObligation::PublishTypedResidence,
      LegacyProcessTransition::Publish(next),
    )
    | (
      ProcessTransitionObligation::AtomicSuccessorOrRemoval,
      LegacyProcessTransition::Replace(Some(next)),
    ) => compile_legacy_process(current.generation, current.last_attempted, next)
      .map_err(ProcessTransitionError::Compile),
    (ProcessTransitionObligation::PreserveProcess, LegacyProcessTransition::Preserve)
    | (ProcessTransitionObligation::CarrierOnly, LegacyProcessTransition::CarrierOnly) => {
      Ok(current)
    }
    (
      ProcessTransitionObligation::AtomicSuccessorOrRemoval,
      LegacyProcessTransition::Replace(None),
    ) => Err(ProcessTransitionError::DetachWithoutSuccessor),
    (
      ProcessTransitionObligation::AtomicSuccessorOrRemoval
      | ProcessTransitionObligation::RetireOrDisable,
      LegacyProcessTransition::Disable(disablement),
    ) => Ok(ActorProcess {
      generation: current.generation,
      last_attempted: current.last_attempted,
      status: ProcessStatus::Disabled(disablement),
      residence: None,
    }),
    (
      ProcessTransitionObligation::AtomicSuccessorOrRemoval
      | ProcessTransitionObligation::RetireOrDisable,
      LegacyProcessTransition::Retire(reason),
    ) => Ok(ActorProcess {
      generation: current.generation,
      last_attempted: current.last_attempted,
      status: ProcessStatus::Retired(reason),
      residence: None,
    }),
    _ => Err(ProcessTransitionError::ObligationMismatch),
  }
}

pub const ACTOR_RUN_PAYLOAD_HASH_DOMAIN: &[u8] = b"DEOS_ACTOR_RUN_PAYLOAD";
pub const PIPELINE_SERVICE_IDENTITY_HASH_DOMAIN: &[u8] = b"DEOS_PIPELINE_SERVICE_IDENTITY";

pub fn pipeline_service_identity(admission_identity: [u8; 32]) -> [u8; 32] {
  (PIPELINE_SERVICE_IDENTITY_HASH_DOMAIN, admission_identity)
    .using_encoded(frame::hashing::blake2_256)
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ActorType {
  User,
  System,
}

/// Lifecycle at the moment a fresh actor identity is created. Excludes Paused by
/// construction; a newly created actor is either Dormant or Active.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum InitialLifecycle {
  Dormant,
  Active,
}

pub type SystemSovereignId = u64;

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum SystemSovereignState {
  Vacant,
  Occupied(ActorId),
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
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
pub enum Mutability {
  #[default]
  Mutable,
  Immutable,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
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
pub enum CloseReason {
  OwnerInitiated,
  CycleAdmissionInsufficient,
  TriggerAdmissionInsufficient,
  ConsecutiveFailures,
  WindowExpired,
  CycleNonceExhausted,
  AutoCloseNonceReached,
  RetryAttemptsExhausted,
  ProductiveCycleCompleted,
  SchedulerIndexExhausted,
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
pub enum CompletionPolicy {
  #[default]
  Persistent,
  CloseAfterProductiveCycle,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
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
pub enum SuspensionReason {
  FundingUnavailable,
  Temporary,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum CycleResult {
  Completed,
  Failed,
  Cancelled,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum CancellationReason {
  Explicit,
  ContractReplaced,
  Deactivated,
  Closing(CloseReason),
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum StepSkippedReason {
  PreconditionFalse,
  ResolutionSkipped,
  FundingUnavailable,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum SimulationMode {
  FreshCurrentPlan,
  CurrentRun,
}

/// Final disposition of one canonical production or simulated attempt.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum AttemptDisposition {
  Completed,
  Continued,
  Failed,
  Suspended,
  Closed(CloseReason),
}

/// Canonical result produced once for each visited Step before its error policy is interpreted.
#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
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
pub struct SimulationStepRecord {
  pub step_index: u32,
  pub outcome: StepOutcome,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum SimulationError {
  TransactionDepthExceeded,
  Classification(ActorClassificationError),
  ActorNotFound,
  TypeMismatch,
  MutabilityMismatch,
  InvalidContract,
  InvalidBudget,
  ContractMismatch,
  ModeCycleStateMismatch,
  GlobalCircuitBreaker,
  Paused,
  NotReady,
  ResourceDeferred,
  FeeCollectionFailed,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct SimulationResult {
  pub status: AttemptDisposition,
  pub cycle_nonce: u64,
  pub start_cursor: u32,
  pub run_cursor: Option<u32>,
  pub unsuccessful_attempts_at_cursor: Option<u32>,
  pub cumulative_outcomes: OutcomeTotals,
  pub steps: BoundedVec<SimulationStepRecord, ConstU32<1>>,
}

/// Internal execution-phase output of the canonical actor classifier.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ActorExecutionPhase<BlockNumber> {
  GlobalCircuitBreaker,
  Paused,
  WaitingRetry(BlockNumber),
  WaitingBlock(BlockNumber),
  WaitingCadenceTick(u64),
  WaitingSignal,
  Ready,
}

/// Internal canonical actor classification shared by runtime projections.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorClassification<BlockNumber> {
  pub terminal_reason: Option<CloseReason>,
  pub execution_phase: ActorExecutionPhase<BlockNumber>,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ActorClassificationError {
  ActorInvariant,
  RunInvariant,
  ComputationOverflow,
}

/// One read-only eligibility algebra. Active actors expose the canonical
/// classification directly, retaining every retry and temporal block payload.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ActorActivationPlacement<BlockNumber> {
  Unplaced,
  Queue(u64),
  Wakeup(WakeupKey<BlockNumber>),
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ActorTriggerActivation<FeedId> {
  Manual,
  AddressEvent,
  ObservationChange {
    feed: FeedId,
    subscriber_count: u32,
    pending_revision: Option<u64>,
  },
  ObservationCrossing {
    feed: FeedId,
    direction: CrossingDirection,
    threshold: u128,
    rearm_threshold: u128,
    phase: CrossingPhase,
    installed_at_revision: u64,
    pending_revisions: u32,
    processing_revision: Option<u64>,
  },
  AtTime {
    after_ticks: u64,
    consumed: bool,
  },
  Cadenced {
    every_ticks: u64,
  },
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActiveActorActivation<FeedId, BlockNumber> {
  pub trigger: ActorTriggerActivation<FeedId>,
  pub pending_signal: bool,
  pub placement: ActorActivationPlacement<BlockNumber>,
  pub eligibility: ActorClassification<BlockNumber>,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ActorEligibility<FeedId, BlockNumber> {
  NotRegistered,
  Dormant,
  Active(ActiveActorActivation<FeedId, BlockNumber>),
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum TriggerCauseProvenance {
  ExternalPhase,
  Deferred,
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
pub enum CycleState {
  #[default]
  Idle,
  Running,
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
pub struct OutcomeTotals {
  pub executed_steps: u32,
  pub committed_effectful_tasks: u32,
  pub precondition_skips: u32,
  pub skipped_resolution: u32,
  pub skipped_funding_unavailable: u32,
  pub failed_steps: u32,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorRunAuthority<Hash> {
  pub semantic_contract_id: Hash,
  pub body_commitment: Hash,
  pub admission_identity: Hash,
  pub pipeline_service_identity: Hash,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorRunHead<BlockNumber> {
  pub contract_authority: ActorRunAuthority<[u8; 32]>,
  pub payload_commitment: [u8; 32],
  pub cycle_nonce: u64,
  pub cursor: u32,
  pub unsuccessful_attempts_at_cursor: u32,
  pub last_attempt_block: BlockNumber,
  pub last_committed_step_block: Option<BlockNumber>,
  pub eligible_at: BlockNumber,
  pub cumulative_outcomes: OutcomeTotals,
  pub last_step_outcome: Option<StepOutcome>,
  pub suspension: Option<SuspensionReason>,
}

impl<BlockNumber> ActorRunHead<BlockNumber> {
  pub fn has_contract_authority(
    &self,
    semantic_contract_id: [u8; 32],
    body_commitment: [u8; 32],
    admission_identity: [u8; 32],
  ) -> bool {
    self.contract_authority
      == ActorRunAuthority {
        semantic_contract_id,
        body_commitment,
        admission_identity,
        pipeline_service_identity: pipeline_service_identity(admission_identity),
      }
  }

  pub fn running_is_coherent(&self) -> bool
  where
    BlockNumber: PartialOrd,
  {
    self.suspension.is_none()
      && self
        .last_committed_step_block
        .as_ref()
        .is_some_and(|last_committed| last_committed < &self.eligible_at)
  }

  pub fn suspension_is_coherent(&self) -> bool {
    matches!(
      (&self.last_step_outcome, self.suspension),
      (
        Some(StepOutcome::FundingUnavailable),
        Some(SuspensionReason::FundingUnavailable)
      ) | (
        Some(StepOutcome::Failed(crate::TaskFailure {
          retry: crate::RetryClass::Temporary,
          ..
        })),
        Some(SuspensionReason::Temporary)
      )
    )
  }
}

#[derive(Debug, Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxSnapshotEntries))]
pub struct ActorRunPayload<AssetId, Balance, MaxSnapshotEntries: Get<u32>> {
  pub opening_snapshot: BoundedBTreeMap<OpeningSurface<AssetId>, Balance, MaxSnapshotEntries>,
}

#[derive(Debug, Decode, DecodeWithMemTracking, Encode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxSnapshotEntries))]
pub struct ActorRunState<AssetId, Balance, BlockNumber, MaxSnapshotEntries: Get<u32>> {
  pub contract_authority: ActorRunAuthority<[u8; 32]>,
  pub cycle_nonce: u64,
  pub cursor: u32,
  pub unsuccessful_attempts_at_cursor: u32,
  pub last_attempt_block: BlockNumber,
  pub last_committed_step_block: Option<BlockNumber>,
  pub eligible_at: BlockNumber,
  pub opening_snapshot: BoundedBTreeMap<OpeningSurface<AssetId>, Balance, MaxSnapshotEntries>,
  pub cumulative_outcomes: OutcomeTotals,
  pub last_step_outcome: Option<StepOutcome>,
  pub suspension: Option<SuspensionReason>,
}

impl<AssetId: Clone + Ord, Balance: Clone, BlockNumber: Clone, MaxSnapshotEntries: Get<u32>> Clone
  for ActorRunState<AssetId, Balance, BlockNumber, MaxSnapshotEntries>
{
  fn clone(&self) -> Self {
    Self {
      contract_authority: self.contract_authority,
      cycle_nonce: self.cycle_nonce,
      cursor: self.cursor,
      unsuccessful_attempts_at_cursor: self.unsuccessful_attempts_at_cursor,
      last_attempt_block: self.last_attempt_block.clone(),
      last_committed_step_block: self.last_committed_step_block.clone(),
      eligible_at: self.eligible_at.clone(),
      opening_snapshot: self.opening_snapshot.clone(),
      cumulative_outcomes: self.cumulative_outcomes,
      last_step_outcome: self.last_step_outcome.clone(),
      suspension: self.suspension,
    }
  }
}

impl<AssetId: Encode, Balance: Encode, BlockNumber, MaxSnapshotEntries: Get<u32>>
  ActorRunState<AssetId, Balance, BlockNumber, MaxSnapshotEntries>
{
  pub fn into_tiers(
    self,
  ) -> (
    ActorRunHead<BlockNumber>,
    ActorRunPayload<AssetId, Balance, MaxSnapshotEntries>,
  ) {
    let payload = ActorRunPayload {
      opening_snapshot: self.opening_snapshot,
    };
    let payload_commitment =
      (ACTOR_RUN_PAYLOAD_HASH_DOMAIN, &payload).using_encoded(frame::hashing::blake2_256);
    (
      ActorRunHead {
        contract_authority: self.contract_authority,
        payload_commitment,
        cycle_nonce: self.cycle_nonce,
        cursor: self.cursor,
        unsuccessful_attempts_at_cursor: self.unsuccessful_attempts_at_cursor,
        last_attempt_block: self.last_attempt_block,
        last_committed_step_block: self.last_committed_step_block,
        eligible_at: self.eligible_at,
        cumulative_outcomes: self.cumulative_outcomes,
        last_step_outcome: self.last_step_outcome,
        suspension: self.suspension,
      },
      payload,
    )
  }

  pub fn from_tiers(
    head: ActorRunHead<BlockNumber>,
    payload: ActorRunPayload<AssetId, Balance, MaxSnapshotEntries>,
  ) -> Option<Self> {
    if (ACTOR_RUN_PAYLOAD_HASH_DOMAIN, &payload).using_encoded(frame::hashing::blake2_256)
      != head.payload_commitment
    {
      return None;
    }
    Some(Self {
      contract_authority: head.contract_authority,
      cycle_nonce: head.cycle_nonce,
      cursor: head.cursor,
      unsuccessful_attempts_at_cursor: head.unsuccessful_attempts_at_cursor,
      last_attempt_block: head.last_attempt_block,
      last_committed_step_block: head.last_committed_step_block,
      eligible_at: head.eligible_at,
      opening_snapshot: payload.opening_snapshot,
      cumulative_outcomes: head.cumulative_outcomes,
      last_step_outcome: head.last_step_outcome,
      suspension: head.suspension,
    })
  }

  pub(crate) fn has_contract_authority(
    &self,
    semantic_contract_id: [u8; 32],
    body_commitment: [u8; 32],
    admission_identity: [u8; 32],
  ) -> bool {
    self.contract_authority
      == ActorRunAuthority {
        semantic_contract_id,
        body_commitment,
        admission_identity,
        pipeline_service_identity: pipeline_service_identity(admission_identity),
      }
  }

  pub(crate) fn running_is_coherent(&self) -> bool
  where
    BlockNumber: PartialOrd,
  {
    self.suspension.is_none()
      && self
        .last_committed_step_block
        .as_ref()
        .is_some_and(|last_committed| last_committed < &self.eligible_at)
  }

  pub(crate) fn suspension_is_coherent(&self) -> bool {
    matches!(
      (&self.last_step_outcome, self.suspension),
      (
        Some(StepOutcome::FundingUnavailable),
        Some(SuspensionReason::FundingUnavailable)
      ) | (
        Some(StepOutcome::Failed(crate::TaskFailure {
          retry: crate::RetryClass::Temporary,
          ..
        })),
        Some(SuspensionReason::Temporary)
      )
    )
  }
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorIdentity<AccountId, BlockNumber> {
  pub sovereign_account: AccountId,
  pub owner: AccountId,
  pub actor_class: ActorClass,
  pub mutability: Mutability,
  pub cycle_nonce: u64,
  pub last_control_mutation_block: BlockNumber,
}

/// Named refundable resource backing for one User Actor's exact retained geometry.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorStateHoldBreakdown<Balance> {
  pub identity: Balance,
  pub contract_head: Balance,
  pub contract_body: Balance,
  pub detector: Balance,
  pub run: Balance,
}

/// Per-Actor accounting authority for the owner's aggregate dedicated hold reason.
#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorStateHoldRecord<AccountId, Balance> {
  pub owner: AccountId,
  pub breakdown: ActorStateHoldBreakdown<Balance>,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum PipelineMachineFeeStrategy {
  UpfrontBounded,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorTriggerFeeQuote<Balance> {
  pub trigger_family: TriggerFamily,
  pub maximum_weight: Weight,
  pub fee: Balance,
  pub production_weight_identity: [u8; 32],
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorPipelineFeeQuote<Balance> {
  pub pipeline_machine_fee: Balance,
  pub cleanup_fee: Balance,
  pub total_fee: Balance,
  pub strategy: PipelineMachineFeeStrategy,
  pub admission_identity: [u8; 32],
  pub production_weight_identity: [u8; 32],
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorActionFeeQuote<Balance> {
  pub maximum_effect_weight: Weight,
  pub maximum_effect_fee: Balance,
  pub production_weight_identity: [u8; 32],
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorStateHoldQuote<Balance> {
  pub exempt: bool,
  pub base_per_component: Balance,
  pub per_encoded_byte: Balance,
  pub breakdown: ActorStateHoldBreakdown<Balance>,
  pub total: Balance,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorCostQuote<Balance> {
  pub actor_type: ActorType,
  pub creation_fee: Balance,
  pub prospective_trigger_fee: Option<ActorTriggerFeeQuote<Balance>>,
  pub prospective_pipeline_fee: Option<ActorPipelineFeeQuote<Balance>>,
  pub maximum_next_action_fee: ActorActionFeeQuote<Balance>,
  pub actor_state_hold: ActorStateHoldQuote<Balance>,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ActorCostQuoteError {
  ActorNotFound,
  ActorInvariant,
  ComputationOverflow,
  WeightAuthorityUnavailable,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum TriggerRuntimeState {
  Stateless,
  ObservationCrossing {
    phase: CrossingPhase,
    installed_at_revision: u64,
  },
  AtTime {
    anchor_tick: Option<u64>,
    consumed: bool,
  },
  Cadenced {
    anchor_tick: Option<u64>,
  },
}

impl TriggerRuntimeState {
  pub fn temporal_anchor_tick(&self) -> Option<u64> {
    match self {
      Self::AtTime { anchor_tick, .. } | Self::Cadenced { anchor_tick } => *anchor_tick,
      Self::Stateless | Self::ObservationCrossing { .. } => None,
    }
  }

  pub fn temporal_occurrence_consumed(&self) -> bool {
    matches!(self, Self::AtTime { consumed: true, .. })
  }

  pub fn is_compatible_with<AccountId, AssetId, MaxWhitelistSize, ObservationFeedId>(
    &self,
    trigger: &Trigger<AccountId, AssetId, MaxWhitelistSize, ObservationFeedId>,
  ) -> bool
  where
    MaxWhitelistSize: Get<u32>,
  {
    matches!(
      (self, trigger),
      (
        Self::Stateless,
        Trigger::Manual | Trigger::AddressEvent { .. } | Trigger::ObservationChange { .. }
      ) | (
        Self::ObservationCrossing { .. },
        Trigger::ObservationCrossing { .. }
      ) | (Self::AtTime { .. }, Trigger::AtTime { .. })
        | (Self::Cadenced { .. }, Trigger::Cadenced { .. })
    )
  }
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorHotState<BlockNumber> {
  pub lifecycle: ActiveLifecycle,
  pub cycle_state: CycleState,
  pub trigger_runtime_state: TriggerRuntimeState,
  pub unsuccessful_attempt_streak: u32,
  pub pending_signal: bool,
  pub queue_ticket: Option<u64>,
  pub wakeup_pointer: Option<WakeupPointer<BlockNumber>>,
  pub trigger_wakeup_pointer: Option<TriggerWakeupPointer>,
  pub terminal_at: Option<BlockNumber>,
  pub schedule_anchor: BlockNumber,
  pub last_cycle_block: Option<BlockNumber>,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActiveActorState<Identity, Hot, Contract, RunState> {
  pub identity: Identity,
  pub hot: Hot,
  pub contract: Contract,
  pub run_state: Option<RunState>,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub(crate) struct ActiveActorView<AccountId, BlockNumber, Trigger, Steps> {
  pub sovereign_account: AccountId,
  pub owner: AccountId,
  pub actor_class: ActorClass,
  pub mutability: Mutability,
  pub lifecycle: ActiveLifecycle,
  pub cycle_state: CycleState,
  pub trigger: Trigger,
  pub cooldown_blocks: u32,
  pub window: Option<ScheduleWindow<BlockNumber>>,
  pub steps: Steps,
  pub completion: CompletionPolicy,
  pub trigger_runtime_state: TriggerRuntimeState,
  pub cycle_nonce: u64,
  pub auto_close_at_cycle_nonce: Option<u64>,
  pub unsuccessful_attempt_streak: u32,
  pub pending_signal: bool,
  pub queue_ticket: Option<u64>,
  pub wakeup_pointer: Option<WakeupPointer<BlockNumber>>,
  pub trigger_wakeup_pointer: Option<TriggerWakeupPointer>,
  pub last_control_mutation_block: BlockNumber,
  pub schedule_anchor: BlockNumber,
  pub temporal_anchor_tick: Option<u64>,
  pub temporal_occurrence_consumed: bool,
  pub last_cycle_block: Option<BlockNumber>,
}
