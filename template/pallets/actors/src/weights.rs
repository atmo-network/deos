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
  fn manual_observation_park() -> Weight;
  fn address_event_trigger_occurrence() -> Weight;
  fn observation_change_trigger_occurrence() -> Weight;
  fn observation_crossing_trigger_occurrence() -> Weight;
  fn at_time_trigger_occurrence() -> Weight;
  fn cadenced_trigger_occurrence() -> Weight;
  fn cadenced_running_rearm() -> Weight { Self::cadenced_trigger_occurrence() }
  fn cadenced_suspended_service_rearm() -> Weight { Self::cadenced_trigger_occurrence() }
  fn cadenced_suspended_deadline_rearm() -> Weight { Self::cadenced_trigger_occurrence() }
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
  fn process_due_observation_availability_review() -> Weight;
  fn process_due_observation_availability_review_deep_index() -> Weight {
    Self::process_due_observation_availability_review()
  }
  fn process_due_parked_balance_review() -> Weight;
  fn process_due_parked_balance_review_deep_index() -> Weight {
    Self::process_due_parked_balance_review()
  }
  fn complete_cycle_to_parked_balance() -> Weight;
  fn process_pending_parked_balance_event() -> Weight;
  fn process_pending_observation_availability_event() -> Weight;
  fn process_pending_observation_predicate_event() -> Weight;
  fn process_due_observation_predicate_review() -> Weight;
  fn process_due_observation_predicate_review_deep_index() -> Weight {
    Self::process_due_observation_predicate_review()
  }
  fn dependency_scan_source_probe() -> Weight;
  fn process_dependency_scan_unit() -> Weight;
  fn process_dependency_scan_completion_unit() -> Weight;
  fn classify_due_block_deadline() -> Weight;
  fn classify_due_tick_deadline() -> Weight;
  fn deadline_destination_search(p: u32) -> Weight;
  fn return_due_block_deadline_to_service() -> Weight;
  fn return_due_block_deadline_to_service_deep_index() -> Weight;
  fn crossing_worker_base() -> Weight { Weight::from_parts(25_000_000, 8_000) }
  fn crossing_work_probe() -> Weight { Weight::from_parts(400_000_000, 20_000) }
  fn crossing_selection_probe() -> Weight { Weight::from_parts(50_000_000, 0) }
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
  fn action_invocation_receipt() -> Weight;
  fn predicate_set_evaluation(predicates: u32) -> Weight;
  fn predicate_asset_evaluation(predicates: u32) -> Weight;
  fn predicate_observation_heavy_evaluation(observations: u32) -> Weight;
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
  fn scheduler_on_initialize_cutoff() -> Weight;
  fn scheduler_on_idle_base() -> Weight;
  fn materialization_coordinator_base() -> Weight;
  fn service_member_publish_empty() -> Weight;
  fn service_member_publish_populated() -> Weight;
  fn service_member_retire_singleton() -> Weight;
  fn service_member_retire_pair_cursor() -> Weight;
  fn service_member_retire_interior() -> Weight;
  fn service_member_insert_populated() -> Weight;
  fn service_round_begin_populated() -> Weight;
  fn service_round_probe_eligible() -> Weight;
  fn service_round_admit_eligible() -> Weight;
  fn scheduler_service_successful_interior() -> Weight;
  fn scheduler_service_retry_to_deadline() -> Weight;
  fn scheduler_service_retry_to_deadline_new_key() -> Weight;
  fn scheduler_due_deadline_to_service() -> Weight;
  fn scheduler_due_deadline_to_service_deep_index() -> Weight;
  fn scheduler_service_late_refusal_rollback() -> Weight;
  fn scheduler_service_terminal_retain_close() -> Weight;
  fn scheduler_service_minimal_apoptosis() -> Weight;
  fn dependency_publication_begun_empty_source_list() -> Weight;
  fn dependency_publication_begun_populated_source_list() -> Weight;
  fn dependency_publication_coalesced_active_source() -> Weight;
  fn service_member_to_deadline_new_key() -> Weight;
  fn scheduler_inner_zero_step_complete() -> Weight;
  /// Complete state-preserving FIFO refusal for a paid zero-Step User Crossing whose current
  /// observation is unavailable. The conservative fallback exists only until the host regenerates
  /// this benchmark-owned method with its complete production Weight artifact.
  fn scheduler_paged_zero_step_user_crossing_unavailable() -> Weight {
    Self::scheduler_paged_execute_opening_max()
  }
  fn scheduler_paged_execute_opening_max() -> Weight;
  fn scheduler_inner_opening_failed_min(tail_chunks: u32) -> Weight;
  fn scheduler_inner_opening_failed_max(tail_chunks: u32) -> Weight;
  fn scheduler_inner_opening_retry_min(tail_chunks: u32) -> Weight;
  fn scheduler_inner_opening_retry_max(tail_chunks: u32) -> Weight;
  fn scheduler_inner_opening_complete_min(tail_chunks: u32) -> Weight;
  fn scheduler_inner_opening_complete_max(tail_chunks: u32) -> Weight;
  /// Complete zero-tail User Opening with the maximum admitted funding-policy header.
  fn scheduler_inner_opening_user_complete_header_max() -> Weight {
    Self::scheduler_inner_opening_complete_max(0)
  }
  /// Complete positive-tail User Opening with the maximum admitted funding-policy header.
  fn scheduler_inner_opening_user_complete_header_max_tail(tail_chunks: u32) -> Weight {
    Self::scheduler_inner_opening_complete_max(tail_chunks)
  }
  fn scheduler_inner_opening_progress_min(tail_chunks: u32) -> Weight;
  fn scheduler_inner_opening_progress_max(tail_chunks: u32) -> Weight;
  fn scheduler_inner_running_complete(steps_in_fragment: u32, predicates: u32) -> Weight;
  fn scheduler_inner_running_progress(steps_in_fragment: u32, predicates: u32) -> Weight;
  fn scheduler_inner_suspended_tail_retry(steps_in_fragment: u32, predicates: u32) -> Weight;
  fn scheduler_inner_suspended_tail_complete(steps_in_fragment: u32, predicates: u32) -> Weight;
  fn scheduler_inner_suspended_tail_progress(steps_in_fragment: u32, predicates: u32) -> Weight;
  fn scheduler_inner_suspended_head_retry(
    tail_opening_amount_entries: u32,
    current_predicates: u32,
  ) -> Weight;
  fn scheduler_inner_suspended_head_complete(current_predicates: u32) -> Weight;
  fn scheduler_inner_suspended_head_progress(
    tail_opening_amount_entries: u32,
    current_predicates: u32,
  ) -> Weight;
  fn scheduler_inner_suspended_head_opening_retry(
    tail_opening_amount_entries: u32,
    current_predicates: u32,
  ) -> Weight;
  fn scheduler_inner_suspended_head_opening_complete(current_predicates: u32) -> Weight;
  fn scheduler_inner_suspended_head_opening_progress(
    tail_opening_amount_entries: u32,
    current_predicates: u32,
  ) -> Weight;
  fn scheduler_actor_state_probe() -> Weight;
  fn transaction_extension_ingress_base() -> Weight;
  fn transaction_extension_ingress_notify() -> Weight;
  fn run_progress() -> Weight;
  fn run_suspend() -> Weight;
  fn run_complete() -> Weight;
  fn run_cancel() -> Weight;
  fn update_contract() -> Weight;
  fn set_global_circuit_breaker() -> Weight;
  fn clear_crossing_worker_fault() -> Weight;
  fn clear_observation_fanout_worker_fault() -> Weight;
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

  fn manual_observation_park() -> Weight {
    Self::manual_trigger().saturating_add(Self::complete_cycle_to_parked_balance())
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

  fn process_due_observation_availability_review() -> Weight {
    Weight::from_parts(1_500_000_000, 400_000)
      .saturating_add(T::DbWeight::get().reads_writes(24, 20))
  }

  fn process_due_parked_balance_review() -> Weight {
    Weight::from_parts(740_190_000, 43_950)
      .saturating_add(T::DbWeight::get().reads(118))
      .saturating_add(T::DbWeight::get().writes(13))
  }

  fn complete_cycle_to_parked_balance() -> Weight {
    Weight::from_parts(680_824_000, 43_950)
      .saturating_add(T::DbWeight::get().reads(128))
      .saturating_add(T::DbWeight::get().writes(86))
  }

  fn process_pending_parked_balance_event() -> Weight {
    Weight::from_parts(667_554_000, 43_950)
      .saturating_add(T::DbWeight::get().reads(92))
      .saturating_add(T::DbWeight::get().writes(74))
  }

  fn process_pending_observation_availability_event() -> Weight {
    Weight::from_parts(204_638_000, 4_570)
      .saturating_add(T::DbWeight::get().reads(28))
      .saturating_add(T::DbWeight::get().writes(19))
  }

  fn process_pending_observation_predicate_event() -> Weight {
    Self::process_pending_observation_availability_event()
  }

  fn process_due_observation_predicate_review() -> Weight {
    Self::process_due_observation_availability_review()
  }

  fn dependency_scan_source_probe() -> Weight {
    Weight::from_parts(6_774_000, 1_498)
      .saturating_add(T::DbWeight::get().reads(1))
  }

  fn process_dependency_scan_unit() -> Weight {
    Weight::from_parts(63_417_000, 4_570)
      .saturating_add(T::DbWeight::get().reads(11))
      .saturating_add(T::DbWeight::get().writes(6))
  }

  fn process_dependency_scan_completion_unit() -> Weight {
    Weight::from_parts(23_886_000, 3_523)
      .saturating_add(T::DbWeight::get().reads(3))
      .saturating_add(T::DbWeight::get().writes(3))
  }

  fn classify_due_block_deadline() -> Weight {
    Weight::from_parts(100_000_000, 100_000)
      .saturating_add(T::DbWeight::get().reads(6))
  }

  fn classify_due_tick_deadline() -> Weight {
    Weight::from_parts(100_000_000, 100_000)
      .saturating_add(T::DbWeight::get().reads(6))
  }

  fn deadline_destination_search(p: u32) -> Weight {
    Weight::from_parts(19_365_297, 5_074)
      .saturating_add(Weight::from_parts(20_716, 2).saturating_mul(p.into()))
      .saturating_add(T::DbWeight::get().reads(2))
  }

  fn return_due_block_deadline_to_service() -> Weight {
    Weight::from_parts(146_041_000, 13_414)
      .saturating_add(T::DbWeight::get().reads_writes(16, 9))
  }

  fn return_due_block_deadline_to_service_deep_index() -> Weight {
    Weight::from_parts(981_635_000, 51_480)
      .saturating_add(T::DbWeight::get().reads_writes(44, 32))
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

  fn action_invocation_receipt() -> Weight {
    Weight::from_parts(10_000_000, 0)
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

  fn predicate_asset_evaluation(predicates: u32) -> Weight {
    Self::predicate_set_evaluation(predicates)
  }

  fn predicate_observation_heavy_evaluation(observations: u32) -> Weight {
    Self::predicate_set_evaluation(observations.saturating_add(1))
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

  fn service_member_publish_empty() -> Weight {
    Weight::from_parts(150_000_000, 24_000).saturating_add(T::DbWeight::get().reads_writes(6, 3))
  }
  fn service_member_publish_populated() -> Weight {
    Weight::from_parts(200_000_000, 32_000).saturating_add(T::DbWeight::get().reads_writes(10, 6))
  }
  fn service_member_retire_singleton() -> Weight {
    Weight::from_parts(34_851_000, 3_948).saturating_add(T::DbWeight::get().reads_writes(5, 3))
  }
  fn service_member_retire_pair_cursor() -> Weight {
    Weight::from_parts(42_394_000, 6_086).saturating_add(T::DbWeight::get().reads_writes(6, 4))
  }
  fn service_member_retire_interior() -> Weight {
    Weight::from_parts(46_724_000, 8_634).saturating_add(T::DbWeight::get().reads_writes(7, 5))
  }
  fn service_member_insert_populated() -> Weight {
    Weight::from_parts(150_000_000, 24_000).saturating_add(T::DbWeight::get().reads_writes(9, 5))
  }

  fn service_round_begin_populated() -> Weight {
    Weight::from_parts(50_000_000, 8_000).saturating_add(T::DbWeight::get().reads_writes(1, 1))
  }

  fn service_round_probe_eligible() -> Weight {
    Weight::from_parts(75_000_000, 12_000).saturating_add(T::DbWeight::get().reads(3))
  }

  fn service_round_admit_eligible() -> Weight {
    Weight::from_parts(125_000_000, 16_000).saturating_add(T::DbWeight::get().reads_writes(6, 3))
  }

  /// Storage: `Actors::ActorReadyTail` (r:1 w:0)
  /// Proof: `Actors::ActorReadyTail` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `Measured`)
  /// Storage: `Actors::ServiceHeader` (r:1 w:1)
  /// Proof: `Actors::ServiceHeader` (`max_values`: Some(1), `max_size`: Some(26), added: 521, mode: `Measured`)
  /// Storage: `Actors::ServiceNodes` (r:1 w:1)
  /// Proof: `Actors::ServiceNodes` (`max_values`: None, `max_size`: Some(73), added: 2548, mode: `Measured`)
  /// Storage: `Actors::ActorProcesses` (r:1 w:1)
  /// Proof: `Actors::ActorProcesses` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `Measured`)
  /// Storage: `Actors::ActorControlLocators` (r:1 w:0)
  /// Proof: `Actors::ActorControlLocators` (`max_values`: None, `max_size`: Some(43), added: 2518, mode: `Measured`)
  /// Storage: `Actors::ActorUnsignaledControlCells` (r:1 w:0)
  /// Proof: `Actors::ActorUnsignaledControlCells` (`max_values`: None, `max_size`: Some(483), added: 2958, mode: `Measured`)
  /// Storage: `Actors::ActorSemanticStates` (r:1 w:1)
  /// Proof: `Actors::ActorSemanticStates` (`max_values`: None, `max_size`: Some(452), added: 2927, mode: `Measured`)
  /// Storage: `Actors::DeadlineHandles` (r:1 w:0)
  /// Proof: `Actors::DeadlineHandles` (`max_values`: None, `max_size`: Some(58), added: 2533, mode: `Measured`)
  /// Storage: `Actors::PendingCheckOwners` (r:1 w:0)
  /// Proof: `Actors::PendingCheckOwners` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `Measured`)
  /// Storage: `Actors::ActorContractHead` (r:1 w:0)
  /// Proof: `Actors::ActorContractHead` (`max_values`: None, `max_size`: Some(2255), added: 4730, mode: `Measured`)
  /// Storage: `Actors::ActorRunHead` (r:1 w:1)
  /// Proof: `Actors::ActorRunHead` (`max_values`: None, `max_size`: Some(248), added: 2723, mode: `Measured`)
  /// Storage: `Actors::ActorRunPayload` (r:1 w:0)
  /// Proof: `Actors::ActorRunPayload` (`max_values`: None, `max_size`: Some(553), added: 3028, mode: `Measured`)
  /// Storage: `Actors::ActorContractTailChunk` (r:1 w:0)
  /// Proof: `Actors::ActorContractTailChunk` (`max_values`: None, `max_size`: Some(4006), added: 6481, mode: `Measured`)
  /// Storage: `Actors::GlobalCircuitBreaker` (r:1 w:0)
  /// Proof: `Actors::GlobalCircuitBreaker` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `Measured`)
  /// Storage: `Assets::Asset` (r:1 w:0)
  /// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(210), added: 2685, mode: `Measured`)
  /// Storage: `Assets::Account` (r:1 w:0)
  /// Proof: `Assets::Account` (`max_values`: None, `max_size`: Some(134), added: 2609, mode: `Measured`)
  fn scheduler_service_successful_interior() -> Weight {
    // Proof Size summary in bytes:
    //  Measured:  `3757`
    //  Estimated: `7222`
    // Minimum execution time: 177_120_000 picoseconds.
    Weight::from_parts(184_664_000, 0)
      .saturating_add(Weight::from_parts(0, 7222))
      .saturating_add(T::DbWeight::get().reads(16))
      .saturating_add(T::DbWeight::get().writes(5))
  }
  /// Storage: `Actors::ActorReadyTail` (r:1 w:0)
  /// Proof: `Actors::ActorReadyTail` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `Measured`)
  /// Storage: `Actors::ServiceHeader` (r:1 w:1)
  /// Proof: `Actors::ServiceHeader` (`max_values`: Some(1), `max_size`: Some(26), added: 521, mode: `Measured`)
  /// Storage: `Actors::ServiceNodes` (r:1 w:1)
  /// Proof: `Actors::ServiceNodes` (`max_values`: None, `max_size`: Some(73), added: 2548, mode: `Measured`)
  /// Storage: `Actors::ActorProcesses` (r:1 w:1)
  /// Proof: `Actors::ActorProcesses` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `Measured`)
  /// Storage: `Actors::ActorControlLocators` (r:1 w:0)
  /// Proof: `Actors::ActorControlLocators` (`max_values`: None, `max_size`: Some(43), added: 2518, mode: `Measured`)
  /// Storage: `Actors::ActorUnsignaledControlCells` (r:1 w:0)
  /// Proof: `Actors::ActorUnsignaledControlCells` (`max_values`: None, `max_size`: Some(483), added: 2958, mode: `Measured`)
  /// Storage: `Actors::ActorSemanticStates` (r:1 w:1)
  /// Proof: `Actors::ActorSemanticStates` (`max_values`: None, `max_size`: Some(452), added: 2927, mode: `Measured`)
  /// Storage: `Actors::DeadlineHandles` (r:1 w:1)
  /// Proof: `Actors::DeadlineHandles` (`max_values`: None, `max_size`: Some(58), added: 2533, mode: `Measured`)
  /// Storage: `Actors::PendingCheckOwners` (r:1 w:0)
  /// Proof: `Actors::PendingCheckOwners` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `Measured`)
  /// Storage: `Actors::ActorContractHead` (r:1 w:0)
  /// Proof: `Actors::ActorContractHead` (`max_values`: None, `max_size`: Some(2255), added: 4730, mode: `Measured`)
  /// Storage: `Actors::ActorRunHead` (r:1 w:1)
  /// Proof: `Actors::ActorRunHead` (`max_values`: None, `max_size`: Some(248), added: 2723, mode: `Measured`)
  /// Storage: `Actors::GlobalCircuitBreaker` (r:1 w:0)
  /// Proof: `Actors::GlobalCircuitBreaker` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `Measured`)
  /// Storage: `Actors::DeadlineHeaders` (r:1 w:1)
  /// Proof: `Actors::DeadlineHeaders` (`max_values`: None, `max_size`: Some(66), added: 2541, mode: `Measured`)
  /// Storage: `Actors::DeadlinePages` (r:2 w:2)
  /// Proof: `Actors::DeadlinePages` (`max_values`: None, `max_size`: Some(631), added: 3106, mode: `Measured`)
  /// Storage: `Actors::ActorWaitingOccupancies` (r:1 w:0)
  /// Proof: `Actors::ActorWaitingOccupancies` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `Measured`)
  /// Storage: `Actors::ActorWaitingCursorIndices` (r:1 w:0)
  /// Proof: `Actors::ActorWaitingCursorIndices` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `Measured`)
  /// Storage: `Actors::DeadlineIndexPositions` (r:1 w:0)
  /// Proof: `Actors::DeadlineIndexPositions` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `Measured`)
  /// Storage: `Actors::DeadlineIndexLen` (r:1 w:0)
  /// Proof: `Actors::DeadlineIndexLen` (`max_values`: None, `max_size`: Some(21), added: 2496, mode: `Measured`)
  /// Storage: `Actors::DeadlineIndexPages` (r:1 w:0)
  /// Proof: `Actors::DeadlineIndexPages` (`max_values`: None, `max_size`: Some(330), added: 2805, mode: `Measured`)
  /// Storage: `Actors::ActorRunPayload` (r:1 w:1)
  /// Proof: `Actors::ActorRunPayload` (`max_values`: None, `max_size`: Some(553), added: 3028, mode: `Measured`)
  /// Storage: `Assets::Asset` (r:1 w:0)
  /// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(210), added: 2685, mode: `Measured`)
  /// Storage: `Assets::Account` (r:1 w:0)
  /// Proof: `Assets::Account` (`max_values`: None, `max_size`: Some(134), added: 2609, mode: `Measured`)
  fn scheduler_service_retry_to_deadline() -> Weight {
    Weight::from_parts(376_242_000, 11_293)
      .saturating_add(T::DbWeight::get().reads_writes(23, 10))
  }

  fn scheduler_service_retry_to_deadline_new_key() -> Weight {
    Weight::from_parts(1_886_095_000, 63_893)
      .saturating_add(T::DbWeight::get().reads_writes(44, 34))
  }
  /// Storage: `Actors::DeadlineIndexPages` (r:2 w:1)
  /// Proof: `Actors::DeadlineIndexPages` (`max_values`: None, `max_size`: Some(330), added: 2805, mode: `Measured`)
  /// Storage: `Actors::DeadlineHeaders` (r:3 w:3)
  /// Proof: `Actors::DeadlineHeaders` (`max_values`: None, `max_size`: Some(66), added: 2541, mode: `Measured`)
  /// Storage: `Actors::DeadlinePages` (r:6 w:6)
  /// Proof: `Actors::DeadlinePages` (`max_values`: None, `max_size`: Some(631), added: 3106, mode: `Measured`)
  /// Storage: `Actors::ActorProcesses` (r:2 w:1)
  /// Proof: `Actors::ActorProcesses` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `Measured`)
  /// Storage: `Actors::DeadlineHandles` (r:1 w:1)
  /// Proof: `Actors::DeadlineHandles` (`max_values`: None, `max_size`: Some(58), added: 2533, mode: `Measured`)
  /// Storage: `Actors::ActorControlLocators` (r:2 w:0)
  /// Proof: `Actors::ActorControlLocators` (`max_values`: None, `max_size`: Some(43), added: 2518, mode: `Measured`)
  /// Storage: `Actors::ActorUnsignaledControlCells` (r:2 w:0)
  /// Proof: `Actors::ActorUnsignaledControlCells` (`max_values`: None, `max_size`: Some(483), added: 2958, mode: `Measured`)
  /// Storage: `Actors::ActorWaitingOccupancies` (r:3 w:0)
  /// Proof: `Actors::ActorWaitingOccupancies` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `Measured`)
  /// Storage: `Actors::ActorWaitingCursorIndices` (r:3 w:0)
  /// Proof: `Actors::ActorWaitingCursorIndices` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `Measured`)
  /// Storage: `Actors::DeadlineIndexPositions` (r:3 w:2)
  /// Proof: `Actors::DeadlineIndexPositions` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `Measured`)
  /// Storage: `Actors::DeadlineIndexLen` (r:2 w:1)
  /// Proof: `Actors::DeadlineIndexLen` (`max_values`: None, `max_size`: Some(21), added: 2496, mode: `Measured`)
  /// Storage: `Actors::ServiceNodes` (r:1 w:1)
  /// Proof: `Actors::ServiceNodes` (`max_values`: None, `max_size`: Some(73), added: 2548, mode: `Measured`)
  /// Storage: `Actors::ServiceHeader` (r:1 w:1)
  /// Proof: `Actors::ServiceHeader` (`max_values`: Some(1), `max_size`: Some(26), added: 521, mode: `Measured`)
  /// Storage: `Actors::TriggerDeadlineHandles` (r:1 w:1)
  /// Proof: `Actors::TriggerDeadlineHandles` (`max_values`: None, `max_size`: Some(58), added: 2533, mode: `Measured`)
  /// Storage: `Actors::ActorSemanticStates` (r:1 w:1)
  /// Proof: `Actors::ActorSemanticStates` (`max_values`: None, `max_size`: Some(452), added: 2927, mode: `Measured`)
  /// Storage: `Actors::ActorContractHead` (r:1 w:0)
  /// Proof: `Actors::ActorContractHead` (`max_values`: None, `max_size`: Some(2255), added: 4730, mode: `Measured`)
  /// Storage: `Actors::ActorRunHead` (r:1 w:0)
  /// Proof: `Actors::ActorRunHead` (`max_values`: None, `max_size`: Some(248), added: 2723, mode: `Measured`)
  /// Storage: `Timestamp::Now` (r:1 w:0)
  /// Proof: `Timestamp::Now` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `Measured`)
  fn scheduler_due_deadline_to_service() -> Weight {
    Weight::from_parts(368_908_000, 22_650)
      .saturating_add(T::DbWeight::get().reads_writes(36, 19))
  }

  fn scheduler_due_deadline_to_service_deep_index() -> Weight {
    Weight::from_parts(1_403_273_000, 88_173)
      .saturating_add(T::DbWeight::get().reads_writes(64, 42))
  }
  /// Storage: `Actors::ServiceHeader` (r:1 w:0)
  /// Proof: `Actors::ServiceHeader` (`max_values`: Some(1), `max_size`: Some(26), added: 521, mode: `Measured`)
  /// Storage: `Actors::ServiceNodes` (r:1 w:0)
  /// Proof: `Actors::ServiceNodes` (`max_values`: None, `max_size`: Some(73), added: 2548, mode: `Measured`)
  /// Storage: `Actors::ActorProcesses` (r:1 w:0)
  /// Proof: `Actors::ActorProcesses` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `Measured`)
  /// Storage: `Actors::ActorControlLocators` (r:1 w:0)
  /// Proof: `Actors::ActorControlLocators` (`max_values`: None, `max_size`: Some(43), added: 2518, mode: `Measured`)
  /// Storage: `Actors::ActorUnsignaledControlCells` (r:1 w:0)
  /// Proof: `Actors::ActorUnsignaledControlCells` (`max_values`: None, `max_size`: Some(483), added: 2958, mode: `Measured`)
  /// Storage: `Actors::ActorSemanticStates` (r:1 w:0)
  /// Proof: `Actors::ActorSemanticStates` (`max_values`: None, `max_size`: Some(452), added: 2927, mode: `Measured`)
  /// Storage: `Actors::DeadlineHandles` (r:1 w:0)
  /// Proof: `Actors::DeadlineHandles` (`max_values`: None, `max_size`: Some(58), added: 2533, mode: `Measured`)
  /// Storage: `Actors::PendingCheckOwners` (r:1 w:0)
  /// Proof: `Actors::PendingCheckOwners` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `Measured`)
  /// Storage: `Actors::ActorContractHead` (r:1 w:0)
  /// Proof: `Actors::ActorContractHead` (`max_values`: None, `max_size`: Some(2255), added: 4730, mode: `Measured`)
  /// Storage: `Actors::ActorRunHead` (r:1 w:0)
  /// Proof: `Actors::ActorRunHead` (`max_values`: None, `max_size`: Some(248), added: 2723, mode: `Measured`)
  /// Storage: `Actors::ActorRunPayload` (r:1 w:0)
  /// Proof: `Actors::ActorRunPayload` (`max_values`: None, `max_size`: Some(553), added: 3028, mode: `Measured`)
  /// Storage: `Actors::ActorContractTailChunk` (r:1 w:0)
  /// Proof: `Actors::ActorContractTailChunk` (`max_values`: None, `max_size`: Some(4006), added: 6481, mode: `Measured`)
  /// Storage: `Actors::GlobalCircuitBreaker` (r:1 w:0)
  /// Proof: `Actors::GlobalCircuitBreaker` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `Measured`)
  fn scheduler_service_late_refusal_rollback() -> Weight {
    // Proof Size summary in bytes:
    //  Measured:  `2714`
    //  Estimated: `6179`
    // Minimum execution time: 76_478_000 picoseconds.
    Weight::from_parts(88_141_000, 0)
      .saturating_add(Weight::from_parts(0, 6179))
      .saturating_add(T::DbWeight::get().reads(13))
  }
  /// Storage: `Actors::ActorReadyTail` (r:1 w:0)
  /// Proof: `Actors::ActorReadyTail` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `Measured`)
  /// Storage: `Actors::ServiceHeader` (r:1 w:1)
  /// Proof: `Actors::ServiceHeader` (`max_values`: Some(1), `max_size`: Some(26), added: 521, mode: `Measured`)
  /// Storage: `Actors::ServiceNodes` (r:1 w:1)
  /// Proof: `Actors::ServiceNodes` (`max_values`: None, `max_size`: Some(73), added: 2548, mode: `Measured`)
  /// Storage: `Actors::ActorProcesses` (r:1 w:1)
  /// Proof: `Actors::ActorProcesses` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `Measured`)
  /// Storage: `Actors::ActorControlLocators` (r:1 w:0)
  /// Proof: `Actors::ActorControlLocators` (`max_values`: None, `max_size`: Some(43), added: 2518, mode: `Measured`)
  /// Storage: `Actors::ActorUnsignaledControlCells` (r:1 w:0)
  /// Proof: `Actors::ActorUnsignaledControlCells` (`max_values`: None, `max_size`: Some(483), added: 2958, mode: `Measured`)
  /// Storage: `Actors::ActorSemanticStates` (r:1 w:1)
  /// Proof: `Actors::ActorSemanticStates` (`max_values`: None, `max_size`: Some(452), added: 2927, mode: `Measured`)
  /// Storage: `Actors::DeadlineHandles` (r:1 w:0)
  /// Proof: `Actors::DeadlineHandles` (`max_values`: None, `max_size`: Some(58), added: 2533, mode: `Measured`)
  /// Storage: `Actors::PendingCheckOwners` (r:1 w:0)
  /// Proof: `Actors::PendingCheckOwners` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `Measured`)
  /// Storage: `Actors::ActorContractHead` (r:1 w:1)
  /// Proof: `Actors::ActorContractHead` (`max_values`: None, `max_size`: Some(2255), added: 4730, mode: `Measured`)
  /// Storage: `Actors::ActorRunHead` (r:1 w:1)
  /// Proof: `Actors::ActorRunHead` (`max_values`: None, `max_size`: Some(248), added: 2723, mode: `Measured`)
  /// Storage: `Actors::GlobalCircuitBreaker` (r:1 w:0)
  /// Proof: `Actors::GlobalCircuitBreaker` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `Measured`)
  /// Storage: `Actors::ActorIdentities` (r:1 w:0)
  /// Proof: `Actors::ActorIdentities` (`max_values`: None, `max_size`: Some(110), added: 2585, mode: `Measured`)
  /// Storage: `Actors::ActorRunPayload` (r:1 w:1)
  /// Proof: `Actors::ActorRunPayload` (`max_values`: None, `max_size`: Some(553), added: 3028, mode: `Measured`)
  /// Storage: `Actors::TriggerDeadlineHandles` (r:1 w:0)
  /// Proof: `Actors::TriggerDeadlineHandles` (`max_values`: None, `max_size`: Some(58), added: 2533, mode: `Measured`)
  /// Storage: `Actors::ActiveActorCount` (r:1 w:1)
  /// Proof: `Actors::ActiveActorCount` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `Measured`)
  /// Storage: `Actors::ActorIdentityCount` (r:1 w:1)
  /// Proof: `Actors::ActorIdentityCount` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `Measured`)
  /// Storage: `Actors::SovereignIndex` (r:1 w:1)
  /// Proof: `Actors::SovereignIndex` (`max_values`: None, `max_size`: Some(56), added: 2531, mode: `Measured`)
  /// Storage: `Actors::OwnerSlotBitmaps` (r:1 w:1)
  /// Proof: `Actors::OwnerSlotBitmaps` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `Measured`)
  /// Storage: `Actors::ActorObservationFeeds` (r:1 w:0)
  /// Proof: `Actors::ActorObservationFeeds` (`max_values`: None, `max_size`: Some(42), added: 2517, mode: `Measured`)
  /// Storage: `Actors::ObservationSubscriptionSlot` (r:1 w:0)
  /// Proof: `Actors::ObservationSubscriptionSlot` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `Measured`)
  /// Storage: `Actors::CrossingMemberships` (r:1 w:0)
  /// Proof: `Actors::CrossingMemberships` (`max_values`: None, `max_size`: Some(74), added: 2549, mode: `Measured`)
  /// Storage: `Actors::ActorStateHolds` (r:1 w:1)
  /// Proof: `Actors::ActorStateHolds` (`max_values`: None, `max_size`: Some(136), added: 2611, mode: `Measured`)
  /// Storage: `System::Account` (r:1 w:1)
  /// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `Measured`)
  /// Storage: `Balances::Holds` (r:1 w:1)
  /// Proof: `Balances::Holds` (`max_values`: None, `max_size`: Some(121), added: 2596, mode: `Measured`)
  /// Storage: `Actors::ActorActivationAuthority` (r:0 w:1)
  /// Proof: `Actors::ActorActivationAuthority` (`max_values`: None, `max_size`: Some(159), added: 2634, mode: `Measured`)
  /// Storage: `Actors::IndexedTriggerDetectionDisabled` (r:0 w:1)
  /// Proof: `Actors::IndexedTriggerDetectionDisabled` (`max_values`: None, `max_size`: Some(24), added: 2499, mode: `Measured`)
  fn scheduler_service_terminal_retain_close() -> Weight {
    // Proof Size summary in bytes:
    //  Measured:  `2837`
    //  Estimated: `6302`
    // Minimum execution time: 258_626_000 picoseconds.
    Weight::from_parts(278_880_000, 0)
      .saturating_add(Weight::from_parts(0, 6302))
      .saturating_add(T::DbWeight::get().reads(25))
      .saturating_add(T::DbWeight::get().writes(16))
  }
  /// Storage: `Actors::ActorReadyTail` (r:1 w:0)
  /// Proof: `Actors::ActorReadyTail` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `Measured`)
  /// Storage: `Actors::ServiceHeader` (r:1 w:1)
  /// Proof: `Actors::ServiceHeader` (`max_values`: Some(1), `max_size`: Some(26), added: 521, mode: `Measured`)
  /// Storage: `Actors::ServiceNodes` (r:1 w:1)
  /// Proof: `Actors::ServiceNodes` (`max_values`: None, `max_size`: Some(73), added: 2548, mode: `Measured`)
  /// Storage: `Actors::ActorProcesses` (r:1 w:1)
  /// Proof: `Actors::ActorProcesses` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `Measured`)
  /// Storage: `Actors::ActorControlLocators` (r:1 w:0)
  /// Proof: `Actors::ActorControlLocators` (`max_values`: None, `max_size`: Some(43), added: 2518, mode: `Measured`)
  /// Storage: `Actors::ActorUnsignaledControlCells` (r:1 w:0)
  /// Proof: `Actors::ActorUnsignaledControlCells` (`max_values`: None, `max_size`: Some(483), added: 2958, mode: `Measured`)
  /// Storage: `Actors::ActorSemanticStates` (r:1 w:1)
  /// Proof: `Actors::ActorSemanticStates` (`max_values`: None, `max_size`: Some(452), added: 2927, mode: `Measured`)
  /// Storage: `Actors::DeadlineHandles` (r:1 w:0)
  /// Proof: `Actors::DeadlineHandles` (`max_values`: None, `max_size`: Some(58), added: 2533, mode: `Measured`)
  /// Storage: `Actors::PendingCheckOwners` (r:1 w:0)
  /// Proof: `Actors::PendingCheckOwners` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `Measured`)
  /// Storage: `Actors::ActorContractHead` (r:1 w:1)
  /// Proof: `Actors::ActorContractHead` (`max_values`: None, `max_size`: Some(2255), added: 4730, mode: `Measured`)
  /// Storage: `Actors::ActorRunHead` (r:1 w:1)
  /// Proof: `Actors::ActorRunHead` (`max_values`: None, `max_size`: Some(248), added: 2723, mode: `Measured`)
  /// Storage: `Actors::GlobalCircuitBreaker` (r:1 w:0)
  /// Proof: `Actors::GlobalCircuitBreaker` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `Measured`)
  /// Storage: `System::Account` (r:2 w:1)
  /// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `Measured`)
  /// Storage: `Actors::ActorIdentities` (r:1 w:0)
  /// Proof: `Actors::ActorIdentities` (`max_values`: None, `max_size`: Some(110), added: 2585, mode: `Measured`)
  /// Storage: `Actors::ActorRunPayload` (r:1 w:1)
  /// Proof: `Actors::ActorRunPayload` (`max_values`: None, `max_size`: Some(553), added: 3028, mode: `Measured`)
  /// Storage: `Actors::TriggerDeadlineHandles` (r:1 w:0)
  /// Proof: `Actors::TriggerDeadlineHandles` (`max_values`: None, `max_size`: Some(58), added: 2533, mode: `Measured`)
  /// Storage: `Actors::ActiveActorCount` (r:1 w:1)
  /// Proof: `Actors::ActiveActorCount` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `Measured`)
  /// Storage: `Actors::ActorIdentityCount` (r:1 w:1)
  /// Proof: `Actors::ActorIdentityCount` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `Measured`)
  /// Storage: `Actors::SovereignIndex` (r:1 w:1)
  /// Proof: `Actors::SovereignIndex` (`max_values`: None, `max_size`: Some(56), added: 2531, mode: `Measured`)
  /// Storage: `Actors::OwnerSlotBitmaps` (r:1 w:1)
  /// Proof: `Actors::OwnerSlotBitmaps` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `Measured`)
  /// Storage: `Actors::ActorObservationFeeds` (r:1 w:0)
  /// Proof: `Actors::ActorObservationFeeds` (`max_values`: None, `max_size`: Some(42), added: 2517, mode: `Measured`)
  /// Storage: `Actors::ObservationSubscriptionSlot` (r:1 w:0)
  /// Proof: `Actors::ObservationSubscriptionSlot` (`max_values`: None, `max_size`: Some(28), added: 2503, mode: `Measured`)
  /// Storage: `Actors::CrossingMemberships` (r:1 w:0)
  /// Proof: `Actors::CrossingMemberships` (`max_values`: None, `max_size`: Some(74), added: 2549, mode: `Measured`)
  /// Storage: `Actors::ActorStateHolds` (r:1 w:1)
  /// Proof: `Actors::ActorStateHolds` (`max_values`: None, `max_size`: Some(136), added: 2611, mode: `Measured`)
  /// Storage: `Balances::Holds` (r:1 w:1)
  /// Proof: `Balances::Holds` (`max_values`: None, `max_size`: Some(121), added: 2596, mode: `Measured`)
  /// Storage: `Actors::ActorActivationAuthority` (r:0 w:1)
  /// Proof: `Actors::ActorActivationAuthority` (`max_values`: None, `max_size`: Some(159), added: 2634, mode: `Measured`)
  /// Storage: `Actors::IndexedTriggerDetectionDisabled` (r:0 w:1)
  /// Proof: `Actors::IndexedTriggerDetectionDisabled` (`max_values`: None, `max_size`: Some(24), added: 2499, mode: `Measured`)
  fn scheduler_service_minimal_apoptosis() -> Weight {
    // Proof Size summary in bytes:
    //  Measured:  `3132`
    //  Estimated: `9072`
    // Minimum execution time: 267_147_000 picoseconds.
    Weight::from_parts(288_170_000, 0)
      .saturating_add(Weight::from_parts(0, 9072))
      .saturating_add(T::DbWeight::get().reads(26))
      .saturating_add(T::DbWeight::get().writes(16))
  }
  fn dependency_publication_begun_empty_source_list() -> Weight {
    Weight::from_parts(100_000_000, 16_000).saturating_add(T::DbWeight::get().reads_writes(5, 3))
  }

  fn dependency_publication_begun_populated_source_list() -> Weight {
    Weight::from_parts(150_000_000, 24_000).saturating_add(T::DbWeight::get().reads_writes(7, 5))
  }

  fn dependency_publication_coalesced_active_source() -> Weight {
    Weight::from_parts(75_000_000, 12_000).saturating_add(T::DbWeight::get().reads_writes(3, 1))
  }

  /// Storage: `Actors::DeadlineHeaders` (r:1 w:1)
  /// Proof: `Actors::DeadlineHeaders` (`max_values`: None, `max_size`: Some(66), added: 2541, mode: `Measured`)
  /// Storage: `Actors::ActorControlLocators` (r:1 w:0)
  /// Proof: `Actors::ActorControlLocators` (`max_values`: None, `max_size`: Some(43), added: 2518, mode: `Measured`)
  /// Storage: `Actors::ActorUnsignaledControlCells` (r:1 w:0)
  /// Proof: `Actors::ActorUnsignaledControlCells` (`max_values`: None, `max_size`: Some(483), added: 2958, mode: `Measured`)
  /// Storage: `Actors::ActorProcesses` (r:1 w:1)
  /// Proof: `Actors::ActorProcesses` (`max_values`: None, `max_size`: Some(85), added: 2560, mode: `Measured`)
  /// Storage: `Actors::ServiceNodes` (r:1 w:1)
  /// Proof: `Actors::ServiceNodes` (`max_values`: None, `max_size`: Some(73), added: 2548, mode: `Measured`)
  /// Storage: `Actors::ServiceHeader` (r:1 w:1)
  /// Proof: `Actors::ServiceHeader` (`max_values`: Some(1), `max_size`: Some(26), added: 521, mode: `Measured`)
  /// Storage: `Actors::DeadlineHandles` (r:1 w:1)
  /// Proof: `Actors::DeadlineHandles` (`max_values`: None, `max_size`: Some(58), added: 2533, mode: `Measured`)
  /// Storage: `Actors::DeadlinePages` (r:1 w:1)
  /// Proof: `Actors::DeadlinePages` (`max_values`: None, `max_size`: Some(631), added: 3106, mode: `Measured`)
  /// Storage: `Actors::ActorWaitingOccupancies` (r:1 w:0)
  /// Proof: `Actors::ActorWaitingOccupancies` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `Measured`)
  /// Storage: `Actors::ActorWaitingCursorIndices` (r:1 w:0)
  /// Proof: `Actors::ActorWaitingCursorIndices` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `Measured`)
  /// Storage: `Actors::DeadlineIndexPositions` (r:14 w:14)
  /// Proof: `Actors::DeadlineIndexPositions` (`max_values`: None, `max_size`: Some(29), added: 2504, mode: `Measured`)
  /// Storage: `Actors::DeadlineIndexLen` (r:1 w:1)
  /// Proof: `Actors::DeadlineIndexLen` (`max_values`: None, `max_size`: Some(21), added: 2496, mode: `Measured`)
  /// Storage: `Actors::DeadlineIndexPages` (r:10 w:10)
  /// Proof: `Actors::DeadlineIndexPages` (`max_values`: None, `max_size`: Some(330), added: 2805, mode: `Measured`)
  fn service_member_to_deadline_new_key() -> Weight {
    // Proof Size summary in bytes:
    //  Measured:  `26194`
    //  Estimated: `61834`
    // Minimum execution time: 764_565_000 picoseconds.
    Weight::from_parts(822_674_000, 0)
      .saturating_add(Weight::from_parts(0, 61834))
      .saturating_add(T::DbWeight::get().reads(35))
      .saturating_add(T::DbWeight::get().writes(31))
  }
















  fn scheduler_inner_zero_step_complete() -> Weight {
    Weight::from_parts(37_645_000, 4_570)
  }

  fn scheduler_paged_execute_opening_max() -> Weight {
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

  fn scheduler_inner_suspended_head_retry(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_suspended_head_complete(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_suspended_head_progress(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_suspended_head_opening_retry(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_suspended_head_opening_complete(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }

  fn scheduler_inner_suspended_head_opening_progress(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
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

  fn run_progress() -> Weight {
    Weight::from_parts(30_000_000, 8_000).saturating_add(T::DbWeight::get().reads_writes(6, 2))
  }

  fn run_suspend() -> Weight {
    Weight::from_parts(28_668_868, 4_178)
      .saturating_add(T::DbWeight::get().reads_writes(2, 2))
  }
  fn run_complete() -> Weight {
    Weight::from_parts(18_019_000, 4_030).saturating_add(T::DbWeight::get().reads_writes(1, 2))
  }
  fn run_cancel() -> Weight {
    Weight::from_parts(56_782_000, 8_120).saturating_add(T::DbWeight::get().reads_writes(6, 4))
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
    Weight::from_parts(10_000_000, 1_560)
      .saturating_add(T::DbWeight::get().reads(1))
      .saturating_add(T::DbWeight::get().writes(2))
  }
}

#[cfg(any(test, feature = "runtime-benchmarks"))]
pub struct TestWeightInfo;

#[cfg(any(test, feature = "runtime-benchmarks"))]
impl WeightInfo for TestWeightInfo {
  fn scheduler_service_successful_interior() -> Weight { Weight::from_parts(184_664_000, 7_222) }
  fn scheduler_service_retry_to_deadline() -> Weight { Weight::from_parts(376_242_000, 11_293) }
  fn scheduler_service_retry_to_deadline_new_key() -> Weight { Weight::from_parts(1_886_095_000, 63_893) }
  fn scheduler_due_deadline_to_service() -> Weight { Weight::from_parts(368_908_000, 22_650) }
  fn scheduler_due_deadline_to_service_deep_index() -> Weight { Weight::from_parts(1_403_273_000, 88_173) }
  fn scheduler_service_late_refusal_rollback() -> Weight { Weight::from_parts(88_141_000, 6_179) }
  fn scheduler_service_terminal_retain_close() -> Weight { Weight::from_parts(278_880_000, 6_302) }
  fn scheduler_service_minimal_apoptosis() -> Weight { Weight::from_parts(288_170_000, 9_072) }
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
  fn manual_observation_park() -> Weight {
    Self::manual_trigger().saturating_add(Self::complete_cycle_to_parked_balance())
  }
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
  fn process_due_observation_availability_review() -> Weight { Weight::from_parts(1_500_000_000, 400_000) }
  fn process_due_parked_balance_review() -> Weight { Weight::from_parts(740_190_000, 43_950) }
  fn complete_cycle_to_parked_balance() -> Weight { Weight::from_parts(680_824_000, 43_950) }
  fn process_pending_parked_balance_event() -> Weight { Weight::from_parts(667_554_000, 43_950) }
  fn process_pending_observation_availability_event() -> Weight { Weight::from_parts(204_638_000, 4_570) }
  fn process_pending_observation_predicate_event() -> Weight {
    Self::process_pending_observation_availability_event()
  }
  fn process_due_observation_predicate_review() -> Weight {
    Self::process_due_observation_availability_review()
  }
  fn dependency_scan_source_probe() -> Weight { Weight::from_parts(6_774_000, 1_498) }
  fn process_dependency_scan_unit() -> Weight { Weight::from_parts(63_417_000, 4_570) }
  fn process_dependency_scan_completion_unit() -> Weight { Weight::from_parts(23_886_000, 3_523) }
  fn classify_due_block_deadline() -> Weight { Weight::from_parts(100_000_000, 100_000) }
  fn classify_due_tick_deadline() -> Weight { Weight::from_parts(100_000_000, 100_000) }
  fn deadline_destination_search(p: u32) -> Weight {
    Weight::from_parts(19_365_297, 5_074)
      .saturating_add(Weight::from_parts(20_716, 2).saturating_mul(p.into()))
  }
  fn return_due_block_deadline_to_service() -> Weight { Weight::from_parts(800_000_000, 300_000) }
  fn return_due_block_deadline_to_service_deep_index() -> Weight { Weight::from_parts(981_635_000, 51_480) }
  fn pipeline_admission_apoptosis() -> Weight { Weight::from_parts(161_616_000, 5_736) }
  fn close_actor() -> Weight { Weight::from_parts(84_719_000, 8_120) }
  fn fee_collection() -> Weight { Weight::from_parts(112_097_000, 8_120) }
  fn action_invocation_receipt() -> Weight { Weight::from_parts(10_000_000, 0) }
  fn predicate_set_evaluation(predicates: u32) -> Weight {
    if predicates == 0 {
      return Weight::zero();
    }
    let bounded = u64::from(predicates.min(4));
    Weight::from_parts(8_660_000, 3_675)
      .saturating_add(Weight::from_parts(9_778_566, 2_561).saturating_mul(bounded))

  }
  fn predicate_asset_evaluation(predicates: u32) -> Weight {
    Self::predicate_set_evaluation(predicates)
  }
  fn predicate_observation_heavy_evaluation(observations: u32) -> Weight {
    Self::predicate_set_evaluation(observations.saturating_add(1))
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
  fn scheduler_on_initialize_cutoff() -> Weight { Weight::from_parts(7_543_000, 1_493) }
  fn scheduler_on_idle_base() -> Weight { Weight::from_parts(25_000_000, 2_500) }
  fn materialization_coordinator_base() -> Weight { Weight::from_parts(20_000_000, 4_000) }
  fn service_member_publish_empty() -> Weight { Weight::from_parts(150_000_000, 24_000) }
  fn service_member_publish_populated() -> Weight { Weight::from_parts(200_000_000, 32_000) }
  fn service_member_retire_singleton() -> Weight { Weight::from_parts(34_851_000, 3_948) }
  fn service_member_retire_pair_cursor() -> Weight { Weight::from_parts(42_394_000, 6_086) }
  fn service_member_retire_interior() -> Weight { Weight::from_parts(46_724_000, 8_634) }
  fn service_member_insert_populated() -> Weight { Weight::from_parts(150_000_000, 24_000) }
  fn service_round_begin_populated() -> Weight { Weight::from_parts(50_000_000, 8_000) }
  fn service_round_probe_eligible() -> Weight { Weight::from_parts(75_000_000, 12_000) }
  fn service_round_admit_eligible() -> Weight { Weight::from_parts(125_000_000, 16_000) }
  fn dependency_publication_begun_empty_source_list() -> Weight { Weight::from_parts(100_000_000, 16_000) }
  fn dependency_publication_begun_populated_source_list() -> Weight { Weight::from_parts(150_000_000, 24_000) }
  fn dependency_publication_coalesced_active_source() -> Weight { Weight::from_parts(75_000_000, 12_000) }
  fn service_member_to_deadline_new_key() -> Weight { Weight::from_parts(822_674_000, 61_834) }
  fn scheduler_inner_zero_step_complete() -> Weight {
    Weight::from_parts(37_645_000, 4_570)
  }
  fn scheduler_paged_execute_opening_max() -> Weight {
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
  fn scheduler_inner_suspended_head_retry(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_suspended_head_complete(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_suspended_head_progress(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_suspended_head_opening_retry(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_suspended_head_opening_complete(_: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_inner_suspended_head_opening_progress(_: u32, _: u32) -> Weight {
    Weight::from_parts(20_000_000_000, 600_000)
  }
  fn scheduler_actor_state_probe() -> Weight { Weight::from_parts(38_413_000, 12_200) }
  fn transaction_extension_ingress_base() -> Weight { Weight::from_parts(15_226_000, 6_052) }
  fn transaction_extension_ingress_notify() -> Weight { Weight::from_parts(88_280_000, 8_120) }
  fn run_progress() -> Weight { Weight::from_parts(30_000_000, 8_000) }
  fn run_suspend() -> Weight { Weight::from_parts(28_668_868, 4_178) }
  fn run_complete() -> Weight { Weight::from_parts(18_019_000, 4_030) }
  fn run_cancel() -> Weight { Weight::from_parts(56_782_000, 8_120) }
  fn update_contract() -> Weight { Weight::from_parts(162_733_000, 10_181) }
  fn set_global_circuit_breaker() -> Weight { Weight::from_parts(8_000_000, 600) }
  fn clear_crossing_worker_fault() -> Weight { Weight::from_parts(16_000_000, 1_529) }
  fn clear_observation_fanout_worker_fault() -> Weight { Weight::from_parts(16_000_000, 1_529) }
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
