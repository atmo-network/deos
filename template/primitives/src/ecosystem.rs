//! Ecosystem Constants for the DEOS Reference Runtime
//!
//! This module centralizes all system-level constants, including dedicated account IDs for
//! token-driven coordination, pallet IDs, and fundamental economic parameters.
//!
//! These constants are the single source of truth for system architecture and are re-used
//! across all runtime configurations via the primitives crate.

/// Balance type alias for consistency across ecosystem
pub type Balance = u128;

/// Fixed `actor_id` values for well-known Actors addresses.
///
/// The corresponding sovereign account is derived via:
/// `Blake2_256( SCALE(ActorPalletId, b"system", actor_id) )` → `AccountId32`
///
/// IDs are sequential from `0` for all core system actors in the current launch line.
pub mod actor_ids {
  /// Burn Actor System Actors — collects DEOS Router fees and burns native tokens
  /// Created first at genesis (`actor_id = 0`)
  ///
  /// Sovereign account (ActorPalletId = `*b"actors00"`, SS58 prefix 42):
  ///   hex:  `0xe5d2c431c880d0bfbad3663b09164d86a76696dc2f137eeb502359fd28363f42`
  ///   SS58: `5HG3S6PLHrykv65Vw8j19zRaEx2Bmb37iywfo2qK3cHosGKX`
  pub const BURNING_MANAGER_ACTORS_ID: u64 = 0;

  /// Fee Sink System Actors — unified fee collection and phase-aware redistribution
  /// Created at genesis (`actor_id = 1`)
  ///
  /// Canonical role: unified collection address for 100% of transaction, Actors User-action,
  /// governance-opening, and XCM-execution fees, with no immediate author share. DEOS Router
  /// trading fees remain a separate deflationary flow to the Burn Actor. During the trusted,
  /// permissioned-collator phase, available native balance splits 50/50 between staking ingress
  /// and liquidity provisioning. Equal security/staking/liquidity thirds require permissionless
  /// collators plus a bounded security-reward settlement contract; indivisible remainder stays
  /// in Fee Sink for a later cycle.
  ///
  /// Sovereign account (ActorPalletId = `*b"actors00"`, SS58 prefix 42):
  ///   hex:  `0x7576c68c853f9f0427ae0c26043cd168ca5672bcdb221d9c0ad4ae7234d17e43`
  ///   SS58: `5Eiik51gjANLwbjZUXnVJv8pPpoTTVVic2x5sNwy8NaoVaJ9`
  pub const FEE_SINK_ACTORS_ID: u64 = 1;

  /// Liquidity Actor System Actors — transforms protocol capital into LP tokens
  /// Created second active System Actors at genesis (`actor_id = 2`)
  ///
  /// Sovereign account (ActorPalletId = `*b"actors00"`, SS58 prefix 42):
  ///   hex:  `0x643d7f4212a9f0ad63071393bc9accbcc2eabb4d32e30ebbf546bb8c3f852b70`
  ///   SS58: `5EL8uyEoZA3JQkhCC3ackopXhdujtKjHHRYVSM1BVrf5x6LW`
  pub const LIQUIDITY_ACTOR_ACTORS_ID: u64 = 2;
  /// TOL Bucket A (Anchor) — immutable LP accumulator
  /// Created at genesis (`actor_id = 3`)
  ///
  /// Sovereign account (ActorPalletId = `*b"actors00"`, SS58 prefix 42):
  ///   hex:  `0x35c4420572bfee8130a3ad5072f26d9b9ce0cf349bdb6fe1fb2c5b8fa99d4186`
  ///   SS58: `5DHChJzyAY9pz54d6PXLmScG5vhdiarfNY2VjhkP4pG8vqSs`
  pub const TOL_BUCKET_A_ACTORS_ID: u64 = 3;

  /// TOL Bucket B (Building) — gradual LP unwind for BLDR buyback
  /// Created at genesis (`actor_id = 4`)
  ///
  /// Sovereign account (ActorPalletId = `*b"actors00"`, SS58 prefix 42):
  ///   hex:  `0x8667dc4e696df85145ff65005d50f842d4aa196b2b0481681d6086d38a98c263`
  ///   SS58: `5F6w8Jd8mHTPphhHgBdUJdkTaT2hQ8mKYojDhzCre5TJqGPg`
  pub const TOL_BUCKET_B_ACTORS_ID: u64 = 4;

  /// TOL Bucket C (Capital) — gradual LP unwind for treasury operations
  /// Created at genesis (`actor_id = 5`)
  ///
  /// Sovereign account (ActorPalletId = `*b"actors00"`, SS58 prefix 42):
  ///   hex:  `0x0c90365514a0e365f883e8f4a14f18b2090e77d952d3be055847a10ef7fc8b0e`
  ///   SS58: `5CMBGiT8bLjfecCBLf7jSeWXoHKwEXtF7epoFHaLSTmxPhyp`
  pub const TOL_BUCKET_C_ACTORS_ID: u64 = 5;

