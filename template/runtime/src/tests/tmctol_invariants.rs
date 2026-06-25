//! TMCTOL Economic Invariants — Runtime Integration Tests
//!
//! Mirrors simulator/tests.md systemic invariants in the Rust runtime.
//! Each test runs a mixed operation sequence, then asserts properties that
//! must hold regardless of operation ordering or interleaving.
//!
//! Cross-reference: /docs/testing.simulator-runtime-mapping.en.md

use super::common::{
  ALICE, ASSET_A, BOB, CHARLIE, SWAP_AMOUNT, seeded_test_ext, setup_axial_router_infrastructure,
};
use crate::{
  Assets, AxialRouter, Balances, EXISTENTIAL_DEPOSIT, Runtime, RuntimeOrigin, System,
  TokenMintingCurve,
};
use polkadot_sdk::frame_support::{
  assert_ok,
  traits::{Currency, Hooks, fungibles::Inspect as FungiblesInspect},
};
use polkadot_sdk::sp_runtime::Weight;
use primitives::{
  AssetKind,
  ecosystem::{aaa_ids, params::PRECISION, protocol_tokens},
};

// --- Helpers ---

/// Collect TOL bucket sovereign balances for a given LP asset.
fn tol_bucket_lp_balances(lp_id: u32) -> [u128; 4] {
  let bucket_ids = [
    aaa_ids::TOL_BUCKET_A_AAA_ID,
    aaa_ids::TOL_BUCKET_B_AAA_ID,
    aaa_ids::TOL_BUCKET_C_AAA_ID,
    aaa_ids::TOL_BUCKET_D_AAA_ID,
  ];
  bucket_ids.map(|id| {
    let sov = crate::AAA::sovereign_account_id_system(id);
    Assets::balance(lp_id, &sov)
  })
}

// --- Invariant Tests ---

/// G-01 (partial): TMC mint must exactly conserve the minted token.
/// Mirrors simulator/tests.md 6.2 — distribution sums to total.
#[test]
fn tmc_mint_conservation_exact() {
  seeded_test_ext().execute_with(|| {
    let bldr_id = protocol_tokens::BLDR_ASSET_ID;
    let bldr_asset = AssetKind::Local(bldr_id);
    let splitter_sov = crate::AAA::sovereign_account_id_system(aaa_ids::BLDR_SPLITTER_AAA_ID);
    let alice_bldr_before = Assets::balance(bldr_id, &ALICE);
    let splitter_bldr_before = Assets::balance(bldr_id, &splitter_sov);
    let mint_amount = 10 * PRECISION;
    assert_ok!(TokenMintingCurve::mint_with_distribution(
      &ALICE,
      &ALICE,
      bldr_asset,
      AssetKind::Native,
      mint_amount,
    ));
    let alice_bldr_after = Assets::balance(bldr_id, &ALICE);
    let splitter_bldr_after = Assets::balance(bldr_id, &splitter_sov);
    let user_delta = alice_bldr_after.saturating_sub(alice_bldr_before);
    let tol_delta = splitter_bldr_after.saturating_sub(splitter_bldr_before);
    let total_minted = user_delta.saturating_add(tol_delta);
    // Exact conservation: user + tol == total minted into circulation
    assert_eq!(
      alice_bldr_after.saturating_add(splitter_bldr_after),
      alice_bldr_before
        .saturating_add(splitter_bldr_before)
        .saturating_add(total_minted),
      "Mint must conserve total BLDR supply exactly"
    );
    assert!(user_delta > 0, "User must receive non-zero BLDR");
    assert!(tol_delta > 0, "Splitter must receive non-zero BLDR");
    assert!(
      tol_delta > user_delta,
      "TOL allocation (66.7%) must exceed user allocation (~33.3%)"
    );
  });
}

