extern crate alloc;

use crate as pallet_tmc;
use polkadot_sdk::frame_support::{
  PalletId, construct_runtime, derive_impl,
  traits::{ConstU32, ConstU64, ConstU128, Get},
};
use polkadot_sdk::frame_system::{self, EnsureRoot};
use polkadot_sdk::sp_runtime::{
  BuildStorage, Perbill,
  testing::H256,
  traits::{BlakeTwo256, IdentityLookup},
};

type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
  pub struct Test {
    System: frame_system,
    Balances: polkadot_sdk::pallet_balances,
    Assets: polkadot_sdk::pallet_assets,
    TokenMintingCurve: pallet_tmc,
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

pub struct TmcPalletId;
impl Get<PalletId> for TmcPalletId {
  fn get() -> PalletId {
    PalletId(*b"tmcmtst0")
  }
}

pub struct UserAllocationRatio;
impl Get<Perbill> for UserAllocationRatio {
  fn get() -> Perbill {
    Perbill::from_parts(333_333_333)
  }
}

/// Default resolver: routes all TMC output to account 888
pub struct DefaultMintOutput;
impl crate::MintOutputResolver<u64> for DefaultMintOutput {
  fn output_account(_minted_asset: primitives::AssetKind) -> u64 {
    888
  }
}

impl pallet_tmc::Config for Test {
  // type RuntimeEvent = RuntimeEvent;
  type Assets = Assets;
  type Currency = Balances;
  type Balance = u128;
  type PalletId = TmcPalletId;
  type TreasuryAccount = ConstU64<999>;
  type InitialPrice = ConstU128<{ primitives::ecosystem::params::PRECISION }>;
  type SlopeParameter = ConstU128<{ primitives::ecosystem::params::TMC_SLOPE_PARAMETER }>;
  type Precision = ConstU128<{ primitives::ecosystem::params::PRECISION }>;
  type MintOutputResolver = DefaultMintOutput;
  type UserAllocationRatio = UserAllocationRatio;
  type DomainGlueHook = ();
  type AdminOrigin = EnsureRoot<u64>;
  type WeightInfo = ();
}

pub fn new_test_ext() -> polkadot_sdk::sp_io::TestExternalities {
  let mut t = frame_system::GenesisConfig::<Test>::default()
    .build_storage()
    .unwrap();

  polkadot_sdk::pallet_assets::GenesisConfig::<Test> {
    assets: alloc::vec![(1, 1, true, 1), (2, 1, true, 1)],
    metadata: alloc::vec![],
    accounts: alloc::vec![],
    reserves: alloc::vec![],
    next_asset_id: None,
  }
  .assimilate_storage(&mut t)
  .unwrap();

  pallet_tmc::GenesisConfig::<Test>::default()
    .assimilate_storage(&mut t)
    .unwrap();

  t.into()
}
