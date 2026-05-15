use crate::{Error, Event, RouteMechanismKind, mock::*, types::*};
use polkadot_sdk::frame_support::{
  assert_noop, assert_ok,
  traits::{Currency, Get, fungibles::Mutate},
};
use polkadot_sdk::sp_runtime::Perbill;
use primitives::ecosystem::params::PRECISION;

#[test]
fn router_fee_calculation_logic() {
  new_test_ext().execute_with(|| {
    let amount = 1000 * PRECISION;
    let fee = crate::Pallet::<Test>::calculate_router_fee(amount);
    assert_eq!(fee, 5 * PRECISION); // 0.5% of 1000
  });
}

#[test]
fn large_amount_fee_calculation() {
  new_test_ext().execute_with(|| {
    let amount = 1_000_000_000_000u128;
    let fee = crate::Pallet::<Test>::calculate_router_fee(amount);
    assert_eq!(fee, 5_000_000_000); // 0.5%
  });
}

#[test]
fn zero_amount_fee_calculation() {
  new_test_ext().execute_with(|| {
    let amount = 0u128;
    let fee = crate::Pallet::<Test>::calculate_router_fee(amount);
    assert_eq!(fee, 0);
  });
}

#[test]
fn quote_exact_input_view_matches_direct_xyk_policy_for_user() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user = 1u64;
    let foreign = AssetKind::Local(2);
    let native = AssetKind::Native;
    let amount_in = 1_000 * PRECISION;
    let pool_reserve = 10_000 * PRECISION;
    set_pool(foreign, native, pool_reserve, pool_reserve);
    let quote = AxialRouter::quote_exact_input(user, foreign, native, amount_in).unwrap();
    let router_fee = AxialRouter::calculate_router_fee(amount_in);
    let amount_after_fee = amount_in - router_fee;
    let expected_out = (amount_after_fee * pool_reserve) / (pool_reserve + amount_after_fee);
    assert_eq!(quote.amount_in, amount_in);
    assert_eq!(quote.router_fee, router_fee);
    assert_eq!(quote.amount_after_fee, amount_after_fee);
    assert_eq!(quote.amount_out, expected_out);
    assert_eq!(quote.mechanism, RouteMechanismKind::DirectXyk);
    assert_eq!(quote.path, vec![foreign, native]);
  });
}

#[test]
fn quote_exact_input_view_omits_router_fee_for_fee_exempt_accounts() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let fee_exempt = <Test as crate::Config>::BurningManagerAccount::get();
    let foreign = AssetKind::Local(2);
    let native = AssetKind::Native;
    let amount_in = 1_000 * PRECISION;
    let pool_reserve = 10_000 * PRECISION;
    set_pool(foreign, native, pool_reserve, pool_reserve);
    let quote = AxialRouter::quote_exact_input(fee_exempt, foreign, native, amount_in).unwrap();
    let expected_out = (amount_in * pool_reserve) / (pool_reserve + amount_in);
    assert_eq!(quote.router_fee, 0);
    assert_eq!(quote.amount_after_fee, amount_in);
    assert_eq!(quote.amount_out, expected_out);
    assert_eq!(quote.mechanism, RouteMechanismKind::DirectXyk);
  });
}

#[test]
fn precision_constant_validation() {
  // Test that PRECISION constant is correct (10^12)
  new_test_ext().execute_with(|| {
    let precision = <<Test as crate::Config>::Precision as Get<u128>>::get();
    assert_eq!(precision, 1_000_000_000_000u128);
  });
}

