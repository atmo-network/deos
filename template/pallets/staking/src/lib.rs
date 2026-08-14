#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame::{
  prelude::BoundedVec,
  traits::{Get, StorageVersion},
};
use polkadot_sdk::sp_runtime::{DispatchError, DispatchResult, FixedU128};
use scale_info::TypeInfo;

pub use pallet::*;

pub mod weights;
pub use weights::WeightInfo;

pub trait NativeOperatorValidator<AccountId> {
  fn is_valid_operator(_account: &AccountId) -> bool {
    true
  }

  #[cfg(feature = "runtime-benchmarks")]
  fn benchmark_prepare_valid_operator(_account: &AccountId) {}

  #[cfg(feature = "runtime-benchmarks")]
  fn benchmark_prepare_snapshot_operator(account: &AccountId) {
    Self::benchmark_prepare_valid_operator(account);
  }
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

#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum NativeSecurityMode {
  TrustedSet,
  LpBackedSelection,
}

#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub struct NativeSecurityCapabilities {
  pub new_nominations: bool,
  pub redelegation: bool,
  pub candidate_selection: bool,
  pub reward_funding: bool,
  pub reward_claims: bool,
  pub reward_compound: bool,
  pub custody_exit: bool,
}

pub type SecurityEpoch = polkadot_sdk::sp_staking::SessionIndex;

#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum NativeSecurityReadiness {
  Inactive,
  NativePoolMissing,
  StakedAssetMissing,
  LiquidityPoolMissing,
  CanonicalLpMismatch,
  EmptyNativeReserve,
  EmptyStakedReserve,
  EmptyLpIssuance,
  ValuationUnavailable,
  ParticipantIndexInconsistent,
  SnapshotOpenFailed,
  EligibleOperatorSetEmpty,
  CandidateSetInconsistent,
  Ready,
}

#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub struct NativeSecurityBoundaryDiagnostic {
  pub planned_epoch: SecurityEpoch,
  pub readiness: NativeSecurityReadiness,
}

#[derive(
  Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub struct NativeSecurityAccountSnapshot<AccountId, Balance> {
  pub account: AccountId,
  pub conservative_native_value: Balance,
  pub governance_coefficient: FixedU128,
  pub reward_weight: Balance,
}

#[derive(
  Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub struct NativeSecurityOperatorSnapshot<AccountId, Balance> {
  pub operator: AccountId,
  pub conservative_native_backing: Balance,
}

#[derive(Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo)]
#[scale_info(skip_type_params(MaxParticipants, MaxOperators))]
pub struct NativeSecurityEpochSnapshot<AccountId, Balance, MaxParticipants, MaxOperators>
where
  MaxParticipants: Get<u32>,
  MaxOperators: Get<u32>,
{
  pub epoch: SecurityEpoch,
  pub participants: BoundedVec<NativeSecurityAccountSnapshot<AccountId, Balance>, MaxParticipants>,
  pub eligible_operators:
    BoundedVec<NativeSecurityOperatorSnapshot<AccountId, Balance>, MaxOperators>,
  pub total_reward_weight: Balance,
}

#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum NativeSecurityRewardPotStatus {
  Planned,
  Open,
  Finalized,
  Expired,
}

#[derive(
  Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub struct NativeSecurityRewardPot<Balance> {
  pub total_reward_weight: Balance,
  pub credited: Balance,
  pub claimed: Balance,
  pub status: NativeSecurityRewardPotStatus,
}

pub trait NativeSecurityModeProvider {
  fn mode() -> NativeSecurityMode {
    NativeSecurityMode::TrustedSet
  }

  #[cfg(feature = "runtime-benchmarks")]
  fn benchmark_prepare_lp_backed_selection() {}
}

impl NativeSecurityModeProvider for () {}

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

pub trait SecurityEpochProvider {
  /// Canonical native-security identity. Implementations MUST derive this from
  /// the host session owner, never block cadence or maintenance progress.
  fn current_security_epoch() -> SecurityEpoch {
    Default::default()
  }
}

impl SecurityEpochProvider for () {}

pub trait GovernanceParticipationCoefficientProvider<AccountId, GovernanceDomainId> {
  fn governance_participation_coefficient(
    _domain: GovernanceDomainId,
    _account: &AccountId,
  ) -> FixedU128 {
    FixedU128::from_inner(0)
  }

  #[cfg(feature = "runtime-benchmarks")]
  fn benchmark_prepare_positive_coefficient(_domain: GovernanceDomainId, _account: &AccountId) {}
}

impl<AccountId, GovernanceDomainId>
  GovernanceParticipationCoefficientProvider<AccountId, GovernanceDomainId> for ()
{
}

pub trait NativeStakingReadModelProvider<AssetId, Balance> {
  fn native_staking_liquidity_pool() -> Option<(AssetId, Balance, Balance, Balance)> {
    None
  }

  fn native_lp_value(_locked_lp: Balance) -> Option<Balance> {
    None
  }

  fn native_security_topology_readiness() -> Option<NativeSecurityReadiness> {
    Some(NativeSecurityReadiness::Ready)
  }
}

impl<AssetId, Balance> NativeStakingReadModelProvider<AssetId, Balance> for () {}

pub trait NativeSecurityRewardCompound<AccountId, AssetId, Balance> {
  fn compound(
    _account: &AccountId,
    _reward: Balance,
    _min_lp_out: Balance,
  ) -> Result<(AssetId, Balance), DispatchError> {
    Err(DispatchError::Other(
      "NativeSecurityRewardCompoundUnavailable",
    ))
  }
}

impl<AccountId, AssetId, Balance> NativeSecurityRewardCompound<AccountId, AssetId, Balance> for () {}

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
  fn set_security_epoch(epoch: SecurityEpoch);
  fn fund_native_account(account: &AccountId, amount: Balance);
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

  fn set_security_epoch(_epoch: SecurityEpoch) {}

  fn fund_native_account(_account: &AccountId, _amount: Balance) {}
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

