//! Ecosystem Constants for the DEOS Reference Runtime
//!
//! This module centralizes all system-level constants, including dedicated account IDs for
//! token-driven coordination, pallet IDs, and fundamental economic parameters.
//!
//! These constants are the single source of truth for system architecture and are re-used
//! across all runtime configurations via the primitives crate.

/// Balance type alias for consistency across ecosystem
pub type Balance = u128;

/// Fixed `aaa_id` values for well-known AAA addresses.
///
/// The corresponding sovereign account is derived via:
/// `Blake2_256( SCALE(AaaPalletId, b"system", aaa_id) )` → `AccountId32`
///
/// IDs are sequential from `0` for all core system actors in the current launch line.
pub mod aaa_ids {
  /// Burn Actor System AAA — collects Axial Router fees and burns native tokens
  /// Created first at genesis (`aaa_id = 0`)
  ///
  /// Sovereign account (AaaPalletId = `*b"aaactor0"`, SS58 prefix 42):
  ///   hex:  `0xeba61f8494ba498cb84ce3b771bc3c193dbd82f9a999153a55c383349f6e512e`
  ///   SS58: `5HPgTa8GLrmzMDktPEWmuC82WtipKSibwd9C2pUQnESn4nAv`
  pub const BURNING_MANAGER_AAA_ID: u64 = 0;

  /// Fee Sink System AAA — unified fee collection and phase-aware redistribution
  /// Created at genesis (`aaa_id = 1`)
  ///
  /// Canonical role: unified collection address for 100% of transaction, AAA User-action,
  /// governance-opening, and XCM-execution fees, with no immediate author share. Axial Router
  /// trading fees remain a separate deflationary flow to the Burn Actor. During the trusted,
  /// permissioned-collator phase, available native balance splits 50/50 between staking ingress
  /// and liquidity provisioning. Equal security/staking/liquidity thirds require permissionless
  /// collators plus a bounded security-reward settlement contract; indivisible remainder stays
  /// in Fee Sink for a later cycle.
  ///
  /// Sovereign account (AaaPalletId = `*b"aaactor0"`, SS58 prefix 42):
  ///   hex:  `0xab373631522954b038699419fadc732893dff1230239bc30fbe17bf5fb12f084`
  ///   SS58: `5FwCSs6WuW2tTv7uQFRB1o4rjmPQsgE6PesjKUUbroxfzKKh`
  pub const FEE_SINK_AAA_ID: u64 = 1;

  /// Liquidity Actor System AAA — transforms protocol capital into LP tokens
  /// Created second active System AAA at genesis (`aaa_id = 2`)
  ///
  /// Sovereign account (AaaPalletId = `*b"aaactor0"`, SS58 prefix 42):
  ///   hex:  `0xb136dc3f6dba4aac24a8c9f8be3c7b20e26b08422803b6999b7cd019c4ca50ab`
  ///   SS58: `5G54dUVans8Rvnn1qdTea3fQ28osh8T7ijaWbi3gygm9sa7C`
  pub const LIQUIDITY_ACTOR_AAA_ID: u64 = 2;
  /// Legacy alias for [`LIQUIDITY_ACTOR_AAA_ID`] (pre-AAA-abstraction Zap Manager name).
  pub const ZAP_MANAGER_AAA_ID: u64 = LIQUIDITY_ACTOR_AAA_ID;

  /// TOL Bucket A (Anchor) — immutable LP accumulator
  /// Created at genesis (`aaa_id = 3`)
  ///
  /// Sovereign account (AaaPalletId = `*b"aaactor0"`, SS58 prefix 42):
  ///   hex:  `0x6f9a5aa8cd9ba27b2e69f1bac1c521d2ffde543275ebd787da11dbd131c50d25`
  ///   SS58: `5Eb32Qkj9FpPMUXZMNreJzRESQRbYQWwiKXK4zf9VXifTEqX`
  pub const TOL_BUCKET_A_AAA_ID: u64 = 3;

