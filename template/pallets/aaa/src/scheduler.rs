use super::pallet::*;
use super::{AssetOps, FundingAuthority, weights::WeightInfo};
use alloc::{
  collections::{BTreeSet, VecDeque},
  vec::Vec,
};
use frame::prelude::*;
use polkadot_sdk::sp_runtime::traits::{One, Saturating, Zero};
use polkadot_sdk::sp_weights::WeightMeter;

enum AdmissionDecision {
  Admit(Weight),
  Closed(Weight),
  CloseFailed(Weight),
  Defer(DeferReason),
  Skip,
}

impl<T: Config> Pallet<T> {
  pub(crate) fn execute_cycle(remaining_weight: Weight) -> Weight {
    if remaining_weight.is_zero() {
      return Weight::zero();
    }
    // Two-layer scheduler: overdue temporal wakeups are first admitted into the
    // active run queue, then execution proceeds from the merged run-queue state.
    let mut cycle_meter = WeightMeter::with_limit(remaining_weight);
    let queue_storage_weight = Self::queue_storage_weight_upper();
    if !cycle_meter.can_consume(queue_storage_weight) {
      return Weight::zero();
    }
    cycle_meter.consume(queue_storage_weight);
    let current = CurrentQueue::<T>::get().into_inner();
    let staged = NextQueue::<T>::get().into_inner();
    let queue_units = current
      .len()
      .saturating_add(staged.len())
      .saturating_add(T::MaxQueueInsertionsPerBlock::get() as usize) as u64;
    let queue_work_weight = Self::queue_item_weight_upper().saturating_mul(queue_units);
    if !cycle_meter.can_consume(queue_work_weight) {
      return cycle_meter.consumed();
    }
    cycle_meter.consume(queue_work_weight);
    let now = frame_system::Pallet::<T>::block_number();
    let queue_epoch = QueueEpoch::<T>::get();
    CurrentQueue::<T>::kill();
    NextQueue::<T>::kill();
    let (mut run_queue, mut queued_set) = Self::merge_queue_state(current, staged);
    let mut next_queue: VecDeque<AaaId> = VecDeque::new();
    let mut queue_insertions = 0u32;
    let mut periodic_continuations: Vec<AaaId> = Vec::new();
    Self::drain_overdue_wakeups(
      now,
      &mut run_queue,
      &mut queued_set,
      &mut queue_insertions,
      &mut cycle_meter,
    );
    let max_executions = T::MaxExecutionsPerBlock::get();
    let probe_weight = Self::scheduler_probe_weight_upper();
    let mut executed = 0u32;
    while executed < max_executions {
      if run_queue.is_empty() || !cycle_meter.can_consume(probe_weight) {
        break;
      }
      cycle_meter.consume(probe_weight);
      let aaa_id = run_queue
        .pop_front()
        .expect("queue emptiness was checked before admitted pop");
      queued_set.remove(&aaa_id);
      ActorQueueEpoch::<T>::remove(aaa_id);
      let Some(instance) = AaaInstances::<T>::get(aaa_id) else {
        continue;
      };
      let cycle_weight_upper = match Self::apply_admission(aaa_id, &instance, &cycle_meter) {
        AdmissionDecision::Admit(weight) => weight,
        AdmissionDecision::Closed(weight) => {
          cycle_meter.consume(weight);
          continue;
        }
        AdmissionDecision::CloseFailed(weight) => {
          cycle_meter.consume(weight);
          Self::deposit_event(Event::CycleDeferred {
            aaa_id,
            reason: DeferReason::CloseTransitionFailed,
          });
          Self::enqueue_block_local(
            aaa_id,
            now,
            queue_epoch,
            &mut next_queue,
            &mut queue_insertions,
          );
          continue;
        }
        AdmissionDecision::Defer(reason) => {
          Self::deposit_event(Event::CycleDeferred { aaa_id, reason });
          Self::enqueue_block_local(
            aaa_id,
            now,
            queue_epoch,
            &mut next_queue,
            &mut queue_insertions,
          );
          continue;
        }
        AdmissionDecision::Skip => {
          if let Some(updated) = AaaInstances::<T>::get(aaa_id) {
            Self::preserve_skipped_readiness(&updated, now, &mut periodic_continuations);
          }
          continue;
        }
      };
      let _actual = Self::execute_single_cycle(aaa_id);
      cycle_meter.consume(cycle_weight_upper);
      executed = executed.saturating_add(1);
      if let Some(updated) = AaaInstances::<T>::get(aaa_id) {
        Self::schedule_next_timer_wakeup_local(&updated, now, &mut periodic_continuations);
      }
    }
    while let Some(aaa_id) = run_queue.pop_front() {
      queued_set.remove(&aaa_id);
      Self::carry_over_block_local(aaa_id, now, queue_epoch, &mut next_queue);
    }
    Self::merge_late_enqueues(now, queue_epoch, &mut next_queue, &mut queue_insertions);
    for aaa_id in periodic_continuations.into_iter() {
      Self::enqueue_block_local(
        aaa_id,
        now,
        queue_epoch,
        &mut next_queue,
        &mut queue_insertions,
      );
    }
    CurrentQueue::<T>::put(BoundedVec::<AaaId, T::MaxQueueLength>::truncate_from(
      next_queue.into_iter().collect::<Vec<AaaId>>(),
    ));
    QueueEpoch::<T>::put(queue_epoch.saturating_add(1));
    cycle_meter.consumed()
  }

