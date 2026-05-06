#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use frame::traits::StorageVersion;
use polkadot_sdk::{
  frame_support::weights::Weight,
  sp_runtime::{DispatchError, DispatchResult, FixedU128},
};

pub use pallet::*;

pub mod weights;
pub use weights::WeightInfo;

pub trait NativeOperatorValidator<AccountId> {
  fn is_valid_operator(_account: &AccountId) -> bool {
    true
  }

  #[cfg(feature = "runtime-benchmarks")]
  fn benchmark_prepare_valid_operator(_account: &AccountId) {}
}

impl<AccountId> NativeOperatorValidator<AccountId> for () {}

pub trait NativeStakingLpAssetValidator<AssetId> {
  fn is_valid_native_staking_lp_asset(_asset_id: AssetId) -> bool {
    false
  }
}

impl<AssetId> NativeStakingLpAssetValidator<AssetId> for () {}

pub trait NativeLpAssetNamespaceInitializer {
  fn ensure_namespace() {}
}

impl NativeLpAssetNamespaceInitializer for () {}

pub trait NativeGovernanceLockProvider<AccountId, BlockNumber> {
  fn lock_until(_account: &AccountId) -> Option<BlockNumber> {
    None
  }
}

impl<AccountId, BlockNumber> NativeGovernanceLockProvider<AccountId, BlockNumber> for () {}

pub trait StakedAssetIdResolver<AssetId> {
  fn staked_asset_id(_asset_id: AssetId) -> Option<AssetId> {
    None
  }
}

impl<AssetId> StakedAssetIdResolver<AssetId> for () {}

pub trait StakedAssetMetadataProvider<AssetId> {
  fn metadata(_asset_id: AssetId) -> Option<(alloc::vec::Vec<u8>, alloc::vec::Vec<u8>, u8)> {
    None
  }
}

impl<AssetId> StakedAssetMetadataProvider<AssetId> for () {}

pub trait StakedAssetLifecycle<AccountId, AssetId> {
  fn register(_asset_id: AssetId, _staked_asset_id: AssetId, _admin: &AccountId) -> DispatchResult {
    Ok(())
  }
}

impl<AccountId, AssetId> StakedAssetLifecycle<AccountId, AssetId> for () {}

pub trait RewardGovernanceDomainResolver<AssetId, GovernanceDomainId> {
  fn reward_governance_domain(_asset_id: AssetId) -> Option<GovernanceDomainId> {
    None
  }
}

impl<AssetId, GovernanceDomainId> RewardGovernanceDomainResolver<AssetId, GovernanceDomainId>
  for ()
{
}

pub trait RewardEpochProvider<Epoch> {
  fn current_reward_epoch() -> Epoch;
}

impl<Epoch: Default> RewardEpochProvider<Epoch> for () {
  fn current_reward_epoch() -> Epoch {
    Default::default()
  }
}

pub trait RewardCoefficientProvider<AccountId, GovernanceDomainId> {
  fn reward_coefficient(_domain: GovernanceDomainId, _account: &AccountId) -> FixedU128 {
    FixedU128::from_inner(0)
  }

  #[cfg(feature = "runtime-benchmarks")]
  fn benchmark_prepare_positive_coefficient(_domain: GovernanceDomainId, _account: &AccountId) {}
}

impl<AccountId, GovernanceDomainId> RewardCoefficientProvider<AccountId, GovernanceDomainId>
  for ()
{
}

pub trait RewardBaseWeightProvider<AccountId, AssetId, Balance> {
  fn reward_base_weight(_asset_id: AssetId, _account: &AccountId) -> Option<Balance> {
    None
  }
}

impl<AccountId, AssetId, Balance> RewardBaseWeightProvider<AccountId, AssetId, Balance> for () {}

pub trait NativeNominationRewardCompounder<AccountId, Balance> {
  fn compound(
    _account: &AccountId,
    _operator: &AccountId,
    _amount: Balance,
  ) -> Result<Balance, DispatchError> {
    Err(DispatchError::Other("NominationRewardCompoundUnsupported"))
  }
}

impl<AccountId, Balance> NativeNominationRewardCompounder<AccountId, Balance> for () {}

pub trait NativeStakingReadModelProvider<AssetId, Balance> {
  fn native_staking_liquidity_pool() -> Option<(AssetId, Balance, Balance, Balance)> {
    None
  }

  fn native_lp_value(_locked_lp: Balance) -> Option<Balance> {
    None
  }
}

impl<AssetId, Balance> NativeStakingReadModelProvider<AssetId, Balance> for () {}

pub trait RewardSnapshotEventIngress<Epoch> {
  fn ingest(_epoch: Epoch, _max_scan: usize, _remaining_weight: Weight) -> Weight {
    Weight::zero()
  }
}

impl<Epoch> RewardSnapshotEventIngress<Epoch> for () {}

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AccountId, AssetId, Balance> {
  fn prepare_native_staking_lp(
    account: &AccountId,
    amount: Balance,
  ) -> Result<AssetId, DispatchError>;
  fn prepare_native_governance_asset(
    account: &AccountId,
    amount: Balance,
  ) -> Result<AssetId, DispatchError>;
}

#[cfg(feature = "runtime-benchmarks")]
impl<AccountId, AssetId, Balance> BenchmarkHelper<AccountId, AssetId, Balance> for () {
  fn prepare_native_staking_lp(
    _account: &AccountId,
    _amount: Balance,
  ) -> Result<AssetId, DispatchError> {
    Err(DispatchError::Other(
      "StakingBenchmarkHelper not configured",
    ))
  }

  fn prepare_native_governance_asset(
    _account: &AccountId,
    _amount: Balance,
  ) -> Result<AssetId, DispatchError> {
    Err(DispatchError::Other(
      "StakingBenchmarkHelper not configured",
    ))
  }
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

const STORAGE_VERSION: StorageVersion = StorageVersion::new(11);

#[frame::pallet]
pub mod pallet {
  use crate::{
    NativeGovernanceLockProvider as _, NativeLpAssetNamespaceInitializer as _,
    NativeNominationRewardCompounder as _, NativeOperatorValidator as _,
    NativeStakingLpAssetValidator as _, NativeStakingReadModelProvider as _,
    RewardBaseWeightProvider as _, RewardCoefficientProvider as _, RewardEpochProvider as _,
    RewardGovernanceDomainResolver as _, RewardSnapshotEventIngress as _,
    StakedAssetIdResolver as _, StakedAssetLifecycle as _, weights::WeightInfo as _,
  };
  use alloc::{collections::BTreeSet, vec::Vec};
  use codec::{Decode, Encode};
  use frame::prelude::*;
  use polkadot_sdk::frame_support::traits::fungibles::{Inspect, Mutate};
  use polkadot_sdk::frame_support::traits::tokens::{Fortitude, Precision, Preservation};
  use polkadot_sdk::frame_support::{PalletId, transactional, weights::Weight};
  use polkadot_sdk::sp_core::U256;
  use polkadot_sdk::sp_runtime::{
    FixedU128, Perbill,
    traits::{
      AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, CheckedSub, MaybeSerializeDeserialize,
      SaturatedConversion, Zero,
    },
  };

  #[pallet::config]
  pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
    type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    type AssetId: Parameter
      + MaxEncodedLen
      + Member
      + Copy
      + Ord
      + TypeInfo
      + MaybeSerializeDeserialize;
    type NativeStakingAssetId: Get<Self::AssetId>;
    type GovernanceDomainId: Parameter + MaxEncodedLen + Member + Copy + Ord + TypeInfo;
    type RewardEpoch: Parameter + MaxEncodedLen + Member + Copy + Ord + TypeInfo;
    type NativeOperatorValidator: crate::NativeOperatorValidator<Self::AccountId>;
    type NativeStakingLpAssetValidator: crate::NativeStakingLpAssetValidator<Self::AssetId>;
    type NativeLpAssetNamespaceInitializer: crate::NativeLpAssetNamespaceInitializer;
    type NativeGovernanceLockProvider: crate::NativeGovernanceLockProvider<Self::AccountId, BlockNumberFor<Self>>;
    type StakedAssetIdResolver: crate::StakedAssetIdResolver<Self::AssetId>;
    type StakedAssetLifecycle: crate::StakedAssetLifecycle<Self::AccountId, Self::AssetId>;
    type RewardGovernanceDomainResolver: crate::RewardGovernanceDomainResolver<Self::AssetId, Self::GovernanceDomainId>;
    type RewardEpochProvider: crate::RewardEpochProvider<Self::RewardEpoch>;
    type RewardCoefficientProvider: crate::RewardCoefficientProvider<Self::AccountId, Self::GovernanceDomainId>;
    type RewardBaseWeightProvider: crate::RewardBaseWeightProvider<Self::AccountId, Self::AssetId, Self::Balance>;
    type NativeNominationRewardCompounder: crate::NativeNominationRewardCompounder<Self::AccountId, Self::Balance>;
    type NativeStakingReadModelProvider: crate::NativeStakingReadModelProvider<Self::AssetId, Self::Balance>;
    type RewardSnapshotEventIngress: crate::RewardSnapshotEventIngress<Self::RewardEpoch>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper: crate::BenchmarkHelper<Self::AccountId, Self::AssetId, Self::Balance>;
    #[pallet::constant]
    type MaxOperatorCommission: Get<Perbill>;
    #[pallet::constant]
    type MaxRewardEventScanPerBlock: Get<u32>;
    #[pallet::constant]
    type MaxRewardRolloverAssetsPerBlock: Get<u32>;
    #[pallet::constant]
    type MaxRewardAccountsPerAssetEpoch: Get<u32>;
    #[pallet::constant]
    type MaxRewardAssetsPerGovernanceDomain: Get<u32>;
    #[pallet::constant]
    type MaxClaimEpochsPerCall: Get<u32>;
    #[pallet::constant]
    type NativeLpUnlockDelay: Get<BlockNumberFor<Self>>;
    type Balance: Parameter
      + MaxEncodedLen
      + Member
      + AtLeast32BitUnsigned
      + Default
      + Copy
      + TypeInfo
      + CheckedAdd
      + CheckedSub;
    type Assets: Inspect<Self::AccountId, AssetId = Self::AssetId, Balance = Self::Balance>
      + Mutate<Self::AccountId>;
    type PalletId: Get<PalletId>;
    type WeightInfo: crate::WeightInfo;
  }

  #[pallet::pallet]
  #[pallet::storage_version(crate::STORAGE_VERSION)]
  pub struct Pallet<T>(_);