  /// TOL Bucket B (Building) — gradual LP unwind for BLDR buyback
  /// Created at genesis (`aaa_id = 4`)
  ///
  /// Sovereign account (AaaPalletId = `*b"aaactor0"`, SS58 prefix 42):
  ///   hex:  `0x03699bb4549d77d91390fc161867ccd3ef97d4f305f01757708905c84cb7d882`
  ///   SS58: `5C9BNb4AoxDngwC6nzu8SEtAEbtGHiKeBjzJwgUewA9qDNL3`
  pub const TOL_BUCKET_B_AAA_ID: u64 = 4;

  /// TOL Bucket C (Capital) — gradual LP unwind for treasury operations
  /// Created at genesis (`aaa_id = 5`)
  ///
  /// Sovereign account (AaaPalletId = `*b"aaactor0"`, SS58 prefix 42):
  ///   hex:  `0x313e7fb07ed6681741b54c3d421f8c261027048e2a9b0668e1058654d369de29`
  ///   SS58: `5DBGmawvmUvHAg9e2A4bcwZm3NiGX5KE5sPCKepN36SMJvfX`
  pub const TOL_BUCKET_C_AAA_ID: u64 = 5;

  /// TOL Bucket D (Dormant) — LP held until governance decides future policy
  /// Created at genesis (`aaa_id = 6`)
  ///
  /// Sovereign account (AaaPalletId = `*b"aaactor0"`, SS58 prefix 42):
  ///   hex:  `0xd23baab9890a6990ff23e7ad7ab9d1ad34712d7add2344917d110e3cec5b9242`
  ///   SS58: `5GpMdwY6iMiA8LRUczsZH6p9WoxN4rX15U7FJWbeqTqTrPLX`
  pub const TOL_BUCKET_D_AAA_ID: u64 = 6;

  /// Treasury B (Building Treasury) — paired custody lane for admitted Bucket B LP unwind
  /// Created at genesis (`aaa_id = 7`), Noop by default
  ///
  /// Sovereign account (AaaPalletId = `*b"aaactor0"`, SS58 prefix 42):
  ///   hex:  `0xa027809984f38031e61246efe8ad1f28ddacd9870f6bed081560089c15f9b966`
  ///   SS58: `5FghFeZDxtGWmvASpM4etxnYtreW9yamSx1Pwh1aGYkny2uv`
  pub const TREASURY_B_AAA_ID: u64 = 7;

  /// Treasury C (Capital Treasury) — paired custody lane for admitted Bucket C LP unwind
  /// Created at genesis (`aaa_id = 8`), Noop by default
  ///
  /// Sovereign account (AaaPalletId = `*b"aaactor0"`, SS58 prefix 42):
  ///   hex:  `0xcae77c85e5665e0cbe994898429478d3facf4c29a9b7539902f95ad7b3b4bf9b`
  ///   SS58: `5GekJ6zNwu6ABqhpcagnxbPmP6UtJ1gUKdvJywZKugWkCLhe`
  pub const TREASURY_C_AAA_ID: u64 = 8;

  /// Treasury D (Dormant Treasury) — paired custody lane for admitted Bucket D LP unwind
  /// Created at genesis (`aaa_id = 9`), Noop by default
  ///
  /// Sovereign account (AaaPalletId = `*b"aaactor0"`, SS58 prefix 42):
  ///   hex:  `0xc81b0eb40aea260eb09b950cfbe2c43f9be1dc73bf62cf081c376cff4bdae0ca`
  ///   SS58: `5Gb5UKWyYyyttHG3GCsyEhN2Qtb92auewWLZzPaQCvp1RHaj`
  pub const TREASURY_D_AAA_ID: u64 = 9;

  // --- BLDR Domain (L2 Token Economy) ---

  /// BLDR Splitter — receives minted $BLDR and splits to BLDR ZM and BLDR Treasury
  /// Created at genesis (`aaa_id = 10`)
  ///
  /// Sovereign account (AaaPalletId = `*b"aaactor0"`, SS58 prefix 42):
  ///   hex:  `0x8a420d09aa8842c9075deefab7791be5e9f9471bc68baa8c926128cfc29b6962`
  ///   SS58: `5FBz5y9kWN7ArW1w5TZiCLbszGmG3FmCSx6njj9w7VEuiK8N`
  pub const BLDR_SPLITTER_AAA_ID: u64 = 10;