  pub(crate) fn merge_queue_state(
    current: Vec<AaaId>,
    staged: Vec<AaaId>,
  ) -> (VecDeque<AaaId>, BTreeSet<AaaId>) {
    let mut run_queue: VecDeque<AaaId> = current.into();
    let mut queued_set: BTreeSet<AaaId> = BTreeSet::new();
    for aaa_id in &run_queue {
      queued_set.insert(*aaa_id);
    }
    for aaa_id in staged {
      if queued_set.insert(aaa_id) {
        run_queue.push_back(aaa_id);
      }
    }
    (run_queue, queued_set)
  }

  pub(crate) fn enqueue(aaa_id: AaaId) {
    let now = frame_system::Pallet::<T>::block_number();
    let queue_epoch = QueueEpoch::<T>::get();
    let mut next_queue: VecDeque<AaaId> = NextQueue::<T>::take().into_inner().into();
    let mut queue_insertions = next_queue.len() as u32;
    Self::enqueue_block_local(
      aaa_id,
      now,
      queue_epoch,
      &mut next_queue,
      &mut queue_insertions,
    );
    NextQueue::<T>::put(BoundedVec::<AaaId, T::MaxQueueLength>::truncate_from(
      next_queue.into_iter().collect::<Vec<AaaId>>(),
    ));
  }

  pub(crate) fn prime_actor_schedule(aaa_id: AaaId) {
    let Some(instance) = AaaInstances::<T>::get(aaa_id) else {
      return;
    };
    let now = frame_system::Pallet::<T>::block_number();
    match &instance.schedule.trigger {
      Trigger::Timer { .. } => Self::schedule_next_timer_wakeup(&instance, now),
      Trigger::Manual => {
        if instance.manual_trigger_pending {
          Self::enqueue(aaa_id);
        }
      }
      Trigger::OnAddressEvent { .. } => {
        if Self::evaluate_on_address_event(aaa_id) || instance.manual_trigger_pending {
          Self::enqueue(aaa_id);
        }
      }
    }
  }

  fn enqueue_block_local(
    aaa_id: AaaId,
    now: BlockNumberFor<T>,
    queue_epoch: u64,
    next_queue: &mut VecDeque<AaaId>,
    queue_insertions: &mut u32,
  ) {
    let marker = queue_epoch.saturating_add(1);
    if ActorQueueEpoch::<T>::get(aaa_id) == marker {
      return;
    }
    if *queue_insertions >= T::MaxQueueInsertionsPerBlock::get() {
      let next_block = now.saturating_add(One::one());
      Self::defer_wakeup(aaa_id, next_block);
      return;
    }
    if next_queue.len() >= T::MaxQueueLength::get() as usize {
      let next_block = now.saturating_add(One::one());
      Self::defer_wakeup(aaa_id, next_block);
      return;
    }
    next_queue.push_back(aaa_id);
    ActorQueueEpoch::<T>::insert(aaa_id, marker);
    *queue_insertions = queue_insertions.saturating_add(1);
  }

  fn merge_late_enqueues(
    now: BlockNumberFor<T>,
    queue_epoch: u64,
    next_queue: &mut VecDeque<AaaId>,
    queue_insertions: &mut u32,
  ) {
    let marker = queue_epoch.saturating_add(1);
    for aaa_id in NextQueue::<T>::take().into_inner() {
      if next_queue.contains(&aaa_id) {
        continue;
      }
      if *queue_insertions >= T::MaxQueueInsertionsPerBlock::get()
        || next_queue.len() >= T::MaxQueueLength::get() as usize
      {
        Self::defer_wakeup(aaa_id, now.saturating_add(One::one()));
        continue;
      }
      next_queue.push_back(aaa_id);
      ActorQueueEpoch::<T>::insert(aaa_id, marker);
      *queue_insertions = queue_insertions.saturating_add(1);
    }
  }

  fn carry_over_block_local(
    aaa_id: AaaId,
    now: BlockNumberFor<T>,
    _queue_epoch: u64,
    next_queue: &mut VecDeque<AaaId>,
  ) {
    if next_queue.len() >= T::MaxQueueLength::get() as usize {
      let next_block = now.saturating_add(One::one());
      Self::defer_wakeup(aaa_id, next_block);
      return;
    }
    // Already-admitted backlog carries forward without a fresh epoch-marker write.
    // A concurrent next-block enqueue may stage one duplicate, but queue seeding
    // deterministically removes it before execution and block-local dedup bounds it.
    next_queue.push_back(aaa_id);
  }

  fn enqueue_run_queue(
    aaa_id: AaaId,
    now: BlockNumberFor<T>,
    run_queue: &mut VecDeque<AaaId>,
    queued_set: &mut BTreeSet<AaaId>,
    queue_insertions: &mut u32,
  ) {
    if queued_set.contains(&aaa_id) {
      return;
    }
    if *queue_insertions >= T::MaxQueueInsertionsPerBlock::get() {
      let next_block = now.saturating_add(One::one());
      Self::defer_wakeup(aaa_id, next_block);
      return;
    }
    if run_queue.len() >= T::MaxQueueLength::get() as usize {
      let next_block = now.saturating_add(One::one());
      Self::defer_wakeup(aaa_id, next_block);
      return;
    }
    run_queue.push_back(aaa_id);
    queued_set.insert(aaa_id);
    *queue_insertions = queue_insertions.saturating_add(1);
  }