#[frame::pallet]
pub mod pallet {
  use crate::{
    GovernanceParticipationCoefficientProvider as _, NativeGovernanceLockProvider as _,
    NativeLpAssetNamespaceInitializer as _, NativeOperatorValidator as _,
    NativeSecurityAccountSnapshot, NativeSecurityBoundaryDiagnostic, NativeSecurityCapabilities,
    NativeSecurityEpochSnapshot, NativeSecurityMode, NativeSecurityModeProvider as _,
    NativeSecurityOperatorSnapshot, NativeSecurityReadiness, NativeSecurityRewardCompound as _,
    NativeSecurityRewardPot, NativeSecurityRewardPotStatus, NativeStakingLpAssetValidator as _,
    NativeStakingReadModelProvider as _, SecurityEpoch, SecurityEpochProvider as _,
    StakedAssetIdResolver as _, StakedAssetLifecycle as _, weights::WeightInfo as _,
  };
  use alloc::vec::Vec;
  use codec::{Decode, Encode};
  use frame::prelude::*;
  use polkadot_sdk::frame_support::traits::tokens::{Fortitude, Precision, Preservation};
  use polkadot_sdk::frame_support::traits::{
    Currency,
    fungibles::{Inspect, Mutate},
  };
  use polkadot_sdk::frame_support::{PalletId, transactional};
  use polkadot_sdk::sp_core::U256;
  use polkadot_sdk::sp_runtime::{
    ArithmeticError, FixedU128,
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
    type NativeCurrency: Currency<Self::AccountId, Balance = Self::Balance>;
    type SecurityRewardFundingOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    type SecurityRewardFundingSource: Get<Self::AccountId>;
    type GovernanceDomainId: Parameter + MaxEncodedLen + Member + Copy + Ord + TypeInfo;
    type NativeGovernanceDomainId: Get<Self::GovernanceDomainId>;
    type NativeOperatorValidator: crate::NativeOperatorValidator<Self::AccountId>;
    type NativeStakingLpAssetValidator: crate::NativeStakingLpAssetValidator<Self::AssetId>;
    type NativeLpAssetNamespaceInitializer: crate::NativeLpAssetNamespaceInitializer;
    type NativeGovernanceLockProvider: crate::NativeGovernanceLockProvider<Self::AccountId, BlockNumberFor<Self>>;
    type NativeSecurityModeProvider: crate::NativeSecurityModeProvider;
    type StakedAssetIdResolver: crate::StakedAssetIdResolver<Self::AssetId>;
    type StakedAssetLifecycle: crate::StakedAssetLifecycle<Self::AccountId, Self::AssetId>;
    type SecurityEpochProvider: crate::SecurityEpochProvider;
    type GovernanceParticipationCoefficientProvider: crate::GovernanceParticipationCoefficientProvider<Self::AccountId, Self::GovernanceDomainId>;
    type NativeStakingReadModelProvider: crate::NativeStakingReadModelProvider<Self::AssetId, Self::Balance>;
    type NativeSecurityRewardCompound: crate::NativeSecurityRewardCompound<Self::AccountId, Self::AssetId, Self::Balance>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper: crate::BenchmarkHelper<Self::AccountId, Self::AssetId, Self::Balance>;
    #[pallet::constant]
    type MaxNativeSecurityParticipants: Get<u32>;
    #[pallet::constant]
    type MaxNativeSecurityOperators: Get<u32>;
    #[pallet::constant]
    type MaxNominationsPerAccount: Get<u32>;
    #[pallet::constant]
    type NativeLpUnlockDelay: Get<BlockNumberFor<Self>>;
    #[pallet::constant]
    type SecurityRewardClaimHorizon: Get<SecurityEpoch>;
    #[pallet::constant]
    type MaxSecurityRewardClaimsPerCall: Get<u32>;
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
    #[cfg(feature = "try-runtime")]
    fn try_state(_n: BlockNumberFor<T>) -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      Self::do_try_state()
    }
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct PoolState<Balance> {
    pub total_shares: Balance,
    pub accounted_balance: Balance,
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

  #[pallet::storage]
  #[pallet::getter(fn pool)]
  pub type Pools<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AssetId, PoolState<T::Balance>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn base_asset_for_staked_asset)]
  pub type LiveStakedAssetBaseAssets<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AssetId, T::AssetId, OptionQuery>;

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
  #[pallet::getter(fn last_native_security_boundary_diagnostic)]
  pub type LastNativeSecurityBoundaryDiagnostic<T: Config> =
    StorageValue<_, NativeSecurityBoundaryDiagnostic, OptionQuery>;

  pub type NativeSecurityEpochSnapshotOf<T> = NativeSecurityEpochSnapshot<
    <T as frame_system::Config>::AccountId,
    <T as Config>::Balance,
    <T as Config>::MaxNativeSecurityParticipants,
    <T as Config>::MaxNativeSecurityOperators,
  >;

  #[pallet::storage]
  #[pallet::getter(fn active_native_security_epoch_snapshot)]
  pub type ActiveNativeSecurityEpochSnapshot<T: Config> =
    StorageValue<_, NativeSecurityEpochSnapshotOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn native_security_epoch_snapshot)]
  pub type NativeSecurityEpochSnapshots<T: Config> =
    StorageMap<_, Blake2_128Concat, SecurityEpoch, NativeSecurityEpochSnapshotOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn native_security_reward_pot)]
  pub type NativeSecurityRewardPots<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    SecurityEpoch,
    NativeSecurityRewardPot<T::Balance>,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn native_security_reward_liability)]
  pub type NativeSecurityRewardLiability<T: Config> = StorageValue<_, T::Balance, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn native_security_reward_claimed)]
  pub type NativeSecurityRewardClaims<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    SecurityEpoch,
    Blake2_128Concat,
    T::AccountId,
    (),
    OptionQuery,
  >;

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
  #[pallet::getter(fn native_security_participants)]
  pub type NativeSecurityParticipants<T: Config> =
    StorageValue<_, BoundedVec<T::AccountId, T::MaxNativeSecurityParticipants>, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn native_nomination_operators)]
  pub type NativeNominationOperators<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    BoundedVec<T::AccountId, T::MaxNominationsPerAccount>,
    ValueQuery,
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
          },
        );
        Pallet::<T>::create_staked_asset_for_pool(*asset_id)
          .expect("genesis staked asset creation must succeed");
      }
    }
  }

  #[pallet::event]
  #[pallet::generate_deposit(pub(super) fn deposit_event)]
  pub enum Event<T: Config> {
    StakingAssetRegistered {
      asset_id: T::AssetId,
      pool_account: T::AccountId,
    },
    PoolSynced {
      asset_id: T::AssetId,
      actual_balance: T::Balance,
      inflow: T::Balance,
    },
    Staked {
      asset_id: T::AssetId,
      account: T::AccountId,
      amount_in: T::Balance,
      minted_shares: T::Balance,
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
    NativeSecurityRewardFunded {
      epoch: SecurityEpoch,
      source: T::AccountId,
      amount: T::Balance,
      epoch_credited: T::Balance,
      outstanding_liability: T::Balance,
    },
    NativeSecurityRewardClaimed {
      epoch: SecurityEpoch,
      account: T::AccountId,
      amount: T::Balance,
      outstanding_liability: T::Balance,
    },
    NativeSecurityRewardExpired {
      epoch: SecurityEpoch,
      returned: T::Balance,
      uncredited_excess: T::Balance,
      outstanding_liability: T::Balance,
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
    NativeStakeRequiresDedicatedCall,
    CannotNominateSelf,
    InvalidNativeOperatorTarget,
    NativeGovernanceLockActive,
    InvalidNativeGovernanceAsset,
    InvalidNativeLpAsset,
    NativeLpAssetMismatch,
    NativeSecurityParticipantLimitReached,
    NativeSecurityOperatorLimitReached,
    NativeSecurityValuationUnavailable,
    NativeNominationLimitReached,
    NativeSecurityParticipantIndexCorrupt,
    NativeNominationIndexCorrupt,
    InsufficientLockedLp,
    NoPendingNativeLpUnlock,
    NativeLpUnlockNotReady,
    NativeSecurityModeInactive,
    NativeSecurityEpochNotCurrent,
    NativeSecurityEpochNotOpen,
    NativeSecurityEpochAlreadyOpen,
    NativeSecurityRewardFundingUnavailable,
    NativeSecurityRewardAccountingOverflow,
    NativeSecurityRewardPotNotFinalized,
    NativeSecurityRewardEpochExpired,
    NativeSecurityRewardAlreadyClaimed,
    NativeSecurityRewardAccountIneligible,
    NativeSecurityRewardZeroPot,
    DuplicateSecurityRewardEpoch,
    NoSecurityRewardClaimable,
    NativeSecurityRewardExpiryInvalid,
    InsufficientCompoundLpOutput,
  }

  impl<T: Config> Pallet<T> {
    fn ensure_lp_backed_selection() -> DispatchResult {
      ensure!(
        T::NativeSecurityModeProvider::mode() == NativeSecurityMode::LpBackedSelection,
        Error::<T>::NativeSecurityModeInactive
      );
      Ok(())
    }
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
      let (_, pool_account) = Self::create_staked_asset_for_pool(asset_id)?;
      let accounted_balance = T::Assets::balance(asset_id, &pool_account);
      Pools::<T>::insert(
        asset_id,
        PoolState {
          total_shares: Zero::zero(),
          accounted_balance,
        },
      );
      Self::deposit_event(Event::StakingAssetRegistered {
        asset_id,
        pool_account: Self::pool_account_for(asset_id),
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
      let staked_asset_id =
        Self::uses_staked_receipts(asset_id).ok_or(Error::<T>::StakedAssetNotInitialized)?;
      let available_shares = T::Assets::balance(staked_asset_id, &account);
      ensure!(!available_shares.is_zero(), Error::<T>::InsufficientShares);
      ensure!(available_shares >= shares, Error::<T>::InsufficientShares);
      let amount_out = Self::mul_div_floor(shares, pool.accounted_balance, pool.total_shares);
      ensure!(!amount_out.is_zero(), Error::<T>::ZeroAmountOut);
      let _ = T::Assets::burn_from(
        staked_asset_id,
        &account,
        shares,
        Preservation::Expendable,
        Precision::Exact,
        Fortitude::Force,
      )?;
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
      Pools::<T>::insert(asset_id, pool);
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
      ensure!(pool.total_shares.is_zero(), Error::<T>::PoolNotEmpty);
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

    #[pallet::call_index(9)]
    #[pallet::weight(T::WeightInfo::fund_native_security_reward())]
    #[transactional]
    pub fn fund_native_security_reward(origin: OriginFor<T>, amount: T::Balance) -> DispatchResult {
      T::SecurityRewardFundingOrigin::ensure_origin(origin)?;
      Self::do_fund_native_security_reward(
        T::SecurityEpochProvider::current_security_epoch(),
        amount,
      )
    }

    #[pallet::call_index(10)]
    #[pallet::weight(T::WeightInfo::claim_native_security_reward())]
    #[transactional]
    pub fn claim_native_security_reward(
      origin: OriginFor<T>,
      epoch: SecurityEpoch,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      let _ = Self::do_claim_native_security_rewards(&account, &[epoch])?;
      Ok(())
    }

    #[pallet::call_index(11)]
    #[pallet::weight(T::WeightInfo::claim_native_security_reward_batch(epochs.len() as u32))]
    #[transactional]
    pub fn claim_native_security_reward_batch(
      origin: OriginFor<T>,
      epochs: BoundedVec<SecurityEpoch, T::MaxSecurityRewardClaimsPerCall>,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      let _ = Self::do_claim_native_security_rewards(&account, &epochs)?;
      Ok(())
    }

    #[pallet::call_index(14)]
    #[pallet::weight(T::WeightInfo::claim_and_compound_native_security_reward())]
    #[transactional]
    pub fn claim_and_compound_native_security_reward(
      origin: OriginFor<T>,
      epoch: SecurityEpoch,
      operator: T::AccountId,
      min_lp_out: T::Balance,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      Self::ensure_lp_backed_selection()?;
      ensure!(account != operator, Error::<T>::CannotNominateSelf);
      ensure!(
        T::NativeOperatorValidator::is_valid_operator(&operator),
        Error::<T>::InvalidNativeOperatorTarget
      );
      ensure!(!min_lp_out.is_zero(), Error::<T>::ZeroAmount);
      let reward = Self::do_claim_native_security_rewards(&account, &[epoch])?;
      let (lp_asset_id, lp_out) =
        T::NativeSecurityRewardCompound::compound(&account, reward, min_lp_out)?;
      ensure!(
        lp_out >= min_lp_out,
        Error::<T>::InsufficientCompoundLpOutput
      );
      Self::lock_native_lp_for_collator(
        frame_system::RawOrigin::Signed(account).into(),
        lp_asset_id,
        lp_out,
        operator,
      )
    }

    #[pallet::call_index(12)]
    #[pallet::weight(T::WeightInfo::expire_native_security_reward())]
    #[transactional]
    pub fn expire_native_security_reward(
      origin: OriginFor<T>,
      epoch: SecurityEpoch,
    ) -> DispatchResult {
      let _ = ensure_signed(origin)?;
      Self::do_expire_native_security_reward(epoch)
    }

    #[pallet::call_index(13)]
    #[pallet::weight(T::WeightInfo::cleanup_expired_native_security_reward())]
    #[transactional]
    pub fn cleanup_expired_native_security_reward(
      origin: OriginFor<T>,
      epoch: SecurityEpoch,
    ) -> DispatchResult {
      let _ = ensure_signed(origin)?;
      Self::do_cleanup_expired_native_security_reward(epoch)
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
      Self::ensure_lp_backed_selection()?;
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
      let is_new_position = prior_lock.is_none();
      let nomination_operators = NativeNominationOperators::<T>::get(&account);
      let is_new_participant = nomination_operators.is_empty();
      if is_new_position {
        ensure!(
          !nomination_operators.contains(&operator),
          Error::<T>::NativeNominationIndexCorrupt
        );
        ensure!(
          nomination_operators.len() < T::MaxNominationsPerAccount::get() as usize,
          Error::<T>::NativeNominationLimitReached
        );
      } else {
        ensure!(
          nomination_operators.contains(&operator),
          Error::<T>::NativeNominationIndexCorrupt
        );
      }
      let participants = NativeSecurityParticipants::<T>::get();
      if is_new_participant {
        ensure!(
          !participants.contains(&account),
          Error::<T>::NativeSecurityParticipantIndexCorrupt
        );
        ensure!(
          participants.len() < T::MaxNativeSecurityParticipants::get() as usize,
          Error::<T>::NativeSecurityParticipantLimitReached
        );
      } else {
        ensure!(
          participants.contains(&account),
          Error::<T>::NativeSecurityParticipantIndexCorrupt
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
      if is_new_position {
        NativeNominationOperators::<T>::try_mutate(&account, |operators| {
          operators
            .try_push(operator.clone())
            .map_err(|_| Error::<T>::NativeNominationLimitReached)
        })?;
      }
      if is_new_participant {
        NativeSecurityParticipants::<T>::try_mutate(|participants| {
          participants
            .try_push(account.clone())
            .map_err(|_| Error::<T>::NativeSecurityParticipantLimitReached)
        })?;
      }
      OperatorNativeLpLocked::<T>::insert(&operator, new_operator_amount);
      AccountNativeLpLocked::<T>::insert(&account, new_account_amount);
      AccountNativeCollatorLpLocked::<T>::insert(&account, new_account_collator_amount);
      TotalNativeLpLocked::<T>::put(new_total_amount);
      Self::deposit_event(Event::NativeLpLocked {
        account,
        operator,
        lp_asset_id,
        amount,
        total_locked: new_account_operator_amount,
      });
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
      let mut nomination_operators = NativeNominationOperators::<T>::get(&account);
      let operator_index = nomination_operators
        .iter() // deos-bypass: bounded-iter — MaxNominationsPerAccount
        .position(|indexed| indexed == &operator)
        .ok_or(Error::<T>::NativeNominationIndexCorrupt)?;
      let mut participants = NativeSecurityParticipants::<T>::get();
      ensure!(
        participants.contains(&account),
        Error::<T>::NativeSecurityParticipantIndexCorrupt
      );
      lock.amount = lock
        .amount
        .checked_sub(&amount)
        .ok_or(ArithmeticError::Underflow)?;
      if lock.amount.is_zero() {
        NativeLpLocks::<T>::remove(&account, &operator);
        nomination_operators.remove(operator_index);
        if nomination_operators.is_empty() {
          let participant_index = participants
            .iter() // deos-bypass: bounded-iter — MaxNativeSecurityParticipants
            .position(|indexed| indexed == &account)
            .ok_or(Error::<T>::NativeSecurityParticipantIndexCorrupt)?;
          participants.remove(participant_index);
          NativeSecurityParticipants::<T>::put(participants);
          NativeNominationOperators::<T>::remove(&account);
        } else {
          NativeNominationOperators::<T>::insert(&account, nomination_operators);
        }
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
        account,
        operator,
        lp_asset_id: lock.lp_asset_id,
        amount,
        remaining_locked: lock.amount,
        unlock_block: effective_unlock_block,
      });
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
      Self::ensure_lp_backed_selection()?;
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
      let mut nomination_operators = NativeNominationOperators::<T>::get(&account);
      let from_index = nomination_operators
        .iter() // deos-bypass: bounded-iter — MaxNominationsPerAccount
        .position(|indexed| indexed == &from_operator)
        .ok_or(Error::<T>::NativeNominationIndexCorrupt)?;
      ensure!(
        NativeSecurityParticipants::<T>::get().contains(&account),
        Error::<T>::NativeSecurityParticipantIndexCorrupt
      );
      if to_lock.is_some() {
        ensure!(
          nomination_operators.contains(&to_operator),
          Error::<T>::NativeNominationIndexCorrupt
        );
      } else {
        ensure!(
          !nomination_operators.contains(&to_operator),
          Error::<T>::NativeNominationIndexCorrupt
        );
        if from_lock.amount != amount {
          ensure!(
            nomination_operators.len() < T::MaxNominationsPerAccount::get() as usize,
            Error::<T>::NativeNominationLimitReached
          );
        }
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
      if to_lock.is_none() {
        if from_lock.amount.is_zero() {
          nomination_operators[from_index] = to_operator.clone();
        } else {
          nomination_operators
            .try_push(to_operator.clone())
            .map_err(|_| Error::<T>::NativeNominationLimitReached)?;
        }
        NativeNominationOperators::<T>::insert(&account, nomination_operators);
      } else if from_lock.amount.is_zero() {
        nomination_operators.remove(from_index);
        NativeNominationOperators::<T>::insert(&account, nomination_operators);
      }
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
  }

  impl<T: Config> Pallet<T> {
    fn do_stake(
      asset_id: T::AssetId,
      account: &T::AccountId,
      amount: T::Balance,
    ) -> Result<T::Balance, DispatchError> {
      Self::credit_stake_from(asset_id, account, account, amount, Preservation::Protect)
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
      let staked_asset_id_for_mint =
        Self::uses_staked_receipts(asset_id).ok_or(Error::<T>::StakedAssetNotInitialized)?;
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
      let _ = T::Assets::mint_into(staked_asset_id_for_mint, beneficiary, minted_shares)?;
      Pools::<T>::insert(asset_id, pool);
      Ok(minted_shares)
    }

    fn create_staked_asset_for_pool(
      asset_id: T::AssetId,
    ) -> Result<(T::AssetId, T::AccountId), DispatchError> {
      let pool_account = Self::pool_account_for(asset_id);
      let staked_asset_id =
        Self::staked_asset_id(asset_id).ok_or(Error::<T>::StakedAssetUnsupported)?;
      ensure!(
        !T::Assets::asset_exists(staked_asset_id),
        Error::<T>::StakedAssetIdCollision
      );
      T::StakedAssetLifecycle::register(asset_id, staked_asset_id, &pool_account)?;
      Self::index_live_staked_asset(asset_id, staked_asset_id)?;
      Ok((staked_asset_id, pool_account))
    }

    pub fn pool_account_for(asset_id: T::AssetId) -> T::AccountId {
      T::PalletId::get().into_sub_account_truncating(asset_id)
    }

    pub fn native_lp_lock_account() -> T::AccountId {
      let seed = frame::hashing::blake2_256(&(T::PalletId::get(), b"native-lp-lock").encode());
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::new(&seed))
        .expect("hashed native LP lock seed always decodes into AccountId")
    }

    pub fn native_security_reward_account() -> T::AccountId {
      let seed =
        frame::hashing::blake2_256(&(T::PalletId::get(), b"native-security-reward").encode());
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::new(&seed))
        .expect("hashed native security reward seed always decodes into AccountId")
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

    fn effective_share_balance(asset_id: T::AssetId, account: &T::AccountId) -> Option<T::Balance> {
      let staked_asset_id = Self::live_staked_asset_id(asset_id)?;
      let shares = T::Assets::balance(staked_asset_id, account);
      (!shares.is_zero()).then_some(shares)
    }

    pub fn staked_asset_id_for_queries(asset_id: T::AssetId) -> Option<T::AssetId> {
      Self::live_staked_asset_id(asset_id)
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

    #[cfg(feature = "try-runtime")]
    pub(crate) fn do_try_state() -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      use alloc::collections::BTreeSet;
      use polkadot_sdk::sp_runtime::TryRuntimeError;

      let active_reward_epoch =
        ActiveNativeSecurityEpochSnapshot::<T>::get().map(|item| item.epoch);
      let current_reward_epoch = T::SecurityEpochProvider::current_security_epoch();
      let mut retained_reward_epochs = BTreeSet::new();
      let mut expected_reward_liability = T::Balance::zero();
      let reward_snapshot_iter = NativeSecurityEpochSnapshots::<T>::iter(); // deos-bypass: bounded-iter — try-runtime-only full reconciliation
      for (epoch, snapshot) in reward_snapshot_iter {
        if !retained_reward_epochs.insert(epoch) || snapshot.epoch != epoch {
          return Err(TryRuntimeError::Other(
            "Native security snapshot epoch identity is inconsistent",
          ));
        }
        let pot = NativeSecurityRewardPots::<T>::get(epoch).ok_or(TryRuntimeError::Other(
          "Native security snapshot is missing its reward pot",
        ))?;
        if pot.total_reward_weight != snapshot.total_reward_weight || pot.claimed > pot.credited {
          return Err(TryRuntimeError::Other(
            "Native security reward pot disagrees with its frozen snapshot",
          ));
        }
        match pot.status {
          NativeSecurityRewardPotStatus::Planned => {
            if epoch <= current_reward_epoch || !pot.credited.is_zero() || !pot.claimed.is_zero() {
              return Err(TryRuntimeError::Other(
                "Planned native security reward pot has invalid epoch or accounting",
              ));
            }
          }
          NativeSecurityRewardPotStatus::Open => {
            if active_reward_epoch != Some(epoch) || epoch != current_reward_epoch {
              return Err(TryRuntimeError::Other(
                "Open native security reward pot is not the active security epoch",
              ));
            }
            expected_reward_liability = expected_reward_liability
              .checked_add(
                &pot
                  .credited
                  .checked_sub(&pot.claimed)
                  .ok_or(TryRuntimeError::Other("Native reward pot underflowed"))?,
              )
              .ok_or(TryRuntimeError::Other("Native reward liability overflowed"))?;
          }
          NativeSecurityRewardPotStatus::Finalized => {
            if epoch >= current_reward_epoch {
              return Err(TryRuntimeError::Other(
                "Finalized native security reward pot is not historical",
              ));
            }
            expected_reward_liability = expected_reward_liability
              .checked_add(
                &pot
                  .credited
                  .checked_sub(&pot.claimed)
                  .ok_or(TryRuntimeError::Other("Native reward pot underflowed"))?,
              )
              .ok_or(TryRuntimeError::Other("Native reward liability overflowed"))?;
          }
          NativeSecurityRewardPotStatus::Expired => {
            if epoch >= current_reward_epoch {
              return Err(TryRuntimeError::Other(
                "Expired native security reward pot is not historical",
              ));
            }
          }
        }
      }
      let reward_pot_iter = NativeSecurityRewardPots::<T>::iter(); // deos-bypass: bounded-iter — try-runtime-only full reconciliation
      for (epoch, _) in reward_pot_iter {
        if !retained_reward_epochs.contains(&epoch) {
          return Err(TryRuntimeError::Other(
            "Native security reward pot is missing its frozen snapshot",
          ));
        }
      }
      let reward_claim_iter = NativeSecurityRewardClaims::<T>::iter(); // deos-bypass: bounded-iter — try-runtime-only full reconciliation
      for (epoch, account, _) in reward_claim_iter {
        let snapshot = NativeSecurityEpochSnapshots::<T>::get(epoch).ok_or(
          TryRuntimeError::Other("Native security reward claim is missing its snapshot"),
        )?;
        let pot = NativeSecurityRewardPots::<T>::get(epoch).ok_or(TryRuntimeError::Other(
          "Native security reward claim is missing its pot",
        ))?;
        if matches!(
          pot.status,
          NativeSecurityRewardPotStatus::Planned | NativeSecurityRewardPotStatus::Open
        ) || !snapshot
          .participants
          .iter() // deos-bypass: bounded-iter — MaxNativeSecurityParticipants
          .any(|item| item.account == account && !item.reward_weight.is_zero())
        {
          return Err(TryRuntimeError::Other(
            "Native security reward claim is inconsistent with frozen eligibility",
          ));
        }
      }
      if NativeSecurityRewardLiability::<T>::get() != expected_reward_liability {
        return Err(TryRuntimeError::Other(
          "NativeSecurityRewardLiability disagrees with retained pots",
        ));
      }
      let reward_spendable =
        T::NativeCurrency::free_balance(&Self::native_security_reward_account())
          .checked_sub(&T::NativeCurrency::minimum_balance())
          .unwrap_or_else(T::Balance::zero);
      if reward_spendable < expected_reward_liability {
        return Err(TryRuntimeError::Other(
          "Native security reward custody is below outstanding liability",
        ));
      }

      let participants = NativeSecurityParticipants::<T>::get();
      let mut seen_participants = BTreeSet::new();
      let mut expected_operator_totals = alloc::collections::BTreeMap::new();
      let mut expected_account_totals = alloc::collections::BTreeMap::new();
      let mut expected_active_total = T::Balance::zero();
      for account in &participants {
        if !seen_participants.insert(account.clone()) {
          return Err(TryRuntimeError::Other(
            "NativeSecurityParticipants contains a duplicate account",
          ));
        }
        let operators = NativeNominationOperators::<T>::get(account);
        if operators.is_empty() {
          return Err(TryRuntimeError::Other(
            "NativeSecurityParticipants references an account without nominations",
          ));
        }
        let mut seen_operators = BTreeSet::new();
        let mut account_total = T::Balance::zero();
        for operator in &operators {
          if !seen_operators.insert(operator.clone()) {
            return Err(TryRuntimeError::Other(
              "NativeNominationOperators contains a duplicate operator",
            ));
          }
          let Some(lock) = NativeLpLocks::<T>::get(account, operator) else {
            return Err(TryRuntimeError::Other(
              "NativeNominationOperators references a missing position",
            ));
          };
          if lock.amount.is_zero() {
            return Err(TryRuntimeError::Other(
              "NativeLpLocks contains a zero active position",
            ));
          }
          account_total = account_total
            .checked_add(&lock.amount)
            .ok_or(TryRuntimeError::Other(
              "Native nomination account total overflowed",
            ))?;
          expected_active_total =
            expected_active_total
              .checked_add(&lock.amount)
              .ok_or(TryRuntimeError::Other(
                "Native nomination global total overflowed",
              ))?;
          let operator_total = expected_operator_totals
            .entry(operator.clone())
            .or_insert_with(T::Balance::zero);
          *operator_total =
            operator_total
              .checked_add(&lock.amount)
              .ok_or(TryRuntimeError::Other(
                "Native nomination operator total overflowed",
              ))?;
        }
        if account_total != AccountNativeCollatorLpLocked::<T>::get(account) {
          return Err(TryRuntimeError::Other(
            "AccountNativeCollatorLpLocked disagrees with indexed positions",
          ));
        }
        let governance_total = NativeGovernanceLpLocks::<T>::get(account)
          .map(|lock| lock.amount)
          .unwrap_or_else(T::Balance::zero);
        let expected_account_total =
          account_total
            .checked_add(&governance_total)
            .ok_or(TryRuntimeError::Other(
              "Native account custody total overflowed",
            ))?;
        if expected_account_total != AccountNativeLpLocked::<T>::get(account) {
          return Err(TryRuntimeError::Other(
            "AccountNativeLpLocked disagrees with active and governance positions",
          ));
        }
        expected_account_totals.insert(account.clone(), expected_account_total);
      }

      for (operator, expected_total) in &expected_operator_totals {
        if OperatorNativeLpLocked::<T>::get(operator) != *expected_total {
          return Err(TryRuntimeError::Other(
            "OperatorNativeLpLocked disagrees with indexed positions",
          ));
        }
      }
      for (operator, stored_total) in OperatorNativeLpLocked::<T>::iter() {
        if stored_total.is_zero()
          || expected_operator_totals
            .get(&operator)
            .copied()
            .unwrap_or_default()
            != stored_total
        {
          return Err(TryRuntimeError::Other(
            "OperatorNativeLpLocked contains an orphan or zero aggregate",
          ));
        }
      }

      let mut position_iter = NativeLpLocks::<T>::iter();
      while let Some((account, operator, lock)) = position_iter.next() {
        if lock.amount.is_zero() {
          return Err(TryRuntimeError::Other(
            "NativeLpLocks contains a zero active position",
          ));
        }
        if !participants.contains(&account) {
          return Err(TryRuntimeError::Other(
            "NativeLpLocks account is missing from NativeSecurityParticipants",
          ));
        }
        if !NativeNominationOperators::<T>::get(&account).contains(&operator) {
          return Err(TryRuntimeError::Other(
            "NativeLpLocks operator is missing from NativeNominationOperators",
          ));
        }
      }

      let mut expected_total_custody = expected_active_total;
      for (account, lock) in NativeGovernanceLpLocks::<T>::iter() {
        if lock.amount.is_zero() {
          return Err(TryRuntimeError::Other(
            "NativeGovernanceLpLocks contains a zero position",
          ));
        }
        expected_total_custody = expected_total_custody
          .checked_add(&lock.amount)
          .ok_or(TryRuntimeError::Other("Native LP custody total overflowed"))?;
        let expected_account_total = AccountNativeCollatorLpLocked::<T>::get(&account)
          .checked_add(&lock.amount)
          .ok_or(TryRuntimeError::Other(
            "Native governance account total overflowed",
          ))?;
        if AccountNativeLpLocked::<T>::get(&account) != expected_account_total {
          return Err(TryRuntimeError::Other(
            "AccountNativeLpLocked disagrees with governance custody",
          ));
        }
        expected_account_totals.insert(account, expected_account_total);
      }
      for (account, stored_total) in AccountNativeLpLocked::<T>::iter() {
        if stored_total.is_zero()
          || expected_account_totals
            .get(&account)
            .copied()
            .unwrap_or_default()
            != stored_total
        {
          return Err(TryRuntimeError::Other(
            "AccountNativeLpLocked contains an orphan or zero aggregate",
          ));
        }
      }
      if TotalNativeLpLocked::<T>::get() != expected_total_custody {
        return Err(TryRuntimeError::Other(
          "TotalNativeLpLocked disagrees with active and governance positions",
        ));
      }

      let mut pending_lp_total = T::Balance::zero();
      for (_, _, pending) in PendingNativeLpUnlocks::<T>::iter() {
        if pending.amount.is_zero() {
          return Err(TryRuntimeError::Other(
            "PendingNativeLpUnlocks contains a zero request",
          ));
        }
        pending_lp_total =
          pending_lp_total
            .checked_add(&pending.amount)
            .ok_or(TryRuntimeError::Other(
              "Pending native LP unlock total overflowed",
            ))?;
      }
      for (_, pending) in PendingNativeGovernanceLpUnlocks::<T>::iter() {
        if pending.amount.is_zero() {
          return Err(TryRuntimeError::Other(
            "PendingNativeGovernanceLpUnlocks contains a zero request",
          ));
        }
        pending_lp_total =
          pending_lp_total
            .checked_add(&pending.amount)
            .ok_or(TryRuntimeError::Other(
              "Pending governance LP unlock total overflowed",
            ))?;
      }
      let expected_lp_custody = expected_total_custody
        .checked_add(&pending_lp_total)
        .ok_or(TryRuntimeError::Other(
          "Physical native LP custody total overflowed",
        ))?;
      let Some(lp_asset_id) = T::NativeStakingReadModelProvider::native_staking_liquidity_pool()
        .map(|(asset_id, _, _, _)| asset_id)
      else {
        if !expected_lp_custody.is_zero() {
          return Err(TryRuntimeError::Other(
            "Native LP custody exists without a canonical LP asset",
          ));
        }
        return Ok(());
      };
      if T::Assets::balance(lp_asset_id, &Self::native_lp_lock_account()) != expected_lp_custody {
        return Err(TryRuntimeError::Other(
          "Native LP lock account balance disagrees with active and pending custody",
        ));
      }
      Ok(())
    }
  }

  impl<T: Config> Pallet<T> {
    pub fn governance_participation_coefficient(
      domain: T::GovernanceDomainId,
      account: &T::AccountId,
    ) -> Option<FixedU128> {
      Some(
        T::GovernanceParticipationCoefficientProvider::governance_participation_coefficient(
          domain, account,
        ),
      )
    }

    pub fn note_native_security_boundary(
      planned_epoch: SecurityEpoch,
      readiness: NativeSecurityReadiness,
    ) {
      LastNativeSecurityBoundaryDiagnostic::<T>::put(NativeSecurityBoundaryDiagnostic {
        planned_epoch,
        readiness,
      });
    }

    pub fn open_native_security_epoch(
      epoch: SecurityEpoch,
      eligible_operators: &[T::AccountId],
    ) -> Result<(), DispatchError> {
      Self::ensure_lp_backed_selection()?;
      let mut operator_snapshots = BoundedVec::<
        NativeSecurityOperatorSnapshot<T::AccountId, T::Balance>,
        T::MaxNativeSecurityOperators,
      >::default();
      for operator in eligible_operators {
        ensure!(
          T::NativeOperatorValidator::is_valid_operator(operator),
          Error::<T>::InvalidNativeOperatorTarget
        );
        let backing = T::NativeStakingReadModelProvider::native_lp_value(
          OperatorNativeLpLocked::<T>::get(operator),
        )
        .ok_or(Error::<T>::NativeSecurityValuationUnavailable)?;
        ensure!(!backing.is_zero(), Error::<T>::InvalidNativeOperatorTarget);
        operator_snapshots
          .try_push(NativeSecurityOperatorSnapshot {
            operator: operator.clone(),
            conservative_native_backing: backing,
          })
          .map_err(|_| Error::<T>::NativeSecurityOperatorLimitReached)?;
      }

      let mut participant_snapshots = BoundedVec::<
        NativeSecurityAccountSnapshot<T::AccountId, T::Balance>,
        T::MaxNativeSecurityParticipants,
      >::default();
      let mut total_reward_weight = T::Balance::zero();
      for account in NativeSecurityParticipants::<T>::get() {
        let mut eligible_locked_lp = T::Balance::zero();
        for operator in NativeNominationOperators::<T>::get(&account) {
          if !eligible_operators.contains(&operator) {
            continue;
          }
          let lock = NativeLpLocks::<T>::get(&account, &operator)
            .ok_or(Error::<T>::NativeNominationIndexCorrupt)?;
          eligible_locked_lp = eligible_locked_lp
            .checked_add(&lock.amount)
            .ok_or(ArithmeticError::Overflow)?;
        }
        let conservative_native_value =
          T::NativeStakingReadModelProvider::native_lp_value(eligible_locked_lp)
            .ok_or(Error::<T>::NativeSecurityValuationUnavailable)?;
        let governance_coefficient =
          T::GovernanceParticipationCoefficientProvider::governance_participation_coefficient(
            T::NativeGovernanceDomainId::get(),
            &account,
          );
        let reward_weight =
          Self::reward_weight_from_snapshot(conservative_native_value, governance_coefficient);
        total_reward_weight = total_reward_weight
          .checked_add(&reward_weight)
          .ok_or(ArithmeticError::Overflow)?;
        participant_snapshots
          .try_push(NativeSecurityAccountSnapshot {
            account,
            conservative_native_value,
            governance_coefficient,
            reward_weight,
          })
          .map_err(|_| Error::<T>::NativeSecurityParticipantLimitReached)?;
      }

      ensure!(
        !NativeSecurityEpochSnapshots::<T>::contains_key(epoch),
        Error::<T>::NativeSecurityEpochAlreadyOpen
      );
      let snapshot = NativeSecurityEpochSnapshot {
        epoch,
        participants: participant_snapshots,
        eligible_operators: operator_snapshots,
        total_reward_weight,
      };
      NativeSecurityEpochSnapshots::<T>::insert(epoch, snapshot);
      NativeSecurityRewardPots::<T>::insert(
        epoch,
        NativeSecurityRewardPot {
          total_reward_weight,
          credited: Zero::zero(),
          claimed: Zero::zero(),
          status: NativeSecurityRewardPotStatus::Planned,
        },
      );
      Ok(())
    }

    pub fn activate_native_security_epoch(epoch: SecurityEpoch) -> DispatchResult {
      Self::ensure_lp_backed_selection()?;
      ensure!(
        epoch == T::SecurityEpochProvider::current_security_epoch(),
        Error::<T>::NativeSecurityEpochNotCurrent
      );
      let snapshot = NativeSecurityEpochSnapshots::<T>::get(epoch)
        .ok_or(Error::<T>::NativeSecurityEpochNotOpen)?;
      let mut pot =
        NativeSecurityRewardPots::<T>::get(epoch).ok_or(Error::<T>::NativeSecurityEpochNotOpen)?;
      ensure!(
        pot.status == NativeSecurityRewardPotStatus::Planned,
        Error::<T>::NativeSecurityEpochNotOpen
      );
      let prior = ActiveNativeSecurityEpochSnapshot::<T>::get();
      let prior_pot = prior
        .as_ref()
        .map(|active| {
          let pot = NativeSecurityRewardPots::<T>::get(active.epoch)
            .ok_or(Error::<T>::NativeSecurityEpochNotOpen)?;
          ensure!(
            pot.status == NativeSecurityRewardPotStatus::Open,
            Error::<T>::NativeSecurityEpochNotOpen
          );
          Ok::<_, DispatchError>((active.epoch, pot))
        })
        .transpose()?;
      if let Some((prior_epoch, mut prior_pot)) = prior_pot {
        prior_pot.status = NativeSecurityRewardPotStatus::Finalized;
        NativeSecurityRewardPots::<T>::insert(prior_epoch, prior_pot);
      }
      pot.status = NativeSecurityRewardPotStatus::Open;
      NativeSecurityRewardPots::<T>::insert(epoch, pot);
      ActiveNativeSecurityEpochSnapshot::<T>::put(snapshot);
      Ok(())
    }

    pub fn preflight_native_security_reward_funding(
      source: &T::AccountId,
      epoch: SecurityEpoch,
      amount: T::Balance,
    ) -> DispatchResult {
      ensure!(
        source == &T::SecurityRewardFundingSource::get(),
        Error::<T>::NativeSecurityRewardFundingUnavailable
      );
      Self::validate_native_security_reward_funding(epoch, amount)
    }

    pub fn certify_native_security_reward_funding(
      source: &T::AccountId,
      epoch: SecurityEpoch,
      amount: T::Balance,
    ) -> DispatchResult {
      ensure!(
        source == &T::SecurityRewardFundingSource::get(),
        Error::<T>::NativeSecurityRewardFundingUnavailable
      );
      Self::record_native_security_reward_funding(epoch, amount)
    }

    fn do_fund_native_security_reward(epoch: SecurityEpoch, amount: T::Balance) -> DispatchResult {
      Self::validate_native_security_reward_funding(epoch, amount)?;
      let source = T::SecurityRewardFundingSource::get();
      let reward_account = Self::native_security_reward_account();
      T::NativeCurrency::transfer(
        &source,
        &reward_account,
        amount,
        polkadot_sdk::frame_support::traits::ExistenceRequirement::KeepAlive,
      )?;
      Self::record_native_security_reward_funding(epoch, amount)
    }

    fn validate_native_security_reward_funding(
      epoch: SecurityEpoch,
      amount: T::Balance,
    ) -> DispatchResult {
      Self::ensure_lp_backed_selection()?;
      ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
      ensure!(
        epoch == T::SecurityEpochProvider::current_security_epoch(),
        Error::<T>::NativeSecurityEpochNotCurrent
      );
      let active = ActiveNativeSecurityEpochSnapshot::<T>::get()
        .ok_or(Error::<T>::NativeSecurityEpochNotOpen)?;
      ensure!(
        active.epoch == epoch,
        Error::<T>::NativeSecurityEpochNotOpen
      );
      let pot =
        NativeSecurityRewardPots::<T>::get(epoch).ok_or(Error::<T>::NativeSecurityEpochNotOpen)?;
      ensure!(
        pot.status == NativeSecurityRewardPotStatus::Open,
        Error::<T>::NativeSecurityEpochNotOpen
      );
      ensure!(
        NativeSecurityEpochSnapshots::<T>::contains_key(epoch),
        Error::<T>::NativeSecurityEpochNotOpen
      );
      pot
        .credited
        .checked_add(&amount)
        .ok_or(Error::<T>::NativeSecurityRewardAccountingOverflow)?;
      NativeSecurityRewardLiability::<T>::get()
        .checked_add(&amount)
        .ok_or(Error::<T>::NativeSecurityRewardAccountingOverflow)?;
      Ok(())
    }

    fn do_claim_native_security_rewards(
      account: &T::AccountId,
      epochs: &[SecurityEpoch],
    ) -> Result<T::Balance, DispatchError> {
      Self::ensure_lp_backed_selection()?;
      ensure!(!epochs.is_empty(), Error::<T>::NoSecurityRewardClaimable);
      let current_epoch = T::SecurityEpochProvider::current_security_epoch();
      let mut seen = alloc::collections::BTreeSet::new();
      for epoch in epochs {
        ensure!(
          seen.insert(*epoch),
          Error::<T>::DuplicateSecurityRewardEpoch
        );
      }
      let mut claims = Vec::with_capacity(epochs.len());
      let mut total = T::Balance::zero();
      for epoch in epochs {
        let snapshot = NativeSecurityEpochSnapshots::<T>::get(epoch)
          .ok_or(Error::<T>::NativeSecurityEpochNotOpen)?;
        let pot = NativeSecurityRewardPots::<T>::get(epoch)
          .ok_or(Error::<T>::NativeSecurityEpochNotOpen)?;
        ensure!(
          pot.status == NativeSecurityRewardPotStatus::Finalized,
          Error::<T>::NativeSecurityRewardPotNotFinalized
        );
        ensure!(
          current_epoch.saturating_sub(*epoch) <= T::SecurityRewardClaimHorizon::get(),
          Error::<T>::NativeSecurityRewardEpochExpired
        );
        ensure!(
          !pot.credited.is_zero(),
          Error::<T>::NativeSecurityRewardZeroPot
        );
        ensure!(
          !pot.total_reward_weight.is_zero(),
          Error::<T>::NoSecurityRewardClaimable
        );
        ensure!(
          !NativeSecurityRewardClaims::<T>::contains_key(epoch, account),
          Error::<T>::NativeSecurityRewardAlreadyClaimed
        );
        let account_weight = snapshot
          .participants
          .iter() // deos-bypass: bounded-iter — MaxNativeSecurityParticipants
          .find(|participant| &participant.account == account)
          .map(|participant| participant.reward_weight)
          .filter(|weight| !weight.is_zero())
          .ok_or(Error::<T>::NativeSecurityRewardAccountIneligible)?;
        let amount = Self::mul_div_floor(account_weight, pot.credited, pot.total_reward_weight);
        total = total
          .checked_add(&amount)
          .ok_or(Error::<T>::NativeSecurityRewardAccountingOverflow)?;
        let claimed = pot
          .claimed
          .checked_add(&amount)
          .ok_or(Error::<T>::NativeSecurityRewardAccountingOverflow)?;
        ensure!(
          claimed <= pot.credited,
          Error::<T>::NativeSecurityRewardAccountingOverflow
        );
        claims.push((*epoch, pot, claimed, amount));
      }
      ensure!(!total.is_zero(), Error::<T>::NoSecurityRewardClaimable);
      let outstanding_liability = NativeSecurityRewardLiability::<T>::get()
        .checked_sub(&total)
        .ok_or(Error::<T>::NativeSecurityRewardAccountingOverflow)?;
      for (epoch, mut pot, claimed, amount) in claims {
        pot.claimed = claimed;
        NativeSecurityRewardPots::<T>::insert(epoch, pot);
        NativeSecurityRewardClaims::<T>::insert(epoch, account, ());
        Self::deposit_event(Event::NativeSecurityRewardClaimed {
          epoch,
          account: account.clone(),
          amount,
          outstanding_liability,
        });
      }
      T::NativeCurrency::transfer(
        &Self::native_security_reward_account(),
        account,
        total,
        polkadot_sdk::frame_support::traits::ExistenceRequirement::KeepAlive,
      )?;
      NativeSecurityRewardLiability::<T>::put(outstanding_liability);
      Ok(total)
    }

    fn do_expire_native_security_reward(epoch: SecurityEpoch) -> DispatchResult {
      Self::ensure_lp_backed_selection()?;
      let current_epoch = T::SecurityEpochProvider::current_security_epoch();
      ensure!(
        current_epoch.saturating_sub(epoch) > T::SecurityRewardClaimHorizon::get(),
        Error::<T>::NativeSecurityRewardExpiryInvalid
      );
      let mut pot =
        NativeSecurityRewardPots::<T>::get(epoch).ok_or(Error::<T>::NativeSecurityEpochNotOpen)?;
      ensure!(
        pot.status == NativeSecurityRewardPotStatus::Finalized,
        Error::<T>::NativeSecurityRewardExpiryInvalid
      );
      let returned = pot
        .credited
        .checked_sub(&pot.claimed)
        .ok_or(Error::<T>::NativeSecurityRewardAccountingOverflow)?;
      let outstanding_liability = NativeSecurityRewardLiability::<T>::get()
        .checked_sub(&returned)
        .ok_or(Error::<T>::NativeSecurityRewardAccountingOverflow)?;
      let reward_account = Self::native_security_reward_account();
      let spendable_balance = T::NativeCurrency::free_balance(&reward_account)
        .checked_sub(&T::NativeCurrency::minimum_balance())
        .unwrap_or_else(Zero::zero);
      let current_liability = NativeSecurityRewardLiability::<T>::get();
      let uncredited_excess = spendable_balance
        .checked_sub(&current_liability)
        .ok_or(Error::<T>::NativeSecurityRewardAccountingOverflow)?;
      let total_return = returned
        .checked_add(&uncredited_excess)
        .ok_or(Error::<T>::NativeSecurityRewardAccountingOverflow)?;
      if !total_return.is_zero() {
        T::NativeCurrency::transfer(
          &reward_account,
          &T::SecurityRewardFundingSource::get(),
          total_return,
          polkadot_sdk::frame_support::traits::ExistenceRequirement::KeepAlive,
        )?;
      }
      let retained_balance = T::NativeCurrency::free_balance(&reward_account)
        .checked_sub(&T::NativeCurrency::minimum_balance())
        .unwrap_or_else(Zero::zero);
      ensure!(
        retained_balance == outstanding_liability,
        Error::<T>::NativeSecurityRewardAccountingOverflow
      );
      pot.status = NativeSecurityRewardPotStatus::Expired;
      NativeSecurityRewardPots::<T>::insert(epoch, pot);
      NativeSecurityRewardLiability::<T>::put(outstanding_liability);
      Self::deposit_event(Event::NativeSecurityRewardExpired {
        epoch,
        returned,
        uncredited_excess,
        outstanding_liability,
      });
      Ok(())
    }

    fn do_cleanup_expired_native_security_reward(epoch: SecurityEpoch) -> DispatchResult {
      let pot =
        NativeSecurityRewardPots::<T>::get(epoch).ok_or(Error::<T>::NativeSecurityEpochNotOpen)?;
      ensure!(
        pot.status == NativeSecurityRewardPotStatus::Expired,
        Error::<T>::NativeSecurityRewardExpiryInvalid
      );
      NativeSecurityRewardClaims::<T>::clear_prefix(
        epoch,
        T::MaxNativeSecurityParticipants::get(),
        None,
      )
      .maybe_cursor
      .is_none()
      .then_some(())
      .ok_or(Error::<T>::NativeSecurityRewardExpiryInvalid)?;
      NativeSecurityEpochSnapshots::<T>::remove(epoch);
      NativeSecurityRewardPots::<T>::remove(epoch);
      Ok(())
    }

    fn record_native_security_reward_funding(
      epoch: SecurityEpoch,
      amount: T::Balance,
    ) -> DispatchResult {
      Self::validate_native_security_reward_funding(epoch, amount)?;
      let mut pot =
        NativeSecurityRewardPots::<T>::get(epoch).ok_or(Error::<T>::NativeSecurityEpochNotOpen)?;
      let epoch_credited = pot
        .credited
        .checked_add(&amount)
        .ok_or(Error::<T>::NativeSecurityRewardAccountingOverflow)?;
      let outstanding_liability = NativeSecurityRewardLiability::<T>::get()
        .checked_add(&amount)
        .ok_or(Error::<T>::NativeSecurityRewardAccountingOverflow)?;
      pot.credited = epoch_credited;
      NativeSecurityRewardPots::<T>::insert(epoch, pot);
      NativeSecurityRewardLiability::<T>::put(outstanding_liability);
      Self::deposit_event(Event::NativeSecurityRewardFunded {
        epoch,
        source: T::SecurityRewardFundingSource::get(),
        amount,
        epoch_credited,
        outstanding_liability,
      });
      Ok(())
    }
  }

  #[pallet::view_functions]
  impl<T: Config> Pallet<T> {
    pub fn native_security_mode() -> NativeSecurityMode {
      T::NativeSecurityModeProvider::mode()
    }

    pub fn current_security_epoch() -> SecurityEpoch {
      T::SecurityEpochProvider::current_security_epoch()
    }

    pub fn native_security_capabilities() -> NativeSecurityCapabilities {
      let lp_backed =
        T::NativeSecurityModeProvider::mode() == NativeSecurityMode::LpBackedSelection;
      NativeSecurityCapabilities {
        new_nominations: lp_backed,
        redelegation: lp_backed,
        candidate_selection: lp_backed,
        reward_funding: lp_backed,
        reward_claims: lp_backed,
        reward_compound: lp_backed,
        custody_exit: true,
      }
    }

    pub fn native_security_readiness() -> NativeSecurityReadiness {
      if T::NativeSecurityModeProvider::mode() != NativeSecurityMode::LpBackedSelection {
        return NativeSecurityReadiness::Inactive;
      }
      let native_asset_id = T::NativeStakingAssetId::get();
      if !Pools::<T>::contains_key(native_asset_id) {
        return NativeSecurityReadiness::NativePoolMissing;
      }
      if Self::staked_asset_id(native_asset_id).is_none() {
        return NativeSecurityReadiness::StakedAssetMissing;
      }
      let Some((lp_asset_id, reserve_native, reserve_staked, lp_total_issuance)) =
        T::NativeStakingReadModelProvider::native_staking_liquidity_pool()
      else {
        return NativeSecurityReadiness::LiquidityPoolMissing;
      };
      if !T::NativeStakingLpAssetValidator::is_valid_native_staking_lp_asset(lp_asset_id) {
        return NativeSecurityReadiness::CanonicalLpMismatch;
      }
      if reserve_native.is_zero() {
        return NativeSecurityReadiness::EmptyNativeReserve;
      }
      if reserve_staked.is_zero() {
        return NativeSecurityReadiness::EmptyStakedReserve;
      }
      if lp_total_issuance.is_zero() {
        return NativeSecurityReadiness::EmptyLpIssuance;
      }
      if T::NativeStakingReadModelProvider::native_lp_value(T::Balance::from(1u32)).is_none() {
        return NativeSecurityReadiness::ValuationUnavailable;
      }
      let participants = NativeSecurityParticipants::<T>::get();
      for account in &participants {
        let operators = NativeNominationOperators::<T>::get(account);
        if operators.is_empty()
          || operators
            .iter() // deos-bypass: bounded-iter — MaxNominationsPerAccount
            .any(|operator| !NativeLpLocks::<T>::contains_key(account, operator))
        {
          return NativeSecurityReadiness::ParticipantIndexInconsistent;
        }
      }
      T::NativeStakingReadModelProvider::native_security_topology_readiness()
        .unwrap_or(NativeSecurityReadiness::CandidateSetInconsistent)
    }

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
  }
}