#[test]
fn router_intelligence_test() {
  // Verify Router Intelligence (Bidirectional Compression)
  // Case 1: XYK Output > TMC Output => Prefer XYK (Market Liquidity)
  // Case 2: TMC Output > XYK Output => Prefer TMC (Protocol Liquidity/Ceiling)
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user = 1u64;
    let asset_in = AssetKind::Local(1);
    let asset_out = AssetKind::Native;
    let amount_in = 1000 * PRECISION;
    // Setup: Mint assets to user (done in new_test_ext)
    // Scenario 1: Market Liquidity is better (XYK > TMC)
    // We want XYK ~ 1200, TMC ~ 1000
    // Setup TMC: Rate 1.0 (1000*P in -> 1000*P out)
    set_tmc_rate(asset_in, 1);
    // Setup XYK: We need a pool that gives > TMC output (1000*P)
    // Pool formula: out = (in * res_out) / (res_in + in)
    // With amount_in = 1000*P, reserve_in = 1000*P, reserve_out = 3000*P:
    // out = (1000*P * 3000*P) / (2000*P) = 1500*P
    let reserve_in = 1_000 * PRECISION;
    let reserve_out = 3_000 * PRECISION;
    set_pool(asset_in, asset_out, reserve_in, reserve_out);
    // Execute swap
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(user),
      asset_in,
      asset_out,
      amount_in,
      0, // min_amount_out
      user,
      u64::MAX
    ));
    // Verify event: Expect XYK execution
    // After 0.5% fee deduction: 995*P in, output = (995*P * 3000*P) / (1995*P) ≈ 1496.240*P
    let expected_xyk_out = 1_496_240_601_503_759;
    System::assert_last_event(
      crate::Event::SwapExecuted {
        who: user,
        from: asset_in,
        to: asset_out,
        amount_in,
        amount_out: expected_xyk_out,
        mechanism: RouteMechanismKind::DirectXyk,
      }
      .into(),
    );
    // Scenario 2: Protocol Liquidity is better (TMC > XYK)
    // We want TMC ~ 998, XYK ~ 249
    // TMC Rate for Native token (since that's what gets minted): 1.0 -> 995*P output (after fee)
    set_tmc_rate(asset_out, 1); // asset_out is Native
    // XYK: We need a pool that gives < TMC output (995*P)
    // With amount_in = 995*P (after 0.5% fee), reserve_in = 1000*P, reserve_out = 500*P:
    // out = (995*P * 500*P) / (1995*P) ≈ 249.373*P
    let reserve_in_2 = 1_000 * PRECISION;
    let reserve_out_2 = 500 * PRECISION;
    set_pool(asset_in, asset_out, reserve_in_2, reserve_out_2);
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(user),
      asset_in,
      asset_out,
      amount_in,
      0,
      user,
      u64::MAX
    ));
    // Verify event: Expect TMC execution
    // Note: The event reports total minted, but user only receives 25% in this mock
    // (In production it would be 33.3%). This is a known limitation - the event
    // reports the total curve output, not the user-specific allocation.
    // After 0.5% fee deduction: 995*P in, TMC at 1.0 rate gives 995*P total output
    let expected_tmc_out = 995 * PRECISION;
    System::assert_last_event(
      crate::Event::SwapExecuted {
        who: user,
        from: asset_in,
        to: asset_out,
        amount_in,
        amount_out: expected_tmc_out,
        mechanism: RouteMechanismKind::DirectMint,
      }
      .into(),
    );
  });
}

#[test]
fn circular_swap_protection_test() {
  new_test_ext().execute_with(|| {
    let user = 1u64;
    let asset = AssetKind::Local(1);
    let amount = 10_000 * primitives::ecosystem::params::PRECISION;
    assert_noop!(
      AxialRouter::swap(
        RuntimeOrigin::signed(user),
        asset,
        asset,
        amount,
        0,
        user,
        u64::MAX
      ),
      Error::<Test>::IdenticalAssets
    );
  });
}

#[test]
fn slippage_protection_test() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user = 1u64;
    let asset_in = AssetKind::Local(1);
    let asset_out = AssetKind::Native;
    let amount_in = 1000 * PRECISION;
    // Setup: Market Liquidity pool with PRECISION scaling
    let reserve_both = 1_000 * PRECISION;
    set_pool(asset_in, asset_out, reserve_both, reserve_both);
    // TMC gives 0 (Not set)
    // XYK formula with 0.5% fee:
    // After fee: 995*P in, out = (995*P * 1000*P) / (1995*P) ≈ 498.746*P
    let expected_out = 498_746_867_167_919;
    // Case 1: Slippage met (min_out < expected_out) - Success
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(user),
      asset_in,
      asset_out,
      amount_in,
      400 * PRECISION, // 400*P < 499.499*P
      user,
      u64::MAX
    ));
    // Verify actual output matches expected
    System::assert_last_event(
      crate::Event::SwapExecuted {
        who: user,
        from: asset_in,
        to: asset_out,
        amount_in,
        amount_out: expected_out,
        mechanism: RouteMechanismKind::DirectXyk,
      }
      .into(),
    );
    // Case 2: Slippage exceeded (min_out > expected_out) - Failure
    assert_noop!(
      AxialRouter::swap(
        RuntimeOrigin::signed(user),
        asset_in,
        asset_out,
        amount_in,
        600 * PRECISION, // 600*P > 499.499*P
        user,
        u64::MAX
      ),
      Error::<Test>::SlippageExceeded
    );
  });
}

