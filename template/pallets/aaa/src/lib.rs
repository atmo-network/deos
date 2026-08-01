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
  AssetOps, DexOps, ExecutionContext, FundingAuthority, LiquidityOps, ObservationProvider,
  RetryClass, ScalarObservationState, StakingOps, TaskFailure,
};
pub use types::{InputLimit, Task, WakeupBucketState, WakeupCursorIndex};

pub mod weights;
pub use weights::{TaskWeightInfo, WeightInfo};

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
  aaa_type: types::AaaType,
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
  for input in inputs
    .iter() // deos-bypass: bounded-iter — MaxExecutionPlanSteps bounds fee-envelope composition.
    .skip(start_cursor)
  {
    let evaluation = if aaa_type == types::AaaType::User {
      input.evaluation
    } else {
      Balance::zero()
    };
    let execution = if aaa_type == types::AaaType::User {
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
  aaa_type: types::AaaType,
  is_fee_native: bool,
  asset_minimum: Balance,
  min_user_balance: Balance,
) -> Balance {
  if aaa_type == types::AaaType::User && is_fee_native {
    core::cmp::max(asset_minimum, min_user_balance)
  } else {
    asset_minimum
  }
}

/// Settles one admitted fee-envelope step without touching host balances.
///
/// User reservation always releases the step's full upper bound before charging either the
/// evaluation-only or attempted-step amount. System AAA remains fee-exempt.
pub fn settle_attempt_fee_step<Balance>(
  aaa_type: types::AaaType,
  reservation: Balance,
  step: &StepFeeEnvelope<Balance>,
  charge_kind: FeeChargeKind,
) -> Result<FeeStepSettlement<Balance>, FeeEnvelopeError>
where
  Balance: Copy + CheckedSub + Zero,
{
  if aaa_type == types::AaaType::System {
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
  pub trait AaaSimulationApi<Program>
  where
    Program: codec::Codec,
  {
    fn simulate_current_program(
      aaa_id: types::AaaId,
      expected_type: types::AaaType,
      expected_mutability: types::Mutability,
      expected_program: Program,
      mode: types::SimulationMode,
    ) -> Result<types::SimulationResult, types::SimulationError>;
  }
}

#[frame::pallet]
pub mod pallet {
  use super::{
    AssetOps, AttemptFeeEnvelope, DexOps, FeeCollector, FeeEnvelopeError, FeeEnvelopeInput,
    FundingAuthority, LiquidityOps, ObservationProvider, TaskWeightInfo, WeightInfo,
    compose_attempt_fee_envelope, execution_plan_steps_bound_is_valid,
  };
  use crate::adapters::{RetryClass, SovereignAccountPolicy, StakingOps as _};
  use frame::prelude::*;
  use polkadot_sdk::{
    frame_support::{PalletId, traits::EnsureOrigin},
    sp_runtime::traits::{CheckedAdd, One, SaturatedConversion, Saturating, Zero},
    sp_weights::{WeightMeter, WeightToFee as _},
  };

  use super::types::Task as AaaTask;
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
    type NativeAssetId: Get<Self::AssetId>;

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
    type MaxContinuationSnapshotEntries: Get<u32>;
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
    type MaxSweepPerBlock: Get<u32>;
    #[pallet::constant]
    type MaxWhitelistSize: Get<u32>;
    #[pallet::constant]
    type MaxTriggerSources: Get<u32>;
    #[pallet::constant]
    type MaxSplitTransferLegs: Get<u32>;
    #[pallet::constant]
    type MaxExecutionDelayBlocks: Get<BlockNumberFor<Self>>;
    #[pallet::constant]
    type MaxTimerJitterBlocks: Get<u32>;
    #[pallet::constant]
    type MaxIdleStarvationBlocks: Get<u32>;
    /// Gross two-dimensional `on_idle` weight guaranteed by the embedding runtime.
    #[pallet::constant]
    type GuaranteedOnIdleWeight: Get<Weight>;
    #[pallet::constant]
    type MaxAutoCloseNonceHorizon: Get<u64>;
    /// Maximum number of active AAA instances. Bounds the BTreeSet storage.
    /// Set to 10,000 for production use cases.
    #[pallet::constant]
    type MaxActiveActors: Get<u32>;
    /// Hard cap across active and dormant actor identities.
    #[pallet::constant]
    type MaxActorIdentities: Get<u32>;
    /// Lifetime cap on allocated System custody locators, including vacant locators.
    #[pallet::constant]
    type MaxSystemSovereigns: Get<u32>;

    /// Per-step flat evaluation cost
    #[pallet::constant]
    type StepBaseFee: Get<Self::Balance>;
    /// Per-condition balance read cost
    #[pallet::constant]
    type ConditionReadFee: Get<Self::Balance>;
    #[pallet::constant]
    type AaaCreationFee: Get<Self::Balance>;
    /// Converts weight to fee for execution cost calculation
    type WeightToFee: polkadot_sdk::sp_weights::WeightToFee<Balance = Self::Balance>;
    /// Runtime-bound upper weights for every AAA task variant
    type TaskWeightInfo: TaskWeightInfo;

    type FeeSink: Get<Self::AccountId>;
    type FeeCollector: FeeCollector<Self::AccountId, Self::AssetId, Self::Balance>;

    #[pallet::constant]
    type MaxConsecutiveFailures: Get<u32>;
    #[pallet::constant]
    type MaxRetryAttempts: Get<u32>;
    #[pallet::constant]
    type MinUserBalance: Get<Self::Balance>;

    type WeightInfo: WeightInfo;

    /// Provides System AAA specs to initialize at genesis.
    /// Use `()` for no genesis System AAAs (default).
    type GenesisSystemAaas: GenesisSystemAaas<
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
  pub type ObservationSubscriberPageOf<T> = ObservationSubscriberPage<<T as Config>::QueuePageSize>;
  pub type ObservationFreeSlotPageOf<T> = BoundedVec<u32, <T as Config>::QueuePageSize>;

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
    ResolutionSurface<<T as Config>::AssetId>,
    <T as Config>::Balance,
    <T as Config>::MaxContinuationSnapshotEntries,
  >;

  pub type ContinuationStateOf<T> = ContinuationState<
    <T as Config>::AssetId,
    <T as Config>::Balance,
    BlockNumberFor<T>,
    <T as Config>::MaxContinuationSnapshotEntries,
    <T as Config>::MaxFundingTrackedAssets,
  >;

  pub type QueuePageOf<T> = BoundedVec<QueueEntry, <T as Config>::QueuePageSize>;
  pub type WakeupPageEntriesOf<T> = BoundedVec<Option<WakeupEntry>, <T as Config>::WakeupPageSize>;
  pub type WakeupPageOf<T> = WakeupPage<WakeupPageEntriesOf<T>>;
  pub type WakeupCursorPageOf<T> = BoundedVec<BlockNumberFor<T>, <T as Config>::WakeupPageSize>;

  pub type AaaInstanceOf<T> = AaaInstance<
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T>,
    ScheduleOf<T>,
    ExecutionPlanOf<T>,
    BalanceOf<T>,
  >;

  pub type ActorHotStateOf<T> = ActorHotState<BlockNumberFor<T>, BalanceOf<T>>;

  pub type ActorProgramStateOf<T> =
    ActorProgramState<ScheduleOf<T>, BlockNumberFor<T>, ExecutionPlanOf<T>>;

  pub type ActorFundingStateOf<T> =
    ActorFundingState<FundingSourcePolicyOf<T>, FundingAccumulatedOf<T>, FundingTrackedAssetsOf<T>>;

  pub type ActorIdentityOf<T> = ActorIdentity<<T as frame_system::Config>::AccountId>;

  #[pallet::pallet]
  #[pallet::storage_version(STORAGE_VERSION)]
  pub struct Pallet<T>(_);

  const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

  #[pallet::storage]
  #[pallet::getter(fn next_aaa_id)]
  pub type NextAaaId<T> = StorageValue<_, AaaId, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn actor_hot)]
  pub type ActorHot<T: Config> =
    StorageMap<_, Blake2_128Concat, AaaId, ActorHotStateOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn actor_program)]
  pub type ActorProgram<T: Config> =
    StorageMap<_, Blake2_128Concat, AaaId, ActorProgramStateOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn actor_funding)]
  pub type ActorFunding<T: Config> =
    StorageMap<_, Blake2_128Concat, AaaId, ActorFundingStateOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::storage_prefix = "ContinuationState"]
  #[pallet::getter(fn continuation_state)]
  pub type ContinuationStateStore<T: Config> =
    StorageMap<_, Blake2_128Concat, AaaId, ContinuationStateOf<T>, OptionQuery>;

  impl<T: Config> Pallet<T> {
    pub(crate) fn compose_active_actor(
      identity: ActorIdentityOf<T>,
      hot: ActorHotStateOf<T>,
      program: ActorProgramStateOf<T>,
    ) -> AaaInstanceOf<T> {
      AaaInstance {
        sovereign_account: identity.sovereign_account,
        owner: identity.owner,
        actor_class: identity.actor_class,
        mutability: identity.mutability,
        lifecycle: hot.lifecycle,
        run_state: hot.run_state,
        schedule: program.schedule,
        schedule_window: program.schedule_window,
        execution_plan: program.execution_plan,
        completion_policy: program.completion_policy,
        cycle_nonce: identity.cycle_nonce,
        auto_close_at_cycle_nonce: hot.auto_close_at_cycle_nonce,
        consecutive_failures: hot.consecutive_failures,
        pending_signal: hot.pending_signal,
        queue_ticket: hot.queue_ticket,
        last_control_queue_mutation_block: hot.last_control_queue_mutation_block,
        cycle_weight_upper: hot.cycle_weight_upper,
        cycle_fee_upper: hot.cycle_fee_upper,
        funding_tracked_count: hot.funding_tracked_count,
        schedule_anchor: hot.schedule_anchor,
        last_cycle_block: hot.last_cycle_block,
      }
    }

    pub(crate) fn active_actor_snapshot(aaa_id: AaaId) -> Option<AaaInstanceOf<T>> {
      Some(Self::compose_active_actor(
        ActorIdentities::<T>::get(aaa_id)?,
        ActorHot::<T>::get(aaa_id)?,
        ActorProgram::<T>::get(aaa_id)?,
      ))
    }

    pub fn pending_signal(aaa_id: AaaId) -> bool {
      ActorHot::<T>::get(aaa_id).is_some_and(|hot| hot.pending_signal)
    }

    pub(crate) fn active_actor_exists(aaa_id: AaaId) -> bool {
      ActorIdentities::<T>::contains_key(aaa_id)
        && ActorHot::<T>::contains_key(aaa_id)
        && ActorProgram::<T>::contains_key(aaa_id)
    }

    fn split_active_actor(
      instance: AaaInstanceOf<T>,
    ) -> (
      ActorIdentityOf<T>,
      ActorHotStateOf<T>,
      ActorProgramStateOf<T>,
    ) {
      (
        ActorIdentity {
          sovereign_account: instance.sovereign_account,
          owner: instance.owner,
          actor_class: instance.actor_class,
          mutability: instance.mutability,
          cycle_nonce: instance.cycle_nonce,
        },
        ActorHotState {
          lifecycle: instance.lifecycle,
          run_state: instance.run_state,
          auto_close_at_cycle_nonce: instance.auto_close_at_cycle_nonce,
          consecutive_failures: instance.consecutive_failures,
          pending_signal: instance.pending_signal,
          queue_ticket: instance.queue_ticket,
          wakeup_pointer: None,
          terminal_at: instance
            .schedule_window
            .map(|window| Self::window_terminal_at(&window)),
          last_control_queue_mutation_block: instance.last_control_queue_mutation_block,
          cycle_weight_upper: instance.cycle_weight_upper,
          cycle_fee_upper: instance.cycle_fee_upper,
          funding_tracked_count: instance.funding_tracked_count,
          schedule_anchor: instance.schedule_anchor,
          last_cycle_block: instance.last_cycle_block,
        },
        ActorProgramState {
          schedule: instance.schedule,
          schedule_window: instance.schedule_window,
          execution_plan: instance.execution_plan,
          completion_policy: instance.completion_policy,
        },
      )
    }

    pub(crate) fn insert_active_actor(aaa_id: AaaId, instance: AaaInstanceOf<T>) -> DispatchResult {
      let (identity, hot, program) = Self::split_active_actor(instance);
      Self::replace_observation_subscriptions(aaa_id, &program.schedule)?;
      ActorIdentities::<T>::insert(aaa_id, identity);
      ActorHot::<T>::insert(aaa_id, hot);
      ActorProgram::<T>::insert(aaa_id, program);
      Ok(())
    }

    pub(crate) fn remove_active_actor(aaa_id: AaaId) -> DispatchResult {
      Self::remove_observation_subscriptions(aaa_id)?;
      ActorHot::<T>::remove(aaa_id);
      ActorProgram::<T>::remove(aaa_id);
      ContinuationStateStore::<T>::remove(aaa_id);
      Ok(())
    }
  }

  #[pallet::storage]
  #[pallet::getter(fn actor_identities)]
  pub type ActorIdentities<T: Config> =
    StorageMap<_, Blake2_128Concat, AaaId, ActorIdentityOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn actor_identity_count)]
  pub type ActorIdentityCount<T> = StorageValue<_, u32, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn active_aaa_count)]
  pub type ActiveAaaCount<T> = StorageValue<_, u32, ValueQuery>;

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
    StorageMap<_, Blake2_128Concat, T::AccountId, AaaId, OptionQuery>;

  /// Explicit nonzero governance-configurable active actor limit.
  #[pallet::storage]
  #[pallet::getter(fn configured_active_actor_limit)]
  pub type ActiveActorLimit<T: Config> = StorageValue<_, u32, ValueQuery>;

  /// Canonical observation feed ownership derived from each active actor's trigger policy.
  #[pallet::storage]
  #[pallet::getter(fn actor_observation_feeds)]
  pub type ActorObservationFeeds<T: Config> =
    StorageMap<_, Blake2_128Concat, AaaId, ActorObservationFeedsOf<T>, OptionQuery>;

  /// Reusable dense slot owned only while an actor has observation subscriptions.
  #[pallet::storage]
  #[pallet::getter(fn observation_subscription_slot)]
  pub type ObservationSubscriptionSlot<T> =
    StorageMap<_, Blake2_128Concat, AaaId, u32, OptionQuery>;

  #[pallet::storage]
  pub type ObservationSubscriptionSlotOwner<T> =
    StorageMap<_, Blake2_128Concat, u32, AaaId, OptionQuery>;

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
  pub type IdleStarvationState<T: Config> =
    StorageValue<_, IdleStarvationPhase<BlockNumberFor<T>>, ValueQuery>;

  /// Provides runtime-specific System AAA instances to initialize at genesis.
  ///
  /// Implement this on the runtime to return System AAA specs with explicit `aaa_id` values.
  /// IDs may be sparse to reserve stable addresses for non-actor accounts.
  pub trait GenesisSystemAaas<AccountId, Schedule, ScheduleWindow, ExecutionPlan> {
    fn system_aaas() -> alloc::vec::Vec<(
      AaaId,
      AccountId,
      Mutability,
      Schedule,
      Option<ScheduleWindow>,
      ExecutionPlan,
      CompletionPolicy,
    )>;

    fn dormant_system_aaas() -> alloc::vec::Vec<(AaaId, AccountId)> {
      alloc::vec::Vec::new()
    }

    /// Runtime-declared deterministic custody accounts that need a provider at genesis
    /// but own no generic AAA identity, program, or scheduler state.
    fn system_custody_accounts() -> alloc::vec::Vec<AaaId> {
      alloc::vec::Vec::new()
    }
  }

  /// Default no-op implementation: no System AAA created at genesis.
  impl<AccountId, Schedule, ScheduleWindowT, ExecutionPlan>
    GenesisSystemAaas<AccountId, Schedule, ScheduleWindowT, ExecutionPlan> for ()
  {
    fn system_aaas() -> alloc::vec::Vec<(
      AaaId,
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
        aaa_id,
        owner,
        mutability,
        schedule,
        schedule_window,
        execution_plan,
        completion_policy,
      ) in T::GenesisSystemAaas::system_aaas()
      {
        assert!(
          !Pallet::<T>::active_actor_exists(aaa_id),
          "duplicate genesis System AAA id: {aaa_id}"
        );
        let next_id = aaa_id
          .checked_add(1)
          .expect("genesis AAA id must not overflow u64");
        if NextAaaId::<T>::get() < next_id {
          NextAaaId::<T>::put(next_id);
        }
        let sovereign_account = Pallet::<T>::sovereign_account_id_system(aaa_id);
        assert!(
          !SovereignIndex::<T>::contains_key(&sovereign_account),
          "genesis System AAA sovereign collision at aaa_id={aaa_id}"
        );
        assert!(
          mutability == Mutability::Mutable || !schedule.trigger.manual_source_enabled(),
          "genesis System Immutable AAA cannot admit Manual readiness"
        );
        Pallet::<T>::validate_execution_plan_shape(AaaType::System, &execution_plan)
          .expect("genesis execution plan must have valid task and condition shapes");
        Pallet::<T>::validate_recipient_configuration(&execution_plan, &sovereign_account)
          .expect("genesis execution plan cannot transfer to its own sovereign account");
        Pallet::<T>::validate_trigger_amount_compatibility(&schedule, &execution_plan)
          .expect("genesis trigger sources must support PercentageOfTrigger semantics");
        Pallet::<T>::ensure_retry_later_allowed(mutability, &execution_plan)
          .expect("genesis System Immutable AAA cannot use RetryLater");
        Pallet::<T>::ensure_execution_plan_fits_idle_budget(AaaType::System, &execution_plan)
          .unwrap_or_else(|_| {
            panic!("genesis System AAA {aaa_id} exceeds the guaranteed on_idle budget")
          });
        let funding_tracked_assets = Pallet::<T>::derive_funding_tracked_assets(&execution_plan)
          .expect("genesis execution_plan must have valid funding-tracked assets");
        let (cycle_weight_upper, cycle_fee_upper) =
          Pallet::<T>::compute_cycle_bounds(AaaType::System, &execution_plan);
        let schedule_anchor = Pallet::<T>::schedule_anchor_at(schedule_window, Zero::zero());
        let instance = AaaInstance {
          sovereign_account: sovereign_account.clone(),
          owner: owner.clone(),
          actor_class: ActorClass::System {
            sovereign_id: aaa_id,
          },
          mutability,
          lifecycle: ActiveLifecycle::Active,
          run_state: RunState::Idle,
          schedule,
          schedule_window,
          execution_plan,
          completion_policy,
          cycle_nonce: 0,
          consecutive_failures: 0,
          pending_signal: false,
          queue_ticket: None,
          last_control_queue_mutation_block: None,
          cycle_weight_upper,
          cycle_fee_upper,
          funding_tracked_count: funding_tracked_assets.len() as u32,
          auto_close_at_cycle_nonce: None,
          schedule_anchor,
          last_cycle_block: None,
        };
        let active_count = Pallet::<T>::active_instance_count();
        assert!(
          active_count < T::MaxActiveActors::get(),
          "genesis active actor capacity exceeded at aaa_id={aaa_id}"
        );
        assert!(
          SystemSovereignCount::<T>::get() < T::MaxSystemSovereigns::get(),
          "genesis System sovereign capacity exceeded at sovereign_id={aaa_id}"
        );
        assert!(
          !SystemSovereigns::<T>::contains_key(aaa_id),
          "duplicate genesis System sovereign locator: {aaa_id}"
        );
        SystemSovereigns::<T>::insert(aaa_id, SystemSovereignState::Occupied(aaa_id));
        SystemSovereignCount::<T>::mutate(|count| *count += 1);
        SovereignIndex::<T>::insert(&sovereign_account, aaa_id);
        frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        Pallet::<T>::insert_active_actor(aaa_id, instance)
          .unwrap_or_else(|error| panic!("genesis observation subscription failed: {error:?}"));
        ActorFunding::<T>::insert(
          aaa_id,
          ActorFundingState {
            funding_source_policy: FundingSourcePolicy::RuntimePolicy,
            funding_accumulated: Default::default(),
            funding_tracked_assets,
          },
        );
        ActiveAaaCount::<T>::put(
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
          "genesis actor identity capacity exceeded at aaa_id={aaa_id}"
        );
        Pallet::<T>::prime_actor_schedule(aaa_id)
          .expect("genesis placement preserves readiness (spec 8.1.4)");
      }
      for (aaa_id, owner) in T::GenesisSystemAaas::dormant_system_aaas() {
        assert!(
          !Pallet::<T>::active_actor_exists(aaa_id) && !ActorIdentities::<T>::contains_key(aaa_id),
          "duplicate genesis System AAA id: {aaa_id}"
        );
        let next_id = aaa_id
          .checked_add(1)
          .expect("genesis AAA id must not overflow u64");
        if NextAaaId::<T>::get() < next_id {
          NextAaaId::<T>::put(next_id);
        }
        let sovereign_account = Pallet::<T>::sovereign_account_id_system(aaa_id);
        assert!(
          !SovereignIndex::<T>::contains_key(&sovereign_account),
          "genesis System AAA sovereign collision at aaa_id={aaa_id}"
        );
        let identity = ActorIdentity {
          sovereign_account: sovereign_account.clone(),
          owner,
          actor_class: ActorClass::System {
            sovereign_id: aaa_id,
          },
          mutability: Mutability::Mutable,
          cycle_nonce: 0,
        };
        let identity_count = ActorIdentityCount::<T>::get();
        assert!(
          identity_count < T::MaxActorIdentities::get(),
          "genesis actor identity capacity exceeded at aaa_id={aaa_id}"
        );
        assert!(
          SystemSovereignCount::<T>::get() < T::MaxSystemSovereigns::get(),
          "genesis System sovereign capacity exceeded at sovereign_id={aaa_id}"
        );
        assert!(
          !SystemSovereigns::<T>::contains_key(aaa_id),
          "duplicate genesis System sovereign locator: {aaa_id}"
        );
        SystemSovereigns::<T>::insert(aaa_id, SystemSovereignState::Occupied(aaa_id));
        SystemSovereignCount::<T>::mutate(|count| *count += 1);
        SovereignIndex::<T>::insert(&sovereign_account, aaa_id);
        frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        ActorIdentities::<T>::insert(aaa_id, identity);
        ActorIdentityCount::<T>::put(
          identity_count
            .checked_add(1)
            .expect("genesis actor identity count must not overflow"),
        );
      }
      for aaa_id in T::GenesisSystemAaas::system_custody_accounts() {
        assert!(
          !Pallet::<T>::active_actor_exists(aaa_id) && !ActorIdentities::<T>::contains_key(aaa_id),
          "genesis custody account collides with actor identity: {aaa_id}"
        );
        let sovereign_account = Pallet::<T>::sovereign_account_id_system(aaa_id);
        assert!(
          !SovereignIndex::<T>::contains_key(&sovereign_account),
          "genesis custody account has generic sovereign index: {aaa_id}"
        );
        assert!(
          SystemSovereignCount::<T>::get() < T::MaxSystemSovereigns::get(),
          "genesis System sovereign capacity exceeded at sovereign_id={aaa_id}"
        );
        assert!(
          !SystemSovereigns::<T>::contains_key(aaa_id),
          "duplicate genesis System sovereign locator: {aaa_id}"
        );
        SystemSovereigns::<T>::insert(aaa_id, SystemSovereignState::Vacant);
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
      assert!(
        T::MaxContinuationSnapshotEntries::get()
          <= T::MaxExecutionPlanSteps::get().saturating_mul(2),
        "MaxContinuationSnapshotEntries must not exceed twice MaxExecutionPlanSteps"
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
    }

    #[cfg(feature = "try-runtime")]
    fn try_state(_n: BlockNumberFor<T>) -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      Self::do_try_state()
    }

    fn on_initialize(_now: BlockNumberFor<T>) -> Weight {
      let _ = GlobalCircuitBreaker::<T>::get();
      T::DbWeight::get().reads(1)
    }

    fn on_idle(now: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
      let base_weight = T::WeightInfo::scheduler_on_idle_base();
      if !base_weight.all_lte(remaining_weight) {
        return Weight::zero();
      }
      let breaker_active = GlobalCircuitBreaker::<T>::get();
      let after_base = remaining_weight.saturating_sub(base_weight);
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
        Self::update_idle_starvation_state(now, false);
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
    AaaCreated {
      aaa_id: AaaId,
      owner: T::AccountId,
      actor_class: ActorClass,
      mutability: Mutability,
      sovereign_account: T::AccountId,
      initial_lifecycle: InitialLifecycle,
    },
    AaaActivated {
      aaa_id: AaaId,
    },
    AaaDeactivated {
      aaa_id: AaaId,
    },
    AaaPaused {
      aaa_id: AaaId,
      reason: PauseReason,
    },
    AaaResumed {
      aaa_id: AaaId,
    },
    AaaClosed {
      aaa_id: AaaId,
      reason: CloseReason,
    },
    CycleDeferred {
      aaa_id: AaaId,
      candidate_cycle_nonce: u64,
      candidate_attempt: u32,
      cursor: u32,
      reason: DeferReason,
    },
    CycleStarted {
      aaa_id: AaaId,
      cycle_nonce: u64,
    },
    CycleSummary {
      aaa_id: AaaId,
      cycle_nonce: u64,
      result: CycleResult,
      executed_steps: u32,
      committed_effectful_tasks: u32,
      skipped_conditions: u32,
      skipped_resolution: u32,
      skipped_funding_unavailable: u32,
      failed_steps: u32,
    },
    StepSkipped {
      aaa_id: AaaId,
      cycle_nonce: u64,
      step_index: u32,
      reason: StepSkippedReason,
    },
    StepFailed {
      aaa_id: AaaId,
      cycle_nonce: u64,
      step_index: u32,
      retry_class: RetryClass,
      error: DispatchError,
    },
    TransferExecuted {
      aaa_id: AaaId,
      cycle_nonce: u64,
      step_index: u32,
      asset: T::AssetId,
      amount: T::Balance,
      to: T::AccountId,
    },
    SplitTransferExecuted {
      aaa_id: AaaId,
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
      aaa_id: AaaId,
      cycle_nonce: u64,
      step_index: u32,
      asset_in: T::AssetId,
      asset_out: T::AssetId,
      amount_in: T::Balance,
      amount_out: T::Balance,
    },
    BurnExecuted {
      aaa_id: AaaId,
      cycle_nonce: u64,
      step_index: u32,
      asset: T::AssetId,
      amount: T::Balance,
    },
    MintExecuted {
      aaa_id: AaaId,
      cycle_nonce: u64,
      step_index: u32,
      asset: T::AssetId,
      amount: T::Balance,
    },
    StakeExecuted {
      aaa_id: AaaId,
      cycle_nonce: u64,
      step_index: u32,
      asset: T::AssetId,
      amount: T::Balance,
    },
    UnstakeExecuted {
      aaa_id: AaaId,
      cycle_nonce: u64,
      step_index: u32,
      asset: T::AssetId,
      shares: T::Balance,
    },
    LiquidityDonated {
      aaa_id: AaaId,
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
      aaa_id: AaaId,
      cycle_nonce: u64,
      step_index: u32,
      asset_a: T::AssetId,
      asset_b: T::AssetId,
      amount_a: T::Balance,
      amount_b: T::Balance,
      lp_minted: T::Balance,
    },
    LiquidityRemoved {
      aaa_id: AaaId,
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
      aaa_id: AaaId,
    },
    ExecutionPlanUpdated {
      aaa_id: AaaId,
      completion_policy: CompletionPolicy,
    },
    AutoCloseNonceSet {
      aaa_id: AaaId,
      target: Option<u64>,
    },
    AutoCloseNonceIncremented {
      aaa_id: AaaId,
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
      aaa_id: AaaId,
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
    FundingSourcePolicyUpdated {
      aaa_id: AaaId,
    },
    FundingAccumulated {
      aaa_id: AaaId,
      asset: T::AssetId,
      added: BalanceOf<T>,
      accumulated: BalanceOf<T>,
    },
    CycleSuspended {
      aaa_id: AaaId,
      cycle_nonce: u64,
      attempt: u32,
      cursor: u32,
      reason: SuspensionReason,
      cumulative_outcomes: OutcomeTotals,
    },
    CycleContinued {
      aaa_id: AaaId,
      cycle_nonce: u64,
      attempt: u32,
      cursor: u32,
    },
    CycleCancelled {
      aaa_id: AaaId,
      cycle_nonce: u64,
      reason: CancellationReason,
    },
    CycleStopped {
      aaa_id: AaaId,
      cycle_nonce: u64,
      step_index: u32,
    },
  }

  #[pallet::error]
  pub enum Error<T> {
    AaaIdOverflow,
    AaaNotFound,
    ActiveAaaCapacityExceeded,
    ActiveAaaCountInvariant,
    ActorIdentityCapacityExceeded,
    ActorIdentityCountInvariant,
    AaaAlreadyActive,
    AaaDormant,
    ActiveAaaLimitExceedsQueueCapacity,
    ActiveAaaLimitTooHigh,
    ActiveAaaLimitTooLow,
    ActiveAaaLimitBelowCurrent,
    AaaPaused,
    EmptyExecutionPlan,
    ExecutionPlanExceedsOnIdleBudget,
    ExecutionDelayTooLong,
    GlobalCircuitBreakerActive,
    ImmutableAaa,
    InsufficientBalance,
    InsufficientFee,
    InvalidAmountResolution,
    InvalidCondition,
    InvalidAutoCloseNonce,
    InvalidScheduleWindow,
    InvalidSplitTransfer,
    SelfTransferNotAllowed,
    InvalidTriggerConfiguration,
    MintNotAllowedForUserAaa,
    NotGovernance,
    NotOwner,
    NotPaused,
    OwnerSlotCapacityExceeded,
    OwnerSlotOccupied,
    InvalidOwnerSlot,
    AaaIdOccupied,
    SystemSovereignCapacityExceeded,
    SystemSovereignUnknown,
    SystemSovereignOccupied,
    ExecutionPlanTooLong,
    SnapshotUnavailable,
    FundingAccumulatorOverflow,
    SovereignAccountCollision,
    ReservedSovereignAccount,
    QueueTicketExhausted,
    SchedulerIndexExhausted,
    SystemSovereignInvariant,
    AutoCloseNonceHorizonExceeded,
    AutoCloseNonceOverflow,
    AutoCloseNonceIncrementZero,
    QueueMutationRateLimited,
    QueueCapacityUnavailable,
    RetryLaterNotAllowedForImmutableAaa,
    ContinuationNotFound,
    ContinuationInvariant,
    EmptyConditionSet,
    ManualSourceDisabled,
    InvalidTradeBound,
    InvalidRetryAttemptLimit,
    RecipientDepositUnavailable,
    InvalidObservationMaxAge,
    ObservationSubscriptionCapacityExceeded,
    ObservationSubscriptionInvariant,
    InvalidObservationRevision,
    DirtyObservationCapacityExceeded,
    DirtyObservationInvariant,
    InvalidTriggerAmountCompatibility,
    AdmissionBoundOverflow,
  }

  #[pallet::call]
  impl<T: Config> Pallet<T> {
    #[pallet::call_index(0)]
    #[pallet::weight(T::WeightInfo::create_user_aaa())]
    pub fn create_user_aaa(
      origin: OriginFor<T>,
      mutability: Mutability,
      program: ProgramInputOf<T>,
    ) -> DispatchResult {
      let owner = ensure_signed(origin)?;
      Self::do_create_user_aaa(owner, mutability, None, program)
    }

    #[pallet::call_index(1)]
    #[pallet::weight(T::WeightInfo::create_user_aaa_at_slot())]
    pub fn create_user_aaa_at_slot(
      origin: OriginFor<T>,
      owner_slot: u8,
      mutability: Mutability,
      program: ProgramInputOf<T>,
    ) -> DispatchResult {
      let owner = ensure_signed(origin)?;
      Self::do_create_user_aaa(owner, mutability, Some(owner_slot), program)
    }

    #[pallet::call_index(2)]
    #[pallet::weight(match &program {
      ProgramInput::Dormant => T::WeightInfo::create_dormant_system_aaa(),
      ProgramInput::Active(_) => T::WeightInfo::create_system_aaa(),
    })]
    pub fn create_system_aaa(
      origin: OriginFor<T>,
      owner: T::AccountId,
      mutability: Mutability,
      program: ProgramInputOf<T>,
    ) -> DispatchResult {
      T::SystemOrigin::ensure_origin(origin)?;
      Self::do_create_system_aaa(owner, mutability, program, None)
    }

    #[pallet::call_index(3)]
    #[pallet::weight(T::WeightInfo::create_system_aaa_at_sovereign_id())]
    pub fn create_system_aaa_at_sovereign_id(
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
      Self::do_create_system_aaa(owner, mutability, program, Some(sovereign_id))
    }

    #[pallet::call_index(4)]
    #[pallet::weight(T::WeightInfo::pause_aaa().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn pause_aaa(origin: OriginFor<T>, aaa_id: AaaId) -> DispatchResult {
      let snapshot = Self::active_actor_snapshot(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_queue_mutation_allowed(&snapshot, now)?;
      ActorHot::<T>::try_mutate(aaa_id, |maybe| -> DispatchResult {
        let inst = maybe.as_mut().ok_or(Error::<T>::AaaNotFound)?;
        ensure!(
          snapshot.mutability == Mutability::Mutable,
          Error::<T>::ImmutableAaa
        );
        ensure!(!inst.lifecycle.is_paused(), Error::<T>::AaaPaused);
        inst.lifecycle = ActiveLifecycle::Paused(PauseReason::Manual);
        inst.queue_ticket = None;
        inst.last_control_queue_mutation_block = Some(now);
        Self::deposit_event(Event::AaaPaused {
          aaa_id,
          reason: PauseReason::Manual,
        });
        Ok(())
      })?;
      Self::prime_actor_schedule(aaa_id).map_err(Self::placement_error)?;
      Ok(())
    }

    #[pallet::call_index(5)]
    #[pallet::weight(T::WeightInfo::resume_aaa().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn resume_aaa(origin: OriginFor<T>, aaa_id: AaaId) -> DispatchResult {
      let snapshot = Self::active_actor_snapshot(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_queue_mutation_allowed(&snapshot, now)?;
      ActorHot::<T>::try_mutate(aaa_id, |maybe| -> DispatchResult {
        let inst = maybe.as_mut().ok_or(Error::<T>::AaaNotFound)?;
        ensure!(
          snapshot.mutability == Mutability::Mutable,
          Error::<T>::ImmutableAaa
        );
        ensure!(inst.lifecycle.is_paused(), Error::<T>::NotPaused);
        inst.lifecycle = ActiveLifecycle::Active;
        inst.last_control_queue_mutation_block = Some(now);
        Self::deposit_event(Event::AaaResumed { aaa_id });
        Ok(())
      })?;
      Self::prime_actor_schedule(aaa_id).map_err(Self::placement_error)?;
      Ok(())
    }

    #[pallet::call_index(6)]
    #[pallet::weight(T::WeightInfo::manual_trigger().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn manual_trigger(origin: OriginFor<T>, aaa_id: AaaId) -> DispatchResult {
      let snapshot = Self::active_actor_snapshot(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      ensure!(!snapshot.lifecycle.is_paused(), Error::<T>::AaaPaused);
      ensure!(
        snapshot.schedule.trigger.manual_source_enabled(),
        Error::<T>::ManualSourceDisabled
      );
      if !snapshot.pending_signal {
        ActorHot::<T>::try_mutate(aaa_id, |maybe| -> DispatchResult {
          let hot = maybe.as_mut().ok_or(Error::<T>::AaaNotFound)?;
          hot.pending_signal = true;
          Ok(())
        })?;
        Self::deposit_event(Event::ManualTriggerSet { aaa_id });
      }
      Self::prime_actor_schedule(aaa_id).map_err(Self::placement_error)?;
      Ok(())
    }

    #[pallet::call_index(7)]
    #[pallet::weight(
      T::WeightInfo::update_funding_source_policy()
        .saturating_add(Pallet::<T>::close_dispatch_weight_upper())
    )]
    pub fn update_funding_source_policy(
      origin: OriginFor<T>,
      aaa_id: AaaId,
      policy: FundingSourcePolicyOf<T>,
    ) -> DispatchResult {
      let instance = Self::active_actor_snapshot(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin, &instance)?;
      Self::ensure_not_system_immutable(&instance)?;
      if Self::is_window_expired(&instance) {
        return Self::close_actor(aaa_id, &instance, CloseReason::WindowExpired);
      }
      ensure!(
        instance.mutability == Mutability::Mutable,
        Error::<T>::ImmutableAaa
      );
      let current_funding = ActorFunding::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      if current_funding.funding_source_policy == policy {
        return Ok(());
      }
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_queue_mutation_allowed(&instance, now)?;
      let continuation_cancelled =
        Self::cancel_continuation_internal(aaa_id, CancellationReason::FundingPolicyChanged, None)?;
      ActorFunding::<T>::mutate(aaa_id, |maybe| {
        maybe
          .as_mut()
          .expect("active actor funding existence was prevalidated")
          .funding_source_policy = policy;
      });
      ActorHot::<T>::mutate(aaa_id, |maybe| {
        maybe
          .as_mut()
          .expect("active actor hot-state existence was prevalidated")
          .last_control_queue_mutation_block = Some(now);
      });
      Self::deposit_event(Event::FundingSourcePolicyUpdated { aaa_id });
      if continuation_cancelled {
        Self::prime_actor_schedule(aaa_id).map_err(|outcome| match Self::enqueue_outcome_error(
          Err(outcome),
        ) {
          Ok(()) => unreachable!("placement error cannot map to Ok"),
          Err(error) => error,
        })?;
      }
      Ok(())
    }

    #[pallet::call_index(8)]
    #[pallet::weight(Pallet::<T>::close_dispatch_weight_upper())]
    pub fn close_aaa(origin: OriginFor<T>, aaa_id: AaaId) -> DispatchResult {
      if let Some(instance) = Self::active_actor_snapshot(aaa_id) {
        Self::ensure_control_origin(origin, &instance)?;
        Self::ensure_not_system_immutable(&instance)?;
        return Self::close_actor(aaa_id, &instance, CloseReason::OwnerInitiated);
      }
      let identity = ActorIdentities::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_identity_control_origin(origin, &identity)?;
      Self::close_inactive_actor(aaa_id, &identity, CloseReason::OwnerInitiated)
    }

    #[pallet::call_index(9)]
    #[pallet::weight(T::WeightInfo::update_schedule().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn update_schedule(
      origin: OriginFor<T>,
      aaa_id: AaaId,
      schedule: ScheduleOf<T>,
      schedule_window: Option<ScheduleWindow<BlockNumberFor<T>>>,
    ) -> DispatchResult {
      Self::validate_schedule(&schedule)?;
      if let Some(ref window) = schedule_window {
        Self::validate_schedule_window(window)?;
      }
      Self::validate_future_schedule_targets(&schedule, schedule_window)?;
      let snapshot = Self::active_actor_snapshot(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      Self::validate_trigger_amount_compatibility(&schedule, &snapshot.execution_plan)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      ensure!(
        snapshot.mutability == Mutability::Mutable,
        Error::<T>::ImmutableAaa
      );
      if snapshot.schedule == schedule && snapshot.schedule_window == schedule_window {
        return Ok(());
      }
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_queue_mutation_allowed(&snapshot, now)?;
      // Semantic schedule replacement resets the Active-epoch anchor unconditionally
      // (spec 4.3); the exact no-op path above already returned without mutation.
      let schedule_anchor = Self::schedule_anchor_at(schedule_window, now);
      Self::preflight_observation_subscription_replace(aaa_id, &schedule)?;
      Self::cancel_continuation_internal(aaa_id, CancellationReason::ScheduleChanged, None)?;
      Self::replace_observation_subscriptions(aaa_id, &schedule)?;
      ActorProgram::<T>::mutate(aaa_id, |maybe| {
        let program = maybe
          .as_mut()
          .expect("active actor program existence was prevalidated");
        program.schedule = schedule;
        program.schedule_window = schedule_window;
      });
      ActorHot::<T>::mutate(aaa_id, |maybe| {
        if let Some(hot) = maybe.as_mut() {
          hot.schedule_anchor = schedule_anchor;
          hot.terminal_at = schedule_window.map(|window| Self::window_terminal_at(&window));
          hot.last_control_queue_mutation_block = Some(now);
        }
      });
      Self::deposit_event(Event::ScheduleUpdated { aaa_id });
      Self::prime_actor_schedule(aaa_id).map_err(|outcome| {
        match Self::enqueue_outcome_error(Err(outcome)) {
          Ok(()) => unreachable!("placement error cannot map to Ok"),
          Err(error) => error,
        }
      })?;
      Ok(())
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
    pub fn permissionless_sweep(origin: OriginFor<T>, aaa_id: AaaId) -> DispatchResult {
      let _who = ensure_signed(origin)?;
      Self::evaluate_actor_liveness(aaa_id)
    }

    #[pallet::call_index(12)]
    #[pallet::weight(T::WeightInfo::update_execution_plan().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn update_execution_plan(
      origin: OriginFor<T>,
      aaa_id: AaaId,
      execution_plan: ExecutionPlanOf<T>,
      completion_policy: CompletionPolicy,
    ) -> DispatchResult {
      ensure!(!execution_plan.is_empty(), Error::<T>::EmptyExecutionPlan);
      let snapshot = Self::active_actor_snapshot(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_retry_later_allowed(snapshot.mutability, &execution_plan)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      ensure!(
        snapshot.mutability == Mutability::Mutable,
        Error::<T>::ImmutableAaa
      );
      let execution_plan_changed = snapshot.execution_plan != execution_plan;
      let completion_policy_changed = snapshot.completion_policy != completion_policy;
      if !execution_plan_changed && !completion_policy_changed {
        return Ok(());
      }
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_queue_mutation_allowed(&snapshot, now)?;
      Self::validate_execution_plan_shape(snapshot.actor_class.aaa_type(), &execution_plan)?;
      Self::validate_recipient_configuration(&execution_plan, &snapshot.sovereign_account)?;
      Self::validate_trigger_amount_compatibility(&snapshot.schedule, &execution_plan)?;
      Self::ensure_execution_plan_fits_idle_budget(
        snapshot.actor_class.aaa_type(),
        &execution_plan,
      )?;
      ensure!(
        (execution_plan.len() as u32) <= T::MaxExecutionPlanSteps::get(),
        Error::<T>::ExecutionPlanTooLong
      );
      if snapshot.actor_class.aaa_type() == AaaType::User {
        ensure!(
          !Self::execution_plan_contains_mint(&execution_plan),
          Error::<T>::MintNotAllowedForUserAaa
        );
      }
      let new_tracked = Self::derive_funding_tracked_assets(&execution_plan)?;
      let mut funding = ActorFunding::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      funding.funding_tracked_assets = new_tracked.clone();
      funding
        .funding_accumulated
        .retain(|asset, _| new_tracked.contains(asset));
      let funding_tracked_count = new_tracked.len() as u32;
      let (cycle_weight_upper, cycle_fee_upper) =
        Self::compute_cycle_bounds(snapshot.actor_class.aaa_type(), &execution_plan);
      let cancellation_reason = if execution_plan_changed {
        CancellationReason::ExecutionPlanChanged
      } else {
        CancellationReason::CompletionPolicyChanged
      };
      let continuation_cancelled =
        Self::cancel_continuation_internal(aaa_id, cancellation_reason, None)?;
      ActorProgram::<T>::mutate(aaa_id, |maybe| {
        let program = maybe
          .as_mut()
          .expect("active actor program existence was prevalidated");
        program.execution_plan = execution_plan;
        program.completion_policy = completion_policy;
      });
      ActorHot::<T>::mutate(aaa_id, |maybe| {
        let hot = maybe
          .as_mut()
          .expect("active actor hot-state existence was prevalidated");
        hot.cycle_weight_upper = cycle_weight_upper;
        hot.cycle_fee_upper = cycle_fee_upper;
        hot.funding_tracked_count = funding_tracked_count;
        hot.consecutive_failures = 0;
        hot.last_control_queue_mutation_block = Some(now);
      });
      ActorFunding::<T>::insert(aaa_id, funding);
      Self::deposit_event(Event::ExecutionPlanUpdated {
        aaa_id,
        completion_policy,
      });
      if continuation_cancelled {
        Self::prime_actor_schedule(aaa_id).map_err(Self::placement_error)?;
      }
      Ok(())
    }

    #[pallet::call_index(13)]
    #[pallet::weight(T::WeightInfo::set_active_actor_limit())]
    pub fn set_active_actor_limit(origin: OriginFor<T>, new_limit: u32) -> DispatchResult {
      T::SystemOrigin::ensure_origin(origin)?;
      ensure!(new_limit > 0, Error::<T>::ActiveAaaLimitTooLow);
      ensure!(
        new_limit <= T::MaxActiveActors::get(),
        Error::<T>::ActiveAaaLimitTooHigh
      );
      ensure!(
        new_limit <= T::MaxQueueLength::get(),
        Error::<T>::ActiveAaaLimitExceedsQueueCapacity
      );
      let active_count = Self::active_instance_count();
      ensure!(
        new_limit >= active_count,
        Error::<T>::ActiveAaaLimitBelowCurrent
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
      T::WeightInfo::permissionless_sweep_many(aaa_ids.len() as u32)
        .saturating_add(Pallet::<T>::close_dispatch_weight_upper().saturating_mul(aaa_ids.len() as u64))
    )]
    pub fn permissionless_sweep_many(
      origin: OriginFor<T>,
      aaa_ids: BoundedVec<AaaId, T::MaxSweepPerBlock>,
    ) -> DispatchResult {
      let _who = ensure_signed(origin)?;
      let mut closed = 0u32;
      let mut alive = 0u32;
      let mut missing = 0u32;
      for aaa_id in aaa_ids
        .iter(/* deos-bypass: bounded-iter — MaxSweepPerBlock input */)
        .copied()
      {
        let Some(instance) = Self::active_actor_snapshot(aaa_id) else {
          missing = missing.saturating_add(1);
          continue;
        };
        if let Some(reason) = Self::liveness_close_reason(&instance) {
          Self::close_actor(aaa_id, &instance, reason)?;
          closed = closed.saturating_add(1);
        } else {
          alive = alive.saturating_add(1);
        }
      }
      Self::deposit_event(Event::SweepBatchProcessed {
        requested: aaa_ids.len() as u32,
        closed,
        alive,
        missing,
      });
      Ok(())
    }

    #[pallet::call_index(15)]
    #[pallet::weight(T::WeightInfo::update_schedule().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn set_auto_close_at_cycle_nonce(
      origin: OriginFor<T>,
      aaa_id: AaaId,
      target: Option<u64>,
    ) -> DispatchResult {
      let snapshot = Self::active_actor_snapshot(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      ensure!(
        snapshot.mutability == Mutability::Mutable,
        Error::<T>::ImmutableAaa
      );
      if let Some(target_nonce) = target {
        Self::ensure_auto_close_target(snapshot.cycle_nonce, target_nonce)?;
      }
      if snapshot.auto_close_at_cycle_nonce == target {
        return Ok(());
      }
      ActorHot::<T>::mutate(aaa_id, |maybe| {
        maybe
          .as_mut()
          .expect("active actor existence was prevalidated")
          .auto_close_at_cycle_nonce = target;
      });
      Self::deposit_event(Event::AutoCloseNonceSet { aaa_id, target });
      Ok(())
    }

    #[pallet::call_index(16)]
    #[pallet::weight(T::WeightInfo::update_schedule().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn increment_auto_close_nonce(
      origin: OriginFor<T>,
      aaa_id: AaaId,
      by: u64,
    ) -> DispatchResult {
      ensure!(by > 0, Error::<T>::AutoCloseNonceIncrementZero);
      let snapshot = Self::active_actor_snapshot(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      ensure!(
        snapshot.mutability == Mutability::Mutable,
        Error::<T>::ImmutableAaa
      );
      let cycle_nonce = snapshot.cycle_nonce;
      ActorHot::<T>::try_mutate(aaa_id, |maybe| -> DispatchResult {
        let inst = maybe.as_mut().ok_or(Error::<T>::AaaNotFound)?;
        let old_target = inst.auto_close_at_cycle_nonce;
        let base = old_target.unwrap_or(cycle_nonce);
        let new_target = base
          .checked_add(by)
          .ok_or(Error::<T>::AutoCloseNonceOverflow)?;
        Self::ensure_auto_close_target(cycle_nonce, new_target)?;
        inst.auto_close_at_cycle_nonce = Some(new_target);
        Self::deposit_event(Event::AutoCloseNonceIncremented {
          aaa_id,
          old_target,
          new_target,
          by,
        });
        Ok(())
      })?;
      Ok(())
    }

    #[pallet::call_index(21)]
    #[pallet::weight(T::WeightInfo::activate_aaa())]
    pub fn activate_aaa(
      origin: OriginFor<T>,
      aaa_id: AaaId,
      program: ProgramInputOf<T>,
    ) -> DispatchResult {
      let identity = ActorIdentities::<T>::get(aaa_id).ok_or_else(|| {
        if Self::active_actor_exists(aaa_id) {
          Error::<T>::AaaAlreadyActive
        } else {
          Error::<T>::AaaNotFound
        }
      })?;
      Self::ensure_identity_control_origin(origin, &identity)?;
      Self::do_activate_aaa(aaa_id, identity, program)
    }

    #[pallet::call_index(22)]
    #[pallet::weight(T::WeightInfo::deactivate_aaa())]
    pub fn deactivate_aaa(origin: OriginFor<T>, aaa_id: AaaId) -> DispatchResult {
      let instance = Self::active_actor_snapshot(aaa_id).ok_or_else(|| {
        if ActorIdentities::<T>::contains_key(aaa_id) {
          Error::<T>::AaaDormant
        } else {
          Error::<T>::AaaNotFound
        }
      })?;
      Self::ensure_control_origin(origin, &instance)?;
      ensure!(
        instance.mutability == Mutability::Mutable,
        Error::<T>::ImmutableAaa
      );
      Self::ensure_control_queue_mutation_allowed(
        &instance,
        frame_system::Pallet::<T>::block_number(),
      )?;
      Self::do_deactivate_aaa(aaa_id, instance)
    }

    #[pallet::call_index(23)]
    #[pallet::weight(T::WeightInfo::continuation_cancel())]
    pub fn cancel_continuation(origin: OriginFor<T>, aaa_id: AaaId) -> DispatchResult {
      let instance = Self::active_actor_snapshot(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin, &instance)?;
      ensure!(
        instance.mutability == Mutability::Mutable,
        Error::<T>::ImmutableAaa
      );
      if Self::is_window_expired(&instance) {
        return Self::close_actor(aaa_id, &instance, CloseReason::WindowExpired);
      }
      ensure!(
        instance.run_state == RunState::Suspended,
        Error::<T>::ContinuationNotFound
      );
      let now = frame_system::Pallet::<T>::block_number();
      Self::ensure_control_queue_mutation_allowed(&instance, now)?;
      ensure!(
        Self::cancel_continuation_internal(aaa_id, CancellationReason::Explicit, None)?,
        Error::<T>::ContinuationNotFound
      );
      ActorHot::<T>::mutate(aaa_id, |maybe| {
        maybe
          .as_mut()
          .expect("cancelled actor remains active")
          .last_control_queue_mutation_block = Some(now);
      });
      Self::prime_actor_schedule(aaa_id).map_err(Self::placement_error)?;
      Ok(())
    }
  }

  impl<T: Config> Pallet<T> {
    pub fn aaa_instances(aaa_id: AaaId) -> Option<AaaInstanceOf<T>> {
      Self::active_actor_snapshot(aaa_id)
    }

    pub fn weight_upper_bound(task: &TaskOf<T>) -> Weight {
      // Runtime owns upper-bound pricing via coarse task classes to reduce calibration churn
      match task {
        AaaTask::Transfer { .. } => T::TaskWeightInfo::transfer(),
        AaaTask::Burn { .. } => T::TaskWeightInfo::burn(),
        AaaTask::Mint { .. } => T::TaskWeightInfo::mint(),
        AaaTask::SplitTransfer { legs, .. } => T::TaskWeightInfo::split_transfer(legs.len() as u32),
        AaaTask::SwapIn { .. } => T::TaskWeightInfo::dex_exact_in(),
        AaaTask::SwapOut { .. } => T::TaskWeightInfo::dex_exact_out(),
        AaaTask::AddLiquidity { .. } => T::TaskWeightInfo::add_liquidity(),
        AaaTask::RemoveLiquidity { .. } => T::TaskWeightInfo::remove_liquidity(),
        AaaTask::Stake { .. } => T::TaskWeightInfo::stake(),
        AaaTask::DonateLiquidity { .. } => T::TaskWeightInfo::donate_liquidity(),
        AaaTask::Unstake { .. } => T::TaskWeightInfo::unstake(),
        AaaTask::StopCycle => T::TaskWeightInfo::stop_cycle(),
      }
    }

    /// Conservative FRAME dispatch weight for explicit or lifecycle-touch pure cleanup.
    pub fn close_dispatch_weight_upper() -> Weight {
      Self::close_cleanup_weight_upper()
    }

    pub(crate) fn compute_cycle_weight_upper_from(
      aaa_type: AaaType,
      execution_plan: &ExecutionPlanOf<T>,
      start_cursor: usize,
    ) -> Weight {
      let mut upper =
        Weight::from_parts(5_000_000, 1000).saturating_add(T::DbWeight::get().reads_writes(2, 2));
      for step_index in start_cursor..execution_plan.len() {
        let step = &execution_plan[step_index];
        let step_overhead = Weight::from_parts(1_000_000, 128);
        let condition_evaluation = T::WeightInfo::condition_set_evaluation(step.conditions.len());
        upper = upper
          .saturating_add(step_overhead)
          .saturating_add(condition_evaluation)
          .saturating_add(Self::weight_upper_bound(&step.task));
        if aaa_type == AaaType::User {
          upper = upper.saturating_add(T::WeightInfo::fee_collection());
        }
      }
      if (start_cursor..execution_plan.len()).any(|step_index| {
        execution_plan[step_index]
          .on_error
          .retry_max_attempts()
          .is_some()
      }) {
        let snapshot_entries = Self::trigger_surfaces(execution_plan, start_cursor).len() as u32;
        upper = upper.saturating_add(
          T::WeightInfo::continuation_suspend(snapshot_entries)
            .max(T::WeightInfo::continuation_complete())
            .max(T::WeightInfo::continuation_cancel()),
        );
      }
      upper
    }

    pub(crate) fn compute_cycle_weight_upper(
      aaa_type: AaaType,
      execution_plan: &ExecutionPlanOf<T>,
    ) -> Weight {
      Self::compute_cycle_weight_upper_from(aaa_type, execution_plan, 0)
    }

    pub fn attempt_fee_envelope(
      aaa_type: AaaType,
      execution_plan: &ExecutionPlanOf<T>,
      start_cursor: usize,
    ) -> Result<AttemptFeeEnvelopeOf<T>, Error<T>> {
      let mut inputs = BoundedVec::default();
      for step in execution_plan {
        let evaluation = if aaa_type == AaaType::User {
          Self::compute_eval_fee_checked(step.conditions.len())?
        } else {
          Zero::zero()
        };
        let execution = if aaa_type == AaaType::User {
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
      compose_attempt_fee_envelope(aaa_type, &inputs, start_cursor).map_err(|error| match error {
        FeeEnvelopeError::CursorOutOfBounds | FeeEnvelopeError::ReservationUnderflow => {
          Error::<T>::ContinuationInvariant
        }
        FeeEnvelopeError::Overflow => Error::<T>::AdmissionBoundOverflow,
      })
    }

    pub(crate) fn compute_cycle_fee_upper_from(
      aaa_type: AaaType,
      execution_plan: &ExecutionPlanOf<T>,
      start_cursor: usize,
    ) -> BalanceOf<T> {
      Self::attempt_fee_envelope(aaa_type, execution_plan, start_cursor)
        .expect("admitted execution plans have a checked fee envelope")
        .total
    }

    pub(crate) fn compute_cycle_fee_upper(
      aaa_type: AaaType,
      execution_plan: &ExecutionPlanOf<T>,
    ) -> BalanceOf<T> {
      Self::compute_cycle_fee_upper_from(aaa_type, execution_plan, 0)
    }

    pub(crate) fn compute_cycle_bounds(
      aaa_type: AaaType,
      execution_plan: &ExecutionPlanOf<T>,
    ) -> (Weight, BalanceOf<T>) {
      (
        Self::compute_cycle_weight_upper(aaa_type, execution_plan),
        Self::compute_cycle_fee_upper(aaa_type, execution_plan),
      )
    }

    pub(crate) fn cycle_weight_upper_bound(instance: &AaaInstanceOf<T>) -> Weight {
      instance.cycle_weight_upper
    }

    pub(crate) fn attempt_weight_upper_bound(
      instance: &AaaInstanceOf<T>,
      start_cursor: usize,
    ) -> Weight {
      let mut upper = if start_cursor == 0 {
        Self::cycle_weight_upper_bound(instance)
      } else {
        Self::compute_cycle_weight_upper_from(
          instance.actor_class.aaa_type(),
          &instance.execution_plan,
          start_cursor,
        )
      };
      if instance.run_state == RunState::Suspended {
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

    pub(crate) fn cycle_fee_upper_bound(instance: &AaaInstanceOf<T>) -> BalanceOf<T> {
      instance.cycle_fee_upper
    }

    pub(crate) fn attempt_fee_upper_bound(
      instance: &AaaInstanceOf<T>,
      start_cursor: usize,
    ) -> BalanceOf<T> {
      let envelope = Self::attempt_fee_envelope(
        instance.actor_class.aaa_type(),
        &instance.execution_plan,
        start_cursor,
      )
      .expect("admitted execution plans have a checked fee envelope");
      if start_cursor == 0 {
        debug_assert_eq!(instance.cycle_fee_upper, envelope.total);
      }
      envelope.total
    }

    pub(crate) fn close_cycle_weight_upper_bound(_instance: &AaaInstanceOf<T>) -> Weight {
      Self::close_cleanup_weight_upper()
    }

    /// Upper-bounds one prospective run plus pure terminal cleanup after the baseline scheduler
    /// envelope. Independently metered durable housekeeping may defer this work across blocks.
    pub fn execution_plan_admission_weight_upper(
      aaa_type: AaaType,
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
        .saturating_add(Self::compute_cycle_weight_upper(aaa_type, execution_plan))
        .saturating_add(continuation_retry)
        .saturating_add(snapshot_open)
        .saturating_add(Self::close_cleanup_weight_upper())
    }

    fn ensure_execution_plan_fits_idle_budget(
      aaa_type: AaaType,
      execution_plan: &ExecutionPlanOf<T>,
    ) -> DispatchResult {
      ensure!(
        Self::execution_plan_admission_weight_upper(aaa_type, execution_plan)
          .all_lte(T::GuaranteedOnIdleWeight::get()),
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
      bitmap
        .iter() // deos-bypass: bounded-iter — fixed 32-byte OwnerSlotBitmap validity check.
        .enumerate()
        .all(|(index, byte)| {
          if index < full_bytes {
            return true;
          }
          if index == full_bytes && remaining_bits > 0 {
            return *byte & !((1u8 << remaining_bits) - 1) == 0;
          }
          *byte == 0
        })
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
      bitmap
        .iter() // deos-bypass: bounded-iter — fixed 32-byte OwnerSlotBitmap emptiness check.
        .all(|byte| *byte == 0)
    }

    fn charge_creation_fee(owner: &T::AccountId) -> DispatchResult {
      let creation_fee = T::AaaCreationFee::get();
      if creation_fee.is_zero() {
        return Ok(());
      }
      let native = T::NativeAssetId::get();
      let fee_sink = T::FeeSink::get();
      T::FeeCollector::collect_fee(owner, &fee_sink, native, creation_fee)
        .map_err(|_| Error::<T>::InsufficientFee.into())
    }

    pub fn sovereign_account_id(owner: &T::AccountId, owner_slot: u8) -> T::AccountId {
      let seed = frame::hashing::blake2_256(&(T::PalletId::get(), owner, owner_slot).encode());
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::new(&seed))
        .expect("hashed seed always decodes into AccountId")
    }

    pub fn sovereign_account_id_system(aaa_id: AaaId) -> T::AccountId {
      let seed = frame::hashing::blake2_256(&(T::PalletId::get(), b"system", aaa_id).encode());
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::new(&seed))
        .expect("hashed seed always decodes into AccountId")
    }

    fn available_owner_slot(
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
          for (byte_index, byte) in bitmap
            .iter() // deos-bypass: bounded-iter — fixed 32-byte lowest-free-slot scan.
            .enumerate()
          {
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
            let free_bits = !*byte & valid_bits;
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

    fn allocate_system_sovereign(aaa_id: AaaId) -> Result<T::AccountId, Error<T>> {
      let sovereign_account = Self::sovereign_account_id_system(aaa_id);
      // Context-aware reservation: a fresh (unregistered) derivation that aliases a
      // host-reserved account fails ReservedSovereignAccount; reattachment to an
      // existing registered Vacant locator is allowed for that exact locator even
      // when its account belongs to the genesis System custody range, so the locator
      // is not permanently unrecoverable after close (spec 5.4).
      let is_registered_reattachment =
        SystemSovereigns::<T>::get(aaa_id) == Some(SystemSovereignState::Vacant);
      if !is_registered_reattachment && T::SovereignAccountPolicy::is_reserved(&sovereign_account) {
        return Err(Error::<T>::ReservedSovereignAccount);
      }
      if SovereignIndex::<T>::contains_key(&sovereign_account) {
        return Err(Error::<T>::SovereignAccountCollision);
      }
      Ok(sovereign_account)
    }

    fn do_create_dormant_aaa(
      owner: T::AccountId,
      aaa_type: AaaType,
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
      let aaa_id = NextAaaId::<T>::get();
      ensure!(
        !Self::active_actor_exists(aaa_id) && !ActorIdentities::<T>::contains_key(aaa_id),
        Error::<T>::AaaIdOccupied
      );
      let next_id = aaa_id.checked_add(1).ok_or(Error::<T>::AaaIdOverflow)?;
      let system_sovereign_id = requested_system_sovereign_id.unwrap_or(aaa_id);
      if aaa_type == AaaType::System {
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
      let mut created_identity: Option<ActorIdentityOf<T>> = None;
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        if aaa_type == AaaType::User {
          if let Err(error) = Self::charge_creation_fee(&owner) {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
          }
        }
        let (owner_slot, sovereign_account) = match aaa_type {
          AaaType::User => match Self::allocate_owner_slot(&owner, preferred_user_slot) {
            Ok((slot, account)) => (Some(slot), account),
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error.into(),
              ));
            }
          },
          AaaType::System => match Self::allocate_system_sovereign(system_sovereign_id) {
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
          actor_class: match aaa_type {
            AaaType::User => ActorClass::User {
              owner_slot: owner_slot.expect("User allocation always returns a slot"),
            },
            AaaType::System => ActorClass::System {
              sovereign_id: system_sovereign_id,
            },
          },
          mutability: Mutability::Mutable,
          cycle_nonce: 0,
        };
        SovereignIndex::<T>::insert(&sovereign_account, aaa_id);
        ActorIdentities::<T>::insert(aaa_id, &identity);
        if let Err(error) = ActorIdentityCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_add(1)
            .ok_or(Error::<T>::ActorIdentityCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if aaa_type == AaaType::System {
          SystemSovereigns::<T>::insert(
            system_sovereign_id,
            SystemSovereignState::Occupied(aaa_id),
          );
          if requested_system_sovereign_id.is_none() {
            SystemSovereignCount::<T>::mutate(|count| *count += 1);
          }
        }
        NextAaaId::<T>::put(next_id);
        if aaa_type == AaaType::User || requested_system_sovereign_id.is_none() {
          frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        }
        created_identity = Some(identity);
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })?;
      let identity = created_identity.expect("atomic dormant create always sets identity");
      Self::deposit_event(Event::AaaCreated {
        aaa_id,
        owner,
        actor_class: identity.actor_class,
        mutability: Mutability::Mutable,
        sovereign_account: identity.sovereign_account,
        initial_lifecycle: InitialLifecycle::Dormant,
      });
      Ok(())
    }

    fn do_create_user_aaa(
      owner: T::AccountId,
      mutability: Mutability,
      preferred_slot: Option<u8>,
      program: ProgramInputOf<T>,
    ) -> DispatchResult {
      match program {
        ProgramInput::Dormant => {
          ensure!(mutability == Mutability::Mutable, Error::<T>::ImmutableAaa);
          Self::do_create_dormant_aaa(owner, AaaType::User, preferred_slot, None)
        }
        ProgramInput::Active(ActiveProgramInput {
          schedule,
          schedule_window,
          execution_plan,
          completion_policy,
          funding_source_policy,
          auto_close_at_cycle_nonce,
        }) => Self::do_create_aaa(
          owner,
          AaaType::User,
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

    fn do_create_system_aaa(
      owner: T::AccountId,
      mutability: Mutability,
      program: ProgramInputOf<T>,
      requested_system_sovereign_id: Option<SystemSovereignId>,
    ) -> DispatchResult {
      match program {
        ProgramInput::Dormant => {
          ensure!(mutability == Mutability::Mutable, Error::<T>::ImmutableAaa);
          Self::do_create_dormant_aaa(owner, AaaType::System, None, requested_system_sovereign_id)
        }
        ProgramInput::Active(ActiveProgramInput {
          schedule,
          schedule_window,
          execution_plan,
          completion_policy,
          funding_source_policy,
          auto_close_at_cycle_nonce,
        }) => Self::do_create_aaa(
          owner,
          AaaType::System,
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

    fn do_create_aaa(
      owner: T::AccountId,
      aaa_type: AaaType,
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
      if aaa_type == AaaType::User {
        ensure!(
          !Self::execution_plan_contains_mint(&execution_plan),
          Error::<T>::MintNotAllowedForUserAaa
        );
      }
      Self::validate_schedule(&schedule)?;
      if let Some(ref window) = schedule_window {
        Self::validate_schedule_window(window)?;
      }
      Self::validate_future_schedule_targets(&schedule, schedule_window)?;
      Self::validate_execution_plan_shape(aaa_type, &execution_plan)?;
      Self::validate_trigger_amount_compatibility(&schedule, &execution_plan)?;
      Self::ensure_retry_later_allowed(mutability, &execution_plan)?;
      if let Some(target_nonce) = auto_close_at_cycle_nonce {
        Self::ensure_auto_close_target(0, target_nonce)?;
      }
      if aaa_type == AaaType::System && mutability == Mutability::Immutable {
        ensure!(
          !schedule.trigger.manual_source_enabled(),
          Error::<T>::InvalidTriggerConfiguration
        );
      }
      let active_count = Self::active_instance_count();
      ensure!(
        active_count < Self::effective_active_actor_limit(),
        Error::<T>::ActiveAaaCapacityExceeded
      );
      ensure!(
        ActorIdentityCount::<T>::get() < T::MaxActorIdentities::get(),
        Error::<T>::ActorIdentityCapacityExceeded
      );
      Self::ensure_execution_plan_fits_idle_budget(aaa_type, &execution_plan)?;
      let funding_tracked_assets = Self::derive_funding_tracked_assets(&execution_plan)?;
      let aaa_id = NextAaaId::<T>::get();
      ensure!(
        !Self::active_actor_exists(aaa_id) && !ActorIdentities::<T>::contains_key(aaa_id),
        Error::<T>::AaaIdOccupied
      );
      let system_sovereign_id = requested_system_sovereign_id.unwrap_or(aaa_id);
      if aaa_type == AaaType::System {
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
      let prospective_sovereign_account = match aaa_type {
        AaaType::User => {
          let owner_slot = Self::available_owner_slot(&owner, preferred_user_slot)?;
          Self::sovereign_account_id(&owner, owner_slot)
        }
        AaaType::System => Self::sovereign_account_id_system(system_sovereign_id),
      };
      Self::validate_recipient_configuration(&execution_plan, &prospective_sovereign_account)?;
      let next_id = aaa_id.checked_add(1).ok_or(Error::<T>::AaaIdOverflow)?;
      let now = frame_system::Pallet::<T>::block_number();
      let mut created_owner_slot: Option<u8> = None;
      let mut created_sovereign_account: Option<T::AccountId> = None;
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        if aaa_type == AaaType::User {
          if let Err(error) = Self::charge_creation_fee(&owner) {
            return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
          }
        }
        let (owner_slot, sovereign_account) = match aaa_type {
          AaaType::User => match Self::allocate_owner_slot(&owner, preferred_user_slot) {
            Ok((slot, account)) => (Some(slot), account),
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error.into(),
              ));
            }
          },
          AaaType::System => match Self::allocate_system_sovereign(system_sovereign_id) {
            Ok(account) => (None, account),
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error.into(),
              ));
            }
          },
        };
        let (cycle_weight_upper, cycle_fee_upper) =
          Self::compute_cycle_bounds(aaa_type, &execution_plan);
        let schedule_anchor = Self::schedule_anchor_at(schedule_window, now);
        let instance = AaaInstance {
          sovereign_account: sovereign_account.clone(),
          owner: owner.clone(),
          actor_class: match aaa_type {
            AaaType::User => ActorClass::User {
              owner_slot: owner_slot.expect("User allocation always returns a slot"),
            },
            AaaType::System => ActorClass::System {
              sovereign_id: system_sovereign_id,
            },
          },
          mutability,
          lifecycle: ActiveLifecycle::Active,
          run_state: RunState::Idle,
          schedule,
          schedule_window,
          execution_plan,
          completion_policy,
          cycle_nonce: 0,
          consecutive_failures: 0,
          pending_signal: false,
          queue_ticket: None,
          last_control_queue_mutation_block: None,
          cycle_weight_upper,
          cycle_fee_upper,
          funding_tracked_count: funding_tracked_assets.len() as u32,
          auto_close_at_cycle_nonce,
          schedule_anchor,
          last_cycle_block: None,
        };
        created_owner_slot = owner_slot;
        created_sovereign_account = Some(sovereign_account.clone());
        SovereignIndex::<T>::insert(sovereign_account.clone(), aaa_id);
        if let Err(error) = Self::insert_active_actor(aaa_id, instance) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        ActorFunding::<T>::insert(
          aaa_id,
          ActorFundingState {
            funding_source_policy,
            funding_accumulated: Default::default(),
            funding_tracked_assets,
          },
        );
        if let Err(error) = ActiveAaaCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_add(1)
            .ok_or(Error::<T>::ActiveAaaCountInvariant)?;
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
        if aaa_type == AaaType::System {
          SystemSovereigns::<T>::insert(
            system_sovereign_id,
            SystemSovereignState::Occupied(aaa_id),
          );
          if requested_system_sovereign_id.is_none() {
            SystemSovereignCount::<T>::mutate(|count| *count += 1);
          }
        }
        NextAaaId::<T>::put(next_id);
        if aaa_type == AaaType::System && requested_system_sovereign_id.is_none() {
          frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        }
        #[cfg(test)]
        if let Err(error) = crate::mock::create_atomicity_checkpoint(aaa_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })?;
      let sovereign_account =
        created_sovereign_account.expect("atomic create always sets sovereign_account");
      Self::deposit_event(Event::AaaCreated {
        aaa_id,
        owner,
        actor_class: match aaa_type {
          AaaType::User => ActorClass::User {
            owner_slot: created_owner_slot.expect("User allocation always returns a slot"),
          },
          AaaType::System => ActorClass::System {
            sovereign_id: system_sovereign_id,
          },
        },
        mutability,
        sovereign_account,
        initial_lifecycle: InitialLifecycle::Active,
      });
      Self::prime_actor_schedule(aaa_id).map_err(Self::placement_error)?;
      Ok(())
    }

    fn do_activate_aaa(
      aaa_id: AaaId,
      identity: ActorIdentityOf<T>,
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
        Error::<T>::ImmutableAaa
      );
      let aaa_type = identity.actor_class.aaa_type();
      ensure!(!execution_plan.is_empty(), Error::<T>::EmptyExecutionPlan);
      ensure!(
        (execution_plan.len() as u32) <= T::MaxExecutionPlanSteps::get(),
        Error::<T>::ExecutionPlanTooLong
      );
      if aaa_type == AaaType::User {
        ensure!(
          !Self::execution_plan_contains_mint(&execution_plan),
          Error::<T>::MintNotAllowedForUserAaa
        );
      }
      Self::validate_schedule(&schedule)?;
      if let Some(ref window) = schedule_window {
        Self::validate_schedule_window(window)?;
      }
      Self::validate_future_schedule_targets(&schedule, schedule_window)?;
      Self::validate_execution_plan_shape(aaa_type, &execution_plan)?;
      Self::validate_recipient_configuration(&execution_plan, &identity.sovereign_account)?;
      Self::validate_trigger_amount_compatibility(&schedule, &execution_plan)?;
      Self::ensure_retry_later_allowed(identity.mutability, &execution_plan)?;
      if let Some(target_nonce) = auto_close_at_cycle_nonce {
        Self::ensure_auto_close_target(identity.cycle_nonce, target_nonce)?;
      }
      Self::ensure_execution_plan_fits_idle_budget(aaa_type, &execution_plan)?;
      let funding_tracked_assets = Self::derive_funding_tracked_assets(&execution_plan)?;
      ensure!(
        Self::active_instance_count() < Self::effective_active_actor_limit(),
        Error::<T>::ActiveAaaCapacityExceeded
      );
      let (cycle_weight_upper, cycle_fee_upper) =
        Self::compute_cycle_bounds(aaa_type, &execution_plan);
      let now = frame_system::Pallet::<T>::block_number();
      // Reactivation anchors the fresh Active epoch at the current block; the fresh hot
      // state has no last_cycle_block, so cooldown/cadence use this conservative anchor
      // rather than block zero (spec 4.3.3).
      let schedule_anchor = Self::schedule_anchor_at(schedule_window, now);
      let instance = AaaInstance {
        sovereign_account: identity.sovereign_account,
        owner: identity.owner,
        actor_class: identity.actor_class,
        mutability: identity.mutability,
        lifecycle: ActiveLifecycle::Active,
        run_state: RunState::Idle,
        schedule,
        schedule_window,
        execution_plan,
        completion_policy,
        cycle_nonce: identity.cycle_nonce,
        consecutive_failures: 0,
        pending_signal: false,
        queue_ticket: None,
        last_control_queue_mutation_block: Some(now),
        cycle_weight_upper,
        cycle_fee_upper,
        funding_tracked_count: funding_tracked_assets.len() as u32,
        auto_close_at_cycle_nonce,
        schedule_anchor,
        last_cycle_block: None,
      };
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        if !ActorIdentities::<T>::contains_key(aaa_id) || Self::active_actor_exists(aaa_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::AaaAlreadyActive.into(),
          ));
        }
        if let Err(error) = Self::insert_active_actor(aaa_id, instance) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        ActorFunding::<T>::insert(
          aaa_id,
          ActorFundingState {
            funding_source_policy,
            funding_accumulated: Default::default(),
            funding_tracked_assets,
          },
        );
        if let Err(error) = ActiveAaaCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_add(1)
            .ok_or(Error::<T>::ActiveAaaCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })?;
      Self::deposit_event(Event::AaaActivated { aaa_id });
      Self::prime_actor_schedule(aaa_id).map_err(Self::placement_error)?;
      Ok(())
    }

    fn do_deactivate_aaa(aaa_id: AaaId, _instance: AaaInstanceOf<T>) -> DispatchResult {
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        if let Err(error) =
          Self::cancel_continuation_internal(aaa_id, CancellationReason::Deactivated, None)
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        Self::remove_actor_from_queues(aaa_id);
        if ActorHot::<T>::get(aaa_id).is_some_and(|hot| hot.wakeup_pointer.is_some())
          && Self::wakeup_substrate_invalidate(aaa_id).is_none()
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::AaaNotFound.into(),
          ));
        }
        if let Err(error) = Self::remove_active_actor(aaa_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        ActorFunding::<T>::remove(aaa_id);
        if let Err(error) = ActiveAaaCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_sub(1)
            .ok_or(Error::<T>::ActiveAaaCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })?;
      Self::deposit_event(Event::AaaDeactivated { aaa_id });
      Ok(())
    }

    fn execution_plan_contains_mint(execution_plan: &ExecutionPlanOf<T>) -> bool {
      execution_plan
        .iter() // deos-bypass: bounded-iter — MaxExecutionPlanSteps plan
        .any(|step| matches!(step.task, AaaTask::Mint { .. }))
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
      if let TriggerPolicy::Cadenced { every_blocks, .. } = schedule.trigger {
        let cadence: BlockNumberFor<T> = every_blocks.into();
        let jitter_window = every_blocks
          .saturating_div(4)
          .min(T::MaxTimerJitterBlocks::get());
        let worst_case_jitter: BlockNumberFor<T> = jitter_window.saturating_sub(1).into();
        ensure!(
          schedule_anchor
            .checked_add(&cadence)
            .and_then(|target| target.checked_add(&worst_case_jitter))
            .is_some(),
          Error::<T>::SchedulerIndexExhausted
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
            Error::<T>::RetryLaterNotAllowedForImmutableAaa
          );
        }
      }
      Ok(())
    }

    fn validate_execution_plan_shape(
      aaa_type: AaaType,
      execution_plan: &ExecutionPlanOf<T>,
    ) -> DispatchResult {
      ensure!(
        execution_plan_steps_bound_is_valid(T::MaxExecutionPlanSteps::get()),
        Error::<T>::ExecutionPlanTooLong
      );
      Self::attempt_fee_envelope(aaa_type, execution_plan, 0)?;
      for step in execution_plan
        .iter(/* deos-bypass: bounded-iter — MaxExecutionPlanSteps plan */)
      {
        if let Some(max_attempts) = step.on_error.retry_max_attempts() {
          ensure!(
            max_attempts > 0 && max_attempts <= T::MaxRetryAttempts::get(),
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
          AaaTask::Transfer { amount, .. }
          | AaaTask::Burn { amount, .. }
          | AaaTask::Mint { amount, .. }
          | AaaTask::Stake { amount, .. } => Self::validate_amount_resolution(amount)?,
          AaaTask::SplitTransfer { amount, legs, .. } => {
            Self::validate_amount_resolution(amount)?;
            Self::validate_split_transfer_legs(legs)?;
          }
          AaaTask::SwapIn {
            asset_in,
            amount_in,
            asset_out,
            ..
          } => {
            ensure!(asset_in != asset_out, Error::<T>::InvalidTradeBound);
            Self::validate_amount_resolution(amount_in)?;
          }
          AaaTask::SwapOut {
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
          AaaTask::AddLiquidity {
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
          AaaTask::RemoveLiquidity {
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
          AaaTask::DonateLiquidity {
            asset_a,
            asset_b,
            max_amount_a,
            ..
          } => {
            ensure!(asset_a != asset_b, Error::<T>::InvalidTradeBound);
            Self::validate_amount_resolution(max_amount_a)?;
          }
          AaaTask::Unstake { shares, .. } => Self::validate_amount_resolution(shares)?,
          AaaTask::StopCycle => {}
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
          AaaTask::Transfer { to, .. } => {
            ensure!(to != sovereign_account, Error::<T>::SelfTransferNotAllowed);
          }
          AaaTask::SplitTransfer { legs, .. } => {
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
              | AmountResolution::PercentageOfTrigger(value)
              | AmountResolution::PercentageOfLastFunding(value)
              if value.is_zero()
          ),
        Error::<T>::InvalidAmountResolution
      );
      Ok(())
    }

    fn validate_trigger_amount_compatibility(
      schedule: &ScheduleOf<T>,
      execution_plan: &ExecutionPlanOf<T>,
    ) -> DispatchResult {
      let surfaces = Self::trigger_surfaces(execution_plan, 0);
      if surfaces.is_empty() {
        return Ok(());
      }
      let mut ingress_assets = alloc::vec::Vec::new();
      for surface in surfaces {
        let asset = match surface {
          ResolutionSurface::Asset(asset) => asset,
          ResolutionSurface::StakingShares(position_asset) => {
            T::StakingOps::share_asset(position_asset)
              .ok_or(Error::<T>::InvalidTriggerAmountCompatibility)?
          }
        };
        if !ingress_assets.contains(&asset) {
          ingress_assets.push(asset);
        }
      }
      let sources = schedule
        .trigger
        .sources()
        .ok_or(Error::<T>::InvalidTriggerAmountCompatibility)?;
      ensure!(
        !sources.is_empty(),
        Error::<T>::InvalidTriggerAmountCompatibility
      );
      for source in sources {
        let TriggerSource::OnAddressEvent { asset_filter, .. } = source else {
          return Err(Error::<T>::InvalidTriggerAmountCompatibility.into());
        };
        ensure!(
          ingress_assets
            .iter() // deos-bypass: bounded-iter — execution-plan trigger surfaces are bounded by MaxContinuationSnapshotEntries.
            .all(|asset| match asset_filter {
              AssetFilter::Any => true,
              AssetFilter::Whitelist(assets) => assets.contains(asset),
            }),
          Error::<T>::InvalidTriggerAmountCompatibility
        );
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

      for step in execution_plan
        .iter(/* deos-bypass: bounded-iter — MaxExecutionPlanSteps plan */)
      {
        match &step.task {
          AaaTask::Transfer { asset, amount, .. }
          | AaaTask::SplitTransfer { asset, amount, .. }
          | AaaTask::Burn { asset, amount }
          | AaaTask::Mint { asset, amount } => {
            check_amount(amount, *asset);
          }
          AaaTask::RemoveLiquidity {
            lp_asset: asset,
            lp_amount,
            ..
          } => {
            check_amount(lp_amount, *asset);
          }
          AaaTask::SwapIn {
            asset_in,
            amount_in,
            ..
          } => {
            check_amount(amount_in, *asset_in);
          }
          AaaTask::SwapOut {
            asset_out,
            amount_out,
            ..
          } => {
            check_amount(amount_out, *asset_out);
          }
          AaaTask::AddLiquidity {
            asset_a,
            asset_b,
            amount_a,
            amount_b,
            ..
          } => {
            check_amount(amount_a, *asset_a);
            check_amount(amount_b, *asset_b);
          }
          AaaTask::Stake { asset, amount } => {
            check_amount(amount, *asset);
          }
          AaaTask::DonateLiquidity {
            asset_a,
            max_amount_a,
            ..
          } => {
            check_amount(max_amount_a, *asset_a);
          }
          AaaTask::Unstake { asset, shares } => {
            if matches!(shares, AmountResolution::PercentageOfLastFunding(_)) {
              let share_asset =
                T::StakingOps::share_asset(*asset).ok_or(Error::<T>::InvalidAmountResolution)?;
              check_amount(shares, share_asset);
            }
          }
          AaaTask::StopCycle => {}
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

    fn ensure_not_system_immutable(instance: &AaaInstanceOf<T>) -> DispatchResult {
      ensure!(
        !(instance.actor_class.aaa_type() == AaaType::System
          && instance.mutability == Mutability::Immutable),
        Error::<T>::ImmutableAaa
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
        identity.actor_class.aaa_type() == AaaType::System,
        Error::<T>::NotGovernance
      );
      Ok(())
    }

    fn ensure_control_queue_mutation_allowed(
      instance: &AaaInstanceOf<T>,
      now: BlockNumberFor<T>,
    ) -> DispatchResult {
      ensure!(
        instance.last_control_queue_mutation_block != Some(now),
        Error::<T>::QueueMutationRateLimited
      );
      Ok(())
    }

    fn ensure_control_origin(origin: OriginFor<T>, instance: &AaaInstanceOf<T>) -> DispatchResult {
      if let Ok(who) = ensure_signed(origin.clone()) {
        ensure!(who == instance.owner, Error::<T>::NotOwner);
        return Ok(());
      }
      T::SystemOrigin::ensure_origin(origin)?;
      ensure!(
        instance.actor_class.aaa_type() == AaaType::System,
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
    pub(crate) fn close_actor(
      aaa_id: AaaId,
      instance: &AaaInstanceOf<T>,
      reason: CloseReason,
    ) -> DispatchResult {
      ensure!(
        Self::active_actor_snapshot(aaa_id).as_ref() == Some(instance),
        Error::<T>::AaaNotFound
      );
      ensure!(
        ActorFunding::<T>::contains_key(aaa_id),
        Error::<T>::AaaNotFound
      );
      ensure!(
        ActiveAaaCount::<T>::get() > 0,
        Error::<T>::ActiveAaaCountInvariant
      );
      ensure!(
        ActorIdentityCount::<T>::get() > 0,
        Error::<T>::ActorIdentityCountInvariant
      );
      ensure!(
        SovereignIndex::<T>::get(&instance.sovereign_account) == Some(aaa_id),
        Error::<T>::AaaNotFound
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
          SystemSovereigns::<T>::get(sovereign_id) == Some(SystemSovereignState::Occupied(aaa_id)),
          Error::<T>::SystemSovereignInvariant
        );
      }

      Self::preflight_remove_observation_subscriptions(aaa_id)?;

      polkadot_sdk::frame_support::storage::with_transaction(|| {
        let result = (|| -> DispatchResult {
          Self::cancel_continuation_internal(aaa_id, CancellationReason::Closing(reason), None)?;

          // Actor-local ticket/pointer ownership makes shared queue and wakeup entries stale as
          // soon as hot state disappears. Terminal cleanup performs no shared-container scan.
          Self::remove_active_actor(aaa_id)?;
          ActorIdentities::<T>::remove(aaa_id);
          ActorFunding::<T>::remove(aaa_id);
          ActiveAaaCount::<T>::mutate(|count| *count -= 1);
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
          Self::deposit_event(Event::AaaClosed { aaa_id, reason });
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
      aaa_id: AaaId,
      identity: &ActorIdentityOf<T>,
      reason: CloseReason,
    ) -> DispatchResult {
      ensure!(
        ActorIdentities::<T>::get(aaa_id).as_ref() == Some(identity),
        Error::<T>::AaaNotFound
      );
      ensure!(
        ActorIdentityCount::<T>::get() > 0,
        Error::<T>::ActorIdentityCountInvariant
      );
      ensure!(
        SovereignIndex::<T>::get(&identity.sovereign_account) == Some(aaa_id),
        Error::<T>::AaaNotFound
      );
      if let ActorClass::User { owner_slot } = identity.actor_class {
        ensure!(
          Self::owner_slot_is_set(&OwnerSlotBitmaps::<T>::get(&identity.owner), owner_slot),
          Error::<T>::InvalidOwnerSlot
        );
      }

      ActorIdentities::<T>::remove(aaa_id);
      ActorIdentityCount::<T>::mutate(|count| *count -= 1);
      match identity.actor_class {
        ActorClass::User { owner_slot } => {
          Self::remove_owner_slot_binding(&identity.owner, owner_slot, &identity.sovereign_account)
        }
        ActorClass::System { sovereign_id } => {
          SovereignIndex::<T>::remove(&identity.sovereign_account);
          SystemSovereigns::<T>::insert(sovereign_id, SystemSovereignState::Vacant);
        }
      }
      Self::deposit_event(Event::AaaClosed { aaa_id, reason });
      Ok(())
    }

    pub(crate) fn update_idle_starvation_state(now: BlockNumberFor<T>, starved: bool) {
      let state = IdleStarvationState::<T>::get();
      if !starved {
        if let IdleStarvationPhase::Alerted { since } = state {
          Self::deposit_event(Event::IdleStarvationRecovered {
            consecutive_blocks: now.saturating_sub(since).saturated_into(),
          });
        }
        if !matches!(state, IdleStarvationPhase::Healthy) {
          IdleStarvationState::<T>::kill();
        }
        return;
      }
      match state {
        IdleStarvationPhase::Healthy => {
          if T::MaxIdleStarvationBlocks::get() <= 1 {
            IdleStarvationState::<T>::put(IdleStarvationPhase::Alerted { since: now });
            Self::deposit_event(Event::IdleStarvationDetected {
              consecutive_blocks: 1,
            });
          } else {
            IdleStarvationState::<T>::put(IdleStarvationPhase::Starving { since: now });
          }
        }
        IdleStarvationPhase::Starving { since } => {
          let duration = Self::starvation_duration(now, since);
          if duration >= T::MaxIdleStarvationBlocks::get() {
            IdleStarvationState::<T>::put(IdleStarvationPhase::Alerted { since });
            Self::deposit_event(Event::IdleStarvationDetected {
              consecutive_blocks: duration,
            });
          }
        }
        IdleStarvationPhase::Alerted { .. } => {}
      }
    }

    fn starvation_duration(now: BlockNumberFor<T>, since: BlockNumberFor<T>) -> u32 {
      now
        .saturating_sub(since)
        .saturating_add(One::one())
        .saturated_into()
    }

    // --- Active Actors Set Operations ---

    pub(crate) fn effective_active_actor_limit() -> u32 {
      ActiveActorLimit::<T>::get()
    }

    pub(crate) fn max_configurable_active_actor_limit() -> u32 {
      T::MaxActiveActors::get().min(T::MaxQueueLength::get())
    }

    pub(crate) fn active_instance_count() -> u32 {
      ActiveAaaCount::<T>::get()
    }

    pub(crate) fn remove_actor_from_queues(aaa_id: AaaId) {
      ActorHot::<T>::mutate(aaa_id, |maybe| {
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
          "ActiveAaaCount does not match ActorHot cardinality",
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
      for aaa_id in ActorHot::<T>::iter_keys() {
        if !ActorProgram::<T>::contains_key(aaa_id) {
          return Err(TryRuntimeError::Other(
            "ActorHot entry has no matching ActorProgram entry",
          ));
        }
      }
      for aaa_id in ActorProgram::<T>::iter_keys() {
        if !ActorHot::<T>::contains_key(aaa_id) {
          return Err(TryRuntimeError::Other(
            "ActorProgram entry has no matching ActorHot entry",
          ));
        }
      }
      let mut max_id: Option<AaaId> = None;
      let active_actors = ActorHot::<T>::iter(); // deos-bypass: bounded-iter — try-state-only invariant audit
      for (aaa_id, hot) in active_actors {
        let identity = ActorIdentities::<T>::get(aaa_id).ok_or(TryRuntimeError::Other(
          "ActorHot entry has no matching ActorIdentity entry",
        ))?;
        let program = ActorProgram::<T>::get(aaa_id).ok_or(TryRuntimeError::Other(
          "ActorHot entry has no matching ActorProgram entry",
        ))?;
        let has_continuation = ContinuationStateStore::<T>::contains_key(aaa_id);
        if (hot.run_state == RunState::Suspended) != has_continuation {
          return Err(TryRuntimeError::Other(
            "ActorHot run_state disagrees with ContinuationState",
          ));
        }
        let instance = Self::compose_active_actor(identity, hot, program);
        max_id = Some(max_id.map_or(aaa_id, |prev| prev.max(aaa_id)));
        let Some(funding) = ActorFunding::<T>::get(aaa_id) else {
          return Err(TryRuntimeError::Other(
            "ActorHot entry has no matching ActorFunding entry",
          ));
        };
        if instance.funding_tracked_count != funding.funding_tracked_assets.len() as u32 {
          return Err(TryRuntimeError::Other(
            "ActorHot funding indications disagree with ActorFunding",
          ));
        }
        if funding
          .funding_accumulated
          .iter() // deos-bypass: bounded-iter — try-state-only MaxFundingTrackedAssets audit
          .any(|(asset, amount)| {
            !funding.funding_tracked_assets.contains(asset) || amount.is_zero()
          })
        {
          return Err(TryRuntimeError::Other(
            "ActorFunding accumulator contains an untracked asset or zero amount",
          ));
        }
        match SovereignIndex::<T>::get(&instance.sovereign_account) {
          Some(mapped_id) if mapped_id == aaa_id => {}
          _ => {
            return Err(TryRuntimeError::Other(
              "SovereignIndex does not map sovereign_account back to aaa_id",
            ));
          }
        }
        if let ActorClass::User { owner_slot } = instance.actor_class {
          if owner_slot >= T::MaxOwnerSlots::get() {
            return Err(TryRuntimeError::Other(
              "User AAA owner_slot exceeds MaxOwnerSlots",
            ));
          }
          let bitmap = OwnerSlotBitmaps::<T>::get(&instance.owner);
          if !Self::owner_slot_bitmap_is_valid(&bitmap)
            || !Self::owner_slot_is_set(&bitmap, owner_slot)
          {
            return Err(TryRuntimeError::Other(
              "User AAA owner_slot is missing from OwnerSlotBitmaps",
            ));
          }
        }
      }
      for aaa_id in ActorFunding::<T>::iter_keys() {
        if !Self::active_actor_exists(aaa_id) {
          return Err(TryRuntimeError::Other(
            "ActorFunding entry has no matching split active actor",
          ));
        }
      }
      let continuations = ContinuationStateStore::<T>::iter(); // deos-bypass: bounded-iter — try-state-only active-actor invariant audit
      for (aaa_id, continuation) in continuations {
        let hot = ActorHot::<T>::get(aaa_id).ok_or(TryRuntimeError::Other(
          "ContinuationState entry has no matching ActorHot entry",
        ))?;
        let identity = ActorIdentities::<T>::get(aaa_id).ok_or(TryRuntimeError::Other(
          "ContinuationState entry has no matching ActorIdentity entry",
        ))?;
        let program = ActorProgram::<T>::get(aaa_id).ok_or(TryRuntimeError::Other(
          "ContinuationState entry has no matching ActorProgram entry",
        ))?;
        if hot.run_state != RunState::Suspended
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
          Self::trigger_surfaces(&program.execution_plan, continuation.cursor as usize);
        if expected_surfaces.len() != continuation.trigger_snapshot.len()
          || expected_surfaces
            .iter() // deos-bypass: bounded-iter — suffix surfaces are bounded by MaxContinuationSnapshotEntries
            .any(|surface| !continuation.trigger_snapshot.contains_key(surface))
        {
          return Err(TryRuntimeError::Other(
            "ContinuationState trigger snapshot disagrees with unresolved suffix",
          ));
        }
      }
      let identities = ActorIdentities::<T>::iter(); // deos-bypass: bounded-iter — try-state-only MaxActorIdentities audit
      for (aaa_id, identity) in identities {
        max_id = Some(max_id.map_or(aaa_id, |prev| prev.max(aaa_id)));
        if Self::active_actor_exists(aaa_id) {
          continue;
        }
        if ActorFunding::<T>::contains_key(aaa_id)
          || ContinuationStateStore::<T>::contains_key(aaa_id)
        {
          return Err(TryRuntimeError::Other(
            "Dormant identity owns active scheduler or readiness state",
          ));
        }
        match SovereignIndex::<T>::get(&identity.sovereign_account) {
          Some(mapped_id) if mapped_id == aaa_id => {}
          _ => {
            return Err(TryRuntimeError::Other(
              "Dormant SovereignIndex does not map sovereign_account back to aaa_id",
            ));
          }
        }
        match identity.actor_class {
          ActorClass::User { owner_slot } => {
            if owner_slot >= T::MaxOwnerSlots::get() {
              return Err(TryRuntimeError::Other(
                "Dormant User AAA owner_slot exceeds MaxOwnerSlots",
              ));
            }
            let bitmap = OwnerSlotBitmaps::<T>::get(&identity.owner);
            if !Self::owner_slot_bitmap_is_valid(&bitmap)
              || !Self::owner_slot_is_set(&bitmap, owner_slot)
            {
              return Err(TryRuntimeError::Other(
                "Dormant User AAA owner_slot is missing from OwnerSlotBitmaps",
              ));
            }
          }
          ActorClass::System { .. } if identity.mutability != Mutability::Mutable => {
            return Err(TryRuntimeError::Other("Dormant System AAA must be Mutable"));
          }
          ActorClass::System { .. } => {}
        }
      }
      let owner_slot_bitmaps = OwnerSlotBitmaps::<T>::iter(); // deos-bypass: bounded-iter — try-state-only owner map audit bounded by MaxActorIdentities.
      for (owner, bitmap) in owner_slot_bitmaps {
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
          let Some(aaa_id) = SovereignIndex::<T>::get(&sovereign) else {
            return Err(TryRuntimeError::Other(
              "OwnerSlotBitmaps bit has no SovereignIndex owner",
            ));
          };
          let Some(identity) = ActorIdentities::<T>::get(aaa_id) else {
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
      let pages = QueuePages::<T>::iter(); // deos-bypass: bounded-iter — try-state-only canonical queue audit
      for (page_id, page) in pages {
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
            .insert(entry.ticket, entry.aaa_id)
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
      let hot_states = ActorHot::<T>::iter(); // deos-bypass: bounded-iter — try-state-only live-ticket invariant audit
      for (aaa_id, hot) in hot_states {
        let Some(ticket) = hot.queue_ticket else {
          continue;
        };
        if ticket >= next_ticket || !live_queue_tickets.insert(ticket) {
          return Err(TryRuntimeError::Other(
            "ActorHot owns an invalid or duplicate global queue ticket",
          ));
        }
        if !ActorIdentities::<T>::contains_key(aaa_id) {
          return Err(TryRuntimeError::Other(
            "ActorHot live ticket has no ActorIdentity",
          ));
        }
        if physical_tickets.get(&ticket) != Some(&aaa_id) {
          return Err(TryRuntimeError::Other(
            "ActorHot live ticket does not resolve to its canonical queue entry",
          ));
        }
      }
      if T::WakeupPageSize::get() == 0 {
        return Err(TryRuntimeError::Other("WakeupPageSize must be non-zero"));
      }
      let mut wakeup_live_by_block = alloc::collections::BTreeMap::new();
      let mut wakeup_page_count = 0u32;
      let wakeup_pages = WakeupPages::<T>::iter(); // deos-bypass: bounded-iter — try-state-only paged-wakeup invariant audit
      for ((block, page_id), page) in wakeup_pages {
        wakeup_page_count = wakeup_page_count.saturating_add(1);
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
        let live_entries = page
          .entries
          .iter() // deos-bypass: bounded-iter — try-state-only WakeupPageSize slot audit
          .filter(|entry| entry.is_some())
          .count() as u32;
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
        for (slot, entry) in page
          .entries
          .iter() // deos-bypass: bounded-iter — try-state-only WakeupPageSize pointer audit
          .enumerate()
        {
          let Some(entry) = entry else {
            continue;
          };
          let expected = WakeupPointer {
            block,
            page_id,
            slot: slot as WakeupSlot,
          };
          if ActorHot::<T>::get(entry.aaa_id).and_then(|hot| hot.wakeup_pointer) != Some(expected) {
            return Err(TryRuntimeError::Other(
              "WakeupPage live slot has no matching ActorHot pointer",
            ));
          }
        }
      }
      if wakeup_page_count > active_count {
        return Err(TryRuntimeError::Other(
          "WakeupPages count exceeds active actor count",
        ));
      }
      let cursor_len = WakeupCursorLen::<T>::get();
      let wakeup_buckets = WakeupBuckets::<T>::iter(); // deos-bypass: bounded-iter — try-state-only active-actor-bounded bucket audit
      for (block, bucket) in wakeup_buckets {
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
      if cursor_len > T::MaxActiveActors::get() || cursor_len > active_count {
        return Err(TryRuntimeError::Other(
          "WakeupCursorLen exceeds active actor capacity",
        ));
      }
      let cursor_page_size = T::WakeupPageSize::get();
      let expected_cursor_pages = cursor_len.div_ceil(cursor_page_size);
      let actual_cursor_pages = WakeupCursorPages::<T>::iter().count() as u32; // deos-bypass: bounded-iter — try-state-only MaxActiveActors cursor-page audit
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
      let hot_wakeup_states = ActorHot::<T>::iter(); // deos-bypass: bounded-iter — try-state-only MaxActiveActors pointer audit
      for (aaa_id, hot) in hot_wakeup_states {
        let Some(pointer) = hot.wakeup_pointer else {
          continue;
        };
        if !live_wakeup_pointers.insert((pointer.block, pointer.page_id, pointer.slot)) {
          return Err(TryRuntimeError::Other(
            "multiple actors own the same wakeup pointer",
          ));
        }
        if !Self::wakeup_page_entry_matches(pointer, aaa_id) {
          return Err(TryRuntimeError::Other(
            "ActorHot wakeup pointer does not resolve to its actor",
          ));
        }
      }
      let next_id = NextAaaId::<T>::get();
      if let Some(max_aaa_id) = max_id {
        if next_id <= max_aaa_id {
          return Err(TryRuntimeError::Other(
            "NextAaaId is not greater than the largest active aaa_id",
          ));
        }
      }
      let system_sovereigns = SystemSovereigns::<T>::iter(); // deos-bypass: bounded-iter — try-state-only MaxSystemSovereigns audit
      let mut system_sovereign_count = 0u32;
      for (sovereign_id, state) in system_sovereigns {
        system_sovereign_count = system_sovereign_count
          .checked_add(1)
          .ok_or(TryRuntimeError::Other("System sovereign count overflow"))?;
        if let SystemSovereignState::Occupied(aaa_id) = state {
          let class = Self::active_actor_snapshot(aaa_id)
            .map(|actor| actor.actor_class)
            .or_else(|| ActorIdentities::<T>::get(aaa_id).map(|identity| identity.actor_class));
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
