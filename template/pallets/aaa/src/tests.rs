use crate::{
  AaaType, ActiveLifecycle, ActorClass, ActorHot, ActorProgram, AmountResolution, AssetFilter,
  AssetFilterOf, CloseReason, Condition, DeferReason, Error, Event, FundingSourcePolicy,
  IdleStarvationBlocks, Mutability, NextAaaId, OnCloseStepFailureKind, OwnerSlotMask, PauseReason,
  ProgramInput, QueueEntry, SYSTEM_OWNER_SLOT_SENTINEL, Schedule, ScheduleOf, ScheduleWindow,
  SourceFilter, SourceFilterOf, SovereignIndex, SplitLeg, SplitTransferLegsOf, StepErrorPolicy,
  StepOf, StepSkippedReason, SweepCursor, Task, TaskOf, Trigger, adapters::AssetOps, mock::*,
  types::FundingBatch,
};
use alloc::collections::BTreeSet;
use polkadot_sdk::frame_support::{
  __private::metadata_ir::{
    StorageEntryMetadataIR, StorageEntryModifierIR, StorageEntryTypeIR, StorageHasherIR,
  },
  BoundedBTreeSet, BoundedVec, assert_noop, assert_ok,
  traits::{Currency, Get, Hooks, LockableCurrency, StorageInfoTrait, WithdrawReasons},
};
use polkadot_sdk::{
  frame_system,
  sp_runtime::{DispatchError, Perbill, Weight},
  sp_weights::WeightToFee,
};
use scale_info::{TypeDef, TypeInfo};

type RuntimeSchedule = ScheduleOf<Test>;
type RuntimeSourceFilter = SourceFilterOf<Test>;
type RuntimeAssetFilter = AssetFilterOf<Test>;
type RuntimeTask = TaskOf<Test>;
type RuntimeStep = StepOf<Test>;
type RuntimeProgramInput = crate::ProgramInputOf<Test>;
type MockBlockNumber = polkadot_sdk::frame_system::pallet_prelude::BlockNumberFor<Test>;

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

fn assert_variant_contract<T: TypeInfo>(expected: &[(&str, u8)]) {
  let info = T::type_info();
  let TypeDef::Variant(definition) = info.type_def else {
    panic!("contract type must be a SCALE variant");
  };
  let actual: alloc::vec::Vec<_> = definition
    .variants
    .iter()
    .map(|variant| (variant.name, variant.index))
    .collect();
  assert_eq!(actual, expected);
}

#[test]
fn aaa_0_7_2_candidate_scale_variant_indices_are_explicit() {
  assert_variant_contract::<RuntimeTask>(&[
    ("Transfer", 0),
    ("SplitTransfer", 1),
    ("SwapExactIn", 2),
    ("SwapExactOut", 3),
    ("AddLiquidity", 4),
    ("RemoveLiquidity", 5),
    ("Burn", 6),
    ("Mint", 7),
    ("Stake", 8),
    ("DonateLiquidity", 9),
    ("Unstake", 10),
  ]);
  assert_variant_contract::<AmountResolution<u128>>(&[
    ("Fixed", 0),
    ("PercentageOfCurrent", 1),
    ("PercentageOfTrigger", 2),
    ("PercentageOfLastFunding", 3),
    ("AllBalance", 4),
  ]);
  assert_variant_contract::<Condition<TestAsset, u128>>(&[
    ("BalanceAbove", 0),
    ("BalanceBelow", 1),
    ("BalanceEquals", 2),
    ("BalanceNotEquals", 3),
    ("BlockNumberAbove", 4),
    ("BlockNumberBelow", 5),
  ]);
  assert_variant_contract::<RuntimeSourceFilter>(&[("Any", 0), ("OwnerOnly", 1), ("Whitelist", 2)]);
  assert_variant_contract::<RuntimeAssetFilter>(&[("Any", 0), ("Whitelist", 1)]);
  assert_variant_contract::<Trigger<AccountId, TestAsset, <Test as crate::Config>::MaxWhitelistSize>>(
    &[("Timer", 0), ("OnAddressEvent", 1), ("Manual", 2)],
  );
  assert_variant_contract::<AaaType>(&[("User", 0), ("System", 1)]);
  assert_variant_contract::<ActorClass>(&[("User", 0), ("System", 1)]);
  assert_variant_contract::<Mutability>(&[("Mutable", 0), ("Immutable", 1)]);
  assert_variant_contract::<RuntimeProgramInput>(&[("Dormant", 0), ("Active", 1)]);
  assert_variant_contract::<PauseReason>(&[("Manual", 0), ("CycleNonceExhausted", 1)]);
  assert_variant_contract::<ActiveLifecycle>(&[("Active", 0), ("Paused", 1)]);
  assert_variant_contract::<CloseReason>(&[
    ("OwnerInitiated", 0),
    ("BalanceExhausted", 1),
    ("ConsecutiveFailures", 2),
    ("WindowExpired", 3),
    ("CycleNonceExhausted", 4),
    ("FeeBudgetExhausted", 5),
    ("AutoCloseNonceReached", 6),
  ]);
  assert_variant_contract::<StepErrorPolicy>(&[("AbortCycle", 0), ("ContinueNextStep", 1)]);
  assert_variant_contract::<DeferReason>(&[
    ("InsufficientWeightBudget", 0),
    ("CloseTransitionFailed", 1),
  ]);
  assert_variant_contract::<StepSkippedReason>(&[
    ("ConditionsNotMet", 0),
    ("ResolutionSkipped", 1),
    ("FundingUnavailable", 2),
  ]);
  assert_variant_contract::<OnCloseStepFailureKind>(&[
    ("EvaluationFee", 0),
    ("ExecutionFee", 1),
    ("Condition", 2),
    ("Resolution", 3),
    ("Adapter", 4),
  ]);
  assert_variant_contract::<
    FundingSourcePolicy<AccountId, <Test as crate::Config>::MaxWhitelistSize>,
  >(&[
    ("OwnerOnly", 0),
    ("SignedAllowlist", 1),
    ("RuntimePolicy", 2),
    ("AnySource", 3),
  ]);
  assert_variant_contract::<Event<Test>>(&[
    ("AaaCreated", 0),
    ("AaaActivated", 1),
    ("AaaDeactivated", 2),
    ("AaaPaused", 3),
    ("AaaResumed", 4),
    ("AaaClosed", 5),
    ("CycleDeferred", 6),
    ("WakeupRescheduled", 7),
    ("WakeupScheduleDropped", 8),
    ("CycleStarted", 9),
    ("CycleSummary", 10),
    ("StepSkipped", 11),
    ("StepFailed", 12),
    ("TransferExecuted", 13),
    ("SplitTransferExecuted", 14),
    ("SwapExecuted", 15),
    ("BurnExecuted", 16),
    ("MintExecuted", 17),
    ("StakeExecuted", 18),
    ("UnstakeExecuted", 19),
    ("LiquidityDonated", 20),
    ("LiquidityAdded", 21),
    ("LiquidityRemoved", 22),
    ("ScheduleUpdated", 23),
    ("ExecutionPlanUpdated", 24),
    ("OnCloseExecutionPlanUpdated", 25),
    ("OnCloseStepFailed", 26),
    ("OnCloseStepSkipped", 27),
    ("OnCloseExecutionPlanSummary", 28),
    ("AutoCloseNonceSet", 29),
    ("AutoCloseNonceIncremented", 30),
    ("ActiveActorLimitSet", 31),
    ("GlobalCircuitBreakerSet", 32),
    ("ManualTriggerSet", 33),
    ("SweepBatchProcessed", 34),
    ("IdleStarvationDetected", 35),
    ("FundingSourcePolicyUpdated", 36),
    ("FundingBatchActivated", 37),
    ("FundingBatchPendingAccumulated", 38),
    ("FundingBatchPromoted", 39),
  ]);
  assert_variant_contract::<Error<Test>>(&[
    ("AaaIdOverflow", 0),
    ("AaaNotFound", 1),
    ("ActiveAaaCapacityExceeded", 2),
    ("ActiveAaaCountInvariant", 3),
    ("ActorIdentityCapacityExceeded", 4),
    ("ActorIdentityCountInvariant", 5),
    ("AaaAlreadyActive", 6),
    ("AaaDormant", 7),
    ("ActiveAaaLimitExceedsQueueCapacity", 8),
    ("ActiveAaaLimitTooHigh", 9),
    ("ActiveAaaLimitTooLow", 10),
    ("AaaPaused", 11),
    ("EmptyExecutionPlan", 12),
    ("ExecutionPlanExceedsOnIdleBudget", 13),
    ("ExecutionDelayTooLong", 14),
    ("GlobalCircuitBreakerActive", 15),
    ("ImmutableAaa", 16),
    ("InsufficientBalance", 17),
    ("InsufficientFee", 18),
    ("InvalidAmountResolution", 19),
    ("InvalidAutoCloseNonce", 20),
    ("InvalidScheduleWindow", 21),
    ("InvalidSplitTransfer", 22),
    ("InvalidTriggerConfiguration", 23),
    ("MintNotAllowedForUserAaa", 24),
    ("NotGovernance", 25),
    ("NotOwner", 26),
    ("NotPaused", 27),
    ("OwnerSlotCapacityExceeded", 28),
    ("OwnerSlotOccupied", 29),
    ("InvalidOwnerSlot", 30),
    ("AaaIdOccupied", 31),
    ("SystemAaaNotClosed", 32),
    ("ExecutionPlanTooLong", 33),
    ("SnapshotUnavailable", 34),
    ("FundingBatchOverflow", 35),
    ("SovereignAccountCollision", 36),
    ("AutoCloseNonceHorizonExceeded", 37),
    ("AutoCloseNonceOverflow", 38),
    ("AutoCloseNonceIncrementZero", 39),
    ("QueueMutationRateLimited", 40),
  ]);
  assert_variant_contract::<crate::Call<Test>>(&[
    ("create_user_aaa", 0),
    ("create_user_aaa_at_slot", 1),
    ("create_system_aaa", 2),
    ("reopen_system_aaa", 3),
    ("pause_aaa", 4),
    ("resume_aaa", 5),
    ("manual_trigger", 6),
    ("update_funding_source_policy", 7),
    ("close_aaa", 8),
    ("update_schedule", 9),
    ("set_global_circuit_breaker", 10),
    ("permissionless_sweep", 11),
    ("update_execution_plan", 12),
    ("set_active_actor_limit", 13),
    ("permissionless_sweep_many", 14),
    ("set_auto_close_at_cycle_nonce", 15),
    ("increment_auto_close_nonce", 16),
    ("update_on_close_execution_plan", 17),
    ("activate_aaa", 21),
    ("deactivate_aaa", 22),
  ]);
}

#[test]
fn aaa_0_7_2_candidate_storage_schema_is_explicit() {
  let storage_info = AAA::storage_info();
  assert!(storage_info.iter().all(|entry| entry.pallet_name == b"AAA"));
  let actual: alloc::vec::Vec<_> = storage_info
    .iter()
    .map(|entry| core::str::from_utf8(&entry.storage_name).expect("storage name is UTF-8"))
    .collect();
  assert_eq!(
    actual,
    [
      "NextAaaId",
      "SweepCursor",
      "ActorHot",
      "ActorProgram",
      "ActorFunding",
      "DormantAaaIdentities",
      "ActorIdentityCount",
      "ActiveAaaCount",
      "ClosedSystemAaaIds",
      "QueueHead",
      "QueueTail",
      "QueuePages",
      "WakeupIndex",
      "MinWakeupBlock",
      "ScheduledWakeupBlock",
      "WakeupScheduleDrops",
      "WakeupRetryPending",
      "OwnerSlotMask",
      "SovereignIndex",
      "ActiveActorLimit",
      "GlobalCircuitBreaker",
      "AddressEventInbox",
      "IngressOverflowSlots",
      "IngressOverflowHead",
      "IngressOverflowLen",
      "IdleStarvationBlocks",
      "LastIngressIngestBlock",
    ]
  );

  let metadata = AAA::storage_metadata();
  assert_eq!(metadata.prefix, "AAA");
  let actual_shapes: alloc::vec::Vec<_> = metadata
    .entries
    .iter()
    .map(|entry| {
      let optional = matches!(entry.modifier, StorageEntryModifierIR::Optional);
      let is_blake_map = match &entry.ty {
        StorageEntryTypeIR::Plain(_) => false,
        StorageEntryTypeIR::Map { hashers, .. } => {
          assert_eq!(hashers, &[StorageHasherIR::Blake2_128Concat]);
          true
        }
      };
      (entry.name, optional, is_blake_map)
    })
    .collect();
  assert_eq!(
    actual_shapes,
    [
      ("NextAaaId", false, false),
      ("SweepCursor", false, false),
      ("ActorHot", true, true),
      ("ActorProgram", true, true),
      ("ActorFunding", true, true),
      ("DormantAaaIdentities", true, true),
      ("ActorIdentityCount", false, false),
      ("ActiveAaaCount", false, false),
      ("ClosedSystemAaaIds", true, true),
      ("QueueHead", false, false),
      ("QueueTail", false, false),
      ("QueuePages", true, true),
      ("WakeupIndex", false, true),
      ("MinWakeupBlock", true, false),
      ("ScheduledWakeupBlock", true, true),
      ("WakeupScheduleDrops", false, false),
      ("WakeupRetryPending", false, true),
      ("OwnerSlotMask", false, true),
      ("SovereignIndex", true, true),
      ("ActiveActorLimit", false, false),
      ("GlobalCircuitBreaker", false, false),
      ("AddressEventInbox", true, true),
      ("IngressOverflowSlots", true, true),
      ("IngressOverflowHead", false, false),
      ("IngressOverflowLen", false, false),
      ("IdleStarvationBlocks", false, false),
      ("LastIngressIngestBlock", true, false),
    ]
  );

  let entries = &metadata.entries;
  assert_plain_storage_type::<u64>(&entries[0]);
  assert_plain_storage_type::<u64>(&entries[1]);
  assert_map_storage_types::<u64, crate::ActorHotStateOf<Test>>(&entries[2]);
  assert_map_storage_types::<u64, crate::ActorProgramStateOf<Test>>(&entries[3]);
  assert_map_storage_types::<u64, crate::ActorFundingStateOf<Test>>(&entries[4]);
  assert_map_storage_types::<u64, crate::DormantAaaIdentityOf<Test>>(&entries[5]);
  assert_plain_storage_type::<u32>(&entries[6]);
  assert_plain_storage_type::<u32>(&entries[7]);
  assert_map_storage_types::<u64, Mutability>(&entries[8]);
  assert_plain_storage_type::<u64>(&entries[9]);
  assert_plain_storage_type::<u64>(&entries[10]);
  assert_map_storage_types::<u64, crate::QueuePageOf<Test>>(&entries[11]);
  assert_map_storage_types::<
    MockBlockNumber,
    BoundedVec<u64, <Test as crate::Config>::MaxWakeupBucketSize>,
  >(&entries[12]);
  assert_plain_storage_type::<MockBlockNumber>(&entries[13]);
  assert_map_storage_types::<u64, MockBlockNumber>(&entries[14]);
  assert_plain_storage_type::<u64>(&entries[15]);
  assert_map_storage_types::<u64, bool>(&entries[16]);
  assert_map_storage_types::<AccountId, u8>(&entries[17]);
  assert_map_storage_types::<AccountId, u64>(&entries[18]);
  assert_plain_storage_type::<u32>(&entries[19]);
  assert_plain_storage_type::<bool>(&entries[20]);
  assert_map_storage_types::<u64, ()>(&entries[21]);
  assert_map_storage_types::<u32, crate::IngressOverflowEventOf<Test>>(&entries[22]);
  assert_plain_storage_type::<u32>(&entries[23]);
  assert_plain_storage_type::<u32>(&entries[24]);
  assert_plain_storage_type::<u32>(&entries[25]);
  assert_plain_storage_type::<MockBlockNumber>(&entries[26]);
}

fn ordinary_transfer_to_aaa(
  origin: RuntimeOrigin,
  aaa_id: u64,
  asset: TestAsset,
  amount: u128,
) -> polkadot_sdk::sp_runtime::DispatchResult {
  let source = frame_system::ensure_signed(origin)?;
  let instance = AAA::aaa_instances(aaa_id).ok_or(Error::<Test>::AaaNotFound)?;
  AAA::preflight_funding_event(
    aaa_id,
    asset,
    amount,
    Some(&crate::FundingProvenance::Signed(source)),
  )?;
  MockAssetOps::transfer(&source, &instance.sovereign_account, asset, amount)?;
  AAA::notify_address_event(aaa_id, asset, amount, &source)?;
  Ok(())
}

fn manual_schedule() -> RuntimeSchedule {
  Schedule {
    trigger: Trigger::Manual,
    cooldown_blocks: 0,
  }
}

fn on_address_event_schedule(
  source_filter: RuntimeSourceFilter,
  asset_filter: RuntimeAssetFilter,
) -> RuntimeSchedule {
  Schedule {
    trigger: Trigger::OnAddressEvent {
      source_filter,
      asset_filter,
    },
    cooldown_blocks: 0,
  }
}

fn timer_schedule(every_blocks: u32) -> RuntimeSchedule {
  Schedule {
    trigger: Trigger::Timer { every_blocks },
    cooldown_blocks: 0,
  }
}

fn make_step(task: RuntimeTask) -> RuntimeStep {
  StepOf::<Test> {
    conditions: BoundedVec::default(),
    task,
    on_error: StepErrorPolicy::AbortCycle,
  }
}

fn inert_execution_plan() -> crate::ExecutionPlanOf<Test> {
  execution_plan_with_step(make_step(Task::Stake {
    asset: TestAsset::Native,
    amount: AmountResolution::Fixed(0),
  }))
}

fn execution_plan_with_step(step: RuntimeStep) -> crate::ExecutionPlanOf<Test> {
  BoundedVec::try_from(vec![step]).expect("execution_plan must fit")
}

fn transfer_execution_plan(to: AccountId, amount: Balance) -> crate::ExecutionPlanOf<Test> {
  execution_plan_with_step(make_step(Task::Transfer {
    to,
    asset: TestAsset::Native,
    amount: AmountResolution::Fixed(amount),
  }))
}

fn user_active_program(
  schedule: RuntimeSchedule,
  schedule_window: Option<ScheduleWindow<u64>>,
  execution_plan: crate::ExecutionPlanOf<Test>,
) -> crate::ProgramInputOf<Test> {
  ProgramInput::Active {
    schedule,
    schedule_window,
    execution_plan,
    on_close_execution_plan: Default::default(),
    funding_source_policy: FundingSourcePolicy::OwnerOnly,
  }
}

fn create_user_with(
  owner: AccountId,
  mutability: Mutability,
  schedule: RuntimeSchedule,
  schedule_window: Option<ScheduleWindow<u64>>,
  execution_plan: crate::ExecutionPlanOf<Test>,
) -> u64 {
  let id = AAA::next_aaa_id();
  assert_ok!(AAA::create_user_aaa(
    RuntimeOrigin::signed(owner),
    mutability,
    user_active_program(schedule, schedule_window, execution_plan),
  ));
  id
}

fn create_user_with_slot(
  owner: AccountId,
  owner_slot: u8,
  mutability: Mutability,
  schedule: RuntimeSchedule,
  schedule_window: Option<ScheduleWindow<u64>>,
  execution_plan: crate::ExecutionPlanOf<Test>,
) -> u64 {
  let id = AAA::next_aaa_id();
  assert_ok!(AAA::create_user_aaa_at_slot(
    RuntimeOrigin::signed(owner),
    owner_slot,
    mutability,
    user_active_program(schedule, schedule_window, execution_plan),
  ));
  id
}

fn create_system_with(
  owner: AccountId,
  schedule: RuntimeSchedule,
  schedule_window: Option<ScheduleWindow<u64>>,
  execution_plan: crate::ExecutionPlanOf<Test>,
) -> u64 {
  let id = AAA::next_aaa_id();
  assert_ok!(AAA::create_system_aaa(
    RuntimeOrigin::root(),
    owner,
    Mutability::Mutable,
    system_active_program(schedule, schedule_window, execution_plan),
  ));
  id
}

fn system_active_program(
  schedule: RuntimeSchedule,
  schedule_window: Option<ScheduleWindow<u64>>,
  execution_plan: crate::ExecutionPlanOf<Test>,
) -> crate::ProgramInputOf<Test> {
  ProgramInput::Active {
    schedule,
    schedule_window,
    execution_plan,
    on_close_execution_plan: Default::default(),
    funding_source_policy: FundingSourcePolicy::RuntimePolicy,
  }
}

fn reopen_system_with_id(
  aaa_id: u64,
  owner: AccountId,
  schedule: RuntimeSchedule,
  schedule_window: Option<ScheduleWindow<u64>>,
  execution_plan: crate::ExecutionPlanOf<Test>,
) {
  assert_ok!(AAA::reopen_system_aaa(
    RuntimeOrigin::root(),
    aaa_id,
    owner,
    Mutability::Mutable,
    system_active_program(schedule, schedule_window, execution_plan),
  ));
}

fn actor_funding(aaa_id: u64) -> crate::ActorFundingStateOf<Test> {
  AAA::actor_funding(aaa_id).expect("active actor funding exists")
}

fn sovereign_account(aaa_id: u64) -> AccountId {
  AAA::aaa_instances(aaa_id)
    .map(|inst| inst.sovereign_account)
    .expect("AAA must exist")
}

fn fund_native(aaa_id: u64, amount: Balance) {
  let aaa_acc = sovereign_account(aaa_id);
  let _ = <Balances as frame::traits::Currency<AccountId>>::deposit_creating(&aaa_acc, amount);
}

fn native_balance(who: &AccountId) -> Balance {
  <Balances as Currency<AccountId>>::free_balance(who)
}

fn asset_balance(who: &AccountId, asset: TestAsset) -> Balance {
  MockAssetOps::balance(who, asset)
}

fn set_asset_balance(who: &AccountId, asset: TestAsset, amount: Balance) {
  MockAssetOps::mint(who, asset, amount).expect("mint must succeed");
}

fn set_native_transfer_lock(who: &AccountId, amount: Balance) {
  <Balances as LockableCurrency<AccountId>>::set_lock(
    *b"aaalock0",
    who,
    amount,
    WithdrawReasons::TRANSFER,
  );
}

fn setup_pool(asset_a: TestAsset, asset_b: TestAsset, reserve_a: Balance, reserve_b: Balance) {
  crate::mock::set_pool_reserves(asset_a, asset_b, reserve_a, reserve_b);
}

fn fund_native_raw(who: &AccountId, amount: Balance) {
  let _ = <Balances as frame::traits::Currency<AccountId>>::deposit_creating(who, amount);
}

fn run_idle(weight: Weight) {
  let now = frame_system::Pallet::<Test>::block_number();
  AAA::on_idle(now, weight);
}

fn starvation_observation_weight() -> Weight {
  <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_on_idle_base()
}

fn run_idle_until_cycle_nonce(aaa_id: u64, target_cycle_nonce: u64) {
  for _ in 0..4 {
    run_idle(Weight::MAX);
    if AAA::aaa_instances(aaa_id)
      .map(|instance| instance.cycle_nonce >= target_cycle_nonce)
      .unwrap_or(false)
    {
      return;
    }
  }
  panic!("cycle nonce did not reach target");
}

fn has_aaa_event(predicate: impl Fn(&Event<Test>) -> bool) -> bool {
  frame_system::Pallet::<Test>::events()
    .into_iter()
    .filter_map(|record| match record.event {
      RuntimeEvent::AAA(event) => Some(event),
      _ => None,
    })
    .any(|event| predicate(&event))
}

#[test]
fn create_user_charges_creation_fee_and_emits_event() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let fee = TestAaaCreationFee::get();
    let fee_sink = TestFeeSink::get();
    let owner_before = native_balance(&ALICE);
    let sink_before = native_balance(&fee_sink);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    let inst = AAA::aaa_instances(aaa_id).expect("AAA must exist");
    assert_eq!(inst.actor_class, ActorClass::User { owner_slot: 0 });
    assert_eq!(native_balance(&ALICE), owner_before.saturating_sub(fee));
    assert_eq!(native_balance(&fee_sink), sink_before.saturating_add(fee));
    assert_eq!(OwnerSlotMask::<Test>::get(ALICE), 0b0000_0001);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::AaaCreated {
          aaa_id: id,
          owner,
          owner_slot: 0,
          aaa_type: AaaType::User,
          mutability: Mutability::Mutable,
          ..
        } if *id == aaa_id && *owner == ALICE
      )
    }));
  });
}

#[test]
fn create_system_does_not_charge_creation_fee() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let owner_before = native_balance(&ALICE);
    let _ = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    assert_eq!(native_balance(&ALICE), owner_before);
  });
}

#[test]
fn system_creation_accepts_dormant_program_input() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(AAA::create_system_aaa(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      ProgramInput::Dormant,
    ));
    let identity = AAA::dormant_aaa_identities(0).expect("dormant identity exists");
    assert_eq!(identity.actor_class, ActorClass::System);
    assert!(AAA::aaa_instances(0).is_none());
    assert_eq!(AAA::actor_identity_count(), 1);
    assert_eq!(AAA::active_aaa_count(), 0);
  });
}

#[test]
fn exact_slot_user_creation_accepts_dormant_program_input() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let owner_slot = 2;
    assert_ok!(AAA::create_user_aaa_at_slot(
      RuntimeOrigin::signed(ALICE),
      owner_slot,
      Mutability::Mutable,
      ProgramInput::Dormant,
    ));
    let identity = AAA::dormant_aaa_identities(0).expect("dormant identity exists");
    assert_eq!(identity.actor_class, ActorClass::User { owner_slot });
    assert!(AAA::aaa_instances(0).is_none());
    assert_eq!(AAA::actor_identity_count(), 1);
    assert_eq!(AAA::active_aaa_count(), 0);
  });
}

