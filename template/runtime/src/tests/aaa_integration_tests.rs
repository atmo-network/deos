use super::common::{
  ALICE, ASSET_A, BOB, CHARLIE, create_test_asset, mint_tokens, seeded_test_ext,
};
use crate::{
  AAA, Assets, Balances, Runtime, RuntimeEvent, RuntimeOrigin, System,
  configs::{AddressEventIngress, RuntimeAddressEventIngress, RuntimeAddressEventIngressHook},
};
use alloc::boxed::Box;
use pallet_aaa::{
  AaaId, AmountResolution, AssetFilter, AssetFilterOf, CloseReason, DeferReason, DexOps, Error,
  Event, ExecutionPlanOf, IdleStarvationBlocks, Mutability, Schedule, ScheduleOf, ScheduleWindow,
  SourceFilter, SourceFilterOf, SplitLeg, SplitTransferLegsOf, StepErrorPolicy, StepOf,
  StepSkippedReason, Task, TaskOf, Trigger, WeightInfo,
};
use pallet_axial_router::FeeRoutingAdapter;
use polkadot_sdk::frame_support::{
  BoundedVec, assert_noop, assert_ok,
  traits::{
    Currency, Get, Hooks,
    tokens::imbalance::{ImbalanceAccounting, UnsafeConstructorDestructor, UnsafeManualAccounting},
  },
  weights::Weight,
};
use polkadot_sdk::sp_runtime::{DispatchError, Perbill};
use polkadot_sdk::sp_weights::WeightToFee;
use polkadot_sdk::{
  staging_xcm as xcm,
  staging_xcm_executor::{AssetsInHolding, traits::TransactAsset},
};
use primitives::AssetKind;

type RuntimeSchedule = ScheduleOf<Runtime>;
type RuntimeSourceFilter = SourceFilterOf<Runtime>;
type RuntimeAssetFilter = AssetFilterOf<Runtime>;
type RuntimeTask = TaskOf<Runtime>;
type RuntimeStep = StepOf<Runtime>;
type ExecutionPlan = ExecutionPlanOf<Runtime>;

fn make_step(task: RuntimeTask) -> RuntimeStep {
  StepOf::<Runtime> {
    conditions: BoundedVec::default(),
    task,
    on_error: StepErrorPolicy::AbortCycle,
  }
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

fn transfer_execution_plan(to: crate::AccountId, asset: AssetKind, amount: u128) -> ExecutionPlan {
  BoundedVec::try_from(vec![make_step(Task::Transfer {
    to,
    asset,
    amount: AmountResolution::Fixed(amount),
  })])
  .expect("execution_plan fits")
}

fn create_user(
  who: crate::AccountId,
  schedule: RuntimeSchedule,
  schedule_window: Option<ScheduleWindow<u32>>,
  execution_plan: ExecutionPlan,
) -> AaaId {
  let id = AAA::next_aaa_id();
  assert_ok!(AAA::create_user_aaa(
    RuntimeOrigin::signed(who),
    Mutability::Mutable,
    schedule,
    schedule_window,
    execution_plan,
  ));
  id
}

fn create_system(
  owner: crate::AccountId,
  schedule: RuntimeSchedule,
  schedule_window: Option<ScheduleWindow<u32>>,
  execution_plan: ExecutionPlan,
) -> AaaId {
  let id = AAA::next_aaa_id();
  assert_ok!(AAA::create_system_aaa(
    RuntimeOrigin::root(),
    owner,
    schedule,
    schedule_window,
    execution_plan,
  ));
  id
}

fn aaa_account(aaa_id: AaaId) -> crate::AccountId {
  AAA::aaa_instances(aaa_id)
    .map(|instance| instance.sovereign_account)
    .expect("AAA must exist")
}

fn fund_native(aaa_id: AaaId, amount: u128) {
  let aaa_acc = aaa_account(aaa_id);
  let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&aaa_acc, amount);
}

fn fund_native_via_call(funder: crate::AccountId, aaa_id: AaaId, amount: u128) {
  assert_ok!(AAA::fund_aaa(
    RuntimeOrigin::signed(funder),
    aaa_id,
    AssetKind::Native,
    amount
  ));
}

fn native_balance(who: &crate::AccountId) -> u128 {
  Balances::free_balance(who)
}

fn account_location(who: crate::AccountId) -> xcm::latest::Location {
  let mut id = [0u8; 32];
  id.copy_from_slice(who.as_ref());
  xcm::latest::Location::new(
    0,
    [xcm::latest::Junction::AccountId32 { network: None, id }],
  )
}

fn native_xcm_asset(amount: u128) -> xcm::latest::Asset {
  xcm::latest::Asset {
    id: xcm::latest::AssetId(xcm::latest::Location::parent()),
    fun: xcm::latest::Fungibility::Fungible(amount),
  }
}

#[derive(Clone)]
struct MockCredit(u128);

impl UnsafeConstructorDestructor<u128> for MockCredit {
  fn unsafe_clone(&self) -> Box<dyn ImbalanceAccounting<u128>> {
    Box::new(Self(self.0))
  }

  fn forget_imbalance(&mut self) -> u128 {
    core::mem::take(&mut self.0)
  }
}

impl UnsafeManualAccounting<u128> for MockCredit {
  fn saturating_subsume(&mut self, mut other: Box<dyn ImbalanceAccounting<u128>>) {
    self.0 = self.0.saturating_add(other.amount());
    let _ = other.forget_imbalance();
  }
}

impl ImbalanceAccounting<u128> for MockCredit {
  fn amount(&self) -> u128 {
    self.0
  }

  fn saturating_take(&mut self, amount: u128) -> Box<dyn ImbalanceAccounting<u128>> {
    let taken = self.0.min(amount);
    self.0 -= taken;
    Box::new(Self(taken))
  }
}

fn asset_to_holding(asset: xcm::latest::Asset) -> AssetsInHolding {
  let mut holding = AssetsInHolding::new();
  match asset.fun {
    xcm::latest::Fungibility::Fungible(amount) => {
      holding
        .fungible
        .insert(asset.id, Box::new(MockCredit(amount)));
    }
    xcm::latest::Fungibility::NonFungible(instance) => {
      holding.non_fungible.insert((asset.id, instance));
    }
  }
  holding
}

fn run_idle(weight: Weight) {
  AAA::on_idle(System::block_number(), weight);
}

fn run_idle_until_cycle_nonce(aaa_id: AaaId, target_cycle_nonce: u64) {
  for _ in 0..20 {
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

fn aaa_events() -> alloc::vec::Vec<Event<Runtime>> {
  System::events()
    .into_iter()
    .filter_map(|record| match record.event {
      RuntimeEvent::AAA(event) => Some(event),
      _ => None,
    })
    .collect()
}

pub fn has_aaa_event(predicate: impl Fn(&Event<Runtime>) -> bool) -> bool {
  aaa_events().iter().any(predicate)
}

// --- AAA Platform: Lifecycle ---

#[test]
fn manual_trigger_executes_transfer_execution_plan() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 5_000_000_000_000u128;
    let aaa_id = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    fund_native(aaa_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          aaa_id: id,
          cycle_nonce: 1,
          executed_steps: 1,
          skipped_conditions: 0,
          skipped_resolution: 0,
          skipped_funding_unavailable: 0,
          failed_steps: 0,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn create_user_charges_creation_fee_to_fee_sink() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let fee = <Runtime as pallet_aaa::Config>::AaaCreationFee::get();
    let fee_sink = <Runtime as pallet_aaa::Config>::FeeSink::get();
    let sink_before = native_balance(&fee_sink);
    let alice_before = native_balance(&ALICE);
    let _ = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    assert_eq!(native_balance(&fee_sink), sink_before.saturating_add(fee));
    assert_eq!(native_balance(&ALICE), alice_before.saturating_sub(fee));
  });
}

#[test]
fn runtime_policy_allows_probabilistic_financial_timer_without_secure_entropy_provider() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::Timer {
        every_blocks: 1,
        probability: Some(Perbill::from_percent(50)),
      },
      cooldown_blocks: 0,
    };
    assert_ok!(AAA::create_system_aaa(
      RuntimeOrigin::root(),
      ALICE,
      schedule,
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    ));
  });
}

#[test]
fn permissionless_sweep_many_batches_lifecycle_evaluation() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let user_a = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    let user_b = create_user(
      BOB,
      manual_schedule(),
      None,
      transfer_execution_plan(ALICE, AssetKind::Native, 1),
    );
    let system_alive = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    let sweep_ids: BoundedVec<AaaId, <Runtime as pallet_aaa::Config>::MaxSweepPerBlock> =
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
fn zombie_spam_attack_cost_dominates_batch_cleanup_cost() {
  seeded_test_ext().execute_with(|| {
    let active_cap = <Runtime as pallet_aaa::Config>::MaxActiveActors::get();
    let creation_fee = <Runtime as pallet_aaa::Config>::AaaCreationFee::get();
    let create_weight =
      <<Runtime as pallet_aaa::Config>::WeightInfo as WeightInfo>::create_user_aaa();
    let create_tx_fee = <Runtime as pallet_aaa::Config>::WeightToFee::weight_to_fee(&create_weight);
    let attacker_cost_per_actor = creation_fee.saturating_add(create_tx_fee);
    let attacker_total_cost = attacker_cost_per_actor.saturating_mul(active_cap as u128);
    let sweep_batch_size = <Runtime as pallet_aaa::Config>::MaxSweepPerBlock::get().max(1);
    let sweep_calls = active_cap.div_ceil(sweep_batch_size);
    let batch_sweep_weight =
      <<Runtime as pallet_aaa::Config>::WeightInfo as WeightInfo>::permissionless_sweep_many(
        sweep_batch_size,
      );
    let batch_sweep_tx_fee =
      <Runtime as pallet_aaa::Config>::WeightToFee::weight_to_fee(&batch_sweep_weight);
    let cleanup_total_cost = batch_sweep_tx_fee.saturating_mul(sweep_calls as u128);
    assert!(cleanup_total_cost > 0, "Cleanup fee floor must be non-zero");
    assert!(
      attacker_total_cost >= cleanup_total_cost.saturating_mul(100),
      "Creation-cost floor must dominate bounded cleanup cost by >=100x"
    );
    let cost_ratio_bp = attacker_total_cost.saturating_mul(10_000) / cleanup_total_cost;
    println!(
      "AAA zombie economics: active_cap={}, creation_fee={}, create_tx_fee={}, attacker_total_cost={}, sweep_batch_size={}, sweep_calls={}, batch_sweep_tx_fee={}, cleanup_total_cost={}, cost_ratio={:.2}x",
      active_cap,
      creation_fee,
      create_tx_fee,
      attacker_total_cost,
      sweep_batch_size,
      sweep_calls,
      batch_sweep_tx_fee,
      cleanup_total_cost,
      (cost_ratio_bp as f64) / 10_000.0,
    );
  });
}

#[test]
fn min_user_balance_is_not_below_native_existential_deposit() {
  seeded_test_ext().execute_with(|| {
    let configured_min_user_balance = crate::configs::aaa_config::AaaMinUserBalance::get();
    let min_user_balance = <Runtime as pallet_aaa::Config>::MinUserBalance::get();
    let native_ed = <Balances as Currency<crate::AccountId>>::minimum_balance();
    assert_eq!(
      min_user_balance,
      configured_min_user_balance.max(native_ed),
      "Runtime MinUserBalance guard must clamp below-ED configurations"
    );
    assert!(
      min_user_balance >= native_ed,
      "MinUserBalance must be >= native ExistentialDeposit"
    );
  });
}

#[test]
fn queue_length_covers_active_actor_capacity() {
  seeded_test_ext().execute_with(|| {
    let queue_cap = <Runtime as pallet_aaa::Config>::MaxQueueLength::get();
    let active_cap = <Runtime as pallet_aaa::Config>::MaxActiveActors::get();
    assert!(
      queue_cap >= active_cap,
      "MaxQueueLength must be >= MaxActiveActors to avoid scheduler actor loss under full activation"
    );
  });
}

