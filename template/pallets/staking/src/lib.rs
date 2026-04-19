#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use frame::traits::StorageVersion;
use polkadot_sdk::{
  frame_support::weights::Weight,
  sp_runtime::{DispatchResult, FixedU128},
};

pub use pallet::*;

pub mod weights;
pub use weights::WeightInfo;

pub trait NativeBindingTargetValidator<AccountId> {
  fn is_valid_operator(_account: &AccountId) -> bool {
    true
  }

  #[cfg(feature = "runtime-benchmarks")]
  fn benchmark_prepare_valid_operator(_account: &AccountId) {}
}

impl<AccountId> NativeBindingTargetValidator<AccountId> for () {}

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

pub trait RewardSnapshotEventIngress<Epoch> {
  fn ingest(_epoch: Epoch, _max_scan: usize) -> Weight {
    Weight::zero()
  }
}

impl<Epoch> RewardSnapshotEventIngress<Epoch> for () {}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

#[frame::pallet]
pub mod pallet {
  use crate::{
    NativeBindingTargetValidator as _, RewardCoefficientProvider as _, RewardEpochProvider as _,
    RewardGovernanceDomainResolver as _, RewardSnapshotEventIngress as _,
    StakedAssetIdResolver as _, StakedAssetLifecycle as _, weights::WeightInfo as _,
  };
  use alloc::{collections::BTreeSet, vec::Vec};
  use codec::{Decode, Encode};
  use frame::prelude::*;
  use polkadot_sdk::frame_support::traits::fungibles::{Inspect, Mutate};
  use polkadot_sdk::frame_support::traits::tokens::{Fortitude, Precision, Preservation};
  use polkadot_sdk::frame_support::{PalletId, weights::Weight};
  use polkadot_sdk::sp_core::U256;
  use polkadot_sdk::sp_runtime::{
    FixedU128, Perbill,
    traits::{
      AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, CheckedSub, SaturatedConversion, Zero,
    },
  };

  #[pallet::config]
  pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
    type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    type AssetId: Parameter + MaxEncodedLen + Member + Copy + Ord + TypeInfo;
    type NativeStakingAssetId: Get<Self::AssetId>;
    type GovernanceDomainId: Parameter + MaxEncodedLen + Member + Copy + Ord + TypeInfo;
    type RewardEpoch: Parameter + MaxEncodedLen + Member + Copy + Ord + TypeInfo;
    type NativeBindingTargetValidator: crate::NativeBindingTargetValidator<Self::AccountId>;
    type StakedAssetIdResolver: crate::StakedAssetIdResolver<Self::AssetId>;
    type StakedAssetLifecycle: crate::StakedAssetLifecycle<Self::AccountId, Self::AssetId>;
    type RewardGovernanceDomainResolver: crate::RewardGovernanceDomainResolver<Self::AssetId, Self::GovernanceDomainId>;
    type RewardEpochProvider: crate::RewardEpochProvider<Self::RewardEpoch>;
    type RewardCoefficientProvider: crate::RewardCoefficientProvider<Self::AccountId, Self::GovernanceDomainId>;
    type RewardSnapshotEventIngress: crate::RewardSnapshotEventIngress<Self::RewardEpoch>;
    #[pallet::constant]
    type MaxOperatorCommission: Get<Perbill>;
    #[pallet::constant]
    type MaxRewardEventScanPerBlock: Get<u32>;
    #[pallet::constant]
    type MaxRewardAccountsPerAssetEpoch: Get<u32>;
    #[pallet::constant]
    type MaxClaimEpochsPerCall: Get<u32>;
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