#[test]
fn dormant_identity_owns_no_scheduler_state_and_round_trips_activation() {
  new_test_ext().execute_with(|| {
    use polkadot_sdk::frame_support::traits::{Currency, Hooks};
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(AAA::create_user_aaa(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      ProgramInput::Dormant,
    ));
    let aaa_id = 0;
    let identity = AAA::dormant_aaa_identities(aaa_id).expect("dormant identity exists");
    assert_eq!(AAA::actor_identity_count(), 1);
    assert_eq!(AAA::active_aaa_count(), 0);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(!crate::AddressEventInbox::<Test>::contains_key(aaa_id));
    assert!(!crate::ScheduledWakeupBlock::<Test>::contains_key(aaa_id));
    assert!(!crate::WakeupRetryPending::<Test>::get(aaa_id));
    System::reset_events();
    for block in 2..=5 {
      System::set_block_number(block);
      let _ = <AAA as Hooks<MockBlockNumber>>::on_idle(block, Weight::MAX);
    }
    assert!(System::events().iter().all(|record| !matches!(
      record.event,
      RuntimeEvent::AAA(Event::CycleStarted { aaa_id: id, .. })
        | RuntimeEvent::AAA(Event::CycleSummary { aaa_id: id, .. }) if id == aaa_id
    )));
    let preserved = 777;
    let _ =
      <Balances as Currency<AccountId>>::deposit_creating(&identity.sovereign_account, preserved);
    assert_noop!(
      AAA::activate_aaa(RuntimeOrigin::signed(ALICE), aaa_id, ProgramInput::Dormant,),
      Error::<Test>::EmptyExecutionPlan
    );
    assert!(AAA::dormant_aaa_identities(aaa_id).is_some());
    assert_eq!(AAA::active_aaa_count(), 0);
    assert_noop!(
      AAA::activate_aaa(
        RuntimeOrigin::signed(ALICE),
        aaa_id,
        ProgramInput::Active {
          schedule: manual_schedule(),
          schedule_window: None,
          execution_plan: transfer_execution_plan(BOB, 10),
          on_close_execution_plan: execution_plan_with_step(make_step(Task::Mint {
            asset: TestAsset::Native,
            amount: AmountResolution::Fixed(1),
          })),
          funding_source_policy: FundingSourcePolicy::AnySource,
        },
      ),
      Error::<Test>::MintNotAllowedForUserAaa
    );
    assert!(AAA::dormant_aaa_identities(aaa_id).is_some());
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert_eq!(AAA::active_aaa_count(), 0);
    assert_ok!(AAA::activate_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      ProgramInput::Active {
        schedule: manual_schedule(),
        schedule_window: None,
        execution_plan: transfer_execution_plan(BOB, 10),
        on_close_execution_plan: Default::default(),
        funding_source_policy: FundingSourcePolicy::AnySource,
      },
    ));
    assert!(AAA::dormant_aaa_identities(aaa_id).is_none());
    let activated = AAA::aaa_instances(aaa_id).expect("active program exists");
    assert!(activated.on_close_execution_plan.is_empty());
    assert_eq!(
      actor_funding(aaa_id).funding_source_policy,
      FundingSourcePolicy::AnySource
    );
    assert_eq!(AAA::actor_identity_count(), 1);
    assert_eq!(AAA::active_aaa_count(), 1);
    assert_ok!(AAA::deactivate_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(AAA::actor_funding(aaa_id).is_none());
    assert!(AAA::dormant_aaa_identities(aaa_id).is_some());
    assert_eq!(AAA::actor_identity_count(), 1);
    assert_eq!(AAA::active_aaa_count(), 0);
    assert_eq!(native_balance(&identity.sovereign_account), preserved);
    assert!(!crate::AddressEventInbox::<Test>::contains_key(aaa_id));
    assert!(!crate::ScheduledWakeupBlock::<Test>::contains_key(aaa_id));
    assert!(!crate::WakeupRetryPending::<Test>::get(aaa_id));
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert!(AAA::dormant_aaa_identities(aaa_id).is_none());
    assert_eq!(AAA::actor_identity_count(), 0);
    assert_eq!(AAA::owner_slot_mask(ALICE), 0);
    assert_eq!(native_balance(&identity.sovereign_account), preserved);
  });
}

#[test]
fn cycle_bounds_cache_is_initialized_on_create() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    let inst = AAA::aaa_instances(aaa_id).expect("AAA must exist");
    assert_eq!(
      inst.cycle_weight_upper,
      AAA::compute_cycle_weight_upper(inst.actor_class.aaa_type(), &inst.execution_plan)
    );
    assert_eq!(
      inst.cycle_fee_upper,
      AAA::compute_cycle_fee_upper(inst.actor_class.aaa_type(), &inst.execution_plan)
    );
  });
}

#[test]
fn create_admission_enforces_both_idle_weight_dimensions_before_charging() {
  new_test_ext().execute_with(|| {
    let execution_plan = transfer_execution_plan(BOB, 10);
    let on_close_execution_plan = AAA::default_on_close_execution_plan();
    let required = AAA::execution_plan_admission_weight_upper(
      AaaType::User,
      &execution_plan,
      &on_close_execution_plan,
    );
    set_guaranteed_on_idle_weight(required);
    assert_ok!(AAA::create_user_aaa(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_program(manual_schedule(), None, execution_plan.clone()),
    ));
    let owner_before = native_balance(&BOB);
    set_guaranteed_on_idle_weight(Weight::from_parts(
      required.ref_time(),
      required.proof_size().saturating_sub(1),
    ));
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(BOB),
        Mutability::Mutable,
        user_active_program(manual_schedule(), None, execution_plan),
      ),
      Error::<Test>::ExecutionPlanExceedsOnIdleBudget
    );
    assert_eq!(native_balance(&BOB), owner_before);
  });
}

#[test]
fn plan_updates_reject_a_prospective_pair_above_the_idle_budget() {
  new_test_ext().execute_with(|| {
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    let before = AAA::aaa_instances(aaa_id).expect("AAA exists");
    let replacement = transfer_execution_plan(BOB, 10);
    let required = AAA::execution_plan_admission_weight_upper(
      AaaType::User,
      &replacement,
      &before.on_close_execution_plan,
    );
    set_guaranteed_on_idle_weight(Weight::from_parts(
      required.ref_time(),
      required.proof_size().saturating_sub(1),
    ));
    assert_noop!(
      AAA::update_execution_plan(RuntimeOrigin::signed(ALICE), aaa_id, replacement),
      Error::<Test>::ExecutionPlanExceedsOnIdleBudget
    );
    assert_eq!(AAA::aaa_instances(aaa_id), Some(before.clone()));
    let replacement_close = transfer_execution_plan(BOB, 10);
    let required = AAA::execution_plan_admission_weight_upper(
      AaaType::User,
      &before.execution_plan,
      &replacement_close,
    );
    set_guaranteed_on_idle_weight(Weight::from_parts(
      required.ref_time(),
      required.proof_size().saturating_sub(1),
    ));
    assert_noop!(
      AAA::update_on_close_execution_plan(RuntimeOrigin::signed(ALICE), aaa_id, replacement_close,),
      Error::<Test>::ExecutionPlanExceedsOnIdleBudget
    );
    assert_eq!(AAA::aaa_instances(aaa_id), Some(before));
  });
}

#[test]
fn user_cycle_weight_includes_two_fee_collections_per_step() {
  new_test_ext().execute_with(|| {
    let execution_plan = inert_execution_plan();
    let system_weight = AAA::compute_cycle_weight_upper(AaaType::System, &execution_plan);
    let user_weight = AAA::compute_cycle_weight_upper(AaaType::User, &execution_plan);
    assert_eq!(
      user_weight,
      system_weight.saturating_add(<() as crate::WeightInfo>::fee_collection().saturating_mul(2))
    );
  });
}

#[test]
fn cycle_bounds_cache_refreshes_on_update_execution_plan() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    let before = AAA::aaa_instances(aaa_id).expect("AAA must exist");
    let replacement = inert_execution_plan();
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      replacement
    ));
    let after = AAA::aaa_instances(aaa_id).expect("AAA must exist");
    assert_eq!(
      after.cycle_weight_upper,
      AAA::compute_cycle_weight_upper(after.actor_class.aaa_type(), &after.execution_plan)
    );
    assert_eq!(
      after.cycle_fee_upper,
      AAA::compute_cycle_fee_upper(after.actor_class.aaa_type(), &after.execution_plan)
    );
    assert!(
      after.cycle_weight_upper.all_lte(before.cycle_weight_upper),
      "lower-weight execution_plan must not increase the cached weight bound"
    );
  });
}

#[test]
fn canonical_instance_readiness_state_tracks_lifecycle_and_schedule() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    let initial = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert_eq!(initial.actor_class.aaa_type(), AaaType::User);
    assert!(matches!(initial.schedule.trigger, Trigger::Manual));
    assert_eq!(initial.lifecycle, ActiveLifecycle::Active);
    assert!(!initial.manual_trigger_pending);
    assert_eq!(initial.cycle_nonce, 0);
    fund_native(aaa_id, 1_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    assert!(
      AAA::aaa_instances(aaa_id)
        .expect("AAA exists")
        .manual_trigger_pending
    );
    run_idle(Weight::MAX);
    let after_cycle = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert!(!after_cycle.manual_trigger_pending);
    assert_eq!(after_cycle.cycle_nonce, 1);
    assert_ok!(AAA::pause_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert_eq!(
      AAA::aaa_instances(aaa_id).expect("AAA exists").lifecycle,
      ActiveLifecycle::Paused(PauseReason::Manual)
    );
    let timer_schedule = Schedule {
      trigger: Trigger::Timer { every_blocks: 3 },
      cooldown_blocks: 2,
    };
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(AAA::resume_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert_ok!(AAA::update_schedule(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      timer_schedule,
      None,
    ));
    let after_update = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert_eq!(after_update.schedule.cooldown_blocks, 2);
    assert!(matches!(
      after_update.schedule.trigger,
      Trigger::Timer { every_blocks: 3 }
    ));
  });
}

#[test]
fn owner_slot_capacity_is_enforced() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_slots = <<Test as crate::Config>::MaxOwnerSlots as Get<u8>>::get() as u64;
    for _ in 0..max_slots {
      let _ = create_user_with(
        ALICE,
        Mutability::Mutable,
        manual_schedule(),
        None,
        transfer_execution_plan(BOB, 1),
      );
    }
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_program(manual_schedule(), None, transfer_execution_plan(BOB, 1)),
      ),
      Error::<Test>::OwnerSlotCapacityExceeded
    );
  });
}

#[test]
fn active_actor_capacity_is_enforced() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(AAA::set_active_actor_limit(RuntimeOrigin::root(), 1));
    let _first = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    assert_noop!(
      AAA::create_system_aaa(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_program(manual_schedule(), None, transfer_execution_plan(BOB, 1)),
      ),
      Error::<Test>::ActiveAaaCapacityExceeded
    );
  });
}

#[test]
fn system_aaa_count_is_not_limited_by_owner_slots() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let attempts = <<Test as crate::Config>::MaxOwnerSlots as Get<u8>>::get() as u64 + 2;
    let mut sovereign_accounts: Vec<AccountId> = Vec::new();
    for _ in 0..attempts {
      let aaa_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_execution_plan(BOB, 1),
      );
      let inst = AAA::aaa_instances(aaa_id).expect("AAA exists");
      assert_eq!(inst.actor_class, ActorClass::System);
      sovereign_accounts.push(inst.sovereign_account);
    }
    assert_eq!(OwnerSlotMask::<Test>::get(ALICE), 0);
    for i in 0..sovereign_accounts.len() {
      for j in i + 1..sovereign_accounts.len() {
        assert_ne!(sovereign_accounts[i], sovereign_accounts[j]);
      }
    }
  });
}

#[test]
fn actor_class_separates_user_slots_from_system_actors() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let user_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    assert_eq!(
      AAA::aaa_instances(user_id)
        .expect("user AAA exists")
        .actor_class,
      ActorClass::User { owner_slot: 0 }
    );
    assert_eq!(OwnerSlotMask::<Test>::get(ALICE), 0b0000_0001);
    let system_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let system = AAA::aaa_instances(system_id).expect("system AAA exists");
    assert_eq!(system.actor_class, ActorClass::System);
    assert_eq!(OwnerSlotMask::<Test>::get(ALICE), 0b0000_0001);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::AaaCreated {
          aaa_id,
          owner,
          owner_slot,
          aaa_type: AaaType::System,
          ..
        } if *aaa_id == system_id
          && *owner == ALICE
          && *owner_slot == SYSTEM_OWNER_SLOT_SENTINEL
      )
    }));
  });
}

#[test]
fn reopen_system_aaa_reuses_same_sovereign_after_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let first_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let first = AAA::aaa_instances(first_id).expect("system AAA exists");
    let original_sovereign = first.sovereign_account;
    let _ = Balances::deposit_creating(&original_sovereign, 777);
    assert_ok!(AAA::close_aaa(RuntimeOrigin::root(), first_id));
    assert_eq!(AAA::next_aaa_id(), first_id + 1);
    reopen_system_with_id(
      first_id,
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let reopened = AAA::aaa_instances(first_id).expect("reopened system AAA exists");
    assert_eq!(reopened.sovereign_account, original_sovereign);
    assert_eq!(native_balance(&original_sovereign), 777);
    assert_eq!(AAA::next_aaa_id(), first_id + 1);
  });
}

#[test]
fn reopen_system_aaa_accepts_dormant_program_input() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let sovereign = sovereign_account(aaa_id);
    assert_ok!(AAA::close_aaa(RuntimeOrigin::root(), aaa_id));
    assert_ok!(AAA::reopen_system_aaa(
      RuntimeOrigin::root(),
      aaa_id,
      ALICE,
      Mutability::Mutable,
      ProgramInput::Dormant,
    ));
    let identity = AAA::dormant_aaa_identities(aaa_id).expect("dormant identity exists");
    assert_eq!(identity.sovereign_account, sovereign);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(!crate::ClosedSystemAaaIds::<Test>::contains_key(aaa_id));
  });
}

#[test]
fn reopen_system_aaa_rejects_immutable_target_for_mutable_tombstone() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    assert_ok!(AAA::close_aaa(RuntimeOrigin::root(), aaa_id));
    assert_eq!(
      crate::ClosedSystemAaaIds::<Test>::get(aaa_id),
      Some(Mutability::Mutable)
    );
    assert_noop!(
      AAA::reopen_system_aaa(
        RuntimeOrigin::root(),
        aaa_id,
        ALICE,
        Mutability::Immutable,
        system_active_program(manual_schedule(), None, transfer_execution_plan(BOB, 1)),
      ),
      Error::<Test>::ImmutableAaa
    );
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert_eq!(
      crate::ClosedSystemAaaIds::<Test>::get(aaa_id),
      Some(Mutability::Mutable)
    );
  });
}

#[test]
fn reopen_system_aaa_rejects_id_that_was_not_closed_system_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_noop!(
      AAA::reopen_system_aaa(
        RuntimeOrigin::root(),
        42,
        ALICE,
        Mutability::Mutable,
        system_active_program(manual_schedule(), None, transfer_execution_plan(BOB, 1)),
      ),
      Error::<Test>::SystemAaaNotClosed
    );
    let active_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    assert_noop!(
      AAA::reopen_system_aaa(
        RuntimeOrigin::root(),
        active_id,
        ALICE,
        Mutability::Mutable,
        system_active_program(manual_schedule(), None, transfer_execution_plan(BOB, 1)),
      ),
      Error::<Test>::AaaIdOccupied
    );
  });
}

#[test]
fn owner_slot_reused_after_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let id0 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let id1 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let slot0 = AAA::aaa_instances(id0)
      .expect("id0 exists")
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    let slot1 = AAA::aaa_instances(id1)
      .expect("id1 exists")
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    assert_eq!(slot0, 0);
    assert_eq!(slot1, 1);
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), id0));
    let id2 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let slot2 = AAA::aaa_instances(id2)
      .expect("id2 exists")
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    assert_eq!(slot2, slot0);
    assert!(AAA::aaa_instances(id0).is_none());
  });
}

#[test]
fn create_user_at_slot_reuses_same_sovereign_after_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let target_slot = 3;
    let first_id = create_user_with_slot(
      ALICE,
      target_slot,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let first = AAA::aaa_instances(first_id).expect("first AAA exists");
    assert_eq!(first.actor_class.owner_slot(), Some(target_slot));
    assert_eq!(
      first.sovereign_account,
      AAA::sovereign_account_id(&ALICE, target_slot)
    );
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), first_id));
    let second_id = create_user_with_slot(
      ALICE,
      target_slot,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let second = AAA::aaa_instances(second_id).expect("second AAA exists");
    assert_eq!(second.actor_class.owner_slot(), Some(target_slot));
    assert_eq!(second.sovereign_account, first.sovereign_account);
  });
}

#[test]
fn create_user_at_slot_fails_when_requested_slot_is_occupied() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let target_slot = 2;
    let _first = create_user_with_slot(
      ALICE,
      target_slot,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    assert_noop!(
      AAA::create_user_aaa_at_slot(
        RuntimeOrigin::signed(ALICE),
        target_slot,
        Mutability::Mutable,
        user_active_program(manual_schedule(), None, transfer_execution_plan(BOB, 1)),
      ),
      Error::<Test>::OwnerSlotOccupied
    );
  });
}

#[test]
fn create_user_at_slot_fails_when_requested_slot_is_out_of_range() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let invalid_slot = <<Test as crate::Config>::MaxOwnerSlots as Get<u8>>::get();
    assert_noop!(
      AAA::create_user_aaa_at_slot(
        RuntimeOrigin::signed(ALICE),
        invalid_slot,
        Mutability::Mutable,
        user_active_program(manual_schedule(), None, transfer_execution_plan(BOB, 1)),
      ),
      Error::<Test>::InvalidOwnerSlot
    );
  });
}

#[test]
fn close_aaa_emits_owner_initiated_reason() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let active_before = AAA::active_aaa_count();
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    assert_eq!(AAA::active_aaa_count(), active_before + 1);
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert_eq!(AAA::active_aaa_count(), active_before);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::AaaClosed {
          aaa_id: id,
          reason: CloseReason::OwnerInitiated,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn create_atomicity_checkpoint_failure_rolls_back_all_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_fail_create_checkpoint(true);
    let aaa_id = AAA::next_aaa_id();
    let active_before = AAA::active_aaa_count();
    let expected_sovereign = AAA::sovereign_account_id(&ALICE, 0);
    let owner_before = native_balance(&ALICE);
    let sink_before = native_balance(&TestFeeSink::get());
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_program(manual_schedule(), None, transfer_execution_plan(BOB, 1)),
      ),
      DispatchError::Other("AtomicityCreateCheckpointFailed")
    );
    assert_eq!(AAA::next_aaa_id(), aaa_id);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert_eq!(SovereignIndex::<Test>::get(&expected_sovereign), None);
    assert_eq!(OwnerSlotMask::<Test>::get(ALICE), 0);
    assert_eq!(AAA::active_aaa_count(), active_before);
    assert_eq!(ActorHot::<Test>::iter_keys().count() as u32, active_before);
    assert_eq!(native_balance(&ALICE), owner_before);
    assert_eq!(native_balance(&TestFeeSink::get()), sink_before);
  });
}

#[test]
fn creation_fee_route_failure_rolls_back_actor_creation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let active_before = AAA::active_aaa_count();
    let owner_before = native_balance(&ALICE);
    set_fail_fee_sink_transfer(true);
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_program(manual_schedule(), None, transfer_execution_plan(BOB, 1)),
      ),
      Error::<Test>::InsufficientFee
    );
    set_fail_fee_sink_transfer(false);
    assert_eq!(AAA::active_aaa_count(), active_before);
    assert_eq!(native_balance(&ALICE), owner_before);
  });
}

#[test]
fn close_atomicity_checkpoint_failure_rolls_back_all_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let instance_before = AAA::aaa_instances(aaa_id).expect("AAA exists");
    let active_before = AAA::active_aaa_count();
    set_fail_close_checkpoint(true);
    assert_noop!(
      AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id),
      DispatchError::Other("AtomicityCloseCheckpointFailed")
    );
    assert!(AAA::aaa_instances(aaa_id).is_some());
    assert_eq!(
      SovereignIndex::<Test>::get(&instance_before.sovereign_account),
      Some(aaa_id)
    );
    assert!(ActorHot::<Test>::contains_key(aaa_id));
    assert_eq!(AAA::active_aaa_count(), active_before);
    let owner_slot = instance_before
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    assert!(OwnerSlotMask::<Test>::get(ALICE) & (1 << owner_slot) != 0);
    set_fail_close_checkpoint(false);
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert_eq!(AAA::active_aaa_count(), active_before - 1);
  });
}

#[test]
fn create_rejects_empty_whitelist_filter() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let empty_whitelist: BoundedVec<AccountId, <Test as crate::Config>::MaxWhitelistSize> =
      BoundedVec::default();
    let schedule =
      on_address_event_schedule(SourceFilter::Whitelist(empty_whitelist), AssetFilter::Any);
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_program(schedule, None, transfer_execution_plan(BOB, 1)),
      ),
      Error::<Test>::InvalidTriggerConfiguration
    );
  });
}

#[test]
fn whitelist_size_is_bounded_by_runtime_type_limit() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_whitelist = <<Test as crate::Config>::MaxWhitelistSize as Get<u32>>::get() as usize;
    let within_limit = (0..max_whitelist)
      .map(|offset| 50u64.saturating_add(offset as u64))
      .collect::<Vec<_>>();
    let above_limit = (0..max_whitelist.saturating_add(1))
      .map(|offset| 50u64.saturating_add(offset as u64))
      .collect::<Vec<_>>();
    assert!(
      BoundedVec::<AccountId, <Test as crate::Config>::MaxWhitelistSize>::try_from(within_limit)
        .is_ok()
    );
    assert!(
      BoundedVec::<AccountId, <Test as crate::Config>::MaxWhitelistSize>::try_from(above_limit)
        .is_err()
    );
  });
}

#[test]
fn create_rejects_empty_asset_whitelist_filter() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let empty_assets: BoundedVec<TestAsset, <Test as crate::Config>::MaxWhitelistSize> =
      BoundedVec::default();
    let schedule =
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Whitelist(empty_assets));
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_program(schedule, None, transfer_execution_plan(BOB, 1)),
      ),
      Error::<Test>::InvalidTriggerConfiguration
    );
  });
}

#[test]
fn create_rejects_timer_delay_above_max() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_delay = TestMaxExecutionDelayBlocks::get() as u32;
    let schedule = timer_schedule(max_delay.saturating_add(1));
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_program(schedule, None, transfer_execution_plan(BOB, 1)),
      ),
      Error::<Test>::ExecutionDelayTooLong
    );
  });
}

#[test]
fn split_transfer_rejects_share_sum_above_one() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let legs = BoundedVec::try_from(vec![
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(60),
      },
      SplitLeg {
        to: CHARLIE,
        share: Perbill::from_percent(50),
      },
    ])
    .expect("legs fit");
    let execution_plan = execution_plan_with_step(make_step(Task::SplitTransfer {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(100),
      legs,
    }));
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_program(manual_schedule(), None, execution_plan),
      ),
      Error::<Test>::InvalidSplitTransfer
    );
  });
}

#[test]
fn split_transfer_leg_count_is_bounded_by_runtime_type_limit() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_legs = <<Test as crate::Config>::MaxSplitTransferLegs as Get<u32>>::get() as usize;
    let within_limit = (0..max_legs)
      .map(|offset| SplitLeg {
        to: 10u64.saturating_add(offset as u64),
        share: Perbill::from_percent(1),
      })
      .collect::<Vec<_>>();
    let above_limit = (0..max_legs.saturating_add(1))
      .map(|offset| SplitLeg {
        to: 10u64.saturating_add(offset as u64),
        share: Perbill::from_percent(1),
      })
      .collect::<Vec<_>>();
    assert!(SplitTransferLegsOf::<Test>::try_from(within_limit).is_ok());
    assert!(SplitTransferLegsOf::<Test>::try_from(above_limit).is_err());
  });
}

#[test]
fn split_transfer_executes_and_remainder_is_retained() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let total = 101u128;
    let legs = BoundedVec::try_from(vec![
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(50),
      },
      SplitLeg {
        to: CHARLIE,
        share: Perbill::from_percent(50),
      },
    ])
    .expect("legs fit");
    let execution_plan = execution_plan_with_step(make_step(Task::SplitTransfer {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(total),
      legs,
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 1_000);
    let actor = sovereign_account(aaa_id);
    let actor_before = native_balance(&actor);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(native_balance(&CHARLIE), charlie_before.saturating_add(50));
    assert_eq!(native_balance(&actor), actor_before.saturating_sub(100));
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::SplitTransferExecuted {
          aaa_id: id,
          total: emitted_total,
          distributed,
          retained,
          legs: 2,
          effective_legs: 2,
          ..
        } if *id == aaa_id
          && *emitted_total == total
          && *distributed == 100
          && *retained == 1
      )
    }));
  });
}

#[test]
fn on_address_event_owner_filter_is_enforced() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule,
      None,
      transfer_execution_plan(BOB, 10),
    );
    fund_native(aaa_id, 1_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(AAA::notify_address_event(
      aaa_id,
      TestAsset::Native,
      100,
      &BOB
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_ok!(AAA::notify_address_event(
      aaa_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(10));
  });
}

#[test]
fn on_address_event_asset_filter_is_enforced() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_whitelist = BoundedVec::try_from(vec![TestAsset::Local(7)]).expect("fits");
    let schedule =
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Whitelist(asset_whitelist));
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule,
      None,
      transfer_execution_plan(BOB, 10),
    );
    fund_native(aaa_id, 1_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(AAA::notify_address_event(
      aaa_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_ok!(AAA::notify_address_event(
      aaa_id,
      TestAsset::Local(7),
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(10));
  });
}

#[test]
fn on_address_event_without_source_is_ignored_for_owner_filter() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule,
      None,
      transfer_execution_plan(BOB, 10),
    );
    fund_native(aaa_id, 1_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(AAA::notify_address_event_without_source(
      aaa_id,
      TestAsset::Native,
      100
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
  });
}

#[test]
fn manual_trigger_clears_when_cycle_starts() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    fund_native(aaa_id, 1_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    assert!(
      AAA::aaa_instances(aaa_id)
        .expect("AAA exists")
        .manual_trigger_pending
    );
    run_idle(Weight::MAX);
    let inst = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert!(!inst.manual_trigger_pending);
    assert_eq!(inst.cycle_nonce, 1);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::CycleStarted {
          aaa_id: id,
          cycle_nonce: 1,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn manual_trigger_persists_across_pause_resume() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    fund_native(aaa_id, 1_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    assert_ok!(AAA::pause_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert!(
      AAA::aaa_instances(aaa_id)
        .expect("AAA exists")
        .manual_trigger_pending
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(AAA::resume_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert!(
      AAA::aaa_instances(aaa_id)
        .expect("AAA exists")
        .manual_trigger_pending
    );
    run_idle(Weight::MAX);
    let inst = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert!(!inst.manual_trigger_pending);
    assert_eq!(inst.cycle_nonce, 1);
  });
}

#[test]
fn user_pause_resume_churn_is_limited_to_one_queue_mutation_per_block() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    assert_eq!(AAA::queue_tail(), 1);
    assert_ok!(AAA::pause_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert!(
      AAA::actor_hot(aaa_id)
        .expect("paused actor")
        .queue_ticket
        .is_none()
    );
    assert_noop!(
      AAA::resume_aaa(RuntimeOrigin::signed(ALICE), aaa_id),
      Error::<Test>::QueueMutationRateLimited
    );
    assert_eq!(AAA::queue_tail(), 1, "rate-limited resume must not append");

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(AAA::resume_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert_eq!(AAA::queue_tail(), 2);
    assert_noop!(
      AAA::pause_aaa(RuntimeOrigin::signed(ALICE), aaa_id),
      Error::<Test>::QueueMutationRateLimited
    );
    assert_eq!(
      AAA::queue_tail(),
      2,
      "rate-limited pause must not create a tombstone"
    );
  });
}

#[test]
fn manual_trigger_survives_paused_queue_pop_and_resume() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    fund_native(aaa_id, 1_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    assert_ok!(AAA::pause_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    let paused = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert!(paused.manual_trigger_pending);
    assert_eq!(paused.cycle_nonce, 0);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(AAA::resume_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    let resumed = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert!(!resumed.manual_trigger_pending);
    assert_eq!(resumed.cycle_nonce, 1);
  });
}

#[test]
fn manual_trigger_waits_through_cooldown_without_second_signal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::Manual,
      cooldown_blocks: 5,
    };
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule,
      None,
      transfer_execution_plan(BOB, 10),
    );
    fund_native(aaa_id, 2_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(
      AAA::aaa_instances(aaa_id).expect("AAA exists").cycle_nonce,
      1
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(
      AAA::aaa_instances(aaa_id)
        .expect("AAA exists")
        .manual_trigger_pending
    );
    assert_eq!(crate::ScheduledWakeupBlock::<Test>::get(aaa_id), Some(6));
    frame_system::Pallet::<Test>::set_block_number(6);
    run_idle(Weight::MAX);
    let instance = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert_eq!(instance.cycle_nonce, 2);
    assert!(!instance.manual_trigger_pending);
  });
}

#[test]
fn manual_trigger_waits_for_schedule_window_without_second_signal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(ScheduleWindow {
        start: 10,
        end: 110,
      }),
      transfer_execution_plan(BOB, 10),
    );
    fund_native(aaa_id, 1_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(
      AAA::aaa_instances(aaa_id)
        .expect("AAA exists")
        .manual_trigger_pending
    );
    assert_eq!(crate::ScheduledWakeupBlock::<Test>::get(aaa_id), Some(10));
    frame_system::Pallet::<Test>::set_block_number(10);
    run_idle(Weight::MAX);
    assert_eq!(
      AAA::aaa_instances(aaa_id).expect("AAA exists").cycle_nonce,
      1
    );
  });
}

