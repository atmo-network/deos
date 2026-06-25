use crate as pallet_axial_router;

use polkadot_sdk::frame_support::traits::fungible::Mutate as FungibleMutate;
use polkadot_sdk::frame_support::traits::fungibles::Mutate as FungiblesMutate;
use polkadot_sdk::frame_support::traits::tokens::{Fortitude, Precision, Preservation};
use polkadot_sdk::frame_support::{
  PalletId, construct_runtime, derive_impl,
  traits::{ConstU32, ConstU64, ConstU128, Currency, ExistenceRequirement, Get},
};
use polkadot_sdk::frame_system;
use polkadot_sdk::sp_runtime::{
  BuildStorage, DispatchError, Perbill, Permill,
  testing::H256,
  traits::{BlakeTwo256, IdentityLookup},
};

use crate::types::AssetKind;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::vec;

// State containers for stateful mocks
thread_local! {
    // AMM Pools: (AssetA, AssetB) -> (ReserveA, ReserveB)
    // Key is sorted: (min, max)
    pub static POOLS: RefCell<BTreeMap<(AssetKind, AssetKind), (u128, u128)>> = const { RefCell::new(BTreeMap::new()) };

    // TMC Curves: Token Asset -> (Allowed Collateral Asset, Native Amount per 1 Unit)
    pub static TMC_RATES: RefCell<BTreeMap<AssetKind, (AssetKind, u128)>> = const { RefCell::new(BTreeMap::new()) };

    // Oracle Prices: (AssetA, AssetB) -> Price
    pub static ORACLE_PRICES: RefCell<BTreeMap<(AssetKind, AssetKind), u128>> = const { RefCell::new(BTreeMap::new()) };

    // Fee accumulator for verification
    pub static COLLECTED_FEES: RefCell<Vec<(u64, AssetKind, u128)>> = const { RefCell::new(Vec::new()) };

    // Deterministic fee-routing failure switch for atomicity regressions
    pub static FORCE_FEE_FAILURE: RefCell<bool> = const { RefCell::new(false) };
}

// Helper methods to setup state
pub fn set_pool(asset_a: AssetKind, asset_b: AssetKind, reserve_a: u128, reserve_b: u128) {
  POOLS.with(|p| {
    let mut pools = p.borrow_mut();
    if asset_a < asset_b {
      pools.insert((asset_a, asset_b), (reserve_a, reserve_b));
    } else {
      pools.insert((asset_b, asset_a), (reserve_b, reserve_a));
    }
  });
}

pub fn set_tmc_rate(asset: AssetKind, rate: u128) {
  TMC_RATES.with(|r| {
    // Backward-compatible helper for tests that expect AssetKind::Local(1) collateral.
    r.borrow_mut().insert(asset, (AssetKind::Local(1), rate));
  });
}

pub fn set_tmc_curve(token_asset: AssetKind, foreign_asset: AssetKind, rate: u128) {
  TMC_RATES.with(|r| {
    r.borrow_mut().insert(token_asset, (foreign_asset, rate));
  });
}

pub fn set_oracle_price(asset_a: AssetKind, asset_b: AssetKind, price: u128) {
  ORACLE_PRICES.with(|p| {
    p.borrow_mut().insert((asset_a, asset_b), price);
  });
}

pub fn get_collected_fees() -> Vec<(u64, AssetKind, u128)> {
  COLLECTED_FEES.with(|f| f.borrow().clone())
}

pub fn set_force_fee_failure(should_fail: bool) {
  FORCE_FEE_FAILURE.with(|flag| *flag.borrow_mut() = should_fail);
}

pub fn get_pool(asset_a: AssetKind, asset_b: AssetKind) -> Option<(u128, u128)> {
  let key = if asset_a < asset_b {
    (asset_a, asset_b)
  } else {
    (asset_b, asset_a)
  };
  POOLS.with(|p| p.borrow().get(&key).cloned())
}

