//! Integration tests for Axial Router functionality.
//!
//! These tests cover the complete lifecycle of Axial Router operations including:
//! - Asset management and routing infrastructure
//! - Swap functionality with fee processing
//! - Multi-hop routing and path validation
//! - Economic coordination and fee burning
//! - Error handling and edge cases

// Use common module account constants and standardized asset constants

use super::common::{
  ALICE, ASSET_A, ASSET_B, ASSET_NATIVE, LIQUIDITY_AMOUNT, MIN_AMOUNT_OUT, MIN_LIQUIDITY,
  SWAP_AMOUNT, add_liquidity, axial_router_account, burning_manager_account,
  ensure_asset_conversion_pool, seeded_test_ext, setup_axial_router_infrastructure,
};
use crate::{Assets, AxialRouter, Balances, Runtime, RuntimeOrigin, System};
use polkadot_sdk::frame_support::{assert_noop, assert_ok};
use primitives::AssetKind;

/// Setup test environment with pools and liquidity
fn setup_test_environment() -> Result<(), &'static str> {
  setup_axial_router_infrastructure()
}

#[test]
fn test_axial_router_basic_swap_functionality() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Native;
    let quote = AxialRouter::quote_exact_input(ALICE, from, to, SWAP_AMOUNT)
      .expect("quote must exist for seeded direct pool");
    let alice_asset_before = Assets::balance(ASSET_A, ALICE);
    let alice_native_before = Balances::free_balance(ALICE);
    let burning_manager_before = Assets::balance(ASSET_A, burning_manager_account());
    System::reset_events();
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      from,
      to,
      SWAP_AMOUNT,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));
    assert_eq!(
      Assets::balance(ASSET_A, ALICE),
      alice_asset_before - SWAP_AMOUNT
    );
    assert_eq!(
      Balances::free_balance(ALICE),
      alice_native_before + quote.amount_out
    );
    assert_eq!(
      Assets::balance(ASSET_A, burning_manager_account()),
      burning_manager_before + quote.router_fee
    );
    System::assert_has_event(crate::RuntimeEvent::AxialRouter(
      pallet_axial_router::Event::SwapExecuted {
        who: ALICE,
        from,
        to,
        amount_in: SWAP_AMOUNT,
        amount_out: quote.amount_out,
        mechanism: quote.mechanism,
      },
    ));
  });
}

#[test]
fn test_axial_router_fee_processing() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Native;
    let quote = AxialRouter::quote_exact_input(ALICE, from, to, SWAP_AMOUNT)
      .expect("quote must exist for seeded direct pool");
    let burning_manager = burning_manager_account();
    let burning_manager_before = Assets::balance(ASSET_A, burning_manager.clone());
    System::reset_events();
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      from,
      to,
      SWAP_AMOUNT,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));
    assert_eq!(
      Assets::balance(ASSET_A, burning_manager.clone()),
      burning_manager_before + quote.router_fee
    );
    System::assert_has_event(crate::RuntimeEvent::AxialRouter(
      pallet_axial_router::Event::FeeCollected {
        asset: from,
        amount: quote.router_fee,
        source: ALICE,
        collector: burning_manager,
      },
    ));
  });
}

#[test]
fn test_axial_router_anti_self_taxation() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let router = axial_router_account();
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Native;
    let quote = AxialRouter::quote_exact_input(router.clone(), from, to, SWAP_AMOUNT)
      .expect("router account should still receive a direct quote");
    let router_asset_before = Assets::balance(ASSET_A, router.clone());
    let router_native_before = Balances::free_balance(router.clone());
    let burning_manager_before = Assets::balance(ASSET_A, burning_manager_account());
    System::reset_events();
    assert_eq!(quote.router_fee, 0);
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(router.clone()),
      from,
      to,
      SWAP_AMOUNT,
      MIN_AMOUNT_OUT,
      router.clone(),
      1000,
    ));
    assert_eq!(
      Assets::balance(ASSET_A, router.clone()),
      router_asset_before - SWAP_AMOUNT
    );
    assert_eq!(
      Balances::free_balance(router.clone()),
      router_native_before + quote.amount_out
    );
    assert_eq!(
      Assets::balance(ASSET_A, burning_manager_account()),
      burning_manager_before
    );
    assert!(System::events().iter().all(|record| {
      !matches!(
        &record.event,
        crate::RuntimeEvent::AxialRouter(pallet_axial_router::Event::FeeCollected { .. })
      )
    }));
  });
}

