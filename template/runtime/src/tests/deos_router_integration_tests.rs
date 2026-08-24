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
use crate::{Actors, Assets, Balances, DeosRouter, Oracle, Runtime, RuntimeOrigin, System};
use pallet_deos_actors::{
  ActorContract, CompletionPolicy, FundingSourcePolicy, Mutability, Step, StepErrorPolicy, Task,
  Trigger,
};
use polkadot_sdk::{
  frame_support::{BoundedVec, assert_noop, assert_ok, traits::Contains},
  pallet_asset_conversion::PoolLocator,
  sp_runtime::traits::Dispatchable,
};
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
fn complete_pool_topology_integrity_rejects_every_corruption_class() {
  use pallet_deos_router::LpPairIntegrity;

  #[derive(Clone, Copy)]
  enum Corruption {
    MissingReverse,
    OrphanReverse,
    MissingOracle,
    PhysicalAlias,
  }

  for corruption in [
    Corruption::MissingReverse,
    Corruption::OrphanReverse,
    Corruption::MissingOracle,
    Corruption::PhysicalAlias,
  ] {
    seeded_test_ext().execute_with(|| {
      assert_ok!(setup_test_environment());
      assert!(
        <crate::configs::deos_router_config::RuntimeLpPairIntegrity as LpPairIntegrity>::validate_complete_topology()
          .is_ok()
      );
      let (pair, pool) = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::iter()
        .next()
        .expect("one pool exists");
      match corruption {
        Corruption::MissingReverse => {
          pallet_deos_router::LpPairByTokenId::<Runtime>::mutate(|pairs| {
            pairs.remove(&pool.lp_token);
          });
        }
        Corruption::OrphanReverse => {
          pallet_deos_router::LpPairByTokenId::<Runtime>::mutate(|pairs| {
            pairs
              .try_insert(
                pool.lp_token.saturating_add(10_000),
                (AssetKind::Local(800_001), AssetKind::Local(800_002)),
              )
              .expect("orphan entry fits");
          });
        }
        Corruption::MissingOracle => {
          pallet_oracle::Feeds::<Runtime>::remove(
            crate::configs::oracle_config::deos_router_pool_feed(pair.0, pair.1),
          );
        }
        Corruption::PhysicalAlias => {
          let id = primitives::TYPE_FOREIGN | 99;
          polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::insert(
            (AssetKind::Local(id), AssetKind::Foreign(id)),
            polkadot_sdk::pallet_asset_conversion::PoolInfo {
              lp_token: pool.lp_token.saturating_add(10_000),
            },
          );
        }
      }
      assert!(
        <crate::configs::deos_router_config::RuntimeLpPairIntegrity as LpPairIntegrity>::validate_complete_topology()
          .is_err()
      );
    });
  }
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
fn raw_asset_conversion_genesis_topology_is_rejected_fail_closed() {
  use pallet_deos_router::LpPairIntegrity;

  seeded_test_ext().execute_with(|| {
    assert!(
      <crate::configs::deos_router_config::RuntimeLpPairIntegrity as LpPairIntegrity>::validate_genesis_topology()
        .is_ok()
    );
    polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::insert(
      (AssetKind::Native, AssetKind::Local(ASSET_A)),
      polkadot_sdk::pallet_asset_conversion::PoolInfo { lp_token: 700_001 },
    );
    assert_eq!(
      <crate::configs::deos_router_config::RuntimeLpPairIntegrity as LpPairIntegrity>::validate_genesis_topology(),
      Err(
        "raw Asset Conversion genesis pools are unsupported; create complete pools through the permissionless DEOS lifecycle after genesis"
      )
    );
  });
}

#[test]
fn canonical_pool_creation_rolls_back_when_lp_index_is_full() {
  seeded_test_ext().execute_with(|| {
    pallet_deos_router::LpPairByTokenId::<Runtime>::mutate(|pairs| {
      for index in 0..500u32 {
        pairs
          .try_insert(
            100_000 + index,
            (
              AssetKind::Local(200_000 + index * 2),
              AssetKind::Local(200_001 + index * 2),
            ),
          )
          .expect("production LP index capacity");
      }
    });
    let pair = (AssetKind::Native, AssetKind::Local(ASSET_A));
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_noop!(
      DeosRouter::create_pool(RuntimeOrigin::signed(ALICE), pair.0, pair.1),
      pallet_deos_router::Error::<Runtime>::LpPairCapacityExceeded
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before
    );
    let pool_id = <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::pool_id(
      &pair.0, &pair.1,
    )
    .expect("canonical pool identity");
    assert!(!polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::contains_key(pool_id));
    assert!(!pallet_oracle::Feeds::<Runtime>::contains_key(
      crate::configs::oracle_config::deos_router_pool_feed(pool_id.0, pool_id.1)
    ));
  });
}

#[test]
fn canonical_pool_creation_rolls_back_on_lp_identity_collision() {
  seeded_test_ext().execute_with(|| {
    let pair = (AssetKind::Native, AssetKind::Local(ASSET_A));
    crate::configs::AssetConversionAdapter::ensure_lp_asset_namespace();
    let expected_lp = polkadot_sdk::pallet_asset_conversion::NextPoolAssetId::<Runtime>::get()
      .expect("LP namespace initialized");
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      expected_lp,
      ALICE.into(),
      true,
      1,
    ));
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert!(DeosRouter::create_pool(RuntimeOrigin::signed(ALICE), pair.0, pair.1).is_err());
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before
    );
    let pool_id = <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::pool_id(
      &pair.0, &pair.1,
    )
    .expect("canonical pool identity");
    assert!(!polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::contains_key(pool_id));
    assert!(pallet_deos_router::LpPairByTokenId::<Runtime>::get().is_empty());
    assert!(!pallet_oracle::Feeds::<Runtime>::contains_key(
      crate::configs::oracle_config::deos_router_pool_feed(pool_id.0, pool_id.1)
    ));
  });
}

