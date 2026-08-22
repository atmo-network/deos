#![cfg(feature = "runtime-benchmarks")]

extern crate alloc;

use crate::types::Task as ActorTask;
use crate::*;
use alloc::vec;
use frame::prelude::*;
use polkadot_sdk::frame_benchmarking::{account, v2::*};
use polkadot_sdk::frame_support::traits::Hooks;
use polkadot_sdk::frame_system::RawOrigin;
use polkadot_sdk::sp_runtime::Perbill;

#[benchmarks]
mod benches {
  use super::*;

  #[derive(Clone)]
  struct Schedule<Trigger> {
    trigger: Trigger,
    cooldown_blocks: u32,
  }

  type ScheduleOf<T> = Schedule<TriggerOf<T>>;

  fn ensure_creation_balance<T: Config>(owner: &T::AccountId) {
    let creation_fee = T::ActorCreationFee::get();
    if creation_fee.is_zero() {
      return;
    }
    let amount = creation_fee.saturating_add(One::one());
    let _ = T::AssetOps::mint(owner, T::FeeNativeAssetId::get(), amount);
  }

  fn prefund_user_sovereign<T: Config>(
    owner: &T::AccountId,
    slot: u8,
    contract_steps: &ContractSteps<T>,
  ) {
    let envelope = Pallet::<T>::attempt_fee_envelope(ActorType::User, contract_steps, 0)
      .expect("benchmark execution plan has a checked fee envelope");
    let required = T::MinUserBalance::get().saturating_add(envelope.total);
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

  fn deplete_user_sovereign<T: Config>(actor_id: ActorId) {
    let instance =
      Pallet::<T>::active_actor_view(actor_id).expect("benchmark actor must exist for depletion");
    let requirement = Pallet::<T>::attempt_fee_envelope(ActorType::User, &instance.steps, 0)
      .expect("benchmark execution plan has a checked fee envelope");
    let required = T::MinUserBalance::get().saturating_add(requirement.total);
    T::AssetOps::burn(
      &instance.sovereign_account,
      T::FeeNativeAssetId::get(),
      required,
    )
    .expect("benchmark depletion must not overdraw the sovereign");
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
    ensure_creation_balance::<T>(&caller);
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    let contract_steps = make_contract_steps::<T>(recipient);
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
    let contract_steps = make_contract_steps::<T>(recipient);
    prefund_user_sovereign::<T>(&caller, expected_slot, &contract_steps);
    // Pool layout: [slot donor, measured].
    let feeds = observation_feed_pool::<T>(2);
    seed_recycled_observation_slot::<T>(feeds[0]);
    let schedule = Schedule {
      trigger: observation_trigger::<T>(feeds[1]),
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
    assert_eq!(
      ActorObservationFeeds::<T>::get(actor_id).map(|feeds| feeds.len() as u32),
      Some(1),
      "measured create must install its one observation subscription"
    );
  }

  #[benchmark]
  fn create_user_actor_at_slot() {
    let caller: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&caller);
    let requested_slot = T::MaxOwnerSlots::get().saturating_sub(1);
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    let contract_steps = make_contract_steps::<T>(recipient);
    prefund_user_sovereign::<T>(&caller, requested_slot, &contract_steps);
    let schedule = Schedule {
      trigger: Trigger::manual(),
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
  }

  #[benchmark]
  fn create_system_actor() {
    let owner: T::AccountId = whitelisted_caller();
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    let contract_steps = make_contract_steps::<T>(recipient);
    let schedule = Schedule {
      trigger: Trigger::manual(),
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
  }

  #[benchmark]
  fn create_system_actor_at_sovereign_id() {
    let owner: T::AccountId = whitelisted_caller();
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    let contract_steps = make_contract_steps::<T>(recipient.clone());
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
    #[extrinsic_call]
    create_system_actor_at_sovereign_id(
      RawOrigin::Root,
      actor_id,
      owner,
      Mutability::Mutable,
      system_contract::<T>(schedule, contract_steps),
    );
    assert!(Pallet::<T>::active_actor_exists(fresh_id));
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
    // Pool layout: [slot donor, measured].
    let feeds = observation_feed_pool::<T>(2);
    seed_recycled_observation_slot::<T>(feeds[0]);
    let contract = system_contract::<T>(
      Schedule {
        trigger: observation_trigger::<T>(feeds[1]),
        cooldown_blocks: 100,
      },
      make_contract_steps::<T>(recipient),
    );
    #[extrinsic_call]
    activate_actor(
      RawOrigin::Signed(owner),
      actor_id,
      contract.expect("benchmark active contract"),
    );
    assert!(Pallet::<T>::active_actor_exists(actor_id));
    assert!(ActorIdentities::<T>::contains_key(actor_id));
    assert_eq!(
      ActorObservationFeeds::<T>::get(actor_id).map(|feeds| feeds.len() as u32),
      Some(1),
      "measured activation must install its one observation subscription"
    );
  }

  #[benchmark]
  fn deactivate_actor() {
    let owner: T::AccountId = whitelisted_caller();
    let recipient: T::AccountId = account("deactivate-recipient", 0, 0);
    let contract_steps = make_contract_steps::<T>(recipient);
    Pallet::<T>::create_system_actor(
      RawOrigin::Root.into(),
      owner.clone(),
      Mutability::Mutable,
      system_contract::<T>(
        Schedule {
          trigger: Trigger::manual(),
          cooldown_blocks: 100,
        },
        contract_steps,
      ),
    )
    .expect("System Actors creation must succeed");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    install_continuation::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    #[extrinsic_call]
    deactivate_actor(RawOrigin::Signed(owner), actor_id);
    assert!(!Pallet::<T>::active_actor_exists(actor_id));
    assert!(ActorIdentities::<T>::contains_key(actor_id));
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

  #[benchmark]
  fn close_actor() {
    let owner: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&owner);
    let owner_slot = prefill_owner_slots_for_worst_case::<T>(&owner);
    let recipient: T::AccountId = account("close-recipient", 0, 0);
    // Pool layout: [guard_low, measured, guard_high].
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
    .expect("create_user_actor_at_slot must succeed in close_actor benchmark setup");
    let actor_id = NextActorId::<T>::get().saturating_sub(1);
    install_continuation::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    mark_observation_chain_dirty::<T>(&feeds);
    #[extrinsic_call]
    close_actor(RawOrigin::Signed(owner), actor_id);
    assert!(!Pallet::<T>::active_actor_exists(actor_id));
    assert!(ActorObservationFeeds::<T>::get(actor_id).is_none());
    assert_eq!(
      DirtyObservationListState::<T>::get().count,
      2,
      "only the two guard feeds remain dirty after the measured close"
    );
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
  fn update_contract() {
    let caller: T::AccountId = whitelisted_caller();
    // Worst case replaces one dirty middle-node subscription with a disjoint feed.
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
    install_continuation::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
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

  #[benchmark]
  fn set_global_circuit_breaker() {
    #[extrinsic_call]
    set_global_circuit_breaker(RawOrigin::Root, true);
    assert!(GlobalCircuitBreaker::<T>::get());
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
      // Restore the zombie fixture state: swept actors must be balance-exhausted so the
      // sweep closes them, keeping the prefunding admission honest but the postcondition real.
      deplete_user_sovereign::<T>(actor_id);
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

  fn bench_create_system_observation<T: Config>(
    owner: T::AccountId,
    feed: T::ObservationFeedId,
  ) -> ActorId {
    let schedule = Schedule {
      trigger: Trigger::observation_change(feed),
      cooldown_blocks: 0,
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

  fn install_continuation<T: Config>(actor_id: ActorId, snapshot_entries: u32) {
    let bounded = snapshot_entries.min(T::MaxOpeningSnapshotEntries::get());
    let asset_count = bounded.saturating_add(1) / 2;
    let assets = T::BenchmarkHelper::funding_assets(asset_count);
    let mut opening_snapshot = ContinuationSnapshotOf::<T>::default();
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
    ActorContracts::<T>::mutate(actor_id, |maybe_contract| {
      maybe_contract
        .as_mut()
        .expect("benchmark actor contract exists")
        .steps[0]
        .on_error = StepErrorPolicy::RetryLater {
        max_attempts: T::MaxRetryAttempts::get(),
      };
    });
    ActorHot::<T>::mutate(actor_id, |maybe_hot| {
      let hot = maybe_hot
        .as_mut()
        .expect("benchmark actor hot state exists");
      hot.cycle_state = CycleState::Suspended;
      hot.pending_signal = false;
      hot.queue_ticket = None;
      hot.wakeup_pointer = None;
    });
    ActorIdentities::<T>::mutate(actor_id, |maybe_identity| {
      maybe_identity
        .as_mut()
        .expect("benchmark actor identity exists")
        .cycle_nonce = 1;
    });
    ContinuationStateStore::<T>::insert(
      actor_id,
      ContinuationState {
        cursor: 0,
        unsuccessful_attempts_at_cursor: 1,
        last_attempt_block: 1u32.into(),
        opening_snapshot,
        opening_predicate_results: Default::default(),
        funding_snapshot: Default::default(),
        cumulative_outcomes: OutcomeTotals::default(),
      },
    );
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
    let actor_ids = ActorHot::<T>::iter()
      .filter_map(|(actor_id, hot)| hot.wakeup_pointer.map(|_| actor_id))
      .collect::<alloc::vec::Vec<_>>();
    for actor_id in actor_ids {
      Pallet::<T>::wakeup_substrate_invalidate(actor_id)
        .expect("host genesis wakeup placement must be removable");
    }
    assert_eq!(WakeupCursorLen::<T>::get(WakeupClock::Block), 0);
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
        .map(|offset| QueueEntry {
          ticket: u64::from(first.saturating_add(offset)),
          actor_id: u64::MAX.saturating_sub(u64::from(first.saturating_add(offset))),
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
    ActorContracts::<T>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("benchmark Actor Contract exists")
        .funding = FundingSourcePolicy::AnyVerifiedIngress;
    });
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
  fn scheduler_on_idle_base() {
    let threshold = T::MaxIdleStarvationBlocks::get().max(1);
    let now: BlockNumberFor<T> = threshold.into();
    frame_system::Pallet::<T>::set_block_number(now);
    GlobalCircuitBreaker::<T>::put(false);
    IdleStarvationState::<T>::put(IdleStarvationPhase::Starving {
      consecutive_blocks: 1,
    });
    #[block]
    {
      let _breaker_active = GlobalCircuitBreaker::<T>::get();
      core::hint::black_box(QueueHead::<T>::get());
      core::hint::black_box(QueueTail::<T>::get());
      core::hint::black_box(QueueOccupancy::<T>::get());
      core::hint::black_box(DirtyObservationListState::<T>::get());
      Pallet::<T>::update_idle_starvation_state(now, true);
    }
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

  #[benchmark]
  fn scheduler_actor_state_probe() {
    let actor_id = bench_create_system_manual::<T>(3_000);
    install_continuation::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
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
      core::hint::black_box(Pallet::<T>::load_actor_state(actor_id));
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
      ActorContracts::<T>::mutate(actor_id, |maybe_contract| {
        maybe_contract
          .as_mut()
          .expect("benchmark actor contract exists")
          .trigger = Trigger::Cadenced { every_ticks: 1 };
      });
      ActorHot::<T>::mutate(actor_id, |maybe_hot| {
        maybe_hot
          .as_mut()
          .expect("benchmark actor hot state exists")
          .cadence_anchor_tick = None;
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
      .and_then(|hot| hot.cadence_anchor_tick)
      .and_then(|anchor| anchor.checked_add(1))
      .expect("benchmark cadence re-anchors");
    assert_eq!(
      WakeupBuckets::<T>::get(WakeupKey::Tick(rearmed_tick)).map(|bucket| bucket.live_entries),
      Some(1)
    );
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
        .map(|offset| QueueEntry {
          ticket: ticket.saturating_add(offset),
          actor_id: 37_000_000u64.saturating_add(ticket).saturating_add(offset),
        })
        .collect::<alloc::vec::Vec<_>>();
      QueuePages::<T>::insert(
        page_id,
        BoundedVec::<QueueEntry, T::QueuePageSize>::try_from(page)
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
      ActorContracts::<T>::get(template_id).expect("benchmark contract template");
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
            ActorContracts::<T>::insert(actor_id, contract_template.clone());
            ActorFunding::<T>::insert(actor_id, funding_template.clone());
          }
          QueueEntry {
            ticket: logical_ticket,
            actor_id,
          }
        })
        .collect::<alloc::vec::Vec<_>>();
      QueuePages::<T>::insert(
        page_id,
        BoundedVec::<QueueEntry, T::QueuePageSize>::try_from(page)
          .expect("benchmark queue page must fit configured page size"),
      );
      ticket = ticket.saturating_add(entries);
    }
    ActorIdentities::<T>::remove(template_id);
    ActorHot::<T>::remove(template_id);
    ActorContracts::<T>::remove(template_id);
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
      ActorContracts::<T>::get(template_id).expect("benchmark contract template");
    let funding_template = ActorFunding::<T>::get(template_id).expect("benchmark funding template");
    ActorIdentities::<T>::remove(template_id);
    ActorHot::<T>::remove(template_id);
    ActorContracts::<T>::remove(template_id);
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
      ActorContracts::<T>::insert(actor_id, contract_template.clone());
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
      ActorContracts::<T>::take(system_template_id).expect("System contract template");
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
        ActorContracts::<T>::remove(template_id);
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
      ActorContracts::<T>::insert(actor_id, contract_template.clone());
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
  fn continuation_suspend(s: Linear<0, 20>) {
    let actor_id = bench_create_system_manual::<T>(50_000_000);
    install_continuation::<T>(actor_id, s);
    let state = ContinuationStateStore::<T>::take(actor_id).expect("benchmark continuation exists");
    ActorHot::<T>::mutate(actor_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("benchmark actor hot state exists")
        .cycle_state = CycleState::Idle;
    });
    #[block]
    {
      Pallet::<T>::persist_continuation_suspension(actor_id, 1, state, SuspensionReason::Temporary)
        .expect("benchmark suspension must persist");
    }
    assert!(ContinuationStateStore::<T>::contains_key(actor_id));
  }

  #[benchmark(pov_mode = Measured)]
  fn continuation_retry() {
    let actor_id = bench_create_system_manual::<T>(50_000_001);
    install_continuation::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    #[block]
    {
      core::hint::black_box(Pallet::<T>::begin_continuation_attempt(
        actor_id,
        1,
        2u32.into(),
      ));
    }
    assert_eq!(
      ContinuationStateStore::<T>::get(actor_id)
        .expect("benchmark continuation remains")
        .last_attempt_block,
      2u32.into()
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn continuation_complete() {
    let actor_id = bench_create_system_manual::<T>(50_000_002);
    install_continuation::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
    #[block]
    {
      Pallet::<T>::write_continuation_state(actor_id, None)
        .expect("benchmark completion must clear continuation");
    }
    assert!(ContinuationStateStore::<T>::get(actor_id).is_none());
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| hot.cycle_state == CycleState::Idle));
  }

  #[benchmark]
  fn continuation_cancel() {
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
    install_continuation::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
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
    cancel_continuation(RawOrigin::Root, actor_id);
    assert!(ContinuationStateStore::<T>::get(actor_id).is_none());
    assert!(ActorHot::<T>::get(actor_id).is_some_and(|hot| {
      hot.cycle_state == CycleState::Idle && hot.pending_signal && hot.queue_ticket.is_some()
    }));
    assert!(!WakeupPages::<T>::contains_key((
      WakeupKey::Block(wakeup_block),
      1,
    )));
  }

  #[benchmark]
  fn continuation_suffix_admission(n: Linear<1, 10>) {
    let bounded = n.min(T::MaxContractSteps::get());
    let recipient: T::AccountId = account("continuation_suffix_recipient", 0, 0);
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
      core::hint::black_box(Pallet::<T>::dirty_observation_fanout_base_probe());
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
      actors.push(bench_create_system_observation::<T>(owner, feed));
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
  fn observation_fanout_blocked_page() {
    let feed = T::BenchmarkHelper::setup_observation_feeds(1)
      .expect("observation benchmark feed must be available")
      .into_iter()
      .next()
      .expect("one observation benchmark feed is required");
    let mut actors = alloc::vec::Vec::new();
    for index in 0..T::ObservationPageSize::get() {
      let owner: T::AccountId = account("observation-fanout-blocked", index, 0);
      actors.push(bench_create_system_observation::<T>(owner, feed));
    }
    install_saturated_tombstone_queue::<T>();
    Pallet::<T>::note_observation_changed(feed, 1)
      .expect("observation change ingress must succeed");
    #[block]
    {
      Pallet::<T>::do_fanout_dirty_observation_page()
        .expect("blocked observation fanout page must remain retryable");
    }
    assert!(DirtyObservationFeeds::<T>::get(feed).is_some());
    for actor_id in actors {
      assert!(
        ActorHot::<T>::get(actor_id)
          .is_some_and(|hot| { hot.pending_signal && hot.queue_ticket.is_none() })
      );
    }
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
    install_continuation::<T>(actor_id, T::MaxOpeningSnapshotEntries::get());
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
      let contract = ActorContracts::<T>::get(*actor_id).expect("Actor Contract exists");
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

  #[cfg(test)]
  use crate::mock::{Test, new_test_ext};
  #[cfg(test)]
  impl_benchmark_test_suite!(Pallet, new_test_ext(), Test, extra = false);
}