  fn defer_wakeup(aaa_id: AaaId, wakeup_block: BlockNumberFor<T>) -> bool {
    // Temporal wakeup layer: place future eligibility into bounded block buckets.
    let previous_block = ScheduledWakeupBlock::<T>::get(aaa_id);
    let mut target_block = wakeup_block;
    let mut scheduled_block: Option<BlockNumberFor<T>> = None;
    for _ in 0..=T::MaxSpilloverBlocks::get() {
      let inserted = WakeupIndex::<T>::mutate(target_block, |queue| {
        if queue.contains(&aaa_id) {
          return true;
        }
        queue.try_push(aaa_id).is_ok()
      });
      if inserted {
        scheduled_block = Some(target_block);
        break;
      }
      target_block = target_block.saturating_add(One::one());
    }
    let Some(scheduled_block) = scheduled_block else {
      WakeupRetryPending::<T>::insert(aaa_id, true);
      WakeupScheduleDrops::<T>::mutate(|drops| *drops = drops.saturating_add(1));
      Self::deposit_event(Event::WakeupScheduleDropped {
        aaa_id,
        requested_block: wakeup_block,
      });
      return false;
    };
    if previous_block != Some(scheduled_block) {
      if let Some(previous_block) = previous_block {
        Self::remove_wakeup_bucket_entry(previous_block, aaa_id);
      }
      ScheduledWakeupBlock::<T>::insert(aaa_id, scheduled_block);
    }
    if scheduled_block != wakeup_block {
      Self::deposit_event(Event::WakeupRescheduled {
        aaa_id,
        requested_block: wakeup_block,
        scheduled_block,
      });
    }
    WakeupRetryPending::<T>::remove(aaa_id);
    match MinWakeupBlock::<T>::get() {
      Some(current_min) if current_min <= scheduled_block => {}
      _ => MinWakeupBlock::<T>::put(scheduled_block),
    }
    true
  }

  pub(crate) fn remove_wakeup_bucket_entry(block: BlockNumberFor<T>, aaa_id: AaaId) {
    WakeupIndex::<T>::mutate_exists(block, |maybe_queue| {
      let Some(queue) = maybe_queue.as_mut() else {
        return;
      };
      queue.retain(|id| *id != aaa_id);
      if queue.is_empty() {
        *maybe_queue = None;
      }
    });
  }

  /// Baseline scheduler envelope reserved ahead of one actor run/close pair.
  ///
  /// Compatibility-ingress drains and heavyweight wakeup-retry or terminal
  /// sweep units remain independently metered durable carry-over work. They
  /// may defer actor execution in a saturated block and therefore do not form
  /// part of this same-block plan-admission guarantee.
  pub fn scheduler_admission_overhead() -> Weight {
    T::WeightInfo::scheduler_on_idle_base()
      .saturating_add(T::WeightInfo::compatibility_ingress_probe())
      .saturating_add(T::WeightInfo::scheduler_zombie_sweep_base())
      .saturating_add(
        T::WeightInfo::permissionless_sweep()
          .saturating_add(T::DbWeight::get().writes(1))
          .saturating_mul(u64::from(T::MaxSweepPerBlock::get())),
      )
      .saturating_add(Self::queue_storage_weight_upper())
      .saturating_add(
        Self::queue_item_weight_upper()
          .saturating_mul(u64::from(T::MaxQueueInsertionsPerBlock::get()).saturating_add(1)),
      )
      .saturating_add(Self::wakeup_cursor_weight())
      .saturating_add(Self::scheduler_probe_weight_upper())
  }

  pub fn queue_bootstrap_weight_upper(queue_len: u32) -> Weight {
    T::WeightInfo::scheduler_queue_bootstrap(queue_len)
  }

  fn queue_storage_weight_upper() -> Weight {
    Self::queue_bootstrap_weight_upper(T::MaxQueueLength::get())
  }

  fn queue_item_weight_upper() -> Weight {
    Weight::zero()
  }

  /// Upper-bounds terminal state deletion after the close plan has run.
  ///
  /// Queue removal decodes, scans, and rewrites both bounded queue stores. The
  /// generated wakeup-bucket scan proxy covers removal from one full future bucket.
  /// The fixed tail covers wakeup pointer/retry state, actor/readiness/cardinality
  /// state, owner/system reverse indexes, tombstone, checkpoint, and terminal event.
  pub(crate) fn close_cleanup_weight_upper() -> Weight {
    let queue_items = u64::from(T::MaxQueueLength::get()).saturating_mul(2);
    Self::queue_storage_weight_upper()
      .saturating_add(Self::queue_item_weight_upper().saturating_mul(queue_items))
      .saturating_add(Self::queue_bootstrap_weight_upper(
        T::MaxWakeupBucketSize::get(),
      ))
      .saturating_add(Weight::from_parts(10_000_000, 32_768))
      .saturating_add(T::DbWeight::get().reads_writes(9, 12))
  }

  pub fn wakeup_registration_weight_upper() -> Weight {
    Self::wakeup_retry_probe_weight_upper()
  }