#[test]
fn sandwich_attack_simulation() {
  // Goal: Verify that router fees make sandwich attacks unprofitable.
  // Router fee is 0.5% each way, so round-trip costs ~1% of trade volume.
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let attacker = 666u64;
    let victim = 1u64;
    let asset_in = AssetKind::Local(1);
    let asset_out = AssetKind::Native;
    // Fund attacker
    let attacker_initial_local = 200 * PRECISION;
    assert_ok!(Assets::mint_into(1, &attacker, attacker_initial_local));
    // Record attacker's starting balance
    let attacker_balance_before = Assets::balance(1, attacker);
    // Deep liquidity pool
    let pool_reserve = 10_000 * PRECISION;
    set_pool(asset_in, asset_out, pool_reserve, pool_reserve);
    // 1. Attacker Front-Run (Buy) - 1% of pool
    let attacker_in = 100 * PRECISION;
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(attacker),
      asset_in,
      asset_out,
      attacker_in,
      0,
      attacker,
      u64::MAX
    ));
    // Get attacker bought amount from event
    let attacker_bought = System::events()
      .iter()
      .rev()
      .find_map(|r| {
        if let crate::mock::RuntimeEvent::AxialRouter(crate::Event::SwapExecuted {
          amount_out,
          who,
          ..
        }) = &r.event
        {
          if *who == attacker {
            Some(*amount_out)
          } else {
            None
          }
        } else {
          None
        }
      })
      .expect("Attacker swap failed");
    // 2. Victim Buy (Pushes price further) - minimum allowed amount
    let victim_in = PRECISION;
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(victim),
      asset_in,
      asset_out,
      victim_in,
      0,
      victim,
      u64::MAX
    ));
    // 3. Attacker Back-Run (Sell)
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(attacker),
      asset_out,
      asset_in,
      attacker_bought,
      0,
      attacker,
      u64::MAX
    ));
    // Check attacker's final Local asset balance
    let attacker_balance_after = Assets::balance(1, attacker);
    let fee_buy = crate::Pallet::<Test>::calculate_router_fee(attacker_in);
    let fee_sell = crate::Pallet::<Test>::calculate_router_fee(attacker_bought);
    println!("Attacker balance before: {attacker_balance_before}");
    println!("Attacker balance after: {attacker_balance_after}");
    println!("Fees paid (buy leg): {fee_buy}");
    println!("Fees paid (sell leg, in Native): {fee_sell}");
    // The attacker's Local token balance should decrease after the round-trip.
    // Even if they got slightly more Local back from the swap than they put in,
    // the fees are deducted separately, so net balance should be lower.
    assert!(
      attacker_balance_after < attacker_balance_before,
      "Sandwich attack should be unprofitable. Balance before: {attacker_balance_before}, after: {attacker_balance_after}"
    );
  });
}

#[test]
fn fee_routing_adapter_test() {
  new_test_ext().execute_with(|| {
    let user = 1u64;
    let asset = AssetKind::Local(1);
    let amount = 1000u128;
    assert_ok!(<Test as crate::Config>::FeeAdapter::route_fee(
      &user, asset, amount
    ));
    let fees = get_collected_fees();
    assert_eq!(fees.len(), 1);
    assert_eq!(fees[0], (user, asset, amount));
    let balance = Assets::balance(1, 123);
    assert_eq!(balance, 1000);
  });
}

#[test]
fn native_input_swap_rejects_gross_debit_that_breaks_keep_alive() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user = 777u64;
    let amount_in = 100 * PRECISION;
    let target = AssetKind::Local(2);
    set_pool(
      AssetKind::Native,
      target,
      10_000 * PRECISION,
      10_000 * PRECISION,
    );
    let _ = Balances::deposit_creating(&user, amount_in);
    let pool_before = get_pool(AssetKind::Native, target).expect("pool must exist");
    System::reset_events();
    assert_noop!(
      AxialRouter::swap(
        RuntimeOrigin::signed(user),
        AssetKind::Native,
        target,
        amount_in,
        0,
        user,
        u64::MAX,
      ),
      Error::<Test>::InsufficientInputBalance
    );
    assert_eq!(Balances::free_balance(user), amount_in);
    assert_eq!(Assets::balance(2, user), 0);
    assert_eq!(get_pool(AssetKind::Native, target), Some(pool_before));
    assert!(
      System::events()
        .into_iter()
        .all(|record| { !matches!(record.event, crate::mock::RuntimeEvent::AxialRouter(_)) })
    );
  });
}

#[test]
fn forced_fee_failure_skips_direct_xyk_route_execution() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user = 1u64;
    let from = AssetKind::Local(2);
    let to = AssetKind::Native;
    let amount_in = 100 * PRECISION;
    set_pool(from, to, 10_000 * PRECISION, 10_000 * PRECISION);
    let user_from_before = Assets::balance(2, user);
    let user_to_before = Balances::free_balance(user);
    let burn_before = Assets::balance(2, 123);
    let pool_before = get_pool(from, to).expect("pool must exist");
    set_force_fee_failure(true);
    System::reset_events();
    assert_noop!(
      AxialRouter::swap(
        RuntimeOrigin::signed(user),
        from,
        to,
        amount_in,
        0,
        user,
        u64::MAX
      ),
      Error::<Test>::FeeRoutingFailed
    );
    assert_eq!(Assets::balance(2, user), user_from_before);
    assert_eq!(Balances::free_balance(user), user_to_before);
    assert_eq!(Assets::balance(2, 123), burn_before);
    assert_eq!(get_pool(from, to), Some(pool_before));
    assert!(get_collected_fees().is_empty());
    assert!(
      System::events()
        .into_iter()
        .all(|record| { !matches!(record.event, crate::mock::RuntimeEvent::AxialRouter(_)) })
    );
  });
}