  /// TOL Bucket D (Dormant) — LP held until governance decides future policy
  /// Created at genesis (`actor_id = 6`)
  ///
  /// Sovereign account (ActorPalletId = `*b"actors00"`, SS58 prefix 42):
  ///   hex:  `0x7a2cdcdf546f84c94b2de0d2db31906a3872ece0f1604816a6ff16b2f292d459`
  ///   SS58: `5Epu2U8sJbpBH1AQhc2KW6yuPA62Hst9r3zSdEHx4vS386JW`
  pub const TOL_BUCKET_D_ACTORS_ID: u64 = 6;

  /// Treasury B (Building Treasury) — paired custody lane for admitted Bucket B LP unwind
  /// Created at genesis (`actor_id = 7`), Noop by default
  ///
  /// Sovereign account (ActorPalletId = `*b"actors00"`, SS58 prefix 42):
  ///   hex:  `0x25cca60a36d1458c32e01b8d6d70aa836a98d53e13c5c51b1f8566633677d72d`
  ///   SS58: `5CvGRScqAYFFZRymun1fNJogwgUZCigd2ncmxCGvpquWy4nM`
  pub const TREASURY_B_ACTORS_ID: u64 = 7;

  /// Treasury C (Capital Treasury) — paired custody lane for admitted Bucket C LP unwind
  /// Created at genesis (`actor_id = 8`), Noop by default
  ///
  /// Sovereign account (ActorPalletId = `*b"actors00"`, SS58 prefix 42):
  ///   hex:  `0x9ab9d1e2aa163c1e0df8910b3f840824bde1c3be288be2d2c4a75910b68362fd`
  ///   SS58: `5FZaRybmQEh2eHXM95zB2tyty3vxBZPyrCYTekHu5YxuCKj8`
  pub const TREASURY_C_ACTORS_ID: u64 = 8;

  /// Treasury D (Dormant Treasury) — paired custody lane for admitted Bucket D LP unwind
  /// Created at genesis (`actor_id = 9`), Noop by default
  ///
  /// Sovereign account (ActorPalletId = `*b"actors00"`, SS58 prefix 42):
  ///   hex:  `0x1a01084c8c17375cf01299a8f492de6023bc29b78e56024510630be56b5c38f3`
  ///   SS58: `5CeoQfeA6zkG7yToYZm3L8g5gjR5aMikm4b1gVLK69CgYzsC`
  pub const TREASURY_D_ACTORS_ID: u64 = 9;

  // --- BLDR Domain (L2 Token Economy) ---

  /// BLDR Splitter — receives minted $BLDR and splits to BLDR ZM and BLDR Treasury
  /// Created at genesis (`actor_id = 10`)
  ///
  /// Sovereign account (ActorPalletId = `*b"actors00"`, SS58 prefix 42):
  ///   hex:  `0xdc201c83f1db632704da438c2fe7e6212c4a25921c48cd9294f6dde633ef1d85`
  ///   SS58: `5H3KvwhcEmU5QZNcXWjwwmtduXdrKTrR5WYZqjrJm23KK14u`
  pub const BLDR_SPLITTER_ACTORS_ID: u64 = 10;

  /// BLDR Liquidity Actor — provisions NTVE-BLDR liquidity
  /// Created at genesis (`actor_id = 11`; legacy constant name)
  ///
  /// Sovereign account (ActorPalletId = `*b"actors00"`, SS58 prefix 42):
  ///   hex:  `0x2e699b4acc26bcf078237dc13eda2470505c8bd99450269eeb7eb4c5f5472968`
  ///   SS58: `5D7ZRz4hMphgVdq9UYBA9Gtk1q2cBjKTgoDCqpBETQi6Ziq4`
  pub const BLDR_ZM_ACTORS_ID: u64 = 11;

  /// BLDR Bucket A (Anchor) — permanent LP accumulator for NTVE-BLDR pair
  /// Created at genesis (`actor_id = 12`), Noop by default
  ///
  /// Sovereign account (ActorPalletId = `*b"actors00"`, SS58 prefix 42):
  ///   hex:  `0x791ec3fe30f34d005232cdf3bb5abdc0ae14e51fe3caeb62914d35f7c81ae544`
  ///   SS58: `5EoWnoVuB925BHs9UwHUfLkcm5rSbmqzrHgFZRzY5nA4M5B6`
  pub const BLDR_BUCKET_A_ACTORS_ID: u64 = 12;

  /// BLDR Treasury — receives 50% of minted $BLDR from Splitter
  /// Created at genesis (`actor_id = 13`), Noop by default
  ///
  /// Sovereign account (ActorPalletId = `*b"actors00"`, SS58 prefix 42):
  ///   hex:  `0x07297bfba697b7593a93b6bc2c52f7dc4452d968c1e2c3badb09f2fafb8d1709`
  ///   SS58: `5CE6WsJ12vyyjAPMuvaqf2cdSQMVzAAxVjZDvXZK99VswFGe`
  pub const BLDR_TREASURY_ACTORS_ID: u64 = 13;

