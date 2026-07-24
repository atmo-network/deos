//! Runtime adapter traits for AAA task execution.

use frame::prelude::*;
use polkadot_sdk::sp_runtime::Perbill;

/// Closed retryability classification supplied by runtime mutation adapters.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum RetryClass {
  Permanent,
  Temporary,
}

/// Typed adapter failure. Unclassified dispatch failures convert to Permanent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskFailure {
  pub error: DispatchError,
  pub retry: RetryClass,
}

impl TaskFailure {
  pub fn permanent(error: impl Into<DispatchError>) -> Self {
    Self {
      error: error.into(),
      retry: RetryClass::Permanent,
    }
  }

  pub fn temporary(error: impl Into<DispatchError>) -> Self {
    Self {
      error: error.into(),
      retry: RetryClass::Temporary,
    }
  }
}

impl From<DispatchError> for TaskFailure {
  fn from(error: DispatchError) -> Self {
    Self::permanent(error)
  }
}

/// Runtime authorization for actors whose stored funding policy is `RuntimePolicy`.
///
/// The pallet handles `OwnerOnly`, `SignedAllowlist`, and `AnySource` itself. This adapter must
/// default deny and authorize only explicit actor/source pairs over runtime-verified provenance.
pub trait FundingAuthority<AccountId> {
  fn allows(
    aaa_id: crate::AaaId,
    owner: &AccountId,
    provenance: &crate::FundingProvenance<AccountId>,
  ) -> bool;
}

impl<AccountId> FundingAuthority<AccountId> for () {
  fn allows(_: crate::AaaId, _: &AccountId, _: &crate::FundingProvenance<AccountId>) -> bool {
    false
  }
}

/// Asset mutations and queries.
///
/// Covers Transfer, SplitTransfer, Burn, Mint, and balance queries. `mint` is privileged — the
/// pallet rejects Mint tasks for User AAA at creation.
pub trait AssetOps<AccountId, AssetId, Balance> {
  fn transfer(
    from: &AccountId,
    to: &AccountId,
    asset: AssetId,
    amount: Balance,
  ) -> Result<(), TaskFailure>;

  fn burn(who: &AccountId, asset: AssetId, amount: Balance) -> Result<(), TaskFailure>;

  fn mint(to: &AccountId, asset: AssetId, amount: Balance) -> Result<(), TaskFailure>;

  /// Adapter-visible transferable balance before AAA-local fee reservation.
  fn balance(who: &AccountId, asset: AssetId) -> Balance;

  fn minimum_balance(asset: AssetId) -> Balance;

  fn can_deposit(who: &AccountId, asset: AssetId, amount: Balance) -> bool;
}

/// Runtime staking operations. Required only when Stake or Unstake appears in a plan.
pub trait StakingOps<AccountId, AssetId, Balance> {
  fn stake(who: &AccountId, asset: AssetId, amount: Balance) -> Result<(), TaskFailure>;
  fn unstake(who: &AccountId, asset: AssetId, shares: Balance) -> Result<(), TaskFailure>;
  fn share_balance(who: &AccountId, asset: AssetId) -> Balance;
  fn share_asset(asset: AssetId) -> Option<AssetId>;
}

/// Runtime protocol-liquidity donation operation.
pub trait LiquidityDonationOps<AccountId, AssetId, Balance> {
  fn donate_liquidity(
    who: &AccountId,
    asset_a: AssetId,
    asset_b: AssetId,
    amount: Balance,
    max_ratio_error: Perbill,
  ) -> Result<(Balance, Balance), TaskFailure>;
}

/// Runtime DEX operations.
pub trait DexOps<AccountId, AssetId, Balance> {
  fn swap_exact_in(
    who: &AccountId,
    asset_in: AssetId,
    asset_out: AssetId,
    amount_in: Balance,
    slippage_tolerance: Perbill,
  ) -> Result<Balance, TaskFailure>;

  fn swap_exact_out(
    who: &AccountId,
    asset_in: AssetId,
    asset_out: AssetId,
    amount_out: Balance,
    max_amount_in: Balance,
    slippage_tolerance: Perbill,
  ) -> Result<Balance, TaskFailure>;