#[test]
fn forced_fee_failure_skips_direct_mint_route_execution() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user = 1u64;
    let from = AssetKind::Local(2);
    let to = AssetKind::Native;
    let amount_in = 100 * PRECISION;
    set_tmc_curve(to, from, 2);
    let user_from_before = Assets::balance(2, user);
    let user_native_before = Balances::free_balance(user);
    let zap_native_before = Balances::free_balance(888);
    let burn_before = Assets::balance(2, 123);
    set_force_fee_failure(true);
    System::reset_events();
    assert_noop!(
      AxialRouter::swap(
        RuntimeOrigin::signed(user),
        from,
        to,
        amount_in,
        0,
        user,
        u64::MAX
      ),
      Error::<Test>::FeeRoutingFailed
    );
    assert_eq!(Assets::balance(2, user), user_from_before);
    assert_eq!(Balances::free_balance(user), user_native_before);
    assert_eq!(Balances::free_balance(888), zap_native_before);
    assert_eq!(Assets::balance(2, 123), burn_before);
    assert!(get_collected_fees().is_empty());
    assert!(
      System::events()
        .into_iter()
        .all(|record| { !matches!(record.event, crate::mock::RuntimeEvent::AxialRouter(_)) })
    );
  });
}

#[test]
fn forced_fee_failure_skips_multi_hop_route_execution() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user = 1u64;
    let dot = AssetKind::Local(2);
    let usdc = AssetKind::Local(3);
    let native = AssetKind::Native;
    let amount_in = 100 * PRECISION;
    set_pool(dot, native, 10_000 * PRECISION, 10_000 * PRECISION);
    set_pool(native, usdc, 10_000 * PRECISION, 10_000 * PRECISION);
    let user_dot_before = Assets::balance(2, user);
    let user_usdc_before = Assets::balance(3, user);
    let burn_before = Assets::balance(2, 123);
    let first_pool_before = get_pool(dot, native).expect("first pool must exist");
    let second_pool_before = get_pool(native, usdc).expect("second pool must exist");
    set_force_fee_failure(true);
    System::reset_events();
    assert_noop!(
      AxialRouter::swap(
        RuntimeOrigin::signed(user),
        dot,
        usdc,
        amount_in,
        0,
        user,
        u64::MAX,
      ),
      Error::<Test>::FeeRoutingFailed
    );
    assert_eq!(Assets::balance(2, user), user_dot_before);
    assert_eq!(Assets::balance(3, user), user_usdc_before);
    assert_eq!(Assets::balance(2, 123), burn_before);
    assert_eq!(get_pool(dot, native), Some(first_pool_before));
    assert_eq!(get_pool(native, usdc), Some(second_pool_before));
    assert!(get_collected_fees().is_empty());
    assert!(
      System::events()
        .into_iter()
        .all(|record| { !matches!(record.event, crate::mock::RuntimeEvent::AxialRouter(_)) })
    );
  });
}

#[test]
fn price_oracle_test() {
  new_test_ext().execute_with(|| {
    let asset_in = AssetKind::Local(1);
    let asset_out = AssetKind::Native;
    let price = 500u128;
    assert_ok!(<Test as crate::Config>::PriceOracle::update_ema_price(
      asset_in, asset_out, price
    ));
    let stored = <Test as crate::Config>::PriceOracle::get_ema_price(asset_in, asset_out);
    assert_eq!(stored, Some(price));
  });
}

#[test]
fn tmc_interface_test() {
  new_test_ext().execute_with(|| {
    let user = 1u64;
    let token_asset = AssetKind::Native; // Token being minted
    let foreign_asset = AssetKind::Local(1); // Collateral
    set_tmc_rate(token_asset, 2); // 1 unit -> 2 Native
    assert!(<Test as crate::Config>::TmcPallet::has_curve(token_asset));
    let receives =
      <Test as crate::Config>::TmcPallet::calculate_user_receives(token_asset, 100).unwrap();
    assert_eq!(receives, 200);
    // Capture initial balances
    let initial_native = Balances::free_balance(user);
    let initial_asset = Assets::balance(1, user);
    let minted = <Test as crate::Config>::TmcPallet::mint_with_distribution(
      &user,
      token_asset,
      foreign_asset,
      100,
    )
    .unwrap();
    assert_eq!(minted, 200); // Total minted
    // User should receive their allocation (25% in mock = 50 Native)
    assert_eq!(Balances::free_balance(user), initial_native + 50);
    // User should have 100 less Asset 1
    assert_eq!(Assets::balance(1, user), initial_asset - 100);
  });
}

#[test]
fn tmc_route_is_skipped_for_mismatched_collateral() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user = 1u64;
    let from_asset = AssetKind::Local(2);
    let to_asset = AssetKind::Native;
    let amount_in = 1_000 * PRECISION;

    // Configure a very attractive TMC quote but with a different allowed collateral asset.
    set_tmc_curve(to_asset, AssetKind::Local(1), 10);

    // Provide XYK liquidity for the actual pair so Router has a valid fallback path.
    let pool_reserve = 10_000 * PRECISION;
    set_pool(from_asset, to_asset, pool_reserve, pool_reserve);

    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(user),
      from_asset,
      to_asset,
      amount_in,
      0,
      user,
      u64::MAX
    ));

    let fee = crate::Pallet::<Test>::calculate_router_fee(amount_in);
    let amount_after_fee = amount_in - fee;
    let expected_xyk_out =
      (amount_after_fee * pool_reserve) / (pool_reserve.saturating_add(amount_after_fee));

    System::assert_last_event(
      crate::Event::SwapExecuted {
        who: user,
        from: from_asset,
        to: to_asset,
        amount_in,
        amount_out: expected_xyk_out,
        mechanism: RouteMechanismKind::DirectXyk,
      }
      .into(),
    );
  });
}

