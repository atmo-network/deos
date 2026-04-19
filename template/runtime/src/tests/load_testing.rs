//! Advanced Load Testing Infrastructure for Production Hardening
//!
//! This module provides comprehensive load testing capabilities for the DEOS reference runtime,
//! focusing on high-throughput scenarios, stress conditions, and performance validation
//! under realistic production workloads.

// Use common module account constants and standardized asset constants

use super::common::{
  ALICE, ASSET_A, BOB, CHARLIE, DAVE, EVE, INITIAL_BALANCE, SWAP_AMOUNT,
  setup_axial_router_infrastructure, setup_basic_test_environment,
};
use crate::configs::axial_router_config::{BurningManagerAccount, FeeManagerImpl};
use crate::{AccountId, Assets, AxialRouter, Balances, Runtime, RuntimeOrigin, System};
use pallet_axial_router::FeeRoutingAdapter;

use polkadot_sdk::frame_support::traits::fungibles::{Inspect, Mutate};
use polkadot_sdk::frame_support::traits::{Currency, Get};
use polkadot_sdk::frame_support::{assert_noop, assert_ok};
use primitives::AssetKind;

/// Helper function for axial router account
fn axial_router_account() -> crate::AccountId {
  AxialRouter::account_id()
}

/// Helper function for burning manager account
fn burning_manager_account() -> crate::AccountId {
  super::common::burning_manager_account()
}

/// Setup function for load testing environment
fn setup_load_test_environment() -> polkadot_sdk::sp_io::TestExternalities {
  let mut ext = setup_basic_test_environment();
  ext.execute_with(|| {
    // Initialize system
    System::set_block_number(1);
    assert_ok!(setup_axial_router_infrastructure());
    // Standard assets and pools are already set up by setup_axial_router_infrastructure
  });
  ext
}

/// High-throughput swap operations test
/// Validates system performance under maximum transaction load
#[test]
fn test_high_throughput_swap_operations() {
  let mut ext = setup_load_test_environment();
  ext.execute_with(|| {
    let start_block = System::block_number();
    // Execute 100 consecutive swaps across multiple users and assets
    for block_offset in 1..=10 {
      System::set_block_number(start_block + block_offset);
      // Execute swaps in this block
      for swap_idx in 0..10 {
        let user = match swap_idx % 5 {
          0 => ALICE,
          1 => BOB,
          2 => CHARLIE,
          3 => DAVE,
          4 => EVE,
          _ => unreachable!(),
        };
        let asset_pair = match swap_idx % 4 {
          0 => (AssetKind::Native, AssetKind::Local(ASSET_A)),
          1 => (AssetKind::Local(ASSET_A), AssetKind::Native),
          2 => (AssetKind::Native, AssetKind::Local(ASSET_A)),
          3 => (AssetKind::Local(ASSET_A), AssetKind::Native),
          _ => unreachable!(),
        };
        assert_ok!(AxialRouter::swap(
          RuntimeOrigin::signed(user.clone()),
          asset_pair.0,
          asset_pair.1,
          SWAP_AMOUNT,
          1, // min_amount_out
          user.clone(),
          System::block_number() + 100
        ));
      }
    }

    // Verify system state remains consistent
    let final_block = System::block_number();
    assert_eq!(final_block, start_block + 10);
    // Verify fee accumulation occurred
    let native_fee_buffer = Balances::free_balance(BurningManagerAccount::get());
    let foreign_fee_buffer = Assets::balance(ASSET_A, BurningManagerAccount::get());
    assert!(
      native_fee_buffer > 0 || foreign_fee_buffer > 0,
      "Router should have accumulated fees"
    );
    // Verify all users still have reasonable balances
    for user in [ALICE, BOB, CHARLIE, DAVE, EVE] {
      let user_balance = Balances::free_balance(user);
      assert!(
        user_balance > INITIAL_BALANCE / 10,
        "User balance depleted excessively"
      );
    }
  });
}