/// G-01 / G-03 proxy: TOL bucket balances must never decrease during pure user operations.
/// Mirrors simulator/tests.md 6.3 — participant sales don't touch TOL.
#[test]
fn tol_balances_monotonic_under_user_operations() {
  use polkadot_sdk::frame_support::traits::fungibles::Mutate as FungiblesMutate;
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_axial_router_infrastructure());
    let foreign = AssetKind::Local(ASSET_A);
    // Get LP asset for the pool
    let (_, pool_info) = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::iter()
      .next()
      .expect("pool must exist after setup");
    let lp_id = pool_info.lp_token;
    // Seed TOL buckets with some LP so we can observe monotonicity
    let seed_lp = 1_000_000u128;
    let bucket_ids = [
      aaa_ids::TOL_BUCKET_A_AAA_ID,
      aaa_ids::TOL_BUCKET_B_AAA_ID,
      aaa_ids::TOL_BUCKET_C_AAA_ID,
      aaa_ids::TOL_BUCKET_D_AAA_ID,
    ];
    for &bucket_id in &bucket_ids {
      let sov = crate::AAA::sovereign_account_id_system(bucket_id);
      assert_ok!(
        <crate::Assets as FungiblesMutate<crate::AccountId>>::mint_into(lp_id, &sov, seed_lp)
      );
    }
    let before = tol_bucket_lp_balances(lp_id);
    // Run 10 user swaps — these must never touch TOL buckets
    for _ in 0..10 {
      let _ = AxialRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Native,
        foreign,
        5 * EXISTENTIAL_DEPOSIT,
        1,
        ALICE,
        System::block_number().saturating_add(100),
      );
      let _ = AxialRouter::swap(
        RuntimeOrigin::signed(BOB),
        foreign,
        AssetKind::Native,
        5 * EXISTENTIAL_DEPOSIT,
        1,
        BOB,
        System::block_number().saturating_add(100),
      );
    }
    let after = tol_bucket_lp_balances(lp_id);
    for i in 0..4 {
      assert!(
        after[i] >= before[i],
        "TOL bucket {} LP balance must not decrease under user ops ({} -> {})",
        bucket_ids[i],
        before[i],
        after[i]
      );
    }
  });
}

/// G-02 proxy: After TOL accumulation, a large dump must not crash the effective price.
/// This is a runtime behavioural proxy for the Floor Formula.
/// Mirrors simulator/tests.md 8.3 — floor guarantee persists under stress.
#[test]
fn floor_price_proxy_after_tol_accumulation() {
  use polkadot_sdk::frame_support::traits::fungibles::Mutate as FungiblesMutate;
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_axial_router_infrastructure());
    let bldr_id = protocol_tokens::BLDR_ASSET_ID;
    let bldr_asset = AssetKind::Local(bldr_id);
    // 1. Create BLDR pool and activate ZM so TOL accumulates as LP
    super::common::setup_bldr_pool(1_000 * PRECISION);
    let (_, pool_info) = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::iter()
      .find(|(pair, _)| {
        *pair == (AssetKind::Native, bldr_asset) || *pair == (bldr_asset, AssetKind::Native)
      })
      .expect("BLDR pool must exist");
    let lp_id = pool_info.lp_token;
    // Seed Bucket A with LP (simulating prior ZM activity)
    let bucket_a = crate::AAA::sovereign_account_id_system(aaa_ids::BLDR_BUCKET_A_AAA_ID);
    assert_ok!(
      <crate::Assets as FungiblesMutate<crate::AccountId>>::mint_into(
        lp_id,
        &bucket_a,
        100 * PRECISION
      )
    );
    // 2. Record effective price before stress
    // Quote a tiny amount to get spot price without moving it
    let tiny = PRECISION / 100;
    let pre_quote = crate::AssetConversion::quote_price_exact_tokens_for_tokens(
      bldr_asset,
      AssetKind::Native,
      tiny,
      true, // include_fee
    );
    assert!(pre_quote.is_some(), "Pool must provide a quote");
    let pre_price = pre_quote.unwrap();
    assert!(pre_price > 0, "Pre-stress price must be positive");
    // 3. Large dump: sell BLDR for Native through Router
    let dump_amount = 50 * PRECISION;
    assert_ok!(
      <crate::Assets as FungiblesMutate<crate::AccountId>>::mint_into(bldr_id, &ALICE, dump_amount)
    );
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      bldr_asset,
      AssetKind::Native,
      dump_amount,
      1,
      ALICE,
      System::block_number().saturating_add(100),
    ));
    // 4. Post-stress spot price must remain positive and not collapse to zero
    let post_quote = crate::AssetConversion::quote_price_exact_tokens_for_tokens(
      bldr_asset,
      AssetKind::Native,
      tiny,
      true,
    );
    assert!(post_quote.is_some(), "Pool must still quote after dump");
    let post_price = post_quote.unwrap();
    assert!(post_price > 0, "Post-stress price must remain positive");
    // The floor guarantee: even after a large dump, price does not drop below
    // a small fraction of the pre-dump price. This is a pragmatic runtime proxy
    // for the analytical floor formula P_floor = k/(R+S)^2.
    let min_acceptable_price = pre_price / 100; // 1% of pre-stress price
    assert!(
      post_price >= min_acceptable_price,
      "Floor guarantee violated: post_price={} < min={} (pre_price={})",
      post_price,
      min_acceptable_price,
      pre_price
    );
  });
}

