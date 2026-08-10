//! Integration tests for DEOS Router functionality.
//!
//! These tests cover the complete lifecycle of DEOS Router operations including:
//! - Asset management and routing infrastructure
//! - Swap functionality with fee processing
//! - Multi-hop routing and path validation
//! - Economic coordination and fee burning
//! - Error handling and edge cases

// Use common module account constants and standardized asset constants

use super::common::{
  ALICE, ASSET_A, ASSET_B, ASSET_NATIVE, LIQUIDITY_AMOUNT, MIN_AMOUNT_OUT, MIN_LIQUIDITY,
  SWAP_AMOUNT, add_liquidity, burn_actor_account, deos_router_account,
  ensure_asset_conversion_pool, seeded_test_ext, setup_deos_router_infrastructure,
};
use crate::{Assets, Balances, DeosRouter, Runtime, RuntimeOrigin, System};
use polkadot_sdk::frame_support::{assert_noop, assert_ok};
use primitives::AssetKind;

/// Setup test environment with pools and liquidity
fn setup_test_environment() -> Result<(), &'static str> {
  setup_deos_router_infrastructure()
}

fn check_router_host_pool_registry() -> Result<(), &'static str> {
  let indexed = pallet_deos_router::LpPairByTokenId::<Runtime>::get();
  for (lp_token, pair) in &indexed {
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get(pair)
      .ok_or("Router reverse index references an absent host pool")?;
    if pool.lp_token != *lp_token {
      return Err("Router LP token disagrees with the host pool registry");
    }
  }
  for (pair, pool) in polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::iter() {
    if indexed.get(&pool.lp_token) != Some(&pair) {
      return Err("Host pool lacks its exact Router reverse entry");
    }
  }
  Ok(())
}

#[test]
fn router_host_pool_registry_corruption_matrix_is_deterministic() {
  #[derive(Clone, Copy)]
  enum Corruption {
    None,
    MissingReverse,
    WrongLp,
    OrphanReverse,
  }

  let cases = [
    (Corruption::None, None),
    (
      Corruption::MissingReverse,
      Some("Host pool lacks its exact Router reverse entry"),
    ),
    (
      Corruption::WrongLp,
      Some("Router LP token disagrees with the host pool registry"),
    ),
    (
      Corruption::OrphanReverse,
      Some("Router reverse index references an absent host pool"),
    ),
  ];
  for (corruption, expected) in cases {
    seeded_test_ext().execute_with(|| {
      assert_ok!(setup_test_environment());
      let (pair, pool) = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::iter()
        .next()
        .expect("one host pool exists");
      match corruption {
        Corruption::None => {}
        Corruption::MissingReverse => {
          pallet_deos_router::LpPairByTokenId::<Runtime>::mutate(|pairs| {
            pairs.remove(&pool.lp_token);
          });
        }
        Corruption::WrongLp => {
          pallet_deos_router::LpPairByTokenId::<Runtime>::mutate(|pairs| {
            pairs.remove(&pool.lp_token);
            pairs
              .try_insert(pool.lp_token.saturating_add(1), pair)
              .expect("one corrupt entry fits");
          });
        }
        Corruption::OrphanReverse => {
          pallet_deos_router::LpPairByTokenId::<Runtime>::mutate(|pairs| {
            pairs
              .try_insert(
                pool.lp_token.saturating_add(1),
                (AssetKind::Local(900_000), AssetKind::Local(900_001)),
              )
              .expect("one corrupt entry fits");
          });
        }
      }
      assert_eq!(check_router_host_pool_registry().err(), expected);
    });
  }
}

