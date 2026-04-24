#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

pub mod types;

mod execution;
mod scheduler;

pub mod adapters;
pub use adapters::{AssetOps, DexOps, LiquidityDonationOps, StakingOps};
pub use types::{SYSTEM_OWNER_SLOT_SENTINEL, Task};

pub mod weights;
pub use weights::{TaskWeightInfo, WeightInfo};

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AccountId, AssetId, Balance> {
  fn setup_remove_liquidity_max_k(
    owner: &AccountId,
    max_scan: u32,
  ) -> Result<(AssetId, Balance), polkadot_sdk::sp_runtime::DispatchError>;
}

pub trait EntropyProvider<Hash> {
  fn entropy(subject: &[u8]) -> Option<Hash>;
  fn secure_entropy_for_financial_probability(subject: &[u8]) -> Option<Hash> {
    Self::entropy(subject)
  }
  fn is_secure_for_financial_probability() -> bool {
    false
  }
}

pub trait AtomicityHook {
  fn on_create_checkpoint(_aaa_id: u64) -> polkadot_sdk::frame_support::dispatch::DispatchResult {
    Ok(())
  }
  fn on_close_checkpoint(_aaa_id: u64) -> polkadot_sdk::frame_support::dispatch::DispatchResult {
    Ok(())
  }
}

impl AtomicityHook for () {}

pub trait AddressEventIngressHook<BlockNumber> {
  fn ingest(_now: BlockNumber) -> polkadot_sdk::frame_support::weights::Weight;
}

impl<BlockNumber> AddressEventIngressHook<BlockNumber> for () {
  fn ingest(_now: BlockNumber) -> polkadot_sdk::frame_support::weights::Weight {
    polkadot_sdk::frame_support::weights::Weight::zero()
  }
}

pub struct NoEntropyProvider;

impl<Hash> EntropyProvider<Hash> for NoEntropyProvider {
  fn entropy(_subject: &[u8]) -> Option<Hash> {
    None
  }
}

#[frame::pallet]
pub mod pallet {
  use super::{
    AddressEventIngressHook, AssetOps, AtomicityHook, DexOps, EntropyProvider,
    LiquidityDonationOps, TaskWeightInfo, WeightInfo,
  };
  use frame::prelude::*;
  use polkadot_sdk::{
    frame_support::{PalletId, traits::EnsureOrigin},
    sp_runtime::traits::Zero,
    sp_weights::WeightToFee as _,
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
    type DexOps: DexOps<Self::AccountId, Self::AssetId, Self::Balance>;
    type StakingOps: crate::adapters::StakingOps<Self::AccountId, Self::AssetId, Self::Balance>;
    type LiquidityDonationOps: LiquidityDonationOps<Self::AccountId, Self::AssetId, Self::Balance>;

    #[pallet::constant]
    type MinWindowLength: Get<BlockNumberFor<Self>>;
    #[pallet::constant]
    type PalletId: Get<PalletId>;

    type SystemOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    type GlobalBreakerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

    #[pallet::constant]
    type MaxExecutionPlanSteps: Get<u32>;
    #[pallet::constant]
    type MaxUserExecutionPlanSteps: Get<u32>;
    #[pallet::constant]
    type MaxSystemExecutionPlanSteps: Get<u32>;
    #[pallet::constant]
    type MaxFundingTrackedAssets: Get<u32>;
    #[pallet::constant]
    type MaxIngressOverflowQueue: Get<u32>;
    #[pallet::constant]
    type MaxConditionsPerStep: Get<u32>;
    #[pallet::constant]
    type MaxOwnerSlots: Get<u8>;
    #[pallet::constant]
    type MaxExecutionsPerBlock: Get<u32>;
    #[pallet::constant]
    type MaxQueueLength: Get<u32>;
    #[pallet::constant]
    type MaxWakeupBucketSize: Get<u32>;
    #[pallet::constant]
    type MaxWakeupsPerBlock: Get<u32>;
    #[pallet::constant]
    type MaxQueueInsertionsPerBlock: Get<u32>;
    #[pallet::constant]
    type FairnessWeightSystem: Get<u32>;
    #[pallet::constant]
    type FairnessWeightUser: Get<u32>;
    #[pallet::constant]
    type MaxSweepPerBlock: Get<u32>;
    #[pallet::constant]
    type MaxWhitelistSize: Get<u32>;
    #[pallet::constant]
    type MaxSplitTransferLegs: Get<u32>;
    #[pallet::constant]
    type MaxAdapterScan: Get<u32>;
    #[pallet::constant]
    type MaxExecutionDelayBlocks: Get<BlockNumberFor<Self>>;
    #[pallet::constant]
    type MaxTimerJitterBlocks: Get<u32>;
    #[pallet::constant]
    type MaxIdleStarvationBlocks: Get<u32>;
    #[pallet::constant]
    type MaxAutoCloseNonceHorizon: Get<u64>;
    /// Maximum number of active AAA instances. Bounds the BTreeSet storage.
    /// Set to 10,000 for production use cases.
    #[pallet::constant]
    type MaxActiveActors: Get<u32>;

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
    /// If true, probabilistic timer schedules with economically sensitive tasks
    /// require a secure external entropy provider
    #[pallet::constant]
    type RequireSecureEntropyForProbabilisticTasks: Get<bool>;
    /// Optional external entropy hook for timer probability sampling
    type EntropyProvider: EntropyProvider<Self::Hash>;
    /// Testable atomicity checkpoints for create/close lifecycle paths
    type AtomicityHook: AtomicityHook;

    /// Runtime ingress hook for address-event notifications
    type AddressEventIngressHook: AddressEventIngressHook<BlockNumberFor<Self>>;

    type FeeSink: Get<Self::AccountId>;

    #[pallet::constant]
    type MaxConsecutiveFailures: Get<u32>;
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
    type BenchmarkHelper: crate::BenchmarkHelper<Self::AccountId, Self::AssetId, Self::Balance>;
  }

  pub type BalanceOf<T> = <T as Config>::Balance;
  pub type AssetIdOf<T> = <T as Config>::AssetId;

  pub type SourceFilterOf<T> =
    SourceFilter<<T as frame_system::Config>::AccountId, <T as Config>::MaxWhitelistSize>;

  pub type AssetFilterOf<T> = AssetFilter<<T as Config>::AssetId, <T as Config>::MaxWhitelistSize>;

  pub type TriggerOf<T> = Trigger<
    <T as frame_system::Config>::AccountId,
    <T as Config>::AssetId,
    <T as Config>::MaxWhitelistSize,
  >;

  pub type ScheduleOf<T> = Schedule<
    <T as frame_system::Config>::AccountId,
    <T as Config>::AssetId,
    <T as Config>::MaxWhitelistSize,
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
  >;

  pub type ExecutionPlanOf<T> = BoundedVec<StepOf<T>, <T as Config>::MaxExecutionPlanSteps>;

  pub type FundingSnapshotsOf<T> = BoundedBTreeMap<
    <T as Config>::AssetId,
    FundingSnapshot<<T as Config>::Balance, BlockNumberFor<T>>,
    <T as Config>::MaxFundingTrackedAssets,
  >;

  pub type FundingTrackedAssetsOf<T> =
    BoundedBTreeSet<<T as Config>::AssetId, <T as Config>::MaxFundingTrackedAssets>;

  pub type IngressOverflowEventOf<T> = IngressOverflowEvent<
    AaaId,
    <T as Config>::AssetId,
    <T as Config>::Balance,
    <T as frame_system::Config>::AccountId,
  >;

  pub type AaaInstanceOf<T> = AaaInstance<
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T>,
    ScheduleOf<T>,
    ExecutionPlanOf<T>,
    FundingSnapshotsOf<T>,
    FundingTrackedAssetsOf<T>,
    BalanceOf<T>,
  >;

  pub type AaaReadinessStateOf<T> = AaaReadinessState<BlockNumberFor<T>>;

  #[pallet::pallet]
  #[pallet::storage_version(STORAGE_VERSION)]
  pub struct Pallet<T>(_);

  const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

  #[pallet::storage]
  #[pallet::getter(fn next_aaa_id)]
  pub type NextAaaId<T> = StorageValue<_, AaaId, ValueQuery>;