#[test]
fn test_axial_router_multi_hop_routing() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    // setup_test_environment creates Native/ASSET_A pool with LIQUIDITY_AMOUNT.
    // Add a Native/ASSET_B pool with smaller liquidity (ALICE's remaining native budget).
    let second_pool_liq = LIQUIDITY_AMOUNT / 4;
    ensure_asset_conversion_pool(ASSET_NATIVE, AssetKind::Local(ASSET_B));
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(ALICE),
      ASSET_NATIVE,
      AssetKind::Local(ASSET_B),
      second_pool_liq,
      second_pool_liq,
      MIN_LIQUIDITY,
      MIN_LIQUIDITY,
      &ALICE,
    ));

    let alice_b_before = Assets::balance(ASSET_B, ALICE);

    // Multi-hop swap: ASSET_A → Native → ASSET_B
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      AssetKind::Local(ASSET_A),
      AssetKind::Local(ASSET_B),
      SWAP_AMOUNT,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));

    let alice_b_after = Assets::balance(ASSET_B, ALICE);
    assert!(
      alice_b_after > alice_b_before,
      "ALICE should have received ASSET_B via multi-hop: before={alice_b_before}, after={alice_b_after}"
    );

    // Verify SwapExecuted event with correct from/to
    assert!(
      System::events().iter().any(|r| matches!(
        &r.event,
        crate::RuntimeEvent::AxialRouter(pallet_axial_router::Event::SwapExecuted {
          from: AssetKind::Local(a),
          to: AssetKind::Local(b),
          ..
        }) if *a == ASSET_A && *b == ASSET_B
      )),
      "SwapExecuted event should show ASSET_A → ASSET_B"
    );
  });
}

#[test]
fn test_axial_router_multi_hop_fee_collected_once() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let second_pool_liq = LIQUIDITY_AMOUNT / 4;
    ensure_asset_conversion_pool(ASSET_NATIVE, AssetKind::Local(ASSET_B));
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(ALICE),
      ASSET_NATIVE,
      AssetKind::Local(ASSET_B),
      second_pool_liq,
      second_pool_liq,
      MIN_LIQUIDITY,
      MIN_LIQUIDITY,
      &ALICE,
    ));

    System::reset_events();

    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      AssetKind::Local(ASSET_A),
      AssetKind::Local(ASSET_B),
      SWAP_AMOUNT,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));

    // Verify exactly one FeeCollected event (fee charged once, not per hop)
    let fee_event_count = System::events()
      .iter()
      .filter(|r| {
        matches!(
          &r.event,
          crate::RuntimeEvent::AxialRouter(pallet_axial_router::Event::FeeCollected { .. })
        )
      })
      .count();
    assert_eq!(
      fee_event_count, 1,
      "Fee must be collected exactly once for multi-hop swap"
    );
  });
}

#[test]
fn test_axial_router_multi_hop_no_route_when_second_pool_missing() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    // Only Native/ASSET_A pool exists. No Native/ASSET_B → no ASSET_A→ASSET_B route.
    assert_noop!(
      AxialRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Local(ASSET_A),
        AssetKind::Local(ASSET_B),
        SWAP_AMOUNT,
        MIN_AMOUNT_OUT,
        ALICE,
        1000,
      ),
      pallet_axial_router::pallet::Error::<Runtime>::NoRouteFound
    );
  });
}

#[test]
fn test_axial_router_error_handling() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    // Test identical assets error
    assert_noop!(
      AxialRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Local(ASSET_A),
        AssetKind::Local(ASSET_A),
        SWAP_AMOUNT,
        MIN_AMOUNT_OUT,
        ALICE,
        1000,
      ),
      pallet_axial_router::pallet::Error::<Runtime>::IdenticalAssets
    );
    // Test zero amount error (caught by MinSwapForeign check)
    assert_noop!(
      AxialRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Local(ASSET_A),
        AssetKind::Native,
        0,
        MIN_AMOUNT_OUT,
        ALICE,
        1000,
      ),
      pallet_axial_router::pallet::Error::<Runtime>::AmountTooLow
    );
    // Test deadline passed error
    System::set_block_number(1000);
    assert_noop!(
      AxialRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Local(ASSET_A),
        AssetKind::Native,
        SWAP_AMOUNT,
        MIN_AMOUNT_OUT,
        ALICE,
        999, // deadline already passed
      ),
      pallet_axial_router::pallet::Error::<Runtime>::DeadlinePassed
    );
  });
}

