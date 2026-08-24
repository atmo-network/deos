#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]

extern crate alloc;

use polkadot_sdk::{
  frame_support::{BoundedVec, traits::Get},
  sp_runtime::traits::{CheckedAdd, CheckedSub, Zero},
};

pub use pallet::*;

pub trait ActorPrepassContext {
  fn context_ready() -> bool;
}

impl ActorPrepassContext for () {
  fn context_ready() -> bool {
    true
  }
}

pub const ACTOR_PREPASS_INHERENT_VERSION: u8 = 1;

#[derive(codec::Decode, codec::Encode)]
pub struct ActorPrepassInherentData {
  version: u8,
}

pub fn provide_actor_prepass_inherent_data(
  data: &mut polkadot_sdk::sp_inherents::InherentData,
) -> Result<(), polkadot_sdk::sp_inherents::Error> {
  data.put_data(
    ACTOR_PREPASS_INHERENT_IDENTIFIER,
    &ActorPrepassInherentData {
      version: ACTOR_PREPASS_INHERENT_VERSION,
    },
  )
}

pub mod contract;

pub mod types;

mod crossing;
mod execution;
mod reactions;
mod scheduler;
mod subscriptions;

pub use scheduler::EnqueueOutcome;

pub mod adapters;
pub use adapters::{
  AddressEventIngress, AdmissionCertificateAuthority, AdmissionCertificateAuthorityProvider,
  AssetOps, CanonicalObservationState, DexOps, DexSwapOutcome, ExecutionContext, FundingAuthority,
  IngressFailure, LiquidityOps, ObservationProvider, ObservationTransition,
  ObservationTransitionIngress, RetryClass, ScalarObservationState, SovereignAccountDeriver,
  StakingOps, StepControlExecution, StepControlOutcome, StepControlPhase, StepControlPlacement,
  StepControlWeightContext, StepControlWeightProvider, SystemActorContractValidator,
  TaskEffectExecution, TaskEffectWeightProvider, TaskFailure,
};
pub use types::{
  ActorStepResourceReservation, AddressEvent, BlockResourceBudget, BlockResourceDomain,
  BlockResourceLimits, BlockResourcePhase, BlockResourceReservation, BlockResourceState,
  CrossingCapacity, FinalizedBlockResourceSnapshot, FixedBlockWeightComponents, InputLimit,
  MAX_STEPS_PER_TAIL_CHUNK, MaterializationFaults, Task, WakeupBucketState, WakeupCursorIndex,
};

