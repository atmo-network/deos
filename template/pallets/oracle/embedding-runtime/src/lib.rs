#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use pallet_oracle::{Aggregation, ZeroPolicy};
use polkadot_sdk::{
  frame_support::{
    construct_runtime,
    traits::{ConstU8, ConstU32},
  },
  frame_system::{EnsureRoot, EnsureSigned},
  sp_runtime::{
    generic,
    traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Lazy, Verify},
  },
};
use scale_info::TypeInfo;

pub type AccountId = u64;
pub type BlockNumber = u64;
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
pub type UncheckedExtrinsic =
  generic::UncheckedExtrinsic<AccountId, RuntimeCall, FixtureSignature, ()>;
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct FixtureSigner(AccountId);

impl IdentifyAccount for FixtureSigner {
  type AccountId = AccountId;

  fn into_account(self) -> Self::AccountId {
    self.0
  }
}

#[derive(Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo)]
pub struct FixtureSignature {
  signer: AccountId,
  payload: Vec<u8>,
}

impl Verify for FixtureSignature {
  type Signer = FixtureSigner;

  fn verify<L: Lazy<[u8]>>(&self, mut message: L, signer: &AccountId) -> bool {
    self.signer == *signer && message.get() == self.payload
  }
}

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  MaxEncodedLen,
  Ord,
  PartialEq,
  PartialOrd,
  TypeInfo,
)]
pub enum FeedId {
  Temperature,
  Pressure,
  Benchmark(u32),
}

impl From<u32> for FeedId {
  fn from(value: u32) -> Self {
    Self::Benchmark(value)
  }
}

#[derive(
  Clone,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Default,
  Encode,
  Eq,
  MaxEncodedLen,
  PartialEq,
  TypeInfo,
)]
pub enum Meaning {
  #[default]
  Pascals,
  Celsius {
    decimals: u8,
  },
}

#[derive(
  Clone,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Default,
  Encode,
  Eq,
  MaxEncodedLen,
  PartialEq,
  TypeInfo,
)]
pub enum Provenance {
  #[default]
  LocalSensor,
  RuntimeCalculation,
}

construct_runtime!(
  pub enum Runtime {
    System: polkadot_sdk::frame_system,
    Oracle: pallet_oracle,
  }
);

impl polkadot_sdk::frame_system::Config for Runtime {
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

impl pallet_oracle::Config for Runtime {
  type FeedId = FeedId;
  type ProducerId = AccountId;
  type Meaning = Meaning;
  type Provenance = Provenance;
  type RegisterOrigin = EnsureRoot<AccountId>;
  type PublishOrigin = EnsureSigned<AccountId>;
  type OnObservationChanged = ();
  type MaxFeeds = ConstU32<16>;
  type MaxFeedsPerProducer = ConstU32<4>;
  type MaxScale = ConstU8<18>;
  type WeightInfo = ();
}

pub fn register_fixture_feed() -> polkadot_sdk::sp_runtime::DispatchResult {
  Oracle::register_feed(
    RuntimeOrigin::root(),
    FeedId::Temperature,
    1,
    Meaning::Celsius { decimals: 2 },
    Provenance::LocalSensor,
    2,
    Aggregation::Ema {
      half_life_blocks: 10,
    },
    ZeroPolicy::Allow,
    false,
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use polkadot_sdk::{frame_support::assert_ok, sp_runtime::BuildStorage};

  #[test]
  fn independent_runtime_registers_and_publishes_typed_feed() {
    let storage = polkadot_sdk::frame_system::GenesisConfig::<Runtime>::default()
      .build_storage()
      .expect("fixture storage builds");
    let mut ext = polkadot_sdk::sp_io::TestExternalities::new(storage);
    ext.execute_with(|| {
      System::set_block_number(1);
      assert_ok!(register_fixture_feed());
      assert_ok!(Oracle::publish(
        RuntimeOrigin::signed(1),
        FeedId::Temperature,
        2_150,
      ));
      let observation = Oracle::observations(FeedId::Temperature).expect("observation exists");
      assert_eq!((observation.value, observation.revision), (2_150, 1));
    });
  }
}
