use crate as pallet_oracle;
use core::cell::RefCell;
use polkadot_sdk::{
  frame_support::{
    construct_runtime,
    traits::{ConstU8, ConstU32},
  },
  frame_system::{EnsureRoot, EnsureSigned},
  sp_runtime::{
    BuildStorage,
    traits::{BlakeTwo256, IdentityLookup},
  },
};

type Block = polkadot_sdk::frame_system::mocking::MockBlock<Test>;
pub type AccountId = u64;

std::thread_local! {
  static HOOK_FAILURE: RefCell<bool> = const { RefCell::new(false) };
  static HOOK_CALLS: RefCell<Vec<(u32, u64, Option<u128>, u128)>> = const { RefCell::new(Vec::new()) };
  static HOOK_PROVENANCE: RefCell<Vec<crate::ObservationCauseProvenance>> = const { RefCell::new(Vec::new()) };
}

pub struct TestObservationHook;
impl crate::OnObservationChanged<u32> for TestObservationHook {
  fn on_observation_changed(
    feed: u32,
    revision: crate::Revision,
    previous: Option<crate::OracleValue>,
    current: crate::OracleValue,
    cause_provenance: crate::ObservationCauseProvenance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    if HOOK_FAILURE.with(|value| *value.borrow()) {
      return Err(polkadot_sdk::sp_runtime::DispatchError::Other(
        "ObservationHookRejected",
      ));
    }
    HOOK_CALLS.with(|calls| calls.borrow_mut().push((feed, revision, previous, current)));
    HOOK_PROVENANCE.with(|causes| causes.borrow_mut().push(cause_provenance));
    Ok(())
  }
}

pub fn take_hook_provenance() -> Vec<crate::ObservationCauseProvenance> {
  HOOK_PROVENANCE.with(|causes| core::mem::take(&mut *causes.borrow_mut()))
}

pub fn set_hook_failure(fail: bool) {
  HOOK_FAILURE.with(|value| *value.borrow_mut() = fail);
}

pub fn hook_calls() -> Vec<(u32, u64, Option<u128>, u128)> {
  HOOK_CALLS.with(|calls| calls.borrow().clone())
}

construct_runtime!(
  pub enum Test {
    System: polkadot_sdk::frame_system,
    Oracle: pallet_oracle,
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
  type BlockHashCount = polkadot_sdk::frame_support::traits::ConstU64<250>;
  type Version = ();
  type PalletInfo = PalletInfo;
  type AccountData = ();
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

impl crate::Config for Test {
  type FeedId = u32;
  type ProducerId = AccountId;
  type Meaning = u32;
  type Provenance = u8;
  type RegisterOrigin = EnsureRoot<AccountId>;
  type PublishOrigin = EnsureSigned<AccountId>;
  type OnObservationChanged = TestObservationHook;
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = ();
  type MaxFeeds = ConstU32<3>;
  type MaxFeedsPerProducer = ConstU32<2>;
  type MaxScale = ConstU8<18>;
  type WeightInfo = ();
}

pub fn new_test_ext() -> polkadot_sdk::sp_io::TestExternalities {
  let storage = polkadot_sdk::frame_system::GenesisConfig::<Test>::default()
    .build_storage()
    .expect("test storage builds");
  let mut ext = polkadot_sdk::sp_io::TestExternalities::new(storage);
  HOOK_FAILURE.with(|value| *value.borrow_mut() = false);
  HOOK_CALLS.with(|calls| calls.borrow_mut().clear());
  HOOK_PROVENANCE.with(|causes| causes.borrow_mut().clear());
  ext.execute_with(|| System::set_block_number(1));
  ext
}
