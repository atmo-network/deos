use crate::{NativeGovernanceLockProvider as _, pallet::*};
use frame::prelude::Get;
use polkadot_sdk::frame_support::ensure;
use polkadot_sdk::sp_runtime::{
  ArithmeticError, DispatchResult,
  traits::{CheckedAdd, CheckedSub, Zero},
};

impl<T: Config> Pallet<T> {
  pub(crate) fn ensure_native_governance_unlocked(account: &T::AccountId) -> DispatchResult {
    let Some(lock_until) = T::NativeGovernanceLockProvider::lock_until(account) else {
      return Ok(());
    };
    ensure!(
      polkadot_sdk::frame_system::Pallet::<T>::block_number() >= lock_until,
      Error::<T>::NativeGovernanceLockActive
    );
    Ok(())
  }

  pub(crate) fn is_native_governance_asset(asset_id: T::AssetId) -> bool {
    if asset_id == T::NativeStakingAssetId::get() {
      return true;
    }
    Self::staked_asset_id(T::NativeStakingAssetId::get())
      .is_some_and(|staked_asset_id| staked_asset_id == asset_id)
  }

  pub(crate) fn decrease_total_native_governance_asset_locked(
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

  pub(crate) fn increase_operator_native_lp_locked(
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

  pub(crate) fn decrease_operator_native_lp_locked(
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

  pub(crate) fn decrease_account_native_lp_locked(
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

  pub(crate) fn decrease_total_native_lp_locked(amount: T::Balance) -> DispatchResult {
    let current = TotalNativeLpLocked::<T>::get();
    let updated = current
      .checked_sub(&amount)
      .ok_or(ArithmeticError::Underflow)?;
    TotalNativeLpLocked::<T>::put(updated);
    Ok(())
  }
}