  #[pallet::hooks]
  impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
      Self::roll_reward_epoch_if_needed()
    }

    fn on_idle(_n: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
      let epoch = T::RewardEpochProvider::current_reward_epoch();
      let max_scan = T::MaxRewardEventScanPerBlock::get() as usize;
      T::RewardSnapshotEventIngress::ingest(epoch, max_scan, remaining_weight)
    }
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct PoolState<Balance> {
    pub total_shares: Balance,
    pub accounted_balance: Balance,
    pub active_staker_count: u32,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct StakePosition<Balance> {
    pub shares: Balance,
  }

  #[derive(Clone, Debug, PartialEq, Eq)]
  pub struct StakeExposure<AccountId, Balance> {
    pub total_value: Balance,
    pub passive_value: Balance,
    pub delegated_value: Balance,
    pub delegated_operator: Option<AccountId>,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct NativeStakingLiquidityPool<AssetId, Balance> {
    pub native_asset_id: AssetId,
    pub staked_asset_id: AssetId,
    pub lp_asset_id: AssetId,
    pub reserve_native: Balance,
    pub reserve_staked: Balance,
    pub lp_total_issuance: Balance,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct NativeLockedLpPosition<Balance> {
    pub total_locked_lp: Balance,
    pub collator_locked_lp: Balance,
    pub governance_locked_lp: Balance,
    pub conservative_native_value: Option<Balance>,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct NativeCollatorLpPosition<AssetId, Balance, BlockNumber> {
    pub lp_asset_id: Option<AssetId>,
    pub locked_lp: Balance,
    pub pending_unlock_lp: Balance,
    pub pending_unlock_block: Option<BlockNumber>,
    pub conservative_native_value: Option<Balance>,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct NativeGovernanceCustodyPosition<AssetId, Balance, BlockNumber> {
    pub lp_asset_id: Option<AssetId>,
    pub governance_locked_lp: Balance,
    pub pending_governance_lp_unlock: Balance,
    pub pending_governance_lp_unlock_block: Option<BlockNumber>,
    pub asset_id: AssetId,
    pub asset_locked: Balance,
    pub pending_asset_unlock: Balance,
    pub pending_asset_unlock_block: Option<BlockNumber>,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct RewardWeightSnapshot<Epoch, Balance> {
    pub effective_from_epoch: Epoch,
    pub shares: Balance,
    pub coefficient: FixedU128,
    pub weight: Balance,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct RewardEpochRolloverState<Epoch> {
    pub from_epoch: Epoch,
    pub to_epoch: Epoch,
  }

  #[pallet::storage]
  #[pallet::getter(fn pool)]
  pub type Pools<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AssetId, PoolState<T::Balance>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn base_asset_for_staked_asset)]
  pub type LiveStakedAssetBaseAssets<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AssetId, T::AssetId, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn position)]
  pub type Positions<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::AssetId,
    Blake2_128Concat,
    T::AccountId,
    StakePosition<T::Balance>,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn operator_commission)]
  pub type OperatorCommissions<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AccountId, Perbill, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn reward_assets_for_governance_domain)]
  pub type RewardAssetsByGovernanceDomain<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::GovernanceDomainId,
    BoundedVec<T::AssetId, T::MaxRewardAssetsPerGovernanceDomain>,
    ValueQuery,
  >;

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct NativeLpLock<AssetId, Balance> {
    pub lp_asset_id: AssetId,
    pub amount: Balance,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct PendingNativeLpUnlock<AssetId, Balance, BlockNumber> {
    pub lp_asset_id: AssetId,
    pub amount: Balance,
    pub unlock_block: BlockNumber,
  }

  #[pallet::storage]
  #[pallet::getter(fn native_lp_lock)]
  pub type NativeLpLocks<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    Blake2_128Concat,
    T::AccountId,
    NativeLpLock<T::AssetId, T::Balance>,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn operator_native_lp_locked)]
  pub type OperatorNativeLpLocked<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AccountId, T::Balance, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn account_native_lp_locked)]
  pub type AccountNativeLpLocked<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AccountId, T::Balance, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn account_native_collator_lp_locked)]
  pub type AccountNativeCollatorLpLocked<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AccountId, T::Balance, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn total_native_lp_locked)]
  pub type TotalNativeLpLocked<T: Config> = StorageValue<_, T::Balance, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn pending_native_lp_unlock)]
  pub type PendingNativeLpUnlocks<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    Blake2_128Concat,
    T::AccountId,
    PendingNativeLpUnlock<T::AssetId, T::Balance, BlockNumberFor<T>>,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn native_governance_lp_lock)]
  pub type NativeGovernanceLpLocks<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    NativeLpLock<T::AssetId, T::Balance>,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn pending_native_governance_lp_unlock)]
  pub type PendingNativeGovernanceLpUnlocks<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    PendingNativeLpUnlock<T::AssetId, T::Balance, BlockNumberFor<T>>,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn native_governance_asset_locked)]
  pub type NativeGovernanceAssetLocked<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    Blake2_128Concat,
    T::AssetId,
    T::Balance,
    ValueQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn total_native_governance_asset_locked)]
  pub type TotalNativeGovernanceAssetLocked<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AssetId, T::Balance, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn pending_native_governance_asset_unlock)]
  pub type PendingNativeGovernanceAssetUnlocks<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    Blake2_128Concat,
    T::AssetId,
    PendingNativeLpUnlock<T::AssetId, T::Balance, BlockNumberFor<T>>,
    OptionQuery,
  >;

  #[pallet::genesis_config]
  pub struct GenesisConfig<T: Config> {
    pub registered_assets: Vec<T::AssetId>,
    pub _marker: core::marker::PhantomData<T>,
  }

  impl<T: Config> Default for GenesisConfig<T> {
    fn default() -> Self {
      Self {
        registered_assets: Vec::new(),
        _marker: Default::default(),
      }
    }
  }

  #[pallet::genesis_build]
  impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
    fn build(&self) {
      T::NativeLpAssetNamespaceInitializer::ensure_namespace();
      for asset_id in &self.registered_assets {
        if Pools::<T>::contains_key(asset_id) {
          continue;
        }
        Pools::<T>::insert(
          asset_id,
          PoolState {
            total_shares: Zero::zero(),
            accounted_balance: Zero::zero(),
            active_staker_count: 0,
          },
        );
        Pallet::<T>::initialize_staked_asset_for_pool(*asset_id)
          .expect("genesis staked asset initialization must succeed");
      }
    }
  }

  #[pallet::storage]
  #[pallet::getter(fn reward_epoch_accrued)]
  pub type RewardEpochAccruals<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::AssetId,
    Blake2_128Concat,
    T::RewardEpoch,
    T::Balance,
    ValueQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn reward_liability_balance)]
  pub type RewardLiabilityBalances<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AssetId, T::Balance, ValueQuery>;

  #[pallet::storage]
  pub type RewardEpochTouchedAccounts<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::RewardEpoch,
    Blake2_128Concat,
    T::AssetId,
    BoundedVec<T::AccountId, T::MaxRewardAccountsPerAssetEpoch>,
    ValueQuery,
  >;

  #[pallet::storage]
  pub type RewardActiveWeightSnapshots<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::AssetId,
    Blake2_128Concat,
    T::AccountId,
    RewardWeightSnapshot<T::RewardEpoch, T::Balance>,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn reward_active_total_weight)]
  pub type RewardActiveTotalWeights<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AssetId, T::Balance, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn reward_epoch_total_weight)]
  pub type RewardEpochTotalWeights<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::AssetId,
    Blake2_128Concat,
    T::RewardEpoch,
    T::Balance,
    ValueQuery,
  >;

  #[pallet::storage]
  pub type RewardEpochWeightSnapshots<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    (T::AssetId, T::RewardEpoch),
    Blake2_128Concat,
    T::AccountId,
    RewardWeightSnapshot<T::RewardEpoch, T::Balance>,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn reward_claimed)]
  pub type RewardClaims<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    (T::AssetId, T::RewardEpoch),
    Blake2_128Concat,
    T::AccountId,
    T::Balance,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn last_processed_reward_epoch)]
  pub type LastProcessedRewardEpoch<T: Config> = StorageValue<_, T::RewardEpoch, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn pending_reward_epoch_rollover)]
  pub type PendingRewardEpochRollover<T: Config> =
    StorageValue<_, RewardEpochRolloverState<T::RewardEpoch>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn last_reward_ingress_truncated_epoch)]
  pub type LastRewardIngressTruncatedEpoch<T: Config> =
    StorageValue<_, T::RewardEpoch, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn reward_epoch_truncated)]
  pub type RewardTruncatedEpochs<T: Config> =
    StorageMap<_, Blake2_128Concat, T::RewardEpoch, (), OptionQuery>;

  #[pallet::event]
  #[pallet::generate_deposit(pub(super) fn deposit_event)]
  pub enum Event<T: Config> {
    StakingAssetRegistered {
      asset_id: T::AssetId,
      pool_account: T::AccountId,
      reward_account: T::AccountId,
    },
    StakedAssetInitialized {
      asset_id: T::AssetId,
      staked_asset_id: T::AssetId,
      pool_account: T::AccountId,
    },
    PoolSynced {
      asset_id: T::AssetId,
      actual_balance: T::Balance,
      inflow: T::Balance,
    },
    RewardInflowRecorded {
      asset_id: T::AssetId,
      reward_account: T::AccountId,
      epoch: T::RewardEpoch,
      amount: T::Balance,
    },
    RewardSnapshotBootstrapped {
      asset_id: T::AssetId,
      account: T::AccountId,
      epoch: T::RewardEpoch,
      weight: T::Balance,
    },
    RewardClaimed {
      asset_id: T::AssetId,
      account: T::AccountId,
      epoch: T::RewardEpoch,
      reward_amount: T::Balance,
      minted_shares: T::Balance,
    },
    NominationRewardClaimed {
      account: T::AccountId,
      epoch: T::RewardEpoch,
      reward_amount: T::Balance,
    },
    NominationRewardCompounded {
      account: T::AccountId,
      operator: T::AccountId,
      epoch: T::RewardEpoch,
      reward_amount: T::Balance,
      locked_lp_amount: T::Balance,
    },
    RewardIngressTruncated {
      epoch: T::RewardEpoch,
      scanned: u32,
      max_scan: u32,
    },
    RewardTouchedAccountsOverflow {
      epoch: T::RewardEpoch,
      asset_id: T::AssetId,
      account: T::AccountId,
    },
    Staked {
      asset_id: T::AssetId,
      account: T::AccountId,
      amount_in: T::Balance,
      minted_shares: T::Balance,
    },
    LegacyPositionConverted {
      asset_id: T::AssetId,
      account: T::AccountId,
      converted_shares: T::Balance,
    },
    Unstaked {
      asset_id: T::AssetId,
      account: T::AccountId,
      burned_shares: T::Balance,
      amount_out: T::Balance,
    },
    UnownedPoolRecovered {
      asset_id: T::AssetId,
      beneficiary: T::AccountId,
      amount: T::Balance,
    },
    NativeLpLocked {
      account: T::AccountId,
      operator: T::AccountId,
      lp_asset_id: T::AssetId,
      amount: T::Balance,
      total_locked: T::Balance,
    },
    NativeLpUnlockRequested {
      account: T::AccountId,
      operator: T::AccountId,
      lp_asset_id: T::AssetId,
      amount: T::Balance,
      remaining_locked: T::Balance,
      unlock_block: BlockNumberFor<T>,
    },
    NativeLpWithdrawn {
      account: T::AccountId,
      operator: T::AccountId,
      lp_asset_id: T::AssetId,
      amount: T::Balance,
    },
    NativeLpRedelegated {
      account: T::AccountId,
      from_operator: T::AccountId,
      to_operator: T::AccountId,
      lp_asset_id: T::AssetId,
      amount: T::Balance,
    },
    NativeGovernanceLpLocked {
      account: T::AccountId,
      lp_asset_id: T::AssetId,
      amount: T::Balance,
      total_locked: T::Balance,
    },
    NativeGovernanceLpUnlockRequested {
      account: T::AccountId,
      lp_asset_id: T::AssetId,
      amount: T::Balance,
      remaining_locked: T::Balance,
      unlock_block: BlockNumberFor<T>,
    },
    NativeGovernanceLpWithdrawn {
      account: T::AccountId,
      lp_asset_id: T::AssetId,
      amount: T::Balance,
    },
    NativeGovernanceAssetLocked {
      account: T::AccountId,
      asset_id: T::AssetId,
      amount: T::Balance,
      total_locked: T::Balance,
    },
    NativeGovernanceAssetUnlockRequested {
      account: T::AccountId,
      asset_id: T::AssetId,
      amount: T::Balance,
      remaining_locked: T::Balance,
      unlock_block: BlockNumberFor<T>,
    },
    NativeGovernanceAssetWithdrawn {
      account: T::AccountId,
      asset_id: T::AssetId,
      amount: T::Balance,
    },
    OperatorCommissionSet {
      operator: T::AccountId,
      commission: Perbill,
    },
  }

  #[pallet::error]
  pub enum Error<T> {
    AssetAlreadyRegistered,
    AssetNotRegistered,
    AssetDoesNotExist,
    ZeroAmount,
    PoolOutflowDetected,
    PoolHasUnownedBalance,
    PoolNotEmpty,
    NoRecoverableBalance,
    ZeroSharesMinted,
    InsufficientShares,
    ZeroAmountOut,
    StakedAssetIdCollision,
    StakedAssetUnsupported,
    StakedAssetNotInitialized,
    StakedAssetAlreadyInitialized,
    NativeStakeRequiresDedicatedCall,
    NativeRewardRequiresDedicatedClaim,
    CannotNominateSelf,
    InvalidNativeOperatorTarget,
    NativeGovernanceLockActive,
    InvalidNativeGovernanceAsset,
    InvalidNativeLpAsset,
    NativeLpAssetMismatch,
    InsufficientLockedLp,
    NoPendingNativeLpUnlock,
    NativeLpUnlockNotReady,
    CommissionExceedsMaximum,
    RewardAccountUnderfunded,
    RewardAccountingOverflow,
    RewardEpochWeightFrozen,
    RewardEpochStillOpen,
    RewardEpochIncomplete,
    RewardAlreadyClaimed,
    DuplicateRewardEpoch,
    NoRewardClaimable,
    RewardGovernanceDomainAssetSetFull,
  }

  #[pallet::call]
  impl<T: Config> Pallet<T> {
    #[pallet::call_index(0)]
    #[pallet::weight(T::WeightInfo::register_staking_asset())]
    #[transactional]
    pub fn register_staking_asset(origin: OriginFor<T>, asset_id: T::AssetId) -> DispatchResult {
      T::AdminOrigin::ensure_origin(origin)?;
      ensure!(
        !Pools::<T>::contains_key(asset_id),
        Error::<T>::AssetAlreadyRegistered
      );
      ensure!(
        T::Assets::asset_exists(asset_id),
        Error::<T>::AssetDoesNotExist
      );
      let pool_account = Self::pool_account_for(asset_id);
      if let Some(staked_asset_id) = Self::staked_asset_id(asset_id) {
        ensure!(
          !T::Assets::asset_exists(staked_asset_id),
          Error::<T>::StakedAssetIdCollision
        );
        T::StakedAssetLifecycle::register(asset_id, staked_asset_id, &pool_account)?;
        Self::index_live_staked_asset(asset_id, staked_asset_id)?;
      }
      let accounted_balance = T::Assets::balance(asset_id, &pool_account);
      Pools::<T>::insert(
        asset_id,
        PoolState {
          total_shares: Zero::zero(),
          accounted_balance,
          active_staker_count: 0,
        },
      );
      Self::index_reward_governance_domain_asset(asset_id)?;
      Self::deposit_event(Event::StakingAssetRegistered {
        asset_id,
        pool_account: Self::pool_account_for(asset_id),
        reward_account: Self::reward_account_for(asset_id),
      });
      Ok(())
    }

    #[pallet::call_index(1)]
    #[pallet::weight(T::WeightInfo::sync_pool())]
    pub fn sync_pool(origin: OriginFor<T>, asset_id: T::AssetId) -> DispatchResult {
      let _ = ensure_signed(origin)?;
      let _ = Self::sync_pool_state(asset_id)?;
      Ok(())
    }

    #[pallet::call_index(2)]
    #[pallet::weight(T::WeightInfo::stake())]
    pub fn stake(origin: OriginFor<T>, asset_id: T::AssetId, amount: T::Balance) -> DispatchResult {
      let account = ensure_signed(origin)?;
      ensure!(
        asset_id != T::NativeStakingAssetId::get(),
        Error::<T>::NativeStakeRequiresDedicatedCall
      );
      let minted_shares = Self::do_stake(asset_id, &account, amount)?;
      Self::deposit_event(Event::Staked {
        asset_id,
        account,
        amount_in: amount,
        minted_shares,
      });
      Ok(())
    }

    #[pallet::call_index(3)]
    #[pallet::weight(T::WeightInfo::unstake())]
    pub fn unstake(
      origin: OriginFor<T>,
      asset_id: T::AssetId,
      shares: T::Balance,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      ensure!(!shares.is_zero(), Error::<T>::ZeroAmount);
      let mut pool = Self::sync_pool_state(asset_id)?;
      let using_staked_receipts = Self::uses_staked_receipts(asset_id);
      let mut position = Positions::<T>::get(asset_id, &account);
      let receipt_shares = using_staked_receipts
        .map(|staked_asset_id| T::Assets::balance(staked_asset_id, &account))
        .unwrap_or_else(Zero::zero);
      let legacy_shares = position
        .as_ref()
        .map(|stored| stored.shares)
        .unwrap_or_else(Zero::zero);
      let available_shares = receipt_shares
        .checked_add(&legacy_shares)
        .ok_or(ArithmeticError::Overflow)?;
      ensure!(!available_shares.is_zero(), Error::<T>::InsufficientShares);
      ensure!(available_shares >= shares, Error::<T>::InsufficientShares);
      let amount_out = Self::mul_div_floor(shares, pool.accounted_balance, pool.total_shares);
      ensure!(!amount_out.is_zero(), Error::<T>::ZeroAmountOut);
      let receipt_shares_to_burn = if receipt_shares >= shares {
        shares
      } else {
        receipt_shares
      };
      if let Some(staked_asset_id) = using_staked_receipts {
        if !receipt_shares_to_burn.is_zero() {
          let _ = T::Assets::burn_from(
            staked_asset_id,
            &account,
            receipt_shares_to_burn,
            Preservation::Expendable,
            Precision::Exact,
            Fortitude::Force,
          )?;
        }
      }
      let legacy_shares_to_burn = shares
        .checked_sub(&receipt_shares_to_burn)
        .ok_or(ArithmeticError::Underflow)?;
      if !legacy_shares_to_burn.is_zero() {
        if let Some(ref mut stored_position) = position {
          stored_position.shares = stored_position
            .shares
            .checked_sub(&legacy_shares_to_burn)
            .ok_or(ArithmeticError::Underflow)?;
        }
      }
      let pool_account = Self::pool_account_for(asset_id);
      T::Assets::transfer(
        asset_id,
        &pool_account,
        &account,
        amount_out,
        Preservation::Expendable,
      )?;
      pool.total_shares = pool
        .total_shares
        .checked_sub(&shares)
        .ok_or(ArithmeticError::Underflow)?;
      pool.accounted_balance = pool
        .accounted_balance
        .checked_sub(&amount_out)
        .ok_or(ArithmeticError::Underflow)?;
      match position {
        Some(stored_position) if stored_position.shares.is_zero() => {
          Positions::<T>::remove(asset_id, &account);
          pool.active_staker_count = pool.active_staker_count.saturating_sub(1);
        }
        Some(stored_position) => {
          Positions::<T>::insert(asset_id, &account, stored_position);
        }
        None => {}
      }
      Pools::<T>::insert(asset_id, pool);
      let _ = Self::note_reward_touch(asset_id, &account);
      Self::deposit_event(Event::Unstaked {
        asset_id,
        account,
        burned_shares: shares,
        amount_out,
      });
      Ok(())
    }

    #[pallet::call_index(4)]
    #[pallet::weight(T::WeightInfo::recover_unowned_pool())]
    pub fn recover_unowned_pool(
      origin: OriginFor<T>,
      asset_id: T::AssetId,
      beneficiary: T::AccountId,
    ) -> DispatchResult {
      T::AdminOrigin::ensure_origin(origin)?;
      let mut pool = Pools::<T>::get(asset_id).ok_or(Error::<T>::AssetNotRegistered)?;
      if Self::live_staked_asset_id(asset_id).is_some() {
        ensure!(pool.total_shares.is_zero(), Error::<T>::PoolNotEmpty);
      } else {
        ensure!(
          pool.total_shares.is_zero() && pool.active_staker_count == 0,
          Error::<T>::PoolNotEmpty
        );
      }
      let pool_account = Self::pool_account_for(asset_id);
      let recoverable = T::Assets::balance(asset_id, &pool_account);
      ensure!(!recoverable.is_zero(), Error::<T>::NoRecoverableBalance);
      T::Assets::transfer(
        asset_id,
        &pool_account,
        &beneficiary,
        recoverable,
        Preservation::Expendable,
      )?;
      pool.accounted_balance = Zero::zero();
      Pools::<T>::insert(asset_id, &pool);
      Self::deposit_event(Event::UnownedPoolRecovered {
        asset_id,
        beneficiary,
        amount: recoverable,
      });
      Ok(())
    }

    #[pallet::call_index(7)]
    #[pallet::weight(T::WeightInfo::set_operator_commission())]
    pub fn set_operator_commission(origin: OriginFor<T>, commission: Perbill) -> DispatchResult {
      let operator = ensure_signed(origin)?;
      ensure!(
        commission <= T::MaxOperatorCommission::get(),
        Error::<T>::CommissionExceedsMaximum
      );
      OperatorCommissions::<T>::insert(&operator, commission);
      Self::deposit_event(Event::OperatorCommissionSet {
        operator,
        commission,
      });
      Ok(())
    }

    #[pallet::call_index(8)]
    #[pallet::weight(T::WeightInfo::stake())]
    pub fn stake_native(origin: OriginFor<T>, amount: T::Balance) -> DispatchResult {
      let account = ensure_signed(origin)?;
      let asset_id = T::NativeStakingAssetId::get();
      let minted_shares = Self::do_stake(asset_id, &account, amount)?;
      Self::deposit_event(Event::Staked {
        asset_id,
        account,
        amount_in: amount,
        minted_shares,
      });
      Ok(())
    }

    #[pallet::call_index(15)]
    #[pallet::weight(T::WeightInfo::lock_native_lp_for_collator())]
    #[transactional]
    pub fn lock_native_lp_for_collator(
      origin: OriginFor<T>,
      lp_asset_id: T::AssetId,
      amount: T::Balance,
      operator: T::AccountId,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
      ensure!(account != operator, Error::<T>::CannotNominateSelf);
      ensure!(
        T::NativeOperatorValidator::is_valid_operator(&operator),
        Error::<T>::InvalidNativeOperatorTarget
      );
      ensure!(
        T::NativeStakingLpAssetValidator::is_valid_native_staking_lp_asset(lp_asset_id),
        Error::<T>::InvalidNativeLpAsset
      );
      let prior_lock = NativeLpLocks::<T>::get(&account, &operator);
      if let Some(lock) = prior_lock.as_ref() {
        ensure!(
          lock.lp_asset_id == lp_asset_id,
          Error::<T>::NativeLpAssetMismatch
        );
      }
      let prior_account_operator_amount = prior_lock
        .as_ref()
        .map(|lock| lock.amount)
        .unwrap_or_else(Zero::zero);
      let new_account_operator_amount = prior_account_operator_amount
        .checked_add(&amount)
        .ok_or(ArithmeticError::Overflow)?;
      let prior_operator_amount = OperatorNativeLpLocked::<T>::get(&operator);
      let new_operator_amount = prior_operator_amount
        .checked_add(&amount)
        .ok_or(ArithmeticError::Overflow)?;
      let new_account_amount = AccountNativeLpLocked::<T>::get(&account)
        .checked_add(&amount)
        .ok_or(ArithmeticError::Overflow)?;
      let new_account_collator_amount = AccountNativeCollatorLpLocked::<T>::get(&account)
        .checked_add(&amount)
        .ok_or(ArithmeticError::Overflow)?;
      let new_total_amount = TotalNativeLpLocked::<T>::get()
        .checked_add(&amount)
        .ok_or(ArithmeticError::Overflow)?;
      let lock_account = Self::native_lp_lock_account();
      if frame_system::Pallet::<T>::providers(&lock_account).is_zero() {
        let _ = frame_system::Pallet::<T>::inc_providers(&lock_account);
      }
      T::Assets::transfer(
        lp_asset_id,
        &account,
        &lock_account,
        amount,
        Preservation::Expendable,
      )?;
      NativeLpLocks::<T>::insert(
        &account,
        &operator,
        NativeLpLock {
          lp_asset_id,
          amount: new_account_operator_amount,
        },
      );
      OperatorNativeLpLocked::<T>::insert(&operator, new_operator_amount);
      AccountNativeLpLocked::<T>::insert(&account, new_account_amount);
      AccountNativeCollatorLpLocked::<T>::insert(&account, new_account_collator_amount);
      TotalNativeLpLocked::<T>::put(new_total_amount);
      Self::deposit_event(Event::NativeLpLocked {
        account: account.clone(),
        operator,
        lp_asset_id,
        amount,
        total_locked: new_account_operator_amount,
      });
      let _ = Self::note_reward_touch(T::NativeStakingAssetId::get(), &account);
      Ok(())
    }

    #[pallet::call_index(16)]
    #[pallet::weight(T::WeightInfo::request_unlock_native_lp())]
    #[transactional]
    pub fn request_unlock_native_lp(
      origin: OriginFor<T>,
      operator: T::AccountId,
      amount: T::Balance,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
      Self::ensure_native_governance_unlocked(&account)?;
      let mut lock =
        NativeLpLocks::<T>::get(&account, &operator).ok_or(Error::<T>::InsufficientLockedLp)?;
      ensure!(lock.amount >= amount, Error::<T>::InsufficientLockedLp);
      lock.amount = lock
        .amount
        .checked_sub(&amount)
        .ok_or(ArithmeticError::Underflow)?;
      if lock.amount.is_zero() {
        NativeLpLocks::<T>::remove(&account, &operator);
      } else {
        NativeLpLocks::<T>::insert(&account, &operator, &lock);
      }
      Self::decrease_operator_native_lp_locked(&operator, amount)?;
      Self::decrease_account_native_lp_locked(&account, amount)?;
      Self::decrease_account_native_collator_lp_locked(&account, amount)?;
      Self::decrease_total_native_lp_locked(amount)?;
      let unlock_block =
        frame_system::Pallet::<T>::block_number().saturating_add(T::NativeLpUnlockDelay::get());
      let pending = PendingNativeLpUnlocks::<T>::get(&account, &operator);
      let pending_amount = pending
        .as_ref()
        .map(|item| item.amount)
        .unwrap_or_else(Zero::zero);
      if let Some(item) = pending.as_ref() {
        ensure!(
          item.lp_asset_id == lock.lp_asset_id,
          Error::<T>::NativeLpAssetMismatch
        );
      }
      let total_pending = pending_amount
        .checked_add(&amount)
        .ok_or(ArithmeticError::Overflow)?;
      let effective_unlock_block = pending
        .as_ref()
        .map(|item| item.unlock_block.max(unlock_block))
        .unwrap_or(unlock_block);
      PendingNativeLpUnlocks::<T>::insert(
        &account,
        &operator,
        PendingNativeLpUnlock {
          lp_asset_id: lock.lp_asset_id,
          amount: total_pending,
          unlock_block: effective_unlock_block,
        },
      );
      Self::deposit_event(Event::NativeLpUnlockRequested {
        account: account.clone(),
        operator,
        lp_asset_id: lock.lp_asset_id,
        amount,
        remaining_locked: lock.amount,
        unlock_block: effective_unlock_block,
      });
      let _ = Self::note_reward_touch(T::NativeStakingAssetId::get(), &account);
      Ok(())
    }

    #[pallet::call_index(17)]
    #[pallet::weight(T::WeightInfo::withdraw_unlocked_native_lp())]
    #[transactional]
    pub fn withdraw_unlocked_native_lp(
      origin: OriginFor<T>,
      operator: T::AccountId,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      let pending = PendingNativeLpUnlocks::<T>::get(&account, &operator)
        .ok_or(Error::<T>::NoPendingNativeLpUnlock)?;
      ensure!(
        frame_system::Pallet::<T>::block_number() >= pending.unlock_block,
        Error::<T>::NativeLpUnlockNotReady
      );
      PendingNativeLpUnlocks::<T>::remove(&account, &operator);
      T::Assets::transfer(
        pending.lp_asset_id,
        &Self::native_lp_lock_account(),
        &account,
        pending.amount,
        Preservation::Expendable,
      )?;
      Self::deposit_event(Event::NativeLpWithdrawn {
        account,
        operator,
        lp_asset_id: pending.lp_asset_id,
        amount: pending.amount,
      });
      Ok(())
    }

    #[pallet::call_index(18)]
    #[pallet::weight(T::WeightInfo::redelegate_native_lp())]
    #[transactional]
    pub fn redelegate_native_lp(
      origin: OriginFor<T>,
      from_operator: T::AccountId,
      to_operator: T::AccountId,
      amount: T::Balance,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
      ensure!(
        from_operator != to_operator,
        Error::<T>::InvalidNativeOperatorTarget
      );
      ensure!(account != to_operator, Error::<T>::CannotNominateSelf);
      ensure!(
        T::NativeOperatorValidator::is_valid_operator(&to_operator),
        Error::<T>::InvalidNativeOperatorTarget
      );
      let mut from_lock = NativeLpLocks::<T>::get(&account, &from_operator)
        .ok_or(Error::<T>::InsufficientLockedLp)?;
      ensure!(from_lock.amount >= amount, Error::<T>::InsufficientLockedLp);
      let to_lock = NativeLpLocks::<T>::get(&account, &to_operator);
      if let Some(lock) = to_lock.as_ref() {
        ensure!(
          lock.lp_asset_id == from_lock.lp_asset_id,
          Error::<T>::NativeLpAssetMismatch
        );
      }
      from_lock.amount = from_lock
        .amount
        .checked_sub(&amount)
        .ok_or(ArithmeticError::Underflow)?;
      if from_lock.amount.is_zero() {
        NativeLpLocks::<T>::remove(&account, &from_operator);
      } else {
        NativeLpLocks::<T>::insert(&account, &from_operator, &from_lock);
      }
      let new_to_amount = to_lock
        .as_ref()
        .map(|lock| lock.amount)
        .unwrap_or_else(Zero::zero)
        .checked_add(&amount)
        .ok_or(ArithmeticError::Overflow)?;
      NativeLpLocks::<T>::insert(
        &account,
        &to_operator,
        NativeLpLock {
          lp_asset_id: from_lock.lp_asset_id,
          amount: new_to_amount,
        },
      );
      Self::decrease_operator_native_lp_locked(&from_operator, amount)?;
      Self::increase_operator_native_lp_locked(&to_operator, amount)?;
      Self::deposit_event(Event::NativeLpRedelegated {
        account,
        from_operator,
        to_operator,
        lp_asset_id: from_lock.lp_asset_id,
        amount,
      });
      Ok(())
    }

    #[pallet::call_index(19)]
    #[pallet::weight(T::WeightInfo::lock_native_lp_for_governance())]
    #[transactional]
    pub fn lock_native_lp_for_governance(
      origin: OriginFor<T>,
      lp_asset_id: T::AssetId,
      amount: T::Balance,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
      ensure!(
        T::NativeStakingLpAssetValidator::is_valid_native_staking_lp_asset(lp_asset_id),
        Error::<T>::InvalidNativeLpAsset
      );
      let prior_lock = NativeGovernanceLpLocks::<T>::get(&account);
      if let Some(lock) = prior_lock.as_ref() {
        ensure!(
          lock.lp_asset_id == lp_asset_id,
          Error::<T>::NativeLpAssetMismatch
        );
      }
      let new_governance_amount = prior_lock
        .as_ref()
        .map(|lock| lock.amount)
        .unwrap_or_else(Zero::zero)
        .checked_add(&amount)
        .ok_or(ArithmeticError::Overflow)?;
      let new_account_amount = AccountNativeLpLocked::<T>::get(&account)
        .checked_add(&amount)
        .ok_or(ArithmeticError::Overflow)?;
      let new_total_amount = TotalNativeLpLocked::<T>::get()
        .checked_add(&amount)
        .ok_or(ArithmeticError::Overflow)?;
      let lock_account = Self::native_lp_lock_account();
      if frame_system::Pallet::<T>::providers(&lock_account).is_zero() {
        let _ = frame_system::Pallet::<T>::inc_providers(&lock_account);
      }
      T::Assets::transfer(
        lp_asset_id,
        &account,
        &lock_account,
        amount,
        Preservation::Expendable,
      )?;
      NativeGovernanceLpLocks::<T>::insert(
        &account,
        NativeLpLock {
          lp_asset_id,
          amount: new_governance_amount,
        },
      );
      AccountNativeLpLocked::<T>::insert(&account, new_account_amount);
      TotalNativeLpLocked::<T>::put(new_total_amount);
      Self::deposit_event(Event::NativeGovernanceLpLocked {
        account,
        lp_asset_id,
        amount,
        total_locked: new_governance_amount,
      });
      Ok(())
    }

    #[pallet::call_index(20)]
    #[pallet::weight(T::WeightInfo::request_unlock_native_lp_for_governance())]
    #[transactional]
    pub fn request_unlock_native_lp_for_governance(
      origin: OriginFor<T>,
      amount: T::Balance,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
      Self::ensure_native_governance_unlocked(&account)?;
      let mut lock =
        NativeGovernanceLpLocks::<T>::get(&account).ok_or(Error::<T>::InsufficientLockedLp)?;
      ensure!(lock.amount >= amount, Error::<T>::InsufficientLockedLp);
      lock.amount = lock
        .amount
        .checked_sub(&amount)
        .ok_or(ArithmeticError::Underflow)?;
      if lock.amount.is_zero() {
        NativeGovernanceLpLocks::<T>::remove(&account);
      } else {
        NativeGovernanceLpLocks::<T>::insert(&account, &lock);
      }
      Self::decrease_account_native_lp_locked(&account, amount)?;
      Self::decrease_total_native_lp_locked(amount)?;
      let unlock_block =
        frame_system::Pallet::<T>::block_number().saturating_add(T::NativeLpUnlockDelay::get());
      let pending = PendingNativeGovernanceLpUnlocks::<T>::get(&account);
      let pending_amount = pending
        .as_ref()
        .map(|item| item.amount)
        .unwrap_or_else(Zero::zero);
      if let Some(item) = pending.as_ref() {
        ensure!(
          item.lp_asset_id == lock.lp_asset_id,
          Error::<T>::NativeLpAssetMismatch
        );
      }
      let total_pending = pending_amount
        .checked_add(&amount)
        .ok_or(ArithmeticError::Overflow)?;
      let effective_unlock_block = pending
        .as_ref()
        .map(|item| item.unlock_block.max(unlock_block))
        .unwrap_or(unlock_block);
      PendingNativeGovernanceLpUnlocks::<T>::insert(
        &account,
        PendingNativeLpUnlock {
          lp_asset_id: lock.lp_asset_id,
          amount: total_pending,
          unlock_block: effective_unlock_block,
        },
      );
      Self::deposit_event(Event::NativeGovernanceLpUnlockRequested {
        account,
        lp_asset_id: lock.lp_asset_id,
        amount,
        remaining_locked: lock.amount,
        unlock_block: effective_unlock_block,
      });
      Ok(())
    }

    #[pallet::call_index(21)]
    #[pallet::weight(T::WeightInfo::withdraw_unlocked_native_lp_for_governance())]
    #[transactional]
    pub fn withdraw_unlocked_native_lp_for_governance(origin: OriginFor<T>) -> DispatchResult {
      let account = ensure_signed(origin)?;
      let pending = PendingNativeGovernanceLpUnlocks::<T>::get(&account)
        .ok_or(Error::<T>::NoPendingNativeLpUnlock)?;
      ensure!(
        frame_system::Pallet::<T>::block_number() >= pending.unlock_block,
        Error::<T>::NativeLpUnlockNotReady
      );
      PendingNativeGovernanceLpUnlocks::<T>::remove(&account);
      T::Assets::transfer(
        pending.lp_asset_id,
        &Self::native_lp_lock_account(),
        &account,
        pending.amount,
        Preservation::Expendable,
      )?;
      Self::deposit_event(Event::NativeGovernanceLpWithdrawn {
        account,
        lp_asset_id: pending.lp_asset_id,
        amount: pending.amount,
      });
      Ok(())
    }

    #[pallet::call_index(22)]
    #[pallet::weight(T::WeightInfo::lock_native_asset_for_governance())]
    #[transactional]
    pub fn lock_native_asset_for_governance(
      origin: OriginFor<T>,
      asset_id: T::AssetId,
      amount: T::Balance,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
      ensure!(
        Self::is_native_governance_asset(asset_id),
        Error::<T>::InvalidNativeGovernanceAsset
      );
      let updated = NativeGovernanceAssetLocked::<T>::get(&account, asset_id)
        .checked_add(&amount)
        .ok_or(ArithmeticError::Overflow)?;
      let updated_total = TotalNativeGovernanceAssetLocked::<T>::get(asset_id)
        .checked_add(&amount)
        .ok_or(ArithmeticError::Overflow)?;
      let lock_account = Self::native_lp_lock_account();
      if frame_system::Pallet::<T>::providers(&lock_account).is_zero() {
        let _ = frame_system::Pallet::<T>::inc_providers(&lock_account);
      }
      T::Assets::transfer(
        asset_id,
        &account,
        &lock_account,
        amount,
        Preservation::Expendable,
      )?;
      NativeGovernanceAssetLocked::<T>::insert(&account, asset_id, updated);
      TotalNativeGovernanceAssetLocked::<T>::insert(asset_id, updated_total);
      Self::deposit_event(Event::NativeGovernanceAssetLocked {
        account,
        asset_id,
        amount,
        total_locked: updated,
      });
      Ok(())
    }

    #[pallet::call_index(23)]
    #[pallet::weight(T::WeightInfo::request_unlock_native_asset_for_governance())]
    #[transactional]
    pub fn request_unlock_native_asset_for_governance(
      origin: OriginFor<T>,
      asset_id: T::AssetId,
      amount: T::Balance,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
      Self::ensure_native_governance_unlocked(&account)?;
      let locked = NativeGovernanceAssetLocked::<T>::get(&account, asset_id);
      ensure!(locked >= amount, Error::<T>::InsufficientLockedLp);
      let updated = locked
        .checked_sub(&amount)
        .ok_or(ArithmeticError::Underflow)?;
      if updated.is_zero() {
        NativeGovernanceAssetLocked::<T>::remove(&account, asset_id);
      } else {
        NativeGovernanceAssetLocked::<T>::insert(&account, asset_id, updated);
      }
      Self::decrease_total_native_governance_asset_locked(asset_id, amount)?;
      let unlock_block =
        frame_system::Pallet::<T>::block_number().saturating_add(T::NativeLpUnlockDelay::get());
      let pending = PendingNativeGovernanceAssetUnlocks::<T>::get(&account, asset_id);
      let pending_amount = pending
        .as_ref()
        .map(|item| item.amount)
        .unwrap_or_else(Zero::zero);
      let total_pending = pending_amount
        .checked_add(&amount)
        .ok_or(ArithmeticError::Overflow)?;
      let effective_unlock_block = pending
        .as_ref()
        .map(|item| item.unlock_block.max(unlock_block))
        .unwrap_or(unlock_block);
      PendingNativeGovernanceAssetUnlocks::<T>::insert(
        &account,
        asset_id,
        PendingNativeLpUnlock {
          lp_asset_id: asset_id,
          amount: total_pending,
          unlock_block: effective_unlock_block,
        },
      );
      Self::deposit_event(Event::NativeGovernanceAssetUnlockRequested {
        account,
        asset_id,
        amount,
        remaining_locked: updated,
        unlock_block: effective_unlock_block,
      });
      Ok(())
    }

    #[pallet::call_index(24)]
    #[pallet::weight(T::WeightInfo::withdraw_unlocked_native_asset_for_governance())]
    #[transactional]
    pub fn withdraw_unlocked_native_asset_for_governance(
      origin: OriginFor<T>,
      asset_id: T::AssetId,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      let pending = PendingNativeGovernanceAssetUnlocks::<T>::get(&account, asset_id)
        .ok_or(Error::<T>::NoPendingNativeLpUnlock)?;
      ensure!(
        frame_system::Pallet::<T>::block_number() >= pending.unlock_block,
        Error::<T>::NativeLpUnlockNotReady
      );
      PendingNativeGovernanceAssetUnlocks::<T>::remove(&account, asset_id);
      T::Assets::transfer(
        asset_id,
        &Self::native_lp_lock_account(),
        &account,
        pending.amount,
        Preservation::Expendable,
      )?;
      Self::deposit_event(Event::NativeGovernanceAssetWithdrawn {
        account,
        asset_id,
        amount: pending.amount,
      });
      Ok(())
    }

    #[pallet::call_index(9)]
    #[pallet::weight(T::WeightInfo::initialize_staked_asset())]
    #[transactional]
    pub fn initialize_staked_asset(origin: OriginFor<T>, asset_id: T::AssetId) -> DispatchResult {
      T::AdminOrigin::ensure_origin(origin)?;
      Pools::<T>::get(asset_id).ok_or(Error::<T>::AssetNotRegistered)?;
      let (staked_asset_id, pool_account) = Self::initialize_staked_asset_for_pool(asset_id)?;
      Self::deposit_event(Event::StakedAssetInitialized {
        asset_id,
        staked_asset_id,
        pool_account,
      });
      Ok(())
    }

    #[pallet::call_index(10)]
    #[pallet::weight(T::WeightInfo::convert_position_to_receipt())]
    pub fn convert_position_to_receipt(
      origin: OriginFor<T>,
      asset_id: T::AssetId,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      let mut pool = Pools::<T>::get(asset_id).ok_or(Error::<T>::AssetNotRegistered)?;
      let staked_asset_id =
        Self::staked_asset_id(asset_id).ok_or(Error::<T>::StakedAssetUnsupported)?;
      ensure!(
        T::Assets::asset_exists(staked_asset_id),
        Error::<T>::StakedAssetNotInitialized
      );
      let position =
        Positions::<T>::get(asset_id, &account).ok_or(Error::<T>::InsufficientShares)?;
      ensure!(!position.shares.is_zero(), Error::<T>::InsufficientShares);
      let _ = T::Assets::mint_into(staked_asset_id, &account, position.shares)?;
      Positions::<T>::remove(asset_id, &account);
      pool.active_staker_count = pool.active_staker_count.saturating_sub(1);
      Pools::<T>::insert(asset_id, pool);
      Self::deposit_event(Event::LegacyPositionConverted {
        asset_id,
        account,
        converted_shares: position.shares,
      });
      Ok(())
    }

    #[pallet::call_index(11)]
    #[pallet::weight(T::WeightInfo::bootstrap_reward_snapshot())]
    pub fn bootstrap_reward_snapshot(
      origin: OriginFor<T>,
      asset_id: T::AssetId,
      account: T::AccountId,
    ) -> DispatchResult {
      T::AdminOrigin::ensure_origin(origin)?;
      Pools::<T>::get(asset_id).ok_or(Error::<T>::AssetNotRegistered)?;
      let epoch = T::RewardEpochProvider::current_reward_epoch();
      ensure!(
        !RewardEpochTotalWeights::<T>::contains_key(asset_id, epoch),
        Error::<T>::RewardEpochWeightFrozen
      );
      let snapshot = Self::roll_reward_account_snapshot(asset_id, epoch, &account);
      Self::deposit_event(Event::RewardSnapshotBootstrapped {
        asset_id,
        account,
        epoch,
        weight: snapshot.weight,
      });
      Ok(())
    }

    #[pallet::call_index(12)]
    #[pallet::weight(T::WeightInfo::claim_reward())]
    pub fn claim_reward(
      origin: OriginFor<T>,
      asset_id: T::AssetId,
      epoch: T::RewardEpoch,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      ensure!(
        asset_id != T::NativeStakingAssetId::get(),
        Error::<T>::NativeRewardRequiresDedicatedClaim
      );
      Pools::<T>::get(asset_id).ok_or(Error::<T>::AssetNotRegistered)?;
      let current_epoch = T::RewardEpochProvider::current_reward_epoch();
      Self::ensure_reward_epoch_claimable(&account, asset_id, epoch, current_epoch)?;
      let reward_amount = Self::do_claim_reward(&account, asset_id, epoch)?;
      ensure!(!reward_amount.is_zero(), Error::<T>::NoRewardClaimable);
      Ok(())
    }

    #[pallet::call_index(25)]
    #[pallet::weight(T::WeightInfo::claim_nomination_reward())]
    #[transactional]
    pub fn claim_nomination_reward(origin: OriginFor<T>, epoch: T::RewardEpoch) -> DispatchResult {
      let account = ensure_signed(origin)?;
      let asset_id = T::NativeStakingAssetId::get();
      Pools::<T>::get(asset_id).ok_or(Error::<T>::AssetNotRegistered)?;
      let current_epoch = T::RewardEpochProvider::current_reward_epoch();
      Self::ensure_reward_epoch_claimable(&account, asset_id, epoch, current_epoch)?;
      let reward_amount = Self::do_claim_nomination_reward(&account, epoch)?;
      ensure!(!reward_amount.is_zero(), Error::<T>::NoRewardClaimable);
      Ok(())
    }

    #[pallet::call_index(26)]
    #[pallet::weight(T::WeightInfo::claim_and_compound_nomination_reward())]
    #[transactional]
    pub fn claim_and_compound_nomination_reward(
      origin: OriginFor<T>,
      epoch: T::RewardEpoch,
      operator: T::AccountId,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      let asset_id = T::NativeStakingAssetId::get();
      Pools::<T>::get(asset_id).ok_or(Error::<T>::AssetNotRegistered)?;
      let current_epoch = T::RewardEpochProvider::current_reward_epoch();
      Self::ensure_reward_epoch_claimable(&account, asset_id, epoch, current_epoch)?;
      let reward_amount = Self::do_claim_nomination_reward(&account, epoch)?;
      ensure!(!reward_amount.is_zero(), Error::<T>::NoRewardClaimable);
      let locked_lp_amount =
        T::NativeNominationRewardCompounder::compound(&account, &operator, reward_amount)?;
      ensure!(!locked_lp_amount.is_zero(), Error::<T>::NoRewardClaimable);
      Self::deposit_event(Event::NominationRewardCompounded {
        account,
        operator,
        epoch,
        reward_amount,
        locked_lp_amount,
      });
      Ok(())
    }

    #[pallet::call_index(27)]
    #[pallet::weight(T::WeightInfo::claim_nomination_reward_batch(epochs.len().saturated_into()))]
    #[transactional]
    pub fn claim_nomination_reward_batch(
      origin: OriginFor<T>,
      epochs: BoundedVec<T::RewardEpoch, T::MaxClaimEpochsPerCall>,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      let asset_id = T::NativeStakingAssetId::get();
      Pools::<T>::get(asset_id).ok_or(Error::<T>::AssetNotRegistered)?;
      let current_epoch = T::RewardEpochProvider::current_reward_epoch();
      let mut seen_epochs = BTreeSet::new();
      let mut index = 0usize;
      while index < epochs.len() {
        let epoch = epochs[index];
        ensure!(seen_epochs.insert(epoch), Error::<T>::DuplicateRewardEpoch);
        Self::ensure_reward_epoch_claimable(&account, asset_id, epoch, current_epoch)?;
        index += 1;
      }
      let mut claimed_any = false;
      let mut index = 0usize;
      while index < epochs.len() {
        let reward_amount = Self::do_claim_nomination_reward(&account, epochs[index])?;
        claimed_any = claimed_any || !reward_amount.is_zero();
        index += 1;
      }
      ensure!(claimed_any, Error::<T>::NoRewardClaimable);
      Ok(())
    }

    #[pallet::call_index(13)]
    #[pallet::weight(T::WeightInfo::claim_reward_batch(epochs.len().saturated_into()))]
    pub fn claim_reward_batch(
      origin: OriginFor<T>,
      asset_id: T::AssetId,
      epochs: BoundedVec<T::RewardEpoch, T::MaxClaimEpochsPerCall>,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      ensure!(
        asset_id != T::NativeStakingAssetId::get(),
        Error::<T>::NativeRewardRequiresDedicatedClaim
      );
      Pools::<T>::get(asset_id).ok_or(Error::<T>::AssetNotRegistered)?;
      let current_epoch = T::RewardEpochProvider::current_reward_epoch();
      let mut seen_epochs = BTreeSet::new();
      for epoch in epochs.iter().copied() {
        ensure!(seen_epochs.insert(epoch), Error::<T>::DuplicateRewardEpoch);
        Self::ensure_reward_epoch_claimable(&account, asset_id, epoch, current_epoch)?;
      }
      let mut claimed_any = false;
      for epoch in epochs.into_iter() {
        let reward_amount = Self::do_claim_reward(&account, asset_id, epoch)?;
        claimed_any = claimed_any || !reward_amount.is_zero();
      }
      ensure!(claimed_any, Error::<T>::NoRewardClaimable);
      Ok(())
    }
  }

  impl<T: Config> Pallet<T> {
    fn do_stake(
      asset_id: T::AssetId,
      account: &T::AccountId,
      amount: T::Balance,
    ) -> Result<T::Balance, DispatchError> {
      Self::credit_stake_from(asset_id, account, account, amount, Preservation::Protect)
    }

    fn ensure_reward_epoch_claimable(
      account: &T::AccountId,
      asset_id: T::AssetId,
      epoch: T::RewardEpoch,
      current_epoch: T::RewardEpoch,
    ) -> DispatchResult {
      ensure!(epoch < current_epoch, Error::<T>::RewardEpochStillOpen);
      ensure!(
        !RewardTruncatedEpochs::<T>::contains_key(epoch),
        Error::<T>::RewardEpochIncomplete
      );
      ensure!(
        !RewardClaims::<T>::contains_key((asset_id, epoch), account),
        Error::<T>::RewardAlreadyClaimed
      );
      Ok(())
    }

    fn do_claim_nomination_reward(
      account: &T::AccountId,
      epoch: T::RewardEpoch,
    ) -> Result<T::Balance, DispatchError> {
      let asset_id = T::NativeStakingAssetId::get();
      let reward_amount = match Self::reward_claimable(asset_id, epoch, account) {
        Some(reward_amount) => reward_amount,
        None => return Ok(Zero::zero()),
      };
      let reward_account = Self::reward_account_for(asset_id);
      let actual_balance = T::Assets::balance(asset_id, &reward_account);
      ensure!(
        actual_balance >= reward_amount,
        Error::<T>::RewardAccountUnderfunded
      );
      let liability_after = RewardLiabilityBalances::<T>::get(asset_id)
        .checked_sub(&reward_amount)
        .ok_or(Error::<T>::RewardAccountingOverflow)?;
      T::Assets::transfer(
        asset_id,
        &reward_account,
        account,
        reward_amount,
        Preservation::Expendable,
      )?;
      RewardLiabilityBalances::<T>::insert(asset_id, liability_after);
      RewardClaims::<T>::insert((asset_id, epoch), account, reward_amount);
      Self::deposit_event(Event::NominationRewardClaimed {
        account: account.clone(),
        epoch,
        reward_amount,
      });
      Ok(reward_amount)
    }

    fn do_claim_reward(
      account: &T::AccountId,
      asset_id: T::AssetId,
      epoch: T::RewardEpoch,
    ) -> Result<T::Balance, DispatchError> {
      let reward_amount = match Self::reward_claimable(asset_id, epoch, account) {
        Some(reward_amount) => reward_amount,
        None => return Ok(Zero::zero()),
      };
      let reward_account = Self::reward_account_for(asset_id);
      let actual_balance = T::Assets::balance(asset_id, &reward_account);
      ensure!(
        actual_balance >= reward_amount,
        Error::<T>::RewardAccountUnderfunded
      );
      let liability_after = RewardLiabilityBalances::<T>::get(asset_id)
        .checked_sub(&reward_amount)
        .ok_or(Error::<T>::RewardAccountingOverflow)?;
      let _ = Self::live_staked_asset_id(asset_id).ok_or(Error::<T>::StakedAssetNotInitialized)?;
      let minted_shares = Self::credit_stake_from(
        asset_id,
        &reward_account,
        account,
        reward_amount,
        Preservation::Expendable,
      )?;
      RewardLiabilityBalances::<T>::insert(asset_id, liability_after);
      RewardClaims::<T>::insert((asset_id, epoch), account, reward_amount);
      Self::deposit_event(Event::RewardClaimed {
        asset_id,
        account: account.clone(),
        epoch,
        reward_amount,
        minted_shares,
      });
      Ok(reward_amount)
    }

    fn credit_stake_from(
      asset_id: T::AssetId,
      funding_account: &T::AccountId,
      beneficiary: &T::AccountId,
      amount: T::Balance,
      preservation: Preservation,
    ) -> Result<T::Balance, DispatchError> {
      ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
      let mut pool = Self::sync_pool_state(asset_id)?;
      ensure!(
        !(pool.total_shares.is_zero() && !pool.accounted_balance.is_zero()),
        Error::<T>::PoolHasUnownedBalance
      );
      let minted_shares = if pool.total_shares.is_zero() {
        amount
      } else {
        Self::mul_div_floor(amount, pool.total_shares, pool.accounted_balance)
      };
      ensure!(!minted_shares.is_zero(), Error::<T>::ZeroSharesMinted);
      let staked_asset_id_for_mint = Self::uses_staked_receipts(asset_id);
      let pool_account = Self::pool_account_for(asset_id);
      T::Assets::transfer(
        asset_id,
        funding_account,
        &pool_account,
        amount,
        preservation,
      )?;
      pool.total_shares = pool
        .total_shares
        .checked_add(&minted_shares)
        .ok_or(ArithmeticError::Overflow)?;
      pool.accounted_balance = pool
        .accounted_balance
        .checked_add(&amount)
        .ok_or(ArithmeticError::Overflow)?;
      if let Some(staked_asset_id) = staked_asset_id_for_mint {
        let _ = T::Assets::mint_into(staked_asset_id, beneficiary, minted_shares)?;
      } else {
        let mut position = Positions::<T>::get(asset_id, beneficiary).unwrap_or(StakePosition {
          shares: Zero::zero(),
        });
        let was_zero = position.shares.is_zero();
        position.shares = position
          .shares
          .checked_add(&minted_shares)
          .ok_or(ArithmeticError::Overflow)?;
        if was_zero {
          pool.active_staker_count = pool.active_staker_count.saturating_add(1);
        }
        Positions::<T>::insert(asset_id, beneficiary, position);
      }
      Pools::<T>::insert(asset_id, pool);
      let _ = Self::note_reward_touch(asset_id, beneficiary);
      Ok(minted_shares)
    }

    fn initialize_staked_asset_for_pool(
      asset_id: T::AssetId,
    ) -> Result<(T::AssetId, T::AccountId), DispatchError> {
      let pool_account = Self::pool_account_for(asset_id);
      let staked_asset_id =
        Self::staked_asset_id(asset_id).ok_or(Error::<T>::StakedAssetUnsupported)?;
      ensure!(
        !T::Assets::asset_exists(staked_asset_id),
        Error::<T>::StakedAssetAlreadyInitialized
      );
      T::StakedAssetLifecycle::register(asset_id, staked_asset_id, &pool_account)?;
      Self::index_live_staked_asset(asset_id, staked_asset_id)?;
      Ok((staked_asset_id, pool_account))
    }

    pub fn pool_account_for(asset_id: T::AssetId) -> T::AccountId {
      T::PalletId::get().into_sub_account_truncating(asset_id)
    }

    pub fn reward_account_for(asset_id: T::AssetId) -> T::AccountId {
      let seed = frame::hashing::blake2_256(&(T::PalletId::get(), b"reward", asset_id).encode());
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::new(&seed))
        .expect("hashed reward seed always decodes into AccountId")
    }

    pub fn lp_reward_account_for(asset_id: T::AssetId) -> T::AccountId {
      let seed = frame::hashing::blake2_256(&(T::PalletId::get(), b"lp-reward", asset_id).encode());
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::new(&seed))
        .expect("hashed LP reward seed always decodes into AccountId")
    }

    pub fn native_lp_lock_account() -> T::AccountId {
      let seed = frame::hashing::blake2_256(&(T::PalletId::get(), b"native-lp-lock").encode());
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::new(&seed))
        .expect("hashed native LP lock seed always decodes into AccountId")
    }

    fn ensure_native_governance_unlocked(account: &T::AccountId) -> DispatchResult {
      let Some(lock_until) = T::NativeGovernanceLockProvider::lock_until(account) else {
        return Ok(());
      };
      ensure!(
        frame_system::Pallet::<T>::block_number() >= lock_until,
        Error::<T>::NativeGovernanceLockActive
      );
      Ok(())
    }

    fn is_native_governance_asset(asset_id: T::AssetId) -> bool {
      if asset_id == T::NativeStakingAssetId::get() {
        return true;
      }
      Self::staked_asset_id(T::NativeStakingAssetId::get())
        .is_some_and(|staked_asset_id| staked_asset_id == asset_id)
    }

    fn decrease_total_native_governance_asset_locked(
      asset_id: T::AssetId,
      amount: T::Balance,
    ) -> DispatchResult {
      let current = TotalNativeGovernanceAssetLocked::<T>::get(asset_id);
      let updated = current
        .checked_sub(&amount)
        .ok_or(ArithmeticError::Underflow)?;
      if updated.is_zero() {
        TotalNativeGovernanceAssetLocked::<T>::remove(asset_id);
      } else {
        TotalNativeGovernanceAssetLocked::<T>::insert(asset_id, updated);
      }
      Ok(())
    }

    fn increase_operator_native_lp_locked(
      operator: &T::AccountId,
      amount: T::Balance,
    ) -> DispatchResult {
      let current = OperatorNativeLpLocked::<T>::get(operator);
      let updated = current
        .checked_add(&amount)
        .ok_or(ArithmeticError::Overflow)?;
      OperatorNativeLpLocked::<T>::insert(operator, updated);
      Ok(())
    }

    fn decrease_operator_native_lp_locked(
      operator: &T::AccountId,
      amount: T::Balance,
    ) -> DispatchResult {
      let current = OperatorNativeLpLocked::<T>::get(operator);
      let updated = current
        .checked_sub(&amount)
        .ok_or(ArithmeticError::Underflow)?;
      if updated.is_zero() {
        OperatorNativeLpLocked::<T>::remove(operator);
      } else {
        OperatorNativeLpLocked::<T>::insert(operator, updated);
      }
      Ok(())
    }

    fn decrease_account_native_lp_locked(
      account: &T::AccountId,
      amount: T::Balance,
    ) -> DispatchResult {
      let current = AccountNativeLpLocked::<T>::get(account);
      let updated = current
        .checked_sub(&amount)
        .ok_or(ArithmeticError::Underflow)?;
      if updated.is_zero() {
        AccountNativeLpLocked::<T>::remove(account);
      } else {
        AccountNativeLpLocked::<T>::insert(account, updated);
      }
      Ok(())
    }

    fn decrease_account_native_collator_lp_locked(
      account: &T::AccountId,
      amount: T::Balance,
    ) -> DispatchResult {
      let current = AccountNativeCollatorLpLocked::<T>::get(account);
      let updated = current
        .checked_sub(&amount)
        .ok_or(ArithmeticError::Underflow)?;
      if updated.is_zero() {
        AccountNativeCollatorLpLocked::<T>::remove(account);
      } else {
        AccountNativeCollatorLpLocked::<T>::insert(account, updated);
      }
      Ok(())
    }

    fn decrease_total_native_lp_locked(amount: T::Balance) -> DispatchResult {
      let current = TotalNativeLpLocked::<T>::get();
      let updated = current
        .checked_sub(&amount)
        .ok_or(ArithmeticError::Underflow)?;
      TotalNativeLpLocked::<T>::put(updated);
      Ok(())
    }

    pub fn reward_governance_domain(asset_id: T::AssetId) -> Option<T::GovernanceDomainId> {
      T::RewardGovernanceDomainResolver::reward_governance_domain(asset_id)
    }

    pub fn reward_coefficient(asset_id: T::AssetId, account: &T::AccountId) -> Option<FixedU128> {
      let governance_domain = Self::reward_governance_domain(asset_id)?;
      Some(T::RewardCoefficientProvider::reward_coefficient(
        governance_domain,
        account,
      ))
    }

    pub fn reward_active_weight_snapshot(
      asset_id: T::AssetId,
      account: &T::AccountId,
    ) -> Option<RewardWeightSnapshot<T::RewardEpoch, T::Balance>> {
      RewardActiveWeightSnapshots::<T>::get(asset_id, account)
    }

    pub fn reward_epoch_weight_snapshot(
      asset_id: T::AssetId,
      epoch: T::RewardEpoch,
      account: &T::AccountId,
    ) -> Option<RewardWeightSnapshot<T::RewardEpoch, T::Balance>> {
      RewardEpochWeightSnapshots::<T>::get((asset_id, epoch), account)
    }

    pub fn reward_active_weight(
      asset_id: T::AssetId,
      account: &T::AccountId,
    ) -> Option<T::Balance> {
      Self::reward_active_weight_snapshot(asset_id, account).map(|snapshot| snapshot.weight)
    }

    fn mark_reward_epoch_truncated(epoch: T::RewardEpoch) {
      RewardTruncatedEpochs::<T>::insert(epoch, ());
    }

    pub fn note_reward_ingress_truncated(epoch: T::RewardEpoch, scanned: u32, max_scan: u32) {
      LastRewardIngressTruncatedEpoch::<T>::put(epoch);
      Self::mark_reward_epoch_truncated(epoch);
      Self::deposit_event(Event::RewardIngressTruncated {
        epoch,
        scanned,
        max_scan,
      });
    }

    pub fn reward_claimable(
      asset_id: T::AssetId,
      epoch: T::RewardEpoch,
      account: &T::AccountId,
    ) -> Option<T::Balance> {
      if epoch >= T::RewardEpochProvider::current_reward_epoch() {
        return None;
      }
      if RewardTruncatedEpochs::<T>::contains_key(epoch) {
        return None;
      }
      if RewardClaims::<T>::contains_key((asset_id, epoch), account) {
        return None;
      }
      let epoch_reward = RewardEpochAccruals::<T>::get(asset_id, epoch);
      if epoch_reward.is_zero() {
        return None;
      }
      let total_weight = RewardEpochTotalWeights::<T>::get(asset_id, epoch);
      if total_weight.is_zero() {
        return None;
      }
      let snapshot = RewardEpochWeightSnapshots::<T>::get((asset_id, epoch), account)?;
      if snapshot.weight.is_zero() {
        return None;
      }
      let reward_amount = Self::mul_div_floor(epoch_reward, snapshot.weight, total_weight);
      if reward_amount.is_zero() {
        return None;
      }
      Some(reward_amount)
    }

    pub fn note_reward_touch(asset_id: T::AssetId, account: &T::AccountId) -> bool {
      if !Pools::<T>::contains_key(asset_id) {
        return false;
      }
      let epoch = T::RewardEpochProvider::current_reward_epoch();
      RewardEpochTouchedAccounts::<T>::mutate(epoch, asset_id, |accounts| {
        if accounts.iter().any(|existing| existing == account) {
          return true;
        }
        if accounts.try_push(account.clone()).is_err() {
          Self::mark_reward_epoch_truncated(epoch);
          Self::deposit_event(Event::RewardTouchedAccountsOverflow {
            epoch,
            asset_id,
            account: account.clone(),
          });
          return false;
        }
        true
      })
    }

    pub fn note_reward_inflow(asset_id: T::AssetId, amount: T::Balance) -> DispatchResult {
      ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
      Self::record_reward_inflow(asset_id, amount).map(|_| ())
    }

    pub fn reconcile_reward_inflow(asset_id: T::AssetId) -> Result<T::Balance, DispatchError> {
      ensure!(
        Pools::<T>::contains_key(asset_id),
        Error::<T>::AssetNotRegistered
      );
      let reward_account = Self::reward_account_for(asset_id);
      let actual_balance = T::Assets::balance(asset_id, &reward_account);
      let liability = RewardLiabilityBalances::<T>::get(asset_id);
      if actual_balance <= liability {
        return Ok(Zero::zero());
      }
      let amount = actual_balance
        .checked_sub(&liability)
        .ok_or(Error::<T>::RewardAccountingOverflow)?;
      Self::record_reward_inflow(asset_id, amount)
    }

    fn record_reward_inflow(
      asset_id: T::AssetId,
      amount: T::Balance,
    ) -> Result<T::Balance, DispatchError> {
      ensure!(
        Pools::<T>::contains_key(asset_id),
        Error::<T>::AssetNotRegistered
      );
      let reward_account = Self::reward_account_for(asset_id);
      let actual_balance = T::Assets::balance(asset_id, &reward_account);
      let liability_after = RewardLiabilityBalances::<T>::get(asset_id)
        .checked_add(&amount)
        .ok_or(Error::<T>::RewardAccountingOverflow)?;
      ensure!(
        actual_balance >= liability_after,
        Error::<T>::RewardAccountUnderfunded
      );
      let epoch = T::RewardEpochProvider::current_reward_epoch();
      RewardEpochAccruals::<T>::try_mutate(asset_id, epoch, |accrued| -> DispatchResult {
        *accrued = accrued
          .checked_add(&amount)
          .ok_or(Error::<T>::RewardAccountingOverflow)?;
        Ok(())
      })?;
      if !RewardEpochTotalWeights::<T>::contains_key(asset_id, epoch) {
        RewardEpochTotalWeights::<T>::insert(
          asset_id,
          epoch,
          RewardActiveTotalWeights::<T>::get(asset_id),
        );
      }
      RewardLiabilityBalances::<T>::insert(asset_id, liability_after);
      Self::deposit_event(Event::RewardInflowRecorded {
        asset_id,
        reward_account,
        epoch,
        amount,
      });
      Ok(amount)
    }

    pub fn staked_asset_id(asset_id: T::AssetId) -> Option<T::AssetId> {
      T::StakedAssetIdResolver::staked_asset_id(asset_id)
    }

    pub fn live_base_asset_for_staked_asset(staked_asset_id: T::AssetId) -> Option<T::AssetId> {
      let asset_id = LiveStakedAssetBaseAssets::<T>::get(staked_asset_id)?;
      if Self::live_staked_asset_id(asset_id) == Some(staked_asset_id) {
        return Some(asset_id);
      }
      None
    }

    fn live_staked_asset_id(asset_id: T::AssetId) -> Option<T::AssetId> {
      let staked_asset_id = Self::staked_asset_id(asset_id)?;
      if T::Assets::asset_exists(staked_asset_id) {
        return Some(staked_asset_id);
      }
      None
    }

    fn index_live_staked_asset(
      asset_id: T::AssetId,
      staked_asset_id: T::AssetId,
    ) -> DispatchResult {
      if let Some(existing_asset_id) = LiveStakedAssetBaseAssets::<T>::get(staked_asset_id) {
        ensure!(
          existing_asset_id == asset_id,
          Error::<T>::StakedAssetIdCollision
        );
      }
      LiveStakedAssetBaseAssets::<T>::insert(staked_asset_id, asset_id);
      Ok(())
    }

    fn index_reward_governance_domain_asset(asset_id: T::AssetId) -> DispatchResult {
      let Some(domain_id) = Self::reward_governance_domain(asset_id) else {
        return Ok(());
      };
      RewardAssetsByGovernanceDomain::<T>::try_mutate(domain_id, |assets| {
        if assets.contains(&asset_id) {
          return Ok(());
        }
        assets
          .try_push(asset_id)
          .map_err(|_| Error::<T>::RewardGovernanceDomainAssetSetFull)?;
        Ok(())
      })
    }

    fn legacy_position_shares(asset_id: T::AssetId, account: &T::AccountId) -> T::Balance {
      Positions::<T>::get(asset_id, account)
        .map(|position| position.shares)
        .unwrap_or_else(Zero::zero)
    }

    fn effective_share_balance(asset_id: T::AssetId, account: &T::AccountId) -> Option<T::Balance> {
      let legacy_shares = Self::legacy_position_shares(asset_id, account);
      let receipt_shares = Self::live_staked_asset_id(asset_id)
        .map(|staked_asset_id| T::Assets::balance(staked_asset_id, account))
        .unwrap_or_else(Zero::zero);
      let total_shares = legacy_shares.checked_add(&receipt_shares)?;
      if total_shares.is_zero() {
        return None;
      }
      Some(total_shares)
    }

    pub fn staked_receipt_balance(
      asset_id: T::AssetId,
      account: &T::AccountId,
    ) -> Option<T::Balance> {
      let staked_asset_id = Self::live_staked_asset_id(asset_id)?;
      Some(T::Assets::balance(staked_asset_id, account))
    }

    pub fn live_native_staked_receipt_balance(account: &T::AccountId) -> Option<T::Balance> {
      Self::staked_receipt_balance(T::NativeStakingAssetId::get(), account)
    }

    pub fn staked_receipt_value(
      asset_id: T::AssetId,
      account: &T::AccountId,
    ) -> Option<T::Balance> {
      let pool = Pools::<T>::get(asset_id)?;
      if pool.total_shares.is_zero() {
        return None;
      }
      let staked_receipt_balance = Self::staked_receipt_balance(asset_id, account)?;
      Some(Self::mul_div_floor(
        staked_receipt_balance,
        pool.accounted_balance,
        pool.total_shares,
      ))
    }

    pub fn live_native_staked_receipt_value(account: &T::AccountId) -> Option<T::Balance> {
      Self::staked_receipt_value(T::NativeStakingAssetId::get(), account)
    }

    fn uses_staked_receipts(asset_id: T::AssetId) -> Option<T::AssetId> {
      Self::live_staked_asset_id(asset_id)
    }

    pub fn effective_share_balance_for_queries(
      asset_id: T::AssetId,
      account: &T::AccountId,
    ) -> Option<T::Balance> {
      Self::effective_share_balance(asset_id, account)
    }

    pub fn stake_fraction(
      asset_id: T::AssetId,
      account: &T::AccountId,
    ) -> Option<(T::Balance, T::Balance)> {
      let pool = Pools::<T>::get(asset_id)?;
      let shares = Self::effective_share_balance_for_queries(asset_id, account)?;
      if pool.total_shares.is_zero() {
        return None;
      }
      Some((shares, pool.total_shares))
    }

    pub fn stake_value(asset_id: T::AssetId, account: &T::AccountId) -> Option<T::Balance> {
      let pool = Pools::<T>::get(asset_id)?;
      let shares = Self::effective_share_balance_for_queries(asset_id, account)?;
      if pool.total_shares.is_zero() {
        return None;
      }
      Some(Self::mul_div_floor(
        shares,
        pool.accounted_balance,
        pool.total_shares,
      ))
    }

    pub fn stake_exposure(
      asset_id: T::AssetId,
      account: &T::AccountId,
    ) -> Option<StakeExposure<T::AccountId, T::Balance>> {
      let total_value = Self::stake_value(asset_id, account)?;
      if asset_id != T::NativeStakingAssetId::get() {
        return Some(StakeExposure {
          total_value,
          passive_value: total_value,
          delegated_value: Zero::zero(),
          delegated_operator: None,
        });
      }
      Some(StakeExposure {
        total_value,
        passive_value: total_value,
        delegated_value: Zero::zero(),
        delegated_operator: None,
      })
    }

    pub fn passive_stake_value(asset_id: T::AssetId, account: &T::AccountId) -> Option<T::Balance> {
      let exposure = Self::stake_exposure(asset_id, account)?;
      if exposure.passive_value.is_zero() {
        return None;
      }
      Some(exposure.passive_value)
    }

    pub fn delegated_stake_value(
      asset_id: T::AssetId,
      account: &T::AccountId,
    ) -> Option<(T::AccountId, T::Balance)> {
      let exposure = Self::stake_exposure(asset_id, account)?;
      let operator = exposure.delegated_operator?;
      if exposure.delegated_value.is_zero() {
        return None;
      }
      Some((operator, exposure.delegated_value))
    }

    pub fn native_stake_value(account: &T::AccountId) -> Option<T::Balance> {
      Self::stake_value(T::NativeStakingAssetId::get(), account)
    }

    pub fn passive_native_stake_value(account: &T::AccountId) -> Option<T::Balance> {
      Self::passive_stake_value(T::NativeStakingAssetId::get(), account)
    }

    pub fn delegated_native_stake_value(
      delegator: &T::AccountId,
    ) -> Option<(T::AccountId, T::Balance)> {
      Self::delegated_stake_value(T::NativeStakingAssetId::get(), delegator)
    }

    fn roll_reward_epoch_if_needed() -> Weight {
      const REWARD_ASSET_ROLLOVER_WEIGHT_REF_TIME: u64 = 5_000;
      const REWARD_ACCOUNT_ROLLOVER_WEIGHT_REF_TIME: u64 = 20_000;
      const REWARD_RECONCILIATION_WEIGHT_REF_TIME: u64 = 10_000;
      let current_epoch = T::RewardEpochProvider::current_reward_epoch();
      let Some(last_epoch) = LastProcessedRewardEpoch::<T>::get() else {
        LastProcessedRewardEpoch::<T>::put(current_epoch);
        return Weight::zero();
      };
      let Some(rollover) = PendingRewardEpochRollover::<T>::get().or_else(|| {
        (current_epoch != last_epoch).then_some(RewardEpochRolloverState {
          from_epoch: last_epoch,
          to_epoch: current_epoch,
        })
      }) else {
        return Weight::zero();
      };
      let max_assets = (T::MaxRewardRolloverAssetsPerBlock::get() as usize).max(1);
      let mut processed_assets = 0u64;
      let mut processed_accounts = 0u64;
      while (processed_assets as usize) < max_assets {
        let Some((asset_id, accounts)) =
          RewardEpochTouchedAccounts::<T>::iter_prefix(rollover.from_epoch).next()
        else {
          break;
        };
        processed_assets = processed_assets.saturating_add(1);
        RewardEpochTouchedAccounts::<T>::remove(rollover.from_epoch, asset_id);
        for account in accounts {
          processed_accounts = processed_accounts.saturating_add(1);
          Self::roll_reward_account_snapshot(asset_id, rollover.to_epoch, &account);
        }
      }
      let rollover_complete = RewardEpochTouchedAccounts::<T>::iter_prefix(rollover.from_epoch)
        .next()
        .is_none();
      let reconciled_assets = if rollover_complete {
        PendingRewardEpochRollover::<T>::kill();
        LastProcessedRewardEpoch::<T>::put(rollover.to_epoch);
        if Pools::<T>::contains_key(T::NativeStakingAssetId::get()) {
          let _ = Self::reconcile_reward_inflow(T::NativeStakingAssetId::get());
          1u64
        } else {
          0u64
        }
      } else {
        PendingRewardEpochRollover::<T>::put(rollover);
        0u64
      };
      Weight::from_parts(
        processed_assets
          .saturating_mul(REWARD_ASSET_ROLLOVER_WEIGHT_REF_TIME)
          .saturating_add(
            processed_accounts.saturating_mul(REWARD_ACCOUNT_ROLLOVER_WEIGHT_REF_TIME),
          )
          .saturating_add(reconciled_assets.saturating_mul(REWARD_RECONCILIATION_WEIGHT_REF_TIME)),
        0,
      )
    }

    fn roll_reward_account_snapshot(
      asset_id: T::AssetId,
      epoch: T::RewardEpoch,
      account: &T::AccountId,
    ) -> RewardWeightSnapshot<T::RewardEpoch, T::Balance> {
      let old_weight = RewardActiveWeightSnapshots::<T>::get(asset_id, account)
        .map(|snapshot| snapshot.weight)
        .unwrap_or_else(Zero::zero);
      let snapshot = Self::reward_snapshot_from_current_state(asset_id, epoch, account);
      RewardEpochWeightSnapshots::<T>::insert((asset_id, epoch), account, &snapshot);
      if snapshot.shares.is_zero() {
        RewardActiveWeightSnapshots::<T>::remove(asset_id, account);
      } else {
        RewardActiveWeightSnapshots::<T>::insert(asset_id, account, &snapshot);
      }
      let total_weight = RewardActiveTotalWeights::<T>::get(asset_id);
      let updated_total_weight = if snapshot.weight >= old_weight {
        total_weight.saturating_add(snapshot.weight.saturating_sub(old_weight))
      } else {
        total_weight.saturating_sub(old_weight.saturating_sub(snapshot.weight))
      };
      RewardActiveTotalWeights::<T>::insert(asset_id, updated_total_weight);
      snapshot
    }

    fn reward_snapshot_from_current_state(
      asset_id: T::AssetId,
      epoch: T::RewardEpoch,
      account: &T::AccountId,
    ) -> RewardWeightSnapshot<T::RewardEpoch, T::Balance> {
      let shares = T::RewardBaseWeightProvider::reward_base_weight(asset_id, account)
        .unwrap_or_else(|| {
          Self::effective_share_balance(asset_id, account).unwrap_or_else(Zero::zero)
        });
      let coefficient =
        Self::reward_coefficient(asset_id, account).unwrap_or_else(|| FixedU128::from_inner(0));
      let weight = Self::reward_weight_from_snapshot(shares, coefficient);
      RewardWeightSnapshot {
        effective_from_epoch: epoch,
        shares,
        coefficient,
        weight,
      }
    }

    fn reward_weight_from_snapshot(shares: T::Balance, coefficient: FixedU128) -> T::Balance {
      let shares_u128: u128 = shares.saturated_into();
      coefficient.saturating_mul_int(shares_u128).saturated_into()
    }

    fn sync_pool_state(asset_id: T::AssetId) -> Result<PoolState<T::Balance>, DispatchError> {
      let mut pool = Pools::<T>::get(asset_id).ok_or(Error::<T>::AssetNotRegistered)?;
      let actual_balance = T::Assets::balance(asset_id, &Self::pool_account_for(asset_id));
      ensure!(
        actual_balance >= pool.accounted_balance,
        Error::<T>::PoolOutflowDetected
      );
      let inflow = actual_balance
        .checked_sub(&pool.accounted_balance)
        .ok_or(ArithmeticError::Underflow)?;
      if !inflow.is_zero() {
        pool.accounted_balance = actual_balance;
        Pools::<T>::insert(asset_id, &pool);
        Self::deposit_event(Event::PoolSynced {
          asset_id,
          actual_balance,
          inflow,
        });
      }
      Ok(pool)
    }

    fn mul_div_floor(a: T::Balance, b: T::Balance, c: T::Balance) -> T::Balance {
      let a_u128: u128 = a.saturated_into();
      let b_u128: u128 = b.saturated_into();
      let c_u128: u128 = c.saturated_into();
      let result = (U256::from(a_u128) * U256::from(b_u128)) / U256::from(c_u128);
      result.low_u128().saturated_into()
    }
  }

  #[pallet::view_functions]
  impl<T: Config> Pallet<T> {
    pub fn native_staking_exchange_rate() -> Option<FixedU128> {
      let pool = Pools::<T>::get(T::NativeStakingAssetId::get())?;
      if pool.total_shares.is_zero() || pool.accounted_balance.is_zero() {
        return None;
      }
      Some(FixedU128::from_rational(
        pool.accounted_balance.saturated_into::<u128>(),
        pool.total_shares.saturated_into::<u128>(),
      ))
    }

    pub fn native_staking_liquidity_pool()
    -> Option<NativeStakingLiquidityPool<T::AssetId, T::Balance>> {
      let native_asset_id = T::NativeStakingAssetId::get();
      let staked_asset_id = Self::staked_asset_id(native_asset_id)?;
      let (lp_asset_id, reserve_native, reserve_staked, lp_total_issuance) =
        T::NativeStakingReadModelProvider::native_staking_liquidity_pool()?;
      Some(NativeStakingLiquidityPool {
        native_asset_id,
        staked_asset_id,
        lp_asset_id,
        reserve_native,
        reserve_staked,
        lp_total_issuance,
      })
    }

    pub fn native_locked_lp_position(account: T::AccountId) -> NativeLockedLpPosition<T::Balance> {
      let total_locked_lp = AccountNativeLpLocked::<T>::get(&account);
      let collator_locked_lp = AccountNativeCollatorLpLocked::<T>::get(&account);
      let governance_locked_lp = NativeGovernanceLpLocks::<T>::get(&account)
        .map(|lock| lock.amount)
        .unwrap_or_else(Zero::zero);
      let conservative_native_value =
        T::NativeStakingReadModelProvider::native_lp_value(total_locked_lp);
      NativeLockedLpPosition {
        total_locked_lp,
        collator_locked_lp,
        governance_locked_lp,
        conservative_native_value,
      }
    }

    pub fn native_collator_lp_position(
      account: T::AccountId,
      operator: T::AccountId,
    ) -> NativeCollatorLpPosition<T::AssetId, T::Balance, BlockNumberFor<T>> {
      let lock = NativeLpLocks::<T>::get(&account, &operator);
      let pending = PendingNativeLpUnlocks::<T>::get(&account, &operator);
      let locked_lp = lock
        .as_ref()
        .map(|item| item.amount)
        .unwrap_or_else(Zero::zero);
      let pending_unlock_lp = pending
        .as_ref()
        .map(|item| item.amount)
        .unwrap_or_else(Zero::zero);
      let pending_unlock_block = pending.as_ref().map(|item| item.unlock_block);
      let lp_asset_id = lock
        .as_ref()
        .map(|item| item.lp_asset_id)
        .or_else(|| pending.as_ref().map(|item| item.lp_asset_id));
      let conservative_native_value = T::NativeStakingReadModelProvider::native_lp_value(locked_lp);
      NativeCollatorLpPosition {
        lp_asset_id,
        locked_lp,
        pending_unlock_lp,
        pending_unlock_block,
        conservative_native_value,
      }
    }

    pub fn native_governance_custody_position(
      account: T::AccountId,
      asset_id: T::AssetId,
    ) -> NativeGovernanceCustodyPosition<T::AssetId, T::Balance, BlockNumberFor<T>> {
      let lp_lock = NativeGovernanceLpLocks::<T>::get(&account);
      let pending_lp = PendingNativeGovernanceLpUnlocks::<T>::get(&account);
      let pending_asset = PendingNativeGovernanceAssetUnlocks::<T>::get(&account, asset_id);
      let governance_locked_lp = lp_lock
        .as_ref()
        .map(|item| item.amount)
        .unwrap_or_else(Zero::zero);
      let pending_governance_lp_unlock = pending_lp
        .as_ref()
        .map(|item| item.amount)
        .unwrap_or_else(Zero::zero);
      let pending_governance_lp_unlock_block = pending_lp.as_ref().map(|item| item.unlock_block);
      let lp_asset_id = lp_lock
        .as_ref()
        .map(|item| item.lp_asset_id)
        .or_else(|| pending_lp.as_ref().map(|item| item.lp_asset_id));
      let pending_asset_unlock = pending_asset
        .as_ref()
        .map(|item| item.amount)
        .unwrap_or_else(Zero::zero);
      let pending_asset_unlock_block = pending_asset.as_ref().map(|item| item.unlock_block);
      NativeGovernanceCustodyPosition {
        lp_asset_id,
        governance_locked_lp,
        pending_governance_lp_unlock,
        pending_governance_lp_unlock_block,
        asset_id,
        asset_locked: NativeGovernanceAssetLocked::<T>::get(&account, asset_id),
        pending_asset_unlock,
        pending_asset_unlock_block,
      }
    }

    pub fn native_nomination_reward_claimable(
      epoch: T::RewardEpoch,
      account: T::AccountId,
    ) -> Option<T::Balance> {
      let asset_id = T::NativeStakingAssetId::get();
      let current_epoch = T::RewardEpochProvider::current_reward_epoch();
      if Self::ensure_reward_epoch_claimable(&account, asset_id, epoch, current_epoch).is_err() {
        return None;
      }
      Self::reward_claimable(asset_id, epoch, &account)
    }
  }
}