  fn wakeup_retry_probe_weight_upper() -> Weight {
    T::WeightInfo::scheduler_wakeup_spillover_probe(T::MaxSpilloverBlocks::get().saturating_add(1))
  }

  pub fn scheduler_actor_probe_weight_upper() -> Weight {
    T::WeightInfo::scheduler_actor_probe()
  }

  fn scheduler_probe_weight_upper() -> Weight {
    Self::scheduler_actor_probe_weight_upper()
  }

  #[cfg(feature = "runtime-benchmarks")]
  pub(crate) fn benchmark_scheduler_actor_probe(aaa_id: AaaId) {
    let now = frame_system::Pallet::<T>::block_number();
    let queue_epoch = QueueEpoch::<T>::get();
    ActorQueueEpoch::<T>::remove(aaa_id);
    let instance = AaaInstances::<T>::get(aaa_id).expect("benchmark actor must exist");
    let meter = WeightMeter::with_limit(Weight::zero());
    let AdmissionDecision::Defer(reason) = Self::apply_admission(aaa_id, &instance, &meter) else {
      panic!("benchmark actor must defer on an exhausted cycle budget");
    };
    Self::deposit_event(Event::CycleDeferred { aaa_id, reason });
    let mut next_queue = VecDeque::new();
    let mut queue_insertions = T::MaxQueueInsertionsPerBlock::get();
    Self::enqueue_block_local(
      aaa_id,
      now,
      queue_epoch,
      &mut next_queue,
      &mut queue_insertions,
    );
  }

  pub fn wakeup_drain_weight_upper(wakeups: u32) -> Weight {
    T::WeightInfo::scheduler_wakeup_dense_due_drain(wakeups)
  }

  fn wakeup_cursor_weight() -> Weight {
    Self::wakeup_drain_weight_upper(0)
  }

  fn wakeup_drain_actor_weight_upper() -> Weight {
    Self::wakeup_drain_weight_upper(1).saturating_add(Self::wakeup_retry_probe_weight_upper())
  }

  fn drain_overdue_wakeups(
    now: BlockNumberFor<T>,
    run_queue: &mut VecDeque<AaaId>,
    queued_set: &mut BTreeSet<AaaId>,
    queue_insertions: &mut u32,
    meter: &mut WeightMeter,
  ) {
    let cursor_weight = Self::wakeup_cursor_weight();
    if !meter.can_consume(cursor_weight) {
      return;
    }
    meter.consume(cursor_weight);
    let mut processed = 0u32;
    let max_wakeups = T::MaxWakeupsPerBlock::get();
    let mut scanned_blocks = 0u32;
    let mut cursor = MinWakeupBlock::<T>::get();
    while processed < max_wakeups && scanned_blocks < max_wakeups {
      let Some(block_cursor) = cursor else {
        break;
      };
      if block_cursor > now {
        break;
      }
      let bucket_weight = T::WeightInfo::scheduler_wakeup_dense_due_drain(0);
      if !meter.can_consume(bucket_weight) {
        break;
      }
      meter.consume(bucket_weight);
      scanned_blocks = scanned_blocks.saturating_add(1);
      let actors = WakeupIndex::<T>::take(block_cursor).into_inner();
      if actors.is_empty() {
        cursor = Some(block_cursor.saturating_add(One::one()));
        continue;
      }
      let actor_weight = Self::wakeup_drain_actor_weight_upper();
      let mut index = 0usize;
      while index < actors.len() && processed < max_wakeups {
        if !meter.can_consume(actor_weight) {
          break;
        }
        meter.consume(actor_weight);
        let aaa_id = actors[index];
        processed = processed.saturating_add(1);
        index = index.saturating_add(1);
        if ScheduledWakeupBlock::<T>::get(aaa_id) != Some(block_cursor) {
          continue;
        }
        ScheduledWakeupBlock::<T>::remove(aaa_id);
        Self::enqueue_run_queue(aaa_id, now, run_queue, queued_set, queue_insertions);
      }
      if index < actors.len() {
        WakeupIndex::<T>::insert(
          block_cursor,
          BoundedVec::<AaaId, T::MaxWakeupBucketSize>::truncate_from(actors[index..].to_vec()),
        );
        cursor = Some(block_cursor);
        break;
      }
      cursor = Some(block_cursor.saturating_add(One::one()));
    }
    match cursor {
      Some(block_cursor) => MinWakeupBlock::<T>::put(block_cursor),
      None => MinWakeupBlock::<T>::kill(),
    }
  }