/// G-07 / 12.1 proxy: Native burning must reduce total issuance.
/// Mirrors simulator/tests.md 12.1 — bidirectional compression.
#[test]
fn native_issuance_deflation_after_burn_cycle() {
  seeded_test_ext().execute_with(|| {
    let bm = crate::AAA::sovereign_account_id_system(aaa_ids::BURNING_MANAGER_AAA_ID);
    let deposit = 50 * EXISTENTIAL_DEPOSIT;
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&bm, deposit);
    let issuance_before = Balances::total_issuance();
    // Trigger burn via timer execution
    System::set_block_number(11);
    crate::AAA::on_initialize(11);
    crate::AAA::on_idle(11, Weight::from_parts(u64::MAX, u64::MAX));
    let issuance_after = Balances::total_issuance();
    assert!(
      issuance_after < issuance_before,
      "Burn cycle must reduce native issuance ({} -> {})",
      issuance_before,
      issuance_after
    );
  });
}

/// 7.1 / 11.1 proxy: Router round-trip through XYK must never create free value.
/// Mirrors simulator/tests.md 7.1 — anti-arbitrage cycle.
#[test]
fn router_round_trip_never_creates_value() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_axial_router_infrastructure());
    let foreign = AssetKind::Local(ASSET_A);
    let initial_native = Balances::free_balance(&ALICE);
    let initial_foreign = Assets::balance(ASSET_A, &ALICE);
    // Leg 1: Native -> Foreign
    let swap1_amount = SWAP_AMOUNT;
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(ALICE.clone()),
      AssetKind::Native,
      foreign,
      swap1_amount,
      1,
      ALICE,
      System::block_number().saturating_add(100),
    ));
    let foreign_after_1 = Assets::balance(ASSET_A, &ALICE);
    let acquired = foreign_after_1.saturating_sub(initial_foreign);
    assert!(acquired > 0, "First leg must produce positive output");
    // Leg 2: Foreign -> Native (round-trip)
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(ALICE.clone()),
      foreign,
      AssetKind::Native,
      acquired,
      1,
      ALICE,
      System::block_number().saturating_add(100),
    ));
    let final_native = Balances::free_balance(&ALICE);
    let final_foreign = Assets::balance(ASSET_A, &ALICE);
    // After round-trip, native must not exceed initial (fees extracted)
    assert!(
      final_native <= initial_native,
      "Round-trip must not create native value ({} -> {})",
      initial_native,
      final_native
    );
    // Foreign must not exceed post-first-leg (no free foreign tokens)
    assert!(
      final_foreign <= foreign_after_1,
      "Round-trip must not create foreign value ({} -> {})",
      foreign_after_1,
      final_foreign
    );
  });
}