#[test]
fn test_deos_router_basic_swap_functionality() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Native;
    let quote = DeosRouter::quote_exact_input(ALICE, from, to, SWAP_AMOUNT)
      .expect("quote must exist for seeded direct pool");
    let alice_asset_before = Assets::balance(ASSET_A, ALICE);
    let alice_native_before = Balances::free_balance(ALICE);
    let burn_actor_before = Assets::balance(ASSET_A, burn_actor_account());
    System::reset_events();
    assert_ok!(DeosRouter::swap(
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
      Assets::balance(ASSET_A, burn_actor_account()),
      burn_actor_before + quote.router_fee
    );
    assert!(System::events().iter().any(|record| matches!(
      &record.event,
      crate::RuntimeEvent::DeosRouter(pallet_deos_router::Event::SwapExecuted {
        who,
        from: event_from,
        to: event_to,
        outcome,
      }) if *who == ALICE
        && *event_from == from
        && *event_to == to
        && outcome.total_amount_in == SWAP_AMOUNT
        && outcome.recipient_amount_out == quote.amount_out
        && outcome.legs == quote.legs
    )));
  });
}

#[test]
fn test_deos_router_fee_processing() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Native;
    let quote = DeosRouter::quote_exact_input(ALICE, from, to, SWAP_AMOUNT)
      .expect("quote must exist for seeded direct pool");
    let burn_actor = burn_actor_account();
    let burn_actor_before = Assets::balance(ASSET_A, burn_actor.clone());
    System::reset_events();
    assert_ok!(DeosRouter::swap(
      RuntimeOrigin::signed(ALICE),
      from,
      to,
      SWAP_AMOUNT,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));
    assert_eq!(
      Assets::balance(ASSET_A, burn_actor.clone()),
      burn_actor_before + quote.router_fee
    );
    System::assert_has_event(crate::RuntimeEvent::DeosRouter(
      pallet_deos_router::Event::FeeCollected {
        asset: from,
        amount: quote.router_fee,
        source: ALICE,
        collector: burn_actor,
      },
    ));
  });
}

#[test]
fn test_deos_router_anti_self_taxation() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let router = deos_router_account();
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Native;
    let quote = DeosRouter::quote_exact_input(router.clone(), from, to, SWAP_AMOUNT)
      .expect("router account should still receive a direct quote");
    let router_asset_before = Assets::balance(ASSET_A, router.clone());
    let router_native_before = Balances::free_balance(router.clone());
    let burn_actor_before = Assets::balance(ASSET_A, burn_actor_account());
    System::reset_events();
    assert_eq!(quote.router_fee, 0);
    assert_ok!(DeosRouter::swap(
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
      Assets::balance(ASSET_A, burn_actor_account()),
      burn_actor_before
    );
    assert!(System::events().iter().all(|record| {
      !matches!(
        &record.event,
        crate::RuntimeEvent::DeosRouter(pallet_deos_router::Event::FeeCollected { .. })
      )
    }));
  });
}

#[test]
fn test_deos_router_multi_hop_routing() {
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
    assert_ok!(DeosRouter::swap(
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
        crate::RuntimeEvent::DeosRouter(pallet_deos_router::Event::SwapExecuted {
          from: AssetKind::Local(a),
          to: AssetKind::Local(b),
          ..
        }) if *a == ASSET_A && *b == ASSET_B
      )),
      "SwapExecuted event should show ASSET_A → ASSET_B"
    );
  });
}

