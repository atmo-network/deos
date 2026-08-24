use crate::{
  ActiveLifecycle, ActorActivationPlacement, ActorAdmissionCertificates, ActorClass,
  ActorClassification, ActorClassificationError, ActorContract, ActorEligibility,
  ActorExecutionPhase, ActorFunding, ActorHot, ActorId, ActorIdentities, ActorRunAuthority,
  ActorRunStateStore, ActorTriggerActivation, ActorType, AmountResolution, AssetFilter,
  AssetFilterOf, AttemptDisposition, CancellationReason, CloseReason, CrossingDirection,
  CrossingMemberPages, CrossingMemberships, CrossingPhase, CrossingTransition, CycleResult,
  CycleState, Error, Event, FeeChargeKind, FeeEnvelopeError, FeeEnvelopeInput, FundingSourcePolicy,
  GlobalCircuitBreaker, IdleStarvationPhase, IdleStarvationState, InitialLifecycle, InputLimit,
  LoadedActorStateOf, Mutability, NextActorId, ObservationCrossing, ObservationSubscriberPageList,
  ObservationTiming, OpeningSurface, OutcomeTotals, OwnerSlotBitmaps, Precondition, Predicate,
  QueueEntry, QueueHead, QueueOccupancy, QueuePages, QueueTail, RetryClass, ScheduleWindow,
  SimulationError, SimulationMode, SimulationStepRecord, SourceFilter, SourceFilterOf,
  SovereignIndex, SplitLeg, SplitTransferLegsOf, StepErrorPolicy, StepOf, StepOutcome,
  StepSkippedReason, SuspensionReason, SystemSovereignState, Task, TaskFailure, TaskOf,
  TimedPredicate, Trigger, TriggerFamily, TriggerRuntimeState, WakeupBucketState, WakeupBuckets,
  WakeupClock, WakeupEntry, WakeupKey, WakeupPage, WakeupPages, WakeupPointer, adapters::AssetOps,
  compose_attempt_fee_envelope, fee_native_protected_minimum, mock::*, settle_attempt_fee_step,
};
use alloc::collections::BTreeSet;

const RETRY_LATER: StepErrorPolicy = StepErrorPolicy::RetryLater { max_attempts: 10 };

fn manual_trigger_fee() -> Balance {
  Actors::trigger_fee_for_weight(
    ActorType::User,
    TriggerFamily::Manual,
    <TestWeightInfo as crate::WeightInfo>::manual_trigger(),
  )
  .trigger_fee
}

fn address_event_trigger_fee() -> Balance {
  Actors::trigger_fee_for_weight(
    ActorType::User,
    TriggerFamily::AddressEvent,
    <TestWeightInfo as crate::WeightInfo>::address_event_trigger_occurrence(),
  )
  .trigger_fee
}

fn observation_change_trigger_fee() -> Balance {
  Actors::trigger_fee_for_weight(
    ActorType::User,
    TriggerFamily::ObservationChange,
    <TestWeightInfo as crate::WeightInfo>::observation_change_trigger_occurrence(),
  )
  .trigger_fee
}

fn observation_crossing_trigger_fee() -> Balance {
  Actors::trigger_fee_for_weight(
    ActorType::User,
    TriggerFamily::ObservationCrossing,
    <TestWeightInfo as crate::WeightInfo>::observation_crossing_trigger_occurrence(),
  )
  .trigger_fee
}

fn at_time_trigger_fee() -> Balance {
  Actors::trigger_fee_for_weight(
    ActorType::User,
    TriggerFamily::AtTime,
    <TestWeightInfo as crate::WeightInfo>::at_time_trigger_occurrence(),
  )
  .trigger_fee
}

fn cadenced_trigger_fee() -> Balance {
  Actors::trigger_fee_for_weight(
    ActorType::User,
    TriggerFamily::Cadenced,
    <TestWeightInfo as crate::WeightInfo>::cadenced_trigger_occurrence(),
  )
  .trigger_fee
}

fn pipeline_opening_fee(plan: &crate::ContractSteps<crate::mock::Test>) -> Balance {
  Actors::user_pipeline_machine_capacity_requirement(plan)
    .expect("fixture Pipeline Machine requirement fits")
    .checked_sub(TestMinUserBalance::get())
    .expect("Pipeline requirement contains the ledger minimum")
}

fn update_contract_parts(actor_id: ActorId) -> crate::ActorContractOf<Test> {
  Actors::load_actor_contract(actor_id).expect("Actor Contract exists")
}

fn replace_auto_close(
  origin: RuntimeOrigin,
  actor_id: ActorId,
  target: Option<u64>,
) -> polkadot_sdk::sp_runtime::DispatchResult {
  let mut contract = update_contract_parts(actor_id);
  contract.auto_close_at_cycle_nonce = target;
  Actors::update_contract(origin, actor_id, contract)
}

macro_rules! update_contract_partial {
  ($origin:expr, $actor_id:expr, $funding:expr $(,)?) => {{
    let mut contract = crate::tests::update_contract_parts($actor_id);
    contract.funding = $funding;
    crate::mock::Actors::update_contract($origin, $actor_id, contract)
  }};
  ($origin:expr, $actor_id:expr, $first:expr, $second:expr $(,)?) => {{
    trait PartialContractUpdate {
      fn apply(self, contract: &mut crate::ActorContractOf<crate::mock::Test>);
    }
    impl PartialContractUpdate
      for (
        crate::tests::RuntimeSchedule,
        Option<crate::ScheduleWindow<u64>>,
      )
    {
      fn apply(self, contract: &mut crate::ActorContractOf<crate::mock::Test>) {
        contract.trigger = self.0.trigger;
        contract.cooldown_blocks = self.0.cooldown_blocks;
        contract.window = self.1;
      }
    }
    impl PartialContractUpdate
      for (
        crate::ContractSteps<crate::mock::Test>,
        crate::CompletionPolicy,
      )
    {
      fn apply(self, contract: &mut crate::ActorContractOf<crate::mock::Test>) {
        contract.steps = self.0;
        contract.completion = self.1;
      }
    }
    let mut contract = crate::tests::update_contract_parts($actor_id);
    ($first, $second).apply(&mut contract);
    crate::mock::Actors::update_contract($origin, $actor_id, contract)
  }};
}
use codec::{Decode, Encode, MaxEncodedLen};
use polkadot_sdk::frame_support::{
  __private::metadata_ir::{
    StorageEntryMetadataIR, StorageEntryModifierIR, StorageEntryTypeIR, StorageHasherIR,
  },
  BoundedBTreeMap, BoundedBTreeSet, BoundedVec, assert_noop, assert_ok,
  traits::{Currency, Get, Hooks, LockableCurrency, StorageInfoTrait, WithdrawReasons},
};
use polkadot_sdk::sp_runtime::StateVersion;
use polkadot_sdk::{
  frame_system,
  sp_runtime::{DispatchError, Perbill, Weight},
  sp_weights::{WeightMeter, WeightToFee},
};
use scale_info::{TypeDef, TypeInfo};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Schedule {
  trigger: crate::TriggerOf<Test>,
  cooldown_blocks: u32,
}
type RuntimeSchedule = Schedule;
type RuntimeSourceFilter = SourceFilterOf<Test>;
type RuntimeAssetFilter = AssetFilterOf<Test>;
type RuntimeTrigger = crate::TriggerOf<Test>;
type RuntimeTask = TaskOf<Test>;
type RuntimeStep = StepOf<Test>;
type RuntimeActorContract = crate::ActorContractOf<Test>;
type RuntimeActorRunState = crate::ActorRunStateOf<Test>;
type MockBlockNumber = polkadot_sdk::frame_system::pallet_prelude::BlockNumberFor<Test>;
type TestWeightInfo = crate::weights::TestWeightInfo;

fn run_contract_authority(actor_id: ActorId) -> ActorRunAuthority<[u8; 32]> {
  let admission =
    ActorAdmissionCertificates::<Test>::get(actor_id).expect("Actor admission certificate exists");
  ActorRunAuthority {
    semantic_contract_id: admission.semantic_contract_id,
    body_commitment: admission.body_commitment,
    admission_identity: admission.admission_identity,
  }
}

fn scheduled_wakeup_block(actor_id: crate::ActorId) -> Option<MockBlockNumber> {
  Actors::actor_hot(actor_id).and_then(|hot| {
    hot
      .wakeup_pointer
      .map(|pointer| match pointer.block {
        WakeupKey::Block(block) => block,
        WakeupKey::Tick(tick) => tick,
      })
      .or_else(|| hot.trigger_wakeup_pointer.map(|pointer| pointer.tick))
  })
}

fn queue_entry(ticket: u64, actor_id: ActorId) -> QueueEntry<MockBlockNumber> {
  QueueEntry {
    actor_id,
    cycle_nonce: 0,
    cursor: 0,
    ticket,
    eligible_at: 0,
    contract_commitment: crate::ActorContractCommitment {
      semantic_contract_id: [0; 32],
      body_commitment: [0; 32],
    },
  }
}

fn seed_saturated_tombstone_queue() {
  let page_size: u32 = <Test as crate::Config>::QueuePageSize::get();
  let capacity: u32 = <Test as crate::Config>::MaxQueueLength::get();
  for page_id in 0..capacity.div_ceil(page_size) {
    let first = page_id * page_size;
    let len = page_size.min(capacity - first);
    let entries = (0..len)
      .map(|offset| {
        queue_entry(
          u64::from(first + offset),
          10_000_000 + u64::from(first + offset),
        )
      })
      .collect::<Vec<_>>();
    QueuePages::<Test>::insert(
      u64::from(page_id),
      BoundedVec::try_from(entries).expect("saturated queue page fits"),
    );
  }
  QueueHead::<Test>::put(0);
  QueueTail::<Test>::put(u64::from(capacity));
  QueueOccupancy::<Test>::put(capacity);
  crate::NextQueueTicket::<Test>::put(u64::from(capacity));
}

fn assert_plain_storage_type<T: TypeInfo + 'static>(entry: &StorageEntryMetadataIR) {
  let StorageEntryTypeIR::Plain(actual) = entry.ty else {
    panic!("{} must remain plain storage", entry.name);
  };
  assert_eq!(
    actual,
    scale_info::meta_type::<T>(),
    "{} value type",
    entry.name
  );
}

fn assert_map_storage_types<K: TypeInfo + 'static, V: TypeInfo + 'static>(
  entry: &StorageEntryMetadataIR,
) {
  let StorageEntryTypeIR::Map { key, value, .. } = entry.ty else {
    panic!("{} must remain map storage", entry.name);
  };
  assert_eq!(key, scale_info::meta_type::<K>(), "{} key type", entry.name);
  assert_eq!(
    value,
    scale_info::meta_type::<V>(),
    "{} value type",
    entry.name
  );
}

/// Names-and-order contract for SCALE variant types. Numeric indices are metadata-derived and
/// owned by the generated ABI manifest plus PAPI descriptors; this guard only prevents silent
/// variant renames or reorderings that would break the semantic surface.
fn variant_count<T: TypeInfo>() -> usize {
  let info = T::type_info();
  let TypeDef::Variant(definition) = info.type_def else {
    panic!("contract type must be a SCALE variant");
  };
  definition.variants.len()
}

fn assert_variant_names<T: TypeInfo>(expected: &[&str]) {
  let info = T::type_info();
  let TypeDef::Variant(definition) = info.type_def else {
    panic!("contract type must be a SCALE variant");
  };
  let actual: alloc::vec::Vec<_> = definition
    .variants
    .iter()
    .map(|variant| variant.name)
    .collect();
  assert_eq!(actual, expected);
}