#[test]
fn close_aaa_emits_owner_initiated_reason() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let aaa_id = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    let fee_sink = <Runtime as pallet_aaa::Config>::FeeSink::get();
    let fee_sink_before = native_balance(&fee_sink);
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert_eq!(native_balance(&fee_sink), fee_sink_before);
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
fn close_aaa_executes_fee_bearing_user_close_tail_and_routes_fees_to_fee_sink() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let local_asset_id = 43u32;
    assert_ok!(create_test_asset(local_asset_id, &ALICE));
    assert_ok!(Assets::set_team(
      RuntimeOrigin::signed(ALICE),
      local_asset_id,
      ALICE.into(),
      ALICE.into(),
      ALICE.into(),
    ));
    let aaa_id = create_user(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![make_step(Task::Noop)]).expect("execution_plan fits"),
    );
    let aaa_acc = aaa_account(aaa_id);
    assert_ok!(mint_tokens(local_asset_id, &ALICE, &aaa_acc, 500));
    fund_native(aaa_id, 100_000_000_000_000);
    let close_task = Task::Transfer {
      to: ALICE,
      asset: AssetKind::Local(local_asset_id),
      amount: AmountResolution::Fixed(499),
    };
    let on_close_execution_plan = BoundedVec::try_from(vec![make_step(close_task.clone())])
      .expect("on_close_execution_plan fits");
    assert_ok!(AAA::update_on_close_execution_plan(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      on_close_execution_plan,
    ));
    let fee_sink = <Runtime as pallet_aaa::Config>::FeeSink::get();
    let fee_sink_before = native_balance(&fee_sink);
    let alice_before = Assets::balance(local_asset_id, ALICE);
    let expected_close_fee = <Runtime as pallet_aaa::Config>::StepBaseFee::get().saturating_add(
      crate::WeightToFee::weight_to_fee(&AAA::weight_upper_bound(&close_task)),
    );
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert_eq!(
      native_balance(&fee_sink),
      fee_sink_before.saturating_add(expected_close_fee)
    );
    assert_eq!(Assets::balance(local_asset_id, aaa_acc.clone()), 1);
    assert_eq!(
      Assets::balance(local_asset_id, ALICE),
      alice_before.saturating_add(499)
    );
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
fn close_aaa_executes_system_close_tail_without_charging_fee_sink() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let local_asset_id = 44u32;
    assert_ok!(create_test_asset(local_asset_id, &ALICE));
    assert_ok!(Assets::set_team(
      RuntimeOrigin::signed(ALICE),
      local_asset_id,
      ALICE.into(),
      ALICE.into(),
      ALICE.into(),
    ));
    let aaa_id = create_system(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![make_step(Task::Noop)]).expect("execution_plan fits"),
    );
    let aaa_acc = aaa_account(aaa_id);
    assert_ok!(mint_tokens(local_asset_id, &ALICE, &aaa_acc, 500));
    let on_close_execution_plan = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: ALICE,
      asset: AssetKind::Local(local_asset_id),
      amount: AmountResolution::Fixed(499),
    })])
    .expect("on_close_execution_plan fits");
    assert_ok!(AAA::update_on_close_execution_plan(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      on_close_execution_plan,
    ));
    let fee_sink = <Runtime as pallet_aaa::Config>::FeeSink::get();
    let fee_sink_before = native_balance(&fee_sink);
    let alice_before = Assets::balance(local_asset_id, ALICE);
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), aaa_id));
    assert_eq!(native_balance(&fee_sink), fee_sink_before);
    assert_eq!(Assets::balance(local_asset_id, aaa_acc.clone()), 1);
    assert_eq!(
      Assets::balance(local_asset_id, ALICE),
      alice_before.saturating_add(499)
    );
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
fn automatic_close_defers_until_runtime_can_admit_close_tail() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let local_asset_id = 42u32;
    assert_ok!(create_test_asset(local_asset_id, &ALICE));
    assert_ok!(Assets::set_team(
      RuntimeOrigin::signed(ALICE),
      local_asset_id,
      ALICE.into(),
      ALICE.into(),
      ALICE.into(),
    ));
    let aaa_id = create_user(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![make_step(Task::Noop)]).expect("execution_plan fits"),
    );
    let aaa_acc = aaa_account(aaa_id);
    assert_ok!(mint_tokens(local_asset_id, &ALICE, &aaa_acc, 500));
    fund_native(aaa_id, 100_000_000_000_000);
    let on_close_execution_plan = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: ALICE,
      asset: AssetKind::Local(local_asset_id),
      amount: AmountResolution::AllBalance,
    })])
    .expect("on_close_execution_plan fits");
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
    run_idle(Weight::from_parts(1, 0));
    let inst =
      AAA::aaa_instances(aaa_id).expect("AAA must remain active until close-tail budget fits");
    assert_eq!(inst.cycle_nonce, 0);
    assert_eq!(Assets::balance(local_asset_id, aaa_acc.clone()), 500);
    System::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert_eq!(Assets::balance(local_asset_id, aaa_acc.clone()), 1);
    assert_eq!(Assets::balance(local_asset_id, ALICE), 499);
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
fn window_expired_actor_closes_on_fund_touchpoint() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let window = ScheduleWindow { start: 1, end: 101 };
    let aaa_id = create_user(
      ALICE,
      manual_schedule(),
      Some(window),
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    System::set_block_number(102);
    assert_ok!(AAA::fund_aaa(
      RuntimeOrigin::signed(ALICE),
      aaa_id,
      AssetKind::Native,
      10
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

// --- AAA Platform: Amount Resolution ---

#[test]
fn percentage_of_trigger_uses_cycle_start_snapshot() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let execution_plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: AssetKind::Native,
        amount: AmountResolution::Fixed(50),
      }),
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: AssetKind::Native,
        amount: AmountResolution::PercentageOfTrigger(Perbill::from_percent(50)),
      }),
    ])
    .expect("execution_plan fits");
    let aaa_id = create_system(ALICE, manual_schedule(), None, execution_plan);
    let funding = 1_000_000_000_000u128;
    fund_native(aaa_id, funding);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(
      native_balance(&CHARLIE),
      charlie_before.saturating_add(funding / 2)
    );
  });
}

#[test]
fn percentage_of_last_funding_keeps_system_actor_active_on_exhaustion() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let execution_plan = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    })])
    .expect("execution_plan fits");
    let aaa_id = create_system(ALICE, manual_schedule(), None, execution_plan);
    fund_native_via_call(ALICE, aaa_id, 10_000_000_000_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle_until_cycle_nonce(aaa_id, 1);
    System::set_block_number(2);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle_until_cycle_nonce(aaa_id, 2);
    System::set_block_number(3);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle_until_cycle_nonce(aaa_id, 3);
    let instance = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert!(!instance.is_paused);
    assert_eq!(instance.pause_reason, None);
    fund_native_via_call(CHARLIE, aaa_id, 8_000_000_000_000);
    let updated = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert_eq!(
      updated
        .funding_snapshots
        .get(&AssetKind::Native)
        .unwrap()
        .amount,
      8_000_000_000_000
    );
  });
}

#[test]
fn cycle_summary_reports_funding_unavailable_skip() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let execution_plan = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    })])
    .expect("execution_plan fits");
    let aaa_id = create_system(ALICE, manual_schedule(), None, execution_plan);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle_until_cycle_nonce(aaa_id, 1);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          aaa_id: id,
          cycle_nonce: 1,
          executed_steps: 0,
          skipped_conditions: 0,
          skipped_resolution: 0,
          skipped_funding_unavailable: 1,
          failed_steps: 0,
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn percentage_of_last_funding_keeps_user_actor_active_on_exhaustion() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let execution_plan = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(100)),
    })])
    .expect("execution_plan fits");
    let aaa_id = create_user(ALICE, manual_schedule(), None, execution_plan);
    fund_native_via_call(ALICE, aaa_id, 1_000_000_000_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_some());
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
fn swap_exact_out_rounding_boundary_uses_minimal_input_for_target_output() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let target_out = crate::EXISTENTIAL_DEPOSIT;
    let execution_plan = BoundedVec::try_from(vec![make_step(Task::SwapExactOut {
      asset_in: AssetKind::Native,
      asset_out: AssetKind::Local(ASSET_A),
      amount_out: AmountResolution::Fixed(target_out),
      slippage_tolerance: Perbill::zero(),
    })])
    .expect("execution_plan fits");
    let aaa_id = create_user(ALICE, manual_schedule(), None, execution_plan);
    let sovereign = aaa_account(aaa_id);
    fund_native(aaa_id, 100_000_000_000_000);
    let out_before = Assets::balance(ASSET_A, sovereign.clone());
    let effective_quote = |gross_in: u128| -> Option<u128> {
      if gross_in == 0 {
        return None;
      }
      let fee = if crate::AxialRouter::is_fee_exempt(&sovereign) {
        0
      } else {
        crate::AxialRouter::calculate_router_fee(gross_in)
      };
      let net_in = gross_in.saturating_sub(fee);
      if net_in == 0 {
        return None;
      }
      crate::AxialRouter::quote_price(AssetKind::Native, AssetKind::Local(ASSET_A), net_in).ok()
    };
    let mut high = 1u128;
    let mut found = false;
    for _ in 0..128 {
      match effective_quote(high) {
        Some(quoted) if quoted >= target_out => {
          found = true;
          break;
        }
        _ => {
          high = high.checked_mul(2).expect("search overflow");
        }
      }
    }
    assert!(found, "target output must be quotable in seeded pool");
    let mut low = 1u128;
    while low < high {
      let mid = low.saturating_add(high.saturating_sub(low) / 2);
      match effective_quote(mid) {
        Some(quoted) if quoted >= target_out => {
          high = mid;
        }
        _ => {
          low = mid.saturating_add(1);
        }
      }
    }
    let expected_required_in = high;
    if expected_required_in > 1 {
      let prev_quote = effective_quote(expected_required_in.saturating_sub(1)).unwrap_or_default();
      assert!(
        prev_quote < target_out,
        "selected input must be minimal at rounding boundary"
      );
    }
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    let events = aaa_events();
    let (amount_in, amount_out) = events
      .iter()
      .find_map(|event| match event {
        Event::SwapExecuted {
          aaa_id: id,
          asset_in,
          asset_out,
          amount_in,
          amount_out,
        } if *id == aaa_id
          && *asset_in == AssetKind::Native
          && *asset_out == AssetKind::Local(ASSET_A) =>
        {
          Some((*amount_in, *amount_out))
        }
        _ => None,
      })
      .unwrap_or_else(|| panic!("SwapExecuted must be emitted, events={events:?}"));
    assert_eq!(amount_out, target_out);
    assert_eq!(amount_in, expected_required_in);
    let out_after = Assets::balance(ASSET_A, sovereign.clone());
    assert!(out_after >= out_before.saturating_add(target_out));
  });
}

#[test]
fn swap_exact_out_liquidity_boundary_fails_without_partial_execution() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let impossible_out = super::common::LIQUIDITY_AMOUNT;
    let execution_plan = BoundedVec::try_from(vec![make_step(Task::SwapExactOut {
      asset_in: AssetKind::Native,
      asset_out: AssetKind::Local(ASSET_A),
      amount_out: AmountResolution::Fixed(impossible_out),
      slippage_tolerance: Perbill::zero(),
    })])
    .expect("execution_plan fits");
    let aaa_id = create_user(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100_000_000_000_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
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
    assert!(!has_aaa_event(|event| {
      matches!(event, Event::SwapExecuted { aaa_id: id, .. } if *id == aaa_id)
    }));
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          aaa_id: id,
          executed_steps: 0,
          failed_steps: 1,
          ..
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn swap_exact_out_fails_when_required_input_exceeds_actor_balance() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let target_out = crate::EXISTENTIAL_DEPOSIT;
    let execution_plan = BoundedVec::try_from(vec![make_step(Task::SwapExactOut {
      asset_in: AssetKind::Native,
      asset_out: AssetKind::Local(ASSET_A),
      amount_out: AmountResolution::Fixed(target_out),
      slippage_tolerance: Perbill::zero(),
    })])
    .expect("execution_plan fits");
    let aaa_id = create_system(ALICE, manual_schedule(), None, execution_plan);
    let sovereign = aaa_account(aaa_id);
    let quote_output = |amount_in: u128| -> Option<u128> {
      if amount_in == 0 {
        return None;
      }
      let fee = if crate::AxialRouter::is_fee_exempt(&sovereign) {
        0
      } else {
        crate::AxialRouter::calculate_router_fee(amount_in)
      };
      let net_in = amount_in.saturating_sub(fee);
      if net_in == 0 {
        return None;
      }
      crate::AxialRouter::quote_price(AssetKind::Native, AssetKind::Local(ASSET_A), net_in).ok()
    };
    let mut high = 1u128;
    let mut found = false;
    for _ in 0..128 {
      match quote_output(high) {
        Some(quoted) if quoted >= target_out => {
          found = true;
          break;
        }
        _ => {
          high = high.checked_mul(2).expect("search overflow");
        }
      }
    }
    assert!(found, "target output must be quotable in seeded pool");
    let mut low = 1u128;
    while low < high {
      let mid = low.saturating_add(high.saturating_sub(low) / 2);
      match quote_output(mid) {
        Some(quoted) if quoted >= target_out => {
          high = mid;
        }
        _ => {
          low = mid.saturating_add(1);
        }
      }
    }
    let required_in = high;
    fund_native(aaa_id, required_in.saturating_sub(1));
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
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
    assert!(!has_aaa_event(|event| {
      matches!(event, Event::SwapExecuted { aaa_id: id, .. } if *id == aaa_id)
    }));
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          aaa_id: id,
          executed_steps: 0,
          failed_steps: 1,
          ..
        } if *id == aaa_id
      )
    }));
  });
}

#[test]
fn dex_exact_out_adapter_rejects_unfunded_input_with_explicit_error() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let unfunded = crate::AccountId::new([99u8; 32]);
    let result = <crate::configs::aaa_config::TmctolDexOps as DexOps<
      crate::AccountId,
      AssetKind,
      u128,
    >>::swap_exact_out(
      &unfunded,
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      crate::EXISTENTIAL_DEPOSIT,
      Perbill::zero(),
    );
    assert_eq!(
      result,
      Err(DispatchError::Other("InsufficientInputForExactOut"))
    );
  });
}