fn assert_native_anchored_market_failure_rolls_back(failure_index: usize) {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Local(ASSET_B);
    let native = ASSET_NATIVE;
    let second_pool_liq = LIQUIDITY_AMOUNT / 4;
    ensure_asset_conversion_pool(native, to);
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(ALICE),
      native,
      to,
      second_pool_liq,
      second_pool_liq,
      MIN_LIQUIDITY,
      MIN_LIQUIDITY,
      &ALICE,
    ));
    let first_pool_before = crate::AssetConversion::get_reserves(from, native).unwrap();
    let second_pool_before = crate::AssetConversion::get_reserves(native, to).unwrap();
    let input_before = Assets::balance(ASSET_A, ALICE);
    let output_before = Assets::balance(ASSET_B, ALICE);
    let native_before = Balances::free_balance(ALICE);
    let fee_before = Assets::balance(ASSET_A, burn_actor_account());
    let first_feed = crate::configs::oracle_config::deos_router_pool_feed(from, native);
    let second_feed = crate::configs::oracle_config::deos_router_pool_feed(native, to);
    let first_observation_before = crate::Oracle::observations(first_feed);
    let second_observation_before = crate::Oracle::observations(second_feed);
    let events_before = System::events();
    crate::configs::deos_router_config::set_fail_after_xyk_execution_at(Some(failure_index));

    assert!(
      DeosRouter::swap(
        RuntimeOrigin::signed(ALICE),
        from,
        to,
        SWAP_AMOUNT,
        MIN_AMOUNT_OUT,
        ALICE,
        1_000,
      )
      .is_err()
    );
    assert_eq!(
      crate::AssetConversion::get_reserves(from, native).unwrap(),
      first_pool_before
    );
    assert_eq!(
      crate::AssetConversion::get_reserves(native, to).unwrap(),
      second_pool_before
    );
    assert_eq!(Assets::balance(ASSET_A, ALICE), input_before);
    assert_eq!(Assets::balance(ASSET_B, ALICE), output_before);
    assert_eq!(Balances::free_balance(ALICE), native_before);
    assert_eq!(Assets::balance(ASSET_A, burn_actor_account()), fee_before);
    assert_eq!(
      crate::Oracle::observations(first_feed),
      first_observation_before
    );
    assert_eq!(
      crate::Oracle::observations(second_feed),
      second_observation_before
    );
    assert_eq!(System::events(), events_before);
  });
}

#[test]
fn native_anchored_first_market_failure_rolls_back_composed_runtime_state() {
  assert_native_anchored_market_failure_rolls_back(0);
}

#[test]
fn native_anchored_second_market_failure_rolls_back_composed_runtime_state() {
  assert_native_anchored_market_failure_rolls_back(1);
}

#[test]
fn test_deos_router_multi_hop_fee_collected_once() {
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

    assert_ok!(DeosRouter::swap(
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
          crate::RuntimeEvent::DeosRouter(pallet_deos_router::Event::FeeCollected { .. })
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
fn test_deos_router_multi_hop_no_route_when_second_pool_missing() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    // Only Native/ASSET_A pool exists. No Native/ASSET_B → no ASSET_A→ASSET_B route.
    assert_noop!(
      DeosRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Local(ASSET_A),
        AssetKind::Local(ASSET_B),
        SWAP_AMOUNT,
        MIN_AMOUNT_OUT,
        ALICE,
        1000,
      ),
      pallet_deos_router::pallet::Error::<Runtime>::NoRouteFound
    );
  });
}

#[test]
fn test_deos_router_error_handling() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    // Test identical assets error
    assert_noop!(
      DeosRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Local(ASSET_A),
        AssetKind::Local(ASSET_A),
        SWAP_AMOUNT,
        MIN_AMOUNT_OUT,
        ALICE,
        1000,
      ),
      pallet_deos_router::pallet::Error::<Runtime>::IdenticalAssets
    );
    // Test zero amount error (caught by MinSwapForeign check)
    assert_noop!(
      DeosRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Local(ASSET_A),
        AssetKind::Native,
        0,
        MIN_AMOUNT_OUT,
        ALICE,
        1000,
      ),
      pallet_deos_router::pallet::Error::<Runtime>::AmountTooLow
    );
    // Test deadline passed error
    System::set_block_number(1000);
    assert_noop!(
      DeosRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Local(ASSET_A),
        AssetKind::Native,
        SWAP_AMOUNT,
        MIN_AMOUNT_OUT,
        ALICE,
        999, // deadline already passed
      ),
      pallet_deos_router::pallet::Error::<Runtime>::DeadlinePassed
    );
  });
}

