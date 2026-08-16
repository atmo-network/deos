use crate::{
  NativeSecurityRewardPotStatus, NativeSecurityView, NativeSecurityViewError,
  NativeStakingReadModelProvider as _, pallet::*,
};
use frame::prelude::{BlockNumberFor, Get, Saturating};
use polkadot_sdk::sp_runtime::{
  FixedU128,
  traits::{SaturatedConversion, Zero},
};

impl<T: Config> Pallet<T> {
  pub(crate) fn build_native_security_view() -> Result<NativeSecurityView, NativeSecurityViewError>
  {
    let retention_bound = T::SecurityRewardClaimHorizon::get()
      .checked_add(2)
      .ok_or(NativeSecurityViewError::RetentionBoundExceeded)?;
    let mut retained = 0u32;
    let mut planned_epoch = None;
    let mut settlement_obligations_remain = !NativeSecurityRewardLiability::<T>::get().is_zero();
    for (epoch, pot) in
      NativeSecurityRewardPots::<T>::iter().take(retention_bound.saturating_add(1) as usize)
    {
      retained = retained.saturating_add(1);
      if retained > retention_bound {
        return Err(NativeSecurityViewError::RetentionBoundExceeded);
      }
      match pot.status {
        NativeSecurityRewardPotStatus::Planned => {
          if planned_epoch.replace(epoch).is_some() {
            return Err(NativeSecurityViewError::MultiplePlannedEpochs);
          }
        }
        NativeSecurityRewardPotStatus::Open | NativeSecurityRewardPotStatus::Finalized => {
          settlement_obligations_remain = true;
        }
      }
    }
    Ok(NativeSecurityView {
      mode: Self::native_security_mode(),
      readiness: Self::native_security_readiness(),
      current_epoch: Self::current_security_epoch(),
      planned_epoch,
      settlement_obligations_remain,
    })
  }

  pub(crate) fn build_native_staking_exchange_rate() -> Option<FixedU128> {
    let pool = Pools::<T>::get(T::NativeStakingAssetId::get())?;
    if pool.total_shares.is_zero() || pool.accounted_balance.is_zero() {
      return None;
    }
    Some(FixedU128::from_rational(
      pool.accounted_balance.saturated_into::<u128>(),
      pool.total_shares.saturated_into::<u128>(),
    ))
  }

  pub(crate) fn build_native_staking_liquidity_pool()
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

  pub(crate) fn build_native_locked_lp_position(
    account: T::AccountId,
  ) -> NativeLockedLpPosition<T::Balance> {
    let total_locked_lp = AccountNativeLpLocked::<T>::get(&account);
    let collator_locked_lp = NativeNominationOperators::<T>::get(&account)
      .iter() // deos-bypass: bounded-iter — MaxNominationsPerAccount
      .filter_map(|operator| NativeLpLocks::<T>::get(&account, operator))
      .fold(T::Balance::zero(), |total, lock| {
        total.saturating_add(lock.amount)
      });
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

  pub(crate) fn build_native_collator_lp_position(
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

  pub(crate) fn build_native_governance_custody_position(
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
