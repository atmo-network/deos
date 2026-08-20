use crate::{
  ActiveLifecycle, ActorClass, ActorClassification, ActorClassificationError, ActorContract,
  ActorContracts, ActorEligibility, ActorExecutionPhase, ActorFunding, ActorHot, ActorId,
  ActorIdentities, ActorType, AmountResolution, AssetFilter, AssetFilterOf, AttemptDisposition,
  CancellationReason, CloseReason, ContinuationStateStore, CycleResult, CycleState, Error, Event,
  FeeChargeKind, FeeEnvelopeError, FeeEnvelopeInput, FundingSourcePolicy, GlobalCircuitBreaker,
  IdleStarvationPhase, IdleStarvationState, InitialLifecycle, InputLimit, Mutability, NextActorId,
  ObservationSubscriberPageList, ObservationTiming, OpeningSurface, OutcomeTotals,
  OwnerSlotBitmaps, Precondition, Predicate, QueueEntry, QueueHead, QueueOccupancy, QueuePages,
  QueueTail, RetryClass, ScheduleWindow, SimulationError, SimulationMode, SimulationStepRecord,
  SourceFilter, SourceFilterOf, SovereignIndex, SplitLeg, SplitTransferLegsOf, StepErrorPolicy,
  StepOf, StepOutcome, StepSkippedReason, SuspensionReason, SystemSovereignState, Task,
  TaskFailure, TaskOf, TimedPredicate, Trigger, WakeupBucketState, WakeupBuckets, WakeupClock,
  WakeupEntry, WakeupKey, WakeupPage, WakeupPages, WakeupPointer, adapters::AssetOps,
  compose_attempt_fee_envelope, fee_native_protected_minimum, mock::*, settle_attempt_fee_step,
};
use alloc::collections::BTreeSet;

const RETRY_LATER: StepErrorPolicy = StepErrorPolicy::RetryLater { max_attempts: 10 };

fn update_contract_parts(actor_id: ActorId) -> crate::ActorContractOf<Test> {
  ActorContracts::<Test>::get(actor_id).expect("Actor Contract exists")
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
type RuntimeContinuationState = crate::ContinuationStateOf<Test>;
type MockBlockNumber = polkadot_sdk::frame_system::pallet_prelude::BlockNumberFor<Test>;
type TestWeightInfo = crate::weights::TestWeightInfo;

fn scheduled_wakeup_block(actor_id: crate::ActorId) -> Option<MockBlockNumber> {
  Actors::actor_hot(actor_id).and_then(|hot| match hot.wakeup_pointer?.block {
    WakeupKey::Block(block) => Some(block),
    WakeupKey::Tick(tick) => Some(tick),
  })
}

fn seed_saturated_tombstone_queue() {
  let page_size: u32 = <Test as crate::Config>::QueuePageSize::get();
  let capacity: u32 = <Test as crate::Config>::MaxQueueLength::get();
  for page_id in 0..capacity.div_ceil(page_size) {
    let first = page_id * page_size;
    let len = page_size.min(capacity - first);
    let entries = (0..len)
      .map(|offset| QueueEntry {
        ticket: u64::from(first + offset),
        actor_id: 10_000_000 + u64::from(first + offset),
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

#[test]
fn trigger_grammar_is_single_source_and_non_nested() {
  let manual = RuntimeTrigger::manual();
  let address = RuntimeTrigger::address_event(SourceFilter::OwnerOnly, AssetFilter::Any);
  let observation = RuntimeTrigger::observation_change(7);
  let cadence = RuntimeTrigger::cadenced(10);

  assert_eq!(observation.encode(), vec![2, 7, 0, 0, 0]);
  assert!(manual.manual_source_enabled());
  assert!(address.address_event_source_enabled());
  assert!(observation.observation_source_enabled());
  assert_eq!(cadence.cadence_ticks(), Some(10));
  assert!(!cadence.manual_source_enabled());
  assert!(!cadence.address_event_source_enabled());
  assert!(!cadence.observation_source_enabled());

  for trigger in [manual, address, observation, cadence] {
    assert!(trigger.has_canonical_filters());
    let encoded = trigger.encode();
    assert!(encoded.len() <= RuntimeTrigger::max_encoded_len());
    assert_eq!(RuntimeTrigger::decode(&mut &encoded[..]), Ok(trigger));
  }

  let non_canonical_filter = RuntimeTrigger::address_event(
    SourceFilter::Whitelist(BoundedVec::try_from(vec![2, 1]).expect("within whitelist bound")),
    AssetFilter::Any,
  );
  assert!(!non_canonical_filter.has_canonical_filters());
}

#[test]
fn percentage_at_opening_is_independent_from_trigger_kind_and_payload() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let plan = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageAtOpening(Perbill::from_percent(50)),
    }));
    for schedule in [manual_schedule(), observation_schedule(vec![1])] {
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(schedule, None, plan.clone()),
      ));
    }
    let actor_id = Actors::next_actor_id().saturating_sub(1);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      percentage_trigger_schedule(),
      None,
    ));
  });
}

#[test]
fn observation_only_sources_admit_non_trigger_amount_resolutions() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let plan = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
    }));
    let actor_id = create_system_with(ALICE, observation_schedule(vec![4]), None, plan);
    assert_eq!(Actors::observation_subscriber_count(4), 1);
    assert!(Actors::actor_contract(actor_id).is_some());
  });
}

#[test]
fn observation_subscriptions_follow_schedule_lifecycle_exactly() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      observation_schedule(vec![1]),
      None,
      inert_contract_steps(),
    );
    let slot = Actors::observation_subscription_slot(actor_id).expect("subscription slot");
    assert_eq!(
      Actors::actor_observation_feeds(actor_id),
      Some(BoundedVec::truncate_from(vec![1]))
    );
    assert_eq!(Actors::observation_subscription_count(), 1);
    assert_eq!(Actors::observation_subscriber_count(1), 1);
    assert_eq!(Actors::observation_ingress_revision(1), None);
    assert_eq!(Actors::observation_ingress_revision(2), None);
    assert!(Actors::dirty_observation_feeds(1).is_none());
    assert!(Actors::dirty_observation_feeds(2).is_none());
    let page_size: u32 = <Test as crate::Config>::ObservationPageSize::get();
    let page_id = slot / page_size;
    let offset = (slot % page_size) as usize;
    assert_eq!(
      Actors::observation_subscriber_pages(1, page_id).expect("subscriber page")[offset],
      Some(actor_id)
    );
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      observation_schedule(vec![3]),
      None,
    ));
    assert_eq!(
      Actors::actor_observation_feeds(actor_id),
      Some(BoundedVec::truncate_from(vec![3]))
    );
    assert_eq!(Actors::observation_subscriber_count(1), 0);
    assert_eq!(Actors::observation_subscriber_count(3), 1);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      transfer_contract_steps(BOB, 1),
      crate::CompletionPolicy::Persistent,
    ));
    assert_eq!(
      Actors::actor_observation_feeds(actor_id),
      Some(BoundedVec::truncate_from(vec![3]))
    );
    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(Actors::actor_observation_feeds(actor_id).is_none());
    assert!(Actors::observation_subscription_slot(actor_id).is_none());
    assert_eq!(Actors::observation_subscription_count(), 0);
    assert_eq!(crate::ObservationFreeSlotLen::<Test>::get(), 1);
    frame_system::Pallet::<Test>::set_block_number(4);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      {
        let schedule = observation_schedule(vec![4]);
        ActorContract {
          trigger: schedule.trigger,
          cooldown_blocks: schedule.cooldown_blocks,
          window: None,
          steps: inert_contract_steps(),
          completion: crate::CompletionPolicy::Persistent,
          funding: FundingSourcePolicy::OwnerOnly,
          auto_close_at_cycle_nonce: None,
        }
      },
    ));
    assert_eq!(Actors::observation_subscription_slot(actor_id), Some(slot));
    assert_eq!(crate::ObservationFreeSlotLen::<Test>::get(), 0);
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(Actors::observation_subscription_count(), 0);
    assert_eq!(Actors::observation_subscriber_count(4), 0);
    assert!(Actors::observation_subscriber_pages(4, page_id).is_none());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn observation_occupied_page_list_follows_live_pages_after_fragmentation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size = <Test as crate::Config>::ObservationPageSize::get();
    let mut actors = Vec::new();
    for _ in 0..=page_size {
      actors.push(create_system_with(
        ALICE,
        observation_schedule(vec![17]),
        None,
        inert_contract_steps(),
      ));
    }

    assert_eq!(
      Actors::observation_subscriber_page_list(17),
      Some(ObservationSubscriberPageList {
        head: 0,
        tail: 1,
        count: 2,
      })
    );
    let first = Actors::observation_subscriber_pages(17, 0).expect("first occupied page");
    let second = Actors::observation_subscriber_pages(17, 1).expect("second occupied page");
    assert_eq!((first.previous, first.next), (None, Some(1)));
    assert_eq!((second.previous, second.next), (Some(0), None));

    let remaining_actor = *actors.last().expect("one actor remains");
    for actor_id in actors.iter().copied().take(page_size as usize) {
      assert_ok!(Actors::deactivate_actor(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
    }

    assert!(Actors::observation_subscriber_pages(17, 0).is_none());
    assert_eq!(
      Actors::observation_subscriber_page_list(17),
      Some(ObservationSubscriberPageList {
        head: 1,
        tail: 1,
        count: 1,
      })
    );
    let remaining = Actors::observation_subscriber_pages(17, 1).expect("remaining occupied page");
    assert_eq!((remaining.previous, remaining.next), (None, None));

    assert_ok!(Actors::note_observation_changed(17, 1));
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    assert_eq!(
      Actors::fanout_dirty_observations(base.saturating_add(unit)),
      base.saturating_add(unit)
    );
    assert!(Actors::dirty_observation_feeds(17).is_none());
    assert!(Actors::pending_signal(remaining_actor));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn observation_provider_mutation_without_certified_ingress_has_no_actor_effect() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let feed = 1;
    let actor_id = create_system_with(
      ALICE,
      observation_schedule(vec![feed]),
      None,
      inert_contract_steps(),
    );

    set_observation(
      feed,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );

    assert!(!Actors::pending_signal(actor_id));
    assert_eq!(Actors::observation_ingress_revision(feed), None);
    assert!(Actors::dirty_observation_feeds(feed).is_none());
    assert_eq!(Actors::dirty_observation_feed_count(), 0);
  });
}

#[test]
fn stale_observation_subscriber_page_fails_closed_without_losing_dirty_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      observation_schedule(vec![19]),
      None,
      inert_contract_steps(),
    );
    let slot = Actors::observation_subscription_slot(actor_id).expect("subscription slot");
    let page_size: u32 = <Test as crate::Config>::ObservationPageSize::get();
    let page_id = slot / page_size;
    assert_ok!(Actors::note_observation_changed(19, 1));
    crate::ObservationSubscriberPages::<Test>::remove(19, page_id);
    let dirty_before = Actors::dirty_observation_feeds(19).expect("dirty feed");
    let list_before = Actors::dirty_observation_list();
    let events_before = System::events();

    assert_eq!(
      crate::Pallet::<Test>::do_fanout_dirty_observation_page(),
      Err(Error::<Test>::DirtyObservationInvariant.into())
    );
    assert_eq!(Actors::dirty_observation_feeds(19), Some(dirty_before));
    assert_eq!(Actors::dirty_observation_list(), list_before);
    assert!(!Actors::pending_signal(actor_id));
    assert_eq!(System::events(), events_before);
  });
}

#[test]
fn observation_change_ingress_coalesces_latest_revision_without_subscriber_work() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_noop!(
      Actors::note_observation_changed(1, 0),
      Error::<Test>::InvalidObservationRevision
    );
    assert_ok!(Actors::note_observation_changed(1, 1));
    assert_eq!(Actors::observation_ingress_revision(1), None);
    assert!(Actors::dirty_observation_feeds(1).is_none());
    assert_eq!(Actors::dirty_observation_feed_count(), 0);
    assert_eq!(Actors::dirty_observation_list(), Default::default());

    let actor_id = create_system_with(
      ALICE,
      observation_schedule(vec![1]),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::note_observation_changed(1, 1));
    assert_eq!(Actors::observation_ingress_revision(1), Some(1));
    assert!(!Actors::pending_signal(actor_id));
    let initial = Actors::dirty_observation_feeds(1).expect("dirty feed");
    assert_eq!(initial.latest_revision, 1);
    assert_eq!(initial.fanout_revision, 0);
    assert_eq!(initial.dirty_since, 1);
    assert_eq!(initial.next_subscriber_page, None);
    frame_system::Pallet::<Test>::set_block_number(5);
    assert_ok!(Actors::note_observation_changed(1, 1));
    assert_eq!(Actors::dirty_observation_feeds(1), Some(initial));
    frame_system::Pallet::<Test>::set_block_number(8);
    assert_ok!(Actors::note_observation_changed(1, 3));
    assert_eq!(Actors::observation_ingress_revision(1), Some(3));
    let coalesced = Actors::dirty_observation_feeds(1).expect("coalesced dirty feed");
    assert_eq!(coalesced.latest_revision, 3);
    assert_eq!(coalesced.dirty_since, 1);
    assert_noop!(
      Actors::note_observation_changed(1, 2),
      Error::<Test>::InvalidObservationRevision
    );
    assert_eq!(Actors::dirty_observation_feed_count(), 1);
    assert_eq!(Actors::dirty_observation_list().head, Some(1));
    assert_eq!(Actors::dirty_observation_list().tail, Some(1));
    assert_eq!(Actors::dirty_observation_list().cursor, Some(1));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());

    assert_ok!(crate::Pallet::<Test>::clear_dirty_observation_feed(1));
    assert_eq!(Actors::observation_ingress_revision(1), Some(3));
    assert_noop!(
      Actors::note_observation_changed(1, 2),
      Error::<Test>::InvalidObservationRevision
    );
    frame_system::Pallet::<Test>::set_block_number(13);
    assert_ok!(Actors::note_observation_changed(1, 4));
    assert_eq!(Actors::observation_ingress_revision(1), Some(4));
    assert_eq!(
      Actors::dirty_observation_feeds(1)
        .expect("new dirty interval")
        .dirty_since,
      13
    );
  });
}

#[test]
fn last_subscription_cleanup_unlinks_exact_dirty_feed() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let first = create_system_with(
      ALICE,
      observation_schedule(vec![7]),
      None,
      inert_contract_steps(),
    );
    let second = create_system_with(
      ALICE,
      observation_schedule(vec![7]),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::note_observation_changed(7, 1));
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      first
    ));
    assert_eq!(Actors::observation_subscriber_count(7), 1);
    assert!(Actors::dirty_observation_feeds(7).is_some());
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      second
    ));
    assert_eq!(Actors::observation_subscriber_count(7), 0);
    assert_eq!(Actors::observation_ingress_revision(7), None);
    assert!(Actors::dirty_observation_feeds(7).is_none());
    assert_eq!(Actors::dirty_observation_feed_count(), 0);
    assert_eq!(Actors::dirty_observation_list(), Default::default());

    create_system_with(
      ALICE,
      observation_schedule(vec![8]),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::note_observation_changed(8, 4));
    assert_eq!(Actors::dirty_observation_list().head, Some(8));
    assert_eq!(Actors::dirty_observation_list().tail, Some(8));
    assert_eq!(Actors::dirty_observation_list().cursor, Some(8));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn same_block_wakeup_precedes_fanout_in_ticket_order() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // A timer actor with a due wakeup and an observation subscriber with a dirty feed
    // in the same block: the on_idle phase order (wakeups before fanout) must give the
    // wakeup-eligible actor a strictly earlier queue ticket than the fanout-signaled
    // subscriber (spec 8.2.1). We observe this through the execution order of the two
    // one-shot transfers: the wakeup actor's transfer must precede the fanout actor's.
    let wakeup_id = create_system_with(
      ALICE,
      timer_schedule(3),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(wakeup_id, 1_000);
    let subscriber_id = create_system_with(
      ALICE,
      observation_schedule(vec![7]),
      None,
      transfer_contract_steps(CHARLIE, 10),
    );
    fund_native(subscriber_id, 1_000);
    // The timer's first wakeup fires at block 4 (anchor 1 + 3); the observation change
    // lands at block 4 too, so both are due in the same on_idle pass.
    frame_system::Pallet::<Test>::set_block_number(4);
    assert_ok!(Actors::note_observation_changed(7, 1));
    assert_eq!(scheduled_wakeup_block(wakeup_id), Some(4));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    let wakeup_pos = events
      .iter()
      .position(|event| matches!(
        event,
        Event::TransferExecuted { actor_id: id, to, .. } if *id == wakeup_id && *to == BOB
      ))
      .expect("wakeup actor transfer executed");
    let fanout_pos = events
      .iter()
      .position(|event| matches!(
        event,
        Event::TransferExecuted { actor_id: id, to, .. } if *id == subscriber_id && *to == CHARLIE
      ))
      .expect("fanout actor transfer executed");
    assert!(
      wakeup_pos < fanout_pos,
      "wakeup-enqueued actor must execute before fanout-enqueued actor: wakeup={wakeup_pos}, fanout={fanout_pos}"
    );
  });
}

#[test]
fn subscription_cleanup_failure_rolls_back_actor_deactivation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      observation_schedule(vec![18]),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::note_observation_changed(18, 1));
    crate::DirtyObservationListState::<Test>::mutate(|list| list.tail = None);
    let actor_before = Actors::actor_hot(actor_id).expect("active actor");
    let dirty_before = Actors::dirty_observation_feeds(18).expect("dirty feed");
    let list_before = Actors::dirty_observation_list();
    let events_before = System::events();

    assert_noop!(
      Actors::deactivate_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::DirtyObservationInvariant
    );
    assert_eq!(Actors::actor_hot(actor_id), Some(actor_before));
    assert_eq!(Actors::observation_subscriber_count(18), 1);
    assert_eq!(Actors::dirty_observation_feeds(18), Some(dirty_before));
    assert_eq!(Actors::dirty_observation_list(), list_before);
    assert_eq!(System::events(), events_before);

    crate::DirtyObservationListState::<Test>::mutate(|list| list.tail = Some(18));
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(Actors::actor_hot(actor_id).is_none());
    assert_eq!(Actors::observation_subscriber_count(18), 0);
    assert!(Actors::dirty_observation_feeds(18).is_none());
    assert_eq!(Actors::dirty_observation_list(), Default::default());
  });
}

#[test]
fn active_dirty_list_rotates_fairly_and_repairs_cursor_on_removal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actors = [1u32, 2, 3].map(|feed| {
      create_system_with(
        ALICE,
        observation_schedule(vec![feed]),
        None,
        inert_contract_steps(),
      )
    });
    for feed in [1u32, 2, 3] {
      assert_ok!(Actors::note_observation_changed(feed, 1));
    }
    let list = Actors::dirty_observation_list();
    assert_eq!(
      (list.head, list.tail, list.cursor, list.count),
      (Some(1), Some(3), Some(1), 3)
    );
    assert_eq!(
      Actors::dirty_observation_feeds(2)
        .expect("middle dirty feed")
        .previous_dirty_feed,
      Some(1)
    );

    assert!(crate::Pallet::<Test>::do_fanout_dirty_observation_page().expect("first page"));
    assert!(Actors::dirty_observation_feeds(1).is_none());
    assert_eq!(Actors::dirty_observation_list().cursor, Some(2));
    assert!(Actors::pending_signal(actors[0]));

    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      actors[1]
    ));
    let repaired = Actors::dirty_observation_list();
    assert_eq!(
      (
        repaired.head,
        repaired.tail,
        repaired.cursor,
        repaired.count
      ),
      (Some(3), Some(3), Some(3), 1)
    );
    let last = Actors::dirty_observation_feeds(3).expect("last dirty feed");
    assert_eq!(
      (last.previous_dirty_feed, last.next_dirty_feed),
      (None, None)
    );

    assert!(!crate::Pallet::<Test>::do_fanout_dirty_observation_page().expect("last page"));
    assert_eq!(Actors::dirty_observation_list(), Default::default());
    assert!(Actors::pending_signal(actors[2]));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn multiple_dense_dirty_feeds_receive_round_robin_service() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size: u32 = <Test as crate::Config>::ObservationPageSize::get();
    let feeds = [21u32, 22, 23];
    let actors = feeds
      .into_iter()
      .map(|feed| {
        (0..=page_size)
          .map(|_| {
            create_system_with(
              ALICE,
              observation_schedule(vec![feed]),
              None,
              inert_contract_steps(),
            )
          })
          .collect::<Vec<_>>()
      })
      .collect::<Vec<_>>();
    for feed in feeds {
      assert_ok!(Actors::note_observation_changed(feed, 1));
    }
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    let budget = base.saturating_add(unit);

    for (index, feed) in feeds.into_iter().enumerate() {
      assert_eq!(Actors::dirty_observation_list().cursor, Some(feed));
      assert_eq!(Actors::fanout_dirty_observations(budget), budget);
      let delivered = actors[index]
        .iter()
        .filter(|actor_id| Actors::pending_signal(**actor_id))
        .count();
      assert!(delivered > 0);
      assert!(delivered < actors[index].len());
    }
    assert_eq!(Actors::dirty_observation_list().cursor, Some(feeds[0]));

    for _ in 0..12 {
      if Actors::dirty_observation_feed_count() == 0 {
        break;
      }
      Actors::fanout_dirty_observations(budget);
    }
    assert_eq!(Actors::dirty_observation_feed_count(), 0);
    assert_eq!(Actors::dirty_observation_list(), Default::default());
    let tickets = actors
      .iter()
      .flatten()
      .map(|actor_id| {
        let hot = Actors::actor_hot(*actor_id).expect("dense-feed actor");
        assert!(hot.pending_signal);
        hot.queue_ticket.expect("dense-feed actor queued")
      })
      .collect::<alloc::collections::BTreeSet<_>>();
    assert_eq!(tickets.len(), feeds.len() * (page_size as usize + 1));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn dirty_feed_capacity_failure_rolls_back_list_insertion() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    create_system_with(
      ALICE,
      observation_schedule(vec![9]),
      None,
      inert_contract_steps(),
    );
    let maximum: u32 = <Test as crate::Config>::MaxActiveActors::get();
    crate::DirtyObservationListState::<Test>::put(crate::types::DirtyObservationList {
      count: maximum,
      ..Default::default()
    });
    assert_noop!(
      Actors::note_observation_changed(9, 1),
      Error::<Test>::DirtyObservationCapacityExceeded
    );
    assert!(Actors::dirty_observation_feeds(9).is_none());
    assert_eq!(Actors::dirty_observation_feed_count(), maximum);
  });
}

#[test]
fn fanout_requires_complete_ref_time_and_proof_size_before_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      observation_schedule(vec![10]),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::note_observation_changed(10, 1));
    let before = Actors::dirty_observation_feeds(10).expect("dirty feed");
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    let required = base.saturating_add(unit);

    let ref_time_short = Weight::from_parts(required.ref_time().saturating_sub(1), u64::MAX);
    assert_eq!(Actors::fanout_dirty_observations(ref_time_short), base);
    assert_eq!(Actors::dirty_observation_feeds(10), Some(before));
    assert!(!Actors::pending_signal(actor_id));
    assert_eq!(Actors::dirty_observation_list().cursor, Some(10));

    let proof_short = Weight::from_parts(u64::MAX, required.proof_size().saturating_sub(1));
    assert_eq!(Actors::fanout_dirty_observations(proof_short), base);
    assert_eq!(Actors::dirty_observation_feeds(10), Some(before));
    assert!(!Actors::pending_signal(actor_id));
    assert_eq!(Actors::dirty_observation_list().cursor, Some(10));
  });
}

#[test]
fn one_fanout_page_sets_existing_latches_and_scheduler_membership() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actors = (0..3)
      .map(|_| {
        create_system_with(
          ALICE,
          observation_schedule(vec![11]),
          None,
          inert_contract_steps(),
        )
      })
      .collect::<Vec<_>>();
    assert_ok!(Actors::note_observation_changed(11, 1));
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    assert_eq!(
      Actors::fanout_dirty_observations(base.saturating_add(unit)),
      base.saturating_add(unit)
    );
    for actor_id in actors {
      let hot = Actors::actor_hot(actor_id).expect("active actor");
      assert!(hot.pending_signal);
      assert!(hot.queue_ticket.is_some());
    }
    assert!(Actors::dirty_observation_feeds(11).is_none());
    assert_eq!(Actors::dirty_observation_feed_count(), 0);
    assert_eq!(Actors::dirty_observation_list(), Default::default());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn saturated_queue_retains_the_fanout_page_until_admission_recovers() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      observation_schedule(vec![16]),
      None,
      inert_contract_steps(),
    );
    let page_size: u32 = <Test as crate::Config>::QueuePageSize::get();
    let capacity: u32 = <Test as crate::Config>::MaxQueueLength::get();
    for page_id in 0..capacity.div_ceil(page_size) {
      let first = page_id.saturating_mul(page_size);
      let len = page_size.min(capacity.saturating_sub(first));
      let entries = (0..len)
        .map(|offset| QueueEntry {
          ticket: u64::from(first.saturating_add(offset)),
          actor_id: 20_000_000u64.saturating_add(u64::from(first.saturating_add(offset))),
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
    assert_ok!(Actors::note_observation_changed(16, 1));
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    let budget = base.saturating_add(unit);

    Actors::fanout_dirty_observations(budget);
    let retained = Actors::dirty_observation_feeds(16).expect("fanout page remains dirty");
    assert_eq!(retained.next_subscriber_page, Some(0));
    assert!(Actors::pending_signal(actor_id));
    assert!(
      Actors::actor_hot(actor_id)
        .expect("actor")
        .queue_ticket
        .is_none()
    );

    let cutoff = Actors::next_queue_ticket();
    let drained = Actors::paged_drain_tombstones(cutoff, 1).expect("valid queue topology");
    assert_eq!(drained.entries_scanned, 1);
    Actors::fanout_dirty_observations(budget);
    assert!(Actors::dirty_observation_feeds(16).is_none());
    assert!(
      Actors::actor_hot(actor_id)
        .expect("actor")
        .queue_ticket
        .is_some()
    );
  });
}

#[test]
fn on_idle_fanout_feeds_the_existing_scheduler_without_direct_execution() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      observation_schedule(vec![14]),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::note_observation_changed(14, 1));
    assert_eq!(
      Actors::actor_identities(actor_id)
        .expect("identity")
        .cycle_nonce,
      0
    );
    assert!(!Actors::pending_signal(actor_id));

    let consumed = <Actors as Hooks<MockBlockNumber>>::on_idle(1, Weight::MAX);
    assert_ne!(consumed, Weight::zero());
    assert!(Actors::dirty_observation_feeds(14).is_none());
    let after = Actors::actor_hot(actor_id).expect("actor survives productive cycle");
    assert_eq!(
      Actors::actor_identities(actor_id)
        .expect("identity")
        .cycle_nonce,
      1
    );
    assert!(!after.pending_signal);
  });
}

#[test]
fn newer_revision_during_fanout_restarts_from_the_first_subscriber_page() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size: u32 = <Test as crate::Config>::ObservationPageSize::get();
    let actors = (0..=page_size)
      .map(|_| {
        create_system_with(
          ALICE,
          observation_schedule(vec![12]),
          None,
          inert_contract_steps(),
        )
      })
      .collect::<Vec<_>>();
    assert_ok!(Actors::note_observation_changed(12, 1));
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    Actors::fanout_dirty_observations(base.saturating_add(unit));
    let in_progress = Actors::dirty_observation_feeds(12).expect("fanout remains in progress");
    assert_eq!(in_progress.fanout_revision, 1);
    assert_eq!(in_progress.next_subscriber_page, Some(1));
    let first = actors[0];
    let first_ticket = Actors::actor_hot(first).expect("first actor").queue_ticket;
    crate::ActorHot::<Test>::mutate(first, |maybe| {
      maybe.as_mut().expect("first actor").pending_signal = false;
    });

    assert_ok!(Actors::note_observation_changed(12, 2));
    Actors::fanout_dirty_observations(base.saturating_add(unit));
    let restarted = Actors::dirty_observation_feeds(12).expect("new revision restarts fanout");
    assert_eq!(restarted.latest_revision, 2);
    assert_eq!(restarted.fanout_revision, 2);
    assert_eq!(restarted.next_subscriber_page, Some(0));
    assert!(!Actors::pending_signal(first));

    Actors::fanout_dirty_observations(base.saturating_add(unit));
    assert!(Actors::pending_signal(first));
    assert_eq!(
      Actors::actor_hot(first).expect("first actor").queue_ticket,
      first_ticket
    );
    Actors::fanout_dirty_observations(base.saturating_add(unit));
    assert!(Actors::dirty_observation_feeds(12).is_none());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn latest_revision_fanout_model_converges_across_seeded_races() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size: u32 = <Test as crate::Config>::ObservationPageSize::get();
    let actors = (0..page_size.saturating_mul(2).saturating_add(1))
      .map(|_| {
        create_system_with(
          ALICE,
          observation_schedule(vec![15]),
          None,
          inert_contract_steps(),
        )
      })
      .collect::<Vec<_>>();
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    let budget = base.saturating_add(unit);
    let mut latest_revision = 1u64;
    let mut delivered = vec![0u64; actors.len()];
    let mut seed = 0xDE05_0777u64;
    assert_ok!(Actors::note_observation_changed(15, latest_revision));

    let process_one = |delivered: &mut Vec<u64>| {
      let Some(before) = Actors::dirty_observation_feeds(15) else {
        return false;
      };
      let page_revision = if before.fanout_revision == 0 {
        before.latest_revision
      } else {
        before.fanout_revision
      };
      let page_id = before.next_subscriber_page.unwrap_or_else(|| {
        Actors::observation_subscriber_page_list(15)
          .expect("occupied pages")
          .head
      });
      let start = page_id.saturating_mul(page_size) as usize;
      let end = start
        .saturating_add(page_size as usize)
        .min(delivered.len());
      Actors::fanout_dirty_observations(budget);
      for revision in &mut delivered[start..end] {
        *revision = (*revision).max(page_revision);
      }
      true
    };

    for _step in 0..96u32 {
      seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
      if seed.is_multiple_of(3) {
        latest_revision += 1;
        assert_ok!(Actors::note_observation_changed(15, latest_revision));
      }
      if seed.is_multiple_of(5) {
        let index = (seed as usize) % actors.len();
        crate::ActorHot::<Test>::mutate(actors[index], |maybe| {
          maybe.as_mut().expect("model actor").pending_signal = false;
        });
      }
      process_one(&mut delivered);
    }
    for _ in 0..256 {
      if !process_one(&mut delivered) {
        break;
      }
    }
    assert!(Actors::dirty_observation_feeds(15).is_none());
    assert!(
      delivered
        .iter()
        .all(|revision| *revision == latest_revision)
    );
    let tickets = actors
      .iter()
      .filter_map(|actor_id| Actors::actor_hot(*actor_id).and_then(|hot| hot.queue_ticket))
      .collect::<alloc::collections::BTreeSet<_>>();
    assert_eq!(tickets.len(), actors.len());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn partial_fanout_page_then_deactivation_reconciles_dirty_feed() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size: u32 = <Test as crate::Config>::ObservationPageSize::get();
    let actors = (0..=page_size)
      .map(|_| {
        create_system_with(
          ALICE,
          observation_schedule(vec![30]),
          None,
          inert_contract_steps(),
        )
      })
      .collect::<Vec<_>>();
    assert_ok!(Actors::note_observation_changed(30, 1));
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    Actors::fanout_dirty_observations(base.saturating_add(unit));
    let in_progress = Actors::dirty_observation_feeds(30).expect("first page consumed");
    assert_eq!(in_progress.next_subscriber_page, Some(1));

    // Deactivate the only subscriber on the unvisited second page mid-fanout. This unlinks the
    // page and must adjust the dirty feed's next page instead of leaving a dangling invariant.
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      actors[page_size as usize]
    ));
    let repaired = Actors::dirty_observation_feeds(30).expect("dirty feed survives");
    assert!(
      repaired.next_subscriber_page.is_none(),
      "unlinked page must not remain the fanout cursor"
    );

    Actors::fanout_dirty_observations(base.saturating_add(unit));
    assert!(Actors::dirty_observation_feeds(30).is_none());
    for actor_id in &actors[..page_size as usize] {
      assert!(
        Actors::pending_signal(*actor_id),
        "first-page subscriber signalled"
      );
    }
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn subscriber_mutation_during_fanout_reconciles_without_invariant_errors() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size: u32 = <Test as crate::Config>::ObservationPageSize::get();
    let actors = (0..=page_size)
      .map(|_| {
        create_system_with(
          ALICE,
          observation_schedule(vec![31]),
          None,
          inert_contract_steps(),
        )
      })
      .collect::<Vec<_>>();
    assert_ok!(Actors::note_observation_changed(31, 1));
    let base =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_base();
    let unit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::observation_fanout_page();
    Actors::fanout_dirty_observations(base.saturating_add(unit));
    assert_eq!(
      Actors::dirty_observation_feeds(31)
        .expect("in progress")
        .next_subscriber_page,
      Some(1)
    );

    // Remove the only second-page subscriber mid-fanout via a schedule change away from the feed.
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actors[page_size as usize],
      manual_schedule(),
      None,
    ));
    assert!(
      Actors::dirty_observation_feeds(31).is_some(),
      "dirty feed survives"
    );

    // Re-add a subscriber to the same feed mid-fanout; the fanout must finish without an
    // invariant error and signal the first page. The late subscriber joins delivery on the next
    // revision rather than the in-flight one.
    let late_subscriber = create_system_with(
      ALICE,
      observation_schedule(vec![31]),
      None,
      inert_contract_steps(),
    );
    assert!(!Actors::pending_signal(late_subscriber));

    for _ in 0..4 {
      Actors::fanout_dirty_observations(base.saturating_add(unit));
      if Actors::dirty_observation_feeds(31).is_none() {
        break;
      }
    }
    assert!(Actors::dirty_observation_feeds(31).is_none());
    for actor_id in &actors[..page_size as usize] {
      assert!(
        Actors::pending_signal(*actor_id),
        "first-page subscriber signalled"
      );
    }

    assert_ok!(Actors::note_observation_changed(31, 2));
    for _ in 0..4 {
      Actors::fanout_dirty_observations(base.saturating_add(unit));
      if Actors::dirty_observation_feeds(31).is_none() {
        break;
      }
    }
    assert!(Actors::dirty_observation_feeds(31).is_none());
    assert!(
      Actors::pending_signal(late_subscriber),
      "late subscriber delivered next revision"
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn maximum_density_fanout_converges_without_duplicate_queue_membership() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_count = Actors::effective_active_actor_limit();
    let actors = (0..actor_count)
      .map(|_| {
        create_system_with(
          ALICE,
          observation_schedule(vec![13]),
          None,
          inert_contract_steps(),
        )
      })
      .collect::<Vec<_>>();
    assert_ok!(Actors::note_observation_changed(13, 1));
    let consumed = Actors::fanout_dirty_observations(Weight::MAX);
    assert_ne!(consumed, Weight::zero());
    assert!(Actors::dirty_observation_feeds(13).is_none());
    assert_eq!(Actors::dirty_observation_feed_count(), 0);
    let mut tickets = alloc::collections::BTreeSet::new();
    for actor_id in actors {
      let hot = Actors::actor_hot(actor_id).expect("active actor");
      assert!(hot.pending_signal);
      assert!(tickets.insert(hot.queue_ticket.expect("one queue ticket")));
    }
    assert_eq!(tickets.len() as u32, actor_count);
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn maximum_observation_subscription_density_is_paged_and_bounded() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_count = Actors::effective_active_actor_limit();
    let feed = 1;
    for _ in 0..actor_count {
      create_system_with(
        ALICE,
        observation_schedule(vec![feed]),
        None,
        inert_contract_steps(),
      );
    }
    assert_eq!(Actors::active_actor_count(), actor_count);
    assert_eq!(Actors::observation_subscription_count(), actor_count);
    let page_size: u32 = <Test as crate::Config>::ObservationPageSize::get();
    let page_count = actor_count.div_ceil(page_size);
    assert_eq!(Actors::observation_subscriber_count(feed), actor_count);
    assert_eq!(
      crate::ObservationSubscriberPages::<Test>::iter_prefix(feed).count() as u32,
      page_count
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_dirty_observation_list_drift() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    create_system_with(
      ALICE,
      observation_schedule(vec![6]),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::note_observation_changed(6, 1));
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    crate::ObservationIngressRevisions::<Test>::insert(6, 2);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    crate::ObservationIngressRevisions::<Test>::insert(6, 1);
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    crate::DirtyObservationListState::<Test>::mutate(|list| list.tail = None);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_subscription_reverse_index_drift() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      observation_schedule(vec![7]),
      None,
      inert_contract_steps(),
    );
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    crate::ActorObservationFeeds::<Test>::remove(actor_id);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn actor_step_metadata_is_a_closed_linear_control_surface() {
  let info = RuntimeStep::type_info();
  let TypeDef::Composite(definition) = info.type_def else {
    panic!("Step must remain a SCALE composite");
  };
  assert_eq!(definition.fields.len(), 3);
  assert_eq!(definition.fields[0].name, Some("precondition"));
  assert_eq!(definition.fields[1].name, Some("task"));
  assert_eq!(definition.fields[2].name, Some("on_error"));
  assert!(
    definition.fields[0]
      .type_name
      .unwrap_or_default()
      .contains("Option")
  );
  assert!(
    definition.fields[1]
      .type_name
      .unwrap_or_default()
      .contains("Task")
  );
  assert!(
    definition.fields[2]
      .type_name
      .unwrap_or_default()
      .contains("StepErrorPolicy")
  );
}

#[test]
fn unit_asset_adapter_fails_closed_for_every_mutation() {
  type UnsupportedAssetOps = ();
  assert_eq!(
    <UnsupportedAssetOps as AssetOps<AccountId, TestAsset, Balance>>::transfer(
      &ALICE,
      &BOB,
      TestAsset::Native,
      1,
    ),
    Err(TaskFailure {
      error: DispatchError::Other("AssetOps not configured"),
      retry: RetryClass::Permanent,
    })
  );
  assert_eq!(
    <UnsupportedAssetOps as AssetOps<AccountId, TestAsset, Balance>>::burn(
      &ALICE,
      TestAsset::Native,
      1,
    ),
    Err(TaskFailure {
      error: DispatchError::Other("AssetOps not configured"),
      retry: RetryClass::Permanent,
    })
  );
  assert_eq!(
    <UnsupportedAssetOps as AssetOps<AccountId, TestAsset, Balance>>::mint(
      &ALICE,
      TestAsset::Native,
      1,
    ),
    Err(TaskFailure {
      error: DispatchError::Other("AssetOps not configured"),
      retry: RetryClass::Permanent,
    })
  );
  assert_eq!(
    <UnsupportedAssetOps as AssetOps<AccountId, TestAsset, Balance>>::preflight_transfer(
      &ALICE,
      &BOB,
      TestAsset::Native,
      1,
    ),
    Err(TaskFailure {
      error: DispatchError::Other("AssetOps not configured"),
      retry: RetryClass::Permanent,
    })
  );
}

#[test]
fn task_failure_defaults_unknown_errors_to_permanent() {
  let error = DispatchError::Other("UnclassifiedAdapterFailure");
  assert_eq!(TaskFailure::from(error), TaskFailure::permanent(error));
  assert_eq!(TaskFailure::temporary(error).retry, RetryClass::Temporary);
}

#[test]
fn public_api_error_signatures_use_shared_typed_cores() {
  let _: fn(ActorId) -> Result<ActorEligibility<u64>, ActorClassificationError> =
    Actors::actor_eligibility;
  let _: fn(
    ActorId,
    ActorType,
    Mutability,
    RuntimeActorContract,
    SimulationMode,
  ) -> Result<crate::SimulationResultOf<Test>, SimulationError> = Actors::simulate_current_contract;

  let classification_cases = [
    (
      ActorClassificationError::ActorInvariant,
      Error::<Test>::ActorInvariant,
    ),
    (
      ActorClassificationError::ContinuationInvariant,
      Error::<Test>::ContinuationInvariant,
    ),
    (
      ActorClassificationError::ComputationOverflow,
      Error::<Test>::ComputationOverflow,
    ),
  ];
  for (core, dispatch) in classification_cases {
    assert_eq!(
      Actors::classification_dispatch_error(core).encode(),
      dispatch.encode()
    );
  }
}

#[test]
fn public_reachability_inventory_is_closed_and_canonical() {
  assert_variant_names::<RuntimeTask>(&[
    "Transfer",
    "SplitTransfer",
    "SwapIn",
    "SwapOut",
    "AddLiquidity",
    "RemoveLiquidity",
    "Burn",
    "Mint",
    "Stake",
    "DonateLiquidity",
    "Unstake",
    "StopCycle",
  ]);
  assert_variant_names::<AmountResolution<u128>>(&[
    "Fixed",
    "PercentageOfCurrent",
    "PercentageAtOpening",
    "PercentageOfLastFunding",
    "AllAvailable",
  ]);
  assert_variant_names::<InputLimit<u128>>(&["LiveQuote", "Absolute"]);
  assert_variant_names::<Predicate<TestAsset, u128, u32, u32>>(&[
    "BalanceAbove",
    "BalanceBelow",
    "BalanceEquals",
    "BalanceNotEquals",
    "BlockNumberAbove",
    "BlockNumberBelow",
    "ObservationAbove",
    "ObservationBelow",
    "ObservationEquals",
    "ObservationNotEquals",
  ]);
  assert_variant_names::<ObservationTiming>(&["Opening", "Current"]);
  assert_variant_names::<crate::PredicateError>(&["InvalidObservation"]);
  assert_variant_names::<RuntimeSourceFilter>(&["Any", "OwnerOnly", "Whitelist"]);
  assert_variant_names::<RuntimeAssetFilter>(&["Any", "Whitelist"]);
  assert_variant_names::<RuntimeTrigger>(&[
    "Manual",
    "AddressEvent",
    "ObservationChange",
    "Cadenced",
  ]);
  assert_variant_names::<Trigger<AccountId, TestAsset, <Test as crate::Config>::MaxWhitelistSize>>(
    &["Manual", "AddressEvent", "ObservationChange", "Cadenced"],
  );
  assert_variant_names::<ActorType>(&["User", "System"]);
  assert_variant_names::<ActorClass>(&["User", "System"]);
  assert_variant_names::<Mutability>(&["Mutable", "Immutable"]);
  assert_variant_names::<crate::CompletionPolicy>(&["Persistent", "CloseAfterProductiveCycle"]);
  assert_variant_names::<ActiveLifecycle>(&["Active", "Paused"]);
  assert_variant_names::<CycleState>(&["Idle", "Suspended"]);
  assert_variant_names::<AttemptDisposition>(&["Completed", "Failed", "Suspended", "Closed"]);
  assert_variant_names::<StepOutcome>(&[
    "Executed",
    "Stopped",
    "Skipped",
    "FundingUnavailable",
    "Failed",
  ]);
  assert_variant_names::<OpeningSurface<TestAsset>>(&[
    "PreservableAsset",
    "TargetAsset",
    "StakingShares",
  ]);
  assert_variant_names::<CloseReason>(&[
    "OwnerInitiated",
    "BalanceExhausted",
    "ConsecutiveFailures",
    "WindowExpired",
    "CycleNonceExhausted",
    "FeeBudgetExhausted",
    "AutoCloseNonceReached",
    "RetryAttemptsExhausted",
    "ProductiveCycleCompleted",
  ]);
  assert_variant_names::<StepErrorPolicy>(&["AbortCycle", "ContinueNextStep", "RetryLater"]);
  assert_variant_names::<SuspensionReason>(&["FundingUnavailable", "Temporary"]);
  assert_variant_names::<CancellationReason>(&[
    "Explicit",
    "ContractReplaced",
    "Deactivated",
    "Closing",
  ]);
  assert_variant_names::<StepSkippedReason>(&[
    "PreconditionFalse",
    "ResolutionSkipped",
    "FundingUnavailable",
  ]);
  assert_variant_names::<FundingSourcePolicy<AccountId, <Test as crate::Config>::MaxWhitelistSize>>(
    &[
      "OwnerOnly",
      "SignedAllowlist",
      "RuntimePolicy",
      "AnyVerifiedIngress",
    ],
  );
  assert_variant_names::<crate::FundingProvenance>(&["Signed", "InternalProtocol", "Xcm"]);
  assert_variant_names::<RetryClass>(&["Permanent", "Temporary"]);
  assert_variant_names::<crate::ScalarObservationState<u64>>(&[
    "Unavailable",
    "Uninitialized",
    "Fresh",
    "Stale",
  ]);
  assert_variant_names::<ActorEligibility<u64>>(&["NotRegistered", "Dormant", "Active"]);
  assert_variant_names::<SimulationMode>(&["FreshCurrentPlan", "CurrentContinuation"]);
  assert_variant_names::<SimulationError>(&[
    "TransactionDepthExceeded",
    "Classification",
    "ActorNotFound",
    "TypeMismatch",
    "MutabilityMismatch",
    "ContractMismatch",
    "ModeCycleStateMismatch",
    "GlobalCircuitBreaker",
    "Paused",
    "NotReady",
    "FeeCollectionFailed",
  ]);
}

#[test]
fn paged_wakeup_primitives_encode_exact_pointer_and_bounded_page_ownership() {
  let pointer = WakeupPointer {
    block: WakeupKey::Block(42u64),
    page_id: 7,
    slot: 3,
  };
  assert_eq!(pointer.block, WakeupKey::Block(42));
  assert_eq!(pointer.page_id, 7);
  assert_eq!(pointer.slot, 3);

  let entries =
    BoundedVec::<Option<WakeupEntry>, <Test as crate::Config>::WakeupPageSize>::try_from(vec![
      Some(WakeupEntry { actor_id: 9 }),
      None,
    ])
    .expect("wakeup page entries fit");
  let page = WakeupPage {
    entries,
    live_entries: 1,
    scan_slot: 0,
    previous_page: Some(6),
    next_page: Some(8),
  };
  assert_eq!(page.entries[0], Some(WakeupEntry { actor_id: 9 }));
  assert_eq!(page.entries[1], None);
  assert_eq!(page.live_entries, 1);
  assert_eq!((page.previous_page, page.next_page), (Some(6), Some(8)));

  let bucket = WakeupBucketState {
    head_page: 6,
    tail_page: 8,
    next_page_id: 9,
    live_entries: 65,
    cursor_index: Some(3),
  };
  assert_eq!(bucket.head_page, 6);
  assert_eq!(bucket.tail_page, 8);
  assert_eq!(bucket.next_page_id, 9);
  assert_eq!(bucket.live_entries, 65);
}

#[test]
fn paged_wakeup_substrate_replaces_and_invalidates_exact_slots() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );

    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    let first = Actors::actor_hot(actor_id)
      .expect("hot state")
      .wakeup_pointer
      .expect("first wakeup pointer");
    assert_eq!(
      (first.block, first.page_id, first.slot),
      (WakeupKey::Block(10), 0, 0)
    );
    assert_eq!(
      Actors::wakeup_buckets(10)
        .expect("first bucket")
        .live_entries,
      1
    );
    assert_eq!(Actors::wakeup_cursor_len(), 1);
    assert_eq!(Actors::wakeup_cursor_peek(), Some(10));

    assert!(Actors::wakeup_substrate_schedule(actor_id, 20));
    let replacement = Actors::actor_hot(actor_id)
      .expect("hot state")
      .wakeup_pointer
      .expect("replacement wakeup pointer");
    assert_eq!(
      (replacement.block, replacement.page_id, replacement.slot),
      (WakeupKey::Block(20), 0, 0)
    );
    assert!(Actors::wakeup_buckets(10).is_none());
    assert!(Actors::wakeup_pages((10, 0)).is_none());
    assert_eq!(Actors::wakeup_cursor_len(), 1);
    assert_eq!(Actors::wakeup_cursor_peek(), Some(20));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());

    assert_eq!(
      Actors::wakeup_substrate_invalidate(actor_id),
      Some(replacement)
    );
    assert!(
      Actors::actor_hot(actor_id)
        .expect("hot state")
        .wakeup_pointer
        .is_none()
    );
    assert!(Actors::wakeup_buckets(20).is_none());
    assert!(Actors::wakeup_pages((20, 0)).is_none());
    assert_eq!(Actors::wakeup_cursor_len(), 0);
    assert_eq!(Actors::wakeup_cursor_peek(), None);
  });
}

#[test]
fn wakeup_replacement_rolls_back_when_existing_cursor_is_corrupt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    WakeupBuckets::<Test>::mutate(WakeupKey::Block(10), |maybe_bucket| {
      maybe_bucket.as_mut().expect("bucket").cursor_index = Some(1);
    });
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    assert_eq!(
      Actors::try_wakeup_substrate_schedule_inner(actor_id, 20),
      Err(crate::EnqueueOutcome::CorruptedTopology),
    );

    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });
}

#[test]
fn wakeup_replacement_rolls_back_when_existing_page_or_slot_is_missing() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    WakeupPages::<Test>::remove((WakeupKey::Block(10), 0));
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_eq!(
      Actors::try_wakeup_substrate_schedule_inner(actor_id, 20),
      Err(crate::EnqueueOutcome::CorruptedTopology),
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    ActorHot::<Test>::mutate(actor_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("hot")
        .wakeup_pointer
        .as_mut()
        .expect("pointer")
        .slot = 7;
    });
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_eq!(
      Actors::try_wakeup_substrate_schedule_inner(actor_id, 20),
      Err(crate::EnqueueOutcome::CorruptedTopology),
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });
}

#[test]
fn wakeup_replacement_rolls_back_on_live_count_underflow() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    WakeupBuckets::<Test>::mutate(WakeupKey::Block(10), |maybe_bucket| {
      maybe_bucket.as_mut().expect("bucket").live_entries = 0;
    });
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    assert_eq!(
      Actors::try_wakeup_substrate_schedule_inner(actor_id, 20),
      Err(crate::EnqueueOutcome::CorruptedTopology),
    );

    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });
}

#[test]
fn wakeup_cursor_capacity_overflow_fails_closed_and_preserves_existing_path() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    let pointer = Actors::actor_hot(actor_id)
      .expect("hot state")
      .wakeup_pointer
      .expect("existing path");
    assert_eq!(
      (pointer.block, pointer.page_id, pointer.slot),
      (WakeupKey::Block(10), 0, 0)
    );

    // Saturate the wakeup cursor heap at its capacity bound; the worker's index insert fails.
    crate::WakeupCursorLen::<Test>::insert(
      WakeupClock::Block,
      <<Test as crate::Config>::MaxActiveActors as Get<u32>>::get(),
    );
    assert!(
      !Actors::wakeup_substrate_schedule(actor_id, 20),
      "cursor capacity overflow must fail closed"
    );
    // The transactional wrapper rolls back the attempted replacement; the actor keeps its exact
    // existing pointer and the original bucket/cursor entries stay intact.
    assert_eq!(
      Actors::actor_hot(actor_id)
        .expect("hot state")
        .wakeup_pointer,
      Some(pointer)
    );
    assert!(
      Actors::wakeup_buckets(10).is_some(),
      "original bucket survives"
    );
    assert!(
      Actors::wakeup_pages((10, 0)).is_some(),
      "original page survives"
    );
    assert!(
      Actors::wakeup_buckets(20).is_none(),
      "no partial replacement bucket"
    );
    assert_eq!(Actors::wakeup_cursor_peek(), Some(10));
    #[cfg(feature = "try-runtime")]
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn wakeup_page_index_overflow_fails_closed_as_namespace_exhaustion() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    // Seed a bucket whose next_page_id is at the u64 ceiling; appending a new page
    // cannot advance the monotonic page index and must fail closed as
    // WakeupIndexExhausted (mapped to the SchedulerIndexExhausted public error).
    let block = 10u64;
    assert!(Actors::wakeup_substrate_schedule(actor_id, block));
    // Fill the tail page to force the page-append branch, then set the next page id
    // at the u64 ceiling so the monotonic index cannot advance.
    let page_size = <<Test as crate::Config>::WakeupPageSize as Get<u32>>::get();
    for _ in 0..page_size.saturating_sub(1) {
      let extra = create_system_with(
        BOB,
        manual_schedule(),
        None,
        transfer_contract_steps(CHARLIE, 1),
      );
      assert!(Actors::wakeup_substrate_schedule(extra, block));
    }
    crate::WakeupBuckets::<Test>::mutate(WakeupKey::Block(block), |bucket| {
      let bucket = bucket.as_mut().expect("bucket exists");
      bucket.next_page_id = u64::MAX;
    });
    let pointer_before = Actors::actor_hot(actor_id)
      .expect("hot")
      .wakeup_pointer
      .expect("existing pointer");
    // A different actor scheduling into the full bucket with the page index at the
    // u64 ceiling cannot advance the monotonic index and fails closed.
    let new_actor = create_system_with(
      BOB,
      manual_schedule(),
      None,
      transfer_contract_steps(CHARLIE, 1),
    );
    assert!(matches!(
      crate::Pallet::<Test>::try_wakeup_substrate_schedule_inner(new_actor, block),
      Err(crate::EnqueueOutcome::WakeupIndexExhausted)
    ));
    // The existing pointer is never cleared before the replacement path fits.
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot").wakeup_pointer,
      Some(pointer_before)
    );
  });
}

#[test]
fn wakeup_bucket_corruption_fails_closed_as_corrupted_topology() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let block = 10u64;
    assert!(Actors::wakeup_substrate_schedule(actor_id, block));
    // Corrupt the TARGET bucket for a different actor: a bucket exists at block + 1 but
    // its cursor_index is missing, so the schedule path must fail closed as corrupted
    // topology instead of retrying as queue-full.
    let target = block + 1;
    crate::WakeupBuckets::<Test>::insert(
      WakeupKey::Block(target),
      crate::WakeupBucketState {
        head_page: 0,
        tail_page: 0,
        next_page_id: 1,
        live_entries: 1,
        cursor_index: None,
      },
    );
    let pointer_before = Actors::actor_hot(actor_id)
      .expect("hot")
      .wakeup_pointer
      .expect("existing pointer");
    let new_actor = create_system_with(
      BOB,
      manual_schedule(),
      None,
      transfer_contract_steps(CHARLIE, 1),
    );
    assert!(matches!(
      crate::Pallet::<Test>::try_wakeup_substrate_schedule_inner(new_actor, target),
      Err(crate::EnqueueOutcome::CorruptedTopology)
    ));
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot").wakeup_pointer,
      Some(pointer_before)
    );
  });
}

#[test]
fn saturated_enqueue_falls_back_to_exact_next_block_wakeup_not_silent_loss() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    // Saturate the canonical FIFO so ticket placement is impossible.
    seed_saturated_tombstone_queue();
    // Placement must not silently lose readiness: the fallback is an exact
    // next-block wakeup, so the actor owns a wakeup path even though the FIFO
    // rejected the ticket (spec 8.1.4).
    assert_ok!(Actors::enqueue(actor_id));
    let hot = Actors::actor_hot(actor_id).expect("hot");
    assert!(
      hot.queue_ticket.is_none(),
      "saturated FIFO grants no ticket"
    );
    assert_eq!(
      hot.wakeup_pointer.expect("readiness wakeup").block,
      WakeupKey::Block(2),
      "exact next-block wakeup preserves readiness"
    );
    assert_eq!(Actors::wakeup_buckets(2).expect("bucket").live_entries, 1);
  });
}

#[test]
fn saturated_enqueue_fails_closed_when_wakeup_fallback_also_fails() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    // Saturate the FIFO so enqueue falls back to the next-block wakeup, then
    // corrupt the target wakeup bucket so that fallback placement also fails.
    seed_saturated_tombstone_queue();
    crate::WakeupBuckets::<Test>::insert(
      WakeupKey::Block(2),
      crate::WakeupBucketState {
        head_page: 0,
        tail_page: 0,
        next_page_id: 1,
        live_entries: 0,
        cursor_index: None,
      },
    );
    // The placement must fail closed: the caller learns the actor owns no path
    // instead of believing enqueue succeeded while readiness was lost.
    assert!(matches!(
      Actors::enqueue(actor_id),
      Err(crate::EnqueueOutcome::CorruptedTopology)
    ));
    let hot = Actors::actor_hot(actor_id).expect("hot");
    assert!(hot.queue_ticket.is_none());
    assert!(hot.wakeup_pointer.is_none(), "no phantom wakeup on failure");
    assert!(Actors::wakeup_cursor_peek().is_none(), "no cursor residue");
  });
}

#[test]
fn manual_trigger_fails_closed_when_wakeup_fallback_cannot_preserve_readiness() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    // Saturate the FIFO so any ticket placement falls back to the next-block
    // wakeup, then corrupt that target bucket so the fallback also fails.
    seed_saturated_tombstone_queue();
    crate::WakeupBuckets::<Test>::insert(
      WakeupKey::Block(2),
      crate::WakeupBucketState {
        head_page: 0,
        tail_page: 0,
        next_page_id: 1,
        live_entries: 0,
        cursor_index: None,
      },
    );
    // The extrinsic must fail closed (namespace/corruption is not retryable
    // queue-full), leaving the actor with its pre-trigger state: no ticket and
    // no wakeup, and the pending-signal mutation rolled back.
    // FRAME dispatch wraps the call transactionally, so the pending-signal
    // mutation rolls back together with the failed placement.
    let before_signal = Actors::actor_hot(actor_id).expect("hot").pending_signal;
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let result = Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id);
    let after_signal = Actors::actor_hot(actor_id).expect("hot").pending_signal;
    assert!(
      result.is_err(),
      "manual_trigger must fail closed: {result:?}"
    );
    assert_eq!(
      (before_signal, after_signal),
      (false, false),
      "signal mutation must roll back with the failed placement"
    );
    let hot = Actors::actor_hot(actor_id).expect("hot");
    assert!(hot.queue_ticket.is_none());
    assert!(hot.wakeup_pointer.is_none());
    assert!(
      !hot.pending_signal,
      "signal mutation rolled back with the call"
    );
    assert_eq!(System::events(), events_before, "trigger event rolls back");
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn paged_wakeup_substrate_invalidation_rolls_back_on_cursor_mismatch() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    let pointer = Actors::actor_hot(actor_id)
      .expect("hot state")
      .wakeup_pointer
      .expect("wakeup pointer");
    crate::pallet::WakeupBuckets::<Test>::mutate(WakeupKey::Block(10), |maybe_bucket| {
      maybe_bucket.as_mut().expect("wakeup bucket").cursor_index = None;
    });

    assert_eq!(Actors::wakeup_substrate_invalidate(actor_id), None);
    assert_eq!(
      Actors::actor_hot(actor_id)
        .expect("hot state")
        .wakeup_pointer,
      Some(pointer)
    );
    assert!(Actors::wakeup_page_entry_matches(pointer, actor_id));
    assert_eq!(
      Actors::wakeup_buckets(10)
        .expect("wakeup bucket")
        .live_entries,
      1
    );
    assert_eq!(Actors::wakeup_cursor_len(), 1);
    assert_eq!(Actors::wakeup_cursor_peek(), Some(10));
  });
}

#[test]
fn paged_wakeup_substrate_links_and_unlinks_middle_pages() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size: u32 = <Test as crate::Config>::WakeupPageSize::get();
    let count = page_size.saturating_mul(2).saturating_add(1);
    let mut actors = Vec::new();
    for _ in 0..count {
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
      actors.push(actor_id);
    }

    let bucket = Actors::wakeup_buckets(10).expect("dense bucket");
    assert_eq!(bucket.live_entries, count);
    assert_eq!(
      (bucket.head_page, bucket.tail_page, bucket.next_page_id),
      (0, 2, 3)
    );
    assert_eq!(
      Actors::wakeup_pages((10, 0)).expect("head page").next_page,
      Some(1)
    );
    assert_eq!(
      Actors::wakeup_pages((10, 1))
        .expect("middle page")
        .next_page,
      Some(2)
    );
    assert_eq!(
      Actors::wakeup_pages((10, 2))
        .expect("tail page")
        .previous_page,
      Some(1)
    );

    let page_size = page_size as usize;
    for actor_id in &actors[page_size..page_size * 2] {
      assert!(Actors::wakeup_substrate_invalidate(*actor_id).is_some());
    }
    let bucket = Actors::wakeup_buckets(10).expect("bucket after middle unlink");
    assert_eq!(bucket.live_entries, count.saturating_sub(page_size as u32));
    assert!(Actors::wakeup_pages((10, 1)).is_none());
    assert_eq!(
      Actors::wakeup_pages((10, 0)).expect("head page").next_page,
      Some(2)
    );
    assert_eq!(
      Actors::wakeup_pages((10, 2))
        .expect("tail page")
        .previous_page,
      Some(0)
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());

    for actor_id in actors {
      let _ = Actors::wakeup_substrate_invalidate(actor_id);
    }
    assert!(Actors::wakeup_buckets(10).is_none());
    assert!(Actors::wakeup_pages((10, 0)).is_none());
    assert!(Actors::wakeup_pages((10, 2)).is_none());
  });
}

#[test]
fn paged_wakeup_drain_preserves_partial_progress_and_crosses_page_boundaries() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size: u32 = <Test as crate::Config>::WakeupPageSize::get();
    let count = page_size.saturating_mul(2).saturating_add(1);
    let mut actors = Vec::new();
    for _ in 0..count {
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
      actors.push(actor_id);
    }

    let first_limit = page_size / 2;
    let (first, first_stats) = Actors::wakeup_substrate_drain_block(10, first_limit);
    assert_eq!(first.as_slice(), &actors[..first_limit as usize]);
    assert_eq!(first_stats.entries_scanned, first_limit);
    assert_eq!(first_stats.ready_entries, first_limit);
    assert_eq!(first_stats.pages_touched, 1);
    assert_eq!(first_stats.pages_deleted, 0);
    let head = Actors::wakeup_pages((10, 0)).expect("partially drained head");
    assert_eq!(head.scan_slot, first_limit);
    assert_eq!(head.live_entries, page_size - first_limit);

    let (second, second_stats) = Actors::wakeup_substrate_drain_block(10, page_size);
    let second_end = first_limit.saturating_add(page_size) as usize;
    assert_eq!(second.as_slice(), &actors[first_limit as usize..second_end]);
    assert_eq!(second_stats.entries_scanned, page_size);
    assert_eq!(second_stats.ready_entries, page_size);
    assert_eq!(second_stats.pages_touched, 2);
    assert_eq!(second_stats.pages_deleted, 1);
    let bucket = Actors::wakeup_buckets(10).expect("remaining wakeup bucket");
    assert_eq!(bucket.head_page, 1);
    let head = Actors::wakeup_pages((10, 1)).expect("second partial head");
    assert_eq!(head.previous_page, None);
    assert_eq!(head.scan_slot, first_limit);

    let (final_ready, final_stats) = Actors::wakeup_substrate_drain_block(10, u32::MAX);
    assert_eq!(final_ready.as_slice(), &actors[second_end..]);
    assert_eq!(final_stats.ready_entries, count - first_limit - page_size);
    assert_eq!(final_stats.pages_touched, 2);
    assert_eq!(final_stats.pages_deleted, 2);
    assert!(Actors::wakeup_buckets(10).is_none());
    assert!(Actors::wakeup_pages((10, 1)).is_none());
    assert!(Actors::wakeup_pages((10, 2)).is_none());
    assert_eq!(Actors::wakeup_cursor_len(), 0);
    assert_eq!(Actors::wakeup_cursor_peek(), None);
    assert!(actors.iter().all(|actor_id| {
      Actors::actor_hot(*actor_id)
        .expect("hot state")
        .wakeup_pointer
        .is_none()
    }));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn paged_wakeup_drain_discards_stale_only_bucket() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut actors = Vec::new();
    for _ in 0..3 {
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
      ActorHot::<Test>::mutate(actor_id, |maybe_hot| {
        maybe_hot.as_mut().expect("hot state").wakeup_pointer = None;
      });
      actors.push(actor_id);
    }

    let (ready, stats) = Actors::wakeup_substrate_drain_block(10, 3);
    assert!(ready.is_empty());
    assert_eq!(stats.entries_scanned, 3);
    assert_eq!(stats.ready_entries, 0);
    assert_eq!(stats.stale_entries, 3);
    assert_eq!(stats.pages_touched, 1);
    assert_eq!(stats.pages_deleted, 1);
    assert!(Actors::wakeup_buckets(10).is_none());
    assert!(Actors::wakeup_pages((10, 0)).is_none());
    assert!(actors.iter().all(|actor_id| {
      Actors::actor_hot(*actor_id)
        .expect("hot state")
        .wakeup_pointer
        .is_none()
    }));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn cursor_wakeup_drain_recovers_sparse_overdue_blocks_without_scanning_gaps() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let due = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let future = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert!(Actors::wakeup_substrate_schedule(due, 10));
    assert!(Actors::wakeup_substrate_schedule(future, 1_000_000));

    let mut halted = WeightMeter::with_limit(Weight::zero());
    assert_eq!(
      Actors::drain_overdue_wakeups_cursor(100, &mut halted).entries_scanned,
      0
    );
    assert!(
      Actors::actor_hot(due)
        .expect("due actor")
        .queue_ticket
        .is_none()
    );

    let mut ample = WeightMeter::with_limit(Weight::from_parts(u64::MAX, u64::MAX));
    let stats = Actors::drain_overdue_wakeups_cursor(100, &mut ample);
    assert_eq!(stats.entries_scanned, 1);
    assert_eq!(stats.ready_entries, 1);
    assert!(
      Actors::actor_hot(due)
        .expect("due actor")
        .queue_ticket
        .is_some()
    );
    assert_eq!(Actors::wakeup_cursor_len(), 1);
    assert_eq!(Actors::wakeup_cursor_peek(), Some(1_000_000));
    assert!(
      Actors::actor_hot(future)
        .expect("future actor")
        .wakeup_pointer
        .is_some()
    );
  });
}

#[test]
fn cursor_wakeup_drain_halts_and_resumes_between_slot_units() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut actors = Vec::new();
    for _ in 0..3 {
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
      actors.push(actor_id);
    }
    let limit =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_wakeup_cursor_worker_future()
        .saturating_add(Actors::wakeup_cursor_drain_unit_weight_upper(false));
    let mut one_slot = WeightMeter::with_limit(limit);
    let first = Actors::drain_overdue_wakeups_cursor(10, &mut one_slot);
    assert_eq!(first.entries_scanned, 1);
    assert_eq!(first.ready_entries, 1);
    assert_eq!(
      Actors::wakeup_buckets(10)
        .expect("partial bucket")
        .live_entries,
      2
    );
    assert_eq!(Actors::wakeup_cursor_peek(), Some(10));

    let mut resume = WeightMeter::with_limit(Weight::from_parts(u64::MAX, u64::MAX));
    let second = Actors::drain_overdue_wakeups_cursor(10, &mut resume);
    assert_eq!(second.entries_scanned, 2);
    assert_eq!(second.ready_entries, 2);
    assert!(Actors::wakeup_buckets(10).is_none());
    assert_eq!(Actors::wakeup_cursor_len(), 0);
    assert!(actors.iter().all(|actor_id| {
      Actors::actor_hot(*actor_id)
        .expect("actor")
        .queue_ticket
        .is_some()
    }));
  });
}

#[test]
fn cursor_wakeup_drain_stops_independently_on_reftime_and_proof_size() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    let required =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_wakeup_cursor_worker_future()
        .saturating_add(Actors::wakeup_cursor_drain_unit_weight_upper(true));

    let mut reftime_short = WeightMeter::with_limit(Weight::from_parts(
      required.ref_time().saturating_sub(1),
      u64::MAX,
    ));
    assert_eq!(
      Actors::drain_overdue_wakeups_cursor(10, &mut reftime_short).entries_scanned,
      0
    );
    assert!(Actors::actor_hot(actor_id)
      .expect("actor after RefTime stop")
      .wakeup_pointer
      .is_some());

    let mut proof_short = WeightMeter::with_limit(Weight::from_parts(
      u64::MAX,
      required.proof_size().saturating_sub(1),
    ));
    assert_eq!(
      Actors::drain_overdue_wakeups_cursor(10, &mut proof_short).entries_scanned,
      0
    );
    assert!(Actors::actor_hot(actor_id)
      .expect("actor after ProofSize stop")
      .wakeup_pointer
      .is_some());
  });
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
    assert_eq!(System::events(), events_before);
  });
}

#[test]
fn on_idle_wakeup_worker_respects_remaining_weight_in_each_dimension() {
  let required =
    <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_wakeup_cursor_worker_future()
      .saturating_add(Actors::wakeup_cursor_drain_unit_weight_upper(true));
  assert_on_idle_wakeup_insufficiency_preserves_state(Weight::from_parts(
    required.ref_time().saturating_sub(1),
    u64::MAX,
  ));
  assert_on_idle_wakeup_insufficiency_preserves_state(Weight::from_parts(
    u64::MAX,
    required.proof_size().saturating_sub(1),
  ));
  assert_on_idle_wakeup_insufficiency_preserves_state(Weight::from_parts(
    required.ref_time().saturating_sub(1),
    required.proof_size().saturating_sub(1),
  ));
}

#[test]
fn wakeup_materialization_rolls_back_when_queue_ticket_is_exhausted() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    let hot_before = Actors::actor_hot(actor_id).expect("actor before wakeup materialization");
    let bucket_before = Actors::wakeup_buckets(10).expect("wakeup bucket");
    let page_before = Actors::wakeup_pages((10, 0)).expect("wakeup page");
    crate::NextQueueTicket::<Test>::put(u64::MAX);
    let mut meter = WeightMeter::with_limit(Weight::MAX);

    let stats = Actors::drain_overdue_wakeups_cursor(10, &mut meter);

    assert_eq!(stats.entries_scanned, 0);
    assert_eq!(Actors::actor_hot(actor_id), Some(hot_before));
    assert_eq!(Actors::wakeup_buckets(10), Some(bucket_before));
    assert_eq!(Actors::wakeup_pages((10, 0)), Some(page_before));
    assert_eq!(Actors::wakeup_cursor_peek(), Some(10));
  });
}

#[test]
fn wakeup_materialization_rolls_back_on_fifo_topology_corruption() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    QueueTail::<Test>::put(1);
    QueueOccupancy::<Test>::put(0);
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let mut meter = WeightMeter::with_limit(Weight::MAX);

    let stats = Actors::drain_overdue_wakeups_cursor(10, &mut meter);

    assert_eq!(stats.entries_scanned, 0);
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
    assert_eq!(scheduled_wakeup_block(actor_id), Some(10));
  });
}

#[test]
fn paged_wakeup_cursor_orders_sparse_blocks_across_page_boundaries() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size: u32 = <Test as crate::Config>::WakeupPageSize::get();
    let count = page_size.saturating_add(3);
    let mut actors = Vec::new();
    let blocks: Vec<MockBlockNumber> = (0..count)
      .map(|index| 10_000u64.saturating_add(u64::from(index).saturating_mul(10_000)))
      .collect();

    for block in blocks.iter().rev().copied() {
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      assert!(Actors::wakeup_substrate_schedule(actor_id, block));
      assert!(Actors::wakeup_cursor_insert(block));
      actors.push(actor_id);
    }

    assert_eq!(Actors::wakeup_cursor_len(), count);
    assert_eq!(Actors::wakeup_cursor_peek(), blocks.first().copied());
    assert_eq!(
      Actors::wakeup_cursor_pages(0).map(|page| page.len()),
      Some(page_size as usize)
    );
    assert_eq!(
      Actors::wakeup_cursor_pages(1).map(|page| page.len()),
      Some(3)
    );
    assert!(Actors::wakeup_cursor_insert(blocks[0]));
    assert_eq!(Actors::wakeup_cursor_len(), count);

    let removed = blocks[(count / 2) as usize];
    assert!(Actors::wakeup_cursor_remove(removed));
    assert!(!Actors::wakeup_cursor_remove(removed));
    assert_eq!(Actors::wakeup_cursor_len(), count.saturating_sub(1));
    assert_eq!(
      Actors::wakeup_buckets(removed)
        .expect("removed cursor bucket")
        .cursor_index,
      None
    );
    let expected: Vec<_> = blocks
      .iter()
      .copied()
      .filter(|block| *block != removed)
      .collect();
    let mut popped = Vec::new();
    while let Some(block) = Actors::wakeup_cursor_pop_min() {
      popped.push(block);
    }
    assert_eq!(popped, expected);
    assert_eq!(Actors::wakeup_cursor_len(), 0);
    assert!(Actors::wakeup_cursor_pages(0).is_none());
    assert!(Actors::wakeup_cursor_pages(1).is_none());
    assert!(blocks.iter().all(|block| {
      Actors::wakeup_buckets(*block)
        .expect("wakeup bucket")
        .cursor_index
        .is_none()
    }));

    for actor_id in actors {
      let _ = Actors::wakeup_substrate_invalidate(actor_id);
    }
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn actor_storage_schema_is_explicit() {
  let storage_info = Actors::storage_info();
  assert!(
    storage_info
      .iter()
      .all(|entry| entry.pallet_name == b"Actors")
  );
  let actual: alloc::vec::Vec<_> = storage_info
    .iter()
    .map(|entry| core::str::from_utf8(&entry.storage_name).expect("storage name is UTF-8"))
    .collect();
  assert_eq!(
    actual,
    [
      "NextActorId",
      "ActorHot",
      "ActorContract",
      "ActorFunding",
      "ContinuationState",
      "ActorIdentities",
      "ActorIdentityCount",
      "ActiveActorCount",
      "SystemSovereigns",
      "SystemSovereignCount",
      "NextQueueTicket",
      "QueueHead",
      "QueueTail",
      "QueueOccupancy",
      "QueuePages",
      "WakeupPages",
      "WakeupBuckets",
      "WakeupCursorPages",
      "WakeupCursorLen",
      "NextWakeupClock",
      "OwnerSlotBitmaps",
      "SovereignIndex",
      "ActiveActorLimit",
      "ActorObservationFeeds",
      "ObservationSubscriptionSlot",
      "ObservationSubscriptionSlotOwner",
      "NextObservationSubscriptionSlot",
      "ObservationFreeSlotLen",
      "ObservationFreeSlotPages",
      "ObservationSubscriberPages",
      "ObservationSubscriberPageLists",
      "ObservationSubscriberCount",
      "ObservationSubscriptionCount",
      "ObservationIngressRevisions",
      "DirtyObservationFeeds",
      "DirtyObservationListState",
      "GlobalCircuitBreaker",
      "IdleStarvationState",
    ]
  );

  let metadata = Actors::storage_metadata();
  assert_eq!(metadata.prefix, "Actors");
  let actual_shapes: alloc::vec::Vec<_> = metadata
    .entries
    .iter()
    .map(|entry| {
      let optional = matches!(entry.modifier, StorageEntryModifierIR::Optional);
      let is_blake_map = match &entry.ty {
        StorageEntryTypeIR::Plain(_) => false,
        StorageEntryTypeIR::Map { hashers, .. } => {
          assert!(
            hashers
              .iter()
              .all(|hasher| *hasher == StorageHasherIR::Blake2_128Concat)
          );
          true
        }
      };
      (entry.name, optional, is_blake_map)
    })
    .collect();
  assert_eq!(
    actual_shapes,
    [
      ("NextActorId", false, false),
      ("ActorHot", true, true),
      ("ActorContract", true, true),
      ("ActorFunding", true, true),
      ("ContinuationState", true, true),
      ("ActorIdentities", true, true),
      ("ActorIdentityCount", false, false),
      ("ActiveActorCount", false, false),
      ("SystemSovereigns", true, true),
      ("SystemSovereignCount", false, false),
      ("NextQueueTicket", false, false),
      ("QueueHead", false, false),
      ("QueueTail", false, false),
      ("QueueOccupancy", false, false),
      ("QueuePages", true, true),
      ("WakeupPages", true, true),
      ("WakeupBuckets", true, true),
      ("WakeupCursorPages", true, true),
      ("WakeupCursorLen", false, true),
      ("NextWakeupClock", false, false),
      ("OwnerSlotBitmaps", false, true),
      ("SovereignIndex", true, true),
      ("ActiveActorLimit", false, false),
      ("ActorObservationFeeds", true, true),
      ("ObservationSubscriptionSlot", true, true),
      ("ObservationSubscriptionSlotOwner", true, true),
      ("NextObservationSubscriptionSlot", false, false),
      ("ObservationFreeSlotLen", false, false),
      ("ObservationFreeSlotPages", true, true),
      ("ObservationSubscriberPages", true, true),
      ("ObservationSubscriberPageLists", true, true),
      ("ObservationSubscriberCount", false, true),
      ("ObservationSubscriptionCount", false, false),
      ("ObservationIngressRevisions", true, true),
      ("DirtyObservationFeeds", true, true),
      ("DirtyObservationListState", false, false),
      ("GlobalCircuitBreaker", false, false),
      ("IdleStarvationState", false, false),
    ]
  );

  let entries = &metadata.entries;
  assert_plain_storage_type::<u64>(&entries[0]);
  assert_map_storage_types::<u64, crate::ActorHotStateOf<Test>>(&entries[1]);
  assert_map_storage_types::<u64, crate::ActorContractOf<Test>>(&entries[2]);
  assert_map_storage_types::<u64, crate::ActorFundingStateOf<Test>>(&entries[3]);

  let mut registry = scale_info::Registry::new();
  let contract_type =
    registry.register_type(&scale_info::meta_type::<crate::ActorContractOf<Test>>());
  let (_, contract) = registry
    .types()
    .find(|(symbol, _)| symbol.id == contract_type.id)
    .expect("Actor Contract type is registered");
  let scale_info::TypeDef::Composite(contract_fields) = &contract.type_def else {
    panic!("Actor Contract metadata must be composite");
  };
  assert_eq!(
    contract_fields
      .fields
      .iter()
      .map(|field| field.name.as_deref().expect("named Actor Contract field"))
      .collect::<Vec<_>>(),
    [
      "trigger",
      "cooldown_blocks",
      "window",
      "steps",
      "funding",
      "completion",
      "auto_close_at_cycle_nonce"
    ]
  );
  assert_map_storage_types::<u64, RuntimeContinuationState>(&entries[4]);
  assert_map_storage_types::<u64, crate::ActorIdentityOf<Test>>(&entries[5]);
  assert_plain_storage_type::<u32>(&entries[6]);
  assert_plain_storage_type::<u32>(&entries[7]);
  assert_map_storage_types::<u64, SystemSovereignState>(&entries[8]);
  assert_plain_storage_type::<u32>(&entries[9]);
  assert_plain_storage_type::<u64>(&entries[10]);
  assert_plain_storage_type::<u64>(&entries[11]);
  assert_plain_storage_type::<u64>(&entries[12]);
  assert_plain_storage_type::<u32>(&entries[13]);
  assert_map_storage_types::<u64, crate::QueuePageOf<Test>>(&entries[14]);
  assert_map_storage_types::<(WakeupKey<MockBlockNumber>, u64), crate::WakeupPageOf<Test>>(
    &entries[15],
  );
  assert_map_storage_types::<WakeupKey<MockBlockNumber>, WakeupBucketState>(&entries[16]);
  assert_map_storage_types::<(WakeupClock, u64), crate::WakeupCursorPageOf<Test>>(&entries[17]);
  assert_map_storage_types::<WakeupClock, u32>(&entries[18]);
  assert_plain_storage_type::<WakeupClock>(&entries[19]);
  assert_map_storage_types::<AccountId, [u8; 32]>(&entries[20]);
  assert_map_storage_types::<AccountId, u64>(&entries[21]);
  assert_plain_storage_type::<u32>(&entries[22]);
  assert_map_storage_types::<u64, crate::ActorObservationFeedsOf<Test>>(&entries[23]);
  assert_map_storage_types::<u64, u32>(&entries[24]);
  assert_map_storage_types::<u32, u64>(&entries[25]);
  assert_plain_storage_type::<u32>(&entries[26]);
  assert_plain_storage_type::<u32>(&entries[27]);
  assert_map_storage_types::<u32, crate::ObservationFreeSlotPageOf<Test>>(&entries[28]);
  assert_map_storage_types::<(u32, u32), crate::ObservationSubscriberPageOf<Test>>(&entries[29]);
  assert_map_storage_types::<u32, ObservationSubscriberPageList>(&entries[30]);
  assert_map_storage_types::<u32, u32>(&entries[31]);
  assert_plain_storage_type::<u32>(&entries[32]);
  assert_map_storage_types::<u32, u64>(&entries[33]);
  assert_map_storage_types::<
    u32,
    crate::types::DirtyObservationState<
      u32,
      polkadot_sdk::frame_system::pallet_prelude::BlockNumberFor<Test>,
    >,
  >(&entries[34]);
  assert_plain_storage_type::<crate::types::DirtyObservationList<u32>>(&entries[35]);
  assert_plain_storage_type::<bool>(&entries[36]);
  assert_plain_storage_type::<IdleStarvationPhase>(&entries[37]);
}

#[test]
fn fresh_genesis_baseline_carries_no_migration_ceremony() {
  new_test_ext().execute_with(|| {
    // Genesis writes the current storage version directly; no legacy reader,
    // dual write, queue-merge bridge, or migration cursor exists (COMPAT-STORAGE).
    use polkadot_sdk::frame_support::traits::GetStorageVersion;
    let on_chain =
      polkadot_sdk::frame_support::traits::StorageVersion::get::<crate::Pallet<Test>>();
    let in_code = crate::Pallet::<Test>::in_code_storage_version();
    assert_eq!(
      on_chain, in_code,
      "genesis baseline must equal the current storage version"
    );
    // No queue-merge or migration-cursor storage exists on the schema surface.
    let storage_info = Actors::storage_info();
    let names: alloc::vec::Vec<_> = storage_info
      .iter()
      .map(|entry| core::str::from_utf8(&entry.storage_name).expect("UTF-8"))
      .collect();
    assert!(
      names.iter().all(|name| {
        !name.starts_with("Legacy")
          && !name.starts_with("Migration")
          && !name.contains("Merge")
          && !name.contains("OnRuntimeUpgrade")
      }),
      "fresh baseline must not carry migration-ceremony storage: {names:?}"
    );
    // The embedding fixture independently starts from the same fresh schema.
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn continuation_schema_round_trips_retry_position_and_typed_snapshot_surfaces() {
  new_test_ext().execute_with(|| {
    let mut contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageAtOpening(Perbill::one()),
      }),
      make_step(Task::Unstake {
        asset: TestAsset::Local(1),
        shares: AmountResolution::PercentageAtOpening(Perbill::one()),
      }),
    ])
    .expect("two-step plan fits");
    contract_steps[0].on_error = RETRY_LATER;
    let actor_id = create_system_with(ALICE, percentage_trigger_schedule(), None, contract_steps);
    let mut opening_snapshot: BoundedBTreeMap<
      OpeningSurface<TestAsset>,
      Balance,
      <Test as crate::Config>::MaxOpeningSnapshotEntries,
    > = Default::default();
    opening_snapshot
      .try_insert(OpeningSurface::PreservableAsset(TestAsset::Native), 100)
      .expect("asset snapshot fits");
    opening_snapshot
      .try_insert(OpeningSurface::StakingShares(TestAsset::Local(1)), 40)
      .expect("staking snapshot fits");
    let continuation = RuntimeContinuationState {
      cursor: 0,
      unsuccessful_attempts_at_cursor: 1,
      last_attempt_block: 1,
      opening_snapshot,
      opening_predicate_results: Default::default(),
      funding_snapshot: Default::default(),
      cumulative_outcomes: OutcomeTotals {
        executed_steps: 1,
        ..Default::default()
      },
    };
    let encoded = continuation.encode();
    let decoded =
      RuntimeContinuationState::decode(&mut &encoded[..]).expect("continuation decodes");
    assert_eq!(decoded.cursor, 0);
    assert_eq!(decoded.unsuccessful_attempts_at_cursor, 1);
    assert_eq!(decoded.last_attempt_block, 1);
    assert_eq!(decoded.opening_snapshot.len(), 2);
    assert_eq!(decoded.cumulative_outcomes.executed_steps, 1);

    ContinuationStateStore::<Test>::insert(actor_id, continuation);
    ActorHot::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("active actor").cycle_state = CycleState::Suspended;
    });
    ActorIdentities::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("actor identity").cycle_nonce = 1;
    });
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("continuation exists")
        .unsuccessful_attempts_at_cursor,
      1
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn ordinary_one_attempt_run_keeps_continuation_sparse() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_eq!(
      Actors::actor_hot(actor_id)
        .expect("actor exists")
        .cycle_state,
      CycleState::Idle
    );
    assert!(Actors::continuation_state(actor_id).is_none());
    fund_native(actor_id, 10);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::actor_hot(actor_id)
        .expect("actor exists")
        .cycle_state,
      CycleState::Idle
    );
    assert!(Actors::continuation_state(actor_id).is_none());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn owner_slot_bitmap_try_state_rejects_invalid_and_orphaned_bits() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    OwnerSlotBitmaps::<Test>::mutate(ALICE, |bitmap| bitmap[31] |= 0b1000_0000);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    OwnerSlotBitmaps::<Test>::mutate(ALICE, |bitmap| bitmap[31] &= 0b0111_1111);
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    OwnerSlotBitmaps::<Test>::insert(CHARLIE, [1; 32]);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn continuation_try_state_rejects_marker_and_cursor_drift() {
  new_test_ext().execute_with(|| {
    let mut plan = transfer_contract_steps(BOB, 1);
    plan[0].on_error = RETRY_LATER;
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    ContinuationStateStore::<Test>::insert(
      actor_id,
      RuntimeContinuationState {
        cursor: 0,
        unsuccessful_attempts_at_cursor: 1,
        last_attempt_block: 1,
        opening_snapshot: Default::default(),
        opening_predicate_results: Default::default(),
        funding_snapshot: Default::default(),
        cumulative_outcomes: Default::default(),
      },
    );
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    ActorHot::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("actor exists").cycle_state = CycleState::Suspended;
    });
    ActorIdentities::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("identity exists").cycle_nonce = 1;
    });
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    ContinuationStateStore::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("continuation exists")
        .opening_snapshot
        .try_insert(OpeningSurface::PreservableAsset(TestAsset::Native), 10)
        .expect("snapshot entry fits");
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    ContinuationStateStore::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("continuation exists")
        .opening_snapshot
        .clear();
    });
    ContinuationStateStore::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("continuation exists").cursor = 1;
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    ContinuationStateStore::<Test>::remove(actor_id);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
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
  assert!(Actors::continuation_state(actor_id).is_some());
  actor_id
}

fn user_step_fee(step: &StepOf<Test>) -> Balance {
  let plan = BoundedVec::try_from(vec![step.clone()]).expect("one step fits the shared bound");
  Actors::attempt_fee_envelope(ActorType::User, &plan, 0)
    .expect("one step has a checked fee envelope")
    .total
}

fn fund_native_raw(who: &AccountId, amount: Balance) {
  let _ = <Balances as frame::traits::Currency<AccountId>>::deposit_creating(who, amount);
}

/// User Active prefunding requirement: `MinUserBalance + attempt_fee_envelope(plan, 0, User).total`.
fn user_prefunding_requirement(plan: &crate::ContractSteps<crate::mock::Test>) -> Balance {
  <Test as crate::Config>::MinUserBalance::get().saturating_add(
    Actors::attempt_fee_envelope(ActorType::User, plan, 0)
      .expect("fixture plan has a checked fee envelope")
      .total,
  )
}

/// Pre-funds the deterministic User sovereign account so Active creation/activation admits
/// (spec 7.1), without mutating any pallet state.
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

fn run_idle(weight: Weight) {
  let now = frame_system::Pallet::<Test>::block_number();
  Actors::on_idle(now, weight);
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
  let hot = Actors::scheduler_actor_hot_probe_weight_upper();
  let contract = Actors::scheduler_actor_contract_probe_weight_upper();
  let consume = <TestWeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
    .max(<TestWeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page());
  let instance = Actors::active_actor_view(actor_id).expect("actor exists");
  let cycle =
    Actors::compute_cycle_weight_upper(instance.actor_class.actor_type(), &instance.steps);
  let full = base
    .saturating_add(cursor)
    .saturating_add(scan)
    .saturating_add(hot)
    .saturating_add(contract)
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

fn has_actor_event(predicate: impl Fn(&Event<Test>) -> bool) -> bool {
  frame_system::Pallet::<Test>::events()
    .into_iter()
    .filter_map(|record| match record.event {
      RuntimeEvent::Actors(event) => Some(event),
      _ => None,
    })
    .any(|event| predicate(&event))
}

#[test]
fn create_user_charges_creation_fee_and_emits_event() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let fee = TestActorCreationFee::get();
    let fee_sink = TestFeeSink::get();
    let owner_before = native_balance(&ALICE);
    let sink_before = native_balance(&fee_sink);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    let inst = Actors::active_actor_view(actor_id).expect("Actors must exist");
    assert_eq!(inst.actor_class, ActorClass::User { owner_slot: 0 });
    assert_eq!(native_balance(&ALICE), owner_before.saturating_sub(fee));
    assert_eq!(native_balance(&fee_sink), sink_before.saturating_add(fee));
    assert_eq!(OwnerSlotBitmaps::<Test>::get(ALICE)[0], 0b0000_0001);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorCreated {
          actor_id: id,
          owner,
          actor_class: ActorClass::User { owner_slot: 0 },
          mutability: Mutability::Mutable,
          initial_lifecycle: InitialLifecycle::Active,
          ..
        } if *id == actor_id && *owner == ALICE
      )
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::ActorActivated { actor_id: id } if *id == actor_id)
    }));
  });
}

#[test]
fn both_user_creation_calls_charge_dormant_admission_fee_before_identity_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let fee = TestActorCreationFee::get();
    let fee_sink = TestFeeSink::get();
    let sink_before = native_balance(&fee_sink);
    let alice_before = native_balance(&ALICE);
    let charlie_before = native_balance(&CHARLIE);

    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      None,
    ));
    assert_ok!(Actors::create_user_actor_at_slot(
      RuntimeOrigin::signed(CHARLIE),
      2,
      Mutability::Mutable,
      None,
    ));

    assert_eq!(native_balance(&ALICE), alice_before.saturating_sub(fee));
    assert_eq!(native_balance(&CHARLIE), charlie_before.saturating_sub(fee));
    assert_eq!(
      native_balance(&fee_sink),
      sink_before.saturating_add(fee.saturating_mul(2))
    );
    assert_eq!(Actors::owner_slot_bitmap(ALICE)[0], 0b0000_0001);
    assert_eq!(Actors::owner_slot_bitmap(CHARLIE)[0], 0b0000_0100);
    assert_eq!(Actors::actor_identity_count(), 2);
    assert_eq!(Actors::active_actor_count(), 0);
  });
}

#[test]
fn user_active_creation_requires_prefunded_sovereign_before_opening_fee() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // An unfunded User Active creation fails InsufficientBalance before the opening fee
    // or any identity/locator mutation (spec 7.1).
    let owner_before = native_balance(&ALICE);
    let sink_before = native_balance(&TestFeeSink::get());
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::InsufficientBalance
    );
    assert_eq!(Actors::next_actor_id(), 0);
    assert_eq!(Actors::actor_identity_count(), 0);
    assert_eq!(Actors::active_actor_count(), 0);
    assert_eq!(Actors::owner_slot_bitmap(ALICE), [0; 32]);
    assert_eq!(native_balance(&ALICE), owner_before);
    assert_eq!(native_balance(&TestFeeSink::get()), sink_before);
    assert!(fee_collections().is_empty());

    // Dormant creation remains unfunded and admits with no sovereign balance.
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      None,
    ));
    assert_eq!(native_balance(&Actors::sovereign_account_id(&ALICE, 0)), 0);
  });
}

#[test]
fn user_active_creation_prefunding_boundary_is_exact_floor_plus_envelope() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let plan = transfer_contract_steps(BOB, 1);
    let requirement = user_prefunding_requirement(&plan);
    // requirement - 1 still fails closed before any opening-fee mutation.
    fund_native_raw(
      &Actors::sovereign_account_id(&ALICE, 0),
      requirement.saturating_sub(1),
    );
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, plan.clone()),
      ),
      Error::<Test>::InsufficientBalance
    );
    assert_eq!(Actors::actor_identity_count(), 0);
    assert_eq!(Actors::owner_slot_bitmap(ALICE), [0; 32]);
    // Exactly floor + envelope admits.
    fund_native_raw(&Actors::sovereign_account_id(&ALICE, 0), 1);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, plan),
    ));
    assert_eq!(Actors::active_actor_count(), 1);
  });
}

#[test]
fn user_dormant_fund_then_activate_is_the_unfunded_lifecycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // The unfunded lifecycle is create Dormant -> fund the deterministic sovereign ->
    // activate; activation of an unfunded sovereign fails InsufficientBalance.
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      None,
    ));
    let actor_id = Actors::next_actor_id() - 1;
    let plan = transfer_contract_steps(BOB, 1);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_noop!(
      Actors::activate_actor(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        user_active_contract(manual_schedule(), None, plan.clone()).expect("direct Actor Contract"),
      ),
      Error::<Test>::InsufficientBalance
    );
    assert!(Actors::actor_identities(actor_id).is_some());
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(Actors::active_actor_count(), 0);

    // Funding the deterministic sovereign account admits activation.
    fund_native_raw(
      &Actors::sovereign_account_id(&ALICE, 0),
      user_prefunding_requirement(&plan),
    );
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      user_active_contract(manual_schedule(), None, plan).expect("direct Actor Contract"),
    ));
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert_eq!(Actors::active_actor_count(), 1);
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
      transfer_contract_steps(BOB, 10),
    );
    assert_eq!(native_balance(&ALICE), owner_before);
  });
}

#[test]
fn system_creation_accepts_absent_contract() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None,
    ));
    let identity = Actors::actor_identities(0).expect("dormant identity exists");
    assert_eq!(identity.actor_class, ActorClass::System { sovereign_id: 0 });
    assert!(Actors::active_actor_view(0).is_none());
    assert_eq!(Actors::actor_identity_count(), 1);
    assert_eq!(Actors::active_actor_count(), 0);
  });
}

#[test]
fn exact_slot_user_creation_accepts_absent_contract() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let owner_slot = 2;
    assert_ok!(Actors::create_user_actor_at_slot(
      RuntimeOrigin::signed(ALICE),
      owner_slot,
      Mutability::Mutable,
      None,
    ));
    let identity = Actors::actor_identities(0).expect("dormant identity exists");
    assert_eq!(identity.actor_class, ActorClass::User { owner_slot });
    assert!(Actors::active_actor_view(0).is_none());
    assert_eq!(Actors::actor_identity_count(), 1);
    assert_eq!(Actors::active_actor_count(), 0);
  });
}

#[test]
fn dormant_identity_owns_no_scheduler_state_and_round_trips_activation() {
  new_test_ext().execute_with(|| {
    use polkadot_sdk::frame_support::traits::{Currency, Hooks};
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      None,
    ));
    let actor_id = 0;
    let identity = Actors::actor_identities(actor_id).expect("dormant identity exists");
    assert_eq!(Actors::actor_identity_count(), 1);
    assert_eq!(Actors::active_actor_count(), 0);
    assert!(Actors::active_actor_view(actor_id).is_none());
    System::reset_events();
    for block in 2..=5 {
      System::set_block_number(block);
      let _ = <Actors as Hooks<MockBlockNumber>>::on_idle(block, Weight::MAX);
    }
    assert!(System::events().iter().all(|record| !matches!(
      record.event,
      RuntimeEvent::Actors(Event::CycleStarted { actor_id: id, .. })
        | RuntimeEvent::Actors(Event::CycleSummary { actor_id: id, .. }) if id == actor_id
    )));
    let preserved = 777;
    let _ =
      <Balances as Currency<AccountId>>::deposit_creating(&identity.sovereign_account, preserved);
    assert_noop!(
      Actors::activate_actor(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        user_active_contract(manual_schedule(), None, BoundedVec::default())
          .expect("direct empty contract"),
      ),
      Error::<Test>::EmptyContractSteps
    );
    assert!(Actors::actor_identities(actor_id).is_some());
    assert_eq!(Actors::active_actor_count(), 0);
    assert_noop!(
      Actors::activate_actor(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        ActorContract {
          funding: FundingSourcePolicy::AnyVerifiedIngress,
          ..user_active_contract(
            manual_schedule(),
            None,
            contract_steps_with_step(make_step(Task::Mint {
              asset: TestAsset::Native,
              amount: AmountResolution::Fixed(1),
            })),
          )
          .expect("direct Actor Contract")
        },
      ),
      Error::<Test>::MintNotAllowedForUserActor
    );
    assert!(Actors::actor_identities(actor_id).is_some());
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(Actors::active_actor_count(), 0);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      ActorContract {
        funding: FundingSourcePolicy::AnyVerifiedIngress,
        ..user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 10))
          .expect("direct Actor Contract")
      },
    ));
    assert!(Actors::actor_identities(actor_id).is_some());
    let _activated = Actors::active_actor_view(actor_id).expect("active Actor Contract exists");
    assert_eq!(
      ActorContracts::<Test>::get(actor_id)
        .expect("active Actor Contract")
        .funding,
      FundingSourcePolicy::AnyVerifiedIngress
    );
    assert_eq!(Actors::actor_identity_count(), 1);
    assert_eq!(Actors::active_actor_count(), 1);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(Actors::actor_funding(actor_id).is_none());
    assert!(Actors::actor_identities(actor_id).is_some());
    assert_eq!(Actors::actor_identity_count(), 1);
    assert_eq!(Actors::active_actor_count(), 0);
    assert_eq!(native_balance(&identity.sovereign_account), preserved);
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(Actors::actor_identities(actor_id).is_none());
    assert_eq!(Actors::actor_identity_count(), 0);
    assert_eq!(Actors::owner_slot_bitmap(ALICE), [0; 32]);
    assert_eq!(native_balance(&identity.sovereign_account), preserved);
  });
}

#[test]
fn deactivate_activate_preserves_nonce_but_resets_active_epoch_state_for_both_classes() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let user_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let system_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    fund_native(user_id, 1_000_000_000_000);
    fund_native(system_id, 100);
    for actor_id in [user_id, system_id] {
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
    }
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(user_id)
        .expect("User active")
        .cycle_nonce,
      1
    );
    assert_eq!(
      Actors::active_actor_view(system_id)
        .expect("System active")
        .cycle_nonce,
      1
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      user_id,
    ));
    assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), system_id));
    for actor_id in [user_id, system_id] {
      let dormant = Actors::actor_identities(actor_id).expect("durable identity");
      assert_eq!(dormant.cycle_nonce, 1);
      assert!(Actors::actor_funding(actor_id).is_none());
      assert!(Actors::continuation_state(actor_id).is_none());
    }

    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      user_id,
      user_active_contract(manual_schedule(), None, inert_contract_steps())
        .expect("direct Actor Contract"),
    ));
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::root(),
      system_id,
      system_active_contract(manual_schedule(), None, inert_contract_steps())
        .expect("direct Actor Contract"),
    ));
    for actor_id in [user_id, system_id] {
      let active = Actors::active_actor_view(actor_id).expect("reactivated actor");
      assert_eq!(active.cycle_nonce, 1);
      assert_eq!(active.unsuccessful_attempt_streak, 0);
      assert!(!active.pending_signal);
      assert!(actor_funding(actor_id).funding_accumulated.is_empty());
    }

    frame_system::Pallet::<Test>::set_block_number(4);
    for actor_id in [user_id, system_id] {
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
    }
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(user_id)
        .expect("User active")
        .cycle_nonce,
      2
    );
    assert_eq!(
      Actors::active_actor_view(system_id)
        .expect("System active")
        .cycle_nonce,
      2
    );
  });
}

#[test]
fn reactivation_with_positive_nonce_uses_schedule_anchor_for_cooldown() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 10,
    };
    let actor_id = create_system_with(
      ALICE,
      schedule.clone(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    // First run: nonce 0 -> 1, last_cycle_block = Some(1).
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("active")
        .cycle_nonce,
      1
    );
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot").last_cycle_block,
      Some(1)
    );
    // Deactivate at block 5.
    frame_system::Pallet::<Test>::set_block_number(5);
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    // Reactivate at block 8 with the same contract: the fresh hot state has no
    // last_cycle_block, so the conservative schedule_anchor (8) anchors cooldown.
    frame_system::Pallet::<Test>::set_block_number(8);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      system_active_contract(schedule, None, transfer_contract_steps(BOB, 10))
        .expect("direct Actor Contract"),
    ));
    let instance = Actors::active_actor_view(actor_id).expect("reactivated");
    assert_eq!(instance.cycle_nonce, 1);
    assert_eq!(instance.schedule_anchor, 8);
    assert_eq!(instance.last_cycle_block, None);
    // A manual trigger at block 8 must NOT fire immediately: cooldown runs from the
    // anchor (8 + 10 = 18), not from block zero.
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("still active")
        .cycle_nonce,
      1,
      "reactivated actor must not become immediately eligible"
    );
    // At block 18 the cooldown has elapsed and the run proceeds.
    frame_system::Pallet::<Test>::set_block_number(18);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("active")
        .cycle_nonce,
      2
    );
  });
}

#[test]
fn attempt_fee_envelope_owns_step_and_suffix_bounds() {
  new_test_ext().execute_with(|| {
    let plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
      make_step(Task::Burn {
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
    ])
    .expect("two steps fit the shared bound");
    let envelope = Actors::attempt_fee_envelope(ActorType::User, &plan, 0)
      .expect("User plan has a checked fee envelope");
    assert_eq!(envelope.steps.len(), 2);
    for cursor in 0..=plan.len() {
      let suffix = Actors::attempt_fee_envelope(ActorType::User, &plan, cursor)
        .expect("User suffix has a checked fee envelope");
      let expected = suffix.steps.iter().fold(0u128, |total, step| {
        total
          .checked_add(step.total)
          .expect("small test fees cannot overflow")
      });
      assert_eq!(suffix.total, expected);
      if cursor == 1 {
        assert_eq!(suffix.steps.len(), 1);
        assert_eq!(suffix.total, envelope.steps[1].total);
      }
    }
    let system = Actors::attempt_fee_envelope(ActorType::System, &plan, 0)
      .expect("System plan has a checked fee envelope");
    assert_eq!(system.total, 0);
    assert!(system.steps.iter().all(|step| step.total == 0));
  });
}

#[test]
fn fee_envelope_composition_is_portable_and_checked() {
  type VectorBound = frame::traits::ConstU32<2>;
  let inputs = BoundedVec::<_, VectorBound>::try_from(vec![
    FeeEnvelopeInput {
      evaluation: 2u128,
      execution: 100,
    },
    FeeEnvelopeInput {
      evaluation: 5,
      execution: 7,
    },
  ])
  .expect("portable vector inputs fit");
  let user = compose_attempt_fee_envelope(ActorType::User, &inputs, 1)
    .expect("User portable suffix composes");
  assert_eq!(user.steps[0].total, 12);
  assert_eq!(user.total, 12);
  let system = compose_attempt_fee_envelope(ActorType::System, &inputs, 0)
    .expect("System portable suffix composes");
  assert_eq!(system.total, 0);
  assert!(matches!(
    compose_attempt_fee_envelope(ActorType::User, &inputs, 3),
    Err(FeeEnvelopeError::CursorOutOfBounds)
  ));
  let overflowing = BoundedVec::<_, VectorBound>::try_from(vec![FeeEnvelopeInput {
    evaluation: u128::MAX,
    execution: 1,
  }])
  .expect("overflow vector input fits");
  assert!(matches!(
    compose_attempt_fee_envelope(ActorType::User, &overflowing, 0),
    Err(FeeEnvelopeError::Overflow)
  ));
}

#[test]
fn fee_envelope_settlement_releases_reservation_and_prices_attempts() {
  type VectorBound = frame::traits::ConstU32<2>;
  let inputs = BoundedVec::<_, VectorBound>::try_from(vec![
    FeeEnvelopeInput {
      evaluation: 2u128,
      execution: 100,
    },
    FeeEnvelopeInput {
      evaluation: 5,
      execution: 7,
    },
  ])
  .expect("settlement vector inputs fit");
  let envelope =
    compose_attempt_fee_envelope(ActorType::User, &inputs, 0).expect("User envelope composes");
  let skipped = settle_attempt_fee_step(
    ActorType::User,
    envelope.total,
    &envelope.steps[0],
    FeeChargeKind::EvaluationOnly,
  )
  .expect("first User settlement has a reservation");
  assert_eq!(skipped.charged, 2);
  assert_eq!(skipped.reservation_remaining, 12);
  let attempted = settle_attempt_fee_step(
    ActorType::User,
    skipped.reservation_remaining,
    &envelope.steps[1],
    FeeChargeKind::Attempted,
  )
  .expect("second User settlement releases the suffix to zero");
  assert_eq!(attempted.charged, 12);
  assert_eq!(attempted.reservation_remaining, 0);
  assert!(matches!(
    settle_attempt_fee_step(
      ActorType::User,
      11,
      &envelope.steps[1],
      FeeChargeKind::Attempted,
    ),
    Err(FeeEnvelopeError::ReservationUnderflow)
  ));
  let system = settle_attempt_fee_step(
    ActorType::System,
    0,
    &envelope.steps[0],
    FeeChargeKind::Attempted,
  )
  .expect("System settlement remains fee-exempt");
  assert_eq!(system.charged, 0);
  assert_eq!(system.reservation_remaining, 0);
}

#[test]
fn fee_native_protected_minimum_uses_the_configured_user_fee_native_floor() {
  assert_eq!(
    fee_native_protected_minimum(ActorType::User, true, 1u128, 50),
    50
  );
  assert_eq!(
    fee_native_protected_minimum(ActorType::User, true, 100u128, 50),
    50
  );
  assert_eq!(
    fee_native_protected_minimum(ActorType::User, false, 1u128, 50),
    1
  );
  assert_eq!(
    fee_native_protected_minimum(ActorType::System, true, 100u128, 50),
    100
  );
}

#[test]
fn guaranteed_actor_service_rejects_housekeeping_underflow_in_each_dimension() {
  new_test_ext().execute_with(|| {
    let fixed = <TestWeightInfo as crate::WeightInfo>::scheduler_on_idle_base()
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1))
      .saturating_add(TestWakeupWeightLimit::get())
      .saturating_add(TestObservationFanoutWeightLimit::get());

    set_guaranteed_on_idle_weight(Weight::from_parts(
      fixed.ref_time().saturating_sub(1),
      u64::MAX,
    ));
    assert!(Actors::guaranteed_actor_service_weight().is_none());
    set_guaranteed_on_idle_weight(Weight::from_parts(
      u64::MAX,
      fixed.proof_size().saturating_sub(1),
    ));
    assert!(Actors::guaranteed_actor_service_weight().is_none());
  });
}

#[test]
fn create_admission_enforces_both_idle_weight_dimensions_before_charging() {
  new_test_ext().execute_with(|| {
    let contract_steps = transfer_contract_steps(BOB, 10);
    let required = Actors::contract_steps_admission_weight_upper(ActorType::User, &contract_steps);
    let fixed = <TestWeightInfo as crate::WeightInfo>::scheduler_on_idle_base()
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1))
      .saturating_add(TestWakeupWeightLimit::get())
      .saturating_add(TestObservationFanoutWeightLimit::get());
    let gross_required = required.saturating_add(fixed);
    set_guaranteed_on_idle_weight(gross_required);
    prefund_active_user_creation(ALICE, &contract_steps);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, contract_steps.clone()),
    ));
    let owner_before = native_balance(&BOB);
    set_guaranteed_on_idle_weight(Weight::from_parts(
      gross_required.ref_time(),
      gross_required.proof_size().saturating_sub(1),
    ));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(BOB),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, contract_steps),
      ),
      Error::<Test>::ContractStepsExceedOnIdleBudget
    );
    assert_eq!(native_balance(&BOB), owner_before);
  });
}

#[test]
fn on_idle_never_consumes_above_the_runtime_reserve() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let reserve = <TestWeightInfo as crate::WeightInfo>::scheduler_on_idle_base();
    set_guaranteed_on_idle_weight(reserve);

    let used = Actors::on_idle(1, Weight::MAX);

    assert!(used.all_lte(reserve));
    assert_eq!(
      Actors::actor_identities(actor_id)
        .expect("actor identity remains")
        .cycle_nonce,
      0,
    );
  });
}

#[test]
fn plan_updates_reject_a_prospective_run_above_the_idle_budget() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let before = Actors::active_actor_view(actor_id).expect("Actors exists");
    let replacement = transfer_contract_steps(BOB, 10);
    let required = Actors::contract_steps_admission_weight_upper(ActorType::User, &replacement);
    set_guaranteed_on_idle_weight(Weight::from_parts(
      required.ref_time(),
      required.proof_size().saturating_sub(1),
    ));
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        replacement,
        crate::CompletionPolicy::Persistent,
      ),
      Error::<Test>::ContractStepsExceedOnIdleBudget
    );
    assert_eq!(Actors::active_actor_view(actor_id), Some(before));
  });
}

#[test]
fn test_weight_fallback_equals_reference_interface_for_all_classes() {
  type Reference = crate::weights::SubstrateWeight<Test>;
  macro_rules! same {
    ($($method:ident),+ $(,)?) => {$({
      assert_eq!(
        <TestWeightInfo as crate::WeightInfo>::$method(),
        <Reference as crate::WeightInfo>::$method(),
        stringify!($method),
      );
    })+};
  }
  same!(
    create_user_actor,
    create_user_actor_at_slot,
    create_system_actor,
    create_system_actor_at_sovereign_id,
    create_dormant_system_actor,
    activate_actor,
    deactivate_actor,
    pause_actor,
    resume_actor,
    manual_trigger,
    observation_change_ingress,
    observation_fanout_base,
    observation_fanout_page,
    close_actor,
    fee_collection,
    cycle_orchestration,
    task_transfer,
    task_burn,
    task_mint,
    task_stop_cycle,
    xcm_asset_deposit,
    task_add_liquidity,
    task_donate_liquidity,
    task_remove_liquidity,
    task_stake,
    task_unstake,
    task_dex_exact_in,
    task_dex_exact_out,
    scheduler_on_idle_base,
    scheduler_paged_append_existing_page,
    scheduler_paged_append_new_page,
    scheduler_wakeup_append_existing_page,
    scheduler_wakeup_append_new_page,
    scheduler_wakeup_replace_exact,
    scheduler_wakeup_invalidate_middle_page,
    scheduler_wakeup_drain_partial_page,
    scheduler_wakeup_drain_full_page,
    scheduler_wakeup_drain_dense_boundary,
    scheduler_wakeup_drain_stale_page,
    scheduler_wakeup_cursor_insert,
    scheduler_wakeup_cursor_pop_min,
    scheduler_wakeup_cursor_remove_exact,
    scheduler_wakeup_cursor_worker_partial,
    scheduler_wakeup_cursor_worker_remove,
    scheduler_wakeup_cursor_worker_future,
    scheduler_paged_consume_preserve_page,
    scheduler_paged_consume_delete_page,
    scheduler_actor_hot_probe,
    scheduler_actor_contract_probe,
    transaction_extension_ingress_base,
    transaction_extension_ingress_notify,
    continuation_retry,
    continuation_complete,
    continuation_cancel,
    update_contract,
    set_global_circuit_breaker,
    set_active_actor_limit,
    permissionless_sweep,
  );
  macro_rules! same_at {
    ($method:ident, $($value:expr),+ $(,)?) => {$({
      assert_eq!(
        <TestWeightInfo as crate::WeightInfo>::$method($value),
        <Reference as crate::WeightInfo>::$method($value),
        concat!(stringify!($method), " parameterized"),
      );
    })+};
  }
  same_at!(predicate_set_evaluation, 0, 1, 8);
  same_at!(task_split_transfer, 0, 1, 8);
  same_at!(step_orchestration, 0, 1, 8);
  same_at!(scheduler_paged_tombstone_drain, 0, 1, 10_000);
  same_at!(scheduler_paged_mixed_scan, 0, 1, 10_000);
  same_at!(scheduler_paged_execute_cheap, 0, 1, 1_000);
  same_at!(scheduler_paged_execute_cheap_mixed, 0, 1, 1_000);
  same_at!(funding_snapshot_open, 0, 1, 10);
  same_at!(continuation_suspend, 0, 1, 16);
  same_at!(continuation_suffix_admission, 0, 1, 8);
  same_at!(permissionless_sweep_many, 0, 1, 5);
}

#[test]
fn transfer_burn_and_mint_use_independent_weight_classes() {
  new_test_ext().execute_with(|| {
    let transfer = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    };
    let burn = Task::Burn {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    };
    let mint = Task::Mint {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    };
    assert_eq!(
      Actors::weight_upper_bound(&transfer),
      <TestWeightInfo as crate::WeightInfo>::task_transfer()
    );
    assert_eq!(
      Actors::weight_upper_bound(&burn),
      <TestWeightInfo as crate::WeightInfo>::task_burn()
    );
    assert_eq!(
      Actors::weight_upper_bound(&mint),
      <TestWeightInfo as crate::WeightInfo>::task_mint()
    );
  });
}

#[test]
fn user_cycle_weight_includes_one_fee_collection_per_step() {
  new_test_ext().execute_with(|| {
    let contract_steps = inert_contract_steps();
    let system_weight = Actors::compute_cycle_weight_upper(ActorType::System, &contract_steps);
    let user_weight = Actors::compute_cycle_weight_upper(ActorType::User, &contract_steps);
    assert_eq!(
      user_weight,
      system_weight.saturating_add(<TestWeightInfo as crate::WeightInfo>::fee_collection())
    );
  });
}

#[test]
fn generated_continuation_weights_cover_distinct_storage_paths() {
  new_test_ext().execute_with(|| {
    let suspend_min = <TestWeightInfo as crate::WeightInfo>::continuation_suspend(0);
    let suspend_max = <TestWeightInfo as crate::WeightInfo>::continuation_suspend(20);
    assert_eq!(suspend_min, Weight::from_parts(27_920_348, 4_178));
    assert_eq!(suspend_max, Weight::from_parts(28_668_868, 4_178));
    assert_eq!(
      <TestWeightInfo as crate::WeightInfo>::continuation_retry(),
      Weight::from_parts(22_070_000, 4_266)
    );
    assert_eq!(
      <TestWeightInfo as crate::WeightInfo>::continuation_complete(),
      Weight::from_parts(18_019_000, 4_030)
    );
    assert_eq!(
      <TestWeightInfo as crate::WeightInfo>::continuation_cancel(),
      Weight::from_parts(56_782_000, 8_120)
    );
    let suffix_min = <TestWeightInfo as crate::WeightInfo>::continuation_suffix_admission(1);
    let suffix_max = <TestWeightInfo as crate::WeightInfo>::continuation_suffix_admission(10);
    assert_eq!(suffix_min, Weight::from_parts(1_439_006, 0));
    assert_eq!(suffix_max, Weight::from_parts(1_442_894, 0));
    assert!(suffix_min.ref_time() < suffix_max.ref_time());
    assert_eq!(suffix_min.proof_size(), suffix_max.proof_size());
  });
}

#[test]
fn generated_predicate_weight_scales_and_chunks_opening_plus_step_visits() {
  use crate::WeightInfo;

  let zero = <Test as crate::Config>::WeightInfo::predicate_set_evaluation(0);
  let one = <Test as crate::Config>::WeightInfo::predicate_set_evaluation(1);
  let maximum = <Test as crate::Config>::WeightInfo::predicate_set_evaluation(4);
  assert_eq!(zero, Weight::zero());
  assert!(one.ref_time() > 0 && one.proof_size() > 0);
  assert!(maximum.ref_time() >= one.ref_time());
  assert!(maximum.proof_size() >= one.proof_size());
  let doubled = Actors::predicate_evaluation_weight(8);
  assert_eq!(doubled, maximum.saturating_mul(2));
  assert!(doubled.ref_time() > maximum.ref_time());
  assert!(doubled.proof_size() > maximum.proof_size());
  assert!(maximum.ref_time() >= 41_068_000);
  assert!(maximum.proof_size() >= 9_045);
}

#[test]
fn canonical_instance_readiness_state_tracks_lifecycle_and_schedule() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    let initial = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(initial.actor_class.actor_type(), ActorType::User);
    assert!(matches!(initial.trigger, Trigger::Manual));
    assert_eq!(initial.lifecycle, ActiveLifecycle::Active);
    assert!(!initial.pending_signal);
    assert_eq!(initial.cycle_nonce, 0);
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .pending_signal
    );
    run_idle(Weight::MAX);
    let after_cycle = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(!after_cycle.pending_signal);
    assert_eq!(after_cycle.cycle_nonce, 1);
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .lifecycle,
      ActiveLifecycle::Paused
    );
    let timer_schedule = Schedule {
      trigger: Trigger::cadenced(3),
      cooldown_blocks: 0,
    };
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      timer_schedule,
      None,
    ));
    let after_update = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(after_update.cooldown_blocks, 0);
    assert!(matches!(
      after_update.trigger,
      Trigger::Cadenced { every_ticks: 3 }
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
        transfer_contract_steps(BOB, 1),
      );
    }
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::OwnerSlotCapacityExceeded
    );
  });
}

#[test]
fn active_actor_capacity_is_enforced() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::set_active_actor_limit(RuntimeOrigin::root(), 1));
    let _first = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::ActiveActorCapacityExceeded
    );
  });
}

#[test]
fn actor_contract_installs_and_validates_auto_close_target() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      Some(ActorContract {
        auto_close_at_cycle_nonce: Some(2),
        ..system_active_contract(manual_schedule(), None, inert_contract_steps())
          .expect("direct Actor Contract")
      }),
    ));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("active actor exists")
        .auto_close_at_cycle_nonce,
      Some(2)
    );

    let next_id = Actors::next_actor_id();
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        Some(ActorContract {
          auto_close_at_cycle_nonce: Some(0),
          ..system_active_contract(manual_schedule(), None, inert_contract_steps())
            .expect("direct Actor Contract")
        }),
      ),
      Error::<Test>::InvalidAutoCloseNonce
    );
    assert_eq!(Actors::next_actor_id(), next_id);
  });
}

#[test]
fn system_actor_count_is_not_limited_by_owner_slots() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let attempts = <<Test as crate::Config>::MaxOwnerSlots as Get<u8>>::get() as u64 + 2;
    let mut sovereign_accounts: Vec<AccountId> = Vec::new();
    for _ in 0..attempts {
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      let inst = Actors::active_actor_view(actor_id).expect("Actors exists");
      assert_eq!(
        inst.actor_class,
        ActorClass::System {
          sovereign_id: actor_id,
        }
      );
      sovereign_accounts.push(inst.sovereign_account);
    }
    assert_eq!(OwnerSlotBitmaps::<Test>::get(ALICE), [0; 32]);
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
      transfer_contract_steps(BOB, 10),
    );
    assert_eq!(
      Actors::active_actor_view(user_id)
        .expect("user Actors exists")
        .actor_class,
      ActorClass::User { owner_slot: 0 }
    );
    assert_eq!(OwnerSlotBitmaps::<Test>::get(ALICE)[0], 0b0000_0001);
    let system_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let system = Actors::active_actor_view(system_id).expect("system Actors exists");
    assert_eq!(
      system.actor_class,
      ActorClass::System {
        sovereign_id: system_id,
      }
    );
    assert_eq!(OwnerSlotBitmaps::<Test>::get(ALICE)[0], 0b0000_0001);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorCreated {
          actor_id,
          owner,
          actor_class,
          initial_lifecycle: InitialLifecycle::Active,
          ..
        } if *actor_id == system_id
          && *owner == ALICE
          && matches!(actor_class, ActorClass::System { .. })
      )
    }));
  });
}

#[test]
fn system_sovereign_reattachment_creates_fresh_identity_with_same_custody() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let first_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let original_sovereign = sovereign_account(first_id);
    let _ = Balances::deposit_creating(&original_sovereign, 777);
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), first_id));
    assert_eq!(
      Actors::system_sovereigns(first_id),
      Some(SystemSovereignState::Vacant)
    );

    let fresh_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor_at_sovereign_id(
      RuntimeOrigin::root(),
      first_id,
      ALICE,
      Mutability::Mutable,
      system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
    ));
    let fresh = Actors::active_actor_view(fresh_id).expect("fresh System identity exists");
    assert_ne!(fresh_id, first_id);
    assert_eq!(fresh.sovereign_account, original_sovereign);
    assert_eq!(
      fresh.actor_class,
      ActorClass::System {
        sovereign_id: first_id
      }
    );
    assert_eq!(fresh.cycle_nonce, 0);
    assert_eq!(native_balance(&original_sovereign), 777);
    assert_eq!(
      Actors::system_sovereigns(first_id),
      Some(SystemSovereignState::Occupied(fresh_id))
    );
  });
}

#[test]
fn system_sovereign_reattachment_accepts_dormant_contract_input() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let first_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let sovereign = sovereign_account(first_id);
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), first_id));
    let fresh_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor_at_sovereign_id(
      RuntimeOrigin::root(),
      first_id,
      ALICE,
      Mutability::Mutable,
      None,
    ));
    let identity = Actors::actor_identities(fresh_id).expect("dormant identity exists");
    assert_eq!(identity.sovereign_account, sovereign);
    assert_eq!(
      identity.actor_class,
      ActorClass::System {
        sovereign_id: first_id
      }
    );
  });
}

#[test]
fn system_sovereign_reattachment_requires_allocated_vacant_locator() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_noop!(
      Actors::create_system_actor_at_sovereign_id(
        RuntimeOrigin::root(),
        42,
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::SystemSovereignUnknown
    );
    let occupied_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_noop!(
      Actors::create_system_actor_at_sovereign_id(
        RuntimeOrigin::root(),
        occupied_id,
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::SystemSovereignOccupied
    );
  });
}

#[test]
fn fresh_system_creation_cannot_capture_a_vacant_locator() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // A vacant locator exists after close. A fresh (non-reattachment) creation always
    // derives its own unregistered next id, so it can never capture the vacant locator's
    // registry entry; reattachment remains the only path authorized to regain control.
    let first_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), first_id));
    assert_eq!(
      Actors::system_sovereigns(first_id),
      Some(SystemSovereignState::Vacant)
    );
    let next_before = Actors::next_actor_id();
    let fresh_id = create_system_with(
      BOB,
      manual_schedule(),
      None,
      transfer_contract_steps(CHARLIE, 1),
    );
    assert_eq!(fresh_id, next_before);
    assert_ne!(fresh_id, first_id);
    assert_eq!(
      Actors::system_sovereigns(first_id),
      Some(SystemSovereignState::Vacant),
      "fresh creation must not capture the vacant locator"
    );
    assert_eq!(
      Actors::system_sovereigns(fresh_id),
      Some(SystemSovereignState::Occupied(fresh_id))
    );
  });
}

#[test]
fn reserved_sovereign_account_rejects_creation_at_that_slot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // The host marks a derived sovereign account as reserved; creation at the slot whose
    // sovereign would alias it must fail closed with SovereignAccountCollision before any
    // identity or fee mutation.
    let slot = 4u8;
    let sovereign = Actors::sovereign_account_id(&ALICE, slot);
    set_reserved_sovereign_account(sovereign);
    let alice_before = native_balance(&ALICE);
    assert_noop!(
      Actors::create_user_actor_at_slot(
        RuntimeOrigin::signed(ALICE),
        slot,
        Mutability::Mutable,
        None,
      ),
      Error::<Test>::ReservedSovereignAccount
    );
    assert_eq!(native_balance(&ALICE), alice_before);
    assert_eq!(Actors::actor_identity_count(), 0);
    assert_eq!(
      Actors::owner_slot_bitmap(ALICE)[slot as usize / 8] & (1 << (slot % 8)),
      0
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
      transfer_contract_steps(BOB, 1),
    );
    let id1 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let slot0 = Actors::active_actor_view(id0)
      .expect("id0 exists")
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    let slot1 = Actors::active_actor_view(id1)
      .expect("id1 exists")
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    assert_eq!(slot0, 0);
    assert_eq!(slot1, 1);
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), id0));
    let id2 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let slot2 = Actors::active_actor_view(id2)
      .expect("id2 exists")
      .actor_class
      .owner_slot()
      .expect("User actor has an owner slot");
    assert_eq!(slot2, slot0);
    assert!(Actors::active_actor_view(id0).is_none());
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
      transfer_contract_steps(BOB, 1),
    );
    let first = Actors::active_actor_view(first_id).expect("first Actors exists");
    assert_eq!(first.actor_class.owner_slot(), Some(target_slot));
    assert_eq!(
      first.sovereign_account,
      Actors::sovereign_account_id(&ALICE, target_slot)
    );
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), first_id));
    let second_id = create_user_with_slot(
      ALICE,
      target_slot,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let second = Actors::active_actor_view(second_id).expect("second Actors exists");
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
      transfer_contract_steps(BOB, 1),
    );
    assert_noop!(
      Actors::create_user_actor_at_slot(
        RuntimeOrigin::signed(ALICE),
        target_slot,
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
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
      Actors::create_user_actor_at_slot(
        RuntimeOrigin::signed(ALICE),
        invalid_slot,
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::InvalidOwnerSlot
    );
  });
}

#[test]
fn owner_slot_bitmap_covers_byte_boundaries_highest_slot_and_reuse() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    for slot in [7, 8, 15, 16] {
      create_user_with_slot(
        ALICE,
        slot,
        Mutability::Mutable,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
    }
    let highest = create_user_with_slot(
      ALICE,
      254,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let bitmap = Actors::owner_slot_bitmap(ALICE);
    assert_eq!(bitmap[0], 0b1000_0000);
    assert_eq!(bitmap[1], 0b1000_0001);
    assert_eq!(bitmap[2], 0b0000_0001);
    assert_eq!(bitmap[31], 0b0100_0000);
    assert_eq!(
      Actors::active_actor_view(highest)
        .unwrap()
        .actor_class
        .owner_slot(),
      Some(254)
    );
    assert_noop!(
      Actors::create_user_actor_at_slot(
        RuntimeOrigin::signed(ALICE),
        255,
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::InvalidOwnerSlot
    );
    let sovereign = Actors::active_actor_view(highest)
      .unwrap()
      .sovereign_account;
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), highest));
    assert_eq!(Actors::owner_slot_bitmap(ALICE)[31], 0);
    let replacement = create_user_with_slot(
      ALICE,
      254,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_eq!(
      Actors::active_actor_view(replacement)
        .unwrap()
        .sovereign_account,
      sovereign
    );
  });
}

#[test]
fn close_actor_emits_owner_initiated_reason() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let active_before = Actors::active_actor_count();
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_eq!(Actors::active_actor_count(), active_before + 1);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(Actors::active_actor_count(), active_before);
    let events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(events.len(), 1, "pure close emits only its close boundary");
    assert!(matches!(
      events[0],
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::OwnerInitiated,
      } if id == actor_id
    ));
  });
}

#[test]
fn create_atomicity_checkpoint_failure_rolls_back_all_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_fail_create_checkpoint(true);
    let actor_id = Actors::next_actor_id();
    let active_before = Actors::active_actor_count();
    let expected_sovereign = Actors::sovereign_account_id(&ALICE, 0);
    let owner_before = native_balance(&ALICE);
    let sink_before = native_balance(&TestFeeSink::get());
    prefund_active_user_creation(ALICE, &transfer_contract_steps(BOB, 1));
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      DispatchError::Other("AtomicityCreateCheckpointFailed")
    );
    assert_eq!(Actors::next_actor_id(), actor_id);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(SovereignIndex::<Test>::get(&expected_sovereign), None);
    assert_eq!(OwnerSlotBitmaps::<Test>::get(ALICE), [0; 32]);
    assert_eq!(Actors::active_actor_count(), active_before);
    assert_eq!(ActorHot::<Test>::iter_keys().count() as u32, active_before);
    assert_eq!(native_balance(&ALICE), owner_before);
    assert_eq!(native_balance(&TestFeeSink::get()), sink_before);
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn creation_and_activation_before_cutoff_use_exact_next_block_wakeup() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    crate::NextQueueTicket::<Test>::put(u64::MAX);
    prefund_active_user_creation(ALICE, &transfer_contract_steps(BOB, 1));
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(timer_schedule(1), None, transfer_contract_steps(BOB, 1)),
    ));
    assert_eq!(scheduled_wakeup_block(0), Some(2));
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      user_active_contract(timer_schedule(1), None, transfer_contract_steps(BOB, 1)),
    ));
    assert_eq!(scheduled_wakeup_block(1), Some(2));
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      None,
    ));
    let actor_id = Actors::next_actor_id() - 1;
    crate::NextQueueTicket::<Test>::put(u64::MAX);
    prefund_user_sovereign(ALICE, 0, &transfer_contract_steps(BOB, 1));
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      user_active_contract(timer_schedule(1), None, transfer_contract_steps(BOB, 1))
        .expect("direct Actor Contract"),
    ));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(3));
  });
}

#[test]
fn address_ingress_reuses_existing_transaction_and_fails_closed_at_depth_limit() {
  fn run_nested(depth: u32, actor_id: ActorId) -> polkadot_sdk::sp_runtime::DispatchResult {
    if depth == 0 {
      return Actors::notify_address_event(actor_id, TestAsset::Native, 1, &ALICE);
    }
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      match run_nested(depth - 1, actor_id) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let available_depth =
      u32::from(polkadot_sdk::frame_support::storage::transactional::TRANSACTIONAL_LIMIT);

    assert_ok!(run_nested(available_depth - 2, actor_id));
    assert!(Actors::pending_signal(actor_id));
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();
    let available_depth =
      u32::from(polkadot_sdk::frame_support::storage::transactional::TRANSACTIONAL_LIMIT);

    assert!(run_nested(available_depth, actor_id).is_err());
    assert!(!Actors::pending_signal(actor_id));
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn authored_abort_preserves_earlier_provisional_task_commit() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
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
        on_error: StepErrorPolicy::AbortCycle,
      },
    ])
    .expect("two steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    run_idle(Weight::MAX);

    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("failed actor remains below cutoff")
        .unsuccessful_attempt_streak,
      1
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        result: CycleResult::Failed,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn post_attempt_placement_failure_rolls_back_provisional_tasks_and_events() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      timer_schedule(1),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 100);
    crate::NextQueueTicket::<Test>::put(u64::MAX);
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();
    let bob_before = native_balance(&BOB);

    run_idle(Weight::MAX);

    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("rolled-back actor remains")
        .cycle_nonce,
      0
    );
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn control_transition_reuses_existing_transaction_at_depth_limit() {
  fn run_nested(depth: u32, actor_id: ActorId) -> polkadot_sdk::sp_runtime::DispatchResult {
    if depth == 0 {
      return Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id);
    }
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      match run_nested(depth - 1, actor_id) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );

    assert_ok!(run_nested(
      u32::from(polkadot_sdk::frame_support::storage::transactional::TRANSACTIONAL_LIMIT) - 2,
      actor_id,
    ));
    assert!(Actors::pending_signal(actor_id));
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();

    assert_noop!(
      run_nested(
        u32::from(polkadot_sdk::frame_support::storage::transactional::TRANSACTIONAL_LIMIT) - 1,
        actor_id,
      ),
      Error::<Test>::SchedulerIndexExhausted
    );
    assert!(!Actors::pending_signal(actor_id));
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn continuation_control_placement_failures_roll_back_exactly() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    crate::NextQueueTicket::<Test>::put(u64::MAX);
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();

    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::root(),
        actor_id,
        FundingSourcePolicy::AnyVerifiedIngress,
      ),
      Error::<Test>::QueueTicketExhausted
    );
    assert!(Actors::continuation_state(actor_id).is_some());
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });

  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    crate::NextQueueTicket::<Test>::put(u64::MAX);
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();

    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::root(),
        actor_id,
        inert_contract_steps(),
        crate::CompletionPolicy::Persistent,
      ),
      Error::<Test>::QueueTicketExhausted
    );
    assert!(Actors::continuation_state(actor_id).is_some());
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });

  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    crate::NextQueueTicket::<Test>::put(u64::MAX);
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();

    assert_noop!(
      Actors::cancel_continuation(RuntimeOrigin::root(), actor_id),
      Error::<Test>::QueueTicketExhausted
    );
    assert!(Actors::continuation_state(actor_id).is_some());
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn control_placement_failures_roll_back_state_and_events() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    crate::NextQueueTicket::<Test>::put(u64::MAX);
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();

    assert_noop!(
      Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::QueueTicketExhausted
    );
    assert!(
      !Actors::actor_hot(actor_id)
        .expect("actor remains active")
        .pending_signal
    );
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      timer_schedule(1),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(Actors::wakeup_substrate_invalidate(actor_id).is_some());
    frame_system::Pallet::<Test>::set_block_number(2);
    crate::NextQueueTicket::<Test>::put(u64::MAX);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(3));
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      transfer_contract_steps(BOB, 1),
    );
    assert!(Actors::wakeup_substrate_invalidate(actor_id).is_some());
    crate::WakeupCursorLen::<Test>::insert(
      WakeupClock::Block,
      <<Test as crate::Config>::MaxActiveActors as Get<u32>>::get(),
    );
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();

    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::SchedulerIndexExhausted
    );
    assert!(
      !Actors::actor_hot(actor_id)
        .expect("actor remains active")
        .lifecycle
        .is_paused()
    );
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    crate::NextQueueTicket::<Test>::put(u64::MAX);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      timer_schedule(1),
      None
    ));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
  });
}

#[test]
fn creation_subscription_failures_roll_back_exactly() {
  for corrupt_topology in [false, true] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      if corrupt_topology {
        crate::ObservationSubscriptionSlotOwner::<Test>::insert(0, 999);
      } else {
        crate::NextObservationSubscriptionSlot::<Test>::put(
          <<Test as crate::Config>::MaxActiveActors as Get<u32>>::get(),
        );
      }
      let expected_error = if corrupt_topology {
        Error::<Test>::ObservationSubscriptionInvariant
      } else {
        Error::<Test>::ObservationSubscriptionCapacityExceeded
      };
      prefund_active_user_creation(ALICE, &transfer_contract_steps(BOB, 1));
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
      let events_before = System::events();

      assert_noop!(
        Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          Mutability::Mutable,
          user_active_contract(
            observation_schedule(vec![1]),
            None,
            transfer_contract_steps(BOB, 1),
          ),
        ),
        expected_error
      );
      assert_eq!(Actors::next_actor_id(), 0);
      assert_eq!(System::events(), events_before);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        root_before
      );
    });
  }
}

#[test]
fn creation_wakeup_failures_roll_back_exactly() {
  for saturate_queue in [false, true] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      if saturate_queue {
        seed_saturated_tombstone_queue();
      }
      crate::WakeupCursorLen::<Test>::insert(
        WakeupClock::Tick,
        <<Test as crate::Config>::MaxActiveActors as Get<u32>>::get(),
      );
      let schedule = timer_schedule(if saturate_queue { 1 } else { 10 });
      prefund_active_user_creation(ALICE, &transfer_contract_steps(BOB, 1));
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
      let events_before = System::events();

      assert_noop!(
        Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          Mutability::Mutable,
          user_active_contract(schedule, None, transfer_contract_steps(BOB, 1)),
        ),
        Error::<Test>::SchedulerIndexExhausted
      );
      assert_eq!(Actors::next_actor_id(), 0);
      assert_eq!(System::events(), events_before);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        root_before
      );
    });
  }
}

#[test]
fn deactivation_checkpoint_failure_rolls_back_state_and_event() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();
    set_fail_create_checkpoint(true);

    assert_noop!(
      Actors::deactivate_actor(RuntimeOrigin::signed(ALICE), actor_id),
      DispatchError::Other("AtomicityCreateCheckpointFailed")
    );
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert_eq!(Actors::active_actor_count(), 1);
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn activation_checkpoint_failure_rolls_back_state_and_event() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      None,
    ));
    let actor_id = Actors::next_actor_id() - 1;
    frame_system::Pallet::<Test>::set_block_number(2);
    let identity_before = ActorIdentities::<Test>::get(actor_id);
    fund_native_raw(
      &identity_before
        .as_ref()
        .expect("dormant identity exists")
        .sovereign_account,
      user_prefunding_requirement(&transfer_contract_steps(BOB, 1)),
    );
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    set_fail_create_checkpoint(true);

    assert_noop!(
      Actors::activate_actor(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1))
          .expect("direct Actor Contract"),
      ),
      DispatchError::Other("AtomicityCreateCheckpointFailed")
    );

    assert_eq!(ActorIdentities::<Test>::get(actor_id), identity_before);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(Actors::active_actor_count(), 0);
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn creation_fee_route_failure_rolls_back_actor_creation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let active_before = Actors::active_actor_count();
    let owner_before = native_balance(&ALICE);
    set_fail_fee_sink_transfer(true);
    prefund_active_user_creation(ALICE, &transfer_contract_steps(BOB, 1));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::InsufficientFee
    );
    assert_noop!(
      Actors::create_user_actor_at_slot(
        RuntimeOrigin::signed(CHARLIE),
        2,
        Mutability::Mutable,
        None,
      ),
      Error::<Test>::InsufficientFee
    );
    set_fail_fee_sink_transfer(false);
    assert_eq!(Actors::active_actor_count(), active_before);
    assert_eq!(Actors::actor_identity_count(), 0);
    assert_eq!(Actors::next_actor_id(), 0);
    assert_eq!(Actors::owner_slot_bitmap(ALICE), [0; 32]);
    assert_eq!(Actors::owner_slot_bitmap(CHARLIE), [0; 32]);
    assert_eq!(native_balance(&ALICE), owner_before);
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
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(schedule, None, transfer_contract_steps(BOB, 1)),
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
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(schedule, None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::InvalidTriggerConfiguration
    );
  });
}

#[test]
fn create_rejects_zero_cadence() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::Cadenced { every_ticks: 0 },
      cooldown_blocks: 0,
    };
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(schedule, None, transfer_contract_steps(BOB, 1)),
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
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(schedule, None, transfer_contract_steps(BOB, 1)),
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
    let contract_steps = contract_steps_with_step(make_step(Task::SplitTransfer {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(100),
      legs,
    }));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, contract_steps),
      ),
      Error::<Test>::InvalidSplitTransfer
    );
  });
}

#[test]
fn split_transfer_rejects_duplicate_recipients() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let legs = BoundedVec::try_from(vec![
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(50),
      },
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(50),
      },
    ])
    .expect("legs fit");
    let contract_steps = contract_steps_with_step(make_step(Task::SplitTransfer {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(100),
      legs,
    }));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, contract_steps),
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
    let contract_steps = contract_steps_with_step(make_step(Task::SplitTransfer {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(total),
      legs,
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 1_000);
    let actor = sovereign_account(actor_id);
    let actor_before = native_balance(&actor);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(native_balance(&CHARLIE), charlie_before.saturating_add(50));
    assert_eq!(native_balance(&actor), actor_before.saturating_sub(100));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::SplitTransferExecuted {
          actor_id: id,
          total: emitted_total,
          distributed,
          retained,
          legs: 2,
          effective_legs: 2,
          ..
        } if *id == actor_id
          && *emitted_total == total
          && *distributed == 100
          && *retained == 1
      )
    }));
  });
}

#[test]
fn split_transfer_rejects_five_ineligible_legs_atomically_then_retries() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_asset_minimum_balance(11);
    let asset = TestAsset::Local(11);
    let recipients = [ALICE, BOB, CHARLIE, 4, 5, 6, 7, 8];
    let legs = recipients
      .iter()
      .map(|to| SplitLeg {
        to: *to,
        share: Perbill::from_parts(125_000_000),
      })
      .collect::<Vec<_>>()
      .try_into()
      .expect("eight legs fit");
    let mut step = make_step(Task::SplitTransfer {
      asset,
      amount: AmountResolution::Fixed(80),
      legs,
    });
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 2 };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 1_000);
    let actor_before = asset_balance(&actor, asset);
    let recipient_balances = recipients.map(|recipient| asset_balance(&recipient, asset));

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(asset_balance(&actor, asset), actor_before);
    assert_eq!(
      recipients.map(|recipient| asset_balance(&recipient, asset)),
      recipient_balances
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepFailed { actor_id: id, error, .. }
        if *id == actor_id
          && *error == Error::<Test>::RecipientDepositUnavailable.into()
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::SplitTransferExecuted { actor_id: id, .. } if *id == actor_id
    )));
    let continuation = Actors::continuation_state(actor_id).expect("temporary rejection suspends");
    assert_eq!(continuation.cursor, 0);
    assert_eq!(continuation.unsuccessful_attempts_at_cursor, 1);

    for recipient in recipients {
      set_asset_balance(&recipient, asset, 1);
    }
    let retry_balances = recipients.map(|recipient| asset_balance(&recipient, asset));
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);

    assert_eq!(asset_balance(&actor, asset), actor_before - 80);
    for (recipient, before) in recipients.iter().zip(retry_balances) {
      assert_eq!(asset_balance(recipient, asset), before + 10);
    }
    assert!(Actors::continuation_state(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::SplitTransferExecuted {
        actor_id: id,
        total: 80,
        distributed: 80,
        retained: 0,
        legs: 8,
        effective_legs: 8,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn split_transfer_late_leg_failure_rolls_back_every_leg() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
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
    let contract_steps = contract_steps_with_step(make_step(Task::SplitTransfer {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(100),
      legs,
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 1_000);
    let actor = sovereign_account(actor_id);
    let actor_before = native_balance(&actor);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    set_fail_transfer_to(Some(CHARLIE));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    set_fail_transfer_to(None);
    assert_eq!(native_balance(&actor), actor_before);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert!(has_actor_event(|event| {
      matches!(event, Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id)
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::SplitTransferExecuted { actor_id: id, .. } if *id == actor_id)
    }));
  });
}

#[test]
fn all_zero_split_transfer_total_is_an_explicit_resolution_skip() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
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
    let contract_steps = contract_steps_with_step(make_step(Task::SplitTransfer {
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
      legs,
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    let actor_before = native_balance(&actor);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    // No silent zero-leg transfer: the all-zero total resolves as a skip with no balance read,
    // preflight, or SplitTransferExecuted event, and the cycle continues.
    assert_eq!(native_balance(&actor), actor_before);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert!(!has_actor_event(|event| {
      matches!(event, Event::SplitTransferExecuted { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepSkipped { actor_id: id, reason: StepSkippedReason::ResolutionSkipped, .. }
        if *id == actor_id
    )));
  });
}

#[test]
fn split_transfer_rounding_skips_zero_distribution_and_allows_one_effective_leg() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let make_plan = |amount, first_share, second_share| {
      contract_steps_with_step(make_step(Task::SplitTransfer {
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(amount),
        legs: BoundedVec::try_from(vec![
          SplitLeg {
            to: BOB,
            share: first_share,
          },
          SplitLeg {
            to: CHARLIE,
            share: second_share,
          },
        ])
        .expect("two legs fit"),
      }))
    };
    let skipped = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      make_plan(1, Perbill::from_percent(50), Perbill::from_percent(50)),
    );
    fund_native(skipped, 10);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      skipped
    ));
    run_idle(Weight::MAX);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::SplitTransferExecuted { actor_id, .. } if *actor_id == skipped
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepSkipped {
        actor_id,
        reason: StepSkippedReason::ResolutionSkipped,
        ..
      } if *actor_id == skipped
    )));

    let one_effective = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      make_plan(2, Perbill::from_percent(60), Perbill::from_percent(40)),
    );
    fund_native(one_effective, 10);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      one_effective,
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::SplitTransferExecuted {
        actor_id,
        distributed: 1,
        effective_legs: 1,
        ..
      } if *actor_id == one_effective
    )));
  });
}

#[test]
fn on_address_event_owner_filter_is_enforced() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule,
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &BOB
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_ok!(Actors::notify_address_event(
      actor_id,
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
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule,
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_ok!(Actors::notify_address_event(
      actor_id,
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
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule,
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::notify_address_event_without_source(
      actor_id,
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
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .pending_signal
    );
    run_idle(Weight::MAX);
    let inst = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(!inst.pending_signal);
    assert_eq!(inst.cycle_nonce, 1);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleStarted {
          actor_id: id,
          cycle_nonce: 1,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn manual_trigger_persists_across_pause_resume() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .pending_signal
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .pending_signal
    );
    run_idle(Weight::MAX);
    let inst = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(!inst.pending_signal);
    assert_eq!(inst.cycle_nonce, 1);
  });
}

#[test]
fn user_pause_resume_churn_is_limited_to_one_queue_mutation_per_block() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_eq!(Actors::queue_tail(), 1);
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(
      Actors::actor_hot(actor_id)
        .expect("paused actor")
        .queue_ticket
        .is_none()
    );
    assert_noop!(
      Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ControlMutationRateLimited
    );
    assert_eq!(
      Actors::queue_tail(),
      1,
      "rate-limited resume must not append"
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(Actors::queue_tail(), 2);
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ControlMutationRateLimited
    );
    assert_eq!(
      Actors::queue_tail(),
      2,
      "rate-limited pause must not create a tombstone"
    );
  });
}

#[test]
fn system_pause_resume_churn_is_limited_for_governance_and_owner_origins() {
  new_test_ext().execute_with(|| {
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_ok!(Actors::pause_actor(RuntimeOrigin::root(), actor_id));
    assert_noop!(
      Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ControlMutationRateLimited
    );
    assert_eq!(
      Actors::actor_identities(actor_id)
        .expect("paused System actor identity")
        .last_control_mutation_block,
      1
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ControlMutationRateLimited
    );
    assert_eq!(
      Actors::actor_identities(actor_id)
        .expect("active System actor identity")
        .last_control_mutation_block,
      2
    );
  });
}

#[test]
fn manual_trigger_survives_paused_queue_pop_and_resume() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    run_idle(Weight::MAX);
    let paused = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(paused.pending_signal);
    assert_eq!(paused.cycle_nonce, 0);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    run_idle(Weight::MAX);
    let resumed = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(!resumed.pending_signal);
    assert_eq!(resumed.cycle_nonce, 1);
  });
}

#[test]
fn checkpoint_a_s4_paused_head_uses_hot_only_admission() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    let scan = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1);
    let consume = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
      .max(<<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page());
    let hot = Actors::scheduler_actor_hot_probe_weight_upper();
    Actors::execute_cycle(scan.saturating_add(hot).saturating_add(consume));

    let paused = Actors::actor_hot(actor_id).expect("paused actor");
    assert!(paused.pending_signal);
    assert_eq!(Actors::actor_identities(actor_id).expect("identity").cycle_nonce, 0);
    assert!(paused.queue_ticket.is_none());
  });
}

#[test]
fn manual_trigger_rejects_address_and_observation_only_policies() {
  let observation_schedule = observation_schedule(vec![7]);
  for schedule in [
    on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
    observation_schedule,
  ] {
    new_test_ext().execute_with(move || {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        schedule,
        None,
        transfer_contract_steps(BOB, 10),
      );
      assert_noop!(
        Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id),
        Error::<Test>::ManualSourceDisabled
      );
      let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
      assert!(!instance.pending_signal);
      assert!(instance.queue_ticket.is_none());
    });
  }
}

#[test]
fn manual_trigger_waits_through_cooldown_without_second_signal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 5,
    };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule,
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 2_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      1
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .pending_signal
    );
    assert_eq!(scheduled_wakeup_block(actor_id), Some(6));
    frame_system::Pallet::<Test>::set_block_number(6);
    run_idle(Weight::MAX);
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(instance.cycle_nonce, 2);
    assert!(!instance.pending_signal);
  });
}

#[test]
fn manual_trigger_waits_for_schedule_window_without_second_signal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(ScheduleWindow {
        start: 10,
        end: 110,
      }),
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .pending_signal
    );
    assert_eq!(scheduled_wakeup_block(actor_id), Some(10));
    frame_system::Pallet::<Test>::set_block_number(10);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      1
    );
  });
}

#[test]
fn address_event_waits_through_cooldown_without_second_signal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::address_event(SourceFilter::Any, AssetFilter::Any),
      cooldown_blocks: 5,
    };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule,
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 2_000);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      1
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert!(Actors::actor_hot(actor_id).is_some_and(|hot| hot.pending_signal));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(6));
    frame_system::Pallet::<Test>::set_block_number(6);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      2
    );
    assert!(!Actors::actor_hot(actor_id).is_some_and(|hot| hot.pending_signal));
  });
}

#[test]
fn manual_trigger_is_preserved_on_weight_defer() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Actors::scheduler_admission_overhead().saturating_add(Weight::from_parts(10, 0)));
    let inst = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(inst.pending_signal);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. } | Event::CycleSummary { actor_id: id, .. }
        if *id == actor_id
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
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(task)),
    );
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    let queue_weight = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1)
      .saturating_add(Actors::scheduler_actor_probe_weight_upper())
      .saturating_add(
        <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
          .max(<<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page()),
      );
    let proof_limit = queue_weight
      .proof_size()
      .saturating_add(Actors::attempt_weight_upper_bound(&instance, 0).proof_size())
      .saturating_sub(1);
    Actors::execute_cycle(Weight::from_parts(u64::MAX, proof_limit));
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(instance.pending_signal);
    assert_eq!(instance.cycle_nonce, 0);
  });
}

#[test]
fn failed_pre_opening_weight_admission_preserves_latch_and_funding() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE,
    ));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    let queue_weight = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1)
      .saturating_add(Actors::scheduler_actor_probe_weight_upper())
      .saturating_add(
        <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
          .max(<<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page()),
      );
    // Only the RefTime dimension is exhausted; ProofSize remains unlimited.
    let ref_time_limit = queue_weight
      .ref_time()
      .saturating_add(Actors::attempt_weight_upper_bound(&instance, 0).ref_time())
      .saturating_sub(1);
    Actors::execute_cycle(Weight::from_parts(ref_time_limit, u64::MAX));
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(instance.pending_signal);
    assert_eq!(instance.cycle_nonce, 0);
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&100),
    );
  });
}

#[test]
fn weight_deferral_is_silent_when_both_dimensions_are_exhausted() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    let queue_weight = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1)
      .saturating_add(Actors::scheduler_actor_probe_weight_upper())
      .saturating_add(
        <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
          .max(<<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page()),
      );
    // Both dimensions are exhausted one unit below the full cycle envelope.
    let limit = Weight::from_parts(
      queue_weight
        .ref_time()
        .saturating_add(Actors::attempt_weight_upper_bound(&instance, 0).ref_time())
        .saturating_sub(1),
      queue_weight
        .proof_size()
        .saturating_add(Actors::attempt_weight_upper_bound(&instance, 0).proof_size())
        .saturating_sub(1),
    );
    Actors::execute_cycle(limit);
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(instance.pending_signal);
    assert_eq!(instance.cycle_nonce, 0);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. } | Event::CycleSummary { actor_id: id, .. }
        if *id == actor_id
    )));
  });
}

#[test]
fn queued_actor_is_preserved_when_proof_budget_cannot_admit_probe() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let scan_weight =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(
        1,
      );
    Actors::execute_cycle(Weight::from_parts(
      u64::MAX,
      scan_weight
        .proof_size()
        .saturating_add(Actors::scheduler_actor_probe_weight_upper().proof_size())
        .saturating_sub(1),
    ));
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(instance.pending_signal);
    assert_eq!(instance.cycle_nonce, 0);
    assert!(
      Actors::actor_hot(actor_id)
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
    let prefunded = user_prefunding_requirement(&transfer_contract_steps(BOB, 10));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    deplete_user_sovereign(actor_id, prefunded);
    fund_native(actor_id, TestMinUserBalance::get());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::FeeBudgetExhausted,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn balance_exhausted_takes_precedence_over_fee_budget_exhausted() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let prefunded = user_prefunding_requirement(&transfer_contract_steps(BOB, 10));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    deplete_user_sovereign(actor_id, prefunded);
    fund_native(actor_id, TestMinUserBalance::get() - 1);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::BalanceExhausted,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn fee_insufficiency_is_terminal_without_deferral_guard() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let prefunded = user_prefunding_requirement(&transfer_contract_steps(BOB, 10));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    deplete_user_sovereign(actor_id, prefunded);
    fund_native(actor_id, TestMinUserBalance::get());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
  });
}

#[test]
fn condition_skip_fee_route_failure_aborts_before_skip_event() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: Balance::MAX,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(step),
    );
    fund_native(actor_id, 1_000_000_000);
    let bob_before = native_balance(&BOB);
    let fee_sink_before = native_balance(&TestFeeSink::get());
    clear_fee_collections();
    set_fail_fee_sink_transfer(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    set_fail_fee_sink_transfer(false);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&TestFeeSink::get()), fee_sink_before);
    assert_eq!(fee_collections(), vec![Actors::compute_eval_fee(1)]);
    assert!(!has_actor_event(|event| {
      matches!(event, Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id)
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::StepSkipped { actor_id: id, step_index: 0, .. } if *id == actor_id)
    }));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor remains")
        .cycle_nonce,
      0
    );
  });
}

#[test]
fn combined_step_fee_route_failure_aborts_before_task_execution() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let task = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(task.clone())),
    );
    fund_native(actor_id, 1_000_000_000);
    let bob_before = native_balance(&BOB);
    let fee_sink_before = native_balance(&TestFeeSink::get());
    clear_fee_collections();
    set_fail_fee_sink_transfer(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    set_fail_fee_sink_transfer(false);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&TestFeeSink::get()), fee_sink_before);
    let expected = Actors::compute_eval_fee(0).saturating_add(TestWeightToFee::weight_to_fee(
      &Actors::weight_upper_bound(&task),
    ));
    assert_eq!(fee_collections(), vec![expected]);
    assert!(!has_actor_event(|event| {
      matches!(event, Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id)
    }));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor remains")
        .cycle_nonce,
      0
    );
  });
}

#[test]
fn completed_failed_and_suspended_attempts_update_failure_streak_once() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    ActorHot::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("active actor")
        .unsuccessful_attempt_streak = 1;
    });
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("completed actor remains")
        .unsuccessful_attempt_streak,
      0
    );
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let plan = contract_steps_with_step(StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::AbortCycle,
    });
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("failed actor remains below cutoff")
        .unsuccessful_attempt_streak,
      1
    );
  });

  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("suspended actor remains")
        .unsuccessful_attempt_streak,
      1
    );
  });
}

#[test]
fn unsuccessful_attempt_streak_closes_actor_at_inclusive_threshold() {
  new_test_ext().execute_with(|| {
    let threshold = <Test as crate::Config>::MaxConsecutiveFailures::get();
    frame_system::Pallet::<Test>::set_block_number(1);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(failing_step),
    );
    fund_native(actor_id, 100);
    for cycle in 1..=threshold {
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      run_idle(Weight::MAX);
      if cycle < threshold {
        let inst = Actors::active_actor_view(actor_id).expect("actor remains before threshold");
        assert_eq!(inst.unsuccessful_attempt_streak, cycle);
        frame_system::Pallet::<Test>::set_block_number((cycle + 1) as u64);
      }
    }
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::ConsecutiveFailures,
        } if *id == actor_id
      )
    }));
    let events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    let cycle_summary = events
      .iter()
      .position(|event| {
        matches!(
          event,
          Event::CycleSummary {
            actor_id: id,
            cycle_nonce,
            ..
          } if *id == actor_id && *cycle_nonce == u64::from(threshold)
        )
      })
      .expect("terminal cycle summary exists");
    let closed = events
      .iter()
      .position(|event| {
        matches!(
          event,
          Event::ActorClosed {
            actor_id: id,
            reason: CloseReason::ConsecutiveFailures,
          } if *id == actor_id
        )
      })
      .expect("terminal close exists");
    assert!(
      cycle_summary < closed,
      "the admitted cycle must summarize before the terminal event"
    );
  });
}

#[test]
fn system_immutable_actor_closes_internally_at_failure_threshold_without_tasks() {
  new_test_ext().execute_with(|| {
    let threshold = <Test as crate::Config>::MaxConsecutiveFailures::get();
    frame_system::Pallet::<Test>::set_block_number(1);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Immutable,
      system_active_contract(
        timer_schedule(1),
        None,
        contract_steps_with_step(failing_step)
      ),
    ));
    assert_noop!(
      Actors::close_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ImmutableActor
    );
    fund_native(actor_id, 100);
    let charlie_before = native_balance(&CHARLIE);
    for cycle in 1..=threshold {
      frame_system::Pallet::<Test>::set_block_number(u64::from(cycle) + 1);
      run_idle(Weight::MAX);
      if cycle < threshold {
        assert_eq!(
          Actors::active_actor_view(actor_id)
            .expect("actor remains before threshold")
            .unsuccessful_attempt_streak,
          cycle
        );
      }
    }
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(
      native_balance(&CHARLIE),
      charlie_before,
      "terminal cleanup must not execute hidden transfer work"
    );
    assert_eq!(
      Actors::system_sovereigns(actor_id),
      Some(SystemSovereignState::Vacant)
    );
    let fresh_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor_at_sovereign_id(
      RuntimeOrigin::root(),
      actor_id,
      ALICE,
      Mutability::Mutable,
      system_active_contract(timer_schedule(1), None, transfer_contract_steps(BOB, 1)),
    ));
    assert!(Actors::active_actor_view(fresh_id).is_some());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::ConsecutiveFailures,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn global_fifo_eventually_services_system_actor_after_many_users() {
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
        inert_contract_steps(),
      );
      fund_native(user_id, 1_000);
    }
    let system_id = create_system_with(ALICE, timer_schedule(1), None, inert_contract_steps());
    // With MaxExecutionsPerBlock=3 and mixed User/System contention,
    // run enough blocks for the bounded queue to service the System actor.
    for block in 2..=20 {
      frame_system::Pallet::<Test>::set_block_number(block);
      run_idle(Weight::MAX);
    }
    let system = Actors::active_actor_view(system_id).expect("system Actors exists");
    assert!(
      system.cycle_nonce >= 1,
      "system actor must execute at least once over 20 blocks (nonce={})",
      system.cycle_nonce,
    );
  });
}

#[test]
fn global_fifo_services_system_actor_when_it_is_the_only_ready_work() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let system_id = create_system_with(ALICE, timer_schedule(1), None, inert_contract_steps());
    for block in 2..=4 {
      frame_system::Pallet::<Test>::set_block_number(block);
      run_idle(Weight::MAX);
    }
    let system = Actors::active_actor_view(system_id).expect("system Actors exists");
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
      let id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      ids.push(id);
      assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
    }
    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let started_block_2 = frame_system::Pallet::<Test>::events()
      .iter()
      .filter(|record| {
        matches!(
          record.event,
          RuntimeEvent::Actors(Event::CycleStarted { .. })
        )
      })
      .count() as u32;
    assert_eq!(started_block_2, max_exec);
    frame_system::Pallet::<Test>::set_block_number(3);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let started_block_3 = frame_system::Pallet::<Test>::events()
      .iter()
      .filter(|record| {
        matches!(
          record.event,
          RuntimeEvent::Actors(Event::CycleStarted { .. })
        )
      })
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
      let id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      ids.push(id);
    }
    for actor_id in ids {
      assert!(Actors::wakeup_substrate_schedule(actor_id, 1));
    }
    run_idle(Weight::MAX);
    let remaining = Actors::wakeup_buckets(1)
      .map(|bucket| bucket.live_entries)
      .unwrap_or(0);
    assert_eq!(remaining, total - max_wakeups);
  });
}

#[test]
fn wakeup_worker_stops_at_its_own_weight_envelope_without_lending() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::wakeup_substrate_schedule(actor_id, 1));
    // The shared on_idle meter has far more than the wakeup worker's dedicated envelope; the
    // worker must stop at its own ceiling and leave the surplus for actor service (spec 8.4.5).
    let cursor_probe = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_wakeup_cursor_worker_future();
    let worker_unit = Actors::wakeup_cursor_drain_unit_weight_upper(true);
    let worker_envelope = cursor_probe.saturating_add(worker_unit);
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(
      worker_envelope.saturating_sub(Weight::from_parts(1, 0)),
    );
    let stats = Actors::drain_overdue_wakeups_cursor(1, &mut meter);
    assert_eq!(stats.entries_scanned, 0, "worker cannot afford one complete unit");
    assert_eq!(stats.ready_entries, 0);
    assert_eq!(
      Actors::wakeup_buckets(1).expect("preserved bucket").live_entries,
      1
    );
    // The probe charge stays below one complete unit and never reaches the actor envelope.
    assert!(meter.consumed().all_lt(worker_envelope));
    // A full envelope admits the unit.
    let mut full_meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(worker_envelope);
    let full_stats = Actors::drain_overdue_wakeups_cursor(1, &mut full_meter);
    assert_eq!(full_stats.ready_entries, 1);
    assert!(Actors::wakeup_buckets(1).is_none());
  });
}

#[test]
fn wakeup_drain_preserves_bucket_when_proof_budget_cannot_admit_it() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::wakeup_substrate_schedule(actor_id, 1));
    run_idle(Weight::from_parts(u64::MAX, 300));
    assert_eq!(
      Actors::wakeup_buckets(1)
        .expect("preserved bucket")
        .live_entries,
      1
    );
    assert_eq!(scheduled_wakeup_block(actor_id), Some(1));
    assert_eq!(Actors::wakeup_cursor_peek(), Some(1));
  });
}

#[test]
fn wakeup_drain_stops_at_the_sparse_future_minimum() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(10_000);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::wakeup_substrate_schedule(actor_id, 1_000_000));
    run_idle(Weight::MAX);
    assert_eq!(Actors::wakeup_cursor_peek(), Some(1_000_000));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(1_000_000));
  });
}

#[test]
fn paged_enqueue_coalesces_without_a_per_block_insertion_cap() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let total = <<Test as crate::Config>::QueuePageSize as Get<u32>>::get() + 7;
    for _ in 0..total {
      let id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
    }
    assert_eq!(
      Actors::queue_tail().saturating_sub(Actors::queue_head()),
      u64::from(total)
    );
    assert_eq!(Actors::wakeup_cursor_len(), 0);
  });
}

#[test]
fn tombstone_drain_rolls_back_on_occupancy_underflow() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::paged_enqueue(actor_id));
    assert_eq!(Actors::paged_invalidate(actor_id), Some(0));
    QueueOccupancy::<Test>::put(0);
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    assert_eq!(
      Actors::paged_drain_tombstones(Actors::queue_tail(), 1),
      Err(crate::EnqueueOutcome::CorruptedTopology),
    );

    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });
}

#[test]
fn tombstone_drain_rolls_back_on_cross_page_corruption() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size = <Test as crate::Config>::QueuePageSize::get();
    for _ in 0..=page_size {
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert!(Actors::paged_enqueue(actor_id));
      assert!(Actors::paged_invalidate(actor_id).is_some());
    }
    QueuePages::<Test>::remove(1);
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    assert_eq!(
      Actors::paged_drain_tombstones(Actors::queue_tail(), page_size + 1),
      Err(crate::EnqueueOutcome::CorruptedTopology),
    );

    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
    assert!(
      Actors::queue_pages(0).is_some(),
      "first page deletion rolls back"
    );
  });
}

#[test]
fn tombstone_drain_missing_current_page_is_blocked_without_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::paged_enqueue(actor_id));
    assert!(Actors::paged_invalidate(actor_id).is_some());
    QueuePages::<Test>::remove(0);
    let cutoff = Actors::queue_tail();
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    let (kind, entry, scanned) = Actors::test_head_discovery(cutoff, 1, 0, Weight::MAX);

    assert_eq!((kind, entry, scanned), (3, None, 0));
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });
}

#[test]
fn enqueue_rolls_back_on_span_occupancy_mismatch() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    QueueTail::<Test>::put(1);
    QueueOccupancy::<Test>::put(0);
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    assert_eq!(
      Actors::try_paged_enqueue(actor_id),
      Err(crate::EnqueueOutcome::CorruptedTopology)
    );

    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn enqueue_rolls_back_on_missing_or_malformed_tail_page() {
  for malformed in [false, true] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let mut actors = Vec::new();
      for _ in 0..33 {
        let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
        assert!(Actors::paged_enqueue(actor_id));
        actors.push(actor_id);
      }
      let candidate = create_system_with(BOB, manual_schedule(), None, inert_contract_steps());
      if malformed {
        QueuePages::<Test>::mutate(1, |maybe_page| {
          maybe_page
            .as_mut()
            .expect("tail page")
            .try_push(QueueEntry {
              ticket: 99_999,
              actor_id: candidate,
            })
            .expect("malformed extra slot fits");
        });
      } else {
        QueuePages::<Test>::remove(1);
      }
      let events_before = System::events();
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

      assert_eq!(
        Actors::try_paged_enqueue(candidate),
        Err(crate::EnqueueOutcome::CorruptedTopology)
      );

      assert_eq!(System::events(), events_before);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        root_before
      );
      assert_eq!(actors.len(), 33);
    });
  }
}

#[test]
fn live_head_consume_rolls_back_on_occupancy_or_span_corruption() {
  for (tail, occupancy) in [(1u64, 0u32), (2u64, 1u32)] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert!(Actors::paged_enqueue(actor_id));
      QueueTail::<Test>::put(tail);
      QueueOccupancy::<Test>::put(occupancy);
      let events_before = System::events();
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

      assert_eq!(
        Actors::paged_consume_head_at(0),
        Err(crate::EnqueueOutcome::CorruptedTopology)
      );

      assert_eq!(System::events(), events_before);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        root_before
      );
      assert_eq!(
        Actors::actor_hot(actor_id).expect("hot").queue_ticket,
        Some(0)
      );
    });
  }
}

#[test]
fn live_head_consume_rolls_back_on_missing_or_malformed_head_page() {
  for malformed in [false, true] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert!(Actors::paged_enqueue(actor_id));
      if malformed {
        QueuePages::<Test>::insert(0, BoundedVec::default());
      } else {
        QueuePages::<Test>::remove(0);
      }
      let events_before = System::events();
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

      assert_eq!(
        Actors::paged_consume_head_at(0),
        Err(crate::EnqueueOutcome::CorruptedTopology)
      );

      assert_eq!(System::events(), events_before);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        root_before
      );
      assert_eq!(
        Actors::actor_hot(actor_id).expect("hot").queue_ticket,
        Some(0)
      );
    });
  }
}

#[test]
fn physical_occupancy_counts_tombstones_until_drained() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut actors = Vec::new();
    for _ in 0..5 {
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert!(Actors::paged_enqueue(actor_id));
      actors.push(actor_id);
    }
    assert_eq!(Actors::combined_queue_occupancy(), 5);
    // Invalidate the first four: their physical entries remain as tombstones and still count as
    // occupied capacity until drained, so tail gaps never weaken the namespace bound.
    for actor_id in &actors[0..4] {
      assert!(Actors::paged_invalidate(*actor_id).is_some());
    }
    assert_eq!(
      Actors::combined_queue_occupancy(),
      5,
      "tombstones stay inside exact physical occupancy"
    );
    let cutoff = Actors::next_queue_ticket();
    let drained = Actors::paged_drain_tombstones(cutoff, 10).expect("valid queue topology");
    assert_eq!(drained.tombstones_skipped, 4);
    assert_eq!(
      Actors::combined_queue_occupancy(),
      1,
      "draining tombstones releases exactly their occupancy"
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn paged_queue_uses_one_live_actor_ticket_and_lazy_invalidation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());

    assert!(Actors::paged_enqueue(actor_id));
    assert!(Actors::paged_enqueue(actor_id));
    assert_eq!(Actors::queue_head(), 0);
    assert_eq!(Actors::queue_tail(), 1);
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot state").queue_ticket,
      Some(0)
    );
    assert_eq!(Actors::queue_pages(0).expect("head page").len(), 1);

    assert_eq!(Actors::paged_invalidate(actor_id), Some(0));
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot state").queue_ticket,
      None
    );
    assert_eq!(
      Actors::paged_head_entry(),
      Some((
        0,
        QueueEntry {
          ticket: 0,
          actor_id
        }
      ))
    );
    let drained = Actors::paged_drain_tombstones(Actors::next_queue_ticket(), 1)
      .expect("invalidated head drains as a tombstone");
    assert_eq!(drained.tombstones_skipped, 1);
    assert_eq!(Actors::queue_head(), 32);
    assert_eq!(Actors::queue_tail(), 32);
    assert!(Actors::queue_pages(0).is_none());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn canonical_queue_try_state_rejects_an_internal_physical_gap() {
  new_test_ext().execute_with(|| {
    QueuePages::<Test>::insert(
      0,
      BoundedVec::try_from(vec![QueueEntry {
        ticket: 0,
        actor_id: 99_999,
      }])
      .expect("one queue entry fits"),
    );
    QueueHead::<Test>::put(0);
    QueueTail::<Test>::put(2);
    QueueOccupancy::<Test>::put(1);
    crate::NextQueueTicket::<Test>::put(2);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn paged_queue_crosses_and_reclaims_page_boundaries_without_prefix_rewrites() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut actors = Vec::new();
    for _ in 0..33 {
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert!(Actors::paged_enqueue(actor_id));
      actors.push(actor_id);
    }
    assert_eq!(Actors::queue_tail(), 33);
    assert_eq!(Actors::queue_pages(0).expect("full first page").len(), 32);
    assert_eq!(
      Actors::queue_pages(1).expect("partial second page").len(),
      1
    );

    for (ticket, actor_id) in actors.iter().take(32).copied().enumerate() {
      assert_eq!(
        Actors::paged_head_entry(),
        Some((
          ticket as u64,
          QueueEntry {
            ticket: ticket as u64,
            actor_id,
          },
        ))
      );
      assert!(Actors::paged_consume_head(ticket as u64));
    }
    assert_eq!(Actors::queue_head(), 32);
    assert!(Actors::queue_pages(0).is_none());
    assert_eq!(
      Actors::queue_pages(1).expect("remaining head page").len(),
      1
    );

    assert!(Actors::paged_consume_head(32));
    assert_eq!(Actors::queue_head(), 64);
    assert_eq!(Actors::queue_tail(), 64);
    assert!(Actors::queue_pages(1).is_none());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn paged_queue_replacement_ticket_leaves_old_entry_as_tombstone() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_a = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let actor_b = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::paged_enqueue(actor_a));
    assert_eq!(Actors::paged_invalidate(actor_a), Some(0));
    assert!(Actors::paged_enqueue(actor_b));
    assert!(Actors::paged_enqueue(actor_a));

    assert_eq!(
      Actors::actor_hot(actor_a)
        .expect("actor A hot")
        .queue_ticket,
      Some(2)
    );
    assert_eq!(
      Actors::actor_hot(actor_b)
        .expect("actor B hot")
        .queue_ticket,
      Some(1)
    );
    assert_eq!(
      Actors::paged_head_entry(),
      Some((
        0,
        QueueEntry {
          ticket: 0,
          actor_id: actor_a,
        },
      ))
    );
    let drained = Actors::paged_drain_tombstones(Actors::next_queue_ticket(), 1)
      .expect("replacement head drains as a tombstone");
    assert_eq!(drained.tombstones_skipped, 1);
    assert_eq!(
      Actors::actor_hot(actor_a)
        .expect("actor A hot")
        .queue_ticket,
      Some(2)
    );
    assert_eq!(
      Actors::paged_head_entry(),
      Some((
        1,
        QueueEntry {
          ticket: 1,
          actor_id: actor_b,
        },
      ))
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
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert!(Actors::paged_enqueue(actor_id));
      actors.push(actor_id);
    }
    for actor_id in actors {
      assert!(Actors::paged_invalidate(actor_id).is_some());
    }

    let cutoff = Actors::queue_tail();
    let first = Actors::paged_drain_tombstones(cutoff, 10).expect("valid first drain");
    assert_eq!(first.entries_scanned, 10);
    assert_eq!(first.tombstones_skipped, 10);
    assert_eq!(first.pages_touched, 1);
    assert_eq!(first.pages_deleted, 0);
    assert_eq!(Actors::queue_head(), 10);

    let rest = Actors::paged_drain_tombstones(cutoff, 55).expect("valid remaining drain");
    assert_eq!(rest.entries_scanned, 55);
    assert_eq!(rest.tombstones_skipped, 55);
    assert_eq!(rest.pages_touched, 3);
    assert_eq!(rest.pages_deleted, 3);
    assert_eq!(Actors::queue_head(), 96);
    assert_eq!(Actors::queue_tail(), 96);
    assert!(Actors::queue_pages(0).is_none());
    assert!(Actors::queue_pages(1).is_none());
    assert!(Actors::queue_pages(2).is_none());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn saturated_tombstone_queue_reclaims_head_before_ingress_and_recovers_deferred_work() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let page_size = <<Test as crate::Config>::QueuePageSize as Get<u32>>::get();
    let capacity = <<Test as crate::Config>::MaxQueueLength as Get<u32>>::get();
    for page_id in 0..capacity.div_ceil(page_size) {
      let first_ticket = page_id.saturating_mul(page_size);
      let len = page_size.min(capacity.saturating_sub(first_ticket));
      let entries = (0..len)
        .map(|offset| QueueEntry {
          ticket: u64::from(first_ticket).saturating_add(u64::from(offset)),
          actor_id: 10_000_000u64
            .saturating_add(u64::from(first_ticket))
            .saturating_add(u64::from(offset)),
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
    crate::pallet::NextQueueTicket::<Test>::put(u64::from(capacity));

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    let cleanup_budget =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_on_idle_base()
        .saturating_add(
        <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(
          1,
        ),
      );
    Actors::on_idle(1, cleanup_budget);
    assert_eq!(
      Actors::queue_head(),
      1,
      "saturated stale head must make progress before ingress"
    );
    assert_eq!(Actors::queue_tail(), u64::from(capacity));

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      false
    ));
    Actors::on_idle(2, Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("deferred actor survives")
        .cycle_nonce,
      1
    );
    assert_eq!(Actors::queue_head(), Actors::queue_tail());
    assert_eq!(scheduled_wakeup_block(actor_id), None);
  });
}

#[test]
fn queue_ticket_exhaustion_fails_closed_through_the_public_error() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    // Monotonic ticket namespace at the ceiling: the next enqueue must fail closed
    // with QueueTicketExhausted and roll back the producer movement.
    crate::NextQueueTicket::<Test>::put(u64::MAX);
    let actor_before = native_balance(&sovereign_account(actor_id));
    assert_noop!(
      Actors::notify_address_event(actor_id, TestAsset::Native, 100, &ALICE),
      Error::<Test>::QueueTicketExhausted
    );
    assert_eq!(native_balance(&sovereign_account(actor_id)), actor_before);
  });
}

#[test]
fn scheduler_index_exhaustion_fails_closed_when_tail_cannot_advance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    // Tail at the u64 ceiling: the next placement cannot advance the monotonic
    // index and must fail closed with SchedulerIndexExhausted. The ticket stays
    // well below its ceiling so the tail checked-add is the failing surface.
    crate::QueueTail::<Test>::put(u64::MAX);
    crate::NextQueueTicket::<Test>::put(5);
    QueueOccupancy::<Test>::put(0);

    let actor_before = native_balance(&sovereign_account(actor_id));
    assert_noop!(
      Actors::notify_address_event(actor_id, TestAsset::Native, 100, &ALICE),
      Error::<Test>::SchedulerIndexExhausted
    );
    assert_eq!(native_balance(&sovereign_account(actor_id)), actor_before);
  });
}

#[test]
fn system_locator_corruption_surfaces_one_invariant_error_on_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    // Corrupt the locator truth: the live actor's entry no longer points at it.
    crate::SystemSovereigns::<Test>::insert(actor_id, SystemSovereignState::Vacant);
    assert_noop!(
      Actors::close_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::SystemSovereignInvariant
    );
  });
}

#[test]
fn typed_ingress_preflight_and_notify_classify_exhaustion_as_permanent() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, 10),
    );
    let sovereign = sovereign_account(actor_id);
    let event = crate::AddressEvent {
      destination: sovereign,
      source: Some(ALICE),
      asset: TestAsset::Native,
      amount: 100,
      provenance: Some(crate::FundingProvenance::Signed),
    };
    // Preflight is read-only: it covers lifecycle and funding but performs no
    // placement, so monotonic namespace exhaustion cannot fail it.
    assert_ok!(Actors::preflight_ingress(&event));
    // Monotonic ticket namespace at the ceiling fails closed with a Permanent
    // classification and rolls back every Actors effect (spec 5.3, 6.2).
    crate::NextQueueTicket::<Test>::put(u64::MAX);
    let actor_before = native_balance(&sovereign);
    let failure = Actors::notify_ingress(&event).expect_err("ticket exhaustion must reject");
    assert_eq!(failure.retry, crate::RetryClass::Permanent);
    assert_eq!(
      failure.error,
      Error::<Test>::QueueTicketExhausted.into(),
      "monotonic ticket exhaustion maps to the public error"
    );
    assert_eq!(
      native_balance(&sovereign),
      actor_before,
      "failed certified movement rolls back with the value movement"
    );
    assert!(
      !Actors::actor_hot(actor_id)
        .expect("hot state")
        .pending_signal,
      "no signal latch survives a rejected certified movement"
    );
  });
}

#[test]
fn typed_ingress_notify_classifies_wakeup_capacity_as_temporary() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, 10),
    );
    let sovereign = sovereign_account(actor_id);
    let event = crate::AddressEvent {
      destination: sovereign,
      source: Some(ALICE),
      asset: TestAsset::Native,
      amount: 100,
      provenance: Some(crate::FundingProvenance::Signed),
    };
    // Saturate the FIFO so ticket placement falls back to an exact next-block
    // wakeup, then force that wakeup placement to fail with capacity exhaustion.
    seed_saturated_tombstone_queue();
    Actors::test_fail_wakeup_placement_with_capacity();
    let actor_before = native_balance(&sovereign);
    let failure = Actors::notify_ingress(&event).expect_err("wakeup capacity must reject");
    assert_eq!(
      failure.retry,
      crate::RetryClass::Temporary,
      "recoverable queue/wakeup capacity is Temporary"
    );
    assert_eq!(
      failure.error,
      Error::<Test>::QueueCapacityUnavailable.into(),
      "failed wakeup placement surfaces as queue capacity unavailability"
    );
    assert_eq!(native_balance(&sovereign), actor_before);
    let hot = Actors::actor_hot(actor_id).expect("hot state");
    assert!(hot.queue_ticket.is_none());
    assert!(hot.wakeup_pointer.is_none(), "no phantom wakeup on failure");
  });
}

#[test]
fn typed_ingress_zero_movement_creates_no_ingress() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, 10),
    );
    let sovereign = sovereign_account(actor_id);
    let event = crate::AddressEvent {
      destination: sovereign,
      source: Some(ALICE),
      asset: TestAsset::Native,
      amount: 0,
      provenance: Some(crate::FundingProvenance::Signed),
    };
    assert_ok!(Actors::notify_ingress(&event));
    let hot = Actors::actor_hot(actor_id).expect("hot state");
    assert!(
      !hot.pending_signal,
      "zero movement must not latch readiness (spec 5.3)"
    );
    assert!(hot.queue_ticket.is_none(), "zero movement must not enqueue");
    let funding = crate::ActorFunding::<Test>::get(actor_id).expect("funding state");
    assert!(
      funding.funding_accumulated.is_empty(),
      "zero movement must not accumulate funding"
    );
  });
}

#[test]
fn typed_ingress_absent_destination_is_balance_only() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let event = crate::AddressEvent {
      destination: BOB,
      source: Some(ALICE),
      asset: TestAsset::Native,
      amount: 100,
      provenance: Some(crate::FundingProvenance::Signed),
    };
    // A movement to a non-sovereign destination is balance-only: the typed
    // boundary accepts it without lifecycle, funding, trigger, or placement work.
    assert_ok!(Actors::preflight_ingress(&event));
    assert_ok!(Actors::notify_ingress(&event));
  });
}

#[test]
fn stale_close_entry_drains_as_tombstone_before_recreated_slot_runs() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // Create a User actor at slot 3, trigger it into the FIFO, then close it while queued.
    let first = create_user_with_slot(
      ALICE,
      3,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    fund_native(first, 1_000_000_000_000_000);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), first));
    assert!(
      Actors::actor_hot(first)
        .expect("queued actor")
        .queue_ticket
        .is_some(),
      "closed actor is physically queued"
    );
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), first));
    assert!(Actors::actor_hot(first).is_none(), "actor is closed");

    // Recreate at the same slot; the stale queue entry must not signal the fresh identity.
    let second = create_user_with_slot(
      ALICE,
      3,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    fund_native(second, 1_000_000_000_000_000);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), second));
    let second_ticket = Actors::actor_hot(second)
      .and_then(|hot| hot.queue_ticket)
      .expect("recreated actor has its own ticket");
    assert_ne!(
      second_ticket, 0,
      "fresh ticket must differ from the stale one"
    );

    // The stale head is a tombstone (actor closed, ticket cleared) and drains in physical order.
    let cutoff = Actors::next_queue_ticket();
    let drained = Actors::paged_drain_tombstones(cutoff, 10).expect("valid stale drain");
    assert_eq!(drained.tombstones_skipped, 1, "stale entry is a tombstone");
    assert_eq!(Actors::queue_head(), 1);

    // No CycleStarted for the recreated actor from the stale entry; the live head is the fresh
    // actor only after the fresh trigger.
    assert!(Actors::actor_hot(second).is_some_and(|hot| hot.pending_signal));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let started: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(Event::CycleStarted { actor_id, .. }) => Some(actor_id),
        _ => None,
      })
      .collect();
    assert_eq!(started, vec![second], "only the recreated actor executes");
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn paged_tombstone_drain_stops_at_live_head_and_honors_cutoff() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let stale = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let live = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let appended_after_cutoff =
      create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::paged_enqueue(stale));
    assert!(Actors::paged_enqueue(live));
    let cutoff = Actors::queue_tail();
    assert!(Actors::paged_enqueue(appended_after_cutoff));
    assert_eq!(Actors::paged_invalidate(stale), Some(0));
    assert_eq!(Actors::paged_invalidate(appended_after_cutoff), Some(2));

    let drained = Actors::paged_drain_tombstones(cutoff, 100).expect("valid cutoff drain");
    assert_eq!(drained.entries_scanned, 2);
    assert_eq!(drained.tombstones_skipped, 1);
    assert_eq!(drained.pages_touched, 1);
    assert_eq!(Actors::queue_head(), 1);
    assert_eq!(Actors::queue_tail(), 3);
    assert_eq!(
      Actors::actor_hot(live).expect("live actor").queue_ticket,
      Some(1)
    );

    assert!(Actors::paged_consume_head(1));
    let after_live = Actors::paged_drain_tombstones(cutoff, 100).expect("valid post-live drain");
    assert_eq!(
      after_live.entries_scanned, 0,
      "ticket 2 is beyond the captured cutoff"
    );
    assert_eq!(Actors::queue_head(), 2);
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
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      ids.push(actor_id);
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
    }
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(Actors::queue_tail().saturating_sub(Actors::queue_head()), 2);
    assert_eq!(
      Actors::paged_head_entry().map(|(_, entry)| entry.actor_id),
      Some(ids[max_exec as usize])
    );
    frame_system::Pallet::<Test>::set_block_number(3);
    run_idle(Weight::MAX);
    assert_eq!(Actors::queue_head(), Actors::queue_tail());
  });
}

#[test]
fn strict_head_of_line_heavy_head_deferral_preserves_follower_order() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // Heavy head first: three transfer steps make its cycle envelope strictly larger than the
    // single-step followers behind it.
    let step = |amount| {
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(amount),
      })
    };
    let heavy_plan = BoundedVec::try_from(vec![step(1), step(2), step(3)]).expect("plan fits");
    let head = create_system_with(ALICE, manual_schedule(), None, heavy_plan);
    fund_native(head, 10_000);
    let light_a = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    fund_native(light_a, 10_000);
    let light_b = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    fund_native(light_b, 10_000);
    for actor_id in [head, light_a, light_b] {
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
    }
    let tickets: Vec<_> = [head, light_a, light_b]
      .into_iter()
      .map(|id| {
        Actors::actor_hot(id)
          .and_then(|hot| hot.queue_ticket)
          .expect("triggered actor is queued")
      })
      .collect();
    assert_eq!(
      tickets,
      vec![0, 1, 2],
      "physical FIFO order is head, light A, light B"
    );

    // Constrained remainder: admits the head's probes and consume but not the head's full cycle
    // admission, while the lighter followers would fit. The pass must stop at the head.
    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(starvation_blocked_budget(head));

    let head_inst = Actors::active_actor_view(head).expect("head survives deferral");
    assert_eq!(
      head_inst.cycle_nonce, 0,
      "head attempt is deferred, not admitted"
    );
    assert_eq!(head_inst.pending_signal, true);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. } | Event::CycleSummary { actor_id: id, .. }
        if *id == head
    )));
    for (id, ticket) in [(light_a, 1), (light_b, 2)] {
      let inst = Actors::active_actor_view(id).expect("follower survives");
      assert_eq!(
        inst.cycle_nonce, 0,
        "follower never admitted behind the head"
      );
      assert!(
        Actors::actor_hot(id).is_some_and(|hot| hot.queue_ticket == Some(ticket)),
        "follower retains its exact physical ticket"
      );
    }
    assert!(
      !has_actor_event(|event| matches!(
        event,
        Event::CycleStarted { actor_id: id, .. } if *id == light_a || *id == light_b
      )),
      "no follower attempt starts behind an unadmitted head"
    );

    // Conforming full envelope: the head advances first, then followers in exact ticket order.
    frame_system::Pallet::<Test>::set_block_number(3);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let started: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(Event::CycleStarted { actor_id, .. }) => Some(actor_id),
        _ => None,
      })
      .collect();
    assert_eq!(started, vec![head, light_a, light_b]);
    for id in [head, light_a, light_b] {
      assert_eq!(
        Actors::active_actor_view(id)
          .expect("actor executed")
          .cycle_nonce,
        1
      );
    }
  });
}

#[test]
fn paged_wakeup_uses_the_exact_requested_block_without_spillover() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    seed_saturated_tombstone_queue();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    assert_eq!(
      Actors::wakeup_buckets(2)
        .expect("paged bucket")
        .live_entries,
      1
    );
    assert_eq!(Actors::wakeup_cursor_peek(), Some(2));
  });
}

#[test]
fn repeated_trigger_same_block_yields_one_ticket_and_one_execution() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000_000_000_000_000);
    // Two manual triggers in the same block latch one pending_signal and one FIFO ticket;
    // the post-worker cutoff enforces executions(A, B) <= 1.
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_eq!(
      Actors::actor_hot(actor_id)
        .and_then(|hot| hot.queue_ticket)
        .expect("one live ticket"),
      0
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let started = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter(|record| {
        matches!(
          record.event,
          RuntimeEvent::Actors(Event::CycleStarted { actor_id: id, .. }) if id == actor_id
        )
      })
      .count();
    assert_eq!(started, 1, "exactly one CycleStarted per actor per block");
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor")
        .cycle_nonce,
      1
    );
    assert_eq!(
      Actors::queue_head(),
      Actors::queue_tail(),
      "FIFO fully consumed"
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn defer_wakeup_deduplicates_repeated_manual_trigger_for_same_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    seed_saturated_tombstone_queue();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    assert_eq!(
      Actors::wakeup_buckets(2)
        .expect("deduplicated bucket")
        .live_entries,
      1
    );
    assert_eq!(Actors::wakeup_cursor_len(), 1);
  });
}

#[test]
fn schedule_anchor_is_the_canonical_initial_timer_anchor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(7);
    let actor_id = create_system_with(ALICE, timer_schedule(20), None, inert_contract_steps());
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(instance.schedule_anchor, 7);
    assert_eq!(instance.last_cycle_block, None);
    assert_eq!(scheduled_wakeup_block(actor_id), Some(27));
  });
}

#[test]
fn dormant_activation_anchors_first_eligibility_at_activation_time() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None,
    ));
    let actor_id = 0;
    frame_system::Pallet::<Test>::set_block_number(10);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      ActorContract {
        funding: FundingSourcePolicy::AnyVerifiedIngress,
        ..system_active_contract(timer_schedule(20), None, inert_contract_steps())
          .expect("direct Actor Contract")
      },
    ));
    let instance = Actors::active_actor_view(actor_id).expect("active Actors exists");
    // Activation anchors the fresh epoch at block 10; the first gate is one exact cadence later.
    assert_eq!(instance.schedule_anchor, 10);
    assert_eq!(instance.last_cycle_block, None);
    assert_eq!(scheduled_wakeup_block(actor_id), Some(30));
  });
}

#[test]
fn cadence_update_replaces_live_future_wakeup_instead_of_accumulating() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(20), None, inert_contract_steps());
    let initial_block = scheduled_wakeup_block(actor_id).expect("timer wakeup should be scheduled");
    assert_eq!(scheduled_wakeup_block(actor_id), Some(initial_block));
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      timer_schedule(5),
      None,
    ));
    let rescheduled_block = scheduled_wakeup_block(actor_id).expect("replacement wakeup");
    assert_ne!(rescheduled_block, initial_block);
    assert!(WakeupBuckets::<Test>::get(WakeupKey::Tick(initial_block)).is_none());
    assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick), 1);
  });
}

#[test]
fn cadence_update_rolls_back_exactly_when_existing_wakeup_cursor_is_corrupt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(20), None, inert_contract_steps());
    let initial_block = scheduled_wakeup_block(actor_id).expect("initial wakeup");
    WakeupBuckets::<Test>::mutate(WakeupKey::Tick(initial_block), |maybe_bucket| {
      maybe_bucket.as_mut().expect("bucket").cursor_index = None;
    });
    frame_system::Pallet::<Test>::set_block_number(2);
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    assert!(
      update_contract_partial!(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        timer_schedule(5),
        None,
      )
      .is_err()
    );

    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn ticket_and_terminal_window_wakeup_coexist_under_one_pointer() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // Manual actor inside a bounded schedule window: the Manual trigger queues it (live FIFO
    // ticket); updating the schedule to a still-future window then installs the terminal-only
    // expiry wakeup, which must coexist with the live ticket (SCHED-MEMBERSHIP).
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_contract_steps(),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ticket = Actors::actor_hot(actor_id)
      .and_then(|hot| hot.queue_ticket)
      .expect("manual trigger queues the actor");
    assert_eq!(ticket, 0);

    // Re-schedule the same window; the terminal-only expiry wakeup is installed while the actor
    // keeps its live FIFO ticket.
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
    ));
    let terminal_wakeup =
      Actors::actor_hot(actor_id).and_then(|hot| hot.wakeup_pointer.map(|pointer| pointer.block));
    assert!(
      terminal_wakeup.is_some(),
      "terminal-only window wakeup must coexist with the live ticket"
    );
    assert_eq!(
      Actors::actor_hot(actor_id).and_then(|hot| hot.queue_ticket),
      Some(ticket),
      "the live FIFO ticket survives the schedule update"
    );
    assert_eq!(Actors::wakeup_cursor_len(), 1);
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn temporal_membership_try_state_rejects_terminal_at_drift() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_contract_steps(),
    );
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot").terminal_at,
      Some(102)
    );
    #[cfg(feature = "try-runtime")]
    {
      assert_ok!(crate::Pallet::<Test>::do_try_state());
      // Terminal membership is derived from the schedule window: any `terminal_at` that is not
      // the exact window terminal (or absent without a window) must fail try_state.
      ActorHot::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("hot").terminal_at = Some(999);
      });
      assert_eq!(
        crate::Pallet::<Test>::do_try_state().map_err(|error| format!("{error:?}")),
        Err(
          "Other(\"ActorHot terminal_at disagrees with schedule window terminal membership\")"
            .into()
        )
      );
      ActorHot::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("hot").terminal_at = None;
      });
      assert!(crate::Pallet::<Test>::do_try_state().is_err());
      ActorHot::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("hot").terminal_at = Some(102);
      });
      assert_ok!(crate::Pallet::<Test>::do_try_state());
    }
  });
}

#[test]
fn temporal_membership_try_state_rejects_wakeup_pointer_beyond_terminal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_contract_steps(),
    );
    assert_eq!(scheduled_wakeup_block(actor_id), Some(102));
    #[cfg(feature = "try-runtime")]
    {
      assert_ok!(crate::Pallet::<Test>::do_try_state());
      // Drift the window and terminal membership together to a shorter terminal, leaving the
      // existing wakeup beyond it: the earlier-due service-point contract must fail try_state.
      ActorHot::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("hot").terminal_at = Some(50);
      });
      ActorContracts::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("contract").window = Some(ScheduleWindow { start: 1, end: 49 });
      });
      assert_eq!(
        crate::Pallet::<Test>::do_try_state().map_err(|error| format!("{error:?}")),
        Err("Other(\"ActorHot wakeup pointer exceeds its terminal membership\")".into())
      );
      ActorHot::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("hot").terminal_at = Some(102);
      });
      ActorContracts::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("contract").window = Some(ScheduleWindow { start: 1, end: 101 });
      });
      assert_ok!(crate::Pallet::<Test>::do_try_state());
    }
  });
}

#[test]
fn temporal_membership_try_state_rejects_page_slot_pointing_at_different_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let other = create_system_with(BOB, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::wakeup_substrate_schedule(actor_id, 10));
    assert!(Actors::wakeup_substrate_schedule(other, 10));
    // A physical slot whose entry addresses an actor that owns a different pointer is
    // corruption: `wakeup_pointer` is the sole ordinary temporal-membership authority.
    WakeupPages::<Test>::mutate((WakeupKey::Block(10), 0), |maybe| {
      let page = maybe.as_mut().expect("wakeup page");
      page.entries[0] = Some(crate::WakeupEntry { actor_id: other });
    });
    #[cfg(feature = "try-runtime")]
    assert_eq!(
      crate::Pallet::<Test>::do_try_state().map_err(|error| format!("{error:?}")),
      Err("Other(\"WakeupPage slot addresses an actor with a different wakeup pointer\")".into())
    );
  });
}

#[test]
fn temporal_membership_try_state_accepts_lazy_wakeup_tombstones() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(20), None, inert_contract_steps());
    let scheduled_block = scheduled_wakeup_block(actor_id).expect("timer wakeup scheduled");
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
    // Lazy terminal cleanup leaves a stale physical wakeup entry behind; the entry carries no
    // membership authority and must not fail try_state (spec 5.1 stale-entry semantics).
    assert!(WakeupBuckets::<Test>::contains_key(WakeupKey::Tick(
      scheduled_block
    )));
    assert!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick) > 0);
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    // The bounded drain converges the tombstone at its due tick.
    frame_system::Pallet::<Test>::set_block_number(scheduled_block);
    run_idle(Weight::MAX);
    assert!(!WakeupBuckets::<Test>::contains_key(WakeupKey::Tick(
      scheduled_block
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn terminal_membership_rolls_back_atomically_with_failed_schedule_replacement() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_contract_steps(),
    );
    // Corrupt the existing wakeup cursor so the replacement placement must roll back; the
    // terminal membership update and the whole control transaction revert together.
    WakeupBuckets::<Test>::mutate(WakeupKey::Block(102), |maybe| {
      maybe.as_mut().expect("bucket").cursor_index = None;
    });
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert!(
      update_contract_partial!(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        timer_schedule(5),
        Some(ScheduleWindow { start: 1, end: 201 }),
      )
      .is_err()
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot").terminal_at,
      Some(102),
      "terminal membership stays at the original window terminal"
    );
  });
}

#[test]
fn paged_wakeup_recovery_is_independent_of_sparse_actor_ids() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    seed_saturated_tombstone_queue();
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    let capacity = <Test as crate::Config>::MaxQueueLength::get();
    Actors::paged_drain_tombstones(Actors::next_queue_ticket(), capacity)
      .expect("saturated tombstones drain coherently");
    NextActorId::<Test>::put(10_000_000);
    let bob_before = native_balance(&BOB);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(10));
    assert!(!Actors::actor_hot(actor_id).is_some_and(|hot| hot.pending_signal));
  });
}

#[test]
fn close_before_future_wakeup_leaves_harmless_lazy_tombstone() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(20), None, inert_contract_steps());
    let scheduled_block =
      scheduled_wakeup_block(actor_id).expect("timer wakeup should be scheduled");
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(scheduled_wakeup_block(actor_id).is_none());
    assert!(WakeupBuckets::<Test>::contains_key(WakeupKey::Tick(
      scheduled_block
    )));
    assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick), 1);
    frame_system::Pallet::<Test>::set_block_number(scheduled_block);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(Actors::wakeup_buckets(scheduled_block).is_none());
    assert_eq!(Actors::queue_head(), Actors::queue_tail());
    assert!(!has_actor_event(|event| {
      matches!(event, Event::CycleStarted { actor_id: id, .. } if *id == actor_id)
    }));
  });
}

#[test]
fn repeated_timer_close_churn_converges_lazy_wakeup_tombstones() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let total = <<Test as crate::Config>::MaxWakeupsPerBlock as Get<u32>>::get() + 2;
    let mut actors = Vec::new();
    let mut latest_wakeup = 1u64;
    for _ in 0..total {
      let actor_id = create_system_with(ALICE, timer_schedule(4_000), None, inert_contract_steps());
      let wakeup = scheduled_wakeup_block(actor_id).expect("timer wakeup must be scheduled");
      latest_wakeup = latest_wakeup.max(wakeup);
      actors.push(actor_id);
    }

    for actor_id in actors {
      assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
      assert!(scheduled_wakeup_block(actor_id).is_none());
    }
    assert!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick) > 0);
    frame_system::Pallet::<Test>::reset_events();
    for offset in 0..10 {
      frame_system::Pallet::<Test>::set_block_number(
        latest_wakeup.saturating_add(1_000).saturating_add(offset),
      );
      run_idle(Weight::MAX);
      if crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick) == 0 {
        break;
      }
    }
    assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick), 0);
    assert_eq!(Actors::queue_head(), Actors::queue_tail());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { .. }
    )));
  });
}

#[test]
fn window_expiry_wakeup_closes_inactive_actor_without_identity_scan() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_contract_steps(),
    );
    assert_eq!(scheduled_wakeup_block(actor_id), Some(102));
    NextActorId::<Test>::put(10_000_000);
    frame_system::Pallet::<Test>::set_block_number(102);
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::WindowExpired,
      } if *id == actor_id
    )));
  });
}

#[test]
fn terminal_window_wakeup_survives_queue_saturation_and_continuation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    // Retryable swap inside a bounded window: the first attempt creates a Continuation whose
    // retry backoff would land far past the window end; the terminal expiry wakeup at end + 1
    // must win, and then close the actor even when the queue is fully saturated.
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      temporary_retry_swap_plan(),
    );
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    run_idle(Weight::MAX);
    assert!(
      Actors::continuation_state(actor_id).is_some(),
      "retryable step leaves a Continuation"
    );
    assert_eq!(
      scheduled_wakeup_block(actor_id),
      Some(102),
      "terminal expiry at end + 1 wins over the retry backoff"
    );
    // Saturate the physical queue coherently while preserving the Continuation's live ticket.
    let existing_ticket = Actors::actor_hot(actor_id)
      .and_then(|hot| hot.queue_ticket)
      .expect("Continuation retains its live queue ticket");
    let page_size: u32 = <Test as crate::Config>::QueuePageSize::get();
    let capacity: u32 = <Test as crate::Config>::MaxQueueLength::get();
    for page_id in 0..capacity.div_ceil(page_size) {
      let first = page_id.saturating_mul(page_size);
      let len = page_size.min(capacity.saturating_sub(first));
      let entries = (0..len)
        .map(|offset| {
          let ticket = u64::from(first.saturating_add(offset));
          QueueEntry {
            ticket,
            actor_id: if ticket == existing_ticket {
              actor_id
            } else {
              30_000_000u64.saturating_add(ticket)
            },
          }
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
    frame_system::Pallet::<Test>::set_block_number(102);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(
      Actors::active_actor_view(actor_id).is_none(),
      "expiry closes the actor despite saturation and Continuation; head={} tail={} occupancy={} wakeup={:?} queue_ticket={:?}",
      Actors::queue_head(),
      Actors::queue_tail(),
      Actors::combined_queue_occupancy(),
      scheduled_wakeup_block(actor_id),
      Actors::actor_hot(actor_id).and_then(|hot| hot.queue_ticket),
    );
    assert!(Actors::continuation_state(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::WindowExpired,
      } if *id == actor_id
    )));
  });
}

#[test]
fn paused_actor_retains_direct_window_expiry_wakeup() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_contract_steps(),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(102));
    frame_system::Pallet::<Test>::set_block_number(102);
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
  });
}

#[test]
fn absent_schedule_window_never_expires() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    frame_system::Pallet::<Test>::set_block_number(u64::MAX);
    let instance = Actors::active_actor_view(actor_id).expect("actor remains live");
    assert!(!Actors::is_window_expired(&instance));
  });
}

#[test]
fn expired_ingress_remains_balance_only_and_closes_inline() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let window = ScheduleWindow { start: 1, end: 101 };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      Some(window),
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    let actor = sovereign_account(actor_id);
    let balance_before = native_balance(&actor);
    frame_system::Pallet::<Test>::set_block_number(102);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      1_000
    ));
    assert_eq!(native_balance(&actor), balance_before.saturating_add(1_000));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(Actors::actor_funding(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::WindowExpired,
      } if *id == actor_id
    )));
  });
}

#[test]
fn window_expired_takes_precedence_over_balance_exhausted() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let window = ScheduleWindow { start: 1, end: 101 };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(window),
      transfer_contract_steps(BOB, 1),
    );
    frame_system::Pallet::<Test>::set_block_number(102);
    assert_ok!(Actors::permissionless_sweep(
      RuntimeOrigin::signed(ALICE),
      actor_id,
    ));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::WindowExpired,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn retry_later_is_mutable_only_at_creation_and_update() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut retry_plan = transfer_contract_steps(BOB, 1);
    retry_plan[0].on_error = RETRY_LATER;

    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Immutable,
        user_active_contract(manual_schedule(), None, retry_plan.clone()),
      ),
      Error::<Test>::RetryLaterNotAllowedForImmutableActor
    );
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Immutable,
        system_active_contract(manual_schedule(), None, retry_plan.clone()),
      ),
      Error::<Test>::RetryLaterNotAllowedForImmutableActor
    );

    prefund_active_user_creation(ALICE, &retry_plan);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, retry_plan.clone()),
    ));

    let immutable_id = create_user_with(
      BOB,
      Mutability::Immutable,
      manual_schedule(),
      None,
      transfer_contract_steps(ALICE, 1),
    );
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::signed(BOB),
        immutable_id,
        retry_plan,
        crate::CompletionPolicy::Persistent,
      ),
      Error::<Test>::RetryLaterNotAllowedForImmutableActor
    );
  });
}

#[test]
fn retry_later_enforces_the_protocol_fixed_attempt_range() {
  new_test_ext().execute_with(|| {
    for max_attempts in [0, 1, 11] {
      let mut plan = transfer_contract_steps(BOB, 1);
      plan[0].on_error = StepErrorPolicy::RetryLater { max_attempts };
      assert_noop!(
        Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          Mutability::Mutable,
          user_active_contract(manual_schedule(), None, plan),
        ),
        Error::<Test>::InvalidRetryAttemptLimit
      );
    }
    for max_attempts in [2, 10] {
      let mut valid_plan = transfer_contract_steps(BOB, 1);
      valid_plan[0].on_error = StepErrorPolicy::RetryLater { max_attempts };
      prefund_active_user_creation(ALICE, &valid_plan);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, valid_plan),
      ));
    }
  });
}

#[test]
fn retry_later_aborts_permanent_failure_without_executing_suffix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: RETRY_LATER,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    });
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![failing_step, succeeding_step]).expect("two steps fit"),
    );
    fund_native(actor_id, 100);
    let charlie_before = native_balance(&CHARLIE);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    let instance = Actors::active_actor_view(actor_id).expect("actor remains active");
    assert_eq!(instance.cycle_nonce, 1);
    assert_eq!(instance.unsuccessful_attempt_streak, 1);
    assert_eq!(instance.cycle_state, CycleState::Idle);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          outcomes: OutcomeTotals { executed_steps: 0, failed_steps: 1, .. },
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn retry_later_resumes_same_cursor_without_replaying_committed_prefix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_pool(TestAsset::Native, TestAsset::Local(77), 10_000, 10_000);
    set_asset_balance(&u64::MAX, TestAsset::Local(77), 10_000);
    let retry_step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(20),
        slippage_tolerance: Perbill::one(),
      },
      on_error: RETRY_LATER,
    };
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      }),
      retry_step,
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(5),
      }),
    ])
    .expect("three steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    let first = Actors::active_actor_view(actor_id).expect("suspended actor remains");
    let first_continuation = Actors::continuation_state(actor_id).expect("continuation exists");
    assert_eq!(first.cycle_state, CycleState::Suspended);
    assert_eq!(first.cycle_nonce, 1);
    assert_eq!(first.unsuccessful_attempt_streak, 1);
    assert_eq!(first_continuation.cursor, 1);
    assert_eq!(first_continuation.unsuccessful_attempts_at_cursor, 1);
    assert_eq!(first_continuation.cumulative_outcomes.executed_steps, 1);
    assert_eq!(first_continuation.cumulative_outcomes.failed_steps, 1);
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert_eq!(native_balance(&first.sovereign_account), 90);
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    assert!(!has_actor_event(|event| {
      matches!(event, Event::CycleSummary { actor_id: id, .. } if *id == actor_id)
    }));

    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    let second_continuation = Actors::continuation_state(actor_id).expect("continuation remains");
    assert_eq!(second_continuation.cursor, 1);
    assert_eq!(second_continuation.unsuccessful_attempts_at_cursor, 2);
    assert_eq!(second_continuation.cumulative_outcomes.executed_steps, 1);
    assert_eq!(second_continuation.cumulative_outcomes.failed_steps, 2);
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(native_balance(&CHARLIE), charlie_before);

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(4);
    run_idle(Weight::MAX);

    let completed = Actors::active_actor_view(actor_id).expect("completed actor remains");
    assert_eq!(completed.cycle_state, CycleState::Idle);
    assert_eq!(completed.cycle_nonce, 1);
    assert_eq!(completed.unsuccessful_attempt_streak, 0);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 5);
    let starts = frame_system::Pallet::<Test>::events()
      .iter()
      .filter(|record| {
        matches!(
          record.event,
          RuntimeEvent::Actors(Event::CycleStarted { actor_id: id, .. }) if id == actor_id
        )
      })
      .count();
    assert_eq!(starts, 1);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          cycle_nonce: 1,
          outcomes: OutcomeTotals { executed_steps: 3, failed_steps: 2, .. },
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn stop_cycle_commits_prefix_and_completes_before_unreachable_suffix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      }),
      StepOf::<Test> {
        precondition: None,
        task: Task::StopCycle,
        on_error: RETRY_LATER,
      },
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(5),
      }),
    ])
    .expect("three steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    let actor = Actors::active_actor_view(actor_id).expect("actor remains active");
    assert_eq!(actor.cycle_state, CycleState::Idle);
    assert_eq!(actor.cycle_nonce, 1);
    assert_eq!(actor.unsuccessful_attempt_streak, 0);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleStopped {
        actor_id: id,
        cycle_nonce: 1,
        step_index: 1,
      } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 2, failed_steps: 0, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn skipped_stop_cycle_advances_to_the_suffix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let stop = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 1_000,
      }]),
      task: Task::StopCycle,
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = BoundedVec::try_from(vec![
      stop,
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(5),
      }),
    ])
    .expect("two steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(native_balance(&BOB), bob_before + 5);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStopped { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 1, precondition_skips: 1, failed_steps: 0, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn close_after_productive_cycle_ignores_false_cycles_then_closes_immutable_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 100,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Immutable,
      system_active_contract_with_completion(
        timer_schedule(1),
        None,
        contract_steps_with_step(step),
        crate::CompletionPolicy::CloseAfterProductiveCycle,
      ),
    ));
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 50);
    let bob_before = native_balance(&BOB);

    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);

    let retained = Actors::active_actor_view(actor_id).expect("false cycle remains active");
    assert_eq!(retained.cycle_nonce, 1);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&actor), 50);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { committed_effectful_tasks: 0, .. },
        ..
      } if *id == actor_id
    )));

    fund_native(actor_id, 100);
    frame_system::Pallet::<Test>::set_block_number(3);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(native_balance(&actor), 140);
    let events: Vec<_> = System::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    let summary = events
      .iter()
      .position(|event| matches!(event, Event::CycleSummary { actor_id: id, outcomes: OutcomeTotals { committed_effectful_tasks: 1, .. }, .. } if *id == actor_id))
      .expect("productive summary");
    let closed = events
      .iter()
      .position(|event| matches!(event, Event::ActorClosed { actor_id: id, reason: CloseReason::ProductiveCycleCompleted } if *id == actor_id))
      .expect("productive close");
    assert!(summary < closed);
  });
}

#[test]
fn close_after_productive_cycle_rechecks_latest_observation_before_execution() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let feed = 1;
    set_observation(
      feed,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 1,
      },
    );
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::ObservationBelow {
        feed,
        threshold: 100,
        max_age_blocks: 10,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract_with_completion(
        observation_schedule(vec![feed]),
        None,
        contract_steps_with_step(step),
        crate::CompletionPolicy::CloseAfterProductiveCycle,
      ),
    ));
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::note_observation_changed(feed, 1));
    set_observation(
      feed,
      crate::ScalarObservationState::Fresh {
        value: 150,
        observed_at: 1,
      },
    );

    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_some());
    assert_eq!(native_balance(&BOB), bob_before);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { committed_effectful_tasks: 0, .. },
        ..
      } if *id == actor_id
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ProductiveCycleCompleted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn close_after_productive_cycle_does_not_treat_stop_cycle_as_effectful() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract_with_completion(
        manual_schedule(),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
        crate::CompletionPolicy::CloseAfterProductiveCycle,
      ),
    ));

    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_some());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 1, committed_effectful_tasks: 0, .. },
        ..
      } if *id == actor_id
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ProductiveCycleCompleted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn close_after_productive_cycle_waits_for_retry_completion() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract_with_completion(
        manual_schedule(),
        None,
        temporary_retry_swap_plan(),
        crate::CompletionPolicy::CloseAfterProductiveCycle,
      ),
    ));
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);

    assert!(Actors::continuation_state(actor_id).is_some());
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ProductiveCycleCompleted,
      } if *id == actor_id
    )));

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);

    assert!(Actors::continuation_state(actor_id).is_none());
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ProductiveCycleCompleted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn close_after_productive_cycle_keeps_retry_exhaustion_as_failure_terminal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let retry_step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(20),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 2 },
    };
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract_with_completion(
        manual_schedule(),
        None,
        contract_steps_with_step(retry_step),
        crate::CompletionPolicy::CloseAfterProductiveCycle,
      ),
    ));
    fund_native(actor_id, 100);
    let actor = sovereign_account(actor_id);
    let balance_before = native_balance(&actor);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);
    assert!(Actors::continuation_state(actor_id).is_some());
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(native_balance(&actor), balance_before);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::RetryAttemptsExhausted,
      } if *id == actor_id
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ProductiveCycleCompleted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn stop_cycle_runs_normal_auto_close_after_the_summary() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    assert_ok!(replace_auto_close(
      RuntimeOrigin::root(),
      actor_id,
      Some(1),
    ));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    frame_system::Pallet::<Test>::reset_events();

    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
    let events: Vec<_> = System::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    let stopped = events
      .iter()
      .position(|event| matches!(event, Event::CycleStopped { actor_id: id, .. } if *id == actor_id))
      .expect("stop event");
    let summary = events
      .iter()
      .position(|event| matches!(event, Event::CycleSummary { actor_id: id, .. } if *id == actor_id))
      .expect("summary event");
    let closed = events
      .iter()
      .position(|event| matches!(event, Event::ActorClosed { actor_id: id, reason: CloseReason::AutoCloseNonceReached } if *id == actor_id))
      .expect("auto-close event");
    assert!(stopped < summary && summary < closed);
  });
}

#[test]
fn stop_cycle_fee_failure_rolls_back_before_step_policy_or_stop() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    fund_native(actor_id, 100_000);
    let sink_before = native_balance(&TestFeeSink::get());
    set_fail_fee_sink_transfer(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStopped { actor_id: id, .. } if *id == actor_id
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleSummary { actor_id: id, .. } if *id == actor_id
    )));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor remains active")
        .unsuccessful_attempt_streak,
      0
    );
    assert_eq!(native_balance(&TestFeeSink::get()), sink_before);

    set_fail_fee_sink_transfer(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    let task: TaskOf<Test> = Task::StopCycle;
    let task_weight = Actors::weight_upper_bound(&task);
    let expected_fee = Actors::compute_eval_fee(0).saturating_add(
      <TestWeightToFee as polkadot_sdk::sp_weights::WeightToFee>::weight_to_fee(&task_weight),
    );
    assert_eq!(
      native_balance(&TestFeeSink::get()),
      sink_before.saturating_add(expected_fee)
    );
    assert_eq!(fee_collections().last(), Some(&expected_fee));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleStopped {
        actor_id: id,
        cycle_nonce: 1,
        step_index: 0,
      } if *id == actor_id
    )));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("successful actor remains active")
        .unsuccessful_attempt_streak,
      0
    );
  });
}

#[test]
fn matching_runtime_simulation_reports_stop_and_rolls_everything_back() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::StopCycle),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(5),
      }),
    ])
    .expect("two steps fit");
    let contract = system_active_contract(manual_schedule(), None, contract_steps.clone())
      .expect("direct Actor Contract");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let events_before = System::events();
    let actor_before = Actors::active_actor_view(actor_id).expect("actor exists");

    let result = Actors::simulate_current_contract(
      actor_id,
      ActorType::System,
      Mutability::Mutable,
      contract,
      SimulationMode::FreshCurrentPlan,
    )
    .expect("ready current plan simulates");

    assert_eq!(result.status, AttemptDisposition::Completed);
    assert_eq!(result.cumulative_outcomes.executed_steps, 1);
    assert_eq!(
      result.steps,
      vec![SimulationStepRecord {
        step_index: 0,
        outcome: StepOutcome::Stopped,
      }]
    );
    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(System::events(), events_before);
    assert!(Actors::continuation_state(actor_id).is_none());
  });
}

#[test]
fn simulation_and_scheduler_reject_the_same_protected_fee_floor_boundary() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = transfer_contract_steps(BOB, 10);
    let contract = user_active_contract(manual_schedule(), None, contract_steps.clone())
      .expect("direct Actor Contract");
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let instance = Actors::active_actor_view(actor_id).expect("User actor exists");
    let attempt_fee = Actors::attempt_fee_upper_bound(&instance, 0);
    let raw_balance = attempt_fee.max(TestMinUserBalance::get());
    fund_native(actor_id, raw_balance);
    assert!(
      raw_balance >= attempt_fee,
      "raw balance covers the attempt envelope"
    );
    assert!(
      raw_balance.saturating_sub(TestMinUserBalance::get()) < attempt_fee,
      "balance above the protected floor does not cover the attempt envelope",
    );
    let actor_before = Actors::active_actor_view(actor_id).expect("actor before simulation");
    let events_before = System::events();

    let result = Actors::simulate_current_contract(
      actor_id,
      ActorType::User,
      Mutability::Mutable,
      contract,
      SimulationMode::FreshCurrentPlan,
    )
    .expect("terminal viability projects as a closed simulation");
    assert_eq!(
      result.status,
      AttemptDisposition::Closed(CloseReason::FeeBudgetExhausted)
    );
    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(System::events(), events_before);

    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::FeeBudgetExhausted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn simulation_projects_fee_collection_failure_as_interface_error_and_rolls_back() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = transfer_contract_steps(BOB, 10);
    let contract = user_active_contract(manual_schedule(), None, contract_steps.clone())
      .expect("direct Actor Contract");
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    fund_native(actor_id, 100_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let actor_before = Actors::active_actor_view(actor_id).expect("actor before simulation");
    let events_before = System::events();
    let sink_before = native_balance(&TestFeeSink::get());
    set_fail_fee_sink_transfer(true);

    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::User,
        Mutability::Mutable,
        contract,
        SimulationMode::FreshCurrentPlan,
      ),
      Err(SimulationError::FeeCollectionFailed)
    );

    set_fail_fee_sink_transfer(false);
    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(System::events(), events_before);
    assert_eq!(native_balance(&TestFeeSink::get()), sink_before);
  });
}

#[test]
fn continuation_can_complete_at_stop_cycle_without_replaying_prefix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_pool(TestAsset::Native, TestAsset::Local(77), 10_000, 10_000);
    set_asset_balance(&u64::MAX, TestAsset::Local(77), 10_000);
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      }),
      StepOf::<Test> {
        precondition: None,
        task: Task::SwapIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::Fixed(20),
          slippage_tolerance: Perbill::one(),
        },
        on_error: RETRY_LATER,
      },
      make_step(Task::StopCycle),
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(5),
      }),
    ])
    .expect("four steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("suspended")
        .cursor,
      1
    );
    assert_eq!(native_balance(&BOB), bob_before + 10);

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);

    let actor = Actors::active_actor_view(actor_id).expect("actor remains active");
    assert_eq!(actor.cycle_state, CycleState::Idle);
    assert_eq!(actor.unsuccessful_attempt_streak, 0);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleStopped {
        actor_id: id,
        cycle_nonce: 1,
        step_index: 2,
      } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 3, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn temporary_failure_keeps_continue_next_step_semantics() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_pool(TestAsset::Native, TestAsset::Local(77), 10_000, 10_000);
    set_asset_balance(&u64::MAX, TestAsset::Local(77), 10_000);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![
        failing_step,
        make_step(Task::Transfer {
          to: CHARLIE,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(1),
        }),
      ])
      .expect("two steps fit"),
    );
    fund_native(actor_id, 100);
    let charlie_before = native_balance(&CHARLIE);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    let completed = Actors::active_actor_view(actor_id).expect("actor remains");
    assert_eq!(completed.cycle_state, CycleState::Idle);
    assert_eq!(completed.unsuccessful_attempt_streak, 0);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert_eq!(native_balance(&CHARLIE), charlie_before + 1);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          outcomes: OutcomeTotals { executed_steps: 1, failed_steps: 1, .. },
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn retry_later_funding_unavailable_resumes_without_new_logical_run() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut contract_steps = transfer_contract_steps(BOB, 50);
    contract_steps[0].on_error = RETRY_LATER;
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    let bob_before = native_balance(&BOB);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let suspended = Actors::continuation_state(actor_id).expect("funding retry persists");
    assert_eq!(suspended.cursor, 0);
    assert_eq!(suspended.cumulative_outcomes.failed_steps, 0);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor remains")
        .unsuccessful_attempt_streak,
      1
    );

    fund_native_raw(&actor, 51);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    let completed = Actors::active_actor_view(actor_id).expect("actor completes");
    assert_eq!(completed.cycle_nonce, 1);
    assert_eq!(completed.cycle_state, CycleState::Idle);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before + 50);
  });
}

#[test]
fn retry_later_local_attempt_cutoff_closes_without_prefix_replay() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_max_consecutive_failures(10);
    setup_pool(TestAsset::Native, TestAsset::Local(77), 10_000, 10_000);
    set_asset_balance(&u64::MAX, TestAsset::Local(77), 10_000);
    let retry_step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![
        make_step(Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(1),
        }),
        retry_step,
      ])
      .expect("two steps fit"),
    );
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("first unsuccessful attempt persists")
        .unsuccessful_attempts_at_cursor,
      1
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("second unsuccessful attempt persists")
        .unsuccessful_attempts_at_cursor,
      2
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(
      Actors::active_actor_view(actor_id)
        .expect("suspended actor remains")
        .pending_signal
    );

    frame_system::Pallet::<Test>::set_block_number(4);
    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(Actors::continuation_state(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before + 1);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::RetryAttemptsExhausted,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn retry_later_resets_local_attempt_count_after_cursor_advancement() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_max_consecutive_failures(10);
    setup_temporary_retry_pool();
    let plan = BoundedVec::try_from(vec![
      StepOf::<Test> {
        precondition: None,
        task: Task::SwapIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::Fixed(10),
          slippage_tolerance: Perbill::one(),
        },
        on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
      },
      StepOf::<Test> {
        precondition: None,
        task: Task::AddLiquidity {
          asset_a: TestAsset::Local(77),
          asset_b: TestAsset::Local(88),
          amount_a: AmountResolution::Fixed(1),
          amount_b: AmountResolution::Fixed(1),
          min_lp_out: 1,
        },
        on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
      },
    ])
    .expect("two retry steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 100);
    set_asset_balance(&actor, TestAsset::Local(88), 10);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let first = Actors::continuation_state(actor_id).expect("first cursor suspends");
    assert_eq!(
      (first.cursor, first.unsuccessful_attempts_at_cursor),
      (0, 1)
    );

    set_temporary_dex_failure(false);
    set_temporary_add_liquidity_failure(true);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    let advanced = Actors::continuation_state(actor_id).expect("later cursor suspends");
    assert_eq!(advanced.cursor, 1);
    assert_eq!(advanced.unsuccessful_attempts_at_cursor, 1);
  });
}

#[test]
fn global_failure_limit_can_close_before_retry_later_local_limit() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_max_consecutive_failures(1);
    setup_temporary_retry_pool();
    let plan = contract_steps_with_step(StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 2 },
    });
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(Actors::continuation_state(actor_id).is_none());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleCancelled { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary { actor_id: id, result: CycleResult::Failed, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ConsecutiveFailures,
      } if *id == actor_id
    )));
  });
}

#[test]
fn reached_global_failure_cutoff_closes_before_another_attempt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, temporary_retry_swap_plan());
    fund_native(actor_id, 100);
    let threshold = <Test as crate::Config>::MaxConsecutiveFailures::get();
    ActorHot::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("active actor")
        .unsuccessful_attempt_streak = threshold;
    });
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed { actor_id: id, reason: CloseReason::ConsecutiveFailures }
        if *id == actor_id
    )));
  });
}

#[test]
fn global_failure_limit_can_close_before_local_retry_limit() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_max_consecutive_failures(1);
    setup_temporary_retry_pool();
    let plan = contract_steps_with_step(StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 2 },
    });
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ConsecutiveFailures,
      } if *id == actor_id
    )));
  });
}

#[test]
fn scheduler_retries_manual_continuation_after_cooldown_without_new_signal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 2,
    };
    let actor_id = create_system_with(ALICE, schedule, None, temporary_retry_swap_plan());
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(scheduled_wakeup_block(actor_id), Some(3));
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("suspended")
        .unsuccessful_attempts_at_cursor,
      1
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("still suspended")
        .unsuccessful_attempts_at_cursor,
      1
    );

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(3);
    run_idle(Weight::MAX);
    let completed = Actors::active_actor_view(actor_id).expect("actor completes");
    assert_eq!(completed.cycle_nonce, 1);
    assert_eq!(completed.cycle_state, CycleState::Idle);
    assert!(Actors::continuation_state(actor_id).is_none());
  });
}

#[test]
fn canonical_fifo_executes_global_ticket_order_across_actor_types() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let user_a = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let system_a = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let user_b = create_user_with(
      BOB,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let system_b = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    fund_native(user_a, 1_000_000_000_000_000);
    fund_native(user_b, 1_000_000_000_000_000);

    for (owner, actor_id) in [
      (ALICE, user_a),
      (ALICE, system_a),
      (BOB, user_b),
      (ALICE, system_b),
    ] {
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(owner),
        actor_id
      ));
    }
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);

    let started: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(Event::CycleStarted { actor_id, .. }) => Some(actor_id),
        _ => None,
      })
      .collect();
    assert_eq!(started, vec![user_a, system_a, user_b]);
    assert_eq!(
      Actors::active_actor_view(system_b)
        .expect("fourth FIFO actor remains")
        .cycle_nonce,
      0
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id, .. } if *actor_id == system_b
    )));
  });
}

#[test]
fn canonical_fifo_uses_one_physical_ticket_sequence() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let system_a = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let user_a = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let system_b = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let user_b = create_user_with(
      BOB,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );

    for (owner, actor_id) in [
      (ALICE, system_a),
      (ALICE, user_a),
      (ALICE, system_b),
      (BOB, user_b),
    ] {
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(owner),
        actor_id
      ));
    }

    assert_eq!(Actors::next_queue_ticket(), 4);
    let tickets: Vec<_> = Actors::queue_pages(0)
      .expect("canonical queue page")
      .into_iter()
      .map(|entry| entry.ticket)
      .collect();
    assert_eq!(tickets, vec![0, 1, 2, 3]);
    assert_eq!(Actors::queue_occupancy(), 4);
  });
}

#[test]
fn canonical_head_discovery_distinguishes_empty_head_and_blocked() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let scan = <TestWeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1);
    assert_eq!(Actors::test_head_discovery(0, 1, 0, scan), (0, None, 0));

    let system = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::paged_enqueue(system));
    let cutoff = Actors::next_queue_ticket();
    let (state, entry, scanned) = Actors::test_head_discovery(cutoff, 1, 0, scan);
    assert_eq!((state, scanned), (1, 1));
    assert_eq!(entry.map(|entry| entry.actor_id), Some(system));

    assert_eq!(
      Actors::test_head_discovery(cutoff, 1, 1, scan),
      (4, None, 1),
      "an exhausted scan ceiling is a silent pass exhaustion"
    );
    assert_eq!(
      Actors::test_head_discovery(cutoff, 1, 0, Weight::zero()),
      (2, None, 0),
      "an unadmitted live-head probe is a weight stall"
    );
    QueueTail::<Test>::mutate(|tail| *tail = tail.saturating_add(1));
    assert_eq!(
      Actors::test_head_discovery(cutoff, 1, 0, scan),
      (3, None, 0),
      "defensive topology rejection is an invariant stall"
    );
    assert!(Actors::execute_cycle(Weight::MAX).starved);
  });
}

#[test]
fn canonical_tombstones_cannot_bypass_the_oldest_live_head() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let scan = <TestWeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1);
    let old_user = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    assert!(Actors::paged_enqueue(old_user));
    for _ in 0..3 {
      let tombstone = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert!(Actors::paged_enqueue(tombstone));
      assert!(Actors::paged_invalidate(tombstone).is_some());
    }
    let later_system = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::paged_enqueue(later_system));
    let cutoff = Actors::next_queue_ticket();
    let (state, entry, _) = Actors::test_head_discovery(cutoff, 1, 0, scan);
    assert_eq!(state, 1);
    assert_eq!(entry.map(|entry| entry.actor_id), Some(old_user));

    assert_eq!(Actors::paged_invalidate(old_user), Some(0));
    let (state, entry, scanned) = Actors::test_head_discovery(cutoff, 5, 0, scan.saturating_mul(5));
    assert_eq!((state, scanned), (1, 5));
    assert_eq!(entry.map(|entry| entry.actor_id), Some(later_system));
  });
}

#[test]
fn capped_exponential_balances_retry_pressure_and_recovery_at_maximum_occupancy() {
  const HORIZON: u32 = 64;
  const MAX_DELAY: u32 = 8;
  let max_occupancy = <<Test as crate::Config>::MaxActiveActors as Get<u32>>::get() as u64;

  let due_blocks = |fixed_delay: Option<u32>| {
    let mut blocks = Vec::new();
    let mut due = 0u32;
    let mut attempt = 0u32;
    while due <= HORIZON.saturating_add(MAX_DELAY) {
      let delay = fixed_delay.unwrap_or_else(|| Actors::retry_backoff_blocks(attempt));
      due = due.checked_add(delay).expect("bounded experiment horizon");
      blocks.push(due);
      attempt = attempt.checked_add(1).expect("bounded experiment attempts");
    }
    blocks
  };
  let metrics = |fixed_delay| {
    let due = due_blocks(fixed_delay);
    let attempts_per_actor = due.iter().filter(|block| **block <= HORIZON).count() as u64;
    let recovery_wait_sum = (1..=HORIZON)
      .map(|available_at| {
        due
          .iter()
          .copied()
          .find(|block| *block >= available_at)
          .expect("experiment extends through the capped delay")
          - available_at
      })
      .sum::<u32>();
    (
      attempts_per_actor.saturating_mul(max_occupancy),
      recovery_wait_sum,
      max_occupancy,
    )
  };

  let exponential = metrics(None);
  let fixed_one = metrics(Some(1));
  let fixed_four = metrics(Some(4));
  let fixed_eight = metrics(Some(8));
  let evidence: serde_json::Value = serde_json::from_str(include_str!(
    "../tests/fixtures/retry-backoff-decision.v1.json"
  ))
  .expect("retry-backoff decision fixture parses");
  let expected = |policy: &str, metric: &str| {
    evidence[policy][metric]
      .as_u64()
      .expect("decision metric is an unsigned integer")
  };
  assert_eq!(
    exponential.0,
    expected("cappedExponential", "dueRetryObligations")
  );
  assert_eq!(
    u64::from(exponential.1),
    expected("cappedExponential", "recoveryWaitSum")
  );
  assert_eq!(fixed_one.0, expected("fixedDelay1", "dueRetryObligations"));
  assert_eq!(
    u64::from(fixed_one.1),
    expected("fixedDelay1", "recoveryWaitSum")
  );
  assert_eq!(fixed_four.0, expected("fixedDelay4", "dueRetryObligations"));
  assert_eq!(
    u64::from(fixed_four.1),
    expected("fixedDelay4", "recoveryWaitSum")
  );
  assert_eq!(
    fixed_eight.0,
    expected("fixedDelay8", "dueRetryObligations")
  );
  assert_eq!(
    u64::from(fixed_eight.1),
    expected("fixedDelay8", "recoveryWaitSum")
  );
  assert_eq!(evidence["decision"], "retain-capped-exponential");
  assert!(exponential.0 < fixed_four.0);
  assert!(exponential.1 < fixed_eight.1);
  assert_eq!(
    exponential.2, fixed_eight.2,
    "delay policy changes neither maximum occupancy nor FIFO cohort fairness"
  );
}

#[test]
fn temporary_retry_backoff_is_one_two_four_eight_then_capped() {
  assert_eq!(Actors::retry_backoff_blocks(0), 1);
  assert_eq!(Actors::retry_backoff_blocks(1), 2);
  assert_eq!(Actors::retry_backoff_blocks(2), 4);
  assert_eq!(Actors::retry_backoff_blocks(3), 8);
  assert_eq!(Actors::retry_backoff_blocks(31), 8);
  assert_eq!(Actors::retry_backoff_blocks(u32::MAX), 8);

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    set_max_consecutive_failures(10);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, temporary_retry_swap_plan());
    fund_native(actor_id, 100);
    let actor = sovereign_account(actor_id);
    let balance_before = native_balance(&actor);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let initial_hot = Actors::actor_hot(actor_id).expect("suspended actor stays schedulable");
    assert!(initial_hot.queue_ticket.is_some());
    assert!(initial_hot.wakeup_pointer.is_none());
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("attempt zero")
        .cursor,
      0
    );
    assert_eq!(native_balance(&actor), balance_before);

    for (due, expected_attempt, next_due) in [(2, 1, 4), (4, 2, 8), (8, 3, 16), (16, 4, 24)] {
      frame_system::Pallet::<Test>::set_block_number(due - 1);
      run_idle(Weight::MAX);
      assert_eq!(
        Actors::continuation_state(actor_id)
          .expect("not eligible before due block")
          .unsuccessful_attempts_at_cursor,
        expected_attempt
      );
      frame_system::Pallet::<Test>::set_block_number(due);
      run_idle(Weight::MAX);
      let continuation =
        Actors::continuation_state(actor_id).expect("temporary failure resuspends");
      assert_eq!(
        continuation.unsuccessful_attempts_at_cursor,
        expected_attempt + 1
      );
      assert_eq!(continuation.cursor, 0);
      assert_eq!(scheduled_wakeup_block(actor_id), Some(next_due));
      assert_eq!(native_balance(&actor), balance_before);
    }

    frame_system::Pallet::<Test>::set_block_number(23);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("capped delay holds")
        .unsuccessful_attempts_at_cursor,
      5
    );
    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(24);
    run_idle(Weight::MAX);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert_eq!(native_balance(&actor), balance_before - 10);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor completed")
        .cycle_state,
      CycleState::Idle
    );
  });
}

#[test]
fn continuation_weight_deferral_does_not_admit_attempt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      temporary_retry_swap_plan(),
    );
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    run_idle(Weight::MAX);
    let before = Actors::continuation_state(actor_id).expect("suspended");
    let ticket_before = Actors::actor_hot(actor_id)
      .expect("suspended actor")
      .queue_ticket;

    frame_system::Pallet::<Test>::set_block_number(2);
    let instance = Actors::active_actor_view(actor_id).expect("suspended actor");
    let queue_weight = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1)
      .saturating_add(Actors::scheduler_actor_probe_weight_upper())
      .saturating_add(
        <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
          .max(<<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page()),
      );
    let proof_limit = queue_weight
      .proof_size()
      .saturating_add(Actors::attempt_weight_upper_bound(&instance, 1).proof_size())
      .saturating_sub(1);
    Actors::execute_cycle(Weight::from_parts(u64::MAX, proof_limit));

    let after = Actors::continuation_state(actor_id).expect("deferral preserves continuation");
    assert_eq!(
      after.unsuccessful_attempts_at_cursor,
      before.unsuccessful_attempts_at_cursor
    );
    assert_eq!(after.last_attempt_block, before.last_attempt_block);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor remains")
        .unsuccessful_attempt_streak,
      1
    );
    assert_eq!(
      Actors::actor_hot(actor_id)
        .expect("suspended actor")
        .queue_ticket,
      ticket_before
    );
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleContinued { actor_id: id, .. } | Event::CycleSummary { actor_id: id, .. }
        if *id == actor_id
    )));
  });
}

#[test]
fn fresh_attempt_rolls_back_before_effect_when_tick_rearm_is_exhausted() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      timer_schedule(1),
      None,
      transfer_contract_steps(BOB, 10),
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    let mut wakeup_meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut wakeup_meter);
    let actor_before = Actors::active_actor_view(actor_id).expect("queued fresh actor");
    let bob_before = native_balance(&BOB);
    let events_before = System::events();
    crate::WakeupCursorLen::<Test>::insert(
      WakeupClock::Tick,
      <<Test as crate::Config>::MaxActiveActors as Get<u32>>::get(),
    );
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    let _ = Actors::execute_cycle(Weight::MAX);

    assert_eq!(native_balance(&BOB), bob_before, "task effect rolls back");
    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(System::events(), events_before, "run events roll back");
    assert!(
      Actors::actor_hot(actor_id)
        .expect("fresh actor remains queued")
        .queue_ticket
        .is_some()
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn post_attempt_rearm_rolls_back_on_fifo_topology_corruption() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      timer_schedule(1),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    frame_system::Pallet::<Test>::set_block_number(2);
    let mut wakeup_meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut wakeup_meter);
    crate::WakeupBuckets::<Test>::insert(
      WakeupKey::Tick(3),
      WakeupBucketState {
        head_page: 0,
        tail_page: 0,
        next_page_id: 1,
        live_entries: 0,
        cursor_index: None,
      },
    );
    let actor_before = Actors::active_actor_view(actor_id).expect("queued actor");
    let bob_before = native_balance(&BOB);
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    let _ = Actors::execute_cycle(Weight::MAX);

    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn scheduler_close_rolls_back_on_fifo_topology_corruption() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    frame_system::Pallet::<Test>::set_block_number(102);
    Actors::test_corrupt_queue_before_close_consume();
    let actor_before = Actors::active_actor_view(actor_id).expect("actor");
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    let _ = Actors::execute_cycle(Weight::MAX);

    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn continuation_attempt_rolls_back_when_retry_wakeup_topology_is_corrupt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    set_max_consecutive_failures(10);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, temporary_retry_swap_plan());
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let due = 2u64;
    frame_system::Pallet::<Test>::set_block_number(due);
    let next_retry = due.saturating_add(2);
    crate::WakeupBuckets::<Test>::insert(
      WakeupKey::Block(next_retry),
      crate::WakeupBucketState {
        head_page: 0,
        tail_page: 0,
        next_page_id: 1,
        live_entries: 0,
        cursor_index: None,
      },
    );
    let actor_before = Actors::active_actor_view(actor_id).expect("queued continuation");
    let continuation_before = Actors::continuation_state(actor_id)
      .expect("continuation before corrupt retry placement")
      .encode();
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    let _ = Actors::execute_cycle(Weight::MAX);

    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("continuation survives failed placement")
        .encode(),
      continuation_before,
    );
    assert_eq!(System::events(), events_before, "attempt events roll back");
    assert!(
      Actors::actor_hot(actor_id)
        .expect("continuation remains queued")
        .queue_ticket
        .is_some()
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn continuation_retry_omits_external_timer_cadence() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let schedule = Schedule {
      trigger: Trigger::cadenced(100),
      cooldown_blocks: 0,
    };
    let actor_id = create_system_with(ALICE, schedule, None, temporary_retry_swap_plan());
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);

    let cadence_due = scheduled_wakeup_block(actor_id).expect("cadenced wakeup");
    frame_system::Pallet::<Test>::set_block_number(cadence_due);
    run_idle(Weight::MAX);

    assert_eq!(scheduled_wakeup_block(actor_id), None);
    assert!(
      Actors::actor_hot(actor_id)
        .expect("suspended cadence actor")
        .queue_ticket
        .is_some()
    );
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("suspended")
        .unsuccessful_attempts_at_cursor,
      1
    );
  });
}

#[test]
fn signal_during_suspension_latches_a_later_logical_run() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let schedule = on_address_event_schedule(SourceFilter::Any, AssetFilter::Any);
    let actor_id = create_system_with(ALICE, schedule, None, temporary_retry_swap_plan());
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      1,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("suspended")
        .cycle_nonce,
      1
    );
    assert!(!Actors::pending_signal(actor_id));
    let retry_ticket = Actors::actor_hot(actor_id)
      .expect("suspended actor")
      .queue_ticket;
    assert!(retry_ticket.is_some());

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      1,
      &ALICE
    ));
    assert!(Actors::pending_signal(actor_id));
    assert_eq!(
      Actors::actor_hot(actor_id)
        .expect("suspended actor")
        .queue_ticket,
      retry_ticket
    );
    set_temporary_dex_failure(false);
    run_idle(Weight::MAX);

    let after_retry = Actors::active_actor_view(actor_id).expect("retry completes");
    assert_eq!(after_retry.cycle_nonce, 1);
    assert_eq!(after_retry.cycle_state, CycleState::Idle);
    assert!(after_retry.pending_signal);
    assert!(after_retry.queue_ticket.is_some());

    frame_system::Pallet::<Test>::set_block_number(3);
    run_idle(Weight::MAX);
    let after_next_run = Actors::active_actor_view(actor_id).expect("later run completes");
    assert_eq!(after_next_run.cycle_nonce, 2);
    assert!(!after_next_run.pending_signal);
  });
}

#[test]
fn pause_and_breaker_gate_scheduler_owned_retry() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, temporary_retry_swap_plan());
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("suspended")
        .unsuccessful_attempts_at_cursor,
      1
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("paused")
        .unsuccessful_attempts_at_cursor,
      1
    );

    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("breaker gated")
        .unsuccessful_attempts_at_cursor,
      1
    );

    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      false
    ));
    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(4);
    run_idle(Weight::MAX);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("completed")
        .cycle_state,
      CycleState::Idle
    );
  });
}

#[test]
fn suspended_retry_wakes_for_window_expiry_before_cooldown() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 200,
    };
    let actor_id = create_system_with(
      ALICE,
      schedule,
      Some(ScheduleWindow { start: 1, end: 101 }),
      temporary_retry_swap_plan(),
    );
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(scheduled_wakeup_block(actor_id), Some(102));

    frame_system::Pallet::<Test>::set_block_number(102);
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(Actors::continuation_state(actor_id).is_none());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::WindowExpired,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn funding_arrival_during_suspension_accumulates_for_the_next_cycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: None,
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      },
      on_error: RETRY_LATER,
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(step),
    );
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("suspended")
        .cycle_state,
      CycleState::Suspended
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSuspended {
        actor_id: id,
        cycle_nonce: 1,
        cursor: 0,
        reason: SuspensionReason::FundingUnavailable,
        ..
      } if *id == actor_id
    )));
    let retry_ticket = Actors::actor_hot(actor_id)
      .expect("suspended actor")
      .queue_ticket;

    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      7,
      &ALICE
    ));
    let funding = actor_funding(actor_id);
    assert_eq!(
      funding.funding_accumulated.get(&TestAsset::Native),
      Some(&7)
    );
    assert_eq!(
      Actors::actor_hot(actor_id)
        .expect("suspended actor")
        .queue_ticket,
      retry_ticket
    );
  });
}

#[test]
fn user_retry_admits_and_charges_only_the_unresolved_suffix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
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
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      }),
    ])
    .expect("three-step User plan fits");
    let prefix_fee = user_step_fee(&plan[0]);
    let retry_fee = user_step_fee(&plan[1]);
    let tail_fee = user_step_fee(&plan[2]);
    let actor_id = create_user_with(ALICE, Mutability::Mutable, manual_schedule(), None, plan);
    fund_native(actor_id, 1_000_000_000_000_000_000);
    let sink = TestFeeSink::get();
    let sink_before = native_balance(&sink);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&sink) - sink_before, prefix_fee + retry_fee);
    let suspended = Actors::active_actor_view(actor_id).expect("suspended User actor");
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("continuation")
        .cursor,
      1
    );
    let retry_weight = Actors::attempt_weight_upper_bound(&suspended, 1);
    let full_weight = Actors::attempt_weight_upper_bound(&suspended, 0);
    assert!(retry_weight.ref_time() < full_weight.ref_time());
    assert!(retry_weight.proof_size() < full_weight.proof_size());

    frame_system::Pallet::<Test>::set_block_number(2);
    let retry_budget = Actors::scheduler_admission_overhead().saturating_add(retry_weight);
    run_idle(retry_budget);
    assert_eq!(
      native_balance(&sink) - sink_before,
      prefix_fee + retry_fee.saturating_mul(2)
    );
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("retry remains")
        .unsuccessful_attempts_at_cursor,
      2
    );

    frame_system::Pallet::<Test>::set_block_number(4);
    set_temporary_dex_failure(false);
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&sink) - sink_before,
      prefix_fee + retry_fee.saturating_mul(3) + tail_fee
    );
    assert!(Actors::continuation_state(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
  });
}

#[test]
fn continuation_snapshot_is_trimmed_frozen_and_capacity_checked_live() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_a = TestAsset::Local(1);
    let asset_b = TestAsset::Local(2);
    let asset_c = TestAsset::Local(3);
    let asset_out = TestAsset::Local(4);
    setup_pool(asset_b, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: asset_a,
        amount: AmountResolution::PercentageAtOpening(Perbill::from_percent(10)),
      }),
      StepOf::<Test> {
        precondition: None,
        task: Task::SwapIn {
          asset_in: asset_b,
          asset_out,
          amount_in: AmountResolution::PercentageAtOpening(Perbill::from_percent(50)),
          slippage_tolerance: Perbill::one(),
        },
        on_error: RETRY_LATER,
      },
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: asset_c,
        amount: AmountResolution::PercentageAtOpening(Perbill::from_percent(10)),
      }),
    ])
    .expect("three-step plan fits");
    let actor_id = create_system_with(ALICE, percentage_trigger_schedule(), None, plan);
    let actor = sovereign_account(actor_id);
    for asset in [asset_a, asset_b, asset_c] {
      set_asset_balance(&actor, asset, 100);
    }
    set_temporary_dex_failure(true);
    signal_percentage_trigger(actor_id, asset_b);
    run_idle(Weight::MAX);

    let continuation = Actors::continuation_state(actor_id).expect("suspended");
    assert_eq!(continuation.cursor, 1);
    assert_eq!(continuation.opening_snapshot.len(), 2);
    let suspended = Actors::active_actor_view(actor_id).expect("suspended actor");
    let retry_weight = Actors::attempt_weight_upper_bound(&suspended, continuation.cursor as usize);
    let full_weight = Actors::attempt_weight_upper_bound(&suspended, 0);
    assert!(retry_weight.ref_time() < full_weight.ref_time());
    assert!(retry_weight.proof_size() < full_weight.proof_size());
    assert!(
      !continuation
        .opening_snapshot
        .contains_key(&OpeningSurface::PreservableAsset(asset_a))
    );
    assert_eq!(
      continuation
        .opening_snapshot
        .get(&OpeningSurface::PreservableAsset(asset_b)),
      Some(&99)
    );
    assert_eq!(asset_balance(&BOB, asset_a), 9);

    assert_ok!(MockAssetOps::transfer(&actor, &BOB, asset_b, 20));
    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    let after_capacity_failure = Actors::continuation_state(actor_id).expect("still suspended");
    assert_eq!(after_capacity_failure.cursor, 1);
    assert_eq!(after_capacity_failure.unsuccessful_attempts_at_cursor, 2);
    assert_eq!(
      after_capacity_failure
        .opening_snapshot
        .get(&OpeningSurface::PreservableAsset(asset_b)),
      Some(&99)
    );

    set_asset_balance(&actor, asset_b, 100);
    frame_system::Pallet::<Test>::set_block_number(4);
    run_idle(Weight::MAX);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert_eq!(asset_balance(&actor, asset_b), 82);
    assert_eq!(asset_balance(&CHARLIE, asset_c), 9);
    assert_eq!(asset_balance(&BOB, asset_a), 9);
  });
}

#[test]
fn missing_frozen_snapshot_is_a_permanent_invariant_failure() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_in = TestAsset::Local(8);
    let asset_out = TestAsset::Local(9);
    setup_pool(asset_in, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in,
        asset_out,
        amount_in: AmountResolution::PercentageAtOpening(Perbill::from_percent(50)),
        slippage_tolerance: Perbill::one(),
      },
      on_error: RETRY_LATER,
    };
    let actor_id = create_system_with(
      ALICE,
      percentage_trigger_schedule(),
      None,
      contract_steps_with_step(step),
    );
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_in, 100);
    set_temporary_dex_failure(true);
    signal_percentage_trigger(actor_id, asset_in);
    run_idle(Weight::MAX);
    ContinuationStateStore::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("suspended continuation")
        .opening_snapshot
        .clear();
    });

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(Actors::continuation_state(actor_id).is_none());
    let actor_state =
      Actors::active_actor_view(actor_id).expect("actor remains after permanent failure");
    assert_eq!(actor_state.cycle_state, CycleState::Idle);
    assert_eq!(actor_state.unsuccessful_attempt_streak, 2);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepFailed {
        actor_id: id,
        step_index: 0,
        error,
        ..
      } if *id == actor_id && *error == Error::<Test>::SnapshotUnavailable.into()
    )));
  });
}

#[test]
fn maximal_continuation_snapshot_stays_bounded_to_unresolved_surfaces() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut steps = Vec::new();
    for index in 0..8u32 {
      steps.push(StepOf::<Test> {
        precondition: None,
        task: Task::AddLiquidity {
          asset_a: TestAsset::Local(100 + index * 2),
          asset_b: TestAsset::Local(101 + index * 2),
          amount_a: AmountResolution::PercentageAtOpening(Perbill::from_percent(10)),
          amount_b: AmountResolution::PercentageAtOpening(Perbill::from_percent(10)),
          min_lp_out: 1,
        },
        on_error: if index == 0 {
          RETRY_LATER
        } else {
          StepErrorPolicy::AbortCycle
        },
      });
    }
    let plan = BoundedVec::try_from(steps).expect("maximal System plan fits");
    let actor_id = create_system_with(ALICE, percentage_trigger_schedule(), None, plan);
    let actor = sovereign_account(actor_id);
    for index in 0..16u32 {
      set_asset_balance(&actor, TestAsset::Local(100 + index), 100);
    }
    set_temporary_add_liquidity_failure(true);
    signal_percentage_trigger(actor_id, TestAsset::Local(100));
    run_idle(Weight::MAX);

    let continuation = Actors::continuation_state(actor_id).expect("maximal continuation");
    assert_eq!(continuation.cursor, 0);
    assert_eq!(
      continuation.opening_snapshot.len() as u32,
      <<Test as crate::Config>::MaxOpeningSnapshotEntries as Get<u32>>::get()
    );

    set_temporary_add_liquidity_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(Actors::continuation_state(actor_id).is_none());
  });
}

#[test]
fn suspended_cycle_freezes_its_funding_snapshot_and_preserves_new_accumulation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_in = TestAsset::Local(5);
    let asset_out = TestAsset::Local(6);
    setup_pool(asset_in, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in,
        asset_out,
        amount_in: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
        slippage_tolerance: Perbill::one(),
      },
      on_error: RETRY_LATER,
    };
    let actor_id = create_system_with(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      contract_steps_with_step(step),
    );
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_in, 100);
    assert_ok!(Actors::notify_address_event(
      actor_id, asset_in, 100, &ALICE
    ));
    set_temporary_dex_failure(true);
    run_idle(Weight::MAX);
    let continuation = Actors::continuation_state(actor_id).expect("suspended");
    assert!(continuation.opening_snapshot.is_empty());
    assert_eq!(continuation.funding_snapshot.get(&asset_in), Some(&100));

    set_asset_balance(&actor, asset_in, 30);
    assert_ok!(Actors::notify_address_event(actor_id, asset_in, 30, &ALICE));
    assert_eq!(
      actor_funding(actor_id).funding_accumulated.get(&asset_in),
      Some(&30)
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("still suspended")
        .funding_snapshot
        .get(&asset_in),
      Some(&100)
    );
    assert_eq!(
      actor_funding(actor_id).funding_accumulated.get(&asset_in),
      Some(&30)
    );

    set_asset_balance(&actor, asset_in, 100);
    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(4);
    run_idle(Weight::MAX);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert_eq!(
      actor_funding(actor_id).funding_accumulated.get(&asset_in),
      Some(&30)
    );
  });
}

#[test]
fn explicit_cancellation_preserves_committed_effects_and_emits_terminal_summary() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      }),
      StepOf::<Test> {
        precondition: None,
        task: Task::SwapIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
          slippage_tolerance: Perbill::one(),
        },
        on_error: RETRY_LATER,
      },
    ])
    .expect("two-step plan fits");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 100);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      20,
      &ALICE
    ));
    let bob_before = native_balance(&BOB);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      7,
      &ALICE
    ));
    let actor_before_cancel = native_balance(&actor);
    let failures_before_cancel = Actors::active_actor_view(actor_id)
      .expect("suspended actor")
      .unsuccessful_attempt_streak;

    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::cancel_continuation(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(Actors::continuation_state(actor_id).is_none());
    let actor_state = Actors::active_actor_view(actor_id).expect("cancelled actor remains");
    assert_eq!(actor_state.cycle_state, CycleState::Idle);
    assert_eq!(actor_state.cycle_nonce, 1);
    assert_eq!(
      actor_state.unsuccessful_attempt_streak,
      failures_before_cancel
    );
    assert!(actor_state.queue_ticket.is_none());
    assert!(
      Actors::actor_hot(actor_id)
        .expect("cancelled actor hot state")
        .wakeup_pointer
        .is_none()
    );
    assert_eq!(native_balance(&actor), actor_before_cancel);
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&7)
    );
    let events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(events.len(), 2);
    assert!(matches!(
      events[0],
      Event::CycleCancelled {
        actor_id: id,
        cycle_nonce: 1,
        reason: CancellationReason::Explicit,
      } if id == actor_id
    ));
    assert!(matches!(
      events[1],
      Event::CycleSummary {
        actor_id: id,
        cycle_nonce: 1,
        result: CycleResult::Cancelled,
        outcomes: OutcomeTotals { executed_steps: 1, failed_steps: 1, .. },
        ..
      } if id == actor_id
    ));
    assert_noop!(
      Actors::cancel_continuation(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ContinuationNotFound
    );
  });
}

#[test]
fn cancelled_continuation_leaves_only_convergent_queue_and_wakeup_tombstones() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 10,
      },
      None,
      temporary_retry_swap_plan(),
    );
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(scheduled_wakeup_block(actor_id), Some(11));
    assert!(Actors::wakeup_buckets(11).is_some());

    assert_ok!(Actors::cancel_continuation(RuntimeOrigin::root(), actor_id));
    assert!(scheduled_wakeup_block(actor_id).is_none());
    frame_system::Pallet::<Test>::set_block_number(11);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(Actors::wakeup_buckets(11).is_none());
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor remains")
        .cycle_nonce,
      1
    );
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn cancellation_requeues_a_signal_latched_for_the_next_logical_run() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(Actors::pending_signal(actor_id));
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      inert_contract_steps(),
      crate::CompletionPolicy::Persistent,
    ));
    let cancelled = Actors::active_actor_view(actor_id).expect("cancelled actor remains");
    assert_eq!(cancelled.cycle_state, CycleState::Idle);
    assert!(cancelled.pending_signal);
    assert!(cancelled.queue_ticket.is_some());

    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    let completed = Actors::active_actor_view(actor_id).expect("next logical cycle completes");
    assert_eq!(completed.cycle_nonce, 2);
    assert!(!completed.pending_signal);
  });
}

#[test]
fn continuation_attempts_have_unique_chain_coordinates_without_the_stored_ordinal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      temporary_retry_swap_plan(),
    );
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    run_idle(Weight::MAX);
    let opening_event_index = frame_system::Pallet::<Test>::events()
      .iter()
      .position(|record| matches!(record.event, RuntimeEvent::Actors(Event::CycleStarted { actor_id: id, cycle_nonce: 1 }) if id == actor_id))
      .expect("opening attempt has a chain event coordinate");
    let opening: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert!(matches!(opening[1], Event::CycleStarted { actor_id: id, cycle_nonce: 1 } if id == actor_id));
    assert!(matches!(opening[2], Event::StepFailed { actor_id: id, cycle_nonce: 1, step_index: 0, .. } if id == actor_id));
    assert!(matches!(opening[3], Event::CycleSuspended { actor_id: id, cycle_nonce: 1, cursor: 0, reason: SuspensionReason::Temporary, .. } if id == actor_id));

    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let retry_event_index = frame_system::Pallet::<Test>::events()
      .iter()
      .position(|record| matches!(record.event, RuntimeEvent::Actors(Event::CycleContinued { actor_id: id, cycle_nonce: 1, cursor: 0, .. }) if id == actor_id))
      .expect("retry attempt has a chain event coordinate");
    let retry: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert!(matches!(retry[0], Event::CycleContinued { actor_id: id, cycle_nonce: 1, cursor: 0 } if id == actor_id));
    assert!(matches!(retry[1], Event::StepFailed { actor_id: id, cycle_nonce: 1, step_index: 0, .. } if id == actor_id));
    assert!(matches!(retry[2], Event::CycleSuspended { actor_id: id, cycle_nonce: 1, cursor: 0, .. } if id == actor_id));

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(4);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let completion_event_index = frame_system::Pallet::<Test>::events()
      .iter()
      .position(|record| matches!(record.event, RuntimeEvent::Actors(Event::CycleContinued { actor_id: id, cycle_nonce: 1, cursor: 0, .. }) if id == actor_id))
      .expect("completion attempt has a chain event coordinate");
    let completion: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert!(matches!(completion[0], Event::CycleContinued { actor_id: id, cycle_nonce: 1, cursor: 0 } if id == actor_id));
    assert!(matches!(completion[1], Event::SwapExecuted { actor_id: id, .. } if id == actor_id));
    assert!(matches!(completion[2], Event::CycleSummary { actor_id: id, cycle_nonce: 1, outcomes: OutcomeTotals { failed_steps: 2, .. }, .. } if id == actor_id));

    let attempt_coordinates = [
      (1u64, 1u64, opening_event_index),
      (1u64, 2u64, retry_event_index),
      (1u64, 4u64, completion_event_index),
    ];
    assert_eq!(
      attempt_coordinates.into_iter().collect::<BTreeSet<_>>().len(),
      attempt_coordinates.len(),
      "cycle nonce plus block/event coordinates uniquely identify every attempt without its stored ordinal"
    );
  });
}

#[test]
fn exact_update_noops_preserve_all_actor_state_and_emit_nothing() {
  new_test_ext().execute_with(|| {
    let encoded_actor_state = |actor_id| {
      (
        ActorHot::<Test>::get(actor_id).encode(),
        ActorContracts::<Test>::get(actor_id).encode(),
        crate::ActorFunding::<Test>::get(actor_id).encode(),
        ContinuationStateStore::<Test>::get(actor_id).encode(),
      )
    };
    let plan_id = create_suspended_system_retry(1);
    let plan_before = encoded_actor_state(plan_id);
    let stored_contract = ActorContracts::<Test>::get(plan_id).expect("active Actor Contract");
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      plan_id,
      stored_contract.steps,
      stored_contract.completion,
    ));
    assert_eq!(encoded_actor_state(plan_id), plan_before);
    assert!(frame_system::Pallet::<Test>::events().is_empty());

    let policy_id = create_suspended_system_retry(2);
    let policy_before = encoded_actor_state(policy_id);
    let policy = ActorContracts::<Test>::get(policy_id)
      .expect("active Actor Contract")
      .funding;
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      policy_id,
      policy,
    ));
    assert_eq!(encoded_actor_state(policy_id), policy_before);
    assert!(frame_system::Pallet::<Test>::events().is_empty());

    let schedule_id = create_suspended_system_retry(3);
    let schedule_before = encoded_actor_state(schedule_id);
    let stored_schedule = ActorContracts::<Test>::get(schedule_id).expect("active Actor Contract");
    let schedule = RuntimeSchedule {
      trigger: stored_schedule.trigger,
      cooldown_blocks: stored_schedule.cooldown_blocks,
    };
    let schedule_window = stored_schedule.window;
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      schedule_id,
      schedule,
      schedule_window,
    ));
    assert_eq!(encoded_actor_state(schedule_id), schedule_before);
    assert!(frame_system::Pallet::<Test>::events().is_empty());

    let auto_close_id = create_suspended_system_retry(4);
    let continuation_before = ContinuationStateStore::<Test>::get(auto_close_id).encode();
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(replace_auto_close(
      RuntimeOrigin::root(),
      auto_close_id,
      Some(2),
    ));
    assert_eq!(
      ContinuationStateStore::<Test>::get(auto_close_id).encode(),
      continuation_before
    );
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleCancelled { actor_id, .. } if *actor_id == auto_close_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ContractUpdated { actor_id } if *actor_id == auto_close_id
    )));
    let auto_close_before = encoded_actor_state(auto_close_id);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(replace_auto_close(
      RuntimeOrigin::root(),
      auto_close_id,
      Some(2),
    ));
    assert_eq!(encoded_actor_state(auto_close_id), auto_close_before);
    assert!(frame_system::Pallet::<Test>::events().is_empty());
  });
}

#[test]
fn semantic_control_origins_share_one_queue_churn_clock() {
  new_test_ext().execute_with(|| {
    let plan_id = create_suspended_system_retry(1);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      plan_id,
      inert_contract_steps(),
      crate::CompletionPolicy::Persistent,
    ));
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::signed(ALICE),
        plan_id,
        timer_schedule(2),
        None,
      ),
      Error::<Test>::ControlMutationRateLimited
    );

    let policy_id = create_suspended_system_retry(2);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      policy_id,
      FundingSourcePolicy::AnyVerifiedIngress,
    ));
    assert_noop!(
      Actors::deactivate_actor(RuntimeOrigin::root(), policy_id),
      Error::<Test>::ControlMutationRateLimited
    );

    let schedule_id = create_suspended_system_retry(3);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      schedule_id,
      timer_schedule(2),
      None,
    ));
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::signed(ALICE), schedule_id),
      Error::<Test>::ControlMutationRateLimited
    );

    let cancel_id = create_suspended_system_retry(4);
    assert_ok!(Actors::pause_actor(RuntimeOrigin::root(), cancel_id));
    assert_noop!(
      Actors::cancel_continuation(RuntimeOrigin::signed(ALICE), cancel_id),
      Error::<Test>::ControlMutationRateLimited
    );

    frame_system::Pallet::<Test>::set_block_number(5);
    let dormant_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None,
    ));
    frame_system::Pallet::<Test>::set_block_number(6);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      dormant_id,
      system_active_contract(manual_schedule(), None, inert_contract_steps())
        .expect("direct Actor Contract"),
    ));
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::root(), dormant_id),
      Error::<Test>::ControlMutationRateLimited
    );
  });
}

#[test]
fn completion_policy_only_replacement_preserves_continuation() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let before = Actors::active_actor_view(actor_id).expect("suspended actor");
    let continuation_before = Actors::continuation_state(actor_id).expect("suspended continuation");
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      before.steps.clone(),
      crate::CompletionPolicy::CloseAfterProductiveCycle,
    ));
    let after = Actors::active_actor_view(actor_id).expect("updated actor");
    assert_eq!(after.steps, before.steps);
    assert_eq!(
      after.completion,
      crate::CompletionPolicy::CloseAfterProductiveCycle
    );
    assert_eq!(after.cycle_state, CycleState::Suspended);
    assert_eq!(
      Actors::continuation_state(actor_id).map(|state| state.encode()),
      Some(continuation_before.encode())
    );
    let events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(events.len(), 1);
    assert!(matches!(
      events.first(),
      Some(Event::ContractUpdated { actor_id: id, .. }) if *id == actor_id
    ));
  });
}

#[test]
fn contract_policy_schedule_deactivation_and_close_cancel_with_typed_reasons() {
  new_test_ext().execute_with(|| {
    let plan_id = create_suspended_system_retry(1);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      plan_id,
      inert_contract_steps(),
      crate::CompletionPolicy::Persistent,
    ));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleCancelled { actor_id, reason: CancellationReason::ContractReplaced, .. }
        if *actor_id == plan_id
    )));

    let policy_id = create_suspended_system_retry(2);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      policy_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleCancelled { actor_id, reason: CancellationReason::ContractReplaced, .. }
        if *actor_id == policy_id
    )));

    let schedule_id = create_suspended_system_retry(3);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      schedule_id,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 1,
      },
      None
    ));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleCancelled { actor_id, reason: CancellationReason::ContractReplaced, .. }
        if *actor_id == schedule_id
    )));

    let deactivate_id = create_suspended_system_retry(4);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), deactivate_id));
    let deactivate_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert!(matches!(deactivate_events[0], Event::CycleCancelled { actor_id, reason: CancellationReason::Deactivated, .. } if actor_id == deactivate_id));
    assert!(matches!(deactivate_events[1], Event::CycleSummary { actor_id, .. } if actor_id == deactivate_id));
    assert!(matches!(deactivate_events[2], Event::ActorDeactivated { actor_id } if actor_id == deactivate_id));

    let close_id = create_suspended_system_retry(5);
    let sovereign = sovereign_account(close_id);
    let balance_before = native_balance(&sovereign);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), close_id));
    assert_eq!(native_balance(&sovereign), balance_before);
    let close_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert!(matches!(close_events[0], Event::CycleCancelled { actor_id, reason: CancellationReason::Closing(CloseReason::OwnerInitiated), .. } if actor_id == close_id));
    assert!(matches!(close_events[1], Event::CycleSummary { actor_id, .. } if actor_id == close_id));
    assert!(matches!(close_events[2], Event::ActorClosed { actor_id, reason: CloseReason::OwnerInitiated } if actor_id == close_id));
  });
}

#[test]
fn window_expiry_cancels_while_failure_cutoff_finalizes_before_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let window_id = create_system_with(
      ALICE,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 200,
      },
      Some(ScheduleWindow { start: 1, end: 101 }),
      temporary_retry_swap_plan(),
    );
    fund_native(window_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), window_id));
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(102);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let expiry_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert!(matches!(expiry_events[0], Event::CycleCancelled { actor_id, reason: CancellationReason::Closing(CloseReason::WindowExpired), .. } if actor_id == window_id));
    assert!(matches!(expiry_events[1], Event::CycleSummary { actor_id, .. } if actor_id == window_id));
    assert!(matches!(expiry_events[2], Event::ActorClosed { actor_id, reason: CloseReason::WindowExpired } if actor_id == window_id));

    let cutoff_id = create_suspended_system_retry(103);
    frame_system::Pallet::<Test>::set_block_number(104);
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(106);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let cutoff_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert!(matches!(cutoff_events[0], Event::CycleContinued { actor_id, .. } if actor_id == cutoff_id));
    assert!(!cutoff_events.iter().any(|event| matches!(
      event,
      Event::CycleCancelled { actor_id, .. } if *actor_id == cutoff_id
    )));
    let summary_index = cutoff_events
      .iter()
      .position(|event| matches!(event, Event::CycleSummary { actor_id, result: CycleResult::Failed, .. } if *actor_id == cutoff_id))
      .expect("terminal summary");
    let close_index = cutoff_events
      .iter()
      .position(|event| matches!(event, Event::ActorClosed { actor_id, reason: CloseReason::ConsecutiveFailures } if *actor_id == cutoff_id))
      .expect("terminal close");
    assert!(summary_index < close_index);
  });
}

#[test]
fn cancellation_is_mutable_only() {
  new_test_ext().execute_with(|| {
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Immutable,
      system_active_contract(timer_schedule(1), None, inert_contract_steps()),
    ));
    assert_noop!(
      Actors::cancel_continuation(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ImmutableActor
    );
  });
}

#[test]
fn user_immutable_manual_source_changes_readiness_without_mutating_contract() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Immutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    fund_native(actor_id, 1_000);
    let contract_before = ActorContracts::<Test>::get(actor_id).expect("immutable contract");
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(Actors::pending_signal(actor_id));
    assert_eq!(ActorContracts::<Test>::get(actor_id), Some(contract_before));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("immutable actor remains")
        .cycle_nonce,
      1
    );
  });
}

#[test]
fn immutable_actor_rejects_pause_and_update_contract() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Immutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ImmutableActor
    );
    let replacement = transfer_contract_steps(CHARLIE, 1);
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        replacement,
        crate::CompletionPolicy::Persistent,
      ),
      Error::<Test>::ImmutableActor
    );
  });
}

#[test]
fn user_actor_rejects_mint_task_on_create() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Mint {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    }));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, contract_steps),
      ),
      Error::<Test>::MintNotAllowedForUserActor
    );
  });
}

#[test]
fn update_contract_prunes_stale_funding_accumulators() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let initial_contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      initial_contract_steps,
    );
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      100
    ));
    assert!(
      actor_funding(actor_id)
        .funding_accumulated
        .contains_key(&TestAsset::Native)
    );
    let replacement = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(1),
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      replacement,
      crate::CompletionPolicy::Persistent,
    ));
    let funding_after = actor_funding(actor_id);
    assert!(
      !funding_after
        .funding_accumulated
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
fn update_contract_rejects_mint_for_user_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let replacement = contract_steps_with_step(make_step(Task::Mint {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    }));
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        replacement,
        crate::CompletionPolicy::Persistent,
      ),
      Error::<Test>::MintNotAllowedForUserActor
    );
  });
}

#[test]
fn permissionless_sweep_closes_user_below_min_balance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let prefunded = user_prefunding_requirement(&transfer_contract_steps(BOB, 1));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    deplete_user_sovereign(actor_id, prefunded);
    assert_ok!(Actors::permissionless_sweep(
      RuntimeOrigin::signed(BOB),
      actor_id
    ));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::BalanceExhausted,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn permissionless_sweep_is_lifecycle_touchpoint_only_under_breaker() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    assert_ok!(Actors::permissionless_sweep(
      RuntimeOrigin::signed(BOB),
      actor_id
    ));
    let instance = Actors::active_actor_view(actor_id).expect("system Actors remains alive");
    assert_eq!(instance.cycle_nonce, 0);
    assert_eq!(Actors::queue_head(), Actors::queue_tail());
  });
}

#[test]
fn permissionless_sweep_many_closes_multiple_and_reports_counts() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let user_a_prefunded = user_prefunding_requirement(&transfer_contract_steps(BOB, 1));
    let user_b_prefunded = user_prefunding_requirement(&transfer_contract_steps(ALICE, 1));
    let user_a = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let user_b = create_user_with(
      BOB,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(ALICE, 1),
    );
    deplete_user_sovereign(user_a, user_a_prefunded);
    deplete_user_sovereign(user_b, user_b_prefunded);
    let system_alive = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let sweep_ids: BoundedVec<u64, <Test as crate::Config>::MaxSweepBatch> =
      BoundedVec::try_from(vec![user_a, user_b, system_alive]).expect("batch fits");
    assert_ok!(Actors::permissionless_sweep_many(
      RuntimeOrigin::signed(CHARLIE),
      sweep_ids,
    ));
    assert!(Actors::active_actor_view(user_a).is_none());
    assert!(Actors::active_actor_view(user_b).is_none());
    assert!(Actors::active_actor_view(system_alive).is_some());
    assert!(has_actor_event(|event| {
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
fn permissionless_sweep_many_rolls_back_prior_closes_on_late_failure() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let user_a_prefunded = user_prefunding_requirement(&transfer_contract_steps(BOB, 1));
    let user_b_prefunded = user_prefunding_requirement(&transfer_contract_steps(ALICE, 1));
    let user_a = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let user_b = create_user_with(
      BOB,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(ALICE, 1),
    );
    deplete_user_sovereign(user_a, user_a_prefunded);
    deplete_user_sovereign(user_b, user_b_prefunded);
    OwnerSlotBitmaps::<Test>::remove(BOB);
    let sweep_ids: BoundedVec<u64, <Test as crate::Config>::MaxSweepBatch> =
      BoundedVec::try_from(vec![user_a, user_b]).expect("batch fits");
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();

    assert_noop!(
      Actors::permissionless_sweep_many(RuntimeOrigin::signed(CHARLIE), sweep_ids),
      Error::<Test>::InvalidOwnerSlot
    );
    assert!(Actors::active_actor_view(user_a).is_some());
    assert!(Actors::active_actor_view(user_b).is_some());
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn permissionless_sweep_many_ignores_missing_ids() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let prefunded = user_prefunding_requirement(&transfer_contract_steps(BOB, 1));
    let user_a = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    deplete_user_sovereign(user_a, prefunded);
    let missing_id = user_a.saturating_add(10_000);
    let sweep_ids: BoundedVec<u64, <Test as crate::Config>::MaxSweepBatch> =
      BoundedVec::try_from(vec![user_a, missing_id]).expect("batch fits");
    assert_ok!(Actors::permissionless_sweep_many(
      RuntimeOrigin::signed(CHARLIE),
      sweep_ids,
    ));
    assert!(Actors::active_actor_view(user_a).is_none());
    assert!(has_actor_event(|event| {
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
fn tiny_percentage_amount_is_skipped_without_contract_steps_failure() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfCurrent(Perbill::from_parts(1)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let inst = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(inst.unsuccessful_attempt_streak, 0);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::ResolutionSkipped,
          ..
        } if *id == actor_id
      )
    }));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          cycle_nonce: 1,
          result: CycleResult::Completed,
          outcomes: OutcomeTotals {
            executed_steps: 0,
            committed_effectful_tasks: 0,
            precondition_skips: 0,
            skipped_resolution: 1,
            skipped_funding_unavailable: 0,
            failed_steps: 0,
          },
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn user_resolution_skip_charges_only_eval_fee() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfCurrent(Perbill::from_parts(1)),
    }));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 1_000);
    let before = native_balance(&actor);
    clear_fee_collections();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let after = native_balance(&actor);
    assert_eq!(after, before.saturating_sub(Actors::compute_eval_fee(0)));
    assert_eq!(fee_collections(), vec![Actors::compute_eval_fee(0)]);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::ResolutionSkipped,
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn condition_skip_charges_one_evaluation_fee() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: Balance::MAX,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(step),
    );
    fund_native(actor_id, 1_000);
    clear_fee_collections();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(fee_collections(), vec![Actors::compute_eval_fee(1)]);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepSkipped {
        actor_id: id,
        reason: StepSkippedReason::PreconditionFalse,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn balance_conditions_never_read_staking_share_surfaces() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let plan = contract_steps_with_step(StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 0,
      }]),
      task: Task::StopCycle,
      on_error: StepErrorPolicy::AbortCycle,
    });
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 10);

    assert_eq!(staking_share_balance_reads(), 0);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(staking_share_balance_reads(), 0);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        result: CycleResult::Completed,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn true_balance_condition_can_precede_funding_unavailable_resolution() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let task = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(StepOf::<Test> {
        precondition: all_conditions(vec![Predicate::BalanceAbove {
          asset: TestAsset::Native,
          threshold: 1,
        }]),
        task,
        on_error: StepErrorPolicy::ContinueNextStep,
      }),
    );
    fund_native(actor_id, 1_000);
    clear_fee_collections();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(fee_collections(), vec![Actors::compute_eval_fee(1)]);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepSkipped {
        actor_id: id,
        reason: StepSkippedReason::FundingUnavailable,
        ..
      } if *id == actor_id
    )));
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
    let contract_steps = contract_steps_with_step(make_step(task.clone()));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 1_000);
    let actor_before = native_balance(&actor);
    let fee_sink_before = native_balance(&TestFeeSink::get());
    let task_weight = Actors::weight_upper_bound(&task);
    assert!(task_weight.ref_time() > 0);
    let expected_fee =
      Actors::compute_eval_fee(0).saturating_add(TestWeightToFee::weight_to_fee(&task_weight));
    let instance = Actors::active_actor_view(actor_id).expect("user actor");
    assert_eq!(Actors::attempt_fee_upper_bound(&instance, 0), expected_fee);
    clear_fee_collections();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&actor),
      actor_before.saturating_sub(expected_fee).saturating_sub(1)
    );
    assert_eq!(
      native_balance(&TestFeeSink::get()),
      fee_sink_before.saturating_add(expected_fee)
    );
    assert_eq!(fee_collections(), vec![expected_fee]);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          outcomes: OutcomeTotals {
            executed_steps: 1,
            precondition_skips: 0,
            skipped_resolution: 0,
            skipped_funding_unavailable: 0,
            failed_steps: 0,
            ..
          },
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn adapter_failure_retains_one_combined_step_fee() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let foreign = TestAsset::Local(77);
    let pool_account: AccountId = u64::MAX;
    setup_pool(TestAsset::Native, foreign, 10_000, 10_000);
    fund_native_raw(&pool_account, 10_000);
    set_asset_balance(&pool_account, foreign, 10_000);
    let task = Task::SwapIn {
      asset_in: TestAsset::Native,
      asset_out: foreign,
      amount_in: AmountResolution::Fixed(100),
      slippage_tolerance: Perbill::from_percent(10),
    };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(task.clone())),
    );
    let actor = sovereign_account(actor_id);
    fund_native_raw(&actor, 1_000);
    let actor_before = native_balance(&actor);
    let pool_before = native_balance(&pool_account);
    let expected = Actors::compute_eval_fee(0).saturating_add(TestWeightToFee::weight_to_fee(
      &Actors::weight_upper_bound(&task),
    ));
    clear_fee_collections();
    set_fail_dex_after_input_transfer(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    set_fail_dex_after_input_transfer(false);
    assert_eq!(fee_collections(), vec![expected]);
    assert_eq!(
      native_balance(&actor),
      actor_before.saturating_sub(expected)
    );
    assert_eq!(native_balance(&pool_account), pool_before);
    assert!(has_actor_event(|event| {
      matches!(event, Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id)
    }));
  });
}

#[test]
fn cycle_summary_tracks_step_outcomes() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step_conditions = all_conditions(vec![Predicate::BalanceAbove {
      asset: TestAsset::Native,
      threshold: 1_000,
    }]);
    let contract_steps = BoundedVec::try_from(vec![
      StepOf::<Test> {
        precondition: step_conditions,
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
        precondition: None,
        task: Task::SwapIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::Fixed(10),
          slippage_tolerance: Perbill::one(),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
    ])
    .expect("contract_steps must fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          cycle_nonce: 1,
          result: CycleResult::Completed,
          outcomes: OutcomeTotals {
            executed_steps: 1,
            committed_effectful_tasks: 1,
            precondition_skips: 1,
            skipped_resolution: 1,
            skipped_funding_unavailable: 0,
            failed_steps: 1,
          },
        } if *id == actor_id
      )
    }));
    let last_actor_event = frame_system::Pallet::<Test>::events()
      .into_iter()
      .rev()
      .find_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .expect("Actors event stream must not be empty");
    assert!(matches!(
      last_actor_event,
      Event::CycleSummary { actor_id: id, cycle_nonce: 1, .. } if id == actor_id
    ));
  });
}

#[test]
fn cycle_success_predicate_drives_failure_reset_auto_close_and_event_order() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let failing_step = |on_error| StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
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
      maybe.as_mut().expect("actor hot state exists").unsuccessful_attempt_streak = 2;
    });
    assert_ok!(replace_auto_close(
      RuntimeOrigin::root(),
      continue_id,
      Some(2)
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      continue_id
    ));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let after_first = Actors::active_actor_view(continue_id).expect("successful actor remains active");
    assert_eq!(after_first.cycle_nonce, 1);
    assert_eq!(after_first.unsuccessful_attempt_streak, 0);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      continue_id
    ));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(continue_id).is_none());
    let continue_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(continue_events.len(), 5);
    assert!(matches!(continue_events[0], Event::CycleStarted { actor_id, .. } if actor_id == continue_id));
    assert!(matches!(continue_events[1], Event::StepFailed { actor_id, step_index: 0, .. } if actor_id == continue_id));
    assert!(matches!(continue_events[2], Event::StepFailed { actor_id, step_index: 1, .. } if actor_id == continue_id));
    assert!(matches!(continue_events[3], Event::CycleSummary { actor_id, result: CycleResult::Completed, outcomes: OutcomeTotals { failed_steps: 2, .. }, .. } if actor_id == continue_id));
    assert!(matches!(continue_events[4], Event::ActorClosed { actor_id, reason: CloseReason::AutoCloseNonceReached } if actor_id == continue_id));
    let skip_step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 1,
      }]),
      task: Task::Stake {
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let skip_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![skip_step]).expect("one step fits"),
    );
    assert_ok!(replace_auto_close(
      RuntimeOrigin::root(),
      skip_id,
      Some(1)
    ));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), skip_id));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(skip_id).is_none());
    let skip_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(skip_events.len(), 4);
    assert!(matches!(skip_events[0], Event::CycleStarted { actor_id, .. } if actor_id == skip_id));
    assert!(matches!(skip_events[1], Event::StepSkipped { actor_id, step_index: 0, .. } if actor_id == skip_id));
    assert!(matches!(skip_events[2], Event::CycleSummary { actor_id, result: CycleResult::Completed, outcomes: OutcomeTotals { precondition_skips: 1, failed_steps: 0, .. }, .. } if actor_id == skip_id));
    assert!(matches!(skip_events[3], Event::ActorClosed { actor_id, reason: CloseReason::AutoCloseNonceReached } if actor_id == skip_id));
    let abort_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![
        failing_step(StepErrorPolicy::AbortCycle),
        make_step(Task::Stake {
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(1),
        }),
      ])
      .expect("two steps fit"),
    );
    fund_native(abort_id, 100);
    assert_ok!(replace_auto_close(
      RuntimeOrigin::root(),
      abort_id,
      Some(1)
    ));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), abort_id));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let abort_instance = Actors::active_actor_view(abort_id).expect("aborted actor remains active");
    assert_eq!(abort_instance.unsuccessful_attempt_streak, 1);
    let abort_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(abort_events.len(), 3);
    assert!(matches!(abort_events[0], Event::CycleStarted { actor_id, .. } if actor_id == abort_id));
    assert!(matches!(abort_events[1], Event::StepFailed { actor_id, step_index: 0, .. } if actor_id == abort_id));
    assert!(matches!(abort_events[2], Event::CycleSummary { actor_id, result: CycleResult::Failed, outcomes: OutcomeTotals { executed_steps: 0, failed_steps: 1, .. }, .. } if actor_id == abort_id));
    let close_only_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), close_only_id));
    let close_events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(close_events.len(), 1);
    assert!(matches!(close_events[0], Event::ActorClosed { actor_id, reason: CloseReason::OwnerInitiated } if actor_id == close_only_id));
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
    let eval_fee = Actors::compute_eval_fee(0);
    for (idx, (funding, pct, expect_skip)) in cases.into_iter().enumerate() {
      let task = Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfCurrent(pct),
      };
      let contract_steps = contract_steps_with_step(make_step(task.clone()));
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        manual_schedule(),
        None,
        contract_steps,
      );
      fund_native(actor_id, funding);
      let fee_sink_before = native_balance(&TestFeeSink::get());
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      run_idle(Weight::MAX);
      let fee_sink_after = native_balance(&TestFeeSink::get());
      let fee_delta = fee_sink_after.saturating_sub(fee_sink_before);
      let summary = frame_system::Pallet::<Test>::events()
        .into_iter()
        .rev()
        .find_map(|record| match record.event {
          RuntimeEvent::Actors(Event::CycleSummary {
            actor_id: id,
            outcomes,
            ..
          }) if id == actor_id => Some((
            outcomes.executed_steps,
            outcomes.precondition_skips,
            outcomes.skipped_resolution,
            outcomes.skipped_funding_unavailable,
            outcomes.failed_steps,
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
          &Actors::weight_upper_bound(&task),
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
fn percentage_at_opening_uses_cycle_start_snapshot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      }),
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageAtOpening(Perbill::from_percent(50)),
      }),
    ])
    .expect("contract_steps fits");
    let actor_id = create_system_with(ALICE, percentage_trigger_schedule(), None, contract_steps);
    fund_native(actor_id, 101);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    let actor = sovereign_account(actor_id);
    let actor_before = native_balance(&actor);
    signal_percentage_trigger(actor_id, TestAsset::Native);
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(native_balance(&CHARLIE), charlie_before.saturating_add(50));
    assert_eq!(native_balance(&actor), actor_before.saturating_sub(100));
  });
}

#[test]
fn percentage_at_opening_uses_preservable_native_snapshot_for_user() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let task = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageAtOpening(Perbill::one()),
    };
    let contract_steps = contract_steps_with_step(make_step(task.clone()));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      percentage_trigger_schedule(),
      None,
      contract_steps,
    );
    let actor = sovereign_account(actor_id);
    let funding = 500;
    fund_native(actor_id, funding);
    let expected_fees = Actors::compute_eval_fee(0).saturating_add(
      <TestWeightToFee as polkadot_sdk::sp_weights::WeightToFee>::weight_to_fee(
        &Actors::weight_upper_bound(&task),
      ),
    );
    let actor_before = native_balance(&actor);
    let expected_transfer = actor_before
      .saturating_sub(expected_fees)
      .saturating_sub(TestMinUserBalance::get());
    let bob_before = native_balance(&BOB);
    signal_percentage_trigger(actor_id, TestAsset::Native);
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&BOB),
      bob_before.saturating_add(expected_transfer)
    );
    assert_eq!(native_balance(&actor), TestMinUserBalance::get());
    assert!(expected_transfer > 0);
  });
}

#[test]
fn percentage_of_last_funding_consumes_each_cycle_open_snapshot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let bob_before = native_balance(&BOB);
    let actor = sovereign_account(actor_id);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      100
    ));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&100)
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 1);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(native_balance(&actor), 50);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(CHARLIE),
      actor_id,
      TestAsset::Native,
      200
    ));
    assert_eq!(native_balance(&actor), 250);
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&200)
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 2);
    let inst = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(inst.cycle_nonce, 2);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(150));
    assert!(actor_funding(actor_id).funding_accumulated.is_empty());
  });
}

#[test]
fn system_keeps_running_on_last_funding_exhaustion_and_accepts_refill() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      100
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 1);
    assert_eq!(native_balance(&actor), 50);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 2);
    assert_eq!(native_balance(&actor), 50);
    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 3);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == actor_id
      )
    }));
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(instance.lifecycle, ActiveLifecycle::Active);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(CHARLIE),
      actor_id,
      TestAsset::Native,
      80
    ));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&80)
    );
  });
}

#[test]
fn user_closes_when_the_full_fee_envelope_cannot_fit_above_the_floor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      500
    ));
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    // After the first run transfers 250 and charges the fee envelope (~101), the
    // remaining balance cannot fit the full envelope above MinUserBalance, so the
    // second run is NOT admitted and the actor closes with FeeBudgetExhausted
    // instead of running with a step skip (spec 5.2.1).
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::FeeBudgetExhausted,
          ..
        } if *id == actor_id
      )
    }));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(250));
  });
}

#[test]
fn create_accepts_swap_in_with_slippage_tolerance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::SwapIn {
      asset_in: TestAsset::Native,
      asset_out: TestAsset::Local(1),
      amount_in: AmountResolution::Fixed(10),
      slippage_tolerance: Perbill::from_percent(5),
    }));
    prefund_active_user_creation(ALICE, &contract_steps);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, contract_steps),
    ));
  });
}

#[test]
fn dex_adapter_receives_authoritative_actor_type() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      temporary_retry_swap_plan(),
    );
    fund_native(actor_id, 1_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(last_dex_actor_type(), Some(ActorType::User));
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, temporary_retry_swap_plan());
    fund_native(actor_id, 1_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(last_dex_actor_type(), Some(ActorType::System));
  });
}

#[test]
fn full_slippage_cannot_accept_zero_swap_output() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_out = TestAsset::Local(91);
    setup_pool(TestAsset::Native, asset_out, 1_000_000, 1);
    set_asset_balance(&u64::MAX, asset_out, 1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out,
        amount_in: AmountResolution::Fixed(1),
        slippage_tolerance: Perbill::one(),
      })),
    );
    fund_native(actor_id, 10);
    frame_system::Pallet::<Test>::reset_events();

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert!(!has_actor_event(|event| matches!(
      event,
      Event::SwapExecuted { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepFailed {
        actor_id: id,
        retry_class: RetryClass::Permanent,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn create_rejects_swap_out_with_zero_input_cap() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::SwapOut {
      asset_out: TestAsset::Local(2),
      amount_out: AmountResolution::Fixed(10),
      asset_in: TestAsset::Local(1),
      input_limit: InputLimit::Absolute(0),
      slippage_tolerance: Perbill::zero(),
    }));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, contract_steps),
      ),
      Error::<Test>::InvalidTradeBound
    );
  });
}

#[test]
fn create_rejects_zero_liquidity_output_bounds() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let add_plan = contract_steps_with_step(make_step(Task::AddLiquidity {
      asset_a: TestAsset::Local(1),
      asset_b: TestAsset::Local(2),
      amount_a: AmountResolution::Fixed(10),
      amount_b: AmountResolution::Fixed(10),
      min_lp_out: 0,
    }));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, add_plan),
      ),
      Error::<Test>::InvalidTradeBound
    );

    let remove_plan = contract_steps_with_step(make_step(Task::RemoveLiquidity {
      lp_asset: TestAsset::Local(3),
      asset_a: TestAsset::Local(3),
      asset_b: TestAsset::Local(3),
      lp_amount: AmountResolution::Fixed(10),
      min_amount_a: 1,
      min_amount_b: 0,
    }));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, remove_plan),
      ),
      Error::<Test>::InvalidTradeBound
    );
  });
}

#[test]
fn liquidity_tasks_fail_before_effects_when_output_minima_are_unmet() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let add_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::AddLiquidity {
        asset_a: TestAsset::Local(1),
        asset_b: TestAsset::Local(2),
        amount_a: AmountResolution::Fixed(4),
        amount_b: AmountResolution::Fixed(4),
        min_lp_out: 5,
      })),
    );
    fund_native(add_id, 1_000);
    let add_actor = sovereign_account(add_id);
    set_asset_balance(&add_actor, TestAsset::Local(1), 10);
    set_asset_balance(&add_actor, TestAsset::Local(2), 10);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), add_id));
    run_idle(Weight::MAX);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::LiquidityAdded { actor_id, .. } if *actor_id == add_id
    )));

    let remove_id = create_user_with(
      BOB,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::RemoveLiquidity {
        lp_asset: TestAsset::Local(3),
        asset_a: TestAsset::Local(3),
        asset_b: TestAsset::Local(3),
        lp_amount: AmountResolution::Fixed(10),
        min_amount_a: 6,
        min_amount_b: 6,
      })),
    );
    fund_native(remove_id, 1_000);
    let remove_actor = sovereign_account(remove_id);
    set_asset_balance(&remove_actor, TestAsset::Local(3), 20);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(BOB),
      remove_id
    ));
    run_idle(Weight::MAX);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::LiquidityRemoved { actor_id, .. } if *actor_id == remove_id
    )));
  });
}

#[test]
fn market_tasks_dispatch_their_resolved_task_local_amounts_without_a_system_cap() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let swap_in = TestAsset::Local(81);
    let swap_out = TestAsset::Local(82);
    set_pool_reserves(swap_in, swap_out, 1_000, 1_000);
    set_asset_balance(&u64::MAX, swap_out, 1_000);
    let plan = BoundedVec::try_from(vec![
      make_step(Task::SwapIn {
        asset_in: swap_in,
        asset_out: swap_out,
        amount_in: AmountResolution::Fixed(100),
        slippage_tolerance: Perbill::one(),
      }),
      make_step(Task::AddLiquidity {
        asset_a: TestAsset::Local(83),
        asset_b: TestAsset::Local(84),
        amount_a: AmountResolution::Fixed(100),
        amount_b: AmountResolution::Fixed(100),
        min_lp_out: 1,
      }),
      make_step(Task::RemoveLiquidity {
        lp_asset: TestAsset::Local(85),
        asset_a: TestAsset::Local(83),
        asset_b: TestAsset::Local(84),
        lp_amount: AmountResolution::Fixed(100),
        min_amount_a: 1,
        min_amount_b: 1,
      }),
      make_step(Task::DonateLiquidity {
        asset_a: TestAsset::Local(86),
        asset_b: TestAsset::Local(87),
        max_amount_a: AmountResolution::Fixed(100),
        max_ratio_error: Perbill::zero(),
      }),
    ])
    .expect("four market tasks fit System plan");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 1_000);
    for asset in [
      swap_in,
      TestAsset::Local(83),
      TestAsset::Local(84),
      TestAsset::Local(85),
      TestAsset::Local(86),
      TestAsset::Local(87),
    ] {
      set_asset_balance(&actor, asset, 101);
    }

    // The remove-liquidity step's LP token maps to the ordered pair created by the
    // add-liquidity step; the mock pair registry honors the host-owned binding.
    register_lp_pair(
      TestAsset::Local(85),
      TestAsset::Local(83),
      TestAsset::Local(84),
    );

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::SwapExecuted { actor_id: id, amount_in: 100, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::LiquidityAdded { actor_id: id, lp_minted: 100, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::LiquidityRemoved { actor_id: id, amount_a: 50, amount_b: 50, .. }
        if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::LiquidityDonated { actor_id: id, max_amount_a: 100, amount_a: 100, amount_b: 100, .. }
        if *id == actor_id
    )));
  });
}

#[test]
fn create_accepts_swap_out_with_optional_absolute_cap() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::SwapOut {
      asset_out: TestAsset::Local(2),
      amount_out: AmountResolution::Fixed(10),
      asset_in: TestAsset::Local(1),
      input_limit: InputLimit::Absolute(100),
      slippage_tolerance: Perbill::from_percent(5),
    }));
    prefund_active_user_creation(ALICE, &contract_steps);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, contract_steps),
    ));
  });
}

#[test]
fn swap_out_live_market_mode_uses_preservable_capacity_and_emits_swap_event() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_in = TestAsset::Local(1);
    let asset_out = TestAsset::Local(2);
    set_pool_reserves(asset_in, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::SwapOut {
        asset_out,
        amount_out: AmountResolution::Fixed(100),
        asset_in,
        input_limit: InputLimit::LiveQuote,
        slippage_tolerance: Perbill::from_percent(0),
      })),
    );
    fund_native(actor_id, 10_000);
    let sovereign = sovereign_account(actor_id);
    set_asset_balance(&sovereign, asset_in, 1_000);
    let out_before = asset_balance(&sovereign, asset_out);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let out_after = asset_balance(&sovereign, asset_out);
    assert!(out_after >= out_before.saturating_add(100));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::SwapExecuted {
          actor_id: id,
          asset_in: in_asset,
          asset_out: out_asset,
          amount_in,
          amount_out,
          ..
        } if *id == actor_id
          && *in_asset == asset_in
          && *out_asset == asset_out
          && *amount_in > 0
          && *amount_out >= 100
      )
    }));
  });
}

#[test]
fn swap_out_never_spends_above_explicit_input_cap() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_in = TestAsset::Local(1);
    let asset_out = TestAsset::Local(2);
    set_pool_reserves(asset_in, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::SwapOut {
        asset_out,
        amount_out: AmountResolution::Fixed(100),
        asset_in,
        input_limit: InputLimit::Absolute(50),
        slippage_tolerance: Perbill::zero(),
      })),
    );
    fund_native(actor_id, 10_000);
    let sovereign = sovereign_account(actor_id);
    set_asset_balance(&sovereign, asset_in, 1_000);
    let input_before = asset_balance(&sovereign, asset_in);
    let output_before = asset_balance(&sovereign, asset_out);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(asset_balance(&sovereign, asset_in), input_before);
    assert_eq!(asset_balance(&sovereign, asset_out), output_before);
    assert!(has_actor_event(|event| {
      matches!(event, Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id)
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
    }));
  });
}

#[test]
fn swap_out_absolute_input_is_a_cap_not_a_balance_gate() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_in = TestAsset::Local(1);
    let asset_out = TestAsset::Local(2);
    set_pool_reserves(asset_in, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::SwapOut {
        asset_out,
        amount_out: AmountResolution::Fixed(100),
        asset_in,
        input_limit: InputLimit::Absolute(1_000),
        slippage_tolerance: Perbill::zero(),
      })),
    );
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_in, 200);
    let input_before = asset_balance(&actor, asset_in);
    let output_before = asset_balance(&actor, asset_out);

    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);

    let input_spent = input_before.saturating_sub(asset_balance(&actor, asset_in));
    assert!(input_spent > 0 && input_spent <= 200);
    assert!(asset_balance(&actor, asset_out) >= output_before.saturating_add(100));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::SwapExecuted { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn on_initialize_is_a_zero_weight_noop() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_eq!(Actors::on_initialize(2), Weight::zero());
    let inst = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(inst.cycle_nonce, 0);
    assert!(!has_actor_event(|event| {
      matches!(
        event,
        Event::CycleStarted {
          actor_id: id,
          cycle_nonce: 1,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn zero_on_idle_budget_performs_no_storage_or_telemetry_work() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    IdleStarvationState::<Test>::put(IdleStarvationPhase::Alerted {
      consecutive_blocks: 1,
    });
    let event_count = frame_system::Pallet::<Test>::event_count();
    let used = Actors::on_idle(1, Weight::zero());
    assert_eq!(used, Weight::zero());
    assert_eq!(
      IdleStarvationState::<Test>::get(),
      IdleStarvationPhase::Alerted {
        consecutive_blocks: 1,
      }
    );
    assert_eq!(frame_system::Pallet::<Test>::event_count(), event_count);
  });
}

#[test]
fn starvation_emits_observability_event_once_without_control_effects() {
  new_test_ext().execute_with(|| {
    let threshold = TestMaxIdleStarvationBlocks::get();
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let queue_ticket = Actors::actor_hot(actor_id)
      .expect("queued actor")
      .queue_ticket;
    assert!(!GlobalCircuitBreaker::<Test>::get());
    run_idle(starvation_blocked_budget(actor_id));
    assert_eq!(
      IdleStarvationState::<Test>::get(),
      IdleStarvationPhase::Starving {
        consecutive_blocks: 1,
      }
    );
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::IdleStarvationDetected { .. } | Event::IdleStarvationRecovered { .. }
    )));
    for block in 2..=(threshold + 2) {
      frame_system::Pallet::<Test>::set_block_number(block as u64);
      run_idle(starvation_blocked_budget(actor_id));
    }
    let detections = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(Event::IdleStarvationDetected { consecutive_blocks }) => {
          Some(consecutive_blocks)
        }
        _ => None,
      })
      .collect::<std::vec::Vec<_>>();
    assert_eq!(detections, vec![threshold]);
    assert_eq!(
      IdleStarvationState::<Test>::get(),
      IdleStarvationPhase::Alerted {
        consecutive_blocks: threshold + 2,
      }
    );
    assert!(
      Actors::active_actor_view(actor_id).is_some(),
      "live head survives"
    );
    assert_eq!(
      Actors::actor_hot(actor_id).expect("live head").queue_ticket,
      queue_ticket,
    );
    assert!(!GlobalCircuitBreaker::<Test>::get());
  });
}

#[test]
fn proof_size_exhaustion_counts_as_idle_starvation() {
  new_test_ext().execute_with(|| {
    let threshold = TestMaxIdleStarvationBlocks::get();
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    for block in 1..=threshold {
      frame_system::Pallet::<Test>::set_block_number(u64::from(block));
      run_idle(starvation_blocked_budget(actor_id));
    }
    assert_eq!(
      IdleStarvationState::<Test>::get(),
      IdleStarvationPhase::Alerted {
        consecutive_blocks: threshold,
      }
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::IdleStarvationDetected { consecutive_blocks } if *consecutive_blocks == threshold
    )));
  });
}

#[test]
fn starvation_requires_live_fifo_work_and_clears_without_work() {
  new_test_ext().execute_with(|| {
    let threshold = TestMaxIdleStarvationBlocks::get();
    assert!(!IdleStarvationState::<Test>::exists());
    frame_system::Pallet::<Test>::set_block_number(1);
    run_idle(Weight::MAX);
    assert!(!IdleStarvationState::<Test>::exists());
    // An empty queue with an exhausted budget must never starve: no live FIFO work exists.
    for block in 1..=(threshold + 2) {
      frame_system::Pallet::<Test>::set_block_number(block as u64);
      run_idle(starvation_observation_weight());
    }
    assert!(!IdleStarvationState::<Test>::exists());
  });
}

#[test]
fn starvation_recovery_is_observable_once_and_healthy_idle_stays_sparse() {
  new_test_ext().execute_with(|| {
    let threshold = TestMaxIdleStarvationBlocks::get();
    assert!(!IdleStarvationState::<Test>::exists());
    frame_system::Pallet::<Test>::set_block_number(1);
    run_idle(Weight::MAX);
    assert!(!IdleStarvationState::<Test>::exists());
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    for block in 2..=(threshold + 1) {
      frame_system::Pallet::<Test>::set_block_number(block as u64);
      run_idle(starvation_blocked_budget(actor_id));
    }
    assert_eq!(
      IdleStarvationState::<Test>::get(),
      IdleStarvationPhase::Alerted {
        consecutive_blocks: threshold,
      }
    );
    frame_system::Pallet::<Test>::set_block_number(threshold.saturating_add(2) as u64);
    run_idle(Weight::MAX);
    assert!(!IdleStarvationState::<Test>::exists());
    let recoveries = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(Event::IdleStarvationRecovered { consecutive_blocks }) => {
          Some(consecutive_blocks)
        }
        _ => None,
      })
      .collect::<std::vec::Vec<_>>();
    assert_eq!(recoveries, vec![threshold]);
    frame_system::Pallet::<Test>::set_block_number(threshold.saturating_add(3) as u64);
    run_idle(Weight::MAX);
    assert!(!IdleStarvationState::<Test>::exists());
    assert_eq!(
      frame_system::Pallet::<Test>::events()
        .into_iter()
        .filter(|record| matches!(
          record.event,
          RuntimeEvent::Actors(Event::IdleStarvationRecovered { .. })
        ))
        .count(),
      1
    );
  });
}

#[test]
fn breaker_freezes_starvation_count_without_recovery_event() {
  new_test_ext().execute_with(|| {
    let threshold = TestMaxIdleStarvationBlocks::get();
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    for block in 1..=threshold {
      frame_system::Pallet::<Test>::set_block_number(block as u64);
      run_idle(starvation_blocked_budget(actor_id));
    }
    GlobalCircuitBreaker::<Test>::put(true);
    frame_system::Pallet::<Test>::set_block_number(threshold.saturating_add(1) as u64);
    run_idle(starvation_blocked_budget(actor_id));
    assert_eq!(
      IdleStarvationState::<Test>::get(),
      IdleStarvationPhase::Alerted {
        consecutive_blocks: threshold,
      }
    );
    let recovery_count = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter(|record| {
        matches!(
          record.event,
          RuntimeEvent::Actors(Event::IdleStarvationRecovered { .. })
        )
      })
      .count();
    frame_system::Pallet::<Test>::set_block_number(threshold.saturating_add(2) as u64);
    run_idle(starvation_blocked_budget(actor_id));
    assert_eq!(
      IdleStarvationState::<Test>::get(),
      IdleStarvationPhase::Alerted {
        consecutive_blocks: threshold,
      }
    );
    assert_eq!(
      frame_system::Pallet::<Test>::events()
        .into_iter()
        .filter(|record| matches!(
          record.event,
          RuntimeEvent::Actors(Event::IdleStarvationRecovered { .. })
        ))
        .count(),
      recovery_count
    );
  });
}

#[test]
fn breaker_keeps_explicit_repair_sweep_operational() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let prefunded = user_prefunding_requirement(&transfer_contract_steps(BOB, 1));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    deplete_user_sovereign(actor_id, prefunded);
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    assert_ok!(Actors::permissionless_sweep(
      RuntimeOrigin::signed(BOB),
      actor_id,
    ));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::BalanceExhausted,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn breaker_defers_scheduler_owned_fee_budget_close_without_partial_terminal_events() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let prefunded = user_prefunding_requirement(&transfer_contract_steps(BOB, 1));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    deplete_user_sovereign(actor_id, prefunded);
    fund_native(actor_id, 60);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let instance = Actors::active_actor_view(actor_id).expect("breaker keeps actor pending");
    assert_eq!(instance.cycle_nonce, 0);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. }
        | Event::CycleSummary { actor_id: id, .. }
        | Event::ActorClosed { actor_id: id, .. }
        if *id == actor_id
    )));
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      false
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::FeeBudgetExhausted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn breaker_defers_scheduler_owned_window_expiry_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      transfer_contract_steps(BOB, 1),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    frame_system::Pallet::<Test>::set_block_number(102);
    frame_system::Pallet::<Test>::reset_events();
    let _ = Actors::execute_cycle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActorClosed { actor_id: id, .. } if *id == actor_id
    )));
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      false
    ));
    frame_system::Pallet::<Test>::set_block_number(103);
    frame_system::Pallet::<Test>::reset_events();
    let _ = Actors::execute_cycle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::WindowExpired,
      } if *id == actor_id
    )));
  });
}

#[test]
fn default_funding_policies_authorize_system_runtime_sources_but_only_user_owner() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps_sys = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let contract_steps_usr = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let system_actor = create_system_with(ALICE, manual_schedule(), None, contract_steps_sys);
    let user_actor = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_usr,
    );
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      system_actor,
      TestAsset::Native,
      100
    ));
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      user_actor,
      TestAsset::Native,
      100
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::notify_address_event(
      system_actor,
      TestAsset::Native,
      500,
      &CHARLIE
    ));
    assert_ok!(Actors::notify_address_event(
      user_actor,
      TestAsset::Native,
      500,
      &CHARLIE
    ));
    assert_ok!(Actors::notify_address_event(
      user_actor,
      TestAsset::Native,
      25,
      &ALICE
    ));
    assert_ok!(Actors::notify_internal_address_event(
      user_actor,
      TestAsset::Native,
      30,
      &ALICE
    ));
    assert_ok!(Actors::notify_xcm_address_event(
      user_actor,
      TestAsset::Native,
      35,
      &ALICE
    ));
    let sys_inst = actor_funding(system_actor);
    assert!(matches!(
      ActorContracts::<Test>::get(system_actor)
        .expect("system Actor Contract")
        .funding,
      FundingSourcePolicy::RuntimePolicy
    ));
    assert_eq!(
      sys_inst.funding_accumulated.get(&TestAsset::Native),
      Some(&600)
    );
    let user_inst = actor_funding(user_actor);
    assert!(matches!(
      ActorContracts::<Test>::get(user_actor)
        .expect("user Actor Contract")
        .funding,
      FundingSourcePolicy::OwnerOnly
    ));
    assert_eq!(
      user_inst.funding_accumulated.get(&TestAsset::Native),
      Some(&125)
    );
  });
}

#[test]
fn any_verified_ingress_accepts_each_verified_context_field_but_not_all_none() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    assert_ok!(Actors::notify_internal_address_event(
      actor_id,
      TestAsset::Native,
      40,
      &CHARLIE
    ));
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      30,
      &BOB
    ));
    assert_ok!(Actors::notify_xcm_address_event(
      actor_id,
      TestAsset::Native,
      20,
      &CHARLIE
    ));
    assert_ok!(Actors::notify_address_event_without_source(
      actor_id,
      TestAsset::Native,
      1_000
    ));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&90)
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ContractUpdated { actor_id: id } if *id == actor_id
    )));

    crate::ActorFunding::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("funding state")
        .funding_accumulated
        .get_mut(&TestAsset::Native)
        .map(|accumulated| *accumulated = u128::MAX);
    });
    assert_noop!(
      Actors::preflight_funding_event(actor_id, TestAsset::Native, 1, Some(&ALICE), None,),
      Error::<Test>::FundingAccumulatorOverflow
    );
    assert_noop!(
      Actors::preflight_funding_event(
        actor_id,
        TestAsset::Native,
        1,
        None,
        Some(&crate::FundingProvenance::Xcm),
      ),
      Error::<Test>::FundingAccumulatorOverflow
    );
    assert_ok!(Actors::preflight_funding_event(
      actor_id,
      TestAsset::Native,
      1,
      None,
      None,
    ));
  });
}

#[test]
fn any_verified_ingress_third_party_shapes_basis_only_with_real_delivered_value() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
      })),
    );
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress,
    ));
    let actor = sovereign_account(actor_id);
    let delivered = 100;
    let actor_before = native_balance(&actor);
    let bob_before = native_balance(&BOB);
    assert_ok!(MockAssetOps::transfer(
      &CHARLIE,
      &actor,
      TestAsset::Native,
      delivered,
    ));
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      delivered,
      &CHARLIE,
    ));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&delivered),
    );

    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);

    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(native_balance(&actor), actor_before.saturating_add(50));
  });
}

#[test]
fn trigger_source_and_funding_provenance_are_evaluated_independently() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::notify_internal_address_event(
      actor_id,
      TestAsset::Native,
      25,
      &ALICE,
    ));
    assert!(
      Actors::actor_hot(actor_id)
        .expect("actor hot state")
        .pending_signal,
      "verified source must satisfy trigger filtering independently"
    );
    assert!(
      actor_funding(actor_id).funding_accumulated.is_empty(),
      "InternalProtocol provenance must not satisfy OwnerOnly funding"
    );
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::FundingAccumulated { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn untracked_credit_can_trigger_without_allocating_funding_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    frame_system::Pallet::<Test>::reset_events();

    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Local(99),
      25,
      &ALICE,
    ));

    assert!(Actors::actor_hot(actor_id).is_some_and(|hot| hot.pending_signal));
    assert!(actor_funding(actor_id).funding_accumulated.is_empty());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::FundingAccumulated { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn one_ingress_matching_the_single_source_mutates_funding_once() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let trigger = Trigger::address_event(SourceFilter::Any, AssetFilter::Any);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      Schedule {
        trigger,
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );

    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE
    ));

    let hot = Actors::actor_hot(actor_id).expect("actor hot state");
    assert!(hot.pending_signal);
    assert!(hot.queue_ticket.is_some());
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&100)
    );
    assert_eq!(
      frame_system::Pallet::<Test>::events()
        .into_iter()
        .filter(|record| matches!(
          record.event,
          RuntimeEvent::Actors(Event::FundingAccumulated { actor_id: id, .. }) if id == actor_id
        ))
        .count(),
      1
    );
  });
}

#[test]
fn identical_authoritative_transfers_remain_distinct_funding_events() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    let first_ticket = Actors::actor_hot(actor_id)
      .expect("actor hot state")
      .queue_ticket;
    assert!(first_ticket.is_some());
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    assert_eq!(
      Actors::actor_hot(actor_id)
        .expect("actor hot state")
        .queue_ticket,
      first_ticket,
      "an already-pending signal must retain one live FIFO ticket"
    );
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&200)
    );
  });
}

#[test]
fn direct_notification_reports_funding_overflow_without_partial_readiness_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    crate::ActorFunding::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("user actor funding")
        .funding_accumulated
        .try_insert(TestAsset::Native, u128::MAX)
        .expect("funding accumulator fits");
    });
    assert_noop!(
      Actors::notify_address_event(actor_id, TestAsset::Native, 1, &ALICE),
      Error::<Test>::FundingAccumulatorOverflow
    );
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&u128::MAX)
    );
    assert!(!Actors::actor_hot(actor_id).is_some_and(|hot| hot.pending_signal));
  });
}

#[test]
fn signed_allowlist_accepts_only_verified_listed_signers_for_funding() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    let allowed = BoundedBTreeSet::try_from([CHARLIE].into_iter().collect::<BTreeSet<_>>())
      .expect("one funding signer fits");
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      FundingSourcePolicy::SignedAllowlist(allowed),
    ));
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &CHARLIE
    ));
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      900,
      &BOB
    ));
    assert_ok!(Actors::notify_internal_address_event(
      actor_id,
      TestAsset::Native,
      700,
      &CHARLIE
    ));
    assert_ok!(Actors::notify_address_event_without_source(
      actor_id,
      TestAsset::Native,
      500
    ));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&100)
    );
  });
}

#[test]
fn immutable_user_cannot_update_contract_funding() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Immutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        FundingSourcePolicy::AnyVerifiedIngress
      ),
      Error::<Test>::ImmutableActor
    );
  });
}

#[test]
fn notify_address_event_accumulates_without_pause_resume_cycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(100)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      100
    ));
    assert_eq!(native_balance(&actor), 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 1);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 2);
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(instance.lifecycle, ActiveLifecycle::Active);
    frame_system::Pallet::<Test>::set_block_number(3);
    fund_native(actor_id, 500);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      500,
      &CHARLIE
    ));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&500)
    );
    assert!(!has_actor_event(|event| {
      matches!(event, Event::ActorResumed { actor_id: id } if *id == actor_id)
    }));
  });
}

#[test]
fn ordinary_transfer_updates_accumulator_without_resuming_paused_system_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(100)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    ActorHot::<Test>::mutate(actor_id, |maybe| {
      let hot = maybe.as_mut().expect("Actors hot state exists");
      hot.lifecycle = ActiveLifecycle::Paused;
    });
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      123
    ));
    let updated = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(updated.lifecycle, ActiveLifecycle::Paused);
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&123)
    );
    assert!(!has_actor_event(|event| {
      matches!(event, Event::ActorResumed { actor_id: id } if *id == actor_id)
    }));
  });
}

#[test]
fn notify_address_event_updates_accumulator_without_resuming_paused_system_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(100)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 500);
    ActorHot::<Test>::mutate(actor_id, |maybe| {
      let hot = maybe.as_mut().expect("Actors hot state exists");
      hot.lifecycle = ActiveLifecycle::Paused;
    });
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      500,
      &CHARLIE
    ));
    let updated = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(updated.lifecycle, ActiveLifecycle::Paused);
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&500)
    );
    assert_eq!(native_balance(&actor), 500);
    assert!(!has_actor_event(|event| {
      matches!(event, Event::ActorResumed { actor_id: id } if *id == actor_id)
    }));
  });
}

#[test]
fn multi_asset_funding_accumulators_are_independent() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // ContractSteps with TWO assets using PercentageOfLastFunding
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
    let contract_steps = BoundedVec::try_from(vec![step1, step2]).expect("contract_steps fits");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    // Verify both assets are tracked
    let inst = actor_funding(actor_id);
    assert!(inst.funding_tracked_assets.contains(&TestAsset::Native));
    assert!(inst.funding_tracked_assets.contains(&TestAsset::Local(1)));
    assert_eq!(inst.funding_tracked_assets.len(), 2);
    // Transfer Native first
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      1000
    ));
    let inst = actor_funding(actor_id);
    assert_eq!(
      inst.funding_accumulated.get(&TestAsset::Native),
      Some(&1000)
    );
    assert!(inst.funding_accumulated.get(&TestAsset::Local(1)).is_none());
    // Fund Local(1) separately
    frame_system::Pallet::<Test>::set_block_number(2);
    <crate::mock::MockAssetOps as crate::adapters::AssetOps<AccountId, TestAsset, Balance>>::mint(
      &ALICE,
      TestAsset::Local(1),
      400,
    )
    .unwrap();
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Local(1),
      400
    ));
    let inst = actor_funding(actor_id);
    assert_eq!(
      inst.funding_accumulated.get(&TestAsset::Native),
      Some(&1000)
    );
    assert_eq!(
      inst.funding_accumulated.get(&TestAsset::Local(1)),
      Some(&400)
    );
    // Another Native transfer accumulates only the Native asset.
    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      500
    ));
    let inst = actor_funding(actor_id);
    assert_eq!(
      inst.funding_accumulated.get(&TestAsset::Native),
      Some(&1500)
    );
    assert_eq!(
      inst.funding_accumulated.get(&TestAsset::Local(1)),
      Some(&400)
    );
  });
}

// --- Error Coverage Tests ---

#[test]
fn actor_not_found_on_nonexistent_id() {
  new_test_ext().execute_with(|| {
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::signed(ALICE), 999),
      Error::<Test>::ActorNotFound
    );
  });
}

#[test]
fn actor_id_overflow_at_max() {
  new_test_ext().execute_with(|| {
    NextActorId::<Test>::put(u64::MAX);
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 10)),
      ),
      Error::<Test>::ActorIdOverflow
    );
  });
}

#[test]
fn empty_contract_steps_rejected() {
  new_test_ext().execute_with(|| {
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, BoundedVec::default()),
      ),
      Error::<Test>::EmptyContractSteps
    );
  });
}

#[test]
fn contract_steps_hard_ceiling_is_255_steps() {
  assert!(!crate::contract_steps_bound_is_valid(0));
  assert!(crate::contract_steps_bound_is_valid(1));
  assert!(crate::contract_steps_bound_is_valid(8));
  assert!(crate::contract_steps_bound_is_valid(255));
  assert!(!crate::contract_steps_bound_is_valid(256));
}

#[test]
fn contract_steps_bound_is_single_and_encoded_for_both_classes() {
  new_test_ext().execute_with(|| {
    assert_eq!(
      <<Test as crate::Config>::MaxContractSteps as Get<u32>>::get(),
      8
    );
    let steps: Vec<_> = (0..9)
      .map(|i| {
        make_step(Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(i + 1),
        })
      })
      .collect();
    assert!(crate::ContractSteps::<Test>::try_from(steps).is_err());
    let maximum = BoundedVec::try_from(
      (0..8)
        .map(|i| {
          make_step(Task::Transfer {
            to: BOB,
            asset: TestAsset::Native,
            amount: AmountResolution::Fixed(i + 1),
          })
        })
        .collect::<Vec<_>>(),
    )
    .expect("eight steps fit the shared encoded bound");
    prefund_active_user_creation(ALICE, &maximum);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, maximum.clone()),
    ));
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract(manual_schedule(), None, maximum),
    ));
  });
}

#[test]
fn sovereign_account_collision_rejected() {
  new_test_ext().execute_with(|| {
    let contract_steps = transfer_contract_steps(BOB, 10);
    let _actor_id = Actors::next_actor_id();
    let sovereign = Actors::sovereign_account_id(&ALICE, 0);
    SovereignIndex::<Test>::insert(&sovereign, 999u64);
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, contract_steps),
      ),
      Error::<Test>::SovereignAccountCollision
    );
  });
}

#[test]
fn control_authority_matrix_is_class_mutability_and_breaker_complete() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actors = [
      (
        create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          inert_contract_steps(),
        ),
        ActorType::User,
      ),
      (
        create_user_with(
          ALICE,
          Mutability::Immutable,
          timer_schedule(2),
          None,
          inert_contract_steps(),
        ),
        ActorType::User,
      ),
      (
        create_system_with_mutability(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          inert_contract_steps(),
        ),
        ActorType::System,
      ),
      (
        create_system_with_mutability(
          ALICE,
          Mutability::Immutable,
          timer_schedule(2),
          None,
          inert_contract_steps(),
        ),
        ActorType::System,
      ),
    ];

    for breaker in [false, true] {
      GlobalCircuitBreaker::<Test>::put(breaker);
      for (actor_id, actor_type) in actors {
        let actor = Actors::active_actor_view(actor_id).expect("matrix actor exists");
        assert_ok!(Actors::ensure_control_origin(
          RuntimeOrigin::signed(ALICE),
          &actor,
        ));
        assert_noop!(
          Actors::ensure_control_origin(RuntimeOrigin::signed(BOB), &actor),
          Error::<Test>::NotOwner
        );
        if actor_type == ActorType::System {
          assert_ok!(Actors::ensure_control_origin(RuntimeOrigin::root(), &actor));
        } else {
          assert_noop!(
            Actors::ensure_control_origin(RuntimeOrigin::root(), &actor),
            Error::<Test>::NotGovernance
          );
        }
      }
    }
  });
}

#[test]
fn not_owner_on_foreign_actor() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::signed(BOB), actor_id),
      Error::<Test>::NotOwner
    );
  });
}

#[test]
fn not_governance_on_user_actor_via_root() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::NotGovernance
    );
  });
}

#[test]
fn governance_can_manage_system_actor_control_surface() {
  new_test_ext().execute_with(|| {
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::pause_actor(RuntimeOrigin::root(), actor_id));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("system actor")
        .lifecycle,
      ActiveLifecycle::Paused
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::root(), actor_id));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("system actor")
        .lifecycle,
      ActiveLifecycle::Active
    );
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    assert!(
      Actors::active_actor_view(actor_id)
        .expect("system actor")
        .pending_signal
    );
    let updated_schedule = timer_schedule(3);
    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      updated_schedule.clone(),
      None,
    ));
    let updated_view = Actors::active_actor_view(actor_id).expect("system actor");
    assert_eq!(updated_view.trigger, updated_schedule.trigger);
    assert_eq!(
      updated_view.cooldown_blocks,
      updated_schedule.cooldown_blocks
    );
    ActorHot::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("system actor hot state")
        .unsuccessful_attempt_streak = 2;
    });
    frame_system::Pallet::<Test>::set_block_number(4);
    let updated_plan = transfer_contract_steps(BOB, 1);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      updated_plan.clone(),
      crate::CompletionPolicy::Persistent,
    ));
    let updated = Actors::active_actor_view(actor_id).expect("system actor");
    assert_eq!(updated.steps, updated_plan);
    assert_eq!(updated.unsuccessful_attempt_streak, 0);
    frame_system::Pallet::<Test>::set_block_number(5);
    assert_ok!(replace_auto_close(RuntimeOrigin::root(), actor_id, Some(5),));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("system actor")
        .auto_close_at_cycle_nonce,
      Some(5)
    );
    frame_system::Pallet::<Test>::set_block_number(6);
    assert_ok!(replace_auto_close(RuntimeOrigin::root(), actor_id, Some(7),));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("system actor")
        .auto_close_at_cycle_nonce,
      Some(7)
    );
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), actor_id));
    assert!(Actors::active_actor_view(actor_id).is_none());
  });
}

#[test]
fn resume_on_active_is_an_exact_noop() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    let before = Actors::active_actor_view(actor_id).expect("actor exists");
    System::reset_events();
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(Actors::active_actor_view(actor_id), Some(before));
    assert!(System::events().is_empty());
  });
}

#[test]
fn identity_control_clock_tracks_creation_and_survives_deactivation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(7);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract(manual_schedule(), None, inert_contract_steps()),
    ));
    let actor_id = Actors::next_actor_id().saturating_sub(1);
    assert_eq!(
      Actors::actor_identities(actor_id)
        .expect("created identity")
        .last_control_mutation_block,
      7
    );
    frame_system::Pallet::<Test>::set_block_number(8);
    assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), actor_id));
    assert_eq!(
      Actors::actor_identities(actor_id)
        .expect("dormant identity")
        .last_control_mutation_block,
      8
    );
  });
}

#[test]
fn funding_policy_replacement_preserves_placement_without_continuation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(10), None, inert_contract_steps());
    let before = Actors::actor_hot(actor_id).expect("actor hot state exists");
    assert!(before.wakeup_pointer.is_some());
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress,
    ));
    let after = Actors::actor_hot(actor_id).expect("actor hot state exists");
    assert_eq!(after.queue_ticket, before.queue_ticket);
    assert_eq!(after.wakeup_pointer, before.wakeup_pointer);
  });
}

#[test]
fn canonical_control_replacements_are_exact_noops_before_rate_limiting() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    let before = Actors::active_actor_view(actor_id).expect("actor exists");
    let funding_before = actor_funding(actor_id);
    let funding_policy_before = ActorContracts::<Test>::get(actor_id)
      .expect("active Actor Contract")
      .funding;
    System::reset_events();
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      funding_policy_before,
    ));
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      RuntimeSchedule {
        trigger: before.trigger.clone(),
        cooldown_blocks: before.cooldown_blocks,
      },
      before.window,
    ));
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      before.steps.clone(),
      before.completion,
    ));
    assert_eq!(Actors::active_actor_view(actor_id), Some(before));
    assert_eq!(actor_funding(actor_id), funding_before);
    assert!(System::events().is_empty());
  });
}

#[test]
fn pause_on_paused_is_an_exact_noop() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    frame_system::Pallet::<Test>::set_block_number(1);
    let before = Actors::active_actor_view(actor_id).expect("actor exists");
    System::reset_events();
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(Actors::active_actor_view(actor_id), Some(before));
    assert!(System::events().is_empty());
  });
}

#[test]
fn already_paused_on_manual_trigger() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_noop!(
      Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ActorPaused
    );
  });
}

#[test]
fn cycle_nonce_max_value_is_the_last_executable_cycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    ActorIdentities::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("system Actors identity exists").cycle_nonce = u64::MAX - 1;
    });
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    run_idle(Weight::MAX);
    let instance = Actors::active_actor_view(actor_id).expect("system Actors remains");
    assert_eq!(instance.cycle_nonce, u64::MAX);
    assert_eq!(native_balance(&BOB), bob_before + 1);
    assert!(has_actor_event(|event| {
      matches!(event, Event::CycleStarted { actor_id: id, cycle_nonce } if *id == actor_id && *cycle_nonce == u64::MAX)
    }));
    assert!(has_actor_event(|event| {
      matches!(event, Event::CycleSummary { actor_id: id, cycle_nonce, .. } if *id == actor_id && *cycle_nonce == u64::MAX)
    }));
  });
}

#[test]
fn cycle_nonce_exhaustion_closes_user_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    fund_native(actor_id, 1_000);
    let bob_before = native_balance(&BOB);
    ActorIdentities::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("user Actors identity exists")
        .cycle_nonce = u64::MAX;
    });
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before);
    assert!(!has_actor_event(|event| {
      matches!(event, Event::CycleStarted { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::CycleSummary { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::CycleNonceExhausted,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn cycle_nonce_exhaustion_closes_system_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let bob_before = native_balance(&BOB);
    ActorIdentities::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("system Actors identity exists")
        .cycle_nonce = u64::MAX;
    });
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before);
    assert!(!has_actor_event(|event| {
      matches!(event, Event::CycleStarted { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::CycleSummary { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::CycleNonceExhausted,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn missing_tracked_snapshot_resolves_to_funding_unavailable() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == actor_id
      )
    }));
    assert!(Actors::active_actor_view(actor_id).is_some());
  });
}

#[test]
fn zero_snapshot_resolves_to_funding_unavailable() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    crate::ActorFunding::<Test>::mutate(actor_id, |maybe| {
      let funding = maybe.as_mut().expect("actor funding exists");
      funding
        .funding_accumulated
        .try_insert(TestAsset::Native, 0)
        .expect("snapshot must fit");
    });
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn stale_tracked_snapshot_remains_valid_until_overwritten() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let bob_before = native_balance(&BOB);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      100
    ));
    frame_system::Pallet::<Test>::set_block_number(25);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 1);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
  });
}

#[test]
fn burn_last_funding_overspend_resolves_to_funding_unavailable() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Burn {
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 100);
    crate::ActorFunding::<Test>::mutate(actor_id, |maybe| {
      let funding = maybe.as_mut().expect("actor funding exists");
      funding
        .funding_accumulated
        .try_insert(TestAsset::Native, 200)
        .expect("snapshot must fit");
    });
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepSkipped {
        actor_id: id,
        step_index: 0,
        reason: StepSkippedReason::FundingUnavailable,
        ..
      } if *id == actor_id
    )));
    assert_eq!(native_balance(&actor), 100);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors remains active")
        .unsuccessful_attempt_streak,
      0
    );
  });
}

#[test]
fn overspend_resolves_to_funding_unavailable_for_system_without_pause() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1_000_000),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == actor_id
      )
    }));
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(instance.lifecycle, ActiveLifecycle::Active);
    let actor = sovereign_account(actor_id);
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
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1_000_000),
    }));
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == actor_id
      )
    }));
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert_eq!(
      native_balance(&actor),
      1_000u128.saturating_sub(Actors::compute_eval_fee(0))
    );
  });
}

#[test]
fn funding_unavailable_releases_exec_fee_reservation_for_later_step_spend() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1_000_000),
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(650),
      }),
    ])
    .expect("execution plan must fit");
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    let actor = sovereign_account(actor_id);
    let bob_before = native_balance(&BOB);
    fund_native(actor_id, 1_000);
    clear_fee_collections();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == actor_id
      )
    }));
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(650));
    let transfer = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(650),
    };
    let expected_attempt_fee = Actors::compute_eval_fee(0).saturating_add(
      TestWeightToFee::weight_to_fee(&Actors::weight_upper_bound(&transfer)),
    );
    assert_eq!(
      fee_collections(),
      vec![Actors::compute_eval_fee(0), expected_attempt_fee]
    );
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
    let contract_steps = contract_steps_with_step(make_step(Task::SwapIn {
      asset_in: TestAsset::Native,
      asset_out: TestAsset::Local(99),
      amount_in: AmountResolution::Fixed(10),
      slippage_tolerance: Perbill::zero(),
    }));
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let expected_fee = Actors::compute_eval_fee(0).saturating_add(TestWeightToFee::weight_to_fee(
      &Actors::weight_upper_bound(&Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(99),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::zero(),
      }),
    ));
    assert_eq!(
      native_balance(&actor),
      1_000u128.saturating_sub(expected_fee),
      "failed executable path should charge exactly eval+exec fee with no refund"
    );
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepFailed {
          actor_id: id,
          step_index: 0,
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn global_circuit_breaker_blocks_creation() {
  new_test_ext().execute_with(|| {
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 10)),
      ),
      Error::<Test>::GlobalCircuitBreakerActive
    );
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 10)),
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
    assert_eq!(Actors::configured_active_actor_limit(), old_limit);
    assert_ok!(Actors::set_active_actor_limit(RuntimeOrigin::root(), 2));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActiveActorLimitSet {
          old_limit: prev,
          new_limit: 2,
        } if *prev == old_limit
      )
    }));
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::set_active_actor_limit(RuntimeOrigin::root(), 2));
    assert!(frame_system::Pallet::<Test>::events().is_empty());
    let _ = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let _ = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::ActiveActorCapacityExceeded
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
      transfer_contract_steps(BOB, 1),
    );
    let _ = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_noop!(
      Actors::set_active_actor_limit(RuntimeOrigin::root(), 1),
      Error::<Test>::ActiveActorLimitBelowCurrent
    );
    assert_noop!(
      Actors::set_active_actor_limit(RuntimeOrigin::root(), 0),
      Error::<Test>::ActiveActorLimitTooLow
    );
    assert_noop!(
      Actors::set_active_actor_limit(RuntimeOrigin::root(), u32::MAX),
      Error::<Test>::ActiveActorLimitTooHigh
    );
    assert_noop!(
      Actors::set_active_actor_limit(
        RuntimeOrigin::root(),
        <<Test as crate::Config>::MaxQueueLength as Get<u32>>::get().saturating_add(1),
      ),
      Error::<Test>::ActiveActorLimitExceedsQueueCapacity
    );
    crate::ActiveActorLimit::<Test>::put(0);
    assert_eq!(Actors::effective_active_actor_limit(), 0);
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::ActiveActorCapacityExceeded
    );
    #[cfg(feature = "try-runtime")]
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
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
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(
          manual_schedule(),
          Some(window),
          transfer_contract_steps(BOB, 10)
        ),
      ),
      Error::<Test>::InvalidScheduleWindow
    );
  });
}

#[test]
fn schedule_window_requires_an_exact_end_plus_one_terminal_block() {
  new_test_ext().execute_with(|| {
    let window = ScheduleWindow {
      start: u64::MAX - 100,
      end: u64::MAX,
    };
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(
          manual_schedule(),
          Some(window),
          transfer_contract_steps(BOB, 10),
        ),
      ),
      Error::<Test>::InvalidScheduleWindow
    );
  });
}

#[test]
fn in_progress_window_is_admissible_when_now_is_inside() {
  new_test_ext().execute_with(|| {
    // Section 7.3.3: an in-progress window (start <= now <= end) is admissible;
    // only an already-expired window (end < now) is rejected.
    frame_system::Pallet::<Test>::set_block_number(50);
    let window = ScheduleWindow {
      start: 10,
      end: 200,
    };
    prefund_active_user_creation(ALICE, &transfer_contract_steps(BOB, 10));
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(
        manual_schedule(),
        Some(window),
        transfer_contract_steps(BOB, 10),
      ),
    ));
  });
}

#[test]
fn already_expired_window_is_rejected() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(300);
    let window = ScheduleWindow {
      start: 10,
      end: 200,
    };
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(
          manual_schedule(),
          Some(window),
          transfer_contract_steps(BOB, 10),
        ),
      ),
      Error::<Test>::InvalidScheduleWindow
    );
  });
}

#[test]
fn inclusive_window_span_exact_minimum_is_admissible() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // Inclusive span: end - start + 1 >= MinWindowLength (100). start 1, end 100
    // has span 100 and is admissible; start 1, end 99 has span 99 and is rejected.
    prefund_active_user_creation(ALICE, &transfer_contract_steps(BOB, 10));
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(
        manual_schedule(),
        Some(ScheduleWindow { start: 1, end: 100 }),
        transfer_contract_steps(BOB, 10),
      ),
    ));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(BOB),
        Mutability::Mutable,
        user_active_contract(
          manual_schedule(),
          Some(ScheduleWindow { start: 1, end: 99 }),
          transfer_contract_steps(CHARLIE, 10),
        ),
      ),
      Error::<Test>::InvalidScheduleWindow
    );
  });
}

#[test]
fn schedule_cooldown_is_bounded_by_max_execution_delay() {
  new_test_ext().execute_with(|| {
    let too_long = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: u32::try_from(
        <Test as crate::Config>::MaxExecutionDelayBlocks::get().saturating_add(1),
      )
      .unwrap_or(u32::MAX),
    };
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(too_long, None, transfer_contract_steps(BOB, 10),),
      ),
      Error::<Test>::ExecutionDelayTooLong
    );
  });
}

#[test]
fn future_schedule_targets_accept_exact_boundary_and_reject_overflow_without_mutation() {
  new_test_ext().execute_with(|| {
    let max = u64::MAX;
    frame_system::Pallet::<Test>::set_block_number(max - 5_000);
    let exact_cooldown = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 5_000,
    };
    let actor_id = create_system_with(ALICE, exact_cooldown, None, inert_contract_steps());
    assert!(Actors::active_actor_view(actor_id).is_some());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });

  new_test_ext().execute_with(|| {
    let max = u64::MAX;
    frame_system::Pallet::<Test>::set_block_number(max - 4_999);
    let overflowing_cooldown = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 5_000,
    };
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(overflowing_cooldown, None, inert_contract_steps()),
      ),
      Error::<Test>::SchedulerIndexExhausted
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });

  new_test_ext().execute_with(|| {
    let max = u64::MAX;
    frame_system::Pallet::<Test>::set_block_number(max - 4);
    let actor_id = create_system_with(ALICE, timer_schedule(4), None, inert_contract_steps());
    assert!(Actors::active_actor_view(actor_id).is_some());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });

  new_test_ext().execute_with(|| {
    let max = u64::MAX;
    frame_system::Pallet::<Test>::set_block_number(max - 3);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract(timer_schedule(4), None, inert_contract_steps()),
    ));
  });
}

#[test]
fn future_window_terminal_and_exact_next_block_reject_overflow_without_mutation() {
  new_test_ext().execute_with(|| {
    let max = u64::MAX;
    frame_system::Pallet::<Test>::set_block_number(max - 101);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow {
        start: max - 100,
        end: max - 1,
      }),
      inert_contract_steps(),
    );
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot").terminal_at,
      Some(max)
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(u64::MAX);
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, inert_contract_steps()),
      ),
      Error::<Test>::SchedulerIndexExhausted
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });
}

#[test]
fn retry_target_uses_only_cursor_local_count_and_last_attempt_block() {
  new_test_ext().execute_with(|| {
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let instance = Actors::active_actor_view(actor_id).expect("actor");
    let continuation = |last_attempt_block| RuntimeContinuationState {
      cursor: 0,
      unsuccessful_attempts_at_cursor: 1,
      last_attempt_block,
      opening_snapshot: Default::default(),
      opening_predicate_results: Default::default(),
      funding_snapshot: Default::default(),
      cumulative_outcomes: Default::default(),
    };
    ContinuationStateStore::<Test>::insert(actor_id, continuation(u64::MAX - 1));
    assert_eq!(Actors::retry_eligible_at(actor_id, &instance), Ok(u64::MAX));

    ContinuationStateStore::<Test>::insert(actor_id, continuation(u64::MAX));
    assert_eq!(
      Actors::retry_eligible_at(actor_id, &instance),
      Err(crate::EnqueueOutcome::SchedulerIndexExhausted)
    );
  });
}

#[test]
fn queue_saturation_at_block_max_cannot_create_same_block_wakeup() {
  new_test_ext().execute_with(|| {
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    frame_system::Pallet::<Test>::set_block_number(u64::MAX);
    seed_saturated_tombstone_queue();
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    assert_eq!(
      Actors::enqueue(actor_id),
      Err(crate::EnqueueOutcome::SchedulerIndexExhausted)
    );

    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
    assert!(
      Actors::actor_hot(actor_id)
        .expect("hot")
        .wakeup_pointer
        .is_none()
    );
  });
}

// --- Task & Predicate Coverage Tests ---

#[test]
fn mint_works_for_system_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Mint {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(500),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    // Mint on empty account works — mint policy skips source-balance check
    let before = native_balance(&actor);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), before + 500);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::MintExecuted { actor_id: id, asset: TestAsset::Native, amount: 500, .. } if *id == actor_id
    )));
  });
}

#[test]
fn mint_percentage_at_opening_uses_target_not_preservable_surface() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(5);
    let contract_steps = contract_steps_with_step(make_step(Task::Mint {
      asset,
      amount: AmountResolution::PercentageAtOpening(Perbill::from_percent(50)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 100);

    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);

    assert_eq!(asset_balance(&actor, asset), 150);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::MintExecuted {
        actor_id: id,
        asset: minted_asset,
        amount: 50,
        ..
      } if *id == actor_id && *minted_asset == asset
    )));
  });
}

#[test]
fn optional_bounded_dnf_is_canonical_and_mode_distinct() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(5);
    let observed = TestAsset::Local(99);
    set_asset_balance(&ALICE, observed, 100);
    let unconditional: Option<crate::PreconditionOf<Test>> = None;
    assert!(unconditional.is_none());

    let all = all_conditions(vec![
      Predicate::BalanceAbove {
        asset: observed,
        threshold: 50,
      },
      Predicate::BlockNumberBelow { threshold: 10 },
      Predicate::BalanceBelow {
        asset: observed,
        threshold: 200,
      },
      Predicate::BlockNumberAbove { threshold: 1 },
    ]);
    let any = any_conditions(vec![
      Predicate::BalanceAbove {
        asset: observed,
        threshold: 1_000,
      },
      Predicate::BlockNumberBelow { threshold: 1 },
      Predicate::BalanceBelow {
        asset: observed,
        threshold: 200,
      },
      Predicate::BlockNumberAbove { threshold: 10 },
    ]);
    assert_eq!(all.as_ref().expect("all precondition").predicate_count(), 4);
    assert_eq!(any.as_ref().expect("any precondition").predicate_count(), 4);
    assert_eq!(
      Actors::evaluate_precondition(all.as_ref().expect("all precondition"), &ALICE, 0),
      Ok(true)
    );
    assert_eq!(
      Actors::evaluate_precondition(any.as_ref().expect("any precondition"), &ALICE, 0),
      Ok(true)
    );
    assert_ne!(all.encode(), any.encode());

    let all_false = all_conditions(vec![
      Predicate::BalanceAbove {
        asset: observed,
        threshold: 1_000,
      },
      Predicate::BlockNumberAbove { threshold: 10 },
    ]);
    let any_false = any_conditions(vec![
      Predicate::BalanceAbove {
        asset: observed,
        threshold: 1_000,
      },
      Predicate::BlockNumberAbove { threshold: 10 },
    ]);
    assert_eq!(
      Actors::evaluate_precondition(all_false.as_ref().expect("all precondition"), &ALICE, 0),
      Ok(false)
    );
    assert_eq!(
      Actors::evaluate_precondition(any_false.as_ref().expect("any precondition"), &ALICE, 0),
      Ok(false)
    );
  });
}

#[test]
fn predicate_evaluator_visits_every_atom_and_preserves_first_error() {
  let precondition = any_conditions(vec![
    Predicate::BlockNumberAbove { threshold: 0 },
    Predicate::BlockNumberAbove { threshold: 1 },
    Predicate::BlockNumberAbove { threshold: 2 },
    Predicate::BlockNumberAbove { threshold: 3 },
  ]);
  let mut visits = 0u32;
  let result = crate::execution::evaluate_precondition_with(
    precondition.as_ref().expect("precondition"),
    |_| {
      visits += 1;
      if visits == 1 { Err("first") } else { Ok(true) }
    },
  );
  assert_eq!(result, Err("first"));
  assert_eq!(visits, 4);
}

#[test]
fn admission_canonicalizes_dnf_and_equivalent_update_is_exact_noop() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let first = TimedPredicate {
      timing: ObservationTiming::Current,
      predicate: Predicate::BlockNumberAbove { threshold: 0 },
    };
    let second = TimedPredicate {
      timing: ObservationTiming::Current,
      predicate: Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 0,
      },
    };
    let raw_precondition = || {
      Some(Precondition {
        clauses: BoundedVec::try_from(vec![
          BoundedVec::try_from(vec![second, first, second]).expect("predicates fit"),
        ])
        .expect("clause fits"),
      })
    };
    let raw_plan = || {
      contract_steps_with_step(StepOf::<Test> {
        precondition: raw_precondition(),
        task: Task::StopCycle,
        on_error: StepErrorPolicy::AbortCycle,
      })
    };
    let actor_id = create_system_with(ALICE, manual_schedule(), None, raw_plan());
    let stored = Actors::actor_contract(actor_id).expect("contract exists");
    assert_eq!(
      stored.steps[0]
        .precondition
        .as_ref()
        .expect("stored precondition")
        .predicate_count(),
      2
    );
    let clauses = &stored.steps[0]
      .precondition
      .as_ref()
      .expect("stored precondition")
      .clauses;
    assert!(clauses[0][0].encode() < clauses[0][1].encode());

    let event_count = frame_system::Pallet::<Test>::event_count();
    let control_block = ActorIdentities::<Test>::get(actor_id)
      .expect("identity exists")
      .last_control_mutation_block;
    assert_ok!(Actors::update_contract(
      RuntimeOrigin::root(),
      actor_id,
      ActorContract {
        steps: raw_plan(),
        ..stored.clone()
      },
    ));
    assert_eq!(frame_system::Pallet::<Test>::event_count(), event_count);
    assert_eq!(
      ActorIdentities::<Test>::get(actor_id)
        .expect("identity exists")
        .last_control_mutation_block,
      control_block
    );

    let duplicate_clauses = Some(Precondition {
      clauses: BoundedVec::try_from(vec![
        BoundedVec::try_from(vec![first, second]).expect("first clause fits"),
        BoundedVec::try_from(vec![second, first, second]).expect("second clause fits"),
      ])
      .expect("clauses fit"),
    });
    let duplicate_plan = contract_steps_with_step(StepOf::<Test> {
      precondition: duplicate_clauses,
      task: Task::StopCycle,
      on_error: StepErrorPolicy::AbortCycle,
    });
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, duplicate_plan),
      ),
      Error::<Test>::InvalidPredicate
    );
  });
}

#[test]
fn admission_absorbs_exact_dnf_superset_clause() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let first = TimedPredicate {
      timing: ObservationTiming::Current,
      predicate: Predicate::BlockNumberAbove { threshold: 0 },
    };
    let second = TimedPredicate {
      timing: ObservationTiming::Current,
      predicate: Predicate::BlockNumberBelow { threshold: 10 },
    };
    let absorbed = Some(Precondition {
      clauses: BoundedVec::try_from(vec![
        BoundedVec::try_from(vec![first]).expect("subset fits"),
        BoundedVec::try_from(vec![second, first]).expect("superset fits"),
      ])
      .expect("clauses fit"),
    });
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(StepOf::<Test> {
        precondition: absorbed,
        task: Task::StopCycle,
        on_error: StepErrorPolicy::AbortCycle,
      }),
    );
    let stored = Actors::actor_contract(actor_id).expect("contract exists");
    let clauses = &stored.steps[0]
      .precondition
      .as_ref()
      .expect("stored precondition")
      .clauses;
    assert_eq!(clauses.len(), 1);
    assert_eq!(clauses[0].as_slice(), &[first]);
  });
}

#[test]
fn opening_and_current_predicates_observe_distinct_step_state() {
  for (timing, second_step_executes) in [
    (ObservationTiming::Opening, true),
    (ObservationTiming::Current, false),
  ] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let plan = BoundedVec::try_from(vec![
        make_step(Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(60),
        }),
        StepOf::<Test> {
          precondition: timed_all_conditions(
            timing,
            vec![Predicate::BalanceAbove {
              asset: TestAsset::Native,
              threshold: 50,
            }],
          ),
          task: Task::Transfer {
            to: CHARLIE,
            asset: TestAsset::Native,
            amount: AmountResolution::Fixed(10),
          },
          on_error: StepErrorPolicy::AbortCycle,
        },
      ])
      .expect("two-step plan fits");
      let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
      fund_native(actor_id, 100);
      let charlie_before = native_balance(&CHARLIE);
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      run_idle(Weight::MAX);
      assert_eq!(
        native_balance(&CHARLIE) > charlie_before,
        second_step_executes
      );
      assert_eq!(
        has_actor_event(|event| matches!(
          event,
          Event::StepSkipped {
            actor_id: id,
            step_index: 1,
            reason: StepSkippedReason::PreconditionFalse,
            ..
          } if *id == actor_id
        )),
        !second_step_executes
      );
    });
  }
}

#[test]
fn opening_predicate_result_is_reused_by_continuation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let plan = contract_steps_with_step(StepOf::<Test> {
      precondition: timed_all_conditions(
        ObservationTiming::Opening,
        vec![Predicate::BalanceAbove {
          asset: TestAsset::Native,
          threshold: 50,
        }],
      ),
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: RETRY_LATER,
    });
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 100);
    let actor = sovereign_account(actor_id);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let continuation = Actors::continuation_state(actor_id).expect("continuation exists");
    assert_eq!(
      continuation.opening_predicate_results.as_slice(),
      &[Ok(true)]
    );
    assert_ok!(MockAssetOps::transfer(&actor, &BOB, TestAsset::Native, 60));
    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::SwapExecuted { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn above_and_below_conditions_are_strict_at_equality() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(5);
    let asset = TestAsset::Local(99);
    set_asset_balance(&ALICE, asset, 100);
    set_observation(
      1,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 5,
      },
    );
    let equality_boundaries = [
      all_conditions(vec![Predicate::BalanceAbove {
        asset,
        threshold: 100,
      }]),
      all_conditions(vec![Predicate::BalanceBelow {
        asset,
        threshold: 100,
      }]),
      all_conditions(vec![Predicate::BlockNumberAbove { threshold: 5 }]),
      all_conditions(vec![Predicate::BlockNumberBelow { threshold: 5 }]),
      all_conditions(vec![Predicate::ObservationAbove {
        feed: 1,
        threshold: 50,
        max_age_blocks: 10,
      }]),
      all_conditions(vec![Predicate::ObservationBelow {
        feed: 1,
        threshold: 50,
        max_age_blocks: 10,
      }]),
    ];
    for conditions in equality_boundaries {
      assert_eq!(
        Actors::evaluate_precondition(
          conditions.as_ref().expect("bounded precondition"),
          &ALICE,
          0
        ),
        Ok(false)
      );
    }
  });
}

#[test]
fn unconfigured_observation_provider_fails_closed() {
  assert_eq!(
    <() as crate::ObservationProvider<u32, u64>>::observe(&1, 0, 10),
    crate::ScalarObservationState::Unavailable
  );
}

#[test]
fn observation_conditions_compare_only_fresh_scalar_values() {
  new_test_ext().execute_with(|| {
    set_observation(
      1,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 0,
      },
    );
    set_observation(2, crate::ScalarObservationState::Unavailable);
    set_observation(3, crate::ScalarObservationState::Uninitialized);
    set_observation(4, crate::ScalarObservationState::Stale);
    let fresh = all_conditions(vec![
      Predicate::ObservationAbove {
        feed: 1,
        threshold: 49,
        max_age_blocks: 10,
      },
      Predicate::ObservationBelow {
        feed: 1,
        threshold: 51,
        max_age_blocks: 10,
      },
      Predicate::ObservationEquals {
        feed: 1,
        threshold: 50,
        max_age_blocks: 10,
      },
      Predicate::ObservationNotEquals {
        feed: 1,
        threshold: 49,
        max_age_blocks: 10,
      },
    ]);
    assert_eq!(
      Actors::evaluate_precondition(fresh.as_ref().expect("bounded precondition"), &ALICE, 0),
      Ok(true)
    );

    for feed in 2..=4 {
      let unavailable = all_conditions(vec![Predicate::ObservationNotEquals {
        feed,
        threshold: 50,
        max_age_blocks: 10,
      }]);
      assert_eq!(
        Actors::evaluate_precondition(
          unavailable.as_ref().expect("bounded precondition"),
          &ALICE,
          0
        ),
        Ok(false)
      );
    }
  });
}

#[test]
fn invalid_fresh_observation_fails_permanently_and_applies_step_policy() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(10);
    set_observation(
      1,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 11,
      },
    );
    let invalid_condition_step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::ObservationAbove {
        feed: 1,
        threshold: 1,
        max_age_blocks: 5,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(7),
    });
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![invalid_condition_step, succeeding_step])
        .expect("two-step plan fits"),
    );
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before + 7);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepFailed {
        actor_id: id,
        step_index: 0,
        error,
        ..
      } if *id == actor_id && *error == Error::<Test>::InvalidPredicate.into()
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        result: CycleResult::Completed,
        outcomes: OutcomeTotals { failed_steps: 1, committed_effectful_tasks: 1, .. },
        ..
      } if *id == actor_id
    )));

    set_observation(
      1,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 4,
      },
    );
    let over_age = all_conditions(vec![Predicate::ObservationAbove {
      feed: 1,
      threshold: 1,
      max_age_blocks: 5,
    }]);
    assert_eq!(
      Actors::evaluate_precondition(over_age.as_ref().expect("bounded precondition"), &ALICE, 0),
      Err(Error::<Test>::InvalidPredicate.into())
    );
  });
}

#[test]
fn zero_amount_resolutions_and_identical_market_assets_are_rejected() {
  new_test_ext().execute_with(|| {
    for amount in [
      AmountResolution::Fixed(0),
      AmountResolution::PercentageOfCurrent(Perbill::zero()),
      AmountResolution::PercentageAtOpening(Perbill::zero()),
      AmountResolution::PercentageOfLastFunding(Perbill::zero()),
    ] {
      let plan = contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount,
      }));
      assert_noop!(
        Actors::create_system_actor(
          RuntimeOrigin::root(),
          ALICE,
          Mutability::Mutable,
          system_active_contract(manual_schedule(), None, plan),
        ),
        Error::<Test>::InvalidAmountResolution
      );
    }

    let zero_nested_amount_tasks = vec![
      Task::SwapOut {
        asset_out: TestAsset::Local(77),
        amount_out: AmountResolution::Fixed(0),
        asset_in: TestAsset::Native,
        input_limit: InputLimit::LiveQuote,
        slippage_tolerance: Perbill::one(),
      },
      Task::AddLiquidity {
        asset_a: TestAsset::Native,
        asset_b: TestAsset::Local(77),
        amount_a: AmountResolution::Fixed(1),
        amount_b: AmountResolution::Fixed(0),
        min_lp_out: 1,
      },
      Task::RemoveLiquidity {
        lp_asset: TestAsset::Local(99),
        asset_a: TestAsset::Local(99),
        asset_b: TestAsset::Local(99),
        lp_amount: AmountResolution::Fixed(0),
        min_amount_a: 1,
        min_amount_b: 1,
      },
      Task::Unstake {
        asset: TestAsset::Local(77),
        shares: AmountResolution::Fixed(0),
      },
    ];
    for task in zero_nested_amount_tasks {
      assert_noop!(
        Actors::create_system_actor(
          RuntimeOrigin::root(),
          ALICE,
          Mutability::Mutable,
          system_active_contract(
            manual_schedule(),
            None,
            contract_steps_with_step(make_step(task)),
          ),
        ),
        Error::<Test>::InvalidAmountResolution
      );
    }

    let identical_asset_tasks = vec![
      Task::SwapIn {
        asset_in: TestAsset::Native,
        amount_in: AmountResolution::Fixed(1),
        asset_out: TestAsset::Native,
        slippage_tolerance: Perbill::one(),
      },
      Task::AddLiquidity {
        asset_a: TestAsset::Native,
        asset_b: TestAsset::Native,
        amount_a: AmountResolution::Fixed(1),
        amount_b: AmountResolution::Fixed(1),
        min_lp_out: 1,
      },
      Task::DonateLiquidity {
        asset_a: TestAsset::Native,
        asset_b: TestAsset::Native,
        max_amount_a: AmountResolution::Fixed(1),
        max_ratio_error: Perbill::one(),
      },
    ];
    for task in identical_asset_tasks {
      assert_noop!(
        Actors::create_system_actor(
          RuntimeOrigin::root(),
          ALICE,
          Mutability::Mutable,
          system_active_contract(
            manual_schedule(),
            None,
            contract_steps_with_step(make_step(task)),
          ),
        ),
        Error::<Test>::InvalidTradeBound
      );
    }
  });
}

#[test]
fn self_transfer_rejection_covers_create_update_and_activation_paths() {
  new_test_ext().execute_with(|| {
    let system_id = Actors::next_actor_id();
    let system_sovereign = Actors::sovereign_account_id_system(system_id);
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(
          manual_schedule(),
          None,
          transfer_contract_steps(system_sovereign, 1),
        ),
      ),
      Error::<Test>::SelfTransferNotAllowed
    );

    let user_sovereign = Actors::sovereign_account_id(&ALICE, 0);
    assert_noop!(
      Actors::create_user_actor_at_slot(
        RuntimeOrigin::signed(ALICE),
        0,
        Mutability::Mutable,
        user_active_contract(
          manual_schedule(),
          None,
          transfer_contract_steps(user_sovereign, 1),
        ),
      ),
      Error::<Test>::SelfTransferNotAllowed
    );
    assert_eq!(fee_collections(), Vec::<Balance>::new());
    assert_eq!(Actors::owner_slot_bitmap(ALICE), [0; 32]);

    let active_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let active_sovereign = sovereign_account(active_id);
    let before = Actors::active_actor_view(active_id).expect("active actor");
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::root(),
        active_id,
        transfer_contract_steps(active_sovereign, 1),
        crate::CompletionPolicy::Persistent,
      ),
      Error::<Test>::SelfTransferNotAllowed
    );
    assert_eq!(Actors::active_actor_view(active_id), Some(before));

    let dormant_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None,
    ));
    let dormant = Actors::actor_identities(dormant_id).expect("dormant identity");
    let self_leg_plan = contract_steps_with_step(make_step(Task::SplitTransfer {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
      legs: BoundedVec::try_from(vec![
        SplitLeg {
          to: dormant.sovereign_account,
          share: Perbill::from_percent(50),
        },
        SplitLeg {
          to: BOB,
          share: Perbill::from_percent(50),
        },
      ])
      .expect("two legs fit"),
    }));
    assert_noop!(
      Actors::activate_actor(
        RuntimeOrigin::signed(ALICE),
        dormant_id,
        system_active_contract(manual_schedule(), None, self_leg_plan)
          .expect("direct Actor Contract"),
      ),
      Error::<Test>::SelfTransferNotAllowed
    );
    assert!(Actors::actor_identities(dormant_id).is_some());
    assert!(Actors::active_actor_view(dormant_id).is_none());
  });
}

#[test]
fn zero_observation_max_age_is_rejected_during_plan_validation() {
  new_test_ext().execute_with(|| {
    let plan = contract_steps_with_step(StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::ObservationAbove {
        feed: 1,
        threshold: 0,
        max_age_blocks: 0,
      }]),
      task: Task::StopCycle,
      on_error: StepErrorPolicy::AbortCycle,
    });
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, plan),
      ),
      Error::<Test>::InvalidObservationMaxAge
    );
  });
}

#[test]
fn empty_outer_and_inner_precondition_forms_are_rejected() {
  new_test_ext().execute_with(|| {
    let empty_outer = Precondition {
      clauses: BoundedVec::default(),
    };
    let empty_inner = Precondition {
      clauses: BoundedVec::try_from(vec![BoundedVec::default()]).expect("empty clause fits"),
    };
    for precondition in [empty_outer, empty_inner] {
      let plan = contract_steps_with_step(StepOf::<Test> {
        precondition: Some(precondition),
        task: Task::StopCycle,
        on_error: StepErrorPolicy::AbortCycle,
      });
      assert_noop!(
        Actors::create_system_actor(
          RuntimeOrigin::root(),
          ALICE,
          Mutability::Mutable,
          system_active_contract(manual_schedule(), None, plan),
        ),
        Error::<Test>::EmptyPrecondition
      );
    }
  });
}

#[test]
fn any_with_multiple_true_atoms_executes_the_task_once() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(5);
    let step = StepOf::<Test> {
      precondition: any_conditions(vec![
        Predicate::BlockNumberAbove { threshold: 1 },
        Predicate::BlockNumberBelow { threshold: 10 },
      ]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(step),
    );
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(
      System::events()
        .iter()
        .filter(|record| matches!(
          record.event,
          RuntimeEvent::Actors(Event::TransferExecuted { actor_id: id, .. }) if id == actor_id
        ))
        .count(),
      1
    );
  });
}

#[test]
fn any_skip_cannot_create_continuation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(5);
    let step = StepOf::<Test> {
      precondition: any_conditions(vec![
        Predicate::BlockNumberBelow { threshold: 1 },
        Predicate::BlockNumberAbove { threshold: 10 },
      ]),
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: RETRY_LATER,
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(step),
    );
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor remains")
        .unsuccessful_attempt_streak,
      0
    );
  });
}

#[test]
fn retry_re_evaluates_live_any_conditions_at_the_same_cursor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let step = StepOf::<Test> {
      precondition: any_conditions(vec![
        Predicate::BlockNumberBelow { threshold: 2 },
        Predicate::BlockNumberAbove { threshold: 100 },
      ]),
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: RETRY_LATER,
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(step),
    );
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("suspended")
        .cursor,
      0
    );

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { precondition_skips: 1, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn condition_fee_depends_only_on_total_atomic_count() {
  let forward = all_conditions(vec![
    Predicate::BlockNumberAbove { threshold: 1 },
    Predicate::BlockNumberBelow { threshold: 10 },
  ]);
  let reverse = any_conditions(vec![
    Predicate::BlockNumberBelow { threshold: 10 },
    Predicate::BlockNumberAbove { threshold: 1 },
  ]);
  let forward = forward.expect("bounded precondition");
  let reverse = reverse.expect("bounded precondition");
  assert_eq!(forward.predicate_count(), reverse.predicate_count());
  assert_eq!(
    Actors::compute_eval_fee(forward.predicate_count()),
    Actors::compute_eval_fee(reverse.predicate_count())
  );
}

#[test]
fn unavailable_observation_skips_without_incrementing_failures() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::ObservationNotEquals {
        feed: 99,
        threshold: 0,
        max_age_blocks: 10,
      }]),
      task: Task::StopCycle,
      on_error: RETRY_LATER,
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(step),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepSkipped {
        actor_id: id,
        reason: StepSkippedReason::PreconditionFalse,
        ..
      } if *id == actor_id
    )));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor remains")
        .unsuccessful_attempt_streak,
      0
    );
    assert!(Actors::continuation_state(actor_id).is_none());
  });
}

#[test]
fn condition_balance_above_skips_when_below() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 1_000,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let actor = sovereign_account(actor_id);
    assert_eq!(
      native_balance(&actor),
      100,
      "transfer skipped — balance below threshold"
    );
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepSkipped {
        actor_id: id,
        step_index: 0,
        reason: StepSkippedReason::PreconditionFalse,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn condition_balance_above_executes_when_above() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 50,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
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
      precondition: all_conditions(vec![Predicate::BlockNumberAbove { threshold: 10 }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepSkipped { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
  });
}

#[test]
fn condition_block_number_below_skips_after_threshold() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(20);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BlockNumberBelow { threshold: 10 }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepSkipped { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
  });
}

#[test]
fn continue_next_step_error_policy_proceeds_after_failure() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
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
    let contract_steps = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(
      has_actor_event(|e| matches!(
        e,
        Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id
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
      precondition: None,
      task: Task::SwapIn {
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
    let contract_steps = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 120);
    set_fail_dex_after_input_transfer(true);
    let charlie_before = native_balance(&CHARLIE);
    let pool_native_before = native_balance(&u64::MAX);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 110);
    assert_eq!(asset_balance(&actor, asset_out), 0);
    assert_eq!(native_balance(&u64::MAX), pool_native_before);
    assert_eq!(asset_balance(&u64::MAX, asset_out), 10_000);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(!has_actor_event(|e| matches!(
      e,
      Event::SwapExecuted { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 1, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn invalid_schedule_window_too_short() {
  new_test_ext().execute_with(|| {
    // MinWindowLength = 100 in mock
    let window = ScheduleWindow { start: 10, end: 50 };
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(
          manual_schedule(),
          Some(window),
          transfer_contract_steps(BOB, 10)
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
    let contract_steps = BoundedVec::try_from(vec![
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
        amount: AmountResolution::PercentageAtOpening(Perbill::one()),
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
        amount: AmountResolution::AllAvailable,
      }),
    ])
    .expect("system execution plan fits");
    let actor_id = create_system_with(ALICE, percentage_trigger_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    crate::ActorFunding::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("System Actors funding exists")
        .funding_accumulated
        .try_insert(TestAsset::Native, 100)
        .expect("tracked snapshot fits");
    });
    let actor = sovereign_account(actor_id);
    let bob_before = native_balance(&BOB);
    signal_percentage_trigger(actor_id, TestAsset::Native);
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 1);
    assert_eq!(native_balance(&BOB), bob_before + 99);
    let funding_skips = frame_system::Pallet::<Test>::events()
      .iter()
      .filter(|record| {
        matches!(
          &record.event,
          RuntimeEvent::Actors(Event::StepSkipped {
            actor_id: id,
            reason: StepSkippedReason::FundingUnavailable,
            ..
          }) if *id == actor_id
        )
      })
      .count();
    assert_eq!(funding_skips, 4);
    let resolution_skips = frame_system::Pallet::<Test>::events()
      .iter()
      .filter(|record| {
        matches!(
          &record.event,
          RuntimeEvent::Actors(Event::StepSkipped {
            actor_id: id,
            reason: StepSkippedReason::ResolutionSkipped,
            ..
          }) if *id == actor_id
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
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfCurrent(Perbill::one()),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let actor = sovereign_account(actor_id);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 1);
    assert_eq!(native_balance(&BOB), bob_before + 99);
    assert!(frame_system::Pallet::<Test>::events().iter().any(|record| {
      matches!(
        &record.event,
        RuntimeEvent::Actors(Event::TransferExecuted {
          actor_id: id,
          amount: 99,
          ..
        }) if *id == actor_id
      )
    }));
  });
}

#[test]
fn user_all_available_preserves_current_floor_but_no_future_cycle_fee() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let task = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::AllAvailable,
    };
    let contract_steps = contract_steps_with_step(make_step(task));
    let fee = Actors::attempt_fee_envelope(ActorType::User, &contract_steps, 0)
      .expect("User fee envelope")
      .total;
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    let actor = sovereign_account(actor_id);
    let floor = TestMinUserBalance::get();
    let initial = floor.saturating_add(fee).saturating_add(50);
    fund_native(actor_id, initial);
    let bob_before = native_balance(&BOB);
    clear_fee_collections();

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(native_balance(&actor), floor);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(fee_collections(), vec![fee]);

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::FeeBudgetExhausted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn user_fee_admission_boundary_is_exact_floor_plus_envelope() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    }));
    let fee = Actors::attempt_fee_envelope(ActorType::User, &contract_steps, 0)
      .expect("User fee envelope")
      .total;
    let floor = TestMinUserBalance::get();

    // floor + envelope - 1: the complete envelope cannot fit above the floor, so
    // the User attempt is NOT admitted and the actor closes with FeeBudgetExhausted
    // even though the raw balance covers the envelope (spec 5.2.1).
    let prefunded = user_prefunding_requirement(&contract_steps);
    let short = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps.clone(),
    );
    deplete_user_sovereign(short, prefunded);
    fund_native(short, floor.saturating_add(fee).saturating_sub(1));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), short));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(short).is_none());
    assert!(has_actor_event(|e| matches!(
      e,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::FeeBudgetExhausted,
        ..
      } if *id == short
    )));

    // floor + envelope: the envelope fits exactly above the floor and the run admits.
    let prefunded = user_prefunding_requirement(&contract_steps);
    let exact = create_user_with(
      BOB,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(exact, prefunded);
    fund_native(exact, floor.saturating_add(fee));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(BOB), exact));
    run_idle(Weight::MAX);
    // The attempt is admitted (the envelope fits exactly above the floor); the
    // transfer itself is floor-limited, so assert admission via the nonce advance.
    assert_eq!(
      Actors::active_actor_view(exact)
        .expect("active")
        .cycle_nonce,
      1
    );
  });
}

#[test]
fn user_swap_out_native_input_cap_preserves_fee_native_floor_after_failed_attempt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_out = TestAsset::Local(88);
    let task = Task::SwapOut {
      asset_out,
      amount_out: AmountResolution::Fixed(98),
      asset_in: TestAsset::Native,
      input_limit: InputLimit::LiveQuote,
      slippage_tolerance: Perbill::zero(),
    };
    set_pool_reserves(TestAsset::Native, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let contract_steps = contract_steps_with_step(make_step(task));
    let fee_envelope =
      Actors::attempt_fee_envelope(ActorType::User, &contract_steps, 0).expect("User fee envelope");
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    let actor = sovereign_account(actor_id);
    let floor = TestMinUserBalance::get();
    let initial = floor.saturating_add(fee_envelope.total).saturating_add(50);
    fund_native(actor_id, initial);
    clear_fee_collections();

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(
      native_balance(&actor),
      initial.saturating_sub(fee_envelope.total)
    );
    assert!(native_balance(&actor) >= floor);
    assert_eq!(fee_collections(), vec![fee_envelope.total]);
    assert!(has_actor_event(|event| {
      matches!(event, Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id)
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
    }));
  });
}

#[test]
fn percentage_of_current_uses_sufficient_asset_preservable_balance_as_its_base() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(7);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset,
      amount: AmountResolution::PercentageOfCurrent(Perbill::one()),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 10);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
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
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset,
        amount: AmountResolution::Fixed(10),
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset,
        amount: AmountResolution::AllAvailable,
      }),
    ])
    .expect("system execution plan fits");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 10);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 1);
    assert_eq!(asset_balance(&BOB, asset), 9);
  });
}

#[test]
fn burn_all_balance_preserves_the_asset_minimum() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Burn {
      asset: TestAsset::Native,
      amount: AmountResolution::AllAvailable,
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 500);
    let actor = sovereign_account(actor_id);
    assert_eq!(native_balance(&actor), 500);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&actor),
      1,
      "Burn(AllAvailable) must preserve the asset minimum"
    );
    assert!(has_actor_event(|e| matches!(
      e,
      Event::BurnExecuted { actor_id: id, asset: TestAsset::Native, amount: 499, .. } if *id == actor_id
    )));
  });
}

#[test]
fn mint_on_unfunded_account_creates_tokens() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Mint {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1000),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    assert_eq!(native_balance(&actor), 0);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 1000);
  });
}

#[test]
fn stake_task_delegates_to_staking_adapter() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(7);
    let contract_steps = contract_steps_with_step(make_step(Task::Stake {
      asset,
      amount: AmountResolution::Fixed(120),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 200);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 80);
    assert_eq!(staked_balance(actor, asset), 120);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StakeExecuted { actor_id: id, asset: event_asset, amount, .. }
        if *id == actor_id && *event_asset == asset && *amount == 120
    )));
  });
}

#[test]
fn stake_preserve_spend_keeps_asset_minimum_balance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(70);
    let contract_steps = contract_steps_with_step(make_step(Task::Stake {
      asset,
      amount: AmountResolution::Fixed(99),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
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
    let contract_steps = contract_steps_with_step(make_step(Task::AddLiquidity {
      asset_a,
      asset_b,
      amount_a: AmountResolution::PercentageOfCurrent(Perbill::from_percent(1)),
      amount_b: AmountResolution::Fixed(50),
      min_lp_out: 1,
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_a, 2);
    set_asset_balance(&actor, asset_b, 50);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == actor_id
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
    let contract_steps = contract_steps_with_step(make_step(Task::Unstake {
      asset,
      shares: AmountResolution::Fixed(50),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 75);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 25);
    assert_eq!(unstaked_shares(actor, asset), 50);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::UnstakeExecuted { actor_id: id, asset: event_asset, shares, .. }
        if *id == actor_id && *event_asset == asset && *shares == 50
    )));
  });
}

#[test]
fn unstake_dynamic_modes_resolve_against_staking_shares() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(8);
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Unstake {
        asset,
        shares: AmountResolution::PercentageOfCurrent(Perbill::from_percent(25)),
      }),
      make_step(Task::Unstake {
        asset,
        shares: AmountResolution::PercentageAtOpening(Perbill::from_percent(50)),
      }),
    ])
    .expect("system execution plan fits");
    let actor_id = create_system_with(ALICE, percentage_trigger_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 100);
    signal_percentage_trigger(actor_id, asset);
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
    let contract_steps = contract_steps_with_step(make_step(Task::Unstake {
      asset,
      shares: AmountResolution::AllAvailable,
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 80);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
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
    let contract_steps = contract_steps_with_step(make_step(Task::Unstake {
      asset,
      shares: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    set_asset_balance(&ALICE, asset, 100);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      asset,
      100,
    ));
    let actor = sovereign_account(actor_id);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 50);
    assert_eq!(unstaked_shares(actor, asset), 50);
  });
}

#[test]
fn unstake_last_funding_rejects_position_without_transferable_share_asset() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Unstake {
      asset: TestAsset::Local(u32::MAX),
      shares: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    }));
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, contract_steps),
      ),
      Error::<Test>::InvalidAmountResolution
    );
  });
}

#[test]
fn unstake_last_funding_fails_closed_if_share_mapping_disappears_mid_lifetime() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(8);
    let contract_steps = contract_steps_with_step(make_step(Task::Unstake {
      asset,
      shares: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    set_asset_balance(&ALICE, asset, 100);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      asset,
      100,
    ));
    let actor = sovereign_account(actor_id);
    set_staking_share_asset_available(false);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(asset_balance(&actor, asset), 100);
    assert_eq!(unstaked_shares(actor, asset), 0);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
  });
}

#[test]
fn stake_adapter_failure_can_continue_next_step() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(13);
    let failing_step = StepOf::<Test> {
      precondition: None,
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
    let contract_steps = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 100);
    fund_native(actor_id, 20);
    set_fail_staking_ops(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 100);
    assert_eq!(staked_balance(actor, asset), 0);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 1, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn unstake_adapter_failure_aborts_cycle_without_partial_effects() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(14);
    let failing_step = StepOf::<Test> {
      precondition: None,
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
    let contract_steps = BoundedVec::try_from(vec![failing_step, skipped_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 100);
    fund_native(actor_id, 20);
    set_fail_staking_ops(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 100);
    assert_eq!(unstaked_shares(actor, asset), 0);
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 0, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn staking_adapter_late_failure_rolls_back_partial_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Native;
    let failing_step = StepOf::<Test> {
      precondition: None,
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
    let contract_steps = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 120);
    set_fail_staking_after_burn(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 110);
    assert_eq!(staked_balance(actor, asset), 0);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(!has_actor_event(|e| matches!(
      e,
      Event::StakeExecuted { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 1, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn unstake_adapter_late_failure_rolls_back_partial_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Native;
    let failing_step = StepOf::<Test> {
      precondition: None,
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
    let contract_steps = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 120);
    set_fail_staking_after_burn(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 110);
    assert_eq!(unstaked_shares(actor, asset), 0);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(!has_actor_event(|e| matches!(
      e,
      Event::UnstakeExecuted { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 1, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn donate_liquidity_task_delegates_to_liquidity_donation_adapter() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_a = TestAsset::Local(9);
    let asset_b = TestAsset::Local(10);
    let contract_steps = contract_steps_with_step(make_step(Task::DonateLiquidity {
      asset_a,
      asset_b,
      max_amount_a: AmountResolution::Fixed(40),
      max_ratio_error: Perbill::from_percent(1),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_a, 100);
    set_asset_balance(&actor, asset_b, 90);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset_a), 60);
    assert_eq!(asset_balance(&actor, asset_b), 50);
    assert_eq!(donated_liquidity(actor, asset_a, asset_b), (40, 40));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::LiquidityDonated {
        actor_id: id,
        asset_a: event_asset_a,
        asset_b: event_asset_b,
        max_amount_a,
        amount_a,
        amount_b,
        ..
      } if *id == actor_id
        && *event_asset_a == asset_a
        && *event_asset_b == asset_b
        && *max_amount_a == 40
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
    let contract_steps = contract_steps_with_step(make_step(Task::DonateLiquidity {
      asset_a,
      asset_b,
      max_amount_a: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
      max_ratio_error: Perbill::from_percent(1),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_a, 101);
    set_asset_balance(&actor, asset_b, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(donated_liquidity(actor, asset_a, asset_b), (50, 50));
    assert_eq!(asset_balance(&actor, asset_a), 51);
    assert_eq!(asset_balance(&actor, asset_b), 50);
  });
}

#[test]
fn donate_liquidity_asset_b_debit_is_capped_at_preservable_capacity() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // The resolved asset-a amount exceeds the preservable asset-b capacity, so the generic
    // contract must cap the paired debit at the smaller max_amount_b and report exact used
    // amounts without overdrawing the b-side or the fee-native protected floor.
    let asset_a = TestAsset::Local(81);
    let asset_b = TestAsset::Local(82);
    let contract_steps = contract_steps_with_step(make_step(Task::DonateLiquidity {
      asset_a,
      asset_b,
      max_amount_a: AmountResolution::PercentageOfCurrent(Perbill::from_percent(100)),
      max_ratio_error: Perbill::from_percent(1),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_a, 100);
    // Asset b holds less than the resolved a-side, and its protected floor keeps part reserved.
    set_asset_balance(&actor, asset_b, 60);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let (used_a, used_b) = donated_liquidity(actor, asset_a, asset_b);
    assert!(
      used_b <= 60,
      "asset-b debit must respect its preservable capacity"
    );
    assert_eq!(
      used_a, used_b,
      "balanced mock donation caps at the smaller side"
    );
    assert_eq!(asset_balance(&actor, asset_a), 100 - used_a);
    assert_eq!(asset_balance(&actor, asset_b), 60 - used_b);
  });
}

#[test]
fn donate_liquidity_user_fee_native_b_side_preserves_protected_floor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // A User actor donates with the fee-native asset as the paired asset-b side. The pallet
    // resolves the asset-b debit cap as the preservable capacity, so the paired donation must
    // never consume the fee-native protected floor (MinUserBalance = 50).
    let asset_a = TestAsset::Local(90);
    let asset_b = TestAsset::Native;
    let contract_steps = contract_steps_with_step(make_step(Task::DonateLiquidity {
      asset_a,
      asset_b,
      max_amount_a: AmountResolution::PercentageOfCurrent(Perbill::from_percent(100)),
      max_ratio_error: Perbill::from_percent(1),
    }));
    let fee = Actors::attempt_fee_envelope(ActorType::User, &contract_steps, 0)
      .expect("User fee envelope")
      .total;
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    let actor = sovereign_account(actor_id);
    let floor = TestMinUserBalance::get();
    set_asset_balance(&actor, asset_a, 1_000);
    // Fund the floor plus the reserved User fee plus a spendable donation budget.
    fund_native(actor_id, floor.saturating_add(fee).saturating_add(60));
    clear_fee_collections();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let (used_a, used_b) = donated_liquidity(actor, asset_a, asset_b);
    assert!(
      used_b <= 60,
      "asset-b fee-native debit must respect its preservable capacity"
    );
    assert_eq!(
      native_balance(&actor),
      floor
        .saturating_add(fee)
        .saturating_add(60)
        .saturating_sub(used_b)
        .saturating_sub(fee),
      "the fee-native protected floor is preserved by the asset-b cap"
    );
    assert!(native_balance(&actor) >= floor);
    assert_eq!(
      used_a, used_b,
      "balanced mock donation caps at the smaller side"
    );
  });
}

#[test]
fn donate_liquidity_asset_b_cap_keeps_capped_task_continuing() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_a = TestAsset::Local(11);
    let asset_b = TestAsset::Local(12);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::DonateLiquidity {
        asset_a,
        asset_b,
        max_amount_a: AmountResolution::Fixed(40),
        max_ratio_error: Perbill::from_percent(1),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let contract_steps = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_a, 100);
    set_asset_balance(&actor, asset_b, 10);
    fund_native(actor_id, 20);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    // The asset-b debit cap caps the balanced donation at the smaller preservable side instead
    // of overdrawing; the capped task succeeds and the cycle continues to the next step.
    assert_eq!(asset_balance(&actor, asset_a), 91);
    assert_eq!(asset_balance(&actor, asset_b), 1);
    assert_eq!(donated_liquidity(actor, asset_a, asset_b), (9, 9));
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::LiquidityDonated { actor_id: id, amount_a: 9, amount_b: 9, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::TransferExecuted { actor_id: id, to, amount, .. }
        if *id == actor_id && *to == CHARLIE && *amount == 10
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 2, failed_steps: 0, .. },
        ..
      } if *id == actor_id
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
      precondition: None,
      task: Task::DonateLiquidity {
        asset_a,
        asset_b,
        max_amount_a: AmountResolution::Fixed(40),
        max_ratio_error: Perbill::from_percent(1),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let skipped_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let contract_steps = BoundedVec::try_from(vec![failing_step, skipped_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_a, 100);
    set_asset_balance(&actor, asset_b, 100);
    fund_native(actor_id, 20);
    set_fail_liquidity_donation_ops(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset_a), 100);
    assert_eq!(asset_balance(&actor, asset_b), 100);
    assert_eq!(donated_liquidity(actor, asset_a, asset_b), (0, 0));
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 0, failed_steps: 1, .. },
        ..
      } if *id == actor_id
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
      precondition: None,
      task: Task::DonateLiquidity {
        asset_a,
        asset_b,
        max_amount_a: AmountResolution::Fixed(40),
        max_ratio_error: Perbill::from_percent(1),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let contract_steps = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_b, 100);
    fund_native(actor_id, 120);
    set_fail_liquidity_donation_after_first_burn(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 110);
    assert_eq!(asset_balance(&actor, asset_b), 100);
    assert_eq!(donated_liquidity(actor, asset_a, asset_b), (0, 0));
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(!has_actor_event(|e| matches!(
      e,
      Event::LiquidityDonated { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 1, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn condition_balance_below_skips_when_above() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceBelow {
        asset: TestAsset::Native,
        threshold: 50,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepSkipped { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
  });
}

#[test]
fn condition_balance_below_executes_when_below() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceBelow {
        asset: TestAsset::Native,
        threshold: 200,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before + 10);
  });
}

#[test]
fn condition_balance_equals_matches_exact() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceEquals {
        asset: TestAsset::Native,
        threshold: 100,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
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
      precondition: all_conditions(vec![Predicate::BalanceEquals {
        asset: TestAsset::Native,
        threshold: 999,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepSkipped { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
  });
}

#[test]
fn condition_balance_not_equals_executes_when_different() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceNotEquals {
        asset: TestAsset::Native,
        threshold: 999,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before + 10);
  });
}

#[test]
fn condition_balance_not_equals_skips_when_equal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceNotEquals {
        asset: TestAsset::Native,
        threshold: 100,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepSkipped { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
  });
}

#[test]
fn insufficient_fee_closes_cycle_immediately() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    }));
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    // Fund above MinUserBalance(50) but below fee threshold (eval=1 + exec=100 = 101)
    fund_native(actor_id, 60);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|e| matches!(
      e,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::FeeBudgetExhausted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn condition_sees_spendable_not_raw_balance_for_user_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let threshold = 350;
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    fund_native(actor_id, 400);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    // The raw balance exceeds the threshold, but the fee reservation leaves less spendable.
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepSkipped { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
  });
}

#[test]
fn condition_sees_full_balance_for_system_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // System Actors: reserved=0, so spendable == raw
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 150,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 200);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
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
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 100,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(step),
    );
    fund_native(actor_id, 200);
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("system actor")
      .sovereign_account;
    set_native_transfer_lock(&sovereign, 150);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepSkipped { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
  });
}

#[test]
fn user_condition_combines_adapter_lock_with_reserved_fee_budget() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 60,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    fund_native(actor_id, 300);
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("user actor")
      .sovereign_account;
    // A transfer lock reduces the native reducible balance below what the full fee
    // envelope needs above MinUserBalance, so the User attempt is not admitted and
    // the actor closes with FeeBudgetExhausted (spec 5.2.1) rather than running.
    set_native_transfer_lock(&sovereign, 150);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::FeeBudgetExhausted,
        ..
      } if *id == actor_id
    )));
  });
}

// --- Deterministic Timer Tests ---

#[test]
fn timer_always_executes_on_interval() {
  new_test_ext().execute_with(|| {
    let schedule = timer_schedule(5);
    let contract_steps = transfer_contract_steps(BOB, 10);
    let actor_id = create_system_with(ALICE, schedule, None, contract_steps);
    fund_native(actor_id, 1000);
    // Track executions via cycle_nonce changes (each cycle increments nonce)
    let mut last_cycle_nonce = 0u64;
    let mut execution_count = 0usize;
    for block in 2..22 {
      frame_system::Pallet::<Test>::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::MAX);
      if let Some(inst) = Actors::active_actor_view(actor_id) {
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
fn uninitialized_genesis_cadence_reanchors_without_first_service_execution() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(1), None, inert_contract_steps());
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    crate::ActorHot::<Test>::mutate(actor_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("active cadence actor")
        .cadence_anchor_tick = None;
    });

    frame_system::Pallet::<Test>::set_block_number(100);
    Actors::on_idle(100, Weight::MAX);

    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(instance.cycle_nonce, 0);
    assert!(!instance.pending_signal);
    assert_eq!(instance.cadence_anchor_tick, Some(100));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(101));
  });
}

#[test]
fn cadence_every_tick_rearms_one_future_tick_without_late_fifo_tickets() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(1), None, inert_contract_steps());
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    Actors::on_idle(1, Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      0
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_idle(2, Weight::MAX);
    let after_first = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(after_first.cycle_nonce, 1);
    assert!(after_first.queue_ticket.is_none());
    assert_eq!(scheduled_wakeup_block(actor_id), Some(3));
    for block in 3..=6 {
      frame_system::Pallet::<Test>::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::MAX);
      assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick), 1);
    }
    let cycle_nonce = Actors::active_actor_view(actor_id)
      .expect("Actors exists")
      .cycle_nonce;
    assert!(
      cycle_nonce >= 5,
      "every-block timer should keep progressing"
    );
  });
}

#[test]
fn paused_timer_waits_for_resume_without_queue_churn_or_signal_loss() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::cadenced(1),
      cooldown_blocks: 0,
    };
    let actor_id = create_system_with(ALICE, schedule, None, inert_contract_steps());
    run_idle(Weight::MAX);
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    assert_ok!(Actors::pause_actor(RuntimeOrigin::root(), actor_id));
    frame_system::Pallet::<Test>::set_block_number(6);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      0
    );
    assert_eq!(scheduled_wakeup_block(actor_id), None);
    assert!(
      Actors::actor_hot(actor_id)
        .expect("paused actor")
        .queue_ticket
        .is_none()
    );
    frame_system::Pallet::<Test>::set_block_number(7);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      0
    );
    assert_eq!(scheduled_wakeup_block(actor_id), Some(8));
    frame_system::Pallet::<Test>::set_block_number(8);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      1
    );
  });
}

#[test]
fn timer_jitter_removal_evidence_is_machine_readable_and_decisive() {
  let evidence: serde_json::Value = serde_json::from_str(include_str!(
    "../tests/fixtures/timer-jitter-decision.v1.json"
  ))
  .expect("timer-jitter decision fixture parses");
  assert_eq!(evidence["profile"]["actorIds"], "1..=10000");
  assert_eq!(evidence["zeroPhase"]["tailServiceBlock"], 7_429);
  assert_eq!(evidence["historicalJitter"]["tailServiceBlock"], 7_429);
  assert_eq!(evidence["zeroPhase"]["passExhaustedBlocks"], 6_666);
  assert_eq!(evidence["historicalJitter"]["passExhaustedBlocks"], 6_666);
  assert_eq!(evidence["decision"], "remove-timer-jitter");
}

#[test]
fn timer_wakeup_uses_exact_cadence_without_actor_phase() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let cadence = 20u32;
    let actor_id = create_system_with(ALICE, timer_schedule(cadence), None, inert_contract_steps());
    assert_eq!(scheduled_wakeup_block(actor_id), Some(21));

    frame_system::Pallet::<Test>::set_block_number(21);
    run_idle(Weight::MAX);
    assert_eq!(scheduled_wakeup_block(actor_id), Some(41));
  });
}

#[test]
fn timer_validation_accepts_the_exact_maximum_cadence() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_delay = TestMaxExecutionDelayBlocks::get() as u32;
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract(timer_schedule(max_delay), None, inert_contract_steps()),
    ));
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(
          timer_schedule(max_delay.saturating_add(1)),
          None,
          inert_contract_steps(),
        ),
      ),
      Error::<Test>::ExecutionDelayTooLong
    );
  });
}

// --- User Actors E2E Lifecycle Tests ---

#[test]
fn user_dca_complete_lifecycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // Step 1: Create User Actors with Timer trigger
    let schedule = timer_schedule(5);
    let foreign = TestAsset::Local(1);
    set_asset_balance(&ALICE, foreign, 10_000);
    let contract_steps = contract_steps_with_step(StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: foreign,
        threshold: 50,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: foreign,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    });
    let actor_id = Actors::next_actor_id();
    prefund_active_user_creation(ALICE, &contract_steps);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(schedule, None, contract_steps),
    ));
    // Verify creation
    assert!(has_actor_event(|e| matches!(
      e,
      Event::ActorCreated { actor_id: id, actor_class: ActorClass::User { .. }, .. } if *id == actor_id
    )));
    let instance = Actors::active_actor_view(actor_id).expect("instance exists");
    assert_eq!(instance.owner, ALICE);
    // Step 2: Fund sovereign
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, foreign, 500);
    fund_native(actor_id, 500); // For fees
    // Step 3-4: Advance blocks and verify execution
    for block in 2..7 {
      frame_system::Pallet::<Test>::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::MAX);
    }
    assert!(
      has_actor_event(|e| matches!(
        e,
        Event::TransferExecuted { actor_id: id, .. } if *id == actor_id
      )),
      "Should execute transfer on timer"
    );
    // Step 5: Multiple cycles
    let bob_before = asset_balance(&BOB, foreign);
    for block in 7..27 {
      frame_system::Pallet::<Test>::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::MAX);
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
    // Sweep cursor iterates MaxSweepBatch=3 IDs per block; run enough blocks
    for block in 30..50 {
      frame_system::Pallet::<Test>::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::MAX);
      if Actors::active_actor_view(actor_id).is_none() {
        break;
      }
    }
    assert!(
      Actors::active_actor_view(actor_id).is_none(),
      "User Actors must be destroyed by sweep when native < MinUserBalance"
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
    // ContractSteps: SwapIn(foreign → native) → Transfer(native → cold wallet)
    let contract_steps = BoundedVec::try_from(vec![
      StepOf::<Test> {
        precondition: all_conditions(vec![Predicate::BalanceAbove {
          asset: foreign,
          threshold: 50,
        }]),
        task: Task::SwapIn {
          asset_in: foreign,
          asset_out: TestAsset::Native,
          amount_in: AmountResolution::Fixed(100),
          slippage_tolerance: Perbill::from_percent(10),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
      StepOf::<Test> {
        precondition: all_conditions(vec![Predicate::BalanceAbove {
          asset: TestAsset::Native,
          threshold: 10,
        }]),
        task: Task::Transfer {
          to: cold_wallet,
          asset: TestAsset::Native,
          amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(80)),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
    ])
    .unwrap();
    let actor_id = create_user_with(ALICE, Mutability::Mutable, schedule, None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, foreign, 1000);
    fund_native(actor_id, 5000);
    let cold_before = native_balance(&cold_wallet);
    frame_system::Pallet::<Test>::set_block_number(6);
    Actors::on_initialize(6);
    Actors::on_idle(6, Weight::MAX);
    assert!(
      native_balance(&cold_wallet) > cold_before,
      "Cold storage should receive native after swap + transfer"
    );
    assert!(
      has_actor_event(|e| matches!(
        e,
        Event::SwapExecuted { actor_id: id, .. } if *id == actor_id
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
    let contract_steps = contract_steps_with_step(StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 100,
      }]),
      task: Task::Transfer {
        to: savings,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(5)),
      },
      on_error: StepErrorPolicy::AbortCycle,
    });
    let actor_id = create_user_with(ALICE, Mutability::Mutable, schedule, None, contract_steps);
    fund_native(actor_id, 10000);
    let initial_savings = native_balance(&savings);
    // Execute multiple cycles
    for block in 2..32 {
      frame_system::Pallet::<Test>::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::MAX);
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
    // Full two-step attempt fee envelope ≈ 2*(1+100) = 202
    // BalanceBelow checks spendable = raw - fee_reserve
    let contract_steps = BoundedVec::try_from(vec![
      StepOf::<Test> {
        precondition: all_conditions(vec![Predicate::BalanceAbove {
          asset: TestAsset::Native,
          threshold: 5000,
        }]),
        task: Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(20)),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      StepOf::<Test> {
        precondition: all_conditions(vec![
          Predicate::BalanceBelow {
            asset: TestAsset::Native,
            threshold: 500,
          },
          Predicate::BalanceAbove {
            asset: foreign,
            threshold: 500,
          },
        ]),
        task: Task::Transfer {
          to: CHARLIE,
          asset: foreign,
          amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
    ])
    .unwrap();
    let actor_id = create_user_with(ALICE, Mutability::Mutable, schedule, None, contract_steps);
    let actor = sovereign_account(actor_id);
    // First evaluation: native high → step 0 fires, step 1 skipped
    fund_native(actor_id, 10000);
    set_asset_balance(&actor, foreign, 2000);
    frame_system::Pallet::<Test>::set_block_number(6);
    Actors::on_initialize(6);
    Actors::on_idle(6, Weight::MAX);
    assert!(
      has_actor_event(|e| matches!(
        e,
        Event::TransferExecuted { actor_id: id, asset: TestAsset::Native, to, .. }
        if *id == actor_id && *to == BOB
      )),
      "Step 0 should execute when spendable native > 5000"
    );
    // Second evaluation: slash native so spendable < 500, keep raw > fee_reserve (202)
    // Set raw native to 600: spendable = 600 - 202 = 398 < 500 ✓
    let actor_native = native_balance(&actor);
    let _ = <Balances as Currency<AccountId>>::slash(&actor, actor_native.saturating_sub(600));
    let charlie_before = asset_balance(&CHARLIE, foreign);
    frame_system::Pallet::<Test>::set_block_number(11);
    Actors::on_initialize(11);
    Actors::on_idle(11, Weight::MAX);
    assert!(
      asset_balance(&CHARLIE, foreign) > charlie_before,
      "Step 1 should execute when spendable native < 500 AND foreign > 500"
    );
  });
}

// --- Multi-Asset Funding Tests ---

#[test]
fn multi_asset_contract_steps_tracks_all_referenced_assets() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let foreign_a = TestAsset::Local(1);
    let foreign_b = TestAsset::Local(2);
    // ContractSteps references both assets via PercentageOfLastFunding
    let contract_steps = BoundedVec::try_from(vec![
      StepOf::<Test> {
        precondition: None,
        task: Task::Transfer {
          to: BOB,
          asset: foreign_a,
          amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(10)),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
      StepOf::<Test> {
        precondition: None,
        task: Task::Transfer {
          to: CHARLIE,
          asset: foreign_b,
          amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(20)),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
    ])
    .unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let funding = actor_funding(actor_id);
    // Verify both assets are tracked
    assert!(funding.funding_tracked_assets.contains(&foreign_a));
    assert!(funding.funding_tracked_assets.contains(&foreign_b));
  });
}

#[test]
fn manual_readiness_mutates_hot_state_without_rewriting_contract() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let contract_before = Actors::actor_contract(actor_id).expect("actor contract exists");

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    assert!(
      Actors::actor_hot(actor_id)
        .expect("actor hot state exists")
        .pending_signal
    );
    assert_eq!(Actors::actor_contract(actor_id), Some(contract_before));
  });
}

#[test]
fn funding_ingress_mutates_only_actor_funding_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    let instance_before = Actors::active_actor_view(actor_id).expect("Actors exists");
    let hot_before = Actors::actor_hot(actor_id).expect("actor hot state exists");
    let contract_before = Actors::actor_contract(actor_id).expect("actor contract exists");
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      100,
    ));
    assert_eq!(Actors::active_actor_view(actor_id), Some(instance_before));
    assert_eq!(Actors::actor_hot(actor_id), Some(hot_before));
    assert_eq!(Actors::actor_contract(actor_id), Some(contract_before));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&100)
    );
  });
}

#[test]
fn funding_accumulator_isolated_per_asset() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let foreign = TestAsset::Local(1);
    let contract_steps = contract_steps_with_step(StepOf::<Test> {
      precondition: None,
      task: Task::Transfer {
        to: BOB,
        asset: foreign,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
      },
      on_error: StepErrorPolicy::AbortCycle,
    });
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    // Mint on ALICE before the ordinary transfer to the sovereign account
    set_asset_balance(&ALICE, foreign, 1000);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      foreign,
      1000
    ));
    // Verify accumulation.
    let funding = actor_funding(actor_id);
    assert_eq!(funding.funding_accumulated.get(&foreign), Some(&1000));
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
    let contract_steps = BoundedVec::try_from(vec![
      StepOf::<Test> {
        precondition: None,
        task: Task::Transfer {
          to: BOB,
          asset: foreign_a,
          amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(10)),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      StepOf::<Test> {
        precondition: None,
        task: Task::Transfer {
          to: CHARLIE,
          asset: foreign_b,
          amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(20)),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
    ])
    .unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    // Mint both assets on ALICE before ordinary transfers to the sovereign account
    set_asset_balance(&ALICE, foreign_a, 1000);
    set_asset_balance(&ALICE, foreign_b, 500);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      foreign_a,
      1000
    ));
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      foreign_b,
      500
    ));
    // Execute
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
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
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset,
        amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(10)),
      })),
    );
    let sovereign = sovereign_account(actor_id);
    set_asset_balance(&sovereign, asset, 1_000);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&BOB, asset), 99);
    assert_eq!(asset_balance(&sovereign, asset), 901);
  });
}

#[test]
fn scheduler_ignores_sparse_id_gaps() {
  // Sparse Actors IDs must not create a scheduler "shadow zone".
  // Create Actors at ID 0, bump NextActorId to 2000 (huge gap), create Actors at ID 2000.
  // Both must execute in the first block.
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let schedule = timer_schedule(1);
    let contract_steps = inert_contract_steps();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract(schedule.clone(), None, contract_steps.clone()),
    ));
    let sov_0 = Actors::sovereign_account_id_system(0);
    let _ = Balances::deposit_creating(&sov_0, 1_000_000);
    // Bump NextActorId to create 2000-wide gap
    crate::pallet::NextActorId::<Test>::put(2000u64);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract(schedule, None, contract_steps),
    ));
    let sov_2000 = Actors::sovereign_account_id_system(2000);
    let _ = Balances::deposit_creating(&sov_2000, 1_000_000);
    assert_eq!(Actors::next_actor_id(), 2001);
    assert!(Actors::active_actor_view(0).is_some());
    assert!(Actors::active_actor_view(2000).is_some());
    // Run one block: both actors must execute despite 2000-wide ID gap
    System::set_block_number(2);
    System::reset_events();
    Actors::on_idle(2, Weight::from_parts(u64::MAX, u64::MAX));
    let executed: alloc::vec::Vec<_> = System::events()
      .iter()
      .filter_map(|r| {
        if let RuntimeEvent::Actors(Event::CycleSummary { actor_id, .. }) = &r.event {
          Some(*actor_id)
        } else {
          None
        }
      })
      .collect();
    assert!(
      executed.contains(&0),
      "ID 0 must execute despite sparse Actors IDs"
    );
    assert!(
      executed.contains(&2000),
      "ID 2000 must execute despite sparse Actors IDs"
    );
  });
}

#[test]
fn active_actors_set_maintains_integrity() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let schedule = timer_schedule(1);
    let inert_plan = inert_contract_steps();
    for _ in 0..3 {
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(schedule.clone(), None, inert_plan.clone()),
      ));
    }
    assert_eq!(ActorHot::<Test>::iter_keys().count(), 3);
    assert!(Actors::active_actor_view(0).is_some());
    assert!(Actors::active_actor_view(1).is_some());
    assert!(Actors::active_actor_view(2).is_some());
    let inst = Actors::active_actor_view(1).unwrap();
    let _ = Balances::deposit_creating(&inst.sovereign_account, 1_000_000);
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), 1));
    assert_eq!(ActorHot::<Test>::iter_keys().count(), 2);
    assert!(Actors::active_actor_view(0).is_some());
    assert!(Actors::active_actor_view(2).is_some());
    assert!(Actors::active_actor_view(1).is_none());
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
      inert_contract_steps(),
    );
    deplete_user_sovereign(
      close_id,
      user_prefunding_requirement(&inert_contract_steps()),
    );
    let live_id_1 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let live_id_2 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    fund_native(live_id_1, 1_000);
    fund_native(live_id_2, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      close_id
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      live_id_1
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      live_id_2
    ));
    System::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(close_id).is_none());
    assert_eq!(
      Actors::active_actor_view(live_id_1)
        .expect("live actor")
        .cycle_nonce,
      1
    );
    assert_eq!(
      Actors::active_actor_view(live_id_2)
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
      inert_contract_steps(),
    );
    let id1 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let id2 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let id3 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    deplete_user_sovereign(id3, user_prefunding_requirement(&inert_contract_steps()));
    fund_native(id0, 1_000);
    fund_native(id1, 1_000);
    fund_native(id2, 1_000);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id0));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id1));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id2));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id3));
    System::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(id3).is_some());
    assert_eq!(
      Actors::active_actor_view(id0)
        .expect("id0 live")
        .cycle_nonce,
      1
    );
    assert_eq!(
      Actors::active_actor_view(id1)
        .expect("id1 live")
        .cycle_nonce,
      1
    );
    assert_eq!(
      Actors::active_actor_view(id2)
        .expect("id2 executed")
        .cycle_nonce,
      1
    );
    System::set_block_number(3);
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(id3).is_none());
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
          inert_contract_steps(),
        ),
        create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          inert_contract_steps(),
        ),
        create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          inert_contract_steps(),
        ),
      ];
      for (idx, actor_id) in ids.iter().enumerate() {
        deplete_user_sovereign(
          *actor_id,
          user_prefunding_requirement(&inert_contract_steps()),
        );
        if (funded_mask & (1 << idx)) != 0 {
          fund_native(*actor_id, 1_000);
        }
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          *actor_id
        ));
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
        .filter(|record| {
          matches!(
            record.event,
            RuntimeEvent::Actors(Event::CycleStarted { .. })
          )
        })
        .count() as u32;
      assert_eq!(started, expected_started);
      for (idx, actor_id) in ids.iter().enumerate() {
        if (funded_mask & (1 << idx)) != 0 {
          assert_eq!(
            Actors::active_actor_view(*actor_id)
              .expect("funded actor")
              .cycle_nonce,
            1
          );
        } else {
          assert!(Actors::active_actor_view(*actor_id).is_none());
        }
      }
    });
  }
}

#[test]
fn auto_close_threshold_reached_closes_actor_after_successful_cycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(replace_auto_close(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      Some(2),
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_some());
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::AutoCloseNonceReached,
        } if *id == actor_id
      )
    }));
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
      inert_contract_steps(),
    );
    assert_noop!(
      replace_auto_close(RuntimeOrigin::signed(ALICE), immutable_id, Some(2)),
      Error::<Test>::ImmutableActor
    );
    let mutable_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_noop!(
      replace_auto_close(RuntimeOrigin::signed(BOB), mutable_id, Some(2)),
      Error::<Test>::NotOwner
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      mutable_id
    ));
    run_idle(Weight::MAX);
    assert_noop!(
      replace_auto_close(RuntimeOrigin::signed(ALICE), mutable_id, Some(1)),
      Error::<Test>::InvalidAutoCloseNonce
    );
    let horizon = TestMaxAutoCloseNonceHorizon::get();
    assert_noop!(
      replace_auto_close(
        RuntimeOrigin::signed(ALICE),
        mutable_id,
        Some(1u64.saturating_add(horizon).saturating_add(1)),
      ),
      Error::<Test>::AutoCloseNonceHorizonExceeded
    );
    let boundary_target = 1u64.saturating_add(horizon);
    assert_ok!(replace_auto_close(
      RuntimeOrigin::signed(ALICE),
      mutable_id,
      Some(boundary_target),
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_noop!(
      replace_auto_close(
        RuntimeOrigin::signed(ALICE),
        mutable_id,
        Some(boundary_target.saturating_add(1)),
      ),
      Error::<Test>::AutoCloseNonceHorizonExceeded
    );
    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(replace_auto_close(
      RuntimeOrigin::signed(ALICE),
      mutable_id,
      None,
    ));
    assert_eq!(
      Actors::active_actor_view(mutable_id)
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
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(replace_auto_close(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      Some(1),
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(starvation_blocked_budget(actor_id));
    let inst = Actors::active_actor_view(actor_id).expect("Actors must exist");
    assert_eq!(inst.cycle_nonce, 0);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. } | Event::CycleSummary { actor_id: id, .. }
        if *id == actor_id
    )));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
  });
}

#[test]
fn system_immutable_rejects_runtime_control_paths_even_for_root() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = inert_contract_steps();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Immutable,
      system_active_contract(timer_schedule(1), None, contract_steps.clone()),
    ));
    let actor_id = Actors::next_actor_id().saturating_sub(1);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .mutability,
      Mutability::Immutable
    );
    assert_noop!(
      update_contract_partial!(RuntimeOrigin::root(), actor_id, timer_schedule(2), None),
      Error::<Test>::ImmutableActor
    );
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::root(),
        actor_id,
        contract_steps.clone(),
        crate::CompletionPolicy::CloseAfterProductiveCycle,
      ),
      Error::<Test>::ImmutableActor
    );
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ImmutableActor
    );
    assert_noop!(
      Actors::resume_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ImmutableActor
    );
    assert_noop!(
      Actors::manual_trigger(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ImmutableActor
    );
    assert_noop!(
      Actors::close_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ImmutableActor
    );
    assert_noop!(
      Actors::create_system_actor_at_sovereign_id(
        RuntimeOrigin::root(),
        actor_id,
        ALICE,
        Mutability::Immutable,
        system_active_contract(manual_schedule(), None, inert_contract_steps()),
      ),
      Error::<Test>::SystemSovereignOccupied
    );
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert_eq!(
      Actors::system_sovereigns(actor_id),
      Some(SystemSovereignState::Occupied(actor_id))
    );
  });
}

#[test]
fn system_immutable_indefinite_commitment_survives_breaker_mitigation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Immutable,
      system_active_contract(timer_schedule(10), None, inert_contract_steps()),
    ));
    let actor_id = Actors::next_actor_id().saturating_sub(1);
    let before = Actors::active_actor_view(actor_id).expect("immutable actor");
    assert!(Actors::attempt_weight_upper_bound(&before, 0).ref_time() > 0);
    assert_eq!(
      Actors::system_sovereigns(actor_id),
      Some(SystemSovereignState::Occupied(actor_id)),
    );

    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    frame_system::Pallet::<Test>::set_block_number(100);
    run_idle(Weight::MAX);

    let after = Actors::active_actor_view(actor_id).expect("breaker cannot remove commitment");
    assert_eq!(after.sovereign_account, before.sovereign_account);
    assert_eq!(after.steps, before.steps);
    assert_eq!(after.cycle_nonce, 0);
    assert_eq!(
      Actors::system_sovereigns(actor_id),
      Some(SystemSovereignState::Occupied(actor_id)),
    );
    assert_noop!(
      Actors::close_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ImmutableActor
    );
  });
}

#[test]
fn system_immutable_creation_rejects_manual_but_allows_internal_window_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Immutable,
        system_active_contract(manual_schedule(), None, inert_contract_steps()),
      ),
      Error::<Test>::InvalidTriggerConfiguration
    );
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Immutable,
      system_active_contract(
        on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
        Some(ScheduleWindow { start: 1, end: 101 }),
        inert_contract_steps(),
      ),
    ));
    let actor_id = Actors::next_actor_id().saturating_sub(1);
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert_noop!(
      Actors::close_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ImmutableActor
    );
    assert!(Actors::active_actor_view(actor_id).is_some());
    frame_system::Pallet::<Test>::set_block_number(102);
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed { actor_id: id, reason: CloseReason::WindowExpired } if *id == actor_id
    )));
  });
}

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
      CloseReason::BalanceExhausted => {}
      CloseReason::FeeBudgetExhausted => fund_native(actor_id, 60),
      CloseReason::CycleNonceExhausted => {
        fund_native(actor_id, 1_000);
        ActorIdentities::<Test>::mutate(actor_id, |maybe| {
          maybe.as_mut().expect("actor identity exists").cycle_nonce = u64::MAX;
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
        ActorContracts::<Test>::mutate(actor_id, |maybe| {
          maybe
            .as_mut()
            .expect("actor contract exists")
            .auto_close_at_cycle_nonce = Some(1);
        });
      }
      unsupported => panic!("unsupported admission-time close reason: {unsupported:?}"),
    }
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    if reason == CloseReason::WindowExpired {
      ActorContracts::<Test>::mutate(actor_id, |maybe| {
        maybe.as_mut().expect("actor contract exists").window =
          Some(ScheduleWindow { start: 0, end: 0 });
      });
    }
    let before = Actors::actor_hot(actor_id)
      .unwrap_or_else(|| panic!("{reason:?} actor remains active before scheduler admission"));
    let queue_head = QueueHead::<Test>::get();
    frame_system::Pallet::<Test>::reset_events();
    let discovery = <TestWeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1);
    // Wakeups drain in the on_idle phase before execute_cycle, so the execute_cycle
    // admission budget covers only the queue discovery and actor probes plus the close.
    let pre_admission = discovery
      .saturating_add(Actors::scheduler_actor_hot_probe_weight_upper())
      .saturating_add(Actors::scheduler_actor_contract_probe_weight_upper());
    let close = Actors::close_cleanup_weight_upper();
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

#[test]
fn admission_time_close_reasons_require_complete_queue_and_cleanup_budget() {
  let reasons = [
    CloseReason::WindowExpired,
    CloseReason::BalanceExhausted,
    CloseReason::FeeBudgetExhausted,
    CloseReason::CycleNonceExhausted,
    CloseReason::ConsecutiveFailures,
    CloseReason::AutoCloseNonceReached,
  ];
  for reason in reasons {
    assert_scheduler_close_requires_atomic_budget(reason, Weight::from_parts(1, 0));
    assert_scheduler_close_requires_atomic_budget(reason, Weight::from_parts(0, 1));
  }
}

#[test]
fn close_cleanup_admission_uses_measured_pure_close_weight() {
  new_test_ext().execute_with(|| {
    let measured = <TestWeightInfo as crate::WeightInfo>::close_actor();
    assert!(measured.ref_time() > 0);
    assert!(measured.proof_size() > 0);
    assert_eq!(Actors::close_cleanup_weight_upper(), measured);
    assert_eq!(Actors::close_dispatch_weight_upper(), measured);
  });
}

#[test]
fn pure_close_does_not_start_normal_cycle_or_execute_tasks() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let sovereign = sovereign_account(actor_id);
    fund_native_raw(&sovereign, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let before_close_nonce = Actors::active_actor_view(actor_id)
      .expect("Actors exists")
      .cycle_nonce;
    assert_eq!(before_close_nonce, 1);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(!has_actor_event(|event| {
      matches!(event, Event::CycleStarted { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::CycleSummary { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(has_actor_event(|event| {
      matches!(event, Event::ActorClosed { actor_id: id, .. } if *id == actor_id)
    }));
  });
}

#[cfg(test)]
mod proptest_actor {
  use super::Schedule;
  use super::{
    RETRY_LATER, all_conditions, asset_balance, create_system_with, fund_native, make_step,
    manual_schedule, native_balance, prefund_active_user_creation, run_idle, set_asset_balance,
    setup_pool, setup_temporary_retry_pool, sovereign_account,
  };
  use crate::{
    ActorContracts, ActorFunding, ActorHot, ActorIdentities, AmountResolution, AssetFilter,
    ContinuationStateStore, CycleState, Event, FundingSourcePolicy, Mutability, QueueOccupancy,
    QueuePages, SourceFilter, StepErrorPolicy, StepOf, SystemSovereignState, SystemSovereigns,
    Task, Trigger, WakeupBuckets, WakeupPages, mock::*,
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
        Actors::continuation_state(actor_id)
          .expect("suspended")
          .cursor,
        1
      );
      assert_eq!(native_balance(&actor), 80);
      assert_eq!(native_balance(&BOB), bob_before);
      let output_after_prefix = asset_balance(&actor, TestAsset::Local(77));
      assert!(output_after_prefix > 0);

      frame_system::Pallet::<Test>::set_block_number(2);
      run_idle(Weight::MAX);
      assert_eq!(
        Actors::continuation_state(actor_id)
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
      frame_system::Pallet::<Test>::set_block_number(4);
      run_idle(Weight::MAX);
      assert!(Actors::continuation_state(actor_id).is_none());
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
        Actors::continuation_state(actor_id).expect("suspended").cursor,
        1
      );

      frame_system::Pallet::<Test>::set_block_number(2);
      run_idle(Weight::MAX);
      assert_eq!(native_balance(&actor), 90);
      assert_eq!(
        Actors::continuation_state(actor_id).expect("same cursor").cursor,
        1
      );

      assert_ok!(Actors::cancel_continuation(RuntimeOrigin::root(), actor_id));
      assert_eq!(native_balance(&actor), 90);
      assert!(Actors::continuation_state(actor_id).is_none());
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
  }

  fn model_op() -> impl Strategy<Value = ModelOp> {
    (0u8..18).prop_map(|index| match index {
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
      _ => ModelOp::Cancel,
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
    let contract_ids: std::collections::BTreeSet<_> = ActorContracts::<Test>::iter_keys().collect();
    let funding_ids: std::collections::BTreeSet<_> = ActorFunding::<Test>::iter_keys().collect();
    let identity_ids: std::collections::BTreeSet<_> =
      ActorIdentities::<Test>::iter_keys().collect();
    let dormant_ids: std::collections::BTreeSet<_> =
      identity_ids.difference(&hot_ids).copied().collect();
    let continuation_ids: std::collections::BTreeSet<_> =
      ContinuationStateStore::<Test>::iter_keys().collect();
    assert_eq!(hot_ids, contract_ids);
    assert_eq!(hot_ids, funding_ids);
    assert!(continuation_ids.is_subset(&hot_ids));
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
        hot.cycle_state == CycleState::Suspended,
        continuation_ids.contains(actor_id)
      );
      let funding = ActorFunding::<Test>::get(actor_id).expect("funding key resolves");
      assert!(
        funding
          .funding_accumulated
          .keys()
          .all(|asset| funding.funding_tracked_assets.contains(asset))
      );
      if let Some(continuation) = ContinuationStateStore::<Test>::get(actor_id) {
        let contract = ActorContracts::<Test>::get(actor_id).expect("contract key resolves");
        assert!((continuation.cursor as usize) < contract.steps.len());
        assert!(continuation.cumulative_outcomes.executed_steps <= continuation.cursor);
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
    fn seeded_continuation_state_machine_preserves_cross_store_and_scheduler_invariants(
      operations in prop::collection::vec(model_op(), 1..80),
    ) {
      new_test_ext().execute_with(|| {
        use polkadot_sdk::frame_support::traits::{Currency, ExistenceRequirement};

        frame_system::Pallet::<Test>::set_block_number(1);
        setup_temporary_retry_pool();
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
          let before_continuation = ContinuationStateStore::<Test>::get(system_id);
          let before_funding = ActorFunding::<Test>::get(system_id);
          let before_system_balance = Balances::free_balance(system_sovereign);
          let before_bob_balance = Balances::free_balance(BOB);

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
            ModelOp::Continue if ContinuationStateStore::<Test>::contains_key(system_id) => {
              set_temporary_dex_failure(false);
              let _ = Actors::on_idle(block, Weight::MAX);
            }
            ModelOp::Cancel if ContinuationStateStore::<Test>::contains_key(system_id) => {
              let _ = Actors::cancel_continuation(RuntimeOrigin::root(), system_id);
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
          if !closed
            && !ActorHot::<Test>::contains_key(system_id)
            && !ActorIdentities::<Test>::contains_key(system_id)
            && SystemSovereigns::<Test>::get(system_id) == Some(SystemSovereignState::Vacant)
          {
            closed = true;
          }

          let after_continuation = ContinuationStateStore::<Test>::get(system_id);
          if matches!(operation, ModelOp::Cancel) && before_continuation.is_some() {
            assert!(after_continuation.is_none());
            assert_eq!(Balances::free_balance(system_sovereign), before_system_balance);
            assert_eq!(Balances::free_balance(BOB), before_bob_balance);
            assert_eq!(
              ActorFunding::<Test>::get(system_id).as_ref().map(Encode::encode),
              before_funding.as_ref().map(Encode::encode)
            );
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
        max_age_blocks: 5,
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
  if let Some(continuation) = Actors::continuation_state(actor_id) {
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

#[test]
fn canonical_step_transition_matrix_has_production_simulation_parity() {
  let mut covered_rows = BTreeSet::new();
  let mut covered_policies = BTreeSet::new();
  let mut covered_mutability = BTreeSet::new();
  let mut covered_actor_types = BTreeSet::new();
  let mut covered_outcome_variants = BTreeSet::new();
  for case in STEP_TRANSITION_PARITY_MATRIX {
    covered_rows.insert(case.row);
    covered_policies.insert(case.policy.encode());
    covered_mutability.insert(case.mutability.encode());
    covered_actor_types.insert(case.actor_type.encode());
    let target_outcome = parity_expected_steps(*case)
      .into_iter()
      .find(|record| record.step_index == 1)
      .expect("matrix target outcome exists")
      .outcome;
    covered_outcome_variants.insert(
      *target_outcome
        .encode()
        .first()
        .expect("StepOutcome encoding carries a variant index"),
    );
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(10);
      set_max_consecutive_failures(if case.bound == StepParityBound::GlobalReached {
        1
      } else {
        10
      });
      set_observation(
        1,
        crate::ScalarObservationState::Fresh {
          value: 50,
          observed_at: 11,
        },
      );
      setup_temporary_retry_pool();
      set_temporary_dex_failure(case.stimulus == StepParityStimulus::TemporaryFailure);
      set_fail_dex_after_input_transfer(case.stimulus == StepParityStimulus::PermanentFailure);
      let steps = parity_contract_steps(*case);
      let schedule = if case.stimulus == StepParityStimulus::PredicateError {
        observation_schedule(vec![1])
      } else if case.actor_type == ActorType::System && case.mutability == Mutability::Immutable {
        timer_schedule(1)
      } else {
        manual_schedule()
      };
      let expected_contract = match case.actor_type {
        ActorType::User => user_active_contract(schedule.clone(), None, steps.clone()),
        ActorType::System => system_active_contract(schedule.clone(), None, steps.clone()),
      }
      .expect("direct Actor Contract");
      let actor_id = match case.actor_type {
        ActorType::User => create_user_with(ALICE, case.mutability, schedule, None, steps),
        ActorType::System => {
          create_system_with_mutability(ALICE, case.mutability, schedule, None, steps)
        }
      };
      fund_native(
        actor_id,
        if case.stimulus == StepParityStimulus::FundingUnavailable {
          8
        } else {
          100
        },
      );
      if case.actor_type == ActorType::System && case.mutability == Mutability::Immutable {
        frame_system::Pallet::<Test>::set_block_number(11);
        ActorHot::<Test>::mutate(actor_id, |maybe| {
          maybe
            .as_mut()
            .expect("parity actor hot state exists")
            .pending_signal = true;
        });
        Actors::enqueue(actor_id).expect("parity actor enqueues");
      } else if case.stimulus == StepParityStimulus::PredicateError {
        ActorHot::<Test>::mutate(actor_id, |maybe| {
          maybe
            .as_mut()
            .expect("parity actor hot state exists")
            .pending_signal = true;
        });
        Actors::enqueue(actor_id).expect("parity actor enqueues");
      } else {
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          actor_id
        ));
      }
      if case.bound == StepParityBound::LocalReached {
        run_idle(Weight::MAX);
        assert_eq!(
          Actors::continuation_state(actor_id).map(|state| state.cursor),
          Some(1)
        );
        frame_system::Pallet::<Test>::set_block_number(11);
      }
      let mode = if case.bound == StepParityBound::LocalReached {
        SimulationMode::CurrentContinuation
      } else {
        SimulationMode::FreshCurrentPlan
      };
      System::reset_events();
      let actor = sovereign_account(actor_id);
      let streak_before = Actors::active_actor_view(actor_id)
        .expect("matrix actor remains active before production")
        .unsuccessful_attempt_streak;
      let before = (
        Actors::active_actor_view(actor_id).map(|view| view.encode()),
        Actors::continuation_state(actor_id).map(|state| state.encode()),
        native_balance(&actor),
        native_balance(&BOB),
        native_balance(&CHARLIE),
        native_balance(&TestFeeSink::get()),
        native_balance(&u64::MAX),
        asset_balance(&actor, TestAsset::Local(77)),
        asset_balance(&u64::MAX, TestAsset::Local(77)),
        System::events(),
      );
      let simulation = Actors::simulate_current_contract(
        actor_id,
        case.actor_type,
        case.mutability,
        expected_contract,
        mode,
      )
      .unwrap_or_else(|error| panic!("{} simulation failed: {error:?}", case.name));
      let after_simulation = (
        Actors::active_actor_view(actor_id).map(|view| view.encode()),
        Actors::continuation_state(actor_id).map(|state| state.encode()),
        native_balance(&actor),
        native_balance(&BOB),
        native_balance(&CHARLIE),
        native_balance(&TestFeeSink::get()),
        native_balance(&u64::MAX),
        asset_balance(&actor, TestAsset::Local(77)),
        asset_balance(&u64::MAX, TestAsset::Local(77)),
        System::events(),
      );
      assert_eq!(
        after_simulation, before,
        "{} simulation persisted state or custody",
        case.name
      );
      assert_eq!(
        simulation.steps.as_slice(),
        parity_expected_steps(*case),
        "{} trace",
        case.name
      );

      clear_fee_collections();
      System::reset_events();
      let bob_before = native_balance(&BOB);
      let charlie_before = native_balance(&CHARLIE);
      let actor_before = native_balance(&actor);
      let sink_before = native_balance(&TestFeeSink::get());
      let pool_native_before = native_balance(&u64::MAX);
      run_idle(Weight::MAX);
      let (status, outcomes, cursor, attempts) = observed_attempt_projection(actor_id);
      assert_eq!(status, simulation.status, "{} disposition", case.name);
      assert_eq!(
        outcomes, simulation.cumulative_outcomes,
        "{} outcomes",
        case.name
      );
      assert_eq!(
        cursor, simulation.continuation_cursor,
        "{} cursor",
        case.name
      );
      assert_eq!(
        attempts, simulation.unsuccessful_attempts_at_cursor,
        "{} local attempts",
        case.name
      );
      match status {
        AttemptDisposition::Completed => assert_eq!(
          Actors::active_actor_view(actor_id)
            .expect("completed persistent matrix actor remains active")
            .unsuccessful_attempt_streak,
          0,
          "{} completed attempt resets the failure streak",
          case.name,
        ),
        AttemptDisposition::Failed | AttemptDisposition::Suspended => assert_eq!(
          Actors::active_actor_view(actor_id)
            .expect("failed or suspended matrix actor remains active")
            .unsuccessful_attempt_streak,
          streak_before
            .checked_add(1)
            .expect("matrix streak remains bounded"),
          "{} unsuccessful attempt increments the failure streak once",
          case.name,
        ),
        AttemptDisposition::Closed(_) => assert!(
          Actors::active_actor_view(actor_id).is_none(),
          "{} closed disposition removes active state",
          case.name,
        ),
      }

      let advances = parity_advances(*case);
      let prefix_runs = case.bound != StepParityBound::LocalReached;
      let target_transfer = case.stimulus == StepParityStimulus::SuccessfulTask;
      let expected_bob: Balance =
        (if prefix_runs { 2 } else { 0 }) + (if target_transfer { 5 } else { 0 });
      let expected_charlie: Balance = if advances { 3 } else { 0 };
      assert_eq!(
        native_balance(&BOB),
        bob_before + expected_bob,
        "{} committed prefix/target",
        case.name
      );
      assert_eq!(
        native_balance(&CHARLIE),
        charlie_before + expected_charlie,
        "{} suffix boundary",
        case.name
      );
      if matches!(
        case.stimulus,
        StepParityStimulus::TemporaryFailure | StepParityStimulus::PermanentFailure
      ) {
        assert_eq!(
          native_balance(&u64::MAX),
          pool_native_before,
          "{} failed task custody rollback",
          case.name
        );
      }
      let fee_delta = native_balance(&TestFeeSink::get()).saturating_sub(sink_before);
      match case.actor_type {
        ActorType::User => {
          assert!(
            fee_delta > 0,
            "{} must charge its committed attempt",
            case.name
          );
          assert_eq!(
            fee_collections().iter().copied().sum::<Balance>(),
            fee_delta,
            "{} fee accounting",
            case.name
          );
        }
        ActorType::System => assert_eq!(fee_delta, 0, "{} System fee exemption", case.name),
      }
      assert_eq!(
        actor_before.saturating_sub(native_balance(&actor)),
        expected_bob + expected_charlie + fee_delta,
        "{} reservation, fee, and custody conservation",
        case.name,
      );
      set_temporary_dex_failure(false);
      set_fail_dex_after_input_transfer(false);
    });
  }
  assert_eq!(
    covered_rows,
    BTreeSet::from([
      "ST-01", "ST-02", "ST-03", "ST-04", "ST-05", "ST-06", "ST-07", "ST-08", "ST-09", "ST-10",
      "ST-11", "ST-12", "ST-13"
    ]),
    "matrix inventory must remain closed against specification section 3.4",
  );
  assert_eq!(
    covered_policies.len(),
    variant_count::<StepErrorPolicy>(),
    "every StepErrorPolicy variant must be represented",
  );
  assert_eq!(
    covered_mutability.len(),
    variant_count::<Mutability>(),
    "every Mutability variant must be represented",
  );
  assert_eq!(
    covered_actor_types.len(),
    variant_count::<ActorType>(),
    "every ActorType variant must be represented",
  );
  assert_eq!(
    covered_outcome_variants.len(),
    variant_count::<StepOutcome>(),
    "every canonical StepOutcome variant must be represented",
  );
}

#[test]
fn fresh_current_plan_simulation_returns_runtime_trace_and_rolls_back_every_write() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = transfer_contract_steps(BOB, 10);
    let expected_contract = system_active_contract(manual_schedule(), None, contract_steps.clone())
      .expect("direct Actor Contract");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    let actor_before = Actors::active_actor_view(actor_id).expect("actor exists");
    let actor_balance_before = native_balance(&actor_before.sovereign_account);
    let bob_before = native_balance(&BOB);
    let event_count_before = frame_system::Pallet::<Test>::event_count();
    let result = Actors::simulate_current_contract(
      actor_id,
      ActorType::System,
      Mutability::Mutable,
      expected_contract,
      SimulationMode::FreshCurrentPlan,
    )
    .expect("ready current plan simulates");

    assert_eq!(result.status, AttemptDisposition::Completed);
    assert_eq!(result.cycle_nonce, 1);
    assert_eq!(result.start_cursor, 0);
    assert_eq!(result.continuation_cursor, None);
    assert_eq!(result.unsuccessful_attempts_at_cursor, None);
    assert_eq!(result.cumulative_outcomes.executed_steps, 1);
    assert_eq!(
      result.steps.as_slice(),
      &[SimulationStepRecord {
        step_index: 0,
        outcome: StepOutcome::Executed,
      }]
    );
    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(
      native_balance(
        &Actors::active_actor_view(actor_id)
          .expect("actor remains")
          .sovereign_account
      ),
      actor_balance_before
    );
    assert_eq!(
      frame_system::Pallet::<Test>::event_count(),
      event_count_before
    );
    assert!(Actors::continuation_state(actor_id).is_none());
  });
}

#[test]
fn continuation_simulation_preserves_retry_position_and_committed_state() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let expected_contract =
      system_active_contract(manual_schedule(), None, temporary_retry_swap_plan())
        .expect("direct Actor Contract");
    let continuation_before = Actors::continuation_state(actor_id).expect("continuation exists");
    let actor_before = Actors::active_actor_view(actor_id).expect("actor exists");
    let events_before = frame_system::Pallet::<Test>::event_count();
    frame_system::Pallet::<Test>::set_block_number(2);

    let result = Actors::simulate_current_contract(
      actor_id,
      ActorType::System,
      Mutability::Mutable,
      expected_contract,
      SimulationMode::CurrentContinuation,
    )
    .expect("eligible continuation simulates");

    assert_eq!(result.status, AttemptDisposition::Suspended);
    assert_eq!(result.cycle_nonce, actor_before.cycle_nonce);
    assert_eq!(result.start_cursor, continuation_before.cursor);
    assert_eq!(result.continuation_cursor, Some(continuation_before.cursor));
    assert_eq!(
      result.unsuccessful_attempts_at_cursor,
      Some(
        continuation_before
          .unsuccessful_attempts_at_cursor
          .saturating_add(1)
      )
    );
    assert_eq!(result.cumulative_outcomes.failed_steps, 2);
    assert_eq!(
      result.steps.as_slice(),
      &[SimulationStepRecord {
        step_index: 0,
        outcome: StepOutcome::Failed(TaskFailure::temporary(DispatchError::Other(
          "TemporaryDexCapacity"
        ))),
      }]
    );
    assert_eq!(
      Actors::continuation_state(actor_id).map(|state| state.encode()),
      Some(continuation_before.encode())
    );
    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(frame_system::Pallet::<Test>::event_count(), events_before);
  });
}

#[test]
fn simulation_projects_first_retry_suspension_and_rolls_back_actor_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let contract_steps = contract_steps_with_step(StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 2 },
    });
    let expected_contract = system_active_contract(manual_schedule(), None, contract_steps.clone())
      .expect("direct Actor Contract");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let actor_before = Actors::active_actor_view(actor_id).expect("actor exists");
    let events_before = frame_system::Pallet::<Test>::event_count();

    let result = Actors::simulate_current_contract(
      actor_id,
      ActorType::System,
      Mutability::Mutable,
      expected_contract,
      SimulationMode::FreshCurrentPlan,
    )
    .expect("retry exhaustion simulates");

    assert_eq!(result.status, AttemptDisposition::Suspended);
    assert_eq!(result.continuation_cursor, Some(0));
    assert_eq!(result.unsuccessful_attempts_at_cursor, Some(1));
    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(frame_system::Pallet::<Test>::event_count(), events_before);
  });
}

#[test]
fn simulation_rejects_contract_and_mode_mismatch_without_execution() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = transfer_contract_steps(BOB, 10);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps.clone());
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, contract_steps.clone())
          .expect("direct Actor Contract"),
        SimulationMode::FreshCurrentPlan,
      )
      .err(),
      Some(SimulationError::NotReady)
    );
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let events_before = frame_system::Pallet::<Test>::event_count();

    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        ActorContract {
          cooldown_blocks: 1,
          ..system_active_contract(manual_schedule(), None, contract_steps.clone())
            .expect("direct Actor Contract")
        },
        SimulationMode::FreshCurrentPlan,
      )
      .err(),
      Some(SimulationError::ContractMismatch)
    );
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, contract_steps)
          .expect("direct Actor Contract"),
        SimulationMode::CurrentContinuation,
      )
      .err(),
      Some(SimulationError::ModeCycleStateMismatch)
    );
    assert_eq!(frame_system::Pallet::<Test>::event_count(), events_before);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor exists")
        .cycle_nonce,
      0
    );
  });
}

// --- Eligibility Projection API (spec 7.3) ---

fn eligibility(actor_id: ActorId) -> ActorEligibility<u64> {
  Actors::actor_eligibility(actor_id).expect("eligibility computes")
}

fn active_eligibility(actor_id: ActorId) -> ActorClassification<u64> {
  match eligibility(actor_id) {
    ActorEligibility::Active(classification) => classification,
    other => panic!("expected active eligibility, got {other:?}"),
  }
}

#[test]
fn eligibility_projection_reports_not_registered_and_dormant() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let fresh_id = Actors::next_actor_id();
    assert_eq!(eligibility(fresh_id), ActorEligibility::NotRegistered);

    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None,
    ));
    assert_eq!(eligibility(fresh_id), ActorEligibility::Dormant);
  });
}

#[test]
fn eligibility_projection_rejects_partial_active_partitions() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let plan = inert_contract_steps();
    let expected_contract =
      system_active_contract(manual_schedule(), None, plan.clone()).expect("direct Actor Contract");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    ActorFunding::<Test>::remove(actor_id);
    assert_eq!(
      Actors::actor_eligibility(actor_id),
      Err(ActorClassificationError::ActorInvariant)
    );
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        expected_contract,
        SimulationMode::FreshCurrentPlan,
      ),
      Err(SimulationError::Classification(
        ActorClassificationError::ActorInvariant
      ))
    );
  });
}

#[test]
fn classifier_projections_agree_on_breaker_terminal_and_paused_products() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let window = ScheduleWindow { start: 1, end: 101 };
    let contract_steps = inert_contract_steps();
    let expected_contract =
      system_active_contract(manual_schedule(), Some(window), contract_steps.clone())
        .expect("direct Actor Contract");
    let actor_id = create_system_with(ALICE, manual_schedule(), Some(window), contract_steps);
    frame_system::Pallet::<Test>::set_block_number(102);
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    let instance = Actors::active_actor_view(actor_id).expect("actor exists");
    let classification = Actors::classify_actor(actor_id, &instance).expect("classification");
    assert_eq!(
      classification.terminal_reason,
      Some(CloseReason::WindowExpired)
    );
    assert_eq!(
      classification.execution_phase,
      ActorExecutionPhase::GlobalCircuitBreaker
    );
    assert_eq!(active_eligibility(actor_id), classification);
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        expected_contract.clone(),
        SimulationMode::FreshCurrentPlan,
      ),
      Err(SimulationError::GlobalCircuitBreaker)
    );

    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      false
    ));
    assert_eq!(
      active_eligibility(actor_id).terminal_reason,
      Some(CloseReason::WindowExpired)
    );
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        expected_contract,
        SimulationMode::FreshCurrentPlan,
      )
      .expect("terminal simulation projects")
      .status,
      AttemptDisposition::Closed(CloseReason::WindowExpired)
    );
    assert_ok!(Actors::permissionless_sweep(
      RuntimeOrigin::signed(BOB),
      actor_id
    ));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::WindowExpired,
      } if *id == actor_id
    )));
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = inert_contract_steps();
    let expected_contract = system_active_contract(manual_schedule(), None, contract_steps.clone())
      .expect("direct Actor Contract");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    assert_ok!(Actors::pause_actor(RuntimeOrigin::root(), actor_id));
    let instance = Actors::active_actor_view(actor_id).expect("actor exists");
    assert_eq!(
      Actors::classify_actor(actor_id, &instance)
        .expect("classification")
        .execution_phase,
      ActorExecutionPhase::Paused
    );
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::Paused
    );
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        expected_contract,
        SimulationMode::FreshCurrentPlan,
      ),
      Err(SimulationError::Paused)
    );
  });
}

#[test]
fn eligibility_projection_ready_after_signal_and_waits_without_latch() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingSignal
    );

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::Ready
    );
  });
}

#[test]
fn eligibility_projection_waits_for_cooldown_and_reports_next_block() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 5,
    };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule,
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 2_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      1
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingBlock(6)
    );
  });
}

#[test]
fn eligibility_projection_waits_for_schedule_window_until_gate() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow {
        start: 50,
        end: 150,
      }),
      inert_contract_steps(),
    );
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingBlock(50)
    );

    frame_system::Pallet::<Test>::set_block_number(50);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::Ready
    );
  });
}

#[test]
fn eligibility_projection_reports_exact_cadence_gate_without_actor_phase() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let cadence = 20u32;
    let actor_id = create_system_with(ALICE, timer_schedule(cadence), None, inert_contract_steps());

    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingCadenceTick(21)
    );

    frame_system::Pallet::<Test>::set_block_number(11);
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingCadenceTick(21)
    );
  });
}

#[test]
fn eligibility_projection_reports_paused_breaker_and_window_expired() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::Paused
    );
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::GlobalCircuitBreaker
    );
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow {
        start: 50,
        end: 150,
      }),
      inert_contract_steps(),
    );
    frame_system::Pallet::<Test>::set_block_number(151);
    assert_eq!(
      active_eligibility(actor_id).terminal_reason,
      Some(CloseReason::WindowExpired)
    );
  });
}

#[test]
fn eligibility_projection_reports_suspended_retry_then_ready_at_attempt_block() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingRetry(2)
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::Ready
    );
  });
}

#[test]
fn eligibility_projection_reports_failure_limit_auto_close_and_nonce_exhaustion() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    ActorHot::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("active actor")
        .unsuccessful_attempt_streak = <Test as crate::Config>::MaxConsecutiveFailures::get();
    });
    assert_eq!(
      active_eligibility(actor_id).terminal_reason,
      Some(CloseReason::ConsecutiveFailures)
    );
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor")
        .cycle_nonce,
      1
    );
    // A failed terminal cycle leaves the incremented nonce at the target without
    // closing; the next admission closes before any further cycle (spec 2.4).
    ActorContracts::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("active Actor Contract")
        .auto_close_at_cycle_nonce = Some(1);
    });
    assert_eq!(
      active_eligibility(actor_id).terminal_reason,
      Some(CloseReason::AutoCloseNonceReached)
    );
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    ActorIdentities::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("identity").cycle_nonce = u64::MAX;
    });
    assert_eq!(
      active_eligibility(actor_id).terminal_reason,
      Some(CloseReason::CycleNonceExhausted)
    );
  });
}
