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

#[benchmarks]
mod benches {
  use super::*;

  const CROSSING_COHORT_BENCHMARK_MAX: u32 = 128;
  const CROSSING_NON_TAIL_BENCHMARK_MAX: u32 = 64;
  const CROSSING_TRIMMED_BENCHMARK_TAIL: u32 = CROSSING_NON_TAIL_BENCHMARK_MAX + 2;

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

  fn current_all<T: Config>(
    predicates: alloc::vec::Vec<Predicate<T::AssetId, T::Balance, u32, T::ObservationFeedId>>,
  ) -> PreconditionOf<T> {
    let clause = BoundedVec::try_from(
      predicates
        .into_iter()
        .map(|predicate| TimedPredicate {
          timing: ObservationTiming::Current,
          predicate,
        })
        .collect::<alloc::vec::Vec<_>>(),
    )
    .expect("benchmark predicates fit");
    Precondition {
      clauses: BoundedVec::try_from(alloc::vec![clause]).expect("clause fits"),
    }
  }

  fn opening_all<T: Config>(
    predicates: alloc::vec::Vec<Predicate<T::AssetId, T::Balance, u32, T::ObservationFeedId>>,
  ) -> PreconditionOf<T> {
    let clause = BoundedVec::try_from(
      predicates
        .into_iter()
        .map(|predicate| TimedPredicate {
          timing: ObservationTiming::Opening,
          predicate,
        })
        .collect::<alloc::vec::Vec<_>>(),
    )
    .expect("benchmark Opening predicates fit");
    Precondition {
      clauses: BoundedVec::try_from(alloc::vec![clause]).expect("Opening clause fits"),
    }
  }