#[test]
fn canonical_pool_creation_rolls_back_on_lp_identity_mismatch() {
  seeded_test_ext().execute_with(|| {
    let pair = (AssetKind::Native, AssetKind::Local(ASSET_A));
    crate::configs::assets_config::set_force_lp_identity_mismatch(true);
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_noop!(
      DeosRouter::create_pool(RuntimeOrigin::signed(ALICE), pair.0, pair.1),
      polkadot_sdk::sp_runtime::DispatchError::Other("LP identity mismatch")
    );
    crate::configs::assets_config::set_force_lp_identity_mismatch(false);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before
    );
    let pool_id = <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::pool_id(
      &pair.0, &pair.1,
    )
    .expect("canonical pool identity");
    assert!(!polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::contains_key(pool_id));
    assert!(pallet_deos_router::LpPairByTokenId::<Runtime>::get().is_empty());
  });
}

#[test]
fn canonical_pool_creation_rolls_back_complete_topology_after_underlying_mutation() {
  seeded_test_ext().execute_with(|| {
    let pair = (AssetKind::Native, AssetKind::Local(ASSET_A));
    crate::configs::assets_config::set_fail_after_pool_creation(true);
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_noop!(
      DeosRouter::create_pool(RuntimeOrigin::signed(ALICE), pair.0, pair.1),
      polkadot_sdk::sp_runtime::DispatchError::Other("Injected post-pool lifecycle failure")
    );
    crate::configs::assets_config::set_fail_after_pool_creation(false);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before
    );
    let pool_id = <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::pool_id(
      &pair.0, &pair.1,
    )
    .expect("canonical pool identity");
    assert!(!polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::contains_key(pool_id));
    assert!(pallet_deos_router::LpPairByTokenId::<Runtime>::get().is_empty());
    assert!(!pallet_oracle::Feeds::<Runtime>::contains_key(
      crate::configs::oracle_config::deos_router_pool_feed(pool_id.0, pool_id.1)
    ));
  });
}