fn prepare_crossing_pair_after_sparse_open() {
  frame_system::Pallet::<Test>::set_block_number(1);
  set_observation(
    7,
    crate::ScalarObservationState::Fresh {
      value: 50,
      observed_at: 1,
    },
  );
  for owner in [ALICE, BOB, CHARLIE] {
    create_system_with(
      owner,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
  }
  assert_ok!(Actors::note_observation_transition(
    7,
    crate::ObservationTransition {
      revision: 2,
      previous: Some(50),
      current: 150,
    },
  ));
  assert_ok!(Actors::crossing_work_unit());
  assert_eq!(
    Actors::classify_crossing_work(),
    crate::CrossingWorkPlan::FireCohortPlacedBatch
  );
}

fn crossing_phase(actor_id: ActorId) -> CrossingPhase {
  match ActorHot::<Test>::get(actor_id)
    .expect("active Crossing actor")
    .trigger_runtime_state
  {
    TriggerRuntimeState::ObservationCrossing { phase, .. } => phase,
    TriggerRuntimeState::Stateless
    | TriggerRuntimeState::AtTime { .. }
    | TriggerRuntimeState::Cadenced { .. } => {
      panic!("actor does not own Crossing runtime state")
    }
  }
}

fn drain_crossing_work() -> u32 {
  drain_crossing_work_with_limit(128)
}

fn drain_crossing_work_with_limit(limit: u32) -> u32 {
  for unit in 1..=limit {
    if !Actors::crossing_work_unit().expect("Crossing worker remains valid") {
      return unit;
    }
  }
  panic!("Crossing work did not converge within the bounded fixture");
}

fn assert_on_idle_wakeup_insufficiency_preserves_state(wakeup_budget: Weight) {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(10);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    GlobalCircuitBreaker::<Test>::put(true);
    let hot_before = Actors::actor_hot(actor_id).expect("actor before bounded wakeup pass");
    let bucket_before = Actors::wakeup_buckets(10).expect("due wakeup bucket");
    let page_before = Actors::wakeup_pages((10, 0)).expect("due wakeup page");
    let cursor_before = Actors::wakeup_cursor_peek();
    let events_before = System::events();
    let remaining =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_on_idle_base()
        .saturating_add(wakeup_budget);

    let used = Actors::on_idle(10, remaining);

    assert!(
      used.all_lte(remaining),
      "on_idle must not exceed its caller budget"
    );
    assert_eq!(Actors::actor_hot(actor_id), Some(hot_before));
    assert_eq!(Actors::wakeup_buckets(10), Some(bucket_before));
    assert_eq!(Actors::wakeup_pages((10, 0)), Some(page_before));
    assert_eq!(Actors::wakeup_cursor_peek(), cursor_before);
    assert!(Actors::wakeup_worker_fault().is_none());
    assert_eq!(System::events(), events_before);
  });
}

fn ordinary_transfer_to_actor(
  origin: RuntimeOrigin,
  actor_id: u64,
  asset: TestAsset,
  amount: u128,
) -> polkadot_sdk::sp_runtime::DispatchResult {
  let source = frame_system::ensure_signed(origin)?;
  let instance = Actors::active_actor_view(actor_id).ok_or(Error::<Test>::ActorNotFound)?;
  Actors::preflight_funding_event(
    actor_id,
    asset,
    amount,
    Some(&source),
    Some(&crate::FundingProvenance::Signed),
  )?;
  MockAssetOps::transfer(&source, &instance.sovereign_account, asset, amount)
    .map_err(|failure| failure.error)?;
  Actors::notify_address_event(actor_id, asset, amount, &source)?;
  Ok(())
}

fn manual_schedule() -> RuntimeSchedule {
  Schedule {
    trigger: Trigger::manual(),
    cooldown_blocks: 0,
  }
}

fn on_address_event_schedule(
  source_filter: RuntimeSourceFilter,
  asset_filter: RuntimeAssetFilter,
) -> RuntimeSchedule {
  Schedule {
    trigger: Trigger::address_event(source_filter, asset_filter),
    cooldown_blocks: 0,
  }
}

fn percentage_trigger_schedule() -> RuntimeSchedule {
  on_address_event_schedule(SourceFilter::Any, AssetFilter::Any)
}

fn signal_percentage_trigger(actor_id: ActorId, asset: TestAsset) {
  assert_ok!(Actors::notify_address_event(actor_id, asset, 1, &ALICE));
}

fn observation_schedule(feeds: Vec<u32>) -> RuntimeSchedule {
  let [feed]: [u32; 1] = feeds
    .try_into()
    .expect("one observation trigger feed is required");
  Schedule {
    trigger: RuntimeTrigger::observation_change(feed),
    cooldown_blocks: 0,
  }
}

fn at_time_schedule(after_ticks: u32) -> RuntimeSchedule {
  Schedule {
    trigger: Trigger::at_time(u64::from(after_ticks)),
    cooldown_blocks: 0,
  }
}

fn timer_schedule(every_ticks: u32) -> RuntimeSchedule {
  Schedule {
    trigger: Trigger::cadenced(u64::from(every_ticks)),
    cooldown_blocks: 0,
  }
}

fn timed_all_conditions(
  timing: ObservationTiming,
  predicates: Vec<Predicate<TestAsset, Balance, u32, u32>>,
) -> Option<crate::PreconditionOf<Test>> {
  let clause = BoundedVec::try_from(
    predicates
      .into_iter()
      .map(|predicate| TimedPredicate { timing, predicate })
      .collect::<Vec<_>>(),
  )
  .expect("predicates fit");
  Some(Precondition {
    clauses: BoundedVec::try_from(vec![clause]).expect("clause fits"),
  })
}

fn all_conditions(
  predicates: Vec<Predicate<TestAsset, Balance, u32, u32>>,
) -> Option<crate::PreconditionOf<Test>> {
  timed_all_conditions(ObservationTiming::Current, predicates)
}

fn any_conditions(
  predicates: Vec<Predicate<TestAsset, Balance, u32, u32>>,
) -> Option<crate::PreconditionOf<Test>> {
  let clauses = predicates
    .into_iter()
    .map(|predicate| {
      BoundedVec::try_from(vec![TimedPredicate {
        timing: ObservationTiming::Current,
        predicate,
      }])
      .expect("predicate fits")
    })
    .collect::<Vec<_>>();
  Some(Precondition {
    clauses: BoundedVec::try_from(clauses).expect("clauses fit"),
  })
}

fn make_step(task: RuntimeTask) -> RuntimeStep {
  StepOf::<Test> {
    precondition: None,
    task,
    on_error: StepErrorPolicy::AbortCycle,
  }
}

fn inert_contract_steps() -> crate::ContractSteps<crate::mock::Test> {
  contract_steps_with_step(StepOf::<Test> {
    precondition: all_conditions(vec![Predicate::BlockNumberBelow { threshold: 0 }]),
    task: Task::StopCycle,
    on_error: StepErrorPolicy::AbortCycle,
  })
}

fn contract_steps_with_step(step: RuntimeStep) -> crate::ContractSteps<crate::mock::Test> {
  BoundedVec::try_from(vec![step]).expect("contract_steps must fit")
}

fn transfer_contract_steps(
  to: AccountId,
  amount: Balance,
) -> crate::ContractSteps<crate::mock::Test> {
  contract_steps_with_step(make_step(Task::Transfer {
    to,
    asset: TestAsset::Native,
    amount: AmountResolution::Fixed(amount),
  }))
}

fn user_active_contract(
  schedule: RuntimeSchedule,
  window: Option<crate::ScheduleWindow<u64>>,
  steps: crate::ContractSteps<crate::mock::Test>,
) -> Option<crate::ActorContractOf<Test>> {
  Some(ActorContract {
    trigger: schedule.trigger,
    cooldown_blocks: schedule.cooldown_blocks,
    window,
    steps,
    completion: crate::CompletionPolicy::Persistent,
    funding: FundingSourcePolicy::OwnerOnly,
    auto_close_at_cycle_nonce: None,
  })
}

fn create_user_with(
  owner: AccountId,
  mutability: Mutability,
  schedule: RuntimeSchedule,
  schedule_window: Option<crate::ScheduleWindow<u64>>,
  contract_steps: crate::ContractSteps<crate::mock::Test>,
) -> u64 {
  prefund_active_user_creation(owner, &contract_steps);
  let id = Actors::next_actor_id();
  assert_ok!(Actors::create_user_actor(
    RuntimeOrigin::signed(owner),
    mutability,
    user_active_contract(schedule, schedule_window, contract_steps),
  ));
  age_fixture_control_clock(id);
  id
}

fn create_user_with_slot(
  owner: AccountId,
  owner_slot: u8,
  mutability: Mutability,
  schedule: RuntimeSchedule,
  schedule_window: Option<crate::ScheduleWindow<u64>>,
  contract_steps: crate::ContractSteps<crate::mock::Test>,
) -> u64 {
  prefund_user_sovereign(owner, owner_slot, &contract_steps);
  let id = Actors::next_actor_id();
  assert_ok!(Actors::create_user_actor_at_slot(
    RuntimeOrigin::signed(owner),
    owner_slot,
    mutability,
    user_active_contract(schedule, schedule_window, contract_steps),
  ));
  age_fixture_control_clock(id);
  id
}

fn create_system_with(
  owner: AccountId,
  schedule: RuntimeSchedule,
  schedule_window: Option<crate::ScheduleWindow<u64>>,
  contract_steps: crate::ContractSteps<crate::mock::Test>,
) -> u64 {
  let id = Actors::next_actor_id();
  assert_ok!(Actors::create_system_actor(
    RuntimeOrigin::root(),
    owner,
    Mutability::Mutable,
    system_active_contract(schedule, schedule_window, contract_steps),
  ));
  age_fixture_control_clock(id);
  id
}

fn create_system_with_mutability(
  owner: AccountId,
  mutability: Mutability,
  schedule: RuntimeSchedule,
  schedule_window: Option<crate::ScheduleWindow<u64>>,
  contract_steps: crate::ContractSteps<crate::mock::Test>,
) -> u64 {
  let id = Actors::next_actor_id();
  assert_ok!(Actors::create_system_actor(
    RuntimeOrigin::root(),
    owner,
    mutability,
    system_active_contract(schedule, schedule_window, contract_steps),
  ));
  age_fixture_control_clock(id);
  id
}

fn age_fixture_control_clock(actor_id: ActorId) {
  let now = frame_system::Pallet::<Test>::block_number();
  if now == 0 {
    frame_system::Pallet::<Test>::set_block_number(1);
    return;
  }
  ActorIdentities::<Test>::mutate(actor_id, |maybe| {
    maybe
      .as_mut()
      .expect("fixture actor identity exists")
      .last_control_mutation_block = now.saturating_sub(1);
  });
}

fn system_active_contract(
  schedule: RuntimeSchedule,
  window: Option<crate::ScheduleWindow<u64>>,
  steps: crate::ContractSteps<crate::mock::Test>,
) -> Option<crate::ActorContractOf<Test>> {
  system_active_contract_with_completion(
    schedule,
    window,
    steps,
    crate::CompletionPolicy::Persistent,
  )
}

