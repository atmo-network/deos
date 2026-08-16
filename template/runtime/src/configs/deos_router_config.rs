//! DEOS Router pallet configuration for the parachain runtime.
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
use polkadot_sdk::sp_runtime::{DispatchError, Perbill, TokenError, traits::AccountIdConversion};
use polkadot_sdk::*;

use crate::{AssetConversion, RuntimeOrigin};
use primitives::{AssetKind, ecosystem};

#[cfg(test)]
std::thread_local! {
  static FAIL_AFTER_XYK_EXECUTION_AT: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
  static XYK_EXECUTION_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn set_fail_after_xyk_execution_at(index: Option<usize>) {
  FAIL_AFTER_XYK_EXECUTION_AT.with(|value| value.set(index));
  XYK_EXECUTION_COUNT.with(|value| value.set(0));
}

#[cfg(test)]
fn fail_after_xyk_execution() -> bool {
  let index = XYK_EXECUTION_COUNT.with(|value| {
    let index = value.get();
    value.set(index.saturating_add(1));
    index
  });
  FAIL_AFTER_XYK_EXECUTION_AT.with(|value| value.get() == Some(index))
}

parameter_types! {
  /// Router fee as Perbill (derived from ecosystem constant 50bps = 0.5%)
  pub const DeosRouterFee: Perbill = ecosystem::params::DEOS_ROUTER_FEE;
  /// Maximum governance-settable router fee for the current launch line
  pub const DeosRouterMaxFee: Perbill = ecosystem::params::MAX_DEOS_ROUTER_FEE;
  /// Maximum bounded LP reverse-index entries.
  pub const DeosRouterMaxLpPairs: u32 = ecosystem::params::MAX_ROUTER_LP_PAIRS;
  /// Native asset (AssetKind::Native)
  pub const NativeAsset: AssetKind = AssetKind::Native;
  /// Pallet ID for the DEOS router
  pub const RouterPalletId: PalletId = PalletId(*ecosystem::pallet_ids::ROUTER_PALLET_ID);
  /// Minimum foreign amount for swapping (threshold for buffer processing)
  pub const MinSwapForeign: Balance = ecosystem::params::MIN_SWAP_FOREIGN;
  /// Precision constant for all calculations
  pub const DeosRouterPrecision: Balance = ecosystem::params::PRECISION;
  /// EMA oracle half-life in blocks
  pub const DeosRouterEmaHalfLife: u32 = ecosystem::params::EMA_HALF_LIFE_BLOCKS;
  /// Maximum price deviation allowed
  pub const DeosRouterMaxPriceDeviation: Perbill = ecosystem::params::MAX_PRICE_DEVIATION;
}

/// The sovereign account of the Burn Actor (actor_id=0).
/// Address is deterministic from `(ActorsPalletId, b"system", 0)` — see `ecosystem::actor_ids`.
pub struct BurnActorAccount;

impl polkadot_sdk::frame_support::traits::Get<AccountId> for BurnActorAccount {
  fn get() -> AccountId {
    pallet_deos_actors::Pallet::<crate::Runtime>::sovereign_account_id_system(
      primitives::ecosystem::actor_ids::BURN_ACTOR_ID,
    )
  }
}

pub struct LiquidityActorAccount;

impl polkadot_sdk::frame_support::traits::Get<AccountId> for LiquidityActorAccount {
  fn get() -> AccountId {
    pallet_deos_actors::Pallet::<crate::Runtime>::sovereign_account_id_system(
      primitives::ecosystem::actor_ids::LIQUIDITY_ACTOR_ACTORS_ID,
    )
  }
}

/// TMC pallet adapter for DEOS Router integration
pub struct TmcPalletAdapter<T: pallet_deos_router::pallet::Config>(core::marker::PhantomData<T>);

/// Price-observation implementation for local deviation checks
pub struct PriceOracleImpl<T: pallet_deos_router::pallet::Config>(core::marker::PhantomData<T>);

/// Token-driven fee manager implementation with account-based coordination
pub struct FeeManagerImpl<T: pallet_deos_router::pallet::Config>(core::marker::PhantomData<T>);

fn router_adapter_failure(
  error: DispatchError,
  failure_class: pallet_deos_router::RouterFailureClass,
  retry_disposition: pallet_deos_router::RetryDisposition,
) -> pallet_deos_router::AdapterFailure {
  pallet_deos_router::AdapterFailure::new(error, failure_class, retry_disposition)
}

fn actor_ingress_failure(
  failure: pallet_deos_actors::IngressFailure,
) -> pallet_deos_router::AdapterFailure {
  let retry = match failure.retry {
    pallet_deos_actors::RetryClass::Permanent => pallet_deos_router::RetryDisposition::Permanent,
    pallet_deos_actors::RetryClass::Temporary => pallet_deos_router::RetryDisposition::RetryLater,
  };
  router_adapter_failure(
    failure.error,
    pallet_deos_router::RouterFailureClass::IngressRejected,
    retry,
  )
}

pub(crate) fn market_execution_failure(error: DispatchError) -> pallet_deos_router::AdapterFailure {
  use pallet_asset_conversion::Error as MarketError;
  let retryable = [
    MarketError::<Runtime>::ReserveLeftLessThanMinimal.into(),
    MarketError::<Runtime>::AmountOutTooHigh.into(),
    MarketError::<Runtime>::PoolNotFound.into(),
    MarketError::<Runtime>::ProvidedMinimumNotSufficientForSwap.into(),
    MarketError::<Runtime>::ProvidedMaximumNotSufficientForSwap.into(),
    MarketError::<Runtime>::BelowMinimum.into(),
    MarketError::<Runtime>::PoolEmpty.into(),
  ];
  if retryable.contains(&error) {
    router_adapter_failure(
      error,
      pallet_deos_router::RouterFailureClass::LiquidityUnavailable,
      pallet_deos_router::RetryDisposition::RetryLater,
    )
  } else {
    pallet_deos_router::AdapterFailure::unknown(error)
  }
}

fn oracle_publication_failure(error: DispatchError) -> pallet_deos_router::AdapterFailure {
  let retryable = error == pallet_oracle::Error::<Runtime>::FeedPaused.into();
  if error == pallet_deos_actors::Error::<Runtime>::DirtyObservationCapacityExceeded.into() {
    return router_adapter_failure(
      error,
      pallet_deos_router::RouterFailureClass::IngressRejected,
      pallet_deos_router::RetryDisposition::RetryLater,
    );
  }
  if error == pallet_deos_actors::Error::<Runtime>::DirtyObservationInvariant.into() {
    return router_adapter_failure(
      error,
      pallet_deos_router::RouterFailureClass::IngressRejected,
      pallet_deos_router::RetryDisposition::Permanent,
    );
  }
  router_adapter_failure(
    error,
    pallet_deos_router::RouterFailureClass::PublicationRejected,
    if retryable {
      pallet_deos_router::RetryDisposition::RetryLater
    } else {
      pallet_deos_router::RetryDisposition::Permanent
    },
  )
}

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
    Self::donate_balanced_liquidity_classified(
      donor,
      asset1,
      asset2,
      amount1,
      amount2,
      max_ratio_error,
    )
    .map_err(|failure| failure.error)
  }

  pub(crate) fn donate_balanced_liquidity_classified(
    donor: &AccountId,
    asset1: AssetKind,
    asset2: AssetKind,
    amount1: Balance,
    amount2: Balance,
    max_ratio_error: Perbill,
  ) -> Result<(), pallet_deos_actors::TaskFailure> {
    if amount1.is_zero() || amount2.is_zero() || asset1 == asset2 {
      return Err(pallet_deos_actors::TaskFailure::permanent(
        DispatchError::Other("InvalidDonation"),
      ));
    }
    let pool_account =
      <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_address(&asset1, &asset2)
        .map_err(|_| {
          pallet_deos_actors::TaskFailure::temporary(DispatchError::Other(
            "DonationPoolUnavailable",
          ))
        })?;
    let reserve1 = Self::asset_balance(asset1, &pool_account);
    let reserve2 = Self::asset_balance(asset2, &pool_account);
    if reserve1.is_zero() || reserve2.is_zero() {
      return Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("DonationPoolEmpty"),
      ));
    }
    Self::ensure_ratio_within_tolerance(amount1, amount2, reserve1, reserve2, max_ratio_error)
      .map_err(pallet_deos_actors::TaskFailure::temporary)?;
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      if let Err(error) = Self::transfer_asset(asset1, donor, &pool_account, amount1) {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          pallet_deos_actors::TaskFailure::permanent(error),
        ));
      }
      if let Err(error) = Self::transfer_asset(asset2, donor, &pool_account, amount2) {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          pallet_deos_actors::TaskFailure::permanent(error),
        ));
      }
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(()))
    })
  }

  pub fn donate_native_staking_liquidity_from_ntve(
    donor: &AccountId,
    total_native: Balance,
    max_staked_debit: Balance,
    max_ratio_error: Perbill,
  ) -> Result<(Balance, Balance), pallet_deos_actors::TaskFailure> {
    if total_native.is_zero() {
      return Err(pallet_deos_actors::TaskFailure::permanent(
        DispatchError::Other("InvalidDonation"),
      ));
    }
    let native_asset_id = <Runtime as pallet_staking::Config>::NativeStakingAssetId::get();
    let staked_asset_id = crate::Staking::staked_asset_id(native_asset_id).ok_or_else(|| {
      pallet_deos_actors::TaskFailure::permanent(DispatchError::Other("StakedAssetUnavailable"))
    })?;
    let base_asset = AssetKind::Local(native_asset_id);
    let staked_asset = AssetKind::Local(staked_asset_id);
    let pool_account = <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_address(
      &base_asset,
      &staked_asset,
    )
    .map_err(|_| {
      pallet_deos_actors::TaskFailure::temporary(DispatchError::Other("DonationPoolUnavailable"))
    })?;
    let reserve_native = Self::asset_balance(base_asset, &pool_account);
    let reserve_staked = Self::asset_balance(staked_asset, &pool_account);
    let staking_pool = pallet_staking::Pools::<Runtime>::get(native_asset_id).ok_or_else(|| {
      pallet_deos_actors::TaskFailure::temporary(DispatchError::Other(
        "NativeStakingPoolUnavailable",
      ))
    })?;
    if reserve_native.is_zero()
      || reserve_staked.is_zero()
      || staking_pool.accounted_balance.is_zero()
      || staking_pool.total_shares.is_zero()
    {
      return Err(pallet_deos_actors::TaskFailure::temporary(
        DispatchError::Other("DonationPoolEmpty"),
      ));
    }
    let stake_amount = Self::native_stake_amount_for_balanced_donation(
      total_native,
      reserve_native,
      reserve_staked,
      staking_pool.accounted_balance,
      staking_pool.total_shares,
    )
    .map_err(pallet_deos_actors::TaskFailure::permanent)?;
    let native_donation = total_native.checked_sub(stake_amount).ok_or_else(|| {
      pallet_deos_actors::TaskFailure::permanent(DispatchError::Other("DonationAmountOverflow"))
    })?;
    if stake_amount.is_zero() || native_donation.is_zero() {
      return Err(pallet_deos_actors::TaskFailure::permanent(
        DispatchError::Other("DonationAmountTooSmall"),
      ));
    }
    let staked_before = Self::asset_balance(staked_asset, donor);
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      if let Err(error) = crate::Staking::stake(
        RuntimeOrigin::signed(donor.clone()),
        native_asset_id,
        stake_amount,
      ) {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          pallet_deos_actors::TaskFailure::permanent(error),
        ));
      }
      let staked_after = Self::asset_balance(staked_asset, donor);
      let staked_donation = staked_after.saturating_sub(staked_before);
      if staked_donation.is_zero() {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          pallet_deos_actors::TaskFailure::permanent(DispatchError::Other(
            "DonationAmountTooSmall",
          )),
        ));
      }
      // The asset-B cap bounds the debit against the actor's pre-existing staked balance: the
      // donation may not drive the staked position below the preservable floor (existing minus
      // max_amount_b). A donation that staked-then-donated the same amount never consumes
      // existing balance, so the cap is trivially satisfied; any donation consuming pre-existing
      // staked balance beyond the preservable capacity is rejected before mutation.
      let staked_after_donation = staked_after.saturating_sub(staked_donation);
      if staked_after_donation < staked_before.saturating_sub(max_staked_debit) {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          pallet_deos_actors::TaskFailure::permanent(DispatchError::Other(
            "DonationExceedsAssetBCap",
          )),
        ));
      }
      if let Err(error) = Self::donate_balanced_liquidity_classified(
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

impl pallet_deos_router::AssetConversionApi<AccountId, Balance> for AssetConversionAdapter {
  fn single_pool_id(asset_a: AssetKind, asset_b: AssetKind) -> Option<(AssetKind, AssetKind)> {
    if asset_a == asset_b {
      return None;
    }
    if asset_a < asset_b {
      Some((asset_a, asset_b))
    } else {
      Some((asset_b, asset_a))
    }
  }

  fn single_pool_reserves(pool_id: (AssetKind, AssetKind)) -> Option<(Balance, Balance)> {
    let (asset_a, asset_b) = pool_id;
    AssetConversion::get_reserves(asset_a, asset_b).ok()
  }

  fn quote_single_pool_exact_input(
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

  fn quote_single_pool_exact_output(
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_out: Balance,
    include_fee: bool,
  ) -> Option<Balance> {
    AssetConversion::quote_price_tokens_for_exact_tokens(
      asset_in,
      asset_out,
      amount_out,
      include_fee,
    )
  }

  fn execute_single_pool_exact_input(
    who: AccountId,
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_in: Balance,
    min_amount_out: Balance,
    recipient: AccountId,
    keep_alive: bool,
  ) -> Result<Balance, pallet_deos_router::AdapterFailure> {
    let path = [asset_in, asset_out];
    // Get target asset and snapshot balance before swap
    let target_asset = asset_out;
    // Snapshot recipient balance before swap
    let balance_before = match target_asset {
      AssetKind::Native => <Balances as NativeInspect<AccountId>>::balance(&recipient),
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::balance(id, &recipient)
      }
    };
    // Convert path from RouterAssetKind to AssetKind and box it
    let boxed_path: Vec<Box<AssetKind>> = path.iter(/* deos-bypass: bounded-iter — Router path has at most three assets */)
      .cloned()
      .map(Box::new)
      .collect();
    let origin = RuntimeOrigin::signed(who.clone());
    AssetConversion::swap_exact_tokens_for_tokens(
      origin,
      boxed_path,
      amount_in,
      min_amount_out,
      recipient.clone(),
      keep_alive,
    )
    .map_err(market_execution_failure)?;
    #[cfg(test)]
    if fail_after_xyk_execution() {
      return Err(pallet_deos_router::AdapterFailure::unknown(
        DispatchError::Other("Injected post-XYK execution failure"),
      ));
    }
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

  fn execute_single_pool_exact_output(
    who: AccountId,
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_out: Balance,
    max_amount_in: Balance,
    recipient: AccountId,
    keep_alive: bool,
  ) -> Result<pallet_deos_router::ExactOutputExecution, pallet_deos_router::AdapterFailure> {
    let path = [asset_in, asset_out];
    let input_asset = asset_in;
    let output_asset = asset_out;
    let balance_before = match input_asset {
      AssetKind::Native => <Balances as NativeInspect<AccountId>>::balance(&who),
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::balance(id, &who)
      }
    };
    let recipient_before = match output_asset {
      AssetKind::Native => <Balances as NativeInspect<AccountId>>::balance(&recipient),
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::balance(id, &recipient)
      }
    };
    let boxed_path: Vec<Box<AssetKind>> = path.into_iter().map(Box::new).collect();
    AssetConversion::swap_tokens_for_exact_tokens(
      RuntimeOrigin::signed(who.clone()),
      boxed_path,
      amount_out,
      max_amount_in,
      recipient.clone(),
      keep_alive,
    )
    .map_err(market_execution_failure)?;
    let balance_after = match input_asset {
      AssetKind::Native => <Balances as NativeInspect<AccountId>>::balance(&who),
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::balance(id, &who)
      }
    };
    let recipient_after = match output_asset {
      AssetKind::Native => <Balances as NativeInspect<AccountId>>::balance(&recipient),
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::balance(id, &recipient)
      }
    };
    Ok(pallet_deos_router::ExactOutputExecution {
      amount_in: balance_before.saturating_sub(balance_after),
      recipient_amount_out: recipient_after.saturating_sub(recipient_before),
    })
  }
}

