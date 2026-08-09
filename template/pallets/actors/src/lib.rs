#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use polkadot_sdk::{
  frame_support::{BoundedVec, traits::Get},
  sp_runtime::traits::{CheckedAdd, CheckedSub, Zero},
};

pub use pallet::*;

pub mod contract;
pub mod types;

mod execution;
mod reactions;
mod scheduler;
mod subscriptions;

pub use scheduler::EnqueueOutcome;

pub mod adapters;
pub use adapters::{
  AddressEventIngress, AssetOps, DexOps, ExecutionContext, FundingAuthority, IngressFailure,
  LiquidityOps, ObservationChangeIngress, ObservationProvider, RetryClass, ScalarObservationState,
  StakingOps, TaskFailure,
};
pub use types::{AddressEvent, InputLimit, Task, WakeupBucketState, WakeupCursorIndex};

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
  fn setup_condition_assets(
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

pub(crate) const MAX_EXECUTION_PLAN_STEPS_HARD_LIMIT: u32 = u8::MAX as u32;

pub(crate) const fn execution_plan_steps_bound_is_valid(bound: u32) -> bool {
  bound > 0 && bound <= MAX_EXECUTION_PLAN_STEPS_HARD_LIMIT
}

sp_api::decl_runtime_apis! {
  pub trait ActorSimulationApi<Program, Simulation>
  where
    Program: codec::Codec,
    Simulation: codec::Codec,
  {
    fn simulate_current_program(
      actor_id: types::ActorId,
      expected_type: types::ActorType,
      expected_mutability: types::Mutability,
      expected_program: Program,
      mode: types::SimulationMode,
    ) -> Result<Simulation, types::SimulationError>;
  }

  /// Read-only eligibility projection for one actor (spec 7.3).
  ///
  /// Returns the scheduler's current readiness verdict and the next block at
  /// which temporal eligibility opens, reusing the same pure owners as
  /// admission so clients do not reimplement cadence phase, cooldown, window
  /// floor, retry backoff, breaker, or latch arithmetic.
  pub trait ActorEligibilityApi<BlockNumber>
  where
    BlockNumber: codec::Codec,
  {
    fn actor_eligibility(
      actor_id: types::ActorId,
    ) -> Result<types::ActorEligibilityProjection<BlockNumber>, types::ActorEligibilityError>;
  }
}

#[frame::pallet]
pub mod pallet {
  use super::{
    AssetOps, AttemptFeeEnvelope, DexOps, FeeCollector, FeeEnvelopeError, FeeEnvelopeInput,
    FundingAuthority, LiquidityOps, ObservationProvider, WeightInfo, compose_attempt_fee_envelope,
    execution_plan_steps_bound_is_valid,
  };
  use crate::adapters::{RetryClass, SovereignAccountPolicy, StakingOps as _};
  use frame::prelude::*;
  use polkadot_sdk::{
    frame_support::{PalletId, traits::EnsureOrigin},
    sp_runtime::traits::{CheckedAdd, One, SaturatedConversion, Saturating, Zero},
    sp_weights::{WeightMeter, WeightToFee as _},
  };

  use super::types::Task as ActorTask;
  pub use super::types::*;

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
    type ObservationFeedId: Parameter + Member + Copy + MaxEncodedLen + Ord;
    type ObservationProvider: ObservationProvider<Self::ObservationFeedId, BlockNumberFor<Self>>;
    type FundingAuthority: FundingAuthority<Self::AccountId>;
    type SovereignAccountPolicy: crate::adapters::SovereignAccountPolicy<Self::AccountId>;
    type DexOps: DexOps<Self::AccountId, Self::AssetId, Self::Balance>;
    type StakingOps: crate::adapters::StakingOps<Self::AccountId, Self::AssetId, Self::Balance>;
    type LiquidityOps: LiquidityOps<Self::AccountId, Self::AssetId, Self::Balance>;

    #[pallet::constant]
    type MinWindowLength: Get<BlockNumberFor<Self>>;
    #[pallet::constant]
    type PalletId: Get<PalletId>;

    type SystemOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    type GlobalBreakerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

    #[pallet::constant]
    type MaxExecutionPlanSteps: Get<u32>;
    #[pallet::constant]
    type MaxFundingTrackedAssets: Get<u32>;
    #[pallet::constant]
    type MaxOpeningSnapshotEntries: Get<u32>;
    #[pallet::constant]
    type MaxConditionsPerStep: Get<u32>;
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
    type MaxTriggerSources: Get<u32>;
    #[pallet::constant]
    type MaxSplitTransferLegs: Get<u32>;
    /// Target block duration in whole seconds.
    #[pallet::constant]
    type TargetBlockTime: Get<u64>;
    #[pallet::constant]
    type MaxExecutionDelayBlocks: Get<BlockNumberFor<Self>>;
    #[pallet::constant]
    type MaxTimerJitterBlocks: Get<u32>;
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

    /// Provides System Actors specs to initialize at genesis.
    /// Use `()` for no genesis System Actors (default).
    type GenesisSystemActors: GenesisSystemActors<
        Self::AccountId,
        ScheduleOf<Self>,
        ScheduleWindow<BlockNumberFor<Self>>,
        ExecutionPlanOf<Self>,
      >;

    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper: crate::BenchmarkHelper<Self::AccountId, Self::AssetId, Self::Balance, Self::ObservationFeedId>;
  }

  pub type BalanceOf<T> = <T as Config>::Balance;
  pub type AssetIdOf<T> = <T as Config>::AssetId;

  pub type SourceFilterOf<T> =
    SourceFilter<<T as frame_system::Config>::AccountId, <T as Config>::MaxWhitelistSize>;

  pub type AssetFilterOf<T> = AssetFilter<<T as Config>::AssetId, <T as Config>::MaxWhitelistSize>;

  pub type ActorObservationFeedsOf<T> =
    BoundedVec<<T as Config>::ObservationFeedId, <T as Config>::MaxTriggerSources>;
  pub type SimulationResultOf<T> = SimulationResult<<T as Config>::MaxExecutionPlanSteps>;
  pub type ActorEligibilityProjectionOf<T> =
    ActorEligibilityProjection<frame::prelude::BlockNumberFor<T>>;
  pub type ObservationSubscriberPageOf<T> =
    ObservationSubscriberPage<<T as Config>::ObservationPageSize>;
  pub type ObservationFreeSlotPageOf<T> = BoundedVec<u32, <T as Config>::ObservationPageSize>;

  pub type TriggerSourceOf<T> = TriggerSource<
    <T as frame_system::Config>::AccountId,
    <T as Config>::AssetId,
    <T as Config>::MaxWhitelistSize,
    <T as Config>::ObservationFeedId,
  >;

  pub type TriggerPolicyOf<T> = TriggerPolicy<
    <T as frame_system::Config>::AccountId,
    <T as Config>::AssetId,
    <T as Config>::MaxWhitelistSize,
    <T as Config>::MaxTriggerSources,
    <T as Config>::ObservationFeedId,
  >;

  pub type TriggerOf<T> = Trigger<
    <T as frame_system::Config>::AccountId,
    <T as Config>::AssetId,
    <T as Config>::MaxWhitelistSize,
    <T as Config>::MaxTriggerSources,
    <T as Config>::ObservationFeedId,
  >;

  pub type ScheduleOf<T> = Schedule<
    <T as frame_system::Config>::AccountId,
    <T as Config>::AssetId,
    <T as Config>::MaxWhitelistSize,
    <T as Config>::MaxTriggerSources,
    <T as Config>::ObservationFeedId,
  >;

  pub type ConditionSetOf<T> = ConditionSet<
    Condition<
      <T as Config>::AssetId,
      <T as Config>::Balance,
      u32,
      <T as Config>::ObservationFeedId,
    >,
    <T as Config>::MaxConditionsPerStep,
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
    <T as Config>::MaxConditionsPerStep,
    <T as Config>::MaxSplitTransferLegs,
    <T as Config>::ObservationFeedId,
  >;

  pub type ExecutionPlanOf<T> = BoundedVec<StepOf<T>, <T as Config>::MaxExecutionPlanSteps>;

  pub type AttemptFeeEnvelopeOf<T> =
    AttemptFeeEnvelope<BalanceOf<T>, <T as Config>::MaxExecutionPlanSteps>;

  pub type FundingSourcePolicyOf<T> =
    FundingSourcePolicy<<T as frame_system::Config>::AccountId, <T as Config>::MaxWhitelistSize>;

  pub type ActiveProgramInputOf<T> = ActiveProgramInput<
    ScheduleOf<T>,
    BlockNumberFor<T>,
    ExecutionPlanOf<T>,
    FundingSourcePolicyOf<T>,
  >;

  pub type ProgramInputOf<T> =
    ProgramInput<ScheduleOf<T>, BlockNumberFor<T>, ExecutionPlanOf<T>, FundingSourcePolicyOf<T>>;

  pub type FundingAccumulatedOf<T> = BoundedBTreeMap<
    <T as Config>::AssetId,
    <T as Config>::Balance,
    <T as Config>::MaxFundingTrackedAssets,
  >;

  pub type FundingTrackedAssetsOf<T> =
    BoundedBTreeSet<<T as Config>::AssetId, <T as Config>::MaxFundingTrackedAssets>;

  pub type FundingSnapshotOf<T> = FundingAccumulatedOf<T>;

  pub type ContinuationSnapshotOf<T> = BoundedBTreeMap<
    OpeningSurface<<T as Config>::AssetId>,
    <T as Config>::Balance,
    <T as Config>::MaxOpeningSnapshotEntries,
  >;

  pub type ContinuationStateOf<T> = ContinuationState<
    <T as Config>::AssetId,
    <T as Config>::Balance,
    BlockNumberFor<T>,
    <T as Config>::MaxOpeningSnapshotEntries,
    <T as Config>::MaxFundingTrackedAssets,
  >;

  pub type QueuePageOf<T> = BoundedVec<QueueEntry, <T as Config>::QueuePageSize>;
  pub type WakeupPageEntriesOf<T> = BoundedVec<Option<WakeupEntry>, <T as Config>::WakeupPageSize>;
  pub type WakeupPageOf<T> = WakeupPage<WakeupPageEntriesOf<T>>;
  pub type WakeupCursorPageOf<T> = BoundedVec<BlockNumberFor<T>, <T as Config>::WakeupPageSize>;

  pub type ActiveActorViewOf<T> = ActiveActorView<
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T>,
    ScheduleOf<T>,
    ExecutionPlanOf<T>,
  >;

  pub type ActorHotStateOf<T> = ActorHotState<BlockNumberFor<T>>;

  pub type ActorProgramStateOf<T> =
    ActorProgramState<ScheduleOf<T>, BlockNumberFor<T>, ExecutionPlanOf<T>>;

  pub type ActorFundingStateOf<T> =
    ActorFundingState<FundingSourcePolicyOf<T>, FundingAccumulatedOf<T>, FundingTrackedAssetsOf<T>>;

  pub type ActorIdentityOf<T> =
    ActorIdentity<<T as frame_system::Config>::AccountId, BlockNumberFor<T>>;

  #[pallet::pallet]
  #[pallet::storage_version(STORAGE_VERSION)]
  pub struct Pallet<T>(_);

  const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

  #[pallet::storage]
  #[pallet::getter(fn next_actor_id)]
  pub type NextActorId<T> = StorageValue<_, ActorId, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn actor_hot)]
  pub type ActorHot<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorHotStateOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn actor_program)]
  pub type ActorProgram<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorProgramStateOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn actor_funding)]
  pub type ActorFunding<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ActorFundingStateOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::storage_prefix = "ContinuationState"]
  #[pallet::getter(fn continuation_state)]
  pub type ContinuationStateStore<T: Config> =
    StorageMap<_, Blake2_128Concat, ActorId, ContinuationStateOf<T>, OptionQuery>;

  impl<T: Config> Pallet<T> {
    pub(crate) fn derive_active_actor_view(
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      program: ActorProgramStateOf<T>,
    ) -> ActiveActorViewOf<T> {
      ActiveActorView {
        sovereign_account: identity.sovereign_account,
        owner: identity.owner,
        actor_class: identity.actor_class,
        mutability: identity.mutability,
        lifecycle: hot.lifecycle,
        cycle_state: hot.cycle_state,
        schedule: program.schedule,
        schedule_window: program.schedule_window,
        execution_plan: program.execution_plan,
        completion_policy: program.completion_policy,
        cycle_nonce: identity.cycle_nonce,
        auto_close_at_cycle_nonce: hot.auto_close_at_cycle_nonce,
        consecutive_failures: hot.consecutive_failures,
        pending_signal: hot.pending_signal,
        queue_ticket: hot.queue_ticket,
        last_control_mutation_block: identity.last_control_mutation_block,
        schedule_anchor: hot.schedule_anchor,
        last_cycle_block: hot.last_cycle_block,
      }
    }

    pub fn active_actor_view(actor_id: ActorId) -> Option<ActiveActorViewOf<T>> {
      Some(Self::derive_active_actor_view(
        ActorIdentities::<T>::get(actor_id)?,
        ActorHot::<T>::get(actor_id)?,
        ActorProgram::<T>::get(actor_id)?,
      ))
    }

    pub fn pending_signal(actor_id: ActorId) -> bool {
      ActorHot::<T>::get(actor_id).is_some_and(|hot| hot.pending_signal)
    }

    pub(crate) fn active_actor_exists(actor_id: ActorId) -> bool {
      ActorIdentities::<T>::contains_key(actor_id)
        && ActorHot::<T>::contains_key(actor_id)
        && ActorProgram::<T>::contains_key(actor_id)
    }

    pub(crate) fn insert_active_actor(
      actor_id: ActorId,
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      program: ActorProgramStateOf<T>,
    ) -> DispatchResult {
      Self::replace_observation_subscriptions(actor_id, &program.schedule)?;
      ActorIdentities::<T>::insert(actor_id, identity);
      ActorHot::<T>::insert(actor_id, hot);
      ActorProgram::<T>::insert(actor_id, program);
      Ok(())
    }

    pub(crate) fn remove_active_actor(actor_id: ActorId) -> DispatchResult {
      Self::remove_observation_subscriptions(actor_id)?;
      ActorHot::<T>::remove(actor_id);
      ActorProgram::<T>::remove(actor_id);
      ContinuationStateStore::<T>::remove(actor_id);
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

  /// Next never-used ticket and current-block cutoff source for the canonical FIFO.
  #[pallet::storage]
  #[pallet::getter(fn next_queue_ticket)]
  pub type NextQueueTicket<T> = StorageValue<_, QueueTicket, ValueQuery>;

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

  /// Fixed-size pages for the next temporal wakeup substrate.
  #[pallet::storage]
  #[pallet::getter(fn wakeup_pages)]
  pub type WakeupPages<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    (BlockNumberFor<T>, WakeupPageId),
    WakeupPageOf<T>,
    OptionQuery,
  >;

  /// Small per-block ownership and allocation metadata for temporal pages.
  #[pallet::storage]
  #[pallet::getter(fn wakeup_buckets)]
  pub type WakeupBuckets<T: Config> =
    StorageMap<_, Blake2_128Concat, BlockNumberFor<T>, WakeupBucketState, OptionQuery>;

  /// Paged binary min-heap of distinct wakeup blocks for sparse due discovery.
  #[pallet::storage]
  #[pallet::getter(fn wakeup_cursor_pages)]
  pub type WakeupCursorPages<T: Config> =
    StorageMap<_, Blake2_128Concat, WakeupPageId, WakeupCursorPageOf<T>, OptionQuery>;

  /// Logical length of the paged sparse-wakeup cursor heap.
  #[pallet::storage]
  #[pallet::getter(fn wakeup_cursor_len)]
  pub type WakeupCursorLen<T> = StorageValue<_, WakeupCursorIndex, ValueQuery>;

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
  #[pallet::getter(fn global_circuit_breaker)]
  pub type GlobalCircuitBreaker<T> = StorageValue<_, bool, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn idle_starvation_state)]
  pub type IdleStarvationState<T: Config> = StorageValue<_, IdleStarvationPhase, ValueQuery>;

  /// Provides runtime-specific System Actors instances to initialize at genesis.
  ///
  /// Implement this on the runtime to return System Actors specs with explicit `actor_id` values.
  /// IDs may be sparse to reserve stable addresses for non-actor accounts.
  pub trait GenesisSystemActors<AccountId, Schedule, ScheduleWindow, ExecutionPlan> {
    fn system_actors() -> alloc::vec::Vec<(
      ActorId,
      AccountId,
      Mutability,
      Schedule,
      Option<ScheduleWindow>,
      ExecutionPlan,
      CompletionPolicy,
    )>;

    fn dormant_system_actors() -> alloc::vec::Vec<(ActorId, AccountId)> {
      alloc::vec::Vec::new()
    }

    /// Runtime-declared deterministic custody accounts that need a provider at genesis
    /// but own no generic Actors identity, program, or scheduler state.
    fn system_custody_accounts() -> alloc::vec::Vec<ActorId> {
      alloc::vec::Vec::new()
    }
  }

  /// Default no-op implementation: no System Actors created at genesis.
  impl<AccountId, Schedule, ScheduleWindowT, ExecutionPlan>
    GenesisSystemActors<AccountId, Schedule, ScheduleWindowT, ExecutionPlan> for ()
  {
    fn system_actors() -> alloc::vec::Vec<(
      ActorId,
      AccountId,
      Mutability,
      Schedule,
      Option<ScheduleWindowT>,
      ExecutionPlan,
      CompletionPolicy,
    )> {
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
        execution_plan_steps_bound_is_valid(T::MaxExecutionPlanSteps::get()),
        "MaxExecutionPlanSteps must be in 1..=255"
      );
      STORAGE_VERSION.put::<Pallet<T>>();
      if ActiveActorLimit::<T>::get() == 0 {
        ActiveActorLimit::<T>::put(Pallet::<T>::max_configurable_active_actor_limit());
      }
      for (
        actor_id,
        owner,
        mutability,
        schedule,
        schedule_window,
        execution_plan,
        completion_policy,
      ) in T::GenesisSystemActors::system_actors()
      {
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
          mutability == Mutability::Mutable || !schedule.trigger.manual_source_enabled(),
          "genesis System Immutable Actors cannot admit Manual readiness"
        );
        Pallet::<T>::validate_execution_plan_shape(ActorType::System, &execution_plan)
          .expect("genesis execution plan must have valid task and condition shapes");
        Pallet::<T>::validate_recipient_configuration(&execution_plan, &sovereign_account)
          .expect("genesis execution plan cannot transfer to its own sovereign account");
        Pallet::<T>::validate_opening_snapshot_surfaces(&execution_plan)
          .expect("genesis opening snapshot surfaces must be valid");
        Pallet::<T>::ensure_retry_later_allowed(mutability, &execution_plan)
          .expect("genesis System Immutable Actors cannot use RetryLater");
        Pallet::<T>::ensure_execution_plan_fits_idle_budget(ActorType::System, &execution_plan)
          .unwrap_or_else(|_| {
            panic!("genesis System Actors {actor_id} exceeds the guaranteed on_idle budget")
          });
        let funding_tracked_assets = Pallet::<T>::derive_funding_tracked_assets(&execution_plan)
          .expect("genesis execution_plan must have valid funding-tracked assets");
        let schedule_anchor = Pallet::<T>::schedule_anchor_at(schedule_window, Zero::zero());
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
        let hot = ActorHotState {
          lifecycle: ActiveLifecycle::Active,
          cycle_state: CycleState::Idle,
          auto_close_at_cycle_nonce: None,
          consecutive_failures: 0,
          pending_signal: false,
          queue_ticket: None,
          wakeup_pointer: None,
          terminal_at: schedule_window.map(|window| Pallet::<T>::window_terminal_at(&window)),
          schedule_anchor,
          last_cycle_block: None,
        };
        let program = ActorProgramState {
          schedule,
          schedule_window,
          execution_plan,
          completion_policy,
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
        SystemSovereignCount::<T>::mutate(|count| *count += 1);
        SovereignIndex::<T>::insert(&sovereign_account, actor_id);
        frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        Pallet::<T>::insert_active_actor(actor_id, identity, hot, program)
          .unwrap_or_else(|error| panic!("genesis observation subscription failed: {error:?}"));
        ActorFunding::<T>::insert(
          actor_id,
          ActorFundingState {
            funding_source_policy: FundingSourcePolicy::RuntimePolicy,
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
        SystemSovereignCount::<T>::mutate(|count| *count += 1);
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
        SystemSovereignCount::<T>::mutate(|count| *count += 1);
        frame_system::Pallet::<T>::inc_providers(&sovereign_account);
      }
    }
  }

  #[pallet::hooks]
  impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    fn integrity_test() {
      assert!(
        T::MaxConsecutiveFailures::get() > 0,
        "MaxConsecutiveFailures must be non-zero for bounded Continuation lifetime"
      );
      assert!(
        execution_plan_steps_bound_is_valid(T::MaxExecutionPlanSteps::get()),
        "MaxExecutionPlanSteps must be in 1..=255"
      );
      assert_eq!(
        T::MaxRetryAttempts::get(),
        10,
        "MaxRetryAttempts must equal the protocol-fixed bound"
      );
      assert!(
        T::MaxExecutionPlanSteps::get()
          .checked_mul(T::MaxRetryAttempts::get())
          .is_some(),
        "plan and retry bounds must compose without u32 overflow"
      );
      let target_block_time = T::TargetBlockTime::get();
      assert!(target_block_time > 0, "TargetBlockTime must be non-zero");
      let expected_horizon = 315_576_000u64.div_ceil(target_block_time);
      let configured_horizon: u64 = T::MaxExecutionDelayBlocks::get().saturated_into();
      assert_eq!(
        configured_horizon, expected_horizon,
        "MaxExecutionDelayBlocks must cover exactly ten Julian years"
      );
      assert_eq!(
        T::MaxOpeningSnapshotEntries::get(),
        T::MaxExecutionPlanSteps::get()
          .checked_mul(2)
          .expect("validated plan bound fits u32"),
        "MaxOpeningSnapshotEntries must equal twice MaxExecutionPlanSteps"
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
      let wakeup_limit = T::WakeupWeightLimit::get();
      assert!(
        wakeup_limit.ref_time() > 0 && wakeup_limit.proof_size() > 0,
        "wakeup worker Weight limit must be non-zero in both dimensions"
      );
      let actor_service = Self::guaranteed_actor_service_weight()
        .expect("configured housekeeping Weight must fit ActorOnIdleReserve");
      assert!(
        Self::close_cleanup_weight_upper().all_lte(actor_service),
        "one maximum automatic cleanup must fit GuaranteedActorServiceWeight"
      );
    }

    #[cfg(feature = "try-runtime")]
    fn try_state(_n: BlockNumberFor<T>) -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      Self::do_try_state()
    }

    fn on_idle(now: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
      let reserved = T::ActorOnIdleReserve::get();
      let available = Weight::from_parts(
        remaining_weight.ref_time().min(reserved.ref_time()),
        remaining_weight.proof_size().min(reserved.proof_size()),
      );
      let base_weight = T::WeightInfo::scheduler_on_idle_base();
      if !base_weight.all_lte(available) {
        return Weight::zero();
      }
      let breaker_active = GlobalCircuitBreaker::<T>::get();
      let after_base = available.saturating_sub(base_weight);
      let cleanup_units = u32::from(QueueHead::<T>::get() < QueueTail::<T>::get());
      let queue_cleanup_weight = T::WeightInfo::scheduler_paged_tombstone_drain(cleanup_units);
      let saturated_cleanup_weight = if cleanup_units > 0
        && Self::combined_queue_occupancy() >= u64::from(T::MaxQueueLength::get())
        && queue_cleanup_weight.all_lte(after_base)
      {
        let cutoff = NextQueueTicket::<T>::get();
        match Self::paged_drain_tombstones(cutoff, 1) {
          Ok(queue) if queue.entries_scanned > 0 => queue_cleanup_weight,
          Ok(_) => Weight::zero(),
          Err(_) => queue_cleanup_weight,
        }
      } else {
        Weight::zero()
      };
      let remaining_after_cleanup = after_base.saturating_sub(saturated_cleanup_weight);
      // Phase 2: due wakeups and bounded lazy physical cleanup before fanout (spec 8.2.1).
      // The worker is bounded component-wise by both its configured ceiling and the actual
      // on_idle budget left after base work and saturated queue cleanup.
      let configured_wakeup_limit = T::WakeupWeightLimit::get();
      let wakeup_limit = Weight::from_parts(
        configured_wakeup_limit
          .ref_time()
          .min(remaining_after_cleanup.ref_time()),
        configured_wakeup_limit
          .proof_size()
          .min(remaining_after_cleanup.proof_size()),
      );
      let mut wakeup_meter = WeightMeter::with_limit(wakeup_limit);
      Self::drain_overdue_wakeups_cursor(now, &mut wakeup_meter);
      let wakeup_weight = wakeup_meter.consumed();
      let remaining_after_wakeups = remaining_after_cleanup.saturating_sub(wakeup_weight);
      // Phase 3: observation fanout under ObservationFanoutWeightLimit, after wakeups and
      // before the cutoff/actor-execution pass.
      let fanout_weight = if DirtyObservationListState::<T>::get().count > 0 {
        Self::fanout_dirty_observations(remaining_after_wakeups)
      } else {
        Weight::zero()
      };
      let remaining_after_housekeeping = remaining_after_wakeups.saturating_sub(fanout_weight);
      let housekeeping_weight = base_weight
        .saturating_add(saturated_cleanup_weight)
        .saturating_add(wakeup_weight)
        .saturating_add(fanout_weight);
      if breaker_active {
        return housekeeping_weight;
      }
      let pass = Self::execute_cycle(remaining_after_housekeeping);
      Self::update_idle_starvation_state(now, pass.starved);
      housekeeping_weight.saturating_add(pass.consumed)
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
      attempt: u32,
      cursor: u32,
      reason: SuspensionReason,
      cumulative_outcomes: OutcomeTotals,
    },
    CycleContinued {
      actor_id: ActorId,
      cycle_nonce: u64,
      attempt: u32,
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
    ScheduleUpdated {
      actor_id: ActorId,
    },
    ExecutionPlanUpdated {
      actor_id: ActorId,
      completion_policy: CompletionPolicy,
    },
    FundingSourcePolicyUpdated {
      actor_id: ActorId,
    },
    AutoCloseNonceSet {
      actor_id: ActorId,
      target: Option<u64>,
    },
    AutoCloseNonceIncremented {
      actor_id: ActorId,
      old_target: Option<u64>,
      new_target: u64,
      by: u64,
    },
    ActiveActorLimitSet {
      old_limit: u32,
      new_limit: u32,
    },
    GlobalCircuitBreakerSet {
      paused: bool,
    },
    ManualTriggerSet {
      actor_id: ActorId,
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
    EmptyExecutionPlan,
    ExecutionPlanExceedsOnIdleBudget,
    ExecutionDelayTooLong,
    GlobalCircuitBreakerActive,
    ImmutableActor,
    InsufficientBalance,
    InsufficientFee,
    InvalidAmountResolution,
    InvalidCondition,
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
    ExecutionPlanTooLong,
    SnapshotUnavailable,
    FundingAccumulatorOverflow,
    QueueTicketExhausted,
    SchedulerIndexExhausted,
    AutoCloseNonceHorizonExceeded,
    AutoCloseNonceOverflow,
    AutoCloseNonceIncrementZero,
    ControlMutationRateLimited,
    QueueCapacityUnavailable,
    RetryLaterNotAllowedForImmutableActor,
    ContinuationNotFound,
    ContinuationInvariant,
    ComputationOverflow,
    EmptyConditionSet,
    ManualSourceDisabled,
    RecipientDepositUnavailable,
    ObservationSubscriptionCapacityExceeded,
    ObservationSubscriptionInvariant,
    InvalidObservationRevision,
    DirtyObservationCapacityExceeded,
    DirtyObservationInvariant,
    AdmissionBoundOverflow,
  }

  #[pallet::call]
  impl<T: Config> Pallet<T> {
    #[pallet::call_index(0)]
    #[pallet::weight(T::WeightInfo::create_user_actor())]
    pub fn create_user_actor(
      origin: OriginFor<T>,
      mutability: Mutability,
      program: ProgramInputOf<T>,
    ) -> DispatchResult {
      let owner = ensure_signed(origin)?;
      Self::do_create_user_actor(owner, mutability, None, program)
    }

    #[pallet::call_index(1)]
    #[pallet::weight(T::WeightInfo::create_user_actor_at_slot())]
    pub fn create_user_actor_at_slot(
      origin: OriginFor<T>,
      owner_slot: u8,
      mutability: Mutability,
      program: ProgramInputOf<T>,
    ) -> DispatchResult {
      let owner = ensure_signed(origin)?;
      Self::do_create_user_actor(owner, mutability, Some(owner_slot), program)
    }

    #[pallet::call_index(2)]
    #[pallet::weight(match &program {
      ProgramInput::Dormant => T::WeightInfo::create_dormant_system_actor(),
      ProgramInput::Active(_) => T::WeightInfo::create_system_actor(),
    })]
    pub fn create_system_actor(
      origin: OriginFor<T>,
      owner: T::AccountId,
      mutability: Mutability,
      program: ProgramInputOf<T>,
    ) -> DispatchResult {
      T::SystemOrigin::ensure_origin(origin)?;
      Self::do_create_system_actor(owner, mutability, program, None)
    }

    #[pallet::call_index(3)]
    #[pallet::weight(T::WeightInfo::create_system_actor_at_sovereign_id())]
    pub fn create_system_actor_at_sovereign_id(
      origin: OriginFor<T>,
      sovereign_id: SystemSovereignId,
      owner: T::AccountId,
      mutability: Mutability,
      program: ProgramInputOf<T>,
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
      Self::do_create_system_actor(owner, mutability, program, Some(sovereign_id))
    }

    #[pallet::call_index(4)]
    #[pallet::weight(T::WeightInfo::pause_actor().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn pause_actor(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      let snapshot = Self::active_actor_view(actor_id).ok_or(Error::<T>::ActorNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::expiry_substitution_due(actor_id, &snapshot)? {
        return Self::finalize_actor(actor_id, &snapshot, CloseReason::WindowExpired);
      }
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
          Self::record_control_mutation(actor_id, now);
          Self::deposit_event(Event::ActorPaused { actor_id });
          Ok(())
        })?;
        Self::prime_actor_schedule(actor_id).map_err(Self::placement_error)
      })
    }

    #[pallet::call_index(5)]
    #[pallet::weight(T::WeightInfo::resume_actor().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn resume_actor(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      let snapshot = Self::active_actor_view(actor_id).ok_or(Error::<T>::ActorNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::expiry_substitution_due(actor_id, &snapshot)? {
        return Self::finalize_actor(actor_id, &snapshot, CloseReason::WindowExpired);
      }
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
          Self::record_control_mutation(actor_id, now);
          Self::deposit_event(Event::ActorResumed { actor_id });
          Ok(())
        })?;
        Self::prime_actor_schedule(actor_id).map_err(Self::placement_error)
      })
    }

    #[pallet::call_index(6)]
    #[pallet::weight(T::WeightInfo::manual_trigger().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn manual_trigger(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      let snapshot = Self::active_actor_view(actor_id).ok_or(Error::<T>::ActorNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::expiry_substitution_due(actor_id, &snapshot)? {
        return Self::finalize_actor(actor_id, &snapshot, CloseReason::WindowExpired);
      }
      ensure!(!snapshot.lifecycle.is_paused(), Error::<T>::ActorPaused);
      ensure!(
        snapshot.schedule.trigger.manual_source_enabled(),
        Error::<T>::ManualSourceDisabled
      );
      Self::with_control_transaction(|| {
        if !snapshot.pending_signal {
          ActorHot::<T>::try_mutate(actor_id, |maybe| -> DispatchResult {
            let hot = maybe.as_mut().ok_or(Error::<T>::ActorNotFound)?;
            hot.pending_signal = true;
            Ok(())
          })?;
          Self::deposit_event(Event::ManualTriggerSet { actor_id });
        }
        Self::prime_actor_schedule(actor_id).map_err(Self::placement_error)
      })
    }

    #[pallet::call_index(7)]
    #[pallet::weight(
      T::WeightInfo::update_funding_source_policy()
        .saturating_add(Pallet::<T>::close_dispatch_weight_upper())
    )]
    pub fn update_funding_source_policy(
      origin: OriginFor<T>,
      actor_id: ActorId,
      policy: FundingSourcePolicyOf<T>,
    ) -> DispatchResult {
      let instance = Self::active_actor_view(actor_id).ok_or(Error::<T>::ActorNotFound)?;
      Self::ensure_control_origin(origin, &instance)?;
      Self::ensure_not_system_immutable(&instance)?;
      if Self::expiry_substitution_due(actor_id, &instance)? {
        return Self::finalize_actor(actor_id, &instance, CloseReason::WindowExpired);
      }
      ensure!(
        instance.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      let current_funding = ActorFunding::<T>::get(actor_id).ok_or(Error::<T>::ActorNotFound)?;
      if current_funding.funding_source_policy == policy {
        return Ok(());
      }
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_mutation_allowed(&instance, now)?;
      Self::with_control_transaction(|| {
        let continuation_cancelled = Self::cancel_continuation_internal(
          actor_id,
          CancellationReason::FundingPolicyChanged,
          None,
        )?;
        ActorFunding::<T>::mutate(actor_id, |maybe| {
          maybe
            .as_mut()
            .expect("active actor funding existence was prevalidated")
            .funding_source_policy = policy;
        });
        Self::record_control_mutation(actor_id, now);
        Self::deposit_event(Event::FundingSourcePolicyUpdated { actor_id });
        if continuation_cancelled {
          Self::prime_actor_schedule(actor_id).map_err(Self::placement_error)?;
        }
        Ok(())
      })
    }

    #[pallet::call_index(8)]
    #[pallet::weight(Pallet::<T>::close_dispatch_weight_upper())]
    pub fn close_actor(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      if let Some(instance) = Self::active_actor_view(actor_id) {
        Self::ensure_control_origin(origin, &instance)?;
        Self::ensure_not_system_immutable(&instance)?;
        return Self::finalize_actor(actor_id, &instance, CloseReason::OwnerInitiated);
      }
      let identity = ActorIdentities::<T>::get(actor_id).ok_or(Error::<T>::ActorNotFound)?;
      Self::ensure_identity_control_origin(origin, &identity)?;
      Self::close_inactive_actor(actor_id, &identity, CloseReason::OwnerInitiated)
    }

    #[pallet::call_index(9)]
    #[pallet::weight(T::WeightInfo::update_schedule().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn update_schedule(
      origin: OriginFor<T>,
      actor_id: ActorId,
      schedule: ScheduleOf<T>,
      schedule_window: Option<ScheduleWindow<BlockNumberFor<T>>>,
    ) -> DispatchResult {
      Self::validate_schedule(&schedule)?;
      if let Some(ref window) = schedule_window {
        Self::validate_schedule_window(window)?;
      }
      Self::validate_future_schedule_targets(actor_id, &schedule, schedule_window)?;
      let snapshot = Self::active_actor_view(actor_id).ok_or(Error::<T>::ActorNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      Self::validate_opening_snapshot_surfaces(&snapshot.execution_plan)?;
      if Self::expiry_substitution_due(actor_id, &snapshot)? {
        return Self::finalize_actor(actor_id, &snapshot, CloseReason::WindowExpired);
      }
      ensure!(
        snapshot.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      if snapshot.schedule == schedule && snapshot.schedule_window == schedule_window {
        return Ok(());
      }
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_mutation_allowed(&snapshot, now)?;
      // Semantic schedule replacement resets the Active-epoch anchor unconditionally
      // (spec 4.3); the exact no-op path above already returned without mutation.
      let schedule_anchor = Self::schedule_anchor_at(schedule_window, now);
      Self::preflight_observation_subscription_replace(actor_id, &schedule)?;
      Self::with_control_transaction(|| {
        Self::cancel_continuation_internal(actor_id, CancellationReason::ScheduleChanged, None)?;
        Self::replace_observation_subscriptions(actor_id, &schedule)?;
        ActorProgram::<T>::mutate(actor_id, |maybe| {
          let program = maybe
            .as_mut()
            .expect("active actor program existence was prevalidated");
          program.schedule = schedule;
          program.schedule_window = schedule_window;
        });
        ActorHot::<T>::mutate(actor_id, |maybe| {
          if let Some(hot) = maybe.as_mut() {
            hot.schedule_anchor = schedule_anchor;
            hot.terminal_at = schedule_window.map(|window| Self::window_terminal_at(&window));
            Self::record_control_mutation(actor_id, now);
          }
        });
        Self::deposit_event(Event::ScheduleUpdated { actor_id });
        Self::prime_actor_schedule(actor_id).map_err(Self::placement_error)
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

    #[pallet::call_index(12)]
    #[pallet::weight(T::WeightInfo::update_execution_plan().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn update_execution_plan(
      origin: OriginFor<T>,
      actor_id: ActorId,
      execution_plan: ExecutionPlanOf<T>,
      completion_policy: CompletionPolicy,
    ) -> DispatchResult {
      ensure!(!execution_plan.is_empty(), Error::<T>::EmptyExecutionPlan);
      let snapshot = Self::active_actor_view(actor_id).ok_or(Error::<T>::ActorNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_retry_later_allowed(snapshot.mutability, &execution_plan)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::expiry_substitution_due(actor_id, &snapshot)? {
        return Self::finalize_actor(actor_id, &snapshot, CloseReason::WindowExpired);
      }
      ensure!(
        snapshot.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      let execution_plan_changed = snapshot.execution_plan != execution_plan;
      let completion_policy_changed = snapshot.completion_policy != completion_policy;
      if !execution_plan_changed && !completion_policy_changed {
        return Ok(());
      }
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_mutation_allowed(&snapshot, now)?;
      Self::validate_execution_plan_shape(snapshot.actor_class.actor_type(), &execution_plan)?;
      Self::validate_recipient_configuration(&execution_plan, &snapshot.sovereign_account)?;
      Self::validate_opening_snapshot_surfaces(&execution_plan)?;
      Self::ensure_execution_plan_fits_idle_budget(
        snapshot.actor_class.actor_type(),
        &execution_plan,
      )?;
      ensure!(
        (execution_plan.len() as u32) <= T::MaxExecutionPlanSteps::get(),
        Error::<T>::ExecutionPlanTooLong
      );
      if snapshot.actor_class.actor_type() == ActorType::User {
        ensure!(
          !Self::execution_plan_contains_mint(&execution_plan),
          Error::<T>::MintNotAllowedForUserActor
        );
      }
      let new_tracked = Self::derive_funding_tracked_assets(&execution_plan)?;
      let mut funding = ActorFunding::<T>::get(actor_id).ok_or(Error::<T>::ActorNotFound)?;
      funding.funding_tracked_assets = new_tracked.clone();
      funding
        .funding_accumulated
        .retain(|asset, _| new_tracked.contains(asset));
      let cancellation_reason = if execution_plan_changed {
        CancellationReason::ExecutionPlanChanged
      } else {
        CancellationReason::CompletionPolicyChanged
      };
      Self::with_control_transaction(|| {
        let continuation_cancelled =
          Self::cancel_continuation_internal(actor_id, cancellation_reason, None)?;
        ActorProgram::<T>::mutate(actor_id, |maybe| {
          let program = maybe
            .as_mut()
            .expect("active actor program existence was prevalidated");
          program.execution_plan = execution_plan;
          program.completion_policy = completion_policy;
        });
        ActorHot::<T>::mutate(actor_id, |maybe| {
          let hot = maybe
            .as_mut()
            .expect("active actor hot-state existence was prevalidated");
          hot.consecutive_failures = 0;
          Self::record_control_mutation(actor_id, now);
        });
        ActorFunding::<T>::insert(actor_id, funding);
        Self::deposit_event(Event::ExecutionPlanUpdated {
          actor_id,
          completion_policy,
        });
        if continuation_cancelled {
          Self::prime_actor_schedule(actor_id).map_err(Self::placement_error)?;
        }
        Ok(())
      })
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
          let Some(instance) = Self::active_actor_for_classification(actor_id)
            .map_err(Self::classification_dispatch_error)?
          else {
            missing = missing.saturating_add(1);
            continue;
          };
          if let Some(reason) = Self::sweep_close_reason(actor_id, &instance)
            .map_err(Self::classification_dispatch_error)?
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

    #[pallet::call_index(15)]
    #[pallet::weight(T::WeightInfo::update_schedule().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn set_auto_close_at_cycle_nonce(
      origin: OriginFor<T>,
      actor_id: ActorId,
      target: Option<u64>,
    ) -> DispatchResult {
      let snapshot = Self::active_actor_view(actor_id).ok_or(Error::<T>::ActorNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      if Self::expiry_substitution_due(actor_id, &snapshot)? {
        return Self::finalize_actor(actor_id, &snapshot, CloseReason::WindowExpired);
      }
      ensure!(
        snapshot.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      if let Some(target_nonce) = target {
        Self::ensure_auto_close_target(snapshot.cycle_nonce, target_nonce)?;
      }
      if snapshot.auto_close_at_cycle_nonce == target {
        return Ok(());
      }
      ActorHot::<T>::mutate(actor_id, |maybe| {
        maybe
          .as_mut()
          .expect("active actor existence was prevalidated")
          .auto_close_at_cycle_nonce = target;
      });
      Self::deposit_event(Event::AutoCloseNonceSet { actor_id, target });
      Ok(())
    }

    #[pallet::call_index(16)]
    #[pallet::weight(T::WeightInfo::update_schedule().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn increment_auto_close_nonce(
      origin: OriginFor<T>,
      actor_id: ActorId,
      by: u64,
    ) -> DispatchResult {
      ensure!(by > 0, Error::<T>::AutoCloseNonceIncrementZero);
      let snapshot = Self::active_actor_view(actor_id).ok_or(Error::<T>::ActorNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      if Self::expiry_substitution_due(actor_id, &snapshot)? {
        return Self::finalize_actor(actor_id, &snapshot, CloseReason::WindowExpired);
      }
      ensure!(
        snapshot.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      let cycle_nonce = snapshot.cycle_nonce;
      ActorHot::<T>::try_mutate(actor_id, |maybe| -> DispatchResult {
        let inst = maybe.as_mut().ok_or(Error::<T>::ActorNotFound)?;
        let old_target = inst.auto_close_at_cycle_nonce;
        let base = old_target.unwrap_or(cycle_nonce);
        let new_target = base
          .checked_add(by)
          .ok_or(Error::<T>::AutoCloseNonceOverflow)?;
        Self::ensure_auto_close_target(cycle_nonce, new_target)?;
        inst.auto_close_at_cycle_nonce = Some(new_target);
        Self::deposit_event(Event::AutoCloseNonceIncremented {
          actor_id,
          old_target,
          new_target,
          by,
        });
        Ok(())
      })?;
      Ok(())
    }

    #[pallet::call_index(17)]
    #[pallet::weight(T::WeightInfo::activate_actor())]
    pub fn activate_actor(
      origin: OriginFor<T>,
      actor_id: ActorId,
      program: ProgramInputOf<T>,
    ) -> DispatchResult {
      let identity = ActorIdentities::<T>::get(actor_id).ok_or_else(|| {
        if Self::active_actor_exists(actor_id) {
          Error::<T>::ActorAlreadyActive
        } else {
          Error::<T>::ActorNotFound
        }
      })?;
      Self::ensure_identity_control_origin(origin, &identity)?;
      Self::do_activate_actor(actor_id, identity, program)
    }

    #[pallet::call_index(18)]
    #[pallet::weight(T::WeightInfo::deactivate_actor())]
    pub fn deactivate_actor(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      let instance = Self::active_actor_view(actor_id).ok_or_else(|| {
        if ActorIdentities::<T>::contains_key(actor_id) {
          Error::<T>::ActorDormant
        } else {
          Error::<T>::ActorNotFound
        }
      })?;
      Self::ensure_control_origin(origin, &instance)?;
      ensure!(
        instance.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      Self::ensure_control_mutation_allowed(&instance, frame_system::Pallet::<T>::block_number())?;
      Self::do_deactivate_actor(actor_id, instance)
    }

    #[pallet::call_index(19)]
    #[pallet::weight(T::WeightInfo::continuation_cancel())]
    pub fn cancel_continuation(origin: OriginFor<T>, actor_id: ActorId) -> DispatchResult {
      let instance = Self::active_actor_view(actor_id).ok_or(Error::<T>::ActorNotFound)?;
      Self::ensure_control_origin(origin, &instance)?;
      ensure!(
        instance.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      if Self::expiry_substitution_due(actor_id, &instance)? {
        return Self::finalize_actor(actor_id, &instance, CloseReason::WindowExpired);
      }
      ensure!(
        instance.cycle_state == CycleState::Suspended,
        Error::<T>::ContinuationNotFound
      );
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_mutation_allowed(&instance, now)?;
      Self::with_control_transaction(|| {
        ensure!(
          Self::cancel_continuation_internal(actor_id, CancellationReason::Explicit, None)?,
          Error::<T>::ContinuationNotFound
        );
        Self::record_control_mutation(actor_id, now);
        Self::prime_actor_schedule(actor_id).map_err(Self::placement_error)
      })
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
      execution_plan: &ExecutionPlanOf<T>,
      start_cursor: usize,
    ) -> Weight {
      let mut upper = T::WeightInfo::step_orchestration(execution_plan.len() as u32);
      for step_index in start_cursor..execution_plan.len() {
        let step = &execution_plan[step_index];
        let condition_evaluation = T::WeightInfo::condition_set_evaluation(step.conditions.len());
        upper = upper
          .saturating_add(condition_evaluation)
          .saturating_add(Self::weight_upper_bound(&step.task));
        if actor_type == ActorType::User {
          upper = upper.saturating_add(T::WeightInfo::fee_collection());
        }
      }
      if (start_cursor..execution_plan.len()).any(|step_index| {
        execution_plan[step_index]
          .on_error
          .retry_max_attempts()
          .is_some()
      }) {
        let snapshot_entries = Self::opening_surfaces(execution_plan, start_cursor).len() as u32;
        upper = upper.saturating_add(
          T::WeightInfo::continuation_suspend(snapshot_entries)
            .max(T::WeightInfo::continuation_complete())
            .max(T::WeightInfo::continuation_cancel()),
        );
      }
      upper
    }

    pub fn compute_cycle_weight_upper(
      actor_type: ActorType,
      execution_plan: &ExecutionPlanOf<T>,
    ) -> Weight {
      Self::compute_cycle_weight_upper_from(actor_type, execution_plan, 0)
    }

    pub fn attempt_fee_envelope(
      actor_type: ActorType,
      execution_plan: &ExecutionPlanOf<T>,
      start_cursor: usize,
    ) -> Result<AttemptFeeEnvelopeOf<T>, Error<T>> {
      let mut inputs = BoundedVec::default();
      for step in execution_plan {
        let evaluation = if actor_type == ActorType::User {
          Self::compute_eval_fee_checked(step.conditions.len())?
        } else {
          Zero::zero()
        };
        let execution = if actor_type == ActorType::User {
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
          Error::<T>::ContinuationInvariant
        }
        FeeEnvelopeError::Overflow => Error::<T>::AdmissionBoundOverflow,
      })
    }

    pub(crate) fn attempt_weight_upper_bound(
      instance: &ActiveActorViewOf<T>,
      start_cursor: usize,
    ) -> Weight {
      let mut upper = Self::compute_cycle_weight_upper_from(
        instance.actor_class.actor_type(),
        &instance.execution_plan,
        start_cursor,
      );
      if instance.cycle_state == CycleState::Suspended {
        let suffix_steps = instance.execution_plan.len().saturating_sub(start_cursor) as u32;
        // Retry and terminal transition touch the same bounded Continuation value. The transition
        // envelope already carries its maximum proof, so only incremental RefTime composes here.
        let retry = T::WeightInfo::continuation_retry();
        let suffix_admission = T::WeightInfo::continuation_suffix_admission(suffix_steps);
        upper = upper
          .saturating_add(Weight::from_parts(retry.ref_time(), 0))
          .saturating_add(Weight::from_parts(suffix_admission.ref_time(), 0));
      }
      upper
    }

    pub(crate) fn attempt_fee_upper_bound(
      instance: &ActiveActorViewOf<T>,
      start_cursor: usize,
    ) -> BalanceOf<T> {
      Self::attempt_fee_envelope(
        instance.actor_class.actor_type(),
        &instance.execution_plan,
        start_cursor,
      )
      .expect("admitted execution plans have a checked fee envelope")
      .total
    }

    pub(crate) fn close_cycle_weight_upper_bound(_instance: &ActiveActorViewOf<T>) -> Weight {
      Self::close_cleanup_weight_upper()
    }

    /// Upper-bounds one prospective run plus pure terminal cleanup after the baseline scheduler
    /// envelope. Independently metered durable housekeeping may defer this work across blocks.
    pub fn execution_plan_admission_weight_upper(
      actor_type: ActorType,
      execution_plan: &ExecutionPlanOf<T>,
    ) -> Weight {
      let funding_count = Self::derive_funding_tracked_assets(execution_plan)
        .map(|assets| assets.len() as u32)
        .unwrap_or_else(|_| T::MaxFundingTrackedAssets::get());
      let snapshot_open = if funding_count == 0 {
        Weight::zero()
      } else {
        T::WeightInfo::funding_snapshot_open(funding_count)
      };
      let continuation_retry = if (0..execution_plan.len()).any(|step_index| {
        execution_plan[step_index]
          .on_error
          .retry_max_attempts()
          .is_some()
      }) {
        let retry = T::WeightInfo::continuation_retry();
        let suffix = T::WeightInfo::continuation_suffix_admission(execution_plan.len() as u32);
        Weight::from_parts(retry.ref_time().saturating_add(suffix.ref_time()), 0)
      } else {
        Weight::zero()
      };
      Self::scheduler_admission_overhead()
        .saturating_add(Self::compute_cycle_weight_upper(actor_type, execution_plan))
        .saturating_add(continuation_retry)
        .saturating_add(snapshot_open)
        .saturating_add(Self::close_cleanup_weight_upper())
    }

    pub fn guaranteed_actor_service_weight() -> Option<Weight> {
      T::ActorOnIdleReserve::get()
        .checked_sub(&T::WeightInfo::scheduler_on_idle_base())
        .and_then(|remaining| {
          remaining.checked_sub(&T::WeightInfo::scheduler_paged_tombstone_drain(1))
        })
        .and_then(|remaining| remaining.checked_sub(&T::WakeupWeightLimit::get()))
        .and_then(|remaining| remaining.checked_sub(&T::ObservationFanoutWeightLimit::get()))
    }

    fn ensure_execution_plan_fits_idle_budget(
      actor_type: ActorType,
      execution_plan: &ExecutionPlanOf<T>,
    ) -> DispatchResult {
      let actor_service = Self::guaranteed_actor_service_weight()
        .ok_or(Error::<T>::ExecutionPlanExceedsOnIdleBudget)?;
      ensure!(
        Self::execution_plan_admission_weight_upper(actor_type, execution_plan)
          .all_lte(actor_service),
        Error::<T>::ExecutionPlanExceedsOnIdleBudget
      );
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

    /// User Active creation/activation prefunding requirement (spec 7.1): the prospective/current
    /// sovereign fee-native balance must cover `MinUserBalance + attempt_fee_envelope(plan, 0, User).total`
    /// so the first opening can charge the whole cycle fee while preserving `MinUserBalance`.
    fn user_active_prefunding_requirement(
      execution_plan: &ExecutionPlanOf<T>,
    ) -> Result<BalanceOf<T>, Error<T>> {
      let envelope_total = Self::attempt_fee_envelope(ActorType::User, execution_plan, 0)?.total;
      T::MinUserBalance::get()
        .checked_add(&envelope_total)
        .ok_or(Error::<T>::AdmissionBoundOverflow)
    }

    fn ensure_user_active_prefunding(
      sovereign_account: &T::AccountId,
      execution_plan: &ExecutionPlanOf<T>,
    ) -> DispatchResult {
      let required = Self::user_active_prefunding_requirement(execution_plan)?;
      ensure!(
        T::AssetOps::balance(sovereign_account, T::FeeNativeAssetId::get()) >= required,
        Error::<T>::InsufficientBalance
      );
      Ok(())
    }

    pub fn sovereign_account_id(owner: &T::AccountId, owner_slot: u8) -> T::AccountId {
      let seed =
        frame::hashing::blake2_256(&(T::PalletId::get(), b"user", owner, owner_slot).encode());
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::new(&seed))
        .expect("hashed seed always decodes into AccountId")
    }

    pub fn sovereign_account_id_system(actor_id: ActorId) -> T::AccountId {
      let seed = frame::hashing::blake2_256(&(T::PalletId::get(), b"system", actor_id).encode());
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::new(&seed))
        .expect("hashed seed always decodes into AccountId")
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
        let (owner_slot, sovereign_account) = match actor_type {
          ActorType::User => match Self::allocate_owner_slot(&owner, preferred_user_slot) {
            Ok((slot, account)) => (Some(slot), account),
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error.into(),
              ));
            }
          },
          ActorType::System => match Self::allocate_system_sovereign(system_sovereign_id) {
            Ok(account) => (None, account),
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
          actor_class: match actor_type {
            ActorType::User => ActorClass::User {
              owner_slot: owner_slot.expect("User allocation always returns a slot"),
            },
            ActorType::System => ActorClass::System {
              sovereign_id: system_sovereign_id,
            },
          },
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
            SystemSovereignCount::<T>::mutate(|count| *count += 1);
          }
        }
        NextActorId::<T>::put(next_id);
        if actor_type == ActorType::User || requested_system_sovereign_id.is_none() {
          frame_system::Pallet::<T>::inc_providers(&sovereign_account);
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
      program: ProgramInputOf<T>,
    ) -> DispatchResult {
      match program {
        ProgramInput::Dormant => {
          ensure!(
            mutability == Mutability::Mutable,
            Error::<T>::ImmutableActor
          );
          Self::do_create_dormant_actor(owner, ActorType::User, preferred_slot, None)
        }
        ProgramInput::Active(ActiveProgramInput {
          schedule,
          schedule_window,
          execution_plan,
          completion_policy,
          funding_source_policy,
          auto_close_at_cycle_nonce,
        }) => Self::do_create_actor(
          owner,
          ActorType::User,
          mutability,
          schedule,
          schedule_window,
          execution_plan,
          completion_policy,
          funding_source_policy,
          auto_close_at_cycle_nonce,
          preferred_slot,
          None,
        ),
      }
    }

    fn do_create_system_actor(
      owner: T::AccountId,
      mutability: Mutability,
      program: ProgramInputOf<T>,
      requested_system_sovereign_id: Option<SystemSovereignId>,
    ) -> DispatchResult {
      match program {
        ProgramInput::Dormant => {
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
        ProgramInput::Active(ActiveProgramInput {
          schedule,
          schedule_window,
          execution_plan,
          completion_policy,
          funding_source_policy,
          auto_close_at_cycle_nonce,
        }) => Self::do_create_actor(
          owner,
          ActorType::System,
          mutability,
          schedule,
          schedule_window,
          execution_plan,
          completion_policy,
          funding_source_policy,
          auto_close_at_cycle_nonce,
          None,
          requested_system_sovereign_id,
        ),
      }
    }

    fn do_create_actor(
      owner: T::AccountId,
      actor_type: ActorType,
      mutability: Mutability,
      schedule: ScheduleOf<T>,
      schedule_window: Option<ScheduleWindow<BlockNumberFor<T>>>,
      execution_plan: ExecutionPlanOf<T>,
      completion_policy: CompletionPolicy,
      funding_source_policy: FundingSourcePolicyOf<T>,
      auto_close_at_cycle_nonce: Option<u64>,
      preferred_user_slot: Option<u8>,
      requested_system_sovereign_id: Option<SystemSovereignId>,
    ) -> DispatchResult {
      ensure!(
        !GlobalCircuitBreaker::<T>::get(),
        Error::<T>::GlobalCircuitBreakerActive
      );
      ensure!(!execution_plan.is_empty(), Error::<T>::EmptyExecutionPlan);
      ensure!(
        (execution_plan.len() as u32) <= T::MaxExecutionPlanSteps::get(),
        Error::<T>::ExecutionPlanTooLong
      );
      if actor_type == ActorType::User {
        ensure!(
          !Self::execution_plan_contains_mint(&execution_plan),
          Error::<T>::MintNotAllowedForUserActor
        );
      }
      Self::validate_schedule(&schedule)?;
      if let Some(ref window) = schedule_window {
        Self::validate_schedule_window(window)?;
      }
      Self::validate_future_schedule_targets(NextActorId::<T>::get(), &schedule, schedule_window)?;
      Self::validate_execution_plan_shape(actor_type, &execution_plan)?;
      Self::validate_opening_snapshot_surfaces(&execution_plan)?;
      Self::ensure_retry_later_allowed(mutability, &execution_plan)?;
      if let Some(target_nonce) = auto_close_at_cycle_nonce {
        Self::ensure_auto_close_target(0, target_nonce)?;
      }
      if actor_type == ActorType::System && mutability == Mutability::Immutable {
        ensure!(
          !schedule.trigger.manual_source_enabled(),
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
      Self::ensure_execution_plan_fits_idle_budget(actor_type, &execution_plan)?;
      let funding_tracked_assets = Self::derive_funding_tracked_assets(&execution_plan)?;
      let actor_id = NextActorId::<T>::get();
      ensure!(
        !Self::active_actor_exists(actor_id) && !ActorIdentities::<T>::contains_key(actor_id),
        Error::<T>::ActorIdOccupied
      );
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
      Self::validate_recipient_configuration(&execution_plan, &prospective_sovereign_account)?;
      let next_id = actor_id.checked_add(1).ok_or(Error::<T>::ActorIdOverflow)?;
      let now = frame_system::Pallet::<T>::block_number();
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        let (owner_slot, sovereign_account) = match actor_type {
          ActorType::User => match Self::allocate_owner_slot(&owner, preferred_user_slot) {
            Ok((slot, account)) => (Some(slot), account),
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error.into(),
              ));
            }
          },
          ActorType::System => match Self::allocate_system_sovereign(system_sovereign_id) {
            Ok(account) => (None, account),
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error.into(),
              ));
            }
          },
        };
        if actor_type == ActorType::User {
          // Spec 7.1: the allocated sovereign fee-native balance must cover
          // `MinUserBalance + attempt_fee_envelope(plan, 0, User).total` before the opening
          // fee or Active state commits; Dormant creation remains unfunded.
          if let Err(error) =
            Self::ensure_user_active_prefunding(&sovereign_account, &execution_plan)
          {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
          }
          if let Err(error) = Self::charge_creation_fee(&owner) {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
          }
        }
        let schedule_anchor = Self::schedule_anchor_at(schedule_window, now);
        let actor_class = match actor_type {
          ActorType::User => ActorClass::User {
            owner_slot: owner_slot.expect("User allocation always returns a slot"),
          },
          ActorType::System => ActorClass::System {
            sovereign_id: system_sovereign_id,
          },
        };
        let identity = ActorIdentity {
          sovereign_account: sovereign_account.clone(),
          owner: owner.clone(),
          actor_class,
          mutability,
          cycle_nonce: 0,
          last_control_mutation_block: now,
        };
        let hot = ActorHotState {
          lifecycle: ActiveLifecycle::Active,
          cycle_state: CycleState::Idle,
          auto_close_at_cycle_nonce,
          consecutive_failures: 0,
          pending_signal: false,
          queue_ticket: None,
          wakeup_pointer: None,
          terminal_at: schedule_window.map(|window| Self::window_terminal_at(&window)),
          schedule_anchor,
          last_cycle_block: None,
        };
        let program = ActorProgramState {
          schedule,
          schedule_window,
          execution_plan,
          completion_policy,
        };
        SovereignIndex::<T>::insert(sovereign_account.clone(), actor_id);
        if let Err(error) = Self::insert_active_actor(actor_id, identity, hot, program) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        ActorFunding::<T>::insert(
          actor_id,
          ActorFundingState {
            funding_source_policy,
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
            SystemSovereignCount::<T>::mutate(|count| *count += 1);
          }
        }
        NextActorId::<T>::put(next_id);
        if actor_type == ActorType::System && requested_system_sovereign_id.is_none() {
          frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        }
        Self::deposit_event(Event::ActorCreated {
          actor_id,
          owner,
          actor_class: match actor_type {
            ActorType::User => ActorClass::User {
              owner_slot: owner_slot.expect("User allocation always returns a slot"),
            },
            ActorType::System => ActorClass::System {
              sovereign_id: system_sovereign_id,
            },
          },
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
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })
    }

    fn do_activate_actor(
      actor_id: ActorId,
      mut identity: ActorIdentityOf<T>,
      program: ProgramInputOf<T>,
    ) -> DispatchResult {
      let ProgramInput::Active(ActiveProgramInput {
        schedule,
        schedule_window,
        execution_plan,
        completion_policy,
        funding_source_policy,
        auto_close_at_cycle_nonce,
      }) = program
      else {
        return Err(Error::<T>::EmptyExecutionPlan.into());
      };
      ensure!(
        !GlobalCircuitBreaker::<T>::get(),
        Error::<T>::GlobalCircuitBreakerActive
      );
      ensure!(
        identity.mutability == Mutability::Mutable,
        Error::<T>::ImmutableActor
      );
      let actor_type = identity.actor_class.actor_type();
      ensure!(!execution_plan.is_empty(), Error::<T>::EmptyExecutionPlan);
      ensure!(
        (execution_plan.len() as u32) <= T::MaxExecutionPlanSteps::get(),
        Error::<T>::ExecutionPlanTooLong
      );
      if actor_type == ActorType::User {
        ensure!(
          !Self::execution_plan_contains_mint(&execution_plan),
          Error::<T>::MintNotAllowedForUserActor
        );
      }
      Self::validate_schedule(&schedule)?;
      if let Some(ref window) = schedule_window {
        Self::validate_schedule_window(window)?;
      }
      Self::validate_future_schedule_targets(actor_id, &schedule, schedule_window)?;
      Self::validate_execution_plan_shape(actor_type, &execution_plan)?;
      Self::validate_recipient_configuration(&execution_plan, &identity.sovereign_account)?;
      Self::validate_opening_snapshot_surfaces(&execution_plan)?;
      Self::ensure_retry_later_allowed(identity.mutability, &execution_plan)?;
      if let Some(target_nonce) = auto_close_at_cycle_nonce {
        Self::ensure_auto_close_target(identity.cycle_nonce, target_nonce)?;
      }
      Self::ensure_execution_plan_fits_idle_budget(actor_type, &execution_plan)?;
      let funding_tracked_assets = Self::derive_funding_tracked_assets(&execution_plan)?;
      ensure!(
        Self::active_instance_count() < Self::effective_active_actor_limit(),
        Error::<T>::ActiveActorCapacityExceeded
      );
      if actor_type == ActorType::User {
        Self::ensure_user_active_prefunding(&identity.sovereign_account, &execution_plan)?;
      }
      let now = frame_system::Pallet::<T>::block_number();
      ensure!(
        identity.last_control_mutation_block != now,
        Error::<T>::ControlMutationRateLimited
      );
      identity.last_control_mutation_block = now;
      // Reactivation anchors the fresh Active epoch at the current block; the fresh hot
      // state has no last_cycle_block, so cooldown/cadence use this conservative anchor
      // rather than block zero (spec 4.3.3).
      let schedule_anchor = Self::schedule_anchor_at(schedule_window, now);
      let hot = ActorHotState {
        lifecycle: ActiveLifecycle::Active,
        cycle_state: CycleState::Idle,
        auto_close_at_cycle_nonce,
        consecutive_failures: 0,
        pending_signal: false,
        queue_ticket: None,
        wakeup_pointer: None,
        terminal_at: schedule_window.map(|window| Self::window_terminal_at(&window)),
        schedule_anchor,
        last_cycle_block: None,
      };
      let program = ActorProgramState {
        schedule,
        schedule_window,
        execution_plan,
        completion_policy,
      };
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        if !ActorIdentities::<T>::contains_key(actor_id) || Self::active_actor_exists(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorAlreadyActive.into(),
          ));
        }
        if let Err(error) = Self::insert_active_actor(actor_id, identity, hot, program) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        ActorFunding::<T>::insert(
          actor_id,
          ActorFundingState {
            funding_source_policy,
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
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })
    }

    fn do_deactivate_actor(actor_id: ActorId, _instance: ActiveActorViewOf<T>) -> DispatchResult {
      let now = frame_system::Pallet::<T>::block_number();
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        Self::record_control_mutation(actor_id, now);
        if let Err(error) =
          Self::cancel_continuation_internal(actor_id, CancellationReason::Deactivated, None)
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        Self::remove_actor_from_queues(actor_id);
        if ActorHot::<T>::get(actor_id).is_some_and(|hot| hot.wakeup_pointer.is_some())
          && Self::wakeup_substrate_invalidate(actor_id).is_none()
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::ActorNotFound.into(),
          ));
        }
        if let Err(error) = Self::remove_active_actor(actor_id) {
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
        Self::deposit_event(Event::ActorDeactivated { actor_id });
        #[cfg(test)]
        if let Err(error) = crate::mock::control_atomicity_checkpoint(actor_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })
    }

    fn execution_plan_contains_mint(execution_plan: &ExecutionPlanOf<T>) -> bool {
      for step in execution_plan.as_slice() {
        if matches!(step.task, ActorTask::Mint { .. }) {
          return true;
        }
      }
      false
    }

    fn validate_schedule(schedule: &ScheduleOf<T>) -> DispatchResult {
      ensure!(
        schedule.trigger.has_canonical_sources(),
        Error::<T>::InvalidTriggerConfiguration
      );
      // Both cadence and cooldown are bounded by MaxExecutionDelayBlocks (spec 7.3.1).
      let max_delay: u32 = T::MaxExecutionDelayBlocks::get().saturated_into();
      if let TriggerPolicy::Cadenced { every_blocks, .. } = &schedule.trigger {
        ensure!(*every_blocks > 0, Error::<T>::InvalidTriggerConfiguration);
        let jitter_window = every_blocks
          .saturating_div(4)
          .min(T::MaxTimerJitterBlocks::get());
        let worst_case_jitter = jitter_window.saturating_sub(1);
        let composed_delay = every_blocks
          .checked_add(&worst_case_jitter)
          .ok_or(Error::<T>::ExecutionDelayTooLong)?;
        ensure!(
          composed_delay <= max_delay,
          Error::<T>::ExecutionDelayTooLong
        );
      }
      let cooldown_blocks: u32 = schedule.cooldown_blocks.saturated_into();
      ensure!(
        cooldown_blocks <= max_delay,
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

    fn validate_future_schedule_targets(
      actor_id: ActorId,
      schedule: &ScheduleOf<T>,
      schedule_window: Option<ScheduleWindow<BlockNumberFor<T>>>,
    ) -> DispatchResult {
      let now = frame_system::Pallet::<T>::block_number();
      let schedule_anchor = schedule_window
        .map(|window| now.max(window.start))
        .unwrap_or(now);
      ensure!(
        now.checked_add(&One::one()).is_some(),
        Error::<T>::SchedulerIndexExhausted
      );
      let cooldown: BlockNumberFor<T> = schedule.cooldown_blocks.into();
      ensure!(
        schedule_anchor.checked_add(&cooldown).is_some(),
        Error::<T>::SchedulerIndexExhausted
      );
      let first_temporal_eligible =
        if let TriggerPolicy::Cadenced { every_blocks, .. } = schedule.trigger {
          let cadence: BlockNumberFor<T> = every_blocks.into();
          let phase_window = every_blocks
            .saturating_div(4)
            .min(T::MaxTimerJitterBlocks::get());
          let worst_case_phase: BlockNumberFor<T> = phase_window.saturating_sub(1).into();
          ensure!(
            schedule_anchor
              .checked_add(&cadence)
              .and_then(|target| target.checked_add(&worst_case_phase))
              .is_some(),
            Error::<T>::SchedulerIndexExhausted
          );
          Self::cadence_at_or_after(actor_id, schedule_anchor, every_blocks, schedule_anchor)
            .map_err(|_| Error::<T>::SchedulerIndexExhausted)?
        } else {
          schedule_anchor
        };
      if let Some(window) = schedule_window {
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
      execution_plan: &ExecutionPlanOf<T>,
    ) -> DispatchResult {
      if mutability == Mutability::Immutable {
        for step in execution_plan {
          ensure!(
            step.on_error.retry_max_attempts().is_none(),
            Error::<T>::RetryLaterNotAllowedForImmutableActor
          );
        }
      }
      Ok(())
    }

    fn validate_execution_plan_shape(
      actor_type: ActorType,
      execution_plan: &ExecutionPlanOf<T>,
    ) -> DispatchResult {
      ensure!(
        execution_plan_steps_bound_is_valid(T::MaxExecutionPlanSteps::get()),
        Error::<T>::ExecutionPlanTooLong
      );
      Self::attempt_fee_envelope(actor_type, execution_plan, 0)?;
      for step in execution_plan.as_slice() {
        if let Some(max_attempts) = step.on_error.retry_max_attempts() {
          ensure!(
            max_attempts >= 2 && max_attempts <= T::MaxRetryAttempts::get(),
            Error::<T>::InvalidRetryAttemptLimit
          );
        }
        ensure!(
          !matches!(
            &step.conditions,
            ConditionSet::All(conditions) | ConditionSet::Any(conditions) if conditions.is_empty()
          ),
          Error::<T>::EmptyConditionSet
        );
        if let ConditionSet::All(conditions) | ConditionSet::Any(conditions) = &step.conditions {
          for condition in conditions {
            let max_age_blocks = match condition {
              Condition::ObservationAbove { max_age_blocks, .. }
              | Condition::ObservationBelow { max_age_blocks, .. }
              | Condition::ObservationEquals { max_age_blocks, .. }
              | Condition::ObservationNotEquals { max_age_blocks, .. } => Some(max_age_blocks),
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
      execution_plan: &ExecutionPlanOf<T>,
      sovereign_account: &T::AccountId,
    ) -> DispatchResult {
      for step in execution_plan {
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

    fn validate_opening_snapshot_surfaces(execution_plan: &ExecutionPlanOf<T>) -> DispatchResult {
      for surface in Self::opening_surfaces(execution_plan, 0) {
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
      execution_plan: &ExecutionPlanOf<T>,
    ) -> Result<BoundedBTreeSet<T::AssetId, T::MaxFundingTrackedAssets>, DispatchError> {
      let mut tracked = alloc::collections::BTreeSet::new();

      let mut check_amount = |amount: &AmountResolution<T::Balance>, asset: T::AssetId| {
        if matches!(amount, AmountResolution::PercentageOfLastFunding(_)) {
          tracked.insert(asset);
        }
      };

      for step in execution_plan.as_slice() {
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

      BoundedBTreeSet::try_from(tracked).map_err(|_| Error::<T>::ExecutionPlanTooLong.into())
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

    fn record_control_mutation(actor_id: ActorId, now: BlockNumberFor<T>) {
      ActorIdentities::<T>::mutate(actor_id, |maybe| {
        maybe
          .as_mut()
          .expect("active actor identity existence was prevalidated")
          .last_control_mutation_block = now;
      });
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

      Self::preflight_remove_observation_subscriptions(actor_id)?;

      polkadot_sdk::frame_support::storage::with_transaction(|| {
        let result = (|| -> DispatchResult {
          Self::cancel_continuation_internal(actor_id, CancellationReason::Closing(reason), None)?;

          // Actor-local ticket/pointer ownership makes shared queue and wakeup entries stale as
          // soon as hot state disappears. Terminal cleanup performs no shared-container scan.
          Self::remove_active_actor(actor_id)?;
          ActorIdentities::<T>::remove(actor_id);
          ActorFunding::<T>::remove(actor_id);
          ActiveActorCount::<T>::mutate(|count| *count -= 1);
          ActorIdentityCount::<T>::mutate(|count| *count -= 1);
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
      if let ActorClass::User { owner_slot } = identity.actor_class {
        ensure!(
          Self::owner_slot_is_set(&OwnerSlotBitmaps::<T>::get(&identity.owner), owner_slot),
          Error::<T>::InvalidOwnerSlot
        );
      }

      Self::with_control_transaction(|| {
        ActorIdentities::<T>::remove(actor_id);
        ActorIdentityCount::<T>::mutate(|count| *count -= 1);
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

    pub(crate) fn remove_actor_from_queues(actor_id: ActorId) {
      ActorHot::<T>::mutate(actor_id, |maybe| {
        if let Some(hot) = maybe.as_mut() {
          hot.queue_ticket = None;
        }
      });
    }

    #[cfg(feature = "try-runtime")]
    pub(crate) fn do_try_state() -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      use polkadot_sdk::sp_runtime::TryRuntimeError;
      let limit = Self::effective_active_actor_limit();
      let active_count = Self::active_instance_count();
      let actual_active_count = ActorHot::<T>::iter_keys().count() as u32;
      if T::MaxOwnerSlots::get() == 0 {
        return Err(TryRuntimeError::Other("MaxOwnerSlots must be nonzero"));
      }
      if !execution_plan_steps_bound_is_valid(T::MaxExecutionPlanSteps::get()) {
        return Err(TryRuntimeError::Other(
          "MaxExecutionPlanSteps must be in 1..=255",
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
      for actor_id in ActorHot::<T>::iter_keys() {
        if !ActorProgram::<T>::contains_key(actor_id) {
          return Err(TryRuntimeError::Other(
            "ActorHot entry has no matching ActorProgram entry",
          ));
        }
      }
      for actor_id in ActorProgram::<T>::iter_keys() {
        if !ActorHot::<T>::contains_key(actor_id) {
          return Err(TryRuntimeError::Other(
            "ActorProgram entry has no matching ActorHot entry",
          ));
        }
      }
      let mut max_id: Option<ActorId> = None;
      for actor_id in ActorHot::<T>::iter_keys() {
        let hot = ActorHot::<T>::get(actor_id)
          .ok_or(TryRuntimeError::Other("active hot key has no value"))?;
        let identity = ActorIdentities::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
          "ActorHot entry has no matching ActorIdentity entry",
        ))?;
        let program = ActorProgram::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
          "ActorHot entry has no matching ActorProgram entry",
        ))?;
        let has_continuation = ContinuationStateStore::<T>::contains_key(actor_id);
        if (hot.cycle_state == CycleState::Suspended) != has_continuation {
          return Err(TryRuntimeError::Other(
            "ActorHot cycle_state disagrees with ContinuationState",
          ));
        }
        // Terminal membership is derived from the schedule window: `terminal_at` is the sole
        // terminal-membership authority and must equal the window's exact terminal block, or be
        // absent without a window (spec 5.1).
        let program_window = program.schedule_window;
        let expected_terminal_at = program_window.map(|window| Self::window_terminal_at(&window));
        if hot.terminal_at != expected_terminal_at {
          return Err(TryRuntimeError::Other(
            "ActorHot terminal_at disagrees with schedule window terminal membership",
          ));
        }
        let instance = Self::derive_active_actor_view(identity, hot, program);
        if !Self::execution_plan_admission_weight_upper(
          instance.actor_class.actor_type(),
          &instance.execution_plan,
        )
        .all_lte(
          Self::guaranteed_actor_service_weight().ok_or(TryRuntimeError::Other(
            "configured housekeeping Weight exceeds ActorOnIdleReserve",
          ))?,
        ) {
          return Err(TryRuntimeError::Other(
            "active actor plan exceeds current actor-service envelope",
          ));
        }
        max_id = Some(max_id.map_or(actor_id, |prev| prev.max(actor_id)));
        let Some(funding) = ActorFunding::<T>::get(actor_id) else {
          return Err(TryRuntimeError::Other(
            "ActorHot entry has no matching ActorFunding entry",
          ));
        };
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
        if let ActorClass::User { owner_slot } = instance.actor_class {
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
      }
      for actor_id in ActorFunding::<T>::iter_keys() {
        if !Self::active_actor_exists(actor_id) {
          return Err(TryRuntimeError::Other(
            "ActorFunding entry has no matching split active actor",
          ));
        }
      }
      for actor_id in ContinuationStateStore::<T>::iter_keys() {
        let continuation = ContinuationStateStore::<T>::get(actor_id)
          .ok_or(TryRuntimeError::Other("Continuation key has no value"))?;
        let hot = ActorHot::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
          "ContinuationState entry has no matching ActorHot entry",
        ))?;
        let identity = ActorIdentities::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
          "ContinuationState entry has no matching ActorIdentity entry",
        ))?;
        let program = ActorProgram::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
          "ContinuationState entry has no matching ActorProgram entry",
        ))?;
        if hot.cycle_state != CycleState::Suspended
          || identity.mutability != Mutability::Mutable
          || identity.cycle_nonce == 0
          || continuation.cursor >= program.execution_plan.len() as u32
        {
          return Err(TryRuntimeError::Other(
            "ContinuationState violates run marker, mutability, or cursor bounds",
          ));
        }
        let max_attempts = program.execution_plan[continuation.cursor as usize]
          .on_error
          .retry_max_attempts()
          .ok_or(TryRuntimeError::Other(
            "ContinuationState cursor does not own RetryLater",
          ))?;
        if continuation.unsuccessful_attempts_at_cursor == 0
          || continuation.unsuccessful_attempts_at_cursor >= max_attempts
        {
          return Err(TryRuntimeError::Other(
            "ContinuationState cursor-local attempt count is outside its live range",
          ));
        }
        let expected_surfaces =
          Self::opening_surfaces(&program.execution_plan, continuation.cursor as usize);
        let mut surfaces_match = expected_surfaces.len() == continuation.opening_snapshot.len();
        for surface in &expected_surfaces {
          if !continuation.opening_snapshot.contains_key(surface) {
            surfaces_match = false;
            break;
          }
        }
        if !surfaces_match {
          return Err(TryRuntimeError::Other(
            "ContinuationState opening snapshot disagrees with unresolved suffix",
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
          || ContinuationStateStore::<T>::contains_key(actor_id)
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
          ActorClass::System { .. } if identity.mutability != Mutability::Mutable => {
            return Err(TryRuntimeError::Other(
              "Dormant System Actors must be Mutable",
            ));
          }
          ActorClass::System { .. } => {}
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
      if head < tail && tail.saturating_sub(head) != u64::from(queue_occupancy) {
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
          let position = page_start.saturating_add(slot as u64);
          if position < head {
            continue;
          }
          if position >= tail || entry.ticket >= next_ticket {
            return Err(TryRuntimeError::Other(
              "canonical queue entry lies beyond its physical or global ticket range",
            ));
          }
          physical_occupancy = physical_occupancy.saturating_add(1);
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
      let mut wakeup_live_by_block = alloc::collections::BTreeMap::new();
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
            live_entries = live_entries.saturating_add(1);
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
        let block_live = wakeup_live_by_block.entry(block).or_insert(0u32);
        *block_live = block_live.saturating_add(live_entries);
        // `wakeup_pointer` is the sole ordinary temporal-membership authority (spec 5.1): a
        // physical page slot is a live member only when the actor's pointer addresses exactly
        // this slot. A slot whose actor owns a different pointer is corruption; a slot whose
        // actor owns no pointer (or no hot state after a lazy terminal cleanup) is a stale
        // physical entry with no authority until the bounded drain converges.
        for slot in 0..page.entries.len() {
          let Some(entry) = &page.entries[slot] else {
            continue;
          };
          let expected = WakeupPointer {
            block,
            page_id,
            slot: slot as WakeupSlot,
          };
          match ActorHot::<T>::get(entry.actor_id).and_then(|hot| hot.wakeup_pointer) {
            Some(pointer) if pointer == expected => {
              live_wakeup_memberships = live_wakeup_memberships.saturating_add(1);
            }
            Some(_) => {
              return Err(TryRuntimeError::Other(
                "WakeupPage slot addresses an actor with a different wakeup pointer",
              ));
            }
            None => {}
          }
        }
      }
      // Live wakeup memberships are one-per-active-actor and never exceed the active set;
      // stale physical pages and cursor blocks may legitimately outlive their actors.
      if live_wakeup_memberships > active_count {
        return Err(TryRuntimeError::Other(
          "live wakeup memberships exceed active actor count",
        ));
      }
      let cursor_len = WakeupCursorLen::<T>::get();
      for block in WakeupBuckets::<T>::iter_keys() {
        let bucket = WakeupBuckets::<T>::get(block)
          .ok_or(TryRuntimeError::Other("wakeup bucket key has no value"))?;
        if wakeup_live_by_block.get(&block).copied() != Some(bucket.live_entries) {
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
        if let Some(index) = bucket.cursor_index
          && (index >= cursor_len || Self::wakeup_cursor_get(index) != Some(block))
        {
          return Err(TryRuntimeError::Other(
            "WakeupBucket cursor reverse index does not resolve",
          ));
        }
      }
      if cursor_len > T::MaxActiveActors::get() {
        return Err(TryRuntimeError::Other(
          "WakeupCursorLen exceeds configured active actor capacity",
        ));
      }
      let cursor_page_size = T::WakeupPageSize::get();
      let expected_cursor_pages = cursor_len.div_ceil(cursor_page_size);
      let actual_cursor_pages = WakeupCursorPages::<T>::iter_keys().count() as u32;
      if actual_cursor_pages != expected_cursor_pages {
        return Err(TryRuntimeError::Other(
          "WakeupCursorPages count disagrees with cursor length",
        ));
      }
      for page_id in 0..expected_cursor_pages {
        let Some(page) = WakeupCursorPages::<T>::get(u64::from(page_id)) else {
          return Err(TryRuntimeError::Other(
            "WakeupCursorPages has a gap in logical page order",
          ));
        };
        let consumed = page_id.saturating_mul(cursor_page_size);
        let expected_len = cursor_len.saturating_sub(consumed).min(cursor_page_size) as usize;
        if page.len() != expected_len {
          return Err(TryRuntimeError::Other(
            "WakeupCursorPage length disagrees with logical position",
          ));
        }
      }
      let mut cursor_blocks = alloc::collections::BTreeSet::new();
      for index in 0..cursor_len {
        let Some(block) = Self::wakeup_cursor_get(index) else {
          return Err(TryRuntimeError::Other(
            "WakeupCursor index does not resolve to a page entry",
          ));
        };
        if !cursor_blocks.insert(block) {
          return Err(TryRuntimeError::Other(
            "WakeupCursor contains a duplicate block",
          ));
        }
        if WakeupBuckets::<T>::get(block).and_then(|bucket| bucket.cursor_index) != Some(index) {
          return Err(TryRuntimeError::Other(
            "WakeupCursor block has no matching bucket reverse index",
          ));
        }
        if index > 0 {
          let parent = index.saturating_sub(1) / 2;
          if Self::wakeup_cursor_get(parent).is_none_or(|parent_block| parent_block > block) {
            return Err(TryRuntimeError::Other(
              "WakeupCursor violates min-heap ordering",
            ));
          }
        }
      }
      let mut live_wakeup_pointers = alloc::collections::BTreeSet::new();
      for actor_id in ActorHot::<T>::iter_keys() {
        let hot = ActorHot::<T>::get(actor_id).ok_or(TryRuntimeError::Other(
          "wakeup-pointer hot key has no value",
        ))?;
        let Some(pointer) = hot.wakeup_pointer else {
          continue;
        };
        if !live_wakeup_pointers.insert((pointer.block, pointer.page_id, pointer.slot)) {
          return Err(TryRuntimeError::Other(
            "multiple actors own the same wakeup pointer",
          ));
        }
        if !Self::wakeup_page_entry_matches(pointer, actor_id) {
          return Err(TryRuntimeError::Other(
            "ActorHot wakeup pointer does not resolve to its actor",
          ));
        }
        // The earlier due requirement determines the next temporal service point (spec 5.1):
        // every placement path clamps the wakeup target to the terminal block, so an ordinary
        // or terminal wakeup must never be scheduled beyond `terminal_at`.
        if hot
          .terminal_at
          .is_some_and(|terminal_at| pointer.block > terminal_at)
        {
          return Err(TryRuntimeError::Other(
            "ActorHot wakeup pointer exceeds its terminal membership",
          ));
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
      Self::do_try_state_observation_subscriptions()?;
      Self::do_try_state_dirty_observations()?;
      Ok(())
    }
  }
}