fn system_active_contract_with_completion(
  schedule: RuntimeSchedule,
  window: Option<crate::ScheduleWindow<u64>>,
  steps: crate::ContractSteps<crate::mock::Test>,
  completion: crate::CompletionPolicy,
) -> Option<crate::ActorContractOf<Test>> {
  Some(ActorContract {
    trigger: schedule.trigger,
    cooldown_blocks: schedule.cooldown_blocks,
    window,
    steps,
    completion,
    funding: FundingSourcePolicy::RuntimePolicy,
    auto_close_at_cycle_nonce: None,
  })
}

fn actor_funding(actor_id: u64) -> crate::ActorFundingStateOf<Test> {
  Actors::actor_funding(actor_id).expect("active actor funding exists")
}

fn sovereign_account(actor_id: u64) -> AccountId {
  Actors::active_actor_view(actor_id)
    .map(|inst| inst.sovereign_account)
    .expect("Actors must exist")
}

fn fund_native(actor_id: u64, amount: Balance) {
  let actor_acc = sovereign_account(actor_id);
  let _ = <Balances as frame::traits::Currency<AccountId>>::deposit_creating(&actor_acc, amount);
}

fn native_balance(who: &AccountId) -> Balance {
  <Balances as Currency<AccountId>>::free_balance(who)
}

fn actor_state_hold_total(actor_id: ActorId) -> Balance {
  let hold = Actors::actor_state_hold(actor_id).expect("User Actor state hold exists");
  hold
    .breakdown
    .identity
    .saturating_add(hold.breakdown.contract_head)
    .saturating_add(hold.breakdown.contract_body)
    .saturating_add(hold.breakdown.detector)
    .saturating_add(hold.breakdown.funding)
    .saturating_add(hold.breakdown.run)
}

fn asset_balance(who: &AccountId, asset: TestAsset) -> Balance {
  MockAssetOps::balance(who, asset)
}

fn set_asset_balance(who: &AccountId, asset: TestAsset, amount: Balance) {
  MockAssetOps::mint(who, asset, amount).expect("mint must succeed");
}

fn set_native_transfer_lock(who: &AccountId, amount: Balance) {
  <Balances as LockableCurrency<AccountId>>::set_lock(
    *b"actlock0",
    who,
    amount,
    WithdrawReasons::TRANSFER,
  );
}

fn setup_pool(asset_a: TestAsset, asset_b: TestAsset, reserve_a: Balance, reserve_b: Balance) {
  crate::mock::set_pool_reserves(asset_a, asset_b, reserve_a, reserve_b);
}

fn temporary_retry_swap_plan() -> crate::ContractSteps<crate::mock::Test> {
  BoundedVec::try_from(vec![StepOf::<Test> {
    precondition: None,
    task: Task::SwapIn {
      asset_in: TestAsset::Native,
      asset_out: TestAsset::Local(77),
      amount_in: AmountResolution::Fixed(10),
      slippage_tolerance: Perbill::one(),
    },
    on_error: RETRY_LATER,
  }])
  .expect("single retry step fits")
}

fn setup_temporary_retry_pool() {
  setup_pool(TestAsset::Native, TestAsset::Local(77), 10_000, 10_000);
  set_asset_balance(&u64::MAX, TestAsset::Local(77), 10_000);
}

fn create_suspended_system_retry(block: u64) -> u64 {
  frame_system::Pallet::<Test>::set_block_number(block);
  setup_temporary_retry_pool();
  let actor_id = create_system_with(ALICE, manual_schedule(), None, temporary_retry_swap_plan());
  fund_native(actor_id, 100);
  set_temporary_dex_failure(true);
  assert_ok!(Actors::manual_trigger(
    RuntimeOrigin::signed(ALICE),
    actor_id
  ));
  run_idle(Weight::MAX);
  assert!(Actors::actor_run_state(actor_id).is_some());
  actor_id
}

fn user_step_fee(step: &StepOf<Test>) -> Balance {
  let plan = BoundedVec::try_from(vec![step.clone()]).expect("one step fits the shared bound");
  Actors::maximum_contract_step_fee(ActorType::User, &plan, 0)
    .expect("one Step has a checked fee")
    .total_fee
}

fn fund_native_raw(who: &AccountId, amount: Balance) {
  let _ = <Balances as frame::traits::Currency<AccountId>>::deposit_creating(who, amount);
}

/// Fixture funding: ledger minimum plus one Manual Trigger occurrence and the complete
/// Pipeline Machine/cleanup maximum. Creation itself still requires no service prefunding.
fn user_prefunding_requirement(plan: &crate::ContractSteps<crate::mock::Test>) -> Balance {
  let pipeline = Actors::user_pipeline_machine_capacity_requirement(plan)
    .expect("fixture plan has a checked Pipeline Machine requirement");
  let trigger = manual_trigger_fee();
  pipeline
    .checked_add(trigger)
    .expect("fixture Trigger and Pipeline funding fits")
}

/// Pre-funds the deterministic User sovereign account for later activation/execution fixtures
/// without mutating Actors state.
fn prefund_user_sovereign(
  owner: AccountId,
  slot: u8,
  plan: &crate::ContractSteps<crate::mock::Test>,
) {
  fund_native_raw(
    &Actors::sovereign_account_id(&owner, slot),
    user_prefunding_requirement(plan),
  );
}

/// Pre-funds the next automatically allocated User slot for a direct Active creation fixture.
fn prefund_active_user_creation(owner: AccountId, plan: &crate::ContractSteps<crate::mock::Test>) {
  let slot = Actors::available_owner_slot(&owner, None).expect("fixture owner has a free slot");
  prefund_user_sovereign(owner, slot, plan);
}

/// Depletes the sovereign fee-native balance after creation, restoring the historical
/// unfunded post-creation fixture state while keeping creation itself admitted.
fn deplete_user_sovereign(actor_id: u64, amount: Balance) {
  let acc = sovereign_account(actor_id);
  MockAssetOps::burn(&acc, TestAsset::Native, amount).expect("fixture depletion burn succeeds");
}

fn run_actor_hook_order_with_external(
  now: u64,
  external: impl FnOnce(),
  idle_weight: Weight,
) -> Weight {
  frame_system::Pallet::<Test>::set_block_number(now);
  let initialize_weight = Actors::on_initialize(now);
  let prepass_weight = Actors::actor_prepass(RuntimeOrigin::none())
    .expect("fixture prepass succeeds")
    .actual_weight
    .unwrap_or_else(Weight::zero);
  external();
  initialize_weight
    .saturating_add(prepass_weight)
    .saturating_add(Actors::on_idle(now, idle_weight))
}

fn run_next_idle(weight: Weight) {
  let now = frame_system::Pallet::<Test>::block_number()
    .checked_add(1)
    .expect("test block number advances");
  frame_system::Pallet::<Test>::set_block_number(now);
  Actors::on_initialize(now);
  run_prepass();
  run_idle(weight);
}

fn run_prepass() {
  let now = frame_system::Pallet::<Test>::block_number();
  if Actors::block_resource_state().is_some_and(|state| state.ensure_block(now).is_err()) {
    crate::CurrentBlockResourceState::<Test>::kill();
  }
  assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
}

fn run_idle(weight: Weight) {
  let mut now = frame_system::Pallet::<Test>::block_number();
  Actors::on_idle(now, weight);
  let max_blocks =
    <<Test as crate::Config>::MaxContractSteps as Get<u32>>::get().saturating_mul(2u32);
  for _ in 1..max_blocks {
    if !ActorHot::<Test>::iter_values().any(|hot| hot.cycle_state == CycleState::Running) {
      break;
    }
    let Some(next) = now.checked_add(1) else {
      break;
    };
    now = next;
    frame_system::Pallet::<Test>::set_block_number(now);
    Actors::on_initialize(now);
    run_prepass();
    Actors::on_idle(now, weight);
  }
}

fn starvation_observation_weight() -> Weight {
  <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_on_idle_base()
}

/// Proof-limited on_idle budget that admits the wakeup cursor, queue scan, hot/contract probes, and
/// the head consume, but not the actor's full cycle admission. This materializes a live FIFO head
/// blocked by weight with no admitted attempt, the only spec 8.6.3 starvation trigger.
fn starvation_blocked_budget(actor_id: u64) -> Weight {
  let base = starvation_observation_weight();
  let cursor = <TestWeightInfo as crate::WeightInfo>::scheduler_wakeup_cursor_worker_future();
  let scan = <TestWeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1);
  let state_probe = Actors::scheduler_actor_state_probe_weight_upper();
  let consume = <TestWeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
    .max(<TestWeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page());
  let instance = Actors::active_actor_view(actor_id).expect("actor exists");
  let cycle =
    Actors::compute_cycle_weight_upper(instance.actor_class.actor_type(), &instance.steps);
  let full = base
    .saturating_add(cursor)
    .saturating_add(scan)
    .saturating_add(state_probe)
    .saturating_add(consume)
    .saturating_add(cycle);
  Weight::from_parts(u64::MAX, full.proof_size().saturating_sub(1))
}

fn run_idle_until_cycle_nonce(actor_id: u64, target_cycle_nonce: u64) {
  for _ in 0..4 {
    run_idle(Weight::MAX);
    if Actors::active_actor_view(actor_id)
      .map(|instance| instance.cycle_nonce >= target_cycle_nonce)
      .unwrap_or(false)
    {
      return;
    }
  }
  panic!("cycle nonce did not reach target");
}

fn actor_event_count(predicate: impl Fn(&Event<Test>) -> bool) -> usize {
  frame_system::Pallet::<Test>::events()
    .into_iter()
    .filter_map(|record| match record.event {
      RuntimeEvent::Actors(event) => Some(event),
      _ => None,
    })
    .filter(predicate)
    .count()
}

fn has_actor_event(predicate: impl Fn(&Event<Test>) -> bool) -> bool {
  actor_event_count(predicate) > 0
}

fn mixed_materialization_ticket_trace() -> Vec<u64> {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_observation(
      7,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let steps = contract_steps_with_step(make_step(Task::StopCycle));
    let cadence_actor = create_system_with(
      ALICE,
      Schedule {
        trigger: RuntimeTrigger::cadenced(1),
        cooldown_blocks: 0,
      },
      None,
      steps.clone(),
    );
    let crossing_actor = create_system_with(
      BOB,
      Schedule {
        trigger: RuntimeTrigger::observation_crossing(7, CrossingDirection::Rising, 100, 80),
        cooldown_blocks: 0,
      },
      None,
      steps.clone(),
    );
    let change_actor = create_system_with(
      CHARLIE,
      Schedule {
        trigger: RuntimeTrigger::observation_change(8),
        cooldown_blocks: 0,
      },
      None,
      steps,
    );
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    assert_ok!(Actors::note_observation_transition(
      7,
      crate::ObservationTransition {
        revision: 2,
        previous: Some(50),
        current: 150,
      },
    ));
    assert_ok!(Actors::note_observation_changed(8, 1));
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);

    let mut trace = QueuePages::<Test>::iter()
      .flat_map(|(_, page)| page.into_iter().map(|entry| (entry.ticket, entry.actor_id)))
      .collect::<Vec<_>>();
    trace.sort_unstable_by_key(|(ticket, _)| *ticket);
    let actor_trace = trace
      .into_iter()
      .map(|(_, actor_id)| actor_id)
      .collect::<Vec<_>>();
    assert_eq!(
      actor_trace,
      vec![cadence_actor, crossing_actor, change_actor]
    );
    actor_trace
  })
}

// --- Error Coverage Tests ---

// --- Task & Predicate Coverage Tests ---

// --- Progressive Improvement Tests ---

// --- Deterministic Timer Tests ---

// --- User Actors E2E Lifecycle Tests ---

// --- Multi-Asset Funding Tests ---