impl<T> pallet_deos_router::TmcInterface<T::AccountId, Balance> for TmcPalletAdapter<T>
where
  T: pallet_deos_router::pallet::Config + pallet_tmc::pallet::Config<Balance = Balance>,
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
  ) -> Result<Balance, pallet_deos_router::AdapterFailure> {
    let total_minted = pallet_tmc::Pallet::<T>::calculate_total_mint(token_asset, foreign_amount)
      .map_err(pallet_deos_router::AdapterFailure::unknown)?;
    Ok(<T as pallet_tmc::pallet::Config>::UserAllocationRatio::get().mul_floor(total_minted))
  }

  fn mint_with_distribution(
    who: &T::AccountId,
    recipient: &T::AccountId,
    token_asset: AssetKind,
    foreign_asset: AssetKind,
    foreign_amount: Balance,
  ) -> Result<Balance, pallet_deos_router::AdapterFailure> {
    let total_minted = pallet_tmc::Pallet::<T>::mint_with_distribution(
      who,
      recipient,
      token_asset,
      foreign_asset,
      foreign_amount,
    )
    .map_err(pallet_deos_router::AdapterFailure::unknown)?;
    Ok(<T as pallet_tmc::pallet::Config>::UserAllocationRatio::get().mul_floor(total_minted))
  }
}