  #[pallet::storage]
  pub type SweepCursor<T: Config> = StorageValue<_, AaaId, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn aaa_instances)]
  pub type AaaInstances<T: Config> =
    StorageMap<_, Blake2_128Concat, AaaId, AaaInstanceOf<T>, OptionQuery>;

  #[pallet::storage]
  pub type ClosedSystemAaaIds<T: Config> = StorageMap<_, Blake2_128Concat, AaaId, (), OptionQuery>;

  #[pallet::storage]
  pub type CurrentQueue<T: Config> =
    StorageValue<_, BoundedVec<AaaId, T::MaxQueueLength>, ValueQuery>;

  #[pallet::storage]
  pub type NextQueue<T: Config> = StorageValue<_, BoundedVec<AaaId, T::MaxQueueLength>, ValueQuery>;

  #[pallet::storage]
  pub type WakeupIndex<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    BlockNumberFor<T>,
    BoundedVec<AaaId, T::MaxWakeupBucketSize>,
    ValueQuery,
  >;

  #[pallet::storage]
  pub type MinWakeupBlock<T: Config> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;

  #[pallet::storage]
  pub type ScheduledWakeupBlock<T: Config> =
    StorageMap<_, Blake2_128Concat, AaaId, BlockNumberFor<T>, OptionQuery>;

  #[pallet::storage]
  pub type WakeupScheduleDrops<T: Config> = StorageValue<_, u64, ValueQuery>;

  #[pallet::storage]
  pub type QueueEpoch<T: Config> = StorageValue<_, u64, ValueQuery>;

  #[pallet::storage]
  pub type ActorQueueEpoch<T: Config> = StorageMap<_, Blake2_128Concat, AaaId, u64, ValueQuery>;

  #[pallet::storage]
  pub type AaaReadiness<T: Config> =
    StorageMap<_, Blake2_128Concat, AaaId, AaaReadinessStateOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn owner_slot_mask)]
  pub type OwnerSlotMask<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u8, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn sovereign_index)]
  pub type SovereignIndex<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AccountId, AaaId, OptionQuery>;

  /// Governance-configurable active actor limit.
  /// `0` means fallback to `min(T::MaxActiveActors, T::MaxQueueLength)` for backward compatibility.
  #[pallet::storage]
  #[pallet::getter(fn configured_active_actor_limit)]
  pub type ActiveActorLimit<T: Config> = StorageValue<_, u32, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn global_circuit_breaker)]
  pub type GlobalCircuitBreaker<T> = StorageValue<_, bool, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn address_event_inbox)]
  pub type AddressEventInbox<T: Config> =
    StorageMap<_, Blake2_128Concat, AaaId, InboxState<BlockNumberFor<T>>, OptionQuery>;

  #[pallet::storage]
  pub type IngressOverflowSlots<T: Config> =
    StorageMap<_, Blake2_128Concat, u32, IngressOverflowEventOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn ingress_overflow_head)]
  pub type IngressOverflowHead<T: Config> = StorageValue<_, u32, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn ingress_overflow_len)]
  pub type IngressOverflowLen<T: Config> = StorageValue<_, u32, ValueQuery>;

  #[pallet::storage]
  pub type IngressSeenBlock<T: Config> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;

  #[pallet::storage]
  pub type IngressSeenSet<T: Config> =
    StorageValue<_, BoundedBTreeSet<T::Hash, T::MaxIngressOverflowQueue>, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn idle_starvation_blocks)]
  pub type IdleStarvationBlocks<T: Config> = StorageValue<_, u32, ValueQuery>;

  #[pallet::storage]
  pub type LastIngressIngestBlock<T: Config> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;

  /// Provides runtime-specific System AAA instances to initialize at genesis.
  ///
  /// Implement this on the runtime to return System AAA specs with explicit `aaa_id` values.
  /// IDs may be sparse to reserve stable addresses for non-actor accounts.
  pub trait GenesisSystemAaas<AccountId, Schedule, ScheduleWindow, ExecutionPlan> {
    fn system_aaas() -> alloc::vec::Vec<(
      AaaId,
      AccountId,
      Schedule,
      Option<ScheduleWindow>,
      ExecutionPlan,
    )>;
  }

  /// Default no-op implementation: no System AAA created at genesis.
  impl<AccountId, Schedule, ScheduleWindowT, ExecutionPlan>
    GenesisSystemAaas<AccountId, Schedule, ScheduleWindowT, ExecutionPlan> for ()
  {
    fn system_aaas() -> alloc::vec::Vec<(
      AaaId,
      AccountId,
      Schedule,
      Option<ScheduleWindowT>,
      ExecutionPlan,
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
      if ActiveActorLimit::<T>::get() == 0 {
        ActiveActorLimit::<T>::put(Pallet::<T>::max_configurable_active_actor_limit());
      }
      for (aaa_id, owner, schedule, schedule_window, execution_plan) in
        T::GenesisSystemAaas::system_aaas()
      {
        assert!(
          !AaaInstances::<T>::contains_key(aaa_id),
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
        Pallet::<T>::validate_probability_entropy_policy(&schedule, &execution_plan)
          .expect("genesis probabilistic financial actor requires secure entropy provider");
        let on_close_execution_plan = Pallet::<T>::default_on_close_execution_plan();
        let funding_tracked_assets = Pallet::<T>::derive_combined_funding_tracked_assets(
          &execution_plan,
          &on_close_execution_plan,
        )
        .expect("genesis execution_plan must have valid funding-tracked assets");
        let (cycle_weight_upper, cycle_fee_upper) =
          Pallet::<T>::compute_cycle_bounds(AaaType::System, &execution_plan);
        let instance = AaaInstance {
          aaa_id,
          sovereign_account: sovereign_account.clone(),
          owner: owner.clone(),
          owner_slot: SYSTEM_OWNER_SLOT_SENTINEL,
          aaa_type: AaaType::System,
          mutability: Mutability::Mutable,
          is_paused: false,
          pause_reason: None,
          schedule,
          schedule_window,
          execution_plan,
          on_close_execution_plan,
          cycle_nonce: 0,
          consecutive_failures: 0,
          manual_trigger_pending: false,
          funding_snapshots: Default::default(),
          funding_tracked_assets,
          cycle_weight_upper,
          cycle_fee_upper,
          auto_close_at_cycle_nonce: None,
          created_at: Zero::zero(),
          updated_at: Zero::zero(),
          last_cycle_block: Zero::zero(),
        };
        let active_count = Pallet::<T>::active_instance_count();
        assert!(
          active_count < T::MaxActiveActors::get(),
          "genesis active actor capacity exceeded at aaa_id={aaa_id}"
        );
        let readiness = Pallet::<T>::readiness_state_from_instance(&instance);
        SovereignIndex::<T>::insert(&sovereign_account, aaa_id);
        frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        AaaInstances::<T>::insert(aaa_id, instance);
        AaaReadiness::<T>::insert(aaa_id, readiness);
        Pallet::<T>::prime_actor_schedule(aaa_id);
      }
    }
  }

  #[pallet::hooks]
  impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    #[cfg(feature = "try-runtime")]
    fn try_state(_n: BlockNumberFor<T>) -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      Self::do_try_state()
    }

    fn on_initialize(_now: BlockNumberFor<T>) -> Weight {
      let _ = GlobalCircuitBreaker::<T>::get();
      T::DbWeight::get().reads(1)
    }

    fn on_idle(now: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
      let ingress_weight = if LastIngressIngestBlock::<T>::get() == Some(now) {
        Weight::zero()
      } else {
        let weight = T::AddressEventIngressHook::ingest(now);
        LastIngressIngestBlock::<T>::put(now);
        weight
      };
      let remaining_after_ingress = remaining_weight.saturating_sub(ingress_weight);
      let sweep_weight = Self::execute_zombie_sweep(remaining_after_ingress);
      let remaining_after_housekeeping = remaining_after_ingress.saturating_sub(sweep_weight);
      let breaker_active = GlobalCircuitBreaker::<T>::get();
      Self::update_idle_starvation_state(breaker_active, remaining_after_housekeeping);
      if breaker_active {
        return ingress_weight.saturating_add(sweep_weight);
      }
      let cycle_weight = Self::execute_cycle(remaining_after_housekeeping);
      ingress_weight
        .saturating_add(sweep_weight)
        .saturating_add(cycle_weight)
    }
  }

  #[pallet::event]
  #[pallet::generate_deposit(pub(super) fn deposit_event)]
  pub enum Event<T: Config> {
    AaaCreated {
      aaa_id: AaaId,
      owner: T::AccountId,
      owner_slot: u8,
      aaa_type: AaaType,
      mutability: Mutability,
      sovereign_account: T::AccountId,
    },
    AaaFunded {
      aaa_id: AaaId,
      asset: T::AssetId,
      amount: BalanceOf<T>,
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
      reason: DeferReason,
    },
    WakeupRescheduled {
      aaa_id: AaaId,
      requested_block: BlockNumberFor<T>,
      scheduled_block: BlockNumberFor<T>,
    },
    WakeupScheduleDropped {
      aaa_id: AaaId,
      requested_block: BlockNumberFor<T>,
    },
    CycleStarted {
      aaa_id: AaaId,
      cycle_nonce: u64,
    },
    CycleSummary {
      aaa_id: AaaId,
      cycle_nonce: u64,
      executed_steps: u32,
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
      error: DispatchError,
    },
    TransferExecuted {
      aaa_id: AaaId,
      asset: T::AssetId,
      amount: T::Balance,
      to: T::AccountId,
    },
    SplitTransferExecuted {
      aaa_id: AaaId,
      asset: T::AssetId,
      total: T::Balance,
      distributed: T::Balance,
      retained: T::Balance,
      legs: u32,
      effective_legs: u32,
    },
    SwapExecuted {
      aaa_id: AaaId,
      asset_in: T::AssetId,
      asset_out: T::AssetId,
      amount_in: T::Balance,
      amount_out: T::Balance,
    },
    BurnExecuted {
      aaa_id: AaaId,
      asset: T::AssetId,
      amount: T::Balance,
    },
    MintExecuted {
      aaa_id: AaaId,
      asset: T::AssetId,
      amount: T::Balance,
    },
    StakeExecuted {
      aaa_id: AaaId,
      asset: T::AssetId,
      amount: T::Balance,
    },
    UnstakeExecuted {
      aaa_id: AaaId,
      asset: T::AssetId,
      shares: T::Balance,
    },
    LiquidityDonated {
      aaa_id: AaaId,
      asset_a: T::AssetId,
      asset_b: T::AssetId,
      amount: T::Balance,
      amount_a: T::Balance,
      amount_b: T::Balance,
    },
    LiquidityAdded {
      aaa_id: AaaId,
      asset_a: T::AssetId,
      asset_b: T::AssetId,
      lp_minted: T::Balance,
    },
    LiquidityRemoved {
      aaa_id: AaaId,
      lp_asset: T::AssetId,
      amount_a: T::Balance,
      amount_b: T::Balance,
    },
    ScheduleUpdated {
      aaa_id: AaaId,
    },
    ExecutionPlanUpdated {
      aaa_id: AaaId,
    },
    OnCloseExecutionPlanUpdated {
      aaa_id: AaaId,
    },
    OnCloseStepFailed {
      aaa_id: AaaId,
      step_index: u32,
      error: DispatchError,
    },
    OnCloseExecutionPlanSummary {
      aaa_id: AaaId,
      executed_steps: u32,
      skipped_steps: u32,
      failed_steps: u32,
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
  }

  #[pallet::error]
  pub enum Error<T> {
    AaaIdOverflow,
    AaaNotFound,
    ActiveAaaCapacityExceeded,
    ActiveAaaLimitExceedsQueueCapacity,
    ActiveAaaLimitTooHigh,
    ActiveAaaLimitTooLow,
    AaaPaused,
    EmptyExecutionPlan,
    ExecutionDelayTooLong,
    GlobalCircuitBreakerActive,
    ImmutableAaa,
    InsecureEntropyProvider,
    InsufficientBalance,
    InsufficientFee,
    InvalidAmountResolution,
    InvalidAutoCloseNonce,
    InvalidScheduleWindow,
    InvalidSplitTransfer,
    InvalidTriggerConfiguration,
    MintNotAllowedForUserAaa,
    NotGovernance,
    NotOwner,
    NotPaused,
    OwnerSlotCapacityExceeded,
    OwnerSlotOccupied,
    InvalidOwnerSlot,
    AaaIdOccupied,
    SystemAaaNotClosed,
    ExecutionPlanTooLong,
    SnapshotUnavailable,
    SovereignAccountCollision,
    AutoCloseNonceHorizonExceeded,
    AutoCloseNonceOverflow,
    AutoCloseNonceIncrementZero,
  }

  #[pallet::call]
  impl<T: Config> Pallet<T> {
    #[pallet::call_index(0)]
    #[pallet::weight(T::WeightInfo::create_user_aaa())]
    pub fn create_user_aaa(
      origin: OriginFor<T>,
      mutability: Mutability,
      schedule: ScheduleOf<T>,
      schedule_window: Option<ScheduleWindow<BlockNumberFor<T>>>,
      execution_plan: ExecutionPlanOf<T>,
    ) -> DispatchResult {
      let owner = ensure_signed(origin)?;
      Self::do_create_user_aaa(
        owner,
        mutability,
        None,
        schedule,
        schedule_window,
        execution_plan,
      )
    }

    #[pallet::call_index(16)]
    #[pallet::weight(T::WeightInfo::create_user_aaa_at_slot())]
    pub fn create_user_aaa_at_slot(
      origin: OriginFor<T>,
      owner_slot: u8,
      mutability: Mutability,
      schedule: ScheduleOf<T>,
      schedule_window: Option<ScheduleWindow<BlockNumberFor<T>>>,
      execution_plan: ExecutionPlanOf<T>,
    ) -> DispatchResult {
      let owner = ensure_signed(origin)?;
      Self::do_create_user_aaa(
        owner,
        mutability,
        Some(owner_slot),
        schedule,
        schedule_window,
        execution_plan,
      )
    }

    #[pallet::call_index(1)]
    #[pallet::weight(T::WeightInfo::create_system_aaa())]
    pub fn create_system_aaa(
      origin: OriginFor<T>,
      owner: T::AccountId,
      schedule: ScheduleOf<T>,
      schedule_window: Option<ScheduleWindow<BlockNumberFor<T>>>,
      execution_plan: ExecutionPlanOf<T>,
    ) -> DispatchResult {
      T::SystemOrigin::ensure_origin(origin)?;
      Self::do_create_system_aaa(owner, schedule, schedule_window, execution_plan, None)
    }

    #[pallet::call_index(17)]
    #[pallet::weight(T::WeightInfo::reopen_system_aaa())]
    pub fn reopen_system_aaa(
      origin: OriginFor<T>,
      aaa_id: AaaId,
      owner: T::AccountId,
      schedule: ScheduleOf<T>,
      schedule_window: Option<ScheduleWindow<BlockNumberFor<T>>>,
      execution_plan: ExecutionPlanOf<T>,
    ) -> DispatchResult {
      T::SystemOrigin::ensure_origin(origin)?;
      ensure!(
        !AaaInstances::<T>::contains_key(aaa_id),
        Error::<T>::AaaIdOccupied
      );
      ensure!(
        ClosedSystemAaaIds::<T>::contains_key(aaa_id),
        Error::<T>::SystemAaaNotClosed
      );
      Self::do_create_system_aaa(
        owner,
        schedule,
        schedule_window,
        execution_plan,
        Some(aaa_id),
      )
    }

    #[pallet::call_index(2)]
    #[pallet::weight(T::WeightInfo::pause_aaa().saturating_add(T::WeightInfo::close_aaa()))]
    pub fn pause_aaa(origin: OriginFor<T>, aaa_id: AaaId) -> DispatchResult {
      let snapshot = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      AaaInstances::<T>::try_mutate(aaa_id, |maybe| -> DispatchResult {
        let inst = maybe.as_mut().ok_or(Error::<T>::AaaNotFound)?;
        ensure!(
          inst.mutability == Mutability::Mutable,
          Error::<T>::ImmutableAaa
        );
        ensure!(!inst.is_paused, Error::<T>::AaaPaused);
        inst.is_paused = true;
        inst.pause_reason = Some(PauseReason::Manual);
        inst.updated_at = frame_system::Pallet::<T>::block_number();
        // Ringless: no need to remove from ring - scheduler checks is_paused flag
        Self::deposit_event(Event::AaaPaused {
          aaa_id,
          reason: PauseReason::Manual,
        });
        Ok(())
      })?;
      Self::sync_readiness_state(aaa_id);
      Ok(())
    }

    #[pallet::call_index(3)]
    #[pallet::weight(T::WeightInfo::resume_aaa().saturating_add(T::WeightInfo::close_aaa()))]
    pub fn resume_aaa(origin: OriginFor<T>, aaa_id: AaaId) -> DispatchResult {
      let snapshot = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      AaaInstances::<T>::try_mutate(aaa_id, |maybe| -> DispatchResult {
        let inst = maybe.as_mut().ok_or(Error::<T>::AaaNotFound)?;
        ensure!(
          inst.mutability == Mutability::Mutable,
          Error::<T>::ImmutableAaa
        );
        ensure!(inst.is_paused, Error::<T>::NotPaused);
        inst.is_paused = false;
        inst.pause_reason = None;
        inst.updated_at = frame_system::Pallet::<T>::block_number();
        // Ringless: no need to requeue - scheduler discovers actor on next block
        Self::deposit_event(Event::AaaResumed { aaa_id });
        Ok(())
      })?;
      Self::sync_readiness_state(aaa_id);
      Ok(())
    }

    #[pallet::call_index(4)]
    #[pallet::weight(T::WeightInfo::manual_trigger().saturating_add(T::WeightInfo::close_aaa()))]
    pub fn manual_trigger(origin: OriginFor<T>, aaa_id: AaaId) -> DispatchResult {
      let snapshot = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      AaaInstances::<T>::try_mutate(aaa_id, |maybe| -> DispatchResult {
        let inst = maybe.as_mut().ok_or(Error::<T>::AaaNotFound)?;
        ensure!(!inst.is_paused, Error::<T>::AaaPaused);
        inst.manual_trigger_pending = true;
        inst.updated_at = frame_system::Pallet::<T>::block_number();
        // Ringless: no need to requeue - scheduler checks manual_trigger_pending flag
        Self::deposit_event(Event::ManualTriggerSet { aaa_id });
        Ok(())
      })?;
      Self::sync_readiness_state(aaa_id);
      Self::enqueue(aaa_id);
      Ok(())
    }

    #[pallet::call_index(5)]
    #[pallet::weight(T::WeightInfo::fund_aaa().saturating_add(T::WeightInfo::close_aaa()))]
    pub fn fund_aaa(
      origin: OriginFor<T>,
      aaa_id: AaaId,
      asset: T::AssetId,
      amount: BalanceOf<T>,
    ) -> DispatchResult {
      let who = ensure_signed(origin)?;
      ensure!(!amount.is_zero(), Error::<T>::InvalidAmountResolution);
      let instance = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      if Self::is_window_expired(&instance) {
        let _ = Self::close_actor(aaa_id, &instance, CloseReason::WindowExpired);
        return Ok(());
      }
      ensure!(
        instance.funding_tracked_assets.contains(&asset),
        Error::<T>::SnapshotUnavailable
      );
      T::AssetOps::transfer(&who, &instance.sovereign_account, asset, amount)?;
      let now = frame_system::Pallet::<T>::block_number();
      AaaInstances::<T>::mutate(aaa_id, |maybe| {
        let Some(inst) = maybe.as_mut() else {
          return;
        };
        let _ = inst
          .funding_snapshots
          .try_insert(asset, FundingSnapshot { amount, block: now });
        inst.updated_at = now;
      });
      Self::sync_readiness_state(aaa_id);
      let _ = who;
      Self::deposit_event(Event::AaaFunded {
        aaa_id,
        asset,
        amount,
      });
      Ok(())
    }

    #[pallet::call_index(6)]
    #[pallet::weight(T::WeightInfo::close_aaa())]
    pub fn close_aaa(origin: OriginFor<T>, aaa_id: AaaId) -> DispatchResult {
      let instance = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin, &instance)?;
      Self::close_actor(aaa_id, &instance, CloseReason::OwnerInitiated)
    }

    #[pallet::call_index(7)]
    #[pallet::weight(T::WeightInfo::update_schedule().saturating_add(T::WeightInfo::close_aaa()))]
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
      let snapshot = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      Self::validate_probability_entropy_policy(&schedule, &snapshot.execution_plan)?;
      AaaInstances::<T>::try_mutate(aaa_id, |maybe| -> DispatchResult {
        let inst = maybe.as_mut().ok_or(Error::<T>::AaaNotFound)?;
        ensure!(
          inst.mutability == Mutability::Mutable,
          Error::<T>::ImmutableAaa
        );
        inst.schedule = schedule;
        inst.schedule_window = schedule_window;
        inst.updated_at = frame_system::Pallet::<T>::block_number();
        Self::deposit_event(Event::ScheduleUpdated { aaa_id });
        Ok(())
      })?;
      Self::sync_readiness_state(aaa_id);
      Self::prime_actor_schedule(aaa_id);
      Ok(())
    }

    #[pallet::call_index(8)]
    #[pallet::weight(T::WeightInfo::set_global_circuit_breaker())]
    pub fn set_global_circuit_breaker(origin: OriginFor<T>, paused: bool) -> DispatchResult {
      T::GlobalBreakerOrigin::ensure_origin(origin)?;
      GlobalCircuitBreaker::<T>::put(paused);
      Self::deposit_event(Event::GlobalCircuitBreakerSet { paused });
      Ok(())
    }

    /// Force lifecycle evaluation for a specific actor
    #[pallet::call_index(9)]
    #[pallet::weight(T::WeightInfo::permissionless_sweep().saturating_add(T::WeightInfo::close_aaa()))]
    pub fn permissionless_sweep(origin: OriginFor<T>, aaa_id: AaaId) -> DispatchResult {
      let _who = ensure_signed(origin)?;
      Self::evaluate_actor_liveness(aaa_id)
    }

    #[pallet::call_index(10)]
    #[pallet::weight(T::WeightInfo::update_execution_plan().saturating_add(T::WeightInfo::close_aaa()))]
    pub fn update_execution_plan(
      origin: OriginFor<T>,
      aaa_id: AaaId,
      execution_plan: ExecutionPlanOf<T>,
    ) -> DispatchResult {
      ensure!(!execution_plan.is_empty(), Error::<T>::EmptyExecutionPlan);
      Self::validate_execution_plan_shape(&execution_plan)?;
      let snapshot = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      Self::validate_probability_entropy_policy(&snapshot.schedule, &execution_plan)?;
      AaaInstances::<T>::try_mutate(aaa_id, |maybe| -> DispatchResult {
        let inst = maybe.as_mut().ok_or(Error::<T>::AaaNotFound)?;
        ensure!(
          inst.mutability == Mutability::Mutable,
          Error::<T>::ImmutableAaa
        );
        let max_steps = match inst.aaa_type {
          AaaType::User => T::MaxUserExecutionPlanSteps::get(),
          AaaType::System => T::MaxSystemExecutionPlanSteps::get(),
        };
        ensure!(
          (execution_plan.len() as u32) <= max_steps,
          Error::<T>::ExecutionPlanTooLong
        );
        if inst.aaa_type == AaaType::User {
          ensure!(
            !Self::execution_plan_contains_mint(&execution_plan),
            Error::<T>::MintNotAllowedForUserAaa
          );
        }
        let new_tracked = Self::derive_combined_funding_tracked_assets(
          &execution_plan,
          &inst.on_close_execution_plan,
        )?;
        inst.funding_tracked_assets = new_tracked.clone();
        let mut stale_keys = alloc::vec::Vec::new();
        for key in inst.funding_snapshots.keys() {
          if !new_tracked.contains(key) {
            stale_keys.push(*key);
          }
        }
        for key in stale_keys {
          let _ = inst.funding_snapshots.remove(&key);
        }
        let (cycle_weight_upper, cycle_fee_upper) =
          Self::compute_cycle_bounds(inst.aaa_type, &execution_plan);
        inst.execution_plan = execution_plan;
        inst.cycle_weight_upper = cycle_weight_upper;
        inst.cycle_fee_upper = cycle_fee_upper;
        inst.consecutive_failures = 0;
        inst.updated_at = frame_system::Pallet::<T>::block_number();
        Self::deposit_event(Event::ExecutionPlanUpdated { aaa_id });
        Ok(())
      })
    }

    #[pallet::call_index(11)]
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
      ensure!(new_limit >= active_count, Error::<T>::ActiveAaaLimitTooLow);
      let old_limit = Self::effective_active_actor_limit();
      ActiveActorLimit::<T>::put(new_limit);
      Self::deposit_event(Event::ActiveActorLimitSet {
        old_limit,
        new_limit,
      });
      Ok(())
    }

    #[pallet::call_index(12)]
    #[pallet::weight(
      T::WeightInfo::permissionless_sweep_many(aaa_ids.len() as u32)
        .saturating_add(T::WeightInfo::close_aaa().saturating_mul(aaa_ids.len() as u64))
    )]
    pub fn permissionless_sweep_many(
      origin: OriginFor<T>,
      aaa_ids: BoundedVec<AaaId, T::MaxSweepPerBlock>,
    ) -> DispatchResult {
      let _who = ensure_signed(origin)?;
      let mut closed = 0u32;
      let mut alive = 0u32;
      let mut missing = 0u32;
      for aaa_id in aaa_ids.iter().copied() {
        let Some(instance) = AaaInstances::<T>::get(aaa_id) else {
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

    #[pallet::call_index(13)]
    #[pallet::weight(T::WeightInfo::update_schedule().saturating_add(T::WeightInfo::close_aaa()))]
    pub fn set_auto_close_at_cycle_nonce(
      origin: OriginFor<T>,
      aaa_id: AaaId,
      target: Option<u64>,
    ) -> DispatchResult {
      let snapshot = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      AaaInstances::<T>::try_mutate(aaa_id, |maybe| -> DispatchResult {
        let inst = maybe.as_mut().ok_or(Error::<T>::AaaNotFound)?;
        ensure!(
          inst.mutability == Mutability::Mutable,
          Error::<T>::ImmutableAaa
        );
        if let Some(target_nonce) = target {
          Self::ensure_auto_close_target(inst.cycle_nonce, target_nonce)?;
        }
        inst.auto_close_at_cycle_nonce = target;
        inst.updated_at = frame_system::Pallet::<T>::block_number();
        Self::deposit_event(Event::AutoCloseNonceSet { aaa_id, target });
        Ok(())
      })?;
      Self::sync_readiness_state(aaa_id);
      Ok(())
    }

    #[pallet::call_index(14)]
    #[pallet::weight(T::WeightInfo::update_schedule().saturating_add(T::WeightInfo::close_aaa()))]
    pub fn increment_auto_close_nonce(
      origin: OriginFor<T>,
      aaa_id: AaaId,
      by: u64,
    ) -> DispatchResult {
      ensure!(by > 0, Error::<T>::AutoCloseNonceIncrementZero);
      let snapshot = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      AaaInstances::<T>::try_mutate(aaa_id, |maybe| -> DispatchResult {
        let inst = maybe.as_mut().ok_or(Error::<T>::AaaNotFound)?;
        ensure!(
          inst.mutability == Mutability::Mutable,
          Error::<T>::ImmutableAaa
        );
        let old_target = inst.auto_close_at_cycle_nonce;
        let base = old_target.unwrap_or(inst.cycle_nonce);
        let new_target = base
          .checked_add(by)
          .ok_or(Error::<T>::AutoCloseNonceOverflow)?;
        Self::ensure_auto_close_target(inst.cycle_nonce, new_target)?;
        inst.auto_close_at_cycle_nonce = Some(new_target);
        inst.updated_at = frame_system::Pallet::<T>::block_number();
        Self::deposit_event(Event::AutoCloseNonceIncremented {
          aaa_id,
          old_target,
          new_target,
          by,
        });
        Ok(())
      })?;
      Self::sync_readiness_state(aaa_id);
      Ok(())
    }

    #[pallet::call_index(15)]
    #[pallet::weight(
      T::WeightInfo::update_on_close_execution_plan().saturating_add(T::WeightInfo::close_aaa())
    )]
    pub fn update_on_close_execution_plan(
      origin: OriginFor<T>,
      aaa_id: AaaId,
      on_close_execution_plan: ExecutionPlanOf<T>,
    ) -> DispatchResult {
      ensure!(
        !on_close_execution_plan.is_empty(),
        Error::<T>::EmptyExecutionPlan
      );
      Self::validate_execution_plan_shape(&on_close_execution_plan)?;
      let snapshot = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      AaaInstances::<T>::try_mutate(aaa_id, |maybe| -> DispatchResult {
        let inst = maybe.as_mut().ok_or(Error::<T>::AaaNotFound)?;
        ensure!(
          inst.mutability == Mutability::Mutable,
          Error::<T>::ImmutableAaa
        );
        let max_steps = match inst.aaa_type {
          AaaType::User => T::MaxUserExecutionPlanSteps::get(),
          AaaType::System => T::MaxSystemExecutionPlanSteps::get(),
        };
        ensure!(
          (on_close_execution_plan.len() as u32) <= max_steps,
          Error::<T>::ExecutionPlanTooLong
        );
        if inst.aaa_type == AaaType::User {
          ensure!(
            !Self::execution_plan_contains_mint(&on_close_execution_plan),
            Error::<T>::MintNotAllowedForUserAaa
          );
        }
        let new_tracked = Self::derive_combined_funding_tracked_assets(
          &inst.execution_plan,
          &on_close_execution_plan,
        )?;
        inst.funding_tracked_assets = new_tracked.clone();
        let mut stale_keys = alloc::vec::Vec::new();
        for key in inst.funding_snapshots.keys() {
          if !new_tracked.contains(key) {
            stale_keys.push(*key);
          }
        }
        for key in stale_keys {
          let _ = inst.funding_snapshots.remove(&key);
        }
        inst.on_close_execution_plan = on_close_execution_plan;
        inst.updated_at = frame_system::Pallet::<T>::block_number();
        Self::deposit_event(Event::OnCloseExecutionPlanUpdated { aaa_id });
        Ok(())
      })?;
      Self::sync_readiness_state(aaa_id);
      Ok(())
    }
  }

  impl<T: Config> Pallet<T> {
    pub fn weight_upper_bound(task: &TaskOf<T>) -> Weight {
      // Runtime owns upper-bound pricing via coarse task classes to reduce calibration churn
      match task {
        AaaTask::Transfer { .. } | AaaTask::Burn { .. } | AaaTask::Mint { .. } => {
          T::TaskWeightInfo::simple_asset_op()
        }
        AaaTask::SplitTransfer { legs, .. } => T::TaskWeightInfo::split_transfer(legs.len() as u32),
        AaaTask::SwapExactIn { .. } | AaaTask::SwapExactOut { .. } => T::TaskWeightInfo::dex_swap(),
        AaaTask::AddLiquidity { .. } | AaaTask::RemoveLiquidity { .. } => {
          T::TaskWeightInfo::dex_liquidity()
        }
        AaaTask::Noop => T::TaskWeightInfo::noop(),
        AaaTask::Stake { .. } => T::DbWeight::get()
          .reads(2)
          .saturating_add(T::DbWeight::get().writes(2)),
        AaaTask::DonateLiquidity { .. } => {
          T::TaskWeightInfo::dex_liquidity().saturating_add(T::DbWeight::get().reads_writes(6, 6))
        }
        AaaTask::Unstake { .. } => T::DbWeight::get()
          .reads(2)
          .saturating_add(T::DbWeight::get().writes(2)),
      }
    }

    pub(crate) fn compute_cycle_weight_upper(execution_plan: &ExecutionPlanOf<T>) -> Weight {
      let mut upper =
        Weight::from_parts(5_000_000, 1000).saturating_add(T::DbWeight::get().reads_writes(2, 2));
      for step in execution_plan.iter() {
        let step_overhead = Weight::from_parts(1_000_000, 128);
        let condition_overhead =
          Weight::from_parts(500_000, 64).saturating_mul(step.conditions.len() as u64);
        upper = upper
          .saturating_add(step_overhead)
          .saturating_add(condition_overhead)
          .saturating_add(Self::weight_upper_bound(&step.task));
      }
      upper
    }

    pub(crate) fn compute_cycle_fee_upper(
      aaa_type: AaaType,
      execution_plan: &ExecutionPlanOf<T>,
    ) -> BalanceOf<T> {
      if aaa_type != AaaType::User {
        return Zero::zero();
      }
      let mut upper = T::Balance::zero();
      for step in execution_plan.iter() {
        let eval_fee = Self::compute_eval_fee(step.conditions.len() as u32);
        upper = upper.saturating_add(eval_fee);
        if !matches!(step.task, AaaTask::Noop) {
          let exec_fee = T::WeightToFee::weight_to_fee(&Self::weight_upper_bound(&step.task));
          upper = upper.saturating_add(exec_fee);
        }
      }
      upper
    }

    pub(crate) fn compute_cycle_bounds(
      aaa_type: AaaType,
      execution_plan: &ExecutionPlanOf<T>,
    ) -> (Weight, BalanceOf<T>) {
      (
        Self::compute_cycle_weight_upper(execution_plan),
        Self::compute_cycle_fee_upper(aaa_type, execution_plan),
      )
    }

    pub(crate) fn cycle_weight_upper_bound(instance: &AaaInstanceOf<T>) -> Weight {
      instance.cycle_weight_upper
    }

    pub(crate) fn cycle_fee_upper_bound(instance: &AaaInstanceOf<T>) -> BalanceOf<T> {
      instance.cycle_fee_upper
    }

    pub(crate) fn close_cycle_weight_upper_bound(instance: &AaaInstanceOf<T>) -> Weight {
      Self::compute_cycle_weight_upper(&instance.on_close_execution_plan)
    }

    pub(crate) fn close_cycle_fee_upper_bound(instance: &AaaInstanceOf<T>) -> BalanceOf<T> {
      Self::compute_cycle_fee_upper(instance.aaa_type, &instance.on_close_execution_plan)
    }

    pub(crate) fn default_on_close_execution_plan() -> ExecutionPlanOf<T> {
      BoundedVec::try_from(alloc::vec![Step {
        conditions: BoundedVec::default(),
        task: AaaTask::Noop,
        on_error: StepErrorPolicy::ContinueNextStep,
      }])
      .expect("default on-close execution_plan must fit")
    }

    fn valid_owner_mask() -> u8 {
      let max_slots = T::MaxOwnerSlots::get();
      if max_slots >= 8 {
        return u8::MAX;
      }
      if max_slots == 0 {
        return 0;
      }
      ((1u16 << max_slots) - 1) as u8
    }

    fn charge_creation_fee(owner: &T::AccountId) -> DispatchResult {
      let creation_fee = T::AaaCreationFee::get();
      if creation_fee.is_zero() {
        return Ok(());
      }
      let native = T::NativeAssetId::get();
      let fee_sink = T::FeeSink::get();
      T::AssetOps::transfer(owner, &fee_sink, native, creation_fee)
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

    fn allocate_owner_slot(
      owner: &T::AccountId,
      preferred_slot: Option<u8>,
    ) -> Result<(u8, T::AccountId), Error<T>> {
      let valid_mask = Self::valid_owner_mask();
      let mut mask = OwnerSlotMask::<T>::get(owner) & valid_mask;
      OwnerSlotMask::<T>::insert(owner, mask);
      let owner_slot = match preferred_slot {
        Some(slot) => {
          if slot >= T::MaxOwnerSlots::get() {
            return Err(Error::<T>::InvalidOwnerSlot);
          }
          if (mask & (1u8 << slot)) != 0 {
            return Err(Error::<T>::OwnerSlotOccupied);
          }
          slot
        }
        None => (0..T::MaxOwnerSlots::get())
          .find(|slot| (mask & (1u8 << *slot)) == 0)
          .ok_or(Error::<T>::OwnerSlotCapacityExceeded)?,
      };
      let sovereign_account = Self::sovereign_account_id(owner, owner_slot);
      if SovereignIndex::<T>::contains_key(&sovereign_account) {
        return Err(Error::<T>::SovereignAccountCollision);
      }
      mask |= 1u8 << owner_slot;
      OwnerSlotMask::<T>::insert(owner, mask & valid_mask);
      Ok((owner_slot, sovereign_account))
    }

    fn allocate_system_sovereign(aaa_id: AaaId) -> Result<(u8, T::AccountId), Error<T>> {
      let sovereign_account = Self::sovereign_account_id_system(aaa_id);
      if SovereignIndex::<T>::contains_key(&sovereign_account) {
        return Err(Error::<T>::SovereignAccountCollision);
      }
      Ok((SYSTEM_OWNER_SLOT_SENTINEL, sovereign_account))
    }

    fn do_create_user_aaa(
      owner: T::AccountId,
      mutability: Mutability,
      preferred_slot: Option<u8>,
      schedule: ScheduleOf<T>,
      schedule_window: Option<ScheduleWindow<BlockNumberFor<T>>>,
      execution_plan: ExecutionPlanOf<T>,
    ) -> DispatchResult {
      ensure!(
        !GlobalCircuitBreaker::<T>::get(),
        Error::<T>::GlobalCircuitBreakerActive
      );
      ensure!(!execution_plan.is_empty(), Error::<T>::EmptyExecutionPlan);
      ensure!(
        (execution_plan.len() as u32) <= T::MaxUserExecutionPlanSteps::get(),
        Error::<T>::ExecutionPlanTooLong
      );
      ensure!(
        !Self::execution_plan_contains_mint(&execution_plan),
        Error::<T>::MintNotAllowedForUserAaa
      );
      Self::do_create_aaa(
        owner,
        AaaType::User,
        mutability,
        schedule,
        schedule_window,
        execution_plan,
        preferred_slot,
        None,
      )
    }

    fn do_create_system_aaa(
      owner: T::AccountId,
      schedule: ScheduleOf<T>,
      schedule_window: Option<ScheduleWindow<BlockNumberFor<T>>>,
      execution_plan: ExecutionPlanOf<T>,
      requested_aaa_id: Option<AaaId>,
    ) -> DispatchResult {
      ensure!(
        !GlobalCircuitBreaker::<T>::get(),
        Error::<T>::GlobalCircuitBreakerActive
      );
      ensure!(!execution_plan.is_empty(), Error::<T>::EmptyExecutionPlan);
      ensure!(
        (execution_plan.len() as u32) <= T::MaxSystemExecutionPlanSteps::get(),
        Error::<T>::ExecutionPlanTooLong
      );
      Self::do_create_aaa(
        owner,
        AaaType::System,
        Mutability::Mutable,
        schedule,
        schedule_window,
        execution_plan,
        None,
        requested_aaa_id,
      )
    }

    fn do_create_aaa(
      owner: T::AccountId,
      aaa_type: AaaType,
      mutability: Mutability,
      schedule: ScheduleOf<T>,
      schedule_window: Option<ScheduleWindow<BlockNumberFor<T>>>,
      execution_plan: ExecutionPlanOf<T>,
      preferred_user_slot: Option<u8>,
      requested_aaa_id: Option<AaaId>,
    ) -> DispatchResult {
      Self::validate_schedule(&schedule)?;
      if let Some(ref window) = schedule_window {
        Self::validate_schedule_window(window)?;
      }
      Self::validate_execution_plan_shape(&execution_plan)?;
      Self::validate_probability_entropy_policy(&schedule, &execution_plan)?;
      let active_count = Self::active_instance_count();
      ensure!(
        active_count < Self::effective_active_actor_limit(),
        Error::<T>::ActiveAaaCapacityExceeded
      );
      let on_close_execution_plan = Self::default_on_close_execution_plan();
      let funding_tracked_assets =
        Self::derive_combined_funding_tracked_assets(&execution_plan, &on_close_execution_plan)?;
      let current_next_id = NextAaaId::<T>::get();
      let aaa_id = requested_aaa_id.unwrap_or(current_next_id);
      ensure!(
        !AaaInstances::<T>::contains_key(aaa_id),
        Error::<T>::AaaIdOccupied
      );
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
            Ok(result) => result,
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error.into(),
              ));
            }
          },
          AaaType::System => match Self::allocate_system_sovereign(aaa_id) {
            Ok(result) => result,
            Err(error) => {
              return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
                error.into(),
              ));
            }
          },
        };
        let (cycle_weight_upper, cycle_fee_upper) =
          Self::compute_cycle_bounds(aaa_type, &execution_plan);
        let instance = AaaInstance {
          aaa_id,
          sovereign_account: sovereign_account.clone(),
          owner: owner.clone(),
          owner_slot,
          aaa_type,
          mutability,
          is_paused: false,
          pause_reason: None,
          schedule,
          schedule_window,
          execution_plan,
          on_close_execution_plan,
          cycle_nonce: 0,
          consecutive_failures: 0,
          manual_trigger_pending: false,
          funding_snapshots: Default::default(),
          funding_tracked_assets,
          cycle_weight_upper,
          cycle_fee_upper,
          auto_close_at_cycle_nonce: None,
          created_at: now,
          updated_at: now,
          last_cycle_block: Zero::zero(),
        };
        let readiness = Self::readiness_state_from_instance(&instance);
        created_owner_slot = Some(owner_slot);
        created_sovereign_account = Some(sovereign_account.clone());
        SovereignIndex::<T>::insert(sovereign_account.clone(), aaa_id);
        AaaInstances::<T>::insert(aaa_id, instance);
        AaaReadiness::<T>::insert(aaa_id, readiness);
        if aaa_type == AaaType::System && requested_aaa_id.is_some() {
          ClosedSystemAaaIds::<T>::remove(aaa_id);
        }
        if current_next_id < next_id {
          NextAaaId::<T>::put(next_id);
        }
        if let Err(error) = T::AtomicityHook::on_create_checkpoint(aaa_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })?;
      let owner_slot = created_owner_slot.expect("atomic create always sets owner_slot");
      let sovereign_account =
        created_sovereign_account.expect("atomic create always sets sovereign_account");
      Self::deposit_event(Event::AaaCreated {
        aaa_id,
        owner,
        owner_slot,
        aaa_type,
        mutability,
        sovereign_account,
      });
      Self::prime_actor_schedule(aaa_id);
      Ok(())
    }

    fn execution_plan_contains_mint(execution_plan: &ExecutionPlanOf<T>) -> bool {
      execution_plan
        .iter()
        .any(|step| matches!(step.task, AaaTask::Mint { .. }))
    }

    fn task_is_economically_sensitive(task: &TaskOf<T>) -> bool {
      !matches!(task, AaaTask::Noop)
    }

    fn has_probabilistic_timer(schedule: &ScheduleOf<T>) -> bool {
      match &schedule.trigger {
        Trigger::Timer {
          probability: Some(probability),
          ..
        } => !probability.is_zero() && *probability < Perbill::one(),
        _ => false,
      }
    }

    fn validate_probability_entropy_policy(
      schedule: &ScheduleOf<T>,
      execution_plan: &ExecutionPlanOf<T>,
    ) -> DispatchResult {
      if !T::RequireSecureEntropyForProbabilisticTasks::get() {
        return Ok(());
      }
      if !Self::has_probabilistic_timer(schedule) {
        return Ok(());
      }
      let has_financial_step = execution_plan
        .iter()
        .any(|step| Self::task_is_economically_sensitive(&step.task));
      if !has_financial_step {
        return Ok(());
      }
      ensure!(
        T::EntropyProvider::is_secure_for_financial_probability(),
        Error::<T>::InsecureEntropyProvider
      );
      Ok(())
    }

    fn validate_schedule(schedule: &ScheduleOf<T>) -> DispatchResult {
      match &schedule.trigger {
        Trigger::Timer { every_blocks, .. } => {
          ensure!(*every_blocks > 0, Error::<T>::InvalidTriggerConfiguration);
          let max_delay: u32 = T::MaxExecutionDelayBlocks::get().saturated_into();
          ensure!(
            *every_blocks <= max_delay,
            Error::<T>::ExecutionDelayTooLong
          );
        }
        Trigger::OnAddressEvent {
          source_filter,
          asset_filter,
        } => {
          if let SourceFilter::Whitelist(list) = source_filter {
            ensure!(!list.is_empty(), Error::<T>::InvalidTriggerConfiguration);
            ensure!(
              (list.len() as u32) <= T::MaxWhitelistSize::get(),
              Error::<T>::InvalidTriggerConfiguration
            );
          }
          if let AssetFilter::Whitelist(list) = asset_filter {
            ensure!(!list.is_empty(), Error::<T>::InvalidTriggerConfiguration);
            ensure!(
              (list.len() as u32) <= T::MaxWhitelistSize::get(),
              Error::<T>::InvalidTriggerConfiguration
            );
          }
        }
        Trigger::Manual => {}
      }
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

    fn validate_schedule_window(window: &ScheduleWindow<BlockNumberFor<T>>) -> DispatchResult {
      ensure!(window.end > window.start, Error::<T>::InvalidScheduleWindow);
      ensure!(
        window.end.saturating_sub(window.start) >= T::MinWindowLength::get(),
        Error::<T>::InvalidScheduleWindow
      );
      let now = frame_system::Pallet::<T>::block_number();
      ensure!(window.start >= now, Error::<T>::InvalidScheduleWindow);
      ensure!(
        window.start.saturating_sub(now) <= T::MaxExecutionDelayBlocks::get(),
        Error::<T>::ExecutionDelayTooLong
      );
      Ok(())
    }

    fn validate_execution_plan_shape(execution_plan: &ExecutionPlanOf<T>) -> DispatchResult {
      for step in execution_plan.iter() {
        match &step.task {
          AaaTask::Transfer { .. }
          | AaaTask::SplitTransfer { .. }
          | AaaTask::Burn { .. }
          | AaaTask::Mint { .. }
          | AaaTask::RemoveLiquidity { .. } => {
            if let AaaTask::SplitTransfer { legs, .. } = &step.task {
              Self::validate_split_transfer_legs(legs)?;
            }
          }
          AaaTask::SwapExactIn { .. }
          | AaaTask::SwapExactOut { .. }
          | AaaTask::AddLiquidity { .. }
          | AaaTask::Noop
          | AaaTask::Stake { .. }
          | AaaTask::DonateLiquidity { .. }
          | AaaTask::Unstake { .. } => {}
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

      for step in execution_plan.iter() {
        match &step.task {
          AaaTask::Transfer { asset, amount, .. }
          | AaaTask::SplitTransfer { asset, amount, .. }
          | AaaTask::Burn { asset, amount }
          | AaaTask::Mint { asset, amount }
          | AaaTask::RemoveLiquidity {
            lp_asset: asset,
            amount,
          } => {
            check_amount(amount, *asset);
          }
          AaaTask::SwapExactIn {
            asset_in,
            amount_in,
            ..
          } => {
            check_amount(amount_in, *asset_in);
          }
          AaaTask::SwapExactOut {
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
          } => {
            check_amount(amount_a, *asset_a);
            check_amount(amount_b, *asset_b);
          }
          AaaTask::Stake { asset, amount } => {
            check_amount(amount, *asset);
          }
          AaaTask::DonateLiquidity {
            asset_a, amount, ..
          } => {
            check_amount(amount, *asset_a);
          }
          AaaTask::Noop | AaaTask::Unstake { .. } => {}
        }
      }

      BoundedBTreeSet::try_from(tracked).map_err(|_| Error::<T>::ExecutionPlanTooLong.into())
    }

    fn derive_combined_funding_tracked_assets(
      execution_plan: &ExecutionPlanOf<T>,
      on_close_execution_plan: &ExecutionPlanOf<T>,
    ) -> Result<BoundedBTreeSet<T::AssetId, T::MaxFundingTrackedAssets>, DispatchError> {
      let mut tracked = alloc::collections::BTreeSet::new();
      for asset in Self::derive_funding_tracked_assets(execution_plan)?
        .iter()
        .copied()
      {
        tracked.insert(asset);
      }
      for asset in Self::derive_funding_tracked_assets(on_close_execution_plan)?
        .iter()
        .copied()
      {
        tracked.insert(asset);
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

    fn ensure_control_origin(origin: OriginFor<T>, instance: &AaaInstanceOf<T>) -> DispatchResult {
      if let Ok(who) = ensure_signed(origin.clone()) {
        ensure!(who == instance.owner, Error::<T>::NotOwner);
        return Ok(());
      }
      T::SystemOrigin::ensure_origin(origin)?;
      ensure!(
        instance.aaa_type == AaaType::System,
        Error::<T>::NotGovernance
      );
      Ok(())
    }

    fn readiness_trigger_from_schedule(schedule: &ScheduleOf<T>) -> ReadinessTrigger {
      match &schedule.trigger {
        Trigger::Timer {
          every_blocks,
          probability,
        } => ReadinessTrigger::Timer {
          every_blocks: *every_blocks,
          probability: *probability,
        },
        Trigger::OnAddressEvent { .. } => ReadinessTrigger::OnAddressEvent,
        Trigger::Manual => ReadinessTrigger::Manual,
      }
    }

    pub(crate) fn readiness_state_from_instance(
      instance: &AaaInstanceOf<T>,
    ) -> AaaReadinessStateOf<T> {
      AaaReadinessState {
        aaa_type: instance.aaa_type,
        is_paused: instance.is_paused,
        trigger: Self::readiness_trigger_from_schedule(&instance.schedule),
        cooldown_blocks: instance.schedule.cooldown_blocks,
        schedule_window: instance.schedule_window,
        manual_trigger_pending: instance.manual_trigger_pending,
        cycle_nonce: instance.cycle_nonce,
        last_cycle_block: instance.last_cycle_block,
      }
    }

    pub(crate) fn sync_readiness_state(aaa_id: AaaId) {
      let Some(instance) = AaaInstances::<T>::get(aaa_id) else {
        AaaReadiness::<T>::remove(aaa_id);
        return;
      };
      AaaReadiness::<T>::insert(aaa_id, Self::readiness_state_from_instance(&instance));
    }

    fn remove_owner_slot_binding(owner: &T::AccountId, owner_slot: u8, sovereign: &T::AccountId) {
      let valid_mask = Self::valid_owner_mask();
      let mut mask = OwnerSlotMask::<T>::get(owner) & valid_mask;
      mask &= !(1u8 << owner_slot);
      if mask == 0 {
        OwnerSlotMask::<T>::remove(owner);
      } else {
        OwnerSlotMask::<T>::insert(owner, mask & valid_mask);
      }
      SovereignIndex::<T>::remove(sovereign);
    }

    pub(crate) fn close_actor(
      aaa_id: AaaId,
      instance: &AaaInstanceOf<T>,
      reason: CloseReason,
    ) -> DispatchResult {
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        let reserved_fee_remaining = Self::admit_on_close_execution_plan(instance);
        Self::execute_on_close_execution_plan(aaa_id, instance, reserved_fee_remaining);
        // Keep close-path cost independent from future wakeup backlog; delayed
        // wakeups for missing actors are discarded lazily when their bucket is due.
        Self::remove_actor_from_queues(aaa_id);
        ScheduledWakeupBlock::<T>::remove(aaa_id);
        AaaInstances::<T>::remove(aaa_id);
        AaaReadiness::<T>::remove(aaa_id);
        match instance.aaa_type {
          AaaType::User => Self::remove_owner_slot_binding(
            &instance.owner,
            instance.owner_slot,
            &instance.sovereign_account,
          ),
          AaaType::System => {
            SovereignIndex::<T>::remove(&instance.sovereign_account);
            ClosedSystemAaaIds::<T>::insert(aaa_id, ());
          }
        }
        if let Err(error) = T::AtomicityHook::on_close_checkpoint(aaa_id) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })?;
      Self::deposit_event(Event::AaaClosed { aaa_id, reason });
      Ok(())
    }

    fn admit_on_close_execution_plan(instance: &AaaInstanceOf<T>) -> BalanceOf<T> {
      if instance.aaa_type != AaaType::User {
        return Zero::zero();
      }
      let close_cycle_fee_upper = Self::close_cycle_fee_upper_bound(instance);
      Self::user_native_balance(instance).min(close_cycle_fee_upper)
    }

    fn update_idle_starvation_state(breaker_active: bool, remaining_execution_budget: Weight) {
      if !breaker_active && remaining_execution_budget.ref_time() == 0 {
        let previous = IdleStarvationBlocks::<T>::get();
        let current = previous.saturating_add(1);
        IdleStarvationBlocks::<T>::put(current);
        let threshold = T::MaxIdleStarvationBlocks::get();
        let crossed_threshold = if threshold == 0 {
          previous == 0
        } else {
          previous < threshold && current >= threshold
        };
        if crossed_threshold {
          Self::deposit_event(Event::IdleStarvationDetected {
            consecutive_blocks: current,
          });
        }
        return;
      }
      IdleStarvationBlocks::<T>::put(0);
    }

    // --- Active Actors Set Operations ---

    pub(crate) fn effective_active_actor_limit() -> u32 {
      let configured = ActiveActorLimit::<T>::get();
      if configured == 0 {
        return Self::max_configurable_active_actor_limit();
      }
      configured.min(Self::max_configurable_active_actor_limit())
    }

    pub(crate) fn max_configurable_active_actor_limit() -> u32 {
      T::MaxActiveActors::get().min(T::MaxQueueLength::get())
    }

    pub(crate) fn active_instance_count() -> u32 {
      AaaInstances::<T>::iter_keys().count() as u32
    }

    pub(crate) fn remove_actor_from_queues(aaa_id: AaaId) {
      CurrentQueue::<T>::mutate(|queue| {
        queue.retain(|id| *id != aaa_id);
      });
      NextQueue::<T>::mutate(|queue| {
        queue.retain(|id| *id != aaa_id);
      });
      ActorQueueEpoch::<T>::remove(aaa_id);
    }

    #[cfg(feature = "try-runtime")]
    pub(crate) fn do_try_state() -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      use polkadot_sdk::sp_runtime::TryRuntimeError;
      let limit = Self::effective_active_actor_limit();
      let active_count = Self::active_instance_count();
      let valid_owner_mask = Self::valid_owner_mask();
      if active_count > limit {
        return Err(TryRuntimeError::Other(
          "AaaInstances count exceeds effective active actor limit",
        ));
      }
      let mut max_id: Option<AaaId> = None;
      for (aaa_id, instance) in AaaInstances::<T>::iter() {
        max_id = Some(max_id.map_or(aaa_id, |prev| prev.max(aaa_id)));
        if !AaaReadiness::<T>::contains_key(aaa_id) {
          return Err(TryRuntimeError::Other(
            "AaaInstances entry has no matching AaaReadiness entry",
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
        match instance.aaa_type {
          AaaType::User => {
            if instance.owner_slot >= T::MaxOwnerSlots::get() {
              return Err(TryRuntimeError::Other(
                "User AAA owner_slot exceeds MaxOwnerSlots",
              ));
            }
            let owner_mask = OwnerSlotMask::<T>::get(&instance.owner) & valid_owner_mask;
            if (owner_mask & (1u8 << instance.owner_slot)) == 0 {
              return Err(TryRuntimeError::Other(
                "User AAA owner_slot is missing from OwnerSlotMask",
              ));
            }
          }
          AaaType::System => {
            if instance.owner_slot != SYSTEM_OWNER_SLOT_SENTINEL {
              return Err(TryRuntimeError::Other(
                "System AAA owner_slot is not the compatibility sentinel",
              ));
            }
          }
        }
      }
      let queue_capacity = T::MaxQueueLength::get();
      if queue_capacity < limit {
        return Err(TryRuntimeError::Other(
          "MaxQueueLength is below effective active actor limit",
        ));
      }
      let next_id = NextAaaId::<T>::get();
      if let Some(max_aaa_id) = max_id {
        if next_id <= max_aaa_id {
          return Err(TryRuntimeError::Other(
            "NextAaaId is not greater than the largest active aaa_id",
          ));
        }
      }
      for aaa_id in ClosedSystemAaaIds::<T>::iter_keys() {
        if AaaInstances::<T>::contains_key(aaa_id) {
          return Err(TryRuntimeError::Other(
            "ClosedSystemAaaIds contains an active aaa_id",
          ));
        }
      }
      let queue_epoch = QueueEpoch::<T>::get();
      let next_marker = queue_epoch.saturating_add(1);
      let mut current_seen = alloc::collections::BTreeSet::new();
      for aaa_id in CurrentQueue::<T>::get().iter().copied() {
        if !AaaInstances::<T>::contains_key(aaa_id) {
          return Err(TryRuntimeError::Other(
            "CurrentQueue contains missing aaa_id",
          ));
        }
        if !current_seen.insert(aaa_id) {
          return Err(TryRuntimeError::Other(
            "CurrentQueue contains duplicate aaa_id",
          ));
        }
        let marker = ActorQueueEpoch::<T>::get(aaa_id);
        if marker != queue_epoch && marker != next_marker {
          return Err(TryRuntimeError::Other(
            "CurrentQueue entry is missing ActorQueueEpoch marker",
          ));
        }
      }
      let mut next_seen = alloc::collections::BTreeSet::new();
      for aaa_id in NextQueue::<T>::get().iter().copied() {
        if !AaaInstances::<T>::contains_key(aaa_id) {
          return Err(TryRuntimeError::Other("NextQueue contains missing aaa_id"));
        }
        if !next_seen.insert(aaa_id) {
          return Err(TryRuntimeError::Other(
            "NextQueue contains duplicate aaa_id",
          ));
        }
        if ActorQueueEpoch::<T>::get(aaa_id) != next_marker {
          return Err(TryRuntimeError::Other(
            "NextQueue entry is missing ActorQueueEpoch marker",
          ));
        }
      }
      for (aaa_id, marker) in ActorQueueEpoch::<T>::iter() {
        if !AaaInstances::<T>::contains_key(aaa_id) {
          return Err(TryRuntimeError::Other(
            "ActorQueueEpoch contains missing aaa_id",
          ));
        }
        if marker == queue_epoch {
          if !current_seen.contains(&aaa_id) {
            return Err(TryRuntimeError::Other(
              "ActorQueueEpoch current marker has no matching CurrentQueue entry",
            ));
          }
          if next_seen.contains(&aaa_id) {
            return Err(TryRuntimeError::Other(
              "ActorQueueEpoch current marker conflicts with NextQueue membership",
            ));
          }
          continue;
        }
        if marker == next_marker {
          if !next_seen.contains(&aaa_id) {
            return Err(TryRuntimeError::Other(
              "ActorQueueEpoch next marker has no matching NextQueue entry",
            ));
          }
          continue;
        }
        return Err(TryRuntimeError::Other(
          "ActorQueueEpoch marker is not aligned with QueueEpoch",
        ));
      }
      let min_wakeup = MinWakeupBlock::<T>::get();
      for (aaa_id, block) in ScheduledWakeupBlock::<T>::iter() {
        if !AaaInstances::<T>::contains_key(aaa_id) {
          return Err(TryRuntimeError::Other(
            "ScheduledWakeupBlock contains missing aaa_id",
          ));
        }
        if !WakeupIndex::<T>::get(block).contains(&aaa_id) {
          return Err(TryRuntimeError::Other(
            "ScheduledWakeupBlock points to missing WakeupIndex entry",
          ));
        }
      }
      let mut has_wakeup = false;
      for (block, queued) in WakeupIndex::<T>::iter() {
        if queued.is_empty() {
          return Err(TryRuntimeError::Other(
            "WakeupIndex contains empty queue entry",
          ));
        }
        if let Some(min_block) = min_wakeup {
          if block < min_block {
            return Err(TryRuntimeError::Other(
              "WakeupIndex contains key below MinWakeupBlock",
            ));
          }
        }
        let mut wakeup_seen = alloc::collections::BTreeSet::new();
        for aaa_id in queued.iter().copied() {
          if !wakeup_seen.insert(aaa_id) {
            return Err(TryRuntimeError::Other(
              "WakeupIndex contains duplicate aaa_id in one block queue",
            ));
          }
          if let Some(live_block) = ScheduledWakeupBlock::<T>::get(aaa_id) {
            if live_block == block && !AaaInstances::<T>::contains_key(aaa_id) {
              return Err(TryRuntimeError::Other(
                "WakeupIndex contains live wakeup for missing aaa_id",
              ));
            }
          }
        }
        has_wakeup = true;
      }
      if min_wakeup.is_none() && has_wakeup {
        return Err(TryRuntimeError::Other(
          "MinWakeupBlock is missing while WakeupIndex is non-empty",
        ));
      }
      Ok(())
    }
  }
}
