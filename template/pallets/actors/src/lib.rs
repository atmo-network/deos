#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "512"]

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

pub use scheduler::{EnqueueOutcome, WakeupBucketDisposition};

pub mod adapters;
pub use adapters::{
  AddressEventIngress, AdmissionCertificateAuthority, AdmissionCertificateAuthorityProvider,
  AssetOps, CanonicalObservationState, DependencyEventIngress, DexOps, DexSwapOutcome,
  ExecutionContext, FundingAuthority, IngressFailure, LiquidityOps, ObservationProvider,
  ObservationTransition, ObservationTransitionIngress, RetryClass, ScalarObservationState,
  SovereignAccountDeriver, StakingOps, StepControlExecution, StepControlOutcome, StepControlPhase,
  StepControlPlacement, StepControlWeightContext, StepControlWeightProvider,
  SystemActorContractValidator, TaskEffectExecution, TaskEffectWeightProvider, TaskFailure,
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

/// Host read branches used by observation capture diagnostics.
#[cfg(feature = "runtime-benchmarks")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BenchmarkObservationReadState {
  Missing,
  Deactivated,
  Uninitialized,
  Stale,
  Fresh,
}

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AccountId, AssetId, Balance, ObservationFeedId> {
  fn setup_add_liquidity(
    owner: &AccountId,
  ) -> Result<(AssetId, AssetId, Balance, Balance), polkadot_sdk::sp_runtime::DispatchError>;
  /// Change host account spendability through its authorized lifecycle without moving custody.
  fn set_asset_account_frozen(
    _owner: &AccountId,
    _who: &AccountId,
    _asset: AssetId,
    _frozen: bool,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkAssetFreezeUnsupported",
    ))
  }
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
  /// Prepare distinct funded positions with maximum encoded host receipt-account state.
  fn setup_max_encoded_staking_positions(
    _owner: &AccountId,
    _max: u32,
  ) -> Result<alloc::vec::Vec<(AssetId, Balance)>, polkadot_sdk::sp_runtime::DispatchError> {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkMaximumStakingEncodingUnsupported",
    ))
  }
  /// Remove an empty receipt through the host lifecycle after an Unstake Contract is admitted.
  fn remove_empty_staking_receipt(
    _owner: &AccountId,
    _asset: AssetId,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkReceiptRemovalUnsupported",
    ))
  }
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
  /// Prepare distinct assets with maximum encoded host balance-account state.
  fn setup_max_encoded_predicate_assets(
    _owner: &AccountId,
    _max: u32,
  ) -> Result<alloc::vec::Vec<AssetId>, polkadot_sdk::sp_runtime::DispatchError> {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkMaximumAssetEncodingUnsupported",
    ))
  }
  fn setup_observation_feeds(
    max: u32,
  ) -> Result<alloc::vec::Vec<ObservationFeedId>, polkadot_sdk::sp_runtime::DispatchError>;
  /// Prepare published feeds with maximum encoded host key, configuration and observation widths.
  fn setup_max_encoded_observation_feeds(
    _max: u32,
  ) -> Result<alloc::vec::Vec<ObservationFeedId>, polkadot_sdk::sp_runtime::DispatchError> {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkMaximumObservationEncodingUnsupported",
    ))
  }
  /// Prepare maximum-width observation reads, keeping populated neighbors for absent values.
  fn setup_observation_read_state(
    _max: u32,
    _state: BenchmarkObservationReadState,
    _max_age_blocks: u32,
  ) -> Result<alloc::vec::Vec<ObservationFeedId>, polkadot_sdk::sp_runtime::DispatchError> {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkObservationReadStateUnsupported",
    ))
  }
  fn enable_asset_ops_ingress() {}
  /// Publish a sample through the host observation owner, including revision and Actor ingress.
  fn publish_observation(
    _feed: ObservationFeedId,
    _sample: u128,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkObservationPublishUnsupported",
    ))
  }
  /// Make a feed unavailable through the host observation owner's lifecycle.
  fn deactivate_observation_feed(
    _feed: ObservationFeedId,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkObservationDeactivateUnsupported",
    ))
  }
  /// Begin the next fixture block and advance its authoritative clock to the requested tick.
  fn advance_to_scheduler_tick(_tick: u64) -> polkadot_sdk::sp_runtime::DispatchResult {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkClockAdvanceUnsupported",
    ))
  }
  /// Finalize the host clock before a fixture advances to another block.
  fn finalize_scheduler_clock() -> polkadot_sdk::sp_runtime::DispatchResult {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkClockFinalizeUnsupported",
    ))
  }
  /// Encoded Contract-head witness only; collection does not require a published observation.
  fn contract_head_observation_feed()
  -> Result<ObservationFeedId, polkadot_sdk::sp_runtime::DispatchError> {
    Self::setup_observation_feeds(1)?.into_iter().next().ok_or(
      polkadot_sdk::sp_runtime::DispatchError::Other("BenchmarkContractHeadFeedUnsupported"),
    )
  }
  /// Move custody through the host's signed-transfer producer, including certified ingress.
  fn transfer_signed(
    _source: &AccountId,
    _recipient: &AccountId,
    _asset: AssetId,
    _amount: Balance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "BenchmarkSignedTransferUnsupported",
    ))
  }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeeAssetClass {
  FeeNative,
  Other,
}

