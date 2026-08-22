use crate::{
  NativeSecurityMode, NativeSecurityModeProvider as _, NativeSecurityReadiness,
  NativeSecurityRewardPotStatus, NativeStakingLpAssetValidator as _,
  NativeStakingReadModelProvider as _, SecurityEpoch, SecurityEpochProvider as _, pallet::*,
};
use frame::prelude::{FixedPointNumber, Get};
use polkadot_sdk::frame_support::{
  ensure,
  traits::{Currency, ExistenceRequirement},
};
use polkadot_sdk::sp_runtime::{
  ArithmeticError, DispatchError, DispatchResult, FixedU128,
  traits::{CheckedAdd, SaturatedConversion, Zero},
};

#[derive(Clone, Copy)]
pub(crate) enum NativeSecurityOperation {
  NewNomination,
  Redelegation,
  CandidateSelection,
  RewardFunding,
  RewardCompound,
  ContractObligations,
}

impl<T: Config> Pallet<T> {
  pub(crate) fn native_security_operation_available(operation: NativeSecurityOperation) -> bool {
    let lp_backed = T::NativeSecurityModeProvider::mode() == NativeSecurityMode::LpBackedSelection;
    match operation {
      NativeSecurityOperation::NewNomination
      | NativeSecurityOperation::Redelegation
      | NativeSecurityOperation::CandidateSelection
      | NativeSecurityOperation::RewardFunding
      | NativeSecurityOperation::RewardCompound => lp_backed,
      NativeSecurityOperation::ContractObligations => !lp_backed,
    }
  }

  pub fn native_security_mode() -> NativeSecurityMode {
    T::NativeSecurityModeProvider::mode()
  }

  pub fn current_security_epoch() -> SecurityEpoch {
    T::SecurityEpochProvider::current_security_epoch()
  }

  pub fn native_security_readiness() -> NativeSecurityReadiness {
    debug_assert!(Self::native_security_operation_available(
      NativeSecurityOperation::CandidateSelection
    ));
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

  pub(crate) fn ensure_native_security_operation(
    operation: NativeSecurityOperation,
  ) -> DispatchResult {
    ensure!(
      Self::native_security_operation_available(operation),
      Error::<T>::NativeSecurityModeInactive
    );
    Ok(())
  }

  pub(crate) fn reward_weight_from_snapshot(
    shares: T::Balance,
    coefficient: FixedU128,
  ) -> Result<T::Balance, DispatchError> {
    let shares_u128: u128 = shares.saturated_into();
    let shares_roundtrip: T::Balance = shares_u128.saturated_into();
    ensure!(shares_roundtrip == shares, ArithmeticError::Overflow);
    let weighted = coefficient
      .checked_mul_int(shares_u128)
      .ok_or(ArithmeticError::Overflow)?;
    let narrowed: T::Balance = weighted.saturated_into();
    let weighted_roundtrip: u128 = narrowed.saturated_into();
    ensure!(weighted_roundtrip == weighted, ArithmeticError::Overflow);
    Ok(narrowed)
  }

  pub(crate) fn native_security_retention_state(
    current_epoch: SecurityEpoch,
  ) -> Result<(u32, Option<SecurityEpoch>, bool), DispatchError> {
    let retention_bound = T::SecurityRewardClaimHorizon::get()
      .checked_add(2)
      .ok_or(Error::<T>::NativeSecurityRetentionBlocked)?;
    let expiry_offset = T::SecurityRewardClaimHorizon::get()
      .checked_add(1)
      .ok_or(Error::<T>::NativeSecurityRetentionBlocked)?;
    let due_threshold = current_epoch.checked_sub(expiry_offset);
    let mut retained = 0u32;
    let mut oldest_due = None;
    let mut has_planned = false;
    let reward_pot_iter = NativeSecurityRewardPots::<T>::iter(); // deos-bypass: bounded-iter — retention bound + 1
    for (epoch, pot) in reward_pot_iter {
      retained = retained
        .checked_add(1)
        .ok_or(Error::<T>::NativeSecurityRetentionBlocked)?;
      ensure!(
        retained <= retention_bound,
        Error::<T>::NativeSecurityRetentionBlocked
      );
      has_planned |= pot.status == NativeSecurityRewardPotStatus::Planned;
      if due_threshold.is_some_and(|threshold| epoch <= threshold) {
        oldest_due = Some(oldest_due.map_or(epoch, |oldest: SecurityEpoch| oldest.min(epoch)));
      }
    }
    Ok((retained, oldest_due, has_planned))
  }

  pub(crate) fn ensure_native_security_retention_admission() -> DispatchResult {
    let current_epoch = T::SecurityEpochProvider::current_security_epoch();
    let (retained, oldest_due, has_planned) = Self::native_security_retention_state(current_epoch)?;
    let retention_bound = T::SecurityRewardClaimHorizon::get()
      .checked_add(2)
      .ok_or(Error::<T>::NativeSecurityRetentionBlocked)?;
    ensure!(
      retained < retention_bound && oldest_due.is_none() && !has_planned,
      Error::<T>::NativeSecurityRetentionBlocked
    );
    Ok(())
  }

  pub(crate) fn do_fund_native_security_reward(
    epoch: SecurityEpoch,
    amount: T::Balance,
  ) -> DispatchResult {
    Self::validate_native_security_reward_funding(epoch, amount)?;
    let source = T::SecurityRewardFundingSource::get();
    let reward_account = Self::native_security_reward_account();
    T::NativeCurrency::transfer(
      &source,
      &reward_account,
      amount,
      ExistenceRequirement::KeepAlive,
    )?;
    Self::record_native_security_reward_funding(epoch, amount)
  }

  pub(crate) fn validate_native_security_reward_funding(
    epoch: SecurityEpoch,
    amount: T::Balance,
  ) -> DispatchResult {
    Self::ensure_native_security_operation(NativeSecurityOperation::RewardFunding)?;
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

  pub(crate) fn record_native_security_reward_funding(
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