impl pallet_deos_router::PriceOracle<Balance> for PriceOracleImpl<Runtime> {
  fn update_ema_price(
    asset_in: AssetKind,
    asset_out: AssetKind,
    price: Balance,
  ) -> Result<(), pallet_deos_router::AdapterFailure> {
    let feed = super::oracle_config::deos_router_pool_feed(asset_in, asset_out);
    if !pallet_oracle::Feeds::<Runtime>::contains_key(feed) {
      return Ok(());
    }
    let producer: AccountId = RouterPalletId::get().into_account_truncating();
    crate::Oracle::publish_from(producer, feed, price).map_err(oracle_publication_failure)
  }

  fn get_ema_price(asset_in: AssetKind, asset_out: AssetKind) -> Option<Balance> {
    let feed = super::oracle_config::deos_router_pool_feed(asset_in, asset_out);
    crate::Oracle::observation_state(feed, u32::MAX)
      .ok()
      .and_then(|state| match state {
        pallet_oracle::ObservationState::Fresh(observation) if observation.value > 0 => {
          Some(observation.value)
        }
        _ => None,
      })
  }

  fn validate_price_deviation(
    asset_in: AssetKind,
    asset_out: AssetKind,
    current_price: Balance,
  ) -> Result<(), pallet_deos_router::AdapterFailure> {
    if let Some(ema_price) = Self::get_ema_price(asset_in, asset_out) {
      let deviation = if current_price > ema_price {
        polkadot_sdk::sp_runtime::Perbill::from_rational(current_price - ema_price, ema_price)
      } else {
        polkadot_sdk::sp_runtime::Perbill::from_rational(ema_price - current_price, ema_price)
      };
      if deviation > DeosRouterMaxPriceDeviation::get() {
        return Err(router_adapter_failure(
          DispatchError::Other("Price deviation exceeded"),
          pallet_deos_router::RouterFailureClass::ProtectionRejected,
          pallet_deos_router::RetryDisposition::RetryLater,
        ));
      }
    }
    Ok(())
  }
}