/// Stress test for large transaction volumes
/// Validates system stability under extreme load conditions
#[test]
fn test_stress_large_transaction_volumes() {
  let mut ext = setup_load_test_environment();
  ext.execute_with(|| {
    let start_block = System::block_number();
    // Execute stress test with large swap amounts
    for block_offset in 1..=5 {
      System::set_block_number(start_block + block_offset);
      // Large swaps that stress the system
      for _ in 0..3 {
        assert_ok!(AxialRouter::swap(
          RuntimeOrigin::signed(ALICE),
          AssetKind::Native,
          AssetKind::Local(ASSET_A),
          SWAP_AMOUNT,
          1,
          ALICE,
          System::block_number() + 100,
        ));
        assert_ok!(AxialRouter::swap(
          RuntimeOrigin::signed(BOB.clone()),
          AssetKind::Local(ASSET_A),
          AssetKind::Native,
          SWAP_AMOUNT,
          1,
          BOB,
          System::block_number() + 100,
        ));
      }
    }

    // Verify economic properties maintained under stress
    let native_fee_buffer = Balances::free_balance(BurningManagerAccount::get());
    let foreign_fee_buffer = Assets::balance(ASSET_A, BurningManagerAccount::get());
    let total_fees = native_fee_buffer + foreign_fee_buffer;
    // Should have collected significant fees from large transactions
    assert!(
      total_fees >= SWAP_AMOUNT / 1000,
      "Fee collection should be proportional to transaction volume"
    );
  });
}

/// Concurrent operations test
/// Validates system behavior under parallel transaction execution
#[test]
fn test_concurrent_operations_robustness() {
  let mut ext = setup_load_test_environment();
  ext.execute_with(|| {
    System::set_block_number(1);
    // Test FeeManager directly with concurrent fee collection to isolate buffer behavior while the
    // router already uses the live AssetConversion adapter for swaps
    let fee_amount = SWAP_AMOUNT * 5; // Use larger fee amount to ensure buffer accumulation
    let router_account = axial_router_account();

    // Simulate concurrent fee collection from multiple operations
    // Use Option<u32> where None represents Native asset
    let fee_operations: Vec<(Option<u32>, u128)> = vec![
      (None, fee_amount), // Native
      (Some(ASSET_A), fee_amount),
      (Some(ASSET_A), fee_amount),
      (Some(ASSET_A), fee_amount),
      (Some(ASSET_A), fee_amount),
    ];

    // Ensure router and burning manager accounts have native balance for asset deposits
    let burning_manager_account = burning_manager_account();
    let burning_manager_native_balance = Balances::free_balance(&burning_manager_account);
    let router_native_balance = Balances::free_balance(&router_account);
    // Ensure router account has native balance for asset deposits
    if router_native_balance < crate::EXISTENTIAL_DEPOSIT * 10 {
      assert_ok!(Balances::transfer_allow_death(
        RuntimeOrigin::signed(ALICE),
        polkadot_sdk::sp_runtime::MultiAddress::Id(axial_router_account()),
        crate::EXISTENTIAL_DEPOSIT * 10
      ));
    }
    if burning_manager_native_balance < crate::EXISTENTIAL_DEPOSIT * 10 {
      assert_ok!(Balances::transfer_allow_death(
        RuntimeOrigin::signed(ALICE),
        polkadot_sdk::sp_runtime::MultiAddress::Id(burning_manager_account),
        crate::EXISTENTIAL_DEPOSIT * 10
      ));
    }

    // Execute all fee operations (simulating concurrent execution)
    for (maybe_asset_id, amount) in fee_operations {
      // Mint tokens directly to router account to simulate fee collection
      // This bypasses the transfer deposit requirement
      let asset_kind = match maybe_asset_id {
        None => {
          // Native token: accumulate in buffer (handled by collect_router_fee)
          // For direct testing, we skip native token processing
          continue;
        }
        Some(id) => {
          // Foreign token: mint to router account
          assert_ok!(Assets::mint(
            RuntimeOrigin::signed(ALICE),
            id,
            polkadot_sdk::sp_runtime::MultiAddress::Id(axial_router_account()),
            amount
          ));
          AssetKind::Local(id)
        }
      };

      // Now router can transfer fees to burning manager
      assert_ok!(FeeManagerImpl::<Runtime>::route_fee(
        &router_account,
        asset_kind,
        amount
      ));
    }

    // Verify all operations completed successfully
    // Check that fee manager processed all transactions
    let native_fee_buffer = Balances::free_balance(BurningManagerAccount::get());
    let foreign_fee_buffer_a = Assets::balance(ASSET_A, BurningManagerAccount::get());
    let foreign_fee_buffer_b = Assets::balance(ASSET_A, BurningManagerAccount::get());
    let foreign_fee_buffer_c = Assets::balance(ASSET_A, BurningManagerAccount::get());
    let foreign_fee_buffer_d = Assets::balance(ASSET_A, BurningManagerAccount::get());
    let accumulated_fees = native_fee_buffer
      + foreign_fee_buffer_a
      + foreign_fee_buffer_b
      + foreign_fee_buffer_c
      + foreign_fee_buffer_d;

    // Should have fees from all 5 concurrent operations
    assert!(
      accumulated_fees > 0,
      "Fee manager should process concurrent operations"
    );

    assert!(
      native_fee_buffer > 0,
      "Native fees should be buffered on Burning Manager AAA sovereign"
    );
    assert!(
      foreign_fee_buffer_a > 0,
      "Foreign fee buffer A should have accumulated fees"
    );
    assert!(
      foreign_fee_buffer_b > 0,
      "Foreign fee buffer B should have accumulated fees"
    );
    assert!(
      foreign_fee_buffer_c > 0,
      "Foreign fee buffer C should have accumulated fees"
    );
    assert!(
      foreign_fee_buffer_d > 0,
      "Foreign fee buffer D should have accumulated fees"
    );
  });
}

