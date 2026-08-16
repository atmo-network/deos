#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use core::marker::PhantomData;
use polkadot_sdk::frame_support::weights::{constants::RocksDbWeight, Weight};

pub trait WeightInfo {
	fn register_staking_asset() -> Weight { default_weight(1, 1) }
	fn sync_pool() -> Weight { default_weight(2, 1) }
	fn stake() -> Weight { default_weight(3, 2) }
	fn unstake() -> Weight { default_weight(3, 2) }
	fn recover_unowned_pool() -> Weight { default_weight(2, 2) }
	fn fund_native_security_reward() -> Weight { default_weight(8, 5) }
	fn claim_native_security_reward() -> Weight { default_weight(9, 6) }
	fn claim_native_security_reward_batch(epochs: u32) -> Weight {
		Weight::from_parts(79_094_587, 6_196)
			.saturating_add(Weight::from_parts(19_396_231, 15_319).saturating_mul(epochs.into()))
			.saturating_add(RocksDbWeight::get().reads(4_u64.saturating_add(3_u64.saturating_mul(epochs.into()))))
			.saturating_add(RocksDbWeight::get().writes(3_u64.saturating_add(2_u64.saturating_mul(epochs.into()))))
	}
	fn claim_and_compound_native_security_reward() -> Weight {
		Weight::from_parts(464_522_000, 19_253)
			.saturating_add(RocksDbWeight::get().reads(26))
			.saturating_add(RocksDbWeight::get().writes(22))
	}
	fn expire_native_security_reward() -> Weight {
		Weight::from_parts(232_645_000, 255_290)
			.saturating_add(RocksDbWeight::get().reads(105))
			.saturating_add(RocksDbWeight::get().writes(105))
	}
	fn settle_due_native_security_reward(retained_epochs: u32) -> Weight {
		Weight::from_parts(225_363_075, 254_580)
			.saturating_add(Weight::from_parts(2_721_806, 2_551).saturating_mul(retained_epochs.into()))
			.saturating_add(RocksDbWeight::get().reads(105_u64.saturating_add(retained_epochs.into())))
			.saturating_add(RocksDbWeight::get().writes(105))
	}
	fn cancel_native_security_epoch_plan() -> Weight {
		Weight::from_parts(11_803_000, 3_534)
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(2))
	}
	fn contract_native_security_obligations() -> Weight {
		Weight::from_parts(27_099_000, 14_309)
			.saturating_add(RocksDbWeight::get().reads(4))
			.saturating_add(RocksDbWeight::get().writes(4))
	}
	fn lock_native_lp_for_collator() -> Weight {
		Weight::from_parts(117_824_000, 6_208)
			.saturating_add(RocksDbWeight::get().reads(12))
			.saturating_add(RocksDbWeight::get().writes(7))
	}
	fn request_unlock_native_lp() -> Weight {
		Weight::from_parts(66_629_000, 4_687)
			.saturating_add(RocksDbWeight::get().reads(8))
			.saturating_add(RocksDbWeight::get().writes(5))
	}
	fn withdraw_unlocked_native_lp() -> Weight { default_weight(5, 5) }
	fn redelegate_native_lp() -> Weight { default_weight(5, 4) }
	fn lock_native_lp_for_governance() -> Weight { default_weight(8, 6) }
	fn request_unlock_native_lp_for_governance() -> Weight { default_weight(5, 4) }
	fn withdraw_unlocked_native_lp_for_governance() -> Weight { default_weight(5, 5) }
	fn lock_native_asset_for_governance() -> Weight { default_weight(6, 5) }
	fn request_unlock_native_asset_for_governance() -> Weight { default_weight(4, 3) }
	fn withdraw_unlocked_native_asset_for_governance() -> Weight { default_weight(5, 5) }
	fn open_native_security_epoch(participants: u32, retained_epochs: u32) -> Weight {
		Weight::from_parts(91_186_330, 11_061)
			.saturating_add(Weight::from_parts(51_406_814, 2_854).saturating_mul(participants.into()))
			.saturating_add(Weight::from_parts(4_054_386, 2_597).saturating_mul(retained_epochs.into()))
			.saturating_add(RocksDbWeight::get().reads(13))
			.saturating_add(RocksDbWeight::get().reads((4_u64).saturating_mul(participants.into())))
			.saturating_add(RocksDbWeight::get().reads(retained_epochs.into()))
			.saturating_add(RocksDbWeight::get().writes(2))
	}
}

fn default_weight(reads: u64, writes: u64) -> Weight {
	Weight::from_parts(45_000_000, 6000)
		.saturating_add(RocksDbWeight::get().reads(reads))
		.saturating_add(RocksDbWeight::get().writes(writes))
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: polkadot_sdk::frame_system::Config> WeightInfo for SubstrateWeight<T> {}
impl WeightInfo for () {}