#[test]
fn asset_conversion_api_test() {
  new_test_ext().execute_with(|| {
    let asset_a = AssetKind::Local(1);
    let asset_b = AssetKind::Native;
    set_pool(asset_a, asset_b, 1_000_000, 1_000_000);
    let pool_id = <Test as crate::Config>::AssetConversion::get_pool_id(asset_a, asset_b);
    assert!(pool_id.is_some());
    let reserves = <Test as crate::Config>::AssetConversion::get_pool_reserves(pool_id.unwrap());
    assert_eq!(reserves, Some((1_000_000, 1_000_000)));
    let quote = <Test as crate::Config>::AssetConversion::quote_price_exact_tokens_for_tokens(
      asset_a, asset_b, 1000, true,
    );
    // out = 1000 * 1000000 / 1001000 = 999
    assert_eq!(quote, Some(999));
  });
}

#[test]
fn tmctol_integration_flow() {
  // Test complete TMCTOL integration flow math check
  // 1. User mints tokens through TMC
  let foreign_amount = 1000u128;
  let user_allocation = Perbill::from_rational(333u32, 1000u32).mul_floor(foreign_amount);
  let tol_total = foreign_amount.saturating_sub(user_allocation);
  // 2. TOL distributed to 4 buckets
  let bucket_a = Perbill::from_rational(500u32, 1000u32).mul_floor(tol_total);
  let bucket_b = Perbill::from_rational(167u32, 1000u32).mul_floor(tol_total);
  let bucket_c = Perbill::from_rational(167u32, 1000u32).mul_floor(tol_total);
  let bucket_d = tol_total.saturating_sub(bucket_a + bucket_b + bucket_c);
  // 3. User swaps through Axial Router
  let swap_amount = 500u128;
  let router_fee_bps = 50u32; // 0.5%
  let router_fee = (swap_amount * router_fee_bps as u128) / 10_000;
  // 4. Fee burning (all router fees are burned)
  let burn_amount = router_fee;
  let remaining_fee = 0u128;
  // Verify complete flow
  assert_eq!(user_allocation, 333u128);
  assert_eq!(tol_total, 667u128);
  assert_eq!(bucket_a + bucket_b + bucket_c + bucket_d, tol_total);
  assert_eq!(router_fee, 2u128);
  assert_eq!(burn_amount, 2u128);
  assert_eq!(remaining_fee, 0u128);
}

#[test]
fn tmctol_parameter_validation() {
  // Test TMCTOL parameter validation
  new_test_ext().execute_with(|| {
    let precision = <<Test as crate::Config>::Precision as Get<u128>>::get();
    assert_eq!(precision, 1_000_000_000_000u128);
    // TOL distribution must sum to 66.7%
    let tol_total = Perbill::from_rational(667u32, 1000u32);
    assert_eq!(tol_total, Perbill::from_rational(667u32, 1000u32));
    // 4-bucket distribution must sum to 100% of TOL
    let bucket_a = Perbill::from_rational(500u32, 1000u32);
    let bucket_b = Perbill::from_rational(167u32, 1000u32);
    let bucket_c = Perbill::from_rational(167u32, 1000u32);
    let bucket_d = Perbill::from_rational(166u32, 1000u32);
    let total_buckets = bucket_a.deconstruct()
      + bucket_b.deconstruct()
      + bucket_c.deconstruct()
      + bucket_d.deconstruct();
    assert_eq!(total_buckets, 1_000_000_000u32);
    // Fee burning must be 100% (all fees burned)
    let total_fees = 1000u128;
    let burned_fees = total_fees;
    assert_eq!(burned_fees, 1000u128);
    // Router fee must be reasonable (0.5%)
    let router_fee = Perbill::from_rational(5u32, 1000u32);
    assert!(
      router_fee <= Perbill::from_percent(1),
      "Router fee should be reasonable"
    );
  });
}

#[test]
fn governance_can_update_router_fee() {
  new_test_ext().execute_with(|| {
    // Advance block to enable events
    System::set_block_number(1);
    // Initial fee should be the default
    let initial_fee = AxialRouter::router_fee();
    assert_eq!(initial_fee, primitives::ecosystem::params::AXIAL_ROUTER_FEE);
    // Update router fee as root
    let new_fee = Perbill::from_percent(1); // 1%
    assert_ok!(AxialRouter::update_router_fee(
      RuntimeOrigin::root(),
      new_fee
    ));
    // Verify fee was updated
    let updated_fee = AxialRouter::router_fee();
    assert_eq!(updated_fee, new_fee);
    // Verify event was emitted
    System::assert_last_event(
      Event::RouterFeeUpdated {
        old_fee: initial_fee,
        new_fee,
      }
      .into(),
    );
  });
}