pub mod weights;
pub use weights::WeightInfo;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AccountId, AssetId, Balance, ObservationFeedId> {
  fn setup_add_liquidity(
    owner: &AccountId,
  ) -> Result<(AssetId, AssetId, Balance, Balance), polkadot_sdk::sp_runtime::DispatchError>;
  fn setup_donate_liquidity(
    owner: &AccountId,
  ) -> Result<(AssetId, AssetId, Balance), polkadot_sdk::sp_runtime::DispatchError>;
  fn setup_remove_liquidity(
    owner: &AccountId,
  ) -> Result<(AssetId, AssetId, AssetId, Balance), polkadot_sdk::sp_runtime::DispatchError>;
  fn setup_stake(
    owner: &AccountId,
  ) -> Result<(AssetId, Balance), polkadot_sdk::sp_runtime::DispatchError>;
  fn setup_unstake(
    owner: &AccountId,
  ) -> Result<(AssetId, Balance), polkadot_sdk::sp_runtime::DispatchError>;
  fn setup_swap_exact_in(
    owner: &AccountId,
  ) -> Result<(AssetId, AssetId, Balance), polkadot_sdk::sp_runtime::DispatchError>;
  fn setup_swap_exact_out(
    owner: &AccountId,
  ) -> Result<(AssetId, AssetId, Balance, Balance), polkadot_sdk::sp_runtime::DispatchError>;
  fn funding_assets(max: u32) -> alloc::vec::Vec<AssetId>;
  fn setup_predicate_assets(
    owner: &AccountId,
    max: u32,
  ) -> Result<alloc::vec::Vec<AssetId>, polkadot_sdk::sp_runtime::DispatchError>;
  fn setup_observation_feeds(
    max: u32,
  ) -> Result<alloc::vec::Vec<ObservationFeedId>, polkadot_sdk::sp_runtime::DispatchError>;
  fn enable_asset_ops_ingress() {}
  fn setup_address_event_ingress(
    recipient: &AccountId,
    source: &AccountId,
    amount: Balance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult;
  fn run_address_event_ingress(recipient: &AccountId, source: &AccountId, amount: Balance) -> bool;
  fn setup_xcm_asset_deposit() -> polkadot_sdk::sp_runtime::DispatchResult;
  fn run_xcm_asset_deposit(
    recipient: &AccountId,
    source: &AccountId,
    amount: Balance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult;
  type MaximumContextInherent;
  fn prepare_maximum_context_inherent() -> Self::MaximumContextInherent;
  fn execute_maximum_context_inherent(
    inherent: Self::MaximumContextInherent,
  ) -> polkadot_sdk::sp_runtime::DispatchResult;
  fn verify_maximum_context_inherent();
  fn prepare_maximum_xcm_version_discovery();
  fn execute_maximum_xcm_version_discovery();
  fn verify_maximum_xcm_version_discovery();
  fn prepare_block_resource_meter_extension();
  fn execute_block_resource_meter_extension();
  fn verify_block_resource_meter_extension();
}

pub trait FeeCollector<AccountId, AssetId, Balance> {
  fn collect_fee(
    payer: &AccountId,
    fee_sink: &AccountId,
    native_asset: AssetId,
    amount: Balance,
  ) -> polkadot_sdk::frame_support::dispatch::DispatchResult;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeEnvelopeInput<Balance> {
  pub evaluation: Balance,
  pub execution: Balance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepFeeEnvelope<Balance> {
  pub evaluation: Balance,
  pub execution: Balance,
  pub total: Balance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TriggerFeeBreakdown<Balance> {
  pub trigger_family: types::TriggerFamily,
  pub trigger_fee: Balance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PipelineFeeBreakdown<Balance> {
  pub pipeline_machine_fee: Balance,
  pub cleanup_fee: Balance,
  pub total_fee: Balance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepFeeBreakdown<Balance> {
  pub control_fee: Balance,
  pub effect_fee: Balance,
  pub total_fee: Balance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeeChargeKind {
  EvaluationOnly,
  Attempted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeStepSettlement<Balance> {
  pub charged: Balance,
  pub reservation_remaining: Balance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttemptFeeEnvelope<Balance, MaxSteps: Get<u32>> {
  pub steps: BoundedVec<StepFeeEnvelope<Balance>, MaxSteps>,
  pub total: Balance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeeEnvelopeError {
  CursorOutOfBounds,
  Overflow,
  ReservationUnderflow,
}

pub fn compose_attempt_fee_envelope<Balance, MaxSteps>(
  actor_type: types::ActorType,
  inputs: &BoundedVec<FeeEnvelopeInput<Balance>, MaxSteps>,
  start_cursor: usize,
) -> Result<AttemptFeeEnvelope<Balance, MaxSteps>, FeeEnvelopeError>
where
  Balance: Copy + CheckedAdd + Zero,
  MaxSteps: Get<u32>,
{
  if start_cursor > inputs.len() {
    return Err(FeeEnvelopeError::CursorOutOfBounds);
  }
  let mut steps = BoundedVec::default();
  let mut total = Balance::zero();
  for index in start_cursor..inputs.len() {
    let input = &inputs[index];
    let evaluation = if actor_type == types::ActorType::User {
      input.evaluation
    } else {
      Balance::zero()
    };
    let execution = if actor_type == types::ActorType::User {
      input.execution
    } else {
      Balance::zero()
    };
    let step_total = evaluation
      .checked_add(&execution)
      .ok_or(FeeEnvelopeError::Overflow)?;
    total = total
      .checked_add(&step_total)
      .ok_or(FeeEnvelopeError::Overflow)?;
    steps
      .try_push(StepFeeEnvelope {
        evaluation,
        execution,
        total: step_total,
      })
      .map_err(|_| FeeEnvelopeError::Overflow)?;
  }
  Ok(AttemptFeeEnvelope { steps, total })
}

/// Returns the preserve-spend floor for a direct or adapter-reported debit surface.
pub fn fee_native_protected_minimum<Balance: Ord>(
  actor_type: types::ActorType,
  is_fee_native: bool,
  asset_minimum: Balance,
  min_user_balance: Balance,
) -> Balance {
  if actor_type == types::ActorType::User && is_fee_native {
    min_user_balance
  } else {
    asset_minimum
  }
}

/// Settles one admitted fee-envelope step without touching host balances.
///
/// User reservation always releases the step's full upper bound before charging either the
/// evaluation-only or attempted-step amount. System Actors remains fee-exempt.
pub fn settle_attempt_fee_step<Balance>(
  actor_type: types::ActorType,
  reservation: Balance,
  step: &StepFeeEnvelope<Balance>,
  charge_kind: FeeChargeKind,
) -> Result<FeeStepSettlement<Balance>, FeeEnvelopeError>
where
  Balance: Copy + CheckedSub + Zero,
{
  if actor_type == types::ActorType::System {
    return Ok(FeeStepSettlement {
      charged: Balance::zero(),
      reservation_remaining: Balance::zero(),
    });
  }
  let reservation_remaining = reservation
    .checked_sub(&step.total)
    .ok_or(FeeEnvelopeError::ReservationUnderflow)?;
  let charged = match charge_kind {
    FeeChargeKind::EvaluationOnly => step.evaluation,
    FeeChargeKind::Attempted => step.total,
  };
  Ok(FeeStepSettlement {
    charged,
    reservation_remaining,
  })
}

pub(crate) const MAX_CONTRACT_STEPS_HARD_LIMIT: u32 = u8::MAX as u32;

pub(crate) const fn contract_steps_bound_is_valid(bound: u32) -> bool {
  bound > 0 && bound <= MAX_CONTRACT_STEPS_HARD_LIMIT
}

sp_api::decl_runtime_apis! {
  pub trait ActorSimulationApi<Contract, Simulation>
  where
    Contract: codec::Codec,
    Simulation: codec::Codec,
  {
    fn simulate_current_contract(
      actor_id: types::ActorId,
      expected_type: types::ActorType,
      expected_mutability: types::Mutability,
      expected_contract: Contract,
      mode: types::SimulationMode,
    ) -> Result<Simulation, types::SimulationError>;
  }

  /// Read-only named Actors cost projection with independent fee and hold provenance.
  pub trait ActorCostApi<Balance>
  where
    Balance: codec::Codec,
  {
    fn actor_cost_quote(
      actor_id: types::ActorId,
    ) -> Result<types::ActorCostQuote<Balance>, types::ActorCostQuoteError>;
  }

  /// Bounded current and finalized block-resource projection.
  pub trait ActorResourceApi<BlockNumber>
  where
    BlockNumber: codec::Codec,
  {
    fn block_resource_budget() -> types::BlockResourceBudget;

    fn current_block_resource_state() -> Option<types::BlockResourceState<BlockNumber>>;

    fn finalized_block_resource_snapshot(
    ) -> Option<types::FinalizedBlockResourceSnapshot<BlockNumber>>;
  }

  /// Read-only eligibility projection for one actor (spec 7.3).
  ///
  /// Returns absence/dormancy or the canonical Active classification, reusing
  /// the same pure owners as
  /// admission so clients do not reimplement cadence phase, cooldown, window
  /// floor, retry backoff, breaker, or latch arithmetic.
  #[api_version(6)]
  pub trait ActorEligibilityApi<FeedId, BlockNumber>
  where
    FeedId: codec::Codec,
    BlockNumber: codec::Codec,
  {
    fn actor_eligibility(
      actor_id: types::ActorId,
    ) -> Result<types::ActorEligibility<FeedId, BlockNumber>, types::ActorClassificationError>;

    fn materialization_faults() -> types::MaterializationFaults<FeedId, BlockNumber>;

    fn crossing_capacity(feed: FeedId) -> types::CrossingCapacity;

  }
}

#[frame::pallet]
pub mod pallet {
  use super::{
    ACTOR_PREPASS_INHERENT_VERSION, ActorPrepassContext, ActorPrepassInherentData,
    AdmissionCertificateAuthorityProvider, AssetOps, AttemptFeeEnvelope, DexOps, FeeCollector,
    FeeEnvelopeError, FeeEnvelopeInput, FundingAuthority, LiquidityOps, ObservationProvider,
    PipelineFeeBreakdown, StepControlWeightContext, StepControlWeightProvider, StepFeeBreakdown,
    TaskEffectWeightProvider, TriggerFeeBreakdown, WeightInfo, compose_attempt_fee_envelope,
    contract_steps_bound_is_valid,
  };
  use crate::adapters::{
    RetryClass, SovereignAccountDeriver as _, SovereignAccountPolicy, StakingOps as _,
    SystemActorContractValidator as _,
  };
  use alloc::{collections::BTreeSet, vec::Vec};
  use frame::prelude::*;
  use polkadot_sdk::{
    frame_support::{
      PalletId,
      traits::{
        EnsureOrigin, Time,
        fungible::{InspectHold, MutateHold},
        tokens::Precision,
      },
    },
    sp_inherents::{InherentData, InherentIdentifier, IsFatalError},
    sp_runtime::traits::{
      CheckedAdd, CheckedMul, CheckedSub, One, SaturatedConversion, Saturating, Zero,
    },
    sp_weights::{WeightMeter, WeightToFee as _},
  };

  pub const ACTOR_PREPASS_INHERENT_IDENTIFIER: InherentIdentifier = *b"deosact0";

  #[derive(codec::Encode, Debug)]
  pub enum ActorPrepassInherentError {
    MissingData,
    MissingCall,
    UnsupportedVersion,
  }

  impl IsFatalError for ActorPrepassInherentError {
    fn is_fatal_error(&self) -> bool {
      true
    }
  }

  use super::types::Task as ActorTask;
  pub use super::types::*;

  #[pallet::composite_enum]
  pub enum HoldReason {
    /// Refundable backing for retained User Actor process state.
    #[codec(index = 0)]
    ActorState,
  }

  #[pallet::config]
  pub trait Config: frame_system::Config {
    type AssetId: Parameter + Member + Copy + MaybeSerializeDeserialize + MaxEncodedLen + Ord;

    type Balance: Parameter
      + Member
      + AtLeast32BitUnsigned
      + Default
      + Copy
      + MaybeSerializeDeserialize
      + MaxEncodedLen;

    #[pallet::constant]
    type FeeNativeAssetId: Get<Self::AssetId>;

    type AssetOps: AssetOps<Self::AccountId, Self::AssetId, Self::Balance>;
    type AdmissionCertificateAuthority: AdmissionCertificateAuthorityProvider;
    type StepControlWeight: StepControlWeightProvider<StepOf<Self>>;
    type TaskEffectWeight: TaskEffectWeightProvider<TaskOf<Self>>;
    type ObservationFeedId: Parameter + Member + Copy + MaxEncodedLen + Ord;
    type ObservationProvider: ObservationProvider<Self::ObservationFeedId, BlockNumberFor<Self>>;
    type FundingAuthority: FundingAuthority<Self::AccountId>;
    type SovereignAccountDeriver: crate::adapters::SovereignAccountDeriver<Self::AccountId>;
    type SovereignAccountPolicy: crate::adapters::SovereignAccountPolicy<Self::AccountId>;
    type DexOps: DexOps<Self::AccountId, Self::AssetId, Self::Balance>;
    type StakingOps: crate::adapters::StakingOps<Self::AccountId, Self::AssetId, Self::Balance>;
    type LiquidityOps: LiquidityOps<Self::AccountId, Self::AssetId, Self::Balance>;
    type Time: Time<Moment = u64>;

    #[pallet::constant]
    type CadenceTickMillis: Get<u64>;
    #[pallet::constant]
    type MinWindowLength: Get<BlockNumberFor<Self>>;
    #[pallet::constant]
    type PalletId: Get<PalletId>;

    type SystemOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    type GlobalBreakerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

    #[pallet::constant]
    type MaxContractSteps: Get<u32>;
    #[pallet::constant]
    type MaxFundingTrackedAssets: Get<u32>;
    #[pallet::constant]
    type MaxOpeningSnapshotEntries: Get<u32>;
    #[pallet::constant]
    type MaxOpeningPredicateResults: Get<u32>;
    #[pallet::constant]
    type MaxPreconditionClauses: Get<u32>;
    #[pallet::constant]
    type MaxPredicatesPerClause: Get<u32>;
    #[pallet::constant]
    type MaxPredicatesPerStep: Get<u32>;
    #[pallet::constant]
    type MaxOwnerSlots: Get<u8>;
    #[pallet::constant]
    type MaxExecutionsPerBlock: Get<u32>;
    #[pallet::constant]
    type MaxQueueLength: Get<u32>;
    /// Physical I/O granularity for the monotonic active FIFO.
    #[pallet::constant]
    type QueuePageSize: Get<u32>;
    /// Physical I/O granularity for the paged temporal wakeup index.
    #[pallet::constant]
    type WakeupPageSize: Get<u32>;
    /// Physical I/O granularity for observation subscriber pages.
    #[pallet::constant]
    type ObservationPageSize: Get<u32>;
    /// Physical I/O granularity for ObservationCrossing membership pages.
    #[pallet::constant]
    type CrossingPageSize: Get<u32>;
    #[pallet::constant]
    type MaxCrossingMembersPerFeed: Get<u32>;
    #[pallet::constant]
    type MaxUserCrossingMembersPerFeed: Get<u32>;
    #[pallet::constant]
    type MaxCrossingTransitionsPerFeed: Get<u32>;
    #[pallet::constant]
    type MaxCrossingTransitionsPerBlock: Get<u32>;
    #[pallet::constant]
    type MaxCrossingLeavesPerBlock: Get<u32>;
    #[pallet::constant]
    type MaxCrossingPagesPerBlock: Get<u32>;
    #[pallet::constant]
    type MaxCrossingActorsPerBlock: Get<u32>;
    #[pallet::constant]
    type CrossingWorkerWeightLimit: Get<Weight>;
    /// Independent ceiling for physical queue-entry inspection per scheduler pass.
    #[pallet::constant]
    type MaxQueueEntriesScannedPerBlock: Get<u32>;
    #[pallet::constant]
    type MaxObservationFanoutPagesPerBlock: Get<u32>;
    #[pallet::constant]
    type ObservationFanoutWeightLimit: Get<Weight>;
    /// Hard two-dimensional ceiling for the overdue wakeup worker. The worker also remains
    /// bounded by the actual on_idle budget left after fixed base and saturated queue cleanup,
    /// then leaves the remainder for actor service.
    #[pallet::constant]
    type WakeupWeightLimit: Get<Weight>;
    #[pallet::constant]
    type MaxWakeupsPerBlock: Get<u32>;
    #[pallet::constant]
    type MaxSweepBatch: Get<u32>;
    #[pallet::constant]
    type MaxWhitelistSize: Get<u32>;
    #[pallet::constant]
    type MaxSplitTransferLegs: Get<u32>;
    /// Target block duration in whole seconds.
    #[pallet::constant]
    type TargetBlockTime: Get<u64>;
    #[pallet::constant]
    type MaxExecutionDelayBlocks: Get<BlockNumberFor<Self>>;
    #[pallet::constant]
    type MaxTemporalDelayTicks: Get<SchedulerTick>;
    #[pallet::constant]
    type MaxIdleStarvationBlocks: Get<u32>;
    /// Gross two-dimensional `on_idle` weight guaranteed by the embedding runtime.
    #[pallet::constant]
    type ActorOnIdleReserve: Get<Weight>;
    #[pallet::constant]
    type MaxAutoCloseNonceHorizon: Get<u64>;
    /// Maximum number of active Actors instances. Bounds the BTreeSet storage.
    /// Set to 10,000 for production use cases.
    #[pallet::constant]
    type MaxActiveActors: Get<u32>;
    /// Hard cap across active and dormant actor identities.
    #[pallet::constant]
    type MaxActorIdentities: Get<u32>;
    /// Lifetime cap on allocated System custody locators, including vacant locators.
    #[pallet::constant]
    type MaxSystemSovereigns: Get<u32>;

    #[pallet::constant]
    type ActorCreationFee: Get<Self::Balance>;
    type RuntimeHoldReason: Parameter + Member + MaxEncodedLen + Copy + From<HoldReason>;
    type StateHoldCurrency: InspectHold<Self::AccountId, Balance = Self::Balance, Reason = Self::RuntimeHoldReason>
      + MutateHold<Self::AccountId>;
    /// Fixed accounting price for each present retained-state component.
    #[pallet::constant]
    type ActorStateHoldBase: Get<Self::Balance>;
    /// Linear accounting price for each SCALE-encoded retained byte.
    #[pallet::constant]
    type ActorStateHoldPerByte: Get<Self::Balance>;
    /// Converts weight to fee for execution cost calculation
    type WeightToFee: polkadot_sdk::sp_weights::WeightToFee<Balance = Self::Balance>;
    /// Runtime-bound upper weights for every Actors task variant
    type FeeSink: Get<Self::AccountId>;
    type FeeCollector: FeeCollector<Self::AccountId, Self::AssetId, Self::Balance>;
    #[pallet::constant]
    type MaxConsecutiveFailures: Get<u32>;
    #[pallet::constant]
    type MaxRetryAttempts: Get<u32>;
    #[pallet::constant]
    type MinUserBalance: Get<Self::Balance>;

    type WeightInfo: WeightInfo;

    /// Runtime-owned immutable block-resource budget derived from the fixed envelope.
    type BlockResourceBudget: Get<BlockResourceBudget>;

    type PrepassContext: ActorPrepassContext;

    /// Provides System Actors specs to initialize at genesis.
    /// Use `()` for no genesis System Actors (default).
    type GenesisSystemActors: GenesisSystemActors<Self::AccountId, ActorContractOf<Self>>;
    /// Host policy for bounded System Actor effect topology. User Actor
    /// contracts intentionally remain outside this reference-runtime DAG.
    type SystemActorContractValidator: crate::SystemActorContractValidator<ActorContractOf<Self>>;

    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper: crate::BenchmarkHelper<Self::AccountId, Self::AssetId, Self::Balance, Self::ObservationFeedId>;
  }

  pub type BalanceOf<T> = <T as Config>::Balance;
  pub type AssetIdOf<T> = <T as Config>::AssetId;

  pub type SourceFilterOf<T> =
    SourceFilter<<T as frame_system::Config>::AccountId, <T as Config>::MaxWhitelistSize>;

  pub type AssetFilterOf<T> = AssetFilter<<T as Config>::AssetId, <T as Config>::MaxWhitelistSize>;

  pub type ActorObservationFeedsOf<T> = BoundedVec<<T as Config>::ObservationFeedId, ConstU32<1>>;
  pub type SimulationResultOf<T> = SimulationResult<<T as Config>::MaxContractSteps>;
  pub type ObservationSubscriberPageOf<T> =
    ObservationSubscriberPage<<T as Config>::ObservationPageSize>;
  pub type ObservationFreeSlotPageOf<T> = BoundedVec<u32, <T as Config>::ObservationPageSize>;
  pub type CrossingMemberPageOf<T> = CrossingMemberPage<<T as Config>::CrossingPageSize>;
  pub type CrossingLeafKeyOf<T> = CrossingLeafKey<<T as Config>::ObservationFeedId>;
  pub type CrossingRadixNodeKeyOf<T> = CrossingRadixNodeKey<<T as Config>::ObservationFeedId>;
  pub type CrossingMembershipLocatorOf<T> =
    CrossingMembershipLocator<<T as Config>::ObservationFeedId>;
  pub type CrossingTransitionQueueOf<T> =
    BoundedVec<CrossingTransitionObligation, <T as Config>::MaxCrossingTransitionsPerFeed>;

  #[derive(Clone, Copy)]
  pub(crate) enum TriggerTransitionIntent {
    GenesisInstallation,
    CreateActive,
    ActivateDormant,
    ReplaceActive,
    Deactivate,
    Close,
  }

  pub(crate) struct TriggerTransitionPlan<T: Config> {
    intent: TriggerTransitionIntent,
    crossing: crate::crossing::CrossingMembershipTransition<T::ObservationFeedId>,
    observation_feeds: ActorObservationFeedsOf<T>,
  }

  pub type TriggerOf<T> = Trigger<
    <T as frame_system::Config>::AccountId,
    <T as Config>::AssetId,
    <T as Config>::MaxWhitelistSize,
    <T as Config>::ObservationFeedId,
  >;

  pub type PreconditionOf<T> = Precondition<
    Predicate<
      <T as Config>::AssetId,
      <T as Config>::Balance,
      u32,
      <T as Config>::ObservationFeedId,
    >,
    <T as Config>::MaxPreconditionClauses,
    <T as Config>::MaxPredicatesPerClause,
  >;

  pub type TaskOf<T> = super::types::Task<
    <T as Config>::AssetId,
    <T as Config>::Balance,
    <T as frame_system::Config>::AccountId,
    <T as Config>::MaxSplitTransferLegs,
  >;

  pub type SplitTransferLegsOf<T> = BoundedVec<
    SplitLeg<<T as frame_system::Config>::AccountId>,
    <T as Config>::MaxSplitTransferLegs,
  >;

  pub type StepOf<T> = Step<
    <T as Config>::AssetId,
    <T as Config>::Balance,
    <T as frame_system::Config>::AccountId,
    <T as Config>::MaxPreconditionClauses,
    <T as Config>::MaxPredicatesPerClause,
    <T as Config>::MaxSplitTransferLegs,
    <T as Config>::ObservationFeedId,
  >;

  pub type ContractSteps<T> = BoundedVec<StepOf<T>, <T as Config>::MaxContractSteps>;

  pub type AttemptFeeEnvelopeOf<T> =
    AttemptFeeEnvelope<BalanceOf<T>, <T as Config>::MaxContractSteps>;

  pub type FundingSourcePolicyOf<T> =
    FundingSourcePolicy<<T as frame_system::Config>::AccountId, <T as Config>::MaxWhitelistSize>;

  pub type ActorContractOf<T> = super::types::ActorContract<
    TriggerOf<T>,
    BlockNumberFor<T>,
    ContractSteps<T>,
    FundingSourcePolicyOf<T>,
  >;

  pub type ActorContractHeaderOf<T> = super::types::ActorContractHeader<
    TriggerOf<T>,
    BlockNumberFor<T>,
    FundingSourcePolicyOf<T>,
    <T as Config>::Balance,
    [u8; 32],
  >;

  pub type ActorContractHeadOf<T> =
    super::types::ActorContractHead<ActorContractHeaderOf<T>, StepOf<T>>;

  pub type ActorActivationAuthorityOf<T> = super::types::ActorActivationAuthority<
    <T as Config>::ObservationFeedId,
    BlockNumberFor<T>,
    [u8; 32],
  >;

  pub type ActorStepChunkOf<T> = super::types::ActorStepChunk<
    ActorId,
    [u8; 32],
    BoundedVec<StepOf<T>, ConstU32<{ super::types::MAX_STEPS_PER_TAIL_CHUNK }>>,
    BoundedVec<ActorStepResourceEnvelope, ConstU32<{ super::types::MAX_STEPS_PER_TAIL_CHUNK }>>,
  >;

  pub type ActorAdmissionResourcesOf<T> =
    BoundedVec<ActorStepResourceEnvelope, <T as Config>::MaxContractSteps>;

  pub type ActorAdmissionCertificateOf<T> = ActorAdmissionCertificate<ActorAdmissionResourcesOf<T>>;

  pub type ActorStepTicketOf<T> =
    ActorStepTicket<BlockNumberFor<T>, ActorContractCommitment<[u8; 32]>>;

  pub type LoadedActorStepOf<T> = LoadedActorStep<StepOf<T>>;

  pub type CurrentStepPlanOf<T> = StepExecutionPlan<
    ActorIdentityOf<T>,
    ActorHotStateOf<T>,
    ActorRunStateOf<T>,
    ActorFundingStateOf<T>,
    ActorAdmissionCertificateOf<T>,
    ActorStepTicketOf<T>,
    LoadedActorStepOf<T>,
    StepFeeBreakdown<<T as Config>::Balance>,
  >;

  pub type FundingAccumulatedOf<T> = BoundedBTreeMap<
    <T as Config>::AssetId,
    <T as Config>::Balance,
    <T as Config>::MaxFundingTrackedAssets,
  >;

  pub type FundingTrackedAssetsOf<T> =
    BoundedBTreeSet<<T as Config>::AssetId, <T as Config>::MaxFundingTrackedAssets>;

  pub type FundingSnapshotOf<T> = FundingAccumulatedOf<T>;

  pub type RunOpeningSnapshotOf<T> = BoundedBTreeMap<
    OpeningSurface<<T as Config>::AssetId>,
    <T as Config>::Balance,
    <T as Config>::MaxOpeningSnapshotEntries,
  >;

  pub type OpeningPredicateResultsOf<T> =
    BoundedVec<Result<bool, PredicateError>, <T as Config>::MaxOpeningPredicateResults>;

  pub type ActorRunHeadOf<T> = ActorRunHead<BlockNumberFor<T>>;

  pub type ActorRunPayloadOf<T> = ActorRunPayload<
    <T as Config>::AssetId,
    <T as Config>::Balance,
    <T as Config>::MaxOpeningSnapshotEntries,
    <T as Config>::MaxFundingTrackedAssets,
    <T as Config>::MaxOpeningPredicateResults,
  >;

  pub type ActorRunStateOf<T> = ActorRunState<
    <T as Config>::AssetId,
    <T as Config>::Balance,
    BlockNumberFor<T>,
    <T as Config>::MaxOpeningSnapshotEntries,
    <T as Config>::MaxFundingTrackedAssets,
    <T as Config>::MaxOpeningPredicateResults,
  >;

  pub type QueuePageOf<T> = BoundedVec<QueueEntry<BlockNumberFor<T>>, <T as Config>::QueuePageSize>;
  pub type WakeupPageEntriesOf<T> = BoundedVec<Option<WakeupEntry>, <T as Config>::WakeupPageSize>;
  pub type WakeupPageOf<T> = WakeupPage<WakeupPageEntriesOf<T>>;
  pub type WakeupCursorPageOf<T> =
    BoundedVec<WakeupKey<BlockNumberFor<T>>, <T as Config>::WakeupPageSize>;

  pub(crate) type ActiveActorViewOf<T> = ActiveActorView<
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T>,
    TriggerOf<T>,
    ContractSteps<T>,
  >;

  pub type ActorHotStateOf<T> = ActorHotState<BlockNumberFor<T>>;

  pub type ActorFundingStateOf<T> =
    ActorFundingState<FundingAccumulatedOf<T>, FundingTrackedAssetsOf<T>>;

  pub type ActorIdentityOf<T> =
    ActorIdentity<<T as frame_system::Config>::AccountId, BlockNumberFor<T>>;

  pub type ActorStateHoldBreakdownOf<T> = ActorStateHoldBreakdown<<T as Config>::Balance>;
  pub type ActorStateHoldRecordOf<T> =
    ActorStateHoldRecord<<T as frame_system::Config>::AccountId, <T as Config>::Balance>;

  pub type ActiveActorStateOf<T> = ActiveActorState<
    ActorIdentityOf<T>,
    ActorHotStateOf<T>,
    ActorContractOf<T>,
    ActorFundingStateOf<T>,
    ActorRunStateOf<T>,
  >;

  pub(crate) enum LoadedActorStateOf<T: Config> {
    NotRegistered,
    Dormant(ActorIdentityOf<T>),
    Active(ActiveActorStateOf<T>),
    Corrupt,
  }

  pub struct ObservationActivationState<T: Config> {
    pub actor_id: ActorId,
    pub identity: ActorIdentityOf<T>,
    pub hot: ActorHotStateOf<T>,
    pub authority: ActorActivationAuthorityOf<T>,
    pub run_head: Option<ActorRunHeadOf<T>>,
    pub loaded_step: Option<LoadedActorStepOf<T>>,
  }

  #[pallet::pallet]
  #[pallet::storage_version(STORAGE_VERSION)]
  pub struct Pallet<T>(_);

  const STORAGE_VERSION: StorageVersion = StorageVersion::new(15);

  #[pallet::storage]
  #[pallet::getter(fn next_actor_id)]
  pub type NextActorId<T> = StorageValue<_, ActorId, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn actor_hot)]
  pub type ActorHot<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorHotStateOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::storage_prefix = "ActorContractHead"]
  pub type ActorContractHeads<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorContractHeadOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::storage_prefix = "ActorActivationAuthority"]
  pub type ActorActivationAuthorities<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorActivationAuthorityOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::storage_prefix = "ActorAdmissionCertificate"]
  pub type ActorAdmissionCertificates<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorAdmissionCertificateOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::storage_prefix = "ActorContractTailChunk"]
  pub type ActorContractTailChunks<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    ActorId,
    Blake2_128Concat,
    u32,
    ActorStepChunkOf<T>,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn actor_funding)]
  pub type ActorFunding<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorFundingStateOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::storage_prefix = "ActorRunHead"]
  pub type ActorRunHeads<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorRunHeadOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::storage_prefix = "ActorRunPayload"]
  pub type ActorRunPayloads<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorRunPayloadOf<T>, OptionQuery>;

  pub struct ActorRunStateStore<T: Config>(core::marker::PhantomData<T>);

  impl<T: Config> ActorRunStateStore<T> {
    pub fn get(actor_id: ActorId) -> Option<ActorRunStateOf<T>> {
      ActorRunState::from_tiers(
        ActorRunHeads::<T>::get(actor_id)?,
        ActorRunPayloads::<T>::get(actor_id)?,
      )
    }

    pub fn insert(actor_id: ActorId, state: ActorRunStateOf<T>) {
      let (head, payload) = state.into_tiers();
      let payload_changed = ActorRunHeads::<T>::get(actor_id)
        .is_none_or(|current| current.payload_commitment != head.payload_commitment);
      ActorRunHeads::<T>::insert(actor_id, head);
      if payload_changed {
        ActorRunPayloads::<T>::insert(actor_id, payload);
      }
    }

    pub fn remove(actor_id: ActorId) {
      ActorRunHeads::<T>::remove(actor_id);
      ActorRunPayloads::<T>::remove(actor_id);
    }

    pub fn take(actor_id: ActorId) -> Option<ActorRunStateOf<T>> {
      let state = Self::get(actor_id)?;
      Self::remove(actor_id);
      Some(state)
    }

    pub fn contains_key(actor_id: ActorId) -> bool {
      ActorRunHeads::<T>::contains_key(actor_id) && ActorRunPayloads::<T>::contains_key(actor_id)
    }

    pub fn iter_keys() -> impl Iterator<Item = ActorId> {
      ActorRunHeads::<T>::iter_keys()
    }

    pub fn mutate<R>(
      actor_id: ActorId,
      mutate: impl FnOnce(&mut Option<ActorRunStateOf<T>>) -> R,
    ) -> R {
      let mut state = Self::get(actor_id);
      let result = mutate(&mut state);
      if let Some(state) = state {
        Self::insert(actor_id, state);
      } else {
        Self::remove(actor_id);
      }
      result
    }
  }

  impl<T: Config> Pallet<T> {
    pub fn actor_run_state(actor_id: ActorId) -> Option<ActorRunStateOf<T>> {
      ActorRunStateStore::<T>::get(actor_id)
    }
    pub fn actor_contract(actor_id: ActorId) -> Option<ActorContractOf<T>> {
      Self::load_actor_contract(actor_id)
    }

    pub fn actor_cost_quote(
      actor_id: ActorId,
    ) -> Result<ActorCostQuote<T::Balance>, ActorCostQuoteError> {
      let (state, admission) = Self::load_actor_state_with_admission(actor_id);
      let (identity, active) = match state {
        LoadedActorStateOf::NotRegistered => return Err(ActorCostQuoteError::ActorNotFound),
        LoadedActorStateOf::Dormant(identity) => (identity, None),
        LoadedActorStateOf::Active(state) => (state.identity.clone(), Some(state)),
        LoadedActorStateOf::Corrupt => return Err(ActorCostQuoteError::ActorInvariant),
      };
      let actor_type = identity.actor_class.actor_type();
      let creation_fee = if actor_type == ActorType::System {
        T::Balance::zero()
      } else {
        T::ActorCreationFee::get()
      };
      let effect_identity = T::TaskEffectWeight::production_weight_identity()
        .ok_or(ActorCostQuoteError::WeightAuthorityUnavailable)?;
      let zero_action = ActorActionFeeQuote {
        maximum_effect_weight: Weight::zero(),
        maximum_effect_fee: T::Balance::zero(),
        production_weight_identity: effect_identity,
      };
      let (prospective_trigger_fee, prospective_pipeline_fee, maximum_next_action_fee) =
        match active {
          None => (None, None, zero_action),
          Some(state) => {
            let admission = admission.ok_or(ActorCostQuoteError::ActorInvariant)?;
            let trigger_family = state.contract.trigger.family();
            let maximum_weight = Self::trigger_occurrence_weight(trigger_family);
            let trigger_fee =
              Self::trigger_fee_for_weight(actor_type, trigger_family, maximum_weight);
            let machine_envelope = ActorContractHeads::<T>::get(actor_id)
              .ok_or(ActorCostQuoteError::ActorInvariant)?
              .header
              .pipeline_machine_envelope;
            let pipeline_fee = Self::pipeline_fee_breakdown(actor_type, machine_envelope)
              .map_err(|_| ActorCostQuoteError::ComputationOverflow)?;
            let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
            let action = if state.contract.steps.is_empty() {
              zero_action
            } else {
              let loaded = Self::load_current_step_from_storage(actor_id, cursor)
                .ok_or(ActorCostQuoteError::ActorInvariant)?;
              let maximum =
                Self::maximum_current_action_fee(actor_type, &loaded.step, loaded.resources)
                  .map_err(|_| ActorCostQuoteError::ComputationOverflow)?;
              ActorActionFeeQuote {
                maximum_effect_weight: if matches!(loaded.step.task, ActorTask::StopCycle) {
                  Weight::zero()
                } else {
                  loaded.resources.effect
                },
                maximum_effect_fee: maximum.effect_fee,
                production_weight_identity: effect_identity,
              }
            };
            (
              Some(ActorTriggerFeeQuote {
                trigger_family,
                maximum_weight,
                fee: trigger_fee.trigger_fee,
                production_weight_identity: Self::trigger_weight_identity(),
              }),
              Some(ActorPipelineFeeQuote {
                pipeline_machine_fee: pipeline_fee.pipeline_machine_fee,
                cleanup_fee: pipeline_fee.cleanup_fee,
                total_fee: pipeline_fee.total_fee,
                strategy: PipelineMachineFeeStrategy::UpfrontBounded,
                admission_identity: admission.admission_identity,
                production_weight_identity: admission.production_weight_identity,
              }),
              action,
            )
          }
        };
      let actor_state_hold = Self::actor_state_hold_quote(actor_id, actor_type)?;
      Ok(ActorCostQuote {
        actor_type,
        creation_fee,
        prospective_trigger_fee,
        prospective_pipeline_fee,
        maximum_next_action_fee,
        actor_state_hold,
      })
    }

    pub(crate) fn load_actor_contract(actor_id: ActorId) -> Option<ActorContractOf<T>> {
      Self::load_admitted_contract_geometry(actor_id).map(|(contract, _)| contract)
    }

    pub(crate) fn store_actor_contract(
      actor_id: ActorId,
      contract: ActorContractOf<T>,
    ) -> DispatchResult {
      let certificate =
        Self::build_admission_certificate(&contract).ok_or(Error::<T>::AdmissionBoundOverflow)?;
      let stored = if ActorContractHeads::<T>::contains_key(actor_id) {
        Self::replace_admitted_contract_geometry(actor_id, &contract, &certificate)
      } else {
        Self::insert_admitted_contract_geometry(actor_id, &contract, &certificate)
      };
      ensure!(stored, Error::<T>::ActorInvariant);
      if CrossingMemberships::<T>::contains_key(actor_id)
        && let Some(crossing) = Self::crossing_from_trigger(&contract.trigger)
      {
        let phase = match ActorHot::<T>::get(actor_id).map(|hot| hot.trigger_runtime_state) {
          Some(TriggerRuntimeState::ObservationCrossing { phase, .. }) => phase,
          _ => return Err(Error::<T>::ActorInvariant.into()),
        };
        Self::sync_crossing_compiled_authority(
          actor_id,
          crossing,
          phase,
          certificate.admission_identity,
        )?;
      }
      Self::sync_activation_authority(actor_id, &contract, &certificate);
      Ok(())
    }

    fn sync_activation_authority(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) {
      let feed = match &contract.trigger {
        Trigger::ObservationChange { feed } => Some(*feed),
        Trigger::ObservationCrossing { feed, .. } => Some(*feed),
        _ => None,
      };
      if let Some(feed) = feed {
        ActorActivationAuthorities::<T>::insert(
          actor_id,
          ActorActivationAuthority {
            feed,
            cooldown_blocks: contract.cooldown_blocks,
            window: contract.window,
            auto_close_at_cycle_nonce: contract.auto_close_at_cycle_nonce,
            semantic_contract_id: certificate.semantic_contract_id,
            body_commitment: certificate.body_commitment,
            admission_identity: certificate.admission_identity,
          },
        );
      } else {
        ActorActivationAuthorities::<T>::remove(actor_id);
      }
    }

    pub(crate) fn remove_actor_contract(actor_id: ActorId) -> DispatchResult {
      ensure!(
        Self::remove_admitted_contract_geometry(actor_id).is_some(),
        Error::<T>::ActorInvariant
      );
      Ok(())
    }

    pub(crate) fn insert_admitted_contract_geometry(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> bool {
      if ActorContractHeads::<T>::contains_key(actor_id)
        || ActorAdmissionCertificates::<T>::contains_key(actor_id)
      {
        return false;
      }
      // An orphan Contract partition has no execution authority. System's zero-fee projection
      // keeps corruption-mask tests and diagnostic geometry independent of identity state.
      let actor_type = ActorIdentities::<T>::get(actor_id)
        .map(|identity| identity.actor_class.actor_type())
        .unwrap_or(ActorType::System);
      let Some((head, chunks)) =
        Self::decompose_admitted_contract_geometry(actor_id, actor_type, contract, certificate)
      else {
        return false;
      };
      if chunks
        .iter(/* deos-bypass: bounded-iter */)
        .any(|(chunk_index, _)| {
          ActorContractTailChunks::<T>::contains_key(actor_id, chunk_index)
        })
      {
        return false;
      }
      ActorAdmissionCertificates::<T>::insert(actor_id, certificate);
      ActorContractHeads::<T>::insert(actor_id, head);
      for (chunk_index, chunk) in chunks {
        ActorContractTailChunks::<T>::insert(actor_id, chunk_index, chunk);
      }
      true
    }

    #[allow(
      dead_code,
      reason = "I2 bounded reconstruction is staged behind the centralized Contract owner"
    )]
    pub(crate) fn load_admitted_contract_geometry(
      actor_id: ActorId,
    ) -> Option<(ActorContractOf<T>, ActorAdmissionCertificateOf<T>)> {
      let head = ActorContractHeads::<T>::get(actor_id)?;
      let certificate = ActorAdmissionCertificates::<T>::get(actor_id)?;
      if !certificate.has_valid_identity()
        || certificate.semantic_contract_id != head.header.semantic_contract_id
        || certificate.body_commitment != head.header.body_commitment
        || certificate.admission_identity != head.header.admission_identity
      {
        return None;
      }
      let chunk_count = head
        .header
        .step_count
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK);
      let chunks = (0..chunk_count)
        .map(|chunk_index| {
          Some((
            chunk_index,
            ActorContractTailChunks::<T>::get(actor_id, chunk_index)?,
          ))
        })
        .collect::<Option<Vec<_>>>()?;
      let mut resource_count = usize::from(head.header.step_count > 0);
      for (_, chunk) in &chunks {
        if chunk.steps.len() != chunk.step_resources.len() {
          return None;
        }
        resource_count = resource_count.checked_add(chunk.step_resources.len())?;
      }
      if resource_count != head.header.step_count as usize {
        return None;
      }
      let contract = Self::reconstruct_contract_geometry(actor_id, head, &chunks)?;
      Some((contract, certificate))
    }

    #[allow(
      dead_code,
      reason = "I2 lazy production loading is staged before current-Step scheduler wiring"
    )]
    pub(crate) fn load_current_step_from_storage(
      actor_id: ActorId,
      cursor: u32,
    ) -> Option<LoadedActorStepOf<T>> {
      let head = ActorContractHeads::<T>::get(actor_id)?;
      let certificate = ActorAdmissionCertificates::<T>::get(actor_id)?;
      let tail_chunk = if cursor == 0 {
        None
      } else {
        let chunk_index = cursor.checked_sub(1)? / MAX_STEPS_PER_TAIL_CHUNK;
        Some((
          chunk_index,
          ActorContractTailChunks::<T>::get(actor_id, chunk_index)?,
        ))
      };
      Self::load_current_step_from_geometry(
        actor_id,
        &head,
        &certificate,
        cursor,
        tail_chunk.as_ref().map(|(index, chunk)| (*index, chunk)),
      )
    }

    pub(crate) fn replace_admitted_contract_geometry(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> bool {
      let Some((current_contract, _)) = Self::load_admitted_contract_geometry(actor_id) else {
        return false;
      };
      let actor_type = ActorIdentities::<T>::get(actor_id)
        .map(|identity| identity.actor_class.actor_type())
        .unwrap_or(ActorType::System);
      let Some((head, chunks)) =
        Self::decompose_admitted_contract_geometry(actor_id, actor_type, contract, certificate)
      else {
        return false;
      };
      let Ok(old_step_count) = u32::try_from(current_contract.steps.len()) else {
        return false;
      };
      let old_chunk_count = old_step_count
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK);
      let Ok(new_chunk_count) = u32::try_from(chunks.len()) else {
        return false;
      };
      ActorAdmissionCertificates::<T>::insert(actor_id, certificate);
      ActorContractHeads::<T>::insert(actor_id, head);
      for (chunk_index, chunk) in chunks {
        ActorContractTailChunks::<T>::insert(actor_id, chunk_index, chunk);
      }
      for chunk_index in new_chunk_count..old_chunk_count {
        ActorContractTailChunks::<T>::remove(actor_id, chunk_index);
      }
      true
    }

    pub(crate) fn remove_admitted_contract_geometry(
      actor_id: ActorId,
    ) -> Option<ActorContractOf<T>> {
      let (contract, _) = Self::load_admitted_contract_geometry(actor_id)?;
      let chunk_count = u32::try_from(contract.steps.len())
        .ok()?
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK);
      ActorActivationAuthorities::<T>::remove(actor_id);
      ActorAdmissionCertificates::<T>::remove(actor_id);
      ActorContractHeads::<T>::remove(actor_id);
      for chunk_index in 0..chunk_count {
        ActorContractTailChunks::<T>::remove(actor_id, chunk_index);
      }
      Some(contract)
    }

    #[allow(
      dead_code,
      reason = "I4 dual-meter admission is staged before scheduler plan wiring"
    )]
    pub(crate) fn current_step_resources_fit(
      control_meter: &WeightMeter,
      effect_meter: &WeightMeter,
      resources: ActorStepResourceEnvelope,
    ) -> bool {
      control_meter.can_consume(resources.control) && effect_meter.can_consume(resources.effect)
    }

    fn trigger_occurrence_weight(trigger_family: TriggerFamily) -> Weight {
      match trigger_family {
        TriggerFamily::Manual => T::WeightInfo::manual_trigger(),
        TriggerFamily::AddressEvent => T::WeightInfo::address_event_trigger_occurrence(),
        TriggerFamily::ObservationChange => T::WeightInfo::observation_change_trigger_occurrence(),
        TriggerFamily::ObservationCrossing => {
          T::WeightInfo::observation_crossing_trigger_occurrence()
        }
        TriggerFamily::AtTime => T::WeightInfo::at_time_trigger_occurrence(),
        TriggerFamily::Cadenced => T::WeightInfo::cadenced_trigger_occurrence(),
      }
    }

    fn trigger_weight_identity() -> [u8; 32] {
      (
        b"DEOS_ACTOR_TRIGGER_WEIGHT_V1",
        Self::trigger_occurrence_weight(TriggerFamily::Manual),
        Self::trigger_occurrence_weight(TriggerFamily::AddressEvent),
        Self::trigger_occurrence_weight(TriggerFamily::ObservationChange),
        Self::trigger_occurrence_weight(TriggerFamily::ObservationCrossing),
        Self::trigger_occurrence_weight(TriggerFamily::AtTime),
        Self::trigger_occurrence_weight(TriggerFamily::Cadenced),
      )
        .using_encoded(frame::hashing::blake2_256)
    }

    pub(crate) fn trigger_fee_for_weight(
      actor_type: ActorType,
      trigger_family: TriggerFamily,
      weight: Weight,
    ) -> TriggerFeeBreakdown<T::Balance> {
      let trigger_fee = if actor_type == ActorType::System || weight == Weight::zero() {
        Zero::zero()
      } else {
        T::WeightToFee::weight_to_fee(&weight)
      };
      TriggerFeeBreakdown {
        trigger_family,
        trigger_fee,
      }
    }

    pub(crate) fn pipeline_fee_breakdown(
      actor_type: ActorType,
      envelope: PipelineMachineEnvelope<T::Balance>,
    ) -> Result<PipelineFeeBreakdown<T::Balance>, Error<T>> {
      if actor_type == ActorType::System {
        return Ok(PipelineFeeBreakdown {
          pipeline_machine_fee: Zero::zero(),
          cleanup_fee: Zero::zero(),
          total_fee: Zero::zero(),
        });
      }
      let total_fee = envelope
        .pipeline_machine_fee_upper
        .checked_add(&envelope.cleanup_fee_upper)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      Ok(PipelineFeeBreakdown {
        pipeline_machine_fee: envelope.pipeline_machine_fee_upper,
        cleanup_fee: envelope.cleanup_fee_upper,
        total_fee,
      })
    }

    pub(crate) fn step_fee_for_resources(
      actor_type: ActorType,
      resources: ActorStepResourceEnvelope,
    ) -> Result<StepFeeBreakdown<T::Balance>, Error<T>> {
      if actor_type == ActorType::System {
        return Ok(StepFeeBreakdown {
          control_fee: Zero::zero(),
          effect_fee: Zero::zero(),
          total_fee: Zero::zero(),
        });
      }
      let control_fee: T::Balance = Zero::zero();
      let effect_fee = if resources.effect == Weight::zero() {
        Zero::zero()
      } else {
        T::WeightToFee::weight_to_fee(&resources.effect)
      };
      let total_fee = control_fee
        .checked_add(&effect_fee)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      Ok(StepFeeBreakdown {
        control_fee,
        effect_fee,
        total_fee,
      })
    }

    pub(crate) fn maximum_current_action_fee(
      actor_type: ActorType,
      step: &StepOf<T>,
      resources: ActorStepResourceEnvelope,
    ) -> Result<StepFeeBreakdown<T::Balance>, Error<T>> {
      if matches!(step.task, super::types::Task::StopCycle) {
        return Ok(StepFeeBreakdown {
          control_fee: Zero::zero(),
          effect_fee: Zero::zero(),
          total_fee: Zero::zero(),
        });
      }
      Self::step_fee_for_resources(actor_type, resources)
    }

    pub(crate) fn maximum_current_step_fee(
      actor_type: ActorType,
      resources: ActorStepResourceEnvelope,
    ) -> Result<StepFeeBreakdown<T::Balance>, Error<T>> {
      Self::step_fee_for_resources(actor_type, resources)
    }

    pub(crate) fn derive_pipeline_machine_envelope(
      actor_type: ActorType,
      contract_steps: &ContractSteps<T>,
      resources: &ActorAdmissionResourcesOf<T>,
    ) -> Result<PipelineMachineEnvelope<T::Balance>, Error<T>> {
      ensure!(
        contract_steps.len() == resources.len(),
        Error::<T>::ActorRunInvariant
      );
      if actor_type == ActorType::System {
        return Ok(PipelineMachineEnvelope {
          pipeline_machine_fee_upper: Zero::zero(),
          cleanup_fee_upper: Zero::zero(),
        });
      }
      let mut pipeline_machine_fee_upper: T::Balance = if contract_steps.is_empty() {
        T::WeightToFee::weight_to_fee(&T::WeightInfo::scheduler_inner_zero_step_complete())
      } else {
        Zero::zero()
      };
      for (step, resource) in contract_steps
        .iter(/* deos-bypass: bounded-iter — admitted Contract Steps bound the complete visit. */)
        .zip(
          resources
            .iter(/* deos-bypass: bounded-iter — admitted resource count equals Step count. */),
        )
      {
        let machine_weight = if matches!(step.task, super::types::Task::StopCycle) {
          resource.control.saturating_add(resource.effect)
        } else {
          resource.control
        };
        let machine_attempt_fee = if machine_weight == Weight::zero() {
          Zero::zero()
        } else {
          T::WeightToFee::weight_to_fee(&machine_weight)
        };
        let attempts = step.on_error.retry_max_attempts().unwrap_or(1);
        for _ in 0..attempts {
          pipeline_machine_fee_upper = pipeline_machine_fee_upper
            .checked_add(&machine_attempt_fee)
            .ok_or(Error::<T>::AdmissionBoundOverflow)?;
        }
      }
      let cleanup_fee_upper = T::WeightToFee::weight_to_fee(&T::WeightInfo::close_actor());
      pipeline_machine_fee_upper
        .checked_add(&cleanup_fee_upper)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      Ok(PipelineMachineEnvelope {
        pipeline_machine_fee_upper,
        cleanup_fee_upper,
      })
    }

    pub(crate) fn step_control_weight_context(
      step_count: u32,
      cursor: u32,
      predicate_evaluation_units: u32,
      opening_snapshot_entries: u32,
      opening_predicate_results: u32,
      funding_snapshot_entries: u32,
    ) -> Option<StepControlWeightContext> {
      if step_count == 0 || step_count > T::MaxContractSteps::get() || cursor >= step_count {
        return None;
      }
      if cursor == 0 {
        return Some(StepControlWeightContext {
          cursor,
          steps_in_fragment: 1,
          opening_tail_chunks: step_count
            .saturating_sub(1)
            .div_ceil(MAX_STEPS_PER_TAIL_CHUNK),
          predicate_evaluation_units,
          opening_snapshot_entries,
          opening_predicate_results,
          funding_snapshot_entries,
        });
      }
      let chunk_index = cursor.checked_sub(1)? / MAX_STEPS_PER_TAIL_CHUNK;
      let first_step_index =
        1u32.checked_add(chunk_index.checked_mul(MAX_STEPS_PER_TAIL_CHUNK)?)?;
      Some(StepControlWeightContext {
        cursor,
        steps_in_fragment: step_count
          .checked_sub(first_step_index)?
          .min(MAX_STEPS_PER_TAIL_CHUNK),
        opening_tail_chunks: 0,
        predicate_evaluation_units,
        opening_snapshot_entries: 0,
        opening_predicate_results: 0,
        funding_snapshot_entries: 0,
      })
    }

    fn opening_control_geometry(steps: &ContractSteps<T>) -> Option<(u32, u32)> {
      let snapshot_entries = u32::try_from(
        Self::opening_surfaces(steps, 0)
          .into_iter()
          .collect::<BTreeSet<_>>()
          .len(),
      )
      .ok()?;
      let predicate_results = steps
        .iter(/* deos-bypass: bounded-iter */)
        .try_fold(0u32, |total, step| {
          total.checked_add(
            step
              .precondition
              .as_ref()
              .map_or(0, Precondition::opening_predicate_count),
          )
        })?;
      Some((snapshot_entries, predicate_results))
    }

    pub(crate) fn execution_step_control_weight_context(
      instance: &ActiveActorViewOf<T>,
      run: Option<&ActorRunStateOf<T>>,
      loaded_step: &LoadedActorStepOf<T>,
    ) -> Option<StepControlWeightContext> {
      let step_count = u32::try_from(instance.steps.len()).ok()?;
      let cursor = loaded_step.cursor;
      if instance.cycle_state == CycleState::Idle && cursor != 0 {
        return None;
      }
      let predicate_evaluation_units = loaded_step
        .step
        .precondition
        .as_ref()
        .map_or(0, Precondition::evaluation_units);
      let (opening_snapshot_entries, opening_predicate_results) = if cursor == 0 {
        if instance.cycle_state == CycleState::Idle {
          Self::opening_control_geometry(&instance.steps)?
        } else {
          let run = run?;
          (
            u32::try_from(run.opening_snapshot.len()).ok()?,
            u32::try_from(run.opening_predicate_results.len()).ok()?,
          )
        }
      } else {
        (0, 0)
      };
      Self::step_control_weight_context(
        step_count,
        cursor,
        predicate_evaluation_units,
        opening_snapshot_entries,
        opening_predicate_results,
        T::MaxFundingTrackedAssets::get(),
      )
    }

    #[allow(
      dead_code,
      reason = "I4 resource derivation is staged before the production control-Weight owner"
    )]
    pub(crate) fn derive_step_resource_envelopes(
      contract: &ActorContractOf<T>,
    ) -> Option<ActorAdmissionResourcesOf<T>> {
      let step_count = u32::try_from(contract.steps.len()).ok()?;
      let (opening_snapshot_entries, opening_predicate_results) =
        Self::opening_control_geometry(&contract.steps)?;
      contract
        .steps
        .iter(/* deos-bypass: bounded-iter */)
        .enumerate()
        .map(|(cursor, step)| {
          let cursor = u32::try_from(cursor).ok()?;
          let predicate_evaluation_units = step
            .precondition
            .as_ref()
            .map_or(0, Precondition::evaluation_units);
          let context = Self::step_control_weight_context(
            step_count,
            cursor,
            predicate_evaluation_units,
            opening_snapshot_entries,
            opening_predicate_results,
            T::MaxFundingTrackedAssets::get(),
          )?;
          Some(ActorStepResourceEnvelope {
            control: T::StepControlWeight::maximum_control_weight(context, step)?,
            effect: T::TaskEffectWeight::maximum_effect_weight(&step.task)?,
          })
        })
        .collect::<Option<Vec<_>>>()?
        .try_into()
        .ok()
    }

    #[allow(
      dead_code,
      reason = "I2 certificate construction is staged before centralized Contract persistence"
    )]
    pub(crate) fn build_admission_certificate(
      contract: &ActorContractOf<T>,
    ) -> Option<ActorAdmissionCertificateOf<T>> {
      let authority = T::AdmissionCertificateAuthority::current()?;
      Some(ActorAdmissionCertificate::new(
        contract.semantic_contract_id(),
        contract.body_commitment()?,
        authority.runtime_actor_semantics_version,
        authority.production_weight_identity,
        authority.body_geometry_version,
        authority.configured_bounds_commitment,
        authority.maximum_lifecycle_weight,
      ))
    }

    #[allow(
      dead_code,
      reason = "I2 admitted geometry is staged before production storage wiring"
    )]
    pub(crate) fn decompose_admitted_contract_geometry(
      actor_id: ActorId,
      actor_type: ActorType,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> Option<(ActorContractHeadOf<T>, Vec<(u32, ActorStepChunkOf<T>)>)> {
      let semantic_contract_id = contract.semantic_contract_id();
      let body_commitment = contract.body_commitment()?;
      let step_resources = Self::derive_step_resource_envelopes(contract)?;
      if !certificate.has_valid_identity()
        || certificate.semantic_contract_id != semantic_contract_id
        || certificate.body_commitment != body_commitment
      {
        return None;
      }
      let pipeline_machine_envelope =
        Self::derive_pipeline_machine_envelope(actor_type, &contract.steps, &step_resources)
          .ok()?;
      Self::decompose_contract_geometry(
        actor_id,
        contract,
        certificate.admission_identity,
        pipeline_machine_envelope,
        &step_resources,
      )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_actor_step_ticket(
      actor_id: ActorId,
      queue_ticket: QueueTicket,
      eligible_at: BlockNumberFor<T>,
      identity: &ActorIdentityOf<T>,
      hot: &ActorHotStateOf<T>,
      run: Option<&ActorRunStateOf<T>>,
      admission: &ActorAdmissionCertificateOf<T>,
    ) -> Option<ActorStepTicketOf<T>> {
      if hot.queue_ticket != Some(queue_ticket) || !admission.has_valid_identity() {
        return None;
      }
      let (cycle_nonce, cursor, eligible_at) = match (hot.cycle_state, run) {
        (CycleState::Idle, None) => (identity.cycle_nonce.checked_add(1)?, 0, eligible_at),
        (CycleState::Running, Some(run))
          if run.running_is_coherent()
            && run.has_contract_authority(
              admission.semantic_contract_id,
              admission.body_commitment,
              admission.admission_identity,
            ) =>
        {
          if run.eligible_at != eligible_at {
            return None;
          }
          (run.cycle_nonce, run.cursor, run.eligible_at)
        }
        (CycleState::Suspended, Some(run))
          if run.suspension_is_coherent()
            && run.has_contract_authority(
              admission.semantic_contract_id,
              admission.body_commitment,
              admission.admission_identity,
            ) =>
        {
          if run.eligible_at != eligible_at {
            return None;
          }
          (run.cycle_nonce, run.cursor, run.eligible_at)
        }
        _ => return None,
      };
      Some(ActorStepTicket {
        actor_id,
        cycle_nonce,
        cursor,
        ticket: queue_ticket,
        eligible_at,
        contract_commitment: ActorContractCommitment {
          semantic_contract_id: admission.semantic_contract_id,
          body_commitment: admission.body_commitment,
        },
      })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_current_step_plan(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      run: Option<ActorRunStateOf<T>>,
      funding: ActorFundingStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
      ticket: ActorStepTicketOf<T>,
      loaded_step: LoadedActorStepOf<T>,
      maximum_fee: StepFeeBreakdown<T::Balance>,
    ) -> Option<CurrentStepPlanOf<T>> {
      let queue_ticket = hot.queue_ticket?;
      if !Self::validate_loaded_step_authority(
        actor_id,
        queue_ticket,
        &admission,
        &ticket,
        &loaded_step,
      ) {
        return None;
      }
      match (hot.cycle_state, run.as_ref()) {
        (CycleState::Idle, None) => {
          if ticket.cursor != 0 || ticket.cycle_nonce != identity.cycle_nonce.checked_add(1)? {
            return None;
          }
        }
        (CycleState::Running, Some(run)) => {
          if !run.running_is_coherent()
            || !run.has_contract_authority(
              admission.semantic_contract_id,
              admission.body_commitment,
              admission.admission_identity,
            )
            || ticket.cycle_nonce != run.cycle_nonce
            || ticket.cursor != run.cursor
            || ticket.eligible_at != run.eligible_at
          {
            return None;
          }
        }
        (CycleState::Suspended, Some(run)) => {
          if !run.suspension_is_coherent()
            || !run.has_contract_authority(
              admission.semantic_contract_id,
              admission.body_commitment,
              admission.admission_identity,
            )
            || ticket.cycle_nonce != run.cycle_nonce
            || ticket.cursor != run.cursor
            || ticket.eligible_at != run.eligible_at
          {
            return None;
          }
        }
        _ => return None,
      }
      Some(StepExecutionPlan {
        identity,
        hot,
        run,
        funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      })
    }

    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn load_current_step_plan_from_storage(
      ticket: ActorStepTicketOf<T>,
    ) -> Option<CurrentStepPlanOf<T>> {
      if ticket.eligible_at > frame_system::Pallet::<T>::block_number() {
        return None;
      }
      let actor_id = ticket.actor_id;
      let identity = ActorIdentities::<T>::get(actor_id)?;
      let hot = ActorHot::<T>::get(actor_id)?;
      let run = ActorRunStateStore::<T>::get(actor_id);
      let funding = ActorFunding::<T>::get(actor_id)?;
      let admission = ActorAdmissionCertificates::<T>::get(actor_id)?;
      let loaded_step = Self::load_current_step_from_storage(actor_id, ticket.cursor)?;
      let maximum_fee =
        Self::maximum_current_step_fee(identity.actor_class.actor_type(), loaded_step.resources)
          .ok()?;
      Self::build_current_step_plan(
        actor_id,
        identity,
        hot,
        run,
        funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
    }

    pub(crate) fn validate_loaded_step_authority(
      actor_id: ActorId,
      queue_ticket: QueueTicket,
      certificate: &ActorAdmissionCertificateOf<T>,
      ticket: &ActorStepTicketOf<T>,
      loaded_step: &LoadedActorStepOf<T>,
    ) -> bool {
      certificate.has_valid_identity()
        && ticket.actor_id == actor_id
        && ticket.ticket == queue_ticket
        && ticket.cursor == loaded_step.cursor
        && ticket.contract_commitment.semantic_contract_id == certificate.semantic_contract_id
        && ticket.contract_commitment.body_commitment == certificate.body_commitment
    }

    pub(crate) fn load_current_step_from_geometry(
      actor_id: ActorId,
      head: &ActorContractHeadOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
      cursor: u32,
      tail_chunk: Option<(u32, &ActorStepChunkOf<T>)>,
    ) -> Option<LoadedActorStep<StepOf<T>>> {
      if !certificate.has_valid_identity()
        || certificate.semantic_contract_id != head.header.semantic_contract_id
        || certificate.body_commitment != head.header.body_commitment
        || certificate.admission_identity != head.header.admission_identity
      {
        return None;
      }
      if cursor == 0 {
        if tail_chunk.is_some() || head.header.step_count == 0 {
          return None;
        }
        return Some(LoadedActorStep {
          cursor,
          step: head.first_step.clone()?,
          resources: head.first_step_resources?,
        });
      }
      let expected_chunk_index = cursor.checked_sub(1)? / MAX_STEPS_PER_TAIL_CHUNK;
      let expected_first_step_index =
        1u32.checked_add(expected_chunk_index.checked_mul(MAX_STEPS_PER_TAIL_CHUNK)?)?;
      let (chunk_index, chunk) = tail_chunk?;
      if chunk_index != expected_chunk_index
        || !chunk.matches(
          &actor_id,
          &head.header.semantic_contract_id,
          &head.header.body_commitment,
          &head.header.admission_identity,
          expected_first_step_index,
        )
      {
        return None;
      }
      let local_index = cursor.checked_sub(expected_first_step_index)? as usize;
      let resources = *chunk.step_resources.get(local_index)?;
      if chunk.steps.len() != chunk.step_resources.len() {
        return None;
      }
      Some(LoadedActorStep {
        cursor,
        step: chunk.steps.get(local_index)?.clone(),
        resources,
      })
    }

    #[allow(
      dead_code,
      reason = "I2 geometry decomposition is staged before production storage wiring"
    )]
    pub(crate) fn decompose_contract_geometry(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
      admission_identity: [u8; 32],
      pipeline_machine_envelope: PipelineMachineEnvelope<T::Balance>,
      resources: &ActorAdmissionResourcesOf<T>,
    ) -> Option<(ActorContractHeadOf<T>, Vec<(u32, ActorStepChunkOf<T>)>)> {
      let semantic_contract_id = contract.semantic_contract_id();
      let body_commitment = contract.body_commitment()?;
      let header = contract.try_header(
        semantic_contract_id,
        body_commitment,
        admission_identity,
        pipeline_machine_envelope,
      )?;
      if resources.len() != contract.steps.len() {
        return None;
      }
      let first_step = contract.steps.first().cloned();
      let first_step_resources = resources.first().copied();
      let authority = ActorBodyAuthority {
        actor_id,
        semantic_contract_id,
        body_commitment,
        admission_identity,
      };
      let chunks = contract
        .steps
        .as_slice()
        .get(1..)
        .unwrap_or_default()
        .chunks(MAX_STEPS_PER_TAIL_CHUNK as usize)
        .enumerate()
        .map(|(chunk_index, steps)| {
          let chunk_index = u32::try_from(chunk_index).ok()?;
          let first_step_index =
            1u32.checked_add(chunk_index.checked_mul(MAX_STEPS_PER_TAIL_CHUNK)?)?;
          let first_resource = usize::try_from(first_step_index).ok()?;
          let last_resource = first_resource.checked_add(steps.len())?;
          Some((
            chunk_index,
            ActorStepChunk {
              authority: authority.clone(),
              first_step_index,
              steps: BoundedVec::try_from(steps.to_vec()).ok()?,
              step_resources: BoundedVec::try_from(
                resources
                  .as_slice()
                  .get(first_resource..last_resource)?
                  .to_vec(),
              )
              .ok()?,
            },
          ))
        })
        .collect::<Option<Vec<_>>>()?;
      Some((
        ActorContractHead {
          header,
          first_step,
          first_step_resources,
        },
        chunks,
      ))
    }

    #[allow(
      dead_code,
      reason = "I2 bounded reconstruction is staged before production storage wiring"
    )]
    pub(crate) fn reconstruct_contract_geometry(
      actor_id: ActorId,
      head: ActorContractHeadOf<T>,
      chunks: &[(u32, ActorStepChunkOf<T>)],
    ) -> Option<ActorContractOf<T>> {
      let mut steps = Vec::with_capacity(head.header.step_count as usize);
      match (head.first_step, head.first_step_resources) {
        (Some(first_step), Some(_)) if head.header.step_count > 0 => steps.push(first_step),
        (None, None) if head.header.step_count == 0 => {}
        _ => return None,
      }
      for (expected_chunk_index, (chunk_index, chunk)) in chunks
        .iter(/* deos-bypass: bounded-iter */)
        .enumerate()
      {
        let expected_chunk_index = u32::try_from(expected_chunk_index).ok()?;
        if *chunk_index != expected_chunk_index
          || chunk.steps.is_empty()
          || chunk.steps.len() != chunk.step_resources.len()
          || (expected_chunk_index + 1 < chunks.len() as u32
            && chunk.steps.len() != MAX_STEPS_PER_TAIL_CHUNK as usize)
        {
          return None;
        }
        let first_step_index =
          1u32.checked_add(expected_chunk_index.checked_mul(MAX_STEPS_PER_TAIL_CHUNK)?)?;
        if !chunk.matches(
          &actor_id,
          &head.header.semantic_contract_id,
          &head.header.body_commitment,
          &head.header.admission_identity,
          first_step_index,
        ) {
          return None;
        }
        steps.extend(chunk
            .steps
            .iter(/* deos-bypass: bounded-iter */)
            .cloned());
      }
      if steps.len() != head.header.step_count as usize {
        return None;
      }
      let contract = ActorContract {
        trigger: head.header.trigger,
        cooldown_blocks: head.header.cooldown_blocks,
        window: head.header.window,
        steps: ContractSteps::<T>::try_from(steps).ok()?,
        funding: head.header.funding,
        completion: head.header.completion,
        auto_close_at_cycle_nonce: head.header.auto_close_at_cycle_nonce,
      };
      if contract.semantic_contract_id() != head.header.semantic_contract_id
        || contract.body_commitment()? != head.header.body_commitment
      {
        return None;
      }
      Some(contract)
    }

    pub(crate) fn record_crossing_worker_fault(
      meter: &mut WeightMeter,
      fault: CrossingWorkerFault<T::ObservationFeedId>,
    ) -> bool {
      if CrossingWorkerFaultState::<T>::exists() {
        return false;
      }
      let weight = T::WeightInfo::record_crossing_worker_fault();
      if !meter.can_consume(weight) {
        return false;
      }
      meter.consume(weight);
      CrossingWorkerFaultState::<T>::put(fault);
      Self::deposit_event(Event::ActorFaultRecorded {
        fault_id: FaultId::CrossingWorker,
        kind: ActorFaultKind::Detector,
        first_recorded_block: frame_system::Pallet::<T>::block_number(),
        context: FaultContext::Crossing(fault),
      });
      true
    }

    pub(crate) fn record_observation_fanout_worker_fault(
      meter: &mut WeightMeter,
      fault: ObservationFanoutWorkerFault<T::ObservationFeedId>,
    ) -> bool {
      if ObservationFanoutWorkerFaultState::<T>::exists() {
        return false;
      }
      let weight = T::WeightInfo::record_observation_fanout_worker_fault();
      if !meter.can_consume(weight) {
        return false;
      }
      meter.consume(weight);
      ObservationFanoutWorkerFaultState::<T>::put(fault);
      Self::deposit_event(Event::ActorFaultRecorded {
        fault_id: FaultId::ObservationFanoutWorker,
        kind: ActorFaultKind::Detector,
        first_recorded_block: frame_system::Pallet::<T>::block_number(),
        context: FaultContext::ObservationFanout(fault),
      });
      true
    }

    pub(crate) fn record_wakeup_worker_fault(
      meter: &mut WeightMeter,
      fault: WakeupWorkerFault<BlockNumberFor<T>>,
    ) -> bool {
      if WakeupWorkerFaultState::<T>::exists() {
        return false;
      }
      let weight = T::WeightInfo::record_wakeup_worker_fault();
      if !meter.can_consume(weight) {
        return false;
      }
      meter.consume(weight);
      WakeupWorkerFaultState::<T>::put(fault);
      Self::deposit_event(Event::ActorFaultRecorded {
        fault_id: FaultId::WakeupWorker,
        kind: ActorFaultKind::Wakeup,
        first_recorded_block: frame_system::Pallet::<T>::block_number(),
        context: FaultContext::Wakeup(fault),
      });
      true
    }

    pub(crate) fn derive_active_actor_view(
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      contract: ActorContractOf<T>,
    ) -> ActiveActorViewOf<T> {
      ActiveActorView {
        sovereign_account: identity.sovereign_account,
        owner: identity.owner,
        actor_class: identity.actor_class,
        mutability: identity.mutability,
        lifecycle: hot.lifecycle,
        cycle_state: hot.cycle_state,
        trigger: contract.trigger,
        cooldown_blocks: contract.cooldown_blocks,
        window: contract.window,
        steps: contract.steps,
        completion: contract.completion,
        cycle_nonce: identity.cycle_nonce,
        auto_close_at_cycle_nonce: contract.auto_close_at_cycle_nonce,
        unsuccessful_attempt_streak: hot.unsuccessful_attempt_streak,
        pending_signal: hot.pending_signal,
        queue_ticket: hot.queue_ticket,
        wakeup_pointer: hot.wakeup_pointer,
        trigger_wakeup_pointer: hot.trigger_wakeup_pointer,
        last_control_mutation_block: identity.last_control_mutation_block,
        schedule_anchor: hot.schedule_anchor,
        temporal_anchor_tick: hot.trigger_runtime_state.temporal_anchor_tick(),
        temporal_occurrence_consumed: hot.trigger_runtime_state.temporal_occurrence_consumed(),
        last_cycle_block: hot.last_cycle_block,
      }
    }

    pub(crate) fn load_actor_state_with_admission(
      actor_id: ActorId,
    ) -> (
      LoadedActorStateOf<T>,
      Option<ActorAdmissionCertificateOf<T>>,
    ) {
      let identity = ActorIdentities::<T>::get(actor_id);
      let hot = ActorHot::<T>::get(actor_id);
      let admitted_contract = Self::load_admitted_contract_geometry(actor_id);
      let funding = ActorFunding::<T>::get(actor_id);
      let run_state = ActorRunStateStore::<T>::get(actor_id);
      match (identity, hot, admitted_contract, funding, run_state) {
        (None, None, None, None, None) => (LoadedActorStateOf::NotRegistered, None),
        (Some(identity), None, None, None, None) => (LoadedActorStateOf::Dormant(identity), None),
        (Some(identity), Some(hot), Some((contract, admission)), Some(funding), run_state)
          if match (hot.cycle_state, run_state.as_ref()) {
            (CycleState::Idle, None) => true,
            (CycleState::Running, Some(run)) => {
              run.running_is_coherent()
                && run.has_contract_authority(
                  admission.semantic_contract_id,
                  admission.body_commitment,
                  admission.admission_identity,
                )
            }
            (CycleState::Suspended, Some(run)) => {
              run.suspension_is_coherent()
                && run.has_contract_authority(
                  admission.semantic_contract_id,
                  admission.body_commitment,
                  admission.admission_identity,
                )
            }
            _ => false,
          } && hot
            .trigger_runtime_state
            .is_compatible_with(&contract.trigger) =>
        {
          (
            LoadedActorStateOf::Active(ActiveActorState {
              identity,
              hot,
              contract,
              funding,
              run_state,
            }),
            Some(admission),
          )
        }
        _ => (LoadedActorStateOf::Corrupt, None),
      }
    }

    pub(crate) fn load_actor_state(actor_id: ActorId) -> LoadedActorStateOf<T> {
      Self::load_actor_state_with_admission(actor_id).0
    }

    pub fn load_crossing_idle_activation_state(
      actor_id: ActorId,
      feed: T::ObservationFeedId,
    ) -> Option<ObservationActivationState<T>> {
      let identity = ActorIdentities::<T>::get(actor_id)?;
      let hot = ActorHot::<T>::get(actor_id)?;
      if hot.cycle_state != CycleState::Idle
        || !matches!(
          hot.trigger_runtime_state,
          TriggerRuntimeState::ObservationCrossing { .. }
        )
        || ActorRunHeads::<T>::contains_key(actor_id)
      {
        return None;
      }
      let authority = ActorActivationAuthorities::<T>::get(actor_id)?;
      let certificate = ActorAdmissionCertificates::<T>::get(actor_id)?;
      if authority.feed != feed
        || authority.semantic_contract_id != certificate.semantic_contract_id
        || authority.body_commitment != certificate.body_commitment
        || authority.admission_identity != certificate.admission_identity
      {
        return None;
      }
      Some(ObservationActivationState {
        actor_id,
        identity,
        hot,
        authority,
        run_head: None,
        loaded_step: None,
      })
    }

    pub(crate) fn load_observation_activation_state(
      actor_id: ActorId,
      feed: T::ObservationFeedId,
    ) -> Option<ObservationActivationState<T>> {
      let identity = ActorIdentities::<T>::get(actor_id)?;
      let hot = ActorHot::<T>::get(actor_id)?;
      let authority = ActorActivationAuthorities::<T>::get(actor_id)?;
      if authority.feed != feed {
        return None;
      }
      let head = ActorContractHeads::<T>::get(actor_id)?;
      if !matches!(
        &head.header.trigger,
        Trigger::ObservationChange { feed: contract_feed } if *contract_feed == feed
      ) || authority.cooldown_blocks != head.header.cooldown_blocks
        || authority.window != head.header.window
        || authority.auto_close_at_cycle_nonce != head.header.auto_close_at_cycle_nonce
        || authority.semantic_contract_id != head.header.semantic_contract_id
        || authority.body_commitment != head.header.body_commitment
        || authority.admission_identity != head.header.admission_identity
        || !hot
          .trigger_runtime_state
          .is_compatible_with(&head.header.trigger)
      {
        return None;
      }
      let run_head = ActorRunHeads::<T>::get(actor_id);
      let cursor = match (hot.cycle_state, run_head.as_ref()) {
        (CycleState::Idle, None) => 0,
        (CycleState::Running, Some(run))
          if run.running_is_coherent()
            && run.has_contract_authority(
              authority.semantic_contract_id,
              authority.body_commitment,
              authority.admission_identity,
            ) =>
        {
          run.cursor
        }
        (CycleState::Suspended, Some(run))
          if run.suspension_is_coherent()
            && run.has_contract_authority(
              authority.semantic_contract_id,
              authority.body_commitment,
              authority.admission_identity,
            ) =>
        {
          run.cursor
        }
        _ => return None,
      };
      if cursor > head.header.step_count
        || (cursor == head.header.step_count && head.header.step_count > 0)
      {
        return None;
      }
      let loaded_step = if head.header.step_count == 0 {
        if cursor != 0 || head.first_step.is_some() || head.first_step_resources.is_some() {
          return None;
        }
        None
      } else if cursor == 0 {
        Some(LoadedActorStep {
          cursor,
          step: head.first_step.clone()?,
          resources: head.first_step_resources?,
        })
      } else {
        let chunk_index = cursor.checked_sub(1)? / MAX_STEPS_PER_TAIL_CHUNK;
        let chunk = ActorContractTailChunks::<T>::get(actor_id, chunk_index)?;
        let expected_first_step_index =
          1u32.checked_add(chunk_index.checked_mul(MAX_STEPS_PER_TAIL_CHUNK)?)?;
        if !chunk.matches(
          &actor_id,
          &authority.semantic_contract_id,
          &authority.body_commitment,
          &authority.admission_identity,
          expected_first_step_index,
        ) || chunk.steps.len() != chunk.step_resources.len()
        {
          return None;
        }
        let local_index = cursor.checked_sub(expected_first_step_index)? as usize;
        Some(LoadedActorStep {
          cursor,
          step: chunk.steps.get(local_index)?.clone(),
          resources: *chunk.step_resources.get(local_index)?,
        })
      };
      Some(ObservationActivationState {
        actor_id,
        identity,
        hot,
        authority,
        run_head,
        loaded_step,
      })
    }

    pub(crate) fn load_actor_service_state(
      actor_id: ActorId,
    ) -> Option<(
      ActiveActorStateOf<T>,
      ActorAdmissionCertificateOf<T>,
      Option<LoadedActorStepOf<T>>,
    )> {
      let identity = ActorIdentities::<T>::get(actor_id)?;
      let hot = ActorHot::<T>::get(actor_id)?;
      let head = ActorContractHeads::<T>::get(actor_id)?;
      let funding = ActorFunding::<T>::get(actor_id)?;
      let run_state = ActorRunStateStore::<T>::get(actor_id);
      let admission = ActorAdmissionCertificates::<T>::get(actor_id)?;
      let cursor = match (hot.cycle_state, run_state.as_ref()) {
        (CycleState::Idle, None) => 0,
        (CycleState::Running, Some(run))
          if run.running_is_coherent()
            && run.has_contract_authority(
              admission.semantic_contract_id,
              admission.body_commitment,
              admission.admission_identity,
            ) =>
        {
          run.cursor
        }
        (CycleState::Suspended, Some(run))
          if run.suspension_is_coherent()
            && run.has_contract_authority(
              admission.semantic_contract_id,
              admission.body_commitment,
              admission.admission_identity,
            ) =>
        {
          run.cursor
        }
        _ => return None,
      };
      let tail_chunk = if cursor == 0 {
        None
      } else {
        let chunk_index = cursor.checked_sub(1)? / MAX_STEPS_PER_TAIL_CHUNK;
        Some((
          chunk_index,
          ActorContractTailChunks::<T>::get(actor_id, chunk_index)?,
        ))
      };
      let loaded_step = if head.header.step_count == 0 {
        if cursor != 0
          || tail_chunk.is_some()
          || head.first_step.is_some()
          || head.first_step_resources.is_some()
        {
          return None;
        }
        None
      } else {
        Some(Self::load_current_step_from_geometry(
          actor_id,
          &head,
          &admission,
          cursor,
          tail_chunk
            .as_ref()
            .map(|(chunk_index, chunk)| (*chunk_index, chunk)),
        )?)
      };
      if !hot
        .trigger_runtime_state
        .is_compatible_with(&head.header.trigger)
      {
        return None;
      }
      let mut steps = ContractSteps::<T>::default();
      if let Some(loaded_step) = loaded_step.as_ref() {
        for _ in 0..head.header.step_count {
          steps.try_push(loaded_step.step.clone()).ok()?;
        }
      }
      let contract = ActorContract {
        trigger: head.header.trigger,
        cooldown_blocks: head.header.cooldown_blocks,
        window: head.header.window,
        funding: head.header.funding,
        steps,
        completion: head.header.completion,
        auto_close_at_cycle_nonce: head.header.auto_close_at_cycle_nonce,
      };
      Some((
        ActiveActorState {
          identity,
          hot,
          contract,
          funding,
          run_state,
        },
        admission,
        loaded_step,
      ))
    }

    pub(crate) fn load_current_step_service_state(
      actor_id: ActorId,
    ) -> Option<(
      ActiveActorStateOf<T>,
      ActorAdmissionCertificateOf<T>,
      LoadedActorStepOf<T>,
    )> {
      let (state, admission, loaded_step) = Self::load_actor_service_state(actor_id)?;
      Some((state, admission, loaded_step?))
    }

    pub(crate) fn active_actor_view(actor_id: ActorId) -> Option<ActiveActorViewOf<T>> {
      let LoadedActorStateOf::Active(state) = Self::load_actor_state(actor_id) else {
        return None;
      };
      Some(Self::derive_active_actor_view(
        state.identity,
        state.hot,
        state.contract,
      ))
    }

    pub(crate) fn active_actor_state_for_control(
      actor_id: ActorId,
    ) -> Result<ActiveActorStateOf<T>, Error<T>> {
      match Self::load_actor_state(actor_id) {
        LoadedActorStateOf::Active(state) => Ok(state),
        LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => {
          Err(Error::<T>::ActorNotFound)
        }
        LoadedActorStateOf::Corrupt => Err(Error::<T>::ActorInvariant),
      }
    }

    pub fn active_actor_state(actor_id: ActorId) -> Option<ActiveActorStateOf<T>> {
      match Self::load_actor_state(actor_id) {
        LoadedActorStateOf::Active(state) => Some(state),
        _ => None,
      }
    }

    pub fn pending_signal(actor_id: ActorId) -> bool {
      match Self::load_actor_state(actor_id) {
        LoadedActorStateOf::Active(state) => state.hot.pending_signal,
        _ => false,
      }
    }

    pub fn wakeup_pages(key: (BlockNumberFor<T>, WakeupPageId)) -> Option<WakeupPageOf<T>> {
      WakeupPages::<T>::get((WakeupKey::Block(key.0), key.1))
    }

    pub fn wakeup_buckets(block: BlockNumberFor<T>) -> Option<WakeupBucketState> {
      WakeupBuckets::<T>::get(WakeupKey::Block(block))
    }

    pub fn wakeup_cursor_pages(page_id: WakeupPageId) -> Option<WakeupCursorPageOf<T>> {
      WakeupCursorPages::<T>::get((WakeupClock::Block, page_id))
    }

    pub fn wakeup_cursor_len() -> WakeupCursorIndex {
      WakeupCursorLen::<T>::get(WakeupClock::Block)
    }

    pub(crate) fn active_actor_exists(actor_id: ActorId) -> bool {
      matches!(
        Self::load_actor_state(actor_id),
        LoadedActorStateOf::Active(_)
      )
    }

    pub(crate) fn preflight_trigger_transition(
      actor_id: ActorId,
      trigger: &TriggerOf<T>,
      intent: TriggerTransitionIntent,
    ) -> Result<TriggerTransitionPlan<T>, DispatchError> {
      Ok(TriggerTransitionPlan {
        intent,
        crossing: Self::preflight_crossing_membership(actor_id, trigger)?,
        observation_feeds: Self::preflight_observation_subscription_replace(actor_id, trigger)?,
      })
    }

    fn preflight_trigger_cleanup(
      actor_id: ActorId,
      intent: TriggerTransitionIntent,
    ) -> Result<TriggerTransitionPlan<T>, DispatchError> {
      ensure!(
        matches!(
          intent,
          TriggerTransitionIntent::Deactivate | TriggerTransitionIntent::Close
        ),
        Error::<T>::ActorInvariant
      );
      Self::preflight_remove_observation_subscriptions(actor_id)?;
      Self::preflight_trigger_transition(actor_id, &TriggerOf::<T>::Manual, intent)
    }

    fn commit_trigger_transition(
      actor_id: ActorId,
      plan: TriggerTransitionPlan<T>,
      prospective_is_user: Option<bool>,
    ) -> Result<Option<(CrossingPhase, ObservationRevision)>, DispatchError> {
      let _intent = plan.intent;
      IndexedTriggerDetectionDisabled::<T>::remove(actor_id);
      let is_user = prospective_is_user.unwrap_or_else(|| {
        ActorIdentities::<T>::get(actor_id)
          .is_some_and(|identity| matches!(identity.actor_class, ActorClass::User { .. }))
      });
      let crossing_state = Self::commit_crossing_membership(actor_id, plan.crossing, is_user)?;
      Self::commit_observation_subscription_replace(actor_id, plan.observation_feeds)?;
      Ok(crossing_state)
    }

    pub(crate) fn insert_active_actor(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      mut hot: ActorHotStateOf<T>,
      contract: ActorContractOf<T>,
      intent: TriggerTransitionIntent,
    ) -> DispatchResult {
      let transition = Self::preflight_trigger_transition(actor_id, &contract.trigger, intent)?;
      let crossing_state = Self::commit_trigger_transition(
        actor_id,
        transition,
        Some(matches!(identity.actor_class, ActorClass::User { .. })),
      )?;
      hot.trigger_runtime_state = Self::installed_trigger_runtime_state(
        &contract.trigger,
        hot.trigger_runtime_state.temporal_anchor_tick(),
        crossing_state,
      )?;
      ActorIdentities::<T>::insert(actor_id, identity);
      ActorHot::<T>::insert(actor_id, hot);
      Self::store_actor_contract(actor_id, contract)
    }

    fn provisional_trigger_runtime_state(
      trigger: &TriggerOf<T>,
      temporal_anchor_tick: Option<SchedulerTick>,
    ) -> TriggerRuntimeState {
      match trigger {
        Trigger::AtTime { .. } => TriggerRuntimeState::AtTime {
          anchor_tick: temporal_anchor_tick,
          consumed: false,
        },
        Trigger::Cadenced { .. } => TriggerRuntimeState::Cadenced {
          anchor_tick: temporal_anchor_tick,
        },
        Trigger::Manual
        | Trigger::AddressEvent { .. }
        | Trigger::ObservationChange { .. }
        | Trigger::ObservationCrossing { .. } => TriggerRuntimeState::Stateless,
      }
    }

    fn installed_trigger_runtime_state(
      trigger: &TriggerOf<T>,
      temporal_anchor_tick: Option<SchedulerTick>,
      crossing_state: Option<(CrossingPhase, ObservationRevision)>,
    ) -> Result<TriggerRuntimeState, DispatchError> {
      match trigger {
        Trigger::ObservationCrossing { .. } => {
          let (phase, installed_at_revision) =
            crossing_state.ok_or(Error::<T>::CrossingIndexInvariant)?;
          Ok(TriggerRuntimeState::ObservationCrossing {
            phase,
            installed_at_revision,
          })
        }
        Trigger::AtTime { .. } => Ok(TriggerRuntimeState::AtTime {
          anchor_tick: temporal_anchor_tick,
          consumed: false,
        }),
        Trigger::Cadenced { .. } => Ok(TriggerRuntimeState::Cadenced {
          anchor_tick: temporal_anchor_tick,
        }),
        Trigger::Manual | Trigger::AddressEvent { .. } | Trigger::ObservationChange { .. } => {
          Ok(TriggerRuntimeState::Stateless)
        }
      }
    }

    pub(crate) fn remove_active_actor(
      actor_id: ActorId,
      trigger_transition: TriggerTransitionPlan<T>,
    ) -> DispatchResult {
      Self::commit_trigger_transition(actor_id, trigger_transition, None)?;
      ActorHot::<T>::remove(actor_id);
      Self::remove_actor_contract(actor_id)?;
      ActorRunStateStore::<T>::remove(actor_id);
      Ok(())
    }
  }

  #[pallet::storage]
  #[pallet::getter(fn actor_identities)]
  pub type ActorIdentities<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorIdentityOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn actor_identity_count)]
  pub type ActorIdentityCount<T> = StorageValue<_, u32, ValueQuery>;

  /// Per-Actor authority for the owner's aggregate dedicated Actor-state hold.
  #[pallet::storage]
  #[pallet::getter(fn actor_state_hold)]
  pub type ActorStateHolds<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorStateHoldRecordOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn active_actor_count)]
  pub type ActiveActorCount<T> = StorageValue<_, u32, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn system_sovereigns)]
  pub type SystemSovereigns<T: Config> =
    StorageMap<_, Blake2_128Concat, SystemSovereignId, SystemSovereignState, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn system_sovereign_count)]
  pub type SystemSovereignCount<T> = StorageValue<_, u32, ValueQuery>;

  /// Next never-used ticket for the canonical FIFO.
  #[pallet::storage]
  #[pallet::getter(fn next_queue_ticket)]
  pub type NextQueueTicket<T> = StorageValue<_, QueueTicket, ValueQuery>;

  /// Ticket frontier frozen at block initialization before ordinary external causes.
  #[pallet::storage]
  #[pallet::getter(fn prepass_execution_cutoff)]
  pub type PrepassExecutionCutoff<T: Config> =
    StorageValue<_, (BlockNumberFor<T>, QueueTicket), OptionQuery>;

  /// Authoritative transient resource protocol state for the current block.
  #[pallet::storage]
  #[pallet::getter(fn block_resource_state)]
  pub type CurrentBlockResourceState<T: Config> =
    StorageValue<_, BlockResourceState<BlockNumberFor<T>>, OptionQuery>;

  /// Latest successfully reconciled block resource counters; read-only and non-authoritative.
  #[pallet::storage]
  #[pallet::getter(fn finalized_block_resource_telemetry)]
  pub type FinalizedBlockResourceTelemetry<T: Config> =
    StorageValue<_, FinalizedBlockResourceSnapshot<BlockNumberFor<T>>, OptionQuery>;

  /// Physical head position of the canonical paged FIFO.
  #[pallet::storage]
  #[pallet::getter(fn queue_head)]
  pub type QueueHead<T> = StorageValue<_, QueueTicket, ValueQuery>;

  /// Next physical position in the canonical paged FIFO.
  #[pallet::storage]
  #[pallet::getter(fn queue_tail)]
  pub type QueueTail<T> = StorageValue<_, QueueTicket, ValueQuery>;

  /// Exact unconsumed physical entries, including tombstones, in the canonical FIFO.
  #[pallet::storage]
  #[pallet::getter(fn queue_occupancy)]
  pub type QueueOccupancy<T> = StorageValue<_, u32, ValueQuery>;

  /// Bounded physical pages for the canonical FIFO.
  #[pallet::storage]
  #[pallet::getter(fn queue_pages)]
  pub type QueuePages<T: Config> =
    StorageMap<_, Blake2_128Concat, QueuePageId, QueuePageOf<T>, OptionQuery>;

  /// Fixed-size pages for block and timestamp-tick wakeups.
  #[pallet::storage]
  pub type WakeupPages<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    (WakeupKey<BlockNumberFor<T>>, WakeupPageId),
    WakeupPageOf<T>,
    OptionQuery,
  >;

  /// Small per-deadline ownership and allocation metadata for temporal pages.
  #[pallet::storage]
  pub type WakeupBuckets<T: Config> =
    StorageMap<_, Blake2_128Concat, WakeupKey<BlockNumberFor<T>>, WakeupBucketState, OptionQuery>;

  /// Paged binary min-heaps of distinct block and timestamp-tick deadlines.
  #[pallet::storage]
  pub type WakeupCursorPages<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    (WakeupClock, WakeupPageId),
    WakeupCursorPageOf<T>,
    OptionQuery,
  >;

  /// Logical length of each sparse-wakeup cursor heap.
  #[pallet::storage]
  pub type WakeupCursorLen<T> =
    StorageMap<_, Blake2_128Concat, WakeupClock, WakeupCursorIndex, ValueQuery>;

  /// Clock selected first when both temporal domains have due work.
  #[pallet::storage]
  pub type NextWakeupClock<T> = StorageValue<_, WakeupClock, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn wakeup_worker_fault)]
  pub type WakeupWorkerFaultState<T: Config> =
    StorageValue<_, WakeupWorkerFault<BlockNumberFor<T>>, OptionQuery>;

  pub type OwnerSlotBitmap = [u8; 32];

  #[pallet::storage]
  #[pallet::getter(fn owner_slot_bitmap)]
  pub type OwnerSlotBitmaps<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AccountId, OwnerSlotBitmap, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn sovereign_index)]
  pub type SovereignIndex<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AccountId, ActorId, OptionQuery>;

  /// Explicit nonzero governance-configurable active actor limit.
  #[pallet::storage]
  #[pallet::getter(fn configured_active_actor_limit)]
  pub type ActiveActorLimit<T: Config> = StorageValue<_, u32, ValueQuery>;

  /// Detector-local latch authority for indexed Trigger memberships retained during active traversal.
  #[pallet::storage]
  #[pallet::getter(fn indexed_trigger_detection_disabled)]
  pub type IndexedTriggerDetectionDisabled<T> =
    StorageMap<_, Blake2_128Concat, ActorId, (), OptionQuery>;

  /// Canonical observation feed ownership derived from each active actor's trigger policy.
  #[pallet::storage]
  #[pallet::getter(fn actor_observation_feeds)]
  pub type ActorObservationFeeds<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorObservationFeedsOf<T>, OptionQuery>;

  /// Reusable dense slot owned only while an actor has observation subscriptions.
  #[pallet::storage]
  #[pallet::getter(fn observation_subscription_slot)]
  pub type ObservationSubscriptionSlot<T> =
    StorageMap<_, Blake2_128Concat, ActorId, u32, OptionQuery>;

  #[pallet::storage]
  pub type ObservationSubscriptionSlotOwner<T> =
    StorageMap<_, Blake2_128Concat, u32, ActorId, OptionQuery>;

  #[pallet::storage]
  pub type NextObservationSubscriptionSlot<T> = StorageValue<_, u32, ValueQuery>;

  #[pallet::storage]
  pub type ObservationFreeSlotLen<T> = StorageValue<_, u32, ValueQuery>;

  #[pallet::storage]
  pub type ObservationFreeSlotPages<T: Config> =
    StorageMap<_, Blake2_128Concat, u32, ObservationFreeSlotPageOf<T>, OptionQuery>;

  /// Fixed slot-addressed subscriber pages linked through occupied pages only.
  #[pallet::storage]
  #[pallet::getter(fn observation_subscriber_pages)]
  pub type ObservationSubscriberPages<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::ObservationFeedId,
    Blake2_128Concat,
    u32,
    ObservationSubscriberPageOf<T>,
    OptionQuery,
  >;

  /// Exact occupied-page list for one feed; absent when the feed has no subscribers.
  #[pallet::storage]
  #[pallet::getter(fn observation_subscriber_page_list)]
  pub type ObservationSubscriberPageLists<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::ObservationFeedId,
    ObservationSubscriberPageList,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn observation_subscriber_count)]
  pub type ObservationSubscriberCount<T: Config> =
    StorageMap<_, Blake2_128Concat, T::ObservationFeedId, u32, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn observation_subscription_count)]
  pub type ObservationSubscriptionCount<T> = StorageValue<_, u32, ValueQuery>;

  /// Highest accepted revision retained while a feed has at least one subscriber.
  #[pallet::storage]
  #[pallet::getter(fn observation_ingress_revision)]
  pub type ObservationIngressRevisions<T: Config> =
    StorageMap<_, Blake2_128Concat, T::ObservationFeedId, ObservationRevision, OptionQuery>;

  /// Latest changed revision and deferred-fanout cursor for one subscribed feed.
  #[pallet::storage]
  #[pallet::getter(fn dirty_observation_feeds)]
  pub type DirtyObservationFeeds<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::ObservationFeedId,
    DirtyObservationState<T::ObservationFeedId, BlockNumberFor<T>>,
    OptionQuery,
  >;

  /// Exact bounded active-dirty ownership and fair fanout cursor.
  #[pallet::storage]
  #[pallet::getter(fn dirty_observation_list)]
  pub type DirtyObservationListState<T: Config> =
    StorageValue<_, DirtyObservationList<T::ObservationFeedId>, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn observation_fanout_worker_fault)]
  pub type ObservationFanoutWorkerFaultState<T: Config> =
    StorageValue<_, ObservationFanoutWorkerFault<T::ObservationFeedId>, OptionQuery>;

  /// Exact current Crossing obligation owned by one active Actor.
  #[pallet::storage]
  #[pallet::getter(fn crossing_membership)]
  pub type CrossingMemberships<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, CrossingMembershipLocatorOf<T>, OptionQuery>;

  /// Dense bounded membership pages at one exact occupied threshold leaf.
  #[pallet::storage]
  pub type CrossingMemberPages<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    CrossingLeafKeyOf<T>,
    Blake2_128Concat,
    u32,
    CrossingMemberPageOf<T>,
    OptionQuery,
  >;

  /// Allocation and cardinality state for one exact occupied threshold leaf.
  #[pallet::storage]
  pub type CrossingLeafStates<T: Config> =
    StorageMap<_, Blake2_128Concat, CrossingLeafKeyOf<T>, CrossingLeafState, OptionQuery>;

  /// Sixteen-way occupancy at each sparse u128 threshold radix node.
  #[pallet::storage]
  pub type CrossingRadixNodes<T: Config> =
    StorageMap<_, Blake2_128Concat, CrossingRadixNodeKeyOf<T>, u16, OptionQuery>;

  /// Exact live Crossing membership count per feed.
  #[pallet::storage]
  #[pallet::getter(fn crossing_feed_membership_count)]
  pub type CrossingFeedMembershipCount<T: Config> =
    StorageMap<_, Blake2_128Concat, T::ObservationFeedId, u32, ValueQuery>;

  /// Exact live User Crossing membership count per feed; System capacity remains reserved.
  #[pallet::storage]
  #[pallet::getter(fn crossing_user_feed_membership_count)]
  pub type CrossingUserFeedMembershipCount<T: Config> =
    StorageMap<_, Blake2_128Concat, T::ObservationFeedId, u32, ValueQuery>;

  /// Exact bounded revision queue retained while one feed has Crossing members.
  #[pallet::storage]
  #[pallet::getter(fn crossing_transition_queue)]
  pub type CrossingTransitionQueues<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::ObservationFeedId,
    CrossingTransitionQueueOf<T>,
    OptionQuery,
  >;

  /// Linked ownership for feeds with at least one pending Crossing transition.
  #[pallet::storage]
  pub type CrossingPendingFeeds<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::ObservationFeedId,
    CrossingPendingFeedState<T::ObservationFeedId>,
    OptionQuery,
  >;

  /// Fair cursor across feeds with pending Crossing transition work.
  #[pallet::storage]
  #[pallet::getter(fn crossing_pending_feed_list)]
  pub type CrossingPendingFeedListState<T: Config> =
    StorageValue<_, CrossingPendingFeedList<T::ObservationFeedId>, ValueQuery>;

  /// Exact suffix cursor for the head transition currently materializing on one feed.
  #[pallet::storage]
  #[pallet::getter(fn crossing_range_cursor)]
  pub type CrossingRangeCursors<T: Config> =
    StorageMap<_, Blake2_128Concat, T::ObservationFeedId, CrossingRangeCursor, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn crossing_worker_fault)]
  pub type CrossingWorkerFaultState<T: Config> =
    StorageValue<_, CrossingWorkerFault<T::ObservationFeedId>, OptionQuery>;

  /// Round-robin start family: 0 wakeups, 1 Crossing, 2 broad fanout.
  #[pallet::storage]
  #[pallet::getter(fn materialization_family_cursor)]
  pub type MaterializationFamilyCursor<T> = StorageValue<_, u8, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn global_circuit_breaker)]
  pub type GlobalCircuitBreaker<T> = StorageValue<_, bool, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn idle_starvation_state)]
  pub type IdleStarvationState<T: Config> = StorageValue<_, IdleStarvationPhase, ValueQuery>;

  /// Provides runtime-specific System Actors instances to initialize at genesis.
  ///
  /// Implement this on the runtime to return System Actors specs with explicit `actor_id` values.
  /// IDs may be sparse to reserve stable addresses for non-actor accounts.
  pub trait GenesisSystemActors<AccountId, Contract> {
    fn system_actors() -> alloc::vec::Vec<(ActorId, AccountId, Mutability, Contract)>;

    fn dormant_system_actors() -> alloc::vec::Vec<(ActorId, AccountId)> {
      alloc::vec::Vec::new()
    }

    /// Runtime-declared deterministic custody accounts that need a provider at genesis
    /// but own no generic Actors identity, contract, or scheduler state.
    fn system_custody_accounts() -> alloc::vec::Vec<ActorId> {
      alloc::vec::Vec::new()
    }

    /// Host-composition assertions that must remain true across runtime upgrades.
    fn integrity_test() {}
  }

  /// Default no-op implementation: no System Actors created at genesis.
  impl<AccountId, Contract> GenesisSystemActors<AccountId, Contract> for () {
    fn system_actors() -> alloc::vec::Vec<(ActorId, AccountId, Mutability, Contract)> {
      alloc::vec::Vec::new()
    }
  }

  #[pallet::genesis_config]
  #[derive(frame::prelude::DefaultNoBound)]
  pub struct GenesisConfig<T: Config> {
    #[serde(skip)]
    pub _marker: core::marker::PhantomData<T>,
  }

  #[pallet::genesis_build]
  impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
    fn build(&self) {
      assert!(
        contract_steps_bound_is_valid(T::MaxContractSteps::get()),
        "MaxContractSteps must be in 1..=255"
      );
      assert_eq!(
        T::MaxOpeningSnapshotEntries::get(),
        T::MaxContractSteps::get()
          .checked_mul(2)
          .expect("opening amount-surface bound must fit u32"),
        "MaxOpeningSnapshotEntries must equal two per execution-plan step"
      );
      assert_eq!(
        T::MaxOpeningPredicateResults::get(),
        T::MaxContractSteps::get()
          .checked_mul(T::MaxPredicatesPerStep::get())
          .expect("opening predicate-result bound must fit u32"),
        "MaxOpeningPredicateResults must equal MaxContractSteps * MaxPredicatesPerStep"
      );
      STORAGE_VERSION.put::<Pallet<T>>();
      if ActiveActorLimit::<T>::get() == 0 {
        ActiveActorLimit::<T>::put(Pallet::<T>::max_configurable_active_actor_limit());
      }
      for (actor_id, owner, mutability, mut contract) in T::GenesisSystemActors::system_actors() {
        assert!(
          !Pallet::<T>::active_actor_exists(actor_id),
          "duplicate genesis System Actors id: {actor_id}"
        );
        let next_id = actor_id
          .checked_add(1)
          .expect("genesis Actors id must not overflow u64");
        if NextActorId::<T>::get() < next_id {
          NextActorId::<T>::put(next_id);
        }
        let sovereign_account = Pallet::<T>::sovereign_account_id_system(actor_id);
        assert!(
          !SovereignIndex::<T>::contains_key(&sovereign_account),
          "genesis System Actors sovereign collision at actor_id={actor_id}"
        );
        assert!(
          mutability == Mutability::Mutable || !contract.trigger.manual_source_enabled(),
          "genesis System Immutable Actors cannot admit Manual readiness"
        );
        Pallet::<T>::validate_trigger(&contract.trigger, contract.cooldown_blocks)
          .expect("genesis trigger and cooldown must be valid");
        if let Some(ref window) = contract.window {
          Pallet::<T>::validate_schedule_window(window)
            .expect("genesis execution window must be valid");
        }
        Pallet::<T>::validate_future_schedule_targets(&contract)
          .expect("genesis future schedule targets must be valid");
        if let Some(target_nonce) = contract.auto_close_at_cycle_nonce {
          Pallet::<T>::ensure_auto_close_target(0, target_nonce)
            .expect("genesis auto-close target must be nonzero");
        }
        Pallet::<T>::canonicalize_preconditions(&mut contract.steps)
          .expect("genesis precondition formulas must have valid bounded DNF");
        Pallet::<T>::validate_contract_steps_shape(ActorType::System, &contract.steps)
          .expect("genesis execution plan must have valid task and predicate shapes");
        T::SystemActorContractValidator::validate(actor_id, &contract)
          .expect("genesis System Actor topology must be valid"); // deos-bypass: panic-owner — genesis integrity validation fails closed before launch.
        Pallet::<T>::validate_recipient_configuration(&contract.steps, &sovereign_account)
          .expect("genesis execution plan cannot transfer to its own sovereign account");
        Pallet::<T>::validate_opening_snapshot_surfaces(&contract.steps)
          .expect("genesis opening snapshot surfaces must be valid");
        Pallet::<T>::ensure_retry_later_allowed(mutability, &contract.steps)
          .expect("genesis System Immutable Actors cannot use RetryLater");
        Pallet::<T>::ensure_contract_steps_fits_idle_budget(ActorType::System, &contract.steps)
          .unwrap_or_else(|_| {
            panic!("genesis System Actors {actor_id} exceeds the guaranteed on_idle budget")
          });
        let funding_tracked_assets = Pallet::<T>::derive_funding_tracked_assets(&contract.steps)
          .expect("genesis contract steps must have valid funding-tracked assets");
        let schedule_anchor = Pallet::<T>::schedule_anchor_at(contract.window, Zero::zero());
        // Genesis has no consensus timestamp. Temporal actors use `None` as a bounded bootstrap
        // marker and anchor from the first timestamp observed by ordinary wakeup service.
        let temporal_anchor_tick = None;
        let identity = ActorIdentity {
          sovereign_account: sovereign_account.clone(),
          owner: owner.clone(),
          actor_class: ActorClass::System {
            sovereign_id: actor_id,
          },
          mutability,
          cycle_nonce: 0,
          last_control_mutation_block: Zero::zero(),
        };
        let trigger_runtime_state =
          Pallet::<T>::provisional_trigger_runtime_state(&contract.trigger, temporal_anchor_tick);
        let hot = ActorHotState {
          lifecycle: ActiveLifecycle::Active,
          cycle_state: CycleState::Idle,
          trigger_runtime_state,
          unsuccessful_attempt_streak: 0,
          pending_signal: false,
          queue_ticket: None,
          wakeup_pointer: None,
          trigger_wakeup_pointer: None,
          terminal_at: contract
            .window
            .map(|window| Pallet::<T>::window_terminal_at(&window)),
          schedule_anchor,
          last_cycle_block: None,
        };
        let active_count = Pallet::<T>::active_instance_count();
        assert!(
          active_count < T::MaxActiveActors::get(),
          "genesis active actor capacity exceeded at actor_id={actor_id}"
        );
        assert!(
          SystemSovereignCount::<T>::get() < T::MaxSystemSovereigns::get(),
          "genesis System sovereign capacity exceeded at sovereign_id={actor_id}"
        );
        assert!(
          !SystemSovereigns::<T>::contains_key(actor_id),
          "duplicate genesis System sovereign locator: {actor_id}"
        );
        SystemSovereigns::<T>::insert(actor_id, SystemSovereignState::Occupied(actor_id));
        SystemSovereignCount::<T>::mutate(|count| *count = count.saturating_add(1));
        SovereignIndex::<T>::insert(&sovereign_account, actor_id);
        frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        Pallet::<T>::insert_active_actor(
          actor_id,
          identity,
          hot,
          contract,
          TriggerTransitionIntent::GenesisInstallation,
        )
        .unwrap_or_else(|error| panic!("genesis observation subscription failed: {error:?}")); // deos-bypass: panic-owner genesis construction fails before launch
        ActorFunding::<T>::insert(
          actor_id,
          ActorFundingState {
            funding_accumulated: Default::default(),
            funding_tracked_assets,
          },
        );
        ActiveActorCount::<T>::put(
          active_count
            .checked_add(1)
            .expect("genesis active actor count must not overflow"),
        );
        ActorIdentityCount::<T>::put(
          ActorIdentityCount::<T>::get()
            .checked_add(1)
            .expect("genesis actor identity count must not overflow"),
        );
        assert!(
          ActorIdentityCount::<T>::get() <= T::MaxActorIdentities::get(),
          "genesis actor identity capacity exceeded at actor_id={actor_id}"
        );
        Pallet::<T>::prime_actor_schedule(actor_id)
          .expect("genesis placement preserves readiness (spec 8.1.4)");
      }
      for (actor_id, owner) in T::GenesisSystemActors::dormant_system_actors() {
        assert!(
          !Pallet::<T>::active_actor_exists(actor_id)
            && !ActorIdentities::<T>::contains_key(actor_id),
          "duplicate genesis System Actors id: {actor_id}"
        );
        let next_id = actor_id
          .checked_add(1)
          .expect("genesis Actors id must not overflow u64");
        if NextActorId::<T>::get() < next_id {
          NextActorId::<T>::put(next_id);
        }
        let sovereign_account = Pallet::<T>::sovereign_account_id_system(actor_id);
        assert!(
          !SovereignIndex::<T>::contains_key(&sovereign_account),
          "genesis System Actors sovereign collision at actor_id={actor_id}"
        );
        let identity = ActorIdentity {
          sovereign_account: sovereign_account.clone(),
          owner,
          actor_class: ActorClass::System {
            sovereign_id: actor_id,
          },
          mutability: Mutability::Mutable,
          cycle_nonce: 0,
          last_control_mutation_block: Zero::zero(),
        };
        let identity_count = ActorIdentityCount::<T>::get();
        assert!(
          identity_count < T::MaxActorIdentities::get(),
          "genesis actor identity capacity exceeded at actor_id={actor_id}"
        );
        assert!(
          SystemSovereignCount::<T>::get() < T::MaxSystemSovereigns::get(),
          "genesis System sovereign capacity exceeded at sovereign_id={actor_id}"
        );
        assert!(
          !SystemSovereigns::<T>::contains_key(actor_id),
          "duplicate genesis System sovereign locator: {actor_id}"
        );
        SystemSovereigns::<T>::insert(actor_id, SystemSovereignState::Occupied(actor_id));
        SystemSovereignCount::<T>::mutate(|count| *count = count.saturating_add(1));
        SovereignIndex::<T>::insert(&sovereign_account, actor_id);
        frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        ActorIdentities::<T>::insert(actor_id, identity);
        ActorIdentityCount::<T>::put(
          identity_count
            .checked_add(1)
            .expect("genesis actor identity count must not overflow"),
        );
      }
      for actor_id in T::GenesisSystemActors::system_custody_accounts() {
        assert!(
          !Pallet::<T>::active_actor_exists(actor_id)
            && !ActorIdentities::<T>::contains_key(actor_id),
          "genesis custody account collides with actor identity: {actor_id}"
        );
        let sovereign_account = Pallet::<T>::sovereign_account_id_system(actor_id);
        assert!(
          !SovereignIndex::<T>::contains_key(&sovereign_account),
          "genesis custody account has generic sovereign index: {actor_id}"
        );
        assert!(
          SystemSovereignCount::<T>::get() < T::MaxSystemSovereigns::get(),
          "genesis System sovereign capacity exceeded at sovereign_id={actor_id}"
        );
        assert!(
          !SystemSovereigns::<T>::contains_key(actor_id),
          "duplicate genesis System sovereign locator: {actor_id}"
        );
        SystemSovereigns::<T>::insert(actor_id, SystemSovereignState::Vacant);
        SystemSovereignCount::<T>::mutate(|count| *count = count.saturating_add(1));
        frame_system::Pallet::<T>::inc_providers(&sovereign_account);
      }
    }
  }

  impl<T: Config> Pallet<T> {
    pub(crate) fn materialization_family_has_work(family: u8, now: BlockNumberFor<T>) -> bool {
      match family {
        0 if !WakeupWorkerFaultState::<T>::exists() => {
          let Ok(now_tick) = Self::current_scheduler_tick() else {
            return false;
          };
          [WakeupClock::Block, WakeupClock::Tick]
            .into_iter()
            .filter_map(Self::wakeup_cursor_peek_key)
            .any(|key| match key {
              WakeupKey::Block(block) => block <= now,
              WakeupKey::Tick(tick) => tick <= now_tick,
            })
        }
        1 => {
          !CrossingWorkerFaultState::<T>::exists()
            && CrossingPendingFeedListState::<T>::get().count > 0
        }
        2 => {
          !ObservationFanoutWorkerFaultState::<T>::exists()
            && DirtyObservationListState::<T>::get().count > 0
        }
        _ => false,
      }
    }

    fn service_materialization_family(
      family: u8,
      now: BlockNumberFor<T>,
      remaining: Weight,
      wakeups: &mut WakeupDrainStats,
      crossing: &mut crate::crossing::CrossingWorkCounters,
      fanout_pages: &mut u32,
    ) -> Weight {
      match family {
        0 => {
          let mut meter = WeightMeter::with_limit(remaining);
          *wakeups = Self::drain_overdue_wakeups_cursor_resuming(now, &mut meter, *wakeups);
          meter.consumed()
        }
        1 => {
          let (consumed, updated) =
            Self::service_crossing_transitions_resuming(remaining, *crossing);
          *crossing = updated;
          consumed
        }
        2 => {
          let (consumed, updated) =
            Self::fanout_dirty_observations_with_pages(remaining, *fanout_pages);
          *fanout_pages = updated;
          consumed
        }
        _ => Weight::zero(),
      }
    }

    fn service_materialization_families(
      now: BlockNumberFor<T>,
      available: Weight,
    ) -> Option<Weight> {
      let family_cursor = MaterializationFamilyCursor::<T>::get();
      if family_cursor >= 3 {
        return None;
      }
      let shared_limit = Self::materialization_weight_limit();
      let mut remaining = Weight::from_parts(
        shared_limit.ref_time().min(available.ref_time()),
        shared_limit.proof_size().min(available.proof_size()),
      );
      let mut consumed_total = Weight::zero();
      let mut wakeups = WakeupDrainStats::default();
      let mut crossing = crate::crossing::CrossingWorkCounters::default();
      let mut fanout_pages = 0u32;
      let all_minimum_quanta = Self::materialization_family_minimum(0)
        .saturating_add(Self::materialization_family_minimum(1))
        .saturating_add(Self::materialization_family_minimum(2));
      let can_reserve_all_minima = all_minimum_quanta.all_lte(remaining);
      for offset in 0u8..3 {
        let family = family_cursor.saturating_add(offset) % 3;
        let family_budget = Self::materialization_family_budget(
          family_cursor,
          offset,
          remaining,
          can_reserve_all_minima,
        );
        let consumed = Self::service_materialization_family(
          family,
          now,
          family_budget,
          &mut wakeups,
          &mut crossing,
          &mut fanout_pages,
        );
        consumed_total = consumed_total.saturating_add(consumed);
        remaining = remaining.saturating_sub(consumed);
      }
      if !remaining.is_zero() && Self::materialization_family_has_work(family_cursor, now) {
        consumed_total = consumed_total.saturating_add(Self::service_materialization_family(
          family_cursor,
          now,
          remaining,
          &mut wakeups,
          &mut crossing,
          &mut fanout_pages,
        ));
      }
      MaterializationFamilyCursor::<T>::put(family_cursor.saturating_add(1) % 3);
      Some(consumed_total)
    }
  }

  impl<T: Config> Pallet<T> {
    fn execute_mandatory_prepass(now: BlockNumberFor<T>) -> Result<Weight, Error<T>> {
      ensure!(
        T::PrepassContext::context_ready(),
        Error::<T>::PrepassContextIncomplete
      );
      let control_weight = T::WeightInfo::scheduler_on_initialize_cutoff();
      if let Some(mut existing) = CurrentBlockResourceState::<T>::get()
        && existing.ensure_block(now).is_ok()
      {
        existing.halt_optional_actor_work();
        CurrentBlockResourceState::<T>::put(existing);
        return Err(Error::<T>::PrepassDuplicateOrStale);
      }

      let budget = T::BlockResourceBudget::get();
      let mut state = BlockResourceState::new(now);
      if state.begin_prepass().is_err() {
        state.halt_optional_actor_work();
        CurrentBlockResourceState::<T>::put(state);
        return Err(Error::<T>::ResourceProtocolFailed);
      }
      let cutoff = NextQueueTicket::<T>::get();
      let mut cutoff_reservation = state
        .reserve(
          budget.limits(),
          BlockResourceDomain::ActorControl,
          control_weight,
        )
        .map_err(|_| Error::<T>::ResourceProtocolFailed)?;
      PrepassExecutionCutoff::<T>::put((now, cutoff));
      state
        .settle(&mut cutoff_reservation, control_weight)
        .map_err(|_| Error::<T>::ResourceProtocolFailed)?;

      let cleanup_units = u32::from(
        QueueHead::<T>::get() < QueueTail::<T>::get()
          && Self::combined_queue_occupancy() >= u64::from(T::MaxQueueLength::get()),
      );
      let cleanup_weight = if cleanup_units > 0 {
        T::WeightInfo::scheduler_paged_tombstone_drain(cleanup_units)
      } else {
        Weight::zero()
      };
      if cleanup_units > 0 {
        let mut cleanup_reservation = state
          .reserve(
            budget.limits(),
            BlockResourceDomain::ActorControl,
            cleanup_weight,
          )
          .map_err(|_| Error::<T>::ResourceProtocolFailed)?;
        let _ = Self::paged_drain_tombstones(cutoff, 1);
        state
          .settle(&mut cleanup_reservation, cleanup_weight)
          .map_err(|_| Error::<T>::ResourceProtocolFailed)?;
      }

      let materialization_configured = T::WeightInfo::materialization_coordinator_base()
        .saturating_add(Self::materialization_weight_limit());
      let materialization_remaining = budget
        .limits()
        .actor_control()
        .checked_sub(&state.usage().actor_control_used())
        .ok_or(Error::<T>::ResourceProtocolFailed)?;
      let materialization_maximum = Weight::from_parts(
        materialization_configured
          .ref_time()
          .min(materialization_remaining.ref_time()),
        materialization_configured
          .proof_size()
          .min(materialization_remaining.proof_size()),
      );
      ensure!(
        T::WeightInfo::materialization_coordinator_base().all_lte(materialization_maximum),
        Error::<T>::ResourceProtocolFailed
      );
      let materialization_family_budget =
        materialization_maximum.saturating_sub(T::WeightInfo::materialization_coordinator_base());
      let mut materialization_reservation = state
        .reserve(
          budget.limits(),
          BlockResourceDomain::ActorControl,
          materialization_maximum,
        )
        .map_err(|_| Error::<T>::ResourceProtocolFailed)?;
      let materialization_weight =
        Self::service_materialization_families(now, materialization_family_budget)
          .ok_or(Error::<T>::ResourceProtocolFailed)?;
      let materialization_actual =
        T::WeightInfo::materialization_coordinator_base().saturating_add(materialization_weight);
      state
        .settle(&mut materialization_reservation, materialization_actual)
        .map_err(|_| Error::<T>::ResourceProtocolFailed)?;

      let control_remaining = budget
        .limits()
        .actor_control()
        .checked_sub(&state.usage().actor_control_used())
        .unwrap_or_else(Weight::zero);
      let prepass_limit = control_remaining
        .checked_add(&budget.limits().actor_base_turn())
        .unwrap_or_else(Weight::zero);
      let pass = Self::execute_cycle_to_cutoff_with_resources(
        prepass_limit,
        cutoff,
        &mut state,
        budget.limits(),
        BlockResourceDomain::ActorBaseEffect,
        control_remaining,
      );
      if state.open_external_phase().is_err() {
        state.halt_optional_actor_work();
      }
      CurrentBlockResourceState::<T>::put(state);
      Ok(
        control_weight
          .saturating_add(cleanup_weight)
          .saturating_add(materialization_actual)
          .saturating_add(pass.consumed),
      )
    }
  }

  #[pallet::hooks]
  impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    fn integrity_test() {
      assert!(
        T::MaxConsecutiveFailures::get() > 0,
        "MaxConsecutiveFailures must be non-zero for bounded Actor run lifetime"
      );
      assert!(
        contract_steps_bound_is_valid(T::MaxContractSteps::get()),
        "MaxContractSteps must be in 1..=255"
      );
      assert_eq!(
        T::MaxRetryAttempts::get(),
        10,
        "MaxRetryAttempts must equal the protocol-fixed bound"
      );
      assert!(
        T::MaxContractSteps::get()
          .checked_mul(T::MaxRetryAttempts::get())
          .is_some(),
        "plan and retry bounds must compose without u32 overflow"
      );
      let target_block_time = T::TargetBlockTime::get();
      assert!(target_block_time > 0, "TargetBlockTime must be non-zero");
      assert!(
        T::CadenceTickMillis::get() > 0,
        "CadenceTickMillis must be non-zero"
      );
      let cadence_tick_millis = T::CadenceTickMillis::get();
      let expected_temporal_horizon = 315_576_000_000u64.div_ceil(cadence_tick_millis);
      assert_eq!(
        T::MaxTemporalDelayTicks::get(),
        expected_temporal_horizon,
        "MaxTemporalDelayTicks must cover exactly ten Julian years"
      );
      let expected_horizon = 315_576_000u64.div_ceil(target_block_time);
      let configured_horizon: u64 = T::MaxExecutionDelayBlocks::get().saturated_into();
      assert_eq!(
        configured_horizon, expected_horizon,
        "MaxExecutionDelayBlocks must cover exactly ten Julian years"
      );
      assert_eq!(
        T::MaxOpeningSnapshotEntries::get(),
        T::MaxContractSteps::get()
          .checked_mul(2)
          .expect("validated plan bound fits u32"),
        "MaxOpeningSnapshotEntries must equal twice MaxContractSteps"
      );
      // Genesis asserts this too, but genesis runs once. Only this gate re-checks the bound after
      // a runtime upgrade, and `capture_opening_predicates` traps on `on_idle` if it ever breaks.
      assert_eq!(
        T::MaxOpeningPredicateResults::get(),
        T::MaxContractSteps::get()
          .checked_mul(T::MaxPredicatesPerStep::get())
          .expect("opening predicate-result bound must fit u32"),
        "MaxOpeningPredicateResults must equal MaxContractSteps * MaxPredicatesPerStep"
      );
      assert!(
        T::MinUserBalance::get() >= T::AssetOps::minimum_balance(T::FeeNativeAssetId::get()),
        "MinUserBalance must cover the fee-native asset minimum"
      );
      assert!(
        T::QueuePageSize::get() > 0,
        "QueuePageSize must be non-zero"
      );
      assert!(
        T::QueuePageSize::get() < T::MaxQueueLength::get(),
        "QueuePageSize must remain an intermediate I/O granularity"
      );
      assert!(
        T::ObservationPageSize::get() > 0,
        "ObservationPageSize must be non-zero"
      );
      assert!(
        T::CrossingPageSize::get() > 0,
        "CrossingPageSize must be non-zero"
      );
      assert!(
        T::MaxCrossingMembersPerFeed::get() > 0
          && T::MaxCrossingMembersPerFeed::get() <= T::MaxActiveActors::get(),
        "MaxCrossingMembersPerFeed must be non-zero and bounded by active capacity"
      );
      assert!(
        T::MaxUserCrossingMembersPerFeed::get() > 0
          && T::MaxUserCrossingMembersPerFeed::get() < T::MaxCrossingMembersPerFeed::get(),
        "MaxUserCrossingMembersPerFeed must leave positive System capacity"
      );
      assert!(
        T::MaxCrossingTransitionsPerFeed::get() > 0,
        "MaxCrossingTransitionsPerFeed must be non-zero"
      );
      assert!(
        T::MaxCrossingTransitionsPerBlock::get() > 0
          && T::MaxCrossingLeavesPerBlock::get() > 0
          && T::MaxCrossingPagesPerBlock::get() > 0
          && T::MaxCrossingActorsPerBlock::get() > 0,
        "Crossing worker component caps must be non-zero"
      );
      let crossing_limit = T::CrossingWorkerWeightLimit::get();
      assert!(
        crossing_limit.ref_time() > 0 && crossing_limit.proof_size() > 0,
        "Crossing worker Weight limit must be non-zero in both dimensions"
      );
      assert!(
        T::MaxQueueEntriesScannedPerBlock::get() > 0
          && T::MaxQueueEntriesScannedPerBlock::get() <= T::MaxQueueLength::get(),
        "queue scan ceiling must be independently bounded by physical capacity"
      );
      assert!(
        T::MaxObservationFanoutPagesPerBlock::get() > 0,
        "observation fanout page ceiling must be non-zero"
      );
      let fanout_limit = T::ObservationFanoutWeightLimit::get();
      assert!(
        fanout_limit.ref_time() > 0 && fanout_limit.proof_size() > 0,
        "observation fanout Weight limit must be non-zero in both dimensions"
      );
      let fanout_unit = T::WeightInfo::observation_fanout_base().saturating_add(
        Self::observation_fanout_ordinary_weight_upper()
          .max(T::WeightInfo::observation_fanout_terminal()),
      );
      assert!(
        fanout_unit.all_lte(fanout_limit),
        "positive observation fanout cap must admit one complete page unit"
      );
      let crossing_branch = T::WeightInfo::crossing_transition_unit()
        .max(T::WeightInfo::crossing_leaf_unit())
        .max(T::WeightInfo::crossing_page_unit())
        .max(T::WeightInfo::crossing_rearm_unit())
        .max(T::WeightInfo::crossing_rearm_pair_unit())
        .max(T::WeightInfo::crossing_coalesced_unit())
        .max(T::WeightInfo::crossing_coalesced_pair_unit())
        .max(T::WeightInfo::crossing_placed_unit())
        .max(T::WeightInfo::crossing_placed_pair_unit())
        .max(T::WeightInfo::crossing_skip_unit())
        .max(T::WeightInfo::crossing_skip_pair_unit())
        .max(T::WeightInfo::crossing_actor_unit());
      let crossing_unit = T::WeightInfo::crossing_worker_base()
        .saturating_add(T::WeightInfo::crossing_work_probe())
        .saturating_add(
          T::WeightInfo::crossing_fire_pair_probe()
            .max(T::WeightInfo::crossing_rearm_pair_probe())
            .max(T::WeightInfo::crossing_skip_pair_probe()),
        )
        .saturating_add(crossing_branch);
      assert!(
        crossing_unit.all_lte(crossing_limit),
        "positive Crossing cap must admit one complete maximum unit"
      );
      let wakeup_limit = T::WakeupWeightLimit::get();
      assert!(
        wakeup_limit.ref_time() > 0 && wakeup_limit.proof_size() > 0,
        "wakeup worker Weight limit must be non-zero in both dimensions"
      );
      let wakeup_unit = T::WeightInfo::scheduler_wakeup_cursor_worker_future()
        .saturating_add(Self::wakeup_cursor_drain_unit_weight_upper(true));
      assert!(
        wakeup_unit.all_lte(wakeup_limit),
        "positive wakeup cap must admit one complete maximum unit"
      );
      let minimum_quanta = Self::materialization_family_minimum(0)
        .saturating_add(Self::materialization_family_minimum(1))
        .saturating_add(Self::materialization_family_minimum(2));
      assert!(
        minimum_quanta.all_lte(Self::materialization_weight_limit()),
        "shared materialization envelope must admit one complete maximum unit from every family"
      );
      let actor_service = Self::guaranteed_actor_service_weight()
        .expect("configured housekeeping Weight must fit ActorOnIdleReserve");
      assert!(
        Self::close_cleanup_weight_upper().all_lte(actor_service),
        "one maximum automatic cleanup must fit GuaranteedActorServiceWeight"
      );
      T::GenesisSystemActors::integrity_test();
    }

    #[cfg(feature = "try-runtime")]
    fn try_state(_n: BlockNumberFor<T>) -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      Self::do_try_state()
    }

    fn on_initialize(_now: BlockNumberFor<T>) -> Weight {
      Weight::zero()
    }

    fn on_idle(now: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
      let reserved = T::ActorOnIdleReserve::get();
      let available = Weight::from_parts(
        remaining_weight.ref_time().min(reserved.ref_time()),
        remaining_weight.proof_size().min(reserved.proof_size()),
      );
      let (control_available, resource_state) = match CurrentBlockResourceState::<T>::get() {
        Some(state)
          if state.ensure_block(now).is_ok()
            && state.phase() == BlockResourcePhase::ExternalPhase
            && !state.optional_actor_work_halted() =>
        {
          (
            T::BlockResourceBudget::get()
              .limits()
              .actor_control()
              .checked_sub(&state.usage().actor_control_used())
              .map(|remaining| {
                Weight::from_parts(
                  available.ref_time().min(remaining.ref_time()),
                  available.proof_size().min(remaining.proof_size()),
                )
              })
              .unwrap_or_else(Weight::zero),
            Some(state),
          )
        }
        Some(_) => (Weight::zero(), None),
        None => (available, None),
      };
      let legacy_unmetered_materialization = resource_state.is_none();
      let base_weight = T::WeightInfo::scheduler_on_idle_base();
      let coordinator_weight = if legacy_unmetered_materialization {
        T::WeightInfo::materialization_coordinator_base()
      } else {
        Weight::zero()
      };
      let finalize_weight = T::WeightInfo::block_resource_finalize();
      let fixed_weight = base_weight
        .saturating_add(coordinator_weight)
        .saturating_add(finalize_weight);
      if !fixed_weight.all_lte(control_available) {
        return Weight::zero();
      }
      let mut control_authority = match resource_state {
        Some(mut state) => match state.reserve(
          T::BlockResourceBudget::get().limits(),
          BlockResourceDomain::ActorControl,
          control_available,
        ) {
          Ok(reservation) => Some((state, reservation)),
          Err(_) => {
            state.halt_optional_actor_work();
            CurrentBlockResourceState::<T>::put(state);
            return Weight::zero();
          }
        },
        None => None,
      };
      let breaker_active = GlobalCircuitBreaker::<T>::get();
      let after_base = control_available.saturating_sub(fixed_weight);
      let cleanup_units = u32::from(QueueHead::<T>::get() < QueueTail::<T>::get());
      let queue_cleanup_weight = T::WeightInfo::scheduler_paged_tombstone_drain(cleanup_units);
      let saturated_cleanup_weight = if legacy_unmetered_materialization
        && cleanup_units > 0
        && Self::combined_queue_occupancy() >= u64::from(T::MaxQueueLength::get())
        && queue_cleanup_weight.all_lte(after_base)
      {
        let cutoff = NextQueueTicket::<T>::get();
        // The probe reads queue topology and the head page before it can know whether anything is
        // drainable, so a scan that finds nothing still consumed that work. Charge the attempt
        // unconditionally rather than letting the empty outcome bill zero every block.
        let _ = Self::paged_drain_tombstones(cutoff, 1);
        queue_cleanup_weight
      } else {
        Weight::zero()
      };
      let remaining_after_cleanup = after_base.saturating_sub(saturated_cleanup_weight);
      let materialization_weight = if legacy_unmetered_materialization {
        let Some(consumed) = Self::service_materialization_families(now, remaining_after_cleanup)
        else {
          return fixed_weight.saturating_add(saturated_cleanup_weight);
        };
        consumed
      } else {
        Weight::zero()
      };
      let housekeeping_weight = fixed_weight
        .saturating_add(saturated_cleanup_weight)
        .saturating_add(materialization_weight);
      let remaining_after_housekeeping = available.saturating_sub(housekeeping_weight);
      Self::settle_on_idle_control(&mut control_authority, housekeeping_weight);
      if breaker_active {
        Self::finalize_empty_actor_drain(now);
        return housekeeping_weight;
      }
      let execution_cutoff = PrepassExecutionCutoff::<T>::get()
        .filter(|(cutoff_block, _)| *cutoff_block == now)
        .map(|(_, cutoff)| cutoff)
        .unwrap_or_else(NextQueueTicket::<T>::get);
      let pass = match CurrentBlockResourceState::<T>::get() {
        Some(mut state)
          if state.ensure_block(now).is_ok()
            && state.phase() == BlockResourcePhase::ExternalPhase
            && !state.optional_actor_work_halted() =>
        {
          let budget = T::BlockResourceBudget::get();
          if state.begin_drain().is_err() {
            state.halt_optional_actor_work();
            CurrentBlockResourceState::<T>::put(state);
            return housekeeping_weight;
          }
          let control_maximum = budget
            .limits()
            .actor_control()
            .checked_sub(&state.usage().actor_control_used())
            .unwrap_or_else(Weight::zero);
          let pass = Self::execute_cycle_to_cutoff_with_resources(
            remaining_after_housekeeping,
            execution_cutoff,
            &mut state,
            budget.limits(),
            BlockResourceDomain::ActorDrainEffect,
            control_maximum,
          );
          if state.finish_drain(budget, budget.fixed_envelope()).is_err() {
            state.halt_optional_actor_work();
          } else if let Ok(snapshot) = state.finalized_snapshot() {
            FinalizedBlockResourceTelemetry::<T>::put(snapshot);
          }
          CurrentBlockResourceState::<T>::put(state);
          pass
        }
        Some(mut state) => {
          state.halt_optional_actor_work();
          CurrentBlockResourceState::<T>::put(state);
          return housekeeping_weight;
        }
        None => Self::execute_cycle_to_cutoff(remaining_after_housekeeping, execution_cutoff),
      };
      Self::update_idle_starvation_state(now, pass.starved);
      housekeeping_weight.saturating_add(pass.consumed)
    }

    fn on_finalize(now: BlockNumberFor<T>) {
      let valid = CurrentBlockResourceState::<T>::take().is_some_and(|state| {
        state.ensure_block(now).is_ok()
          && state.phase() == BlockResourcePhase::Finalizable
          && state.outstanding_reservations() == 0
      }) && FinalizedBlockResourceTelemetry::<T>::get()
        .is_some_and(|snapshot| snapshot.block_number() == now);
      assert!(valid, "Actors block resource protocol did not finalize"); // deos-bypass: panic-owner — invalid phase/reservation/telemetry makes the authored block consensus-invalid; finalization tests cover every accepted marker.
    }
  }

  #[pallet::event]
  #[pallet::generate_deposit(pub(super) fn deposit_event)]
  pub enum Event<T: Config> {
    ActorCreated {
      actor_id: ActorId,
      owner: T::AccountId,
      actor_class: ActorClass,
      mutability: Mutability,
      sovereign_account: T::AccountId,
      initial_lifecycle: InitialLifecycle,
    },
    ActorActivated {
      actor_id: ActorId,
    },
    ActorDeactivated {
      actor_id: ActorId,
    },
    ActorPaused {
      actor_id: ActorId,
    },
    ActorResumed {
      actor_id: ActorId,
    },
    ActorClosed {
      actor_id: ActorId,
      reason: CloseReason,
    },
    CycleStarted {
      actor_id: ActorId,
      cycle_nonce: u64,
    },
    CycleSummary {
      actor_id: ActorId,
      cycle_nonce: u64,
      result: CycleResult,
      outcomes: OutcomeTotals,
    },
    CycleSuspended {
      actor_id: ActorId,
      cycle_nonce: u64,
      cursor: u32,
      reason: SuspensionReason,
      cumulative_outcomes: OutcomeTotals,
    },
    CycleContinued {
      actor_id: ActorId,
      cycle_nonce: u64,
      cursor: u32,
    },
    CycleCancelled {
      actor_id: ActorId,
      cycle_nonce: u64,
      reason: CancellationReason,
    },
    CycleStopped {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
    },
    StepSkipped {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      reason: StepSkippedReason,
    },
    StepFailed {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      retry_class: RetryClass,
      error: DispatchError,
    },
    TransferExecuted {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      asset: T::AssetId,
      amount: T::Balance,
      to: T::AccountId,
    },
    SplitTransferExecuted {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      asset: T::AssetId,
      total: T::Balance,
      distributed: T::Balance,
      retained: T::Balance,
      legs: u32,
      effective_legs: u32,
    },
    SwapExecuted {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      asset_in: T::AssetId,
      asset_out: T::AssetId,
      amount_in: T::Balance,
      amount_out: T::Balance,
    },
    BurnExecuted {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      asset: T::AssetId,
      amount: T::Balance,
    },
    MintExecuted {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      asset: T::AssetId,
      amount: T::Balance,
    },
    StakeExecuted {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      asset: T::AssetId,
      amount: T::Balance,
    },
    UnstakeExecuted {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      asset: T::AssetId,
      shares: T::Balance,
    },
    LiquidityDonated {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      asset_a: T::AssetId,
      asset_b: T::AssetId,
      max_amount_a: T::Balance,
      max_amount_b: T::Balance,
      amount_a: T::Balance,
      amount_b: T::Balance,
    },
    LiquidityAdded {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      asset_a: T::AssetId,
      asset_b: T::AssetId,
      amount_a: T::Balance,
      amount_b: T::Balance,
      lp_minted: T::Balance,
    },
    LiquidityRemoved {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      lp_asset: T::AssetId,
      lp_amount: T::Balance,
      asset_a: T::AssetId,
      asset_b: T::AssetId,
      amount_a: T::Balance,
      amount_b: T::Balance,
    },
    ContractUpdated {
      actor_id: ActorId,
    },
    ActiveActorLimitSet {
      old_limit: u32,
      new_limit: u32,
    },
    GlobalCircuitBreakerSet {
      paused: bool,
    },
    ActorFaultRecorded {
      fault_id: FaultId,
      kind: ActorFaultKind,
      first_recorded_block: BlockNumberFor<T>,
      context: FaultContext<T::ObservationFeedId, BlockNumberFor<T>>,
    },
    CrossingWorkerFaultCleared {
      feed: T::ObservationFeedId,
      revision: Option<ObservationRevision>,
      class: CrossingWorkerFaultClass,
    },
    ObservationFanoutWorkerFaultCleared {
      feed: T::ObservationFeedId,
      revision: ObservationRevision,
      subscriber_page: Option<u32>,
      class: CrossingWorkerFaultClass,
    },
    WakeupWorkerFaultCleared {
      key: WakeupKey<BlockNumberFor<T>>,
      page: WakeupPageId,
      class: CrossingWorkerFaultClass,
    },
    ManualTriggerSet {
      actor_id: ActorId,
    },
    TriggerOccurrenceProcessed {
      actor_id: ActorId,
      trigger_family: TriggerFamily,
      fee: BalanceOf<T>,
    },
    PipelineFeeCharged {
      actor_id: ActorId,
      fee: BalanceOf<T>,
    },
    ActionFeeCharged {
      actor_id: ActorId,
      cycle_nonce: u64,
      step_index: u32,
      actual_effect_weight: Weight,
      fee: BalanceOf<T>,
    },
    FundingAccumulated {
      actor_id: ActorId,
      asset: T::AssetId,
      added: BalanceOf<T>,
      accumulated: BalanceOf<T>,
    },
    SweepBatchProcessed {
      requested: u32,
      closed: u32,
      alive: u32,
      missing: u32,
    },
    IdleStarvationDetected {
      consecutive_blocks: u32,
    },
    IdleStarvationRecovered {
      consecutive_blocks: u32,
    },
  }

  #[pallet::error]
  pub enum Error<T> {
    ActorIdOverflow,
    ActorNotFound,
    ActiveActorCapacityExceeded,
    ActiveActorCountInvariant,
    ActorIdentityCapacityExceeded,
    ActorIdentityCountInvariant,
    ActorInvariant,
    ActorAlreadyActive,
    ActorDormant,
    ActiveActorLimitExceedsQueueCapacity,
    ActiveActorLimitTooHigh,
    ActiveActorLimitTooLow,
    ActiveActorLimitBelowCurrent,
    ActorPaused,
    ContractStepsExceedOnIdleBudget,
    ExecutionDelayTooLong,
    GlobalCircuitBreakerActive,
    ImmutableActor,
    InsufficientBalance,
    InsufficientFee,
    InvalidAmountResolution,
    InvalidPredicate,
    InvalidAutoCloseNonce,
    InvalidScheduleWindow,
    InvalidSplitTransfer,
    InvalidTriggerConfiguration,
    InvalidTradeBound,
    InvalidRetryAttemptLimit,
    InvalidObservationMaxAge,
    SelfTransferNotAllowed,
    MintNotAllowedForUserActor,
    NotGovernance,
    NotOwner,
    OwnerSlotCapacityExceeded,
    OwnerSlotOccupied,
    InvalidOwnerSlot,
    ActorIdOccupied,
    SystemSovereignCapacityExceeded,
    SystemSovereignUnknown,
    SystemSovereignOccupied,
    SystemSovereignInvariant,
    SovereignAccountCollision,
    ReservedSovereignAccount,
    TooManyContractSteps,
    SnapshotUnavailable,
    FundingAccumulatorOverflow,
    QueueTicketExhausted,
    SchedulerIndexExhausted,
    AutoCloseNonceHorizonExceeded,
    ControlMutationRateLimited,
    QueueCapacityUnavailable,
    RetryLaterNotAllowedForImmutableActor,
    ActorRunNotFound,
    ActorRunInvariant,
    ComputationOverflow,
    EmptyPrecondition,
    ManualSourceDisabled,
    RecipientDepositUnavailable,
    ObservationSubscriptionCapacityExceeded,
    ObservationSubscriptionInvariant,
    InvalidObservationRevision,
    DirtyObservationCapacityExceeded,
    DirtyObservationInvariant,
    ObservationUnavailable,
    ObservationUninitialized,
    CrossingIndexCapacityExceeded,
    CrossingUserCapacityExceeded,
    CrossingIndexInvariant,
    CrossingGenerationExhausted,
    CrossingTransitionCapacityExceeded,
    CrossingTransitionInvariant,
    CrossingWorkerFaultNotFound,
    ObservationFanoutWorkerFaultNotFound,
    WakeupWorkerFaultNotFound,
    SystemActorTopologyInvalid,
    AdmissionBoundOverflow,
    StateHoldUnavailable,
    StateHoldInvariant,
    StateHoldOverflow,
    PrepassDuplicateOrStale,
    ResourceProtocolFailed,
    PrepassContextIncomplete,
  }

  #[pallet::call]
  impl<T: Config> Pallet<T> {
    #[pallet::call_index(0)]
    #[pallet::weight(T::WeightInfo::create_user_actor().max(T::WeightInfo::create_user_actor_crossing_new_page()))]
    pub fn create_user_actor(
      origin: OriginFor<T>,
      mutability: Mutability,
      contract: Option<ActorContractOf<T>>,
    ) -> DispatchResult {
      let owner = ensure_signed(origin)?;
      Self::do_create_user_actor(owner, mutability, None, contract)
    }

    #[pallet::call_index(1)]
    #[pallet::weight(T::WeightInfo::create_user_actor_at_slot().max(T::WeightInfo::create_user_actor_crossing_new_page()))]
    pub fn create_user_actor_at_slot(
      origin: OriginFor<T>,
      owner_slot: u8,
      mutability: Mutability,
      contract: Option<ActorContractOf<T>>,
    ) -> DispatchResult {
      let owner = ensure_signed(origin)?;
      Self::do_create_user_actor(owner, mutability, Some(owner_slot), contract)
    }

    #[pallet::call_index(2)]
    #[pallet::weight(if contract.is_some() {
      T::WeightInfo::create_system_actor()
        .max(T::WeightInfo::create_user_actor_crossing_new_page())
    } else {
      T::WeightInfo::create_dormant_system_actor()
    })]
    pub fn create_system_actor(
      origin: OriginFor<T>,
      owner: T::AccountId,
      mutability: Mutability,
      contract: Option<ActorContractOf<T>>,
    ) -> DispatchResult {
      T::SystemOrigin::ensure_origin(origin)?;
      Self::do_create_system_actor(owner, mutability, contract, None)
    }

    #[pallet::call_index(3)]
    #[pallet::weight(T::WeightInfo::create_system_actor_at_sovereign_id().max(T::WeightInfo::create_user_actor_crossing_new_page()))]
    pub fn create_system_actor_at_sovereign_id(
      origin: OriginFor<T>,
      sovereign_id: SystemSovereignId,
      owner: T::AccountId,
      mutability: Mutability,
      contract: Option<ActorContractOf<T>>,
    ) -> DispatchResult {
      T::SystemOrigin::ensure_origin(origin)?;
      match SystemSovereigns::<T>::get(sovereign_id) {
        Some(SystemSovereignState::Vacant) => {}
        Some(SystemSovereignState::Occupied(_)) => {
          return Err(Error::<T>::SystemSovereignOccupied.into());
        }
        None => {
          return Err(Error::<T>::SystemSovereignUnknown.into());
        }
      }
      Self::do_create_system_actor(owner, mutability, contract, Some(sovereign_id))
    }

    #[pallet::call_index(4)]
    #[pallet::weight(T::WeightInfo::pause_actor().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn pause_actor(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      let state = Self::active_actor_state_for_control(actor_id)?;
      let continuation = state.run_state;
      let snapshot = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::expiry_substitution_due_loaded(&snapshot, continuation.as_ref())? {
        return Self::finalize_actor(actor_id, &snapshot, CloseReason::WindowExpired);
      }
      ensure!(
        snapshot.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      if snapshot.lifecycle.is_paused() {
        return Ok(());
      }
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_mutation_allowed(&snapshot, now)?;
      Self::with_control_transaction(|| {
        ActorHot::<T>::try_mutate(actor_id, |maybe| -> DispatchResult {
          let inst = maybe.as_mut().ok_or(Error::<T>::ActorNotFound)?;
          ensure!(
            snapshot.mutability == Mutability::Mutable,
            Error::<T>::ImmutableActor
          );
          inst.lifecycle = ActiveLifecycle::Paused;
          inst.queue_ticket = None;
          Self::record_control_mutation(actor_id, now)?;
          Self::deposit_event(Event::ActorPaused { actor_id });
          Ok(())
        })?;
        Self::prime_actor_schedule(actor_id).map_err(Self::placement_error)
      })
    }

    #[pallet::call_index(5)]
    #[pallet::weight(T::WeightInfo::resume_actor().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn resume_actor(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      let state = Self::active_actor_state_for_control(actor_id)?;
      let continuation = state.run_state;
      let snapshot = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::expiry_substitution_due_loaded(&snapshot, continuation.as_ref())? {
        return Self::finalize_actor(actor_id, &snapshot, CloseReason::WindowExpired);
      }
      ensure!(
        snapshot.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      if !snapshot.lifecycle.is_paused() {
        return Ok(());
      }
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_mutation_allowed(&snapshot, now)?;
      Self::with_control_transaction(|| {
        ActorHot::<T>::try_mutate(actor_id, |maybe| -> DispatchResult {
          let inst = maybe.as_mut().ok_or(Error::<T>::ActorNotFound)?;
          ensure!(
            snapshot.mutability == Mutability::Mutable,
            Error::<T>::ImmutableActor
          );
          inst.lifecycle = ActiveLifecycle::Active;
          Self::record_control_mutation(actor_id, now)?;
          Self::deposit_event(Event::ActorResumed { actor_id });
          Ok(())
        })?;
        Self::prime_actor_schedule(actor_id).map_err(Self::placement_error)
      })
    }

    #[pallet::call_index(6)]
    #[pallet::weight(T::WeightInfo::manual_trigger().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn manual_trigger(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResultWithPostInfo {
      let state = Self::active_actor_state_for_control(actor_id)?;
      let continuation = state.run_state;
      let snapshot = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::expiry_substitution_due_loaded(&snapshot, continuation.as_ref())? {
        Self::finalize_actor(actor_id, &snapshot, CloseReason::WindowExpired)?;
        return Ok(Pays::Yes.into());
      }
      ensure!(!snapshot.lifecycle.is_paused(), Error::<T>::ActorPaused);
      ensure!(
        snapshot.trigger.manual_source_enabled(),
        Error::<T>::ManualSourceDisabled
      );
      let actor_type = snapshot.actor_class.actor_type();
      let breakdown = Self::trigger_fee_for_weight(
        actor_type,
        TriggerFamily::Manual,
        T::WeightInfo::manual_trigger(),
      );
      let mut trigger_processed = false;
      Self::with_control_transaction(|| {
        let Some(outcome) = Self::commit_trigger_occurrence(
          actor_id,
          actor_type,
          &snapshot.sovereign_account,
          breakdown,
          TriggerCauseProvenance::ExternalPhase,
        )?
        else {
          return Ok(());
        };
        if matches!(outcome, crate::scheduler::ActivationOutcome::Latched) {
          Self::deposit_event(Event::ManualTriggerSet { actor_id });
        }
        trigger_processed = true;
        Ok(())
      })?;
      Ok(PostDispatchInfo {
        actual_weight: Some(T::WeightInfo::manual_trigger()),
        pays_fee: if actor_type == ActorType::User && trigger_processed {
          Pays::No
        } else {
          Pays::Yes
        },
      })
    }

    #[pallet::call_index(8)]
    #[pallet::weight(Pallet::<T>::close_dispatch_weight_upper())]
    pub fn close_actor(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      match Self::load_actor_state(actor_id) {
        LoadedActorStateOf::Active(state) => {
          let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
          Self::ensure_control_origin(origin, &instance)?;
          ensure!(
            instance.mutability == Mutability::Mutable,
            Error::<T>::ImmutableActor
          );
          Self::finalize_actor(actor_id, &instance, CloseReason::OwnerInitiated)
        }
        LoadedActorStateOf::Dormant(identity) => {
          Self::ensure_identity_control_origin(origin, &identity)?;
          Self::close_inactive_actor(actor_id, &identity, CloseReason::OwnerInitiated)
        }
        LoadedActorStateOf::NotRegistered => Err(Error::<T>::ActorNotFound.into()),
        LoadedActorStateOf::Corrupt => Err(Error::<T>::ActorInvariant.into()),
      }
    }

    #[pallet::call_index(9)]
    #[pallet::weight(T::WeightInfo::update_contract().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn update_contract(
      origin: OriginFor<T>,
      actor_id: ActorId,
      mut contract: ActorContractOf<T>,
    ) -> DispatchResult {
      Self::canonicalize_preconditions(&mut contract.steps)?;
      Self::validate_trigger(&contract.trigger, contract.cooldown_blocks)?;
      if let Some(ref window) = contract.window {
        Self::validate_schedule_window(window)?;
      }
      Self::validate_future_schedule_targets(&contract)?;
      let state = Self::active_actor_state_for_control(actor_id)?;
      let continuation = state.run_state;
      let snapshot = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      let current_contract =
        Self::load_actor_contract(actor_id).ok_or(Error::<T>::ActorInvariant)?;
      if current_contract == contract {
        return Ok(());
      }
      Self::ensure_retry_later_allowed(snapshot.mutability, &contract.steps)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::expiry_substitution_due_loaded(&snapshot, continuation.as_ref())? {
        return Self::finalize_actor(actor_id, &snapshot, CloseReason::WindowExpired);
      }
      ensure!(
        snapshot.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      let schedule_changed = current_contract.trigger != contract.trigger
        || current_contract.cooldown_blocks != contract.cooldown_blocks
        || current_contract.window != contract.window;
      let steps_changed = current_contract.steps != contract.steps;
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_mutation_allowed(&snapshot, now)?;
      Self::validate_contract_steps_shape(snapshot.actor_class.actor_type(), &contract.steps)?;
      if snapshot.actor_class.actor_type() == ActorType::System {
        T::SystemActorContractValidator::validate(actor_id, &contract)
          .map_err(|_| Error::<T>::SystemActorTopologyInvalid)?;
      }
      Self::validate_recipient_configuration(&contract.steps, &snapshot.sovereign_account)?;
      Self::validate_opening_snapshot_surfaces(&contract.steps)?;
      Self::ensure_contract_steps_fits_idle_budget(
        snapshot.actor_class.actor_type(),
        &contract.steps,
      )?;
      ensure!(
        (contract.steps.len() as u32) <= T::MaxContractSteps::get(),
        Error::<T>::TooManyContractSteps
      );
      if snapshot.actor_class.actor_type() == ActorType::User {
        ensure!(
          !Self::contract_steps_contains_mint(&contract.steps),
          Error::<T>::MintNotAllowedForUserActor
        );
      }
      if let Some(target_nonce) = contract.auto_close_at_cycle_nonce {
        Self::ensure_auto_close_target(snapshot.cycle_nonce, target_nonce)?;
      }
      let new_tracked = Self::derive_funding_tracked_assets(&contract.steps)?;
      let mut funding_state = ActorFunding::<T>::get(actor_id).ok_or(Error::<T>::ActorNotFound)?;
      funding_state.funding_tracked_assets = new_tracked.clone();
      funding_state
        .funding_accumulated
        .retain(|asset, _| new_tracked.contains(asset));
      // Every non-no-op Contract update rotates semantic and admission authority, so an open run
      // cannot remain bound to the replaced Contract even when only completion policy changes.
      let cancellation_reason = Some(CancellationReason::ContractReplaced);
      let schedule_anchor = Self::schedule_anchor_at(contract.window, now);
      let temporal_anchor_tick =
        Self::temporal_anchor_tick(&contract.trigger).map_err(Self::placement_error)?;
      let trigger_transition = schedule_changed
        .then(|| {
          Self::preflight_trigger_transition(
            actor_id,
            &contract.trigger,
            TriggerTransitionIntent::ReplaceActive,
          )
        })
        .transpose()?;
      Self::with_control_transaction(|| {
        let run_cancelled = if let Some(reason) = cancellation_reason {
          Self::cancel_run_internal(actor_id, reason, None)?
        } else {
          false
        };
        let crossing_state = if let Some(transition) = trigger_transition {
          Self::commit_trigger_transition(actor_id, transition, None)?
        } else {
          None
        };
        if schedule_changed
          && ActorHot::<T>::get(actor_id).is_some_and(|hot| hot.trigger_wakeup_pointer.is_some())
        {
          Self::trigger_wakeup_substrate_invalidate_inner(actor_id)
            .map_err(Self::placement_error)?;
        }
        ActorHot::<T>::try_mutate(actor_id, |maybe| -> DispatchResult {
          let hot = maybe.as_mut().ok_or(Error::<T>::ActorNotFound)?;
          if schedule_changed {
            hot.schedule_anchor = schedule_anchor;
            hot.trigger_runtime_state = Self::installed_trigger_runtime_state(
              &contract.trigger,
              temporal_anchor_tick,
              crossing_state,
            )?;
            hot.terminal_at = contract
              .window
              .map(|window| Self::window_terminal_at(&window));
          }
          if steps_changed {
            hot.unsuccessful_attempt_streak = crate::execution::transition_failure_streak(
              hot.unsuccessful_attempt_streak,
              crate::execution::FailureStreakTransition::Reset,
            )
            .ok_or(Error::<T>::ActorInvariant)?;
          }
          Self::record_control_mutation(actor_id, now)?;
          Ok(())
        })?;
        // Crossing compilation binds the newly installed runtime phase to the replacement
        // admission identity, so publish hot schedule authority before storing its Contract.
        Self::store_actor_contract(actor_id, contract.clone())?;
        ActorFunding::<T>::insert(actor_id, funding_state);
        Self::deposit_event(Event::ContractUpdated { actor_id });
        #[cfg(test)]
        crate::mock::control_atomicity_checkpoint(actor_id)?;
        if schedule_changed || run_cancelled {
          Self::prime_actor_schedule(actor_id).map_err(Self::placement_error)?;
        }
        Self::reconcile_actor_state_hold(actor_id)?;
        Ok(())
      })
    }

    #[pallet::call_index(10)]
    #[pallet::weight(T::WeightInfo::set_global_circuit_breaker())]
    pub fn set_global_circuit_breaker(origin: OriginFor<T>, paused: bool) -> DispatchResult {
      T::GlobalBreakerOrigin::ensure_origin(origin)?;
      GlobalCircuitBreaker::<T>::put(paused);
      Self::deposit_event(Event::GlobalCircuitBreakerSet { paused });
      Ok(())
    }

    /// Force lifecycle evaluation for a specific actor
    #[pallet::call_index(11)]
    #[pallet::weight(T::WeightInfo::permissionless_sweep().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn permissionless_sweep(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      let _who = ensure_signed(origin)?;
      Self::evaluate_actor_liveness(actor_id)
    }

    #[pallet::call_index(13)]
    #[pallet::weight(T::WeightInfo::set_active_actor_limit())]
    pub fn set_active_actor_limit(origin: OriginFor<T>, new_limit: u32) -> DispatchResult {
      T::SystemOrigin::ensure_origin(origin)?;
      ensure!(new_limit > 0, Error::<T>::ActiveActorLimitTooLow);
      ensure!(
        new_limit <= T::MaxActiveActors::get(),
        Error::<T>::ActiveActorLimitTooHigh
      );
      ensure!(
        new_limit <= T::MaxQueueLength::get(),
        Error::<T>::ActiveActorLimitExceedsQueueCapacity
      );
      let active_count = Self::active_instance_count();
      ensure!(
        new_limit >= active_count,
        Error::<T>::ActiveActorLimitBelowCurrent
      );
      let old_limit = Self::effective_active_actor_limit();
      if old_limit == new_limit {
        return Ok(());
      }
      ActiveActorLimit::<T>::put(new_limit);
      Self::deposit_event(Event::ActiveActorLimitSet {
        old_limit,
        new_limit,
      });
      Ok(())
    }

    #[pallet::call_index(14)]
    #[pallet::weight(
      T::WeightInfo::permissionless_sweep_many(actor_ids.len() as u32)
        .saturating_add(Pallet::<T>::close_dispatch_weight_upper().saturating_mul(actor_ids.len() as u64))
    )]
    pub fn permissionless_sweep_many(
      origin: OriginFor<T>,
      actor_ids: BoundedVec<ActorId, T::MaxSweepBatch>,
    ) -> DispatchResult {
      let _who = ensure_signed(origin)?;
      Self::with_control_transaction(|| {
        let mut closed = 0u32;
        let mut alive = 0u32;
        let mut missing = 0u32;
        let requested = actor_ids.len() as u32;
        for actor_id in actor_ids {
          let state = match Self::load_actor_state(actor_id) {
            LoadedActorStateOf::Active(state) => state,
            LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => {
              missing = missing.saturating_add(1);
              continue;
            }
            LoadedActorStateOf::Corrupt => return Err(Error::<T>::ActorInvariant.into()),
          };
          let continuation = state.run_state;
          let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
          if let Some(reason) = Self::classify_actor_loaded(&instance, continuation.as_ref())
            .map_err(Self::classification_dispatch_error)?
            .terminal_reason
          {
            Self::finalize_actor(actor_id, &instance, reason)?;
            closed = closed.saturating_add(1);
          } else {
            alive = alive.saturating_add(1);
          }
        }
        Self::deposit_event(Event::SweepBatchProcessed {
          requested,
          closed,
          alive,
          missing,
        });
        Ok(())
      })
    }

    #[pallet::call_index(17)]
    #[pallet::weight(T::WeightInfo::activate_actor())]
    pub fn activate_actor(
      origin: OriginFor<T>,
      actor_id: ActorId,
      contract: ActorContractOf<T>,
    ) -> DispatchResult {
      let identity = match Self::load_actor_state(actor_id) {
        LoadedActorStateOf::Dormant(identity) => identity,
        LoadedActorStateOf::Active(_) => return Err(Error::<T>::ActorAlreadyActive.into()),
        LoadedActorStateOf::NotRegistered => return Err(Error::<T>::ActorNotFound.into()),
        LoadedActorStateOf::Corrupt => return Err(Error::<T>::ActorInvariant.into()),
      };
      Self::ensure_identity_control_origin(origin, &identity)?;
      Self::do_activate_actor(actor_id, identity, contract)
    }

    #[pallet::call_index(18)]
    #[pallet::weight(T::WeightInfo::deactivate_actor())]
    pub fn deactivate_actor(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      let instance = match Self::load_actor_state(actor_id) {
        LoadedActorStateOf::Active(state) => {
          Self::derive_active_actor_view(state.identity, state.hot, state.contract)
        }
        LoadedActorStateOf::Dormant(_) => return Err(Error::<T>::ActorDormant.into()),
        LoadedActorStateOf::NotRegistered => return Err(Error::<T>::ActorNotFound.into()),
        LoadedActorStateOf::Corrupt => return Err(Error::<T>::ActorInvariant.into()),
      };
      Self::ensure_control_origin(origin, &instance)?;
      ensure!(
        instance.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      Self::ensure_control_mutation_allowed(&instance, frame_system::Pallet::<T>::block_number())?;
      Self::do_deactivate_actor(actor_id, instance)
    }

    #[pallet::call_index(20)]
    #[pallet::weight(T::WeightInfo::clear_crossing_worker_fault())]
    pub fn clear_crossing_worker_fault(origin: OriginFor<T>) -> DispatchResult {
      T::GlobalBreakerOrigin::ensure_origin(origin)?;
      let fault =
        CrossingWorkerFaultState::<T>::take().ok_or(Error::<T>::CrossingWorkerFaultNotFound)?;
      Self::deposit_event(Event::CrossingWorkerFaultCleared {
        feed: fault.feed,
        revision: fault.revision,
        class: fault.class,
      });
      Ok(())
    }

    #[pallet::call_index(21)]
    #[pallet::weight(T::WeightInfo::clear_observation_fanout_worker_fault())]
    pub fn clear_observation_fanout_worker_fault(origin: OriginFor<T>) -> DispatchResult {
      T::GlobalBreakerOrigin::ensure_origin(origin)?;
      let fault = ObservationFanoutWorkerFaultState::<T>::take()
        .ok_or(Error::<T>::ObservationFanoutWorkerFaultNotFound)?;
      Self::deposit_event(Event::ObservationFanoutWorkerFaultCleared {
        feed: fault.feed,
        revision: fault.revision,
        subscriber_page: fault.subscriber_page,
        class: fault.class,
      });
      Ok(())
    }

    #[pallet::call_index(22)]
    #[pallet::weight(T::WeightInfo::clear_wakeup_worker_fault())]
    pub fn clear_wakeup_worker_fault(origin: OriginFor<T>) -> DispatchResult {
      T::GlobalBreakerOrigin::ensure_origin(origin)?;
      let fault =
        WakeupWorkerFaultState::<T>::take().ok_or(Error::<T>::WakeupWorkerFaultNotFound)?;
      Self::deposit_event(Event::WakeupWorkerFaultCleared {
        key: fault.key,
        page: fault.page,
        class: fault.class,
      });
      Ok(())
    }

    #[pallet::call_index(19)]
    #[pallet::weight(T::WeightInfo::run_cancel())]
    pub fn cancel_run(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      let state = Self::active_actor_state_for_control(actor_id)?;
      let run_state = state.run_state;
      let instance = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
      Self::ensure_control_origin(origin, &instance)?;
      ensure!(
        instance.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      if Self::expiry_substitution_due_loaded(&instance, run_state.as_ref())? {
        return Self::finalize_actor(actor_id, &instance, CloseReason::WindowExpired);
      }
      ensure!(
        matches!(
          instance.cycle_state,
          CycleState::Running | CycleState::Suspended
        ),
        Error::<T>::ActorRunNotFound
      );
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_mutation_allowed(&instance, now)?;
      Self::with_control_transaction(|| {
        ensure!(
          Self::cancel_run_internal(actor_id, CancellationReason::Explicit, None)?,
          Error::<T>::ActorRunNotFound
        );
        Self::record_control_mutation(actor_id, now)?;
        Self::prime_actor_schedule(actor_id).map_err(Self::placement_error)
      })
    }

    #[pallet::call_index(23)]
    #[pallet::weight((
      T::BlockResourceBudget::get()
        .limits()
        .actor_control()
        .saturating_add(T::BlockResourceBudget::get().limits().actor_base_turn()),
      DispatchClass::Mandatory,
      Pays::No,
    ))]
    pub fn actor_prepass(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
      ensure_none(origin)?;
      let now = frame_system::Pallet::<T>::block_number();
      let actual_weight = Self::execute_mandatory_prepass(now)?;
      Ok(PostDispatchInfo {
        actual_weight: Some(actual_weight),
        pays_fee: Pays::No,
      })
    }
  }

  #[pallet::inherent]
  impl<T: Config> ProvideInherent for Pallet<T> {
    type Call = Call<T>;
    type Error = ActorPrepassInherentError;
    const INHERENT_IDENTIFIER: InherentIdentifier = ACTOR_PREPASS_INHERENT_IDENTIFIER;

    fn create_inherent(data: &InherentData) -> Option<Self::Call> {
      data
        .get_data::<ActorPrepassInherentData>(&Self::INHERENT_IDENTIFIER)
        .ok()
        .flatten()
        .filter(|provided| provided.version == ACTOR_PREPASS_INHERENT_VERSION)
        .map(|_| Call::actor_prepass {})
    }

    fn is_inherent_required(_data: &InherentData) -> Result<Option<Self::Error>, Self::Error> {
      Ok(Some(ActorPrepassInherentError::MissingCall))
    }

    fn check_inherent(call: &Self::Call, data: &InherentData) -> Result<(), Self::Error> {
      if !Self::is_inherent(call) {
        return Ok(());
      }
      match data.get_data::<ActorPrepassInherentData>(&Self::INHERENT_IDENTIFIER) {
        Ok(Some(provided)) if provided.version == ACTOR_PREPASS_INHERENT_VERSION => Ok(()),
        Ok(Some(_)) => Err(ActorPrepassInherentError::UnsupportedVersion),
        _ => Err(ActorPrepassInherentError::MissingData),
      }
    }

    fn is_inherent(call: &Self::Call) -> bool {
      matches!(call, Call::actor_prepass {})
    }
  }

  impl<T: Config> Pallet<T> {
    pub fn weight_upper_bound(task: &TaskOf<T>) -> Weight {
      // Runtime owns upper-bound pricing via coarse task classes to reduce calibration churn
      match task {
        ActorTask::Transfer { .. } => T::WeightInfo::task_transfer(),
        ActorTask::Burn { .. } => T::WeightInfo::task_burn(),
        ActorTask::Mint { .. } => T::WeightInfo::task_mint(),
        ActorTask::SplitTransfer { legs, .. } => {
          T::WeightInfo::task_split_transfer(legs.len() as u32)
        }
        ActorTask::SwapIn { .. } => T::WeightInfo::task_dex_exact_in(),
        ActorTask::SwapOut { .. } => T::WeightInfo::task_dex_exact_out(),
        ActorTask::AddLiquidity { .. } => T::WeightInfo::task_add_liquidity(),
        ActorTask::RemoveLiquidity { .. } => T::WeightInfo::task_remove_liquidity(),
        ActorTask::Stake { .. } => T::WeightInfo::task_stake(),
        ActorTask::DonateLiquidity { .. } => T::WeightInfo::task_donate_liquidity(),
        ActorTask::Unstake { .. } => T::WeightInfo::task_unstake(),
        ActorTask::StopCycle => T::WeightInfo::task_stop_cycle(),
      }
    }

    /// Conservative FRAME dispatch weight for explicit or lifecycle-touch pure cleanup.
    pub fn close_dispatch_weight_upper() -> Weight {
      Self::close_cleanup_weight_upper()
    }

    pub(crate) fn compute_cycle_weight_upper_from(
      actor_type: ActorType,
      contract_steps: &ContractSteps<T>,
      start_cursor: usize,
    ) -> Weight {
      let mut upper = T::WeightInfo::step_orchestration(contract_steps.len() as u32);
      for step_index in start_cursor..contract_steps.len() {
        let step = &contract_steps[step_index];
        let predicate_evaluation = Self::predicate_evaluation_weight(
          step
            .precondition
            .as_ref()
            .map_or(0, Precondition::evaluation_units),
        );
        upper = upper
          .saturating_add(predicate_evaluation)
          .saturating_add(Self::weight_upper_bound(&step.task));
        if actor_type == ActorType::User {
          upper = upper.saturating_add(T::WeightInfo::fee_collection());
        }
      }
      if (start_cursor..contract_steps.len()).any(|step_index| {
        contract_steps[step_index]
          .on_error
          .retry_max_attempts()
          .is_some()
      }) {
        upper = upper.saturating_add(
          T::WeightInfo::run_suspend()
            .max(T::WeightInfo::run_complete())
            .max(T::WeightInfo::run_cancel()),
        );
      }
      upper
    }

    pub fn compute_cycle_weight_upper(
      actor_type: ActorType,
      contract_steps: &ContractSteps<T>,
    ) -> Weight {
      Self::compute_cycle_weight_upper_from(actor_type, contract_steps, 0)
    }

    pub fn attempt_fee_envelope(
      actor_type: ActorType,
      contract_steps: &ContractSteps<T>,
      start_cursor: usize,
    ) -> Result<AttemptFeeEnvelopeOf<T>, Error<T>> {
      let mut inputs = BoundedVec::default();
      for step in contract_steps {
        let evaluation = Zero::zero();
        let execution =
          if actor_type == ActorType::User && !matches!(step.task, super::types::Task::StopCycle) {
            T::WeightToFee::weight_to_fee(&Self::weight_upper_bound(&step.task))
          } else {
            Zero::zero()
          };
        inputs
          .try_push(FeeEnvelopeInput {
            evaluation,
            execution,
          })
          .map_err(|_| Error::<T>::AdmissionBoundOverflow)?;
      }
      compose_attempt_fee_envelope(actor_type, &inputs, start_cursor).map_err(|error| match error {
        FeeEnvelopeError::CursorOutOfBounds | FeeEnvelopeError::ReservationUnderflow => {
          Error::<T>::ActorRunInvariant
        }
        FeeEnvelopeError::Overflow => Error::<T>::AdmissionBoundOverflow,
      })
    }

    #[cfg(test)]
    pub(crate) fn attempt_weight_upper_bound(
      instance: &ActiveActorViewOf<T>,
      start_cursor: usize,
    ) -> Weight {
      let mut upper = Self::compute_cycle_weight_upper_from(
        instance.actor_class.actor_type(),
        &instance.steps,
        start_cursor,
      );
      if instance.cycle_state == CycleState::Suspended {
        let suffix_steps = instance.steps.len().saturating_sub(start_cursor) as u32;
        // Retry and terminal transition touch the same bounded Actor run value. The transition
        // envelope already carries its maximum proof, so only incremental RefTime composes here.
        let retry = T::WeightInfo::run_retry();
        let suffix_admission = T::WeightInfo::run_suffix_admission(suffix_steps);
        upper = upper
          .saturating_add(Weight::from_parts(retry.ref_time(), 0))
          .saturating_add(Weight::from_parts(suffix_admission.ref_time(), 0));
      }
      upper
    }

    pub(crate) fn close_cycle_weight_upper_bound(_instance: &ActiveActorViewOf<T>) -> Weight {
      Self::close_cleanup_weight_upper()
    }

    /// Upper-bounds one prospective run plus pure terminal cleanup after the baseline scheduler
    /// envelope. Independently metered durable housekeeping may defer this work across blocks.
    pub fn contract_steps_admission_weight_upper(
      _actor_type: ActorType,
      contract_steps: &ContractSteps<T>,
    ) -> Weight {
      let maximum_step = if contract_steps.is_empty() {
        T::WeightInfo::scheduler_inner_zero_step_complete()
      } else {
        Self::derive_step_resource_envelopes(&ActorContract {
          trigger: Trigger::manual(),
          cooldown_blocks: Zero::zero(),
          window: None,
          steps: contract_steps.clone(),
          funding: FundingSourcePolicy::OwnerOnly,
          completion: CompletionPolicy::Persistent,
          auto_close_at_cycle_nonce: None,
        })
        .map(|resources| {
          resources
            .into_iter()
            .fold(Weight::zero(), |maximum, resource| {
              let current = resource.control.saturating_add(resource.effect);
              Weight::from_parts(
                maximum.ref_time().max(current.ref_time()),
                maximum.proof_size().max(current.proof_size()),
              )
            })
        })
        .unwrap_or(Weight::MAX)
      };
      Self::scheduler_admission_overhead()
        .saturating_add(maximum_step)
        .saturating_add(Self::close_cleanup_weight_upper())
    }

    fn finalize_empty_actor_drain(now: BlockNumberFor<T>) {
      let Some(mut state) = CurrentBlockResourceState::<T>::get() else {
        return;
      };
      let budget = T::BlockResourceBudget::get();
      if state.ensure_block(now).is_err()
        || state.begin_drain().is_err()
        || state.finish_drain(budget, budget.fixed_envelope()).is_err()
      {
        state.halt_optional_actor_work();
      } else if let Ok(snapshot) = state.finalized_snapshot() {
        FinalizedBlockResourceTelemetry::<T>::put(snapshot);
      }
      CurrentBlockResourceState::<T>::put(state);
    }

    fn settle_on_idle_control(
      authority: &mut Option<(
        BlockResourceState<BlockNumberFor<T>>,
        BlockResourceReservation,
      )>,
      actual: Weight,
    ) {
      let Some((mut state, mut reservation)) = authority.take() else {
        return;
      };
      if state.settle(&mut reservation, actual).is_err() {
        state.halt_optional_actor_work();
      }
      CurrentBlockResourceState::<T>::put(state);
    }

    pub fn materialization_weight_limit() -> Weight {
      T::WakeupWeightLimit::get()
        .saturating_add(T::CrossingWorkerWeightLimit::get())
        .saturating_add(T::ObservationFanoutWeightLimit::get())
    }

    pub(crate) fn materialization_family_budget(
      family_cursor: u8,
      offset: u8,
      remaining: Weight,
      reserve_all_minima: bool,
    ) -> Weight {
      if !reserve_all_minima {
        return remaining;
      }
      let reserved_for_later = ((offset + 1)..3).fold(Weight::zero(), |reserved, later| {
        reserved.saturating_add(Self::materialization_family_minimum(
          family_cursor.saturating_add(later) % 3,
        ))
      });
      remaining
        .checked_sub(&reserved_for_later)
        .unwrap_or_else(Weight::zero)
    }

    pub fn observation_fanout_ordinary_weight_upper() -> Weight {
      [
        T::WeightInfo::observation_fanout_page(),
        T::WeightInfo::observation_fanout_wakeup_page(),
        T::WeightInfo::observation_fanout_coalesced_page(),
        T::WeightInfo::observation_fanout_blocked_page(),
      ]
      .into_iter()
      .fold(Weight::zero(), |maximum, weight| {
        Weight::from_parts(
          maximum.ref_time().max(weight.ref_time()),
          maximum.proof_size().max(weight.proof_size()),
        )
      })
    }

    pub fn materialization_family_minimum(family: u8) -> Weight {
      match family {
        0 => T::WeightInfo::scheduler_wakeup_cursor_worker_future()
          .saturating_mul(2)
          .saturating_add(Self::wakeup_cursor_drain_unit_weight_upper(true)),
        1 => {
          let branch = T::WeightInfo::crossing_transition_unit()
            .max(T::WeightInfo::crossing_leaf_unit())
            .max(T::WeightInfo::crossing_page_unit())
            .max(T::WeightInfo::crossing_rearm_unit())
            .max(T::WeightInfo::crossing_rearm_pair_unit())
            .max(T::WeightInfo::crossing_coalesced_unit())
            .max(T::WeightInfo::crossing_coalesced_pair_unit())
            .max(T::WeightInfo::crossing_placed_unit())
            .max(T::WeightInfo::crossing_placed_pair_unit())
            .max(T::WeightInfo::crossing_skip_unit())
            .max(T::WeightInfo::crossing_skip_pair_unit())
            .max(T::WeightInfo::crossing_actor_unit());
          T::WeightInfo::crossing_worker_base()
            .saturating_add(T::WeightInfo::crossing_work_probe())
            .saturating_add(
              T::WeightInfo::crossing_fire_pair_probe()
                .max(T::WeightInfo::crossing_rearm_pair_probe())
                .max(T::WeightInfo::crossing_skip_pair_probe()),
            )
            .saturating_add(branch)
            .saturating_add(T::WeightInfo::record_crossing_worker_fault())
        }
        2 => T::WeightInfo::observation_fanout_base()
          .saturating_add(Self::observation_fanout_ordinary_weight_upper())
          .saturating_add(T::WeightInfo::record_observation_fanout_worker_fault()),
        _ => Weight::zero(),
      }
    }

    pub fn guaranteed_actor_service_weight() -> Option<Weight> {
      T::ActorOnIdleReserve::get()
        .checked_sub(&T::WeightInfo::scheduler_on_idle_base())
        .and_then(|remaining| {
          remaining.checked_sub(&T::WeightInfo::materialization_coordinator_base())
        })
        .and_then(|remaining| {
          remaining.checked_sub(&T::WeightInfo::scheduler_paged_tombstone_drain(1))
        })
        .and_then(|remaining| remaining.checked_sub(&Self::materialization_weight_limit()))
    }

    fn ensure_contract_steps_fits_idle_budget(
      actor_type: ActorType,
      contract_steps: &ContractSteps<T>,
    ) -> DispatchResult {
      let actor_service = Self::guaranteed_actor_service_weight()
        .ok_or(Error::<T>::ContractStepsExceedOnIdleBudget)?;
      ensure!(
        Self::contract_steps_admission_weight_upper(actor_type, contract_steps)
          .all_lte(actor_service),
        Error::<T>::ContractStepsExceedOnIdleBudget
      );
      if actor_type == ActorType::User {
        let resources = Self::derive_step_resource_envelopes(&ActorContract {
          trigger: Trigger::manual(),
          cooldown_blocks: Zero::zero(),
          window: None,
          steps: contract_steps.clone(),
          funding: FundingSourcePolicy::OwnerOnly,
          completion: CompletionPolicy::Persistent,
          auto_close_at_cycle_nonce: None,
        })
        .ok_or(Error::<T>::ActorRunInvariant)?;
        for resource in resources.iter(/* deos-bypass: bounded-iter */) {
          Self::maximum_current_step_fee(actor_type, *resource)?;
        }
      }
      Ok(())
    }

    fn owner_slot_bitmap_is_valid(bitmap: &OwnerSlotBitmap) -> bool {
      let max_slots = T::MaxOwnerSlots::get() as usize;
      if max_slots == 0 {
        return false;
      }
      let full_bytes = max_slots / 8;
      let remaining_bits = max_slots % 8;
      for index in 0..bitmap.len() {
        let byte = bitmap[index];
        let valid = if index < full_bytes {
          true
        } else if index == full_bytes && remaining_bits > 0 {
          byte & !((1u8 << remaining_bits) - 1) == 0
        } else {
          byte == 0
        };
        if !valid {
          return false;
        }
      }
      true
    }

    fn owner_slot_is_set(bitmap: &OwnerSlotBitmap, owner_slot: u8) -> bool {
      let byte = (owner_slot / 8) as usize;
      let bit = owner_slot % 8;
      bitmap[byte] & (1u8 << bit) != 0
    }

    fn set_owner_slot(bitmap: &mut OwnerSlotBitmap, owner_slot: u8) {
      let byte = (owner_slot / 8) as usize;
      let bit = owner_slot % 8;
      bitmap[byte] |= 1u8 << bit;
    }

    fn clear_owner_slot(bitmap: &mut OwnerSlotBitmap, owner_slot: u8) {
      let byte = (owner_slot / 8) as usize;
      let bit = owner_slot % 8;
      bitmap[byte] &= !(1u8 << bit);
    }

    fn owner_slot_bitmap_is_empty(bitmap: &OwnerSlotBitmap) -> bool {
      for byte in bitmap.as_slice() {
        if *byte != 0 {
          return false;
        }
      }
      true
    }

    fn state_hold_component(encoded_bytes: usize) -> Result<T::Balance, Error<T>> {
      if encoded_bytes == 0 {
        return Ok(T::Balance::zero());
      }
      let encoded_bytes =
        u32::try_from(encoded_bytes).map_err(|_| Error::<T>::StateHoldOverflow)?;
      let bytes: T::Balance = encoded_bytes.into();
      T::ActorStateHoldPerByte::get()
        .checked_mul(&bytes)
        .and_then(|priced_bytes| priced_bytes.checked_add(&T::ActorStateHoldBase::get()))
        .ok_or(Error::<T>::StateHoldOverflow)
    }

    fn state_hold_total(breakdown: &ActorStateHoldBreakdownOf<T>) -> Result<T::Balance, Error<T>> {
      [
        breakdown.identity,
        breakdown.contract_head,
        breakdown.contract_body,
        breakdown.detector,
        breakdown.funding,
        breakdown.run,
      ]
      .into_iter()
      .try_fold(T::Balance::zero(), |total, component| {
        total
          .checked_add(&component)
          .ok_or(Error::<T>::StateHoldOverflow)
      })
    }

    fn actor_state_hold_quote(
      actor_id: ActorId,
      actor_type: ActorType,
    ) -> Result<ActorStateHoldQuote<T::Balance>, ActorCostQuoteError> {
      let breakdown = if actor_type == ActorType::System {
        if ActorStateHolds::<T>::contains_key(actor_id) {
          return Err(ActorCostQuoteError::ActorInvariant);
        }
        ActorStateHoldBreakdown {
          identity: T::Balance::zero(),
          contract_head: T::Balance::zero(),
          contract_body: T::Balance::zero(),
          detector: T::Balance::zero(),
          funding: T::Balance::zero(),
          run: T::Balance::zero(),
        }
      } else {
        ActorStateHolds::<T>::get(actor_id)
          .ok_or(ActorCostQuoteError::ActorInvariant)?
          .breakdown
      };
      let total =
        Self::state_hold_total(&breakdown).map_err(|_| ActorCostQuoteError::ComputationOverflow)?;
      Ok(ActorStateHoldQuote {
        exempt: actor_type == ActorType::System,
        base_per_component: T::ActorStateHoldBase::get(),
        per_encoded_byte: T::ActorStateHoldPerByte::get(),
        breakdown,
        total,
      })
    }

    fn add_state_hold_encoded_size<Value: codec::Encode>(
      total: &mut usize,
      value: &Value,
    ) -> Result<(), Error<T>> {
      *total = total
        .checked_add(codec::Encode::encoded_size(value))
        .ok_or(Error::<T>::StateHoldOverflow)?;
      Ok(())
    }

    fn derive_actor_state_hold(
      actor_id: ActorId,
      identity: &ActorIdentityOf<T>,
    ) -> Result<ActorStateHoldBreakdownOf<T>, Error<T>> {
      let mut identity_bytes = 0usize;
      Self::add_state_hold_encoded_size(&mut identity_bytes, &actor_id)?;
      Self::add_state_hold_encoded_size(&mut identity_bytes, identity)?;
      Self::add_state_hold_encoded_size(&mut identity_bytes, &identity.sovereign_account)?;

      let mut breakdown = ActorStateHoldBreakdown {
        identity: Self::state_hold_component(identity_bytes)?,
        contract_head: T::Balance::zero(),
        contract_body: T::Balance::zero(),
        detector: T::Balance::zero(),
        funding: T::Balance::zero(),
        run: T::Balance::zero(),
      };
      if identity.actor_class.actor_type() == ActorType::System {
        return Ok(ActorStateHoldBreakdown {
          identity: T::Balance::zero(),
          ..breakdown
        });
      }

      let Some(hot) = ActorHot::<T>::get(actor_id) else {
        ensure!(
          !ActorContractHeads::<T>::contains_key(actor_id)
            && !ActorAdmissionCertificates::<T>::contains_key(actor_id)
            && !ActorFunding::<T>::contains_key(actor_id)
            && !ActorRunStateStore::<T>::contains_key(actor_id),
          Error::<T>::StateHoldInvariant
        );
        return Ok(breakdown);
      };
      let head = ActorContractHeads::<T>::get(actor_id).ok_or(Error::<T>::StateHoldInvariant)?;
      let admission =
        ActorAdmissionCertificates::<T>::get(actor_id).ok_or(Error::<T>::StateHoldInvariant)?;
      let funding = ActorFunding::<T>::get(actor_id).ok_or(Error::<T>::StateHoldInvariant)?;

      let mut head_bytes = <ActorHotStateOf<T> as codec::MaxEncodedLen>::max_encoded_len();
      Self::add_state_hold_encoded_size(&mut head_bytes, &head)?;
      Self::add_state_hold_encoded_size(&mut head_bytes, &admission)?;
      breakdown.contract_head = Self::state_hold_component(head_bytes)?;

      let chunk_count = head
        .header
        .step_count
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK);
      let mut body_bytes = 0usize;
      for chunk_index in 0..chunk_count {
        let chunk = ActorContractTailChunks::<T>::get(actor_id, chunk_index)
          .ok_or(Error::<T>::StateHoldInvariant)?;
        Self::add_state_hold_encoded_size(&mut body_bytes, &chunk)?;
      }
      breakdown.contract_body = Self::state_hold_component(body_bytes)?;

      let mut detector_bytes = 0usize;
      let activation = ActorActivationAuthorities::<T>::get(actor_id);
      if let Some(activation) = &activation {
        Self::add_state_hold_encoded_size(&mut detector_bytes, activation)?;
      }
      if let Some(feeds) = ActorObservationFeeds::<T>::get(actor_id) {
        Self::add_state_hold_encoded_size(&mut detector_bytes, &feeds)?;
      }
      if let Some(slot) = ObservationSubscriptionSlot::<T>::get(actor_id) {
        Self::add_state_hold_encoded_size(&mut detector_bytes, &slot)?;
      }
      let crossing_locator = CrossingMemberships::<T>::get(actor_id);
      if let Some(locator) = &crossing_locator {
        Self::add_state_hold_encoded_size(&mut detector_bytes, locator)?;
      }
      if activation.is_some() || crossing_locator.is_some() {
        Self::add_state_hold_encoded_size(&mut detector_bytes, &())?;
      }
      if let Some(pointer) = hot.trigger_wakeup_pointer {
        Self::add_state_hold_encoded_size(&mut detector_bytes, &pointer)?;
      }
      breakdown.detector = Self::state_hold_component(detector_bytes)?;
      breakdown.funding = Self::state_hold_component(codec::Encode::encoded_size(&funding))?;
      breakdown.run = Self::state_hold_component(
        <ActorRunStateOf<T> as codec::MaxEncodedLen>::max_encoded_len(),
      )?;
      Ok(breakdown)
    }

    pub(crate) fn ensure_funding_state_hold_capacity(
      actor_id: ActorId,
      identity: &ActorIdentityOf<T>,
      prospective_funding: &ActorFundingStateOf<T>,
    ) -> DispatchResult {
      if identity.actor_class.actor_type() == ActorType::System {
        return Ok(());
      }
      let existing = ActorStateHolds::<T>::get(actor_id).ok_or(Error::<T>::StateHoldInvariant)?;
      ensure!(
        existing.owner == identity.owner,
        Error::<T>::StateHoldInvariant
      );
      let prospective =
        Self::state_hold_component(codec::Encode::encoded_size(prospective_funding))?;
      if prospective > existing.breakdown.funding {
        let increase = prospective
          .checked_sub(&existing.breakdown.funding)
          .ok_or(Error::<T>::StateHoldOverflow)?;
        let reason: T::RuntimeHoldReason = HoldReason::ActorState.into();
        T::StateHoldCurrency::ensure_can_hold(&reason, &identity.owner, increase)
          .map_err(|_| Error::<T>::StateHoldUnavailable)?;
      }
      Ok(())
    }

    pub(crate) fn reconcile_actor_state_hold(actor_id: ActorId) -> DispatchResult {
      let existing = ActorStateHolds::<T>::get(actor_id);
      let target = match ActorIdentities::<T>::get(actor_id) {
        Some(identity) if identity.actor_class.actor_type() == ActorType::User => Some((
          identity.owner.clone(),
          Self::derive_actor_state_hold(actor_id, &identity)?,
        )),
        Some(_) | None => None,
      };
      let (owner, target_breakdown) = match (existing.as_ref(), target) {
        (Some(existing), Some((owner, breakdown))) => {
          ensure!(existing.owner == owner, Error::<T>::StateHoldInvariant);
          (owner, breakdown)
        }
        (None, Some(target)) => target,
        (Some(existing), None) => (
          existing.owner.clone(),
          ActorStateHoldBreakdown {
            identity: T::Balance::zero(),
            contract_head: T::Balance::zero(),
            contract_body: T::Balance::zero(),
            detector: T::Balance::zero(),
            funding: T::Balance::zero(),
            run: T::Balance::zero(),
          },
        ),
        (None, None) => return Ok(()),
      };
      if existing
        .as_ref()
        .is_some_and(|record| record.owner == owner && record.breakdown == target_breakdown)
      {
        return Ok(());
      }
      let old_total = existing
        .as_ref()
        .map(|record| Self::state_hold_total(&record.breakdown))
        .transpose()?
        .unwrap_or_else(T::Balance::zero);
      let target_total = Self::state_hold_total(&target_breakdown)?;
      let reason: T::RuntimeHoldReason = HoldReason::ActorState.into();
      if target_total > old_total {
        let increase = target_total
          .checked_sub(&old_total)
          .ok_or(Error::<T>::StateHoldOverflow)?;
        T::StateHoldCurrency::hold(&reason, &owner, increase)
          .map_err(|_| Error::<T>::StateHoldUnavailable)?;
      } else if old_total > target_total {
        let decrease = old_total
          .checked_sub(&target_total)
          .ok_or(Error::<T>::StateHoldOverflow)?;
        let released = T::StateHoldCurrency::release(&reason, &owner, decrease, Precision::Exact)
          .map_err(|_| Error::<T>::StateHoldInvariant)?;
        ensure!(released == decrease, Error::<T>::StateHoldInvariant);
      }
      if target_total.is_zero() {
        ActorStateHolds::<T>::remove(actor_id);
      } else {
        ActorStateHolds::<T>::insert(
          actor_id,
          ActorStateHoldRecord {
            owner,
            breakdown: target_breakdown,
          },
        );
      }
      Ok(())
    }

    fn charge_creation_fee(owner: &T::AccountId) -> DispatchResult {
      let creation_fee = T::ActorCreationFee::get();
      if creation_fee.is_zero() {
        return Ok(());
      }
      let native = T::FeeNativeAssetId::get();
      let fee_sink = T::FeeSink::get();
      T::FeeCollector::collect_fee(owner, &fee_sink, native, creation_fee)
        .map_err(|_| Error::<T>::InsufficientFee.into())
    }

    fn ensure_trigger_occurrence_capacity(
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      breakdown: TriggerFeeBreakdown<T::Balance>,
    ) -> DispatchResult {
      ensure!(
        Self::trigger_occurrence_capacity_sufficient(actor_type, sovereign_account, breakdown,)?,
        Error::<T>::InsufficientFee
      );
      Ok(())
    }

    pub(crate) fn trigger_occurrence_capacity_sufficient(
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      breakdown: TriggerFeeBreakdown<T::Balance>,
    ) -> Result<bool, Error<T>> {
      if actor_type == ActorType::System || breakdown.trigger_fee.is_zero() {
        return Ok(true);
      }
      let required = T::MinUserBalance::get()
        .checked_add(&breakdown.trigger_fee)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      Ok(T::AssetOps::balance(sovereign_account, T::FeeNativeAssetId::get()) >= required)
    }

    fn charge_trigger_occurrence(
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      breakdown: TriggerFeeBreakdown<T::Balance>,
    ) -> DispatchResult {
      Self::ensure_trigger_occurrence_capacity(actor_type, sovereign_account, breakdown)?;
      if actor_type == ActorType::System || breakdown.trigger_fee.is_zero() {
        return Ok(());
      }
      let native = T::FeeNativeAssetId::get();
      let fee_sink = T::FeeSink::get();
      T::FeeCollector::collect_fee(sovereign_account, &fee_sink, native, breakdown.trigger_fee)
        .map_err(|_| Error::<T>::InsufficientFee.into())
    }

    pub(crate) fn commit_trigger_occurrence(
      actor_id: ActorId,
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      breakdown: TriggerFeeBreakdown<T::Balance>,
      _cause_provenance: TriggerCauseProvenance,
    ) -> Result<Option<crate::scheduler::ActivationOutcome>, DispatchError> {
      if ActorHot::<T>::get(actor_id).is_some_and(|hot| hot.pending_signal) {
        return Ok(None);
      }
      Self::ensure_trigger_occurrence_capacity(actor_type, sovereign_account, breakdown)?;
      let outcome = Self::request_activation(actor_id).map_err(Self::activation_failure_error)?;
      if matches!(outcome, crate::scheduler::ActivationOutcome::Closed) {
        return Ok(None);
      }
      Self::charge_trigger_occurrence(actor_type, sovereign_account, breakdown)?;
      Self::deposit_event(Event::TriggerOccurrenceProcessed {
        actor_id,
        trigger_family: breakdown.trigger_family,
        fee: breakdown.trigger_fee,
      });
      Ok(Some(outcome))
    }

    pub(crate) fn try_commit_automatic_trigger_occurrence(
      actor_id: ActorId,
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      breakdown: TriggerFeeBreakdown<T::Balance>,
      cause_provenance: TriggerCauseProvenance,
    ) -> Result<Option<crate::scheduler::ActivationOutcome>, DispatchError> {
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Self::commit_trigger_occurrence(
          actor_id,
          actor_type,
          sovereign_account,
          breakdown,
          cause_provenance,
        ) {
          Ok(outcome) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(outcome))
          }
          Err(error) if error == Error::<T>::InsufficientFee.into() => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Ok(None))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    pub(crate) fn try_charge_prechecked_automatic_trigger_occurrence(
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      breakdown: TriggerFeeBreakdown<T::Balance>,
    ) -> Result<bool, DispatchError> {
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        if actor_type == ActorType::System || breakdown.trigger_fee.is_zero() {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(true));
        }
        let result = T::FeeCollector::collect_fee(
          sovereign_account,
          &T::FeeSink::get(),
          T::FeeNativeAssetId::get(),
          breakdown.trigger_fee,
        );
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(true)),
          Err(_) => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Ok(false)),
        }
      })
    }

    pub(crate) fn try_charge_automatic_trigger_occurrence(
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      breakdown: TriggerFeeBreakdown<T::Balance>,
    ) -> Result<bool, DispatchError> {
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Self::charge_trigger_occurrence(actor_type, sovereign_account, breakdown) {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(true)),
          Err(error) if error == Error::<T>::InsufficientFee.into() => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Ok(false))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    pub(crate) fn pipeline_fee_for_actor(
      actor_id: ActorId,
      actor_type: ActorType,
    ) -> Result<PipelineFeeBreakdown<T::Balance>, Error<T>> {
      let head = ActorContractHeads::<T>::get(actor_id).ok_or(Error::<T>::ActorInvariant)?;
      Self::pipeline_fee_breakdown(actor_type, head.header.pipeline_machine_envelope)
    }

    pub(crate) fn pipeline_capacity_sufficient(
      actor_id: ActorId,
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
    ) -> Result<bool, Error<T>> {
      if actor_type == ActorType::System {
        return Ok(true);
      }
      let breakdown = Self::pipeline_fee_for_actor(actor_id, actor_type)?;
      let required = T::MinUserBalance::get()
        .checked_add(&breakdown.total_fee)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      Ok(T::AssetOps::balance(sovereign_account, T::FeeNativeAssetId::get()) >= required)
    }

    pub(crate) fn action_capacity_sufficient(
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      step: &StepOf<T>,
      resources: ActorStepResourceEnvelope,
    ) -> Result<bool, Error<T>> {
      if actor_type == ActorType::System {
        return Ok(true);
      }
      let fee = Self::maximum_current_action_fee(actor_type, step, resources)?;
      if fee.total_fee.is_zero() {
        return Ok(true);
      }
      let required = T::MinUserBalance::get()
        .checked_add(&fee.total_fee)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      Ok(T::AssetOps::balance(sovereign_account, T::FeeNativeAssetId::get()) >= required)
    }

    pub(crate) fn collect_pipeline_fee(
      actor_id: ActorId,
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
    ) -> Result<PipelineFeeBreakdown<T::Balance>, DispatchError> {
      let breakdown = Self::pipeline_fee_for_actor(actor_id, actor_type)?;
      if actor_type == ActorType::System || breakdown.total_fee.is_zero() {
        return Ok(breakdown);
      }
      ensure!(
        Self::pipeline_capacity_sufficient(actor_id, actor_type, sovereign_account)?,
        Error::<T>::InsufficientFee
      );
      let native = T::FeeNativeAssetId::get();
      T::FeeCollector::collect_fee(
        sovereign_account,
        &T::FeeSink::get(),
        native,
        breakdown.total_fee,
      )
      .map_err(|_| Error::<T>::InsufficientFee)?;
      Ok(breakdown)
    }

    #[cfg(test)]
    pub(crate) fn maximum_contract_step_fee(
      actor_type: ActorType,
      contract_steps: &ContractSteps<T>,
      cursor: usize,
    ) -> Result<StepFeeBreakdown<T::Balance>, Error<T>> {
      let resources = Self::derive_step_resource_envelopes(&ActorContract {
        trigger: Trigger::manual(),
        cooldown_blocks: Zero::zero(),
        window: None,
        steps: contract_steps.clone(),
        funding: FundingSourcePolicy::OwnerOnly,
        completion: CompletionPolicy::Persistent,
        auto_close_at_cycle_nonce: None,
      })
      .and_then(|resources| resources.get(cursor).copied())
      .ok_or(Error::<T>::ActorRunInvariant)?;
      let step = contract_steps
        .get(cursor)
        .ok_or(Error::<T>::ActorRunInvariant)?;
      Self::maximum_current_action_fee(actor_type, step, resources)
    }

    /// Returns ledger minimum plus the generated Pipeline Machine and cleanup charge.
    /// Trigger-family pricing is composed separately at ready Opening.
    pub fn user_pipeline_machine_capacity_requirement(
      contract_steps: &ContractSteps<T>,
    ) -> Result<BalanceOf<T>, Error<T>> {
      let contract = ActorContract {
        trigger: Trigger::manual(),
        cooldown_blocks: Zero::zero(),
        window: None,
        steps: contract_steps.clone(),
        funding: FundingSourcePolicy::OwnerOnly,
        completion: CompletionPolicy::Persistent,
        auto_close_at_cycle_nonce: None,
      };
      let resources =
        Self::derive_step_resource_envelopes(&contract).ok_or(Error::<T>::ActorRunInvariant)?;
      let envelope =
        Self::derive_pipeline_machine_envelope(ActorType::User, contract_steps, &resources)?;
      envelope
        .pipeline_machine_fee_upper
        .checked_add(&envelope.cleanup_fee_upper)
        .and_then(|cycle_requirement| T::MinUserBalance::get().checked_add(&cycle_requirement))
        .ok_or(Error::<T>::AdmissionBoundOverflow)
    }

    pub fn sovereign_account_id(owner: &T::AccountId, owner_slot: u8) -> T::AccountId {
      T::SovereignAccountDeriver::user(T::PalletId::get(), owner, owner_slot)
    }

    pub fn sovereign_account_id_system(actor_id: ActorId) -> T::AccountId {
      T::SovereignAccountDeriver::system(T::PalletId::get(), actor_id)
    }

    pub(crate) fn available_owner_slot(
      owner: &T::AccountId,
      preferred_slot: Option<u8>,
    ) -> Result<u8, Error<T>> {
      let bitmap = OwnerSlotBitmaps::<T>::get(owner);
      let max_slots = T::MaxOwnerSlots::get();
      ensure!(max_slots > 0, Error::<T>::InvalidOwnerSlot);
      ensure!(
        Self::owner_slot_bitmap_is_valid(&bitmap),
        Error::<T>::InvalidOwnerSlot
      );
      match preferred_slot {
        Some(slot) => {
          ensure!(slot < max_slots, Error::<T>::InvalidOwnerSlot);
          ensure!(
            !Self::owner_slot_is_set(&bitmap, slot),
            Error::<T>::OwnerSlotOccupied
          );
          Ok(slot)
        }
        None => {
          for byte_index in 0..bitmap.len() {
            let byte = bitmap[byte_index];
            let first_slot = byte_index * 8;
            if first_slot >= max_slots as usize {
              break;
            }
            let remaining = (max_slots as usize).saturating_sub(first_slot);
            let valid_bits = if remaining >= 8 {
              u8::MAX
            } else {
              (1u8 << remaining) - 1
            };
            let free_bits = !byte & valid_bits;
            if free_bits != 0 {
              return Ok((first_slot + free_bits.trailing_zeros() as usize) as u8);
            }
          }
          Err(Error::<T>::OwnerSlotCapacityExceeded)
        }
      }
    }

    fn allocate_owner_slot(
      owner: &T::AccountId,
      preferred_slot: Option<u8>,
    ) -> Result<(u8, T::AccountId), Error<T>> {
      let mut bitmap = OwnerSlotBitmaps::<T>::get(owner);
      let owner_slot = Self::available_owner_slot(owner, preferred_slot)?;
      let sovereign_account = Self::sovereign_account_id(owner, owner_slot);
      if T::SovereignAccountPolicy::is_reserved(&sovereign_account) {
        return Err(Error::<T>::ReservedSovereignAccount);
      }
      if SovereignIndex::<T>::contains_key(&sovereign_account) {
        return Err(Error::<T>::SovereignAccountCollision);
      }
      Self::set_owner_slot(&mut bitmap, owner_slot);
      OwnerSlotBitmaps::<T>::insert(owner, bitmap);
      Ok((owner_slot, sovereign_account))
    }

    fn allocate_system_sovereign(actor_id: ActorId) -> Result<T::AccountId, Error<T>> {
      let sovereign_account = Self::sovereign_account_id_system(actor_id);
      // Context-aware reservation: a fresh (unregistered) derivation that aliases a
      // host-reserved account fails ReservedSovereignAccount; reattachment to an
      // existing registered Vacant locator is allowed for that exact locator even
      // when its account belongs to the genesis System custody range, so the locator
      // is not permanently unrecoverable after close (spec 5.4).
      let is_registered_reattachment =
        SystemSovereigns::<T>::get(actor_id) == Some(SystemSovereignState::Vacant);
      if !is_registered_reattachment && T::SovereignAccountPolicy::is_reserved(&sovereign_account) {
        return Err(Error::<T>::ReservedSovereignAccount);
      }
      if SovereignIndex::<T>::contains_key(&sovereign_account) {
        return Err(Error::<T>::SovereignAccountCollision);
      }
      Ok(sovereign_account)
    }

    fn do_create_dormant_actor(
      owner: T::AccountId,
      actor_type: ActorType,
      preferred_user_slot: Option<u8>,
      requested_system_sovereign_id: Option<SystemSovereignId>,
    ) -> DispatchResult {
      ensure!(
        !GlobalCircuitBreaker::<T>::get(),
        Error::<T>::GlobalCircuitBreakerActive
      );
      ensure!(
        ActorIdentityCount::<T>::get() < T::MaxActorIdentities::get(),
        Error::<T>::ActorIdentityCapacityExceeded
      );
      let actor_id = NextActorId::<T>::get();
      ensure!(
        !Self::active_actor_exists(actor_id) && !ActorIdentities::<T>::contains_key(actor_id),
        Error::<T>::ActorIdOccupied
      );
      let next_id = actor_id.checked_add(1).ok_or(Error::<T>::ActorIdOverflow)?;
      let system_sovereign_id = requested_system_sovereign_id.unwrap_or(actor_id);
      if actor_type == ActorType::System {
        match requested_system_sovereign_id {
          Some(_) => match SystemSovereigns::<T>::get(system_sovereign_id) {
            Some(SystemSovereignState::Vacant) => {}
            Some(SystemSovereignState::Occupied(_)) => {
              return Err(Error::<T>::SystemSovereignOccupied.into());
            }
            None => {
              return Err(Error::<T>::SystemSovereignUnknown.into());
            }
          },
          None => {
            ensure!(
              !SystemSovereigns::<T>::contains_key(system_sovereign_id),
              Error::<T>::SystemSovereignOccupied
            );
            ensure!(
              SystemSovereignCount::<T>::get() < T::MaxSystemSovereigns::get(),
              Error::<T>::SystemSovereignCapacityExceeded
            );
          }
        }
      }
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        if actor_type == ActorType::User {
          if let Err(error) = Self::charge_creation_fee(&owner) {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
          }
        }
        let (actor_class, sovereign_account) = match actor_type {
          ActorType::User => match Self::allocate_owner_slot(&owner, preferred_user_slot) {
            Ok((slot, account)) => (ActorClass::User { owner_slot: slot }, account),
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error.into(),
              ));
            }
          },
          ActorType::System => match Self::allocate_system_sovereign(system_sovereign_id) {
            Ok(account) => (
              ActorClass::System {
                sovereign_id: system_sovereign_id,
              },
              account,
            ),
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error.into(),
              ));
            }
          },
        };
        let identity = ActorIdentity {
          sovereign_account: sovereign_account.clone(),
          owner: owner.clone(),
          actor_class,
          mutability: Mutability::Mutable,
          cycle_nonce: 0,
          last_control_mutation_block: frame_system::Pallet::<T>::block_number(),
        };
        SovereignIndex::<T>::insert(&sovereign_account, actor_id);
        ActorIdentities::<T>::insert(actor_id, &identity);
        if let Err(error) = ActorIdentityCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_add(1)
            .ok_or(Error::<T>::ActorIdentityCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if actor_type == ActorType::System {
          SystemSovereigns::<T>::insert(
            system_sovereign_id,
            SystemSovereignState::Occupied(actor_id),
          );
          if requested_system_sovereign_id.is_none() {
            if let Err(error) = SystemSovereignCount::<T>::try_mutate(|count| -> DispatchResult {
              *count = count
                .checked_add(1)
                .ok_or(Error::<T>::SystemSovereignCapacityExceeded)?;
              Ok(())
            }) {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error,
              ));
            }
          }
        }
        NextActorId::<T>::put(next_id);
        if actor_type == ActorType::User || requested_system_sovereign_id.is_none() {
          frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        }
        if let Err(error) = Self::reconcile_actor_state_hold(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        Self::deposit_event(Event::ActorCreated {
          actor_id,
          owner,
          actor_class: identity.actor_class,
          mutability: Mutability::Mutable,
          sovereign_account: identity.sovereign_account,
          initial_lifecycle: InitialLifecycle::Dormant,
        });
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })
    }

    fn do_create_user_actor(
      owner: T::AccountId,
      mutability: Mutability,
      preferred_slot: Option<u8>,
      contract: Option<ActorContractOf<T>>,
    ) -> DispatchResult {
      match contract {
        None => {
          ensure!(
            mutability == Mutability::Mutable,
            Error::<T>::ImmutableActor
          );
          Self::do_create_dormant_actor(owner, ActorType::User, preferred_slot, None)
        }
        Some(contract) => Self::do_create_actor(
          owner,
          ActorType::User,
          mutability,
          contract,
          preferred_slot,
          None,
        ),
      }
    }

    fn do_create_system_actor(
      owner: T::AccountId,
      mutability: Mutability,
      contract: Option<ActorContractOf<T>>,
      requested_system_sovereign_id: Option<SystemSovereignId>,
    ) -> DispatchResult {
      match contract {
        None => {
          ensure!(
            mutability == Mutability::Mutable,
            Error::<T>::ImmutableActor
          );
          Self::do_create_dormant_actor(
            owner,
            ActorType::System,
            None,
            requested_system_sovereign_id,
          )
        }
        Some(contract) => Self::do_create_actor(
          owner,
          ActorType::System,
          mutability,
          contract,
          None,
          requested_system_sovereign_id,
        ),
      }
    }

    fn do_create_actor(
      owner: T::AccountId,
      actor_type: ActorType,
      mutability: Mutability,
      mut contract: ActorContractOf<T>,
      preferred_user_slot: Option<u8>,
      requested_system_sovereign_id: Option<SystemSovereignId>,
    ) -> DispatchResult {
      ensure!(
        !GlobalCircuitBreaker::<T>::get(),
        Error::<T>::GlobalCircuitBreakerActive
      );
      Self::canonicalize_preconditions(&mut contract.steps)?;
      ensure!(
        (contract.steps.len() as u32) <= T::MaxContractSteps::get(),
        Error::<T>::TooManyContractSteps
      );
      if actor_type == ActorType::User {
        ensure!(
          !Self::contract_steps_contains_mint(&contract.steps),
          Error::<T>::MintNotAllowedForUserActor
        );
      }
      Self::validate_trigger(&contract.trigger, contract.cooldown_blocks)?;
      if let Some(ref window) = contract.window {
        Self::validate_schedule_window(window)?;
      }
      Self::validate_future_schedule_targets(&contract)?;
      Self::validate_contract_steps_shape(actor_type, &contract.steps)?;
      Self::validate_opening_snapshot_surfaces(&contract.steps)?;
      Self::ensure_retry_later_allowed(mutability, &contract.steps)?;
      if let Some(target_nonce) = contract.auto_close_at_cycle_nonce {
        Self::ensure_auto_close_target(0, target_nonce)?;
      }
      if actor_type == ActorType::System && mutability == Mutability::Immutable {
        ensure!(
          !contract.trigger.manual_source_enabled(),
          Error::<T>::InvalidTriggerConfiguration
        );
      }
      let active_count = Self::active_instance_count();
      ensure!(
        active_count < Self::effective_active_actor_limit(),
        Error::<T>::ActiveActorCapacityExceeded
      );
      ensure!(
        ActorIdentityCount::<T>::get() < T::MaxActorIdentities::get(),
        Error::<T>::ActorIdentityCapacityExceeded
      );
      Self::ensure_contract_steps_fits_idle_budget(actor_type, &contract.steps)?;
      let funding_tracked_assets = Self::derive_funding_tracked_assets(&contract.steps)?;
      let actor_id = NextActorId::<T>::get();
      ensure!(
        !Self::active_actor_exists(actor_id) && !ActorIdentities::<T>::contains_key(actor_id),
        Error::<T>::ActorIdOccupied
      );
      if actor_type == ActorType::System {
        T::SystemActorContractValidator::validate(actor_id, &contract)
          .map_err(|_| Error::<T>::SystemActorTopologyInvalid)?;
      }
      let system_sovereign_id = requested_system_sovereign_id.unwrap_or(actor_id);
      if actor_type == ActorType::System {
        match requested_system_sovereign_id {
          Some(_) => match SystemSovereigns::<T>::get(system_sovereign_id) {
            Some(SystemSovereignState::Vacant) => {}
            Some(SystemSovereignState::Occupied(_)) => {
              return Err(Error::<T>::SystemSovereignOccupied.into());
            }
            None => {
              return Err(Error::<T>::SystemSovereignUnknown.into());
            }
          },
          None => {
            ensure!(
              !SystemSovereigns::<T>::contains_key(system_sovereign_id),
              Error::<T>::SystemSovereignOccupied
            );
            ensure!(
              SystemSovereignCount::<T>::get() < T::MaxSystemSovereigns::get(),
              Error::<T>::SystemSovereignCapacityExceeded
            );
          }
        }
      }
      let prospective_sovereign_account = match actor_type {
        ActorType::User => {
          let owner_slot = Self::available_owner_slot(&owner, preferred_user_slot)?;
          Self::sovereign_account_id(&owner, owner_slot)
        }
        ActorType::System => Self::sovereign_account_id_system(system_sovereign_id),
      };
      Self::validate_recipient_configuration(&contract.steps, &prospective_sovereign_account)?;
      let next_id = actor_id.checked_add(1).ok_or(Error::<T>::ActorIdOverflow)?;
      let now = frame_system::Pallet::<T>::block_number();
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        let (actor_class, sovereign_account) = match actor_type {
          ActorType::User => match Self::allocate_owner_slot(&owner, preferred_user_slot) {
            Ok((slot, account)) => (ActorClass::User { owner_slot: slot }, account),
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error.into(),
              ));
            }
          },
          ActorType::System => match Self::allocate_system_sovereign(system_sovereign_id) {
            Ok(account) => (
              ActorClass::System {
                sovereign_id: system_sovereign_id,
              },
              account,
            ),
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error.into(),
              ));
            }
          },
        };
        if actor_type == ActorType::User {
          // Creation establishes process state only. Sovereign activation capacity is checked
          // when a Trigger-owned Opening becomes ready.
          if let Err(error) = Self::charge_creation_fee(&owner) {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
          }
        }
        let schedule_anchor = Self::schedule_anchor_at(contract.window, now);
        let temporal_anchor_tick = match Self::temporal_anchor_tick(&contract.trigger) {
          Ok(anchor) => anchor,
          Err(error) => {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
              Self::placement_error(error),
            ));
          }
        };
        let identity = ActorIdentity {
          sovereign_account: sovereign_account.clone(),
          owner: owner.clone(),
          actor_class,
          mutability,
          cycle_nonce: 0,
          last_control_mutation_block: now,
        };
        let trigger_runtime_state =
          Self::provisional_trigger_runtime_state(&contract.trigger, temporal_anchor_tick);
        let hot = ActorHotState {
          lifecycle: ActiveLifecycle::Active,
          cycle_state: CycleState::Idle,
          trigger_runtime_state,
          unsuccessful_attempt_streak: 0,
          pending_signal: false,
          queue_ticket: None,
          wakeup_pointer: None,
          trigger_wakeup_pointer: None,
          terminal_at: contract
            .window
            .map(|window| Self::window_terminal_at(&window)),
          schedule_anchor,
          last_cycle_block: None,
        };
        SovereignIndex::<T>::insert(sovereign_account.clone(), actor_id);
        if let Err(error) = Self::insert_active_actor(
          actor_id,
          identity,
          hot,
          contract,
          TriggerTransitionIntent::CreateActive,
        ) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        ActorFunding::<T>::insert(
          actor_id,
          ActorFundingState {
            funding_accumulated: Default::default(),
            funding_tracked_assets,
          },
        );
        if let Err(error) = ActiveActorCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_add(1)
            .ok_or(Error::<T>::ActiveActorCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if let Err(error) = ActorIdentityCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_add(1)
            .ok_or(Error::<T>::ActorIdentityCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if actor_type == ActorType::System {
          SystemSovereigns::<T>::insert(
            system_sovereign_id,
            SystemSovereignState::Occupied(actor_id),
          );
          if requested_system_sovereign_id.is_none() {
            if let Err(error) = SystemSovereignCount::<T>::try_mutate(|count| -> DispatchResult {
              *count = count
                .checked_add(1)
                .ok_or(Error::<T>::SystemSovereignCapacityExceeded)?;
              Ok(())
            }) {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error,
              ));
            }
          }
        }
        NextActorId::<T>::put(next_id);
        if actor_type == ActorType::System && requested_system_sovereign_id.is_none() {
          frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        }
        Self::deposit_event(Event::ActorCreated {
          actor_id,
          owner,
          actor_class,
          mutability,
          sovereign_account,
          initial_lifecycle: InitialLifecycle::Active,
        });
        #[cfg(test)]
        if let Err(error) = crate::mock::control_atomicity_checkpoint(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if let Err(error) = Self::prime_actor_schedule(actor_id).map_err(Self::placement_error) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if let Err(error) = Self::reconcile_actor_state_hold(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })
    }

    fn do_activate_actor(
      actor_id: ActorId,
      mut identity: ActorIdentityOf<T>,
      mut contract: ActorContractOf<T>,
    ) -> DispatchResult {
      ensure!(
        !GlobalCircuitBreaker::<T>::get(),
        Error::<T>::GlobalCircuitBreakerActive
      );
      ensure!(
        identity.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      let actor_type = identity.actor_class.actor_type();
      Self::canonicalize_preconditions(&mut contract.steps)?;
      ensure!(
        (contract.steps.len() as u32) <= T::MaxContractSteps::get(),
        Error::<T>::TooManyContractSteps
      );
      if actor_type == ActorType::User {
        ensure!(
          !Self::contract_steps_contains_mint(&contract.steps),
          Error::<T>::MintNotAllowedForUserActor
        );
      }
      Self::validate_trigger(&contract.trigger, contract.cooldown_blocks)?;
      if let Some(ref window) = contract.window {
        Self::validate_schedule_window(window)?;
      }
      Self::validate_future_schedule_targets(&contract)?;
      Self::validate_contract_steps_shape(actor_type, &contract.steps)?;
      if actor_type == ActorType::System {
        T::SystemActorContractValidator::validate(actor_id, &contract)
          .map_err(|_| Error::<T>::SystemActorTopologyInvalid)?;
      }
      Self::validate_recipient_configuration(&contract.steps, &identity.sovereign_account)?;
      Self::validate_opening_snapshot_surfaces(&contract.steps)?;
      Self::ensure_retry_later_allowed(identity.mutability, &contract.steps)?;
      if let Some(target_nonce) = contract.auto_close_at_cycle_nonce {
        Self::ensure_auto_close_target(identity.cycle_nonce, target_nonce)?;
      }
      Self::ensure_contract_steps_fits_idle_budget(actor_type, &contract.steps)?;
      let funding_tracked_assets = Self::derive_funding_tracked_assets(&contract.steps)?;
      ensure!(
        Self::active_instance_count() < Self::effective_active_actor_limit(),
        Error::<T>::ActiveActorCapacityExceeded
      );
      let now = frame_system::Pallet::<T>::block_number();
      ensure!(
        identity.last_control_mutation_block != now,
        Error::<T>::ControlMutationRateLimited
      );
      identity.last_control_mutation_block = now;
      // Reactivation anchors the fresh Active epoch at the current block; the fresh hot
      // state has no last_cycle_block, so cooldown/cadence use this conservative anchor
      // rather than block zero (spec 4.3.3).
      let schedule_anchor = Self::schedule_anchor_at(contract.window, now);
      let temporal_anchor_tick =
        Self::temporal_anchor_tick(&contract.trigger).map_err(Self::placement_error)?;
      let trigger_runtime_state =
        Self::provisional_trigger_runtime_state(&contract.trigger, temporal_anchor_tick);
      let hot = ActorHotState {
        lifecycle: ActiveLifecycle::Active,
        cycle_state: CycleState::Idle,
        trigger_runtime_state,
        unsuccessful_attempt_streak: 0,
        pending_signal: false,
        queue_ticket: None,
        wakeup_pointer: None,
        trigger_wakeup_pointer: None,
        terminal_at: contract
          .window
          .map(|window| Self::window_terminal_at(&window)),
        schedule_anchor,
        last_cycle_block: None,
      };
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        if !ActorIdentities::<T>::contains_key(actor_id) || Self::active_actor_exists(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorAlreadyActive.into(),
          ));
        }
        if let Err(error) = Self::insert_active_actor(
          actor_id,
          identity,
          hot,
          contract,
          TriggerTransitionIntent::ActivateDormant,
        ) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        ActorFunding::<T>::insert(
          actor_id,
          ActorFundingState {
            funding_accumulated: Default::default(),
            funding_tracked_assets,
          },
        );
        if let Err(error) = ActiveActorCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_add(1)
            .ok_or(Error::<T>::ActiveActorCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        Self::deposit_event(Event::ActorActivated { actor_id });
        #[cfg(test)]
        if let Err(error) = crate::mock::control_atomicity_checkpoint(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if let Err(error) = Self::prime_actor_schedule(actor_id).map_err(Self::placement_error) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if let Err(error) = Self::reconcile_actor_state_hold(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })
    }

    fn do_deactivate_actor(actor_id: ActorId, _instance: ActiveActorViewOf<T>) -> DispatchResult {
      let now = frame_system::Pallet::<T>::block_number();
      let trigger_transition =
        Self::preflight_trigger_cleanup(actor_id, TriggerTransitionIntent::Deactivate)?;
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        if let Err(error) = Self::record_control_mutation(actor_id, now) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if let Err(error) =
          Self::cancel_run_internal(actor_id, CancellationReason::Deactivated, None)
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if let Err(error) = Self::remove_actor_from_queues(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        let LoadedActorStateOf::Active(state) = Self::load_actor_state(actor_id) else {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorInvariant.into(),
          ));
        };
        if state.hot.wakeup_pointer.is_some()
          && Self::wakeup_substrate_invalidate(actor_id).is_none()
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorNotFound.into(),
          ));
        }
        if state.hot.trigger_wakeup_pointer.is_some()
          && Self::trigger_wakeup_substrate_invalidate_inner(actor_id).is_err()
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorNotFound.into(),
          ));
        }
        if let Err(error) = Self::remove_active_actor(actor_id, trigger_transition) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        ActorFunding::<T>::remove(actor_id);
        if let Err(error) = ActiveActorCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_sub(1)
            .ok_or(Error::<T>::ActiveActorCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if let Err(error) = Self::reconcile_actor_state_hold(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        Self::deposit_event(Event::ActorDeactivated { actor_id });
        #[cfg(test)]
        if let Err(error) = crate::mock::control_atomicity_checkpoint(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })
    }

    fn contract_steps_contains_mint(contract_steps: &ContractSteps<T>) -> bool {
      for step in contract_steps.as_slice() {
        if matches!(step.task, ActorTask::Mint { .. }) {
          return true;
        }
      }
      false
    }

    pub(crate) fn validate_trigger(trigger: &TriggerOf<T>, cooldown_blocks: u32) -> DispatchResult {
      ensure!(
        trigger.has_canonical_filters(),
        Error::<T>::InvalidTriggerConfiguration
      );
      if let Some(crossing) = trigger.observation_crossing_contract() {
        ensure!(
          crossing.has_valid_hysteresis(),
          Error::<T>::InvalidTriggerConfiguration
        );
      }
      let max_block_delay: u32 = T::MaxExecutionDelayBlocks::get().saturated_into();
      match trigger {
        Trigger::AtTime { after_ticks } => {
          ensure!(*after_ticks > 0, Error::<T>::InvalidTriggerConfiguration);
          ensure!(
            *after_ticks <= T::MaxTemporalDelayTicks::get(),
            Error::<T>::ExecutionDelayTooLong
          );
          ensure!(
            cooldown_blocks == 0,
            Error::<T>::InvalidTriggerConfiguration
          );
        }
        Trigger::Cadenced { every_ticks } => {
          ensure!(*every_ticks > 0, Error::<T>::InvalidTriggerConfiguration);
          ensure!(
            *every_ticks <= T::MaxTemporalDelayTicks::get(),
            Error::<T>::ExecutionDelayTooLong
          );
          ensure!(
            cooldown_blocks == 0,
            Error::<T>::InvalidTriggerConfiguration
          );
        }
        Trigger::Manual
        | Trigger::AddressEvent { .. }
        | Trigger::ObservationChange { .. }
        | Trigger::ObservationCrossing { .. } => {}
      }
      ensure!(
        cooldown_blocks <= max_block_delay,
        Error::<T>::ExecutionDelayTooLong
      );
      Ok(())
    }

    fn ensure_auto_close_target(current_cycle_nonce: u64, target_nonce: u64) -> DispatchResult {
      ensure!(
        target_nonce > current_cycle_nonce,
        Error::<T>::InvalidAutoCloseNonce
      );
      let horizon = target_nonce
        .checked_sub(current_cycle_nonce)
        .ok_or(Error::<T>::InvalidAutoCloseNonce)?;
      ensure!(
        horizon <= T::MaxAutoCloseNonceHorizon::get(),
        Error::<T>::AutoCloseNonceHorizonExceeded
      );
      Ok(())
    }

    fn validate_future_schedule_targets(contract: &ActorContractOf<T>) -> DispatchResult {
      let now = frame_system::Pallet::<T>::block_number();
      let schedule_anchor = contract
        .window
        .map(|window| now.max(window.start))
        .unwrap_or(now);
      ensure!(
        now.checked_add(&One::one()).is_some(),
        Error::<T>::SchedulerIndexExhausted
      );
      let cooldown: BlockNumberFor<T> = contract.cooldown_blocks.into();
      ensure!(
        schedule_anchor.checked_add(&cooldown).is_some(),
        Error::<T>::SchedulerIndexExhausted
      );
      if matches!(
        contract.trigger,
        Trigger::AtTime { .. } | Trigger::Cadenced { .. }
      ) {
        ensure!(contract.window.is_none(), Error::<T>::InvalidScheduleWindow);
        return Ok(());
      }
      let first_temporal_eligible = schedule_anchor;
      if let Some(window) = contract.window {
        ensure!(
          first_temporal_eligible <= window.end,
          Error::<T>::InvalidScheduleWindow
        );
      }
      Ok(())
    }

    fn validate_schedule_window(window: &ScheduleWindow<BlockNumberFor<T>>) -> DispatchResult {
      ensure!(window.end > window.start, Error::<T>::InvalidScheduleWindow);
      ensure!(
        window.end.checked_add(&One::one()).is_some(),
        Error::<T>::InvalidScheduleWindow
      );
      // Inclusive span: `end - start + 1 >= MinWindowLength` (spec 7.3.2).
      let span = window
        .end
        .checked_sub(&window.start)
        .and_then(|distance| distance.checked_add(&One::one()))
        .ok_or(Error::<T>::InvalidScheduleWindow)?;
      ensure!(
        span >= T::MinWindowLength::get(),
        Error::<T>::InvalidScheduleWindow
      );
      let now = frame_system::Pallet::<T>::block_number();
      // Newly installed Active state requires `end >= current_block`; an in-progress
      // window (`start <= now <= end`) is admissible, and only an already-expired
      // window (`end < now`) is rejected (spec 7.3.3).
      ensure!(window.end >= now, Error::<T>::InvalidScheduleWindow);
      ensure!(
        window.start.saturating_sub(now) <= T::MaxExecutionDelayBlocks::get(),
        Error::<T>::ExecutionDelayTooLong
      );
      Ok(())
    }

    pub(crate) fn window_terminal_at(
      window: &ScheduleWindow<BlockNumberFor<T>>,
    ) -> BlockNumberFor<T> {
      window
        .end
        .checked_add(&One::one())
        .expect("admitted schedule windows have an exact terminal block")
    }

    fn ensure_retry_later_allowed(
      mutability: Mutability,
      contract_steps: &ContractSteps<T>,
    ) -> DispatchResult {
      if mutability == Mutability::Immutable {
        for step in contract_steps {
          ensure!(
            step.on_error.retry_max_attempts().is_none(),
            Error::<T>::RetryLaterNotAllowedForImmutableActor
          );
        }
      }
      Ok(())
    }

    fn canonicalize_preconditions(contract_steps: &mut ContractSteps<T>) -> DispatchResult {
      let mut opening_predicate_count = 0u32;
      for step in contract_steps.iter_mut() {
        let Some(precondition) = &mut step.precondition else {
          continue;
        };
        let clauses = &mut precondition.clauses;
        ensure!(!clauses.is_empty(), Error::<T>::EmptyPrecondition);
        let mut canonical_clauses = alloc::vec::Vec::with_capacity(clauses.len());
        for clause in clauses.iter() {
          ensure!(!clause.is_empty(), Error::<T>::EmptyPrecondition);
          let mut predicates = clause.to_vec();
          predicates.sort_by_key(Encode::encode);
          predicates.dedup();
          canonical_clauses.push(
            BoundedVec::try_from(predicates).map_err(|_| Error::<T>::AdmissionBoundOverflow)?,
          );
        }
        canonical_clauses.sort_by_key(Encode::encode);
        ensure!(
          !canonical_clauses.windows(2).any(|pair| pair[0] == pair[1]),
          Error::<T>::InvalidPredicate
        );
        let mut absorbed = alloc::vec![false; canonical_clauses.len()];
        for subset_index in 0..canonical_clauses.len() {
          for superset_index in 0..canonical_clauses.len() {
            if subset_index == superset_index
              || canonical_clauses[subset_index].len() >= canonical_clauses[superset_index].len()
            {
              continue;
            }
            if canonical_clauses[subset_index]
              .iter() // deos-bypass: bounded-iter -- MaxPredicateClauses bounds canonical DNF.
              .all(|predicate| canonical_clauses[superset_index].contains(predicate))
            {
              absorbed[superset_index] = true;
            }
          }
        }
        canonical_clauses = canonical_clauses
          .into_iter()
          .zip(absorbed)
          .filter_map(|(clause, is_absorbed)| (!is_absorbed).then_some(clause))
          .collect();
        let predicate_count = canonical_clauses
          .iter() // deos-bypass: bounded-iter -- MaxPredicateClauses bounds canonical DNF.
          .try_fold(0u32, |total, clause| total.checked_add(clause.len() as u32))
          .ok_or(Error::<T>::AdmissionBoundOverflow)?;
        ensure!(
          predicate_count <= T::MaxPredicatesPerStep::get(),
          Error::<T>::AdmissionBoundOverflow
        );
        let step_opening_count = canonical_clauses
          .iter() // deos-bypass: bounded-iter -- MaxPredicateClauses bounds canonical DNF.
          .flat_map(|clause| clause.iter())
          .filter(|timed| timed.timing == ObservationTiming::Opening)
          .count() as u32;
        opening_predicate_count = opening_predicate_count
          .checked_add(step_opening_count)
          .ok_or(Error::<T>::AdmissionBoundOverflow)?;
        *clauses = BoundedVec::try_from(canonical_clauses)
          .map_err(|_| Error::<T>::AdmissionBoundOverflow)?;
      }
      ensure!(
        opening_predicate_count <= T::MaxOpeningPredicateResults::get(),
        Error::<T>::AdmissionBoundOverflow
      );
      Ok(())
    }

    fn validate_contract_steps_shape(
      _actor_type: ActorType,
      contract_steps: &ContractSteps<T>,
    ) -> DispatchResult {
      ensure!(
        contract_steps_bound_is_valid(T::MaxContractSteps::get()),
        Error::<T>::TooManyContractSteps
      );
      for step in contract_steps.as_slice() {
        if let Some(max_attempts) = step.on_error.retry_max_attempts() {
          ensure!(
            max_attempts >= 2 && max_attempts <= T::MaxRetryAttempts::get(),
            Error::<T>::InvalidRetryAttemptLimit
          );
        }
        if let Some(precondition) = &step.precondition {
          ensure!(
            !precondition.clauses.is_empty(),
            Error::<T>::EmptyPrecondition
          );
          ensure!(
            precondition.clauses.iter().all(|clause| !clause.is_empty()),
            Error::<T>::EmptyPrecondition
          );
          ensure!(
            precondition.predicate_count() <= T::MaxPredicatesPerStep::get(),
            Error::<T>::AdmissionBoundOverflow
          );
          for timed in precondition.clauses.iter().flat_map(|clause| clause.iter()) {
            let max_age_blocks = match &timed.predicate {
              Predicate::ObservationAbove { max_age_blocks, .. }
              | Predicate::ObservationBelow { max_age_blocks, .. }
              | Predicate::ObservationEquals { max_age_blocks, .. }
              | Predicate::ObservationNotEquals { max_age_blocks, .. } => Some(max_age_blocks),
              _ => None,
            };
            if let Some(max_age_blocks) = max_age_blocks {
              ensure!(*max_age_blocks > 0, Error::<T>::InvalidObservationMaxAge);
            }
          }
        }
        match &step.task {
          ActorTask::Transfer { amount, .. }
          | ActorTask::Burn { amount, .. }
          | ActorTask::Mint { amount, .. }
          | ActorTask::Stake { amount, .. } => Self::validate_amount_resolution(amount)?,
          ActorTask::SplitTransfer { amount, legs, .. } => {
            Self::validate_amount_resolution(amount)?;
            Self::validate_split_transfer_legs(legs)?;
          }
          ActorTask::SwapIn {
            asset_in,
            amount_in,
            asset_out,
            ..
          } => {
            ensure!(asset_in != asset_out, Error::<T>::InvalidTradeBound);
            Self::validate_amount_resolution(amount_in)?;
          }
          ActorTask::SwapOut {
            asset_out,
            amount_out,
            asset_in,
            input_limit,
            ..
          } => {
            ensure!(asset_in != asset_out, Error::<T>::InvalidTradeBound);
            Self::validate_amount_resolution(amount_out)?;
            if let InputLimit::Absolute(max_amount_in) = input_limit {
              ensure!(!max_amount_in.is_zero(), Error::<T>::InvalidTradeBound);
            }
          }
          ActorTask::AddLiquidity {
            asset_a,
            asset_b,
            amount_a,
            amount_b,
            min_lp_out,
          } => {
            ensure!(asset_a != asset_b, Error::<T>::InvalidTradeBound);
            Self::validate_amount_resolution(amount_a)?;
            Self::validate_amount_resolution(amount_b)?;
            ensure!(!min_lp_out.is_zero(), Error::<T>::InvalidTradeBound);
          }
          ActorTask::RemoveLiquidity {
            lp_amount,
            min_amount_a,
            min_amount_b,
            ..
          } => {
            Self::validate_amount_resolution(lp_amount)?;
            ensure!(
              !min_amount_a.is_zero() && !min_amount_b.is_zero(),
              Error::<T>::InvalidTradeBound
            );
          }
          ActorTask::DonateLiquidity {
            asset_a,
            asset_b,
            max_amount_a,
            ..
          } => {
            ensure!(asset_a != asset_b, Error::<T>::InvalidTradeBound);
            Self::validate_amount_resolution(max_amount_a)?;
          }
          ActorTask::Unstake { shares, .. } => Self::validate_amount_resolution(shares)?,
          ActorTask::StopCycle => {}
        }
      }
      Ok(())
    }

    fn validate_recipient_configuration(
      contract_steps: &ContractSteps<T>,
      sovereign_account: &T::AccountId,
    ) -> DispatchResult {
      for step in contract_steps {
        match &step.task {
          ActorTask::Transfer { to, .. } => {
            ensure!(to != sovereign_account, Error::<T>::SelfTransferNotAllowed);
          }
          ActorTask::SplitTransfer { legs, .. } => {
            ensure!(
              legs.iter().all(|leg| &leg.to != sovereign_account),
              Error::<T>::SelfTransferNotAllowed
            );
          }
          _ => {}
        }
      }
      Ok(())
    }

    fn validate_amount_resolution(amount: &AmountResolution<T::Balance>) -> DispatchResult {
      ensure!(
        !matches!(amount, AmountResolution::Fixed(value) if value.is_zero())
          && !matches!(
            amount,
            AmountResolution::PercentageOfCurrent(value)
              | AmountResolution::PercentageAtOpening(value)
              | AmountResolution::PercentageOfLastFunding(value)
              if value.is_zero()
          ),
        Error::<T>::InvalidAmountResolution
      );
      Ok(())
    }

    fn validate_opening_snapshot_surfaces(contract_steps: &ContractSteps<T>) -> DispatchResult {
      for surface in Self::opening_surfaces(contract_steps, 0) {
        if let OpeningSurface::StakingShares(position_asset) = surface {
          ensure!(
            T::StakingOps::share_asset(position_asset).is_some(),
            Error::<T>::InvalidAmountResolution
          );
        }
      }
      Ok(())
    }

    fn derive_funding_tracked_assets(
      contract_steps: &ContractSteps<T>,
    ) -> Result<BoundedBTreeSet<T::AssetId, T::MaxFundingTrackedAssets>, DispatchError> {
      let mut tracked = alloc::collections::BTreeSet::new();

      let mut check_amount = |amount: &AmountResolution<T::Balance>, asset: T::AssetId| {
        if matches!(amount, AmountResolution::PercentageOfLastFunding(_)) {
          tracked.insert(asset);
        }
      };

      for step in contract_steps.as_slice() {
        match &step.task {
          ActorTask::Transfer { asset, amount, .. }
          | ActorTask::SplitTransfer { asset, amount, .. }
          | ActorTask::Burn { asset, amount }
          | ActorTask::Mint { asset, amount } => {
            check_amount(amount, *asset);
          }
          ActorTask::RemoveLiquidity {
            lp_asset: asset,
            lp_amount,
            ..
          } => {
            check_amount(lp_amount, *asset);
          }
          ActorTask::SwapIn {
            asset_in,
            amount_in,
            ..
          } => {
            check_amount(amount_in, *asset_in);
          }
          ActorTask::SwapOut {
            asset_out,
            amount_out,
            ..
          } => {
            check_amount(amount_out, *asset_out);
          }
          ActorTask::AddLiquidity {
            asset_a,
            asset_b,
            amount_a,
            amount_b,
            ..
          } => {
            check_amount(amount_a, *asset_a);
            check_amount(amount_b, *asset_b);
          }
          ActorTask::Stake { asset, amount } => {
            check_amount(amount, *asset);
          }
          ActorTask::DonateLiquidity {
            asset_a,
            max_amount_a,
            ..
          } => {
            check_amount(max_amount_a, *asset_a);
          }
          ActorTask::Unstake { asset, shares } => {
            if matches!(shares, AmountResolution::PercentageOfLastFunding(_)) {
              let share_asset =
                T::StakingOps::share_asset(*asset).ok_or(Error::<T>::InvalidAmountResolution)?;
              check_amount(shares, share_asset);
            }
          }
          ActorTask::StopCycle => {}
        }
      }

      BoundedBTreeSet::try_from(tracked).map_err(|_| Error::<T>::TooManyContractSteps.into())
    }

    pub(crate) fn validate_split_transfer_legs(legs: &SplitTransferLegsOf<T>) -> DispatchResult {
      ensure!(legs.len() >= 2, Error::<T>::InvalidSplitTransfer);
      ensure!(
        (legs.len() as u32) <= T::MaxSplitTransferLegs::get(),
        Error::<T>::InvalidSplitTransfer
      );
      let mut sum_parts: u32 = 0;
      for (idx, leg) in legs.iter().enumerate() {
        ensure!(!leg.share.is_zero(), Error::<T>::InvalidSplitTransfer);
        sum_parts = sum_parts
          .checked_add(leg.share.deconstruct())
          .ok_or(Error::<T>::InvalidSplitTransfer)?;
        let duplicate = legs.iter().take(idx).any(|existing| existing.to == leg.to);
        ensure!(!duplicate, Error::<T>::InvalidSplitTransfer);
      }
      ensure!(
        sum_parts <= Perbill::ACCURACY,
        Error::<T>::InvalidSplitTransfer
      );
      Ok(())
    }

    fn ensure_not_system_immutable(instance: &ActiveActorViewOf<T>) -> DispatchResult {
      ensure!(
        !(instance.actor_class.actor_type() == ActorType::System
          && instance.mutability == Mutability::Immutable),
        Error::<T>::ImmutableActor
      );
      Ok(())
    }

    fn ensure_identity_control_origin(
      origin: OriginFor<T>,
      identity: &ActorIdentityOf<T>,
    ) -> DispatchResult {
      if let Ok(who) = ensure_signed(origin.clone()) {
        ensure!(who == identity.owner, Error::<T>::NotOwner);
        return Ok(());
      }
      T::SystemOrigin::ensure_origin(origin)?;
      ensure!(
        identity.actor_class.actor_type() == ActorType::System,
        Error::<T>::NotGovernance
      );
      Ok(())
    }

    pub(crate) fn with_reused_transaction(
      operation: impl FnOnce() -> DispatchResult,
    ) -> DispatchResult {
      if polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return operation();
      }
      polkadot_sdk::frame_support::storage::with_transaction(|| match operation() {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      })
    }

    fn with_control_transaction(operation: impl FnOnce() -> DispatchResult) -> DispatchResult {
      Self::with_reused_transaction(operation)
    }

    fn record_control_mutation(actor_id: ActorId, now: BlockNumberFor<T>) -> DispatchResult {
      ActorIdentities::<T>::try_mutate(actor_id, |maybe| -> DispatchResult {
        maybe
          .as_mut()
          .ok_or(Error::<T>::ActorNotFound)?
          .last_control_mutation_block = now;
        Ok(())
      })
    }

    fn ensure_control_mutation_allowed(
      instance: &ActiveActorViewOf<T>,
      now: BlockNumberFor<T>,
    ) -> DispatchResult {
      ensure!(
        instance.last_control_mutation_block != now,
        Error::<T>::ControlMutationRateLimited
      );
      Ok(())
    }

    pub(crate) fn ensure_control_origin(
      origin: OriginFor<T>,
      instance: &ActiveActorViewOf<T>,
    ) -> DispatchResult {
      if let Ok(who) = ensure_signed(origin.clone()) {
        ensure!(who == instance.owner, Error::<T>::NotOwner);
        return Ok(());
      }
      T::SystemOrigin::ensure_origin(origin)?;
      ensure!(
        instance.actor_class.actor_type() == ActorType::System,
        Error::<T>::NotGovernance
      );
      Ok(())
    }

    fn remove_owner_slot_binding(owner: &T::AccountId, owner_slot: u8, sovereign: &T::AccountId) {
      let mut bitmap = OwnerSlotBitmaps::<T>::get(owner);
      Self::clear_owner_slot(&mut bitmap, owner_slot);
      if Self::owner_slot_bitmap_is_empty(&bitmap) {
        OwnerSlotBitmaps::<T>::remove(owner);
      } else {
        OwnerSlotBitmaps::<T>::insert(owner, bitmap);
      }
      SovereignIndex::<T>::remove(sovereign);
    }

    /// Performs a runtime-owned terminal transition.
    ///
    /// Callers at extrinsic boundaries must enforce control immutability before
    /// reaching this function. Mandatory protocol closure remains available for
    /// System Immutable actors after terminal execution outcomes.
    pub(crate) fn finalize_actor(
      actor_id: ActorId,
      instance: &ActiveActorViewOf<T>,
      reason: CloseReason,
    ) -> DispatchResult {
      ensure!(
        Self::active_actor_view(actor_id).as_ref() == Some(instance),
        Error::<T>::ActorNotFound
      );
      Self::finalize_actor_loaded(actor_id, instance, reason)
    }

    pub(crate) fn finalize_actor_loaded(
      actor_id: ActorId,
      instance: &ActiveActorViewOf<T>,
      reason: CloseReason,
    ) -> DispatchResult {
      ensure!(
        ActorFunding::<T>::contains_key(actor_id),
        Error::<T>::ActorNotFound
      );
      ensure!(
        ActiveActorCount::<T>::get() > 0,
        Error::<T>::ActiveActorCountInvariant
      );
      ensure!(
        ActorIdentityCount::<T>::get() > 0,
        Error::<T>::ActorIdentityCountInvariant
      );
      ensure!(
        SovereignIndex::<T>::get(&instance.sovereign_account) == Some(actor_id),
        Error::<T>::ActorNotFound
      );
      if let ActorClass::User { owner_slot } = instance.actor_class {
        ensure!(
          Self::owner_slot_is_set(&OwnerSlotBitmaps::<T>::get(&instance.owner), owner_slot),
          Error::<T>::InvalidOwnerSlot
        );
      }
      if let ActorClass::System { sovereign_id } = instance.actor_class {
        // Locator truth: a live System actor must own an occupied locator entry that
        // points back at this actor; any other state is corruption surfaced by the
        // public close path with one exact invariant error.
        ensure!(
          SystemSovereigns::<T>::get(sovereign_id)
            == Some(SystemSovereignState::Occupied(actor_id)),
          Error::<T>::SystemSovereignInvariant
        );
      }
      let trigger_transition =
        Self::preflight_trigger_cleanup(actor_id, TriggerTransitionIntent::Close)?;

      polkadot_sdk::frame_support::storage::with_transaction(|| {
        let result = (|| -> DispatchResult {
          Self::cancel_run_internal(actor_id, CancellationReason::Closing(reason), None)?;

          // Actor-local ticket/pointer ownership makes shared queue and wakeup entries stale as
          // soon as hot state disappears. Terminal cleanup performs no shared-container scan.
          Self::remove_active_actor(actor_id, trigger_transition)?;
          ActorIdentities::<T>::remove(actor_id);
          ActorFunding::<T>::remove(actor_id);
          ActiveActorCount::<T>::try_mutate(|count| -> DispatchResult {
            *count = count
              .checked_sub(1)
              .ok_or(Error::<T>::ActiveActorCountInvariant)?;
            Ok(())
          })?;
          ActorIdentityCount::<T>::try_mutate(|count| -> DispatchResult {
            *count = count
              .checked_sub(1)
              .ok_or(Error::<T>::ActorIdentityCountInvariant)?;
            Ok(())
          })?;
          match instance.actor_class {
            ActorClass::User { owner_slot } => Self::remove_owner_slot_binding(
              &instance.owner,
              owner_slot,
              &instance.sovereign_account,
            ),
            ActorClass::System { sovereign_id } => {
              SovereignIndex::<T>::remove(&instance.sovereign_account);
              SystemSovereigns::<T>::insert(sovereign_id, SystemSovereignState::Vacant);
            }
          }
          Self::reconcile_actor_state_hold(actor_id)?;
          Self::deposit_event(Event::ActorClosed { actor_id, reason });
          Ok(())
        })();
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    fn close_inactive_actor(
      actor_id: ActorId,
      identity: &ActorIdentityOf<T>,
      reason: CloseReason,
    ) -> DispatchResult {
      ensure!(
        ActorIdentities::<T>::get(actor_id).as_ref() == Some(identity),
        Error::<T>::ActorNotFound
      );
      ensure!(
        ActorIdentityCount::<T>::get() > 0,
        Error::<T>::ActorIdentityCountInvariant
      );
      ensure!(
        SovereignIndex::<T>::get(&identity.sovereign_account) == Some(actor_id),
        Error::<T>::ActorNotFound
      );
      match identity.actor_class {
        ActorClass::User { owner_slot } => ensure!(
          Self::owner_slot_is_set(&OwnerSlotBitmaps::<T>::get(&identity.owner), owner_slot),
          Error::<T>::InvalidOwnerSlot
        ),
        ActorClass::System { sovereign_id } => ensure!(
          SystemSovereigns::<T>::get(sovereign_id)
            == Some(SystemSovereignState::Occupied(actor_id)),
          Error::<T>::SystemSovereignInvariant
        ),
      }

      Self::with_control_transaction(|| {
        ActorIdentities::<T>::remove(actor_id);
        ActorIdentityCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_sub(1)
            .ok_or(Error::<T>::ActorIdentityCountInvariant)?;
          Ok(())
        })?;
        match identity.actor_class {
          ActorClass::User { owner_slot } => Self::remove_owner_slot_binding(
            &identity.owner,
            owner_slot,
            &identity.sovereign_account,
          ),
          ActorClass::System { sovereign_id } => {
            SovereignIndex::<T>::remove(&identity.sovereign_account);
            SystemSovereigns::<T>::insert(sovereign_id, SystemSovereignState::Vacant);
          }
        }
        Self::reconcile_actor_state_hold(actor_id)?;
        Self::deposit_event(Event::ActorClosed { actor_id, reason });
        Ok(())
      })
    }

    pub(crate) fn update_idle_starvation_state(_now: BlockNumberFor<T>, starved: bool) {
      let state = IdleStarvationState::<T>::get();
      if !starved {
        if let IdleStarvationPhase::Alerted { consecutive_blocks } = state {
          Self::deposit_event(Event::IdleStarvationRecovered { consecutive_blocks });
        }
        if !matches!(state, IdleStarvationPhase::Healthy) {
          IdleStarvationState::<T>::kill();
        }
        return;
      }
      let consecutive_blocks = match state {
        IdleStarvationPhase::Healthy => 1,
        IdleStarvationPhase::Starving { consecutive_blocks }
        | IdleStarvationPhase::Alerted { consecutive_blocks } => {
          consecutive_blocks.saturating_add(1)
        }
      };
      if consecutive_blocks >= T::MaxIdleStarvationBlocks::get() {
        let first_alert = !matches!(state, IdleStarvationPhase::Alerted { .. });
        IdleStarvationState::<T>::put(IdleStarvationPhase::Alerted { consecutive_blocks });
        if first_alert {
          Self::deposit_event(Event::IdleStarvationDetected { consecutive_blocks });
        }
      } else {
        IdleStarvationState::<T>::put(IdleStarvationPhase::Starving { consecutive_blocks });
      }
    }

    // --- Active Actors Set Operations ---

    pub(crate) fn effective_active_actor_limit() -> u32 {
      ActiveActorLimit::<T>::get()
    }

    pub(crate) fn max_configurable_active_actor_limit() -> u32 {
      T::MaxActiveActors::get().min(T::MaxQueueLength::get())
    }

    pub(crate) fn active_instance_count() -> u32 {
      ActiveActorCount::<T>::get()
    }

    pub(crate) fn remove_actor_from_queues(actor_id: ActorId) -> DispatchResult {
      Self::try_paged_invalidate(actor_id)
        .map(|_| ())
        .map_err(Self::placement_error)
    }

    #[cfg(feature = "try-runtime")]
    pub(crate) fn do_try_state() -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      use polkadot_sdk::sp_runtime::TryRuntimeError;
      if PrepassExecutionCutoff::<T>::get()
        .is_some_and(|(_, cutoff)| cutoff > NextQueueTicket::<T>::get())
      {
        return Err(TryRuntimeError::Other(
          "prepass execution cutoff exceeds the allocated ticket frontier",
        ));
      }
      if MaterializationFamilyCursor::<T>::get() >= 3 {
        return Err(TryRuntimeError::Other(
          "materialization family cursor is outside the canonical three-family domain",
        ));
      }
      let limit = Self::effective_active_actor_limit();
      let active_count = Self::active_instance_count();
      let actual_active_count = ActorHot::<T>::iter_keys().count() as u32;
      if T::MaxOwnerSlots::get() == 0 {
        return Err(TryRuntimeError::Other("MaxOwnerSlots must be nonzero"));
      }
      if !contract_steps_bound_is_valid(T::MaxContractSteps::get()) {
        return Err(TryRuntimeError::Other(
          "MaxContractSteps must be in 1..=255",
        ));
      }
      if limit == 0 || limit > Self::max_configurable_active_actor_limit() {
        return Err(TryRuntimeError::Other(
          "ActiveActorLimit is outside the configured bounds",
        ));
      }
      if active_count != actual_active_count {
        return Err(TryRuntimeError::Other(
          "ActiveActorCount does not match ActorHot cardinality",
        ));
      }
      if active_count > limit {
        return Err(TryRuntimeError::Other(
          "ActorHot count exceeds effective active actor limit",
        ));
      }
      let stored_identity_count = ActorIdentities::<T>::iter_keys().count() as u32;
      let identity_count = ActorIdentityCount::<T>::get();
      if identity_count != stored_identity_count {
        return Err(TryRuntimeError::Other(
          "ActorIdentityCount does not match ActorIdentities cardinality",
        ));
      }
      if identity_count > T::MaxActorIdentities::get() {
        return Err(TryRuntimeError::Other(
          "ActorIdentityCount exceeds MaxActorIdentities",
        ));
      }
      for actor_id in ActorContractHeads::<T>::iter_keys() {
        if !matches!(
          Self::load_actor_state(actor_id),
          LoadedActorStateOf::Active(_)
        ) {
          return Err(TryRuntimeError::Other(
            "ActorContract entry belongs to a corrupt actor partition set",
          ));
        }
      }
      let mut max_id: Option<ActorId> = None;
      for actor_id in ActorHot::<T>::iter_keys() {
        let LoadedActorStateOf::Active(state) = Self::load_actor_state(actor_id) else {
          return Err(TryRuntimeError::Other(
            "ActorHot entry belongs to a corrupt actor partition set",
          ));
        };
        let identity = state.identity;
        let hot = state.hot;
        let contract = state.contract;
        let funding = state.funding;
        if !ActorAdmissionCertificates::<T>::contains_key(actor_id) {
          return Err(TryRuntimeError::Other(
            "Active actor has no admission certificate",
          ));
        }
        let head = ActorContractHeads::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
          "Active actor has no C6 Contract head",
        ))?;
        let resources = Self::derive_step_resource_envelopes(&contract).ok_or(
          TryRuntimeError::Other("Active actor Step resources cannot be rederived"),
        )?;
        let expected_pipeline_machine_envelope = Self::derive_pipeline_machine_envelope(
          identity.actor_class.actor_type(),
          &contract.steps,
          &resources,
        )
        .map_err(|_| {
          TryRuntimeError::Other("Active actor Pipeline Machine envelope cannot be rederived")
        })?;
        if head.header.pipeline_machine_envelope != expected_pipeline_machine_envelope {
          return Err(TryRuntimeError::Other(
            "Active actor Pipeline Machine envelope disagrees with canonical Step control resources",
          ));
        }
        let activation_authority = ActorActivationAuthorities::<T>::get(actor_id);
        let expected_activation_feed = match &contract.trigger {
          Trigger::ObservationChange { feed } => Some(*feed),
          Trigger::ObservationCrossing { feed, .. } => Some(*feed),
          _ => None,
        };
        match (expected_activation_feed, activation_authority) {
          (Some(feed), Some(authority)) => {
            let certificate = ActorAdmissionCertificates::<T>::get(actor_id).ok_or(
              TryRuntimeError::Other("indexed Observation actor has no admission certificate"),
            )?;
            if authority.feed != feed
              || authority.cooldown_blocks != contract.cooldown_blocks
              || authority.window != contract.window
              || authority.auto_close_at_cycle_nonce != contract.auto_close_at_cycle_nonce
              || authority.semantic_contract_id != certificate.semantic_contract_id
              || authority.body_commitment != certificate.body_commitment
              || authority.admission_identity != certificate.admission_identity
            {
              return Err(TryRuntimeError::Other(
                "indexed Observation activation authority disagrees with C6 Contract authority",
              ));
            }
          }
          (Some(_), None) => {
            return Err(TryRuntimeError::Other(
              "indexed Observation actor has no activation authority",
            ));
          }
          (None, Some(_)) => {
            return Err(TryRuntimeError::Other(
              "non-indexed-Observation actor retains activation authority",
            ));
          }
          (None, None) => {}
        }
        // Terminal membership is derived from the schedule window: `terminal_at` is the sole
        // terminal-membership authority and must equal the window's exact terminal block, or be
        // absent without a window (spec 5.1).
        let program_window = contract.window;
        let expected_terminal_at = program_window.map(|window| Self::window_terminal_at(&window));
        if hot.terminal_at != expected_terminal_at {
          return Err(TryRuntimeError::Other(
            "ActorHot terminal_at disagrees with schedule window terminal membership",
          ));
        }
        match hot.trigger_runtime_state {
          TriggerRuntimeState::AtTime { consumed: true, .. }
            if hot.trigger_wakeup_pointer.is_some() =>
          {
            return Err(TryRuntimeError::Other(
              "consumed AtTime actor retains Trigger temporal membership",
            ));
          }
          TriggerRuntimeState::AtTime {
            consumed: false, ..
          }
          | TriggerRuntimeState::Cadenced { .. }
            if hot.trigger_wakeup_pointer.is_none() =>
          {
            return Err(TryRuntimeError::Other(
              "pending temporal actor has no Trigger temporal membership",
            ));
          }
          TriggerRuntimeState::Stateless | TriggerRuntimeState::ObservationCrossing { .. }
            if hot.trigger_wakeup_pointer.is_some() =>
          {
            return Err(TryRuntimeError::Other(
              "non-temporal actor retains Trigger temporal membership",
            ));
          }
          _ => {}
        }
        let instance = Self::derive_active_actor_view(identity, hot, contract);
        if !Self::contract_steps_admission_weight_upper(
          instance.actor_class.actor_type(),
          &instance.steps,
        )
        .all_lte(
          Self::guaranteed_actor_service_weight().ok_or(TryRuntimeError::Other(
            "configured housekeeping Weight exceeds ActorOnIdleReserve",
          ))?,
        ) {
          return Err(TryRuntimeError::Other(
            "active Actor Contract exceeds current actor-service envelope",
          ));
        }
        max_id = Some(max_id.map_or(actor_id, |prev| prev.max(actor_id)));
        for (asset, amount) in &funding.funding_accumulated {
          if !funding.funding_tracked_assets.contains(asset) || amount.is_zero() {
            return Err(TryRuntimeError::Other(
              "ActorFunding accumulator contains an untracked asset or zero amount",
            ));
          }
        }
        match SovereignIndex::<T>::get(&instance.sovereign_account) {
          Some(mapped_id) if mapped_id == actor_id => {}
          _ => {
            return Err(TryRuntimeError::Other(
              "SovereignIndex does not map sovereign_account back to actor_id",
            ));
          }
        }
        match instance.actor_class {
          ActorClass::User { owner_slot } => {
            if owner_slot >= T::MaxOwnerSlots::get() {
              return Err(TryRuntimeError::Other(
                "User Actors owner_slot exceeds MaxOwnerSlots",
              ));
            }
            let bitmap = OwnerSlotBitmaps::<T>::get(&instance.owner);
            if !Self::owner_slot_bitmap_is_valid(&bitmap)
              || !Self::owner_slot_is_set(&bitmap, owner_slot)
            {
              return Err(TryRuntimeError::Other(
                "User Actors owner_slot is missing from OwnerSlotBitmaps",
              ));
            }
          }
          ActorClass::System { sovereign_id }
            if SystemSovereigns::<T>::get(sovereign_id)
              != Some(SystemSovereignState::Occupied(actor_id)) =>
          {
            return Err(TryRuntimeError::Other(
              "active System Actor disagrees with its sovereign locator",
            ));
          }
          ActorClass::System { .. } => {}
        }
      }
      for actor_id in ActorFunding::<T>::iter_keys() {
        if !matches!(
          Self::load_actor_state(actor_id),
          LoadedActorStateOf::Active(_)
        ) {
          return Err(TryRuntimeError::Other(
            "ActorFunding entry belongs to a corrupt actor partition set",
          ));
        }
      }
      for actor_id in ActorRunHeads::<T>::iter_keys() {
        if !ActorRunPayloads::<T>::contains_key(actor_id) {
          return Err(TryRuntimeError::Other(
            "ActorRunHead has no immutable ActorRunPayload",
          ));
        }
        let LoadedActorStateOf::Active(state) = Self::load_actor_state(actor_id) else {
          return Err(TryRuntimeError::Other(
            "ActorRunState entry belongs to a corrupt actor partition set",
          ));
        };
        let run_state = state.run_state.ok_or(TryRuntimeError::Other(
          "ActorRunState key is absent from loaded Active state",
        ))?;
        let hot = state.hot;
        let identity = state.identity;
        let contract = state.contract;
        if !matches!(hot.cycle_state, CycleState::Running | CycleState::Suspended)
          || identity.cycle_nonce.checked_add(1) != Some(run_state.cycle_nonce)
          || run_state.cursor >= contract.steps.len() as u32
        {
          return Err(TryRuntimeError::Other(
            "ActorRunState violates run marker, nonce, mutability, or cursor bounds",
          ));
        }
        match hot.cycle_state {
          CycleState::Suspended => {
            if identity.mutability != Mutability::Mutable || !run_state.suspension_is_coherent() {
              return Err(TryRuntimeError::Other(
                "Suspended ActorRunState has incoherent outcome or suspension authority",
              ));
            }
            let max_attempts = contract.steps[run_state.cursor as usize]
              .on_error
              .retry_max_attempts()
              .ok_or(TryRuntimeError::Other(
                "ActorRunState cursor does not own RetryLater",
              ))?;
            if run_state.unsuccessful_attempts_at_cursor == 0
              || run_state.unsuccessful_attempts_at_cursor >= max_attempts
            {
              return Err(TryRuntimeError::Other(
                "ActorRunState cursor-local attempt count is outside its live range",
              ));
            }
            let expected_eligible_at = Self::suspension_eligible_at(
              contract.cooldown_blocks,
              contract.window,
              run_state.last_attempt_block,
              run_state.unsuccessful_attempts_at_cursor,
            )
            .map_err(|_| {
              TryRuntimeError::Other("ActorRunState retry eligibility is unrepresentable")
            })?;
            if run_state.eligible_at != expected_eligible_at {
              return Err(TryRuntimeError::Other(
                "ActorRunState retry eligibility disagrees with its suspension facts",
              ));
            }
          }
          CycleState::Running => {
            if run_state.unsuccessful_attempts_at_cursor != 0 || !run_state.running_is_coherent() {
              return Err(TryRuntimeError::Other(
                "Running ActorRunState lacks a causal committed-Step boundary",
              ));
            }
          }
          CycleState::Idle => {
            return Err(TryRuntimeError::Other(
              "Idle Actor cannot retain ActorRunState",
            ));
          }
        }
        let expected_surfaces = Self::opening_surfaces(&contract.steps, 0);
        let mut surfaces_match = expected_surfaces.len() == run_state.opening_snapshot.len();
        for surface in &expected_surfaces {
          if !run_state.opening_snapshot.contains_key(surface) {
            surfaces_match = false;
            break;
          }
        }
        if !surfaces_match {
          return Err(TryRuntimeError::Other(
            "ActorRunState opening snapshot disagrees with the complete Contract",
          ));
        }
        let expected_opening_predicates = contract
          .steps
          .iter() // deos-bypass: bounded-iter -- MaxSteps bounds the Contract.
          .map(|step| {
            step.precondition.as_ref().map_or(0, |precondition| {
              precondition.opening_predicate_count() as usize
            })
          })
          .sum::<usize>();
        if run_state.opening_predicate_results.len() != expected_opening_predicates {
          return Err(TryRuntimeError::Other(
            "ActorRunState opening predicate results disagree with the Actor Contract",
          ));
        }
        if run_state
          .funding_snapshot
          .keys()
          .any(|asset| !state.funding.funding_tracked_assets.contains(asset))
        {
          return Err(TryRuntimeError::Other(
            "ActorRunState funding snapshot contains an untracked asset",
          ));
        }
      }
      for actor_id in ActorRunPayloads::<T>::iter_keys() {
        if !ActorRunHeads::<T>::contains_key(actor_id) {
          return Err(TryRuntimeError::Other(
            "ActorRunPayload has no mutable ActorRunHead",
          ));
        }
      }
      for actor_id in ActorIdentities::<T>::iter_keys() {
        let identity = ActorIdentities::<T>::get(actor_id)
          .ok_or(TryRuntimeError::Other("actor identity key has no value"))?;
        if identity.last_control_mutation_block > frame_system::Pallet::<T>::block_number() {
          return Err(TryRuntimeError::Other(
            "actor control mutation block is in the future",
          ));
        }
        max_id = Some(max_id.map_or(actor_id, |prev| prev.max(actor_id)));
        if Self::active_actor_exists(actor_id) {
          continue;
        }
        if ActorFunding::<T>::contains_key(actor_id)
          || ActorRunHeads::<T>::contains_key(actor_id)
          || ActorRunPayloads::<T>::contains_key(actor_id)
        {
          return Err(TryRuntimeError::Other(
            "Dormant identity owns active scheduler or readiness state",
          ));
        }
        match SovereignIndex::<T>::get(&identity.sovereign_account) {
          Some(mapped_id) if mapped_id == actor_id => {}
          _ => {
            return Err(TryRuntimeError::Other(
              "Dormant SovereignIndex does not map sovereign_account back to actor_id",
            ));
          }
        }
        match identity.actor_class {
          ActorClass::User { owner_slot } => {
            if owner_slot >= T::MaxOwnerSlots::get() {
              return Err(TryRuntimeError::Other(
                "Dormant User Actors owner_slot exceeds MaxOwnerSlots",
              ));
            }
            let bitmap = OwnerSlotBitmaps::<T>::get(&identity.owner);
            if !Self::owner_slot_bitmap_is_valid(&bitmap)
              || !Self::owner_slot_is_set(&bitmap, owner_slot)
            {
              return Err(TryRuntimeError::Other(
                "Dormant User Actors owner_slot is missing from OwnerSlotBitmaps",
              ));
            }
          }
          ActorClass::System { sovereign_id } => {
            if identity.mutability != Mutability::Mutable {
              return Err(TryRuntimeError::Other(
                "Dormant System Actors must be Mutable",
              ));
            }
            if SystemSovereigns::<T>::get(sovereign_id)
              != Some(SystemSovereignState::Occupied(actor_id))
            {
              return Err(TryRuntimeError::Other(
                "dormant System Actor disagrees with its sovereign locator",
              ));
            }
          }
        }
      }
      for owner in OwnerSlotBitmaps::<T>::iter_keys() {
        let bitmap = OwnerSlotBitmaps::<T>::get(&owner);
        if !Self::owner_slot_bitmap_is_valid(&bitmap) || Self::owner_slot_bitmap_is_empty(&bitmap) {
          return Err(TryRuntimeError::Other(
            "OwnerSlotBitmaps contains an invalid or empty bitmap",
          ));
        }
        for owner_slot in 0..T::MaxOwnerSlots::get() {
          if !Self::owner_slot_is_set(&bitmap, owner_slot) {
            continue;
          }
          let sovereign = Self::sovereign_account_id(&owner, owner_slot);
          let Some(actor_id) = SovereignIndex::<T>::get(&sovereign) else {
            return Err(TryRuntimeError::Other(
              "OwnerSlotBitmaps bit has no SovereignIndex owner",
            ));
          };
          let Some(identity) = ActorIdentities::<T>::get(actor_id) else {
            return Err(TryRuntimeError::Other(
              "OwnerSlotBitmaps bit has no ActorIdentity owner",
            ));
          };
          if identity.owner != owner
            || identity.actor_class != (ActorClass::User { owner_slot })
            || identity.sovereign_account != sovereign
          {
            return Err(TryRuntimeError::Other(
              "OwnerSlotBitmaps bit disagrees with ActorIdentity",
            ));
          }
        }
      }
      let actual_system_sovereign_count = u32::try_from(SystemSovereigns::<T>::iter_keys().count())
        .map_err(|_| TryRuntimeError::Other("SystemSovereigns cardinality exceeds u32"))?;
      if SystemSovereignCount::<T>::get() != actual_system_sovereign_count {
        return Err(TryRuntimeError::Other(
          "SystemSovereignCount does not match SystemSovereigns cardinality",
        ));
      }
      if actual_system_sovereign_count > T::MaxSystemSovereigns::get() {
        return Err(TryRuntimeError::Other(
          "SystemSovereigns cardinality exceeds MaxSystemSovereigns",
        ));
      }
      let mut system_identity_owners = alloc::collections::BTreeMap::new();
      for (actor_id, identity) in ActorIdentities::<T>::iter() {
        if let ActorClass::System { sovereign_id } = identity.actor_class
          && system_identity_owners
            .insert(sovereign_id, actor_id)
            .is_some()
        {
          return Err(TryRuntimeError::Other(
            "multiple System Actor identities own one sovereign locator",
          ));
        }
      }
      let mut derived_system_accounts = alloc::collections::BTreeSet::new();
      for (sovereign_id, locator_state) in SystemSovereigns::<T>::iter() {
        let sovereign_account = Self::sovereign_account_id_system(sovereign_id);
        if !derived_system_accounts.insert(sovereign_account.clone()) {
          return Err(TryRuntimeError::Other(
            "System sovereign locators derive a duplicate custody account",
          ));
        }
        match locator_state {
          SystemSovereignState::Vacant => {
            if system_identity_owners.contains_key(&sovereign_id)
              || SovereignIndex::<T>::contains_key(&sovereign_account)
            {
              return Err(TryRuntimeError::Other(
                "vacant System sovereign locator retains identity ownership",
              ));
            }
          }
          SystemSovereignState::Occupied(actor_id) => {
            if system_identity_owners.get(&sovereign_id) != Some(&actor_id) {
              return Err(TryRuntimeError::Other(
                "occupied System sovereign locator disagrees with identity ownership",
              ));
            }
            let identity = ActorIdentities::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
              "occupied System sovereign locator has no ActorIdentity",
            ))?;
            if identity.sovereign_account != sovereign_account
              || SovereignIndex::<T>::get(&sovereign_account) != Some(actor_id)
            {
              return Err(TryRuntimeError::Other(
                "occupied System sovereign locator disagrees with derived custody ownership",
              ));
            }
          }
        }
      }
      if system_identity_owners.len() > actual_system_sovereign_count as usize {
        return Err(TryRuntimeError::Other(
          "System Actor identity has no sovereign locator",
        ));
      }
      let mut sovereign_index_count = 0u32;
      for (sovereign_account, actor_id) in SovereignIndex::<T>::iter() {
        sovereign_index_count =
          sovereign_index_count
            .checked_add(1)
            .ok_or(TryRuntimeError::Other(
              "SovereignIndex cardinality exceeds u32",
            ))?;
        let identity = ActorIdentities::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
          "SovereignIndex owner has no ActorIdentity",
        ))?;
        if identity.sovereign_account != sovereign_account {
          return Err(TryRuntimeError::Other(
            "SovereignIndex key disagrees with ActorIdentity custody account",
          ));
        }
      }
      if sovereign_index_count != identity_count {
        return Err(TryRuntimeError::Other(
          "SovereignIndex cardinality does not match ActorIdentities",
        ));
      }

      let queue_capacity = T::MaxQueueLength::get();
      if queue_capacity < limit {
        return Err(TryRuntimeError::Other(
          "MaxQueueLength is below effective active actor limit",
        ));
      }
      let queue_occupancy = QueueOccupancy::<T>::get();
      if queue_occupancy > queue_capacity {
        return Err(TryRuntimeError::Other(
          "canonical queue physical occupancy exceeds MaxQueueLength",
        ));
      }
      let next_ticket = NextQueueTicket::<T>::get();
      let page_size = u64::from(T::QueuePageSize::get());
      let head = QueueHead::<T>::get();
      let tail = QueueTail::<T>::get();
      if head > tail {
        return Err(TryRuntimeError::Other("canonical queue head exceeds tail"));
      }
      let queue_span = tail.checked_sub(head).ok_or(TryRuntimeError::Other(
        "canonical queue physical span underflows",
      ))?;
      if head < tail && queue_span != u64::from(queue_occupancy) {
        return Err(TryRuntimeError::Other(
          "canonical queue occupancy disagrees with its nonempty physical span",
        ));
      }
      let mut physical_tickets = alloc::collections::BTreeMap::new();
      let mut physical_occupancy = 0u32;
      for page_id in QueuePages::<T>::iter_keys() {
        let page = QueuePages::<T>::get(page_id)
          .ok_or(TryRuntimeError::Other("queue page key has no value"))?;
        if page.is_empty() || page.len() > T::QueuePageSize::get() as usize {
          return Err(TryRuntimeError::Other(
            "canonical queue page has invalid length",
          ));
        }
        let Some(page_start) = page_id.checked_mul(page_size) else {
          return Err(TryRuntimeError::Other(
            "canonical queue page range overflows",
          ));
        };
        let Some(page_end) = page_start.checked_add(page.len() as u64) else {
          return Err(TryRuntimeError::Other(
            "canonical queue page range overflows",
          ));
        };
        if page_end <= head || page_start >= tail {
          return Err(TryRuntimeError::Other(
            "canonical queue page lies outside its live physical range",
          ));
        }
        for (slot, entry) in page.into_iter().enumerate() {
          let position = page_start
            .checked_add(slot as u64)
            .ok_or(TryRuntimeError::Other("canonical queue position overflows"))?;
          if position < head {
            continue;
          }
          if position >= tail || entry.ticket >= next_ticket {
            return Err(TryRuntimeError::Other(
              "canonical queue entry lies beyond its physical or global ticket range",
            ));
          }
          physical_occupancy = physical_occupancy
            .checked_add(1)
            .ok_or(TryRuntimeError::Other(
              "canonical queue occupancy overflows",
            ))?;
          if physical_tickets
            .insert(entry.ticket, entry.actor_id)
            .is_some()
          {
            return Err(TryRuntimeError::Other(
              "canonical queue contains a duplicate global ticket",
            ));
          }
        }
      }
      if physical_occupancy != queue_occupancy {
        return Err(TryRuntimeError::Other(
          "canonical queue occupancy disagrees with physical entries",
        ));
      }
      let mut live_queue_tickets = alloc::collections::BTreeSet::new();
      for actor_id in ActorHot::<T>::iter_keys() {
        let hot = ActorHot::<T>::get(actor_id)
          .ok_or(TryRuntimeError::Other("live-ticket hot key has no value"))?;
        let Some(ticket) = hot.queue_ticket else {
          continue;
        };
        if ticket >= next_ticket || !live_queue_tickets.insert(ticket) {
          return Err(TryRuntimeError::Other(
            "ActorHot owns an invalid or duplicate global queue ticket",
          ));
        }
        if !ActorIdentities::<T>::contains_key(actor_id) {
          return Err(TryRuntimeError::Other(
            "ActorHot live ticket has no ActorIdentity",
          ));
        }
        if physical_tickets.get(&ticket) != Some(&actor_id) {
          return Err(TryRuntimeError::Other(
            "ActorHot live ticket does not resolve to its canonical queue entry",
          ));
        }
      }
      if T::WakeupPageSize::get() == 0 {
        return Err(TryRuntimeError::Other("WakeupPageSize must be non-zero"));
      }
      let mut wakeup_live_by_key = alloc::collections::BTreeMap::new();
      let mut live_wakeup_memberships = 0u32;
      for (block, page_id) in WakeupPages::<T>::iter_keys() {
        let page = WakeupPages::<T>::get((block, page_id))
          .ok_or(TryRuntimeError::Other("wakeup page key has no value"))?;
        if page.entries.is_empty() || page.entries.len() > T::WakeupPageSize::get() as usize {
          return Err(TryRuntimeError::Other(
            "WakeupPages entry has invalid length",
          ));
        }
        if page.scan_slot as usize > page.entries.len() {
          return Err(TryRuntimeError::Other(
            "WakeupPage scan cursor exceeds page length",
          ));
        }
        let mut live_entries = 0u32;
        for entry in page.entries.as_slice() {
          if entry.is_some() {
            live_entries = live_entries
              .checked_add(1)
              .ok_or(TryRuntimeError::Other("wakeup page live count overflows"))?;
          }
        }
        if live_entries == 0 || page.live_entries != live_entries {
          return Err(TryRuntimeError::Other(
            "WakeupPage live-entry count disagrees with slots",
          ));
        }
        let Some(bucket) = WakeupBuckets::<T>::get(block) else {
          return Err(TryRuntimeError::Other(
            "WakeupPage has no matching bucket metadata",
          ));
        };
        if let Some(previous_page) = page.previous_page
          && WakeupPages::<T>::get((block, previous_page)).and_then(|previous| previous.next_page)
            != Some(page_id)
        {
          return Err(TryRuntimeError::Other(
            "WakeupPage previous link is not reciprocal",
          ));
        }
        if let Some(next_page) = page.next_page
          && WakeupPages::<T>::get((block, next_page)).and_then(|next| next.previous_page)
            != Some(page_id)
        {
          return Err(TryRuntimeError::Other(
            "WakeupPage next link is not reciprocal",
          ));
        }
        if page_id == bucket.head_page && page.previous_page.is_some() {
          return Err(TryRuntimeError::Other(
            "WakeupBucket head page has a predecessor",
          ));
        }
        if page_id == bucket.tail_page && page.next_page.is_some() {
          return Err(TryRuntimeError::Other(
            "WakeupBucket tail page has a successor",
          ));
        }
        let key_live = wakeup_live_by_key.entry(block).or_insert(0u32);
        *key_live = key_live
          .checked_add(live_entries)
          .ok_or(TryRuntimeError::Other("wakeup bucket live count overflows"))?;
        // Block-keyed Pipeline service and tick-keyed Trigger detection own independent exact
        // pointers into one physical substrate. A slot whose clock-domain pointer differs is
        // corruption; an absent pointer is a lazy stale entry until bounded drain converges.
        for slot in 0..page.entries.len() {
          let Some(entry) = &page.entries[slot] else {
            continue;
          };
          let expected = WakeupPointer {
            block,
            page_id,
            slot: slot as WakeupSlot,
          };
          let authoritative = ActorHot::<T>::get(entry.actor_id).and_then(|hot| match block {
            WakeupKey::Block(_) => hot.wakeup_pointer,
            WakeupKey::Tick(_) => hot.trigger_wakeup_pointer.map(|pointer| WakeupPointer {
              block: WakeupKey::Tick(pointer.tick),
              page_id: pointer.page_id,
              slot: pointer.slot,
            }),
          });
          match authoritative {
            Some(pointer) if pointer == expected => {
              live_wakeup_memberships =
                live_wakeup_memberships
                  .checked_add(1)
                  .ok_or(TryRuntimeError::Other(
                    "live wakeup membership count overflows",
                  ))?;
            }
            Some(_) => {
              return Err(TryRuntimeError::Other(
                "WakeupPage slot addresses an actor with a different clock-domain pointer",
              ));
            }
            None => {}
          }
        }
      }
      // One Active Actor may own one Pipeline-service pointer and one Trigger-temporal pointer.
      // Stale physical pages and cursor blocks may legitimately outlive their actors.
      if live_wakeup_memberships > active_count.saturating_mul(2) {
        return Err(TryRuntimeError::Other(
          "live wakeup memberships exceed dual active-actor capacity",
        ));
      }
      for block in WakeupBuckets::<T>::iter_keys() {
        let bucket = WakeupBuckets::<T>::get(block)
          .ok_or(TryRuntimeError::Other("wakeup bucket key has no value"))?;
        if wakeup_live_by_key.get(&block).copied() != Some(bucket.live_entries) {
          return Err(TryRuntimeError::Other(
            "WakeupBucket live-entry count disagrees with pages",
          ));
        }
        if !WakeupPages::<T>::contains_key((block, bucket.head_page))
          || !WakeupPages::<T>::contains_key((block, bucket.tail_page))
        {
          return Err(TryRuntimeError::Other(
            "WakeupBucket head or tail page is missing",
          ));
        }
        let cursor_len = WakeupCursorLen::<T>::get(block.clock());
        if let Some(index) = bucket.cursor_index
          && (index >= cursor_len || Self::wakeup_cursor_get(block.clock(), index) != Some(block))
        {
          return Err(TryRuntimeError::Other(
            "WakeupBucket cursor reverse index does not resolve",
          ));
        }
      }
      let cursor_page_size = T::WakeupPageSize::get();
      for clock in [WakeupClock::Block, WakeupClock::Tick] {
        let cursor_len = WakeupCursorLen::<T>::get(clock);
        if cursor_len > T::MaxActiveActors::get() {
          return Err(TryRuntimeError::Other(
            "WakeupCursorLen exceeds configured active actor capacity",
          ));
        }
        let expected_cursor_pages = cursor_len.div_ceil(cursor_page_size);
        let actual_cursor_pages = u32::try_from(
          WakeupCursorPages::<T>::iter_keys()
            .filter(|(stored_clock, _)| *stored_clock == clock)
            .count(),
        )
        .map_err(|_| TryRuntimeError::Other("wakeup cursor page count overflows"))?;
        if actual_cursor_pages != expected_cursor_pages {
          return Err(TryRuntimeError::Other(
            "WakeupCursorPages count disagrees with cursor length",
          ));
        }
        for page_id in 0..expected_cursor_pages {
          let Some(page) = WakeupCursorPages::<T>::get((clock, u64::from(page_id))) else {
            return Err(TryRuntimeError::Other(
              "WakeupCursorPages has a gap in logical page order",
            ));
          };
          let consumed = page_id
            .checked_mul(cursor_page_size)
            .ok_or(TryRuntimeError::Other(
              "wakeup cursor page offset overflows",
            ))?;
          let remaining = cursor_len
            .checked_sub(consumed)
            .ok_or(TryRuntimeError::Other(
              "wakeup cursor page offset exceeds length",
            ))?;
          let expected_len = remaining.min(cursor_page_size) as usize;
          if page.len() != expected_len {
            return Err(TryRuntimeError::Other(
              "WakeupCursorPage length disagrees with logical position",
            ));
          }
        }
        let mut cursor_keys = alloc::collections::BTreeSet::new();
        for index in 0..cursor_len {
          let Some(key) = Self::wakeup_cursor_get(clock, index) else {
            return Err(TryRuntimeError::Other(
              "WakeupCursor index does not resolve to a page entry",
            ));
          };
          if key.clock() != clock || !cursor_keys.insert(key) {
            return Err(TryRuntimeError::Other(
              "WakeupCursor contains a duplicate or wrong-clock key",
            ));
          }
          if WakeupBuckets::<T>::get(key).and_then(|bucket| bucket.cursor_index) != Some(index) {
            return Err(TryRuntimeError::Other(
              "WakeupCursor key has no matching bucket reverse index",
            ));
          }
          if index > 0 {
            let parent = index
              .checked_sub(1)
              .ok_or(TryRuntimeError::Other("wakeup cursor parent underflows"))?
              / 2;
            if Self::wakeup_cursor_get(clock, parent).is_none_or(|parent_key| parent_key > key) {
              return Err(TryRuntimeError::Other(
                "WakeupCursor violates min-heap ordering",
              ));
            }
          }
        }
      }
      let mut live_wakeup_pointers = alloc::collections::BTreeSet::new();
      for actor_id in ActorHot::<T>::iter_keys() {
        let hot = ActorHot::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
          "wakeup-pointer hot key has no value",
        ))?;
        if let Some(pointer) = hot.wakeup_pointer {
          if !live_wakeup_pointers.insert((pointer.block, pointer.page_id, pointer.slot)) {
            return Err(TryRuntimeError::Other(
              "multiple actors own the same wakeup pointer",
            ));
          }
          if !Self::wakeup_page_entry_matches(pointer, actor_id) {
            return Err(TryRuntimeError::Other(
              "ActorHot Pipeline wakeup pointer does not resolve to its actor",
            ));
          }
          if let Some(terminal_at) = hot.terminal_at
            && !matches!(pointer.block, WakeupKey::Block(block) if block <= terminal_at)
          {
            return Err(TryRuntimeError::Other(
              "ActorHot Pipeline wakeup pointer exceeds its terminal membership",
            ));
          }
        }
        if let Some(trigger_pointer) = hot.trigger_wakeup_pointer {
          let pointer = WakeupPointer {
            block: WakeupKey::Tick(trigger_pointer.tick),
            page_id: trigger_pointer.page_id,
            slot: trigger_pointer.slot,
          };
          if !matches!(
            hot.trigger_runtime_state,
            TriggerRuntimeState::AtTime {
              consumed: false,
              ..
            } | TriggerRuntimeState::Cadenced { .. }
          ) {
            return Err(TryRuntimeError::Other(
              "non-pending temporal Actor owns a Trigger wakeup pointer",
            ));
          }
          if !live_wakeup_pointers.insert((pointer.block, pointer.page_id, pointer.slot)) {
            return Err(TryRuntimeError::Other(
              "multiple actors own the same wakeup pointer",
            ));
          }
          if !Self::wakeup_page_entry_matches(pointer, actor_id) {
            return Err(TryRuntimeError::Other(
              "ActorHot Trigger wakeup pointer does not resolve to its actor",
            ));
          }
        }
      }
      let next_id = NextActorId::<T>::get();
      if let Some(max_actor_id) = max_id {
        if next_id <= max_actor_id {
          return Err(TryRuntimeError::Other(
            "NextActorId is not greater than the largest active actor_id",
          ));
        }
      }
      let mut system_sovereign_count = 0u32;
      for sovereign_id in SystemSovereigns::<T>::iter_keys() {
        let state = SystemSovereigns::<T>::get(sovereign_id)
          .ok_or(TryRuntimeError::Other("System sovereign key has no value"))?;
        system_sovereign_count = system_sovereign_count
          .checked_add(1)
          .ok_or(TryRuntimeError::Other("System sovereign count overflow"))?;
        if let SystemSovereignState::Occupied(actor_id) = state {
          let class = Self::active_actor_view(actor_id)
            .map(|actor| actor.actor_class)
            .or_else(|| ActorIdentities::<T>::get(actor_id).map(|identity| identity.actor_class));
          if class != Some(ActorClass::System { sovereign_id }) {
            return Err(TryRuntimeError::Other(
              "occupied System sovereign locator has no matching actor identity",
            ));
          }
        }
      }
      if system_sovereign_count != SystemSovereignCount::<T>::get()
        || system_sovereign_count > T::MaxSystemSovereigns::get()
      {
        return Err(TryRuntimeError::Other(
          "SystemSovereignCount disagrees with bounded locator registry",
        ));
      }
      let mut owner_hold_totals = alloc::collections::BTreeMap::new();
      for (actor_id, identity) in ActorIdentities::<T>::iter() {
        match identity.actor_class.actor_type() {
          ActorType::System => {
            if ActorStateHolds::<T>::contains_key(actor_id) {
              return Err(TryRuntimeError::Other(
                "System Actor retains a User Actor-state hold",
              ));
            }
          }
          ActorType::User => {
            let expected = Self::derive_actor_state_hold(actor_id, &identity).map_err(|_| {
              TryRuntimeError::Other("User Actor-state hold geometry cannot be rederived")
            })?;
            let record = ActorStateHolds::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
              "User Actor has no Actor-state hold record",
            ))?;
            if record.owner != identity.owner || record.breakdown != expected {
              return Err(TryRuntimeError::Other(
                "User Actor-state hold record disagrees with retained geometry",
              ));
            }
            let total = Self::state_hold_total(&expected).map_err(|_| {
              TryRuntimeError::Other("User Actor-state hold total cannot be rederived")
            })?;
            let owner_total = owner_hold_totals
              .entry(record.owner)
              .or_insert(T::Balance::zero());
            *owner_total = owner_total
              .checked_add(&total)
              .ok_or(TryRuntimeError::Other(
                "aggregate User Actor-state hold overflows",
              ))?;
          }
        }
      }
      for (actor_id, _) in ActorStateHolds::<T>::iter() {
        // deos-bypass: bounded-iter -- MaxActorIdentities bounds per-Actor hold records.
        if !ActorIdentities::<T>::contains_key(actor_id) {
          return Err(TryRuntimeError::Other(
            "Actor-state hold record has no Actor identity",
          ));
        }
      }
      let hold_reason: T::RuntimeHoldReason = HoldReason::ActorState.into();
      for (owner, expected) in owner_hold_totals {
        if T::StateHoldCurrency::balance_on_hold(&hold_reason, &owner) != expected {
          return Err(TryRuntimeError::Other(
            "owner Actor-state hold balance disagrees with per-Actor records",
          ));
        }
      }
      Self::do_try_state_observation_subscriptions()?;
      Self::do_try_state_dirty_observations()?;
      Self::do_try_state_crossing()?;
      Ok(())
    }
  }
}
