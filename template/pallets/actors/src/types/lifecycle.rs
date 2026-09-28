use super::{
  contract::{ScheduleWindow, Trigger, TriggerFamily},
  scheduler::{TriggerWakeupPointer, WakeupKey, WakeupPointer},
};
use frame::prelude::*;

pub type ActorId = u64;
pub type ActorGeneration = u64;

/// One certified fixed-anchor balance dependency for a parked activation episode. The host's
/// current minimum remains an input to every classification so a changed asset definition cannot
/// silently reuse stale threshold authority.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ParkedBalanceWatch<AssetId, Balance> {
  pub asset: AssetId,
  pub authored_min_delta: Balance,
  pub certified_minimum_balance: Balance,
  pub anchor: Balance,
  pub acknowledged_revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParkedBalanceCertificationError {
  ZeroMinimumBalance,
  ThresholdOverflow,
  PlanCapacityExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParkedBalanceClassificationError {
  MinimumBalanceChanged,
  ThresholdOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParkedBalanceQualification {
  BelowThreshold,
  Qualified,
}

/// Certifies one watch without saturating the protocol floor of 100 asset minima.
pub fn certify_parked_balance_watch<AssetId, Balance>(
  asset: AssetId,
  authored_min_delta: Balance,
  minimum_balance: Balance,
  anchor: Balance,
  acknowledged_revision: u64,
) -> Result<ParkedBalanceWatch<AssetId, Balance>, ParkedBalanceCertificationError>
where
  Balance: Copy + CheckedMul + From<u8> + Ord + Zero,
{
  if minimum_balance.is_zero() {
    return Err(ParkedBalanceCertificationError::ZeroMinimumBalance);
  }
  minimum_balance
    .checked_mul(&Balance::from(100u8))
    .ok_or(ParkedBalanceCertificationError::ThresholdOverflow)?;
  Ok(ParkedBalanceWatch {
    asset,
    authored_min_delta,
    certified_minimum_balance: minimum_balance,
    anchor,
    acknowledged_revision,
  })
}

/// Classifies current total ownership against the immutable episode anchor. A negative check never
/// returns a replacement watch, which makes moving the anchor through sampling impossible.
pub fn classify_parked_balance<AssetId, Balance>(
  watch: &ParkedBalanceWatch<AssetId, Balance>,
  current_minimum_balance: Balance,
  current_total_balance: Balance,
) -> Result<ParkedBalanceQualification, ParkedBalanceClassificationError>
where
  Balance: Copy + CheckedMul + CheckedSub + From<u8> + Ord,
{
  if current_minimum_balance != watch.certified_minimum_balance {
    return Err(ParkedBalanceClassificationError::MinimumBalanceChanged);
  }
  let floor = current_minimum_balance
    .checked_mul(&Balance::from(100u8))
    .ok_or(ParkedBalanceClassificationError::ThresholdOverflow)?;
  let threshold = watch.authored_min_delta.max(floor);
  let delta = if current_total_balance >= watch.anchor {
    current_total_balance
      .checked_sub(&watch.anchor)
      .ok_or(ParkedBalanceClassificationError::ThresholdOverflow)?
  } else {
    watch
      .anchor
      .checked_sub(&current_total_balance)
      .ok_or(ParkedBalanceClassificationError::ThresholdOverflow)?
  };
  Ok(if delta >= threshold {
    ParkedBalanceQualification::Qualified
  } else {
    ParkedBalanceQualification::BelowThreshold
  })
}

/// Classifies one complete bounded watch plan from a single current host snapshot per asset. Every
/// certified minimum is checked even after one asset qualifies, so configuration drift cannot be
/// hidden by watch ordering.
pub fn classify_parked_balance_plan<AssetId, Balance, Observe>(
  watches: &[ParkedBalanceWatch<AssetId, Balance>],
  mut observe: Observe,
) -> Result<ParkedBalanceQualification, ParkedBalanceClassificationError>
where
  AssetId: Copy,
  Balance: Copy + CheckedMul + CheckedSub + From<u8> + Ord,
  Observe: FnMut(AssetId) -> (Balance, Balance),
{
  let mut qualification = ParkedBalanceQualification::BelowThreshold;
  for watch in watches
    .iter(/* deos-bypass: bounded-iter -- caller supplies a MaxAssets-bounded authored watch plan. */)
  {
    let (current_minimum_balance, current_total_balance) = observe(watch.asset);
    if classify_parked_balance(watch, current_minimum_balance, current_total_balance)?
      == ParkedBalanceQualification::Qualified
    {
      qualification = ParkedBalanceQualification::Qualified;
    }
  }
  Ok(qualification)
}

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
  ParkedBalanceBelowThreshold,
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

/// Carrier mutation required after one useful Trigger occurrence has been accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalOccurrencePublication<BlockNumber> {
  /// Idle readiness leaves its Park/Deadline source and enters Pending service at B+1.
  PublishPending { eligible_from: BlockNumber },
  /// A busy occurrence is only a deferred semantic latch; its current residence is retained.
  PreserveResidence,
}

/// Storage-neutral successor shared by every Trigger-family writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalOccurrencePlan<BlockNumber> {
  pub process: ActorProcess<BlockNumber>,
  pub pending_signal: bool,
  pub publication: CanonicalOccurrencePublication<BlockNumber>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalOccurrenceError {
  InvalidProcess,
  InvalidResidence,
  BlockNumberOverflow,
}

/// Plans the common occurrence transition without reading or writing storage. Duplicate latched
/// occurrences are rejected by the caller before matching/charging and therefore return no plan.
/// Idle readiness from a carrier-free Disabled process, a retained Idle Service resident, or a
/// Deadline/Park residence always enters Pending at B+1; Running/Suspended work preserves its exact
/// current Service or Deadline residence and changes only the deferred semantic latch.
pub fn plan_canonical_occurrence<BlockNumber>(
  cycle_state: CycleState,
  pending_signal: bool,
  process: ActorProcess<BlockNumber>,
  now: BlockNumber,
) -> Result<Option<CanonicalOccurrencePlan<BlockNumber>>, CanonicalOccurrenceError>
where
  BlockNumber: Copy + CheckedAdd + One,
{
  if pending_signal {
    return Ok(None);
  }
  if matches!(process.status, ProcessStatus::Retired(_)) {
    return Err(CanonicalOccurrenceError::InvalidProcess);
  }

  let (process, publication) = match cycle_state {
    CycleState::Idle => {
      if !matches!(
        (process.status, process.residence),
        (ProcessStatus::Disabled(_), None)
          | (
            ProcessStatus::Serving,
            Some(
              ProcessResidence::Deadline { .. }
                | ProcessResidence::Parked(_)
                | ProcessResidence::Service(_)
            ),
          )
      ) {
        return Err(CanonicalOccurrenceError::InvalidResidence);
      }
      let eligible_from = now
        .checked_add(&One::one())
        .ok_or(CanonicalOccurrenceError::BlockNumberOverflow)?;
      (
        ActorProcess {
          status: ProcessStatus::Serving,
          residence: Some(ProcessResidence::Service(ServiceResidenceKind::Pending)),
          ..process
        },
        CanonicalOccurrencePublication::PublishPending { eligible_from },
      )
    }
    CycleState::Running | CycleState::Suspended => {
      if !matches!(process.status, ProcessStatus::Serving)
        || !matches!(
          process.residence,
          Some(ProcessResidence::Service(ServiceResidenceKind::Live))
            | Some(ProcessResidence::Deadline { .. })
        )
      {
        return Err(CanonicalOccurrenceError::InvalidResidence);
      }
      (process, CanonicalOccurrencePublication::PreserveResidence)
    }
  };

  Ok(Some(CanonicalOccurrencePlan {
    process,
    pending_signal: true,
    publication,
  }))
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
pub enum ActorTriggerActivation {
  Manual,
  AddressEvent,
  AtTime { after_ticks: u64, consumed: bool },
  Cadenced { every_ticks: u64 },
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActiveActorActivation<BlockNumber> {
  pub trigger: ActorTriggerActivation,
  pub pending_signal: bool,
  pub placement: ActorActivationPlacement<BlockNumber>,
  pub eligibility: ActorClassification<BlockNumber>,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum ActorEligibility<BlockNumber> {
  NotRegistered,
  Dormant,
  Active(ActiveActorActivation<BlockNumber>),
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
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct ActorRunState<BlockNumber> {
  pub contract_authority: ActorRunAuthority<[u8; 32]>,
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

impl<BlockNumber> ActorRunState<BlockNumber> {
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

#[derive(Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
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
  pub total_fee: Balance,
  pub strategy: PipelineMachineFeeStrategy,
  pub admission_identity: [u8; 32],
  pub production_weight_identity: [u8; 32],
}

impl<Balance: MaxEncodedLen> MaxEncodedLen for ActorTriggerFeeQuote<Balance> {
  fn max_encoded_len() -> usize {
    (TriggerFamily::max_encoded_len()
      + super::resource::weight_max_encoded_len()
      + <[u8; 32]>::max_encoded_len())
    .saturating_add(Balance::max_encoded_len())
  }
}

#[derive(Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct ActorActionFeeQuote<Balance> {
  pub maximum_effect_weight: Weight,
  pub maximum_effect_fee: Balance,
  pub production_weight_identity: [u8; 32],
}

impl<Balance: MaxEncodedLen> MaxEncodedLen for ActorActionFeeQuote<Balance> {
  fn max_encoded_len() -> usize {
    (super::resource::weight_max_encoded_len() + <[u8; 32]>::max_encoded_len())
      .saturating_add(Balance::max_encoded_len())
  }
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
      Self::Stateless => None,
    }
  }

  pub fn temporal_occurrence_consumed(&self) -> bool {
    matches!(self, Self::AtTime { consumed: true, .. })
  }

  pub fn is_compatible_with<AccountId, AssetId, MaxWhitelistSize>(
    &self,
    trigger: &Trigger<AccountId, AssetId, MaxWhitelistSize>,
  ) -> bool
  where
    MaxWhitelistSize: Get<u32>,
  {
    matches!(
      (self, trigger),
      (
        Self::Stateless,
        Trigger::Manual | Trigger::AddressEvent { .. }
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