#[test]
fn test_axial_router_accumulated_balance_processing() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let amount = SWAP_AMOUNT / 10;
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Native;
    let quote = AxialRouter::quote_exact_input(ALICE, from, to, amount)
      .expect("quote must exist for seeded direct pool");
    let burning_manager_before = Assets::balance(ASSET_A, burning_manager_account());
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      from,
      to,
      amount,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));
    assert_eq!(
      Assets::balance(ASSET_A, burning_manager_account()),
      burning_manager_before + quote.router_fee
    );
  });
}

#[test]
fn test_axial_router_native_token_swaps() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let from = AssetKind::Native;
    let to = AssetKind::Local(ASSET_A);
    let quote = AxialRouter::quote_exact_input(ALICE, from, to, SWAP_AMOUNT)
      .expect("quote must exist for seeded direct pool");
    let alice_native_before = Balances::free_balance(ALICE);
    let alice_asset_before = Assets::balance(ASSET_A, ALICE);
    let burning_manager_before = Balances::free_balance(burning_manager_account());
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      from,
      to,
      SWAP_AMOUNT,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));
    assert_eq!(
      Balances::free_balance(ALICE),
      alice_native_before - SWAP_AMOUNT
    );
    assert_eq!(
      Assets::balance(ASSET_A, ALICE),
      alice_asset_before + quote.amount_out
    );
    assert_eq!(
      Balances::free_balance(burning_manager_account()),
      burning_manager_before + quote.router_fee
    );
  });
}

#[test]
fn test_axial_router_fee_calculation_accuracy() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let expected_fee = AxialRouter::calculate_router_fee(SWAP_AMOUNT);
    let quote = AxialRouter::quote_exact_input(
      ALICE,
      AssetKind::Local(ASSET_A),
      AssetKind::Native,
      SWAP_AMOUNT,
    )
    .expect("quote must exist for seeded direct pool");
    assert_eq!(quote.router_fee, expected_fee);
    assert_eq!(quote.amount_after_fee, SWAP_AMOUNT - expected_fee);
  });
}

#[test]
fn test_axial_router_minimum_amount_out_protection() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    // Initialize EMA prices to avoid deviation errors
    pallet_axial_router::pallet::EmaPrices::<Runtime>::insert(
      AssetKind::Local(ASSET_A),
      AssetKind::Local(ASSET_B),
      SWAP_AMOUNT,
    );
    pallet_axial_router::pallet::EmaPrices::<Runtime>::insert(
      AssetKind::Local(ASSET_B),
      AssetKind::Local(ASSET_A),
      SWAP_AMOUNT,
    );
    pallet_axial_router::pallet::EmaPrices::<Runtime>::insert(
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      SWAP_AMOUNT,
    );
    pallet_axial_router::pallet::EmaPrices::<Runtime>::insert(
      AssetKind::Local(ASSET_A),
      AssetKind::Native,
      SWAP_AMOUNT,
    );
    let unreasonably_high_min = SWAP_AMOUNT * 10;
    assert_noop!(
      AxialRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Local(ASSET_A),
        AssetKind::Native,
        SWAP_AMOUNT,
        unreasonably_high_min,
        ALICE,
        1000,
      ),
      pallet_axial_router::pallet::Error::<Runtime>::SlippageExceeded
    );
  });
}

#[test]
fn test_axial_router_direct_fee_processing() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Native;
    let quote = AxialRouter::quote_exact_input(ALICE, from, to, SWAP_AMOUNT)
      .expect("quote must exist for seeded direct pool");
    System::reset_events();
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      from,
      to,
      SWAP_AMOUNT,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));
    let router_events = System::events()
      .into_iter()
      .filter_map(|record| match record.event {
        crate::RuntimeEvent::AxialRouter(event) => Some(event),
        _ => None,
      })
      .collect::<Vec<_>>();
    let fee_index = router_events
      .iter()
      .position(|event| {
        matches!(
          event,
          pallet_axial_router::Event::FeeCollected {
            asset,
            amount,
            source,
            collector,
          } if *asset == from
            && *amount == quote.router_fee
            && *source == ALICE
            && *collector == burning_manager_account()
        )
      })
      .expect("fee event must be present");
    let swap_index = router_events
      .iter()
      .position(|event| {
        matches!(
          event,
          pallet_axial_router::Event::SwapExecuted {
            who,
            from: event_from,
            to: event_to,
            amount_in,
            amount_out,
            ..
          } if *who == ALICE
            && *event_from == from
            && *event_to == to
            && *amount_in == SWAP_AMOUNT
            && *amount_out == quote.amount_out
        )
      })
      .expect("swap event must be present");
    assert!(fee_index < swap_index, "fee event must precede swap event");
  });
}