fn assert_scheduler_close_requires_atomic_budget(reason: CloseReason, shortfall: Weight) {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let is_system = matches!(
      reason,
      CloseReason::ConsecutiveFailures | CloseReason::AutoCloseNonceReached
    );
    let actor_id = if is_system {
      create_system_with(ALICE, manual_schedule(), None, inert_contract_steps())
    } else {
      create_user_with(
        ALICE,
        Mutability::Mutable,
        manual_schedule(),
        None,
        inert_contract_steps(),
      )
    };
    match reason {
      CloseReason::WindowExpired => fund_native(actor_id, 1_000),
      CloseReason::CycleAdmissionInsufficient => {
        let balance = native_balance(&sovereign_account(actor_id));
        deplete_user_sovereign(actor_id, balance);
        fund_native(
          actor_id,
          TestMinUserBalance::get().saturating_add(manual_trigger_fee()),
        );
      }
      CloseReason::CycleNonceExhausted => {
        fund_native(actor_id, 1_000);
        ActorIdentities::<Test>::mutate(actor_id, |maybe| {
          maybe.as_mut().expect("actor identity exists").cycle_nonce = u64::MAX - 1;
        });
      }
      CloseReason::ConsecutiveFailures => {
        ActorHot::<Test>::mutate(actor_id, |maybe| {
          maybe
            .as_mut()
            .expect("actor hot state exists")
            .unsuccessful_attempt_streak = <Test as crate::Config>::MaxConsecutiveFailures::get();
        });
      }
      CloseReason::AutoCloseNonceReached => {
        ActorIdentities::<Test>::mutate(actor_id, |maybe| {
          maybe.as_mut().expect("actor identity exists").cycle_nonce = 1;
        });
        let mut contract = Actors::load_actor_contract(actor_id).expect("actor contract exists");
        contract.auto_close_at_cycle_nonce = Some(1);
        assert_ok!(Actors::store_actor_contract(actor_id, contract));
      }
      unsupported => panic!("unsupported admission-time close reason: {unsupported:?}"),
    }
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    if reason == CloseReason::CycleNonceExhausted {
      ActorIdentities::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("actor identity exists").cycle_nonce = u64::MAX;
      });
    }
    if reason == CloseReason::WindowExpired {
      let mut contract = Actors::load_actor_contract(actor_id).expect("actor contract exists");
      contract.window = Some(ScheduleWindow { start: 0, end: 0 });
      assert_ok!(Actors::store_actor_contract(actor_id, contract));
    }
    let before = Actors::actor_hot(actor_id)
      .unwrap_or_else(|| panic!("{reason:?} actor remains active before scheduler admission"));
    let queue_head = QueueHead::<Test>::get();
    frame_system::Pallet::<Test>::reset_events();
    let discovery = <TestWeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1);
    // Wakeups drain in the on_idle phase before execute_cycle, so the execute_cycle
    // admission budget covers only the queue discovery and actor probes plus the close.
    let pre_admission =
      discovery.saturating_add(Actors::scheduler_actor_state_probe_weight_upper());
    let close = if reason == CloseReason::CycleAdmissionInsufficient {
      <TestWeightInfo as crate::WeightInfo>::pipeline_admission_apoptosis()
    } else {
      Actors::close_cleanup_weight_upper()
    };
    let consume = <TestWeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
      .max(<TestWeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page());
    let budget = pre_admission
      .saturating_add(close)
      .saturating_add(consume)
      .saturating_sub(shortfall);
    let consumed = Actors::execute_cycle(budget).consumed;
    let after =
      Actors::actor_hot(actor_id).expect("incomplete atomic close budget preserves actor");
    assert_eq!(after.queue_ticket, before.queue_ticket);
    assert_eq!(QueueHead::<Test>::get(), queue_head);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActorClosed { actor_id: id, .. } if *id == actor_id
    )));
    assert_eq!(consumed, pre_admission);
    assert!(consumed.all_lte(budget));
  });
}

#[cfg(test)]
mod proptest_actor {
  use super::Schedule;
  use super::{
    RETRY_LATER, all_conditions, asset_balance, create_system_with, fund_native, make_step,
    manual_schedule, native_balance, prefund_active_user_creation, run_idle, run_next_idle,
    run_prepass, set_asset_balance, setup_pool, setup_temporary_retry_pool, sovereign_account,
  };
  use crate::{
    ActorFunding, ActorHot, ActorIdentities, ActorRunStateStore, AmountResolution, AssetFilter,
    CrossingDirection, CrossingPhase, CrossingTransition, CycleState, Event, FundingSourcePolicy,
    Mutability, ObservationCrossing, QueueOccupancy, QueuePages, SourceFilter, StepErrorPolicy,
    StepOf, SystemSovereignState, SystemSovereigns, Task, Trigger, WakeupBuckets, WakeupPages,
    mock::*,
  };
  use codec::Encode;
  use polkadot_sdk::frame_support::{
    BoundedVec, assert_ok,
    traits::{Get, Hooks},
  };
  use polkadot_sdk::{
    frame_system,
    sp_runtime::{Perbill, StateVersion, Weight},
  };
  use proptest::prelude::*;

  type RuntimeSchedule = Schedule;
  type RuntimeStep = StepOf<Test>;

  #[derive(Clone, Copy, Debug, Eq, PartialEq)]
  struct CrossingReferenceModel {
    direction: CrossingDirection,
    threshold: u128,
    rearm_threshold: u128,
    phase: CrossingPhase,
    revision: u64,
    current: u128,
    latched: bool,
  }

  impl CrossingReferenceModel {
    fn apply(&mut self, revision: u64, current: u128) -> Result<CrossingTransition, ()> {
      if revision <= self.revision {
        return Err(());
      }
      let transition = match (self.direction, self.phase) {
        (CrossingDirection::Rising, CrossingPhase::Armed)
          if self.current < self.threshold && current >= self.threshold =>
        {
          CrossingTransition::Fire
        }
        (CrossingDirection::Rising, CrossingPhase::WaitingForRearm)
          if self.current > self.rearm_threshold && current <= self.rearm_threshold =>
        {
          CrossingTransition::Rearm
        }
        (CrossingDirection::Falling, CrossingPhase::Armed)
          if self.current > self.threshold && current <= self.threshold =>
        {
          CrossingTransition::Fire
        }
        (CrossingDirection::Falling, CrossingPhase::WaitingForRearm)
          if self.current < self.rearm_threshold && current >= self.rearm_threshold =>
        {
          CrossingTransition::Rearm
        }
        _ => CrossingTransition::None,
      };
      self.revision = revision;
      self.current = current;
      match transition {
        CrossingTransition::Fire => {
          self.phase = CrossingPhase::WaitingForRearm;
          self.latched = true;
        }
        CrossingTransition::Rearm => self.phase = CrossingPhase::Armed,
        CrossingTransition::None => {}
      }
      Ok(transition)
    }

    fn obligation(&self) -> (u128, CrossingPhase) {
      match self.phase {
        CrossingPhase::Armed => (self.threshold, CrossingPhase::Armed),
        CrossingPhase::WaitingForRearm => (self.rearm_threshold, CrossingPhase::WaitingForRearm),
      }
    }
  }

  fn timer_schedule_pt(every_ticks: u32) -> RuntimeSchedule {
    Schedule {
      trigger: Trigger::cadenced(u64::from(every_ticks)),
      cooldown_blocks: 0,
    }
  }

  fn inert_contract_steps() -> crate::ContractSteps<crate::mock::Test> {
    BoundedVec::try_from(vec![RuntimeStep {
      precondition: all_conditions(vec![crate::Predicate::BlockNumberBelow { threshold: 0 }]),
      task: Task::StopCycle,
      on_error: StepErrorPolicy::AbortCycle,
    }])
    .expect("contract_steps must fit")
  }

