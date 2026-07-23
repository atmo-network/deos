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
use std::cell::RefCell;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForcedMintFailure {
  User,
  Sink,
}

thread_local! {
  pub static FORCED_MINT_FAILURE: RefCell<Option<ForcedMintFailure>> = const { RefCell::new(None) };
}

pub fn set_forced_mint_failure(mode: Option<ForcedMintFailure>) {
  FORCED_MINT_FAILURE.with(|state| *state.borrow_mut() = mode);
}

/// Default resolver: routes all TMC output to account 888
pub struct DefaultMintOutput;
impl crate::MintOutputResolver<u64> for DefaultMintOutput {
  fn output_accounts(_minted_asset: primitives::AssetKind) -> crate::MintOutputAccounts<u64> {
    crate::MintOutputAccounts {
      collateral: 888,
      minted: 888,
    }
  }
}

pub struct MockMintDistributionHook;
impl crate::MintDistributionHook<u64> for MockMintDistributionHook {
  fn before_user_mint(
    _minted_asset: primitives::AssetKind,
    _account: &u64,
    _amount: u128,
  ) -> Result<(), polkadot_sdk::sp_runtime::DispatchError> {
    let should_fail =
      FORCED_MINT_FAILURE.with(|state| *state.borrow() == Some(ForcedMintFailure::User));
    if should_fail {
      return Err(polkadot_sdk::sp_runtime::DispatchError::Other(
        "Forced user mint failure",
      ));
    }
    Ok(())
  }

  fn before_sink_mint(
    _minted_asset: primitives::AssetKind,
    _account: &u64,
    _amount: u128,
  ) -> Result<(), polkadot_sdk::sp_runtime::DispatchError> {
    let should_fail =
      FORCED_MINT_FAILURE.with(|state| *state.borrow() == Some(ForcedMintFailure::Sink));
    if should_fail {
      return Err(polkadot_sdk::sp_runtime::DispatchError::Other(
        "Forced sink mint failure",
      ));
    }
    Ok(())
  }
}

#[cfg(feature = "runtime-benchmarks")]
pub struct TmcBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl crate::BenchmarkHelper<u64> for TmcBenchmarkHelper {
  fn create_asset(asset_id: u32) -> polkadot_sdk::sp_runtime::DispatchResult {
    let _ = Assets::force_create(frame_system::RawOrigin::Root.into(), asset_id, 1, true, 1);
    Ok(())
  }

  fn mint_native(to: &u64, amount: u128) -> polkadot_sdk::sp_runtime::DispatchResult {
    use polkadot_sdk::frame_support::traits::fungible::Mutate;
    let _ = Balances::mint_into(to, amount);
    Ok(())
  }

  fn mint_local(asset_id: u32, to: &u64, amount: u128) -> polkadot_sdk::sp_runtime::DispatchResult {
    use polkadot_sdk::frame_support::traits::fungibles::Mutate;
    Assets::mint_into(asset_id, to, amount)?;
    Ok(())
  }
}

impl pallet_tmc::Config for Test {
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
  type MintDistributionHook = MockMintDistributionHook;
  type AdminOrigin = EnsureRoot<u64>;
  type WeightInfo = ();
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = TmcBenchmarkHelper;
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

  FORCED_MINT_FAILURE.with(|state| *state.borrow_mut() = None);

  t.into()
}
