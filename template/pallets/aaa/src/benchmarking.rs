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
use polkadot_sdk::sp_weights::WeightToFee;

#[benchmarks]
mod benches {
  use super::*;

  fn ensure_creation_balance<T: Config>(owner: &T::AccountId) {
    let creation_fee = T::AaaCreationFee::get();
    if creation_fee.is_zero() {
      return;
    }
    let amount = creation_fee.saturating_add(One::one());
    let _ = T::AssetOps::mint(owner, T::NativeAssetId::get(), amount);
  }

  fn cycle_fee_upper<T: Config>(execution_plan: &ExecutionPlanOf<T>) -> T::Balance {
    let mut total = T::Balance::zero();
    for step in execution_plan.iter() {
      let eval_fee = T::StepBaseFee::get().saturating_add(
        T::ConditionReadFee::get().saturating_mul((step.conditions.len() as u32).into()),
      );
      total = total.saturating_add(eval_fee);
      if !matches!(step.task, AaaTask::Noop) {
        let exec_fee = T::WeightToFee::weight_to_fee(&Pallet::<T>::weight_upper_bound(&step.task));
        total = total.saturating_add(exec_fee);
      }
    }
    total
  }

  fn make_execution_plan<T: Config>(recipient: T::AccountId) -> ExecutionPlanOf<T> {
    let step = Step {
      conditions: BoundedVec::default(),
      task: AaaTask::Transfer {
        to: recipient,
        asset: T::NativeAssetId::get(),
        amount: AmountResolution::AllBalance,
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    BoundedVec::try_from(vec![step]).expect("single-step execution_plan must fit")
  }

  fn make_last_funding_execution_plan<T: Config>(recipient: T::AccountId) -> ExecutionPlanOf<T> {
    let step = Step {
      conditions: BoundedVec::default(),
      task: AaaTask::Transfer {
        to: recipient,
        asset: T::NativeAssetId::get(),
        amount: AmountResolution::PercentageOfLastFunding(
          polkadot_sdk::sp_runtime::Perbill::from_percent(10),
        ),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    BoundedVec::try_from(vec![step]).expect("single-step execution_plan must fit")
  }

  fn make_remove_liquidity_execution_plan<T: Config>(
    lp_asset: T::AssetId,
    amount: T::Balance,
  ) -> ExecutionPlanOf<T> {
    let step = Step {
      conditions: BoundedVec::default(),
      task: AaaTask::RemoveLiquidity {
        lp_asset,
        amount: AmountResolution::Fixed(amount),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    BoundedVec::try_from(vec![step]).expect("single-step execution_plan must fit")
  }

  fn make_split_legs<T: Config>(legs: u32, seed: u32) -> SplitTransferLegsOf<T> {
    let bounded_legs = legs.max(2).min(T::MaxSplitTransferLegs::get());
    let share_parts = Perbill::ACCURACY / bounded_legs;
    let mut split_legs: alloc::vec::Vec<SplitLeg<T::AccountId>> = alloc::vec::Vec::new();
    for i in 0..bounded_legs {
      let recipient: T::AccountId =
        account("close-leg", seed.saturating_mul(1000).saturating_add(i), 0);
      split_legs.push(SplitLeg {
        to: recipient,
        share: Perbill::from_parts(share_parts),
      });
    }
    BoundedVec::try_from(split_legs).expect("split legs must fit benchmark bounds")
  }

  fn make_dense_conditions<T: Config>()
  -> BoundedVec<Condition<T::AssetId, T::Balance>, T::MaxConditionsPerStep> {
    let mut conditions: alloc::vec::Vec<Condition<T::AssetId, T::Balance>> = alloc::vec::Vec::new();
    conditions.push(Condition::BlockNumberAbove { threshold: 0 });
    conditions.push(Condition::BlockNumberBelow {
      threshold: u32::MAX,
    });
    conditions.push(Condition::BalanceAbove {
      asset: T::NativeAssetId::get(),
      threshold: Zero::zero(),
    });
    conditions.push(Condition::BalanceNotEquals {
      asset: T::NativeAssetId::get(),
      threshold: Zero::zero(),
    });
    let bounded = T::MaxConditionsPerStep::get() as usize;
    conditions.truncate(bounded);
    BoundedVec::try_from(conditions).expect("conditions must fit benchmark bounds")
  }

  fn make_complex_on_close_execution_plan<T: Config>(
    owner: &T::AccountId,
    steps: u32,
    legs: u32,
  ) -> ExecutionPlanOf<T> {
    let bounded_steps = steps.max(1).min(T::MaxSystemExecutionPlanSteps::get());
    let amount = T::MinUserBalance::get()
      .saturating_mul(100u32.into())
      .saturating_add(One::one());
    let mut plan: alloc::vec::Vec<StepOf<T>> = alloc::vec::Vec::new();
    for idx in 0..bounded_steps {
      let task = match idx % 3 {
        0 => AaaTask::SplitTransfer {
          asset: T::NativeAssetId::get(),
          amount: AmountResolution::Fixed(amount),
          legs: make_split_legs::<T>(legs, idx),
        },
        1 => AaaTask::Transfer {
          to: owner.clone(),
          asset: T::NativeAssetId::get(),
          amount: AmountResolution::Fixed(amount),
        },
        _ => AaaTask::Burn {
          asset: T::NativeAssetId::get(),
          amount: AmountResolution::Fixed(amount),
        },
      };
      plan.push(Step {
        conditions: make_dense_conditions::<T>(),
        task,
        on_error: StepErrorPolicy::ContinueNextStep,
      });
    }
    BoundedVec::try_from(plan).expect("on-close execution_plan must fit benchmark bounds")
  }

  fn seed_on_close_complexity_budget<T: Config>(aaa_id: AaaId, steps: u32, legs: u32) {
    let Some(instance) = AaaInstances::<T>::get(aaa_id) else {
      return;
    };
    let native = T::NativeAssetId::get();
    let amount = T::MinUserBalance::get()
      .saturating_mul(100u32.into())
      .saturating_add(One::one());
    let legs_cost = amount.saturating_mul(legs.max(2).into());
    let total_budget = amount
      .saturating_mul(steps.max(1).into())
      .saturating_add(legs_cost.saturating_mul(steps.max(1).into()))
      .saturating_add(T::MinUserBalance::get().saturating_mul(1_000u32.into()));
    let _ = T::AssetOps::mint(&instance.sovereign_account, native, total_budget);
    for step_idx in 0..steps.max(1).min(T::MaxSystemExecutionPlanSteps::get()) {
      for leg_idx in 0..legs.max(2).min(T::MaxSplitTransferLegs::get()) {
        let recipient: T::AccountId = account(
          "close-leg",
          step_idx.saturating_mul(1000).saturating_add(leg_idx),
          0,
        );
        let _ = T::AssetOps::mint(&recipient, native, One::one());
      }
    }
  }

  fn prefill_owner_slots_for_worst_case<T: Config>(owner: &T::AccountId) -> u8 {
    let max_slots = T::MaxOwnerSlots::get();
    assert!(max_slots > 0, "MaxOwnerSlots must be greater than zero");
    assert!(max_slots <= 8, "MaxOwnerSlots must fit in u8 bitmask");
    let target_slot = max_slots.saturating_sub(1);
    let occupied_mask = if target_slot == 0 {
      0
    } else {
      ((1u16 << target_slot) - 1) as u8
    };
    OwnerSlotMask::<T>::insert(owner.clone(), occupied_mask);
    target_slot
  }

  fn seed_actor_for_cycle<T: Config>(aaa_id: AaaId) {
    let Some(instance) = AaaInstances::<T>::get(aaa_id) else {
      return;
    };
    let reserve = cycle_fee_upper::<T>(&instance.execution_plan)
      .saturating_add(T::MinUserBalance::get())
      .saturating_add(One::one());
    let _ = T::AssetOps::mint(
      &instance.sovereign_account,
      T::NativeAssetId::get(),
      reserve,
    );
  }

  fn bench_create_user<T: Config>(caller: T::AccountId) -> AaaId {
    ensure_creation_balance::<T>(&caller);
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    let execution_plan = make_execution_plan::<T>(recipient);
    let schedule = Schedule {
      trigger: Trigger::Manual,
      cooldown_blocks: 10,
    };
    Pallet::<T>::create_user_aaa(
      RawOrigin::Signed(caller).into(),
      Mutability::Mutable,
      schedule,
      None,
      execution_plan,
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
    let schedule = Schedule {
      trigger: Trigger::Manual,
      cooldown_blocks: 10,
    };
    #[extrinsic_call]
    create_user_aaa(
      RawOrigin::Signed(caller),
      Mutability::Mutable,
      schedule,
      None,
      execution_plan,
    );
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let inst = AaaInstances::<T>::get(aaa_id).expect("AAA must exist after create_user_aaa");
    assert_eq!(inst.owner_slot, expected_slot);
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
    let schedule = Schedule {
      trigger: Trigger::Manual,
      cooldown_blocks: 10,
    };
    #[extrinsic_call]
    create_user_aaa_at_slot(
      RawOrigin::Signed(caller),
      requested_slot,
      Mutability::Mutable,
      schedule,
      None,
      execution_plan,
    );
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let inst =
      AaaInstances::<T>::get(aaa_id).expect("AAA must exist after create_user_aaa_at_slot");
    assert_eq!(inst.owner_slot, requested_slot);
  }

  #[benchmark]
  fn create_system_aaa() {
    let owner: T::AccountId = whitelisted_caller();
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    let execution_plan = make_execution_plan::<T>(recipient);
    let schedule = Schedule {
      trigger: Trigger::Manual,
      cooldown_blocks: 100,
    };
    #[extrinsic_call]
    create_system_aaa(RawOrigin::Root, owner, schedule, None, execution_plan);
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let inst = AaaInstances::<T>::get(aaa_id).expect("AAA must exist after create_system_aaa");
    assert_eq!(inst.owner_slot, SYSTEM_OWNER_SLOT_SENTINEL);
  }

  #[benchmark]
  fn reopen_system_aaa() {
    let owner: T::AccountId = whitelisted_caller();
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    let execution_plan = make_execution_plan::<T>(recipient.clone());
    let schedule = Schedule {
      trigger: Trigger::Manual,
      cooldown_blocks: 100,
    };
    Pallet::<T>::create_system_aaa(
      RawOrigin::Root.into(),
      owner.clone(),
      schedule.clone(),
      None,
      execution_plan.clone(),
    )
    .expect("create_system_aaa must succeed in benchmark setup");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    Pallet::<T>::close_aaa(RawOrigin::Root.into(), aaa_id)
      .expect("close_aaa must succeed in benchmark setup");
    #[extrinsic_call]
    reopen_system_aaa(
      RawOrigin::Root,
      aaa_id,
      owner,
      schedule,
      None,
      execution_plan,
    );
    let inst = AaaInstances::<T>::get(aaa_id).expect("AAA must exist after reopen_system_aaa");
    assert_eq!(inst.aaa_id, aaa_id);
  }

  #[benchmark]
  fn pause_aaa() {
    let caller: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(caller.clone());
    #[extrinsic_call]
    pause_aaa(RawOrigin::Signed(caller), aaa_id);
    let inst = AaaInstances::<T>::get(aaa_id).expect("AAA must exist after pause_aaa");
    assert!(inst.is_paused);
  }

  #[benchmark]
  fn resume_aaa() {
    let caller: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(caller.clone());
    Pallet::<T>::pause_aaa(RawOrigin::Signed(caller.clone()).into(), aaa_id)
      .expect("pause_aaa must succeed in setup");
    #[extrinsic_call]
    resume_aaa(RawOrigin::Signed(caller), aaa_id);
    let inst = AaaInstances::<T>::get(aaa_id).expect("AAA must exist after resume_aaa");
    assert!(!inst.is_paused);
  }

  #[benchmark]
  fn manual_trigger() {
    let caller: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(caller.clone());
    #[extrinsic_call]
    manual_trigger(RawOrigin::Signed(caller), aaa_id);
    let inst = AaaInstances::<T>::get(aaa_id).expect("AAA must exist after manual_trigger");
    assert!(inst.manual_trigger_pending);
  }

  #[benchmark]
  fn fund_aaa() {
    let caller: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&caller);
    let recipient =
      T::AccountId::decode(&mut polkadot_sdk::sp_runtime::traits::TrailingZeroInput::zeroes())
        .expect("decode zero account");
    let schedule = Schedule {
      trigger: Trigger::Manual,
      cooldown_blocks: 10,
    };
    let execution_plan = make_last_funding_execution_plan::<T>(recipient);
    Pallet::<T>::create_user_aaa(
      RawOrigin::Signed(caller.clone()).into(),
      Mutability::Mutable,
      schedule,
      None,
      execution_plan,
    )
    .expect("create_user_aaa must succeed in fund_aaa benchmark setup");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let amount = T::MinUserBalance::get().saturating_add(One::one());
    T::AssetOps::mint(&caller, T::NativeAssetId::get(), amount)
      .expect("mint for fund_aaa benchmark must succeed");
    #[extrinsic_call]
    fund_aaa(
      RawOrigin::Signed(caller),
      aaa_id,
      T::NativeAssetId::get(),
      amount,
    );
    assert!(AaaInstances::<T>::get(aaa_id).is_some());
  }

  #[benchmark]
  fn close_aaa() {
    let owner: T::AccountId = whitelisted_caller();
    let recipient: T::AccountId = account("close-recipient", 0, 0);
    let schedule = Schedule {
      trigger: Trigger::Manual,
      cooldown_blocks: 1,
    };
    let execution_plan = make_execution_plan::<T>(recipient);
    Pallet::<T>::create_system_aaa(
      RawOrigin::Root.into(),
      owner.clone(),
      schedule,
      None,
      execution_plan,
    )
    .expect("create_system_aaa must succeed in close_aaa benchmark setup");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let max_steps = T::MaxSystemExecutionPlanSteps::get();
    let max_legs = T::MaxSplitTransferLegs::get();
    AaaInstances::<T>::mutate(aaa_id, |maybe| {
      let Some(instance) = maybe.as_mut() else {
        return;
      };
      instance.on_close_execution_plan =
        make_complex_on_close_execution_plan::<T>(&owner, max_steps, max_legs);
    });
    seed_on_close_complexity_budget::<T>(aaa_id, max_steps, max_legs);
    #[extrinsic_call]
    close_aaa(RawOrigin::Root, aaa_id);
    assert!(AaaInstances::<T>::get(aaa_id).is_none());
  }

  #[benchmark]
  fn update_schedule() {
    let caller: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(caller.clone());
    let new_schedule = Schedule {
      trigger: Trigger::Manual,
      cooldown_blocks: 20,
    };
    #[extrinsic_call]
    update_schedule(RawOrigin::Signed(caller), aaa_id, new_schedule, None);
    let inst = AaaInstances::<T>::get(aaa_id).expect("AAA must exist after update_schedule");
    assert_eq!(inst.schedule.cooldown_blocks, 20);
  }

  #[benchmark]
  fn update_execution_plan() {
    let caller: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(caller.clone());
    let recipient = account("recipient", 0, 0);
    let replacement = make_execution_plan::<T>(recipient);
    #[extrinsic_call]
    update_execution_plan(RawOrigin::Signed(caller), aaa_id, replacement.clone());
    let inst = AaaInstances::<T>::get(aaa_id).expect("AAA must exist after update_execution_plan");
    assert_eq!(inst.execution_plan, replacement);
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
    assert!(AaaInstances::<T>::get(aaa_id).is_some());
  }

  #[benchmark]
  fn permissionless_sweep_many(n: Linear<1, 3>) {
    let caller: T::AccountId = whitelisted_caller();
    let mut aaa_ids: BoundedVec<AaaId, T::MaxSweepPerBlock> = BoundedVec::default();
    let schedule = Schedule {
      trigger: Trigger::Manual,
      cooldown_blocks: 10,
    };
    for i in 0..n {
      let owner: T::AccountId = account("sweep-owner", i, 0);
      let recipient: T::AccountId = account("sweep-recipient", i, 0);
      ensure_creation_balance::<T>(&owner);
      let execution_plan = make_execution_plan::<T>(recipient);
      Pallet::<T>::create_user_aaa(
        RawOrigin::Signed(owner).into(),
        Mutability::Mutable,
        schedule.clone(),
        None,
        execution_plan,
      )
      .expect("create_user_aaa must succeed in permissionless_sweep_many setup");
      let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
      aaa_ids
        .try_push(aaa_id)
        .expect("benchmark n must fit MaxSweepPerBlock");
    }
    let expected_len = aaa_ids.len();
    #[extrinsic_call]
    permissionless_sweep_many(RawOrigin::Signed(caller), aaa_ids.clone());
    for aaa_id in aaa_ids {
      assert!(AaaInstances::<T>::get(aaa_id).is_none());
    }
    assert_eq!(expected_len, n as usize);
  }

  // Non-dispatch diagnostic benchmark excluded from runtime weight artifact generation
  #[benchmark]
  fn close_aaa_on_close_execution_plan_complex(s: Linear<1, 10>, l: Linear<2, 8>) {
    let owner: T::AccountId = whitelisted_caller();
    let recipient: T::AccountId = account("diag-close-recipient", 0, 0);
    let schedule = Schedule {
      trigger: Trigger::Manual,
      cooldown_blocks: 1,
    };
    let execution_plan = make_execution_plan::<T>(recipient);
    Pallet::<T>::create_system_aaa(
      RawOrigin::Root.into(),
      owner.clone(),
      schedule,
      None,
      execution_plan,
    )
    .expect("create_system_aaa must succeed in diagnostic setup");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let steps = s.min(T::MaxSystemExecutionPlanSteps::get());
    let legs = l.min(T::MaxSplitTransferLegs::get());
    AaaInstances::<T>::mutate(aaa_id, |maybe| {
      let Some(instance) = maybe.as_mut() else {
        return;
      };
      instance.on_close_execution_plan =
        make_complex_on_close_execution_plan::<T>(&owner, steps, legs);
    });
    seed_on_close_complexity_budget::<T>(aaa_id, steps, legs);
    #[block]
    {
      Pallet::<T>::close_aaa(RawOrigin::Root.into(), aaa_id)
        .expect("close_aaa must succeed in diagnostic benchmark");
    }
    assert!(AaaInstances::<T>::get(aaa_id).is_none());
  }

  // Non-dispatch diagnostic benchmark excluded from runtime weight artifact generation
  #[benchmark]
  fn close_aaa_user_fee_bearing_tail(s: Linear<1, 3>, l: Linear<2, 8>) {
    let owner: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&owner);
    let recipient: T::AccountId = account("diag-user-close-recipient", 0, 0);
    let schedule = Schedule {
      trigger: Trigger::Manual,
      cooldown_blocks: 1,
    };
    let execution_plan = make_execution_plan::<T>(recipient);
    Pallet::<T>::create_user_aaa(
      RawOrigin::Signed(owner.clone()).into(),
      Mutability::Mutable,
      schedule,
      None,
      execution_plan,
    )
    .expect("create_user_aaa must succeed in diagnostic setup");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let steps = s.min(T::MaxUserExecutionPlanSteps::get());
    let legs = l.min(T::MaxSplitTransferLegs::get());
    AaaInstances::<T>::mutate(aaa_id, |maybe| {
      let Some(instance) = maybe.as_mut() else {
        return;
      };
      instance.on_close_execution_plan =
        make_complex_on_close_execution_plan::<T>(&owner, steps, legs);
    });
    seed_on_close_complexity_budget::<T>(aaa_id, steps, legs);
    #[block]
    {
      Pallet::<T>::close_aaa(RawOrigin::Signed(owner).into(), aaa_id)
        .expect("close_aaa must succeed in user fee-bearing diagnostic benchmark");
    }
    assert!(AaaInstances::<T>::get(aaa_id).is_none());
  }

  // Non-dispatch diagnostic benchmark excluded from runtime weight artifact generation
  #[benchmark]
  fn process_remove_liquidity_max_k() {
    let caller: T::AccountId = whitelisted_caller();
    ensure_creation_balance::<T>(&caller);
    let max_scan = T::MaxAdapterScan::get();
    assert!(max_scan > 0, "MaxAdapterScan must be greater than zero");
    let (lp_asset, lp_amount) = T::BenchmarkHelper::setup_remove_liquidity_max_k(&caller, max_scan)
      .expect("benchmark helper must prepare remove-liquidity worst-case state");
    let schedule = Schedule {
      trigger: Trigger::Manual,
      cooldown_blocks: 10,
    };
    let execution_plan = make_remove_liquidity_execution_plan::<T>(lp_asset, lp_amount);
    Pallet::<T>::create_user_aaa(
      RawOrigin::Signed(caller.clone()).into(),
      Mutability::Mutable,
      schedule,
      None,
      execution_plan,
    )
    .expect("create_user_aaa must succeed in setup");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let actor = AaaInstances::<T>::get(aaa_id)
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
    let inst = AaaInstances::<T>::get(aaa_id).expect("actor must survive benchmark cycle");
    assert_eq!(inst.cycle_nonce, 1);
    assert_eq!(inst.consecutive_failures, 0);
  }

  fn make_noop_execution_plan<T: Config>() -> ExecutionPlanOf<T> {
    let step = Step {
      conditions: BoundedVec::default(),
      task: AaaTask::Noop,
      on_error: StepErrorPolicy::AbortCycle,
    };
    BoundedVec::try_from(vec![step]).expect("single-step execution_plan must fit")
  }

  fn bench_create_system_manual<T: Config>(seed: u32) -> AaaId {
    let owner: T::AccountId = account("wakeup_owner", seed, 0);
    let schedule = Schedule {
      trigger: Trigger::Manual,
      cooldown_blocks: 0,
    };
    let execution_plan = make_noop_execution_plan::<T>();
    Pallet::<T>::create_system_aaa(
      RawOrigin::Root.into(),
      owner,
      schedule,
      None,
      execution_plan,
    )
    .expect("create_system_aaa must succeed in wakeup benchmark setup");
    NextAaaId::<T>::get().saturating_sub(1)
  }

  fn fill_wakeup_bucket<T: Config>(block: BlockNumberFor<T>, count: u32, seed: u64) {
    let bounded = count.min(T::MaxWakeupBucketSize::get());
    let mut ids: alloc::vec::Vec<AaaId> = alloc::vec::Vec::with_capacity(bounded as usize);
    for i in 0..bounded {
      ids.push(seed.saturating_add(u64::from(i)));
    }
    WakeupIndex::<T>::insert(
      block,
      BoundedVec::<AaaId, T::MaxWakeupBucketSize>::try_from(ids)
        .expect("wakeup bucket must fit benchmark bounds"),
    );
  }

  fn setup_scan_only_manual_actors<T: Config>(
    requested_n: u32,
    clear_readiness: bool,
  ) -> alloc::vec::Vec<AaaId> {
    let existing_active = AaaInstances::<T>::iter_keys().count() as u32;
    let available = T::MaxActiveActors::get().saturating_sub(existing_active);
    let n = requested_n.min(available);
    assert!(
      n > 0,
      "benchmark requires at least one available active slot"
    );
    let schedule = Schedule {
      trigger: Trigger::Manual,
      cooldown_blocks: 0,
    };
    let execution_plan = make_noop_execution_plan::<T>();
    let mut aaa_ids: alloc::vec::Vec<AaaId> = alloc::vec::Vec::with_capacity(n as usize);
    for i in 0..n {
      let owner: T::AccountId = account("scan_owner", i, 0);
      Pallet::<T>::create_system_aaa(
        RawOrigin::Root.into(),
        owner,
        schedule.clone(),
        None,
        execution_plan.clone(),
      )
      .expect("create_system_aaa must succeed");
      let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
      aaa_ids.push(aaa_id);
    }
    if clear_readiness {
      for &aaa_id in &aaa_ids {
        AaaReadiness::<T>::remove(aaa_id);
      }
    }
    aaa_ids
  }

  fn setup_scan_only_manual_sparse_actors<T: Config>(
    requested_n: u32,
    stride: u64,
  ) -> alloc::vec::Vec<AaaId> {
    let existing_active = AaaInstances::<T>::iter_keys().count() as u32;
    let available = T::MaxActiveActors::get().saturating_sub(existing_active);
    let n = requested_n.min(available);
    let effective_stride = stride.max(2);
    assert!(
      n > 0,
      "benchmark requires at least one available active slot"
    );
    let schedule = Schedule {
      trigger: Trigger::Manual,
      cooldown_blocks: 0,
    };
    let execution_plan = make_noop_execution_plan::<T>();
    let mut aaa_ids: alloc::vec::Vec<AaaId> = alloc::vec::Vec::with_capacity(n as usize);
    for i in 0..n {
      let owner: T::AccountId = account("scan_sparse_owner", i, 0);
      Pallet::<T>::create_system_aaa(
        RawOrigin::Root.into(),
        owner,
        schedule.clone(),
        None,
        execution_plan.clone(),
      )
      .expect("create_system_aaa must succeed");
      let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
      aaa_ids.push(aaa_id);
      let bumped_next = aaa_id.saturating_add(effective_stride);
      NextAaaId::<T>::put(bumped_next);
    }
    aaa_ids
  }

  // Non-dispatch diagnostic benchmark for proof-size decomposition of scheduler scan path
  #[benchmark]
  fn scheduler_scan_hot_readiness(n: Linear<100, 1_000>) {
    let aaa_ids = setup_scan_only_manual_actors::<T>(n, false);
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    #[block]
    {
      let _ = Pallet::<T>::execute_cycle(Weight::MAX);
    }
    for aaa_id in aaa_ids.into_iter().take(4) {
      assert!(AaaReadiness::<T>::contains_key(aaa_id));
    }
  }

  // Non-dispatch diagnostic benchmark for fallback path when compact readiness state is missing
  #[benchmark]
  fn scheduler_scan_fallback_readiness(n: Linear<100, 1_000>) {
    let aaa_ids = setup_scan_only_manual_actors::<T>(n, true);
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    #[block]
    {
      let _ = Pallet::<T>::execute_cycle(Weight::MAX);
    }
    for aaa_id in aaa_ids.into_iter().take(4) {
      assert!(!AaaReadiness::<T>::contains_key(aaa_id));
    }
  }

  // Non-dispatch diagnostic benchmark for sparse-id topology in active actor set
  #[benchmark]
  fn scheduler_scan_sparse_hot_readiness(n: Linear<100, 1_000>) {
    let aaa_ids = setup_scan_only_manual_sparse_actors::<T>(n, 16);
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    #[block]
    {
      let _ = Pallet::<T>::execute_cycle(Weight::MAX);
    }
    for aaa_id in aaa_ids.into_iter().take(4) {
      assert!(AaaReadiness::<T>::contains_key(aaa_id));
    }
  }

  // Non-dispatch diagnostic benchmark for dense overdue wakeup admission.
  #[benchmark]
  fn scheduler_wakeup_dense_due_drain(n: Linear<1, 64>) {
    let due = n
      .min(T::MaxWakeupsPerBlock::get())
      .min(T::MaxWakeupBucketSize::get());
    let due_block: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(due_block);
    fill_wakeup_bucket::<T>(due_block, due, 9_000_000);
    MinWakeupBlock::<T>::put(due_block);
    #[block]
    {
      let _ = Pallet::<T>::execute_cycle(Weight::MAX);
    }
    assert!(WakeupIndex::<T>::get(due_block).is_empty());
  }

  // Non-dispatch diagnostic benchmark for bounded sparse-gap recovery after long halts.
  #[benchmark]
  fn scheduler_wakeup_sparse_gap_recovery(g: Linear<64, 4_096>) {
    let gap = g.max(T::MaxWakeupsPerBlock::get());
    let min_block: BlockNumberFor<T> = 1u32.into();
    let now: BlockNumberFor<T> = gap.saturating_add(1).into();
    let expected: BlockNumberFor<T> = T::MaxWakeupsPerBlock::get().saturating_add(1).into();
    MinWakeupBlock::<T>::put(min_block);
    frame_system::Pallet::<T>::set_block_number(now);
    #[block]
    {
      let _ = Pallet::<T>::execute_cycle(Weight::MAX);
    }
    assert_eq!(MinWakeupBlock::<T>::get(), Some(expected));
  }

  // Non-dispatch diagnostic benchmark for bounded spillover probing in WakeupIndex.
  #[benchmark]
  fn scheduler_wakeup_spillover_probe(b: Linear<0, 9>) {
    let aaa_id = bench_create_system_manual::<T>(b);
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    let blocked_buckets = b.min(9);
    let next_queue_fill = T::MaxQueueInsertionsPerBlock::get().min(T::MaxQueueLength::get());
    let next_queue_ids: alloc::vec::Vec<AaaId> =
      (20_000_000..20_000_000u64.saturating_add(u64::from(next_queue_fill))).collect();
    NextQueue::<T>::put(
      BoundedVec::<AaaId, T::MaxQueueLength>::try_from(next_queue_ids)
        .expect("next queue preload must fit benchmark bounds"),
    );
    for offset in 0..blocked_buckets {
      let block: BlockNumberFor<T> = (2u32.saturating_add(offset)).into();
      fill_wakeup_bucket::<T>(
        block,
        T::MaxWakeupBucketSize::get(),
        10_000_000 + u64::from(offset) * 100_000,
      );
    }
    #[block]
    {
      Pallet::<T>::enqueue(aaa_id);
    }
    if blocked_buckets < 9 {
      let scheduled_block: BlockNumberFor<T> = (2u32.saturating_add(blocked_buckets)).into();
      assert!(WakeupIndex::<T>::get(scheduled_block).contains(&aaa_id));
      assert_eq!(WakeupScheduleDrops::<T>::get(), 0);
    } else {
      assert_eq!(WakeupScheduleDrops::<T>::get(), 1);
    }
  }

  /// Builds a circular chain of `n` system AAAs where each transfers 1% of its
  /// NTVE balance to the next in ring, then runs 3 blocks and asserts zero drift.
  pub(super) fn setup_and_run_circular_chain<T: Config>(
    requested_n: u32,
  ) -> alloc::vec::Vec<T::AccountId> {
    let existing_active = AaaInstances::<T>::iter_keys().count() as u32;
    let available = T::MaxActiveActors::get().saturating_sub(existing_active);
    let n = requested_n.min(available);
    assert!(
      n > 0,
      "benchmark requires at least one available active slot"
    );
    let pct = polkadot_sdk::sp_runtime::Perbill::from_percent(1);
    let initial_balance = T::MinUserBalance::get().saturating_mul(1_000_000u32.into());
    let native = T::NativeAssetId::get();
    let schedule = Schedule {
      trigger: Trigger::Timer {
        every_blocks: 1,
        probability: None,
      },
      cooldown_blocks: 0,
    };
    let mut sovereigns: alloc::vec::Vec<T::AccountId> = alloc::vec::Vec::with_capacity(n as usize);
    let mut aaa_ids: alloc::vec::Vec<AaaId> = alloc::vec::Vec::with_capacity(n as usize);
    for i in 0..n {
      let owner: T::AccountId = account("owner", i, 0);
      let temp_execution_plan: ExecutionPlanOf<T> = BoundedVec::try_from(alloc::vec![Step {
        conditions: BoundedVec::default(),
        task: AaaTask::Noop,
        on_error: StepErrorPolicy::AbortCycle,
      }])
      .expect("temp execution_plan fits");
      Pallet::<T>::create_system_aaa(
        RawOrigin::Root.into(),
        owner,
        schedule.clone(),
        None,
        temp_execution_plan,
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
        conditions: BoundedVec::default(),
        task: AaaTask::Transfer {
          to: next_sov,
          asset: native,
          amount: AmountResolution::PercentageOfCurrent(pct),
        },
        on_error: StepErrorPolicy::AbortCycle,
      }])
      .expect("transfer execution_plan fits");
      Pallet::<T>::update_execution_plan(RawOrigin::Root.into(), *aaa_id, transfer_execution_plan)
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