#[test]
fn address_event_waits_through_cooldown_without_second_signal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::OnAddressEvent {
        source_filter: SourceFilter::Any,
        asset_filter: AssetFilter::Any,
      },
      cooldown_blocks: 5,
    };
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule,
      None,
      transfer_execution_plan(BOB, 10),
    );
    fund_native(aaa_id, 2_000);
    assert_ok!(AAA::notify_address_event(
      aaa_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      AAA::aaa_instances(aaa_id).expect("AAA exists").cycle_nonce,
      1
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(AAA::notify_address_event(
      aaa_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert!(AAA::address_event_inbox(aaa_id).is_some());
    assert_eq!(crate::ScheduledWakeupBlock::<Test>::get(aaa_id), Some(6));
    frame_system::Pallet::<Test>::set_block_number(6);
    run_idle(Weight::MAX);
    assert_eq!(
      AAA::aaa_instances(aaa_id).expect("AAA exists").cycle_nonce,
      2
    );
    assert!(AAA::address_event_inbox(aaa_id).is_none());
  });
}

#[test]
fn manual_trigger_is_preserved_on_weight_defer() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(AAA::scheduler_admission_overhead().saturating_add(Weight::from_parts(10, 0)));
    let inst = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert!(inst.manual_trigger_pending);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::CycleDeferred {
          aaa_id: id,
          reason: DeferReason::InsufficientWeightBudget,
        } if *id == aaa_id
      )
    }));
    assert!(!has_aaa_event(|event| matches!(
      event,
      Event::CycleStarted { aaa_id: id, .. } | Event::CycleSummary { aaa_id: id, .. }
        if *id == aaa_id
    )));
  });
}

#[test]
fn manual_trigger_is_preserved_on_proof_size_defer() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let task = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    };
    let aaa_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      execution_plan_with_step(make_step(task)),
    );
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    let instance = AAA::aaa_instances(aaa_id).expect("AAA exists");
    let queue_weight = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1)
      .saturating_add(AAA::scheduler_actor_probe_weight_upper())
      .saturating_add(
        <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
          .max(<<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page()),
      );
    let proof_limit = queue_weight
      .proof_size()
      .saturating_add(instance.cycle_weight_upper.proof_size())
      .saturating_sub(1);
    AAA::execute_cycle(Weight::from_parts(u64::MAX, proof_limit));
    let instance = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert!(instance.manual_trigger_pending);
    assert_eq!(instance.cycle_nonce, 0);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::CycleDeferred {
          aaa_id: id,
          reason: DeferReason::InsufficientWeightBudget,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn queued_actor_is_preserved_when_proof_budget_cannot_admit_probe() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    let scan_weight =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(
        1,
      );
    AAA::execute_cycle(Weight::from_parts(
      u64::MAX,
      scan_weight
        .proof_size()
        .saturating_add(AAA::scheduler_actor_probe_weight_upper().proof_size())
        .saturating_sub(1),
    ));
    let instance = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert!(instance.manual_trigger_pending);
    assert_eq!(instance.cycle_nonce, 0);
    assert!(
      AAA::actor_hot(aaa_id)
        .expect("queued actor")
        .queue_ticket
        .is_some()
    );
  });
}

#[test]
fn cycle_closes_with_fee_budget_exhausted_when_unfunded() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    fund_native(aaa_id, TestMinUserBalance::get());
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::AaaClosed {
          aaa_id: id,
          reason: CloseReason::FeeBudgetExhausted,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn balance_exhausted_takes_precedence_over_fee_budget_exhausted() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    fund_native(aaa_id, TestMinUserBalance::get() - 1);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::AaaClosed {
          aaa_id: id,
          reason: CloseReason::BalanceExhausted,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn fee_insufficiency_is_terminal_without_deferral_guard() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    fund_native(aaa_id, TestMinUserBalance::get());
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(!has_aaa_event(|event| matches!(
      event,
      Event::CycleDeferred { .. }
    )));
  });
}

#[test]
fn evaluation_fee_route_failure_aborts_before_task_execution() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      conditions: vec![Condition::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 0,
      }]
      .try_into()
      .expect("one condition fits"),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      execution_plan_with_step(step),
    );
    fund_native(aaa_id, 1_000_000_000);
    let bob_before = native_balance(&BOB);
    let fee_sink_before = native_balance(&TestFeeSink::get());
    set_fail_fee_sink_transfer(true);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    set_fail_fee_sink_transfer(false);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&TestFeeSink::get()), fee_sink_before);
    assert!(has_aaa_event(|event| {
      matches!(event, Event::StepFailed { aaa_id: id, step_index: 0, .. } if *id == aaa_id)
    }));
  });
}

#[test]
fn execution_fee_route_failure_aborts_before_task_execution() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    fund_native(aaa_id, 1_000_000_000);
    let bob_before = native_balance(&BOB);
    let fee_sink_before = native_balance(&TestFeeSink::get());
    set_fail_fee_sink_transfer(true);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    set_fail_fee_sink_transfer(false);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&TestFeeSink::get()), fee_sink_before);
    assert!(has_aaa_event(|event| {
      matches!(event, Event::StepFailed { aaa_id: id, step_index: 0, .. } if *id == aaa_id)
    }));
  });
}

#[test]
fn consecutive_failures_close_actor_at_inclusive_threshold() {
  new_test_ext().execute_with(|| {
    let threshold = <Test as crate::Config>::MaxConsecutiveFailures::get();
    frame_system::Pallet::<Test>::set_block_number(1);
    let failing_step = StepOf::<Test> {
      conditions: BoundedVec::default(),
      task: Task::SwapExactIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let aaa_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      execution_plan_with_step(failing_step),
    );
    fund_native(aaa_id, 100);
    for cycle in 1..=threshold {
      assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
      run_idle(Weight::MAX);
      if cycle < threshold {
        let inst = AAA::aaa_instances(aaa_id).expect("actor remains before threshold");
        assert_eq!(inst.consecutive_failures, cycle);
        frame_system::Pallet::<Test>::set_block_number((cycle + 1) as u64);
      }
    }
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::AaaClosed {
          aaa_id: id,
          reason: CloseReason::ConsecutiveFailures,
        } if *id == aaa_id
      )
    }));
    let events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::AAA(event) => Some(event),
        _ => None,
      })
      .collect();
    let cycle_summary = events
      .iter()
      .position(|event| {
        matches!(
          event,
          Event::CycleSummary {
            aaa_id: id,
            cycle_nonce,
            ..
          } if *id == aaa_id && *cycle_nonce == u64::from(threshold)
        )
      })
      .expect("terminal cycle summary exists");
    let close_summary = events
      .iter()
      .position(|event| {
        matches!(event, Event::OnCloseExecutionPlanSummary { aaa_id: id, .. } if *id == aaa_id)
      })
      .expect("close-tail summary exists");
    let closed = events
      .iter()
      .position(|event| {
        matches!(
          event,
          Event::AaaClosed {
            aaa_id: id,
            reason: CloseReason::ConsecutiveFailures,
          } if *id == aaa_id
        )
      })
      .expect("terminal close exists");
    assert!(
      cycle_summary < close_summary && close_summary < closed,
      "the admitted cycle must summarize before its close tail and terminal event"
    );
  });
}

#[test]
fn system_immutable_actor_closes_internally_at_failure_threshold_with_close_tail() {
  new_test_ext().execute_with(|| {
    let threshold = <Test as crate::Config>::MaxConsecutiveFailures::get();
    frame_system::Pallet::<Test>::set_block_number(1);
    let failing_step = StepOf::<Test> {
      conditions: BoundedVec::default(),
      task: Task::SwapExactIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let aaa_id = AAA::next_aaa_id();
    assert_ok!(AAA::create_system_aaa(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Immutable,
      system_active_program(
        timer_schedule(1),
        None,
        execution_plan_with_step(failing_step)
      ),
    ));
    assert_noop!(
      AAA::close_aaa(RuntimeOrigin::root(), aaa_id),
      Error::<Test>::ImmutableAaa
    );
    ActorProgram::<Test>::mutate(aaa_id, |maybe| {
      maybe
        .as_mut()
        .expect("immutable System AAA program exists")
        .on_close_execution_plan = transfer_execution_plan(CHARLIE, 7);
    });
    fund_native(aaa_id, 100);
    let charlie_before = native_balance(&CHARLIE);
    for cycle in 1..=threshold {
      frame_system::Pallet::<Test>::set_block_number(u64::from(cycle) + 1);
      run_idle(Weight::MAX);
      if cycle < threshold {
        assert_eq!(
          AAA::aaa_instances(aaa_id)
            .expect("actor remains before threshold")
            .consecutive_failures,
          cycle
        );
      }
    }
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert_eq!(native_balance(&CHARLIE), charlie_before + 7);
    assert_eq!(
      crate::ClosedSystemAaaIds::<Test>::get(aaa_id),
      Some(Mutability::Immutable)
    );
    assert_noop!(
      AAA::reopen_system_aaa(
        RuntimeOrigin::root(),
        aaa_id,
        ALICE,
        Mutability::Mutable,
        system_active_program(timer_schedule(1), None, transfer_execution_plan(BOB, 1)),
      ),
      Error::<Test>::ImmutableAaa
    );
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::AaaClosed {
          aaa_id: id,
          reason: CloseReason::ConsecutiveFailures,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn system_class_not_starved_by_many_user_actors() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let user_count = 32u32;
    for i in 0..user_count {
      let owner: AccountId = 10_000 + i as AccountId;
      let _ = <Balances as frame::traits::Currency<AccountId>>::deposit_creating(
        &owner,
        TEST_INITIAL_BALANCE,
      );
      let user_id = create_user_with(
        owner,
        Mutability::Mutable,
        timer_schedule(1),
        None,
        inert_execution_plan(),
      );
      fund_native(user_id, 1_000);
    }
    let system_id = create_system_with(ALICE, timer_schedule(1), None, inert_execution_plan());
    // With MaxExecutionsPerBlock=3 and mixed User/System contention,
    // run enough blocks for the bounded queue to service the System actor.
    for block in 2..=20 {
      frame_system::Pallet::<Test>::set_block_number(block);
      run_idle(Weight::MAX);
    }
    let system = AAA::aaa_instances(system_id).expect("system AAA exists");
    assert!(
      system.cycle_nonce >= 1,
      "system actor must execute at least once over 20 blocks (nonce={})",
      system.cycle_nonce,
    );
  });
}

#[test]
fn zombie_sweep_cursor_skips_missing_ids_and_reaches_live_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    NextAaaId::<Test>::put(100);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    assert_eq!(aaa_id, 100);
    SweepCursor::<Test>::put(96);
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_some());
    assert_eq!(SweepCursor::<Test>::get(), 99);
    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert_eq!(SweepCursor::<Test>::get(), 1);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::AaaClosed {
          aaa_id: id,
          reason: CloseReason::BalanceExhausted,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn scheduler_falls_back_to_non_empty_class_when_preferred_queue_is_empty() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let system_id = create_system_with(ALICE, timer_schedule(1), None, inert_execution_plan());
    for block in 2..=4 {
      frame_system::Pallet::<Test>::set_block_number(block);
      run_idle(Weight::MAX);
    }
    let system = AAA::aaa_instances(system_id).expect("system AAA exists");
    assert!(system.cycle_nonce >= 1);
  });
}

#[test]
fn execute_cycle_respects_max_executions_per_block() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_exec: u32 = <Test as crate::Config>::MaxExecutionsPerBlock::get();
    let total = max_exec + 2;
    let mut ids = Vec::new();
    for _ in 0..total {
      let id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
      ids.push(id);
      assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), id));
    }
    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let started_block_2 = frame_system::Pallet::<Test>::events()
      .iter()
      .filter(|record| matches!(record.event, RuntimeEvent::AAA(Event::CycleStarted { .. })))
      .count() as u32;
    assert_eq!(started_block_2, max_exec);
    frame_system::Pallet::<Test>::set_block_number(3);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let started_block_3 = frame_system::Pallet::<Test>::events()
      .iter()
      .filter(|record| matches!(record.event, RuntimeEvent::AAA(Event::CycleStarted { .. })))
      .count() as u32;
    assert_eq!(started_block_3, total - max_exec);
  });
}

#[test]
fn wakeup_drain_respects_max_wakeups_per_block() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_wakeups: u32 = <Test as crate::Config>::MaxWakeupsPerBlock::get();
    let total = max_wakeups + 5;
    let mut ids = Vec::new();
    for _ in 0..total {
      let id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
      ids.push(id);
    }
    crate::pallet::WakeupIndex::<Test>::insert(
      1,
      BoundedVec::<u64, <Test as crate::Config>::MaxWakeupBucketSize>::try_from(ids)
        .expect("wakeup batch must fit"),
    );
    crate::pallet::MinWakeupBlock::<Test>::put(1);
    run_idle(Weight::MAX);
    let remaining = crate::pallet::WakeupIndex::<Test>::get(1).len() as u32;
    assert_eq!(remaining, total - max_wakeups);
  });
}

#[test]
fn wakeup_drain_preserves_bucket_when_proof_budget_cannot_admit_it() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    crate::pallet::WakeupIndex::<Test>::insert(
      1,
      BoundedVec::<u64, <Test as crate::Config>::MaxWakeupBucketSize>::try_from(vec![aaa_id])
        .expect("wakeup fits"),
    );
    crate::pallet::ScheduledWakeupBlock::<Test>::insert(aaa_id, 1);
    crate::pallet::MinWakeupBlock::<Test>::put(1);
    run_idle(Weight::from_parts(u64::MAX, 300));
    assert_eq!(
      crate::pallet::WakeupIndex::<Test>::get(1).as_slice(),
      &[aaa_id]
    );
    assert_eq!(
      crate::pallet::ScheduledWakeupBlock::<Test>::get(aaa_id),
      Some(1)
    );
    assert_eq!(crate::pallet::MinWakeupBlock::<Test>::get(), Some(1));
  });
}

#[test]
fn wakeup_drain_caps_sparse_block_scan_per_idle_pass() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(10_000);
    let max_wakeups: u32 = <Test as crate::Config>::MaxWakeupsPerBlock::get();
    crate::pallet::MinWakeupBlock::<Test>::put(1);
    run_idle(Weight::MAX);
    let expected = 1u64.saturating_add(u64::from(max_wakeups));
    assert_eq!(crate::pallet::MinWakeupBlock::<Test>::get(), Some(expected));
  });
}

#[test]
fn paged_enqueue_coalesces_without_a_per_block_insertion_cap() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let total = <<Test as crate::Config>::QueuePageSize as Get<u32>>::get() + 7;
    for _ in 0..total {
      let id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
      assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), id));
    }
    assert_eq!(
      AAA::queue_tail().saturating_sub(AAA::queue_head()),
      u64::from(total)
    );
    assert!(crate::pallet::WakeupIndex::<Test>::get(2).is_empty());
  });
}

#[test]
fn paged_queue_uses_one_live_actor_ticket_and_lazy_invalidation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());

    assert!(AAA::paged_enqueue(aaa_id));
    assert!(AAA::paged_enqueue(aaa_id));
    assert_eq!(AAA::queue_head(), 0);
    assert_eq!(AAA::queue_tail(), 1);
    assert_eq!(
      AAA::actor_hot(aaa_id).expect("hot state").queue_ticket,
      Some(0)
    );
    assert_eq!(AAA::queue_pages(0).expect("head page").len(), 1);

    assert_eq!(AAA::paged_invalidate(aaa_id), Some(0));
    assert_eq!(
      AAA::actor_hot(aaa_id).expect("hot state").queue_ticket,
      None
    );
    assert_eq!(AAA::paged_head_entry(), Some((0, QueueEntry { aaa_id })));
    assert!(AAA::paged_consume_head(0));
    assert_eq!(AAA::queue_head(), 32);
    assert_eq!(AAA::queue_tail(), 32);
    assert!(AAA::queue_pages(0).is_none());
  });
}

#[test]
fn paged_queue_crosses_and_reclaims_page_boundaries_without_prefix_rewrites() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut actors = Vec::new();
    for _ in 0..33 {
      let aaa_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
      assert!(AAA::paged_enqueue(aaa_id));
      actors.push(aaa_id);
    }
    assert_eq!(AAA::queue_tail(), 33);
    assert_eq!(AAA::queue_pages(0).expect("full first page").len(), 32);
    assert_eq!(AAA::queue_pages(1).expect("partial second page").len(), 1);

    for (ticket, aaa_id) in actors.iter().take(32).copied().enumerate() {
      assert_eq!(
        AAA::paged_head_entry(),
        Some((ticket as u64, QueueEntry { aaa_id }))
      );
      assert!(AAA::paged_consume_head(ticket as u64));
    }
    assert_eq!(AAA::queue_head(), 32);
    assert!(AAA::queue_pages(0).is_none());
    assert_eq!(AAA::queue_pages(1).expect("remaining head page").len(), 1);

    assert!(AAA::paged_consume_head(32));
    assert_eq!(AAA::queue_head(), 64);
    assert_eq!(AAA::queue_tail(), 64);
    assert!(AAA::queue_pages(1).is_none());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn paged_queue_replacement_ticket_leaves_old_entry_as_tombstone() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_a = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    let actor_b = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    assert!(AAA::paged_enqueue(actor_a));
    assert_eq!(AAA::paged_invalidate(actor_a), Some(0));
    assert!(AAA::paged_enqueue(actor_b));
    assert!(AAA::paged_enqueue(actor_a));

    assert_eq!(
      AAA::actor_hot(actor_a).expect("actor A hot").queue_ticket,
      Some(2)
    );
    assert_eq!(
      AAA::actor_hot(actor_b).expect("actor B hot").queue_ticket,
      Some(1)
    );
    assert_eq!(
      AAA::paged_head_entry(),
      Some((0, QueueEntry { aaa_id: actor_a }))
    );
    assert!(AAA::paged_consume_head(0));
    assert_eq!(
      AAA::actor_hot(actor_a).expect("actor A hot").queue_ticket,
      Some(2)
    );
    assert_eq!(
      AAA::paged_head_entry(),
      Some((1, QueueEntry { aaa_id: actor_b }))
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn paged_tombstone_drain_is_scan_bounded_and_reclaims_multiple_pages() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut actors = Vec::new();
    for _ in 0..65 {
      let aaa_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
      assert!(AAA::paged_enqueue(aaa_id));
      actors.push(aaa_id);
    }
    for aaa_id in actors {
      assert!(AAA::paged_invalidate(aaa_id).is_some());
    }

    let cutoff = AAA::queue_tail();
    let first = AAA::paged_drain_tombstones(cutoff, 10);
    assert_eq!(first.entries_scanned, 10);
    assert_eq!(first.tombstones_skipped, 10);
    assert_eq!(first.pages_touched, 1);
    assert_eq!(first.pages_deleted, 0);
    assert_eq!(AAA::queue_head(), 10);

    let rest = AAA::paged_drain_tombstones(cutoff, 55);
    assert_eq!(rest.entries_scanned, 55);
    assert_eq!(rest.tombstones_skipped, 55);
    assert_eq!(rest.pages_touched, 3);
    assert_eq!(rest.pages_deleted, 3);
    assert_eq!(AAA::queue_head(), 96);
    assert_eq!(AAA::queue_tail(), 96);
    assert!(AAA::queue_pages(0).is_none());
    assert!(AAA::queue_pages(1).is_none());
    assert!(AAA::queue_pages(2).is_none());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn saturated_tombstone_queue_reclaims_head_before_ingress_and_recovers_deferred_work() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    let page_size = <<Test as crate::Config>::QueuePageSize as Get<u32>>::get();
    let capacity = <<Test as crate::Config>::MaxQueueLength as Get<u32>>::get();
    for page_id in 0..capacity.div_ceil(page_size) {
      let first_ticket = page_id.saturating_mul(page_size);
      let len = page_size.min(capacity.saturating_sub(first_ticket));
      let entries = (0..len)
        .map(|offset| QueueEntry {
          aaa_id: 10_000_000u64
            .saturating_add(u64::from(first_ticket))
            .saturating_add(u64::from(offset)),
        })
        .collect::<Vec<_>>();
      crate::pallet::QueuePages::<Test>::insert(
        u64::from(page_id),
        BoundedVec::try_from(entries).expect("saturated queue page fits"),
      );
    }
    crate::pallet::QueueHead::<Test>::put(0);
    crate::pallet::QueueTail::<Test>::put(u64::from(capacity));

    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    assert_eq!(crate::ScheduledWakeupBlock::<Test>::get(aaa_id), Some(2));
    assert_ok!(AAA::set_global_circuit_breaker(RuntimeOrigin::root(), true));
    let cleanup_budget =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_on_idle_base()
        .saturating_add(
        <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(
          1,
        ),
      );
    AAA::on_idle(1, cleanup_budget);
    assert_eq!(
      AAA::queue_head(),
      1,
      "saturated stale head must make progress before ingress"
    );
    assert_eq!(AAA::queue_tail(), u64::from(capacity));

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(AAA::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      false
    ));
    AAA::on_idle(2, Weight::MAX);
    assert_eq!(
      AAA::aaa_instances(aaa_id)
        .expect("deferred actor survives")
        .cycle_nonce,
      1
    );
    assert_eq!(AAA::queue_head(), AAA::queue_tail());
    assert_eq!(crate::ScheduledWakeupBlock::<Test>::get(aaa_id), None);
  });
}

#[test]
fn paged_tombstone_drain_stops_at_live_head_and_honors_cutoff() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let stale = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    let live = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    let appended_after_cutoff =
      create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    assert!(AAA::paged_enqueue(stale));
    assert!(AAA::paged_enqueue(live));
    let cutoff = AAA::queue_tail();
    assert!(AAA::paged_enqueue(appended_after_cutoff));
    assert_eq!(AAA::paged_invalidate(stale), Some(0));
    assert_eq!(AAA::paged_invalidate(appended_after_cutoff), Some(2));

    let drained = AAA::paged_drain_tombstones(cutoff, 100);
    assert_eq!(drained.entries_scanned, 2);
    assert_eq!(drained.tombstones_skipped, 1);
    assert_eq!(drained.pages_touched, 1);
    assert_eq!(AAA::queue_head(), 1);
    assert_eq!(AAA::queue_tail(), 3);
    assert_eq!(
      AAA::actor_hot(live).expect("live actor").queue_ticket,
      Some(1)
    );

    assert!(AAA::paged_consume_head(1));
    let after_live = AAA::paged_drain_tombstones(cutoff, 100);
    assert_eq!(
      after_live.entries_scanned, 0,
      "ticket 2 is beyond the captured cutoff"
    );
    assert_eq!(AAA::queue_head(), 2);
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn paged_scheduler_preserves_the_unexecuted_fifo_suffix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_exec: u32 = <Test as crate::Config>::MaxExecutionsPerBlock::get();
    let total = max_exec + 2;
    let mut ids = Vec::new();
    for _ in 0..total {
      let aaa_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
      ids.push(aaa_id);
      assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    }
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(AAA::queue_tail().saturating_sub(AAA::queue_head()), 2);
    assert_eq!(
      AAA::paged_head_entry().map(|(_, entry)| entry.aaa_id),
      Some(ids[max_exec as usize])
    );
    frame_system::Pallet::<Test>::set_block_number(3);
    run_idle(Weight::MAX);
    assert_eq!(AAA::queue_head(), AAA::queue_tail());
  });
}

#[test]
fn defer_wakeup_spills_to_later_block_when_requested_bucket_is_full() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_bucket: u32 = <Test as crate::Config>::MaxWakeupBucketSize::get();
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    crate::pallet::QueueTail::<Test>::put(u64::from(
      <<Test as crate::Config>::MaxQueueLength as Get<u32>>::get(),
    ));
    let full_ids: Vec<u64> = (2_000_000..2_000_000 + u64::from(max_bucket)).collect();
    crate::pallet::WakeupIndex::<Test>::insert(
      2,
      BoundedVec::<u64, <Test as crate::Config>::MaxWakeupBucketSize>::try_from(full_ids)
        .expect("full wakeup queue must fit"),
    );
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    assert!(crate::pallet::WakeupIndex::<Test>::get(3).contains(&aaa_id));
    assert_eq!(crate::pallet::WakeupScheduleDrops::<Test>::get(), 0);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::WakeupRescheduled {
          aaa_id: id,
          requested_block: 2,
          scheduled_block: 3,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn defer_wakeup_deduplicates_repeated_manual_trigger_for_same_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    crate::pallet::QueueTail::<Test>::put(u64::from(
      <<Test as crate::Config>::MaxQueueLength as Get<u32>>::get(),
    ));
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    let queued = crate::pallet::WakeupIndex::<Test>::get(2);
    let duplicates = queued
      .iter()
      .filter(|queued_id| **queued_id == aaa_id)
      .count();
    assert_eq!(duplicates, 1);
    assert_eq!(crate::pallet::WakeupScheduleDrops::<Test>::get(), 0);
  });
}

#[test]
fn first_eligible_at_is_the_canonical_initial_timer_anchor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(7);
    let aaa_id = create_system_with(ALICE, timer_schedule(20), None, inert_execution_plan());
    let instance = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert_eq!(
      crate::ScheduledWakeupBlock::<Test>::get(aaa_id),
      Some(instance.first_eligible_at)
    );
    assert!(instance.first_eligible_at >= 27);
  });
}

#[test]
fn dormant_activation_anchors_first_eligibility_at_activation_time() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(AAA::create_system_aaa(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      ProgramInput::Dormant,
    ));
    let aaa_id = 0;
    frame_system::Pallet::<Test>::set_block_number(10);
    assert_ok!(AAA::activate_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      ProgramInput::Active {
        schedule: timer_schedule(20),
        schedule_window: None,
        execution_plan: inert_execution_plan(),
        on_close_execution_plan: BoundedVec::default(),
        funding_source_policy: FundingSourcePolicy::AnySource,
      },
    ));
    let instance = AAA::aaa_instances(aaa_id).expect("active AAA exists");
    assert!(instance.first_eligible_at >= 30);
    assert_eq!(
      crate::ScheduledWakeupBlock::<Test>::get(aaa_id),
      Some(instance.first_eligible_at)
    );
  });
}

#[test]
fn early_reexecution_replaces_live_future_wakeup_instead_of_accumulating() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(ALICE, timer_schedule(20), None, inert_execution_plan());
    let initial_block = crate::pallet::WakeupIndex::<Test>::iter()
      .find_map(|(block, queue)| queue.contains(&aaa_id).then_some(block))
      .expect("timer wakeup should be scheduled");
    assert_eq!(
      crate::pallet::ScheduledWakeupBlock::<Test>::get(aaa_id),
      Some(initial_block)
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    let scheduled_blocks = crate::pallet::WakeupIndex::<Test>::iter()
      .filter_map(|(block, queue)| queue.contains(&aaa_id).then_some(block))
      .collect::<Vec<_>>();
    assert_eq!(
      scheduled_blocks.len(),
      1,
      "live actors must keep only one future wakeup entry"
    );
    let rescheduled_block = scheduled_blocks[0];
    assert_ne!(rescheduled_block, initial_block);
    assert_eq!(
      crate::pallet::ScheduledWakeupBlock::<Test>::get(aaa_id),
      Some(rescheduled_block)
    );
    assert!(
      !crate::pallet::WakeupIndex::<Test>::get(initial_block).contains(&aaa_id),
      "old live wakeup must be replaced on early re-execution"
    );
  });
}

