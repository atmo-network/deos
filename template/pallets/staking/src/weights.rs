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
		default_weight(2_u64.saturating_add(7_u64.saturating_mul(epochs.into())), 2_u64.saturating_add(4_u64.saturating_mul(epochs.into())))
	}
	fn claim_and_compound_native_security_reward() -> Weight { default_weight(30, 20) }
	fn expire_native_security_reward() -> Weight { default_weight(7, 5) }
	fn cleanup_expired_native_security_reward() -> Weight { default_weight(3, 3) }
	fn lock_native_lp_for_collator() -> Weight { default_weight(13, 9) }
	fn request_unlock_native_lp() -> Weight { default_weight(9, 7) }
	fn withdraw_unlocked_native_lp() -> Weight { default_weight(5, 5) }
	fn redelegate_native_lp() -> Weight { default_weight(5, 4) }
	fn lock_native_lp_for_governance() -> Weight { default_weight(8, 6) }
	fn request_unlock_native_lp_for_governance() -> Weight { default_weight(5, 4) }
	fn withdraw_unlocked_native_lp_for_governance() -> Weight { default_weight(5, 5) }
	fn lock_native_asset_for_governance() -> Weight { default_weight(6, 5) }
	fn request_unlock_native_asset_for_governance() -> Weight { default_weight(4, 3) }
	fn withdraw_unlocked_native_asset_for_governance() -> Weight { default_weight(5, 5) }
	fn open_native_security_epoch(participants: u32) -> Weight {
		Weight::from_parts(28_134_919, 10_871)
			.saturating_add(Weight::from_parts(51_351_969, 2_850).saturating_mul(participants.into()))
			.saturating_add(RocksDbWeight::get().reads(10))
			.saturating_add(RocksDbWeight::get().reads((4_u64).saturating_mul(participants.into())))
			.saturating_add(RocksDbWeight::get().writes(1))
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
