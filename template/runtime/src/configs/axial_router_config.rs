//! Axial Router pallet configuration for the parachain runtime.
//!
//! Configures the minimalist multi-token routing system optimized for TMC ecosystems
//! with Native-anchored routing and advanced fee processing.

use super::*;

use alloc::{boxed::Box, vec::Vec};
use codec::{Decode, Encode};
use polkadot_sdk::frame_support::pallet_prelude::Zero;
use polkadot_sdk::frame_support::traits::fungible::Inspect as NativeInspect;
use polkadot_sdk::frame_support::traits::{Currency, Get, fungibles::Inspect as FungiblesInspect};

use polkadot_sdk::sp_runtime::{DispatchError, Perbill, Saturating, TokenError};
use polkadot_sdk::*;

use crate::{AssetConversion, RuntimeOrigin};
use primitives::{AssetKind, ecosystem};

parameter_types! {
  /// Router fee as Perbill (derived from ecosystem constant 50bps = 0.5%)
  pub const AxialRouterFee: Perbill = ecosystem::params::AXIAL_ROUTER_FEE;
  /// Native asset (AssetKind::Native)
  pub const NativeAsset: AssetKind = AssetKind::Native;
  /// Pallet ID for the Axial router
  pub const AxialRouterPalletId: PalletId = PalletId(*ecosystem::pallet_ids::AXIAL_ROUTER_PALLET_ID);
  /// Minimum foreign amount for swapping (threshold for buffer processing)
  pub const MinSwapForeign: Balance = ecosystem::params::MIN_SWAP_FOREIGN;
  /// Precision constant for all calculations
  pub const AxialRouterPrecision: Balance = ecosystem::params::PRECISION;
  /// EMA oracle half-life in blocks
  pub const AxialRouterEmaHalfLife: u32 = ecosystem::params::EMA_HALF_LIFE_BLOCKS;
  /// Maximum price deviation allowed
  pub const AxialRouterMaxPriceDeviation: Perbill = ecosystem::params::MAX_PRICE_DEVIATION;
}

/// The sovereign account of the Burning Manager System AAA (aaa_id=0).
/// Address is deterministic from `(AaaPalletId, b"system", 0)` — see `ecosystem::aaa_ids`.
pub struct BurningManagerAccount;

impl polkadot_sdk::frame_support::traits::Get<AccountId> for BurningManagerAccount {
  fn get() -> AccountId {
    pallet_aaa::Pallet::<crate::Runtime>::sovereign_account_id_system(
      primitives::ecosystem::aaa_ids::BURNING_MANAGER_AAA_ID,
    )
  }
}

pub struct ZapManagerAccount;

impl polkadot_sdk::frame_support::traits::Get<AccountId> for ZapManagerAccount {
  fn get() -> AccountId {
    pallet_aaa::Pallet::<crate::Runtime>::sovereign_account_id_system(
      primitives::ecosystem::aaa_ids::ZAP_MANAGER_AAA_ID,
    )
  }
}

/// TMC pallet adapter for Axial Router integration
pub struct TmcPalletAdapter<T: pallet_axial_router::pallet::Config>(core::marker::PhantomData<T>);

/// Price oracle implementation for manipulation-resistant pricing
pub struct PriceOracleImpl<T: pallet_axial_router::pallet::Config>(core::marker::PhantomData<T>);

/// Token-driven fee manager implementation with account-based coordination
pub struct FeeManagerImpl<T: pallet_axial_router::pallet::Config>(core::marker::PhantomData<T>);

pub struct AssetConversionAdapter;

impl AssetConversionAdapter {
  pub fn encode_pool_id(pool: (AssetKind, AssetKind)) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    let encoded = pool.encode();
    if encoded.len() <= 32 {
      bytes[..encoded.len()].copy_from_slice(&encoded);
    }
    bytes
  }

  pub fn decode_pool_id(pool_id: [u8; 32]) -> Option<(AssetKind, AssetKind)> {
    let mut slice = &pool_id[..];
    <(AssetKind, AssetKind)>::decode(&mut slice).ok()
  }
}

impl pallet_axial_router::AssetConversionApi<AccountId, Balance> for AssetConversionAdapter {
  fn get_pool_id(asset_a: AssetKind, asset_b: AssetKind) -> Option<(AssetKind, AssetKind)> {
    if asset_a == asset_b {
      return None;
    }
    if asset_a < asset_b {
      Some((asset_a, asset_b))
    } else {
      Some((asset_b, asset_a))
    }
  }

  fn get_pool_reserves(pool_id: (AssetKind, AssetKind)) -> Option<(Balance, Balance)> {
    let (asset_a, asset_b) = pool_id;
    AssetConversion::get_reserves(asset_a, asset_b).ok()
  }

  fn quote_price_exact_tokens_for_tokens(
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_in: Balance,
    include_fee: bool,
  ) -> Option<Balance> {
    AssetConversion::quote_price_exact_tokens_for_tokens(
      asset_in,
      asset_out,
      amount_in,
      include_fee,
    )
  }

