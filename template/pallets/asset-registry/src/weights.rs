#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use polkadot_sdk::frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

pub trait WeightInfo {
	fn register_foreign_asset() -> Weight;
	fn register_foreign_asset_with_id() -> Weight;
	fn link_existing_asset() -> Weight;
	fn migrate_location_key() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: polkadot_sdk::frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn register_foreign_asset() -> Weight {
		Weight::from_parts(54_757_000, 4087)
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(4))
	}
	fn register_foreign_asset_with_id() -> Weight {
		Weight::from_parts(51_404_000, 4087)
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(4))
	}
	fn link_existing_asset() -> Weight {
		Weight::from_parts(33_873_000, 4087)
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	fn migrate_location_key() -> Weight {
		Weight::from_parts(25_632_000, 7184)
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(3))
	}
}

impl WeightInfo for () {
	fn register_foreign_asset() -> Weight {
		Weight::from_parts(50_000_000, 3000)
	}
	fn register_foreign_asset_with_id() -> Weight {
		Weight::from_parts(50_000_000, 3000)
	}
	fn link_existing_asset() -> Weight {
		Weight::from_parts(25_000_000, 4500)
	}
	fn migrate_location_key() -> Weight {
		Weight::from_parts(20_000_000, 2000)
	}
}