#[test]
fn test_deos_router_accumulated_balance_processing() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let amount = SWAP_AMOUNT / 10;
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Native;
    let quote = DeosRouter::quote_exact_input(ALICE, from, to, amount)
      .expect("quote must exist for seeded direct pool");
    let burn_actor_before = Assets::balance(ASSET_A, burn_actor_account());
    assert_ok!(DeosRouter::swap(
      RuntimeOrigin::signed(ALICE),
      from,
      to,
      amount,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));
    assert_eq!(
      Assets::balance(ASSET_A, burn_actor_account()),
      burn_actor_before + quote.router_fee
    );
  });
}

#[test]
fn test_deos_router_native_token_swaps() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let from = AssetKind::Native;
    let to = AssetKind::Local(ASSET_A);
    let quote = DeosRouter::quote_exact_input(ALICE, from, to, SWAP_AMOUNT)
      .expect("quote must exist for seeded direct pool");
    let alice_native_before = Balances::free_balance(ALICE);
    let alice_asset_before = Assets::balance(ASSET_A, ALICE);
    let burn_actor_before = Balances::free_balance(burn_actor_account());
    assert_ok!(DeosRouter::swap(
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
      Balances::free_balance(burn_actor_account()),
      burn_actor_before + quote.router_fee
    );
  });
}

#[test]
fn test_deos_router_fee_calculation_accuracy() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let expected_fee = DeosRouter::calculate_router_fee(SWAP_AMOUNT);
    let quote = DeosRouter::quote_exact_input(
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
fn test_deos_router_minimum_amount_out_protection() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let unreasonably_high_min = SWAP_AMOUNT * 10;
    assert_noop!(
      DeosRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Local(ASSET_A),
        AssetKind::Native,
        SWAP_AMOUNT,
        unreasonably_high_min,
        ALICE,
        1000,
      ),
      pallet_deos_router::pallet::Error::<Runtime>::SlippageExceeded
    );
  });
}

#[test]
fn test_deos_router_direct_fee_processing() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Native;
    let quote = DeosRouter::quote_exact_input(ALICE, from, to, SWAP_AMOUNT)
      .expect("quote must exist for seeded direct pool");
    System::reset_events();
    assert_ok!(DeosRouter::swap(
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
        crate::RuntimeEvent::DeosRouter(event) => Some(event),
        _ => None,
      })
      .collect::<Vec<_>>();
    let fee_index = router_events
      .iter()
      .position(|event| {
        matches!(
          event,
          pallet_deos_router::Event::FeeCollected {
            asset,
            amount,
            source,
            collector,
          } if *asset == from
            && *amount == quote.router_fee
            && *source == ALICE
            && *collector == burn_actor_account()
        )
      })
      .expect("fee event must be present");
    let swap_index = router_events
      .iter()
      .position(|event| {
        matches!(
          event,
          pallet_deos_router::Event::SwapExecuted {
            who,
            from: event_from,
            to: event_to,
            outcome,
          } if *who == ALICE
            && *event_from == from
            && *event_to == to
            && outcome.total_amount_in == SWAP_AMOUNT
            && outcome.recipient_amount_out == quote.amount_out
        )
      })
      .expect("swap event must be present");
    assert!(fee_index < swap_index, "fee event must precede swap event");
  });
}

#[test]
fn test_deos_router_consistent_fee_burning() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let amount = SWAP_AMOUNT / 10;
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Native;
    let fee = DeosRouter::calculate_router_fee(amount);
    let burn_actor_before = Assets::balance(ASSET_A, burn_actor_account());
    assert_ok!(DeosRouter::swap(
      RuntimeOrigin::signed(ALICE),
      from,
      to,
      amount,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));
    assert_ok!(DeosRouter::swap(
      RuntimeOrigin::signed(ALICE),
      from,
      to,
      amount,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));
    assert_eq!(
      Assets::balance(ASSET_A, burn_actor_account()),
      burn_actor_before + fee * 2
    );
  });
}