  fn create_timer_actor(owner: AccountId, every_ticks: u32) -> u64 {
    let id = Actors::next_actor_id();
    let plan = inert_contract_steps();
    prefund_active_user_creation(owner, &plan);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(owner),
      Mutability::Mutable,
      {
        let schedule = timer_schedule_pt(every_ticks);
        Some(crate::ActorContract {
          trigger: schedule.trigger,
          cooldown_blocks: schedule.cooldown_blocks,
          window: None,
          steps: plan,
          completion: crate::CompletionPolicy::Persistent,
          funding: crate::FundingSourcePolicy::OwnerOnly,
          auto_close_at_cycle_nonce: None,
        })
      },
    ));
    id
  }

  #[test]
  fn swap_add_liquidity_transfer_pipeline_retries_only_the_unresolved_suffix() {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      setup_pool(TestAsset::Native, TestAsset::Local(77), 10_000, 10_000);
      set_asset_balance(&u64::MAX, TestAsset::Local(77), 10_000);
      let plan = BoundedVec::try_from(vec![
        make_step(Task::SwapIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::Fixed(20),
          slippage_tolerance: Perbill::one(),
        }),
        StepOf::<Test> {
          precondition: None,
          task: Task::AddLiquidity {
            asset_a: TestAsset::Local(77),
            asset_b: TestAsset::Local(88),
            amount_a: AmountResolution::Fixed(5),
            amount_b: AmountResolution::Fixed(5),
            min_lp_out: 1,
          },
          on_error: RETRY_LATER,
        },
        make_step(Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(10),
        }),
      ])
      .expect("three-step pipeline fits");
      let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
      let actor = sovereign_account(actor_id);
      fund_native(actor_id, 100);
      set_asset_balance(&actor, TestAsset::Local(88), 100);
      let bob_before = native_balance(&BOB);
      set_temporary_add_liquidity_failure(true);
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      run_idle(Weight::MAX);
      assert_eq!(
        Actors::actor_run_state(actor_id).expect("suspended").cursor,
        1
      );
      assert_eq!(native_balance(&actor), 80);
      assert_eq!(native_balance(&BOB), bob_before);
      let output_after_prefix = asset_balance(&actor, TestAsset::Local(77));
      assert!(output_after_prefix > 0);

      let retry_block = frame_system::Pallet::<Test>::block_number().saturating_add(1);
      frame_system::Pallet::<Test>::set_block_number(retry_block);
      Actors::on_initialize(retry_block);
      run_prepass();
      run_idle(Weight::MAX);
      run_next_idle(Weight::MAX);
      assert_eq!(
        Actors::actor_run_state(actor_id)
          .expect("same cursor")
          .cursor,
        1
      );
      assert_eq!(native_balance(&actor), 80);
      assert_eq!(
        asset_balance(&actor, TestAsset::Local(77)),
        output_after_prefix
      );

      set_temporary_add_liquidity_failure(false);
      let eligible_at = Actors::actor_run_state(actor_id)
        .expect("retry remains")
        .eligible_at;
      let prepass_block =
        eligible_at.max(frame_system::Pallet::<Test>::block_number().saturating_add(1));
      frame_system::Pallet::<Test>::set_block_number(prepass_block);
      Actors::on_initialize(prepass_block);
      run_prepass();
      run_idle(Weight::MAX);
      run_next_idle(Weight::MAX);
      assert!(Actors::actor_run_state(actor_id).is_none());
      assert_eq!(native_balance(&actor), 70);
      assert_eq!(native_balance(&BOB), bob_before + 10);
      assert_eq!(
        System::events()
          .iter()
          .filter(|record| matches!(
            record.event,
            RuntimeEvent::Actors(Event::SwapExecuted { actor_id: id, .. }) if id == actor_id
          ))
          .count(),
        1
      );
      assert_eq!(
        System::events()
          .iter()
          .filter(|record| matches!(
            record.event,
            RuntimeEvent::Actors(Event::LiquidityAdded { actor_id: id, .. }) if id == actor_id
          ))
          .count(),
        1
      );
    });
  }

  #[test]
  fn burn_before_temporary_failure_remains_committed_after_cancellation() {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      setup_temporary_retry_pool();
      let plan = BoundedVec::try_from(vec![
        make_step(Task::Burn {
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(10),
        }),
        StepOf::<Test> {
          precondition: None,
          task: Task::SwapIn {
            asset_in: TestAsset::Native,
            asset_out: TestAsset::Local(77),
            amount_in: AmountResolution::Fixed(10),
            slippage_tolerance: Perbill::one(),
          },
          on_error: RETRY_LATER,
        },
      ])
      .expect("two-step pipeline fits");
      let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
      let actor = sovereign_account(actor_id);
      fund_native(actor_id, 100);
      set_temporary_dex_failure(true);
      assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
      run_idle(Weight::MAX);
      assert_eq!(native_balance(&actor), 90);
      assert_eq!(
        Actors::actor_run_state(actor_id).expect("suspended").cursor,
        1
      );

      frame_system::Pallet::<Test>::set_block_number(2);
      run_idle(Weight::MAX);
      assert_eq!(native_balance(&actor), 90);
      assert_eq!(
        Actors::actor_run_state(actor_id).expect("same cursor").cursor,
        1
      );

      assert_ok!(Actors::cancel_run(RuntimeOrigin::root(), actor_id));
      assert_eq!(native_balance(&actor), 90);
      assert!(Actors::actor_run_state(actor_id).is_none());
      assert_eq!(
        System::events()
          .iter()
          .filter(|record| matches!(
            record.event,
            RuntimeEvent::Actors(Event::BurnExecuted { actor_id: id, amount: 10, .. }) if id == actor_id
          ))
          .count(),
        1
      );
    });
  }

  #[derive(Clone, Copy, Debug)]
  enum ModelOp {
    Create,
    Activate,
    Deactivate,
    Fund,
    Signal,
    ManualTrigger,
    Pause,
    Resume,
    UpdateContract,
    Enqueue,
    Wakeup,
    Execute,
    Close,
    UserSlotRoundTrip,
    Suspend,
    Continue,
    Cancel,
    UpdateCrossing,
    PublishObservation,
    MaterializeCrossing,
  }

  fn model_op() -> impl Strategy<Value = ModelOp> {
    (0u8..21).prop_map(|index| match index {
      0 => ModelOp::Create,
      1 => ModelOp::Activate,
      2 => ModelOp::Deactivate,
      3 => ModelOp::Fund,
      4 => ModelOp::Signal,
      5 => ModelOp::ManualTrigger,
      6 => ModelOp::Pause,
      7 => ModelOp::Resume,
      8 => ModelOp::UpdateContract,
      9 => ModelOp::Enqueue,
      10 => ModelOp::Wakeup,
      11 => ModelOp::Execute,
      12 => ModelOp::Close,
      13 => ModelOp::UserSlotRoundTrip,
      14 => ModelOp::UserSlotRoundTrip,
      15 => ModelOp::Suspend,
      16 => ModelOp::Continue,
      17 => ModelOp::Cancel,
      18 => ModelOp::UpdateCrossing,
      19 => ModelOp::PublishObservation,
      _ => ModelOp::MaterializeCrossing,
    })
  }

  fn model_retry_plan() -> crate::ContractSteps<crate::mock::Test> {
    BoundedVec::try_from(vec![
      RuntimeStep {
        precondition: None,
        task: Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(1),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
      RuntimeStep {
        precondition: None,
        task: Task::SwapIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(10)),
          slippage_tolerance: Perbill::one(),
        },
        on_error: RETRY_LATER,
      },
    ])
    .expect("model retry plan fits")
  }

  fn system_contract(
    trigger: Trigger<
      AccountId,
      TestAsset,
      <Test as crate::Config>::MaxWhitelistSize,
      <Test as crate::Config>::ObservationFeedId,
    >,
  ) -> Option<crate::ActorContractOf<Test>> {
    Some(crate::ActorContract {
      trigger,
      cooldown_blocks: 0,
      window: None,
      steps: inert_contract_steps(),
      completion: crate::CompletionPolicy::Persistent,
      funding: FundingSourcePolicy::AnyVerifiedIngress,
      auto_close_at_cycle_nonce: None,
    })
  }

  fn assert_model_invariants(
    system_id: Option<u64>,
    system_sovereign: Option<AccountId>,
    closed: bool,
    tracked_accounts: &std::collections::BTreeSet<AccountId>,
    conserved_total: Balance,
  ) {
    let hot_ids: std::collections::BTreeSet<_> = ActorHot::<Test>::iter_keys().collect();
    let contract_ids: std::collections::BTreeSet<_> =
      crate::ActorContractHeads::<Test>::iter_keys().collect();
    let funding_ids: std::collections::BTreeSet<_> = ActorFunding::<Test>::iter_keys().collect();
    let identity_ids: std::collections::BTreeSet<_> =
      ActorIdentities::<Test>::iter_keys().collect();
    let dormant_ids: std::collections::BTreeSet<_> =
      identity_ids.difference(&hot_ids).copied().collect();
    let run_ids: std::collections::BTreeSet<_> = ActorRunStateStore::<Test>::iter_keys().collect();
    assert_eq!(hot_ids, contract_ids);
    assert_eq!(hot_ids, funding_ids);
    assert!(run_ids.is_subset(&hot_ids));
    assert!(hot_ids.is_subset(&identity_ids));
    assert_eq!(Actors::active_actor_count() as usize, hot_ids.len());
    assert_eq!(Actors::actor_identity_count() as usize, identity_ids.len());

    let mut live_tickets = std::collections::BTreeSet::new();
    let mut live_wakeups = std::collections::BTreeSet::new();
    for actor_id in &hot_ids {
      let hot = ActorHot::<Test>::get(actor_id).expect("hot key resolves");
      let identity = ActorIdentities::<Test>::get(actor_id).expect("identity key resolves");
      assert_eq!(
        Actors::sovereign_index(&identity.sovereign_account),
        Some(*actor_id)
      );
      assert_eq!(
        matches!(hot.cycle_state, CycleState::Running | CycleState::Suspended),
        run_ids.contains(actor_id)
      );
      let funding = ActorFunding::<Test>::get(actor_id).expect("funding key resolves");
      assert!(
        funding
          .funding_accumulated
          .keys()
          .all(|asset| funding.funding_tracked_assets.contains(asset))
      );
      if let Some(run_state) = ActorRunStateStore::<Test>::get(*actor_id) {
        let contract = Actors::load_actor_contract(*actor_id).expect("contract key resolves");
        assert_eq!(
          identity.cycle_nonce.checked_add(1),
          Some(run_state.cycle_nonce)
        );
        assert!((run_state.cursor as usize) < contract.steps.len());
        assert!(run_state.cumulative_outcomes.executed_steps <= run_state.cursor);
        assert!(run_state.last_step_outcome.is_some());
        match hot.cycle_state {
          CycleState::Running => {
            assert!(run_state.suspension.is_none());
            assert!(run_state.last_committed_step_block.is_some());
          }
          CycleState::Suspended => assert!(run_state.suspension.is_some()),
          CycleState::Idle => panic!("Idle Actor cannot retain run state"),
        }
      }
      if let Some(ticket) = hot.queue_ticket {
        assert!(
          live_tickets.insert(ticket),
          "duplicate live queue ticket {ticket}"
        );
        let resolves = QueuePages::<Test>::iter().any(|(_, page)| {
          page
            .iter()
            .any(|entry| entry.ticket == ticket && entry.actor_id == *actor_id)
        });
        assert!(resolves, "live ticket resolves inside the canonical FIFO");
      }
      if let Some(pointer) = hot.wakeup_pointer {
        assert!(
          live_wakeups.insert((pointer.block, pointer.page_id, pointer.slot)),
          "duplicate live wakeup pointer"
        );
        let page = WakeupPages::<Test>::get((pointer.block, pointer.page_id))
          .expect("live wakeup page exists");
        assert_eq!(
          page.entries.get(pointer.slot as usize),
          Some(&Some(crate::WakeupEntry {
            actor_id: *actor_id
          }))
        );
      }
      if let Some(pointer) = hot.trigger_wakeup_pointer {
        let key = crate::WakeupKey::Tick(pointer.tick);
        assert!(
          live_wakeups.insert((key, pointer.page_id, pointer.slot)),
          "duplicate live Trigger wakeup pointer"
        );
        let page = WakeupPages::<Test>::get((key, pointer.page_id))
          .expect("live Trigger wakeup page exists");
        assert_eq!(
          page.entries.get(pointer.slot as usize),
          Some(&Some(crate::WakeupEntry {
            actor_id: *actor_id
          }))
        );
      }
    }
    for actor_id in &dormant_ids {
      let identity = ActorIdentities::<Test>::get(actor_id).expect("dormant key resolves");
      assert_eq!(
        Actors::sovereign_index(&identity.sovereign_account),
        Some(*actor_id)
      );
      assert!(ActorHot::<Test>::get(actor_id).is_none());
    }

    if let Some(actor_id) = system_id {
      if closed {
        assert_eq!(
          SystemSovereigns::<Test>::get(actor_id),
          Some(SystemSovereignState::Vacant)
        );
        assert!(ActorHot::<Test>::get(actor_id).is_none());
        assert!(ActorIdentities::<Test>::get(actor_id).is_none());
        if let Some(sovereign) = system_sovereign {
          assert_eq!(Actors::sovereign_index(sovereign), None);
        }
      } else {
        assert!(hot_ids.contains(&actor_id) || dormant_ids.contains(&actor_id));
      }
    }
    assert_eq!(Actors::owner_slot_bitmap(ALICE), [0; 32]);
    let actual_total = tracked_accounts.iter().fold(0u128, |total, account| {
      total.saturating_add(Balances::free_balance(account))
    });
    assert_eq!(actual_total, conserved_total);
  }

  proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn crossing_reference_model_matches_hysteresis_revision_and_latch_semantics(
      rising in any::<bool>(),
      threshold in 100u16..65_000u16,
      gap in 1u16..100u16,
      initial in any::<u16>(),
      updates in prop::collection::vec((0u8..3u8, any::<u16>()), 0..128),
    ) {
      let direction = if rising {
        CrossingDirection::Rising
      } else {
        CrossingDirection::Falling
      };
      let threshold = u128::from(threshold);
      let rearm_threshold = if rising {
        threshold - u128::from(gap)
      } else {
        threshold + u128::from(gap)
      };
      let crossing = ObservationCrossing {
        feed: 7u32,
        direction,
        threshold,
        rearm_threshold,
      };
      let initial = u128::from(initial);
      let mut model = CrossingReferenceModel {
        direction,
        threshold,
        rearm_threshold,
        phase: crossing.initial_phase(initial),
        revision: 1,
        current: initial,
        latched: false,
      };
      for (revision_delta, next) in updates {
        let revision = model.revision.saturating_add(u64::from(revision_delta));
        let before = model;
        let next = u128::from(next);
        let expected = model.apply(revision, next);
        if revision_delta == 0 {
          prop_assert_eq!(expected, Err(()));
          prop_assert_eq!(model, before);
          continue;
        }
        let actual = crossing.transition(before.phase, before.current, next);
        prop_assert_eq!(expected, Ok(actual));
        prop_assert_eq!(
          model.phase,
          match actual {
            CrossingTransition::Fire => CrossingPhase::WaitingForRearm,
            CrossingTransition::Rearm => CrossingPhase::Armed,
            CrossingTransition::None => before.phase,
          }
        );
        prop_assert_eq!(model.latched, before.latched || actual == CrossingTransition::Fire);
        let (obligation_threshold, obligation_phase) = model.obligation();
        prop_assert_eq!(obligation_phase, model.phase);
        prop_assert_eq!(
          obligation_threshold,
          if model.phase == CrossingPhase::Armed {
            threshold
          } else {
            rearm_threshold
          }
        );
      }
    }
  }

  proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// For any number of actors (2..max), every actor executes within bounded blocks
    #[test]
    fn scheduler_starvation_freedom(
      actor_count in 2u32..20u32,
    ) {
      let (executed_count, total_count) = new_test_ext().execute_with(|| {
        let mut actor_ids = Vec::new();
        for i in 0..actor_count {
          let owner = 100 + i as u64;
          let _ = <Balances as frame::traits::Currency<AccountId>>::deposit_creating(
            &owner, 10_000_000,
          );
          let actor_id = create_timer_actor(owner, 1);
          let sovereign = Actors::active_actor_view(actor_id)
            .expect("must exist")
            .sovereign_account;
          let _ = <Balances as frame::traits::Currency<AccountId>>::deposit_creating(
            &sovereign, 10_000_000,
          );
          actor_ids.push(actor_id);
        }
        let max_blocks = (actor_count * 3) as u64;
        let mut executed: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
        for block in 1..=max_blocks {
          frame_system::Pallet::<Test>::set_block_number(block);
          Actors::on_idle(block, Weight::MAX);
          for &actor_id in &actor_ids {
            if let Some(instance) = Actors::active_actor_view(actor_id) {
              if instance.cycle_nonce > 0 {
                executed.insert(actor_id);
              }
            }
          }
          if executed.len() == actor_ids.len() {
            break;
          }
        }
        (executed.len(), actor_ids.len())
      });
      prop_assert_eq!(
        executed_count,
        total_count,
        "Not all actors executed: {}/{}",
        executed_count,
        total_count
      );
    }

    /// Active actor count invariant holds after random create/close sequences
    #[test]
    fn active_actors_count_invariant(
      creates in 1u32..10u32,
      closes in 0u32..5u32,
    ) {
      let (active_after_create, active_after_close, expected_after_close) =
        new_test_ext().execute_with(|| {
          let mut actor_ids = Vec::new();
          for i in 0..creates {
            let owner = 200 + i as u64;
            let _ = <Balances as frame::traits::Currency<AccountId>>::deposit_creating(
              &owner, 10_000_000,
            );
            let actor_id = create_timer_actor(owner, 1);
            let sovereign = Actors::active_actor_view(actor_id)
              .expect("must exist")
              .sovereign_account;
            let _ = <Balances as frame::traits::Currency<AccountId>>::deposit_creating(
              &sovereign, 10_000_000,
            );
            actor_ids.push((actor_id, owner));
          }
          let after_create = ActorHot::<Test>::iter_keys().count();
          let close_count = closes.min(creates);
          for i in 0..close_count {
            let (actor_id, owner) = actor_ids[i as usize];
            assert_ok!(Actors::close_actor(RuntimeOrigin::signed(owner), actor_id));
          }
          let after_close = ActorHot::<Test>::iter_keys().count();
          (after_create, after_close, (creates - close_count) as usize)
        });
      prop_assert_eq!(active_after_create, creates as usize);
      prop_assert_eq!(
        active_after_close,
        expected_after_close,
        "Expected {} active actors, got {}",
        expected_after_close,
        active_after_close
      );
    }
  }

  proptest! {
    #![proptest_config(ProptestConfig {
      cases: 32,
      rng_seed: proptest::test_runner::RngSeed::Fixed(0xDE05_0731),
      ..ProptestConfig::default()
    })]

    #[test]
    fn seeded_scheduler_corruption_transitions_preserve_exact_pre_state(
      corruption in 0u8..5,
    ) {
      new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(1);
        let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
        let rejected = match corruption {
          0 => {
            assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
            WakeupBuckets::<Test>::mutate(crate::WakeupKey::Block(10), |maybe_bucket| {
              maybe_bucket.as_mut().expect("bucket").cursor_index = None;
            });
            let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
            let rejected = Actors::try_wakeup_substrate_schedule_inner(actor_id, 20).is_err();
            prop_assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
            rejected
          }
          1 => {
            assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
            ActorHot::<Test>::mutate(actor_id, |maybe_hot| {
              maybe_hot.as_mut().expect("hot").wakeup_pointer
                .as_mut().expect("pointer").slot = 7;
            });
            let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
            let rejected = Actors::try_wakeup_substrate_schedule_inner(actor_id, 20).is_err();
            prop_assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
            rejected
          }
          2 => {
            assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
            WakeupBuckets::<Test>::mutate(crate::WakeupKey::Block(10), |maybe_bucket| {
              maybe_bucket.as_mut().expect("bucket").live_entries = 0;
            });
            let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
            let rejected = Actors::try_wakeup_substrate_schedule_inner(actor_id, 20).is_err();
            prop_assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
            rejected
          }
          3 => {
            assert!(Actors::paged_enqueue(actor_id));
            assert!(Actors::paged_invalidate(actor_id).is_some());
            QueueOccupancy::<Test>::put(0);
            let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
            let rejected = Actors::paged_drain_tombstones(Actors::next_queue_ticket(), 1).is_err();
            prop_assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
            rejected
          }
          _ => {
            assert!(Actors::paged_enqueue(actor_id));
            assert!(Actors::paged_invalidate(actor_id).is_some());
            QueuePages::<Test>::remove(0);
            let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
            let rejected = Actors::paged_drain_tombstones(Actors::next_queue_ticket(), 1).is_err();
            prop_assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
            rejected
          }
        };
        prop_assert!(rejected);
        Ok(())
      })?;
    }
  }

  proptest! {
    #![proptest_config(ProptestConfig {
      cases: 32,
      rng_seed: proptest::test_runner::RngSeed::Fixed(0xDE05_0730),
      ..ProptestConfig::default()
    })]

    #[test]
    fn seeded_actor_run_state_machine_preserves_cross_store_and_scheduler_invariants(
      operations in prop::collection::vec(model_op(), 1..80),
    ) {
      new_test_ext().execute_with(|| {
        use polkadot_sdk::frame_support::traits::{Currency, ExistenceRequirement};

        frame_system::Pallet::<Test>::set_block_number(1);
        setup_temporary_retry_pool();
        set_observation(
          9,
          crate::ScalarObservationState::Fresh {
            value: 50,
            observed_at: 1,
          },
        );
        let mut observation_revision = 1u64;
        let mut observation_value = 50u128;
        let system_id = 0;
        let system_sovereign = Actors::sovereign_account_id_system(system_id);
        let user_sovereign = Actors::sovereign_account_id(&ALICE, 0);
        let tracked_accounts: std::collections::BTreeSet<AccountId> =
          std::collections::BTreeSet::from([
          ALICE,
          BOB,
          TestFeeSink::get(),
          u64::MAX,
          system_sovereign,
          user_sovereign,
        ]);
        let conserved_total = tracked_accounts.iter().fold(0u128, |total, account| {
          total.saturating_add(Balances::free_balance(account))
        });

        assert_ok!(Actors::create_system_actor(
          RuntimeOrigin::root(),
          ALICE,
          Mutability::Mutable,
          None,
        ));
        let mut closed = false;
        assert_model_invariants(
          Some(system_id),
          Some(system_sovereign),
          closed,
          &tracked_accounts,
          conserved_total,
        );

        for (index, operation) in operations.iter().enumerate() {
          let block = (index as u64).saturating_add(2);
          frame_system::Pallet::<Test>::set_block_number(block);
          let before_hot: std::collections::BTreeMap<_, _> = ActorHot::<Test>::iter().collect();
          let before_identities: std::collections::BTreeMap<_, _> =
            ActorIdentities::<Test>::iter().collect();
          let before_continuation = ActorRunStateStore::<Test>::get(system_id);
          let before_funding = ActorFunding::<Test>::get(system_id);
          let before_system_balance = Balances::free_balance(system_sovereign);
          let before_bob_balance = Balances::free_balance(BOB);
          let before_event_count = frame_system::Pallet::<Test>::events().len();

          match operation {
            ModelOp::Create => {}
            ModelOp::Activate
              if !closed
                && ActorIdentities::<Test>::contains_key(system_id)
                && !ActorHot::<Test>::contains_key(system_id) => {
              let _ = Actors::activate_actor(
                RuntimeOrigin::root(),
                system_id,
                system_contract(Trigger::manual()).expect("direct Actor Contract"),
              );
            }
            ModelOp::Deactivate if !closed && ActorHot::<Test>::contains_key(system_id) => {
              let _ = Actors::deactivate_actor(RuntimeOrigin::root(), system_id);
            }
            ModelOp::Fund if !closed => {
              let recipient = ActorIdentities::<Test>::get(system_id)
                .map(|identity| identity.sovereign_account);
              if let Some(recipient) = recipient {
                if ActorHot::<Test>::contains_key(system_id) {
                  let provenance = crate::FundingProvenance::Signed;
                  if Actors::preflight_funding_event(
                    system_id,
                    TestAsset::Native,
                    100,
                    Some(&ALICE),
                    Some(&provenance),
                  )
                  .is_ok()
                  {
                    assert_ok!(<Balances as Currency<AccountId>>::transfer(
                      &ALICE,
                      &recipient,
                      100,
                      ExistenceRequirement::AllowDeath,
                    ));
                    assert_ok!(Actors::notify_address_event(
                      system_id,
                      TestAsset::Native,
                      100,
                      &ALICE,
                    ));
                  }
                } else {
                  assert_ok!(<Balances as Currency<AccountId>>::transfer(
                    &ALICE,
                    &recipient,
                    100,
                    ExistenceRequirement::AllowDeath,
                  ));
                }
              }
            }
            ModelOp::Signal if !closed && ActorHot::<Test>::contains_key(system_id) => {
              let schedule = Schedule {
                trigger: Trigger::address_event(
                  SourceFilter::Any,
                  AssetFilter::Any,
                ),
                cooldown_blocks: 0,
              };
              let _ = update_contract_partial!(RuntimeOrigin::root(), system_id, schedule, None);
              if let Some(identity) = ActorIdentities::<Test>::get(system_id) {
                let provenance = crate::FundingProvenance::Signed;
                if Actors::preflight_funding_event(
                  system_id,
                  TestAsset::Native,
                  10,
                  Some(&ALICE),
                  Some(&provenance),
                )
                .is_ok()
                {
                  assert_ok!(<Balances as Currency<AccountId>>::transfer(
                    &ALICE,
                    &identity.sovereign_account,
                    10,
                    ExistenceRequirement::AllowDeath,
                  ));
                  assert_ok!(Actors::notify_address_event(
                    system_id,
                    TestAsset::Native,
                    10,
                    &ALICE,
                  ));
                }
              }
            }
            ModelOp::ManualTrigger | ModelOp::Enqueue
              if !closed && ActorHot::<Test>::contains_key(system_id) =>
            {
              let _ = Actors::manual_trigger(RuntimeOrigin::root(), system_id);
            }
            ModelOp::Pause if !closed && ActorHot::<Test>::contains_key(system_id) => {
              let _ = Actors::pause_actor(RuntimeOrigin::root(), system_id);
            }
            ModelOp::Resume if !closed && ActorHot::<Test>::contains_key(system_id) => {
              let _ = Actors::resume_actor(RuntimeOrigin::root(), system_id);
            }
            ModelOp::UpdateContract if !closed && ActorHot::<Test>::contains_key(system_id) => {
              let _ = update_contract_partial!(
                RuntimeOrigin::root(),
                system_id,
                inert_contract_steps(),
                crate::CompletionPolicy::Persistent,
              );
            }
            ModelOp::Wakeup if !closed && ActorHot::<Test>::contains_key(system_id) => {
              let schedule = timer_schedule_pt(2);
              let _ = update_contract_partial!(RuntimeOrigin::root(), system_id, schedule, None);
            }
            ModelOp::UpdateCrossing if !closed && ActorHot::<Test>::contains_key(system_id) => {
              let schedule = Schedule {
                trigger: Trigger::observation_crossing(
                  9,
                  CrossingDirection::Rising,
                  100,
                  80,
                ),
                cooldown_blocks: 0,
              };
              let _ = update_contract_partial!(RuntimeOrigin::root(), system_id, schedule, None);
            }
            ModelOp::PublishObservation => {
              let next = if observation_value < 100 { 150 } else { 50 };
              let next_revision = observation_revision.saturating_add(1);
              if Actors::note_observation_transition(
                9,
                crate::ObservationTransition {
                  revision: next_revision,
                  previous: Some(observation_value),
                  current: next,
                },
              )
              .is_ok()
              {
                observation_revision = next_revision;
                observation_value = next;
              }
            }
            ModelOp::MaterializeCrossing => {
              let _ = Actors::service_crossing_transitions(Weight::MAX);
            }
            ModelOp::Execute => {
              let _ = Actors::on_idle(block, Weight::MAX);
            }
            ModelOp::Suspend if !closed => {
              if let Some(hot) = ActorHot::<Test>::get(system_id)
                && !hot.lifecycle.is_paused()
              {
                let identity = ActorIdentities::<Test>::get(system_id)
                  .expect("active actor identity exists");
                let _ = update_contract_partial!(
                  RuntimeOrigin::root(),
                  system_id,
                  model_retry_plan(),
                  crate::CompletionPolicy::Persistent,
                );
                assert_ok!(<Balances as Currency<AccountId>>::transfer(
                  &ALICE,
                  &identity.sovereign_account,
                  100,
                  ExistenceRequirement::AllowDeath,
                ));
                assert_ok!(Actors::notify_address_event(
                  system_id,
                  TestAsset::Native,
                  100,
                  &ALICE,
                ));
                set_temporary_dex_failure(true);
                let _ = Actors::manual_trigger(RuntimeOrigin::root(), system_id);
                let _ = Actors::on_idle(block, Weight::MAX);
              }
            }
            ModelOp::Continue if ActorRunStateStore::<Test>::contains_key(system_id) => {
              set_temporary_dex_failure(false);
              let _ = Actors::on_idle(block, Weight::MAX);
            }
            ModelOp::Cancel if ActorRunStateStore::<Test>::contains_key(system_id) => {
              let _ = Actors::cancel_run(RuntimeOrigin::root(), system_id);
            }
            ModelOp::Close if !closed => {
              if Actors::close_actor(RuntimeOrigin::root(), system_id).is_ok() {
                closed = true;
              }
            }
            ModelOp::UserSlotRoundTrip => {
              assert_eq!(Actors::owner_slot_bitmap(ALICE), [0; 32]);
              assert_ok!(Actors::create_user_actor_at_slot(
                RuntimeOrigin::signed(ALICE),
                0,
                Mutability::Mutable,
                None,
              ));
              let user_id = Actors::next_actor_id().saturating_sub(1);
              assert_eq!(
                Actors::actor_identities(user_id)
                  .expect("temporary User identity exists")
                  .sovereign_account,
                user_sovereign
              );
              assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), user_id));
              assert_eq!(Actors::owner_slot_bitmap(ALICE), [0; 32]);
            }
            _ => {}
          }
          let events = frame_system::Pallet::<Test>::events();
          let event_delta = &events[before_event_count..];
          if matches!(operation, ModelOp::PublishObservation | ModelOp::MaterializeCrossing) {
            assert!(
              event_delta.is_empty(),
              "observation ingress and deferred Crossing materialization are event-silent"
            );
          }
          let actor_event_delta = event_delta
            .iter()
            .filter_map(|record| match &record.event {
              RuntimeEvent::Actors(event) => Some(event),
              _ => None,
            })
            .collect::<Vec<_>>();
          if matches!(
            operation,
            ModelOp::Activate
              | ModelOp::Deactivate
              | ModelOp::Pause
              | ModelOp::Resume
              | ModelOp::UpdateContract
              | ModelOp::Wakeup
              | ModelOp::UpdateCrossing
              | ModelOp::Close
          ) {
            assert!(actor_event_delta.iter().all(|event| match operation {
              ModelOp::Activate => matches!(event, Event::ActorActivated { actor_id } if *actor_id == system_id),
              ModelOp::Deactivate => matches!(
                event,
                Event::ActorDeactivated { actor_id }
                  | Event::CycleCancelled { actor_id, .. }
                  | Event::CycleSummary { actor_id, .. }
                  if *actor_id == system_id
              ),
              ModelOp::Pause => matches!(event, Event::ActorPaused { actor_id } if *actor_id == system_id),
              ModelOp::Resume => matches!(event, Event::ActorResumed { actor_id } if *actor_id == system_id),
              ModelOp::UpdateContract | ModelOp::Wakeup | ModelOp::UpdateCrossing => matches!(
                event,
                Event::ContractUpdated { actor_id }
                  | Event::CycleCancelled { actor_id, .. }
                  | Event::CycleSummary { actor_id, .. }
                  if *actor_id == system_id
              ),
              ModelOp::Close => matches!(
                event,
                Event::ActorClosed { actor_id, .. }
                  | Event::CycleCancelled { actor_id, .. }
                  | Event::CycleSummary { actor_id, .. }
                  if *actor_id == system_id
              ),
              _ => false,
            }), "unexpected control-event delta: {actor_event_delta:?}");
          }
          if !closed
            && !ActorHot::<Test>::contains_key(system_id)
            && !ActorIdentities::<Test>::contains_key(system_id)
            && SystemSovereigns::<Test>::get(system_id) == Some(SystemSovereignState::Vacant)
          {
            closed = true;
          }

          let after_continuation = ActorRunStateStore::<Test>::get(system_id);
          if matches!(operation, ModelOp::Cancel) && before_continuation.is_some() {
            if after_continuation.is_some() {
              assert_eq!(
                after_continuation.as_ref().map(Encode::encode),
                before_continuation.as_ref().map(Encode::encode),
                "rejected cancellation preserves the exact run"
              );
            } else {
              assert_eq!(Balances::free_balance(system_sovereign), before_system_balance);
              assert_eq!(Balances::free_balance(BOB), before_bob_balance);
              assert_eq!(
                ActorFunding::<Test>::get(system_id).as_ref().map(Encode::encode),
                before_funding.as_ref().map(Encode::encode)
              );
            }
          }
          if matches!(operation, ModelOp::Pause | ModelOp::Resume)
            && before_continuation.is_some()
            && ActorHot::<Test>::contains_key(system_id)
          {
            assert_eq!(
              after_continuation.as_ref().map(Encode::encode),
              before_continuation.as_ref().map(Encode::encode)
            );
          }
          if let (Some(before), Some(after), Some(previous_identity), Some(current_identity)) = (
            before_continuation.as_ref(),
            after_continuation.as_ref(),
            before_identities.get(&system_id),
            ActorIdentities::<Test>::get(system_id),
          ) && previous_identity.cycle_nonce == current_identity.cycle_nonce
          {
            assert!(after.cursor >= before.cursor);
            assert!(
              after.cumulative_outcomes.executed_steps
                >= before.cumulative_outcomes.executed_steps
            );
          }

          for (actor_id, previous) in &before_hot {
            if let (Some(_hot), Some(previous_identity), Some(identity)) = (
              ActorHot::<Test>::get(actor_id),
              before_identities.get(actor_id),
              ActorIdentities::<Test>::get(actor_id),
            ) {
              assert!(identity.cycle_nonce <= previous_identity.cycle_nonce.saturating_add(1));
              if matches!(operation, ModelOp::Execute) && previous.lifecycle.is_paused() {
                assert_eq!(identity.cycle_nonce, previous_identity.cycle_nonce);
              }
            }
          }
          assert_model_invariants(
            Some(system_id),
            Some(system_sovereign),
            closed,
            &tracked_accounts,
            conserved_total,
          );
        }
      });
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StepParityStimulus {
  PreconditionFalse,
  PredicateError,
  ResolutionSkipped,
  FundingUnavailable,
  SuccessfulTask,
  StopCycle,
  TemporaryFailure,
  PermanentFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StepParityBound {
  Below,
  GlobalReached,
  LocalReached,
}

#[derive(Clone, Copy, Debug)]
struct StepParityCase {
  row: &'static str,
  name: &'static str,
  stimulus: StepParityStimulus,
  policy: StepErrorPolicy,
  mutability: Mutability,
  actor_type: ActorType,
  bound: StepParityBound,
}

const PARITY_RETRY: StepErrorPolicy = StepErrorPolicy::RetryLater { max_attempts: 2 };

const STEP_TRANSITION_PARITY_MATRIX: &[StepParityCase] = &[
  StepParityCase {
    row: "ST-01",
    name: "precondition-false/continue/user",
    stimulus: StepParityStimulus::PreconditionFalse,
    policy: StepErrorPolicy::ContinueNextStep,
    mutability: Mutability::Mutable,
    actor_type: ActorType::User,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-01",
    name: "precondition-false/abort/immutable",
    stimulus: StepParityStimulus::PreconditionFalse,
    policy: StepErrorPolicy::AbortCycle,
    mutability: Mutability::Immutable,
    actor_type: ActorType::System,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-01",
    name: "precondition-false/retry",
    stimulus: StepParityStimulus::PreconditionFalse,
    policy: PARITY_RETRY,
    mutability: Mutability::Mutable,
    actor_type: ActorType::System,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-02",
    name: "resolution-skip/continue",
    stimulus: StepParityStimulus::ResolutionSkipped,
    policy: StepErrorPolicy::ContinueNextStep,
    mutability: Mutability::Mutable,
    actor_type: ActorType::System,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-02",
    name: "resolution-skip/abort/user",
    stimulus: StepParityStimulus::ResolutionSkipped,
    policy: StepErrorPolicy::AbortCycle,
    mutability: Mutability::Immutable,
    actor_type: ActorType::User,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-02",
    name: "resolution-skip/retry",
    stimulus: StepParityStimulus::ResolutionSkipped,
    policy: PARITY_RETRY,
    mutability: Mutability::Mutable,
    actor_type: ActorType::System,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-03",
    name: "funding-unavailable/continue",
    stimulus: StepParityStimulus::FundingUnavailable,
    policy: StepErrorPolicy::ContinueNextStep,
    mutability: Mutability::Mutable,
    actor_type: ActorType::System,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-03",
    name: "funding-unavailable/abort/immutable",
    stimulus: StepParityStimulus::FundingUnavailable,
    policy: StepErrorPolicy::AbortCycle,
    mutability: Mutability::Immutable,
    actor_type: ActorType::System,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-04",
    name: "funding-unavailable/retry/suspend",
    stimulus: StepParityStimulus::FundingUnavailable,
    policy: PARITY_RETRY,
    mutability: Mutability::Mutable,
    actor_type: ActorType::System,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-05",
    name: "funding-unavailable/retry/global-bound",
    stimulus: StepParityStimulus::FundingUnavailable,
    policy: PARITY_RETRY,
    mutability: Mutability::Mutable,
    actor_type: ActorType::System,
    bound: StepParityBound::GlobalReached,
  },
  StepParityCase {
    row: "ST-05",
    name: "funding-unavailable/retry/local-bound",
    stimulus: StepParityStimulus::FundingUnavailable,
    policy: PARITY_RETRY,
    mutability: Mutability::Mutable,
    actor_type: ActorType::System,
    bound: StepParityBound::LocalReached,
  },
  StepParityCase {
    row: "ST-06",
    name: "successful-task/continue",
    stimulus: StepParityStimulus::SuccessfulTask,
    policy: StepErrorPolicy::ContinueNextStep,
    mutability: Mutability::Mutable,
    actor_type: ActorType::System,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-06",
    name: "successful-task/abort/user-immutable",
    stimulus: StepParityStimulus::SuccessfulTask,
    policy: StepErrorPolicy::AbortCycle,
    mutability: Mutability::Immutable,
    actor_type: ActorType::User,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-06",
    name: "successful-task/retry",
    stimulus: StepParityStimulus::SuccessfulTask,
    policy: PARITY_RETRY,
    mutability: Mutability::Mutable,
    actor_type: ActorType::System,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-07",
    name: "stop-cycle/continue/user",
    stimulus: StepParityStimulus::StopCycle,
    policy: StepErrorPolicy::ContinueNextStep,
    mutability: Mutability::Mutable,
    actor_type: ActorType::User,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-07",
    name: "stop-cycle/abort/immutable",
    stimulus: StepParityStimulus::StopCycle,
    policy: StepErrorPolicy::AbortCycle,
    mutability: Mutability::Immutable,
    actor_type: ActorType::System,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-07",
    name: "stop-cycle/retry",
    stimulus: StepParityStimulus::StopCycle,
    policy: PARITY_RETRY,
    mutability: Mutability::Mutable,
    actor_type: ActorType::System,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-08",
    name: "temporary-failure/continue/user-immutable",
    stimulus: StepParityStimulus::TemporaryFailure,
    policy: StepErrorPolicy::ContinueNextStep,
    mutability: Mutability::Immutable,
    actor_type: ActorType::User,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-09",
    name: "temporary-failure/abort",
    stimulus: StepParityStimulus::TemporaryFailure,
    policy: StepErrorPolicy::AbortCycle,
    mutability: Mutability::Mutable,
    actor_type: ActorType::System,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-09",
    name: "temporary-failure/abort/global-bound",
    stimulus: StepParityStimulus::TemporaryFailure,
    policy: StepErrorPolicy::AbortCycle,
    mutability: Mutability::Immutable,
    actor_type: ActorType::System,
    bound: StepParityBound::GlobalReached,
  },
  StepParityCase {
    row: "ST-10",
    name: "temporary-failure/retry/suspend/user",
    stimulus: StepParityStimulus::TemporaryFailure,
    policy: PARITY_RETRY,
    mutability: Mutability::Mutable,
    actor_type: ActorType::User,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-11",
    name: "temporary-failure/retry/global-bound",
    stimulus: StepParityStimulus::TemporaryFailure,
    policy: PARITY_RETRY,
    mutability: Mutability::Mutable,
    actor_type: ActorType::System,
    bound: StepParityBound::GlobalReached,
  },
  StepParityCase {
    row: "ST-11",
    name: "temporary-failure/retry/local-bound",
    stimulus: StepParityStimulus::TemporaryFailure,
    policy: PARITY_RETRY,
    mutability: Mutability::Mutable,
    actor_type: ActorType::System,
    bound: StepParityBound::LocalReached,
  },
  StepParityCase {
    row: "ST-12",
    name: "permanent-failure/continue/user-immutable",
    stimulus: StepParityStimulus::PermanentFailure,
    policy: StepErrorPolicy::ContinueNextStep,
    mutability: Mutability::Immutable,
    actor_type: ActorType::User,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-12",
    name: "predicate-error/continue",
    stimulus: StepParityStimulus::PredicateError,
    policy: StepErrorPolicy::ContinueNextStep,
    mutability: Mutability::Mutable,
    actor_type: ActorType::System,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-13",
    name: "permanent-failure/abort",
    stimulus: StepParityStimulus::PermanentFailure,
    policy: StepErrorPolicy::AbortCycle,
    mutability: Mutability::Immutable,
    actor_type: ActorType::System,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-13",
    name: "permanent-failure/retry/user",
    stimulus: StepParityStimulus::PermanentFailure,
    policy: PARITY_RETRY,
    mutability: Mutability::Mutable,
    actor_type: ActorType::User,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-13",
    name: "predicate-error/abort/user-immutable",
    stimulus: StepParityStimulus::PredicateError,
    policy: StepErrorPolicy::AbortCycle,
    mutability: Mutability::Immutable,
    actor_type: ActorType::User,
    bound: StepParityBound::Below,
  },
  StepParityCase {
    row: "ST-13",
    name: "predicate-error/retry",
    stimulus: StepParityStimulus::PredicateError,
    policy: PARITY_RETRY,
    mutability: Mutability::Mutable,
    actor_type: ActorType::System,
    bound: StepParityBound::Below,
  },
];

fn parity_target_step(case: StepParityCase) -> StepOf<Test> {
  let transfer = |amount| Task::Transfer {
    to: BOB,
    asset: TestAsset::Native,
    amount,
  };
  let (precondition, task) = match case.stimulus {
    StepParityStimulus::PreconditionFalse => (
      all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 1_000_000,
      }]),
      transfer(AmountResolution::Fixed(5)),
    ),
    StepParityStimulus::PredicateError => (
      all_conditions(vec![Predicate::ObservationAbove {
        feed: 1,
        threshold: 1,
        max_age_blocks: 1,
      }]),
      transfer(AmountResolution::Fixed(5)),
    ),
    StepParityStimulus::ResolutionSkipped => (
      None,
      transfer(AmountResolution::PercentageOfCurrent(Perbill::from_parts(
        1,
      ))),
    ),
    StepParityStimulus::FundingUnavailable => (None, transfer(AmountResolution::Fixed(10))),
    StepParityStimulus::SuccessfulTask => (None, transfer(AmountResolution::Fixed(5))),
    StepParityStimulus::StopCycle => (None, Task::StopCycle),
    StepParityStimulus::TemporaryFailure | StepParityStimulus::PermanentFailure => (
      None,
      Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
    ),
  };
  StepOf::<Test> {
    precondition,
    task,
    on_error: case.policy,
  }
}