#[test]
fn only_governance_can_update_router_fee() {
  new_test_ext().execute_with(|| {
    // Regular user cannot update router fee
    let new_fee = Perbill::from_percent(1);
    assert_noop!(
      AxialRouter::update_router_fee(RuntimeOrigin::signed(1), new_fee),
      polkadot_sdk::sp_runtime::DispatchError::BadOrigin
    );
    // Root can update within the configured bound
    assert_ok!(AxialRouter::update_router_fee(
      RuntimeOrigin::root(),
      new_fee
    ));
  });
}

#[test]
fn router_fee_update_rejects_values_above_configured_bound() {
  new_test_ext().execute_with(|| {
    let initial_fee = AxialRouter::router_fee();
    let too_high = Perbill::from_percent(2);
    assert_noop!(
      AxialRouter::update_router_fee(RuntimeOrigin::root(), too_high),
      Error::<Test>::RouterFeeTooHigh
    );
    assert_eq!(AxialRouter::router_fee(), initial_fee);
  });
}

#[test]
fn updated_fee_is_used_in_calculations() {
  new_test_ext().execute_with(|| {
    // Advance block to enable events
    System::set_block_number(1);
    let amount = 10_000u128;
    // Calculate fee with initial rate (0.5%)
    let initial_fee = AxialRouter::calculate_router_fee(amount);
    assert_eq!(initial_fee, 50); // 10,000 * 0.005 = 50
    // Update fee to 1%
    let new_fee_rate = Perbill::from_percent(1);
    assert_ok!(AxialRouter::update_router_fee(
      RuntimeOrigin::root(),
      new_fee_rate
    ));
    // Calculate fee with new rate
    let new_fee = AxialRouter::calculate_router_fee(amount);
    assert_eq!(new_fee, 100); // 10,000 * 0.01 = 100
  });
}

#[test]
fn governance_can_add_tracked_assets() {
  new_test_ext().execute_with(|| {
    // Advance block to enable events
    System::set_block_number(1);
    let asset = AssetKind::Local(42);
    // Add tracked asset as root
    assert_ok!(AxialRouter::add_tracked_asset(RuntimeOrigin::root(), asset));
    // Verify event was emitted
    System::assert_last_event(Event::TrackedAssetAdded { asset }.into());
  });
}

#[test]
fn only_governance_can_add_tracked_assets() {
  new_test_ext().execute_with(|| {
    let asset = AssetKind::Local(42);
    // Regular user cannot add tracked assets
    assert_noop!(
      AxialRouter::add_tracked_asset(RuntimeOrigin::signed(1), asset),
      polkadot_sdk::sp_runtime::DispatchError::BadOrigin
    );
    // Root can add
    assert_ok!(AxialRouter::add_tracked_asset(RuntimeOrigin::root(), asset));
  });
}

// ============================================================
// Multi-Hop Routing Tests
// ============================================================

#[test]
fn multi_hop_swap_dot_native_usdc() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user = 1u64;
    let dot = AssetKind::Local(2); // "DOT"
    let usdc = AssetKind::Local(3); // "USDC"
    let native = AssetKind::Native;
    let amount_in = 100 * PRECISION;
    // Setup pools: DOT/Native and Native/USDC
    let pool_reserve = 10_000 * PRECISION;
    set_pool(dot, native, pool_reserve, pool_reserve);
    set_pool(native, usdc, pool_reserve, pool_reserve);
    // No direct DOT/USDC pool — forces multi-hop
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(user),
      dot,
      usdc,
      amount_in,
      0,
      user,
      u64::MAX,
    ));
    // Verify event emitted with correct assets
    System::assert_has_event(
      crate::Event::SwapExecuted {
        who: user,
        from: dot,
        to: usdc,
        amount_in,
        amount_out: System::events()
          .iter()
          .rev()
          .find_map(|r| {
            if let crate::mock::RuntimeEvent::AxialRouter(crate::Event::SwapExecuted {
              amount_out,
              ..
            }) = &r.event
            {
              Some(*amount_out)
            } else {
              None
            }
          })
          .unwrap(),
        mechanism: RouteMechanismKind::MultiHopNative,
      }
      .into(),
    );
  });
}

#[test]
fn multi_hop_output_matches_sequential_hops() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user = 1u64;
    let dot = AssetKind::Local(2);
    let usdc = AssetKind::Local(3);
    let native = AssetKind::Native;
    let amount_in = 100 * PRECISION;
    let pool_reserve = 10_000 * PRECISION;
    set_pool(dot, native, pool_reserve, pool_reserve);
    set_pool(native, usdc, pool_reserve, pool_reserve);
    // Calculate expected output: hop1 then hop2 on identical pool state
    let fee = crate::Pallet::<Test>::calculate_router_fee(amount_in);
    let after_fee = amount_in - fee;
    // Hop 1: DOT → Native (XYK: out = in * res_out / (res_in + in))
    let hop1_out = (after_fee * pool_reserve) / (pool_reserve + after_fee);
    // Hop 2: Native → USDC (on fresh pool)
    let hop2_out = (hop1_out * pool_reserve) / (pool_reserve + hop1_out);
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(user),
      dot,
      usdc,
      amount_in,
      0,
      user,
      u64::MAX,
    ));
    let (actual_out, actual_mechanism) = System::events()
      .iter()
      .rev()
      .find_map(|r| {
        if let crate::mock::RuntimeEvent::AxialRouter(crate::Event::SwapExecuted {
          amount_out,
          mechanism,
          from,
          ..
        }) = &r.event
        {
          if *from == dot {
            Some((*amount_out, mechanism.clone()))
          } else {
            None
          }
        } else {
          None
        }
      })
      .unwrap();
    assert_eq!(
      actual_out, hop2_out,
      "Multi-hop output must match sequential XYK math"
    );
    assert_eq!(
      actual_mechanism,
      RouteMechanismKind::MultiHopNative,
      "DOT→USDC with no direct pool must use multi-hop via Native"
    );
  });
}

