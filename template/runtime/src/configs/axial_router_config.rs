//! Axial Router pallet configuration for the parachain runtime.
//!
//! Configures the minimalist multi-token routing system optimized for TMC ecosystems
//! with Native-anchored routing and advanced fee processing.

use super::assets_config::AssetId as LocalAssetId;
use super::*;

use alloc::{boxed::Box, vec::Vec};
use codec::{Decode, Encode};
use polkadot_sdk::frame_support::pallet_prelude::Zero;
use polkadot_sdk::frame_support::traits::fungible::Inspect as NativeInspect;
use polkadot_sdk::frame_support::traits::{
  Currency, Get,
  fungibles::{Inspect as FungiblesInspect, Mutate as FungiblesMutate},
};

use polkadot_sdk::pallet_asset_conversion::PoolLocator;
use polkadot_sdk::sp_core::U256;
use polkadot_sdk::sp_runtime::{DispatchError, Perbill, Saturating, TokenError};
use polkadot_sdk::*;

use crate::{AssetConversion, RuntimeOrigin};
use primitives::{AssetKind, ecosystem};

parameter_types! {
  /// Router fee as Perbill (derived from ecosystem constant 50bps = 0.5%)
  pub const AxialRouterFee: Perbill = ecosystem::params::AXIAL_ROUTER_FEE;
  /// Maximum governance-settable router fee for the current launch line
  pub const AxialRouterMaxFee: Perbill = ecosystem::params::MAX_AXIAL_ROUTER_FEE;
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

pub struct LiquidityActorAccount;

impl polkadot_sdk::frame_support::traits::Get<AccountId> for LiquidityActorAccount {
  fn get() -> AccountId {
    pallet_aaa::Pallet::<crate::Runtime>::sovereign_account_id_system(
      primitives::ecosystem::aaa_ids::LIQUIDITY_ACTOR_AAA_ID,
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

  pub fn ensure_lp_asset_namespace() {
    let lp_namespace_start = primitives::assets::TYPE_LP | 1;
    let current_next_lp = pallet_asset_conversion::NextPoolAssetId::<Runtime>::get().unwrap_or(0);
    if current_next_lp < lp_namespace_start {
      pallet_asset_conversion::NextPoolAssetId::<Runtime>::put(lp_namespace_start);
    }
  }

  pub fn native_staking_liquidity_pool_read_model()
  -> Option<(LocalAssetId, Balance, Balance, Balance)> {
    let native_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    let staked_asset_id = crate::Staking::staked_asset_id(native_asset_id)?;
    let base_asset = AssetKind::Local(native_asset_id);
    let staked_asset = AssetKind::Local(staked_asset_id);
    let pool_id = <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(
      &base_asset,
      &staked_asset,
    )
    .ok()?;
    let pool = pallet_asset_conversion::Pools::<Runtime>::get(pool_id)?;
    let pool_account = <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_address(
      &base_asset,
      &staked_asset,
    )
    .ok()?;
    let reserve_native = Self::asset_balance(base_asset, &pool_account);
    let reserve_staked = Self::asset_balance(staked_asset, &pool_account);
    let lp_total_issuance =
      <Runtime as pallet_asset_conversion::Config>::PoolAssets::total_issuance(pool.lp_token);
    Some((
      pool.lp_token,
      reserve_native,
      reserve_staked,
      lp_total_issuance,
    ))
  }

  pub fn donate_balanced_liquidity(
    donor: &AccountId,
    asset1: AssetKind,
    asset2: AssetKind,
    amount1: Balance,
    amount2: Balance,
    max_ratio_error: Perbill,
  ) -> Result<(), DispatchError> {
    if amount1.is_zero() || amount2.is_zero() || asset1 == asset2 {
      return Err(DispatchError::Other("InvalidDonation"));
    }
    let pool_account =
      <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_address(&asset1, &asset2)
        .map_err(|_| DispatchError::Other("DonationPoolUnavailable"))?;
    let reserve1 = Self::asset_balance(asset1, &pool_account);
    let reserve2 = Self::asset_balance(asset2, &pool_account);
    if reserve1.is_zero() || reserve2.is_zero() {
      return Err(DispatchError::Other("DonationPoolEmpty"));
    }
    Self::ensure_ratio_within_tolerance(amount1, amount2, reserve1, reserve2, max_ratio_error)?;
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      if let Err(error) = Self::transfer_asset(asset1, donor, &pool_account, amount1) {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
      }
      if let Err(error) = Self::transfer_asset(asset2, donor, &pool_account, amount2) {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
      }
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
    })
  }

  pub fn compound_native_nomination_reward_to_locked_lp(
    account: &AccountId,
    operator: &AccountId,
    total_native: Balance,
  ) -> Result<Balance, DispatchError> {
    if total_native.is_zero() {
      return Err(DispatchError::Other("InvalidCompoundAmount"));
    }
    let native_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    let staked_asset_id = crate::Staking::staked_asset_id(native_asset_id)
      .ok_or(DispatchError::Other("StakedAssetUnavailable"))?;
    let base_asset = AssetKind::Local(native_asset_id);
    let staked_asset = AssetKind::Local(staked_asset_id);
    let pool_id = <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(
      &base_asset,
      &staked_asset,
    )
    .map_err(|_| DispatchError::Other("NativeStakingAmmUnavailable"))?;
    let pool = pallet_asset_conversion::Pools::<Runtime>::get(pool_id)
      .ok_or(DispatchError::Other("NativeStakingAmmUnavailable"))?;
    let pool_account = <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_address(
      &base_asset,
      &staked_asset,
    )
    .map_err(|_| DispatchError::Other("NativeStakingAmmUnavailable"))?;
    let reserve_native = Self::asset_balance(base_asset, &pool_account);
    let reserve_staked = Self::asset_balance(staked_asset, &pool_account);
    let staking_pool = pallet_staking::Pools::<Runtime>::get(native_asset_id)
      .ok_or(DispatchError::Other("NativeStakingPoolUnavailable"))?;
    if reserve_native.is_zero()
      || reserve_staked.is_zero()
      || staking_pool.accounted_balance.is_zero()
      || staking_pool.total_shares.is_zero()
    {
      return Err(DispatchError::Other("NativeStakingAmmEmpty"));
    }
    let stake_amount = Self::native_stake_amount_for_balanced_donation(
      total_native,
      reserve_native,
      reserve_staked,
      staking_pool.accounted_balance,
      staking_pool.total_shares,
    )?;
    let native_liquidity = total_native
      .checked_sub(stake_amount)
      .ok_or(DispatchError::Other("CompoundAmountOverflow"))?;
    if stake_amount.is_zero() || native_liquidity.is_zero() {
      return Err(DispatchError::Other("CompoundAmountTooSmall"));
    }
    let staked_before = Self::asset_balance(staked_asset, account);
    let lp_before =
      <Runtime as pallet_asset_conversion::Config>::PoolAssets::balance(pool.lp_token, account);
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      if let Err(error) =
        crate::Staking::stake_native(RuntimeOrigin::signed(account.clone()), stake_amount)
      {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
      }
      let staked_after = Self::asset_balance(staked_asset, account);
      let staked_liquidity = staked_after.saturating_sub(staked_before);
      if staked_liquidity.is_zero() {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          DispatchError::Other("CompoundAmountTooSmall"),
        ));
      }
      if let Err(error) = AssetConversion::add_liquidity(
        RuntimeOrigin::signed(account.clone()),
        Box::new(base_asset),
        Box::new(staked_asset),
        native_liquidity,
        staked_liquidity,
        1,
        1,
        account.clone(),
      ) {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
      }
      let lp_after =
        <Runtime as pallet_asset_conversion::Config>::PoolAssets::balance(pool.lp_token, account);
      let lp_minted = lp_after.saturating_sub(lp_before);
      if lp_minted.is_zero() {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          DispatchError::Other("CompoundLpNotMinted"),
        ));
      }
      if let Err(error) = crate::Staking::lock_native_lp_for_collator(
        RuntimeOrigin::signed(account.clone()),
        pool.lp_token,
        lp_minted,
        operator.clone(),
      ) {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
      }
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(lp_minted))
    })
  }

  pub fn donate_native_staking_liquidity_from_ntve(
    donor: &AccountId,
    total_native: Balance,
    max_ratio_error: Perbill,
  ) -> Result<(Balance, Balance), DispatchError> {
    if total_native.is_zero() {
      return Err(DispatchError::Other("InvalidDonation"));
    }
    let native_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    let staked_asset_id = crate::Staking::staked_asset_id(native_asset_id)
      .ok_or(DispatchError::Other("StakedAssetUnavailable"))?;
    let base_asset = AssetKind::Local(native_asset_id);
    let staked_asset = AssetKind::Local(staked_asset_id);
    let pool_account = <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_address(
      &base_asset,
      &staked_asset,
    )
    .map_err(|_| DispatchError::Other("DonationPoolUnavailable"))?;
    let reserve_native = Self::asset_balance(base_asset, &pool_account);
    let reserve_staked = Self::asset_balance(staked_asset, &pool_account);
    let staking_pool = pallet_staking::Pools::<Runtime>::get(native_asset_id)
      .ok_or(DispatchError::Other("NativeStakingPoolUnavailable"))?;
    if reserve_native.is_zero()
      || reserve_staked.is_zero()
      || staking_pool.accounted_balance.is_zero()
      || staking_pool.total_shares.is_zero()
    {
      return Err(DispatchError::Other("DonationPoolEmpty"));
    }
    let stake_amount = Self::native_stake_amount_for_balanced_donation(
      total_native,
      reserve_native,
      reserve_staked,
      staking_pool.accounted_balance,
      staking_pool.total_shares,
    )?;
    let native_donation = total_native
      .checked_sub(stake_amount)
      .ok_or(DispatchError::Other("DonationAmountOverflow"))?;
    if stake_amount.is_zero() || native_donation.is_zero() {
      return Err(DispatchError::Other("DonationAmountTooSmall"));
    }
    let staked_before = Self::asset_balance(staked_asset, donor);
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      if let Err(error) =
        crate::Staking::stake_native(RuntimeOrigin::signed(donor.clone()), stake_amount)
      {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
      }
      let staked_after = Self::asset_balance(staked_asset, donor);
      let staked_donation = staked_after.saturating_sub(staked_before);
      if staked_donation.is_zero() {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          DispatchError::Other("DonationAmountTooSmall"),
        ));
      }
      if let Err(error) = Self::donate_balanced_liquidity(
        donor,
        base_asset,
        staked_asset,
        native_donation,
        staked_donation,
        max_ratio_error,
      ) {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error));
      }
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok((
        native_donation,
        staked_donation,
      )))
    })
  }

  fn native_stake_amount_for_balanced_donation(
    total_native: Balance,
    reserve_native: Balance,
    reserve_staked: Balance,
    staking_accounted_balance: Balance,
    staking_total_shares: Balance,
  ) -> Result<Balance, DispatchError> {
    let numerator = U256::from(reserve_staked)
      .saturating_mul(U256::from(total_native))
      .saturating_mul(U256::from(staking_accounted_balance));
    let denominator = U256::from(reserve_staked)
      .saturating_mul(U256::from(staking_accounted_balance))
      .saturating_add(U256::from(reserve_native).saturating_mul(U256::from(staking_total_shares)));
    if denominator.is_zero() {
      return Err(DispatchError::Other("DonationAmountOverflow"));
    }
    numerator
      .checked_div(denominator)
      .ok_or(DispatchError::Other("DonationAmountOverflow"))?
      .try_into()
      .map_err(|_| DispatchError::Other("DonationAmountOverflow"))
  }

  fn ensure_ratio_within_tolerance(
    amount1: Balance,
    amount2: Balance,
    reserve1: Balance,
    reserve2: Balance,
    max_ratio_error: Perbill,
  ) -> Result<(), DispatchError> {
    let left = U256::from(amount1).saturating_mul(U256::from(reserve2));
    let right = U256::from(amount2).saturating_mul(U256::from(reserve1));
    let difference = left.abs_diff(right);
    let reference = left.max(right);
    let allowed = max_ratio_error * reference;
    if difference > allowed {
      return Err(DispatchError::Other("DonationRatioExceeded"));
    }
    Ok(())
  }

  fn asset_balance(asset: AssetKind, account: &AccountId) -> Balance {
    match asset {
      AssetKind::Native => <Balances as NativeInspect<AccountId>>::balance(account),
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::balance(id, account)
      }
    }
  }

  fn transfer_asset(
    asset: AssetKind,
    from: &AccountId,
    to: &AccountId,
    amount: Balance,
  ) -> Result<(), DispatchError> {
    match asset {
      AssetKind::Native => <Balances as Currency<AccountId>>::transfer(
        from,
        to,
        amount,
        polkadot_sdk::frame_support::traits::ExistenceRequirement::AllowDeath,
      ),
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesMutate<AccountId>>::transfer(
          id,
          from,
          to,
          amount,
          polkadot_sdk::frame_support::traits::tokens::Preservation::Expendable,
        )
        .map(|_| ())
      }
    }
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

  fn calculate_recipient_receives(
    token_asset: AssetKind,
    foreign_amount: Balance,
  ) -> Result<Balance, sp_runtime::DispatchError> {
    let total_minted = pallet_tmc::Pallet::<T>::calculate_total_mint(token_asset, foreign_amount)?;
    Ok(<T as pallet_tmc::pallet::Config>::UserAllocationRatio::get().mul_floor(total_minted))
  }

  fn mint_with_distribution(
    who: &T::AccountId,
    recipient: &T::AccountId,
    token_asset: AssetKind,
    foreign_asset: AssetKind,
    foreign_amount: Balance,
  ) -> Result<Balance, sp_runtime::DispatchError> {
    let total_minted = pallet_tmc::Pallet::<T>::mint_with_distribution(
      who,
      recipient,
      token_asset,
      foreign_asset,
      foreign_amount,
    )?;
    Ok(<T as pallet_tmc::pallet::Config>::UserAllocationRatio::get().mul_floor(total_minted))
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
        // This is a bounded runtime safety check. Durable price-deviation analytics,
        // dashboards, and alert history belong in external indexers/operator tooling.
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
  type LiquidityActorAccount = LiquidityActorAccount;
  type Currency = Balances;
  type DefaultRouterFee = AxialRouterFee;
  type EmaHalfLife = AxialRouterEmaHalfLife;
  type FeeAdapter = FeeManagerImpl<Runtime>;
  type MaxPriceDeviation = AxialRouterMaxPriceDeviation;
  type MaxRouterFee = AxialRouterMaxFee;
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
    AssetConversionAdapter::ensure_lp_asset_namespace();
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