type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
  pub struct Test {
    System: frame_system,
    Balances: polkadot_sdk::pallet_balances,
    Assets: polkadot_sdk::pallet_assets,
    // We don't strictly need real AssetConversion pallet since we mock the adapter,
    // but keeping it doesn't hurt if we want types from it.
    AssetConversion: polkadot_sdk::pallet_asset_conversion,
    AxialRouter: pallet_axial_router,
  }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
  type Block = Block;
  type AccountId = u64;
  type Lookup = IdentityLookup<Self::AccountId>;
  type Hash = H256;
  type Hashing = BlakeTwo256;
  type AccountData = polkadot_sdk::pallet_balances::AccountData<u128>;
}

impl polkadot_sdk::pallet_balances::Config for Test {
  type MaxLocks = ();
  type MaxReserves = ();
  type ReserveIdentifier = [u8; 8];
  type Balance = u128;
  type DustRemoval = ();
  type RuntimeEvent = RuntimeEvent;
  type ExistentialDeposit = ConstU128<1>;
  type AccountStore = System;
  type WeightInfo = ();
  type FreezeIdentifier = ();
  type MaxFreezes = ();
  type RuntimeHoldReason = ();
  type RuntimeFreezeReason = ();
  type DoneSlashHandler = ();
}

impl polkadot_sdk::pallet_assets::Config for Test {
  type RuntimeEvent = RuntimeEvent;
  type Balance = u128;
  type AssetId = u32;
  type AssetIdParameter = u32;
  type Currency = Balances;
  type CreateOrigin = polkadot_sdk::frame_support::traits::AsEnsureOriginWithArg<
    frame_system::EnsureSigned<Self::AccountId>,
  >;
  type ForceOrigin = frame_system::EnsureRoot<Self::AccountId>;
  type AssetDeposit = ConstU128<1>;
  type AssetAccountDeposit = ConstU128<1>;
  type MetadataDepositBase = ConstU128<1>;
  type MetadataDepositPerByte = ConstU128<1>;
  type ApprovalDeposit = ConstU128<1>;
  type StringLimit = ConstU32<50>;
  type Freezer = ();
  type Extra = ();
  type ReserveData = ();
  type CallbackHandle = ();
  type WeightInfo = ();
  type RemoveItemsLimit = ConstU32<5>;
  type Holder = ();
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = AssetBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct AssetBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl polkadot_sdk::pallet_assets::BenchmarkHelper<u32, ()> for AssetBenchmarkHelper {
  fn create_asset_id_parameter(id: u32) -> u32 {
    id
  }
  fn create_reserve_id_parameter(_id: u32) -> () {
    ()
  }
}

pub struct NativeOrAssetIdConverter;
impl polkadot_sdk::sp_runtime::traits::Convert<AssetKind, polkadot_sdk::sp_runtime::Either<(), u32>>
  for NativeOrAssetIdConverter
{
  fn convert(a: AssetKind) -> polkadot_sdk::sp_runtime::Either<(), u32> {
    match a {
      AssetKind::Native => polkadot_sdk::sp_runtime::Either::Left(()),
      AssetKind::Local(id) | AssetKind::Foreign(id) => polkadot_sdk::sp_runtime::Either::Right(id),
    }
  }
}

type AssetConversionAssets = polkadot_sdk::frame_support::traits::fungible::UnionOf<
  Balances,
  Assets,
  NativeOrAssetIdConverter,
  AssetKind,
  u64,
>;

pub struct LiquidityWithdrawalFee;
impl Get<Permill> for LiquidityWithdrawalFee {
  fn get() -> Permill {
    Permill::zero()
  }
}

// Minimal config for real pallet-asset-conversion (unused logic, but needed for compilation)
impl polkadot_sdk::pallet_asset_conversion::Config for Test {
  type RuntimeEvent = RuntimeEvent;
  type Balance = u128;
  type HigherPrecisionBalance = u128;
  type AssetKind = AssetKind;
  type Assets = AssetConversionAssets;
  type PoolId = (AssetKind, AssetKind);
  type PoolLocator = MockPoolLocator;
  type PoolAssetId = u32;
  type PoolAssets = Assets;
  type PoolSetupFee = ConstU128<100>;
  type PoolSetupFeeAsset = PoolSetupFeeAsset;
  type LiquidityWithdrawalFee = LiquidityWithdrawalFee;
  type LPFee = ConstU32<3>;
  type PalletId = PalletIdStub;
  type MaxSwapPathLength = ConstU32<4>;
  type WeightInfo = ();
  type PoolSetupFeeTarget = ();
  type MintMinLiquidity = ConstU128<100>;
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = AssetConversionBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct AssetConversionBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl polkadot_sdk::pallet_asset_conversion::BenchmarkHelper<AssetKind>
  for AssetConversionBenchmarkHelper
{
  fn create_pair(seed1: u32, seed2: u32) -> (AssetKind, AssetKind) {
    (AssetKind::Local(seed1), AssetKind::Local(seed2))
  }
}

pub struct MockPoolLocator;
impl polkadot_sdk::pallet_asset_conversion::PoolLocator<u64, AssetKind, (AssetKind, AssetKind)>
  for MockPoolLocator
{
  fn pool_id(a: &AssetKind, b: &AssetKind) -> Result<(AssetKind, AssetKind), ()> {
    Ok((*a, *b))
  }
  fn address(_: &(AssetKind, AssetKind)) -> Result<u64, ()> {
    Ok(12345)
  }
}

pub struct PoolSetupFeeAsset;
impl Get<AssetKind> for PoolSetupFeeAsset {
  fn get() -> AssetKind {
    AssetKind::Native
  }
}

pub struct NativeAsset;
impl Get<AssetKind> for NativeAsset {
  fn get() -> AssetKind {
    AssetKind::Native
  }
}

pub struct PalletIdStub;
impl Get<PalletId> for PalletIdStub {
  fn get() -> PalletId {
    PalletId(*b"asconv00")
  }
}

// MOCK IMPLEMENTATIONS

pub struct MockTmcPallet;
impl pallet_axial_router::types::TmcInterface<u64, u128> for MockTmcPallet {
  fn has_curve(asset: AssetKind) -> bool {
    TMC_RATES.with(|r| r.borrow().contains_key(&asset))
  }
  fn supports_collateral(token_asset: AssetKind, foreign_asset: AssetKind) -> bool {
    TMC_RATES
      .with(|r| r.borrow().get(&token_asset).cloned())
      .map(|(configured_collateral, _)| configured_collateral == foreign_asset)
      .unwrap_or(false)
  }
  fn calculate_recipient_receives(
    token_asset: AssetKind,
    foreign_amount: u128,
  ) -> Result<u128, DispatchError> {
    let (_, rate) = TMC_RATES
      .with(|r| r.borrow().get(&token_asset).cloned())
      .ok_or(DispatchError::Other("No TMC Curve"))?;

    // Simulating Linear Mint: Amount * Rate, then returning only the
    // recipient allocation because this is the router-facing output.
    Ok(foreign_amount.saturating_mul(rate) / 4)
  }
  fn mint_with_distribution(
    who: &u64,
    recipient: &u64,
    token_asset: AssetKind,
    foreign_asset: AssetKind,
    foreign_amount: u128,
  ) -> Result<u128, DispatchError> {
    if !Self::supports_collateral(token_asset, foreign_asset) {
      return Err(DispatchError::Other("TMC collateral mismatch"));
    }

    let (_, rate) = TMC_RATES
      .with(|r| r.borrow().get(&token_asset).cloned())
      .ok_or(DispatchError::Other("No TMC Curve"))?;
    let total_amount = foreign_amount.saturating_mul(rate);

    // 1. Burn foreign asset from user
    match foreign_asset {
      AssetKind::Native => {
        return Err(DispatchError::Other("TMC invalid asset"));
      }
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        <Assets as FungiblesMutate<u64>>::burn_from(
          id,
          who,
          foreign_amount,
          Preservation::Expendable,
          Precision::Exact,
          Fortitude::Polite,
        )?;
      }
    }

