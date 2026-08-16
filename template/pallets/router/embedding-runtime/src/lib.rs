#![cfg_attr(not(feature = "std"), no_std)]

use pallet_deos_router::{
  AdapterFailure, AssetKind, ExactOutputExecution, FeeRoutingAdapter, PriceOracle, TmcInterface,
  types::AssetConversionApi,
};
#[cfg(feature = "runtime-benchmarks")]
use polkadot_sdk::sp_runtime::DispatchResult;
use polkadot_sdk::{
  frame_support::{
    PalletId, construct_runtime, derive_impl, parameter_types,
    traits::{ConstU32, ConstU128},
  },
  frame_system,
  sp_runtime::{
    DispatchError, Perbill,
    testing::H256,
    traits::{BlakeTwo256, IdentityLookup},
  },
};

pub type AccountId = u64;
pub type Balance = u128;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
  pub struct Runtime {
    System: frame_system,
    Balances: polkadot_sdk::pallet_balances,
    Assets: polkadot_sdk::pallet_assets,
    Router: pallet_deos_router,
  }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Runtime {
  type Block = Block;
  type AccountId = AccountId;
  type Lookup = IdentityLookup<Self::AccountId>;
  type Hash = H256;
  type Hashing = BlakeTwo256;
  type AccountData = polkadot_sdk::pallet_balances::AccountData<Balance>;
}

impl polkadot_sdk::pallet_balances::Config for Runtime {
  type MaxLocks = ();
  type MaxReserves = ();
  type ReserveIdentifier = [u8; 8];
  type Balance = Balance;
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

impl polkadot_sdk::pallet_assets::Config for Runtime {
  type RuntimeEvent = RuntimeEvent;
  type Balance = Balance;
  type AssetId = u32;
  type AssetIdParameter = u32;
  type Currency = Balances;
  type CreateOrigin = polkadot_sdk::frame_support::traits::AsEnsureOriginWithArg<
    frame_system::EnsureSigned<AccountId>,
  >;
  type ForceOrigin = frame_system::EnsureRoot<AccountId>;
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
  fn create_reserve_id_parameter(_id: u32) {}
}

pub struct HostTmc;
impl TmcInterface<AccountId, Balance> for HostTmc {
  fn has_curve(_asset: AssetKind) -> bool {
    false
  }
  fn supports_collateral(_token: AssetKind, _collateral: AssetKind) -> bool {
    false
  }
  fn calculate_recipient_receives(
    _token: AssetKind,
    _amount: Balance,
  ) -> Result<Balance, AdapterFailure> {
    Err(AdapterFailure::unknown(DispatchError::Other("NoCurve")))
  }
  fn mint_with_distribution(
    _who: &AccountId,
    _recipient: &AccountId,
    _token: AssetKind,
    _collateral: AssetKind,
    _amount: Balance,
  ) -> Result<Balance, AdapterFailure> {
    Err(AdapterFailure::unknown(DispatchError::Other("NoCurve")))
  }
}

pub struct HostXyk;
impl AssetConversionApi<AccountId, Balance> for HostXyk {
  fn single_pool_id(_a: AssetKind, _b: AssetKind) -> Option<(AssetKind, AssetKind)> {
    None
  }
  fn single_pool_reserves(_pool: (AssetKind, AssetKind)) -> Option<(Balance, Balance)> {
    None
  }
  fn quote_single_pool_exact_input(
    _a: AssetKind,
    _b: AssetKind,
    _amount: Balance,
    _fee: bool,
  ) -> Option<Balance> {
    None
  }
  fn quote_single_pool_exact_output(
    _a: AssetKind,
    _b: AssetKind,
    _amount: Balance,
    _fee: bool,
  ) -> Option<Balance> {
    None
  }
  fn execute_single_pool_exact_input(
    _who: AccountId,
    _a: AssetKind,
    _b: AssetKind,
    _amount: Balance,
    _minimum: Balance,
    _recipient: AccountId,
    _keep_alive: bool,
  ) -> Result<Balance, AdapterFailure> {
    Err(AdapterFailure::unknown(DispatchError::Other("NoPool")))
  }
  fn execute_single_pool_exact_output(
    _who: AccountId,
    _a: AssetKind,
    _b: AssetKind,
    _amount: Balance,
    _maximum: Balance,
    _recipient: AccountId,
    _keep_alive: bool,
  ) -> Result<ExactOutputExecution, AdapterFailure> {
    Err(AdapterFailure::unknown(DispatchError::Other("NoPool")))
  }
}

pub struct HostFees;
impl FeeRoutingAdapter<AccountId, Balance> for HostFees {
  fn route_fee(
    _who: &AccountId,
    _asset: AssetKind,
    _amount: Balance,
  ) -> Result<(), AdapterFailure> {
    Ok(())
  }
}

pub struct HostOracle;
impl PriceOracle<Balance> for HostOracle {
  fn update_ema_price(_a: AssetKind, _b: AssetKind, _price: Balance) -> Result<(), AdapterFailure> {
    Ok(())
  }
  fn get_ema_price(_a: AssetKind, _b: AssetKind) -> Option<Balance> {
    None
  }
  fn validate_price_deviation(
    _a: AssetKind,
    _b: AssetKind,
    _price: Balance,
  ) -> Result<(), AdapterFailure> {
    Ok(())
  }
}

parameter_types! {
  pub const RouterPalletId: PalletId = PalletId(*b"hostrout");
  pub const NativeAsset: AssetKind = AssetKind::Native;
  pub DefaultRouterFee: Perbill = Perbill::from_percent(1);
  pub MaxRouterFee: Perbill = Perbill::from_percent(5);
  pub MaxPriceDeviation: Perbill = Perbill::from_percent(20);
  pub const BurningAccount: AccountId = 90;
  pub const LiquidityAccount: AccountId = 91;
}

impl pallet_deos_router::Config for Runtime {
  type Currency = Balances;
  type Assets = Assets;
  type TmcPallet = HostTmc;
  type AssetConversion = HostXyk;
  type AdminOrigin = frame_system::EnsureRoot<AccountId>;
  type PalletId = RouterPalletId;
  type NativeAsset = NativeAsset;
  type DefaultRouterFee = DefaultRouterFee;
  type MaxLpPairs = ConstU32<8>;
  type MaxRouterFee = MaxRouterFee;
  type Precision = ConstU128<1_000_000_000_000>;
  type EmaHalfLife = ConstU32<100>;
  type MaxPriceDeviation = MaxPriceDeviation;
  type FeeAdapter = HostFees;
  type BurnActorAccount = BurningAccount;
  type LiquidityActorAccount = LiquidityAccount;
  type PriceOracle = HostOracle;
  type MinSwapForeign = ConstU128<1>;
  type WeightInfo = ();
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = HostBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct HostBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_deos_router::types::BenchmarkHelper<AssetKind, AccountId, Balance>
  for HostBenchmarkHelper
{
  fn create_asset(_asset: AssetKind) -> DispatchResult {
    Ok(())
  }
  fn mint_asset(_asset: AssetKind, _to: &AccountId, _amount: Balance) -> DispatchResult {
    Ok(())
  }
  fn create_pool(_a: AssetKind, _b: AssetKind) -> DispatchResult {
    Ok(())
  }
  fn create_tmc_curve(_token: AssetKind, _collateral: AssetKind) -> DispatchResult {
    Ok(())
  }
  fn add_liquidity(
    _who: &AccountId,
    _a: AssetKind,
    _b: AssetKind,
    _x: Balance,
    _y: Balance,
  ) -> DispatchResult {
    Ok(())
  }
}