fn parity_contract_steps(case: StepParityCase) -> crate::ContractSteps<Test> {
  BoundedVec::try_from(vec![
    make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(2),
    }),
    parity_target_step(case),
    make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(3),
    }),
  ])
  .expect("parity matrix contract fits")
}

fn parity_advances(case: StepParityCase) -> bool {
  match case.stimulus {
    StepParityStimulus::PreconditionFalse
    | StepParityStimulus::ResolutionSkipped
    | StepParityStimulus::SuccessfulTask => true,
    StepParityStimulus::StopCycle => false,
    StepParityStimulus::FundingUnavailable => {
      !matches!(case.policy, StepErrorPolicy::RetryLater { .. })
    }
    StepParityStimulus::PredicateError
    | StepParityStimulus::TemporaryFailure
    | StepParityStimulus::PermanentFailure => case.policy == StepErrorPolicy::ContinueNextStep,
  }
}

fn parity_expected_steps(case: StepParityCase) -> Vec<SimulationStepRecord> {
  let target = match case.stimulus {
    StepParityStimulus::PreconditionFalse => {
      StepOutcome::Skipped(StepSkippedReason::PreconditionFalse)
    }
    StepParityStimulus::PredicateError => {
      StepOutcome::Failed(TaskFailure::permanent(Error::<Test>::InvalidPredicate))
    }
    StepParityStimulus::ResolutionSkipped => {
      StepOutcome::Skipped(StepSkippedReason::ResolutionSkipped)
    }
    StepParityStimulus::FundingUnavailable => StepOutcome::FundingUnavailable,
    StepParityStimulus::SuccessfulTask => StepOutcome::Executed,
    StepParityStimulus::StopCycle => StepOutcome::Stopped,
    StepParityStimulus::TemporaryFailure => StepOutcome::Failed(TaskFailure::temporary(
      DispatchError::Other("TemporaryDexCapacity"),
    )),
    StepParityStimulus::PermanentFailure => StepOutcome::Failed(TaskFailure::permanent(
      DispatchError::Other("MockDexAfterInputTransferFailed"),
    )),
  };
  let mut steps = if case.bound == StepParityBound::LocalReached {
    Vec::new()
  } else {
    vec![SimulationStepRecord {
      step_index: 0,
      outcome: StepOutcome::Executed,
    }]
  };
  steps.push(SimulationStepRecord {
    step_index: 1,
    outcome: target,
  });
  if parity_advances(case) {
    steps.push(SimulationStepRecord {
      step_index: 2,
      outcome: StepOutcome::Executed,
    });
  }
  steps
}