  /// Native Staking LP Farmer — donates NTVE/stNTVE reserves without minting LP
  /// Created at genesis (`actor_id = 14`), Noop until the canonical pool is activated
  ///
  /// Sovereign account (ActorPalletId = `*b"actors00"`, SS58 prefix 42):
  ///   hex:  `0x14292af3e9e70acb4c39cfe83317039c1f2111b475b99e660d87b16948edc339`
  ///   SS58: `5CX93X5agA9cbvbv4JKpXmR8RF9ywdLbyg6WR9qY15evri5L`
  pub const NATIVE_STAKING_LP_FARMER_ACTORS_ID: u64 = 14;
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
  /// Actors (Account Abstraction Actors) pallet ID
  ///
  /// Pallet account (SS58 prefix 42):
  ///   hex:  `0x6d6f646c6163746f727330300000000000000000000000000000000000000000`
  ///   SS58: `5EYCAe5fiQWMqjyVakD96Nwxv8toW2XYiWaTHmnmop8X9u5J`
  pub const ACTORS_PALLET_ID: &[u8; 8] = b"actors00";

  /// DEOS Router pallet ID (multi-token routing engine)
  ///
  /// Pallet account (SS58 prefix 42):
  ///   hex:  `0x6d6f646c726f7574657230300000000000000000000000000000000000000000`
  ///   SS58: `5EYCAe5j8X3dxkxG3NE9Yzf561FKmh4XYPRgrjz26bNojgZ6`
  pub const ROUTER_PALLET_ID: &[u8; 8] = b"router00";

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
  /// Local deviation threshold: if execution price differs from the stored EMA
  /// by more than this percentage, the router rejects the direct trade. This is
  /// not an external fair-price or transaction-ordering guarantee.
  pub const MAX_PRICE_DEVIATION: Perbill = Perbill::from_percent(20);

  /// Stricter reference-price deviation guard for every System Actors swap.
  pub const MAX_SYSTEM_PRICE_DEVIATION: Perbill = Perbill::from_percent(5);

  /// Maximum age of an EMA used by the System Actors reference-deviation guard.
  pub const MAX_SYSTEM_REFERENCE_AGE_BLOCKS: u32 = 100;

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

  /// DEOS Router fee (0.5%).
  ///
  /// Protocol captures 0.5% on all swaps routed through DEOS Router.
  /// XYK pool fee is 0.0% — all fee revenue flows through the Router to the Burning Manager.
  pub const DEOS_ROUTER_FEE: Perbill = Perbill::from_parts(5_000_000); // 50 bps

  /// Maximum governance-settable DEOS Router fee (1%).
  ///
  /// Bounds fee mutation so router policy cannot silently invalidate TMCTOL liveness
  /// or conservation assumptions while preserving a narrow launch-line adjustment band.
  pub const MAX_DEOS_ROUTER_FEE: Perbill = Perbill::from_percent(1);

  /// Maximum canonical LP reverse-index entries retained by DEOS Router.
  pub const MAX_ROUTER_LP_PAIRS: u32 = 500;

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
  /// Applied to genesis System Actors (Burn Actor, Liquidity Actor) to prevent
  /// resource exhaustion on repeated cycle failures.
  pub const SYSTEM_ACTORS_COOLDOWN_BLOCKS: u32 = 10;

  /// Maximum tolerated slippage for generic System Actors swap operations (5%).
  /// Maximum swap slippage tolerance for generic System Actors execution plans.
  /// Used directly as `SwapIn.slippage_tolerance` unless a runtime-specific
  /// builder chooses a stricter policy.
  pub const SYSTEM_ACTORS_MAX_SWAP_SLIPPAGE: Perbill = Perbill::from_percent(5);

  /// Maximum tolerated slippage for Liquidity Actor swap steps (3%).
  /// Liquidity Actor execution plans derive their concrete `SwapIn.slippage_tolerance`
  /// from the current native reserve depth and clamp it to this upper bound.
  pub const LIQUIDITY_ACTOR_MAX_SWAP_SLIPPAGE: Perbill = Perbill::from_percent(3);
  /// Minimum tolerated slippage for Liquidity Actor swap steps (0.25%).
  /// Deep pools tighten toward this floor instead of keeping the shallow-pool cap.
  pub const LIQUIDITY_ACTOR_MIN_SWAP_SLIPPAGE: Perbill = Perbill::from_parts(2_500_000);
  /// Native reserve depth reference for Liquidity Actor dynamic slippage.
  /// At this native reserve depth, the clamp still allows the configured max;
  /// deeper pools tighten inversely from there.
  pub const LIQUIDITY_ACTOR_SLIPPAGE_REFERENCE_NATIVE_RESERVE: Balance = 1_000 * PRECISION;
  /// Maximum accepted donation ratio error for native staking LP farming (1%).
  pub const NATIVE_STAKING_LP_DONATION_MAX_RATIO_ERROR: Perbill = Perbill::from_percent(1);

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
    assert_eq!(pallet_ids::ACTORS_PALLET_ID.len(), 8);
    assert_eq!(pallet_ids::ROUTER_PALLET_ID.len(), 8);
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
  }
}