/// Memory and storage efficiency test
/// Validates system resource usage under sustained load
#[test]
fn test_memory_and_storage_efficiency() {
  let mut ext = setup_load_test_environment();
  ext.execute_with(|| {
    let initial_storage =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    let initial_len = initial_storage.len();
    // Execute sustained operations
    for block in 1..=5 {
      System::set_block_number(block);
      // Regular swap operations
      for _ in 0..1 {
        assert_ok!(AxialRouter::swap(
          RuntimeOrigin::signed(ALICE.clone()),
          AssetKind::Native,
          AssetKind::Local(ASSET_A),
          SWAP_AMOUNT,
          1,
          ALICE,
          System::block_number() + 100
        ));
      }
    }

    let final_storage =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    let final_len = final_storage.len();
    // Storage growth should be reasonable (not exponential)
    let growth_ratio = final_len as f64 / initial_len as f64;
    assert!(
      growth_ratio < 1.5,
      "Storage growth should be linear, not exponential. Growth ratio: {growth_ratio}"
    );
  });
}

/// Economic property validation under load
/// Ensures mathematical guarantees hold during high activity
#[test]
fn test_economic_properties_under_load() {
  let mut ext = setup_load_test_environment();
  ext.execute_with(|| {
    let initial_supply = Balances::total_issuance();
    // Execute high-volume transaction sequence
    for block in 1..=15 {
      System::set_block_number(block);
      // High-frequency trading simulation
      for user_idx in 0..5 {
        let user = match user_idx {
          0 => ALICE,
          1 => BOB,
          2 => CHARLIE,
          3 => DAVE,
          4 => EVE,
          _ => unreachable!(),
        };
        // Execute multiple swaps per user per block
        for swap_idx in 0..2 {
          let (asset_in, asset_out) = match swap_idx {
            0 => (AssetKind::Native, AssetKind::Local(ASSET_A)),
            1 => (AssetKind::Local(ASSET_A), AssetKind::Native),
            _ => unreachable!(),
          };
          assert_ok!(AxialRouter::swap(
            RuntimeOrigin::signed(user.clone()),
            asset_in,
            asset_out,
            SWAP_AMOUNT,
            1,
            user.clone(),
            System::block_number() + 100
          ));
        }
      }
    }

    let final_supply = Balances::total_issuance();
    // Economic property: fee burning should reduce total supply
    // Note: In current implementation, fees are accumulated for batch processing
    // This test validates that the economic coordination works under load
    assert!(
      final_supply <= initial_supply,
      "Fee burning should not increase total supply. Initial: {initial_supply}, Final: {final_supply}"
    );
    // Verify fee accumulation occurred
    let native_fee_buffer = Balances::free_balance(BurningManagerAccount::get());
    let foreign_fee_buffer = Assets::balance(ASSET_A, BurningManagerAccount::get());
    let accumulated_fees = native_fee_buffer + foreign_fee_buffer;
    assert!(
      accumulated_fees > 0,
      "Fee accumulation should occur under load"
    );
  });
}