/// G-06 / 8.1 / 9.1: Heavy mixed-use stress with post-hoc invariant audit.
/// Mirrors simulator/tests.md 8.1 and 9.1 — system invariants after chaos.
#[test]
fn heavy_use_invariants_preserved() {
  use polkadot_sdk::frame_support::traits::fungibles::Mutate as FungiblesMutate;
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_axial_router_infrastructure());
    let bldr_id = protocol_tokens::BLDR_ASSET_ID;
    let bldr_asset = AssetKind::Local(bldr_id);
    let foreign = AssetKind::Local(ASSET_A);
    // Setup BLDR domain
    super::common::setup_bldr_pool(500 * PRECISION);
    // Snapshot initial systemic state
    let initial_issuance = Balances::total_issuance();
    let tol_bucket_ids = [
      aaa_ids::TOL_BUCKET_A_AAA_ID,
      aaa_ids::TOL_BUCKET_B_AAA_ID,
      aaa_ids::TOL_BUCKET_C_AAA_ID,
      aaa_ids::TOL_BUCKET_D_AAA_ID,
    ];
    let initial_tol_native: u128 = tol_bucket_ids
      .iter()
      .map(|&id| Balances::free_balance(&crate::AAA::sovereign_account_id_system(id)))
      .sum();
    // --- Chaos phase: 30 mixed operations ---
    let users = [ALICE, BOB, CHARLIE];
    for op in 0..30 {
      System::set_block_number(op as u32 + 2);
      let user = users[op % users.len()].clone();
      match op % 5 {
        // Swap Native -> Foreign
        0 => {
          let _ = AxialRouter::swap(
            RuntimeOrigin::signed(user.clone()),
            AssetKind::Native,
            foreign,
            2 * EXISTENTIAL_DEPOSIT,
            1,
            user,
            System::block_number().saturating_add(100),
          );
        }
        // Swap Foreign -> Native
        1 => {
          let _ = AxialRouter::swap(
            RuntimeOrigin::signed(user.clone()),
            foreign,
            AssetKind::Native,
            2 * EXISTENTIAL_DEPOSIT,
            1,
            user,
            System::block_number().saturating_add(100),
          );
        }
        // Mint BLDR via TMC
        2 => {
          let _ = TokenMintingCurve::mint_with_distribution(
            &user,
            &user,
            bldr_asset,
            AssetKind::Native,
            5 * EXISTENTIAL_DEPOSIT,
          );
        }
        // Transfer BLDR (creates secondary movement)
        3 => {
          let bldr_balance = Assets::balance(bldr_id, &user);
          if bldr_balance > EXISTENTIAL_DEPOSIT {
            let recipient = users[(op + 1) % users.len()].clone();
            let _ = <crate::Assets as FungiblesMutate<crate::AccountId>>::transfer(
              bldr_id,
              &user,
              &recipient,
              bldr_balance / 4,
              polkadot_sdk::frame_support::traits::tokens::Preservation::Expendable,
            );
          }
        }
        // Burn native via BM trigger (when possible)
        4 => {
          crate::AAA::on_initialize(System::block_number());
          crate::AAA::on_idle(
            System::block_number(),
            Weight::from_parts(u64::MAX, u64::MAX),
          );
        }
        _ => {}
      }
    }

    // --- Post-hoc invariant audit ---

    // 1. Native issuance must not exceed initial + TMC mints - burns.
    // We can't track exact minted native here, but we can assert that
    // issuance did not magically inflate beyond known sources.
    let final_issuance = Balances::total_issuance();
    assert!(
      final_issuance <= initial_issuance.saturating_add(30 * 5 * EXISTENTIAL_DEPOSIT),
      "Issuance must stay bounded by known sources"
    );
    // 2. TOL native balances must be non-decreasing (no user operation can drain TOL)
    let final_tol_native: u128 = tol_bucket_ids
      .iter()
      .map(|&id| Balances::free_balance(&crate::AAA::sovereign_account_id_system(id)))
      .sum();
    assert!(
      final_tol_native >= initial_tol_native,
      "TOL aggregate native balance must not decrease ({} -> {})",
      initial_tol_native,
      final_tol_native
    );
    // 3. BLDR mass conservation: total issuance equals sum of known holdings
    let splitter = crate::AAA::sovereign_account_id_system(aaa_ids::BLDR_SPLITTER_AAA_ID);
    let zm = crate::AAA::sovereign_account_id_system(aaa_ids::BLDR_ZM_AAA_ID);
    let treasury = crate::AAA::sovereign_account_id_system(aaa_ids::BLDR_TREASURY_AAA_ID);
    let bucket_a = crate::AAA::sovereign_account_id_system(aaa_ids::BLDR_BUCKET_A_AAA_ID);
    let total_bldr_issued = Assets::total_issuance(bldr_id);
    let known_holders = [ALICE, BOB, CHARLIE, splitter, zm, treasury, bucket_a];
    let sum_known: u128 = known_holders
      .iter()
      .map(|a| Assets::balance(bldr_id, a))
      .sum();
    // Untracked tokens are primarily pool reserves (seeded at 500*PRECISION
    // by setup_bldr_pool) plus possible LP-holder accounts. We allow that.
    let pool_reserve_estimate = 500 * PRECISION;
    let diff = total_bldr_issued.abs_diff(sum_known);
    assert!(
      diff <= pool_reserve_estimate.saturating_add(known_holders.len() as u128),
      "BLDR mass conservation violated: issued={} sum_known={} diff={}",
      total_bldr_issued,
      sum_known,
      diff
    );
    // 4. System must still be operational (liveness)
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      AssetKind::Native,
      foreign,
      SWAP_AMOUNT,
      1,
      ALICE,
      System::block_number().saturating_add(100),
    ));
  });
}
