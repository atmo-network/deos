//! Common Test Utilities for Runtime Integration Tests
//!
//! This module provides shared utilities and setup functions for all runtime integration tests,
//! ensuring consistent test environment initialization and reducing code duplication.

use crate::{
  AccountId, AssetConversion, Assets, Balance, Balances, EXISTENTIAL_DEPOSIT, Runtime,
  RuntimeOrigin, Staking, System, configs::AssetKind,
};
use alloc::vec;
use polkadot_sdk::frame_support::{
  assert_ok,
  dispatch::DispatchResult,
  traits::{Currency, Get, Hooks},
};
use polkadot_sdk::sp_std::boxed::Box;
use polkadot_sdk::{
  pallet_asset_conversion::{self, PoolLocator},
  polkadot_runtime_common::BuildStorage,
  sp_io::TestExternalities,
  sp_runtime::{DispatchError, ModuleError},
};
use primitives::assets::TYPE_FOREIGN;

// Standard test accounts
pub const ALICE: AccountId = AccountId::new([1u8; 32]);
pub const BOB: AccountId = AccountId::new([2u8; 32]);
pub const CHARLIE: AccountId = AccountId::new([3u8; 32]);
pub const DAVE: AccountId = AccountId::new([4u8; 32]);
pub const EVE: AccountId = AccountId::new([5u8; 32]);

// Axial Router account from pallet configuration
pub fn axial_router_account() -> AccountId {
  crate::AxialRouter::account_id()
}

// Standard test constants
pub const INITIAL_BALANCE: Balance = 10000000 * EXISTENTIAL_DEPOSIT;

// Test asset IDs using Bitmask Architecture
pub const ASSET_NATIVE: AssetKind = AssetKind::Native;

// Test-only local assets (0x2... — unassigned namespace, safe for tests)
const TYPE_TEST: u32 = 0x2000_0000;
pub const ASSET_A: u32 = TYPE_TEST | 1;
pub const ASSET_B: u32 = TYPE_TEST | 2;
pub const ASSET_D: u32 = TYPE_TEST | 3;
pub const ASSET_E: u32 = TYPE_TEST | 4;

// Foreign Assets (0xF...)
pub const ASSET_FOREIGN: u32 = TYPE_FOREIGN | 1;

// Token-driven actor accounts from pallet configurations
pub fn burning_manager_account() -> AccountId {
  crate::AAA::sovereign_account_id_system(primitives::ecosystem::aaa_ids::BURNING_MANAGER_AAA_ID)
}

pub fn liquidity_actor_account() -> AccountId {
  crate::AAA::sovereign_account_id_system(primitives::ecosystem::aaa_ids::ZAP_MANAGER_AAA_ID)
}

pub fn aaa_fee_sink_account() -> AccountId {
  <Runtime as pallet_aaa::Config>::FeeSink::get()
}

pub fn tmc_pallet_account() -> AccountId {
  crate::TokenMintingCurve::account_id()
}

// Swap test constants
pub const SWAP_AMOUNT: Balance = 20000 * EXISTENTIAL_DEPOSIT;
pub const MIN_AMOUNT_OUT: Balance = 1;
// TMCTOL test constants
pub const MINT_AMOUNT: Balance = 10 * EXISTENTIAL_DEPOSIT;

// Pool constants
pub const LIQUIDITY_AMOUNT: Balance = INITIAL_BALANCE / 2;
pub const MIN_LIQUIDITY: Balance = 0;