  fn current_any<T: Config>(
    predicates: alloc::vec::Vec<Predicate<T::AssetId, T::Balance, u32, T::ObservationFeedId>>,
  ) -> PreconditionOf<T> {
    let clauses = predicates
      .into_iter()
      .map(|predicate| {
        BoundedVec::try_from(alloc::vec![TimedPredicate {
          timing: ObservationTiming::Current,
          predicate,
        }])
        .expect("benchmark predicate fits")
      })
      .collect::<alloc::vec::Vec<_>>();
    Precondition {
      clauses: BoundedVec::try_from(clauses).expect("clauses fit"),
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

  /// Marks `feeds` dirty in list order. Ingress appends to the tail, so passing a slice ordered as
  /// `[guard_low, measured.., guard_high]` leaves every measured feed a middle node. Must run after
  /// the measured actor exists, because ingress skips feeds with no subscriber.
  fn mark_observation_chain_dirty<T: Config>(feeds: &[T::ObservationFeedId]) {
    for feed in feeds {
      Pallet::<T>::note_observation_changed(*feed, 1)
        .expect("observation change ingress must succeed for a subscribed feed");
    }
    assert_eq!(
      DirtyObservationListState::<T>::get().count,
      feeds.len() as u32,
      "every prepared observation feed must be dirty"
    );
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
    assert!(ActorIdentities::<T>::contains_key(actor_id));
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
    assert!(ActorIdentities::<T>::contains_key(actor_id));
    assert_max_contract_geometry::<T>(actor_id);
    assert!(CrossingMemberships::<T>::contains_key(actor_id));
    assert!(ActorObservationFeeds::<T>::get(actor_id).is_none());
  }

  #[benchmark]
  fn deactivate_actor() {
    let owner: T::AccountId = whitelisted_caller();
    let recipient: T::AccountId = account("deactivate-recipient", 0, 0);
    let contract_steps = make_max_contract_steps::<T>(recipient);
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
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    #[extrinsic_call]
    deactivate_actor(RawOrigin::Signed(owner), actor_id);
    assert!(!Pallet::<T>::active_actor_exists(actor_id));
    assert!(ActorIdentities::<T>::contains_key(actor_id));
    assert!(
      ActorContractTailChunks::<T>::iter_prefix(actor_id)
        .next()
        .is_none()
    );
    assert!(!CrossingMemberships::<T>::contains_key(actor_id));
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
    let hot = ActorHot::<T>::get(actor_id).expect("AddressEvent Actor remains active");
    assert!(hot.pending_signal);
    assert!(hot.queue_ticket.is_some() || hot.wakeup_pointer.is_some());
  }

  /// Measures the exact lifecycle-only cleanup selected when an Idle User cannot admit a paid
  /// pending Pipeline. The maximum Contract is retained, while Crossing, ObservationChange,
  /// temporal membership, and Run state are intentionally absent from this apoptosis branch.
  #[benchmark]
  fn pipeline_admission_apoptosis() {
    let owner: T::AccountId = whitelisted_caller();
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
  fn close_actor() {
    let owner: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&owner);
    let owner_slot = prefill_owner_slots_for_worst_case::<T>(&owner);
    let recipient: T::AccountId = account("close-recipient", 0, 0);
    let feed = observation_feed_pool::<T>(1)[0];
    let schedule = Schedule {
      trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, u128::MAX, 0),
      cooldown_blocks: 1,
    };
    let contract_steps = make_max_contract_steps::<T>(recipient);
    prefund_user_sovereign::<T>(&owner, owner_slot, &contract_steps);
    Pallet::<T>::create_user_actor_at_slot(
      RawOrigin::Signed(owner.clone()).into(),
      owner_slot,
      Mutability::Mutable,
      user_contract::<T>(schedule, contract_steps),
    )
    .expect("create_user_actor_at_slot must succeed in close_actor benchmark setup");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let locator = CrossingMemberships::<T>::get(actor_id).expect("Crossing membership exists");
    Pallet::<T>::note_observation_transition(
      feed,
      ObservationTransition {
        revision: 2,
        previous: Some(1),
        current: 2,
      },
    )
    .expect("pending Crossing transition must be admitted");
    CrossingRangeCursors::<T>::insert(
      feed,
      CrossingRangeCursor {
        revision: 2,
        traversal: locator.key.traversal,
        search_bound: 2,
        current_threshold: None,
        page: 0,
        offset: 0,
        exhausted: false,
      },
    );
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
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
  }

  // Diagnostic removal branch: delete the last page while the Crossing leaf survives.
  #[benchmark]
  fn close_actor_crossing_page() {
    let owner: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&owner);
    let feed = observation_feed_pool::<T>(1)[0];
    let threshold = u128::MAX;
    for index in 0..T::CrossingPageSize::get() {
      let guard_owner: T::AccountId = account("crossing-remove-page-guard", index, 0);
      let _ = bench_create_system_crossing::<T>(guard_owner, feed, threshold);
    }
    let owner_slot = prefill_owner_slots_for_worst_case::<T>(&owner);
    let recipient: T::AccountId = account("crossing-remove-page-recipient", 0, 0);
    let contract_steps = make_contract_steps::<T>(recipient);
    prefund_user_sovereign::<T>(&owner, owner_slot, &contract_steps);
    let schedule = Schedule {
      trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, threshold, 0),
      cooldown_blocks: 1,
    };
    Pallet::<T>::create_user_actor_at_slot(
      RawOrigin::Signed(owner.clone()).into(),
      owner_slot,
      Mutability::Mutable,
      user_contract::<T>(schedule, contract_steps),
    )
    .expect("page-removal User setup must succeed");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let removed = CrossingMemberships::<T>::get(actor_id).expect("tail-page membership exists");
    assert_eq!(removed.page, 1);
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
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
  }

  // Diagnostic removal branch: remove the tail member while its leaf page survives.
  #[benchmark]
  fn close_actor_crossing_tail() {
    let owner: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&owner);
    let feed = observation_feed_pool::<T>(1)[0];
    let threshold = u128::MAX;
    let guard_owner: T::AccountId = account("crossing-tail-guard", 0, 0);
    let guard = bench_create_system_crossing::<T>(guard_owner, feed, threshold);
    let owner_slot = prefill_owner_slots_for_worst_case::<T>(&owner);
    let recipient: T::AccountId = account("crossing-tail-recipient", 0, 0);
    let contract_steps = make_contract_steps::<T>(recipient);
    prefund_user_sovereign::<T>(&owner, owner_slot, &contract_steps);
    let schedule = Schedule {
      trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, threshold, 0),
      cooldown_blocks: 1,
    };
    Pallet::<T>::create_user_actor_at_slot(
      RawOrigin::Signed(owner.clone()).into(),
      owner_slot,
      Mutability::Mutable,
      user_contract::<T>(schedule, contract_steps),
    )
    .expect("tail-removal User setup must succeed");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let removed = CrossingMemberships::<T>::get(actor_id).expect("tail membership exists");
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
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
  }

  // Diagnostic removal branch: repair an in-progress range cursor after dense compaction.
  #[benchmark]
  fn close_actor_crossing_cursor_repair() {
    let owner: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&owner);
    let feed = observation_feed_pool::<T>(1)[0];
    let threshold = u128::MAX;
    let guard_owner: T::AccountId = account("crossing-cursor-guard", 0, 0);
    let _ = bench_create_system_crossing::<T>(guard_owner, feed, threshold);
    let owner_slot = prefill_owner_slots_for_worst_case::<T>(&owner);
    let recipient: T::AccountId = account("crossing-cursor-recipient", 0, 0);
    let contract_steps = make_contract_steps::<T>(recipient);
    prefund_user_sovereign::<T>(&owner, owner_slot, &contract_steps);
    let schedule = Schedule {
      trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, threshold, 0),
      cooldown_blocks: 1,
    };
    Pallet::<T>::create_user_actor_at_slot(
      RawOrigin::Signed(owner.clone()).into(),
      owner_slot,
      Mutability::Mutable,
      user_contract::<T>(schedule, contract_steps),
    )
    .expect("cursor-repair User setup must succeed");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let tail_owner: T::AccountId = account("crossing-cursor-tail", 0, 0);
    let _ = bench_create_system_crossing::<T>(tail_owner, feed, threshold);
    let removed = CrossingMemberships::<T>::get(actor_id).expect("middle membership exists");
    CrossingRangeCursors::<T>::insert(
      feed,
      CrossingRangeCursor {
        revision: 1,
        traversal: removed.key.traversal,
        search_bound: threshold,
        current_threshold: Some(threshold),
        page: removed.page,
        offset: removed.offset.saturating_add(1),
        exhausted: false,
      },
    );
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    #[block]
    {
      Pallet::<T>::close_actor(RawOrigin::Signed(owner).into(), actor_id)
        .expect("cursor-repair Crossing close must succeed");
    }
    let cursor = CrossingRangeCursors::<T>::get(feed).expect("range cursor survives");
    assert_eq!(cursor.page, removed.page);
    assert_eq!(cursor.offset, removed.offset);
  }

  // Diagnostic removal branch: remove a dense middle member and repair the moved tail locator.
  #[benchmark]
  fn close_actor_crossing_middle() {
    let owner: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&owner);
    let feed = observation_feed_pool::<T>(1)[0];
    let threshold = u128::MAX;
    let guard_owner: T::AccountId = account("crossing-middle-guard", 0, 0);
    let _ = bench_create_system_crossing::<T>(guard_owner, feed, threshold);
    let owner_slot = prefill_owner_slots_for_worst_case::<T>(&owner);
    let recipient: T::AccountId = account("crossing-middle-recipient", 0, 0);
    let contract_steps = make_contract_steps::<T>(recipient);
    prefund_user_sovereign::<T>(&owner, owner_slot, &contract_steps);
    let schedule = Schedule {
      trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, threshold, 0),
      cooldown_blocks: 1,
    };
    Pallet::<T>::create_user_actor_at_slot(
      RawOrigin::Signed(owner.clone()).into(),
      owner_slot,
      Mutability::Mutable,
      user_contract::<T>(schedule, contract_steps),
    )
    .expect("middle-removal User setup must succeed");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    let tail_owner: T::AccountId = account("crossing-middle-tail", 0, 0);
    let tail_id = bench_create_system_crossing::<T>(tail_owner, feed, threshold);
    let removed = CrossingMemberships::<T>::get(actor_id).expect("middle membership exists");
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    #[block]
    {
      Pallet::<T>::close_actor(RawOrigin::Signed(owner).into(), actor_id)
        .expect("middle Crossing close must succeed");
    }
    assert!(!CrossingMemberships::<T>::contains_key(actor_id));
    let moved = CrossingMemberships::<T>::get(tail_id).expect("moved tail membership exists");
    assert_eq!(moved.page, removed.page);
    assert_eq!(moved.offset, removed.offset);
  }

  // Diagnostic counterpart for broad ObservationChange cleanup; compare it with the production
  // Crossing close before accepting one conservative public close owner.
  #[benchmark]
  fn close_actor_observation_change() {
    let owner: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&owner);
    let owner_slot = prefill_owner_slots_for_worst_case::<T>(&owner);
    let recipient: T::AccountId = account("close-observation-recipient", 0, 0);
    let feeds = observation_feed_pool::<T>(3);
    let measured = feeds[1];
    install_observation_guard::<T>(feeds[0], 0);
    install_observation_guard::<T>(feeds[feeds.len() - 1], 1);
    let schedule = Schedule {
      trigger: observation_trigger::<T>(measured),
      cooldown_blocks: 1,
    };
    let contract_steps = make_contract_steps::<T>(recipient);
    prefund_user_sovereign::<T>(&owner, owner_slot, &contract_steps);
    Pallet::<T>::create_user_actor_at_slot(
      RawOrigin::Signed(owner.clone()).into(),
      owner_slot,
      Mutability::Mutable,
      user_contract::<T>(schedule, contract_steps),
    )
    .expect("ObservationChange close benchmark setup must succeed");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    mark_observation_chain_dirty::<T>(&feeds);
    #[block]
    {
      Pallet::<T>::close_actor(RawOrigin::Signed(owner).into(), actor_id)
        .expect("ObservationChange close must succeed");
    }
    assert!(!Pallet::<T>::active_actor_exists(actor_id));
    assert_eq!(DirtyObservationListState::<T>::get().count, 2);
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
  fn update_contract_observation_change() {
    let caller: T::AccountId = whitelisted_caller();
    // Diagnostic broad branch replaces one dirty middle-node subscription with a disjoint feed.
    // Pool layout: [slot donor, guard_low, replaced, installed, guard_high].
    let feeds = observation_feed_pool::<T>(5);
    let guard_high = feeds[4];
    seed_recycled_observation_slot::<T>(feeds[0]);
    install_observation_guard::<T>(feeds[1], 0);
    install_observation_guard::<T>(guard_high, 1);
    let replaced = feeds[2];
    let installed = feeds[3];
    let actor_id =
      bench_create_user_with_trigger::<T>(caller.clone(), observation_trigger::<T>(replaced));
    let dirty_chain = alloc::vec![feeds[1], replaced, guard_high];
    mark_observation_chain_dirty::<T>(&dirty_chain);
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    ActorHot::<T>::mutate(actor_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("benchmark actor hot state exists")
        .pending_signal = true;
    });
    let funding_assets = T::BenchmarkHelper::funding_assets(T::MaxFundingTrackedAssets::get());
    ActorFunding::<T>::mutate(actor_id, |maybe| {
      let funding = maybe.as_mut().expect("benchmark actor funding exists");
      for asset in funding_assets {
        funding
          .funding_accumulated
          .try_insert(asset, One::one())
          .expect("funding accumulator benchmark bound fits");
      }
    });
    let recipient = account("recipient", 0, 0);
    let replacement = make_contract_steps::<T>(recipient);
    let mut allowed: BoundedBTreeSet<T::AccountId, T::MaxWhitelistSize> =
      BoundedBTreeSet::default();
    for index in 0..T::MaxWhitelistSize::get() {
      allowed
        .try_insert(account("funding-source", index, 0))
        .expect("funding source must fit benchmark bound");
    }
    let funding = FundingSourcePolicy::SignedAllowlist(allowed);
    #[extrinsic_call]
    update_contract(
      RawOrigin::Signed(caller),
      actor_id,
      ActorContract {
        trigger: observation_trigger::<T>(installed),
        cooldown_blocks: 20,
        window: None,
        steps: replacement.clone(),
        funding,
        completion: CompletionPolicy::Persistent,
        auto_close_at_cycle_nonce: None,
      },
    );
    assert_eq!(
      ActorObservationFeeds::<T>::get(actor_id).map(|feeds| feeds.to_vec()),
      Some(alloc::vec![installed]),
      "measured update must replace the one observation source"
    );
    let inst =
      Pallet::<T>::active_actor_view(actor_id).expect("Actors must exist after update_contract");
    assert_eq!(inst.steps, replacement);
    assert_eq!(inst.cooldown_blocks, 20);
    assert!(
      ActorFunding::<T>::get(actor_id)
        .expect("actor funding exists")
        .funding_accumulated
        .is_empty()
    );
  }

  // Production update owner: focused comparison proves unique-leaf Crossing replacement dominates
  // the excluded broad ObservationChange branch in RefTime, ProofSize, reads, and writes.
  #[benchmark]
  fn update_contract() {
    let caller: T::AccountId = whitelisted_caller();
    let feed = observation_feed_pool::<T>(1)[0];
    let old_recipient: T::AccountId = account("crossing-update-old-recipient", 0, 0);
    let actor_id = bench_create_user_with_trigger_and_steps::<T>(
      caller.clone(),
      Trigger::observation_crossing(feed, CrossingDirection::Rising, u128::MAX - 1, 0),
      make_max_contract_steps::<T>(old_recipient),
    );
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    ActorHot::<T>::mutate(actor_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("benchmark actor hot state exists")
        .pending_signal = true;
    });
    let funding_assets = T::BenchmarkHelper::funding_assets(T::MaxFundingTrackedAssets::get());
    ActorFunding::<T>::mutate(actor_id, |maybe| {
      let funding = maybe.as_mut().expect("benchmark actor funding exists");
      for asset in funding_assets {
        funding
          .funding_accumulated
          .try_insert(asset, One::one())
          .expect("funding accumulator benchmark bound fits");
      }
    });
    let recipient = account("crossing-update-recipient", 0, 0);
    let replacement = make_max_contract_steps::<T>(recipient);
    let mut allowed: BoundedBTreeSet<T::AccountId, T::MaxWhitelistSize> =
      BoundedBTreeSet::default();
    for index in 0..T::MaxWhitelistSize::get() {
      allowed
        .try_insert(account("crossing-funding-source", index, 0))
        .expect("funding source must fit benchmark bound");
    }
    let funding = FundingSourcePolicy::SignedAllowlist(allowed);
    #[block]
    {
      Pallet::<T>::update_contract(
        RawOrigin::Signed(caller).into(),
        actor_id,
        ActorContract {
          trigger: Trigger::observation_crossing(feed, CrossingDirection::Rising, u128::MAX, 0),
          cooldown_blocks: 20,
          window: None,
          steps: replacement,
          funding,
          completion: CompletionPolicy::Persistent,
          auto_close_at_cycle_nonce: None,
        },
      )
      .expect("Crossing update must succeed");
    }
    let locator =
      CrossingMemberships::<T>::get(actor_id).expect("Crossing replacement membership must exist");
    assert_max_contract_geometry::<T>(actor_id);
    assert_eq!(locator.key.threshold, u128::MAX);
    assert!(ActorObservationFeeds::<T>::get(actor_id).is_none());
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
      ActorIdentities::<T>::mutate(actor_id, |maybe| {
        maybe
          .as_mut()
          .expect("benchmark actor identity exists")
          .cycle_nonce = u64::MAX;
      });
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
    let payer: T::AccountId = whitelisted_caller();
    let owner: T::AccountId = account("fee-sink-owner", 0, 0);
    let schedule = Schedule {
      trigger: Trigger::cadenced(1),
      cooldown_blocks: 0,
    };
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      system_contract::<T>(schedule, make_inert_contract_steps::<T>()),
    )
    .expect("fee-collection benchmark sink must be created");
    let fee_sink_id = NextActorId::<T>::get().saturating_sub(1);
    let fee_sink = Pallet::<T>::sovereign_account_id_system(fee_sink_id);
    let native = T::FeeNativeAssetId::get();
    let amount = T::MinUserBalance::get().saturating_add(One::one());
    T::AssetOps::mint(&payer, native, amount.saturating_mul(2u32.into()))
      .expect("fee-collection benchmark payer must be funded");
    #[block]
    {
      T::FeeCollector::collect_fee(&payer, &fee_sink, native, amount)
        .expect("fee collection must succeed");
    }
    assert!(T::AssetOps::balance(&fee_sink, native) >= amount);
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
    assert!(ActorHot::<T>::get(target_id).is_some_and(|hot| hot.wakeup_pointer.is_some()));
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
    assert!(ActorHot::<T>::get(target_id).is_some_and(|hot| hot.wakeup_pointer.is_some()));
  }

  #[benchmark]
  fn precondition_all_max() {
    let actor: T::AccountId = account("condition-all", 0, 0);
    let max_predicates = T::MaxPredicatesPerStep::get();
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
    let precondition = current_all::<T>(predicates);
    #[block]
    {
      assert_eq!(
        Pallet::<T>::evaluate_precondition(&precondition, &actor, T::Balance::zero()),
        Ok(true)
      );
    }
  }

  #[benchmark]
  fn precondition_observation(c: Linear<1, 4>) {
    let actor: T::AccountId = account("condition-observation", 0, 0);
    let bounded = c.min(T::MaxPredicatesPerStep::get());
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
    let precondition = current_all::<T>(predicates);
    #[block]
    {
      let _ = Pallet::<T>::evaluate_precondition(&precondition, &actor, T::Balance::zero());
    }
  }

  #[benchmark]
  fn predicate_set_evaluation(c: Linear<1, 4>) {
    let actor: T::AccountId = account("condition-any", 0, 0);
    let bounded = c.min(T::MaxPredicatesPerStep::get());
    let assets = T::BenchmarkHelper::setup_predicate_assets(&actor, bounded)
      .expect("predicate benchmark assets must be available");
    assert!(assets.len() >= bounded as usize);
    let predicates = assets
      .into_iter()
      .take(bounded as usize)
      .map(|asset| {
        T::AssetOps::mint(&actor, asset, T::MinUserBalance::get())
          .expect("condition benchmark asset must be funded");
        Predicate::BalanceAbove {
          asset,
          threshold: T::Balance::zero(),
        }
      })
      .collect::<alloc::vec::Vec<_>>();
    let precondition = current_any::<T>(predicates);
    #[block]
    {
      assert_eq!(
        Pallet::<T>::evaluate_precondition(&precondition, &actor, T::Balance::zero()),
        Ok(true)
      );
    }
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
      assert!(ActorHot::<T>::get(target_id).is_some_and(|hot| hot.wakeup_pointer.is_some()));
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
    assert!(ActorHot::<T>::get(target_id).is_some_and(|hot| hot.wakeup_pointer.is_some()));
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

  fn benchmark_queue_entry<T: Config>(
    ticket: u64,
    actor_id: ActorId,
  ) -> QueueEntry<BlockNumberFor<T>> {
    QueueEntry {
      actor_id,
      cycle_nonce: 0,
      cursor: 0,
      ticket,
      eligible_at: Zero::zero(),
      contract_commitment: ActorContractCommitment {
        semantic_contract_id: [0; 32],
        body_commitment: [0; 32],
      },
    }
  }

  fn install_chunked_contract<T: Config>(actor_id: ActorId, contract: &ActorContractOf<T>) {
    let (certificate, head, chunks) = admitted_contract_geometry::<T>(actor_id, contract);
    ActorRunStateStore::<T>::mutate(actor_id, |maybe| {
      if let Some(run) = maybe {
        run.contract_authority = ActorRunAuthority {
          semantic_contract_id: certificate.semantic_contract_id,
          body_commitment: certificate.body_commitment,
          admission_identity: certificate.admission_identity,
        };
      }
    });
    ActorAdmissionCertificates::<T>::insert(actor_id, certificate);
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

  fn install_run_state<T: Config>(actor_id: ActorId, snapshot_entries: u32) {
    let bounded = snapshot_entries.min(T::MaxOpeningSnapshotEntries::get());
    let asset_count = bounded.saturating_add(1) / 2;
    let assets = T::BenchmarkHelper::funding_assets(asset_count);
    let mut opening_snapshot = RunOpeningSnapshotOf::<T>::default();
    for asset in assets {
      if opening_snapshot.len() as u32 >= bounded {
        break;
      }
      opening_snapshot
        .try_insert(OpeningSurface::PreservableAsset(asset), One::one())
        .expect("benchmark snapshot asset entry fits");
      if opening_snapshot.len() as u32 >= bounded {
        break;
      }
      opening_snapshot
        .try_insert(OpeningSurface::StakingShares(asset), One::one())
        .expect("benchmark snapshot staking entry fits");
    }
    assert_eq!(opening_snapshot.len() as u32, bounded);
    let mut admitted_contract =
      Pallet::<T>::load_actor_contract(actor_id).expect("benchmark actor contract exists");
    admitted_contract.steps[0].on_error = StepErrorPolicy::RetryLater {
      max_attempts: T::MaxRetryAttempts::get(),
    };
    Pallet::<T>::store_actor_contract(actor_id, admitted_contract)
      .expect("benchmark retry Contract remains admitted");
    ActorHot::<T>::mutate(actor_id, |maybe_hot| {
      let hot = maybe_hot
        .as_mut()
        .expect("benchmark actor hot state exists");
      hot.cycle_state = CycleState::Suspended;
      hot.pending_signal = false;
      hot.queue_ticket = None;
      hot.wakeup_pointer = None;
    });
    let contract =
      Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Actor Contract exists");
    let last_attempt_block = 1u32.into();
    let eligible_at = Pallet::<T>::suspension_eligible_at(
      contract.cooldown_blocks,
      contract.window,
      last_attempt_block,
      1,
    )
    .expect("benchmark retry target is representable");
    let admission = ActorAdmissionCertificates::<T>::get(actor_id)
      .expect("benchmark admission certificate exists");
    ActorRunStateStore::<T>::insert(
      actor_id,
      ActorRunState {
        contract_authority: ActorRunAuthority {
          semantic_contract_id: admission.semantic_contract_id,
          body_commitment: admission.body_commitment,
          admission_identity: admission.admission_identity,
        },
        cycle_nonce: 1,
        cursor: 0,
        opening_predicate_cursor: 0,
        unsuccessful_attempts_at_cursor: 1,
        last_attempt_block,
        last_committed_step_block: None,
        eligible_at,
        opening_snapshot,
        opening_predicate_results: Default::default(),
        funding_snapshot: Default::default(),
        cumulative_outcomes: OutcomeTotals::default(),
        last_step_outcome: Some(StepOutcome::FundingUnavailable),
        suspension: Some(SuspensionReason::FundingUnavailable),
      },
    );
  }

  fn maximize_run_state_geometry<T: Config>(state: &mut ActorRunStateOf<T>) {
    let assets = T::BenchmarkHelper::funding_assets(T::MaxFundingTrackedAssets::get());
    for asset in assets {
      state
        .funding_snapshot
        .try_insert(asset, One::one())
        .expect("benchmark funding snapshot entry fits");
    }
    assert_eq!(
      state.funding_snapshot.len() as u32,
      T::MaxFundingTrackedAssets::get()
    );
    for index in 0..T::MaxOpeningPredicateResults::get() {
      state
        .opening_predicate_results
        .try_push(if index % 2 == 0 {
          Ok(true)
        } else {
          Err(PredicateError::InvalidObservation)
        })
        .expect("benchmark Opening predicate result fits");
    }
    state.cumulative_outcomes = OutcomeTotals {
      executed_steps: u32::MAX,
      committed_effectful_tasks: u32::MAX,
      precondition_skips: u32::MAX,
      skipped_resolution: u32::MAX,
      skipped_funding_unavailable: u32::MAX,
      failed_steps: u32::MAX,
    };
    state.last_step_outcome = Some(StepOutcome::Failed(TaskFailure::temporary(
      Error::<T>::ActorInvariant,
    )));
    state.suspension = Some(SuspensionReason::Temporary);
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
      WakeupBuckets::<T>::insert(
        WakeupKey::Block(block),
        WakeupBucketState {
          head_page: 0,
          tail_page: 0,
          next_page_id: 1,
          live_entries: 1,
          cursor_index: Some(index),
        },
      );
    }
    WakeupCursorPages::<T>::insert((WakeupClock::Block, page_id), page);
  }

  fn clear_host_genesis_wakeup_placements<T: Config>() {
    let block_actor_ids = ActorHot::<T>::iter()
      .filter_map(|(actor_id, hot)| hot.wakeup_pointer.map(|_| actor_id))
      .collect::<alloc::vec::Vec<_>>();
    for actor_id in block_actor_ids {
      Pallet::<T>::wakeup_substrate_invalidate(actor_id)
        .expect("host genesis Pipeline wakeup placement must be removable");
    }
    let trigger_actor_ids = ActorHot::<T>::iter()
      .filter_map(|(actor_id, hot)| hot.trigger_wakeup_pointer.map(|_| actor_id))
      .collect::<alloc::vec::Vec<_>>();
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

  fn install_saturated_tombstone_queue<T: Config>() {
    let page_size = T::QueuePageSize::get();
    let capacity = T::MaxQueueLength::get();
    for page_id in 0..capacity.div_ceil(page_size) {
      let first = page_id.saturating_mul(page_size);
      let len = page_size.min(capacity.saturating_sub(first));
      let entries = (0..len)
        .map(|offset| {
          benchmark_queue_entry::<T>(
            u64::from(first.saturating_add(offset)),
            u64::MAX.saturating_sub(u64::from(first.saturating_add(offset))),
          )
        })
        .collect::<alloc::vec::Vec<_>>();
      QueuePages::<T>::insert(
        u64::from(page_id),
        BoundedVec::try_from(entries).expect("saturated queue page fits"),
      );
    }
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(u64::from(capacity));
    QueueOccupancy::<T>::put(capacity);
    NextQueueTicket::<T>::put(u64::from(capacity));
  }

  fn prepare_saturated_address_actor<T: Config>(
    seed: u32,
    matched_source: Option<T::AccountId>,
  ) -> (ActorId, T::AccountId) {
    let owner: T::AccountId = account("ingress_owner", seed, 0);
    let source_filter = if let Some(matched_source) = matched_source {
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
    };
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
    NextQueueTicket::<T>::put(7);
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
      PrepassExecutionCutoff::<T>::put((now, NextQueueTicket::<T>::get()));
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
      core::hint::black_box(QueueHead::<T>::get());
      core::hint::black_box(QueueTail::<T>::get());
      core::hint::black_box(QueueOccupancy::<T>::get());
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
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(0);
    QueueOccupancy::<T>::put(0);
    WakeupCursorLen::<T>::insert(WakeupClock::Block, 0);
    IdleStarvationState::<T>::kill();
    #[block]
    {
      core::hint::black_box(Pallet::<T>::on_idle(now, Weight::MAX));
    }
    assert!(!IdleStarvationState::<T>::exists());
  }

  #[benchmark(pov_mode = Measured)]
  fn benchmark_monolithic_create(n: Linear<1, 8>) {
    let actor_id = 2_940;
    let contract = system_contract::<T>(
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 0,
      },
      inert_contract_steps_of_len::<T>(n),
    )
    .expect("active benchmark Contract");
    #[block]
    {
      Pallet::<T>::store_actor_contract(actor_id, contract)
        .expect("benchmark Contract remains admitted");
    }
    assert!(ActorContractHeads::<T>::contains_key(actor_id));
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
    assert!(ActorAdmissionCertificates::<T>::contains_key(actor_id));
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
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::remove_admitted_contract_geometry(actor_id)
          .expect("benchmark geometry removes coherently"),
      );
    }
    assert!(!ActorAdmissionCertificates::<T>::contains_key(actor_id));
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
  fn benchmark_chunked_load_tail(s: Linear<1, 4>) {
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
  }

  #[benchmark(pov_mode = Measured)]
  fn contract_geometry_create(c: Linear<0, 8>) {
    let c = c.min(
      T::MaxContractSteps::get()
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK),
    );
    let actor_id = 2_994;
    let n = 1u32.saturating_add(c.saturating_mul(4).min(31));
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
    assert!(ActorAdmissionCertificates::<T>::contains_key(actor_id));
    assert!(ActorContractHeads::<T>::contains_key(actor_id));
  }

  #[benchmark(pov_mode = Measured)]
  fn contract_geometry_close(c: Linear<0, 8>) {
    let c = c.min(
      T::MaxContractSteps::get()
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK),
    );
    let n = 1u32.saturating_add(c.saturating_mul(4).min(31));
    let actor_id = bench_create_system_with_plan::<T>(2_995, inert_contract_steps_of_len::<T>(n));
    let contract = Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Contract exists");
    install_chunked_contract::<T>(actor_id, &contract);
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::remove_admitted_contract_geometry(actor_id)
          .expect("benchmark geometry removes coherently"),
      );
    }
    assert!(!ActorAdmissionCertificates::<T>::contains_key(actor_id));
    assert!(!ActorContractHeads::<T>::contains_key(actor_id));
  }

  #[benchmark(pov_mode = Measured)]
  fn contract_geometry_reconstruct(c: Linear<0, 8>) {
    let c = c.min(
      T::MaxContractSteps::get()
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK),
    );
    let n = 1u32.saturating_add(c.saturating_mul(4).min(31));
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
  fn current_step_load_tail(s: Linear<1, 4>) {
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
  }

  #[benchmark(pov_mode = Measured)]
  fn current_step_plan_opening_head() {
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let actor_id = bench_create_system_with_plan::<T>(2_999, inert_contract_steps_of_len::<T>(1));
    let contract = Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Contract exists");
    install_chunked_contract::<T>(actor_id, &contract);
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("benchmark hot state exists")
        .queue_ticket = Some(9);
    });
    let identity = ActorIdentities::<T>::get(actor_id).expect("benchmark identity exists");
    let certificate = ActorAdmissionCertificates::<T>::get(actor_id)
      .expect("benchmark admission certificate exists");
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
  fn current_step_plan_suspended_head() {
    let actor_id = bench_create_system_with_plan::<T>(3_000, inert_contract_steps_of_len::<T>(1));
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    ActorRunStateStore::<T>::mutate(actor_id, |maybe| {
      maximize_run_state_geometry::<T>(maybe.as_mut().expect("benchmark run state exists"));
    });
    let contract = Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Contract exists");
    install_chunked_contract::<T>(actor_id, &contract);
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("benchmark hot state exists")
        .queue_ticket = Some(9);
    });
    let run = ActorRunStateStore::<T>::get(actor_id).expect("benchmark run state exists");
    frame_system::Pallet::<T>::set_block_number(run.eligible_at);
    let certificate = ActorAdmissionCertificates::<T>::get(actor_id)
      .expect("benchmark admission certificate exists");
    let ticket = ActorStepTicket {
      actor_id,
      cycle_nonce: run.cycle_nonce,
      cursor: 0,
      ticket: 9,
      eligible_at: run.eligible_at,
      contract_commitment: ActorContractCommitment {
        semantic_contract_id: certificate.semantic_contract_id,
        body_commitment: certificate.body_commitment,
      },
    };
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::load_current_step_plan_from_storage(ticket)
          .expect("benchmark Suspended head plan loads coherently"),
      );
    }
  }

  #[benchmark(pov_mode = Measured)]
  fn current_step_plan_running_tail(s: Linear<1, 4>) {
    let now: BlockNumberFor<T> = 2u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let actor_id =
      bench_create_system_with_plan::<T>(3_000, inert_contract_steps_of_len::<T>(1 + s));
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    ActorRunStateStore::<T>::mutate(actor_id, |maybe| {
      let run = maybe.as_mut().expect("benchmark run state exists");
      maximize_run_state_geometry::<T>(run);
      run.cursor = s;
      run.last_committed_step_block = Some(1u32.into());
      run.eligible_at = now;
      run.suspension = None;
    });
    let contract = Pallet::<T>::load_actor_contract(actor_id).expect("benchmark Contract exists");
    install_chunked_contract::<T>(actor_id, &contract);
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe.as_mut().expect("benchmark hot state exists");
      hot.cycle_state = CycleState::Running;
      hot.queue_ticket = Some(9);
    });
    let run = ActorRunStateStore::<T>::get(actor_id).expect("benchmark run state exists");
    let certificate = ActorAdmissionCertificates::<T>::get(actor_id)
      .expect("benchmark admission certificate exists");
    let ticket = ActorStepTicket {
      actor_id,
      cycle_nonce: run.cycle_nonce,
      cursor: run.cursor,
      ticket: 9,
      eligible_at: run.eligible_at,
      contract_commitment: ActorContractCommitment {
        semantic_contract_id: certificate.semantic_contract_id,
        body_commitment: certificate.body_commitment,
      },
    };
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::load_current_step_plan_from_storage(ticket)
          .expect("benchmark Running plan loads coherently"),
      );
    }
  }

  #[benchmark(pov_mode = Measured)]
  fn opening_snapshot_capture(e: Linear<1, 16>) {
    let actor: T::AccountId = account("opening_snapshot_actor", 0, 0);
    let assets = T::BenchmarkHelper::setup_predicate_assets(&actor, e)
      .expect("benchmark Opening assets exist");
    assert_eq!(u32::try_from(assets.len()).expect("asset count fits"), e);
    for asset in &assets {
      let _ = T::AssetOps::mint(&actor, *asset, One::one());
    }
    let mut steps = ContractSteps::<T>::default();
    for pair in assets.chunks(2) {
      let asset_a = pair[0];
      let asset_b = pair.get(1).copied().unwrap_or(asset_a);
      steps
        .try_push(Step {
          precondition: None,
          task: ActorTask::AddLiquidity {
            asset_a,
            asset_b,
            amount_a: AmountResolution::PercentageAtOpening(Perbill::one()),
            amount_b: if pair.len() == 2 {
              AmountResolution::PercentageAtOpening(Perbill::one())
            } else {
              AmountResolution::Fixed(Zero::zero())
            },
            min_lp_out: Zero::zero(),
          },
          on_error: StepErrorPolicy::AbortCycle,
        })
        .expect("benchmark Opening Step fits");
    }
    #[block]
    {
      let snapshot =
        Pallet::<T>::capture_opening_snapshot(ActorType::System, &actor, &steps, Zero::zero());
      assert_eq!(
        u32::try_from(snapshot.len()).expect("snapshot count fits"),
        e
      );
      core::hint::black_box(snapshot);
    }
  }

  #[benchmark(pov_mode = Measured)]
  fn opening_predicate_capture(p: Linear<1, 32>) {
    let actor: T::AccountId = account("opening_predicate_actor", 0, 0);
    let assets = T::BenchmarkHelper::setup_predicate_assets(&actor, p)
      .expect("benchmark predicate assets exist");
    assert_eq!(u32::try_from(assets.len()).expect("asset count fits"), p);
    for asset in &assets {
      let _ = T::AssetOps::mint(&actor, *asset, One::one());
    }
    let mut steps = ContractSteps::<T>::default();
    for chunk in assets.chunks(T::MaxPredicatesPerClause::get() as usize) {
      let predicates = BoundedVec::try_from(
        chunk
          .iter()
          .map(|asset| TimedPredicate {
            timing: ObservationTiming::Opening,
            predicate: Predicate::BalanceAbove {
              asset: *asset,
              threshold: Zero::zero(),
            },
          })
          .collect::<Vec<_>>(),
      )
      .expect("benchmark predicate clause fits");
      steps
        .try_push(Step {
          precondition: Some(Precondition {
            clauses: BoundedVec::try_from(vec![predicates]).expect("benchmark predicate set fits"),
          }),
          task: ActorTask::StopCycle,
          on_error: StepErrorPolicy::AbortCycle,
        })
        .expect("benchmark predicate Step fits");
    }
    #[block]
    {
      let results = Pallet::<T>::capture_opening_predicate_results(&actor, &steps, Zero::zero());
      assert_eq!(u32::try_from(results.len()).expect("result count fits"), p);
      core::hint::black_box(results);
    }
  }

  #[benchmark]
  fn scheduler_actor_state_probe() {
    let actor_id = bench_create_system_manual::<T>(3_000);
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    let assets = T::BenchmarkHelper::funding_assets(T::MaxFundingTrackedAssets::get());
    ActorFunding::<T>::mutate(actor_id, |maybe_funding| {
      let funding = maybe_funding
        .as_mut()
        .expect("benchmark actor funding state must exist");
      for asset in assets {
        funding
          .funding_tracked_assets
          .try_insert(asset)
          .expect("benchmark tracked asset fits");
        funding
          .funding_accumulated
          .try_insert(asset, One::one())
          .expect("benchmark funding entry fits");
      }
    });
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
  }
  /// One complete minimal cycle execution over one inert StopCycle step (fixed cycle
  /// orchestration plus finalization), measured on the execution path only; queue probes and
  /// head consumption are separate scheduler classes.
  #[benchmark]
  fn cycle_orchestration() {
    let actor_id = bench_create_system_with_plan::<T>(3_100, make_inert_contract_steps::<T>());
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let instance = Pallet::<T>::active_actor_view(actor_id).expect("cycle actor exists");
    #[block]
    {
      core::hint::black_box(Pallet::<T>::execute_single_cycle(actor_id, instance, now));
    }
    let updated = Pallet::<T>::active_actor_view(actor_id).expect("cycle actor survives");
    assert_eq!(updated.cycle_nonce, 1);
  }

  /// One complete cycle execution over `steps` inert StopCycle steps: the linear model prices
  /// the fixed cycle orchestration plus per-step bookkeeping, the exact cycle overhead the
  /// admission composition uses for arbitrary plans.
  #[benchmark]
  fn step_orchestration(n: Linear<1, 8>) {
    let actor_id = bench_create_system_with_plan::<T>(3_200, inert_contract_steps_of_len::<T>(n));
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let instance = Pallet::<T>::active_actor_view(actor_id).expect("cycle actor exists");
    #[block]
    {
      core::hint::black_box(Pallet::<T>::execute_single_cycle(actor_id, instance, now));
    }
    let updated = Pallet::<T>::active_actor_view(actor_id).expect("cycle actor survives");
    assert_eq!(updated.cycle_nonce, 1);
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_append_existing_page() {
    let page_size = T::QueuePageSize::get();
    assert!(
      page_size >= 2,
      "benchmark requires a non-trivial queue page"
    );
    for i in 0..page_size.saturating_sub(1) {
      let actor_id = bench_create_system_manual::<T>(31_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::paged_enqueue(actor_id));
    }
    let actor_id = bench_create_system_manual::<T>(32_000_000);
    #[block]
    {
      assert!(Pallet::<T>::paged_enqueue(actor_id));
    }
    assert_eq!(QueueTail::<T>::get(), u64::from(page_size));
    assert_eq!(
      ActorHot::<T>::get(actor_id).and_then(|hot| hot.queue_ticket),
      Some(u64::from(page_size - 1))
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_append_new_page() {
    let page_size = T::QueuePageSize::get();
    for i in 0..page_size {
      let actor_id = bench_create_system_manual::<T>(33_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::paged_enqueue(actor_id));
    }
    let actor_id = bench_create_system_manual::<T>(34_000_000);
    #[block]
    {
      assert!(Pallet::<T>::paged_enqueue(actor_id));
    }
    assert_eq!(
      QueueTail::<T>::get(),
      u64::from(page_size).saturating_add(1)
    );
    assert_eq!(QueuePages::<T>::get(1).map(|page| page.len()), Some(1));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_append_existing_page() {
    let page_size = T::WakeupPageSize::get();
    assert!(
      page_size >= 2,
      "benchmark requires a non-trivial wakeup page"
    );
    let wakeup_block = 100u32.into();
    for i in 0..page_size.saturating_sub(1) {
      let actor_id = bench_create_system_manual::<T>(41_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::wakeup_substrate_schedule(
        actor_id,
        wakeup_block
      ));
    }
    let actor_id = bench_create_system_manual::<T>(42_000_000);
    #[block]
    {
      assert!(Pallet::<T>::wakeup_substrate_schedule(
        actor_id,
        wakeup_block
      ));
    }
    let pointer = ActorHot::<T>::get(actor_id)
      .and_then(|hot| hot.wakeup_pointer)
      .expect("benchmark wakeup pointer must exist");
    assert_eq!((pointer.page_id, pointer.slot), (0, page_size - 1));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_append_new_page() {
    let page_size = T::WakeupPageSize::get();
    let wakeup_block = 100u32.into();
    for i in 0..page_size {
      let actor_id = bench_create_system_manual::<T>(43_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::wakeup_substrate_schedule(
        actor_id,
        wakeup_block
      ));
    }
    let actor_id = bench_create_system_manual::<T>(44_000_000);
    #[block]
    {
      assert!(Pallet::<T>::wakeup_substrate_schedule(
        actor_id,
        wakeup_block
      ));
    }
    let pointer = ActorHot::<T>::get(actor_id)
      .and_then(|hot| hot.wakeup_pointer)
      .expect("benchmark wakeup pointer must exist");
    assert_eq!((pointer.page_id, pointer.slot), (1, 0));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_replace_exact() {
    let actor_id = bench_create_system_manual::<T>(45_000_000);
    let old_block = 100u32.into();
    let replacement_block = 200u32.into();
    assert!(Pallet::<T>::wakeup_substrate_schedule(actor_id, old_block));
    #[block]
    {
      assert!(Pallet::<T>::wakeup_substrate_schedule(
        actor_id,
        replacement_block
      ));
    }
    let pointer = ActorHot::<T>::get(actor_id)
      .and_then(|hot| hot.wakeup_pointer)
      .expect("replacement wakeup pointer must exist");
    assert_eq!(
      (pointer.block, pointer.page_id, pointer.slot),
      (WakeupKey::Block(replacement_block), 0, 0)
    );
    assert!(!WakeupBuckets::<T>::contains_key(WakeupKey::Block(
      old_block
    )));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_invalidate_middle_page() {
    let page_size = T::WakeupPageSize::get();
    let wakeup_block = 100u32.into();
    let count = page_size.saturating_mul(2).saturating_add(1);
    let mut actors = alloc::vec::Vec::with_capacity(count as usize);
    for i in 0..count {
      let actor_id = bench_create_system_manual::<T>(46_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::wakeup_substrate_schedule(
        actor_id,
        wakeup_block
      ));
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
    assert!(!WakeupPages::<T>::contains_key((
      WakeupKey::Block(wakeup_block),
      1,
    )));
    assert_eq!(
      WakeupPages::<T>::get((WakeupKey::Block(wakeup_block), 0)).and_then(|page| page.next_page),
      Some(2)
    );
    assert_eq!(
      WakeupPages::<T>::get((WakeupKey::Block(wakeup_block), 2))
        .and_then(|page| page.previous_page),
      Some(0)
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_drain_partial_page() {
    let page_size = T::WakeupPageSize::get();
    assert!(page_size >= 2, "benchmark requires a partial page");
    let wakeup_block = 100u32.into();
    for i in 0..page_size {
      let actor_id = bench_create_system_manual::<T>(47_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::wakeup_substrate_schedule(
        actor_id,
        wakeup_block
      ));
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
      WakeupPages::<T>::get((WakeupKey::Block(wakeup_block), 0)).map(|page| page.scan_slot),
      Some(scan_limit)
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_drain_full_page() {
    let page_size = T::WakeupPageSize::get();
    let wakeup_block = 100u32.into();
    for i in 0..page_size {
      let actor_id = bench_create_system_manual::<T>(48_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::wakeup_substrate_schedule(
        actor_id,
        wakeup_block
      ));
    }
    #[block]
    {
      let (ready, stats) = Pallet::<T>::wakeup_substrate_drain_block(wakeup_block, page_size);
      assert_eq!(ready.len(), page_size as usize);
      assert_eq!(stats.entries_scanned, page_size);
      assert_eq!(stats.pages_deleted, 1);
    }
    assert!(!WakeupBuckets::<T>::contains_key(WakeupKey::Block(
      wakeup_block
    )));
    assert!(!WakeupPages::<T>::contains_key((
      WakeupKey::Block(wakeup_block),
      0,
    )));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_drain_dense_boundary() {
    let page_size = T::WakeupPageSize::get();
    let count = page_size.saturating_add(1);
    assert!(
      count <= T::MaxWakeupsPerBlock::get(),
      "benchmark requires one boundary-crossing drain"
    );
    let wakeup_block = 100u32.into();
    for i in 0..count {
      let actor_id = bench_create_system_manual::<T>(49_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::wakeup_substrate_schedule(
        actor_id,
        wakeup_block
      ));
    }
    #[block]
    {
      let (ready, stats) = Pallet::<T>::wakeup_substrate_drain_block(wakeup_block, count);
      assert_eq!(ready.len(), count as usize);
      assert_eq!(stats.entries_scanned, count);
      assert_eq!(stats.pages_touched, 2);
      assert_eq!(stats.pages_deleted, 2);
    }
    assert!(!WakeupBuckets::<T>::contains_key(WakeupKey::Block(
      wakeup_block
    )));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_drain_stale_page() {
    let page_size = T::WakeupPageSize::get();
    let wakeup_block = 100u32.into();
    for i in 0..page_size {
      let actor_id = bench_create_system_manual::<T>(50_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::wakeup_substrate_schedule(
        actor_id,
        wakeup_block
      ));
      ActorHot::<T>::mutate(actor_id, |maybe_hot| {
        maybe_hot
          .as_mut()
          .expect("benchmark actor hot state must exist")
          .wakeup_pointer = None;
      });
    }
    #[block]
    {
      let (ready, stats) = Pallet::<T>::wakeup_substrate_drain_block(wakeup_block, page_size);
      assert!(ready.is_empty());
      assert_eq!(stats.entries_scanned, page_size);
      assert_eq!(stats.stale_entries, page_size);
      assert_eq!(stats.pages_deleted, 1);
    }
    assert!(!WakeupBuckets::<T>::contains_key(WakeupKey::Block(
      wakeup_block
    )));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_cursor_insert() {
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
    let inserted_block: BlockNumberFor<T> = 1u32.into();
    WakeupBuckets::<T>::insert(
      WakeupKey::Block(inserted_block),
      WakeupBucketState {
        head_page: 0,
        tail_page: 0,
        next_page_id: 1,
        live_entries: 1,
        cursor_index: None,
      },
    );
    #[block]
    {
      assert!(Pallet::<T>::wakeup_cursor_insert(inserted_block));
    }
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Block), max_active);
    assert_eq!(Pallet::<T>::wakeup_cursor_peek(), Some(inserted_block));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_cursor_pop_min() {
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
      WakeupBuckets::<T>::get(WakeupKey::Block(expected_min))
        .and_then(|bucket| bucket.cursor_index),
      None
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_cursor_remove_exact() {
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
      WakeupBuckets::<T>::get(WakeupKey::Block(removed_block))
        .and_then(|bucket| bucket.cursor_index),
      None
    );
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
      ActorHot::<T>::mutate(actor_id, |maybe_hot| {
        maybe_hot
          .as_mut()
          .expect("benchmark actor hot state exists")
          .trigger_runtime_state = TriggerRuntimeState::Cadenced { anchor_tick: None };
      });
      Pallet::<T>::benchmark_defer_tick_wakeup(actor_id, 0)
        .expect("benchmark bootstrap wakeup fits");
    }
    let limit = T::WeightInfo::scheduler_wakeup_cursor_worker_future()
      .saturating_mul(2)
      .saturating_add(Pallet::<T>::wakeup_cursor_drain_unit_weight_upper(false));
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(limit);
    #[block]
    {
      let stats = Pallet::<T>::drain_overdue_wakeups_cursor(10u32.into(), &mut meter);
      assert_eq!(stats.entries_scanned, 1);
      assert_eq!(stats.ready_entries, 1);
    }
    assert_eq!(
      WakeupBuckets::<T>::get(WakeupKey::Tick(0)).map(|bucket| bucket.live_entries),
      Some(1)
    );
    let rearmed_tick = ActorHot::<T>::get(first)
      .and_then(|hot| hot.trigger_runtime_state.temporal_anchor_tick())
      .and_then(|anchor| anchor.checked_add(1))
      .expect("benchmark cadence re-anchors");
    assert_eq!(
      WakeupBuckets::<T>::get(WakeupKey::Tick(rearmed_tick)).map(|bucket| bucket.live_entries),
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
    ActorHot::<T>::mutate(actor_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("AtTime Actor hot state exists")
        .trigger_runtime_state = TriggerRuntimeState::AtTime {
        anchor_tick: Some(0),
        consumed: false,
      };
    });
    Pallet::<T>::benchmark_defer_tick_wakeup(actor_id, 1).expect("due AtTime occurrence fits");
    Pallet::<T>::trigger_wakeup_substrate_invalidate_inner(actor_id)
      .expect("due AtTime pointer is coherent")
      .expect("due AtTime pointer exists");
    #[block]
    {
      assert_eq!(
        Pallet::<T>::process_due_temporal_occurrence(actor_id, 1),
        Ok(false)
      );
    }
    let hot = ActorHot::<T>::get(actor_id).expect("AtTime Actor remains active");
    assert!(hot.pending_signal);
    assert!(hot.queue_ticket.is_some());
    assert!(hot.trigger_wakeup_pointer.is_none());
    assert!(matches!(
      hot.trigger_runtime_state,
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
    ActorHot::<T>::mutate(actor_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("Cadenced Actor hot state exists")
        .trigger_runtime_state = TriggerRuntimeState::Cadenced {
        anchor_tick: Some(0),
      };
    });
    Pallet::<T>::benchmark_defer_tick_wakeup(actor_id, 0).expect("due Cadenced occurrence fits");
    Pallet::<T>::trigger_wakeup_substrate_invalidate_inner(actor_id)
      .expect("due Cadenced pointer is coherent")
      .expect("due Cadenced pointer exists");
    #[block]
    {
      assert_eq!(
        Pallet::<T>::process_due_temporal_occurrence(actor_id, 0),
        Ok(false)
      );
    }
    let hot = ActorHot::<T>::get(actor_id).expect("Cadenced Actor remains active");
    assert!(hot.pending_signal);
    assert!(hot.queue_ticket.is_some());
    assert!(hot.trigger_wakeup_pointer.is_none());
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_cursor_worker_remove() {
    clear_host_genesis_wakeup_placements::<T>();
    let cursor_len = T::MaxActiveActors::get();
    let wakeup_block = prepare_wakeup_cursor_repair::<T>(0);
    let actor_id = bench_create_system_manual::<T>(34_200_000);
    let mut entries = WakeupPageEntriesOf::<T>::default();
    entries
      .try_push(Some(WakeupEntry { actor_id }))
      .expect("one wakeup entry fits");
    WakeupPages::<T>::insert(
      (WakeupKey::Block(wakeup_block), 0),
      WakeupPage {
        entries,
        live_entries: 1,
        scan_slot: 0,
        previous_page: None,
        next_page: None,
      },
    );
    WakeupBuckets::<T>::mutate(WakeupKey::Block(wakeup_block), |maybe_bucket| {
      let bucket = maybe_bucket.as_mut().expect("cursor bucket exists");
      bucket.head_page = 0;
      bucket.tail_page = 0;
      bucket.next_page_id = 1;
      bucket.live_entries = 1;
    });
    ActorHot::<T>::mutate(actor_id, |maybe_hot| {
      maybe_hot.as_mut().expect("actor hot state").wakeup_pointer = Some(WakeupPointer {
        block: WakeupKey::Block(wakeup_block),
        page_id: 0,
        slot: 0,
      });
    });
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
    assert!(WakeupBuckets::<T>::get(WakeupKey::Block(wakeup_block)).is_none());
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_cursor_worker_future() {
    clear_host_genesis_wakeup_placements::<T>();
    let wakeup_block: BlockNumberFor<T> = 1_000_000u32.into();
    let actor_id = bench_create_system_manual::<T>(34_300_000);
    assert!(Pallet::<T>::wakeup_substrate_schedule(
      actor_id,
      wakeup_block
    ));
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    #[block]
    {
      let stats = Pallet::<T>::drain_overdue_wakeups_cursor(10u32.into(), &mut meter);
      assert_eq!(stats.entries_scanned, 0);
    }
    assert_eq!(Pallet::<T>::wakeup_cursor_peek(), Some(wakeup_block));
    assert!(
      ActorHot::<T>::get(actor_id)
        .and_then(|hot| hot.wakeup_pointer)
        .is_some()
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_consume_preserve_page() {
    let first = bench_create_system_manual::<T>(35_000_000);
    let second = bench_create_system_manual::<T>(35_000_001);
    assert!(Pallet::<T>::paged_enqueue(first));
    assert!(Pallet::<T>::paged_enqueue(second));
    #[block]
    {
      assert!(Pallet::<T>::paged_consume_head(0));
    }
    assert_eq!(QueueHead::<T>::get(), 1);
    assert!(QueuePages::<T>::contains_key(0));
    assert_eq!(
      ActorHot::<T>::get(first).and_then(|hot| hot.queue_ticket),
      None
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_consume_delete_page() {
    let actor_id = bench_create_system_manual::<T>(36_000_000);
    assert!(Pallet::<T>::paged_enqueue(actor_id));
    #[block]
    {
      assert!(Pallet::<T>::paged_consume_head(0));
    }
    assert_eq!(QueueHead::<T>::get(), u64::from(T::QueuePageSize::get()));
    assert_eq!(QueueTail::<T>::get(), u64::from(T::QueuePageSize::get()));
    assert!(!QueuePages::<T>::contains_key(0));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_tombstone_drain(n: Linear<1, 10_000>) {
    let bounded = n.min(T::MaxQueueLength::get());
    let page_size = T::QueuePageSize::get();
    let mut ticket = 0u64;
    while ticket < u64::from(bounded) {
      let page_id = ticket / u64::from(page_size);
      let remaining = u64::from(bounded).saturating_sub(ticket);
      let entries = remaining.min(u64::from(page_size));
      let page = (0..entries)
        .map(|offset| {
          benchmark_queue_entry::<T>(
            ticket.saturating_add(offset),
            37_000_000u64.saturating_add(ticket).saturating_add(offset),
          )
        })
        .collect::<alloc::vec::Vec<_>>();
      QueuePages::<T>::insert(
        page_id,
        BoundedVec::<QueueEntry<BlockNumberFor<T>>, T::QueuePageSize>::try_from(page)
          .expect("benchmark queue page must fit configured page size"),
      );
      ticket = ticket.saturating_add(entries);
    }
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(u64::from(bounded));
    QueueOccupancy::<T>::put(bounded);
    NextQueueTicket::<T>::put(u64::from(bounded));
    #[block]
    {
      core::hint::black_box(
        Pallet::<T>::paged_drain_tombstones(u64::from(bounded), bounded)
          .expect("benchmark queue topology is valid"),
      );
    }
    assert!(QueueHead::<T>::get() >= u64::from(bounded));
    assert_eq!(QueueHead::<T>::get(), QueueTail::<T>::get());
    assert_eq!(QueueOccupancy::<T>::get(), 0);
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_mixed_scan(n: Linear<1, 10_000>) {
    let bounded = n.min(T::MaxQueueLength::get());
    let page_size = T::QueuePageSize::get();
    let template_id = bench_create_system_manual::<T>(38_000_000);
    let hot_template = ActorHot::<T>::get(template_id).expect("benchmark hot template");
    let contract_template =
      Pallet::<T>::load_actor_contract(template_id).expect("benchmark contract template");
    let funding_template = ActorFunding::<T>::get(template_id).expect("benchmark funding template");
    let mut identity_template =
      ActorIdentities::<T>::get(template_id).expect("benchmark identity template");
    identity_template.actor_class = ActorClass::User { owner_slot: 0 };
    let mut ticket = 0u64;
    while ticket < u64::from(bounded) {
      let page_id = ticket / u64::from(page_size);
      let remaining = u64::from(bounded).saturating_sub(ticket);
      let entries = remaining.min(u64::from(page_size));
      let page = (0..entries)
        .map(|offset| {
          let logical_ticket = ticket.saturating_add(offset);
          let actor_id = 39_000_000u64.saturating_add(logical_ticket);
          if logical_ticket % 2 == 1 {
            let mut hot = hot_template.clone();
            hot.queue_ticket = Some(logical_ticket);
            ActorIdentities::<T>::insert(actor_id, identity_template.clone());
            ActorHot::<T>::insert(actor_id, hot);
            Pallet::<T>::store_actor_contract(actor_id, contract_template.clone())
              .expect("benchmark mixed Contract remains admitted");
            ActorFunding::<T>::insert(actor_id, funding_template.clone());
          }
          benchmark_queue_entry::<T>(logical_ticket, actor_id)
        })
        .collect::<alloc::vec::Vec<_>>();
      QueuePages::<T>::insert(
        page_id,
        BoundedVec::<QueueEntry<BlockNumberFor<T>>, T::QueuePageSize>::try_from(page)
          .expect("benchmark queue page must fit configured page size"),
      );
      ticket = ticket.saturating_add(entries);
    }
    ActorIdentities::<T>::remove(template_id);
    ActorHot::<T>::remove(template_id);
    Pallet::<T>::remove_actor_contract(template_id).expect("benchmark template Contract removes");
    ActorFunding::<T>::remove(template_id);
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(u64::from(bounded));
    QueueOccupancy::<T>::put(bounded);
    NextQueueTicket::<T>::put(u64::from(bounded));
    let cutoff = u64::from(bounded);
    #[block]
    {
      while QueueHead::<T>::get() < cutoff {
        core::hint::black_box(
          Pallet::<T>::paged_drain_tombstones(cutoff, bounded)
            .expect("benchmark mixed queue topology is valid"),
        );
        if let Some((_, entry)) = Pallet::<T>::paged_head_entry() {
          assert!(Pallet::<T>::paged_consume_head(entry.ticket));
        }
      }
    }
    assert!(QueueHead::<T>::get() >= cutoff);
    assert_eq!(QueueHead::<T>::get(), QueueTail::<T>::get());
    assert_eq!(QueueOccupancy::<T>::get(), 0);
  }

  /// Measures one persistent zero-Step Opening after FIFO head consumption. The branch charges no
  /// Action, creates no Run state, commits the Cycle nonce and summary, and leaves no successor
  /// Pipeline placement for a Manual Contract.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_zero_step_complete() {
    let owner: T::AccountId = account("zero_step_opening_owner", 0, 0);
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 0,
    };
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      system_contract::<T>(schedule, ContractSteps::<T>::default()),
    )
    .expect("zero-Step System Contract exists");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe.as_mut().expect("zero-Step actor is active");
      hot.pending_signal = true;
      hot.queue_ticket = Some(9);
    });
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let LoadedActorStateOf::Active(state) = Pallet::<T>::load_actor_state(actor_id) else {
      panic!("zero-Step actor state exists");
    };
    let instance = Pallet::<T>::derive_active_actor_view(state.identity, state.hot, state.contract);
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("zero-Step actor remains active")
        .queue_ticket = None;
    });
    #[block]
    {
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_zero_step_opening_and_place(actor_id, instance, now) {
          Ok(()) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("zero-Step inner atom commits");
    }
    let identity = ActorIdentities::<T>::get(actor_id).expect("zero-Step actor remains registered");
    assert_eq!(identity.cycle_nonce, 1);
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
      !hot.pending_signal && hot.queue_ticket.is_none() && hot.wakeup_pointer.is_none()
    }));
  }

  /// Measures one complete fresh-Opening control path at maximum configured Contract geometry.
  /// Every Step references one zero-balance Opening amount, so resolution skips and no Task effect
  /// executes. The measured branch owns queue service, exact current-Step planning, maximum Contract
  /// and funding geometry, immutable Opening capture, Running persistence, and causal successor
  /// placement without effect-Weight contamination. Parameterized inner benchmarks separately own
  /// maximum snapshot, predicate, and heavier Task envelopes.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_execute_opening_max() {
    let _ = QueuePages::<T>::clear(u32::MAX, None);
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(0);
    QueueOccupancy::<T>::put(0);
    NextQueueTicket::<T>::put(0);
    GlobalCircuitBreaker::<T>::put(false);
    let step_count = T::MaxContractSteps::get();
    let asset_count = step_count
      .checked_mul(2)
      .expect("maximum Opening asset count fits")
      .min(T::MaxFundingTrackedAssets::get());
    let asset_owner: T::AccountId = account("opening_atomic_assets", 0, 0);
    let assets = T::BenchmarkHelper::setup_predicate_assets(&asset_owner, asset_count)
      .expect("maximum Opening assets exist");
    assert_eq!(assets.len(), asset_count as usize);
    let asset = assets[0];
    let destination: T::AccountId = account("opening_atomic_destination", 0, 0);
    let mut steps = ContractSteps::<T>::default();
    for _ in 0..step_count {
      steps
        .try_push(Step {
          precondition: None,
          task: ActorTask::Transfer {
            to: destination.clone(),
            asset,
            amount: AmountResolution::PercentageAtOpening(Perbill::one()),
          },
          on_error: StepErrorPolicy::AbortCycle,
        })
        .expect("maximum Opening Contract fits");
    }
    assert_eq!(steps.len(), step_count as usize);
    let admission = Pallet::<T>::contract_steps_admission_weight_upper(ActorType::User, &steps);
    let service = Pallet::<T>::guaranteed_actor_service_weight()
      .expect("Opening benchmark service floor exists");
    assert!(
      admission.all_lte(service),
      "maximum public Opening fixture must fit admission: admission={admission:?}, service={service:?}"
    );
    let owner: T::AccountId = account("opening_atomic_owner", 0, 0);
    ensure_creation_balance::<T>(&owner);
    prefund_active_user_creation::<T>(&owner, &steps);
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 0,
    };
    Pallet::<T>::create_user_actor(
      RawOrigin::Signed(owner).into(),
      Mutability::Mutable,
      user_contract::<T>(schedule, steps),
    )
    .expect("maximum User Opening Contract exists");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    ActorFunding::<T>::mutate(actor_id, |maybe| {
      let funding = maybe
        .as_mut()
        .expect("maximum Opening actor funding exists");
      for asset in assets
        .iter(/* deos-bypass: bounded-iter */)
        .take(T::MaxFundingTrackedAssets::get() as usize)
      {
        funding
          .funding_tracked_assets
          .try_insert(*asset)
          .expect("maximum Opening tracked asset fits");
        funding
          .funding_accumulated
          .try_insert(*asset, One::one())
          .expect("maximum Opening funding snapshot fits");
      }
    });
    assert_eq!(
      ActorFunding::<T>::get(actor_id)
        .expect("maximum Opening funding exists")
        .funding_accumulated
        .len(),
      T::MaxFundingTrackedAssets::get() as usize,
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("maximum Opening actor is active")
        .pending_signal = true;
    });
    assert!(Pallet::<T>::paged_enqueue(actor_id));
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    #[block]
    {
      core::hint::black_box(Pallet::<T>::execute_cycle(Weight::MAX));
    }
    let run = ActorRunStateStore::<T>::get(actor_id).expect("maximum Opening run persists");
    assert_eq!(run.cursor, 1);
    assert_eq!(run.opening_snapshot.len(), 1);
    assert_eq!(
      run.funding_snapshot.len(),
      T::MaxFundingTrackedAssets::get() as usize,
    );
    assert!(run.opening_predicate_results.is_empty());
    assert_eq!(run.last_committed_step_block, Some(now));
    assert_eq!(run.eligible_at, now.saturating_add(One::one()));
    assert_eq!(
      QueueHead::<T>::get().saturating_add(1),
      QueueTail::<T>::get()
    );
    assert_eq!(QueueOccupancy::<T>::get(), 1);
  }

  /// Measures direct minimal-geometry fresh-Opening Actor close at every C6 tail count. StopCycle
  /// reaches auto-close nonce one and removes the complete authored geometry without an effect.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_close_min(t: Linear<0, 8>) {
    let tail_chunks = t.min(
      T::MaxContractSteps::get()
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK),
    );
    let step_count = 1u32
      .saturating_add(tail_chunks.saturating_mul(MAX_STEPS_PER_TAIL_CHUNK))
      .min(T::MaxContractSteps::get());
    let mut steps = inert_contract_steps_of_len::<T>(step_count);
    steps[0].task = ActorTask::StopCycle;
    let actor_id = bench_create_system_manual::<T>(41_607_812);
    let mut contract = system_contract::<T>(
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 0,
      },
      steps,
    )
    .expect("minimal Opening-close Contract exists");
    contract.auto_close_at_cycle_nonce = Some(1);
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("minimal Opening-close geometry stores");
    let funding_assets = T::BenchmarkHelper::funding_assets(T::MaxFundingTrackedAssets::get());
    ActorFunding::<T>::mutate(actor_id, |maybe| {
      let funding = maybe
        .as_mut()
        .expect("minimal Opening-close funding exists");
      for asset in funding_assets {
        funding
          .funding_tracked_assets
          .try_insert(asset)
          .expect("minimal Opening-close tracked asset fits");
        funding
          .funding_accumulated
          .try_insert(asset, One::one())
          .expect("minimal Opening-close funding value fits");
      }
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe
        .as_mut()
        .expect("minimal Opening-close actor is active");
      hot.pending_signal = true;
      hot.queue_ticket = Some(9);
    });
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("minimal Opening-close service state is coherent");
    let LoadedActorStateOf::Active(full_state) = Pallet::<T>::load_actor_state(actor_id) else {
      panic!("minimal Opening-close full state exists");
    };
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      full_state.identity,
      full_state.hot,
      full_state.contract,
    );
    let control_context = Pallet::<T>::execution_step_control_weight_context(
      &execution_instance,
      state.run_state.as_ref(),
      &loaded_step,
    )
    .expect("minimal Opening-close control context exists");
    assert_eq!(control_context.opening_tail_chunks, tail_chunks);
    assert_eq!(control_context.opening_snapshot_entries, 0);
    assert_eq!(control_context.opening_predicate_results, 0);
    assert_eq!(control_context.predicate_evaluation_units, 0);
    assert_eq!(
      T::StepControlWeight::maximum_control_weight(control_context, &loaded_step.step),
      Some(loaded_step.resources.control),
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("minimal Opening-close hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("minimal Opening-close Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("minimal Opening-close maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("minimal Opening-close plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            assert!(!evidence.closed_for_exhaustion);
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("minimal Opening-close inner atom commits");
    }
    assert!(ActorHot::<T>::get(actor_id).is_none());
    assert!(ActorIdentities::<T>::get(actor_id).is_none());
    assert!(ActorContractHeads::<T>::get(actor_id).is_none());
    assert_eq!(
      ActorContractTailChunks::<T>::iter_prefix(actor_id).count(),
      0,
    );
  }

  /// Measures direct minimal-geometry fresh-Opening failure at every C6 tail count. An unfunded
  /// fixed Transfer aborts the cycle without effect invocation, run persistence, or placement.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_failed_min(t: Linear<0, 8>) {
    let tail_chunks = t.min(
      T::MaxContractSteps::get()
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK),
    );
    let step_count = 1u32
      .saturating_add(tail_chunks.saturating_mul(MAX_STEPS_PER_TAIL_CHUNK))
      .min(T::MaxContractSteps::get());
    let mut steps = inert_contract_steps_of_len::<T>(step_count);
    let actor_id = bench_create_system_manual::<T>(41_609_375);
    let funding_assets = T::BenchmarkHelper::funding_assets(T::MaxFundingTrackedAssets::get());
    let invalid_legs = SplitTransferLegsOf::<T>::try_from(alloc::vec![
      SplitLeg {
        to: account("opening_failure_leg", 0, 0),
        share: Perbill::from_percent(50),
      },
      SplitLeg {
        to: account("opening_failure_leg", 0, 0),
        share: Perbill::from_percent(50),
      },
    ])
    .expect("minimal Opening-failure split legs fit");
    steps[0].task = ActorTask::SplitTransfer {
      asset: T::FeeNativeAssetId::get(),
      amount: AmountResolution::Fixed(One::one()),
      legs: invalid_legs,
    };
    steps[0].on_error = StepErrorPolicy::AbortCycle;
    let contract = system_contract::<T>(
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 0,
      },
      steps,
    )
    .expect("minimal Opening-failure Contract exists");
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("minimal Opening-failure geometry stores");
    ActorFunding::<T>::mutate(actor_id, |maybe| {
      let funding = maybe
        .as_mut()
        .expect("minimal Opening-failure funding exists");
      for asset in funding_assets {
        funding
          .funding_tracked_assets
          .try_insert(asset)
          .expect("minimal Opening-failure tracked asset fits");
        funding
          .funding_accumulated
          .try_insert(asset, One::one())
          .expect("minimal Opening-failure funding value fits");
      }
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe
        .as_mut()
        .expect("minimal Opening-failure actor is active");
      hot.pending_signal = true;
      hot.queue_ticket = Some(9);
    });
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("minimal Opening-failure service state is coherent");
    let LoadedActorStateOf::Active(full_state) = Pallet::<T>::load_actor_state(actor_id) else {
      panic!("minimal Opening-failure full state exists");
    };
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      full_state.identity,
      full_state.hot,
      full_state.contract,
    );
    let control_context = Pallet::<T>::execution_step_control_weight_context(
      &execution_instance,
      state.run_state.as_ref(),
      &loaded_step,
    )
    .expect("minimal Opening-failure control context exists");
    assert_eq!(control_context.opening_tail_chunks, tail_chunks);
    assert_eq!(control_context.opening_snapshot_entries, 0);
    assert_eq!(control_context.opening_predicate_results, 0);
    assert_eq!(control_context.predicate_evaluation_units, 0);
    assert_eq!(
      T::StepControlWeight::maximum_control_weight(control_context, &loaded_step.step),
      Some(loaded_step.resources.control),
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("minimal Opening-failure hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("minimal Opening-failure Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("minimal Opening-failure maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("minimal Opening-failure plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("minimal Opening-failure inner atom commits");
    }
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    let hot = ActorHot::<T>::get(actor_id).expect("minimal Opening-failure hot state persists");
    assert_eq!(hot.cycle_state, CycleState::Idle);
    assert!(hot.queue_ticket.is_none());
    assert!(hot.wakeup_pointer.is_none());
    assert_eq!(hot.unsuccessful_attempt_streak, 1);
  }

  /// Measures direct minimal-geometry fresh-Opening retry at every C6 tail count. An unfunded
  /// fixed Transfer suspends at cursor zero and installs one wakeup without Opening capture.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_retry_min(t: Linear<0, 8>) {
    let _ = WakeupBuckets::<T>::clear(u32::MAX, None);
    let _ = WakeupPages::<T>::clear(u32::MAX, None);
    let _ = WakeupCursorPages::<T>::clear(u32::MAX, None);
    let _ = WakeupCursorLen::<T>::clear(u32::MAX, None);
    let tail_chunks = t.min(
      T::MaxContractSteps::get()
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK),
    );
    let step_count = 1u32
      .saturating_add(tail_chunks.saturating_mul(MAX_STEPS_PER_TAIL_CHUNK))
      .min(T::MaxContractSteps::get());
    let mut steps = inert_contract_steps_of_len::<T>(step_count);
    let actor_id = bench_create_system_manual::<T>(41_612_500);
    let funding_assets = T::BenchmarkHelper::funding_assets(T::MaxFundingTrackedAssets::get());
    let retry_asset = *funding_assets
      .first()
      .expect("minimal Opening-retry funding asset exists");
    steps[0].task = ActorTask::Transfer {
      to: account("opening_retry_to", 0, 0),
      asset: retry_asset,
      amount: AmountResolution::Fixed(One::one()),
    };
    steps[0].on_error = StepErrorPolicy::RetryLater {
      max_attempts: T::MaxRetryAttempts::get(),
    };
    let contract = system_contract::<T>(
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 2,
      },
      steps,
    )
    .expect("minimal Opening-retry Contract exists");
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("minimal Opening-retry geometry stores");
    ActorFunding::<T>::mutate(actor_id, |maybe| {
      let funding = maybe
        .as_mut()
        .expect("minimal Opening-retry funding exists");
      for asset in funding_assets {
        funding
          .funding_tracked_assets
          .try_insert(asset)
          .expect("minimal Opening-retry tracked asset fits");
        funding
          .funding_accumulated
          .try_insert(asset, One::one())
          .expect("minimal Opening-retry funding value fits");
      }
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe
        .as_mut()
        .expect("minimal Opening-retry actor is active");
      hot.pending_signal = true;
      hot.queue_ticket = Some(9);
    });
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("minimal Opening-retry service state is coherent");
    let LoadedActorStateOf::Active(full_state) = Pallet::<T>::load_actor_state(actor_id) else {
      panic!("minimal Opening-retry full state exists");
    };
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      full_state.identity,
      full_state.hot,
      full_state.contract,
    );
    let control_context = Pallet::<T>::execution_step_control_weight_context(
      &execution_instance,
      state.run_state.as_ref(),
      &loaded_step,
    )
    .expect("minimal Opening-retry control context exists");
    assert_eq!(control_context.opening_tail_chunks, tail_chunks);
    assert_eq!(control_context.opening_snapshot_entries, 0);
    assert_eq!(control_context.opening_predicate_results, 0);
    assert_eq!(control_context.predicate_evaluation_units, 0);
    assert_eq!(
      T::StepControlWeight::maximum_control_weight(control_context, &loaded_step.step),
      Some(loaded_step.resources.control),
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("minimal Opening-retry hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("minimal Opening-retry Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("minimal Opening-retry maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("minimal Opening-retry plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("minimal Opening-retry inner atom commits");
    }
    let run = ActorRunStateStore::<T>::get(actor_id).expect("minimal Opening-retry run persists");
    assert_eq!(run.cursor, 0);
    assert_eq!(run.unsuccessful_attempts_at_cursor, 1);
    assert_eq!(run.suspension, Some(SuspensionReason::FundingUnavailable));
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
      hot.cycle_state == CycleState::Suspended && hot.wakeup_pointer.is_some()
    }));
  }

  /// Measures direct maximum-realizable fresh-Opening failure at every C6 tail count. Opening
  /// dependencies are fully captured before an invalid split plan fails pre-effect and aborts.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_failed_max(t: Linear<0, 8>) {
    let tail_chunks = t.min(
      T::MaxContractSteps::get()
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK),
    );
    let step_count = 1u32
      .saturating_add(tail_chunks.saturating_mul(MAX_STEPS_PER_TAIL_CHUNK))
      .min(T::MaxContractSteps::get());
    let opening_asset_count = step_count
      .checked_mul(2)
      .expect("Opening-failure asset count fits");
    let setup_asset_count = opening_asset_count.max(T::MaxFundingTrackedAssets::get());
    let asset_owner: T::AccountId = account("opening_failure_assets", 0, 0);
    let assets = T::BenchmarkHelper::setup_predicate_assets(&asset_owner, setup_asset_count)
      .expect("Opening-failure assets exist");
    let invalid_legs = SplitTransferLegsOf::<T>::try_from(alloc::vec![
      SplitLeg {
        to: account("opening_failure_max_leg", 0, 0),
        share: Perbill::from_percent(50),
      },
      SplitLeg {
        to: account("opening_failure_max_leg", 0, 0),
        share: Perbill::from_percent(50),
      },
    ])
    .expect("Opening-failure split legs fit");
    let mut steps = ContractSteps::<T>::default();
    for (step_index, pair) in assets
      .iter(/* deos-bypass: bounded-iter */)
      .take(opening_asset_count as usize)
      .copied()
      .collect::<alloc::vec::Vec<_>>()
      .chunks_exact(2)
      .enumerate()
    {
      let asset_a = pair[0];
      let asset_b = pair[1];
      let predicates = alloc::vec![
        Predicate::BalanceEquals {
          asset: asset_a,
          threshold: Zero::zero(),
        },
        Predicate::BalanceBelow {
          asset: asset_a,
          threshold: One::one(),
        },
        Predicate::BalanceNotEquals {
          asset: asset_b,
          threshold: One::one(),
        },
        Predicate::BalanceEquals {
          asset: asset_b,
          threshold: Zero::zero(),
        },
      ];
      let task = if step_index == 0 {
        ActorTask::SplitTransfer {
          asset: T::FeeNativeAssetId::get(),
          amount: AmountResolution::Fixed(One::one()),
          legs: invalid_legs.clone(),
        }
      } else {
        ActorTask::AddLiquidity {
          asset_a,
          asset_b,
          amount_a: AmountResolution::PercentageAtOpening(Perbill::one()),
          amount_b: AmountResolution::PercentageAtOpening(Perbill::one()),
          min_lp_out: One::one(),
        }
      };
      steps
        .try_push(Step {
          precondition: Some(opening_all::<T>(predicates)),
          task,
          on_error: StepErrorPolicy::AbortCycle,
        })
        .expect("Opening-failure Contract fits");
    }
    let actor_id = bench_create_system_manual::<T>(41_615_625);
    let contract = system_contract::<T>(
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 0,
      },
      steps,
    )
    .expect("Opening-failure Contract exists");
    Pallet::<T>::store_actor_contract(actor_id, contract).expect("Opening-failure geometry stores");
    ActorFunding::<T>::mutate(actor_id, |maybe| {
      let funding = maybe
        .as_mut()
        .expect("Opening-failure actor funding exists");
      for asset in assets
        .iter(/* deos-bypass: bounded-iter */)
        .take(T::MaxFundingTrackedAssets::get() as usize)
      {
        funding
          .funding_tracked_assets
          .try_insert(*asset)
          .expect("Opening-failure tracked asset fits");
        funding
          .funding_accumulated
          .try_insert(*asset, One::one())
          .expect("Opening-failure funding snapshot fits");
      }
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe.as_mut().expect("Opening-failure actor is active");
      hot.pending_signal = true;
      hot.queue_ticket = Some(9);
    });
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("Opening-failure service state is coherent");
    let LoadedActorStateOf::Active(full_state) = Pallet::<T>::load_actor_state(actor_id) else {
      panic!("Opening-failure full state exists");
    };
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      full_state.identity,
      full_state.hot,
      full_state.contract,
    );
    let control_context = Pallet::<T>::execution_step_control_weight_context(
      &execution_instance,
      state.run_state.as_ref(),
      &loaded_step,
    )
    .expect("Opening-failure control context exists");
    assert_eq!(control_context.opening_tail_chunks, tail_chunks);
    assert_eq!(
      control_context.opening_snapshot_entries,
      step_count.saturating_sub(1).saturating_mul(2),
    );
    assert_eq!(
      control_context.opening_predicate_results,
      step_count.saturating_mul(T::MaxPredicatesPerStep::get()),
    );
    assert_eq!(control_context.predicate_evaluation_units, 8);
    assert_eq!(
      T::StepControlWeight::maximum_control_weight(control_context, &loaded_step.step),
      Some(loaded_step.resources.control),
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Opening-failure hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("Opening-failure Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("Opening-failure maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("Opening-failure plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("Opening-failure inner atom commits");
    }
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    let hot = ActorHot::<T>::get(actor_id).expect("Opening-failure hot state persists");
    assert_eq!(hot.cycle_state, CycleState::Idle);
    assert_eq!(hot.unsuccessful_attempt_streak, 1);
  }

  /// Measures direct maximum-realizable fresh-Opening retry at every C6 tail count. Four true
  /// Opening predicates per Step and two tail-Step Opening surfaces are captured before an
  /// unfunded PercentageOfLastFunding Transfer suspends cursor zero into one wakeup.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_retry_max(t: Linear<0, 8>) {
    let _ = WakeupBuckets::<T>::clear(u32::MAX, None);
    let _ = WakeupPages::<T>::clear(u32::MAX, None);
    let _ = WakeupCursorPages::<T>::clear(u32::MAX, None);
    let _ = WakeupCursorLen::<T>::clear(u32::MAX, None);
    let tail_chunks = t.min(
      T::MaxContractSteps::get()
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK),
    );
    let step_count = 1u32
      .saturating_add(tail_chunks.saturating_mul(MAX_STEPS_PER_TAIL_CHUNK))
      .min(T::MaxContractSteps::get());
    let opening_asset_count = step_count
      .checked_mul(2)
      .expect("Opening-retry asset count fits");
    let setup_asset_count = opening_asset_count.max(T::MaxFundingTrackedAssets::get());
    let asset_owner: T::AccountId = account("opening_retry_assets", 0, 0);
    let assets = T::BenchmarkHelper::setup_predicate_assets(&asset_owner, setup_asset_count)
      .expect("Opening-retry assets exist");
    assert_eq!(assets.len(), setup_asset_count as usize);
    let recipient: T::AccountId = account("opening_retry_recipient", 0, 0);
    let mut steps = ContractSteps::<T>::default();
    for (step_index, pair) in assets
      .iter(/* deos-bypass: bounded-iter */)
      .take(opening_asset_count as usize)
      .copied()
      .collect::<alloc::vec::Vec<_>>()
      .chunks_exact(2)
      .enumerate()
    {
      let asset_a = pair[0];
      let asset_b = pair[1];
      let predicates = alloc::vec![
        Predicate::BalanceEquals {
          asset: asset_a,
          threshold: Zero::zero(),
        },
        Predicate::BalanceBelow {
          asset: asset_a,
          threshold: One::one(),
        },
        Predicate::BalanceNotEquals {
          asset: asset_b,
          threshold: One::one(),
        },
        Predicate::BalanceEquals {
          asset: asset_b,
          threshold: Zero::zero(),
        },
      ];
      let task = if step_index == 0 {
        ActorTask::Transfer {
          to: recipient.clone(),
          asset: asset_a,
          amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
        }
      } else {
        ActorTask::AddLiquidity {
          asset_a,
          asset_b,
          amount_a: AmountResolution::PercentageAtOpening(Perbill::one()),
          amount_b: AmountResolution::PercentageAtOpening(Perbill::one()),
          min_lp_out: One::one(),
        }
      };
      steps
        .try_push(Step {
          precondition: Some(opening_all::<T>(predicates)),
          task,
          on_error: if step_index == 0 {
            StepErrorPolicy::RetryLater {
              max_attempts: T::MaxRetryAttempts::get(),
            }
          } else {
            StepErrorPolicy::AbortCycle
          },
        })
        .expect("Opening-retry Contract fits");
    }
    let actor_id = bench_create_system_manual::<T>(41_618_750);
    let contract = system_contract::<T>(
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 2,
      },
      steps,
    )
    .expect("Opening-retry Contract exists");
    Pallet::<T>::store_actor_contract(actor_id, contract).expect("Opening-retry geometry stores");
    ActorFunding::<T>::mutate(actor_id, |maybe| {
      let funding = maybe.as_mut().expect("Opening-retry actor funding exists");
      for asset in assets
        .iter(/* deos-bypass: bounded-iter */)
        .take(T::MaxFundingTrackedAssets::get() as usize)
      {
        funding
          .funding_tracked_assets
          .try_insert(*asset)
          .expect("Opening-retry tracked asset fits");
        funding
          .funding_accumulated
          .try_insert(*asset, One::one())
          .expect("Opening-retry funding snapshot fits");
      }
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe.as_mut().expect("Opening-retry actor is active");
      hot.pending_signal = true;
      hot.queue_ticket = Some(9);
    });
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("Opening-retry service state is coherent");
    let LoadedActorStateOf::Active(full_state) = Pallet::<T>::load_actor_state(actor_id) else {
      panic!("Opening-retry full state exists");
    };
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      full_state.identity,
      full_state.hot,
      full_state.contract,
    );
    let control_context = Pallet::<T>::execution_step_control_weight_context(
      &execution_instance,
      state.run_state.as_ref(),
      &loaded_step,
    )
    .expect("Opening-retry control context exists");
    assert_eq!(control_context.opening_tail_chunks, tail_chunks);
    assert_eq!(
      control_context.opening_snapshot_entries,
      step_count.saturating_sub(1).saturating_mul(2),
    );
    assert_eq!(
      control_context.opening_predicate_results,
      step_count
        .checked_mul(T::MaxPredicatesPerStep::get())
        .expect("Opening-retry predicate count fits"),
    );
    assert_eq!(control_context.predicate_evaluation_units, 8);
    assert_eq!(
      T::StepControlWeight::maximum_control_weight(control_context, &loaded_step.step),
      Some(loaded_step.resources.control),
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Opening-retry hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("Opening-retry Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("Opening-retry maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("Opening-retry plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("Opening-retry inner atom commits");
    }
    let run = ActorRunStateStore::<T>::get(actor_id).expect("Opening-retry run persists");
    assert_eq!(run.cursor, 0);
    assert_eq!(run.unsuccessful_attempts_at_cursor, 1);
    assert_eq!(run.suspension, Some(SuspensionReason::FundingUnavailable));
    assert_eq!(
      run.opening_snapshot.len(),
      step_count.saturating_sub(1).saturating_mul(2) as usize,
    );
    assert_eq!(
      run.opening_predicate_results.len(),
      step_count.saturating_mul(T::MaxPredicatesPerStep::get()) as usize,
    );
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
      hot.cycle_state == CycleState::Suspended && hot.wakeup_pointer.is_some()
    }));
  }

  /// Measures direct minimal-geometry fresh-Opening completion at every C6 tail count. StopCycle
  /// terminates the cycle without an effect, Opening capture, run persistence, or placement.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_complete_min(t: Linear<0, 8>) {
    let tail_chunks = t.min(
      T::MaxContractSteps::get()
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK),
    );
    let step_count = 1u32
      .saturating_add(tail_chunks.saturating_mul(MAX_STEPS_PER_TAIL_CHUNK))
      .min(T::MaxContractSteps::get());
    let mut steps = inert_contract_steps_of_len::<T>(step_count);
    steps[0].task = ActorTask::StopCycle;
    let actor_id = bench_create_system_manual::<T>(41_625_000);
    let contract = system_contract::<T>(
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 0,
      },
      steps,
    )
    .expect("minimal Opening-completion Contract exists");
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("minimal Opening-completion geometry stores");
    let funding_assets = T::BenchmarkHelper::funding_assets(T::MaxFundingTrackedAssets::get());
    ActorFunding::<T>::mutate(actor_id, |maybe| {
      let funding = maybe
        .as_mut()
        .expect("minimal Opening-completion funding exists");
      for asset in funding_assets {
        funding
          .funding_tracked_assets
          .try_insert(asset)
          .expect("minimal Opening-completion tracked asset fits");
        funding
          .funding_accumulated
          .try_insert(asset, One::one())
          .expect("minimal Opening-completion funding value fits");
      }
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe
        .as_mut()
        .expect("minimal Opening-completion actor is active");
      hot.pending_signal = true;
      hot.queue_ticket = Some(9);
    });
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("minimal Opening-completion service state is coherent");
    let LoadedActorStateOf::Active(full_state) = Pallet::<T>::load_actor_state(actor_id) else {
      panic!("minimal Opening-completion full state exists");
    };
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      full_state.identity,
      full_state.hot,
      full_state.contract,
    );
    let control_context = Pallet::<T>::execution_step_control_weight_context(
      &execution_instance,
      state.run_state.as_ref(),
      &loaded_step,
    )
    .expect("minimal Opening-completion control context exists");
    assert_eq!(control_context.opening_tail_chunks, tail_chunks);
    assert_eq!(control_context.opening_snapshot_entries, 0);
    assert_eq!(control_context.opening_predicate_results, 0);
    assert_eq!(control_context.predicate_evaluation_units, 0);
    assert_eq!(
      T::StepControlWeight::maximum_control_weight(control_context, &loaded_step.step),
      Some(loaded_step.resources.control),
    );
    let _ = QueuePages::<T>::clear(u32::MAX, None);
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(0);
    QueueOccupancy::<T>::put(0);
    NextQueueTicket::<T>::put(0);
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("minimal Opening-completion hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("minimal Opening-completion Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("minimal Opening-completion maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("minimal Opening-completion plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("minimal Opening-completion inner atom commits");
    }
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    assert_eq!(QueueOccupancy::<T>::get(), 0);
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
      hot.cycle_state == CycleState::Idle
        && hot.queue_ticket.is_none()
        && hot.wakeup_pointer.is_none()
    }));
  }

  /// Measures the direct minimal-geometry fresh-Opening progress path at every C6 tail count.
  /// The current fixed-zero Transfer resolves to an effect-free skip; no Step contributes Opening
  /// surfaces or predicates, while the conservative funding payload is fully materialized.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_progress_min(t: Linear<1, 8>) {
    let tail_chunks = t.min(
      T::MaxContractSteps::get()
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK),
    );
    let step_count = 1u32
      .saturating_add(tail_chunks.saturating_mul(MAX_STEPS_PER_TAIL_CHUNK))
      .min(T::MaxContractSteps::get());
    let mut steps = inert_contract_steps_of_len::<T>(step_count);
    let actor_id = bench_create_system_manual::<T>(41_650_000);
    let actor = Pallet::<T>::sovereign_account_id_system(actor_id);
    steps[0].task = ActorTask::Transfer {
      to: actor,
      asset: T::FeeNativeAssetId::get(),
      amount: AmountResolution::Fixed(Zero::zero()),
    };
    let contract = system_contract::<T>(
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 0,
      },
      steps,
    )
    .expect("minimal Opening-progress Contract exists");
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("minimal Opening-progress geometry stores");
    let funding_assets = T::BenchmarkHelper::funding_assets(T::MaxFundingTrackedAssets::get());
    ActorFunding::<T>::mutate(actor_id, |maybe| {
      let funding = maybe
        .as_mut()
        .expect("minimal Opening-progress funding exists");
      for asset in funding_assets {
        funding
          .funding_tracked_assets
          .try_insert(asset)
          .expect("minimal Opening-progress tracked asset fits");
        funding
          .funding_accumulated
          .try_insert(asset, One::one())
          .expect("minimal Opening-progress funding value fits");
      }
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe
        .as_mut()
        .expect("minimal Opening-progress actor is active");
      hot.pending_signal = true;
      hot.queue_ticket = Some(9);
    });
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("minimal Opening-progress service state is coherent");
    let LoadedActorStateOf::Active(full_state) = Pallet::<T>::load_actor_state(actor_id) else {
      panic!("minimal Opening-progress full state exists");
    };
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      full_state.identity,
      full_state.hot,
      full_state.contract,
    );
    let control_context = Pallet::<T>::execution_step_control_weight_context(
      &execution_instance,
      state.run_state.as_ref(),
      &loaded_step,
    )
    .expect("minimal Opening-progress control context exists");
    assert_eq!(control_context.opening_tail_chunks, tail_chunks);
    assert_eq!(control_context.opening_snapshot_entries, 0);
    assert_eq!(control_context.opening_predicate_results, 0);
    assert_eq!(control_context.predicate_evaluation_units, 0);
    assert_eq!(
      T::StepControlWeight::maximum_control_weight(control_context, &loaded_step.step),
      Some(loaded_step.resources.control),
    );
    let _ = QueuePages::<T>::clear(u32::MAX, None);
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(0);
    QueueOccupancy::<T>::put(0);
    NextQueueTicket::<T>::put(0);
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("minimal Opening-progress hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("minimal Opening-progress Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("minimal Opening-progress maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("minimal Opening-progress plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("minimal Opening-progress inner atom commits");
    }
    let run =
      ActorRunStateStore::<T>::get(actor_id).expect("minimal Opening-progress run persists");
    assert_eq!(run.cursor, 1);
    assert_eq!(run.opening_snapshot.len(), 0);
    assert_eq!(run.opening_predicate_results.len(), 0);
    assert_eq!(
      run.funding_snapshot.len(),
      T::MaxFundingTrackedAssets::get() as usize,
    );
    assert_eq!(QueueOccupancy::<T>::get(), 1);
  }

  /// Measures direct maximum-realizable fresh-Opening Actor close at every C6 tail count. Opening
  /// dependencies are captured before StopCycle reaches auto-close nonce one and removes the Actor.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_close_max(t: Linear<0, 8>) {
    let tail_chunks = t.min(
      T::MaxContractSteps::get()
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK),
    );
    let step_count = 1u32
      .saturating_add(tail_chunks.saturating_mul(MAX_STEPS_PER_TAIL_CHUNK))
      .min(T::MaxContractSteps::get());
    let opening_asset_count = step_count
      .checked_mul(2)
      .expect("Opening-close asset count fits");
    let setup_asset_count = opening_asset_count.max(T::MaxFundingTrackedAssets::get());
    let asset_owner: T::AccountId = account("opening_close_assets", 0, 0);
    let assets = T::BenchmarkHelper::setup_predicate_assets(&asset_owner, setup_asset_count)
      .expect("Opening-close assets exist");
    let mut steps = ContractSteps::<T>::default();
    for (step_index, pair) in assets
      .iter(/* deos-bypass: bounded-iter */)
      .take(opening_asset_count as usize)
      .copied()
      .collect::<alloc::vec::Vec<_>>()
      .chunks_exact(2)
      .enumerate()
    {
      let asset_a = pair[0];
      let asset_b = pair[1];
      let predicates = alloc::vec![
        Predicate::BalanceEquals {
          asset: asset_a,
          threshold: Zero::zero(),
        },
        Predicate::BalanceBelow {
          asset: asset_a,
          threshold: One::one(),
        },
        Predicate::BalanceNotEquals {
          asset: asset_b,
          threshold: One::one(),
        },
        Predicate::BalanceEquals {
          asset: asset_b,
          threshold: Zero::zero(),
        },
      ];
      let task = if step_index == 0 {
        ActorTask::StopCycle
      } else {
        ActorTask::AddLiquidity {
          asset_a,
          asset_b,
          amount_a: AmountResolution::PercentageAtOpening(Perbill::one()),
          amount_b: AmountResolution::PercentageAtOpening(Perbill::one()),
          min_lp_out: One::one(),
        }
      };
      steps
        .try_push(Step {
          precondition: Some(opening_all::<T>(predicates)),
          task,
          on_error: StepErrorPolicy::AbortCycle,
        })
        .expect("Opening-close Contract fits");
    }
    let actor_id = bench_create_system_manual::<T>(41_671_875);
    let mut contract = system_contract::<T>(
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 0,
      },
      steps,
    )
    .expect("Opening-close Contract exists");
    contract.auto_close_at_cycle_nonce = Some(1);
    Pallet::<T>::store_actor_contract(actor_id, contract).expect("Opening-close geometry stores");
    ActorFunding::<T>::mutate(actor_id, |maybe| {
      let funding = maybe.as_mut().expect("Opening-close actor funding exists");
      for asset in assets
        .iter(/* deos-bypass: bounded-iter */)
        .take(T::MaxFundingTrackedAssets::get() as usize)
      {
        funding
          .funding_tracked_assets
          .try_insert(*asset)
          .expect("Opening-close tracked asset fits");
        funding
          .funding_accumulated
          .try_insert(*asset, One::one())
          .expect("Opening-close funding snapshot fits");
      }
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe.as_mut().expect("Opening-close actor is active");
      hot.pending_signal = true;
      hot.queue_ticket = Some(9);
    });
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("Opening-close service state is coherent");
    let LoadedActorStateOf::Active(full_state) = Pallet::<T>::load_actor_state(actor_id) else {
      panic!("Opening-close full state exists");
    };
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      full_state.identity,
      full_state.hot,
      full_state.contract,
    );
    let control_context = Pallet::<T>::execution_step_control_weight_context(
      &execution_instance,
      state.run_state.as_ref(),
      &loaded_step,
    )
    .expect("Opening-close control context exists");
    assert_eq!(control_context.opening_tail_chunks, tail_chunks);
    assert_eq!(
      control_context.opening_snapshot_entries,
      step_count.saturating_sub(1).saturating_mul(2),
    );
    assert_eq!(
      control_context.opening_predicate_results,
      step_count.saturating_mul(T::MaxPredicatesPerStep::get()),
    );
    assert_eq!(control_context.predicate_evaluation_units, 8);
    assert_eq!(
      T::StepControlWeight::maximum_control_weight(control_context, &loaded_step.step),
      Some(loaded_step.resources.control),
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Opening-close hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("Opening-close Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("Opening-close maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("Opening-close plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("Opening-close inner atom commits");
    }
    assert!(ActorHot::<T>::get(actor_id).is_none());
    assert!(ActorIdentities::<T>::get(actor_id).is_none());
    assert!(ActorContractHeads::<T>::get(actor_id).is_none());
    assert_eq!(
      ActorContractTailChunks::<T>::iter_prefix(actor_id).count(),
      0,
    );
  }

  /// Measures direct maximum-geometry fresh-Opening completion at every C6 tail count. Every
  /// authored Step contributes two Opening surfaces and four true Opening predicates; Step 0
  /// executes effect-free StopCycle and leaves no run or placement.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_complete_max(t: Linear<0, 8>) {
    let tail_chunks = t.min(
      T::MaxContractSteps::get()
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK),
    );
    let step_count = 1u32
      .saturating_add(tail_chunks.saturating_mul(MAX_STEPS_PER_TAIL_CHUNK))
      .min(T::MaxContractSteps::get());
    let opening_asset_count = step_count
      .checked_mul(2)
      .expect("Opening-completion asset count fits");
    let setup_asset_count = opening_asset_count.max(T::MaxFundingTrackedAssets::get());
    let asset_owner: T::AccountId = account("opening_complete_assets", 0, 0);
    let assets = T::BenchmarkHelper::setup_predicate_assets(&asset_owner, setup_asset_count)
      .expect("Opening-completion assets exist");
    assert_eq!(assets.len(), setup_asset_count as usize);
    let mut steps = ContractSteps::<T>::default();
    for (step_index, pair) in assets
      .iter(/* deos-bypass: bounded-iter */)
      .take(opening_asset_count as usize)
      .copied()
      .collect::<alloc::vec::Vec<_>>()
      .chunks_exact(2)
      .enumerate()
    {
      let asset_a = pair[0];
      let asset_b = pair[1];
      let predicates = alloc::vec![
        Predicate::BalanceEquals {
          asset: asset_a,
          threshold: Zero::zero(),
        },
        Predicate::BalanceBelow {
          asset: asset_a,
          threshold: One::one(),
        },
        Predicate::BalanceNotEquals {
          asset: asset_b,
          threshold: One::one(),
        },
        Predicate::BalanceEquals {
          asset: asset_b,
          threshold: Zero::zero(),
        },
      ];
      let task = if step_index == 0 {
        ActorTask::StopCycle
      } else {
        ActorTask::AddLiquidity {
          asset_a,
          asset_b,
          amount_a: AmountResolution::PercentageAtOpening(Perbill::one()),
          amount_b: AmountResolution::PercentageAtOpening(Perbill::one()),
          min_lp_out: One::one(),
        }
      };
      steps
        .try_push(Step {
          precondition: Some(opening_all::<T>(predicates)),
          task,
          on_error: StepErrorPolicy::AbortCycle,
        })
        .expect("Opening-completion Contract fits");
    }
    assert_eq!(steps.len(), step_count as usize);
    let actor_id = bench_create_system_manual::<T>(41_675_000);
    let contract = system_contract::<T>(
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 0,
      },
      steps,
    )
    .expect("Opening-completion Contract exists");
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("Opening-completion geometry stores");
    ActorFunding::<T>::mutate(actor_id, |maybe| {
      let funding = maybe
        .as_mut()
        .expect("Opening-completion actor funding exists");
      for asset in assets
        .iter(/* deos-bypass: bounded-iter */)
        .take(T::MaxFundingTrackedAssets::get() as usize)
      {
        funding
          .funding_tracked_assets
          .try_insert(*asset)
          .expect("Opening-completion tracked asset fits");
        funding
          .funding_accumulated
          .try_insert(*asset, One::one())
          .expect("Opening-completion funding snapshot fits");
      }
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe.as_mut().expect("Opening-completion actor is active");
      hot.pending_signal = true;
      hot.queue_ticket = Some(9);
    });
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("Opening-completion service state is coherent");
    let LoadedActorStateOf::Active(full_state) = Pallet::<T>::load_actor_state(actor_id) else {
      panic!("Opening-completion full state exists");
    };
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      full_state.identity,
      full_state.hot,
      full_state.contract,
    );
    let control_context = Pallet::<T>::execution_step_control_weight_context(
      &execution_instance,
      state.run_state.as_ref(),
      &loaded_step,
    )
    .expect("Opening-completion control context exists");
    assert_eq!(control_context.opening_tail_chunks, tail_chunks);
    assert_eq!(
      control_context.opening_snapshot_entries,
      step_count.saturating_sub(1).saturating_mul(2),
    );
    assert_eq!(
      control_context.opening_predicate_results,
      step_count
        .checked_mul(T::MaxPredicatesPerStep::get())
        .expect("Opening-completion predicate count fits"),
    );
    assert_eq!(control_context.predicate_evaluation_units, 8);
    assert_eq!(
      T::StepControlWeight::maximum_control_weight(control_context, &loaded_step.step),
      Some(loaded_step.resources.control),
    );
    let _ = QueuePages::<T>::clear(u32::MAX, None);
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(0);
    QueueOccupancy::<T>::put(0);
    NextQueueTicket::<T>::put(0);
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Opening-completion hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("Opening-completion Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("Opening-completion maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("Opening-completion plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("Opening-completion inner atom commits");
    }
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    assert_eq!(QueueOccupancy::<T>::get(), 0);
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
      hot.cycle_state == CycleState::Idle
        && hot.queue_ticket.is_none()
        && hot.wakeup_pointer.is_none()
    }));
  }

  /// Measures direct maximum-geometry fresh-Opening progress after queue discovery, complete state
  /// loading, and physical head consumption. Each tail geometry materializes the maximum realizable Contract
  /// for that chunk count, maximum funding payload, four Opening predicates and two Opening amount
  /// surfaces per Step, and effect-free false evaluation before causal FIFO successor placement.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_opening_progress_max(t: Linear<1, 8>) {
    let tail_chunks = t.min(
      T::MaxContractSteps::get()
        .saturating_sub(1)
        .div_ceil(MAX_STEPS_PER_TAIL_CHUNK),
    );
    let step_count = 1u32
      .saturating_add(tail_chunks.saturating_mul(MAX_STEPS_PER_TAIL_CHUNK))
      .min(T::MaxContractSteps::get());
    let opening_asset_count = step_count
      .checked_mul(2)
      .expect("Opening-progress asset count fits");
    let setup_asset_count = opening_asset_count.max(T::MaxFundingTrackedAssets::get());
    let asset_owner: T::AccountId = account("opening_inner_assets", 0, 0);
    let assets = T::BenchmarkHelper::setup_predicate_assets(&asset_owner, setup_asset_count)
      .expect("Opening-progress assets exist");
    assert_eq!(assets.len(), setup_asset_count as usize);
    let mut steps = ContractSteps::<T>::default();
    for pair in assets
      .iter(/* deos-bypass: bounded-iter */)
      .take(opening_asset_count as usize)
      .copied()
      .collect::<alloc::vec::Vec<_>>()
      .chunks_exact(2)
    {
      let asset_a = pair[0];
      let asset_b = pair[1];
      let predicates = alloc::vec![
        Predicate::BalanceAbove {
          asset: asset_a,
          threshold: One::one(),
        },
        Predicate::BalanceBelow {
          asset: asset_a,
          threshold: One::one(),
        },
        Predicate::BalanceEquals {
          asset: asset_b,
          threshold: Zero::zero(),
        },
        Predicate::BalanceNotEquals {
          asset: asset_b,
          threshold: One::one(),
        },
      ];
      steps
        .try_push(Step {
          precondition: Some(opening_all::<T>(predicates)),
          task: ActorTask::AddLiquidity {
            asset_a,
            asset_b,
            amount_a: AmountResolution::PercentageAtOpening(Perbill::one()),
            amount_b: AmountResolution::PercentageAtOpening(Perbill::one()),
            min_lp_out: One::one(),
          },
          on_error: StepErrorPolicy::AbortCycle,
        })
        .expect("Opening-progress Contract fits");
    }
    assert_eq!(steps.len(), step_count as usize);
    let actor_id = bench_create_system_manual::<T>(41_700_000);
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 0,
    };
    let contract = system_contract::<T>(schedule, steps).expect("Opening-progress Contract exists");
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("Opening-progress geometry stores");
    ActorFunding::<T>::mutate(actor_id, |maybe| {
      let funding = maybe
        .as_mut()
        .expect("Opening-progress actor funding exists");
      for asset in assets
        .iter(/* deos-bypass: bounded-iter */)
        .take(T::MaxFundingTrackedAssets::get() as usize)
      {
        funding
          .funding_tracked_assets
          .try_insert(*asset)
          .expect("Opening-progress tracked asset fits");
        funding
          .funding_accumulated
          .try_insert(*asset, One::one())
          .expect("Opening-progress funding snapshot fits");
      }
    });
    assert_eq!(
      ActorFunding::<T>::get(actor_id)
        .expect("Opening-progress funding exists")
        .funding_accumulated
        .len(),
      T::MaxFundingTrackedAssets::get() as usize,
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe.as_mut().expect("Opening-progress actor is active");
      hot.pending_signal = true;
      hot.queue_ticket = Some(9);
    });
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("Opening-progress service state is coherent");
    let LoadedActorStateOf::Active(full_state) = Pallet::<T>::load_actor_state(actor_id) else {
      panic!("Opening-progress full state exists");
    };
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      full_state.identity,
      full_state.hot,
      full_state.contract,
    );
    let control_context = Pallet::<T>::execution_step_control_weight_context(
      &execution_instance,
      state.run_state.as_ref(),
      &loaded_step,
    )
    .expect("Opening-progress control context exists");
    assert_eq!(
      control_context.opening_tail_chunks, tail_chunks,
      "Opening-progress context carries authored tail geometry",
    );
    assert_eq!(
      T::StepControlWeight::maximum_control_weight(control_context, &loaded_step.step),
      Some(loaded_step.resources.control),
      "Opening-progress execution context matches admission",
    );
    let _ = QueuePages::<T>::clear(u32::MAX, None);
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(0);
    QueueOccupancy::<T>::put(0);
    NextQueueTicket::<T>::put(0);
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Opening-progress hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("Opening-progress Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("Opening-progress maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("Opening-progress plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("Opening-progress inner atom commits");
    }
    let run = ActorRunStateStore::<T>::get(actor_id).expect("Opening-progress run persists");
    assert_eq!(run.cursor, 1);
    assert_eq!(run.opening_snapshot.len(), opening_asset_count as usize);
    assert_eq!(
      run.funding_snapshot.len(),
      T::MaxFundingTrackedAssets::get() as usize,
    );
    assert_eq!(
      run.opening_predicate_results.len(),
      step_count
        .checked_mul(T::MaxPredicatesPerStep::get())
        .expect("Opening-progress predicate count fits") as usize,
    );
    assert_eq!(run.last_committed_step_block, Some(now));
    assert_eq!(QueueOccupancy::<T>::get(), 1);
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| hot.queue_ticket.is_some()));
  }

  /// Measures the direct inner Running-final control owner after queue discovery, current-state
  /// loading, and physical head consumption. The measured atom builds the exact carried plan,
  /// evaluates one current Step, commits completion, validates actual evidence, and performs
  /// post-placement. Task effect Weight is zero for StopCycle and false-predicate branches.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_running_complete(s: Linear<1, 4>, p: Linear<0, 4>) {
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 2u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let actor_id =
      bench_create_system_with_plan::<T>(40_600_000, inert_contract_steps_of_len::<T>(1 + s));
    let actor = Pallet::<T>::sovereign_account_id_system(actor_id);
    let predicate_count = p.min(T::MaxPredicatesPerStep::get());
    let assets = T::BenchmarkHelper::setup_predicate_assets(&actor, predicate_count)
      .expect("Running predicate assets exist");
    assert_eq!(assets.len(), predicate_count as usize);
    let mut contract =
      Pallet::<T>::load_actor_contract(actor_id).expect("Running benchmark Contract exists");
    if predicate_count > 0 {
      let predicates = assets
        .into_iter()
        .map(|asset| Predicate::BalanceAbove {
          asset,
          threshold: One::one(),
        })
        .collect::<alloc::vec::Vec<_>>();
      contract.steps[s as usize].precondition = Some(current_any::<T>(predicates));
    }
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("Running benchmark Contract remains admitted");
    install_run_state::<T>(actor_id, 0);
    ActorRunStateStore::<T>::mutate(actor_id, |maybe| {
      let run = maybe.as_mut().expect("Running benchmark run exists");
      maximize_run_state_geometry::<T>(run);
      run.cursor = s;
      run.opening_predicate_cursor = 0;
      run.unsuccessful_attempts_at_cursor = 0;
      run.cumulative_outcomes = OutcomeTotals::default();
      run.last_step_outcome = None;
      run.last_committed_step_block = Some(1u32.into());
      run.eligible_at = now;
      run.suspension = None;
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe.as_mut().expect("Running benchmark hot state exists");
      hot.cycle_state = CycleState::Running;
      hot.queue_ticket = Some(9);
      hot.wakeup_pointer = None;
    });
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("Running benchmark service state is coherent");
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Running benchmark hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("Running benchmark Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("Running benchmark maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("Running benchmark plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("Running benchmark inner atom commits");
    }
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    assert_eq!(
      ActorIdentities::<T>::get(actor_id)
        .expect("Running benchmark identity remains")
        .cycle_nonce,
      1,
    );
  }

  /// Measures the direct inner Running-progress owner through causal FIFO successor placement.
  /// A fixed-zero Transfer or false current predicates skip without Task effect execution.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_running_progress(s: Linear<2, 4>, p: Linear<0, 4>) {
    let _ = QueuePages::<T>::clear(u32::MAX, None);
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(0);
    QueueOccupancy::<T>::put(0);
    NextQueueTicket::<T>::put(0);
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 2u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let actor_id =
      bench_create_system_with_plan::<T>(40_700_000, inert_contract_steps_of_len::<T>(1 + s));
    let actor = Pallet::<T>::sovereign_account_id_system(actor_id);
    let predicate_count = p.min(T::MaxPredicatesPerStep::get());
    let assets = T::BenchmarkHelper::setup_predicate_assets(&actor, predicate_count)
      .expect("Running-progress predicate assets exist");
    assert_eq!(assets.len(), predicate_count as usize);
    let mut contract =
      Pallet::<T>::load_actor_contract(actor_id).expect("Running-progress Contract exists");
    contract.steps[1].task = ActorTask::Transfer {
      to: actor.clone(),
      asset: T::FeeNativeAssetId::get(),
      amount: AmountResolution::Fixed(Zero::zero()),
    };
    if predicate_count > 0 {
      let predicates = assets
        .into_iter()
        .map(|asset| Predicate::BalanceAbove {
          asset,
          threshold: One::one(),
        })
        .collect::<alloc::vec::Vec<_>>();
      contract.steps[1].precondition = Some(current_any::<T>(predicates));
    }
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("Running-progress Contract remains admitted");
    install_run_state::<T>(actor_id, 0);
    ActorRunStateStore::<T>::mutate(actor_id, |maybe| {
      let run = maybe.as_mut().expect("Running-progress run exists");
      maximize_run_state_geometry::<T>(run);
      run.cursor = 1;
      run.opening_predicate_cursor = 0;
      run.unsuccessful_attempts_at_cursor = 0;
      run.cumulative_outcomes = OutcomeTotals::default();
      run.last_step_outcome = None;
      run.last_committed_step_block = Some(1u32.into());
      run.eligible_at = now;
      run.suspension = None;
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe.as_mut().expect("Running-progress hot state exists");
      hot.cycle_state = CycleState::Running;
      hot.queue_ticket = Some(9);
      hot.wakeup_pointer = None;
    });
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("Running-progress service state is coherent");
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Running-progress hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("Running-progress Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("Running-progress maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("Running-progress plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("Running-progress inner atom commits");
    }
    let run = ActorRunStateStore::<T>::get(actor_id).expect("Running-progress run remains");
    assert_eq!(run.cursor, 2);
    assert_eq!(run.last_committed_step_block, Some(now));
    assert_eq!(QueueOccupancy::<T>::get(), 1);
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| hot.queue_ticket.is_some()));
  }

  /// Measures the direct inner Suspended-tail retry owner through durable wakeup placement.
  /// Current predicates are true and an unfunded fixed Transfer yields FundingUnavailable before
  /// Task effect invocation, preserving retry classification without effect contamination.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_tail_retry(s: Linear<1, 4>, p: Linear<0, 4>) {
    let _ = WakeupBuckets::<T>::clear(u32::MAX, None);
    let _ = WakeupPages::<T>::clear(u32::MAX, None);
    let _ = WakeupCursorPages::<T>::clear(u32::MAX, None);
    let _ = WakeupCursorLen::<T>::clear(u32::MAX, None);
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 2u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let actor_id =
      bench_create_system_with_plan::<T>(40_800_000, inert_contract_steps_of_len::<T>(1 + s));
    let actor = Pallet::<T>::sovereign_account_id_system(actor_id);
    let predicate_count = p.min(T::MaxPredicatesPerStep::get());
    let assets = T::BenchmarkHelper::setup_predicate_assets(&actor, predicate_count)
      .expect("Suspended-tail predicate assets exist");
    assert_eq!(assets.len(), predicate_count as usize);
    let mut contract =
      Pallet::<T>::load_actor_contract(actor_id).expect("Suspended-tail Contract exists");
    contract.steps[s as usize].task = ActorTask::Transfer {
      to: actor.clone(),
      asset: T::FeeNativeAssetId::get(),
      amount: AmountResolution::Fixed(One::one()),
    };
    contract.steps[s as usize].on_error = StepErrorPolicy::RetryLater {
      max_attempts: T::MaxRetryAttempts::get(),
    };
    if predicate_count > 0 {
      let predicates = assets
        .into_iter()
        .map(|asset| Predicate::BalanceBelow {
          asset,
          threshold: One::one(),
        })
        .collect::<alloc::vec::Vec<_>>();
      contract.steps[s as usize].precondition = Some(current_any::<T>(predicates));
    }
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("Suspended-tail Contract remains admitted");
    install_run_state::<T>(actor_id, 0);
    ActorRunStateStore::<T>::mutate(actor_id, |maybe| {
      let run = maybe.as_mut().expect("Suspended-tail run exists");
      maximize_run_state_geometry::<T>(run);
      run.cursor = s;
      run.opening_predicate_cursor = 0;
      run.cumulative_outcomes = OutcomeTotals::default();
      run.last_step_outcome = Some(StepOutcome::FundingUnavailable);
      run.last_attempt_block = 1u32.into();
      run.last_committed_step_block = Some(0u32.into());
      run.eligible_at = now;
      run.suspension = Some(SuspensionReason::FundingUnavailable);
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe.as_mut().expect("Suspended-tail hot state exists");
      hot.cycle_state = CycleState::Suspended;
      hot.queue_ticket = Some(9);
      hot.wakeup_pointer = None;
    });
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("Suspended-tail service state is coherent");
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Suspended-tail hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("Suspended-tail Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("Suspended-tail maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("Suspended-tail plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("Suspended-tail inner atom commits");
    }
    let run = ActorRunStateStore::<T>::get(actor_id).expect("Suspended-tail run remains");
    assert_eq!(run.cursor, s);
    assert_eq!(run.unsuccessful_attempts_at_cursor, 2);
    assert_eq!(run.last_attempt_block, now);
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| hot.wakeup_pointer.is_some()));
  }

  /// Measures the direct inner Suspended-tail successful-completion owner.
  /// A fixed-zero Transfer or false current predicates skips without Task effect execution.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_tail_complete(s: Linear<1, 4>, p: Linear<0, 4>) {
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 2u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let actor_id =
      bench_create_system_with_plan::<T>(40_900_000, inert_contract_steps_of_len::<T>(1 + s));
    let actor = Pallet::<T>::sovereign_account_id_system(actor_id);
    let predicate_count = p.min(T::MaxPredicatesPerStep::get());
    let assets = T::BenchmarkHelper::setup_predicate_assets(&actor, predicate_count)
      .expect("Suspended-complete predicate assets exist");
    assert_eq!(assets.len(), predicate_count as usize);
    let mut contract =
      Pallet::<T>::load_actor_contract(actor_id).expect("Suspended-complete Contract exists");
    contract.steps[s as usize].task = ActorTask::Transfer {
      to: actor.clone(),
      asset: T::FeeNativeAssetId::get(),
      amount: AmountResolution::Fixed(Zero::zero()),
    };
    contract.steps[s as usize].on_error = StepErrorPolicy::RetryLater {
      max_attempts: T::MaxRetryAttempts::get(),
    };
    if predicate_count > 0 {
      let predicates = assets
        .into_iter()
        .map(|asset| Predicate::BalanceAbove {
          asset,
          threshold: One::one(),
        })
        .collect::<alloc::vec::Vec<_>>();
      contract.steps[s as usize].precondition = Some(current_any::<T>(predicates));
    }
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("Suspended-complete Contract remains admitted");
    install_run_state::<T>(actor_id, 0);
    ActorRunStateStore::<T>::mutate(actor_id, |maybe| {
      let run = maybe.as_mut().expect("Suspended-complete run exists");
      maximize_run_state_geometry::<T>(run);
      run.cursor = s;
      run.opening_predicate_cursor = 0;
      run.cumulative_outcomes = OutcomeTotals::default();
      run.last_step_outcome = Some(StepOutcome::FundingUnavailable);
      run.last_attempt_block = 1u32.into();
      run.last_committed_step_block = Some(0u32.into());
      run.eligible_at = now;
      run.suspension = Some(SuspensionReason::FundingUnavailable);
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe.as_mut().expect("Suspended-complete hot state exists");
      hot.cycle_state = CycleState::Suspended;
      hot.queue_ticket = Some(9);
      hot.wakeup_pointer = None;
    });
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("Suspended-complete service state is coherent");
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Suspended-complete hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("Suspended-complete Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("Suspended-complete maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("Suspended-complete plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("Suspended-complete inner atom commits");
    }
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
      hot.cycle_state == CycleState::Idle
        && hot.queue_ticket.is_none()
        && hot.wakeup_pointer.is_none()
    }));
  }

  /// Measures the direct inner Suspended-tail successful-progress owner through causal FIFO
  /// successor placement. A fixed-zero Transfer or false current predicates skips without Task
  /// effect execution.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_tail_progress(s: Linear<2, 4>, p: Linear<0, 4>) {
    let _ = QueuePages::<T>::clear(u32::MAX, None);
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(0);
    QueueOccupancy::<T>::put(0);
    NextQueueTicket::<T>::put(0);
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 2u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let actor_id =
      bench_create_system_with_plan::<T>(41_000_000, inert_contract_steps_of_len::<T>(1 + s));
    let actor = Pallet::<T>::sovereign_account_id_system(actor_id);
    let predicate_count = p.min(T::MaxPredicatesPerStep::get());
    let assets = T::BenchmarkHelper::setup_predicate_assets(&actor, predicate_count)
      .expect("Suspended-progress predicate assets exist");
    assert_eq!(assets.len(), predicate_count as usize);
    let mut contract =
      Pallet::<T>::load_actor_contract(actor_id).expect("Suspended-progress Contract exists");
    contract.steps[1].task = ActorTask::Transfer {
      to: actor.clone(),
      asset: T::FeeNativeAssetId::get(),
      amount: AmountResolution::Fixed(Zero::zero()),
    };
    contract.steps[1].on_error = StepErrorPolicy::RetryLater {
      max_attempts: T::MaxRetryAttempts::get(),
    };
    if predicate_count > 0 {
      let predicates = assets
        .into_iter()
        .map(|asset| Predicate::BalanceAbove {
          asset,
          threshold: One::one(),
        })
        .collect::<alloc::vec::Vec<_>>();
      contract.steps[1].precondition = Some(current_any::<T>(predicates));
    }
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("Suspended-progress Contract remains admitted");
    install_run_state::<T>(actor_id, 0);
    ActorRunStateStore::<T>::mutate(actor_id, |maybe| {
      let run = maybe.as_mut().expect("Suspended-progress run exists");
      maximize_run_state_geometry::<T>(run);
      run.cursor = 1;
      run.opening_predicate_cursor = 0;
      run.cumulative_outcomes = OutcomeTotals::default();
      run.last_step_outcome = Some(StepOutcome::FundingUnavailable);
      run.last_attempt_block = 1u32.into();
      run.last_committed_step_block = Some(0u32.into());
      run.eligible_at = now;
      run.suspension = Some(SuspensionReason::FundingUnavailable);
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe.as_mut().expect("Suspended-progress hot state exists");
      hot.cycle_state = CycleState::Suspended;
      hot.queue_ticket = Some(9);
      hot.wakeup_pointer = None;
    });
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("Suspended-progress service state is coherent");
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Suspended-progress hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("Suspended-progress Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("Suspended-progress maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("Suspended-progress plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("Suspended-progress inner atom commits");
    }
    let run = ActorRunStateStore::<T>::get(actor_id).expect("Suspended-progress run remains");
    assert_eq!(run.cursor, 2);
    assert_eq!(run.last_committed_step_block, Some(now));
    assert_eq!(QueueOccupancy::<T>::get(), 1);
    assert!(
      ActorHot::<T>::get(actor_id).is_some_and(|hot| {
        hot.cycle_state == CycleState::Running && hot.queue_ticket.is_some()
      })
    );
  }

  /// Measures the direct inner Suspended-head retry owner across retained immutable payload
  /// geometry and current predicates. An unfunded fixed Transfer yields FundingUnavailable before
  /// Task effect invocation.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_head_retry(
    n: Linear<0, 64>,
    r: Linear<0, 128>,
    f: Linear<0, 40>,
    p: Linear<0, 4>,
  ) {
    let _ = WakeupBuckets::<T>::clear(u32::MAX, None);
    let _ = WakeupPages::<T>::clear(u32::MAX, None);
    let _ = WakeupCursorPages::<T>::clear(u32::MAX, None);
    let _ = WakeupCursorLen::<T>::clear(u32::MAX, None);
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 2u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let actor_id =
      bench_create_system_with_plan::<T>(41_100_000, inert_contract_steps_of_len::<T>(1));
    let actor = Pallet::<T>::sovereign_account_id_system(actor_id);
    let evaluation_units = p.min(T::MaxPredicatesPerStep::get().saturating_mul(2));
    let opening_count = evaluation_units.saturating_sub(T::MaxPredicatesPerStep::get());
    let current_count = evaluation_units.saturating_sub(opening_count.saturating_mul(2));
    let predicate_count = opening_count.saturating_add(current_count);
    let assets = T::BenchmarkHelper::setup_predicate_assets(&actor, predicate_count)
      .expect("Suspended-head predicate assets exist");
    assert_eq!(assets.len(), predicate_count as usize);
    let mut contract =
      Pallet::<T>::load_actor_contract(actor_id).expect("Suspended-head Contract exists");
    contract.steps[0].task = ActorTask::Transfer {
      to: actor.clone(),
      asset: T::FeeNativeAssetId::get(),
      amount: AmountResolution::Fixed(One::one()),
    };
    contract.steps[0].on_error = StepErrorPolicy::RetryLater {
      max_attempts: T::MaxRetryAttempts::get(),
    };
    if predicate_count > 0 {
      let clause = assets
        .into_iter()
        .enumerate()
        .map(|(index, asset)| TimedPredicate {
          timing: if index < opening_count as usize {
            ObservationTiming::Opening
          } else {
            ObservationTiming::Current
          },
          predicate: Predicate::BalanceBelow {
            asset,
            threshold: One::one(),
          },
        })
        .collect::<alloc::vec::Vec<_>>();
      contract.steps[0].precondition = Some(Precondition {
        clauses: BoundedVec::try_from(alloc::vec![
          BoundedVec::try_from(clause).expect("Suspended-head predicates fit"),
        ])
        .expect("Suspended-head clause fits"),
      });
    }
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("Suspended-head Contract remains admitted");
    let snapshot_count = n.min(T::MaxOpeningSnapshotEntries::get());
    let result_count = r
      .min(T::MaxOpeningPredicateResults::get())
      .max(opening_count);
    let funding_count = f.min(T::MaxFundingTrackedAssets::get());
    install_run_state::<T>(actor_id, snapshot_count);
    ActorRunStateStore::<T>::mutate(actor_id, |maybe| {
      let run = maybe.as_mut().expect("Suspended-head run exists");
      for index in 0..result_count {
        run
          .opening_predicate_results
          .try_push(if index < opening_count {
            Ok(true)
          } else if index % 2 == 0 {
            Ok(true)
          } else {
            Err(PredicateError::InvalidObservation)
          })
          .expect("Suspended-head Opening result fits");
      }
      for asset in T::BenchmarkHelper::funding_assets(funding_count) {
        run
          .funding_snapshot
          .try_insert(asset, One::one())
          .expect("Suspended-head funding entry fits");
      }
      assert_eq!(run.opening_snapshot.len() as u32, snapshot_count);
      assert_eq!(run.opening_predicate_results.len() as u32, result_count);
      assert_eq!(run.funding_snapshot.len() as u32, funding_count);
      run.cursor = 0;
      run.opening_predicate_cursor = 0;
      run.cumulative_outcomes = OutcomeTotals::default();
      run.last_step_outcome = Some(StepOutcome::FundingUnavailable);
      run.last_attempt_block = 1u32.into();
      run.last_committed_step_block = None;
      run.eligible_at = now;
      run.suspension = Some(SuspensionReason::FundingUnavailable);
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe.as_mut().expect("Suspended-head hot state exists");
      hot.cycle_state = CycleState::Suspended;
      hot.queue_ticket = Some(9);
      hot.wakeup_pointer = None;
    });
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("Suspended-head service state is coherent");
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Suspended-head hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("Suspended-head Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("Suspended-head maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("Suspended-head plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("Suspended-head inner atom commits");
    }
    let run = ActorRunStateStore::<T>::get(actor_id).expect("Suspended-head run remains");
    assert_eq!(run.cursor, 0);
    assert_eq!(run.unsuccessful_attempts_at_cursor, 2);
    assert_eq!(run.last_attempt_block, now);
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| hot.wakeup_pointer.is_some()));
  }

  /// Measures the direct inner current-predicate Suspended-head completion owner across retained
  /// immutable payload geometry. A fixed-zero Transfer or false current predicates skips without
  /// Task effect execution.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_head_complete(
    n: Linear<0, 64>,
    r: Linear<0, 128>,
    f: Linear<0, 40>,
    p: Linear<0, 4>,
  ) {
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 2u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let actor_id =
      bench_create_system_with_plan::<T>(41_200_000, inert_contract_steps_of_len::<T>(1));
    let actor = Pallet::<T>::sovereign_account_id_system(actor_id);
    let evaluation_units = p.min(T::MaxPredicatesPerStep::get().saturating_mul(2));
    let opening_count = evaluation_units.saturating_sub(T::MaxPredicatesPerStep::get());
    let current_count = evaluation_units.saturating_sub(opening_count.saturating_mul(2));
    let predicate_count = opening_count.saturating_add(current_count);
    let assets = T::BenchmarkHelper::setup_predicate_assets(&actor, predicate_count)
      .expect("Suspended-head completion predicate assets exist");
    assert_eq!(assets.len(), predicate_count as usize);
    let mut contract = Pallet::<T>::load_actor_contract(actor_id)
      .expect("Suspended-head completion Contract exists");
    contract.steps[0].task = ActorTask::Transfer {
      to: actor.clone(),
      asset: T::FeeNativeAssetId::get(),
      amount: AmountResolution::Fixed(Zero::zero()),
    };
    contract.steps[0].on_error = StepErrorPolicy::RetryLater {
      max_attempts: T::MaxRetryAttempts::get(),
    };
    if predicate_count > 0 {
      let clause = assets
        .into_iter()
        .enumerate()
        .map(|(index, asset)| TimedPredicate {
          timing: if index < opening_count as usize {
            ObservationTiming::Opening
          } else {
            ObservationTiming::Current
          },
          predicate: if index + 1 == predicate_count as usize && current_count > 0 {
            Predicate::BalanceAbove {
              asset,
              threshold: One::one(),
            }
          } else {
            Predicate::BalanceBelow {
              asset,
              threshold: One::one(),
            }
          },
        })
        .collect::<alloc::vec::Vec<_>>();
      contract.steps[0].precondition = Some(Precondition {
        clauses: BoundedVec::try_from(alloc::vec![
          BoundedVec::try_from(clause).expect("Suspended-head completion predicates fit"),
        ])
        .expect("Suspended-head completion clause fits"),
      });
    }
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("Suspended-head completion Contract remains admitted");
    let snapshot_count = n.min(T::MaxOpeningSnapshotEntries::get());
    let result_count = r
      .min(T::MaxOpeningPredicateResults::get())
      .max(opening_count);
    let funding_count = f.min(T::MaxFundingTrackedAssets::get());
    install_run_state::<T>(actor_id, snapshot_count);
    ActorRunStateStore::<T>::mutate(actor_id, |maybe| {
      let run = maybe
        .as_mut()
        .expect("Suspended-head completion run exists");
      for index in 0..result_count {
        let frozen = if current_count == 0 && index + 1 == opening_count {
          Ok(false)
        } else if index < opening_count || index % 2 == 0 {
          Ok(true)
        } else {
          Err(PredicateError::InvalidObservation)
        };
        run
          .opening_predicate_results
          .try_push(frozen)
          .expect("Suspended-head completion Opening result fits");
      }
      for asset in T::BenchmarkHelper::funding_assets(funding_count) {
        run
          .funding_snapshot
          .try_insert(asset, One::one())
          .expect("Suspended-head completion funding entry fits");
      }
      assert_eq!(run.opening_snapshot.len() as u32, snapshot_count);
      assert_eq!(run.opening_predicate_results.len() as u32, result_count);
      assert_eq!(run.funding_snapshot.len() as u32, funding_count);
      run.cursor = 0;
      run.opening_predicate_cursor = 0;
      run.cumulative_outcomes = OutcomeTotals::default();
      run.last_step_outcome = Some(StepOutcome::FundingUnavailable);
      run.last_attempt_block = 1u32.into();
      run.last_committed_step_block = None;
      run.eligible_at = now;
      run.suspension = Some(SuspensionReason::FundingUnavailable);
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe
        .as_mut()
        .expect("Suspended-head completion hot state exists");
      hot.cycle_state = CycleState::Suspended;
      hot.queue_ticket = Some(9);
      hot.wakeup_pointer = None;
    });
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("Suspended-head completion service state is coherent");
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Suspended-head completion hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("Suspended-head completion Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("Suspended-head completion maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("Suspended-head completion plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("Suspended-head completion inner atom commits");
    }
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
      hot.cycle_state == CycleState::Idle
        && hot.queue_ticket.is_none()
        && hot.wakeup_pointer.is_none()
    }));
  }

  /// Measures the direct inner current-predicate Suspended-head progress owner across retained
  /// immutable payload geometry and causal FIFO successor placement. A fixed-zero Transfer or
  /// false current predicates skips without Task effect execution.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_head_progress(
    n: Linear<0, 64>,
    r: Linear<0, 128>,
    f: Linear<0, 40>,
    p: Linear<0, 4>,
  ) {
    let _ = QueuePages::<T>::clear(u32::MAX, None);
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(0);
    QueueOccupancy::<T>::put(0);
    NextQueueTicket::<T>::put(0);
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 2u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let actor_id =
      bench_create_system_with_plan::<T>(41_300_000, inert_contract_steps_of_len::<T>(2));
    let actor = Pallet::<T>::sovereign_account_id_system(actor_id);
    let evaluation_units = p.min(T::MaxPredicatesPerStep::get().saturating_mul(2));
    let opening_count = evaluation_units.saturating_sub(T::MaxPredicatesPerStep::get());
    let current_count = evaluation_units.saturating_sub(opening_count.saturating_mul(2));
    let predicate_count = opening_count.saturating_add(current_count);
    let assets = T::BenchmarkHelper::setup_predicate_assets(&actor, predicate_count)
      .expect("Suspended-head progress predicate assets exist");
    assert_eq!(assets.len(), predicate_count as usize);
    let mut contract =
      Pallet::<T>::load_actor_contract(actor_id).expect("Suspended-head progress Contract exists");
    contract.steps[0].task = ActorTask::Transfer {
      to: actor.clone(),
      asset: T::FeeNativeAssetId::get(),
      amount: AmountResolution::Fixed(Zero::zero()),
    };
    contract.steps[0].on_error = StepErrorPolicy::RetryLater {
      max_attempts: T::MaxRetryAttempts::get(),
    };
    if predicate_count > 0 {
      let clause = assets
        .into_iter()
        .enumerate()
        .map(|(index, asset)| TimedPredicate {
          timing: if index < opening_count as usize {
            ObservationTiming::Opening
          } else {
            ObservationTiming::Current
          },
          predicate: if index + 1 == predicate_count as usize && current_count > 0 {
            Predicate::BalanceAbove {
              asset,
              threshold: One::one(),
            }
          } else {
            Predicate::BalanceBelow {
              asset,
              threshold: One::one(),
            }
          },
        })
        .collect::<alloc::vec::Vec<_>>();
      contract.steps[0].precondition = Some(Precondition {
        clauses: BoundedVec::try_from(alloc::vec![
          BoundedVec::try_from(clause).expect("Suspended-head progress predicates fit"),
        ])
        .expect("Suspended-head progress clause fits"),
      });
    }
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("Suspended-head progress Contract remains admitted");
    let snapshot_count = n.min(T::MaxOpeningSnapshotEntries::get());
    let result_count = r
      .min(T::MaxOpeningPredicateResults::get())
      .max(opening_count);
    let funding_count = f.min(T::MaxFundingTrackedAssets::get());
    install_run_state::<T>(actor_id, snapshot_count);
    ActorRunStateStore::<T>::mutate(actor_id, |maybe| {
      let run = maybe.as_mut().expect("Suspended-head progress run exists");
      for index in 0..result_count {
        let frozen = if current_count == 0 && index + 1 == opening_count {
          Ok(false)
        } else if index < opening_count || index % 2 == 0 {
          Ok(true)
        } else {
          Err(PredicateError::InvalidObservation)
        };
        run
          .opening_predicate_results
          .try_push(frozen)
          .expect("Suspended-head progress Opening result fits");
      }
      for asset in T::BenchmarkHelper::funding_assets(funding_count) {
        run
          .funding_snapshot
          .try_insert(asset, One::one())
          .expect("Suspended-head progress funding entry fits");
      }
      assert_eq!(run.opening_snapshot.len() as u32, snapshot_count);
      assert_eq!(run.opening_predicate_results.len() as u32, result_count);
      assert_eq!(run.funding_snapshot.len() as u32, funding_count);
      run.cursor = 0;
      run.opening_predicate_cursor = 0;
      run.cumulative_outcomes = OutcomeTotals::default();
      run.last_step_outcome = Some(StepOutcome::FundingUnavailable);
      run.last_attempt_block = 1u32.into();
      run.last_committed_step_block = None;
      run.eligible_at = now;
      run.suspension = Some(SuspensionReason::FundingUnavailable);
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe
        .as_mut()
        .expect("Suspended-head progress hot state exists");
      hot.cycle_state = CycleState::Suspended;
      hot.queue_ticket = Some(9);
      hot.wakeup_pointer = None;
    });
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("Suspended-head progress service state is coherent");
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Suspended-head progress hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("Suspended-head progress Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("Suspended-head progress maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("Suspended-head progress plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("Suspended-head progress inner atom commits");
    }
    let run = ActorRunStateStore::<T>::get(actor_id).expect("Suspended-head progress run remains");
    assert_eq!(run.cursor, 1);
    assert_eq!(run.last_committed_step_block, Some(now));
    assert_eq!(QueueOccupancy::<T>::get(), 1);
    assert!(
      ActorHot::<T>::get(actor_id).is_some_and(|hot| {
        hot.cycle_state == CycleState::Running && hot.queue_ticket.is_some()
      })
    );
  }

  /// Measures the direct inner Opening-heavy Suspended-head retry owner. The fixed worst
  /// realizable mixed composition is one frozen Opening predicate plus three Current predicates;
  /// all evaluate true before an unfunded Transfer yields FundingUnavailable without Task effect.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_head_opening_retry(
    n: Linear<0, 64>,
    r: Linear<0, 128>,
    f: Linear<0, 40>,
  ) {
    let _ = WakeupBuckets::<T>::clear(u32::MAX, None);
    let _ = WakeupPages::<T>::clear(u32::MAX, None);
    let _ = WakeupCursorPages::<T>::clear(u32::MAX, None);
    let _ = WakeupCursorLen::<T>::clear(u32::MAX, None);
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 2u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let actor_id =
      bench_create_system_with_plan::<T>(41_400_000, inert_contract_steps_of_len::<T>(1));
    let actor = Pallet::<T>::sovereign_account_id_system(actor_id);
    let assets = T::BenchmarkHelper::setup_predicate_assets(&actor, 4)
      .expect("Opening-heavy retry predicate assets exist");
    assert_eq!(assets.len(), 4);
    let clause = assets
      .into_iter()
      .enumerate()
      .map(|(index, asset)| TimedPredicate {
        timing: if index == 0 {
          ObservationTiming::Opening
        } else {
          ObservationTiming::Current
        },
        predicate: Predicate::BalanceBelow {
          asset,
          threshold: One::one(),
        },
      })
      .collect::<alloc::vec::Vec<_>>();
    let precondition = Precondition {
      clauses: BoundedVec::try_from(alloc::vec![
        BoundedVec::try_from(clause).expect("Opening-heavy retry predicates fit"),
      ])
      .expect("Opening-heavy retry clause fits"),
    };
    assert_eq!(precondition.evaluation_units(), 5);
    let mut contract =
      Pallet::<T>::load_actor_contract(actor_id).expect("Opening-heavy retry Contract exists");
    contract.steps[0].precondition = Some(precondition);
    contract.steps[0].task = ActorTask::Transfer {
      to: actor.clone(),
      asset: T::FeeNativeAssetId::get(),
      amount: AmountResolution::Fixed(One::one()),
    };
    contract.steps[0].on_error = StepErrorPolicy::RetryLater {
      max_attempts: T::MaxRetryAttempts::get(),
    };
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("Opening-heavy retry Contract remains admitted");
    let snapshot_count = n.min(T::MaxOpeningSnapshotEntries::get());
    let result_count = r.min(T::MaxOpeningPredicateResults::get()).max(1);
    let funding_count = f.min(T::MaxFundingTrackedAssets::get());
    install_run_state::<T>(actor_id, snapshot_count);
    ActorRunStateStore::<T>::mutate(actor_id, |maybe| {
      let run = maybe.as_mut().expect("Opening-heavy retry run exists");
      for index in 0..result_count {
        run
          .opening_predicate_results
          .try_push(if index == 0 {
            Ok(true)
          } else if index % 2 == 0 {
            Ok(true)
          } else {
            Err(PredicateError::InvalidObservation)
          })
          .expect("Opening-heavy retry result fits");
      }
      for asset in T::BenchmarkHelper::funding_assets(funding_count) {
        run
          .funding_snapshot
          .try_insert(asset, One::one())
          .expect("Opening-heavy retry funding entry fits");
      }
      assert_eq!(run.opening_snapshot.len() as u32, snapshot_count);
      assert_eq!(run.opening_predicate_results.len() as u32, result_count);
      assert_eq!(run.funding_snapshot.len() as u32, funding_count);
      run.cursor = 0;
      run.opening_predicate_cursor = 0;
      run.cumulative_outcomes = OutcomeTotals::default();
      run.last_step_outcome = Some(StepOutcome::FundingUnavailable);
      run.last_attempt_block = 1u32.into();
      run.last_committed_step_block = None;
      run.eligible_at = now;
      run.suspension = Some(SuspensionReason::FundingUnavailable);
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe
        .as_mut()
        .expect("Opening-heavy retry hot state exists");
      hot.cycle_state = CycleState::Suspended;
      hot.queue_ticket = Some(9);
      hot.wakeup_pointer = None;
    });
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("Opening-heavy retry service state is coherent");
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Opening-heavy retry hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("Opening-heavy retry Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("Opening-heavy retry maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("Opening-heavy retry plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("Opening-heavy retry inner atom commits");
    }
    let run = ActorRunStateStore::<T>::get(actor_id).expect("Opening-heavy retry run remains");
    assert_eq!(run.cursor, 0);
    assert_eq!(run.unsuccessful_attempts_at_cursor, 2);
    assert_eq!(run.last_attempt_block, now);
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| hot.wakeup_pointer.is_some()));
  }

  /// Measures the direct inner Opening-heavy Suspended-head completion owner. The fixed worst
  /// mixed composition is one frozen Opening predicate plus three Current predicates; the final
  /// Current predicate is false so completion commits without Task effect execution.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_head_opening_complete(
    n: Linear<0, 64>,
    r: Linear<0, 128>,
    f: Linear<0, 40>,
  ) {
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 2u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let actor_id =
      bench_create_system_with_plan::<T>(41_500_000, inert_contract_steps_of_len::<T>(1));
    let actor = Pallet::<T>::sovereign_account_id_system(actor_id);
    let assets = T::BenchmarkHelper::setup_predicate_assets(&actor, 4)
      .expect("Opening-heavy completion predicate assets exist");
    assert_eq!(assets.len(), 4);
    let clause = assets
      .into_iter()
      .enumerate()
      .map(|(index, asset)| TimedPredicate {
        timing: if index == 0 {
          ObservationTiming::Opening
        } else {
          ObservationTiming::Current
        },
        predicate: if index == 3 {
          Predicate::BalanceAbove {
            asset,
            threshold: One::one(),
          }
        } else {
          Predicate::BalanceBelow {
            asset,
            threshold: One::one(),
          }
        },
      })
      .collect::<alloc::vec::Vec<_>>();
    let precondition = Precondition {
      clauses: BoundedVec::try_from(alloc::vec![
        BoundedVec::try_from(clause).expect("Opening-heavy completion predicates fit"),
      ])
      .expect("Opening-heavy completion clause fits"),
    };
    assert_eq!(precondition.evaluation_units(), 5);
    let mut contract =
      Pallet::<T>::load_actor_contract(actor_id).expect("Opening-heavy completion Contract exists");
    contract.steps[0].precondition = Some(precondition);
    contract.steps[0].task = ActorTask::Transfer {
      to: actor.clone(),
      asset: T::FeeNativeAssetId::get(),
      amount: AmountResolution::Fixed(Zero::zero()),
    };
    contract.steps[0].on_error = StepErrorPolicy::RetryLater {
      max_attempts: T::MaxRetryAttempts::get(),
    };
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("Opening-heavy completion Contract remains admitted");
    let snapshot_count = n.min(T::MaxOpeningSnapshotEntries::get());
    let result_count = r.min(T::MaxOpeningPredicateResults::get()).max(1);
    let funding_count = f.min(T::MaxFundingTrackedAssets::get());
    install_run_state::<T>(actor_id, snapshot_count);
    ActorRunStateStore::<T>::mutate(actor_id, |maybe| {
      let run = maybe.as_mut().expect("Opening-heavy completion run exists");
      for index in 0..result_count {
        run
          .opening_predicate_results
          .try_push(if index == 0 {
            Ok(true)
          } else if index % 2 == 0 {
            Ok(true)
          } else {
            Err(PredicateError::InvalidObservation)
          })
          .expect("Opening-heavy completion result fits");
      }
      for asset in T::BenchmarkHelper::funding_assets(funding_count) {
        run
          .funding_snapshot
          .try_insert(asset, One::one())
          .expect("Opening-heavy completion funding entry fits");
      }
      assert_eq!(run.opening_snapshot.len() as u32, snapshot_count);
      assert_eq!(run.opening_predicate_results.len() as u32, result_count);
      assert_eq!(run.funding_snapshot.len() as u32, funding_count);
      run.cursor = 0;
      run.opening_predicate_cursor = 0;
      run.cumulative_outcomes = OutcomeTotals::default();
      run.last_step_outcome = Some(StepOutcome::FundingUnavailable);
      run.last_attempt_block = 1u32.into();
      run.last_committed_step_block = None;
      run.eligible_at = now;
      run.suspension = Some(SuspensionReason::FundingUnavailable);
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe
        .as_mut()
        .expect("Opening-heavy completion hot state exists");
      hot.cycle_state = CycleState::Suspended;
      hot.queue_ticket = Some(9);
      hot.wakeup_pointer = None;
    });
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("Opening-heavy completion service state is coherent");
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Opening-heavy completion hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("Opening-heavy completion Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("Opening-heavy completion maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("Opening-heavy completion plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("Opening-heavy completion inner atom commits");
    }
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
      hot.cycle_state == CycleState::Idle
        && hot.queue_ticket.is_none()
        && hot.wakeup_pointer.is_none()
    }));
  }

  /// Measures the direct inner Opening-heavy Suspended-head progress owner. The fixed worst mixed
  /// composition is one frozen Opening predicate plus three Current predicates; the final Current
  /// predicate is false before causal FIFO successor placement, without Task effect execution.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_inner_suspended_head_opening_progress(
    n: Linear<0, 64>,
    r: Linear<0, 128>,
    f: Linear<0, 40>,
  ) {
    let _ = QueuePages::<T>::clear(u32::MAX, None);
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(0);
    QueueOccupancy::<T>::put(0);
    NextQueueTicket::<T>::put(0);
    GlobalCircuitBreaker::<T>::put(false);
    let now: BlockNumberFor<T> = 2u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let actor_id =
      bench_create_system_with_plan::<T>(41_600_000, inert_contract_steps_of_len::<T>(2));
    let actor = Pallet::<T>::sovereign_account_id_system(actor_id);
    let assets = T::BenchmarkHelper::setup_predicate_assets(&actor, 4)
      .expect("Opening-heavy progress predicate assets exist");
    assert_eq!(assets.len(), 4);
    let clause = assets
      .into_iter()
      .enumerate()
      .map(|(index, asset)| TimedPredicate {
        timing: if index == 0 {
          ObservationTiming::Opening
        } else {
          ObservationTiming::Current
        },
        predicate: if index == 3 {
          Predicate::BalanceAbove {
            asset,
            threshold: One::one(),
          }
        } else {
          Predicate::BalanceBelow {
            asset,
            threshold: One::one(),
          }
        },
      })
      .collect::<alloc::vec::Vec<_>>();
    let precondition = Precondition {
      clauses: BoundedVec::try_from(alloc::vec![
        BoundedVec::try_from(clause).expect("Opening-heavy progress predicates fit"),
      ])
      .expect("Opening-heavy progress clause fits"),
    };
    assert_eq!(precondition.evaluation_units(), 5);
    let mut contract =
      Pallet::<T>::load_actor_contract(actor_id).expect("Opening-heavy progress Contract exists");
    contract.steps[0].precondition = Some(precondition);
    contract.steps[0].task = ActorTask::Transfer {
      to: actor.clone(),
      asset: T::FeeNativeAssetId::get(),
      amount: AmountResolution::Fixed(Zero::zero()),
    };
    contract.steps[0].on_error = StepErrorPolicy::RetryLater {
      max_attempts: T::MaxRetryAttempts::get(),
    };
    Pallet::<T>::store_actor_contract(actor_id, contract)
      .expect("Opening-heavy progress Contract remains admitted");
    let snapshot_count = n.min(T::MaxOpeningSnapshotEntries::get());
    let result_count = r.min(T::MaxOpeningPredicateResults::get()).max(1);
    let funding_count = f.min(T::MaxFundingTrackedAssets::get());
    install_run_state::<T>(actor_id, snapshot_count);
    ActorRunStateStore::<T>::mutate(actor_id, |maybe| {
      let run = maybe.as_mut().expect("Opening-heavy progress run exists");
      for index in 0..result_count {
        run
          .opening_predicate_results
          .try_push(if index == 0 {
            Ok(true)
          } else if index % 2 == 0 {
            Ok(true)
          } else {
            Err(PredicateError::InvalidObservation)
          })
          .expect("Opening-heavy progress result fits");
      }
      for asset in T::BenchmarkHelper::funding_assets(funding_count) {
        run
          .funding_snapshot
          .try_insert(asset, One::one())
          .expect("Opening-heavy progress funding entry fits");
      }
      assert_eq!(run.opening_snapshot.len() as u32, snapshot_count);
      assert_eq!(run.opening_predicate_results.len() as u32, result_count);
      assert_eq!(run.funding_snapshot.len() as u32, funding_count);
      run.cursor = 0;
      run.opening_predicate_cursor = 0;
      run.cumulative_outcomes = OutcomeTotals::default();
      run.last_step_outcome = Some(StepOutcome::FundingUnavailable);
      run.last_attempt_block = 1u32.into();
      run.last_committed_step_block = None;
      run.eligible_at = now;
      run.suspension = Some(SuspensionReason::FundingUnavailable);
    });
    ActorHot::<T>::mutate(actor_id, |maybe| {
      let hot = maybe
        .as_mut()
        .expect("Opening-heavy progress hot state exists");
      hot.cycle_state = CycleState::Suspended;
      hot.queue_ticket = Some(9);
      hot.wakeup_pointer = None;
    });
    let (state, admission, loaded_step) = Pallet::<T>::load_current_step_service_state(actor_id)
      .expect("Opening-heavy progress service state is coherent");
    let execution_instance = Pallet::<T>::derive_active_actor_view(
      state.identity.clone(),
      state.hot.clone(),
      state.contract.clone(),
    );
    ActorHot::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Opening-heavy progress hot state remains")
        .queue_ticket = None;
    });
    #[block]
    {
      let ticket = Pallet::<T>::build_actor_step_ticket(
        actor_id,
        9,
        now,
        &state.identity,
        &state.hot,
        state.run_state.as_ref(),
        &admission,
      )
      .expect("Opening-heavy progress Step ticket builds");
      let maximum_fee = Pallet::<T>::maximum_current_step_fee(
        state.identity.actor_class.actor_type(),
        loaded_step.resources,
      )
      .expect("Opening-heavy progress maximum fee exists");
      let plan = Pallet::<T>::build_current_step_plan(
        actor_id,
        state.identity,
        state.hot,
        state.run_state,
        state.funding,
        admission,
        ticket,
        loaded_step,
        maximum_fee,
      )
      .expect("Opening-heavy progress plan builds");
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        match Pallet::<T>::execute_current_step_and_place(actor_id, execution_instance, plan, now) {
          Ok(evidence) => {
            core::hint::black_box(evidence);
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok::<
              (),
              AttemptTransactionError,
            >(()))
          }
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      })
      .expect("Opening-heavy progress inner atom commits");
    }
    let run = ActorRunStateStore::<T>::get(actor_id).expect("Opening-heavy progress run remains");
    assert_eq!(run.cursor, 1);
    assert_eq!(run.last_committed_step_block, Some(now));
    assert_eq!(QueueOccupancy::<T>::get(), 1);
    assert!(
      ActorHot::<T>::get(actor_id).is_some_and(|hot| {
        hot.cycle_state == CycleState::Running && hot.queue_ticket.is_some()
      })
    );
  }

  /// Measures actual scheduler admission and complete execution for up to 1,000
  /// minimal one-step System actors. `Weight::MAX` exposes the full production-Wasm
  /// cost curve; separate guaranteed-budget stress evidence determines how many
  /// executions the reference block budget actually admits. Setup writes the split actor stores and canonical paged FIFO outside
  /// the measured block so the result isolates queue scanning, admission,
  /// execution, and consumption rather than actor creation.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_execute_cheap(n: Linear<1, 1_000>) {
    let _ = QueuePages::<T>::clear(u32::MAX, None);
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(0);
    QueueOccupancy::<T>::put(0);
    NextQueueTicket::<T>::put(0);
    GlobalCircuitBreaker::<T>::put(false);
    let bounded = n
      .min(T::MaxExecutionsPerBlock::get())
      .min(T::MaxQueueEntriesScannedPerBlock::get())
      .min(T::MaxQueueLength::get());
    assert!(bounded > 0, "runtime limits must admit at least one sample");
    let template_id = bench_create_system_manual::<T>(40_000_000);
    let hot_template = ActorHot::<T>::get(template_id).expect("benchmark hot template");
    let identity_template =
      ActorIdentities::<T>::get(template_id).expect("benchmark identity template");
    let contract_template =
      Pallet::<T>::load_actor_contract(template_id).expect("benchmark contract template");
    let funding_template = ActorFunding::<T>::get(template_id).expect("benchmark funding template");
    ActorIdentities::<T>::remove(template_id);
    ActorHot::<T>::remove(template_id);
    Pallet::<T>::remove_actor_contract(template_id).expect("benchmark template Contract removes");
    ActorFunding::<T>::remove(template_id);

    let first_id = 41_000_000u64;
    for offset in 0..bounded {
      let actor_id = first_id.saturating_add(u64::from(offset));
      let mut hot = hot_template.clone();
      let mut identity = identity_template.clone();
      identity.cycle_nonce = 0;
      hot.last_cycle_block = None;
      hot.pending_signal = true;
      hot.queue_ticket = None;
      ActorIdentities::<T>::insert(actor_id, identity);
      ActorHot::<T>::insert(actor_id, hot);
      Pallet::<T>::store_actor_contract(actor_id, contract_template.clone())
        .expect("benchmark queued Contract remains admitted");
      ActorFunding::<T>::insert(actor_id, funding_template.clone());
      assert!(Pallet::<T>::paged_enqueue(actor_id));
    }
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    #[block]
    {
      core::hint::black_box(Pallet::<T>::execute_cycle(Weight::MAX));
    }
    let executed = (0..bounded)
      .filter(|offset| {
        let actor_id = first_id.saturating_add(u64::from(*offset));
        ActorIdentities::<T>::get(actor_id).is_some_and(|identity| identity.cycle_nonce == 1)
      })
      .count() as u32;
    assert_eq!(
      executed, bounded,
      "unbounded diagnostic budget completed only {executed} of {bounded} requested cheap actors"
    );
    assert_eq!(QueueHead::<T>::get(), QueueTail::<T>::get());
  }

  /// Measures canonical FIFO execution over alternating System/User actors.
  /// Setup materializes one ticket-ordered queue outside the measured block.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_execute_cheap_mixed(n: Linear<2, 1_000>) {
    let _ = QueuePages::<T>::clear(u32::MAX, None);
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(0);
    QueueOccupancy::<T>::put(0);
    NextQueueTicket::<T>::put(0);
    GlobalCircuitBreaker::<T>::put(false);
    let bounded = n
      .min(T::MaxExecutionsPerBlock::get())
      .min(T::MaxQueueEntriesScannedPerBlock::get())
      .min(T::MaxQueueLength::get());
    assert!(
      bounded >= 2,
      "runtime limits must admit alternating actor classes"
    );

    let system_template_id = bench_create_system_manual::<T>(42_000_000);
    let system_identity =
      ActorIdentities::<T>::take(system_template_id).expect("System identity template");
    let system_hot = ActorHot::<T>::take(system_template_id).expect("System hot template");
    let contract_template =
      Pallet::<T>::load_actor_contract(system_template_id).expect("System contract template");
    Pallet::<T>::remove_actor_contract(system_template_id)
      .expect("System template Contract removes");
    let funding_template =
      ActorFunding::<T>::take(system_template_id).expect("System funding template");
    let first_id = 43_000_000u64;
    for offset in 0..bounded {
      let actor_id = first_id.saturating_add(u64::from(offset));
      let is_user = offset % 2 != 0;
      let mut identity = if is_user {
        let owner: T::AccountId = account("mixed_user_owner", offset, 0);
        let template_id = bench_create_user::<T>(owner);
        let identity = ActorIdentities::<T>::take(template_id).expect("User identity template");
        ActorHot::<T>::remove(template_id);
        Pallet::<T>::remove_actor_contract(template_id).expect("User template Contract removes");
        ActorFunding::<T>::remove(template_id);
        identity
      } else {
        system_identity.clone()
      };
      identity.cycle_nonce = 0;
      let mut hot = system_hot.clone();
      hot.last_cycle_block = None;
      hot.pending_signal = true;
      hot.queue_ticket = None;
      ActorIdentities::<T>::insert(actor_id, identity);
      ActorHot::<T>::insert(actor_id, hot);
      Pallet::<T>::store_actor_contract(actor_id, contract_template.clone())
        .expect("benchmark mixed-class Contract remains admitted");
      ActorFunding::<T>::insert(actor_id, funding_template.clone());
      assert!(Pallet::<T>::paged_enqueue(actor_id));
    }
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    #[block]
    {
      core::hint::black_box(Pallet::<T>::execute_cycle(Weight::MAX));
    }
    let executed = (0..bounded)
      .filter(|offset| {
        let actor_id = first_id.saturating_add(u64::from(*offset));
        ActorIdentities::<T>::get(actor_id).is_some_and(|identity| identity.cycle_nonce == 1)
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
    let consumed = (0..bounded)
      .filter(|offset| {
        let actor_id = first_id.saturating_add(u64::from(*offset));
        ActorHot::<T>::get(actor_id).is_some_and(|hot| hot.queue_ticket.is_none())
      })
      .count() as u32;
    assert_eq!(
      consumed, executed,
      "every executed cohort actor must release its queue ticket"
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn run_progress() {
    let actor_id =
      bench_create_system_with_plan::<T>(49_999_999, inert_contract_steps_of_len::<T>(2));
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    let mut maximum = ActorRunStateStore::<T>::take(actor_id).expect("benchmark Actor run exists");
    maximize_run_state_geometry::<T>(&mut maximum);
    ActorRunStateStore::<T>::insert(actor_id, maximum);
    let mut state = ActorRunStateStore::<T>::get(actor_id).expect("maximum Actor run exists");
    state.cursor = 1;
    state.unsuccessful_attempts_at_cursor = 0;
    state.last_attempt_block = 1u32.into();
    state.last_committed_step_block = Some(1u32.into());
    state.eligible_at = 2u32.into();
    state.suspension = None;
    #[block]
    {
      Pallet::<T>::persist_run_progress(actor_id, state)
        .expect("benchmark Running progress must persist");
    }
    assert!(ActorRunStateStore::<T>::contains_key(actor_id));
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| hot.cycle_state == CycleState::Running));
  }

  #[benchmark(pov_mode = Measured)]
  fn run_suspend() {
    let actor_id = bench_create_system_manual::<T>(50_000_000);
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    let mut maximum = ActorRunStateStore::<T>::take(actor_id).expect("benchmark Actor run exists");
    maximize_run_state_geometry::<T>(&mut maximum);
    ActorRunStateStore::<T>::insert(actor_id, maximum);
    let state = ActorRunStateStore::<T>::get(actor_id).expect("maximum Actor run exists");
    #[block]
    {
      Pallet::<T>::persist_run_suspension(actor_id, state)
        .expect("benchmark suspension must persist");
    }
    assert!(ActorRunStateStore::<T>::contains_key(actor_id));
  }

  #[benchmark(pov_mode = Measured)]
  fn run_retry() {
    let actor_id = bench_create_system_manual::<T>(50_000_001);
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    #[block]
    {
      core::hint::black_box(Pallet::<T>::begin_run_attempt(actor_id, 2u32.into()));
    }
    assert_eq!(
      ActorRunStateStore::<T>::get(actor_id)
        .expect("benchmark Actor run remains")
        .last_attempt_block,
      2u32.into()
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn run_complete() {
    let actor_id = bench_create_system_manual::<T>(50_000_002);
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    #[block]
    {
      Pallet::<T>::write_run_state(actor_id, None)
        .expect("benchmark completion must clear Actor run");
    }
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| hot.cycle_state == CycleState::Idle));
  }

  #[benchmark]
  fn run_cancel() {
    let page_size = T::WakeupPageSize::get();
    let wakeup_block = 100u32.into();
    for i in 0..page_size {
      let filler = bench_create_system_manual::<T>(50_000_003u32.saturating_add(i));
      assert!(Pallet::<T>::wakeup_substrate_schedule(filler, wakeup_block));
    }
    let mut middle_fillers = alloc::vec::Vec::with_capacity(page_size.saturating_sub(1) as usize);
    for i in 0..page_size.saturating_sub(1) {
      let filler = bench_create_system_manual::<T>(51_000_003u32.saturating_add(i));
      assert!(Pallet::<T>::wakeup_substrate_schedule(filler, wakeup_block));
      middle_fillers.push(filler);
    }
    let actor_id = bench_create_system_manual::<T>(52_000_003);
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    assert!(Pallet::<T>::wakeup_substrate_schedule(
      actor_id,
      wakeup_block
    ));
    let tail_filler = bench_create_system_manual::<T>(53_000_003);
    assert!(Pallet::<T>::wakeup_substrate_schedule(
      tail_filler,
      wakeup_block
    ));
    for filler in middle_fillers {
      assert!(Pallet::<T>::wakeup_substrate_invalidate(filler).is_some());
    }
    ActorHot::<T>::mutate(actor_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("benchmark actor hot state exists")
        .pending_signal = true;
    });
    #[extrinsic_call]
    cancel_run(RawOrigin::Root, actor_id);
    assert!(ActorRunStateStore::<T>::get(actor_id).is_none());
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
      hot.cycle_state == CycleState::Idle && hot.pending_signal && hot.queue_ticket.is_some()
    }));
    assert!(!WakeupPages::<T>::contains_key((
      WakeupKey::Block(wakeup_block),
      1,
    )));
  }

  #[benchmark]
  fn run_suffix_admission(n: Linear<1, 10>) {
    let bounded = n.min(T::MaxContractSteps::get());
    let recipient: T::AccountId = account("run_suffix_recipient", 0, 0);
    let mut plan = ContractSteps::<T>::default();
    for _ in 0..bounded {
      plan
        .try_push(Step {
          precondition: None,
          task: ActorTask::Transfer {
            to: recipient.clone(),
            asset: T::FeeNativeAssetId::get(),
            amount: AmountResolution::Fixed(One::one()),
          },
          on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
        })
        .expect("benchmark suffix step fits");
    }
    #[block]
    {
      let mut total = Weight::zero();
      for step_index in 0..plan.len() {
        let step = &plan[step_index];
        total = total
          .saturating_add(Pallet::<T>::weight_upper_bound(&step.task))
          .saturating_add(Weight::from_parts(
            step
              .precondition
              .as_ref()
              .map_or(0, Precondition::predicate_count) as u64,
            0,
          ));
      }
      core::hint::black_box(total);
    }
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
    let hot = ActorHot::<T>::get(actor_id).expect("ObservationChange Actor remains active");
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
      assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| hot.pending_signal));
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
      ActorHot::<T>::mutate(actor_id, |maybe| {
        maybe
          .as_mut()
          .expect("observation wakeup actor")
          .last_cycle_block = Some(One::one());
      });
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
      assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
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
        ActorHot::<T>::get(actor_id)
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
    assert!(ActorHot::<T>::get(actor_id).is_none());
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
      assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
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
    let hot = ActorHot::<T>::get(actor_id).expect("Crossing Actor remains active");
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
    assert_eq!(QueueOccupancy::<T>::get(), 31);
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
      let preflight = Pallet::<T>::preflight_crossing_cohort(&snapshot, transition, true, None)
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
        true,
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
    NextQueueTicket::<T>::put(u64::MAX);
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
        true,
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
      ActorHot::<T>::mutate(actor_id, |maybe_hot| {
        let hot = maybe_hot.as_mut().expect("Crossing cohort Actor");
        let TriggerRuntimeState::ObservationCrossing {
          installed_at_revision,
          ..
        } = &mut hot.trigger_runtime_state
        else {
          panic!("Crossing runtime state")
        };
        *installed_at_revision = 2;
      });
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
        true,
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
        true,
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
      ActorHot::<T>::mutate(actor_id, |maybe_hot| {
        let hot = maybe_hot.as_mut().expect("pair Actor hot state must exist");
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
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| hot.pending_signal));
  }

  #[benchmark]
  fn crossing_page_unit() {
    let (_, actor_id) = prepare_crossing_work::<T>(2);
    #[block]
    {
      Pallet::<T>::crossing_work_unit().expect("matched Crossing page work must succeed");
    }
    assert!(matches!(
      ActorHot::<T>::get(actor_id).map(|hot| hot.trigger_runtime_state),
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
      ActorHot::<T>::get(actor_id).map(|hot| hot.trigger_runtime_state),
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
        ActorHot::<T>::get(actor_id).map(|hot| hot.trigger_runtime_state),
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
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
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
      assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
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
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
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
      assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
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
      assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
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
        ActorHot::<T>::get(actor_id)
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
        ActorHot::<T>::get(actor_id)
          .is_some_and(|hot| { hot.pending_signal && hot.queue_ticket.is_some() })
      );
    }
  }

  #[benchmark]
  fn crossing_skip_unit() {
    let (_, actor_id) = prepare_crossing_work::<T>(2);
    ActorHot::<T>::mutate(actor_id, |maybe_hot| {
      let hot = maybe_hot
        .as_mut()
        .expect("benchmark actor hot state must exist");
      let TriggerRuntimeState::ObservationCrossing {
        installed_at_revision,
        ..
      } = &mut hot.trigger_runtime_state
      else {
        panic!("benchmark actor must use Crossing state");
      };
      *installed_at_revision = 2;
    });
    #[block]
    {
      Pallet::<T>::crossing_work_unit().expect("post-installation Crossing skip must succeed");
    }
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
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
      ActorHot::<T>::mutate(actor_id, |maybe_hot| {
        let hot = maybe_hot.as_mut().expect("pair Actor hot state must exist");
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
    #[block]
    {
      Pallet::<T>::crossing_pair_work_unit().expect("Crossing skip pair must succeed");
    }
    for actor_id in [first_actor, second_actor] {
      assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
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
    NextQueueTicket::<T>::put(u64::MAX);
    #[block]
    {
      Pallet::<T>::crossing_work_unit()
        .expect("matched Crossing actor terminal cleanup must succeed");
    }
    assert!(!ActorHot::<T>::contains_key(actor_id));
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
  fn transaction_extension_ingress_notify() {
    let source: T::AccountId = account("ingress_source", 0, 0);
    let (actor_id, recipient) = prepare_saturated_address_actor::<T>(0, Some(source.clone()));
    install_run_state::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
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
      ActorHot::<T>::get(actor_id)
        .is_some_and(|hot| { hot.pending_signal && hot.wakeup_pointer.is_some() })
    );
  }

  #[benchmark]
  fn funding_snapshot_open(a: Linear<1, 10>) {
    let owner: T::AccountId = whitelisted_caller();
    let actor_id = bench_create_user::<T>(owner);
    let assets = T::BenchmarkHelper::funding_assets(a);
    ActorFunding::<T>::mutate(actor_id, |maybe| {
      let funding = maybe.as_mut().expect("benchmark actor funding exists");
      for asset in assets {
        funding
          .funding_accumulated
          .try_insert(asset, 2u32.into())
          .expect("snapshot-open benchmark bound fits");
      }
    });
    let snapshot;
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
  }

  /// Builds a circular chain of `n` system Actors where each transfers 1% of its
  /// native balance to the next actor in the ring, then runs 3 blocks and asserts zero drift.
  pub(super) fn setup_and_run_circular_chain<T: Config>(
    requested_n: u32,
  ) -> alloc::vec::Vec<T::AccountId> {
    let existing_active = ActorHot::<T>::iter_keys().count() as u32;
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

  #[cfg(test)]
  use crate::mock::{Test, new_test_ext};
  #[cfg(test)]
  impl_benchmark_test_suite!(Pallet, new_test_ext(), Test, extra = false);
}
