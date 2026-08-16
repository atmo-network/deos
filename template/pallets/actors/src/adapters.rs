//! Runtime adapter traits for Actors task execution.

use frame::prelude::*;
use polkadot_sdk::sp_runtime::{DispatchResult, Perbill};

use crate::types::AddressEvent;

/// Certified bounded ingress for one externally owned observation revision.
pub trait ObservationChangeIngress<FeedId> {
  fn note_observation_changed(feed: FeedId, revision: u64) -> DispatchResult;
}

/// Minimal authoritative actor context for adapter operations whose policy depends on Actors type.
pub struct ExecutionContext<'a, AccountId> {
  pub actor: &'a AccountId,
  pub actor_type: crate::ActorType,
}

impl<'a, AccountId> ExecutionContext<'a, AccountId> {
  pub const fn new(actor: &'a AccountId, actor_type: crate::ActorType) -> Self {
    Self { actor, actor_type }
  }
}

/// Closed retryability classification supplied by runtime mutation adapters.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum RetryClass {
  Permanent,
  Temporary,
}

/// Typed Step failure. Unclassified dispatch failures convert to Permanent.
///
/// The concrete cause and independent retry disposition remain one canonical fact
/// from adapter execution through production events and simulation traces.
#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
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

/// Typed rejection of one certified AddressEvent ingress movement (spec 6.2).
///
/// `retry` carries the same closed classification as `TaskFailure`: recoverable
/// queue/wakeup capacity or placement unavailability is Temporary, while monotonic
/// ticket/index exhaustion, topology corruption, invalid provenance, and invariant
/// failure are Permanent. A non-Actors producer maps the same failure to its outer
/// dispatch error, which rejects and rolls back the certified movement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IngressFailure {
  pub error: DispatchError,
  pub retry: RetryClass,
}

impl IngressFailure {
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

impl From<IngressFailure> for DispatchError {
  fn from(failure: IngressFailure) -> Self {
    failure.error
  }
}

impl From<IngressFailure> for TaskFailure {
  fn from(failure: IngressFailure) -> Self {
    Self {
      error: failure.error,
      retry: failure.retry,
    }
  }
}

/// Certified typed ingress boundary for Actors AddressEvent semantics (spec 5.3, 6.2).
///
/// `preflight` is read-only and covers lifecycle, funding, trigger, and required
/// placement. `notify` executes exactly once after the value movement. Host
/// producers route every movement that claims Actors ingress through this boundary;
/// movement outside the certified-producer inventory is balance-only.
pub trait AddressEventIngress<AccountId, AssetId, Balance> {
  fn preflight(event: &AddressEvent<AccountId, AssetId, Balance>) -> Result<(), IngressFailure>;
  fn notify(event: &AddressEvent<AccountId, AssetId, Balance>) -> Result<(), IngressFailure>;
}

/// No-op fallback for runtimes without Actors ingress: every movement is balance-only.
impl<AccountId, AssetId, Balance> AddressEventIngress<AccountId, AssetId, Balance> for () {
  fn preflight(_: &AddressEvent<AccountId, AssetId, Balance>) -> Result<(), IngressFailure> {
    Ok(())
  }

  fn notify(_: &AddressEvent<AccountId, AssetId, Balance>) -> Result<(), IngressFailure> {
    Ok(())
  }
}

/// Runtime authorization for actors whose stored funding policy is `RuntimePolicy`.
///
/// The pallet handles `OwnerOnly`, `SignedAllowlist`, and `AnyVerifiedIngress` itself. This adapter must
/// default deny and authorize only explicit actor/source pairs over runtime-verified provenance.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum ScalarObservationState<BlockNumber> {
  Unavailable,
  Uninitialized,
  Fresh {
    value: u128,
    observed_at: BlockNumber,
  },
  Stale,
}

/// Bounded current-scalar observation reads supplied by the host runtime.
///
/// The provider classifies availability and freshness. Actors never depends on a concrete oracle,
/// history store, producer, or off-chain service through this boundary.
pub trait ObservationProvider<FeedId, BlockNumber> {
  fn observe(
    feed: &FeedId,
    now: BlockNumber,
    max_age_blocks: u32,
  ) -> ScalarObservationState<BlockNumber>;
}

