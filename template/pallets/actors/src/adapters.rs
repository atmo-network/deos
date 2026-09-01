//! Runtime adapter traits for Actors task execution.

use frame::prelude::*;
use polkadot_sdk::{
  frame_support::PalletId,
  sp_runtime::{DispatchResult, Perbill},
  sp_weights::Weight,
};

use crate::types::AddressEvent;

/// Host-owned validation boundary for System Actor contract topology.
///
/// Generic Actors does not infer product-specific effect graphs. A composing
/// runtime may reject a System contract whose declared host topology would be
/// invalid; User contracts never pass through this boundary.
pub trait SystemActorContractValidator<Contract> {
  fn validate(actor_id: crate::ActorId, contract: &Contract) -> DispatchResult;
}

impl<Contract> SystemActorContractValidator<Contract> for () {
  fn validate(_: crate::ActorId, _: &Contract) -> DispatchResult {
    Ok(())
  }
}

/// Deterministic host-owned derivation of canonical Actor custody accounts.
pub trait SovereignAccountDeriver<AccountId> {
  fn user(pallet_id: PalletId, owner: &AccountId, owner_slot: u8) -> AccountId;
  fn system(pallet_id: PalletId, actor_id: crate::ActorId) -> AccountId;
}

/// Exact certified transition produced by one committed canonical observation update.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct ObservationTransition {
  pub revision: u64,
  pub previous: Option<u128>,
  pub current: u128,
}

/// Certified bounded ingress for one externally owned observation transition.
pub trait ObservationTransitionIngress<FeedId> {
  fn note_observation_transition(
    feed: FeedId,
    transition: ObservationTransition,
    cause_provenance: crate::TriggerCauseProvenance,
  ) -> DispatchResult;
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
/// placement. `notify` executes exactly once at the host protocol's declared
/// post-movement or transactional-precommit consequence point. Host producers route
/// every movement that claims Actors ingress through this boundary; movement outside
/// the certified-producer inventory is balance-only.
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

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum CanonicalObservationState {
  Unavailable,
  Uninitialized,
  Available { value: u128, revision: u64 },
}

/// Bounded current-scalar observation reads supplied by the host runtime.
///
/// The provider classifies availability and freshness. Actors never depends on a concrete oracle,
/// history store, producer, or off-chain service through this boundary.
pub trait ObservationProvider<FeedId, BlockNumber> {
  fn current(feed: &FeedId) -> CanonicalObservationState;

  fn observe(
    feed: &FeedId,
    now: BlockNumber,
    max_age_blocks: u32,
  ) -> ScalarObservationState<BlockNumber>;
}

impl<FeedId, BlockNumber> ObservationProvider<FeedId, BlockNumber> for () {
  fn current(_: &FeedId) -> CanonicalObservationState {
    CanonicalObservationState::Unavailable
  }

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

/// Runtime-owned non-semantic authority committed by an Actor admission certificate.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct AdmissionCertificateAuthority {
  pub runtime_actor_semantics_version: u32,
  pub production_weight_identity: [u8; 32],
  pub body_geometry_version: u32,
  pub configured_bounds_commitment: [u8; 32],
  pub maximum_lifecycle_weight: Weight,
}

impl AdmissionCertificateAuthority {
  pub fn compose_production_weight_identity(
    control_identity: [u8; 32],
    effect_identity: [u8; 32],
  ) -> [u8; 32] {
    (
      *b"DEOS_ACTOR_PRODUCTION_WEIGHT",
      control_identity,
      effect_identity,
    )
      .using_encoded(frame::hashing::blake2_256)
  }
}

/// Fail-closed host boundary for the exact runtime authority used to admit Actor Contracts.
pub trait AdmissionCertificateAuthorityProvider {
  fn current() -> Option<AdmissionCertificateAuthority>;
}

impl AdmissionCertificateAuthorityProvider for () {
  fn current() -> Option<AdmissionCertificateAuthority> {
    None
  }
}

/// Physical geometry and admission bounds for current-Step control pricing.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct StepControlWeightContext {
  pub cursor: u32,
  pub steps_in_fragment: u32,
  pub opening_tail_chunks: u32,
  pub predicate_evaluation_units: u32,
  pub opening_snapshot_entries: u32,
  pub opening_predicate_results: u32,
  /// Configured bound for admission/Opening; retained count for a resumed head.
  pub funding_snapshot_entries: u32,
}

/// Runtime-owned maximum Actor-control Weight used by admission before semantic evaluation.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum StepControlPhase {
  Opening,
  Running,
  Suspended,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum StepControlOutcome {
  Continued,
  Suspended,
  Completed,
  Failed,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub enum StepControlPlacement {
  None,
  Queue,
  Wakeup,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct StepControlExecution {
  pub phase: StepControlPhase,
  pub outcome: StepControlOutcome,
  pub placement: StepControlPlacement,
  /// Nonzero Action fee collection required for this attempt to commit, separate from effects.
  pub action_fee_collected: bool,
}

pub trait StepControlWeightProvider<Step> {
  fn production_weight_identity() -> Option<[u8; 32]>;
  fn maximum_control_weight(context: StepControlWeightContext, step: &Step) -> Option<Weight>;
  fn actual_control_weight(
    context: StepControlWeightContext,
    step: &Step,
    maximum: Weight,
    execution: StepControlExecution,
  ) -> Option<Weight>;
}

impl<Step> StepControlWeightProvider<Step> for () {
  fn production_weight_identity() -> Option<[u8; 32]> {
    None
  }

  fn maximum_control_weight(_: StepControlWeightContext, _: &Step) -> Option<Weight> {
    None
  }

  fn actual_control_weight(
    _: StepControlWeightContext,
    _: &Step,
    _: Weight,
    _: StepControlExecution,
  ) -> Option<Weight> {
    None
  }
}

/// Exact post-dispatch branch owned by one current-Step Task effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskEffectExecution {
  /// Predicate evaluation, amount resolution, or funding classification ended before the Task
  /// operation was invoked. The exact effect Weight is zero.
  NotInvoked,
  /// The canonical Task operation was invoked, whether it committed or returned a typed failure.
  Invoked,
}

/// Runtime-owned maximum and valid-actual Task-effect Weight authority.
pub trait TaskEffectWeightProvider<Task> {
  fn production_weight_identity() -> Option<[u8; 32]>;
  fn maximum_effect_weight(task: &Task) -> Option<Weight>;
  fn actual_effect_weight(task: &Task, execution: TaskEffectExecution) -> Option<Weight>;
}

impl<Task> TaskEffectWeightProvider<Task> for () {
  fn production_weight_identity() -> Option<[u8; 32]> {
    None
  }

  fn maximum_effect_weight(_: &Task) -> Option<Weight> {
    None
  }

  fn actual_effect_weight(_: &Task, _: TaskEffectExecution) -> Option<Weight> {
    None
  }
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
