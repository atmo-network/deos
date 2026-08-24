//! # Unmeasured reference values
//!
//! The `WeightInfo` implementations in this file are hand-written estimates, not benchmark
//! output. They exist so the pallet compiles and tests run standalone.
//!
//! A host runtime MUST generate its own weights with `frame-benchmarking` and bind those instead.
//! Binding `SubstrateWeight` or `()` from this file in production underprices execution: the DEOS
//! reference runtime measures several of these calls at more than ten times the value below, with
//! ProofSize and database access that these estimates omit entirely.

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use core::marker::PhantomData;
use polkadot_sdk::frame_support::{
  traits::Get,
  weights::Weight,
};

pub trait WeightInfo {
  fn create_user_actor() -> Weight;
  fn create_user_actor_crossing_new_page() -> Weight;
  fn create_user_actor_at_slot() -> Weight;
  fn create_system_actor() -> Weight;
  fn create_system_actor_at_sovereign_id() -> Weight;
  fn create_dormant_system_actor() -> Weight;
  fn activate_actor() -> Weight;
  fn deactivate_actor() -> Weight;
  fn pause_actor() -> Weight;
  fn resume_actor() -> Weight;
  fn manual_trigger() -> Weight;
  fn address_event_trigger_occurrence() -> Weight;
  fn observation_change_trigger_occurrence() -> Weight;
  fn observation_crossing_trigger_occurrence() -> Weight;
  fn at_time_trigger_occurrence() -> Weight;
  fn cadenced_trigger_occurrence() -> Weight;
  fn observation_change_ingress() -> Weight;
  fn observation_fanout_base() -> Weight;
  fn observation_fanout_branch_probe() -> Weight;
  fn observation_fanout_page() -> Weight;
  fn observation_fanout_wakeup_page() -> Weight;
  fn observation_fanout_coalesced_page() -> Weight;
  fn observation_fanout_blocked_page() -> Weight;
  fn observation_fanout_terminal() -> Weight;
  fn record_crossing_worker_fault() -> Weight;
  fn record_observation_fanout_worker_fault() -> Weight;
  fn record_wakeup_worker_fault() -> Weight;
  fn crossing_worker_base() -> Weight { Weight::from_parts(25_000_000, 8_000) }
  fn crossing_work_probe() -> Weight { Weight::from_parts(400_000_000, 20_000) }
  fn crossing_search_probe() -> Weight { Weight::from_parts(400_000_000, 100_000) }
  fn crossing_fire_probe() -> Weight { Weight::from_parts(1_000_000_000, 300_000) }
  fn crossing_tail_refill_probe() -> Weight { Weight::from_parts(50_000_000, 10_000) }
  fn crossing_fire_pair_probe() -> Weight { Weight::from_parts(2_000_000_000, 500_000) }
  fn crossing_fire_cohort_preflight(c: u32) -> Weight {
    Weight::from_parts(100_000_000, 40_000)
      .saturating_mul(c.into())
  }
  fn crossing_coalesced_cohort_preflight(c: u32) -> Weight {
    Weight::from_parts(100_000_000, 40_000)
      .saturating_mul(c.into())
  }
  fn crossing_terminal_cohort_preflight(c: u32) -> Weight {
    Weight::from_parts(100_000_000, 40_000)
      .saturating_mul(c.into())
  }
  fn crossing_skip_cohort_preflight(c: u32) -> Weight {
    Weight::from_parts(30_000_000, 10_000)
      .saturating_mul(c.into())
  }
  fn crossing_rearm_cohort_preflight(c: u32) -> Weight {
    Weight::from_parts(40_000_000, 20_000)
      .saturating_mul(c.into())
  }
  fn crossing_rearm_pair_probe() -> Weight { Weight::from_parts(1_500_000_000, 400_000) }
  fn crossing_skip_pair_probe() -> Weight { Weight::from_parts(1_000_000_000, 300_000) }
  fn crossing_transition_unit() -> Weight { Weight::from_parts(75_000_000, 24_000) }
  fn crossing_leaf_unit() -> Weight { Weight::from_parts(500_000_000, 180_000) }
  fn crossing_page_unit() -> Weight { Weight::from_parts(100_000_000, 48_000) }
  fn crossing_rearm_unit() -> Weight { Weight::from_parts(750_000_000, 250_000) }
  fn crossing_rearm_pair_unit() -> Weight { Weight::from_parts(1_200_000_000, 400_000) }
  fn crossing_coalesced_unit() -> Weight { Weight::from_parts(750_000_000, 250_000) }
  fn crossing_coalesced_pair_unit() -> Weight { Weight::from_parts(1_200_000_000, 400_000) }
  fn crossing_placed_unit() -> Weight { Weight::from_parts(650_000_000, 220_000) }
  fn crossing_placed_pair_unit() -> Weight { Weight::from_parts(1_300_000_000, 400_000) }
  fn crossing_placed_maximum_unit() -> Weight { Weight::from_parts(2_600_000_000, 800_000) }
  fn crossing_placed_non_tail_emptied_unit() -> Weight { Weight::from_parts(2_600_000_000, 800_000) }
  fn crossing_placed_non_tail_trimmed_unit() -> Weight { Weight::from_parts(2_600_000_000, 800_000) }
  fn crossing_skip_unit() -> Weight { Weight::from_parts(500_000_000, 180_000) }
  fn crossing_skip_pair_unit() -> Weight { Weight::from_parts(750_000_000, 250_000) }
  fn crossing_actor_unit() -> Weight { Weight::from_parts(750_000_000, 250_000) }
  fn pipeline_admission_apoptosis() -> Weight;
  fn close_actor() -> Weight;
  fn fee_collection() -> Weight;
  fn predicate_set_evaluation(predicates: u32) -> Weight;
  fn cycle_orchestration() -> Weight;
  fn step_orchestration(steps: u32) -> Weight;
  fn task_transfer() -> Weight;
  fn task_burn() -> Weight;
  fn task_mint() -> Weight;
  fn task_stop_cycle() -> Weight;
  fn task_split_transfer(legs: u32) -> Weight;
  fn xcm_asset_deposit() -> Weight;
  fn task_add_liquidity() -> Weight;
  fn task_donate_liquidity() -> Weight;
  fn task_remove_liquidity() -> Weight;
  fn task_stake() -> Weight;
  fn task_unstake() -> Weight;
  fn task_dex_exact_in() -> Weight;
  fn task_dex_exact_out() -> Weight;
  fn contract_geometry_create(chunks: u32) -> Weight;
  fn contract_geometry_close(chunks: u32) -> Weight;
  fn contract_geometry_reconstruct(chunks: u32) -> Weight;
  fn current_step_load_head() -> Weight;
  fn current_step_load_tail(steps_in_chunk: u32) -> Weight;
  fn current_step_plan_opening_head() -> Weight;
  fn current_step_plan_suspended_head() -> Weight;
  fn current_step_plan_running_tail(steps_in_chunk: u32) -> Weight;
  fn opening_snapshot_capture(entries: u32) -> Weight;
  fn opening_predicate_capture(predicates: u32) -> Weight;
  fn scheduler_on_initialize_cutoff() -> Weight;
  fn scheduler_on_idle_base() -> Weight;
  fn materialization_coordinator_base() -> Weight;
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
  fn scheduler_wakeup_cursor_insert() -> Weight;
  fn scheduler_wakeup_cursor_pop_min() -> Weight;
  fn scheduler_wakeup_cursor_remove_exact() -> Weight;
  fn scheduler_wakeup_cursor_worker_partial() -> Weight;
  fn scheduler_wakeup_cursor_worker_remove() -> Weight;
  fn scheduler_wakeup_cursor_worker_future() -> Weight;
  fn scheduler_paged_consume_preserve_page() -> Weight;
  fn scheduler_paged_consume_delete_page() -> Weight;
  fn scheduler_paged_tombstone_drain(entries: u32) -> Weight;
  fn scheduler_paged_mixed_scan(entries: u32) -> Weight;
  fn scheduler_inner_zero_step_complete() -> Weight;
  fn scheduler_paged_execute_opening_max() -> Weight;
  fn scheduler_inner_opening_close_min(tail_chunks: u32) -> Weight;
  fn scheduler_inner_opening_close_max(tail_chunks: u32) -> Weight;
  fn scheduler_inner_opening_failed_min(tail_chunks: u32) -> Weight;
  fn scheduler_inner_opening_failed_max(tail_chunks: u32) -> Weight;
  fn scheduler_inner_opening_retry_min(tail_chunks: u32) -> Weight;
  fn scheduler_inner_opening_retry_max(tail_chunks: u32) -> Weight;
  fn scheduler_inner_opening_complete_min(tail_chunks: u32) -> Weight;
  fn scheduler_inner_opening_complete_max(tail_chunks: u32) -> Weight;
  fn scheduler_inner_opening_progress_min(tail_chunks: u32) -> Weight;
  fn scheduler_inner_opening_progress_max(tail_chunks: u32) -> Weight;
  fn scheduler_inner_running_complete(steps_in_fragment: u32, predicates: u32) -> Weight;
  fn scheduler_inner_running_progress(steps_in_fragment: u32, predicates: u32) -> Weight;
  fn scheduler_inner_suspended_tail_retry(steps_in_fragment: u32, predicates: u32) -> Weight;
  fn scheduler_inner_suspended_tail_complete(steps_in_fragment: u32, predicates: u32) -> Weight;
  fn scheduler_inner_suspended_tail_progress(steps_in_fragment: u32, predicates: u32) -> Weight;
  fn scheduler_inner_suspended_head_retry(
    opening_snapshot_entries: u32,
    opening_predicate_results: u32,
    funding_snapshot_entries: u32,
    predicates: u32,
  ) -> Weight;
  fn scheduler_inner_suspended_head_complete(
    opening_snapshot_entries: u32,
    opening_predicate_results: u32,
    funding_snapshot_entries: u32,
    predicates: u32,
  ) -> Weight;
  fn scheduler_inner_suspended_head_progress(
    opening_snapshot_entries: u32,
    opening_predicate_results: u32,
    funding_snapshot_entries: u32,
    predicates: u32,
  ) -> Weight;
  fn scheduler_inner_suspended_head_opening_retry(
    opening_snapshot_entries: u32,
    opening_predicate_results: u32,
    funding_snapshot_entries: u32,
  ) -> Weight;
  fn scheduler_inner_suspended_head_opening_complete(
    opening_snapshot_entries: u32,
    opening_predicate_results: u32,
    funding_snapshot_entries: u32,
  ) -> Weight;
  fn scheduler_inner_suspended_head_opening_progress(
    opening_snapshot_entries: u32,
    opening_predicate_results: u32,
    funding_snapshot_entries: u32,
  ) -> Weight;
  fn scheduler_paged_execute_cheap(executions: u32) -> Weight;
  fn scheduler_paged_execute_cheap_mixed(executions: u32) -> Weight;
  fn scheduler_actor_state_probe() -> Weight;
  fn transaction_extension_ingress_base() -> Weight;
  fn transaction_extension_ingress_notify() -> Weight;
  fn funding_snapshot_open(assets: u32) -> Weight;
  fn run_progress() -> Weight;
  fn run_suspend() -> Weight;
  fn run_retry() -> Weight;
  fn run_complete() -> Weight;
  fn run_cancel() -> Weight;
  fn run_suffix_admission(steps: u32) -> Weight;
  fn update_contract() -> Weight;
  fn set_global_circuit_breaker() -> Weight;
  fn clear_crossing_worker_fault() -> Weight;
  fn clear_observation_fanout_worker_fault() -> Weight;
  fn clear_wakeup_worker_fault() -> Weight;
  fn set_active_actor_limit() -> Weight;
  fn permissionless_sweep() -> Weight;
  fn permissionless_sweep_many(batch: u32) -> Weight;
  fn maximum_context_inherent() -> Weight;
  fn maximum_xcm_version_discovery() -> Weight;
  fn block_resource_meter_extension() -> Weight;
  fn block_resource_finalize() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: polkadot_sdk::frame_system::Config + crate::Config> WeightInfo for SubstrateWeight<T> {
  fn create_user_actor() -> Weight {
    Weight::from_parts(25_000_000, 2000)
      .saturating_add(T::DbWeight::get().reads(4))
      .saturating_add(T::DbWeight::get().writes(5))
  }

  fn create_user_actor_crossing_new_page() -> Weight {
    Self::create_user_actor()
  }

  fn create_user_actor_at_slot() -> Weight {
    Self::create_user_actor()
  }

  fn create_system_actor() -> Weight {
    Weight::from_parts(25_000_000, 2000)
      .saturating_add(T::DbWeight::get().reads(3))
      .saturating_add(T::DbWeight::get().writes(4))
  }

  fn create_system_actor_at_sovereign_id() -> Weight {
    Weight::from_parts(100_642_000, 174_945)
      .saturating_add(T::DbWeight::get().reads(20))
      .saturating_add(T::DbWeight::get().writes(4))
  }

  fn create_dormant_system_actor() -> Weight {
    Self::create_system_actor()
  }

  fn activate_actor() -> Weight {
    Self::create_system_actor()
  }

  fn deactivate_actor() -> Weight {
    Weight::from_parts(60_623_000, 8_120)
      .saturating_add(T::DbWeight::get().reads(4))
      .saturating_add(T::DbWeight::get().writes(6))
  }

  fn pause_actor() -> Weight {
    Weight::from_parts(15_000_000, 1200)
      .saturating_add(T::DbWeight::get().reads(1))
      .saturating_add(T::DbWeight::get().writes(2))
  }

  fn resume_actor() -> Weight {
    Weight::from_parts(15_000_000, 1200)
      .saturating_add(T::DbWeight::get().reads(1))
      .saturating_add(T::DbWeight::get().writes(2))
  }

  fn manual_trigger() -> Weight {
    Weight::from_parts(113_494_000, 9_635)
      .saturating_add(T::DbWeight::get().reads(13))
      .saturating_add(T::DbWeight::get().writes(5))
  }

  fn address_event_trigger_occurrence() -> Weight {
    Weight::from_parts(169_927_000, 8_366)
      .saturating_add(T::DbWeight::get().reads(14))
      .saturating_add(T::DbWeight::get().writes(7))
  }

  fn observation_change_trigger_occurrence() -> Weight {
    Weight::from_parts(117_615_000, 8_295)
      .saturating_add(T::DbWeight::get().reads(13))
      .saturating_add(T::DbWeight::get().writes(7))
  }

  fn observation_crossing_trigger_occurrence() -> Weight {
    Weight::from_parts(499_862_000, 164_106)
      .saturating_add(T::DbWeight::get().reads(89))
      .saturating_add(T::DbWeight::get().writes(80))
  }

  fn at_time_trigger_occurrence() -> Weight {
    Weight::from_parts(157_425_000, 8_317)
      .saturating_add(T::DbWeight::get().reads(14))
      .saturating_add(T::DbWeight::get().writes(7))
  }

  fn cadenced_trigger_occurrence() -> Weight {
    Weight::from_parts(195_070_000, 8_325)
      .saturating_add(T::DbWeight::get().reads(17))
      .saturating_add(T::DbWeight::get().writes(11))
  }

  fn observation_change_ingress() -> Weight {
    Weight::from_parts(75_000_000, 24_000)
  }

  fn observation_fanout_base() -> Weight {
    Weight::from_parts(15_000_000, 4_000)
  }

  fn observation_fanout_branch_probe() -> Weight {
    Weight::from_parts(20_000_000, 6_000)
  }

  fn observation_fanout_page() -> Weight {
    Weight::from_parts(150_000_000_000, 750_000)
  }

  fn observation_fanout_wakeup_page() -> Weight {
    Weight::from_parts(8_000_000_000, 750_000)
  }

  fn observation_fanout_coalesced_page() -> Weight {
    Weight::from_parts(8_000_000_000, 750_000)
  }

  fn observation_fanout_blocked_page() -> Weight {
    Weight::from_parts(150_000_000_000, 400_000)
  }

  fn observation_fanout_terminal() -> Weight {
    Weight::from_parts(8_000_000_000, 750_000)
  }

  fn record_crossing_worker_fault() -> Weight {
    Weight::from_parts(16_000_000, 1_529)
      .saturating_add(T::DbWeight::get().reads_writes(1, 1))
  }

  fn record_observation_fanout_worker_fault() -> Weight {
    Weight::from_parts(16_000_000, 1_529)
      .saturating_add(T::DbWeight::get().reads_writes(1, 1))
  }

  fn record_wakeup_worker_fault() -> Weight {
    Weight::from_parts(16_000_000, 1_529)
      .saturating_add(T::DbWeight::get().reads_writes(1, 1))
  }

  fn pipeline_admission_apoptosis() -> Weight {
    Weight::from_parts(161_616_000, 5_736)
      .saturating_add(T::DbWeight::get().reads(15))
      .saturating_add(T::DbWeight::get().writes(15))
  }

  fn close_actor() -> Weight {
    Weight::from_parts(84_719_000, 8_120)
      .saturating_add(T::DbWeight::get().reads(8))
      .saturating_add(T::DbWeight::get().writes(8))
  }

  fn fee_collection() -> Weight {
    Weight::from_parts(112_097_000, 8_120)
      .saturating_add(T::DbWeight::get().reads(6))
      .saturating_add(T::DbWeight::get().writes(1))
  }

  fn predicate_set_evaluation(predicates: u32) -> Weight {
    if predicates == 0 {
      return Weight::zero();
    }
    let bounded = u64::from(predicates.min(4));
    Weight::from_parts(8_660_000, 3_675)
      .saturating_add(Weight::from_parts(9_778_566, 2_561).saturating_mul(bounded))
      .saturating_add(T::DbWeight::get().reads(1u64.saturating_add(2u64.saturating_mul(bounded))))
  }

  fn cycle_orchestration() -> Weight {
    Weight::from_parts(44_699_000, 9667).saturating_add(T::DbWeight::get().reads_writes(3, 2))
  }

  fn step_orchestration(steps: u32) -> Weight {
    Weight::from_parts(44_555_323, 9667)
      .saturating_add(Weight::from_parts(215_321, 0).saturating_mul(steps.into()))
      .saturating_add(T::DbWeight::get().reads_writes(3, 2))
  }

  fn task_transfer() -> Weight {
    Weight::from_parts(159_800_000, 8_120)
      .saturating_add(T::DbWeight::get().reads(12))
      .saturating_add(T::DbWeight::get().writes(8))
  }

  fn task_burn() -> Weight {
    Weight::from_parts(23_397_000, 3_593)
      .saturating_add(T::DbWeight::get().reads_writes(1, 1))
  }

  fn task_mint() -> Weight {
    Weight::from_parts(105_812_000, 8_120)
      .saturating_add(T::DbWeight::get().reads(10))
      .saturating_add(T::DbWeight::get().writes(6))
  }

  fn task_stop_cycle() -> Weight {
    Weight::from_parts(5_238_000, 0)
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
    Weight::from_parts(178_587_000, 8_817)
      .saturating_add(T::DbWeight::get().reads(8))
      .saturating_add(T::DbWeight::get().writes(6))
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

  fn contract_geometry_create(chunks: u32) -> Weight {
    Weight::from_parts(100_000_000, 16_000)
      .saturating_add(Weight::from_parts(25_000_000, 8_000).saturating_mul(chunks.into()))
      .saturating_add(T::DbWeight::get().reads_writes(
        u64::from(2u32.saturating_add(chunks)),
        u64::from(2u32.saturating_add(chunks)),
      ))
  }

  fn contract_geometry_close(chunks: u32) -> Weight {
    Weight::from_parts(100_000_000, 16_000)
      .saturating_add(Weight::from_parts(25_000_000, 8_000).saturating_mul(chunks.into()))
      .saturating_add(T::DbWeight::get().reads_writes(
        u64::from(2u32.saturating_add(chunks)),
        u64::from(2u32.saturating_add(chunks)),
      ))
  }

  fn contract_geometry_reconstruct(chunks: u32) -> Weight {
    Weight::from_parts(75_000_000, 16_000)
      .saturating_add(Weight::from_parts(20_000_000, 8_000).saturating_mul(chunks.into()))
      .saturating_add(T::DbWeight::get().reads(u64::from(
        2u32.saturating_add(chunks),
      )))
  }

  fn current_step_load_head() -> Weight {
    Weight::from_parts(30_000_000, 8_000).saturating_add(T::DbWeight::get().reads(2))
  }

  fn current_step_load_tail(steps_in_chunk: u32) -> Weight {
    Weight::from_parts(40_000_000, 16_000)
      .saturating_add(
        Weight::from_parts(1_000_000, 512).saturating_mul(steps_in_chunk.into()),
      )
      .saturating_add(T::DbWeight::get().reads(3))
  }

  fn current_step_plan_opening_head() -> Weight {
    Weight::from_parts(100_000_000, 24_000).saturating_add(T::DbWeight::get().reads(8))
  }

  fn current_step_plan_suspended_head() -> Weight {
    Weight::from_parts(150_000_000, 32_000).saturating_add(T::DbWeight::get().reads(8))
  }

  fn current_step_plan_running_tail(steps_in_chunk: u32) -> Weight {
    Weight::from_parts(150_000_000, 32_000)
      .saturating_add(
        Weight::from_parts(1_000_000, 512).saturating_mul(steps_in_chunk.into()),
      )
      .saturating_add(T::DbWeight::get().reads(10))
  }

  fn opening_snapshot_capture(entries: u32) -> Weight {
    Weight::from_parts(25_000_000, 4_000)
      .saturating_add(Weight::from_parts(15_000_000, 3_000).saturating_mul(entries.into()))
      .saturating_add(T::DbWeight::get().reads(u64::from(entries)))
  }

  fn opening_predicate_capture(predicates: u32) -> Weight {
    Weight::from_parts(25_000_000, 4_000)
      .saturating_add(Weight::from_parts(15_000_000, 3_000).saturating_mul(predicates.into()))
      .saturating_add(T::DbWeight::get().reads(u64::from(predicates)))
  }

  fn scheduler_on_initialize_cutoff() -> Weight {
    Weight::from_parts(7_543_000, 1_493)
      .saturating_add(T::DbWeight::get().reads(1))
      .saturating_add(T::DbWeight::get().writes(1))
  }

  fn scheduler_on_idle_base() -> Weight {
    Weight::from_parts(25_000_000, 2_500)
      .saturating_add(T::DbWeight::get().reads(7))
      .saturating_add(T::DbWeight::get().writes(1))
  }

  fn materialization_coordinator_base() -> Weight {
    Weight::from_parts(20_000_000, 4_000)
      .saturating_add(T::DbWeight::get().reads(1))
      .saturating_add(T::DbWeight::get().writes(1))
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

  fn scheduler_wakeup_cursor_insert() -> Weight {
    Weight::from_parts(2_000_000_000, 500_000).saturating_add(T::DbWeight::get().reads_writes(100, 100))
  }

  fn scheduler_wakeup_cursor_pop_min() -> Weight {
    Weight::from_parts(2_000_000_000, 500_000).saturating_add(T::DbWeight::get().reads_writes(100, 100))
  }

  fn scheduler_wakeup_cursor_remove_exact() -> Weight {
    Weight::from_parts(2_000_000_000, 500_000).saturating_add(T::DbWeight::get().reads_writes(100, 100))
  }

  fn scheduler_wakeup_cursor_worker_partial() -> Weight {
    Weight::from_parts(3_000_000_000, 750_000).saturating_add(T::DbWeight::get().reads_writes(150, 150))
  }

  fn scheduler_wakeup_cursor_worker_remove() -> Weight {
    Weight::from_parts(3_000_000_000, 750_000).saturating_add(T::DbWeight::get().reads_writes(150, 150))
  }

  fn scheduler_wakeup_cursor_worker_future() -> Weight {
    Weight::from_parts(500_000_000, 100_000).saturating_add(T::DbWeight::get().reads(10))
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

  fn scheduler_inner_zero_step_complete() -> Weight {
    Weight::from_parts(37_645_000, 4_570)
  }

  fn scheduler_paged_execute_opening_max() -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_opening_close_min(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_opening_close_max(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_opening_failed_min(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_opening_failed_max(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_opening_retry_min(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_opening_retry_max(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_opening_complete_min(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_opening_complete_max(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_opening_progress_min(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_opening_progress_max(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_running_complete(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_running_progress(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_suspended_tail_retry(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_suspended_tail_complete(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_suspended_tail_progress(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_suspended_head_retry(_: u32, _: u32, _: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_suspended_head_complete(_: u32, _: u32, _: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_suspended_head_progress(_: u32, _: u32, _: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_suspended_head_opening_retry(_: u32, _: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_suspended_head_opening_complete(_: u32, _: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_suspended_head_opening_progress(_: u32, _: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_paged_execute_cheap(executions: u32) -> Weight {
    Weight::from_parts(50_000_000, 8_000)
      .saturating_add(Weight::from_parts(100_000_000, 8_000).saturating_mul(executions.into()))
  }

  fn scheduler_paged_execute_cheap_mixed(executions: u32) -> Weight {
    Weight::from_parts(75_000_000, 16_000)
      .saturating_add(Weight::from_parts(125_000_000, 16_000).saturating_mul(executions.into()))
  }

  fn scheduler_actor_state_probe() -> Weight {
    Weight::from_parts(38_413_000, 12_200)
      .saturating_add(T::DbWeight::get().reads(5))
  }

  fn transaction_extension_ingress_base() -> Weight {
    Weight::from_parts(15_226_000, 6_052).saturating_add(T::DbWeight::get().reads(2))
  }

  fn transaction_extension_ingress_notify() -> Weight {
    Weight::from_parts(88_280_000, 8_120)
      .saturating_add(T::DbWeight::get().reads(9))
      .saturating_add(T::DbWeight::get().writes(6))
  }

  fn funding_snapshot_open(assets: u32) -> Weight {
    Weight::from_parts(15_653_872, 4_265)
      .saturating_add(Weight::from_parts(146_253, 0).saturating_mul(assets.into()))
      .saturating_add(T::DbWeight::get().reads_writes(1, 1))
  }

  fn run_progress() -> Weight {
    Weight::from_parts(30_000_000, 8_000).saturating_add(T::DbWeight::get().reads_writes(6, 2))
  }

  fn run_suspend() -> Weight {
    Weight::from_parts(28_668_868, 4_178)
      .saturating_add(T::DbWeight::get().reads_writes(2, 2))
  }
  fn run_retry() -> Weight {
    Weight::from_parts(22_070_000, 4_266).saturating_add(T::DbWeight::get().reads_writes(1, 1))
  }
  fn run_complete() -> Weight {
    Weight::from_parts(18_019_000, 4_030).saturating_add(T::DbWeight::get().reads_writes(1, 2))
  }
  fn run_cancel() -> Weight {
    Weight::from_parts(56_782_000, 8_120).saturating_add(T::DbWeight::get().reads_writes(6, 4))
  }
  fn run_suffix_admission(steps: u32) -> Weight {
    Weight::from_parts(1_438_574, 0)
      .saturating_add(Weight::from_parts(432, 0).saturating_mul(steps.into()))
  }

  fn update_contract() -> Weight {
    Weight::from_parts(162_733_000, 10_181)
      .saturating_add(T::DbWeight::get().reads(11))
      .saturating_add(T::DbWeight::get().writes(9))
  }

  fn set_global_circuit_breaker() -> Weight {
    Weight::from_parts(8_000_000, 600)
      .saturating_add(T::DbWeight::get().writes(1))
  }

  fn clear_crossing_worker_fault() -> Weight {
    Weight::from_parts(16_000_000, 1_529)
      .saturating_add(T::DbWeight::get().reads_writes(1, 1))
  }

  fn clear_observation_fanout_worker_fault() -> Weight {
    Weight::from_parts(16_000_000, 1_529)
      .saturating_add(T::DbWeight::get().reads_writes(1, 1))
  }

  fn clear_wakeup_worker_fault() -> Weight {
    Weight::from_parts(16_000_000, 1_529)
      .saturating_add(T::DbWeight::get().reads_writes(1, 1))
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
    let bounded = u64::from(batch.min(T::MaxSweepBatch::get()));
    Weight::from_parts(
      12_000_000u64.saturating_add(18_000_000u64.saturating_mul(bounded)),
      1200u64.saturating_add(384u64.saturating_mul(bounded)),
    )
    .saturating_add(T::DbWeight::get().reads(1u64.saturating_add(bounded)))
    .saturating_add(T::DbWeight::get().writes(bounded.saturating_mul(5)))
  }

  fn maximum_context_inherent() -> Weight {
    Weight::MAX
  }
  fn maximum_xcm_version_discovery() -> Weight {
    Weight::MAX
  }
  fn block_resource_meter_extension() -> Weight {
    Weight::MAX
  }
  fn block_resource_finalize() -> Weight {
    Weight::MAX
  }
}

#[cfg(any(test, feature = "runtime-benchmarks"))]
pub struct TestWeightInfo;

#[cfg(any(test, feature = "runtime-benchmarks"))]
impl WeightInfo for TestWeightInfo {
  fn create_user_actor() -> Weight { Weight::from_parts(25_000_000, 2000) }
  fn create_user_actor_crossing_new_page() -> Weight { Self::create_user_actor() }
  fn create_user_actor_at_slot() -> Weight { Self::create_user_actor() }
  fn create_system_actor() -> Weight { Weight::from_parts(25_000_000, 2000) }
  fn create_system_actor_at_sovereign_id() -> Weight { Weight::from_parts(100_642_000, 174_945) }
  fn create_dormant_system_actor() -> Weight { Self::create_system_actor() }
  fn activate_actor() -> Weight { Self::create_system_actor() }
  fn deactivate_actor() -> Weight { Weight::from_parts(60_623_000, 8_120) }
  fn pause_actor() -> Weight { Weight::from_parts(15_000_000, 1200) }
  fn resume_actor() -> Weight { Weight::from_parts(15_000_000, 1200) }
  fn manual_trigger() -> Weight { Weight::from_parts(113_494_000, 9_635) }
  fn address_event_trigger_occurrence() -> Weight { Weight::from_parts(169_927_000, 8_366) }
  fn observation_change_trigger_occurrence() -> Weight { Weight::from_parts(117_615_000, 8_295) }
  fn observation_crossing_trigger_occurrence() -> Weight { Weight::from_parts(499_862_000, 164_106) }
  fn at_time_trigger_occurrence() -> Weight { Weight::from_parts(157_425_000, 8_317) }
  fn cadenced_trigger_occurrence() -> Weight { Weight::from_parts(195_070_000, 8_325) }
  fn observation_change_ingress() -> Weight { Weight::from_parts(75_000_000, 24_000) }
  fn observation_fanout_base() -> Weight { Weight::from_parts(15_000_000, 4_000) }
  fn observation_fanout_branch_probe() -> Weight { Weight::zero() }
  fn observation_fanout_page() -> Weight { Weight::from_parts(150_000_000_000, 750_000) }
  fn observation_fanout_wakeup_page() -> Weight { Weight::from_parts(8_000_000_000, 750_000) }
  fn observation_fanout_coalesced_page() -> Weight { Weight::from_parts(8_000_000_000, 750_000) }
  fn observation_fanout_blocked_page() -> Weight { Weight::from_parts(150_000_000_000, 400_000) }
  fn observation_fanout_terminal() -> Weight { Weight::from_parts(8_000_000_000, 750_000) }
  fn record_crossing_worker_fault() -> Weight { Weight::from_parts(16_000_000, 1_529) }
  fn record_observation_fanout_worker_fault() -> Weight { Weight::from_parts(16_000_000, 1_529) }
  fn record_wakeup_worker_fault() -> Weight { Weight::from_parts(16_000_000, 1_529) }
  fn pipeline_admission_apoptosis() -> Weight { Weight::from_parts(161_616_000, 5_736) }
  fn close_actor() -> Weight { Weight::from_parts(84_719_000, 8_120) }
  fn fee_collection() -> Weight { Weight::from_parts(112_097_000, 8_120) }
  fn predicate_set_evaluation(predicates: u32) -> Weight {
    if predicates == 0 {
      return Weight::zero();
    }
    let bounded = u64::from(predicates.min(4));
    Weight::from_parts(8_660_000, 3_675)
      .saturating_add(Weight::from_parts(9_778_566, 2_561).saturating_mul(bounded))

  }
  fn cycle_orchestration() -> Weight { Weight::from_parts(44_699_000, 9667) }
  fn step_orchestration(steps: u32) -> Weight {
    Weight::from_parts(44_555_323, 9667)
      .saturating_add(Weight::from_parts(215_321, 0).saturating_mul(steps.into()))
  }
  fn task_transfer() -> Weight { Weight::from_parts(159_800_000, 8_120) }
  fn task_burn() -> Weight { Weight::from_parts(23_397_000, 3_593) }
  fn task_mint() -> Weight { Weight::from_parts(105_812_000, 8_120) }
  fn task_stop_cycle() -> Weight { Weight::from_parts(5_238_000, 0) }
  fn task_split_transfer(legs: u32) -> Weight {
    Weight::from_parts(50_000_000, 4_000)
      .saturating_add(Weight::from_parts(1_500_000_000, 800_000).saturating_mul(legs.min(8).into()))
  }
  fn xcm_asset_deposit() -> Weight { Weight::from_parts(1_600_000_000, 850_000) }
  fn task_add_liquidity() -> Weight { Weight::from_parts(300_000_000, 24_000) }
  fn task_donate_liquidity() -> Weight { Weight::from_parts(600_000_000, 48_000) }
  fn task_remove_liquidity() -> Weight { Weight::from_parts(178_587_000, 8_817) }
  fn task_stake() -> Weight { Weight::from_parts(200_000_000, 24_000) }
  fn task_unstake() -> Weight { Weight::from_parts(200_000_000, 24_000) }
  fn task_dex_exact_in() -> Weight { Weight::from_parts(280_000_000, 13_000) }
  fn task_dex_exact_out() -> Weight { Weight::from_parts(1_500_000_000, 64_000) }
  fn contract_geometry_create(chunks: u32) -> Weight {
    Weight::from_parts(100_000_000, 16_000)
      .saturating_add(Weight::from_parts(25_000_000, 8_000).saturating_mul(chunks.into()))
  }
  fn contract_geometry_close(chunks: u32) -> Weight {
    Weight::from_parts(100_000_000, 16_000)
      .saturating_add(Weight::from_parts(25_000_000, 8_000).saturating_mul(chunks.into()))
  }
  fn contract_geometry_reconstruct(chunks: u32) -> Weight {
    Weight::from_parts(75_000_000, 16_000)
      .saturating_add(Weight::from_parts(20_000_000, 8_000).saturating_mul(chunks.into()))
  }
  fn current_step_load_head() -> Weight { Weight::from_parts(30_000_000, 8_000) }
  fn current_step_load_tail(steps_in_chunk: u32) -> Weight {
    Weight::from_parts(40_000_000, 16_000)
      .saturating_add(Weight::from_parts(1_000_000, 512).saturating_mul(steps_in_chunk.into()))
  }
  fn current_step_plan_opening_head() -> Weight { Weight::from_parts(100_000_000, 24_000) }
  fn current_step_plan_suspended_head() -> Weight { Weight::from_parts(150_000_000, 32_000) }
  fn current_step_plan_running_tail(steps_in_chunk: u32) -> Weight {
    Weight::from_parts(150_000_000, 32_000)
      .saturating_add(Weight::from_parts(1_000_000, 512).saturating_mul(steps_in_chunk.into()))
  }
  fn opening_snapshot_capture(entries: u32) -> Weight {
    Weight::from_parts(25_000_000, 4_000)
      .saturating_add(Weight::from_parts(15_000_000, 3_000).saturating_mul(entries.into()))
  }
  fn opening_predicate_capture(predicates: u32) -> Weight {
    Weight::from_parts(25_000_000, 4_000)
      .saturating_add(Weight::from_parts(15_000_000, 3_000).saturating_mul(predicates.into()))
  }
  fn scheduler_on_initialize_cutoff() -> Weight { Weight::from_parts(7_543_000, 1_493) }
  fn scheduler_on_idle_base() -> Weight { Weight::from_parts(25_000_000, 2_500) }
  fn materialization_coordinator_base() -> Weight { Weight::from_parts(20_000_000, 4_000) }
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
  fn scheduler_wakeup_cursor_insert() -> Weight { Weight::from_parts(2_000_000_000, 500_000) }
  fn scheduler_wakeup_cursor_pop_min() -> Weight { Weight::from_parts(2_000_000_000, 500_000) }
  fn scheduler_wakeup_cursor_remove_exact() -> Weight { Weight::from_parts(2_000_000_000, 500_000) }
  fn scheduler_wakeup_cursor_worker_partial() -> Weight { Weight::from_parts(3_000_000_000, 750_000) }
  fn scheduler_wakeup_cursor_worker_remove() -> Weight { Weight::from_parts(3_000_000_000, 750_000) }
  fn scheduler_wakeup_cursor_worker_future() -> Weight { Weight::from_parts(500_000_000, 100_000) }
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
  fn scheduler_inner_zero_step_complete() -> Weight {
    Weight::from_parts(37_645_000, 4_570)
  }
  fn scheduler_paged_execute_opening_max() -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_opening_close_min(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_opening_close_max(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_opening_failed_min(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_opening_failed_max(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_opening_retry_min(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_opening_retry_max(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_opening_complete_min(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_opening_complete_max(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_opening_progress_min(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_opening_progress_max(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_running_complete(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_running_progress(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_suspended_tail_retry(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_suspended_tail_complete(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_suspended_tail_progress(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_suspended_head_retry(_: u32, _: u32, _: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_suspended_head_complete(_: u32, _: u32, _: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_suspended_head_progress(_: u32, _: u32, _: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_suspended_head_opening_retry(_: u32, _: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_suspended_head_opening_complete(_: u32, _: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_suspended_head_opening_progress(_: u32, _: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_paged_execute_cheap(executions: u32) -> Weight {
    Weight::from_parts(50_000_000, 8_000)
      .saturating_add(Weight::from_parts(100_000_000, 8_000).saturating_mul(executions.into()))
  }
  fn scheduler_paged_execute_cheap_mixed(executions: u32) -> Weight {
    Weight::from_parts(75_000_000, 16_000)
      .saturating_add(Weight::from_parts(125_000_000, 16_000).saturating_mul(executions.into()))
  }
  fn scheduler_actor_state_probe() -> Weight { Weight::from_parts(38_413_000, 12_200) }
  fn transaction_extension_ingress_base() -> Weight { Weight::from_parts(15_226_000, 6_052) }
  fn transaction_extension_ingress_notify() -> Weight { Weight::from_parts(88_280_000, 8_120) }
  fn funding_snapshot_open(assets: u32) -> Weight {
    Weight::from_parts(15_653_872, 4_265)
      .saturating_add(Weight::from_parts(146_253, 0).saturating_mul(assets.into()))
  }
  fn run_progress() -> Weight { Weight::from_parts(30_000_000, 8_000) }
  fn run_suspend() -> Weight { Weight::from_parts(28_668_868, 4_178) }
  fn run_retry() -> Weight { Weight::from_parts(22_070_000, 4_266) }
  fn run_complete() -> Weight { Weight::from_parts(18_019_000, 4_030) }
  fn run_cancel() -> Weight { Weight::from_parts(56_782_000, 8_120) }
  fn run_suffix_admission(steps: u32) -> Weight {
    Weight::from_parts(1_438_574, 0)
      .saturating_add(Weight::from_parts(432, 0).saturating_mul(steps.into()))
  }
  fn update_contract() -> Weight { Weight::from_parts(162_733_000, 10_181) }
  fn set_global_circuit_breaker() -> Weight { Weight::from_parts(8_000_000, 600) }
  fn clear_crossing_worker_fault() -> Weight { Weight::from_parts(16_000_000, 1_529) }
  fn clear_observation_fanout_worker_fault() -> Weight { Weight::from_parts(16_000_000, 1_529) }
  fn clear_wakeup_worker_fault() -> Weight { Weight::from_parts(16_000_000, 1_529) }
  fn set_active_actor_limit() -> Weight { Weight::from_parts(10_000_000, 800) }
  fn crossing_placed_non_tail_emptied_unit() -> Weight {
    Weight::from_parts(2_700_000_000, 850_000)
  }
  fn crossing_placed_non_tail_trimmed_unit() -> Weight {
    Weight::from_parts(2_800_000_000, 900_000)
  }
  fn permissionless_sweep() -> Weight { Weight::from_parts(18_000_000, 1200) }
  fn permissionless_sweep_many(batch: u32) -> Weight {
    let bounded = u64::from(batch.min(3));
    Weight::from_parts(
      12_000_000u64.saturating_add(18_000_000u64.saturating_mul(bounded)),
      1200u64.saturating_add(384u64.saturating_mul(bounded)),
    )
  }
  fn maximum_context_inherent() -> Weight { Weight::MAX }
  fn maximum_xcm_version_discovery() -> Weight { Weight::MAX }
  fn block_resource_meter_extension() -> Weight { Weight::MAX }
  fn block_resource_finalize() -> Weight { Weight::from_parts(10_000_000, 1_000) }
}