#[test]
fn staking_adapter_rejects_native_stake_without_operator_context() {
  seeded_test_ext().execute_with(|| {
    let who = crate::AccountId::new([77u8; 32]);
    let result =
      <crate::configs::aaa_config::TmctolStakingOps as pallet_aaa::adapters::StakingOps<
        crate::AccountId,
        AssetKind,
        u128,
      >>::stake(&who, AssetKind::Native, crate::EXISTENTIAL_DEPOSIT);
    assert_eq!(
      result,
      Err(pallet_staking::Error::<crate::Runtime>::NativeStakeRequiresOperator.into())
    );
  });
}

#[test]
fn staking_adapter_supports_native_stake_with_operator_context() {
  seeded_test_ext().execute_with(|| {
    crate::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(crate::Staking::register_staking_asset(
      RuntimeOrigin::root(),
      0
    ));
    let who = crate::AccountId::new([77u8; 32]);
    assert_ok!(Assets::set_team(
      RuntimeOrigin::signed(ALICE),
      0,
      ALICE.into(),
      ALICE.into(),
      ALICE.into(),
    ));
    assert_ok!(mint_tokens(
      0,
      &ALICE,
      &who,
      crate::EXISTENTIAL_DEPOSIT.saturating_mul(10)
    ));
    let before = Assets::balance(0, who.clone());
    let result =
      <crate::configs::aaa_config::TmctolStakingOps as pallet_aaa::adapters::StakingOps<
        crate::AccountId,
        AssetKind,
        u128,
      >>::stake_native(&who, crate::EXISTENTIAL_DEPOSIT, &ALICE);
    assert_ok!(result);
    assert_eq!(crate::Staking::native_binding(&who), Some(ALICE));
    assert!(crate::Staking::live_native_staked_receipt_balance(&who).unwrap_or_default() > 0);
    assert!(Assets::balance(0, who.clone()) < before);
  });
}

#[test]
fn aaa_native_stake_task_binds_operator_and_mints_stntve() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    crate::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(crate::Staking::register_staking_asset(
      RuntimeOrigin::root(),
      0
    ));
    assert_ok!(Assets::set_team(
      RuntimeOrigin::signed(ALICE),
      0,
      ALICE.into(),
      ALICE.into(),
      ALICE.into(),
    ));
    let execution_plan = BoundedVec::try_from(vec![make_step(Task::StakeNative {
      amount: AmountResolution::Fixed(crate::EXISTENTIAL_DEPOSIT),
      operator: ALICE,
    })])
    .expect("execution_plan fits");
    let aaa_id = create_user(BOB, manual_schedule(), None, execution_plan);
    let aaa_acc = aaa_account(aaa_id);
    assert_ok!(mint_tokens(
      0,
      &ALICE,
      &aaa_acc,
      crate::EXISTENTIAL_DEPOSIT.saturating_mul(10),
    ));
    fund_native(aaa_id, 100_000_000_000_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(BOB), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(crate::Staking::native_binding(&aaa_acc), Some(ALICE));
    assert!(
      crate::Staking::live_native_staked_receipt_balance(&aaa_acc).unwrap_or_default() > 0,
      "AAA sovereign must receive stNTVE after native stake"
    );
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::StakeNativeExecuted {
          aaa_id: id,
          amount,
          operator,
        } if *id == aaa_id && *amount == crate::EXISTENTIAL_DEPOSIT && *operator == ALICE
      )
    }));
  });
}

// --- AAA Platform: SplitTransfer ---

#[test]
fn split_transfer_uses_perbill_and_keeps_remainder_on_aaa() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
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
    let execution_plan = BoundedVec::try_from(vec![make_step(Task::SplitTransfer {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(total),
      legs,
    })])
    .expect("execution_plan fits");
    let aaa_id = create_user(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100_000_000_000_000);
    let aaa_acc = aaa_account(aaa_id);
    let aaa_before = native_balance(&aaa_acc);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(native_balance(&CHARLIE), charlie_before.saturating_add(50));
    let spent = aaa_before.saturating_sub(native_balance(&aaa_acc));
    assert!(spent >= 100, "AAA must spend at least distributed amount");
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::SplitTransferExecuted {
          aaa_id: id,
          total: amount,
          distributed,
          retained,
          legs: 2,
          effective_legs: 2,
          ..
        } if *id == aaa_id
          && *amount == total
          && *distributed == 100
          && *retained == 1
      )
    }));
  });
}

#[test]
fn split_transfer_event_reports_filtered_legs_and_retained_amount() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let unknown = crate::AccountId::new([9u8; 32]);
    let total = 100u128;
    let legs = BoundedVec::try_from(vec![
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(50),
      },
      SplitLeg {
        to: unknown.clone(),
        share: Perbill::from_percent(50),
      },
    ])
    .expect("legs fit");
    let execution_plan = BoundedVec::try_from(vec![make_step(Task::SplitTransfer {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(total),
      legs,
    })])
    .expect("execution_plan fits");
    let aaa_id = create_user(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    let unknown_before = native_balance(&unknown);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(native_balance(&unknown), unknown_before);
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::SplitTransferExecuted {
          aaa_id: id,
          total: amount,
          distributed,
          retained,
          legs: 2,
          effective_legs: 1,
          ..
        } if *id == aaa_id
          && *amount == total
          && *distributed == 50
          && *retained == 50
      )
    }));
  });
}

#[test]
fn create_rejects_split_transfer_share_sum_above_one() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
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
    let execution_plan = BoundedVec::try_from(vec![make_step(Task::SplitTransfer {
      asset: AssetKind::Native,
      amount: AmountResolution::Fixed(100),
      legs,
    })])
    .expect("execution_plan fits");
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        manual_schedule(),
        None,
        execution_plan,
      ),
      Error::<Runtime>::InvalidSplitTransfer
    );
  });
}

// --- AAA Platform: Bounds & Validation ---

#[test]
fn split_transfer_leg_count_is_bounded_by_runtime_type_limit() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let max_legs =
      <<Runtime as pallet_aaa::Config>::MaxSplitTransferLegs as Get<u32>>::get() as usize;
    let within_limit = (0..max_legs)
      .map(|offset| SplitLeg {
        to: crate::AccountId::new([10u8.saturating_add(offset as u8); 32]),
        share: Perbill::from_percent(1),
      })
      .collect::<Vec<_>>();
    let above_limit = (0..max_legs.saturating_add(1))
      .map(|offset| SplitLeg {
        to: crate::AccountId::new([10u8.saturating_add(offset as u8); 32]),
        share: Perbill::from_percent(1),
      })
      .collect::<Vec<_>>();
    assert!(SplitTransferLegsOf::<Runtime>::try_from(within_limit).is_ok());
    assert!(SplitTransferLegsOf::<Runtime>::try_from(above_limit).is_err());
  });
}

#[test]
fn whitelist_size_is_bounded_by_runtime_type_limit() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let max_whitelist =
      <<Runtime as pallet_aaa::Config>::MaxWhitelistSize as Get<u32>>::get() as usize;
    let within_limit = (0..max_whitelist)
      .map(|offset| crate::AccountId::new([40u8.saturating_add(offset as u8); 32]))
      .collect::<Vec<_>>();
    let above_limit = (0..max_whitelist.saturating_add(1))
      .map(|offset| crate::AccountId::new([40u8.saturating_add(offset as u8); 32]))
      .collect::<Vec<_>>();
    assert!(
      BoundedVec::<crate::AccountId, <Runtime as pallet_aaa::Config>::MaxWhitelistSize>::try_from(
        within_limit
      )
      .is_ok()
    );
    assert!(
      BoundedVec::<crate::AccountId, <Runtime as pallet_aaa::Config>::MaxWhitelistSize>::try_from(
        above_limit
      )
      .is_err()
    );
  });
}

#[test]
fn fund_aaa_rejects_asset_not_tracked_by_execution_plan() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let execution_plan = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(10)),
    })])
    .expect("execution_plan fits");
    let aaa_id = create_user(ALICE, manual_schedule(), None, execution_plan);
    assert_noop!(
      AAA::fund_aaa(
        RuntimeOrigin::signed(ALICE),
        aaa_id,
        AssetKind::from(1u32),
        100
      ),
      Error::<Runtime>::SnapshotUnavailable
    );
  });
}

#[test]
fn timer_rejects_execution_delay_above_max() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let max_delay: u32 = <Runtime as pallet_aaa::Config>::MaxExecutionDelayBlocks::get();
    let schedule = Schedule {
      trigger: Trigger::Timer {
        every_blocks: max_delay.saturating_add(1),
        probability: None,
      },
      cooldown_blocks: 0,
    };
    assert_noop!(
      AAA::create_user_aaa(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        schedule,
        None,
        transfer_execution_plan(BOB, AssetKind::Native, 1),
      ),
      Error::<Runtime>::ExecutionDelayTooLong
    );
  });
}

// --- AAA Platform: Trigger & Source Filter ---

#[test]
fn on_address_event_owner_only_respects_source_filter() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 1_000u128;
    let aaa_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    fund_native(aaa_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    AAA::notify_address_event(aaa_id, AssetKind::Native, 100, &BOB);
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    AAA::notify_address_event(aaa_id, AssetKind::Native, 100, &ALICE);
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
  });
}

#[test]
fn on_address_event_asset_filter_is_enforced() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 1_000u128;
    let asset_whitelist = BoundedVec::try_from(vec![AssetKind::Local(ASSET_A)]).expect("fits");
    let aaa_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Whitelist(asset_whitelist)),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    fund_native(aaa_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    AAA::notify_address_event(aaa_id, AssetKind::Native, 100, &ALICE);
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    AAA::notify_address_event(aaa_id, AssetKind::Local(ASSET_A), 100, &ALICE);
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
  });
}

#[test]
fn on_address_event_without_source_is_ignored_for_filtered_trigger() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 1_000u128;
    let aaa_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    fund_native(aaa_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    AAA::notify_address_event_without_source(aaa_id, AssetKind::Native, 100);
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
  });
}

#[test]
fn asset_ops_transfer_notifies_on_address_event_via_runtime_ingress_adapter() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let receiver_amount = 1_000u128;
    let receiver_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, receiver_amount),
    );
    let receiver_sovereign = aaa_account(receiver_id);
    fund_native(receiver_id, 100_000_000_000_000);
    let sender_id = create_user(
      CHARLIE,
      manual_schedule(),
      None,
      transfer_execution_plan(receiver_sovereign, AssetKind::Native, 5_000),
    );
    let sender_sovereign = aaa_account(sender_id);
    let sender_whitelist = BoundedVec::try_from(vec![sender_sovereign]).expect("fits");
    assert_ok!(AAA::update_schedule(
      RuntimeOrigin::signed(ALICE),
      receiver_id,
      on_address_event_schedule(SourceFilter::Whitelist(sender_whitelist), AssetFilter::Any),
      None,
    ));
    fund_native(sender_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(AAA::manual_trigger(
      RuntimeOrigin::signed(CHARLIE),
      sender_id
    ));
    run_idle(Weight::MAX);
    System::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&BOB),
      bob_before.saturating_add(receiver_amount)
    );
  });
}

#[test]
fn router_fee_routing_notifies_burning_manager_via_runtime_ingress_adapter() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let bm_id = primitives::ecosystem::aaa_ids::BURNING_MANAGER_AAA_ID;
    assert_ok!(AAA::update_schedule(
      RuntimeOrigin::root(),
      bm_id,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
    ));
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::root(),
      bm_id,
      transfer_execution_plan(BOB, AssetKind::Native, 777),
    ));
    let bob_before = native_balance(&BOB);
    assert_ok!(
      crate::configs::axial_router_config::FeeManagerImpl::<Runtime>::route_fee(
        &ALICE,
        AssetKind::Native,
        10_000,
      )
    );
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(777));
  });
}

#[test]
fn direct_pallet_assets_transfer_to_sovereign_triggers_on_address_event() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 222u128;
    let aaa_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    let sovereign = aaa_account(aaa_id);
    fund_native(aaa_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      sovereign.into(),
      10_000,
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
  });
}

#[test]
fn direct_pallet_assets_mint_to_sovereign_with_owner_only_is_ignored() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 222u128;
    let aaa_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    let sovereign = aaa_account(aaa_id);
    fund_native(aaa_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Assets::mint(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      sovereign.into(),
      10_000,
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
  });
}

#[test]
fn ingress_adapter_without_source_matches_any_source_filter() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let receiver_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 333),
    );
    let receiver_sovereign = aaa_account(receiver_id);
    fund_native(receiver_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    <RuntimeAddressEventIngress as AddressEventIngress>::on_inbound_without_source(
      &receiver_sovereign,
      AssetKind::Native,
      5_000,
    );
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(333));
  });
}

#[test]
fn ingress_adapter_without_source_is_ignored_by_owner_only_filter() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let receiver_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 333),
    );
    let receiver_sovereign = aaa_account(receiver_id);
    fund_native(receiver_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    <RuntimeAddressEventIngress as AddressEventIngress>::on_inbound_without_source(
      &receiver_sovereign,
      AssetKind::Native,
      5_000,
    );
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
  });
}

