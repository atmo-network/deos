use crate::{
  StakedAssetIdResolver as _, StakedAssetLifecycle as _,
  pallet::{Config, Error, Event, LiveStakedAssetBaseAssets, Pallet, PoolState, Pools},
};
use codec::{Decode, Encode};
use frame::prelude::Get;
use polkadot_sdk::frame_support::{
  ensure,
  traits::{
    fungibles::{Inspect, Mutate},
    tokens::Preservation,
  },
};
use polkadot_sdk::sp_core::U256;
use polkadot_sdk::sp_runtime::{
  ArithmeticError, DispatchError,
  traits::{AccountIdConversion, CheckedAdd, CheckedSub, SaturatedConversion, Zero},
};

impl<T: Config> Pallet<T> {
  pub(crate) fn do_stake(
    asset_id: T::AssetId,
    account: &T::AccountId,
    amount: T::Balance,
  ) -> Result<T::Balance, DispatchError> {
    Self::credit_stake_from(asset_id, account, account, amount, Preservation::Protect)
  }

  pub(crate) fn credit_stake_from(
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

  pub(crate) fn create_staked_asset_for_pool(
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

  pub(crate) fn live_staked_asset_id(asset_id: T::AssetId) -> Option<T::AssetId> {
    let staked_asset_id = Self::staked_asset_id(asset_id)?;
    if T::Assets::asset_exists(staked_asset_id) {
      return Some(staked_asset_id);
    }
    None
  }

  fn index_live_staked_asset(
    asset_id: T::AssetId,
    staked_asset_id: T::AssetId,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
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

  pub fn staked_receipt_value(asset_id: T::AssetId, account: &T::AccountId) -> Option<T::Balance> {
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

  pub(crate) fn uses_staked_receipts(asset_id: T::AssetId) -> Option<T::AssetId> {
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

  pub(crate) fn sync_pool_state(
    asset_id: T::AssetId,
  ) -> Result<PoolState<T::Balance>, DispatchError> {
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

  pub(crate) fn mul_div_floor(a: T::Balance, b: T::Balance, c: T::Balance) -> T::Balance {
    let a_u128: u128 = a.saturated_into();
    let b_u128: u128 = b.saturated_into();
    let c_u128: u128 = c.saturated_into();
    let result = (U256::from(a_u128) * U256::from(b_u128)) / U256::from(c_u128);
    result.low_u128().saturated_into()
  }
}