impl<FeedId, BlockNumber> ObservationProvider<FeedId, BlockNumber> for () {
  fn observe(_: &FeedId, _: BlockNumber, _: u32) -> ScalarObservationState<BlockNumber> {
    ScalarObservationState::Unavailable
  }
}

/// Host-declared reservation check for derived sovereign accounts.
///
/// Actors derives sovereign accounts from a hashed seed; the host MUST reject any derived account
/// that is already a runtime-controlled reserved account (treasury, staking pool, pallet account,
/// and so on) so a sovereign collision with host-reserved identity fails closed in O(1).
pub trait SovereignAccountPolicy<AccountId> {
  fn is_reserved(account: &AccountId) -> bool;
}

impl<AccountId> SovereignAccountPolicy<AccountId> for () {
  fn is_reserved(_: &AccountId) -> bool {
    false
  }
}

pub trait FundingAuthority<AccountId> {
  fn permits(
    actor_id: crate::ActorId,
    owner: &AccountId,
    source: Option<&AccountId>,
    provenance: Option<&crate::FundingProvenance>,
  ) -> bool;
}

impl<AccountId> FundingAuthority<AccountId> for () {
  fn permits(
    _: crate::ActorId,
    _: &AccountId,
    _: Option<&AccountId>,
    _: Option<&crate::FundingProvenance>,
  ) -> bool {
    false
  }
}

/// Asset mutations and queries.
///
/// Covers Transfer, SplitTransfer, Burn, Mint, and balance queries. `mint` is privileged — the
/// pallet rejects Mint tasks for User Actors at creation.
pub trait AssetOps<AccountId, AssetId, Balance> {
  fn transfer(
    from: &AccountId,
    to: &AccountId,
    asset: AssetId,
    amount: Balance,
  ) -> Result<(), TaskFailure>;

  fn burn(who: &AccountId, asset: AssetId, amount: Balance) -> Result<(), TaskFailure>;

  fn mint(to: &AccountId, asset: AssetId, amount: Balance) -> Result<(), TaskFailure>;

  /// Adapter-visible transferable balance before Actors-local fee reservation.
  fn balance(who: &AccountId, asset: AssetId) -> Balance;

  fn minimum_balance(asset: AssetId) -> Balance;

  /// Preflights the exact transfer consequence under unchanged ledger state.
  ///
  /// Implementations must account for source withdrawal rules and recipient depositability,
  /// including provider/reference semantics for native balances. `SplitTransfer` preflights every
  /// non-zero leg before mutation and retries the whole task on an explicitly temporary result.
  fn preflight_transfer(
    from: &AccountId,
    to: &AccountId,
    asset: AssetId,
    amount: Balance,
  ) -> Result<(), TaskFailure>;
}

/// Runtime staking operations. Required only when Stake or Unstake appears in a plan.
pub trait StakingOps<AccountId, AssetId, Balance> {
  fn stake(who: &AccountId, asset: AssetId, amount: Balance) -> Result<(), TaskFailure>;
  fn unstake(who: &AccountId, asset: AssetId, shares: Balance) -> Result<(), TaskFailure>;
  fn share_balance(who: &AccountId, asset: AssetId) -> Balance;
  fn share_asset(asset: AssetId) -> Option<AssetId>;
}

/// Actual committed swap facts returned by the runtime DEX adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DexSwapOutcome<Balance> {
  pub total_amount_in: Balance,
  pub recipient_amount_out: Balance,
}

/// Runtime DEX swap operations.
pub trait DexOps<AccountId, AssetId, Balance> {
  fn swap_exact_in(
    context: ExecutionContext<'_, AccountId>,
    asset_in: AssetId,
    asset_out: AssetId,
    amount_in: Balance,
    slippage_tolerance: Perbill,
  ) -> Result<DexSwapOutcome<Balance>, TaskFailure>;