  fn timer_jitter_blocks(aaa_id: AaaId, every_blocks: u32) -> BlockNumberFor<T> {
    let window = every_blocks
      .saturating_div(4)
      .min(T::MaxTimerJitterBlocks::get());
    if window == 0 {
      return Zero::zero();
    }
    let hash = frame::hashing::blake2_256(&aaa_id.encode());
    let raw = u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]);
    (raw % window).into()
  }

  fn next_timer_wakeup_block(
    instance: &AaaInstanceOf<T>,
    now: BlockNumberFor<T>,
    every_blocks: u32,
  ) -> BlockNumberFor<T> {
    let cadence: BlockNumberFor<T> = every_blocks.into();
    let jitter = Self::timer_jitter_blocks(instance.aaa_id, every_blocks);
    let mut wakeup_block = now.saturating_add(cadence).saturating_add(jitter);
    if let Some(window) = instance.schedule_window {
      if wakeup_block < window.start {
        wakeup_block = window.start;
      }
    }
    wakeup_block
  }

  fn preserve_skipped_readiness(
    instance: &AaaInstanceOf<T>,
    now: BlockNumberFor<T>,
    periodic_continuations: &mut Vec<AaaId>,
  ) {
    if matches!(instance.schedule.trigger, Trigger::Timer { .. }) {
      Self::schedule_next_timer_wakeup_local(instance, now, periodic_continuations);
      return;
    }
    let pending = instance.manual_trigger_pending
      || matches!(instance.schedule.trigger, Trigger::OnAddressEvent { .. })
        && Self::evaluate_on_address_event(instance.aaa_id);
    if instance.is_paused || !pending {
      return;
    }
    let mut eligibility_block = now;
    if let Some(window) = instance.schedule_window {
      eligibility_block = eligibility_block.max(window.start);
    }
    if instance.cycle_nonce > 0 && instance.cycle_nonce < u64::MAX {
      let cooldown: BlockNumberFor<T> = instance.schedule.cooldown_blocks.into();
      eligibility_block = eligibility_block.max(instance.last_cycle_block.saturating_add(cooldown));
    }
    if eligibility_block > now {
      Self::defer_wakeup(instance.aaa_id, eligibility_block);
    } else {
      periodic_continuations.push(instance.aaa_id);
    }
  }

  fn schedule_next_timer_wakeup_local(
    instance: &AaaInstanceOf<T>,
    now: BlockNumberFor<T>,
    periodic_continuations: &mut Vec<AaaId>,
  ) {
    let Trigger::Timer { every_blocks, .. } = instance.schedule.trigger else {
      return;
    };
    if every_blocks <= 1 {
      periodic_continuations.push(instance.aaa_id);
      return;
    }
    let wakeup_block = Self::next_timer_wakeup_block(instance, now, every_blocks);
    Self::defer_wakeup(instance.aaa_id, wakeup_block);
  }

  fn schedule_next_timer_wakeup(instance: &AaaInstanceOf<T>, now: BlockNumberFor<T>) {
    let Trigger::Timer { every_blocks, .. } = instance.schedule.trigger else {
      return;
    };
    if every_blocks <= 1 {
      Self::enqueue(instance.aaa_id);
      return;
    }
    let wakeup_block = Self::next_timer_wakeup_block(instance, now, every_blocks);
    Self::defer_wakeup(instance.aaa_id, wakeup_block);
  }

  pub(crate) fn is_window_expired(instance: &AaaInstanceOf<T>) -> bool {
    let now = frame_system::Pallet::<T>::block_number();
    instance
      .schedule_window
      .map(|window| now > window.end)
      .unwrap_or(false)
  }

  pub(crate) fn user_native_balance(instance: &AaaInstanceOf<T>) -> T::Balance {
    let native = T::NativeAssetId::get();
    T::AssetOps::balance(&instance.sovereign_account, native)
  }

  pub(crate) fn liveness_close_reason(instance: &AaaInstanceOf<T>) -> Option<CloseReason> {
    if Self::is_window_expired(instance) {
      return Some(CloseReason::WindowExpired);
    }
    if instance.is_paused {
      return None;
    }
    Self::user_resource_close_reason(instance, false)
  }

  // Deterministic pre-cycle User precedence is BalanceExhausted, then
  // FeeBudgetExhausted. WindowExpired is handled by the caller first.
  fn user_resource_close_reason(
    instance: &AaaInstanceOf<T>,
    include_fee_budget: bool,
  ) -> Option<CloseReason> {
    if instance.aaa_type != AaaType::User {
      return None;
    }
    let native_balance = Self::user_native_balance(instance);
    if native_balance < T::MinUserBalance::get() {
      return Some(CloseReason::BalanceExhausted);
    }
    if include_fee_budget && native_balance < Self::cycle_fee_upper_bound(instance) {
      return Some(CloseReason::FeeBudgetExhausted);
    }
    None
  }

  fn close_within_budget(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    reason: CloseReason,
    meter: &WeightMeter,
  ) -> AdmissionDecision {
    let close_weight_upper = Self::close_cycle_weight_upper_bound(instance);
    if !meter.can_consume(close_weight_upper) {
      return AdmissionDecision::Defer(DeferReason::InsufficientWeightBudget);
    }
    match Self::close_actor(aaa_id, instance, reason) {
      Ok(()) => AdmissionDecision::Closed(close_weight_upper),
      Err(_) => AdmissionDecision::CloseFailed(close_weight_upper),
    }
  }

  fn pending_post_cycle_close_reason(instance: &AaaInstanceOf<T>) -> Option<CloseReason> {
    if Self::failure_limit_reached(instance.consecutive_failures) {
      return Some(CloseReason::ConsecutiveFailures);
    }
    instance
      .auto_close_at_cycle_nonce
      .filter(|target| instance.cycle_nonce >= *target)
      .map(|_| CloseReason::AutoCloseNonceReached)
  }

  fn cycle_may_close_on_failure(instance: &AaaInstanceOf<T>) -> bool {
    Self::failure_limit_reached(instance.consecutive_failures.saturating_add(1))
  }

  fn cycle_may_auto_close_on_success(instance: &AaaInstanceOf<T>) -> bool {
    instance
      .auto_close_at_cycle_nonce
      .map(|target| instance.cycle_nonce.saturating_add(1) >= target)
      .unwrap_or(false)
  }

  fn cycle_requires_close_tail_budget(instance: &AaaInstanceOf<T>) -> bool {
    Self::cycle_may_close_on_failure(instance) || Self::cycle_may_auto_close_on_success(instance)
  }

  fn cycle_admission_weight_upper(instance: &AaaInstanceOf<T>) -> Weight {
    let mut weight = Self::cycle_weight_upper_bound(instance);
    if Self::cycle_requires_close_tail_budget(instance) {
      weight = weight.saturating_add(Self::close_cycle_weight_upper_bound(instance));
    }
    weight
  }

  fn apply_admission(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    meter: &WeightMeter,
  ) -> AdmissionDecision {
    if GlobalCircuitBreaker::<T>::get() {
      return AdmissionDecision::Skip;
    }
    if Self::is_window_expired(instance) {
      return Self::close_within_budget(aaa_id, instance, CloseReason::WindowExpired, meter);
    }
    if instance.is_paused {
      return AdmissionDecision::Skip;
    }
    if instance.aaa_type == AaaType::User && instance.cycle_nonce == u64::MAX {
      return Self::close_within_budget(aaa_id, instance, CloseReason::CycleNonceExhausted, meter);
    }
    if let Some(reason) = Self::pending_post_cycle_close_reason(instance) {
      return Self::close_within_budget(aaa_id, instance, reason, meter);
    }
    if !Self::is_ready_for_execution(instance) {
      return AdmissionDecision::Skip;
    }
    if let Some(reason) = Self::user_resource_close_reason(instance, true) {
      return Self::close_within_budget(aaa_id, instance, reason, meter);
    }
    let cycle_weight_upper = Self::cycle_admission_weight_upper(instance);
    if !meter.can_consume(cycle_weight_upper) {
      return AdmissionDecision::Defer(DeferReason::InsufficientWeightBudget);
    }
    AdmissionDecision::Admit(cycle_weight_upper)
  }

  fn is_ready_for_execution(instance: &AaaInstanceOf<T>) -> bool {
    if instance.is_paused {
      return false;
    }
    if GlobalCircuitBreaker::<T>::get() {
      return false;
    }
    if let Some(window) = instance.schedule_window {
      let now = frame_system::Pallet::<T>::block_number();
      if now < window.start {
        return false;
      }
    }
    if instance.cycle_nonce > 0 && instance.cycle_nonce < u64::MAX {
      let now = frame_system::Pallet::<T>::block_number();
      let cooldown: BlockNumberFor<T> = instance.schedule.cooldown_blocks.into();
      if now.saturating_sub(instance.last_cycle_block) < cooldown {
        return false;
      }
    }
    Self::evaluate_trigger(instance)
  }

  fn evaluate_trigger(instance: &AaaInstanceOf<T>) -> bool {
    if instance.manual_trigger_pending {
      return true;
    }
    match instance.schedule.trigger {
      Trigger::Manual => false,
      Trigger::Timer { every_blocks } => Self::evaluate_timer(instance, every_blocks),
      Trigger::OnAddressEvent { .. } => Self::evaluate_on_address_event(instance.aaa_id),
    }
  }

  fn evaluate_timer(instance: &AaaInstanceOf<T>, every_blocks: u32) -> bool {
    let now = frame_system::Pallet::<T>::block_number();
    let cadence: BlockNumberFor<T> = every_blocks.into();
    now.saturating_sub(instance.last_cycle_block) >= cadence
  }

  fn source_matches_filter(
    filter: &SourceFilterOf<T>,
    owner: &T::AccountId,
    source: Option<&T::AccountId>,
  ) -> bool {
    match (filter, source) {
      (SourceFilter::Any, _) => true,
      (SourceFilter::OwnerOnly, Some(who)) => who == owner,
      (SourceFilter::OwnerOnly, None) => false,
      (SourceFilter::Whitelist(list), Some(who)) => list.iter().any(|a| a == who),
      (SourceFilter::Whitelist(_), None) => false,
    }
  }

  fn asset_matches_filter(filter: &AssetFilterOf<T>, asset: T::AssetId) -> bool {
    match filter {
      AssetFilter::Any => true,
      AssetFilter::Whitelist(list) => list.iter().any(|id| *id == asset),
    }
  }

  pub fn notify_address_event(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    source: &T::AccountId,
  ) -> DispatchResult {
    let provenance = FundingProvenance::Signed(source.clone());
    Self::notify_address_event_with_provenance(aaa_id, asset, amount, Some(&provenance))
  }

  pub fn notify_internal_address_event(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    source: &T::AccountId,
  ) -> DispatchResult {
    let provenance = FundingProvenance::InternalProtocol(source.clone());
    Self::notify_address_event_with_provenance(aaa_id, asset, amount, Some(&provenance))
  }

  pub fn notify_xcm_address_event(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    source: &T::AccountId,
  ) -> DispatchResult {
    let provenance = FundingProvenance::Xcm(source.clone());
    Self::notify_address_event_with_provenance(aaa_id, asset, amount, Some(&provenance))
  }

  pub fn notify_address_event_without_source(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
  ) -> DispatchResult {
    Self::notify_address_event_with_provenance(aaa_id, asset, amount, None)
  }

  pub fn queue_address_event(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    provenance: Option<FundingProvenance<T::AccountId>>,
  ) -> bool {
    if amount == Zero::zero() {
      return true;
    }
    let Some(instance) = AaaInstances::<T>::get(aaa_id) else {
      return true;
    };
    if Self::is_window_expired(&instance) {
      return true;
    }
    let capacity = T::MaxIngressOverflowQueue::get();
    if capacity == 0 {
      return false;
    }
    let len = IngressOverflowLen::<T>::get();
    if len >= capacity
      || Self::preflight_funding_event(aaa_id, asset, amount, provenance.as_ref()).is_err()
    {
      return false;
    }
    let head = IngressOverflowHead::<T>::get() % capacity;
    let tail = (head.saturating_add(len)) % capacity;
    let event = IngressOverflowEvent {
      aaa_id,
      asset,
      amount,
      provenance,
    };
    if Self::apply_address_event_parts(
      aaa_id,
      asset,
      amount,
      event.provenance.as_ref(),
      false,
      true,
    )
    .is_err()
    {
      return false;
    }
    IngressOverflowSlots::<T>::insert(tail, event);
    IngressOverflowLen::<T>::put(len.saturating_add(1));
    true
  }

  pub fn drain_address_event_overflow(max_events: u32) -> u32 {
    if max_events == 0 {
      return 0;
    }
    let capacity = T::MaxIngressOverflowQueue::get();
    if capacity == 0 {
      return 0;
    }
    let mut drained = 0u32;
    let mut head = IngressOverflowHead::<T>::get() % capacity;
    let mut len = IngressOverflowLen::<T>::get();
    while drained < max_events && len > 0 {
      let Some(event) = IngressOverflowSlots::<T>::take(head) else {
        head = (head.saturating_add(1)) % capacity;
        len = len.saturating_sub(1);
        continue;
      };
      let _ = Self::apply_address_event_parts(
        event.aaa_id,
        event.asset,
        event.amount,
        event.provenance.as_ref(),
        true,
        false,
      );
      head = (head.saturating_add(1)) % capacity;
      len = len.saturating_sub(1);
      drained = drained.saturating_add(1);
    }
    IngressOverflowHead::<T>::put(head);
    IngressOverflowLen::<T>::put(len);
    drained
  }

  fn funding_event_authorized(
    instance: &AaaInstanceOf<T>,
    provenance: Option<&FundingProvenance<T::AccountId>>,
  ) -> bool {
    provenance.is_some_and(|provenance| match &instance.funding_source_policy {
      FundingSourcePolicy::OwnerOnly => matches!(
        provenance,
        FundingProvenance::Signed(source) if source == &instance.owner
      ),
      FundingSourcePolicy::SignedAllowlist(allowed) => matches!(
        provenance,
        FundingProvenance::Signed(source) if allowed.contains(source)
      ),
      FundingSourcePolicy::RuntimePolicy => {
        T::FundingAuthority::allows(instance.aaa_id, &instance.owner, provenance)
      }
      FundingSourcePolicy::AnySource => true,
    })
  }

  pub fn preflight_funding_event(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    provenance: Option<&FundingProvenance<T::AccountId>>,
  ) -> DispatchResult {
    let Some(instance) = AaaInstances::<T>::get(aaa_id) else {
      return Ok(());
    };
    if Self::is_window_expired(&instance)
      || !Self::funding_event_authorized(&instance, provenance)
      || !instance.funding_tracked_assets.contains(&asset)
      || amount.is_zero()
    {
      return Ok(());
    }
    if let Some(batch) = instance.funding_snapshots.get(&asset) {
      ensure!(
        batch.pending_amount.checked_add(&amount).is_some(),
        Error::<T>::FundingBatchOverflow
      );
    }
    Ok(())
  }

  fn notify_address_event_with_provenance(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    provenance: Option<&FundingProvenance<T::AccountId>>,
  ) -> DispatchResult {
    Self::preflight_funding_event(aaa_id, asset, amount, provenance)?;
    polkadot_sdk::frame_support::storage::with_transaction(
      || match Self::apply_address_event_parts(aaa_id, asset, amount, provenance, true, true) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      },
    )
  }

  fn apply_address_event_parts(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    provenance: Option<&FundingProvenance<T::AccountId>>,
    apply_trigger: bool,
    apply_funding: bool,
  ) -> DispatchResult {
    let mut instance = match AaaInstances::<T>::get(aaa_id) {
      Some(inst) => inst,
      None => return Ok(()),
    };
    if Self::is_window_expired(&instance) {
      return Ok(());
    }
    let now = frame_system::Pallet::<T>::block_number();
    let mut instance_modified = false;
    let mut inbox_matched = false;
    if apply_trigger
      && let Trigger::OnAddressEvent {
        source_filter,
        asset_filter,
      } = &instance.schedule.trigger
    {
      if Self::source_matches_filter(
        source_filter,
        &instance.owner,
        provenance.map(FundingProvenance::account),
      ) && Self::asset_matches_filter(asset_filter, asset)
      {
        inbox_matched = true;
        AddressEventInbox::<T>::mutate(aaa_id, |maybe_entry| {
          let entry = maybe_entry.get_or_insert_with(|| InboxState {
            is_pending: false,
            generation: 0,
            last_event_block: now,
          });
          entry.is_pending = true;
          entry.generation = entry.generation.saturating_add(1);
          entry.last_event_block = now;
        });
      }
    }
    if apply_funding
      && Self::funding_event_authorized(&instance, provenance)
      && instance.funding_tracked_assets.contains(&asset)
      && amount > Zero::zero()
    {
      if let Some(batch) = instance.funding_snapshots.get_mut(&asset) {
        let pending_amount = batch
          .pending_amount
          .checked_add(&amount)
          .ok_or(Error::<T>::FundingBatchOverflow)?;
        batch.pending_amount = pending_amount;
        batch.pending_last_block = Some(now);
        instance_modified = true;
        Self::deposit_event(Event::FundingBatchPendingAccumulated {
          aaa_id,
          asset,
          added: amount,
          pending_amount,
        });
      } else {
        instance
          .funding_snapshots
          .try_insert(
            asset,
            FundingBatch {
              amount,
              block: now,
              pending_amount: Zero::zero(),
              pending_last_block: None,
            },
          )
          .map_err(|_| Error::<T>::FundingBatchOverflow)?;
        instance_modified = true;
        Self::deposit_event(Event::FundingBatchActivated {
          aaa_id,
          asset,
          amount,
        });
      }
    }
    if instance_modified {
      AaaInstances::<T>::insert(aaa_id, instance);
      Self::sync_readiness_state(aaa_id);
    }
    if inbox_matched {
      Self::enqueue(aaa_id);
    }
    Ok(())
  }

  fn evaluate_on_address_event(aaa_id: AaaId) -> bool {
    AddressEventInbox::<T>::get(aaa_id)
      .map(|entry| entry.is_pending)
      .unwrap_or(false)
  }

  pub(crate) fn consume_address_event(aaa_id: AaaId) {
    AddressEventInbox::<T>::remove(aaa_id);
  }

  pub(crate) fn execute_zombie_sweep(remaining_weight: Weight) -> Weight {
    let max_check = T::MaxSweepPerBlock::get();
    if remaining_weight.is_zero() {
      return Weight::zero();
    }
    let mut meter = WeightMeter::with_limit(remaining_weight);
    let base_weight = T::WeightInfo::scheduler_zombie_sweep_base();
    if !meter.can_consume(base_weight) {
      return Weight::zero();
    }
    meter.consume(base_weight);
    let max_id = NextAaaId::<T>::get();
    if max_id == 0 {
      return meter.consumed();
    }
    let mut cursor = SweepCursor::<T>::get();
    let iteration_weight =
      T::WeightInfo::permissionless_sweep().saturating_add(T::DbWeight::get().writes(1));
    let retry_weight =
      Self::scheduler_probe_weight_upper().saturating_add(Self::wakeup_retry_probe_weight_upper());
    let now = frame_system::Pallet::<T>::block_number();
    let mut retry_limit = 0u32;
    let mut admitted_retry_weight = Weight::zero();
    while retry_limit < max_check {
      let next_weight = admitted_retry_weight.saturating_add(retry_weight);
      if !meter.can_consume(next_weight) {
        break;
      }
      admitted_retry_weight = next_weight;
      retry_limit = retry_limit.saturating_add(1);
    }
    let retry_ids = WakeupRetryPending::<T>::iter_keys()
      .take(retry_limit as usize)
      .collect::<Vec<_>>();
    for aaa_id in &retry_ids {
      if AaaInstances::<T>::contains_key(aaa_id) {
        let _ = Self::defer_wakeup(*aaa_id, now.saturating_add(One::one()));
      } else {
        WakeupRetryPending::<T>::remove(aaa_id);
      }
      meter.consume(retry_weight);
    }
    let mut checked = retry_ids.len() as u32;
    while checked < max_check {
      if !meter.can_consume(iteration_weight) {
        break;
      }
      let next_cursor = (cursor + 1) % max_id;
      if let Some(instance) = AaaInstances::<T>::get(next_cursor) {
        if let Some(reason) = Self::liveness_close_reason(&instance) {
          let close_weight_upper = Self::close_cycle_weight_upper_bound(&instance);
          let required_weight = iteration_weight.saturating_add(close_weight_upper);
          if !meter.can_consume(required_weight) {
            break;
          }
          match Self::close_actor(next_cursor, &instance, reason) {
            Ok(()) => {
              cursor = next_cursor;
              SweepCursor::<T>::put(cursor);
              checked = checked.saturating_add(1);
              meter.consume(required_weight);
              continue;
            }
            Err(_) => {
              Self::deposit_event(Event::CycleDeferred {
                aaa_id: next_cursor,
                reason: DeferReason::CloseTransitionFailed,
              });
              meter.consume(required_weight);
              break;
            }
          }
        }
      }
      cursor = next_cursor;
      SweepCursor::<T>::put(cursor);
      checked = checked.saturating_add(1);
      meter.consume(iteration_weight);
    }
    meter.consumed()
  }

  pub(crate) fn evaluate_actor_liveness(aaa_id: AaaId) -> DispatchResult {
    let instance = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
    if let Some(reason) = Self::liveness_close_reason(&instance) {
      return Self::close_actor(aaa_id, &instance, reason);
    }
    Ok(())
  }
}