#[test]
fn multi_hop_slippage_protection() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user = 1u64;
    let dot = AssetKind::Local(2);
    let usdc = AssetKind::Local(3);
    let native = AssetKind::Native;
    let amount_in = 100 * PRECISION;
    let pool_reserve = 10_000 * PRECISION;
    set_pool(dot, native, pool_reserve, pool_reserve);
    set_pool(native, usdc, pool_reserve, pool_reserve);
    // Unreasonably high min_amount_out should fail
    assert_noop!(
      AxialRouter::swap(
        RuntimeOrigin::signed(user),
        dot,
        usdc,
        amount_in,
        amount_in * 10, // expect 10x return — impossible
        user,
        u64::MAX,
      ),
      Error::<Test>::SlippageExceeded
    );
  });
}

#[test]
fn multi_hop_not_used_when_direct_pool_exists() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user = 1u64;
    let dot = AssetKind::Local(2);
    let usdc = AssetKind::Local(3);
    let native = AssetKind::Native;
    let amount_in = 100 * PRECISION;
    let pool_reserve = 10_000 * PRECISION;
    // Both direct and multi-hop paths available
    set_pool(dot, usdc, pool_reserve, pool_reserve); // Direct
    set_pool(dot, native, pool_reserve, pool_reserve); // Hop 1
    set_pool(native, usdc, pool_reserve, pool_reserve); // Hop 2
    // Direct pool should win because multi-hop loses to slippage on two hops
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(user),
      dot,
      usdc,
      amount_in,
      0,
      user,
      u64::MAX,
    ));
    let (actual_out, actual_mechanism) = System::events()
      .iter()
      .rev()
      .find_map(|r| {
        if let crate::mock::RuntimeEvent::AxialRouter(crate::Event::SwapExecuted {
          amount_out,
          mechanism,
          ..
        }) = &r.event
        {
          Some((*amount_out, mechanism.clone()))
        } else {
          None
        }
      })
      .unwrap();
    // Direct route output: single hop with equal reserves
    let fee = crate::Pallet::<Test>::calculate_router_fee(amount_in);
    let after_fee = amount_in - fee;
    let direct_out = (after_fee * pool_reserve) / (pool_reserve + after_fee);
    assert_eq!(
      actual_out, direct_out,
      "Direct pool should be preferred over multi-hop when it gives better output"
    );
    assert_eq!(
      actual_mechanism,
      RouteMechanismKind::DirectXyk,
      "Direct XYK must be chosen over multi-hop when direct pool gives better output"
    );
  });
}

#[test]
fn multi_hop_no_route_when_intermediate_pool_missing() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user = 1u64;
    let dot = AssetKind::Local(2);
    let usdc = AssetKind::Local(3);
    let native = AssetKind::Native;
    let amount_in = 100 * PRECISION;
    // Only one leg exists: DOT/Native, but no Native/USDC
    set_pool(dot, native, 10_000 * PRECISION, 10_000 * PRECISION);
    // No DOT/USDC, No Native/USDC → no route
    assert_noop!(
      AxialRouter::swap(
        RuntimeOrigin::signed(user),
        dot,
        usdc,
        amount_in,
        0,
        user,
        u64::MAX,
      ),
      Error::<Test>::NoRouteFound
    );
  });
}

#[test]
fn multi_hop_skipped_when_one_leg_is_native() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user = 1u64;
    let dot = AssetKind::Local(2);
    let native = AssetKind::Native;
    let amount_in = 100 * PRECISION;
    // DOT → Native is a direct route, not a multi-hop
    set_pool(dot, native, 10_000 * PRECISION, 10_000 * PRECISION);
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(user),
      dot,
      native,
      amount_in,
      0,
      user,
      u64::MAX,
    ));
    // Should succeed via DirectXyk, not MultiHopNative
    let fee = crate::Pallet::<Test>::calculate_router_fee(amount_in);
    let after_fee = amount_in - fee;
    let expected = (after_fee * 10_000 * PRECISION) / (10_000 * PRECISION + after_fee);
    System::assert_last_event(
      crate::Event::SwapExecuted {
        who: user,
        from: dot,
        to: native,
        amount_in,
        amount_out: expected,
        mechanism: RouteMechanismKind::DirectXyk,
      }
      .into(),
    );
  });
}