#[test]
fn transfer_ingress_updates_system_snapshot_without_pause_resume() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let execution_plan = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: AssetKind::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    })])
    .expect("execution_plan fits");
    let target_id = create_system(ALICE, manual_schedule(), None, execution_plan);
    fund_native_via_call(ALICE, target_id, 10_000_000_000_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), target_id));
    run_idle_until_cycle_nonce(target_id, 1);
    System::set_block_number(2);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), target_id));
    run_idle_until_cycle_nonce(target_id, 2);
    System::set_block_number(3);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), target_id));
    run_idle_until_cycle_nonce(target_id, 3);
    let instance = AAA::aaa_instances(target_id).expect("AAA exists");
    assert!(!instance.is_paused);
    assert_eq!(instance.pause_reason, None);
    let target_sovereign = aaa_account(target_id);
    let refill_amount = 8_000_000_000_000u128;
    let sender_id = create_user(
      CHARLIE,
      manual_schedule(),
      None,
      transfer_execution_plan(target_sovereign, AssetKind::Native, refill_amount),
    );
    fund_native(sender_id, 100_000_000_000_000);
    assert_ok!(AAA::manual_trigger(
      RuntimeOrigin::signed(CHARLIE),
      sender_id
    ));
    run_idle(Weight::MAX);
    let updated = AAA::aaa_instances(target_id).expect("AAA exists");
    assert_eq!(
      updated
        .funding_snapshots
        .get(&AssetKind::Native)
        .expect("snapshot")
        .amount,
      refill_amount
    );
    assert!(!has_aaa_event(|event| {
      matches!(event, Event::AaaResumed { aaa_id: id } if *id == target_id)
    }));
  });
}

#[test]
fn xcm_ingress_with_source_triggers_owner_only_on_address_event() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 444u128;
    let aaa_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    let sovereign = aaa_account(aaa_id);
    fund_native(aaa_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    let recipient = account_location(sovereign);
    let origin = account_location(ALICE);
    let context = xcm::latest::XcmContext {
      origin: Some(origin),
      message_id: [7u8; 32],
      topic: None,
    };
    let asset = native_xcm_asset(5_000);
    assert!(
      <crate::configs::AaaAwareAssetTransactor as TransactAsset>::deposit_asset(
        asset_to_holding(asset),
        &recipient,
        Some(&context),
      )
      .is_ok()
    );
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
  });
}

#[test]
fn xcm_ingress_without_source_is_ignored_for_owner_only() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 444u128;
    let aaa_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    let sovereign = aaa_account(aaa_id);
    fund_native(aaa_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    let recipient = account_location(sovereign);
    let asset = native_xcm_asset(5_000);
    assert!(
      <crate::configs::AaaAwareAssetTransactor as TransactAsset>::deposit_asset(
        asset_to_holding(asset),
        &recipient,
        None,
      )
      .is_ok()
    );
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
  });
}

#[test]
fn xcm_mixed_ingress_single_deposit_triggers_single_cycle() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 444u128;
    let aaa_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    let sovereign = aaa_account(aaa_id);
    fund_native(aaa_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    let recipient = account_location(sovereign);
    let origin = account_location(ALICE);
    let context = xcm::latest::XcmContext {
      origin: Some(origin),
      message_id: [9u8; 32],
      topic: None,
    };
    let asset = native_xcm_asset(5_000);
    assert!(
      <crate::configs::AaaAwareAssetTransactor as TransactAsset>::deposit_asset(
        asset_to_holding(asset),
        &recipient,
        Some(&context),
      )
      .is_ok()
    );
    run_idle(Weight::MAX);
    run_idle(Weight::MAX);
    let instance = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert_eq!(instance.cycle_nonce, 1);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
  });
}

#[test]
fn ingress_hook_scan_weight_is_capped_by_runtime_limit() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let aaa_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    let sovereign = aaa_account(aaa_id);
    System::reset_events();
    let admit_cap = crate::configs::aaa_config::AaaMaxIngressEventsPerBlock::get();
    let scan_cap = crate::configs::aaa_config::AaaMaxIngressScanEventsPerBlock::get();
    let transfer_count = admit_cap.saturating_add(10);
    for _ in 0..transfer_count {
      assert_ok!(Assets::transfer(
        RuntimeOrigin::signed(ALICE),
        ASSET_A,
        sovereign.clone().into(),
        1,
      ));
    }
    let scanned = u64::from(System::event_count().min(scan_cap));
    let weight = <RuntimeAddressEventIngressHook as pallet_aaa::AddressEventIngressHook<
      crate::BlockNumber,
    >>::ingest(System::block_number());
    let queued = 0u64;
    let expected = scanned.saturating_mul(1_000);
    assert_eq!(weight.ref_time(), expected);
    assert_eq!(u64::from(AAA::ingress_overflow_len()), queued);
  });
}

#[test]
fn ingress_overflow_queue_carries_events_to_next_block() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 321u128;
    let aaa_id = create_user(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    fund_native(aaa_id, 100_000_000_000_000);
    assert!(AAA::queue_address_event(
      aaa_id,
      AssetKind::Native,
      5_000,
      Some(ALICE)
    ));
    assert_eq!(AAA::ingress_overflow_len(), 1);
    let bob_before = native_balance(&BOB);
    System::set_block_number(2);
    run_idle(Weight::MAX);
    let after = AAA::aaa_instances(aaa_id).expect("AAA exists");
    assert_eq!(after.cycle_nonce, 1);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(amount));
    assert_eq!(AAA::ingress_overflow_len(), 0);
  });
}

// --- AAA Platform: Scheduling & Budget ---

#[test]
fn cycle_defers_with_weight_reason_when_budget_is_too_small() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let heavy_task = Task::RemoveLiquidity {
      lp_asset: AssetKind::Local(ASSET_A),
      amount: AmountResolution::Fixed(1),
    };
    let step = make_step(heavy_task);
    let execution_plan =
      BoundedVec::try_from(vec![step.clone(), step.clone(), step]).expect("execution_plan fits");
    let aaa_id = create_user(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, 1_000_000_000_000);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    let cycle_weight_upper = AAA::aaa_instances(aaa_id)
      .expect("AAA exists")
      .cycle_weight_upper;
    assert_ok!(AAA::set_global_circuit_breaker(RuntimeOrigin::root(), true));
    let housekeeping_weight = AAA::on_idle(System::block_number(), Weight::MAX);
    assert_ok!(AAA::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      false
    ));
    System::set_block_number(2);
    let target_weight = housekeeping_weight
      .saturating_add(cycle_weight_upper)
      .saturating_sub(Weight::from_parts(1, 0));
    run_idle(target_weight);
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
fn cycle_closes_with_fee_budget_exhausted_when_fee_reserve_is_missing() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let heavy_task = Task::RemoveLiquidity {
      lp_asset: AssetKind::Local(ASSET_A),
      amount: AmountResolution::Fixed(1),
    };
    let step = make_step(heavy_task.clone());
    let execution_plan =
      BoundedVec::try_from(vec![step.clone(), step.clone(), step]).expect("execution_plan fits");
    let per_step_fee_upper = <Runtime as pallet_aaa::Config>::StepBaseFee::get().saturating_add(
      crate::WeightToFee::weight_to_fee(&AAA::weight_upper_bound(&heavy_task)),
    );
    let cycle_fee_upper = per_step_fee_upper.saturating_mul(3);
    let min_balance = <Runtime as pallet_aaa::Config>::MinUserBalance::get();
    assert!(
      cycle_fee_upper > min_balance,
      "test requires cycle_fee_upper > MinUserBalance"
    );
    let aaa_id = create_user(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, cycle_fee_upper.saturating_sub(1));
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
fn fee_insufficiency_is_terminal_without_deferral_guard() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let heavy_task = Task::RemoveLiquidity {
      lp_asset: AssetKind::Local(ASSET_A),
      amount: AmountResolution::Fixed(1),
    };
    let step = make_step(heavy_task.clone());
    let execution_plan =
      BoundedVec::try_from(vec![step.clone(), step.clone(), step]).expect("execution_plan fits");
    let per_step_fee_upper = <Runtime as pallet_aaa::Config>::StepBaseFee::get().saturating_add(
      crate::WeightToFee::weight_to_fee(&AAA::weight_upper_bound(&heavy_task)),
    );
    let cycle_fee_upper = per_step_fee_upper.saturating_mul(3);
    let aaa_id = create_user(ALICE, manual_schedule(), None, execution_plan);
    fund_native(aaa_id, cycle_fee_upper.saturating_sub(1));
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    run_idle(Weight::MAX);
    assert!(AAA::aaa_instances(aaa_id).is_none());
    assert!(!has_aaa_event(|event| {
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
fn scheduler_falls_back_to_non_empty_class_when_preferred_queue_is_empty() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let execution_plan =
      BoundedVec::try_from(vec![make_step(Task::Noop)]).expect("execution_plan fits");
    let schedule = Schedule {
      trigger: Trigger::Timer {
        every_blocks: 1,
        probability: None,
      },
      cooldown_blocks: 0,
    };
    let aaa_id = create_system(ALICE, schedule, None, execution_plan);
    System::set_block_number(2);
    run_idle(Weight::MAX);
    let instance = AAA::aaa_instances(aaa_id).expect("system AAA exists");
    assert_eq!(instance.cycle_nonce, 1);
  });
}

#[test]
fn scheduler_cycle_started_order_is_deterministic_for_equal_state() {
  let cases = [(2u32, 2u32), (3u32, 3u32), (4u32, 2u32)];
  for (system_count, user_count) in cases {
    let run_case = || -> alloc::vec::Vec<AaaId> {
      seeded_test_ext().execute_with(|| {
        System::set_block_number(1);
        let schedule = Schedule {
          trigger: Trigger::Timer {
            every_blocks: 1,
            probability: None,
          },
          cooldown_blocks: 0,
        };
        let execution_plan =
          BoundedVec::try_from(vec![make_step(Task::Noop)]).expect("execution_plan fits");
        let mut tracked: alloc::vec::Vec<AaaId> = alloc::vec::Vec::new();
        for _ in 0..system_count {
          tracked.push(create_system(
            ALICE,
            schedule.clone(),
            None,
            execution_plan.clone(),
          ));
        }
        for _ in 0..user_count {
          let user_id = create_user(ALICE, schedule.clone(), None, execution_plan.clone());
          fund_native(user_id, 100_000_000_000);
          tracked.push(user_id);
        }
        run_idle(Weight::MAX);
        aaa_events()
          .into_iter()
          .filter_map(|event| match event {
            Event::CycleStarted { aaa_id, .. } if tracked.contains(&aaa_id) => Some(aaa_id),
            _ => None,
          })
          .collect()
      })
    };
    let first = run_case();
    let second = run_case();
    assert_eq!(
      first, second,
      "cycle order must be deterministic for system_count={}, user_count={}",
      system_count, user_count
    );
  }
}

#[test]
fn swap_task_weight_upper_bound_is_not_below_router_benchmark_swap() {
  seeded_test_ext().execute_with(|| {
    let task = Task::SwapExactIn {
      asset_in: AssetKind::Native,
      asset_out: AssetKind::Local(ASSET_A),
      amount_in: AmountResolution::Fixed(1),
      slippage_tolerance: Perbill::from_percent(1),
    };
    let aaa_upper = AAA::weight_upper_bound(&task);
    let router_swap = <crate::weights::pallet_axial_router::SubstrateWeight<Runtime> as pallet_axial_router::WeightInfo>::swap();
    assert!(
      aaa_upper.ref_time() >= router_swap.ref_time(),
      "AAA swap upper bound must stay >= axial-router swap benchmark"
    );
  });
}

#[test]
fn liquidity_tasks_share_single_weight_bucket() {
  seeded_test_ext().execute_with(|| {
    let add = Task::AddLiquidity {
      asset_a: AssetKind::Native,
      asset_b: AssetKind::Local(ASSET_A),
      amount_a: AmountResolution::Fixed(1),
      amount_b: AmountResolution::Fixed(1),
    };
    let remove = Task::RemoveLiquidity {
      lp_asset: AssetKind::Local(ASSET_A),
      amount: AmountResolution::Fixed(1),
    };
    assert_eq!(
      AAA::weight_upper_bound(&add),
      AAA::weight_upper_bound(&remove)
    );
  });
}

#[test]
fn split_transfer_task_weight_clamps_to_runtime_limit() {
  seeded_test_ext().execute_with(|| {
    let max_legs = <<Runtime as pallet_aaa::Config>::MaxSplitTransferLegs as Get<u32>>::get();
    let at_limit = <<Runtime as pallet_aaa::Config>::TaskWeightInfo as pallet_aaa::TaskWeightInfo>::split_transfer(max_legs);
    let above_limit = <<Runtime as pallet_aaa::Config>::TaskWeightInfo as pallet_aaa::TaskWeightInfo>::split_transfer(max_legs.saturating_add(32));
    assert_eq!(at_limit, above_limit);
  });
}