  fn swap_exact_tokens_for_tokens(
    who: AccountId,
    path: Vec<AssetKind>,
    amount_in: Balance,
    min_amount_out: Balance,
    recipient: AccountId,
    keep_alive: bool,
  ) -> Result<Balance, sp_runtime::DispatchError> {
    if path.len() < 2usize {
      return Err(DispatchError::Other("Invalid asset path"));
    }
    // Get target asset and snapshot balance before swap
    let target_asset = *path.last().unwrap();
    // Snapshot recipient balance before swap
    let balance_before = match target_asset {
      AssetKind::Native => <Balances as NativeInspect<AccountId>>::balance(&recipient),
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::balance(id, &recipient)
      }
    };
    // Convert path from RouterAssetKind to AssetKind and box it
    let boxed_path: Vec<Box<AssetKind>> = path.iter().cloned().map(Box::new).collect();
    let origin = RuntimeOrigin::signed(who.clone());
    AssetConversion::swap_exact_tokens_for_tokens(
      origin,
      boxed_path,
      amount_in,
      min_amount_out,
      recipient.clone(),
      keep_alive,
    )?;
    // Snapshot recipient balance after swap and calculate actual amount received
    let balance_after = match target_asset {
      AssetKind::Native => <Balances as NativeInspect<AccountId>>::balance(&recipient),
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::balance(id, &recipient)
      }
    };
    let actual_amount_out = balance_after.saturating_sub(balance_before);
    // Return actual amount received instead of calculated quote
    Ok(actual_amount_out)
  }
}

impl<T> pallet_axial_router::TmcInterface<T::AccountId, Balance> for TmcPalletAdapter<T>
where
  T: pallet_axial_router::pallet::Config + pallet_tmc::pallet::Config<Balance = Balance>,
{
  fn has_curve(asset: AssetKind) -> bool {
    pallet_tmc::Pallet::<T>::has_curve(asset)
  }

  fn supports_collateral(token_asset: AssetKind, foreign_asset: AssetKind) -> bool {
    pallet_tmc::Pallet::<T>::get_curve(token_asset)
      .map(|curve| curve.foreign_asset == foreign_asset)
      .unwrap_or(false)
  }

  fn calculate_user_receives(
    token_asset: AssetKind,
    foreign_amount: Balance,
  ) -> Result<Balance, sp_runtime::DispatchError> {
    pallet_tmc::Pallet::<T>::calculate_user_receives(token_asset, foreign_amount)
  }

  fn mint_with_distribution(
    who: &T::AccountId,
    token_asset: AssetKind,
    foreign_asset: AssetKind,
    foreign_amount: Balance,
  ) -> Result<Balance, sp_runtime::DispatchError> {
    pallet_tmc::Pallet::<T>::mint_with_distribution(who, token_asset, foreign_asset, foreign_amount)
  }
}

impl<T: pallet_axial_router::pallet::Config> pallet_axial_router::PriceOracle<Balance>
  for PriceOracleImpl<T>
{
  fn update_ema_price(
    asset_in: AssetKind,
    asset_out: AssetKind,
    price: Balance,
  ) -> Result<(), sp_runtime::DispatchError> {
    let ema_half_life = T::EmaHalfLife::get();
    let current_block = polkadot_sdk::frame_system::Pallet::<T>::block_number();
    let last_update = pallet_axial_router::EmaLastUpdate::<T>::get(asset_in, asset_out);
    let previous_ema_price = pallet_axial_router::EmaPrices::<T>::get(asset_in, asset_out);
    let new_ema_price = if previous_ema_price.is_zero() {
      price
    } else {
      // Time-weighted alpha: elapsed blocks increase EMA responsiveness
      let elapsed: u32 = current_block
        .saturating_sub(last_update)
        .try_into()
        .unwrap_or(u32::MAX);
      let effective_elapsed = elapsed.max(1);
      let alpha = polkadot_sdk::sp_runtime::Perbill::from_rational(
        effective_elapsed,
        ema_half_life.saturating_add(effective_elapsed),
      );
      let ema_part1 = alpha.mul_floor(price);
      let ema_part2 = (polkadot_sdk::sp_runtime::Perbill::from_percent(100) - alpha)
        .mul_floor(previous_ema_price);
      ema_part1 + ema_part2
    };
    pallet_axial_router::EmaPrices::<T>::insert(asset_in, asset_out, new_ema_price);
    pallet_axial_router::EmaLastUpdate::<T>::insert(asset_in, asset_out, current_block);
    Ok(())
  }

  fn get_ema_price(asset_in: AssetKind, asset_out: AssetKind) -> Option<Balance> {
    Some(pallet_axial_router::EmaPrices::<T>::get(
      asset_in, asset_out,
    ))
  }

  fn validate_price_deviation(
    asset_in: AssetKind,
    asset_out: AssetKind,
    current_price: Balance,
  ) -> Result<(), sp_runtime::DispatchError> {
    let max_price_deviation = T::MaxPriceDeviation::get();
    if let Some(ema_price) = Self::get_ema_price(asset_in, asset_out) {
      if ema_price.is_zero() {
        return Ok(()); // No EMA data yet, skip validation
      }
      // Calculate price deviation
      let deviation = if current_price > ema_price {
        polkadot_sdk::sp_runtime::Perbill::from_rational(current_price - ema_price, ema_price)
      } else {
        polkadot_sdk::sp_runtime::Perbill::from_rational(ema_price - current_price, ema_price)
      };
      if deviation > max_price_deviation {
        // Price deviation events are handled by the Axial Router pallet's monitoring system
        // The deviation is logged through standard error mechanisms
        return Err(DispatchError::Other("Price deviation exceeded"));
      }
    }
    Ok(())
  }
}