#[test]
fn wakeup_retry_ignores_sparse_historical_id_distance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_bucket: u32 = <Test as crate::Config>::MaxWakeupBucketSize::get();
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, 10),
    );
    fund_native(aaa_id, 1_000);
    crate::pallet::QueueTail::<Test>::put(u64::from(
      <<Test as crate::Config>::MaxQueueLength as Get<u32>>::get(),
    ));
    let full_ids: Vec<u64> = (5_000_000..5_000_000 + u64::from(max_bucket)).collect();
    for block in 2..=10 {
      crate::pallet::WakeupIndex::<Test>::insert(
        block,
        BoundedVec::<u64, <Test as crate::Config>::MaxWakeupBucketSize>::try_from(full_ids.clone())
          .expect("full wakeup queue must fit"),
      );
    }
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(AAA::notify_address_event(
      aaa_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    assert_eq!(crate::pallet::WakeupScheduleDrops::<Test>::get(), 1);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::WakeupScheduleDropped {
          aaa_id: id,
          requested_block: 2,
        } if *id == aaa_id
      )
    }));
    assert!(AAA::address_event_inbox(aaa_id).is_some());
    assert!(crate::pallet::WakeupRetryPending::<Test>::get(aaa_id));
    NextAaaId::<Test>::put(10_000_000);
    SweepCursor::<Test>::put(0);
    crate::pallet::QueueHead::<Test>::put(0);
    crate::pallet::QueueTail::<Test>::put(0);
    for block in 2..=10 {
      crate::pallet::WakeupIndex::<Test>::remove(block);
    }
    crate::pallet::MinWakeupBlock::<Test>::kill();
    let bob_before = native_balance(&BOB);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(!crate::pallet::WakeupRetryPending::<Test>::get(aaa_id));
    assert_eq!(
      crate::pallet::ScheduledWakeupBlock::<Test>::get(aaa_id),
      Some(3)
    );
    frame_system::Pallet::<Test>::set_block_number(3);
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(10));
    assert!(AAA::address_event_inbox(aaa_id).is_none());
    assert_eq!(crate::pallet::WakeupScheduleDrops::<Test>::get(), 1);
  });
}

#[test]
fn close_before_future_wakeup_removes_authoritative_bucket_entry() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(ALICE, timer_schedule(20), None, inert_execution_plan());
    let scheduled_block = crate::pallet::WakeupIndex::<Test>::iter()
      .find_map(|(block, queue)| queue.contains(&aaa_id).then_some(block))
      .expect("timer wakeup should be scheduled");
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(crate::pallet::ScheduledWakeupBlock::<Test>::get(aaa_id).is_none());
    assert!(
      !crate::pallet::WakeupIndex::<Test>::get(scheduled_block).contains(&aaa_id),
      "close must remove the entry from its reverse-indexed future bucket"
    );
    frame_system::Pallet::<Test>::set_block_number(scheduled_block);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(!crate::pallet::WakeupIndex::<Test>::get(scheduled_block).contains(&aaa_id));
    assert_eq!(AAA::queue_head(), AAA::queue_tail());
    assert!(!has_aaa_event(|event| {
      matches!(event, Event::CycleStarted { aaa_id: id, .. } if *id == aaa_id)
    }));
  });
}

#[test]
fn repeated_timer_close_churn_leaves_no_long_horizon_wakeup_entries() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let total = <<Test as crate::Config>::MaxWakeupsPerBlock as Get<u32>>::get() + 2;
    let mut actors = Vec::new();
    let mut latest_wakeup = 1u64;
    for _ in 0..total {
      let aaa_id = create_system_with(ALICE, timer_schedule(4_000), None, inert_execution_plan());
      let wakeup = crate::pallet::ScheduledWakeupBlock::<Test>::get(aaa_id)
        .expect("timer wakeup must be scheduled");
      latest_wakeup = latest_wakeup.max(wakeup);
      actors.push(aaa_id);
    }

    for aaa_id in actors {
      assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
      assert!(crate::pallet::ScheduledWakeupBlock::<Test>::get(aaa_id).is_none());
    }
    assert!(crate::pallet::WakeupIndex::<Test>::iter().next().is_none());
    frame_system::Pallet::<Test>::set_block_number(latest_wakeup.saturating_add(1_000));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(crate::pallet::WakeupIndex::<Test>::iter().next().is_none());
    assert_eq!(AAA::queue_head(), AAA::queue_tail());
    assert!(!has_aaa_event(|event| matches!(
      event,
      Event::CycleStarted { .. }
    )));
  });
}

#[test]
fn absent_schedule_window_never_expires() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    frame_system::Pallet::<Test>::set_block_number(u64::MAX);
    let instance = AAA::aaa_instances(aaa_id).expect("actor remains live");
    assert!(!AAA::is_window_expired(&instance));
  });
}

#[test]
fn expired_ingress_remains_balance_only_and_policy_touch_closes() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let window = ScheduleWindow { start: 1, end: 101 };
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      Some(window),
      execution_plan_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    let actor = sovereign_account(aaa_id);
    let balance_before = native_balance(&actor);
    frame_system::Pallet::<Test>::set_block_number(102);
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      TestAsset::Native,
      1_000
    ));
    assert_eq!(native_balance(&actor), balance_before.saturating_add(1_000));
    assert!(actor_funding(aaa_id).funding_snapshots.is_empty());
    assert!(AAA::address_event_inbox(aaa_id).is_none());
    let overflow_len = AAA::ingress_overflow_len();
    assert!(AAA::queue_address_event(
      aaa_id,
      TestAsset::Native,
      1,
      Some(crate::FundingProvenance::Signed(ALICE))
    ));
    assert_eq!(AAA::ingress_overflow_len(), overflow_len);
    assert_ok!(AAA::update_funding_source_policy(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      FundingSourcePolicy::AnySource
    ));
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(has_aaa_event(|event| matches!(
      event,
      Event::AaaClosed {
        aaa_id: id,
        reason: CloseReason::WindowExpired,
      } if *id == aaa_id
    )));
  });
}

#[test]
fn window_expired_takes_precedence_over_balance_exhausted() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let window = ScheduleWindow { start: 1, end: 101 };
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(window),
      transfer_execution_plan(BOB, 1),
    );
    frame_system::Pallet::<Test>::set_block_number(102);
    assert_ok!(AAA::permissionless_sweep(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
    ));
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::AaaClosed {
          aaa_id: id,
          reason: CloseReason::WindowExpired,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn immutable_creation_surfaces_fix_close_plan_to_empty() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let user_id = create_user_with(
      ALICE,
      Mutability::Immutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let system_id = AAA::next_aaa_id();
    assert_ok!(AAA::create_system_aaa(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Immutable,
      system_active_program(manual_schedule(), None, transfer_execution_plan(BOB, 1)),
    ));
    let replacement = transfer_execution_plan(CHARLIE, 1);
    for aaa_id in [user_id, system_id] {
      let instance = AAA::aaa_instances(aaa_id).expect("immutable actor exists");
      assert!(instance.on_close_execution_plan.is_empty());
    }
    assert_noop!(
      AAA::update_on_close_execution_plan(
        RuntimeOrigin::signed(ALICE),
        user_id,
        replacement.clone()
      ),
      Error::<Test>::ImmutableAaa
    );
    assert_noop!(
      AAA::update_on_close_execution_plan(RuntimeOrigin::root(), system_id, replacement),
      Error::<Test>::ImmutableAaa
    );
  });
}

#[test]
fn immutable_actor_rejects_pause_and_update_execution_plan() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Immutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    assert_noop!(
      AAA::pause_aaa(RuntimeOrigin::signed(ALICE), aaa_id),
      Error::<Test>::ImmutableAaa
    );
    let replacement = transfer_execution_plan(CHARLIE, 1);
    assert_noop!(
      AAA::update_execution_plan(RuntimeOrigin::signed(ALICE), aaa_id, replacement),
      Error::<Test>::ImmutableAaa
    );
  });
}

#[test]
fn user_actor_rejects_mint_task_on_create() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Mint {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    }));
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_program(manual_schedule(), None, execution_plan),
      ),
      Error::<Test>::MintNotAllowedForUserAaa
    );
  });
}

#[test]
fn update_execution_plan_prunes_stale_funding_snapshots() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let initial_execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      initial_execution_plan,
    );
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      TestAsset::Native,
      100
    ));
    assert!(
      actor_funding(aaa_id)
        .funding_snapshots
        .contains_key(&TestAsset::Native)
    );
    let replacement = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(1),
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      replacement
    ));
    let funding_after = actor_funding(aaa_id);
    assert!(
      !funding_after
        .funding_snapshots
        .contains_key(&TestAsset::Native)
    );
    assert!(
      funding_after
        .funding_tracked_assets
        .contains(&TestAsset::Local(1))
    );
  });
}

#[test]
fn update_execution_plan_rejects_mint_for_user_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let replacement = execution_plan_with_step(make_step(Task::Mint {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    }));
    assert_noop!(
      AAA::update_execution_plan(RuntimeOrigin::signed(ALICE), aaa_id, replacement),
      Error::<Test>::MintNotAllowedForUserAaa
    );
  });
}

#[test]
fn permissionless_sweep_closes_user_below_min_balance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    assert_ok!(AAA::permissionless_sweep(
      RuntimeOrigin::signed(BOB),
      aaa_id
    ));
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::AaaClosed {
          aaa_id: id,
          reason: CloseReason::BalanceExhausted,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn permissionless_sweep_is_lifecycle_touchpoint_only_under_breaker() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    assert_ok!(AAA::set_global_circuit_breaker(RuntimeOrigin::root(), true));
    assert_ok!(AAA::permissionless_sweep(
      RuntimeOrigin::signed(BOB),
      aaa_id
    ));
    let instance = AAA::aaa_instances(aaa_id).expect("system AAA remains alive");
    assert_eq!(instance.cycle_nonce, 0);
    assert_eq!(AAA::queue_head(), AAA::queue_tail());
  });
}

#[test]
fn permissionless_sweep_many_closes_multiple_and_reports_counts() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let user_a = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let user_b = create_user_with(
      BOB,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(ALICE, 1),
    );
    let system_alive = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let sweep_ids: BoundedVec<u64, <Test as crate::Config>::MaxSweepPerBlock> =
      BoundedVec::try_from(vec![user_a, user_b, system_alive]).expect("batch fits");
    assert_ok!(AAA::permissionless_sweep_many(
      RuntimeOrigin::signed(CHARLIE),
      sweep_ids,
    ));
    assert!(AAA::aaa_instances(user_a).is_none());
    assert!(AAA::aaa_instances(user_b).is_none());
    assert!(AAA::aaa_instances(system_alive).is_some());
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::SweepBatchProcessed {
          requested: 3,
          closed: 2,
          alive: 1,
          missing: 0,
        }
      )
    }));
  });
}

#[test]
fn permissionless_sweep_many_ignores_missing_ids() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let user_a = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let missing_id = user_a.saturating_add(10_000);
    let sweep_ids: BoundedVec<u64, <Test as crate::Config>::MaxSweepPerBlock> =
      BoundedVec::try_from(vec![user_a, missing_id]).expect("batch fits");
    assert_ok!(AAA::permissionless_sweep_many(
      RuntimeOrigin::signed(CHARLIE),
      sweep_ids,
    ));
    assert!(AAA::aaa_instances(user_a).is_none());
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::SweepBatchProcessed {
          requested: 2,
          closed: 1,
          alive: 0,
          missing: 1,
        }
      )
    }));
  });
}

#[test]
fn tiny_percentage_amount_is_skipped_without_execution_plan_failure() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfCurrent(Perbill::from_parts(1)),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    let inst = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert_eq!(inst.consecutive_failures, 0);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          aaa_id: id,
          step_index: 0,
          reason: StepSkippedReason::ResolutionSkipped,
          ..
        } if *id == aaa_id
      )
    }));
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          aaa_id: id,
          cycle_nonce: 1,
          executed_steps: 0,
          skipped_conditions: 0,
          skipped_resolution: 1,
          skipped_funding_unavailable: 0,
          failed_steps: 0,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn user_resolution_skip_charges_only_eval_fee() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfCurrent(Perbill::from_parts(1)),
    }));
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      execution_plan,
    );
    let actor = sovereign_account(aaa_id);
    fund_native(aaa_id, 1_000);
    let before = native_balance(&actor);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    let after = native_balance(&actor);
    assert_eq!(after, before.saturating_sub(TestStepBaseFee::get()));
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          aaa_id: id,
          step_index: 0,
          reason: StepSkippedReason::ResolutionSkipped,
          ..
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn executable_task_charges_eval_and_execution_fees() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let task = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    };
    let execution_plan = execution_plan_with_step(make_step(task.clone()));
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      execution_plan,
    );
    let actor = sovereign_account(aaa_id);
    fund_native(aaa_id, 1_000);
    let actor_before = native_balance(&actor);
    let fee_sink_before = native_balance(&TestFeeSink::get());
    let task_weight = AAA::weight_upper_bound(&task);
    assert!(task_weight.ref_time() > 0);
    let expected_fee =
      TestStepBaseFee::get().saturating_add(TestWeightToFee::weight_to_fee(&task_weight));
    assert_eq!(
      AAA::aaa_instances(aaa_id)
        .expect("user aaa")
        .cycle_fee_upper,
      expected_fee
    );
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&actor),
      actor_before.saturating_sub(expected_fee).saturating_sub(1)
    );
    assert_eq!(
      native_balance(&TestFeeSink::get()),
      fee_sink_before.saturating_add(expected_fee)
    );
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          aaa_id: id,
          executed_steps: 1,
          skipped_conditions: 0,
          skipped_resolution: 0,
          skipped_funding_unavailable: 0,
          failed_steps: 0,
          ..
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn cycle_summary_tracks_step_outcomes() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step_conditions = BoundedVec::try_from(vec![Condition::BalanceAbove {
      asset: TestAsset::Native,
      threshold: 1_000,
    }])
    .expect("single condition must fit");
    let execution_plan = BoundedVec::try_from(vec![
      StepOf::<Test> {
        conditions: step_conditions,
        task: Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(10),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfCurrent(Perbill::from_parts(1)),
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      }),
      StepOf::<Test> {
        conditions: BoundedVec::default(),
        task: Task::SwapExactIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::Fixed(10),
          slippage_tolerance: Perbill::one(),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
    ])
    .expect("execution_plan must fit");
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          aaa_id: id,
          cycle_nonce: 1,
          executed_steps: 1,
          skipped_conditions: 1,
          skipped_resolution: 1,
          skipped_funding_unavailable: 0,
          failed_steps: 1,
        } if *id == aaa_id
      )
    }));
    let last_aaa_event = frame_system::Pallet::<Test>::events()
      .into_iter()
      .rev()
      .find_map(|record| match record.event {
        RuntimeEvent::AAA(event) => Some(event),
        _ => None,
      })
      .expect("AAA event stream must not be empty");
    assert!(matches!(
      last_aaa_event,
      Event::CycleSummary { aaa_id: id, cycle_nonce: 1, .. } if id == aaa_id
    ));
  });
}

#[test]
fn cycle_success_predicate_drives_failure_reset_auto_close_and_event_order() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let failing_step = |on_error| StepOf::<Test> {
      conditions: BoundedVec::default(),
      task: Task::SwapExactIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error,
    };
    let continue_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![
        failing_step(StepErrorPolicy::ContinueNextStep),
        failing_step(StepErrorPolicy::ContinueNextStep),
      ])
      .expect("two steps fit"),
    );
    fund_native(continue_id, 100);
    ActorHot::<Test>::mutate(continue_id, |maybe| {
      maybe.as_mut().expect("actor hot state exists").consecutive_failures = 2;
    });
    assert_ok!(AAA::set_auto_close_at_cycle_nonce(
      RuntimeOrigin::root(),
      continue_id,
      Some(2)
    ));
    assert_ok!(AAA::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      continue_id
    ));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let after_first = AAA::aaa_instances(continue_id).expect("successful actor remains active");
    assert_eq!(after_first.cycle_nonce, 1);
    assert_eq!(after_first.consecutive_failures, 0);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(AAA::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      continue_id
    ));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(continue_id).is_none());
    let continue_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::AAA(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(continue_events.len(), 6);
    assert!(matches!(continue_events[0], Event::CycleStarted { aaa_id, .. } if aaa_id == continue_id));
    assert!(matches!(continue_events[1], Event::StepFailed { aaa_id, step_index: 0, .. } if aaa_id == continue_id));
    assert!(matches!(continue_events[2], Event::StepFailed { aaa_id, step_index: 1, .. } if aaa_id == continue_id));
    assert!(matches!(continue_events[3], Event::CycleSummary { aaa_id, failed_steps: 2, .. } if aaa_id == continue_id));
    assert!(matches!(continue_events[4], Event::OnCloseExecutionPlanSummary { aaa_id, .. } if aaa_id == continue_id));
    assert!(matches!(continue_events[5], Event::AaaClosed { aaa_id, reason: CloseReason::AutoCloseNonceReached } if aaa_id == continue_id));
    let skip_step = StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 1,
      }])
      .expect("one condition fits"),
      task: Task::Stake {
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(0),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let skip_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![skip_step]).expect("one step fits"),
    );
    assert_ok!(AAA::set_auto_close_at_cycle_nonce(
      RuntimeOrigin::root(),
      skip_id,
      Some(1)
    ));
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), skip_id));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(skip_id).is_none());
    let skip_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::AAA(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(skip_events.len(), 5);
    assert!(matches!(skip_events[0], Event::CycleStarted { aaa_id, .. } if aaa_id == skip_id));
    assert!(matches!(skip_events[1], Event::StepSkipped { aaa_id, step_index: 0, .. } if aaa_id == skip_id));
    assert!(matches!(skip_events[2], Event::CycleSummary { aaa_id, skipped_conditions: 1, failed_steps: 0, .. } if aaa_id == skip_id));
    assert!(matches!(skip_events[3], Event::OnCloseExecutionPlanSummary { aaa_id, .. } if aaa_id == skip_id));
    assert!(matches!(skip_events[4], Event::AaaClosed { aaa_id, reason: CloseReason::AutoCloseNonceReached } if aaa_id == skip_id));
    let abort_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![
        failing_step(StepErrorPolicy::AbortCycle),
        make_step(Task::Stake {
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(0),
        }),
      ])
      .expect("two steps fit"),
    );
    fund_native(abort_id, 100);
    crate::ActorFunding::<Test>::mutate(abort_id, |maybe| {
      maybe
        .as_mut()
        .expect("abort actor funding")
        .funding_snapshots
        .try_insert(
          TestAsset::Native,
          FundingBatch {
            amount: 100,
            pending_amount: 80,
          },
        )
        .expect("funding batch fits");
    });
    assert_ok!(AAA::set_auto_close_at_cycle_nonce(
      RuntimeOrigin::root(),
      abort_id,
      Some(1)
    ));
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), abort_id));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let abort_instance = AAA::aaa_instances(abort_id).expect("aborted actor remains active");
    assert_eq!(abort_instance.consecutive_failures, 1);
    let abort_funding = actor_funding(abort_id);
    let abort_batch = abort_funding
      .funding_snapshots
      .get(&TestAsset::Native)
      .expect("abort funding batch");
    assert_eq!(abort_batch.amount, 100);
    assert_eq!(abort_batch.pending_amount, 80);
    let abort_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::AAA(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(abort_events.len(), 3);
    assert!(matches!(abort_events[0], Event::CycleStarted { aaa_id, .. } if aaa_id == abort_id));
    assert!(matches!(abort_events[1], Event::StepFailed { aaa_id, step_index: 0, .. } if aaa_id == abort_id));
    assert!(matches!(abort_events[2], Event::CycleSummary { aaa_id, executed_steps: 0, failed_steps: 1, .. } if aaa_id == abort_id));
    let close_only_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(AAA::close_aaa(RuntimeOrigin::root(), close_only_id));
    let close_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::AAA(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(close_events.len(), 2);
    assert!(matches!(close_events[0], Event::OnCloseExecutionPlanSummary { aaa_id, .. } if aaa_id == close_only_id));
    assert!(matches!(close_events[1], Event::AaaClosed { aaa_id, reason: CloseReason::OwnerInitiated } if aaa_id == close_only_id));
  });
}

#[test]
fn cycle_summary_fee_fairness_property_matrix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let cases = [
      (1_000u128, Perbill::from_parts(1), true),
      (1_000u128, Perbill::from_percent(10), false),
      (10_000u128, Perbill::from_percent(50), false),
    ];
    let eval_fee = TestStepBaseFee::get();
    for (idx, (funding, pct, expect_skip)) in cases.into_iter().enumerate() {
      let task = Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfCurrent(pct),
      };
      let execution_plan = execution_plan_with_step(make_step(task.clone()));
      let aaa_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        manual_schedule(),
        None,
        execution_plan,
      );
      fund_native(aaa_id, funding);
      let fee_sink_before = native_balance(&TestFeeSink::get());
      assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
      run_idle(Weight::MAX);
      let fee_sink_after = native_balance(&TestFeeSink::get());
      let fee_delta = fee_sink_after.saturating_sub(fee_sink_before);
      let summary = frame_system::Pallet::<Test>::events()
        .into_iter()
        .rev()
        .find_map(|record| match record.event {
          RuntimeEvent::AAA(Event::CycleSummary {
            aaa_id: id,
            executed_steps,
            skipped_conditions,
            skipped_resolution,
            skipped_funding_unavailable,
            failed_steps,
            ..
          }) if id == aaa_id => Some((
            executed_steps,
            skipped_conditions,
            skipped_resolution,
            skipped_funding_unavailable,
            failed_steps,
          )),
          _ => None,
        })
        .expect("CycleSummary must be emitted");
      if expect_skip {
        assert_eq!(summary.0, 0);
        assert_eq!(summary.1, 0);
        assert_eq!(summary.2, 1);
        assert_eq!(summary.3, 0);
        assert_eq!(summary.4, 0);
        assert_eq!(fee_delta, eval_fee);
      } else {
        let exec_fee = <TestWeightToFee as polkadot_sdk::sp_weights::WeightToFee>::weight_to_fee(
          &AAA::weight_upper_bound(&task),
        );
        assert_eq!(summary.0, 1);
        assert_eq!(summary.1, 0);
        assert_eq!(summary.2, 0);
        assert_eq!(summary.3, 0);
        assert_eq!(summary.4, 0);
        assert_eq!(fee_delta, eval_fee.saturating_add(exec_fee));
      }
      frame_system::Pallet::<Test>::set_block_number((idx as u64).saturating_add(2));
    }
  });
}

#[test]
fn percentage_of_trigger_uses_cycle_start_snapshot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      }),
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfTrigger(Perbill::from_percent(50)),
      }),
    ])
    .expect("execution_plan fits");
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 101);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    let actor = sovereign_account(aaa_id);
    let actor_before = native_balance(&actor);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(native_balance(&CHARLIE), charlie_before.saturating_add(50));
    assert_eq!(native_balance(&actor), actor_before.saturating_sub(100));
  });
}

#[test]
fn percentage_of_trigger_uses_spendable_native_snapshot_for_user() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let task = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfTrigger(Perbill::one()),
    };
    let execution_plan = execution_plan_with_step(make_step(task.clone()));
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      execution_plan,
    );
    let actor = sovereign_account(aaa_id);
    let funding = 500;
    fund_native(aaa_id, funding);
    let expected_fees = TestStepBaseFee::get().saturating_add(
      <TestWeightToFee as polkadot_sdk::sp_weights::WeightToFee>::weight_to_fee(
        &AAA::weight_upper_bound(&task),
      ),
    );
    let expected_transfer = funding.saturating_sub(expected_fees);
    let bob_before = native_balance(&BOB);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(
      native_balance(&actor),
      funding.saturating_sub(TestStepBaseFee::get())
    );
    assert!(expected_transfer > 0);
  });
}

#[test]
fn percentage_of_last_funding_freezes_active_and_accumulates_pending() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let bob_before = native_balance(&BOB);
    let actor = sovereign_account(aaa_id);
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      TestAsset::Native,
      100
    ));
    let funding = actor_funding(aaa_id);
    assert!(!funding.has_pending_funding);
    assert_eq!(
      funding
        .funding_snapshots
        .get(&TestAsset::Native)
        .unwrap()
        .amount,
      100
    );
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle_until_cycle_nonce(aaa_id, 1);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(native_balance(&actor), 50);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(CHARLIE),
      aaa_id,
      TestAsset::Native,
      200
    ));
    assert_eq!(native_balance(&actor), 250);
    let funding = actor_funding(aaa_id);
    let batch = funding
      .funding_snapshots
      .get(&TestAsset::Native)
      .expect("funding batch");
    assert_eq!(batch.amount, 100);
    assert_eq!(batch.pending_amount, 200);
    assert!(funding.has_pending_funding);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle_until_cycle_nonce(aaa_id, 2);
    let inst = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert_eq!(inst.cycle_nonce, 2);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(100));
    let funding = actor_funding(aaa_id);
    let batch = funding
      .funding_snapshots
      .get(&TestAsset::Native)
      .expect("promoted funding batch");
    assert_eq!(batch.amount, 200);
    assert_eq!(batch.pending_amount, 0);
    assert!(!funding.has_pending_funding);
    assert!(has_aaa_event(|event| matches!(
      event,
      Event::FundingBatchPromoted {
        aaa_id: id,
        asset: TestAsset::Native,
        amount: 200,
      } if *id == aaa_id
    )));
  });
}

#[test]
fn system_keeps_running_on_last_funding_exhaustion_and_accepts_refill() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      TestAsset::Native,
      100
    ));
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle_until_cycle_nonce(aaa_id, 1);
    assert_eq!(native_balance(&actor), 50);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle_until_cycle_nonce(aaa_id, 2);
    assert_eq!(native_balance(&actor), 50);
    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle_until_cycle_nonce(aaa_id, 3);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          aaa_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == aaa_id
      )
    }));
    let instance = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert_eq!(instance.lifecycle, ActiveLifecycle::Active);
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(CHARLIE),
      aaa_id,
      TestAsset::Native,
      80
    ));
    let updated = actor_funding(aaa_id);
    let batch = updated
      .funding_snapshots
      .get(&TestAsset::Native)
      .expect("funding batch");
    assert_eq!(batch.amount, 100);
    assert_eq!(batch.pending_amount, 80);
  });
}

#[test]
fn user_keeps_running_on_last_funding_exhaustion() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      execution_plan,
    );
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      TestAsset::Native,
      500
    ));
    let bob_before = native_balance(&BOB);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          aaa_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == aaa_id
      )
    }));
    assert!(AAA::aaa_instances(aaa_id).is_some());
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(250));
  });
}

#[test]
fn create_accepts_swap_exact_in_with_slippage_tolerance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::SwapExactIn {
      asset_in: TestAsset::Native,
      asset_out: TestAsset::Local(1),
      amount_in: AmountResolution::Fixed(10),
      slippage_tolerance: Perbill::from_percent(5),
    }));
    assert_ok!(AAA::create_user_aaa(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_program(manual_schedule(), None, execution_plan),
    ));
  });
}

#[test]
fn create_accepts_swap_exact_out_with_slippage_tolerance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::SwapExactOut {
      asset_in: TestAsset::Local(1),
      asset_out: TestAsset::Local(2),
      amount_out: AmountResolution::Fixed(10),
      slippage_tolerance: Perbill::from_percent(5),
    }));
    assert_ok!(AAA::create_user_aaa(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_program(manual_schedule(), None, execution_plan),
    ));
  });
}

#[test]
fn swap_exact_out_executes_and_emits_swap_event() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_in = TestAsset::Local(1);
    let asset_out = TestAsset::Local(2);
    set_pool_reserves(asset_in, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      execution_plan_with_step(make_step(Task::SwapExactOut {
        asset_in,
        asset_out,
        amount_out: AmountResolution::Fixed(100),
        slippage_tolerance: Perbill::from_percent(0),
      })),
    );
    fund_native(aaa_id, 10_000);
    let sovereign = sovereign_account(aaa_id);
    set_asset_balance(&sovereign, asset_in, 1_000);
    let out_before = asset_balance(&sovereign, asset_out);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    let out_after = asset_balance(&sovereign, asset_out);
    assert!(out_after >= out_before.saturating_add(100));
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::SwapExecuted {
          aaa_id: id,
          asset_in: in_asset,
          asset_out: out_asset,
          amount_out,
          ..
        } if *id == aaa_id && *in_asset == asset_in && *out_asset == asset_out && *amount_out >= 100
      )
    }));
  });
}