impl pallet_deos_router::FeeRoutingAdapter<AccountId, Balance> for FeeManagerImpl<Runtime> {
  fn route_fee(
    who: &AccountId,
    asset: AssetKind,
    amount: Balance,
  ) -> Result<(), pallet_deos_router::AdapterFailure> {
    let burn_actor_account = BurnActorAccount::get();
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      if let Err(failure) = RuntimeAddressEventIngress::preflight_internal_inbound(
        &burn_actor_account,
        asset,
        amount,
        who,
      ) {
        return polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          actor_ingress_failure(failure),
        ));
      }
      let result = (|| -> Result<(), pallet_deos_router::AdapterFailure> {
        let funding_failure = || {
          router_adapter_failure(
            DispatchError::Token(TokenError::FundsUnavailable),
            pallet_deos_router::RouterFailureClass::FeeRejected,
            pallet_deos_router::RetryDisposition::RetryLater,
          )
        };
        match asset {
          AssetKind::Native => {
            Balances::transfer(
              who,
              &burn_actor_account,
              amount,
              polkadot_sdk::frame_support::traits::tokens::ExistenceRequirement::KeepAlive,
            )
            .map_err(|_| funding_failure())?;
          }
          AssetKind::Local(id) | AssetKind::Foreign(id) => {
            use polkadot_sdk::frame_support::traits::fungibles::Mutate;
            <pallet_assets::Pallet<Runtime> as Mutate<AccountId>>::transfer(
              id,
              who,
              &burn_actor_account,
              amount,
              polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
            )
            .map_err(|_| funding_failure())?;
          }
        }
        RuntimeAddressEventIngress::on_internal_inbound(&burn_actor_account, asset, amount, who)
          .map_err(actor_ingress_failure)?;
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
}

impl pallet_deos_router::pallet::Config for Runtime {
  type AdminOrigin = frame_system::EnsureRoot<AccountId>;
  type AssetConversion = AssetConversionAdapter;
  type Assets = pallet_assets::Pallet<Runtime>;
  type BurnActorAccount = BurnActorAccount;
  type LiquidityActorAccount = LiquidityActorAccount;
  type Currency = Balances;
  type DefaultRouterFee = DeosRouterFee;
  type EmaHalfLife = DeosRouterEmaHalfLife;
  type FeeAdapter = FeeManagerImpl<Runtime>;
  type MaxPriceDeviation = DeosRouterMaxPriceDeviation;
  type MaxLpPairs = DeosRouterMaxLpPairs;
  type MaxRouterFee = DeosRouterMaxFee;
  type MinSwapForeign = MinSwapForeign;
  type NativeAsset = NativeAsset;
  type PalletId = RouterPalletId;
  type Precision = DeosRouterPrecision;
  type PriceOracle = PriceOracleImpl<Runtime>;
  type TmcPallet = TmcPalletAdapter<Runtime>;
  type WeightInfo = crate::weights::pallet_deos_router::SubstrateWeight<Runtime>;
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = RuntimeBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct RuntimeBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_deos_router::types::BenchmarkHelper<AssetKind, AccountId, Balance>
  for RuntimeBenchmarkHelper
{
  fn create_asset(asset: AssetKind) -> polkadot_sdk::sp_runtime::DispatchResult {
    if let AssetKind::Local(id) | AssetKind::Foreign(id) = asset {
      if !<pallet_assets::Pallet<Runtime> as FungiblesInspect<AccountId>>::asset_exists(id) {
        let _ = pallet_assets::Pallet::<Runtime>::force_create(
          RuntimeOrigin::root(),
          id,
          polkadot_sdk::sp_runtime::MultiAddress::Id(BurnActorAccount::get()),
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
    let creator = BurnActorAccount::get();
    let _ =
      <Balances as Currency<AccountId>>::deposit_creating(&creator, 1_000_000_000_000_000_000);
    AssetConversionAdapter::ensure_lp_asset_namespace();
    AssetConversion::create_pool(
      RuntimeOrigin::signed(creator),
      Box::new(asset1),
      Box::new(asset2),
    )?;
    super::assets_config::register_pool_lp_pair(asset1, asset2)
  }

  fn create_tmc_curve(
    token_asset: AssetKind,
    collateral_asset: AssetKind,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    pallet_tmc::Pallet::<Runtime>::create_curve(
      RuntimeOrigin::root(),
      token_asset,
      collateral_asset,
      DeosRouterPrecision::get(),
      0,
    )
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