#[test]
fn test_axial_router_consistent_fee_burning() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let amount = SWAP_AMOUNT / 10;
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Native;
    let fee = AxialRouter::calculate_router_fee(amount);
    let burning_manager_before = Assets::balance(ASSET_A, burning_manager_account());
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      from,
      to,
      amount,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      from,
      to,
      amount,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));
    assert_eq!(
      Assets::balance(ASSET_A, burning_manager_account()),
      burning_manager_before + fee * 2
    );
  });
}

#[test]
fn test_axial_router_multiple_accumulation_cycles() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let amount = SWAP_AMOUNT / 10;
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Native;
    System::reset_events();
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      from,
      to,
      amount,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      from,
      to,
      amount,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));
    let fee_events = System::events()
      .into_iter()
      .filter(|record| {
        matches!(
          &record.event,
          crate::RuntimeEvent::AxialRouter(pallet_axial_router::Event::FeeCollected {
            asset,
            amount: event_amount,
            source,
            ..
          }) if *asset == from
            && *event_amount == AxialRouter::calculate_router_fee(amount)
            && *source == ALICE
        )
      })
      .count();
    assert_eq!(fee_events, 2);
  });
}

#[test]
fn test_axial_router_fee_collection_only_on_successful_swaps() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let burning_manager_before = Assets::balance(ASSET_A, burning_manager_account());
    let unreasonably_high_min = SWAP_AMOUNT * 100;
    System::reset_events();
    assert_noop!(
      AxialRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Local(ASSET_A),
        AssetKind::Native,
        SWAP_AMOUNT,
        unreasonably_high_min,
        ALICE,
        1000,
      ),
      pallet_axial_router::pallet::Error::<Runtime>::SlippageExceeded
    );
    assert_eq!(
      Assets::balance(ASSET_A, burning_manager_account()),
      burning_manager_before
    );
    assert!(System::events().into_iter().all(|record| {
      !matches!(
        record.event,
        crate::RuntimeEvent::AxialRouter(pallet_axial_router::Event::FeeCollected { .. })
      )
    }));
  });
}

#[test]
fn test_axial_router_path_validation() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let burning_manager_before = Assets::balance(ASSET_A, burning_manager_account());
    let non_existent_asset = 999;
    System::reset_events();
    assert_noop!(
      AxialRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Local(ASSET_A),
        AssetKind::Local(non_existent_asset),
        SWAP_AMOUNT,
        MIN_AMOUNT_OUT,
        ALICE,
        1000,
      ),
      pallet_axial_router::pallet::Error::<Runtime>::NoRouteFound
    );
    assert_eq!(
      Assets::balance(ASSET_A, burning_manager_account()),
      burning_manager_before
    );
    assert!(
      System::events()
        .into_iter()
        .all(|record| { !matches!(record.event, crate::RuntimeEvent::AxialRouter(_)) })
    );
  });
}

#[test]
fn test_axial_router_with_empty_pools() {
  seeded_test_ext().execute_with(|| {
    // Use basic test environment without pools (setup_axial_router_infrastructure is not called)

    // Test swap with empty/non-existent pools should fail with NoRouteFound
    assert_noop!(
      AxialRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Local(ASSET_A),
        AssetKind::Native,
        SWAP_AMOUNT,
        MIN_AMOUNT_OUT,
        ALICE,
        1000,
      ),
      pallet_axial_router::pallet::Error::<Runtime>::NoRouteFound
    );
  });
}

#[test]
fn test_axial_router_events() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Native;
    let quote = AxialRouter::quote_exact_input(ALICE, from, to, SWAP_AMOUNT)
      .expect("quote must exist for seeded direct pool");
    System::reset_events();
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      from,
      to,
      SWAP_AMOUNT,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));
    System::assert_has_event(crate::RuntimeEvent::AxialRouter(
      pallet_axial_router::Event::FeeCollected {
        asset: from,
        amount: quote.router_fee,
        source: ALICE,
        collector: burning_manager_account(),
      },
    ));
    System::assert_has_event(crate::RuntimeEvent::AxialRouter(
      pallet_axial_router::Event::SwapExecuted {
        who: ALICE,
        from,
        to,
        amount_in: SWAP_AMOUNT,
        amount_out: quote.amount_out,
        mechanism: quote.mechanism,
      },
    ));
  });
}