    // 2. Apply 33.3% allocation to user (matching real TMC behavior)
    // For simplicity in mock: user gets 25% (1/4), rest goes to zap account (888)
    let user_allocation = total_amount / 4;
    let zap_allocation = total_amount - user_allocation;

    // Mint Native asset to recipient (only their allocation)
    <Balances as FungibleMutate<u64>>::mint_into(recipient, user_allocation)?;
    // Mint to zap account
    <Balances as FungibleMutate<u64>>::mint_into(&888u64, zap_allocation)?;

    // Return the recipient amount because this is the router-facing output.
    Ok(user_allocation)
  }
}

pub struct MockFeeAdapter;
impl pallet_axial_router::types::FeeRoutingAdapter<u64, u128> for MockFeeAdapter {
  fn route_fee(who: &u64, asset: AssetKind, amount: u128) -> Result<(), DispatchError> {
    if FORCE_FEE_FAILURE.with(|flag| *flag.borrow()) {
      return Err(DispatchError::Other("Forced fee routing failure"));
    }
    COLLECTED_FEES.with(|f| f.borrow_mut().push((*who, asset, amount)));
    let burn_mgr = 123;

    match asset {
      AssetKind::Native => {
        <Balances as Currency<u64>>::transfer(
          who,
          &burn_mgr,
          amount,
          ExistenceRequirement::KeepAlive,
        )?;
      }
      AssetKind::Local(id) | AssetKind::Foreign(id) => {
        // Simulate transfer via Burn + Mint since Fungibles::Transfer might be tricky to import
        <Assets as FungiblesMutate<u64>>::burn_from(
          id,
          who,
          amount,
          Preservation::Expendable,
          Precision::Exact,
          Fortitude::Polite,
        )?;
        <Assets as FungiblesMutate<u64>>::mint_into(id, &burn_mgr, amount)?;
      }
    }
    Ok(())
  }
}

pub struct MockPriceOracle;
impl pallet_axial_router::types::PriceOracle<u128> for MockPriceOracle {
  fn update_ema_price(
    asset_in: AssetKind,
    asset_out: AssetKind,
    price: u128,
  ) -> Result<(), DispatchError> {
    set_oracle_price(asset_in, asset_out, price);
    Ok(())
  }
  fn get_ema_price(asset_in: AssetKind, asset_out: AssetKind) -> Option<u128> {
    ORACLE_PRICES.with(|p| p.borrow().get(&(asset_in, asset_out)).cloned())
  }
  fn validate_price_deviation(_: AssetKind, _: AssetKind, _: u128) -> Result<(), DispatchError> {
    Ok(()) // Always valid for now
  }
}

// Adapter for AssetConversionApi
pub struct MockAssetConversionAdapter;
impl pallet_axial_router::types::AssetConversionApi<u64, u128> for MockAssetConversionAdapter {
  fn get_pool_id(asset_a: AssetKind, asset_b: AssetKind) -> Option<(AssetKind, AssetKind)> {
    POOLS.with(|p| {
      let pools = p.borrow();
      if pools.contains_key(&(asset_a, asset_b)) {
        Some((asset_a, asset_b))
      } else if pools.contains_key(&(asset_b, asset_a)) {
        Some((asset_b, asset_a))
      } else {
        None
      }
    })
  }