/// Initialize test externalities with a clean state
pub fn new_test_ext() -> TestExternalities {
  let mut t = polkadot_sdk::frame_system::GenesisConfig::<Runtime>::default()
    .build_storage()
    .unwrap();
  // Initialize balances for test accounts (sufficient for asset deposits)
  let initial_balances = vec![
    (ALICE, INITIAL_BALANCE),
    (BOB, INITIAL_BALANCE),
    (CHARLIE, INITIAL_BALANCE),
    (DAVE, INITIAL_BALANCE),
    (EVE, INITIAL_BALANCE),
  ];
  polkadot_sdk::pallet_balances::GenesisConfig::<Runtime> {
    balances: initial_balances,
    ..Default::default()
  }
  .assimilate_storage(&mut t)
  .unwrap();
  polkadot_sdk::pallet_assets::GenesisConfig::<Runtime> {
    assets: crate::configs::genesis_protocol_assets(),
    metadata: crate::configs::genesis_protocol_asset_metadata(),
    accounts: vec![],
    next_asset_id: None,
    reserves: vec![],
  }
  .assimilate_storage(&mut t)
  .unwrap();
  // Pallet genesis configs: ED-free accounts + tracked assets
  pallet_axial_router::GenesisConfig::<Runtime>::default()
    .assimilate_storage(&mut t)
    .unwrap();
  pallet_tmc::GenesisConfig::<Runtime> {
    curves: alloc::vec![
      // BLDR TMC curve: mints $BLDR, collateral = $NTVE
      (
        AssetKind::Local(primitives::ecosystem::protocol_tokens::BLDR_ASSET_ID),
        AssetKind::Native,
        primitives::ecosystem::params::PRECISION,
        primitives::ecosystem::params::TMC_SLOPE_PARAMETER,
      ),
    ],
    ..Default::default()
  }
  .assimilate_storage(&mut t)
  .unwrap();
  pallet_aaa::GenesisConfig::<Runtime>::default()
    .assimilate_storage(&mut t)
    .unwrap();
  let mut ext = TestExternalities::new(t);
  ext.execute_with(|| {
    System::set_block_number(1);
    let _ = Staking::on_initialize(1);
  });
  ext
}

/// Primary helper for tests that need seeded assets/accounts.
pub fn seeded_test_ext() -> TestExternalities {
  setup_basic_test_environment()
}

/// Mint test assets to multiple accounts
pub fn create_test_asset(asset_id: u32, owner: &AccountId) -> DispatchResult {
  Assets::force_create(
    RuntimeOrigin::root(),
    asset_id,
    owner.clone().into(),
    true,
    1,
  )
}

/// Mint helper for tests
pub fn mint_tokens(
  asset_id: u32,
  minter: &AccountId,
  beneficiary: &AccountId,
  amount: Balance,
) -> DispatchResult {
  Assets::mint(
    RuntimeOrigin::signed(minter.clone()),
    asset_id,
    beneficiary.clone().into(),
    amount,
  )
}

/// Setup a basic test environment with common assets and accounts
pub fn setup_basic_test_environment() -> TestExternalities {
  let mut ext = new_test_ext();
  ext.execute_with(|| {
    System::set_block_number(1);
    // Create test assets using standard asset IDs for consistency
    let basic_assets = vec![ASSET_A, ASSET_B, ASSET_D, ASSET_E, ASSET_FOREIGN];
    for &asset_id in &basic_assets {
      create_test_asset(asset_id, &ALICE).unwrap();
      // Set ALICE as admin for minting to other accounts
      let _ = Assets::set_team(
        RuntimeOrigin::signed(ALICE),
        asset_id,
        ALICE.into(), // issuer
        ALICE.into(), // admin
        ALICE.into(), // freezer
      );
    }
    // Add native token deposits for system accounts to enable asset operations
    let system_accounts = vec![
      axial_router_account(),
      burning_manager_account(),
      liquidity_actor_account(),
      aaa_fee_sink_account(),
      tmc_pallet_account(),
    ];
    for account in &system_accounts {
      let _ = <Balances as Currency<AccountId>>::deposit_creating(account, INITIAL_BALANCE);
    }
    // Mint assets to test accounts
    let test_accounts = vec![
      ALICE,
      BOB,
      CHARLIE,
      DAVE,
      EVE,
      axial_router_account(),
      burning_manager_account(),
      liquidity_actor_account(),
      tmc_pallet_account(),
    ];
    for &asset_id in &basic_assets {
      for account in &test_accounts {
        let amount = if asset_id == ASSET_FOREIGN && *account == ALICE {
          INITIAL_BALANCE.saturating_mul(1000)
        } else {
          INITIAL_BALANCE
        };
        let _ = mint_tokens(asset_id, &ALICE, account, amount);
      }
    }
    // Create BLDR protocol token asset and fund BLDR domain sovereigns
    setup_bldr_domain();
  });
  ext
}

/// Assert that an operation returns Ok and return the result
#[macro_export]
macro_rules! assert_ok_result {
  ($result:expr) => {
    match $result {
      Ok(result) => result,
      Err(e) => panic!("Expected Ok, got Err: {:?}", e),
    }
  };
}

/// Assert that an operation returns Err with a specific error
#[macro_export]
macro_rules! assert_err {
  ($result:expr, $expected_error:pat) => {
    match $result {
      Err(e) => {
        if let $expected_error = e.error {
          // Expected error pattern matched
        } else {
          panic!(
            "Expected error pattern {:?}, got {:?}",
            stringify!($expected_error),
            e.error
          );
        }
      }
      Ok(_) => panic!("Expected Err, got Ok"),
    }
  };
}