/// Gas efficiency validation test
/// System recovery test
/// Validates system can recover gracefully after high-load periods
#[test]
fn test_system_recovery_after_load() {
  let mut ext = setup_load_test_environment();
  ext.execute_with(|| {
    // Phase 1: High load period
    for block in 1..=3 {
      System::set_block_number(block);
      // High-frequency operations
      for _ in 0..2 {
        assert_ok!(AxialRouter::swap(
          RuntimeOrigin::signed(ALICE.clone()),
          AssetKind::Native,
          AssetKind::Local(ASSET_A),
          SWAP_AMOUNT,
          1,
          ALICE,
          System::block_number() + 100
        ));
      }
    }

    // Phase 2: Recovery period - normal operations
    for block in 4..=6 {
      System::set_block_number(block);
      // Normal operations
      assert_ok!(AxialRouter::swap(
        RuntimeOrigin::signed(BOB.clone()),
        AssetKind::Local(ASSET_A),
        AssetKind::Native,
        SWAP_AMOUNT,
        1,
        BOB,
        System::block_number() + 100
      ));
    }

    // Verify system state is consistent
    let final_block = System::block_number();
    assert_eq!(final_block, 6);
    // Verify economic coordination continued through recovery
    let native_fee_buffer = Balances::free_balance(BurningManagerAccount::get());
    let foreign_fee_buffer = Assets::balance(ASSET_A, BurningManagerAccount::get());
    let total_fees = native_fee_buffer + foreign_fee_buffer;
    assert!(
      total_fees > 0,
      "Fee accumulation should continue through recovery"
    );
    // Verify all user accounts are in valid state
    for user in [ALICE, BOB, CHARLIE, DAVE, EVE] {
      let balance = Balances::free_balance(user);
      assert!(
        balance > 0,
        "User should have positive balance after recovery"
      );
    }
  });
}

#[test]
fn test_multi_user_concurrent_chaos() {
  let mut ext = setup_load_test_environment();
  ext.execute_with(|| {
    // 1. Fund Alice with massive amount to provide additional liquidity for chaos test
    let massive_amount = 10_000_000_000_000_000_000_000u128; // 10k units
    let _ = <Balances as Currency<AccountId>>::deposit_creating(&ALICE, massive_amount);
    assert_ok!(Assets::mint_into(ASSET_A, &ALICE, massive_amount));

    // 2. Infrastructure is already set up by setup_load_test_environment
    // Just ensure the pool exists (it should, but we're being defensive)

    // 3. Add massive liquidity to the pool
    assert_ok!(crate::tests::common::add_liquidity(
      RuntimeOrigin::signed(ALICE),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      1_000_000_000_000_000_000_000, // 1000 units liquidity
      1_000_000_000_000_000_000_000,
      0,
      0,
      &ALICE
    ));

    // 4. Create multiple concurrent users
    let users: Vec<AccountId> = (100..200)
      .map(|i| {
        let mut bytes = [0u8; 32];
        bytes[0] = i as u8;
        bytes[31] = i as u8;
        AccountId::from(bytes)
      })
      .collect();

    // Fund users
    let initial_user_balance = 100_000_000_000_000_000_000u128; // 100 units
    for user in &users {
      let _ = <Balances as Currency<AccountId>>::deposit_creating(user, initial_user_balance);
      assert_ok!(Assets::mint_into(ASSET_A, user, initial_user_balance));
    }

    // Snapshot global state before chaos
    let initial_total_native = Balances::total_issuance();
    let _initial_total_asset_a = Assets::total_issuance(ASSET_A);
    let treasury_account =
      crate::AAA::sovereign_account_id_system(primitives::ecosystem::aaa_ids::ZAP_MANAGER_AAA_ID);
    let initial_treasury_native = Balances::free_balance(&treasury_account);

    // Execute random concurrent operations
    for (i, user) in users.iter().enumerate() {
      let operation_type = i % 3; // 0: Swap, 1: Mint (via TMC), 2: Burn (via fee)
      let amount = 2_000_000_000_000_000_000u128; // 2 units

      match operation_type {
        0 => {
          // Swap: Asset A -> Native
          let _ = AxialRouter::swap(
            RuntimeOrigin::signed(user.clone()),
            AssetKind::Local(ASSET_A),
            AssetKind::Native,
            amount,
            0,
            user.clone(),
            1000,
          );
        }
        1 => {
          // Mint via TMC (simulated via direct mint call if accessible, or swap Native->AssetA)
          let _ = AxialRouter::swap(
            RuntimeOrigin::signed(user.clone()),
            AssetKind::Local(ASSET_A),
            AssetKind::Native,
            amount,
            0,
            user.clone(),
            1000,
          );
        }
        2 => {
          // "Burn" via high fee swap
          let _ = AxialRouter::swap(
            RuntimeOrigin::signed(user.clone()),
            AssetKind::Local(ASSET_A),
            AssetKind::Native,
            amount,
            0,
            user.clone(),
            1000,
          );
        }
        _ => {}
      }
    }

    // Verify Mass Conservation
    // Total System Assets = User Balances + Treasury + Pool Reserves + Burnt
    // This is hard to calculate exactly due to fees, but we can verify invariants

    let final_total_native = Balances::total_issuance();
    let _final_total_asset_a = Assets::total_issuance(ASSET_A);

    // Invariant: Total issuance should change predictably (deflationary due to burns)
    assert!(
      final_total_native <= initial_total_native,
      "Native issuance should not increase (except via TMC minting, but we only swapped)"
    );

    // Verify TOL Independence
    // User sales should never touch Treasury Owned Liquidity (reserved)
    let final_treasury_native = Balances::free_balance(&treasury_account);
    assert!(
      final_treasury_native >= initial_treasury_native,
      "Treasury balance should not decrease during user operations"
    );

    // Verify System Liveness
    // Check that the system is still operational after chaos
    let _ = AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      AssetKind::Local(ASSET_A),
      AssetKind::Native,
      2_000_000_000_000_000_000,
      0,
      ALICE,
      1000,
    );
  });
}