    fn on_idle(_n: BlockNumberFor<T>, _remaining_weight: Weight) -> Weight {
      let epoch = T::RewardEpochProvider::current_reward_epoch();
      let max_scan = T::MaxRewardEventScanPerBlock::get() as usize;
      T::RewardSnapshotEventIngress::ingest(epoch, max_scan)
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
  pub struct RewardWeightSnapshot<Epoch, Balance> {
    pub effective_from_epoch: Epoch,
    pub shares: Balance,
    pub coefficient: FixedU128,
    pub weight: Balance,
  }

  #[pallet::storage]
  #[pallet::getter(fn pool)]
  pub type Pools<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AssetId, PoolState<T::Balance>, OptionQuery>;

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
  #[pallet::getter(fn native_binding)]
  pub type NativeBindings<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn operator_commission)]
  pub type OperatorCommissions<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AccountId, Perbill, ValueQuery>;

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
    RewardIngressTruncated {
      epoch: T::RewardEpoch,
      scanned: u32,
      max_scan: u32,
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
    NativeBindingSet {
      account: T::AccountId,
      operator: T::AccountId,
    },
    NativeBindingCleared {
      account: T::AccountId,
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
    NativeStakeRequiresOperator,
    CannotBindToSelf,
    InvalidBindingTarget,
    CommissionExceedsMaximum,
    RewardAccountUnderfunded,
    RewardAccountingOverflow,
    RewardEpochWeightFrozen,
    RewardEpochStillOpen,
    RewardEpochIncomplete,
    RewardAlreadyClaimed,
    DuplicateRewardEpoch,
    NoRewardClaimable,
  }

  #[pallet::call]
  impl<T: Config> Pallet<T> {
    #[pallet::call_index(0)]
    #[pallet::weight(T::WeightInfo::register_staking_asset())]
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
        Error::<T>::NativeStakeRequiresOperator
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

    #[pallet::call_index(5)]
    #[pallet::weight(T::WeightInfo::bind_native())]
    pub fn bind_native(origin: OriginFor<T>, operator: T::AccountId) -> DispatchResult {
      let account = ensure_signed(origin)?;
      ensure!(account != operator, Error::<T>::CannotBindToSelf);
      ensure!(
        T::NativeBindingTargetValidator::is_valid_operator(&operator),
        Error::<T>::InvalidBindingTarget
      );
      NativeBindings::<T>::insert(&account, &operator);
      Self::deposit_event(Event::NativeBindingSet { account, operator });
      Ok(())
    }

    #[pallet::call_index(6)]
    #[pallet::weight(T::WeightInfo::clear_native_binding())]
    pub fn clear_native_binding(origin: OriginFor<T>) -> DispatchResult {
      let account = ensure_signed(origin)?;
      NativeBindings::<T>::remove(&account);
      Self::deposit_event(Event::NativeBindingCleared { account });
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
    #[pallet::weight(T::WeightInfo::stake().saturating_add(T::WeightInfo::bind_native()))]
    pub fn stake_native(
      origin: OriginFor<T>,
      amount: T::Balance,
      operator: T::AccountId,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      ensure!(account != operator, Error::<T>::CannotBindToSelf);
      ensure!(
        T::NativeBindingTargetValidator::is_valid_operator(&operator),
        Error::<T>::InvalidBindingTarget
      );
      let asset_id = T::NativeStakingAssetId::get();
      let minted_shares = Self::do_stake(asset_id, &account, amount)?;
      NativeBindings::<T>::insert(&account, &operator);
      Self::deposit_event(Event::Staked {
        asset_id,
        account: account.clone(),
        amount_in: amount,
        minted_shares,
      });
      Self::deposit_event(Event::NativeBindingSet { account, operator });
      Ok(())
    }

    #[pallet::call_index(9)]
    #[pallet::weight(T::WeightInfo::initialize_staked_asset())]
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
      Pools::<T>::get(asset_id).ok_or(Error::<T>::AssetNotRegistered)?;
      let current_epoch = T::RewardEpochProvider::current_reward_epoch();
      Self::ensure_reward_epoch_claimable(&account, asset_id, epoch, current_epoch)?;
      let reward_amount = Self::do_claim_reward(&account, asset_id, epoch)?;
      ensure!(!reward_amount.is_zero(), Error::<T>::NoRewardClaimable);
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

    pub fn note_reward_ingress_truncated(epoch: T::RewardEpoch, scanned: u32, max_scan: u32) {
      LastRewardIngressTruncatedEpoch::<T>::put(epoch);
      RewardTruncatedEpochs::<T>::insert(epoch, ());
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
        accounts.try_push(account.clone()).is_ok()
      })
    }

    pub fn note_reward_inflow(asset_id: T::AssetId, amount: T::Balance) -> DispatchResult {
      ensure!(
        Pools::<T>::contains_key(asset_id),
        Error::<T>::AssetNotRegistered
      );
      ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
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
      Ok(())
    }

    pub fn staked_asset_id(asset_id: T::AssetId) -> Option<T::AssetId> {
      T::StakedAssetIdResolver::staked_asset_id(asset_id)
    }

    fn live_staked_asset_id(asset_id: T::AssetId) -> Option<T::AssetId> {
      let staked_asset_id = Self::staked_asset_id(asset_id)?;
      if T::Assets::asset_exists(staked_asset_id) {
        return Some(staked_asset_id);
      }
      None
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
      let Some(operator) = NativeBindings::<T>::get(account) else {
        return Some(StakeExposure {
          total_value,
          passive_value: total_value,
          delegated_value: Zero::zero(),
          delegated_operator: None,
        });
      };
      Some(StakeExposure {
        total_value,
        passive_value: Zero::zero(),
        delegated_value: total_value,
        delegated_operator: Some(operator),
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

    // O(B) where B = total NativeBindings entries; acceptable at current
    // scale but should migrate to a cached per-operator counter if B grows
    // into thousands.
    pub fn delegated_native_backing(operator: &T::AccountId) -> T::Balance {
      let asset_id = T::NativeStakingAssetId::get();
      NativeBindings::<T>::iter()
        .filter_map(|(account, bound_operator)| {
          if &bound_operator == operator {
            Self::stake_value(asset_id, &account)
          } else {
            None
          }
        })
        .fold(Zero::zero(), |total, value| total.saturating_add(value))
    }

    fn roll_reward_epoch_if_needed() -> Weight {
      const REWARD_ASSET_ROLLOVER_WEIGHT_REF_TIME: u64 = 5_000;
      const REWARD_ACCOUNT_ROLLOVER_WEIGHT_REF_TIME: u64 = 20_000;
      let current_epoch = T::RewardEpochProvider::current_reward_epoch();
      let Some(last_epoch) = LastProcessedRewardEpoch::<T>::get() else {
        LastProcessedRewardEpoch::<T>::put(current_epoch);
        return Weight::zero();
      };
      if current_epoch == last_epoch {
        return Weight::zero();
      }
      let touched_assets: Vec<_> =
        RewardEpochTouchedAccounts::<T>::iter_prefix(last_epoch).collect();
      let mut processed_assets = 0u64;
      let mut processed_accounts = 0u64;
      for (asset_id, accounts) in touched_assets {
        processed_assets = processed_assets.saturating_add(1);
        RewardEpochTouchedAccounts::<T>::remove(last_epoch, asset_id);
        for account in accounts {
          processed_accounts = processed_accounts.saturating_add(1);
          Self::roll_reward_account_snapshot(asset_id, current_epoch, &account);
        }
      }
      LastProcessedRewardEpoch::<T>::put(current_epoch);
      Weight::from_parts(
        processed_assets
          .saturating_mul(REWARD_ASSET_ROLLOVER_WEIGHT_REF_TIME)
          .saturating_add(
            processed_accounts.saturating_mul(REWARD_ACCOUNT_ROLLOVER_WEIGHT_REF_TIME),
          ),
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
      let shares = Self::effective_share_balance(asset_id, account).unwrap_or_else(Zero::zero);
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
}