/// Helper to create a new liquidity pool for a given pair of assets
pub fn create_pool(
  origin: RuntimeOrigin,
  asset1: crate::configs::AssetKind,
  asset2: crate::configs::AssetKind,
) -> DispatchResult {
  crate::configs::AssetConversionAdapter::ensure_lp_asset_namespace();
  crate::AssetConversion::create_pool(origin, Box::new(asset1), Box::new(asset2))
}

/// Helper to add liquidity to an existing pool
fn canonical_asset_pair(
  asset1: &crate::configs::AssetKind,
  asset2: &crate::configs::AssetKind,
) -> (crate::configs::AssetKind, crate::configs::AssetKind) {
  if let Ok(pair) =
    <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(asset1, asset2)
  {
    pair
  } else if let Ok(pair) =
    <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(asset2, asset1)
  {
    pair
  } else if asset1 <= asset2 {
    (*asset1, *asset2)
  } else {
    (*asset2, *asset1)
  }
}

#[allow(clippy::too_many_arguments)]
pub fn add_liquidity(
  origin: RuntimeOrigin,
  asset1: crate::configs::AssetKind,
  asset2: crate::configs::AssetKind,
  amount1_desired: Balance,
  amount2_desired: Balance,
  amount1_min: Balance,
  amount2_min: Balance,
  mint_to: &AccountId,
) -> DispatchResult {
  let (canonical_asset1, canonical_asset2) = canonical_asset_pair(&asset1, &asset2);
  let (desired_first, desired_second, min_first, min_second) = if canonical_asset1 == asset1 {
    (amount1_desired, amount2_desired, amount1_min, amount2_min)
  } else {
    (amount2_desired, amount1_desired, amount2_min, amount1_min)
  };

  crate::AssetConversion::add_liquidity(
    origin,
    Box::new(canonical_asset1),
    Box::new(canonical_asset2),
    desired_first,
    desired_second,
    min_first,
    min_second,
    mint_to.clone(),
  )
}

/// Ensure an AssetConversion pool exists, ignoring `PoolExists` and Assets `InUse` errors.
pub fn ensure_asset_conversion_pool(asset1: AssetKind, asset2: AssetKind) {
  let (canonical_asset1, canonical_asset2) = canonical_asset_pair(&asset1, &asset2);
  let result = AssetConversion::create_pool(
    RuntimeOrigin::signed(ALICE),
    Box::new(canonical_asset1),
    Box::new(canonical_asset2),
  );
  if let Err(error) = result {
    // Handle Assets pallet "InUse" error (index 12) - asset already in use
    if let DispatchError::Module(ModuleError {
      index: 12,
      error: [3, 0, 0, 0],
      ..
    }) = &error
    {
      return;
    }
    if let DispatchError::Module(ModuleError {
      index: 12,
      message: Some("InUse"),
      ..
    }) = &error
    {
      return;
    }
    // Handle AssetConversion pallet "PoolExists" error (index 13)
    if let DispatchError::Module(ModuleError {
      index: 13,
      message: Some("PoolExists"),
      ..
    }) = &error
    {
      return;
    }
    panic!("Unexpected AssetConversion pool creation error: {error:?}");
  }
}

/// Creates BLDR protocol token and funds BLDR domain AAA sovereigns with ED.
fn setup_bldr_domain() {
  use primitives::ecosystem::{aaa_ids, protocol_tokens};
  let bldr_id = protocol_tokens::BLDR_ASSET_ID;
  create_test_asset(bldr_id, &ALICE).unwrap();
  let _ = Assets::set_team(
    RuntimeOrigin::signed(ALICE),
    bldr_id,
    ALICE.into(),
    ALICE.into(),
    ALICE.into(),
  );
  let bldr_sovereigns = [
    aaa_ids::BLDR_SPLITTER_AAA_ID,
    aaa_ids::BLDR_ZM_AAA_ID,
    aaa_ids::BLDR_BUCKET_A_AAA_ID,
    aaa_ids::BLDR_TREASURY_AAA_ID,
  ];
  for id in bldr_sovereigns {
    let sov = crate::AAA::sovereign_account_id_system(id);
    let _ = <Balances as Currency<AccountId>>::deposit_creating(&sov, EXISTENTIAL_DEPOSIT);
  }
}

