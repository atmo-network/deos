#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use core::marker::PhantomData;
use polkadot_sdk::frame_support::{
  traits::Get,
  weights::{constants::RocksDbWeight, Weight},
};

pub trait WeightInfo {
  fn create_user_aaa() -> Weight;
  fn create_user_aaa_at_slot() -> Weight;
  fn create_system_aaa() -> Weight;
  fn reopen_system_aaa() -> Weight;
  fn pause_aaa() -> Weight;
  fn resume_aaa() -> Weight;
  fn manual_trigger() -> Weight;
  fn fund_aaa() -> Weight;
  fn close_aaa() -> Weight;
  fn update_schedule() -> Weight;
  fn update_execution_plan() -> Weight;
  fn update_on_close_execution_plan() -> Weight;
  fn set_global_circuit_breaker() -> Weight;
  fn set_active_actor_limit() -> Weight;
  fn permissionless_sweep() -> Weight;
  fn permissionless_sweep_many(batch: u32) -> Weight;
}

pub trait TaskWeightInfo {
  fn simple_asset_op() -> Weight;
  fn split_transfer(legs: u32) -> Weight;
  fn dex_swap() -> Weight;
  fn dex_liquidity() -> Weight;
  fn noop() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: polkadot_sdk::frame_system::Config + crate::Config> WeightInfo for SubstrateWeight<T> {
  fn create_user_aaa() -> Weight {
    let slot_scan_reads = u64::from(T::MaxOwnerSlots::get());
    Weight::from_parts(25_000_000, 2000)
      .saturating_add(T::DbWeight::get().reads(slot_scan_reads.saturating_add(3)))
      .saturating_add(T::DbWeight::get().writes(5))
  }

  fn create_user_aaa_at_slot() -> Weight {
    Self::create_user_aaa()
  }

  fn create_system_aaa() -> Weight {
    Weight::from_parts(25_000_000, 2000)
      .saturating_add(T::DbWeight::get().reads(3))
      .saturating_add(T::DbWeight::get().writes(4))
  }

  fn reopen_system_aaa() -> Weight {
    Weight::from_parts(100_642_000, 174_945)
      .saturating_add(T::DbWeight::get().reads(20))
      .saturating_add(T::DbWeight::get().writes(4))
  }

  fn pause_aaa() -> Weight {
    Weight::from_parts(15_000_000, 1200)
      .saturating_add(T::DbWeight::get().reads(1))
      .saturating_add(T::DbWeight::get().writes(2))
  }

  fn resume_aaa() -> Weight {
    Weight::from_parts(15_000_000, 1200)
      .saturating_add(T::DbWeight::get().reads(1))
      .saturating_add(T::DbWeight::get().writes(2))
  }

  fn manual_trigger() -> Weight {
    Weight::from_parts(12_000_000, 1200)
      .saturating_add(T::DbWeight::get().reads(1))
      .saturating_add(T::DbWeight::get().writes(2))
  }

  fn fund_aaa() -> Weight {
    Weight::from_parts(20_000_000, 1800)
      .saturating_add(T::DbWeight::get().reads(2))
      .saturating_add(T::DbWeight::get().writes(2))
  }

  fn close_aaa() -> Weight {
    Weight::from_parts(30_000_000, 2200)
      .saturating_add(T::DbWeight::get().reads(4))
      .saturating_add(T::DbWeight::get().writes(5))
  }

  fn update_schedule() -> Weight {
    Weight::from_parts(12_000_000, 900)
      .saturating_add(T::DbWeight::get().reads(1))
      .saturating_add(T::DbWeight::get().writes(1))
  }

  fn update_execution_plan() -> Weight {
    Weight::from_parts(18_000_000, 1200)
      .saturating_add(T::DbWeight::get().reads(1))
      .saturating_add(T::DbWeight::get().writes(1))
  }

  fn update_on_close_execution_plan() -> Weight {
    Self::update_execution_plan()
  }

  fn set_global_circuit_breaker() -> Weight {
    Weight::from_parts(8_000_000, 600)
      .saturating_add(T::DbWeight::get().writes(1))
  }

  fn set_active_actor_limit() -> Weight {
    Weight::from_parts(10_000_000, 800)
      .saturating_add(T::DbWeight::get().reads(1))
      .saturating_add(T::DbWeight::get().writes(1))
  }

  fn permissionless_sweep() -> Weight {
    Weight::from_parts(18_000_000, 1200)
      .saturating_add(T::DbWeight::get().reads(2))
      .saturating_add(T::DbWeight::get().writes(1))
  }

  fn permissionless_sweep_many(batch: u32) -> Weight {
    let bounded = u64::from(batch.min(T::MaxSweepPerBlock::get()));
    Weight::from_parts(
      12_000_000u64.saturating_add(18_000_000u64.saturating_mul(bounded)),
      1200u64.saturating_add(384u64.saturating_mul(bounded)),
    )
    .saturating_add(T::DbWeight::get().reads(1u64.saturating_add(bounded)))
    .saturating_add(T::DbWeight::get().writes(bounded.saturating_mul(5)))
  }
}

pub struct SubstrateTaskWeightInfo<T>(PhantomData<T>);
impl<T: polkadot_sdk::frame_system::Config + crate::Config> TaskWeightInfo
  for SubstrateTaskWeightInfo<T>
{
  fn simple_asset_op() -> Weight {
    Weight::from_parts(10_000_000, 1000)
      .saturating_add(T::DbWeight::get().reads_writes(2, 2))
  }

  fn split_transfer(legs: u32) -> Weight {
    let bounded_legs = u64::from(legs.min(T::MaxSplitTransferLegs::get()));
    Weight::from_parts(
      15_000_000u64.saturating_add(bounded_legs.saturating_mul(2_000_000)),
      1400u64.saturating_add(bounded_legs.saturating_mul(96)),
    )
    .saturating_add(T::DbWeight::get().reads_writes(2, bounded_legs.saturating_add(1)))
  }

  fn dex_swap() -> Weight {
    Weight::from_parts(260_000_000, 3200)
      .saturating_add(T::DbWeight::get().reads_writes(12, 8))
  }

  fn dex_liquidity() -> Weight {
    let max_scan = u64::from(T::MaxAdapterScan::get());
    Weight::from_parts(
      220_000_000u64.saturating_add(max_scan.saturating_mul(3_000_000)),
      3600u64.saturating_add(max_scan.saturating_mul(96)),
    )
    .saturating_add(T::DbWeight::get().reads_writes(
      12u64.saturating_add(max_scan),
      10,
    ))
  }

  fn noop() -> Weight {
    Weight::from_parts(1_000_000, 0)
  }
}

impl WeightInfo for () {
  fn create_user_aaa() -> Weight { Weight::from_parts(25_000_000, 2000) }
  fn create_user_aaa_at_slot() -> Weight { Self::create_user_aaa() }
  fn create_system_aaa() -> Weight { Weight::from_parts(25_000_000, 2000) }
  fn reopen_system_aaa() -> Weight { Weight::from_parts(100_642_000, 174_945) }
  fn pause_aaa() -> Weight { Weight::from_parts(15_000_000, 1200) }
  fn resume_aaa() -> Weight { Weight::from_parts(15_000_000, 1200) }
  fn manual_trigger() -> Weight { Weight::from_parts(12_000_000, 1200) }
  fn fund_aaa() -> Weight { Weight::from_parts(20_000_000, 1800) }
  fn close_aaa() -> Weight { Weight::from_parts(30_000_000, 2200) }
  fn update_schedule() -> Weight { Weight::from_parts(12_000_000, 900) }
  fn update_execution_plan() -> Weight { Weight::from_parts(18_000_000, 1200) }
  fn update_on_close_execution_plan() -> Weight { Weight::from_parts(18_000_000, 1200) }
  fn set_global_circuit_breaker() -> Weight { Weight::from_parts(8_000_000, 600) }
  fn set_active_actor_limit() -> Weight { Weight::from_parts(10_000_000, 800) }
  fn permissionless_sweep() -> Weight { Weight::from_parts(18_000_000, 1200) }
  fn permissionless_sweep_many(batch: u32) -> Weight {
    let bounded = u64::from(batch.min(16));
    Weight::from_parts(
      12_000_000u64.saturating_add(18_000_000u64.saturating_mul(bounded)),
      1200,
    )
  }
}

impl TaskWeightInfo for () {
  fn simple_asset_op() -> Weight { Weight::from_parts(10_000_000, 1000) }
  fn split_transfer(legs: u32) -> Weight {
    let bounded_legs = u64::from(legs.min(8));
    Weight::from_parts(15_000_000u64.saturating_add(bounded_legs.saturating_mul(2_000_000)), 1400)
  }
  fn dex_swap() -> Weight { Weight::from_parts(260_000_000, 3200) }
  fn dex_liquidity() -> Weight { Weight::from_parts(220_000_000, 3600) }
  fn noop() -> Weight { Weight::from_parts(1_000_000, 0) }
}