impl pallet_axial_router::FeeRoutingAdapter<AccountId, Balance> for FeeManagerImpl<Runtime> {
  fn route_fee(who: &AccountId, asset: AssetKind, amount: Balance) -> sp_runtime::DispatchResult {
    let burning_manager_account = BurningManagerAccount::get();
    match asset {
      AssetKind::Native => {
        Balances::transfer(
          who,
          &burning_manager_account,
          amount,
          polkadot_sdk::frame_support::traits::tokens::ExistenceRequirement::KeepAlive,
        )
        .map_err(|_| DispatchError::Token(TokenError::FundsUnavailable))?;
      }
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        use polkadot_sdk::frame_support::traits::fungibles::Mutate;
        <pallet_assets::Pallet<Runtime> as Mutate<AccountId>>::transfer(
          id,
          who,
          &burning_manager_account,
          amount,
          polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
        )
        .map_err(|_| DispatchError::Token(TokenError::FundsUnavailable))?;
      }
    }
    <RuntimeAddressEventIngress as AddressEventIngress>::on_inbound_with_source(
      &burning_manager_account,
      asset,
      amount,
      who,
    );
    Ok(())
  }
}

impl pallet_axial_router::pallet::Config for Runtime {
  type AdminOrigin = frame_system::EnsureRoot<AccountId>;
  type AssetConversion = AssetConversionAdapter;
  type Assets = pallet_assets::Pallet<Runtime>;
  type BurningManagerAccount = BurningManagerAccount;
  type ZapManagerAccount = ZapManagerAccount;
  type Currency = Balances;
  type DefaultRouterFee = AxialRouterFee;
  type EmaHalfLife = AxialRouterEmaHalfLife;
  type FeeAdapter = FeeManagerImpl<Runtime>;
  type MaxPriceDeviation = AxialRouterMaxPriceDeviation;
  type MaxTrackedAssets = ConstU32<64>;
  type MinSwapForeign = MinSwapForeign;
  type NativeAsset = NativeAsset;
  type PalletId = AxialRouterPalletId;
  type Precision = AxialRouterPrecision;
  type PriceOracle = PriceOracleImpl<Runtime>;
  type TmcPallet = TmcPalletAdapter<Runtime>;
  type WeightInfo = crate::weights::pallet_axial_router::SubstrateWeight<Runtime>;
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = RuntimeBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct RuntimeBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_axial_router::types::BenchmarkHelper<AssetKind, AccountId, Balance>
  for RuntimeBenchmarkHelper
{
  fn create_asset(asset: AssetKind) -> polkadot_sdk::sp_runtime::DispatchResult {
    if let AssetKind::Local(id) | AssetKind::Foreign(id) = asset {
      if !<pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::asset_exists(id) {
        let _ = pallet_assets::Pallet::<Runtime>::force_create(
          RuntimeOrigin::root(),
          id,
          polkadot_sdk::sp_runtime::MultiAddress::Id(BurningManagerAccount::get()),
          true,
          1,
        );
      }
    }
    Ok(())
  }

  fn mint_asset(
    asset: AssetKind,
    to: &AccountId,
    amount: Balance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    match asset {
      AssetKind::Native => {
        let _ = <Balances as Currency<AccountId>>::deposit_creating(to, amount);
      }
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        use polkadot_sdk::frame_support::traits::fungibles::Mutate;
        <pallet_assets::Pallet<Runtime> as Mutate<AccountId>>::mint_into(id, to, amount)?;
      }
    }
    Ok(())
  }

  fn create_pool(asset1: AssetKind, asset2: AssetKind) -> polkadot_sdk::sp_runtime::DispatchResult {
    let creator = BurningManagerAccount::get();
    let _ =
      <Balances as Currency<AccountId>>::deposit_creating(&creator, 1_000_000_000_000_000_000);
    AssetConversion::create_pool(
      RuntimeOrigin::signed(creator),
      Box::new(asset1),
      Box::new(asset2),
    )?;
    Ok(())
  }

  fn add_liquidity(
    who: &AccountId,
    asset1: AssetKind,
    asset2: AssetKind,
    amount1: Balance,
    amount2: Balance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    AssetConversion::add_liquidity(
      RuntimeOrigin::signed(who.clone()),
      Box::new(asset1),
      Box::new(asset2),
      amount1,
      amount2,
      0,
      0,
      who.clone(),
    )?;
    Ok(())
  }
}