  /// BLDR Liquidity Actor — provisions NTVE-BLDR liquidity
  /// Created at genesis (`aaa_id = 11`; legacy constant name)
  ///
  /// Sovereign account (AaaPalletId = `*b"aaactor0"`, SS58 prefix 42):
  ///   hex:  `0x6324e98949d19dbe10162a939df82b28368bef743a14aa8ce0a3d9a02d567221`
  ///   SS58: `5EJhZc6rdqBKzZcJXfjeMwTaQvYsyTF9YJS39sWr1HEuEy17`
  pub const BLDR_ZM_AAA_ID: u64 = 11;

  /// BLDR Bucket A (Anchor) — permanent LP accumulator for NTVE-BLDR pair
  /// Created at genesis (`aaa_id = 12`), Noop by default
  ///
  /// Sovereign account (AaaPalletId = `*b"aaactor0"`, SS58 prefix 42):
  ///   hex:  `0xb31a379c50afe1ba1ad65f1afafaf51df1c40ed2b6c08e9faf1a1ac2caf026de`
  ///   SS58: `5G7YDX7r2L8q5Wn73dNyhp8cnbpP3sTGUcRW6Eos5Urrxax8`
  pub const BLDR_BUCKET_A_AAA_ID: u64 = 12;

  /// BLDR Treasury — receives 50% of minted $BLDR from Splitter
  /// Created at genesis (`aaa_id = 13`), Noop by default
  ///
  /// Sovereign account (AaaPalletId = `*b"aaactor0"`, SS58 prefix 42):
  ///   hex:  `0x3a1bedf666c4852432a75dc0099fec586a02b813acb4457c9d4b150a03bdce45`
  ///   SS58: `5DNtvy5YymuvPBM6Wk8ADHs9ggLK2gjEZoaSoeM3aHLykNKG`
  pub const BLDR_TREASURY_AAA_ID: u64 = 13;

  /// Native Staking LP Farmer — donates NTVE/stNTVE reserves without minting LP
  /// Created at genesis (`aaa_id = 14`), Noop until the canonical pool is activated
  pub const NATIVE_STAKING_LP_FARMER_AAA_ID: u64 = 14;
}

/// Protocol-native token asset IDs.
///
/// These tokens are built into the TMCTOL protocol itself and are
/// pre-registered at genesis. They use `AssetKind::Local(id)` in
/// the low ID range (outside any bitmask prefix).
pub mod protocol_tokens {
  use crate::assets::{MASK_INDEX, TYPE_PROTOCOL};
  /// $VETO governance token — `AssetKind::Local(0x5000_0001)`
  pub const VETO_ASSET_ID: u32 = TYPE_PROTOCOL | (1 & MASK_INDEX);

  /// $BLDR builder incentive token — `AssetKind::Local(0x5000_0002)`
  pub const BLDR_ASSET_ID: u32 = TYPE_PROTOCOL | (2 & MASK_INDEX);
}

/// Pallet identifiers for deriving pallet-owned accounts
///
/// Pallet accounts are derived via `PalletId::into_account_truncating()`
/// For `AccountId32`, this yields `("modl", pallet_id, ..zeroes)`
///
/// All addresses below are for SS58 prefix 42
/// Convention: lowercase ASCII, exactly 8 bytes, no legacy `py/` prefix
pub mod pallet_ids {
  /// AAA (Account Abstraction Actors) pallet ID
  ///
  /// Pallet account (SS58 prefix 42):
  ///   hex:  `0x6d6f646c61616163746f72300000000000000000000000000000000000000000`
  ///   SS58: `5EYCAe5fiK3ZpinaPEDXwvtT6tFp5gBL16S5vyt4TYmgLaT1`
  pub const AAA_PALLET_ID: &[u8; 8] = b"aaactor0";

  /// Axial Router pallet ID (multi-token routing engine)
  ///
  /// Pallet account (SS58 prefix 42):
  ///   hex:  `0x6d6f646c617869616c7274300000000000000000000000000000000000000000`
  ///   SS58: `5EYCAe5fjMgntj8Tch49FZ3RXMR1XiQbrSA1z2oYgQAiXukN`
  pub const AXIAL_ROUTER_PALLET_ID: &[u8; 8] = b"axialrt0";

