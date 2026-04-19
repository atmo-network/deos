use super::pallet::*;
use super::types::Task as AaaTask;
use super::{AssetOps, EntropyProvider};
use alloc::{
  collections::{BTreeSet, VecDeque},
  vec::Vec,
};
use frame::prelude::*;
use polkadot_sdk::sp_runtime::traits::{Hash as HashT, One, Saturating, Zero};

enum AdmissionDecision {
  Admit(Weight),
  Closed(Weight),
  Defer(DeferReason),
  Skip,
}

impl<T: Config> Pallet<T> {
  pub(crate) fn execute_cycle(remaining_weight: Weight) -> Weight {
    let budget = remaining_weight.ref_time();
    if budget == 0 {
      return Weight::zero();
    }
    // Two-layer scheduler: overdue temporal wakeups are first admitted into the
    // active run queue, then execution proceeds from the merged run-queue state.
    let now = frame_system::Pallet::<T>::block_number();
    let queue_epoch = QueueEpoch::<T>::get();
    let mut consumed = 0u64;
    let mut total_consumed = Weight::zero();
    let mut run_queue: VecDeque<AaaId> = CurrentQueue::<T>::take().into_inner().into();
    let staged_next = NextQueue::<T>::take().into_inner();
    let mut queued_set: BTreeSet<AaaId> = BTreeSet::new();
    for aaa_id in run_queue.iter().copied() {
      queued_set.insert(aaa_id);
    }
    for aaa_id in staged_next.into_iter() {
      if queued_set.insert(aaa_id) {
        run_queue.push_back(aaa_id);
      }
    }
    for aaa_id in queued_set.iter().copied() {
      ActorQueueEpoch::<T>::remove(aaa_id);
    }
    let mut next_queue: VecDeque<AaaId> = VecDeque::new();
    let mut queue_insertions = 0u32;
    let mut periodic_continuations: Vec<AaaId> = Vec::new();
    let wakeup_weight =
      Self::drain_overdue_wakeups(now, &mut run_queue, &mut queued_set, &mut queue_insertions);
    consumed = consumed.saturating_add(wakeup_weight.ref_time());
    total_consumed = total_consumed.saturating_add(wakeup_weight);
    let max_executions = T::MaxExecutionsPerBlock::get();
    let mut executed = 0u32;
    while executed < max_executions && consumed < budget {
      let Some(aaa_id) = run_queue.pop_front() else {
        break;
      };
      queued_set.remove(&aaa_id);
      let Some(instance) = AaaInstances::<T>::get(aaa_id) else {
        continue;
      };
      let cycle_weight_upper = match Self::apply_admission(aaa_id, &instance, consumed, budget) {
        AdmissionDecision::Admit(weight) => weight,
        AdmissionDecision::Closed(weight) => {
          consumed = consumed.saturating_add(weight.ref_time());
          total_consumed = total_consumed.saturating_add(weight);
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
            Self::schedule_next_timer_wakeup_local(&updated, now, &mut periodic_continuations);
          }
          continue;
        }
      };
      let _actual = Self::execute_single_cycle(aaa_id);
      consumed = consumed.saturating_add(cycle_weight_upper.ref_time());
      total_consumed = total_consumed.saturating_add(cycle_weight_upper);
      executed = executed.saturating_add(1);
      if let Some(updated) = AaaInstances::<T>::get(aaa_id) {
        Self::schedule_next_timer_wakeup_local(&updated, now, &mut periodic_continuations);
      }
    }
    while let Some(aaa_id) = run_queue.pop_front() {
      queued_set.remove(&aaa_id);
      Self::carry_over_block_local(aaa_id, now, queue_epoch, &mut next_queue);
    }
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
    NextQueue::<T>::kill();
    QueueEpoch::<T>::put(queue_epoch.saturating_add(1));
    total_consumed
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

