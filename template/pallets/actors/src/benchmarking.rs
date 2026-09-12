#![cfg(feature = "runtime-benchmarks")]

extern crate alloc;

use crate::scheduler::AttemptTransactionError;
use crate::types::Task as ActorTask;
use crate::*;
use alloc::{vec, vec::Vec};
use frame::prelude::*;
use polkadot_sdk::frame_benchmarking::{account, v2::*};
use polkadot_sdk::frame_support::traits::Hooks;
use polkadot_sdk::frame_system::RawOrigin;
use polkadot_sdk::sp_runtime::Perbill;

const BENCHMARK_ADMISSION_CELL_BYTES: usize = 171;
type BenchmarkAdmissionCell = [u8; BENCHMARK_ADMISSION_CELL_BYTES];

#[frame::storage_alias]
type BenchmarkAdmissionChunk4<T: Config> = StorageMap<
  Pallet<T>,
  Blake2_128Concat,
  u32,
  BoundedVec<BenchmarkAdmissionCell, ConstU32<4>>,
  OptionQuery,
>;
#[frame::storage_alias]
type BenchmarkAdmissionChunk8<T: Config> = StorageMap<
  Pallet<T>,
  Blake2_128Concat,
  u32,
  BoundedVec<BenchmarkAdmissionCell, ConstU32<8>>,
  OptionQuery,
>;
#[frame::storage_alias]
type BenchmarkAdmissionChunk16<T: Config> = StorageMap<
  Pallet<T>,
  Blake2_128Concat,
  u32,
  BoundedVec<BenchmarkAdmissionCell, ConstU32<16>>,
  OptionQuery,
>;
#[frame::storage_alias]
type BenchmarkAdmissionChunk32<T: Config> = StorageMap<
  Pallet<T>,
  Blake2_128Concat,
  u32,
  BoundedVec<BenchmarkAdmissionCell, ConstU32<32>>,
  OptionQuery,
>;
#[frame::storage_alias]
type BenchmarkAdmissionChunk64<T: Config> = StorageMap<
  Pallet<T>,
  Blake2_128Concat,
  u32,
  BoundedVec<BenchmarkAdmissionCell, ConstU32<64>>,
  OptionQuery,
>;
#[benchmarks]
mod benches {
  use super::*;

  const CROSSING_COHORT_BENCHMARK_MAX: u32 = 128;
  const CROSSING_NON_TAIL_BENCHMARK_MAX: u32 = 64;
  const CROSSING_TRIMMED_BENCHMARK_TAIL: u32 = CROSSING_NON_TAIL_BENCHMARK_MAX + 2;

  fn control_named_cell<T: Config>(actor_id: ActorId) -> ActorControlCellOf<T> {
    let owner: T::AccountId = account("control-cell", actor_id as u32, 0);
    ActorControlCell {
      actor_id,
      identity: ActorControlIdentity {
        owner,
        actor_class: ActorClass::System {
          sovereign_id: actor_id,
        },
        mutability: Mutability::Mutable,
        cycle_nonce: 0,
        last_control_mutation_block: 0u32.into(),
      },
      hot: ActorControlHotState {
        lifecycle: ActiveLifecycle::Active,
        cycle_state: CycleState::Idle,
        trigger_runtime_state: TriggerRuntimeState::Stateless,
        unsuccessful_attempt_streak: 0,
        pending_signal: true,
        wakeup_pointer: None,
        trigger_wakeup_pointer: None,
        terminal_at: None,
        schedule_anchor: 0u32.into(),
        last_cycle_block: None,
      },
      cursor: 0,
      eligible_at: Some(1u32.into()),
      admission: ActorAdmissionCertificate::<ActorAdmissionResourcesOf<T>>::new(
        [1u8; 32],
        [2u8; 32],
        1,
        [3u8; 32],
        1,
        [4u8; 32],
        Weight::from_parts(1, 1),
      ),
      resources: ActorStepResourceEnvelope {
        control: Weight::from_parts(1, 1),
        effect: Weight::from_parts(1, 1),
      },
    }
  }

  fn control_named_chunk<T: Config>(first_actor_id: ActorId, len: u32) -> ActorControlChunkOf<T> {
    BoundedVec::try_from(
      (0..len)
        .map(|offset| {
          Some(control_named_cell::<T>(
            first_actor_id.saturating_add(offset as u64),
          ))
        })
        .collect::<Vec<_>>(),
    )
    .expect("Actor control named C32 chunk fits")
  }

  fn control_waiting_page<T: Config>(entries: ActorWaitingChunkOf<T>) -> ActorWaitingPageOf<T> {
    ActorWaitingPageOf::<T> {
      live_entries: entries.iter().filter(|entry| entry.is_some()).count() as u32,
      entries,
      scan_slot: 0,
      previous_page: None,
      next_page: None,
    }
  }

  #[derive(Clone)]
  struct Schedule<Trigger> {
    trigger: Trigger,
    cooldown_blocks: u32,
  }

  type ScheduleOf<T> = Schedule<TriggerOf<T>>;

  fn ensure_creation_balance<T: Config>(owner: &T::AccountId) {
    // The generic mint adapter need not establish a System provider before a host bond adapter
    // creates reserved/held custody. Real signed owners already have one through their funded
    // account; benchmark setup must establish the same prerequisite explicitly.
    polkadot_sdk::frame_system::Pallet::<T>::inc_providers(owner);
    let creation_fee = T::ActorCreationFee::get();
    let hold_capacity: T::Balance = (u64::MAX / 4).saturated_into();
    let amount = creation_fee
      .saturating_add(hold_capacity)
      .saturating_add(One::one());
    let _ = T::AssetOps::mint(owner, T::FeeNativeAssetId::get(), amount);
  }

  fn prefund_user_sovereign<T: Config>(
    owner: &T::AccountId,
    slot: u8,
    contract_steps: &ContractSteps<T>,
  ) {
    let required = Pallet::<T>::user_pipeline_machine_capacity_requirement(contract_steps)
      .expect("benchmark User Cycle has a checked prefunding requirement");
    let sovereign = Pallet::<T>::sovereign_account_id(owner, slot);
    let _ = T::AssetOps::mint(&sovereign, T::FeeNativeAssetId::get(), required);
  }

  fn prefund_active_user_creation<T: Config>(
    owner: &T::AccountId,
    contract_steps: &ContractSteps<T>,
  ) {
    let slot =
      Pallet::<T>::available_owner_slot(owner, None).expect("benchmark owner has a free User slot");
    prefund_user_sovereign::<T>(owner, slot, contract_steps);
  }

  fn user_contract<T: Config>(
    schedule: ScheduleOf<T>,
    contract_steps: ContractSteps<T>,
  ) -> Option<ActorContractOf<T>> {
    Some(ActorContract {
      trigger: schedule.trigger,
      cooldown_blocks: schedule.cooldown_blocks,
      window: None,
      steps: contract_steps,
      completion: CompletionPolicy::Persistent,
      funding: FundingSourcePolicy::OwnerOnly,
      auto_close_at_cycle_nonce: None,
    })
  }

  fn system_contract<T: Config>(
    schedule: ScheduleOf<T>,
    contract_steps: ContractSteps<T>,
  ) -> Option<ActorContractOf<T>> {
    Some(ActorContract {
      trigger: schedule.trigger,
      cooldown_blocks: schedule.cooldown_blocks,
      window: None,
      steps: contract_steps,
      completion: CompletionPolicy::Persistent,
      funding: FundingSourcePolicy::RuntimePolicy,
      auto_close_at_cycle_nonce: None,
    })
  }

  fn full_attempt_fee<T: Config>(contract_steps: &ContractSteps<T>) -> T::Balance {
    Pallet::<T>::attempt_fee_envelope(ActorType::User, contract_steps, 0)
      .expect("benchmark execution plan has a checked fee envelope")
      .total
  }

  fn benchmark_predicate_capacity<T: Config>() -> u32 {
    T::MaxPredicatesPerStep::get()
      .min(T::MaxPreconditionClauses::get().saturating_mul(T::MaxPredicatesPerClause::get()))
  }

  fn packed_predicate_clauses<T: Config>(
    predicates: Vec<TimedPredicate<Predicate<T::AssetId, T::Balance, u32, T::ObservationFeedId>>>,
    width: u32,
  ) -> PreconditionOf<T> {
    assert!(width > 0 && width <= T::MaxPredicatesPerClause::get());
    assert!(predicates.len() as u32 <= benchmark_predicate_capacity::<T>());
    let clauses = predicates
      .chunks(width as usize)
      .map(|chunk| BoundedVec::try_from(chunk.to_vec()).expect("benchmark predicate clause fits"))
      .collect::<Vec<_>>();
    Precondition {
      clauses: BoundedVec::try_from(clauses)
        .expect("host cannot represent benchmark predicate count"),
    }
  }

  fn make_contract_steps<T: Config>(recipient: T::AccountId) -> ContractSteps<T> {
    let step = Step {
      precondition: None,
      task: ActorTask::Transfer {
        to: recipient,
        asset: T::FeeNativeAssetId::get(),
        amount: AmountResolution::AllAvailable,
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    BoundedVec::try_from(vec![step]).expect("single-step contract_steps must fit")
  }

  fn make_max_contract_steps<T: Config>(recipient: T::AccountId) -> ContractSteps<T> {
    let step = Step {
      precondition: None,
      task: ActorTask::Transfer {
        to: recipient,
        asset: T::FeeNativeAssetId::get(),
        amount: AmountResolution::AllAvailable,
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    BoundedVec::try_from(vec![step; T::MaxContractSteps::get() as usize])
      .expect("maximum Contract Steps must fit")
  }

  /// Retains the requested structural geometry and protected measured Steps while reducing only
  /// unrelated setup payload until ordinary component-wise admission accepts the Contract.
  fn retain_admitted_contract_geometry<T: Config>(
    actor_type: ActorType,
    steps: &mut ContractSteps<T>,
    protected: &[u32],
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let service = Pallet::<T>::guaranteed_actor_service_weight().ok_or(
      polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host does not expose a valid Actor service floor",
      ),
    )?;
    if Pallet::<T>::contract_steps_admission_weight_upper(actor_type, steps).all_lte(service) {
      return Ok(());
    }
    for index in (0..steps.len()).rev() {
      if protected.contains(&(index as u32)) {
        continue;
      }
      steps[index] = Step {
        precondition: None,
        task: ActorTask::Transfer {
          to: account("admitted-contract-filler", index as u32, 0),
          asset: T::FeeNativeAssetId::get(),
          amount: AmountResolution::AllAvailable,
        },
        on_error: StepErrorPolicy::AbortCycle,
      };
      if Pallet::<T>::contract_steps_admission_weight_upper(actor_type, steps).all_lte(service) {
        return Ok(());
      }
    }
    Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
      "protected benchmark branch exceeds ordinary idle-budget admission",
    ))
  }

  fn assert_max_contract_geometry<T: Config>(actor_id: ActorId) {
    let expected_tail_chunks = T::MaxContractSteps::get()
      .saturating_sub(1)
      .div_ceil(MAX_STEPS_PER_TAIL_CHUNK) as usize;
    assert_eq!(
      ActorContractTailChunks::<T>::iter_prefix(actor_id).count(),
      expected_tail_chunks
    );
  }

  fn make_tracked_funding_contract_steps<T: Config>(recipient: T::AccountId) -> ContractSteps<T> {
    BoundedVec::try_from(vec![Step {
      precondition: None,
      task: ActorTask::Transfer {
        to: recipient,
        asset: T::FeeNativeAssetId::get(),
        amount: AmountResolution::PercentageOfLastFunding(polkadot_sdk::sp_runtime::Perbill::one()),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }])
    .expect("single-step tracked funding plan must fit")
  }

  fn make_remove_liquidity_contract_steps<T: Config>(
    lp_asset: T::AssetId,
    asset_a: T::AssetId,
    asset_b: T::AssetId,
    amount: T::Balance,
  ) -> ContractSteps<T> {
    let step = Step {
      precondition: None,
      task: ActorTask::RemoveLiquidity {
        lp_asset,
        asset_a,
        asset_b,
        lp_amount: AmountResolution::Fixed(amount),
        min_amount_a: One::one(),
        min_amount_b: One::one(),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    BoundedVec::try_from(vec![step]).expect("single-step contract_steps must fit")
  }

  fn prefill_owner_slots_for_worst_case<T: Config>(owner: &T::AccountId) -> u8 {
    let max_slots = T::MaxOwnerSlots::get();
    assert!(max_slots > 0, "MaxOwnerSlots must be greater than zero");
    let target_slot = max_slots.saturating_sub(1);
    let mut bitmap = [0; 32];
    for slot in 0..target_slot {
      bitmap[(slot / 8) as usize] |= 1u8 << (slot % 8);
    }
    OwnerSlotBitmaps::<T>::insert(owner.clone(), bitmap);
    target_slot
  }

  fn prefill_reachable_owner_slots<T: Config>(owner: &T::AccountId) -> u8 {
    let target_slot = T::MaxOwnerSlots::get()
      .checked_sub(1)
      .expect("host must admit at least one owner slot");
    for slot in 0..target_slot {
      Pallet::<T>::create_user_actor_at_slot(
        RawOrigin::Signed(owner.clone()).into(),
        slot,
        Mutability::Mutable,
        None,
      )
      .expect("dormant User slot guard is admitted");
    }
    target_slot
  }

  // Observation scaffolding for lifecycle benchmarks.
  //
  // Each Actor owns at most one observation subscription. The helpers below keep the measured
  // actor on the heavier subscribe/unsubscribe branches, including recycled slots and dirty-list
  // middle-node removal.

  /// Allocates `count` distinct observation feeds. Benchmarks carve their measured sets out of one
  /// pool because the helper is deterministic and repeated calls would return overlapping feeds.
  fn observation_feed_pool<T: Config>(count: u32) -> alloc::vec::Vec<T::ObservationFeedId> {
    let feeds = T::BenchmarkHelper::setup_observation_feeds(count)
      .expect("observation benchmark feeds must be available");
    assert_eq!(
      feeds.len() as u32,
      count,
      "observation benchmark helper must supply every requested feed"
    );
    feeds
  }

  /// Builds the one admitted observation trigger.
  fn observation_trigger<T: Config>(feed: T::ObservationFeedId) -> TriggerOf<T> {
    Trigger::observation_change(feed)
  }

  /// Subscribes a permanent guard actor to `feed`. Guards bracket the measured feeds in the dirty
  /// list so each unsubscribed feed is a middle node and pays both neighbour writes on unlink.
  fn install_observation_guard<T: Config>(feed: T::ObservationFeedId, seed: u32) {
    let owner: T::AccountId = account("observation-dirty-guard", seed, 0);
    let _ = bench_create_system_observation::<T>(owner, feed);
  }

  /// Creates and closes a subscribed actor on `feed` so the next slot allocation takes the
  /// free-list branch (page read, pop, page delete-or-write, length write) rather than the bare
  /// counter increment. Any runtime that has ever closed a subscribed actor is in this state.
  fn seed_recycled_observation_slot<T: Config>(feed: T::ObservationFeedId) {
    let owner: T::AccountId = account("observation-slot-donor", 0, 0);
    let actor_id = bench_create_system_observation::<T>(owner, feed);
    Pallet::<T>::close_actor(RawOrigin::Root.into(), actor_id)
      .expect("observation slot donor close must succeed");
    assert!(
      ObservationFreeSlotLen::<T>::get() > 0,
      "donor close must recycle one observation subscription slot"
    );
  }

  fn benchmark_fixture_hot<T: Config>(actor_id: ActorId) -> Option<ActorHotStateOf<T>> {
    Pallet::<T>::load_frame_control_authority(actor_id).map(|(_, _, hot, _)| hot)
  }

  fn benchmark_fixture_scalar_hot<T: Config>(actor_id: ActorId) -> Option<ActorHotStateOf<T>> {
    benchmark_fixture_hot::<T>(actor_id)
  }

  fn benchmark_fixture_mutate_hot<T: Config>(
    actor_id: ActorId,
    mutate: impl FnOnce(&mut ActorHotStateOf<T>),
  ) {
    let mut hot =
      benchmark_fixture_hot::<T>(actor_id).expect("benchmark fixture hot authority exists");
    let previous_ticket = hot.queue_ticket;
    mutate(&mut hot);
    let requested_ticket = hot.queue_ticket;
    if let Some(ticket) = requested_ticket.filter(|ticket| Some(*ticket) != previous_ticket) {
      // Prepare the successor from captured primary authority, before publishing the latch or Run
      // transition. An Unsignaled pending/Running cell is not a valid reload boundary.
      let mut cell = Pallet::<T>::remove_primary_control_cell_inner(actor_id)
        .expect("benchmark source primary exists");
      hot.queue_ticket = None;
      hot.wakeup_pointer = None;
      let run = ActorRunStateStore::<T>::get(actor_id);
      if hot.cycle_state == CycleState::Idle {
        hot.pending_signal = true;
      }
      cell.hot = Pallet::<T>::control_hot_from_scalar(hot);
      cell.cursor = run.as_ref().map_or(0, |run| run.cursor);
      cell.eligible_at = Some(
        run
          .as_ref()
          .map_or_else(frame_system::Pallet::<T>::block_number, |run| {
            run.eligible_at
          }),
      );
      benchmark_fixture_advance_ready_tail::<T>(ticket);
      Pallet::<T>::control_append_ready(cell).expect("benchmark Ready placement fits");
      return;
    }
    if previous_ticket != requested_ticket {
      if previous_ticket.is_some() {
        assert_eq!(Pallet::<T>::paged_invalidate(actor_id), previous_ticket);
      }
      hot.queue_ticket = None;
    }
    Pallet::<T>::update_existing_frame_control_hot(actor_id, &hot)
      .expect("benchmark fixture updates the existing primary");
  }

  fn benchmark_fixture_identity<T: Config>(actor_id: ActorId) -> Option<ActorIdentityOf<T>> {
    Pallet::<T>::load_frame_control_authority(actor_id).map(|(_, identity, _, _)| identity)
  }

  fn benchmark_fixture_scalar_identity<T: Config>(actor_id: ActorId) -> Option<ActorIdentityOf<T>> {
    Pallet::<T>::actor_identity(actor_id)
  }

  fn benchmark_fixture_contains_scalar_identity<T: Config>(actor_id: ActorId) -> bool {
    Pallet::<T>::actor_identity(actor_id).is_some()
  }

  fn benchmark_fixture_mutate_identity<T: Config>(
    actor_id: ActorId,
    mutate: impl FnOnce(&mut ActorIdentityOf<T>),
  ) {
    let mut identity = benchmark_fixture_scalar_identity::<T>(actor_id)
      .expect("benchmark fixture identity authority exists");
    mutate(&mut identity);
    if ActorControlLocators::<T>::contains_key(actor_id) {
      Pallet::<T>::update_existing_frame_control_identity(actor_id, &identity)
        .expect("benchmark fixture updates active identity");
    } else {
      ActorIdentities::<T>::insert(actor_id, identity);
    }
  }

  fn benchmark_fixture_admission<T: Config>(
    actor_id: ActorId,
  ) -> Option<ActorAdmissionCertificateOf<T>> {
    Pallet::<T>::load_frame_control_authority(actor_id).map(|(_, _, _, admission)| admission)
  }

  fn benchmark_fixture_scalar_admission<T: Config>(
    actor_id: ActorId,
  ) -> Option<ActorAdmissionCertificateOf<T>> {
    benchmark_fixture_admission::<T>(actor_id)
  }

  fn benchmark_fixture_align_primary_control<T: Config>(actor_id: ActorId) {
    let identity =
      benchmark_fixture_scalar_identity::<T>(actor_id).expect("benchmark identity exists");
    let hot = benchmark_fixture_scalar_hot::<T>(actor_id).expect("benchmark hot state exists");
    let admission =
      benchmark_fixture_scalar_admission::<T>(actor_id).expect("benchmark admission exists");
    let location = ActorControlLocators::<T>::get(actor_id).expect("benchmark primary exists");
    assert!(Pallet::<T>::store_frame_control_authority(
      actor_id, location, identity, hot, admission,
    ));
  }

  fn benchmark_fixture_publish_trigger_waiting<T: Config>(
    actor_id: ActorId,
    wakeup_tick: SchedulerTick,
  ) {
    let mut cell = Pallet::<T>::remove_primary_control_cell_inner(actor_id)
      .expect("benchmark temporal source primary exists");
    cell.cursor = ActorRunStateStore::<T>::get(actor_id).map_or(0, |run| run.cursor);
    cell.eligible_at = None;
    Pallet::<T>::control_append_waiting(
      cell,
      WakeupKey::Tick(wakeup_tick),
      crate::scheduler::ActorWaitingAuthority::Trigger,
    )
    .expect("benchmark temporal Waiting placement fits");
  }

  fn benchmark_fixture_schedule_service_waiting<T: Config>(
    actor_id: ActorId,
    wakeup_block: BlockNumberFor<T>,
  ) {
    benchmark_fixture_prepare_service_waiting::<T>(actor_id, wakeup_block)();
  }

  fn benchmark_fixture_prepare_consumed_service_waiting<T: Config>(
    actor_id: ActorId,
    wakeup_block: BlockNumberFor<T>,
  ) -> impl FnOnce() {
    let (state, admission, _) = Pallet::<T>::load_frame_actor_service_state(actor_id)
      .expect("benchmark consumed source service authority exists");
    let (_, cell) =
      Pallet::<T>::actor_control_cell(actor_id).expect("benchmark consumed source primary exists");
    let mut hot = state.hot;
    hot.pending_signal = true;
    Pallet::<T>::remove_primary_control_cell_inner(actor_id)
      .expect("benchmark service source is consumed before Waiting publication");
    move || {
      Pallet::<T>::try_wakeup_substrate_schedule_transition_with_authority(
        actor_id,
        WakeupKey::Block(wakeup_block),
        hot,
        &state.identity,
        cell.cursor,
        &admission,
        cell.resources,
      )
      .expect("benchmark consumed service Waiting placement fits")
    }
  }

  fn benchmark_fixture_prepare_service_waiting<T: Config>(
    actor_id: ActorId,
    wakeup_block: BlockNumberFor<T>,
  ) -> impl FnOnce() {
    let (state, admission, _) = Pallet::<T>::load_frame_actor_service_state(actor_id)
      .expect("benchmark source service authority exists");
    let (_, cell) =
      Pallet::<T>::actor_control_cell(actor_id).expect("benchmark source primary exists");
    let mut hot = state.hot;
    hot.pending_signal = true;
    move || {
      Pallet::<T>::try_wakeup_substrate_schedule_transition_with_authority(
        actor_id,
        WakeupKey::Block(wakeup_block),
        hot,
        &state.identity,
        cell.cursor,
        &admission,
        cell.resources,
      )
      .expect("benchmark latched service Waiting placement fits")
    }
  }

  fn benchmark_fixture_active_count<T: Config>() -> u32 {
    ActorControlLocators::<T>::iter_keys().count() as u32
  }

  fn benchmark_fixture_hot_actor_ids<T: Config>(
    select: impl Fn(&ActorHotStateOf<T>) -> bool,
  ) -> alloc::vec::Vec<ActorId> {
    ActorControlLocators::<T>::iter_keys()
      .filter_map(|actor_id| {
        benchmark_fixture_hot::<T>(actor_id)
          .filter(|hot| select(hot))
          .map(|_| actor_id)
      })
      .collect()
  }

  fn benchmark_fixture_advance_ready_tail<T: Config>(ticket: QueueTicket) {
    let tail = ActorReadyTail::<T>::get();
    assert!(ticket >= tail, "fixture preserves global ticket chronology");
    if ActorReadyHead::<T>::get() == tail {
      ActorReadyHead::<T>::put(ticket);
    } else {
      assert!(ticket - ActorReadyHead::<T>::get() < u64::from(T::MaxQueueLength::get()));
      for page_id in tail / 32..=ticket / 32 {
        if !ActorReadyFrameChunks::<T>::contains_key(page_id) {
          ActorReadyFrameChunks::<T>::insert(
            page_id,
            ActorControlChunkOf::<T>::try_from(vec![None; 32]).expect("fixed Ready chunk"),
          );
        }
      }
    }
    ActorReadyTail::<T>::put(ticket);
  }

  fn benchmark_fixture_ready_page_len<T: Config>(page_id: QueuePageId) -> Option<usize> {
    ActorReadyFrameChunks::<T>::get(page_id).map(|page| page.len())
  }

  fn benchmark_fixture_contains_ready_page<T: Config>(page_id: QueuePageId) -> bool {
    ActorReadyFrameChunks::<T>::contains_key(page_id)
  }

  fn benchmark_fixture_ready_enqueue<T: Config>(actor_id: ActorId) -> bool {
    let Some((state, admission, _)) = Pallet::<T>::load_frame_actor_service_state(actor_id) else {
      return false;
    };
    let Some((_, cell)) = Pallet::<T>::actor_control_cell(actor_id) else {
      return false;
    };
    let mut hot = state.hot;
    if hot.cycle_state == CycleState::Idle {
      hot.pending_signal = true;
    }
    Pallet::<T>::preflight_paged_enqueue_authority(
      actor_id,
      hot,
      &state.identity,
      state.run_state.as_ref(),
      &admission,
      cell.resources,
    )
    .and_then(Pallet::<T>::commit_paged_enqueue)
    .is_ok()
  }

  fn benchmark_fixture_prepare_consumed_ready_enqueue<T: Config>(
    actor_id: ActorId,
  ) -> impl FnOnce() -> bool {
    let (state, admission, _) = Pallet::<T>::load_frame_actor_service_state(actor_id)
      .expect("benchmark destination authority exists");
    let (_, cell) =
      Pallet::<T>::actor_control_cell(actor_id).expect("benchmark destination primary exists");
    let mut hot = state.hot;
    if hot.cycle_state == CycleState::Idle {
      hot.pending_signal = true;
    }
    Pallet::<T>::remove_primary_control_cell_inner(actor_id)
      .expect("benchmark destination source is consumed before measurement");
    move || {
      Pallet::<T>::preflight_paged_enqueue_authority(
        actor_id,
        hot,
        &state.identity,
        state.run_state.as_ref(),
        &admission,
        cell.resources,
      )
      .and_then(Pallet::<T>::commit_paged_enqueue)
      .is_ok()
    }
  }

  #[inline(always)]
  fn benchmark_fixture_ready_consume_head<T: Config>(ticket: QueueTicket) -> bool {
    Pallet::<T>::paged_consume_head(ticket)
  }

  #[inline(always)]
  fn benchmark_fixture_ready_consume_loaded_head<T: Config>(
    position: QueueTicket,
    actor_id: ActorId,
    ticket: QueueTicket,
    hot: ActorHotStateOf<T>,
  ) -> bool {
    Pallet::<T>::paged_consume_loaded_head_at(position, actor_id, ticket, hot).is_ok()
  }

  #[inline(always)]
  fn benchmark_fixture_ready_drain_tombstones<T: Config>(
    cutoff: QueueTicket,
    scan_limit: u32,
  ) -> Result<QueueDrainStats, EnqueueOutcome> {
    Pallet::<T>::paged_drain_tombstones(cutoff, scan_limit)
  }

  #[inline(always)]
  fn benchmark_fixture_ready_head_entry<T: Config>()
  -> Option<(QueueTicket, QueueEntry<BlockNumberFor<T>>)> {
    Pallet::<T>::paged_head_entry()
  }

  fn benchmark_fixture_ready_head<T: Config>() -> u64 {
    ActorReadyHead::<T>::get()
  }

  fn benchmark_fixture_ready_tail<T: Config>() -> u64 {
    ActorReadyTail::<T>::get()
  }

  fn benchmark_fixture_ready_occupancy<T: Config>() -> u32 {
    ActorReadyOccupancy::<T>::get()
  }

  fn benchmark_fixture_next_ready_ticket<T: Config>() -> QueueTicket {
    ActorReadyTail::<T>::get()
  }

  fn benchmark_fixture_set_next_ready_ticket<T: Config>(ticket: QueueTicket) {
    ActorReadyTail::<T>::put(ticket);
  }

  fn benchmark_fixture_set_ready_queue_state<T: Config>(head: u64, tail: u64, occupancy: u32) {
    ActorReadyHead::<T>::put(head);
    ActorReadyTail::<T>::put(tail);
    ActorReadyOccupancy::<T>::put(occupancy);
  }

  fn benchmark_fixture_seed_saturated_ready_tombstones<T: Config>() {
    let pages = ActorReadyFrameChunks::<T>::iter_keys().collect::<Vec<_>>();
    for page in pages {
      ActorReadyFrameChunks::<T>::remove(page);
    }
    let capacity = T::MaxQueueLength::get();
    for page in 0..capacity.div_ceil(32) {
      ActorReadyFrameChunks::<T>::insert(
        u64::from(page),
        ActorControlChunkOf::<T>::try_from(vec![None; 32])
          .expect("canonical saturated tombstone chunk fits"),
      );
    }
    ActorReadyHead::<T>::put(0);
    ActorReadyTail::<T>::put(u64::from(capacity));
    ActorReadyOccupancy::<T>::put(0);
  }

  fn benchmark_fixture_reset_ready_queue<T: Config>() {
    let ids = ActorControlLocators::<T>::iter()
      .filter_map(|(id, location)| {
        matches!(location, ActorControlLocation::Ready { .. }).then_some(id)
      })
      .collect::<alloc::vec::Vec<_>>();
    for id in ids {
      assert!(Pallet::<T>::paged_invalidate(id).is_some());
    }
    let tail = ActorReadyTail::<T>::get();
    let _ = ActorReadyFrameChunks::<T>::clear(u32::MAX, None);
    benchmark_fixture_set_ready_queue_state::<T>(tail, tail, 0);
  }

  fn seed_actor_for_cycle<T: Config>(actor_id: ActorId) {
    let Some(instance) = Pallet::<T>::active_actor_view(actor_id) else {
      return;
    };
    let reserve = full_attempt_fee::<T>(&instance.steps)
      .saturating_add(T::MinUserBalance::get())
      .saturating_add(One::one());
    let _ = T::AssetOps::mint(
      &instance.sovereign_account,
      T::FeeNativeAssetId::get(),
      reserve,
    );
  }

  fn bench_create_user<T: Config>(caller: T::AccountId) -> ActorId {
    bench_create_user_with_trigger::<T>(caller, Trigger::manual())
  }

  fn bench_create_user_with_trigger<T: Config>(
    caller: T::AccountId,
    trigger: TriggerOf<T>,
  ) -> ActorId {
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    bench_create_user_with_trigger_and_steps::<T>(
      caller,
      trigger,
      make_contract_steps::<T>(recipient),
    )
  }

  fn bench_create_user_with_trigger_and_steps<T: Config>(
    caller: T::AccountId,
    trigger: TriggerOf<T>,
    contract_steps: ContractSteps<T>,
  ) -> ActorId {
    ensure_creation_balance::<T>(&caller);
    prefund_active_user_creation::<T>(&caller, &contract_steps);
    let schedule = Schedule {
      trigger,
      cooldown_blocks: 10,
    };
    Pallet::<T>::create_user_actor(
      RawOrigin::Signed(caller).into(),
      Mutability::Mutable,
      user_contract::<T>(schedule, contract_steps),
    )
    .expect("create_user_actor must succeed in benchmark setup");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    seed_actor_for_cycle::<T>(actor_id);
    actor_id
  }

  #[benchmark]
  fn create_user_actor() {
    let caller: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&caller);
    let expected_slot = prefill_owner_slots_for_worst_case::<T>(&caller);
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    let contract_steps = make_max_contract_steps::<T>(recipient);
    prefund_user_sovereign::<T>(&caller, expected_slot, &contract_steps);
    let feed = observation_feed_pool::<T>(1)[0];
    let schedule = Schedule {
      trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, u128::MAX, 0),
      cooldown_blocks: 10,
    };
    #[extrinsic_call]
    create_user_actor(
      RawOrigin::Signed(caller),
      Mutability::Mutable,
      user_contract::<T>(schedule, contract_steps),
    );
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let inst =
      Pallet::<T>::active_actor_view(actor_id).expect("Actors must exist after create_user_actor");
    assert_eq!(inst.actor_class.owner_slot(), Some(expected_slot));
    assert_max_contract_geometry::<T>(actor_id);
    assert!(CrossingMemberships::<T>::contains_key(actor_id));
    assert!(ActorObservationFeeds::<T>::get(actor_id).is_none());
  }

  #[benchmark]
  fn create_user_actor_at_slot() {
    let caller: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&caller);
    let requested_slot = T::MaxOwnerSlots::get().saturating_sub(1);
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    let contract_steps = make_max_contract_steps::<T>(recipient);
    prefund_user_sovereign::<T>(&caller, requested_slot, &contract_steps);
    let feed = observation_feed_pool::<T>(1)[0];
    let schedule = Schedule {
      trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, u128::MAX, 0),
      cooldown_blocks: 10,
    };
    #[extrinsic_call]
    create_user_actor_at_slot(
      RawOrigin::Signed(caller),
      requested_slot,
      Mutability::Mutable,
      user_contract::<T>(schedule, contract_steps),
    );
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let inst = Pallet::<T>::active_actor_view(actor_id)
      .expect("Actors must exist after create_user_actor_at_slot");
    assert_eq!(inst.actor_class.owner_slot(), Some(requested_slot));
    assert_max_contract_geometry::<T>(actor_id);
    assert!(CrossingMemberships::<T>::contains_key(actor_id));
  }

  #[benchmark]
  fn create_system_actor() {
    let owner: T::AccountId = whitelisted_caller();
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    let contract_steps = make_max_contract_steps::<T>(recipient);
    let feed = observation_feed_pool::<T>(1)[0];
    let schedule = Schedule {
      trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, u128::MAX, 0),
      cooldown_blocks: 100,
    };
    #[extrinsic_call]
    create_system_actor(
      RawOrigin::Root,
      owner,
      Mutability::Mutable,
      system_contract::<T>(schedule, contract_steps),
    );
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let inst = Pallet::<T>::active_actor_view(actor_id)
      .expect("Actors must exist after create_system_actor");
    assert_eq!(
      inst.actor_class,
      ActorClass::System {
        sovereign_id: actor_id,
      }
    );
    assert_max_contract_geometry::<T>(actor_id);
    assert!(CrossingMemberships::<T>::contains_key(actor_id));
  }

  #[benchmark]
  fn create_system_actor_at_sovereign_id() {
    let owner: T::AccountId = whitelisted_caller();
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    let contract_steps = make_max_contract_steps::<T>(recipient.clone());
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 100,
    };
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner.clone(),
      Mutability::Mutable,
      system_contract::<T>(schedule.clone(), contract_steps.clone()),
    )
    .expect("create_system_actor must succeed in benchmark setup");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    Pallet::<T>::close_actor(RawOrigin::Root.into(), actor_id)
      .expect("close_actor must succeed in benchmark setup");
    let fresh_id = NextActorId::<T>::get();
    let feed = observation_feed_pool::<T>(1)[0];
    let crossing_schedule = Schedule {
      trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, u128::MAX, 0),
      cooldown_blocks: 100,
    };
    #[extrinsic_call]
    create_system_actor_at_sovereign_id(
      RawOrigin::Root,
      actor_id,
      owner,
      Mutability::Mutable,
      system_contract::<T>(crossing_schedule, contract_steps),
    );
    assert!(Pallet::<T>::active_actor_exists(fresh_id));
    assert_max_contract_geometry::<T>(fresh_id);
    assert!(CrossingMemberships::<T>::contains_key(fresh_id));
  }

  // Diagnostic install branch: append to an existing non-full Crossing leaf page.
  #[benchmark]
  fn create_user_actor_crossing_existing() {
    let caller: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&caller);
    let feed = observation_feed_pool::<T>(1)[0];
    let threshold = u128::MAX;
    let guard_owner: T::AccountId = account("crossing-existing-guard", 0, 0);
    let guard = bench_create_system_crossing::<T>(guard_owner, feed, threshold);
    let expected_slot = prefill_owner_slots_for_worst_case::<T>(&caller);
    let recipient: T::AccountId = account("crossing-existing-recipient", 0, 0);
    let contract_steps = make_contract_steps::<T>(recipient);
    prefund_user_sovereign::<T>(&caller, expected_slot, &contract_steps);
    let schedule = Schedule {
      trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, threshold, 0),
      cooldown_blocks: 10,
    };
    #[block]
    {
      Pallet::<T>::create_user_actor(
        RawOrigin::Signed(caller).into(),
        Mutability::Mutable,
        user_contract::<T>(schedule, contract_steps),
      )
      .expect("existing-leaf Crossing creation must succeed");
    }
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let guard_locator = CrossingMemberships::<T>::get(guard).expect("guard membership exists");
    let locator = CrossingMemberships::<T>::get(actor_id).expect("measured membership exists");
    assert_eq!(locator.key, guard_locator.key);
    assert_eq!(locator.page, guard_locator.page);
    assert_eq!(locator.offset, guard_locator.offset.saturating_add(1));
  }

  // Diagnostic install branch: allocate a new page on an existing full Crossing leaf.
  #[benchmark]
  fn create_user_actor_crossing_new_page() {
    let caller: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&caller);
    let feed = observation_feed_pool::<T>(1)[0];
    let threshold = u128::MAX;
    for index in 0..T::CrossingPageSize::get() {
      let guard_owner: T::AccountId = account("crossing-page-guard", index, 0);
      let _ = bench_create_system_crossing::<T>(guard_owner, feed, threshold);
    }
    let expected_slot = prefill_owner_slots_for_worst_case::<T>(&caller);
    let recipient: T::AccountId = account("crossing-page-recipient", 0, 0);
    let contract_steps = make_max_contract_steps::<T>(recipient);
    prefund_user_sovereign::<T>(&caller, expected_slot, &contract_steps);
    let schedule = Schedule {
      trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, threshold, 0),
      cooldown_blocks: 10,
    };
    #[block]
    {
      Pallet::<T>::create_user_actor(
        RawOrigin::Signed(caller).into(),
        Mutability::Mutable,
        user_contract::<T>(schedule, contract_steps),
      )
      .expect("new-page Crossing creation must succeed");
    }
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let locator = CrossingMemberships::<T>::get(actor_id).expect("measured membership exists");
    let leaf = CrossingLeafStates::<T>::get(locator.key).expect("Crossing leaf exists");
    assert_max_contract_geometry::<T>(actor_id);
    assert_eq!(locator.page, 1);
    assert_eq!(locator.offset, 0);
    assert_eq!(leaf.page_count, 2);
  }

  #[benchmark]
  fn create_dormant_system_actor() {
    let owner: T::AccountId = whitelisted_caller();
    #[block]
    {
      Pallet::<T>::create_system_actor(RawOrigin::Root.into(), owner, Mutability::Mutable, None)
        .expect("dormant System identity creation must succeed");
    }
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    assert!(benchmark_fixture_contains_scalar_identity::<T>(actor_id));
    assert!(!Pallet::<T>::active_actor_exists(actor_id));
  }

  #[benchmark]
  fn activate_actor() {
    let owner: T::AccountId = whitelisted_caller();
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner.clone(),
      Mutability::Mutable,
      None,
    )
    .expect("dormant System identity creation must succeed");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let recipient: T::AccountId = account("activate-recipient", 0, 0);
    let feed = observation_feed_pool::<T>(1)[0];
    let contract = system_contract::<T>(
      Schedule {
        trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, u128::MAX, 0),
        cooldown_blocks: 100,
      },
      make_max_contract_steps::<T>(recipient),
    );
    #[extrinsic_call]
    activate_actor(
      RawOrigin::Signed(owner),
      actor_id,
      contract.expect("benchmark active contract"),
    );
    assert!(Pallet::<T>::active_actor_exists(actor_id));
    assert_max_contract_geometry::<T>(actor_id);
    assert!(CrossingMemberships::<T>::contains_key(actor_id));
    assert!(ActorObservationFeeds::<T>::get(actor_id).is_none());
  }

  #[benchmark]
  fn deactivate_actor() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    frame_system::Pallet::<T>::set_block_number(0u32.into());
    let owner: T::AccountId = whitelisted_caller();
    let (contract_steps, (asset_a, asset_b, amount_a, amount_b)) = reachable_retry_contract::<T>()?;
    let feed = observation_feed_pool::<T>(1)[0];
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner.clone(),
      Mutability::Mutable,
      system_contract::<T>(
        Schedule {
          trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, u128::MAX, 0),
          cooldown_blocks: 100,
        },
        contract_steps,
      ),
    )
    .expect("System Actors creation must succeed");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let sovereign = Pallet::<T>::actor_identity(actor_id)
      .expect("created actor identity exists")
      .sovereign_account;
    T::AssetOps::mint(&sovereign, asset_a, amount_a).expect("first liquidity leg is funded");
    T::AssetOps::mint(&sovereign, asset_b, amount_b).expect("second liquidity leg is funded");
    let custody_before = (
      T::AssetOps::balance(&sovereign, asset_a),
      T::AssetOps::balance(&sovereign, asset_b),
    );
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    GlobalCircuitBreaker::<T>::put(false);
    Pallet::<T>::note_observation_transition(
      feed,
      ObservationTransition {
        revision: 2,
        previous: Some(u128::MAX - 1),
        current: u128::MAX,
      },
    )
    .expect("real Crossing transition is admitted");
    while CrossingPendingFeedListState::<T>::get().count > 0 {
      Pallet::<T>::crossing_work_unit().expect("Crossing transition materializes");
    }
    assert!(benchmark_fixture_hot::<T>(actor_id).is_some_and(|hot| hot.pending_signal));
    let (_, ready) = Pallet::<T>::actor_control_cell(actor_id)
      .expect("Crossing occurrence retains canonical Ready authority");
    let eligible_at = ready
      .eligible_at
      .expect("Ready authority has an eligibility boundary");
    frame_system::Pallet::<T>::set_block_number(
      frame_system::Pallet::<T>::block_number().max(eligible_at),
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real Crossing Ready fixture passes full state audit");
    Pallet::<T>::execute_cycle(Weight::MAX);
    assert_reachable_retry::<T>(actor_id);
    assert!(
      CrossingMemberships::<T>::contains_key(actor_id),
      "Opening rearmed the detector removed by deactivate"
    );
    assert_eq!(
      (
        T::AssetOps::balance(&sovereign, asset_a),
        T::AssetOps::balance(&sovereign, asset_b)
      ),
      custody_before,
      "rejected liquidity attempt preserves custody"
    );
    #[extrinsic_call]
    deactivate_actor(RawOrigin::Signed(owner), actor_id);
    assert!(!Pallet::<T>::active_actor_exists(actor_id));
    assert!(benchmark_fixture_contains_scalar_identity::<T>(actor_id));
    assert!(
      ActorContractTailChunks::<T>::iter_prefix(actor_id)
        .next()
        .is_none()
    );
    assert!(!CrossingMemberships::<T>::contains_key(actor_id));
    assert!(!ActorRunStateStore::<T>::contains_key(actor_id));
    assert!(!ActorControlLocators::<T>::contains_key(actor_id));
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("Crossing deactivation leaves no orphan state");
    Ok(())
  }

  #[benchmark]
  fn pause_actor() {
    let caller: T::AccountId = whitelisted_caller();
    let actor_id = bench_create_user::<T>(caller.clone());
    #[extrinsic_call]
    pause_actor(RawOrigin::Signed(caller), actor_id);
    let inst =
      Pallet::<T>::active_actor_view(actor_id).expect("Actors must exist after pause_actor");
    assert!(inst.lifecycle.is_paused());
  }

  #[benchmark]
  fn resume_actor() {
    let caller: T::AccountId = whitelisted_caller();
    let actor_id = bench_create_user::<T>(caller.clone());
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    Pallet::<T>::pause_actor(RawOrigin::Signed(caller.clone()).into(), actor_id)
      .expect("pause_actor must succeed in setup");
    frame_system::Pallet::<T>::set_block_number(2u32.into());
    #[extrinsic_call]
    resume_actor(RawOrigin::Signed(caller), actor_id);
    let inst =
      Pallet::<T>::active_actor_view(actor_id).expect("Actors must exist after resume_actor");
    assert!(!inst.lifecycle.is_paused());
  }

  #[benchmark]
  fn manual_trigger() {
    let caller: T::AccountId = whitelisted_caller();
    let actor_id = bench_create_user::<T>(caller.clone());
    #[extrinsic_call]
    manual_trigger(RawOrigin::Signed(caller), actor_id);
    let inst =
      Pallet::<T>::active_actor_view(actor_id).expect("Actors must exist after manual_trigger");
    assert!(inst.pending_signal);
  }

  /// Measures one matched User AddressEvent occurrence without source-publication or funding-state
  /// work: filter detection, exact Trigger capacity/collection, readiness materialization, and
  /// canonical placement are the complete disjoint Actor-owned boundary.
  #[benchmark(pov_mode = Measured)]
  fn address_event_trigger_occurrence() {
    let caller: T::AccountId = whitelisted_caller();
    let recipient: T::AccountId = account("address-event-recipient", 0, 0);
    let contract_steps = make_contract_steps::<T>(recipient);
    ensure_creation_balance::<T>(&caller);
    prefund_active_user_creation::<T>(&caller, &contract_steps);
    Pallet::<T>::create_user_actor(
      RawOrigin::Signed(caller.clone()).into(),
      Mutability::Mutable,
      user_contract::<T>(
        Schedule {
          trigger: Trigger::address_event(SourceFilter::Any, AssetFilter::Any),
          cooldown_blocks: 0,
        },
        contract_steps,
      ),
    )
    .expect("AddressEvent benchmark Actor exists");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    #[block]
    {
      Pallet::<T>::notify_address_event(actor_id, T::FeeNativeAssetId::get(), One::one(), &caller)
        .expect("matched AddressEvent occurrence commits");
    }
    let hot = benchmark_fixture_hot::<T>(actor_id).expect("AddressEvent Actor remains active");
    assert!(hot.pending_signal);
    assert!(hot.queue_ticket.is_some() || hot.wakeup_pointer.is_some());
  }

  /// Measures the exact lifecycle-only cleanup selected when an Idle User cannot admit a paid
  /// pending Pipeline. A one-Step Manual Contract is retained, while Crossing, ObservationChange,
  /// temporal membership, and Run state are intentionally absent from this apoptosis branch.
  /// This direct cleanup fixture does not qualify the affordability decision or other Triggers.
  #[benchmark]
  fn pipeline_admission_apoptosis() {
    let owner: T::AccountId = account("pipeline-apoptosis-owner", 0, 0);
    let recipient: T::AccountId = account("pipeline-apoptosis-recipient", 0, 0);
    let contract_steps = make_contract_steps::<T>(recipient);
    ensure_creation_balance::<T>(&owner);
    prefund_active_user_creation::<T>(&owner, &contract_steps);
    Pallet::<T>::create_user_actor(
      RawOrigin::Signed(owner).into(),
      Mutability::Mutable,
      user_contract::<T>(
        Schedule {
          trigger: Trigger::Manual,
          cooldown_blocks: 0u32.into(),
        },
        contract_steps,
      ),
    )
    .expect("apoptosis benchmark Actor exists");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    Pallet::<T>::request_activation(actor_id).expect("apoptosis readiness must latch");
    let instance = Pallet::<T>::active_actor_view(actor_id).expect("active Actor view exists");
    assert_eq!(instance.cycle_state, CycleState::Idle);
    assert!(instance.pending_signal);
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    assert!(ActorObservationFeeds::<T>::get(actor_id).is_none());
    assert!(CrossingMemberships::<T>::get(actor_id).is_none());
    #[block]
    {
      Pallet::<T>::finalize_actor(actor_id, &instance, CloseReason::CycleAdmissionInsufficient)
        .expect("minimal Pipeline-admission apoptosis must succeed");
    }
    assert!(!Pallet::<T>::active_actor_exists(actor_id));
  }

  #[benchmark]
  fn close_actor() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    // Automatic cleanup reuses this Weight, including the owner's state-hold ledger access.
    let owner: T::AccountId = account("close-owner", 0, 0);
    ensure_creation_balance::<T>(&owner);
    let owner_slot = prefill_reachable_owner_slots::<T>(&owner);
    let feed = observation_feed_pool::<T>(1)[0];
    let schedule = Schedule {
      trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, u128::MAX, 0),
      cooldown_blocks: 100,
    };
    let (contract_steps, retry_funding) = reachable_retry_contract::<T>()?;
    prefund_user_sovereign::<T>(&owner, owner_slot, &contract_steps);
    Pallet::<T>::create_user_actor_at_slot(
      RawOrigin::Signed(owner.clone()).into(),
      owner_slot,
      Mutability::Mutable,
      user_contract::<T>(schedule, contract_steps),
    )
    .expect("create_user_actor_at_slot must succeed in close_actor benchmark setup");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    open_reachable_retry::<T>(actor_id, retry_funding);
    assert_reachable_retry::<T>(actor_id);
    frame_system::Pallet::<T>::set_block_number(
      frame_system::Pallet::<T>::block_number().saturating_add(One::one()),
    );
    publish_zero_step_observation::<T>(feed, 1);
    assert!(CrossingTransitionQueues::<T>::contains_key(feed));
    assert!(!CrossingRangeCursors::<T>::contains_key(feed));
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("reachable close premeasurement state is valid");
    #[extrinsic_call]
    close_actor(RawOrigin::Signed(owner), actor_id);
    assert!(!Pallet::<T>::active_actor_exists(actor_id));
    assert!(
      ActorContractTailChunks::<T>::iter_prefix(actor_id)
        .next()
        .is_none()
    );
    assert!(!CrossingMemberships::<T>::contains_key(actor_id));
    assert!(ActorObservationFeeds::<T>::get(actor_id).is_none());
    assert!(!CrossingTransitionQueues::<T>::contains_key(feed));
    assert!(!CrossingRangeCursors::<T>::contains_key(feed));
    assert!(!CrossingPendingFeeds::<T>::contains_key(feed));
    assert!(!ActorRunStateStore::<T>::contains_key(actor_id));
    assert!(!ActorControlLocators::<T>::contains_key(actor_id));
    assert!(
      ActorContractTailChunks::<T>::iter_prefix(actor_id)
        .next()
        .is_none()
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("close leaves no orphan state");
    Ok(())
  }

  fn prepare_user_retry_opening_partition<T: Config>(
    opening_amounts: u32,
    opening_predicates: u32,
  ) -> Result<ActorId, polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let owner: T::AccountId = account("close-owner", 0, 0);
    ensure_creation_balance::<T>(&owner);
    let owner_slot = prefill_reachable_owner_slots::<T>(&owner);
    let (steps, funding) =
      retry_contract_with_opening_partition::<T>(opening_amounts, opening_predicates)?;
    let feed = observation_feed_pool::<T>(1)[0];
    prefund_user_sovereign::<T>(&owner, owner_slot, &steps);
    Pallet::<T>::create_user_actor_at_slot(
      RawOrigin::Signed(owner).into(),
      owner_slot,
      Mutability::Mutable,
      user_contract::<T>(
        Schedule {
          trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, 4, 1),
          cooldown_blocks: 100,
        },
        steps,
      ),
    )
    .expect("maximum-Step retry Opening partition is ordinarily admitted");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    open_reachable_retry::<T>(actor_id, funding);
    let run = ActorRunStateStore::<T>::get(actor_id).unwrap();
    assert_eq!(run.opening_snapshot.len() as u32, opening_amounts);
    assert_eq!(
      run.opening_predicate_results.len() as u32,
      opening_predicates
    );
    Ok(actor_id)
  }

  /// Maximum Step count with independent Opening captures; other positions retain Current reads.
  #[benchmark(pov_mode = Measured)]
  fn close_actor_opening_partition(
    o: Linear<0, { T::MaxContractSteps::get() * 2 }>,
    p: Linear<0, { T::MaxContractSteps::get() * benchmark_predicate_capacity::<T>() }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_user_retry_opening_partition::<T>(o, p)?;
    let contract = Pallet::<T>::load_actor_contract(actor_id).unwrap();
    let Trigger::ObservationCrossing { feed, .. } = contract.trigger else {
      panic!("partition fixture owns a Crossing Trigger")
    };
    let identity = benchmark_fixture_identity::<T>(actor_id).unwrap();
    let retained_run = ActorRunStateStore::<T>::get(actor_id).unwrap().encode();
    let occurrence_fee = Pallet::<T>::trigger_fee_for_weight(
      ActorType::User,
      TriggerFamily::ObservationCrossing,
      T::WeightInfo::observation_crossing_trigger_occurrence(),
    )
    .trigger_fee;
    T::AssetOps::mint(
      &identity.sovereign_account,
      T::FeeNativeAssetId::get(),
      occurrence_fee,
    )
    .unwrap();
    for value in [1, 4] {
      frame_system::Pallet::<T>::set_block_number(
        frame_system::Pallet::<T>::block_number().saturating_add(One::one()),
      );
      if value == 1 {
        publish_observation_at_most::<T>(feed, 1);
      } else {
        assert!(publish_zero_step_observation::<T>(feed, 1_000) >= value);
      }
      while CrossingPendingFeedListState::<T>::get().count > 0 {
        Pallet::<T>::crossing_work_unit().expect("real retry occurrence materializes");
      }
      assert!(
        matches!(benchmark_fixture_hot::<T>(actor_id).unwrap().trigger_runtime_state,
        TriggerRuntimeState::ObservationCrossing { phase, .. }
          if phase == if value == 1 { CrossingPhase::Armed } else { CrossingPhase::WaitingForRearm }),
        "real transition changes the suspended Actor detector phase"
      );
    }
    assert!(benchmark_fixture_hot::<T>(actor_id).unwrap().pending_signal);
    assert_eq!(
      ActorRunStateStore::<T>::get(actor_id).unwrap().encode(),
      retained_run
    );
    let source = CrossingMemberships::<T>::get(actor_id).unwrap();
    assert_eq!((source.page, source.offset), (0, 0));
    let page_size = T::CrossingPageSize::get();
    for index in 0..2 * page_size - 1 {
      let guard = create_zero_step_crossing_guard::<T>(index, feed, 4);
      assert_eq!(
        CrossingMemberships::<T>::get(guard).unwrap().key,
        source.key
      );
    }
    frame_system::Pallet::<T>::set_block_number(
      frame_system::Pallet::<T>::block_number().saturating_add(One::one()),
    );
    publish_observation_at_most::<T>(feed, 1);
    let mut candidates = 0u32;
    for _ in 0..T::MaxCrossingTransitionsPerBlock::get() {
      let (_, counters) = Pallet::<T>::service_crossing_transitions_with_counters(Weight::MAX);
      assert_eq!(
        (counters.faults, counters.activations, counters.closes),
        (0, 0, 0)
      );
      candidates += counters.candidates;
      if candidates > 1 {
        break;
      }
      frame_system::Pallet::<T>::set_block_number(
        frame_system::Pallet::<T>::block_number().saturating_add(One::one()),
      );
    }
    assert!(candidates > 1);
    let cursor_before = CrossingRangeCursors::<T>::get(feed).unwrap();
    assert_eq!(cursor_before.current_threshold, Some(source.key.threshold));
    assert_eq!(cursor_before.traversal, source.key.traversal);
    assert!((source.page, source.offset) < (cursor_before.page, cursor_before.offset));
    let leaf = CrossingLeafStates::<T>::get(source.key).unwrap();
    assert_eq!(leaf.page_count, 2);
    assert_eq!(leaf.member_count, 2 * page_size - (candidates - 1));
    let tail = CrossingMemberPages::<T>::get(source.key, leaf.tail_page).unwrap();
    assert!(
      (leaf.tail_page, tail.entries.len() as u32 - 1) >= (cursor_before.page, cursor_before.offset)
    );
    let moved_actor = tail.entries.last().unwrap().actor_id;
    assert_ne!(moved_actor, actor_id);
    assert_eq!(
      ActorRunStateStore::<T>::get(actor_id).unwrap().encode(),
      retained_run
    );
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Waiting { .. })
    ));
    assert!(
      ActorRunStateStore::<T>::get(actor_id).unwrap().eligible_at
        > frame_system::Pallet::<T>::block_number()
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("joint retry, deferred latch and real cursor are coherent");
    let native = T::FeeNativeAssetId::get();
    let custody = T::AssetOps::balance(&identity.sovereign_account, native);
    let sink = T::AssetOps::balance(&T::FeeSink::get(), native);
    #[block]
    {
      Pallet::<T>::close_actor(RawOrigin::Signed(identity.owner).into(), actor_id).unwrap();
    }
    assert!(!Pallet::<T>::active_actor_exists(actor_id));
    assert!(!ActorStateHolds::<T>::contains_key(actor_id));
    assert!(!ActorRunStateStore::<T>::contains_key(actor_id));
    assert!(!ActorControlLocators::<T>::contains_key(actor_id));
    assert!(!CrossingMemberships::<T>::contains_key(actor_id));
    assert!(!IndexedTriggerDetectionDisabled::<T>::contains_key(
      actor_id
    ));
    let moved = CrossingMemberships::<T>::get(moved_actor).unwrap();
    assert_eq!((moved.page, moved.offset), (0, 0));
    let mut expected_cursor = cursor_before;
    expected_cursor.page = moved.page;
    expected_cursor.offset = moved.offset;
    assert_eq!(CrossingRangeCursors::<T>::get(feed), Some(expected_cursor));
    assert_eq!(
      CrossingLeafStates::<T>::get(source.key)
        .unwrap()
        .member_count,
      leaf.member_count - 1
    );
    assert!(
      ActorContractTailChunks::<T>::iter_prefix(actor_id)
        .next()
        .is_none()
    );
    assert_eq!(
      T::AssetOps::balance(&identity.sovereign_account, native),
      custody
    );
    assert_eq!(T::AssetOps::balance(&T::FeeSink::get(), native), sink);
    frame_system::Pallet::<T>::set_block_number(
      frame_system::Pallet::<T>::block_number().saturating_add(One::one()),
    );
    let (_, resumed) = Pallet::<T>::service_crossing_transitions_with_counters(Weight::MAX);
    assert_eq!(
      (resumed.faults, resumed.activations, resumed.closes),
      (0, 0, 0)
    );
    assert_ne!(
      CrossingMemberships::<T>::get(moved_actor).unwrap().key,
      moved.key
    );
    assert_eq!(
      benchmark_fixture_identity::<T>(moved_actor)
        .unwrap()
        .cycle_nonce,
      0
    );
    let guard_hot = benchmark_fixture_hot::<T>(moved_actor).unwrap();
    assert!(!guard_hot.pending_signal && guard_hot.queue_ticket.is_none());
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("partitioned retry close leaves no orphan state");
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn close_actor_mixed_waiting_reference()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let fixture = prepare_mixed_waiting_close::<T>(false)?;
    #[block]
    {
      Pallet::<T>::close_actor(RawOrigin::Root.into(), fixture.actor_id).unwrap();
    }
    assert_mixed_waiting_close::<T>(&fixture, false);
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn close_actor_mixed_waiting_unlink()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let fixture = prepare_mixed_waiting_close::<T>(true)?;
    #[block]
    {
      Pallet::<T>::close_actor(RawOrigin::Root.into(), fixture.actor_id).unwrap();
    }
    assert_mixed_waiting_close::<T>(&fixture, true);
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn close_actor_mixed_waiting_deadline()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (fixture, keys) = prepare_mixed_waiting_deadline_close::<T>()?;
    #[block]
    {
      Pallet::<T>::close_actor(RawOrigin::Root.into(), fixture.actor_id).unwrap();
    }
    assert_mixed_waiting_deadline_close::<T>(&fixture, keys);
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn close_actor_full_waiting_upward() {
    let fixture = prepare_full_waiting_heap_close::<T>(true);
    #[block]
    {
      Pallet::<T>::close_actor(RawOrigin::Root.into(), fixture.actor_id).unwrap();
    }
    assert_full_waiting_heap_close::<T>(&fixture);
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn close_actor_full_waiting_tail() {
    let fixture = prepare_full_waiting_heap_close::<T>(false);
    #[block]
    {
      Pallet::<T>::close_actor(RawOrigin::Root.into(), fixture.actor_id).unwrap();
    }
    assert_full_waiting_heap_close::<T>(&fixture);
  }

  // Diagnostic removal branch: delete the last page while the Crossing leaf survives.
  #[benchmark]
  fn close_actor_crossing_page() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let owner: T::AccountId = account("close-owner", 0, 0);
    ensure_creation_balance::<T>(&owner);
    let feed = observation_feed_pool::<T>(1)[0];
    let threshold = u128::MAX;
    for index in 0..T::CrossingPageSize::get() {
      let guard_owner: T::AccountId = account("crossing-remove-page-guard", index, 0);
      let _ = bench_create_system_crossing::<T>(guard_owner, feed, threshold);
    }
    let owner_slot = prefill_reachable_owner_slots::<T>(&owner);
    let (contract_steps, retry_funding) = reachable_retry_contract::<T>()?;
    prefund_user_sovereign::<T>(&owner, owner_slot, &contract_steps);
    let schedule = Schedule {
      trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, threshold, 0),
      cooldown_blocks: 100,
    };
    Pallet::<T>::create_user_actor_at_slot(
      RawOrigin::Signed(owner.clone()).into(),
      owner_slot,
      Mutability::Mutable,
      user_contract::<T>(schedule, contract_steps),
    )
    .expect("page-removal User setup must succeed");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let pause_at = frame_system::Pallet::<T>::block_number().saturating_add(One::one());
    frame_system::Pallet::<T>::set_block_number(pause_at);
    Pallet::<T>::pause_actor(RawOrigin::Signed(owner.clone()).into(), actor_id)
      .expect("target pauses before its real occurrence");
    open_reachable_retry::<T>(actor_id, retry_funding);
    assert_reachable_retry::<T>(actor_id);
    frame_system::Pallet::<T>::set_block_number(
      frame_system::Pallet::<T>::block_number().saturating_add(One::one()),
    );
    let removed = CrossingMemberships::<T>::get(actor_id).expect("tail-page membership exists");
    assert_eq!(removed.page, 1);
    let leaf = CrossingLeafStates::<T>::get(removed.key).expect("full guard leaf survives Opening");
    assert_eq!(leaf.page_count, 2);
    assert_eq!(leaf.member_count, T::CrossingPageSize::get() + 1);
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("reachable close premeasurement state is valid");
    #[block]
    {
      Pallet::<T>::close_actor(RawOrigin::Signed(owner).into(), actor_id)
        .expect("page-removal Crossing close must succeed");
    }
    assert!(!CrossingMemberships::<T>::contains_key(actor_id));
    assert!(!CrossingMemberPages::<T>::contains_key(removed.key, 1));
    let leaf = CrossingLeafStates::<T>::get(removed.key).expect("surviving leaf exists");
    assert_eq!(leaf.tail_page, 0);
    assert_eq!(leaf.page_count, 1);
    assert_eq!(leaf.member_count, T::CrossingPageSize::get());
    assert!(!ActorRunStateStore::<T>::contains_key(actor_id));
    assert!(!ActorControlLocators::<T>::contains_key(actor_id));
    assert!(
      ActorContractTailChunks::<T>::iter_prefix(actor_id)
        .next()
        .is_none()
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("close leaves no orphan state");
    Ok(())
  }

  // Diagnostic removal branch: remove the tail member while its leaf page survives.
  #[benchmark]
  fn close_actor_crossing_tail() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let owner: T::AccountId = account("close-owner", 0, 0);
    ensure_creation_balance::<T>(&owner);
    let feed = observation_feed_pool::<T>(1)[0];
    let threshold = u128::MAX;
    let guard_owner: T::AccountId = account("crossing-tail-guard", 0, 0);
    let guard = bench_create_system_crossing::<T>(guard_owner, feed, threshold);
    let owner_slot = prefill_reachable_owner_slots::<T>(&owner);
    let (contract_steps, retry_funding) = reachable_retry_contract::<T>()?;
    prefund_user_sovereign::<T>(&owner, owner_slot, &contract_steps);
    let schedule = Schedule {
      trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, threshold, 0),
      cooldown_blocks: 100,
    };
    Pallet::<T>::create_user_actor_at_slot(
      RawOrigin::Signed(owner.clone()).into(),
      owner_slot,
      Mutability::Mutable,
      user_contract::<T>(schedule, contract_steps),
    )
    .expect("tail-removal User setup must succeed");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    open_reachable_retry::<T>(actor_id, retry_funding);
    assert_reachable_retry::<T>(actor_id);
    let removed = CrossingMemberships::<T>::get(actor_id).expect("tail membership exists");
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("reachable close premeasurement state is valid");
    #[block]
    {
      Pallet::<T>::close_actor(RawOrigin::Signed(owner).into(), actor_id)
        .expect("tail Crossing close must succeed");
    }
    assert!(!CrossingMemberships::<T>::contains_key(actor_id));
    assert!(CrossingMemberships::<T>::contains_key(guard));
    let leaf = CrossingLeafStates::<T>::get(removed.key).expect("surviving leaf exists");
    assert_eq!(leaf.page_count, 1);
    assert_eq!(leaf.member_count, 1);
    assert!(!ActorRunStateStore::<T>::contains_key(actor_id));
    assert!(!ActorControlLocators::<T>::contains_key(actor_id));
    assert!(
      ActorContractTailChunks::<T>::iter_prefix(actor_id)
        .next()
        .is_none()
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("close leaves no orphan state");
    Ok(())
  }

  // Public close removes a latched User before Opening, preserving a real detector cursor.
  #[benchmark(pov_mode = Measured)]
  fn close_actor_crossing_cursor_repair()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let fixture = prepare_zero_step_crossing_cursor::<T>(true);
    let actor_id = fixture.actor_id;
    let identity = benchmark_fixture_identity::<T>(actor_id).unwrap();
    let native = T::FeeNativeAssetId::get();
    let custody = T::AssetOps::balance(&identity.sovereign_account, native);
    let sink = T::AssetOps::balance(&T::FeeSink::get(), native);
    assert!(ActorStateHolds::<T>::contains_key(actor_id));
    assert!(benchmark_fixture_hot::<T>(actor_id).unwrap().pending_signal);
    frame_system::Pallet::<T>::reset_events();
    #[block]
    {
      Pallet::<T>::close_actor(RawOrigin::Signed(identity.owner).into(), actor_id)
        .expect("public close repairs the published Crossing traversal");
    }
    assert!(!Pallet::<T>::active_actor_exists(actor_id));
    assert!(!ActorStateHolds::<T>::contains_key(actor_id));
    assert!(!ActorRunStateStore::<T>::contains_key(actor_id));
    assert!(!ActorControlLocators::<T>::contains_key(actor_id));
    assert!(!CrossingMemberships::<T>::contains_key(actor_id));
    assert!(!IndexedTriggerDetectionDisabled::<T>::contains_key(
      actor_id
    ));
    assert_eq!(
      T::AssetOps::balance(&identity.sovereign_account, native),
      custody
    );
    assert_eq!(T::AssetOps::balance(&T::FeeSink::get(), native), sink);
    frame_system::Pallet::<T>::assert_has_event(
      Event::<T>::ActorClosed {
        actor_id,
        reason: CloseReason::OwnerInitiated,
      }
      .into(),
    );
    assert_zero_step_crossing_cursor_topology::<T>(&fixture, true);
    Ok(())
  }

  // Diagnostic removal branch: remove a dense middle member and repair the moved tail locator.
  #[benchmark]
  fn close_actor_crossing_middle() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let owner: T::AccountId = account("close-owner", 0, 0);
    ensure_creation_balance::<T>(&owner);
    let feed = observation_feed_pool::<T>(1)[0];
    let threshold = u128::MAX;
    let guard_owner: T::AccountId = account("crossing-middle-guard", 0, 0);
    let _ = bench_create_system_crossing::<T>(guard_owner, feed, threshold);
    let owner_slot = prefill_reachable_owner_slots::<T>(&owner);
    let (contract_steps, retry_funding) = reachable_retry_contract::<T>()?;
    prefund_user_sovereign::<T>(&owner, owner_slot, &contract_steps);
    let schedule = Schedule {
      trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, threshold, 0),
      cooldown_blocks: 100,
    };
    Pallet::<T>::create_user_actor_at_slot(
      RawOrigin::Signed(owner.clone()).into(),
      owner_slot,
      Mutability::Mutable,
      user_contract::<T>(schedule, contract_steps),
    )
    .expect("middle-removal User setup must succeed");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    open_reachable_retry::<T>(actor_id, retry_funding);
    assert_reachable_retry::<T>(actor_id);
    let tail_owner: T::AccountId = account("crossing-middle-tail", 0, 0);
    let tail_id = bench_create_system_crossing::<T>(tail_owner, feed, threshold);
    let removed = CrossingMemberships::<T>::get(actor_id).expect("middle membership exists");
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("reachable close premeasurement state is valid");
    #[block]
    {
      Pallet::<T>::close_actor(RawOrigin::Signed(owner).into(), actor_id)
        .expect("middle Crossing close must succeed");
    }
    assert!(!CrossingMemberships::<T>::contains_key(actor_id));
    let moved = CrossingMemberships::<T>::get(tail_id).expect("moved tail membership exists");
    assert_eq!(moved.page, removed.page);
    assert_eq!(moved.offset, removed.offset);
    assert!(!ActorRunStateStore::<T>::contains_key(actor_id));
    assert!(!ActorControlLocators::<T>::contains_key(actor_id));
    assert!(
      ActorContractTailChunks::<T>::iter_prefix(actor_id)
        .next()
        .is_none()
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("close leaves no orphan state");
    Ok(())
  }

  // Diagnostic counterpart for broad ObservationChange cleanup; compare it with the production
  // Crossing close before accepting one conservative public close owner.
  #[benchmark]
  fn close_actor_observation_change() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError>
  {
    let owner: T::AccountId = account("close-owner", 0, 0);
    ensure_creation_balance::<T>(&owner);
    let owner_slot = prefill_reachable_owner_slots::<T>(&owner);
    let feeds = observation_feed_pool::<T>(3);
    let measured = feeds[1];
    install_observation_guard::<T>(feeds[0], 0);
    install_observation_guard::<T>(feeds[feeds.len() - 1], 1);
    let schedule = Schedule {
      trigger: observation_trigger::<T>(measured),
      cooldown_blocks: 100,
    };
    let (contract_steps, retry_funding) = reachable_retry_contract::<T>()?;
    prefund_user_sovereign::<T>(&owner, owner_slot, &contract_steps);
    Pallet::<T>::create_user_actor_at_slot(
      RawOrigin::Signed(owner.clone()).into(),
      owner_slot,
      Mutability::Mutable,
      user_contract::<T>(schedule, contract_steps),
    )
    .expect("ObservationChange close benchmark setup must succeed");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    open_reachable_retry::<T>(actor_id, retry_funding);
    assert_reachable_retry::<T>(actor_id);
    for feed in &feeds {
      Pallet::<T>::note_observation_changed(*feed, 3).expect("close dirty topology is admitted");
    }
    assert_eq!(DirtyObservationListState::<T>::get().count, 3);
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("reachable close premeasurement state is valid");
    #[block]
    {
      Pallet::<T>::close_actor(RawOrigin::Signed(owner).into(), actor_id)
        .expect("ObservationChange close must succeed");
    }
    assert!(!Pallet::<T>::active_actor_exists(actor_id));
    assert_eq!(DirtyObservationListState::<T>::get().count, 2);
    assert!(!ActorRunStateStore::<T>::contains_key(actor_id));
    assert!(!ActorControlLocators::<T>::contains_key(actor_id));
    assert!(
      ActorContractTailChunks::<T>::iter_prefix(actor_id)
        .next()
        .is_none()
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("close leaves no orphan state");
    Ok(())
  }

  // Diagnostic counterpart for the System branch; production close pricing uses the heavier User path.
  #[benchmark]
  fn close_actor_system_pure() {
    let owner: T::AccountId = whitelisted_caller();
    let recipient: T::AccountId = account("system-close-recipient", 0, 0);
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 1,
    };
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      system_contract::<T>(schedule, make_contract_steps::<T>(recipient)),
    )
    .expect("create_system_actor must succeed in System close benchmark setup");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    #[block]
    {
      Pallet::<T>::close_actor(RawOrigin::Root.into(), actor_id)
        .expect("System close must succeed in benchmark");
    }
    assert!(!Pallet::<T>::active_actor_exists(actor_id));
  }

  #[benchmark]
  fn update_contract_observation_change()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    if T::MaxFundingTrackedAssets::get() == 0 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent the nonempty funding-prune ObservationChange profile",
      ));
    }
    let opening_legs = T::MaxContractSteps::get()
      .saturating_mul(2)
      .saturating_sub(1);
    let (owner, actor_id, replacement, old_feed) =
      prepare_reachable_update::<T>(opening_legs, TriggerFamily::ObservationChange)?;
    let expected = replacement.clone();
    #[block]
    {
      execute_reachable_update::<T>(owner, actor_id, replacement);
    }
    assert_reachable_update::<T>(actor_id, &expected, old_feed);
    Ok(())
  }

  // Reachable funding-heavy reference corner, not a proved envelope over the allocation frontier.
  // Production Weight selection remains open until the extra diagnostic is measured in Wasm.
  #[benchmark]
  fn update_contract() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let opening_legs = T::MaxContractSteps::get()
      .saturating_mul(2)
      .saturating_sub(T::MaxFundingTrackedAssets::get());
    let (owner, actor_id, replacement, old_feed) =
      prepare_reachable_update::<T>(opening_legs, TriggerFamily::ObservationCrossing)?;
    let expected = replacement.clone();
    #[block]
    {
      execute_reachable_update::<T>(owner, actor_id, replacement);
    }
    assert_reachable_update::<T>(actor_id, &expected, old_feed);
    Ok(())
  }

  #[benchmark(extra)]
  fn update_contract_reachable_allocation(
    o: Linear<
      {
        T::MaxContractSteps::get()
          .saturating_mul(2)
          .saturating_sub(T::MaxFundingTrackedAssets::get())
      },
      { T::MaxContractSteps::get().saturating_mul(2) },
    >,
    t: Linear<0, 1>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let family = if t == 0 {
      TriggerFamily::ObservationCrossing
    } else {
      TriggerFamily::ObservationChange
    };
    let (owner, actor_id, replacement, old_feed) = prepare_reachable_update::<T>(o, family)?;
    let expected = replacement.clone();
    #[block]
    {
      execute_reachable_update::<T>(owner, actor_id, replacement);
    }
    assert_reachable_update::<T>(actor_id, &expected, old_feed);
    Ok(())
  }

  #[benchmark]
  fn set_global_circuit_breaker() {
    #[extrinsic_call]
    set_global_circuit_breaker(RawOrigin::Root, true);
    assert!(GlobalCircuitBreaker::<T>::get());
  }

  #[benchmark]
  fn record_crossing_worker_fault() {
    let fault = CrossingWorkerFault {
      feed: observation_feed_pool::<T>(1)[0],
      revision: Some(u64::MAX),
      threshold: Some(u128::MAX),
      class: CrossingWorkerFaultClass::Other,
    };
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    #[block]
    {
      assert!(Pallet::<T>::record_crossing_worker_fault(&mut meter, fault));
    }
    assert_eq!(CrossingWorkerFaultState::<T>::get(), Some(fault));
  }

  #[benchmark]
  fn record_observation_fanout_worker_fault() {
    let feed = observation_feed_pool::<T>(1)[0];
    let owner: T::AccountId = account("observation-fault", 0, 0);
    let actor_id = bench_create_system_observation::<T>(owner, feed);
    let fault = ObservationFanoutWorkerFault {
      feed,
      revision: u64::MAX,
      subscriber_page: Some(u32::MAX),
      subscriber_position: u32::MAX,
      actor_id: Some(actor_id),
      semantic_contract_id: Some([u8::MAX; 32]),
      body_commitment: Some([u8::MAX; 32]),
      admission_identity: Some([u8::MAX; 32]),
      branch: ObservationFanoutBranch::Terminal,
      class: CrossingWorkerFaultClass::Other,
    };
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    #[block]
    {
      core::hint::black_box(ObservationSubscriberPages::<T>::get(feed, 0));
      core::hint::black_box(ActorActivationAuthorities::<T>::get(actor_id));
      assert!(Pallet::<T>::record_observation_fanout_worker_fault(
        &mut meter, fault
      ));
    }
    assert_eq!(ObservationFanoutWorkerFaultState::<T>::get(), Some(fault));
  }

  #[benchmark]
  fn record_wakeup_worker_fault() {
    let fault = WakeupWorkerFault {
      key: WakeupKey::Tick(u64::MAX),
      page: u64::MAX,
      class: CrossingWorkerFaultClass::Other,
    };
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    #[block]
    {
      assert!(Pallet::<T>::record_wakeup_worker_fault(&mut meter, fault));
    }
    assert_eq!(WakeupWorkerFaultState::<T>::get(), Some(fault));
  }

  #[benchmark]
  fn clear_crossing_worker_fault() {
    CrossingWorkerFaultState::<T>::put(CrossingWorkerFault {
      feed: observation_feed_pool::<T>(1)[0],
      revision: Some(1),
      threshold: Some(1),
      class: CrossingWorkerFaultClass::Invariant,
    });
    #[extrinsic_call]
    clear_crossing_worker_fault(RawOrigin::Root);
    assert!(CrossingWorkerFaultState::<T>::get().is_none());
  }

  #[benchmark]
  fn clear_observation_fanout_worker_fault() {
    ObservationFanoutWorkerFaultState::<T>::put(ObservationFanoutWorkerFault {
      feed: observation_feed_pool::<T>(1)[0],
      revision: 1,
      subscriber_page: Some(0),
      subscriber_position: 0,
      actor_id: None,
      semantic_contract_id: None,
      body_commitment: None,
      admission_identity: None,
      branch: ObservationFanoutBranch::Ordinary,
      class: CrossingWorkerFaultClass::Invariant,
    });
    #[extrinsic_call]
    clear_observation_fanout_worker_fault(RawOrigin::Root);
    assert!(ObservationFanoutWorkerFaultState::<T>::get().is_none());
  }

  #[benchmark]
  fn clear_wakeup_worker_fault() {
    WakeupWorkerFaultState::<T>::put(WakeupWorkerFault {
      key: WakeupKey::Block(1u32.into()),
      page: 0,
      class: CrossingWorkerFaultClass::Invariant,
    });
    #[extrinsic_call]
    clear_wakeup_worker_fault(RawOrigin::Root);
    assert!(WakeupWorkerFaultState::<T>::get().is_none());
  }

  #[benchmark]
  fn set_active_actor_limit() {
    let limit = Pallet::<T>::max_configurable_active_actor_limit();
    #[extrinsic_call]
    set_active_actor_limit(RawOrigin::Root, limit);
    assert_eq!(ActiveActorLimit::<T>::get(), limit);
  }

  #[benchmark]
  fn permissionless_sweep() {
    let caller: T::AccountId = whitelisted_caller();
    let actor_id = bench_create_user::<T>(caller.clone());
    #[extrinsic_call]
    permissionless_sweep(RawOrigin::Signed(caller), actor_id);
    assert!(Pallet::<T>::active_actor_exists(actor_id));
  }

  #[benchmark]
  fn permissionless_sweep_many(n: Linear<1, 5>) {
    let caller: T::AccountId = whitelisted_caller();
    let mut actor_ids: BoundedVec<ActorId, T::MaxSweepBatch> = BoundedVec::default();
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 10,
    };
    let bounded_n = n.min(T::MaxSweepBatch::get());
    for i in 0..bounded_n {
      let owner: T::AccountId = account("sweep-owner", i, 0);
      let recipient: T::AccountId = account("sweep-recipient", i, 0);
      ensure_creation_balance::<T>(&owner);
      let contract_steps = make_contract_steps::<T>(recipient);
      prefund_active_user_creation::<T>(&owner, &contract_steps);
      Pallet::<T>::create_user_actor(
        RawOrigin::Signed(owner).into(),
        Mutability::Mutable,
        user_contract::<T>(schedule.clone(), contract_steps),
      )
      .expect("create_user_actor must succeed in permissionless_sweep_many setup");
      let actor_id = NextActorId::<T>::get().saturating_sub(1);
      // Permissionless sweep no longer predicts economic affordability. Use the canonical
      // nonce terminal so every requested live Actor still exercises bounded close cleanup.
      benchmark_fixture_mutate_identity::<T>(actor_id, |identity| {
        identity.cycle_nonce = u64::MAX;
      });
      benchmark_fixture_align_primary_control::<T>(actor_id);
      actor_ids
        .try_push(actor_id)
        .expect("benchmark n must fit MaxSweepBatch");
    }
    let expected_len = actor_ids.len();
    #[extrinsic_call]
    permissionless_sweep_many(RawOrigin::Signed(caller), actor_ids.clone());
    for actor_id in actor_ids {
      assert!(!Pallet::<T>::active_actor_exists(actor_id));
    }
    assert_eq!(expected_len, bounded_n as usize);
  }

  #[benchmark]
  fn fee_collection() {
    let payer: T::AccountId = account("fee-payer", 0, 0);
    let fee_sink = T::FeeSink::get();
    assert_ne!(payer, fee_sink);
    let native = T::FeeNativeAssetId::get();
    let minimum = T::AssetOps::minimum_balance(native);
    let amount = T::MinUserBalance::get()
      .max(minimum)
      .saturating_add(One::one());
    T::AssetOps::mint(&payer, native, amount.saturating_mul(2u32.into()))
      .expect("fee-collection benchmark payer must be funded");
    T::AssetOps::mint(&fee_sink, native, minimum.max(One::one()))
      .expect("configured fee sink must retain its account minimum");
    let payer_before = T::AssetOps::balance(&payer, native);
    let sink_before = T::AssetOps::balance(&fee_sink, native);
    #[block]
    {
      Pallet::<T>::collect_user_step_fee(&payer, amount).expect("fee collection must succeed");
    }
    assert_eq!(T::AssetOps::balance(&payer, native), payer_before - amount);
    assert_eq!(
      T::AssetOps::balance(&fee_sink, native),
      sink_before + amount
    );
  }

  /// Isolated invocation-receipt owner at maximum encoded field width. No Actor lifecycle,
  /// Task effect, fee transfer or state-hold work is included in this measurement.
  #[benchmark]
  fn action_invocation_receipt() {
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    let fee = <T::Balance as polkadot_sdk::sp_runtime::traits::Bounded>::max_value();
    let expected = Event::<T>::ActionFeeCharged {
      actor_id: ActorId::MAX,
      cycle_nonce: u64::MAX,
      step_index: u32::MAX,
      actual_effect_weight: Weight::MAX,
      fee,
    };
    #[block]
    {
      Pallet::<T>::deposit_action_fee_receipt(ActorId::MAX, u64::MAX, u32::MAX, Weight::MAX, fee);
    }
    frame_system::Pallet::<T>::assert_last_event(expected.into());
  }

  #[benchmark]
  fn task_transfer() {
    let caller: T::AccountId = account("transfer-caller", 0, 0);
    let (target_id, recipient) = prepare_saturated_address_actor::<T>(0, Some(caller.clone()));
    let native = T::FeeNativeAssetId::get();
    let amount = T::MinUserBalance::get().saturating_add(One::one());
    T::AssetOps::mint(&caller, native, amount.saturating_mul(2u32.into()))
      .expect("simple-transfer benchmark caller must be funded");
    T::BenchmarkHelper::enable_asset_ops_ingress();
    #[block]
    {
      T::AssetOps::transfer(&caller, &recipient, native, amount)
        .expect("ingress-aware transfer must succeed");
    }
    assert!(benchmark_fixture_hot::<T>(target_id).is_some_and(|hot| hot.wakeup_pointer.is_some()));
  }

  #[benchmark]
  fn task_burn() {
    let caller: T::AccountId = account("burn-caller", 0, 0);
    let native = T::FeeNativeAssetId::get();
    let amount = T::MinUserBalance::get().saturating_add(One::one());
    T::AssetOps::mint(&caller, native, amount.saturating_mul(2u32.into()))
      .expect("burn benchmark caller must be funded");
    let before = T::AssetOps::balance(&caller, native);
    #[block]
    {
      T::AssetOps::burn(&caller, native, amount).expect("burn must succeed");
    }
    assert_eq!(
      T::AssetOps::balance(&caller, native),
      before.saturating_sub(amount)
    );
  }

  #[benchmark]
  fn task_mint() {
    let (target_id, recipient) = prepare_saturated_address_actor::<T>(0, None);
    let native = T::FeeNativeAssetId::get();
    let amount = T::MinUserBalance::get().saturating_add(One::one());
    T::BenchmarkHelper::enable_asset_ops_ingress();
    #[block]
    {
      T::AssetOps::mint(&recipient, native, amount).expect("ingress-aware mint must succeed");
    }
    assert!(benchmark_fixture_hot::<T>(target_id).is_some_and(|hot| hot.wakeup_pointer.is_some()));
  }

  #[benchmark]
  fn precondition_all_max() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    if benchmark_predicate_capacity::<T>() == 0 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host has no representable predicate capacity for this benchmark",
      ));
    }
    let actor: T::AccountId = account("condition-all", 0, 0);
    let max_predicates = benchmark_predicate_capacity::<T>();
    let assets = T::BenchmarkHelper::setup_predicate_assets(&actor, max_predicates)
      .expect("predicate benchmark assets must be available");
    assert!(assets.len() >= max_predicates as usize);
    let predicates = assets
      .into_iter()
      .take(max_predicates as usize)
      .map(|asset| {
        T::AssetOps::mint(&actor, asset, T::MinUserBalance::get())
          .expect("condition benchmark asset must be funded");
        Predicate::BalanceAbove {
          asset,
          threshold: T::Balance::zero(),
        }
      })
      .collect::<alloc::vec::Vec<_>>();
    let precondition = packed_predicate_clauses::<T>(
      predicates
        .into_iter()
        .map(|predicate| TimedPredicate {
          timing: ObservationTiming::Current,
          predicate,
        })
        .collect(),
      T::MaxPredicatesPerClause::get(),
    );
    #[block]
    {
      assert_eq!(
        Pallet::<T>::evaluate_precondition(&precondition, &actor, T::Balance::zero()),
        Ok(true)
      );
    }
    Ok(())
  }

  #[benchmark]
  fn precondition_observation(
    c: Linear<1, { benchmark_predicate_capacity::<T>().max(1) }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    if benchmark_predicate_capacity::<T>() == 0 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host has no representable predicate capacity for this benchmark",
      ));
    }
    let actor: T::AccountId = account("condition-observation", 0, 0);
    let bounded = c;
    let feeds = T::BenchmarkHelper::setup_observation_feeds(bounded)
      .expect("observation benchmark feeds must be available");
    assert!(feeds.len() >= bounded as usize);
    let predicates = feeds
      .into_iter()
      .take(bounded as usize)
      .map(|feed| Predicate::ObservationAbove {
        feed,
        threshold: 0,
        max_age_blocks: 100,
      })
      .collect::<alloc::vec::Vec<_>>();
    let precondition = packed_predicate_clauses::<T>(
      predicates
        .into_iter()
        .map(|predicate| TimedPredicate {
          timing: ObservationTiming::Current,
          predicate,
        })
        .collect(),
      T::MaxPredicatesPerClause::get(),
    );
    #[block]
    {
      let _ = Pallet::<T>::evaluate_precondition(&precondition, &actor, T::Balance::zero());
    }
    Ok(())
  }

  #[benchmark(pov_mode = Measured)]
  fn predicate_set_evaluation(
    c: Linear<1, { benchmark_predicate_capacity::<T>().max(1) }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let observations = u32::from(c >= 2);
    let (actor, precondition) =
      prepare_max_encoded_step_precondition::<T>(c - observations, observations, false)?;
    let result;
    #[block]
    {
      result = Pallet::<T>::evaluate_precondition(&precondition, &actor, Zero::zero());
      core::hint::black_box(&result);
    }
    assert_eq!(result, Ok(true));
    Ok(())
  }

  #[benchmark]
  fn task_stop_cycle() {
    let before = frame_system::Pallet::<T>::event_count();
    #[block]
    {
      Pallet::<T>::record_stop_cycle_event(1, 1, 0);
    }
    assert_eq!(frame_system::Pallet::<T>::event_count(), before + 1);
  }

  #[benchmark]
  fn task_split_transfer(l: Linear<2, 8>) {
    let caller: T::AccountId = whitelisted_caller();
    let bounded_legs = l.min(T::MaxSplitTransferLegs::get());
    let native = T::FeeNativeAssetId::get();
    let amount = T::MinUserBalance::get().saturating_add(One::one());
    let mut targets: alloc::vec::Vec<(ActorId, T::AccountId)> = alloc::vec::Vec::new();
    for seed in 0..bounded_legs {
      targets.push(prepare_saturated_address_actor::<T>(
        seed,
        Some(caller.clone()),
      ));
    }
    let total = amount
      .saturating_mul(bounded_legs.into())
      .saturating_add(T::MinUserBalance::get());
    T::AssetOps::mint(&caller, native, total)
      .expect("split-transfer benchmark caller must be funded");
    T::BenchmarkHelper::enable_asset_ops_ingress();
    #[block]
    {
      for (_, recipient) in &targets {
        T::AssetOps::transfer(&caller, recipient, native, amount)
          .expect("ingress-aware split leg must succeed");
      }
    }
    for (target_id, _) in targets {
      assert!(
        benchmark_fixture_hot::<T>(target_id).is_some_and(|hot| hot.wakeup_pointer.is_some())
      );
    }
  }

  #[benchmark]
  fn xcm_asset_deposit() {
    T::BenchmarkHelper::setup_xcm_asset_deposit()
      .expect("XCM deposit benchmark asset must be registered");
    let source: T::AccountId = account("xcm-source", 0, 0);
    let (target_id, recipient) = prepare_saturated_address_actor::<T>(0, Some(source.clone()));
    let amount = T::MinUserBalance::get().saturating_add(One::one());
    #[block]
    {
      T::BenchmarkHelper::run_xcm_asset_deposit(&recipient, &source, amount)
        .expect("Actors-aware XCM deposit must succeed");
    }
    assert!(benchmark_fixture_hot::<T>(target_id).is_some_and(|hot| hot.wakeup_pointer.is_some()));
  }

  #[benchmark]
  fn task_add_liquidity() {
    let caller: T::AccountId = whitelisted_caller();
    let (asset_a, asset_b, amount_a, amount_b) = T::BenchmarkHelper::setup_add_liquidity(&caller)
      .expect("benchmark helper must prepare add-liquidity state");
    #[block]
    {
      T::LiquidityOps::add_liquidity(&caller, asset_a, asset_b, amount_a, amount_b, One::one())
        .expect("add-liquidity benchmark operation must succeed");
    }
  }

  #[benchmark]
  fn task_donate_liquidity() {
    let caller: T::AccountId = whitelisted_caller();
    let (asset_a, asset_b, amount) = T::BenchmarkHelper::setup_donate_liquidity(&caller)
      .expect("benchmark helper must prepare liquidity-donation state");
    #[block]
    {
      T::LiquidityOps::donate_liquidity(&caller, asset_a, asset_b, amount, amount, Perbill::zero())
        .expect("liquidity-donation benchmark operation must succeed");
    }
  }

  #[benchmark]
  fn task_remove_liquidity() {
    let caller: T::AccountId = whitelisted_caller();
    let (lp_asset, asset_a, asset_b, lp_amount) =
      T::BenchmarkHelper::setup_remove_liquidity(&caller)
        .expect("benchmark helper must prepare indexed remove-liquidity state");
    #[block]
    {
      T::LiquidityOps::remove_liquidity(
        &caller,
        lp_asset,
        asset_a,
        asset_b,
        lp_amount,
        One::one(),
        One::one(),
      )
      .expect("remove-liquidity benchmark operation must succeed");
    }
  }

  #[benchmark]
  fn task_stake() {
    let caller: T::AccountId = whitelisted_caller();
    let (asset, amount) = T::BenchmarkHelper::setup_stake(&caller)
      .expect("benchmark helper must prepare staking state");
    #[block]
    {
      T::StakingOps::stake(&caller, asset, amount)
        .expect("staking benchmark operation must succeed");
    }
  }

  #[benchmark]
  fn task_unstake() {
    let caller: T::AccountId = whitelisted_caller();
    let (asset, shares) = T::BenchmarkHelper::setup_unstake(&caller)
      .expect("benchmark helper must prepare unstaking state");
    #[block]
    {
      T::StakingOps::unstake(&caller, asset, shares)
        .expect("unstaking benchmark operation must succeed");
    }
  }

  #[benchmark]
  fn task_dex_exact_in() {
    let caller: T::AccountId = whitelisted_caller();
    let (asset_in, asset_out, amount_in) = T::BenchmarkHelper::setup_swap_exact_in(&caller)
      .expect("benchmark helper must prepare exact-input swap state");
    #[block]
    {
      // System is the heavier branch: it additionally runs the typed reference-deviation guard,
      // which reads the Oracle observation and the pool reserves before executing.
      T::DexOps::swap_exact_in(
        ExecutionContext::new(&caller, ActorType::System),
        asset_in,
        asset_out,
        amount_in,
        Perbill::zero(),
      )
      .expect("exact-input benchmark swap must succeed");
    }
  }

  #[benchmark]
  fn task_dex_exact_out() {
    let caller: T::AccountId = whitelisted_caller();
    let (asset_in, asset_out, amount_out, max_amount_in) =
      T::BenchmarkHelper::setup_swap_exact_out(&caller)
        .expect("benchmark helper must prepare exact-output swap state");
    #[block]
    {
      // System is the heavier branch: see `task_dex_exact_in`.
      T::DexOps::swap_exact_out(
        ExecutionContext::new(&caller, ActorType::System),
        asset_in,
        asset_out,
        amount_out,
        max_amount_in,
        Perbill::zero(),
      )
      .expect("exact-output benchmark swap must succeed");
    }
  }

  // Non-dispatch diagnostic benchmark excluded from runtime weight artifact generation
  #[benchmark]
  fn process_remove_liquidity_indexed() {
    let caller: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&caller);
    let (lp_asset, asset_a, asset_b, lp_amount) =
      T::BenchmarkHelper::setup_remove_liquidity(&caller)
        .expect("benchmark helper must prepare indexed remove-liquidity state");
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 10,
    };
    let contract_steps =
      make_remove_liquidity_contract_steps::<T>(lp_asset, asset_a, asset_b, lp_amount);
    prefund_active_user_creation::<T>(&caller, &contract_steps);
    Pallet::<T>::create_user_actor(
      RawOrigin::Signed(caller.clone()).into(),
      Mutability::Mutable,
      user_contract::<T>(schedule, contract_steps),
    )
    .expect("create_user_actor must succeed in setup");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let actor = Pallet::<T>::active_actor_view(actor_id)
      .map(|instance| instance.sovereign_account)
      .expect("actor must exist after setup");
    seed_actor_for_cycle::<T>(actor_id);
    T::AssetOps::transfer(&caller, &actor, lp_asset, lp_amount)
      .expect("LP transfer to actor must succeed");
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    Pallet::<T>::manual_trigger(RawOrigin::Signed(caller).into(), actor_id)
      .expect("manual_trigger must succeed in setup");
    #[block]
    {
      let _ = Pallet::<T>::on_idle(1u32.into(), Weight::MAX);
    }
    let inst =
      Pallet::<T>::active_actor_view(actor_id).expect("actor must survive benchmark cycle");
    assert_eq!(inst.cycle_nonce, 1);
    assert_eq!(inst.unsuccessful_attempt_streak, 0);
  }

  fn make_inert_contract_steps<T: Config>() -> ContractSteps<T> {
    let step = Step {
      precondition: None,
      task: ActorTask::StopCycle,
      on_error: StepErrorPolicy::AbortCycle,
    };
    BoundedVec::try_from(vec![step]).expect("single-step contract_steps must fit")
  }

  fn inert_contract_steps_of_len<T: Config>(steps: u32) -> ContractSteps<T> {
    let bounded = steps.min(T::MaxContractSteps::get());
    let mut plan = alloc::vec::Vec::new();
    for _ in 0..bounded {
      plan.push(Step {
        precondition: None,
        task: ActorTask::StopCycle,
        on_error: StepErrorPolicy::AbortCycle,
      });
    }
    BoundedVec::try_from(plan).expect("benchmark inert contract_steps must fit")
  }

  fn bench_create_system_with_plan<T: Config>(
    seed: u32,
    contract_steps: ContractSteps<T>,
  ) -> ActorId {
    let owner: T::AccountId = account("cycle_owner", seed, 0);
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 0,
    };
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      system_contract::<T>(schedule, contract_steps),
    )
    .expect("create_system_actor must succeed in cycle benchmark setup");
    NextActorId::<T>::get().saturating_sub(1)
  }

  fn admitted_contract_geometry<T: Config>(
    actor_id: ActorId,
    contract: &ActorContractOf<T>,
  ) -> (
    ActorAdmissionCertificateOf<T>,
    ActorContractHeadOf<T>,
    Vec<(u32, ActorStepChunkOf<T>)>,
  ) {
    let certificate = ActorAdmissionCertificate::new(
      contract.semantic_contract_id(),
      contract
        .body_commitment()
        .expect("bounded benchmark body commitment"),
      1,
      [2u8; 32],
      1,
      [3u8; 32],
      Weight::zero(),
    );
    let (head, chunks) = Pallet::<T>::decompose_admitted_contract_geometry(
      actor_id,
      ActorType::System,
      contract,
      &certificate,
    )
    .expect("admitted benchmark Contract decomposes");
    (certificate, head, chunks)
  }

  fn install_chunked_contract<T: Config>(actor_id: ActorId, contract: &ActorContractOf<T>) {
    let identity = benchmark_fixture_identity::<T>(actor_id).expect("benchmark identity exists");
    let certificate =
      benchmark_fixture_admission::<T>(actor_id).expect("benchmark canonical admission exists");
    let (head, chunks) = Pallet::<T>::decompose_admitted_contract_geometry(
      actor_id,
      identity.actor_class.actor_type(),
      contract,
      &certificate,
    )
    .expect("benchmark canonical Contract decomposes");
    ActorContractHeads::<T>::insert(actor_id, head);
    for (chunk_index, chunk) in chunks {
      ActorContractTailChunks::<T>::insert(actor_id, chunk_index, chunk);
    }
  }

  fn bench_create_user_observation_with_cooldown<T: Config>(
    owner: T::AccountId,
    feed: T::ObservationFeedId,
    cooldown_blocks: u32,
  ) -> ActorId {
    let contract_steps = make_inert_contract_steps::<T>();
    ensure_creation_balance::<T>(&owner);
    prefund_active_user_creation::<T>(&owner, &contract_steps);
    Pallet::<T>::create_user_actor(
      RawOrigin::Signed(owner).into(),
      Mutability::Mutable,
      user_contract::<T>(
        Schedule {
          trigger: Trigger::observation_change(feed),
          cooldown_blocks,
        },
        contract_steps,
      ),
    )
    .expect("User observation benchmark actor creation must succeed");
    NextActorId::<T>::get().saturating_sub(1)
  }

  fn bench_create_system_observation<T: Config>(
    owner: T::AccountId,
    feed: T::ObservationFeedId,
  ) -> ActorId {
    bench_create_system_observation_with_cooldown::<T>(owner, feed, 0)
  }

  fn bench_create_system_observation_with_cooldown<T: Config>(
    owner: T::AccountId,
    feed: T::ObservationFeedId,
    cooldown_blocks: u32,
  ) -> ActorId {
    let schedule = Schedule {
      trigger: Trigger::observation_change(feed),
      cooldown_blocks,
    };
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      system_contract::<T>(schedule, make_inert_contract_steps::<T>()),
    )
    .expect("observation benchmark actor creation must succeed");
    NextActorId::<T>::get().saturating_sub(1)
  }

  fn bench_create_expiring_system_observation<T: Config>(
    owner: T::AccountId,
    feed: T::ObservationFeedId,
  ) -> (ActorId, BlockNumberFor<T>) {
    let now = frame_system::Pallet::<T>::block_number();
    let end = now.saturating_add(T::MinWindowLength::get());
    let schedule = Schedule {
      trigger: Trigger::observation_change(feed),
      cooldown_blocks: 0,
    };
    let mut contract = system_contract::<T>(schedule, make_inert_contract_steps::<T>())
      .expect("system observation contract exists");
    contract.window = Some(ScheduleWindow { start: now, end });
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      Some(contract),
    )
    .expect("expiring observation benchmark actor creation must succeed");
    (NextActorId::<T>::get().saturating_sub(1), end)
  }

  fn bench_create_system_crossing<T: Config>(
    owner: T::AccountId,
    feed: T::ObservationFeedId,
    threshold: u128,
  ) -> ActorId {
    let schedule = Schedule {
      trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, threshold, 0),
      cooldown_blocks: 0,
    };
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      system_contract::<T>(schedule, make_inert_contract_steps::<T>()),
    )
    .expect("Crossing benchmark actor creation must succeed");
    NextActorId::<T>::get().saturating_sub(1)
  }

  fn clear_indexed_detection_disablement<T: Config>() {
    let actor_ids = IndexedTriggerDetectionDisabled::<T>::iter_keys().collect::<Vec<_>>();
    for actor_id in actor_ids {
      IndexedTriggerDetectionDisabled::<T>::remove(actor_id);
    }
  }

  fn prepare_crossing_work<T: Config>(threshold: u128) -> (T::ObservationFeedId, ActorId) {
    let feed = T::BenchmarkHelper::setup_observation_feeds(1)
      .expect("Crossing benchmark feed must be available")
      .into_iter()
      .next()
      .expect("one Crossing benchmark feed is required");
    let owner: T::AccountId = account("crossing-worker", 0, 0);
    let actor_id = bench_create_user_with_trigger::<T>(
      owner,
      Trigger::observation_crossing(feed, CrossingDirection::Rising, threshold, 0),
    );
    Pallet::<T>::note_observation_transition(
      feed,
      ObservationTransition {
        revision: 2,
        previous: Some(1),
        current: 2,
      },
    )
    .expect("Crossing benchmark transition must be admitted");
    (feed, actor_id)
  }

  fn prepare_non_tail_crossing_batch<T: Config>(tail_members: u32) -> alloc::vec::Vec<ActorId> {
    let (feed, first_actor) = prepare_crossing_work::<T>(2);
    let total = T::CrossingPageSize::get().saturating_add(tail_members);
    let mut actors = alloc::vec![first_actor];
    for index in 0..total.saturating_sub(1) {
      let owner: T::AccountId = account("crossing-non-tail-unit", index, tail_members);
      actors.push(bench_create_system_crossing::<T>(owner, feed, 2));
    }
    CrossingRangeCursors::<T>::insert(
      feed,
      CrossingRangeCursor {
        revision: 2,
        traversal: CrossingTraversal::Upward,
        search_bound: 2,
        current_threshold: Some(2),
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    actors
  }

  fn bench_create_system_manual<T: Config>(seed: u32) -> ActorId {
    let owner: T::AccountId = account("wakeup_owner", seed, 0);
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 0,
    };
    let contract_steps = make_inert_contract_steps::<T>();
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      system_contract::<T>(schedule, contract_steps),
    )
    .expect("create_system_actor must succeed in wakeup benchmark setup");
    NextActorId::<T>::get().saturating_sub(1)
  }

  fn fund_reachable_update_assets<T: Config>(actor_id: ActorId, amount: T::Balance) {
    fund_reachable_assets_except::<T>(actor_id, amount, None);
  }

  fn fund_reachable_assets_except<T: Config>(
    actor_id: ActorId,
    amount: T::Balance,
    excluded: Option<T::AssetId>,
  ) {
    let identity = Pallet::<T>::actor_identity(actor_id).expect("update Actor identity exists");
    let funding = ActorFunding::<T>::get(actor_id).expect("update funding exists");
    T::BenchmarkHelper::enable_asset_ops_ingress();
    for asset in &funding.funding_tracked_assets {
      if excluded == Some(*asset) {
        continue;
      }
      T::AssetOps::mint(&identity.owner, *asset, amount.saturating_mul(2u32.into()))
        .expect("authorized funding source has transferable custody");
      T::BenchmarkHelper::transfer_signed(
        &identity.owner,
        &identity.sovereign_account,
        *asset,
        amount,
      )
      .expect("real owner ingress accumulates tracked funding");
    }
    let accumulated = ActorFunding::<T>::get(actor_id).expect("funded Actor exists");
    assert_eq!(
      accumulated.funding_accumulated.len(),
      funding
        .funding_tracked_assets
        .iter()
        .filter(|asset| excluded != Some(**asset))
        .count()
    );
    assert!(
      funding
        .funding_tracked_assets
        .iter()
        .all(|asset| if excluded == Some(*asset) {
          !accumulated.funding_accumulated.contains_key(asset)
        } else {
          accumulated.funding_accumulated.get(asset) == Some(&amount)
        })
    );
  }

  fn prepare_reachable_update<T: Config>(
    opening_legs: u32,
    family: TriggerFamily,
  ) -> Result<
    (
      T::AccountId,
      ActorId,
      ActorContractOf<T>,
      T::ObservationFeedId,
    ),
    polkadot_sdk::frame_benchmarking::BenchmarkError,
  > {
    let caller: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&caller);
    let (steps, liquidity_funding) = reachable_retry_contract_allocation::<T>(opening_legs, 0)?;
    let feeds = observation_feed_pool::<T>(if family == TriggerFamily::ObservationChange {
      5
    } else {
      1
    });
    let (old_feed, trigger, replacement_trigger) = match family {
      TriggerFamily::ObservationCrossing => (
        feeds[0],
        Trigger::observation_crossing(feeds[0], CrossingDirection::Rising, 4, 1),
        Trigger::observation_crossing(feeds[0], CrossingDirection::Rising, u128::MAX, 0),
      ),
      TriggerFamily::ObservationChange => {
        seed_recycled_observation_slot::<T>(feeds[0]);
        install_observation_guard::<T>(feeds[1], 0);
        install_observation_guard::<T>(feeds[4], 1);
        (
          feeds[2],
          observation_trigger::<T>(feeds[2]),
          observation_trigger::<T>(feeds[3]),
        )
      }
      _ => panic!("update diagnostic admits only its two measured Trigger families"),
    };
    prefund_active_user_creation::<T>(&caller, &steps);
    Pallet::<T>::create_user_actor(
      RawOrigin::Signed(caller.clone()).into(),
      Mutability::Mutable,
      user_contract::<T>(
        Schedule {
          trigger,
          cooldown_blocks: 100,
        },
        steps,
      ),
    )
    .expect("reachable update Contract is admitted");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let funding_amount = liquidity_funding.2.min(liquidity_funding.3);
    assert!(!funding_amount.is_zero());
    fund_reachable_update_assets::<T>(actor_id, funding_amount);
    open_reachable_retry::<T>(actor_id, liquidity_funding);
    let state = Pallet::<T>::active_actor_state(actor_id).expect("update Run is live");
    let run = state.run_state.as_ref().expect("real update Run exists");
    assert_eq!(
      run.opening_snapshot.len(),
      Pallet::<T>::opening_surfaces(&state.contract.steps, 0).len()
    );
    assert_eq!(
      run.funding_snapshot.len(),
      state.funding.funding_tracked_assets.len()
    );
    assert!(
      run
        .funding_snapshot
        .values()
        .all(|amount| *amount == funding_amount)
    );
    assert!(state.funding.funding_accumulated.is_empty());
    let retained_run = run.encode();
    // A second paid family occurrence establishes the real deferred latch of the active Run.
    match family {
      TriggerFamily::ObservationCrossing => {
        for rearm in [true, false] {
          frame_system::Pallet::<T>::set_block_number(
            frame_system::Pallet::<T>::block_number().saturating_add(One::one()),
          );
          if rearm {
            publish_observation_at_most::<T>(old_feed, 1);
          } else {
            assert!(publish_zero_step_observation::<T>(old_feed, 1_000) >= 4);
          }
          while CrossingPendingFeedListState::<T>::get().count > 0 {
            Pallet::<T>::crossing_work_unit()
              .expect("real deferred Crossing occurrence materializes");
          }
        }
      }
      TriggerFamily::ObservationChange => {
        Pallet::<T>::note_observation_changed(old_feed, 3)
          .expect("deferred observation occurrence is admitted");
        while DirtyObservationListState::<T>::get().count > 0 {
          Pallet::<T>::do_fanout_dirty_observation_page()
            .expect("deferred observation occurrence materializes");
        }
      }
      _ => unreachable!(),
    }
    assert!(
      benchmark_fixture_hot::<T>(actor_id)
        .expect("deferred Actor exists")
        .pending_signal
    );
    assert_eq!(
      ActorRunStateStore::<T>::get(actor_id)
        .expect("deferred Run remains live")
        .encode(),
      retained_run
    );
    fund_reachable_update_assets::<T>(actor_id, funding_amount);
    if family == TriggerFamily::ObservationChange {
      for feed in [feeds[1], old_feed, feeds[4]] {
        Pallet::<T>::note_observation_changed(feed, 4)
          .expect("dirty middle-node topology is admitted");
      }
      assert_eq!(DirtyObservationListState::<T>::get().count, 3);
    }
    let mut allowed: BoundedBTreeSet<T::AccountId, T::MaxWhitelistSize> =
      BoundedBTreeSet::default();
    for index in 0..T::MaxWhitelistSize::get() {
      allowed
        .try_insert(account("update-funding-source", index, 0))
        .expect("full allowlist fits");
    }
    let replacement = ActorContract {
      trigger: replacement_trigger,
      cooldown_blocks: 20,
      window: None,
      steps: make_max_contract_steps::<T>(account("update-recipient", 0, 0)),
      funding: FundingSourcePolicy::SignedAllowlist(allowed),
      completion: CompletionPolicy::Persistent,
      auto_close_at_cycle_nonce: None,
    };
    assert_max_contract_geometry::<T>(actor_id);
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("reachable update premeasurement state passes full audit");
    Ok((caller, actor_id, replacement, old_feed))
  }

  fn execute_reachable_update<T: Config>(
    owner: T::AccountId,
    actor_id: ActorId,
    replacement: ActorContractOf<T>,
  ) {
    Pallet::<T>::update_contract(RawOrigin::Signed(owner).into(), actor_id, replacement)
      .expect("real Contract replacement succeeds");
  }

  fn assert_reachable_update<T: Config>(
    actor_id: ActorId,
    expected: &ActorContractOf<T>,
    old_feed: T::ObservationFeedId,
  ) {
    assert_eq!(
      Pallet::<T>::load_actor_contract(actor_id)
        .expect("replaced Contract exists")
        .encode(),
      expected.encode()
    );
    assert!(!ActorRunStateStore::<T>::contains_key(actor_id));
    assert!(
      benchmark_fixture_hot::<T>(actor_id)
        .expect("updated Actor is active")
        .pending_signal
    );
    let funding = ActorFunding::<T>::get(actor_id).expect("updated funding exists");
    assert!(funding.funding_tracked_assets.is_empty());
    assert!(funding.funding_accumulated.is_empty());
    assert_max_contract_geometry::<T>(actor_id);
    match expected.trigger {
      Trigger::ObservationCrossing { threshold, .. } => {
        assert_eq!(
          CrossingMemberships::<T>::get(actor_id)
            .expect("replacement Crossing membership exists")
            .key
            .threshold,
          threshold
        );
        assert!(ActorObservationFeeds::<T>::get(actor_id).is_none());
      }
      Trigger::ObservationChange { feed } => {
        assert_eq!(
          ActorObservationFeeds::<T>::get(actor_id).map(|feeds| feeds.to_vec()),
          Some(vec![feed])
        );
        assert!(!DirtyObservationFeeds::<T>::contains_key(old_feed));
        assert_eq!(DirtyObservationListState::<T>::get().count, 2);
      }
      _ => unreachable!(),
    }
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("Contract replacement preserves full state invariants");
  }

  fn reachable_retry_contract_allocation<T: Config>(
    opening_legs: u32,
    opening_start: u32,
  ) -> Result<
    (
      ContractSteps<T>,
      (T::AssetId, T::AssetId, T::Balance, T::Balance),
    ),
    polkadot_sdk::frame_benchmarking::BenchmarkError,
  > {
    let (mut steps, liquidity_funding) = retry_contract_with_opening_partition::<T>(
      T::MaxContractSteps::get() * 2,
      T::MaxContractSteps::get() * benchmark_predicate_capacity::<T>(),
    )?;
    let leg_count = T::MaxContractSteps::get()
      .checked_mul(2)
      .expect("host amount surface bound fits");
    assert!(opening_legs <= leg_count);
    assert!(opening_start < leg_count);
    for (index, step) in steps.iter_mut().enumerate() {
      let ActorTask::AddLiquidity {
        amount_a, amount_b, ..
      } = &mut step.task
      else {
        panic!("reachable retry allocation requires the admitted two-leg Contract");
      };
      for (offset, amount) in [amount_a, amount_b].into_iter().enumerate() {
        let position = ((index * 2 + offset) as u32 + leg_count - opening_start) % leg_count;
        if position >= opening_legs {
          *amount = AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50));
        }
      }
    }
    retain_admitted_contract_geometry::<T>(ActorType::System, &mut steps, &[0])?;
    Ok((steps, liquidity_funding))
  }

  fn create_reachable_manual_retry<T: Config>(
    opening_legs: u32,
    opening_start: u32,
    cooldown_blocks: u32,
  ) -> Result<
    (ActorId, (T::AssetId, T::AssetId, T::Balance, T::Balance)),
    polkadot_sdk::frame_benchmarking::BenchmarkError,
  > {
    let owner: T::AccountId = account("manual-retry-owner", 0, 0);
    ensure_creation_balance::<T>(&owner);
    let (steps, funding) = reachable_retry_contract_allocation::<T>(opening_legs, opening_start)?;
    let mut contract = system_contract::<T>(
      Schedule {
        trigger: Trigger::Manual,
        cooldown_blocks,
      },
      steps,
    )
    .expect("active Manual Contract exists");
    contract.funding = FundingSourcePolicy::OwnerOnly;
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      Some(contract),
    )
    .expect("real Manual retry Contract is admitted");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    fund_reachable_update_assets::<T>(actor_id, funding.2.min(funding.3));
    Ok((actor_id, funding))
  }

  fn prepare_reachable_suspended_head<T: Config>(
    opening_legs: u32,
    opening_start: u32,
  ) -> Result<(ActorId, ActorStepTicketOf<T>), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, funding) = create_reachable_manual_retry::<T>(opening_legs, opening_start, 100)?;
    open_reachable_retry::<T>(actor_id, funding);
    let run = ActorRunStateStore::<T>::get(actor_id).expect("real retry Run exists");
    let retained_run = run.encode();
    frame_system::Pallet::<T>::set_block_number(run.eligible_at);
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    let stats = Pallet::<T>::drain_overdue_wakeups_cursor(run.eligible_at, &mut meter);
    assert!(stats.ready_entries >= 1);
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Ready { .. })
    ));
    assert_eq!(
      ActorRunStateStore::<T>::get(actor_id)
        .expect("due Run remains")
        .encode(),
      retained_run
    );
    let state =
      Pallet::<T>::active_actor_state(actor_id).expect("due Actor has canonical authority");
    let Some(ActorControlLocation::Ready { ticket }) = ActorControlLocators::<T>::get(actor_id)
    else {
      panic!("actual due wakeup must publish a Ready ticket");
    };
    let (_, cell) = Pallet::<T>::actor_control_cell(actor_id).expect("Ready control cell exists");
    let ticket = Pallet::<T>::build_actor_step_ticket(
      actor_id,
      ticket,
      run.eligible_at,
      &state.identity,
      &state.hot,
      state.run_state.as_ref(),
      &cell.admission,
    )
    .expect("canonical due authority produces the measured Step ticket");
    assert_eq!(state.hot.cycle_state, CycleState::Suspended);
    assert_eq!(
      run.opening_snapshot.len(),
      Pallet::<T>::opening_surfaces(&state.contract.steps, 0).len()
    );
    assert_eq!(
      run.funding_snapshot.len(),
      state.funding.funding_tracked_assets.len()
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real due Ready fixture passes full state audit");
    Ok((actor_id, ticket))
  }

  fn create_reachable_waiting_guard<T: Config>(seed: u32, wakeup_at: BlockNumberFor<T>) -> ActorId {
    let owner: T::AccountId = account("cancel-waiting-guard", seed, 0);
    let mut contract = system_contract::<T>(
      Schedule {
        trigger: Trigger::Manual,
        cooldown_blocks: 0,
      },
      make_inert_contract_steps::<T>(),
    )
    .expect("active guard Contract exists");
    contract.window = Some(ScheduleWindow {
      start: wakeup_at,
      end: wakeup_at.saturating_add(T::MinWindowLength::get()),
    });
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner.clone(),
      Mutability::Mutable,
      Some(contract),
    )
    .expect("future-window guard is admitted");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    Pallet::<T>::manual_trigger(RawOrigin::Signed(owner).into(), actor_id)
      .expect("guard occurrence waits for its authored window");
    assert!(
      matches!(ActorControlLocators::<T>::get(actor_id), Some(ActorControlLocation::Waiting { key: WakeupKey::Block(at), .. }) if at == wakeup_at)
    );
    actor_id
  }

  fn prepare_reachable_waiting_opening<T: Config>(
    destination_entries: u32,
  ) -> Result<(ActorId, u32, BlockNumberFor<T>), polkadot_sdk::frame_benchmarking::BenchmarkError>
  {
    let (actor_id, count, keys, due) = prepare_reachable_waiting_publication::<T>(
      destination_entries,
      0,
      ReachableOpeningProfile::RetryMin,
    )?;
    assert_eq!(count, 1);
    Ok((actor_id, keys, due))
  }

  fn prepare_reachable_waiting_publication<T: Config>(
    destination_entries: u32,
    tail_chunks: u32,
    profile: ReachableOpeningProfile,
  ) -> Result<
    (ActorId, u32, u32, BlockNumberFor<T>),
    polkadot_sdk::frame_benchmarking::BenchmarkError,
  > {
    let (actor_id, count) = prepare_reachable_opening::<T>(tail_chunks, profile)?;
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Block), 0);
    let due = frame_system::Pallet::<T>::block_number().saturating_add(2u32.into());
    let active_room = Pallet::<T>::effective_active_actor_limit()
      .checked_sub(ActiveActorCount::<T>::get())
      .expect("admitted Actors fit host capacity");
    let identity_room = T::MaxActorIdentities::get()
      .checked_sub(ActorIdentityCount::<T>::get())
      .expect("admitted identities fit host capacity");
    let sovereign_room = T::MaxSystemSovereigns::get()
      .checked_sub(SystemSovereignCount::<T>::get())
      .expect("admitted sovereigns fit host capacity");
    let guards = active_room.min(identity_room).min(sovereign_room);
    assert!(guards > 0 && destination_entries <= guards && due < 1_000u32.into());
    frame::log::info!(
      "Waiting population: active_room={}, identity_room={}, sovereign_room={}, guards={}",
      active_room,
      identity_room,
      sovereign_room,
      guards,
    );
    for index in 0..guards {
      let deadline = if index < destination_entries {
        due
      } else {
        1_000u32.saturating_add(index).into()
      };
      create_reachable_waiting_guard::<T>(index, deadline);
    }
    let keys = guards - destination_entries.saturating_sub(1);
    assert_waiting_population_capacity::<T>();
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Block), keys);
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("complete populated pre-retry ownership");
    Ok((actor_id, count, keys, due))
  }

  fn assert_reachable_waiting_reference_publication<T: Config>(
    actor_id: ActorId,
    keys: u32,
    due: BlockNumberFor<T>,
  ) {
    let key = WakeupKey::Block(due);
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Block), keys + 1);
    assert_eq!(ActorWaitingOccupancies::<T>::get(key), 1);
    let (location, cell) =
      Pallet::<T>::actor_control_cell(actor_id).expect("retained Ready primary exists");
    assert!(matches!(location, ActorControlLocation::Ready { .. }));
    let pointer = cell
      .hot
      .wakeup_pointer
      .expect("Block reference pointer exists");
    assert_eq!((pointer.block, pointer.page_id, pointer.slot), (key, 0, 0));
    assert!(matches!(
      &ActorWaitingFrameChunks::<T>::get((key, 0))
        .expect("new reference page exists")
        .entries[0],
      Some(ActorWaitingEntry::Reference(reference))
        if reference.actor_id == actor_id
          && reference.admission_identity == cell.admission.admission_identity
    ));
    assert_waiting_population_capacity::<T>();
    assert_wakeup_cursor_page_indices::<T>();
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("complete populated Reference publication is legal");
  }

  fn assert_reachable_waiting_publication<T: Config>(
    actor_id: ActorId,
    count: u32,
    keys: u32,
    due: BlockNumberFor<T>,
    destination_entries: u32,
  ) {
    let expected_keys = keys + u32::from(destination_entries == 0);
    let key = WakeupKey::Block(due);
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Block), expected_keys);
    assert_eq!(
      ActorWaitingOccupancies::<T>::get(key),
      destination_entries + 1
    );
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Waiting { key: stored, page, slot })
        if stored == key && page == u64::from(destination_entries / 32)
          && u32::from(slot) == destination_entries % 32
    ));
    let state = Pallet::<T>::active_actor_state(actor_id)
      .expect("consumed service authority is republished in Waiting");
    assert_eq!(state.contract.steps.len() as u32, count);
    assert_eq!(state.hot.cycle_state, CycleState::Idle);
    assert!(state.hot.pending_signal && state.run_state.is_none());
    assert_waiting_population_capacity::<T>();
    assert_wakeup_cursor_page_indices::<T>();
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("complete populated Waiting publication is legal");
  }

  fn prepare_reachable_cadenced_cross_clock_publication<T: Config>() -> Result<
    (
      ActorId,
      ActorControlLocation<BlockNumberFor<T>>,
      SchedulerTick,
      u32,
    ),
    polkadot_sdk::frame_benchmarking::BenchmarkError,
  > {
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    T::BenchmarkHelper::advance_to_scheduler_tick(2)?;
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      account("cadenced-cross-clock-target", 0, 0),
      Mutability::Mutable,
      system_contract::<T>(
        Schedule {
          trigger: Trigger::cadenced(100),
          cooldown_blocks: 0,
        },
        make_inert_contract_steps::<T>(),
      ),
    )
    .expect("ordinary Cadenced target is admitted");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let first = Pallet::<T>::actor_hot(actor_id)
      .and_then(|hot| hot.trigger_wakeup_pointer)
      .expect("Cadenced target owns its initial Tick authority")
      .tick;
    T::BenchmarkHelper::finalize_scheduler_clock()?;
    T::BenchmarkHelper::advance_to_scheduler_tick(first)?;
    let now = frame_system::Pallet::<T>::block_number();
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    Pallet::<T>::drain_overdue_wakeups_cursor(now, &mut meter);
    let (location, cell) =
      Pallet::<T>::actor_control_cell(actor_id).expect("due Cadenced target is Ready");
    assert!(matches!(location, ActorControlLocation::Ready { .. }));
    assert!(cell.hot.pending_signal && cell.hot.trigger_wakeup_pointer.is_none());
    let service_due = now.saturating_add(2u32.into());
    benchmark_fixture_prepare_consumed_service_waiting::<T>(actor_id, service_due)();
    let block_location = ActorControlLocators::<T>::get(actor_id)
      .expect("consumed Cadenced source republishes one Block primary");
    assert!(matches!(
      block_location,
      ActorControlLocation::Waiting {
        key: WakeupKey::Block(at),
        ..
      } if at == service_due
    ));

    let active_room = Pallet::<T>::effective_active_actor_limit()
      .checked_sub(ActiveActorCount::<T>::get())
      .expect("Cadenced guards fit active capacity");
    let identity_room = T::MaxActorIdentities::get()
      .checked_sub(ActorIdentityCount::<T>::get())
      .expect("Cadenced guards fit identity capacity");
    let sovereign_room = T::MaxSystemSovereigns::get()
      .checked_sub(SystemSovereignCount::<T>::get())
      .expect("Cadenced guards fit sovereign capacity");
    let guards = active_room.min(identity_room).min(sovereign_room);
    for index in 0..guards {
      Pallet::<T>::create_system_actor(
        RawOrigin::Root.into(),
        account("cadenced-cross-clock-guard", index, 0),
        Mutability::Mutable,
        system_contract::<T>(
          Schedule {
            trigger: Trigger::cadenced(u64::from(1_000 + index)),
            cooldown_blocks: 0,
          },
          make_inert_contract_steps::<T>(),
        ),
      )
      .expect("ordinary Cadenced guard is admitted");
    }
    assert_waiting_population_capacity::<T>();
    let next = first.checked_add(100).expect("next cadence fits");
    assert!(
      Pallet::<T>::wakeup_cursor_peek_key(WakeupClock::Tick)
        .is_none_or(|minimum| minimum > WakeupKey::Tick(next))
    );
    let keys = WakeupCursorLen::<T>::get(WakeupClock::Tick);
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("complete cross-clock source population is legal");
    Ok((actor_id, block_location, next, keys))
  }

  fn prepare_reachable_max_cell_waiting_publication<T: Config>() -> Result<
    (ActorId, usize, u32, BlockNumberFor<T>),
    polkadot_sdk::frame_benchmarking::BenchmarkError,
  > {
    let actor_id = prepare_temporal_control_cell::<T>(false);
    Pallet::<T>::execute_cycle(Weight::MAX);
    let first = ActorRunStateStore::<T>::get(actor_id).expect("first Cadenced retry exists");
    frame_system::Pallet::<T>::set_block_number(first.eligible_at);
    Pallet::<T>::execute_cycle(Weight::MAX);
    let second = ActorRunStateStore::<T>::get(actor_id).expect("second Cadenced retry exists");
    let (_, maximum) =
      Pallet::<T>::actor_control_cell(actor_id).expect("maximal suspended control cell exists");
    assert_eq!(maximum.hot.cycle_state, CycleState::Suspended);
    assert!(maximum.hot.wakeup_pointer.is_some());
    assert!(maximum.hot.trigger_wakeup_pointer.is_some());
    assert!(maximum.hot.last_cycle_block.is_some() && maximum.eligible_at.is_some());
    let maximum_size = maximum.encoded_size();

    frame_system::Pallet::<T>::set_block_number(second.eligible_at);
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    let stats = Pallet::<T>::drain_overdue_wakeups_cursor(second.eligible_at, &mut meter);
    assert_eq!(stats.ready_entries, 1);
    let (location, ready) =
      Pallet::<T>::actor_control_cell(actor_id).expect("due maximal-cell target is Ready");
    assert!(matches!(location, ActorControlLocation::Ready { .. }));
    assert!(ready.hot.wakeup_pointer.is_none() && ready.hot.trigger_wakeup_pointer.is_some());

    let due = second.eligible_at.saturating_add(2u32.into());
    let active_room = Pallet::<T>::effective_active_actor_limit()
      .checked_sub(ActiveActorCount::<T>::get())
      .expect("maximal-cell population fits active capacity");
    let identity_room = T::MaxActorIdentities::get()
      .checked_sub(ActorIdentityCount::<T>::get())
      .expect("maximal-cell population fits identity capacity");
    let sovereign_room = T::MaxSystemSovereigns::get()
      .checked_sub(SystemSovereignCount::<T>::get())
      .expect("maximal-cell population fits sovereign capacity");
    let guards = active_room.min(identity_room).min(sovereign_room);
    assert!(guards > 0 && due < 1_000u32.into());
    for index in 0..guards {
      create_reachable_waiting_guard::<T>(index, 1_000u32.saturating_add(index).into());
    }
    assert_waiting_population_capacity::<T>();
    let keys = WakeupCursorLen::<T>::get(WakeupClock::Block);
    assert_eq!(keys, guards);
    assert!(Pallet::<T>::wakeup_cursor_peek().is_none_or(|minimum| minimum > due));
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("maximal-cell publication source population is legal");
    Ok((actor_id, maximum_size, keys, due))
  }

  fn assert_reachable_waiting_opening<T: Config>(
    actor_id: ActorId,
    keys: u32,
    due: BlockNumberFor<T>,
  ) {
    assert_reachable_opening::<T>(actor_id, 1, ReachableOpeningProfile::RetryMin);
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Block), keys);
    assert_eq!(Pallet::<T>::wakeup_cursor_peek(), Some(due));
    assert_waiting_population_capacity::<T>();
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Waiting { key: WakeupKey::Block(at), .. }) if at == due
    ));
    assert_wakeup_cursor_page_indices::<T>();
  }

  fn assert_waiting_population_capacity<T: Config>() {
    assert!(
      ActiveActorCount::<T>::get() == Pallet::<T>::effective_active_actor_limit()
        || ActorIdentityCount::<T>::get() == T::MaxActorIdentities::get()
        || SystemSovereignCount::<T>::get() == T::MaxSystemSovereigns::get(),
      "complete System population exhausts an explicit host capacity"
    );
  }

  struct WaitingHeapClose<T: Config> {
    actor_id: ActorId,
    removed_key: WakeupKey<BlockNumberFor<T>>,
    expected_heap: Vec<WakeupKey<BlockNumberFor<T>>>,
    active_count: u32,
  }

  fn prepare_full_waiting_heap_close<T: Config>(upward: bool) -> WaitingHeapClose<T> {
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Block), 0);
    let guards = (Pallet::<T>::effective_active_actor_limit() - ActiveActorCount::<T>::get())
      .min(T::MaxActorIdentities::get() - ActorIdentityCount::<T>::get())
      .min(T::MaxSystemSovereigns::get() - SystemSovereignCount::<T>::get());
    let size = T::WakeupPageSize::get();
    let shared = (guards - 1) % size;
    let keys = guards - shared;
    assert!(keys > size * 2);
    let horizon: BlockNumberFor<T> = keys
      .checked_mul(3)
      .and_then(|range| range.checked_add(10))
      .expect("bounded fixture deadline range")
      .into();
    assert!(horizon <= T::MaxExecutionDelayBlocks::get());
    let root_branch = |mut index: u32| {
      while index > 2 {
        index = (index - 1) / 2;
      }
      index
    };
    let low_branch = root_branch(keys - 1);
    let removed_index = if upward {
      (3..keys - 1)
        .rev()
        .find(|index| root_branch(*index) != low_branch)
        .expect("opposite subtree contains a deep removal target")
    } else {
      keys - 1
    };
    let mut actor_id = None;
    let mut heap = Vec::new();
    for index in 0..keys {
      let deadline = if index == 0 {
        10
      } else if root_branch(index) == low_branch {
        keys + 10 + index
      } else {
        2 * keys + 10 + index
      };
      let actor = create_reachable_waiting_guard::<T>(index, deadline.into());
      if index == removed_index {
        actor_id = Some(actor);
      }
      let key = WakeupKey::Block(deadline.into());
      assert_eq!(ActorWaitingCursorIndices::<T>::get(key), Some(index));
      heap.push(key);
    }
    for index in 0..shared {
      create_reachable_waiting_guard::<T>(keys + index, 10u32.into());
    }
    assert_waiting_population_capacity::<T>();
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Block), keys);
    assert_eq!(keys % size, 1);
    assert_wakeup_cursor_page_indices::<T>();
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("complete public-created Waiting heap is legal");
    let removed_key = heap.swap_remove(removed_index as usize);
    if upward {
      // The tail is in the low subtree; every ancestor below the opposite root is high.
      // It replaces the removed key and moves to that root, shifting only this path down.
      let mut index = removed_index;
      assert!(heap[index as usize] < heap[((index - 1) / 2) as usize]);
      while index > 2 {
        let parent = (index - 1) / 2;
        heap.swap(index as usize, parent as usize);
        index = parent;
      }
    }
    frame::log::info!(
      "Waiting removal population: guards={}, keys={}, removed_index={}, upward={}",
      guards,
      keys,
      removed_index,
      upward,
    );
    WaitingHeapClose {
      actor_id: actor_id.unwrap(),
      removed_key,
      expected_heap: heap,
      active_count: ActiveActorCount::<T>::get(),
    }
  }

  fn assert_full_waiting_heap_close<T: Config>(fixture: &WaitingHeapClose<T>) {
    assert!(Pallet::<T>::actor_identity(fixture.actor_id).is_none());
    assert!(!ActorControlLocators::<T>::contains_key(fixture.actor_id));
    assert_eq!(ActiveActorCount::<T>::get(), fixture.active_count - 1);
    let key = fixture.removed_key;
    assert!(!ActorWaitingOccupancies::<T>::contains_key(key));
    assert!(!ActorWaitingHeads::<T>::contains_key(key));
    assert!(!ActorWaitingTails::<T>::contains_key(key));
    assert!(!ActorWaitingCursorIndices::<T>::contains_key(key));
    assert!(!ActorWaitingFrameChunks::<T>::contains_key((key, 0)));
    let size = T::WakeupPageSize::get();
    let len = fixture.expected_heap.len() as u32;
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Block), len);
    assert!(!WakeupCursorPages::<T>::contains_key((
      WakeupClock::Block,
      u64::from(len / size)
    )));
    for (index, expected) in fixture.expected_heap.chunks(size as usize).enumerate() {
      let page = WakeupCursorPages::<T>::get((WakeupClock::Block, index as u64)).unwrap();
      assert_eq!(page.as_slice(), expected);
    }
    assert_wakeup_cursor_page_indices::<T>();
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("exact close preserves the complete Waiting heap");
  }

  struct MixedWaitingClose<T: Config> {
    actor_id: ActorId,
    tick: SchedulerTick,
    sovereign: T::AccountId,
    custody: T::Balance,
    active_count: u32,
    head_entries: Vec<u8>,
    tail_entries: Vec<u8>,
    retired_sovereigns: Vec<SystemSovereignId>,
  }

  fn prepare_mixed_waiting_close<T: Config>(
    unlink: bool,
  ) -> Result<MixedWaitingClose<T>, polkadot_sdk::frame_benchmarking::BenchmarkError> {
    if T::MaxContractSteps::get() < 2 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "mixed Waiting reference requires a two-Step Contract",
      ));
    }
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    T::BenchmarkHelper::advance_to_scheduler_tick(2)?;
    let create = |seed: u32, every_ticks: u64, steps: ContractSteps<T>| {
      Pallet::<T>::create_system_actor(
        RawOrigin::Root.into(),
        account("mixed-waiting-owner", seed, 0),
        Mutability::Mutable,
        system_contract::<T>(
          Schedule {
            trigger: Trigger::cadenced(every_ticks),
            cooldown_blocks: 0,
          },
          steps,
        ),
      )
      .expect("ordinary Cadenced Actor is admitted");
      NextActorId::<T>::get().saturating_sub(1)
    };
    let mut tick = 0;
    for seed in 0..32 {
      let actor = create(seed, 200, make_inert_contract_steps::<T>());
      let pointer = Pallet::<T>::actor_hot(actor)
        .and_then(|hot| hot.trigger_wakeup_pointer)
        .expect("Cadenced guard owns a temporal primary");
      if seed == 0 {
        tick = pointer.tick;
      }
      assert_eq!(pointer.tick, tick);
    }
    let mut steps = inert_contract_steps_of_len::<T>(2);
    steps[0].precondition = Some(packed_predicate_clauses::<T>(
      vec![TimedPredicate {
        timing: ObservationTiming::Current,
        predicate: Predicate::BlockNumberBelow { threshold: 0 },
      }],
      1,
    ));
    let actor_id = create(32, 100, steps);
    let first = Pallet::<T>::actor_hot(actor_id)
      .and_then(|hot| hot.trigger_wakeup_pointer)
      .expect("target owns its first cadence")
      .tick;
    T::BenchmarkHelper::finalize_scheduler_clock()?;
    T::BenchmarkHelper::advance_to_scheduler_tick(first)?;
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    Pallet::<T>::drain_overdue_wakeups_cursor(
      frame_system::Pallet::<T>::block_number(),
      &mut meter,
    );
    let (location, cell) = Pallet::<T>::actor_control_cell(actor_id).expect("due target exists");
    assert!(matches!(location, ActorControlLocation::Ready { .. }));
    frame_system::Pallet::<T>::set_block_number(
      frame_system::Pallet::<T>::block_number().max(cell.eligible_at.unwrap()),
    );
    Pallet::<T>::execute_cycle(Weight::MAX);
    let run = Pallet::<T>::actor_run_state(actor_id).expect("real Opening retains its suffix");
    assert_eq!(run.cursor, 1);
    let retained_run = run.encode();
    let hot = Pallet::<T>::actor_hot(actor_id).expect("Running target exists");
    assert_eq!(hot.cycle_state, CycleState::Running);
    let pointer = hot.trigger_wakeup_pointer.expect("Opening rearms cadence");
    assert_eq!((pointer.tick, pointer.page_id, pointer.slot), (tick, 1, 0));
    let key = WakeupKey::Tick(tick);
    let period = tick
      .checked_sub(Pallet::<T>::current_scheduler_tick().unwrap())
      .expect("shared deadline remains in the future");
    let mut later = Vec::new();
    for seed in 33..96 {
      later.push(create(seed, period, make_inert_contract_steps::<T>()));
    }
    let middle = ActorWaitingFrameChunks::<T>::get((key, 1)).expect("mixed middle page exists");
    assert_eq!(middle.live_entries, 32);
    assert!(matches!(&middle.entries[0],
      Some(ActorWaitingEntry::Reference(reference)) if reference.actor_id == actor_id));
    for (entry, actor) in middle.entries.iter().skip(1).zip(&later[..31]) {
      assert!(matches!(entry,
        Some(ActorWaitingEntry::Primary(cell)) if cell.actor_id == *actor));
    }
    if unlink {
      for actor in &later[..31] {
        Pallet::<T>::close_actor(RawOrigin::Root.into(), *actor)
          .expect("ordinary close leaves the reference as the last middle entry");
      }
    }
    assert_eq!(
      ActorWaitingOccupancies::<T>::get(key),
      if unlink { 65 } else { 96 }
    );
    assert_eq!(
      Pallet::<T>::actor_run_state(actor_id).unwrap().encode(),
      retained_run
    );
    let head = ActorWaitingFrameChunks::<T>::get((key, 0)).unwrap();
    let tail = ActorWaitingFrameChunks::<T>::get((key, 2)).unwrap();
    assert_eq!((head.live_entries, tail.live_entries), (32, 32));
    let sovereign = Pallet::<T>::actor_identity(actor_id)
      .unwrap()
      .sovereign_account;
    let native = T::FeeNativeAssetId::get();
    let amount = T::AssetOps::minimum_balance(native)
      .max(One::one())
      .saturating_mul(10u32.into());
    T::AssetOps::mint(&sovereign, native, amount).expect("surviving custody is funded");
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real mixed Waiting and Running authority agree");
    Ok(MixedWaitingClose {
      actor_id,
      tick,
      custody: T::AssetOps::balance(&sovereign, native),
      sovereign,
      active_count: ActiveActorCount::<T>::get(),
      head_entries: head.entries.encode(),
      tail_entries: tail.entries.encode(),
      retired_sovereigns: if unlink {
        later[..31].to_vec()
      } else {
        Vec::new()
      },
    })
  }

  fn assert_mixed_waiting_close<T: Config>(fixture: &MixedWaitingClose<T>, unlink: bool) {
    let key = WakeupKey::Tick(fixture.tick);
    assert_mixed_waiting_target_closed::<T>(fixture);
    assert_eq!(
      ActorWaitingOccupancies::<T>::get(key),
      if unlink { 64 } else { 95 }
    );
    let head = ActorWaitingFrameChunks::<T>::get((key, 0)).unwrap();
    let tail = ActorWaitingFrameChunks::<T>::get((key, 2)).unwrap();
    assert_eq!(head.entries.encode(), fixture.head_entries);
    assert_eq!(tail.entries.encode(), fixture.tail_entries);
    assert_eq!(head.next_page, Some(if unlink { 2 } else { 1 }));
    assert_eq!(tail.previous_page, Some(if unlink { 0 } else { 1 }));
    if unlink {
      assert!(!ActorWaitingFrameChunks::<T>::contains_key((key, 1)));
    } else {
      let middle = ActorWaitingFrameChunks::<T>::get((key, 1)).unwrap();
      assert_eq!(middle.live_entries, 31);
      assert!(middle.entries[0].is_none());
    }
    assert!(ActorWaitingCursorIndices::<T>::contains_key(key));
    assert_wakeup_clock_page_indices::<T>(WakeupClock::Tick);
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("public close preserves mixed Waiting ownership");
  }

  fn assert_mixed_waiting_target_closed<T: Config>(fixture: &MixedWaitingClose<T>) {
    assert!(Pallet::<T>::actor_identity(fixture.actor_id).is_none());
    assert!(!ActorControlLocators::<T>::contains_key(fixture.actor_id));
    assert!(!ActorRunHeads::<T>::contains_key(fixture.actor_id));
    assert!(!ActorRunPayloads::<T>::contains_key(fixture.actor_id));
    assert!(!ActorContractHeads::<T>::contains_key(fixture.actor_id));
    assert!(!ActorFunding::<T>::contains_key(fixture.actor_id));
    assert!(!ActorStateHolds::<T>::contains_key(fixture.actor_id));
    assert!(
      ActorContractTailChunks::<T>::iter_prefix(fixture.actor_id)
        .next()
        .is_none()
    );
    assert_eq!(ActiveActorCount::<T>::get(), fixture.active_count - 1);
    assert_eq!(
      T::AssetOps::balance(&fixture.sovereign, T::FeeNativeAssetId::get()),
      fixture.custody
    );
  }

  fn prepare_mixed_waiting_deadline_close<T: Config>()
  -> Result<(MixedWaitingClose<T>, u32), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let mut fixture = prepare_mixed_waiting_close::<T>(true)?;
    let key = WakeupKey::Tick(fixture.tick);
    let run = Pallet::<T>::actor_run_state(fixture.actor_id)
      .unwrap()
      .encode();
    for page in [0, 2] {
      let chunk = ActorWaitingFrameChunks::<T>::get((key, page)).unwrap();
      for entry in chunk.entries.into_iter().flatten() {
        let ActorWaitingEntry::Primary(cell) = entry else {
          panic!("neighbor guards own primary cells");
        };
        Pallet::<T>::close_actor(RawOrigin::Root.into(), cell.actor_id).unwrap();
        fixture.retired_sovereigns.push(cell.actor_id);
      }
    }
    assert_eq!(fixture.retired_sovereigns.len(), 95);
    assert_eq!(ActorWaitingOccupancies::<T>::get(key), 1);
    let active_room = Pallet::<T>::effective_active_actor_limit() - ActiveActorCount::<T>::get();
    let identity_room = T::MaxActorIdentities::get() - ActorIdentityCount::<T>::get();
    let fresh_room = T::MaxSystemSovereigns::get() - SystemSovereignCount::<T>::get();
    let guards = active_room.min(identity_room).min(fresh_room + 95);
    let size = T::WakeupPageSize::get();
    let maximum_keys = WakeupCursorLen::<T>::get(WakeupClock::Tick) + guards;
    let shared = (maximum_keys - 1) % size;
    assert!(guards > 95 && guards > shared && guards > size);
    frame::log::info!(
      "Tick cleanup population: active_room={}, identity_room={}, fresh_room={}, reused=95, guards={}, keys={}",
      active_room,
      identity_room,
      fresh_room,
      guards,
      maximum_keys - shared,
    );
    for index in 0..guards {
      let period = 1_000 + if index <= shared { 0 } else { u64::from(index) };
      let contract = system_contract::<T>(
        Schedule {
          trigger: Trigger::cadenced(period),
          cooldown_blocks: 0,
        },
        make_inert_contract_steps::<T>(),
      );
      let owner = account("waiting-deadline-owner", index, 0);
      if let Some(sovereign_id) = fixture.retired_sovereigns.get(index as usize) {
        assert_eq!(
          SystemSovereigns::<T>::get(sovereign_id),
          Some(SystemSovereignState::Vacant)
        );
        Pallet::<T>::create_system_actor_at_sovereign_id(
          RawOrigin::Root.into(),
          *sovereign_id,
          owner,
          Mutability::Mutable,
          contract,
        )
        .expect("ordinary creation reuses a known retired guard sovereign");
        assert_eq!(
          SystemSovereigns::<T>::get(sovereign_id),
          Some(SystemSovereignState::Occupied(NextActorId::<T>::get() - 1))
        );
      } else {
        Pallet::<T>::create_system_actor(
          RawOrigin::Root.into(),
          owner,
          Mutability::Mutable,
          contract,
        )
        .expect("ordinary creation fills remaining host capacity");
      }
    }
    fixture.active_count = ActiveActorCount::<T>::get();
    assert_waiting_population_capacity::<T>();
    let keys = WakeupCursorLen::<T>::get(WakeupClock::Tick);
    assert_eq!(keys, maximum_keys - shared);
    assert_eq!(keys % size, 1);
    assert_eq!(ActorWaitingCursorIndices::<T>::get(key), Some(0));
    assert_eq!(
      WakeupCursorPages::<T>::get((WakeupClock::Tick, 0)).unwrap()[0],
      key
    );
    assert_eq!(
      Pallet::<T>::actor_run_state(fixture.actor_id)
        .unwrap()
        .encode(),
      run
    );
    assert_eq!(
      WakeupCursorPages::<T>::get((WakeupClock::Tick, u64::from(keys / size)))
        .unwrap()
        .len(),
      1
    );
    assert_wakeup_clock_page_indices::<T>(WakeupClock::Tick);
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("complete Tick heap follows ordinary lifecycle history");
    Ok((fixture, keys))
  }

  fn assert_mixed_waiting_deadline_close<T: Config>(fixture: &MixedWaitingClose<T>, keys: u32) {
    assert_mixed_waiting_target_closed::<T>(fixture);
    let key = WakeupKey::Tick(fixture.tick);
    assert!(!ActorWaitingOccupancies::<T>::contains_key(key));
    assert!(!ActorWaitingHeads::<T>::contains_key(key));
    assert!(!ActorWaitingTails::<T>::contains_key(key));
    assert!(!ActorWaitingCursorIndices::<T>::contains_key(key));
    assert!(!ActorWaitingFrameChunks::<T>::iter_keys().any(|(stored_key, _)| stored_key == key));
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Tick), keys - 1);
    assert!(!WakeupCursorPages::<T>::contains_key((
      WakeupClock::Tick,
      u64::from(keys / T::WakeupPageSize::get())
    )));
    assert_wakeup_clock_page_indices::<T>(WakeupClock::Tick);
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("last-reference close repairs the complete Tick heap");
  }

  fn open_reachable_retry<T: Config>(
    actor_id: ActorId,
    funding: (T::AssetId, T::AssetId, T::Balance, T::Balance),
  ) {
    let (asset_a, asset_b, amount_a, amount_b) = funding;
    let identity = Pallet::<T>::actor_identity(actor_id).expect("retry Actor identity exists");
    T::AssetOps::mint(&identity.sovereign_account, asset_a, amount_a)
      .expect("retry first liquidity leg is funded");
    T::AssetOps::mint(&identity.sovereign_account, asset_b, amount_b)
      .expect("retry second liquidity leg is funded");
    let contract = Pallet::<T>::load_actor_contract(actor_id).expect("retry Contract exists");
    let (trigger_family, trigger_weight) = match contract.trigger {
      Trigger::Manual => (TriggerFamily::Manual, T::WeightInfo::manual_trigger()),
      Trigger::AddressEvent { .. } => (
        TriggerFamily::AddressEvent,
        T::WeightInfo::address_event_trigger_occurrence(),
      ),
      Trigger::ObservationCrossing { .. } => (
        TriggerFamily::ObservationCrossing,
        T::WeightInfo::observation_crossing_trigger_occurrence(),
      ),
      Trigger::ObservationChange { .. } => (
        TriggerFamily::ObservationChange,
        T::WeightInfo::observation_change_trigger_occurrence(),
      ),
      _ => panic!("reachable retry fixture requires a supported Trigger"),
    };
    let fee_reserve = full_attempt_fee::<T>(&contract.steps)
      .checked_add(
        &Pallet::<T>::trigger_fee_for_weight(ActorType::User, trigger_family, trigger_weight)
          .trigger_fee,
      )
      .expect("host fee reserve is representable");
    T::AssetOps::mint(
      &identity.sovereign_account,
      T::FeeNativeAssetId::get(),
      fee_reserve,
    )
    .expect("real Trigger and current Action fees have independent funding");
    let now = frame_system::Pallet::<T>::block_number().saturating_add(One::one());
    frame_system::Pallet::<T>::set_block_number(now);
    GlobalCircuitBreaker::<T>::put(false);
    let occurrence_cohort = match contract.trigger {
      Trigger::Manual => {
        Pallet::<T>::manual_trigger(RawOrigin::Signed(identity.owner.clone()).into(), actor_id)
          .expect("real Manual occurrence is admitted");
        vec![actor_id]
      }
      Trigger::AddressEvent { .. } => {
        T::BenchmarkHelper::setup_address_event_ingress(
          &identity.sovereign_account,
          &identity.owner,
          One::one(),
        )
        .expect("host prepares the initial AddressEvent");
        assert!(T::BenchmarkHelper::run_address_event_ingress(
          &identity.sovereign_account,
          &identity.owner,
          One::one(),
        ));
        vec![actor_id]
      }
      Trigger::ObservationCrossing {
        feed, threshold, ..
      } => {
        assert!(threshold > 0, "retry Crossing threshold has a rearm side");
        publish_observation_at_most::<T>(feed, threshold - 1);
        while CrossingPendingFeedListState::<T>::get().count > 0 {
          Pallet::<T>::crossing_work_unit().expect("Crossing rearm materializes");
        }
        frame_system::Pallet::<T>::set_block_number(
          frame_system::Pallet::<T>::block_number().saturating_add(One::one()),
        );
        let CanonicalObservationState::Available { revision, .. } =
          T::ObservationProvider::current(&feed)
        else {
          panic!("retry Crossing feed remains available")
        };
        Pallet::<T>::note_observation_transition(
          feed,
          ObservationTransition {
            revision: revision.saturating_add(1),
            previous: Some(threshold - 1),
            current: threshold,
          },
        )
        .expect("real Crossing fire transition is admitted");
        while CrossingPendingFeedListState::<T>::get().count > 0 {
          Pallet::<T>::crossing_work_unit().expect("Crossing cohort materializes");
        }
        CrossingMemberships::<T>::iter()
          .filter_map(|(id, locator)| (locator.key.feed == feed).then_some(id))
          .collect::<Vec<_>>()
      }
      Trigger::ObservationChange { feed } => {
        Pallet::<T>::note_observation_changed(feed, 2)
          .expect("real observation occurrence is admitted");
        while DirtyObservationListState::<T>::get().count > 0 {
          Pallet::<T>::do_fanout_dirty_observation_page()
            .expect("observation occurrence materializes");
        }
        vec![actor_id]
      }
      _ => panic!("reachable retry fixture requires a supported Trigger"),
    };
    assert!(
      benchmark_fixture_hot::<T>(actor_id)
        .expect("latched Actor exists")
        .pending_signal
    );
    if benchmark_fixture_hot::<T>(actor_id)
      .expect("latched Actor exists")
      .lifecycle
      .is_paused()
    {
      // A paused target retains its paid latch while the existing guard cohort finishes Opening.
      for _ in 0..T::MaxQueueLength::get() {
        if occurrence_cohort
          .iter()
          .filter(|id| **id != actor_id)
          .all(|id| benchmark_fixture_hot::<T>(*id).is_some_and(|hot| !hot.pending_signal))
        {
          break;
        }
        Pallet::<T>::execute_cycle(Weight::MAX);
      }
      assert!(
        occurrence_cohort
          .iter()
          .filter(|id| **id != actor_id)
          .all(|id| benchmark_fixture_hot::<T>(*id).is_some_and(|hot| !hot.pending_signal)),
        "guard cohort finishes before the target resumes"
      );
      Pallet::<T>::resume_actor(RawOrigin::Signed(identity.owner.clone()).into(), actor_id)
        .expect("target resumes its retained occurrence through the public lifecycle");
    }
    let (_, ready) = Pallet::<T>::actor_control_cell(actor_id)
      .expect("real occurrence retains canonical control authority");
    let eligible_at = ready
      .eligible_at
      .expect("latched service has an eligibility boundary");
    frame_system::Pallet::<T>::set_block_number(now.max(eligible_at));
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real close occurrence preserves canonical state");
    for _ in 0..T::MaxQueueLength::get() {
      Pallet::<T>::execute_cycle(Weight::MAX);
      if !Pallet::<T>::active_actor_exists(actor_id) {
        break;
      }
      if benchmark_fixture_hot::<T>(actor_id)
        .is_some_and(|hot| hot.cycle_state == CycleState::Suspended)
        && matches!(
          ActorControlLocators::<T>::get(actor_id),
          Some(ActorControlLocation::Waiting { .. })
        )
        && occurrence_cohort
          .iter()
          .all(|id| benchmark_fixture_hot::<T>(*id).is_some_and(|hot| !hot.pending_signal))
      {
        break;
      }
      if let Some((ActorControlLocation::Ready { .. }, cell)) =
        Pallet::<T>::actor_control_cell(actor_id)
      {
        let current = frame_system::Pallet::<T>::block_number();
        if let Some(next) = cell.eligible_at.filter(|next| *next > current) {
          frame_system::Pallet::<T>::set_block_number(next);
        }
      }
    }
    assert!(
      occurrence_cohort
        .iter()
        .all(|id| benchmark_fixture_hot::<T>(*id).is_some_and(|hot| !hot.pending_signal)),
      "every fired guard completes its Opening before topology measurement"
    );
    assert_contract_derived_retry::<T>(actor_id);
  }

  fn assert_reachable_retry<T: Config>(actor_id: ActorId) {
    assert_contract_derived_retry::<T>(actor_id);
    let state = Pallet::<T>::active_actor_state(actor_id).expect("retry Actor remains active");
    let run = state.run_state.as_ref().expect("retry Run exists");
    assert_eq!(
      run.opening_snapshot.len(),
      Pallet::<T>::opening_surfaces(&state.contract.steps, 0).len()
    );
    assert_eq!(
      run.opening_predicate_results.len() as u32,
      state
        .contract
        .steps
        .iter()
        .map(|step| step
          .precondition
          .as_ref()
          .map_or(0, Precondition::opening_predicate_count))
        .sum::<u32>()
    );
    assert!(state.funding.funding_tracked_assets.is_empty());
    assert!(run.funding_snapshot.is_empty());
  }

  fn assert_contract_derived_retry<T: Config>(actor_id: ActorId) {
    let state = Pallet::<T>::active_actor_state(actor_id).expect("retry Actor remains active");
    assert_eq!(state.hot.cycle_state, CycleState::Suspended);
    assert!(!state.hot.pending_signal);
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Waiting { .. })
    ));
    let contract =
      Pallet::<T>::load_actor_contract(actor_id).expect("retry Contract remains admitted");
    let surfaces = Pallet::<T>::opening_surfaces(&contract.steps, 0);
    let run = state.run_state.as_ref().expect("Opening published the Run");
    assert_eq!(run.cursor, 0);
    assert!(matches!(
      run.last_step_outcome,
      Some(StepOutcome::Failed(TaskFailure {
        retry: RetryClass::Temporary,
        ..
      }))
    ));
    assert_eq!(run.opening_snapshot.len(), surfaces.len());
    assert!(
      surfaces
        .iter()
        .all(|surface| run.opening_snapshot.contains_key(surface))
    );
    assert_eq!(
      run.opening_predicate_results.len(),
      contract
        .steps
        .iter()
        .filter_map(|step| step.precondition.as_ref())
        .flat_map(|precondition| precondition.clauses.iter())
        .flat_map(|clause| clause.iter())
        .filter(|predicate| predicate.timing == ObservationTiming::Opening)
        .count()
    );
    assert!(
      run
        .opening_predicate_results
        .iter()
        .all(|result| *result == Ok(true))
    );
    assert_eq!(
      run.funding_snapshot.len(),
      state.funding.funding_tracked_assets.len()
    );
    assert!(
      state
        .funding
        .funding_tracked_assets
        .iter()
        .all(|asset| run.funding_snapshot.contains_key(asset))
    );
    assert_max_contract_geometry::<T>(actor_id);
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("reachable retry fixture passes full state audit");
  }

  fn reachable_retry_contract<T: Config>() -> Result<
    (
      ContractSteps<T>,
      (T::AssetId, T::AssetId, T::Balance, T::Balance),
    ),
    polkadot_sdk::frame_benchmarking::BenchmarkError,
  > {
    let (mut steps, funding) = retry_contract_with_opening_partition::<T>(
      T::MaxContractSteps::get() * 2,
      T::MaxContractSteps::get() * benchmark_predicate_capacity::<T>(),
    )?;
    retain_admitted_contract_geometry::<T>(ActorType::System, &mut steps, &[0])?;
    Ok((steps, funding))
  }

  fn retry_contract_with_opening_partition<T: Config>(
    opening_amounts: u32,
    opening_predicates: u32,
  ) -> Result<
    (
      ContractSteps<T>,
      (T::AssetId, T::AssetId, T::Balance, T::Balance),
    ),
    polkadot_sdk::frame_benchmarking::BenchmarkError,
  > {
    let step_count = T::MaxContractSteps::get();
    let predicates_per_step = benchmark_predicate_capacity::<T>();
    assert!(opening_amounts <= step_count * 2);
    assert!(opening_predicates <= step_count * predicates_per_step);
    if step_count < 2
      || predicates_per_step == 0
      || predicates_per_step != T::MaxPredicatesPerStep::get()
    {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent the Crossing retry Contract with maximum Opening predicates",
      ));
    }
    let asset_owner: T::AccountId = account("reachable-opening-assets", 0, 0);
    let (asset_a, asset_b, amount_a, amount_b) =
      T::BenchmarkHelper::setup_add_liquidity(&asset_owner)
        .expect("host supplies a valid funded liquidity pair");
    let mut assets = vec![asset_a, asset_b];
    let candidates = T::BenchmarkHelper::setup_predicate_assets(&asset_owner, step_count * 2)
      .expect("Crossing retry assets exist");
    for asset in candidates {
      if !assets.contains(&asset) && assets.len() < (step_count * 2) as usize {
        assets.push(asset);
      }
    }
    assert_eq!(assets.len() as u32, step_count * 2);
    let mut contract_steps = ContractSteps::<T>::default();
    for (step_index, pair) in assets.chunks_exact(2).enumerate() {
      let predicates = (0..predicates_per_step)
        .map(|index| TimedPredicate {
          timing: if step_index as u32 * predicates_per_step + index < opening_predicates {
            ObservationTiming::Opening
          } else {
            ObservationTiming::Current
          },
          predicate: Predicate::BalanceBelow {
            asset: pair[index as usize % 2],
            threshold: <T::Balance as polkadot_sdk::sp_runtime::traits::Bounded>::max_value()
              .saturating_sub(index.saturated_into()),
          },
        })
        .collect();
      let amount = |position: u32| {
        if step_index as u32 * 2 + position < opening_amounts {
          AmountResolution::PercentageAtOpening(Perbill::from_percent(50))
        } else {
          AmountResolution::PercentageOfCurrent(Perbill::from_percent(50))
        }
      };
      let task = ActorTask::AddLiquidity {
        asset_a: pair[0],
        asset_b: pair[1],
        amount_a: amount(0),
        amount_b: amount(1),
        min_lp_out: if step_index == 0 {
          <T::Balance as polkadot_sdk::sp_runtime::traits::Bounded>::max_value()
        } else {
          One::one()
        },
      };
      contract_steps
        .try_push(Step {
          precondition: Some(packed_predicate_clauses::<T>(
            predicates,
            T::MaxPredicatesPerClause::get(),
          )),
          task,
          on_error: if step_index == 0 {
            StepErrorPolicy::RetryLater {
              max_attempts: T::MaxRetryAttempts::get(),
            }
          } else {
            StepErrorPolicy::AbortCycle
          },
        })
        .expect("Crossing retry Contract fits");
    }
    // The funded first attempt rejects its output bound transactionally and enters service Waiting.
    // Amount and predicate prefixes independently select immutable captures.
    let surfaces = Pallet::<T>::opening_surfaces(&contract_steps, 0);
    assert_eq!(surfaces.len() as u32, opening_amounts);
    Ok((contract_steps, (asset_a, asset_b, amount_a, amount_b)))
  }

  fn benchmark_fixture_consume_frame_current_step_service_state<T: Config>(
    actor_id: ActorId,
  ) -> (
    ActiveActorStateOf<T>,
    ActorAdmissionCertificateOf<T>,
    LoadedActorStepOf<T>,
  ) {
    let (state, admission, loaded_step) = Pallet::<T>::load_frame_actor_service_state(actor_id)
      .expect("benchmark frame service state is coherent");
    Pallet::<T>::remove_primary_control_cell_inner(actor_id)
      .expect("benchmark frame source primary is consumed");
    (
      state,
      admission,
      loaded_step.expect("benchmark frame current Step exists"),
    )
  }

  fn install_wakeup_cursor_page<T: Config>(page_id: WakeupPageId, len: u32) {
    let page_size = T::WakeupPageSize::get();
    let page_start = u32::try_from(page_id)
      .expect("benchmark cursor page id fits u32")
      .saturating_mul(page_size);
    let mut page = WakeupCursorPageOf::<T>::default();
    for slot in 0..len {
      let index = page_start.saturating_add(slot);
      let block: BlockNumberFor<T> = 1_000_000u32.saturating_add(index).into();
      page
        .try_push(WakeupKey::Block(block))
        .expect("benchmark cursor page must fit configured bound");
      // Consumed buckets retain their cursor inverse until atomic heap repair.
      ActorWaitingCursorIndices::<T>::insert(WakeupKey::Block(block), index);
    }
    WakeupCursorPages::<T>::insert((WakeupClock::Block, page_id), page);
  }

  fn clear_host_genesis_wakeup_placements<T: Config>() {
    let block_actor_ids = benchmark_fixture_hot_actor_ids::<T>(|hot| hot.wakeup_pointer.is_some());
    for actor_id in block_actor_ids {
      Pallet::<T>::wakeup_substrate_invalidate(actor_id)
        .expect("host genesis Pipeline wakeup placement must be removable");
    }
    let trigger_actor_ids =
      benchmark_fixture_hot_actor_ids::<T>(|hot| hot.trigger_wakeup_pointer.is_some());
    for actor_id in trigger_actor_ids {
      Pallet::<T>::trigger_wakeup_substrate_invalidate_inner(actor_id)
        .expect("host genesis Trigger wakeup placement is coherent")
        .expect("host genesis Trigger wakeup placement must be removable");
    }
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Block), 0);
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Tick), 0);
  }

  fn add_wakeup_cursor_page(page_ids: &mut alloc::vec::Vec<WakeupPageId>, index: u32, size: u32) {
    let page_id = u64::from(index / size);
    if !page_ids.contains(&page_id) {
      page_ids.push(page_id);
    }
  }

  fn prepare_wakeup_cursor_repair<T: Config>(start_index: u32) -> BlockNumberFor<T> {
    let page_size = T::WakeupPageSize::get();
    let cursor_len = T::MaxActiveActors::get();
    assert!(
      page_size > 0 && cursor_len > start_index.saturating_add(1),
      "benchmark requires bounded cursor depth"
    );
    let last_index = cursor_len.saturating_sub(1);
    let tail_page = u64::from(last_index / page_size);
    let tail_len = (last_index % page_size).saturating_add(1);
    let mut page_ids = alloc::vec::Vec::new();
    add_wakeup_cursor_page(&mut page_ids, last_index, page_size);
    if start_index > 0 {
      add_wakeup_cursor_page(&mut page_ids, (start_index - 1) / 2, page_size);
    }
    let mut current = start_index;
    loop {
      add_wakeup_cursor_page(&mut page_ids, current, page_size);
      let left = current.saturating_mul(2).saturating_add(1);
      if left >= cursor_len {
        break;
      }
      add_wakeup_cursor_page(&mut page_ids, left, page_size);
      let right = left.saturating_add(1);
      if right < cursor_len {
        add_wakeup_cursor_page(&mut page_ids, right, page_size);
      }
      current = left;
    }
    for page_id in page_ids {
      let len = if page_id == tail_page {
        tail_len
      } else {
        page_size
      };
      install_wakeup_cursor_page::<T>(page_id, len);
    }
    WakeupCursorLen::<T>::insert(WakeupClock::Block, cursor_len);
    1_000_000u32.saturating_add(start_index).into()
  }

  // Leaf fixtures install only accessed heap pages, not complete Actor/Waiting state.
  fn prepare_upward_cursor_removal<T: Config>(
    removed_index: u32,
  ) -> Result<BlockNumberFor<T>, polkadot_sdk::frame_benchmarking::BenchmarkError> {
    if T::WakeupPageSize::get() != 32 || T::MaxActiveActors::get() != 10_000 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "upward heap diagnostics require the reference 10000-key, 32-slot geometry",
      ));
    }
    clear_host_genesis_wakeup_placements::<T>();
    let mut tail_ancestors = Vec::new();
    let mut index = 9_999u32;
    loop {
      tail_ancestors.push(index);
      if index == 0 {
        break;
      }
      index = (index - 1) / 2;
    }
    let mut page_ids = Vec::new();
    add_wakeup_cursor_page(&mut page_ids, 9_999, 32);
    let mut index = removed_index;
    loop {
      add_wakeup_cursor_page(&mut page_ids, index, 32);
      if index == 0 {
        break;
      }
      index = (index - 1) / 2;
    }
    // Partial accessed pages of the same legal global heap as the complete native witness.
    for page_id in page_ids {
      let first = u32::try_from(page_id).expect("reference page fits u32") * 32;
      let mut page = WakeupCursorPageOf::<T>::default();
      for index in first..(first + 32).min(10_000) {
        let block: BlockNumberFor<T> = if tail_ancestors.contains(&index) {
          u32::BITS - (index + 1).leading_zeros()
        } else {
          100_000 + index
        }
        .into();
        let key = WakeupKey::Block(block);
        page.try_push(key).expect("reference heap page fits");
        ActorWaitingCursorIndices::<T>::insert(key, index);
      }
      WakeupCursorPages::<T>::insert((WakeupClock::Block, page_id), page);
    }
    WakeupCursorLen::<T>::insert(WakeupClock::Block, 10_000);
    assert_wakeup_cursor_page_indices::<T>();
    Ok((100_000 + removed_index).into())
  }

  // Leaf fixtures install only accessed heap pages, not complete Actor/Waiting state.
  fn assert_wakeup_cursor_page_indices<T: Config>() {
    assert_wakeup_clock_page_indices::<T>(WakeupClock::Block);
  }

  fn assert_wakeup_clock_page_indices<T: Config>(expected_clock: WakeupClock) {
    let size = T::WakeupPageSize::get();
    let len = WakeupCursorLen::<T>::get(expected_clock);
    for ((clock, page_id), page) in WakeupCursorPages::<T>::iter() {
      if clock != expected_clock {
        continue;
      }
      let first = u32::try_from(page_id).expect("fixture page fits u32") * size;
      assert!(first < len, "empty tail pages must be reclaimed");
      assert_eq!(page.len() as u32, (len - first).min(size));
      for (slot, key) in page.iter().enumerate() {
        let index = first + slot as u32;
        assert_eq!(key.clock(), expected_clock);
        assert_eq!(ActorWaitingCursorIndices::<T>::get(key), Some(index));
        if index > 0 {
          let parent = (index - 1) / 2;
          if let Some(parent_page) = WakeupCursorPages::<T>::get((clock, u64::from(parent / size)))
          {
            assert!(parent_page[(parent % size) as usize] <= *key);
          }
        }
      }
    }
    for (key, index) in ActorWaitingCursorIndices::<T>::iter() {
      if key.clock() != expected_clock {
        continue;
      }
      assert!(index < len);
      let page = WakeupCursorPages::<T>::get((expected_clock, u64::from(index / size)))
        .expect("retained inverse resolves to an installed page");
      assert_eq!(page.get((index % size) as usize), Some(&key));
    }
  }

  fn install_saturated_tombstone_queue<T: Config>() {
    let capacity = u64::from(T::MaxQueueLength::get());
    let head = ActorReadyTail::<T>::get();
    let tail = head
      .checked_add(capacity)
      .expect("bounded fixture ticket span");
    for page_id in head / 32..tail.div_ceil(32) {
      ActorReadyFrameChunks::<T>::insert(
        page_id,
        ActorControlChunkOf::<T>::try_from(vec![None; 32]).expect("fixed Ready chunk"),
      );
    }
    benchmark_fixture_set_ready_queue_state::<T>(head, tail, 0);
  }

  fn saturated_address_source_filter<T: Config>(
    owner: &T::AccountId,
    matched_source: Option<T::AccountId>,
  ) -> SourceFilter<T::AccountId, T::MaxWhitelistSize> {
    if let Some(matched_source) = matched_source {
      let mut allowed_sources = (0..T::MaxWhitelistSize::get().saturating_sub(2))
        .map(|index| account("ingress-source", index, 0))
        .collect::<alloc::vec::Vec<T::AccountId>>();
      allowed_sources.push(owner.clone());
      allowed_sources.push(matched_source);
      allowed_sources.sort_by_key(Encode::encode);
      allowed_sources.dedup();
      assert_eq!(
        allowed_sources.len() as u32,
        T::MaxWhitelistSize::get(),
        "benchmark source whitelist must saturate MaxWhitelistSize"
      );
      SourceFilter::Whitelist(
        BoundedVec::try_from(allowed_sources).expect("source whitelist must fit runtime bound"),
      )
    } else {
      SourceFilter::Any
    }
  }

  fn prepare_saturated_address_actor<T: Config>(
    seed: u32,
    matched_source: Option<T::AccountId>,
  ) -> (ActorId, T::AccountId) {
    let owner: T::AccountId = account("ingress_owner", seed, 0);
    let source_filter = saturated_address_source_filter::<T>(&owner, matched_source);
    let schedule = Schedule {
      trigger: Trigger::address_event(source_filter, AssetFilter::Any),
      cooldown_blocks: 0,
    };
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner.clone(),
      Mutability::Mutable,
      system_contract::<T>(schedule, make_tracked_funding_contract_steps::<T>(owner)),
    )
    .expect("create_system_actor must succeed in ingress benchmark setup");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let recipient = Pallet::<T>::sovereign_account_id_system(actor_id);
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    let mut contract =
      Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Actor Contract exists");
    contract.funding = FundingSourcePolicy::AnyVerifiedIngress;
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("benchmark ingress Contract remains admitted");
    ActorFunding::<T>::mutate(actor_id, |maybe| {
      let funding = maybe.as_mut().expect("benchmark actor funding exists");
      funding
        .funding_accumulated
        .try_insert(T::FeeNativeAssetId::get(), One::one())
        .expect("tracked funding accumulator fits");
    });
    install_saturated_tombstone_queue::<T>();
    (actor_id, recipient)
  }

  #[benchmark]
  fn scheduler_on_initialize_cutoff() {
    benchmark_fixture_set_next_ready_ticket::<T>(7);
    CurrentBlockResourceState::<T>::kill();
    #[block]
    {
      let now = frame_system::Pallet::<T>::block_number();
      assert!(CurrentBlockResourceState::<T>::get().is_none());
      let budget = T::BlockResourceBudget::get();
      let mut state = BlockResourceState::new(now);
      state.begin_prepass().expect("benchmark prepass opens");
      let mut reservation = state
        .reserve(
          budget.limits(),
          BlockResourceDomain::ActorControl,
          Weight::zero(),
        )
        .expect("zero cutoff owner reserves");
      PrepassExecutionCutoff::<T>::put((now, benchmark_fixture_next_ready_ticket::<T>()));
      state
        .settle(&mut reservation, Weight::zero())
        .expect("zero cutoff owner settles");
      state
        .open_external_phase()
        .expect("empty benchmark prepass closes");
      CurrentBlockResourceState::<T>::put(state);
    }
    assert_eq!(
      PrepassExecutionCutoff::<T>::get(),
      Some((frame_system::Pallet::<T>::block_number(), 7))
    );
    assert_eq!(
      CurrentBlockResourceState::<T>::get().map(|state| state.phase()),
      Some(BlockResourcePhase::ExternalPhase)
    );
  }

  #[benchmark]
  fn scheduler_on_idle_base() {
    let threshold = T::MaxIdleStarvationBlocks::get().max(1);
    let now: BlockNumberFor<T> = threshold.into();
    frame_system::Pallet::<T>::set_block_number(now);
    GlobalCircuitBreaker::<T>::put(false);
    IdleStarvationState::<T>::put(IdleStarvationPhase::Starving {
      consecutive_blocks: 1,
    });
    let mut state = BlockResourceState::new(now);
    state.begin_prepass().expect("benchmark state opens"); // deos-bypass: panic-owner — fresh benchmark state has no reservations.
    state
      .open_external_phase()
      .expect("benchmark prepass closes"); // deos-bypass: panic-owner — preceding transition establishes empty PrepassExecuting.
    CurrentBlockResourceState::<T>::put(state);
    #[block]
    {
      let mut resource_state =
        CurrentBlockResourceState::<T>::get().expect("benchmark resource state exists"); // deos-bypass: panic-owner — setup writes this exact storage value.
      let limits = T::BlockResourceBudget::get().limits();
      let mut reservation = resource_state
        .reserve(
          limits,
          BlockResourceDomain::ActorControl,
          limits.actor_control(),
        )
        .expect("benchmark Actor Control reservation fits"); // deos-bypass: panic-owner — maximum equals the configured empty Actor Control limit.
      resource_state
        .settle(&mut reservation, Weight::zero())
        .expect("benchmark reservation settles"); // deos-bypass: panic-owner — zero actual is component-wise within the reserved maximum.
      CurrentBlockResourceState::<T>::put(resource_state);
      let _breaker_active = GlobalCircuitBreaker::<T>::get();
      core::hint::black_box(benchmark_fixture_ready_head::<T>());
      core::hint::black_box(benchmark_fixture_ready_tail::<T>());
      core::hint::black_box(benchmark_fixture_ready_occupancy::<T>());
      core::hint::black_box(DirtyObservationListState::<T>::get());
      Pallet::<T>::update_idle_starvation_state(now, true);
    }
  }

  #[benchmark]
  fn materialization_coordinator_base() {
    MaterializationFamilyCursor::<T>::put(0);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      core::hint::black_box(Pallet::<T>::materialization_family_has_work(0, now));
      core::hint::black_box(Pallet::<T>::materialization_family_has_work(1, now));
      core::hint::black_box(Pallet::<T>::materialization_family_has_work(2, now));
      let cursor = MaterializationFamilyCursor::<T>::get();
      MaterializationFamilyCursor::<T>::put(cursor.saturating_add(1) % 3);
    }
    assert_eq!(MaterializationFamilyCursor::<T>::get(), 1);
  }

  // Non-dispatch diagnostic benchmark excluded from runtime weight artifact generation
  #[benchmark]
  fn scheduler_on_idle_healthy_empty() {
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    GlobalCircuitBreaker::<T>::put(false);
    benchmark_fixture_set_ready_queue_state::<T>(0, 0, 0);
    WakeupCursorLen::<T>::insert(WakeupClock::Block, 0);
    IdleStarvationState::<T>::kill();
    #[block]
    {
      core::hint::black_box(Pallet::<T>::on_idle(now, Weight::MAX));
    }
    assert!(!IdleStarvationState::<T>::exists());
  }

  #[benchmark(pov_mode = Measured)]
  fn benchmark_chunked_create(c: Linear<0, 2>) {
    let actor_id = 2_951;
    let n = 1u32.saturating_add(c.saturating_mul(4).min(7));
    let contract = system_contract::<T>(
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 0,
      },
      inert_contract_steps_of_len::<T>(n),
    )
    .expect("active benchmark Contract");
    let (certificate, head, chunks) = admitted_contract_geometry::<T>(actor_id, &contract);
    let chunk_count = u32::try_from(chunks.len()).expect("bounded chunk count fits u32");
    assert_eq!(chunk_count, c);
    core::hint::black_box((head, chunks));
    #[block]
    {
      assert!(Pallet::<T>::insert_admitted_contract_geometry(
        actor_id,
        &contract,
        &certificate,
      ));
    }
    // Geometry insertion does not publish process authority or own its admission certificate.
    assert!(!ActorControlLocators::<T>::contains_key(actor_id));
    assert!(ActorContractHeads::<T>::contains_key(actor_id));
    for chunk_index in 0..chunk_count {
      assert!(ActorContractTailChunks::<T>::contains_key(
        actor_id,
        chunk_index
      ));
    }
  }

  #[benchmark(pov_mode = Measured)]
  fn benchmark_monolithic_close(n: Linear<1, 8>) {
    let actor_id = bench_create_system_with_plan::<T>(2_960, inert_contract_steps_of_len::<T>(n));
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::remove_actor_contract(actor_id)
          .expect("benchmark Contract removes coherently"),
      );
    }
    assert!(!ActorContractHeads::<T>::contains_key(actor_id));
  }

  #[benchmark(pov_mode = Measured)]
  fn benchmark_chunked_close(c: Linear<0, 2>) {
    let n = 1u32.saturating_add(c.saturating_mul(4).min(7));
    let actor_id = bench_create_system_with_plan::<T>(2_971, inert_contract_steps_of_len::<T>(n));
    let contract = Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Contract exists");
    install_chunked_contract::<T>(actor_id, &contract);
    let chunk_count = c;
    let authority_before = Pallet::<T>::actor_control_cell(actor_id);
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::remove_admitted_contract_geometry(actor_id)
          .expect("benchmark geometry removes coherently"),
      );
    }
    assert_eq!(Pallet::<T>::actor_control_cell(actor_id), authority_before);
    assert!(!ActorContractHeads::<T>::contains_key(actor_id));
    for chunk_index in 0..chunk_count {
      assert!(!ActorContractTailChunks::<T>::contains_key(
        actor_id,
        chunk_index
      ));
    }
  }

  #[benchmark(pov_mode = Measured)]
  fn benchmark_monolithic_update(n: Linear<1, 8>) {
    let actor_id = bench_create_system_with_plan::<T>(2_975, inert_contract_steps_of_len::<T>(n));
    let mut replacement =
      Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Contract exists");
    replacement.cooldown_blocks = 1;
    #[block]
    {
      Pallet::<T>::store_actor_contract(actor_id, replacement)
        .expect("benchmark replacement remains admitted");
    }
    assert_eq!(
      Pallet::<T>::load_actor_contract(actor_id).map(|contract| contract.cooldown_blocks),
      Some(1)
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn benchmark_monolithic_reconstruct(n: Linear<1, 8>) {
    let actor_id = bench_create_system_with_plan::<T>(2_977, inert_contract_steps_of_len::<T>(n));
    #[block]
    {
      let contract = Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Contract exists");
      let semantic_contract_id = contract.semantic_contract_id();
      let body_commitment = contract
        .body_commitment()
        .expect("bounded benchmark body commitment");
      core::hint::black_box((contract, semantic_contract_id, body_commitment));
    }
  }

  #[benchmark(pov_mode = Measured)]
  fn benchmark_chunked_reconstruct(c: Linear<0, 2>) {
    let n = 1u32.saturating_add(c.saturating_mul(4).min(7));
    let actor_id = bench_create_system_with_plan::<T>(2_979, inert_contract_steps_of_len::<T>(n));
    let contract = Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Contract exists");
    install_chunked_contract::<T>(actor_id, &contract);
    assert_eq!(
      u32::try_from(n.saturating_sub(1).div_ceil(4)).expect("chunk count fits"),
      c
    );
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::load_admitted_contract_geometry(actor_id)
          .expect("benchmark geometry reconstructs coherently"),
      );
    }
  }

  #[benchmark(pov_mode = Measured)]
  fn benchmark_monolithic_load_first() {
    let actor_id = bench_create_system_with_plan::<T>(2_980, inert_contract_steps_of_len::<T>(1));
    #[block]
    {
      let contract = Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Contract exists");
      core::hint::black_box(contract.steps.first().expect("benchmark Step 0 exists"));
    }
  }

  #[benchmark(pov_mode = Measured)]
  fn benchmark_monolithic_load_tail(n: Linear<2, 8>) {
    let actor_id = bench_create_system_with_plan::<T>(2_991, inert_contract_steps_of_len::<T>(n));
    let current = n.saturating_sub(1);
    #[block]
    {
      let contract = Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Contract exists");
      core::hint::black_box(
        contract
          .steps
          .get(current as usize)
          .expect("benchmark tail Step exists"),
      );
    }
  }

  #[benchmark(pov_mode = Measured)]
  fn benchmark_chunked_load_first() {
    let actor_id = bench_create_system_with_plan::<T>(2_992, inert_contract_steps_of_len::<T>(1));
    let contract = Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Contract exists");
    install_chunked_contract::<T>(actor_id, &contract);
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::load_current_step_from_storage(actor_id, 0)
          .expect("benchmark head Step loads coherently"),
      );
    }
  }

  #[benchmark(pov_mode = Measured)]
  fn benchmark_chunked_load_tail(
    s: Linear<
      1,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .clamp(1, MAX_STEPS_PER_TAIL_CHUNK)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    if T::MaxContractSteps::get() <= 1 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host Contract bound cannot represent this tail benchmark branch",
      ));
    }
    let n = 1u32.saturating_add(s);
    let actor_id = bench_create_system_with_plan::<T>(2_993, inert_contract_steps_of_len::<T>(n));
    let contract = Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Contract exists");
    install_chunked_contract::<T>(actor_id, &contract);
    let current = s;
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::load_current_step_from_storage(actor_id, current)
          .expect("benchmark tail Step loads coherently"),
      );
    }
    Ok(())
  }

  #[benchmark(pov_mode = Measured)]
  fn contract_geometry_create(c: Linear<0, 3>) {
    let actor_id = 2_994;
    let n = 1u32.saturating_add(
      c.saturating_mul(MAX_STEPS_PER_TAIL_CHUNK)
        .min(T::MaxContractSteps::get().saturating_sub(1)),
    );
    let contract = system_contract::<T>(
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 0,
      },
      inert_contract_steps_of_len::<T>(n),
    )
    .expect("active benchmark Contract");
    let (certificate, _, chunks) = admitted_contract_geometry::<T>(actor_id, &contract);
    assert_eq!(u32::try_from(chunks.len()).expect("chunk count fits"), c);
    #[block]
    {
      assert!(Pallet::<T>::insert_admitted_contract_geometry(
        actor_id,
        &contract,
        &certificate,
      ));
    }
    assert!(!ActorControlLocators::<T>::contains_key(actor_id));
    assert!(ActorContractHeads::<T>::contains_key(actor_id));
  }

  #[benchmark(pov_mode = Measured)]
  fn contract_geometry_close(c: Linear<0, 3>) {
    let n = 1u32.saturating_add(
      c.saturating_mul(MAX_STEPS_PER_TAIL_CHUNK)
        .min(T::MaxContractSteps::get().saturating_sub(1)),
    );
    let actor_id = bench_create_system_with_plan::<T>(2_995, inert_contract_steps_of_len::<T>(n));
    let contract = Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Contract exists");
    install_chunked_contract::<T>(actor_id, &contract);
    let authority_before = Pallet::<T>::actor_control_cell(actor_id);
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::remove_admitted_contract_geometry(actor_id)
          .expect("benchmark geometry removes coherently"),
      );
    }
    assert_eq!(Pallet::<T>::actor_control_cell(actor_id), authority_before);
    assert!(!ActorContractHeads::<T>::contains_key(actor_id));
  }

  #[benchmark(pov_mode = Measured)]
  fn contract_geometry_reconstruct(c: Linear<0, 3>) {
    let n = 1u32.saturating_add(
      c.saturating_mul(MAX_STEPS_PER_TAIL_CHUNK)
        .min(T::MaxContractSteps::get().saturating_sub(1)),
    );
    let actor_id = bench_create_system_with_plan::<T>(2_996, inert_contract_steps_of_len::<T>(n));
    let contract = Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Contract exists");
    install_chunked_contract::<T>(actor_id, &contract);
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::load_admitted_contract_geometry(actor_id)
          .expect("benchmark geometry reconstructs coherently"),
      );
    }
  }

  #[benchmark(pov_mode = Measured)]
  fn current_step_load_head() {
    let actor_id = bench_create_system_with_plan::<T>(2_997, inert_contract_steps_of_len::<T>(1));
    let contract = Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Contract exists");
    install_chunked_contract::<T>(actor_id, &contract);
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::load_current_step_from_storage(actor_id, 0)
          .expect("benchmark head Step loads coherently"),
      );
    }
  }

  #[benchmark(pov_mode = Measured)]
  fn current_step_load_tail(
    s: Linear<
      1,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .clamp(1, MAX_STEPS_PER_TAIL_CHUNK)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    if T::MaxContractSteps::get() <= 1 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host Contract bound cannot represent this tail benchmark branch",
      ));
    }
    let n = 1u32.saturating_add(s);
    let actor_id = bench_create_system_with_plan::<T>(2_998, inert_contract_steps_of_len::<T>(n));
    let contract = Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Contract exists");
    install_chunked_contract::<T>(actor_id, &contract);
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::load_current_step_from_storage(actor_id, s)
          .expect("benchmark tail Step loads coherently"),
      );
    }
    Ok(())
  }

  #[benchmark(pov_mode = Measured)]
  fn current_step_plan_opening_head() {
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let actor_id = bench_create_system_with_plan::<T>(2_999, inert_contract_steps_of_len::<T>(1));
    let contract = Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Contract exists");
    install_chunked_contract::<T>(actor_id, &contract);
    benchmark_fixture_mutate_hot::<T>(actor_id, |hot| {
      hot.queue_ticket = Some(9);
    });
    let identity = benchmark_fixture_identity::<T>(actor_id).expect("benchmark identity exists");
    let certificate =
      benchmark_fixture_admission::<T>(actor_id).expect("benchmark admission certificate exists");
    let ticket = ActorStepTicket {
      actor_id,
      cycle_nonce: identity.cycle_nonce.saturating_add(1),
      cursor: 0,
      ticket: 9,
      eligible_at: now,
      contract_commitment: ActorContractCommitment {
        semantic_contract_id: certificate.semantic_contract_id,
        body_commitment: certificate.body_commitment,
      },
    };
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::load_current_step_plan_from_storage(ticket)
          .expect("benchmark Opening plan loads coherently"),
      );
    }
  }

  #[benchmark(pov_mode = Measured)]
  fn current_step_plan_suspended_head()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    // Funding-heavy host-feasible corner; frontier cases verify reachability, not a Weight envelope.
    let opening_legs = T::MaxContractSteps::get()
      .saturating_mul(2)
      .saturating_sub(T::MaxFundingTrackedAssets::get());
    let (actor_id, ticket) = prepare_reachable_suspended_head::<T>(opening_legs, 0)?;
    let retained = ActorRunStateStore::<T>::get(actor_id)
      .expect("due Run exists")
      .encode();
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::load_current_step_plan_from_storage(ticket)
          .expect("real Suspended head plan loads coherently"),
      );
    }
    assert_eq!(
      ActorRunStateStore::<T>::get(actor_id)
        .expect("read-only plan load retains Run")
        .encode(),
      retained
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("read-only plan loading preserves full state audit");
    Ok(())
  }

  #[benchmark(pov_mode = Measured)]
  fn current_step_plan_running_tail(
    s: Linear<
      1,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .clamp(1, MAX_STEPS_PER_TAIL_CHUNK)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, ticket) = prepare_reachable_running_tail::<T>(s)?;
    let retained = ActorRunStateStore::<T>::get(actor_id)
      .expect("real tail Run exists")
      .encode();
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::load_current_step_plan_from_storage(ticket)
          .expect("real Running plan loads coherently"),
      );
    }
    assert_reachable_running_tail_unchanged::<T>(actor_id, &retained);
    Ok(())
  }

  #[benchmark(pov_mode = Measured)]
  fn opening_snapshot_traversal() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor: T::AccountId = account("opening_snapshot_actor", 0, 0);
    let assets = T::BenchmarkHelper::setup_predicate_assets(&actor, 2).map_err(|_| {
      polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot prepare two distinct Opening assets",
      )
    })?;
    if assets.len() != 2 || assets[0] == assets[1] {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "Opening traversal requires two distinct assets",
      ));
    }
    let steps: ContractSteps<T> = vec![
      Step {
        precondition: None,
        task: ActorTask::AddLiquidity {
          asset_a: assets[0],
          asset_b: assets[1],
          amount_a: AmountResolution::Fixed(One::one()),
          amount_b: AmountResolution::Fixed(One::one()),
          min_lp_out: One::one(),
        },
        on_error: StepErrorPolicy::AbortCycle,
      };
      T::MaxContractSteps::get() as usize
    ]
    .try_into()
    .expect("full Contract Step count fits");
    let snapshot;
    #[block]
    {
      snapshot =
        Pallet::<T>::capture_opening_snapshot(ActorType::System, &actor, &steps, Zero::zero());
      core::hint::black_box(&snapshot);
    }
    assert!(snapshot.is_empty());
    Ok(())
  }

  fn prepare_opening_snapshot_assets<T: Config>(e: u32) -> (T::AccountId, Vec<T::AssetId>) {
    let actor: T::AccountId = account("opening_snapshot_actor", 0, 0);
    let assets: Vec<_> =
      T::BenchmarkHelper::setup_max_encoded_predicate_assets(&actor, e.max(2) + 1)
        .expect("maximum encoded Opening assets exist")
        .into_iter()
        .filter(|asset| *asset != T::FeeNativeAssetId::get())
        .take(e.max(2) as usize)
        .collect();
    assert_eq!(assets.len() as u32, e.max(2));
    assert_eq!(
      assets
        .iter()
        .collect::<alloc::collections::BTreeSet<_>>()
        .len(),
      assets.len()
    );
    for asset in assets.iter().take(e as usize) {
      let minimum = T::AssetOps::minimum_balance(*asset);
      let amount = minimum
        .checked_add(&One::one())
        .expect("funded capture amount fits");
      T::AssetOps::mint(&actor, *asset, amount).expect("Opening capture asset is funded");
      assert!(T::AssetOps::balance(&actor, *asset) > minimum);
    }
    (actor, assets)
  }

  #[benchmark(pov_mode = Measured)]
  fn opening_snapshot_capture(
    e: Linear<
      1,
      { T::MaxOpeningSnapshotEntries::get().min(T::MaxContractSteps::get().saturating_mul(2)) },
    >,
  ) {
    let (actor, assets) = prepare_opening_snapshot_assets::<T>(e);
    let mut steps = ContractSteps::<T>::default();
    for index in 0..T::MaxContractSteps::get() {
      let asset_a = assets[(index * 2).min(e - 1) as usize];
      let asset_b = if e == 1 {
        assets[1]
      } else {
        assets[if index * 2 + 1 < e {
          index * 2 + 1
        } else {
          e - 2
        } as usize]
      };
      assert_ne!(asset_a, asset_b);
      steps
        .try_push(Step {
          precondition: None,
          task: ActorTask::AddLiquidity {
            asset_a,
            asset_b,
            amount_a: AmountResolution::PercentageAtOpening(Perbill::one()),
            amount_b: if e == 1 {
              AmountResolution::Fixed(One::one())
            } else {
              AmountResolution::PercentageAtOpening(Perbill::one())
            },
            min_lp_out: One::one(),
          },
          on_error: StepErrorPolicy::AbortCycle,
        })
        .expect("benchmark Opening Step fits");
    }
    let snapshot;
    #[block]
    {
      snapshot =
        Pallet::<T>::capture_opening_snapshot(ActorType::System, &actor, &steps, Zero::zero());
      core::hint::black_box(&snapshot);
    }
    assert_eq!(
      u32::try_from(snapshot.len()).expect("snapshot count fits"),
      e
    );
    for asset in assets.iter().take(e as usize) {
      let expected =
        T::AssetOps::balance(&actor, *asset).saturating_sub(T::AssetOps::minimum_balance(*asset));
      assert!(!expected.is_zero());
      assert_eq!(
        snapshot.get(&OpeningSurface::PreservableAsset(*asset)),
        Some(&expected)
      );
    }
  }

  #[benchmark(pov_mode = Measured)]
  fn opening_target_snapshot_capture(
    e: Linear<1, { T::MaxOpeningSnapshotEntries::get().min(T::MaxContractSteps::get()) }>,
  ) {
    let (actor, assets) = prepare_opening_snapshot_assets::<T>(e);
    let steps: ContractSteps<T> = (0..T::MaxContractSteps::get())
      .map(|index| Step {
        precondition: None,
        task: ActorTask::Mint {
          asset: assets[index.min(e - 1) as usize],
          amount: AmountResolution::PercentageAtOpening(Perbill::one()),
        },
        on_error: StepErrorPolicy::AbortCycle,
      })
      .collect::<Vec<_>>()
      .try_into()
      .expect("full target-surface Contract fits");
    let snapshot;
    #[block]
    {
      snapshot =
        Pallet::<T>::capture_opening_snapshot(ActorType::System, &actor, &steps, Zero::zero());
      core::hint::black_box(&snapshot);
    }
    assert_eq!(snapshot.len() as u32, e);
    for asset in assets.iter().take(e as usize) {
      assert_eq!(
        snapshot.get(&OpeningSurface::TargetAsset(*asset)),
        Some(&T::AssetOps::balance(&actor, *asset)),
      );
    }
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn opening_staking_snapshot_capture(
    e: Linear<1, { T::MaxOpeningSnapshotEntries::get().min(T::MaxContractSteps::get()) }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor: T::AccountId = account("opening_snapshot_actor", 0, 0);
    let positions = T::BenchmarkHelper::setup_max_encoded_staking_positions(&actor, e)?;
    assert_eq!(positions.len() as u32, e);
    assert_eq!(
      positions
        .iter()
        .map(|(asset, _)| asset)
        .collect::<alloc::collections::BTreeSet<_>>()
        .len(),
      positions.len()
    );
    let steps: ContractSteps<T> = (0..T::MaxContractSteps::get())
      .map(|index| Step {
        precondition: None,
        task: ActorTask::Unstake {
          asset: positions[index.min(e - 1) as usize].0,
          shares: AmountResolution::PercentageAtOpening(Perbill::one()),
        },
        on_error: StepErrorPolicy::AbortCycle,
      })
      .collect::<Vec<_>>()
      .try_into()
      .expect("full staking-surface Contract fits");
    let snapshot;
    #[block]
    {
      snapshot =
        Pallet::<T>::capture_opening_snapshot(ActorType::System, &actor, &steps, Zero::zero());
      core::hint::black_box(&snapshot);
    }
    assert_eq!(snapshot.len() as u32, e);
    for (asset, shares) in positions {
      assert!(!shares.is_zero());
      assert_eq!(
        snapshot.get(&OpeningSurface::StakingShares(asset)),
        Some(&shares)
      );
    }
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn opening_native_snapshot_capture(
    c: Linear<0, 17>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    if T::MaxContractSteps::get() < 2 || T::MaxOpeningSnapshotEntries::get() < 2 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent two typed Native Opening surfaces",
      ));
    }
    let actor: T::AccountId = account("opening_native_snapshot_actor", 0, 0);
    let native = T::FeeNativeAssetId::get();
    let other = T::BenchmarkHelper::setup_predicate_assets(&actor, 2)?
      .into_iter()
      .find(|asset| *asset != native)
      .expect("distinct fixed auxiliary asset exists");
    let amount = T::MinUserBalance::get()
      .checked_add(&T::AssetOps::minimum_balance(native))
      .and_then(|amount| amount.checked_add(&100u32.into()))
      .expect("benchmark Native funding fits");
    T::AssetOps::mint(&actor, native, amount).expect("Native snapshot custody is funded");
    let raw = T::AssetOps::balance(&actor, native);
    assert!(!raw.is_zero());
    let actor_type = if c / 9 == 0 {
      ActorType::System
    } else {
      ActorType::User
    };
    let family = (c % 9) / 3;
    let reserved = match c % 3 {
      0 => Zero::zero(),
      1 => raw / 2u32.into(),
      _ => raw,
    };
    let steps: ContractSteps<T> = (0..T::MaxContractSteps::get())
      .map(|index| Step {
        precondition: None,
        task: if family == 0 || (family == 2 && index % 2 == 0) {
          ActorTask::AddLiquidity {
            asset_a: native,
            asset_b: other,
            amount_a: AmountResolution::PercentageAtOpening(Perbill::one()),
            amount_b: AmountResolution::Fixed(One::one()),
            min_lp_out: One::one(),
          }
        } else {
          ActorTask::SwapOut {
            asset_out: native,
            amount_out: AmountResolution::PercentageAtOpening(Perbill::one()),
            asset_in: other,
            input_limit: InputLimit::Absolute(One::one()),
            slippage_tolerance: Perbill::zero(),
          }
        },
        on_error: StepErrorPolicy::AbortCycle,
      })
      .collect::<Vec<_>>()
      .try_into()
      .expect("full Native snapshot Contract fits");
    let snapshot;
    #[block]
    {
      snapshot = Pallet::<T>::capture_opening_snapshot(actor_type, &actor, &steps, reserved);
      core::hint::black_box(&snapshot);
    }
    assert_eq!(snapshot.len(), if family == 2 { 2 } else { 1 });
    let spendable = raw.saturating_sub(reserved);
    if family != 1 {
      let minimum = if actor_type == ActorType::User {
        T::MinUserBalance::get()
      } else {
        T::AssetOps::minimum_balance(native)
      };
      assert_eq!(
        snapshot.get(&OpeningSurface::PreservableAsset(native)),
        Some(&spendable.saturating_sub(minimum)),
      );
    }
    if family != 0 {
      assert_eq!(
        snapshot.get(&OpeningSurface::TargetAsset(native)),
        Some(&spendable)
      );
    }
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn opening_unfunded_snapshot_capture(
    c: Linear<
      0,
      {
        T::MaxOpeningSnapshotEntries::get().min(T::MaxContractSteps::get().saturating_mul(2))
          + 2 * T::MaxOpeningSnapshotEntries::get().min(T::MaxContractSteps::get())
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let paired =
      T::MaxOpeningSnapshotEntries::get().min(T::MaxContractSteps::get().saturating_mul(2));
    let single = T::MaxOpeningSnapshotEntries::get().min(T::MaxContractSteps::get());
    if single == 0 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent an Opening surface",
      ));
    }
    let (family, count) = if c < paired {
      (0, c + 1)
    } else if c < paired + single {
      (1, c - paired + 1)
    } else if c < paired + 2 * single {
      (2, c - paired - single + 1)
    } else {
      (3, 1)
    };
    let owner: T::AccountId = account("opening_snapshot_reference_owner", 0, 0);
    let actor: T::AccountId = account("opening_unfunded_snapshot_actor", 0, 0);
    assert_ne!(owner, actor);
    let assets: Vec<_> = match family {
      0 | 1 => T::BenchmarkHelper::setup_max_encoded_predicate_assets(&owner, count.max(2) + 1)?
        .into_iter()
        .filter(|asset| *asset != T::FeeNativeAssetId::get())
        .take(count.max(2) as usize)
        .collect(),
      2 => T::BenchmarkHelper::setup_max_encoded_staking_positions(&owner, count)?
        .into_iter()
        .map(|(asset, _)| asset)
        .collect(),
      _ => {
        let (asset, _) = T::BenchmarkHelper::setup_stake(&owner)?;
        assert!(T::StakingOps::share_asset(asset).is_some());
        T::BenchmarkHelper::remove_empty_staking_receipt(&owner, asset)?;
        assert!(T::StakingOps::share_asset(asset).is_none());
        vec![asset]
      }
    };
    assert_eq!(
      assets.len() as u32,
      if family < 2 { count.max(2) } else { count }
    );
    assert_eq!(
      assets
        .iter()
        .collect::<alloc::collections::BTreeSet<_>>()
        .len(),
      assets.len()
    );
    let steps: ContractSteps<T> = (0..T::MaxContractSteps::get())
      .map(|index| Step {
        precondition: None,
        task: match family {
          0 => ActorTask::AddLiquidity {
            asset_a: assets[(index * 2).min(count - 1) as usize],
            asset_b: assets[if count == 1 {
              1
            } else if index * 2 + 1 < count {
              index * 2 + 1
            } else {
              count - 2
            } as usize],
            amount_a: AmountResolution::PercentageAtOpening(Perbill::one()),
            amount_b: if count == 1 {
              AmountResolution::Fixed(One::one())
            } else {
              AmountResolution::PercentageAtOpening(Perbill::one())
            },
            min_lp_out: One::one(),
          },
          1 => ActorTask::Mint {
            asset: assets[index.min(count - 1) as usize],
            amount: AmountResolution::PercentageAtOpening(Perbill::one()),
          },
          _ => ActorTask::Unstake {
            asset: assets[index.min(count - 1) as usize],
            shares: AmountResolution::PercentageAtOpening(Perbill::one()),
          },
        },
        on_error: StepErrorPolicy::AbortCycle,
      })
      .collect::<Vec<_>>()
      .try_into()
      .expect("full unfunded snapshot Contract fits");
    let snapshot;
    #[block]
    {
      snapshot =
        Pallet::<T>::capture_opening_snapshot(ActorType::System, &actor, &steps, Zero::zero());
      core::hint::black_box(&snapshot);
    }
    assert_eq!(snapshot.len() as u32, count);
    for asset in assets.into_iter().take(count as usize) {
      let surface = match family {
        0 => OpeningSurface::PreservableAsset(asset),
        1 => OpeningSurface::TargetAsset(asset),
        _ => OpeningSurface::StakingShares(asset),
      };
      assert_eq!(snapshot.get(&surface), Some(&Zero::zero()));
    }
    Ok(())
  }

  fn opening_mixed_allocations(step_limit: u32, entry_limit: u32) -> Vec<(u32, u32, u32)> {
    let mut allocations = Vec::new();
    for total in 2..=entry_limit.min(step_limit.saturating_mul(2)) {
      for preservable in 0..=total {
        for target in 0..=total.saturating_sub(preservable).min(step_limit) {
          let shares = total - preservable - target;
          if preservable.div_ceil(2) + target + shares <= step_limit
            && u32::from(preservable > 0) + u32::from(target > 0) + u32::from(shares > 0) >= 2
          {
            allocations.push((preservable, target, shares));
          }
        }
      }
    }
    allocations
  }

  fn prepare_mixed_opening_snapshot<T: Config>(
    preservable: u32,
    target: u32,
    share_count: u32,
    reverse: bool,
  ) -> Result<
    (T::AccountId, ContractSteps<T>, RunOpeningSnapshotOf<T>),
    polkadot_sdk::frame_benchmarking::BenchmarkError,
  > {
    let total = preservable + target + share_count;
    let actor: T::AccountId = account("opening_snapshot_actor", 0, 0);
    let available =
      T::BenchmarkHelper::setup_max_encoded_predicate_assets(&actor, total + share_count + 1)?;
    let positions = if share_count == 0 {
      Vec::new()
    } else {
      T::BenchmarkHelper::setup_max_encoded_staking_positions(&actor, share_count)?
    };
    assert_eq!(positions.len() as u32, share_count);
    let mut position_assets = alloc::collections::BTreeSet::new();
    for (asset, shares) in &positions {
      assert!(!shares.is_zero());
      assert!(position_assets.insert(*asset));
    }
    let receipts: alloc::collections::BTreeSet<_> = positions
      .iter()
      .map(|(asset, _)| T::StakingOps::share_asset(*asset).expect("funded position has a receipt"))
      .collect();
    assert_eq!(receipts.len(), positions.len());
    let assets: Vec<_> = available
      .into_iter()
      .filter(|asset| {
        *asset != T::FeeNativeAssetId::get()
          && !position_assets.contains(asset)
          && !receipts.contains(asset)
      })
      .take((preservable + target).max(2) as usize)
      .collect();
    assert!(assets.len() as u32 >= preservable + target);
    assert_eq!(
      assets
        .iter()
        .collect::<alloc::collections::BTreeSet<_>>()
        .len(),
      assets.len()
    );
    let mut expected = RunOpeningSnapshotOf::<T>::default();
    for (index, asset) in assets
      .iter()
      .take((preservable + target) as usize)
      .enumerate()
    {
      let minimum = T::AssetOps::minimum_balance(*asset);
      let amount = minimum
        .checked_add(&One::one())
        .expect("mixed capture funding fits");
      T::AssetOps::mint(&actor, *asset, amount).expect("mixed capture asset is funded");
      let balance = T::AssetOps::balance(&actor, *asset);
      let (surface, value) = if (index as u32) < preservable {
        (
          OpeningSurface::PreservableAsset(*asset),
          balance.saturating_sub(minimum),
        )
      } else {
        (OpeningSurface::TargetAsset(*asset), balance)
      };
      assert!(!value.is_zero());
      expected
        .try_insert(surface, value)
        .expect("mixed expected snapshot fits");
    }
    let mut steps = Vec::new();
    for pair in assets[..preservable as usize].chunks(2) {
      let other = pair.get(1).copied().unwrap_or_else(|| {
        assets
          .iter()
          .copied()
          .chain(position_assets.iter().copied())
          .find(|asset| *asset != pair[0])
          .expect("fixed auxiliary asset is distinct")
      });
      steps.push(Step {
        precondition: None,
        task: ActorTask::AddLiquidity {
          asset_a: pair[0],
          asset_b: other,
          amount_a: AmountResolution::PercentageAtOpening(Perbill::one()),
          amount_b: if pair.len() == 2 {
            AmountResolution::PercentageAtOpening(Perbill::one())
          } else {
            AmountResolution::Fixed(One::one())
          },
          min_lp_out: One::one(),
        },
        on_error: StepErrorPolicy::AbortCycle,
      });
    }
    let repeated = steps.last().cloned();
    for asset in assets
      .iter()
      .skip(preservable as usize)
      .take(target as usize)
    {
      steps.push(Step {
        precondition: None,
        task: ActorTask::Mint {
          asset: *asset,
          amount: AmountResolution::PercentageAtOpening(Perbill::one()),
        },
        on_error: StepErrorPolicy::AbortCycle,
      });
    }
    for (asset, shares) in positions {
      expected
        .try_insert(OpeningSurface::StakingShares(asset), shares)
        .expect("mixed share snapshot fits");
      steps.push(Step {
        precondition: None,
        task: ActorTask::Unstake {
          asset,
          shares: AmountResolution::PercentageAtOpening(Perbill::one()),
        },
        on_error: StepErrorPolicy::AbortCycle,
      });
    }
    let repeated = repeated.unwrap_or_else(|| steps.last().expect("mixed Steps exist").clone());
    assert!(steps.len() as u32 <= T::MaxContractSteps::get());
    steps.resize(T::MaxContractSteps::get() as usize, repeated);
    if reverse {
      steps.reverse();
    }
    let steps: ContractSteps<T> = steps.try_into().expect("full mixed Contract fits");
    assert_eq!(expected.len() as u32, total);
    Ok((actor, steps, expected))
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn opening_mixed_snapshot_capture(
    c: Linear<
      0,
      {
        (opening_mixed_allocations(
          T::MaxContractSteps::get(),
          T::MaxOpeningSnapshotEntries::get(),
        )
        .len() as u32)
          .saturating_mul(2)
          .saturating_sub(1)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let allocations = opening_mixed_allocations(
      T::MaxContractSteps::get(),
      T::MaxOpeningSnapshotEntries::get(),
    );
    let &(preservable, target, share_count) = allocations.get((c / 2) as usize).ok_or(
      polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent this mixed Opening allocation",
      ),
    )?;
    let (actor, steps, expected) =
      prepare_mixed_opening_snapshot::<T>(preservable, target, share_count, c % 2 == 1)?;
    let snapshot;
    #[block]
    {
      snapshot =
        Pallet::<T>::capture_opening_snapshot(ActorType::System, &actor, &steps, Zero::zero());
      core::hint::black_box(&snapshot);
    }
    assert_eq!(snapshot.len() as u32, preservable + target + share_count);
    assert_eq!(snapshot, expected);
    Ok(())
  }

  #[benchmark(pov_mode = Measured)]
  fn opening_share_mixed_capture(
    e: Linear<
      1,
      {
        T::MaxOpeningSnapshotEntries::get().min(
          T::MaxContractSteps::get()
            .saturating_mul(2)
            .saturating_sub(1),
        )
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor, steps, expected) = prepare_mixed_opening_snapshot::<T>(e - 1, 0, 1, true)?;
    let snapshot;
    #[block]
    {
      snapshot =
        Pallet::<T>::capture_opening_snapshot(ActorType::System, &actor, &steps, Zero::zero());
      core::hint::black_box(&snapshot);
    }
    assert_eq!(snapshot.len() as u32, e);
    assert_eq!(snapshot, expected);
    Ok(())
  }

  fn opening_predicate_capture_steps<T: Config>(
    predicates: Vec<Predicate<T::AssetId, T::Balance, u32, T::ObservationFeedId>>,
  ) -> ContractSteps<T> {
    let mut steps = ContractSteps::<T>::default();
    for chunk in predicates.chunks(benchmark_predicate_capacity::<T>() as usize) {
      let predicates = chunk
        .iter()
        .cloned()
        .map(|predicate| TimedPredicate {
          timing: ObservationTiming::Opening,
          predicate,
        })
        .collect();
      steps
        .try_push(Step {
          precondition: Some(packed_predicate_clauses::<T>(
            predicates,
            T::MaxPredicatesPerClause::get(),
          )),
          task: ActorTask::StopCycle,
          on_error: StepErrorPolicy::AbortCycle,
        })
        .expect("benchmark predicate Step fits");
    }
    steps
  }

  #[benchmark(pov_mode = Measured)]
  fn opening_predicate_traversal() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor, steps) = prepare_full_opening_predicate_capture::<T>(0, 0, false)?;
    let results;
    #[block]
    {
      results = Pallet::<T>::capture_opening_predicate_results(&actor, &steps, Zero::zero());
      core::hint::black_box(&results);
    }
    assert!(results.is_empty());
    Ok(())
  }

  #[benchmark(pov_mode = Measured)]
  fn opening_predicate_capture(
    p: Linear<
      0,
      {
        T::MaxOpeningPredicateResults::get()
          .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()))
          .max(1)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let maximum = T::MaxOpeningPredicateResults::get()
      .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()));
    if maximum == 0 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host has no representable predicate capacity for this benchmark",
      ));
    }
    let observations = u32::from(p >= 2);
    let (actor, steps) =
      prepare_full_opening_predicate_capture::<T>(p - observations, observations, false)?;
    let results;
    #[block]
    {
      results = Pallet::<T>::capture_opening_predicate_results(&actor, &steps, Zero::zero());
      core::hint::black_box(&results);
    }
    assert_eq!(u32::try_from(results.len()).expect("result count fits"), p);
    assert!(results.iter().all(|result| *result == Ok(true)));
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn opening_observation_capture(
    p: Linear<
      1,
      {
        T::MaxOpeningPredicateResults::get()
          .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()))
          .max(1)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    if benchmark_predicate_capacity::<T>() == 0 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host has no representable predicate capacity for this benchmark",
      ));
    }
    let actor: T::AccountId = account("opening_predicate_actor", 0, 0);
    let feeds = T::BenchmarkHelper::setup_max_encoded_observation_feeds(p)
      .expect("maximum-encoded Opening observation feeds are published");
    assert_eq!(feeds.len() as u32, p);
    assert_eq!(
      feeds
        .iter()
        .collect::<alloc::collections::BTreeSet<_>>()
        .len() as u32,
      p
    );
    let steps = opening_predicate_capture_steps::<T>(
      feeds
        .into_iter()
        .map(|feed| Predicate::ObservationAbove {
          feed,
          threshold: 0,
          max_age_blocks: 100,
        })
        .collect(),
    );
    let results;
    #[block]
    {
      results = Pallet::<T>::capture_opening_predicate_results(&actor, &steps, Zero::zero());
      core::hint::black_box(&results);
    }
    assert_eq!(results.len() as u32, p);
    assert!(results.iter().all(|result| *result == Ok(true)));
    Ok(())
  }

  fn prepare_observation_read_state<T: Config>(
    p: u32,
    s: u32,
    max_age_blocks: u32,
  ) -> Result<(Vec<T::ObservationFeedId>, bool), polkadot_sdk::frame_benchmarking::BenchmarkError>
  {
    let state = match s {
      0 => BenchmarkObservationReadState::Missing,
      1 => BenchmarkObservationReadState::Deactivated,
      2 => BenchmarkObservationReadState::Uninitialized,
      3 => BenchmarkObservationReadState::Stale,
      _ => BenchmarkObservationReadState::Fresh,
    };
    let feeds = T::BenchmarkHelper::setup_observation_read_state(p, state, max_age_blocks)?;
    assert_eq!(feeds.len() as u32, p);
    assert_eq!(
      feeds
        .iter()
        .collect::<alloc::collections::BTreeSet<_>>()
        .len(),
      feeds.len()
    );
    let now = frame_system::Pallet::<T>::block_number();
    for feed in &feeds {
      assert!(matches!(
        (
          state,
          T::ObservationProvider::observe(feed, now, max_age_blocks)
        ),
        (
          BenchmarkObservationReadState::Missing | BenchmarkObservationReadState::Deactivated,
          ScalarObservationState::Unavailable
        ) | (
          BenchmarkObservationReadState::Uninitialized,
          ScalarObservationState::Uninitialized
        ) | (
          BenchmarkObservationReadState::Stale,
          ScalarObservationState::Stale
        ) | (
          BenchmarkObservationReadState::Fresh,
          ScalarObservationState::Fresh { .. }
        )
      ));
    }
    Ok((feeds, state == BenchmarkObservationReadState::Fresh))
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn opening_observation_read_state(
    p: Linear<
      1,
      {
        T::MaxOpeningPredicateResults::get()
          .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()))
          .max(1)
      },
    >,
    s: Linear<0, 4>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let maximum = T::MaxOpeningPredicateResults::get()
      .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()));
    if maximum == 0 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent Opening predicates",
      ));
    }
    let max_age_blocks = 100;
    let (feeds, expected) = prepare_observation_read_state::<T>(p, s, max_age_blocks)?;
    let actor: T::AccountId = account("opening_predicate_actor", 0, 0);
    let steps = opening_predicate_capture_steps::<T>(
      feeds
        .into_iter()
        .map(|feed| Predicate::ObservationAbove {
          feed,
          threshold: 0,
          max_age_blocks,
        })
        .collect(),
    );
    let results;
    #[block]
    {
      results = Pallet::<T>::capture_opening_predicate_results(&actor, &steps, Zero::zero());
      core::hint::black_box(&results);
    }
    assert_eq!(results.len() as u32, p);
    assert!(results.iter().all(|result| *result == Ok(expected)));
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn opening_mixed_predicate_capture(
    p: Linear<
      2,
      {
        T::MaxOpeningPredicateResults::get()
          .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()))
          .max(2)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let maximum = T::MaxOpeningPredicateResults::get()
      .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()));
    if maximum < 2 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent mixed Opening predicates",
      ));
    }
    let actor: T::AccountId = account("opening_predicate_actor", 0, 0);
    let native = T::FeeNativeAssetId::get();
    let non_native = T::BenchmarkHelper::setup_predicate_assets(&actor, 2)?
      .into_iter()
      .find(|asset| *asset != native)
      .expect("mixed capture has a non-native asset");
    let assets = if p == 2 {
      vec![non_native]
    } else {
      vec![native, non_native]
    };
    let mut predicates = Vec::with_capacity(p as usize);
    for asset in assets {
      let amount = T::AssetOps::minimum_balance(asset)
        .checked_add(&One::one())
        .expect("mixed capture funding fits");
      T::AssetOps::mint(&actor, asset, amount).expect("mixed capture asset is funded");
      predicates.push(Predicate::BalanceAbove {
        asset,
        threshold: Zero::zero(),
      });
    }
    let observation_count = p - predicates.len() as u32;
    let feeds = T::BenchmarkHelper::setup_max_encoded_observation_feeds(observation_count)?;
    assert_eq!(feeds.len() as u32, observation_count);
    assert_eq!(
      feeds
        .iter()
        .collect::<alloc::collections::BTreeSet<_>>()
        .len() as u32,
      observation_count
    );
    predicates.extend(feeds.into_iter().map(|feed| Predicate::ObservationAbove {
      feed,
      threshold: 0,
      max_age_blocks: 100,
    }));
    let steps = opening_predicate_capture_steps::<T>(predicates);
    let results;
    #[block]
    {
      results = Pallet::<T>::capture_opening_predicate_results(&actor, &steps, Zero::zero());
      core::hint::black_box(&results);
    }
    assert_eq!(results.len() as u32, p);
    assert!(results.iter().all(|result| *result == Ok(true)));
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn opening_asset_heavy_predicate_capture(
    p: Linear<
      2,
      {
        T::MaxOpeningPredicateResults::get()
          .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()))
          .max(2)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let maximum = T::MaxOpeningPredicateResults::get()
      .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()));
    if maximum < 2 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent mixed Opening predicates",
      ));
    }
    let actor: T::AccountId = account("opening_predicate_actor", 0, 0);
    let assets = T::BenchmarkHelper::setup_max_encoded_predicate_assets(&actor, (p - 1).max(2))?;
    let assets: Vec<_> = assets
      .into_iter()
      .filter(|asset| p != 2 || *asset != T::FeeNativeAssetId::get())
      .collect();
    assert_eq!(assets.len() as u32, p - 1);
    assert_eq!(
      assets
        .iter()
        .collect::<alloc::collections::BTreeSet<_>>()
        .len(),
      assets.len()
    );
    let mut predicates = Vec::with_capacity(p as usize);
    for asset in assets {
      let amount = T::AssetOps::minimum_balance(asset)
        .checked_add(&One::one())
        .expect("asset-heavy capture funding fits");
      T::AssetOps::mint(&actor, asset, amount).expect("asset-heavy capture asset is funded");
      predicates.push(Predicate::BalanceAbove {
        asset,
        threshold: Zero::zero(),
      });
    }
    let feeds = T::BenchmarkHelper::setup_max_encoded_observation_feeds(1)?;
    assert_eq!(feeds.len(), 1);
    predicates.push(Predicate::ObservationAbove {
      feed: feeds[0],
      threshold: 0,
      max_age_blocks: 100,
    });
    let steps = opening_predicate_capture_steps::<T>(predicates);
    let results;
    #[block]
    {
      results = Pallet::<T>::capture_opening_predicate_results(&actor, &steps, Zero::zero());
      core::hint::black_box(&results);
    }
    assert_eq!(results.len() as u32, p);
    assert!(results.iter().all(|result| *result == Ok(true)));
    Ok(())
  }

  fn prepare_max_encoded_opening_predicates<T: Config>(
    balances: u32,
    observations: u32,
    include_native: bool,
  ) -> Result<(T::AccountId, ContractSteps<T>), polkadot_sdk::frame_benchmarking::BenchmarkError>
  {
    let actor: T::AccountId = account("opening_predicate_actor", 0, 0);
    let native = T::FeeNativeAssetId::get();
    let mut assets = Vec::new();
    if balances != 0 {
      let requested =
        balances
          .checked_add(1)
          .ok_or(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
            "asset fixture count overflows",
          ))?;
      assets = T::BenchmarkHelper::setup_max_encoded_predicate_assets(&actor, requested)?
        .into_iter()
        .filter(|asset| *asset != native)
        .take(balances as usize)
        .collect();
      assert_eq!(assets.len() as u32, balances);
      assert_eq!(
        assets
          .iter()
          .collect::<alloc::collections::BTreeSet<_>>()
          .len(),
        assets.len()
      );
    }
    if include_native {
      assets.insert(0, native);
    }
    let mut predicates = Vec::new();
    for asset in assets {
      let amount = T::AssetOps::minimum_balance(asset)
        .checked_add(&One::one())
        .expect("maximum capture funding fits");
      T::AssetOps::mint(&actor, asset, amount).expect("maximum capture asset is funded");
      predicates.push(Predicate::BalanceAbove {
        asset,
        threshold: Zero::zero(),
      });
    }
    if observations != 0 {
      let feeds = T::BenchmarkHelper::setup_max_encoded_observation_feeds(observations)?;
      assert_eq!(feeds.len() as u32, observations);
      assert_eq!(
        feeds
          .iter()
          .collect::<alloc::collections::BTreeSet<_>>()
          .len(),
        feeds.len()
      );
      predicates.extend(feeds.into_iter().map(|feed| Predicate::ObservationAbove {
        feed,
        threshold: 0,
        max_age_blocks: 100,
      }));
    }
    Ok((actor, opening_predicate_capture_steps::<T>(predicates)))
  }

  fn prepare_full_opening_predicate_capture<T: Config>(
    balances: u32,
    observations: u32,
    include_native: bool,
  ) -> Result<(T::AccountId, ContractSteps<T>), polkadot_sdk::frame_benchmarking::BenchmarkError>
  {
    let per_step = benchmark_predicate_capacity::<T>();
    let clause_count = T::MaxPreconditionClauses::get().min(per_step) as usize;
    if clause_count == 0 || T::MaxContractSteps::get() == 0 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent predicate traversal",
      ));
    }
    let (actor, opening_steps) =
      prepare_max_encoded_opening_predicates::<T>(balances, observations, include_native)?;
    let mut predicates = opening_steps
      .iter()
      .filter_map(|step| step.precondition.as_ref())
      .flat_map(|precondition| precondition.clauses.iter().flat_map(|clause| clause.iter()))
      .cloned()
      .collect::<Vec<_>>();
    predicates.resize(
      T::MaxContractSteps::get().saturating_mul(per_step) as usize,
      TimedPredicate {
        timing: ObservationTiming::Current,
        predicate: Predicate::BlockNumberAbove { threshold: 0 },
      },
    );
    let mut steps = ContractSteps::<T>::default();
    for chunk in predicates.chunks(per_step as usize) {
      let mut clauses = vec![Vec::new(); clause_count];
      for (index, predicate) in chunk.iter().enumerate() {
        clauses[index % clause_count].push(*predicate);
      }
      let clauses = clauses
        .into_iter()
        .map(|clause| clause.try_into().expect("balanced predicate clause fits"))
        .collect::<Vec<_>>()
        .try_into()
        .expect("maximum predicate clause count fits");
      steps
        .try_push(Step {
          precondition: Some(Precondition { clauses }),
          task: ActorTask::StopCycle,
          on_error: StepErrorPolicy::AbortCycle,
        })
        .expect("full predicate traversal fits Contract");
    }
    assert_eq!(steps.len() as u32, T::MaxContractSteps::get());
    Ok((actor, steps))
  }

  fn prepare_max_encoded_step_precondition<T: Config>(
    balances: u32,
    observations: u32,
    include_native: bool,
  ) -> Result<(T::AccountId, PreconditionOf<T>), polkadot_sdk::frame_benchmarking::BenchmarkError>
  {
    let count = balances
      .saturating_add(observations)
      .saturating_add(u32::from(include_native));
    if count == 0 || count > benchmark_predicate_capacity::<T>() {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent Step predicate count",
      ));
    }
    let (actor, steps) =
      prepare_max_encoded_opening_predicates::<T>(balances, observations, include_native)?;
    assert_eq!(steps.len(), 1);
    let predicates = steps[0]
      .precondition
      .as_ref()
      .expect("prepared Step has predicates")
      .clauses
      .iter()
      .flat_map(|clause| clause.iter())
      .cloned()
      .map(|mut predicate| {
        predicate.timing = ObservationTiming::Current;
        predicate
      })
      .collect();
    let precondition = packed_predicate_clauses::<T>(
      predicates,
      count.div_ceil(T::MaxPreconditionClauses::get()).max(1),
    );
    Ok((actor, precondition))
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn predicate_host_allocation(
    b: Linear<0, { benchmark_predicate_capacity::<T>().max(1) }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let count = benchmark_predicate_capacity::<T>();
    if b > count {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent Step predicate allocation",
      ));
    }
    let (actor, precondition) = prepare_max_encoded_step_precondition::<T>(b, count - b, false)?;
    let result;
    #[block]
    {
      result = Pallet::<T>::evaluate_precondition(&precondition, &actor, Zero::zero());
      core::hint::black_box(&result);
    }
    assert_eq!(result, Ok(true));
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn predicate_native_host_allocation(
    b: Linear<0, { benchmark_predicate_capacity::<T>().saturating_sub(1).max(1) }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let count = benchmark_predicate_capacity::<T>();
    if b >= count {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent native Step predicate allocation",
      ));
    }
    let (actor, precondition) = prepare_max_encoded_step_precondition::<T>(b, count - 1 - b, true)?;
    let result;
    #[block]
    {
      result = Pallet::<T>::evaluate_precondition(&precondition, &actor, Zero::zero());
      core::hint::black_box(&result);
    }
    assert_eq!(result, Ok(true));
    Ok(())
  }

  #[benchmark(pov_mode = Measured)]
  fn predicate_asset_evaluation(
    p: Linear<1, { benchmark_predicate_capacity::<T>().max(1) }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor, precondition) = prepare_max_encoded_step_precondition::<T>(p, 0, false)?;
    let result;
    #[block]
    {
      result = Pallet::<T>::evaluate_precondition(&precondition, &actor, Zero::zero());
      core::hint::black_box(&result);
    }
    assert_eq!(result, Ok(true));
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn predicate_observation_evaluation(
    p: Linear<1, { benchmark_predicate_capacity::<T>().max(1) }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor, precondition) = prepare_max_encoded_step_precondition::<T>(0, p, false)?;
    let result;
    #[block]
    {
      result = Pallet::<T>::evaluate_precondition(&precondition, &actor, Zero::zero());
      core::hint::black_box(&result);
    }
    assert_eq!(result, Ok(true));
    Ok(())
  }

  #[benchmark(pov_mode = Measured)]
  fn predicate_observation_heavy_evaluation(
    o: Linear<1, { benchmark_predicate_capacity::<T>().saturating_sub(1).max(1) }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor, precondition) = prepare_max_encoded_step_precondition::<T>(1, o, false)?;
    let result;
    #[block]
    {
      result = Pallet::<T>::evaluate_precondition(&precondition, &actor, Zero::zero());
      core::hint::black_box(&result);
    }
    assert_eq!(result, Ok(true));
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn predicate_result_branch(
    r: Linear<0, 2>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let count = benchmark_predicate_capacity::<T>();
    if count < 2 || (r == 2 && count > T::MaxOpeningPredicateResults::get()) {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent predicate result branch",
      ));
    }
    let (actor, mut precondition) =
      prepare_max_encoded_step_precondition::<T>(count - 1, 1, false)?;
    for timed in precondition
      .clauses
      .iter_mut()
      .flat_map(|clause| clause.iter_mut())
    {
      if r == 1 {
        match &mut timed.predicate {
          Predicate::BalanceAbove { asset, threshold } => {
            *threshold = T::AssetOps::balance(&actor, *asset);
          }
          Predicate::ObservationAbove { threshold, .. } => *threshold = u128::MAX,
          _ => {
            return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
              "expected mixed comparison predicates",
            ));
          }
        }
      } else if r == 2 {
        timed.timing = ObservationTiming::Opening;
      }
    }
    let opening_results: OpeningPredicateResultsOf<T> = if r == 2 {
      vec![Err(PredicateError::InvalidObservation); T::MaxOpeningPredicateResults::get() as usize]
        .try_into()
        .expect("maximum Opening result vector fits")
    } else {
      Default::default()
    };
    let mut index = if r == 2 {
      opening_results.len() - count as usize
    } else {
      0
    };
    let result;
    #[block]
    {
      result = Pallet::<T>::evaluate_step_precondition(
        Some(&precondition),
        &actor,
        Zero::zero(),
        &opening_results,
        &mut index,
      );
      core::hint::black_box(&result);
    }
    let expected = match r {
      0 => Ok(true),
      1 => Ok(false),
      _ => Err(Error::<T>::InvalidPredicate.into()),
    };
    assert_eq!(result, expected);
    assert_eq!(index, opening_results.len());
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn predicate_mixed_read_state(
    s: Linear<0, 4>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let count = benchmark_predicate_capacity::<T>();
    if count < 2 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent mixed Step predicates",
      ));
    }
    let (actor, precondition) = prepare_max_encoded_step_precondition::<T>(count - 1, 0, false)?;
    let mut predicates: Vec<_> = precondition
      .clauses
      .iter()
      .flat_map(|clause| clause.iter())
      .cloned()
      .collect();
    let (feeds, fresh) = prepare_observation_read_state::<T>(1, s, 100)?;
    predicates.push(TimedPredicate {
      timing: ObservationTiming::Current,
      predicate: Predicate::ObservationAbove {
        feed: feeds[0],
        threshold: 0,
        max_age_blocks: 100,
      },
    });
    let precondition = packed_predicate_clauses::<T>(
      predicates,
      count.div_ceil(T::MaxPreconditionClauses::get()).max(1),
    );
    let expected = precondition.clauses.len() > 1 || fresh;
    let result;
    #[block]
    {
      result = Pallet::<T>::evaluate_precondition(&precondition, &actor, Zero::zero());
      core::hint::black_box(&result);
    }
    assert_eq!(result, Ok(expected));
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn predicate_cached_evaluation(
    o: Linear<0, { benchmark_predicate_capacity::<T>().max(1) }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let count = benchmark_predicate_capacity::<T>();
    if o > T::MaxOpeningPredicateResults::get() {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot retain requested Opening results",
      ));
    }
    let (actor, mut precondition) = prepare_max_encoded_step_precondition::<T>(count, 0, false)?;
    for timed in precondition
      .clauses
      .iter_mut()
      .flat_map(|clause| clause.iter_mut())
      .take(o as usize)
    {
      timed.timing = ObservationTiming::Opening;
    }
    let opening_results: OpeningPredicateResultsOf<T> =
      vec![Ok(true); T::MaxOpeningPredicateResults::get() as usize]
        .try_into()
        .expect("maximum Opening result vector fits");
    let mut index = opening_results.len() - o as usize;
    let result;
    #[block]
    {
      result = Pallet::<T>::evaluate_step_precondition(
        Some(&precondition),
        &actor,
        Zero::zero(),
        &opening_results,
        &mut index,
      );
      core::hint::black_box(&result);
    }
    assert_eq!(result, Ok(true));
    assert_eq!(index, opening_results.len());
    Ok(())
  }

  #[benchmark(pov_mode = Measured)]
  fn opening_max_encoded_balance_capture(
    p: Linear<
      0,
      {
        T::MaxOpeningPredicateResults::get()
          .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()))
          .max(1)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let maximum = T::MaxOpeningPredicateResults::get()
      .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()));
    if maximum == 0 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent Opening predicates",
      ));
    }
    let (actor, steps) = prepare_full_opening_predicate_capture::<T>(p, 0, false)?;
    let results;
    #[block]
    {
      results = Pallet::<T>::capture_opening_predicate_results(&actor, &steps, Zero::zero());
      core::hint::black_box(&results);
    }
    assert_eq!(results.len() as u32, p);
    assert!(results.iter().all(|result| *result == Ok(true)));
    Ok(())
  }

  #[benchmark(pov_mode = Measured)]
  fn opening_observation_heavy_capture(
    o: Linear<
      1,
      {
        T::MaxOpeningPredicateResults::get()
          .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()))
          .saturating_sub(1)
          .max(1)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let maximum = T::MaxOpeningPredicateResults::get()
      .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()));
    if maximum < 2 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent mixed Opening predicates",
      ));
    }
    let (actor, steps) = prepare_full_opening_predicate_capture::<T>(1, o, false)?;
    let results;
    #[block]
    {
      results = Pallet::<T>::capture_opening_predicate_results(&actor, &steps, Zero::zero());
      core::hint::black_box(&results);
    }
    assert_eq!(results.len() as u32, o + 1);
    assert!(results.iter().all(|result| *result == Ok(true)));
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn opening_predicate_composition(
    c: Linear<
      0,
      {
        T::MaxOpeningPredicateResults::get()
          .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()))
          .saturating_add(1)
          .saturating_pow(2)
          .saturating_sub(1)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let maximum = T::MaxOpeningPredicateResults::get()
      .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()));
    let total = (0..=maximum)
      .find(|total| c < total.saturating_add(1).saturating_pow(2))
      .ok_or(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "Opening composition index exceeds the host domain",
      ))?;
    let offset = c - total * total;
    let include_native = offset > total;
    let balances = if include_native {
      offset - total - 1
    } else {
      offset
    };
    let observations = total - balances - u32::from(include_native);
    let (actor, steps) =
      prepare_full_opening_predicate_capture::<T>(balances, observations, include_native)?;
    let results;
    #[block]
    {
      results = Pallet::<T>::capture_opening_predicate_results(&actor, &steps, Zero::zero());
      core::hint::black_box(&results);
    }
    assert_eq!(results.len() as u32, total);
    assert!(results.iter().all(|result| *result == Ok(true)));
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn opening_predicate_allocation(
    b: Linear<
      0,
      {
        T::MaxOpeningPredicateResults::get()
          .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()))
          .max(1)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let maximum = T::MaxOpeningPredicateResults::get()
      .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()));
    if maximum == 0 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent Opening predicates",
      ));
    }
    let (actor, steps) = prepare_max_encoded_opening_predicates::<T>(b, maximum - b, false)?;
    let results;
    #[block]
    {
      results = Pallet::<T>::capture_opening_predicate_results(&actor, &steps, Zero::zero());
      core::hint::black_box(&results);
    }
    assert_eq!(results.len() as u32, maximum);
    assert!(results.iter().all(|result| *result == Ok(true)));
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn opening_predicate_native_allocation(
    b: Linear<
      0,
      {
        T::MaxOpeningPredicateResults::get()
          .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()))
          .saturating_sub(1)
          .max(1)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let maximum = T::MaxOpeningPredicateResults::get()
      .min(T::MaxContractSteps::get().saturating_mul(benchmark_predicate_capacity::<T>()));
    if maximum < 2 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent mixed Opening predicates",
      ));
    }
    let (actor, steps) = prepare_max_encoded_opening_predicates::<T>(b, maximum - 1 - b, true)?;
    let results;
    #[block]
    {
      results = Pallet::<T>::capture_opening_predicate_results(&actor, &steps, Zero::zero());
      core::hint::black_box(&results);
    }
    assert_eq!(results.len() as u32, maximum);
    assert!(results.iter().all(|result| *result == Ok(true)));
    Ok(())
  }

  // This pure-Opening corner maximizes Contract length compatible with the measured fragment,
  // not the independent Opening/funding allocation frontier.
  fn prepare_reachable_running_tail<T: Config>(
    s: u32,
  ) -> Result<(ActorId, ActorStepTicketOf<T>), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let maximum = T::MaxContractSteps::get();
    let chunk = MAX_STEPS_PER_TAIL_CHUNK;
    if s == 0 || s > chunk || maximum <= s {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent the requested tail fragment",
      ));
    }
    let step_count = if s == chunk {
      maximum
    } else {
      1 + s + ((maximum - 1 - s) / chunk) * chunk
    };
    let cursor = if s == chunk { 1 } else { step_count - s };
    let actor_id = prepare_reachable_running::<T>(step_count)?;
    for expected_cursor in 1..cursor {
      let run = ActorRunStateStore::<T>::get(actor_id).expect("committed prefix retains Run");
      assert_eq!(run.cursor, expected_cursor);
      frame_system::Pallet::<T>::set_block_number(run.eligible_at);
      Pallet::<T>::execute_cycle(Weight::MAX);
    }
    let state = Pallet::<T>::active_actor_state(actor_id).expect("real Running authority exists");
    let run = state
      .run_state
      .as_ref()
      .expect("real Running payload exists");
    assert_eq!(state.hot.cycle_state, CycleState::Running);
    assert_eq!(run.cursor, cursor);
    assert_eq!(
      run.opening_snapshot.len(),
      Pallet::<T>::opening_surfaces(&state.contract.steps, 0).len()
    );
    assert_eq!(
      run.opening_predicate_results.len() as u32,
      state
        .contract
        .steps
        .iter()
        .map(|step| step
          .precondition
          .as_ref()
          .map_or(0, Precondition::opening_predicate_count))
        .sum::<u32>()
    );
    assert!(run.funding_snapshot.is_empty());
    frame_system::Pallet::<T>::set_block_number(run.eligible_at);
    let (location, cell) =
      Pallet::<T>::actor_control_cell(actor_id).expect("real Ready authority exists");
    let ActorControlLocation::Ready { ticket } = location else {
      panic!("Q1 continuation must have a real Ready ticket");
    };
    let context = Pallet::<T>::step_control_weight_context(step_count, cursor, 0, 0, 0, 0)
      .expect("tail context exists");
    assert_eq!(context.steps_in_fragment, s);
    let ticket = Pallet::<T>::build_actor_step_ticket(
      actor_id,
      ticket,
      run.eligible_at,
      &state.identity,
      &state.hot,
      Some(run),
      &cell.admission,
    )
    .expect("real Ready authority builds a ticket");
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real Running tail passes full premeasurement audit");
    Ok((actor_id, ticket))
  }

  fn assert_reachable_running_tail_unchanged<T: Config>(actor_id: ActorId, retained: &[u8]) {
    assert_eq!(
      ActorRunStateStore::<T>::get(actor_id)
        .expect("read-only tail load retains Run")
        .encode(),
      retained
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("read-only tail load preserves full state audit");
  }

  #[derive(Clone, Copy)]
  enum RunningInnerBranch {
    Complete,
    Progress,
  }

  fn prepare_reachable_running_inner<T: Config>(
    s: u32,
    p: u32,
    branch: RunningInnerBranch,
  ) -> Result<(ActorId, u32), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let maximum = T::MaxContractSteps::get();
    let chunk = MAX_STEPS_PER_TAIL_CHUNK;
    let minimum = if matches!(branch, RunningInnerBranch::Complete) {
      1
    } else {
      2
    };
    if s < minimum || s > chunk || maximum <= s || p > benchmark_predicate_capacity::<T>() {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent the requested Running inner branch",
      ));
    }
    let step_count = if matches!(branch, RunningInnerBranch::Progress) && s == chunk {
      maximum
    } else {
      1 + s + ((maximum - 1 - s) / chunk) * chunk
    };
    let cursor = if matches!(branch, RunningInnerBranch::Complete) {
      step_count - 1
    } else if s == chunk {
      1
    } else {
      step_count - s
    };
    let predicate_owner: T::AccountId = account("running-current-predicate-assets", 0, 0);
    let mut assets = T::BenchmarkHelper::setup_predicate_assets(&predicate_owner, p + 2)
      .expect("current predicate assets exist");
    assert_eq!(assets.len() as u32, p + 2);
    let zero_asset_index = assets
      .iter()
      .position(|asset| *asset != T::FeeNativeAssetId::get())
      .expect("host supplies a non-native zero-balance asset");
    let zero_asset = assets.remove(zero_asset_index);
    assets.retain(|asset| *asset != T::FeeNativeAssetId::get());
    assets.truncate(p as usize);
    assert_eq!(assets.len() as u32, p);
    let precondition = if p == 0 {
      None
    } else {
      Some(packed_predicate_clauses::<T>(
        assets
          .into_iter()
          .map(|asset| TimedPredicate {
            timing: ObservationTiming::Current,
            predicate: Predicate::BalanceAbove {
              asset,
              threshold: One::one(),
            },
          })
          .collect(),
        p.div_ceil(T::MaxPreconditionClauses::get()).max(1),
      ))
    };
    assert_eq!(
      precondition
        .as_ref()
        .map_or(0, Precondition::evaluation_units),
      p
    );
    let task = match branch {
      RunningInnerBranch::Complete => ActorTask::StopCycle,
      RunningInnerBranch::Progress => ActorTask::Transfer {
        to: account("running-zero-recipient", 0, 0),
        asset: zero_asset,
        amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
      },
    };
    let actor_id = prepare_reachable_running_with_cooldown::<T>(
      ActorType::User,
      step_count,
      Some((
        cursor,
        Step {
          precondition,
          task,
          on_error: StepErrorPolicy::AbortCycle,
        },
      )),
      0,
    )?;
    for expected_cursor in 1..cursor {
      let run = ActorRunStateStore::<T>::get(actor_id).expect("committed prefix retains Run");
      assert_eq!(run.cursor, expected_cursor);
      frame_system::Pallet::<T>::set_block_number(run.eligible_at);
      Pallet::<T>::execute_cycle(Weight::MAX);
    }
    let state = Pallet::<T>::active_actor_state(actor_id).expect("real Running state exists");
    assert!(T::AssetOps::balance(&state.identity.sovereign_account, zero_asset).is_zero());
    let run = state
      .run_state
      .as_ref()
      .expect("real Running payload exists");
    assert_eq!(state.hot.cycle_state, CycleState::Running);
    assert_eq!(run.cursor, cursor);
    let opening_before_cursor: u32 = state.contract.steps[..cursor as usize]
      .iter()
      .map(|step| {
        step
          .precondition
          .as_ref()
          .map_or(0, Precondition::opening_predicate_count)
      })
      .sum();
    assert_eq!(run.opening_predicate_cursor, opening_before_cursor);
    assert_eq!(
      run.opening_snapshot.len(),
      Pallet::<T>::opening_surfaces(&state.contract.steps, 0).len()
    );
    assert_eq!(
      run.opening_predicate_results.len() as u32,
      state
        .contract
        .steps
        .iter()
        .map(|step| step
          .precondition
          .as_ref()
          .map_or(0, Precondition::opening_predicate_count))
        .sum::<u32>()
    );
    assert!(run.funding_snapshot.is_empty());
    assert!(state.funding.funding_tracked_assets.is_empty());
    assert!(state.funding.funding_accumulated.is_empty());
    frame_system::Pallet::<T>::set_block_number(run.eligible_at);
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Ready { .. })
    ));
    let context = Pallet::<T>::step_control_weight_context(step_count, cursor, p, 0, 0, 0)
      .expect("real tail context exists");
    assert_eq!(context.steps_in_fragment, s);
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state()
      .expect("real Running inner source passes full audit before consumption");
    Ok((actor_id, cursor))
  }

  fn execute_reachable_step_inner<T: Config>(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    admission: ActorAdmissionCertificateOf<T>,
    loaded_step: LoadedActorStepOf<T>,
    now: BlockNumberFor<T>,
  ) -> Weight {
    let ticket = Pallet::<T>::build_actor_step_ticket(
      actor_id,
      state
        .hot
        .queue_ticket
        .expect("real consumed Ready ticket exists"),
      now,
      &state.identity,
      &state.hot,
      state.run_state.as_ref(),
      &admission,
    )
    .expect("real Running Step ticket builds");
    let maximum_fee = Pallet::<T>::maximum_current_action_fee(
      state.identity.actor_class.actor_type(),
      &loaded_step.step,
      loaded_step.resources,
    )
    .expect("current Action fee matches production");
    let plan = Pallet::<T>::build_current_step_plan(
      actor_id,
      state.identity.clone(),
      state.hot.clone(),
      state.run_state.clone(),
      state.funding.clone(),
      admission.clone(),
      ticket,
      loaded_step,
      maximum_fee,
    )
    .expect("real carried Running plan builds");
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      let result =
        Pallet::<T>::execute_current_step_and_place(actor_id, &state, plan, &admission, now);
      match result {
        Ok(evidence) => {
          let effect_weight = evidence.actual_effect_weight;
          core::hint::black_box(evidence);
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
            Weight,
            AttemptTransactionError,
          >(
            effect_weight
          ))
        }
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .expect("real carried Step inner atom commits")
  }

  fn assert_reachable_running_inner<T: Config>(
    actor_id: ActorId,
    cursor: u32,
    now: BlockNumberFor<T>,
    branch: RunningInnerBranch,
  ) {
    let state = Pallet::<T>::active_actor_state(actor_id).expect("real successor authority exists");
    assert_eq!(state.identity.actor_class.actor_type(), ActorType::User);
    let hold = ActorStateHolds::<T>::get(actor_id).expect("User state hold remains owned");
    assert_eq!(hold.owner, state.identity.owner);
    match branch {
      RunningInnerBranch::Complete => {
        assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
        assert_eq!(state.identity.cycle_nonce, 1);
        assert_eq!(state.hot.cycle_state, CycleState::Idle);
      }
      RunningInnerBranch::Progress => {
        let run = state
          .run_state
          .as_ref()
          .expect("real Running successor remains");
        assert_eq!(run.cursor, cursor + 1);
        assert_eq!(run.last_committed_step_block, Some(now));
        let expected_skip = if state.contract.steps[cursor as usize].precondition.is_none() {
          StepSkippedReason::ResolutionSkipped
        } else {
          StepSkippedReason::PreconditionFalse
        };
        assert_eq!(
          run.last_step_outcome,
          Some(StepOutcome::Skipped(expected_skip))
        );
        let ActorTask::Transfer { asset, ref to, .. } = state.contract.steps[cursor as usize].task
        else {
          panic!("Progress target retains its authored Transfer");
        };
        assert!(T::AssetOps::balance(&state.identity.sovereign_account, asset).is_zero());
        assert!(T::AssetOps::balance(to, asset).is_zero());
        assert_eq!(benchmark_fixture_ready_occupancy::<T>(), 1);
        assert!(state.hot.queue_ticket.is_some());
      }
    }
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real Running inner result preserves full state audit");
  }

  fn prepare_reachable_running<T: Config>(
    step_count: u32,
  ) -> Result<ActorId, polkadot_sdk::frame_benchmarking::BenchmarkError> {
    prepare_reachable_running_with_step::<T>(step_count, None)
  }

  fn make_reachable_opening_steps<T: Config>(
    step_count: u32,
  ) -> Result<ContractSteps<T>, polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let predicates_per_step = benchmark_predicate_capacity::<T>();
    if step_count == 0
      || step_count > T::MaxContractSteps::get()
      || step_count.saturating_mul(2) > T::MaxOpeningSnapshotEntries::get()
      || step_count.saturating_mul(predicates_per_step) > T::MaxOpeningPredicateResults::get()
      || predicates_per_step == 0
      || predicates_per_step != T::MaxPredicatesPerStep::get()
    {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent the maximum Opening payload with a Running successor",
      ));
    }
    let asset_owner: T::AccountId = account("reachable-state-assets", 0, 0);
    let assets_per_step = 2 + predicates_per_step;
    let assets =
      T::BenchmarkHelper::setup_predicate_assets(&asset_owner, step_count * assets_per_step)
        .expect("reachable Opening assets exist");
    assert_eq!(assets.len() as u32, step_count * assets_per_step);
    assert_eq!(
      assets
        .iter()
        .copied()
        .collect::<alloc::collections::BTreeSet<_>>()
        .len(),
      assets.len(),
      "Opening amounts and predicates use independent asset identities"
    );
    let mut steps = ContractSteps::<T>::default();
    for pair in assets.chunks_exact(assets_per_step as usize) {
      let predicates = (0..predicates_per_step)
        .map(|index| TimedPredicate {
          timing: ObservationTiming::Opening,
          predicate: Predicate::BalanceAbove {
            asset: pair[2 + index as usize],
            threshold: index.saturating_add(1).saturated_into(),
          },
        })
        .collect();
      steps
        .try_push(Step {
          precondition: Some(packed_predicate_clauses::<T>(
            predicates,
            T::MaxPredicatesPerClause::get(),
          )),
          task: ActorTask::AddLiquidity {
            asset_a: pair[0],
            asset_b: pair[1],
            amount_a: AmountResolution::PercentageAtOpening(Perbill::one()),
            amount_b: AmountResolution::PercentageAtOpening(Perbill::one()),
            min_lp_out: One::one(),
          },
          on_error: StepErrorPolicy::AbortCycle,
        })
        .expect("reachable maximum Contract fits");
    }
    Ok(steps)
  }

  #[derive(Clone, Copy)]
  enum ReachableOpeningProfile {
    UserPaged,
    UserCompleteMin,
    UserCompleteHeaderMax,
    Minimal,
    Predicated,
    RetryMin,
    RetryMax,
    CompleteMin,
    CompleteMax,
    FailedMin,
    FailedMax,
  }

  fn prepare_reachable_opening<T: Config>(
    tail_chunks: u32,
    profile: ReachableOpeningProfile,
  ) -> Result<(ActorId, u32), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let maximum = T::MaxContractSteps::get();
    let max_tails = maximum.saturating_sub(1).div_ceil(MAX_STEPS_PER_TAIL_CHUNK);
    let failed = matches!(
      profile,
      ReachableOpeningProfile::FailedMin | ReachableOpeningProfile::FailedMax
    );
    let retry = matches!(
      profile,
      ReachableOpeningProfile::RetryMin | ReachableOpeningProfile::RetryMax
    );
    let complete = matches!(
      profile,
      ReachableOpeningProfile::CompleteMin
        | ReachableOpeningProfile::CompleteMax
        | ReachableOpeningProfile::UserCompleteMin
        | ReachableOpeningProfile::UserCompleteHeaderMax
    );
    let terminal_predicates = matches!(
      profile,
      ReachableOpeningProfile::RetryMax
        | ReachableOpeningProfile::CompleteMax
        | ReachableOpeningProfile::FailedMax
    );
    if (tail_chunks == 0 && !retry && !complete && !failed)
      || tail_chunks > max_tails
      || (retry && T::MaxRetryAttempts::get() < 2)
      || (failed && T::MaxFundingTrackedAssets::get() == 0)
    {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent the Opening progress tail profile",
      ));
    }
    let count = (1 + tail_chunks * MAX_STEPS_PER_TAIL_CHUNK).min(maximum);
    let mut steps = make_reachable_opening_steps::<T>(count)?;
    let owner: T::AccountId = account("reachable-opening-owner", 0, 0);
    ensure_creation_balance::<T>(&owner);
    let missing_receipt = if failed {
      let (position, _) = T::BenchmarkHelper::setup_stake(&owner).map_err(|_| {
        polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
          "host cannot prepare an empty staking receipt",
        )
      })?;
      let receipt = T::StakingOps::share_asset(position).ok_or(
        polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
          "host did not create an admissible staking receipt",
        ),
      )?;
      // A host may alias position/receipt identity; neither may overlap any tail funding leg.
      for step in steps.iter().skip(1) {
        let ActorTask::AddLiquidity {
          asset_a, asset_b, ..
        } = step.task
        else {
          unreachable!()
        };
        assert!(
          asset_a != receipt && asset_b != receipt && asset_a != position && asset_b != position,
          "staking position and receipt are distinct from tail assets"
        );
      }
      Some((position, receipt))
    } else {
      None
    };
    if terminal_predicates {
      for step in steps.iter_mut() {
        let ActorTask::AddLiquidity {
          asset_a, asset_b, ..
        } = step.task
        else {
          unreachable!()
        };
        step.precondition = Some(packed_predicate_clauses::<T>(
          (0..benchmark_predicate_capacity::<T>())
            .map(|index| TimedPredicate {
              timing: ObservationTiming::Opening,
              predicate: Predicate::BalanceBelow {
                asset: if index % 2 == 0 { asset_a } else { asset_b },
                threshold: <T::Balance as polkadot_sdk::sp_runtime::traits::Bounded>::max_value()
                  .saturating_sub(index.saturated_into()),
              },
            })
            .collect(),
          T::MaxPredicatesPerClause::get(),
        ));
      }
    }
    let funding_count =
      if matches!(profile, ReachableOpeningProfile::Predicated) || terminal_predicates {
        0
      } else {
        (2 * (count - 1)).min(T::MaxFundingTrackedAssets::get().saturating_sub(u32::from(failed)))
      };
    if !matches!(profile, ReachableOpeningProfile::Predicated) {
      let ActorTask::AddLiquidity {
        asset_a, asset_b, ..
      } = steps[0].task
      else {
        unreachable!()
      };
      let zero_asset = if asset_a != T::FeeNativeAssetId::get() {
        asset_a
      } else {
        asset_b
      };
      assert_ne!(
        zero_asset,
        T::FeeNativeAssetId::get(),
        "zero resolution uses non-native custody"
      );
      steps[0] = Step {
        precondition: if terminal_predicates {
          steps[0].precondition.clone()
        } else {
          None
        },
        task: if let Some((position, _)) = missing_receipt {
          ActorTask::Unstake {
            asset: position,
            shares: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
          }
        } else if complete {
          ActorTask::StopCycle
        } else {
          ActorTask::Transfer {
            to: account("opening-zero-recipient", 0, 0),
            asset: zero_asset,
            amount: if retry {
              AmountResolution::Fixed(One::one())
            } else if matches!(profile, ReachableOpeningProfile::UserPaged) {
              AmountResolution::PercentageAtOpening(Perbill::from_percent(50))
            } else {
              AmountResolution::PercentageOfCurrent(Perbill::from_percent(50))
            },
          }
        },
        on_error: if retry {
          StepErrorPolicy::RetryLater {
            max_attempts: T::MaxRetryAttempts::get(),
          }
        } else {
          StepErrorPolicy::AbortCycle
        },
      };
      for (index, step) in steps.iter_mut().skip(1).enumerate() {
        if terminal_predicates {
          continue;
        }
        step.precondition = None;
        let ActorTask::AddLiquidity {
          amount_a, amount_b, ..
        } = &mut step.task
        else {
          unreachable!()
        };
        for (offset, amount) in [amount_a, amount_b].into_iter().enumerate() {
          *amount = if ((2 * index + offset) as u32) < funding_count {
            AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50))
          } else {
            AmountResolution::PercentageOfCurrent(Perbill::from_percent(50))
          };
        }
      }
    }
    let actor_type = if matches!(
      profile,
      ReachableOpeningProfile::UserPaged
        | ReachableOpeningProfile::UserCompleteMin
        | ReachableOpeningProfile::UserCompleteHeaderMax
    ) {
      ActorType::User
    } else {
      ActorType::System
    };
    retain_admitted_contract_geometry::<T>(actor_type, &mut steps, &[0])?;
    let mut contract = user_contract::<T>(
      Schedule {
        trigger: Trigger::Manual,
        cooldown_blocks: if retry { 2 } else { 0 },
      },
      steps,
    )
    .expect("Opening Contract exists");
    if matches!(profile, ReachableOpeningProfile::UserCompleteHeaderMax) {
      let maximum = T::MaxWhitelistSize::get();
      if maximum == 0 {
        return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
          "host cannot represent a nonempty maximum funding allowlist",
        ));
      }
      let mut sources = alloc::collections::BTreeSet::from([owner.clone()]);
      for index in 0..maximum.saturating_sub(1) {
        sources.insert(account("user-complete-funding-source", index, 0));
      }
      if sources.len() as u32 != maximum {
        return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
          "benchmark accounts do not fill the maximum funding allowlist",
        ));
      }
      contract.funding = FundingSourcePolicy::SignedAllowlist(
        sources
          .try_into()
          .expect("maximum funding allowlist obeys its configured bound"),
      );
    }
    let fee_reserve = full_attempt_fee::<T>(&contract.steps)
      .checked_add(
        &Pallet::<T>::trigger_fee_for_weight(
          ActorType::User,
          TriggerFamily::Manual,
          T::WeightInfo::manual_trigger(),
        )
        .trigger_fee,
      )
      .expect("real User Opening reserve fits");
    if actor_type == ActorType::User {
      prefund_active_user_creation::<T>(&owner, &contract.steps);
      Pallet::<T>::create_user_actor(
        RawOrigin::Signed(owner.clone()).into(),
        Mutability::Mutable,
        Some(contract),
      )
      .expect("real User Opening Contract is admitted");
    } else {
      Pallet::<T>::create_system_actor(
        RawOrigin::Root.into(),
        owner.clone(),
        Mutability::Mutable,
        Some(contract),
      )
      .expect("real System Opening Contract is admitted");
    }
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let identity = Pallet::<T>::actor_identity(actor_id).expect("real Opening identity exists");
    let funding_amount: T::Balance = 1_000_000_000_000u128.saturated_into();
    assert!(!funding_amount.is_zero());
    fund_reachable_assets_except::<T>(
      actor_id,
      funding_amount,
      missing_receipt.map(|(_, receipt)| receipt),
    );
    let funding = ActorFunding::<T>::get(actor_id).expect("real Opening funding exists");
    assert_eq!(
      funding.funding_tracked_assets.len() as u32,
      funding_count + u32::from(failed)
    );
    assert_eq!(funding.funding_accumulated.len() as u32, funding_count);
    if actor_type == ActorType::User {
      T::AssetOps::mint(
        &identity.sovereign_account,
        T::FeeNativeAssetId::get(),
        fee_reserve,
      )
      .expect("User Trigger and Pipeline have independent funding");
    }
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    GlobalCircuitBreaker::<T>::put(false);
    Pallet::<T>::manual_trigger(RawOrigin::Signed(owner.clone()).into(), actor_id)
      .expect("real Manual occurrence publishes Opening readiness");
    let state = Pallet::<T>::active_actor_state(actor_id).expect("real Idle authority exists");
    if let ActorTask::Transfer { asset, .. } = state.contract.steps[0].task {
      assert!(T::AssetOps::balance(&state.identity.sovereign_account, asset).is_zero());
    }
    assert_eq!(state.hot.cycle_state, CycleState::Idle);
    assert!(state.hot.pending_signal && state.run_state.is_none());
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Ready { .. })
    ));
    let (_, cell) = Pallet::<T>::actor_control_cell(actor_id).expect("Idle Ready cell exists");
    frame_system::Pallet::<T>::set_block_number(
      cell.eligible_at.expect("Opening eligibility exists"),
    );
    let surfaces = Pallet::<T>::opening_surfaces(&state.contract.steps, 0);
    assert!(surfaces.len() as u32 <= T::MaxOpeningSnapshotEntries::get());
    assert_eq!(state.contract.steps.len() as u32, count);
    let opening_results: u32 = state
      .contract
      .steps
      .iter()
      .map(|step| {
        step
          .precondition
          .as_ref()
          .map_or(0, Precondition::opening_predicate_count)
      })
      .sum();
    assert!(opening_results <= T::MaxOpeningPredicateResults::get());
    assert_eq!(
      count.saturating_sub(1).div_ceil(MAX_STEPS_PER_TAIL_CHUNK),
      tail_chunks
    );
    let predicate_units = state.contract.steps[0]
      .precondition
      .as_ref()
      .map_or(0, Precondition::evaluation_units);
    assert_eq!(
      predicate_units,
      if matches!(profile, ReachableOpeningProfile::Predicated) || terminal_predicates {
        2 * benchmark_predicate_capacity::<T>()
      } else {
        0
      }
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real Idle Opening source passes full audit");
    if let Some((position, receipt)) = missing_receipt {
      assert!(T::AssetOps::balance(&identity.sovereign_account, receipt).is_zero());
      T::BenchmarkHelper::remove_empty_staking_receipt(&owner, position).map_err(|_| {
        polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
          "host cannot remove the empty admitted staking receipt",
        )
      })?;
      assert!(T::StakingOps::share_asset(position).is_none());
      #[cfg(feature = "try-runtime")]
      Pallet::<T>::do_try_state()
        .expect("host receipt removal preserves Actor authority invariants");
    }
    Ok((actor_id, count))
  }

  fn assert_reachable_opening<T: Config>(
    actor_id: ActorId,
    count: u32,
    profile: ReachableOpeningProfile,
  ) {
    if matches!(
      profile,
      ReachableOpeningProfile::RetryMin
        | ReachableOpeningProfile::RetryMax
        | ReachableOpeningProfile::CompleteMin
        | ReachableOpeningProfile::CompleteMax
        | ReachableOpeningProfile::UserCompleteMin
        | ReachableOpeningProfile::UserCompleteHeaderMax
        | ReachableOpeningProfile::FailedMin
        | ReachableOpeningProfile::FailedMax
    ) {
      assert_reachable_terminal_opening::<T>(actor_id, count, profile);
      return;
    }
    let state =
      Pallet::<T>::active_actor_state(actor_id).expect("Opening publishes real Running state");
    assert_eq!(state.hot.cycle_state, CycleState::Running);
    let run = state.run_state.as_ref().expect("real Opening Run exists");
    assert_eq!(run.cursor, 1);
    assert_eq!(
      run.last_committed_step_block,
      Some(frame_system::Pallet::<T>::block_number())
    );
    let opening = Pallet::<T>::opening_surfaces(&state.contract.steps, 0).len() as u32;
    let results: u32 = state
      .contract
      .steps
      .iter()
      .map(|step| {
        step
          .precondition
          .as_ref()
          .map_or(0, Precondition::opening_predicate_count)
      })
      .sum();
    let funding = state.funding.funding_tracked_assets.len() as u32;
    let skip = match profile {
      ReachableOpeningProfile::Predicated => StepSkippedReason::PreconditionFalse,
      ReachableOpeningProfile::Minimal | ReachableOpeningProfile::UserPaged => {
        StepSkippedReason::ResolutionSkipped
      }
      _ => unreachable!("terminal Opening profiles are checked separately"),
    };
    assert_eq!(run.opening_snapshot.len() as u32, opening);
    assert_eq!(run.opening_predicate_results.len() as u32, results);
    assert_eq!(run.funding_snapshot.len() as u32, funding);
    let surfaces = Pallet::<T>::opening_surfaces(&state.contract.steps, 0);
    assert!(
      surfaces
        .iter()
        .all(|surface| run.opening_snapshot.contains_key(surface))
    );
    assert!(
      run
        .opening_predicate_results
        .iter()
        .all(|result| *result == Ok(false))
    );
    assert!(
      run
        .funding_snapshot
        .values()
        .all(|amount| *amount == 1_000_000_000_000u128.saturated_into())
    );
    assert!(state.funding.funding_accumulated.is_empty());
    assert!(
      run
        .funding_snapshot
        .keys()
        .all(|asset| state.funding.funding_tracked_assets.contains(asset))
    );
    assert_eq!(run.last_step_outcome, Some(StepOutcome::Skipped(skip)));
    assert_eq!(benchmark_fixture_ready_occupancy::<T>(), 1);
    assert!(state.hot.queue_ticket.is_some());
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real Opening successor passes full audit");
  }

  fn assert_reachable_terminal_opening<T: Config>(
    actor_id: ActorId,
    _count: u32,
    profile: ReachableOpeningProfile,
  ) {
    let state = Pallet::<T>::active_actor_state(actor_id)
      .expect("terminal/retry Opening retains Actor authority");
    let failed = matches!(
      profile,
      ReachableOpeningProfile::FailedMin | ReachableOpeningProfile::FailedMax
    );
    let funding = state
      .funding
      .funding_tracked_assets
      .len()
      .saturating_sub(usize::from(failed)) as u32;
    assert_eq!(
      state.funding.funding_tracked_assets.len() as u32,
      funding + u32::from(failed)
    );
    assert!(state.funding.funding_accumulated.is_empty());
    assert!(!state.hot.pending_signal);
    assert_eq!(benchmark_fixture_ready_occupancy::<T>(), 0);
    let now = frame_system::Pallet::<T>::block_number();
    if failed {
      assert_eq!(state.hot.cycle_state, CycleState::Idle);
      assert_eq!(state.identity.cycle_nonce, 1);
      assert_eq!(state.hot.unsuccessful_attempt_streak, 1);
      assert!(state.run_state.is_none());
      assert!(state.hot.queue_ticket.is_none() && state.hot.wakeup_pointer.is_none());
      assert!(matches!(
        ActorControlLocators::<T>::get(actor_id),
        Some(ActorControlLocation::Unsignaled)
      ));
      let failure: <T as frame_system::Config>::RuntimeEvent = Event::<T>::StepFailed {
        actor_id,
        cycle_nonce: 1,
        step_index: 0,
        retry_class: RetryClass::Permanent,
        error: Error::<T>::InvalidAmountResolution.into(),
      }
      .into();
      assert!(
        frame_system::Pallet::<T>::events()
          .iter()
          .any(|record| record.event == failure)
      );
      let ActorTask::Unstake { asset, .. } = state.contract.steps[0].task else {
        unreachable!()
      };
      assert!(T::StakingOps::share_asset(asset).is_none());
      assert!(T::StakingOps::share_balance(&state.identity.sovereign_account, asset).is_zero());
    } else if matches!(
      profile,
      ReachableOpeningProfile::CompleteMin
        | ReachableOpeningProfile::CompleteMax
        | ReachableOpeningProfile::UserCompleteMin
        | ReachableOpeningProfile::UserCompleteHeaderMax
    ) {
      assert_eq!(state.hot.cycle_state, CycleState::Idle);
      assert_eq!(state.identity.cycle_nonce, 1);
      assert_eq!(state.hot.unsuccessful_attempt_streak, 0);
      let stopped: <T as frame_system::Config>::RuntimeEvent = Event::<T>::CycleStopped {
        actor_id,
        cycle_nonce: 1,
        step_index: 0,
      }
      .into();
      assert!(
        frame_system::Pallet::<T>::events()
          .iter()
          .any(|record| record.event == stopped)
      );
      let receipt: <T as frame_system::Config>::RuntimeEvent = Event::<T>::ActionFeeCharged {
        actor_id,
        cycle_nonce: 1,
        step_index: 0,
        actual_effect_weight: T::TaskEffectWeight::actual_effect_weight(
          &ActorTask::StopCycle,
          TaskEffectExecution::Invoked,
        )
        .expect("StopCycle has host effect evidence"),
        fee: Zero::zero(),
      }
      .into();
      assert_eq!(
        frame_system::Pallet::<T>::events()
          .iter()
          .filter(|record| record.event == receipt)
          .count(),
        usize::from(matches!(profile, ReachableOpeningProfile::CompleteMax)),
        "only the predicated completion profile includes invocation-receipt deposition"
      );
      assert!(state.run_state.is_none());
      assert!(state.hot.queue_ticket.is_none() && state.hot.wakeup_pointer.is_none());
      assert!(matches!(
        ActorControlLocators::<T>::get(actor_id),
        Some(ActorControlLocation::Unsignaled)
      ));
    } else {
      assert_eq!(state.hot.cycle_state, CycleState::Suspended);
      let run = state.run_state.as_ref().expect("real retry Run persists");
      assert_eq!(run.cursor, 0);
      assert_eq!(run.unsuccessful_attempts_at_cursor, 1);
      assert_eq!(run.last_attempt_block, now);
      assert_eq!(run.last_step_outcome, Some(StepOutcome::FundingUnavailable));
      assert_eq!(run.suspension, Some(SuspensionReason::FundingUnavailable));
      let due = Pallet::<T>::suspension_eligible_at(2, None, now, 1)
        .expect("retry eligibility is representable");
      assert_eq!(run.eligible_at, due);
      assert!(
        matches!(ActorControlLocators::<T>::get(actor_id), Some(ActorControlLocation::Waiting { key: WakeupKey::Block(at), .. }) if at == due)
      );
      assert_eq!(
        run.opening_snapshot.len(),
        Pallet::<T>::opening_surfaces(&state.contract.steps, 0).len()
      );
      assert_eq!(
        run.opening_predicate_results.len() as u32,
        state
          .contract
          .steps
          .iter()
          .map(|step| step
            .precondition
            .as_ref()
            .map_or(0, Precondition::opening_predicate_count))
          .sum::<u32>()
      );
      assert!(
        run
          .opening_predicate_results
          .iter()
          .all(|result| *result == Ok(true))
      );
      assert_eq!(run.funding_snapshot.len() as u32, funding);
      assert!(
        run
          .funding_snapshot
          .keys()
          .all(|asset| state.funding.funding_tracked_assets.contains(asset))
      );
      assert!(
        run
          .funding_snapshot
          .values()
          .all(|amount| *amount == 1_000_000_000_000u128.saturated_into())
      );
      let ActorTask::Transfer { asset, ref to, .. } = state.contract.steps[0].task else {
        unreachable!()
      };
      assert!(T::AssetOps::balance(&state.identity.sovereign_account, asset).is_zero());
      assert!(T::AssetOps::balance(to, asset).is_zero());
    }
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real retry/completion preserves full state audit");
  }

  struct ReachableSuspendedSkip<T: Config> {
    actor_id: ActorId,
    cursor: u32,
    ready_others: u32,
    owner: T::AccountId,
    frozen_asset: T::AssetId,
    balances: [(T::AssetId, T::Balance); 2],
    fixture_funding: [(T::AssetId, T::Balance, T::Balance); 2],
  }

  // A terminal head has no tail. For the other branches, `n` allocates the tail's
  // two-leg amount sources between Opening and LastFunding; it is not a free Run axis.
  fn prepare_reachable_suspended_head_inner<T: Config>(
    n: u32,
    current: u32,
    opening: u32,
    branch: Option<RunningInnerBranch>,
  ) -> Result<
    (ActorId, Option<ReachableSuspendedSkip<T>>),
    polkadot_sdk::frame_benchmarking::BenchmarkError,
  > {
    let complete = matches!(branch, Some(RunningInnerBranch::Complete));
    let count = if complete {
      1
    } else {
      T::MaxContractSteps::get()
    };
    let capacity = benchmark_predicate_capacity::<T>();
    let tail_legs = count.saturating_sub(1) * 2;
    if count == 0
      || (!complete && count < 2)
      || n > tail_legs
      || tail_legs - n > T::MaxFundingTrackedAssets::get()
      || current + opening > capacity
      || T::MaxRetryAttempts::get() < 3
    {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent the coupled Suspended-head profile",
      ));
    }
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    GlobalCircuitBreaker::<T>::put(false);
    let owner: T::AccountId = account("suspended-head-owner", 0, 0);
    ensure_creation_balance::<T>(&owner);
    let liquidity = if branch.is_some() {
      Some(
        T::BenchmarkHelper::setup_add_liquidity(&owner).map_err(|_| {
          polkadot_sdk::frame_benchmarking::BenchmarkError::Stop("host liquidity setup unsupported")
        })?,
      )
    } else {
      None
    };
    let mut steps = make_reachable_opening_steps::<T>(count)?;
    for (index, step) in steps.iter_mut().skip(1).enumerate() {
      let ActorTask::AddLiquidity {
        amount_a, amount_b, ..
      } = &mut step.task
      else {
        unreachable!()
      };
      for (offset, amount) in [amount_a, amount_b].into_iter().enumerate() {
        if (index * 2 + offset) as u32 >= n {
          *amount = AmountResolution::PercentageOfLastFunding(Perbill::one());
        }
      }
    }
    let auxiliary_start = count * (2 + capacity);
    let assets = T::BenchmarkHelper::setup_predicate_assets(&owner, auxiliary_start + capacity + 4)
      .map_err(|_| {
        polkadot_sdk::frame_benchmarking::BenchmarkError::Stop("host predicate setup unsupported")
      })?;
    let auxiliary = assets
      .into_iter()
      .skip(auxiliary_start as usize)
      .filter(|asset| {
        *asset != T::FeeNativeAssetId::get()
          && liquidity.is_none_or(|(a, b, _, _)| *asset != a && *asset != b)
      })
      .take((1 + current + opening) as usize)
      .collect::<Vec<_>>();
    assert_eq!(auxiliary.len() as u32, 1 + current + opening);
    steps[0] = Step {
      precondition: (current + opening > 0).then(|| {
        packed_predicate_clauses::<T>(
          auxiliary
            .iter()
            .skip(1)
            .enumerate()
            .map(|(index, asset)| TimedPredicate {
              timing: if (index as u32) < opening {
                ObservationTiming::Opening
              } else {
                ObservationTiming::Current
              },
              predicate: Predicate::BalanceBelow {
                asset: *asset,
                threshold: One::one(),
              },
            })
            .collect(),
          T::MaxPredicatesPerClause::get(),
        )
      }),
      task: if let Some((asset_a, asset_b, _, _)) = liquidity {
        ActorTask::AddLiquidity {
          asset_a,
          asset_b,
          amount_a: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
          amount_b: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
          min_lp_out: <T::Balance as polkadot_sdk::sp_runtime::traits::Bounded>::max_value(),
        }
      } else {
        ActorTask::Transfer {
          to: account("suspended-head-recipient", 0, 0),
          asset: auxiliary[0],
          amount: AmountResolution::Fixed(One::one()),
        }
      },
      on_error: StepErrorPolicy::RetryLater {
        max_attempts: T::MaxRetryAttempts::get(),
      },
    };
    retain_admitted_contract_geometry::<T>(ActorType::System, &mut steps, &[0])?;
    let mut contract = system_contract::<T>(
      Schedule {
        trigger: Trigger::Manual,
        cooldown_blocks: 2,
      },
      steps,
    )
    .expect("active head Contract exists");
    contract.funding = FundingSourcePolicy::OwnerOnly;
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner.clone(),
      Mutability::Mutable,
      Some(contract),
    )
    .expect("coupled head Contract is admitted");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let actor = Pallet::<T>::actor_identity(actor_id)
      .expect("head identity exists")
      .sovereign_account;
    fund_reachable_update_assets::<T>(actor_id, 10u32.into());
    assert!(
      auxiliary
        .iter()
        .all(|asset| T::AssetOps::balance(&actor, *asset).is_zero())
    );
    let mut skip = if let Some((asset_a, asset_b, amount_a, amount_b)) = liquidity {
      let frozen_asset = [asset_a, asset_b]
        .into_iter()
        .find(|asset| *asset != T::FeeNativeAssetId::get())
        .expect("liquidity has a non-native leg");
      let fixture_funding = [
        (asset_a, T::AssetOps::balance(&actor, asset_a), amount_a),
        (asset_b, T::AssetOps::balance(&actor, asset_b), amount_b),
      ];
      T::AssetOps::mint(&actor, asset_a, amount_a).expect("head first liquidity leg funded");
      T::AssetOps::mint(&actor, asset_b, amount_b).expect("head second liquidity leg funded");
      T::BenchmarkHelper::set_asset_account_frozen(&owner, &actor, frozen_asset, false).map_err(
        |_| polkadot_sdk::frame_benchmarking::BenchmarkError::Stop("host account thaw unsupported"),
      )?;
      Some(ReachableSuspendedSkip {
        actor_id,
        cursor: 0,
        ready_others: 0,
        owner: owner.clone(),
        frozen_asset,
        balances: [
          (asset_a, T::AssetOps::balance(&actor, asset_a)),
          (asset_b, T::AssetOps::balance(&actor, asset_b)),
        ],
        fixture_funding,
      })
    } else {
      None
    };
    Pallet::<T>::manual_trigger(RawOrigin::Signed(owner).into(), actor_id)
      .expect("real head occurrence admits Opening");
    Pallet::<T>::execute_cycle(Weight::MAX);
    let state = Pallet::<T>::active_actor_state(actor_id).expect("Opening retains Actor");
    let run = state.run_state.expect("real failure retains Run");
    assert_eq!(state.hot.cycle_state, CycleState::Suspended);
    assert_eq!(run.cursor, 0);
    assert_eq!(run.unsuccessful_attempts_at_cursor, 1);
    assert_eq!(run.last_committed_step_block, None);
    assert_eq!(
      run.opening_snapshot.len(),
      Pallet::<T>::opening_surfaces(&state.contract.steps, 0).len()
    );
    assert_eq!(
      run.funding_snapshot.len(),
      state.funding.funding_tracked_assets.len()
    );
    assert_eq!(
      run.opening_predicate_results.len() as u32,
      state
        .contract
        .steps
        .iter()
        .map(|step| step
          .precondition
          .as_ref()
          .map_or(0, Precondition::opening_predicate_count))
        .sum::<u32>()
    );
    assert!(
      run
        .opening_predicate_results
        .iter()
        .take(opening as usize)
        .all(|result| *result == Ok(true))
    );
    assert!(state.funding.funding_accumulated.is_empty());
    if let Some(fixture) = &skip {
      assert_eq!(run.suspension, Some(SuspensionReason::Temporary));
      T::BenchmarkHelper::set_asset_account_frozen(
        &fixture.owner,
        &actor,
        fixture.frozen_asset,
        true,
      )
      .map_err(|_| {
        polkadot_sdk::frame_benchmarking::BenchmarkError::Stop("host account freeze unsupported")
      })?;
      assert!(T::AssetOps::balance(&actor, fixture.frozen_asset).is_zero());
    } else {
      assert_eq!(run.last_step_outcome, Some(StepOutcome::FundingUnavailable));
    }
    frame_system::Pallet::<T>::set_block_number(run.eligible_at);
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    Pallet::<T>::drain_overdue_wakeups_cursor(run.eligible_at, &mut meter);
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Ready { .. })
    ));
    if let Some(fixture) = &mut skip {
      // Host asset bootstrap may create other due Actors. Preserve their real topology.
      fixture.ready_others = benchmark_fixture_ready_occupancy::<T>()
        .checked_sub(1)
        .expect("target owns one live Ready slot");
    }
    assert_eq!(
      ActorRunStateStore::<T>::get(actor_id)
        .expect("due Run persists")
        .encode(),
      run.encode()
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real Suspended-head source passes full audit");
    Ok((actor_id, skip))
  }

  fn assert_reachable_suspended_head_retry<T: Config>(
    actor_id: ActorId,
    before: ActorRunStateOf<T>,
  ) {
    let after = ActorRunStateStore::<T>::get(actor_id).expect("retry retains Run");
    assert_eq!(after.cursor, 0);
    assert_eq!(after.unsuccessful_attempts_at_cursor, 2);
    assert_eq!(after.last_attempt_block, before.eligible_at);
    assert_eq!(after.last_committed_step_block, None);
    assert_eq!(
      after.last_step_outcome,
      Some(StepOutcome::FundingUnavailable)
    );
    assert_eq!(after.opening_snapshot, before.opening_snapshot);
    assert_eq!(
      after.opening_predicate_results,
      before.opening_predicate_results
    );
    assert_eq!(after.funding_snapshot, before.funding_snapshot);
    assert!(
      matches!(ActorControlLocators::<T>::get(actor_id), Some(ActorControlLocation::Waiting { key: WakeupKey::Block(at), .. }) if at == after.eligible_at)
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("head retry successor passes full audit");
  }

  #[cfg(test)]
  fn prepare_reachable_suspended_tail_skip<T: Config>(
    s: u32,
    p: u32,
    branch: RunningInnerBranch,
  ) -> Result<ReachableSuspendedSkip<T>, polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let maximum = T::MaxContractSteps::get();
    let chunk = MAX_STEPS_PER_TAIL_CHUNK;
    let complete = matches!(branch, RunningInnerBranch::Complete);
    if s < if complete { 1 } else { 2 }
      || s > chunk
      || maximum <= s
      || p > benchmark_predicate_capacity::<T>()
      || T::MaxRetryAttempts::get() < 2
    {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent the Suspended skip tail",
      ));
    }
    let count = if !complete && s == chunk {
      maximum
    } else {
      1 + s + ((maximum - 1 - s) / chunk) * chunk
    };
    let cursor = if complete {
      count - 1
    } else if s == chunk {
      1
    } else {
      count - s
    };
    let owner: T::AccountId = account("suspended-skip-assets", 0, 0);
    let (asset_a, asset_b, amount_a, amount_b) = T::BenchmarkHelper::setup_add_liquidity(&owner)
      .map_err(|_| {
        polkadot_sdk::frame_benchmarking::BenchmarkError::Stop("host liquidity setup unsupported")
      })?;
    let frozen_asset = [asset_a, asset_b]
      .into_iter()
      .find(|asset| *asset != T::FeeNativeAssetId::get())
      .ok_or(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host needs a freezable non-native liquidity leg",
      ))?;
    let assets = T::BenchmarkHelper::setup_predicate_assets(&owner, p + 3).map_err(|_| {
      polkadot_sdk::frame_benchmarking::BenchmarkError::Stop("host predicate setup unsupported")
    })?;
    let assets = assets
      .into_iter()
      .filter(|asset| {
        *asset != asset_a && *asset != asset_b && *asset != T::FeeNativeAssetId::get()
      })
      .take(p as usize)
      .collect::<alloc::vec::Vec<_>>();
    if assets.len() != p as usize {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host needs distinct unfunded predicate assets",
      ));
    }
    let precondition = if p == 0 {
      None
    } else {
      Some(packed_predicate_clauses::<T>(
        assets
          .iter()
          .map(|asset| TimedPredicate {
            timing: ObservationTiming::Current,
            predicate: Predicate::BalanceBelow {
              asset: *asset,
              threshold: One::one(),
            },
          })
          .collect(),
        p.div_ceil(T::MaxPreconditionClauses::get()).max(1),
      ))
    };
    let actor_id = prepare_reachable_running_with_cooldown::<T>(
      ActorType::System,
      count,
      Some((
        cursor,
        Step {
          precondition,
          task: ActorTask::AddLiquidity {
            asset_a,
            asset_b,
            amount_a: AmountResolution::PercentageOfCurrent(Perbill::from_parts(1)),
            amount_b: AmountResolution::PercentageOfCurrent(Perbill::from_parts(1)),
            min_lp_out: <T::Balance as polkadot_sdk::sp_runtime::traits::Bounded>::max_value(),
          },
          on_error: StepErrorPolicy::RetryLater {
            max_attempts: T::MaxRetryAttempts::get(),
          },
        },
      )),
      2,
    )?;
    let sovereign = Pallet::<T>::actor_identity(actor_id)
      .expect("admitted identity exists")
      .sovereign_account;
    // Funding occurs after Opening, so previously captured false predicates stay authoritative.
    let fixture_funding = [
      (asset_a, T::AssetOps::balance(&sovereign, asset_a), amount_a),
      (asset_b, T::AssetOps::balance(&sovereign, asset_b), amount_b),
    ];
    T::AssetOps::mint(&sovereign, asset_a, amount_a).expect("first real liquidity leg funded");
    T::AssetOps::mint(&sovereign, asset_b, amount_b).expect("second real liquidity leg funded");
    T::BenchmarkHelper::set_asset_account_frozen(&owner, &sovereign, frozen_asset, false).map_err(
      |_| {
        polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
          "host liquidity account thaw unsupported",
        )
      },
    )?;
    let balances = [
      (asset_a, T::AssetOps::balance(&sovereign, asset_a)),
      (asset_b, T::AssetOps::balance(&sovereign, asset_b)),
    ];
    assert!(balances.iter().all(|(_, balance)| *balance > One::one()));
    assert!(
      assets
        .iter()
        .all(|asset| T::AssetOps::balance(&sovereign, *asset).is_zero())
    );
    for expected_cursor in 1..cursor {
      let run = ActorRunStateStore::<T>::get(actor_id).expect("real prefix Run exists");
      assert_eq!(run.cursor, expected_cursor);
      frame_system::Pallet::<T>::set_block_number(run.eligible_at);
      Pallet::<T>::execute_cycle(Weight::MAX);
    }
    let run = ActorRunStateStore::<T>::get(actor_id).expect("real target Run exists");
    assert_eq!(run.cursor, cursor);
    frame_system::Pallet::<T>::set_block_number(run.eligible_at);
    Pallet::<T>::execute_cycle(Weight::MAX);
    let state =
      Pallet::<T>::active_actor_state(actor_id).expect("real Temporary retry remains active");
    let run = state
      .run_state
      .as_ref()
      .expect("Temporary retry retains Run");
    assert_eq!(state.hot.cycle_state, CycleState::Suspended);
    assert_eq!(run.cursor, cursor);
    assert_eq!(run.unsuccessful_attempts_at_cursor, 1);
    assert_eq!(run.suspension, Some(SuspensionReason::Temporary));
    assert!(matches!(
      run.last_step_outcome,
      Some(StepOutcome::Failed(TaskFailure {
        retry: RetryClass::Temporary,
        ..
      }))
    ));
    assert!(
      matches!(ActorControlLocators::<T>::get(actor_id), Some(ActorControlLocation::Waiting { key: WakeupKey::Block(at), .. }) if at == run.eligible_at)
    );
    assert_eq!(
      run.opening_snapshot.len(),
      Pallet::<T>::opening_surfaces(&state.contract.steps, 0).len()
    );
    assert_eq!(
      run.opening_predicate_results.len() as u32,
      state
        .contract
        .steps
        .iter()
        .map(|step| step
          .precondition
          .as_ref()
          .map_or(0, Precondition::opening_predicate_count))
        .sum::<u32>()
    );
    assert_eq!(
      run.opening_predicate_cursor,
      state.contract.steps[..cursor as usize]
        .iter()
        .map(|step| step
          .precondition
          .as_ref()
          .map_or(0, Precondition::opening_predicate_count))
        .sum::<u32>()
    );
    assert!(
      run
        .opening_predicate_results
        .iter()
        .all(|result| *result == Ok(false))
    );
    assert!(
      run.funding_snapshot.is_empty()
        && state.funding.funding_tracked_assets.is_empty()
        && state.funding.funding_accumulated.is_empty()
    );
    for (asset, balance) in balances {
      assert_eq!(T::AssetOps::balance(&sovereign, asset), balance);
    }
    let context = Pallet::<T>::step_control_weight_context(count, cursor, p, 0, 0, 0)
      .expect("tail context exists");
    assert_eq!(context.steps_in_fragment, s);
    assert_eq!(context.predicate_evaluation_units, p);
    let retained = run.encode();
    T::BenchmarkHelper::set_asset_account_frozen(&owner, &sovereign, frozen_asset, true).map_err(
      |_| {
        polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
          "host liquidity account freeze unsupported",
        )
      },
    )?;
    assert!(T::AssetOps::balance(&sovereign, frozen_asset).is_zero());
    frame_system::Pallet::<T>::set_block_number(run.eligible_at);
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    assert_eq!(
      Pallet::<T>::drain_overdue_wakeups_cursor(run.eligible_at, &mut meter).ready_entries,
      1
    );
    assert_eq!(
      ActorRunStateStore::<T>::get(actor_id)
        .expect("due Run retained")
        .encode(),
      retained
    );
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Ready { .. })
    ));
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real frozen due source passes full audit");
    Ok(ReachableSuspendedSkip {
      actor_id,
      cursor,
      ready_others: benchmark_fixture_ready_occupancy::<T>()
        .checked_sub(1)
        .expect("target owns one live Ready slot"),
      owner,
      frozen_asset,
      balances,
      fixture_funding,
    })
  }

  struct ReachableSuspendedSuccess<T: Config> {
    actor_id: ActorId,
    cursor: u32,
    ready_others: u32,
    asset: T::AssetId,
    amount: T::Balance,
    recipient: T::AccountId,
  }

  fn prepare_reachable_suspended_tail_success<T: Config>(
    s: u32,
    p: u32,
    branch: RunningInnerBranch,
  ) -> Result<ReachableSuspendedSuccess<T>, polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let maximum = T::MaxContractSteps::get();
    let chunk = MAX_STEPS_PER_TAIL_CHUNK;
    let complete = matches!(branch, RunningInnerBranch::Complete);
    if s < if complete { 1 } else { 2 }
      || s > chunk
      || maximum <= s
      || p > benchmark_predicate_capacity::<T>()
      || T::MaxRetryAttempts::get() < 2
    {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent the successful Suspended tail",
      ));
    }
    let count = if !complete && s == chunk {
      maximum
    } else {
      1 + s + ((maximum - 1 - s) / chunk) * chunk
    };
    let cursor = if complete {
      count - 1
    } else if s == chunk {
      1
    } else {
      count - s
    };
    let owner: T::AccountId = account("suspended-success-assets", 0, 0);
    let mut assets = T::BenchmarkHelper::setup_predicate_assets(&owner, p + 2).map_err(|_| {
      polkadot_sdk::frame_benchmarking::BenchmarkError::Stop("host predicate setup unsupported")
    })?;
    let position = assets
      .iter()
      .position(|asset| *asset != T::FeeNativeAssetId::get())
      .ok_or(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "successful Suspended tail needs a non-native asset",
      ))?;
    let asset = assets.remove(position);
    assets.retain(|candidate| *candidate != asset);
    assets.truncate(p as usize);
    if assets.len() != p as usize {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host needs distinct current-predicate assets",
      ));
    }
    let precondition = (p > 0).then(|| {
      packed_predicate_clauses::<T>(
        assets
          .into_iter()
          .map(|candidate| TimedPredicate {
            timing: ObservationTiming::Current,
            predicate: Predicate::BalanceBelow {
              asset: candidate,
              threshold: One::one(),
            },
          })
          .collect(),
        p.div_ceil(T::MaxPreconditionClauses::get()).max(1),
      )
    });
    let recipient: T::AccountId = account("suspended-success-recipient", 0, 0);
    let amount = T::AssetOps::minimum_balance(asset).max(One::one());
    let actor_id = prepare_reachable_running_with_cooldown::<T>(
      ActorType::System,
      count,
      Some((
        cursor,
        Step {
          precondition,
          task: ActorTask::Transfer {
            to: recipient.clone(),
            asset,
            amount: AmountResolution::Fixed(amount),
          },
          on_error: StepErrorPolicy::RetryLater {
            max_attempts: T::MaxRetryAttempts::get(),
          },
        },
      )),
      2,
    )?;
    for expected_cursor in 1..cursor {
      let run = ActorRunStateStore::<T>::get(actor_id).expect("real prefix Run exists");
      assert_eq!(run.cursor, expected_cursor);
      frame_system::Pallet::<T>::set_block_number(run.eligible_at);
      Pallet::<T>::execute_cycle(Weight::MAX);
    }
    let run = ActorRunStateStore::<T>::get(actor_id).expect("target Run exists");
    frame_system::Pallet::<T>::set_block_number(run.eligible_at);
    Pallet::<T>::execute_cycle(Weight::MAX);
    assert_reachable_suspended_tail_retry_state::<T>(actor_id, cursor, 1);
    let run = ActorRunStateStore::<T>::get(actor_id).expect("suspension persists");
    frame_system::Pallet::<T>::set_block_number(run.eligible_at);
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    assert!(
      Pallet::<T>::drain_overdue_wakeups_cursor(run.eligible_at, &mut meter).ready_entries >= 1
    );
    let sovereign = Pallet::<T>::actor_identity(actor_id)
      .expect("Suspended identity exists")
      .sovereign_account;
    let retry_funding = amount.saturating_mul(2u32.into());
    T::AssetOps::mint(&sovereign, asset, retry_funding)
      .expect("successful retry receives spendable fixed funding");
    Ok(ReachableSuspendedSuccess {
      actor_id,
      cursor,
      ready_others: benchmark_fixture_ready_occupancy::<T>().saturating_sub(1),
      asset,
      amount,
      recipient,
    })
  }

  fn assert_reachable_suspended_tail_success<T: Config>(
    fixture: ReachableSuspendedSuccess<T>,
    now: BlockNumberFor<T>,
    branch: RunningInnerBranch,
  ) {
    let state = Pallet::<T>::active_actor_state(fixture.actor_id)
      .expect("successful retry retains active authority");
    assert_eq!(
      T::AssetOps::balance(&fixture.recipient, fixture.asset),
      fixture.amount
    );
    match branch {
      RunningInnerBranch::Complete => {
        assert!(state.run_state.is_none());
        assert_eq!(state.identity.cycle_nonce, 1);
        assert_eq!(state.hot.cycle_state, CycleState::Idle);
        assert_eq!(
          benchmark_fixture_ready_occupancy::<T>(),
          fixture.ready_others
        );
      }
      RunningInnerBranch::Progress => {
        let run = state
          .run_state
          .as_ref()
          .expect("progress retry retains Run");
        assert_eq!(state.hot.cycle_state, CycleState::Running);
        assert_eq!(run.cursor, fixture.cursor + 1);
        assert_eq!(run.last_committed_step_block, Some(now));
        assert_eq!(run.last_step_outcome, Some(StepOutcome::Executed));
        assert_eq!(
          benchmark_fixture_ready_occupancy::<T>(),
          fixture.ready_others + 1
        );
      }
    }
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("successful Suspended tail preserves full state audit");
  }

  fn assert_reachable_suspended_tail_skip<T: Config>(
    fixture: ReachableSuspendedSkip<T>,
    now: BlockNumberFor<T>,
    branch: RunningInnerBranch,
  ) {
    let state =
      Pallet::<T>::active_actor_state(fixture.actor_id).expect("skip successor remains active");
    assert!(frame_system::Pallet::<T>::events().iter().any(|record| {
      record.event
        == Event::<T>::StepSkipped {
          actor_id: fixture.actor_id,
          cycle_nonce: 1,
          step_index: fixture.cursor,
          reason: StepSkippedReason::ResolutionSkipped,
        }
        .into()
    }));
    assert!(!state.hot.pending_signal);
    match branch {
      RunningInnerBranch::Complete => {
        assert!(state.run_state.is_none());
        assert_eq!(state.identity.cycle_nonce, 1);
        assert_eq!(state.hot.cycle_state, CycleState::Idle);
        assert_eq!(
          ActorControlLocators::<T>::get(fixture.actor_id),
          Some(ActorControlLocation::Unsignaled)
        );
        assert_eq!(
          benchmark_fixture_ready_occupancy::<T>(),
          fixture.ready_others
        );
      }
      RunningInnerBranch::Progress => {
        let run = state.run_state.as_ref().expect("progress retains real Run");
        assert_eq!(state.hot.cycle_state, CycleState::Running);
        assert_eq!(state.identity.cycle_nonce, 0);
        assert_eq!(run.cursor, fixture.cursor + 1);
        assert_eq!(run.last_committed_step_block, Some(now));
        assert_eq!(
          run.last_step_outcome,
          Some(StepOutcome::Skipped(StepSkippedReason::ResolutionSkipped))
        );
        assert_eq!(run.unsuccessful_attempts_at_cursor, 0);
        assert!(run.suspension.is_none());
        assert_eq!(
          benchmark_fixture_ready_occupancy::<T>(),
          fixture.ready_others + 1
        );
        assert!(matches!(
          ActorControlLocators::<T>::get(fixture.actor_id),
          Some(ActorControlLocation::Ready { .. })
        ));
      }
    }
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("frozen skip successor passes full audit");
    T::BenchmarkHelper::set_asset_account_frozen(
      &fixture.owner,
      &state.identity.sovereign_account,
      fixture.frozen_asset,
      false,
    )
    .expect("supported fixture thaws after measurement");
    for (asset, balance) in fixture.balances {
      assert_eq!(
        T::AssetOps::balance(&state.identity.sovereign_account, asset),
        balance
      );
    }
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("custody-preserving thaw passes full audit");
    // Explicit fixture teardown after all measured-transition and custody assertions. The Actor
    // lifecycle does not burn or unwind custody; this removes only this setup's minted balances.
    for (asset, baseline, minted) in fixture.fixture_funding {
      T::AssetOps::burn(&state.identity.sovereign_account, asset, minted)
        .expect("fixture-only funding teardown succeeds");
      assert_eq!(
        T::AssetOps::balance(&state.identity.sovereign_account, asset),
        baseline
      );
    }
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("fixture funding teardown preserves lifecycle invariants");
  }

  fn prepare_reachable_suspended_tail_retry<T: Config>(
    s: u32,
    p: u32,
  ) -> Result<(ActorId, u32), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let maximum = T::MaxContractSteps::get();
    let chunk = MAX_STEPS_PER_TAIL_CHUNK;
    if s == 0
      || s > chunk
      || maximum <= s
      || p > benchmark_predicate_capacity::<T>()
      || T::MaxRetryAttempts::get() < 3
    {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent two nonterminal tail retries",
      ));
    }
    let count = if s == chunk {
      maximum
    } else {
      1 + s + ((maximum - 1 - s) / chunk) * chunk
    };
    let cursor = if s == chunk { 1 } else { count - s };
    let asset_owner: T::AccountId = account("suspended-retry-assets", 0, 0);
    let mut assets = T::BenchmarkHelper::setup_predicate_assets(&asset_owner, p + 2)
      .expect("retry predicate assets exist");
    let position = assets
      .iter()
      .position(|asset| *asset != T::FeeNativeAssetId::get())
      .expect("retry requires a non-native asset");
    let retry_asset = assets.remove(position);
    assets.truncate(p as usize);
    assert_eq!(assets.len() as u32, p);
    assert!(assets.iter().all(|asset| *asset != retry_asset));
    let precondition = if p == 0 {
      None
    } else {
      Some(packed_predicate_clauses::<T>(
        assets
          .into_iter()
          .map(|asset| TimedPredicate {
            timing: ObservationTiming::Current,
            predicate: Predicate::BalanceBelow {
              asset,
              threshold: One::one(),
            },
          })
          .collect(),
        p.div_ceil(T::MaxPreconditionClauses::get()).max(1),
      ))
    };
    let actor_id = prepare_reachable_running_with_cooldown::<T>(
      ActorType::System,
      count,
      Some((
        cursor,
        Step {
          precondition,
          task: ActorTask::Transfer {
            to: account("suspended-retry-recipient", 0, 0),
            asset: retry_asset,
            amount: AmountResolution::Fixed(One::one()),
          },
          on_error: StepErrorPolicy::RetryLater {
            max_attempts: T::MaxRetryAttempts::get(),
          },
        },
      )),
      2,
    )?;
    for expected_cursor in 1..cursor {
      let run = ActorRunStateStore::<T>::get(actor_id).expect("real committed prefix retains Run");
      assert_eq!(run.cursor, expected_cursor);
      frame_system::Pallet::<T>::set_block_number(run.eligible_at);
      Pallet::<T>::execute_cycle(Weight::MAX);
    }
    let state =
      Pallet::<T>::active_actor_state(actor_id).expect("target has real Running authority");
    let run = state.run_state.as_ref().expect("target Run exists");
    assert_eq!(run.cursor, cursor);
    assert!(T::AssetOps::balance(&state.identity.sovereign_account, retry_asset).is_zero());
    assert_eq!(
      state.contract.steps[cursor as usize]
        .precondition
        .as_ref()
        .map_or(0, Precondition::evaluation_units),
      p
    );
    let context = Pallet::<T>::step_control_weight_context(count, cursor, p, 0, 0, 0)
      .expect("tail context exists");
    assert_eq!(context.steps_in_fragment, s);
    assert_eq!(context.predicate_evaluation_units, p);
    frame_system::Pallet::<T>::set_block_number(run.eligible_at);
    Pallet::<T>::execute_cycle(Weight::MAX);
    assert_reachable_suspended_tail_retry_state::<T>(actor_id, cursor, 1);
    let run = ActorRunStateStore::<T>::get(actor_id).expect("first real retry persists");
    let retained = run.encode();
    frame_system::Pallet::<T>::set_block_number(run.eligible_at);
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    let drained = Pallet::<T>::drain_overdue_wakeups_cursor(run.eligible_at, &mut meter);
    assert!(drained.ready_entries >= 1);
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Ready { .. })
    ));
    assert_eq!(
      ActorRunStateStore::<T>::get(actor_id)
        .expect("due retry Run persists")
        .encode(),
      retained
    );
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Ready { .. })
    ));
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real due retry Ready source passes full audit");
    Ok((actor_id, cursor))
  }

  fn assert_reachable_suspended_tail_retry_state<T: Config>(
    actor_id: ActorId,
    cursor: u32,
    attempts: u32,
  ) {
    let state = Pallet::<T>::active_actor_state(actor_id).expect("real retry authority remains");
    let run = state.run_state.as_ref().expect("real retry Run remains");
    assert_eq!(state.hot.cycle_state, CycleState::Suspended);
    assert!(!state.hot.pending_signal);
    assert_eq!(state.identity.cycle_nonce, 0);
    assert_eq!(run.cycle_nonce, 1);
    assert_eq!(run.cursor, cursor);
    assert_eq!(run.unsuccessful_attempts_at_cursor, attempts);
    assert_eq!(run.last_step_outcome, Some(StepOutcome::FundingUnavailable));
    assert_eq!(run.suspension, Some(SuspensionReason::FundingUnavailable));
    let now = frame_system::Pallet::<T>::block_number();
    assert_eq!(run.last_attempt_block, now);
    let due =
      Pallet::<T>::suspension_eligible_at(2, None, now, attempts).expect("retry due boundary fits");
    assert_eq!(run.eligible_at, due);
    assert!(
      matches!(ActorControlLocators::<T>::get(actor_id), Some(ActorControlLocation::Waiting { key: WakeupKey::Block(at), .. }) if at == due)
    );
    assert_eq!(
      run.opening_snapshot.len(),
      Pallet::<T>::opening_surfaces(&state.contract.steps, 0).len()
    );
    assert_eq!(
      run.opening_predicate_results.len() as u32,
      state
        .contract
        .steps
        .iter()
        .map(|step| step
          .precondition
          .as_ref()
          .map_or(0, Precondition::opening_predicate_count))
        .sum::<u32>()
    );
    assert_eq!(
      run.opening_predicate_cursor,
      state.contract.steps[..cursor as usize]
        .iter()
        .map(|step| step
          .precondition
          .as_ref()
          .map_or(0, Precondition::opening_predicate_count))
        .sum::<u32>()
    );
    assert!(
      run
        .opening_predicate_results
        .iter()
        .all(|result| *result == Ok(false))
    );
    assert!(
      run.funding_snapshot.is_empty()
        && state.funding.funding_accumulated.is_empty()
        && state.funding.funding_tracked_assets.is_empty()
    );
    let ActorTask::Transfer { asset, ref to, .. } = state.contract.steps[cursor as usize].task
    else {
      unreachable!()
    };
    assert!(T::AssetOps::balance(&state.identity.sovereign_account, asset).is_zero());
    assert!(T::AssetOps::balance(to, asset).is_zero());
    assert_eq!(benchmark_fixture_ready_occupancy::<T>(), 0);
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real Suspended Waiting retry passes full audit");
  }

  fn prepare_reachable_running_with_step<T: Config>(
    step_count: u32,
    current_step: Option<(u32, StepOf<T>)>,
  ) -> Result<ActorId, polkadot_sdk::frame_benchmarking::BenchmarkError> {
    prepare_reachable_running_with_cooldown::<T>(ActorType::System, step_count, current_step, 0)
  }

  fn prepare_reachable_running_with_cooldown<T: Config>(
    actor_type: ActorType,
    step_count: u32,
    current_step: Option<(u32, StepOf<T>)>,
    cooldown_blocks: u32,
  ) -> Result<ActorId, polkadot_sdk::frame_benchmarking::BenchmarkError> {
    if step_count < 2 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "Running fixture requires a successor Step",
      ));
    }
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    GlobalCircuitBreaker::<T>::put(false);
    let owner: T::AccountId = account("reachable-state-owner", 0, 0);
    let mut steps = make_reachable_opening_steps::<T>(step_count)?;
    if actor_type == ActorType::User {
      // Native prefunding must not turn the authored no-effect prefix into an invoked Task.
      for step in &mut steps {
        for clause in &mut step
          .precondition
          .as_mut()
          .expect("Opening predicates exist")
          .clauses
        {
          for timed in clause {
            if let Predicate::BalanceAbove { asset, threshold } = &mut timed.predicate {
              if *asset == T::FeeNativeAssetId::get() {
                *threshold = <T::Balance as polkadot_sdk::sp_runtime::traits::Bounded>::max_value()
                  .saturating_sub(*threshold);
              }
            }
          }
        }
      }
    }
    let protected_cursor = current_step.as_ref().map(|(cursor, _)| *cursor);
    if let Some((cursor, step)) = current_step {
      assert!(cursor > 0 && cursor < step_count);
      steps[cursor as usize] = step;
    }
    let mut protected = vec![0];
    if let Some(cursor) = protected_cursor {
      protected.push(cursor);
    }
    retain_admitted_contract_geometry::<T>(actor_type, &mut steps, &protected)?;
    let surfaces = Pallet::<T>::opening_surfaces(&steps, 0);
    let opening_predicates = steps
      .iter()
      .filter_map(|step| step.precondition.as_ref())
      .flat_map(|precondition| precondition.clauses.iter())
      .flat_map(|clause| clause.iter())
      .filter(|timed| timed.timing == ObservationTiming::Opening)
      .count() as u32;
    assert!(steps.iter().all(|step| matches!(
      &step.task,
      ActorTask::AddLiquidity {
        amount_a: AmountResolution::PercentageAtOpening(_),
        amount_b: AmountResolution::PercentageAtOpening(_),
        ..
      } | ActorTask::AddLiquidity {
        amount_a: AmountResolution::PercentageOfCurrent(_),
        amount_b: AmountResolution::PercentageOfCurrent(_),
        ..
      } | ActorTask::StopCycle
        | ActorTask::Transfer {
          amount: AmountResolution::PercentageOfCurrent(_),
          ..
        }
        | ActorTask::Transfer {
          amount: AmountResolution::Fixed(_) | AmountResolution::AllAvailable,
          ..
        }
    )));
    assert!(surfaces.len() as u32 <= T::MaxOpeningSnapshotEntries::get());
    assert!(opening_predicates <= T::MaxOpeningPredicateResults::get());
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks,
    };
    if actor_type == ActorType::User {
      ensure_creation_balance::<T>(&owner);
      prefund_active_user_creation::<T>(&owner, &steps);
      Pallet::<T>::create_user_actor(
        RawOrigin::Signed(owner.clone()).into(),
        Mutability::Mutable,
        user_contract::<T>(schedule, steps),
      )
      .expect("reachable maximum User Contract is admitted");
    } else {
      Pallet::<T>::create_system_actor(
        RawOrigin::Root.into(),
        owner.clone(),
        Mutability::Mutable,
        system_contract::<T>(schedule, steps),
      )
      .expect("reachable maximum System Contract is admitted");
    }
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    if actor_type == ActorType::User {
      let sovereign = Pallet::<T>::actor_identity(actor_id)
        .expect("admitted User identity exists")
        .sovereign_account;
      let trigger_fee = Pallet::<T>::trigger_fee_for_weight(
        ActorType::User,
        TriggerFamily::Manual,
        T::WeightInfo::manual_trigger(),
      )
      .trigger_fee;
      T::AssetOps::mint(&sovereign, T::FeeNativeAssetId::get(), trigger_fee)
        .expect("Manual readiness has funding independent of Pipeline capacity");
    }
    Pallet::<T>::manual_trigger(RawOrigin::Signed(owner).into(), actor_id)
      .expect("real Manual occurrence latches the actor");
    // Normal Q1 service consumes only Step 0 at this block; false predicates invoke no Task.
    Pallet::<T>::execute_cycle(Weight::MAX);
    let state = Pallet::<T>::active_actor_state(actor_id).expect("reachable actor remains active");
    assert_eq!(state.hot.cycle_state, CycleState::Running);
    let run = state
      .run_state
      .as_ref()
      .expect("Opening published a real Run");
    assert_eq!(run.cursor, 1);
    assert_eq!(run.last_committed_step_block, Some(now));
    assert_eq!(run.cycle_nonce, state.identity.cycle_nonce + 1);
    assert_eq!(run.opening_snapshot.len(), surfaces.len());
    assert!(
      surfaces
        .iter()
        .all(|surface| run.opening_snapshot.contains_key(surface))
    );
    assert_eq!(
      run.opening_predicate_results.len() as u32,
      opening_predicates
    );
    assert!(state.funding.funding_tracked_assets.is_empty());
    assert!(state.funding.funding_accumulated.is_empty());
    assert!(run.funding_snapshot.is_empty());
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("reachable probe fixture passes full state audit");
    Ok(actor_id)
  }

  #[benchmark]
  fn scheduler_actor_state_probe() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_reachable_running::<T>(T::MaxContractSteps::get())?;
    let state = Pallet::<T>::active_actor_state(actor_id).expect("real probe state exists");
    let run = state.run_state.as_ref().expect("real probe Run exists");
    assert_eq!(
      run.opening_snapshot.len(),
      Pallet::<T>::opening_surfaces(&state.contract.steps, 0).len()
    );
    assert_eq!(
      run.opening_predicate_results.len() as u32,
      state
        .contract
        .steps
        .iter()
        .map(|step| step
          .precondition
          .as_ref()
          .map_or(0, Precondition::opening_predicate_count))
        .sum::<u32>()
    );
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::load_current_step_service_state(actor_id)
          .expect("benchmark current-Step service state loads coherently"),
      );
    }
    assert!(matches!(
      Pallet::<T>::load_actor_state(actor_id),
      LoadedActorStateOf::Active(_)
    ));
    Ok(())
  }
  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_append_existing_page() {
    let page_size = 32u32;
    assert!(
      page_size >= 2,
      "benchmark requires a non-trivial queue page"
    );
    for i in 0..page_size.saturating_sub(1) {
      let actor_id = bench_create_system_manual::<T>(31_000_000u32.saturating_add(i));
      assert!(benchmark_fixture_ready_enqueue::<T>(actor_id));
    }
    let actor_id = bench_create_system_manual::<T>(32_000_000);
    let append = benchmark_fixture_prepare_consumed_ready_enqueue::<T>(actor_id);
    #[block]
    {
      assert!(append());
    }
    assert_eq!(benchmark_fixture_ready_tail::<T>(), u64::from(page_size));
    assert_eq!(
      benchmark_fixture_scalar_hot::<T>(actor_id).and_then(|hot| hot.queue_ticket),
      Some(u64::from(page_size - 1))
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_append_new_page() {
    let page_size = 32u32;
    for i in 0..page_size {
      let actor_id = bench_create_system_manual::<T>(33_000_000u32.saturating_add(i));
      assert!(benchmark_fixture_ready_enqueue::<T>(actor_id));
    }
    let actor_id = bench_create_system_manual::<T>(34_000_000);
    let append = benchmark_fixture_prepare_consumed_ready_enqueue::<T>(actor_id);
    #[block]
    {
      assert!(append());
    }
    assert_eq!(
      benchmark_fixture_ready_tail::<T>(),
      u64::from(page_size).saturating_add(1)
    );
    assert_eq!(benchmark_fixture_ready_page_len::<T>(1), Some(32));
    let page = ActorReadyFrameChunks::<T>::get(1).expect("new Ready page exists");
    assert_eq!(page[0].as_ref().map(|cell| cell.actor_id), Some(actor_id));
    assert!(page[1..].iter().all(Option::is_none));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_append_existing_page() {
    let page_size = 32u32;
    assert!(
      page_size >= 2,
      "benchmark requires a non-trivial wakeup page"
    );
    let wakeup_block = 100u32.into();
    for i in 0..page_size.saturating_sub(1) {
      let actor_id = bench_create_system_manual::<T>(41_000_000u32.saturating_add(i));
      benchmark_fixture_schedule_service_waiting::<T>(actor_id, wakeup_block);
    }
    let actor_id = bench_create_system_manual::<T>(42_000_000);
    let append = benchmark_fixture_prepare_service_waiting::<T>(actor_id, wakeup_block);
    #[block]
    {
      append();
    }
    let pointer = benchmark_fixture_scalar_hot::<T>(actor_id)
      .and_then(|hot| hot.wakeup_pointer)
      .expect("benchmark wakeup pointer must exist");
    assert_eq!((pointer.page_id, pointer.slot), (0, page_size - 1));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_append_new_page() {
    let page_size = 32u32;
    let wakeup_block = 100u32.into();
    for i in 0..page_size {
      let actor_id = bench_create_system_manual::<T>(43_000_000u32.saturating_add(i));
      benchmark_fixture_schedule_service_waiting::<T>(actor_id, wakeup_block);
    }
    let actor_id = bench_create_system_manual::<T>(44_000_000);
    let append = benchmark_fixture_prepare_service_waiting::<T>(actor_id, wakeup_block);
    #[block]
    {
      append();
    }
    let pointer = benchmark_fixture_scalar_hot::<T>(actor_id)
      .and_then(|hot| hot.wakeup_pointer)
      .expect("benchmark wakeup pointer must exist");
    assert_eq!((pointer.page_id, pointer.slot), (1, 0));
    let key = WakeupKey::Block(wakeup_block);
    let previous =
      ActorWaitingFrameChunks::<T>::get((key, 0)).expect("full preceding Waiting page remains");
    let page = ActorWaitingFrameChunks::<T>::get((key, 1)).expect("new linked Waiting page exists");
    assert_eq!(
      (previous.previous_page, previous.next_page),
      (None, Some(1))
    );
    assert_eq!(previous.live_entries, page_size);
    assert_eq!((page.previous_page, page.next_page), (Some(0), None));
    assert_eq!(page.live_entries, 1);
    assert_eq!(page.entries.len(), page_size as usize);
    assert_eq!(
      page.entries[0]
        .as_ref()
        .and_then(ActorWaitingEntry::primary)
        .map(|cell| cell.actor_id),
      Some(actor_id)
    );
    assert!(page.entries[1..].iter().all(Option::is_none));
    assert_eq!(ActorWaitingOccupancies::<T>::get(key), page_size + 1);
    assert!(Pallet::<T>::wakeup_page_entry_matches(pointer, actor_id));
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state()
      .expect("new Waiting page reconciles primary and directory ownership");
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_replace_exact() {
    let actor_id = bench_create_system_manual::<T>(45_000_000);
    let old_block = 100u32.into();
    let replacement_block = 200u32.into();
    benchmark_fixture_schedule_service_waiting::<T>(actor_id, old_block);
    #[block]
    {
      assert!(Pallet::<T>::wakeup_substrate_schedule(
        actor_id,
        replacement_block
      ));
    }
    let pointer = benchmark_fixture_scalar_hot::<T>(actor_id)
      .and_then(|hot| hot.wakeup_pointer)
      .expect("replacement wakeup pointer must exist");
    assert_eq!(
      (pointer.block, pointer.page_id, pointer.slot),
      (WakeupKey::Block(replacement_block), 0, 0)
    );
    assert!(!ActorWaitingOccupancies::<T>::contains_key(
      WakeupKey::Block(old_block)
    ));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_invalidate_middle_page() {
    let page_size = 32u32;
    let wakeup_block = 100u32.into();
    let count = page_size.saturating_mul(2).saturating_add(1);
    let mut actors = alloc::vec::Vec::with_capacity(count as usize);
    for i in 0..count {
      let actor_id = bench_create_system_manual::<T>(46_000_000u32.saturating_add(i));
      benchmark_fixture_schedule_service_waiting::<T>(actor_id, wakeup_block);
      actors.push(actor_id);
    }
    let middle_start = page_size as usize;
    let middle_end = middle_start.saturating_add(page_size as usize);
    for actor_id in &actors[middle_start..middle_end.saturating_sub(1)] {
      assert!(Pallet::<T>::wakeup_substrate_invalidate(*actor_id).is_some());
    }
    let actor_id = actors[middle_end - 1];
    #[block]
    {
      assert!(Pallet::<T>::wakeup_substrate_invalidate(actor_id).is_some());
    }
    assert!(!ActorWaitingFrameChunks::<T>::contains_key((
      WakeupKey::Block(wakeup_block),
      1
    )));
    assert_eq!(
      ActorWaitingFrameChunks::<T>::get((WakeupKey::Block(wakeup_block), 0))
        .and_then(|page| page.next_page),
      Some(2)
    );
    assert_eq!(
      ActorWaitingFrameChunks::<T>::get((WakeupKey::Block(wakeup_block), 2))
        .and_then(|page| page.previous_page),
      Some(0)
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_drain_partial_page() {
    let page_size = 32u32;
    assert!(page_size >= 2, "benchmark requires a partial page");
    let wakeup_block = 100u32.into();
    for i in 0..page_size {
      let actor_id = bench_create_system_manual::<T>(47_000_000u32.saturating_add(i));
      benchmark_fixture_schedule_service_waiting::<T>(actor_id, wakeup_block);
    }
    let scan_limit = page_size / 2;
    #[block]
    {
      let (ready, stats) = Pallet::<T>::wakeup_substrate_drain_block(wakeup_block, scan_limit);
      assert_eq!(ready.len(), scan_limit as usize);
      assert_eq!(stats.entries_scanned, scan_limit);
      assert_eq!(stats.pages_deleted, 0);
    }
    assert_eq!(
      ActorWaitingFrameChunks::<T>::get((WakeupKey::Block(wakeup_block), 0))
        .map(|page| page.scan_slot),
      Some(scan_limit)
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_drain_full_page() {
    let page_size = 32u32;
    let wakeup_block = 100u32.into();
    for i in 0..page_size {
      let actor_id = bench_create_system_manual::<T>(48_000_000u32.saturating_add(i));
      benchmark_fixture_schedule_service_waiting::<T>(actor_id, wakeup_block);
    }
    #[block]
    {
      let (ready, stats) = Pallet::<T>::wakeup_substrate_drain_block(wakeup_block, page_size);
      assert_eq!(ready.len(), page_size as usize);
      assert_eq!(stats.entries_scanned, page_size);
      assert_eq!(stats.pages_deleted, 1);
    }
    assert!(!ActorWaitingOccupancies::<T>::contains_key(
      WakeupKey::Block(wakeup_block)
    ));
    assert!(!ActorWaitingFrameChunks::<T>::contains_key((
      WakeupKey::Block(wakeup_block),
      0
    )));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_drain_dense_boundary() {
    let page_size = 32u32;
    let count = page_size.saturating_add(1);
    assert!(
      count <= T::MaxWakeupsPerBlock::get(),
      "benchmark requires one boundary-crossing drain"
    );
    let wakeup_block = 100u32.into();
    for i in 0..count {
      let actor_id = bench_create_system_manual::<T>(49_000_000u32.saturating_add(i));
      benchmark_fixture_schedule_service_waiting::<T>(actor_id, wakeup_block);
    }
    #[block]
    {
      let (ready, stats) = Pallet::<T>::wakeup_substrate_drain_block(wakeup_block, count);
      assert_eq!(ready.len(), count as usize);
      assert_eq!(stats.entries_scanned, count);
      assert_eq!(stats.pages_touched, 2);
      assert_eq!(stats.pages_deleted, 2);
    }
    assert!(!ActorWaitingOccupancies::<T>::contains_key(
      WakeupKey::Block(wakeup_block)
    ));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_drain_stale_page() {
    let page_size = 32u32;
    let wakeup_block = 100u32.into();
    for i in 0..page_size {
      let actor_id = bench_create_system_manual::<T>(50_000_000u32.saturating_add(i));
      benchmark_fixture_schedule_service_waiting::<T>(actor_id, wakeup_block);
      let (location, cell) =
        Pallet::<T>::actor_control_cell(actor_id).expect("benchmark Waiting primary exists");
      let ActorControlLocation::Waiting { key, page, slot } = location else {
        panic!("benchmark service primary is Waiting");
      };
      // Explicit orphan corruption exercises stale-reference removal, not normal close.
      ActorWaitingFrameChunks::<T>::mutate((key, page), |stored| {
        stored
          .as_mut()
          .expect("benchmark Waiting page exists")
          .entries[slot as usize] = Some(ActorWaitingEntry::Reference(ActorWakeupReference {
          actor_id,
          admission_identity: cell.admission.admission_identity,
        }));
      });
      ActorControlLocators::<T>::remove(actor_id);
    }
    #[block]
    {
      let (ready, stats) = Pallet::<T>::wakeup_substrate_drain_block(wakeup_block, page_size);
      assert!(ready.is_empty());
      assert_eq!(stats.entries_scanned, page_size);
      assert_eq!(stats.stale_entries, page_size);
      assert_eq!(stats.pages_deleted, 1);
    }
    assert!(!ActorWaitingOccupancies::<T>::contains_key(
      WakeupKey::Block(wakeup_block)
    ));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_cursor_insert() {
    clear_host_genesis_wakeup_placements::<T>();
    let inserted_block: BlockNumberFor<T> = 1u32.into();
    let actor_id = bench_create_system_manual::<T>(50_100_000);
    benchmark_fixture_schedule_service_waiting::<T>(actor_id, inserted_block);
    // Preserve the live Waiting owner at the boundary before its cursor is installed.
    ActorWaitingCursorIndices::<T>::remove(WakeupKey::Block(inserted_block));
    let page_size = T::WakeupPageSize::get();
    let max_active = T::MaxActiveActors::get();
    assert!(
      page_size > 0 && max_active > 1,
      "benchmark requires bounded cursor depth"
    );
    let insert_index = max_active.saturating_sub(1);
    let tail_page = u64::from(insert_index / page_size);
    let tail_len = insert_index % page_size;
    let mut page_ids = alloc::vec::Vec::new();
    let mut current = insert_index;
    loop {
      add_wakeup_cursor_page(&mut page_ids, current, page_size);
      if current == 0 {
        break;
      }
      current = current.saturating_sub(1) / 2;
    }
    for page_id in page_ids {
      let len = if page_id == tail_page {
        tail_len
      } else {
        page_size
      };
      if len > 0 {
        install_wakeup_cursor_page::<T>(page_id, len);
      }
    }
    WakeupCursorLen::<T>::insert(WakeupClock::Block, insert_index);
    #[block]
    {
      assert!(Pallet::<T>::wakeup_cursor_insert(inserted_block));
    }
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Block), max_active);
    assert_eq!(Pallet::<T>::wakeup_cursor_peek(), Some(inserted_block));
    assert_wakeup_cursor_page_indices::<T>();
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_cursor_pop_min() {
    clear_host_genesis_wakeup_placements::<T>();
    let cursor_len = T::MaxActiveActors::get();
    let expected_min = prepare_wakeup_cursor_repair::<T>(0);
    #[block]
    {
      assert_eq!(Pallet::<T>::wakeup_cursor_pop_min(), Some(expected_min));
    }
    assert_eq!(
      WakeupCursorLen::<T>::get(WakeupClock::Block),
      cursor_len.saturating_sub(1)
    );
    assert_eq!(Pallet::<T>::wakeup_cursor_peek(), Some(1_000_001u32.into()));
    assert_eq!(
      ActorWaitingCursorIndices::<T>::get(WakeupKey::Block(expected_min)),
      None
    );
    assert_wakeup_cursor_page_indices::<T>();
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_cursor_remove_exact() {
    clear_host_genesis_wakeup_placements::<T>();
    let cursor_len = T::MaxActiveActors::get();
    let removed_block = prepare_wakeup_cursor_repair::<T>(1);
    #[block]
    {
      assert!(Pallet::<T>::wakeup_cursor_remove(removed_block));
    }
    assert_eq!(
      WakeupCursorLen::<T>::get(WakeupClock::Block),
      cursor_len.saturating_sub(1)
    );
    assert_eq!(Pallet::<T>::wakeup_cursor_peek(), Some(1_000_000u32.into()));
    assert_eq!(
      ActorWaitingCursorIndices::<T>::get(WakeupKey::Block(removed_block)),
      None
    );
    assert_wakeup_cursor_page_indices::<T>();
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn scheduler_wakeup_cursor_remove_upward_depth()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let removed = prepare_upward_cursor_removal::<T>(6_143)?;
    #[block]
    {
      assert!(Pallet::<T>::wakeup_cursor_remove(removed));
    }
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Block), 9_999);
    assert_eq!(
      ActorWaitingCursorIndices::<T>::get(WakeupKey::Block(removed)),
      None
    );
    assert_eq!(
      ActorWaitingCursorIndices::<T>::get(WakeupKey::Block(14u32.into())),
      Some(2)
    );
    assert_wakeup_cursor_page_indices::<T>();
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn scheduler_wakeup_cursor_remove_upward_pages()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let removed = prepare_upward_cursor_removal::<T>(8_447)?;
    #[block]
    {
      assert!(Pallet::<T>::wakeup_cursor_remove(removed));
    }
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Block), 9_999);
    assert_eq!(
      ActorWaitingCursorIndices::<T>::get(WakeupKey::Block(removed)),
      None
    );
    assert_eq!(
      ActorWaitingCursorIndices::<T>::get(WakeupKey::Block(14u32.into())),
      Some(7)
    );
    assert_wakeup_cursor_page_indices::<T>();
    Ok(())
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_cursor_worker_partial() {
    clear_host_genesis_wakeup_placements::<T>();
    let first = bench_create_system_manual::<T>(34_100_000);
    let second = bench_create_system_manual::<T>(34_100_001);
    for actor_id in [first, second] {
      let mut contract =
        Pallet::<T>::load_actor_contract(actor_id).expect("benchmark actor contract exists");
      contract.trigger = Trigger::Cadenced { every_ticks: 1 };
      Pallet::<T>::store_actor_contract(actor_id, contract)
        .expect("benchmark cadence Contract remains admitted");
      benchmark_fixture_mutate_hot::<T>(actor_id, |hot| {
        hot.trigger_runtime_state = TriggerRuntimeState::Cadenced { anchor_tick: None };
      });
      Pallet::<T>::benchmark_defer_tick_wakeup(actor_id, 0)
        .expect("benchmark bootstrap wakeup fits");
    }
    let limit = T::WeightInfo::scheduler_wakeup_cursor_worker_future()
      .saturating_mul(2)
      .saturating_add(Pallet::<T>::wakeup_cursor_drain_unit_weight_upper(
        crate::scheduler::WakeupBucketDisposition::Retain,
      ));
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(limit);
    #[block]
    {
      let stats = Pallet::<T>::drain_overdue_wakeups_cursor(10u32.into(), &mut meter);
      assert_eq!(stats.entries_scanned, 1);
      assert_eq!(stats.ready_entries, 1);
    }
    assert_eq!(
      ActorWaitingOccupancies::<T>::try_get(WakeupKey::Tick(0)).ok(),
      Some(1)
    );
    let rearmed_tick = benchmark_fixture_hot::<T>(first)
      .and_then(|hot| hot.trigger_runtime_state.temporal_anchor_tick())
      .and_then(|anchor| anchor.checked_add(1))
      .expect("benchmark cadence re-anchors");
    assert_eq!(
      ActorWaitingOccupancies::<T>::try_get(WakeupKey::Tick(rearmed_tick)).ok(),
      Some(1)
    );
  }

  /// Measures one due User AtTime occurrence independently from timestamp inherent work:
  /// one-shot consumption, exact Trigger collection, and canonical readiness placement.
  #[benchmark(pov_mode = Measured)]
  fn at_time_trigger_occurrence() {
    clear_host_genesis_wakeup_placements::<T>();
    let owner: T::AccountId = whitelisted_caller();
    let recipient: T::AccountId = account("at-time-occurrence-recipient", 0, 0);
    let contract_steps = make_contract_steps::<T>(recipient);
    ensure_creation_balance::<T>(&owner);
    prefund_active_user_creation::<T>(&owner, &contract_steps);
    Pallet::<T>::create_user_actor(
      RawOrigin::Signed(owner).into(),
      Mutability::Mutable,
      user_contract::<T>(
        Schedule {
          trigger: Trigger::at_time(1),
          cooldown_blocks: 0,
        },
        contract_steps,
      ),
    )
    .expect("AtTime benchmark Actor exists");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    seed_actor_for_cycle::<T>(actor_id);
    Pallet::<T>::trigger_wakeup_substrate_invalidate_inner(actor_id)
      .expect("initial AtTime pointer is coherent")
      .expect("initial AtTime pointer exists");
    benchmark_fixture_mutate_hot::<T>(actor_id, |hot| {
      hot.trigger_runtime_state = TriggerRuntimeState::AtTime {
        anchor_tick: Some(0),
        consumed: false,
      };
    });
    Pallet::<T>::benchmark_defer_tick_wakeup(actor_id, 1).expect("due AtTime occurrence fits");
    benchmark_fixture_publish_trigger_waiting::<T>(actor_id, 1);
    let (mut ready, stats) = Pallet::<T>::wakeup_substrate_drain_key(WakeupKey::Tick(1), 1);
    assert_eq!(stats.entries_scanned, 1);
    let (_, state, admission, loaded_step) = ready
      .pop()
      .expect("due AtTime source authority is consumed");
    #[block]
    {
      assert_eq!(
        Pallet::<T>::process_due_temporal_occurrence_loaded(
          actor_id,
          state,
          admission,
          loaded_step,
          1,
        ),
        Ok(false)
      );
    }
    let actor = Pallet::<T>::active_actor_view(actor_id).expect("AtTime Actor remains active");
    assert!(actor.pending_signal);
    assert!(actor.queue_ticket.is_some());
    assert!(actor.trigger_wakeup_pointer.is_none());
    assert!(matches!(
      actor.trigger_runtime_state,
      TriggerRuntimeState::AtTime { consumed: true, .. }
    ));
  }

  /// Measures one due User Cadenced occurrence independently from timestamp inherent work:
  /// trigger-deadline advancement, exact Trigger collection, and canonical readiness placement.
  #[benchmark(pov_mode = Measured)]
  fn cadenced_trigger_occurrence() {
    clear_host_genesis_wakeup_placements::<T>();
    let owner: T::AccountId = whitelisted_caller();
    let recipient: T::AccountId = account("cadenced-occurrence-recipient", 0, 0);
    let contract_steps = make_contract_steps::<T>(recipient);
    ensure_creation_balance::<T>(&owner);
    prefund_active_user_creation::<T>(&owner, &contract_steps);
    Pallet::<T>::create_user_actor(
      RawOrigin::Signed(owner).into(),
      Mutability::Mutable,
      user_contract::<T>(
        Schedule {
          trigger: Trigger::cadenced(1),
          cooldown_blocks: 0,
        },
        contract_steps,
      ),
    )
    .expect("Cadenced benchmark Actor exists");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    seed_actor_for_cycle::<T>(actor_id);
    Pallet::<T>::trigger_wakeup_substrate_invalidate_inner(actor_id)
      .expect("initial Cadenced pointer is coherent")
      .expect("initial Cadenced pointer exists");
    benchmark_fixture_mutate_hot::<T>(actor_id, |hot| {
      hot.trigger_runtime_state = TriggerRuntimeState::Cadenced {
        anchor_tick: Some(0),
      };
    });
    Pallet::<T>::benchmark_defer_tick_wakeup(actor_id, 0).expect("due Cadenced occurrence fits");
    benchmark_fixture_publish_trigger_waiting::<T>(actor_id, 0);
    let (mut ready, stats) = Pallet::<T>::wakeup_substrate_drain_key(WakeupKey::Tick(0), 1);
    assert_eq!(stats.entries_scanned, 1);
    let (_, state, admission, loaded_step) = ready
      .pop()
      .expect("due Cadenced source authority is consumed");
    #[block]
    {
      assert_eq!(
        Pallet::<T>::process_due_temporal_occurrence_loaded(
          actor_id,
          state,
          admission,
          loaded_step,
          0,
        ),
        Ok(false)
      );
    }
    let actor = Pallet::<T>::active_actor_view(actor_id).expect("Cadenced Actor remains active");
    assert!(actor.pending_signal);
    assert!(actor.queue_ticket.is_some());
    assert!(actor.trigger_wakeup_pointer.is_none());
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_cursor_worker_remove() {
    clear_host_genesis_wakeup_placements::<T>();
    let cursor_len = T::MaxActiveActors::get();
    let wakeup_block: BlockNumberFor<T> = 1_000_000u32.into();
    let actor_id = bench_create_system_manual::<T>(34_200_000);
    benchmark_fixture_schedule_service_waiting::<T>(actor_id, wakeup_block);
    assert_eq!(prepare_wakeup_cursor_repair::<T>(0), wakeup_block);
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    #[block]
    {
      let stats = Pallet::<T>::drain_overdue_wakeups_cursor(wakeup_block, &mut meter);
      assert_eq!(stats.entries_scanned, 1);
      assert_eq!(stats.ready_entries, 1);
    }
    assert_eq!(
      WakeupCursorLen::<T>::get(WakeupClock::Block),
      cursor_len.saturating_sub(1)
    );
    assert_eq!(Pallet::<T>::wakeup_cursor_peek(), Some(1_000_001u32.into()));
    assert!(!ActorWaitingOccupancies::<T>::contains_key(
      WakeupKey::Block(wakeup_block)
    ));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_cursor_worker_future() {
    clear_host_genesis_wakeup_placements::<T>();
    let wakeup_block: BlockNumberFor<T> = 1_000_000u32.into();
    let actor_id = bench_create_system_manual::<T>(34_300_000);
    benchmark_fixture_schedule_service_waiting::<T>(actor_id, wakeup_block);
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    #[block]
    {
      let stats = Pallet::<T>::drain_overdue_wakeups_cursor(10u32.into(), &mut meter);
      assert_eq!(stats.entries_scanned, 0);
    }
    assert_eq!(Pallet::<T>::wakeup_cursor_peek(), Some(wakeup_block));
    assert!(
      benchmark_fixture_scalar_hot::<T>(actor_id)
        .and_then(|hot| hot.wakeup_pointer)
        .is_some()
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_consume_preserve_page() {
    let first = bench_create_system_manual::<T>(35_000_000);
    let second = bench_create_system_manual::<T>(35_000_001);
    assert!(benchmark_fixture_ready_enqueue::<T>(first));
    assert!(benchmark_fixture_ready_enqueue::<T>(second));
    let hot = benchmark_fixture_hot::<T>(first).expect("benchmark loaded source exists");
    #[block]
    {
      assert!(benchmark_fixture_ready_consume_loaded_head::<T>(
        0, first, 0, hot,
      ));
    }
    assert_eq!(benchmark_fixture_ready_head::<T>(), 1);
    assert!(benchmark_fixture_contains_ready_page::<T>(0));
    assert!(!ActorControlLocators::<T>::contains_key(first));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_consume_delete_page() {
    let actor_id = bench_create_system_manual::<T>(36_000_000);
    assert!(benchmark_fixture_ready_enqueue::<T>(actor_id));
    let hot = benchmark_fixture_hot::<T>(actor_id).expect("benchmark loaded source exists");
    #[block]
    {
      assert!(benchmark_fixture_ready_consume_loaded_head::<T>(
        0, actor_id, 0, hot,
      ));
    }
    assert_eq!(benchmark_fixture_ready_head::<T>(), 1);
    assert_eq!(benchmark_fixture_ready_tail::<T>(), 1);
    assert!(!benchmark_fixture_contains_ready_page::<T>(0));
    assert!(!ActorControlLocators::<T>::contains_key(actor_id));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_tombstone_drain(n: Linear<1, 10_000>) {
    let bounded = n.min(T::MaxQueueLength::get());
    let head = ActorReadyTail::<T>::get();
    let tail = head
      .checked_add(u64::from(bounded))
      .expect("bounded fixture span");
    for page_id in head / 32..tail.div_ceil(32) {
      ActorReadyFrameChunks::<T>::insert(
        page_id,
        ActorControlChunkOf::<T>::try_from(vec![None; 32]).expect("fixed Ready chunk"),
      );
    }
    benchmark_fixture_set_ready_queue_state::<T>(head, tail, 0);
    #[block]
    {
      core::hint::black_box(
        benchmark_fixture_ready_drain_tombstones::<T>(tail, bounded)
          .expect("benchmark queue topology is valid"),
      );
    }
    assert_eq!(benchmark_fixture_ready_head::<T>(), tail);
    assert_eq!(benchmark_fixture_ready_tail::<T>(), tail);
    assert_eq!(benchmark_fixture_ready_occupancy::<T>(), 0);
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_mixed_scan(n: Linear<1, 10_000>) {
    let bounded = n.min(T::MaxQueueLength::get());
    let head = ActorReadyTail::<T>::get();
    let tail = head
      .checked_add(u64::from(bounded))
      .expect("bounded fixture span");
    for offset in 0..bounded {
      let ticket = head + u64::from(offset);
      if offset % 2 == 1 {
        let actor_id = bench_create_system_manual::<T>(39_000_000u32.saturating_add(offset));
        assert!(benchmark_fixture_ready_enqueue::<T>(actor_id));
      } else {
        let page_id = ticket / 32;
        if !ActorReadyFrameChunks::<T>::contains_key(page_id) {
          ActorReadyFrameChunks::<T>::insert(
            page_id,
            ActorControlChunkOf::<T>::try_from(vec![None; 32]).expect("fixed Ready chunk"),
          );
        }
        ActorReadyTail::<T>::put(ticket + 1);
      }
    }
    assert_eq!(ActorReadyTail::<T>::get(), tail);
    #[block]
    {
      while benchmark_fixture_ready_head::<T>() < tail {
        core::hint::black_box(
          benchmark_fixture_ready_drain_tombstones::<T>(tail, bounded)
            .expect("benchmark mixed queue topology is valid"),
        );
        if let Some((_, entry)) = benchmark_fixture_ready_head_entry::<T>() {
          assert!(benchmark_fixture_ready_consume_head::<T>(entry.ticket));
        }
      }
    }
    assert_eq!(benchmark_fixture_ready_head::<T>(), tail);
    assert_eq!(benchmark_fixture_ready_tail::<T>(), tail);
    assert_eq!(benchmark_fixture_ready_occupancy::<T>(), 0);
  }

  fn prepare_zero_step_opening<T: Config>(actor_type: ActorType) -> ActorId {
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    let owner: T::AccountId = account("zero-step-opening-owner", 0, 0);
    ensure_creation_balance::<T>(&owner);
    let steps = ContractSteps::<T>::default();
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 0,
    };
    if actor_type == ActorType::User {
      prefund_active_user_creation::<T>(&owner, &steps);
      Pallet::<T>::create_user_actor(
        RawOrigin::Signed(owner.clone()).into(),
        Mutability::Mutable,
        user_contract::<T>(schedule, steps),
      )
      .expect("zero-Step User Contract is admitted");
    } else {
      Pallet::<T>::create_system_actor(
        RawOrigin::Root.into(),
        owner.clone(),
        Mutability::Mutable,
        system_contract::<T>(schedule, steps),
      )
      .expect("zero-Step System Contract is admitted");
    }
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let identity = benchmark_fixture_identity::<T>(actor_id).expect("admitted identity exists");
    if actor_type == ActorType::User {
      let fee = full_attempt_fee::<T>(&ContractSteps::<T>::default())
        .checked_add(
          &Pallet::<T>::trigger_fee_for_weight(
            ActorType::User,
            TriggerFamily::Manual,
            T::WeightInfo::manual_trigger(),
          )
          .trigger_fee,
        )
        .expect("User readiness and Pipeline fees fit");
      T::AssetOps::mint(&identity.sovereign_account, T::FeeNativeAssetId::get(), fee)
        .expect("User readiness and Pipeline have funding");
    }
    Pallet::<T>::manual_trigger(RawOrigin::Signed(owner).into(), actor_id)
      .expect("real Manual occurrence publishes zero-Step readiness");
    let (_, cell) = Pallet::<T>::actor_control_cell(actor_id).expect("zero-Step Ready cell exists");
    assert!(cell.hot.pending_signal);
    frame_system::Pallet::<T>::set_block_number(
      cell.eligible_at.expect("zero-Step eligibility exists"),
    );
    actor_id
  }

  fn consume_zero_step_opening<T: Config>(
    actor_id: ActorId,
  ) -> (ActiveActorStateOf<T>, ActorAdmissionCertificateOf<T>) {
    let (state, admission, step) =
      Pallet::<T>::load_frame_actor_service_state(actor_id).expect("zero-Step frame is coherent");
    assert!(step.is_none() && state.contract.steps.is_empty());
    Pallet::<T>::remove_primary_control_cell_inner(actor_id)
      .expect("zero-Step primary is consumed outside the inner measurement");
    (state, admission)
  }

  fn execute_zero_step_inner<T: Config>(
    actor_id: ActorId,
    state: ActiveActorStateOf<T>,
    admission: &ActorAdmissionCertificateOf<T>,
    now: BlockNumberFor<T>,
  ) {
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      match Pallet::<T>::execute_zero_step_from_consumed_fixture(actor_id, state, admission, now) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
          (),
          AttemptTransactionError,
        >(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
    .expect("zero-Step inner atom commits");
  }

  fn assert_zero_step_completion<T: Config>(
    actor_id: ActorId,
    actor_type: ActorType,
    fee: T::Balance,
  ) {
    let identity = benchmark_fixture_identity::<T>(actor_id).expect("zero-Step identity survives");
    assert_eq!(identity.cycle_nonce, 1);
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    assert!(benchmark_fixture_hot::<T>(actor_id).is_some_and(|hot| {
      hot.cycle_state == CycleState::Idle
        && !hot.pending_signal
        && hot.queue_ticket.is_none()
        && hot.wakeup_pointer.is_none()
    }));
    let receipt: <T as frame_system::Config>::RuntimeEvent =
      Event::<T>::PipelineFeeCharged { actor_id, fee }.into();
    assert_eq!(
      frame_system::Pallet::<T>::events()
        .iter()
        .filter(|record| record.event == receipt)
        .count(),
      usize::from(actor_type == ActorType::User)
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("complete zero-Step state and User holds are coherent");
  }

  /// Persistent System zero-Step Opening after a real Manual occurrence and source consumption.
  /// This complete System inner owner excludes User Pipeline collection and hold settlement.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_complete() {
    let actor_id = prepare_zero_step_opening::<T>(ActorType::System);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_zero_step_completion::<T>(actor_id, ActorType::System, Zero::zero());
  }

  /// Complete User zero-Step inner owner, including Pipeline collection/receipt and state holds.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_complete() {
    let actor_id = prepare_zero_step_opening::<T>(ActorType::User);
    let hold_before = ActorStateHolds::<T>::get(actor_id)
      .expect("zero-Step User state hold exists")
      .encode();
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let native = T::FeeNativeAssetId::get();
    let sovereign = state.identity.sovereign_account.clone();
    let sink = T::FeeSink::get();
    let payer_before = T::AssetOps::balance(&sovereign, native);
    let sink_before = T::AssetOps::balance(&sink, native);
    let fee = Pallet::<T>::pipeline_fee_for_actor(actor_id, ActorType::User)
      .expect("certified Pipeline fee exists")
      .total_fee;
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_eq!(T::AssetOps::balance(&sovereign, native), payer_before - fee);
    assert_eq!(T::AssetOps::balance(&sink, native), sink_before + fee);
    assert_zero_step_completion::<T>(actor_id, ActorType::User, fee);
    assert_eq!(
      ActorStateHolds::<T>::get(actor_id).map(|hold| hold.encode()),
      Some(hold_before),
      "persistent zero-Step Opening preserves the installed state hold"
    );
  }

  fn create_zero_step_system_follower<T: Config>() -> ActorId {
    let owner: T::AccountId = account("crossing-unavailable-follower", 0, 0);
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner.clone(),
      Mutability::Mutable,
      system_contract::<T>(
        Schedule {
          trigger: Trigger::manual(),
          cooldown_blocks: 0,
        },
        ContractSteps::<T>::default(),
      ),
    )
    .unwrap();
    let follower = NextActorId::<T>::get().saturating_sub(1);
    Pallet::<T>::manual_trigger(RawOrigin::Signed(owner).into(), follower).unwrap();
    frame_system::Pallet::<T>::set_block_number(
      Pallet::<T>::actor_control_cell(follower)
        .unwrap()
        .1
        .eligible_at
        .unwrap(),
    );
    follower
  }

  /// Full FIFO refusal when paid Crossing readiness loses its required authoritative value.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_zero_step_user_crossing_unavailable() {
    let (actor_id, feed) =
      prepare_zero_step_observation::<T>(Some(CrossingPhase::WaitingForRearm), false);
    let follower = create_zero_step_system_follower::<T>();
    T::BenchmarkHelper::deactivate_observation_feed(feed).unwrap();
    assert!(matches!(
      T::ObservationProvider::current(&feed),
      CanonicalObservationState::Unavailable
    ));
    let state = Pallet::<T>::active_actor_state(actor_id).unwrap();
    let encoded_state = state.encode();
    let head = ActorReadyHead::<T>::get();
    let events = frame_system::Pallet::<T>::events().encode();
    let membership = CrossingMemberships::<T>::get(actor_id);
    let holds = ActorStateHolds::<T>::get(actor_id);
    let before = user_pipeline_accounting::<T>(actor_id);
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("host unavailability is not Actor topology corruption");
    #[block]
    {
      let pass = Pallet::<T>::execute_cycle(Weight::MAX);
      assert!(!pass.consumed.is_zero());
      assert!(pass.effect_consumed.is_zero() && pass.starved);
    }
    assert_eq!(
      Pallet::<T>::active_actor_state(actor_id).unwrap().encode(),
      encoded_state
    );
    assert_eq!(ActorReadyHead::<T>::get(), head);
    assert_eq!(ActorReadyOccupancy::<T>::get(), 2);
    assert_eq!(
      benchmark_fixture_identity::<T>(follower)
        .unwrap()
        .cycle_nonce,
      0
    );
    assert_eq!(CrossingMemberships::<T>::get(actor_id), membership);
    assert_eq!(ActorStateHolds::<T>::get(actor_id), holds);
    assert_eq!(frame_system::Pallet::<T>::events().encode(), events);
    let native = T::FeeNativeAssetId::get();
    assert_eq!(
      T::AssetOps::balance(&before.payer, native),
      before.payer_balance
    );
    assert_eq!(
      T::AssetOps::balance(&T::FeeSink::get(), native),
      before.sink_balance
    );
    Pallet::<T>::close_actor(RawOrigin::Signed(state.identity.owner).into(), actor_id).unwrap();
    Pallet::<T>::execute_cycle(Weight::MAX);
    assert!(benchmark_fixture_identity::<T>(actor_id).is_none());
    assert!(!ActorStateHolds::<T>::contains_key(actor_id));
    assert_eq!(
      benchmark_fixture_identity::<T>(follower)
        .unwrap()
        .cycle_nonce,
      1
    );
    assert_eq!(ActorReadyHead::<T>::get(), ActorReadyTail::<T>::get());
    assert_eq!(
      T::AssetOps::balance(&before.payer, native),
      before.payer_balance
    );
    assert_eq!(
      T::AssetOps::balance(&T::FeeSink::get(), native),
      before.sink_balance
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("authorized Mutable close releases the unavailable head");
  }

  fn prepare_zero_step_user_insolvency<T: Config>(
    crossing: bool,
  ) -> (ActorId, UserPipelineAccounting<T>) {
    prepare_user_pipeline_insolvency::<T>(crossing, ContractSteps::<T>::default(), false)
  }

  fn prepare_user_pipeline_insolvency<T: Config>(
    crossing: bool,
    steps: ContractSteps<T>,
    with_window: bool,
  ) -> (ActorId, UserPipelineAccounting<T>) {
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    assert_eq!(ActorReadyOccupancy::<T>::get(), 0);
    let feed = crossing.then(|| {
      T::BenchmarkHelper::setup_observation_feeds(1)
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
    });
    let (trigger, family, weight) = match feed {
      Some(feed) => {
        assert!(matches!(
          T::ObservationProvider::current(&feed),
          CanonicalObservationState::Available { value: 1, .. }
        ));
        (
          Trigger::observation_crossing(feed, CrossingDirection::Rising, 2, 1),
          TriggerFamily::ObservationCrossing,
          T::WeightInfo::observation_crossing_trigger_occurrence(),
        )
      }
      None => (
        Trigger::manual(),
        TriggerFamily::Manual,
        T::WeightInfo::manual_trigger(),
      ),
    };
    let owner: T::AccountId = account("zero-step-insolvent-owner", 0, 0);
    ensure_creation_balance::<T>(&owner);
    let required = Pallet::<T>::user_pipeline_machine_capacity_requirement(&steps).unwrap();
    prefund_active_user_creation::<T>(&owner, &steps);
    let mut contract = user_contract::<T>(
      Schedule {
        trigger,
        cooldown_blocks: 0,
      },
      steps,
    )
    .unwrap();
    if with_window {
      let start = frame_system::Pallet::<T>::block_number();
      contract.window = Some(ScheduleWindow {
        start,
        end: start.checked_add(&T::MinWindowLength::get()).unwrap(),
      });
    }
    Pallet::<T>::create_user_actor(
      RawOrigin::Signed(owner.clone()).into(),
      Mutability::Mutable,
      Some(contract),
    )
    .expect("User Contract is ordinarily admitted before its paid Trigger");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let payer = benchmark_fixture_identity::<T>(actor_id)
      .unwrap()
      .sovereign_account;
    let native = T::FeeNativeAssetId::get();
    assert_eq!(T::AssetOps::balance(&payer, native), required);
    let trigger_fee =
      Pallet::<T>::trigger_fee_for_weight(ActorType::User, family, weight).trigger_fee;
    assert!(!trigger_fee.is_zero());
    T::AssetOps::mint(&payer, native, trigger_fee - One::one()).unwrap();
    let sink_before = T::AssetOps::balance(&T::FeeSink::get(), native);
    match feed {
      Some(feed) => {
        frame_system::Pallet::<T>::set_block_number(2u32.into());
        assert!(publish_zero_step_observation::<T>(feed, 1_000) >= 2);
        Pallet::<T>::service_crossing_transitions(Weight::MAX);
        assert!(CrossingMemberships::<T>::contains_key(actor_id));
        assert!(IndexedTriggerDetectionDisabled::<T>::contains_key(actor_id));
      }
      None => {
        Pallet::<T>::manual_trigger(RawOrigin::Signed(owner).into(), actor_id).unwrap();
      }
    }
    let (_, cell) = Pallet::<T>::actor_control_cell(actor_id).unwrap();
    assert!(cell.hot.pending_signal);
    assert_eq!(cell.identity.cycle_nonce, 0);
    frame_system::Pallet::<T>::set_block_number(cell.eligible_at.unwrap());
    assert_eq!(ActorReadyOccupancy::<T>::get(), 1);
    assert_eq!(T::AssetOps::balance(&payer, native), required - One::one());
    assert_eq!(
      T::AssetOps::balance(&T::FeeSink::get(), native),
      sink_before + trigger_fee
    );
    assert!(!Pallet::<T>::pipeline_capacity_sufficient(actor_id, ActorType::User, &payer).unwrap());
    frame_system::Pallet::<T>::assert_has_event(
      Event::<T>::TriggerOccurrenceProcessed {
        actor_id,
        trigger_family: family,
        fee: trigger_fee,
      }
      .into(),
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("paid readiness may survive insufficient Pipeline capacity");
    (actor_id, user_pipeline_accounting::<T>(actor_id))
  }

  fn assert_user_pipeline_insolvency<T: Config>(
    actor_id: ActorId,
    before: UserPipelineAccounting<T>,
  ) {
    assert!(benchmark_fixture_identity::<T>(actor_id).is_none());
    assert!(benchmark_fixture_hot::<T>(actor_id).is_none());
    assert!(!ActorContractHeads::<T>::contains_key(actor_id));
    assert!(!ActorStateHolds::<T>::contains_key(actor_id));
    assert!(!ActorFunding::<T>::contains_key(actor_id));
    assert!(
      ActorContractTailChunks::<T>::iter_prefix(actor_id)
        .next()
        .is_none()
    );
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    assert!(!CrossingMemberships::<T>::contains_key(actor_id));
    assert!(!IndexedTriggerDetectionDisabled::<T>::contains_key(
      actor_id
    ));
    assert_eq!(ActorReadyOccupancy::<T>::get(), 0);
    assert_eq!(ActorReadyHead::<T>::get(), ActorReadyTail::<T>::get());
    let native = T::FeeNativeAssetId::get();
    assert_eq!(
      T::AssetOps::balance(&before.payer, native),
      before.payer_balance
    );
    assert_eq!(
      T::AssetOps::balance(&T::FeeSink::get(), native),
      before.sink_balance
    );
    let receipt: <T as frame_system::Config>::RuntimeEvent = Event::<T>::PipelineFeeCharged {
      actor_id,
      fee: before.fee,
    }
    .into();
    assert!(
      !frame_system::Pallet::<T>::events()
        .iter()
        .any(|record| record.event == receipt)
    );
    frame_system::Pallet::<T>::assert_has_event(
      Event::<T>::ActorClosed {
        actor_id,
        reason: CloseReason::CycleAdmissionInsufficient,
      }
      .into(),
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("Pipeline apoptosis releases all process resources");
  }

  /// Direct cleanup-owner comparison with the existing Manual one-Step apoptosis fixture.
  #[benchmark]
  fn pipeline_admission_apoptosis_crossing() {
    let (actor_id, before) = prepare_zero_step_user_insolvency::<T>(true);
    let instance = Pallet::<T>::active_actor_view(actor_id).unwrap();
    let cutoff = ActorReadyTail::<T>::get();
    #[block]
    {
      Pallet::<T>::finalize_actor(actor_id, &instance, CloseReason::CycleAdmissionInsufficient)
        .unwrap();
    }
    assert_eq!(ActorReadyOccupancy::<T>::get(), 0);
    assert!(ActorReadyHead::<T>::get() < cutoff);
    benchmark_fixture_ready_drain_tombstones::<T>(cutoff, 1)
      .expect("outer FIFO retires the closed primary's tombstone after cleanup measurement");
    assert_user_pipeline_insolvency::<T>(actor_id, before);
  }

  /// Complete FIFO liability check and Manual User apoptosis one balance unit below admission.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_zero_step_user_insolvency() {
    let (actor_id, before) = prepare_zero_step_user_insolvency::<T>(false);
    #[block]
    {
      let pass = Pallet::<T>::execute_cycle(Weight::MAX);
      assert!(pass.effect_consumed.is_zero() && !pass.starved);
    }
    assert_user_pipeline_insolvency::<T>(actor_id, before);
  }

  /// The same paid-readiness insolvency with retained Crossing cleanup inside the FIFO atom.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_zero_step_user_crossing_insolvency() {
    let (actor_id, before) = prepare_zero_step_user_insolvency::<T>(true);
    #[block]
    {
      let pass = Pallet::<T>::execute_cycle(Weight::MAX);
      assert!(pass.effect_consumed.is_zero() && !pass.starved);
    }
    assert_user_pipeline_insolvency::<T>(actor_id, before);
  }

  fn prepare_max_contract_user_insolvency<T: Config>(
    with_window: bool,
  ) -> (
    ActorId,
    UserPipelineAccounting<T>,
    Option<WakeupPointer<BlockNumberFor<T>>>,
  ) {
    let steps = make_max_contract_steps::<T>(account("insolvent-contract-recipient", 0, 0));
    let (actor_id, before) = prepare_user_pipeline_insolvency::<T>(true, steps, with_window);
    assert_max_contract_geometry::<T>(actor_id);
    let hot = benchmark_fixture_hot::<T>(actor_id).unwrap();
    let pointer = hot.wakeup_pointer;
    assert_eq!(pointer.is_some(), with_window);
    if let Some(pointer) = pointer {
      let WakeupKey::Block(at) = pointer.block else {
        panic!("window expiry has a Block reference")
      };
      assert!(at > frame_system::Pallet::<T>::block_number());
      assert!(Pallet::<T>::wakeup_page_entry_matches(pointer, actor_id));
      assert_eq!(ActorWaitingOccupancies::<T>::get(pointer.block), 1);
    }
    (actor_id, before, pointer)
  }

  /// Full FIFO insolvency at the host Step-count maximum, including retained Crossing cleanup.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_user_crossing_insolvency_max_contract() {
    let (actor_id, before, _) = prepare_max_contract_user_insolvency::<T>(false);
    #[block]
    {
      let pass = Pallet::<T>::execute_cycle(Weight::MAX);
      assert!(pass.effect_consumed.is_zero() && !pass.starved);
    }
    assert_user_pipeline_insolvency::<T>(actor_id, before);
  }

  /// The same maximum-Step Contract also retains one future window reference before apoptosis.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_user_crossing_insolvency_max_contract_window() {
    let (actor_id, before, pointer) = prepare_max_contract_user_insolvency::<T>(true);
    let pointer = pointer.unwrap();
    #[block]
    {
      let pass = Pallet::<T>::execute_cycle(Weight::MAX);
      assert!(pass.effect_consumed.is_zero() && !pass.starved);
    }
    assert_user_pipeline_insolvency::<T>(actor_id, before);
    assert!(!Pallet::<T>::wakeup_page_entry_matches(pointer, actor_id));
    assert!(!ActorWaitingHeads::<T>::contains_key(pointer.block));
    assert!(!ActorWaitingTails::<T>::contains_key(pointer.block));
    assert!(!ActorWaitingCursorIndices::<T>::contains_key(pointer.block));
  }

  /// Complete single-key Block source consumption and expiry closure, excluding clock selection.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_zero_step_user_expiry() {
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    let owner: T::AccountId = account("zero-step-window-owner", 0, 0);
    ensure_creation_balance::<T>(&owner);
    let steps = ContractSteps::<T>::default();
    prefund_active_user_creation::<T>(&owner, &steps);
    let start = frame_system::Pallet::<T>::block_number();
    let end = start.saturating_add(T::MinWindowLength::get());
    let mut contract = user_contract::<T>(
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 0,
      },
      steps,
    )
    .unwrap();
    contract.window = Some(ScheduleWindow { start, end });
    Pallet::<T>::create_user_actor(
      RawOrigin::Signed(owner).into(),
      Mutability::Immutable,
      Some(contract),
    )
    .expect("Immutable zero-Step User window is ordinarily admitted");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let identity = benchmark_fixture_identity::<T>(actor_id).unwrap();
    assert_eq!(identity.cycle_nonce, 0);
    let hot = benchmark_fixture_hot::<T>(actor_id).unwrap();
    assert!(!hot.pending_signal && hot.queue_ticket.is_none());
    let expired = end.saturating_add(1u32.into());
    let key = WakeupKey::Block(expired);
    assert_eq!(hot.wakeup_pointer.unwrap().block, key);
    assert_eq!(ActorWaitingOccupancies::<T>::get(key), 1);
    assert!(ActorStateHolds::<T>::contains_key(actor_id));
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    frame_system::Pallet::<T>::set_block_number(end);
    T::BenchmarkHelper::advance_to_scheduler_tick(expired.saturated_into()).unwrap();
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("ordinary expired window and host deadlines are coherent");
    let native = T::FeeNativeAssetId::get();
    let payer_before = T::AssetOps::balance(&identity.sovereign_account, native);
    let sink = T::FeeSink::get();
    let sink_before = T::AssetOps::balance(&sink, native);
    let fee = Pallet::<T>::pipeline_fee_for_actor(actor_id, ActorType::User)
      .unwrap()
      .total_fee;
    assert!(!fee.is_zero());
    #[block]
    {
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        let (due, stats) = Pallet::<T>::wakeup_substrate_drain_key(key, 1);
        assert_eq!(stats.entries_scanned, 1);
        assert_eq!(due.len(), 1);
        let (due_actor, state, admission, loaded_step) = due.into_iter().next().unwrap();
        assert_eq!(due_actor, actor_id);
        assert!(loaded_step.is_none());
        let view = Pallet::<T>::derive_active_actor_view(
          state.identity.clone(),
          state.hot.clone(),
          state.contract.clone(),
        );
        assert!(
          Pallet::<T>::expiry_substitution_due_loaded(&view, state.run_state.as_ref()).unwrap()
        );
        Pallet::<T>::finalize_actor_from_consumed_state(
          actor_id,
          state,
          &admission,
          CloseReason::WindowExpired,
        )
        .unwrap();
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Ok::<(), DispatchError>(()),
        )
      })
      .unwrap();
    }
    assert!(benchmark_fixture_identity::<T>(actor_id).is_none());
    assert!(benchmark_fixture_hot::<T>(actor_id).is_none());
    assert!(!ActorContractHeads::<T>::contains_key(actor_id));
    assert!(!ActorStateHolds::<T>::contains_key(actor_id));
    assert!(!ActorFunding::<T>::contains_key(actor_id));
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    assert!(!ActorWaitingOccupancies::<T>::contains_key(key));
    assert_eq!(
      T::AssetOps::balance(&identity.sovereign_account, native),
      payer_before
    );
    assert_eq!(T::AssetOps::balance(&sink, native), sink_before);
    let receipt: <T as frame_system::Config>::RuntimeEvent =
      Event::<T>::PipelineFeeCharged { actor_id, fee }.into();
    assert!(
      !frame_system::Pallet::<T>::events()
        .iter()
        .any(|record| record.event == receipt)
    );
    frame_system::Pallet::<T>::assert_has_event(
      Event::<T>::ActorClosed {
        actor_id,
        reason: CloseReason::WindowExpired,
      }
      .into(),
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("window expiry releases its source and process resources");
  }

  fn prepare_zero_step_cadenced<T: Config>(close: bool) -> ActorId {
    prepare_zero_step_temporal::<T>(Trigger::cadenced(1), Mutability::Mutable, close)
  }

  fn prepare_zero_step_temporal<T: Config>(
    trigger: TriggerOf<T>,
    mutability: Mutability,
    close: bool,
  ) -> ActorId {
    let family = trigger.family();
    let occurrence_weight = match family {
      TriggerFamily::AtTime => T::WeightInfo::at_time_trigger_occurrence(),
      TriggerFamily::Cadenced => T::WeightInfo::cadenced_trigger_occurrence(),
      _ => panic!("temporal profile requires AtTime or Cadenced"),
    };
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    let owner: T::AccountId = account("zero-step-cadenced-owner", 0, 0);
    ensure_creation_balance::<T>(&owner);
    let steps = ContractSteps::<T>::default();
    prefund_active_user_creation::<T>(&owner, &steps);
    let mut contract = user_contract::<T>(
      Schedule {
        trigger,
        cooldown_blocks: 0,
      },
      steps.clone(),
    )
    .unwrap();
    contract.auto_close_at_cycle_nonce = close.then_some(1);
    Pallet::<T>::create_user_actor(RawOrigin::Signed(owner).into(), mutability, Some(contract))
      .expect("zero-Step temporal User Contract is admitted");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let identity = benchmark_fixture_identity::<T>(actor_id).unwrap();
    let extra = full_attempt_fee::<T>(&steps)
      .checked_add(
        &Pallet::<T>::trigger_fee_for_weight(ActorType::User, family, occurrence_weight)
          .trigger_fee,
      )
      .unwrap();
    T::AssetOps::mint(
      &identity.sovereign_account,
      T::FeeNativeAssetId::get(),
      extra,
    )
    .unwrap();
    let tick = benchmark_fixture_hot::<T>(actor_id)
      .unwrap()
      .trigger_wakeup_pointer
      .expect("creation installed a temporal primary")
      .tick;
    T::BenchmarkHelper::advance_to_scheduler_tick(tick).expect("host clock reaches the due tick");
    assert!(Pallet::<T>::current_scheduler_tick().unwrap() >= tick);
    let now = frame_system::Pallet::<T>::block_number();
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    Pallet::<T>::drain_overdue_wakeups_cursor(now, &mut meter);
    let (_, cell) = Pallet::<T>::actor_control_cell(actor_id).expect("due Actor is Ready");
    assert!(cell.hot.pending_signal && cell.hot.trigger_wakeup_pointer.is_none());
    frame_system::Pallet::<T>::set_block_number(
      cell.eligible_at.expect("Ready eligibility exists"),
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("ordinary temporal readiness and holds are coherent");
    actor_id
  }

  /// Complete Mutable User AtTime Opening retains a consumed process with no recurring deadline.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_at_time() {
    let actor_id = prepare_zero_step_temporal::<T>(Trigger::at_time(1), Mutability::Mutable, false);
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    let hot = benchmark_fixture_hot::<T>(actor_id).unwrap();
    assert_eq!(hot.cycle_state, CycleState::Idle);
    assert!(matches!(
      hot.trigger_runtime_state,
      TriggerRuntimeState::AtTime { consumed: true, .. }
    ));
    assert!(!hot.pending_signal && hot.queue_ticket.is_none());
    assert!(hot.trigger_wakeup_pointer.is_none() && hot.wakeup_pointer.is_none());
    assert_eq!(
      benchmark_fixture_identity::<T>(actor_id)
        .unwrap()
        .cycle_nonce,
      1
    );
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    let payer = benchmark_fixture_identity::<T>(actor_id)
      .unwrap()
      .sovereign_account;
    let balance = T::AssetOps::balance(&payer, T::FeeNativeAssetId::get());
    let next_tick = Pallet::<T>::current_scheduler_tick()
      .unwrap()
      .saturating_add(1);
    T::BenchmarkHelper::finalize_scheduler_clock().unwrap();
    T::BenchmarkHelper::advance_to_scheduler_tick(next_tick).unwrap();
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    Pallet::<T>::drain_overdue_wakeups_cursor(
      frame_system::Pallet::<T>::block_number(),
      &mut meter,
    );
    assert_eq!(benchmark_fixture_hot::<T>(actor_id).unwrap(), hot);
    assert_eq!(
      benchmark_fixture_identity::<T>(actor_id)
        .unwrap()
        .cycle_nonce,
      1
    );
    assert_eq!(
      T::AssetOps::balance(&payer, T::FeeNativeAssetId::get()),
      balance
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("consumed Mutable AtTime has no recurring activation");
  }

  /// Complete Immutable User AtTime Opening obeys the mandatory authored nonce-one close.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_at_time_close() {
    let actor_id =
      prepare_zero_step_temporal::<T>(Trigger::at_time(1), Mutability::Immutable, true);
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert!(benchmark_fixture_identity::<T>(actor_id).is_none());
    assert!(!ActorContractHeads::<T>::contains_key(actor_id));
    assert!(!ActorStateHolds::<T>::contains_key(actor_id));
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    frame_system::Pallet::<T>::assert_has_event(
      Event::<T>::ActorClosed {
        actor_id,
        reason: CloseReason::AutoCloseNonceReached,
      }
      .into(),
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state()
      .expect("authored Immutable AtTime close releases all process resources");
  }

  fn assert_zero_step_cadenced<T: Config>(actor_id: ActorId, close: bool, fee: T::Balance) {
    assert!(
      !fee.is_zero(),
      "User collection profile requires a nonzero fee"
    );
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    if close {
      assert!(benchmark_fixture_identity::<T>(actor_id).is_none());
      assert!(!ActorContractHeads::<T>::contains_key(actor_id));
      assert!(!ActorStateHolds::<T>::contains_key(actor_id));
      frame_system::Pallet::<T>::assert_has_event(
        Event::<T>::ActorClosed {
          actor_id,
          reason: CloseReason::AutoCloseNonceReached,
        }
        .into(),
      );
    } else {
      assert_eq!(
        benchmark_fixture_identity::<T>(actor_id)
          .unwrap()
          .cycle_nonce,
        1
      );
      let hot = benchmark_fixture_hot::<T>(actor_id).expect("Cadenced process survives");
      assert_eq!(hot.cycle_state, CycleState::Idle);
      assert!(!hot.pending_signal && hot.queue_ticket.is_none());
      let pointer = hot
        .trigger_wakeup_pointer
        .expect("Cadenced successor has a temporal pointer");
      assert!(pointer.tick > Pallet::<T>::current_scheduler_tick().unwrap());
      assert!(Pallet::<T>::wakeup_page_entry_matches(
        WakeupPointer {
          block: WakeupKey::Tick(pointer.tick),
          page_id: pointer.page_id,
          slot: pointer.slot,
        },
        actor_id
      ));
    }
    let receipt: <T as frame_system::Config>::RuntimeEvent =
      Event::<T>::PipelineFeeCharged { actor_id, fee }.into();
    assert_eq!(
      frame_system::Pallet::<T>::events()
        .iter()
        .filter(|record| record.event == receipt)
        .count(),
      1
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("Cadenced completion/close and holds are coherent");
  }

  /// Complete User zero-Step inner owner with ordinary due readiness and Tick rearm.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_cadenced() {
    let actor_id = prepare_zero_step_cadenced::<T>(false);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let native = T::FeeNativeAssetId::get();
    let sovereign = state.identity.sovereign_account.clone();
    let sink = T::FeeSink::get();
    let payer_before = T::AssetOps::balance(&sovereign, native);
    let sink_before = T::AssetOps::balance(&sink, native);
    let fee = Pallet::<T>::pipeline_fee_for_actor(actor_id, ActorType::User)
      .unwrap()
      .total_fee;
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_eq!(T::AssetOps::balance(&sovereign, native), payer_before - fee);
    assert_eq!(T::AssetOps::balance(&sink, native), sink_before + fee);
    assert_zero_step_cadenced::<T>(actor_id, false, fee);
  }

  /// Inclusive authored close: Cadenced rearm, Pipeline collection and custody-neutral cleanup.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_cadenced_close() {
    let actor_id = prepare_zero_step_cadenced::<T>(true);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let native = T::FeeNativeAssetId::get();
    let sovereign = state.identity.sovereign_account.clone();
    let sink = T::FeeSink::get();
    let payer_before = T::AssetOps::balance(&sovereign, native);
    let sink_before = T::AssetOps::balance(&sink, native);
    let fee = Pallet::<T>::pipeline_fee_for_actor(actor_id, ActorType::User)
      .unwrap()
      .total_fee;
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_eq!(T::AssetOps::balance(&sovereign, native), payer_before - fee);
    assert_eq!(T::AssetOps::balance(&sink, native), sink_before + fee);
    assert_zero_step_cadenced::<T>(actor_id, true, fee);
  }

  struct UserPipelineAccounting<T: Config> {
    payer: T::AccountId,
    payer_balance: T::Balance,
    sink_balance: T::Balance,
    fee: T::Balance,
  }

  fn user_pipeline_accounting<T: Config>(actor_id: ActorId) -> UserPipelineAccounting<T> {
    let payer = benchmark_fixture_identity::<T>(actor_id)
      .unwrap()
      .sovereign_account;
    let native = T::FeeNativeAssetId::get();
    let fee = Pallet::<T>::pipeline_fee_for_actor(actor_id, ActorType::User)
      .unwrap()
      .total_fee;
    assert!(
      !fee.is_zero(),
      "User profile requires a nonzero Pipeline fee"
    );
    UserPipelineAccounting {
      payer_balance: T::AssetOps::balance(&payer, native),
      sink_balance: T::AssetOps::balance(&T::FeeSink::get(), native),
      payer,
      fee,
    }
  }

  fn assert_user_pipeline_accounting<T: Config>(
    actor_id: ActorId,
    before: UserPipelineAccounting<T>,
  ) {
    let native = T::FeeNativeAssetId::get();
    assert_eq!(
      T::AssetOps::balance(&before.payer, native),
      before.payer_balance - before.fee
    );
    assert_eq!(
      T::AssetOps::balance(&T::FeeSink::get(), native),
      before.sink_balance + before.fee
    );
    let receipt: <T as frame_system::Config>::RuntimeEvent = Event::<T>::PipelineFeeCharged {
      actor_id,
      fee: before.fee,
    }
    .into();
    assert_eq!(
      frame_system::Pallet::<T>::events()
        .iter()
        .filter(|record| record.event == receipt)
        .count(),
      1
    );
  }

  fn publish_zero_step_observation<T: Config>(feed: T::ObservationFeedId, sample: u128) -> u128 {
    let CanonicalObservationState::Available {
      value: previous,
      revision,
    } = T::ObservationProvider::current(&feed)
    else {
      panic!("initial observation exists")
    };
    T::BenchmarkHelper::publish_observation(feed, sample)
      .expect("host observation publication succeeds");
    let CanonicalObservationState::Available {
      value,
      revision: published_revision,
    } = T::ObservationProvider::current(&feed)
    else {
      panic!("published observation exists")
    };
    assert_eq!(
      published_revision,
      revision.checked_add(u64::from(value != previous)).unwrap()
    );
    value
  }

  fn publish_observation_at_most<T: Config>(feed: T::ObservationFeedId, ceiling: u128) {
    let CanonicalObservationState::Available {
      value: mut current, ..
    } = T::ObservationProvider::current(&feed)
    else {
      panic!("initial observation exists")
    };
    for _ in 0..T::MaxCrossingTransitionsPerBlock::get() {
      if current <= ceiling {
        return;
      }
      let next = publish_zero_step_observation::<T>(feed, ceiling);
      assert!(
        next < current,
        "host aggregation approaches the rearm boundary"
      );
      current = next;
    }
    assert!(
      current <= ceiling,
      "bounded real publications reach the rearm boundary"
    );
  }

  fn prepare_zero_step_observation<T: Config>(
    crossing_phase: Option<CrossingPhase>,
    close: bool,
  ) -> (ActorId, T::ObservationFeedId) {
    prepare_zero_step_observation_at_value::<T>(crossing_phase, close, 2, 1)
  }

  fn create_zero_step_observation_user<T: Config>(
    feed: T::ObservationFeedId,
    crossing_phase: Option<CrossingPhase>,
    close: bool,
    threshold: u128,
  ) -> (ActorId, T::AccountId) {
    let owner: T::AccountId = account("zero-step-observation-owner", 0, 0);
    ensure_creation_balance::<T>(&owner);
    let steps = ContractSteps::<T>::default();
    prefund_active_user_creation::<T>(&owner, &steps);
    let (trigger, family, trigger_weight) = if crossing_phase.is_some() {
      (
        Trigger::observation_crossing(feed, CrossingDirection::Rising, threshold, 1),
        TriggerFamily::ObservationCrossing,
        T::WeightInfo::observation_crossing_trigger_occurrence(),
      )
    } else {
      (
        Trigger::observation_change(feed),
        TriggerFamily::ObservationChange,
        T::WeightInfo::observation_change_trigger_occurrence(),
      )
    };
    let mut contract = user_contract::<T>(
      Schedule {
        trigger,
        cooldown_blocks: 0,
      },
      steps.clone(),
    )
    .unwrap();
    contract.auto_close_at_cycle_nonce = close.then_some(1);
    Pallet::<T>::create_user_actor(
      RawOrigin::Signed(owner).into(),
      Mutability::Mutable,
      Some(contract),
    )
    .expect("zero-Step observation User Contract is admitted");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let payer = benchmark_fixture_identity::<T>(actor_id)
      .unwrap()
      .sovereign_account;
    let extra = full_attempt_fee::<T>(&steps)
      .checked_add(
        &Pallet::<T>::trigger_fee_for_weight(ActorType::User, family, trigger_weight).trigger_fee,
      )
      .unwrap();
    T::AssetOps::mint(&payer, T::FeeNativeAssetId::get(), extra).unwrap();
    (actor_id, payer)
  }

  fn prepare_zero_step_observation_at_value<T: Config>(
    crossing_phase: Option<CrossingPhase>,
    close: bool,
    threshold: u128,
    rearm_value: u128,
  ) -> (ActorId, T::ObservationFeedId) {
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    let feed = T::BenchmarkHelper::setup_observation_feeds(1)
      .unwrap()
      .into_iter()
      .next()
      .unwrap();
    assert!(matches!(
      T::ObservationProvider::current(&feed),
      CanonicalObservationState::Available { value: 1, .. }
    ));
    let (actor_id, payer) =
      create_zero_step_observation_user::<T>(feed, crossing_phase, close, threshold);
    frame_system::Pallet::<T>::set_block_number(2u32.into());
    const CROSSING_SAMPLE: u128 = 1_000;
    let mut current = publish_zero_step_observation::<T>(feed, CROSSING_SAMPLE);
    assert!(
      (threshold..=CROSSING_SAMPLE).contains(&current),
      "host aggregation crosses the authored threshold"
    );
    if crossing_phase.is_some() {
      Pallet::<T>::service_crossing_transitions(Weight::MAX);
    } else {
      Pallet::<T>::do_fanout_dirty_observation_page()
        .expect("published observation reaches its subscriber");
    }
    assert!(benchmark_fixture_hot::<T>(actor_id).unwrap().pending_signal);
    assert!(IndexedTriggerDetectionDisabled::<T>::contains_key(actor_id));
    if crossing_phase == Some(CrossingPhase::Armed) {
      let balance = T::AssetOps::balance(&payer, T::FeeNativeAssetId::get());
      frame_system::Pallet::<T>::set_block_number(
        frame_system::Pallet::<T>::block_number().saturating_add(1u32.into()),
      );
      for _ in 0..T::MaxCrossingTransitionsPerBlock::get() {
        if current <= rearm_value {
          break;
        }
        let next = publish_zero_step_observation::<T>(feed, rearm_value);
        assert!(
          next < current,
          "host aggregation approaches the qualified Opening rearm value"
        );
        current = next;
      }
      assert_eq!(
        current, rearm_value,
        "real publication reaches the qualified Opening rearm value"
      );
      Pallet::<T>::service_crossing_transitions(Weight::MAX);
      assert_eq!(
        T::AssetOps::balance(&payer, T::FeeNativeAssetId::get()),
        balance
      );
      assert!(IndexedTriggerDetectionDisabled::<T>::contains_key(actor_id));
    }
    let (_, cell) = Pallet::<T>::actor_control_cell(actor_id)
      .expect("published observation creates Ready primary");
    assert!(cell.hot.pending_signal);
    frame_system::Pallet::<T>::set_block_number(
      cell.eligible_at.expect("Ready eligibility exists"),
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("published observation readiness is coherent");
    (actor_id, feed)
  }

  fn assert_zero_step_observation<T: Config>(
    actor_id: ActorId,
    feed: T::ObservationFeedId,
    crossing_phase: Option<CrossingPhase>,
    close: bool,
  ) {
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    assert!(!IndexedTriggerDetectionDisabled::<T>::contains_key(
      actor_id
    ));
    if close {
      assert!(benchmark_fixture_identity::<T>(actor_id).is_none());
      assert!(!ActorContractHeads::<T>::contains_key(actor_id));
      assert!(!ActorStateHolds::<T>::contains_key(actor_id));
      assert!(!CrossingMemberships::<T>::contains_key(actor_id));
      assert!(!ObservationSubscriptionSlot::<T>::contains_key(actor_id));
      frame_system::Pallet::<T>::assert_has_event(
        Event::<T>::ActorClosed {
          actor_id,
          reason: CloseReason::AutoCloseNonceReached,
        }
        .into(),
      );
    } else {
      assert_eq!(
        benchmark_fixture_identity::<T>(actor_id)
          .unwrap()
          .cycle_nonce,
        1
      );
      let hot = benchmark_fixture_hot::<T>(actor_id).unwrap();
      assert_eq!(hot.cycle_state, CycleState::Idle);
      assert!(!hot.pending_signal && hot.queue_ticket.is_none());
      if let Some(expected) = crossing_phase {
        let CanonicalObservationState::Available { revision, .. } =
          T::ObservationProvider::current(&feed)
        else {
          panic!("rearm retains canonical observation")
        };
        assert!(
          matches!(hot.trigger_runtime_state, TriggerRuntimeState::ObservationCrossing {
          phase, installed_at_revision,
        } if phase == expected && installed_at_revision == revision)
        );
        assert!(CrossingMemberships::<T>::contains_key(actor_id));
      } else {
        assert!(ObservationSubscriptionSlot::<T>::contains_key(actor_id));
      }
    }
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("observation zero-Step successor and holds are coherent");
  }

  fn create_zero_step_crossing_guard<T: Config>(
    index: u32,
    feed: T::ObservationFeedId,
    threshold: u128,
  ) -> ActorId {
    let owner: T::AccountId = account("zero-step-crossing-page", index, 0);
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      system_contract::<T>(
        Schedule {
          trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, threshold, 1),
          cooldown_blocks: 0,
        },
        ContractSteps::<T>::default(),
      ),
    )
    .expect("ordinary zero-Step guard is admitted");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    assert!(!benchmark_fixture_hot::<T>(actor_id).unwrap().pending_signal);
    actor_id
  }

  fn prepare_zero_step_crossing_page<T: Config>(
    close: bool,
    tail_members: u32,
  ) -> (ActorId, T::ObservationFeedId, ActorId) {
    let (actor_id, feed) =
      prepare_zero_step_observation::<T>(Some(CrossingPhase::WaitingForRearm), close);
    let target = CrossingMemberships::<T>::get(actor_id).unwrap();
    assert_eq!((target.page, target.offset), (0, 0));
    let mut tail_actor = actor_id;
    assert!((1..=T::CrossingPageSize::get()).contains(&tail_members));
    for index in 0..T::CrossingPageSize::get() + tail_members - 1 {
      tail_actor = create_zero_step_crossing_guard::<T>(index, feed, 2);
      let guard = CrossingMemberships::<T>::get(tail_actor).unwrap();
      assert_eq!(guard.key, target.key);
    }
    let leaf = CrossingLeafStates::<T>::get(target.key).unwrap();
    assert_eq!((leaf.page_count, leaf.tail_page), (2, 1));
    assert_eq!(leaf.member_count, T::CrossingPageSize::get() + tail_members);
    assert_eq!(
      CrossingMemberPages::<T>::get(target.key, 0)
        .unwrap()
        .entries
        .len() as u32,
      T::CrossingPageSize::get()
    );
    let tail = CrossingMemberships::<T>::get(tail_actor).unwrap();
    assert_eq!((tail.page, tail.offset), (1, tail_members - 1));
    assert_eq!(
      CrossingMemberPages::<T>::get(target.key, 1)
        .unwrap()
        .entries
        .len() as u32,
      tail_members
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("full source page and qualified tail are coherent");
    (actor_id, feed, tail_actor)
  }

  fn assert_zero_step_crossing_page<T: Config>(
    actor_id: ActorId,
    feed: T::ObservationFeedId,
    tail_actor: ActorId,
    close: bool,
    tail_members: u32,
  ) {
    assert_zero_step_observation::<T>(actor_id, feed, Some(CrossingPhase::WaitingForRearm), close);
    let moved = CrossingMemberships::<T>::get(tail_actor).unwrap();
    assert_eq!((moved.page, moved.offset), (0, 0));
    let page = CrossingMemberPages::<T>::get(moved.key, 0).unwrap();
    assert_eq!(page.entries.len() as u32, T::CrossingPageSize::get());
    assert_eq!(page.entries[0].actor_id, tail_actor);
    let leaf = CrossingLeafStates::<T>::get(moved.key).unwrap();
    assert_eq!(
      leaf.member_count,
      T::CrossingPageSize::get() + tail_members - u32::from(close)
    );
    assert_eq!(
      leaf.page_count,
      if close && tail_members == 1 { 1 } else { 2 }
    );
    if close && tail_members == 1 {
      assert!(!CrossingMemberPages::<T>::contains_key(moved.key, 1));
    } else if close {
      assert_eq!(
        CrossingMemberPages::<T>::get(moved.key, 1)
          .unwrap()
          .entries
          .len() as u32,
        tail_members - 1
      );
    } else {
      let target = CrossingMemberships::<T>::get(actor_id).unwrap();
      assert_eq!(target.key, moved.key);
      assert_eq!((target.page, target.offset), (1, tail_members - 1));
      let tail = CrossingMemberPages::<T>::get(target.key, 1).unwrap();
      assert_eq!(tail.entries.len() as u32, tail_members);
      assert_eq!(tail.entries.last().unwrap().actor_id, actor_id);
    }
    assert_eq!(
      benchmark_fixture_identity::<T>(tail_actor)
        .unwrap()
        .cycle_nonce,
      0
    );
    let hot = benchmark_fixture_hot::<T>(tail_actor).unwrap();
    assert!(!hot.pending_signal && hot.queue_ticket.is_none());
  }

  fn prepare_zero_step_crossing_armed_pages<T: Config>(
    close: bool,
    allocate_page: bool,
  ) -> (ActorId, T::ObservationFeedId, ActorId, ActorId) {
    let (actor_id, feed) =
      prepare_zero_step_observation_at_value::<T>(Some(CrossingPhase::Armed), close, 4, 2);
    let source = CrossingMemberships::<T>::get(actor_id).unwrap();
    assert_eq!((source.page, source.offset), (0, 0));
    let page_size = T::CrossingPageSize::get();
    assert!(page_size > 1);
    let mut moved_actor = actor_id;
    for index in 0..2 * page_size - 1 {
      moved_actor = create_zero_step_crossing_guard::<T>(index, feed, 2);
      assert_eq!(
        CrossingMemberships::<T>::get(moved_actor).unwrap().key,
        source.key
      );
    }
    let source_leaf = CrossingLeafStates::<T>::get(source.key).unwrap();
    assert_eq!(
      (source_leaf.member_count, source_leaf.page_count),
      (2 * page_size, 2)
    );
    for page in 0..2 {
      assert_eq!(
        CrossingMemberPages::<T>::get(source.key, page)
          .unwrap()
          .entries
          .len() as u32,
        page_size
      );
    }
    let moved = CrossingMemberships::<T>::get(moved_actor).unwrap();
    assert_eq!((moved.page, moved.offset), (1, page_size - 1));
    let destination_members = page_size - u32::from(!allocate_page);
    let mut destination_actor = actor_id;
    for index in 0..destination_members {
      destination_actor = create_zero_step_crossing_guard::<T>(2 * page_size + index, feed, 4);
    }
    let destination = CrossingMemberships::<T>::get(destination_actor).unwrap();
    assert_ne!(destination.key, source.key);
    let leaf = CrossingLeafStates::<T>::get(destination.key).unwrap();
    assert_eq!(
      (leaf.member_count, leaf.page_count),
      (destination_members, 1)
    );
    assert_eq!(
      (destination.page, destination.offset),
      (0, destination_members - 1)
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("ordinary guards qualify both Crossing phase leaves");
    (actor_id, feed, moved_actor, destination_actor)
  }

  fn assert_zero_step_crossing_armed_pages<T: Config>(
    actor_id: ActorId,
    feed: T::ObservationFeedId,
    moved_actor: ActorId,
    destination_actor: ActorId,
    close: bool,
    allocate_page: bool,
  ) {
    assert_zero_step_observation::<T>(actor_id, feed, Some(CrossingPhase::Armed), close);
    let page_size = T::CrossingPageSize::get();
    let moved = CrossingMemberships::<T>::get(moved_actor).unwrap();
    assert_eq!((moved.page, moved.offset), (0, 0));
    let source_leaf = CrossingLeafStates::<T>::get(moved.key).unwrap();
    assert_eq!(
      (source_leaf.member_count, source_leaf.page_count),
      (2 * page_size - 1, 2)
    );
    assert_eq!(
      CrossingMemberPages::<T>::get(moved.key, 1)
        .unwrap()
        .entries
        .len() as u32,
      page_size - 1
    );
    let destination = CrossingMemberships::<T>::get(destination_actor).unwrap();
    let initial_members = page_size - u32::from(!allocate_page);
    assert_eq!(
      (destination.page, destination.offset),
      (0, initial_members - 1)
    );
    let leaf = CrossingLeafStates::<T>::get(destination.key).unwrap();
    assert_eq!(leaf.member_count, initial_members + u32::from(!close));
    assert_eq!(leaf.page_count, if allocate_page && !close { 2 } else { 1 });
    if close {
      assert!(!CrossingMemberPages::<T>::contains_key(destination.key, 1));
    } else {
      let target = CrossingMemberships::<T>::get(actor_id).unwrap();
      assert_eq!(target.key, destination.key);
      assert_eq!(
        (target.page, target.offset),
        if allocate_page {
          (1, 0)
        } else {
          (0, page_size - 1)
        }
      );
    }
    for guard in [moved_actor, destination_actor] {
      assert_eq!(
        benchmark_fixture_identity::<T>(guard).unwrap().cycle_nonce,
        0
      );
      let hot = benchmark_fixture_hot::<T>(guard).unwrap();
      assert!(!hot.pending_signal && hot.queue_ticket.is_none());
    }
  }

  struct ZeroStepCrossingCursor<T: Config> {
    actor_id: ActorId,
    feed: T::ObservationFeedId,
    moved_actor: ActorId,
    destination_actor: ActorId,
    cursor_before: CrossingRangeCursor,
    source_members_before: u32,
  }

  fn prepare_zero_step_crossing_cursor<T: Config>(close: bool) -> ZeroStepCrossingCursor<T> {
    let (actor_id, feed, _, destination_actor) =
      prepare_zero_step_crossing_armed_pages::<T>(close, true);
    frame_system::Pallet::<T>::set_block_number(
      frame_system::Pallet::<T>::block_number().saturating_add(1u32.into()),
    );
    assert_eq!(publish_zero_step_observation::<T>(feed, 1), 1);
    let (_, counters) = Pallet::<T>::service_crossing_transitions_with_counters(Weight::MAX);
    assert_eq!(
      (counters.faults, counters.activations, counters.closes),
      (0, 0, 0)
    );
    assert!(
      counters.candidates > 1,
      "bounded worker skips the latched target and rearms guards"
    );
    let source = CrossingMemberships::<T>::get(actor_id).unwrap();
    let cursor_before =
      CrossingRangeCursors::<T>::get(feed).expect("bounded worker retains live traversal");
    assert_eq!(cursor_before.current_threshold, Some(source.key.threshold));
    assert_eq!(cursor_before.traversal, source.key.traversal);
    assert!((source.page, source.offset) < (cursor_before.page, cursor_before.offset));
    let leaf = CrossingLeafStates::<T>::get(source.key).unwrap();
    assert_eq!(
      leaf.member_count,
      2 * T::CrossingPageSize::get() - (counters.candidates - 1)
    );
    assert_eq!(leaf.page_count, 2);
    let tail = CrossingMemberPages::<T>::get(source.key, leaf.tail_page).unwrap();
    let tail_offset = tail.entries.len() as u32 - 1;
    assert!((leaf.tail_page, tail_offset) >= (cursor_before.page, cursor_before.offset));
    let moved_actor = tail.entries.last().unwrap().actor_id;
    assert_ne!(moved_actor, actor_id);
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state()
      .expect("real bounded worker leaves a coherent repairable range cursor");
    ZeroStepCrossingCursor {
      actor_id,
      feed,
      moved_actor,
      destination_actor,
      cursor_before,
      source_members_before: leaf.member_count,
    }
  }

  fn assert_zero_step_crossing_cursor<T: Config>(fixture: &ZeroStepCrossingCursor<T>, close: bool) {
    assert_zero_step_observation::<T>(
      fixture.actor_id,
      fixture.feed,
      Some(CrossingPhase::Armed),
      close,
    );
    assert_zero_step_crossing_cursor_topology::<T>(fixture, close);
  }

  fn assert_zero_step_crossing_cursor_topology<T: Config>(
    fixture: &ZeroStepCrossingCursor<T>,
    close: bool,
  ) {
    let moved = CrossingMemberships::<T>::get(fixture.moved_actor).unwrap();
    assert_eq!((moved.page, moved.offset), (0, 0));
    let mut expected_cursor = fixture.cursor_before;
    expected_cursor.page = moved.page;
    expected_cursor.offset = moved.offset;
    assert_eq!(
      CrossingRangeCursors::<T>::get(fixture.feed),
      Some(expected_cursor)
    );
    assert_eq!(
      CrossingLeafStates::<T>::get(moved.key)
        .unwrap()
        .member_count,
      fixture.source_members_before - 1
    );
    let destination = CrossingMemberships::<T>::get(fixture.destination_actor).unwrap();
    let leaf = CrossingLeafStates::<T>::get(destination.key).unwrap();
    assert_eq!(
      leaf.member_count,
      T::CrossingPageSize::get() + u32::from(!close)
    );
    assert_eq!(leaf.page_count, if close { 1 } else { 2 });
    if close {
      assert!(!CrossingMemberPages::<T>::contains_key(destination.key, 1));
    } else {
      let target = CrossingMemberships::<T>::get(fixture.actor_id).unwrap();
      assert_eq!(target.key, destination.key);
      assert_eq!((target.page, target.offset), (1, 0));
    }
    for guard in [fixture.moved_actor, fixture.destination_actor] {
      assert_eq!(
        benchmark_fixture_identity::<T>(guard).unwrap().cycle_nonce,
        0
      );
      let hot = benchmark_fixture_hot::<T>(guard).unwrap();
      assert!(!hot.pending_signal && hot.queue_ticket.is_none());
    }
    frame_system::Pallet::<T>::set_block_number(
      frame_system::Pallet::<T>::block_number().saturating_add(1u32.into()),
    );
    let (_, resumed) = Pallet::<T>::service_crossing_transitions_with_counters(Weight::MAX);
    assert_eq!(
      (resumed.faults, resumed.activations, resumed.closes),
      (0, 0, 0)
    );
    let repaired_member = CrossingMemberships::<T>::get(fixture.moved_actor).unwrap();
    assert_ne!(
      repaired_member.key, moved.key,
      "resumed traversal does not skip the moved guard"
    );
    let hot = benchmark_fixture_hot::<T>(fixture.moved_actor).unwrap();
    assert!(matches!(
      hot.trigger_runtime_state,
      TriggerRuntimeState::ObservationCrossing {
        phase: CrossingPhase::Armed,
        ..
      }
    ));
    assert!(!hot.pending_signal && hot.queue_ticket.is_none());
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state()
      .expect("resumed traversal preserves the repaired guard and all survivors");
  }

  /// Complete User Opening repairs a live range cursor after non-tail membership removal.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_crossing_cursor() {
    let fixture = prepare_zero_step_crossing_cursor::<T>(false);
    let before = user_pipeline_accounting::<T>(fixture.actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(fixture.actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(fixture.actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(fixture.actor_id, before);
    assert_zero_step_crossing_cursor::<T>(&fixture, false);
  }

  /// Complete authored close preserves the repaired live range cursor and surviving guards.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_crossing_cursor_close() {
    let fixture = prepare_zero_step_crossing_cursor::<T>(true);
    let before = user_pipeline_accounting::<T>(fixture.actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(fixture.actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(fixture.actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(fixture.actor_id, before);
    assert_zero_step_crossing_cursor::<T>(&fixture, true);
  }

  /// Complete User Armed rearm fills an existing destination page after full source/tail rewrite.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_crossing_armed_pages() {
    let (actor_id, feed, moved, destination) =
      prepare_zero_step_crossing_armed_pages::<T>(false, false);
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert_zero_step_crossing_armed_pages::<T>(actor_id, feed, moved, destination, false, false);
  }

  /// Complete User Armed rearm allocates behind a full destination page.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_crossing_armed_new_page() {
    let (actor_id, feed, moved, destination) =
      prepare_zero_step_crossing_armed_pages::<T>(false, true);
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert_zero_step_crossing_armed_pages::<T>(actor_id, feed, moved, destination, false, true);
  }

  /// Complete authored close includes Armed rearm into an existing destination page.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_crossing_armed_pages_close() {
    let (actor_id, feed, moved, destination) =
      prepare_zero_step_crossing_armed_pages::<T>(true, false);
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert_zero_step_crossing_armed_pages::<T>(actor_id, feed, moved, destination, true, false);
  }

  /// Complete authored close reclaims the destination page allocated by Armed rearm.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_crossing_armed_new_page_close() {
    let (actor_id, feed, moved, destination) =
      prepare_zero_step_crossing_armed_pages::<T>(true, true);
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert_zero_step_crossing_armed_pages::<T>(actor_id, feed, moved, destination, true, true);
  }

  /// Full source-page refill, singleton-tail removal and reallocation during complete User rearm.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_crossing_page() {
    let (actor_id, feed, tail_actor) = prepare_zero_step_crossing_page::<T>(false, 1);
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert_zero_step_crossing_page::<T>(actor_id, feed, tail_actor, false, 1);
  }

  /// Complete authored close after full-page Crossing rearm, retaining every guard Actor.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_crossing_page_close() {
    let (actor_id, feed, tail_actor) = prepare_zero_step_crossing_page::<T>(true, 1);
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert_zero_step_crossing_page::<T>(actor_id, feed, tail_actor, true, 1);
  }

  /// Complete User rearm rewrites two full Crossing member pages and the moved inverse locator.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_crossing_page_full() {
    let members = T::CrossingPageSize::get();
    let (actor_id, feed, tail_actor) = prepare_zero_step_crossing_page::<T>(false, members);
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert_zero_step_crossing_page::<T>(actor_id, feed, tail_actor, false, members);
  }

  /// Complete authored close after full source/tail-page rearm, preserving the surviving tail.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_crossing_page_full_close() {
    let members = T::CrossingPageSize::get();
    let (actor_id, feed, tail_actor) = prepare_zero_step_crossing_page::<T>(true, members);
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert_zero_step_crossing_page::<T>(actor_id, feed, tail_actor, true, members);
  }

  fn create_zero_step_observation_guard<T: Config>(
    index: u32,
    feed: T::ObservationFeedId,
  ) -> ActorId {
    let owner: T::AccountId = account("zero-step-observation-page", index, 0);
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      system_contract::<T>(
        Schedule {
          trigger: Trigger::observation_change(feed),
          cooldown_blocks: 0,
        },
        ContractSteps::<T>::default(),
      ),
    )
    .expect("ordinary zero-Step observation guard is admitted");
    NextActorId::<T>::get().saturating_sub(1)
  }

  /// Complete User close unlinks the current middle dirty feed and allocates a new free-slot page.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_observation_feed_close() {
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    let feeds = observation_feed_pool::<T>(3);
    let (previous_feed, feed, next_feed) = (feeds[0], feeds[1], feeds[2]);
    let page_size = T::ObservationPageSize::get();
    assert!(page_size > 1 && T::MaxObservationFanoutPagesPerBlock::get() >= 2);
    assert_eq!(NextObservationSubscriptionSlot::<T>::get(), 0);
    assert_eq!(ObservationFreeSlotLen::<T>::get(), 0);
    let mut guards = Vec::new();
    for index in 0..2 * page_size - 1 {
      guards.push(create_zero_step_observation_guard::<T>(
        index,
        previous_feed,
      ));
    }
    let (actor_id, _) = create_zero_step_observation_user::<T>(feed, None, true, 2);
    let slot = ObservationSubscriptionSlot::<T>::get(actor_id).unwrap();
    assert_eq!(slot, 2 * page_size - 1);
    guards.push(create_zero_step_observation_guard::<T>(
      2 * page_size,
      next_feed,
    ));
    let mut donors = Vec::new();
    for index in 0..page_size {
      donors.push(create_zero_step_observation_guard::<T>(
        3 * page_size + index,
        previous_feed,
      ));
    }
    frame_system::Pallet::<T>::set_block_number(2u32.into());
    for guard in &guards {
      Pallet::<T>::pause_actor(RawOrigin::Root.into(), *guard).unwrap();
    }
    for donor in donors {
      Pallet::<T>::close_actor(RawOrigin::Root.into(), donor).unwrap();
    }
    assert_eq!(ObservationFreeSlotLen::<T>::get(), page_size);
    let full_free_page = ObservationFreeSlotPages::<T>::get(0).unwrap();
    assert_eq!(full_free_page.len() as u32, page_size);
    assert!(!ObservationFreeSlotPages::<T>::contains_key(1));
    assert_eq!(
      ObservationSubscriberPageLists::<T>::get(previous_feed)
        .unwrap()
        .count,
      2
    );
    frame_system::Pallet::<T>::set_block_number(3u32.into());
    let first_value = publish_zero_step_observation::<T>(feed, 1_000);
    Pallet::<T>::do_fanout_dirty_observation_page().unwrap();
    assert!(benchmark_fixture_hot::<T>(actor_id).unwrap().pending_signal);
    assert_eq!(DirtyObservationListState::<T>::get().count, 0);
    frame_system::Pallet::<T>::set_block_number(4u32.into());
    publish_zero_step_observation::<T>(previous_feed, 1_000);
    assert_ne!(publish_zero_step_observation::<T>(feed, 1), first_value);
    publish_zero_step_observation::<T>(next_feed, 1_000);
    Pallet::<T>::do_fanout_dirty_observation_page().unwrap();
    let list = DirtyObservationListState::<T>::get();
    assert_eq!(
      (list.head, list.tail, list.cursor, list.count),
      (Some(previous_feed), Some(next_feed), Some(feed), 3)
    );
    let target_dirty = DirtyObservationFeeds::<T>::get(feed).unwrap();
    assert_eq!(
      (
        target_dirty.previous_dirty_feed,
        target_dirty.next_dirty_feed
      ),
      (Some(previous_feed), Some(next_feed))
    );
    let previous_before = DirtyObservationFeeds::<T>::get(previous_feed).unwrap();
    let next_before = DirtyObservationFeeds::<T>::get(next_feed).unwrap();
    assert_eq!(previous_before.next_subscriber_page, Some(1));
    let page = ObservationSubscriberPages::<T>::get(feed, 1).unwrap();
    assert_eq!(page.entries.len() as u32, page_size);
    assert!(
      page.entries[..page.entries.len() - 1]
        .iter()
        .all(Option::is_none)
    );
    assert_eq!(page.entries.last(), Some(&Some(actor_id)));
    assert_eq!(ObservationSubscriberCount::<T>::get(feed), 1);
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state()
      .expect("real publications retain the target as the current middle dirty feed");
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert_zero_step_observation::<T>(actor_id, feed, None, true);
    assert!(!ObservationSubscriberPages::<T>::contains_key(feed, 1));
    assert!(!ObservationSubscriberPageLists::<T>::contains_key(feed));
    assert!(!ObservationSubscriberCount::<T>::contains_key(feed));
    assert!(!ObservationIngressRevisions::<T>::contains_key(feed));
    assert!(!DirtyObservationFeeds::<T>::contains_key(feed));
    let list = DirtyObservationListState::<T>::get();
    assert_eq!(
      (list.head, list.tail, list.cursor, list.count),
      (Some(previous_feed), Some(next_feed), Some(next_feed), 2)
    );
    let mut expected_previous = previous_before;
    expected_previous.next_dirty_feed = Some(next_feed);
    let mut expected_next = next_before;
    expected_next.previous_dirty_feed = Some(previous_feed);
    assert_eq!(
      DirtyObservationFeeds::<T>::get(previous_feed),
      Some(expected_previous)
    );
    assert_eq!(
      DirtyObservationFeeds::<T>::get(next_feed),
      Some(expected_next)
    );
    assert_eq!(ObservationFreeSlotLen::<T>::get(), page_size + 1);
    assert_eq!(ObservationFreeSlotPages::<T>::get(0), Some(full_free_page));
    assert_eq!(
      ObservationFreeSlotPages::<T>::get(1).unwrap().as_slice(),
      &[slot]
    );
    frame_system::Pallet::<T>::set_block_number(5u32.into());
    for _ in 0..2 {
      Pallet::<T>::do_fanout_dirty_observation_page().unwrap();
    }
    assert_eq!(DirtyObservationListState::<T>::get().count, 0);
    for guard in guards {
      assert_eq!(
        benchmark_fixture_identity::<T>(guard).unwrap().cycle_nonce,
        0
      );
      let hot = benchmark_fixture_hot::<T>(guard).unwrap();
      assert!(hot.lifecycle.is_paused() && hot.queue_ticket.is_none());
    }
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("both surviving dirty feeds complete after middle unlink");
  }

  /// Complete User close unlinks a sparse middle page and advances a real dirty fanout cursor.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_observation_unlink() {
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    let feeds = observation_feed_pool::<T>(2);
    let feed = feeds[0];
    let page_size = T::ObservationPageSize::get();
    assert!(page_size > 1 && T::MaxObservationFanoutPagesPerBlock::get() >= 3);
    assert_eq!(NextObservationSubscriptionSlot::<T>::get(), 0);
    assert_eq!(ObservationFreeSlotLen::<T>::get(), 0);
    let mut guards = Vec::new();
    for index in 0..page_size {
      guards.push(create_zero_step_observation_guard::<T>(index, feed));
    }
    let mut fillers = Vec::new();
    for index in 0..page_size - 1 {
      fillers.push(create_zero_step_observation_guard::<T>(
        page_size + index,
        feeds[1],
      ));
    }
    let (actor_id, _) = create_zero_step_observation_user::<T>(feed, None, true, 2);
    let slot = ObservationSubscriptionSlot::<T>::get(actor_id).unwrap();
    assert_eq!(slot, 2 * page_size - 1);
    for index in 0..page_size {
      guards.push(create_zero_step_observation_guard::<T>(
        2 * page_size + index,
        feed,
      ));
    }
    frame_system::Pallet::<T>::set_block_number(2u32.into());
    for guard in &guards {
      Pallet::<T>::pause_actor(RawOrigin::Root.into(), *guard)
        .expect("ordinary pause retains guard subscriptions without Ready service");
    }
    for filler in fillers {
      Pallet::<T>::close_actor(RawOrigin::Root.into(), filler)
        .expect("ordinary filler close retains the target's sparse slot geometry");
    }
    frame_system::Pallet::<T>::set_block_number(3u32.into());
    let first_value = publish_zero_step_observation::<T>(feed, 1_000);
    for _ in 0..3 {
      Pallet::<T>::do_fanout_dirty_observation_page().unwrap();
    }
    assert!(DirtyObservationFeeds::<T>::get(feed).is_none());
    assert!(benchmark_fixture_hot::<T>(actor_id).unwrap().pending_signal);
    for guard in &guards {
      let hot = benchmark_fixture_hot::<T>(*guard).unwrap();
      assert!(hot.lifecycle.is_paused() && hot.queue_ticket.is_none());
    }
    frame_system::Pallet::<T>::set_block_number(4u32.into());
    assert_ne!(publish_zero_step_observation::<T>(feed, 1), first_value);
    Pallet::<T>::do_fanout_dirty_observation_page().unwrap();
    let dirty_before = DirtyObservationFeeds::<T>::get(feed).unwrap();
    assert_eq!(dirty_before.next_subscriber_page, Some(1));
    assert_eq!(dirty_before.next_subscriber_position, 0);
    let page = ObservationSubscriberPages::<T>::get(feed, 1).unwrap();
    assert_eq!((page.previous, page.next), (Some(0), Some(2)));
    assert_eq!(page.entries.len() as u32, page_size);
    assert_eq!(page.entries.last(), Some(&Some(actor_id)));
    assert!(
      page.entries[..page.entries.len() - 1]
        .iter()
        .all(Option::is_none)
    );
    for page_id in [0, 2] {
      let page = ObservationSubscriberPages::<T>::get(feed, page_id).unwrap();
      assert_eq!(page.entries.len() as u32, page_size);
      assert!(page.entries.iter().all(Option::is_some));
    }
    assert_eq!(ObservationFreeSlotLen::<T>::get(), page_size - 1);
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("ordinary sparse middle page and live fanout are coherent");
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert_zero_step_observation::<T>(actor_id, feed, None, true);
    assert!(!ObservationSubscriberPages::<T>::contains_key(feed, 1));
    assert_eq!(
      ObservationSubscriberPages::<T>::get(feed, 0).unwrap().next,
      Some(2)
    );
    assert_eq!(
      ObservationSubscriberPages::<T>::get(feed, 2)
        .unwrap()
        .previous,
      Some(0)
    );
    let list = ObservationSubscriberPageLists::<T>::get(feed).unwrap();
    assert_eq!((list.head, list.tail, list.count), (0, 2, 2));
    assert_eq!(ObservationSubscriberCount::<T>::get(feed), 2 * page_size);
    let mut expected_dirty = dirty_before;
    expected_dirty.next_subscriber_page = Some(2);
    assert_eq!(DirtyObservationFeeds::<T>::get(feed), Some(expected_dirty));
    assert_eq!(ObservationFreeSlotLen::<T>::get(), page_size);
    assert_eq!(
      ObservationFreeSlotPages::<T>::get(0).unwrap().last(),
      Some(&slot)
    );
    frame_system::Pallet::<T>::set_block_number(5u32.into());
    Pallet::<T>::do_fanout_dirty_observation_page().unwrap();
    assert!(DirtyObservationFeeds::<T>::get(feed).is_none());
    for guard in guards {
      assert_eq!(
        benchmark_fixture_identity::<T>(guard).unwrap().cycle_nonce,
        0
      );
      let hot = benchmark_fixture_hot::<T>(guard).unwrap();
      assert!(hot.lifecycle.is_paused() && hot.queue_ticket.is_none());
    }
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state()
      .expect("fanout resumes past the removed page without losing guards");
  }

  /// Complete User close rewrites a full subscriber page and fills an existing free-slot page.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_observation_page_close() {
    let (actor_id, feed) = prepare_zero_step_observation::<T>(None, true);
    let page_size = T::ObservationPageSize::get();
    assert!(page_size > 1);
    let slot = ObservationSubscriptionSlot::<T>::get(actor_id).unwrap();
    assert_eq!(
      slot % page_size,
      0,
      "qualified target starts a subscription page"
    );
    assert_eq!(ObservationFreeSlotLen::<T>::get(), 0);
    let page_id = slot / page_size;
    let mut last_guard = actor_id;
    for index in 0..page_size - 1 {
      last_guard = create_zero_step_observation_guard::<T>(index, feed);
      assert_eq!(
        ObservationSubscriptionSlot::<T>::get(last_guard),
        Some(slot + index + 1)
      );
    }
    let mut donors = Vec::new();
    for index in 0..page_size - 1 {
      donors.push(create_zero_step_observation_guard::<T>(
        page_size + index,
        feed,
      ));
    }
    for donor in donors {
      Pallet::<T>::close_actor(RawOrigin::Root.into(), donor)
        .expect("ordinary donor close fills the reusable free-slot page");
    }
    assert_eq!(ObservationFreeSlotLen::<T>::get(), page_size - 1);
    assert_eq!(
      ObservationFreeSlotPages::<T>::get(0).unwrap().len() as u32,
      page_size - 1
    );
    let page = ObservationSubscriberPages::<T>::get(feed, page_id).unwrap();
    assert_eq!(page.entries.len() as u32, page_size);
    assert!(page.entries.iter().all(Option::is_some));
    assert_eq!(ObservationSubscriberCount::<T>::get(feed), page_size);
    assert!(DirtyObservationFeeds::<T>::get(feed).is_none());
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state()
      .expect("dense subscriber and reusable free-slot pages are coherent");
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert_zero_step_observation::<T>(actor_id, feed, None, true);
    let page = ObservationSubscriberPages::<T>::get(feed, page_id).unwrap();
    assert_eq!(page.entries.len() as u32, page_size);
    assert_eq!(page.entries[0], None);
    assert_eq!(page.entries.last(), Some(&Some(last_guard)));
    assert_eq!(ObservationSubscriberCount::<T>::get(feed), page_size - 1);
    assert_eq!(ObservationFreeSlotLen::<T>::get(), page_size);
    let free = ObservationFreeSlotPages::<T>::get(0).unwrap();
    assert_eq!(free.len() as u32, page_size);
    assert_eq!(free.last(), Some(&slot));
    assert_eq!(
      benchmark_fixture_identity::<T>(last_guard)
        .unwrap()
        .cycle_nonce,
      0
    );
    let hot = benchmark_fixture_hot::<T>(last_guard).unwrap();
    assert!(!hot.pending_signal && hot.queue_ticket.is_none());
  }

  /// Complete User zero-Step observation inner atom on a fresh single-member feed.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_crossing_armed() {
    let (actor_id, feed) = prepare_zero_step_observation::<T>(Some(CrossingPhase::Armed), false);
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert_zero_step_observation::<T>(actor_id, feed, Some(CrossingPhase::Armed), false);
  }

  /// Complete User zero-Step observation inner atom on a fresh single-member feed.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_crossing_waiting() {
    let (actor_id, feed) =
      prepare_zero_step_observation::<T>(Some(CrossingPhase::WaitingForRearm), false);
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert_zero_step_observation::<T>(actor_id, feed, Some(CrossingPhase::WaitingForRearm), false);
  }

  /// Complete User zero-Step observation inner atom on a fresh single-member feed.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_crossing_close() {
    let (actor_id, feed) =
      prepare_zero_step_observation::<T>(Some(CrossingPhase::WaitingForRearm), true);
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert_zero_step_observation::<T>(actor_id, feed, Some(CrossingPhase::WaitingForRearm), true);
  }

  /// Complete User zero-Step observation inner atom on a fresh single-member feed.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_observation() {
    let (actor_id, feed) = prepare_zero_step_observation::<T>(None, false);
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert_zero_step_observation::<T>(actor_id, feed, None, false);
  }

  /// Complete User zero-Step observation inner atom on a fresh single-member feed.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_observation_close() {
    let (actor_id, feed) = prepare_zero_step_observation::<T>(None, true);
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert_zero_step_observation::<T>(actor_id, feed, None, true);
  }

  /// Exact User Pipeline collection/receipt primitive on an admitted empty Contract head.
  /// It excludes cycle execution, placement and User hold reconciliation, and does not bound a larger head.
  #[benchmark(pov_mode = Measured)]
  fn pipeline_zero_step_opening_collection() {
    let actor_id = prepare_zero_step_opening::<T>(ActorType::User);
    let opening = Pallet::<T>::active_actor_view(actor_id).expect("Idle User view exists");
    let native = T::FeeNativeAssetId::get();
    let sink = T::FeeSink::get();
    let payer_before = T::AssetOps::balance(&opening.sovereign_account, native);
    let sink_before = T::AssetOps::balance(&sink, native);
    let fee = Pallet::<T>::pipeline_fee_for_actor(actor_id, ActorType::User)
      .expect("certified Pipeline fee exists")
      .total_fee;
    assert!(
      !fee.is_zero(),
      "this collection profile requires a nonzero host fee"
    );
    #[block]
    {
      Pallet::<T>::charge_pipeline_opening(actor_id, &opening)
        .expect("funded Pipeline collection succeeds");
    }
    assert_eq!(
      T::AssetOps::balance(&opening.sovereign_account, native),
      payer_before - fee
    );
    assert_eq!(T::AssetOps::balance(&sink, native), sink_before + fee);
    frame_system::Pallet::<T>::assert_last_event(
      Event::<T>::PipelineFeeCharged { actor_id, fee }.into(),
    );
    assert_eq!(
      benchmark_fixture_identity::<T>(actor_id)
        .unwrap()
        .cycle_nonce,
      0
    );
  }

  fn large_header_fields<T: Config>() -> (
    T::AccountId,
    T::AssetId,
    TriggerOf<T>,
    FundingSourcePolicyOf<T>,
  ) {
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    let owner: T::AccountId = account("large-head-owner", 0, 0);
    ensure_creation_balance::<T>(&owner);
    let count = T::MaxWhitelistSize::get();
    assert!(count > 0);
    let mut sources = vec![owner.clone()];
    sources.extend((1..count).map(|index| account("large-head-source", index, 0)));
    sources.sort_by_key(Encode::encode);
    sources.dedup();
    assert_eq!(sources.len(), count as usize);
    let mut assets = T::BenchmarkHelper::setup_predicate_assets(&owner, count + 1)
      .expect("large-head assets exist");
    assets.retain(|asset| asset.encoded_size() == T::AssetId::max_encoded_len());
    assets.sort_by_key(Encode::encode);
    assets.dedup();
    assets.truncate(count as usize);
    assert_eq!(assets.len(), count as usize);
    let asset = assets[0];
    let trigger = Trigger::AddressEvent {
      source_filter: SourceFilter::Whitelist(sources.clone().try_into().unwrap()),
      asset_filter: AssetFilter::Whitelist(assets.try_into().unwrap()),
    };
    assert_eq!(trigger.encoded_size(), TriggerOf::<T>::max_encoded_len());
    let funding = FundingSourcePolicyOf::<T>::SignedAllowlist(
      sources
        .into_iter()
        .collect::<alloc::collections::BTreeSet<_>>()
        .try_into()
        .unwrap(),
    );
    assert_eq!(
      funding.encoded_size(),
      FundingSourcePolicyOf::<T>::max_encoded_len()
    );
    (owner, asset, trigger, funding)
  }

  /// Exact zero-Step head byte maximum, with a future window reference and complete User control.
  /// This byte certificate does not certify maximum Waiting geometry or CPU branch dominance.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_user_max_head() {
    let (owner, asset, trigger, funding) = large_header_fields::<T>();
    let steps = ContractSteps::<T>::default();
    prefund_active_user_creation::<T>(&owner, &steps);
    let extra = full_attempt_fee::<T>(&steps)
      .checked_add(
        &Pallet::<T>::trigger_fee_for_weight(
          ActorType::User,
          TriggerFamily::AddressEvent,
          T::WeightInfo::address_event_trigger_occurrence(),
        )
        .trigger_fee,
      )
      .unwrap();
    let now = frame_system::Pallet::<T>::block_number();
    let end = now.saturating_add(T::MinWindowLength::get());
    Pallet::<T>::create_user_actor(
      RawOrigin::Signed(owner.clone()).into(),
      Mutability::Mutable,
      Some(ActorContract {
        trigger,
        cooldown_blocks: 0,
        window: Some(ScheduleWindow { start: now, end }),
        steps,
        funding,
        completion: CompletionPolicy::Persistent,
        auto_close_at_cycle_nonce: Some(T::MaxAutoCloseNonceHorizon::get()),
      }),
    )
    .expect("maximum-header zero-Step User Contract is ordinarily admitted");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let head = ActorContractHeads::<T>::get(actor_id).unwrap();
    assert_eq!(head.header.step_count, 0);
    assert!(head.first_step.is_none() && head.first_step_resources.is_none());
    assert_eq!(
      head.header.encoded_size(),
      ActorContractHeaderOf::<T>::max_encoded_len()
    );
    assert_eq!(
      head.encoded_size(),
      ActorContractHeaderOf::<T>::max_encoded_len() + 2
    );
    let sovereign = benchmark_fixture_identity::<T>(actor_id)
      .unwrap()
      .sovereign_account;
    T::AssetOps::mint(&sovereign, T::FeeNativeAssetId::get(), extra).unwrap();
    let ingress = T::AssetOps::minimum_balance(asset).max(1u32.into());
    T::AssetOps::mint(&owner, asset, ingress + ingress).unwrap();
    T::BenchmarkHelper::enable_asset_ops_ingress();
    T::BenchmarkHelper::transfer_signed(&owner, &sovereign, asset, ingress)
      .expect("certified ingress activates the maximum-header zero-Step Actor");
    let (_, cell) = Pallet::<T>::actor_control_cell(actor_id).unwrap();
    assert!(cell.hot.pending_signal);
    frame_system::Pallet::<T>::set_block_number(cell.eligible_at.unwrap());
    let funding = ActorFunding::<T>::get(actor_id).unwrap();
    assert!(funding.funding_tracked_assets.is_empty() && funding.funding_accumulated.is_empty());
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("maximum-header readiness and window are coherent");
    let before = user_pipeline_accounting::<T>(actor_id);
    let (state, admission) = consume_zero_step_opening::<T>(actor_id);
    let now = frame_system::Pallet::<T>::block_number();
    #[block]
    {
      execute_zero_step_inner::<T>(actor_id, state, &admission, now);
    }
    assert_user_pipeline_accounting::<T>(actor_id, before);
    assert_eq!(
      benchmark_fixture_identity::<T>(actor_id)
        .unwrap()
        .cycle_nonce,
      1
    );
    let hot = benchmark_fixture_hot::<T>(actor_id).unwrap();
    assert_eq!(hot.cycle_state, CycleState::Idle);
    assert!(!hot.pending_signal && hot.queue_ticket.is_none());
    let pointer = hot
      .wakeup_pointer
      .expect("future window retains its Block reference");
    assert_eq!(
      pointer.block,
      WakeupKey::Block(end.saturating_add(1u32.into()))
    );
    assert!(Pallet::<T>::wakeup_page_entry_matches(pointer, actor_id));
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state()
      .expect("maximum-header zero-Step completion and holds are coherent");
  }

  /// Large admitted head, with host-qualified field widths and semantic predicate cardinality.
  /// Compact resource widths and decode-branch dominance remain separate envelope questions.
  #[benchmark(pov_mode = Measured)]
  fn pipeline_large_head_opening_collection() {
    let (owner, asset, trigger, funding) = large_header_fields::<T>();
    let feed = T::BenchmarkHelper::contract_head_observation_feed()
      .expect("host provides a Contract-head feed witness");
    assert_eq!(feed.encoded_size(), T::ObservationFeedId::max_encoded_len());
    let predicate =
      Predicate::<T::AssetId, T::Balance, u32, T::ObservationFeedId>::ObservationAbove {
        feed,
        threshold: 1,
        max_age_blocks: 1,
      };
    assert_eq!(
      predicate.encoded_size(),
      Predicate::<T::AssetId, T::Balance, u32, T::ObservationFeedId>::max_encoded_len()
    );
    let predicates = benchmark_predicate_capacity::<T>();
    assert!(predicates > 0 && predicates <= T::MaxPreconditionClauses::get());
    let precondition = packed_predicate_clauses::<T>(
      (0..predicates)
        .map(|index| TimedPredicate {
          timing: ObservationTiming::Current,
          predicate: Predicate::ObservationAbove {
            feed,
            threshold: u128::from(index) + 1,
            max_age_blocks: 1,
          },
        })
        .collect(),
      1,
    );
    let legs = T::MaxSplitTransferLegs::get();
    assert!(legs > 0 && legs <= Perbill::ACCURACY);
    let task = ActorTask::SplitTransfer {
      asset,
      amount: AmountResolution::Fixed(1u32.into()),
      legs: (0..legs)
        .map(|index| SplitLeg {
          to: account("large-head-recipient", index, 0),
          share: Perbill::from_parts(Perbill::ACCURACY / legs),
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap(),
    };
    assert_eq!(task.encoded_size(), TaskOf::<T>::max_encoded_len());
    let steps: ContractSteps<T> = vec![Step {
      precondition: Some(precondition),
      task,
      on_error: StepErrorPolicy::RetryLater { max_attempts: 2 },
    }]
    .try_into()
    .unwrap();
    prefund_active_user_creation::<T>(&owner, &steps);
    let extra = full_attempt_fee::<T>(&steps)
      .checked_add(
        &Pallet::<T>::trigger_fee_for_weight(
          ActorType::User,
          TriggerFamily::AddressEvent,
          T::WeightInfo::address_event_trigger_occurrence(),
        )
        .trigger_fee,
      )
      .expect("large-head fee funding fits");
    let now = frame_system::Pallet::<T>::block_number();
    Pallet::<T>::create_user_actor(
      RawOrigin::Signed(owner.clone()).into(),
      Mutability::Mutable,
      Some(ActorContract {
        trigger,
        cooldown_blocks: 0,
        window: Some(ScheduleWindow {
          start: now,
          end: now.saturating_add(T::MinWindowLength::get()),
        }),
        steps,
        funding,
        completion: CompletionPolicy::Persistent,
        auto_close_at_cycle_nonce: Some(T::MaxAutoCloseNonceHorizon::get()),
      }),
    )
    .expect("large-head User Contract is admitted without fixture mutation");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let opening = Pallet::<T>::active_actor_view(actor_id).expect("Idle User view exists");
    let native = T::FeeNativeAssetId::get();
    T::AssetOps::mint(&opening.sovereign_account, native, extra).unwrap();
    let ingress = T::AssetOps::minimum_balance(asset).max(1u32.into());
    T::AssetOps::mint(&owner, asset, ingress + ingress).unwrap();
    T::BenchmarkHelper::enable_asset_ops_ingress();
    T::BenchmarkHelper::transfer_signed(&owner, &opening.sovereign_account, asset, ingress)
      .expect("certified signed ingress reaches the large-head Actor");
    let (_, cell) = Pallet::<T>::actor_control_cell(actor_id).expect("Ready cell exists");
    assert!(cell.hot.pending_signal);
    frame_system::Pallet::<T>::set_block_number(
      cell.eligible_at.expect("Ready eligibility exists"),
    );
    let opening = Pallet::<T>::active_actor_view(actor_id).expect("latched Idle User view exists");
    let head = ActorContractHeads::<T>::get(actor_id).expect("admitted head exists");
    assert_eq!(head.header.step_count, 1);
    let stored = head.first_step.expect("admitted Step exists");
    assert_eq!(stored.precondition.unwrap().predicate_count(), predicates);
    assert_eq!(stored.task.encoded_size(), TaskOf::<T>::max_encoded_len());
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("large-head fixture and User holds are coherent");
    let sink = T::FeeSink::get();
    let payer_before = T::AssetOps::balance(&opening.sovereign_account, native);
    let sink_before = T::AssetOps::balance(&sink, native);
    let fee = Pallet::<T>::pipeline_fee_for_actor(actor_id, ActorType::User)
      .expect("certified Pipeline fee exists")
      .total_fee;
    assert!(
      !fee.is_zero(),
      "this collection profile requires a nonzero host fee"
    );
    #[block]
    {
      Pallet::<T>::charge_pipeline_opening(actor_id, &opening)
        .expect("funded large-head Pipeline collection succeeds");
    }
    assert_eq!(
      T::AssetOps::balance(&opening.sovereign_account, native),
      payer_before - fee
    );
    assert_eq!(T::AssetOps::balance(&sink, native), sink_before + fee);
    frame_system::Pallet::<T>::assert_last_event(
      Event::<T>::PipelineFeeCharged { actor_id, fee }.into(),
    );
    assert_eq!(
      benchmark_fixture_identity::<T>(actor_id)
        .unwrap()
        .cycle_nonce,
      0
    );
  }

  fn prepare_temporal_control_cell<T: Config>(at_time: bool) -> ActorId {
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    let owner: T::AccountId = account("temporal-control-owner", 0, 0);
    let asset = T::BenchmarkHelper::setup_predicate_assets(&owner, 2)
      .expect("ordinary assets exist")
      .into_iter()
      .find(|asset| *asset != T::FeeNativeAssetId::get())
      .expect("ordinary non-native asset exists");
    let leg_count = T::MaxSplitTransferLegs::get();
    assert!(leg_count > 0 && leg_count <= Perbill::ACCURACY);
    let steps = vec![Step {
      precondition: None,
      task: ActorTask::SplitTransfer {
        asset,
        amount: AmountResolution::Fixed(1u32.into()),
        legs: (0..leg_count)
          .map(|index| SplitLeg {
            to: account("temporal-control-recipient", index, 0),
            share: Perbill::from_parts(Perbill::ACCURACY / leg_count),
          })
          .collect::<Vec<_>>()
          .try_into()
          .unwrap(),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
    }]
    .try_into()
    .unwrap();
    let mut contract = system_contract::<T>(
      Schedule {
        trigger: if at_time {
          Trigger::at_time(100)
        } else {
          Trigger::cadenced(100)
        },
        cooldown_blocks: 0,
      },
      steps,
    )
    .unwrap();
    contract.cooldown_blocks = 1;
    polkadot_sdk::frame_support::assert_noop!(
      Pallet::<T>::create_system_actor(
        RawOrigin::Root.into(),
        owner.clone(),
        Mutability::Mutable,
        Some(contract.clone()),
      ),
      Error::<T>::InvalidTriggerConfiguration
    );
    contract.cooldown_blocks = 0;
    contract.window = Some(ScheduleWindow {
      start: 1u32.into(),
      end: T::MinWindowLength::get().saturating_add(1_000u32.into()),
    });
    polkadot_sdk::frame_support::assert_noop!(
      Pallet::<T>::create_system_actor(
        RawOrigin::Root.into(),
        owner.clone(),
        Mutability::Mutable,
        Some(contract.clone()),
      ),
      Error::<T>::InvalidScheduleWindow
    );
    contract.window = None;
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      Some(contract),
    )
    .expect("ordinary temporal control Contract is admitted");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let (_, cell) = Pallet::<T>::actor_control_cell(actor_id).expect("created control cell");
    assert!(cell.hot.wakeup_pointer.is_none() && cell.hot.trigger_wakeup_pointer.is_some());
    assert!(cell.hot.terminal_at.is_none() && cell.eligible_at.is_none());
    assert!(cell.hot.last_cycle_block.is_none());
    report_temporal_control_cell::<T>(actor_id, "created");
    let tick = cell.hot.trigger_wakeup_pointer.unwrap().tick;
    T::BenchmarkHelper::advance_to_scheduler_tick(tick).expect("ordinary due host clock");
    let now = frame_system::Pallet::<T>::block_number();
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    Pallet::<T>::drain_overdue_wakeups_cursor(now, &mut meter);
    let (_, cell) = Pallet::<T>::actor_control_cell(actor_id).expect("latched temporal cell");
    assert!(cell.hot.pending_signal && cell.hot.trigger_wakeup_pointer.is_none());
    frame_system::Pallet::<T>::set_block_number(cell.eligible_at.expect("ordinary eligibility"));
    actor_id
  }

  fn report_temporal_control_cell<T: Config>(actor_id: ActorId, phase: &str) {
    let (_, cell) = Pallet::<T>::actor_control_cell(actor_id).expect("stored control cell");
    let authority = T::AdmissionCertificateAuthority::current().expect("host authority");
    assert_eq!(
      cell.admission.maximum_lifecycle_weight,
      authority.maximum_lifecycle_weight
    );
    frame::log::info!(
      "Temporal control cell {}: total={}, nominal={}, identity={}, hot={}, cursor={}, eligible={}, certificate={}, resources={}; lifecycle={:?}, control={:?}, effect={:?}; Weight bytes={}/{}/{}; service={:?}",
      phase,
      cell.encoded_size(),
      ActorControlCellOf::<T>::max_encoded_len(),
      cell.identity.encoded_size(),
      cell.hot.encoded_size(),
      cell.cursor.encoded_size(),
      cell.eligible_at.encoded_size(),
      cell.admission.encoded_size(),
      cell.resources.encoded_size(),
      cell.admission.maximum_lifecycle_weight,
      cell.resources.control,
      cell.resources.effect,
      cell.admission.maximum_lifecycle_weight.encoded_size(),
      cell.resources.control.encoded_size(),
      cell.resources.effect.encoded_size(),
      Pallet::<T>::guaranteed_actor_service_weight(),
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("ordinary temporal cell reconciles");
  }

  fn assert_temporal_control_cell<T: Config>(actor_id: ActorId, at_time: bool) {
    let (location, cell) =
      Pallet::<T>::actor_control_cell(actor_id).expect("suspended control cell");
    assert!(matches!(
      location,
      ActorControlLocation::Waiting {
        key: WakeupKey::Block(_),
        ..
      }
    ));
    assert_eq!(cell.hot.cycle_state, CycleState::Suspended);
    assert!(cell.hot.wakeup_pointer.is_some() && cell.hot.terminal_at.is_none());
    assert!(cell.hot.last_cycle_block.is_some() && cell.eligible_at.is_some());
    assert_eq!(cell.hot.trigger_wakeup_pointer.is_none(), at_time);
    let run = ActorRunStateStore::<T>::get(actor_id).expect("ordinary Temporary retry Run");
    assert_eq!(run.cursor, 0);
    assert_eq!(run.unsuccessful_attempts_at_cursor, 2);
    assert_eq!(run.last_step_outcome, Some(StepOutcome::FundingUnavailable));
    report_temporal_control_cell::<T>(actor_id, "suspended");
    if !at_time {
      let mut contract = Pallet::<T>::actor_contract(actor_id).expect("retained temporal Contract");
      contract.trigger = Trigger::at_time(100);
      let now = frame_system::Pallet::<T>::block_number();
      frame_system::Pallet::<T>::set_block_number(now.saturating_add(1u32.into()));
      contract.cooldown_blocks = 1;
      polkadot_sdk::frame_support::assert_noop!(
        Pallet::<T>::update_contract(RawOrigin::Root.into(), actor_id, contract.clone()),
        Error::<T>::InvalidTriggerConfiguration
      );
      contract.cooldown_blocks = 0;
      contract.window = Some(ScheduleWindow {
        start: now,
        end: now.saturating_add(T::MinWindowLength::get()),
      });
      polkadot_sdk::frame_support::assert_noop!(
        Pallet::<T>::update_contract(RawOrigin::Root.into(), actor_id, contract.clone()),
        Error::<T>::InvalidScheduleWindow
      );
      contract.window = None;
      Pallet::<T>::update_contract(RawOrigin::Root.into(), actor_id, contract)
        .expect("ordinary AtTime replacement cancels the old Run");
      let (_, replaced) = Pallet::<T>::actor_control_cell(actor_id).expect("replaced AtTime cell");
      assert_eq!(replaced.hot.cycle_state, CycleState::Idle);
      assert_eq!(replaced.hot.last_cycle_block, cell.hot.last_cycle_block);
      assert!(replaced.hot.wakeup_pointer.is_none() && replaced.eligible_at.is_none());
      assert!(replaced.hot.terminal_at.is_none() && replaced.hot.trigger_wakeup_pointer.is_some());
      assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
      report_temporal_control_cell::<T>(actor_id, "replaced AtTime");
    }
  }

  // These diagnostics construct two logical blocks; their timings are not production Weight.
  #[benchmark(extra, pov_mode = Measured)]
  fn control_cell_cadenced_reachability() {
    let actor_id = prepare_temporal_control_cell::<T>(false);
    #[block]
    {
      Pallet::<T>::execute_cycle(Weight::MAX);
      let run = ActorRunStateStore::<T>::get(actor_id).expect("first retry");
      frame_system::Pallet::<T>::set_block_number(run.eligible_at);
      Pallet::<T>::execute_cycle(Weight::MAX);
    }
    assert_temporal_control_cell::<T>(actor_id, false);
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn control_cell_at_time_reachability() {
    let actor_id = prepare_temporal_control_cell::<T>(true);
    #[block]
    {
      Pallet::<T>::execute_cycle(Weight::MAX);
      let run = ActorRunStateStore::<T>::get(actor_id).expect("first retry");
      frame_system::Pallet::<T>::set_block_number(run.eligible_at);
      Pallet::<T>::execute_cycle(Weight::MAX);
    }
    assert_temporal_control_cell::<T>(actor_id, true);
  }

  #[cfg(test)]
  #[test]
  fn temporal_control_cell_public_reachability() {
    for at_time in [false, true] {
      new_test_ext().execute_with(|| {
        let actor_id = prepare_temporal_control_cell::<Test>(at_time);
        Pallet::<Test>::execute_cycle(Weight::MAX);
        let run = ActorRunStateStore::<Test>::get(actor_id).expect("first retry");
        frame_system::Pallet::<Test>::set_block_number(run.eligible_at);
        Pallet::<Test>::execute_cycle(Weight::MAX);
        assert_temporal_control_cell::<Test>(actor_id, at_time);
      });
    }
  }

  /// Full User FIFO Opening at maximum Contract length, one zero-balance Opening surface and
  /// reachable tail funding. This declared corner is not an allocation-frontier envelope.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_execute_opening_max()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let t = T::MaxContractSteps::get()
      .saturating_sub(1)
      .div_ceil(MAX_STEPS_PER_TAIL_CHUNK);
    let (actor_id, count) = prepare_reachable_opening::<T>(t, ReachableOpeningProfile::UserPaged)?;
    let accounting_before = user_pipeline_accounting::<T>(actor_id);
    let hold_before =
      ActorStateHolds::<T>::get(actor_id).expect("nonzero-Step User state hold exists");
    #[block]
    {
      core::hint::black_box(Pallet::<T>::execute_cycle(Weight::MAX));
    }
    assert_user_pipeline_accounting::<T>(actor_id, accounting_before);
    let hold_after =
      ActorStateHolds::<T>::get(actor_id).expect("nonzero-Step User state hold survives");
    assert_eq!(hold_after.owner, hold_before.owner);
    assert_eq!(
      hold_after.breakdown.identity,
      hold_before.breakdown.identity
    );
    assert_eq!(
      hold_after.breakdown.contract_head,
      hold_before.breakdown.contract_head
    );
    assert_eq!(
      hold_after.breakdown.contract_body,
      hold_before.breakdown.contract_body
    );
    assert_eq!(
      hold_after.breakdown.detector,
      hold_before.breakdown.detector
    );
    assert_eq!(hold_after.breakdown.run, hold_before.breakdown.run);
    assert!(
      hold_after.breakdown.funding < hold_before.breakdown.funding,
      "Opening independently reconciles consumed funding geometry"
    );
    assert_reachable_opening::<T>(actor_id, count, ReachableOpeningProfile::UserPaged);
    Ok(())
  }

  /// An admitted Unstake loses its empty host receipt before Opening and fails pre-effect.
  /// Real tail ingress excludes that receipt; this is a reachable minimal-Opening failure corner.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_failed_min(
    t: Linear<
      0,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .div_ceil(MAX_STEPS_PER_TAIL_CHUNK)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, count) = prepare_reachable_opening::<T>(t, ReachableOpeningProfile::FailedMin)?;
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    let effect_weight;
    #[block]
    {
      effect_weight =
        execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_eq!(effect_weight, Weight::zero());
    assert_reachable_opening::<T>(actor_id, count, ReachableOpeningProfile::FailedMin);
    Ok(())
  }

  /// Measures direct minimal-geometry fresh-Opening retry at every tail-chunk count. An unfunded
  /// fixed Transfer suspends at cursor zero and installs one wakeup without Opening host reads.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_retry_min(
    t: Linear<
      0,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .div_ceil(MAX_STEPS_PER_TAIL_CHUNK)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, count) = prepare_reachable_opening::<T>(t, ReachableOpeningProfile::RetryMin)?;
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    #[block]
    {
      execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_reachable_opening::<T>(actor_id, count, ReachableOpeningProfile::RetryMin);
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn scheduler_opening_retry_full_waiting_existing()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, keys, due) = prepare_reachable_waiting_opening::<T>(1)?;
    #[block]
    {
      Pallet::<T>::execute_cycle(Weight::MAX);
    }
    assert_reachable_waiting_opening::<T>(actor_id, keys, due);
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn scheduler_opening_retry_full_waiting_new()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, keys, due) = prepare_reachable_waiting_opening::<T>(0)?;
    #[block]
    {
      Pallet::<T>::execute_cycle(Weight::MAX);
    }
    assert_reachable_waiting_opening::<T>(actor_id, keys + 1, due);
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn scheduler_opening_retry_full_waiting_page()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, keys, due) = prepare_reachable_waiting_opening::<T>(32)?;
    let key = WakeupKey::Block(due);
    let before = ActorWaitingFrameChunks::<T>::get((key, 0)).expect("full Waiting page exists");
    assert_eq!(before.live_entries, 32);
    assert!(before.next_page.is_none());
    assert!(!ActorWaitingFrameChunks::<T>::contains_key((key, 1)));
    #[block]
    {
      Pallet::<T>::execute_cycle(Weight::MAX);
    }
    assert_reachable_waiting_opening::<T>(actor_id, keys, due);
    let head = ActorWaitingFrameChunks::<T>::get((key, 0)).expect("full Waiting head remains");
    let tail = ActorWaitingFrameChunks::<T>::get((key, 1)).expect("new Waiting tail exists");
    assert_eq!(head.entries.encode(), before.entries.encode());
    assert_eq!(head.scan_slot, before.scan_slot);
    assert_eq!((head.live_entries, head.next_page), (32, Some(1)));
    assert_eq!((tail.live_entries, tail.previous_page), (1, Some(0)));
    assert!(head.previous_page.is_none() && tail.next_page.is_none());
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Waiting { key: at, page: 1, slot: 0 }) if at == key
    ));
    Ok(())
  }

  /// Complete consumed-authority Waiting publication into a populated existing Block deadline.
  /// The fixed RetryMin target isolates publication geometry; all background entries follow ordinary
  /// System creation and future-window publication until one host capacity is saturated.
  #[benchmark(extra, pov_mode = Measured)]
  fn waiting_publish_existing_populated()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let destination_entries = 1;
    let (actor_id, count, keys, due) = prepare_reachable_waiting_publication::<T>(
      destination_entries,
      0,
      ReachableOpeningProfile::RetryMin,
    )?;
    assert_eq!(count, 1);
    let publish = benchmark_fixture_prepare_consumed_service_waiting::<T>(actor_id, due);
    #[block]
    {
      publish();
    }
    assert_reachable_waiting_publication::<T>(actor_id, count, keys, due, destination_entries);
    Ok(())
  }

  /// Complete consumed-authority Waiting publication that allocates a second linked page under the
  /// same saturated host population and fixed RetryMin authority.
  #[benchmark(extra, pov_mode = Measured)]
  fn waiting_publish_full_page_populated()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let destination_entries = 32;
    let (actor_id, count, keys, due) = prepare_reachable_waiting_publication::<T>(
      destination_entries,
      0,
      ReachableOpeningProfile::RetryMin,
    )?;
    assert_eq!(count, 1);
    let before = ActorWaitingFrameChunks::<T>::get((WakeupKey::Block(due), 0))
      .expect("full populated Waiting page exists");
    assert_eq!(before.live_entries, 32);
    let publish = benchmark_fixture_prepare_consumed_service_waiting::<T>(actor_id, due);
    #[block]
    {
      publish();
    }
    assert_reachable_waiting_publication::<T>(actor_id, count, keys, due, destination_entries);
    let head = ActorWaitingFrameChunks::<T>::get((WakeupKey::Block(due), 0))
      .expect("full populated Waiting head survives");
    assert_eq!(head.entries.encode(), before.entries.encode());
    assert_eq!((head.live_entries, head.next_page), (32, Some(1)));
    Ok(())
  }

  /// Complete consumed-authority RetryMin publication of a new minimum into the maximum additional
  /// ordinary System population admitted alongside retained reference-runtime genesis authority.
  #[benchmark(extra, pov_mode = Measured)]
  fn waiting_publish_new_minimum_populated()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let destination_entries = 0;
    let (actor_id, count, keys, due) = prepare_reachable_waiting_publication::<T>(
      destination_entries,
      0,
      ReachableOpeningProfile::RetryMin,
    )?;
    assert_eq!(count, 1);
    assert!(Pallet::<T>::wakeup_cursor_peek().is_none_or(|minimum| minimum > due));
    let publish = benchmark_fixture_prepare_consumed_service_waiting::<T>(actor_id, due);
    #[block]
    {
      publish();
    }
    assert_reachable_waiting_publication::<T>(actor_id, count, keys, due, destination_entries);
    assert_eq!(Pallet::<T>::wakeup_cursor_peek(), Some(due));
    Ok(())
  }

  /// Complete new-minimum Primary publication of the largest publicly reachable control cell. The
  /// Cadenced target reaches its second real suspended attempt before its due service authority is
  /// consumed; publication then restores the Block pointer beside the retained Tick pointer under
  /// the maximum additional ordinary System population.
  #[benchmark(extra, pov_mode = Measured)]
  fn waiting_publish_max_cell_new_minimum_populated()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, maximum_size, keys, due) =
      prepare_reachable_max_cell_waiting_publication::<T>()?;
    let publish = benchmark_fixture_prepare_consumed_service_waiting::<T>(actor_id, due);
    #[block]
    {
      publish();
    }
    let (location, cell) =
      Pallet::<T>::actor_control_cell(actor_id).expect("maximal cell is published in Waiting");
    assert!(matches!(
      location,
      ActorControlLocation::Waiting {
        key: WakeupKey::Block(at),
        page: 0,
        slot: 0,
      } if at == due
    ));
    assert_eq!(cell.encoded_size(), maximum_size);
    assert_eq!(cell.hot.cycle_state, CycleState::Suspended);
    assert!(cell.hot.wakeup_pointer.is_some());
    assert!(cell.hot.trigger_wakeup_pointer.is_some());
    assert!(cell.hot.last_cycle_block.is_some() && cell.eligible_at.is_some());
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Block), keys + 1);
    assert_eq!(Pallet::<T>::wakeup_cursor_peek(), Some(due));
    assert_waiting_population_capacity::<T>();
    assert_wakeup_cursor_page_indices::<T>();
    frame::log::info!(
      "Maximal Waiting publication cell: total={}, nominal={}",
      cell.encoded_size(),
      ActorControlCellOf::<T>::max_encoded_len(),
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("maximal-cell Waiting publication is legal");
    Ok(())
  }

  /// Complete new-minimum Block Reference publication while retaining the consumed service
  /// authority's Ready primary. This isolates the bounded Reference entry and primary rewrite from
  /// the separately measured consumed-primary publication branches.
  #[benchmark(extra, pov_mode = Measured)]
  fn waiting_publish_reference_new_minimum_populated()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, count, keys, due) =
      prepare_reachable_waiting_publication::<T>(0, 0, ReachableOpeningProfile::RetryMin)?;
    assert_eq!(count, 1);
    assert!(Pallet::<T>::wakeup_cursor_peek().is_none_or(|minimum| minimum > due));
    let location = ActorControlLocators::<T>::get(actor_id).expect("Ready primary exists");
    assert!(matches!(location, ActorControlLocation::Ready { .. }));
    let publish = benchmark_fixture_prepare_service_waiting::<T>(actor_id, due);
    #[block]
    {
      publish();
    }
    assert_eq!(ActorControlLocators::<T>::get(actor_id), Some(location));
    assert_reachable_waiting_reference_publication::<T>(actor_id, keys, due);
    assert_eq!(Pallet::<T>::wakeup_cursor_peek(), Some(due));
    Ok(())
  }

  /// Complete new-minimum Cadenced Tick Reference publication while retaining an independent Block
  /// Waiting primary. The target and all Tick competitors follow ordinary temporal scheduling; the
  /// measured boundary is the cross-clock publication and retained-primary rewrite only.
  #[benchmark(extra, pov_mode = Measured)]
  fn waiting_publish_cadenced_cross_clock_populated()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, location, next, keys) =
      prepare_reachable_cadenced_cross_clock_publication::<T>()?;
    let (state, admission, loaded_step) = Pallet::<T>::load_frame_actor_service_state(actor_id)
      .expect("cross-clock target retains service authority");
    let (_, cell) = Pallet::<T>::actor_control_cell(actor_id).expect("Block primary exists");
    let resources = loaded_step
      .expect("Cadenced target retains one admitted Step")
      .resources;
    #[block]
    {
      Pallet::<T>::try_wakeup_substrate_schedule_transition_with_authority(
        actor_id,
        WakeupKey::Tick(next),
        state.hot,
        &state.identity,
        cell.cursor,
        &admission,
        resources,
      )
      .expect("cross-clock Tick Reference publication fits");
    }
    assert_eq!(ActorControlLocators::<T>::get(actor_id), Some(location));
    let (_, retained) =
      Pallet::<T>::actor_control_cell(actor_id).expect("Block primary remains authoritative");
    let pointer = retained
      .hot
      .trigger_wakeup_pointer
      .expect("Cadenced Tick Reference exists");
    assert_eq!((pointer.tick, pointer.page_id, pointer.slot), (next, 0, 0));
    let key = WakeupKey::Tick(next);
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Tick), keys + 1);
    assert!(matches!(
      &ActorWaitingFrameChunks::<T>::get((key, 0))
        .expect("new Tick reference page exists")
        .entries[0],
      Some(ActorWaitingEntry::Reference(reference))
        if reference.actor_id == actor_id
          && reference.admission_identity == retained.admission.admission_identity
    ));
    assert_waiting_population_capacity::<T>();
    assert_wakeup_cursor_page_indices::<T>();
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("complete cross-clock publication is legal");
    Ok(())
  }

  /// True host-bounded Opening predicates and tail Opening legs precede missing-receipt failure.
  /// The sole tracked receipt has no accumulated ingress; no independent funding maximum is claimed.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_failed_max(
    t: Linear<
      0,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .div_ceil(MAX_STEPS_PER_TAIL_CHUNK)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, count) = prepare_reachable_opening::<T>(t, ReachableOpeningProfile::FailedMax)?;
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    let effect_weight;
    #[block]
    {
      effect_weight =
        execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_eq!(effect_weight, Weight::zero());
    assert_reachable_opening::<T>(actor_id, count, ReachableOpeningProfile::FailedMax);
    Ok(())
  }

  /// True Opening predicates and two tail-Step Opening legs precede an unfunded fixed Transfer.
  /// This reachable retry corner has no LastFunding legs and does not establish envelope dominance.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_retry_max(
    t: Linear<
      0,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .div_ceil(MAX_STEPS_PER_TAIL_CHUNK)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, count) = prepare_reachable_opening::<T>(t, ReachableOpeningProfile::RetryMax)?;
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    #[block]
    {
      execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_reachable_opening::<T>(actor_id, count, ReachableOpeningProfile::RetryMax);
    Ok(())
  }

  /// Measures direct minimal-geometry fresh-Opening completion at every tail-chunk count. StopCycle
  /// terminates the cycle without an effect, Opening host reads, run persistence, or placement.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_complete_min(
    t: Linear<
      0,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .div_ceil(MAX_STEPS_PER_TAIL_CHUNK)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, count) =
      prepare_reachable_opening::<T>(t, ReachableOpeningProfile::CompleteMin)?;
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    #[block]
    {
      execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_reachable_opening::<T>(actor_id, count, ReachableOpeningProfile::CompleteMin);
    Ok(())
  }

  /// Complete one-Step User inner completion with an unchanged hold. Outer FIFO work stays out.
  #[benchmark(extra, pov_mode = Measured)]
  fn scheduler_inner_opening_user_complete_single()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, count) =
      prepare_reachable_opening::<T>(0, ReachableOpeningProfile::UserCompleteMin)?;
    let accounting_before = user_pipeline_accounting::<T>(actor_id);
    let hold_before = ActorStateHolds::<T>::get(actor_id).expect("User hold exists");
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    assert_eq!(state.identity.actor_class.actor_type(), ActorType::User);
    let effect_weight;
    #[block]
    {
      effect_weight =
        execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_eq!(effect_weight, Weight::zero());
    assert_user_pipeline_accounting::<T>(actor_id, accounting_before);
    assert_reachable_opening::<T>(actor_id, count, ReachableOpeningProfile::UserCompleteMin);
    assert_eq!(ActorStateHolds::<T>::get(actor_id), Some(hold_before));
    Ok(())
  }

  /// Complete one-Step User inner completion with the maximum admitted funding allowlist.
  /// This differs from the single profile only by Contract-head funding-policy width.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_user_complete_header_max()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, count) =
      prepare_reachable_opening::<T>(0, ReachableOpeningProfile::UserCompleteHeaderMax)?;
    let accounting_before = user_pipeline_accounting::<T>(actor_id);
    let state_before =
      Pallet::<T>::active_actor_state(actor_id).expect("maximum-header User authority exists");
    let FundingSourcePolicy::SignedAllowlist(sources) = &state_before.contract.funding else {
      unreachable!("maximum-header profile uses SignedAllowlist")
    };
    assert_eq!(sources.len() as u32, T::MaxWhitelistSize::get());
    assert!(sources.contains(&state_before.identity.owner));
    let hold_before = ActorStateHolds::<T>::get(actor_id).expect("User hold exists");
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    assert_eq!(state.identity.actor_class.actor_type(), ActorType::User);
    let effect_weight;
    #[block]
    {
      effect_weight =
        execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_eq!(effect_weight, Weight::zero());
    assert_user_pipeline_accounting::<T>(actor_id, accounting_before);
    assert_reachable_opening::<T>(
      actor_id,
      count,
      ReachableOpeningProfile::UserCompleteHeaderMax,
    );
    assert_eq!(ActorStateHolds::<T>::get(actor_id), Some(hold_before));
    Ok(())
  }

  /// Complete maximum-header User inner completion with positive tails and funding-hold release.
  /// This reaches the simultaneous funding-policy/tail envelope; outer FIFO work stays out.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_user_complete_header_max_tail(
    t: Linear<
      1,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .div_ceil(MAX_STEPS_PER_TAIL_CHUNK)
          .max(1)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, count) =
      prepare_reachable_opening::<T>(t, ReachableOpeningProfile::UserCompleteHeaderMax)?;
    let accounting_before = user_pipeline_accounting::<T>(actor_id);
    let state_before =
      Pallet::<T>::active_actor_state(actor_id).expect("maximum-header User authority exists");
    let FundingSourcePolicy::SignedAllowlist(sources) = &state_before.contract.funding else {
      unreachable!("maximum-header profile uses SignedAllowlist")
    };
    assert_eq!(sources.len() as u32, T::MaxWhitelistSize::get());
    assert!(sources.contains(&state_before.identity.owner));
    let mut hold_before = ActorStateHolds::<T>::get(actor_id).expect("User hold exists");
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    assert_eq!(state.identity.actor_class.actor_type(), ActorType::User);
    let effect_weight;
    #[block]
    {
      effect_weight =
        execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_eq!(effect_weight, Weight::zero());
    assert_user_pipeline_accounting::<T>(actor_id, accounting_before);
    assert_reachable_opening::<T>(
      actor_id,
      count,
      ReachableOpeningProfile::UserCompleteHeaderMax,
    );
    let hold_after = ActorStateHolds::<T>::get(actor_id).expect("User hold survives completion");
    assert!(hold_after.breakdown.funding < hold_before.breakdown.funding);
    hold_before.breakdown.funding = hold_after.breakdown.funding;
    assert_eq!(
      hold_after, hold_before,
      "only consumed funding changes the hold"
    );
    Ok(())
  }

  /// Complete User inner completion with positive tails and funding-hold release. This branch
  /// cannot share a linear fit with the one-Step no-release case; outer FIFO work stays out.
  #[benchmark(extra, pov_mode = Measured)]
  fn scheduler_inner_opening_user_complete_tail(
    t: Linear<
      1,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .div_ceil(MAX_STEPS_PER_TAIL_CHUNK)
          .max(1)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, count) =
      prepare_reachable_opening::<T>(t, ReachableOpeningProfile::UserCompleteMin)?;
    let accounting_before = user_pipeline_accounting::<T>(actor_id);
    let mut hold_before = ActorStateHolds::<T>::get(actor_id).expect("User hold exists");
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    assert_eq!(state.identity.actor_class.actor_type(), ActorType::User);
    let effect_weight;
    #[block]
    {
      effect_weight =
        execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_eq!(effect_weight, Weight::zero());
    assert_user_pipeline_accounting::<T>(actor_id, accounting_before);
    assert_reachable_opening::<T>(actor_id, count, ReachableOpeningProfile::UserCompleteMin);
    let hold_after = ActorStateHolds::<T>::get(actor_id).expect("User hold survives completion");
    assert!(hold_after.breakdown.funding < hold_before.breakdown.funding);
    hold_before.breakdown.funding = hold_after.breakdown.funding;
    assert_eq!(
      hold_after, hold_before,
      "only consumed funding changes the hold"
    );
    Ok(())
  }

  /// Measures the direct minimal-geometry fresh-Opening progress path at every tail-chunk count.
  /// Current percentage resolution skips on zero custody; real tail ingress supplies the bounded
  /// funding corner without Opening surfaces or predicates.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_progress_min(
    t: Linear<
      1,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .div_ceil(MAX_STEPS_PER_TAIL_CHUNK)
          .max(1)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, count) = prepare_reachable_opening::<T>(t, ReachableOpeningProfile::Minimal)?;
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    #[block]
    {
      execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_reachable_opening::<T>(actor_id, count, ReachableOpeningProfile::Minimal);
    Ok(())
  }

  /// True Opening predicates on every Step and two Opening legs per tail Step precede StopCycle.
  /// No LastFunding payload, retained Run, or successor placement is claimed by this corner.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_complete_max(
    t: Linear<
      0,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .div_ceil(MAX_STEPS_PER_TAIL_CHUNK)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, count) =
      prepare_reachable_opening::<T>(t, ReachableOpeningProfile::CompleteMax)?;
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    #[block]
    {
      execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_reachable_opening::<T>(actor_id, count, ReachableOpeningProfile::CompleteMax);
    Ok(())
  }

  /// Pure-Opening progress after canonical frame loading and source consumption. Two Opening legs
  /// and full Opening predicates per Step imply zero LastFunding legs in this declared corner.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_progress_max(
    t: Linear<
      1,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .div_ceil(MAX_STEPS_PER_TAIL_CHUNK)
          .max(1)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, count) = prepare_reachable_opening::<T>(t, ReachableOpeningProfile::Predicated)?;
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    #[block]
    {
      execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_reachable_opening::<T>(actor_id, count, ReachableOpeningProfile::Predicated);
    Ok(())
  }

  /// Measures the direct inner Running-final control owner after queue discovery, current-state
  /// loading, and physical head consumption. The measured atom builds the exact carried plan,
  /// evaluates one current Step, commits completion, validates actual evidence, and performs
  /// User post-placement hold reconciliation. The host owns StopCycle's effect Weight.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_running_complete(
    s: Linear<
      1,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .clamp(1, MAX_STEPS_PER_TAIL_CHUNK)
      },
    >,
    p: Linear<0, { benchmark_predicate_capacity::<T>() }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, cursor) =
      prepare_reachable_running_inner::<T>(s, p, RunningInnerBranch::Complete)?;
    let expected_effect = T::TaskEffectWeight::actual_effect_weight(
      &ActorTask::StopCycle,
      if p == 0 {
        TaskEffectExecution::Invoked
      } else {
        TaskEffectExecution::NotInvoked
      },
    )
    .expect("host supplies StopCycle effect evidence");
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    #[block]
    {
      assert_eq!(
        execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now),
        expected_effect,
      );
    }
    assert_reachable_running_inner::<T>(actor_id, cursor, now, RunningInnerBranch::Complete);
    Ok(())
  }

  /// Measures the direct inner Running-progress owner through causal FIFO successor placement.
  /// A percentage of a proven zero balance or false current predicates skips Task effects.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_running_progress(
    s: Linear<
      2,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .clamp(2, MAX_STEPS_PER_TAIL_CHUNK)
      },
    >,
    p: Linear<0, { benchmark_predicate_capacity::<T>() }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, cursor) =
      prepare_reachable_running_inner::<T>(s, p, RunningInnerBranch::Progress)?;
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    #[block]
    {
      assert!(
        execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now).is_zero()
      );
    }
    assert_reachable_running_inner::<T>(actor_id, cursor, now, RunningInnerBranch::Progress);
    Ok(())
  }

  /// Measures the direct inner Suspended-tail retry owner through durable wakeup placement.
  /// Current predicates are true and an unfunded fixed Transfer yields FundingUnavailable before
  /// Task effect invocation, preserving retry classification without effect contamination.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_tail_retry(
    s: Linear<
      1,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .clamp(1, MAX_STEPS_PER_TAIL_CHUNK)
      },
    >,
    p: Linear<0, { benchmark_predicate_capacity::<T>() }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    if T::MaxContractSteps::get() <= 1 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host Contract bound cannot represent this tail benchmark branch",
      ));
    }
    GlobalCircuitBreaker::<T>::put(false);
    let (actor_id, cursor) = prepare_reachable_suspended_tail_retry::<T>(s, p)?;
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    let effect_weight;
    #[block]
    {
      effect_weight =
        execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_eq!(effect_weight, Weight::zero());
    assert_reachable_suspended_tail_retry_state::<T>(actor_id, cursor, 2);
    Ok(())
  }

  /// Measures the direct inner Suspended-tail successful-completion owner.
  /// A real unfunded retry receives exact spendable custody and invokes its fixed Transfer.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_tail_complete(
    s: Linear<
      1,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .clamp(1, MAX_STEPS_PER_TAIL_CHUNK)
      },
    >,
    p: Linear<0, { benchmark_predicate_capacity::<T>() }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let fixture =
      prepare_reachable_suspended_tail_success::<T>(s, p, RunningInnerBranch::Complete)?;
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(fixture.actor_id);
    let expected_effect = T::TaskEffectWeight::actual_effect_weight(
      &loaded_step.step.task,
      TaskEffectExecution::Invoked,
    )
    .expect("host supplies Transfer effect evidence");
    #[block]
    {
      assert_eq!(
        execute_reachable_step_inner::<T>(fixture.actor_id, state, admission, loaded_step, now),
        expected_effect,
      );
    }
    assert_reachable_suspended_tail_success::<T>(fixture, now, RunningInnerBranch::Complete);
    Ok(())
  }

  /// Measures the direct inner Suspended-tail successful-progress owner through causal FIFO
  /// successor placement after a real unfunded retry receives exact spendable custody.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_tail_progress(
    s: Linear<
      2,
      {
        T::MaxContractSteps::get()
          .saturating_sub(1)
          .clamp(2, MAX_STEPS_PER_TAIL_CHUNK)
      },
    >,
    p: Linear<0, { benchmark_predicate_capacity::<T>() }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let fixture =
      prepare_reachable_suspended_tail_success::<T>(s, p, RunningInnerBranch::Progress)?;
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(fixture.actor_id);
    let expected_effect = T::TaskEffectWeight::actual_effect_weight(
      &loaded_step.step.task,
      TaskEffectExecution::Invoked,
    )
    .expect("host supplies Transfer effect evidence");
    #[block]
    {
      assert_eq!(
        execute_reachable_step_inner::<T>(fixture.actor_id, state, admission, loaded_step, now),
        expected_effect,
      );
    }
    assert_reachable_suspended_tail_success::<T>(fixture, now, RunningInnerBranch::Progress);
    Ok(())
  }

  /// Measures a real Current-predicate head retry after Opening and due wakeup.
  /// Tail Opening and funding cardinalities share the same authored amount-source allocation.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_head_retry(
    n: Linear<
      {
        (T::MaxContractSteps::get().saturating_sub(1) * 2)
          .saturating_sub(T::MaxFundingTrackedAssets::get())
      },
      { T::MaxContractSteps::get().saturating_sub(1) * 2 },
    >,
    p: Linear<0, { benchmark_predicate_capacity::<T>() }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, skip) = prepare_reachable_suspended_head_inner::<T>(n, p, 0, None)?;
    let before = ActorRunStateStore::<T>::get(actor_id).expect("real suspended Run exists");
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    let effect;
    #[block]
    {
      effect = execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_eq!(effect, Weight::zero());
    assert!(skip.is_none());
    assert_reachable_suspended_head_retry::<T>(actor_id, before);
    Ok(())
  }

  /// Measures a real Current-predicate head complete after Opening and due wakeup.
  /// A terminal head has one authored Step and no tail payload.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_head_complete(
    p: Linear<0, { benchmark_predicate_capacity::<T>() }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, skip) =
      prepare_reachable_suspended_head_inner::<T>(0, p, 0, Some(RunningInnerBranch::Complete))?;
    let before = ActorRunStateStore::<T>::get(actor_id).expect("real suspended Run exists");
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    let effect;
    #[block]
    {
      effect = execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_eq!(effect, Weight::zero());
    assert_eq!(before.cursor, 0);
    assert_reachable_suspended_tail_skip::<T>(
      skip.expect("freeze fixture exists"),
      now,
      RunningInnerBranch::Complete,
    );
    Ok(())
  }

  /// Measures a real Current-predicate head progress after Opening and due wakeup.
  /// Tail Opening and funding cardinalities share the same authored amount-source allocation.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_head_progress(
    n: Linear<
      {
        (T::MaxContractSteps::get().saturating_sub(1) * 2)
          .saturating_sub(T::MaxFundingTrackedAssets::get())
      },
      { T::MaxContractSteps::get().saturating_sub(1) * 2 },
    >,
    p: Linear<0, { benchmark_predicate_capacity::<T>() }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, skip) =
      prepare_reachable_suspended_head_inner::<T>(n, p, 0, Some(RunningInnerBranch::Progress))?;
    let before = ActorRunStateStore::<T>::get(actor_id).expect("real suspended Run exists");
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    let effect;
    #[block]
    {
      effect = execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_eq!(effect, Weight::zero());
    assert_eq!(before.cursor, 0);
    assert_reachable_suspended_tail_skip::<T>(
      skip.expect("freeze fixture exists"),
      now,
      RunningInnerBranch::Progress,
    );
    Ok(())
  }

  /// Measures a real Opening/Current-predicate head retry after Opening and due wakeup.
  /// Tail Opening and funding cardinalities share the same authored amount-source allocation.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_head_opening_retry(
    n: Linear<
      {
        (T::MaxContractSteps::get().saturating_sub(1) * 2)
          .saturating_sub(T::MaxFundingTrackedAssets::get())
      },
      { T::MaxContractSteps::get().saturating_sub(1) * 2 },
    >,
    p: Linear<0, { benchmark_predicate_capacity::<T>().saturating_sub(1) }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, skip) = prepare_reachable_suspended_head_inner::<T>(n, p, 1, None)?;
    let before = ActorRunStateStore::<T>::get(actor_id).expect("real suspended Run exists");
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    let effect;
    #[block]
    {
      effect = execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_eq!(effect, Weight::zero());
    assert!(skip.is_none());
    assert_reachable_suspended_head_retry::<T>(actor_id, before);
    Ok(())
  }

  /// Measures a real Opening/Current-predicate head complete after Opening and due wakeup.
  /// A terminal head has one authored Step and no tail payload.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_head_opening_complete(
    p: Linear<0, { benchmark_predicate_capacity::<T>().saturating_sub(1) }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, skip) =
      prepare_reachable_suspended_head_inner::<T>(0, p, 1, Some(RunningInnerBranch::Complete))?;
    let before = ActorRunStateStore::<T>::get(actor_id).expect("real suspended Run exists");
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    let effect;
    #[block]
    {
      effect = execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_eq!(effect, Weight::zero());
    assert_eq!(before.cursor, 0);
    assert_reachable_suspended_tail_skip::<T>(
      skip.expect("freeze fixture exists"),
      now,
      RunningInnerBranch::Complete,
    );
    Ok(())
  }

  /// Measures a real Opening/Current-predicate head progress after Opening and due wakeup.
  /// Tail Opening and funding cardinalities share the same authored amount-source allocation.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_head_opening_progress(
    n: Linear<
      {
        (T::MaxContractSteps::get().saturating_sub(1) * 2)
          .saturating_sub(T::MaxFundingTrackedAssets::get())
      },
      { T::MaxContractSteps::get().saturating_sub(1) * 2 },
    >,
    p: Linear<0, { benchmark_predicate_capacity::<T>().saturating_sub(1) }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, skip) =
      prepare_reachable_suspended_head_inner::<T>(n, p, 1, Some(RunningInnerBranch::Progress))?;
    let before = ActorRunStateStore::<T>::get(actor_id).expect("real suspended Run exists");
    let now = frame_system::Pallet::<T>::block_number();
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    let effect;
    #[block]
    {
      effect = execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now);
    }
    assert_eq!(effect, Weight::zero());
    assert_eq!(before.cursor, 0);
    assert_reachable_suspended_tail_skip::<T>(
      skip.expect("freeze fixture exists"),
      now,
      RunningInnerBranch::Progress,
    );
    Ok(())
  }

  /// Measures actual scheduler admission and complete execution for up to 1,000
  /// minimal one-step System actors. `Weight::MAX` exposes the full production-Wasm
  /// cost curve; separate guaranteed-budget stress evidence determines how many
  /// executions the reference block budget actually admits. Setup writes the split actor stores and canonical paged FIFO outside
  /// the measured block so the result isolates queue scanning, admission,
  /// execution, and consumption rather than actor creation.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_execute_cheap(n: Linear<1, 1_000>) {
    benchmark_fixture_reset_ready_queue::<T>();
    GlobalCircuitBreaker::<T>::put(false);
    let bounded = n
      .min(T::MaxExecutionsPerBlock::get())
      .min(T::MaxQueueEntriesScannedPerBlock::get())
      .min(T::MaxQueueLength::get());
    assert!(bounded > 0, "runtime limits must admit at least one sample");
    let mut actors = alloc::vec::Vec::with_capacity(bounded as usize);
    for offset in 0..bounded {
      let actor_id = bench_create_system_manual::<T>(41_000_000u32.saturating_add(offset));
      Pallet::<T>::request_activation(actor_id).expect("cheap benchmark readiness must latch");
      actors.push(actor_id);
    }
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    #[block]
    {
      core::hint::black_box(Pallet::<T>::execute_cycle(Weight::MAX));
    }
    let executed = actors
      .iter()
      .filter(|actor_id| {
        benchmark_fixture_identity::<T>(**actor_id)
          .is_some_and(|identity| identity.cycle_nonce == 1)
      })
      .count() as u32;
    assert_eq!(
      executed, bounded,
      "unbounded diagnostic budget completed only {executed} of {bounded} requested cheap actors"
    );
    assert_eq!(
      benchmark_fixture_ready_head::<T>(),
      benchmark_fixture_ready_tail::<T>()
    );
  }

  /// Measures canonical FIFO execution over alternating System/User actors.
  /// Setup materializes one ticket-ordered queue outside the measured block.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_execute_cheap_mixed(n: Linear<2, 1_000>) {
    benchmark_fixture_reset_ready_queue::<T>();
    GlobalCircuitBreaker::<T>::put(false);
    let bounded = n
      .min(T::MaxExecutionsPerBlock::get())
      .min(T::MaxQueueEntriesScannedPerBlock::get())
      .min(T::MaxQueueLength::get());
    assert!(
      bounded >= 2,
      "runtime limits must admit alternating actor classes"
    );

    let mut actors = alloc::vec::Vec::with_capacity(bounded as usize);
    for offset in 0..bounded {
      let actor_id = if offset % 2 == 0 {
        bench_create_system_manual::<T>(43_000_000u32.saturating_add(offset))
      } else {
        let owner: T::AccountId = account("mixed_user_owner", offset, 0);
        bench_create_user::<T>(owner)
      };
      Pallet::<T>::request_activation(actor_id).expect("mixed benchmark readiness must latch");
      actors.push(actor_id);
    }
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    #[block]
    {
      core::hint::black_box(Pallet::<T>::execute_cycle(Weight::MAX));
    }
    let executed = actors
      .iter()
      .filter(|actor_id| {
        benchmark_fixture_identity::<T>(**actor_id)
          .is_some_and(|identity| identity.cycle_nonce == 1)
      })
      .count() as u32;
    // Whether User fee collection materializes a Fee Sink service obligation is host
    // configuration, not a pallet guarantee. When the host does configure one, that obligation
    // consumes a single pass slot ahead of the measured cohort while the post-cutoff ticket stays
    // ordered behind it; a host without a Fee Sink runs the whole cohort.
    let ceiling = bounded.min(T::MaxExecutionsPerBlock::get());
    assert!(
      executed == ceiling || executed == ceiling.saturating_sub(1),
      "mixed canonical-FIFO cohort must reach the pass ceiling, minus at most one slot taken by a host Fee Sink service obligation"
    );
    let consumed = actors
      .iter()
      .filter(|actor_id| {
        benchmark_fixture_hot::<T>(**actor_id).is_some_and(|hot| hot.queue_ticket.is_none())
      })
      .count() as u32;
    assert_eq!(
      consumed, executed,
      "every executed cohort actor must release its queue ticket"
    );
  }

  /// Diagnostic classifier for dense Running-capable Contract geometry. Each singleton component
  /// point uses the ordinary public System constructor and either publishes one admitted actor or
  /// stops with the exact idle-budget rejection. It is excluded from production Weight output.
  #[benchmark(extra)]
  fn run_dense_admission_classifier(
    s: Linear<1, { T::MaxContractSteps::get() }>,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let owner: T::AccountId = account("dense-run-admission-owner", s, 0);
    let steps = make_reachable_opening_steps::<T>(s)?;
    let creation = Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      system_contract::<T>(
        Schedule {
          trigger: Trigger::manual(),
          cooldown_blocks: 0,
        },
        steps,
      ),
    );
    if let Err(error) = creation {
      assert_eq!(
        error,
        Error::<T>::ContractStepsExceedOnIdleBudget.into(),
        "dense Run admission classifier encountered an unrelated creation failure"
      );
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "dense Run point rejected by ordinary idle-budget admission",
      ));
    }
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    #[block]
    {
      core::hint::black_box(Pallet::<T>::active_actor_view(actor_id));
    }
    assert_eq!(
      Pallet::<T>::active_actor_view(actor_id)
        .expect("admitted dense classifier actor exists")
        .steps
        .len() as u32,
      s
    );
    Ok(())
  }

  fn prepare_low_capture_running_with_type<T: Config>(
    actor_type: ActorType,
    current_step: Option<StepOf<T>>,
    cooldown_blocks: u32,
  ) -> Result<ActorId, polkadot_sdk::frame_benchmarking::BenchmarkError> {
    if T::MaxContractSteps::get() < 2 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "low-capture Running fixture requires a successor Step",
      ));
    }
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    GlobalCircuitBreaker::<T>::put(false);
    let owner: T::AccountId = account("low-capture-run-owner", 0, 0);
    let mut steps = make_max_contract_steps::<T>(account("low-capture-run-recipient", 0, 0));
    if let Some(step) = current_step {
      steps[1] = step;
    }
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks,
    };
    if actor_type == ActorType::User {
      ensure_creation_balance::<T>(&owner);
      prefund_active_user_creation::<T>(&owner, &steps);
      Pallet::<T>::create_user_actor(
        RawOrigin::Signed(owner.clone()).into(),
        Mutability::Mutable,
        user_contract::<T>(schedule, steps),
      )
    } else {
      Pallet::<T>::create_system_actor(
        RawOrigin::Root.into(),
        owner.clone(),
        Mutability::Mutable,
        system_contract::<T>(schedule, steps),
      )
    }
    .map_err(|_| {
      polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "maximum-length low-capture Contract rejected by ordinary creation",
      )
    })?;
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    if actor_type == ActorType::User {
      let sovereign = Pallet::<T>::actor_identity(actor_id)
        .expect("admitted low-capture User identity exists")
        .sovereign_account;
      let trigger_fee = Pallet::<T>::trigger_fee_for_weight(
        ActorType::User,
        TriggerFamily::Manual,
        T::WeightInfo::manual_trigger(),
      )
      .trigger_fee;
      T::AssetOps::mint(&sovereign, T::FeeNativeAssetId::get(), trigger_fee)
        .expect("low-capture User Manual readiness is independently funded");
    }
    Pallet::<T>::manual_trigger(RawOrigin::Signed(owner).into(), actor_id).map_err(|_| {
      polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "maximum-length low-capture Contract failed Manual readiness",
      )
    })?;
    Pallet::<T>::execute_cycle(Weight::MAX);
    let run = ActorRunStateStore::<T>::get(actor_id).ok_or(
      polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "maximum-length low-capture Contract did not publish Running authority",
      ),
    )?;
    assert_eq!(run.cursor, 1);
    assert!(run.opening_snapshot.is_empty());
    assert!(run.opening_predicate_results.is_empty());
    assert!(run.funding_snapshot.is_empty());
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Ready { .. })
    ));
    if actor_type == ActorType::User {
      assert!(ActorStateHolds::<T>::contains_key(actor_id));
    }
    Ok(actor_id)
  }

  fn prepare_low_capture_running<T: Config>(
    current_step: Option<StepOf<T>>,
    cooldown_blocks: u32,
  ) -> Result<ActorId, polkadot_sdk::frame_benchmarking::BenchmarkError> {
    prepare_low_capture_running_with_type::<T>(ActorType::System, current_step, cooldown_blocks)
  }

  fn prepare_run_terminal_failure_source<T: Config>()
  -> Result<ActorId, polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let owner: T::AccountId = account("low-capture-run-owner", 0, 0);
    let (position, _) = T::BenchmarkHelper::setup_stake(&owner).map_err(|_| {
      polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot prepare an empty terminal-failure staking receipt",
      )
    })?;
    let receipt = T::StakingOps::share_asset(position).ok_or(
      polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host did not create an admissible terminal-failure staking receipt",
      ),
    )?;
    let actor_id = prepare_low_capture_running_with_type::<T>(
      ActorType::User,
      Some(Step {
        precondition: None,
        task: ActorTask::Unstake {
          asset: position,
          shares: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
        },
        on_error: StepErrorPolicy::AbortCycle,
      }),
      0,
    )?;
    assert!(
      ActorFunding::<T>::get(actor_id)
        .expect("terminal-failure funding authority exists")
        .funding_tracked_assets
        .contains(&receipt)
    );
    T::BenchmarkHelper::remove_empty_staking_receipt(&owner, position).map_err(|_| {
      polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot remove the empty terminal-failure staking receipt",
      )
    })?;
    assert!(T::StakingOps::share_asset(position).is_none());
    Ok(actor_id)
  }

  fn prepare_run_owner_suspension_source<T: Config>(
    dense: bool,
    cooldown_blocks: u32,
  ) -> Result<ActorId, polkadot_sdk::frame_benchmarking::BenchmarkError> {
    if dense && T::MaxContractSteps::get() < 3 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "dense Run owner fixture requires three Steps",
      ));
    }
    let asset_owner: T::AccountId = account("run-owner-retry-assets", 0, 0);
    let capacity = benchmark_predicate_capacity::<T>();
    let mut assets = T::BenchmarkHelper::setup_predicate_assets(
      &asset_owner,
      2 + if dense { capacity } else { 0 },
    )
    .map_err(|_| {
      polkadot_sdk::frame_benchmarking::BenchmarkError::Stop("host retry asset setup unsupported")
    })?;
    let retry_index = assets
      .iter()
      .position(|asset| *asset != T::FeeNativeAssetId::get())
      .ok_or(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "Run suspension fixture needs a non-native unfunded asset",
      ))?;
    let retry_asset = assets.remove(retry_index);
    assets.retain(|asset| *asset != T::FeeNativeAssetId::get());
    assets.truncate(if dense { capacity as usize } else { 0 });
    if dense && assets.len() != capacity as usize {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent dense suspension predicates",
      ));
    }
    let retry_step = Step {
      precondition: dense.then(|| {
        packed_predicate_clauses::<T>(
          assets
            .into_iter()
            .map(|asset| TimedPredicate {
              timing: ObservationTiming::Opening,
              predicate: Predicate::BalanceBelow {
                asset,
                threshold: One::one(),
              },
            })
            .collect(),
          T::MaxPredicatesPerClause::get(),
        )
      }),
      task: ActorTask::Transfer {
        to: account("run-owner-retry-recipient", 0, 0),
        asset: retry_asset,
        amount: AmountResolution::Fixed(One::one()),
      },
      on_error: StepErrorPolicy::RetryLater {
        max_attempts: T::MaxRetryAttempts::get(),
      },
    };
    let actor_id = if dense {
      prepare_reachable_running_with_cooldown::<T>(
        ActorType::System,
        3,
        Some((1, retry_step)),
        cooldown_blocks,
      )?
    } else {
      prepare_low_capture_running::<T>(Some(retry_step), cooldown_blocks)?
    };
    let state = Pallet::<T>::active_actor_state(actor_id).expect("real Running source exists");
    assert!(T::AssetOps::balance(&state.identity.sovereign_account, retry_asset).is_zero());
    Ok(actor_id)
  }

  fn prepare_run_owner_suspension<T: Config>(
    dense: bool,
  ) -> Result<ActorId, polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_run_owner_suspension_source::<T>(dense, 2)?;
    let run = ActorRunStateStore::<T>::get(actor_id).expect("real Running payload exists");
    frame_system::Pallet::<T>::set_block_number(run.eligible_at);
    Pallet::<T>::execute_cycle(Weight::MAX);
    let suspended = ActorRunStateStore::<T>::get(actor_id).expect("real suspension retains Run");
    assert_eq!(suspended.cursor, 1);
    assert_eq!(
      suspended.suspension,
      Some(SuspensionReason::FundingUnavailable)
    );
    if dense {
      assert_eq!(suspended.opening_snapshot.len() as u32, 4);
      assert_eq!(
        suspended.opening_predicate_results.len() as u32,
        3 * benchmark_predicate_capacity::<T>()
      );
    } else {
      assert!(suspended.opening_snapshot.is_empty());
      assert!(suspended.opening_predicate_results.is_empty());
    }
    frame_system::Pallet::<T>::set_block_number(suspended.eligible_at);
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    assert!(
      Pallet::<T>::drain_overdue_wakeups_cursor(suspended.eligible_at, &mut meter).ready_entries
        >= 1
    );
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Ready { .. })
    ));
    Ok(actor_id)
  }

  /// Diagnostic structural probe for a maximum-length Run with no Opening amount or predicate
  /// capture. Public creation, Manual readiness and normal Q1 execution remain in force.
  #[benchmark(extra)]
  fn run_low_capture_max_probe() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_low_capture_running::<T>(None, 0)?;
    #[block]
    {
      core::hint::black_box(Pallet::<T>::load_current_step_service_state(actor_id));
    }
    Ok(())
  }

  /// Candidate complete consumed-source successor for the largest admitted dense Running domain.
  /// The canonical Ready span is full of reachable tombstones, so ordinary Queue publication falls
  /// back to a new Block wakeup without reintroducing the consumed source. This remains diagnostic
  /// until EXP-0058/0059 accept the finite successor envelope.
  #[benchmark(extra, pov_mode = Measured)]
  fn run_successor_continued_wakeup_dense()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_reachable_running::<T>(3)?;
    let source = ActorRunStateStore::<T>::get(actor_id).expect("dense Running source exists");
    assert_eq!(source.cursor, 1);
    assert_eq!(source.opening_snapshot.len() as u32, 6);
    assert_eq!(
      source.opening_predicate_results.len() as u32,
      3 * benchmark_predicate_capacity::<T>()
    );
    let now = source.eligible_at;
    frame_system::Pallet::<T>::set_block_number(now);
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    benchmark_fixture_seed_saturated_ready_tombstones::<T>();
    #[block]
    {
      assert!(
        execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now).is_zero()
      );
    }
    let successor =
      ActorRunStateStore::<T>::get(actor_id).expect("continued dense successor retains Run");
    assert_eq!(successor.cursor, 2);
    assert_eq!(successor.last_committed_step_block, Some(now));
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Waiting {
        key: WakeupKey::Block(at),
        ..
      }) if at == successor.eligible_at
    ));
    assert_eq!(benchmark_fixture_ready_occupancy::<T>(), 0);
    Ok(())
  }

  /// Candidate complete consumed-source successor for the admitted maximum-length, low-capture
  /// Running domain. The canonical Ready span is full of reachable tombstones, so ordinary Queue
  /// publication falls back to a new Block wakeup without reintroducing the consumed source.
  /// This remains diagnostic until EXP-0058/0059 accept the finite successor envelope.
  #[benchmark(extra, pov_mode = Measured)]
  fn run_successor_continued_wakeup_low_capture()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_low_capture_running::<T>(None, 0)?;
    let now = ActorRunStateStore::<T>::get(actor_id)
      .expect("low-capture Running source exists")
      .eligible_at;
    frame_system::Pallet::<T>::set_block_number(now);
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    benchmark_fixture_seed_saturated_ready_tombstones::<T>();
    #[block]
    {
      assert!(
        execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now).is_zero()
      );
    }
    let successor =
      ActorRunStateStore::<T>::get(actor_id).expect("continued low-capture successor retains Run");
    assert_eq!(successor.cursor, 2);
    assert_eq!(successor.last_committed_step_block, Some(now));
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Waiting {
        key: WakeupKey::Block(at),
        ..
      }) if at == successor.eligible_at
    ));
    assert_eq!(benchmark_fixture_ready_occupancy::<T>(), 0);
    Ok(())
  }

  /// Candidate complete consumed-source successful terminal for the admitted maximum-length,
  /// low-capture User Running domain. StopCycle clears Run authority and publishes one Unsignaled
  /// primary while retaining exactly one post-publication state-hold reconciliation.
  #[benchmark(extra, pov_mode = Measured)]
  fn run_successor_completed_unsignaled_low_capture()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_low_capture_running_with_type::<T>(
      ActorType::User,
      Some(Step {
        precondition: None,
        task: ActorTask::StopCycle,
        on_error: StepErrorPolicy::AbortCycle,
      }),
      0,
    )?;
    let now = ActorRunStateStore::<T>::get(actor_id)
      .expect("low-capture Running completion source exists")
      .eligible_at;
    frame_system::Pallet::<T>::set_block_number(now);
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    let expected_effect = T::TaskEffectWeight::actual_effect_weight(
      &ActorTask::StopCycle,
      TaskEffectExecution::Invoked,
    )
    .expect("host supplies StopCycle effect evidence");
    #[block]
    {
      assert_eq!(
        execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now),
        expected_effect
      );
    }
    let successor =
      Pallet::<T>::active_actor_state(actor_id).expect("completed User authority remains active");
    assert!(successor.run_state.is_none());
    assert_eq!(successor.identity.cycle_nonce, 1);
    assert_eq!(successor.hot.cycle_state, CycleState::Idle);
    assert!(!successor.hot.pending_signal);
    assert_eq!(successor.hot.unsuccessful_attempt_streak, 0);
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Unsignaled)
    ));
    assert_eq!(benchmark_fixture_ready_occupancy::<T>(), 0);
    assert!(ActorStateHolds::<T>::contains_key(actor_id));
    Ok(())
  }

  /// Candidate complete consumed-source failed terminal for the admitted maximum-length,
  /// low-capture User Running domain. A receipt mapping removed after ordinary admission causes a
  /// permanent pre-effect failure, clears Run authority and publishes exactly one Unsignaled owner.
  #[benchmark(extra, pov_mode = Measured)]
  fn run_successor_failed_unsignaled_low_capture()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_run_terminal_failure_source::<T>()?;
    let now = ActorRunStateStore::<T>::get(actor_id)
      .expect("low-capture Running failure source exists")
      .eligible_at;
    frame_system::Pallet::<T>::set_block_number(now);
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    #[block]
    {
      assert!(
        execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now).is_zero()
      );
    }
    let successor =
      Pallet::<T>::active_actor_state(actor_id).expect("failed User authority remains active");
    assert!(successor.run_state.is_none());
    assert_eq!(successor.identity.cycle_nonce, 1);
    assert_eq!(successor.hot.cycle_state, CycleState::Idle);
    assert!(!successor.hot.pending_signal);
    assert_eq!(successor.hot.unsuccessful_attempt_streak, 1);
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Unsignaled)
    ));
    assert_eq!(benchmark_fixture_ready_occupancy::<T>(), 0);
    assert!(ActorStateHolds::<T>::contains_key(actor_id));
    Ok(())
  }

  /// Candidate complete consumed-source suspension for the admitted maximum-length, low-capture
  /// Running domain. The first retry has an exact-next-block eligibility and therefore publishes
  /// directly to Queue inside the snapshotted service cutoff.
  #[benchmark(extra, pov_mode = Measured)]
  fn run_successor_suspended_queue_low_capture()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_run_owner_suspension_source::<T>(false, 0)?;
    let now = ActorRunStateStore::<T>::get(actor_id)
      .expect("low-capture Running retry source exists")
      .eligible_at;
    frame_system::Pallet::<T>::set_block_number(now);
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    #[block]
    {
      assert!(
        execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now).is_zero()
      );
    }
    let successor =
      ActorRunStateStore::<T>::get(actor_id).expect("suspended low-capture successor retains Run");
    assert_eq!(successor.cursor, 1);
    assert_eq!(successor.last_attempt_block, now);
    assert_eq!(successor.eligible_at, now.saturating_add(One::one()));
    assert_eq!(
      successor.suspension,
      Some(SuspensionReason::FundingUnavailable)
    );
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Ready { .. })
    ));
    assert_eq!(benchmark_fixture_ready_occupancy::<T>(), 1);
    Ok(())
  }

  /// Candidate complete consumed-source suspension for the admitted maximum-length, low-capture
  /// Running domain. A two-block cooldown makes the retry ineligible inside the snapshotted cutoff,
  /// so the successor is published once into a new Block Waiting key.
  #[benchmark(extra, pov_mode = Measured)]
  fn run_successor_suspended_wakeup_low_capture()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_run_owner_suspension_source::<T>(false, 2)?;
    let now = ActorRunStateStore::<T>::get(actor_id)
      .expect("low-capture Running retry source exists")
      .eligible_at;
    frame_system::Pallet::<T>::set_block_number(now);
    let (state, admission, loaded_step) =
      benchmark_fixture_consume_frame_current_step_service_state::<T>(actor_id);
    #[block]
    {
      assert!(
        execute_reachable_step_inner::<T>(actor_id, state, admission, loaded_step, now).is_zero()
      );
    }
    let successor =
      ActorRunStateStore::<T>::get(actor_id).expect("suspended low-capture successor retains Run");
    assert_eq!(successor.cursor, 1);
    assert_eq!(successor.last_attempt_block, now);
    assert_eq!(successor.eligible_at, now.saturating_add(2u32.into()));
    assert_eq!(
      successor.suspension,
      Some(SuspensionReason::FundingUnavailable)
    );
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Waiting {
        key: WakeupKey::Block(at),
        ..
      }) if at == successor.eligible_at
    ));
    assert_eq!(benchmark_fixture_ready_occupancy::<T>(), 0);
    Ok(())
  }

  // Diagnostic overlap classifiers only: `write_run_state` starts from a retained Ready primary
  // and also maintains a Block wakeup reference. These profiles bound admitted payload extrema,
  // but they are not composable production Run primitives after Ready source consumption.
  #[benchmark(extra, pov_mode = Measured)]
  fn run_progress_dense() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_reachable_running::<T>(3)?;
    let state = ActorRunStateStore::<T>::get(actor_id).expect("dense Running state exists");
    #[block]
    {
      Pallet::<T>::persist_run_progress(actor_id, state)
        .expect("dense Running progress must persist");
    }
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn run_progress_low_capture() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_low_capture_running::<T>(None, 0)?;
    let state = ActorRunStateStore::<T>::get(actor_id).expect("low-capture Running state exists");
    #[block]
    {
      Pallet::<T>::persist_run_progress(actor_id, state)
        .expect("low-capture Running progress must persist");
    }
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn run_suspend_dense() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_run_owner_suspension::<T>(true)?;
    let state = ActorRunStateStore::<T>::get(actor_id).expect("dense Suspended state exists");
    #[block]
    {
      Pallet::<T>::persist_run_suspension(actor_id, state).expect("dense suspension must persist");
    }
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn run_suspend_low_capture() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_run_owner_suspension::<T>(false)?;
    let state = ActorRunStateStore::<T>::get(actor_id).expect("low-capture Suspended state exists");
    #[block]
    {
      Pallet::<T>::persist_run_suspension(actor_id, state)
        .expect("low-capture suspension must persist");
    }
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn run_complete_dense() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_reachable_running::<T>(3)?;
    #[block]
    {
      Pallet::<T>::write_run_state(actor_id, None).expect("dense completion must clear Run");
    }
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    Ok(())
  }

  #[benchmark(extra, pov_mode = Measured)]
  fn run_complete_low_capture() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_low_capture_running::<T>(None, 0)?;
    #[block]
    {
      Pallet::<T>::write_run_state(actor_id, None).expect("low-capture completion must clear Run");
    }
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    Ok(())
  }

  /// Measures persistence of a real Running state, not execution of its next Step.
  #[benchmark(pov_mode = Measured)]
  fn run_progress() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_reachable_running::<T>(T::MaxContractSteps::get())?;
    let state = ActorRunStateStore::<T>::get(actor_id).expect("real Running state exists");
    let eligible_at = state.eligible_at;
    let location = ActorControlLocators::<T>::get(actor_id);
    assert!(matches!(location, Some(ActorControlLocation::Ready { .. })));
    let retained = state.encode();
    #[block]
    {
      Pallet::<T>::persist_run_progress(actor_id, state)
        .expect("benchmark Running progress must persist");
    }
    assert_eq!(
      ActorRunStateStore::<T>::get(actor_id)
        .expect("Running state remains")
        .encode(),
      retained
    );
    assert!(
      benchmark_fixture_hot::<T>(actor_id)
        .is_some_and(|hot| hot.cycle_state == CycleState::Running)
    );
    assert_eq!(ActorControlLocators::<T>::get(actor_id), location);
    let pointer = benchmark_fixture_hot::<T>(actor_id)
      .and_then(|hot| hot.wakeup_pointer)
      .expect("Running temporal reference exists");
    assert_eq!(pointer.block, WakeupKey::Block(eligible_at));
    assert!(Pallet::<T>::wakeup_page_entry_matches(pointer, actor_id));
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real Running persistence passes full state audit");
    Ok(())
  }

  /// Measures persistence of a real suspension after due collection, not a new Task attempt.
  #[benchmark(pov_mode = Measured)]
  fn run_suspend() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let (actor_id, _) = prepare_reachable_suspended_head::<T>(T::MaxContractSteps::get() * 2, 0)?;
    let state = ActorRunStateStore::<T>::get(actor_id).expect("real Suspended state exists");
    let eligible_at = state.eligible_at;
    let location = ActorControlLocators::<T>::get(actor_id);
    assert!(matches!(location, Some(ActorControlLocation::Ready { .. })));
    let expected_event = Event::<T>::CycleSuspended {
      actor_id,
      cycle_nonce: state.cycle_nonce,
      cursor: state.cursor,
      reason: state
        .suspension
        .expect("real retry has a suspension reason"),
      cumulative_outcomes: state.cumulative_outcomes,
    };
    let retained = state.encode();
    frame_system::Pallet::<T>::reset_events();
    #[block]
    {
      Pallet::<T>::persist_run_suspension(actor_id, state)
        .expect("benchmark suspension must persist");
    }
    assert_eq!(
      ActorRunStateStore::<T>::get(actor_id)
        .expect("Suspended state remains")
        .encode(),
      retained
    );
    assert!(
      benchmark_fixture_hot::<T>(actor_id)
        .is_some_and(|hot| hot.cycle_state == CycleState::Suspended)
    );
    assert_eq!(ActorControlLocators::<T>::get(actor_id), location);
    let pointer = benchmark_fixture_hot::<T>(actor_id)
      .and_then(|hot| hot.wakeup_pointer)
      .expect("Suspended temporal reference exists");
    assert_eq!(pointer.block, WakeupKey::Block(eligible_at));
    assert!(Pallet::<T>::wakeup_page_entry_matches(pointer, actor_id));
    frame_system::Pallet::<T>::assert_last_event(expected_event.into());
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real suspension persistence passes full state audit");
    Ok(())
  }

  /// Measures Run removal/Idle publication, not execution of a final Task.
  #[benchmark(pov_mode = Measured)]
  fn run_complete() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let actor_id = prepare_reachable_running::<T>(T::MaxContractSteps::get())?;
    let state = Pallet::<T>::active_actor_state(actor_id).expect("real completion state exists");
    let run = state
      .run_state
      .as_ref()
      .expect("real completion Run exists");
    assert_eq!(
      run.opening_snapshot.len(),
      Pallet::<T>::opening_surfaces(&state.contract.steps, 0).len()
    );
    assert_eq!(
      run.opening_predicate_results.len() as u32,
      state
        .contract
        .steps
        .iter()
        .map(|step| step
          .precondition
          .as_ref()
          .map_or(0, Precondition::opening_predicate_count))
        .sum::<u32>()
    );
    #[block]
    {
      Pallet::<T>::write_run_state(actor_id, None)
        .expect("benchmark completion must clear Actor run");
    }
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    let (_, completed) = Pallet::<T>::load_primary_control_cell(actor_id)
      .expect("benchmark completed primary remains");
    assert_eq!(completed.hot.cycle_state, CycleState::Idle);
    assert_eq!(completed.identity.cycle_nonce, run.cycle_nonce);
    assert!(matches!(
      ActorControlLocators::<T>::get(actor_id),
      Some(ActorControlLocation::Unsignaled)
    ));
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("completed real Run passes full state audit");
    Ok(())
  }

  #[benchmark]
  fn run_cancel() -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    if T::MaxRetryAttempts::get() < 3 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "middle-page cancellation requires two nonterminal retries",
      ));
    }
    let page_size = 32u32;
    let (actor_id, funding) =
      create_reachable_manual_retry::<T>(T::MaxContractSteps::get() * 2, 0, 0)?;
    let second_attempt = frame_system::Pallet::<T>::block_number().saturating_add(2u32.into());
    let wakeup_block = Pallet::<T>::suspension_eligible_at(0, None, second_attempt, 2)
      .expect("second real retry target is representable");
    for i in 0..page_size {
      create_reachable_waiting_guard::<T>(i, wakeup_block);
    }
    let mut middle_fillers = alloc::vec::Vec::with_capacity(page_size.saturating_sub(1) as usize);
    for i in 0..page_size.saturating_sub(1) {
      let filler = create_reachable_waiting_guard::<T>(page_size + i, wakeup_block);
      middle_fillers.push(filler);
    }
    open_reachable_retry::<T>(actor_id, funding);
    assert_reachable_retry::<T>(actor_id);
    let run = ActorRunStateStore::<T>::get(actor_id).expect("real cancellation Run exists");
    assert_eq!(run.unsuccessful_attempts_at_cursor, 2);
    assert_eq!(run.eligible_at, wakeup_block);
    create_reachable_waiting_guard::<T>(page_size * 2, wakeup_block);
    for filler in middle_fillers {
      Pallet::<T>::close_actor(RawOrigin::Root.into(), filler)
        .expect("real guard close unlinks its Waiting entry");
    }
    let owner = Pallet::<T>::actor_identity(actor_id)
      .expect("target identity exists")
      .owner;
    Pallet::<T>::manual_trigger(RawOrigin::Signed(owner).into(), actor_id)
      .expect("second real occurrence retains a deferred latch");
    assert_eq!(
      ActorRunStateStore::<T>::get(actor_id)
        .expect("deferred Run exists")
        .encode(),
      run.encode()
    );
    let middle = ActorWaitingFrameChunks::<T>::get((WakeupKey::Block(wakeup_block), 1))
      .expect("cancel measures unlink of a live middle Waiting page");
    assert_eq!(
      (middle.previous_page, middle.next_page, middle.live_entries),
      (Some(0), Some(2), 1)
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real middle-page cancellation state is valid");
    #[extrinsic_call]
    cancel_run(RawOrigin::Root, actor_id);
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    assert!(
      Pallet::<T>::active_actor_view(actor_id).is_some_and(|actor| {
        actor.cycle_state == CycleState::Idle
          && actor.pending_signal
          && actor.queue_ticket.is_some()
      })
    );
    assert!(!ActorWaitingFrameChunks::<T>::contains_key((
      WakeupKey::Block(wakeup_block),
      1,
    )));
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real cancellation preserves canonical topology");
    Ok(())
  }

  /// Measures one affected User ObservationChange occurrence independently from publisher ingress
  /// and fanout traversal: exact Trigger collection, readiness materialization, and canonical
  /// placement are the complete Actor-owned boundary.
  #[benchmark(pov_mode = Measured)]
  fn observation_change_trigger_occurrence() {
    let owner: T::AccountId = whitelisted_caller();
    let recipient: T::AccountId = account("observation-occurrence-recipient", 0, 0);
    let feed = observation_feed_pool::<T>(1)[0];
    let contract_steps = make_contract_steps::<T>(recipient);
    ensure_creation_balance::<T>(&owner);
    prefund_active_user_creation::<T>(&owner, &contract_steps);
    Pallet::<T>::create_user_actor(
      RawOrigin::Signed(owner).into(),
      Mutability::Mutable,
      user_contract::<T>(
        Schedule {
          trigger: observation_trigger::<T>(feed),
          cooldown_blocks: 0,
        },
        contract_steps,
      ),
    )
    .expect("ObservationChange benchmark Actor exists");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    #[block]
    {
      assert!(
        Pallet::<T>::signal_observation_subscriber(
          actor_id,
          feed,
          TriggerCauseProvenance::Deferred,
          0,
        )
        .expect("ObservationChange occurrence commits")
      );
    }
    let hot = benchmark_fixture_hot::<T>(actor_id).expect("ObservationChange Actor remains active");
    assert!(hot.pending_signal);
    assert!(hot.queue_ticket.is_some() || hot.wakeup_pointer.is_some());
  }

  #[benchmark]
  fn observation_change_ingress() {
    let owner: T::AccountId = account("observation-ingress", 0, 0);
    let mut feeds = T::BenchmarkHelper::setup_observation_feeds(2)
      .expect("observation benchmark feeds must be available")
      .into_iter();
    let seeded_feed = feeds.next().expect("seed observation feed is required");
    let feed = feeds.next().expect("measured observation feed is required");
    let _ = bench_create_system_observation::<T>(owner.clone(), seeded_feed);
    let _ = bench_create_system_observation::<T>(owner, feed);
    Pallet::<T>::note_observation_changed(seeded_feed, 1)
      .expect("seed observation change ingress must succeed");
    #[block]
    {
      Pallet::<T>::note_observation_changed(feed, 1)
        .expect("observation change ingress must succeed");
    }
    let state = DirtyObservationFeeds::<T>::get(feed).expect("feed must be dirty");
    assert_eq!(state.latest_revision, 1);
    assert_eq!(state.previous_dirty_feed, Some(seeded_feed));
    assert_eq!(DirtyObservationListState::<T>::get().count, 2);
  }

  #[benchmark]
  fn observation_fanout_base() {
    #[block]
    {
      core::hint::black_box(ObservationFanoutWorkerFaultState::<T>::get());
      core::hint::black_box(Pallet::<T>::dirty_observation_fanout_base_probe());
    }
  }

  #[benchmark]
  fn observation_fanout_branch_probe() {
    let feed = T::BenchmarkHelper::setup_observation_feeds(1)
      .expect("observation benchmark feed must be available")
      .into_iter()
      .next()
      .expect("one observation benchmark feed is required");
    let owner: T::AccountId = account("observation-branch-probe", 0, 0);
    let _ = bench_create_system_observation::<T>(owner, feed);
    Pallet::<T>::note_observation_changed(feed, 1)
      .expect("observation change ingress must succeed");
    #[block]
    {
      core::hint::black_box(Pallet::<T>::observation_fanout_branch_probe());
    }
  }

  #[benchmark]
  fn observation_fanout_page() {
    let mut feeds = T::BenchmarkHelper::setup_observation_feeds(3)
      .expect("observation benchmark feeds must be available")
      .into_iter();
    let previous_feed = feeds.next().expect("previous observation feed is required");
    let feed = feeds.next().expect("measured observation feed is required");
    let next_feed = feeds.next().expect("next observation feed is required");
    let mut actors = alloc::vec::Vec::new();
    for index in 0..T::ObservationPageSize::get() {
      let owner: T::AccountId = account("observation-fanout", index, 0);
      actors.push(bench_create_user_observation_with_cooldown::<T>(
        owner, feed, 0,
      ));
    }
    let previous_owner: T::AccountId = account("observation-fanout-previous", 0, 0);
    let next_owner: T::AccountId = account("observation-fanout-next", 0, 0);
    let _ = bench_create_system_observation::<T>(previous_owner, previous_feed);
    let _ = bench_create_system_observation::<T>(next_owner, next_feed);
    Pallet::<T>::note_observation_changed(previous_feed, 1)
      .expect("previous observation change ingress must succeed");
    Pallet::<T>::note_observation_changed(feed, 1)
      .expect("measured observation change ingress must succeed");
    Pallet::<T>::note_observation_changed(next_feed, 1)
      .expect("next observation change ingress must succeed");
    DirtyObservationListState::<T>::mutate(|list| list.cursor = Some(feed));
    #[block]
    {
      Pallet::<T>::do_fanout_dirty_observation_page()
        .expect("dense observation fanout page must succeed");
    }
    assert!(DirtyObservationFeeds::<T>::get(feed).is_none());
    assert_eq!(DirtyObservationListState::<T>::get().count, 2);
    for actor_id in actors {
      assert!(benchmark_fixture_hot::<T>(actor_id).is_some_and(|hot| hot.pending_signal));
    }
  }

  #[benchmark]
  fn observation_fanout_wakeup_page() {
    let feed = T::BenchmarkHelper::setup_observation_feeds(1)
      .expect("observation benchmark feed must be available")
      .into_iter()
      .next()
      .expect("one observation benchmark feed is required");
    let mut actors = alloc::vec::Vec::new();
    for index in 0..T::ObservationPageSize::get() {
      let owner: T::AccountId = account("observation-wakeup", index, 0);
      let actor_id = bench_create_user_observation_with_cooldown::<T>(owner, feed, 100);
      benchmark_fixture_mutate_hot::<T>(actor_id, |hot| {
        hot.last_cycle_block = Some(One::one());
      });
      benchmark_fixture_align_primary_control::<T>(actor_id);
      actors.push(actor_id);
    }
    Pallet::<T>::note_observation_changed(feed, 1)
      .expect("observation change ingress must succeed");
    #[block]
    {
      Pallet::<T>::do_fanout_dirty_observation_page()
        .expect("observation wakeup page must succeed");
    }
    assert!(DirtyObservationFeeds::<T>::get(feed).is_none());
    for actor_id in actors {
      assert!(benchmark_fixture_hot::<T>(actor_id).is_some_and(|hot| {
        hot.pending_signal && hot.queue_ticket.is_none() && hot.wakeup_pointer.is_some()
      }));
    }
  }

  #[benchmark]
  fn observation_fanout_coalesced_page() {
    let feed = T::BenchmarkHelper::setup_observation_feeds(1)
      .expect("observation benchmark feed must be available")
      .into_iter()
      .next()
      .expect("one observation benchmark feed is required");
    let mut actors = alloc::vec::Vec::new();
    for index in 0..T::ObservationPageSize::get() {
      let owner: T::AccountId = account("observation-coalesced", index, 0);
      let actor_id = bench_create_user_observation_with_cooldown::<T>(owner, feed, 0);
      Pallet::<T>::request_observation_activation_compact(actor_id, feed)
        .expect("initial observation activation must succeed");
      actors.push(actor_id);
    }
    Pallet::<T>::note_observation_changed(feed, 1)
      .expect("observation change ingress must succeed");
    #[block]
    {
      Pallet::<T>::do_fanout_dirty_observation_page()
        .expect("coalesced observation page must succeed");
    }
    assert!(DirtyObservationFeeds::<T>::get(feed).is_none());
    for actor_id in actors {
      assert!(
        benchmark_fixture_hot::<T>(actor_id)
          .is_some_and(|hot| { hot.pending_signal && hot.queue_ticket.is_some() })
      );
    }
  }

  #[benchmark]
  fn observation_fanout_terminal() {
    let feed = T::BenchmarkHelper::setup_observation_feeds(1)
      .expect("observation benchmark feed must be available")
      .into_iter()
      .next()
      .expect("one observation benchmark feed is required");
    let owner: T::AccountId = account("observation-terminal", 0, 0);
    let (actor_id, end) = bench_create_expiring_system_observation::<T>(owner, feed);
    frame_system::Pallet::<T>::set_block_number(end.saturating_add(One::one()));
    Pallet::<T>::note_observation_changed(feed, 1)
      .expect("observation change ingress must succeed");
    Pallet::<T>::do_fanout_dirty_observation_page()
      .expect("ordinary turn must persist terminal branch");
    assert!(
      DirtyObservationFeeds::<T>::get(feed)
        .is_some_and(|state| { state.next_subscriber_branch == ObservationFanoutBranch::Terminal })
    );
    #[block]
    {
      Pallet::<T>::do_fanout_dirty_observation_page()
        .expect("terminal observation fanout must succeed");
    }
    assert!(benchmark_fixture_hot::<T>(actor_id).is_none());
    assert!(DirtyObservationFeeds::<T>::get(feed).is_none());
  }

  #[benchmark]
  fn observation_fanout_blocked_page() {
    let feed = T::BenchmarkHelper::setup_observation_feeds(1)
      .expect("observation benchmark feed must be available")
      .into_iter()
      .next()
      .expect("one observation benchmark feed is required");
    let mut actors = alloc::vec::Vec::new();
    for index in 0..T::ObservationPageSize::get() {
      let owner: T::AccountId = account("observation-fanout-blocked", index, 0);
      actors.push(bench_create_user_observation_with_cooldown::<T>(
        owner, feed, 0,
      ));
    }
    install_saturated_tombstone_queue::<T>();
    Pallet::<T>::note_observation_changed(feed, 1)
      .expect("observation change ingress must succeed");
    #[block]
    {
      Pallet::<T>::do_fanout_dirty_observation_page()
        .expect("blocked observation fanout page must remain retryable");
    }
    assert!(DirtyObservationFeeds::<T>::get(feed).is_none());
    for actor_id in actors {
      assert!(benchmark_fixture_hot::<T>(actor_id).is_some_and(|hot| {
        hot.pending_signal && hot.queue_ticket.is_none() && hot.wakeup_pointer.is_some()
      }));
    }
  }

  #[benchmark]
  fn crossing_worker_base() {
    #[block]
    {
      core::hint::black_box(CrossingWorkerFaultState::<T>::get());
      core::hint::black_box(CrossingPendingFeedListState::<T>::get().count);
    }
  }

  #[benchmark]
  fn crossing_work_probe() {
    let (feed, _) = prepare_crossing_work::<T>(2);
    while CrossingPendingFeedListState::<T>::get().count > 0 {
      Pallet::<T>::crossing_work_unit().expect("initial Crossing fire must drain");
    }
    clear_indexed_detection_disablement::<T>();
    Pallet::<T>::note_observation_transition(
      feed,
      ObservationTransition {
        revision: 3,
        previous: Some(2),
        current: 0,
      },
    )
    .expect("Crossing rearm transition must be admitted");
    CrossingRangeCursors::<T>::insert(
      feed,
      CrossingRangeCursor {
        revision: 3,
        traversal: CrossingTraversal::Downward,
        search_bound: 0,
        current_threshold: Some(0),
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    #[block]
    {
      assert_eq!(
        Pallet::<T>::classify_crossing_work_preflight(),
        CrossingWorkPlan::RearmCohort
      );
    }
  }

  /// Measures one affected User ObservationCrossing fire independently from observation
  /// publication: threshold traversal, phase movement, exact Trigger collection, and canonical
  /// readiness placement are the complete Actor-owned occurrence boundary.
  #[benchmark(pov_mode = Measured)]
  fn observation_crossing_trigger_occurrence() {
    let (feed, actor_id) = prepare_crossing_work::<T>(2);
    CrossingRangeCursors::<T>::insert(
      feed,
      CrossingRangeCursor {
        revision: 2,
        traversal: CrossingTraversal::Upward,
        search_bound: 2,
        current_threshold: Some(2),
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    #[block]
    {
      Pallet::<T>::crossing_work_unit().expect("Crossing occurrence commits");
    }
    let hot = benchmark_fixture_hot::<T>(actor_id).expect("Crossing Actor remains active");
    assert!(hot.pending_signal);
    assert!(hot.queue_ticket.is_some() || hot.wakeup_pointer.is_some());
  }

  #[benchmark]
  fn crossing_search_probe() {
    let (feed, _) = prepare_crossing_work::<T>(2);
    #[block]
    {
      assert_eq!(
        Pallet::<T>::crossing_radix_min_ge(feed, CrossingTraversal::Upward, 0, 0, 0, u128::MAX,),
        Ok(Some(2))
      );
    }
  }

  #[benchmark]
  fn crossing_fire_probe() {
    let (feed, _) = prepare_crossing_work::<T>(2);
    CrossingRangeCursors::<T>::insert(
      feed,
      CrossingRangeCursor {
        revision: 2,
        traversal: CrossingTraversal::Upward,
        search_bound: 2,
        current_threshold: Some(2),
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    #[block]
    {
      assert!(matches!(
        Pallet::<T>::classify_crossing_work(),
        CrossingWorkPlan::FireCohortPlaced
          | CrossingWorkPlan::FireCohortCoalesced
          | CrossingWorkPlan::FireCohortClosed
      ));
    }
  }

  #[benchmark]
  fn crossing_fire_pair_probe() {
    let (feed, _) = prepare_crossing_work::<T>(2);
    for index in 0..31 {
      let owner: T::AccountId = account("crossing-pair-queue-boundary", index, 0);
      let actor_id = bench_create_user_with_trigger::<T>(owner, Trigger::manual());
      Pallet::<T>::request_activation(actor_id).expect("queue-boundary actor activation");
    }
    assert_eq!(benchmark_fixture_ready_occupancy::<T>(), 31);
    let second_owner: T::AccountId = account("crossing-pair-probe", 0, 0);
    let _ = bench_create_user_with_trigger::<T>(
      second_owner,
      Trigger::observation_crossing(feed, CrossingDirection::Rising, 2, 0),
    );
    CrossingRangeCursors::<T>::insert(
      feed,
      CrossingRangeCursor {
        revision: 2,
        traversal: CrossingTraversal::Upward,
        search_bound: 2,
        current_threshold: Some(2),
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    #[block]
    {
      assert_eq!(
        Pallet::<T>::classify_crossing_work(),
        CrossingWorkPlan::FireCohortPlacedBatch
      );
    }
  }

  /// Measures the fixed, storage-free admission-selection arithmetic executed after
  /// classification: plan-weight lookup, fault-reserve comparison, the retained pair-fallback
  /// preference, the scalar fallback, the component checks and the final reservation check.
  /// The fixture pins the worst-case path: a classified multi-candidate placed batch whose
  /// aggregate and pair reservations miss while the scalar single fits.
  #[benchmark]
  fn crossing_selection_probe() {
    let plan = CrossingWorkPlan::FireCohortPlacedBatch;
    let admitted_candidates = T::MaxCrossingActorsPerBlock::get().max(3);
    let pair_weight = Pallet::<T>::crossing_plan_weight_for_admission(plan, 2)
      .expect("the placed batch owns a generated pair admission weight");
    let single_plan = Pallet::<T>::crossing_single_candidate_plan(plan)
      .expect("the placed batch owns a scalar single fallback");
    let single_weight = Pallet::<T>::crossing_plan_weight_for_admission(single_plan, 1)
      .expect("the scalar single owns a generated admission weight");
    let fault_weight = T::WeightInfo::record_crossing_worker_fault();
    assert!(
      pair_weight.ref_time() > single_weight.ref_time(),
      "the worst-case selection fixture requires the pair reservation to miss before the single fits"
    );
    let meter =
      polkadot_sdk::sp_weights::WeightMeter::with_limit(single_weight.saturating_add(fault_weight));
    let counters = crate::crossing::CrossingWorkCounters::default();
    #[block]
    {
      let selection =
        Pallet::<T>::select_crossing_admission(plan, admitted_candidates, false, &counters, &meter);
      assert!(matches!(
        selection,
        crate::crossing::CrossingAdmissionSelection::Admitted {
          plan: CrossingWorkPlan::FireCohortPlaced,
          admitted_candidates: 1,
          ..
        }
      ));
      core::hint::black_box(selection);
    }
  }

  #[benchmark]
  fn crossing_tail_refill_probe() {
    let (feed, first) = prepare_crossing_work::<T>(2);
    for index in 1..8 {
      let owner: T::AccountId = account("crossing-tail-user", index, 0);
      let _ = bench_create_user_with_trigger::<T>(
        owner,
        Trigger::observation_crossing(feed, CrossingDirection::Rising, 2, 0),
      );
    }
    let system_count = T::CrossingPageSize::get()
      .saturating_add(4)
      .saturating_sub(8);
    for index in 0..system_count {
      let owner: T::AccountId = account("crossing-tail-system", index, 0);
      let _ = bench_create_system_crossing::<T>(owner, feed, 2);
    }
    let locator = CrossingMemberships::<T>::get(first).expect("first Crossing locator");
    let state = CrossingLeafStates::<T>::get(locator.key).expect("Crossing leaf state");
    let source = CrossingMemberPages::<T>::get(locator.key, locator.page)
      .expect("non-tail Crossing source page");
    assert!(locator.page < state.tail_page);
    #[block]
    {
      let tail = CrossingMemberPages::<T>::get(locator.key, state.tail_page)
        .expect("Crossing tail refill page");
      assert_eq!(
        Pallet::<T>::crossing_source_cohort_count(
          &source,
          locator.offset,
          4,
          Some(tail.entries.len() as u32),
        ),
        4
      );
    }
  }

  #[benchmark]
  fn crossing_fire_cohort_preflight(c: Linear<1, { CROSSING_COHORT_BENCHMARK_MAX }>) {
    let (feed, first) = prepare_crossing_work::<T>(2);
    for index in 1..c {
      let owner: T::AccountId = account("crossing-cohort-preflight", index, 0);
      let _ = bench_create_user_with_trigger::<T>(
        owner,
        Trigger::observation_crossing(feed, CrossingDirection::Rising, 2, 0),
      );
    }
    let locator = CrossingMemberships::<T>::get(first).expect("first Crossing locator");
    let page = CrossingMemberPages::<T>::get(locator.key, locator.page)
      .expect("Crossing cohort source page");
    let transition = CrossingTransitionObligation {
      revision: 2,
      previous: 0,
      current: 2,
      cause_provenance: TriggerCauseProvenance::Deferred,
      cause_block: 0,
    };
    #[block]
    {
      let snapshot = Pallet::<T>::snapshot_crossing_source_prefix(
        locator.key,
        locator.page,
        &page,
        locator.offset,
        c,
      )
      .expect("bounded cohort snapshot");
      let preflight = Pallet::<T>::preflight_crossing_cohort(
        &snapshot,
        transition,
        crate::crossing::CrossingFireClassification::Resolve,
        None,
      )
      .expect("homogeneous cohort preflight");
      assert_eq!(preflight.plan, CrossingWorkPlan::FireCohortPlaced);
      assert_eq!(preflight.admitted_candidates, c);
    }
  }

  #[benchmark]
  fn crossing_coalesced_cohort_preflight(c: Linear<1, { CROSSING_COHORT_BENCHMARK_MAX }>) {
    let (feed, first) = prepare_crossing_work::<T>(2);
    let mut actors = alloc::vec![first];
    for index in 1..c {
      let owner: T::AccountId = account("crossing-coalesced-cohort", index, 0);
      actors.push(bench_create_user_with_trigger::<T>(
        owner,
        Trigger::observation_crossing(feed, CrossingDirection::Rising, 2, 0),
      ));
    }
    for actor_id in actors {
      Pallet::<T>::request_activation(actor_id).expect("coalesced cohort activation");
    }
    let locator = CrossingMemberships::<T>::get(first).expect("first Crossing locator");
    let page = CrossingMemberPages::<T>::get(locator.key, locator.page)
      .expect("Crossing cohort source page");
    let transition = CrossingTransitionObligation {
      revision: 2,
      previous: 0,
      current: 2,
      cause_provenance: TriggerCauseProvenance::Deferred,
      cause_block: 0,
    };
    #[block]
    {
      let snapshot = Pallet::<T>::snapshot_crossing_source_prefix(
        locator.key,
        locator.page,
        &page,
        locator.offset,
        c,
      )
      .expect("bounded cohort snapshot");
      let preflight = Pallet::<T>::preflight_crossing_cohort(
        &snapshot,
        transition,
        crate::crossing::CrossingFireClassification::Resolve,
        Some(CrossingWorkPlan::FireCohortCoalesced),
      )
      .expect("homogeneous coalesced preflight");
      assert_eq!(preflight.plan, CrossingWorkPlan::FireCohortCoalesced);
      assert_eq!(preflight.admitted_candidates, c);
    }
  }

  #[benchmark]
  fn crossing_terminal_cohort_preflight(c: Linear<1, { CROSSING_COHORT_BENCHMARK_MAX }>) {
    let (feed, first) = prepare_crossing_work::<T>(2);
    for index in 1..c {
      let owner: T::AccountId = account("crossing-terminal-cohort", index, 0);
      let _ = bench_create_user_with_trigger::<T>(
        owner,
        Trigger::observation_crossing(feed, CrossingDirection::Rising, 2, 0),
      );
    }
    benchmark_fixture_set_next_ready_ticket::<T>(u64::MAX);
    let locator = CrossingMemberships::<T>::get(first).expect("first Crossing locator");
    let page = CrossingMemberPages::<T>::get(locator.key, locator.page)
      .expect("Crossing cohort source page");
    let transition = CrossingTransitionObligation {
      revision: 2,
      previous: 0,
      current: 2,
      cause_provenance: TriggerCauseProvenance::Deferred,
      cause_block: 0,
    };
    #[block]
    {
      let snapshot = Pallet::<T>::snapshot_crossing_source_prefix(
        locator.key,
        locator.page,
        &page,
        locator.offset,
        c,
      )
      .expect("bounded cohort snapshot");
      let preflight = Pallet::<T>::preflight_crossing_cohort(
        &snapshot,
        transition,
        crate::crossing::CrossingFireClassification::Resolve,
        Some(CrossingWorkPlan::FireCohortClosed),
      )
      .expect("homogeneous terminal preflight");
      assert_eq!(preflight.plan, CrossingWorkPlan::FireCohortClosed);
      assert_eq!(preflight.admitted_candidates, c);
    }
  }

  #[benchmark]
  fn crossing_skip_cohort_preflight(c: Linear<1, { CROSSING_COHORT_BENCHMARK_MAX }>) {
    let (feed, first) = prepare_crossing_work::<T>(2);
    let mut actors = alloc::vec![first];
    for index in 1..c {
      let owner: T::AccountId = account("crossing-skip-cohort", index, 0);
      actors.push(bench_create_user_with_trigger::<T>(
        owner,
        Trigger::observation_crossing(feed, CrossingDirection::Rising, 2, 0),
      ));
    }
    for actor_id in actors {
      benchmark_fixture_mutate_hot::<T>(actor_id, |hot| {
        let TriggerRuntimeState::ObservationCrossing {
          installed_at_revision,
          ..
        } = &mut hot.trigger_runtime_state
        else {
          panic!("Crossing runtime state")
        };
        *installed_at_revision = 2;
      });
      benchmark_fixture_align_primary_control::<T>(actor_id);
    }
    let locator = CrossingMemberships::<T>::get(first).expect("first Crossing locator");
    let page = CrossingMemberPages::<T>::get(locator.key, locator.page)
      .expect("Crossing cohort source page");
    let transition = CrossingTransitionObligation {
      revision: 2,
      previous: 0,
      current: 2,
      cause_provenance: TriggerCauseProvenance::Deferred,
      cause_block: 0,
    };
    #[block]
    {
      let snapshot = Pallet::<T>::snapshot_crossing_source_prefix(
        locator.key,
        locator.page,
        &page,
        locator.offset,
        c,
      )
      .expect("bounded cohort snapshot");
      let preflight = Pallet::<T>::preflight_crossing_cohort(
        &snapshot,
        transition,
        crate::crossing::CrossingFireClassification::Resolve,
        Some(CrossingWorkPlan::SkipPostInstallationTransition),
      )
      .expect("homogeneous skip preflight");
      assert_eq!(
        preflight.plan,
        CrossingWorkPlan::SkipPostInstallationTransition
      );
      assert_eq!(preflight.admitted_candidates, c);
    }
  }

  #[benchmark]
  fn crossing_rearm_cohort_preflight(c: Linear<1, { CROSSING_COHORT_BENCHMARK_MAX }>) {
    let (feed, first) = prepare_crossing_work::<T>(2);
    for index in 1..c {
      let owner: T::AccountId = account("crossing-rearm-cohort", index, 0);
      let _ = bench_create_user_with_trigger::<T>(
        owner,
        Trigger::observation_crossing(feed, CrossingDirection::Rising, 2, 0),
      );
    }
    while CrossingPendingFeedListState::<T>::get().count > 0 {
      Pallet::<T>::crossing_work_unit().expect("initial Crossing fire must drain");
    }
    clear_indexed_detection_disablement::<T>();
    Pallet::<T>::note_observation_transition(
      feed,
      ObservationTransition {
        revision: 3,
        previous: Some(2),
        current: 0,
      },
    )
    .expect("Crossing rearm transition must be admitted");
    let locator = CrossingMemberships::<T>::get(first).expect("first Crossing locator");
    let page = CrossingMemberPages::<T>::get(locator.key, locator.page)
      .expect("Crossing cohort source page");
    let transition = CrossingTransitionObligation {
      revision: 3,
      previous: 2,
      current: 0,
      cause_provenance: TriggerCauseProvenance::Deferred,
      cause_block: 0,
    };
    #[block]
    {
      let snapshot = Pallet::<T>::snapshot_crossing_source_prefix(
        locator.key,
        locator.page,
        &page,
        locator.offset,
        c,
      )
      .expect("bounded cohort snapshot");
      let preflight = Pallet::<T>::preflight_crossing_cohort(
        &snapshot,
        transition,
        crate::crossing::CrossingFireClassification::Resolve,
        Some(CrossingWorkPlan::RearmCohort),
      )
      .expect("homogeneous rearm preflight");
      assert_eq!(preflight.plan, CrossingWorkPlan::RearmCohort);
      assert_eq!(preflight.admitted_candidates, c);
    }
  }

  #[benchmark]
  fn crossing_rearm_pair_probe() {
    let (feed, _) = prepare_crossing_work::<T>(2);
    let second_owner: T::AccountId = account("crossing-rearm-pair-probe", 0, 0);
    let _ = bench_create_user_with_trigger::<T>(
      second_owner,
      Trigger::observation_crossing(feed, CrossingDirection::Rising, 2, 0),
    );
    while CrossingPendingFeedListState::<T>::get().count > 0 {
      Pallet::<T>::crossing_work_unit().expect("initial pair fire must drain");
    }
    clear_indexed_detection_disablement::<T>();
    Pallet::<T>::note_observation_transition(
      feed,
      ObservationTransition {
        revision: 3,
        previous: Some(2),
        current: 0,
      },
    )
    .expect("pair rearm transition must be admitted");
    CrossingRangeCursors::<T>::insert(
      feed,
      CrossingRangeCursor {
        revision: 3,
        traversal: CrossingTraversal::Downward,
        search_bound: 0,
        current_threshold: Some(0),
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    #[block]
    {
      assert_eq!(
        Pallet::<T>::classify_crossing_work(),
        CrossingWorkPlan::RearmCohortPair
      );
    }
  }

  #[benchmark]
  fn crossing_skip_pair_probe() {
    let (feed, first_actor) = prepare_crossing_work::<T>(2);
    let second_owner: T::AccountId = account("crossing-skip-pair-probe", 0, 0);
    let second_actor = bench_create_user_with_trigger::<T>(
      second_owner,
      Trigger::observation_crossing(feed, CrossingDirection::Rising, 2, 0),
    );
    for actor_id in [first_actor, second_actor] {
      benchmark_fixture_mutate_hot::<T>(actor_id, |hot| {
        let TriggerRuntimeState::ObservationCrossing {
          installed_at_revision,
          ..
        } = &mut hot.trigger_runtime_state
        else {
          panic!("pair Actor must use Crossing state");
        };
        *installed_at_revision = 2;
      });
    }
    CrossingRangeCursors::<T>::insert(
      feed,
      CrossingRangeCursor {
        revision: 2,
        traversal: CrossingTraversal::Upward,
        search_bound: 2,
        current_threshold: Some(2),
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    for actor_id in [first_actor, second_actor] {
      benchmark_fixture_align_primary_control::<T>(actor_id);
    }
    #[block]
    {
      assert_eq!(
        Pallet::<T>::classify_crossing_work(),
        CrossingWorkPlan::SkipPostInstallationPair
      );
    }
  }

  #[benchmark]
  fn crossing_transition_unit() {
    let (feed, _) = prepare_crossing_work::<T>(u128::MAX);
    #[block]
    {
      Pallet::<T>::crossing_work_unit().expect("no-match Crossing work must succeed");
    }
    assert!(CrossingRangeCursors::<T>::get(feed).is_some_and(|cursor| cursor.exhausted));
  }

  #[benchmark]
  fn crossing_leaf_unit() {
    let (_, actor_id) = prepare_crossing_work::<T>(2);
    #[block]
    {
      Pallet::<T>::crossing_work_unit().expect("matched Crossing leaf work must succeed");
    }
    assert!(benchmark_fixture_hot::<T>(actor_id).is_some_and(|hot| hot.pending_signal));
  }

  #[benchmark]
  fn crossing_page_unit() {
    let (_, actor_id) = prepare_crossing_work::<T>(2);
    #[block]
    {
      Pallet::<T>::crossing_work_unit().expect("matched Crossing page work must succeed");
    }
    assert!(matches!(
      benchmark_fixture_hot::<T>(actor_id).map(|hot| hot.trigger_runtime_state),
      Some(TriggerRuntimeState::ObservationCrossing {
        phase: CrossingPhase::WaitingForRearm,
        ..
      })
    ));
  }

  #[benchmark]
  fn crossing_rearm_unit() {
    let (feed, actor_id) = prepare_crossing_work::<T>(2);
    while CrossingPendingFeedListState::<T>::get().count > 0 {
      Pallet::<T>::crossing_work_unit().expect("initial Crossing fire must drain");
    }
    clear_indexed_detection_disablement::<T>();
    Pallet::<T>::note_observation_transition(
      feed,
      ObservationTransition {
        revision: 3,
        previous: Some(2),
        current: 0,
      },
    )
    .expect("Crossing rearm transition must be admitted");
    #[block]
    {
      Pallet::<T>::crossing_work_unit().expect("Crossing rearm work must succeed");
    }
    assert!(matches!(
      benchmark_fixture_hot::<T>(actor_id).map(|hot| hot.trigger_runtime_state),
      Some(TriggerRuntimeState::ObservationCrossing {
        phase: CrossingPhase::Armed,
        ..
      })
    ));
  }

  #[benchmark]
  fn crossing_rearm_pair_unit() {
    let (feed, first_actor) = prepare_crossing_work::<T>(2);
    let second_owner: T::AccountId = account("crossing-rearm-pair-unit", 0, 0);
    let second_actor = bench_create_user_with_trigger::<T>(
      second_owner,
      Trigger::observation_crossing(feed, CrossingDirection::Rising, 2, 0),
    );
    while CrossingPendingFeedListState::<T>::get().count > 0 {
      Pallet::<T>::crossing_work_unit().expect("initial pair fire must drain");
    }
    clear_indexed_detection_disablement::<T>();
    Pallet::<T>::note_observation_transition(
      feed,
      ObservationTransition {
        revision: 3,
        previous: Some(2),
        current: 0,
      },
    )
    .expect("pair rearm transition must be admitted");
    CrossingRangeCursors::<T>::insert(
      feed,
      CrossingRangeCursor {
        revision: 3,
        traversal: CrossingTraversal::Downward,
        search_bound: 0,
        current_threshold: Some(0),
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    #[block]
    {
      Pallet::<T>::crossing_pair_work_unit().expect("Crossing rearm pair must succeed");
    }
    for actor_id in [first_actor, second_actor] {
      assert!(matches!(
        benchmark_fixture_hot::<T>(actor_id).map(|hot| hot.trigger_runtime_state),
        Some(TriggerRuntimeState::ObservationCrossing {
          phase: CrossingPhase::Armed,
          ..
        })
      ));
    }
  }

  #[benchmark]
  fn crossing_coalesced_unit() {
    let (_, actor_id) = prepare_crossing_work::<T>(2);
    Pallet::<T>::request_activation(actor_id).expect("benchmark actor activation must succeed");
    #[block]
    {
      Pallet::<T>::crossing_work_unit().expect("coalesced Crossing fire must succeed");
    }
    assert!(benchmark_fixture_hot::<T>(actor_id).is_some_and(|hot| {
      hot.pending_signal
        && hot.queue_ticket.is_some()
        && matches!(
          hot.trigger_runtime_state,
          TriggerRuntimeState::ObservationCrossing {
            phase: CrossingPhase::WaitingForRearm,
            ..
          }
        )
    }));
  }

  #[benchmark]
  fn crossing_coalesced_pair_unit() {
    let (feed, first_actor) = prepare_crossing_work::<T>(2);
    let second_owner: T::AccountId = account("crossing-coalesced-pair", 0, 0);
    let second_actor = bench_create_user_with_trigger::<T>(
      second_owner,
      Trigger::observation_crossing(feed, CrossingDirection::Rising, 2, 0),
    );
    Pallet::<T>::request_activation(first_actor).expect("first pair latch must succeed");
    Pallet::<T>::request_activation(second_actor).expect("second pair latch must succeed");
    CrossingRangeCursors::<T>::insert(
      feed,
      CrossingRangeCursor {
        revision: 2,
        traversal: CrossingTraversal::Upward,
        search_bound: 2,
        current_threshold: Some(2),
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    assert_eq!(
      Pallet::<T>::classify_crossing_work(),
      CrossingWorkPlan::FireCohortCoalescedPair
    );
    #[block]
    {
      Pallet::<T>::crossing_pair_work_unit().expect("coalesced Crossing pair must succeed");
    }
    for actor_id in [first_actor, second_actor] {
      assert!(benchmark_fixture_hot::<T>(actor_id).is_some_and(|hot| {
        hot.pending_signal
          && hot.queue_ticket.is_some()
          && matches!(
            hot.trigger_runtime_state,
            TriggerRuntimeState::ObservationCrossing {
              phase: CrossingPhase::WaitingForRearm,
              ..
            }
          )
      }));
    }
  }

  #[benchmark]
  fn crossing_placed_unit() {
    let (_, actor_id) = prepare_crossing_work::<T>(2);
    #[block]
    {
      Pallet::<T>::crossing_work_unit().expect("placed Crossing fire must succeed");
    }
    assert!(benchmark_fixture_hot::<T>(actor_id).is_some_and(|hot| {
      hot.pending_signal
        && hot.queue_ticket.is_some()
        && matches!(
          hot.trigger_runtime_state,
          TriggerRuntimeState::ObservationCrossing {
            phase: CrossingPhase::WaitingForRearm,
            ..
          }
        )
    }));
  }

  #[benchmark]
  fn crossing_placed_pair_unit() {
    let (feed, first_actor) = prepare_crossing_work::<T>(2);
    let second_owner: T::AccountId = account("crossing-pair-unit", 0, 0);
    let second_actor = bench_create_user_with_trigger::<T>(
      second_owner,
      Trigger::observation_crossing(feed, CrossingDirection::Rising, 2, 0),
    );
    CrossingRangeCursors::<T>::insert(
      feed,
      CrossingRangeCursor {
        revision: 2,
        traversal: CrossingTraversal::Upward,
        search_bound: 2,
        current_threshold: Some(2),
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    #[block]
    {
      Pallet::<T>::crossing_placed_batch_work_unit(2).expect("placed Crossing pair must succeed");
    }
    for actor_id in [first_actor, second_actor] {
      assert!(benchmark_fixture_hot::<T>(actor_id).is_some_and(|hot| {
        hot.pending_signal
          && hot.queue_ticket.is_some()
          && matches!(
            hot.trigger_runtime_state,
            TriggerRuntimeState::ObservationCrossing {
              phase: CrossingPhase::WaitingForRearm,
              ..
            }
          )
      }));
    }
  }

  #[benchmark]
  fn crossing_placed_maximum_unit() {
    let (feed, first_actor) = prepare_crossing_work::<T>(2);
    let mut actors = alloc::vec![first_actor];
    for index in 0..T::CrossingPageSize::get().saturating_sub(1) {
      let owner: T::AccountId = account("crossing-maximum-unit", index, 0);
      actors.push(if index < 3 {
        bench_create_user_with_trigger::<T>(
          owner,
          Trigger::observation_crossing(feed, CrossingDirection::Rising, 2, 0),
        )
      } else {
        bench_create_system_crossing::<T>(owner, feed, 2)
      });
    }
    CrossingRangeCursors::<T>::insert(
      feed,
      CrossingRangeCursor {
        revision: 2,
        traversal: CrossingTraversal::Upward,
        search_bound: 2,
        current_threshold: Some(2),
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    #[block]
    {
      Pallet::<T>::crossing_placed_batch_work_unit(CROSSING_COHORT_BENCHMARK_MAX)
        .expect("placed Crossing maximum batch must succeed");
    }
    for actor_id in actors
      .into_iter()
      .take(CROSSING_COHORT_BENCHMARK_MAX as usize)
    {
      assert!(benchmark_fixture_hot::<T>(actor_id).is_some_and(|hot| {
        hot.pending_signal
          && hot.queue_ticket.is_some()
          && matches!(
            hot.trigger_runtime_state,
            TriggerRuntimeState::ObservationCrossing {
              phase: CrossingPhase::WaitingForRearm,
              ..
            }
          )
      }));
    }
  }

  #[benchmark]
  fn crossing_placed_non_tail_emptied_unit() {
    let actors = prepare_non_tail_crossing_batch::<T>(CROSSING_NON_TAIL_BENCHMARK_MAX);
    #[block]
    {
      Pallet::<T>::crossing_placed_batch_work_unit(CROSSING_COHORT_BENCHMARK_MAX)
        .expect("non-tail Crossing batch with emptied tail must succeed");
    }
    for actor_id in actors
      .into_iter()
      .take(CROSSING_NON_TAIL_BENCHMARK_MAX as usize)
    {
      assert!(
        benchmark_fixture_hot::<T>(actor_id)
          .is_some_and(|hot| { hot.pending_signal && hot.queue_ticket.is_some() })
      );
    }
  }

  #[benchmark]
  fn crossing_placed_non_tail_trimmed_unit() {
    let actors = prepare_non_tail_crossing_batch::<T>(CROSSING_TRIMMED_BENCHMARK_TAIL);
    #[block]
    {
      Pallet::<T>::crossing_placed_batch_work_unit(CROSSING_COHORT_BENCHMARK_MAX)
        .expect("non-tail Crossing batch with trimmed tail must succeed");
    }
    for actor_id in actors
      .into_iter()
      .take(CROSSING_NON_TAIL_BENCHMARK_MAX as usize)
    {
      assert!(
        benchmark_fixture_hot::<T>(actor_id)
          .is_some_and(|hot| { hot.pending_signal && hot.queue_ticket.is_some() })
      );
    }
  }

  #[benchmark]
  fn crossing_skip_unit() {
    let (_, actor_id) = prepare_crossing_work::<T>(2);
    benchmark_fixture_mutate_hot::<T>(actor_id, |hot| {
      let TriggerRuntimeState::ObservationCrossing {
        installed_at_revision,
        ..
      } = &mut hot.trigger_runtime_state
      else {
        panic!("benchmark actor must use Crossing state");
      };
      *installed_at_revision = 2;
    });
    benchmark_fixture_align_primary_control::<T>(actor_id);
    #[block]
    {
      Pallet::<T>::crossing_work_unit().expect("post-installation Crossing skip must succeed");
    }
    assert!(benchmark_fixture_hot::<T>(actor_id).is_some_and(|hot| {
      !hot.pending_signal
        && hot.queue_ticket.is_none()
        && matches!(
          hot.trigger_runtime_state,
          TriggerRuntimeState::ObservationCrossing {
            phase: CrossingPhase::Armed,
            installed_at_revision: 2,
          }
        )
    }));
  }

  #[benchmark]
  fn crossing_skip_pair_unit() {
    let (feed, first_actor) = prepare_crossing_work::<T>(2);
    let second_owner: T::AccountId = account("crossing-skip-pair-unit", 0, 0);
    let second_actor = bench_create_user_with_trigger::<T>(
      second_owner,
      Trigger::observation_crossing(feed, CrossingDirection::Rising, 2, 0),
    );
    for actor_id in [first_actor, second_actor] {
      benchmark_fixture_mutate_hot::<T>(actor_id, |hot| {
        let TriggerRuntimeState::ObservationCrossing {
          installed_at_revision,
          ..
        } = &mut hot.trigger_runtime_state
        else {
          panic!("pair Actor must use Crossing state");
        };
        *installed_at_revision = 2;
      });
    }
    CrossingRangeCursors::<T>::insert(
      feed,
      CrossingRangeCursor {
        revision: 2,
        traversal: CrossingTraversal::Upward,
        search_bound: 2,
        current_threshold: Some(2),
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    for actor_id in [first_actor, second_actor] {
      benchmark_fixture_align_primary_control::<T>(actor_id);
    }
    #[block]
    {
      Pallet::<T>::crossing_pair_work_unit().expect("Crossing skip pair must succeed");
    }
    for actor_id in [first_actor, second_actor] {
      assert!(benchmark_fixture_hot::<T>(actor_id).is_some_and(|hot| {
        !hot.pending_signal
          && hot.queue_ticket.is_none()
          && matches!(
            hot.trigger_runtime_state,
            TriggerRuntimeState::ObservationCrossing {
              phase: CrossingPhase::Armed,
              installed_at_revision: 2,
            }
          )
      }));
    }
  }

  #[benchmark]
  fn crossing_actor_unit() {
    let (_, actor_id) = prepare_crossing_work::<T>(2);
    benchmark_fixture_reset_ready_queue::<T>();
    ActorReadyHead::<T>::put(u64::MAX);
    benchmark_fixture_set_next_ready_ticket::<T>(u64::MAX);
    #[block]
    {
      Pallet::<T>::crossing_work_unit()
        .expect("matched Crossing actor terminal cleanup must succeed");
    }
    assert!(benchmark_fixture_hot::<T>(actor_id).is_none());
    assert!(!CrossingMemberships::<T>::contains_key(actor_id));
  }

  #[benchmark]
  fn transaction_extension_ingress_base() {
    let owner: T::AccountId = whitelisted_caller();
    let populated_actor_id = bench_create_user::<T>(owner);
    let proof_witness = Pallet::<T>::active_actor_view(populated_actor_id)
      .expect("benchmark actor exists")
      .sovereign_account;
    let recipient: T::AccountId = account("unmatched_ingress_recipient", 0, 0);
    let source: T::AccountId = account("ingress_source", 0, 0);
    T::BenchmarkHelper::setup_address_event_ingress(&recipient, &source, One::one())
      .expect("benchmark helper must prepare an unmatched producer event");
    #[block]
    {
      // Storage benchmarking does not attribute an absent overlay lookup to its map. Read a
      // populated witness first so the generated envelope includes one conservative database
      // read and the map's maximum proof before exercising the real negative lookup.
      assert!(SovereignIndex::<T>::contains_key(&proof_witness));
      assert!(!T::BenchmarkHelper::run_address_event_ingress(
        &recipient,
        &source,
        One::one(),
      ));
    }
  }

  #[benchmark]
  fn transaction_extension_ingress_notify()
  -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    let source: T::AccountId = account("ingress_source", 0, 0);
    let owner: T::AccountId = account("ingress_owner", 0, 0);
    let (mut steps, funding) = reachable_retry_contract::<T>()?;
    let last = steps.last_mut().expect("retry Contract has a tail");
    last.task = make_tracked_funding_contract_steps::<T>(owner.clone())
      .remove(0)
      .task;
    let mut contract = system_contract::<T>(
      Schedule {
        trigger: Trigger::address_event(
          saturated_address_source_filter::<T>(&owner, Some(source.clone())),
          AssetFilter::Any,
        ),
        cooldown_blocks: 100,
      },
      steps,
    )
    .expect("active ingress retry Contract exists");
    contract.funding = FundingSourcePolicy::AnyVerifiedIngress;
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      Some(contract),
    )
    .expect("ingress retry Contract is admitted");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let recipient = Pallet::<T>::sovereign_account_id_system(actor_id);
    open_reachable_retry::<T>(actor_id, funding);
    let run = ActorRunStateStore::<T>::get(actor_id).expect("real ingress retry Run exists");
    let retained_run = run.encode();
    let location = ActorControlLocators::<T>::get(actor_id);
    assert!(matches!(
      location,
      Some(ActorControlLocation::Waiting { .. })
    ));
    assert!(
      !benchmark_fixture_hot::<T>(actor_id)
        .expect("active ingress target")
        .pending_signal
    );
    assert_eq!(
      ActorReadyOccupancy::<T>::get(),
      0,
      "tombstones cannot replace live Ready actors"
    );
    let native = T::FeeNativeAssetId::get();
    let before_funding = ActorFunding::<T>::get(actor_id).expect("ingress funding exists");
    assert!(before_funding.funding_tracked_assets.contains(&native));
    let expected_funding = before_funding
      .funding_accumulated
      .get(&native)
      .copied()
      .unwrap_or_else(Zero::zero)
      .checked_add(&One::one())
      .expect("second ingress accumulator is representable");
    install_saturated_tombstone_queue::<T>();
    T::BenchmarkHelper::setup_address_event_ingress(&recipient, &source, One::one())
      .expect("benchmark helper must prepare a matched producer event");
    #[block]
    {
      assert!(T::BenchmarkHelper::run_address_event_ingress(
        &recipient,
        &source,
        One::one(),
      ));
    }
    assert!(
      benchmark_fixture_hot::<T>(actor_id)
        .is_some_and(|hot| { hot.pending_signal && hot.wakeup_pointer.is_some() })
    );
    assert_eq!(ActorControlLocators::<T>::get(actor_id), location);
    assert_eq!(
      ActorFunding::<T>::get(actor_id)
        .expect("latched funding remains")
        .funding_accumulated
        .get(&native)
        .copied(),
      Some(expected_funding)
    );
    assert_eq!(
      ActorRunStateStore::<T>::get(actor_id)
        .expect("latched Run remains")
        .encode(),
      retained_run
    );
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("real ingress retry latch passes state audit");
    Ok(())
  }

  #[benchmark]
  fn funding_snapshot_open(
    a: Linear<
      1,
      {
        T::MaxFundingTrackedAssets::get()
          .min(T::MaxContractSteps::get().saturating_mul(2))
          .max(1)
      },
    >,
  ) -> Result<(), polkadot_sdk::frame_benchmarking::BenchmarkError> {
    if T::MaxFundingTrackedAssets::get() == 0 {
      return Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Stop(
        "host cannot represent a nonempty funding snapshot",
      ));
    }
    let owner: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&owner);
    let assets = T::BenchmarkHelper::setup_predicate_assets(&owner, a)
      .expect("funding assets exist in the host ledger");
    assert_eq!(assets.len(), a as usize);
    let mut steps = ContractSteps::<T>::default();
    for pair in assets.chunks(2) {
      let amount = AmountResolution::PercentageOfLastFunding(Perbill::one());
      let task = if pair.len() == 1 {
        ActorTask::Transfer {
          to: account("funding-snapshot-recipient", 0, 0),
          asset: pair[0],
          amount,
        }
      } else {
        ActorTask::AddLiquidity {
          asset_a: pair[0],
          asset_b: pair[1],
          amount_a: amount,
          amount_b: amount,
          min_lp_out: One::one(),
        }
      };
      steps
        .try_push(Step {
          precondition: None,
          task,
          on_error: StepErrorPolicy::AbortCycle,
        })
        .expect("reachable funding sources fit the authored Step bound");
    }
    prefund_active_user_creation::<T>(&owner, &steps);
    Pallet::<T>::create_user_actor(
      RawOrigin::Signed(owner).into(),
      Mutability::Mutable,
      user_contract::<T>(
        Schedule {
          trigger: Trigger::Manual,
          cooldown_blocks: 0,
        },
        steps,
      ),
    )
    .expect("funding snapshot Contract is admitted");
    let actor_id = NextActorId::<T>::get() - 1;
    let amount = assets
      .iter()
      .map(|asset| T::AssetOps::minimum_balance(*asset))
      .max()
      .expect("nonempty funding assets")
      .checked_add(&One::one())
      .expect("funding amount fits");
    fund_reachable_update_assets::<T>(actor_id, amount);
    #[cfg(feature = "try-runtime")]
    Pallet::<T>::do_try_state().expect("certified funding input is a reachable Actor state");
    let snapshot;
    // This measures accumulator consumption only; complete Opening owns hold reconciliation.
    #[block]
    {
      snapshot = ActorFunding::<T>::mutate(actor_id, |maybe| {
        maybe
          .as_mut()
          .map(|funding| core::mem::take(&mut funding.funding_accumulated))
          .expect("benchmark actor funding exists")
      });
    }
    assert_eq!(snapshot.len() as u32, a);
    assert!(
      ActorFunding::<T>::get(actor_id)
        .expect("benchmark actor funding exists")
        .funding_accumulated
        .is_empty()
    );
    Ok(())
  }

  /// Builds a circular chain of `n` system Actors where each transfers 1% of its
  /// native balance to the next actor in the ring, then runs 3 blocks and asserts zero drift.
  pub(super) fn setup_and_run_circular_chain<T: Config>(
    requested_n: u32,
  ) -> alloc::vec::Vec<T::AccountId> {
    let existing_active = benchmark_fixture_active_count::<T>();
    let available = T::MaxActiveActors::get().saturating_sub(existing_active);
    let n = requested_n.min(available);
    assert!(
      n > 0,
      "benchmark requires at least one available active slot"
    );
    let pct = polkadot_sdk::sp_runtime::Perbill::from_percent(1);
    let initial_balance = T::MinUserBalance::get().saturating_mul(1_000_000u32.into());
    let native = T::FeeNativeAssetId::get();
    let schedule = Schedule {
      trigger: Trigger::cadenced(1),
      cooldown_blocks: 0,
    };
    let mut sovereigns: alloc::vec::Vec<T::AccountId> = alloc::vec::Vec::with_capacity(n as usize);
    let mut actor_ids: alloc::vec::Vec<ActorId> = alloc::vec::Vec::with_capacity(n as usize);
    for i in 0..n {
      let owner: T::AccountId = account("owner", i, 0);
      let temp_contract_steps = make_inert_contract_steps::<T>();
      Pallet::<T>::create_system_actor(
        RawOrigin::Root.into(),
        owner,
        Mutability::Mutable,
        system_contract::<T>(schedule.clone(), temp_contract_steps),
      )
      .expect("create_system_actor must succeed");
      let actor_id = NextActorId::<T>::get().saturating_sub(1);
      let sov = Pallet::<T>::sovereign_account_id_system(actor_id);
      let _ = T::AssetOps::mint(&sov, native, initial_balance);
      sovereigns.push(sov);
      actor_ids.push(actor_id);
    }
    // Creation stamps `last_control_mutation_block`, and the one-mutation-per-actor clock is per
    // block, so the rewrite below must land strictly after the block the creation loop ran on.
    let creation_block = frame_system::Pallet::<T>::block_number();
    frame_system::Pallet::<T>::set_block_number(creation_block + One::one());
    for (i, actor_id) in actor_ids.iter().enumerate() {
      let next_sov = sovereigns[(i + 1) % sovereigns.len()].clone();
      let transfer_contract_steps: ContractSteps<T> = BoundedVec::try_from(alloc::vec![Step {
        precondition: None,
        task: ActorTask::Transfer {
          to: next_sov,
          asset: native,
          amount: AmountResolution::PercentageOfCurrent(pct),
        },
        on_error: StepErrorPolicy::AbortCycle,
      }])
      .expect("transfer contract_steps fits");
      let contract = Pallet::<T>::load_actor_contract(*actor_id).expect("Actor Contract exists");
      Pallet::<T>::update_contract(
        RawOrigin::Root.into(),
        *actor_id,
        ActorContract {
          steps: transfer_contract_steps,
          completion: CompletionPolicy::Persistent,
          ..contract
        },
      )
      .expect("update_contract must succeed");
    }
    let total_before: T::Balance = sovereigns
      .iter()
      .map(|sov| T::AssetOps::balance(sov, native))
      .fold(T::Balance::zero(), |acc, b| acc.saturating_add(b));
    for block in 2u32..=4 {
      frame_system::Pallet::<T>::set_block_number(block.into());
      let _ = Pallet::<T>::on_idle(block.into(), Weight::MAX);
    }
    // System Actors don't pay fees → transfers are pure balance moves → zero drift
    let total_after: T::Balance = sovereigns
      .iter()
      .map(|sov| T::AssetOps::balance(sov, native))
      .fold(T::Balance::zero(), |acc, b| acc.saturating_add(b));
    assert_eq!(
      total_before, total_after,
      "Balance must be exactly conserved (System Actors pay no fees)"
    );
    sovereigns
  }

  /// Parametric stress test: circular chain of n system Actors.
  ///
  /// Capacity planning reference points:
  /// - n=100: ~300 transfers/block (baseline)
  /// - n=1_000: ~3000 transfers/block (moderate load)
  /// - n=10_000: ~30000 transfers/block (high load)
  #[benchmark(extra)]
  fn circular_chain_stress(n: Linear<10, 10_000>) {
    #[block]
    {
      setup_and_run_circular_chain::<T>(n);
    }
  }

  /// Extreme stress test request: 10K-100K Actors circular chain.
  /// Effective n is clamped by available `MaxActiveActors` capacity.
  #[benchmark(extra)]
  fn circular_chain_stress_100k(n: Linear<10_000, 100_000>) {
    #[block]
    {
      setup_and_run_circular_chain::<T>(n);
    }
  }

  /// Fixed-size stress tests for scaling analysis.
  /// Run all three and compare times to determine O(n) vs O(n²).
  /// Linear: time ratio ≈ 10x when n increases 10x
  /// Quadratic: time ratio ≈ 100x when n increases 10x

  #[benchmark]
  fn circular_chain_100() {
    #[block]
    {
      setup_and_run_circular_chain::<T>(100);
    }
  }

  #[benchmark]
  fn circular_chain_1000() {
    #[block]
    {
      setup_and_run_circular_chain::<T>(1000);
    }
  }

  #[benchmark(extra)]
  fn circular_chain_10000() {
    #[block]
    {
      setup_and_run_circular_chain::<T>(10_000);
    }
  }

  #[benchmark]
  fn maximum_context_inherent() {
    let inherent = T::BenchmarkHelper::prepare_maximum_context_inherent();
    #[block]
    {
      T::BenchmarkHelper::execute_maximum_context_inherent(inherent)
        .expect("maximum context inherent execution must succeed");
    }
    T::BenchmarkHelper::verify_maximum_context_inherent();
  }

  #[benchmark]
  fn block_resource_finalize() {
    let now = frame_system::Pallet::<T>::block_number();
    let budget = T::BlockResourceBudget::get();
    let mut state = BlockResourceState::new(now);
    state.begin_prepass().expect("benchmark state opens"); // deos-bypass: panic-owner — fresh benchmark state has no reservations.
    state
      .open_external_phase()
      .expect("benchmark prepass closes"); // deos-bypass: panic-owner — preceding transition establishes empty PrepassExecuting.
    state.begin_drain().expect("benchmark drain opens"); // deos-bypass: panic-owner — preceding transition establishes ExternalPhase.
    state
      .finish_drain(budget, budget.fixed_envelope())
      .expect("benchmark state reconciles"); // deos-bypass: panic-owner — empty usage plus the configured fixed envelope exactly satisfies the budget.
    CurrentBlockResourceState::<T>::put(state);
    #[block]
    {
      let current =
        CurrentBlockResourceState::<T>::take().expect("finalized benchmark state exists"); // deos-bypass: panic-owner — setup writes this exact storage value.
      FinalizedBlockResourceTelemetry::<T>::put(
        current
          .finalized_snapshot()
          .expect("finalized snapshot exists"), // deos-bypass: panic-owner — successful finish_drain establishes Finalizable and fixed actual.
      );
      let valid = current.ensure_block(now).is_ok()
        && current.phase() == BlockResourcePhase::Finalizable
        && current.outstanding_reservations() == 0
        && FinalizedBlockResourceTelemetry::<T>::get()
          .is_some_and(|snapshot| snapshot.block_number() == now);
      assert!(valid);
    }
  }

  #[benchmark]
  fn block_resource_meter_extension() {
    T::BenchmarkHelper::prepare_block_resource_meter_extension();
    #[block]
    {
      T::BenchmarkHelper::execute_block_resource_meter_extension();
    }
    T::BenchmarkHelper::verify_block_resource_meter_extension();
  }

  #[benchmark]
  fn maximum_xcm_version_discovery() {
    T::BenchmarkHelper::prepare_maximum_xcm_version_discovery();
    #[block]
    {
      T::BenchmarkHelper::execute_maximum_xcm_version_discovery();
    }
    T::BenchmarkHelper::verify_maximum_xcm_version_discovery();
  }

  // Actor control candidate-only storage probes. These five owners measure one physical chunk
  // read-modify-write while varying only the bounded cell count. They are not production Weight
  // methods and must be deleted with a rejected candidate or replaced by accepted runtime owners.
  #[benchmark(extra)]
  fn control_frame_chunk4_io(n: Linear<1, 4>) {
    let chunk = BoundedVec::<BenchmarkAdmissionCell, ConstU32<4>>::try_from(vec![[1u8; 171]; 4])
      .expect("frame C4 benchmark chunk fits");
    BenchmarkAdmissionChunk4::<T>::insert(0, chunk);
    #[block]
    {
      let mut loaded =
        BenchmarkAdmissionChunk4::<T>::get(0).expect("frame C4 benchmark chunk exists");
      for cell in loaded.iter_mut().take(n as usize) {
        cell[0] = cell[0].wrapping_add(1);
      }
      BenchmarkAdmissionChunk4::<T>::insert(0, loaded);
    }
    let stored = BenchmarkAdmissionChunk4::<T>::get(0).expect("frame C4 benchmark chunk remains");
    assert_eq!(stored.len(), 4);
    assert!(stored.iter().take(n as usize).all(|cell| cell[0] == 2));
  }

  #[benchmark(extra)]
  fn control_frame_chunk8_io(n: Linear<1, 8>) {
    let chunk = BoundedVec::<BenchmarkAdmissionCell, ConstU32<8>>::try_from(vec![[1u8; 171]; 8])
      .expect("frame C8 benchmark chunk fits");
    BenchmarkAdmissionChunk8::<T>::insert(0, chunk);
    #[block]
    {
      let mut loaded =
        BenchmarkAdmissionChunk8::<T>::get(0).expect("frame C8 benchmark chunk exists");
      for cell in loaded.iter_mut().take(n as usize) {
        cell[0] = cell[0].wrapping_add(1);
      }
      BenchmarkAdmissionChunk8::<T>::insert(0, loaded);
    }
    let stored = BenchmarkAdmissionChunk8::<T>::get(0).expect("frame C8 benchmark chunk remains");
    assert_eq!(stored.len(), 8);
    assert!(stored.iter().take(n as usize).all(|cell| cell[0] == 2));
  }

  #[benchmark(extra)]
  fn control_frame_chunk16_io(n: Linear<1, 16>) {
    let chunk = BoundedVec::<BenchmarkAdmissionCell, ConstU32<16>>::try_from(vec![[1u8; 171]; 16])
      .expect("frame C16 benchmark chunk fits");
    BenchmarkAdmissionChunk16::<T>::insert(0, chunk);
    #[block]
    {
      let mut loaded =
        BenchmarkAdmissionChunk16::<T>::get(0).expect("frame C16 benchmark chunk exists");
      for cell in loaded.iter_mut().take(n as usize) {
        cell[0] = cell[0].wrapping_add(1);
      }
      BenchmarkAdmissionChunk16::<T>::insert(0, loaded);
    }
    let stored = BenchmarkAdmissionChunk16::<T>::get(0).expect("frame C16 benchmark chunk remains");
    assert_eq!(stored.len(), 16);
    assert!(stored.iter().take(n as usize).all(|cell| cell[0] == 2));
  }

  #[benchmark(extra)]
  fn control_frame_chunk32_io(n: Linear<1, 32>) {
    ActorReadyFrameChunks::<T>::insert(0, control_named_chunk::<T>(0, 32));
    #[block]
    {
      let mut loaded =
        ActorReadyFrameChunks::<T>::get(0).expect("named frame C32 benchmark chunk exists");
      for cell in loaded.iter_mut().take(n as usize).flatten() {
        cell.hot.pending_signal = false;
      }
      ActorReadyFrameChunks::<T>::insert(0, loaded);
    }
    let stored =
      ActorReadyFrameChunks::<T>::get(0).expect("named frame C32 benchmark chunk remains");
    assert_eq!(stored.len(), 32);
    assert!(
      stored
        .iter()
        .take(n as usize)
        .flatten()
        .all(|cell| !cell.hot.pending_signal)
    );
  }

  #[benchmark(extra)]
  fn control_frame_chunk64_io(n: Linear<1, 64>) {
    let chunk = BoundedVec::<BenchmarkAdmissionCell, ConstU32<64>>::try_from(vec![[1u8; 171]; 64])
      .expect("frame C64 benchmark chunk fits");
    BenchmarkAdmissionChunk64::<T>::insert(0, chunk);
    #[block]
    {
      let mut loaded =
        BenchmarkAdmissionChunk64::<T>::get(0).expect("frame C64 benchmark chunk exists");
      for cell in loaded.iter_mut().take(n as usize) {
        cell[0] = cell[0].wrapping_add(1);
      }
      BenchmarkAdmissionChunk64::<T>::insert(0, loaded);
    }
    let stored = BenchmarkAdmissionChunk64::<T>::get(0).expect("frame C64 benchmark chunk remains");
    assert_eq!(stored.len(), 64);
    assert!(stored.iter().take(n as usize).all(|cell| cell[0] == 2));
  }

  /// Actor control due-only owner move from one waiting C32 chunk into a fresh ready C32 chunk.
  #[benchmark(extra)]
  fn control_frame_move_waiting_to_fresh_ready32(n: Linear<1, 32>) {
    let key = WakeupKey::Block(1u32.into());
    let waiting = ActorWaitingChunkOf::<T>::try_from(
      control_named_chunk::<T>(0, 32)
        .into_iter()
        .map(|cell| cell.map(ActorWaitingEntry::Primary))
        .collect::<Vec<_>>(),
    )
    .expect("named Waiting primary chunk fits");
    ActorWaitingFrameChunks::<T>::insert((key, 0), control_waiting_page::<T>(waiting));
    #[block]
    {
      let loaded = ActorWaitingFrameChunks::<T>::get((key, 0))
        .expect("named frame waiting C32 benchmark chunk exists");
      let mut cells = loaded.entries.into_inner();
      let moved = cells
        .drain(..n as usize)
        .map(|entry| entry.and_then(ActorWaitingEntry::into_primary))
        .collect::<Vec<_>>();
      let remaining =
        ActorWaitingChunkOf::<T>::try_from(cells).expect("named frame waiting C32 remainder fits");
      let ready =
        ActorControlChunkOf::<T>::try_from(moved).expect("named frame fresh ready C32 prefix fits");
      if remaining.is_empty() {
        ActorWaitingFrameChunks::<T>::remove((key, 0));
      } else {
        ActorWaitingFrameChunks::<T>::insert((key, 0), control_waiting_page::<T>(remaining));
      }
      ActorReadyFrameChunks::<T>::insert(0, ready);
    }
    assert_eq!(ActorReadyFrameChunks::<T>::decode_len(0), Some(n as usize));
  }

  /// Actor control worst fixed-C32 append crossing: one cell fills the existing ready tail and the
  /// remaining 31 cells allocate its immediate successor without reordering the waiting cohort.
  #[benchmark(extra)]
  fn control_frame_move_waiting_across_ready32_tail() {
    let key = WakeupKey::Block(1u32.into());
    let waiting = ActorWaitingChunkOf::<T>::try_from(
      control_named_chunk::<T>(0, 32)
        .into_iter()
        .map(|cell| cell.map(ActorWaitingEntry::Primary))
        .collect::<Vec<_>>(),
    )
    .expect("named Waiting primary chunk fits");
    ActorWaitingFrameChunks::<T>::insert((key, 0), control_waiting_page::<T>(waiting));
    ActorReadyFrameChunks::<T>::insert(0, control_named_chunk::<T>(100, 31));
    #[block]
    {
      let waiting = ActorWaitingFrameChunks::<T>::get((key, 0))
        .expect("named frame crossing waiting C32 benchmark chunk exists");
      let mut waiting_cells = waiting
        .entries
        .into_iter()
        .map(|entry| entry.and_then(ActorWaitingEntry::into_primary))
        .collect::<Vec<_>>();
      let first = waiting_cells.remove(0);
      let mut ready_tail = ActorReadyFrameChunks::<T>::get(0)
        .expect("named frame crossing ready C32 benchmark tail exists");
      ready_tail
        .try_push(first)
        .expect("one waiting cell fills the ready C32 tail");
      let successor = ActorControlChunkOf::<T>::try_from(waiting_cells)
        .expect("remaining waiting cells fit the successor C32 chunk");
      ActorReadyFrameChunks::<T>::insert(0, ready_tail);
      ActorReadyFrameChunks::<T>::insert(1, successor);
      ActorWaitingFrameChunks::<T>::remove((key, 0));
    }
    assert_eq!(ActorReadyFrameChunks::<T>::decode_len(0), Some(32));
    assert_eq!(ActorReadyFrameChunks::<T>::decode_len(1), Some(31));
  }

  /// Actor control external-boundary locator transition. Execution never reads this map, but every
  /// physical Waiting/Ready move must publish the new ticket/location without mass compaction.
  #[benchmark(extra)]
  fn control_frame_locator_transition_batch32(n: Linear<1, 32>) {
    let first_actor_id = NextActorId::<T>::get();
    for offset in 0..n as u64 {
      assert!(!ActorControlLocators::<T>::contains_key(
        first_actor_id.saturating_add(offset)
      ));
    }
    #[block]
    {
      for offset in 0..n as u64 {
        let actor_id = first_actor_id.saturating_add(offset);
        ActorControlLocators::<T>::insert(
          actor_id,
          ActorControlLocation::Ready {
            ticket: 10_000u64.saturating_add(offset),
          },
        );
      }
    }
    for offset in 0..n as u64 {
      assert_eq!(
        ActorControlLocators::<T>::get(first_actor_id.saturating_add(offset)),
        Some(ActorControlLocation::Ready {
          ticket: 10_000u64.saturating_add(offset),
        })
      );
    }
  }

  /// Actor control one-batch service cursor settlement after a contiguous ready prefix commits.
  #[benchmark(extra)]
  fn control_frame_service_cursor_batch32(n: Linear<1, 32>) {
    ActorReadyHead::<T>::put(0);
    ActorReadyTail::<T>::put(32);
    ActorReadyOccupancy::<T>::put(32);
    #[block]
    {
      let head = ActorReadyHead::<T>::get();
      let tail = ActorReadyTail::<T>::get();
      let occupancy = ActorReadyOccupancy::<T>::get();
      assert!(head.saturating_add(n as u64) <= tail);
      ActorReadyHead::<T>::put(head.saturating_add(n as u64));
      ActorReadyOccupancy::<T>::put(occupancy.saturating_sub(n));
    }
  }

  /// Actor control one-batch due-page cursor settlement into the current ready tail.
  #[benchmark(extra)]
  fn control_frame_due_cursor_batch32(n: Linear<1, 32>) {
    let key = WakeupKey::Block(1u32.into());
    ActorWaitingTails::<T>::insert(key, n as u64);
    ActorReadyTail::<T>::put(31);
    ActorReadyOccupancy::<T>::put(31);
    #[block]
    {
      let waiting = ActorWaitingTails::<T>::get(key);
      let ready_tail = ActorReadyTail::<T>::get();
      let occupancy = ActorReadyOccupancy::<T>::get();
      assert_eq!(waiting, n as u64);
      ActorWaitingTails::<T>::remove(key);
      ActorReadyTail::<T>::put(ready_tail.saturating_add(n as u64));
      ActorReadyOccupancy::<T>::put(occupancy.saturating_add(n));
    }
  }

  /// Complete minimal User Opening cohort through the canonical FIFO and Pipeline Opening.
  /// Trigger source work remains setup-owned; the measured branch includes control execution,
  /// fees, events, and explicit successor projection/persistence.
  #[benchmark(extra, pov_mode = Measured)]
  fn control_user_opening_complete_batch32(n: Linear<1, 32>) {
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(0u32.into());
    GlobalCircuitBreaker::<T>::put(false);
    let mut actors = Vec::with_capacity(n as usize);
    for index in 0..n {
      let owner: T::AccountId = account("control-complete-user", index, 0);
      let actor_id = bench_create_user_with_trigger_and_steps::<T>(
        owner.clone(),
        Trigger::manual(),
        make_inert_contract_steps::<T>(),
      );
      let sovereign = Pallet::<T>::active_actor_view(actor_id)
        .map(|instance| instance.sovereign_account)
        .expect("Actor control complete User exists after setup");
      let reserve: T::Balance = (u64::MAX / 4).saturated_into();
      let _ = T::AssetOps::mint(&sovereign, T::FeeNativeAssetId::get(), reserve);
      actors.push((actor_id, owner));
    }
    frame_system::Pallet::<T>::set_block_number(now);
    for (actor_id, owner) in actors.iter().cloned() {
      Pallet::<T>::manual_trigger(RawOrigin::Signed(owner).into(), actor_id)
        .expect("Actor control complete User trigger succeeds in setup");
      let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
        .expect("Actor control ready User setup state is coherent");
      let queue_ticket = state
        .hot
        .queue_ticket
        .expect("Actor control ready User setup ticket exists");
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        queue_ticket,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("Actor control ready User setup ticket builds");
      let expected_identity = state.identity.clone();
      let expected_hot = state.hot.clone();
      let expected_admission = admission.clone();
      let cell = Pallet::<T>::control_opening_cell_from_scalar(
        actor_id,
        state.identity,
        state.hot,
        admission,
        &ticket,
        &loaded_step,
      )
      .expect("Actor control ready User setup projects to frame");
      let restored = Pallet::<T>::project_control_cell(
        &cell,
        ActorControlLocation::Ready {
          ticket: queue_ticket,
        },
      )
      .expect("Actor control ready User setup restores from frame");
      assert_eq!(restored.0, expected_identity);
      assert_eq!(restored.1, expected_hot);
      assert_eq!(restored.2, expected_admission);
    }
    let actor_ids = actors
      .iter()
      .map(|(actor_id, _)| *actor_id)
      .collect::<Vec<_>>();
    #[block]
    {
      for _ in 0..n {
        if ActorReadyOccupancy::<T>::get() == 0 {
          break;
        }
        core::hint::black_box(Pallet::<T>::execute_cycle(Weight::MAX));
      }
      for actor_id in actor_ids.iter().take(n as usize).copied() {
        let (state, admission, loaded_step) =
          Pallet::<T>::load_current_step_service_state(actor_id).unwrap_or_else(|| {
            panic!(
              "Actor control completed User state remains coherent: actor={actor_id} identity={} hot={} contract={} admission={} funding={}",
              ActorIdentities::<T>::contains_key(actor_id),
              benchmark_fixture_hot::<T>(actor_id).is_some(),
              ActorContractHeads::<T>::contains_key(actor_id),
              benchmark_fixture_admission::<T>(actor_id).is_some(),
              ActorFunding::<T>::contains_key(actor_id),
            )
          });
        assert_eq!(state.identity.cycle_nonce, 1);
        assert_eq!(state.hot.cycle_state, CycleState::Idle);
        assert!(!state.hot.pending_signal);
        assert!(state.hot.queue_ticket.is_none());
        assert!(state.run_state.is_none());
        let cell = Pallet::<T>::control_unsignaled_cell_from_scalar(
          actor_id,
          state.identity,
          state.hot,
          admission,
          &loaded_step,
        )
        .expect("Actor control complete User authority projects exactly");
        ActorUnsignaledControlCells::<T>::insert(actor_id, &cell);
        ActorControlLocators::<T>::insert(actor_id, ActorControlLocation::Unsignaled);
        core::hint::black_box(cell);
      }
    }
    assert_eq!(ActorReadyOccupancy::<T>::get(), 0);
    for actor_id in actor_ids.iter().take(n as usize).copied() {
      assert!(ActorUnsignaledControlCells::<T>::contains_key(actor_id));
      assert!(!ActorIdentities::<T>::contains_key(actor_id));
      assert!(Pallet::<T>::actor_control_cell(actor_id).is_some());
    }
  }

  fn control_install_temporal_waiting<T: Config>(n: u32, due_tick: SchedulerTick) -> Vec<ActorId> {
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    GlobalCircuitBreaker::<T>::put(false);
    clear_host_genesis_wakeup_placements::<T>();
    let mut actor_ids = Vec::with_capacity(n as usize);
    for index in 0..n {
      let owner: T::AccountId = account("control-temporal-system", index, 0);
      Pallet::<T>::create_system_actor(
        RawOrigin::Root.into(),
        owner,
        Mutability::Mutable,
        system_contract::<T>(
          Schedule {
            trigger: Trigger::cadenced(5),
            cooldown_blocks: 0,
          },
          make_inert_contract_steps::<T>(),
        ),
      )
      .expect("Actor control temporal System creation succeeds");
      let actor_id = NextActorId::<T>::get().saturating_sub(1);
      Pallet::<T>::trigger_wakeup_substrate_invalidate_inner(actor_id)
        .expect("Actor control reference Trigger wakeup invalidates");
      let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
        .expect("Actor control temporal System state is coherent");
      let cell = Pallet::<T>::control_unsignaled_cell_from_scalar(
        actor_id,
        state.identity,
        state.hot,
        admission,
        &loaded_step,
      )
      .expect("Actor control temporal System projects Unsignaled");
      ActorUnsignaledControlCells::<T>::insert(actor_id, cell);
      ActorControlLocators::<T>::insert(actor_id, ActorControlLocation::Unsignaled);
      Pallet::<T>::control_stage_unsignaled_temporal(actor_id, due_tick)
        .expect("Actor control temporal System stages Waiting");
      actor_ids.push(actor_id);
    }
    actor_ids
  }

  /// Actor control Trigger-due transition from one C32 Tick page into N+1 Block waiting. The measured
  /// branch clears Trigger pointers, latches readiness, and publishes service pointers/locators.
  #[benchmark(extra, pov_mode = Measured)]
  fn control_frame_temporal_latch_batch32(n: Linear<1, 32>) {
    let actor_ids = control_install_temporal_waiting::<T>(n, 10);
    #[block]
    {
      let moved = Pallet::<T>::control_latch_temporal_waiting_page(10, 0, 1u32.into(), 10)
        .expect("Actor control temporal C32 latches");
      assert_eq!(moved, actor_ids);
      core::hint::black_box(moved);
    }
    assert!(!ActorWaitingFrameChunks::<T>::contains_key((
      WakeupKey::Tick(10),
      0
    )));
    assert_eq!(
      ActorWaitingOccupancies::<T>::get(WakeupKey::Block(2u32.into())),
      n
    );
  }

  /// Actor control N+1 transition from one C32 Block-waiting page into immutable Ready tickets.
  #[benchmark(extra, pov_mode = Measured)]
  fn control_frame_waiting_ready_batch32(n: Linear<1, 32>) {
    let actor_ids = control_install_temporal_waiting::<T>(n, 10);
    Pallet::<T>::control_latch_temporal_waiting_page(10, 0, 1u32.into(), 10)
      .expect("Actor control temporal C32 latches in setup");
    frame_system::Pallet::<T>::set_block_number(2u32.into());
    #[block]
    {
      let moved = Pallet::<T>::control_promote_due_waiting_page(2u32.into(), 0, 2u32.into())
        .expect("Actor control N+1 C32 promotes");
      assert_eq!(
        moved,
        actor_ids
          .iter()
          .copied()
          .enumerate()
          .map(|(ticket, actor_id)| (actor_id, ticket as QueueTicket))
          .collect::<Vec<_>>()
      );
      core::hint::black_box(moved);
    }
    assert_eq!(ActorReadyTail::<T>::get(), n as QueueTicket);
    assert_eq!(ActorReadyOccupancy::<T>::get(), n);
  }

  #[cfg(test)]
  use crate::mock::{Test, new_test_ext};
  #[cfg(test)]
  #[test]
  fn fixture_admission_reduction_preserves_protected_branch_and_structure() {
    new_test_ext().execute_with(|| {
      let count = <<Test as Config>::MaxContractSteps as Get<u32>>::get();
      let mut steps = make_reachable_opening_steps::<Test>(count)
        .expect("structurally bounded dense fixture exists");
      let protected = steps[0].clone();
      retain_admitted_contract_geometry::<Test>(ActorType::System, &mut steps, &[0])
        .expect("unrelated setup payload can be reduced through ordinary admission");
      let service = Pallet::<Test>::guaranteed_actor_service_weight()
        .expect("mock exposes a valid service floor");
      assert_eq!(steps.len() as u32, count);
      assert_eq!(steps[0], protected);
      assert!(
        Pallet::<Test>::contract_steps_admission_weight_upper(ActorType::System, &steps)
          .all_lte(service)
      );
    });
  }

  #[cfg(test)]
  #[test]
  fn reachable_retry_opening_partition_boundaries() {
    let steps = <<Test as Config>::MaxContractSteps as Get<u32>>::get();
    for amounts in [0, 1, steps * 2] {
      for predicates in [0, 1, steps * benchmark_predicate_capacity::<Test>()] {
        new_test_ext().execute_with(|| {
          let actor_id = prepare_user_retry_opening_partition::<Test>(amounts, predicates)
            .expect("independent capture boundaries reach actual suspension");
          let identity = benchmark_fixture_identity::<Test>(actor_id).unwrap();
          Pallet::<Test>::close_actor(RawOrigin::Signed(identity.owner).into(), actor_id)
            .expect("independent capture boundaries close");
          #[cfg(feature = "try-runtime")]
          Pallet::<Test>::do_try_state().expect("partition close leaves no orphan state");
        });
      }
    }
  }
  #[cfg(test)]
  #[test]
  fn reachable_opening_tail_profile_boundaries() {
    let maximum = <<Test as Config>::MaxContractSteps as Get<u32>>::get();
    for tails in 0..=maximum.saturating_sub(1).div_ceil(MAX_STEPS_PER_TAIL_CHUNK) {
      for profile in [
        ReachableOpeningProfile::UserPaged,
        ReachableOpeningProfile::UserCompleteMin,
        ReachableOpeningProfile::UserCompleteHeaderMax,
        ReachableOpeningProfile::Minimal,
        ReachableOpeningProfile::Predicated,
        ReachableOpeningProfile::RetryMin,
        ReachableOpeningProfile::RetryMax,
        ReachableOpeningProfile::CompleteMin,
        ReachableOpeningProfile::CompleteMax,
        ReachableOpeningProfile::FailedMin,
        ReachableOpeningProfile::FailedMax,
      ] {
        new_test_ext().execute_with(|| {
          if tails == 0
            && matches!(
              profile,
              ReachableOpeningProfile::UserPaged
                | ReachableOpeningProfile::Minimal
                | ReachableOpeningProfile::Predicated
            )
          {
            assert!(prepare_reachable_opening::<Test>(tails, profile).is_err());
            return;
          }
          let (actor_id, count) = prepare_reachable_opening::<Test>(tails, profile)
            .expect("host-feasible Opening tail profile is admitted");
          let accounting = matches!(
            profile,
            ReachableOpeningProfile::UserCompleteMin
              | ReachableOpeningProfile::UserCompleteHeaderMax
          )
          .then(|| user_pipeline_accounting::<Test>(actor_id));
          if matches!(profile, ReachableOpeningProfile::UserPaged) {
            Pallet::<Test>::execute_cycle(Weight::MAX);
          } else {
            let now = frame_system::Pallet::<Test>::block_number();
            let (state, admission, loaded_step) =
              benchmark_fixture_consume_frame_current_step_service_state::<Test>(actor_id);
            let effect_weight =
              execute_reachable_step_inner::<Test>(actor_id, state, admission, loaded_step, now);
            if matches!(
              profile,
              ReachableOpeningProfile::FailedMin | ReachableOpeningProfile::FailedMax
            ) {
              assert_eq!(effect_weight, Weight::zero());
            }
          }
          assert_reachable_opening::<Test>(actor_id, count, profile);
          if let Some(accounting) = accounting {
            assert_user_pipeline_accounting::<Test>(actor_id, accounting);
          }
        });
      }
    }
  }
  #[cfg(test)]
  #[test]
  fn reachable_suspended_head_inner_allocation_boundaries() {
    let capacity = benchmark_predicate_capacity::<Test>();
    let legs = (<<Test as Config>::MaxContractSteps as Get<u32>>::get() - 1) * 2;
    let minimum =
      legs.saturating_sub(<<Test as Config>::MaxFundingTrackedAssets as Get<u32>>::get());
    for branch in [
      None,
      Some(RunningInnerBranch::Complete),
      Some(RunningInnerBranch::Progress),
    ] {
      let allocations = if matches!(branch, Some(RunningInnerBranch::Complete)) {
        vec![0]
      } else {
        vec![minimum, minimum + (legs - minimum) / 2, legs]
      };
      for n in allocations {
        for predicates in 0..=capacity {
          for opening in 0..=predicates {
            new_test_ext().execute_with(|| {
              let (actor_id, skip) = prepare_reachable_suspended_head_inner::<Test>(
                n,
                predicates - opening,
                opening,
                branch,
              )
              .expect("coupled allocation and predicate timing partition are admitted");
              let before = ActorRunStateStore::<Test>::get(actor_id).expect("real Run exists");
              let now = frame_system::Pallet::<Test>::block_number();
              let (state, admission, loaded_step) =
                benchmark_fixture_consume_frame_current_step_service_state::<Test>(actor_id);
              assert_eq!(
                execute_reachable_step_inner::<Test>(actor_id, state, admission, loaded_step, now),
                Weight::zero()
              );
              if let Some(branch) = branch {
                if matches!(branch, RunningInnerBranch::Progress) {
                  let after =
                    ActorRunStateStore::<Test>::get(actor_id).expect("progress retains Run");
                  assert_eq!(after.opening_snapshot, before.opening_snapshot);
                  assert_eq!(
                    after.opening_predicate_results,
                    before.opening_predicate_results
                  );
                  assert_eq!(after.funding_snapshot, before.funding_snapshot);
                }
                assert_reachable_suspended_tail_skip::<Test>(
                  skip.expect("freeze fixture exists"),
                  now,
                  branch,
                );
              } else {
                assert_reachable_suspended_head_retry::<Test>(actor_id, before);
              }
            });
          }
        }
      }
    }
    new_test_ext().execute_with(|| {
      assert!(
        prepare_reachable_suspended_head_inner::<Test>(1, 0, 0, Some(RunningInnerBranch::Complete))
          .is_err()
      );
      assert!(prepare_reachable_suspended_head_inner::<Test>(legs + 1, 0, 0, None).is_err());
      assert!(prepare_reachable_suspended_head_inner::<Test>(legs, capacity, 1, None).is_err());
      assert_eq!(NextActorId::<Test>::get(), 0);
    });
  }
  #[cfg(test)]
  #[test]
  fn reachable_suspended_head_retry_payload_tradeoffs() {
    type Helper = <Test as Config>::BenchmarkHelper;
    type Assets = <Test as Config>::AssetOps;
    let count = <<Test as Config>::MaxContractSteps as Get<u32>>::get();
    let predicates = benchmark_predicate_capacity::<Test>();
    let legs = count * 2;
    let minimum =
      legs.saturating_sub(<<Test as Config>::MaxFundingTrackedAssets as Get<u32>>::get());
    for (opening_legs, opening_start) in [
      (minimum, 0),
      (minimum + 1, 0),
      (legs - 1, 0),
      (legs, 0),
      (minimum, 2),
      (minimum + 1, 2),
      (legs - 2, 2),
    ] {
      new_test_ext().execute_with(|| {
        let (actor_id, _) = prepare_reachable_suspended_head::<Test>(opening_legs, opening_start)
          .expect("two-leg head reaches a real due retry");
        let state = Pallet::<Test>::active_actor_state(actor_id).expect("due source exists");
        let run = state.run_state.as_ref().expect("first failure retained Run");
        assert_eq!(run.unsuccessful_attempts_at_cursor, 1);
        assert_eq!(run.opening_snapshot.len() as u32, opening_legs);
        assert_eq!(run.funding_snapshot.len() as u32, legs - opening_legs);
        assert_eq!(run.opening_predicate_results.len() as u32, count * predicates);
        let ActorTask::AddLiquidity { asset_a, asset_b, amount_a, amount_b, .. } =
          &state.contract.steps[0].task else { panic!("two-leg head remains authored") };
        let (asset, amount) = if *asset_a != <Test as Config>::FeeNativeAssetId::get() {
          (*asset_a, amount_a)
        } else {
          (*asset_b, amount_b)
        };
        assert_ne!(asset, <Test as Config>::FeeNativeAssetId::get());
        assert_eq!(matches!(amount, AmountResolution::PercentageOfLastFunding(_)), opening_start == 2);
        let frozen = match amount {
          AmountResolution::PercentageAtOpening(pct) => pct.mul_floor(
            *run.opening_snapshot.get(&OpeningSurface::PreservableAsset(asset))
              .expect("head Opening amount is retained")),
          AmountResolution::PercentageOfLastFunding(pct) => pct.mul_floor(
            *run.funding_snapshot.get(&asset).expect("head funding amount is retained")),
          _ => panic!("head amount is frozen by authored source"),
        };
        let actor = state.identity.sovereign_account;
        let owner = state.identity.owner;
        let remaining = Assets::minimum_balance(asset).max(1);
        assert!(frozen > remaining);
        let withdrawal = Assets::balance(&actor, asset).checked_sub(remaining)
          .expect("first attempt retains funded head custody");
        Assets::transfer(&actor, &owner, asset, withdrawal)
          .expect("fixture ledger withdrawal leaves the minimum intact");
        assert_eq!(Assets::balance(&actor, asset), remaining);
        assert_eq!(ActorRunStateStore::<Test>::get(actor_id)
          .expect("ledger withdrawal retains Run").encode(), run.encode());
        let custody = (Assets::balance(&actor, asset), Assets::balance(&owner, asset));
        let (source, admission, loaded_step) =
          benchmark_fixture_consume_frame_current_step_service_state::<Test>(actor_id);
        assert_eq!(execute_reachable_step_inner::<Test>(
          actor_id, source, admission, loaded_step, run.eligible_at,
        ), Weight::zero());
        let after = ActorRunStateStore::<Test>::get(actor_id).expect("second failure retains Run");
        assert_eq!(after.cursor, 0);
        assert_eq!(after.cycle_nonce, run.cycle_nonce);
        assert_eq!(after.unsuccessful_attempts_at_cursor, 2);
        assert_eq!(after.last_committed_step_block, None);
        assert_eq!(after.last_attempt_block, run.eligible_at);
        assert_eq!(after.last_step_outcome, Some(StepOutcome::FundingUnavailable));
        assert_eq!(after.opening_snapshot, run.opening_snapshot);
        assert_eq!(after.opening_predicate_results, run.opening_predicate_results);
        assert_eq!(after.funding_snapshot, run.funding_snapshot);
        assert!(after.eligible_at > run.eligible_at);
        assert_eq!((Assets::balance(&actor, asset), Assets::balance(&owner, asset)), custody);
        assert!(matches!(ActorControlLocators::<Test>::get(actor_id),
          Some(ActorControlLocation::Waiting { key: WakeupKey::Block(at), .. }) if at == after.eligible_at));
        #[cfg(feature = "try-runtime")]
        Pallet::<Test>::do_try_state().expect("two-leg retry reconciles exact Waiting ownership");
      });
    }
    let tail_legs = 2 * (count - 1);
    let minimum_opening =
      tail_legs.saturating_sub(<<Test as Config>::MaxFundingTrackedAssets as Get<u32>>::get());
    for opening_legs in [
      minimum_opening,
      minimum_opening + 1,
      tail_legs - 1,
      tail_legs,
    ] {
      for head_predicates in 0..=predicates {
        for head_opening in 0..=head_predicates {
          new_test_ext().execute_with(|| {
            frame_system::Pallet::<Test>::set_block_number(1);
            GlobalCircuitBreaker::<Test>::put(false);
            let owner = account("head-retry-owner", 0, 0);
            ensure_creation_balance::<Test>(&owner);
            let auxiliary_start = count * (2 + predicates);
            let assets = Helper::setup_predicate_assets(&owner, auxiliary_start + 1 + predicates)
              .expect("head and predicate assets exist beyond the distinct tail assets");
            let retry_asset = assets[auxiliary_start as usize];
            assert_ne!(retry_asset, <Test as Config>::FeeNativeAssetId::get());
            let mut steps = make_reachable_opening_steps::<Test>(count)
              .expect("reference host admits maximum Contract");
            for (index, step) in steps.iter_mut().skip(1).enumerate() {
              let ActorTask::AddLiquidity { amount_a, amount_b, .. } = &mut step.task else {
                unreachable!()
              };
              for (offset, amount) in [amount_a, amount_b].into_iter().enumerate() {
                if (index * 2 + offset) as u32 >= opening_legs {
                  *amount = AmountResolution::PercentageOfLastFunding(Perbill::one());
                }
              }
            }
            steps[0] = Step {
              precondition: (head_predicates > 0).then(|| packed_predicate_clauses::<Test>(
                (0..head_predicates).map(|index| TimedPredicate {
                  timing: if index < head_opening {
                    ObservationTiming::Opening
                  } else {
                    ObservationTiming::Current
                  },
                  predicate: Predicate::BalanceBelow {
                    asset: assets[(auxiliary_start + 1 + index) as usize],
                    threshold: (index + 1).into(),
                  },
                }).collect(),
                <<Test as Config>::MaxPredicatesPerClause as Get<u32>>::get(),
              )),
              task: ActorTask::Transfer {
                to: account("head-retry-recipient", 0, 0),
                asset: retry_asset,
                amount: AmountResolution::Fixed(One::one()),
              },
              on_error: StepErrorPolicy::RetryLater {
                max_attempts: <<Test as Config>::MaxRetryAttempts as Get<u32>>::get(),
              },
            };
            Pallet::<Test>::create_system_actor(
              RawOrigin::Root.into(),
              owner,
              Mutability::Mutable,
              system_contract::<Test>(
                Schedule { trigger: Trigger::manual(), cooldown_blocks: 2 },
                steps,
              ),
            )
            .expect("authored head and coupled tail are admitted");
            let actor_id = NextActorId::<Test>::get() - 1;
            fund_reachable_update_assets::<Test>(actor_id, 10);
            Pallet::<Test>::manual_trigger(RawOrigin::Signed(owner).into(), actor_id)
              .expect("real occurrence admits Opening");
            let (opening_state, _, opening_step) =
              Pallet::<Test>::load_current_step_service_state(actor_id)
                .expect("admitted Opening service state exists");
            let reserved_control = opening_step.resources.control;
            let opening_instance = Pallet::<Test>::derive_active_actor_view(
              opening_state.identity, opening_state.hot, opening_state.contract,
            );
            let opening_context = Pallet::<Test>::execution_step_control_weight_context(
              &opening_instance, None, &opening_step,
            ).expect("Opening context retains admission geometry");
            assert_eq!(opening_context.funding_snapshot_entries,
              <<Test as Config>::MaxFundingTrackedAssets as Get<u32>>::get());
            Pallet::<Test>::execute_cycle(Weight::MAX);
            let state = Pallet::<Test>::active_actor_state(actor_id).expect("Opening retains Actor");
            let run = state.run_state.expect("unfunded head publishes real retry");
            assert_eq!(state.hot.cycle_state, CycleState::Suspended);
            assert_eq!(state.identity.cycle_nonce, 0);
            assert_eq!(run.cursor, 0);
            assert_eq!(run.cycle_nonce, 1);
            assert_eq!(run.last_committed_step_block, None);
            assert_eq!(run.unsuccessful_attempts_at_cursor, 1);
            assert_eq!(run.last_step_outcome, Some(StepOutcome::FundingUnavailable));
            assert_eq!(run.opening_snapshot.len() as u32, opening_legs);
            assert_eq!(run.funding_snapshot.len() as u32, tail_legs - opening_legs);
            assert_eq!(
              run.opening_predicate_results.len() as u32,
              (count - 1) * predicates + head_opening,
            );
            assert!(run.opening_predicate_results.iter()
              .take(head_opening as usize).all(|value| *value == Ok(true)));
            assert!(state.funding.funding_accumulated.is_empty());
            let retained = run.encode();
            frame_system::Pallet::<Test>::set_block_number(run.eligible_at);
            let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
            let stats = Pallet::<Test>::drain_overdue_wakeups_cursor(run.eligible_at, &mut meter);
            assert_eq!(stats.ready_entries, 1);
            assert_eq!(
              ActorRunStateStore::<Test>::get(actor_id).expect("due Run persists").encode(),
              retained,
            );
            #[cfg(feature = "try-runtime")]
            Pallet::<Test>::do_try_state().expect("real head retry Ready source reconciles");
            let (state, admission, loaded_step) =
              benchmark_fixture_consume_frame_current_step_service_state::<Test>(actor_id);
            let instance = Pallet::<Test>::derive_active_actor_view(
              state.identity.clone(), state.hot.clone(), state.contract.clone(),
            );
            let context = Pallet::<Test>::execution_step_control_weight_context(
              &instance, state.run_state.as_ref(), &loaded_step,
            ).expect("Suspended context uses retained Run geometry");
            assert_eq!(context.funding_snapshot_entries, tail_legs - opening_legs);
            assert_eq!(loaded_step.resources.control, reserved_control);
            let actor = state.identity.sovereign_account;
            let recipient = account("head-retry-recipient", 0, 0);
            let custody: Vec<_> = assets.iter().map(|asset| (
              Assets::balance(&actor, *asset), Assets::balance(&recipient, *asset),
            )).collect();
            assert_eq!(
              loaded_step.step.precondition.as_ref().map_or(0, Precondition::evaluation_units),
              head_predicates + head_opening,
            );
            let effect = execute_reachable_step_inner::<Test>(
              actor_id, state, admission, loaded_step, run.eligible_at,
            );
            assert_eq!(effect, Weight::zero());
            let after = ActorRunStateStore::<Test>::get(actor_id).expect("second retry retains Run");
            assert_eq!(after.cursor, 0);
            assert_eq!(after.cycle_nonce, run.cycle_nonce);
            assert_eq!(after.last_committed_step_block, None);
            assert_eq!(after.unsuccessful_attempts_at_cursor, 2);
            assert_eq!(after.last_step_outcome, Some(StepOutcome::FundingUnavailable));
            assert_eq!(after.opening_snapshot, run.opening_snapshot);
            assert_eq!(after.opening_predicate_results, run.opening_predicate_results);
            assert_eq!(after.funding_snapshot, run.funding_snapshot);
            assert_eq!(assets.iter().map(|asset| (
              Assets::balance(&actor, *asset), Assets::balance(&recipient, *asset),
            )).collect::<Vec<_>>(), custody);
            assert!(matches!(ActorControlLocators::<Test>::get(actor_id), Some(ActorControlLocation::Waiting { key: WakeupKey::Block(at), .. }) if at == after.eligible_at));
            #[cfg(feature = "try-runtime")]
            Pallet::<Test>::do_try_state().expect("second retry Waiting destination reconciles");
          });
        }
      }
    }
  }
  #[cfg(test)]
  #[test]
  fn reachable_running_inner_fragment_predicate_boundaries() {
    let maximum = <<Test as Config>::MaxContractSteps as Get<u32>>::get();
    for branch in [RunningInnerBranch::Complete, RunningInnerBranch::Progress] {
      let minimum = if matches!(branch, RunningInnerBranch::Complete) {
        1
      } else {
        2
      };
      for fragment in minimum..=maximum.saturating_sub(1).min(MAX_STEPS_PER_TAIL_CHUNK) {
        for predicates in [0, benchmark_predicate_capacity::<Test>()] {
          new_test_ext().execute_with(|| {
            let (actor_id, cursor) =
              prepare_reachable_running_inner::<Test>(fragment, predicates, branch)
                .expect("host-feasible Running inner boundary is reachable");
            let now = frame_system::Pallet::<Test>::block_number();
            let (state, admission, loaded_step) =
              benchmark_fixture_consume_frame_current_step_service_state::<Test>(actor_id);
            execute_reachable_step_inner::<Test>(actor_id, state, admission, loaded_step, now);
            assert_reachable_running_inner::<Test>(actor_id, cursor, now, branch);
          });
          new_test_ext().execute_with(|| {
            let fixture =
              prepare_reachable_suspended_tail_skip::<Test>(fragment, predicates, branch)
                .expect("host-feasible Suspended skip boundary is reachable");
            let now = frame_system::Pallet::<Test>::block_number();
            let (state, admission, loaded_step) =
              benchmark_fixture_consume_frame_current_step_service_state::<Test>(fixture.actor_id);
            let effect_weight = execute_reachable_step_inner::<Test>(
              fixture.actor_id,
              state,
              admission,
              loaded_step,
              now,
            );
            assert_eq!(effect_weight, Weight::zero());
            assert_reachable_suspended_tail_skip::<Test>(fixture, now, branch);
          });
        }
      }
    }
    for fragment in 1..=maximum.saturating_sub(1).min(MAX_STEPS_PER_TAIL_CHUNK) {
      for predicates in [0, benchmark_predicate_capacity::<Test>()] {
        new_test_ext().execute_with(|| {
          let (actor_id, cursor) =
            prepare_reachable_suspended_tail_retry::<Test>(fragment, predicates)
              .expect("host-feasible Suspended retry boundary is reachable");
          let now = frame_system::Pallet::<Test>::block_number();
          let (state, admission, loaded_step) =
            benchmark_fixture_consume_frame_current_step_service_state::<Test>(actor_id);
          let effect_weight =
            execute_reachable_step_inner::<Test>(actor_id, state, admission, loaded_step, now);
          assert_eq!(effect_weight, Weight::zero());
          assert_reachable_suspended_tail_retry_state::<Test>(actor_id, cursor, 2);
        });
      }
    }
  }
  #[cfg(test)]
  #[test]
  fn reachable_running_tail_fragment_boundaries() {
    let maximum = <<Test as Config>::MaxContractSteps as Get<u32>>::get();
    for fragment in 1..=maximum.saturating_sub(1).min(MAX_STEPS_PER_TAIL_CHUNK) {
      new_test_ext().execute_with(|| {
        let (actor_id, ticket) = prepare_reachable_running_tail::<Test>(fragment)
          .expect("host-feasible tail fragment is reachable");
        let retained = ActorRunStateStore::<Test>::get(actor_id)
          .expect("real tail Run exists")
          .encode();
        Pallet::<Test>::load_current_step_plan_from_storage(ticket)
          .expect("each real partial or full tail fragment loads");
        assert_reachable_running_tail_unchanged::<Test>(actor_id, &retained);
      });
    }
  }
  #[cfg(test)]
  #[test]
  fn reachable_update_allocation_boundaries_match_authored_contract() {
    let legs = <<Test as Config>::MaxContractSteps as Get<u32>>::get() * 2;
    let minimum =
      legs.saturating_sub(<<Test as Config>::MaxFundingTrackedAssets as Get<u32>>::get());
    if minimum > 0 {
      new_test_ext().execute_with(|| {
        let (steps, _) = reachable_retry_contract_allocation::<Test>(minimum - 1, 0)
          .expect("the authored Contract fits Step and predicate storage bounds");
        let next_actor = NextActorId::<Test>::get();
        assert_eq!(
          Pallet::<Test>::create_system_actor(
            RawOrigin::Root.into(),
            account("funding-bound-owner", 0, 0),
            Mutability::Mutable,
            system_contract::<Test>(
              Schedule {
                trigger: Trigger::Manual,
                cooldown_blocks: 100
              },
              steps,
            ),
          ),
          Err(Error::<Test>::TooManyContractSteps.into()),
          "independent funding sources exceed the host bound by one",
        );
        assert_eq!(NextActorId::<Test>::get(), next_actor);
        assert!(!ActorFunding::<Test>::contains_key(next_actor));
        assert!(!ActorContractHeads::<Test>::contains_key(next_actor));
        #[cfg(feature = "try-runtime")]
        Pallet::<Test>::do_try_state().expect("rejected admission publishes no Actor state");
      });
    }
    for opening_legs in [minimum, minimum + (legs - minimum) / 2, legs] {
      for family in [
        TriggerFamily::ObservationCrossing,
        TriggerFamily::ObservationChange,
      ] {
        new_test_ext().execute_with(|| {
          let (owner, actor_id, replacement, old_feed) =
            prepare_reachable_update::<Test>(opening_legs, family)
              .expect("host-feasible allocation boundary is admitted");
          let expected = replacement.clone();
          execute_reachable_update::<Test>(owner, actor_id, replacement);
          assert_reachable_update::<Test>(actor_id, &expected, old_feed);
        });
      }
    }
  }
  #[cfg(test)]
  #[test]
  fn mixed_waiting_reference_close_uses_real_opening() {
    for unlink in [false, true] {
      new_test_ext().execute_with(|| {
        let fixture = prepare_mixed_waiting_close::<Test>(unlink)
          .expect("mixed Waiting reference is reachable");
        Pallet::<Test>::close_actor(RawOrigin::Root.into(), fixture.actor_id).unwrap();
        assert_mixed_waiting_close::<Test>(&fixture, unlink);
      });
    }
  }

  #[cfg(test)]
  #[test]
  fn mixed_waiting_deadline_close_reclaims_full_heap_tail() {
    new_test_ext().execute_with(|| {
      let (fixture, keys) = prepare_mixed_waiting_deadline_close::<Test>()
        .expect("last Tick reference and full heap follow ordinary lifecycle transitions");
      Pallet::<Test>::close_actor(RawOrigin::Root.into(), fixture.actor_id).unwrap();
      assert_mixed_waiting_deadline_close::<Test>(&fixture, keys);
    });
  }

  #[cfg(test)]
  #[test]
  fn full_waiting_heap_close_covers_upward_and_tail_removal() {
    for upward in [true, false] {
      new_test_ext().execute_with(|| {
        let fixture = prepare_full_waiting_heap_close::<Test>(upward);
        Pallet::<Test>::close_actor(RawOrigin::Root.into(), fixture.actor_id).unwrap();
        assert_full_waiting_heap_close::<Test>(&fixture);
      });
    }
  }

  #[cfg(test)]
  #[test]
  fn cadenced_opening_rearms_tick_heap_before_ready_continuation() {
    use alloc::collections::BTreeMap;

    for existing_destination in [true, false] {
      new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(1);
        let mut steps = inert_contract_steps_of_len::<Test>(2);
        steps[0].task = ActorTask::Transfer {
          to: account("cadence-noop-recipient", 0, 0),
          asset: <Test as Config>::FeeNativeAssetId::get(),
          amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
        };
        Pallet::<Test>::create_system_actor(
          RawOrigin::Root.into(),
          account("cadence-rearm-target", 0, 0),
          Mutability::Mutable,
          system_contract::<Test>(
            Schedule {
              trigger: Trigger::cadenced(100),
              cooldown_blocks: 0,
            },
            steps,
          ),
        )
        .expect("real Cadenced Contract is admitted");
        let actor_id = NextActorId::<Test>::get() - 1;
        let first = Pallet::<Test>::actor_hot(actor_id)
          .and_then(|hot| hot.trigger_wakeup_pointer)
          .expect("first cadence")
          .tick;
        let next = first + 100;
        for index in 0..96u32 {
          let every = if index == 0 && existing_destination {
            200
          } else {
            1_000 + index
          };
          Pallet::<Test>::create_system_actor(
            RawOrigin::Root.into(),
            account("cadence-rearm-competitor", index, 0),
            Mutability::Mutable,
            system_contract::<Test>(
              Schedule {
                trigger: Trigger::cadenced(u64::from(every)),
                cooldown_blocks: 0,
              },
              make_inert_contract_steps::<Test>(),
            ),
          )
          .expect("competing Tick primary is admitted");
        }
        frame_system::Pallet::<Test>::set_block_number(first);
        let (mut due, stats) =
          Pallet::<Test>::wakeup_substrate_drain_key(WakeupKey::Tick(first), 1);
        assert_eq!(stats.entries_scanned, 1);
        let (due_actor, state, admission, loaded_step) = due.pop().expect("real due primary");
        assert_eq!(due_actor, actor_id);
        assert_eq!(
          Pallet::<Test>::process_due_temporal_occurrence_loaded(
            actor_id,
            state,
            admission,
            loaded_step,
            first,
          ),
          Ok(false)
        );
        assert_eq!(WakeupCursorLen::<Test>::get(WakeupClock::Tick), 96);
        #[cfg(feature = "try-runtime")]
        Pallet::<Test>::do_try_state().expect("due collection reconciles before Opening");
        let before: BTreeMap<_, _> = WakeupCursorPages::<Test>::iter().collect();
        Pallet::<Test>::execute_cycle(Weight::MAX);
        let (location, cell) =
          Pallet::<Test>::actor_control_cell(actor_id).expect("Running primary");
        assert!(matches!(location, ActorControlLocation::Ready { .. }));
        assert_eq!(cell.cursor, 1);
        let pointer = Pallet::<Test>::actor_hot(actor_id)
          .and_then(|hot| hot.trigger_wakeup_pointer)
          .expect("independent rearmed reference");
        assert_eq!(pointer.tick, next);
        assert!(matches!(
          ActorWaitingFrameChunks::<Test>::get((WakeupKey::Tick(next), pointer.page_id))
            .expect("reference page")
            .entries[pointer.slot as usize],
          Some(ActorWaitingEntry::Reference(_))
        ));
        assert_eq!(
          crate::mock::last_step_control_execution()
            .expect("actual selection")
            .placement,
          crate::StepControlPlacement::Queue
        );
        let after: BTreeMap<_, _> = WakeupCursorPages::<Test>::iter().collect();
        if existing_destination {
          assert_eq!(before, after);
        } else {
          let changed = after
            .iter()
            .filter(|(key, page)| before.get(key) != Some(page))
            .count();
          assert_eq!(changed, 3, "Opening itself repairs three Tick heap pages");
          assert_eq!(WakeupCursorLen::<Test>::get(WakeupClock::Tick), 97);
        }
        #[cfg(feature = "try-runtime")]
        Pallet::<Test>::do_try_state()
          .expect("Ready primary and independent Tick reference reconcile");
      });
    }
  }

  #[cfg(test)]
  #[test]
  fn opening_retry_reconciles_existing_and_new_waiting_heap_keys() {
    use alloc::collections::BTreeMap;

    let mut logical_results = Vec::new();
    for destination_entries in [0, 1, 32] {
      logical_results.push(new_test_ext().execute_with(|| {
        let (actor_id, keys, due) = prepare_reachable_waiting_opening::<Test>(destination_entries)
          .expect("full host population is admitted");
        let (_, source) = Pallet::<Test>::actor_control_cell(actor_id).expect("Ready source");
        let capacity = Pallet::<Test>::effective_active_actor_limit();
        assert_eq!(ActiveActorCount::<Test>::get(), capacity);
        assert_eq!(keys, 1_023 - destination_entries.saturating_sub(1));
        let before: BTreeMap<_, _> = WakeupCursorPages::<Test>::iter().collect();
        Pallet::<Test>::execute_cycle(Weight::MAX);
        assert_reachable_waiting_opening::<Test>(
          actor_id,
          if destination_entries > 0 {
            keys
          } else {
            capacity
          },
          due,
        );
        let after: BTreeMap<_, _> = WakeupCursorPages::<Test>::iter().collect();
        if destination_entries > 0 {
          assert_eq!(before, after, "existing deadline needs no heap repair");
        } else {
          assert_eq!(Pallet::<Test>::wakeup_cursor_peek(), Some(due));
          let changed = after
            .iter()
            .filter(|(key, page)| before.get(key) != Some(page))
            .count();
          assert_eq!(changed, 6, "full mock population repairs six heap pages");
        }
        let execution =
          crate::mock::last_step_control_execution().expect("actual control selection");
        assert_eq!(execution.placement, crate::StepControlPlacement::Wakeup);
        (
          source.encode(),
          Pallet::<Test>::actor_run_state(actor_id).encode(),
          execution,
        )
      }));
    }
    assert!(logical_results.windows(2).all(|pair| pair[0] == pair[1]));
  }

  #[cfg(test)]
  #[test]
  fn consumed_waiting_publication_extrema_use_complete_host_populations() {
    let mut source_cells = Vec::new();
    for destination_entries in [0, 1, 32] {
      source_cells.push(new_test_ext().execute_with(|| {
        let (actor_id, count, keys, due) = prepare_reachable_waiting_publication::<Test>(
          destination_entries,
          0,
          ReachableOpeningProfile::RetryMin,
        )
        .expect("RetryMin publication and complete host population are admitted");
        let (_, source) = Pallet::<Test>::actor_control_cell(actor_id).expect("Ready source");
        let encoded_source = source.encode();
        let publish = benchmark_fixture_prepare_consumed_service_waiting::<Test>(actor_id, due);
        publish();
        assert_reachable_waiting_publication::<Test>(
          actor_id,
          count,
          keys,
          due,
          destination_entries,
        );
        assert_eq!(ActiveActorCount::<Test>::get(), 1_024);
        assert_eq!(count, 1);
        encoded_source
      }));
    }
    assert!(source_cells.windows(2).all(|pair| pair[0] == pair[1]));
  }

  #[cfg(test)]
  #[test]
  fn consumed_running_continued_wakeup_extrema_are_complete_successors() {
    for dense in [false, true] {
      new_test_ext().execute_with(|| {
        let actor_id = if dense {
          prepare_reachable_running::<Test>(3).expect("largest dense Running fixture is admitted")
        } else {
          prepare_low_capture_running::<Test>(None, 0)
            .expect("maximum-length low-capture Running fixture is admitted")
        };
        let source =
          ActorRunStateStore::<Test>::get(actor_id).expect("continued Running source exists");
        assert_eq!(source.cursor, 1);
        if dense {
          assert_eq!(source.opening_snapshot.len(), 6);
          assert_eq!(
            source.opening_predicate_results.len() as u32,
            3 * benchmark_predicate_capacity::<Test>()
          );
        } else {
          assert!(source.opening_snapshot.is_empty());
          assert!(source.opening_predicate_results.is_empty());
        }
        let now = source.eligible_at;
        frame_system::Pallet::<Test>::set_block_number(now);
        let (state, admission, loaded_step) =
          benchmark_fixture_consume_frame_current_step_service_state::<Test>(actor_id);
        benchmark_fixture_seed_saturated_ready_tombstones::<Test>();
        assert_eq!(
          execute_reachable_step_inner::<Test>(actor_id, state, admission, loaded_step, now),
          Weight::zero()
        );
        let successor =
          ActorRunStateStore::<Test>::get(actor_id).expect("continued successor retains Run");
        assert_eq!(successor.cursor, 2);
        assert_eq!(successor.last_committed_step_block, Some(now));
        assert_eq!(
          crate::mock::last_step_control_execution()
            .expect("actual selector observes complete successor")
            .placement,
          crate::StepControlPlacement::Wakeup
        );
        assert!(matches!(
          ActorControlLocators::<Test>::get(actor_id),
          Some(ActorControlLocation::Waiting {
            key: WakeupKey::Block(at),
            ..
          }) if at == successor.eligible_at
        ));
        assert_eq!(ActorReadyOccupancy::<Test>::get(), 0);
        #[cfg(feature = "try-runtime")]
        Pallet::<Test>::do_try_state().expect("complete fallback successor is canonical");
      });
    }
  }

  #[cfg(test)]
  #[test]
  fn consumed_running_terminal_successors_publish_one_unsignaled_owner() {
    for failed in [false, true] {
      new_test_ext().execute_with(|| {
        let actor_id = if failed {
          prepare_run_terminal_failure_source::<Test>()
            .expect("maximum-length low-capture failure source is admitted")
        } else {
          prepare_low_capture_running_with_type::<Test>(
            ActorType::User,
            Some(Step {
              precondition: None,
              task: ActorTask::StopCycle,
              on_error: StepErrorPolicy::AbortCycle,
            }),
            0,
          )
          .expect("maximum-length low-capture completion source is admitted")
        };
        let before_hold =
          ActorStateHolds::<Test>::get(actor_id).expect("terminal User state hold exists");
        let now = ActorRunStateStore::<Test>::get(actor_id)
          .expect("low-capture terminal source retains Run")
          .eligible_at;
        frame_system::Pallet::<Test>::set_block_number(now);
        let (state, admission, loaded_step) =
          benchmark_fixture_consume_frame_current_step_service_state::<Test>(actor_id);
        let effect =
          execute_reachable_step_inner::<Test>(actor_id, state, admission, loaded_step, now);
        assert_eq!(
          effect,
          if failed {
            Weight::zero()
          } else {
            <Test as Config>::TaskEffectWeight::actual_effect_weight(
              &ActorTask::StopCycle,
              TaskEffectExecution::Invoked,
            )
            .expect("mock supplies StopCycle effect evidence")
          }
        );
        let successor =
          Pallet::<Test>::active_actor_state(actor_id).expect("terminal authority remains active");
        assert!(successor.run_state.is_none());
        assert_eq!(successor.identity.cycle_nonce, 1);
        assert_eq!(successor.hot.cycle_state, CycleState::Idle);
        assert!(!successor.hot.pending_signal);
        assert_eq!(successor.hot.unsuccessful_attempt_streak, u32::from(failed));
        assert!(successor.hot.queue_ticket.is_none() && successor.hot.wakeup_pointer.is_none());
        assert!(matches!(
          ActorControlLocators::<Test>::get(actor_id),
          Some(ActorControlLocation::Unsignaled)
        ));
        assert_eq!(ActorReadyOccupancy::<Test>::get(), 0);
        let after_hold =
          ActorStateHolds::<Test>::get(actor_id).expect("terminal User state hold remains owned");
        assert_eq!(after_hold.owner, before_hold.owner);
        assert_eq!(after_hold.breakdown, before_hold.breakdown);
        let execution = crate::mock::last_step_control_execution()
          .expect("actual selector observes complete terminal successor");
        assert_eq!(execution.phase, crate::StepControlPhase::Running);
        assert_eq!(
          execution.outcome,
          if failed {
            crate::StepControlOutcome::Failed
          } else {
            crate::StepControlOutcome::Completed
          }
        );
        assert_eq!(execution.placement, crate::StepControlPlacement::None);
        assert_eq!(
          execution.task_effect,
          if failed {
            TaskEffectExecution::NotInvoked
          } else {
            TaskEffectExecution::Invoked
          }
        );
        #[cfg(feature = "try-runtime")]
        Pallet::<Test>::do_try_state().expect("complete terminal successor is canonical");
      });
    }
  }

  #[cfg(test)]
  #[test]
  fn consumed_running_suspension_selects_queue_or_wakeup_from_exact_eligibility() {
    for (cooldown_blocks, expected_placement) in [
      (0, crate::StepControlPlacement::Queue),
      (2, crate::StepControlPlacement::Wakeup),
    ] {
      new_test_ext().execute_with(|| {
        let actor_id = prepare_run_owner_suspension_source::<Test>(false, cooldown_blocks)
          .expect("maximum-length low-capture suspension source is admitted");
        let now = ActorRunStateStore::<Test>::get(actor_id)
          .expect("low-capture Running retry source exists")
          .eligible_at;
        frame_system::Pallet::<Test>::set_block_number(now);
        let (state, admission, loaded_step) =
          benchmark_fixture_consume_frame_current_step_service_state::<Test>(actor_id);
        assert_eq!(
          execute_reachable_step_inner::<Test>(actor_id, state, admission, loaded_step, now),
          Weight::zero()
        );
        let successor =
          ActorRunStateStore::<Test>::get(actor_id).expect("suspended successor retains Run");
        assert_eq!(successor.cursor, 1);
        assert_eq!(successor.last_attempt_block, now);
        assert_eq!(
          successor.suspension,
          Some(SuspensionReason::FundingUnavailable)
        );
        assert_eq!(
          crate::mock::last_step_control_execution()
            .expect("actual selector observes suspension successor")
            .placement,
          expected_placement
        );
        match expected_placement {
          crate::StepControlPlacement::Queue => {
            assert_eq!(successor.eligible_at, now + 1);
            assert!(matches!(
              ActorControlLocators::<Test>::get(actor_id),
              Some(ActorControlLocation::Ready { .. })
            ));
            assert_eq!(ActorReadyOccupancy::<Test>::get(), 1);
          }
          crate::StepControlPlacement::Wakeup => {
            assert_eq!(successor.eligible_at, now + 2);
            assert!(matches!(
              ActorControlLocators::<Test>::get(actor_id),
              Some(ActorControlLocation::Waiting {
                key: WakeupKey::Block(at),
                ..
              }) if at == successor.eligible_at
            ));
            assert_eq!(ActorReadyOccupancy::<Test>::get(), 0);
          }
          _ => unreachable!(),
        }
        #[cfg(feature = "try-runtime")]
        Pallet::<Test>::do_try_state().expect("complete suspension successor is canonical");
      });
    }
  }

  #[cfg(test)]
  #[test]
  fn continued_step_reports_waiting_after_ready_capacity_fallback() {
    new_test_ext().execute_with(|| {
      let (actor_id, _) = prepare_reachable_opening::<Test>(1, ReachableOpeningProfile::Minimal)
        .expect("admitted Opening fixture");
      let (state, admission, loaded_step) =
        benchmark_fixture_consume_frame_current_step_service_state::<Test>(actor_id);
      // Exercise the consumed-Step placement boundary with real competing Ready owners.
      // This is not evidence of a complete Executive interleaving that fills the queue.
      let capacity = <<Test as Config>::MaxQueueLength as Get<u32>>::get();
      for seed in 0..capacity {
        let other = bench_create_system_manual::<Test>(60_000_000 + seed);
        let owner = Pallet::<Test>::actor_identity(other)
          .expect("competitor identity")
          .owner;
        Pallet::<Test>::manual_trigger(RawOrigin::Signed(owner).into(), other)
          .expect("competing Ready ticket fits");
        if seed == 1 {
          frame_system::Pallet::<Test>::set_block_number(2);
          Pallet::<Test>::deactivate_actor(RawOrigin::Root.into(), other)
            .expect("non-head lifecycle churn frees an Active slot but retains its tombstone");
        }
      }
      assert_eq!(
        ActorReadyTail::<Test>::get() - ActorReadyHead::<Test>::get(),
        u64::from(capacity)
      );
      execute_reachable_step_inner::<Test>(actor_id, state, admission, loaded_step, 2);
      assert!(matches!(
        ActorControlLocators::<Test>::get(actor_id),
        Some(ActorControlLocation::Waiting {
          key: WakeupKey::Block(3),
          ..
        })
      ));
      assert_eq!(
        crate::mock::last_step_control_execution().map(|execution| execution.placement),
        Some(crate::StepControlPlacement::Wakeup)
      );
      assert_eq!(
        ActorRunStateStore::<Test>::get(actor_id)
          .expect("retained Run")
          .cursor,
        1
      );
      #[cfg(feature = "try-runtime")]
      Pallet::<Test>::do_try_state().expect("fallback keeps one canonical process owner");
    });
  }

  #[cfg(test)]
  #[test]
  fn upward_heap_repair_preserves_indices_and_exposes_page_spread() {
    for (removed_index, expected_index, changed_pages) in
      [(6_143u32, 2u32, 10usize), (8_447, 7, 11)]
    {
      new_test_ext().execute_with(|| {
        clear_host_genesis_wakeup_placements::<Test>();
        let size = <<Test as Config>::WakeupPageSize as Get<u32>>::get();
        let len = <<Test as Config>::MaxActiveActors as Get<u32>>::get();
        assert_eq!((size, len), (32, 10_000));
        let mut tail_ancestors = Vec::new();
        let mut index = len - 1;
        loop {
          tail_ancestors.push(index);
          if index == 0 {
            break;
          }
          index = (index - 1) / 2;
        }
        // Complete heap-only authority: no claim that these keys own live Actor/Waiting state.
        for page_id in 0..len.div_ceil(size) {
          let mut page = WakeupCursorPageOf::<Test>::default();
          for index in page_id * size..((page_id + 1) * size).min(len) {
            let block = if tail_ancestors.contains(&index) {
              u32::BITS - (index + 1).leading_zeros()
            } else {
              100_000 + index
            };
            let key = WakeupKey::Block(u64::from(block));
            page.try_push(key).expect("complete heap page fits");
            ActorWaitingCursorIndices::<Test>::insert(key, index);
          }
          WakeupCursorPages::<Test>::insert((WakeupClock::Block, u64::from(page_id)), page);
        }
        WakeupCursorLen::<Test>::insert(WakeupClock::Block, len);
        assert_wakeup_cursor_page_indices::<Test>();
        let before = WakeupCursorPages::<Test>::iter()
          .map(|(key, page)| (key, page.encode()))
          .collect::<Vec<_>>();
        assert_eq!(before.len(), len.div_ceil(size) as usize);
        let tail_key = Pallet::<Test>::wakeup_cursor_get(WakeupClock::Block, len - 1)
          .expect("complete heap tail exists");
        assert!(Pallet::<Test>::wakeup_cursor_remove(u64::from(
          100_000 + removed_index
        )));
        assert_eq!(WakeupCursorLen::<Test>::get(WakeupClock::Block), len - 1);
        assert_eq!(
          ActorWaitingCursorIndices::<Test>::get(tail_key),
          Some(expected_index)
        );
        assert!(!ActorWaitingCursorIndices::<Test>::contains_key(
          WakeupKey::Block(u64::from(100_000 + removed_index))
        ));
        assert_wakeup_cursor_page_indices::<Test>();
        assert_eq!(WakeupCursorPages::<Test>::iter().count(), before.len());
        assert_eq!(
          before
            .iter()
            .filter(|(key, encoded)| {
              WakeupCursorPages::<Test>::get(*key)
                .map(|page| page.encode())
                .as_ref()
                != Some(encoded)
            })
            .count(),
          changed_pages
        );
      });
    }
  }

  #[cfg(test)]
  #[test]
  fn pipeline_insolvency_cleans_each_admitted_tail_and_future_window() {
    let maximum = <<Test as Config>::MaxContractSteps as Get<u32>>::get();
    for count in 0..=maximum {
      for with_window in [false, true] {
        new_test_ext().execute_with(|| {
          let mut steps = make_max_contract_steps::<Test>(999);
          steps.truncate(count as usize);
          let (actor_id, before) =
            prepare_user_pipeline_insolvency::<Test>(true, steps, with_window);
          assert_eq!(
            ActorContractTailChunks::<Test>::iter_prefix(actor_id).count(),
            count.saturating_sub(1).div_ceil(MAX_STEPS_PER_TAIL_CHUNK) as usize
          );
          let hot = benchmark_fixture_hot::<Test>(actor_id).unwrap();
          assert_eq!(hot.wakeup_pointer.is_some(), with_window);
          let pass = Pallet::<Test>::execute_cycle(Weight::MAX);
          assert!(pass.effect_consumed.is_zero() && !pass.starved);
          assert_user_pipeline_insolvency::<Test>(actor_id, before);
          if let Some(pointer) = hot.wakeup_pointer {
            assert!(!Pallet::<Test>::wakeup_page_entry_matches(
              pointer, actor_id
            ));
            assert!(!ActorWaitingCursorIndices::<Test>::contains_key(
              pointer.block
            ));
          }
        });
      }
    }
  }

  #[cfg(test)]
  #[test]
  fn crossing_rearm_unavailability_preserves_paid_head_until_host_recovers() {
    for unavailable in [
      ScalarObservationState::Unavailable,
      ScalarObservationState::Uninitialized,
    ] {
      new_test_ext().execute_with(|| {
        let (actor_id, feed) =
          prepare_zero_step_observation::<Test>(Some(CrossingPhase::WaitingForRearm), false);
        let available = crate::mock::MockObservationProvider::observe(&feed, 0, u32::MAX);
        let follower = create_zero_step_system_follower::<Test>();
        crate::mock::set_observation(feed, unavailable);
        let state = Pallet::<Test>::active_actor_state(actor_id)
          .unwrap()
          .encode();
        let head = ActorReadyHead::<Test>::get();
        let events = frame_system::Pallet::<Test>::events();
        let before = user_pipeline_accounting::<Test>(actor_id);
        let complete_refusal =
          <<Test as Config>::WeightInfo as WeightInfo>::scheduler_paged_zero_step_user_crossing_unavailable();
        if unavailable == ScalarObservationState::Unavailable {
          for budget in [
            Weight::from_parts(complete_refusal.ref_time().saturating_sub(1), u64::MAX),
            Weight::from_parts(u64::MAX, complete_refusal.proof_size().saturating_sub(1)),
          ] {
            let refused = Pallet::<Test>::execute_cycle(budget);
            assert!(refused.consumed.all_lte(budget));
            assert!(refused.effect_consumed.is_zero() && refused.starved);
            assert_eq!(
              Pallet::<Test>::active_actor_state(actor_id)
                .unwrap()
                .encode(),
              state
            );
            assert_eq!(ActorReadyHead::<Test>::get(), head);
            assert_eq!(frame_system::Pallet::<Test>::events(), events);
          }
        }
        let pass = Pallet::<Test>::execute_cycle(Weight::MAX);
        assert!(!pass.consumed.is_zero());
        assert!(pass.effect_consumed.is_zero() && pass.starved);
        if unavailable == ScalarObservationState::Unavailable {
          assert_eq!(
            pass.consumed, complete_refusal,
            "the complete Crossing refusal replaces provisional FIFO discovery and actor-probe accounting"
          );
        }
        assert_eq!(
          Pallet::<Test>::active_actor_state(actor_id)
            .unwrap()
            .encode(),
          state
        );
        assert_eq!(ActorReadyHead::<Test>::get(), head);
        assert_eq!(
          benchmark_fixture_identity::<Test>(follower)
            .unwrap()
            .cycle_nonce,
          0
        );
        assert_eq!(frame_system::Pallet::<Test>::events(), events);
        assert_eq!(
          <Test as Config>::AssetOps::balance(
            &before.payer,
            <Test as Config>::FeeNativeAssetId::get()
          ),
          before.payer_balance
        );
        assert_eq!(
          <Test as Config>::AssetOps::balance(
            &<Test as Config>::FeeSink::get(),
            <Test as Config>::FeeNativeAssetId::get()
          ),
          before.sink_balance
        );
        crate::mock::set_observation(feed, available);
        Pallet::<Test>::execute_cycle(Weight::MAX);
        assert_user_pipeline_accounting::<Test>(actor_id, before);
        assert_eq!(
          benchmark_fixture_identity::<Test>(actor_id)
            .unwrap()
            .cycle_nonce,
          1
        );
        assert_eq!(
          benchmark_fixture_identity::<Test>(follower)
            .unwrap()
            .cycle_nonce,
          1
        );
        assert_eq!(ActorReadyHead::<Test>::get(), ActorReadyTail::<Test>::get());
        #[cfg(feature = "try-runtime")]
        Pallet::<Test>::do_try_state().unwrap();
      });
    }
  }

  #[cfg(test)]
  #[test]
  fn crossing_rearm_unavailability_does_not_block_pipeline_insolvency_close() {
    new_test_ext().execute_with(|| {
      let (actor_id, before) = prepare_zero_step_user_insolvency::<Test>(true);
      let feed = Pallet::<Test>::active_actor_view(actor_id)
        .unwrap()
        .trigger
        .observation_crossing_contract()
        .unwrap()
        .feed
        .to_owned();
      crate::mock::set_observation(feed, ScalarObservationState::Unavailable);
      Pallet::<Test>::execute_cycle(Weight::MAX);
      assert_user_pipeline_insolvency::<Test>(actor_id, before);
    });
  }

  #[cfg(test)]
  impl_benchmark_test_suite!(Pallet, new_test_ext(), Test, extra = false);
}
