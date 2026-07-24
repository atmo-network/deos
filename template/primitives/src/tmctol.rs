use codec::{Decode, DecodeWithMemTracking, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

use crate::AssetKind;

/// Bounded status value for live TMCTOL guarantee projections.
#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  PartialEq,
  TypeInfo,
  Serialize,
  Deserialize,
)]
pub enum GuaranteeStatus {
  /// The bounded projection can verify the contract in current state.
  Satisfied,
  /// The required live surface is not initialized yet, but no invariant is broken.
  NotInitialized,
  /// The runtime intentionally does not claim this guarantee yet.
  NotGuaranteed,
  /// The bounded projection found an invariant violation.
  Violated,
}

/// Named TMCTOL anchor domains in the reference topology.
#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  PartialEq,
  TypeInfo,
  Serialize,
  Deserialize,
)]
pub enum AnchorDomain {
  /// Native TMCTOL anchor bucket (`aaa_id = 3`).
  Tol,
  /// BLDR TMCTOL anchor bucket (`aaa_id = 12`).
  Bldr,
}

/// Named burn-liveness domains tracked by the reference projection.
#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  PartialEq,
  TypeInfo,
  Serialize,
  Deserialize,
)]
pub enum BurnDomain {
  /// DEOS framework-level native burn through Burning Manager (`aaa_id = 0`).
  NativeBurningManager,
  /// Reference TMCTOL BLDR buyback/burn policy through Treasury B (`aaa_id = 7`).
  BldrBuyback,
}

/// Aggregate conformance state for the bounded live projection.
#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  PartialEq,
  TypeInfo,
  Serialize,
  Deserialize,
)]
pub enum TmctolConformanceStatus {
  /// All bounded fields needed for the current guarantee projection are satisfied.
  Conformant,
  /// Some live economic surface is not initialized yet.
  NotInitialized,
  /// A field is explicitly outside the current guarantee claim.
  NotGuaranteed,
  /// A hard invariant is violated.
  Violated,
}

/// Bounded read projection for a deterministic custody-only Bucket A anchor.
#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct AnchorBucketState<AccountId> {
  pub domain: AnchorDomain,
  pub aaa_id: u64,
  pub status: GuaranteeStatus,
  pub sovereign_account: AccountId,
  pub is_custody_only: bool,
  pub actor_identity_exists: bool,
  pub scheduler_state_exists: bool,
}

/// Bounded read projection for the current anchor pool behind a domain.
#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct PoolProjection<Balance> {
  pub domain: AnchorDomain,
  pub status: GuaranteeStatus,
  pub asset_a: AssetKind,
  pub asset_b: Option<AssetKind>,
  pub lp_asset_id: Option<u32>,
  pub reserve_a: Balance,
  pub reserve_b: Balance,
  pub lp_total_issuance: Balance,
  pub anchor_lp_balance: Balance,
}

/// Bounded read projection for one burn-liveness domain.
#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct BurnLivenessState<AccountId, Balance> {
  pub domain: BurnDomain,
  pub actor_id: u64,
  pub status: GuaranteeStatus,
  pub sovereign_account: AccountId,
  pub actor_exists: bool,
  pub is_system: bool,
  pub is_paused: bool,
  pub has_address_event_trigger: bool,
  pub requires_swap: bool,
  pub has_required_swap_step: bool,
  pub has_required_burn_step: bool,
  pub target_asset: AssetKind,
  pub target_balance: Balance,
  pub dust_threshold: Balance,
  pub target_balance_above_dust: bool,
}

/// Bounded read projection for Zap configuration postconditions.
#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct ZapPostconditionState<AccountId> {
  pub actor_id: u64,
  pub status: GuaranteeStatus,
  pub sovereign_account: AccountId,
  pub actor_exists: bool,
  pub is_system: bool,
  pub is_paused: bool,
  pub has_address_event_trigger: bool,
  pub configured_foreign_asset: Option<AssetKind>,
  pub configured_lp_asset: Option<AssetKind>,
  pub has_add_liquidity_step: bool,
  pub has_foreign_to_native_swap_step: bool,
  pub has_lp_split_step: bool,
  pub split_targets_all_buckets: bool,
  pub split_shares_sum_to_one: bool,
  pub split_shares_match_policy: bool,
}

/// Canonical bounded inputs needed to independently recompute the live reported floor metric.
#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct ReportedFloorInputs<Balance> {
  pub status: GuaranteeStatus,
  pub curve_exists: bool,
  pub initial_issuance: Balance,
  pub total_native_minted: Balance,
  pub current_native_issuance: Balance,
  pub anchor_lp_balance: Balance,
  pub lp_total_issuance: Balance,
  pub reserve_native: Balance,
  pub reserve_counterparty: Balance,
}

/// Complete bounded TMCTOL guarantee-state projection for the reference runtime.
#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct TmctolGuaranteeState<AccountId, Balance> {
  pub tol_anchor: AnchorBucketState<AccountId>,
  pub bldr_anchor: AnchorBucketState<AccountId>,
  pub tol_pool: PoolProjection<Balance>,
  pub bldr_pool: PoolProjection<Balance>,
  pub native_floor_inputs: ReportedFloorInputs<Balance>,
  pub native_burn_liveness: BurnLivenessState<AccountId, Balance>,
  pub bldr_buyback_liveness: BurnLivenessState<AccountId, Balance>,
  pub zap_postconditions: ZapPostconditionState<AccountId>,
  pub anchor_status: GuaranteeStatus,
  pub pool_status: GuaranteeStatus,
  pub burn_liveness_status: GuaranteeStatus,
  pub zap_status: GuaranteeStatus,
  pub conformance_status: TmctolConformanceStatus,
}

sp_api::decl_runtime_apis! {
  pub trait TmctolReadModelApi<AccountId, Balance>
  where
    AccountId: codec::Codec,
    Balance: codec::Codec,
  {
    fn tmctol_guarantee_state() -> TmctolGuaranteeState<AccountId, Balance>;
  }
}