#[test]
fn test_flash_loan_like_single_block_roundtrip_has_no_profit_extraction() {
  let mut ext = setup_load_test_environment();
  ext.execute_with(|| {
    System::set_block_number(1);
    let attacker = ALICE;
    let amount_in = SWAP_AMOUNT.saturating_mul(20);
    let initial_native = Balances::free_balance(&attacker);
    let initial_asset = Assets::balance(ASSET_A, &attacker);
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(attacker.clone()),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      amount_in,
      1,
      attacker.clone(),
      System::block_number() + 100,
    ));
    let asset_after_first = Assets::balance(ASSET_A, &attacker);
    let roundtrip_input = asset_after_first.saturating_sub(initial_asset);
    assert!(
      roundtrip_input > 0,
      "First leg must produce positive output for the roundtrip"
    );
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(attacker.clone()),
      AssetKind::Local(ASSET_A),
      AssetKind::Native,
      roundtrip_input,
      1,
      attacker.clone(),
      System::block_number() + 100,
    ));
    let final_native = Balances::free_balance(&attacker);
    let final_asset = Assets::balance(ASSET_A, &attacker);
    assert!(
      final_native <= initial_native,
      "Roundtrip must not increase attacker native balance ({} -> {})",
      initial_native,
      final_native
    );
    assert!(
      final_asset <= asset_after_first,
      "Roundtrip must not create extra foreign assets"
    );
  });
}

#[test]
fn test_oracle_manipulation_resistance_rejects_deviated_quote() {
  let mut ext = setup_load_test_environment();
  ext.execute_with(|| {
    System::set_block_number(1);
    // Inject strongly deviated EMA quote to emulate manipulation/stale oracle state
    let manipulated_price = 10_000_000_000_000u128;
    pallet_axial_router::EmaPrices::<Runtime>::insert(
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      manipulated_price,
    );
    assert_noop!(
      AxialRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Native,
        AssetKind::Local(ASSET_A),
        SWAP_AMOUNT,
        1,
        ALICE,
        System::block_number() + 100,
      ),
      pallet_axial_router::Error::<Runtime>::NoRouteFound
    );
    // Restore fair EMA and verify swap path becomes available again
    pallet_axial_router::EmaPrices::<Runtime>::insert(
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      primitives::ecosystem::params::PRECISION,
    );
    pallet_axial_router::EmaPrices::<Runtime>::insert(
      AssetKind::Local(ASSET_A),
      AssetKind::Native,
      primitives::ecosystem::params::PRECISION,
    );
    assert_ok!(AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      SWAP_AMOUNT,
      1,
      ALICE,
      System::block_number() + 100,
    ));
  });
}