/// Returns the preserve-spend floor for a direct or adapter-reported debit surface.
pub fn fee_native_protected_minimum<Balance: Ord>(
  actor_type: types::ActorType,
  asset_class: FeeAssetClass,
  asset_minimum: Balance,
  min_user_balance: Balance,
) -> Balance {
  if actor_type == types::ActorType::User && asset_class == FeeAssetClass::FeeNative {
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
  #[api_version(2)]
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
      budget: types::SimulationBudget,
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
  use crate::scheduler::{CyclePass, ServiceCutoff};
  use alloc::vec::Vec;
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
  pub(crate) enum MaterializationMinimumReservation {
    ReserveAllFamilies,
    Unavailable,
  }

  impl MaterializationMinimumReservation {
    fn reserves_all_families(self) -> bool {
      matches!(self, Self::ReserveAllFamilies)
    }
  }

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

  pub type ActorStepAuthorityOf<T> =
    ActorStepAuthority<BlockNumberFor<T>, ActorContractCommitment<[u8; 32]>>;

  pub type LoadedActorStepOf<T> = LoadedActorStep<StepOf<T>>;

  pub type CurrentStepPlanOf<T> = StepExecutionPlan<
    ActorIdentityOf<T>,
    ActorHotStateOf<T>,
    ActorRunStateOf<T>,
    ActorAdmissionCertificateOf<T>,
    ActorStepAuthorityOf<T>,
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

  pub type ActorRunHeadOf<T> = ActorRunHead<BlockNumberFor<T>>;

  pub type ActorRunPayloadOf<T> = ActorRunPayload<
    <T as Config>::AssetId,
    <T as Config>::Balance,
    <T as Config>::MaxOpeningSnapshotEntries,
  >;

  pub type ActorRunStateOf<T> = ActorRunState<
    <T as Config>::AssetId,
    <T as Config>::Balance,
    BlockNumberFor<T>,
    <T as Config>::MaxOpeningSnapshotEntries,
  >;

  pub type QueuePageOf<T> = BoundedVec<QueueEntry<BlockNumberFor<T>>, <T as Config>::QueuePageSize>;

  /// Canonical movable identity authority. Sovereign account is derived from owner/class and is not
  /// duplicated in every control cell.
  #[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
  )]
  pub struct ActorControlIdentity<AccountId, BlockNumber> {
    pub owner: AccountId,
    pub actor_class: ActorClass,
    pub mutability: Mutability,
    pub cycle_nonce: u64,
    pub last_control_mutation_block: BlockNumber,
  }

  /// Canonical mutable control authority. Ready ticket is represented by physical location; both
  /// wakeup pointers remain explicit because process and Trigger temporal memberships can coexist.
  #[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
  )]
  pub struct ActorControlHotState<BlockNumber> {
    pub lifecycle: ActiveLifecycle,
    pub cycle_state: CycleState,
    pub trigger_runtime_state: TriggerRuntimeState,
    pub unsuccessful_attempt_streak: u32,
    pub pending_signal: bool,
    pub wakeup_pointer: Option<WakeupPointer<BlockNumber>>,
    pub trigger_wakeup_pointer: Option<TriggerWakeupPointer>,
    pub terminal_at: Option<BlockNumber>,
    pub schedule_anchor: BlockNumber,
    pub last_cycle_block: Option<BlockNumber>,
  }

  /// Non-placement semantic authority prepared for the atomic scheduler cutover. Cursor,
  /// eligibility, and current-Step resources are intentionally absent: the run record owns the
  /// first two while contract geometry at that cursor owns the last.
  #[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
  )]
  pub struct ActorSemanticRecord<Identity, Hot, Admission> {
    pub identity: Identity,
    /// Stable Contract-generation authority used by every generation-bound carrier.
    pub generation: u64,
    pub hot: Hot,
    pub admission: Admission,
  }

  /// Dormant semantic authority preserves the last published Contract generation. Generation zero
  /// means that this identity has never published a Contract.
  #[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
  )]
  pub struct DormantActorSemanticRecord<Identity> {
    pub identity: Identity,
    pub generation: u64,
  }

  /// Complete lifecycle shape for the future actor-keyed semantic owner. Active zero-Step Actors
  /// still use `Active` because they retain generation, hot, and admission semantics even though
  /// execution projection has no current Step.
  #[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
  )]
  pub enum ActorSemanticState<Identity, Hot, Admission> {
    Dormant(DormantActorSemanticRecord<Identity>),
    Active(ActorSemanticRecord<Identity, Hot, Admission>),
  }

  pub fn next_actor_generation(current: u64) -> Option<u64> {
    current.checked_add(1).filter(|generation| *generation != 0)
  }

  #[derive(Clone, Copy, Debug, Eq, PartialEq)]
  pub enum ActorSemanticLoadError {
    Corrupt,
  }

  /// Complete storage-neutral operation set for the future actor-keyed semantic owner. Every
  /// update is a compare-and-replace of the whole bounded record, so independently authored field
  /// patches cannot silently overwrite one another. Placement-only transitions need no operation.
  #[derive(Clone, Debug, Eq, PartialEq)]
  pub enum ActorSemanticMutation<Record> {
    Publish(Record),
    Replace {
      expected: Record,
      replacement: Record,
    },
    Remove {
      expected: Record,
    },
  }

  #[derive(Clone, Copy, Debug, Eq, PartialEq)]
  pub enum ActorSemanticMutationError {
    AlreadyPublished,
    Missing,
    Stale,
  }

  pub fn apply_actor_semantic_mutation<Record: Clone + Eq>(
    current: Option<&Record>,
    mutation: &ActorSemanticMutation<Record>,
  ) -> Result<Option<Record>, ActorSemanticMutationError> {
    match (current, mutation) {
      (None, ActorSemanticMutation::Publish(record)) => Ok(Some(record.clone())),
      (Some(_), ActorSemanticMutation::Publish(_)) => {
        Err(ActorSemanticMutationError::AlreadyPublished)
      }
      (
        Some(current),
        ActorSemanticMutation::Replace {
          expected,
          replacement,
        },
      ) if current == expected => Ok(Some(replacement.clone())),
      (Some(current), ActorSemanticMutation::Remove { expected }) if current == expected => {
        Ok(None)
      }
      (None, ActorSemanticMutation::Replace { .. } | ActorSemanticMutation::Remove { .. }) => {
        Err(ActorSemanticMutationError::Missing)
      }
      (Some(_), ActorSemanticMutation::Replace { .. } | ActorSemanticMutation::Remove { .. }) => {
        Err(ActorSemanticMutationError::Stale)
      }
    }
  }

  /// Storage-free projection of fields currently duplicated by placement cells.
  #[derive(Clone, Copy, Debug, Eq, PartialEq)]
  pub struct ActorSemanticExecutionProjection<BlockNumber> {
    pub cursor: u32,
    pub eligible_at: Option<BlockNumber>,
    pub resources: ActorStepResourceEnvelope,
  }

  #[derive(Clone, Copy, Debug, Eq, PartialEq)]
  pub enum ActorSemanticProjectionError {
    RunStateMismatch,
    CurrentStepMissing,
  }

  /// Projects execution fields from their existing canonical owners without making the semantic
  /// record a second cursor or resource authority.
  pub fn project_actor_semantic_execution<BlockNumber: Copy>(
    cycle_state: CycleState,
    run: Option<(u32, BlockNumber)>,
    current_step_resources: Option<ActorStepResourceEnvelope>,
  ) -> Result<ActorSemanticExecutionProjection<BlockNumber>, ActorSemanticProjectionError> {
    let (cursor, eligible_at) = match (cycle_state, run) {
      (CycleState::Idle, None) => (0, None),
      (CycleState::Running | CycleState::Suspended, Some((cursor, eligible_at))) => {
        (cursor, Some(eligible_at))
      }
      _ => return Err(ActorSemanticProjectionError::RunStateMismatch),
    };
    let resources =
      current_step_resources.ok_or(ActorSemanticProjectionError::CurrentStepMissing)?;
    Ok(ActorSemanticExecutionProjection {
      cursor,
      eligible_at,
      resources,
    })
  }

  /// Canonical single-owner control cell.
  #[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
  )]
  pub struct ActorControlCell<AccountId, BlockNumber, Admission> {
    pub actor_id: ActorId,
    pub identity: ActorControlIdentity<AccountId, BlockNumber>,
    pub hot: ActorControlHotState<BlockNumber>,
    pub pipeline_service_identity: [u8; 32],
    pub cursor: u32,
    pub eligible_at: Option<BlockNumber>,
    pub admission: Admission,
    pub resources: ActorStepResourceEnvelope,
  }

  pub type DormantActorSemanticRecordOf<T> = DormantActorSemanticRecord<ActorIdentityOf<T>>;
  pub type ActorSemanticRecordOf<T> =
    ActorSemanticRecord<ActorIdentityOf<T>, ActorHotStateOf<T>, ActorAdmissionCertificateOf<T>>;
  pub type ActorSemanticStateOf<T> =
    ActorSemanticState<ActorIdentityOf<T>, ActorHotStateOf<T>, ActorAdmissionCertificateOf<T>>;

  pub type ActorControlCellOf<T> = ActorControlCell<
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T>,
    ActorAdmissionCertificateOf<T>,
  >;
  pub type ActorControlChunkOf<T> = BoundedVec<Option<ActorControlCellOf<T>>, ConstU32<32>>;

  /// A deadline membership points at the single control owner without copying mutable authority.
  /// Its clock is the containing Waiting key; the primary must carry the exact matching pointer.
  #[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
  )]
  pub struct ActorWakeupReference {
    pub actor_id: ActorId,
    pub admission_identity: [u8; 32],
  }

  #[derive(
    Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
  )]
  pub enum ActorWaitingEntry<Cell> {
    Primary(Cell),
    Reference(ActorWakeupReference),
  }

  impl<Cell> ActorWaitingEntry<Cell> {
    pub fn primary(&self) -> Option<&Cell> {
      match self {
        Self::Primary(cell) => Some(cell),
        Self::Reference(_) => None,
      }
    }

    pub fn primary_mut(&mut self) -> Option<&mut Cell> {
      match self {
        Self::Primary(cell) => Some(cell),
        Self::Reference(_) => None,
      }
    }

    pub fn into_primary(self) -> Option<Cell> {
      match self {
        Self::Primary(cell) => Some(cell),
        Self::Reference(_) => None,
      }
    }
  }

  pub type ActorWaitingChunkOf<T> =
    BoundedVec<Option<ActorWaitingEntry<ActorControlCellOf<T>>>, ConstU32<32>>;
  pub type ActorWaitingPageOf<T> = WakeupPage<ActorWaitingChunkOf<T>>;
  pub type DeadlineHandleOf<T> = DeadlineHandle<BlockNumberFor<T>>;
  pub type DeadlineIndexPageOf<T> = BoundedVec<WakeupKey<BlockNumberFor<T>>, ConstU32<32>>;

  /// External-boundary location; execution writes but never reads this index.
  #[derive(
    Clone,
    Copy,
    Debug,
    Decode,
    DecodeWithMemTracking,
    Encode,
    Eq,
    MaxEncodedLen,
    PartialEq,
    TypeInfo,
  )]
  pub enum ActorControlLocation<BlockNumber> {
    Unsignaled,
    Waiting {
      key: WakeupKey<BlockNumber>,
      page: u64,
      slot: u8,
    },
    Ready {
      ticket: QueueTicket,
    },
  }

  // Temporary source aliases remain only while the atomic cutover is assembled in this
  // diagnostic worktree; the retained production patch removes them after caller conversion.

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
  pub type ActorProcessOf<T> = ActorProcess<BlockNumberFor<T>>;

  pub type ActorIdentityOf<T> =
    ActorIdentity<<T as frame_system::Config>::AccountId, BlockNumberFor<T>>;

  pub type ActorStateHoldBreakdownOf<T> = ActorStateHoldBreakdown<<T as Config>::Balance>;
  pub type ActorStateHoldRecordOf<T> =
    ActorStateHoldRecord<<T as frame_system::Config>::AccountId, <T as Config>::Balance>;

  pub type ActiveActorStateOf<T> = ActiveActorState<
    ActorIdentityOf<T>,
    ActorHotStateOf<T>,
    ActorContractOf<T>,
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
    pub admission: Option<ActorAdmissionCertificateOf<T>>,
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
  #[pallet::storage_prefix = "ActorContractHead"]
  pub type ActorContractHeads<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorContractHeadOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::storage_prefix = "ActorActivationAuthority"]
  pub type ActorActivationAuthorities<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorActivationAuthorityOf<T>, OptionQuery>;

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
      let replacing = ActorContractHeads::<T>::contains_key(actor_id);
      // A canonically published Actor owns no legacy primary or unsignaled cell. Its Contract
      // replacement must rotate the generation-bound process/residence carriers together with
      // the geometry instead of mirroring a physical primary.
      let canonical_replace = replacing
        && !ActorControlLocators::<T>::contains_key(actor_id)
        && !ActorUnsignaledControlCells::<T>::contains_key(actor_id);
      let stored = if replacing {
        if canonical_replace {
          Self::replace_canonical_contract_geometry(actor_id, &contract, &certificate)
        } else {
          Self::replace_admitted_contract_geometry(actor_id, &contract, &certificate)
        }
      } else {
        let actor_type = Self::load_frame_control_authority(actor_id)
          .map(|(_, identity, _, _)| identity.actor_class.actor_type())
          .or_else(|| match ActorSemanticStates::<T>::get(actor_id) {
            Some(ActorSemanticState::Active(record)) => {
              Some(record.identity.actor_class.actor_type())
            }
            _ => None,
          })
          .ok_or(Error::<T>::ActorInvariant)?;
        Self::insert_admitted_contract_geometry_with_actor_type(
          actor_id,
          actor_type,
          &contract,
          &certificate,
        )
      };
      ensure!(stored, Error::<T>::ActorInvariant);
      if CrossingMemberships::<T>::contains_key(actor_id)
        && let Some(crossing) = Self::crossing_from_trigger(&contract.trigger)
      {
        let runtime_state = Self::load_frame_control_authority(actor_id)
          .map(|(_, _, hot, _)| hot.trigger_runtime_state)
          .or_else(|| match ActorSemanticStates::<T>::get(actor_id) {
            Some(ActorSemanticState::Active(record)) => Some(record.hot.trigger_runtime_state),
            _ => None,
          });
        let phase = match runtime_state {
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
      if canonical_replace {
        Self::republish_canonical_contract(actor_id, &contract, &certificate)
          .map_err(Self::placement_error)?;
      }
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

    #[cfg(feature = "runtime-benchmarks")]
    pub(crate) fn remove_actor_contract(actor_id: ActorId) -> DispatchResult {
      ensure!(
        Self::remove_admitted_contract_geometry(actor_id).is_some(),
        Error::<T>::ActorInvariant
      );
      Ok(())
    }

    #[cfg(all(test, feature = "runtime-benchmarks"))]
    pub(crate) fn control_remove_frame_owned_contract_geometry(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
    ) -> bool {
      if !ActorContractHeads::<T>::contains_key(actor_id) {
        return false;
      }
      let Some(chunk_count) = u32::try_from(contract.steps.len())
        .ok()
        .map(|count| count.saturating_sub(1).div_ceil(MAX_STEPS_PER_TAIL_CHUNK))
      else {
        return false;
      };
      ActorActivationAuthorities::<T>::remove(actor_id);
      ActorContractHeads::<T>::remove(actor_id);
      for chunk_index in 0..chunk_count {
        ActorContractTailChunks::<T>::remove(actor_id, chunk_index);
      }
      true
    }

    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn insert_admitted_contract_geometry(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> bool {
      // Deliberately malformed storage fixtures may have no identity owner. They retain System's
      // zero-fee projection solely as a package-test corruption seam.
      let actor_type = Self::load_control_identity(actor_id)
        .map(|identity| identity.actor_class.actor_type())
        .unwrap_or(ActorType::System);
      Self::insert_admitted_contract_geometry_with_actor_type(
        actor_id,
        actor_type,
        contract,
        certificate,
      )
    }

    fn insert_admitted_contract_geometry_with_actor_type(
      actor_id: ActorId,
      actor_type: ActorType,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> bool {
      if ActorContractHeads::<T>::contains_key(actor_id) {
        return false;
      }
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
      ActorContractHeads::<T>::insert(actor_id, head);
      for (chunk_index, chunk) in chunks {
        ActorContractTailChunks::<T>::insert(actor_id, chunk_index, chunk);
      }
      true
    }

    pub(crate) fn load_contract_geometry_with_admission(
      actor_id: ActorId,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> Option<ActorContractOf<T>> {
      let head = ActorContractHeads::<T>::get(actor_id)?;
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
      Self::reconstruct_contract_geometry(actor_id, head, &chunks)
    }

    pub(crate) fn load_admitted_contract_geometry(
      actor_id: ActorId,
    ) -> Option<(ActorContractOf<T>, ActorAdmissionCertificateOf<T>)> {
      let certificate = Self::load_control_admission(actor_id)?;
      let contract = Self::load_contract_geometry_with_admission(actor_id, &certificate)?;
      Some((contract, certificate))
    }

    pub(crate) fn load_current_step_from_storage(
      actor_id: ActorId,
      cursor: u32,
    ) -> Option<LoadedActorStepOf<T>> {
      let certificate = Self::load_control_admission(actor_id)?;
      Self::load_current_step_with_admission(actor_id, cursor, &certificate)
    }

    pub(crate) fn load_current_step_with_admission(
      actor_id: ActorId,
      cursor: u32,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> Option<LoadedActorStepOf<T>> {
      let head = ActorContractHeads::<T>::get(actor_id)?;
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
        certificate,
        cursor,
        tail_chunk.as_ref().map(|(index, chunk)| (*index, chunk)),
      )
    }

    pub(crate) fn replace_admitted_contract_geometry(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> bool {
      let Some((_, identity, _, admission)) = Self::load_frame_control_authority(actor_id) else {
        return false;
      };
      let Some(current_contract) =
        Self::load_contract_geometry_with_admission(actor_id, &admission)
      else {
        return false;
      };
      let actor_type = identity.actor_class.actor_type();
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
      if !Self::replace_control_admission_for_transition(actor_id, certificate, contract) {
        return false;
      }
      ActorContractHeads::<T>::insert(actor_id, head);
      for (chunk_index, chunk) in chunks {
        ActorContractTailChunks::<T>::insert(actor_id, chunk_index, chunk);
      }
      for chunk_index in new_chunk_count..old_chunk_count {
        ActorContractTailChunks::<T>::remove(actor_id, chunk_index);
      }
      true
    }

    /// Replaces Contract geometry and admission for a canonically published Actor, rotating the
    /// generation-bound carriers instead of mirroring a legacy primary. The current process
    /// residence and any independent temporal Trigger deadline are released before the new
    /// geometry commits; the caller republishes one complete canonical successor afterwards.
    fn replace_canonical_contract_geometry(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> bool {
      let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor_id) else {
        return false;
      };
      let Some(process) = ActorProcesses::<T>::get(actor_id)
        .filter(|process| process.generation == record.generation)
      else {
        return false;
      };
      let actor = ActorRef {
        actor_id,
        generation: record.generation,
      };
      let Some(current_contract) =
        Self::load_contract_geometry_with_admission(actor_id, &record.admission)
      else {
        return false;
      };
      let actor_type = record.identity.actor_class.actor_type();
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
      if TriggerDeadlineHandles::<T>::contains_key(actor_id)
        && Self::remove_trigger_deadline_member(actor).is_err()
      {
        return false;
      }
      match process.residence {
        Some(ProcessResidence::Service(_)) => {
          if Self::remove_service_member(actor).is_err() {
            return false;
          }
        }
        Some(ProcessResidence::Deadline { .. }) => {
          if Self::remove_deadline_member(actor).is_err() {
            return false;
          }
        }
        None if matches!(process.status, ProcessStatus::Disabled(_)) => {}
        _ => return false,
      }
      ActorProcesses::<T>::remove(actor_id);
      ActorContractHeads::<T>::insert(actor_id, head);
      for (chunk_index, chunk) in chunks {
        ActorContractTailChunks::<T>::insert(actor_id, chunk_index, chunk);
      }
      for chunk_index in new_chunk_count..old_chunk_count {
        ActorContractTailChunks::<T>::remove(actor_id, chunk_index);
      }
      let Some(next_generation) = next_actor_generation(record.generation) else {
        return false;
      };
      let mut updated = record.clone();
      // The release above removed the superseded `TriggerDeadlineHandles` member. A schedule
      // replacement must also drop the semantic pointer so the successor publication re-derives
      // its temporal deadline from the replacement anchor instead of re-using the replaced
      // cadence/AtTime tick; an unchanged schedule keeps the exact outstanding tick.
      if current_contract.trigger != contract.trigger
        || current_contract.cooldown_blocks != contract.cooldown_blocks
        || current_contract.window != contract.window
      {
        updated.hot.trigger_wakeup_pointer = None;
      }
      updated.admission = certificate.clone();
      updated.generation = next_generation;
      Self::mutate_actor_semantic_state(
        actor_id,
        ActorSemanticMutation::Replace {
          expected: ActorSemanticState::Active(record),
          replacement: ActorSemanticState::Active(updated),
        },
      )
      .is_ok()
    }

    /// Republishes one complete canonical process/residence carrier for a freshly replaced
    /// Contract. The replacement already released every old carrier and rotated the semantic
    /// generation, so this seam plans and commits the successor under the new `ActorRef`.
    fn republish_canonical_contract(
      actor_id: ActorId,
      contract: &ActorContractOf<T>,
      certificate: &ActorAdmissionCertificateOf<T>,
    ) -> Result<(), crate::scheduler::EnqueueOutcome> {
      let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor_id) else {
        return Err(crate::scheduler::EnqueueOutcome::CorruptedTopology);
      };
      if record.admission != *certificate || ActorRunStateStore::<T>::contains_key(actor_id) {
        return Err(crate::scheduler::EnqueueOutcome::CorruptedTopology);
      }
      let resources = if contract.steps.is_empty() {
        ActorStepResourceEnvelope {
          control: T::WeightInfo::scheduler_inner_zero_step_complete(),
          effect: Weight::zero(),
        }
      } else {
        Self::derive_step_resource_envelopes(contract)
          .and_then(|envelopes| envelopes.first().copied())
          .ok_or(crate::scheduler::EnqueueOutcome::CorruptedTopology)?
      };
      let state = ActiveActorState {
        identity: record.identity,
        hot: record.hot,
        contract: contract.clone(),
        run_state: None,
      };
      Self::publish_actor_publication(
        ActorRef {
          actor_id,
          generation: record.generation,
        },
        &state,
        None,
        resources,
        frame_system::Pallet::<T>::block_number(),
        ServiceCutoff::Open,
      )
    }

    pub(crate) fn remove_admitted_contract_geometry(
      actor_id: ActorId,
    ) -> Option<ActorContractOf<T>> {
      let (contract, _) = Self::load_admitted_contract_geometry(actor_id)?;
      Self::remove_loaded_contract_geometry(actor_id, contract)
    }

    pub(crate) fn remove_admitted_contract_geometry_with_admission(
      actor_id: ActorId,
      admission: &ActorAdmissionCertificateOf<T>,
    ) -> Option<ActorContractOf<T>> {
      let contract = Self::load_contract_geometry_with_admission(actor_id, admission)?;
      Self::remove_loaded_contract_geometry(actor_id, contract)
    }

    fn remove_loaded_contract_geometry(
      actor_id: ActorId,
      contract: ActorContractOf<T>,
    ) -> Option<ActorContractOf<T>> {
      let chunk_count = u32::try_from(contract.steps.len())
        .ok()?
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK);
      ActorActivationAuthorities::<T>::remove(actor_id);
      ActorContractHeads::<T>::remove(actor_id);
      for chunk_index in 0..chunk_count {
        ActorContractTailChunks::<T>::remove(actor_id, chunk_index);
      }
      Some(contract)
    }

    #[cfg(test)]
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
      })
    }

    fn opening_control_geometry(_steps: &ContractSteps<T>) -> Option<u32> {
      Some(0)
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
      let opening_snapshot_entries = if cursor == 0 {
        if instance.cycle_state == CycleState::Idle {
          Self::opening_control_geometry(&instance.steps)?
        } else {
          u32::try_from(run?.opening_snapshot.len()).ok()?
        }
      } else {
        0
      };
      Self::step_control_weight_context(
        step_count,
        cursor,
        predicate_evaluation_units,
        opening_snapshot_entries,
      )
    }

    pub(crate) fn derive_step_resource_envelopes(
      contract: &ActorContractOf<T>,
    ) -> Option<ActorAdmissionResourcesOf<T>> {
      let step_count = u32::try_from(contract.steps.len()).ok()?;
      let opening_snapshot_entries = Self::opening_control_geometry(&contract.steps)?;
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

    pub(crate) fn build_admission_certificate(
      contract: &ActorContractOf<T>,
    ) -> Option<ActorAdmissionCertificateOf<T>> {
      let authority = T::AdmissionCertificateAuthority::current()?;
      Some(ActorAdmissionCertificate::new(
        contract.semantic_contract_id(),
        contract.body_commitment()?,
        contract.trigger.wake_qualification(&contract.window),
        authority.runtime_actor_semantics_version,
        authority.production_weight_identity,
        authority.body_geometry_version,
        authority.configured_bounds_commitment,
        authority.maximum_lifecycle_weight,
      ))
    }

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
        admission,
        ticket: ActorStepAuthority {
          actor_id: ticket.actor_id,
          cycle_nonce: ticket.cycle_nonce,
          cursor: ticket.cursor,
          eligible_at: ticket.eligible_at,
          contract_commitment: ticket.contract_commitment,
        },
        loaded_step,
        maximum_fee,
        last_step_outcome: None,
      })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_canonical_current_step_plan(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      run: Option<ActorRunStateOf<T>>,
      admission: ActorAdmissionCertificateOf<T>,
      loaded_step: LoadedActorStepOf<T>,
      maximum_fee: StepFeeBreakdown<T::Balance>,
    ) -> Option<CurrentStepPlanOf<T>> {
      if !admission.has_valid_identity() {
        return None;
      }
      let (cycle_nonce, cursor, eligible_at) = match (hot.cycle_state, run.as_ref()) {
        (CycleState::Idle, None) => (
          identity.cycle_nonce.checked_add(1)?,
          0,
          frame_system::Pallet::<T>::block_number(),
        ),
        (CycleState::Running, Some(run)) if run.running_is_coherent() => {
          (run.cycle_nonce, run.cursor, run.eligible_at)
        }
        (CycleState::Suspended, Some(run)) if run.suspension_is_coherent() => {
          (run.cycle_nonce, run.cursor, run.eligible_at)
        }
        _ => return None,
      };
      if loaded_step.cursor != cursor
        || run.as_ref().is_some_and(|run| {
          !run.has_contract_authority(
            admission.semantic_contract_id,
            admission.body_commitment,
            admission.admission_identity,
          )
        })
      {
        return None;
      }
      Some(StepExecutionPlan {
        identity,
        hot,
        run,
        admission: admission.clone(),
        ticket: ActorStepAuthority {
          actor_id,
          cycle_nonce,
          cursor,
          eligible_at,
          contract_commitment: ActorContractCommitment {
            semantic_contract_id: admission.semantic_contract_id,
            body_commitment: admission.body_commitment,
          },
        },
        loaded_step,
        maximum_fee,
        last_step_outcome: None,
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
      let identity = Self::load_control_identity(actor_id)?;
      let hot = Self::load_control_hot(actor_id)?;
      let run = ActorRunStateStore::<T>::get(actor_id);
      let admission = Self::load_control_admission(actor_id)?;
      let loaded_step = Self::load_current_step_from_storage(actor_id, ticket.cursor)?;
      let maximum_fee =
        Self::maximum_current_step_fee(identity.actor_class.actor_type(), loaded_step.resources)
          .ok()?;
      Self::build_current_step_plan(
        actor_id,
        identity,
        hot,
        run,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
    }

    pub(crate) fn control_identity_from_scalar(
      identity: ActorIdentityOf<T>,
    ) -> Option<ActorControlIdentity<T::AccountId, BlockNumberFor<T>>> {
      let sovereign_account = match identity.actor_class {
        ActorClass::User { owner_slot } => Self::sovereign_account_id(&identity.owner, owner_slot),
        ActorClass::System { sovereign_id } => Self::sovereign_account_id_system(sovereign_id),
      };
      if sovereign_account != identity.sovereign_account {
        return None;
      }
      Some(ActorControlIdentity {
        owner: identity.owner,
        actor_class: identity.actor_class,
        mutability: identity.mutability,
        cycle_nonce: identity.cycle_nonce,
        last_control_mutation_block: identity.last_control_mutation_block,
      })
    }

    pub(crate) fn control_hot_from_scalar(
      hot: ActorHotStateOf<T>,
    ) -> ActorControlHotState<BlockNumberFor<T>> {
      ActorControlHotState {
        lifecycle: hot.lifecycle,
        cycle_state: hot.cycle_state,
        trigger_runtime_state: hot.trigger_runtime_state,
        unsuccessful_attempt_streak: hot.unsuccessful_attempt_streak,
        pending_signal: hot.pending_signal,
        wakeup_pointer: hot.wakeup_pointer,
        trigger_wakeup_pointer: hot.trigger_wakeup_pointer,
        terminal_at: hot.terminal_at,
        schedule_anchor: hot.schedule_anchor,
        last_cycle_block: hot.last_cycle_block,
      }
    }

    pub(crate) fn control_identity_to_scalar(
      identity: &ActorControlIdentity<T::AccountId, BlockNumberFor<T>>,
    ) -> ActorIdentityOf<T> {
      let sovereign_account = match identity.actor_class {
        ActorClass::User { owner_slot } => Self::sovereign_account_id(&identity.owner, owner_slot),
        ActorClass::System { sovereign_id } => Self::sovereign_account_id_system(sovereign_id),
      };
      ActorIdentity {
        sovereign_account,
        owner: identity.owner.clone(),
        actor_class: identity.actor_class,
        mutability: identity.mutability,
        cycle_nonce: identity.cycle_nonce,
        last_control_mutation_block: identity.last_control_mutation_block,
      }
    }

    pub(crate) fn control_hot_to_scalar(
      hot: &ActorControlHotState<BlockNumberFor<T>>,
      queue_ticket: Option<QueueTicket>,
    ) -> ActorHotStateOf<T> {
      ActorHotState {
        lifecycle: hot.lifecycle,
        cycle_state: hot.cycle_state,
        trigger_runtime_state: hot.trigger_runtime_state.clone(),
        unsuccessful_attempt_streak: hot.unsuccessful_attempt_streak,
        pending_signal: hot.pending_signal,
        queue_ticket,
        wakeup_pointer: hot.wakeup_pointer,
        trigger_wakeup_pointer: hot.trigger_wakeup_pointer,
        terminal_at: hot.terminal_at,
        schedule_anchor: hot.schedule_anchor,
        last_cycle_block: hot.last_cycle_block,
      }
    }

    #[cfg(feature = "runtime-benchmarks")]
    pub(crate) fn control_cell_from_parts(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
      loaded_step: &LoadedActorStepOf<T>,
      eligible_at: Option<BlockNumberFor<T>>,
    ) -> Option<ActorControlCellOf<T>> {
      if !admission.has_valid_identity() {
        return None;
      }
      Some(ActorControlCell {
        actor_id,
        identity: Self::control_identity_from_scalar(identity)?,
        hot: Self::control_hot_from_scalar(hot),
        pipeline_service_identity: pipeline_service_identity(admission.admission_identity),
        cursor: loaded_step.cursor,
        eligible_at,
        admission,
        resources: loaded_step.resources,
      })
    }

    #[cfg(feature = "runtime-benchmarks")]
    pub(crate) fn control_opening_cell_from_scalar(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
      ticket: &ActorStepTicketOf<T>,
      loaded_step: &LoadedActorStepOf<T>,
    ) -> Option<ActorControlCellOf<T>> {
      if hot.cycle_state != CycleState::Idle
        || hot.queue_ticket != Some(ticket.ticket)
        || ticket.actor_id != actor_id
        || ticket.cursor != 0
        || ticket.cycle_nonce != identity.cycle_nonce.checked_add(1)?
        || ticket.eligible_at > frame_system::Pallet::<T>::block_number()
        || !Self::validate_loaded_step_authority(
          actor_id,
          ticket.ticket,
          &admission,
          ticket,
          loaded_step,
        )
      {
        return None;
      }
      Self::control_cell_from_parts(
        actor_id,
        identity,
        hot,
        admission,
        loaded_step,
        Some(ticket.eligible_at),
      )
    }

    #[cfg(feature = "runtime-benchmarks")]
    #[allow(
      dead_code,
      reason = "zero-Step frame projection remains test-only until the atomic production cutover"
    )]
    pub(crate) fn control_zero_step_opening_cell_from_scalar(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
      ticket: &ActorStepTicketOf<T>,
    ) -> Option<ActorControlCellOf<T>> {
      if hot.cycle_state != CycleState::Idle
        || hot.queue_ticket != Some(ticket.ticket)
        || ticket.actor_id != actor_id
        || ticket.cursor != 0
        || ticket.cycle_nonce != identity.cycle_nonce.checked_add(1)?
        || ticket.eligible_at > frame_system::Pallet::<T>::block_number()
        || !admission.has_valid_identity()
      {
        return None;
      }
      Some(ActorControlCell {
        actor_id,
        identity: Self::control_identity_from_scalar(identity)?,
        hot: Self::control_hot_from_scalar(hot),
        pipeline_service_identity: pipeline_service_identity(admission.admission_identity),
        cursor: 0,
        eligible_at: Some(ticket.eligible_at),
        admission,
        resources: ActorStepResourceEnvelope {
          control: T::WeightInfo::scheduler_inner_zero_step_complete(),
          effect: Weight::zero(),
        },
      })
    }

    #[cfg(feature = "runtime-benchmarks")]
    #[allow(
      dead_code,
      reason = "zero-Step frame projection remains test-only until the atomic production cutover"
    )]
    pub(crate) fn control_zero_step_unsignaled_cell_from_scalar(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
    ) -> Option<ActorControlCellOf<T>> {
      if hot.cycle_state != CycleState::Idle
        || hot.pending_signal
        || hot.queue_ticket.is_some()
        || hot.wakeup_pointer.is_some()
        || hot.trigger_wakeup_pointer.is_some()
        || !admission.has_valid_identity()
      {
        return None;
      }
      Some(ActorControlCell {
        actor_id,
        identity: Self::control_identity_from_scalar(identity)?,
        hot: Self::control_hot_from_scalar(hot),
        pipeline_service_identity: pipeline_service_identity(admission.admission_identity),
        cursor: 0,
        eligible_at: None,
        admission,
        resources: ActorStepResourceEnvelope {
          control: T::WeightInfo::scheduler_inner_zero_step_complete(),
          effect: Weight::zero(),
        },
      })
    }

    #[cfg(feature = "runtime-benchmarks")]
    pub(crate) fn control_unsignaled_cell_from_scalar(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
      loaded_step: &LoadedActorStepOf<T>,
    ) -> Option<ActorControlCellOf<T>> {
      if hot.cycle_state != CycleState::Idle
        || hot.pending_signal
        || hot.queue_ticket.is_some()
        || hot.wakeup_pointer.is_some()
        || hot.trigger_wakeup_pointer.is_some()
        || loaded_step.cursor != 0
      {
        return None;
      }
      Self::control_cell_from_parts(actor_id, identity, hot, admission, loaded_step, None)
    }

    fn admission_matches_current_authority(admission: &ActorAdmissionCertificateOf<T>) -> bool {
      T::AdmissionCertificateAuthority::current().is_some_and(|authority| {
        admission.runtime_actor_semantics_version == authority.runtime_actor_semantics_version
          && admission.production_weight_identity == authority.production_weight_identity
          && admission.body_geometry_version == authority.body_geometry_version
          && admission.configured_bounds_commitment == authority.configured_bounds_commitment
          && admission.maximum_lifecycle_weight == authority.maximum_lifecycle_weight
      })
    }

    #[cfg(any(test, feature = "runtime-benchmarks"))]
    #[allow(
      dead_code,
      reason = "qualified host wake projection remains candidate-only until the atomic control cutover"
    )]
    pub(crate) fn project_control_cell_for_wake(
      cell: &ActorControlCellOf<T>,
      location: ActorControlLocation<BlockNumberFor<T>>,
      qualification: ActorWakeQualification,
    ) -> Option<(
      ActorIdentityOf<T>,
      ActorHotStateOf<T>,
      ActorAdmissionCertificateOf<T>,
    )> {
      cell
        .admission
        .authorizes_wake(qualification)
        .then(|| Self::project_control_cell(cell, location))?
    }

    pub(crate) fn project_control_cell(
      cell: &ActorControlCellOf<T>,
      location: ActorControlLocation<BlockNumberFor<T>>,
    ) -> Option<(
      ActorIdentityOf<T>,
      ActorHotStateOf<T>,
      ActorAdmissionCertificateOf<T>,
    )> {
      if !cell.admission.has_valid_identity()
        || !Self::admission_matches_current_authority(&cell.admission)
        || cell.pipeline_service_identity
          != pipeline_service_identity(cell.admission.admission_identity)
      {
        return None;
      }
      let queue_ticket = match location {
        ActorControlLocation::Unsignaled => {
          if cell.cursor != 0
            || cell.eligible_at.is_some()
            || (cell.hot.pending_signal && !cell.hot.lifecycle.is_paused())
            || cell.hot.cycle_state != CycleState::Idle
          {
            return None;
          }
          if let Some(pointer) = cell.hot.wakeup_pointer {
            let WakeupKey::Block(terminal_at) = pointer.block else {
              return None;
            };
            if cell.hot.terminal_at != Some(terminal_at)
              || !Self::wakeup_page_entry_matches(pointer, cell.actor_id)
            {
              return None;
            }
          }
          if let Some(pointer) = cell.hot.trigger_wakeup_pointer {
            if ActorControlLocators::<T>::get(cell.actor_id)
              != Some(ActorControlLocation::Unsignaled)
            {
              return None;
            }
            let reference = WakeupPointer {
              block: WakeupKey::Tick(pointer.tick),
              page_id: pointer.page_id,
              slot: pointer.slot,
            };
            if !Self::wakeup_page_entry_matches(reference, cell.actor_id) {
              return None;
            }
          }
          None
        }
        ActorControlLocation::Ready { ticket } => {
          let terminal_ready = cell.hot.cycle_state == CycleState::Idle
            && !cell.hot.pending_signal
            && cell
              .hot
              .terminal_at
              .zip(cell.eligible_at)
              .is_some_and(|(terminal_at, eligible_at)| eligible_at >= terminal_at);
          if cell.eligible_at.is_none()
            || !(matches!(
              cell.hot.cycle_state,
              CycleState::Running | CycleState::Suspended
            ) || (cell.hot.cycle_state == CycleState::Idle && cell.hot.pending_signal)
              || terminal_ready)
          {
            return None;
          }
          Some(ticket)
        }
        ActorControlLocation::Waiting { key, .. } => {
          if let Some(pointer) = cell.hot.wakeup_pointer {
            let terminal_waiting = cell.hot.cycle_state == CycleState::Idle
              && !cell.hot.pending_signal
              && cell
                .hot
                .terminal_at
                .zip(cell.eligible_at)
                .is_some_and(|(terminal_at, eligible_at)| eligible_at >= terminal_at);
            if pointer.block != key
              || cell.eligible_at.is_none()
              || !(matches!(
                cell.hot.cycle_state,
                CycleState::Running | CycleState::Suspended
              ) || (cell.hot.cycle_state == CycleState::Idle && cell.hot.pending_signal)
                || terminal_waiting)
            {
              return None;
            }
          } else {
            let pointer = cell.hot.trigger_wakeup_pointer?;
            let reference = WakeupPointer {
              block: WakeupKey::Tick(pointer.tick),
              page_id: pointer.page_id,
              slot: pointer.slot,
            };
            if key != reference.block
              || cell.eligible_at.is_some()
              || cell.hot.pending_signal
              || cell.hot.cycle_state != CycleState::Idle
            {
              return None;
            }
          }
          None
        }
      };
      Some((
        Self::control_identity_to_scalar(&cell.identity),
        Self::control_hot_to_scalar(&cell.hot, queue_ticket),
        cell.admission.clone(),
      ))
    }

    /// Compiles one coherent legacy control owner without publishing a second storage authority.
    /// Ready Idle work is Pending; already-open or terminal work remains Live. Unsignaled requires
    /// separately supplied Park or lifecycle-disablement evidence and is never guessed from absence.
    #[allow(
      dead_code,
      reason = "storage-free cutover adapter remains candidate-only until every placement owner moves atomically"
    )]
    pub(crate) fn compile_legacy_control_process(
      generation: ActorGeneration,
      last_attempted: Option<BlockNumberFor<T>>,
      location: ActorControlLocation<BlockNumberFor<T>>,
      cell: &ActorControlCellOf<T>,
      unsignaled_evidence: Option<UnsignaledProcessEvidence<BlockNumberFor<T>>>,
    ) -> Result<ActorProcess<BlockNumberFor<T>>, ProcessCompileError> {
      Self::project_control_cell(cell, location)
        .ok_or(ProcessCompileError::MalformedControlCell)?;
      let placement = match location {
        ActorControlLocation::Ready { .. } => {
          let kind = if cell.hot.cycle_state == CycleState::Idle && cell.hot.pending_signal {
            ServiceResidenceKind::Pending
          } else {
            ServiceResidenceKind::Live
          };
          LegacyProcessPlacement::Ready(kind)
        }
        ActorControlLocation::Waiting { key, page, slot } => {
          LegacyProcessPlacement::Waiting { key, page, slot }
        }
        ActorControlLocation::Unsignaled => LegacyProcessPlacement::Unsignaled(unsignaled_evidence),
      };
      compile_legacy_process(generation, last_attempted, placement)
    }

    /// Publishes one already-inventoried legacy transition only inside its caller's transaction.
    /// The legacy locator must have been removed first, so failure rolls the whole authority move
    /// back rather than creating dual process residence.
    #[allow(
      dead_code,
      reason = "publication helper remains unreachable until all legacy mutation cohorts cut over together"
    )]
    pub(crate) fn publish_legacy_process_transition(
      actor_id: ActorId,
      current: ActorProcessOf<T>,
      obligation: ProcessTransitionObligation,
      transition: LegacyProcessTransition<BlockNumberFor<T>>,
    ) -> Result<ActorProcessOf<T>, ProcessPublicationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(ProcessPublicationError::TransactionRequired);
      }
      if ActorControlLocators::<T>::contains_key(actor_id)
        || ActorUnsignaledControlCells::<T>::contains_key(actor_id)
      {
        return Err(ProcessPublicationError::LegacyAuthorityPresent);
      }

      let stored = ActorProcesses::<T>::get(actor_id);
      match transition {
        LegacyProcessTransition::Publish(_) if stored.is_some() => {
          return Err(ProcessPublicationError::ProcessAlreadyExists);
        }
        LegacyProcessTransition::Publish(_) => {}
        _ => match stored {
          None => return Err(ProcessPublicationError::ProcessMissing),
          Some(stored) if stored != current => {
            return Err(ProcessPublicationError::CurrentProcessMismatch);
          }
          Some(_) => {}
        },
      }

      let next = plan_legacy_process_transition(current, obligation, transition)
        .map_err(ProcessPublicationError::Transition)?;
      ActorProcesses::<T>::insert(actor_id, next);
      Ok(next)
    }

    /// Atomically publishes one typed service process and inserts its generation-bound ring node.
    /// This is the complete canonical carrier owner for a caller that has already removed legacy
    /// authority; either both storage surfaces commit or neither does.
    #[allow(
      dead_code,
      reason = "atomic service publication remains unreachable until supported callers cut over"
    )]
    pub(crate) fn publish_service_member(
      actor: ActorRef,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
    ) -> Result<(), ServicePublicationError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let current = ActorProcess {
          generation: actor.generation,
          last_attempted: None,
          status: ProcessStatus::Serving,
          residence: Some(ProcessResidence::Service(kind)),
        };
        let result = Self::publish_legacy_process_transition(
          actor.actor_id,
          current,
          ProcessTransitionObligation::PublishTypedResidence,
          LegacyProcessTransition::Publish(LegacyProcessPlacement::Ready(kind)),
        )
        .map_err(ServicePublicationError::Process)
        .and_then(|_| {
          Self::insert_service_member(actor, kind, now).map_err(ServicePublicationError::Ring)
        });
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Appends one generation-bound process to the inert service ring. The caller's transaction
    /// must publish the matching process and remove legacy authority before entering this boundary.
    #[allow(
      dead_code,
      reason = "service-ring mutation remains unreachable until the complete carrier cutover"
    )]
    pub(crate) fn insert_service_member(
      actor: ActorRef,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
    ) -> Result<(), ServiceRingMutationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(ServiceRingMutationError::TransactionRequired);
      }
      if ActorControlLocators::<T>::contains_key(actor.actor_id)
        || ActorUnsignaledControlCells::<T>::contains_key(actor.actor_id)
      {
        return Err(ServiceRingMutationError::LegacyAuthorityPresent);
      }
      let process =
        ActorProcesses::<T>::get(actor.actor_id).ok_or(ServiceRingMutationError::ProcessMissing)?;
      if process.generation != actor.generation
        || process.status != ProcessStatus::Serving
        || process.residence != Some(ProcessResidence::Service(kind))
      {
        return Err(ServiceRingMutationError::ProcessResidenceMismatch);
      }
      if ServiceNodes::<T>::contains_key(actor.actor_id) {
        return Err(ServiceRingMutationError::MemberAlreadyExists);
      }

      let mut header = ServiceHeader::<T>::get();
      match header.round_block {
        Some(round) if round > now => return Err(ServiceRingMutationError::CorruptRing),
        Some(round) if round == now => {}
        _ => header.round_block = Some(now),
      }
      let eligible_from = now
        .checked_add(&One::one())
        .ok_or(ServiceRingMutationError::BlockNumberOverflow)?;
      let next_count = header
        .count
        .checked_add(1)
        .ok_or(ServiceRingMutationError::CapacityExceeded)?;
      let node = ServiceNode {
        generation: actor.generation,
        previous: actor,
        next: actor,
        kind,
        eligible_from,
        last_considered: now,
      };
      match (header.count, header.cursor) {
        (0, None) => header.cursor = Some(actor),
        (0, Some(_)) | (_, None) => return Err(ServiceRingMutationError::CorruptRing),
        (_, Some(cursor)) => {
          let mut head = ServiceNodes::<T>::get(cursor.actor_id)
            .filter(|node| node.generation == cursor.generation)
            .ok_or(ServiceRingMutationError::CorruptRing)?;
          let tail_ref = head.previous;
          let mut tail = ServiceNodes::<T>::get(tail_ref.actor_id)
            .filter(|node| node.generation == tail_ref.generation)
            .ok_or(ServiceRingMutationError::CorruptRing)?;
          if tail.next != cursor
            || (header.count == 1
              && (tail_ref != cursor || head.next != cursor || head.previous != cursor))
            || (header.count > 1 && tail_ref == cursor)
          {
            return Err(ServiceRingMutationError::CorruptRing);
          }
          head.previous = actor;
          tail.next = actor;
          if cursor.actor_id == tail_ref.actor_id {
            head.next = actor;
            ServiceNodes::<T>::insert(cursor.actor_id, head);
          } else {
            ServiceNodes::<T>::insert(cursor.actor_id, head);
            ServiceNodes::<T>::insert(tail_ref.actor_id, tail);
          }
          ServiceNodes::<T>::insert(
            actor.actor_id,
            ServiceNode {
              previous: tail_ref,
              next: cursor,
              ..node
            },
          );
        }
      }
      if header.count == 0 {
        ServiceNodes::<T>::insert(actor.actor_id, node);
      }
      header.count = next_count;
      ServiceHeader::<T>::put(header);
      Ok(())
    }

    /// Opens or resumes exactly one immutable block round without moving its persistent cursor.
    #[allow(
      dead_code,
      reason = "round frontier remains inert until scheduler authority cutover"
    )]
    pub(crate) fn begin_service_round(now: BlockNumberFor<T>) -> Result<(), ServiceRoundError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(ServiceRoundError::TransactionRequired);
      }
      let mut header = ServiceHeader::<T>::get();
      if header.count == 0 {
        if header.cursor.is_some() {
          return Err(ServiceRoundError::CorruptRing);
        }
      } else if header.cursor.is_none() {
        return Err(ServiceRoundError::CorruptRing);
      }
      match header.round_block {
        Some(round) if round > now => Err(ServiceRoundError::RoundFromFuture),
        Some(round) if round == now => Ok(()),
        _ => {
          header.round_block = Some(now);
          ServiceHeader::<T>::put(header);
          Ok(())
        }
      }
    }

    /// Classifies the current frontier without changing a blocked or closed head.
    #[allow(
      dead_code,
      reason = "round frontier remains inert until scheduler authority cutover"
    )]
    pub(crate) fn consider_service_head(
      now: BlockNumberFor<T>,
    ) -> Result<ServiceRoundEncounter, ServiceRoundError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(ServiceRoundError::TransactionRequired);
      }
      let header = ServiceHeader::<T>::get();
      if header.round_block != Some(now) {
        return Err(ServiceRoundError::RoundNotStarted);
      }
      let Some(actor) = header.cursor else {
        return if header.count == 0 {
          Ok(ServiceRoundEncounter::Empty)
        } else {
          Err(ServiceRoundError::CorruptRing)
        };
      };
      if header.count == 0 {
        return Err(ServiceRoundError::CorruptRing);
      }
      let node = ServiceNodes::<T>::get(actor.actor_id).ok_or(ServiceRoundError::CorruptRing)?;
      if node.generation != actor.generation {
        return Err(ServiceRoundError::StaleGeneration);
      }
      let process =
        ActorProcesses::<T>::get(actor.actor_id).ok_or(ServiceRoundError::ProcessMissing)?;
      if process.generation != actor.generation {
        return Err(ServiceRoundError::StaleGeneration);
      }
      if process.status != ProcessStatus::Serving
        || process.residence != Some(ProcessResidence::Service(node.kind))
      {
        return Err(ServiceRoundError::ProcessResidenceMismatch);
      }
      if node.last_considered > now {
        return Err(ServiceRoundError::RoundFromFuture);
      }
      if node.last_considered == now {
        return Ok(ServiceRoundEncounter::Closed);
      }
      if node.eligible_from > now {
        return Err(ServiceRoundError::FutureMemberUnmarked);
      }
      match process.last_attempted {
        Some(attempted) if attempted > now => Err(ServiceRoundError::AttemptFromFuture),
        Some(attempted) if attempted == now => Ok(ServiceRoundEncounter::AlreadyAttempted(actor)),
        _ => Ok(ServiceRoundEncounter::Eligible(actor)),
      }
    }

    /// Commits one successful retained consideration and advances to its captured successor.
    #[allow(
      dead_code,
      reason = "round frontier remains inert until scheduler authority cutover"
    )]
    pub(crate) fn advance_service_head(
      actor: ActorRef,
      now: BlockNumberFor<T>,
    ) -> Result<(), ServiceRoundError> {
      if Self::consider_service_head(now)? != ServiceRoundEncounter::Eligible(actor) {
        return Err(ServiceRoundError::CorruptRing);
      }
      let mut node =
        ServiceNodes::<T>::get(actor.actor_id).ok_or(ServiceRoundError::CorruptRing)?;
      let mut process =
        ActorProcesses::<T>::get(actor.actor_id).ok_or(ServiceRoundError::ProcessMissing)?;
      let mut header = ServiceHeader::<T>::get();
      node.last_considered = now;
      process.last_attempted = Some(now);
      header.cursor = Some(node.next);
      ServiceNodes::<T>::insert(actor.actor_id, node);
      ActorProcesses::<T>::insert(actor.actor_id, process);
      ServiceHeader::<T>::put(header);
      Ok(())
    }

    /// Advances one already-attempted defensive encounter without admitting another attempt.
    #[allow(
      dead_code,
      reason = "round frontier remains inert until scheduler authority cutover"
    )]
    pub(crate) fn converge_attempted_service_head(
      actor: ActorRef,
      now: BlockNumberFor<T>,
    ) -> Result<(), ServiceRoundError> {
      if Self::consider_service_head(now)? != ServiceRoundEncounter::AlreadyAttempted(actor) {
        return Err(ServiceRoundError::CorruptRing);
      }
      let mut node =
        ServiceNodes::<T>::get(actor.actor_id).ok_or(ServiceRoundError::CorruptRing)?;
      let mut header = ServiceHeader::<T>::get();
      node.last_considered = now;
      header.cursor = Some(node.next);
      ServiceNodes::<T>::insert(actor.actor_id, node);
      ServiceHeader::<T>::put(header);
      Ok(())
    }

    /// Advances one eligible member that holds no admitted work for this round without executing a
    /// Step or recording an attempt. The completed/aborted Idle resident stays in the ring; only the
    /// bounded cursor advances so a subsequently inserted independent member becomes serviceable.
    pub(crate) fn advance_idle_service_head(
      actor: ActorRef,
      now: BlockNumberFor<T>,
    ) -> Result<(), ServiceRoundError> {
      if Self::consider_service_head(now)? != ServiceRoundEncounter::Eligible(actor) {
        return Err(ServiceRoundError::CorruptRing);
      }
      let mut node =
        ServiceNodes::<T>::get(actor.actor_id).ok_or(ServiceRoundError::CorruptRing)?;
      if node.generation != actor.generation {
        return Err(ServiceRoundError::StaleGeneration);
      }
      let mut header = ServiceHeader::<T>::get();
      node.last_considered = now;
      header.cursor = Some(node.next);
      ServiceNodes::<T>::insert(actor.actor_id, node);
      ServiceHeader::<T>::put(header);
      Ok(())
    }

    /// Resolves one typed Oracle feed to a collision-free retained scalar source identity.
    pub(crate) fn resolve_observation_dependency_source(
      feed: T::ObservationFeedId,
    ) -> Result<DependencySourceMutation, DependencySourceError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencySourceError::TransactionRequired);
      }
      if let Some(source) = ObservationDependencySources::<T>::get(feed) {
        return match DependencySourceObservations::<T>::get(source) {
          Some(reverse) if reverse == feed => Ok(DependencySourceMutation::Existing(source)),
          Some(_) => Err(DependencySourceError::ReverseMismatch),
          None => Err(DependencySourceError::ReverseMissing),
        };
      }
      let mut allocator = DependencySourceAllocatorState::<T>::get();
      if allocator.exhausted {
        return Err(DependencySourceError::Exhausted);
      }
      let source = allocator.next;
      if DependencySourceObservations::<T>::contains_key(source) {
        return Err(DependencySourceError::SourceOccupied);
      }
      match source.checked_add(1) {
        Some(next) => allocator.next = next,
        None => allocator.exhausted = true,
      }
      ObservationDependencySources::<T>::insert(feed, source);
      DependencySourceObservations::<T>::insert(source, feed);
      DependencySourceAllocatorState::<T>::put(allocator);
      Ok(DependencySourceMutation::Allocated(source))
    }

    /// Resolves one typed Oracle feed and publishes its event-complete dependency revision.
    pub(crate) fn publish_observation_dependency_event(
      feed: T::ObservationFeedId,
    ) -> DispatchResult {
      let source = match Self::resolve_observation_dependency_source(feed) {
        Ok(
          DependencySourceMutation::Allocated(source) | DependencySourceMutation::Existing(source),
        ) => source,
        Err(DependencySourceError::TransactionRequired) => {
          return Err(DispatchError::Other(
            "dependency event requires transaction",
          ));
        }
        Err(DependencySourceError::Exhausted) => {
          return Err(DispatchError::Other("dependency source identity exhausted"));
        }
        Err(DependencySourceError::ReverseMissing) => {
          return Err(DispatchError::Other("dependency source reverse missing"));
        }
        Err(DependencySourceError::ReverseMismatch) => {
          return Err(DispatchError::Other("dependency source reverse mismatch"));
        }
        Err(DependencySourceError::SourceOccupied) => {
          return Err(DispatchError::Other("dependency source identity occupied"));
        }
      };
      match Self::publish_dependency_event_with_source_retention(source) {
        Ok(
          DependencyPublicationMutation::Begun(_) | DependencyPublicationMutation::Coalesced { .. },
        ) => Ok(()),
        Ok(DependencyPublicationMutation::Exhausted) => {
          Err(DispatchError::Other("dependency revision exhausted"))
        }
        Err(DependencyPublicationError::Revision(DependencyRevisionError::TransactionRequired)) => {
          Err(DispatchError::Other(
            "dependency event requires transaction",
          ))
        }
        Err(DependencyPublicationError::SourceCarrier(
          DependencyScanSourceError::TransactionRequired,
        )) => Err(DispatchError::Other(
          "dependency event requires transaction",
        )),
        Err(DependencyPublicationError::SourceCarrier(
          DependencyScanSourceError::CapacityExceeded,
        )) => Err(DispatchError::Other(
          "dependency scan source capacity reached",
        )),
        Err(DependencyPublicationError::SourceCarrier(DependencyScanSourceError::ScanInactive)) => {
          Err(DispatchError::Other("dependency scan source inactive"))
        }
        Err(DependencyPublicationError::SourceCarrier(DependencyScanSourceError::Missing)) => {
          Err(DispatchError::Other("dependency scan source missing"))
        }
        Err(DependencyPublicationError::SourceCarrier(
          DependencyScanSourceError::CorruptTopology,
        )) => Err(DispatchError::Other(
          "dependency scan source topology corrupt",
        )),
      }
    }

    /// Advances one event-complete dependency source without wrapping its causal identity.
    #[allow(
      dead_code,
      reason = "dependency revisions remain inert until parking authority cutover"
    )]
    pub(crate) fn revise_dependency_source(
      source: DependencySourceId,
    ) -> Result<DependencyRevisionMutation, DependencyRevisionError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRevisionError::TransactionRequired);
      }
      let mut state = DependencyRevisions::<T>::get(source);
      if state.exhausted {
        return Ok(DependencyRevisionMutation::Exhausted);
      }
      let Some(next) = state.revision.checked_add(1) else {
        state.exhausted = true;
        DependencyRevisions::<T>::insert(source, state);
        return Ok(DependencyRevisionMutation::Exhausted);
      };
      state.revision = next;
      DependencyRevisions::<T>::insert(source, state);
      Ok(DependencyRevisionMutation::Advanced(next))
    }

    /// Publishes one event-complete revision and retains exactly one fixed-target scan.
    #[allow(
      dead_code,
      reason = "dependency publication remains inert until parking authority cutover"
    )]
    pub(crate) fn publish_dependency_event(
      source: DependencySourceId,
    ) -> Result<DependencyPublicationMutation, DependencyRevisionError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRevisionError::TransactionRequired);
      }
      let mut state = DependencyRevisions::<T>::get(source);
      if state.exhausted {
        return Ok(DependencyPublicationMutation::Exhausted);
      }
      let Some(next) = state.revision.checked_add(1) else {
        state.exhausted = true;
        DependencyRevisions::<T>::insert(source, state);
        return Ok(DependencyPublicationMutation::Exhausted);
      };
      state.revision = next;
      let outcome = if let Some(active_target) = state.scan_target {
        DependencyPublicationMutation::Coalesced {
          revision: next,
          active_target,
        }
      } else {
        state.scan_target = Some(next);
        state.scan_cursor = 0;
        state.scan_end = DependencyRegistrationHeaders::<T>::get(source).next_index;
        DependencyPublicationMutation::Begun(next)
      };
      DependencyRevisions::<T>::insert(source, state);
      Ok(outcome)
    }

    /// Publishes one revision and transactionally retains its active source for fair scanning.
    pub(crate) fn publish_dependency_event_with_source_retention(
      source: DependencySourceId,
    ) -> Result<DependencyPublicationMutation, DependencyPublicationError> {
      let publication =
        Self::publish_dependency_event(source).map_err(DependencyPublicationError::Revision)?;
      if matches!(publication, DependencyPublicationMutation::Exhausted) {
        return Ok(publication);
      }
      Self::insert_dependency_scan_source(source)
        .map_err(DependencyPublicationError::SourceCarrier)?;
      Ok(publication)
    }

    fn dependency_scan_source_capacity() -> u32 {
      T::MaxActiveActors::get().saturating_mul(T::MaxContractSteps::get())
    }

    /// Inserts one active source into the exact fair scan selector.
    #[allow(
      dead_code,
      reason = "dependency scan carrier remains inert until weighted cutover"
    )]
    pub(crate) fn insert_dependency_scan_source(
      source: DependencySourceId,
    ) -> Result<DependencyScanSourceMutation, DependencyScanSourceError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyScanSourceError::TransactionRequired);
      }
      if DependencyRevisions::<T>::get(source).scan_target.is_none() {
        return Err(DependencyScanSourceError::ScanInactive);
      }
      if DependencyScanSourceNodes::<T>::contains_key(source) {
        return Ok(DependencyScanSourceMutation::AlreadyActive);
      }
      let mut list = DependencyScanSourceListState::<T>::get();
      if list.count >= Self::dependency_scan_source_capacity() {
        return Err(DependencyScanSourceError::CapacityExceeded);
      }
      match list.cursor {
        None => {
          if list.count != 0 {
            return Err(DependencyScanSourceError::CorruptTopology);
          }
          DependencyScanSourceNodes::<T>::insert(
            source,
            DependencyScanSourceNode {
              previous: source,
              next: source,
            },
          );
          list.cursor = Some(source);
        }
        Some(cursor) => {
          let mut cursor_node = DependencyScanSourceNodes::<T>::get(cursor)
            .ok_or(DependencyScanSourceError::CorruptTopology)?;
          let tail = cursor_node.previous;
          if list.count == 1 {
            if tail != cursor || cursor_node.next != cursor {
              return Err(DependencyScanSourceError::CorruptTopology);
            }
            cursor_node.previous = source;
            cursor_node.next = source;
            DependencyScanSourceNodes::<T>::insert(cursor, cursor_node);
          } else {
            let mut tail_node = DependencyScanSourceNodes::<T>::get(tail)
              .ok_or(DependencyScanSourceError::CorruptTopology)?;
            if tail_node.next != cursor {
              return Err(DependencyScanSourceError::CorruptTopology);
            }
            tail_node.next = source;
            cursor_node.previous = source;
            DependencyScanSourceNodes::<T>::insert(tail, tail_node);
            DependencyScanSourceNodes::<T>::insert(cursor, cursor_node);
          }
          DependencyScanSourceNodes::<T>::insert(
            source,
            DependencyScanSourceNode {
              previous: tail,
              next: cursor,
            },
          );
        }
      }
      list.count = list
        .count
        .checked_add(1)
        .ok_or(DependencyScanSourceError::CapacityExceeded)?;
      DependencyScanSourceListState::<T>::put(list);
      Ok(DependencyScanSourceMutation::Inserted)
    }

    /// Removes one completed source while preserving a fair successor cursor.
    #[allow(
      dead_code,
      reason = "dependency scan carrier remains inert until weighted cutover"
    )]
    pub(crate) fn remove_dependency_scan_source(
      source: DependencySourceId,
    ) -> Result<DependencyScanSourceMutation, DependencyScanSourceError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyScanSourceError::TransactionRequired);
      }
      if DependencyRevisions::<T>::get(source).scan_target.is_some() {
        return Err(DependencyScanSourceError::ScanInactive);
      }
      let node =
        DependencyScanSourceNodes::<T>::get(source).ok_or(DependencyScanSourceError::Missing)?;
      let mut list = DependencyScanSourceListState::<T>::get();
      if list.count == 0 || list.cursor.is_none() {
        return Err(DependencyScanSourceError::CorruptTopology);
      }
      if list.count == 1 {
        if list.cursor != Some(source) || node.previous != source || node.next != source {
          return Err(DependencyScanSourceError::CorruptTopology);
        }
        list.cursor = None;
      } else if list.count == 2 {
        if node.previous != node.next {
          return Err(DependencyScanSourceError::CorruptTopology);
        }
        let survivor = node.next;
        let survivor_node = DependencyScanSourceNodes::<T>::get(survivor)
          .ok_or(DependencyScanSourceError::CorruptTopology)?;
        if survivor_node.previous != source || survivor_node.next != source {
          return Err(DependencyScanSourceError::CorruptTopology);
        }
        DependencyScanSourceNodes::<T>::insert(
          survivor,
          DependencyScanSourceNode {
            previous: survivor,
            next: survivor,
          },
        );
        if list.cursor == Some(source) {
          list.cursor = Some(survivor);
        }
      } else {
        let mut previous = DependencyScanSourceNodes::<T>::get(node.previous)
          .ok_or(DependencyScanSourceError::CorruptTopology)?;
        let mut next = DependencyScanSourceNodes::<T>::get(node.next)
          .ok_or(DependencyScanSourceError::CorruptTopology)?;
        if previous.next != source || next.previous != source {
          return Err(DependencyScanSourceError::CorruptTopology);
        }
        previous.next = node.next;
        next.previous = node.previous;
        DependencyScanSourceNodes::<T>::insert(node.previous, previous);
        DependencyScanSourceNodes::<T>::insert(node.next, next);
        if list.cursor == Some(source) {
          list.cursor = Some(node.next);
        }
      }
      list.count = list
        .count
        .checked_sub(1)
        .ok_or(DependencyScanSourceError::CorruptTopology)?;
      DependencyScanSourceNodes::<T>::remove(source);
      DependencyScanSourceListState::<T>::put(list);
      Ok(DependencyScanSourceMutation::Removed)
    }

    /// Starts one fixed-revision source scan without disturbing an already-active target.
    #[allow(
      dead_code,
      reason = "dependency scans remain inert until parking authority cutover"
    )]
    pub(crate) fn begin_dependency_scan(
      source: DependencySourceId,
    ) -> Result<DependencyScanMutation, DependencyScanError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyScanError::TransactionRequired);
      }
      let mut state = DependencyRevisions::<T>::get(source);
      if state.exhausted {
        return Err(DependencyScanError::SourceExhausted);
      }
      if state.scan_target.is_some() {
        return Err(DependencyScanError::ScanAlreadyActive);
      }
      state.scan_target = Some(state.revision);
      state.scan_cursor = 0;
      state.scan_end = DependencyRegistrationHeaders::<T>::get(source).next_index;
      DependencyRevisions::<T>::insert(source, state);
      Ok(DependencyScanMutation::Begun(state.revision))
    }

    /// Processes one cursor-derived registration before advancing the fixed scan frontier.
    #[allow(
      dead_code,
      reason = "dependency scans remain inert until parking authority cutover"
    )]
    pub(crate) fn process_dependency_scan_member(
      source: DependencySourceId,
      expected_target: DependencyRevision,
      expected_cursor: u64,
    ) -> Result<DependencyScanMutation, DependencyScanError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyScanError::TransactionRequired);
      }
      let mut state = DependencyRevisions::<T>::get(source);
      if state.exhausted {
        return Err(DependencyScanError::SourceExhausted);
      }
      if state.scan_target.ok_or(DependencyScanError::ScanMissing)? != expected_target {
        return Err(DependencyScanError::TargetMismatch);
      }
      if state.scan_cursor != expected_cursor {
        return Err(DependencyScanError::CursorMismatch);
      }
      let mut header = DependencyRegistrationHeaders::<T>::get(source);
      if state.scan_cursor >= state.scan_end || state.scan_end > header.next_index {
        return Err(DependencyScanError::ScanComplete);
      }
      let page_id = state.scan_cursor / 32;
      let slot = (state.scan_cursor % 32) as usize;
      let position = DependencyRegistrationPosition {
        page: page_id,
        slot: slot as u8,
      };
      let mut page = DependencyRegistrationPages::<T>::get(source, page_id)
        .ok_or(DependencyScanError::CorruptTopology)?;
      let encountered = *page
        .entries
        .get(slot)
        .ok_or(DependencyScanError::CorruptTopology)?;
      if let Some(handle) = encountered {
        let current = DependencyRegistrations::<T>::get(source, handle.actor.actor_id);
        if current == Some(handle) {
          if DependencyRegistrationPositions::<T>::get(source, handle.actor.actor_id)
            != Some(position)
          {
            return Err(DependencyScanError::CorruptRegistrationPosition);
          }
          let pending = PendingCheckOwners::<T>::get(handle.actor.actor_id)
            .ok_or(DependencyScanError::PendingAuthorityMissing)?;
          if pending
            != (PendingCheckOwner {
              actor: handle.actor,
              plan_revision: handle.plan_revision,
            })
          {
            return Err(DependencyScanError::PendingAuthorityMismatch);
          }
          if handle.acknowledged_revision < expected_target {
            if PendingDependencyReviews::<T>::contains_key(handle.actor.actor_id) {
              return Err(DependencyScanError::PendingDestinationMismatch);
            }
            let destination = PendingDependencyEvent {
              owner: pending,
              source,
              revision: expected_target,
            };
            match PendingDependencyEvents::<T>::get(handle.actor.actor_id) {
              None => PendingDependencyEvents::<T>::insert(handle.actor.actor_id, destination),
              Some(current) if current.owner == pending => {}
              Some(_) => return Err(DependencyScanError::PendingDestinationMismatch),
            }
            let acknowledged = DependencyRegistrationHandle {
              acknowledged_revision: expected_target,
              ..handle
            };
            let mut plan = DependencyPlans::<T>::get(handle.actor.actor_id);
            if !plan.is_empty() {
              let registration = plan
                .iter_mut(/* deos-bypass: bounded-iter -- complete plan is MaxContractSteps-bounded. */)
                .find(|registration| registration.source == source)
                .ok_or(DependencyScanError::CorruptTopology)?;
              if registration.handle != handle {
                return Err(DependencyScanError::CorruptTopology);
              }
              registration.handle = acknowledged;
              DependencyPlans::<T>::insert(handle.actor.actor_id, plan);
            }
            page.entries[slot] = Some(acknowledged);
            DependencyRegistrationPages::<T>::insert(source, page_id, page);
            DependencyRegistrations::<T>::insert(source, acknowledged.actor.actor_id, acknowledged);
          }
        } else {
          let reverse_position =
            DependencyRegistrationPositions::<T>::get(source, handle.actor.actor_id);
          if current.is_some() && reverse_position == Some(position) {
            return Err(DependencyScanError::CorruptRegistrationPosition);
          }
          page.entries[slot] = None;
          header.count = header
            .count
            .checked_sub(1)
            .ok_or(DependencyScanError::CorruptTopology)?;
          if header.free_count >= T::MaxActiveActors::get() {
            return Err(DependencyScanError::CorruptTopology);
          }
          DependencyRegistrationFreePositions::<T>::insert(source, header.free_count, position);
          header.free_count = header
            .free_count
            .checked_add(1)
            .ok_or(DependencyScanError::CorruptTopology)?;
          DependencyRegistrationPages::<T>::insert(source, page_id, page);
          DependencyRegistrationHeaders::<T>::insert(source, header);
          if current.is_none() && reverse_position == Some(position) {
            DependencyRegistrationPositions::<T>::remove(source, handle.actor.actor_id);
          }
        }
      }
      state.scan_cursor = state
        .scan_cursor
        .checked_add(1)
        .ok_or(DependencyScanError::CursorExhausted)?;
      DependencyRevisions::<T>::insert(source, state);
      Ok(DependencyScanMutation::Advanced(state.scan_cursor))
    }

    /// Completes only the fixed target and immediately retains the newest revision as successor.
    #[allow(
      dead_code,
      reason = "dependency scans remain inert until parking authority cutover"
    )]
    pub(crate) fn complete_dependency_scan(
      source: DependencySourceId,
      expected_target: DependencyRevision,
      expected_cursor: u64,
    ) -> Result<DependencyScanMutation, DependencyScanError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyScanError::TransactionRequired);
      }
      let mut state = DependencyRevisions::<T>::get(source);
      if state.exhausted {
        return Err(DependencyScanError::SourceExhausted);
      }
      if state.scan_target.ok_or(DependencyScanError::ScanMissing)? != expected_target {
        return Err(DependencyScanError::TargetMismatch);
      }
      if state.scan_cursor != expected_cursor {
        return Err(DependencyScanError::CursorMismatch);
      }
      if state.scan_cursor != state.scan_end {
        return Err(DependencyScanError::ScanComplete);
      }
      let outcome = if state.revision > expected_target {
        state.scan_target = Some(state.revision);
        state.scan_cursor = 0;
        state.scan_end = DependencyRegistrationHeaders::<T>::get(source).next_index;
        DependencyScanMutation::HandedOff(state.revision)
      } else {
        state.scan_target = None;
        state.scan_cursor = 0;
        state.scan_end = 0;
        DependencyScanMutation::Completed
      };
      DependencyRevisions::<T>::insert(source, state);
      Ok(outcome)
    }

    fn dependency_registration_position(
      source: DependencySourceId,
      actor_id: ActorId,
      expected: DependencyRegistrationHandle,
    ) -> Result<DependencyRegistrationPosition, DependencyRegistrationError> {
      let position = DependencyRegistrationPositions::<T>::get(source, actor_id)
        .ok_or(DependencyRegistrationError::PositionMissing)?;
      let page = DependencyRegistrationPages::<T>::get(source, position.page)
        .ok_or(DependencyRegistrationError::CorruptTopology)?;
      if page
        .entries
        .get(position.slot as usize)
        .and_then(Option::as_ref)
        != Some(&expected)
      {
        return Err(DependencyRegistrationError::PositionMismatch);
      }
      Ok(position)
    }

    fn validate_dependency_registration(
      source: DependencySourceId,
      owner: PendingCheckOwner,
      acknowledged_revision: DependencyRevision,
    ) -> Result<DependencyRegistrationHandle, DependencyRegistrationError> {
      let pending = PendingCheckOwners::<T>::get(owner.actor.actor_id)
        .ok_or(DependencyRegistrationError::PendingOwnerMissing)?;
      if pending != owner {
        return Err(DependencyRegistrationError::PendingOwnerMismatch);
      }
      let state = DependencyRevisions::<T>::get(source);
      if state.exhausted {
        return Err(DependencyRegistrationError::SourceExhausted);
      }
      if acknowledged_revision > state.revision {
        return Err(DependencyRegistrationError::RevisionFromFuture);
      }
      Ok(DependencyRegistrationHandle {
        actor: owner.actor,
        plan_revision: owner.plan_revision,
        acknowledged_revision,
      })
    }

    /// Commits one negative evaluation only while its exact owner and observed source remain current.
    #[allow(
      dead_code,
      reason = "negative evaluation remains inert until parking authority cutover"
    )]
    pub(crate) fn commit_negative_dependency_evaluation(
      source: DependencySourceId,
      owner: PendingCheckOwner,
      observed_revision: DependencyRevision,
    ) -> Result<DependencyRegistrationMutation, DependencyRegistrationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRegistrationError::TransactionRequired);
      }
      let state = DependencyRevisions::<T>::get(source);
      if state.exhausted {
        return Err(DependencyRegistrationError::SourceExhausted);
      }
      if state.revision != observed_revision {
        return Err(DependencyRegistrationError::RevisionMismatch);
      }
      match DependencyRegistrations::<T>::get(source, owner.actor.actor_id) {
        Some(current) => {
          Self::replace_dependency_registration(source, current, owner, observed_revision)
        }
        None => Self::install_dependency_registration(source, owner, observed_revision),
      }
    }

    /// Replaces one explicitly complete dependency plan after validating every old and new source.
    #[allow(
      dead_code,
      reason = "complete dependency plans remain inert until parking authority cutover"
    )]
    pub(crate) fn commit_negative_dependency_plan(
      owner: PendingCheckOwner,
      desired: &[DependencyPlanSource],
      timed_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyPlanMutation, DependencyRegistrationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRegistrationError::TransactionRequired);
      }
      if desired.len() > T::MaxContractSteps::get() as usize {
        return Err(DependencyRegistrationError::PlanTooLarge);
      }
      if PendingCheckOwners::<T>::get(owner.actor.actor_id) != Some(owner) {
        return Err(DependencyRegistrationError::PendingOwnerMismatch);
      }
      for (index, entry) in desired
        .iter(/* deos-bypass: bounded-iter -- MaxContractSteps bounds the complete plan. */)
        .enumerate()
      {
        if desired[..index]
          .iter(/* deos-bypass: bounded-iter -- a prefix of the MaxContractSteps plan. */)
          .any(|prior| prior.source == entry.source)
        {
          return Err(DependencyRegistrationError::DuplicateSource);
        }
        let state = DependencyRevisions::<T>::get(entry.source);
        if state.exhausted {
          return Err(DependencyRegistrationError::SourceExhausted);
        }
        if state.revision != entry.observed_revision {
          return Err(DependencyRegistrationError::RevisionMismatch);
        }
      }

      let old = DependencyPlans::<T>::get(owner.actor.actor_id);
      for registration in &old {
        if registration.handle.actor.actor_id != owner.actor.actor_id
          || DependencyRegistrations::<T>::get(registration.source, owner.actor.actor_id)
            != Some(registration.handle)
        {
          return Err(DependencyRegistrationError::StoredPlanMismatch);
        }
        Self::dependency_registration_position(
          registration.source,
          owner.actor.actor_id,
          registration.handle,
        )?;
      }
      let old_timed_review = DependencyTimedReviews::<T>::get(owner.actor.actor_id);
      if old_timed_review.is_some_and(|value| value.owner.actor != owner.actor) {
        return Err(DependencyRegistrationError::StoredPlanMismatch);
      }
      if let Some(deadline) = timed_review {
        let future = match deadline {
          WakeupKey::Block(block) => block > frame_system::Pallet::<T>::block_number(),
          WakeupKey::Tick(tick) => {
            tick
              > Self::current_scheduler_tick()
                .map_err(|_| DependencyRegistrationError::ClockUnavailable)?
          }
        };
        if !future {
          return Err(DependencyRegistrationError::DeadlineNotFuture);
        }
      }
      for entry in desired {
        let current = DependencyRegistrations::<T>::get(entry.source, owner.actor.actor_id);
        let old_registration = old
          .iter(/* deos-bypass: bounded-iter -- stored plan is MaxContractSteps-bounded. */)
          .find(|value| value.source == entry.source);
        if current != old_registration.map(|value| value.handle) {
          return Err(DependencyRegistrationError::StoredPlanMismatch);
        }
        if current.is_none() {
          let header = DependencyRegistrationHeaders::<T>::get(entry.source);
          if header.count >= T::MaxActiveActors::get()
            || (DependencyRevisions::<T>::get(entry.source)
              .scan_target
              .is_some()
              && header.next_index >= u64::from(T::MaxActiveActors::get()))
            || (header.free_count == 0 && header.next_index >= u64::from(T::MaxActiveActors::get()))
          {
            return Err(DependencyRegistrationError::CapacityExceeded);
          }
        }
      }

      let mut mutation = DependencyPlanMutation::default();
      let mut next = BoundedVec::<DependencyPlanRegistration, T::MaxContractSteps>::default();
      for entry in desired {
        let result = match DependencyRegistrations::<T>::get(entry.source, owner.actor.actor_id) {
          Some(current) => Self::replace_dependency_registration(
            entry.source,
            current,
            owner,
            entry.observed_revision,
          )?,
          None => {
            Self::install_dependency_registration(entry.source, owner, entry.observed_revision)?
          }
        };
        let counter = match result {
          DependencyRegistrationMutation::Installed => &mut mutation.installed,
          DependencyRegistrationMutation::Unchanged => &mut mutation.retained,
          DependencyRegistrationMutation::Replaced => &mut mutation.replaced,
          DependencyRegistrationMutation::Removed => {
            return Err(DependencyRegistrationError::CorruptTopology);
          }
        };
        *counter = counter
          .checked_add(1)
          .ok_or(DependencyRegistrationError::CapacityExceeded)?;
        next
          .try_push(DependencyPlanRegistration {
            source: entry.source,
            handle: DependencyRegistrationHandle {
              actor: owner.actor,
              plan_revision: owner.plan_revision,
              acknowledged_revision: entry.observed_revision,
            },
          })
          .map_err(|_| DependencyRegistrationError::PlanTooLarge)?;
      }
      for registration in &old {
        if !desired
          .iter(/* deos-bypass: bounded-iter -- MaxContractSteps bounds the complete plan. */)
          .any(|entry| entry.source == registration.source)
        {
          Self::remove_dependency_registration(registration.source, registration.handle)?;
          mutation.removed = mutation
            .removed
            .checked_add(1)
            .ok_or(DependencyRegistrationError::CapacityExceeded)?;
        }
      }
      mutation.timed_review = match (old_timed_review, timed_review) {
        (None, None) => DependencyTimedReviewMutation::None,
        (None, Some(deadline)) => {
          DependencyTimedReviews::<T>::insert(
            owner.actor.actor_id,
            DependencyTimedReview { owner, deadline },
          );
          DependencyTimedReviewMutation::Installed
        }
        (Some(current), Some(deadline))
          if current == (DependencyTimedReview { owner, deadline }) =>
        {
          DependencyTimedReviewMutation::Retained
        }
        (Some(_), Some(deadline)) => {
          DependencyTimedReviews::<T>::insert(
            owner.actor.actor_id,
            DependencyTimedReview { owner, deadline },
          );
          DependencyTimedReviewMutation::Replaced
        }
        (Some(_), None) => {
          DependencyTimedReviews::<T>::remove(owner.actor.actor_id);
          DependencyTimedReviewMutation::Removed
        }
      };
      DependencyPlans::<T>::insert(owner.actor.actor_id, next);
      Ok(mutation)
    }

    /// Consumes one exact Pending event only after its complete successor plan is durable.
    #[allow(
      dead_code,
      reason = "Pending-event consumption remains inert until parking authority cutover"
    )]
    pub(crate) fn consume_pending_dependency_event(
      expected: PendingDependencyEvent,
      desired: &[DependencyPlanSource],
      timed_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyPlanMutation, DependencyRegistrationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRegistrationError::TransactionRequired);
      }
      match PendingDependencyEvents::<T>::get(expected.owner.actor.actor_id) {
        None => return Err(DependencyRegistrationError::PendingEventMissing),
        Some(current) if current != expected => {
          return Err(DependencyRegistrationError::PendingEventMismatch);
        }
        Some(_) => {}
      }
      if PendingCheckOwners::<T>::get(expected.owner.actor.actor_id) != Some(expected.owner) {
        return Err(DependencyRegistrationError::PendingOwnerMismatch);
      }
      let source_state = DependencyRevisions::<T>::get(expected.source);
      if source_state.exhausted {
        return Err(DependencyRegistrationError::SourceExhausted);
      }
      if source_state.revision < expected.revision {
        return Err(DependencyRegistrationError::RevisionMismatch);
      }
      let registration =
        DependencyRegistrations::<T>::get(expected.source, expected.owner.actor.actor_id)
          .ok_or(DependencyRegistrationError::RegistrationMissing)?;
      if registration.actor != expected.owner.actor
        || registration.plan_revision != expected.owner.plan_revision
        || registration.acknowledged_revision < expected.revision
      {
        return Err(DependencyRegistrationError::CurrentRegistrationMismatch);
      }
      let mutation = Self::commit_negative_dependency_plan(expected.owner, desired, timed_review)?;
      match PendingDependencyEvents::<T>::get(expected.owner.actor.actor_id) {
        Some(current) if current == expected => {}
        _ => return Err(DependencyRegistrationError::PendingEventMismatch),
      }
      PendingDependencyEvents::<T>::remove(expected.owner.actor.actor_id);
      Ok(mutation)
    }

    /// Consumes one exact Pending review only after its complete successor plan is durable.
    #[allow(
      dead_code,
      reason = "Pending-review consumption remains inert until parking authority cutover"
    )]
    pub(crate) fn consume_pending_dependency_review(
      expected: DependencyTimedReview<BlockNumberFor<T>>,
      desired: &[DependencyPlanSource],
      timed_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyPlanMutation, DependencyRegistrationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRegistrationError::TransactionRequired);
      }
      match PendingDependencyReviews::<T>::get(expected.owner.actor.actor_id) {
        None => return Err(DependencyRegistrationError::PendingReviewMissing),
        Some(current) if current != expected => {
          return Err(DependencyRegistrationError::PendingReviewMismatch);
        }
        Some(_) => {}
      }
      let mutation = Self::commit_negative_dependency_plan(expected.owner, desired, timed_review)?;
      match PendingDependencyReviews::<T>::get(expected.owner.actor.actor_id) {
        Some(current) if current == expected => {}
        _ => return Err(DependencyRegistrationError::PendingReviewMismatch),
      }
      PendingDependencyReviews::<T>::remove(expected.owner.actor.actor_id);
      Ok(mutation)
    }

    /// Publishes one exact due review into durable Pending authority before releasing its deadline.
    #[allow(
      dead_code,
      reason = "due-review publication remains inert until deadline traversal cutover"
    )]
    pub(crate) fn publish_due_dependency_review(
      expected: DependencyTimedReview<BlockNumberFor<T>>,
    ) -> Result<DependencyDueReviewMutation, DependencyDueReviewError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyDueReviewError::TransactionRequired);
      }
      match PendingCheckOwners::<T>::get(expected.owner.actor.actor_id) {
        None => return Err(DependencyDueReviewError::PendingOwnerMissing),
        Some(owner) if owner != expected.owner => {
          return Err(DependencyDueReviewError::PendingOwnerMismatch);
        }
        Some(_) => {}
      }
      let due = match expected.deadline {
        WakeupKey::Block(block) => block <= frame_system::Pallet::<T>::block_number(),
        WakeupKey::Tick(tick) => {
          tick
            <= Self::current_scheduler_tick()
              .map_err(|_| DependencyDueReviewError::ClockUnavailable)?
        }
      };
      if !due {
        return Err(DependencyDueReviewError::NotDue);
      }
      if PendingDependencyEvents::<T>::contains_key(expected.owner.actor.actor_id) {
        return Err(DependencyDueReviewError::DestinationOccupied);
      }
      match PendingDependencyReviews::<T>::get(expected.owner.actor.actor_id) {
        Some(current) if current == expected => {
          if DependencyTimedReviews::<T>::contains_key(expected.owner.actor.actor_id) {
            return Err(DependencyDueReviewError::DestinationOccupied);
          }
          return Ok(DependencyDueReviewMutation::AlreadyPending);
        }
        Some(_) => return Err(DependencyDueReviewError::DestinationOccupied),
        None => {}
      }
      match DependencyTimedReviews::<T>::get(expected.owner.actor.actor_id) {
        None => return Err(DependencyDueReviewError::ReviewMissing),
        Some(current) if current != expected => {
          return Err(DependencyDueReviewError::ReviewMismatch);
        }
        Some(_) => {}
      }
      PendingDependencyReviews::<T>::insert(expected.owner.actor.actor_id, expected);
      DependencyTimedReviews::<T>::remove(expected.owner.actor.actor_id);
      Ok(DependencyDueReviewMutation::Published)
    }

    /// Installs or validates one exact dependency registration without releasing old authority.
    #[allow(
      dead_code,
      reason = "dependency registrations remain inert until parking authority cutover"
    )]
    pub(crate) fn install_dependency_registration(
      source: DependencySourceId,
      owner: PendingCheckOwner,
      acknowledged_revision: DependencyRevision,
    ) -> Result<DependencyRegistrationMutation, DependencyRegistrationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRegistrationError::TransactionRequired);
      }
      let handle = Self::validate_dependency_registration(source, owner, acknowledged_revision)?;
      match DependencyRegistrations::<T>::get(source, owner.actor.actor_id) {
        Some(current) if current == handle => {
          Self::dependency_registration_position(source, owner.actor.actor_id, current)?;
          Ok(DependencyRegistrationMutation::Unchanged)
        }
        Some(_) => Err(DependencyRegistrationError::RegistrationAlreadyExists),
        None => {
          let mut header = DependencyRegistrationHeaders::<T>::get(source);
          let next_count = header
            .count
            .checked_add(1)
            .ok_or(DependencyRegistrationError::CapacityExceeded)?;
          if next_count > T::MaxActiveActors::get() {
            return Err(DependencyRegistrationError::CapacityExceeded);
          }
          let scan_active = DependencyRevisions::<T>::get(source).scan_target.is_some();
          let (position, reused_free_index) = if !scan_active && header.free_count > 0 {
            let free_index = header.free_count - 1;
            let position = DependencyRegistrationFreePositions::<T>::get(source, free_index)
              .ok_or(DependencyRegistrationError::CorruptTopology)?;
            (position, Some(free_index))
          } else {
            if header.next_index >= u64::from(T::MaxActiveActors::get()) {
              return Err(DependencyRegistrationError::CapacityExceeded);
            }
            let index = header.next_index;
            header.next_index = index
              .checked_add(1)
              .ok_or(DependencyRegistrationError::CapacityExceeded)?;
            (
              DependencyRegistrationPosition {
                page: index / 32,
                slot: (index % 32) as u8,
              },
              None,
            )
          };
          let mut page = DependencyRegistrationPages::<T>::get(source, position.page).unwrap_or(
            DependencyRegistrationPage {
              entries: BoundedVec::try_from(alloc::vec![None; 32])
                .map_err(|_| DependencyRegistrationError::CorruptTopology)?,
            },
          );
          if page
            .entries
            .get(position.slot as usize)
            .and_then(Option::as_ref)
            .is_some()
          {
            return Err(DependencyRegistrationError::CorruptTopology);
          }
          page.entries[position.slot as usize] = Some(handle);
          header.count = next_count;
          if let Some(free_index) = reused_free_index {
            DependencyRegistrationFreePositions::<T>::remove(source, free_index);
            header.free_count = free_index;
          }
          DependencyRegistrationPages::<T>::insert(source, position.page, page);
          DependencyRegistrationHeaders::<T>::insert(source, header);
          DependencyRegistrationPositions::<T>::insert(source, owner.actor.actor_id, position);
          DependencyRegistrations::<T>::insert(source, owner.actor.actor_id, handle);
          Ok(DependencyRegistrationMutation::Installed)
        }
      }
    }

    /// Replaces one exact registration only after its successor is fully validated.
    #[allow(
      dead_code,
      reason = "dependency registrations remain inert until parking authority cutover"
    )]
    pub(crate) fn replace_dependency_registration(
      source: DependencySourceId,
      current: DependencyRegistrationHandle,
      owner: PendingCheckOwner,
      acknowledged_revision: DependencyRevision,
    ) -> Result<DependencyRegistrationMutation, DependencyRegistrationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRegistrationError::TransactionRequired);
      }
      let replacement =
        Self::validate_dependency_registration(source, owner, acknowledged_revision)?;
      let stored = DependencyRegistrations::<T>::get(source, current.actor.actor_id)
        .ok_or(DependencyRegistrationError::RegistrationMissing)?;
      if stored != current {
        return Err(DependencyRegistrationError::CurrentRegistrationMismatch);
      }
      if current.actor.actor_id != replacement.actor.actor_id {
        return Err(DependencyRegistrationError::PendingOwnerMismatch);
      }
      let position =
        Self::dependency_registration_position(source, current.actor.actor_id, current)?;
      if current == replacement {
        return Ok(DependencyRegistrationMutation::Unchanged);
      }
      let mut page = DependencyRegistrationPages::<T>::get(source, position.page)
        .ok_or(DependencyRegistrationError::CorruptTopology)?;
      page.entries[position.slot as usize] = Some(replacement);
      DependencyRegistrationPages::<T>::insert(source, position.page, page);
      DependencyRegistrations::<T>::insert(source, replacement.actor.actor_id, replacement);
      Ok(DependencyRegistrationMutation::Replaced)
    }

    /// Removes only the exact registration named by its generation/plan/revision handle.
    #[allow(
      dead_code,
      reason = "dependency registrations remain inert until parking authority cutover"
    )]
    pub(crate) fn remove_dependency_registration(
      source: DependencySourceId,
      expected: DependencyRegistrationHandle,
    ) -> Result<DependencyRegistrationMutation, DependencyRegistrationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DependencyRegistrationError::TransactionRequired);
      }
      let stored = DependencyRegistrations::<T>::get(source, expected.actor.actor_id)
        .ok_or(DependencyRegistrationError::RegistrationMissing)?;
      if stored != expected {
        return Err(DependencyRegistrationError::CurrentRegistrationMismatch);
      }
      let position =
        Self::dependency_registration_position(source, expected.actor.actor_id, expected)?;
      let mut page = DependencyRegistrationPages::<T>::get(source, position.page)
        .ok_or(DependencyRegistrationError::CorruptTopology)?;
      page.entries[position.slot as usize] = None;
      let mut header = DependencyRegistrationHeaders::<T>::get(source);
      header.count = header
        .count
        .checked_sub(1)
        .ok_or(DependencyRegistrationError::CorruptTopology)?;
      if header.free_count >= T::MaxActiveActors::get() {
        return Err(DependencyRegistrationError::CorruptTopology);
      }
      let next_free_count = header
        .free_count
        .checked_add(1)
        .ok_or(DependencyRegistrationError::CorruptTopology)?;
      DependencyRegistrationFreePositions::<T>::insert(source, header.free_count, position);
      header.free_count = next_free_count;
      DependencyRegistrationPages::<T>::insert(source, position.page, page);
      DependencyRegistrationHeaders::<T>::insert(source, header);
      DependencyRegistrationPositions::<T>::remove(source, expected.actor.actor_id);
      DependencyRegistrations::<T>::remove(source, expected.actor.actor_id);
      Ok(DependencyRegistrationMutation::Removed)
    }

    /// Atomically unlinks one service member and irreversibly retires its canonical process.
    /// Canonical zero-Step and effectful terminal outcomes use this owner after finalization.
    pub(crate) fn retire_service_member(
      actor: ActorRef,
      reason: CloseReason,
    ) -> Result<(), ServiceRetirementError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = Self::remove_service_member(actor)
          .map_err(ServiceRetirementError::Ring)
          .and_then(|_| {
            let current = ActorProcesses::<T>::get(actor.actor_id).ok_or(
              ServiceRetirementError::Process(ProcessPublicationError::ProcessMissing),
            )?;
            Self::publish_legacy_process_transition(
              actor.actor_id,
              current,
              ProcessTransitionObligation::RetireOrDisable,
              LegacyProcessTransition::Retire(reason),
            )
            .map(|_| ())
            .map_err(ServiceRetirementError::Process)
          });
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Removes exactly one generation-bound member from the inert service ring.
    #[allow(
      dead_code,
      reason = "service-ring mutation remains unreachable until the complete carrier cutover"
    )]
    pub(crate) fn remove_service_member(
      actor: ActorRef,
    ) -> Result<ServiceNode<BlockNumberFor<T>>, ServiceRingMutationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(ServiceRingMutationError::TransactionRequired);
      }
      if ActorControlLocators::<T>::contains_key(actor.actor_id)
        || ActorUnsignaledControlCells::<T>::contains_key(actor.actor_id)
      {
        return Err(ServiceRingMutationError::LegacyAuthorityPresent);
      }
      let process =
        ActorProcesses::<T>::get(actor.actor_id).ok_or(ServiceRingMutationError::ProcessMissing)?;
      let node =
        ServiceNodes::<T>::get(actor.actor_id).ok_or(ServiceRingMutationError::MemberMissing)?;
      if node.generation != actor.generation || process.generation != actor.generation {
        return Err(ServiceRingMutationError::StaleGeneration);
      }
      if process.status != ProcessStatus::Serving
        || process.residence != Some(ProcessResidence::Service(node.kind))
      {
        return Err(ServiceRingMutationError::ProcessResidenceMismatch);
      }

      let mut header = ServiceHeader::<T>::get();
      if header.count == 0 || header.cursor.is_none() {
        return Err(ServiceRingMutationError::CorruptRing);
      }
      if header.count == 1 {
        if header.cursor != Some(actor) || node.previous != actor || node.next != actor {
          return Err(ServiceRingMutationError::CorruptRing);
        }
        header = ServiceHeaderRecord::default();
      } else {
        let mut previous = ServiceNodes::<T>::get(node.previous.actor_id)
          .filter(|value| value.generation == node.previous.generation)
          .ok_or(ServiceRingMutationError::CorruptRing)?;
        let mut next = ServiceNodes::<T>::get(node.next.actor_id)
          .filter(|value| value.generation == node.next.generation)
          .ok_or(ServiceRingMutationError::CorruptRing)?;
        if previous.next != actor || next.previous != actor {
          return Err(ServiceRingMutationError::CorruptRing);
        }
        previous.next = node.next;
        next.previous = node.previous;
        if node.previous.actor_id == node.next.actor_id {
          previous.previous = node.previous;
          ServiceNodes::<T>::insert(node.previous.actor_id, previous);
        } else {
          ServiceNodes::<T>::insert(node.previous.actor_id, previous);
          ServiceNodes::<T>::insert(node.next.actor_id, next);
        }
        if header.cursor == Some(actor) {
          header.cursor = Some(node.next);
        }
        header.count -= 1;
      }
      ServiceNodes::<T>::remove(actor.actor_id);
      ServiceHeader::<T>::put(header);
      Ok(node)
    }

    fn deadline_handle_matches_process(
      handle: DeadlineHandleOf<T>,
      process: &ActorProcessOf<T>,
    ) -> bool {
      process.generation == handle.actor.generation
        && process.status == ProcessStatus::Serving
        && (process.residence
          == Some(ProcessResidence::Deadline {
            key: handle.key,
            page: handle.page,
            slot: handle.slot,
          })
          || matches!(
            process.residence,
            Some(ProcessResidence::Parked(_))
              if DependencyTimedReviews::<T>::get(handle.actor.actor_id)
                .is_some_and(|review| {
                  review.owner.actor == handle.actor
                    && review.deadline == handle.key
                    && PendingCheckOwners::<T>::get(handle.actor.actor_id) == Some(review.owner)
                })
          ))
    }

    /// Inserts one generation-bound process obligation into an exact retained deadline slot.
    #[allow(
      dead_code,
      reason = "deadline carrier remains unreachable until atomic cutover"
    )]
    pub(crate) fn insert_deadline_member(
      handle: DeadlineHandleOf<T>,
    ) -> Result<(), DeadlineMutationError> {
      Self::insert_deadline_member_for_owner(handle, false)
    }

    /// Inserts one independent temporal Trigger obligation into the shared deadline carrier.
    #[allow(
      dead_code,
      reason = "temporal Trigger carrier remains unreachable until atomic cutover"
    )]
    pub(crate) fn insert_trigger_deadline_member(
      handle: DeadlineHandleOf<T>,
    ) -> Result<(), DeadlineMutationError> {
      Self::insert_deadline_member_for_owner(handle, true)
    }

    fn insert_deadline_member_for_owner(
      handle: DeadlineHandleOf<T>,
      trigger_owner: bool,
    ) -> Result<(), DeadlineMutationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DeadlineMutationError::TransactionRequired);
      }
      if ActorControlLocators::<T>::contains_key(handle.actor.actor_id)
        || ActorUnsignaledControlCells::<T>::contains_key(handle.actor.actor_id)
      {
        return Err(DeadlineMutationError::LegacyAuthorityPresent);
      }
      if trigger_owner {
        let Some(ActorSemanticState::Active(semantic)) =
          ActorSemanticStates::<T>::get(handle.actor.actor_id)
        else {
          return Err(DeadlineMutationError::ProcessMissing);
        };
        let expected_pointer = match handle.key {
          WakeupKey::Tick(tick) => Some(TriggerWakeupPointer {
            tick,
            page_id: handle.page,
            slot: u32::from(handle.slot),
          }),
          WakeupKey::Block(_) => return Err(DeadlineMutationError::InvalidDestination),
        };
        if semantic.generation != handle.actor.generation
          || semantic.hot.trigger_wakeup_pointer != expected_pointer
        {
          return Err(DeadlineMutationError::ProcessResidenceMismatch);
        }
      } else {
        let process = ActorProcesses::<T>::get(handle.actor.actor_id)
          .ok_or(DeadlineMutationError::ProcessMissing)?;
        if !Self::deadline_handle_matches_process(handle, &process) {
          return Err(DeadlineMutationError::ProcessResidenceMismatch);
        }
      }
      let reverse_exists = if trigger_owner {
        TriggerDeadlineHandles::<T>::contains_key(handle.actor.actor_id)
      } else {
        DeadlineHandles::<T>::contains_key(handle.actor.actor_id)
      };
      if reverse_exists {
        return Err(DeadlineMutationError::MemberAlreadyExists);
      }
      let slot = usize::from(handle.slot);
      if slot >= 32 {
        return Err(DeadlineMutationError::InvalidDestination);
      }
      let mut header = DeadlineHeaders::<T>::get(handle.key);
      let creates_bucket = header.is_none();
      let mut page = DeadlinePages::<T>::get(handle.key, handle.page);
      match (&header, &page) {
        (None, None) if handle.page == 0 => {}
        (Some(header), None) if handle.page == header.next_page => {}
        (Some(_), Some(_)) => {}
        _ => return Err(DeadlineMutationError::InvalidDestination),
      }
      if page
        .as_ref()
        .is_some_and(|page| page.entries[slot].is_some())
      {
        return Err(DeadlineMutationError::InvalidDestination);
      }
      let next_count = header.as_ref().map_or(Ok(1), |value| {
        value
          .count
          .checked_add(1)
          .ok_or(DeadlineMutationError::CapacityExceeded)
      })?;
      if next_count > T::MaxActiveActors::get() {
        return Err(DeadlineMutationError::CapacityExceeded);
      }
      if page.is_none() {
        let previous_page = header.as_ref().map(|value| value.last_page);
        page = Some(DeadlinePage {
          previous_page,
          next_page: None,
          live_entries: 0,
          entries: BoundedVec::try_from(alloc::vec![None; 32])
            .map_err(|_| DeadlineMutationError::CorruptCarrier)?,
        });
        if let Some(previous_page) = previous_page {
          let mut previous = DeadlinePages::<T>::get(handle.key, previous_page)
            .ok_or(DeadlineMutationError::CorruptCarrier)?;
          if previous.next_page.is_some() {
            return Err(DeadlineMutationError::CorruptCarrier);
          }
          previous.next_page = Some(handle.page);
          DeadlinePages::<T>::insert(handle.key, previous_page, previous);
        }
      }
      let mut page = page.ok_or(DeadlineMutationError::CorruptCarrier)?;
      if page.live_entries >= 32 || page.entries.len() != 32 {
        return Err(DeadlineMutationError::PageFull);
      }
      page.entries[slot] = Some(handle.actor);
      page.live_entries = page
        .live_entries
        .checked_add(1)
        .ok_or(DeadlineMutationError::CapacityExceeded)?;
      DeadlinePages::<T>::insert(handle.key, handle.page, page);
      match header.as_mut() {
        Some(header) => {
          if handle.page == header.next_page {
            header.last_page = handle.page;
            header.next_page = header
              .next_page
              .checked_add(1)
              .ok_or(DeadlineMutationError::CapacityExceeded)?;
            header.page_count = header
              .page_count
              .checked_add(1)
              .ok_or(DeadlineMutationError::CapacityExceeded)?;
          }
          header.count = next_count;
        }
        None => {
          header = Some(DeadlineHeader {
            first_page: 0,
            last_page: 0,
            next_page: 1,
            page_count: 1,
            count: 1,
          });
        }
      }
      let header = header.ok_or(DeadlineMutationError::CorruptCarrier)?;
      DeadlineHeaders::<T>::insert(handle.key, header);
      if trigger_owner {
        TriggerDeadlineHandles::<T>::insert(handle.actor.actor_id, handle);
      } else {
        DeadlineHandles::<T>::insert(handle.actor.actor_id, handle);
      }
      if creates_bucket {
        Self::insert_deadline_index(handle.key)?;
      } else {
        Self::update_deadline_index(handle.key)?;
      }
      Ok(())
    }

    /// Removes exactly one generation-bound process deadline and unlinks an empty retained page.
    #[allow(
      dead_code,
      reason = "deadline carrier remains unreachable until atomic cutover"
    )]
    pub(crate) fn remove_deadline_member(
      actor: ActorRef,
    ) -> Result<DeadlineHandleOf<T>, DeadlineMutationError> {
      Self::remove_deadline_member_for_owner(actor, false)
    }

    /// Removes the independent temporal Trigger deadline without changing process residence.
    #[allow(
      dead_code,
      reason = "temporal Trigger carrier remains unreachable until atomic cutover"
    )]
    pub(crate) fn remove_trigger_deadline_member(
      actor: ActorRef,
    ) -> Result<DeadlineHandleOf<T>, DeadlineMutationError> {
      Self::remove_deadline_member_for_owner(actor, true)
    }

    fn remove_deadline_member_for_owner(
      actor: ActorRef,
      trigger_owner: bool,
    ) -> Result<DeadlineHandleOf<T>, DeadlineMutationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DeadlineMutationError::TransactionRequired);
      }
      if ActorControlLocators::<T>::contains_key(actor.actor_id)
        || ActorUnsignaledControlCells::<T>::contains_key(actor.actor_id)
      {
        return Err(DeadlineMutationError::LegacyAuthorityPresent);
      }
      let handle = if trigger_owner {
        TriggerDeadlineHandles::<T>::get(actor.actor_id)
      } else {
        DeadlineHandles::<T>::get(actor.actor_id)
      }
      .ok_or(DeadlineMutationError::MemberMissing)?;
      if handle.actor != actor {
        return Err(DeadlineMutationError::StaleGeneration);
      }
      if trigger_owner {
        let Some(ActorSemanticState::Active(semantic)) =
          ActorSemanticStates::<T>::get(actor.actor_id)
        else {
          return Err(DeadlineMutationError::ProcessMissing);
        };
        let expected_pointer = match handle.key {
          WakeupKey::Tick(tick) => Some(TriggerWakeupPointer {
            tick,
            page_id: handle.page,
            slot: u32::from(handle.slot),
          }),
          WakeupKey::Block(_) => return Err(DeadlineMutationError::InvalidDestination),
        };
        if semantic.generation != actor.generation
          || semantic.hot.trigger_wakeup_pointer != expected_pointer
        {
          return Err(DeadlineMutationError::ProcessResidenceMismatch);
        }
      } else {
        let process =
          ActorProcesses::<T>::get(actor.actor_id).ok_or(DeadlineMutationError::ProcessMissing)?;
        if process.generation != actor.generation {
          return Err(DeadlineMutationError::StaleGeneration);
        }
        if !Self::deadline_handle_matches_process(handle, &process) {
          return Err(DeadlineMutationError::ProcessResidenceMismatch);
        }
      }
      let mut header =
        DeadlineHeaders::<T>::get(handle.key).ok_or(DeadlineMutationError::CorruptCarrier)?;
      let mut page = DeadlinePages::<T>::get(handle.key, handle.page)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      let slot = usize::from(handle.slot);
      if page.entries.get(slot) != Some(&Some(actor)) || page.live_entries == 0 || header.count == 0
      {
        return Err(DeadlineMutationError::CorruptCarrier);
      }
      page.entries[slot] = None;
      page.live_entries = page
        .live_entries
        .checked_sub(1)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      header.count = header
        .count
        .checked_sub(1)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      if trigger_owner {
        TriggerDeadlineHandles::<T>::remove(actor.actor_id);
      } else {
        DeadlineHandles::<T>::remove(actor.actor_id);
      }
      if page.live_entries > 0 {
        DeadlinePages::<T>::insert(handle.key, handle.page, page);
        DeadlineHeaders::<T>::insert(handle.key, header);
        Self::update_deadline_index(handle.key)?;
        return Ok(handle);
      }
      if let Some(previous_id) = page.previous_page {
        let mut previous = DeadlinePages::<T>::get(handle.key, previous_id)
          .ok_or(DeadlineMutationError::CorruptCarrier)?;
        if previous.next_page != Some(handle.page) {
          return Err(DeadlineMutationError::CorruptCarrier);
        }
        previous.next_page = page.next_page;
        DeadlinePages::<T>::insert(handle.key, previous_id, previous);
      }
      if let Some(next_id) = page.next_page {
        let mut next = DeadlinePages::<T>::get(handle.key, next_id)
          .ok_or(DeadlineMutationError::CorruptCarrier)?;
        if next.previous_page != Some(handle.page) {
          return Err(DeadlineMutationError::CorruptCarrier);
        }
        next.previous_page = page.previous_page;
        DeadlinePages::<T>::insert(handle.key, next_id, next);
      }
      DeadlinePages::<T>::remove(handle.key, handle.page);
      header.page_count = header
        .page_count
        .checked_sub(1)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      if header.page_count == 0 {
        if header.count != 0 {
          return Err(DeadlineMutationError::CorruptCarrier);
        }
        DeadlineHeaders::<T>::remove(handle.key);
        Self::remove_deadline_index(handle.key)?;
      } else {
        if header.first_page == handle.page {
          header.first_page = page
            .next_page
            .ok_or(DeadlineMutationError::CorruptCarrier)?;
        }
        if header.last_page == handle.page {
          header.last_page = page
            .previous_page
            .ok_or(DeadlineMutationError::CorruptCarrier)?;
        }
        DeadlineHeaders::<T>::insert(handle.key, header);
        Self::update_deadline_index(handle.key)?;
      }
      Ok(handle)
    }

    /// Moves one deadline member atomically after preflighting the exact destination slot.
    #[allow(
      dead_code,
      reason = "deadline carrier remains unreachable until atomic cutover"
    )]
    pub(crate) fn move_deadline_member(
      actor: ActorRef,
      destination: DeadlineHandleOf<T>,
    ) -> Result<(), DeadlineMutationError> {
      if destination.actor != actor {
        return Err(DeadlineMutationError::StaleGeneration);
      }
      let slot = usize::from(destination.slot);
      if slot >= 32
        || DeadlinePages::<T>::get(destination.key, destination.page)
          .is_some_and(|page| page.entries[slot].is_some())
      {
        return Err(DeadlineMutationError::InvalidDestination);
      }
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let outcome = (|| {
          Self::remove_deadline_member(actor)?;
          let mut process = ActorProcesses::<T>::get(actor.actor_id)
            .ok_or(DeadlineMutationError::ProcessMissing)?;
          process.residence = Some(ProcessResidence::Deadline {
            key: destination.key,
            page: destination.page,
            slot: destination.slot,
          });
          ActorProcesses::<T>::insert(actor.actor_id, process);
          Self::insert_deadline_member(destination)
        })();
        match outcome {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    fn deadline_index_page_and_slot(index: u32) -> (u64, usize) {
      (u64::from(index / 32), (index % 32) as usize)
    }

    fn deadline_index_get(clock: WakeupClock, index: u32) -> Option<WakeupKey<BlockNumberFor<T>>> {
      let (page, slot) = Self::deadline_index_page_and_slot(index);
      DeadlineIndexPages::<T>::get(clock, page).and_then(|page| page.get(slot).copied())
    }

    fn deadline_index_set(
      clock: WakeupClock,
      index: u32,
      key: WakeupKey<BlockNumberFor<T>>,
    ) -> Result<(), DeadlineIndexMutationError> {
      if key.clock() != clock {
        return Err(DeadlineIndexMutationError::CorruptHeap);
      }
      let (page_id, slot) = Self::deadline_index_page_and_slot(index);
      let mut page = DeadlineIndexPages::<T>::get(clock, page_id).unwrap_or_default();
      if slot < page.len() {
        page[slot] = key;
      } else if slot == page.len() {
        page
          .try_push(key)
          .map_err(|_| DeadlineIndexMutationError::CapacityExceeded)?;
      } else {
        return Err(DeadlineIndexMutationError::CorruptHeap);
      }
      DeadlineIndexPages::<T>::insert(clock, page_id, page);
      Ok(())
    }

    fn deadline_index_swap(
      clock: WakeupClock,
      left: u32,
      right: u32,
    ) -> Result<(), DeadlineIndexMutationError> {
      let left_key =
        Self::deadline_index_get(clock, left).ok_or(DeadlineIndexMutationError::CorruptHeap)?;
      let right_key =
        Self::deadline_index_get(clock, right).ok_or(DeadlineIndexMutationError::CorruptHeap)?;
      if DeadlineIndexPositions::<T>::get(left_key) != Some(left)
        || DeadlineIndexPositions::<T>::get(right_key) != Some(right)
      {
        return Err(DeadlineIndexMutationError::StaleIndex);
      }
      Self::deadline_index_set(clock, left, right_key)?;
      Self::deadline_index_set(clock, right, left_key)?;
      DeadlineIndexPositions::<T>::insert(right_key, left);
      DeadlineIndexPositions::<T>::insert(left_key, right);
      Ok(())
    }

    fn deadline_index_height_bound() -> u32 {
      u32::BITS.saturating_sub(T::MaxActiveActors::get().max(1).leading_zeros())
    }

    fn validate_deadline_index_header(
      key: WakeupKey<BlockNumberFor<T>>,
    ) -> Result<(), DeadlineIndexMutationError> {
      if ActorWaitingOccupancies::<T>::get(key) > 0
        || ActorWaitingCursorIndices::<T>::contains_key(key)
      {
        return Err(DeadlineIndexMutationError::LegacyAuthorityPresent);
      }
      let header =
        DeadlineHeaders::<T>::get(key).ok_or(DeadlineIndexMutationError::HeaderMissing)?;
      if header.count == 0
        || header.page_count == 0
        || header.count > header.page_count.saturating_mul(32)
        || header.first_page > header.last_page
        || !DeadlinePages::<T>::contains_key(key, header.first_page)
        || !DeadlinePages::<T>::contains_key(key, header.last_page)
      {
        return Err(DeadlineIndexMutationError::CorruptHeader);
      }
      Ok(())
    }

    /// Inserts one nonempty canonical deadline bucket into its clock-local inert min-heap.
    #[allow(
      dead_code,
      reason = "deadline index remains unreachable until atomic cutover"
    )]
    pub(crate) fn insert_deadline_index(
      key: WakeupKey<BlockNumberFor<T>>,
    ) -> Result<(), DeadlineIndexMutationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DeadlineIndexMutationError::TransactionRequired);
      }
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let outcome = (|| {
          Self::validate_deadline_index_header(key)?;
          if DeadlineIndexPositions::<T>::contains_key(key) {
            return Err(DeadlineIndexMutationError::KeyAlreadyExists);
          }
          let clock = key.clock();
          let len = DeadlineIndexLen::<T>::get(clock);
          let next_len = len
            .checked_add(1)
            .ok_or(DeadlineIndexMutationError::CapacityExceeded)?;
          if len >= T::MaxActiveActors::get() {
            return Err(DeadlineIndexMutationError::CapacityExceeded);
          }
          Self::deadline_index_set(clock, len, key)?;
          DeadlineIndexPositions::<T>::insert(key, len);
          DeadlineIndexLen::<T>::insert(clock, next_len);
          let mut current = len;
          for _ in 0..Self::deadline_index_height_bound() {
            if current == 0 {
              break;
            }
            let parent = (current - 1) / 2;
            let parent_key = Self::deadline_index_get(clock, parent)
              .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
            let current_key = Self::deadline_index_get(clock, current)
              .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
            if parent_key <= current_key {
              break;
            }
            Self::deadline_index_swap(clock, parent, current)?;
            current = parent;
          }
          Ok(())
        })();
        match outcome {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Revalidates and repairs one indexed nonempty bucket within the bounded heap height.
    #[allow(
      dead_code,
      reason = "deadline index remains unreachable until atomic cutover"
    )]
    pub(crate) fn update_deadline_index(
      key: WakeupKey<BlockNumberFor<T>>,
    ) -> Result<(), DeadlineIndexMutationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DeadlineIndexMutationError::TransactionRequired);
      }
      Self::validate_deadline_index_header(key)?;
      let clock = key.clock();
      let index =
        DeadlineIndexPositions::<T>::get(key).ok_or(DeadlineIndexMutationError::KeyMissing)?;
      let len = DeadlineIndexLen::<T>::get(clock);
      if index >= len || Self::deadline_index_get(clock, index) != Some(key) {
        return Err(DeadlineIndexMutationError::StaleIndex);
      }
      if index > 0 {
        let parent = (index - 1) / 2;
        if Self::deadline_index_get(clock, parent).ok_or(DeadlineIndexMutationError::CorruptHeap)?
          > key
        {
          return Err(DeadlineIndexMutationError::CorruptHeap);
        }
      }
      let left = index.saturating_mul(2).saturating_add(1);
      for child in [left, left.saturating_add(1)] {
        if child < len
          && Self::deadline_index_get(clock, child)
            .ok_or(DeadlineIndexMutationError::CorruptHeap)?
            < key
        {
          return Err(DeadlineIndexMutationError::CorruptHeap);
        }
      }
      Ok(())
    }

    /// Removes one exact bucket only after its canonical deadline header has been deleted.
    #[allow(
      dead_code,
      reason = "deadline index remains unreachable until atomic cutover"
    )]
    pub(crate) fn remove_deadline_index(
      key: WakeupKey<BlockNumberFor<T>>,
    ) -> Result<(), DeadlineIndexMutationError> {
      if !polkadot_sdk::frame_support::storage::transactional::is_transactional() {
        return Err(DeadlineIndexMutationError::TransactionRequired);
      }
      if ActorWaitingOccupancies::<T>::get(key) > 0
        || ActorWaitingCursorIndices::<T>::contains_key(key)
      {
        return Err(DeadlineIndexMutationError::LegacyAuthorityPresent);
      }
      if DeadlineHeaders::<T>::contains_key(key) {
        return Err(DeadlineIndexMutationError::CorruptHeader);
      }
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let outcome = (|| {
          let clock = key.clock();
          let index =
            DeadlineIndexPositions::<T>::get(key).ok_or(DeadlineIndexMutationError::KeyMissing)?;
          let len = DeadlineIndexLen::<T>::get(clock);
          if index >= len || Self::deadline_index_get(clock, index) != Some(key) {
            return Err(DeadlineIndexMutationError::StaleIndex);
          }
          let last = len
            .checked_sub(1)
            .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
          let last_key =
            Self::deadline_index_get(clock, last).ok_or(DeadlineIndexMutationError::CorruptHeap)?;
          if DeadlineIndexPositions::<T>::get(last_key) != Some(last) {
            return Err(DeadlineIndexMutationError::StaleIndex);
          }
          let (page_id, slot) = Self::deadline_index_page_and_slot(last);
          let mut page = DeadlineIndexPages::<T>::get(clock, page_id)
            .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
          if slot + 1 != page.len() {
            return Err(DeadlineIndexMutationError::CorruptHeap);
          }
          page.pop();
          if page.is_empty() {
            DeadlineIndexPages::<T>::remove(clock, page_id);
          } else {
            DeadlineIndexPages::<T>::insert(clock, page_id, page);
          }
          DeadlineIndexPositions::<T>::remove(key);
          DeadlineIndexLen::<T>::insert(clock, last);
          if index == last {
            return Ok(());
          }
          Self::deadline_index_set(clock, index, last_key)?;
          DeadlineIndexPositions::<T>::insert(last_key, index);
          let mut current = index;
          for _ in 0..Self::deadline_index_height_bound() {
            if current > 0 {
              let parent = (current - 1) / 2;
              let parent_key = Self::deadline_index_get(clock, parent)
                .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
              let current_key = Self::deadline_index_get(clock, current)
                .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
              if parent_key > current_key {
                Self::deadline_index_swap(clock, parent, current)?;
                current = parent;
                continue;
              }
            }
            let left = current.saturating_mul(2).saturating_add(1);
            if left >= last {
              break;
            }
            let right = left.saturating_add(1);
            let left_key = Self::deadline_index_get(clock, left)
              .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
            let mut smallest = left;
            if right < last
              && Self::deadline_index_get(clock, right)
                .ok_or(DeadlineIndexMutationError::CorruptHeap)?
                < left_key
            {
              smallest = right;
            }
            let current_key = Self::deadline_index_get(clock, current)
              .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
            let smallest_key = Self::deadline_index_get(clock, smallest)
              .ok_or(DeadlineIndexMutationError::CorruptHeap)?;
            if current_key <= smallest_key {
              break;
            }
            Self::deadline_index_swap(clock, current, smallest)?;
            current = smallest;
          }
          Ok(())
        })();
        match outcome {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    pub(crate) fn load_frame_control_authority(
      actor_id: ActorId,
    ) -> Option<(
      ActorControlLocation<BlockNumberFor<T>>,
      ActorIdentityOf<T>,
      ActorHotStateOf<T>,
      ActorAdmissionCertificateOf<T>,
    )> {
      let (location, cell) = Self::load_primary_control_cell(actor_id).ok()?;
      let (identity, hot, admission) = Self::project_control_cell(&cell, location)?;
      Some((location, identity, hot, admission))
    }

    pub(crate) fn load_control_authority_with_authority(
      actor_id: ActorId,
    ) -> Option<(
      ActorIdentityOf<T>,
      ActorHotStateOf<T>,
      ActorAdmissionCertificateOf<T>,
    )> {
      let (ActorSemanticState::Active(record), _) =
        Self::load_actor_semantic_state(actor_id).ok()??
      else {
        return None;
      };
      Some((record.identity, record.hot, record.admission))
    }

    #[cfg(feature = "runtime-benchmarks")]
    pub(crate) fn store_frame_control_authority(
      actor_id: ActorId,
      location: ActorControlLocation<BlockNumberFor<T>>,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
    ) -> bool {
      let expected_ticket = match location {
        ActorControlLocation::Ready { ticket } => Some(ticket),
        ActorControlLocation::Unsignaled | ActorControlLocation::Waiting { .. } => None,
      };
      if hot.queue_ticket != expected_ticket || !admission.has_valid_identity() {
        return false;
      }
      let Ok((stored_location, mut cell)) = Self::load_primary_control_cell(actor_id) else {
        return false;
      };
      if stored_location != location {
        return false;
      }
      let Some(control_identity) = Self::control_identity_from_scalar(identity.clone()) else {
        return false;
      };
      cell.identity = control_identity;
      cell.hot = Self::control_hot_from_scalar(hot.clone());
      cell.pipeline_service_identity = pipeline_service_identity(admission.admission_identity);
      cell.admission = admission.clone();
      let Some((restored_identity, restored_hot, restored_admission)) =
        Self::project_control_cell(&cell, location)
      else {
        return false;
      };
      if restored_identity != identity || restored_hot != hot || restored_admission != admission {
        return false;
      }
      Self::store_primary_control_cell(location, cell).is_ok()
    }

    #[cfg(all(test, feature = "runtime-benchmarks"))]
    pub(crate) fn control_load_frame_contract(
      actor_id: ActorId,
      admission: &ActorAdmissionCertificateOf<T>,
    ) -> Option<(ActorContractOf<T>, ActorContractHeadOf<T>)> {
      let head = ActorContractHeads::<T>::get(actor_id)?;
      if !admission.has_valid_identity()
        || admission.semantic_contract_id != head.header.semantic_contract_id
        || admission.body_commitment != head.header.body_commitment
        || admission.admission_identity != head.header.admission_identity
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
      let contract = Self::reconstruct_contract_geometry(actor_id, head.clone(), &chunks)?;
      Some((contract, head))
    }

    #[cfg(all(test, feature = "runtime-benchmarks"))]
    pub(crate) fn control_load_current_step_contract(
      actor_id: ActorId,
      admission: &ActorAdmissionCertificateOf<T>,
      cursor: u32,
    ) -> Option<(
      ActorContractOf<T>,
      LoadedActorStepOf<T>,
      ActorContractHeadOf<T>,
    )> {
      let head = ActorContractHeads::<T>::get(actor_id)?;
      if cursor >= head.header.step_count
        || !admission.has_valid_identity()
        || admission.semantic_contract_id != head.header.semantic_contract_id
        || admission.body_commitment != head.header.body_commitment
        || admission.admission_identity != head.header.admission_identity
      {
        return None;
      }
      let tail_chunk = if cursor == 0 {
        None
      } else {
        let chunk_index = cursor.checked_sub(1)? / MAX_STEPS_PER_TAIL_CHUNK;
        Some((
          chunk_index,
          ActorContractTailChunks::<T>::get(actor_id, chunk_index)?,
        ))
      };
      let loaded_step = Self::load_current_step_from_geometry(
        actor_id,
        &head,
        admission,
        cursor,
        tail_chunk.as_ref().map(|(index, chunk)| (*index, chunk)),
      )?;
      let contract = ActorContract {
        trigger: head.header.trigger.clone(),
        cooldown_blocks: head.header.cooldown_blocks,
        window: head.header.window,
        steps: BoundedVec::try_from(alloc::vec![loaded_step.step.clone()]).ok()?,
        funding: head.header.funding.clone(),
        completion: head.header.completion,
        auto_close_at_cycle_nonce: head.header.auto_close_at_cycle_nonce,
      };
      Some((contract, loaded_step, head))
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
        trigger_runtime_state: hot.trigger_runtime_state.clone(),
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

    pub(crate) fn control_identity_exists(actor_id: ActorId) -> bool {
      ActorSemanticStates::<T>::contains_key(actor_id)
    }

    pub(crate) fn load_control_identity(actor_id: ActorId) -> Option<ActorIdentityOf<T>> {
      ActorSemanticStates::<T>::get(actor_id).map(|state| match state {
        ActorSemanticState::Dormant(record) => record.identity,
        ActorSemanticState::Active(record) => record.identity,
      })
    }

    /// Loads the canonical generation-bound identity for active process carriers.
    pub(crate) fn load_actor_ref(actor_id: ActorId) -> Option<ActorRef> {
      match ActorSemanticStates::<T>::get(actor_id)? {
        ActorSemanticState::Active(record) if record.generation != 0 => Some(ActorRef {
          actor_id,
          generation: record.generation,
        }),
        ActorSemanticState::Dormant(_) | ActorSemanticState::Active(_) => None,
      }
    }

    pub(crate) fn control_hot_exists(actor_id: ActorId) -> bool {
      matches!(
        ActorSemanticStates::<T>::get(actor_id),
        Some(ActorSemanticState::Active(_))
      )
    }

    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn load_control_hot(actor_id: ActorId) -> Option<ActorHotStateOf<T>> {
      match ActorSemanticStates::<T>::get(actor_id)? {
        ActorSemanticState::Active(record) => Some(record.hot),
        ActorSemanticState::Dormant(_) => None,
      }
    }

    #[cfg(feature = "try-runtime")]
    pub(crate) fn control_hot_entries_for_try_state() -> Option<Vec<(ActorId, ActorHotStateOf<T>)>>
    {
      Self::frame_control_entries()?
        .into_iter()
        .map(|(actor_id, location, cell)| {
          let (_, hot, _) = Self::project_control_cell(&cell, location)?;
          Some((actor_id, hot))
        })
        .collect()
    }

    #[cfg(all(test, feature = "runtime-benchmarks"))]
    pub(crate) fn load_control_head(
      actor_id: ActorId,
    ) -> Option<(ActorIdentityOf<T>, ActorHotStateOf<T>)> {
      Self::load_frame_control_authority(actor_id).map(|(_, identity, hot, _)| (identity, hot))
    }

    /// Loads one active semantic owner and its exact canonical process carrier only when the
    /// generation-bound process, concrete residence, and semantic owner agree and no legacy
    /// control authority remains.
    pub(crate) fn load_canonical_actor_semantic_state(
      actor: ActorRef,
    ) -> Result<(ActorSemanticRecordOf<T>, ActorProcessOf<T>), ActorSemanticLoadError> {
      if ActorControlLocators::<T>::contains_key(actor.actor_id)
        || ActorUnsignaledControlCells::<T>::contains_key(actor.actor_id)
      {
        return Err(ActorSemanticLoadError::Corrupt);
      }
      let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor.actor_id)
      else {
        return Err(ActorSemanticLoadError::Corrupt);
      };
      let process = ActorProcesses::<T>::get(actor.actor_id)
        .filter(|process| process.generation == actor.generation)
        .ok_or(ActorSemanticLoadError::Corrupt)?;
      if record.generation != actor.generation {
        return Err(ActorSemanticLoadError::Corrupt);
      }

      let service_node = ServiceNodes::<T>::get(actor.actor_id);
      let deadline_handle = DeadlineHandles::<T>::get(actor.actor_id);
      let pending_owner = PendingCheckOwners::<T>::get(actor.actor_id);
      let deadline_slot_matches = |handle: DeadlineHandleOf<T>| {
        DeadlinePages::<T>::get(handle.key, handle.page).is_some_and(|stored_page| {
          stored_page
            .entries
            .get(usize::from(handle.slot))
            .copied()
            .flatten()
            == Some(actor)
        })
      };
      let carrier_is_coherent = match (process.status, process.residence) {
        (ProcessStatus::Serving, Some(ProcessResidence::Service(kind))) => {
          service_node.is_some_and(|node| node.generation == actor.generation && node.kind == kind)
            && deadline_handle.is_none()
            && pending_owner.is_none()
        }
        (ProcessStatus::Serving, Some(ProcessResidence::Deadline { key, page, slot })) => {
          let expected = DeadlineHandle {
            actor,
            key,
            page,
            slot,
          };
          service_node.is_none()
            && pending_owner.is_none()
            && deadline_handle == Some(expected)
            && deadline_slot_matches(expected)
        }
        (ProcessStatus::Serving, Some(ProcessResidence::Parked(evidence))) => {
          let owner_matches = pending_owner.is_some_and(|owner| {
            owner.actor == actor && evidence.plan_identity == record.admission.admission_identity
          });
          let deadline_matches = match DependencyTimedReviews::<T>::get(actor.actor_id) {
            Some(review) => deadline_handle.is_some_and(|handle| {
              Self::deadline_handle_matches_process(handle, &process)
                && deadline_slot_matches(handle)
                && handle.key == review.deadline
                && pending_owner == Some(review.owner)
                && matches!(handle.key, WakeupKey::Block(block) if evidence.review_at == Some(block))
            }),
            None => deadline_handle.is_none() && evidence.review_at.is_none(),
          };
          service_node.is_none() && owner_matches && deadline_matches
        }
        (ProcessStatus::Disabled(_), None) => {
          service_node.is_none() && deadline_handle.is_none() && pending_owner.is_none()
        }
        (ProcessStatus::Retired(_), None) | (_, _) => false,
      };
      carrier_is_coherent
        .then_some((record, process))
        .ok_or(ActorSemanticLoadError::Corrupt)
    }

    /// Loads one active semantic owner and its exact canonical Service residence only when the
    /// generation-bound process, ring node, and semantic owner agree.
    pub(crate) fn load_service_actor_semantic_state_with_kind(
      actor: ActorRef,
    ) -> Result<(ActorSemanticRecordOf<T>, ServiceResidenceKind), ActorSemanticLoadError> {
      let (record, process) = Self::load_canonical_actor_semantic_state(actor)?;
      let Some(ProcessResidence::Service(kind)) = process.residence else {
        return Err(ActorSemanticLoadError::Corrupt);
      };
      Ok((record, kind))
    }

    pub(crate) fn load_service_actor_semantic_state(
      actor: ActorRef,
      kind: ServiceResidenceKind,
    ) -> Result<ActorSemanticRecordOf<T>, ActorSemanticLoadError> {
      let (record, actual_kind) = Self::load_service_actor_semantic_state_with_kind(actor)?;
      (actual_kind == kind)
        .then_some(record)
        .ok_or(ActorSemanticLoadError::Corrupt)
    }

    /// Atomically commits one retained canonical Service attempt before advancing its ring head.
    /// Any semantic or frontier refusal rolls back both owners and leaves the member retryable.
    pub(crate) fn commit_retained_service_attempt(
      actor: ActorRef,
      kind: ServiceResidenceKind,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      now: BlockNumberFor<T>,
    ) -> Result<(), ServiceRoundError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          if Self::consider_service_head(now)? != ServiceRoundEncounter::Eligible(actor) {
            return Err(ServiceRoundError::CorruptRing);
          }
          Self::try_store_service_control_state(actor, kind, identity, hot)
            .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)?;
          Self::advance_service_head(actor, now)
        })();
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Atomically installs one complete generation-bound parking destination before releasing the
    /// exact canonical Service member. Refusal retains semantic state, Run, attempts, and topology.
    #[allow(
      dead_code,
      reason = "canonical parking remains staged behind the atomic publication cutover"
    )]
    pub(crate) fn transfer_service_member_to_park(
      actor: ActorRef,
      kind: ServiceResidenceKind,
      plan_revision: u64,
      reason: ParkNegativeReason,
      review_at: Option<BlockNumberFor<T>>,
      desired: &[DependencyPlanSource],
      timed_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyPlanMutation, DependencyRegistrationError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let record = Self::load_service_actor_semantic_state(actor, kind)
            .map_err(|_| DependencyRegistrationError::StoredPlanMismatch)?;
          let (state, admission, loaded_step) = Self::load_actor_service_state_with_control(
            actor.actor_id,
            record.identity.clone(),
            record.hot.clone(),
            record.admission.clone(),
          )
          .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          if state.identity != record.identity
            || state.hot != record.hot
            || admission != record.admission
            || loaded_step.is_none()
          {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          let owner = PendingCheckOwner {
            actor,
            plan_revision,
          };
          let deadline_destination = timed_review
            .map(|key| Self::plan_deadline_destination(actor, key))
            .transpose()
            .map_err(|error| match error {
              DeadlineMutationError::CapacityExceeded => {
                DependencyRegistrationError::CapacityExceeded
              }
              _ => DependencyRegistrationError::StoredPlanMismatch,
            })?;
          let evidence = ParkEvidence {
            plan_identity: admission.admission_identity,
            reason,
            review_at,
          };
          if PendingCheckOwners::<T>::contains_key(actor.actor_id)
            || !DependencyPlans::<T>::get(actor.actor_id).is_empty()
            || DependencyTimedReviews::<T>::contains_key(actor.actor_id)
          {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          PendingCheckOwners::<T>::insert(actor.actor_id, owner);
          let mutation = Self::commit_negative_dependency_plan(owner, desired, timed_review)?;
          Self::remove_service_member(actor)
            .map_err(|_| DependencyRegistrationError::StoredPlanMismatch)?;
          let mut process = ActorProcesses::<T>::get(actor.actor_id)
            .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          if process.generation != actor.generation || process.status != ProcessStatus::Serving {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          process.residence = Some(ProcessResidence::Parked(evidence));
          ActorProcesses::<T>::insert(actor.actor_id, process);
          if let Some(destination) = deadline_destination {
            Self::insert_deadline_member(destination).map_err(|error| match error {
              DeadlineMutationError::CapacityExceeded => {
                DependencyRegistrationError::CapacityExceeded
              }
              _ => DependencyRegistrationError::StoredPlanMismatch,
            })?;
          }
          Ok(mutation)
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Consumes one exact negative dependency result only after its complete successor plan is
    /// durable for the same generation/plan-bound Park resident. Refusal preserves the Pending
    /// result, registrations, and Park residence.
    #[allow(
      dead_code,
      reason = "negative dependency continuation remains staged behind the weighted consumer cutover"
    )]
    pub(crate) fn consume_negative_dependency_event_and_rearm(
      expected: PendingDependencyEvent,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      desired: &[DependencyPlanSource],
      timed_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyPlanMutation, DependencyRegistrationError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let process = ActorProcesses::<T>::get(expected.owner.actor.actor_id)
            .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          if process.generation != expected.owner.actor.generation
            || process.status != ProcessStatus::Serving
            || process.residence != Some(ProcessResidence::Parked(evidence))
          {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          Self::consume_pending_dependency_event(expected, desired, timed_review)
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Consumes one exact due-review negative result only after its complete successor plan is
    /// durable for the same generation/plan-bound Park resident. Refusal preserves the Pending
    /// review, registrations, and Park residence.
    #[allow(
      dead_code,
      reason = "negative dependency review continuation remains staged behind the weighted consumer cutover"
    )]
    pub(crate) fn consume_negative_dependency_review_and_rearm(
      expected: DependencyTimedReview<BlockNumberFor<T>>,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      desired: &[DependencyPlanSource],
      timed_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyPlanMutation, DependencyRegistrationError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let process = ActorProcesses::<T>::get(expected.owner.actor.actor_id)
            .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          if process.generation != expected.owner.actor.generation
            || process.status != ProcessStatus::Serving
            || process.residence != Some(ProcessResidence::Parked(evidence))
          {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          Self::consume_pending_dependency_review(expected, desired, timed_review)
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Consumes one exact positive dependency result and wakes its generation/plan-bound Park
    /// resident only after revalidating the current Pending and registration authority.
    #[allow(
      dead_code,
      reason = "positive dependency wake remains staged behind the weighted consumer cutover"
    )]
    pub(crate) fn consume_positive_dependency_event_and_wake(
      expected: PendingDependencyEvent,
      kind: ServiceResidenceKind,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      now: BlockNumberFor<T>,
    ) -> Result<(), DependencyRegistrationError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          match PendingDependencyEvents::<T>::get(expected.owner.actor.actor_id) {
            None => return Err(DependencyRegistrationError::PendingEventMissing),
            Some(current) if current != expected => {
              return Err(DependencyRegistrationError::PendingEventMismatch);
            }
            Some(_) => {}
          }
          if PendingCheckOwners::<T>::get(expected.owner.actor.actor_id) != Some(expected.owner) {
            return Err(DependencyRegistrationError::PendingOwnerMismatch);
          }
          let source_state = DependencyRevisions::<T>::get(expected.source);
          if source_state.exhausted || source_state.revision < expected.revision {
            return Err(DependencyRegistrationError::RevisionMismatch);
          }
          let registration =
            DependencyRegistrations::<T>::get(expected.source, expected.owner.actor.actor_id)
              .ok_or(DependencyRegistrationError::RegistrationMissing)?;
          if registration.actor != expected.owner.actor
            || registration.plan_revision != expected.owner.plan_revision
            || registration.acknowledged_revision < expected.revision
          {
            return Err(DependencyRegistrationError::CurrentRegistrationMismatch);
          }
          PendingDependencyEvents::<T>::remove(expected.owner.actor.actor_id);
          Self::wake_parked_member_to_service(
            expected.owner.actor,
            kind,
            expected.owner,
            evidence,
            now,
          )
        })();
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Consumes one exact positive due review and wakes its generation/plan-bound Park resident
    /// only while every retained source still has the interpreted revision. Refusal preserves the
    /// Pending review, registrations, and Park residence.
    #[allow(
      dead_code,
      reason = "positive dependency review wake remains staged behind the weighted consumer cutover"
    )]
    pub(crate) fn consume_positive_dependency_review_and_wake(
      expected: DependencyTimedReview<BlockNumberFor<T>>,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      observed: &[DependencyPlanSource],
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
    ) -> Result<(), DependencyRegistrationError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          match PendingDependencyReviews::<T>::get(expected.owner.actor.actor_id) {
            None => return Err(DependencyRegistrationError::PendingReviewMissing),
            Some(current) if current != expected => {
              return Err(DependencyRegistrationError::PendingReviewMismatch);
            }
            Some(_) => {}
          }
          let plan = DependencyPlans::<T>::get(expected.owner.actor.actor_id);
          if plan.len() != observed.len() {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          let mut index = 0usize;
          while index < plan.len() {
            let registration = &plan[index];
            let snapshot = &observed[index];
            if registration.source != snapshot.source
              || registration.handle.actor != expected.owner.actor
              || registration.handle.plan_revision != expected.owner.plan_revision
              || registration.handle.acknowledged_revision != snapshot.observed_revision
              || {
                let source_state = DependencyRevisions::<T>::get(snapshot.source);
                source_state.exhausted || source_state.revision != snapshot.observed_revision
              }
            {
              return Err(DependencyRegistrationError::RevisionMismatch);
            }
            index = index
              .checked_add(1)
              .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          }
          PendingDependencyReviews::<T>::remove(expected.owner.actor.actor_id);
          Self::wake_parked_member_to_service(
            expected.owner.actor,
            kind,
            expected.owner,
            evidence,
            now,
          )
        })();
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Interprets one exact Pending due review from one bounded current-source snapshot, then
    /// atomically selects either canonical wake or complete-plan re-arm. Any ambiguous
    /// interpretation or authority race preserves the Pending review and Park residence.
    #[allow(
      dead_code,
      reason = "due-review interpretation remains staged behind the weighted consumer cutover"
    )]
    pub(crate) fn interpret_pending_dependency_review<F>(
      expected: DependencyTimedReview<BlockNumberFor<T>>,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
      next_review: Option<WakeupKey<BlockNumberFor<T>>>,
      interpret: F,
    ) -> Result<DependencyReviewMutation, DependencyRegistrationError>
    where
      F: FnOnce(
        &[DependencyPlanSource],
      ) -> Result<DependencyReviewInterpretation, DependencyRegistrationError>,
    {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          match PendingDependencyReviews::<T>::get(expected.owner.actor.actor_id) {
            None => return Err(DependencyRegistrationError::PendingReviewMissing),
            Some(current) if current != expected => {
              return Err(DependencyRegistrationError::PendingReviewMismatch);
            }
            Some(_) => {}
          }
          if PendingCheckOwners::<T>::get(expected.owner.actor.actor_id) != Some(expected.owner) {
            return Err(DependencyRegistrationError::PendingOwnerMismatch);
          }
          let process = ActorProcesses::<T>::get(expected.owner.actor.actor_id)
            .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          if process.generation != expected.owner.actor.generation
            || process.status != ProcessStatus::Serving
            || process.residence != Some(ProcessResidence::Parked(evidence))
          {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          let plan = DependencyPlans::<T>::get(expected.owner.actor.actor_id);
          let mut observed = BoundedVec::<DependencyPlanSource, T::MaxContractSteps>::default();
          for registration in &plan {
            if registration.handle.actor != expected.owner.actor
              || registration.handle.plan_revision != expected.owner.plan_revision
              || DependencyRegistrations::<T>::get(
                registration.source,
                expected.owner.actor.actor_id,
              ) != Some(registration.handle)
            {
              return Err(DependencyRegistrationError::StoredPlanMismatch);
            }
            let source_state = DependencyRevisions::<T>::get(registration.source);
            if source_state.exhausted {
              return Err(DependencyRegistrationError::SourceExhausted);
            }
            observed
              .try_push(DependencyPlanSource {
                source: registration.source,
                observed_revision: source_state.revision,
              })
              .map_err(|_| DependencyRegistrationError::PlanTooLarge)?;
          }
          match interpret(&observed)? {
            DependencyReviewInterpretation::Positive => {
              Self::consume_positive_dependency_review_and_wake(
                expected, evidence, &observed, kind, now,
              )?;
              Ok(DependencyReviewMutation::Woke)
            }
            DependencyReviewInterpretation::Negative => {
              Self::consume_negative_dependency_review_and_rearm(
                expected,
                evidence,
                &observed,
                next_review,
              )
              .map(DependencyReviewMutation::Rearmed)
            }
          }
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Interprets one exact Pending review whose complete retained plan consists of typed Oracle
    /// availability sources. Every source is read exactly once: all available sources wake the
    /// Actor, any unavailable source re-arms it, and an uninitialized or corrupt mapping preserves
    /// the Pending review and Park residence.
    #[allow(
      dead_code,
      reason = "observation due-review interpretation remains staged behind the weighted consumer cutover"
    )]
    pub(crate) fn interpret_pending_observation_availability_review(
      expected: DependencyTimedReview<BlockNumberFor<T>>,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
      next_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyReviewMutation, DependencyRegistrationError> {
      Self::interpret_pending_dependency_review(
        expected,
        evidence,
        kind,
        now,
        next_review,
        |snapshot| {
          let mut interpretation = DependencyReviewInterpretation::Positive;
          for observed in snapshot {
            let feed = DependencySourceObservations::<T>::get(observed.source)
              .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
            if ObservationDependencySources::<T>::get(feed) != Some(observed.source) {
              return Err(DependencyRegistrationError::StoredPlanMismatch);
            }
            match T::ObservationProvider::current(&feed) {
              crate::CanonicalObservationState::Available { .. } => {}
              crate::CanonicalObservationState::Unavailable => {
                interpretation = DependencyReviewInterpretation::Negative;
              }
              crate::CanonicalObservationState::Uninitialized => {
                return Err(DependencyRegistrationError::SourceUninitialized);
              }
            }
          }
          Ok(interpretation)
        },
      )
    }

    /// Resource-admits and atomically carries one exact due Oracle review from retained deadline
    /// publication through current-state interpretation. Every admitted refusal rolls back the
    /// publication, preserving the timed review and complete Park authority for a later attempt.
    #[allow(
      dead_code,
      reason = "bounded due-review worker remains staged behind deadline traversal cutover"
    )]
    pub(crate) fn process_due_observation_availability_review(
      meter: &mut WeightMeter,
      expected: DependencyTimedReview<BlockNumberFor<T>>,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
      next_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DependencyReviewMutation, DependencyReviewWorkerError> {
      let weight = T::WeightInfo::process_due_observation_availability_review();
      if !meter.can_consume(weight) {
        return Err(DependencyReviewWorkerError::InsufficientWeight);
      }
      meter.consume(weight);
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          Self::publish_due_dependency_review(expected)
            .map_err(DependencyReviewWorkerError::Publication)?;
          Self::interpret_pending_observation_availability_review(
            expected,
            evidence,
            kind,
            now,
            next_review,
          )
          .map_err(DependencyReviewWorkerError::Interpretation)
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Traverses the earliest due block deadline and carries one indexed Oracle review through
    /// its generated complete-attempt owner. Insufficient Weight performs no reads or mutation;
    /// every admitted refusal restores the exact deadline, Pending, Park, and Service topology.
    #[allow(
      dead_code,
      reason = "bounded due-review traversal remains staged behind the mandatory service cutover"
    )]
    pub(crate) fn process_next_due_block_observation_availability_review(
      meter: &mut WeightMeter,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
      next_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<(ActorRef, DependencyReviewMutation), DependencyReviewWorkerError> {
      let weight = T::WeightInfo::process_due_observation_availability_review();
      if !meter.can_consume(weight) {
        return Err(DependencyReviewWorkerError::InsufficientWeight);
      }
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let key = Self::deadline_index_get(WakeupClock::Block, 0).ok_or(
            DependencyReviewWorkerError::Deadline(DeadlineMutationError::MemberMissing),
          )?;
          if !matches!(key, WakeupKey::Block(block) if block <= now) {
            return Err(DependencyReviewWorkerError::Deadline(
              DeadlineMutationError::InvalidDestination,
            ));
          }
          let header = DeadlineHeaders::<T>::get(key).ok_or(
            DependencyReviewWorkerError::Deadline(DeadlineMutationError::CorruptCarrier),
          )?;
          let page = DeadlinePages::<T>::get(key, header.first_page).ok_or(
            DependencyReviewWorkerError::Deadline(DeadlineMutationError::CorruptCarrier),
          )?;
          let actor = page
            .entries
            .iter(/* deos-bypass: bounded-iter -- fixed C32 deadline page. */)
            .find_map(|entry| *entry)
            .ok_or(
            DependencyReviewWorkerError::Deadline(DeadlineMutationError::CorruptCarrier),
          )?;
          let process = ActorProcesses::<T>::get(actor.actor_id).ok_or(
            DependencyReviewWorkerError::Deadline(DeadlineMutationError::ProcessMissing),
          )?;
          let Some(ProcessResidence::Parked(evidence)) = process.residence else {
            return Err(DependencyReviewWorkerError::Deadline(
              DeadlineMutationError::ProcessResidenceMismatch,
            ));
          };
          let expected = DependencyTimedReviews::<T>::get(actor.actor_id).ok_or(
            DependencyReviewWorkerError::Publication(DependencyDueReviewError::ReviewMissing),
          )?;
          if expected.owner.actor != actor || expected.deadline != key {
            return Err(DependencyReviewWorkerError::Publication(
              DependencyDueReviewError::ReviewMismatch,
            ));
          }
          Self::remove_deadline_member(actor).map_err(DependencyReviewWorkerError::Deadline)?;
          let mutation = Self::process_due_observation_availability_review(
            meter,
            expected,
            evidence,
            kind,
            now,
            next_review,
          )?;
          if matches!(mutation, DependencyReviewMutation::Rearmed(_)) {
            let review = DependencyTimedReviews::<T>::get(actor.actor_id).ok_or(
              DependencyReviewWorkerError::Publication(DependencyDueReviewError::ReviewMissing),
            )?;
            let destination = Self::plan_deadline_destination(actor, review.deadline)
              .map_err(DependencyReviewWorkerError::Deadline)?;
            Self::insert_deadline_member(destination)
              .map_err(DependencyReviewWorkerError::Deadline)?;
          }
          Ok((actor, mutation))
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Classifies one member from the shared earliest due block bucket without mutation.
    pub(crate) fn classify_next_due_block_deadline(
      now: BlockNumberFor<T>,
    ) -> Result<DueBlockDeadlineBranch, DeadlineMutationError> {
      let key = Self::deadline_index_get(WakeupClock::Block, 0)
        .ok_or(DeadlineMutationError::MemberMissing)?;
      if !matches!(key, WakeupKey::Block(block) if block <= now) {
        return Err(DeadlineMutationError::InvalidDestination);
      }
      let header = DeadlineHeaders::<T>::get(key).ok_or(DeadlineMutationError::CorruptCarrier)?;
      let page = DeadlinePages::<T>::get(key, header.first_page)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      let actor = page
        .entries
        .iter(/* deos-bypass: bounded-iter -- fixed C32 deadline page. */)
        .find_map(|entry| *entry)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      let process =
        ActorProcesses::<T>::get(actor.actor_id).ok_or(DeadlineMutationError::ProcessMissing)?;
      match process.residence {
        Some(ProcessResidence::Deadline { .. }) => Ok(DueBlockDeadlineBranch::Retry(actor)),
        Some(ProcessResidence::Parked(_)) => Ok(DueBlockDeadlineBranch::Review(actor)),
        _ => Err(DeadlineMutationError::ProcessResidenceMismatch),
      }
    }

    /// Classifies and processes one member from the shared earliest due block bucket. Sleeping
    /// retries return directly to Service; Parked members alone enter the timed-review worker.
    /// Classification and the selected complete branch each have an independent generated Weight
    /// owner, so refusal cannot inspect state or consume a member under the wrong branch envelope.
    #[allow(
      dead_code,
      reason = "mixed deadline traversal remains staged behind the mandatory service cutover"
    )]
    pub(crate) fn process_next_due_block_deadline(
      meter: &mut WeightMeter,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
      next_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DueBlockDeadlineMutation, DependencyReviewWorkerError> {
      let selector_weight = T::WeightInfo::classify_due_block_deadline();
      if !meter.can_consume(selector_weight) {
        return Err(DependencyReviewWorkerError::InsufficientWeight);
      }
      meter.consume(selector_weight);
      match Self::classify_next_due_block_deadline(now)
        .map_err(DependencyReviewWorkerError::Deadline)?
      {
        DueBlockDeadlineBranch::Retry(actor) => {
          let branch_weight = T::WeightInfo::return_due_block_deadline_to_service();
          if !meter.can_consume(branch_weight) {
            return Err(DependencyReviewWorkerError::InsufficientWeight);
          }
          Self::return_due_deadline_member_to_service(actor, kind, now)
            .map_err(DependencyReviewWorkerError::Deadline)?;
          meter.consume(branch_weight);
          Ok(DueBlockDeadlineMutation::RetryReturned(actor))
        }
        DueBlockDeadlineBranch::Review(_) => {
          Self::process_next_due_block_observation_availability_review(
            meter,
            kind,
            now,
            next_review,
          )
          .map(|(actor, mutation)| DueBlockDeadlineMutation::ReviewProcessed(actor, mutation))
        }
        DueBlockDeadlineBranch::TemporalTrigger(_) => Err(DependencyReviewWorkerError::Deadline(
          DeadlineMutationError::InvalidDestination,
        )),
      }
    }

    /// Classifies one member from the shared earliest due tick bucket without mutation. Tick
    /// deadlines currently retain timed Park reviews only; a sleeping retry on this clock is an
    /// incoherent carrier state and is never consumed by the dispatcher.
    pub(crate) fn classify_next_due_tick_deadline(
      now_tick: SchedulerTick,
    ) -> Result<DueBlockDeadlineBranch, DeadlineMutationError> {
      let key = Self::deadline_index_get(WakeupClock::Tick, 0)
        .ok_or(DeadlineMutationError::MemberMissing)?;
      if !matches!(key, WakeupKey::Tick(tick) if tick <= now_tick) {
        return Err(DeadlineMutationError::InvalidDestination);
      }
      let header = DeadlineHeaders::<T>::get(key).ok_or(DeadlineMutationError::CorruptCarrier)?;
      let page = DeadlinePages::<T>::get(key, header.first_page)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      let actor = page
        .entries
        .iter(/* deos-bypass: bounded-iter -- fixed C32 deadline page. */)
        .find_map(|entry| *entry)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      if TriggerDeadlineHandles::<T>::get(actor.actor_id)
        .is_some_and(|handle| handle.actor == actor && handle.key == key)
      {
        return Ok(DueBlockDeadlineBranch::TemporalTrigger(actor));
      }
      let process =
        ActorProcesses::<T>::get(actor.actor_id).ok_or(DeadlineMutationError::ProcessMissing)?;
      match process.residence {
        Some(ProcessResidence::Deadline { .. }) => Ok(DueBlockDeadlineBranch::Retry(actor)),
        Some(ProcessResidence::Parked(_)) => Ok(DueBlockDeadlineBranch::Review(actor)),
        _ => Err(DeadlineMutationError::ProcessResidenceMismatch),
      }
    }

    /// Processes one retained timed review or temporal Trigger from the independent Tick frontier. Selection is
    /// admitted before inspection; the existing complete review owner admits the selected branch.
    /// Block-clock members and incoherent Tick retries remain untouched.
    pub(crate) fn process_next_due_tick_deadline(
      meter: &mut WeightMeter,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
      now_tick: SchedulerTick,
      next_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DueTickDeadlineMutation, DependencyReviewWorkerError> {
      let selector_weight = T::WeightInfo::classify_due_tick_deadline();
      if !meter.can_consume(selector_weight) {
        return Err(DependencyReviewWorkerError::InsufficientWeight);
      }
      meter.consume(selector_weight);
      let branch = Self::classify_next_due_tick_deadline(now_tick)
        .map_err(DependencyReviewWorkerError::Deadline)?;
      if let DueBlockDeadlineBranch::TemporalTrigger(actor) = branch {
        let weight = T::WeightInfo::at_time_trigger_occurrence()
          .max(T::WeightInfo::cadenced_trigger_occurrence());
        if !meter.can_consume(weight) {
          return Err(DependencyReviewWorkerError::InsufficientWeight);
        }
        meter.consume(weight);
        let result = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
          let result = (|| {
            let handle = TriggerDeadlineHandles::<T>::get(actor.actor_id).ok_or(
              DependencyReviewWorkerError::Deadline(DeadlineMutationError::MemberMissing),
            )?;
            if handle.actor != actor
              || !matches!(handle.key, WakeupKey::Tick(tick) if tick <= now_tick)
            {
              return Err(DependencyReviewWorkerError::Deadline(
                DeadlineMutationError::ProcessResidenceMismatch,
              ));
            }
            let Some(ActorSemanticState::Active(mut semantic)) =
              ActorSemanticStates::<T>::get(actor.actor_id)
            else {
              return Err(DependencyReviewWorkerError::Deadline(
                DeadlineMutationError::ProcessMissing,
              ));
            };
            if semantic.generation != actor.generation {
              return Err(DependencyReviewWorkerError::Deadline(
                DeadlineMutationError::StaleGeneration,
              ));
            }
            Self::remove_trigger_deadline_member(actor)
              .map_err(DependencyReviewWorkerError::Deadline)?;
            semantic.hot.trigger_wakeup_pointer = None;
            ActorSemanticStates::<T>::insert(
              actor.actor_id,
              ActorSemanticState::Active(semantic.clone()),
            );
            let (state, admission, loaded_step) = Self::load_actor_service_state_with_control(
              actor.actor_id,
              semantic.identity,
              semantic.hot,
              semantic.admission,
            )
            .ok_or(DependencyReviewWorkerError::TemporalOccurrence)?;
            Self::process_due_temporal_occurrence_loaded(
              actor.actor_id,
              state,
              admission,
              loaded_step,
              now_tick,
            )
            .map_err(|_| DependencyReviewWorkerError::TemporalOccurrence)?;
            Ok(DueTickDeadlineMutation::TemporalTriggerProcessed(actor))
          })();
          match result {
            Ok(mutation) => {
              polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
            }
            Err(error) => {
              polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
            }
          }
        });
        return result;
      }
      let DueBlockDeadlineBranch::Review(actor) = branch else {
        return Err(DependencyReviewWorkerError::Deadline(
          DeadlineMutationError::ProcessResidenceMismatch,
        ));
      };
      let weight = T::WeightInfo::process_due_observation_availability_review();
      if !meter.can_consume(weight) {
        return Err(DependencyReviewWorkerError::InsufficientWeight);
      }
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let expected = DependencyTimedReviews::<T>::get(actor.actor_id).ok_or(
            DependencyReviewWorkerError::Publication(DependencyDueReviewError::ReviewMissing),
          )?;
          let handle = DeadlineHandles::<T>::get(actor.actor_id).ok_or(
            DependencyReviewWorkerError::Deadline(DeadlineMutationError::MemberMissing),
          )?;
          if expected.owner.actor != actor
            || handle.actor != actor
            || expected.deadline != handle.key
            || !matches!(expected.deadline, WakeupKey::Tick(tick) if tick <= now_tick)
          {
            return Err(DependencyReviewWorkerError::Publication(
              DependencyDueReviewError::ReviewMismatch,
            ));
          }
          let process = ActorProcesses::<T>::get(actor.actor_id).ok_or(
            DependencyReviewWorkerError::Deadline(DeadlineMutationError::ProcessMissing),
          )?;
          let Some(ProcessResidence::Parked(evidence)) = process.residence else {
            return Err(DependencyReviewWorkerError::Deadline(
              DeadlineMutationError::ProcessResidenceMismatch,
            ));
          };
          Self::remove_deadline_member(actor).map_err(DependencyReviewWorkerError::Deadline)?;
          let mutation = Self::process_due_observation_availability_review(
            meter,
            expected,
            evidence,
            kind,
            now,
            next_review,
          )?;
          if matches!(mutation, DependencyReviewMutation::Rearmed(_)) {
            let review = DependencyTimedReviews::<T>::get(actor.actor_id).ok_or(
              DependencyReviewWorkerError::Publication(DependencyDueReviewError::ReviewMissing),
            )?;
            let destination = Self::plan_deadline_destination(actor, review.deadline)
              .map_err(DependencyReviewWorkerError::Deadline)?;
            Self::insert_deadline_member(destination)
              .map_err(DependencyReviewWorkerError::Deadline)?;
          }
          Ok(DueTickDeadlineMutation::ReviewProcessed(actor, mutation))
        })();
        match result {
          Ok(mutation) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(mutation))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Gives each independent deadline clock one bounded mandatory-service attempt in fixed
    /// Block-then-Tick order. The complete maximum two-frontier envelope is admitted before either
    /// selector reads storage, so an absent, refused, or continuously busy Block frontier cannot
    /// consume the Tick frontier's authority (and vice versa). Each branch still settles only its
    /// actual generated selector and worker Weight through the shared meter.
    pub(crate) fn service_due_deadline_frontiers(
      meter: &mut WeightMeter,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
      now_tick: SchedulerTick,
      next_block_review: Option<WakeupKey<BlockNumberFor<T>>>,
      next_tick_review: Option<WakeupKey<BlockNumberFor<T>>>,
    ) -> Result<DueDeadlineServicePass, DependencyReviewWorkerError> {
      let review = T::WeightInfo::process_due_observation_availability_review();
      let temporal = T::WeightInfo::at_time_trigger_occurrence()
        .max(T::WeightInfo::cadenced_trigger_occurrence());
      let block_branch = T::WeightInfo::return_due_block_deadline_to_service().max(review);
      let complete_envelope = T::WeightInfo::classify_due_block_deadline()
        .saturating_add(block_branch)
        .saturating_add(T::WeightInfo::classify_due_tick_deadline())
        .saturating_add(review.max(temporal));
      if !meter.can_consume(complete_envelope) {
        return Err(DependencyReviewWorkerError::InsufficientWeight);
      }
      let block = Self::process_next_due_block_deadline(meter, kind, now, next_block_review);
      let tick = Self::process_next_due_tick_deadline(meter, kind, now, now_tick, next_tick_review);
      Ok(DueDeadlineServicePass { block, tick })
    }

    /// Opens one canonical Service round and executes an eligible zero-Step or successful
    /// effectful head under one pre-admitted selector/execution envelope. Effectful failure and
    /// retry placement remain captured for the later complete deadline/resource suffix; they are
    /// not marked attempted or advanced. Weight, fee, state, or invariant refusal leaves the prior
    /// round, member, process, and cursor untouched.
    pub(crate) fn service_canonical_round_head(
      meter: &mut WeightMeter,
      now: BlockNumberFor<T>,
    ) -> Result<ServiceRoundEncounter, ServiceRoundError> {
      Self::service_canonical_round_head_inner(
        meter,
        now,
        None,
        BlockResourceDomain::ActorDrainEffect,
        false,
      )
      .map(|(encounter, _)| encounter)
    }

    pub(crate) fn service_canonical_round_head_with_resources(
      meter: &mut WeightMeter,
      now: BlockNumberFor<T>,
      state: &mut BlockResourceState<BlockNumberFor<T>>,
      limits: BlockResourceLimits,
      effect_domain: BlockResourceDomain,
    ) -> Result<ServiceRoundEncounter, ServiceRoundError> {
      Self::service_canonical_round_head_inner(
        meter,
        now,
        Some((state, limits)),
        effect_domain,
        false,
      )
      .map(|(encounter, _)| encounter)
    }

    /// Variant for callers already holding the pass-wide `ActorControl` reservation. Nested step
    /// control is accounted by that outer reservation, so this seam reserves only effect capacity
    /// and leaves control settlement to the enclosing `CyclePass` reconciliation.
    pub(crate) fn service_canonical_round_head_with_reserved_control(
      meter: &mut WeightMeter,
      now: BlockNumberFor<T>,
      state: &mut BlockResourceState<BlockNumberFor<T>>,
      limits: BlockResourceLimits,
      effect_domain: BlockResourceDomain,
    ) -> Result<ServiceRoundEncounter, ServiceRoundError> {
      Self::service_canonical_round_head_inner(
        meter,
        now,
        Some((state, limits)),
        effect_domain,
        true,
      )
      .map(|(encounter, _)| encounter)
    }

    pub(crate) fn service_canonical_round_head_inner(
      meter: &mut WeightMeter,
      now: BlockNumberFor<T>,
      mut resource_authority: Option<(
        &mut BlockResourceState<BlockNumberFor<T>>,
        BlockResourceLimits,
      )>,
      effect_domain: BlockResourceDomain,
      control_owned_by_caller: bool,
    ) -> Result<
      (
        ServiceRoundEncounter,
        Option<crate::scheduler::ActorAttemptEvidence>,
      ),
      ServiceRoundError,
    > {
      let resource_before = resource_authority.as_ref().map(|(state, _)| **state);
      let selector_envelope = T::WeightInfo::service_round_begin_populated()
        .saturating_add(T::WeightInfo::service_round_probe_eligible());
      let zero_step_envelope = T::WeightInfo::scheduler_inner_zero_step_complete().saturating_add(
        T::WeightInfo::service_round_admit_eligible().max(
          T::WeightInfo::service_member_retire_interior()
            .max(T::WeightInfo::service_member_retire_pair_cursor())
            .max(T::WeightInfo::service_member_retire_singleton()),
        ),
      );
      let complete_envelope = selector_envelope.saturating_add(zero_step_envelope);
      if !meter.can_consume(complete_envelope) {
        return Err(ServiceRoundError::InsufficientWeight);
      }
      let result = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          Self::begin_service_round(now)?;
          let mut encounter = Self::consider_service_head(now)?;
          let mut execution_weight = Weight::zero();
          let mut attempt = None;
          if let ServiceRoundEncounter::Eligible(actor) = encounter {
            let (semantic, service_kind) = Self::load_service_actor_semantic_state_with_kind(actor)
              .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)?;
            let (state, admission, loaded_step) = Self::load_actor_service_state_with_control(
              actor.actor_id,
              semantic.identity,
              semantic.hot,
              semantic.admission,
            )
            .ok_or(ServiceRoundError::ProcessResidenceMismatch)?;
            let instance = Self::derive_active_actor_view(
              state.identity.clone(),
              state.hot.clone(),
              state.contract.clone(),
            );
            // The global circuit breaker defers all ordinary Step effects and automatic terminal
            // close while retaining exact placement, mirroring the legacy admission decision which
            // returns `Skip` before terminal or capacity classification. Only explicit lifecycle and
            // bounded sweep cleanup may still act while the breaker is active.
            let classification = Self::classify_actor_loaded(&instance, state.run_state.as_ref())
              .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)?;
            if classification.execution_phase
              == crate::types::ActorExecutionPhase::GlobalCircuitBreaker
            {
              return Ok((ServiceRoundEncounter::BreakerRefused(actor), Weight::zero(), None));
            }
            // A due schedule window or an exhausted cycle nonce is terminal before any Step
            // attempt. The canonical service round must own that decision: an Idle resident has
            // no Step to attempt, so without this branch a window-expiry deadline returned to
            // Service would only advance the ring cursor and leave the Actor active past its
            // window. Close through the same atomic owner used by authored entry points.
            let terminal_reason = classification.terminal_reason;
            // A latched User Idle opening must own the same admission-time insolvency decision as
            // the legacy admission path: when the sovereign balance cannot cover the ledger floor
            // plus the Pipeline Machine fee, close with `CycleAdmissionInsufficient` instead of
            // attempting an effectful or zero-Step cycle. Without this the canonical round defers
            // forever behind the protected floor. Terminal reasons take precedence; other actor
            // classes and non-Idle states keep unlimited System admission.
            let admission_insufficient = if terminal_reason.is_none()
              && state.identity.actor_class.actor_type() == ActorType::User
              && state.hot.cycle_state == CycleState::Idle
              && state.hot.pending_signal
            {
              match Self::pipeline_capacity_sufficient(
                actor.actor_id,
                ActorType::User,
                &instance.sovereign_account,
              ) {
                Ok(sufficient) => !sufficient,
                Err(_) => return Err(ServiceRoundError::ProcessResidenceMismatch),
              }
            } else {
              false
            };
            // A User retry continuation that can no longer cover its current Action's maximum fee
            // plus the protected ledger floor must terminate through the same custody-neutral
            // `CycleAdmissionInsufficient` close the legacy admission decision applies, instead of
            // re-suspending or re-attempting forever behind the floor. System Actors and non-
            // Suspended states retain their unbounded admission.
            let retry_action_insufficient = if terminal_reason.is_none()
              && state.identity.actor_class.actor_type() == ActorType::User
              && state.hot.cycle_state == CycleState::Suspended
            {
              match loaded_step.as_ref() {
                Some(loaded_step) => match Self::action_capacity_sufficient(
                  ActorType::User,
                  &instance.sovereign_account,
                  &loaded_step.step,
                  loaded_step.resources,
                ) {
                  Ok(sufficient) => !sufficient,
                  Err(_) => return Err(ServiceRoundError::ProcessResidenceMismatch),
                },
                None => false,
              }
            } else {
              false
            };
            let close_reason = terminal_reason
              .or_else(|| {
                admission_insufficient.then_some(CloseReason::CycleAdmissionInsufficient)
              })
              .or_else(|| {
                retry_action_insufficient.then_some(CloseReason::CycleAdmissionInsufficient)
              });
            let idle_no_work = state.hot.cycle_state == CycleState::Idle
              && !state.hot.pending_signal
              && state.run_state.is_none();
            if let Some(reason) = close_reason {
              let close_envelope = selector_envelope
                .saturating_add(Self::close_dispatch_weight_upper());
              let mut reservation =
                if let Some((resource_state, limits)) = resource_authority.as_mut() {
                  Some(
                    resource_state
                      .reserve(
                        *limits,
                        BlockResourceDomain::ActorControl,
                        if control_owned_by_caller {
                          Weight::zero()
                        } else {
                          close_envelope
                        },
                      )
                      .map_err(|_| ServiceRoundError::ResourceUnavailable)?,
                  )
                } else {
                  None
                };
              if !meter.can_consume(selector_envelope.saturating_add(close_envelope)) {
                return Err(ServiceRoundError::InsufficientWeight);
              }
              Self::finalize_actor(actor.actor_id, &instance, reason)
                .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)?;
              if let (Some((resource_state, _)), Some(reservation)) =
                (resource_authority.as_mut(), reservation.as_mut())
              {
                resource_state
                  .settle(
                    reservation,
                    if control_owned_by_caller {
                      Weight::zero()
                    } else {
                      close_envelope
                    },
                  )
                  .map_err(|_| ServiceRoundError::ResourceUnavailable)?;
              }
              execution_weight = close_envelope;
              encounter = ServiceRoundEncounter::TerminallyClosed(reason);
            } else if idle_no_work {
              // A completed or aborted member is retained in the ring as an Idle resident. A later
              // round revisits it with no admitted work; it must advance the bounded cursor without
              // opening a pipeline, executing a Step, or recording an attempt.
              let mut reservation =
                if let Some((resource_state, limits)) = resource_authority.as_mut() {
                  Some(
                    resource_state
                      .reserve(
                        *limits,
                        BlockResourceDomain::ActorControl,
                        if control_owned_by_caller {
                          Weight::zero()
                        } else {
                          selector_envelope
                        },
                      )
                      .map_err(|_| ServiceRoundError::ResourceUnavailable)?,
                  )
                } else {
                  None
                };
              Self::advance_idle_service_head(actor, now)
                .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)?;
              if let (Some((resource_state, _)), Some(reservation)) =
                (resource_authority.as_mut(), reservation.as_mut())
              {
                resource_state
                  .settle(
                    reservation,
                    if control_owned_by_caller {
                      Weight::zero()
                    } else {
                      selector_envelope
                    },
                  )
                  .map_err(|_| ServiceRoundError::ResourceUnavailable)?;
              }
              execution_weight = Weight::zero();
              encounter = ServiceRoundEncounter::NoWork(actor);
            } else if let Some(loaded_step) = loaded_step {
              let resources = loaded_step.resources;
              let effectful_envelope = resources
                .control
                .saturating_add(resources.effect)
                .saturating_add(
                  T::WeightInfo::service_round_admit_eligible().max(
                    T::WeightInfo::service_member_retire_interior()
                      .max(T::WeightInfo::service_member_retire_pair_cursor())
                      .max(T::WeightInfo::service_member_retire_singleton()),
                  ),
                );
              if !meter.can_consume(selector_envelope.saturating_add(effectful_envelope)) {
                return Err(ServiceRoundError::InsufficientWeight);
              }
              let maximum_fee = Self::maximum_current_action_fee(
                state.identity.actor_class.actor_type(),
                &loaded_step.step,
                resources,
              )
              .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)?;
              let retry_deadline =
                if let StepErrorPolicy::RetryLater { max_attempts } = loaded_step.step.on_error {
                  let attempted = state.run_state.as_ref().map_or(1, |run| {
                    run.unsuccessful_attempts_at_cursor.saturating_add(1)
                  });
                  if attempted < max_attempts {
                    let eligible_at = Self::suspension_eligible_at(
                      state.contract.cooldown_blocks,
                      state.contract.window,
                      now,
                      attempted,
                    )
                    .map_err(|_| ServiceRoundError::ProcessResidenceMismatch)?;
                    (now.checked_add(&One::one()) != Some(eligible_at))
                      .then_some(WakeupKey::Block(eligible_at))
                  } else {
                    None
                  }
                } else {
                  None
                };
              let plan = Self::build_canonical_current_step_plan(
                actor.actor_id,
                state.identity.clone(),
                state.hot.clone(),
                state.run_state.clone(),
                admission.clone(),
                loaded_step,
                maximum_fee,
              )
              .ok_or(ServiceRoundError::ProcessResidenceMismatch)?;
              let suffix = T::WeightInfo::service_round_admit_eligible().max(
                T::WeightInfo::service_member_retire_interior()
                  .max(T::WeightInfo::service_member_retire_pair_cursor())
                  .max(T::WeightInfo::service_member_retire_singleton()),
              );
              let mut reservation =
                if let Some((resource_state, limits)) = resource_authority.as_mut() {
                  Some(
                    resource_state
                      .reserve_actor_step(
                        *limits,
                        effect_domain,
                        if control_owned_by_caller {
                          Weight::zero()
                        } else {
                          selector_envelope
                            .saturating_add(resources.control)
                            .saturating_add(suffix)
                        },
                        resources.effect,
                      )
                      .map_err(|_| ServiceRoundError::ResourceUnavailable)?,
                  )
                } else {
                  None
                };
              let evidence = Self::execute_effectful_step_on_service_with_deadline(
                actor,
                service_kind,
                state,
                plan,
                &admission,
                now,
                retry_deadline,
              )
              .map_err(|error| match error {
                crate::scheduler::AttemptTransactionError::FeeCollection => {
                  ServiceRoundError::FeeCollection
                }
                _ => ServiceRoundError::ProcessResidenceMismatch,
              })?;
              attempt = Some(evidence.attempt);
              let actual_control = if control_owned_by_caller {
                Weight::zero()
              } else {
                selector_envelope
                  .saturating_add(evidence.actual_control_weight)
                  .saturating_add(suffix)
              };
              if let (Some((resource_state, _)), Some(reservation)) =
                (resource_authority.as_mut(), reservation.as_mut())
              {
                resource_state
                  .settle_actor_step(reservation, actual_control, evidence.actual_effect_weight)
                  .map_err(|_| ServiceRoundError::ResourceUnavailable)?;
              }
              execution_weight = actual_control
                .saturating_sub(selector_envelope)
                .saturating_add(evidence.actual_effect_weight);
            } else {
              let mut reservation =
                if let Some((resource_state, limits)) = resource_authority.as_mut() {
                  Some(
                    resource_state
                      .reserve(
                        *limits,
                        BlockResourceDomain::ActorControl,
                        if control_owned_by_caller {
                          Weight::zero()
                        } else {
                          selector_envelope.saturating_add(zero_step_envelope)
                        },
                      )
                      .map_err(|_| ServiceRoundError::ResourceUnavailable)?,
                  )
                } else {
                  None
                };
              attempt = Some(
                Self::execute_zero_step_on_service(
                  actor,
                  service_kind,
                  state,
                  &admission,
                  now,
                  None,
                )
                .map_err(|error| match error {
                  crate::scheduler::AttemptTransactionError::FeeCollection => {
                    ServiceRoundError::FeeCollection
                  }
                  _ => ServiceRoundError::ProcessResidenceMismatch,
                })?,
              );
              if let (Some((resource_state, _)), Some(reservation)) =
                (resource_authority.as_mut(), reservation.as_mut())
              {
                resource_state
                  .settle(
                    reservation,
                    if control_owned_by_caller {
                      Weight::zero()
                    } else {
                      selector_envelope.saturating_add(zero_step_envelope)
                    },
                  )
                  .map_err(|_| ServiceRoundError::ResourceUnavailable)?;
              }
              execution_weight = zero_step_envelope;
            }
          } else if let Some((resource_state, limits)) = resource_authority.as_mut() {
            let mut reservation = resource_state
              .reserve(
                *limits,
                BlockResourceDomain::ActorControl,
                if control_owned_by_caller {
                  Weight::zero()
                } else {
                  selector_envelope
                },
              )
              .map_err(|_| ServiceRoundError::ResourceUnavailable)?;
            resource_state
              .settle(
                &mut reservation,
                if control_owned_by_caller {
                  Weight::zero()
                } else {
                  selector_envelope
                },
              )
              .map_err(|_| ServiceRoundError::ResourceUnavailable)?;
          }
          Ok((encounter, execution_weight, attempt))
        })();
        match result {
          Ok(outcome) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(outcome))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      });
      let (encounter, execution_weight, attempt) = match result {
        Ok(outcome) => outcome,
        Err(error) => {
          if let (Some((state, _)), Some(before)) = (resource_authority, resource_before) {
            *state = before;
          }
          return Err(error);
        }
      };
      meter.consume(selector_envelope);
      meter.consume(execution_weight);
      Ok((encounter, attempt))
    }

    /// Atomically wakes one exact generation/plan-bound Park resident into canonical Service.
    /// Stale authority and occupied Pending work refuse without consuming the retained plan.
    pub(crate) fn wake_parked_member_to_service(
      actor: ActorRef,
      kind: ServiceResidenceKind,
      owner: PendingCheckOwner,
      evidence: ParkEvidence<BlockNumberFor<T>>,
      now: BlockNumberFor<T>,
    ) -> Result<(), DependencyRegistrationError> {
      if owner.actor != actor {
        return Err(DependencyRegistrationError::PendingOwnerMismatch);
      }
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let mut process = ActorProcesses::<T>::get(actor.actor_id)
            .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          if process.generation != actor.generation
            || process.status != ProcessStatus::Serving
            || process.residence != Some(ProcessResidence::Parked(evidence))
          {
            return Err(DependencyRegistrationError::StoredPlanMismatch);
          }
          if PendingCheckOwners::<T>::get(actor.actor_id) != Some(owner)
            || PendingDependencyEvents::<T>::contains_key(actor.actor_id)
            || PendingDependencyReviews::<T>::contains_key(actor.actor_id)
          {
            return Err(DependencyRegistrationError::PendingOwnerMismatch);
          }
          let plan = DependencyPlans::<T>::get(actor.actor_id);
          for registration in &plan {
            if registration.handle.actor != actor
              || registration.handle.plan_revision != owner.plan_revision
              || DependencyRegistrations::<T>::get(registration.source, actor.actor_id)
                != Some(registration.handle)
            {
              return Err(DependencyRegistrationError::StoredPlanMismatch);
            }
            Self::dependency_registration_position(
              registration.source,
              actor.actor_id,
              registration.handle,
            )?;
          }
          if let Some(review) = DependencyTimedReviews::<T>::get(actor.actor_id) {
            if review.owner != owner {
              return Err(DependencyRegistrationError::StoredPlanMismatch);
            }
            DependencyTimedReviews::<T>::remove(actor.actor_id);
          }
          for registration in &plan {
            Self::remove_dependency_registration(registration.source, registration.handle)?;
          }
          DependencyPlans::<T>::remove(actor.actor_id);
          PendingCheckOwners::<T>::remove(actor.actor_id);
          process.residence = Some(ProcessResidence::Service(kind));
          ActorProcesses::<T>::insert(actor.actor_id, process);
          let admission_round = now
            .checked_sub(&One::one())
            .ok_or(DependencyRegistrationError::StoredPlanMismatch)?;
          Self::insert_service_member(actor, kind, admission_round)
            .map_err(|_| DependencyRegistrationError::StoredPlanMismatch)
        })();
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Selects the first canonical free slot in one deadline bucket without mutation.
    pub(crate) fn plan_deadline_destination(
      actor: ActorRef,
      key: WakeupKey<BlockNumberFor<T>>,
    ) -> Result<DeadlineHandleOf<T>, DeadlineMutationError> {
      let Some(header) = DeadlineHeaders::<T>::get(key) else {
        return Ok(DeadlineHandle {
          actor,
          key,
          page: 0,
          slot: 0,
        });
      };
      let mut page_id = header.first_page;
      for visited in 0..header.page_count {
        let page =
          DeadlinePages::<T>::get(key, page_id).ok_or(DeadlineMutationError::CorruptCarrier)?;
        if page.entries.len() != 32 || page.live_entries > 32 {
          return Err(DeadlineMutationError::CorruptCarrier);
        }
        let mut slot = 0usize;
        while slot < 32 {
          if page.entries[slot].is_none() {
            return Ok(DeadlineHandle {
              actor,
              key,
              page: page_id,
              slot: u8::try_from(slot).map_err(|_| DeadlineMutationError::CorruptCarrier)?,
            });
          }
          slot = slot
            .checked_add(1)
            .ok_or(DeadlineMutationError::CorruptCarrier)?;
        }
        if visited + 1 == header.page_count {
          if page_id != header.last_page || page.next_page.is_some() {
            return Err(DeadlineMutationError::CorruptCarrier);
          }
          return Ok(DeadlineHandle {
            actor,
            key,
            page: header.next_page,
            slot: 0,
          });
        }
        page_id = page
          .next_page
          .ok_or(DeadlineMutationError::CorruptCarrier)?;
      }
      Err(DeadlineMutationError::CorruptCarrier)
    }

    /// Atomically transfers one exact canonical Service member into a preselected deadline slot.
    /// The caller must commit semantic retry state first in the same outer transaction.
    pub(crate) fn transfer_service_member_to_deadline(
      actor: ActorRef,
      destination: DeadlineHandleOf<T>,
    ) -> Result<(), DeadlineMutationError> {
      if destination.actor != actor {
        return Err(DeadlineMutationError::StaleGeneration);
      }
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          Self::remove_service_member(actor)
            .map_err(|_| DeadlineMutationError::ProcessResidenceMismatch)?;
          let mut process = ActorProcesses::<T>::get(actor.actor_id)
            .ok_or(DeadlineMutationError::ProcessMissing)?;
          if process.generation != actor.generation || process.status != ProcessStatus::Serving {
            return Err(DeadlineMutationError::StaleGeneration);
          }
          process.residence = Some(ProcessResidence::Deadline {
            key: destination.key,
            page: destination.page,
            slot: destination.slot,
          });
          ActorProcesses::<T>::insert(actor.actor_id, process);
          Self::insert_deadline_member(destination)
        })();
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Returns one genuinely due deadline member to canonical Service exactly once.
    #[allow(
      dead_code,
      reason = "canonical deadline extraction remains staged behind the atomic service cutover"
    )]
    pub(crate) fn return_due_deadline_member_to_service(
      actor: ActorRef,
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
    ) -> Result<(), DeadlineMutationError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        let result = (|| {
          let handle = DeadlineHandles::<T>::get(actor.actor_id)
            .filter(|handle| handle.actor == actor)
            .ok_or(DeadlineMutationError::MemberMissing)?;
          let due = matches!(handle.key, WakeupKey::Block(block) if block <= now);
          if !due {
            return Err(DeadlineMutationError::InvalidDestination);
          }
          let process = ActorProcesses::<T>::get(actor.actor_id)
            .ok_or(DeadlineMutationError::ProcessMissing)?;
          if process.residence
            != Some(ProcessResidence::Deadline {
              key: handle.key,
              page: handle.page,
              slot: handle.slot,
            })
          {
            return Err(DeadlineMutationError::ProcessResidenceMismatch);
          }
          Self::remove_deadline_member(actor)?;
          let mut process = ActorProcesses::<T>::get(actor.actor_id)
            .ok_or(DeadlineMutationError::ProcessMissing)?;
          process.residence = Some(ProcessResidence::Service(kind));
          ActorProcesses::<T>::insert(actor.actor_id, process);
          let admission_round = now
            .checked_sub(&One::one())
            .ok_or(DeadlineMutationError::InvalidDestination)?;
          Self::insert_service_member(actor, kind, admission_round)
            .map_err(|_| DeadlineMutationError::ProcessResidenceMismatch)
        })();
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    /// Extracts the canonical head member of the earliest due block bucket.
    #[allow(
      dead_code,
      reason = "canonical deadline traversal remains staged behind the atomic service cutover"
    )]
    pub(crate) fn return_next_due_block_deadline_to_service(
      kind: ServiceResidenceKind,
      now: BlockNumberFor<T>,
    ) -> Result<ActorRef, DeadlineMutationError> {
      let key = Self::deadline_index_get(WakeupClock::Block, 0)
        .ok_or(DeadlineMutationError::MemberMissing)?;
      if !matches!(key, WakeupKey::Block(block) if block <= now) {
        return Err(DeadlineMutationError::InvalidDestination);
      }
      let header = DeadlineHeaders::<T>::get(key).ok_or(DeadlineMutationError::CorruptCarrier)?;
      let page = DeadlinePages::<T>::get(key, header.first_page)
        .ok_or(DeadlineMutationError::CorruptCarrier)?;
      let mut slot = 0usize;
      let actor = loop {
        if slot >= 32 {
          return Err(DeadlineMutationError::CorruptCarrier);
        }
        if let Some(actor) = page.entries[slot] {
          break actor;
        }
        slot = slot
          .checked_add(1)
          .ok_or(DeadlineMutationError::CorruptCarrier)?;
      };
      Self::return_due_deadline_member_to_service(actor, kind, now)?;
      Ok(actor)
    }

    /// Proves the complete Service-to-deadline destination without retaining any mutation.
    pub(crate) fn probe_service_member_to_deadline(
      actor: ActorRef,
      destination: DeadlineHandleOf<T>,
    ) -> Result<(), DeadlineMutationError> {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(
          Self::transfer_service_member_to_deadline(actor, destination),
        )
      })
    }

    /// Stores identity and Hot directly through the canonical semantic owner. Canonical service
    /// mutation never recreates or updates a legacy control cell and fails closed on stale
    /// residence authority. Generation and admission remain immutable during an attempt.
    pub(crate) fn try_store_service_control_state(
      actor: ActorRef,
      kind: ServiceResidenceKind,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
    ) -> Result<(), crate::scheduler::EnqueueOutcome> {
      let current = Self::load_service_actor_semantic_state(actor, kind)
        .map_err(|_| crate::scheduler::EnqueueOutcome::CorruptedTopology)?;
      let mut replacement = current.clone();
      replacement.identity = identity;
      replacement.hot = hot;
      Self::mutate_actor_semantic_state(
        actor.actor_id,
        ActorSemanticMutation::Replace {
          expected: ActorSemanticState::Active(current),
          replacement: ActorSemanticState::Active(replacement.clone()),
        },
      )
      .map_err(|_| crate::scheduler::EnqueueOutcome::CorruptedTopology)?;
      matches!(
        Self::load_service_actor_semantic_state(actor, kind),
        Ok(stored) if stored == replacement
      )
      .then_some(())
      .ok_or(crate::scheduler::EnqueueOutcome::CorruptedTopology)
    }

    #[allow(
      dead_code,
      reason = "Hot-only canonical mutation remains a focused seam for carrier validation"
    )]
    pub(crate) fn try_store_service_control_hot(
      actor: ActorRef,
      kind: ServiceResidenceKind,
      hot: ActorHotStateOf<T>,
    ) -> Result<(), crate::scheduler::EnqueueOutcome> {
      let identity = Self::load_service_actor_semantic_state(actor, kind)
        .map_err(|_| crate::scheduler::EnqueueOutcome::CorruptedTopology)?
        .identity;
      Self::try_store_service_control_state(actor, kind, identity, hot)
    }

    /// In-place Hot mutation resolves against whichever authority owns the active record: a live
    /// legacy primary keeps its physical mirror, while a canonically published Actor mutates only
    /// the semantic owner and fails closed on any leftover unsignaled cell. Moving transitions
    /// publish a supplied successor instead of using this in-place seam.
    pub(crate) fn try_store_control_hot_with_authority(
      actor_id: ActorId,
      hot: ActorHotStateOf<T>,
    ) -> Result<(), crate::scheduler::EnqueueOutcome> {
      if !ActorControlLocators::<T>::contains_key(actor_id) {
        if ActorUnsignaledControlCells::<T>::contains_key(actor_id) {
          return Err(crate::scheduler::EnqueueOutcome::CorruptedTopology);
        }
        let current = ActorSemanticStates::<T>::get(actor_id)
          .ok_or(crate::scheduler::EnqueueOutcome::CorruptedTopology)?;
        let ActorSemanticState::Active(mut record) = current.clone() else {
          return Err(crate::scheduler::EnqueueOutcome::CorruptedTopology);
        };
        record.hot = hot;
        return Self::mutate_actor_semantic_state(
          actor_id,
          ActorSemanticMutation::Replace {
            expected: current,
            replacement: ActorSemanticState::Active(record),
          },
        )
        .map(|_| ())
        .map_err(|_| crate::scheduler::EnqueueOutcome::CorruptedTopology);
      }
      let current = ActorSemanticStates::<T>::get(actor_id)
        .ok_or(crate::scheduler::EnqueueOutcome::CorruptedTopology)?;
      let ActorSemanticState::Active(mut record) = current.clone() else {
        return Err(crate::scheduler::EnqueueOutcome::CorruptedTopology);
      };
      record.hot = hot.clone();
      Self::update_existing_frame_control_hot(actor_id, &hot)?;
      matches!(
        ActorSemanticStates::<T>::get(actor_id),
        Some(ActorSemanticState::Active(stored)) if stored == record
      )
      .then_some(())
      .ok_or(crate::scheduler::EnqueueOutcome::CorruptedTopology)
    }

    /// Mutates the physical primary without introducing a second hot-state owner.
    pub(crate) fn try_mutate_control_hot<R>(
      actor_id: ActorId,
      missing: Error<T>,
      mutate: impl FnOnce(&mut ActorHotStateOf<T>) -> Result<R, DispatchError>,
    ) -> Result<R, DispatchError> {
      let Some(current) = ActorSemanticStates::<T>::get(actor_id) else {
        return Err(missing.into());
      };
      let ActorSemanticState::Active(mut record) = current.clone() else {
        return Err(Error::<T>::ActorInvariant.into());
      };
      let output = mutate(&mut record.hot)?;
      Self::try_store_control_hot_with_authority(actor_id, record.hot)
        .map_err(|_| Error::<T>::ActorInvariant)?;
      Ok(output)
    }

    #[cfg(all(test, not(feature = "runtime-benchmarks")))]
    pub(crate) fn mutate_control_hot_or<R>(
      actor_id: ActorId,
      fallback: R,
      mutate: impl FnOnce(&mut ActorHotStateOf<T>) -> R,
    ) -> R {
      let Some(current) = ActorSemanticStates::<T>::get(actor_id) else {
        return fallback;
      };
      let ActorSemanticState::Active(mut record) = current.clone() else {
        return fallback;
      };
      let output = mutate(&mut record.hot);
      match Self::try_store_control_hot_with_authority(actor_id, record.hot) {
        Ok(()) => output,
        Err(_) => fallback,
      }
    }

    pub(crate) fn try_mutate_control_hot_with_authority<R>(
      actor_id: ActorId,
      missing: Error<T>,

      mutate: impl FnOnce(&mut ActorHotStateOf<T>) -> Result<R, DispatchError>,
    ) -> Result<R, DispatchError> {
      Self::try_mutate_control_hot(actor_id, missing, mutate)
    }

    /// Test-only hot mutation for a canonically published actor that owns no legacy control cell.
    /// Updates the semantic owner through the same compare-and-replace seam production uses.
    #[cfg(all(test, not(feature = "runtime-benchmarks")))]
    pub(crate) fn try_mutate_actor_hot_semantic<R>(
      actor_id: ActorId,
      missing: Error<T>,
      mutate: impl FnOnce(&mut ActorHotStateOf<T>) -> Result<R, DispatchError>,
    ) -> Result<R, DispatchError> {
      let current = ActorSemanticStates::<T>::get(actor_id).ok_or(missing)?;
      let ActorSemanticState::Active(mut record) = current.clone() else {
        return Err(Error::<T>::ActorInvariant.into());
      };
      let output = mutate(&mut record.hot)?;
      Self::mutate_actor_semantic_state(
        actor_id,
        ActorSemanticMutation::Replace {
          expected: current,
          replacement: ActorSemanticState::Active(record),
        },
      )
      .map_err(|_| Error::<T>::ActorInvariant)?;
      Ok(output)
    }

    /// Identity mutation follows its active-primary or dormant-registry owner.
    #[cfg(test)]
    pub(crate) fn try_mutate_control_identity<R>(
      actor_id: ActorId,
      missing: Error<T>,
      mutate: impl FnOnce(&mut ActorIdentityOf<T>) -> Result<R, DispatchError>,
    ) -> Result<R, DispatchError> {
      let current = ActorSemanticStates::<T>::get(actor_id).ok_or(missing)?;
      match current.clone() {
        ActorSemanticState::Active(mut record) => {
          ensure!(
            !ActorControlLocators::<T>::contains_key(actor_id)
              && !ActorUnsignaledControlCells::<T>::contains_key(actor_id),
            Error::<T>::ActorInvariant
          );
          let output = mutate(&mut record.identity)?;
          Self::mutate_actor_semantic_state(
            actor_id,
            ActorSemanticMutation::Replace {
              expected: current,
              replacement: ActorSemanticState::Active(record),
            },
          )
          .map_err(|_| Error::<T>::ActorInvariant)?;
          Ok(output)
        }
        ActorSemanticState::Dormant(mut record) => {
          ensure!(
            !ActorControlLocators::<T>::contains_key(actor_id)
              && !ActorUnsignaledControlCells::<T>::contains_key(actor_id),
            Error::<T>::ActorInvariant
          );
          let output = mutate(&mut record.identity)?;
          ActorIdentities::<T>::insert(actor_id, &record.identity);
          Self::mutate_actor_semantic_state(
            actor_id,
            ActorSemanticMutation::Replace {
              expected: current,
              replacement: ActorSemanticState::Dormant(record),
            },
          )
          .map_err(|_| Error::<T>::ActorInvariant)?;
          Ok(output)
        }
      }
    }

    /// Admission is carried by the sole active primary.
    pub(crate) fn control_admission_exists(actor_id: ActorId) -> bool {
      Self::load_control_admission(actor_id).is_some()
    }

    pub(crate) fn load_control_admission(
      actor_id: ActorId,
    ) -> Option<ActorAdmissionCertificateOf<T>> {
      match ActorSemanticStates::<T>::get(actor_id)? {
        ActorSemanticState::Active(record) => Some(record.admission),
        ActorSemanticState::Dormant(_) => None,
      }
    }

    /// Replaces admission and current-Step resources in the existing primary. Source-consumed
    /// transitions must carry admission explicitly into their destination publication.
    pub(crate) fn replace_control_admission_for_transition(
      actor_id: ActorId,
      certificate: &ActorAdmissionCertificateOf<T>,
      contract: &ActorContractOf<T>,
    ) -> bool {
      let Some(current) = ActorSemanticStates::<T>::get(actor_id) else {
        return false;
      };
      let ActorSemanticState::Active(mut semantic_record) = current.clone() else {
        return false;
      };
      let Ok((location, mut cell)) = Self::load_primary_control_cell(actor_id) else {
        return false;
      };
      if semantic_record.admission != cell.admission {
        return false;
      };
      let Some(resources) = Self::derive_step_resource_envelopes(contract).and_then(|resources| {
        if contract.steps.is_empty() {
          Some(ActorStepResourceEnvelope {
            control: T::WeightInfo::scheduler_inner_zero_step_complete(),
            effect: Weight::zero(),
          })
        } else {
          resources.get(cell.cursor as usize).copied()
        }
      }) else {
        return false;
      };
      let pointers = [
        cell.hot.wakeup_pointer,
        cell
          .hot
          .trigger_wakeup_pointer
          .map(|pointer| WakeupPointer {
            block: WakeupKey::Tick(pointer.tick),
            page_id: pointer.page_id,
            slot: pointer.slot,
          }),
      ];
      let mut reference_updates = Vec::with_capacity(2);
      for pointer in pointers.into_iter().flatten() {
        let key = (pointer.block, pointer.page_id);
        let Some(mut page) = ActorWaitingFrameChunks::<T>::get(key) else {
          return false;
        };
        match page
          .entries
          .get_mut(pointer.slot as usize)
          .and_then(Option::as_mut)
        {
          Some(ActorWaitingEntry::Reference(reference))
            if reference.actor_id == actor_id
              && reference.admission_identity == cell.admission.admission_identity =>
          {
            reference.admission_identity = certificate.admission_identity;
            reference_updates.push((key, page));
          }
          Some(ActorWaitingEntry::Primary(primary))
            if primary.actor_id == actor_id
              && primary.admission.admission_identity == cell.admission.admission_identity
              && location
                == (ActorControlLocation::Waiting {
                  key: pointer.block,
                  page: pointer.page_id,
                  slot: pointer.slot as u8,
                }) => {}
          _ => return false,
        }
      }
      cell.pipeline_service_identity = pipeline_service_identity(certificate.admission_identity);
      cell.admission = certificate.clone();
      cell.resources = resources;
      if Self::store_primary_control_cell(location, cell).is_err() {
        return false;
      }
      for (key, page) in reference_updates {
        ActorWaitingFrameChunks::<T>::insert(key, page);
      }
      let Some(next_generation) = next_actor_generation(semantic_record.generation) else {
        return false;
      };
      semantic_record.admission = certificate.clone();
      let Some(current_semantics @ ActorSemanticState::Active(_)) =
        ActorSemanticStates::<T>::get(actor_id)
      else {
        return false;
      };
      if current_semantics != ActorSemanticState::Active(semantic_record.clone()) {
        return false;
      }
      semantic_record.generation = next_generation;
      Self::mutate_actor_semantic_state(
        actor_id,
        ActorSemanticMutation::Replace {
          expected: current_semantics,
          replacement: ActorSemanticState::Active(semantic_record),
        },
      )
      .is_ok()
    }

    pub(crate) fn mutate_actor_semantic_state(
      actor_id: ActorId,
      mutation: ActorSemanticMutation<ActorSemanticStateOf<T>>,
    ) -> Result<Option<ActorSemanticStateOf<T>>, ActorSemanticMutationError> {
      let current = ActorSemanticStates::<T>::get(actor_id);
      let replacement = apply_actor_semantic_mutation(current.as_ref(), &mutation)?;
      match &replacement {
        Some(state) => ActorSemanticStates::<T>::insert(actor_id, state),
        None => ActorSemanticStates::<T>::remove(actor_id),
      }
      Ok(replacement)
    }

    /// Loads the canonical semantic owner together with its independently derived placement.
    pub(crate) fn load_actor_semantic_state(
      actor_id: ActorId,
    ) -> Result<
      Option<(
        ActorSemanticStateOf<T>,
        Option<ActorControlLocation<BlockNumberFor<T>>>,
      )>,
      ActorSemanticLoadError,
    > {
      let state = ActorSemanticStates::<T>::get(actor_id);
      let location = ActorControlLocators::<T>::get(actor_id);
      let dormant_identity = ActorIdentities::<T>::get(actor_id);
      let has_contract_or_run = ActorContractHeads::<T>::contains_key(actor_id)
        || ActorActivationAuthorities::<T>::contains_key(actor_id)
        || ActorRunStateStore::<T>::contains_key(actor_id)
        || ActorRunHeads::<T>::contains_key(actor_id)
        || ActorRunPayloads::<T>::contains_key(actor_id);
      let has_legacy_authority =
        location.is_some() || ActorUnsignaledControlCells::<T>::contains_key(actor_id);
      match (&state, dormant_identity.as_ref()) {
        (Some(ActorSemanticState::Dormant(record)), Some(stored_identity))
          if &record.identity == stored_identity
            && !has_contract_or_run
            && !has_legacy_authority
            && !ActorProcesses::<T>::contains_key(actor_id) => {}
        (Some(ActorSemanticState::Active(record)), None)
          if !has_legacy_authority
            && has_contract_or_run
            && Self::load_canonical_actor_semantic_state(ActorRef {
              actor_id,
              generation: record.generation,
            })
            .is_ok() => {}
        (None, None)
          if !has_contract_or_run
            && !has_legacy_authority
            && !ActorProcesses::<T>::contains_key(actor_id) => {}
        _ => return Err(ActorSemanticLoadError::Corrupt),
      }
      Ok(state.map(|state| (state, None)))
    }

    pub(crate) fn load_actor_state_with_admission(
      actor_id: ActorId,
    ) -> (
      LoadedActorStateOf<T>,
      Option<ActorAdmissionCertificateOf<T>>,
    ) {
      match Self::load_actor_semantic_state(actor_id) {
        Ok(None) => (LoadedActorStateOf::NotRegistered, None),
        Ok(Some((ActorSemanticState::Dormant(record), None))) => {
          (LoadedActorStateOf::Dormant(record.identity), None)
        }
        Ok(Some((ActorSemanticState::Active(record), None))) => {
          let admission = record.admission.clone();
          let state = Self::load_active_actor_state(actor_id, record);
          match state {
            LoadedActorStateOf::Active(_) => (state, Some(admission)),
            _ => (state, None),
          }
        }
        Ok(Some(_)) | Err(_) => (LoadedActorStateOf::Corrupt, None),
      }
    }

    pub(crate) fn load_actor_state(actor_id: ActorId) -> LoadedActorStateOf<T> {
      Self::load_actor_state_with_admission(actor_id).0
    }

    /// Strict active-state loader; malformed primary authority never falls back to dormancy.
    #[allow(
      dead_code,
      reason = "current-state consumer cutover is staged behind the atomic service writer"
    )]
    pub(crate) fn load_frame_actor_state(actor_id: ActorId) -> LoadedActorStateOf<T> {
      Self::load_actor_state(actor_id)
    }

    fn load_active_actor_state(
      actor_id: ActorId,
      record: ActorSemanticRecordOf<T>,
    ) -> LoadedActorStateOf<T> {
      let ActorSemanticRecord {
        identity,
        generation,
        hot,
        admission: frame_admission,
      } = record;
      if Self::load_canonical_actor_semantic_state(ActorRef {
        actor_id,
        generation,
      })
      .is_err()
      {
        return LoadedActorStateOf::Corrupt;
      }
      let Some(contract) = Self::load_contract_geometry_with_admission(actor_id, &frame_admission)
      else {
        return LoadedActorStateOf::Corrupt;
      };
      if !Self::admission_authorizes_contract_wake(&frame_admission, &contract) {
        return LoadedActorStateOf::Corrupt;
      }
      let run_state = ActorRunStateStore::<T>::get(actor_id);
      let run_is_coherent = match (hot.cycle_state, run_state.as_ref()) {
        (CycleState::Idle, None) => {
          !ActorRunHeads::<T>::contains_key(actor_id)
            && !ActorRunPayloads::<T>::contains_key(actor_id)
        }
        (CycleState::Running, Some(run)) => {
          run.running_is_coherent()
            && run.has_contract_authority(
              frame_admission.semantic_contract_id,
              frame_admission.body_commitment,
              frame_admission.admission_identity,
            )
        }
        (CycleState::Suspended, Some(run)) => {
          run.suspension_is_coherent()
            && run.has_contract_authority(
              frame_admission.semantic_contract_id,
              frame_admission.body_commitment,
              frame_admission.admission_identity,
            )
        }
        _ => false,
      };
      if !run_is_coherent
        || !hot
          .trigger_runtime_state
          .is_compatible_with(&contract.trigger)
      {
        return LoadedActorStateOf::Corrupt;
      }
      LoadedActorStateOf::Active(ActiveActorState {
        identity,
        hot,
        contract,
        run_state,
      })
    }

    pub(crate) fn admission_authorizes_contract_wake(
      admission: &ActorAdmissionCertificateOf<T>,
      contract: &ActorContractOf<T>,
    ) -> bool {
      admission.authorizes_wake(contract.trigger.wake_qualification(&contract.window))
    }

    pub(crate) fn load_crossing_idle_activation_state_with_authority(
      actor_id: ActorId,
      feed: T::ObservationFeedId,
    ) -> Option<ObservationActivationState<T>> {
      let authority = ActorActivationAuthorities::<T>::get(actor_id)?;
      let LoadedActorStateOf::Active(state) = Self::load_actor_state(actor_id) else {
        return None;
      };
      if state.hot.cycle_state != CycleState::Idle
        || !matches!(
          state.hot.trigger_runtime_state,
          TriggerRuntimeState::ObservationCrossing { .. }
        )
        || state.run_state.is_some()
      {
        return None;
      }
      let certificate = Self::build_admission_certificate(&state.contract)?;
      if authority.feed != feed
        || authority.semantic_contract_id != certificate.semantic_contract_id
        || authority.body_commitment != certificate.body_commitment
        || authority.admission_identity != certificate.admission_identity
        || !certificate.authorizes_wake(
          state
            .contract
            .trigger
            .wake_qualification(&state.contract.window),
        )
        || !matches!(
          &state.contract.trigger,
          Trigger::ObservationCrossing { feed: contract_feed, .. } if *contract_feed == feed
        )
        || authority.cooldown_blocks != state.contract.cooldown_blocks
        || authority.window != state.contract.window
        || authority.auto_close_at_cycle_nonce != state.contract.auto_close_at_cycle_nonce
        || !state
          .hot
          .trigger_runtime_state
          .is_compatible_with(&state.contract.trigger)
      {
        return None;
      }
      Some(ObservationActivationState {
        actor_id,
        identity: state.identity,
        hot: state.hot,
        authority,
        admission: Some(certificate),
        run_head: None,
        loaded_step: None,
      })
    }

    pub fn load_crossing_idle_activation_state(
      actor_id: ActorId,
      feed: T::ObservationFeedId,
    ) -> Option<ObservationActivationState<T>> {
      Self::load_crossing_idle_activation_state_with_authority(actor_id, feed)
    }

    pub(crate) fn load_observation_activation_state_with_authority(
      actor_id: ActorId,
      feed: T::ObservationFeedId,
    ) -> Option<ObservationActivationState<T>> {
      let authority = ActorActivationAuthorities::<T>::get(actor_id)?;
      if authority.feed != feed {
        return None;
      }
      let (identity, hot, admission) = Self::load_control_authority_with_authority(actor_id)?;
      if authority.semantic_contract_id != admission.semantic_contract_id
        || authority.body_commitment != admission.body_commitment
        || authority.admission_identity != admission.admission_identity
      {
        return None;
      }
      let head = ActorContractHeads::<T>::get(actor_id)?;
      if !admission.authorizes_wake(head.header.trigger.wake_qualification(&head.header.window))
        || !matches!(
          &head.header.trigger,
          Trigger::ObservationChange { feed: contract_feed } if *contract_feed == feed
        )
        || authority.cooldown_blocks != head.header.cooldown_blocks
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
        admission: Some(admission),
        run_head,
        loaded_step,
      })
    }

    pub(crate) fn load_observation_activation_state(
      actor_id: ActorId,
      feed: T::ObservationFeedId,
    ) -> Option<ObservationActivationState<T>> {
      Self::load_observation_activation_state_with_authority(actor_id, feed)
    }

    pub(crate) fn load_actor_service_state_with_control(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
    ) -> Option<(
      ActiveActorStateOf<T>,
      ActorAdmissionCertificateOf<T>,
      Option<LoadedActorStepOf<T>>,
    )> {
      let head = ActorContractHeads::<T>::get(actor_id)?;
      Self::load_actor_service_state_with_head(actor_id, identity, hot, admission, head)
    }

    pub(crate) fn load_actor_service_state_with_head(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      admission: ActorAdmissionCertificateOf<T>,
      head: ActorContractHeadOf<T>,
    ) -> Option<(
      ActiveActorStateOf<T>,
      ActorAdmissionCertificateOf<T>,
      Option<LoadedActorStepOf<T>>,
    )> {
      let run_state = ActorRunStateStore::<T>::get(actor_id);
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
          run_state,
        },
        admission,
        loaded_step,
      ))
    }

    pub(crate) fn load_actor_service_state_with_authority(
      actor_id: ActorId,
    ) -> Option<(
      ActiveActorStateOf<T>,
      ActorAdmissionCertificateOf<T>,
      Option<LoadedActorStepOf<T>>,
    )> {
      let (ActorSemanticState::Active(record), _) =
        Self::load_actor_semantic_state(actor_id).ok()??
      else {
        return None;
      };
      Self::load_actor_service_state_with_control(
        actor_id,
        record.identity,
        record.hot,
        record.admission,
      )
    }

    pub(crate) fn load_frame_actor_service_state(
      actor_id: ActorId,
    ) -> Option<(
      ActiveActorStateOf<T>,
      ActorAdmissionCertificateOf<T>,
      Option<LoadedActorStepOf<T>>,
    )> {
      Self::load_actor_service_state_with_authority(actor_id)
    }

    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn load_current_step_service_state(
      actor_id: ActorId,
    ) -> Option<(
      ActiveActorStateOf<T>,
      ActorAdmissionCertificateOf<T>,
      LoadedActorStepOf<T>,
    )> {
      let (state, admission, loaded_step) = Self::load_frame_actor_service_state(actor_id)?;
      Some((state, admission, loaded_step?))
    }

    #[cfg(any(test, feature = "runtime-benchmarks"))]
    pub(crate) fn active_actor_view(actor_id: ActorId) -> Option<ActiveActorViewOf<T>> {
      let LoadedActorStateOf::Active(state) = Self::load_actor_state_for_frame_control(actor_id)
      else {
        return None;
      };
      Some(Self::derive_active_actor_view(
        state.identity,
        state.hot,
        state.contract,
      ))
    }

    pub(crate) fn load_actor_state_with_authority(actor_id: ActorId) -> LoadedActorStateOf<T> {
      Self::load_actor_state(actor_id)
    }

    /// Lifecycle classification shares one strict canonical state boundary.
    pub(crate) fn load_actor_state_for_frame_control(actor_id: ActorId) -> LoadedActorStateOf<T> {
      Self::load_actor_state_with_authority(actor_id)
    }

    pub(crate) fn active_actor_state_for_frame_control(
      actor_id: ActorId,
    ) -> Result<ActiveActorStateOf<T>, Error<T>> {
      match Self::load_actor_state_for_frame_control(actor_id) {
        LoadedActorStateOf::Active(state) => Ok(state),
        LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Dormant(_) => {
          Err(Error::<T>::ActorNotFound)
        }
        LoadedActorStateOf::Corrupt => Err(Error::<T>::ActorInvariant),
      }
    }

    pub fn active_actor_state(actor_id: ActorId) -> Option<ActiveActorStateOf<T>> {
      match Self::load_actor_state_for_frame_control(actor_id) {
        LoadedActorStateOf::Active(state) => Some(state),
        _ => None,
      }
    }

    /// Resolves identity from its lifecycle owner; malformed active authority never falls back
    /// to a dormant registry row.
    pub fn actor_identity(actor_id: ActorId) -> Option<ActorIdentityOf<T>> {
      match Self::load_actor_state_for_frame_control(actor_id) {
        LoadedActorStateOf::Active(state) => Some(state.identity),
        LoadedActorStateOf::Dormant(identity) => Some(identity),
        LoadedActorStateOf::NotRegistered | LoadedActorStateOf::Corrupt => None,
      }
    }

    /// Reads the bounded control owner independently of funding and Contract projections.
    pub fn actor_control_cell(
      actor_id: ActorId,
    ) -> Option<(
      ActorControlLocation<BlockNumberFor<T>>,
      ActorControlCellOf<T>,
    )> {
      Self::load_primary_control_cell(actor_id).ok()
    }

    /// Reads the active Hot state from its canonical semantic owner, falling back to the legacy
    /// primary only when no semantic record exists. Canonically published Actors keep Hot state in
    /// `ActorSemanticStates`; legacy Actors mirror it there while a primary remains.
    pub fn actor_hot(actor_id: ActorId) -> Option<ActorHotStateOf<T>> {
      match ActorSemanticStates::<T>::get(actor_id) {
        Some(ActorSemanticState::Active(record)) => Some(record.hot),
        Some(ActorSemanticState::Dormant(_)) => None,
        None => Self::load_frame_control_authority(actor_id).map(|(_, _, hot, _)| hot),
      }
    }

    pub fn pending_signal(actor_id: ActorId) -> bool {
      match Self::load_actor_state_for_frame_control(actor_id) {
        LoadedActorStateOf::Active(state) => state.hot.pending_signal,
        _ => false,
      }
    }

    pub fn wakeup_pages(key: (BlockNumberFor<T>, WakeupPageId)) -> Option<ActorWaitingPageOf<T>> {
      ActorWaitingFrameChunks::<T>::get((WakeupKey::Block(key.0), key.1))
    }

    pub fn wakeup_buckets(block: BlockNumberFor<T>) -> Option<WakeupBucketState> {
      Self::wakeup_bucket_state(WakeupKey::Block(block))
    }

    pub(crate) fn wakeup_bucket_state(
      key: WakeupKey<BlockNumberFor<T>>,
    ) -> Option<WakeupBucketState> {
      let live_entries = ActorWaitingOccupancies::<T>::get(key);
      if live_entries == 0 {
        return None;
      }
      let tail = ActorWaitingTails::<T>::get(key);
      Some(WakeupBucketState {
        head_page: ActorWaitingHeads::<T>::get(key) / 32,
        tail_page: tail.checked_sub(1)? / 32,
        next_page_id: tail.div_ceil(32),
        live_entries,
        cursor_index: ActorWaitingCursorIndices::<T>::get(key),
      })
    }

    pub fn wakeup_cursor_pages(page_id: WakeupPageId) -> Option<WakeupCursorPageOf<T>> {
      WakeupCursorPages::<T>::get((WakeupClock::Block, page_id))
    }

    pub fn wakeup_cursor_len() -> WakeupCursorIndex {
      WakeupCursorLen::<T>::get(WakeupClock::Block)
    }

    pub(crate) fn active_actor_exists(actor_id: ActorId) -> bool {
      matches!(
        Self::load_actor_state_for_frame_control(actor_id),
        LoadedActorStateOf::Active(_)
      )
    }

    pub(crate) fn preflight_trigger_transition(
      actor_id: ActorId,
      trigger: &TriggerOf<T>,
      intent: TriggerTransitionIntent,
    ) -> Result<TriggerTransitionPlan<T>, DispatchError> {
      Self::preflight_trigger_transition_with_authority(actor_id, trigger, intent)
    }

    pub(crate) fn preflight_trigger_transition_with_authority(
      actor_id: ActorId,
      trigger: &TriggerOf<T>,
      intent: TriggerTransitionIntent,
    ) -> Result<TriggerTransitionPlan<T>, DispatchError> {
      Ok(TriggerTransitionPlan {
        intent,
        crossing: Self::preflight_crossing_membership_with_authority(actor_id, trigger)?,
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
      Self::preflight_trigger_transition_with_authority(actor_id, &TriggerOf::<T>::Manual, intent)
    }

    fn commit_trigger_transition(
      actor_id: ActorId,
      plan: TriggerTransitionPlan<T>,
      actor_type: ActorType,
      prospective_admission_identity: Option<[u8; 32]>,
    ) -> Result<Option<(CrossingPhase, ObservationRevision)>, DispatchError> {
      let _intent = plan.intent;
      IndexedTriggerDetectionDisabled::<T>::remove(actor_id);
      let admission_identity = prospective_admission_identity.unwrap_or([0; 32]);
      let crossing_state =
        Self::commit_crossing_membership(actor_id, plan.crossing, actor_type, admission_identity)?;
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
      let admission =
        Self::build_admission_certificate(&contract).ok_or(Error::<T>::AdmissionBoundOverflow)?;
      let crossing_state = Self::commit_trigger_transition(
        actor_id,
        transition,
        identity.actor_class.actor_type(),
        Some(admission.admission_identity),
      )?;
      hot.trigger_runtime_state = Self::installed_trigger_runtime_state(
        &contract.trigger,
        hot.trigger_runtime_state.temporal_anchor_tick(),
        crossing_state,
      )?;
      let step_resources = Self::derive_step_resource_envelopes(&contract)
        .ok_or(Error::<T>::AdmissionBoundOverflow)?;
      let resources = step_resources
        .first()
        .copied()
        .unwrap_or(ActorStepResourceEnvelope {
          control: T::WeightInfo::scheduler_inner_zero_step_complete(),
          effect: Weight::zero(),
        });
      let (generation, dormant_expected) = match intent {
        TriggerTransitionIntent::CreateActive | TriggerTransitionIntent::GenesisInstallation => {
          (1, None)
        }
        TriggerTransitionIntent::ActivateDormant => {
          let Some(ActorSemanticState::Dormant(record)) = ActorSemanticStates::<T>::get(actor_id)
          else {
            return Err(Error::<T>::ActorInvariant.into());
          };
          ensure!(record.identity == identity, Error::<T>::ActorInvariant);
          let generation =
            next_actor_generation(record.generation).ok_or(Error::<T>::ActorInvariant)?;
          (generation, Some(record))
        }
        _ => return Err(Error::<T>::ActorInvariant.into()),
      };
      let semantic_record = ActorSemanticRecord {
        identity: identity.clone(),
        generation,
        hot: hot.clone(),
        admission: admission.clone(),
      };
      let semantic_mutation = match dormant_expected {
        None => ActorSemanticMutation::Publish(ActorSemanticState::Active(semantic_record)),
        Some(expected) => ActorSemanticMutation::Replace {
          expected: ActorSemanticState::Dormant(expected),
          replacement: ActorSemanticState::Active(semantic_record),
        },
      };
      Self::mutate_actor_semantic_state(actor_id, semantic_mutation)
        .map_err(|_| Error::<T>::ActorInvariant)?;
      Self::store_actor_contract(actor_id, contract.clone())?;
      ActorIdentities::<T>::remove(actor_id);
      Self::publish_actor_publication(
        ActorRef {
          actor_id,
          generation,
        },
        &ActiveActorState {
          identity,
          hot,
          contract,
          run_state: None,
        },
        None,
        resources,
        frame_system::Pallet::<T>::block_number(),
        ServiceCutoff::Open,
      )
      .map_err(Self::placement_error)
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

    fn remove_active_actor_with_admission(
      actor_id: ActorId,
      trigger_transition: TriggerTransitionPlan<T>,
      admission: Option<&ActorAdmissionCertificateOf<T>>,
      actor_type: ActorType,
    ) -> DispatchResult {
      Self::commit_trigger_transition(actor_id, trigger_transition, actor_type, None)?;
      if ActorControlLocators::<T>::contains_key(actor_id) {
        Self::remove_primary_control_cell_inner(actor_id)
          .map_err(|_| Error::<T>::ActorInvariant)?;
      }
      let removed_contract = match admission {
        Some(admission) => {
          Self::remove_admitted_contract_geometry_with_admission(actor_id, admission)
        }
        None => Self::remove_admitted_contract_geometry(actor_id),
      };
      ensure!(removed_contract.is_some(), Error::<T>::ActorInvariant);
      ActorRunStateStore::<T>::remove(actor_id);
      Ok(())
    }
  }

  #[pallet::storage]
  #[pallet::getter(fn actor_identities)]
  pub type ActorIdentities<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorIdentityOf<T>, OptionQuery>;

  /// Canonical actor-keyed semantic authority. Physical service and deadline cells retain only
  /// placement data and must agree with the active/dormant partition represented here.
  #[pallet::storage]
  #[pallet::getter(fn actor_semantic_states)]
  pub type ActorSemanticStates<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorSemanticStateOf<T>, OptionQuery>;

  /// Canonical generation-bound process owner. This remains inert until the legacy control
  /// mutation cohorts atomically transfer scheduler authority into it.
  #[pallet::storage]
  #[pallet::getter(fn actor_processes)]
  pub type ActorProcesses<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorProcessOf<T>, OptionQuery>;

  /// Inert canonical header for the future actor-keyed persistent service ring.
  #[pallet::storage]
  #[pallet::getter(fn service_header)]
  pub type ServiceHeader<T: Config> =
    StorageValue<_, ServiceHeaderRecord<BlockNumberFor<T>>, ValueQuery>;

  /// Inert canonical generation-bound nodes for the future persistent service ring.
  #[pallet::storage]
  #[pallet::getter(fn service_nodes)]
  pub type ServiceNodes<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ServiceNode<BlockNumberFor<T>>, OptionQuery>;

  /// Inert monotone scalar source allocator for exact typed Oracle-feed identities.
  #[pallet::storage]
  #[pallet::getter(fn dependency_source_allocator)]
  pub type DependencySourceAllocatorState<T: Config> =
    StorageValue<_, DependencySourceAllocator, ValueQuery>;

  /// Inert typed Oracle-feed to scalar dependency-source identity mapping.
  #[pallet::storage]
  #[pallet::getter(fn observation_dependency_sources)]
  pub type ObservationDependencySources<T: Config> =
    StorageMap<_, Blake2_128Concat, T::ObservationFeedId, DependencySourceId, OptionQuery>;

  /// Inert reverse mapping proving scalar source identity ownership without a scan.
  #[pallet::storage]
  #[pallet::getter(fn dependency_source_observations)]
  pub type DependencySourceObservations<T: Config> =
    StorageMap<_, Blake2_128Concat, DependencySourceId, T::ObservationFeedId, OptionQuery>;

  /// Inert checked revisions for future event-complete dependency sources.
  #[pallet::storage]
  #[pallet::getter(fn dependency_revisions)]
  pub type DependencyRevisions<T: Config> =
    StorageMap<_, Blake2_128Concat, DependencySourceId, DependencyRevisionState, ValueQuery>;

  /// Inert fair selector for sources that retain active dependency scans.
  #[pallet::storage]
  #[pallet::getter(fn dependency_scan_source_list)]
  pub type DependencyScanSourceListState<T> = StorageValue<_, DependencyScanSourceList, ValueQuery>;

  /// Inert exact circular-list membership for one active dependency source.
  #[pallet::storage]
  #[pallet::getter(fn dependency_scan_source_node)]
  pub type DependencyScanSourceNodes<T> =
    StorageMap<_, Blake2_128Concat, DependencySourceId, DependencyScanSourceNode, OptionQuery>;

  /// Inert one-per-Actor activation-check ownership, bound to generation and plan revision.
  #[pallet::storage]
  #[pallet::getter(fn pending_check_owners)]
  pub type PendingCheckOwners<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, PendingCheckOwner, OptionQuery>;

  /// Inert source-owned fixed-width registration topology.
  #[pallet::storage]
  #[pallet::getter(fn dependency_registration_headers)]
  pub type DependencyRegistrationHeaders<T: Config> =
    StorageMap<_, Blake2_128Concat, DependencySourceId, DependencyRegistrationHeader, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn dependency_registration_pages)]
  pub type DependencyRegistrationPages<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    DependencySourceId,
    Blake2_128Concat,
    u64,
    DependencyRegistrationPage,
    OptionQuery,
  >;

  /// Inert bounded reusable holes; active scans defer reuse so their cursor cannot be retargeted.
  #[pallet::storage]
  #[pallet::getter(fn dependency_registration_free_positions)]
  pub type DependencyRegistrationFreePositions<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    DependencySourceId,
    Blake2_128Concat,
    u32,
    DependencyRegistrationPosition,
    OptionQuery,
  >;

  /// Inert exact source/Actor positions into canonical registration pages.
  #[pallet::storage]
  #[pallet::getter(fn dependency_registration_positions)]
  pub type DependencyRegistrationPositions<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    DependencySourceId,
    Blake2_128Concat,
    ActorId,
    DependencyRegistrationPosition,
    OptionQuery,
  >;

  /// Inert exact source-to-Actor reverse registrations for future dependency-keyed parking.
  #[pallet::storage]
  #[pallet::getter(fn dependency_registrations)]
  pub type DependencyRegistrations<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    DependencySourceId,
    Blake2_128Concat,
    ActorId,
    DependencyRegistrationHandle,
    OptionQuery,
  >;

  /// Inert Actor-owned complete registration plan; this is the removal-completeness authority.
  #[pallet::storage]
  #[pallet::getter(fn dependency_plans)]
  pub type DependencyPlans<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    ActorId,
    BoundedVec<DependencyPlanRegistration, T::MaxContractSteps>,
    ValueQuery,
  >;

  /// Inert Actor-owned optional timed review retained with the complete dependency plan.
  #[pallet::storage]
  #[pallet::getter(fn dependency_timed_reviews)]
  pub type DependencyTimedReviews<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, DependencyTimedReview<BlockNumberFor<T>>, OptionQuery>;

  /// Inert durable Pending destination for one event-complete source notification.
  #[pallet::storage]
  #[pallet::getter(fn pending_dependency_events)]
  pub type PendingDependencyEvents<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, PendingDependencyEvent, OptionQuery>;

  /// Inert durable Pending destination for one due dependency review.
  #[pallet::storage]
  #[pallet::getter(fn pending_dependency_reviews)]
  pub type PendingDependencyReviews<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, DependencyTimedReview<BlockNumberFor<T>>, OptionQuery>;

  /// Inert bucket ownership for the future retained C32 deadline carrier.
  #[pallet::storage]
  #[pallet::getter(fn deadline_headers)]
  pub type DeadlineHeaders<T: Config> =
    StorageMap<_, Blake2_128Concat, WakeupKey<BlockNumberFor<T>>, DeadlineHeader, OptionQuery>;

  /// Inert retained C32 pages for the future deadline carrier.
  #[pallet::storage]
  #[pallet::getter(fn deadline_pages)]
  pub type DeadlinePages<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    WakeupKey<BlockNumberFor<T>>,
    Blake2_128Concat,
    u64,
    DeadlinePage,
    OptionQuery,
  >;

  /// Inert generation-bound reverse handles for exact process or Park deadline removal.
  ///
  /// Trigger deadlines use their own reverse owner because one Actor may simultaneously own a
  /// process residence and an independent temporal Trigger membership.
  #[pallet::storage]
  #[pallet::getter(fn deadline_handles)]
  pub type DeadlineHandles<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, DeadlineHandleOf<T>, OptionQuery>;

  /// Inert generation-bound reverse handles for exact temporal Trigger deadline removal.
  #[pallet::storage]
  #[pallet::getter(fn trigger_deadline_handles)]
  pub type TriggerDeadlineHandles<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, DeadlineHandleOf<T>, OptionQuery>;

  /// Inert C32 pages of the clock-local min-heaps over nonempty canonical deadline buckets.
  #[pallet::storage]
  #[pallet::getter(fn deadline_index_pages)]
  pub type DeadlineIndexPages<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    WakeupClock,
    Blake2_128Concat,
    u64,
    DeadlineIndexPageOf<T>,
    OptionQuery,
  >;

  /// Exact reverse position for every key in the inert canonical deadline min-heaps.
  #[pallet::storage]
  #[pallet::getter(fn deadline_index_positions)]
  pub type DeadlineIndexPositions<T: Config> =
    StorageMap<_, Blake2_128Concat, WakeupKey<BlockNumberFor<T>>, u32, OptionQuery>;

  /// Logical length of each inert clock-local canonical deadline min-heap.
  #[pallet::storage]
  #[pallet::getter(fn deadline_index_len)]
  pub type DeadlineIndexLen<T> = StorageMap<_, Blake2_128Concat, WakeupClock, u32, ValueQuery>;

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

  /// Sole primary control cells for active Actors without current process readiness.
  #[pallet::storage]
  pub type ActorUnsignaledControlCells<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorControlCellOf<T>, OptionQuery>;

  /// Ticket-addressed C32 chunks for the one canonical Ready FIFO.
  #[pallet::storage]
  pub type ActorReadyFrameChunks<T: Config> =
    StorageMap<_, Blake2_128Concat, QueuePageId, ActorControlChunkOf<T>, OptionQuery>;

  /// Deadline-addressed C32 chunks for non-ready active control cells.
  #[pallet::storage]
  pub type ActorWaitingFrameChunks<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    (WakeupKey<BlockNumberFor<T>>, QueuePageId),
    ActorWaitingPageOf<T>,
    OptionQuery,
  >;

  /// External-boundary locator for the sole primary active control cell.
  #[pallet::storage]
  pub type ActorControlLocators<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorControlLocation<BlockNumberFor<T>>, OptionQuery>;

  #[pallet::storage]
  pub type ActorReadyHead<T> = StorageValue<_, QueueTicket, ValueQuery>;

  #[pallet::storage]
  pub type ActorReadyTail<T> = StorageValue<_, QueueTicket, ValueQuery>;

  #[pallet::storage]
  pub type ActorReadyOccupancy<T> = StorageValue<_, u32, ValueQuery>;

  #[pallet::storage]
  pub type ActorWaitingHeads<T: Config> =
    StorageMap<_, Blake2_128Concat, WakeupKey<BlockNumberFor<T>>, u64, ValueQuery>;

  #[pallet::storage]
  pub type ActorWaitingTails<T: Config> =
    StorageMap<_, Blake2_128Concat, WakeupKey<BlockNumberFor<T>>, u64, ValueQuery>;

  #[pallet::storage]
  pub type ActorWaitingOccupancies<T: Config> =
    StorageMap<_, Blake2_128Concat, WakeupKey<BlockNumberFor<T>>, u32, ValueQuery>;

  #[pallet::storage]
  pub type ActorWaitingCursorIndices<T: Config> =
    StorageMap<_, Blake2_128Concat, WakeupKey<BlockNumberFor<T>>, WakeupCursorIndex, OptionQuery>;

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

    fn dormant_system_actors() -> alloc::vec::Vec<(ActorId, AccountId, Mutability)> {
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
      }
      for (actor_id, owner, mutability) in T::GenesisSystemActors::dormant_system_actors() {
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
          mutability,
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
        assert!(
          Pallet::<T>::mutate_actor_semantic_state(
            actor_id,
            ActorSemanticMutation::Publish(ActorSemanticState::Dormant(
              DormantActorSemanticRecord {
                identity: identity.clone(),
                generation: 0,
              },
            )),
          )
          .is_ok(),
          "duplicate genesis dormant semantic state: {actor_id}"
        );
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
      let minimum_reservation = if all_minimum_quanta.all_lte(remaining) {
        MaterializationMinimumReservation::ReserveAllFamilies
      } else {
        MaterializationMinimumReservation::Unavailable
      };
      for offset in 0u8..3 {
        let family = family_cursor.saturating_add(offset) % 3;
        let family_budget = Self::materialization_family_budget(
          family_cursor,
          offset,
          remaining,
          minimum_reservation,
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
      let cutoff = ActorReadyTail::<T>::get();
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
        Self::ready_work_exists()
          && ActorReadyTail::<T>::get()
            .checked_sub(ActorReadyHead::<T>::get())
            .is_some_and(|span| span >= u64::from(T::MaxQueueLength::get())),
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
        .ok_or(Error::<T>::ResourceProtocolFailed)?;
      let drain_finalization_control = T::WeightInfo::scheduler_on_idle_base()
        .checked_add(&T::WeightInfo::block_resource_finalize())
        .ok_or(Error::<T>::ResourceProtocolFailed)?;
      let prepass_control = control_remaining
        .checked_sub(&drain_finalization_control)
        .ok_or(Error::<T>::ResourceProtocolFailed)?;
      let prepass_limit = prepass_control
        .checked_add(&budget.limits().actor_base_turn())
        .ok_or(Error::<T>::ResourceProtocolFailed)?;
      let pass = Self::execute_cycle_to_cutoff_with_resources(
        prepass_limit,
        cutoff,
        &mut state,
        budget.limits(),
        BlockResourceDomain::ActorBaseEffect,
        prepass_control,
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
      let wakeup_unit = T::WeightInfo::scheduler_wakeup_cursor_worker_future().saturating_add(
        Self::wakeup_cursor_drain_unit_weight_upper(
          crate::scheduler::WakeupBucketDisposition::Remove,
        ),
      );
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
      let cleanup_units = u32::from(Self::ready_work_exists());
      let queue_cleanup_weight = T::WeightInfo::scheduler_paged_tombstone_drain(cleanup_units);
      let saturated_cleanup_weight = if legacy_unmetered_materialization
        && cleanup_units > 0
        && ActorReadyTail::<T>::get()
          .checked_sub(ActorReadyHead::<T>::get())
          .is_some_and(|span| span >= u64::from(T::MaxQueueLength::get()))
        && queue_cleanup_weight.all_lte(after_base)
      {
        let cutoff = ActorReadyTail::<T>::get();
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
      let before_deadlines = fixed_weight
        .saturating_add(saturated_cleanup_weight)
        .saturating_add(materialization_weight);
      let deadline_weight = now
        .checked_add(&One::one())
        .zip(Self::current_scheduler_tick().ok())
        .and_then(|(next_block, now_tick)| {
          now_tick
            .checked_add(1)
            .map(|next_tick| (now_tick, next_block, next_tick))
        })
        .map_or_else(Weight::zero, |(now_tick, next_block, next_tick)| {
          let mut meter =
            WeightMeter::with_limit(control_available.saturating_sub(before_deadlines));
          let _ = Self::service_due_deadline_frontiers(
            &mut meter,
            ServiceResidenceKind::Live,
            now,
            now_tick,
            Some(WakeupKey::Block(next_block)),
            Some(WakeupKey::Tick(next_tick)),
          );
          meter.consumed()
        });
      let housekeeping_weight = before_deadlines.saturating_add(deadline_weight);
      let remaining_after_housekeeping = available.saturating_sub(housekeeping_weight);
      Self::settle_on_idle_control(&mut control_authority, housekeeping_weight);
      let execution_cutoff = PrepassExecutionCutoff::<T>::get()
        .filter(|(cutoff_block, _)| *cutoff_block == now)
        .map(|(_, cutoff)| cutoff)
        .unwrap_or_else(ActorReadyTail::<T>::get);
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
          let mut service_meter = WeightMeter::with_limit(remaining_after_housekeeping);
          let _ = Self::service_canonical_round_head_with_resources(
            &mut service_meter,
            now,
            &mut state,
            budget.limits(),
            BlockResourceDomain::ActorDrainEffect,
          );
          let service_weight = service_meter.consumed();
          let control_maximum = budget
            .limits()
            .actor_control()
            .checked_sub(&state.usage().actor_control_used())
            .unwrap_or_else(Weight::zero);
          let mut pass = if breaker_active {
            CyclePass {
              consumed: Weight::zero(),
              effect_consumed: Weight::zero(),
              effect_reconciliation_uncertain: false,
              starved: false,
            }
          } else {
            Self::execute_cycle_to_cutoff_with_resources(
              remaining_after_housekeeping.saturating_sub(service_weight),
              execution_cutoff,
              &mut state,
              budget.limits(),
              BlockResourceDomain::ActorDrainEffect,
              control_maximum,
            )
          };
          pass.consumed = pass.consumed.saturating_add(service_weight);
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
        None => {
          let mut service_meter = WeightMeter::with_limit(remaining_after_housekeeping);
          let _ = Self::service_canonical_round_head(&mut service_meter, now);
          let service_weight = service_meter.consumed();
          let mut pass = if breaker_active {
            CyclePass {
              consumed: Weight::zero(),
              effect_consumed: Weight::zero(),
              effect_reconciliation_uncertain: false,
              starved: false,
            }
          } else {
            Self::execute_cycle_to_cutoff(
              remaining_after_housekeeping.saturating_sub(service_weight),
              execution_cutoff,
            )
          };
          pass.consumed = pass.consumed.saturating_add(service_weight);
          pass
        }
      };
      if !breaker_active {
        Self::update_idle_starvation_state(now, pass.starved);
      }
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
      let state = Self::active_actor_state_for_frame_control(actor_id)?;
      let continuation = state.run_state.clone();
      let snapshot = Self::derive_active_actor_view(
        state.identity.clone(),
        state.hot.clone(),
        state.contract.clone(),
      );
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
        let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor_id)
        else {
          return Err(Error::<T>::ActorInvariant.into());
        };
        let actor = ActorRef {
          actor_id,
          generation: record.generation,
        };
        let resources = if state.contract.steps.is_empty() {
          ActorStepResourceEnvelope {
            control: T::WeightInfo::scheduler_inner_zero_step_complete(),
            effect: Weight::zero(),
          }
        } else {
          let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
          Self::derive_step_resource_envelopes(&state.contract)
            .and_then(|envelopes| envelopes.get(cursor as usize).copied())
            .ok_or(Error::<T>::ActorInvariant)?
        };
        let mut paused = state.clone();
        paused.identity.last_control_mutation_block = now;
        paused.hot.lifecycle = ActiveLifecycle::Paused;
        paused.hot.queue_ticket = None;
        Self::transition_actor_publication_to_successor(
          actor,
          &state,
          &paused,
          paused.run_state.as_ref(),
          resources,
          now,
          ServiceCutoff::Open,
        )
        .map_err(Self::placement_error)?;
        Self::deposit_event(Event::ActorPaused { actor_id });
        Ok(())
      })
    }

    #[pallet::call_index(5)]
    #[pallet::weight(T::WeightInfo::resume_actor().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn resume_actor(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      ensure!(
        !ActorIdentities::<T>::contains_key(actor_id),
        Error::<T>::ActorInvariant
      );
      let state = Self::active_actor_state_for_frame_control(actor_id)?;
      ensure!(
        state.hot.cycle_state != CycleState::Idle
          || (!ActorRunHeads::<T>::contains_key(actor_id)
            && !ActorRunPayloads::<T>::contains_key(actor_id)),
        Error::<T>::ActorInvariant
      );
      let continuation = state.run_state.clone();
      let snapshot = Self::derive_active_actor_view(
        state.identity.clone(),
        state.hot.clone(),
        state.contract.clone(),
      );
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
        let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor_id)
        else {
          return Err(Error::<T>::ActorInvariant.into());
        };
        let actor = ActorRef {
          actor_id,
          generation: record.generation,
        };
        let (_, process) = Self::load_canonical_actor_semantic_state(actor)
          .map_err(|_| Error::<T>::ActorInvariant)?;
        let resources = if state.contract.steps.is_empty() {
          ActorStepResourceEnvelope {
            control: T::WeightInfo::scheduler_inner_zero_step_complete(),
            effect: Weight::zero(),
          }
        } else {
          let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
          Self::derive_step_resource_envelopes(&state.contract)
            .and_then(|envelopes| envelopes.get(cursor as usize).copied())
            .ok_or(Error::<T>::ActorInvariant)?
        };
        let mut resumed = state.clone();
        resumed.identity.last_control_mutation_block = now;
        resumed.hot.lifecycle = ActiveLifecycle::Active;
        resumed.hot.queue_ticket = None;
        Self::transition_actor_publication_to_successor(
          actor,
          &state,
          &resumed,
          resumed.run_state.as_ref(),
          resources,
          now,
          ServiceCutoff::Open,
        )
        .map_err(Self::placement_error)?;
        ensure!(
          ActorProcesses::<T>::get(actor_id).is_some_and(|successor| {
            successor.generation == actor.generation && successor != process
          }),
          Error::<T>::ActorInvariant
        );
        Self::deposit_event(Event::ActorResumed { actor_id });
        Ok(())
      })
    }

    #[pallet::call_index(6)]
    #[pallet::weight(T::WeightInfo::manual_trigger().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn manual_trigger(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResultWithPostInfo {
      let state = Self::active_actor_state_for_frame_control(actor_id)?;
      let continuation = state.run_state.clone();
      let snapshot = Self::derive_active_actor_view(
        state.identity.clone(),
        state.hot.clone(),
        state.contract.clone(),
      );
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
      // A Manual occurrence while the current Cycle is open is intentionally ignored before
      // Trigger-fee admission: current-state service owns no deferred future-Cycle latch.
      // A duplicate occurrence that is already latched but not yet serviced is likewise a
      // coalescing no-op rather than an invariant failure: one useful false->true transition owns
      // the charged readiness and later occurrences neither re-charge nor create a second cycle.
      if snapshot.cycle_state != CycleState::Idle || snapshot.pending_signal {
        return Ok(().into());
      }
      let actor_type = snapshot.actor_class.actor_type();
      let breakdown = Self::trigger_fee_for_weight(
        actor_type,
        TriggerFamily::Manual,
        T::WeightInfo::manual_trigger(),
      );
      let mut trigger_processed = false;
      Self::with_control_transaction(|| {
        let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor_id)
        else {
          return Err(Error::<T>::ActorInvariant.into());
        };
        let actor = ActorRef {
          actor_id,
          generation: record.generation,
        };
        let outcome = Self::commit_canonical_trigger_occurrence_with_authority(
          actor,
          actor_type,
          &snapshot.sovereign_account,
          breakdown,
          state,
          frame_system::Pallet::<T>::block_number(),
        )?;
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
      match Self::load_actor_state_for_frame_control(actor_id) {
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
          ensure!(
            identity.mutability == Mutability::Mutable,
            Error::<T>::ImmutableActor
          );
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
      let state = Self::active_actor_state_for_frame_control(actor_id)?;
      let continuation = state.run_state;
      let current_contract = state.contract.clone();
      let snapshot = Self::derive_active_actor_view(state.identity, state.hot, state.contract);
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
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
      let replacement_admission =
        Self::build_admission_certificate(&contract).ok_or(Error::<T>::AdmissionBoundOverflow)?;
      // Every non-no-op Contract update rotates semantic and admission authority, so an open run
      // cannot remain bound to the replaced Contract even when only completion policy changes.
      let cancellation_reason = Some(CancellationReason::ContractReplaced);
      let schedule_anchor = Self::schedule_anchor_at(contract.window, now);
      let temporal_anchor_tick =
        Self::temporal_anchor_tick(&contract.trigger).map_err(Self::placement_error)?;
      let trigger_transition = schedule_changed
        .then(|| {
          Self::preflight_trigger_transition_with_authority(
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
          Self::commit_trigger_transition(
            actor_id,
            transition,
            snapshot.actor_class.actor_type(),
            Some(replacement_admission.admission_identity),
          )?
        } else {
          None
        };
        let legacy_authority = ActorControlLocators::<T>::contains_key(actor_id)
          || ActorUnsignaledControlCells::<T>::contains_key(actor_id);
        if schedule_changed && legacy_authority {
          let (state, admission, _) =
            Self::load_frame_actor_service_state(actor_id).ok_or(Error::<T>::ActorInvariant)?;
          if state.hot.trigger_wakeup_pointer.is_some() {
            Self::trigger_wakeup_substrate_invalidate_loaded(actor_id, state, &admission)
              .map_err(Self::placement_error)?;
          }
        }
        Self::try_mutate_control_hot_with_authority(
          actor_id,
          Error::<T>::ActorNotFound,
          |hot| -> DispatchResult {
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
            Self::record_control_mutation_with_authority(actor_id, now)?;
            Ok(())
          },
        )?;
        // Crossing compilation binds the newly installed runtime phase to the replacement
        // admission identity, so publish hot schedule authority before storing its Contract.
        Self::store_actor_contract(actor_id, contract.clone())?;
        Self::deposit_event(Event::ContractUpdated { actor_id });
        #[cfg(test)]
        crate::mock::control_atomicity_checkpoint(actor_id)?;
        // A canonical Contract replacement already republished one generation-bound process and
        // residence, so the legacy prime must not run a second time through another authority.
        let canonical_republished = ActorProcesses::<T>::get(actor_id).is_some_and(|process| {
          matches!(
            ActorSemanticStates::<T>::get(actor_id),
            Some(ActorSemanticState::Active(record)) if record.generation == process.generation
          )
        });
        if !canonical_republished && (schedule_changed || run_cancelled) {
          Self::prime_frame_actor_schedule(actor_id).map_err(Self::placement_error)?;
        }
        Self::reconcile_actor_state_hold_with_authority(actor_id)?;
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
          let state = match Self::load_actor_state_for_frame_control(actor_id) {
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
      let identity = match Self::load_actor_state_for_frame_control(actor_id) {
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
      let instance = match Self::load_actor_state_for_frame_control(actor_id) {
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
    #[pallet::weight(T::WeightInfo::run_cancel().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn cancel_run(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      let state = Self::active_actor_state_for_frame_control(actor_id)?;
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
        Self::record_control_mutation_with_authority(actor_id, now)?;
        Self::prime_frame_actor_schedule(actor_id).map_err(Self::placement_error)
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
      minimum_reservation: MaterializationMinimumReservation,
    ) -> Weight {
      if !minimum_reservation.reserves_all_families() {
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
          .saturating_add(Self::wakeup_cursor_drain_unit_weight_upper(
            crate::scheduler::WakeupBucketDisposition::Remove,
          )),
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

    pub(crate) fn owner_slot_is_set(bitmap: &OwnerSlotBitmap, owner_slot: u8) -> bool {
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

    fn derive_actor_state_hold_with_authority(
      actor_id: ActorId,
      identity: &ActorIdentityOf<T>,
      authority: Option<(&ActorHotStateOf<T>, &ActorAdmissionCertificateOf<T>)>,
    ) -> Result<ActorStateHoldBreakdownOf<T>, Error<T>> {
      let mut identity_bytes = 0usize;
      Self::add_state_hold_encoded_size(&mut identity_bytes, &actor_id)?;
      if authority.is_some() {
        let control_identity = Self::control_identity_from_scalar(identity.clone())
          .ok_or(Error::<T>::StateHoldInvariant)?;
        Self::add_state_hold_encoded_size(&mut identity_bytes, &control_identity)?;
      } else {
        Self::add_state_hold_encoded_size(&mut identity_bytes, identity)?;
      }
      Self::add_state_hold_encoded_size(&mut identity_bytes, &identity.sovereign_account)?;

      let mut breakdown = ActorStateHoldBreakdown {
        identity: Self::state_hold_component(identity_bytes)?,
        contract_head: T::Balance::zero(),
        contract_body: T::Balance::zero(),
        detector: T::Balance::zero(),
        run: T::Balance::zero(),
      };
      if identity.actor_class.actor_type() == ActorType::System {
        return Ok(ActorStateHoldBreakdown {
          identity: T::Balance::zero(),
          ..breakdown
        });
      }

      let Some((hot, admission)) = authority else {
        ensure!(
          !ActorContractHeads::<T>::contains_key(actor_id)
            && !Self::control_admission_exists(actor_id)
            && !ActorRunStateStore::<T>::contains_key(actor_id),
          Error::<T>::StateHoldInvariant
        );
        return Ok(breakdown);
      };
      let head = ActorContractHeads::<T>::get(actor_id).ok_or(Error::<T>::StateHoldInvariant)?;

      breakdown.contract_head =
        Self::state_hold_component(Self::control_state_hold_head_bytes(&head, admission)?)?;

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

      breakdown.detector = Self::state_hold_component(Self::state_hold_detector_bytes(
        actor_id,
        hot.trigger_wakeup_pointer,
      )?)?;
      breakdown.run = Self::state_hold_component(
        <ActorRunStateOf<T> as codec::MaxEncodedLen>::max_encoded_len(),
      )?;
      Ok(breakdown)
    }

    fn control_state_hold_head_bytes(
      head: &ActorContractHeadOf<T>,
      admission: &ActorAdmissionCertificateOf<T>,
    ) -> Result<usize, Error<T>> {
      let mut bytes =
        <ActorControlHotState<BlockNumberFor<T>> as codec::MaxEncodedLen>::max_encoded_len();
      for component in [
        <u32 as codec::MaxEncodedLen>::max_encoded_len(),
        <Option<BlockNumberFor<T>> as codec::MaxEncodedLen>::max_encoded_len(),
        <ActorStepResourceEnvelope as codec::MaxEncodedLen>::max_encoded_len(),
      ] {
        bytes = bytes
          .checked_add(component)
          .ok_or(Error::<T>::StateHoldOverflow)?;
      }
      Self::add_state_hold_encoded_size(&mut bytes, admission)?;
      Self::add_state_hold_encoded_size(&mut bytes, head)?;
      Ok(bytes)
    }

    fn state_hold_detector_bytes(
      actor_id: ActorId,
      trigger_wakeup_pointer: Option<TriggerWakeupPointer>,
    ) -> Result<usize, Error<T>> {
      let mut bytes = 0usize;
      let activation = ActorActivationAuthorities::<T>::get(actor_id);
      if let Some(activation) = &activation {
        Self::add_state_hold_encoded_size(&mut bytes, activation)?;
      }
      if let Some(feeds) = ActorObservationFeeds::<T>::get(actor_id) {
        Self::add_state_hold_encoded_size(&mut bytes, &feeds)?;
      }
      if let Some(slot) = ObservationSubscriptionSlot::<T>::get(actor_id) {
        Self::add_state_hold_encoded_size(&mut bytes, &slot)?;
      }
      let crossing_locator = CrossingMemberships::<T>::get(actor_id);
      if let Some(locator) = &crossing_locator {
        Self::add_state_hold_encoded_size(&mut bytes, locator)?;
      }
      if activation.is_some() || crossing_locator.is_some() {
        Self::add_state_hold_encoded_size(&mut bytes, &())?;
      }
      if let Some(pointer) = trigger_wakeup_pointer {
        Self::add_state_hold_encoded_size(&mut bytes, &pointer)?;
      }
      Ok(bytes)
    }

    pub(crate) fn reconcile_actor_state_hold_with_authority(actor_id: ActorId) -> DispatchResult {
      let existing = ActorStateHolds::<T>::get(actor_id);
      let target = if ActorControlLocators::<T>::contains_key(actor_id) {
        let (_, identity, hot, admission) =
          Self::load_frame_control_authority(actor_id).ok_or(Error::<T>::StateHoldInvariant)?;
        if identity.actor_class.actor_type() == ActorType::User {
          Some((
            identity.owner.clone(),
            Self::derive_actor_state_hold_with_authority(
              actor_id,
              &identity,
              Some((&hot, &admission)),
            )?,
          ))
        } else {
          None
        }
      } else {
        ensure!(
          !ActorUnsignaledControlCells::<T>::contains_key(actor_id),
          Error::<T>::StateHoldInvariant
        );
        match ActorSemanticStates::<T>::get(actor_id) {
          Some(ActorSemanticState::Active(record))
            if record.identity.actor_class.actor_type() == ActorType::User =>
          {
            Some((
              record.identity.owner.clone(),
              Self::derive_actor_state_hold_with_authority(
                actor_id,
                &record.identity,
                Some((&record.hot, &record.admission)),
              )?,
            ))
          }
          Some(ActorSemanticState::Dormant(record))
            if record.identity.actor_class.actor_type() == ActorType::User =>
          {
            Some((
              record.identity.owner.clone(),
              Self::derive_actor_state_hold_with_authority(actor_id, &record.identity, None)?,
            ))
          }
          Some(_) | None => None,
        }
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

    /// Atomically charges one useful Trigger occurrence and replaces an exact canonical
    /// publication with its latched successor. Payment or publication refusal restores the
    /// complete storage root.
    pub(crate) fn commit_canonical_trigger_occurrence_with_authority(
      actor: ActorRef,
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      breakdown: TriggerFeeBreakdown<T::Balance>,
      state: ActiveActorStateOf<T>,
      now: BlockNumberFor<T>,
    ) -> Result<crate::scheduler::ActivationOutcome, DispatchError> {
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        let result = (|| {
          ensure!(!state.hot.pending_signal, Error::<T>::ActorInvariant);
          Self::ensure_trigger_occurrence_capacity(actor_type, sovereign_account, breakdown)?;
          ensure!(
            state.identity.actor_class.actor_type() == actor_type
              && state.identity.sovereign_account == *sovereign_account,
            Error::<T>::ActorInvariant
          );

          // The caller may hold only the bounded current-Step service envelope, whose
          // reconstructed Contract cannot reproduce the full body commitment. The stored semantic
          // record is the authority: require the caller's generation, identity, Hot state, Run
          // store, and Contract trigger authorization to agree before latching it.
          let Some(ActorSemanticState::Active(stored)) =
            ActorSemanticStates::<T>::get(actor.actor_id)
          else {
            return Err(Error::<T>::ActorInvariant.into());
          };
          ensure!(
            stored.generation == actor.generation
              && stored.identity == state.identity
              && stored.hot == state.hot
              && Self::admission_authorizes_contract_wake(&stored.admission, &state.contract)
              && ActorRunStateStore::<T>::get(actor.actor_id)
                .as_ref()
                .map(|run| run.encode())
                == state.run_state.as_ref().map(|run| run.encode())
              && !ActorControlLocators::<T>::contains_key(actor.actor_id)
              && !ActorUnsignaledControlCells::<T>::contains_key(actor.actor_id),
            Error::<T>::ActorInvariant
          );
          let expected_semantic = ActorSemanticState::Active(stored);
          let process = ActorProcesses::<T>::get(actor.actor_id)
            .filter(|process| process.generation == actor.generation)
            .ok_or(Error::<T>::ActorInvariant)?;
          let plan = plan_canonical_occurrence(
            state.hot.cycle_state,
            state.hot.pending_signal,
            process,
            now,
          )
          .map_err(|_| Error::<T>::ActorInvariant)?
          .ok_or(Error::<T>::ActorInvariant)?;

          let mut successor = state.clone();
          successor.hot.pending_signal = plan.pending_signal;
          if TriggerDeadlineHandles::<T>::contains_key(actor.actor_id) {
            Self::remove_trigger_deadline_member(actor).map_err(|_| Error::<T>::ActorInvariant)?;
            successor.hot.trigger_wakeup_pointer = None;
          } else {
            ensure!(
              successor.hot.trigger_wakeup_pointer.is_none(),
              Error::<T>::ActorInvariant
            );
          }

          match plan.publication {
            CanonicalOccurrencePublication::PreserveResidence => {
              ensure!(plan.process == process, Error::<T>::ActorInvariant);
            }
            CanonicalOccurrencePublication::PublishPending { eligible_from } => {
              match process.residence {
                Some(ProcessResidence::Deadline { .. }) => {
                  Self::remove_deadline_member(actor).map_err(|_| Error::<T>::ActorInvariant)?;
                  ActorProcesses::<T>::insert(actor.actor_id, plan.process);
                  Self::insert_service_member(actor, ServiceResidenceKind::Pending, now)
                    .map_err(|_| Error::<T>::ActorInvariant)?;
                }
                Some(ProcessResidence::Parked(evidence)) => {
                  let owner = PendingCheckOwners::<T>::get(actor.actor_id)
                    .ok_or(Error::<T>::ActorInvariant)?;
                  Self::wake_parked_member_to_service(
                    actor,
                    ServiceResidenceKind::Pending,
                    owner,
                    evidence,
                    eligible_from,
                  )
                  .map_err(|_| Error::<T>::ActorInvariant)?;
                  ensure!(
                    ActorProcesses::<T>::get(actor.actor_id) == Some(plan.process),
                    Error::<T>::ActorInvariant
                  );
                }
                None if matches!(process.status, ProcessStatus::Disabled(_)) => {
                  ActorProcesses::<T>::insert(actor.actor_id, plan.process);
                  Self::insert_service_member(actor, ServiceResidenceKind::Pending, now)
                    .map_err(|_| Error::<T>::ActorInvariant)?;
                }
                // A completed Cycle leaves an Idle Service resident in the ring. Re-latching it
                // is a new B+1 occurrence: refresh the semantic latch, relabel the residence to
                // Pending and re-admit it at the next block rather than rejecting the transition.
                Some(ProcessResidence::Service(_)) => {
                  Self::remove_service_member(actor).map_err(|_| Error::<T>::ActorInvariant)?;
                  ActorProcesses::<T>::insert(actor.actor_id, plan.process);
                  Self::insert_service_member(actor, ServiceResidenceKind::Pending, now)
                    .map_err(|_| Error::<T>::ActorInvariant)?;
                }
                _ => return Err(Error::<T>::ActorInvariant.into()),
              }
            }
          }

          let ActorSemanticState::Active(mut replacement) = expected_semantic.clone() else {
            return Err(Error::<T>::ActorInvariant.into());
          };
          replacement.hot = successor.hot;
          Self::mutate_actor_semantic_state(
            actor.actor_id,
            ActorSemanticMutation::Replace {
              expected: expected_semantic,
              replacement: ActorSemanticState::Active(replacement),
            },
          )
          .map_err(|_| Error::<T>::ActorInvariant)?;
          Self::charge_trigger_occurrence(actor_type, sovereign_account, breakdown)?;
          Self::deposit_event(Event::TriggerOccurrenceProcessed {
            actor_id: actor.actor_id,
            trigger_family: breakdown.trigger_family,
            fee: breakdown.trigger_fee,
          });
          Ok(crate::scheduler::ActivationOutcome::Latched)
        })();
        match result {
          Ok(outcome) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(outcome))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
    }

    pub(crate) fn commit_frame_trigger_occurrence(
      actor_id: ActorId,
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      breakdown: TriggerFeeBreakdown<T::Balance>,
      _cause_provenance: TriggerCauseProvenance,
    ) -> Result<Option<crate::scheduler::ActivationOutcome>, DispatchError> {
      let mut state = Self::active_actor_state_for_frame_control(actor_id)?;
      let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor_id) else {
        return Err(Error::<T>::ActorInvariant.into());
      };
      state.identity = record.identity.clone();
      state.hot = record.hot.clone();
      let actor = ActorRef {
        actor_id,
        generation: record.generation,
      };
      Self::commit_canonical_trigger_occurrence_with_authority(
        actor,
        actor_type,
        sovereign_account,
        breakdown,
        state,
        frame_system::Pallet::<T>::block_number(),
      )
      .map(Some)
    }

    pub(crate) fn try_commit_frame_automatic_trigger_occurrence(
      actor_id: ActorId,
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      breakdown: TriggerFeeBreakdown<T::Balance>,
      cause_provenance: TriggerCauseProvenance,
    ) -> Result<Option<crate::scheduler::ActivationOutcome>, DispatchError> {
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Self::commit_frame_trigger_occurrence(
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

    #[allow(
      dead_code,
      reason = "legacy control-reference profiles retain prechecked Trigger collection"
    )]
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
      let head = ActorContractHeads::<T>::get(actor_id).ok_or(Error::<T>::ActorInvariant)?;
      Self::pipeline_capacity_sufficient_with_envelope(
        actor_type,
        sovereign_account,
        head.header.pipeline_machine_envelope,
      )
    }

    pub(crate) fn pipeline_capacity_sufficient_with_envelope(
      actor_type: ActorType,
      sovereign_account: &T::AccountId,
      envelope: PipelineMachineEnvelope<T::Balance>,
    ) -> Result<bool, Error<T>> {
      if actor_type == ActorType::System {
        return Ok(true);
      }
      let breakdown = Self::pipeline_fee_breakdown(actor_type, envelope)?;
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
        matches!(
          Self::load_actor_state(actor_id),
          LoadedActorStateOf::NotRegistered
        ),
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
        if Self::mutate_actor_semantic_state(
          actor_id,
          ActorSemanticMutation::Publish(ActorSemanticState::Dormant(DormantActorSemanticRecord {
            identity: identity.clone(),
            generation: 0,
          })),
        )
        .is_err()
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorInvariant.into(),
          ));
        }
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
        if let Err(error) = Self::reconcile_actor_state_hold_with_authority(actor_id) {
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
      let actor_id = NextActorId::<T>::get();
      ensure!(
        matches!(
          Self::load_actor_state(actor_id),
          LoadedActorStateOf::NotRegistered
        ),
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
        if let Err(error) = Self::reconcile_actor_state_hold_with_authority(actor_id) {
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
      ensure!(
        Self::active_instance_count() < Self::effective_active_actor_limit(),
        Error::<T>::ActiveActorCapacityExceeded
      );
      let now = frame_system::Pallet::<T>::block_number();
      ensure!(
        identity.last_control_mutation_block != now,
        Error::<T>::ControlMutationRateLimited
      );
      let Some(ActorSemanticState::Dormant(dormant_record)) =
        ActorSemanticStates::<T>::get(actor_id)
      else {
        return Err(Error::<T>::ActorNotFound.into());
      };
      ensure!(
        dormant_record.identity == identity,
        Error::<T>::ActorInvariant
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
        if !Self::control_identity_exists(actor_id) || Self::active_actor_exists(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorAlreadyActive.into(),
          ));
        }
        if Self::mutate_actor_semantic_state(
          actor_id,
          ActorSemanticMutation::Replace {
            expected: ActorSemanticState::Dormant(dormant_record.clone()),
            replacement: ActorSemanticState::Dormant(DormantActorSemanticRecord {
              identity: identity.clone(),
              generation: dormant_record.generation,
            }),
          },
        )
        .is_err()
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorInvariant.into(),
          ));
        }
        ActorIdentities::<T>::insert(actor_id, &identity);
        if let Err(error) = Self::insert_active_actor(
          actor_id,
          identity,
          hot,
          contract,
          TriggerTransitionIntent::ActivateDormant,
        ) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
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
        if let Err(error) = Self::reconcile_actor_state_hold_with_authority(actor_id) {
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
        if let Err(error) = Self::record_control_mutation_with_authority(actor_id, now) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if let Err(error) =
          Self::cancel_run_internal(actor_id, CancellationReason::Deactivated, None)
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        let Some((state, admission, _)) = Self::load_frame_actor_service_state(actor_id) else {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorInvariant.into(),
          ));
        };
        // A canonically published Actor owns its scheduler residence in the generation-bound
        // process carrier, not in a legacy primary/FIFO cell. Detach that publication atomically
        // so the dormant identity retains no scheduler work; the legacy retained-frame path keeps
        // its queue/wakeup cleanup for pre-cutover Actors.
        let canonical = !ActorControlLocators::<T>::contains_key(actor_id)
          && !ActorUnsignaledControlCells::<T>::contains_key(actor_id);
        if canonical {
          let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor_id)
          else {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
              Error::<T>::ActorInvariant.into(),
            ));
          };
          let supplied_run = state.run_state.clone();
          if let Err(error) = Self::detach_actor_publication(
            ActorRef {
              actor_id,
              generation: record.generation,
            },
            state.clone(),
            supplied_run.as_ref(),
          ) {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
          }
        } else {
          if let Err(error) = Self::remove_actor_from_queues_with_authority(actor_id) {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
          }
          if state.hot.wakeup_pointer.is_some() {
            let invalidated =
              Self::wakeup_substrate_invalidate_loaded(actor_id, state.clone(), &admission).is_ok();
            if !invalidated {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                Error::<T>::ActorNotFound.into(),
              ));
            }
          }
          if state.hot.trigger_wakeup_pointer.is_some() {
            let invalidated =
              Self::trigger_wakeup_substrate_invalidate_loaded(actor_id, state.clone(), &admission)
                .is_ok();
            if !invalidated {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                Error::<T>::ActorNotFound.into(),
              ));
            }
          }
        }
        if let Err(error) = Self::remove_active_actor_with_admission(
          actor_id,
          trigger_transition,
          Some(&admission),
          state.identity.actor_class.actor_type(),
        ) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        let Some(expected) = ActorSemanticStates::<T>::get(actor_id) else {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorInvariant.into(),
          ));
        };
        let ActorSemanticState::Active(ref active_record) = expected else {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorInvariant.into(),
          ));
        };
        let generation = active_record.generation;
        if Self::mutate_actor_semantic_state(
          actor_id,
          ActorSemanticMutation::Replace {
            expected,
            replacement: ActorSemanticState::Dormant(DormantActorSemanticRecord {
              identity: state.identity.clone(),
              generation,
            }),
          },
        )
        .is_err()
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorInvariant.into(),
          ));
        }
        ActorIdentities::<T>::insert(actor_id, state.identity.clone());
        if let Err(error) = ActiveActorCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_sub(1)
            .ok_or(Error::<T>::ActiveActorCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if let Err(error) = Self::reconcile_actor_state_hold_with_authority(actor_id) {
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
        *clauses = BoundedVec::try_from(canonical_clauses)
          .map_err(|_| Error::<T>::AdmissionBoundOverflow)?;
      }
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
          for predicate in precondition.clauses.iter().flat_map(|clause| clause.iter()) {
            let max_age_blocks = match predicate {
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
            AmountResolution::Percent(value) if value.is_zero()
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

    fn record_control_mutation_with_authority(
      actor_id: ActorId,
      now: BlockNumberFor<T>,
    ) -> DispatchResult {
      let current = ActorSemanticStates::<T>::get(actor_id).ok_or(Error::<T>::ActorNotFound)?;
      let ActorSemanticState::Active(mut record) = current.clone() else {
        return Err(Error::<T>::ActorInvariant.into());
      };
      record.identity.last_control_mutation_block = now;
      Self::mutate_actor_semantic_state(
        actor_id,
        ActorSemanticMutation::Replace {
          expected: current,
          replacement: ActorSemanticState::Active(record),
        },
      )
      .map_err(|_| Error::<T>::ActorInvariant)?;
      Ok(())
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

    pub(crate) fn remove_owner_slot_binding(
      owner: &T::AccountId,
      owner_slot: u8,
      sovereign: &T::AccountId,
    ) {
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
      let state = Self::active_actor_state_for_frame_control(actor_id)?;
      let current = Self::derive_active_actor_view(
        state.identity.clone(),
        state.hot.clone(),
        state.contract.clone(),
      );
      ensure!(current == *instance, Error::<T>::ActorNotFound);
      // A canonically published Actor owns its terminal residence in the generation-bound process
      // carrier instead of a legacy primary cell. Route it through the atomic canonical removal so
      // close releases the Service/Deadline residence and the process publication exactly once.
      if !ActorControlLocators::<T>::contains_key(actor_id)
        && !ActorUnsignaledControlCells::<T>::contains_key(actor_id)
      {
        let Some(ActorSemanticState::Active(record)) = ActorSemanticStates::<T>::get(actor_id)
        else {
          return Err(Error::<T>::ActorNotFound.into());
        };
        let supplied_run = state.run_state.clone();
        return Self::remove_actor_publication_and_finalize(
          ActorRef {
            actor_id,
            generation: record.generation,
          },
          state,
          supplied_run.as_ref(),
          reason,
        );
      }
      let (_, _, admission) =
        Self::load_control_authority_with_authority(actor_id).ok_or(Error::<T>::ActorNotFound)?;
      Self::finalize_actor_from_retained_state(actor_id, state, &admission, reason)
    }

    pub(crate) fn finalize_actor_from_consumed_state(
      actor_id: ActorId,
      state: ActiveActorStateOf<T>,
      admission: &ActorAdmissionCertificateOf<T>,
      reason: CloseReason,
    ) -> DispatchResult {
      ensure!(
        !ActorControlLocators::<T>::contains_key(actor_id)
          && !ActorUnsignaledControlCells::<T>::contains_key(actor_id),
        Error::<T>::ActorInvariant
      );
      let instance = Self::derive_active_actor_view(
        state.identity.clone(),
        state.hot.clone(),
        state.contract.clone(),
      );
      Self::finalize_actor_loaded_inner(
        actor_id,
        &instance,
        reason,
        crate::execution::LoadedCancellationContext::ConsumedFrame {
          admission: admission.clone(),
          state,
        },
      )
    }

    pub(crate) fn finalize_actor_from_retained_state(
      actor_id: ActorId,
      state: ActiveActorStateOf<T>,
      admission: &ActorAdmissionCertificateOf<T>,
      reason: CloseReason,
    ) -> DispatchResult {
      let instance = Self::derive_active_actor_view(
        state.identity.clone(),
        state.hot.clone(),
        state.contract.clone(),
      );
      Self::finalize_actor_loaded_inner(
        actor_id,
        &instance,
        reason,
        crate::execution::LoadedCancellationContext::RetainedFrame {
          admission: admission.clone(),
          state,
        },
      )
    }

    fn finalize_actor_loaded_inner(
      actor_id: ActorId,
      instance: &ActiveActorViewOf<T>,
      reason: CloseReason,
      mut cancellation_context: crate::execution::LoadedCancellationContext<T>,
    ) -> DispatchResult {
      let admission = cancellation_context.admission().clone();
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
      let identity = ActorIdentity {
        sovereign_account: instance.sovereign_account.clone(),
        owner: instance.owner.clone(),
        actor_class: instance.actor_class,
        mutability: instance.mutability,
        cycle_nonce: instance.cycle_nonce,
        last_control_mutation_block: instance.last_control_mutation_block,
      };

      polkadot_sdk::frame_support::storage::with_transaction(|| {
        let result = (|| -> DispatchResult {
          let (close_state, close_admission) = match &mut cancellation_context {
            crate::execution::LoadedCancellationContext::RetainedFrame { state, admission }
            | crate::execution::LoadedCancellationContext::ConsumedFrame { state, admission } => {
              (state, admission)
            }
          };
          // A canonically published Actor owns its temporal Trigger residence in the
          // generation-bound `TriggerDeadlineHandles` carrier; only a pre-cutover Actor keeps the
          // legacy waiting-page reference that `invalidate_wakeup_reference` can release.
          if TriggerDeadlineHandles::<T>::contains_key(actor_id) {
            let generation = ActorSemanticStates::<T>::get(actor_id)
              .and_then(|state| match state {
                ActorSemanticState::Active(record) => Some(record.generation),
                ActorSemanticState::Dormant(_) => None,
              })
              .ok_or(Error::<T>::ActorInvariant)?;
            Self::remove_trigger_deadline_member(ActorRef { actor_id, generation })
              .map_err(|_| Error::<T>::ActorInvariant)?;
            close_state.hot.trigger_wakeup_pointer = None;
          } else if let Some(pointer) = close_state.hot.trigger_wakeup_pointer {
            Self::invalidate_wakeup_reference(
              actor_id,
              WakeupPointer {
                block: WakeupKey::Tick(pointer.tick),
                page_id: pointer.page_id,
                slot: pointer.slot,
              },
              close_admission.admission_identity,
            )
            .map_err(|_| Error::<T>::ActorInvariant)?;
            close_state.hot.trigger_wakeup_pointer = None;
          }
          if let Some(pointer) = close_state.hot.wakeup_pointer {
            Self::invalidate_wakeup_reference(
              actor_id,
              pointer,
              close_admission.admission_identity,
            )
            .map_err(|_| Error::<T>::ActorInvariant)?;
            close_state.hot.wakeup_pointer = None;
          }
          Self::cancel_run_internal_loaded(
            actor_id,
            &identity,
            CancellationReason::Closing(reason),
            None,
            cancellation_context,
          )?;

          // Exact secondary references are gone; remove the sole primary without a population scan.
          Self::remove_active_actor_with_admission(
            actor_id,
            trigger_transition,
            Some(&admission),
            instance.actor_class.actor_type(),
          )?;
          let semantic_state = ActorSemanticStates::<T>::get(actor_id)
            .filter(|state| matches!(state, ActorSemanticState::Active(_)))
            .ok_or(Error::<T>::ActorInvariant)?;
          Self::mutate_actor_semantic_state(
            actor_id,
            ActorSemanticMutation::Remove {
              expected: semantic_state,
            },
          )
          .map_err(|_| Error::<T>::ActorInvariant)?;
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
          Self::reconcile_actor_state_hold_with_authority(actor_id)?;
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
        Self::load_control_identity(actor_id).as_ref() == Some(identity),
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
        Self::mutate_actor_semantic_state(
          actor_id,
          ActorSemanticMutation::Remove {
            expected: ActorSemanticState::Dormant(DormantActorSemanticRecord {
              identity: identity.clone(),
              generation: ActorSemanticStates::<T>::get(actor_id)
                .and_then(|state| match state {
                  ActorSemanticState::Dormant(record) => Some(record.generation),
                  ActorSemanticState::Active(_) => None,
                })
                .ok_or(Error::<T>::ActorInvariant)?,
            }),
          },
        )
        .map_err(|_| Error::<T>::ActorInvariant)?;
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
        Self::reconcile_actor_state_hold_with_authority(actor_id)?;
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

    pub(crate) fn remove_actor_from_queues_with_authority(actor_id: ActorId) -> DispatchResult {
      match ActorControlLocators::<T>::get(actor_id) {
        Some(ActorControlLocation::Unsignaled | ActorControlLocation::Waiting { .. }) => Ok(()),
        Some(ActorControlLocation::Ready { .. }) => {
          Self::remove_primary_control_cell_inner(actor_id)
            .map(|_| ())
            .map_err(|_| Error::<T>::ActorInvariant.into())
        }
        None => Err(Error::<T>::ActorInvariant.into()),
      }
    }

    #[cfg(feature = "try-runtime")]
    pub(crate) fn do_try_state() -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      use polkadot_sdk::sp_runtime::TryRuntimeError;
      if PrepassExecutionCutoff::<T>::get()
        .is_some_and(|(_, cutoff)| cutoff > ActorReadyTail::<T>::get())
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
      let control_entries = Self::frame_control_entries().ok_or(TryRuntimeError::Other(
        "ActorControl frame topology is corrupt",
      ))?;
      let actual_active_count = control_entries.len() as u32;
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
          "ActiveActorCount does not match ActorControl primary cardinality",
        ));
      }
      if active_count > limit {
        return Err(TryRuntimeError::Other(
          "ActorControl primary count exceeds effective active actor limit",
        ));
      }
      let mut semantic_identities = alloc::collections::BTreeMap::new();
      for (actor_id, location, primary) in &control_entries {
        let (identity, _, _) = Self::project_control_cell(primary, *location).ok_or(
          TryRuntimeError::Other("ActorControl primary cannot restore its semantic identity"),
        )?;
        if semantic_identities.insert(*actor_id, identity).is_some() {
          return Err(TryRuntimeError::Other(
            "multiple ActorControl primaries own one identity",
          ));
        }
      }
      for (actor_id, identity) in ActorIdentities::<T>::iter() {
        if ActorControlLocators::<T>::contains_key(actor_id) {
          return Err(TryRuntimeError::Other(
            "dormant ActorIdentity duplicates active primary authority",
          ));
        }
        if !matches!(
          Self::load_actor_state_for_frame_control(actor_id),
          LoadedActorStateOf::Dormant(_)
        ) || semantic_identities.insert(actor_id, identity).is_some()
        {
          return Err(TryRuntimeError::Other(
            "scalar ActorIdentity without a primary is not uniquely dormant",
          ));
        }
      }
      let actual_identity_count = u32::try_from(semantic_identities.len())
        .map_err(|_| TryRuntimeError::Other("semantic Actor identity count exceeds u32"))?;
      let identity_count = ActorIdentityCount::<T>::get();
      if identity_count != actual_identity_count {
        return Err(TryRuntimeError::Other(
          "ActorIdentityCount does not match active primaries plus dormant identities",
        ));
      }
      if identity_count > T::MaxActorIdentities::get() {
        return Err(TryRuntimeError::Other(
          "ActorIdentityCount exceeds MaxActorIdentities",
        ));
      }
      for actor_id in ActorContractHeads::<T>::iter_keys() {
        if !matches!(
          Self::load_actor_state_for_frame_control(actor_id),
          LoadedActorStateOf::Active(_)
        ) {
          return Err(TryRuntimeError::Other(
            "ActorContract entry belongs to a corrupt actor partition set",
          ));
        }
      }
      for (actor_id, chunk_index) in ActorContractTailChunks::<T>::iter_keys() {
        let head = ActorContractHeads::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
          "ActorContract tail has no Contract head owner",
        ))?;
        let tail_count = head
          .header
          .step_count
          .saturating_sub(1)
          .div_ceil(MAX_STEPS_PER_TAIL_CHUNK);
        if chunk_index >= tail_count {
          return Err(TryRuntimeError::Other(
            "ActorContract tail key exceeds the declared Contract geometry",
          ));
        }
      }
      for actor_id in ActorActivationAuthorities::<T>::iter_keys() {
        if !ActorContractHeads::<T>::contains_key(actor_id) {
          return Err(TryRuntimeError::Other(
            "Actor activation authority has no Contract head owner",
          ));
        }
      }
      let mut max_id: Option<ActorId> = None;
      for (actor_id, _, primary) in control_entries {
        let LoadedActorStateOf::Active(state) = Self::load_actor_state_for_frame_control(actor_id)
        else {
          return Err(TryRuntimeError::Other(
            "ActorControl primary belongs to a corrupt actor partition set",
          ));
        };
        let frame_admission = primary.admission;
        let identity = state.identity;
        if identity.last_control_mutation_block > frame_system::Pallet::<T>::block_number() {
          return Err(TryRuntimeError::Other(
            "actor control mutation block is in the future",
          ));
        }
        let hot = state.hot;
        let contract = state.contract;
        let head = ActorContractHeads::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
          "Active actor has no C6 Contract head",
        ))?;
        let resources = Self::derive_step_resource_envelopes(&contract).ok_or(
          TryRuntimeError::Other("Active actor Step resources cannot be rederived"),
        )?;
        if primary.pipeline_service_identity
          != pipeline_service_identity(frame_admission.admission_identity)
        {
          return Err(TryRuntimeError::Other(
            "ActorControl primary service identity is invalid",
          ));
        }
        let expected_cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
        if primary.cursor != expected_cursor {
          return Err(TryRuntimeError::Other(
            "ActorControl primary cursor disagrees with canonical Run authority",
          ));
        }
        let expected_resources = if contract.steps.is_empty() {
          ActorStepResourceEnvelope {
            control: T::WeightInfo::scheduler_inner_zero_step_complete(),
            effect: Weight::zero(),
          }
        } else {
          *resources
            .get(expected_cursor as usize)
            .ok_or(TryRuntimeError::Other(
              "ActorControl primary cursor has no canonical Step resources",
            ))?
        };
        if primary.resources != expected_resources {
          return Err(TryRuntimeError::Other(
            "ActorControl primary resources disagree with canonical Step authority",
          ));
        }
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
            if authority.feed != feed
              || authority.cooldown_blocks != contract.cooldown_blocks
              || authority.window != contract.window
              || authority.auto_close_at_cycle_nonce != contract.auto_close_at_cycle_nonce
              || authority.semantic_contract_id != frame_admission.semantic_contract_id
              || authority.body_commitment != frame_admission.body_commitment
              || authority.admission_identity != frame_admission.admission_identity
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
        let expected_feeds = Self::derive_observation_feeds(&contract.trigger).map_err(|_| {
          TryRuntimeError::Other("Actor Trigger observation feeds cannot be rederived")
        })?;
        if ActorObservationFeeds::<T>::get(actor_id).unwrap_or_default() != expected_feeds {
          return Err(TryRuntimeError::Other(
            "Actor observation subscription membership disagrees with its Trigger",
          ));
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
            if !hot.pending_signal && hot.trigger_wakeup_pointer.is_none() =>
          {
            return Err(TryRuntimeError::Other(
              "unlatched temporal actor has no Trigger temporal membership",
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
      for actor_id in ActorRunHeads::<T>::iter_keys() {
        if !ActorRunPayloads::<T>::contains_key(actor_id) {
          return Err(TryRuntimeError::Other(
            "ActorRunHead has no immutable ActorRunPayload",
          ));
        }
        let LoadedActorStateOf::Active(state) = Self::load_actor_state_for_frame_control(actor_id)
        else {
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
        if !run_state.opening_snapshot.is_empty() {
          return Err(TryRuntimeError::Other(
            "ActorRunState retains removed Opening state",
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
      for (actor_id, identity) in &semantic_identities {
        if ActorControlLocators::<T>::contains_key(actor_id) {
          continue;
        }
        if identity.last_control_mutation_block > frame_system::Pallet::<T>::block_number() {
          return Err(TryRuntimeError::Other(
            "actor control mutation block is in the future",
          ));
        }
        max_id = Some(max_id.map_or(*actor_id, |prev| prev.max(*actor_id)));
        if ActorRunHeads::<T>::contains_key(actor_id)
          || ActorRunPayloads::<T>::contains_key(actor_id)
        {
          return Err(TryRuntimeError::Other(
            "Dormant identity owns active scheduler or readiness state",
          ));
        }
        match SovereignIndex::<T>::get(&identity.sovereign_account) {
          Some(mapped_id) if mapped_id == *actor_id => {}
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
            if identity.mutability == Mutability::Immutable
              && !T::GenesisSystemActors::dormant_system_actors()
                .into_iter()
                .any(|(genesis_id, owner, mutability)| {
                  genesis_id == *actor_id
                    && genesis_id == sovereign_id
                    && owner == identity.owner
                    && mutability == Mutability::Immutable
                })
            {
              return Err(TryRuntimeError::Other(
                "Immutable Dormant System Actor is not declared by genesis",
              ));
            }
            if SystemSovereigns::<T>::get(sovereign_id)
              != Some(SystemSovereignState::Occupied(*actor_id))
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
          let Some(identity) = semantic_identities.get(&actor_id) else {
            return Err(TryRuntimeError::Other(
              "OwnerSlotBitmaps bit has no semantic Actor identity owner",
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
      for (actor_id, identity) in &semantic_identities {
        if let ActorClass::System { sovereign_id } = identity.actor_class
          && system_identity_owners
            .insert(sovereign_id, *actor_id)
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
            let identity = semantic_identities
              .get(&actor_id)
              .ok_or(TryRuntimeError::Other(
                "occupied System sovereign locator has no semantic Actor identity",
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
        let identity = semantic_identities
          .get(&actor_id)
          .ok_or(TryRuntimeError::Other(
            "SovereignIndex owner has no semantic Actor identity",
          ))?;
        if identity.sovereign_account != sovereign_account {
          return Err(TryRuntimeError::Other(
            "SovereignIndex key disagrees with ActorIdentity custody account",
          ));
        }
      }
      if sovereign_index_count != identity_count {
        return Err(TryRuntimeError::Other(
          "SovereignIndex cardinality does not match semantic Actor identities",
        ));
      }

      let queue_capacity = T::MaxQueueLength::get();
      if queue_capacity < limit {
        return Err(TryRuntimeError::Other(
          "MaxQueueLength is below effective active actor limit",
        ));
      }
      let queue_occupancy = ActorReadyOccupancy::<T>::get();
      if queue_occupancy > queue_capacity {
        return Err(TryRuntimeError::Other(
          "canonical queue physical occupancy exceeds MaxQueueLength",
        ));
      }
      const CONTROL_PAGE_SIZE: u64 = 32;
      let head = ActorReadyHead::<T>::get();
      let tail = ActorReadyTail::<T>::get();
      let span = tail
        .checked_sub(head)
        .ok_or(TryRuntimeError::Other("canonical Ready head exceeds tail"))?;
      if span > u64::from(queue_capacity)
        || queue_occupancy > active_count
        || u64::from(queue_occupancy) > span
      {
        return Err(TryRuntimeError::Other(
          "canonical Ready live count or physical span exceeds its bound",
        ));
      }
      let mut ready_pages = alloc::collections::BTreeSet::new();
      let mut ready_live = 0u32;
      for (page_id, page) in ActorReadyFrameChunks::<T>::iter(/* deos-bypass: bounded-iter -- TryRuntime-only topology audit. */)
      {
        let start = page_id
          .checked_mul(CONTROL_PAGE_SIZE)
          .ok_or(TryRuntimeError::Other(
            "canonical Ready page range overflows",
          ))?;
        if page.len() != CONTROL_PAGE_SIZE as usize
          || start >= tail
          || start.saturating_add(CONTROL_PAGE_SIZE) <= head
          || !ready_pages.insert(page_id)
        {
          return Err(TryRuntimeError::Other(
            "canonical Ready page has invalid width or range",
          ));
        }
        for (slot, cell) in page.iter(/* deos-bypass: bounded-iter -- fixed C32. */).enumerate() {
          let Some(cell) = cell else { continue };
          let ticket = start
            .checked_add(slot as u64)
            .ok_or(TryRuntimeError::Other("canonical Ready ticket overflows"))?;
          let location = ActorControlLocation::Ready { ticket };
          if ticket < head
            || ticket >= tail
            || ActorControlLocators::<T>::get(cell.actor_id) != Some(location)
            || Self::project_control_cell(cell, location).is_none()
          {
            return Err(TryRuntimeError::Other(
              "canonical Ready primary disagrees with its ticket or locator",
            ));
          }
          ready_live = ready_live.checked_add(1).ok_or(TryRuntimeError::Other(
            "canonical Ready live count overflows",
          ))?;
        }
      }
      let expected_ready_pages = if head == tail {
        0
      } else {
        (tail - 1) / CONTROL_PAGE_SIZE - head / CONTROL_PAGE_SIZE + 1
      };
      if ready_pages.len() as u64 != expected_ready_pages || ready_live != queue_occupancy {
        return Err(TryRuntimeError::Other(
          "canonical Ready pages or live occupancy disagree",
        ));
      }
      // Empty slots inside the bounded [head, tail) span are FIFO tombstones, not live occupancy.
      if T::WakeupPageSize::get() == 0 {
        return Err(TryRuntimeError::Other("WakeupPageSize must be non-zero"));
      }
      let mut waiting_pages = alloc::collections::BTreeMap::new();
      let mut waiting_live = alloc::collections::BTreeMap::new();
      let mut reference_count = 0u64;
      for ((key, page_id), page) in ActorWaitingFrameChunks::<T>::iter(/* deos-bypass: bounded-iter -- TryRuntime-only topology audit. */)
      {
        if page.entries.len() != CONTROL_PAGE_SIZE as usize
          || page.scan_slot >= CONTROL_PAGE_SIZE as u32
          || page
            .entries
            .iter(/* deos-bypass: bounded-iter -- fixed C32. */)
            .take(page.scan_slot as usize)
            .any(Option::is_some)
        {
          return Err(TryRuntimeError::Other(
            "canonical Waiting page has invalid width or scan cursor",
          ));
        }
        let actual_live = page.entries.iter(/* deos-bypass: bounded-iter -- fixed C32. */).filter(|entry| entry.is_some()).count()
          as u32;
        if actual_live == 0 || actual_live != page.live_entries {
          return Err(TryRuntimeError::Other(
            "canonical Waiting page live count disagrees with slots",
          ));
        }
        let count = waiting_live.entry(key).or_insert(0u32);
        *count = count
          .checked_add(actual_live)
          .ok_or(TryRuntimeError::Other(
            "canonical Waiting per-key count overflows",
          ))?;
        for (slot, entry) in
          page.entries.iter(/* deos-bypass: bounded-iter -- fixed C32. */).enumerate()
        {
          let Some(entry) = entry else { continue };
          let pointer = WakeupPointer {
            block: key,
            page_id,
            slot: slot as u32,
          };
          let (actor_id, primary) = match entry {
            ActorWaitingEntry::Primary(cell) => {
              let location = ActorControlLocation::Waiting {
                key,
                page: page_id,
                slot: slot as u8,
              };
              if ActorControlLocators::<T>::get(cell.actor_id) != Some(location)
                || Self::project_control_cell(cell, location).is_none()
              {
                return Err(TryRuntimeError::Other(
                  "canonical Waiting primary disagrees with its locator",
                ));
              }
              (cell.actor_id, cell.clone())
            }
            ActorWaitingEntry::Reference(reference) => {
              let (_, primary) =
                Self::load_primary_control_cell(reference.actor_id).map_err(|_| {
                  TryRuntimeError::Other("canonical Waiting reference has no primary")
                })?;
              if primary.admission.admission_identity != reference.admission_identity {
                return Err(TryRuntimeError::Other(
                  "canonical Waiting reference admission identity disagrees",
                ));
              }
              reference_count = reference_count
                .checked_add(1)
                .ok_or(TryRuntimeError::Other(
                  "canonical Waiting reference count overflows",
                ))?;
              (reference.actor_id, primary)
            }
          };
          let authoritative = match key {
            WakeupKey::Block(_) => primary.hot.wakeup_pointer,
            WakeupKey::Tick(_) => primary
              .hot
              .trigger_wakeup_pointer
              .map(|trigger| WakeupPointer {
                block: WakeupKey::Tick(trigger.tick),
                page_id: trigger.page_id,
                slot: trigger.slot,
              }),
          };
          if primary.actor_id != actor_id || authoritative != Some(pointer) {
            return Err(TryRuntimeError::Other(
              "WakeupPage slot addresses an actor with a different clock-domain pointer",
            ));
          }
        }
        if waiting_pages.insert((key, page_id), page).is_some() {
          return Err(TryRuntimeError::Other("duplicate canonical Waiting page"));
        }
      }
      if u64::from(active_count)
        .checked_add(reference_count)
        .is_none_or(|count| count > u64::from(active_count) * 3)
      {
        return Err(TryRuntimeError::Other(
          "canonical primary plus reference cardinality exceeds 3N",
        ));
      }
      let mut directory_keys = alloc::collections::BTreeSet::new();
      let mut visited_pages = alloc::collections::BTreeSet::new();
      for (key, occupancy) in ActorWaitingOccupancies::<T>::iter(/* deos-bypass: bounded-iter -- TryRuntime-only topology audit. */)
      {
        if occupancy == 0
          || occupancy > T::MaxActiveActors::get()
          || waiting_live.get(&key).copied() != Some(occupancy)
          || !ActorWaitingHeads::<T>::contains_key(key)
          || !ActorWaitingTails::<T>::contains_key(key)
          || !directory_keys.insert(key)
        {
          return Err(TryRuntimeError::Other(
            "canonical Waiting directory occupancy disagrees with pages",
          ));
        }
        let head = ActorWaitingHeads::<T>::get(key);
        let tail = ActorWaitingTails::<T>::get(key);
        if head >= tail {
          return Err(TryRuntimeError::Other(
            "canonical Waiting directory has invalid range",
          ));
        }
        let first = head / CONTROL_PAGE_SIZE;
        let last = (tail - 1) / CONTROL_PAGE_SIZE;
        let mut page_id = first;
        let mut previous = None;
        loop {
          if !visited_pages.insert((key, page_id)) {
            return Err(TryRuntimeError::Other("canonical Waiting page links cycle"));
          }
          let page = waiting_pages
            .get(&(key, page_id))
            .ok_or(TryRuntimeError::Other(
              "canonical Waiting linked page is missing",
            ))?;
          if page.previous_page != previous
            || page_id > last
            || (page_id == first && head % CONTROL_PAGE_SIZE != u64::from(page.scan_slot))
            || (page_id != first && page.scan_slot != 0)
          {
            return Err(TryRuntimeError::Other(
              "canonical Waiting page links or head cursor disagree",
            ));
          }
          if page_id == last {
            let tail_slot = ((tail - 1) % CONTROL_PAGE_SIZE + 1) as usize;
            if page.next_page.is_some()
              || page.entries.iter(/* deos-bypass: bounded-iter -- fixed C32. */).skip(tail_slot).any(Option::is_some)
            {
              return Err(TryRuntimeError::Other(
                "canonical Waiting tail page exceeds directory range",
              ));
            }
            break;
          }
          let next = page.next_page.ok_or(TryRuntimeError::Other(
            "canonical Waiting linked chain ends before tail",
          ))?;
          if next <= page_id {
            return Err(TryRuntimeError::Other(
              "canonical Waiting links are not strictly ordered",
            ));
          }
          previous = Some(page_id);
          page_id = next;
        }
        let index = ActorWaitingCursorIndices::<T>::get(key).ok_or(TryRuntimeError::Other(
          "canonical Waiting key has no shared cursor index",
        ))?;
        if index >= WakeupCursorLen::<T>::get(key.clock())
          || Self::wakeup_cursor_get(key.clock(), index) != Some(key)
        {
          return Err(TryRuntimeError::Other(
            "canonical Waiting cursor reverse index does not resolve",
          ));
        }
      }
      if visited_pages.len() != waiting_pages.len() || directory_keys.len() != waiting_live.len() {
        return Err(TryRuntimeError::Other(
          "canonical Waiting pages are disconnected or have no directory",
        ));
      }
      for key in ActorWaitingHeads::<T>::iter_keys()
        .chain(ActorWaitingTails::<T>::iter_keys())
        .chain(ActorWaitingCursorIndices::<T>::iter_keys())
      {
        if !directory_keys.contains(&key) {
          return Err(TryRuntimeError::Other(
            "canonical Waiting directory contains an orphan key",
          ));
        }
      }
      for clock in [WakeupClock::Block, WakeupClock::Tick] {
        if directory_keys
          .iter(/* deos-bypass: bounded-iter -- TryRuntime-only topology audit. */)
          .filter(|key| key.clock() == clock)
          .count() as u64
          != u64::from(WakeupCursorLen::<T>::get(clock))
        {
          return Err(TryRuntimeError::Other(
            "canonical Waiting directory and heap cardinality disagree",
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
          if Self::wakeup_cursor_owner_index(key) != Some(index) {
            return Err(TryRuntimeError::Other(
              "WakeupCursor key has no matching owner reverse index",
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
      let wakeup_control_entries = Self::control_hot_entries_for_try_state().ok_or(
        TryRuntimeError::Other("ActorControl frame topology is corrupt"),
      )?;
      for (actor_id, hot) in wakeup_control_entries {
        if let Some(pointer) = hot.wakeup_pointer {
          if !live_wakeup_pointers.insert((pointer.block, pointer.page_id, pointer.slot)) {
            return Err(TryRuntimeError::Other(
              "multiple actors own the same wakeup pointer",
            ));
          }
          if !Self::wakeup_page_entry_matches(pointer, actor_id) {
            return Err(TryRuntimeError::Other(
              "ActorControl Pipeline wakeup pointer does not resolve to its actor",
            ));
          }
          if let Some(terminal_at) = hot.terminal_at
            && !matches!(pointer.block, WakeupKey::Block(block) if block <= terminal_at)
          {
            return Err(TryRuntimeError::Other(
              "ActorControl Pipeline wakeup pointer exceeds its terminal membership",
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
              "ActorControl Trigger wakeup pointer does not resolve to its actor",
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
      let mut owner_hold_totals = alloc::collections::BTreeMap::new();
      for (actor_id, identity) in &semantic_identities {
        match identity.actor_class.actor_type() {
          ActorType::System => {
            if ActorStateHolds::<T>::contains_key(actor_id) {
              return Err(TryRuntimeError::Other(
                "System Actor retains a User Actor-state hold",
              ));
            }
          }
          ActorType::User => {
            let expected = if ActorControlLocators::<T>::contains_key(actor_id) {
              let (_, frame_identity, hot, admission) =
                Self::load_frame_control_authority(*actor_id).ok_or(TryRuntimeError::Other(
                  "User Actor frame authority cannot be loaded for state-hold derivation",
                ))?;
              if frame_identity != *identity {
                return Err(TryRuntimeError::Other(
                  "User Actor frame identity disagrees with semantic state-hold identity",
                ));
              }
              Self::derive_actor_state_hold_with_authority(
                *actor_id,
                identity,
                Some((&hot, &admission)),
              )
            } else {
              Self::derive_actor_state_hold_with_authority(*actor_id, identity, None)
            }
            .map_err(|_| {
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
        if !semantic_identities.contains_key(&actor_id) {
          return Err(TryRuntimeError::Other(
            "Actor-state hold record has no semantic Actor identity",
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