#[test]
fn on_initialize_does_not_execute_cycles_after_starvation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    fund_native(aaa_id, 1_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    IdleStarvationBlocks::<Test>::put(TestMaxIdleStarvationBlocks::get().saturating_add(1));
    frame_system::Pallet::<Test>::set_block_number(2);
    let _ = AAA::on_initialize(2);
    let inst = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert_eq!(inst.cycle_nonce, 0);
    assert!(!has_aaa_event(|event| {
      matches!(
        event,
        Event::CycleStarted {
          aaa_id: id,
          cycle_nonce: 1,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn zero_on_idle_budget_performs_no_storage_or_telemetry_work() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    IdleStarvationBlocks::<Test>::put(3);
    let event_count = frame_system::Pallet::<Test>::event_count();
    let used = AAA::on_idle(1, Weight::zero());
    assert_eq!(used, Weight::zero());
    assert_eq!(IdleStarvationBlocks::<Test>::get(), 3);
    assert!(crate::pallet::LastIngressIngestBlock::<Test>::get().is_none());
    assert_eq!(frame_system::Pallet::<Test>::event_count(), event_count);
  });
}

#[test]
fn starvation_emits_observability_event_once_on_threshold_crossing() {
  new_test_ext().execute_with(|| {
    let threshold = TestMaxIdleStarvationBlocks::get();
    for block in 1..=(threshold + 2) {
      frame_system::Pallet::<Test>::set_block_number(block as u64);
      run_idle(starvation_observation_weight());
    }
    let detections = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::AAA(Event::IdleStarvationDetected { consecutive_blocks }) => {
          Some(consecutive_blocks)
        }
        _ => None,
      })
      .collect::<std::vec::Vec<_>>();
    assert_eq!(detections, vec![threshold]);
  });
}

#[test]
fn proof_size_exhaustion_counts_as_idle_starvation() {
  new_test_ext().execute_with(|| {
    let threshold = TestMaxIdleStarvationBlocks::get();
    let base_weight = starvation_observation_weight();
    for block in 1..=threshold {
      frame_system::Pallet::<Test>::set_block_number(u64::from(block));
      run_idle(Weight::from_parts(u64::MAX, base_weight.proof_size()));
    }
    assert_eq!(IdleStarvationBlocks::<Test>::get(), threshold);
    assert!(has_aaa_event(|event| matches!(
      event,
      Event::IdleStarvationDetected { consecutive_blocks } if *consecutive_blocks == threshold
    )));
  });
}

#[test]
fn starvation_resets_after_positive_post_housekeeping_budget() {
  new_test_ext().execute_with(|| {
    let threshold = TestMaxIdleStarvationBlocks::get();
    for block in 1..threshold {
      frame_system::Pallet::<Test>::set_block_number(block as u64);
      run_idle(starvation_observation_weight());
    }
    assert_eq!(
      IdleStarvationBlocks::<Test>::get(),
      threshold.saturating_sub(1)
    );
    frame_system::Pallet::<Test>::set_block_number(threshold as u64);
    run_idle(Weight::MAX);
    assert_eq!(IdleStarvationBlocks::<Test>::get(), 0);
    frame_system::Pallet::<Test>::set_block_number(threshold.saturating_add(1) as u64);
    run_idle(starvation_observation_weight());
    assert_eq!(IdleStarvationBlocks::<Test>::get(), 1);
    assert!(!has_aaa_event(|event| matches!(
      event,
      Event::IdleStarvationDetected { .. }
    )));
  });
}

#[test]
fn breaker_keeps_sweep_path_operational_on_idle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    assert_ok!(AAA::set_global_circuit_breaker(RuntimeOrigin::root(), true));
    let _ = AAA::on_idle(1, Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::AaaClosed {
          aaa_id: id,
          reason: CloseReason::BalanceExhausted,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn breaker_defers_scheduler_owned_fee_budget_close_without_partial_tail() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    fund_native(aaa_id, 60);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    assert_ok!(AAA::set_global_circuit_breaker(RuntimeOrigin::root(), true));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let instance = AAA::aaa_instances(aaa_id).expect("breaker keeps actor pending");
    assert_eq!(instance.cycle_nonce, 0);
    assert!(!has_aaa_event(|event| matches!(
      event,
      Event::CycleStarted { aaa_id: id, .. }
        | Event::CycleSummary { aaa_id: id, .. }
        | Event::OnCloseExecutionPlanSummary { aaa_id: id, .. }
        | Event::AaaClosed { aaa_id: id, .. }
        if *id == aaa_id
    )));
    assert_ok!(AAA::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      false
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(has_aaa_event(|event| matches!(
      event,
      Event::AaaClosed {
        aaa_id: id,
        reason: CloseReason::FeeBudgetExhausted,
      } if *id == aaa_id
    )));
  });
}

#[test]
fn breaker_defers_scheduler_owned_window_expiry_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      transfer_execution_plan(BOB, 1),
    );
    fund_native(aaa_id, 1_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    assert_ok!(AAA::set_global_circuit_breaker(RuntimeOrigin::root(), true));
    frame_system::Pallet::<Test>::set_block_number(102);
    frame_system::Pallet::<Test>::reset_events();
    let _ = AAA::execute_cycle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_some());
    assert!(!has_aaa_event(|event| matches!(
      event,
      Event::OnCloseExecutionPlanSummary { aaa_id: id, .. }
        | Event::AaaClosed { aaa_id: id, .. }
        if *id == aaa_id
    )));
    assert_ok!(AAA::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      false
    ));
    frame_system::Pallet::<Test>::set_block_number(103);
    frame_system::Pallet::<Test>::reset_events();
    let _ = AAA::execute_cycle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(has_aaa_event(|event| matches!(
      event,
      Event::AaaClosed {
        aaa_id: id,
        reason: CloseReason::WindowExpired,
      } if *id == aaa_id
    )));
  });
}

#[test]
fn scheduler_close_failure_is_charged_deferred_and_retried() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      transfer_execution_plan(BOB, 1),
    );
    fund_native(aaa_id, 1_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    frame_system::Pallet::<Test>::set_block_number(102);
    frame_system::Pallet::<Test>::reset_events();
    set_fail_close_checkpoint(true);
    let consumed = AAA::execute_cycle(Weight::MAX);
    assert!(consumed.ref_time() > 0 || consumed.proof_size() > 0);
    assert!(AAA::aaa_instances(aaa_id).is_some());
    assert!(
      AAA::actor_hot(aaa_id)
        .expect("deferred actor")
        .queue_ticket
        .is_some()
    );
    assert!(has_aaa_event(|event| matches!(
      event,
      Event::CycleDeferred {
        aaa_id: id,
        reason: DeferReason::CloseTransitionFailed,
      } if *id == aaa_id
    )));
    assert!(!has_aaa_event(|event| matches!(
      event,
      Event::OnCloseExecutionPlanSummary { aaa_id: id, .. }
        | Event::AaaClosed { aaa_id: id, .. }
        if *id == aaa_id
    )));
    set_fail_close_checkpoint(false);
    frame_system::Pallet::<Test>::set_block_number(103);
    frame_system::Pallet::<Test>::reset_events();
    let _ = AAA::execute_cycle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(has_aaa_event(|event| matches!(
      event,
      Event::AaaClosed {
        aaa_id: id,
        reason: CloseReason::WindowExpired,
      } if *id == aaa_id
    )));
  });
}

#[test]
fn sweep_close_failure_is_charged_deferred_and_retried_at_same_cursor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let live_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    fund_native(live_id, 1_000);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_execution_plan(),
    );
    fund_native(aaa_id, 1_000);
    frame_system::Pallet::<Test>::set_block_number(102);
    frame_system::Pallet::<Test>::reset_events();
    set_fail_close_checkpoint(true);
    let consumed = AAA::execute_zombie_sweep(Weight::MAX);
    assert!(consumed.ref_time() > 0 || consumed.proof_size() > 0);
    assert!(AAA::aaa_instances(aaa_id).is_some());
    assert_eq!(crate::SweepCursor::<Test>::get(), 0);
    assert!(has_aaa_event(|event| matches!(
      event,
      Event::CycleDeferred {
        aaa_id: id,
        reason: DeferReason::CloseTransitionFailed,
      } if *id == aaa_id
    )));
    assert!(!has_aaa_event(|event| matches!(
      event,
      Event::AaaClosed { aaa_id: id, .. } if *id == aaa_id
    )));
    set_fail_close_checkpoint(false);
    frame_system::Pallet::<Test>::set_block_number(103);
    frame_system::Pallet::<Test>::reset_events();
    let _ = AAA::execute_zombie_sweep(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert_eq!(crate::SweepCursor::<Test>::get(), aaa_id);
    assert!(has_aaa_event(|event| matches!(
      event,
      Event::AaaClosed {
        aaa_id: id,
        reason: CloseReason::WindowExpired,
      } if *id == aaa_id
    )));
  });
}

#[test]
fn post_cycle_close_failure_is_deferred_requeued_and_retried() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    fund_native(aaa_id, 1_000);
    assert_ok!(AAA::set_auto_close_at_cycle_nonce(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      Some(1),
    ));
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    frame_system::Pallet::<Test>::reset_events();
    set_fail_close_checkpoint(true);
    let consumed = AAA::execute_cycle(Weight::MAX);
    assert!(consumed.ref_time() > 0 || consumed.proof_size() > 0);
    let retained = AAA::aaa_instances(aaa_id).expect("failed close must retain actor");
    assert_eq!(retained.cycle_nonce, 1);
    assert!(
      AAA::actor_hot(aaa_id)
        .expect("deferred actor")
        .queue_ticket
        .is_some()
    );
    assert!(has_aaa_event(|event| matches!(
      event,
      Event::CycleSummary { aaa_id: id, .. } if *id == aaa_id
    )));
    assert!(has_aaa_event(|event| matches!(
      event,
      Event::CycleDeferred {
        aaa_id: id,
        reason: DeferReason::CloseTransitionFailed,
      } if *id == aaa_id
    )));
    assert!(!has_aaa_event(|event| matches!(
      event,
      Event::AaaClosed { aaa_id: id, .. } if *id == aaa_id
    )));
    set_fail_close_checkpoint(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    let _ = AAA::execute_cycle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(has_aaa_event(|event| matches!(
      event,
      Event::AaaClosed {
        aaa_id: id,
        reason: CloseReason::AutoCloseNonceReached,
      } if *id == aaa_id
    )));
  });
}

#[test]
fn default_funding_policies_authorize_system_runtime_sources_but_only_user_owner() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan_sys = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let execution_plan_usr = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let system_aaa = create_system_with(ALICE, manual_schedule(), None, execution_plan_sys);
    let user_aaa = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      execution_plan_usr,
    );
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      system_aaa,
      TestAsset::Native,
      100
    ));
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      user_aaa,
      TestAsset::Native,
      100
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(AAA::notify_address_event(
      system_aaa,
      TestAsset::Native,
      500,
      &CHARLIE
    ));
    assert_ok!(AAA::notify_address_event(
      user_aaa,
      TestAsset::Native,
      500,
      &CHARLIE
    ));
    assert_ok!(AAA::notify_address_event(
      user_aaa,
      TestAsset::Native,
      25,
      &ALICE
    ));
    assert_ok!(AAA::notify_internal_address_event(
      user_aaa,
      TestAsset::Native,
      30,
      &ALICE
    ));
    assert_ok!(AAA::notify_xcm_address_event(
      user_aaa,
      TestAsset::Native,
      35,
      &ALICE
    ));
    let sys_inst = actor_funding(system_aaa);
    assert!(matches!(
      sys_inst.funding_source_policy,
      FundingSourcePolicy::RuntimePolicy
    ));
    let sys_batch = sys_inst
      .funding_snapshots
      .get(&TestAsset::Native)
      .expect("system funding batch");
    assert_eq!(sys_batch.amount, 100);
    assert_eq!(sys_batch.pending_amount, 500);
    let user_inst = actor_funding(user_aaa);
    assert!(matches!(
      user_inst.funding_source_policy,
      FundingSourcePolicy::OwnerOnly
    ));
    assert_eq!(
      user_inst
        .funding_snapshots
        .get(&TestAsset::Native)
        .unwrap()
        .amount,
      100
    );
    let user_batch = user_inst
      .funding_snapshots
      .get(&TestAsset::Native)
      .expect("user funding batch");
    assert_eq!(user_batch.pending_amount, 25);
  });
}

#[test]
fn any_source_accepts_all_verified_provenance_classes_but_not_missing_provenance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      execution_plan_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    assert_ok!(AAA::update_funding_source_policy(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      FundingSourcePolicy::AnySource
    ));
    assert_ok!(AAA::notify_internal_address_event(
      aaa_id,
      TestAsset::Native,
      40,
      &CHARLIE
    ));
    assert_ok!(AAA::notify_address_event(
      aaa_id,
      TestAsset::Native,
      30,
      &BOB
    ));
    assert_ok!(AAA::notify_xcm_address_event(
      aaa_id,
      TestAsset::Native,
      20,
      &CHARLIE
    ));
    assert_ok!(AAA::notify_address_event_without_source(
      aaa_id,
      TestAsset::Native,
      1_000
    ));
    let funding = actor_funding(aaa_id);
    let batch = funding
      .funding_snapshots
      .get(&TestAsset::Native)
      .expect("authoritative funding batch");
    assert_eq!(batch.amount, 40);
    assert_eq!(batch.pending_amount, 50);
    assert!(has_aaa_event(|event| matches!(
      event,
      Event::FundingSourcePolicyUpdated { aaa_id: id } if *id == aaa_id
    )));
  });
}

#[test]
fn identical_authoritative_transfers_remain_distinct_funding_events() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      execution_plan_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    assert_ok!(AAA::notify_address_event(
      aaa_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    assert_ok!(AAA::notify_address_event(
      aaa_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    let funding = actor_funding(aaa_id);
    let batch = funding
      .funding_snapshots
      .get(&TestAsset::Native)
      .expect("authoritative funding batch");
    assert_eq!(batch.amount, 100);
    assert_eq!(batch.pending_amount, 100);
  });
}

#[test]
fn durable_ingress_capacity_rejects_before_additional_funding_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      execution_plan_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    let capacity = <Test as crate::Config>::MaxIngressOverflowQueue::get();
    for _ in 0..capacity {
      assert!(AAA::queue_address_event(
        aaa_id,
        TestAsset::Native,
        1,
        Some(crate::FundingProvenance::Signed(ALICE))
      ));
    }
    let before = actor_funding(aaa_id);
    let before_batch = before
      .funding_snapshots
      .get(&TestAsset::Native)
      .expect("authoritative funding batch")
      .clone();
    assert!(!AAA::queue_address_event(
      aaa_id,
      TestAsset::Native,
      1,
      Some(crate::FundingProvenance::Signed(ALICE))
    ));
    let after = actor_funding(aaa_id);
    assert_eq!(
      after.funding_snapshots.get(&TestAsset::Native),
      Some(&before_batch)
    );
    assert_eq!(AAA::ingress_overflow_len(), capacity);
  });
}

#[test]
fn direct_notification_reports_funding_overflow_without_partial_inbox_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      execution_plan_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    crate::ActorFunding::<Test>::mutate(aaa_id, |maybe| {
      maybe
        .as_mut()
        .expect("user actor funding")
        .funding_snapshots
        .try_insert(
          TestAsset::Native,
          FundingBatch {
            amount: 1,
            pending_amount: u128::MAX,
          },
        )
        .expect("funding batch fits");
    });
    assert_noop!(
      AAA::notify_address_event(aaa_id, TestAsset::Native, 1, &ALICE),
      Error::<Test>::FundingBatchOverflow
    );
    let funding = actor_funding(aaa_id);
    assert_eq!(
      funding
        .funding_snapshots
        .get(&TestAsset::Native)
        .expect("funding batch")
        .pending_amount,
      u128::MAX
    );
    assert!(AAA::address_event_inbox(aaa_id).is_none());
  });
}

#[test]
fn signed_allowlist_accepts_only_verified_listed_signers_for_funding() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      execution_plan_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    let allowed = BoundedBTreeSet::try_from([CHARLIE].into_iter().collect::<BTreeSet<_>>())
      .expect("one funding signer fits");
    assert_ok!(AAA::update_funding_source_policy(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      FundingSourcePolicy::SignedAllowlist(allowed),
    ));
    assert_ok!(AAA::notify_address_event(
      aaa_id,
      TestAsset::Native,
      100,
      &CHARLIE
    ));
    assert_ok!(AAA::notify_address_event(
      aaa_id,
      TestAsset::Native,
      900,
      &BOB
    ));
    assert_ok!(AAA::notify_internal_address_event(
      aaa_id,
      TestAsset::Native,
      700,
      &CHARLIE
    ));
    assert_ok!(AAA::notify_address_event_without_source(
      aaa_id,
      TestAsset::Native,
      500
    ));
    let funding = actor_funding(aaa_id);
    let batch = funding
      .funding_snapshots
      .get(&TestAsset::Native)
      .expect("listed signer activates funding");
    assert_eq!(batch.amount, 100);
    assert_eq!(batch.pending_amount, 0);
  });
}

#[test]
fn immutable_user_cannot_update_funding_source_policy() {
  new_test_ext().execute_with(|| {
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Immutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    assert_noop!(
      AAA::update_funding_source_policy(
        RuntimeOrigin::signed(ALICE),
        aaa_id,
        FundingSourcePolicy::AnySource
      ),
      Error::<Test>::ImmutableAaa
    );
  });
}

#[test]
fn notify_address_event_accumulates_system_pending_without_pause_resume_cycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(100)),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      TestAsset::Native,
      100
    ));
    assert_eq!(native_balance(&actor), 100);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle_until_cycle_nonce(aaa_id, 1);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle_until_cycle_nonce(aaa_id, 2);
    let instance = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert_eq!(instance.lifecycle, ActiveLifecycle::Active);
    frame_system::Pallet::<Test>::set_block_number(3);
    fund_native(aaa_id, 500);
    assert_ok!(AAA::notify_address_event(
      aaa_id,
      TestAsset::Native,
      500,
      &CHARLIE
    ));
    let updated = actor_funding(aaa_id);
    let batch = updated
      .funding_snapshots
      .get(&TestAsset::Native)
      .expect("funding batch");
    assert_eq!(batch.amount, 100);
    assert_eq!(batch.pending_amount, 500);
    assert!(!has_aaa_event(|event| {
      matches!(event, Event::AaaResumed { aaa_id: id } if *id == aaa_id)
    }));
  });
}

#[test]
fn ordinary_transfer_updates_snapshot_without_resuming_paused_system_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(100)),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    ActorHot::<Test>::mutate(aaa_id, |maybe| {
      let hot = maybe.as_mut().expect("AAA hot state exists");
      hot.lifecycle = ActiveLifecycle::Paused(PauseReason::Manual);
    });
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      TestAsset::Native,
      123
    ));
    let updated = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert_eq!(
      updated.lifecycle,
      ActiveLifecycle::Paused(PauseReason::Manual)
    );
    assert_eq!(
      actor_funding(aaa_id)
        .funding_snapshots
        .get(&TestAsset::Native)
        .expect("snapshot")
        .amount,
      123
    );
    assert!(!has_aaa_event(|event| {
      matches!(event, Event::AaaResumed { aaa_id: id } if *id == aaa_id)
    }));
  });
}

#[test]
fn notify_address_event_updates_snapshot_without_resuming_paused_system_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(100)),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    fund_native(aaa_id, 500);
    ActorHot::<Test>::mutate(aaa_id, |maybe| {
      let hot = maybe.as_mut().expect("AAA hot state exists");
      hot.lifecycle = ActiveLifecycle::Paused(PauseReason::Manual);
    });
    assert_ok!(AAA::notify_address_event(
      aaa_id,
      TestAsset::Native,
      500,
      &CHARLIE
    ));
    let updated = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert_eq!(
      updated.lifecycle,
      ActiveLifecycle::Paused(PauseReason::Manual)
    );
    assert_eq!(
      actor_funding(aaa_id)
        .funding_snapshots
        .get(&TestAsset::Native)
        .expect("snapshot")
        .amount,
      500
    );
    assert_eq!(native_balance(&actor), 500);
    assert!(!has_aaa_event(|event| {
      matches!(event, Event::AaaResumed { aaa_id: id } if *id == aaa_id)
    }));
  });
}

#[test]
fn multi_asset_funding_batches_are_independent() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // ExecutionPlan with TWO assets using PercentageOfLastFunding
    let step1 = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    });
    let step2 = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Local(1),
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(25)),
    });
    let execution_plan = BoundedVec::try_from(vec![step1, step2]).expect("execution_plan fits");
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    // Verify both assets are tracked
    let inst = actor_funding(aaa_id);
    assert!(inst.funding_tracked_assets.contains(&TestAsset::Native));
    assert!(inst.funding_tracked_assets.contains(&TestAsset::Local(1)));
    assert_eq!(inst.funding_tracked_assets.len(), 2);
    // Transfer Native first
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      TestAsset::Native,
      1000
    ));
    let inst = actor_funding(aaa_id);
    assert_eq!(
      inst
        .funding_snapshots
        .get(&TestAsset::Native)
        .unwrap()
        .amount,
      1000
    );
    assert!(inst.funding_snapshots.get(&TestAsset::Local(1)).is_none());
    // Fund Local(1) separately
    frame_system::Pallet::<Test>::set_block_number(2);
    <crate::mock::MockAssetOps as crate::adapters::AssetOps<AccountId, TestAsset, Balance>>::mint(
      &ALICE,
      TestAsset::Local(1),
      400,
    )
    .unwrap();
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      TestAsset::Local(1),
      400
    ));
    let inst = actor_funding(aaa_id);
    assert_eq!(
      inst
        .funding_snapshots
        .get(&TestAsset::Native)
        .unwrap()
        .amount,
      1000
    ); // unchanged
    assert_eq!(
      inst
        .funding_snapshots
        .get(&TestAsset::Local(1))
        .unwrap()
        .amount,
      400
    );
    // Another Native transfer accumulates only the Native pending batch
    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      TestAsset::Native,
      500
    ));
    let inst = actor_funding(aaa_id);
    let native_batch = inst
      .funding_snapshots
      .get(&TestAsset::Native)
      .expect("native funding batch");
    assert_eq!(native_batch.amount, 1000); // active remains frozen
    assert_eq!(native_batch.pending_amount, 500);
    assert_eq!(
      inst
        .funding_snapshots
        .get(&TestAsset::Local(1))
        .unwrap()
        .amount,
      400
    ); // unchanged
  });
}

// --- Error Coverage Tests ---

#[test]
fn aaa_not_found_on_nonexistent_id() {
  new_test_ext().execute_with(|| {
    assert_noop!(
      AAA::pause_aaa(RuntimeOrigin::signed(ALICE), 999),
      Error::<Test>::AaaNotFound
    );
  });
}

#[test]
fn aaa_id_overflow_at_max() {
  new_test_ext().execute_with(|| {
    NextAaaId::<Test>::put(u64::MAX);
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_program(manual_schedule(), None, transfer_execution_plan(BOB, 10)),
      ),
      Error::<Test>::AaaIdOverflow
    );
  });
}

#[test]
fn empty_execution_plan_rejected() {
  new_test_ext().execute_with(|| {
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_program(manual_schedule(), None, BoundedVec::default()),
      ),
      Error::<Test>::EmptyExecutionPlan
    );
  });
}

#[test]
fn execution_plan_too_long_rejected() {
  new_test_ext().execute_with(|| {
    let steps: Vec<_> = (0..4)
      .map(|i| {
        make_step(Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(i + 1),
        })
      })
      .collect();
    let execution_plan = BoundedVec::try_from(steps).expect("fits MaxExecutionPlanSteps");
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_program(manual_schedule(), None, execution_plan),
      ),
      Error::<Test>::ExecutionPlanTooLong
    );
  });
}

#[test]
fn sovereign_account_collision_rejected() {
  new_test_ext().execute_with(|| {
    let execution_plan = transfer_execution_plan(BOB, 10);
    let _aaa_id = AAA::next_aaa_id();
    let sovereign = AAA::sovereign_account_id(&ALICE, 0);
    SovereignIndex::<Test>::insert(&sovereign, 999u64);
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_program(manual_schedule(), None, execution_plan),
      ),
      Error::<Test>::SovereignAccountCollision
    );
  });
}

#[test]
fn not_owner_on_foreign_aaa() {
  new_test_ext().execute_with(|| {
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    assert_noop!(
      AAA::pause_aaa(RuntimeOrigin::signed(BOB), aaa_id),
      Error::<Test>::NotOwner
    );
  });
}

#[test]
fn not_governance_on_user_aaa_via_root() {
  new_test_ext().execute_with(|| {
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    assert_noop!(
      AAA::pause_aaa(RuntimeOrigin::root(), aaa_id),
      Error::<Test>::NotGovernance
    );
  });
}

#[test]
fn governance_can_manage_system_aaa_control_surface() {
  new_test_ext().execute_with(|| {
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    assert_ok!(AAA::pause_aaa(RuntimeOrigin::root(), aaa_id));
    assert_eq!(
      AAA::aaa_instances(aaa_id).expect("system actor").lifecycle,
      ActiveLifecycle::Paused(PauseReason::Manual)
    );
    assert_ok!(AAA::resume_aaa(RuntimeOrigin::root(), aaa_id));
    assert_eq!(
      AAA::aaa_instances(aaa_id).expect("system actor").lifecycle,
      ActiveLifecycle::Active
    );
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::root(), aaa_id));
    assert!(
      AAA::aaa_instances(aaa_id)
        .expect("system actor")
        .manual_trigger_pending
    );
    let updated_schedule = timer_schedule(3);
    assert_ok!(AAA::update_schedule(
      RuntimeOrigin::root(),
      aaa_id,
      updated_schedule.clone(),
      None,
    ));
    assert_eq!(
      AAA::aaa_instances(aaa_id).expect("system actor").schedule,
      updated_schedule
    );
    ActorHot::<Test>::mutate(aaa_id, |maybe| {
      maybe
        .as_mut()
        .expect("system actor hot state")
        .consecutive_failures = 2;
    });
    let updated_plan = transfer_execution_plan(BOB, 1);
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::root(),
      aaa_id,
      updated_plan.clone(),
    ));
    let updated = AAA::aaa_instances(aaa_id).expect("system actor");
    assert_eq!(updated.execution_plan, updated_plan);
    assert_eq!(updated.consecutive_failures, 0);
    assert_ok!(AAA::set_auto_close_at_cycle_nonce(
      RuntimeOrigin::root(),
      aaa_id,
      Some(5),
    ));
    assert_eq!(
      AAA::aaa_instances(aaa_id)
        .expect("system actor")
        .auto_close_at_cycle_nonce,
      Some(5)
    );
    assert_ok!(AAA::increment_auto_close_nonce(
      RuntimeOrigin::root(),
      aaa_id,
      2,
    ));
    assert_eq!(
      AAA::aaa_instances(aaa_id)
        .expect("system actor")
        .auto_close_at_cycle_nonce,
      Some(7)
    );
    let updated_on_close_plan = transfer_execution_plan(CHARLIE, 1);
    assert_ok!(AAA::update_on_close_execution_plan(
      RuntimeOrigin::root(),
      aaa_id,
      updated_on_close_plan.clone(),
    ));
    assert_eq!(
      AAA::aaa_instances(aaa_id)
        .expect("system actor")
        .on_close_execution_plan,
      updated_on_close_plan
    );
    assert_ok!(AAA::close_aaa(RuntimeOrigin::root(), aaa_id));
    assert!(AAA::aaa_instances(aaa_id).is_none());
  });
}