#[test]
fn canonical_pool_creation_builds_complete_topology_and_closes_raw_bypass() {
  seeded_test_ext().execute_with(|| {
    let asset_a = AssetKind::Native;
    let asset_b = AssetKind::Local(ASSET_A);
    let raw = crate::RuntimeCall::AssetConversion(
      polkadot_sdk::pallet_asset_conversion::Call::create_pool {
        asset1: Box::new(asset_a),
        asset2: Box::new(asset_b),
      },
    );
    assert!(!crate::configs::RuntimeCallFilter::contains(&raw));
    assert_noop!(
      raw.dispatch(RuntimeOrigin::signed(ALICE)),
      polkadot_sdk::frame_system::Error::<Runtime>::CallFiltered
    );

    assert_ok!(DeosRouter::create_pool(
      RuntimeOrigin::signed(ALICE),
      asset_a,
      asset_b,
    ));
    let pool_id = <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::pool_id(
      &asset_a, &asset_b,
    )
    .expect("canonical pool identity");
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get(pool_id)
      .expect("underlying pool exists");
    assert_eq!(
      DeosRouter::lp_pair_by_token_id(pool.lp_token),
      Some(pool_id)
    );
    let forward = crate::configs::oracle_config::deos_router_pool_feed(pool_id.0, pool_id.1);
    let reverse = crate::configs::oracle_config::deos_router_pool_feed(pool_id.1, pool_id.0);
    assert!(pallet_oracle::Feeds::<Runtime>::contains_key(forward));
    assert!(pallet_oracle::Feeds::<Runtime>::contains_key(reverse));
  });
}

#[test]
fn raw_asset_conversion_swaps_are_call_filtered_fail_closed() {
  let path = vec![
    Box::new(AssetKind::Native),
    Box::new(AssetKind::Local(ASSET_A)),
  ];
  let exact_input = crate::RuntimeCall::AssetConversion(
    polkadot_sdk::pallet_asset_conversion::Call::swap_exact_tokens_for_tokens {
      path: path.clone(),
      amount_in: SWAP_AMOUNT,
      amount_out_min: MIN_AMOUNT_OUT,
      send_to: ALICE,
      keep_alive: true,
    },
  );
  let exact_output = crate::RuntimeCall::AssetConversion(
    polkadot_sdk::pallet_asset_conversion::Call::swap_tokens_for_exact_tokens {
      path,
      amount_out: MIN_AMOUNT_OUT,
      amount_in_max: SWAP_AMOUNT,
      send_to: ALICE,
      keep_alive: true,
    },
  );
  assert!(!crate::configs::RuntimeCallFilter::contains(&exact_input));
  assert!(!crate::configs::RuntimeCallFilter::contains(&exact_output));
  seeded_test_ext().execute_with(|| {
    assert_noop!(
      exact_input.clone().dispatch(RuntimeOrigin::signed(ALICE)),
      polkadot_sdk::frame_system::Error::<Runtime>::CallFiltered
    );
    assert_noop!(
      exact_output.clone().dispatch(RuntimeOrigin::signed(ALICE)),
      polkadot_sdk::frame_system::Error::<Runtime>::CallFiltered
    );
  });

  let router = crate::RuntimeCall::DeosRouter(pallet_deos_router::Call::swap {
    from: AssetKind::Native,
    to: AssetKind::Local(ASSET_A),
    amount_in: SWAP_AMOUNT,
    min_amount_out: MIN_AMOUNT_OUT,
    recipient: ALICE,
    deadline: 10,
  });
  assert!(crate::configs::RuntimeCallFilter::contains(&router));
  let router_exact_output =
    crate::RuntimeCall::DeosRouter(pallet_deos_router::Call::swap_exact_output {
      from: AssetKind::Native,
      to: AssetKind::Local(ASSET_A),
      amount_out: MIN_AMOUNT_OUT,
      max_amount_in: SWAP_AMOUNT,
      recipient: ALICE,
      deadline: 10,
    });
  assert!(crate::configs::RuntimeCallFilter::contains(
    &router_exact_output
  ));
}

#[test]
fn protected_pool_balance_is_included_in_reserves() {
  use polkadot_sdk::frame_support::traits::{
    fungible::Inspect,
    tokens::{Fortitude::Polite, Preservation::Preserve},
  };

  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let pair = (AssetKind::Native, AssetKind::Local(ASSET_A));
    let pool_id = <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::pool_id(
      &pair.0, &pair.1,
    )
    .expect("canonical pool identity");
    let pool_account =
      <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::address(&pool_id)
        .expect("canonical pool account");
    let full_native = Balances::balance(&pool_account);
    let preserve_reducible = Balances::reducible_balance(&pool_account, Preserve, Polite);
    let (native_reserve, _) =
      crate::AssetConversion::get_reserves(pair.0, pair.1).expect("pool reserves exist");
    assert_eq!(native_reserve, full_native);
    assert!(native_reserve > preserve_reducible);
  });
}