#[test]
fn on_initialize_does_not_execute_cycles_after_starvation() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let amount = 1_000u128;
    let aaa_id = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, amount),
    );
    fund_native(aaa_id, 100_000_000_000_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(AAA::manual_trigger(RuntimeOrigin::signed(ALICE), aaa_id));
    let starvation_threshold =
      <<Runtime as pallet_aaa::Config>::MaxIdleStarvationBlocks as Get<u32>>::get();
    IdleStarvationBlocks::<Runtime>::put(starvation_threshold.saturating_add(1));
    System::set_block_number(2);
    let _ = AAA::on_initialize(2);
    assert_eq!(native_balance(&BOB), bob_before);
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
fn starvation_emits_observability_event_once_on_threshold_crossing() {
  seeded_test_ext().execute_with(|| {
    let threshold = <<Runtime as pallet_aaa::Config>::MaxIdleStarvationBlocks as Get<u32>>::get();
    for block in 1..=(threshold + 2) {
      System::set_block_number(block);
      run_idle(Weight::zero());
    }
    let detections = System::events()
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
fn starvation_resets_after_positive_post_housekeeping_budget() {
  seeded_test_ext().execute_with(|| {
    let threshold = <<Runtime as pallet_aaa::Config>::MaxIdleStarvationBlocks as Get<u32>>::get();
    for block in 1..threshold {
      System::set_block_number(block);
      run_idle(Weight::zero());
    }
    assert_eq!(
      IdleStarvationBlocks::<Runtime>::get(),
      threshold.saturating_sub(1)
    );
    System::set_block_number(threshold);
    run_idle(Weight::MAX);
    assert_eq!(IdleStarvationBlocks::<Runtime>::get(), 0);
    System::set_block_number(threshold.saturating_add(1));
    run_idle(Weight::zero());
    assert_eq!(IdleStarvationBlocks::<Runtime>::get(), 1);
    assert!(!has_aaa_event(|event| matches!(
      event,
      Event::IdleStarvationDetected { .. }
    )));
  });
}

// --- AAA Platform: Owner Slots ---

#[test]
fn system_aaa_count_is_not_limited_by_owner_slots() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let attempts = <<Runtime as pallet_aaa::Config>::MaxOwnerSlots as Get<u8>>::get() as u64 + 2;
    let mut sovereign_accounts: Vec<crate::AccountId> = Vec::new();
    for _ in 0..attempts {
      let aaa_id = create_system(
        ALICE,
        manual_schedule(),
        None,
        transfer_execution_plan(BOB, AssetKind::Native, 1),
      );
      let inst = AAA::aaa_instances(aaa_id).expect("AAA exists");
      assert_eq!(inst.owner_slot, 0);
      sovereign_accounts.push(inst.sovereign_account);
    }
    assert_eq!(AAA::owner_slot_mask(ALICE), 0);
    for i in 0..sovereign_accounts.len() {
      for j in i + 1..sovereign_accounts.len() {
        assert_ne!(sovereign_accounts[i], sovereign_accounts[j]);
      }
    }
  });
}

#[test]
fn governance_can_update_active_actor_limit() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let max_limit = <Runtime as pallet_aaa::Config>::MaxActiveActors::get();
    assert_ok!(AAA::set_active_actor_limit(
      RuntimeOrigin::root(),
      max_limit - 1,
    ));
    assert_eq!(
      pallet_aaa::ActiveActorLimit::<Runtime>::get(),
      max_limit - 1
    );
    let aaa_id = create_system(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    assert!(AAA::aaa_instances(aaa_id).is_some());
    assert_noop!(
      AAA::set_active_actor_limit(RuntimeOrigin::root(), 0),
      pallet_aaa::Error::<Runtime>::ActiveAaaLimitTooLow
    );
    assert_noop!(
      AAA::set_active_actor_limit(RuntimeOrigin::root(), u32::MAX),
      pallet_aaa::Error::<Runtime>::ActiveAaaLimitTooHigh
    );
  });
}

#[test]
fn owner_slot_reuses_freed_slot_after_close() {
  seeded_test_ext().execute_with(|| {
    System::set_block_number(1);
    let id0 = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    let id1 = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    let slot0 = AAA::aaa_instances(id0).expect("id0 exists").owner_slot;
    let slot1 = AAA::aaa_instances(id1).expect("id1 exists").owner_slot;
    assert_eq!(slot0, 0);
    assert_eq!(slot1, 1);
    assert_ok!(AAA::close_aaa(RuntimeOrigin::signed(ALICE), id0));
    let id2 = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    let slot2 = AAA::aaa_instances(id2).expect("id2 exists").owner_slot;
    assert_eq!(slot2, slot0);
  });
}

// --- User DCA Lifecycle ---

#[test]
fn user_dca_e2e_lifecycle_with_natural_close() {
  use pallet_aaa::CloseReason;
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let create_fee = <Runtime as pallet_aaa::Config>::AaaCreationFee::get();
    let initial_alice_balance = Balances::free_balance(&ALICE);
    let schedule = Schedule {
      trigger: Trigger::Timer {
        every_blocks: 5,
        probability: None,
      },
      cooldown_blocks: 0,
    };
    let foreign = AssetKind::Local(ASSET_A);
    let swap_amount = 50 * primitives::ecosystem::params::PRECISION;
    let execution_plan = BoundedVec::try_from(vec![StepOf::<Runtime> {
      conditions: BoundedVec::default(),
      task: Task::SwapExactIn {
        asset_in: AssetKind::Native,
        asset_out: foreign,
        amount_in: AmountResolution::Fixed(swap_amount),
        slippage_tolerance: Perbill::from_percent(5),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }])
    .unwrap();
    let id = create_user(ALICE, schedule, None, execution_plan.clone());
    assert!(has_aaa_event(
      |e| matches!(e, Event::AaaCreated { aaa_id, .. } if *aaa_id == id)
    ));
    assert_eq!(
      Balances::free_balance(&ALICE),
      initial_alice_balance - create_fee
    );
    let sov = AAA::sovereign_account_id(&ALICE, 0);
    let min_user_balance = <Runtime as pallet_aaa::Config>::MinUserBalance::get();
    let inst = AAA::aaa_instances(id).unwrap();
    let per_cycle_fee = inst.cycle_fee_upper;
    let native_funding = min_user_balance + (per_cycle_fee + swap_amount) * 3;
    let _ = <Balances as Currency<crate::AccountId>>::transfer(
      &ALICE,
      &sov,
      native_funding,
      polkadot_sdk::frame_support::traits::ExistenceRequirement::KeepAlive,
    );
    // Cycle 1 + 2 + 3 + Close
    let mut closed = false;
    let mut max_nonce = 0;
    for block in 2..=300 {
      System::set_block_number(block);
      AAA::on_initialize(block);
      AAA::on_idle(block, Weight::MAX);
      for event in System::events() {
        match event.event {
          RuntimeEvent::AAA(Event::CycleSummary {
            aaa_id: ev_id,
            cycle_nonce,
            ..
          }) if ev_id == id => {
            if cycle_nonce > max_nonce {
              max_nonce = cycle_nonce;
            }
          }
          RuntimeEvent::AAA(Event::AaaClosed {
            aaa_id: ev_id,
            reason: CloseReason::BalanceExhausted,
          }) if ev_id == id => {
            closed = true;
          }
          _ => {}
        }
      }
      System::reset_events();
      if closed {
        break;
      }
    }
    assert!(closed, "User AAA should be closed due to BalanceExhausted");
    assert!(max_nonce >= 2, "Should have executed at least 2 cycles");
    let id_new = create_user(
      ALICE,
      manual_schedule(),
      None,
      transfer_execution_plan(BOB, AssetKind::Native, 1),
    );
    let slot_new = AAA::aaa_instances(id_new)
      .expect("id_new exists")
      .owner_slot;
    assert_eq!(slot_new, 0);
  });
}

// --- Circular Transfer Chain Stress Tests ---

/// Helper: creates `n` System AAAs with Noop execution_plan for scheduler stress testing.
/// Returns aaa_ids.
fn setup_noop_actors(n: u64, initial_balance: u128) -> alloc::vec::Vec<u64> {
  let mut aaa_ids: alloc::vec::Vec<u64> = alloc::vec::Vec::new();
  for _ in 0..n {
    let aaa_id = crate::AAA::next_aaa_id();
    aaa_ids.push(aaa_id);
    assert_ok!(AAA::create_system_aaa(
      RuntimeOrigin::root(),
      ALICE,
      Schedule {
        trigger: Trigger::Timer {
          every_blocks: 1,
          probability: None,
        },
        cooldown_blocks: 0,
      },
      None,
      alloc::vec![pallet_aaa::Step {
        conditions: Default::default(),
        task: Task::Noop,
        on_error: StepErrorPolicy::AbortCycle,
      }]
      .try_into()
      .expect("fits"),
    ));
    let sov = AAA::sovereign_account_id_system(aaa_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov, initial_balance);
  }
  aaa_ids
}

fn setup_noop_actors_sparse(n: u64, initial_balance: u128, stride: u64) -> alloc::vec::Vec<u64> {
  let mut aaa_ids: alloc::vec::Vec<u64> = alloc::vec::Vec::new();
  let effective_stride = stride.max(2);
  for _ in 0..n {
    let aaa_id = crate::AAA::next_aaa_id();
    aaa_ids.push(aaa_id);
    assert_ok!(AAA::create_system_aaa(
      RuntimeOrigin::root(),
      ALICE,
      Schedule {
        trigger: Trigger::Timer {
          every_blocks: 1,
          probability: None,
        },
        cooldown_blocks: 0,
      },
      None,
      alloc::vec![pallet_aaa::Step {
        conditions: Default::default(),
        task: Task::Noop,
        on_error: StepErrorPolicy::AbortCycle,
      }]
      .try_into()
      .expect("fits"),
    ));
    let sov = AAA::sovereign_account_id_system(aaa_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov, initial_balance);
    let bumped_next = aaa_id.saturating_add(effective_stride);
    pallet_aaa::NextAaaId::<Runtime>::put(bumped_next);
  }
  aaa_ids
}