  /// TMC pallet ID (token minting curve)
  ///
  /// Pallet account (SS58 prefix 42):
  ///   hex:  `0x6d6f646c746d6375727665300000000000000000000000000000000000000000`
  ///   SS58: `5EYCAe5jXfhqLzusixrt2Ch3ZateFvpRuiGFejB9K4oodMC1`
  pub const TMC_PALLET_ID: &[u8; 8] = b"tmcurve0";

  /// Asset conversion pallet (Uniswap V2-like DEX)
  ///
  /// Pallet account (SS58 prefix 42):
  ///   hex:  `0x6d6f646c6173636f6e7630300000000000000000000000000000000000000000`
  ///   SS58: `5EYCAe5fj8TfgHAG4378PT1xXraozf8JqHQAHvimgfg7HNR7`
  pub const ASSET_CONVERSION_PALLET_ID: &[u8; 8] = b"asconv00";

  /// Asset Registry pallet ID
  ///
  /// Pallet account (SS58 prefix 42):
  ///   hex:  `0x6d6f646c61737365747265670000000000000000000000000000000000000000`
  ///   SS58: `5EYCAe5fj8dBvWz8Un9gAkZKFqRiKaxdbjQHMLr33ZUfT78H`
  pub const ASSET_REGISTRY_PALLET_ID: &[u8; 8] = b"assetreg";

  /// Staking pallet ID
  pub const STAKING_PALLET_ID: &[u8; 8] = b"staking0";
}

/// Ecosystem parameters defining mathematical constants and thresholds.
///
/// These parameters are global across all pallets and coordinate the
/// economic properties of the system.
pub mod params {
  use super::Balance;
  use sp_arithmetic::Perbill;

  /// Precision scalar for all mathematical calculations (10^12).
  ///
  /// All price curves, fee calculations, and economic metrics use this precision
  /// to maintain consistency and prevent rounding errors.
  pub const PRECISION: Balance = 1_000_000_000_000;

  /// EMA oracle half-life in blocks (~10 minutes at 6s/block).
  ///
  /// Controls the responsiveness of the price oracle to market changes.
  /// Higher values create more stable (but lagged) prices; lower values react faster.
  pub const EMA_HALF_LIFE_BLOCKS: u32 = 100;

  /// Maximum allowed price deviation from EMA price (20%).
  ///
  /// Circuit breaker threshold: if market price deviates from the oracle price
  /// by more than this percentage, the router rejects the trade to prevent
  /// manipulation or anomalies.
  pub const MAX_PRICE_DEVIATION: Perbill = Perbill::from_percent(20);

  /// Maximum hops in multi-asset routing paths (3).
  ///
  /// Limits routing graph complexity and prevents excessive gas consumption
  /// on complex asset paths (e.g., ASSET_A -> Native -> ASSET_B -> ASSET_C).
  pub const MAX_HOPS: u32 = 3;

  /// TMC user allocation ratio (33.3% of minted tokens).
  ///
  /// When tokens are minted via TMC, 33.3% go directly to the user,
  /// and 66.6% go to the resolved liquidity actor for provisioning.
  pub const TMC_USER_ALLOCATION: Perbill = Perbill::from_parts(333_333_333);

  /// TMC liquidity-actor allocation ratio (66.6% of minted tokens).
  pub const TMC_ZAP_ALLOCATION: Perbill = Perbill::from_parts(666_666_667);

  /// Axial Router fee (0.5%).
  ///
  /// Protocol captures 0.5% on all swaps routed through the Axial Router.
  /// XYK pool fee is 0.0% — all fee revenue flows through the Router to the Burning Manager.
  pub const AXIAL_ROUTER_FEE: Perbill = Perbill::from_parts(5_000_000); // 50 bps

  /// Maximum governance-settable Axial Router fee (1%).
  ///
  /// Bounds fee mutation so router policy cannot silently invalidate TMCTOL liveness
  /// or conservation assumptions while preserving a narrow launch-line adjustment band.
  pub const MAX_AXIAL_ROUTER_FEE: Perbill = Perbill::from_percent(1);