/// Creates NTVE-BLDR XYK pool and seeds it with initial liquidity.
/// Requires BLDR asset to exist (via setup_bldr_domain) and ALICE to have BLDR tokens.
pub fn setup_bldr_pool(bldr_amount: Balance) {
  use polkadot_sdk::frame_support::traits::fungibles::Mutate as FungiblesMutate;
  use primitives::ecosystem::protocol_tokens;
  let bldr_id = protocol_tokens::BLDR_ASSET_ID;
  let bldr_asset = AssetKind::Local(bldr_id);
  // Mint enough to cover liquidity + retain balance to avoid NotExpendable error
  assert_ok!(<crate::Assets as FungiblesMutate<AccountId>>::mint_into(
    bldr_id,
    &ALICE,
    bldr_amount * 2,
  ));
  ensure_asset_conversion_pool(ASSET_NATIVE, bldr_asset);
  // Check LP token for this pool
  let (_, pool_info) = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::iter()
    .find(|(pair, _)| *pair == (ASSET_NATIVE, bldr_asset) || *pair == (bldr_asset, ASSET_NATIVE))
    .expect("BLDR pool must exist");
  // Touch LP token for ALICE so she can receive it
  let _ =
    <crate::Assets as polkadot_sdk::frame_support::traits::AccountTouch<u32, AccountId>>::touch(
      pool_info.lp_token,
      &ALICE,
      &ALICE,
    );
  let result = add_liquidity(
    RuntimeOrigin::signed(ALICE),
    ASSET_NATIVE,
    bldr_asset,
    bldr_amount,
    bldr_amount,
    MIN_LIQUIDITY,
    MIN_LIQUIDITY,
    &ALICE,
  );
  assert_ok!(result);
}

/// Creates NTVE-BLDR XYK pool and seeds it with explicit reserves.
/// Useful when tests need router selection to compare against an intentionally
/// favorable or unfavorable market price.
pub fn setup_bldr_pool_with_reserves(native_amount: Balance, bldr_amount: Balance) {
  use polkadot_sdk::frame_support::traits::fungibles::Mutate as FungiblesMutate;
  use primitives::ecosystem::protocol_tokens;
  let bldr_id = protocol_tokens::BLDR_ASSET_ID;
  let bldr_asset = AssetKind::Local(bldr_id);
  assert_ok!(<crate::Assets as FungiblesMutate<AccountId>>::mint_into(
    bldr_id,
    &ALICE,
    bldr_amount.saturating_mul(2),
  ));
  ensure_asset_conversion_pool(ASSET_NATIVE, bldr_asset);
  let (_, pool_info) = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::iter()
    .find(|(pair, _)| *pair == (ASSET_NATIVE, bldr_asset) || *pair == (bldr_asset, ASSET_NATIVE))
    .expect("BLDR pool must exist");
  let _ =
    <crate::Assets as polkadot_sdk::frame_support::traits::AccountTouch<u32, AccountId>>::touch(
      pool_info.lp_token,
      &ALICE,
      &ALICE,
    );
  assert_ok!(add_liquidity(
    RuntimeOrigin::signed(ALICE),
    ASSET_NATIVE,
    bldr_asset,
    native_amount,
    bldr_amount,
    MIN_LIQUIDITY,
    MIN_LIQUIDITY,
    &ALICE,
  ));
}

/// Returns the LP token AssetKind for a given pool pair.
pub fn get_pool_lp_asset(asset1: AssetKind, asset2: AssetKind) -> AssetKind {
  let (_, pool_info) = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::iter()
    .find(|(pair, _)| *pair == (asset1, asset2) || *pair == (asset2, asset1))
    .expect("pool must exist");
  AssetKind::Local(pool_info.lp_token)
}

/// Sets up the asset conversion infrastructure used by Axial Router tests.
pub fn setup_axial_router_infrastructure() -> Result<(), &'static str> {
  use crate::configs::AssetKind;
  // Create single pool for native ↔ asset pair used by tests
  // Using single pool to avoid "InUse" errors from Assets pallet in test environment
  ensure_asset_conversion_pool(ASSET_NATIVE, AssetKind::Local(ASSET_A));
  assert_ok!(add_liquidity(
    RuntimeOrigin::signed(ALICE),
    ASSET_NATIVE,
    AssetKind::Local(ASSET_A),
    LIQUIDITY_AMOUNT,
    LIQUIDITY_AMOUNT,
    MIN_LIQUIDITY,
    MIN_LIQUIDITY,
    &ALICE,
  ));
  Ok(())
}