#[test]
fn not_paused_on_resume() {
  new_test_ext().execute_with(|| {
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    assert_noop!(
      AAA::resume_aaa(RuntimeOrigin::signed(ALICE), aaa_id),
      Error::<Test>::NotPaused
    );
  });
}

#[test]
fn already_paused_on_manual_trigger() {
  new_test_ext().execute_with(|| {
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 10),
    );
    assert_ok!(AAA::pause_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert_noop!(
      AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id),
      Error::<Test>::AaaPaused
    );
  });
}

#[test]
fn cycle_nonce_max_value_is_the_last_executable_cycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    fund_native(aaa_id, 100);
    let bob_before = native_balance(&BOB);
    ActorHot::<Test>::mutate(aaa_id, |maybe| {
      maybe
        .as_mut()
        .expect("system AAA hot state exists")
        .cycle_nonce = u64::MAX - 1;
    });
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    let instance = AAA::aaa_instances(aaa_id).expect("system AAA remains");
    assert_eq!(instance.cycle_nonce, u64::MAX);
    assert_eq!(native_balance(&BOB), bob_before + 1);
    assert!(has_aaa_event(|event| {
      matches!(event, Event::CycleStarted { aaa_id: id, cycle_nonce } if *id == aaa_id && *cycle_nonce == u64::MAX)
    }));
    assert!(has_aaa_event(|event| {
      matches!(event, Event::CycleSummary { aaa_id: id, cycle_nonce, .. } if *id == aaa_id && *cycle_nonce == u64::MAX)
    }));
  });
}

#[test]
fn cycle_nonce_exhaustion_closes_user_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    fund_native(aaa_id, 1_000);
    let bob_before = native_balance(&BOB);
    ActorHot::<Test>::mutate(aaa_id, |maybe| {
      maybe
        .as_mut()
        .expect("user AAA hot state exists")
        .cycle_nonce = u64::MAX;
    });
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before);
    assert!(!has_aaa_event(|event| {
      matches!(event, Event::CycleStarted { aaa_id: id, .. } if *id == aaa_id)
    }));
    assert!(!has_aaa_event(|event| {
      matches!(event, Event::CycleSummary { aaa_id: id, .. } if *id == aaa_id)
    }));
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::AaaClosed {
          aaa_id: id,
          reason: CloseReason::CycleNonceExhausted,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn cycle_nonce_exhaustion_pauses_system_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let bob_before = native_balance(&BOB);
    ActorHot::<Test>::mutate(aaa_id, |maybe| {
      maybe
        .as_mut()
        .expect("system AAA hot state exists")
        .cycle_nonce = u64::MAX;
    });
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    let instance = AAA::aaa_instances(aaa_id).expect("system AAA remains");
    assert_eq!(native_balance(&BOB), bob_before);
    assert!(!has_aaa_event(|event| {
      matches!(event, Event::CycleStarted { aaa_id: id, .. } if *id == aaa_id)
    }));
    assert!(!has_aaa_event(|event| {
      matches!(event, Event::CycleSummary { aaa_id: id, .. } if *id == aaa_id)
    }));
    assert_eq!(
      instance.lifecycle,
      ActiveLifecycle::Paused(PauseReason::CycleNonceExhausted)
    );
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::AaaPaused {
          aaa_id: id,
          reason: PauseReason::CycleNonceExhausted,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn missing_tracked_snapshot_resolves_to_funding_unavailable() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          aaa_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == aaa_id
      )
    }));
    assert!(AAA::aaa_instances(aaa_id).is_some());
  });
}

#[test]
fn zero_snapshot_resolves_to_funding_unavailable() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    crate::ActorFunding::<Test>::mutate(aaa_id, |maybe| {
      let funding = maybe.as_mut().expect("actor funding exists");
      funding
        .funding_snapshots
        .try_insert(
          TestAsset::Native,
          FundingBatch {
            amount: 0,
            pending_amount: 0,
          },
        )
        .expect("snapshot must fit");
    });
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          aaa_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn stale_tracked_snapshot_remains_valid_until_overwritten() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let bob_before = native_balance(&BOB);
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      TestAsset::Native,
      100
    ));
    frame_system::Pallet::<Test>::set_block_number(25);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle_until_cycle_nonce(aaa_id, 1);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
  });
}

#[test]
fn burn_last_funding_overspend_resolves_to_funding_unavailable() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Burn {
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    fund_native(aaa_id, 100);
    crate::ActorFunding::<Test>::mutate(aaa_id, |maybe| {
      let funding = maybe.as_mut().expect("actor funding exists");
      funding
        .funding_snapshots
        .try_insert(
          TestAsset::Native,
          FundingBatch {
            amount: 200,
            pending_amount: 0,
          },
        )
        .expect("snapshot must fit");
    });
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(has_aaa_event(|event| matches!(
      event,
      Event::StepSkipped {
        aaa_id: id,
        step_index: 0,
        reason: StepSkippedReason::FundingUnavailable,
        ..
      } if *id == aaa_id
    )));
    assert_eq!(native_balance(&actor), 100);
    assert_eq!(
      AAA::aaa_instances(aaa_id)
        .expect("AAA remains active")
        .consecutive_failures,
      0
    );
  });
}

#[test]
fn overspend_resolves_to_funding_unavailable_for_system_without_pause() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1_000_000),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          aaa_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == aaa_id
      )
    }));
    let instance = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert_eq!(instance.lifecycle, ActiveLifecycle::Active);
    let actor = sovereign_account(aaa_id);
    assert_eq!(
      native_balance(&actor),
      100,
      "balance stays unchanged on FundingUnavailable"
    );
  });
}

#[test]
fn overspend_resolves_to_funding_unavailable_for_user_without_closing() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1_000_000),
    }));
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      execution_plan,
    );
    let actor = sovereign_account(aaa_id);
    fund_native(aaa_id, 1_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          aaa_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == aaa_id
      )
    }));
    assert!(AAA::aaa_instances(aaa_id).is_some());
    assert_eq!(native_balance(&actor), 999);
  });
}

#[test]
fn funding_unavailable_releases_exec_fee_reservation_for_later_step_spend() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1_000_000),
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(848),
      }),
    ])
    .expect("execution plan must fit");
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      execution_plan,
    );
    let actor = sovereign_account(aaa_id);
    let bob_before = native_balance(&BOB);
    fund_native(aaa_id, 1_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          aaa_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == aaa_id
      )
    }));
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(848));
    assert_eq!(
      native_balance(&actor),
      50,
      "later step can spend the execution-fee reservation released by the funding skip"
    );
  });
}

#[test]
fn failed_executable_step_charges_eval_and_exec_fee_without_refund() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::SwapExactIn {
      asset_in: TestAsset::Native,
      asset_out: TestAsset::Local(99),
      amount_in: AmountResolution::Fixed(10),
      slippage_tolerance: Perbill::zero(),
    }));
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      execution_plan,
    );
    let actor = sovereign_account(aaa_id);
    fund_native(aaa_id, 1_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&actor),
      899,
      "failed executable path should charge exactly eval+exec fee with no refund"
    );
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::StepFailed {
          aaa_id: id,
          step_index: 0,
          ..
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn global_circuit_breaker_blocks_creation() {
  new_test_ext().execute_with(|| {
    assert_ok!(AAA::set_global_circuit_breaker(RuntimeOrigin::root(), true));
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_program(manual_schedule(), None, transfer_execution_plan(BOB, 10)),
      ),
      Error::<Test>::GlobalCircuitBreakerActive
    );
    assert_noop!(
      AAA::create_system_aaa(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_program(manual_schedule(), None, transfer_execution_plan(BOB, 10)),
      ),
      Error::<Test>::GlobalCircuitBreakerActive
    );
  });
}

#[test]
fn governance_updates_active_actor_limit_and_creation_respects_it() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let old_limit: u32 = <<Test as crate::Config>::MaxActiveActors as Get<u32>>::get()
      .min(<<Test as crate::Config>::MaxQueueLength as Get<u32>>::get());
    assert_eq!(AAA::configured_active_actor_limit(), old_limit);
    assert_ok!(AAA::set_active_actor_limit(RuntimeOrigin::root(), 2));
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::ActiveActorLimitSet {
          old_limit: prev,
          new_limit: 2,
        } if *prev == old_limit
      )
    }));
    let _ = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let _ = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    assert_noop!(
      AAA::create_system_aaa(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_program(manual_schedule(), None, transfer_execution_plan(BOB, 1)),
      ),
      Error::<Test>::ActiveAaaCapacityExceeded
    );
  });
}

#[test]
fn active_actor_limit_update_validates_bounds() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let _ = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    let _ = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, 1),
    );
    assert_noop!(
      AAA::set_active_actor_limit(RuntimeOrigin::root(), 1),
      Error::<Test>::ActiveAaaLimitTooLow
    );
    assert_noop!(
      AAA::set_active_actor_limit(RuntimeOrigin::root(), 0),
      Error::<Test>::ActiveAaaLimitTooLow
    );
    assert_noop!(
      AAA::set_active_actor_limit(RuntimeOrigin::root(), u32::MAX),
      Error::<Test>::ActiveAaaLimitTooHigh
    );
    assert_noop!(
      AAA::set_active_actor_limit(
        RuntimeOrigin::root(),
        <<Test as crate::Config>::MaxQueueLength as Get<u32>>::get().saturating_add(1),
      ),
      Error::<Test>::ActiveAaaLimitExceedsQueueCapacity
    );
  });
}

#[test]
fn invalid_schedule_window_end_before_start() {
  new_test_ext().execute_with(|| {
    let window = ScheduleWindow {
      start: 100,
      end: 50,
    };
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_program(
          manual_schedule(),
          Some(window),
          transfer_execution_plan(BOB, 10)
        ),
      ),
      Error::<Test>::InvalidScheduleWindow
    );
  });
}

// --- Task & Condition Coverage Tests ---

#[test]
fn mint_works_for_system_aaa() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Mint {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(500),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    // Mint on empty account works — mint policy skips source-balance check
    let before = native_balance(&actor);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), before + 500);
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::MintExecuted { aaa_id: id, asset: TestAsset::Native, amount: 500 } if *id == aaa_id
    )));
  });
}

#[test]
fn condition_balance_above_skips_when_below() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 1_000,
      }])
      .unwrap(),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let execution_plan = execution_plan_with_step(step);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    let actor = sovereign_account(aaa_id);
    assert_eq!(
      native_balance(&actor),
      100,
      "transfer skipped — balance below threshold"
    );
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::StepSkipped {
        aaa_id: id,
        step_index: 0,
        reason: StepSkippedReason::ConditionsNotMet,
        ..
      } if *id == aaa_id
    )));
  });
}

#[test]
fn condition_balance_above_executes_when_above() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 50,
      }])
      .unwrap(),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let execution_plan = execution_plan_with_step(step);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&BOB),
      bob_before + 10,
      "transfer executed — balance above threshold"
    );
  });
}

#[test]
fn condition_block_number_above_skips_before_threshold() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(5);
    let step = StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BlockNumberAbove { threshold: 10 }])
        .unwrap(),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let execution_plan = execution_plan_with_step(step);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::StepSkipped { aaa_id: id, step_index: 0, .. } if *id == aaa_id
    )));
  });
}

#[test]
fn condition_block_number_below_skips_after_threshold() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(20);
    let step = StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BlockNumberBelow { threshold: 10 }])
        .unwrap(),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let execution_plan = execution_plan_with_step(step);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::StepSkipped { aaa_id: id, step_index: 0, .. } if *id == aaa_id
    )));
  });
}

#[test]
fn continue_next_step_error_policy_proceeds_after_failure() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let failing_step = StepOf::<Test> {
      conditions: BoundedVec::default(),
      task: Task::SwapExactIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let execution_plan = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(
      has_aaa_event(|e| matches!(
        e,
        Event::StepFailed { aaa_id: id, step_index: 0, .. } if *id == aaa_id
      )),
      "step 0 must fail"
    );
    assert_eq!(
      native_balance(&CHARLIE),
      charlie_before + 10,
      "step 1 must execute despite step 0 failure"
    );
  });
}

#[test]
fn dex_adapter_late_failure_rolls_back_input_transfer() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_in = TestAsset::Native;
    let asset_out = TestAsset::Local(77);
    set_pool_reserves(asset_in, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let failing_step = StepOf::<Test> {
      conditions: BoundedVec::default(),
      task: Task::SwapExactIn {
        asset_in,
        asset_out,
        amount_in: AmountResolution::Fixed(40),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let execution_plan = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    fund_native(aaa_id, 120);
    set_fail_dex_after_input_transfer(true);
    let charlie_before = native_balance(&CHARLIE);
    let pool_native_before = native_balance(&u64::MAX);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 110);
    assert_eq!(asset_balance(&actor, asset_out), 0);
    assert_eq!(native_balance(&u64::MAX), pool_native_before);
    assert_eq!(asset_balance(&u64::MAX, asset_out), 10_000);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(!has_aaa_event(|e| matches!(
      e,
      Event::SwapExecuted { aaa_id: id, .. } if *id == aaa_id
    )));
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::CycleSummary {
        aaa_id: id,
        executed_steps: 1,
        failed_steps: 1,
        ..
      } if *id == aaa_id
    )));
  });
}

#[test]
fn invalid_schedule_window_too_short() {
  new_test_ext().execute_with(|| {
    // MinWindowLength = 100 in mock
    let window = ScheduleWindow { start: 10, end: 50 };
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_program(
          manual_schedule(),
          Some(window),
          transfer_execution_plan(BOB, 10)
        ),
      ),
      Error::<Test>::InvalidScheduleWindow
    );
  });
}

// --- Progressive Improvement Tests ---

#[test]
fn preserve_spend_keeps_native_minimum_across_fixed_percentage_split_and_all_balance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let split_legs: SplitTransferLegsOf<Test> = BoundedVec::try_from(vec![
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(50),
      },
      SplitLeg {
        to: CHARLIE,
        share: Perbill::from_percent(50),
      },
    ])
    .expect("two split legs fit");
    let execution_plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(100),
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfCurrent(Perbill::one()),
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfTrigger(Perbill::one()),
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      }),
      make_step(Task::SplitTransfer {
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(100),
        legs: split_legs,
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::AllBalance,
      }),
    ])
    .expect("system execution plan fits");
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100);
    crate::ActorFunding::<Test>::mutate(aaa_id, |maybe| {
      maybe
        .as_mut()
        .expect("System AAA funding exists")
        .funding_snapshots
        .try_insert(
          TestAsset::Native,
          FundingBatch {
            amount: 100,
            pending_amount: 0,
          },
        )
        .expect("tracked snapshot fits");
    });
    let actor = sovereign_account(aaa_id);
    let bob_before = native_balance(&BOB);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 1);
    assert_eq!(native_balance(&BOB), bob_before + 99);
    let funding_skips = frame_system::Pallet::<Test>::events()
      .iter()
      .filter(|record| {
        matches!(
          &record.event,
          RuntimeEvent::AAA(Event::StepSkipped {
            aaa_id: id,
            reason: StepSkippedReason::FundingUnavailable,
            ..
          }) if *id == aaa_id
        )
      })
      .count();
    assert_eq!(funding_skips, 4);
    let resolution_skips = frame_system::Pallet::<Test>::events()
      .iter()
      .filter(|record| {
        matches!(
          &record.event,
          RuntimeEvent::AAA(Event::StepSkipped {
            aaa_id: id,
            reason: StepSkippedReason::ResolutionSkipped,
            ..
          }) if *id == aaa_id
        )
      })
      .count();
    assert_eq!(resolution_skips, 1);
  });
}

#[test]
fn percentage_of_current_uses_native_preservable_balance_as_its_base() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfCurrent(Perbill::one()),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100);
    let actor = sovereign_account(aaa_id);
    let bob_before = native_balance(&BOB);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 1);
    assert_eq!(native_balance(&BOB), bob_before + 99);
    assert!(frame_system::Pallet::<Test>::events().iter().any(|record| {
      matches!(
        &record.event,
        RuntimeEvent::AAA(Event::TransferExecuted {
          aaa_id: id,
          amount: 99,
          ..
        }) if *id == aaa_id
      )
    }));
  });
}

#[test]
fn percentage_of_current_uses_sufficient_asset_preservable_balance_as_its_base() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(7);
    let execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset,
      amount: AmountResolution::PercentageOfCurrent(Perbill::one()),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, asset, 10);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 1);
    assert_eq!(asset_balance(&BOB, asset), 9);
  });
}

#[test]
fn preserve_spend_keeps_sufficient_asset_minimum() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(7);
    let execution_plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset,
        amount: AmountResolution::Fixed(10),
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset,
        amount: AmountResolution::AllBalance,
      }),
    ])
    .expect("system execution plan fits");
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, asset, 10);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 1);
    assert_eq!(asset_balance(&BOB, asset), 9);
  });
}

#[test]
fn burn_all_balance_drains_to_zero() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Burn {
      asset: TestAsset::Native,
      amount: AmountResolution::AllBalance,
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 500);
    let actor = sovereign_account(aaa_id);
    assert_eq!(native_balance(&actor), 500);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&actor),
      0,
      "Burn(AllBalance) must drain to zero"
    );
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::BurnExecuted { aaa_id: id, asset: TestAsset::Native, amount: 500 } if *id == aaa_id
    )));
  });
}

#[test]
fn mint_on_unfunded_account_creates_tokens() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Mint {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1000),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    assert_eq!(native_balance(&actor), 0);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 1000);
  });
}

#[test]
fn stake_task_delegates_to_staking_adapter() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(7);
    let execution_plan = execution_plan_with_step(make_step(Task::Stake {
      asset,
      amount: AmountResolution::Fixed(120),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, asset, 200);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 80);
    assert_eq!(staked_balance(actor, asset), 120);
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::StakeExecuted { aaa_id: id, asset: event_asset, amount }
        if *id == aaa_id && *event_asset == asset && *amount == 120
    )));
  });
}

#[test]
fn stake_preserve_spend_keeps_asset_minimum_balance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(70);
    let execution_plan = execution_plan_with_step(make_step(Task::Stake {
      asset,
      amount: AmountResolution::Fixed(99),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, asset, 100);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 1);
    assert_eq!(staked_balance(actor, asset), 99);
  });
}

#[test]
fn add_liquidity_uses_funding_unavailable_precedence_across_amount_fields() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_a = TestAsset::Local(71);
    let asset_b = TestAsset::Local(72);
    let execution_plan = execution_plan_with_step(make_step(Task::AddLiquidity {
      asset_a,
      asset_b,
      amount_a: AmountResolution::PercentageOfCurrent(Perbill::from_percent(1)),
      amount_b: AmountResolution::Fixed(50),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, asset_a, 2);
    set_asset_balance(&actor, asset_b, 50);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          aaa_id: id,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == aaa_id
      )
    }));
    assert_eq!(asset_balance(&actor, asset_a), 2);
    assert_eq!(asset_balance(&actor, asset_b), 50);
  });
}

#[test]
fn unstake_task_delegates_to_staking_adapter() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(8);
    let execution_plan = execution_plan_with_step(make_step(Task::Unstake {
      asset,
      shares: AmountResolution::Fixed(50),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, asset, 75);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 25);
    assert_eq!(unstaked_shares(actor, asset), 50);
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::UnstakeExecuted { aaa_id: id, asset: event_asset, shares }
        if *id == aaa_id && *event_asset == asset && *shares == 50
    )));
  });
}

#[test]
fn unstake_dynamic_modes_resolve_against_staking_shares() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(8);
    let execution_plan = BoundedVec::try_from(vec![
      make_step(Task::Unstake {
        asset,
        shares: AmountResolution::PercentageOfCurrent(Perbill::from_percent(25)),
      }),
      make_step(Task::Unstake {
        asset,
        shares: AmountResolution::PercentageOfTrigger(Perbill::from_percent(50)),
      }),
    ])
    .expect("system execution plan fits");
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, asset, 100);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 25);
    assert_eq!(unstaked_shares(actor, asset), 75);
  });
}

#[test]
fn unstake_all_balance_withdraws_all_staking_shares() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(8);
    let execution_plan = execution_plan_with_step(make_step(Task::Unstake {
      asset,
      shares: AmountResolution::AllBalance,
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, asset, 80);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 0);
    assert_eq!(unstaked_shares(actor, asset), 80);
  });
}

#[test]
fn unstake_last_funding_tracks_transferable_share_asset() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(8);
    let execution_plan = execution_plan_with_step(make_step(Task::Unstake {
      asset,
      shares: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    set_asset_balance(&ALICE, asset, 100);
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      asset,
      100,
    ));
    let actor = sovereign_account(aaa_id);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 50);
    assert_eq!(unstaked_shares(actor, asset), 50);
  });
}

#[test]
fn unstake_last_funding_rejects_position_without_transferable_share_asset() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Unstake {
      asset: TestAsset::Local(u32::MAX),
      shares: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    }));
    assert_noop!(
      AAA::create_system_aaa(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_program(manual_schedule(), None, execution_plan),
      ),
      Error::<Test>::InvalidAmountResolution
    );
  });
}

#[test]
fn stake_adapter_failure_can_continue_next_step() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(13);
    let failing_step = StepOf::<Test> {
      conditions: BoundedVec::default(),
      task: Task::Stake {
        asset,
        amount: AmountResolution::Fixed(40),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let execution_plan = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, asset, 100);
    fund_native(aaa_id, 20);
    set_fail_staking_ops(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 100);
    assert_eq!(staked_balance(actor, asset), 0);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::StepFailed { aaa_id: id, step_index: 0, .. } if *id == aaa_id
    )));
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::CycleSummary {
        aaa_id: id,
        executed_steps: 1,
        failed_steps: 1,
        ..
      } if *id == aaa_id
    )));
  });
}

#[test]
fn unstake_adapter_failure_aborts_cycle_without_partial_effects() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(14);
    let failing_step = StepOf::<Test> {
      conditions: BoundedVec::default(),
      task: Task::Unstake {
        asset,
        shares: AmountResolution::Fixed(40),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let skipped_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let execution_plan = BoundedVec::try_from(vec![failing_step, skipped_step]).unwrap();
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, asset, 100);
    fund_native(aaa_id, 20);
    set_fail_staking_ops(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 100);
    assert_eq!(unstaked_shares(actor, asset), 0);
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::StepFailed { aaa_id: id, step_index: 0, .. } if *id == aaa_id
    )));
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::CycleSummary {
        aaa_id: id,
        executed_steps: 0,
        failed_steps: 1,
        ..
      } if *id == aaa_id
    )));
  });
}

#[test]
fn staking_adapter_late_failure_rolls_back_partial_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Native;
    let failing_step = StepOf::<Test> {
      conditions: BoundedVec::default(),
      task: Task::Stake {
        asset,
        amount: AmountResolution::Fixed(40),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let execution_plan = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    fund_native(aaa_id, 120);
    set_fail_staking_after_burn(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 110);
    assert_eq!(staked_balance(actor, asset), 0);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(!has_aaa_event(|e| matches!(
      e,
      Event::StakeExecuted { aaa_id: id, .. } if *id == aaa_id
    )));
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::CycleSummary {
        aaa_id: id,
        executed_steps: 1,
        failed_steps: 1,
        ..
      } if *id == aaa_id
    )));
  });
}

#[test]
fn unstake_adapter_late_failure_rolls_back_partial_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Native;
    let failing_step = StepOf::<Test> {
      conditions: BoundedVec::default(),
      task: Task::Unstake {
        asset,
        shares: AmountResolution::Fixed(40),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let execution_plan = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    fund_native(aaa_id, 120);
    set_fail_staking_after_burn(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 110);
    assert_eq!(unstaked_shares(actor, asset), 0);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(!has_aaa_event(|e| matches!(
      e,
      Event::UnstakeExecuted { aaa_id: id, .. } if *id == aaa_id
    )));
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::CycleSummary {
        aaa_id: id,
        executed_steps: 1,
        failed_steps: 1,
        ..
      } if *id == aaa_id
    )));
  });
}

#[test]
fn donate_liquidity_task_delegates_to_liquidity_donation_adapter() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_a = TestAsset::Local(9);
    let asset_b = TestAsset::Local(10);
    let execution_plan = execution_plan_with_step(make_step(Task::DonateLiquidity {
      asset_a,
      asset_b,
      amount: AmountResolution::Fixed(40),
      max_ratio_error: Perbill::from_percent(1),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, asset_a, 100);
    set_asset_balance(&actor, asset_b, 90);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset_a), 60);
    assert_eq!(asset_balance(&actor, asset_b), 50);
    assert_eq!(donated_liquidity(actor, asset_a, asset_b), (40, 40));
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::LiquidityDonated {
        aaa_id: id,
        asset_a: event_asset_a,
        asset_b: event_asset_b,
        amount,
        amount_a,
        amount_b,
      } if *id == aaa_id
        && *event_asset_a == asset_a
        && *event_asset_b == asset_b
        && *amount == 40
        && *amount_a == 40
        && *amount_b == 40
    )));
  });
}

#[test]
fn donate_liquidity_percentage_resolves_only_against_asset_a() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_a = TestAsset::Local(73);
    let asset_b = TestAsset::Local(74);
    let execution_plan = execution_plan_with_step(make_step(Task::DonateLiquidity {
      asset_a,
      asset_b,
      amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
      max_ratio_error: Perbill::from_percent(1),
    }));
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, asset_a, 101);
    set_asset_balance(&actor, asset_b, 100);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(donated_liquidity(actor, asset_a, asset_b), (50, 50));
    assert_eq!(asset_balance(&actor, asset_a), 51);
    assert_eq!(asset_balance(&actor, asset_b), 50);
  });
}

#[test]
fn donate_liquidity_adapter_failure_is_non_partial_and_can_continue() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_a = TestAsset::Local(11);
    let asset_b = TestAsset::Local(12);
    let failing_step = StepOf::<Test> {
      conditions: BoundedVec::default(),
      task: Task::DonateLiquidity {
        asset_a,
        asset_b,
        amount: AmountResolution::Fixed(40),
        max_ratio_error: Perbill::from_percent(1),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let execution_plan = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, asset_a, 100);
    set_asset_balance(&actor, asset_b, 10);
    fund_native(aaa_id, 20);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset_a), 100);
    assert_eq!(asset_balance(&actor, asset_b), 10);
    assert_eq!(donated_liquidity(actor, asset_a, asset_b), (0, 0));
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::StepFailed { aaa_id: id, step_index: 0, .. } if *id == aaa_id
    )));
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::TransferExecuted { aaa_id: id, to, amount, .. }
        if *id == aaa_id && *to == CHARLIE && *amount == 10
    )));
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::CycleSummary {
        aaa_id: id,
        executed_steps: 1,
        failed_steps: 1,
        ..
      } if *id == aaa_id
    )));
  });
}

#[test]
fn donate_liquidity_adapter_failure_aborts_cycle_without_partial_effects() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_a = TestAsset::Local(15);
    let asset_b = TestAsset::Local(16);
    let failing_step = StepOf::<Test> {
      conditions: BoundedVec::default(),
      task: Task::DonateLiquidity {
        asset_a,
        asset_b,
        amount: AmountResolution::Fixed(40),
        max_ratio_error: Perbill::from_percent(1),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let skipped_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let execution_plan = BoundedVec::try_from(vec![failing_step, skipped_step]).unwrap();
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, asset_a, 100);
    set_asset_balance(&actor, asset_b, 100);
    fund_native(aaa_id, 20);
    set_fail_liquidity_donation_ops(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset_a), 100);
    assert_eq!(asset_balance(&actor, asset_b), 100);
    assert_eq!(donated_liquidity(actor, asset_a, asset_b), (0, 0));
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::StepFailed { aaa_id: id, step_index: 0, .. } if *id == aaa_id
    )));
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::CycleSummary {
        aaa_id: id,
        executed_steps: 0,
        failed_steps: 1,
        ..
      } if *id == aaa_id
    )));
  });
}