/// Helper: creates `n` System AAAs in a circular transfer chain.
/// Returns (aaa_ids, sovereign_accounts).
fn setup_circular_chain(
  n: u64,
  initial_balance: u128,
) -> (alloc::vec::Vec<u64>, alloc::vec::Vec<crate::AccountId>) {
  let transfer_pct = Perbill::from_percent(1);
  let mut aaa_ids: alloc::vec::Vec<u64> = alloc::vec::Vec::new();
  let mut sovereign_accounts = alloc::vec::Vec::new();
  for _ in 0..n {
    let aaa_id = crate::AAA::next_aaa_id();
    aaa_ids.push(aaa_id);
    assert_ok!(AAA::create_system_aaa(
      RuntimeOrigin::root(),
      ALICE,
      Schedule {
        trigger: Trigger::Timer {
          every_blocks: 1,
          probability: None
        },
        cooldown_blocks: 0,
      },
      None,
      alloc::vec![pallet_aaa::Step {
        conditions: Default::default(),
        task: Task::Noop,
        on_error: StepErrorPolicy::AbortCycle,
      }]
      .try_into()
      .expect("fits"),
    ));
    let sov = AAA::sovereign_account_id_system(aaa_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov, initial_balance);
    sovereign_accounts.push(sov);
  }
  for i in 0..n {
    let next_sov = sovereign_accounts[((i + 1) % n) as usize].clone();
    let execution_plan: ExecutionPlanOf<Runtime> = alloc::vec![pallet_aaa::Step {
      conditions: alloc::vec![pallet_aaa::Condition::BalanceAbove {
        asset: primitives::AssetKind::Native,
        threshold: crate::EXISTENTIAL_DEPOSIT,
      }]
      .try_into()
      .expect("fits"),
      task: Task::Transfer {
        to: next_sov,
        asset: primitives::AssetKind::Native,
        amount: AmountResolution::PercentageOfCurrent(transfer_pct),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("fits");
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::root(),
      aaa_ids[i as usize],
      execution_plan
    ));
  }
  (aaa_ids, sovereign_accounts)
}

/// Per-block diagnostic counters collected during stress run.
struct StressDiagnostics {
  actor_cycle_counts: alloc::collections::BTreeMap<u64, u32>,
  total_deferred_weight: u32,
  total_failed_steps: u32,
  min_per_block: u32,
  max_per_block: u32,
}

struct QueuePressureDiagnostics {
  max_current_queue_len: u32,
  max_wakeup_backlog: u32,
  max_wakeup_buckets: u32,
}

/// Runs `num_blocks` blocks with on_initialize + on_idle, collecting per-block diagnostics.
fn run_blocks_with_diagnostics(
  aaa_ids: &[u64],
  num_blocks: u32,
  weight: Weight,
) -> StressDiagnostics {
  let (diag, _) = run_blocks_with_queue_diagnostics(aaa_ids, num_blocks, weight);
  diag
}

fn run_blocks_with_queue_diagnostics(
  aaa_ids: &[u64],
  num_blocks: u32,
  weight: Weight,
) -> (StressDiagnostics, QueuePressureDiagnostics) {
  let mut diag = StressDiagnostics {
    actor_cycle_counts: aaa_ids.iter().map(|&id| (id, 0u32)).collect(),
    total_deferred_weight: 0,
    total_failed_steps: 0,
    min_per_block: u32::MAX,
    max_per_block: 0,
  };
  let mut queue_diag = QueuePressureDiagnostics {
    max_current_queue_len: 0,
    max_wakeup_backlog: 0,
    max_wakeup_buckets: 0,
  };
  for block in 2..=(num_blocks + 1) {
    System::set_block_number(block);
    System::reset_events();
    AAA::on_initialize(block);
    AAA::on_idle(block, weight);
    let mut block_executions = 0u32;
    for evt in System::events() {
      match &evt.event {
        RuntimeEvent::AAA(Event::CycleSummary {
          aaa_id,
          failed_steps,
          ..
        }) => {
          if let Some(count) = diag.actor_cycle_counts.get_mut(aaa_id) {
            *count += 1;
          }
          block_executions += 1;
          diag.total_failed_steps += failed_steps;
        }
        RuntimeEvent::AAA(Event::CycleDeferred { reason, .. }) => match reason {
          DeferReason::InsufficientWeightBudget => diag.total_deferred_weight += 1,
        },
        _ => {}
      }
    }
    let current_queue_len = pallet_aaa::CurrentQueue::<Runtime>::get().len() as u32;
    let mut wakeup_backlog = 0u32;
    let mut wakeup_buckets = 0u32;
    for (_, queued) in pallet_aaa::WakeupIndex::<Runtime>::iter() {
      wakeup_backlog = wakeup_backlog.saturating_add(queued.len() as u32);
      wakeup_buckets = wakeup_buckets.saturating_add(1);
    }
    queue_diag.max_current_queue_len = queue_diag.max_current_queue_len.max(current_queue_len);
    queue_diag.max_wakeup_backlog = queue_diag.max_wakeup_backlog.max(wakeup_backlog);
    queue_diag.max_wakeup_buckets = queue_diag.max_wakeup_buckets.max(wakeup_buckets);
    diag.min_per_block = diag.min_per_block.min(block_executions);
    diag.max_per_block = diag.max_per_block.max(block_executions);
  }
  (diag, queue_diag)
}

/// Asserts stability invariants that apply regardless of capacity scenario.
fn assert_core_stability(aaa_ids: &[u64], diag: &StressDiagnostics) {
  assert_eq!(
    diag.total_deferred_weight, 0,
    "Weight budget deferrals must be zero with Weight::MAX (got {})",
    diag.total_deferred_weight,
  );

  assert_eq!(
    diag.total_failed_steps, 0,
    "All transfer steps must succeed (got {} failures)",
    diag.total_failed_steps,
  );
  for &id in aaa_ids {
    let inst = pallet_aaa::AaaInstances::<Runtime>::get(id).expect("actor must still exist");
    assert_eq!(
      inst.consecutive_failures, 0,
      "AAA {} has {} consecutive failures",
      id, inst.consecutive_failures,
    );
  }
}

/// Under-capacity: 35 chain actors + ~12 genesis actors active in worst block.
/// Runtime has MaxExecutionsPerBlock=48.
/// Genesis creates 13 System AAAs (cooldown=10): in worst block ~12 compete.
/// 35 + 12 = 47 < 48 → all chain actors must fire every block.
///
/// Asserts: exact balance conservation, 100% per-block coverage, zero deferrals,
/// zero failures, uniform cycle_nonce, zero consecutive_failures.
#[test]
fn circular_chain_under_capacity_every_actor_every_block() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    // 35 chain + ≤12 active genesis = ≤47 < MaxExecutionsPerBlock(48)
    let chain_len = 35u64;
    let num_blocks = 50u32;
    let initial_balance: u128 = 1_000_000 * crate::EXISTENTIAL_DEPOSIT;
    let (aaa_ids, sovereign_accounts) = setup_circular_chain(chain_len, initial_balance);
    let total_before: u128 = sovereign_accounts
      .iter()
      .map(|s| Balances::free_balance(s))
      .sum();
    let diag =
      run_blocks_with_diagnostics(&aaa_ids, num_blocks, Weight::from_parts(u64::MAX, u64::MAX));
    // Balance conservation (exact: System AAAs pay no fees)
    let total_after: u128 = sovereign_accounts
      .iter()
      .map(|s| Balances::free_balance(s))
      .sum();
    assert_eq!(
      total_before,
      total_after,
      "Balance must be exactly conserved: drift={}",
      total_after.abs_diff(total_before),
    );
    // Every chain actor must execute exactly once per block
    for &id in &aaa_ids {
      let count = diag.actor_cycle_counts[&id];
      assert_eq!(
        count, num_blocks,
        "AAA {} executed {}/{} blocks",
        id, count, num_blocks,
      );
    }
    // Throughput: at least chain_len per block (genesis actors add more)
    assert!(
      diag.min_per_block >= chain_len as u32,
      "Min per-block throughput: expected≥{}, got={}",
      chain_len,
      diag.min_per_block,
    );
    // Fairness: all chain actors must have identical cycle_nonce
    let nonces: alloc::vec::Vec<u64> = aaa_ids
      .iter()
      .filter_map(|&id| pallet_aaa::AaaInstances::<Runtime>::get(id).map(|i| i.cycle_nonce))
      .collect();
    let (min_n, max_n) = (*nonces.iter().min().unwrap(), *nonces.iter().max().unwrap());
    assert_eq!(
      min_n, max_n,
      "Fairness: cycle_nonce spread must be 0 (min={}, max={})",
      min_n, max_n
    );
    assert_eq!(
      min_n, num_blocks as u64,
      "cycle_nonce must equal block count"
    );
    assert_core_stability(&aaa_ids, &diag);
  });
}

/// Diagnostic test: trace first 5 blocks in detail (execute_cycle only, no emergency)
#[test]
fn diagnose_over_capacity_first_blocks() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let chain_len = 100u64;
    let initial_balance: u128 = 1_000_000 * crate::EXISTENTIAL_DEPOSIT;
    let (_aaa_ids, _sovereign_accounts) = setup_circular_chain(chain_len, initial_balance);
    println!("\n=== Initial state ===");
    let active_count = pallet_aaa::AaaInstances::<Runtime>::iter_keys().count();
    println!("Active instances len: {}", active_count);
    for block in 2..=6 {
      System::set_block_number(block);
      System::reset_events();
      AAA::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
      let executions: alloc::vec::Vec<u64> = System::events()
        .iter()
        .filter_map(|evt| {
          if let RuntimeEvent::AAA(Event::CycleSummary { aaa_id, .. }) = &evt.event {
            Some(*aaa_id)
          } else {
            None
          }
        })
        .collect();
      let min_id = executions.iter().min().copied();
      let max_id = executions.iter().max().copied();
      println!("\n=== Block {} ===", block);
      println!(
        "Executions: {} (IDs: {:?}..{:?})",
        executions.len(),
        min_id,
        max_id
      );
      // Check zero actors (2006-2020)
      let zero_actors: alloc::vec::Vec<u64> = (2006..=2020).collect();
      let zero_executed: alloc::vec::Vec<u64> = executions
        .iter()
        .filter(|id| zero_actors.contains(id))
        .cloned()
        .collect();
      println!(
        "Zero actors (2006-2020) executed: {} {:?}",
        zero_executed.len(),
        zero_executed
      );
    }
    // After 5 blocks, check nonce of zero actors
    println!("\n=== After 5 blocks ===");
    for id in 2006..=2010 {
      if let Some(inst) = pallet_aaa::AaaInstances::<Runtime>::get(id) {
        println!(
          "AAA {}: cycle_nonce={}, last_cycle_block={}",
          id, inst.cycle_nonce, inst.last_cycle_block
        );
      }
    }
    for id in 2006..=2010 {
      println!(
        "AAA {} present: {}",
        id,
        pallet_aaa::AaaInstances::<Runtime>::contains_key(id)
      );
    }
  });
}

/// Over-capacity fairness: 100 chain actors + 13 genesis compete for
/// MaxExecutionsPerBlock=48 slots. Scheduler must rotate without starvation.
#[test]
fn circular_chain_over_capacity_fair_rotation() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let chain_len = 100u64;
    let num_blocks = 100u32;
    let initial_balance: u128 = 1_000_000 * crate::EXISTENTIAL_DEPOSIT;
    let (aaa_ids, sovereign_accounts) = setup_circular_chain(chain_len, initial_balance);
    let total_before: u128 = sovereign_accounts
      .iter()
      .map(|s| Balances::free_balance(s))
      .sum();
    let diag =
      run_blocks_with_diagnostics(&aaa_ids, num_blocks, Weight::from_parts(u64::MAX, u64::MAX));
    // Balance conservation (exact)
    let total_after: u128 = sovereign_accounts
      .iter()
      .map(|s| Balances::free_balance(s))
      .sum();
    assert_eq!(
      total_before,
      total_after,
      "Balance must be exactly conserved: drift={}",
      total_after.abs_diff(total_before),
    );
    // Per-block execution cap respected
    assert!(
      diag.max_per_block <= 48,
      "Per-block throughput must not exceed MaxExecutionsPerBlock=48 (got {})",
      diag.max_per_block,
    );
    assert!(
      diag.min_per_block > 0,
      "Every block must execute at least some actors",
    );
    // No starvation: every chain actor must have executed multiple times
    let min_count = *diag.actor_cycle_counts.values().min().unwrap();
    let zero_actors: alloc::vec::Vec<u64> = diag
      .actor_cycle_counts
      .iter()
      .filter(|(_id, count)| **count == 0)
      .map(|(id, _)| *id)
      .collect();
    assert!(
      min_count > 0,
      "No starvation: every actor must execute at least once (min_count={}, \
       zero_actors={:?}, active_actors_len={})",
      min_count,
      &zero_actors[..zero_actors.len().min(10)],
      pallet_aaa::AaaInstances::<Runtime>::iter_keys().count(),
    );
    // Fairness: examine cycle_nonce spread across chain actors.
    // With identical periodic actors, the queue scheduler should keep nonce spread minimal (≤ 2).
    let nonces: alloc::vec::Vec<u64> = aaa_ids
      .iter()
      .filter_map(|&id| pallet_aaa::AaaInstances::<Runtime>::get(id).map(|i| i.cycle_nonce))
      .collect();
    let min_nonce = *nonces.iter().min().unwrap();
    let max_nonce = *nonces.iter().max().unwrap();
    let nonce_spread = max_nonce - min_nonce;
    assert!(
      nonce_spread <= 2,
      "Fairness: nonce spread {} exceeds 2 (min={}, max={})",
      nonce_spread,
      min_nonce,
      max_nonce,
    );
    // Total throughput: should utilize most available slots
    let total_executions: u32 = diag.actor_cycle_counts.values().sum();
    let theoretical_max = num_blocks * 48;
    assert!(
      total_executions > theoretical_max * 9 / 10,
      "Total executions {} must exceed 90% of theoretical max {}",
      total_executions,
      theoretical_max,
    );
    assert_core_stability(&aaa_ids, &diag);
  });
}

fn close_genesis_system_actors() {
  let genesis_ids: alloc::vec::Vec<u64> = alloc::vec![0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
  for id in genesis_ids {
    if AAA::aaa_instances(id).is_some() {
      assert_ok!(AAA::close_aaa(RuntimeOrigin::root(), id));
    }
  }
}

fn run_fairness_matrix_case(total_actors: u64, num_blocks: u32) -> StressDiagnostics {
  System::set_block_number(1);
  close_genesis_system_actors();
  assert_eq!(
    pallet_aaa::AaaInstances::<Runtime>::iter_keys().count(),
    0,
    "Genesis actors must be removed for isolated fairness matrix",
  );
  let initial_balance = 10_000u128;
  let aaa_ids = setup_noop_actors(total_actors, initial_balance);
  let active_count = pallet_aaa::AaaInstances::<Runtime>::iter_keys().count() as u64;
  assert_eq!(
    active_count, total_actors,
    "Scenario must start with exact actor count (expected={}, got={})",
    total_actors, active_count,
  );
  let diag = run_blocks_with_diagnostics(&aaa_ids, num_blocks, Weight::MAX);
  let budget = <Runtime as pallet_aaa::Config>::MaxExecutionsPerBlock::get() as u64;
  assert!(
    diag.max_per_block as u64 <= budget,
    "Per-block throughput must not exceed MaxExecutionsPerBlock={} (got {})",
    budget,
    diag.max_per_block,
  );
  let min_count = *diag.actor_cycle_counts.values().min().unwrap() as u64;
  let max_count = *diag.actor_cycle_counts.values().max().unwrap() as u64;
  let spread = max_count.saturating_sub(min_count);
  assert!(
    spread <= 3,
    "Fairness: nonce spread {} exceeds 3 (min={}, max={}, actors={}, blocks={})",
    spread,
    min_count,
    max_count,
    total_actors,
    num_blocks,
  );
  let numerator = (num_blocks as u64).saturating_mul(budget);
  let floor_avg = numerator / total_actors;
  let ceil_avg = numerator.div_ceil(total_actors);
  let lower = floor_avg.saturating_sub(1);
  let upper = ceil_avg.saturating_add(1);
  for (&id, &count) in &diag.actor_cycle_counts {
    let c = count as u64;
    assert!(
      c >= lower,
      "Actor {} under-served: count={} < lower={} (actors={}, blocks={})",
      id,
      c,
      lower,
      total_actors,
      num_blocks,
    );
    assert!(
      c <= upper,
      "Actor {} over-served: count={} > upper={} (actors={}, blocks={})",
      id,
      c,
      upper,
      total_actors,
      num_blocks,
    );
  }
  let full_rotation_blocks = total_actors.div_ceil(budget);
  assert!(
    num_blocks as u64 >= full_rotation_blocks,
    "Scenario blocks {} must cover at least one full rotation {}",
    num_blocks,
    full_rotation_blocks,
  );
  assert_core_stability(&aaa_ids, &diag);
  diag
}

// --- Scheduler Fast Lane (CI) ---

#[test]
fn scheduler_fast_lane_dense_vs_sparse_topology_smoke() {
  use super::common::new_test_ext;
  let scenarios: [(u64, u32, u64); 2] = [(64, 96, 8), (256, 128, 16)];
  for (actors, blocks, stride) in scenarios {
    let dense_diag = new_test_ext().execute_with(|| {
      System::set_block_number(1);
      close_genesis_system_actors();
      let aaa_ids = setup_noop_actors(actors, 10_000u128);
      run_blocks_with_diagnostics(&aaa_ids, blocks, Weight::MAX)
    });
    let sparse_diag = new_test_ext().execute_with(|| {
      System::set_block_number(1);
      close_genesis_system_actors();
      let aaa_ids = setup_noop_actors_sparse(actors, 10_000u128, stride);
      run_blocks_with_diagnostics(&aaa_ids, blocks, Weight::MAX)
    });
    let dense_total: u32 = dense_diag.actor_cycle_counts.values().sum();
    let sparse_total: u32 = sparse_diag.actor_cycle_counts.values().sum();
    assert_eq!(
      dense_total, sparse_total,
      "Topology must not change total execution throughput (actors={}, blocks={}, stride={})",
      actors, blocks, stride,
    );
    let dense_min = *dense_diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let sparse_min = *sparse_diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let dense_max = *dense_diag.actor_cycle_counts.values().max().unwrap_or(&0);
    let sparse_max = *sparse_diag.actor_cycle_counts.values().max().unwrap_or(&0);
    assert!(
      dense_min > 0 && sparse_min > 0,
      "No starvation allowed for dense or sparse topology (actors={}, blocks={})",
      actors,
      blocks,
    );
    assert!(
      dense_max.saturating_sub(dense_min) <= 3,
      "Dense fairness spread exceeded bound=3 (actors={}, blocks={}, min={}, max={})",
      actors,
      blocks,
      dense_min,
      dense_max,
    );
    assert!(
      sparse_max.saturating_sub(sparse_min) <= 3,
      "Sparse fairness spread exceeded bound=3 (actors={}, blocks={}, min={}, max={})",
      actors,
      blocks,
      sparse_min,
      sparse_max,
    );
  }
}

#[test]
fn scheduler_fast_lane_sparse_topology_liveness_smoke() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let actors = 256u64;
    let blocks = 192u32;
    let stride = 32u64;
    let aaa_ids = setup_noop_actors_sparse(actors, 10_000u128, stride);
    let diag = run_blocks_with_diagnostics(&aaa_ids, blocks, Weight::MAX);
    let min_count = *diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let max_count = *diag.actor_cycle_counts.values().max().unwrap_or(&0);
    assert!(
      min_count > 0,
      "Sparse topology smoke must remain starvation-free (actors={}, blocks={}, stride={})",
      actors,
      blocks,
      stride,
    );
    assert!(
      max_count.saturating_sub(min_count) <= 3,
      "Sparse fairness spread must stay bounded by 3 (min={}, max={})",
      min_count,
      max_count,
    );
  });
}