#[test]
fn unrelated_non_sufficient_asset_does_not_change_quote() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let asset_in = AssetKind::Local(ASSET_A);
    let asset_out = AssetKind::Native;
    let amount_in = SWAP_AMOUNT;
    let quote_before = crate::AssetConversion::quote_price_exact_tokens_for_tokens(
      asset_in, asset_out, amount_in, true,
    );
    let pool_id = <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::pool_id(
      &asset_in, &asset_out,
    )
    .expect("canonical pool identity");
    let pool_account =
      <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::address(&pool_id)
        .expect("canonical pool account");
    const UNRELATED: u32 = 900_001;
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      UNRELATED,
      ALICE.into(),
      false,
      1,
    ));
    assert_ok!(Assets::mint(
      RuntimeOrigin::signed(ALICE),
      UNRELATED,
      ALICE.into(),
      1_000,
    ));
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      UNRELATED,
      pool_account.into(),
      1,
    ));
    assert_eq!(
      crate::AssetConversion::quote_price_exact_tokens_for_tokens(
        asset_in, asset_out, amount_in, true,
      ),
      quote_before
    );
  });
}

#[test]
fn router_exact_output_swap_enforces_fee_conservation() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let from = AssetKind::Local(ASSET_A);
    let to = AssetKind::Native;
    let amount_out = MIN_AMOUNT_OUT;
    let quote = DeosRouter::quote_exact_out(ALICE, from, to, amount_out)
      .expect("exact-output quote must exist for seeded direct pool");
    let sender_before = Assets::balance(ASSET_A, ALICE);
    let recipient_before = Balances::free_balance(ALICE);
    let burn_before = Assets::balance(ASSET_A, burn_actor_account());

    assert_ok!(DeosRouter::swap_exact_output(
      RuntimeOrigin::signed(ALICE),
      from,
      to,
      amount_out,
      quote.amount_in,
      ALICE,
      1_000,
    ));

    let sender_input = sender_before - Assets::balance(ASSET_A, ALICE);
    let burn_credit = Assets::balance(ASSET_A, burn_actor_account()) - burn_before;
    assert_eq!(sender_input, quote.amount_in);
    assert_eq!(sender_input, quote.router_fee + quote.amount_after_fee);
    assert_eq!(burn_credit, quote.router_fee);
    assert!(Balances::free_balance(ALICE) - recipient_before >= amount_out);
  });
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
fn router_oracle_capacity_failure_rolls_back_the_exact_composed_state() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(setup_test_environment());
    let from = AssetKind::Local(ASSET_A);
    let to = ASSET_NATIVE;
    let feed = crate::configs::oracle_config::deos_router_pool_feed(from, to);
    let producer = deos_router_account();
    let spot = 10u128.pow(u32::from(feed.scale));
    assert_ok!(Oracle::publish_from(producer.clone(), feed, spot));
    let contract_steps = BoundedVec::try_from(vec![Step {
      precondition: None,
      task: Task::StopCycle,
      on_error: StepErrorPolicy::AbortCycle,
    }])
    .expect("one inert step fits");
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      Some(ActorContract {
        trigger: Trigger::observation_crossing(
          feed,
          pallet_deos_actors::CrossingDirection::Rising,
          u128::MAX,
          0,
        ),
        cooldown_blocks: 0,
        window: None,
        steps: contract_steps,
        funding: FundingSourcePolicy::RuntimePolicy,
        completion: CompletionPolicy::Persistent,
        auto_close_at_cycle_nonce: None,
      }),
    ));
    let capacity = <Runtime as pallet_deos_actors::Config>::MaxCrossingTransitionsPerFeed::get();
    for offset in 1..=capacity {
      let decrement =
        spot.saturating_mul(u128::from(offset)) / u128::from(capacity).saturating_mul(100);
      assert_ok!(Oracle::publish_from(
        producer.clone(),
        feed,
        spot.saturating_sub(decrement),
      ));
    }
    assert_eq!(
      pallet_deos_actors::CrossingTransitionQueues::<Runtime>::get(feed)
        .expect("full transition queue exists")
        .len() as u32,
      capacity
    );
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);

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
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before
    );
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
      pallet_deos_router::pallet::Error::<Runtime>::InvalidPoolPair
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