  fn add_liquidity(
    who: &AccountId,
    asset_a: AssetId,
    asset_b: AssetId,
    amount_a: Balance,
    amount_b: Balance,
  ) -> Result<(Balance, Balance, Balance), TaskFailure>;

  fn remove_liquidity(
    who: &AccountId,
    lp_asset: AssetId,
    lp_amount: Balance,
  ) -> Result<(Balance, Balance), TaskFailure>;
}

/// Fail-closed `AssetOps` fallback for runtimes without asset mutation support.
impl<AccountId, AssetId, Balance: Default> AssetOps<AccountId, AssetId, Balance> for () {
  fn transfer(_: &AccountId, _: &AccountId, _: AssetId, _: Balance) -> Result<(), TaskFailure> {
    Err(TaskFailure::permanent(DispatchError::Other(
      "AssetOps not configured",
    )))
  }

  fn burn(_: &AccountId, _: AssetId, _: Balance) -> Result<(), TaskFailure> {
    Err(TaskFailure::permanent(DispatchError::Other(
      "AssetOps not configured",
    )))
  }

  fn mint(_: &AccountId, _: AssetId, _: Balance) -> Result<(), TaskFailure> {
    Err(TaskFailure::permanent(DispatchError::Other(
      "AssetOps not configured",
    )))
  }

  fn balance(_: &AccountId, _: AssetId) -> Balance {
    Balance::default()
  }

  fn minimum_balance(_: AssetId) -> Balance {
    Balance::default()
  }

  fn can_deposit(_: &AccountId, _: AssetId, _: Balance) -> bool {
    false
  }
}

/// Fail-closed `DexOps` fallback for runtimes without DEX support.
impl<AccountId, AssetId, Balance: Default> DexOps<AccountId, AssetId, Balance> for () {
  fn swap_exact_in(
    _: &AccountId,
    _: AssetId,
    _: AssetId,
    _: Balance,
    _: Perbill,
  ) -> Result<Balance, TaskFailure> {
    Err(TaskFailure::permanent(DispatchError::Other(
      "DexOps not configured",
    )))
  }

  fn swap_exact_out(
    _: &AccountId,
    _: AssetId,
    _: AssetId,
    _: Balance,
    _: Balance,
    _: Perbill,
  ) -> Result<Balance, TaskFailure> {
    Err(TaskFailure::permanent(DispatchError::Other(
      "DexOps not configured",
    )))
  }

  fn add_liquidity(
    _: &AccountId,
    _: AssetId,
    _: AssetId,
    _: Balance,
    _: Balance,
  ) -> Result<(Balance, Balance, Balance), TaskFailure> {
    Err(TaskFailure::permanent(DispatchError::Other(
      "DexOps not configured",
    )))
  }

  fn remove_liquidity(
    _: &AccountId,
    _: AssetId,
    _: Balance,
  ) -> Result<(Balance, Balance), TaskFailure> {
    Err(TaskFailure::permanent(DispatchError::Other(
      "DexOps not configured",
    )))
  }
}

/// Fail-closed `StakingOps` fallback for runtimes without staking support.
impl<AccountId, AssetId, Balance: Default> StakingOps<AccountId, AssetId, Balance> for () {
  fn stake(_: &AccountId, _: AssetId, _: Balance) -> Result<(), TaskFailure> {
    Err(TaskFailure::permanent(DispatchError::Other(
      "StakingOps not configured",
    )))
  }

  fn unstake(_: &AccountId, _: AssetId, _: Balance) -> Result<(), TaskFailure> {
    Err(TaskFailure::permanent(DispatchError::Other(
      "StakingOps not configured",
    )))
  }

  fn share_balance(_: &AccountId, _: AssetId) -> Balance {
    Balance::default()
  }

  fn share_asset(_: AssetId) -> Option<AssetId> {
    None
  }
}

/// Fail-closed fallback for runtimes without protocol-liquidity donation support.
impl<AccountId, AssetId, Balance: Default> LiquidityDonationOps<AccountId, AssetId, Balance>
  for ()
{
  fn donate_liquidity(
    _: &AccountId,
    _: AssetId,
    _: AssetId,
    _: Balance,
    _: Perbill,
  ) -> Result<(Balance, Balance), TaskFailure> {
    Err(TaskFailure::permanent(DispatchError::Other(
      "LiquidityDonationOps not configured",
    )))
  }
}
