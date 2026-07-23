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
  fn create_dormant_system_aaa() -> Weight;
  fn activate_aaa() -> Weight;
  fn deactivate_aaa() -> Weight;
  fn pause_aaa() -> Weight;
  fn resume_aaa() -> Weight;
  fn manual_trigger() -> Weight;
  fn close_aaa() -> Weight;
  fn close_aaa_user_fee_bearing_tail(steps: u32, legs: u32) -> Weight;
  fn fee_collection() -> Weight;
  fn task_simple_asset_op() -> Weight;
  fn task_split_transfer(legs: u32) -> Weight;
  fn xcm_asset_deposit() -> Weight;
  fn task_add_liquidity() -> Weight;
  fn task_donate_liquidity() -> Weight;
  fn task_remove_liquidity() -> Weight;
  fn task_stake() -> Weight;
  fn task_unstake() -> Weight;
  fn task_dex_exact_in() -> Weight;
  fn task_dex_exact_out() -> Weight;
  fn scheduler_on_idle_base() -> Weight;
  fn scheduler_zombie_sweep_base() -> Weight;
  fn scheduler_paged_append_existing_page() -> Weight;
  fn scheduler_paged_append_new_page() -> Weight;
  fn scheduler_wakeup_append_existing_page() -> Weight;
  fn scheduler_wakeup_append_new_page() -> Weight;
  fn scheduler_wakeup_replace_exact() -> Weight;
  fn scheduler_wakeup_invalidate_middle_page() -> Weight;
  fn scheduler_wakeup_drain_partial_page() -> Weight;
  fn scheduler_wakeup_drain_full_page() -> Weight;
  fn scheduler_wakeup_drain_dense_boundary() -> Weight;
  fn scheduler_wakeup_drain_stale_page() -> Weight;
  fn scheduler_paged_consume_preserve_page() -> Weight;
  fn scheduler_paged_consume_delete_page() -> Weight;
  fn scheduler_paged_tombstone_drain(entries: u32) -> Weight;
  fn scheduler_paged_mixed_scan(entries: u32) -> Weight;
  fn scheduler_paged_execute_cheap(executions: u32) -> Weight;
  fn scheduler_actor_probe() -> Weight;
  fn scheduler_wakeup_spillover_probe(blocked_buckets: u32) -> Weight;
  fn scheduler_wakeup_dense_due_drain(wakeups: u32) -> Weight;
  fn transaction_extension_ingress_base() -> Weight;
  fn transaction_extension_ingress_notify() -> Weight;
  fn compatibility_ingress_probe() -> Weight;
  fn compatibility_ingress_drain() -> Weight;
  fn funding_batch_promotion(assets: u32) -> Weight;
  fn update_funding_source_policy() -> Weight;
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
  fn dex_exact_in() -> Weight;
  fn dex_exact_out() -> Weight;
  fn add_liquidity() -> Weight;
  fn donate_liquidity() -> Weight;
  fn remove_liquidity() -> Weight;
  fn stake() -> Weight;
  fn unstake() -> Weight;
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

  fn create_dormant_system_aaa() -> Weight {
    Self::create_system_aaa()
  }

  fn activate_aaa() -> Weight {
    Self::create_system_aaa()
  }

  fn deactivate_aaa() -> Weight {
    Self::reopen_system_aaa()
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

  fn close_aaa() -> Weight {
    Weight::from_parts(30_000_000, 2200)
      .saturating_add(T::DbWeight::get().reads(4))
      .saturating_add(T::DbWeight::get().writes(5))
  }

  fn close_aaa_user_fee_bearing_tail(steps: u32, legs: u32) -> Weight {
    Weight::from_parts(60_000_000, 4_000)
      .saturating_add(Weight::from_parts(180_000_000, 3_000).saturating_mul(steps.into()))
      .saturating_add(Weight::from_parts(60_000_000, 3_000).saturating_mul(legs.into()))
  }

  fn fee_collection() -> Weight {
    Weight::from_parts(100_000_000, 16_000)
      .saturating_add(T::DbWeight::get().reads_writes(6, 4))
  }

  fn task_simple_asset_op() -> Weight {
    Weight::from_parts(1_500_000_000, 800_000)
      .saturating_add(T::DbWeight::get().reads_writes(20, 18))
  }

  fn task_split_transfer(legs: u32) -> Weight {
    let bounded = u64::from(legs.min(T::MaxSplitTransferLegs::get()));
    Weight::from_parts(50_000_000, 4_000)
      .saturating_add(Weight::from_parts(1_500_000_000, 800_000).saturating_mul(bounded))
      .saturating_add(T::DbWeight::get().reads_writes(
        bounded.saturating_mul(20),
        bounded.saturating_mul(18),
      ))
  }

  fn xcm_asset_deposit() -> Weight {
    Weight::from_parts(1_600_000_000, 850_000)
      .saturating_add(T::DbWeight::get().reads_writes(20, 18))
  }

  fn task_add_liquidity() -> Weight {
    Weight::from_parts(300_000_000, 24_000)
      .saturating_add(T::DbWeight::get().reads_writes(20, 12))
  }

  fn task_donate_liquidity() -> Weight {
    Weight::from_parts(600_000_000, 48_000)
      .saturating_add(T::DbWeight::get().reads_writes(40, 24))
  }

  fn task_remove_liquidity() -> Weight {
    Weight::from_parts(300_000_000, 24_000)
      .saturating_add(T::DbWeight::get().reads_writes(20, 12))
  }

  fn task_stake() -> Weight {
    Weight::from_parts(200_000_000, 24_000)
      .saturating_add(T::DbWeight::get().reads_writes(20, 12))
  }

  fn task_unstake() -> Weight {
    Weight::from_parts(200_000_000, 24_000)
      .saturating_add(T::DbWeight::get().reads_writes(20, 12))
  }

  fn task_dex_exact_in() -> Weight {
    Weight::from_parts(280_000_000, 13_000)
      .saturating_add(T::DbWeight::get().reads_writes(13, 10))
  }

  fn task_dex_exact_out() -> Weight {
    Weight::from_parts(1_500_000_000, 64_000)
      .saturating_add(T::DbWeight::get().reads_writes(64, 12))
  }

  fn scheduler_on_idle_base() -> Weight {
    Weight::from_parts(20_000_000, 8_000)
      .saturating_add(T::DbWeight::get().reads(4))
      .saturating_add(T::DbWeight::get().writes(3))
  }

  fn scheduler_zombie_sweep_base() -> Weight {
    Weight::from_parts(15_000_000, 4_000).saturating_add(T::DbWeight::get().reads(3))
  }

  fn scheduler_paged_append_existing_page() -> Weight {
    Weight::from_parts(80_000_000, 16_000).saturating_add(T::DbWeight::get().reads_writes(4, 3))
  }

  fn scheduler_paged_append_new_page() -> Weight {
    Weight::from_parts(80_000_000, 16_000).saturating_add(T::DbWeight::get().reads_writes(4, 3))
  }

  fn scheduler_wakeup_append_existing_page() -> Weight {
    Weight::from_parts(100_000_000, 32_000).saturating_add(T::DbWeight::get().reads_writes(3, 3))
  }

  fn scheduler_wakeup_append_new_page() -> Weight {
    Weight::from_parts(120_000_000, 48_000).saturating_add(T::DbWeight::get().reads_writes(4, 4))
  }

  fn scheduler_wakeup_replace_exact() -> Weight {
    Weight::from_parts(160_000_000, 64_000).saturating_add(T::DbWeight::get().reads_writes(5, 6))
  }

  fn scheduler_wakeup_invalidate_middle_page() -> Weight {
    Weight::from_parts(140_000_000, 64_000).saturating_add(T::DbWeight::get().reads_writes(5, 5))
  }

  fn scheduler_wakeup_drain_partial_page() -> Weight {
    Weight::from_parts(1_000_000_000, 200_000).saturating_add(T::DbWeight::get().reads_writes(18, 18))
  }

  fn scheduler_wakeup_drain_full_page() -> Weight {
    Weight::from_parts(2_000_000_000, 400_000).saturating_add(T::DbWeight::get().reads_writes(34, 34))
  }

  fn scheduler_wakeup_drain_dense_boundary() -> Weight {
    Weight::from_parts(2_200_000_000, 450_000).saturating_add(T::DbWeight::get().reads_writes(36, 37))
  }

  fn scheduler_wakeup_drain_stale_page() -> Weight {
    Weight::from_parts(1_500_000_000, 400_000).saturating_add(T::DbWeight::get().reads_writes(34, 2))
  }

  fn scheduler_paged_consume_preserve_page() -> Weight {
    Weight::from_parts(80_000_000, 16_000).saturating_add(T::DbWeight::get().reads_writes(4, 2))
  }

  fn scheduler_paged_consume_delete_page() -> Weight {
    Weight::from_parts(80_000_000, 16_000).saturating_add(T::DbWeight::get().reads_writes(4, 4))
  }

  fn scheduler_paged_tombstone_drain(entries: u32) -> Weight {
    Weight::from_parts(20_000_000, 4_000)
      .saturating_add(Weight::from_parts(20_000_000, 3_000).saturating_mul(entries.into()))
      .saturating_add(T::DbWeight::get().reads_writes(
        3u64.saturating_add(u64::from(entries)),
        2u64.saturating_add(u64::from(entries)),
      ))
  }

  fn scheduler_paged_mixed_scan(entries: u32) -> Weight {
    Weight::from_parts(20_000_000, 4_000)
      .saturating_add(Weight::from_parts(40_000_000, 4_000).saturating_mul(entries.into()))
      .saturating_add(T::DbWeight::get().reads_writes(
        4u64.saturating_add(u64::from(entries).saturating_mul(4)),
        2u64.saturating_add(u64::from(entries).saturating_mul(2)),
      ))
  }

  fn scheduler_paged_execute_cheap(executions: u32) -> Weight {
    Weight::from_parts(50_000_000, 8_000)
      .saturating_add(Weight::from_parts(100_000_000, 8_000).saturating_mul(executions.into()))
  }

  fn scheduler_actor_probe() -> Weight {
    Weight::from_parts(1_600_000_000, 850_000)
      .saturating_add(T::DbWeight::get().reads_writes(18, 16))
  }

  fn scheduler_wakeup_spillover_probe(blocked_buckets: u32) -> Weight {
    let bounded = u64::from(blocked_buckets.min(T::MaxSpilloverBlocks::get().saturating_add(1)));
    Weight::from_parts(
      20_000_000u64.saturating_add(bounded.saturating_mul(5_000_000)),
      4_096u64.saturating_add(bounded.saturating_mul(80_497)),
    )
    .saturating_add(T::DbWeight::get().reads_writes(
      6u64.saturating_add(bounded),
      6u64.saturating_add(bounded),
    ))
  }

  fn scheduler_wakeup_dense_due_drain(wakeups: u32) -> Weight {
    Weight::from_parts(30_000_000, 100_000)
      .saturating_add(Weight::from_parts(20_000_000, 16_000).saturating_mul(wakeups.into()))
      .saturating_add(T::DbWeight::get().reads_writes(
        8u64.saturating_add(u64::from(wakeups).saturating_mul(4)),
        8u64.saturating_add(u64::from(wakeups).saturating_mul(5)),
      ))
  }

  fn transaction_extension_ingress_base() -> Weight {
    Weight::from_parts(10_000_000, 4_096).saturating_add(T::DbWeight::get().reads(1))
  }

  fn transaction_extension_ingress_notify() -> Weight {
    Weight::from_parts(1_500_000_000, 800_000)
      .saturating_add(T::DbWeight::get().reads_writes(16, 16))
  }

  fn compatibility_ingress_probe() -> Weight {
    Weight::from_parts(10_000_000, 1_000).saturating_add(T::DbWeight::get().reads(1))
  }

  fn compatibility_ingress_drain() -> Weight {
    Weight::from_parts(1_600_000_000, 850_000)
      .saturating_add(T::DbWeight::get().reads_writes(20, 20))
  }
  fn funding_batch_promotion(assets: u32) -> Weight {
    Weight::from_parts(20_000_000, 12_000)
      .saturating_add(Weight::from_parts(2_000_000, 256).saturating_mul(assets.into()))
      .saturating_add(T::DbWeight::get().reads_writes(1, 1))
  }

  fn update_funding_source_policy() -> Weight {
    Weight::from_parts(50_000_000, 15_000)
      .saturating_add(T::DbWeight::get().reads_writes(1, 1))
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
    <T as crate::Config>::WeightInfo::task_simple_asset_op()
  }

  fn split_transfer(legs: u32) -> Weight {
    <T as crate::Config>::WeightInfo::task_split_transfer(legs.min(T::MaxSplitTransferLegs::get()))
  }

  fn dex_exact_in() -> Weight {
    <T as crate::Config>::WeightInfo::task_dex_exact_in()
  }

  fn dex_exact_out() -> Weight {
    <T as crate::Config>::WeightInfo::task_dex_exact_out()
  }

  fn add_liquidity() -> Weight {
    <T as crate::Config>::WeightInfo::task_add_liquidity()
  }

  fn donate_liquidity() -> Weight {
    <T as crate::Config>::WeightInfo::task_donate_liquidity()
  }

  fn remove_liquidity() -> Weight {
    <T as crate::Config>::WeightInfo::task_remove_liquidity()
  }

  fn stake() -> Weight {
    <T as crate::Config>::WeightInfo::task_stake()
  }

  fn unstake() -> Weight {
    <T as crate::Config>::WeightInfo::task_unstake()
  }
}

