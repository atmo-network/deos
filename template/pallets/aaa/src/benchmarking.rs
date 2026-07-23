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

  fn user_program<T: Config>(
    schedule: ScheduleOf<T>,
    execution_plan: ExecutionPlanOf<T>,
  ) -> ProgramInputOf<T> {
    ProgramInput::Active {
      schedule,
      schedule_window: None,
      execution_plan,
      on_close_execution_plan: Default::default(),
      funding_source_policy: FundingSourcePolicy::OwnerOnly,
    }
  }

  fn system_program<T: Config>(
    schedule: ScheduleOf<T>,
    execution_plan: ExecutionPlanOf<T>,
  ) -> ProgramInputOf<T> {
    ProgramInput::Active {
      schedule,
      schedule_window: None,
      execution_plan,
      on_close_execution_plan: Default::default(),
      funding_source_policy: FundingSourcePolicy::RuntimePolicy,
    }
  }

  fn cycle_fee_upper<T: Config>(execution_plan: &ExecutionPlanOf<T>) -> T::Balance {
    let mut total = T::Balance::zero();
    for step in execution_plan.iter() {
      let eval_fee = T::StepBaseFee::get().saturating_add(
        T::ConditionReadFee::get().saturating_mul((step.conditions.len() as u32).into()),
      );
      total = total.saturating_add(eval_fee);
      let exec_fee = T::WeightToFee::weight_to_fee(&Pallet::<T>::weight_upper_bound(&step.task));
      total = total.saturating_add(exec_fee);
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

  fn make_tracked_funding_execution_plan<T: Config>(recipient: T::AccountId) -> ExecutionPlanOf<T> {
    BoundedVec::try_from(vec![Step {
      conditions: BoundedVec::default(),
      task: AaaTask::Transfer {
        to: recipient,
        asset: T::NativeAssetId::get(),
        amount: AmountResolution::PercentageOfLastFunding(polkadot_sdk::sp_runtime::Perbill::one()),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }])
    .expect("single-step tracked funding plan must fit")
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
    let Some(instance) = Pallet::<T>::active_actor_snapshot(aaa_id) else {
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
    let Some(instance) = Pallet::<T>::active_actor_snapshot(aaa_id) else {
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
    let schedule = Schedule {
      trigger: Trigger::Manual,
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
      Pallet::<T>::active_actor_snapshot(aaa_id).expect("AAA must exist after create_user_aaa");
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
    let schedule = Schedule {
      trigger: Trigger::Manual,
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
    let inst = Pallet::<T>::active_actor_snapshot(aaa_id)
      .expect("AAA must exist after create_user_aaa_at_slot");
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
      trigger: Trigger::Manual,
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
      Pallet::<T>::active_actor_snapshot(aaa_id).expect("AAA must exist after create_system_aaa");
    assert_eq!(inst.actor_class, ActorClass::System);
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
      Mutability::Mutable,
      system_program::<T>(schedule.clone(), execution_plan.clone()),
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
      Mutability::Mutable,
      system_program::<T>(schedule, execution_plan),
    );
    assert!(Pallet::<T>::active_actor_exists(aaa_id));
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
    assert!(DormantAaaIdentities::<T>::contains_key(aaa_id));
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
        trigger: Trigger::Manual,
        cooldown_blocks: 100,
      },
      make_execution_plan::<T>(recipient),
    );
    #[extrinsic_call]
    activate_aaa(RawOrigin::Signed(owner), aaa_id, program);
    assert!(Pallet::<T>::active_actor_exists(aaa_id));
    assert!(!DormantAaaIdentities::<T>::contains_key(aaa_id));
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
          trigger: Trigger::Manual,
          cooldown_blocks: 100,
        },
        execution_plan,
      ),
    )
    .expect("System AAA creation must succeed");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    #[extrinsic_call]
    deactivate_aaa(RawOrigin::Signed(owner), aaa_id);
    assert!(!Pallet::<T>::active_actor_exists(aaa_id));
    assert!(DormantAaaIdentities::<T>::contains_key(aaa_id));
  }

  #[benchmark]
  fn pause_aaa() {
    let caller: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(caller.clone());
    #[extrinsic_call]
    pause_aaa(RawOrigin::Signed(caller), aaa_id);
    let inst = Pallet::<T>::active_actor_snapshot(aaa_id).expect("AAA must exist after pause_aaa");
    assert!(inst.lifecycle.is_paused());
  }

  #[benchmark]
  fn resume_aaa() {
    let caller: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(caller.clone());
    Pallet::<T>::pause_aaa(RawOrigin::Signed(caller.clone()).into(), aaa_id)
      .expect("pause_aaa must succeed in setup");
    #[extrinsic_call]
    resume_aaa(RawOrigin::Signed(caller), aaa_id);
    let inst = Pallet::<T>::active_actor_snapshot(aaa_id).expect("AAA must exist after resume_aaa");
    assert!(!inst.lifecycle.is_paused());
  }

  #[benchmark]
  fn manual_trigger() {
    let caller: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(caller.clone());
    #[extrinsic_call]
    manual_trigger(RawOrigin::Signed(caller), aaa_id);
    let inst =
      Pallet::<T>::active_actor_snapshot(aaa_id).expect("AAA must exist after manual_trigger");
    assert!(inst.manual_trigger_pending);
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
      Mutability::Mutable,
      system_program::<T>(schedule, execution_plan),
    )
    .expect("create_system_aaa must succeed in close_aaa benchmark setup");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let max_steps = T::MaxSystemExecutionPlanSteps::get();
    let max_legs = T::MaxSplitTransferLegs::get();
    ActorProgram::<T>::mutate(aaa_id, |maybe| {
      let program = maybe.as_mut().expect("benchmark actor program exists");
      program.on_close_execution_plan =
        make_complex_on_close_execution_plan::<T>(&owner, max_steps, max_legs);
    });
    seed_on_close_complexity_budget::<T>(aaa_id, max_steps, max_legs);
    #[extrinsic_call]
    close_aaa(RawOrigin::Root, aaa_id);
    assert!(!Pallet::<T>::active_actor_exists(aaa_id));
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
    let inst =
      Pallet::<T>::active_actor_snapshot(aaa_id).expect("AAA must exist after update_schedule");
    assert_eq!(inst.schedule.cooldown_blocks, 20);
  }

  #[benchmark]
  fn update_funding_source_policy() {
    let caller: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(caller.clone());
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
    let funding_assets = T::BenchmarkHelper::funding_assets(T::MaxFundingTrackedAssets::get());
    ActorFunding::<T>::mutate(aaa_id, |maybe| {
      let funding = maybe.as_mut().expect("benchmark actor funding exists");
      for asset in funding_assets {
        funding
          .funding_snapshots
          .try_insert(
            asset,
            FundingBatch {
              amount: One::one(),
              pending_amount: One::one(),
            },
          )
          .expect("funding snapshot benchmark bound fits");
      }
    });
    let recipient = account("recipient", 0, 0);
    let replacement = make_execution_plan::<T>(recipient);
    #[extrinsic_call]
    update_execution_plan(RawOrigin::Signed(caller), aaa_id, replacement.clone());
    let inst = Pallet::<T>::active_actor_snapshot(aaa_id)
      .expect("AAA must exist after update_execution_plan");
    assert_eq!(inst.execution_plan, replacement);
    assert!(
      ActorFunding::<T>::get(aaa_id)
        .expect("actor funding exists")
        .funding_snapshots
        .is_empty()
    );
  }

  #[benchmark]
  fn update_on_close_execution_plan() {
    let caller: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(caller.clone());
    let replacement = make_inert_execution_plan::<T>();
    #[extrinsic_call]
    update_on_close_execution_plan(RawOrigin::Signed(caller), aaa_id, replacement.clone());
    let inst =
      Pallet::<T>::active_actor_snapshot(aaa_id).expect("AAA must exist after close-plan update");
    assert_eq!(inst.on_close_execution_plan, replacement);
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
    let mut aaa_ids: BoundedVec<AaaId, T::MaxSweepPerBlock> = BoundedVec::default();
    let schedule = Schedule {
      trigger: Trigger::Manual,
      cooldown_blocks: 10,
    };
    let bounded_n = n.min(T::MaxSweepPerBlock::get());
    for i in 0..bounded_n {
      let owner: T::AccountId = account("sweep-owner", i, 0);
      let recipient: T::AccountId = account("sweep-recipient", i, 0);
      ensure_creation_balance::<T>(&owner);
      let execution_plan = make_execution_plan::<T>(recipient);
      Pallet::<T>::create_user_aaa(
        RawOrigin::Signed(owner).into(),
        Mutability::Mutable,
        user_program::<T>(schedule.clone(), execution_plan),
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
      assert!(!Pallet::<T>::active_actor_exists(aaa_id));
    }
    assert_eq!(expected_len, bounded_n as usize);
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
      Mutability::Mutable,
      system_program::<T>(schedule, execution_plan),
    )
    .expect("create_system_aaa must succeed in diagnostic setup");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let steps = s.min(T::MaxSystemExecutionPlanSteps::get());
    let legs = l.min(T::MaxSplitTransferLegs::get());
    ActorProgram::<T>::mutate(aaa_id, |maybe| {
      let program = maybe.as_mut().expect("benchmark actor program exists");
      program.on_close_execution_plan =
        make_complex_on_close_execution_plan::<T>(&owner, steps, legs);
    });
    seed_on_close_complexity_budget::<T>(aaa_id, steps, legs);
    #[block]
    {
      Pallet::<T>::close_aaa(RawOrigin::Root.into(), aaa_id)
        .expect("close_aaa must succeed in diagnostic benchmark");
    }
    assert!(!Pallet::<T>::active_actor_exists(aaa_id));
  }

  // Production close-tail admission benchmark; not a dispatchable call.
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
      user_program::<T>(schedule, execution_plan),
    )
    .expect("create_user_aaa must succeed in diagnostic setup");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let steps = s.min(T::MaxUserExecutionPlanSteps::get());
    let legs = l.min(T::MaxSplitTransferLegs::get());
    ActorProgram::<T>::mutate(aaa_id, |maybe| {
      let program = maybe.as_mut().expect("benchmark actor program exists");
      program.on_close_execution_plan =
        make_complex_on_close_execution_plan::<T>(&owner, steps, legs);
    });
    seed_on_close_complexity_budget::<T>(aaa_id, steps, legs);
    #[block]
    {
      Pallet::<T>::close_aaa(RawOrigin::Signed(owner).into(), aaa_id)
        .expect("close_aaa must succeed in user fee-bearing diagnostic benchmark");
    }
    assert!(!Pallet::<T>::active_actor_exists(aaa_id));
  }

  #[benchmark]
  fn fee_collection() {
    let payer: T::AccountId = whitelisted_caller();
    let owner: T::AccountId = account("fee-sink-owner", 0, 0);
    let schedule = Schedule {
      trigger: Trigger::Timer { every_blocks: 1 },
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
    let native = T::NativeAssetId::get();
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
  fn task_simple_asset_op() {
    let caller: T::AccountId = whitelisted_caller();
    let (target_id, recipient) = prepare_saturated_address_actor::<T>(0);
    let native = T::NativeAssetId::get();
    let amount = T::MinUserBalance::get().saturating_add(One::one());
    T::AssetOps::mint(&caller, native, amount.saturating_mul(2u32.into()))
      .expect("simple-transfer benchmark caller must be funded");
    T::BenchmarkHelper::enable_asset_ops_ingress();
    #[block]
    {
      T::AssetOps::transfer(&caller, &recipient, native, amount)
        .expect("ingress-aware transfer must succeed");
    }
    assert!(WakeupRetryPending::<T>::contains_key(target_id));
  }

  #[benchmark]
  fn task_split_transfer(l: Linear<2, 8>) {
    let caller: T::AccountId = whitelisted_caller();
    let bounded_legs = l.min(T::MaxSplitTransferLegs::get());
    let native = T::NativeAssetId::get();
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
      assert!(WakeupRetryPending::<T>::contains_key(target_id));
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
    assert!(WakeupRetryPending::<T>::contains_key(target_id));
  }

  #[benchmark]
  fn task_add_liquidity() {
    let caller: T::AccountId = whitelisted_caller();
    let (asset_a, asset_b, amount_a, amount_b) = T::BenchmarkHelper::setup_add_liquidity(&caller)
      .expect("benchmark helper must prepare add-liquidity state");
    #[block]
    {
      T::DexOps::add_liquidity(&caller, asset_a, asset_b, amount_a, amount_b)
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
      T::LiquidityDonationOps::donate_liquidity(&caller, asset_a, asset_b, amount, Perbill::zero())
        .expect("liquidity-donation benchmark operation must succeed");
    }
  }

  #[benchmark]
  fn task_remove_liquidity() {
    let caller: T::AccountId = whitelisted_caller();
    let max_scan = T::MaxAdapterScan::get();
    assert!(max_scan > 0, "MaxAdapterScan must be greater than zero");
    let (lp_asset, lp_amount) = T::BenchmarkHelper::setup_remove_liquidity_max_k(&caller, max_scan)
      .expect("benchmark helper must prepare remove-liquidity worst-case state");
    #[block]
    {
      T::DexOps::remove_liquidity(&caller, lp_asset, lp_amount)
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
      T::DexOps::swap_exact_in(&caller, asset_in, asset_out, amount_in, Perbill::zero())
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
        &caller,
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
      user_program::<T>(schedule, execution_plan),
    )
    .expect("create_user_aaa must succeed in setup");
    let aaa_id = NextAaaId::<T>::get().saturating_sub(1);
    let actor = Pallet::<T>::active_actor_snapshot(aaa_id)
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
    let inst =
      Pallet::<T>::active_actor_snapshot(aaa_id).expect("actor must survive benchmark cycle");
    assert_eq!(inst.cycle_nonce, 1);
    assert_eq!(inst.consecutive_failures, 0);
  }

  fn make_inert_execution_plan<T: Config>() -> ExecutionPlanOf<T> {
    let step = Step {
      conditions: BoundedVec::default(),
      task: AaaTask::Stake {
        asset: T::NativeAssetId::get(),
        amount: AmountResolution::Fixed(T::Balance::zero()),
      },
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

  fn prepare_saturated_address_actor<T: Config>(seed: u32) -> (AaaId, T::AccountId) {
    let owner: T::AccountId = account("ingress_owner", seed, 0);
    let schedule = Schedule {
      trigger: Trigger::OnAddressEvent {
        source_filter: SourceFilter::Any,
        asset_filter: AssetFilter::Any,
      },
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
      funding.funding_source_policy = FundingSourcePolicy::AnySource;
      funding
        .funding_snapshots
        .try_insert(
          T::NativeAssetId::get(),
          FundingBatch {
            amount: One::one(),
            pending_amount: One::one(),
          },
        )
        .expect("tracked funding batch fits");
    });
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(u64::from(T::MaxQueueLength::get()));
    for offset in 0..=T::MaxSpilloverBlocks::get() {
      let block: BlockNumberFor<T> = (2u32.saturating_add(offset)).into();
      fill_wakeup_bucket::<T>(
        block,
        T::MaxWakeupBucketSize::get(),
        10_000_000 + u64::from(offset) * 100_000,
      );
    }
    (aaa_id, recipient)
  }

  // Non-dispatch diagnostic benchmark proving cooldown-ineligible timers own no queue probe.
  #[benchmark]
  fn scheduler_cooldown_ineligible_idle() {
    let owner: T::AccountId = whitelisted_caller();
    let schedule = Schedule {
      trigger: Trigger::Timer { every_blocks: 1 },
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
    Pallet::<T>::manual_trigger(RawOrigin::Signed(owner).into(), aaa_id)
      .expect("manual trigger must succeed");
    let first_block: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(first_block);
    let _ = Pallet::<T>::on_idle(first_block, Weight::MAX);
    let expected_wakeup: BlockNumberFor<T> = 11u32.into();
    assert_eq!(
      ScheduledWakeupBlock::<T>::get(aaa_id),
      Some(expected_wakeup)
    );
    let now: BlockNumberFor<T> = 2u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    #[block]
    {
      let _ = Pallet::<T>::execute_cycle(Weight::MAX);
    }
    let instance = Pallet::<T>::active_actor_snapshot(aaa_id).expect("AAA exists");
    assert_eq!(instance.cycle_nonce, 1);
    assert_eq!(
      ScheduledWakeupBlock::<T>::get(aaa_id),
      Some(expected_wakeup)
    );
    assert!(ActorHot::<T>::get(aaa_id).is_some_and(|hot| hot.queue_ticket.is_none()));
  }

  #[benchmark]
  fn scheduler_on_idle_base() {
    let now: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(now);
    GlobalCircuitBreaker::<T>::put(false);
    LastIngressIngestBlock::<T>::kill();
    let threshold = T::MaxIdleStarvationBlocks::get();
    IdleStarvationBlocks::<T>::put(threshold.saturating_sub(1));
    #[block]
    {
      let breaker_active = GlobalCircuitBreaker::<T>::get();
      core::hint::black_box(QueueHead::<T>::get());
      core::hint::black_box(QueueTail::<T>::get());
      let _ = LastIngressIngestBlock::<T>::get();
      LastIngressIngestBlock::<T>::put(now);
      Pallet::<T>::update_idle_starvation_state(breaker_active, Weight::zero());
    }
    assert_eq!(LastIngressIngestBlock::<T>::get(), Some(now));
  }

  #[benchmark]
  fn scheduler_zombie_sweep_base() {
    NextAaaId::<T>::put(1);
    SweepCursor::<T>::put(0);
    #[block]
    {
      core::hint::black_box(NextAaaId::<T>::get());
      core::hint::black_box(SweepCursor::<T>::get());
      core::hint::black_box(WakeupRetryPending::<T>::iter_keys().next());
    }
  }

  #[benchmark]
  fn scheduler_actor_probe() {
    let aaa_id = bench_create_system_manual::<T>(3_000);
    ActorHot::<T>::mutate(aaa_id, |maybe_hot| {
      maybe_hot
        .as_mut()
        .expect("benchmark actor hot state must exist")
        .pending_signal = true;
    });
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    #[block]
    {
      Pallet::<T>::benchmark_scheduler_actor_probe(aaa_id);
    }
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
    #[block]
    {
      core::hint::black_box(Pallet::<T>::paged_drain_tombstones(
        u64::from(bounded),
        bounded,
      ));
    }
    assert!(QueueHead::<T>::get() >= u64::from(bounded));
    assert_eq!(QueueHead::<T>::get(), QueueTail::<T>::get());
  }

  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_mixed_scan(n: Linear<1, 10_000>) {
    let bounded = n.min(T::MaxQueueLength::get());
    let page_size = T::QueuePageSize::get();
    let template_id = bench_create_system_manual::<T>(38_000_000);
    let hot_template = ActorHot::<T>::get(template_id).expect("benchmark hot template");
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
            ActorHot::<T>::insert(aaa_id, hot);
          }
          QueueEntry { aaa_id }
        })
        .collect::<alloc::vec::Vec<_>>();
      QueuePages::<T>::insert(
        page_id,
        BoundedVec::<QueueEntry, T::QueuePageSize>::try_from(page)
          .expect("benchmark queue page must fit configured page size"),
      );
      ticket = ticket.saturating_add(entries);
    }
    ActorHot::<T>::remove(template_id);
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(u64::from(bounded));
    let cutoff = u64::from(bounded);
    #[block]
    {
      while QueueHead::<T>::get() < cutoff {
        core::hint::black_box(Pallet::<T>::paged_drain_tombstones(cutoff, bounded));
        if let Some((head, _)) = Pallet::<T>::paged_head_entry() {
          assert!(Pallet::<T>::paged_consume_head(head));
        }
      }
    }
    assert!(QueueHead::<T>::get() >= cutoff);
    assert_eq!(QueueHead::<T>::get(), QueueTail::<T>::get());
  }

  /// Measures actual scheduler admission and complete execution for up to 1,000
  /// minimal one-step System actors. `Weight::MAX` exposes the full production-Wasm
  /// cost curve; separate guaranteed-budget stress evidence determines how many
  /// executions the reference block budget actually admits. Setup writes the split actor stores and paged FIFO outside
  /// the measured block so the result isolates queue scanning, admission,
  /// execution, and consumption rather than actor creation.
  #[benchmark(pov_mode = Measured)]
  fn scheduler_paged_execute_cheap(n: Linear<1, 1_000>) {
    let _ = QueuePages::<T>::clear(u32::MAX, None);
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(0);
    MinWakeupBlock::<T>::kill();
    GlobalCircuitBreaker::<T>::put(false);
    let bounded = n
      .min(T::MaxExecutionsPerBlock::get())
      .min(T::MaxQueueEntriesScannedPerBlock::get())
      .min(T::MaxQueueLength::get());
    assert!(bounded > 0, "runtime limits must admit at least one sample");
    let template_id = bench_create_system_manual::<T>(40_000_000);
    let hot_template = ActorHot::<T>::get(template_id).expect("benchmark hot template");
    let program_template = ActorProgram::<T>::get(template_id).expect("benchmark program template");
    let funding_template = ActorFunding::<T>::get(template_id).expect("benchmark funding template");
    ActorHot::<T>::remove(template_id);
    ActorProgram::<T>::remove(template_id);
    ActorFunding::<T>::remove(template_id);

    let first_id = 41_000_000u64;
    for offset in 0..bounded {
      let aaa_id = first_id.saturating_add(u64::from(offset));
      let mut hot = hot_template.clone();
      hot.cycle_nonce = 0;
      hot.last_cycle_block = Zero::zero();
      hot.pending_signal = true;
      hot.queue_ticket = None;
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
        ActorHot::<T>::get(aaa_id).is_some_and(|hot| hot.cycle_nonce == 1)
      })
      .count() as u32;
    assert_eq!(
      executed, bounded,
      "unbounded diagnostic budget completed only {executed} of {bounded} requested cheap actors"
    );
    assert_eq!(QueueHead::<T>::get(), QueueTail::<T>::get());
  }

  // Runtime-backed hook benchmark for dense overdue wakeup admission.
  #[benchmark]
  fn scheduler_wakeup_dense_due_drain(n: Linear<0, 64>) {
    let due = n
      .min(T::MaxWakeupsPerBlock::get())
      .min(T::MaxWakeupBucketSize::get());
    let due_block: BlockNumberFor<T> = 1u32.into();
    frame_system::Pallet::<T>::set_block_number(due_block);
    fill_wakeup_bucket::<T>(due_block, due, 9_000_000);
    for i in 0..due {
      ScheduledWakeupBlock::<T>::insert(9_000_000u64.saturating_add(u64::from(i)), due_block);
    }
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

  // Runtime-backed hook benchmark for bounded spillover probing in WakeupIndex.
  #[benchmark]
  fn scheduler_wakeup_spillover_probe(b: Linear<0, 9>) {
    let aaa_id = bench_create_system_manual::<T>(b);
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    let blocked_buckets = b.min(9);
    QueueHead::<T>::put(0);
    QueueTail::<T>::put(u64::from(T::MaxQueueLength::get()));
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

  #[benchmark]
  fn transaction_extension_ingress_base() {
    let owner: T::AccountId = whitelisted_caller();
    let populated_aaa_id = bench_create_user::<T>(owner);
    let proof_witness = Pallet::<T>::active_actor_snapshot(populated_aaa_id)
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
      assert!(!T::BenchmarkHelper::run_address_event_ingress(&recipient));
    }
  }

  #[benchmark]
  fn transaction_extension_ingress_notify() {
    let source: T::AccountId = account("ingress_source", 0, 0);
    let (aaa_id, recipient) = prepare_saturated_address_actor::<T>(0);
    T::BenchmarkHelper::setup_address_event_ingress(&recipient, &source, One::one())
      .expect("benchmark helper must prepare a matched producer event");
    #[block]
    {
      assert!(T::BenchmarkHelper::run_address_event_ingress(&recipient));
    }
    assert!(ActorHot::<T>::get(aaa_id).is_some_and(|hot| hot.pending_signal));
    assert!(WakeupRetryPending::<T>::contains_key(aaa_id));
  }

  #[benchmark]
  fn funding_batch_promotion(a: Linear<1, 10>) {
    let owner: T::AccountId = whitelisted_caller();
    let aaa_id = bench_create_user::<T>(owner);
    let assets = T::BenchmarkHelper::funding_assets(a);
    ActorFunding::<T>::mutate(aaa_id, |maybe| {
      let funding = maybe.as_mut().expect("benchmark actor funding exists");
      funding.has_pending_funding = true;
      for asset in assets {
        funding
          .funding_snapshots
          .try_insert(
            asset,
            FundingBatch {
              amount: One::one(),
              pending_amount: 2u32.into(),
            },
          )
          .expect("promotion benchmark bound fits");
      }
    });
    #[block]
    {
      Pallet::<T>::promote_pending_funding(aaa_id);
    }
    let funding = ActorFunding::<T>::get(aaa_id).expect("benchmark actor funding exists");
    assert!(
      funding
        .funding_snapshots
        .values()
        .all(|batch| batch.amount == 2u32.into() && batch.pending_amount.is_zero())
    );
  }

  #[benchmark]
  fn compatibility_ingress_probe() {
    IngressOverflowLen::<T>::put(0);
    #[block]
    {
      core::hint::black_box(IngressOverflowLen::<T>::get());
    }
  }

  #[benchmark]
  fn compatibility_ingress_drain() {
    let source: T::AccountId = account("ingress_source", 1, 0);
    let (aaa_id, _) = prepare_saturated_address_actor::<T>(1);
    T::BenchmarkHelper::clear_address_event_ingress_events();
    assert!(Pallet::<T>::queue_address_event(
      aaa_id,
      T::NativeAssetId::get(),
      One::one(),
      Some(FundingProvenance::Signed(source)),
    ));
    #[block]
    {
      let _ = Pallet::<T>::drain_address_event_overflow(1);
    }
    assert_eq!(IngressOverflowLen::<T>::get(), 0);
    assert!(ActorHot::<T>::get(aaa_id).is_some_and(|hot| hot.pending_signal));
    assert!(WakeupRetryPending::<T>::contains_key(aaa_id));
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
    let native = T::NativeAssetId::get();
    let schedule = Schedule {
      trigger: Trigger::Timer { every_blocks: 1 },
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