#[test]
fn test_deos_router_multiple_accumulation_cycles() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let amount = SWAP_AMOUNT / 10;
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Native;
    System::reset_events();
    assert_ok!(DeosRouter::swap(
      RuntimeOrigin::signed(ALICE),
      from,
      to,
      amount,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));
    assert_ok!(DeosRouter::swap(
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
          crate::RuntimeEvent::DeosRouter(pallet_deos_router::Event::FeeCollected {
            asset,
            amount: event_amount,
            source,
            ..
          }) if *asset == from
            && *event_amount == DeosRouter::calculate_router_fee(amount)
            && *source == ALICE
        )
      })
      .count();
    assert_eq!(fee_events, 2);
  });
}

#[test]
fn test_deos_router_fee_collection_only_on_successful_swaps() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let burn_actor_before = Assets::balance(ASSET_A, burn_actor_account());
    let unreasonably_high_min = SWAP_AMOUNT * 100;
    System::reset_events();
    assert_noop!(
      DeosRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Local(ASSET_A),
        AssetKind::Native,
        SWAP_AMOUNT,
        unreasonably_high_min,
        ALICE,
        1000,
      ),
      pallet_deos_router::pallet::Error::<Runtime>::SlippageExceeded
    );
    assert_eq!(
      Assets::balance(ASSET_A, burn_actor_account()),
      burn_actor_before
    );
    assert!(System::events().into_iter().all(|record| {
      !matches!(
        record.event,
        crate::RuntimeEvent::DeosRouter(pallet_deos_router::Event::FeeCollected { .. })
      )
    }));
  });
}

#[test]
fn test_deos_router_path_validation() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let burn_actor_before = Assets::balance(ASSET_A, burn_actor_account());
    let non_existent_asset = 999;
    System::reset_events();
    assert_noop!(
      DeosRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Local(ASSET_A),
        AssetKind::Local(non_existent_asset),
        SWAP_AMOUNT,
        MIN_AMOUNT_OUT,
        ALICE,
        1000,
      ),
      pallet_deos_router::pallet::Error::<Runtime>::NoRouteFound
    );
    assert_eq!(
      Assets::balance(ASSET_A, burn_actor_account()),
      burn_actor_before
    );
    assert!(
      System::events()
        .into_iter()
        .all(|record| { !matches!(record.event, crate::RuntimeEvent::DeosRouter(_)) })
    );
  });
}

#[test]
fn test_deos_router_with_empty_pools() {
  seeded_test_ext().execute_with(|| {
    // Use basic test environment without pools (setup_deos_router_infrastructure is not called)

    // Test swap with empty/non-existent pools should fail with NoRouteFound
    assert_noop!(
      DeosRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Local(ASSET_A),
        AssetKind::Native,
        SWAP_AMOUNT,
        MIN_AMOUNT_OUT,
        ALICE,
        1000,
      ),
      pallet_deos_router::pallet::Error::<Runtime>::NoRouteFound
    );
  });
}

#[test]
fn test_deos_router_events() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Native;
    let quote = DeosRouter::quote_exact_input(ALICE, from, to, SWAP_AMOUNT)
      .expect("quote must exist for seeded direct pool");
    System::reset_events();
    assert_ok!(DeosRouter::swap(
      RuntimeOrigin::signed(ALICE),
      from,
      to,
      SWAP_AMOUNT,
      MIN_AMOUNT_OUT,
      ALICE,
      1000,
    ));
    System::assert_has_event(crate::RuntimeEvent::DeosRouter(
      pallet_deos_router::Event::FeeCollected {
        asset: from,
        amount: quote.router_fee,
        source: ALICE,
        collector: burn_actor_account(),
      },
    ));
    assert!(System::events().iter().any(|record| matches!(
      &record.event,
      crate::RuntimeEvent::DeosRouter(pallet_deos_router::Event::SwapExecuted {
        who,
        from: event_from,
        to: event_to,
        outcome,
      }) if *who == ALICE
        && *event_from == from
        && *event_to == to
        && outcome.total_amount_in == SWAP_AMOUNT
        && outcome.recipient_amount_out == quote.amount_out
        && outcome.legs == quote.legs
    )));
  });
}