// --- Scheduler Stress Lane (scheduled/nightly) ---

#[test]
#[ignore] // Heavy: run in scheduled lane (release mode)
fn scheduler_stress_lane_over_capacity_fairness_matrix() {
  use super::common::new_test_ext;
  let scenarios: [(u64, u32); 4] = [(48, 96), (100, 150), (1000, 252), (10_000, 418)];
  for (actors, blocks) in scenarios {
    new_test_ext().execute_with(|| {
      let _ = run_fairness_matrix_case(actors, blocks);
    });
  }
}

#[test]
#[ignore] // Heavy topology matrix, run in scheduled lane
fn scheduler_stress_lane_dense_vs_sparse_topology_matrix() {
  use super::common::new_test_ext;
  let scenarios: [(u64, u32, u64); 3] = [(100, 200, 8), (1000, 300, 16), (5000, 420, 32)];
  for (actors, blocks, stride) in scenarios {
    let dense_diag = new_test_ext().execute_with(|| {
      System::set_block_number(1);
      close_genesis_system_actors();
      let aaa_ids = setup_noop_actors(actors, 10_000u128);
      run_blocks_with_diagnostics(&aaa_ids, blocks, Weight::MAX)
    });
    let sparse_diag = new_test_ext().execute_with(|| {
      System::set_block_number(1);
      close_genesis_system_actors();
      let aaa_ids = setup_noop_actors_sparse(actors, 10_000u128, stride);
      run_blocks_with_diagnostics(&aaa_ids, blocks, Weight::MAX)
    });
    let dense_total: u32 = dense_diag.actor_cycle_counts.values().sum();
    let sparse_total: u32 = sparse_diag.actor_cycle_counts.values().sum();
    assert_eq!(
      dense_total, sparse_total,
      "Topology must not change total execution throughput (actors={}, blocks={}, stride={})",
      actors, blocks, stride,
    );
    let dense_min = *dense_diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let sparse_min = *sparse_diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let dense_max = *dense_diag.actor_cycle_counts.values().max().unwrap_or(&0);
    let sparse_max = *sparse_diag.actor_cycle_counts.values().max().unwrap_or(&0);
    assert!(
      dense_min > 0 && sparse_min > 0,
      "No starvation allowed for dense or sparse topology (actors={}, blocks={})",
      actors,
      blocks,
    );
    assert!(
      dense_max.saturating_sub(dense_min) <= 3,
      "Dense fairness spread exceeded bound=3 (actors={}, blocks={}, min={}, max={})",
      actors,
      blocks,
      dense_min,
      dense_max,
    );
    assert!(
      sparse_max.saturating_sub(sparse_min) <= 3,
      "Sparse fairness spread exceeded bound=3 (actors={}, blocks={}, min={}, max={})",
      actors,
      blocks,
      sparse_min,
      sparse_max,
    );
  }
}

#[test]
#[ignore] // Heavy long-run sparse-liveness check, run in scheduled lane
fn scheduler_stress_lane_sparse_topology_long_run_liveness() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let actors = 2000u64;
    let blocks = 1024u32;
    let stride = 32u64;
    let aaa_ids = setup_noop_actors_sparse(actors, 10_000u128, stride);
    let diag = run_blocks_with_diagnostics(&aaa_ids, blocks, Weight::MAX);
    let min_count = *diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let max_count = *diag.actor_cycle_counts.values().max().unwrap_or(&0);
    assert!(
      min_count > 0,
      "Long-run sparse topology must remain starvation-free (actors={}, blocks={}, stride={})",
      actors,
      blocks,
      stride,
    );
    assert!(
      max_count.saturating_sub(min_count) <= 3,
      "Long-run sparse fairness spread must stay bounded by 3 (min={}, max={})",
      min_count,
      max_count,
    );
  });
}

#[test]
#[ignore] // Queue/wakeup occupancy diagnostics for over-capacity stress scenario
fn profile_scheduler_queue_wakeup_occupancy_10k() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    close_genesis_system_actors();
    let actors = 10_000u64;
    let blocks = 418u32;
    let aaa_ids = setup_noop_actors(actors, 10_000u128);
    let (diag, queue_diag) = run_blocks_with_queue_diagnostics(&aaa_ids, blocks, Weight::MAX);
    let min_count = *diag.actor_cycle_counts.values().min().unwrap_or(&0);
    let max_count = *diag.actor_cycle_counts.values().max().unwrap_or(&0);
    let spread = max_count.saturating_sub(min_count);
    println!(
      "AAA queue profile: actors={}, blocks={}, min_cycle_nonce={}, max_cycle_nonce={}, spread={}, max_current_queue_len={}, max_wakeup_backlog={}, max_wakeup_buckets={}",
      actors,
      blocks,
      min_count,
      max_count,
      spread,
      queue_diag.max_current_queue_len,
      queue_diag.max_wakeup_backlog,
      queue_diag.max_wakeup_buckets,
    );
    assert!(min_count > 0, "10k stress profile must remain starvation-free");
  });
}

// Profiling utility: run manually in release mode for wall-clock matrix
#[test]
#[ignore]
fn profile_scheduler_wallclock_matrix() {
  use super::common::new_test_ext;
  use std::time::Instant;
  let scenarios: [(u64, u32); 4] = [(48, 96), (100, 150), (1000, 252), (10_000, 418)];
  for (actors, blocks) in scenarios {
    new_test_ext().execute_with(|| {
      let started = Instant::now();
      let diag = run_fairness_matrix_case(actors, blocks);
      let elapsed = started.elapsed();
      let total_executions: u32 = diag.actor_cycle_counts.values().sum();
      let ms_per_block = (elapsed.as_secs_f64() * 1_000.0) / (blocks as f64);
      println!(
        "AAA scheduler profile: actors={}, blocks={}, elapsed_ms={:.3}, ms_per_block={:.4}, total_executions={}",
        actors,
        blocks,
        elapsed.as_secs_f64() * 1_000.0,
        ms_per_block,
        total_executions,
      );
    });
  }
}

fn run_scheduler_profile_case(
  total_actors: u64,
  num_blocks: u32,
  clear_readiness_before_run: bool,
) -> (f64, u32) {
  use std::time::Instant;
  System::set_block_number(1);
  close_genesis_system_actors();
  let aaa_ids = setup_noop_actors(total_actors, 10_000u128);
  if clear_readiness_before_run {
    for &id in &aaa_ids {
      pallet_aaa::AaaReadiness::<Runtime>::remove(id);
    }
  }
  let started = Instant::now();
  let diag = run_blocks_with_diagnostics(&aaa_ids, num_blocks, Weight::MAX);
  let elapsed = started.elapsed();
  let ms_per_block = (elapsed.as_secs_f64() * 1_000.0) / (num_blocks as f64);
  let total_executions: u32 = diag.actor_cycle_counts.values().sum();
  (ms_per_block, total_executions)
}

#[test]
#[ignore]
fn profile_readiness_hot_state_vs_fallback() {
  use super::common::new_test_ext;
  let actors = 10_000u64;
  let blocks = 64u32;
  let rounds = 3u32;
  let mut hot_samples: alloc::vec::Vec<f64> = alloc::vec::Vec::new();
  let mut fallback_samples: alloc::vec::Vec<f64> = alloc::vec::Vec::new();
  let mut expected_exec: Option<u32> = None;
  for round in 0..rounds {
    let run_hot_first = round % 2 == 0;
    if run_hot_first {
      let (hot_ms, hot_exec) =
        new_test_ext().execute_with(|| run_scheduler_profile_case(actors, blocks, false));
      let (fallback_ms, fallback_exec) =
        new_test_ext().execute_with(|| run_scheduler_profile_case(actors, blocks, true));
      hot_samples.push(hot_ms);
      fallback_samples.push(fallback_ms);
      assert_eq!(
        hot_exec, fallback_exec,
        "Readiness hot-state and fallback paths must preserve execution count semantics",
      );
      if let Some(expected) = expected_exec {
        assert_eq!(
          hot_exec, expected,
          "Execution count must stay stable across rounds"
        );
      } else {
        expected_exec = Some(hot_exec);
      }
    } else {
      let (fallback_ms, fallback_exec) =
        new_test_ext().execute_with(|| run_scheduler_profile_case(actors, blocks, true));
      let (hot_ms, hot_exec) =
        new_test_ext().execute_with(|| run_scheduler_profile_case(actors, blocks, false));
      hot_samples.push(hot_ms);
      fallback_samples.push(fallback_ms);
      assert_eq!(
        hot_exec, fallback_exec,
        "Readiness hot-state and fallback paths must preserve execution count semantics",
      );
      if let Some(expected) = expected_exec {
        assert_eq!(
          hot_exec, expected,
          "Execution count must stay stable across rounds"
        );
      } else {
        expected_exec = Some(hot_exec);
      }
    }
  }
  let hot_avg = hot_samples.iter().sum::<f64>() / (hot_samples.len() as f64);
  let fallback_avg = fallback_samples.iter().sum::<f64>() / (fallback_samples.len() as f64);
  let delta_ms = fallback_avg - hot_avg;
  let delta_pct = if hot_avg > 0.0 {
    (delta_ms / hot_avg) * 100.0
  } else {
    0.0
  };
  println!(
    "AAA readiness profile: actors={}, blocks={}, rounds={}, hot_samples={:?}, fallback_samples={:?}, hot_avg_ms_per_block={:.4}, fallback_avg_ms_per_block={:.4}, delta_ms={:.4}, delta_pct={:.2}%, executions={}",
    actors,
    blocks,
    rounds,
    hot_samples,
    fallback_samples,
    hot_avg,
    fallback_avg,
    delta_ms,
    delta_pct,
    expected_exec.unwrap_or(0),
  );
}

