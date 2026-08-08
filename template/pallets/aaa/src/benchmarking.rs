#![cfg(feature = "runtime-benchmarks")]

extern crate alloc;

use crate::types::Task as AaaTask;
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

  fn ensure_creation_balance<T: Config>(owner: &T::AccountId) {
    let creation_fee = T::AaaCreationFee::get();
    if creation_fee.is_zero() {
      return;
    }
    let amount = creation_fee.saturating_add(One::one());
    let _ = T::AssetOps::mint(owner, T::FeeNativeAssetId::get(), amount);
  }

  fn prefund_user_sovereign<T: Config>(
    owner: &T::AccountId,
    slot: u8,
    execution_plan: &ExecutionPlanOf<T>,
  ) {
    let envelope = Pallet::<T>::attempt_fee_envelope(AaaType::User, execution_plan, 0)
      .expect("benchmark execution plan has a checked fee envelope");
    let required = T::MinUserBalance::get().saturating_add(envelope.total);
    let sovereign = Pallet::<T>::sovereign_account_id(owner, slot);
    let _ = T::AssetOps::mint(&sovereign, T::FeeNativeAssetId::get(), required);
  }

  fn prefund_active_user_creation<T: Config>(
    owner: &T::AccountId,
    execution_plan: &ExecutionPlanOf<T>,
  ) {
    let slot =
      Pallet::<T>::available_owner_slot(owner, None).expect("benchmark owner has a free User slot");
    prefund_user_sovereign::<T>(owner, slot, execution_plan);
  }

  fn deplete_user_sovereign<T: Config>(aaa_id: AaaId) {
    let instance =
      Pallet::<T>::active_actor_view(aaa_id).expect("benchmark actor must exist for depletion");
    let requirement = Pallet::<T>::attempt_fee_envelope(AaaType::User, &instance.execution_plan, 0)
      .expect("benchmark execution plan has a checked fee envelope");
    let required = T::MinUserBalance::get().saturating_add(requirement.total);
    T::AssetOps::burn(
      &instance.sovereign_account,
      T::FeeNativeAssetId::get(),
      required,
    )
    .expect("benchmark depletion must not overdraw the sovereign");
  }

  fn user_program<T: Config>(
    schedule: ScheduleOf<T>,
    execution_plan: ExecutionPlanOf<T>,
  ) -> ProgramInputOf<T> {
    ProgramInput::Active(ActiveProgramInput {
      schedule,
      schedule_window: None,
      execution_plan,
      completion_policy: CompletionPolicy::Persistent,
      funding_source_policy: FundingSourcePolicy::OwnerOnly,
      auto_close_at_cycle_nonce: None,
    })
  }

  fn system_program<T: Config>(
    schedule: ScheduleOf<T>,
    execution_plan: ExecutionPlanOf<T>,
  ) -> ProgramInputOf<T> {
    ProgramInput::Active(ActiveProgramInput {
      schedule,
      schedule_window: None,
      execution_plan,
      completion_policy: CompletionPolicy::Persistent,
      funding_source_policy: FundingSourcePolicy::RuntimePolicy,
      auto_close_at_cycle_nonce: None,
    })
  }

  fn full_attempt_fee<T: Config>(execution_plan: &ExecutionPlanOf<T>) -> T::Balance {
    Pallet::<T>::attempt_fee_envelope(AaaType::User, execution_plan, 0)
      .expect("benchmark execution plan has a checked fee envelope")
      .total
  }

  fn make_execution_plan<T: Config>(recipient: T::AccountId) -> ExecutionPlanOf<T> {
    let step = Step {
      conditions: ConditionSet::Always,
      task: AaaTask::Transfer {
        to: recipient,
        asset: T::FeeNativeAssetId::get(),
        amount: AmountResolution::AllAvailable,
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    BoundedVec::try_from(vec![step]).expect("single-step execution_plan must fit")
  }

  fn make_tracked_funding_execution_plan<T: Config>(recipient: T::AccountId) -> ExecutionPlanOf<T> {
    BoundedVec::try_from(vec![Step {
      conditions: ConditionSet::Always,
      task: AaaTask::Transfer {
        to: recipient,
        asset: T::FeeNativeAssetId::get(),
        amount: AmountResolution::PercentageOfLastFunding(polkadot_sdk::sp_runtime::Perbill::one()),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }])
    .expect("single-step tracked funding plan must fit")
  }

  fn make_remove_liquidity_execution_plan<T: Config>(
    lp_asset: T::AssetId,
    asset_a: T::AssetId,
    asset_b: T::AssetId,
    amount: T::Balance,
  ) -> ExecutionPlanOf<T> {
    let step = Step {
      conditions: ConditionSet::Always,
      task: AaaTask::RemoveLiquidity {
        lp_asset,
        asset_a,
        asset_b,
        lp_amount: AmountResolution::Fixed(amount),
        min_amount_a: One::one(),
        min_amount_b: One::one(),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    BoundedVec::try_from(vec![step]).expect("single-step execution_plan must fit")
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

  fn seed_actor_for_cycle<T: Config>(aaa_id: AaaId) {
    let Some(instance) = Pallet::<T>::active_actor_view(aaa_id) else {
      return;
    };
    let reserve = full_attempt_fee::<T>(&instance.execution_plan)
      .saturating_add(T::MinUserBalance::get())
      .saturating_add(One::one());
    let _ = T::AssetOps::mint(
      &instance.sovereign_account,
      T::FeeNativeAssetId::get(),
      reserve,
    );
  }

  fn bench_create_user<T: Config>(caller: T::AccountId) -> AaaId {
    ensure_creation_balance::<T>(&caller);
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    let execution_plan = make_execution_plan::<T>(recipient);
    prefund_active_user_creation::<T>(&caller, &execution_plan);
    let schedule = Schedule {
      trigger: Trigger::immediate_manual(),
      cooldown_blocks: 10,
    };
    Pallet::<T>::create_user_aaa(
      RawOrigin::Signed(caller).into(),
      Mutability::Mutable,
      user_program::<T>(schedule, execution_plan),
    )
    .expect("create_user_aaa must succeed in benchmark setup");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    seed_actor_for_cycle::<T>(aaa_id);
    aaa_id
  }

  #[benchmark]
  fn create_user_aaa() {
    let caller: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&caller);
    let expected_slot = prefill_owner_slots_for_worst_case::<T>(&caller);
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    let execution_plan = make_execution_plan::<T>(recipient);
    prefund_user_sovereign::<T>(&caller, expected_slot, &execution_plan);
    let schedule = Schedule {
      trigger: Trigger::immediate_manual(),
      cooldown_blocks: 10,
    };
    #[extrinsic_call]
    create_user_aaa(
      RawOrigin::Signed(caller),
      Mutability::Mutable,
      user_program::<T>(schedule, execution_plan),
    );
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let inst =
      Pallet::<T>::active_actor_view(aaa_id).expect("AAA must exist after create_user_aaa");
    assert_eq!(inst.actor_class.owner_slot(), Some(expected_slot));
  }

  #[benchmark]
  fn create_user_aaa_at_slot() {
    let caller: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&caller);
    let requested_slot = T::MaxOwnerSlots::get().saturating_sub(1);
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    let execution_plan = make_execution_plan::<T>(recipient);
    prefund_user_sovereign::<T>(&caller, requested_slot, &execution_plan);
    let schedule = Schedule {
      trigger: Trigger::immediate_manual(),
      cooldown_blocks: 10,
    };
    #[extrinsic_call]
    create_user_aaa_at_slot(
      RawOrigin::Signed(caller),
      requested_slot,
      Mutability::Mutable,
      user_program::<T>(schedule, execution_plan),
    );
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let inst =
      Pallet::<T>::active_actor_view(aaa_id).expect("AAA must exist after create_user_aaa_at_slot");
    assert_eq!(inst.actor_class.owner_slot(), Some(requested_slot));
  }

  #[benchmark]
  fn create_system_aaa() {
    let owner: T::AccountId = whitelisted_caller();
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    let execution_plan = make_execution_plan::<T>(recipient);
    let schedule = Schedule {
      trigger: Trigger::immediate_manual(),
      cooldown_blocks: 100,
    };
    #[extrinsic_call]
    create_system_aaa(
      RawOrigin::Root,
      owner,
      Mutability::Mutable,
      system_program::<T>(schedule, execution_plan),
    );
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let inst =
      Pallet::<T>::active_actor_view(aaa_id).expect("AAA must exist after create_system_aaa");
    assert_eq!(
      inst.actor_class,
      ActorClass::System {
        sovereign_id: aaa_id,
      }
    );
  }

  #[benchmark]
  fn create_system_aaa_at_sovereign_id() {
    let owner: T::AccountId = whitelisted_caller();
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    let execution_plan = make_execution_plan::<T>(recipient.clone());
    let schedule = Schedule {
      trigger: Trigger::immediate_manual(),
      cooldown_blocks: 100,
    };
    Pallet::<T>::create_system_aaa(
      RawOrigin::Root.into(),
      owner.clone(),
      Mutability::Mutable,
      system_program::<T>(schedule.clone(), execution_plan.clone()),
    )
    .expect("create_system_aaa must succeed in benchmark setup");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    Pallet::<T>::close_aaa(RawOrigin::Root.into(), aaa_id)
      .expect("close_aaa must succeed in benchmark setup");
    let fresh_id = NextAaaId::<T>::get();
    #[extrinsic_call]
    create_system_aaa_at_sovereign_id(
      RawOrigin::Root,
      aaa_id,
      owner,
      Mutability::Mutable,
      system_program::<T>(schedule, execution_plan),
    );
    assert!(Pallet::<T>::active_actor_exists(fresh_id));
  }

  #[benchmark]
  fn create_dormant_system_aaa() {
    let owner: T::AccountId = whitelisted_caller();
    #[block]
    {
      Pallet::<T>::create_system_aaa(
        RawOrigin::Root.into(),
        owner,
        Mutability::Mutable,
        ProgramInput::Dormant,
      )
      .expect("dormant System identity creation must succeed");
    }
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    assert!(ActorIdentities::<T>::contains_key(aaa_id));
    assert!(!Pallet::<T>::active_actor_exists(aaa_id));
  }

  #[benchmark]
  fn activate_aaa() {
    let owner: T::AccountId = whitelisted_caller();
    Pallet::<T>::create_system_aaa(
      RawOrigin::Root.into(),
      owner.clone(),
      Mutability::Mutable,
      ProgramInput::Dormant,
    )
    .expect("dormant System identity creation must succeed");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let recipient: T::AccountId = account("activate-recipient", 0, 0);
    let program = system_program::<T>(
      Schedule {
        trigger: Trigger::immediate_manual(),
        cooldown_blocks: 100,
      },
      make_execution_plan::<T>(recipient),
    );
    #[extrinsic_call]
    activate_aaa(RawOrigin::Signed(owner), aaa_id, program);
    assert!(Pallet::<T>::active_actor_exists(aaa_id));
    assert!(ActorIdentities::<T>::contains_key(aaa_id));
  }

  #[benchmark]
  fn deactivate_aaa() {
    let owner: T::AccountId = whitelisted_caller();
    let recipient: T::AccountId = account("deactivate-recipient", 0, 0);
    let execution_plan = make_execution_plan::<T>(recipient);
    Pallet::<T>::create_system_aaa(
      RawOrigin::Root.into(),
      owner.clone(),
      Mutability::Mutable,
      system_program::<T>(
        Schedule {
          trigger: Trigger::immediate_manual(),
          cooldown_blocks: 100,
        },
        execution_plan,
      ),
    )
    .expect("System AAA creation must succeed");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    install_continuation::<T>(aaa_id, T::MaxOpeningSnapshotEntries::get());
    #[extrinsic_call]
    deactivate_aaa(RawOrigin::Signed(owner), aaa_id);
    assert!(!Pallet::<T>::active_actor_exists(aaa_id));
    assert!(ActorIdentities::<T>::contains_key(aaa_id));
  }

  #[benchmark]
  fn pause_aaa() {
    let caller: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(caller.clone());
    #[extrinsic_call]
    pause_aaa(RawOrigin::Signed(caller), aaa_id);
    let inst = Pallet::<T>::active_actor_view(aaa_id).expect("AAA must exist after pause_aaa");
    assert!(inst.lifecycle.is_paused());
  }

  #[benchmark]
  fn resume_aaa() {
    let caller: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(caller.clone());
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    Pallet::<T>::pause_aaa(RawOrigin::Signed(caller.clone()).into(), aaa_id)
      .expect("pause_aaa must succeed in setup");
    frame_system::Pallet::<T>::set_block_number(2u32.into());
    #[extrinsic_call]
    resume_aaa(RawOrigin::Signed(caller), aaa_id);
    let inst = Pallet::<T>::active_actor_view(aaa_id).expect("AAA must exist after resume_aaa");
    assert!(!inst.lifecycle.is_paused());
  }

  #[benchmark]
  fn manual_trigger() {
    let caller: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(caller.clone());
    #[extrinsic_call]
    manual_trigger(RawOrigin::Signed(caller), aaa_id);
    let inst = Pallet::<T>::active_actor_view(aaa_id).expect("AAA must exist after manual_trigger");
    assert!(inst.pending_signal);
  }

  #[benchmark]
  fn close_aaa() {
    let owner: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&owner);
    let owner_slot = prefill_owner_slots_for_worst_case::<T>(&owner);
    let recipient: T::AccountId = account("close-recipient", 0, 0);
    let schedule = Schedule {
      trigger: Trigger::immediate_manual(),
      cooldown_blocks: 1,
    };
    let execution_plan = make_execution_plan::<T>(recipient);
    prefund_user_sovereign::<T>(&owner, owner_slot, &execution_plan);
    Pallet::<T>::create_user_aaa_at_slot(
      RawOrigin::Signed(owner.clone()).into(),
      owner_slot,
      Mutability::Mutable,
      user_program::<T>(schedule, execution_plan),
    )
    .expect("create_user_aaa_at_slot must succeed in close_aaa benchmark setup");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    install_continuation::<T>(aaa_id, T::MaxOpeningSnapshotEntries::get());
    #[extrinsic_call]
    close_aaa(RawOrigin::Signed(owner), aaa_id);
    assert!(!Pallet::<T>::active_actor_exists(aaa_id));
  }

  // Diagnostic counterpart for the System branch; production close pricing uses the heavier User path.
  #[benchmark]
  fn close_aaa_system_pure() {
    let owner: T::AccountId = whitelisted_caller();
    let recipient: T::AccountId = account("system-close-recipient", 0, 0);
    let schedule = Schedule {
      trigger: Trigger::immediate_manual(),
      cooldown_blocks: 1,
    };
    Pallet::<T>::create_system_aaa(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      system_program::<T>(schedule, make_execution_plan::<T>(recipient)),
    )
    .expect("create_system_aaa must succeed in System close benchmark setup");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    #[block]
    {
      Pallet::<T>::close_aaa(RawOrigin::Root.into(), aaa_id)
        .expect("System close must succeed in benchmark");
    }
    assert!(!Pallet::<T>::active_actor_exists(aaa_id));
  }

  #[benchmark]
  fn update_schedule() {
    let caller: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(caller.clone());
    install_continuation::<T>(aaa_id, T::MaxOpeningSnapshotEntries::get());
    ActorHot::<T>::mutate(aaa_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("benchmark actor hot state exists")
        .pending_signal = true;
    });
    let new_schedule = Schedule {
      trigger: Trigger::immediate_manual(),
      cooldown_blocks: 20,
    };
    #[extrinsic_call]
    update_schedule(RawOrigin::Signed(caller), aaa_id, new_schedule, None);
    let inst =
      Pallet::<T>::active_actor_view(aaa_id).expect("AAA must exist after update_schedule");
    assert_eq!(inst.schedule.cooldown_blocks, 20);
  }

  #[benchmark]
  fn update_funding_source_policy() {
    let caller: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(caller.clone());
    install_continuation::<T>(aaa_id, T::MaxOpeningSnapshotEntries::get());
    ActorHot::<T>::mutate(aaa_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("benchmark actor hot state exists")
        .pending_signal = true;
    });
    let mut allowed: BoundedBTreeSet<T::AccountId, T::MaxWhitelistSize> =
      BoundedBTreeSet::default();
    for index in 0..T::MaxWhitelistSize::get() {
      allowed
        .try_insert(account("funding-source", index, 0))
        .expect("funding source must fit benchmark bound");
    }
    let policy = FundingSourcePolicy::SignedAllowlist(allowed);
    #[extrinsic_call]
    update_funding_source_policy(RawOrigin::Signed(caller), aaa_id, policy);
    let funding = ActorFunding::<T>::get(aaa_id).expect("actor funding must exist after update");
    assert!(matches!(
      funding.funding_source_policy,
      FundingSourcePolicy::SignedAllowlist(_)
    ));
  }

  #[benchmark]
  fn update_execution_plan() {
    let caller: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(caller.clone());
    install_continuation::<T>(aaa_id, T::MaxOpeningSnapshotEntries::get());
    ActorHot::<T>::mutate(aaa_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("benchmark actor hot state exists")
        .pending_signal = true;
    });
    let funding_assets = T::BenchmarkHelper::funding_assets(T::MaxFundingTrackedAssets::get());
    ActorFunding::<T>::mutate(aaa_id, |maybe| {
      let funding = maybe.as_mut().expect("benchmark actor funding exists");
      for asset in funding_assets {
        funding
          .funding_accumulated
          .try_insert(asset, One::one())
          .expect("funding accumulator benchmark bound fits");
      }
    });
    let recipient = account("recipient", 0, 0);
    let replacement = make_execution_plan::<T>(recipient);
    #[extrinsic_call]
    update_execution_plan(
      RawOrigin::Signed(caller),
      aaa_id,
      replacement.clone(),
      CompletionPolicy::Persistent,
    );
    let inst =
      Pallet::<T>::active_actor_view(aaa_id).expect("AAA must exist after update_execution_plan");
    assert_eq!(inst.execution_plan, replacement);
    assert!(
      ActorFunding::<T>::get(aaa_id)
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
    let aaa_id = bench_create_user::<T>(caller.clone());
    #[extrinsic_call]
    permissionless_sweep(RawOrigin::Signed(caller), aaa_id);
    assert!(Pallet::<T>::active_actor_exists(aaa_id));
  }

  #[benchmark]
  fn permissionless_sweep_many(n: Linear<1, 5>) {
    let caller: T::AccountId = whitelisted_caller();
    let mut aaa_ids: BoundedVec<AaaId, T::MaxSweepBatch> = BoundedVec::default();
    let schedule = Schedule {
      trigger: Trigger::immediate_manual(),
      cooldown_blocks: 10,
    };
    let bounded_n = n.min(T::MaxSweepBatch::get());
    for i in 0..bounded_n {
      let owner: T::AccountId = account("sweep-owner", i, 0);
      let recipient: T::AccountId = account("sweep-recipient", i, 0);
      ensure_creation_balance::<T>(&owner);
      let execution_plan = make_execution_plan::<T>(recipient);
      prefund_active_user_creation::<T>(&owner, &execution_plan);
      Pallet::<T>::create_user_aaa(
        RawOrigin::Signed(owner).into(),
        Mutability::Mutable,
        user_program::<T>(schedule.clone(), execution_plan),
      )
      .expect("create_user_aaa must succeed in permissionless_sweep_many setup");
      let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
      // Restore the zombie fixture state: swept actors must be balance-exhausted so the
      // sweep closes them, keeping the prefunding admission honest but the postcondition real.
      deplete_user_sovereign::<T>(aaa_id);
      aaa_ids
        .try_push(aaa_id)
        .expect("benchmark n must fit MaxSweepBatch");
    }
    let expected_len = aaa_ids.len();
    #[extrinsic_call]
    permissionless_sweep_many(RawOrigin::Signed(caller), aaa_ids.clone());
    for aaa_id in aaa_ids {
      assert!(!Pallet::<T>::active_actor_exists(aaa_id));
    }
    assert_eq!(expected_len, bounded_n as usize);
  }

  #[benchmark]
  fn fee_collection() {
    let payer: T::AccountId = whitelisted_caller();
    let owner: T::AccountId = account("fee-sink-owner", 0, 0);
    let schedule = Schedule {
      trigger: Trigger::cadenced_always(1),
      cooldown_blocks: 0,
    };
    Pallet::<T>::create_system_aaa(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      system_program::<T>(schedule, make_inert_execution_plan::<T>()),
    )
    .expect("fee-collection benchmark sink must be created");
    let fee_sink_id = NextAaaId::<T>::get().saturating_sub(1);
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
    let (target_id, recipient) = prepare_saturated_address_actor::<T>(0);
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
    let (target_id, recipient) = prepare_saturated_address_actor::<T>(0);
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
  fn condition_set_all_max() {
    let actor: T::AccountId = account("condition-all", 0, 0);
    let max_conditions = T::MaxConditionsPerStep::get();
    let assets = T::BenchmarkHelper::setup_condition_assets(&actor, max_conditions)
      .expect("condition benchmark assets must be available");
    assert!(assets.len() >= max_conditions as usize);
    let conditions = BoundedVec::try_from(
      assets
        .into_iter()
        .take(max_conditions as usize)
        .map(|asset| {
          T::AssetOps::mint(&actor, asset, T::MinUserBalance::get())
            .expect("condition benchmark asset must be funded");
          Condition::BalanceAbove {
            asset,
            threshold: T::Balance::zero(),
          }
        })
        .collect::<alloc::vec::Vec<_>>(),
    )
    .expect("maximum condition group fits");
    let condition_set = ConditionSet::All(conditions);
    #[block]
    {
      assert_eq!(
        Pallet::<T>::evaluate_condition_set(&condition_set, &actor, T::Balance::zero()),
        Ok(true)
      );
    }
  }

  #[benchmark]
  fn condition_set_observation(c: Linear<1, 4>) {
    let actor: T::AccountId = account("condition-observation", 0, 0);
    let bounded = c.min(T::MaxConditionsPerStep::get());
    let feeds = T::BenchmarkHelper::setup_observation_feeds(bounded)
      .expect("observation benchmark feeds must be available");
    assert!(feeds.len() >= bounded as usize);
    let conditions = BoundedVec::try_from(
      feeds
        .into_iter()
        .take(bounded as usize)
        .map(|feed| Condition::ObservationAbove {
          feed,
          threshold: 0,
          max_age_blocks: 100,
        })
        .collect::<alloc::vec::Vec<_>>(),
    )
    .expect("maximum observation condition group fits");
    let condition_set = ConditionSet::All(conditions);
    #[block]
    {
      let _ = Pallet::<T>::evaluate_condition_set(&condition_set, &actor, T::Balance::zero());
    }
  }

  #[benchmark]
  fn condition_set_evaluation(c: Linear<1, 4>) {
    let actor: T::AccountId = account("condition-any", 0, 0);
    let bounded = c.min(T::MaxConditionsPerStep::get());
    let assets = T::BenchmarkHelper::setup_condition_assets(&actor, bounded)
      .expect("condition benchmark assets must be available");
    assert!(assets.len() >= bounded as usize);
    let conditions = BoundedVec::try_from(
      assets
        .into_iter()
        .take(bounded as usize)
        .map(|asset| {
          T::AssetOps::mint(&actor, asset, T::MinUserBalance::get())
            .expect("condition benchmark asset must be funded");
          Condition::BalanceAbove {
            asset,
            threshold: T::Balance::zero(),
          }
        })
        .collect::<alloc::vec::Vec<_>>(),
    )
    .expect("maximum condition group fits");
    let condition_set = ConditionSet::Any(conditions);
    #[block]
    {
      assert_eq!(
        Pallet::<T>::evaluate_condition_set(&condition_set, &actor, T::Balance::zero()),
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
    let mut targets: alloc::vec::Vec<(AaaId, T::AccountId)> = alloc::vec::Vec::new();
    for seed in 0..bounded_legs {
      targets.push(prepare_saturated_address_actor::<T>(seed));
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
    let (target_id, recipient) = prepare_saturated_address_actor::<T>(0);
    let amount = T::MinUserBalance::get().saturating_add(One::one());
    #[block]
    {
      T::BenchmarkHelper::run_xcm_asset_deposit(&recipient, &source, amount)
        .expect("AAA-aware XCM deposit must succeed");
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
      T::DexOps::swap_exact_in(
        ExecutionContext::new(&caller, AaaType::User),
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
      T::DexOps::swap_exact_out(
        ExecutionContext::new(&caller, AaaType::User),
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
      trigger: Trigger::immediate_manual(),
      cooldown_blocks: 10,
    };
    let execution_plan =
      make_remove_liquidity_execution_plan::<T>(lp_asset, asset_a, asset_b, lp_amount);
    prefund_active_user_creation::<T>(&caller, &execution_plan);
    Pallet::<T>::create_user_aaa(
      RawOrigin::Signed(caller.clone()).into(),
      Mutability::Mutable,
      user_program::<T>(schedule, execution_plan),
    )
    .expect("create_user_aaa must succeed in setup");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let actor = Pallet::<T>::active_actor_view(aaa_id)
      .map(|instance| instance.sovereign_account)
      .expect("actor must exist after setup");
    seed_actor_for_cycle::<T>(aaa_id);
    T::AssetOps::transfer(&caller, &actor, lp_asset, lp_amount)
      .expect("LP transfer to actor must succeed");
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    Pallet::<T>::manual_trigger(RawOrigin::Signed(caller).into(), aaa_id)
      .expect("manual_trigger must succeed in setup");
    #[block]
    {
      let _ = Pallet::<T>::on_idle(1u32.into(), Weight::MAX);
    }
    let inst = Pallet::<T>::active_actor_view(aaa_id).expect("actor must survive benchmark cycle");
    assert_eq!(inst.cycle_nonce, 1);
    assert_eq!(inst.consecutive_failures, 0);
  }

  fn make_inert_execution_plan<T: Config>() -> ExecutionPlanOf<T> {
    let step = Step {
      conditions: ConditionSet::Always,
      task: AaaTask::StopCycle,
      on_error: StepErrorPolicy::AbortCycle,
    };
    BoundedVec::try_from(vec![step]).expect("single-step execution_plan must fit")
  }

  fn inert_execution_plan_of_len<T: Config>(steps: u32) -> ExecutionPlanOf<T> {
    let bounded = steps.min(T::MaxExecutionPlanSteps::get());
    let mut plan = alloc::vec::Vec::new();
    for _ in 0..bounded {
      plan.push(Step {
        conditions: ConditionSet::Always,
        task: AaaTask::StopCycle,
        on_error: StepErrorPolicy::AbortCycle,
      });
    }
    BoundedVec::try_from(plan).expect("benchmark inert execution_plan must fit")
  }

  fn bench_create_system_with_plan<T: Config>(
    seed: u32,
    execution_plan: ExecutionPlanOf<T>,
  ) -> AaaId {
    let owner: T::AccountId = account("cycle_owner", seed, 0);
    let schedule = Schedule {
      trigger: Trigger::immediate_manual(),
      cooldown_blocks: 0,
    };
    Pallet::<T>::create_system_aaa(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      system_program::<T>(schedule, execution_plan),
    )
    .expect("create_system_aaa must succeed in cycle benchmark setup");
    NextAaaId::<T>::get().saturating_sub(1)
  }

  fn bench_create_system_observation<T: Config>(
    owner: T::AccountId,
    feed: T::ObservationFeedId,
  ) -> AaaId {
    let schedule = Schedule {
      trigger: Trigger::Immediate {
        sources: BoundedVec::try_from(vec![TriggerSource::OnObservationChange { feed }])
          .expect("one observation source must fit"),
      },
      cooldown_blocks: 0,
    };
    Pallet::<T>::create_system_aaa(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      system_program::<T>(schedule, make_inert_execution_plan::<T>()),
    )
    .expect("observation benchmark actor creation must succeed");
    NextAaaId::<T>::get().saturating_sub(1)
  }

  fn bench_create_system_manual<T: Config>(seed: u32) -> AaaId {
    let owner: T::AccountId = account("wakeup_owner", seed, 0);
    let schedule = Schedule {
      trigger: Trigger::immediate_manual(),
      cooldown_blocks: 0,
    };
    let execution_plan = make_inert_execution_plan::<T>();
    Pallet::<T>::create_system_aaa(
      RawOrigin::Root.into(),
      owner,
      Mutability::Mutable,
      system_program::<T>(schedule, execution_plan),
    )
    .expect("create_system_aaa must succeed in wakeup benchmark setup");
    NextAaaId::<T>::get().saturating_sub(1)
  }

  fn install_continuation<T: Config>(aaa_id: AaaId, snapshot_entries: u32) {
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
    ActorProgram::<T>::mutate(aaa_id, |maybe_program| {
      maybe_program
        .as_mut()
        .expect("benchmark actor program exists")
        .execution_plan[0]
        .on_error = StepErrorPolicy::RetryLater {
        max_attempts: T::MaxRetryAttempts::get(),
      };
    });
    ActorHot::<T>::mutate(aaa_id, |maybe_hot| {
      let hot = maybe_hot
        .as_mut()
        .expect("benchmark actor hot state exists");
      hot.cycle_state = CycleState::Suspended;
      hot.pending_signal = false;
      hot.queue_ticket = None;
      hot.wakeup_pointer = None;
    });
    ActorIdentities::<T>::mutate(aaa_id, |maybe_identity| {
      maybe_identity
        .as_mut()
        .expect("benchmark actor identity exists")
        .cycle_nonce = 1;
    });
    ContinuationStateStore::<T>::insert(
      aaa_id,
      ContinuationState {
        cursor: 0,
        attempt: 0,
        unsuccessful_attempts_at_cursor: 1,
        last_attempt_block: 1u32.into(),
        opening_snapshot,
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
        .try_push(block)
        .expect("benchmark cursor page must fit configured bound");
      WakeupBuckets::<T>::insert(
        block,
        WakeupBucketState {
          head_page: 0,
          tail_page: 0,
          next_page_id: 1,
          live_entries: 1,
          cursor_index: Some(index),
        },
      );
    }
    WakeupCursorPages::<T>::insert(page_id, page);
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
    WakeupCursorLen::<T>::put(cursor_len);
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
          aaa_id: u64::MAX.saturating_sub(u64::from(first.saturating_add(offset))),
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

  fn prepare_saturated_address_actor<T: Config>(seed: u32) -> (AaaId, T::AccountId) {
    let owner: T::AccountId = account("ingress_owner", seed, 0);
    let native = T::FeeNativeAssetId::get();
    let native_only =
      BoundedVec::try_from(vec![native]).expect("one asset must fit the trigger filter bound");
    let mut candidates = vec![
      TriggerSource::Manual,
      TriggerSource::OnAddressEvent {
        source_filter: SourceFilter::Any,
        asset_filter: AssetFilter::Any,
      },
      TriggerSource::OnAddressEvent {
        source_filter: SourceFilter::OwnerOnly,
        asset_filter: AssetFilter::Any,
      },
      TriggerSource::OnAddressEvent {
        source_filter: SourceFilter::Any,
        asset_filter: AssetFilter::Whitelist(native_only),
      },
    ];
    candidates.sort_by_key(Encode::encode);
    let max_sources = T::MaxTriggerSources::get() as usize;
    assert!(
      (1..=candidates.len()).contains(&max_sources),
      "benchmark source corpus must saturate MaxTriggerSources"
    );
    candidates.truncate(max_sources);
    let sources = BoundedVec::try_from(candidates)
      .expect("saturated trigger sources must fit the runtime bound");
    let schedule = Schedule {
      trigger: Trigger::Immediate { sources },
      cooldown_blocks: 0,
    };
    Pallet::<T>::create_system_aaa(
      RawOrigin::Root.into(),
      owner.clone(),
      Mutability::Mutable,
      system_program::<T>(schedule, make_tracked_funding_execution_plan::<T>(owner)),
    )
    .expect("create_system_aaa must succeed in ingress benchmark setup");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let recipient = Pallet::<T>::sovereign_account_id_system(aaa_id);
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    ActorFunding::<T>::mutate(aaa_id, |maybe| {
      let funding = maybe.as_mut().expect("benchmark actor funding exists");
      funding.funding_source_policy = FundingSourcePolicy::AnyVerifiedIngress;
      funding
        .funding_accumulated
        .try_insert(T::FeeNativeAssetId::get(), One::one())
        .expect("tracked funding accumulator fits");
    });
    install_saturated_tombstone_queue::<T>();
    (aaa_id, recipient)
  }

  // Non-dispatch diagnostic benchmark proving cooldown-ineligible timers own no queue probe.
  #[benchmark]
  fn scheduler_cooldown_ineligible_idle() {
    let owner: T::AccountId = whitelisted_caller();
    let schedule = Schedule {
      trigger: Trigger::cadenced_always(1),
      cooldown_blocks: 10,
    };
    Pallet::<T>::create_system_aaa(
      RawOrigin::Root.into(),
      owner.clone(),
      Mutability::Mutable,
      system_program::<T>(schedule, make_inert_execution_plan::<T>()),
    )
    .expect("System timer creation must succeed");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let first_block: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(first_block);
    let _ = Pallet::<T>::on_idle(first_block, Weight::MAX);
    let expected_wakeup: BlockNumberFor<T> = 11u32.into();
    assert_eq!(
      ActorHot::<T>::get(aaa_id).and_then(|hot| hot.wakeup_pointer.map(|pointer| pointer.block)),
      Some(expected_wakeup)
    );
    let now: BlockNumberFor<T> = 2u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    #[block]
    {
      let _ = Pallet::<T>::execute_cycle(Weight::MAX);
    }
    let instance = Pallet::<T>::active_actor_view(aaa_id).expect("AAA exists");
    assert_eq!(instance.cycle_nonce, 1);
    assert_eq!(
      ActorHot::<T>::get(aaa_id).and_then(|hot| hot.wakeup_pointer.map(|pointer| pointer.block)),
      Some(expected_wakeup)
    );
    assert!(ActorHot::<T>::get(aaa_id).is_some_and(|hot| hot.queue_ticket.is_none()));
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
    WakeupCursorLen::<T>::put(0);
    IdleStarvationState::<T>::kill();
    #[block]
    {
      core::hint::black_box(Pallet::<T>::on_idle(now, Weight::MAX));
    }
    assert!(!IdleStarvationState::<T>::exists());
  }

  #[benchmark]
  fn scheduler_actor_hot_probe() {
    let aaa_id = bench_create_system_manual::<T>(3_000);
    ActorHot::<T>::mutate(aaa_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("benchmark actor hot state must exist")
        .lifecycle = ActiveLifecycle::Paused;
    });
    #[block]
    {
      Pallet::<T>::benchmark_scheduler_actor_hot_probe(aaa_id);
    }
  }

  #[benchmark]
  fn scheduler_actor_program_probe() {
    let aaa_id = bench_create_system_manual::<T>(3_001);
    assert!(
      ContinuationStateStore::<T>::get(aaa_id).is_none(),
      "ordinary readiness benchmark must retain the absent-Continuation envelope"
    );
    ActorHot::<T>::mutate(aaa_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("benchmark actor hot state must exist")
        .pending_signal = true;
    });
    let hot = ActorHot::<T>::get(aaa_id).expect("benchmark actor hot state must exist");
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    #[block]
    {
      Pallet::<T>::benchmark_scheduler_actor_program_probe(aaa_id, hot);
    }
  }
  /// One complete minimal cycle execution over one inert StopCycle step (fixed cycle
  /// orchestration plus finalization), measured on the execution path only; queue probes and
  /// head consumption are separate scheduler classes.
  #[benchmark]
  fn cycle_orchestration() {
    let aaa_id = bench_create_system_with_plan::<T>(3_100, make_inert_execution_plan::<T>());
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let instance = Pallet::<T>::active_actor_view(aaa_id).expect("cycle actor exists");
    #[block]
    {
      core::hint::black_box(Pallet::<T>::execute_single_cycle(aaa_id, instance, now));
    }
    let updated = Pallet::<T>::active_actor_view(aaa_id).expect("cycle actor survives");
    assert_eq!(updated.cycle_nonce, 1);
  }

  /// One complete cycle execution over `steps` inert StopCycle steps: the linear model prices
  /// the fixed cycle orchestration plus per-step bookkeeping, the exact cycle overhead the
  /// admission composition uses for arbitrary plans.
  #[benchmark]
  fn step_orchestration(n: Linear<1, 8>) {
    let aaa_id = bench_create_system_with_plan::<T>(3_200, inert_execution_plan_of_len::<T>(n));
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    let instance = Pallet::<T>::active_actor_view(aaa_id).expect("cycle actor exists");
    #[block]
    {
      core::hint::black_box(Pallet::<T>::execute_single_cycle(aaa_id, instance, now));
    }
    let updated = Pallet::<T>::active_actor_view(aaa_id).expect("cycle actor survives");
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
      let aaa_id = bench_create_system_manual::<T>(31_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::paged_enqueue(aaa_id));
    }
    let aaa_id = bench_create_system_manual::<T>(32_000_000);
    #[block]
    {
      assert!(Pallet::<T>::paged_enqueue(aaa_id));
    }
    assert_eq!(QueueTail::<T>::get(), u64::from(page_size));
    assert_eq!(
      ActorHot::<T>::get(aaa_id).and_then(|hot| hot.queue_ticket),
      Some(u64::from(page_size - 1))
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_append_new_page() {
    let page_size = T::QueuePageSize::get();
    for i in 0..page_size {
      let aaa_id = bench_create_system_manual::<T>(33_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::paged_enqueue(aaa_id));
    }
    let aaa_id = bench_create_system_manual::<T>(34_000_000);
    #[block]
    {
      assert!(Pallet::<T>::paged_enqueue(aaa_id));
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
      let aaa_id = bench_create_system_manual::<T>(41_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::wakeup_substrate_schedule(aaa_id, wakeup_block));
    }
    let aaa_id = bench_create_system_manual::<T>(42_000_000);
    #[block]
    {
      assert!(Pallet::<T>::wakeup_substrate_schedule(aaa_id, wakeup_block));
    }
    let pointer = ActorHot::<T>::get(aaa_id)
      .and_then(|hot| hot.wakeup_pointer)
      .expect("benchmark wakeup pointer must exist");
    assert_eq!((pointer.page_id, pointer.slot), (0, page_size - 1));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_append_new_page() {
    let page_size = T::WakeupPageSize::get();
    let wakeup_block = 100u32.into();
    for i in 0..page_size {
      let aaa_id = bench_create_system_manual::<T>(43_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::wakeup_substrate_schedule(aaa_id, wakeup_block));
    }
    let aaa_id = bench_create_system_manual::<T>(44_000_000);
    #[block]
    {
      assert!(Pallet::<T>::wakeup_substrate_schedule(aaa_id, wakeup_block));
    }
    let pointer = ActorHot::<T>::get(aaa_id)
      .and_then(|hot| hot.wakeup_pointer)
      .expect("benchmark wakeup pointer must exist");
    assert_eq!((pointer.page_id, pointer.slot), (1, 0));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_replace_exact() {
    let aaa_id = bench_create_system_manual::<T>(45_000_000);
    let old_block = 100u32.into();
    let replacement_block = 200u32.into();
    assert!(Pallet::<T>::wakeup_substrate_schedule(aaa_id, old_block));
    #[block]
    {
      assert!(Pallet::<T>::wakeup_substrate_schedule(
        aaa_id,
        replacement_block
      ));
    }
    let pointer = ActorHot::<T>::get(aaa_id)
      .and_then(|hot| hot.wakeup_pointer)
      .expect("replacement wakeup pointer must exist");
    assert_eq!(
      (pointer.block, pointer.page_id, pointer.slot),
      (replacement_block, 0, 0)
    );
    assert!(!WakeupBuckets::<T>::contains_key(old_block));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_invalidate_middle_page() {
    let page_size = T::WakeupPageSize::get();
    let wakeup_block = 100u32.into();
    let count = page_size.saturating_mul(2).saturating_add(1);
    let mut actors = alloc::vec::Vec::with_capacity(count as usize);
    for i in 0..count {
      let aaa_id = bench_create_system_manual::<T>(46_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::wakeup_substrate_schedule(aaa_id, wakeup_block));
      actors.push(aaa_id);
    }
    let middle_start = page_size as usize;
    let middle_end = middle_start.saturating_add(page_size as usize);
    for aaa_id in &actors[middle_start..middle_end.saturating_sub(1)] {
      assert!(Pallet::<T>::wakeup_substrate_invalidate(*aaa_id).is_some());
    }
    let aaa_id = actors[middle_end - 1];
    #[block]
    {
      assert!(Pallet::<T>::wakeup_substrate_invalidate(aaa_id).is_some());
    }
    assert!(!WakeupPages::<T>::contains_key((wakeup_block, 1)));
    assert_eq!(
      WakeupPages::<T>::get((wakeup_block, 0)).and_then(|page| page.next_page),
      Some(2)
    );
    assert_eq!(
      WakeupPages::<T>::get((wakeup_block, 2)).and_then(|page| page.previous_page),
      Some(0)
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_drain_partial_page() {
    let page_size = T::WakeupPageSize::get();
    assert!(page_size >= 2, "benchmark requires a partial page");
    let wakeup_block = 100u32.into();
    for i in 0..page_size {
      let aaa_id = bench_create_system_manual::<T>(47_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::wakeup_substrate_schedule(aaa_id, wakeup_block));
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
      WakeupPages::<T>::get((wakeup_block, 0)).map(|page| page.scan_slot),
      Some(scan_limit)
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_drain_full_page() {
    let page_size = T::WakeupPageSize::get();
    let wakeup_block = 100u32.into();
    for i in 0..page_size {
      let aaa_id = bench_create_system_manual::<T>(48_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::wakeup_substrate_schedule(aaa_id, wakeup_block));
    }
    #[block]
    {
      let (ready, stats) = Pallet::<T>::wakeup_substrate_drain_block(wakeup_block, page_size);
      assert_eq!(ready.len(), page_size as usize);
      assert_eq!(stats.entries_scanned, page_size);
      assert_eq!(stats.pages_deleted, 1);
    }
    assert!(!WakeupBuckets::<T>::contains_key(wakeup_block));
    assert!(!WakeupPages::<T>::contains_key((wakeup_block, 0)));
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
      let aaa_id = bench_create_system_manual::<T>(49_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::wakeup_substrate_schedule(aaa_id, wakeup_block));
    }
    #[block]
    {
      let (ready, stats) = Pallet::<T>::wakeup_substrate_drain_block(wakeup_block, count);
      assert_eq!(ready.len(), count as usize);
      assert_eq!(stats.entries_scanned, count);
      assert_eq!(stats.pages_touched, 2);
      assert_eq!(stats.pages_deleted, 2);
    }
    assert!(!WakeupBuckets::<T>::contains_key(wakeup_block));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_drain_stale_page() {
    let page_size = T::WakeupPageSize::get();
    let wakeup_block = 100u32.into();
    for i in 0..page_size {
      let aaa_id = bench_create_system_manual::<T>(50_000_000u32.saturating_add(i));
      assert!(Pallet::<T>::wakeup_substrate_schedule(aaa_id, wakeup_block));
      ActorHot::<T>::mutate(aaa_id, |maybe_hot| {
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
    assert!(!WakeupBuckets::<T>::contains_key(wakeup_block));
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
    WakeupCursorLen::<T>::put(insert_index);
    let inserted_block: BlockNumberFor<T> = 1u32.into();
    WakeupBuckets::<T>::insert(
      inserted_block,
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
    assert_eq!(WakeupCursorLen::<T>::get(), max_active);
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
    assert_eq!(WakeupCursorLen::<T>::get(), cursor_len.saturating_sub(1));
    assert_eq!(Pallet::<T>::wakeup_cursor_peek(), Some(1_000_001u32.into()));
    assert_eq!(
      WakeupBuckets::<T>::get(expected_min).and_then(|bucket| bucket.cursor_index),
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
    assert_eq!(WakeupCursorLen::<T>::get(), cursor_len.saturating_sub(1));
    assert_eq!(Pallet::<T>::wakeup_cursor_peek(), Some(1_000_000u32.into()));
    assert_eq!(
      WakeupBuckets::<T>::get(removed_block).and_then(|bucket| bucket.cursor_index),
      None
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_cursor_worker_partial() {
    let wakeup_block: BlockNumberFor<T> = 10u32.into();
    let first = bench_create_system_manual::<T>(34_100_000);
    let second = bench_create_system_manual::<T>(34_100_001);
    assert!(Pallet::<T>::wakeup_substrate_schedule(first, wakeup_block));
    assert!(Pallet::<T>::wakeup_substrate_schedule(second, wakeup_block));
    let limit = T::WeightInfo::scheduler_wakeup_cursor_worker_future()
      .saturating_add(Pallet::<T>::wakeup_cursor_drain_unit_weight_upper(false));
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(limit);
    #[block]
    {
      let stats = Pallet::<T>::drain_overdue_wakeups_cursor(wakeup_block, &mut meter);
      assert_eq!(stats.entries_scanned, 1);
      assert_eq!(stats.ready_entries, 1);
    }
    assert_eq!(
      WakeupBuckets::<T>::get(wakeup_block).map(|bucket| bucket.live_entries),
      Some(1)
    );
    assert_eq!(Pallet::<T>::wakeup_cursor_peek(), Some(wakeup_block));
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_cursor_worker_remove() {
    let cursor_len = T::MaxActiveActors::get();
    let wakeup_block = prepare_wakeup_cursor_repair::<T>(0);
    let aaa_id = bench_create_system_manual::<T>(34_200_000);
    let mut entries = WakeupPageEntriesOf::<T>::default();
    entries
      .try_push(Some(WakeupEntry { aaa_id }))
      .expect("one wakeup entry fits");
    WakeupPages::<T>::insert(
      (wakeup_block, 0),
      WakeupPage {
        entries,
        live_entries: 1,
        scan_slot: 0,
        previous_page: None,
        next_page: None,
      },
    );
    WakeupBuckets::<T>::mutate(wakeup_block, |maybe_bucket| {
      let bucket = maybe_bucket.as_mut().expect("cursor bucket exists");
      bucket.head_page = 0;
      bucket.tail_page = 0;
      bucket.next_page_id = 1;
      bucket.live_entries = 1;
    });
    ActorHot::<T>::mutate(aaa_id, |maybe_hot| {
      maybe_hot.as_mut().expect("actor hot state").wakeup_pointer = Some(WakeupPointer {
        block: wakeup_block,
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
    assert_eq!(WakeupCursorLen::<T>::get(), cursor_len.saturating_sub(1));
    assert_eq!(Pallet::<T>::wakeup_cursor_peek(), Some(1_000_001u32.into()));
    assert!(WakeupBuckets::<T>::get(wakeup_block).is_none());
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_wakeup_cursor_worker_future() {
    let wakeup_block: BlockNumberFor<T> = 1_000_000u32.into();
    let aaa_id = bench_create_system_manual::<T>(34_300_000);
    assert!(Pallet::<T>::wakeup_substrate_schedule(aaa_id, wakeup_block));
    let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
    #[block]
    {
      let stats = Pallet::<T>::drain_overdue_wakeups_cursor(10u32.into(), &mut meter);
      assert_eq!(stats.entries_scanned, 0);
    }
    assert_eq!(Pallet::<T>::wakeup_cursor_peek(), Some(wakeup_block));
    assert!(
      ActorHot::<T>::get(aaa_id)
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
    let aaa_id = bench_create_system_manual::<T>(36_000_000);
    assert!(Pallet::<T>::paged_enqueue(aaa_id));
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
          aaa_id: 37_000_000u64.saturating_add(ticket).saturating_add(offset),
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
          let aaa_id = 39_000_000u64.saturating_add(logical_ticket);
          if logical_ticket % 2 == 1 {
            let mut hot = hot_template.clone();
            hot.queue_ticket = Some(logical_ticket);
            ActorIdentities::<T>::insert(aaa_id, identity_template.clone());
            ActorHot::<T>::insert(aaa_id, hot);
          }
          QueueEntry {
            ticket: logical_ticket,
            aaa_id,
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
    let program_template = ActorProgram::<T>::get(template_id).expect("benchmark program template");
    let funding_template = ActorFunding::<T>::get(template_id).expect("benchmark funding template");
    ActorIdentities::<T>::remove(template_id);
    ActorHot::<T>::remove(template_id);
    ActorProgram::<T>::remove(template_id);
    ActorFunding::<T>::remove(template_id);

    let first_id = 41_000_000u64;
    for offset in 0..bounded {
      let aaa_id = first_id.saturating_add(u64::from(offset));
      let mut hot = hot_template.clone();
      let mut identity = identity_template.clone();
      identity.cycle_nonce = 0;
      hot.last_cycle_block = None;
      hot.pending_signal = true;
      hot.queue_ticket = None;
      ActorIdentities::<T>::insert(aaa_id, identity);
      ActorHot::<T>::insert(aaa_id, hot);
      ActorProgram::<T>::insert(aaa_id, program_template.clone());
      ActorFunding::<T>::insert(aaa_id, funding_template.clone());
      assert!(Pallet::<T>::paged_enqueue(aaa_id));
    }
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    #[block]
    {
      core::hint::black_box(Pallet::<T>::execute_cycle(Weight::MAX));
    }
    let executed = (0..bounded)
      .filter(|offset| {
        let aaa_id = first_id.saturating_add(u64::from(*offset));
        ActorIdentities::<T>::get(aaa_id).is_some_and(|identity| identity.cycle_nonce == 1)
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
    let program_template =
      ActorProgram::<T>::take(system_template_id).expect("System program template");
    let funding_template =
      ActorFunding::<T>::take(system_template_id).expect("System funding template");
    let first_id = 43_000_000u64;
    for offset in 0..bounded {
      let aaa_id = first_id.saturating_add(u64::from(offset));
      let is_user = offset % 2 != 0;
      let mut identity = if is_user {
        let owner: T::AccountId = account("mixed_user_owner", offset, 0);
        let template_id = bench_create_user::<T>(owner);
        let identity = ActorIdentities::<T>::take(template_id).expect("User identity template");
        ActorHot::<T>::remove(template_id);
        ActorProgram::<T>::remove(template_id);
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
      ActorIdentities::<T>::insert(aaa_id, identity);
      ActorHot::<T>::insert(aaa_id, hot);
      ActorProgram::<T>::insert(aaa_id, program_template.clone());
      ActorFunding::<T>::insert(aaa_id, funding_template.clone());
      assert!(Pallet::<T>::paged_enqueue(aaa_id));
    }
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    #[block]
    {
      core::hint::black_box(Pallet::<T>::execute_cycle(Weight::MAX));
    }
    let executed = (0..bounded)
      .filter(|offset| {
        let aaa_id = first_id.saturating_add(u64::from(*offset));
        ActorIdentities::<T>::get(aaa_id).is_some_and(|identity| identity.cycle_nonce == 1)
      })
      .count() as u32;
    // User fee collection may materialize one Fee Sink service obligation. At the
    // configured attempt ceiling that obligation consumes one pass slot while the
    // post-cutoff ticket remains ordered behind the measured cohort.
    let expected = bounded.min(T::MaxExecutionsPerBlock::get().saturating_sub(1));
    assert_eq!(
      executed, expected,
      "mixed canonical-FIFO benchmark must complete its bounded cohort"
    );
    let consumed = (0..bounded)
      .filter(|offset| {
        let aaa_id = first_id.saturating_add(u64::from(*offset));
        ActorHot::<T>::get(aaa_id).is_some_and(|hot| hot.queue_ticket.is_none())
      })
      .count() as u32;
    assert_eq!(consumed, expected);
  }

  #[benchmark(pov_mode = Measured)]
  fn continuation_suspend(s: Linear<0, 20>) {
    let aaa_id = bench_create_system_manual::<T>(50_000_000);
    install_continuation::<T>(aaa_id, s);
    let state = ContinuationStateStore::<T>::take(aaa_id).expect("benchmark continuation exists");
    ActorHot::<T>::mutate(aaa_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("benchmark actor hot state exists")
        .cycle_state = CycleState::Idle;
    });
    #[block]
    {
      Pallet::<T>::persist_continuation_suspension(aaa_id, 1, state, SuspensionReason::Temporary)
        .expect("benchmark suspension must persist");
    }
    assert!(ContinuationStateStore::<T>::contains_key(aaa_id));
  }

  #[benchmark(pov_mode = Measured)]
  fn continuation_retry() {
    let aaa_id = bench_create_system_manual::<T>(50_000_001);
    install_continuation::<T>(aaa_id, T::MaxOpeningSnapshotEntries::get());
    #[block]
    {
      core::hint::black_box(Pallet::<T>::begin_continuation_attempt(
        aaa_id,
        1,
        2u32.into(),
      ));
    }
    assert_eq!(
      ContinuationStateStore::<T>::get(aaa_id)
        .expect("benchmark continuation remains")
        .attempt,
      1
    );
  }

  #[benchmark(pov_mode = Measured)]
  fn continuation_complete() {
    let aaa_id = bench_create_system_manual::<T>(50_000_002);
    install_continuation::<T>(aaa_id, T::MaxOpeningSnapshotEntries::get());
    #[block]
    {
      Pallet::<T>::write_continuation_state(aaa_id, None)
        .expect("benchmark completion must clear continuation");
    }
    assert!(ContinuationStateStore::<T>::get(aaa_id).is_none());
    assert!(ActorHot::<T>::get(aaa_id).is_some_and(|hot| hot.cycle_state == CycleState::Idle));
  }

  #[benchmark]
  fn continuation_cancel() {
    let aaa_id = bench_create_system_manual::<T>(50_000_003);
    install_continuation::<T>(aaa_id, T::MaxOpeningSnapshotEntries::get());
    ActorHot::<T>::mutate(aaa_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("benchmark actor hot state exists")
        .pending_signal = true;
    });
    #[extrinsic_call]
    cancel_continuation(RawOrigin::Root, aaa_id);
    assert!(ContinuationStateStore::<T>::get(aaa_id).is_none());
    assert!(ActorHot::<T>::get(aaa_id).is_some_and(|hot| {
      hot.cycle_state == CycleState::Idle && hot.pending_signal && hot.queue_ticket.is_some()
    }));
  }

  #[benchmark]
  fn continuation_suffix_admission(n: Linear<1, 10>) {
    let bounded = n.min(T::MaxExecutionPlanSteps::get());
    let recipient: T::AccountId = account("continuation_suffix_recipient", 0, 0);
    let mut plan = ExecutionPlanOf::<T>::default();
    for _ in 0..bounded {
      plan
        .try_push(Step {
          conditions: ConditionSet::Always,
          task: AaaTask::Transfer {
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
          .saturating_add(Weight::from_parts(step.conditions.len() as u64, 0));
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
    for aaa_id in actors {
      assert!(ActorHot::<T>::get(aaa_id).is_some_and(|hot| hot.pending_signal));
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
    for aaa_id in actors {
      assert!(
        ActorHot::<T>::get(aaa_id)
          .is_some_and(|hot| { hot.pending_signal && hot.queue_ticket.is_none() })
      );
    }
  }

  #[benchmark]
  fn transaction_extension_ingress_base() {
    let owner: T::AccountId = whitelisted_caller();
    let populated_aaa_id = bench_create_user::<T>(owner);
    let proof_witness = Pallet::<T>::active_actor_view(populated_aaa_id)
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
    let (aaa_id, recipient) = prepare_saturated_address_actor::<T>(0);
    install_continuation::<T>(aaa_id, T::MaxOpeningSnapshotEntries::get());
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
      ActorHot::<T>::get(aaa_id)
        .is_some_and(|hot| { hot.pending_signal && hot.wakeup_pointer.is_some() })
    );
  }

  #[benchmark]
  fn funding_snapshot_open(a: Linear<1, 10>) {
    let owner: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(owner);
    let assets = T::BenchmarkHelper::funding_assets(a);
    ActorFunding::<T>::mutate(aaa_id, |maybe| {
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
      snapshot = ActorFunding::<T>::mutate(aaa_id, |maybe| {
        maybe
          .as_mut()
          .map(|funding| core::mem::take(&mut funding.funding_accumulated))
          .expect("benchmark actor funding exists")
      });
    }
    assert_eq!(snapshot.len() as u32, a);
    assert!(
      ActorFunding::<T>::get(aaa_id)
        .expect("benchmark actor funding exists")
        .funding_accumulated
        .is_empty()
    );
  }

  /// Builds a circular chain of `n` system AAAs where each transfers 1% of its
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
      trigger: Trigger::cadenced_always(1),
      cooldown_blocks: 0,
    };
    let mut sovereigns: alloc::vec::Vec<T::AccountId> = alloc::vec::Vec::with_capacity(n as usize);
    let mut aaa_ids: alloc::vec::Vec<AaaId> = alloc::vec::Vec::with_capacity(n as usize);
    for i in 0..n {
      let owner: T::AccountId = account("owner", i, 0);
      let temp_execution_plan = make_inert_execution_plan::<T>();
      Pallet::<T>::create_system_aaa(
        RawOrigin::Root.into(),
        owner,
        Mutability::Mutable,
        system_program::<T>(schedule.clone(), temp_execution_plan),
      )
      .expect("create_system_aaa must succeed");
      let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
      let sov = Pallet::<T>::sovereign_account_id_system(aaa_id);
      let _ = T::AssetOps::mint(&sov, native, initial_balance);
      sovereigns.push(sov);
      aaa_ids.push(aaa_id);
    }
    for (i, aaa_id) in aaa_ids.iter().enumerate() {
      let next_sov = sovereigns[(i + 1) % sovereigns.len()].clone();
      let transfer_execution_plan: ExecutionPlanOf<T> = BoundedVec::try_from(alloc::vec![Step {
        conditions: ConditionSet::Always,
        task: AaaTask::Transfer {
          to: next_sov,
          asset: native,
          amount: AmountResolution::PercentageOfCurrent(pct),
        },
        on_error: StepErrorPolicy::AbortCycle,
      }])
      .expect("transfer execution_plan fits");
      Pallet::<T>::update_execution_plan(
        RawOrigin::Root.into(),
        *aaa_id,
        transfer_execution_plan,
        CompletionPolicy::Persistent,
      )
      .expect("update_execution_plan must succeed");
    }
    let total_before: T::Balance = sovereigns
      .iter()
      .map(|sov| T::AssetOps::balance(sov, native))
      .fold(T::Balance::zero(), |acc, b| acc.saturating_add(b));
    for block in 2u32..=4 {
      frame_system::Pallet::<T>::set_block_number(block.into());
      let _ = Pallet::<T>::on_idle(block.into(), Weight::MAX);
    }
    // System AAAs don't pay fees → transfers are pure balance moves → zero drift
    let total_after: T::Balance = sovereigns
      .iter()
      .map(|sov| T::AssetOps::balance(sov, native))
      .fold(T::Balance::zero(), |acc, b| acc.saturating_add(b));
    assert_eq!(
      total_before, total_after,
      "Balance must be exactly conserved (System AAAs pay no fees)"
    );
    sovereigns
  }

  /// Parametric stress test: circular chain of n system AAAs.
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

  /// Extreme stress test request: 10K-100K AAA circular chain.
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