  /// TMC curve slope parameter (0.000001 per token).
  ///
  /// Controls the rate at which the price increases as more tokens are minted.
  /// Steeper slopes create more aggressive price escalation.
  pub const TMC_SLOPE_PARAMETER: Balance = PRECISION / 1_000_000; // 0.000001 in PRECISION units

  /// TOL bucket allocation target - Bucket A (50%)
  pub const TOL_BUCKET_A_ALLOCATION: Perbill = Perbill::from_parts(500_000_000);

  /// TOL bucket allocation target - Bucket B (16.67%)
  pub const TOL_BUCKET_B_ALLOCATION: Perbill = Perbill::from_parts(166_666_667);

  /// TOL bucket allocation target - Bucket C (16.67%)
  pub const TOL_BUCKET_C_ALLOCATION: Perbill = Perbill::from_parts(166_666_667);

  /// TOL bucket allocation target - Bucket D (16.66%)
  pub const TOL_BUCKET_D_ALLOCATION: Perbill = Perbill::from_parts(166_666_666);

  /// Minimum swap amount for foreign assets (1.0 in base units).
  ///
  /// Prevents spam and dust attacks on the router by enforcing a minimum
  /// transaction size.
  pub const MIN_SWAP_FOREIGN: Balance = PRECISION; // 1.0

  /// TOL maximum price deviation (20%).
  pub const TOL_MAX_PRICE_DEVIATION: Perbill = Perbill::from_percent(20);

  /// TOL minimum swap foreign amount (1.0).
  pub const TOL_MIN_SWAP_FOREIGN: Balance = MIN_SWAP_FOREIGN; // 1.0

  /// Default cooldown for System actors (10 blocks ≈ 1 minute).
  ///
  /// Applied to genesis System AAAs (Burn Actor, Liquidity Actor) to prevent
  /// resource exhaustion on repeated cycle failures.
  pub const SYSTEM_AAA_COOLDOWN_BLOCKS: u32 = 10;

  /// Maximum tolerated slippage for generic System AAA swap operations (5%).
  /// Maximum swap slippage tolerance for generic System AAA execution plans.
  /// Used directly as `SwapExactIn.slippage_tolerance` unless a runtime-specific
  /// builder chooses a stricter policy.
  pub const SYSTEM_AAA_MAX_SWAP_SLIPPAGE: Perbill = Perbill::from_percent(5);

  /// Maximum tolerated slippage for Liquidity Actor swap steps (3%).
  /// Liquidity Actor execution plans derive their concrete `SwapExactIn.slippage_tolerance`
  /// from the current native reserve depth and clamp it to this upper bound.
  pub const LIQUIDITY_ACTOR_MAX_SWAP_SLIPPAGE: Perbill = Perbill::from_percent(3);
  /// Legacy alias for [`LIQUIDITY_ACTOR_MAX_SWAP_SLIPPAGE`].
  pub const ZAP_MANAGER_MAX_SWAP_SLIPPAGE: Perbill = LIQUIDITY_ACTOR_MAX_SWAP_SLIPPAGE;

  /// Minimum tolerated slippage for Liquidity Actor swap steps (0.25%).
  /// Deep pools tighten toward this floor instead of keeping the shallow-pool cap.
  pub const LIQUIDITY_ACTOR_MIN_SWAP_SLIPPAGE: Perbill = Perbill::from_parts(2_500_000);
  /// Legacy alias for [`LIQUIDITY_ACTOR_MIN_SWAP_SLIPPAGE`].
  pub const ZAP_MANAGER_MIN_SWAP_SLIPPAGE: Perbill = LIQUIDITY_ACTOR_MIN_SWAP_SLIPPAGE;

  /// Native reserve depth reference for Liquidity Actor dynamic slippage.
  /// At this native reserve depth, the clamp still allows the configured max;
  /// deeper pools tighten inversely from there.
  pub const LIQUIDITY_ACTOR_SLIPPAGE_REFERENCE_NATIVE_RESERVE: Balance = 1_000 * PRECISION;
  /// Legacy alias for [`LIQUIDITY_ACTOR_SLIPPAGE_REFERENCE_NATIVE_RESERVE`].
  pub const ZAP_MANAGER_SLIPPAGE_REFERENCE_NATIVE_RESERVE: Balance =
    LIQUIDITY_ACTOR_SLIPPAGE_REFERENCE_NATIVE_RESERVE;

