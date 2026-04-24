//! Adapter traits for AAA pallet
//!
//! Two traits abstract all runtime-specific operations, keeping pallet-aaa
//! fully generic over asset types and independent of any runtime implementation.

use frame::prelude::*;
use polkadot_sdk::sp_runtime::Perbill;

/// Asset mutations and queries
///
/// Covers Transfer, SplitTransfer, Burn, Mint, and balance queries.
/// `mint` is privileged — pallet rejects Mint tasks for User AAA at creation.
pub trait AssetOps<AccountId, AssetId, Balance> {
  fn transfer(
    from: &AccountId,
    to: &AccountId,
    asset: AssetId,
    amount: Balance,
  ) -> Result<(), DispatchError>;

  fn burn(who: &AccountId, asset: AssetId, amount: Balance) -> Result<(), DispatchError>;

  fn mint(to: &AccountId, asset: AssetId, amount: Balance) -> Result<(), DispatchError>;

  // Adapter-visible immediately transferable balance before any AAA-local
  // fee reservation is subtracted.
  fn balance(who: &AccountId, asset: AssetId) -> Balance;

  fn minimum_balance(asset: AssetId) -> Balance;

  fn can_deposit(who: &AccountId, asset: AssetId, amount: Balance) -> bool;

  fn total_issuance(asset: AssetId) -> Balance;
}

/// DEX operations — swap and liquidity
///
/// Optional: required only when SwapExactIn/Out, AddLiquidity, or
/// RemoveLiquidity tasks are present in a execution_plan.
///
/// `swap_exact_in` receives `slippage_tolerance: Perbill` — the adapter
/// computes `min_out` internally (e.g. via DEX quote). The pallet never
/// touches pricing logic.
/// Staking operations
///
/// Optional: required only when Stake or Unstake tasks are present.
pub trait StakingOps<AccountId, AssetId, Balance> {
  fn stake(who: &AccountId, asset: AssetId, amount: Balance) -> Result<(), DispatchError>;
  fn unstake(who: &AccountId, asset: AssetId, shares: Balance) -> Result<(), DispatchError>;
}

pub trait LiquidityDonationOps<AccountId, AssetId, Balance> {
  fn donate_liquidity(
    who: &AccountId,
    asset_a: AssetId,
    asset_b: AssetId,
    amount: Balance,
    max_ratio_error: Perbill,
  ) -> Result<(Balance, Balance), DispatchError>;
}

pub trait DexOps<AccountId, AssetId, Balance> {
  fn swap_exact_in(
    who: &AccountId,
    asset_in: AssetId,
    asset_out: AssetId,
    amount_in: Balance,
    slippage_tolerance: Perbill,
  ) -> Result<Balance, DispatchError>;

  fn swap_exact_out(
    who: &AccountId,
    asset_in: AssetId,
    asset_out: AssetId,
    amount_out: Balance,
    slippage_tolerance: Perbill,
  ) -> Result<Balance, DispatchError>;

  fn add_liquidity(
    who: &AccountId,
    asset_a: AssetId,
    asset_b: AssetId,
    amount_a: Balance,
    amount_b: Balance,
  ) -> Result<(Balance, Balance, Balance), DispatchError>;

  fn remove_liquidity(
    who: &AccountId,
    lp_asset: AssetId,
    lp_amount: Balance,
  ) -> Result<(Balance, Balance), DispatchError>;

  fn get_pool_reserves(asset_a: AssetId, asset_b: AssetId) -> Option<(Balance, Balance)>;
}

/// No-op `AssetOps` for use in configurations where asset ops are not needed.
impl<AccountId, AssetId, Balance: Default> AssetOps<AccountId, AssetId, Balance> for () {
  fn transfer(_: &AccountId, _: &AccountId, _: AssetId, _: Balance) -> Result<(), DispatchError> {
    Ok(())
  }

  fn burn(_: &AccountId, _: AssetId, _: Balance) -> Result<(), DispatchError> {
    Ok(())
  }

  fn mint(_: &AccountId, _: AssetId, _: Balance) -> Result<(), DispatchError> {
    Ok(())
  }

  fn balance(_: &AccountId, _: AssetId) -> Balance {
    Balance::default()
  }

  fn minimum_balance(_: AssetId) -> Balance {
    Balance::default()
  }

  fn can_deposit(_: &AccountId, _: AssetId, _: Balance) -> bool {
    true
  }

  fn total_issuance(_: AssetId) -> Balance {
    Balance::default()
  }
}

/// No-op `DexOps` for configurations where DEX is not used.
impl<AccountId, AssetId, Balance: Default> DexOps<AccountId, AssetId, Balance> for () {
  fn swap_exact_in(
    _: &AccountId,
    _: AssetId,
    _: AssetId,
    _: Balance,
    _: Perbill,
  ) -> Result<Balance, DispatchError> {
    Err(DispatchError::Other("DexOps not configured"))
  }

  fn swap_exact_out(
    _: &AccountId,
    _: AssetId,
    _: AssetId,
    _: Balance,
    _: Perbill,
  ) -> Result<Balance, DispatchError> {
    Err(DispatchError::Other("DexOps not configured"))
  }

  fn add_liquidity(
    _: &AccountId,
    _: AssetId,
    _: AssetId,
    _: Balance,
    _: Balance,
  ) -> Result<(Balance, Balance, Balance), DispatchError> {
    Err(DispatchError::Other("DexOps not configured"))
  }

  fn remove_liquidity(
    _: &AccountId,
    _: AssetId,
    _: Balance,
  ) -> Result<(Balance, Balance), DispatchError> {
    Err(DispatchError::Other("DexOps not configured"))
  }

  fn get_pool_reserves(_: AssetId, _: AssetId) -> Option<(Balance, Balance)> {
    None
  }
}

/// No-op `StakingOps` for configurations where Staking is not used.
impl<AccountId, AssetId, Balance: Default> StakingOps<AccountId, AssetId, Balance> for () {
  fn stake(_: &AccountId, _: AssetId, _: Balance) -> Result<(), DispatchError> {
    Err(DispatchError::Other("StakingOps not configured"))
  }

  fn unstake(_: &AccountId, _: AssetId, _: Balance) -> Result<(), DispatchError> {
    Err(DispatchError::Other("StakingOps not configured"))
  }
}

/// No-op `LiquidityDonationOps` for configurations where LP donation is not used.
impl<AccountId, AssetId, Balance: Default> LiquidityDonationOps<AccountId, AssetId, Balance>
  for ()
{
  fn donate_liquidity(
    _: &AccountId,
    _: AssetId,
    _: AssetId,
    _: Balance,
    _: Perbill,
  ) -> Result<(Balance, Balance), DispatchError> {
    Err(DispatchError::Other("LiquidityDonationOps not configured"))
  }
}