#[test]
fn donate_liquidity_adapter_late_failure_rolls_back_partial_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_a = TestAsset::Native;
    let asset_b = TestAsset::Local(19);
    let failing_step = StepOf::<Test> {
      conditions: BoundedVec::default(),
      task: Task::DonateLiquidity {
        asset_a,
        asset_b,
        amount: AmountResolution::Fixed(40),
        max_ratio_error: Perbill::from_percent(1),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let execution_plan = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, asset_b, 100);
    fund_native(aaa_id, 120);
    set_fail_liquidity_donation_after_first_burn(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 110);
    assert_eq!(asset_balance(&actor, asset_b), 100);
    assert_eq!(donated_liquidity(actor, asset_a, asset_b), (0, 0));
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(!has_aaa_event(|e| matches!(
      e,
      Event::LiquidityDonated { aaa_id: id, .. } if *id == aaa_id
    )));
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::CycleSummary {
        aaa_id: id,
        executed_steps: 1,
        failed_steps: 1,
        ..
      } if *id == aaa_id
    )));
  });
}

#[test]
fn condition_balance_below_skips_when_above() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BalanceBelow {
        asset: TestAsset::Native,
        threshold: 50,
      }])
      .unwrap(),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let execution_plan = execution_plan_with_step(step);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::StepSkipped { aaa_id: id, step_index: 0, .. } if *id == aaa_id
    )));
  });
}

#[test]
fn condition_balance_below_executes_when_below() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BalanceBelow {
        asset: TestAsset::Native,
        threshold: 200,
      }])
      .unwrap(),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let execution_plan = execution_plan_with_step(step);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before + 10);
  });
}

#[test]
fn condition_balance_equals_matches_exact() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BalanceEquals {
        asset: TestAsset::Native,
        threshold: 100,
      }])
      .unwrap(),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let execution_plan = execution_plan_with_step(step);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&BOB),
      bob_before + 10,
      "executes when balance == threshold"
    );
  });
}

#[test]
fn condition_balance_equals_skips_when_not_equal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BalanceEquals {
        asset: TestAsset::Native,
        threshold: 999,
      }])
      .unwrap(),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let execution_plan = execution_plan_with_step(step);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::StepSkipped { aaa_id: id, step_index: 0, .. } if *id == aaa_id
    )));
  });
}

#[test]
fn condition_balance_not_equals_executes_when_different() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BalanceNotEquals {
        asset: TestAsset::Native,
        threshold: 999,
      }])
      .unwrap(),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let execution_plan = execution_plan_with_step(step);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before + 10);
  });
}

#[test]
fn condition_balance_not_equals_skips_when_equal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BalanceNotEquals {
        asset: TestAsset::Native,
        threshold: 100,
      }])
      .unwrap(),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let execution_plan = execution_plan_with_step(step);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::StepSkipped { aaa_id: id, step_index: 0, .. } if *id == aaa_id
    )));
  });
}

#[test]
fn insufficient_fee_closes_cycle_immediately() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    }));
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      execution_plan,
    );
    // Fund above MinUserBalance(50) but below fee threshold (eval=1 + exec=100 = 101)
    fund_native(aaa_id, 60);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::AaaClosed {
        aaa_id: id,
        reason: CloseReason::FeeBudgetExhausted,
      } if *id == aaa_id
    )));
  });
}

#[test]
fn condition_sees_spendable_not_raw_balance_for_user_aaa() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // fees per step: eval=1 + exec=100 = 101
    // Fund 200: raw=200, spendable=200-101=99
    // Condition threshold=150: raw(200)>150 but spendable(99)<150
    let step = StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 150,
      }])
      .unwrap(),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let execution_plan = execution_plan_with_step(step);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      execution_plan,
    );
    fund_native(aaa_id, 200);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    // Must skip: spendable(99) < threshold(150), even though raw(200) > 150
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::StepSkipped { aaa_id: id, step_index: 0, .. } if *id == aaa_id
    )));
  });
}

#[test]
fn condition_sees_full_balance_for_system_aaa() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // System AAA: reserved=0, so spendable == raw
    let step = StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 150,
      }])
      .unwrap(),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let execution_plan = execution_plan_with_step(step);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 200);
    let bob_before = native_balance(&BOB);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    // Must execute: spendable(200) > threshold(150)
    assert_eq!(native_balance(&BOB), bob_before + 50);
  });
}

#[test]
fn system_condition_respects_adapter_visible_native_lock() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 100,
      }])
      .unwrap(),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let aaa_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      execution_plan_with_step(step),
    );
    fund_native(aaa_id, 200);
    let sovereign = AAA::aaa_instances(aaa_id)
      .expect("system aaa")
      .sovereign_account;
    set_native_transfer_lock(&sovereign, 150);
    let bob_before = native_balance(&BOB);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::StepSkipped { aaa_id: id, step_index: 0, .. } if *id == aaa_id
    )));
  });
}

#[test]
fn user_condition_combines_adapter_lock_with_reserved_fee_budget() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 60,
      }])
      .unwrap(),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      execution_plan_with_step(step),
    );
    fund_native(aaa_id, 300);
    let sovereign = AAA::aaa_instances(aaa_id)
      .expect("user aaa")
      .sovereign_account;
    set_native_transfer_lock(&sovereign, 150);
    let bob_before = native_balance(&BOB);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::StepSkipped { aaa_id: id, step_index: 0, .. } if *id == aaa_id
    )));
  });
}

// --- Deterministic Timer Tests ---

#[test]
fn timer_always_executes_on_interval() {
  new_test_ext().execute_with(|| {
    let schedule = timer_schedule(5);
    let execution_plan = transfer_execution_plan(BOB, 10);
    let aaa_id = create_system_with(ALICE, schedule, None, execution_plan);
    fund_native(aaa_id, 1000);
    // Track executions via cycle_nonce changes (each cycle increments nonce)
    let mut last_cycle_nonce = 0u64;
    let mut execution_count = 0usize;
    for block in 2..22 {
      frame_system::Pallet::<Test>::set_block_number(block);
      AAA::on_initialize(block);
      AAA::on_idle(block, Weight::MAX);
      if let Some(inst) = AAA::aaa_instances(aaa_id) {
        if inst.cycle_nonce > last_cycle_nonce {
          execution_count += 1;
          last_cycle_nonce = inst.cycle_nonce;
        }
      }
    }
    assert!(
      execution_count >= 2,
      "timer should execute every 5 blocks, got {} executions",
      execution_count
    );
  });
}

#[test]
fn timer_every_block_uses_queue_continuation_without_wakeup_index() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(ALICE, timer_schedule(1), None, inert_execution_plan());
    for block in 1..=6 {
      frame_system::Pallet::<Test>::set_block_number(block);
      AAA::on_initialize(block);
      AAA::on_idle(block, Weight::MAX);
      assert!(
        crate::pallet::WakeupIndex::<Test>::iter().next().is_none(),
        "every-block timers must not use WakeupIndex"
      );
    }
    let cycle_nonce = AAA::aaa_instances(aaa_id).expect("AAA exists").cycle_nonce;
    assert!(
      cycle_nonce >= 5,
      "every-block timer should keep progressing"
    );
  });
}

#[test]
fn timer_every_block_with_long_cooldown_uses_one_exact_wakeup() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::Timer { every_blocks: 1 },
      cooldown_blocks: 10,
    };
    let aaa_id = create_system_with(ALICE, schedule, None, inert_execution_plan());
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::root(), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(
      AAA::aaa_instances(aaa_id).expect("AAA exists").cycle_nonce,
      1
    );
    assert_eq!(crate::ScheduledWakeupBlock::<Test>::get(aaa_id), Some(11));
    assert!(
      AAA::actor_hot(aaa_id)
        .expect("timer actor")
        .queue_ticket
        .is_none()
    );
    for block in 2..=10 {
      frame_system::Pallet::<Test>::set_block_number(block);
      run_idle(Weight::MAX);
      assert_eq!(
        AAA::aaa_instances(aaa_id).expect("AAA exists").cycle_nonce,
        1
      );
      assert_eq!(crate::ScheduledWakeupBlock::<Test>::get(aaa_id), Some(11));
    }
    frame_system::Pallet::<Test>::set_block_number(11);
    run_idle(Weight::MAX);
    assert_eq!(
      AAA::aaa_instances(aaa_id).expect("AAA exists").cycle_nonce,
      2
    );
  });
}

#[test]
fn timer_eligibility_uses_maximum_of_cadence_cooldown_and_window() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::Timer { every_blocks: 4 },
      cooldown_blocks: 10,
    };
    let aaa_id = create_system_with(
      ALICE,
      schedule,
      Some(ScheduleWindow {
        start: 15,
        end: 115,
      }),
      inert_execution_plan(),
    );
    assert_eq!(crate::ScheduledWakeupBlock::<Test>::get(aaa_id), Some(15));
    frame_system::Pallet::<Test>::set_block_number(15);
    run_idle(Weight::MAX);
    assert_eq!(
      AAA::aaa_instances(aaa_id).expect("AAA exists").cycle_nonce,
      1
    );
    assert_eq!(crate::ScheduledWakeupBlock::<Test>::get(aaa_id), Some(25));
  });
}

#[test]
fn paused_timer_waits_for_resume_without_queue_churn_or_signal_loss() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::Timer { every_blocks: 1 },
      cooldown_blocks: 5,
    };
    let aaa_id = create_system_with(ALICE, schedule, None, inert_execution_plan());
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::root(), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(crate::ScheduledWakeupBlock::<Test>::get(aaa_id), Some(6));
    assert_ok!(AAA::pause_aaa(RuntimeOrigin::root(), aaa_id));
    frame_system::Pallet::<Test>::set_block_number(6);
    run_idle(Weight::MAX);
    assert_eq!(
      AAA::aaa_instances(aaa_id).expect("AAA exists").cycle_nonce,
      1
    );
    assert_eq!(crate::ScheduledWakeupBlock::<Test>::get(aaa_id), None);
    assert!(
      AAA::actor_hot(aaa_id)
        .expect("paused actor")
        .queue_ticket
        .is_none()
    );
    frame_system::Pallet::<Test>::set_block_number(7);
    assert_ok!(AAA::resume_aaa(RuntimeOrigin::root(), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(
      AAA::aaa_instances(aaa_id).expect("AAA exists").cycle_nonce,
      2
    );
  });
}

#[test]
fn timer_wakeup_uses_deterministic_jitter_for_delayed_cadence() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let cadence = 20u32;
    let aaa_id = create_system_with(ALICE, timer_schedule(cadence), None, inert_execution_plan());
    let scheduled_block = crate::pallet::WakeupIndex::<Test>::iter()
      .find_map(|(block, queue)| {
        if queue.contains(&aaa_id) {
          Some(block)
        } else {
          None
        }
      })
      .expect("timer wakeup should be scheduled");
    let window = cadence
      .saturating_div(4)
      .min(<Test as crate::Config>::MaxTimerJitterBlocks::get());
    let hash = frame::hashing::blake2_256(&aaa_id.to_le_bytes());
    let raw = u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]);
    let jitter = if window == 0 { 0 } else { raw % window };
    let expected = 1u64
      .saturating_add(u64::from(cadence))
      .saturating_add(u64::from(jitter));
    assert_eq!(scheduled_block, expected);
  });
}

#[test]
fn timer_validation_includes_worst_case_deterministic_jitter() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_delay = TestMaxExecutionDelayBlocks::get() as u32;
    let max_jitter =
      <<Test as crate::Config>::MaxTimerJitterBlocks as Get<u32>>::get().saturating_sub(1);
    let largest_valid_cadence = max_delay.saturating_sub(max_jitter);
    assert_ok!(AAA::create_system_aaa(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_program(
        timer_schedule(largest_valid_cadence),
        None,
        inert_execution_plan()
      ),
    ));
    assert_noop!(
      AAA::create_system_aaa(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_program(
          timer_schedule(largest_valid_cadence.saturating_add(1)),
          None,
          inert_execution_plan(),
        ),
      ),
      Error::<Test>::ExecutionDelayTooLong
    );
  });
}

// --- User AAA E2E Lifecycle Tests ---

#[test]
fn user_dca_complete_lifecycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // Step 1: Create User AAA with Timer trigger
    let schedule = timer_schedule(5);
    let foreign = TestAsset::Local(1);
    set_asset_balance(&ALICE, foreign, 10_000);
    let execution_plan = execution_plan_with_step(StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BalanceAbove {
        asset: foreign,
        threshold: 50,
      }])
      .unwrap(),
      task: Task::Transfer {
        to: BOB,
        asset: foreign,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    });
    let aaa_id = AAA::next_aaa_id();
    assert_ok!(AAA::create_user_aaa(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_program(schedule, None, execution_plan),
    ));
    // Verify creation
    assert!(has_aaa_event(|e| matches!(
      e,
      Event::AaaCreated { aaa_id: id, aaa_type: AaaType::User, .. } if *id == aaa_id
    )));
    let instance = AAA::aaa_instances(aaa_id).expect("instance exists");
    assert_eq!(instance.owner, ALICE);
    // Step 2: Fund sovereign
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, foreign, 500);
    fund_native(aaa_id, 500); // For fees
    // Step 3-4: Advance blocks and verify execution
    for block in 2..7 {
      frame_system::Pallet::<Test>::set_block_number(block);
      AAA::on_initialize(block);
      AAA::on_idle(block, Weight::MAX);
    }
    assert!(
      has_aaa_event(|e| matches!(
        e,
        Event::TransferExecuted { aaa_id: id, .. } if *id == aaa_id
      )),
      "Should execute transfer on timer"
    );
    // Step 5: Multiple cycles
    let bob_before = asset_balance(&BOB, foreign);
    for block in 7..27 {
      frame_system::Pallet::<Test>::set_block_number(block);
      AAA::on_initialize(block);
      AAA::on_idle(block, Weight::MAX);
    }
    let bob_after = asset_balance(&BOB, foreign);
    assert!(bob_after > bob_before, "Bob should receive transfers");
    // Step 6-7: Drain native below MinUserBalance to trigger sweep close
    let actor_native = native_balance(&actor);
    let min_user = <Test as crate::Config>::MinUserBalance::get();
    let slash_amount = actor_native.saturating_sub(min_user / 2);
    let _ = <Balances as Currency<AccountId>>::slash(&actor, slash_amount);
    assert!(
      native_balance(&actor) < min_user,
      "Actor balance must be below MinUserBalance after slash"
    );
    // Sweep cursor iterates MaxSweepPerBlock=3 IDs per block; run enough blocks
    for block in 30..50 {
      frame_system::Pallet::<Test>::set_block_number(block);
      AAA::on_initialize(block);
      AAA::on_idle(block, Weight::MAX);
      if AAA::aaa_instances(aaa_id).is_none() {
        break;
      }
    }
    assert!(
      AAA::aaa_instances(aaa_id).is_none(),
      "User AAA must be destroyed by sweep when native < MinUserBalance"
    );
  });
}

#[test]
fn user_dca_swap_then_cold_storage_transfer() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let cold_wallet: AccountId = 9999;
    let schedule = timer_schedule(5);
    let foreign = TestAsset::Local(1);
    // Seed mock AMM pool for swap
    setup_pool(foreign, TestAsset::Native, 10_000, 10_000);
    let pool_account: AccountId = u64::MAX;
    set_asset_balance(&pool_account, foreign, 10_000);
    fund_native_raw(&pool_account, 10_000);
    // ExecutionPlan: SwapExactIn(foreign → native) → Transfer(native → cold wallet)
    let execution_plan = BoundedVec::try_from(vec![
      StepOf::<Test> {
        conditions: BoundedVec::try_from(vec![Condition::BalanceAbove {
          asset: foreign,
          threshold: 50,
        }])
        .unwrap(),
        task: Task::SwapExactIn {
          asset_in: foreign,
          asset_out: TestAsset::Native,
          amount_in: AmountResolution::Fixed(100),
          slippage_tolerance: Perbill::from_percent(10),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
      StepOf::<Test> {
        conditions: BoundedVec::try_from(vec![Condition::BalanceAbove {
          asset: TestAsset::Native,
          threshold: 10,
        }])
        .unwrap(),
        task: Task::Transfer {
          to: cold_wallet,
          asset: TestAsset::Native,
          amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(80)),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
    ])
    .unwrap();
    let aaa_id = create_user_with(ALICE, Mutability::Mutable, schedule, None, execution_plan);
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, foreign, 1000);
    fund_native(aaa_id, 5000);
    let cold_before = native_balance(&cold_wallet);
    frame_system::Pallet::<Test>::set_block_number(6);
    AAA::on_initialize(6);
    AAA::on_idle(6, Weight::MAX);
    assert!(
      native_balance(&cold_wallet) > cold_before,
      "Cold storage should receive native after swap + transfer"
    );
    assert!(
      has_aaa_event(|e| matches!(
        e,
        Event::SwapExecuted { aaa_id: id, .. } if *id == aaa_id
      )),
      "Swap should be executed"
    );
  });
}

#[test]
fn user_copybook_savings() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let savings: AccountId = 8888;
    let schedule = timer_schedule(10);
    // Transfer 5% of current native balance to savings
    let execution_plan = execution_plan_with_step(StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 100,
      }])
      .unwrap(),
      task: Task::Transfer {
        to: savings,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(5)),
      },
      on_error: StepErrorPolicy::AbortCycle,
    });
    let aaa_id = create_user_with(ALICE, Mutability::Mutable, schedule, None, execution_plan);
    fund_native(aaa_id, 10000);
    let initial_savings = native_balance(&savings);
    // Execute multiple cycles
    for block in 2..32 {
      frame_system::Pallet::<Test>::set_block_number(block);
      AAA::on_initialize(block);
      AAA::on_idle(block, Weight::MAX);
    }
    assert!(
      native_balance(&savings) > initial_savings,
      "Savings should accumulate"
    );
  });
}

#[test]
fn user_portfolio_rebalancer_both_directions() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let foreign = TestAsset::Local(1);
    let schedule = timer_schedule(5);
    // Step 0: If native > 5000 (spendable), transfer 20% native to BOB
    // Step 1: If spendable native < 500 AND foreign > 500, transfer 50% foreign to CHARLIE
    // cycle_fee_upper for 2 steps ≈ 2*(1+100) = 202
    // BalanceBelow checks spendable = raw - fee_reserve
    let execution_plan = BoundedVec::try_from(vec![
      StepOf::<Test> {
        conditions: BoundedVec::try_from(vec![Condition::BalanceAbove {
          asset: TestAsset::Native,
          threshold: 5000,
        }])
        .unwrap(),
        task: Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(20)),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      StepOf::<Test> {
        conditions: BoundedVec::try_from(vec![
          Condition::BalanceBelow {
            asset: TestAsset::Native,
            threshold: 500,
          },
          Condition::BalanceAbove {
            asset: foreign,
            threshold: 500,
          },
        ])
        .unwrap(),
        task: Task::Transfer {
          to: CHARLIE,
          asset: foreign,
          amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
    ])
    .unwrap();
    let aaa_id = create_user_with(ALICE, Mutability::Mutable, schedule, None, execution_plan);
    let actor = sovereign_account(aaa_id);
    // Phase 1: native high → step 0 fires, step 1 skipped
    fund_native(aaa_id, 10000);
    set_asset_balance(&actor, foreign, 2000);
    frame_system::Pallet::<Test>::set_block_number(6);
    AAA::on_initialize(6);
    AAA::on_idle(6, Weight::MAX);
    assert!(
      has_aaa_event(|e| matches!(
        e,
        Event::TransferExecuted { aaa_id: id, asset: TestAsset::Native, to, .. }
        if *id == aaa_id && *to == BOB
      )),
      "Step 0 should execute when spendable native > 5000"
    );
    // Phase 2: slash native so spendable < 500, keep raw > fee_reserve (202)
    // Set raw native to 600: spendable = 600 - 202 = 398 < 500 ✓
    let actor_native = native_balance(&actor);
    let _ = <Balances as Currency<AccountId>>::slash(&actor, actor_native.saturating_sub(600));
    let charlie_before = asset_balance(&CHARLIE, foreign);
    frame_system::Pallet::<Test>::set_block_number(11);
    AAA::on_initialize(11);
    AAA::on_idle(11, Weight::MAX);
    assert!(
      asset_balance(&CHARLIE, foreign) > charlie_before,
      "Step 1 should execute when spendable native < 500 AND foreign > 500"
    );
  });
}

// --- Multi-Asset Funding Tests ---

#[test]
fn multi_asset_execution_plan_tracks_all_referenced_assets() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let foreign_a = TestAsset::Local(1);
    let foreign_b = TestAsset::Local(2);
    // ExecutionPlan references both assets via PercentageOfLastFunding
    let execution_plan = BoundedVec::try_from(vec![
      StepOf::<Test> {
        conditions: BoundedVec::default(),
        task: Task::Transfer {
          to: BOB,
          asset: foreign_a,
          amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(10)),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
      StepOf::<Test> {
        conditions: BoundedVec::default(),
        task: Task::Transfer {
          to: CHARLIE,
          asset: foreign_b,
          amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(20)),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
    ])
    .unwrap();
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    let funding = actor_funding(aaa_id);
    // Verify both assets are tracked
    assert!(funding.funding_tracked_assets.contains(&foreign_a));
    assert!(funding.funding_tracked_assets.contains(&foreign_b));
  });
}

#[test]
fn manual_readiness_mutates_hot_state_without_rewriting_program() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    let program_before = AAA::actor_program(aaa_id).expect("actor program exists");

    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));

    assert!(
      AAA::actor_hot(aaa_id)
        .expect("actor hot state exists")
        .manual_trigger_pending
    );
    assert_eq!(AAA::actor_program(aaa_id), Some(program_before));
  });
}

#[test]
fn funding_ingress_mutates_only_actor_funding_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      execution_plan_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    let instance_before = AAA::aaa_instances(aaa_id).expect("AAA exists");
    let hot_before = AAA::actor_hot(aaa_id).expect("actor hot state exists");
    let program_before = AAA::actor_program(aaa_id).expect("actor program exists");
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      TestAsset::Native,
      100,
    ));
    assert_eq!(AAA::aaa_instances(aaa_id), Some(instance_before));
    assert_eq!(AAA::actor_hot(aaa_id), Some(hot_before));
    assert_eq!(AAA::actor_program(aaa_id), Some(program_before));
    assert_eq!(
      actor_funding(aaa_id)
        .funding_snapshots
        .get(&TestAsset::Native)
        .expect("funding snapshot exists")
        .amount,
      100
    );
  });
}

#[test]
fn funding_snapshot_isolated_per_asset() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let foreign = TestAsset::Local(1);
    let execution_plan = execution_plan_with_step(StepOf::<Test> {
      conditions: BoundedVec::default(),
      task: Task::Transfer {
        to: BOB,
        asset: foreign,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
      },
      on_error: StepErrorPolicy::AbortCycle,
    });
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    // Mint on ALICE before the ordinary transfer to the sovereign account
    set_asset_balance(&ALICE, foreign, 1000);
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      foreign,
      1000
    ));
    // Verify snapshot
    let funding = actor_funding(aaa_id);
    let snapshot = funding
      .funding_snapshots
      .get(&foreign)
      .expect("snapshot exists");
    assert_eq!(snapshot.amount, 1000);
    // Native should not be tracked (not referenced by PercentageOfLastFunding)
    assert!(!funding.funding_tracked_assets.contains(&TestAsset::Native));
  });
}

#[test]
fn percentage_of_last_funding_multi_asset() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let foreign_a = TestAsset::Local(1);
    let foreign_b = TestAsset::Local(2);
    let execution_plan = BoundedVec::try_from(vec![
      StepOf::<Test> {
        conditions: BoundedVec::default(),
        task: Task::Transfer {
          to: BOB,
          asset: foreign_a,
          amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(10)),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      StepOf::<Test> {
        conditions: BoundedVec::default(),
        task: Task::Transfer {
          to: CHARLIE,
          asset: foreign_b,
          amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(20)),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
    ])
    .unwrap();
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, execution_plan);
    // Mint both assets on ALICE before ordinary transfers to the sovereign account
    set_asset_balance(&ALICE, foreign_a, 1000);
    set_asset_balance(&ALICE, foreign_b, 500);
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      foreign_a,
      1000
    ));
    assert_ok!(ordinary_transfer_to_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      foreign_b,
      500
    ));
    // Execute
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    // Verify transfers: 10% of 1000 = 100 to BOB, 20% of 500 = 100 to CHARLIE
    assert_eq!(asset_balance(&BOB, foreign_a), 100);
    assert_eq!(asset_balance(&CHARLIE, foreign_b), 100);
  });
}

#[test]
fn percentage_modes_excluding_total_supply_remain_supported() {
  new_test_ext().execute_with(|| {
    let asset = TestAsset::Local(42);
    let aaa_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      execution_plan_with_step(make_step(Task::Transfer {
        to: BOB,
        asset,
        amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(10)),
      })),
    );
    let sovereign = sovereign_account(aaa_id);
    set_asset_balance(&sovereign, asset, 1_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::root(), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&BOB, asset), 99);
    assert_eq!(asset_balance(&sovereign, asset), 901);
  });
}

#[test]
fn scheduler_ignores_sparse_id_gaps() {
  // Sparse AAA IDs must not create a scheduler "shadow zone".
  // Create AAA at ID 0, bump NextAaaId to 2000 (huge gap), create AAA at ID 2000.
  // Both must execute in the first block.
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let schedule = timer_schedule(1);
    let execution_plan = inert_execution_plan();
    assert_ok!(AAA::create_system_aaa(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_program(schedule.clone(), None, execution_plan.clone()),
    ));
    let sov_0 = AAA::sovereign_account_id_system(0);
    let _ = Balances::deposit_creating(&sov_0, 1_000_000);
    // Bump NextAaaId to create 2000-wide gap
    crate::pallet::NextAaaId::<Test>::put(2000u64);
    assert_ok!(AAA::create_system_aaa(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_program(schedule, None, execution_plan),
    ));
    let sov_2000 = AAA::sovereign_account_id_system(2000);
    let _ = Balances::deposit_creating(&sov_2000, 1_000_000);
    assert_eq!(AAA::next_aaa_id(), 2001);
    assert!(AAA::aaa_instances(0).is_some());
    assert!(AAA::aaa_instances(2000).is_some());
    // Run one block: both actors must execute despite 2000-wide ID gap
    System::set_block_number(2);
    System::reset_events();
    AAA::on_idle(2, Weight::from_parts(u64::MAX, u64::MAX));
    let executed: alloc::vec::Vec<_> = System::events()
      .iter()
      .filter_map(|r| {
        if let RuntimeEvent::AAA(Event::CycleSummary { aaa_id, .. }) = &r.event {
          Some(*aaa_id)
        } else {
          None
        }
      })
      .collect();
    assert!(
      executed.contains(&0),
      "ID 0 must execute despite sparse AAA IDs"
    );
    assert!(
      executed.contains(&2000),
      "ID 2000 must execute despite sparse AAA IDs"
    );
  });
}

#[test]
fn active_actors_set_maintains_integrity() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let schedule = timer_schedule(1);
    let inert_plan = inert_execution_plan();
    for _ in 0..3 {
      assert_ok!(AAA::create_system_aaa(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_program(schedule.clone(), None, inert_plan.clone()),
      ));
    }
    assert_eq!(ActorHot::<Test>::iter_keys().count(), 3);
    assert!(AAA::aaa_instances(0).is_some());
    assert!(AAA::aaa_instances(1).is_some());
    assert!(AAA::aaa_instances(2).is_some());
    let inst = AAA::aaa_instances(1).unwrap();
    let _ = Balances::deposit_creating(&inst.sovereign_account, 1_000_000);
    assert_ok!(AAA::close_aaa(RuntimeOrigin::root(), 1));
    assert_eq!(ActorHot::<Test>::iter_keys().count(), 2);
    assert!(AAA::aaa_instances(0).is_some());
    assert!(AAA::aaa_instances(2).is_some());
    assert!(AAA::aaa_instances(1).is_none());
  });
}

