use crate as pallet_asset_registry;
use polkadot_sdk::frame_support::{
  construct_runtime,
  traits::{ConstU32, ConstU64, ConstU128},
};
use polkadot_sdk::frame_system::{EnsureRoot, EnsureSigned};
use polkadot_sdk::sp_runtime::{
  BuildStorage,
  traits::{BlakeTwo256, IdentityLookup},
};

type Block = polkadot_sdk::frame_system::mocking::MockBlock<Test>;
type Balance = u128;
type AccountId = u64;

construct_runtime!(
  pub enum Test {
    System: polkadot_sdk::frame_system,
    Balances: polkadot_sdk::pallet_balances,
    Assets: polkadot_sdk::pallet_assets,
    AssetRegistry: pallet_asset_registry,
  }
);

impl polkadot_sdk::frame_system::Config for Test {
  type BaseCallFilter = polkadot_sdk::frame_support::traits::Everything;
  type BlockWeights = ();
  type BlockLength = ();
  type DbWeight = ();
  type RuntimeOrigin = RuntimeOrigin;
  type RuntimeCall = RuntimeCall;
  type Nonce = u64;
  type Hash = polkadot_sdk::sp_core::H256;
  type Hashing = BlakeTwo256;
  type AccountId = AccountId;
  type Lookup = IdentityLookup<Self::AccountId>;
  type Block = Block;
  type RuntimeEvent = RuntimeEvent;
  type BlockHashCount = ConstU64<250>;
  type Version = ();
  type PalletInfo = PalletInfo;
  type AccountData = polkadot_sdk::pallet_balances::AccountData<Balance>;
  type OnNewAccount = ();
  type OnKilledAccount = ();
  type SystemWeightInfo = ();
  type SS58Prefix = ();
  type OnSetCode = ();
  type MaxConsumers = ConstU32<16>;
  type RuntimeTask = ();
  type ExtensionsWeightInfo = ();
  type SingleBlockMigrations = ();
  type MultiBlockMigrator = ();
  type PreInherents = ();
  type PostInherents = ();
  type PostTransactions = ();
}

impl polkadot_sdk::pallet_balances::Config for Test {
  type MaxLocks = ConstU32<50>;
  type MaxReserves = ();
  type ReserveIdentifier = [u8; 8];
  type Balance = Balance;
  type RuntimeEvent = RuntimeEvent;
  type DustRemoval = ();
  type ExistentialDeposit = ConstU128<1>;
  type AccountStore = System;
  type WeightInfo = ();
  type FreezeIdentifier = ();
  type MaxFreezes = ();
  type RuntimeHoldReason = RuntimeHoldReason;
  type RuntimeFreezeReason = RuntimeFreezeReason;
  type DoneSlashHandler = ();
}

impl polkadot_sdk::pallet_assets::Config for Test {
  type RuntimeEvent = RuntimeEvent;
  type Balance = Balance;
  type AssetId = u32;
  type AssetIdParameter = u32;
  type Currency = Balances;
  type CreateOrigin = EnsureSigned<AccountId>;
  type ForceOrigin = EnsureRoot<AccountId>;
  type AssetDeposit = ConstU128<1>;
  type AssetAccountDeposit = ConstU128<1>;
  type MetadataDepositBase = ConstU128<1>;
  type MetadataDepositPerByte = ConstU128<1>;
  type ApprovalDeposit = ConstU128<1>;
  type StringLimit = ConstU32<50>;
  type Freezer = ();
  type Extra = ();
  type ReserveData = ();
  type WeightInfo = ();
  type RemoveItemsLimit = ConstU32<1000>;
  type CallbackHandle = ();
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = ();
  type Holder = ();
}

// Mock AssetIdGenerator
pub struct MockLocationToAssetId;
impl polkadot_sdk::sp_runtime::traits::Convert<polkadot_sdk::staging_xcm::latest::Location, u32>
  for MockLocationToAssetId
{
  fn convert(location: polkadot_sdk::staging_xcm::latest::Location) -> u32 {
    // Simple mock:
    // Parents: 1, Interior: X1(Parachain(id)) -> id
    use polkadot_sdk::staging_xcm::latest::{Junction::Parachain, Junctions};

    match (location.parents, location.interior) {
      (1, Junctions::X1(junctions)) => {
        if let Parachain(id) = junctions[0] {
          id
        } else {
          0
        }
      }
      _ => 0,
    }
  }
}

// Mock Asset Owner
pub struct MockAssetOwner;
impl polkadot_sdk::frame_support::traits::Get<AccountId> for MockAssetOwner {
  fn get() -> AccountId {
    1 // Account 1 is the registry owner
  }
}

impl pallet_asset_registry::Config for Test {
  type RegistryOrigin = EnsureRoot<AccountId>; // Only root can register in tests
  type AssetIdGenerator = MockLocationToAssetId;
  type AssetOwner = MockAssetOwner;
  type TokenDomainHook = ();
  type WeightInfo = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> polkadot_sdk::sp_io::TestExternalities {
  let mut t = polkadot_sdk::frame_system::GenesisConfig::<Test>::default()
    .build_storage()
    .unwrap();

  polkadot_sdk::pallet_balances::GenesisConfig::<Test> {
    balances: vec![(1, 1000), (2, 1000)],
    dev_accounts: None,
  }
  .assimilate_storage(&mut t)
  .unwrap();

  t.into()
}