impl WeightInfo for () {
  fn create_user_aaa() -> Weight { Weight::from_parts(25_000_000, 2000) }
  fn create_user_aaa_at_slot() -> Weight { Self::create_user_aaa() }
  fn create_system_aaa() -> Weight { Weight::from_parts(25_000_000, 2000) }
  fn reopen_system_aaa() -> Weight { Weight::from_parts(100_642_000, 174_945) }
  fn create_dormant_system_aaa() -> Weight { Self::create_system_aaa() }
  fn activate_aaa() -> Weight { Self::create_system_aaa() }
  fn deactivate_aaa() -> Weight { Self::reopen_system_aaa() }
  fn pause_aaa() -> Weight { Weight::from_parts(15_000_000, 1200) }
  fn resume_aaa() -> Weight { Weight::from_parts(15_000_000, 1200) }
  fn manual_trigger() -> Weight { Weight::from_parts(12_000_000, 1200) }
  fn close_aaa() -> Weight { Weight::from_parts(30_000_000, 2200) }
  fn close_aaa_user_fee_bearing_tail(steps: u32, legs: u32) -> Weight {
    Weight::from_parts(60_000_000, 4_000)
      .saturating_add(Weight::from_parts(180_000_000, 3_000).saturating_mul(steps.into()))
      .saturating_add(Weight::from_parts(60_000_000, 3_000).saturating_mul(legs.into()))
  }
  fn fee_collection() -> Weight { Weight::from_parts(100_000_000, 16_000) }
  fn task_simple_asset_op() -> Weight { Weight::from_parts(1_500_000_000, 800_000) }
  fn task_split_transfer(legs: u32) -> Weight {
    Weight::from_parts(50_000_000, 4_000)
      .saturating_add(Weight::from_parts(1_500_000_000, 800_000).saturating_mul(legs.min(8).into()))
  }
  fn xcm_asset_deposit() -> Weight { Weight::from_parts(1_600_000_000, 850_000) }
  fn task_add_liquidity() -> Weight { Weight::from_parts(300_000_000, 24_000) }
  fn task_donate_liquidity() -> Weight { Weight::from_parts(600_000_000, 48_000) }
  fn task_remove_liquidity() -> Weight { Weight::from_parts(300_000_000, 24_000) }
  fn task_stake() -> Weight { Weight::from_parts(200_000_000, 24_000) }
  fn task_unstake() -> Weight { Weight::from_parts(200_000_000, 24_000) }
  fn task_dex_exact_in() -> Weight { Weight::from_parts(280_000_000, 13_000) }
  fn task_dex_exact_out() -> Weight { Weight::from_parts(1_500_000_000, 64_000) }
  fn scheduler_on_idle_base() -> Weight { Weight::from_parts(20_000_000, 8_000) }
  fn scheduler_zombie_sweep_base() -> Weight { Weight::from_parts(15_000_000, 4_000) }
  fn scheduler_paged_append_existing_page() -> Weight { Weight::from_parts(80_000_000, 16_000) }
  fn scheduler_paged_append_new_page() -> Weight { Weight::from_parts(80_000_000, 16_000) }
  fn scheduler_wakeup_append_existing_page() -> Weight { Weight::from_parts(100_000_000, 32_000) }
  fn scheduler_wakeup_append_new_page() -> Weight { Weight::from_parts(120_000_000, 48_000) }
  fn scheduler_wakeup_replace_exact() -> Weight { Weight::from_parts(160_000_000, 64_000) }
  fn scheduler_wakeup_invalidate_middle_page() -> Weight { Weight::from_parts(140_000_000, 64_000) }
  fn scheduler_wakeup_drain_partial_page() -> Weight { Weight::from_parts(1_000_000_000, 200_000) }
  fn scheduler_wakeup_drain_full_page() -> Weight { Weight::from_parts(2_000_000_000, 400_000) }
  fn scheduler_wakeup_drain_dense_boundary() -> Weight { Weight::from_parts(2_200_000_000, 450_000) }
  fn scheduler_wakeup_drain_stale_page() -> Weight { Weight::from_parts(1_500_000_000, 400_000) }
  fn scheduler_paged_consume_preserve_page() -> Weight { Weight::from_parts(80_000_000, 16_000) }
  fn scheduler_paged_consume_delete_page() -> Weight { Weight::from_parts(80_000_000, 16_000) }
  fn scheduler_paged_tombstone_drain(entries: u32) -> Weight {
    Weight::from_parts(20_000_000, 4_000)
      .saturating_add(Weight::from_parts(20_000_000, 3_000).saturating_mul(entries.into()))
  }
  fn scheduler_paged_mixed_scan(entries: u32) -> Weight {
    Weight::from_parts(20_000_000, 4_000)
      .saturating_add(Weight::from_parts(40_000_000, 4_000).saturating_mul(entries.into()))
  }
  fn scheduler_paged_execute_cheap(executions: u32) -> Weight {
    Weight::from_parts(50_000_000, 8_000)
      .saturating_add(Weight::from_parts(100_000_000, 8_000).saturating_mul(executions.into()))
  }
  fn scheduler_actor_probe() -> Weight { Weight::from_parts(1_600_000_000, 850_000) }
  fn scheduler_wakeup_spillover_probe(blocked_buckets: u32) -> Weight {
    Weight::from_parts(20_000_000u64.saturating_add(u64::from(blocked_buckets).saturating_mul(5_000_000)), 4_096)
  }
  fn scheduler_wakeup_dense_due_drain(wakeups: u32) -> Weight {
    Weight::from_parts(30_000_000, 100_000)
      .saturating_add(Weight::from_parts(20_000_000, 16_000).saturating_mul(wakeups.into()))
  }
  fn transaction_extension_ingress_base() -> Weight { Weight::from_parts(10_000_000, 4_096) }
  fn transaction_extension_ingress_notify() -> Weight { Weight::from_parts(1_500_000_000, 800_000) }
  fn compatibility_ingress_probe() -> Weight { Weight::from_parts(10_000_000, 1_000) }
  fn compatibility_ingress_drain() -> Weight { Weight::from_parts(1_600_000_000, 850_000) }
  fn funding_batch_promotion(assets: u32) -> Weight {
    Weight::from_parts(20_000_000, 12_000)
      .saturating_add(Weight::from_parts(2_000_000, 256).saturating_mul(assets.into()))
  }
  fn update_funding_source_policy() -> Weight { Weight::from_parts(50_000_000, 15_000) }
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
  fn simple_asset_op() -> Weight { <() as WeightInfo>::task_simple_asset_op() }
  fn split_transfer(legs: u32) -> Weight { <() as WeightInfo>::task_split_transfer(legs) }
  fn dex_exact_in() -> Weight { Weight::from_parts(280_000_000, 13_000) }
  fn dex_exact_out() -> Weight { Weight::from_parts(1_500_000_000, 64_000) }
  fn add_liquidity() -> Weight { Weight::from_parts(300_000_000, 24_000) }
  fn donate_liquidity() -> Weight { Weight::from_parts(600_000_000, 48_000) }
  fn remove_liquidity() -> Weight { Weight::from_parts(300_000_000, 24_000) }
  fn stake() -> Weight { Weight::from_parts(200_000_000, 24_000) }
  fn unstake() -> Weight { Weight::from_parts(200_000_000, 24_000) }
}
