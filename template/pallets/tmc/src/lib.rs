//! Token Minting Curve Pallet
//!
//! Implements the TMCTOL standard's linear price ceiling with unidirectional minting on DEOS.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

pub mod types;
pub use types::AssetKind;

#[cfg(test)]
pub mod mock;
#[cfg(test)]
pub mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::WeightInfo;

use frame::prelude::*;
use polkadot_sdk::{
  frame_support::{
    PalletId,
    traits::{
      fungible::{Inspect as NativeInspect, Mutate as NativeMutate},
      fungibles::{Inspect, Mutate},
      tokens::Preservation,
    },
    transactional,
  },
  sp_core::U256,
  sp_runtime::{
    DispatchError, Perbill,
    traits::{AccountIdConversion, AtLeast32BitUnsigned, UniqueSaturatedInto, Zero},
  },
};

/// Hook trait for deterministic TMCTOL domain glue on curve activation
pub trait DomainGlueHook {
  fn on_curve_created(
    token_asset: AssetKind,
    foreign_asset: AssetKind,
  ) -> Result<(), DispatchError>;
}

impl DomainGlueHook for () {
  fn on_curve_created(
    _token_asset: AssetKind,
    _foreign_asset: AssetKind,
  ) -> Result<(), DispatchError> {
    Ok(())
  }
}

/// Resolves the output destination for a given minting curve.
///
/// TMC sends both the collateral and the zap share of minted tokens
/// to a single sink address. Downstream routing (splitting, liquidity
/// provisioning) is the sink's responsibility.
pub trait MintOutputResolver<AccountId> {
  fn output_account(minted_asset: AssetKind) -> AccountId;
}

pub trait MintDistributionHook<AccountId> {
  fn before_user_mint(
    _minted_asset: AssetKind,
    _account: &AccountId,
    _amount: Balance,
  ) -> Result<(), DispatchError> {
    Ok(())
  }

  fn before_sink_mint(
    _minted_asset: AssetKind,
    _account: &AccountId,
    _amount: Balance,
  ) -> Result<(), DispatchError> {
    Ok(())
  }
}

impl<AccountId> MintDistributionHook<AccountId> for () {}

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AccountId> {
  fn create_asset(asset_id: u32) -> DispatchResult;
  fn mint_native(to: &AccountId, amount: Balance) -> DispatchResult;
  fn mint_local(asset_id: u32, to: &AccountId, amount: Balance) -> DispatchResult;
}

#[frame::pallet]
pub mod pallet {
  use super::WeightInfo;
  use super::*;

  #[pallet::config]
  pub trait Config: frame_system::Config {
    /// Asset management interface for local assets
    type Assets: Inspect<Self::AccountId, AssetId = u32, Balance = Balance>
      + Mutate<Self::AccountId, AssetId = u32, Balance = Balance>;

    /// Currency interface for native asset
    type Currency: NativeMutate<Self::AccountId, Balance = Balance>
      + NativeInspect<Self::AccountId, Balance = Balance>;

    /// Origin that can perform governance operations
    type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

    /// Balance type
    type Balance: Parameter + Member + AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen;

    /// Pallet ID for fee collection
    type PalletId: Get<PalletId>;

    /// Treasury account for TOL distribution
    type TreasuryAccount: Get<Self::AccountId>;

    /// Initial price for token minting
    type InitialPrice: Get<Self::Balance>;

    /// Slope parameter for linear price ceiling
    type SlopeParameter: Get<Self::Balance>;

    /// Precision for mathematical calculations
    type Precision: Get<Self::Balance>;

    /// Resolves the output sink per minting curve
    type MintOutputResolver: MintOutputResolver<Self::AccountId>;

    /// Distribution ratio for user allocation (1/3)
    type UserAllocationRatio: Get<Perbill>;

    /// Runtime glue hook executed on curve creation
    type DomainGlueHook: DomainGlueHook;