  fn get_pool_reserves(pool_id: (AssetKind, AssetKind)) -> Option<(u128, u128)> {
    POOLS.with(|p| p.borrow().get(&pool_id).cloned())
  }

  fn quote_price_exact_tokens_for_tokens(
    asset_in: AssetKind,
    asset_out: AssetKind,
    amount_in: u128,
    _include_fee: bool,
  ) -> Option<u128> {
    // 1. Identify Pool
    let pool_id = Self::get_pool_id(asset_in, asset_out)?;
    let (res_a, res_b) = Self::get_pool_reserves(pool_id)?;

    // 2. Identify Reserves (In vs Out)
    let (reserve_in, reserve_out) = if pool_id.0 == asset_in {
      (res_a, res_b)
    } else {
      (res_b, res_a)
    };

    if reserve_in == 0 || reserve_out == 0 {
      return None;
    }

    // 3. XYK Formula: y_out = (x_in * y_res) / (x_res + x_in)
    let amount_out =
      (amount_in.saturating_mul(reserve_out)) / (reserve_in.saturating_add(amount_in));
    Some(amount_out)
  }

  fn swap_exact_tokens_for_tokens(
    who: u64,
    path: vec::Vec<AssetKind>,
    amount_in: u128,
    min_amount_out: u128,
    recipient: u64,
    keep_alive: bool,
  ) -> Result<u128, DispatchError> {
    if path.len() < 2 {
      return Err(DispatchError::Other("Path too short"));
    }

    let mut current_amount = amount_in;
    let mut current_holder = who;

    for window in path.windows(2) {
      let hop_in = window[0];
      let hop_out = window[1];
      let hop_recipient = if window[1] == *path.last().unwrap() {
        recipient
      } else {
        who
      };

      let hop_amount_out =
        Self::quote_price_exact_tokens_for_tokens(hop_in, hop_out, current_amount, true)
          .ok_or(DispatchError::Other("Pool not found for hop"))?;

      // Update pool reserves
      let pool_id =
        Self::get_pool_id(hop_in, hop_out).ok_or(DispatchError::Other("Pool missing"))?;
      POOLS.with(|p| {
        let mut pools = p.borrow_mut();
        let (res_a, res_b) = pools.get(&pool_id).cloned().unwrap();
        let (new_res_a, new_res_b) = if pool_id.0 == hop_in {
          (
            res_a.saturating_add(current_amount),
            res_b.saturating_sub(hop_amount_out),
          )
        } else {
          (
            res_a.saturating_sub(hop_amount_out),
            res_b.saturating_add(current_amount),
          )
        };
        pools.insert(pool_id, (new_res_a, new_res_b));
      });

      // Take from current holder
      match hop_in {
        AssetKind::Native => {
          <Balances as Currency<u64>>::transfer(
            &current_holder,
            &12345,
            current_amount,
            if keep_alive {
              ExistenceRequirement::KeepAlive
            } else {
              ExistenceRequirement::AllowDeath
            },
          )?;
        }
        AssetKind::Local(id) | AssetKind::Foreign(id) => {
          <Assets as FungiblesMutate<u64>>::burn_from(
            id,
            &current_holder,
            current_amount,
            if keep_alive {
              Preservation::Preserve
            } else {
              Preservation::Expendable
            },
            Precision::Exact,
            Fortitude::Polite,
          )?;
          <Assets as FungiblesMutate<u64>>::mint_into(id, &12345, current_amount)?;
        }
      }

      // Give to hop recipient
      match hop_out {
        AssetKind::Native => {
          <Balances as FungibleMutate<u64>>::mint_into(&hop_recipient, hop_amount_out)?;
        }
        AssetKind::Local(id) | AssetKind::Foreign(id) => {
          <Assets as FungiblesMutate<u64>>::mint_into(id, &hop_recipient, hop_amount_out)?;
        }
      }

      current_amount = hop_amount_out;
      current_holder = hop_recipient;
    }

    if current_amount < min_amount_out {
      return Err(DispatchError::Other("SlippageExceeded"));
    }

    Ok(current_amount)
  }
}

pub struct RouterFeeStub;
impl Get<Perbill> for RouterFeeStub {
  fn get() -> Perbill {
    primitives::ecosystem::params::AXIAL_ROUTER_FEE
  }
}

pub struct MaxPriceDeviationStub;
impl Get<Perbill> for MaxPriceDeviationStub {
  fn get() -> Perbill {
    primitives::ecosystem::params::MAX_PRICE_DEVIATION
  }
}

pub struct MaxRouterFeeStub;
impl Get<Perbill> for MaxRouterFeeStub {
  fn get() -> Perbill {
    primitives::ecosystem::params::MAX_AXIAL_ROUTER_FEE
  }
}

impl pallet_axial_router::Config for Test {
  type AdminOrigin = polkadot_sdk::frame_system::EnsureRoot<u64>;
  type Currency = Balances;
  type Assets = Assets;
  type TmcPallet = MockTmcPallet;
  type AssetConversion = MockAssetConversionAdapter;
  type PalletId = PalletIdStub;
  type NativeAsset = NativeAsset;
  type DefaultRouterFee = RouterFeeStub;
  type Precision = ConstU128<1_000_000_000_000>;
  type EmaHalfLife = ConstU32<3600>;
  type MaxPriceDeviation = MaxPriceDeviationStub;
  type MaxRouterFee = MaxRouterFeeStub;
  type MaxTrackedAssets = ConstU32<64>;
  type FeeAdapter = MockFeeAdapter;
  type BurningManagerAccount = ConstU64<123>;
  type ZapManagerAccount = ConstU64<888>;
  type PriceOracle = MockPriceOracle;
  type MinSwapForeign = ConstU128<{ primitives::ecosystem::params::MIN_SWAP_FOREIGN }>;
  type WeightInfo = ();
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = AxialRouterBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct AxialRouterBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl crate::types::BenchmarkHelper<primitives::AssetKind, u64, u128>
  for AxialRouterBenchmarkHelper
{
  fn create_asset(asset: primitives::AssetKind) -> polkadot_sdk::sp_runtime::DispatchResult {
    if let primitives::AssetKind::Local(id) | primitives::AssetKind::Foreign(id) = asset {
      let _ = Assets::force_create(frame_system::RawOrigin::Root.into(), id, 1, true, 1);
    }
    Ok(())
  }

  fn mint_asset(
    asset: primitives::AssetKind,
    to: &u64,
    amount: u128,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    use polkadot_sdk::frame_support::traits::Currency;
    use polkadot_sdk::frame_support::traits::fungibles::Mutate;
    match asset {
      primitives::AssetKind::Native => {
        let _ = Balances::deposit_creating(to, amount);
      }
      primitives::AssetKind::Local(id) | primitives::AssetKind::Foreign(id) => {
        Assets::mint_into(id, to, amount)?;
      }
    }
    Ok(())
  }

  fn create_pool(
    asset1: primitives::AssetKind,
    asset2: primitives::AssetKind,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    set_pool(asset1, asset2, 0, 0);
    Ok(())
  }

  fn add_liquidity(
    _who: &u64,
    asset1: primitives::AssetKind,
    asset2: primitives::AssetKind,
    amount1: u128,
    amount2: u128,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    let key = if asset1 < asset2 {
      (asset1, asset2)
    } else {
      (asset2, asset1)
    };

    let (r1, r2) = POOLS.with(|p| p.borrow().get(&key).cloned().unwrap_or((0, 0)));

    let (new_r1, new_r2) = if key.0 == asset1 {
      (r1 + amount1, r2 + amount2)
    } else {
      (r1 + amount2, r2 + amount1)
    };

    POOLS.with(|p| p.borrow_mut().insert(key, (new_r1, new_r2)));
    Ok(())
  }
}

pub fn new_test_ext() -> polkadot_sdk::sp_io::TestExternalities {
  let ext = frame_system::GenesisConfig::<Test>::default()
    .build_storage()
    .unwrap();
  let mut ext: polkadot_sdk::sp_io::TestExternalities = ext.into();

  // Reset thread locals
  POOLS.with(|p| p.borrow_mut().clear());
  TMC_RATES.with(|r| r.borrow_mut().clear());
  ORACLE_PRICES.with(|p| p.borrow_mut().clear());
  COLLECTED_FEES.with(|f| f.borrow_mut().clear());
  FORCE_FEE_FAILURE.with(|flag| *flag.borrow_mut() = false);

  ext.execute_with(|| {
    // Pre-fund accounts with Native Balance for deposits
    let accounts = vec![1, 2, 3, 123, 12345, 666];
    for acc in accounts {
      let _ = Balances::deposit_creating(&acc, 10_000 * primitives::ecosystem::params::PRECISION);
    }

    // Create test assets and mint initial balances
    for asset_id in 1..=10 {
      // Create asset (Account 1 is creator)
      // Requires Native Balance for deposit
      if Assets::create(RuntimeOrigin::signed(1), asset_id, 1, 1).is_err() {
        // If already exists, that's fine
      }

      // Mint tokens to test accounts (10,000 * PRECISION each)
      let initial_balance = 10_000 * primitives::ecosystem::params::PRECISION;
      let _ = Assets::mint_into(asset_id, &1, initial_balance); // User 1
      let _ = Assets::mint_into(asset_id, &2, initial_balance); // User 2
      let _ = Assets::mint_into(asset_id, &3, initial_balance); // User 3
      let _ = Assets::mint_into(asset_id, &12345, initial_balance); // Pool address
    }

    // Initialize pool for benchmarking (Local(1) <-> Native)
    let pool_reserve = 10_000 * primitives::ecosystem::params::PRECISION;
    set_pool(
      AssetKind::Local(1),
      AssetKind::Native,
      pool_reserve,
      pool_reserve,
    );
  });
  ext
}
