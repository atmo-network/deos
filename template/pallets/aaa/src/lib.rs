#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

pub mod types;

mod execution;
mod scheduler;

pub mod adapters;
pub use adapters::{AssetOps, DexOps, FundingAuthority, LiquidityDonationOps, StakingOps};
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
  fn setup_add_liquidity(
    owner: &AccountId,
  ) -> Result<(AssetId, AssetId, Balance, Balance), polkadot_sdk::sp_runtime::DispatchError>;
  fn setup_donate_liquidity(
    owner: &AccountId,
  ) -> Result<(AssetId, AssetId, Balance), polkadot_sdk::sp_runtime::DispatchError>;
  fn setup_remove_liquidity_max_k(
    owner: &AccountId,
    max_scan: u32,
  ) -> Result<(AssetId, Balance), polkadot_sdk::sp_runtime::DispatchError>;
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
  fn enable_asset_ops_ingress() {}
  fn setup_address_event_ingress(
    recipient: &AccountId,
    source: &AccountId,
    amount: Balance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult;
  fn run_address_event_ingress(recipient: &AccountId) -> bool;
  fn setup_xcm_asset_deposit() -> polkadot_sdk::sp_runtime::DispatchResult;
  fn run_xcm_asset_deposit(
    recipient: &AccountId,
    source: &AccountId,
    amount: Balance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult;
  fn clear_address_event_ingress_events();
  fn run_compatibility_address_event_ingress() -> polkadot_sdk::sp_weights::Weight;
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

pub trait FeeCollector<AccountId, AssetId, Balance> {
  fn collect_fee(
    payer: &AccountId,
    fee_sink: &AccountId,
    native_asset: AssetId,
    amount: Balance,
  ) -> polkadot_sdk::frame_support::dispatch::DispatchResult;
}

pub trait AddressEventIngressHook<BlockNumber> {
  fn ingest(
    _now: BlockNumber,
    _remaining_weight: polkadot_sdk::frame_support::weights::Weight,
  ) -> polkadot_sdk::frame_support::weights::Weight;
}

impl<BlockNumber> AddressEventIngressHook<BlockNumber> for () {
  fn ingest(
    _now: BlockNumber,
    _remaining_weight: polkadot_sdk::frame_support::weights::Weight,
  ) -> polkadot_sdk::frame_support::weights::Weight {
    polkadot_sdk::frame_support::weights::Weight::zero()
  }
}

#[frame::pallet]
pub mod pallet {
  use super::{
    AddressEventIngressHook, AssetOps, AtomicityHook, DexOps, FeeCollector, FundingAuthority,
    LiquidityDonationOps, TaskWeightInfo, WeightInfo,
  };
  use crate::adapters::StakingOps as _;
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
    type FundingAuthority: FundingAuthority<Self::AccountId>;
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
    type MaxSpilloverBlocks: Get<u32>;
    #[pallet::constant]
    type MaxQueueInsertionsPerBlock: Get<u32>;
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
    /// Testable atomicity checkpoints for create/close lifecycle paths
    type AtomicityHook: AtomicityHook;

    /// Runtime ingress hook for address-event notifications
    type AddressEventIngressHook: AddressEventIngressHook<BlockNumberFor<Self>>;

    type FeeSink: Get<Self::AccountId>;
    type FeeCollector: FeeCollector<Self::AccountId, Self::AssetId, Self::Balance>;

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

  pub type FundingSourcePolicyOf<T> =
    FundingSourcePolicy<<T as frame_system::Config>::AccountId, <T as Config>::MaxWhitelistSize>;

  pub type ProgramInputOf<T> =
    ProgramInput<ScheduleOf<T>, BlockNumberFor<T>, ExecutionPlanOf<T>, FundingSourcePolicyOf<T>>;

  pub type FundingSnapshotsOf<T> = BoundedBTreeMap<
    <T as Config>::AssetId,
    FundingBatch<<T as Config>::Balance>,
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
    BalanceOf<T>,
  >;

  pub type ActorFundingStateOf<T> =
    ActorFundingState<FundingSourcePolicyOf<T>, FundingSnapshotsOf<T>, FundingTrackedAssetsOf<T>>;

  pub type DormantAaaIdentityOf<T> = DormantAaaIdentity<<T as frame_system::Config>::AccountId>;

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
  #[pallet::getter(fn actor_funding)]
  pub type ActorFunding<T: Config> =
    StorageMap<_, Blake2_128Concat, AaaId, ActorFundingStateOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn dormant_aaa_identities)]
  pub type DormantAaaIdentities<T: Config> =
    StorageMap<_, Blake2_128Concat, AaaId, DormantAaaIdentityOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn actor_identity_count)]
  pub type ActorIdentityCount<T> = StorageValue<_, u32, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn active_aaa_count)]
  pub type ActiveAaaCount<T> = StorageValue<_, u32, ValueQuery>;

  #[pallet::storage]
  pub type ClosedSystemAaaIds<T: Config> =
    StorageMap<_, Blake2_128Concat, AaaId, Mutability, OptionQuery>;

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
  pub type WakeupRetryPending<T: Config> = StorageMap<_, Blake2_128Concat, AaaId, bool, ValueQuery>;

  #[pallet::storage]
  pub type QueueEpoch<T: Config> = StorageValue<_, u64, ValueQuery>;

  #[pallet::storage]
  pub type ActorQueueEpoch<T: Config> = StorageMap<_, Blake2_128Concat, AaaId, u64, ValueQuery>;

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
  pub type AddressEventInbox<T: Config> = StorageMap<_, Blake2_128Concat, AaaId, (), OptionQuery>;

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
      Mutability,
      Schedule,
      Option<ScheduleWindow>,
      ExecutionPlan,
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
      STORAGE_VERSION.put::<Pallet<T>>();
      if ActiveActorLimit::<T>::get() == 0 {
        ActiveActorLimit::<T>::put(Pallet::<T>::max_configurable_active_actor_limit());
      }
      for (aaa_id, owner, mutability, schedule, schedule_window, execution_plan) in
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
        assert!(
          mutability == Mutability::Mutable || schedule_window.is_none(),
          "genesis System Immutable AAA must be perpetual"
        );
        let on_close_execution_plan = Pallet::<T>::default_on_close_execution_plan();
        Pallet::<T>::ensure_execution_plans_fit_idle_budget(
          AaaType::System,
          &execution_plan,
          &on_close_execution_plan,
        )
        .unwrap_or_else(|_| {
          panic!("genesis System AAA {aaa_id} exceeds the guaranteed on_idle budget")
        });
        let funding_tracked_assets = Pallet::<T>::derive_combined_funding_tracked_assets(
          &execution_plan,
          &on_close_execution_plan,
        )
        .expect("genesis execution_plan must have valid funding-tracked assets");
        let (cycle_weight_upper, cycle_fee_upper) =
          Pallet::<T>::compute_cycle_bounds(AaaType::System, &execution_plan);
        let first_eligible_at =
          Pallet::<T>::initial_eligible_at(aaa_id, &schedule, schedule_window, Zero::zero());
        let instance = AaaInstance {
          sovereign_account: sovereign_account.clone(),
          owner: owner.clone(),
          actor_class: ActorClass::System,
          mutability,
          lifecycle: ActiveLifecycle::Active,
          schedule,
          schedule_window,
          execution_plan,
          on_close_execution_plan,
          cycle_nonce: 0,
          consecutive_failures: 0,
          manual_trigger_pending: false,
          cycle_weight_upper,
          cycle_fee_upper,
          auto_close_at_cycle_nonce: None,
          first_eligible_at,
          last_cycle_block: Zero::zero(),
        };
        let active_count = Pallet::<T>::active_instance_count();
        assert!(
          active_count < T::MaxActiveActors::get(),
          "genesis active actor capacity exceeded at aaa_id={aaa_id}"
        );
        SovereignIndex::<T>::insert(&sovereign_account, aaa_id);
        frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        AaaInstances::<T>::insert(aaa_id, instance);
        ActorFunding::<T>::insert(
          aaa_id,
          ActorFundingState {
            funding_source_policy: FundingSourcePolicy::RuntimePolicy,
            funding_snapshots: Default::default(),
            funding_tracked_assets,
            has_pending_funding: false,
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
        Pallet::<T>::prime_actor_schedule(aaa_id);
      }
      for (aaa_id, owner) in T::GenesisSystemAaas::dormant_system_aaas() {
        assert!(
          !AaaInstances::<T>::contains_key(aaa_id)
            && !DormantAaaIdentities::<T>::contains_key(aaa_id),
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
        let identity = DormantAaaIdentity {
          sovereign_account: sovereign_account.clone(),
          owner,
          actor_class: ActorClass::System,
          mutability: Mutability::Mutable,
        };
        let identity_count = ActorIdentityCount::<T>::get();
        assert!(
          identity_count < T::MaxActorIdentities::get(),
          "genesis actor identity capacity exceeded at aaa_id={aaa_id}"
        );
        SovereignIndex::<T>::insert(&sovereign_account, aaa_id);
        frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        DormantAaaIdentities::<T>::insert(aaa_id, identity);
        ActorIdentityCount::<T>::put(
          identity_count
            .checked_add(1)
            .expect("genesis actor identity count must not overflow"),
        );
      }
      for aaa_id in T::GenesisSystemAaas::system_custody_accounts() {
        assert!(
          !AaaInstances::<T>::contains_key(aaa_id)
            && !DormantAaaIdentities::<T>::contains_key(aaa_id),
          "genesis custody account collides with actor identity: {aaa_id}"
        );
        let sovereign_account = Pallet::<T>::sovereign_account_id_system(aaa_id);
        assert!(
          !SovereignIndex::<T>::contains_key(&sovereign_account),
          "genesis custody account has generic sovereign index: {aaa_id}"
        );
        frame_system::Pallet::<T>::inc_providers(&sovereign_account);
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
      let base_weight = T::WeightInfo::scheduler_on_idle_base();
      if !base_weight.all_lte(remaining_weight) {
        return Weight::zero();
      }
      let breaker_active = GlobalCircuitBreaker::<T>::get();
      let ingress_limit = remaining_weight.saturating_sub(base_weight);
      let ingress_weight = if LastIngressIngestBlock::<T>::get() == Some(now) {
        Weight::zero()
      } else {
        let hook_weight = T::AddressEventIngressHook::ingest(now, ingress_limit);
        LastIngressIngestBlock::<T>::put(now);
        hook_weight
      };
      let remaining_after_ingress = ingress_limit.saturating_sub(ingress_weight);
      let sweep_weight = Self::execute_zombie_sweep(remaining_after_ingress);
      let remaining_after_housekeeping = remaining_after_ingress.saturating_sub(sweep_weight);
      Self::update_idle_starvation_state(breaker_active, remaining_after_housekeeping);
      let housekeeping_weight = base_weight
        .saturating_add(ingress_weight)
        .saturating_add(sweep_weight);
      if breaker_active {
        return housekeeping_weight;
      }
      let cycle_weight = Self::execute_cycle(remaining_after_housekeeping);
      housekeeping_weight.saturating_add(cycle_weight)
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
      kind: OnCloseStepFailureKind,
      error: DispatchError,
    },
    OnCloseStepSkipped {
      aaa_id: AaaId,
      step_index: u32,
      reason: StepSkippedReason,
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
    FundingSourcePolicyUpdated {
      aaa_id: AaaId,
    },
    FundingBatchActivated {
      aaa_id: AaaId,
      asset: T::AssetId,
      amount: BalanceOf<T>,
    },
    FundingBatchPendingAccumulated {
      aaa_id: AaaId,
      asset: T::AssetId,
      added: BalanceOf<T>,
      pending_amount: BalanceOf<T>,
    },
    FundingBatchPromoted {
      aaa_id: AaaId,
      asset: T::AssetId,
      amount: BalanceOf<T>,
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
    AaaPaused,
    EmptyExecutionPlan,
    ExecutionPlanExceedsOnIdleBudget,
    ExecutionDelayTooLong,
    GlobalCircuitBreakerActive,
    ImmutableAaa,
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
    FundingBatchOverflow,
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
      ProgramInput::Active { .. } => T::WeightInfo::create_system_aaa(),
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
    #[pallet::weight(T::WeightInfo::reopen_system_aaa())]
    pub fn reopen_system_aaa(
      origin: OriginFor<T>,
      aaa_id: AaaId,
      owner: T::AccountId,
      mutability: Mutability,
      program: ProgramInputOf<T>,
    ) -> DispatchResult {
      T::SystemOrigin::ensure_origin(origin)?;
      ensure!(mutability == Mutability::Mutable, Error::<T>::ImmutableAaa);
      ensure!(
        !AaaInstances::<T>::contains_key(aaa_id),
        Error::<T>::AaaIdOccupied
      );
      let closed_mutability =
        ClosedSystemAaaIds::<T>::get(aaa_id).ok_or(Error::<T>::SystemAaaNotClosed)?;
      ensure!(
        closed_mutability == Mutability::Mutable,
        Error::<T>::ImmutableAaa
      );
      Self::do_create_system_aaa(owner, mutability, program, Some(aaa_id))
    }

    #[pallet::call_index(4)]
    #[pallet::weight(T::WeightInfo::pause_aaa().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn pause_aaa(origin: OriginFor<T>, aaa_id: AaaId) -> DispatchResult {
      let snapshot = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      AaaInstances::<T>::try_mutate(aaa_id, |maybe| -> DispatchResult {
        let inst = maybe.as_mut().ok_or(Error::<T>::AaaNotFound)?;
        ensure!(
          inst.mutability == Mutability::Mutable,
          Error::<T>::ImmutableAaa
        );
        ensure!(!inst.lifecycle.is_paused(), Error::<T>::AaaPaused);
        inst.lifecycle = ActiveLifecycle::Paused(PauseReason::Manual);
        // Ringless: no need to remove from ring - scheduler checks is_paused flag
        Self::deposit_event(Event::AaaPaused {
          aaa_id,
          reason: PauseReason::Manual,
        });
        Ok(())
      })?;
      Ok(())
    }

    #[pallet::call_index(5)]
    #[pallet::weight(T::WeightInfo::resume_aaa().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn resume_aaa(origin: OriginFor<T>, aaa_id: AaaId) -> DispatchResult {
      let snapshot = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      AaaInstances::<T>::try_mutate(aaa_id, |maybe| -> DispatchResult {
        let inst = maybe.as_mut().ok_or(Error::<T>::AaaNotFound)?;
        ensure!(
          inst.mutability == Mutability::Mutable,
          Error::<T>::ImmutableAaa
        );
        ensure!(inst.lifecycle.is_paused(), Error::<T>::NotPaused);
        inst.lifecycle = ActiveLifecycle::Active;
        Self::deposit_event(Event::AaaResumed { aaa_id });
        Ok(())
      })?;
      Self::prime_actor_schedule(aaa_id);
      Ok(())
    }

    #[pallet::call_index(6)]
    #[pallet::weight(T::WeightInfo::manual_trigger().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
    pub fn manual_trigger(origin: OriginFor<T>, aaa_id: AaaId) -> DispatchResult {
      let snapshot = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      AaaInstances::<T>::try_mutate(aaa_id, |maybe| -> DispatchResult {
        let inst = maybe.as_mut().ok_or(Error::<T>::AaaNotFound)?;
        ensure!(!inst.lifecycle.is_paused(), Error::<T>::AaaPaused);
        inst.manual_trigger_pending = true;
        Self::deposit_event(Event::ManualTriggerSet { aaa_id });
        Ok(())
      })?;
      Self::enqueue(aaa_id);
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
      let instance = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin, &instance)?;
      Self::ensure_not_system_immutable(&instance)?;
      if Self::is_window_expired(&instance) {
        return Self::close_actor(aaa_id, &instance, CloseReason::WindowExpired);
      }
      ensure!(
        instance.mutability == Mutability::Mutable,
        Error::<T>::ImmutableAaa
      );
      ActorFunding::<T>::try_mutate(aaa_id, |maybe| -> DispatchResult {
        let funding = maybe.as_mut().ok_or(Error::<T>::AaaNotFound)?;
        funding.funding_source_policy = policy;
        Ok(())
      })?;
      Self::deposit_event(Event::FundingSourcePolicyUpdated { aaa_id });
      Ok(())
    }

    #[pallet::call_index(8)]
    #[pallet::weight(Pallet::<T>::close_dispatch_weight_upper())]
    pub fn close_aaa(origin: OriginFor<T>, aaa_id: AaaId) -> DispatchResult {
      if let Some(instance) = AaaInstances::<T>::get(aaa_id) {
        Self::ensure_control_origin(origin, &instance)?;
        Self::ensure_not_system_immutable(&instance)?;
        return Self::close_actor(aaa_id, &instance, CloseReason::OwnerInitiated);
      }
      let identity = DormantAaaIdentities::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_dormant_control_origin(origin, &identity)?;
      Self::close_dormant_actor(aaa_id, &identity, CloseReason::OwnerInitiated)
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
      let snapshot = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      let first_eligible_at = Self::initial_eligible_at(
        aaa_id,
        &schedule,
        schedule_window,
        frame_system::Pallet::<T>::block_number(),
      );
      AaaInstances::<T>::try_mutate(aaa_id, |maybe| -> DispatchResult {
        let inst = maybe.as_mut().ok_or(Error::<T>::AaaNotFound)?;
        ensure!(
          inst.mutability == Mutability::Mutable,
          Error::<T>::ImmutableAaa
        );
        inst.schedule = schedule;
        inst.schedule_window = schedule_window;
        if inst.cycle_nonce == 0 {
          inst.first_eligible_at = first_eligible_at;
        }
        Self::deposit_event(Event::ScheduleUpdated { aaa_id });
        Ok(())
      })?;
      Self::prime_actor_schedule(aaa_id);
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
    ) -> DispatchResult {
      ensure!(!execution_plan.is_empty(), Error::<T>::EmptyExecutionPlan);
      Self::validate_execution_plan_shape(&execution_plan)?;
      let snapshot = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      Self::ensure_control_origin(origin.clone(), &snapshot)?;
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      Self::ensure_execution_plans_fit_idle_budget(
        snapshot.actor_class.aaa_type(),
        &execution_plan,
        &snapshot.on_close_execution_plan,
      )?;
      ensure!(
        snapshot.mutability == Mutability::Mutable,
        Error::<T>::ImmutableAaa
      );
      let max_steps = match snapshot.actor_class.aaa_type() {
        AaaType::User => T::MaxUserExecutionPlanSteps::get(),
        AaaType::System => T::MaxSystemExecutionPlanSteps::get(),
      };
      ensure!(
        (execution_plan.len() as u32) <= max_steps,
        Error::<T>::ExecutionPlanTooLong
      );
      if snapshot.actor_class.aaa_type() == AaaType::User {
        ensure!(
          !Self::execution_plan_contains_mint(&execution_plan),
          Error::<T>::MintNotAllowedForUserAaa
        );
      }
      let new_tracked = Self::derive_combined_funding_tracked_assets(
        &execution_plan,
        &snapshot.on_close_execution_plan,
      )?;
      let mut funding = ActorFunding::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      funding.funding_tracked_assets = new_tracked.clone();
      funding
        .funding_snapshots
        .retain(|asset, _| new_tracked.contains(asset));
      funding.has_pending_funding = funding
        .funding_snapshots
        .values()
        .any(|batch| !batch.pending_amount.is_zero());
      let (cycle_weight_upper, cycle_fee_upper) =
        Self::compute_cycle_bounds(snapshot.actor_class.aaa_type(), &execution_plan);
      AaaInstances::<T>::mutate(aaa_id, |maybe| {
        let inst = maybe
          .as_mut()
          .expect("active actor existence was prevalidated");
        inst.execution_plan = execution_plan;
        inst.cycle_weight_upper = cycle_weight_upper;
        inst.cycle_fee_upper = cycle_fee_upper;
        inst.consecutive_failures = 0;
      });
      ActorFunding::<T>::insert(aaa_id, funding);
      Self::deposit_event(Event::ExecutionPlanUpdated { aaa_id });
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
      ensure!(new_limit >= active_count, Error::<T>::ActiveAaaLimitTooLow);
      let old_limit = Self::effective_active_actor_limit();
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

    #[pallet::call_index(15)]
    #[pallet::weight(T::WeightInfo::update_schedule().saturating_add(Pallet::<T>::close_dispatch_weight_upper()))]
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
        Self::deposit_event(Event::AutoCloseNonceSet { aaa_id, target });
        Ok(())
      })?;
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

    #[pallet::call_index(17)]
    #[pallet::weight(
      T::WeightInfo::update_on_close_execution_plan().saturating_add(Pallet::<T>::close_dispatch_weight_upper())
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
      Self::ensure_not_system_immutable(&snapshot)?;
      if Self::is_window_expired(&snapshot) {
        return Self::close_actor(aaa_id, &snapshot, CloseReason::WindowExpired);
      }
      Self::ensure_execution_plans_fit_idle_budget(
        snapshot.actor_class.aaa_type(),
        &snapshot.execution_plan,
        &on_close_execution_plan,
      )?;
      ensure!(
        snapshot.mutability == Mutability::Mutable,
        Error::<T>::ImmutableAaa
      );
      let max_steps = match snapshot.actor_class.aaa_type() {
        AaaType::User => T::MaxUserExecutionPlanSteps::get(),
        AaaType::System => T::MaxSystemExecutionPlanSteps::get(),
      };
      ensure!(
        (on_close_execution_plan.len() as u32) <= max_steps,
        Error::<T>::ExecutionPlanTooLong
      );
      if snapshot.actor_class.aaa_type() == AaaType::User {
        ensure!(
          !Self::execution_plan_contains_mint(&on_close_execution_plan),
          Error::<T>::MintNotAllowedForUserAaa
        );
      }
      let new_tracked = Self::derive_combined_funding_tracked_assets(
        &snapshot.execution_plan,
        &on_close_execution_plan,
      )?;
      let mut funding = ActorFunding::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
      funding.funding_tracked_assets = new_tracked.clone();
      funding
        .funding_snapshots
        .retain(|asset, _| new_tracked.contains(asset));
      funding.has_pending_funding = funding
        .funding_snapshots
        .values()
        .any(|batch| !batch.pending_amount.is_zero());
      AaaInstances::<T>::mutate(aaa_id, |maybe| {
        let inst = maybe
          .as_mut()
          .expect("active actor existence was prevalidated");
        inst.on_close_execution_plan = on_close_execution_plan;
      });
      ActorFunding::<T>::insert(aaa_id, funding);
      Self::deposit_event(Event::OnCloseExecutionPlanUpdated { aaa_id });
      Ok(())
    }

    #[pallet::call_index(21)]
    #[pallet::weight(T::WeightInfo::activate_aaa())]
    pub fn activate_aaa(
      origin: OriginFor<T>,
      aaa_id: AaaId,
      program: ProgramInputOf<T>,
    ) -> DispatchResult {
      let identity = DormantAaaIdentities::<T>::get(aaa_id).ok_or_else(|| {
        if AaaInstances::<T>::contains_key(aaa_id) {
          Error::<T>::AaaAlreadyActive
        } else {
          Error::<T>::AaaNotFound
        }
      })?;
      Self::ensure_dormant_control_origin(origin, &identity)?;
      Self::do_activate_aaa(aaa_id, identity, program)
    }

    #[pallet::call_index(22)]
    #[pallet::weight(T::WeightInfo::deactivate_aaa())]
    pub fn deactivate_aaa(origin: OriginFor<T>, aaa_id: AaaId) -> DispatchResult {
      let instance = AaaInstances::<T>::get(aaa_id).ok_or_else(|| {
        if DormantAaaIdentities::<T>::contains_key(aaa_id) {
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
      Self::do_deactivate_aaa(aaa_id, instance)
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
        AaaTask::SwapExactIn { .. } => T::TaskWeightInfo::dex_exact_in(),
        AaaTask::SwapExactOut { .. } => T::TaskWeightInfo::dex_exact_out(),
        AaaTask::AddLiquidity { .. } => T::TaskWeightInfo::add_liquidity(),
        AaaTask::RemoveLiquidity { .. } => T::TaskWeightInfo::remove_liquidity(),
        AaaTask::Stake { .. } => T::TaskWeightInfo::stake(),
        AaaTask::DonateLiquidity { .. } => T::TaskWeightInfo::donate_liquidity(),
        AaaTask::Unstake { .. } => T::TaskWeightInfo::unstake(),
      }
    }

    fn componentwise_max_weight(left: Weight, right: Weight) -> Weight {
      Weight::from_parts(
        left.ref_time().max(right.ref_time()),
        left.proof_size().max(right.proof_size()),
      )
    }

    fn maximum_task_weight_upper() -> Weight {
      let candidates = [
        T::TaskWeightInfo::simple_asset_op(),
        T::TaskWeightInfo::split_transfer(T::MaxSplitTransferLegs::get()),
        T::TaskWeightInfo::dex_exact_in(),
        T::TaskWeightInfo::dex_exact_out(),
        T::TaskWeightInfo::add_liquidity(),
        T::TaskWeightInfo::donate_liquidity(),
        T::TaskWeightInfo::remove_liquidity(),
        T::TaskWeightInfo::stake(),
        T::TaskWeightInfo::unstake(),
      ];
      candidates
        .into_iter()
        .fold(Weight::zero(), Self::componentwise_max_weight)
    }

    fn maximum_close_plan_weight_upper() -> Weight {
      let max_steps =
        T::MaxUserExecutionPlanSteps::get().max(T::MaxSystemExecutionPlanSteps::get());
      let per_step = Weight::from_parts(1_000_000, 128)
        .saturating_add(
          Weight::from_parts(500_000, 64).saturating_mul(u64::from(T::MaxConditionsPerStep::get())),
        )
        .saturating_add(Self::maximum_task_weight_upper());
      Weight::from_parts(5_000_000, 1000)
        .saturating_add(T::DbWeight::get().reads_writes(2, 2))
        .saturating_add(per_step.saturating_mul(u64::from(max_steps)))
    }

    /// Conservative FRAME dispatch weight for any explicit or lifecycle-touch close.
    pub fn close_dispatch_weight_upper() -> Weight {
      let compositional = T::WeightInfo::close_aaa()
        .saturating_add(Self::maximum_close_plan_weight_upper())
        .saturating_add(Self::close_cleanup_weight_upper());
      let measured_user_tail = T::WeightInfo::close_aaa_user_fee_bearing_tail(
        T::MaxUserExecutionPlanSteps::get(),
        T::MaxSplitTransferLegs::get(),
      );
      Self::componentwise_max_weight(compositional, measured_user_tail)
    }

    pub(crate) fn compute_cycle_weight_upper(
      aaa_type: AaaType,
      execution_plan: &ExecutionPlanOf<T>,
    ) -> Weight {
      let mut upper = Weight::from_parts(5_000_000, 1000)
        .saturating_add(T::DbWeight::get().reads_writes(2, 2))
        .saturating_add(T::WeightInfo::funding_batch_promotion(
          T::MaxFundingTrackedAssets::get(),
        ));
      for step in execution_plan.iter() {
        let step_overhead = Weight::from_parts(1_000_000, 128);
        let condition_overhead =
          Weight::from_parts(500_000, 64).saturating_mul(step.conditions.len() as u64);
        upper = upper
          .saturating_add(step_overhead)
          .saturating_add(condition_overhead)
          .saturating_add(Self::weight_upper_bound(&step.task));
        if aaa_type == AaaType::User {
          upper = upper.saturating_add(T::WeightInfo::fee_collection().saturating_mul(2));
        }
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
        let exec_fee = T::WeightToFee::weight_to_fee(&Self::weight_upper_bound(&step.task));
        upper = upper.saturating_add(exec_fee);
      }
      upper
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

    pub(crate) fn cycle_fee_upper_bound(instance: &AaaInstanceOf<T>) -> BalanceOf<T> {
      instance.cycle_fee_upper
    }

    pub(crate) fn close_cycle_weight_upper_bound(instance: &AaaInstanceOf<T>) -> Weight {
      Self::compute_cycle_weight_upper(
        instance.actor_class.aaa_type(),
        &instance.on_close_execution_plan,
      )
      .saturating_add(Self::close_cleanup_weight_upper())
    }

    pub(crate) fn close_cycle_fee_upper_bound(instance: &AaaInstanceOf<T>) -> BalanceOf<T> {
      Self::compute_cycle_fee_upper(
        instance.actor_class.aaa_type(),
        &instance.on_close_execution_plan,
      )
    }

    /// Upper-bounds one prospective run/close pair after the baseline scheduler envelope.
    /// Independently metered durable housekeeping may defer this pair across blocks.
    pub fn execution_plan_admission_weight_upper(
      aaa_type: AaaType,
      execution_plan: &ExecutionPlanOf<T>,
      on_close_execution_plan: &ExecutionPlanOf<T>,
    ) -> Weight {
      Self::scheduler_admission_overhead()
        .saturating_add(Self::compute_cycle_weight_upper(aaa_type, execution_plan))
        .saturating_add(Self::compute_cycle_weight_upper(
          aaa_type,
          on_close_execution_plan,
        ))
        .saturating_add(Self::close_cleanup_weight_upper())
    }

    fn ensure_execution_plans_fit_idle_budget(
      aaa_type: AaaType,
      execution_plan: &ExecutionPlanOf<T>,
      on_close_execution_plan: &ExecutionPlanOf<T>,
    ) -> DispatchResult {
      ensure!(
        Self::execution_plan_admission_weight_upper(
          aaa_type,
          execution_plan,
          on_close_execution_plan,
        )
        .all_lte(T::GuaranteedOnIdleWeight::get()),
        Error::<T>::ExecutionPlanExceedsOnIdleBudget
      );
      Ok(())
    }

    pub(crate) fn default_on_close_execution_plan() -> ExecutionPlanOf<T> {
      BoundedVec::default()
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

    fn do_create_dormant_aaa(
      owner: T::AccountId,
      aaa_type: AaaType,
      preferred_user_slot: Option<u8>,
      requested_aaa_id: Option<AaaId>,
    ) -> DispatchResult {
      ensure!(
        !GlobalCircuitBreaker::<T>::get(),
        Error::<T>::GlobalCircuitBreakerActive
      );
      ensure!(
        ActorIdentityCount::<T>::get() < T::MaxActorIdentities::get(),
        Error::<T>::ActorIdentityCapacityExceeded
      );
      let current_next_id = NextAaaId::<T>::get();
      let aaa_id = requested_aaa_id.unwrap_or(current_next_id);
      ensure!(
        !AaaInstances::<T>::contains_key(aaa_id)
          && !DormantAaaIdentities::<T>::contains_key(aaa_id),
        Error::<T>::AaaIdOccupied
      );
      let next_id = aaa_id.checked_add(1).ok_or(Error::<T>::AaaIdOverflow)?;
      let mut created_identity: Option<DormantAaaIdentityOf<T>> = None;
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
        let identity = DormantAaaIdentity {
          sovereign_account: sovereign_account.clone(),
          owner: owner.clone(),
          actor_class: match aaa_type {
            AaaType::User => ActorClass::User { owner_slot },
            AaaType::System => ActorClass::System,
          },
          mutability: Mutability::Mutable,
        };
        SovereignIndex::<T>::insert(&sovereign_account, aaa_id);
        DormantAaaIdentities::<T>::insert(aaa_id, &identity);
        if let Err(error) = ActorIdentityCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_add(1)
            .ok_or(Error::<T>::ActorIdentityCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if aaa_type == AaaType::System && requested_aaa_id.is_some() {
          ClosedSystemAaaIds::<T>::remove(aaa_id);
        }
        if current_next_id < next_id {
          NextAaaId::<T>::put(next_id);
        }
        frame_system::Pallet::<T>::inc_providers(&sovereign_account);
        created_identity = Some(identity);
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })?;
      let identity = created_identity.expect("atomic dormant create always sets identity");
      Self::deposit_event(Event::AaaCreated {
        aaa_id,
        owner,
        owner_slot: identity
          .actor_class
          .owner_slot()
          .unwrap_or(SYSTEM_OWNER_SLOT_SENTINEL),
        aaa_type,
        mutability: Mutability::Mutable,
        sovereign_account: identity.sovereign_account,
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
        ProgramInput::Active {
          schedule,
          schedule_window,
          execution_plan,
          on_close_execution_plan,
          funding_source_policy,
        } => Self::do_create_aaa(
          owner,
          AaaType::User,
          mutability,
          schedule,
          schedule_window,
          execution_plan,
          on_close_execution_plan,
          funding_source_policy,
          preferred_slot,
          None,
        ),
      }
    }

    fn do_create_system_aaa(
      owner: T::AccountId,
      mutability: Mutability,
      program: ProgramInputOf<T>,
      requested_aaa_id: Option<AaaId>,
    ) -> DispatchResult {
      match program {
        ProgramInput::Dormant => {
          ensure!(mutability == Mutability::Mutable, Error::<T>::ImmutableAaa);
          Self::do_create_dormant_aaa(owner, AaaType::System, None, requested_aaa_id)
        }
        ProgramInput::Active {
          schedule,
          schedule_window,
          execution_plan,
          on_close_execution_plan,
          funding_source_policy,
        } => Self::do_create_aaa(
          owner,
          AaaType::System,
          mutability,
          schedule,
          schedule_window,
          execution_plan,
          on_close_execution_plan,
          funding_source_policy,
          None,
          requested_aaa_id,
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
      on_close_execution_plan: ExecutionPlanOf<T>,
      funding_source_policy: FundingSourcePolicyOf<T>,
      preferred_user_slot: Option<u8>,
      requested_aaa_id: Option<AaaId>,
    ) -> DispatchResult {
      ensure!(
        !GlobalCircuitBreaker::<T>::get(),
        Error::<T>::GlobalCircuitBreakerActive
      );
      ensure!(!execution_plan.is_empty(), Error::<T>::EmptyExecutionPlan);
      let max_steps = match aaa_type {
        AaaType::User => T::MaxUserExecutionPlanSteps::get(),
        AaaType::System => T::MaxSystemExecutionPlanSteps::get(),
      };
      ensure!(
        (execution_plan.len() as u32) <= max_steps
          && (on_close_execution_plan.len() as u32) <= max_steps,
        Error::<T>::ExecutionPlanTooLong
      );
      if aaa_type == AaaType::User {
        ensure!(
          !Self::execution_plan_contains_mint(&execution_plan)
            && !Self::execution_plan_contains_mint(&on_close_execution_plan),
          Error::<T>::MintNotAllowedForUserAaa
        );
      }
      if aaa_type == AaaType::System && mutability == Mutability::Immutable {
        ensure!(schedule_window.is_none(), Error::<T>::InvalidScheduleWindow);
      }
      Self::validate_schedule(&schedule)?;
      if let Some(ref window) = schedule_window {
        Self::validate_schedule_window(window)?;
      }
      Self::validate_execution_plan_shape(&execution_plan)?;
      Self::validate_execution_plan_shape(&on_close_execution_plan)?;
      let active_count = Self::active_instance_count();
      ensure!(
        active_count < Self::effective_active_actor_limit(),
        Error::<T>::ActiveAaaCapacityExceeded
      );
      ensure!(
        ActorIdentityCount::<T>::get() < T::MaxActorIdentities::get(),
        Error::<T>::ActorIdentityCapacityExceeded
      );
      Self::ensure_execution_plans_fit_idle_budget(
        aaa_type,
        &execution_plan,
        &on_close_execution_plan,
      )?;
      let funding_tracked_assets =
        Self::derive_combined_funding_tracked_assets(&execution_plan, &on_close_execution_plan)?;
      let current_next_id = NextAaaId::<T>::get();
      let aaa_id = requested_aaa_id.unwrap_or(current_next_id);
      ensure!(
        !AaaInstances::<T>::contains_key(aaa_id)
          && !DormantAaaIdentities::<T>::contains_key(aaa_id),
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
        let first_eligible_at = Self::initial_eligible_at(aaa_id, &schedule, schedule_window, now);
        let instance = AaaInstance {
          sovereign_account: sovereign_account.clone(),
          owner: owner.clone(),
          actor_class: match aaa_type {
            AaaType::User => ActorClass::User { owner_slot },
            AaaType::System => ActorClass::System,
          },
          mutability,
          lifecycle: ActiveLifecycle::Active,
          schedule,
          schedule_window,
          execution_plan,
          on_close_execution_plan,
          cycle_nonce: 0,
          consecutive_failures: 0,
          manual_trigger_pending: false,
          cycle_weight_upper,
          cycle_fee_upper,
          auto_close_at_cycle_nonce: None,
          first_eligible_at,
          last_cycle_block: Zero::zero(),
        };
        created_owner_slot = Some(owner_slot);
        created_sovereign_account = Some(sovereign_account.clone());
        SovereignIndex::<T>::insert(sovereign_account.clone(), aaa_id);
        AaaInstances::<T>::insert(aaa_id, instance);
        ActorFunding::<T>::insert(
          aaa_id,
          ActorFundingState {
            funding_source_policy,
            funding_snapshots: Default::default(),
            funding_tracked_assets,
            has_pending_funding: false,
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

    fn do_activate_aaa(
      aaa_id: AaaId,
      identity: DormantAaaIdentityOf<T>,
      program: ProgramInputOf<T>,
    ) -> DispatchResult {
      let ProgramInput::Active {
        schedule,
        schedule_window,
        execution_plan,
        on_close_execution_plan,
        funding_source_policy,
      } = program
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
      let max_steps = match aaa_type {
        AaaType::User => T::MaxUserExecutionPlanSteps::get(),
        AaaType::System => T::MaxSystemExecutionPlanSteps::get(),
      };
      ensure!(
        (execution_plan.len() as u32) <= max_steps,
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
      Self::validate_execution_plan_shape(&execution_plan)?;
      ensure!(
        (on_close_execution_plan.len() as u32) <= max_steps,
        Error::<T>::ExecutionPlanTooLong
      );
      if aaa_type == AaaType::User {
        ensure!(
          !Self::execution_plan_contains_mint(&on_close_execution_plan),
          Error::<T>::MintNotAllowedForUserAaa
        );
      }
      Self::validate_execution_plan_shape(&on_close_execution_plan)?;
      Self::ensure_execution_plans_fit_idle_budget(
        aaa_type,
        &execution_plan,
        &on_close_execution_plan,
      )?;
      let funding_tracked_assets =
        Self::derive_combined_funding_tracked_assets(&execution_plan, &on_close_execution_plan)?;
      ensure!(
        Self::active_instance_count() < Self::effective_active_actor_limit(),
        Error::<T>::ActiveAaaCapacityExceeded
      );
      let (cycle_weight_upper, cycle_fee_upper) =
        Self::compute_cycle_bounds(aaa_type, &execution_plan);
      let first_eligible_at = Self::initial_eligible_at(
        aaa_id,
        &schedule,
        schedule_window,
        frame_system::Pallet::<T>::block_number(),
      );
      let instance = AaaInstance {
        sovereign_account: identity.sovereign_account,
        owner: identity.owner,
        actor_class: identity.actor_class,
        mutability: identity.mutability,
        lifecycle: ActiveLifecycle::Active,
        schedule,
        schedule_window,
        execution_plan,
        on_close_execution_plan,
        cycle_nonce: 0,
        consecutive_failures: 0,
        manual_trigger_pending: false,
        cycle_weight_upper,
        cycle_fee_upper,
        auto_close_at_cycle_nonce: None,
        first_eligible_at,
        last_cycle_block: Zero::zero(),
      };
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        if !DormantAaaIdentities::<T>::contains_key(aaa_id)
          || AaaInstances::<T>::contains_key(aaa_id)
        {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
            Error::<T>::AaaAlreadyActive.into(),
          ));
        }
        DormantAaaIdentities::<T>::remove(aaa_id);
        AaaInstances::<T>::insert(aaa_id, instance);
        ActorFunding::<T>::insert(
          aaa_id,
          ActorFundingState {
            funding_source_policy,
            funding_snapshots: Default::default(),
            funding_tracked_assets,
            has_pending_funding: false,
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
      Self::prime_actor_schedule(aaa_id);
      Ok(())
    }

    fn do_deactivate_aaa(aaa_id: AaaId, instance: AaaInstanceOf<T>) -> DispatchResult {
      let identity = DormantAaaIdentity {
        sovereign_account: instance.sovereign_account,
        owner: instance.owner,
        actor_class: instance.actor_class,
        mutability: instance.mutability,
      };
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        Self::remove_actor_from_queues(aaa_id);
        if let Some(wakeup_block) = ScheduledWakeupBlock::<T>::take(aaa_id) {
          Self::remove_wakeup_bucket_entry(wakeup_block, aaa_id);
        }
        WakeupRetryPending::<T>::remove(aaa_id);
        AddressEventInbox::<T>::remove(aaa_id);
        AaaInstances::<T>::remove(aaa_id);
        ActorFunding::<T>::remove(aaa_id);
        DormantAaaIdentities::<T>::insert(aaa_id, identity);
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
        .iter()
        .any(|step| matches!(step.task, AaaTask::Mint { .. }))
    }

    fn validate_schedule(schedule: &ScheduleOf<T>) -> DispatchResult {
      match &schedule.trigger {
        Trigger::Timer { every_blocks, .. } => {
          ensure!(*every_blocks > 0, Error::<T>::InvalidTriggerConfiguration);
          let max_delay: u32 = T::MaxExecutionDelayBlocks::get().saturated_into();
          let jitter_window = every_blocks
            .saturating_div(4)
            .min(T::MaxTimerJitterBlocks::get());
          let worst_case_jitter = jitter_window.saturating_sub(1);
          ensure!(
            every_blocks.saturating_add(worst_case_jitter) <= max_delay,
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
          AaaTask::Unstake { asset, shares } => {
            if matches!(shares, AmountResolution::PercentageOfLastFunding(_)) {
              let share_asset =
                T::StakingOps::share_asset(*asset).ok_or(Error::<T>::InvalidAmountResolution)?;
              check_amount(shares, share_asset);
            }
          }
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

    fn ensure_not_system_immutable(instance: &AaaInstanceOf<T>) -> DispatchResult {
      ensure!(
        !(instance.actor_class.aaa_type() == AaaType::System
          && instance.mutability == Mutability::Immutable),
        Error::<T>::ImmutableAaa
      );
      Ok(())
    }

    fn ensure_dormant_control_origin(
      origin: OriginFor<T>,
      identity: &DormantAaaIdentityOf<T>,
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
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        let reserved_fee_remaining = Self::admit_on_close_execution_plan(instance);
        Self::execute_on_close_execution_plan(aaa_id, instance, reserved_fee_remaining);
        Self::remove_actor_from_queues(aaa_id);
        if let Some(wakeup_block) = ScheduledWakeupBlock::<T>::take(aaa_id) {
          Self::remove_wakeup_bucket_entry(wakeup_block, aaa_id);
        }
        WakeupRetryPending::<T>::remove(aaa_id);
        AaaInstances::<T>::remove(aaa_id);
        ActorFunding::<T>::remove(aaa_id);
        if let Err(error) = ActiveAaaCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_sub(1)
            .ok_or(Error::<T>::ActiveAaaCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        if let Err(error) = ActorIdentityCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_sub(1)
            .ok_or(Error::<T>::ActorIdentityCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        match instance.actor_class {
          ActorClass::User { owner_slot } => Self::remove_owner_slot_binding(
            &instance.owner,
            owner_slot,
            &instance.sovereign_account,
          ),
          ActorClass::System => {
            SovereignIndex::<T>::remove(&instance.sovereign_account);
            ClosedSystemAaaIds::<T>::insert(aaa_id, instance.mutability);
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

    fn close_dormant_actor(
      aaa_id: AaaId,
      identity: &DormantAaaIdentityOf<T>,
      reason: CloseReason,
    ) -> DispatchResult {
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        DormantAaaIdentities::<T>::remove(aaa_id);
        if let Err(error) = ActorIdentityCount::<T>::try_mutate(|count| -> DispatchResult {
          *count = count
            .checked_sub(1)
            .ok_or(Error::<T>::ActorIdentityCountInvariant)?;
          Ok(())
        }) {
          return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
        }
        match identity.actor_class {
          ActorClass::User { owner_slot } => Self::remove_owner_slot_binding(
            &identity.owner,
            owner_slot,
            &identity.sovereign_account,
          ),
          ActorClass::System => {
            SovereignIndex::<T>::remove(&identity.sovereign_account);
            ClosedSystemAaaIds::<T>::insert(aaa_id, identity.mutability);
          }
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
      })?;
      Self::deposit_event(Event::AaaClosed { aaa_id, reason });
      Ok(())
    }

    fn admit_on_close_execution_plan(instance: &AaaInstanceOf<T>) -> BalanceOf<T> {
      if instance.actor_class.aaa_type() != AaaType::User {
        return Zero::zero();
      }
      let close_cycle_fee_upper = Self::close_cycle_fee_upper_bound(instance);
      Self::user_native_balance(instance).min(close_cycle_fee_upper)
    }

    pub(crate) fn update_idle_starvation_state(
      breaker_active: bool,
      remaining_execution_budget: Weight,
    ) {
      let exhausted =
        remaining_execution_budget.ref_time() == 0 || remaining_execution_budget.proof_size() == 0;
      if !breaker_active && exhausted {
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
      ActiveAaaCount::<T>::get()
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
      let actual_active_count = AaaInstances::<T>::iter_keys().count() as u32;
      let valid_owner_mask = Self::valid_owner_mask();
      if active_count != actual_active_count {
        return Err(TryRuntimeError::Other(
          "ActiveAaaCount does not match AaaInstances cardinality",
        ));
      }
      if active_count > limit {
        return Err(TryRuntimeError::Other(
          "AaaInstances count exceeds effective active actor limit",
        ));
      }
      let dormant_count = DormantAaaIdentities::<T>::iter_keys().count() as u32;
      let identity_count = ActorIdentityCount::<T>::get();
      if identity_count != active_count.saturating_add(dormant_count) {
        return Err(TryRuntimeError::Other(
          "ActorIdentityCount does not match active plus dormant cardinality",
        ));
      }
      if identity_count > T::MaxActorIdentities::get() {
        return Err(TryRuntimeError::Other(
          "ActorIdentityCount exceeds MaxActorIdentities",
        ));
      }
      let mut max_id: Option<AaaId> = None;
      for (aaa_id, instance) in AaaInstances::<T>::iter() {
        max_id = Some(max_id.map_or(aaa_id, |prev| prev.max(aaa_id)));
        let Some(funding) = ActorFunding::<T>::get(aaa_id) else {
          return Err(TryRuntimeError::Other(
            "AaaInstances entry has no matching ActorFunding entry",
          ));
        };
        let has_pending_funding = funding
          .funding_snapshots
          .values()
          .any(|batch| !batch.pending_amount.is_zero());
        if funding.has_pending_funding != has_pending_funding {
          return Err(TryRuntimeError::Other(
            "ActorFunding pending indication disagrees with funding batches",
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
          let owner_mask = OwnerSlotMask::<T>::get(&instance.owner) & valid_owner_mask;
          if (owner_mask & (1u8 << owner_slot)) == 0 {
            return Err(TryRuntimeError::Other(
              "User AAA owner_slot is missing from OwnerSlotMask",
            ));
          }
        }
      }
      for aaa_id in ActorFunding::<T>::iter_keys() {
        if !AaaInstances::<T>::contains_key(aaa_id) {
          return Err(TryRuntimeError::Other(
            "ActorFunding entry has no matching AaaInstances entry",
          ));
        }
      }
      let dormant_identities = DormantAaaIdentities::<T>::iter(); // deos-bypass: bounded-iter — try-state-only invariant audit
      for (aaa_id, identity) in dormant_identities {
        max_id = Some(max_id.map_or(aaa_id, |prev| prev.max(aaa_id)));
        if AaaInstances::<T>::contains_key(aaa_id)
          || ActorFunding::<T>::contains_key(aaa_id)
          || AddressEventInbox::<T>::contains_key(aaa_id)
          || ScheduledWakeupBlock::<T>::contains_key(aaa_id)
          || WakeupRetryPending::<T>::get(aaa_id)
          || ActorQueueEpoch::<T>::contains_key(aaa_id)
        {
          return Err(TryRuntimeError::Other(
            "Dormant identity owns active scheduler or inbox state",
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
            let owner_mask = OwnerSlotMask::<T>::get(&identity.owner) & valid_owner_mask;
            if (owner_mask & (1u8 << owner_slot)) == 0 {
              return Err(TryRuntimeError::Other(
                "Dormant User AAA owner_slot is missing from OwnerSlotMask",
              ));
            }
          }
          ActorClass::System if identity.mutability != Mutability::Mutable => {
            return Err(TryRuntimeError::Other("Dormant System AAA must be Mutable"));
          }
          ActorClass::System => {}
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
        if AaaInstances::<T>::contains_key(aaa_id)
          || DormantAaaIdentities::<T>::contains_key(aaa_id)
        {
          return Err(TryRuntimeError::Other(
            "ClosedSystemAaaIds contains an occupied aaa_id",
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
      for aaa_id in WakeupRetryPending::<T>::iter_keys() {
        if !AaaInstances::<T>::contains_key(aaa_id) {
          return Err(TryRuntimeError::Other(
            "WakeupRetryPending contains missing aaa_id",
          ));
        }
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
