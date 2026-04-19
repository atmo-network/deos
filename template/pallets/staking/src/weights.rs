#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use core::marker::PhantomData;
use polkadot_sdk::frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};

pub trait WeightInfo {
	fn register_staking_asset() -> Weight;
	fn initialize_staked_asset() -> Weight;
	fn convert_position_to_receipt() -> Weight;
	fn sync_pool() -> Weight;
	fn stake() -> Weight;
	fn unstake() -> Weight;
	fn recover_unowned_pool() -> Weight;
	fn bind_native() -> Weight;
	fn clear_native_binding() -> Weight;
	fn set_operator_commission() -> Weight;
	fn bootstrap_reward_snapshot() -> Weight;
	fn claim_reward() -> Weight;
	fn claim_reward_batch(epochs: u32) -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: polkadot_sdk::frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn register_staking_asset() -> Weight {
		Weight::from_parts(20_000_000, 3000)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn initialize_staked_asset() -> Weight {
		Weight::from_parts(20_000_000, 3000)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn convert_position_to_receipt() -> Weight {
		Weight::from_parts(20_000_000, 3000)
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	fn sync_pool() -> Weight {
		Weight::from_parts(20_000_000, 3000)
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn stake() -> Weight {
		Weight::from_parts(45_000_000, 6000)
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	fn unstake() -> Weight {
		Weight::from_parts(45_000_000, 6000)
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	fn recover_unowned_pool() -> Weight {
		Weight::from_parts(35_000_000, 5000)
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	fn bind_native() -> Weight {
		Weight::from_parts(20_000_000, 4000)
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn clear_native_binding() -> Weight {
		Weight::from_parts(15_000_000, 2500)
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn set_operator_commission() -> Weight {
		Weight::from_parts(15_000_000, 2500)
			.saturating_add(T::DbWeight::get().writes(1))
	}
	fn bootstrap_reward_snapshot() -> Weight {
		Weight::from_parts(25_000_000, 4000)
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	fn claim_reward() -> Weight {
		Weight::from_parts(40_000_000, 6000)
			.saturating_add(T::DbWeight::get().reads(8))
			.saturating_add(T::DbWeight::get().writes(6))
	}
	fn claim_reward_batch(epochs: u32) -> Weight {
		Weight::from_parts(15_000_000, 3000)
			.saturating_add(Weight::from_parts(40_000_000, 6000).saturating_mul(epochs.into()))
			.saturating_add(T::DbWeight::get().reads((8_u64).saturating_mul(epochs.into())))
			.saturating_add(T::DbWeight::get().writes((6_u64).saturating_mul(epochs.into())))
	}
}

impl WeightInfo for () {
	fn register_staking_asset() -> Weight {
		Weight::from_parts(20_000_000, 3000)
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}
	fn initialize_staked_asset() -> Weight {
		Weight::from_parts(20_000_000, 3000)
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}
	fn convert_position_to_receipt() -> Weight {
		Weight::from_parts(20_000_000, 3000)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(3))
	}
	fn sync_pool() -> Weight {
		Weight::from_parts(20_000_000, 3000)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}
	fn stake() -> Weight {
		Weight::from_parts(45_000_000, 6000)
			.saturating_add(RocksDbWeight::get().reads(3))
			.saturating_add(RocksDbWeight::get().writes(2))
	}
	fn unstake() -> Weight {
		Weight::from_parts(45_000_000, 6000)
			.saturating_add(RocksDbWeight::get().reads(3))
			.saturating_add(RocksDbWeight::get().writes(2))
	}
	fn recover_unowned_pool() -> Weight {
		Weight::from_parts(35_000_000, 5000)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(2))
	}
	fn bind_native() -> Weight {
		Weight::from_parts(20_000_000, 4000)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}
	fn clear_native_binding() -> Weight {
		Weight::from_parts(15_000_000, 2500)
			.saturating_add(RocksDbWeight::get().writes(1))
	}
	fn set_operator_commission() -> Weight {
		Weight::from_parts(15_000_000, 2500)
			.saturating_add(RocksDbWeight::get().writes(1))
	}
	fn bootstrap_reward_snapshot() -> Weight {
		Weight::from_parts(25_000_000, 4000)
			.saturating_add(RocksDbWeight::get().reads(4))
			.saturating_add(RocksDbWeight::get().writes(3))
	}
	fn claim_reward() -> Weight {
		Weight::from_parts(40_000_000, 6000)
			.saturating_add(RocksDbWeight::get().reads(8))
			.saturating_add(RocksDbWeight::get().writes(6))
	}
	fn claim_reward_batch(epochs: u32) -> Weight {
		Weight::from_parts(15_000_000, 3000)
			.saturating_add(Weight::from_parts(40_000_000, 6000).saturating_mul(epochs.into()))
			.saturating_add(RocksDbWeight::get().reads((8_u64).saturating_mul(epochs.into())))
			.saturating_add(RocksDbWeight::get().writes((6_u64).saturating_mul(epochs.into())))
	}
}