#[test]
fn scheduler_continues_after_in_loop_close_and_executes_following_ready_actors() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let close_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    let live_id_1 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    let live_id_2 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    fund_native(live_id_1, 1_000);
    fund_native(live_id_2, 1_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), close_id));
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), live_id_1));
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), live_id_2));
    System::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(close_id).is_none());
    assert_eq!(
      AAA::aaa_instances(live_id_1)
        .expect("live actor")
        .cycle_nonce,
      1
    );
    assert_eq!(
      AAA::aaa_instances(live_id_2)
        .expect("live actor")
        .cycle_nonce,
      1
    );
  });
}

#[test]
fn queue_progress_handles_adjacent_removal() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let id0 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    let id1 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    let id2 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    let id3 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    fund_native(id0, 1_000);
    fund_native(id1, 1_000);
    fund_native(id2, 1_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), id0));
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), id1));
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), id2));
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), id3));
    System::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(id3).is_none());
    assert_eq!(AAA::aaa_instances(id0).expect("id0 live").cycle_nonce, 1);
    assert_eq!(AAA::aaa_instances(id1).expect("id1 live").cycle_nonce, 1);
    assert_eq!(
      AAA::aaa_instances(id2).expect("id2 executed").cycle_nonce,
      1
    );
  });
}

#[test]
fn queue_progress_matrix_keeps_progress_and_coverage() {
  for funded_mask in 1u8..=7u8 {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let ids = [
        create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          inert_execution_plan(),
        ),
        create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          inert_execution_plan(),
        ),
        create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          inert_execution_plan(),
        ),
      ];
      for (idx, aaa_id) in ids.iter().enumerate() {
        if (funded_mask & (1 << idx)) != 0 {
          fund_native(*aaa_id, 1_000);
        }
        assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), *aaa_id));
      }
      System::set_block_number(2);
      run_idle(Weight::MAX);
      let expected_started = ids
        .iter()
        .enumerate()
        .filter(|(idx, _)| (funded_mask & (1 << idx)) != 0)
        .count() as u32;
      let started = frame_system::Pallet::<Test>::events()
        .iter()
        .filter(|record| matches!(record.event, RuntimeEvent::AAA(Event::CycleStarted { .. })))
        .count() as u32;
      assert_eq!(started, expected_started);
      for (idx, aaa_id) in ids.iter().enumerate() {
        if (funded_mask & (1 << idx)) != 0 {
          assert_eq!(
            AAA::aaa_instances(*aaa_id)
              .expect("funded actor")
              .cycle_nonce,
            1
          );
        } else {
          assert!(AAA::aaa_instances(*aaa_id).is_none());
        }
      }
    });
  }
}

#[test]
fn auto_close_threshold_reached_closes_actor_after_successful_cycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    assert_ok!(AAA::set_auto_close_at_cycle_nonce(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      Some(2),
    ));
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_some());
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::AaaClosed {
          aaa_id: id,
          reason: CloseReason::AutoCloseNonceReached,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn increment_auto_close_nonce_enforces_bounds_and_overflow_rules() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    assert_noop!(
      AAA::increment_auto_close_nonce(RuntimeOrigin::signed(ALICE), aaa_id, 0),
      Error::<Test>::AutoCloseNonceIncrementZero
    );
    let horizon = TestMaxAutoCloseNonceHorizon::get();
    assert_noop!(
      AAA::increment_auto_close_nonce(
        RuntimeOrigin::signed(ALICE),
        aaa_id,
        horizon.saturating_add(1)
      ),
      Error::<Test>::AutoCloseNonceHorizonExceeded
    );
    assert_ok!(AAA::increment_auto_close_nonce(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      5,
    ));
    let inst = AAA::aaa_instances(aaa_id).expect("AAA must exist");
    assert_eq!(inst.auto_close_at_cycle_nonce, Some(5));
    ActorHot::<Test>::mutate(aaa_id, |maybe| {
      if let Some(hot) = maybe.as_mut() {
        hot.auto_close_at_cycle_nonce = Some(u64::MAX);
      }
    });
    assert_noop!(
      AAA::increment_auto_close_nonce(RuntimeOrigin::signed(ALICE), aaa_id, 1),
      Error::<Test>::AutoCloseNonceOverflow
    );
  });
}

#[test]
fn auto_close_configuration_enforces_origin_mutability_and_target_rules() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let immutable_id = create_user_with(
      ALICE,
      Mutability::Immutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    assert_noop!(
      AAA::set_auto_close_at_cycle_nonce(RuntimeOrigin::signed(ALICE), immutable_id, Some(2)),
      Error::<Test>::ImmutableAaa
    );
    let mutable_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    assert_noop!(
      AAA::set_auto_close_at_cycle_nonce(RuntimeOrigin::signed(BOB), mutable_id, Some(2)),
      Error::<Test>::NotOwner
    );
    assert_ok!(AAA::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      mutable_id
    ));
    run_idle(Weight::MAX);
    assert_noop!(
      AAA::set_auto_close_at_cycle_nonce(RuntimeOrigin::signed(ALICE), mutable_id, Some(1)),
      Error::<Test>::InvalidAutoCloseNonce
    );
    let horizon = TestMaxAutoCloseNonceHorizon::get();
    assert_noop!(
      AAA::set_auto_close_at_cycle_nonce(
        RuntimeOrigin::signed(ALICE),
        mutable_id,
        Some(1u64.saturating_add(horizon).saturating_add(1)),
      ),
      Error::<Test>::AutoCloseNonceHorizonExceeded
    );
    let boundary_target = 1u64.saturating_add(horizon);
    assert_ok!(AAA::set_auto_close_at_cycle_nonce(
      RuntimeOrigin::signed(ALICE),
      mutable_id,
      Some(boundary_target),
    ));
    assert_noop!(
      AAA::increment_auto_close_nonce(RuntimeOrigin::signed(ALICE), mutable_id, 1),
      Error::<Test>::AutoCloseNonceHorizonExceeded
    );
    assert_ok!(AAA::set_auto_close_at_cycle_nonce(
      RuntimeOrigin::signed(ALICE),
      mutable_id,
      None,
    ));
    assert_eq!(
      AAA::aaa_instances(mutable_id)
        .expect("system actor remains active")
        .auto_close_at_cycle_nonce,
      None
    );
  });
}

#[test]
fn deferred_cycle_does_not_consume_auto_close_nonce_target() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    assert_ok!(AAA::set_auto_close_at_cycle_nonce(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      Some(1),
    ));
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(AAA::scheduler_admission_overhead().saturating_add(Weight::from_parts(1, 0)));
    let inst = AAA::aaa_instances(aaa_id).expect("AAA must exist");
    assert_eq!(inst.cycle_nonce, 0);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::CycleDeferred {
          aaa_id: id,
          reason: DeferReason::InsufficientWeightBudget,
        } if *id == aaa_id
      )
    }));
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_none());
  });
}

#[test]
fn close_execution_plan_continues_closure_when_fee_budget_is_missing() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    assert_ok!(AAA::update_on_close_execution_plan(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      inert_execution_plan(),
    ));
    let fee_sink_before = native_balance(&TestFeeSink::get());
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert_eq!(native_balance(&TestFeeSink::get()), fee_sink_before);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::OnCloseStepFailed {
          aaa_id: id,
          step_index: 0,
          kind: OnCloseStepFailureKind::EvaluationFee,
          ..
        } if *id == aaa_id
      )
    }));
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::OnCloseExecutionPlanSummary {
          aaa_id: id,
          executed_steps: 0,
          skipped_steps: 0,
          failed_steps: 1,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn close_step_fee_cannot_debit_beyond_remaining_reservation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    assert_ok!(AAA::update_on_close_execution_plan(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      inert_execution_plan(),
    ));
    let actor = sovereign_account(aaa_id);
    fund_native_raw(&actor, 1_000);
    let actor_before = native_balance(&actor);
    let sink_before = native_balance(&TestFeeSink::get());
    let instance = AAA::aaa_instances(aaa_id).expect("AAA exists");
    AAA::execute_on_close_execution_plan(aaa_id, &instance, 0);
    assert_eq!(native_balance(&actor), actor_before);
    assert_eq!(native_balance(&TestFeeSink::get()), sink_before);
    assert!(has_aaa_event(|event| matches!(
      event,
      Event::OnCloseStepFailed {
        aaa_id: id,
        step_index: 0,
        kind: OnCloseStepFailureKind::EvaluationFee,
        ..
      } if *id == aaa_id
    )));
  });
}

#[test]
fn close_step_skips_report_condition_resolution_and_funding_outcomes() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(90);
    let condition_skip = StepOf::<Test> {
      conditions: BoundedVec::try_from(vec![Condition::BalanceAbove {
        asset,
        threshold: 100,
      }])
      .expect("one condition fits"),
      task: Task::Stake {
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(0),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let close_plan = BoundedVec::try_from(vec![
      condition_skip,
      make_step(Task::Transfer {
        to: BOB,
        asset,
        amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(1)),
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset,
        amount: AmountResolution::Fixed(10),
      }),
    ])
    .expect("system close plan fits");
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    let actor = sovereign_account(aaa_id);
    set_asset_balance(&actor, asset, 2);
    assert_ok!(AAA::update_on_close_execution_plan(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      close_plan,
    ));
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    for (step_index, reason) in [
      (0, StepSkippedReason::ConditionsNotMet),
      (1, StepSkippedReason::ResolutionSkipped),
      (2, StepSkippedReason::FundingUnavailable),
    ] {
      assert!(has_aaa_event(|event| matches!(
        event,
        Event::OnCloseStepSkipped {
          aaa_id: id,
          step_index: index,
          reason: actual_reason,
        } if *id == aaa_id && *index == step_index && *actual_reason == reason
      )));
    }
    assert!(has_aaa_event(|event| matches!(
      event,
      Event::OnCloseExecutionPlanSummary {
        aaa_id: id,
        executed_steps: 0,
        skipped_steps: 3,
        failed_steps: 0,
      } if *id == aaa_id
    )));
  });
}

#[test]
fn close_admission_bound_includes_cleanup_in_both_weight_dimensions() {
  new_test_ext().execute_with(|| {
    let aaa_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    let instance = AAA::aaa_instances(aaa_id).expect("AAA exists");
    let plan_weight = AAA::compute_cycle_weight_upper(
      instance.actor_class.aaa_type(),
      &instance.on_close_execution_plan,
    );
    let cleanup_weight = AAA::close_cleanup_weight_upper();
    let admitted_weight = AAA::close_cycle_weight_upper_bound(&instance);
    let dispatch_weight = AAA::close_dispatch_weight_upper();
    assert!(cleanup_weight.ref_time() > 0);
    assert!(cleanup_weight.proof_size() > 0);
    assert!(
      cleanup_weight.all_gte(<() as crate::WeightInfo>::scheduler_paged_consume_preserve_page())
    );
    assert_eq!(admitted_weight, plan_weight.saturating_add(cleanup_weight));
    assert!(dispatch_weight.all_gte(admitted_weight));
    assert!(
      dispatch_weight.all_gte(<() as crate::WeightInfo>::close_aaa_user_fee_bearing_tail(
        <<Test as crate::Config>::MaxUserExecutionPlanSteps as Get<u32>>::get(),
        <<Test as crate::Config>::MaxSplitTransferLegs as Get<u32>>::get(),
      ))
    );
  });
}

#[test]
fn automatic_close_defers_until_budget_can_admit_close_tail() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    let sovereign = sovereign_account(aaa_id);
    let cleanup_asset = TestAsset::Local(88);
    fund_native_raw(&sovereign, 1_000);
    set_asset_balance(&sovereign, cleanup_asset, 500);
    let on_close_execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: ALICE,
      asset: cleanup_asset,
      amount: AmountResolution::AllBalance,
    }));
    assert_ok!(AAA::update_on_close_execution_plan(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      on_close_execution_plan,
    ));
    assert_ok!(AAA::set_auto_close_at_cycle_nonce(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      Some(1),
    ));
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(AAA::scheduler_admission_overhead().saturating_add(Weight::from_parts(1, 0)));
    let inst =
      AAA::aaa_instances(aaa_id).expect("AAA must remain active until close-tail budget fits");
    assert_eq!(inst.cycle_nonce, 0);
    assert_eq!(asset_balance(&sovereign, cleanup_asset), 500);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::CycleDeferred {
          aaa_id: id,
          reason: DeferReason::InsufficientWeightBudget,
        } if *id == aaa_id
      )
    }));
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert_eq!(asset_balance(&sovereign, cleanup_asset), 1);
    assert_eq!(asset_balance(&ALICE, cleanup_asset), 499);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::OnCloseExecutionPlanSummary {
          aaa_id: id,
          executed_steps: 1,
          skipped_steps: 0,
          failed_steps: 0,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn update_on_close_execution_plan_can_cleanup_assets_via_transfer_all() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    let sovereign = sovereign_account(aaa_id);
    let cleanup_asset = TestAsset::Local(42);
    fund_native_raw(&sovereign, 1_000);
    set_asset_balance(&sovereign, cleanup_asset, 500);
    assert_eq!(asset_balance(&ALICE, cleanup_asset), 0);
    let on_close_execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: ALICE,
      asset: cleanup_asset,
      amount: AmountResolution::AllBalance,
    }));
    assert_ok!(AAA::update_on_close_execution_plan(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      on_close_execution_plan,
    ));
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert_eq!(asset_balance(&sovereign, cleanup_asset), 1);
    assert_eq!(asset_balance(&ALICE, cleanup_asset), 499);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::OnCloseExecutionPlanSummary {
          aaa_id: id,
          executed_steps: 1,
          skipped_steps: 0,
          failed_steps: 0,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn on_close_execution_plan_executes_swap_step() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    let sovereign = sovereign_account(aaa_id);
    let foreign = TestAsset::Local(77);
    let pool_account: AccountId = u64::MAX;
    setup_pool(TestAsset::Native, foreign, 10_000, 10_000);
    fund_native_raw(&pool_account, 10_000);
    set_asset_balance(&pool_account, foreign, 10_000);
    fund_native_raw(&sovereign, 1_000);
    let on_close_execution_plan = execution_plan_with_step(make_step(Task::SwapExactIn {
      asset_in: TestAsset::Native,
      asset_out: foreign,
      amount_in: AmountResolution::Fixed(100),
      slippage_tolerance: Perbill::from_percent(10),
    }));
    assert_ok!(AAA::update_on_close_execution_plan(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      on_close_execution_plan,
    ));
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert!(asset_balance(&sovereign, foreign) > 0);
    assert!(has_aaa_event(|event| {
      matches!(event, Event::SwapExecuted { aaa_id: id, .. } if *id == aaa_id)
    }));
  });
}

#[test]
fn on_close_dex_late_failure_rolls_back_task_but_not_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    let sovereign = sovereign_account(aaa_id);
    let foreign = TestAsset::Local(78);
    let pool_account: AccountId = u64::MAX;
    setup_pool(TestAsset::Native, foreign, 10_000, 10_000);
    set_asset_balance(&pool_account, foreign, 10_000);
    fund_native_raw(&sovereign, 1_000);
    let close_plan = execution_plan_with_step(make_step(Task::SwapExactIn {
      asset_in: TestAsset::Native,
      asset_out: foreign,
      amount_in: AmountResolution::Fixed(100),
      slippage_tolerance: Perbill::from_percent(10),
    }));
    assert_ok!(AAA::update_on_close_execution_plan(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      close_plan,
    ));
    let instance = AAA::aaa_instances(aaa_id).expect("AAA exists");
    let close_fee_upper = AAA::close_cycle_fee_upper_bound(&instance);
    let pool_native_before = native_balance(&pool_account);
    set_fail_dex_after_input_transfer(true);
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert_eq!(native_balance(&sovereign), 1_000 - close_fee_upper);
    assert_eq!(asset_balance(&sovereign, foreign), 0);
    assert_eq!(native_balance(&pool_account), pool_native_before);
    assert_eq!(asset_balance(&pool_account, foreign), 10_000);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::OnCloseStepFailed { aaa_id: id, step_index: 0, .. } if *id == aaa_id
      )
    }));
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::OnCloseExecutionPlanSummary {
          aaa_id: id,
          executed_steps: 0,
          skipped_steps: 0,
          failed_steps: 1,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn on_close_execution_plan_uses_frozen_close_snapshot_with_reserved_user_fee_budget() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    let sovereign = sovereign_account(aaa_id);
    let fee_sink = TestFeeSink::get();
    let close_funding = 501u128;
    fund_native_raw(&sovereign, close_funding);
    let fee_sink_before = native_balance(&fee_sink);
    let close_plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfTrigger(Perbill::from_percent(50)),
      }),
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfTrigger(Perbill::from_percent(50)),
      }),
    ])
    .expect("close plan fits");
    assert_ok!(AAA::update_on_close_execution_plan(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      close_plan,
    ));
    let instance = AAA::aaa_instances(aaa_id).expect("AAA exists");
    let close_fee_upper = AAA::close_cycle_fee_upper_bound(&instance);
    let trigger_spendable = close_funding.saturating_sub(close_fee_upper);
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert_eq!(
      native_balance(&BOB),
      TEST_INITIAL_BALANCE + trigger_spendable / 2
    );
    assert_eq!(
      native_balance(&CHARLIE),
      TEST_INITIAL_BALANCE + trigger_spendable / 2
    );
    assert_eq!(
      native_balance(&fee_sink),
      fee_sink_before.saturating_add(close_fee_upper)
    );
    assert_eq!(native_balance(&sovereign), 1);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::OnCloseExecutionPlanSummary {
          aaa_id: id,
          executed_steps: 2,
          skipped_steps: 0,
          failed_steps: 0,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn on_close_execution_plan_failures_do_not_block_closure() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    let cleanup_asset = TestAsset::Local(7);
    let sovereign = sovereign_account(aaa_id);
    fund_native_raw(&sovereign, 1_000);
    set_asset_balance(&sovereign, cleanup_asset, 1_000);
    let failing_on_close_execution_plan = execution_plan_with_step(make_step(Task::Stake {
      asset: cleanup_asset,
      amount: AmountResolution::Fixed(900),
    }));
    assert_ok!(AAA::update_on_close_execution_plan(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      failing_on_close_execution_plan,
    ));
    set_fail_staking_ops(true);
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    set_fail_staking_ops(false);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::OnCloseStepFailed {
          aaa_id: id,
          step_index: 0,
          ..
        } if *id == aaa_id
      )
    }));
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::OnCloseExecutionPlanSummary {
          aaa_id: id,
          executed_steps: 0,
          skipped_steps: 0,
          failed_steps: 1,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn close_execution_plan_fee_sink_transfer_failure_is_observable_and_non_blocking() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    let sovereign = sovereign_account(aaa_id);
    let cleanup_asset = TestAsset::Local(9);
    let fee_sink_before = native_balance(&TestFeeSink::get());
    fund_native_raw(&sovereign, 1_000);
    set_asset_balance(&sovereign, cleanup_asset, 500);
    let close_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: ALICE,
      asset: cleanup_asset,
      amount: AmountResolution::Fixed(499),
    }));
    assert_ok!(AAA::update_on_close_execution_plan(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      close_plan,
    ));
    set_fail_fee_sink_transfer(true);
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    set_fail_fee_sink_transfer(false);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert_eq!(native_balance(&TestFeeSink::get()), fee_sink_before);
    assert_eq!(asset_balance(&sovereign, cleanup_asset), 500);
    assert_eq!(asset_balance(&ALICE, cleanup_asset), 0);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::OnCloseStepFailed {
          aaa_id: id,
          step_index: 0,
          ..
        } if *id == aaa_id
      )
    }));
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::OnCloseExecutionPlanSummary {
          aaa_id: id,
          executed_steps: 0,
          skipped_steps: 0,
          failed_steps: 1,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn update_on_close_execution_plan_accepts_full_execution_plan_surface() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    let dex_close_execution_plan = execution_plan_with_step(make_step(Task::SwapExactIn {
      asset_in: TestAsset::Native,
      asset_out: TestAsset::Local(1),
      amount_in: AmountResolution::AllBalance,
      slippage_tolerance: Perbill::from_percent(1),
    }));
    assert_ok!(AAA::update_on_close_execution_plan(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      dex_close_execution_plan,
    ));
    let stateful_amount_execution_plan = execution_plan_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    assert_ok!(AAA::update_on_close_execution_plan(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      stateful_amount_execution_plan,
    ));
  });
}

#[test]
fn update_on_close_execution_plan_rejects_mint_for_user_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    let mint_close_execution_plan = execution_plan_with_step(make_step(Task::Mint {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    }));
    assert_noop!(
      AAA::update_on_close_execution_plan(
        RuntimeOrigin::signed(ALICE),
        aaa_id,
        mint_close_execution_plan
      ),
      Error::<Test>::MintNotAllowedForUserAaa
    );
  });
}

#[test]
fn system_immutable_rejects_runtime_control_paths_even_for_root() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let execution_plan = inert_execution_plan();
    assert_ok!(AAA::create_system_aaa(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Immutable,
      system_active_program(manual_schedule(), None, execution_plan.clone()),
    ));
    let aaa_id = AAA::next_aaa_id().saturating_sub(1);
    assert_eq!(
      AAA::aaa_instances(aaa_id).expect("AAA exists").mutability,
      Mutability::Immutable
    );
    assert_noop!(
      AAA::update_schedule(RuntimeOrigin::root(), aaa_id, timer_schedule(1), None),
      Error::<Test>::ImmutableAaa
    );
    assert_noop!(
      AAA::update_execution_plan(RuntimeOrigin::root(), aaa_id, execution_plan.clone()),
      Error::<Test>::ImmutableAaa
    );
    assert_noop!(
      AAA::update_on_close_execution_plan(RuntimeOrigin::root(), aaa_id, execution_plan),
      Error::<Test>::ImmutableAaa
    );
    assert_noop!(
      AAA::pause_aaa(RuntimeOrigin::root(), aaa_id),
      Error::<Test>::ImmutableAaa
    );
    assert_noop!(
      AAA::resume_aaa(RuntimeOrigin::root(), aaa_id),
      Error::<Test>::ImmutableAaa
    );
    assert_noop!(
      AAA::manual_trigger(RuntimeOrigin::root(), aaa_id),
      Error::<Test>::ImmutableAaa
    );
    assert_noop!(
      AAA::close_aaa(RuntimeOrigin::root(), aaa_id),
      Error::<Test>::ImmutableAaa
    );
    assert_noop!(
      AAA::reopen_system_aaa(
        RuntimeOrigin::root(),
        aaa_id,
        ALICE,
        Mutability::Immutable,
        system_active_program(manual_schedule(), None, inert_execution_plan()),
      ),
      Error::<Test>::ImmutableAaa
    );
    assert!(AAA::aaa_instances(aaa_id).is_some());
    assert!(!crate::pallet::ClosedSystemAaaIds::<Test>::contains_key(
      aaa_id
    ));
  });
}

#[test]
fn system_immutable_creation_rejects_schedule_window() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_noop!(
      AAA::create_system_aaa(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Immutable,
        system_active_program(
          manual_schedule(),
          Some(ScheduleWindow { start: 1, end: 101 }),
          inert_execution_plan(),
        ),
      ),
      Error::<Test>::InvalidScheduleWindow
    );
  });
}

#[test]
fn default_close_plan_is_empty_for_created_actors() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let user_id = create_user_with(
      ALICE,
      Mutability::Immutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    let system_id = create_system_with(ALICE, manual_schedule(), None, inert_execution_plan());
    for aaa_id in [user_id, system_id] {
      let instance = AAA::aaa_instances(aaa_id).expect("AAA exists");
      assert!(instance.on_close_execution_plan.is_empty());
    }
  });
}

#[test]
fn close_tail_does_not_start_normal_cycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let aaa_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_execution_plan(),
    );
    let sovereign = sovereign_account(aaa_id);
    fund_native_raw(&sovereign, 1_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    let before_close_nonce = AAA::aaa_instances(aaa_id).expect("AAA exists").cycle_nonce;
    assert_eq!(before_close_nonce, 1);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(!has_aaa_event(|event| {
      matches!(event, Event::CycleStarted { aaa_id: id, .. } if *id == aaa_id)
    }));
    assert!(!has_aaa_event(|event| {
      matches!(event, Event::CycleSummary { aaa_id: id, .. } if *id == aaa_id)
    }));
    assert!(has_aaa_event(|event| {
      matches!(event, Event::OnCloseExecutionPlanSummary { aaa_id: id, .. } if *id == aaa_id)
    }));
  });
}

#[cfg(test)]
mod proptest_aaa {
  use crate::{
    ActorHot, AmountResolution, Mutability, Schedule, ScheduleOf, StepErrorPolicy, StepOf, Task,
    Trigger, mock::*,
  };
  use polkadot_sdk::frame_support::{BoundedVec, assert_ok, traits::Hooks};
  use polkadot_sdk::{frame_system, sp_runtime::Weight};
  use proptest::prelude::*;

  type RuntimeSchedule = ScheduleOf<Test>;
  type RuntimeStep = StepOf<Test>;

  fn timer_schedule_pt(every_blocks: u32) -> RuntimeSchedule {
    Schedule {
      trigger: Trigger::Timer { every_blocks },
      cooldown_blocks: 0,
    }
  }

  fn inert_execution_plan() -> crate::ExecutionPlanOf<Test> {
    BoundedVec::try_from(vec![RuntimeStep {
      conditions: BoundedVec::default(),
      task: Task::Stake {
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(0),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }])
    .expect("execution_plan must fit")
  }

  fn create_timer_actor(owner: AccountId, every_blocks: u32) -> u64 {
    let id = AAA::next_aaa_id();
    assert_ok!(AAA::create_user_aaa(
      RuntimeOrigin::signed(owner),
      Mutability::Mutable,
      crate::ProgramInput::Active {
        schedule: timer_schedule_pt(every_blocks),
        schedule_window: None,
        execution_plan: inert_execution_plan(),
        on_close_execution_plan: Default::default(),
        funding_source_policy: crate::FundingSourcePolicy::OwnerOnly,
      },
    ));
    id
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
          let aaa_id = create_timer_actor(owner, 1);
          let sovereign = AAA::aaa_instances(aaa_id)
            .expect("must exist")
            .sovereign_account;
          let _ = <Balances as frame::traits::Currency<AccountId>>::deposit_creating(
            &sovereign, 10_000_000,
          );
          actor_ids.push(aaa_id);
        }
        let max_blocks = (actor_count * 3) as u64;
        let mut executed: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
        for block in 1..=max_blocks {
          frame_system::Pallet::<Test>::set_block_number(block);
          AAA::on_idle(block, Weight::MAX);
          for &aaa_id in &actor_ids {
            if let Some(instance) = AAA::aaa_instances(aaa_id) {
              if instance.cycle_nonce > 0 {
                executed.insert(aaa_id);
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
            let aaa_id = create_timer_actor(owner, 1);
            let sovereign = AAA::aaa_instances(aaa_id)
              .expect("must exist")
              .sovereign_account;
            let _ = <Balances as frame::traits::Currency<AccountId>>::deposit_creating(
              &sovereign, 10_000_000,
            );
            actor_ids.push((aaa_id, owner));
          }
          let after_create = ActorHot::<Test>::iter_keys().count();
          let close_count = closes.min(creates);
          for i in 0..close_count {
            let (aaa_id, owner) = actor_ids[i as usize];
            assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(owner), aaa_id));
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
}