  /// Maximum accepted donation ratio error for native staking LP farming (1%).
  pub const NATIVE_STAKING_LP_DONATION_MAX_RATIO_ERROR: Perbill = Perbill::from_percent(1);

  /// Burning Manager polling interval (10 blocks ≈ 1 minute).
  pub const BURNING_MANAGER_POLL_BLOCKS: u32 = 10;

  /// Minimum foreign balance for BM to attempt a swap (prevents dust churn)
  pub const BURNING_MANAGER_DUST_THRESHOLD: Balance = PRECISION; // 1.0

  // --- BLDR Domain Parameters ---

  /// BLDR Splitter: share directed to BLDR ZM (50%)
  pub const BLDR_SPLITTER_ZM_SHARE: Perbill = Perbill::from_percent(50);

  /// BLDR Splitter: share directed to BLDR Treasury (50%)
  pub const BLDR_SPLITTER_TREASURY_SHARE: Perbill = Perbill::from_percent(50);

  // --- Treasury B: BLDR Buyback & Burn ---

  /// Treasury B buyback cadence (600 blocks ≈ 1 hour at 6s/block)
  pub const TREASURY_B_BUYBACK_EVERY_BLOCKS: u32 = 600;

  /// Treasury B buyback amount: fraction of current NTVE balance per execution.
  /// Target: ~1%/day. At hourly cadence (24 executions): (1-r)^24=0.99 → r≈0.0418%
  pub const TREASURY_B_BUYBACK_PCT: Perbill = Perbill::from_parts(418_000); // ~0.0418%
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn pallet_ids_are_correct_length() {
    assert_eq!(pallet_ids::AAA_PALLET_ID.len(), 8);
    assert_eq!(pallet_ids::AXIAL_ROUTER_PALLET_ID.len(), 8);
    assert_eq!(pallet_ids::TMC_PALLET_ID.len(), 8);
    assert_eq!(pallet_ids::ASSET_CONVERSION_PALLET_ID.len(), 8);
    assert_eq!(pallet_ids::ASSET_REGISTRY_PALLET_ID.len(), 8);
    assert_eq!(pallet_ids::STAKING_PALLET_ID.len(), 8);
  }

  #[test]
  fn parameter_allocations_sum_to_one_billion() {
    let user_zap_sum =
      params::TMC_USER_ALLOCATION.deconstruct() + params::TMC_ZAP_ALLOCATION.deconstruct();
    assert_eq!(
      user_zap_sum, 1_000_000_000,
      "TMC allocations must sum to 100%"
    );

    let bucket_sum = params::TOL_BUCKET_A_ALLOCATION.deconstruct()
      + params::TOL_BUCKET_B_ALLOCATION.deconstruct()
      + params::TOL_BUCKET_C_ALLOCATION.deconstruct()
      + params::TOL_BUCKET_D_ALLOCATION.deconstruct();
    assert_eq!(
      bucket_sum, 1_000_000_000,
      "TOL bucket allocations must sum to 100%"
    );

    let bldr_splitter_sum = params::BLDR_SPLITTER_ZM_SHARE.deconstruct()
      + params::BLDR_SPLITTER_TREASURY_SHARE.deconstruct();
    assert_eq!(
      bldr_splitter_sum, 1_000_000_000,
      "BLDR splitter shares must sum to 100%"
    );
  }

  #[test]
  fn precision_is_standard() {
    assert_eq!(params::PRECISION, 1_000_000_000_000);
  }

  #[test]
  fn zap_slippage_bounds_are_ordered() {
    assert!(
      params::LIQUIDITY_ACTOR_MIN_SWAP_SLIPPAGE.deconstruct()
        <= params::LIQUIDITY_ACTOR_MAX_SWAP_SLIPPAGE.deconstruct()
    );
    assert!(params::LIQUIDITY_ACTOR_SLIPPAGE_REFERENCE_NATIVE_RESERVE >= params::PRECISION);
    assert_eq!(
      params::ZAP_MANAGER_MAX_SWAP_SLIPPAGE,
      params::LIQUIDITY_ACTOR_MAX_SWAP_SLIPPAGE
    );
  }
}