  fn carry_over_block_local(
    aaa_id: AaaId,
    now: BlockNumberFor<T>,
    queue_epoch: u64,
    next_queue: &mut VecDeque<AaaId>,
  ) {
    let marker = queue_epoch.saturating_add(1);
    if ActorQueueEpoch::<T>::get(aaa_id) == marker {
      return;
    }
    if next_queue.len() >= T::MaxQueueLength::get() as usize {
      let next_block = now.saturating_add(One::one());
      Self::defer_wakeup(aaa_id, next_block);
      return;
    }
    // Already-admitted backlog must carry over without consuming fresh enqueue budget,
    // otherwise saturated queues lose fairness simply by persisting their existing order
    next_queue.push_back(aaa_id);
    ActorQueueEpoch::<T>::insert(aaa_id, marker);
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

  fn defer_wakeup(aaa_id: AaaId, wakeup_block: BlockNumberFor<T>) {
    const MAX_SPILLOVER_BLOCKS: u32 = 8;
    // Temporal wakeup layer: place future eligibility into bounded block buckets.
    let previous_block = ScheduledWakeupBlock::<T>::get(aaa_id);
    let mut target_block = wakeup_block;
    let mut scheduled_block: Option<BlockNumberFor<T>> = None;
    for _ in 0..=MAX_SPILLOVER_BLOCKS {
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
      if previous_block.is_none() {
        WakeupScheduleDrops::<T>::mutate(|drops| *drops = drops.saturating_add(1));
        Self::deposit_event(Event::WakeupScheduleDropped {
          aaa_id,
          requested_block: wakeup_block,
        });
      }
      return;
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
    match MinWakeupBlock::<T>::get() {
      Some(current_min) if current_min <= scheduled_block => {}
      _ => MinWakeupBlock::<T>::put(scheduled_block),
    }
  }

  fn remove_wakeup_bucket_entry(block: BlockNumberFor<T>, aaa_id: AaaId) {
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

  fn drain_overdue_wakeups(
    now: BlockNumberFor<T>,
    run_queue: &mut VecDeque<AaaId>,
    queued_set: &mut BTreeSet<AaaId>,
    queue_insertions: &mut u32,
  ) -> Weight {
    let mut total = Weight::zero();
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
      scanned_blocks = scanned_blocks.saturating_add(1);
      let actors = WakeupIndex::<T>::take(block_cursor).into_inner();
      if actors.is_empty() {
        cursor = Some(block_cursor.saturating_add(One::one()));
        total = total.saturating_add(T::DbWeight::get().reads_writes(1, 1));
        continue;
      }
      let mut index = 0usize;
      while index < actors.len() && processed < max_wakeups {
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
        total = total.saturating_add(T::DbWeight::get().reads_writes(1, 1));
        break;
      }
      cursor = Some(block_cursor.saturating_add(One::one()));
      total = total.saturating_add(T::DbWeight::get().reads_writes(1, 1));
    }
    match cursor {
      Some(block_cursor) => MinWakeupBlock::<T>::put(block_cursor),
      None => MinWakeupBlock::<T>::kill(),
    }
    total
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

  fn can_fit_weight(consumed_ref_time: u64, budget_ref_time: u64, weight: Weight) -> bool {
    consumed_ref_time.saturating_add(weight.ref_time()) <= budget_ref_time
  }

  fn close_within_budget(
    aaa_id: AaaId,
    instance: &AaaInstanceOf<T>,
    reason: CloseReason,
    consumed_ref_time: u64,
    budget_ref_time: u64,
  ) -> AdmissionDecision {
    let close_weight_upper = Self::close_cycle_weight_upper_bound(instance);
    if !Self::can_fit_weight(consumed_ref_time, budget_ref_time, close_weight_upper) {
      return AdmissionDecision::Defer(DeferReason::InsufficientWeightBudget);
    }
    let _ = Self::close_actor(aaa_id, instance, reason);
    AdmissionDecision::Closed(close_weight_upper)
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
    consumed_ref_time: u64,
    budget_ref_time: u64,
  ) -> AdmissionDecision {
    if Self::is_window_expired(instance) {
      return Self::close_within_budget(
        aaa_id,
        instance,
        CloseReason::WindowExpired,
        consumed_ref_time,
        budget_ref_time,
      );
    }
    if instance.is_paused {
      return AdmissionDecision::Skip;
    }
    if instance.aaa_type == AaaType::User && instance.cycle_nonce == u64::MAX {
      return Self::close_within_budget(
        aaa_id,
        instance,
        CloseReason::CycleNonceExhausted,
        consumed_ref_time,
        budget_ref_time,
      );
    }
    if !Self::is_ready_for_execution(instance) {
      return AdmissionDecision::Skip;
    }
    if let Some(reason) = Self::user_resource_close_reason(instance, true) {
      return Self::close_within_budget(
        aaa_id,
        instance,
        reason,
        consumed_ref_time,
        budget_ref_time,
      );
    }
    let cycle_weight_upper = Self::cycle_admission_weight_upper(instance);
    if !Self::can_fit_weight(consumed_ref_time, budget_ref_time, cycle_weight_upper) {
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
      Trigger::Timer {
        every_blocks,
        probability,
      } => Self::evaluate_timer(instance, every_blocks, probability),
      Trigger::OnAddressEvent { .. } => Self::evaluate_on_address_event(instance.aaa_id),
    }
  }

  fn evaluate_timer(
    instance: &AaaInstanceOf<T>,
    every_blocks: u32,
    probability: Option<Perbill>,
  ) -> bool {
    let now = frame_system::Pallet::<T>::block_number();
    let cadence: BlockNumberFor<T> = every_blocks.into();
    if now.saturating_sub(instance.last_cycle_block) < cadence {
      return false;
    }
    let Some(probability) = probability else {
      return true;
    };
    if probability == Perbill::one() {
      return true;
    }
    if probability.is_zero() {
      return false;
    }
    let strict_financial_entropy = Self::requires_strict_probability_entropy(instance);
    let Some(entropy_hash) =
      Self::resolve_timer_entropy(now, instance.aaa_id, strict_financial_entropy)
    else {
      return false;
    };
    let seed = Self::mix_seed(entropy_hash.as_ref(), instance.aaa_id, instance.cycle_nonce);
    (seed % 1_000_000_000) < u64::from(probability.deconstruct())
  }

  fn resolve_timer_entropy(
    now: BlockNumberFor<T>,
    aaa_id: AaaId,
    strict_financial_entropy: bool,
  ) -> Option<T::Hash> {
    let subject = (aaa_id, now).encode();
    if strict_financial_entropy {
      return T::EntropyProvider::secure_entropy_for_financial_probability(&subject);
    }
    if let Some(external_entropy_hash) = T::EntropyProvider::entropy(&subject) {
      return Some(external_entropy_hash);
    }
    let parent_hash = frame_system::Pallet::<T>::parent_hash();
    if parent_hash != T::Hash::default() {
      return Some(parent_hash);
    }
    let previous_hash = frame_system::Pallet::<T>::block_hash(now.saturating_sub(One::one()));
    if previous_hash != T::Hash::default() {
      return Some(previous_hash);
    }
    Some(T::Hash::default())
  }

  fn requires_strict_probability_entropy(instance: &AaaInstanceOf<T>) -> bool {
    T::RequireSecureEntropyForProbabilisticTasks::get()
      && instance
        .execution_plan
        .iter()
        .any(|step| !matches!(step.task, AaaTask::Noop))
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

  pub fn register_address_event_seen(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    source: Option<&T::AccountId>,
  ) -> bool {
    let now = frame_system::Pallet::<T>::block_number();
    if IngressSeenBlock::<T>::get() != Some(now) {
      IngressSeenSet::<T>::kill();
      IngressSeenBlock::<T>::put(now);
    }
    let fingerprint =
      <T as frame_system::Config>::Hashing::hash_of(&(aaa_id, asset, amount, source.cloned()));
    IngressSeenSet::<T>::mutate(|set| {
      if set.contains(&fingerprint) {
        return false;
      }
      let _ = set.try_insert(fingerprint);
      true
    })
  }

  pub fn notify_address_event(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    source: &T::AccountId,
  ) {
    Self::notify_address_event_with_source(aaa_id, asset, amount, Some(source));
  }

  pub fn notify_address_event_without_source(aaa_id: AaaId, asset: T::AssetId, amount: T::Balance) {
    Self::notify_address_event_with_source(aaa_id, asset, amount, None);
  }

  pub fn queue_address_event(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    source: Option<T::AccountId>,
  ) -> bool {
    if amount == Zero::zero() {
      return true;
    }
    if !Self::register_address_event_seen(aaa_id, asset, amount, source.as_ref()) {
      return true;
    }
    let capacity = T::MaxIngressOverflowQueue::get();
    if capacity == 0 {
      return false;
    }
    let len = IngressOverflowLen::<T>::get();
    if len >= capacity {
      return false;
    }
    let head = IngressOverflowHead::<T>::get() % capacity;
    let tail = (head.saturating_add(len)) % capacity;
    let event = IngressOverflowEvent {
      aaa_id,
      asset,
      amount,
      source,
    };
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
      Self::notify_address_event_with_source(
        event.aaa_id,
        event.asset,
        event.amount,
        event.source.as_ref(),
      );
      head = (head.saturating_add(1)) % capacity;
      len = len.saturating_sub(1);
      drained = drained.saturating_add(1);
    }
    IngressOverflowHead::<T>::put(head);
    IngressOverflowLen::<T>::put(len);
    drained
  }

  fn notify_address_event_with_source(
    aaa_id: AaaId,
    asset: T::AssetId,
    amount: T::Balance,
    source: Option<&T::AccountId>,
  ) {
    if !Self::register_address_event_seen(aaa_id, asset, amount, source) {
      return;
    }
    let mut instance = match AaaInstances::<T>::get(aaa_id) {
      Some(inst) => inst,
      None => return,
    };
    let now = frame_system::Pallet::<T>::block_number();
    let mut instance_modified = false;
    let mut inbox_matched = false;
    if let Trigger::OnAddressEvent {
      source_filter,
      asset_filter,
    } = &instance.schedule.trigger
    {
      if Self::source_matches_filter(source_filter, &instance.owner, source)
        && Self::asset_matches_filter(asset_filter, asset)
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
    if instance.aaa_type == AaaType::System
      && instance.funding_tracked_assets.contains(&asset)
      && amount > Zero::zero()
    {
      let _ = instance
        .funding_snapshots
        .try_insert(asset, FundingSnapshot { amount, block: now });
      instance_modified = true;
    }
    if instance_modified {
      AaaInstances::<T>::insert(aaa_id, instance);
      Self::sync_readiness_state(aaa_id);
    }
    if inbox_matched {
      Self::enqueue(aaa_id);
    }
  }

  fn evaluate_on_address_event(aaa_id: AaaId) -> bool {
    AddressEventInbox::<T>::get(aaa_id)
      .map(|entry| entry.is_pending)
      .unwrap_or(false)
  }

  pub(crate) fn consume_address_event(aaa_id: AaaId) {
    AddressEventInbox::<T>::remove(aaa_id);
  }

  fn mix_seed(hash_bytes: &[u8], aaa_id: AaaId, cycle_nonce: u64) -> u64 {
    let mut acc: u64 = aaa_id.wrapping_mul(0x517cc1b727220a95);
    acc = acc.wrapping_add(cycle_nonce.wrapping_mul(0x9e3779b97f4a7c15));
    for (i, &byte) in hash_bytes.iter().take(8).enumerate() {
      acc ^= (byte as u64) << (i * 8);
    }
    acc ^= acc >> 33;
    acc = acc.wrapping_mul(0xff51afd7ed558ccd);
    acc ^= acc >> 33;
    acc
  }

  pub(crate) fn execute_zombie_sweep(remaining_weight: Weight) -> Weight {
    let max_check = T::MaxSweepPerBlock::get();
    let max_id = NextAaaId::<T>::get();
    if max_id == 0 || remaining_weight.ref_time() == 0 {
      return Weight::zero();
    }
    let mut cursor = SweepCursor::<T>::get();
    let mut checked = 0u32;
    let mut sweep_weight = Weight::zero();
    let iteration_weight = T::DbWeight::get().reads_writes(1, 1);
    while checked < max_check {
      if !Self::can_fit_weight(
        sweep_weight.ref_time(),
        remaining_weight.ref_time(),
        iteration_weight,
      ) {
        break;
      }
      let next_cursor = (cursor + 1) % max_id;
      if let Some(instance) = AaaInstances::<T>::get(next_cursor) {
        if let Some(reason) = Self::liveness_close_reason(&instance) {
          let close_weight_upper = Self::close_cycle_weight_upper_bound(&instance);
          let required_weight = iteration_weight.saturating_add(close_weight_upper);
          if !Self::can_fit_weight(
            sweep_weight.ref_time(),
            remaining_weight.ref_time(),
            required_weight,
          ) {
            break;
          }
          cursor = next_cursor;
          SweepCursor::<T>::put(cursor);
          let _ = Self::close_actor(next_cursor, &instance, reason);
          checked = checked.saturating_add(1);
          sweep_weight = sweep_weight.saturating_add(required_weight);
          continue;
        }
      }
      cursor = next_cursor;
      SweepCursor::<T>::put(cursor);
      checked = checked.saturating_add(1);
      sweep_weight = sweep_weight.saturating_add(iteration_weight);
    }
    sweep_weight
  }

  pub(crate) fn evaluate_actor_liveness(aaa_id: AaaId) -> DispatchResult {
    let instance = AaaInstances::<T>::get(aaa_id).ok_or(Error::<T>::AaaNotFound)?;
    if let Some(reason) = Self::liveness_close_reason(&instance) {
      return Self::close_actor(aaa_id, &instance, reason);
    }
    Ok(())
  }
}