fn observed_attempt_projection(
  actor_id: ActorId,
) -> (AttemptDisposition, OutcomeTotals, Option<u32>, Option<u32>) {
  let actor_events: Vec<_> = System::events()
    .into_iter()
    .filter_map(|record| match record.event {
      RuntimeEvent::Actors(event) => Some(event),
      _ => None,
    })
    .collect();
  let summary = actor_events.iter().rev().find_map(|event| match event {
    Event::CycleSummary {
      actor_id: id,
      result,
      outcomes,
      ..
    } if *id == actor_id => Some((*result, *outcomes)),
    _ => None,
  });
  let closed = actor_events.iter().rev().find_map(|event| match event {
    Event::ActorClosed {
      actor_id: id,
      reason,
    } if *id == actor_id => Some(*reason),
    _ => None,
  });
  if let Some(reason) = closed {
    let (_, outcomes) = summary.expect("attempt close follows its cycle summary");
    return (AttemptDisposition::Closed(reason), outcomes, None, None);
  }
  if let Some(continuation) = Actors::actor_run_state(actor_id) {
    return (
      AttemptDisposition::Suspended,
      continuation.cumulative_outcomes,
      Some(continuation.cursor),
      Some(continuation.unsuccessful_attempts_at_cursor),
    );
  }
  let (result, outcomes) = summary.expect("terminal attempt emits a cycle summary");
  let disposition = match result {
    CycleResult::Completed => AttemptDisposition::Completed,
    CycleResult::Failed => AttemptDisposition::Failed,
    CycleResult::Cancelled => panic!("matrix does not cancel cycles"),
  };
  (disposition, outcomes, None, None)
}

// --- Eligibility Projection API (spec 7.3) ---

fn eligibility(actor_id: ActorId) -> ActorEligibility<u32, u64> {
  Actors::actor_eligibility(actor_id).expect("eligibility computes")
}

fn active_eligibility(actor_id: ActorId) -> ActorClassification<u64> {
  match eligibility(actor_id) {
    ActorEligibility::Active(activation) => activation.eligibility,
    other => panic!("expected active eligibility, got {other:?}"),
  }
}

mod core;
mod crossing;
mod execution;
mod fees_and_funding;
mod lifecycle;
mod market_tasks;
mod observations;
mod scheduling;
mod storage_and_api;
mod wakeups;