#[test]
fn genesis_sparse_id_space_all_actors_execute_every_block() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let initial_balance: u128 = 1_000_000 * crate::EXISTENTIAL_DEPOSIT;
    // Genesis AAAs occupy IDs: 0, 2-13 (Fee Sink keeps reserved id 1, no actor) (13 actors).
    // NextAaaId = 2005. The gaps (1, 5..999, 1005..2000) are empty.
    // This is a genuine sparse ID space: ~2000 slots, only 13 occupied.
    //
    // Ringless scheduler iterates ActiveActors BTreeSet directly,
    // so sparse IDs are handled efficiently — no scanning over empty slots.
    //
    // Fund all genesis actors so they pass balance checks.
    // ID 0 (Burning Manager) has every_blocks=10, so it won't fire at block 2;
    // all other genesis actors have every_blocks=1.
    let genesis_ids_all: alloc::vec::Vec<u64> =
      alloc::vec![0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,];
    for &id in &genesis_ids_all {
      let sov = AAA::sovereign_account_id_system(id);
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov, initial_balance);
    }
    // Actors expected to fire every block (every_blocks=1, nonce=0 skips cooldown)
    let every_block_genesis: alloc::vec::Vec<u64> =
      alloc::vec![2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,];
    // Create a fresh actor at the high end (ID 2005) to extend the space
    let fresh_id = crate::AAA::next_aaa_id();
    assert_eq!(fresh_id, 14);
    assert_ok!(AAA::create_system_aaa(
      RuntimeOrigin::root(),
      ALICE,
      Schedule {
        trigger: Trigger::Timer {
          every_blocks: 1,
          probability: None
        },
        cooldown_blocks: 0,
      },
      None,
      alloc::vec![pallet_aaa::Step {
        conditions: Default::default(),
        task: Task::Noop,
        on_error: StepErrorPolicy::AbortCycle,
      }]
      .try_into()
      .expect("fits"),
    ));
    let sov_fresh = AAA::sovereign_account_id_system(fresh_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov_fresh, initial_balance);
    let all_ids: alloc::vec::Vec<u64> = {
      let mut ids = every_block_genesis.clone();
      ids.push(fresh_id);
      ids
    };
    // Block 2: first opportunity — all every_blocks=1 actors fire (nonce=0 skips cooldown)
    let block = 2u32;
    System::set_block_number(block);
    System::reset_events();
    AAA::on_initialize(block);
    AAA::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    let executed_block_2: alloc::vec::Vec<_> = System::events()
      .iter()
      .filter_map(|evt| {
        if let RuntimeEvent::AAA(Event::CycleSummary { aaa_id, .. }) = &evt.event {
          Some(*aaa_id)
        } else {
          None
        }
      })
      .collect();
    for &id in &all_ids {
      assert!(
        executed_block_2.contains(&id),
        "AAA {} must execute in first block despite sparse ID gaps \
         (total_actors={}, id_space=0..{}, executed={:?})",
        id,
        all_ids.len(),
        crate::AAA::next_aaa_id(),
        executed_block_2,
      );
    }
    // Genesis actors have cooldown_blocks=10, so they fire again at block 12+.
    // Fresh actor (cooldown=0) fires every block. Advance to block 13 to
    // verify genesis actors survive cooldown in the sparse ID space.
    let block = 13u32;
    System::set_block_number(block);
    System::reset_events();
    AAA::on_initialize(block);
    AAA::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    let executed_block_13: alloc::vec::Vec<_> = System::events()
      .iter()
      .filter_map(|evt| {
        if let RuntimeEvent::AAA(Event::CycleSummary { aaa_id, .. }) = &evt.event {
          Some(*aaa_id)
        } else {
          None
        }
      })
      .collect();
    for &id in &all_ids {
      assert!(
        executed_block_13.contains(&id),
        "AAA {} must execute again after cooldown elapses \
         (block={}, cooldown=10, executed={:?})",
        id,
        block,
        executed_block_13,
      );
    }
  });
}

#[test]
fn execution_order_lower_id_executes_before_higher_id() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let initial_balance: u128 = 1_000_000 * crate::EXISTENTIAL_DEPOSIT;
    // AAA-A (lower ID): transfers 10% of current NTVE to AAA-B sovereign
    let aaa_a_id = crate::AAA::next_aaa_id();
    assert_ok!(AAA::create_system_aaa(
      RuntimeOrigin::root(),
      ALICE,
      Schedule {
        trigger: Trigger::Timer {
          every_blocks: 1,
          probability: None
        },
        cooldown_blocks: 0,
      },
      None,
      alloc::vec![pallet_aaa::Step {
        conditions: Default::default(),
        task: Task::Noop,
        on_error: StepErrorPolicy::AbortCycle,
      }]
      .try_into()
      .expect("fits"),
    ));
    let sov_a = AAA::sovereign_account_id_system(aaa_a_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov_a, initial_balance);
    // AAA-B (higher ID): transfers 10% of current NTVE to CHARLIE
    let aaa_b_id = crate::AAA::next_aaa_id();
    assert!(aaa_b_id > aaa_a_id, "B must have higher ID than A");
    assert_ok!(AAA::create_system_aaa(
      RuntimeOrigin::root(),
      ALICE,
      Schedule {
        trigger: Trigger::Timer {
          every_blocks: 1,
          probability: None
        },
        cooldown_blocks: 0,
      },
      None,
      alloc::vec![pallet_aaa::Step {
        conditions: Default::default(),
        task: Task::Noop,
        on_error: StepErrorPolicy::AbortCycle,
      }]
      .try_into()
      .expect("fits"),
    ));
    let sov_b = AAA::sovereign_account_id_system(aaa_b_id);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&sov_b, initial_balance);
    // Update AAA-A execution_plan: Transfer 10% NTVE → AAA-B sovereign
    let pct = Perbill::from_percent(10);
    let execution_plan_a: ExecutionPlanOf<Runtime> = alloc::vec![pallet_aaa::Step {
      conditions: Default::default(),
      task: Task::Transfer {
        asset: AssetKind::Native.into(),
        amount: AmountResolution::PercentageOfCurrent(pct),
        to: sov_b.clone(),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("fits");
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::root(),
      aaa_a_id,
      execution_plan_a
    ));
    // Update AAA-B execution_plan: Transfer 10% NTVE → CHARLIE
    let execution_plan_b: ExecutionPlanOf<Runtime> = alloc::vec![pallet_aaa::Step {
      conditions: Default::default(),
      task: Task::Transfer {
        asset: AssetKind::Native.into(),
        amount: AmountResolution::PercentageOfCurrent(pct),
        to: CHARLIE,
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .expect("fits");
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::root(),
      aaa_b_id,
      execution_plan_b
    ));
    let charlie_before = Balances::free_balance(CHARLIE);
    // Run one block
    let block = 2u32;
    System::set_block_number(block);
    System::reset_events();
    AAA::on_initialize(block);
    AAA::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    // If A executed before B: A transferred 10% to B, then B has initial + A's transfer,
    // and B transfers 10% of that total to CHARLIE.
    // If B executed before A: B transfers 10% of initial only, then A transfers to B.
    // We can distinguish by checking CHARLIE's balance.
    let a_transfer = pct.mul_floor(initial_balance); // 10% of A's initial
    let b_balance_after_a = initial_balance + a_transfer;
    let b_transfer_correct_order = pct.mul_floor(b_balance_after_a);
    let b_transfer_wrong_order = pct.mul_floor(initial_balance);
    let charlie_after = Balances::free_balance(CHARLIE);
    let charlie_received = charlie_after.saturating_sub(charlie_before);
    assert_eq!(
      charlie_received, b_transfer_correct_order,
      "AAA-A (id={}) must execute before AAA-B (id={}): \
       correct_order_transfer={}, wrong_order_transfer={}, actual={}",
      aaa_a_id, aaa_b_id, b_transfer_correct_order, b_transfer_wrong_order, charlie_received,
    );
    assert_ne!(
      b_transfer_correct_order, b_transfer_wrong_order,
      "Test must distinguish between execution orders"
    );
  });
}

// --- 10K AAA Stress Test ---

/// Validates the queue scheduler at production scale (10,000 active actors).
///
/// Runtime starts with genesis System AAAs already occupying part of the active set.
/// This test fills the remaining capacity so ActiveActors reaches exactly 10,000,
/// then verifies starvation-freedom and fairness for newly added stress actors.
///
/// With MaxExecutionsPerBlock=48, a full rotation takes ceil(10000/48) ≈ 209 blocks.
/// Over 500 blocks, each stress actor should execute approximately floor(500*48/9987) ≈ 2 times.
/// Nonce spread (max - min) must be ≤ 2 for near-perfect fairness.
///
/// Acceptance criteria:
/// - ActiveActors reaches exactly 10,000
/// - Every stress actor executes at least once
/// - Nonce spread ≤ 2
/// - Zero deferrals (System AAAs, Weight::MAX budget)
/// - Zero failed steps
#[test]
#[ignore] // ~30s wall-clock; run manually: cargo test --release stress_10k_actors_queue_scheduler -- --ignored
fn stress_10k_actors_queue_scheduler() {
  use super::common::new_test_ext;
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let num_blocks = 500u32;
    let initial_balance: u128 = 1_000 * crate::EXISTENTIAL_DEPOSIT;
    let max_active = <Runtime as pallet_aaa::Config>::MaxActiveActors::get() as u64;
    // Pause genesis actors to keep them non-ready while still occupying active-set slots.
    // This validates queue fairness with mixed ready/non-ready actors at full capacity.
    let genesis_ids: alloc::vec::Vec<u64> = alloc::vec![0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
    for &id in &genesis_ids {
      let _ = AAA::pause_aaa(RuntimeOrigin::root(), id);
    }
    let active_before = pallet_aaa::AaaInstances::<Runtime>::iter_keys().count() as u64;
    assert!(
      active_before < max_active,
      "Test precondition failed: active_before={} must be < max_active={}",
      active_before,
      max_active,
    );
    let actor_count = max_active - active_before;
    let aaa_ids = setup_noop_actors(actor_count, initial_balance);
    assert_eq!(aaa_ids.len(), actor_count as usize);
    let active_after = pallet_aaa::AaaInstances::<Runtime>::iter_keys().count() as u64;
    assert_eq!(
      active_after, max_active,
      "ActiveActors must be saturated to max capacity",
    );
    let diag = run_blocks_with_diagnostics(&aaa_ids, num_blocks, Weight::MAX);
    // All stress actors must execute at least once
    let zero_actors: alloc::vec::Vec<u64> = aaa_ids
      .iter()
      .filter(|&&id| *diag.actor_cycle_counts.get(&id).unwrap_or(&0) == 0)
      .copied()
      .collect();
    assert!(
      zero_actors.is_empty(),
      "Starvation: {} stress actors never executed (first 10: {:?})",
      zero_actors.len(),
      &zero_actors[..zero_actors.len().min(10)],
    );
    // Fairness: nonce spread ≤ 3
    let nonces: alloc::vec::Vec<u32> = aaa_ids
      .iter()
      .map(|&id| *diag.actor_cycle_counts.get(&id).unwrap_or(&0))
      .collect();
    let min_nonce = *nonces.iter().min().unwrap();
    let max_nonce = *nonces.iter().max().unwrap();
    let nonce_spread = max_nonce - min_nonce;
    assert!(
      nonce_spread <= 3,
      "Fairness: nonce spread {} exceeds 3 (min={}, max={})",
      nonce_spread,
      min_nonce,
      max_nonce,
    );
    // Throughput: per-block cap respected and utilization remains high
    assert!(
      diag.max_per_block <= 48,
      "Per-block executions {} exceeds MaxExecutionsPerBlock=48",
      diag.max_per_block,
    );
    let total_executions: u32 = diag.actor_cycle_counts.values().sum();
    let theoretical_max = num_blocks * 48;
    assert!(
      total_executions > theoretical_max * 9 / 10,
      "Total executions {} should exceed 90% of theoretical max {}",
      total_executions,
      theoretical_max,
    );
    assert_core_stability(&aaa_ids, &diag);
  });
}

#[test]
fn dust_attack_min_balance_actors_preserve_scheduler_stability() {
  seeded_test_ext().execute_with(|| {
    let min_balance = <Runtime as pallet_aaa::Config>::MinUserBalance::get();
    let actor_count = 96u32;
    let baseline_active = pallet_aaa::AaaInstances::<Runtime>::iter_keys().count();
    let mut aaa_ids = Vec::new();
    for i in 0..actor_count {
      let mut owner_bytes = [0u8; 32];
      owner_bytes[0] = (i & 0xFF) as u8;
      owner_bytes[31] = ((i + 17) & 0xFF) as u8;
      let owner = crate::AccountId::from(owner_bytes);
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
        &owner,
        min_balance.saturating_mul(20),
      );
      let schedule = Schedule {
        trigger: Trigger::Timer {
          every_blocks: 1,
          probability: None,
        },
        cooldown_blocks: 0,
      };
      let aaa_id = create_user(
        owner.clone(),
        schedule,
        None,
        transfer_execution_plan(owner, AssetKind::Native, 1),
      );
      let sovereign = aaa_account(aaa_id);
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
        &sovereign,
        min_balance.saturating_mul(10),
      );
      aaa_ids.push(aaa_id);
    }
    let initial_active = pallet_aaa::AaaInstances::<Runtime>::iter_keys().count();
    assert_eq!(initial_active, baseline_active + actor_count as usize);
    for block in 1..=32u32 {
      System::set_block_number(block);
      AAA::on_idle(block, Weight::MAX);
    }
    let final_active = pallet_aaa::AaaInstances::<Runtime>::iter_keys().count();
    let executed = aaa_ids
      .iter()
      .filter(|id| {
        AAA::aaa_instances(**id)
          .map(|inst| inst.cycle_nonce > 0)
          .unwrap_or(false)
      })
      .count();
    assert!(
      executed > 0,
      "Scheduler should execute at least some dust actors"
    );
    assert!(
      final_active > 0,
      "Dust load must not collapse scheduler to zero active actors"
    );
    assert!(
      final_active <= initial_active,
      "Active actors cannot increase without new creations"
    );
  });
}