    /// Optional hook for deterministic mint-failure injection in tests
    type MintDistributionHook: MintDistributionHook<Self::AccountId>;

    /// Weight information
    type WeightInfo: WeightInfo;

    /// Helper for benchmarking
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper: BenchmarkHelper<Self::AccountId>;
  }

  #[pallet::pallet]
  #[pallet::storage_version(STORAGE_VERSION)]
  pub struct Pallet<T>(_);

  /// The current storage version.
  const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

  #[pallet::hooks]
  impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    #[cfg(feature = "try-runtime")]
    fn try_state(_n: BlockNumberFor<T>) -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      Self::do_try_state()
    }
  }

  /// Balance type
  pub type Balance = u128;

  /// Price type for token minting
  pub type Price = Balance;

  /// Slope type for linear price ceiling
  pub type Slope = Balance;

  /// Curve configuration for token minting
  #[derive(Clone, Debug, Encode, Decode, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
  pub struct CurveConfig {
    /// Initial price of the token
    pub initial_price: Price,
    /// Slope parameter for linear price ceiling
    pub slope: Slope,
    /// Initial issuance at curve creation
    pub initial_issuance: Balance,
    /// Foreign asset used for minting
    pub foreign_asset: AssetKind,
    /// Native asset ID
    pub minted_asset: AssetKind,
  }

  /// Storage for token curves
  #[pallet::storage]
  pub type TokenCurves<T: Config> = StorageMap<_, Blake2_128Concat, AssetKind, CurveConfig>;

  /// Cumulative native tokens minted through the bonding curve since genesis.
  #[pallet::storage]
  pub type TotalNativeMinted<T: Config> = StorageValue<_, Balance, ValueQuery>;

  #[pallet::event]
  #[pallet::generate_deposit(pub(super) fn deposit_event)]
  pub enum Event<T: Config> {
    /// Curve created for a token
    CurveCreated {
      token_asset: AssetKind,
      initial_price: Price,
      slope: Slope,
      foreign_asset: AssetKind,
    },
    /// Zap allocation distributed
    ZapAllocationDistributed {
      token_asset: AssetKind,
      user_allocation: Balance,
      zap_allocation: Balance,
      foreign_amount: Balance,
    },
  }

  #[pallet::error]
  pub enum Error<T> {
    /// Curve already exists for this token
    CurveAlreadyExists,
    /// No curve exists for this token
    NoCurveExists,
    /// Insufficient balance for operation
    InsufficientBalance,
    /// Exceeds maximum supply
    ExceedsMaxSupply,
    /// Arithmetic overflow occurred
    ArithmeticOverflow,
    /// Invalid parameters provided
    InvalidParameters,
    /// Asset kind does not exist in the live asset registry
    AssetDoesNotExist,
    /// Foreign asset does not match the curve collateral configuration
    InvalidForeignAsset,
    /// Zero amount not allowed
    ZeroAmount,
  }

  #[pallet::view_functions]
  impl<T: Config> Pallet<T> {
    /// Derived total of native tokens destroyed since genesis.
    ///
    /// `TotalBurned = TotalNativeMinted + GenesisIssuance − CurrentIssuance`
    ///
    /// `GenesisIssuance` is taken from the native curve's `initial_issuance`
    /// recorded at genesis, capturing any pre-curve token distribution.
    /// Because the TMC ratchet is the only source of new native supply,
    /// any issuance deficit versus the cumulative minted+genesis baseline
    /// equals tokens destroyed by any mechanism.
    pub fn total_native_burned() -> Balance {
      let genesis_issuance = TokenCurves::<T>::get(AssetKind::Native)
        .map(|c| c.initial_issuance)
        .unwrap_or(0);
      let total_minted = TotalNativeMinted::<T>::get();
      let current_issuance: Balance = T::Currency::total_issuance().unique_saturated_into();
      let expected = genesis_issuance.saturating_add(total_minted);
      expected.saturating_sub(current_issuance)
    }
  }

  #[pallet::call]
  impl<T: Config> Pallet<T> {
    /// Create a new bonding curve for a token
    #[pallet::call_index(0)]
    #[pallet::weight(T::WeightInfo::create_curve())]
    #[transactional]
    pub fn create_curve(
      origin: OriginFor<T>,
      token_asset: AssetKind,
      foreign_asset: AssetKind,
      initial_price: Price,
      slope: Slope,
    ) -> DispatchResult {
      T::AdminOrigin::ensure_origin(origin)?;
      ensure!(
        !TokenCurves::<T>::contains_key(token_asset),
        Error::<T>::CurveAlreadyExists
      );
      ensure!(
        initial_price > Balance::from(0u32) || slope > Balance::from(0u32),
        Error::<T>::InvalidParameters
      );
      ensure!(token_asset != foreign_asset, Error::<T>::InvalidParameters);
      Self::ensure_asset_exists(token_asset)?;
      Self::ensure_asset_exists(foreign_asset)?;
      T::DomainGlueHook::on_curve_created(token_asset, foreign_asset)?;
      let initial_issuance: Balance = match token_asset {
        AssetKind::Native => T::Currency::total_issuance().unique_saturated_into(),
        AssetKind::Local(id) | AssetKind::Foreign(id) => {
          T::Assets::total_issuance(id).unique_saturated_into()
        }
      };
      let curve = CurveConfig {
        initial_price,
        slope,
        initial_issuance,
        foreign_asset,
        minted_asset: token_asset,
      };
      TokenCurves::<T>::insert(token_asset, curve);
      Self::deposit_event(Event::CurveCreated {
        token_asset,
        initial_price,
        slope,
        foreign_asset,
      });
      Ok(())
    }
  }

  impl<T: Config> Pallet<T> {
    /// Calculate current spot price
    pub(crate) fn calculate_spot_price(curve: &CurveConfig) -> Price {
      let total_issuance: u128 = match curve.minted_asset {
        AssetKind::Native => T::Currency::total_issuance().unique_saturated_into(),
        AssetKind::Local(id) | AssetKind::Foreign(id) => {
          T::Assets::total_issuance(id).unique_saturated_into()
        }
      };
      let effective_supply = total_issuance.saturating_sub(curve.initial_issuance);
      let slope_contribution = curve.slope.saturating_mul(effective_supply);
      let precision: u128 = T::Precision::get().unique_saturated_into();
      let normalized = slope_contribution / precision;
      curve.initial_price.saturating_add(normalized)
    }

    /// Check if a bonding curve exists for the given token asset
    pub fn has_curve(asset_id: AssetKind) -> bool {
      TokenCurves::<T>::contains_key(asset_id)
    }

    /// Get curve configuration
    pub fn get_curve(asset_id: AssetKind) -> Option<CurveConfig> {
      TokenCurves::<T>::get(asset_id)
    }

    /// Get the account ID of the TMC pallet
    pub fn account_id() -> T::AccountId {
      T::PalletId::get().into_account_truncating()
    }

    /// Calculate how much Native tokens the user receives for the foreign payment
    pub fn calculate_user_receives(
      token_asset: AssetKind,
      foreign_amount: Balance,
    ) -> Result<Balance, DispatchError> {
      let curve = TokenCurves::<T>::get(token_asset).ok_or(Error::<T>::NoCurveExists)?;
      let initial_price = curve.initial_price;
      let slope = curve.slope;
      let precision = T::Precision::get();
      // Handle zero slope (constant price)
      if slope.is_zero() {
        if initial_price.is_zero() {
          return Ok(Zero::zero());
        }
        // Linear projection: Cost = Price * Amount -> Amount = Cost / Price
        // Price is scaled by Precision: Cost = (Price_stored / Precision) * Amount
        // Amount = Cost * Precision / Price_stored
        let amount_val: u128 = foreign_amount.unique_saturated_into();
        let price_val: u128 = initial_price.unique_saturated_into();
        let precision_val: u128 = precision.unique_saturated_into();
        let result = U256::from(amount_val)
          .saturating_mul(U256::from(precision_val))
          .checked_div(U256::from(price_val))
          .unwrap_or(U256::zero());
        if result > U256::from(u128::MAX) {
          return Err(Error::<T>::ArithmeticOverflow.into());
        }
        return Ok(result.as_u128().unique_saturated_into());
      }
      // Use quadratic formula to solve for Delta S (amount to mint)
      // derived from Cost = Integral(P(s) ds)
      // Delta S = (sqrt((K*P)^2 + 2*m*K*Cost) - K*P) / m
      let p_current = Self::calculate_spot_price(&curve);
      // Convert to u128 explicitly to avoid U256::from ambiguity
      let k_val: u128 = precision.unique_saturated_into();
      let m_val: u128 = slope.unique_saturated_into();
      let p_val: u128 = p_current.unique_saturated_into();
      let cost_val: u128 = foreign_amount.unique_saturated_into();
      let k_u256 = U256::from(k_val);
      let m_u256 = U256::from(m_val);
      let p_u256 = U256::from(p_val);
      let cost_u256 = U256::from(cost_val);
      // K * P
      let kp = k_u256.saturating_mul(p_u256);
      // (K * P)^2
      let kp_sq = kp.saturating_mul(kp);
      // 2 * m * K^2 * Cost (scaled for precision)
      let two_m_k_cost = U256::from(2)
        .saturating_mul(m_u256)
        .saturating_mul(k_u256)
        .saturating_mul(k_u256)
        .saturating_mul(cost_u256);
      // Inside sqrt
      let inside_sqrt = kp_sq.saturating_add(two_m_k_cost);
      // Sqrt
      let sqrt_res = inside_sqrt.integer_sqrt();
      // Numerator: sqrt - KP
      if sqrt_res < kp {
        return Ok(Zero::zero());
      }
      let numerator = sqrt_res.saturating_sub(kp);
      // Result: Numerator / m
      let result_u256 = numerator
        .checked_div(m_u256)
        .ok_or(Error::<T>::ArithmeticOverflow)?;
      // Convert back to Balance (u128)
      if result_u256 > U256::from(u128::MAX) {
        return Err(Error::<T>::ArithmeticOverflow.into());
      }
      Ok(result_u256.as_u128().unique_saturated_into())
    }

    fn ensure_asset_exists(asset: AssetKind) -> Result<(), DispatchError> {
      match asset {
        AssetKind::Native => Ok(()),
        AssetKind::Local(id) | AssetKind::Foreign(id) => {
          ensure!(T::Assets::asset_exists(id), Error::<T>::AssetDoesNotExist);
          Ok(())
        }
      }
    }

    /// Execute mint through bonding curve with user/TOL distribution
    #[transactional]
    pub fn mint_with_distribution(
      who: &T::AccountId,
      token_asset: AssetKind,
      foreign_asset: AssetKind,
      foreign_amount: Balance,
    ) -> Result<Balance, DispatchError> {
      ensure!(foreign_amount > Balance::from(0u32), Error::<T>::ZeroAmount);
      let curve = TokenCurves::<T>::get(token_asset).ok_or(Error::<T>::NoCurveExists)?;
      ensure!(
        curve.foreign_asset == foreign_asset,
        Error::<T>::InvalidForeignAsset
      );
      Self::ensure_asset_exists(token_asset)?;
      Self::ensure_asset_exists(foreign_asset)?;
      let mint_amount = Self::calculate_user_receives(token_asset, foreign_amount)?;
      let output = T::MintOutputResolver::output_account(token_asset);
      match foreign_asset {
        AssetKind::Native => {
          T::Currency::transfer(who, &output, foreign_amount, Preservation::Expendable)?;
        }
        AssetKind::Local(id) | AssetKind::Foreign(id) => {
          T::Assets::transfer(id, who, &output, foreign_amount, Preservation::Expendable)?;
        }
      }
      let user_allocation = T::UserAllocationRatio::get().mul_floor(mint_amount);
      let zap_allocation = mint_amount.saturating_sub(user_allocation);
      match token_asset {
        AssetKind::Native => {
          T::MintDistributionHook::before_user_mint(token_asset, who, user_allocation)?;
          T::Currency::mint_into(who, user_allocation)?;
          T::MintDistributionHook::before_sink_mint(token_asset, &output, zap_allocation)?;
          T::Currency::mint_into(&output, zap_allocation)?;
          TotalNativeMinted::<T>::mutate(|acc| *acc = acc.saturating_add(mint_amount));
        }
        AssetKind::Local(id) | AssetKind::Foreign(id) => {
          T::MintDistributionHook::before_user_mint(token_asset, who, user_allocation)?;
          T::Assets::mint_into(id, who, user_allocation)?;
          T::MintDistributionHook::before_sink_mint(token_asset, &output, zap_allocation)?;
          T::Assets::mint_into(id, &output, zap_allocation)?;
        }
      }
      Self::deposit_event(Event::ZapAllocationDistributed {
        token_asset,
        user_allocation,
        zap_allocation,
        foreign_amount,
      });
      Ok(mint_amount)
    }

    #[cfg(feature = "try-runtime")]
    pub(crate) fn do_try_state() -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      use polkadot_sdk::sp_runtime::TryRuntimeError;
      let mut curve_iter = TokenCurves::<T>::iter();
      while let Some((asset_kind, curve)) = curve_iter.next() {
        // Invariant 1: Stored minted_asset matches the storage key
        if curve.minted_asset != asset_kind {
          return Err(TryRuntimeError::Other(
            "TokenCurves key does not match CurveConfig.minted_asset",
          ));
        }
        // Invariant 2: Current total issuance ≥ initial issuance (unidirectional minting)
        let total_issuance: Balance = match curve.minted_asset {
          AssetKind::Native => T::Currency::total_issuance().unique_saturated_into(),
          AssetKind::Local(id) | AssetKind::Foreign(id) => {
            T::Assets::total_issuance(id).unique_saturated_into()
          }
        };
        if total_issuance < curve.initial_issuance {
          return Err(TryRuntimeError::Other(
            "Total issuance is less than initial issuance (conservation violation)",
          ));
        }
        // Invariant 3: Spot price ≥ initial price (linear curve is monotonically increasing)
        let spot_price = Self::calculate_spot_price(&curve);
        if spot_price < curve.initial_price {
          return Err(TryRuntimeError::Other(
            "Spot price is below initial price (monotonicity violation)",
          ));
        }
      }
      Ok(())
    }
  }

  #[pallet::genesis_config]
  #[derive(frame::prelude::DefaultNoBound)]
  pub struct GenesisConfig<T: Config> {
    #[serde(skip)]
    pub _marker: core::marker::PhantomData<T>,
    /// Initial curves to create at genesis.
    /// Each entry: (minted_asset, collateral_asset, initial_price, slope)
    pub curves: alloc::vec::Vec<(AssetKind, AssetKind, Price, Slope)>,
  }

  #[pallet::genesis_build]
  impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
    fn build(&self) {
      frame_system::Pallet::<T>::inc_providers(&Pallet::<T>::account_id());
      for (minted_asset, collateral_asset, initial_price, slope) in &self.curves {
        let initial_issuance: Balance = match minted_asset {
          AssetKind::Native => T::Currency::total_issuance().unique_saturated_into(),
          AssetKind::Local(id) | AssetKind::Foreign(id) => {
            T::Assets::total_issuance(*id).unique_saturated_into()
          }
        };
        let curve = CurveConfig {
          initial_price: *initial_price,
          slope: *slope,
          initial_issuance,
          foreign_asset: *collateral_asset,
          minted_asset: *minted_asset,
        };
        TokenCurves::<T>::insert(*minted_asset, curve);
      }
    }
  }
}