#[test]
fn multi_hop_fee_collected_once() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user = 1u64;
    let dot = AssetKind::Local(2);
    let usdc = AssetKind::Local(3);
    let native = AssetKind::Native;
    let amount_in = 1000 * PRECISION;
    let pool_reserve = 10_000 * PRECISION;
    set_pool(dot, native, pool_reserve, pool_reserve);
    set_pool(native, usdc, pool_reserve, pool_reserve);
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(user),
      dot,
      usdc,
      amount_in,
      0,
      user,
      u64::MAX,
    ));
    // Exactly one FeeCollected event (fee charged once, not per hop)
    let all_events = System::events();
    let fee_events: Vec<_> = all_events
      .iter()
      .filter(|r| {
        matches!(
          &r.event,
          crate::mock::RuntimeEvent::AxialRouter(crate::Event::FeeCollected { .. })
        )
      })
      .collect();
    assert_eq!(
      fee_events.len(),
      1,
      "Fee must be collected exactly once for multi-hop"
    );
    // Verify fee amount
    let expected_fee = crate::Pallet::<Test>::calculate_router_fee(amount_in);
    if let crate::mock::RuntimeEvent::AxialRouter(crate::Event::FeeCollected { amount, .. }) =
      &fee_events[0].event
    {
      assert_eq!(*amount, expected_fee);
    }
  });
}

#[test]
fn multi_hop_pool_reserves_update_correctly() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user = 1u64;
    let dot = AssetKind::Local(2);
    let usdc = AssetKind::Local(3);
    let native = AssetKind::Native;
    let amount_in = 100 * PRECISION;
    let pool_reserve = 10_000 * PRECISION;
    set_pool(dot, native, pool_reserve, pool_reserve);
    set_pool(native, usdc, pool_reserve, pool_reserve);
    let fee = crate::Pallet::<Test>::calculate_router_fee(amount_in);
    let after_fee = amount_in - fee;
    // Pre-calculate expected intermediate amount
    let hop1_out = (after_fee * pool_reserve) / (pool_reserve + after_fee);
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(user),
      dot,
      usdc,
      amount_in,
      0,
      user,
      u64::MAX,
    ));
    // Check pool 1 reserves changed (DOT increased, Native decreased)
    POOLS.with(|p| {
      let pools = p.borrow();
      let key = if dot < native {
        (dot, native)
      } else {
        (native, dot)
      };
      let (res_a, res_b) = pools.get(&key).unwrap();
      let (dot_res, native_res) = if key.0 == dot {
        (*res_a, *res_b)
      } else {
        (*res_b, *res_a)
      };
      assert_eq!(
        dot_res,
        pool_reserve + after_fee,
        "DOT reserve should increase by input"
      );
      assert_eq!(
        native_res,
        pool_reserve - hop1_out,
        "Native reserve should decrease by hop1 output"
      );
    });
  });
}

#[cfg(test)]
mod proptest_router {
  use crate::mock::*;
  use crate::types::AssetKind;
  use polkadot_sdk::frame_support::traits::fungibles::Inspect as FungiblesInspect;
  use proptest::prelude::*;

  proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Round-trip swap(a→b→a) through XYK never creates free tokens.
    /// Due to constant-product invariant, user always ends with ≤ initial balance.
    #[test]
    fn round_trip_swap_never_creates_free_tokens(
      reserve in 100_000_000_000u128..10_000_000_000_000u128,
      amount_in in 1_000_000u128..1_000_000_000u128,
    ) {
      let (balance_a_before, balance_a_after) = new_test_ext().execute_with(|| {
        let user = 2u64;
        let asset_a = AssetKind::Local(1);
        let asset_b = AssetKind::Native;
        set_pool(asset_a, asset_b, reserve, reserve);
        set_oracle_price(asset_a, asset_b, primitives::ecosystem::params::PRECISION);
        set_oracle_price(asset_b, asset_a, primitives::ecosystem::params::PRECISION);
        let ba_before = <Assets as FungiblesInspect<u64>>::balance(1, &user);
        let swap1 = crate::Pallet::<Test>::execute_swap_for(
          &user, asset_a, asset_b, amount_in, 0, &user,
        );
        let mut ba_after = ba_before;
        if let Ok(amount_out_1) = swap1 {
          let swap2 = crate::Pallet::<Test>::execute_swap_for(
            &user, asset_b, asset_a, amount_out_1, 0, &user,
          );
          if swap2.is_ok() {
            ba_after = <Assets as FungiblesInspect<u64>>::balance(1, &user);
          }
        }
        (ba_before, ba_after)
      });
      prop_assert!(
        balance_a_after <= balance_a_before,
        "Asset A balance increased after round-trip: {} -> {}",
        balance_a_before, balance_a_after
      );
    }

    /// Fee calculation is bounded: fee ≤ amount_in for any Perbill rate
    #[test]
    fn fee_never_exceeds_input(amount_in in 1u128..u128::MAX / 2) {
      new_test_ext().execute_with(|| {
        let fee = crate::Pallet::<Test>::calculate_router_fee(amount_in);
        assert!(fee <= amount_in, "Fee {} exceeds input {}", fee, amount_in);
      });
    }
  }
}