  fn swap_exact_out(
    context: ExecutionContext<'_, AccountId>,
    asset_in: AssetId,
    asset_out: AssetId,
    amount_out: Balance,
    max_amount_in: Balance,
    slippage_tolerance: Perbill,
  ) -> Result<DexSwapOutcome<Balance>, TaskFailure>;
}

/// Runtime liquidity operations. Required when a liquidity task appears in a plan.
pub trait LiquidityOps<AccountId, AssetId, Balance> {
  /// Resolve the stable ordered asset pair for an admitted LP token. The host owns
  /// pool creation, routing, and the pair registry; an admitted LP token must not be
  /// silently reinterpreted.
  fn lp_assets(lp_asset: AssetId) -> Option<(AssetId, AssetId)>;

  fn add_liquidity(
    who: &AccountId,
    asset_a: AssetId,
    asset_b: AssetId,
    amount_a: Balance,
    amount_b: Balance,
    min_lp_out: Balance,
  ) -> Result<(Balance, Balance, Balance), TaskFailure>;

  fn remove_liquidity(
    who: &AccountId,
    lp_asset: AssetId,
    asset_a: AssetId,
    asset_b: AssetId,
    lp_amount: Balance,
    min_amount_a: Balance,
    min_amount_b: Balance,
  ) -> Result<(Balance, Balance), TaskFailure>;

  fn donate_liquidity(
    who: &AccountId,
    asset_a: AssetId,
    asset_b: AssetId,
    max_amount_a: Balance,
    max_amount_b: Balance,
    max_ratio_error: Perbill,
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

  fn preflight_transfer(
    _: &AccountId,
    _: &AccountId,
    _: AssetId,
    _: Balance,
  ) -> Result<(), TaskFailure> {
    Err(TaskFailure::permanent(DispatchError::Other(
      "AssetOps not configured",
    )))
  }
}

/// Fail-closed `DexOps` fallback for runtimes without DEX support.
impl<AccountId, AssetId, Balance> DexOps<AccountId, AssetId, Balance> for () {
  fn swap_exact_in(
    _: ExecutionContext<'_, AccountId>,
    _: AssetId,
    _: AssetId,
    _: Balance,
    _: Perbill,
  ) -> Result<DexSwapOutcome<Balance>, TaskFailure> {
    Err(TaskFailure::permanent(DispatchError::Other(
      "DexOps not configured",
    )))
  }

  fn swap_exact_out(
    _: ExecutionContext<'_, AccountId>,
    _: AssetId,
    _: AssetId,
    _: Balance,
    _: Balance,
    _: Perbill,
  ) -> Result<DexSwapOutcome<Balance>, TaskFailure> {
    Err(TaskFailure::permanent(DispatchError::Other(
      "DexOps not configured",
    )))
  }
}

/// Fail-closed `LiquidityOps` fallback for runtimes without liquidity support.
impl<AccountId, AssetId, Balance: Default> LiquidityOps<AccountId, AssetId, Balance> for () {
  fn lp_assets(_: AssetId) -> Option<(AssetId, AssetId)> {
    None
  }

  fn add_liquidity(
    _: &AccountId,
    _: AssetId,
    _: AssetId,
    _: Balance,
    _: Balance,
    _: Balance,
  ) -> Result<(Balance, Balance, Balance), TaskFailure> {
    Err(TaskFailure::permanent(DispatchError::Other(
      "LiquidityOps not configured",
    )))
  }

  fn remove_liquidity(
    _: &AccountId,
    _: AssetId,
    _: AssetId,
    _: AssetId,
    _: Balance,
    _: Balance,
    _: Balance,
  ) -> Result<(Balance, Balance), TaskFailure> {
    Err(TaskFailure::permanent(DispatchError::Other(
      "LiquidityOps not configured",
    )))
  }

  fn donate_liquidity(
    _: &AccountId,
    _: AssetId,
    _: AssetId,
    _: Balance,
    _: Balance,
    _: Perbill,
  ) -> Result<(Balance, Balance), TaskFailure> {
    Err(TaskFailure::permanent(DispatchError::Other(
      "LiquidityOps not configured",
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
