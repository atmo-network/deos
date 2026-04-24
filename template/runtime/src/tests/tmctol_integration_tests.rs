//! Integration tests for the TMCTOL economic standard on DEOS runtime.
//!
//! The `tmctol_` prefix is intentional: this module tests the TMCTOL standard
//! (TMC, TOL, Router, Splitter, Zap Manager, Bucket) running on top of the DEOS
//! runtime. Per AGENTS.md Artifact-Name Tail Policy, standard-specific identifiers
//! remain as conscious legacy names; only framework identifiers were migrated.
//! Do not rename this file to `deos_integration_tests`.

use super::common::{
  ALICE, ASSET_A, ASSET_B, add_liquidity, burning_manager_account, create_pool, get_pool_lp_asset,
  new_test_ext, seeded_test_ext, zap_manager_account,
};
use crate::{AAA, Balances, Runtime, RuntimeOrigin, System, TokenMintingCurve};
use pallet_aaa::{AmountResolution, DexOps, Event, ExecutionPlanOf, StepErrorPolicy, Task};
use polkadot_sdk::frame_support::{
  assert_noop, assert_ok,
  traits::{Currency, Hooks, fungibles::Inspect as FungiblesInspect},
  weights::Weight,
};
use polkadot_sdk::sp_runtime::Perbill;
use primitives::AssetKind;
use primitives::ecosystem::aaa_ids;

use super::aaa_integration_tests::has_aaa_event;

// --- Genesis System AAA ---

#[test]
fn genesis_burning_manager_aaa_has_deterministic_sovereign_and_correct_state() {
  new_test_ext().execute_with(|| {
    let aaa_id = aaa_ids::BURNING_MANAGER_AAA_ID;
    let instance = AAA::aaa_instances(aaa_id).expect("Burning Manager AAA must exist at genesis");
    let expected_sovereign = AAA::sovereign_account_id_system(aaa_id);
    assert_eq!(instance.sovereign_account, expected_sovereign);
    assert_eq!(instance.aaa_type, pallet_aaa::AaaType::System);
    assert_eq!(instance.mutability, pallet_aaa::Mutability::Mutable);
    assert!(!instance.is_paused);
    assert_eq!(instance.consecutive_failures, 0);
    assert!(!instance.manual_trigger_pending);
    assert_eq!(
      AAA::next_aaa_id(),
      aaa_ids::NATIVE_STAKING_LP_FARMER_AAA_ID + 1
    );
    assert_eq!(
      pallet_aaa::SovereignIndex::<Runtime>::get(&expected_sovereign),
      Some(aaa_id)
    );
  });
}

#[test]
fn genesis_burning_manager_aaa_sovereign_is_stable_across_rebuilds() {
  let sovereign_a = new_test_ext()
    .execute_with(|| AAA::sovereign_account_id_system(aaa_ids::BURNING_MANAGER_AAA_ID));
  let sovereign_b = new_test_ext()
    .execute_with(|| AAA::sovereign_account_id_system(aaa_ids::BURNING_MANAGER_AAA_ID));
  assert_eq!(sovereign_a, sovereign_b);
}

#[test]
fn genesis_bm_execution_plan_uses_timer_trigger() {
  new_test_ext().execute_with(|| {
    let instance = AAA::aaa_instances(aaa_ids::BURNING_MANAGER_AAA_ID).unwrap();
    assert!(
      matches!(instance.schedule.trigger, pallet_aaa::Trigger::Timer { .. }),
      "BM must use Timer trigger (no coupling with fee source)"
    );
  });
}

// --- Burning Manager ---

#[test]
fn bm_burns_native_on_timer() {
  seeded_test_ext().execute_with(|| {
    let bm = AAA::sovereign_account_id_system(aaa_ids::BURNING_MANAGER_AAA_ID);
    let deposit = 50 * crate::EXISTENTIAL_DEPOSIT;
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&bm, deposit);
    let issuance_before = Balances::total_issuance();
    let bm_balance_before = Balances::free_balance(&bm);
    assert!(bm_balance_before > 0);
    System::set_block_number(11);
    AAA::on_initialize(11);
    AAA::on_idle(11, Weight::from_parts(u64::MAX, u64::MAX));
    let issuance_after = Balances::total_issuance();
    assert!(
      issuance_after < issuance_before,
      "Total issuance must decrease after BM burn"
    );
    let bm_balance_after = Balances::free_balance(&bm);
    assert!(
      bm_balance_after < bm_balance_before,
      "BM sovereign native balance must decrease"
    );
  });
}

#[test]
fn bm_skips_burn_when_balance_below_dust() {
  new_test_ext().execute_with(|| {
    let bm = AAA::sovereign_account_id_system(aaa_ids::BURNING_MANAGER_AAA_ID);
    let _ =
      <Balances as Currency<crate::AccountId>>::deposit_creating(&bm, crate::EXISTENTIAL_DEPOSIT);
    let issuance_before = Balances::total_issuance();
    System::set_block_number(11);
    AAA::on_initialize(11);
    AAA::on_idle(11, Weight::from_parts(u64::MAX, u64::MAX));
    let issuance_after = Balances::total_issuance();
    assert_eq!(
      issuance_before, issuance_after,
      "No burn when balance is below dust threshold"
    );
  });
}

#[test]
fn router_fee_flows_to_bm_sovereign_and_burns_on_next_poll() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let bm_id = aaa_ids::BURNING_MANAGER_AAA_ID;
    let bm = AAA::sovereign_account_id_system(bm_id);
    for block in 11..=30 {
      System::set_block_number(block);
      AAA::on_initialize(block);
      AAA::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    }
    let _ =
      <Balances as Currency<crate::AccountId>>::deposit_creating(&bm, crate::EXISTENTIAL_DEPOSIT);
    let bm_before = Balances::free_balance(&bm);
    let swap_amount = 500 * primitives::ecosystem::params::PRECISION;
    System::set_block_number(31);
    assert_ok!(crate::AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      AssetKind::Native,
      AssetKind::Local(super::common::ASSET_A),
      swap_amount,
      1,
      ALICE,
      System::block_number() + 100,
    ));
    let bm_after_swap = Balances::free_balance(&bm);
    let fee_received = bm_after_swap.saturating_sub(bm_before);
    assert!(fee_received > 0, "BM sovereign must receive router fee");
    let issuance_before_burn = Balances::total_issuance();
    let bm_before_poll = AAA::aaa_instances(bm_id).expect("BM must exist");
    let target_cycle_nonce = bm_before_poll.cycle_nonce.saturating_add(1);
    let cadence = match &bm_before_poll.schedule.trigger {
      pallet_aaa::Trigger::Timer { every_blocks, .. } => *every_blocks,
      _ => panic!("BM must stay timer-driven"),
    };
    let max_wait_blocks = cadence
      .max(bm_before_poll.schedule.cooldown_blocks)
      .saturating_add(2);
    let start_block = System::block_number();
    let mut next_poll_observed = false;
    for offset in 1..=max_wait_blocks {
      let block = start_block.saturating_add(offset);
      System::set_block_number(block);
      AAA::on_initialize(block);
      AAA::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
      if has_aaa_event(|event| {
        matches!(
          event,
          Event::CycleStarted {
            aaa_id,
            cycle_nonce,
          } if *aaa_id == bm_id && *cycle_nonce == target_cycle_nonce
        )
      }) {
        next_poll_observed = true;
        break;
      }
    }
    assert!(
      next_poll_observed,
      "BM must start next timer cycle within cadence/cooldown bound"
    );
    let issuance_after_burn = Balances::total_issuance();
    assert!(
      issuance_after_burn < issuance_before_burn,
      "Fee must be burned on the observed next BM poll"
    );
  });
}

#[test]
fn bm_swap_foreign_to_native_then_burn_via_update_execution_plan() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let bm_id = aaa_ids::BURNING_MANAGER_AAA_ID;
    let bm = AAA::sovereign_account_id_system(bm_id);
    let pre_seeded = crate::Assets::balance(super::common::ASSET_A, &bm);
    if pre_seeded > 0 {
      use polkadot_sdk::frame_support::traits::fungibles::Mutate as FungiblesMutate;
      use polkadot_sdk::frame_support::traits::tokens::{Fortitude, Precision, Preservation};
      let _ = <crate::Assets as FungiblesMutate<crate::AccountId>>::burn_from(
        super::common::ASSET_A,
        &bm,
        pre_seeded,
        Preservation::Expendable,
        Precision::BestEffort,
        Fortitude::Force,
      );
    }
    let price = 1_000_000_000_000u128;
    pallet_axial_router::EmaPrices::<Runtime>::insert(
      AssetKind::Local(super::common::ASSET_A),
      AssetKind::Native,
      price,
    );
    pallet_axial_router::EmaPrices::<Runtime>::insert(
      AssetKind::Native,
      AssetKind::Local(super::common::ASSET_A),
      price,
    );
    let dust = primitives::ecosystem::params::BURNING_MANAGER_DUST_THRESHOLD;
    let new_execution_plan: ExecutionPlanOf<Runtime> = alloc::vec![
      pallet_aaa::Step {
        conditions: alloc::vec![pallet_aaa::Condition::BalanceAbove {
          asset: AssetKind::Local(super::common::ASSET_A),
          threshold: dust,
        }]
        .try_into()
        .unwrap(),
        task: Task::SwapExactIn {
          asset_in: AssetKind::Local(super::common::ASSET_A),
          asset_out: AssetKind::Native,
          amount_in: AmountResolution::AllBalance,
          slippage_tolerance: Perbill::from_percent(5),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      pallet_aaa::Step {
        conditions: alloc::vec![pallet_aaa::Condition::BalanceAbove {
          asset: AssetKind::Native,
          threshold: dust,
        }]
        .try_into()
        .unwrap(),
        task: Task::Burn {
          asset: AssetKind::Native,
          amount: AmountResolution::AllBalance,
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
    ]
    .try_into()
    .unwrap();
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::root(),
      bm_id,
      new_execution_plan
    ));
    let foreign_amount = 2 * primitives::ecosystem::params::PRECISION;
    {
      use polkadot_sdk::frame_support::traits::fungibles::Mutate as FungiblesMutate;
      assert_ok!(
        <crate::Assets as FungiblesMutate<crate::AccountId>>::mint_into(
          super::common::ASSET_A,
          &bm,
          foreign_amount,
        )
      );
    }
    let foreign_before = crate::Assets::balance(super::common::ASSET_A, &bm);
    let issuance_before = Balances::total_issuance();
    for block in 11..=30 {
      System::set_block_number(block);
      AAA::on_initialize(block);
      AAA::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    }
    let foreign_after = crate::Assets::balance(super::common::ASSET_A, &bm);
    assert!(
      foreign_after < foreign_before,
      "Foreign tokens must be swapped"
    );
    let issuance_after = Balances::total_issuance();
    assert!(
      issuance_after < issuance_before,
      "Issuance must decrease after swap+burn"
    );
  });
}

// --- DexOps Adapter ---

#[test]
fn dexops_can_swap_foreign_to_native() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let bm = AAA::sovereign_account_id_system(aaa_ids::BURNING_MANAGER_AAA_ID);
    let price = 1_000_000_000_000u128;
    pallet_axial_router::EmaPrices::<Runtime>::insert(
      AssetKind::Local(super::common::ASSET_A),
      AssetKind::Native,
      price,
    );
    pallet_axial_router::EmaPrices::<Runtime>::insert(
      AssetKind::Native,
      AssetKind::Local(super::common::ASSET_A),
      price,
    );
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &bm,
      10 * crate::EXISTENTIAL_DEPOSIT,
    );
    let foreign_amount = 2 * primitives::ecosystem::params::PRECISION;
    use polkadot_sdk::frame_support::traits::fungibles::Mutate as FungiblesMutate;
    assert_ok!(
      <crate::Assets as FungiblesMutate<crate::AccountId>>::mint_into(
        super::common::ASSET_A,
        &bm,
        foreign_amount,
      )
    );
    let result = <Runtime as pallet_aaa::Config>::DexOps::swap_exact_in(
      &bm,
      AssetKind::Local(super::common::ASSET_A),
      AssetKind::Native,
      foreign_amount,
      Perbill::from_percent(50),
    );
    assert!(
      result.is_ok(),
      "Foreign→Native swap must succeed: {result:?}"
    );
    assert!(result.unwrap() > 0, "Must receive native tokens");
  });
}

#[test]
fn dexops_normal_swap_succeeds() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let bm = AAA::sovereign_account_id_system(aaa_ids::BURNING_MANAGER_AAA_ID);
    let amount = primitives::ecosystem::params::PRECISION;
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&bm, amount * 10);
    let price = 1_000_000_000_000u128;
    pallet_axial_router::EmaPrices::<Runtime>::insert(
      AssetKind::Native,
      AssetKind::Local(super::common::ASSET_A),
      price,
    );
    pallet_axial_router::EmaPrices::<Runtime>::insert(
      AssetKind::Local(super::common::ASSET_A),
      AssetKind::Native,
      price,
    );
    use pallet_aaa::DexOps;
    let result = <Runtime as pallet_aaa::Config>::DexOps::swap_exact_in(
      &bm,
      AssetKind::Native,
      AssetKind::Local(super::common::ASSET_A),
      amount,
      Perbill::from_percent(50),
    );
    assert!(result.is_ok(), "Normal swap must succeed: {result:?}");
    assert!(result.unwrap() > 0);
  });
}

#[test]
fn oracle_deviation_rejects_swap_via_dexops() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let bm = AAA::sovereign_account_id_system(aaa_ids::BURNING_MANAGER_AAA_ID);
    let amount = 10 * primitives::ecosystem::params::PRECISION;
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&bm, amount);
    let deviated_price = 10_000_000_000_000u128;
    pallet_axial_router::EmaPrices::<Runtime>::insert(
      AssetKind::Native,
      AssetKind::Local(super::common::ASSET_A),
      deviated_price,
    );
    let result = <Runtime as pallet_aaa::Config>::DexOps::swap_exact_in(
      &bm,
      AssetKind::Native,
      AssetKind::Local(super::common::ASSET_A),
      amount,
      Perbill::from_percent(50),
    );
    assert!(
      result.is_err(),
      "Swap must fail under oracle deviation: {result:?}"
    );
  });
}

#[test]
fn swap_with_slippage_tolerance_succeeds_under_fair_conditions() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let bm = AAA::sovereign_account_id_system(aaa_ids::BURNING_MANAGER_AAA_ID);
    let amount = primitives::ecosystem::params::PRECISION;
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&bm, amount * 10);
    let price = 1_000_000_000_000u128;
    pallet_axial_router::EmaPrices::<Runtime>::insert(
      AssetKind::Native,
      AssetKind::Local(super::common::ASSET_A),
      price,
    );
    pallet_axial_router::EmaPrices::<Runtime>::insert(
      AssetKind::Local(super::common::ASSET_A),
      AssetKind::Native,
      price,
    );
    let execution_plan: ExecutionPlanOf<Runtime> = alloc::vec![pallet_aaa::Step {
      conditions: Default::default(),
      task: Task::SwapExactIn {
        asset_in: AssetKind::Native,
        asset_out: AssetKind::Local(super::common::ASSET_A),
        amount_in: AmountResolution::Fixed(amount),
        slippage_tolerance: Perbill::from_percent(5),
      },
      on_error: StepErrorPolicy::AbortCycle,
    },]
    .try_into()
    .unwrap();
    let aaa_id = aaa_ids::BURNING_MANAGER_AAA_ID;
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::root(),
      aaa_id,
      execution_plan
    ));
    let balance_before = crate::Assets::balance(super::common::ASSET_A, &bm);
    System::set_block_number(11);
    AAA::on_initialize(11);
    AAA::on_idle(11, Weight::from_parts(u64::MAX, u64::MAX));
    let balance_after = crate::Assets::balance(super::common::ASSET_A, &bm);
    assert!(
      balance_after > balance_before,
      "Swap with 5% slippage tolerance must succeed under fair conditions"
    );
    assert!(has_aaa_event(|event| {
      matches!(event, Event::SwapExecuted { aaa_id: id, .. } if *id == aaa_id)
    }));
  });
}

#[test]
fn swap_without_pool_fails_execution_plan() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let bm_id = aaa_ids::BURNING_MANAGER_AAA_ID;
    let bm = AAA::sovereign_account_id_system(bm_id);
    let execution_plan: ExecutionPlanOf<Runtime> = alloc::vec![pallet_aaa::Step {
      conditions: Default::default(),
      task: Task::SwapExactIn {
        asset_in: AssetKind::Native,
        asset_out: AssetKind::Local(ASSET_A),
        amount_in: AmountResolution::Fixed(1_000_000_000_000),
        slippage_tolerance: Perbill::from_percent(5),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .unwrap();
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::root(),
      bm_id,
      execution_plan
    ));
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &bm,
      100 * primitives::ecosystem::params::PRECISION,
    );
    System::set_block_number(11);
    AAA::on_initialize(11);
    AAA::on_idle(11, Weight::from_parts(u64::MAX, u64::MAX));
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          aaa_id,
          cycle_nonce: 1,
          failed_steps,
          ..
        } if *aaa_id == bm_id && *failed_steps >= 1
      )
    }));
  });
}

// --- Zap Manager ExecutionPlan ---

#[test]
fn zap_execution_plan_builder_produces_valid_3_step_execution_plan() {
  use primitives::ecosystem::aaa_ids;
  seeded_test_ext().execute_with(|| {
    let foreign = AssetKind::Local(ASSET_A);
    let lp_asset = AssetKind::Local(999);
    let dust = primitives::ecosystem::params::BURNING_MANAGER_DUST_THRESHOLD;
    let execution_plan =
      crate::configs::aaa_config::TmctolGenesisSystemAaas::build_zap_execution_plan(
        foreign, lp_asset, dust,
      );
    assert_eq!(
      execution_plan.len(),
      3,
      "ZM execution_plan must have 3 steps"
    );
    assert!(matches!(execution_plan[0].task, Task::AddLiquidity { .. }));
    assert_eq!(
      execution_plan[0].conditions.len(),
      2,
      "AddLiquidity needs dual dust guard"
    );
    if let Task::SwapExactIn {
      asset_in,
      asset_out,
      ..
    } = &execution_plan[1].task
    {
      assert_eq!(*asset_in, foreign);
      assert_eq!(*asset_out, AssetKind::Native);
    } else {
      panic!("Step 2 must be SwapExactIn");
    }
    if let Task::SplitTransfer { asset, legs, .. } = &execution_plan[2].task {
      assert_eq!(*asset, lp_asset);
      assert_eq!(legs.len(), 4, "Must split to 4 TOL buckets");
      assert_eq!(
        legs[0].to,
        AAA::sovereign_account_id_system(aaa_ids::TOL_BUCKET_A_AAA_ID)
      );
      assert_eq!(
        legs[1].to,
        AAA::sovereign_account_id_system(aaa_ids::TOL_BUCKET_B_AAA_ID)
      );
      assert_eq!(
        legs[2].to,
        AAA::sovereign_account_id_system(aaa_ids::TOL_BUCKET_C_AAA_ID)
      );
      assert_eq!(
        legs[3].to,
        AAA::sovereign_account_id_system(aaa_ids::TOL_BUCKET_D_AAA_ID)
      );
      let share_sum: u32 = legs.iter().map(|l| l.share.deconstruct()).sum();
      assert_eq!(
        share_sum,
        Perbill::one().deconstruct(),
        "Bucket shares must sum to 100%"
      );
    } else {
      panic!("Step 3 must be SplitTransfer");
    }
  });
}

#[test]
fn zap_execution_plan_tightens_slippage_as_native_depth_grows() {
  seeded_test_ext().execute_with(|| {
    let dust = primitives::ecosystem::params::BURNING_MANAGER_DUST_THRESHOLD;
    let shallow_foreign = AssetKind::Local(ASSET_A);
    let deep_foreign = AssetKind::Local(ASSET_B);
    let pool_seed = primitives::ecosystem::params::PRECISION * 100;
    let deep_pool_seed = primitives::ecosystem::params::PRECISION * 5_000;
    assert_ok!(create_pool(
      RuntimeOrigin::signed(ALICE),
      AssetKind::Native,
      shallow_foreign,
    ));
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(ALICE),
      AssetKind::Native,
      shallow_foreign,
      pool_seed,
      pool_seed,
      0,
      0,
      &ALICE,
    ));
    assert_ok!(create_pool(
      RuntimeOrigin::signed(ALICE),
      AssetKind::Native,
      deep_foreign,
    ));
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(ALICE),
      AssetKind::Native,
      deep_foreign,
      deep_pool_seed,
      deep_pool_seed,
      0,
      0,
      &ALICE,
    ));
    let shallow_lp = get_pool_lp_asset(AssetKind::Native, shallow_foreign);
    let deep_lp = get_pool_lp_asset(AssetKind::Native, deep_foreign);
    let shallow_plan =
      crate::configs::aaa_config::TmctolGenesisSystemAaas::build_zap_execution_plan(
        shallow_foreign,
        shallow_lp,
        dust,
      );
    let deep_plan = crate::configs::aaa_config::TmctolGenesisSystemAaas::build_zap_execution_plan(
      deep_foreign,
      deep_lp,
      dust,
    );
    let shallow_slippage = match &shallow_plan[1].task {
      Task::SwapExactIn {
        slippage_tolerance, ..
      } => *slippage_tolerance,
      _ => panic!("Step 2 must be SwapExactIn"),
    };
    let deep_slippage = match &deep_plan[1].task {
      Task::SwapExactIn {
        slippage_tolerance, ..
      } => *slippage_tolerance,
      _ => panic!("Step 2 must be SwapExactIn"),
    };
    assert_eq!(
      shallow_slippage,
      primitives::ecosystem::params::ZAP_MANAGER_MAX_SWAP_SLIPPAGE
    );
    assert_eq!(deep_slippage, Perbill::from_parts(6_000_000));
    assert!(deep_slippage < shallow_slippage);
  });
}

#[test]
fn zap_execution_plan_uses_max_slippage_when_pool_depth_is_unavailable() {
  seeded_test_ext().execute_with(|| {
    let foreign = AssetKind::Local(ASSET_A);
    assert_eq!(
      crate::configs::aaa_config::TmctolGenesisSystemAaas::resolve_zap_slippage_tolerance(foreign),
      primitives::ecosystem::params::ZAP_MANAGER_MAX_SWAP_SLIPPAGE
    );
  });
}

#[test]
fn zap_execution_plan_e2e_adds_liquidity_and_splits_lp_to_buckets() {
  use primitives::ecosystem::aaa_ids;
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let zm = AAA::sovereign_account_id_system(aaa_ids::ZAP_MANAGER_AAA_ID);
    let zm_id = aaa_ids::ZAP_MANAGER_AAA_ID;
    let foreign = AssetKind::Local(ASSET_A);
    let pre_seeded = Balances::free_balance(&zm);
    if pre_seeded > 0 {
      let _ = <Balances as Currency<crate::AccountId>>::transfer(
        &zm,
        &ALICE,
        pre_seeded - crate::EXISTENTIAL_DEPOSIT,
        polkadot_sdk::frame_support::traits::ExistenceRequirement::KeepAlive,
      );
    }
    let pre_seeded_foreign = crate::Assets::balance(ASSET_A, &zm);
    if pre_seeded_foreign > 0 {
      use polkadot_sdk::frame_support::traits::fungibles::Mutate;
      let _ = <crate::Assets as Mutate<crate::AccountId>>::transfer(
        ASSET_A,
        &zm,
        &ALICE,
        pre_seeded_foreign,
        polkadot_sdk::frame_support::traits::tokens::Preservation::Expendable,
      );
    }
    let fund_amount = 10 * primitives::ecosystem::params::PRECISION;
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&zm, fund_amount);
    assert_ok!(super::common::mint_tokens(
      ASSET_A,
      &ALICE,
      &zm,
      fund_amount
    ));
    let (_, pool_info) = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::iter()
      .next()
      .expect("pool must exist after setup");
    let lp_asset_id = pool_info.lp_token;
    let lp_asset = AssetKind::Local(lp_asset_id);
    let dust = primitives::ecosystem::params::BURNING_MANAGER_DUST_THRESHOLD;
    let execution_plan =
      crate::configs::aaa_config::TmctolGenesisSystemAaas::build_zap_execution_plan(
        foreign, lp_asset, dust,
      );
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::root(),
      zm_id,
      execution_plan
    ));
    let price = 1_000_000_000_000u128;
    pallet_axial_router::EmaPrices::<Runtime>::insert(AssetKind::Native, foreign, price);
    pallet_axial_router::EmaPrices::<Runtime>::insert(foreign, AssetKind::Native, price);
    System::reset_events();
    for block in 11..=30 {
      System::set_block_number(block);
      AAA::on_initialize(block);
      AAA::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    }
    let bucket_a = AAA::sovereign_account_id_system(aaa_ids::TOL_BUCKET_A_AAA_ID);
    let bucket_b = AAA::sovereign_account_id_system(aaa_ids::TOL_BUCKET_B_AAA_ID);
    let bucket_c = AAA::sovereign_account_id_system(aaa_ids::TOL_BUCKET_C_AAA_ID);
    let bucket_d = AAA::sovereign_account_id_system(aaa_ids::TOL_BUCKET_D_AAA_ID);
    let bucket_a_lp = crate::Assets::balance(lp_asset_id, &bucket_a);
    let bucket_b_lp = crate::Assets::balance(lp_asset_id, &bucket_b);
    let bucket_c_lp = crate::Assets::balance(lp_asset_id, &bucket_c);
    let bucket_d_lp = crate::Assets::balance(lp_asset_id, &bucket_d);
    let total_distributed = bucket_a_lp + bucket_b_lp + bucket_c_lp + bucket_d_lp;

    assert!(
      total_distributed > 0,
      "LP tokens must be distributed to TOL buckets"
    );
    assert!(
      bucket_a_lp > bucket_b_lp,
      "Bucket A (50%) must receive more than B (16.67%)"
    );
    let zm_lp_remaining = crate::Assets::balance(lp_asset_id, &zm);
    assert!(
      zm_lp_remaining < dust,
      "ZM sovereign LP must be below dust after distribution"
    );
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          aaa_id: id,
          failed_steps: 0,
          ..
        } if *id == zm_id
      )
    }));
  });
}

// --- TOL Bucket Unwind to Treasury (SC-014 Variant) ---

#[test]
fn bm_and_zm_activation_for_first_foreign_asset() {
  use polkadot_sdk::frame_support::traits::fungibles::Mutate as FungiblesMutate;
  use polkadot_sdk::frame_support::traits::tokens::{Fortitude, Precision, Preservation};
  use primitives::ecosystem::aaa_ids;
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let foreign = AssetKind::Local(super::common::ASSET_A);
    let (_, pool_info) = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::iter()
      .next()
      .expect("pool must exist after setup");
    let lp_asset_id = pool_info.lp_token;
    let lp_asset = AssetKind::Local(lp_asset_id);
    let dust = primitives::ecosystem::params::BURNING_MANAGER_DUST_THRESHOLD;
    let bm = AAA::sovereign_account_id_system(aaa_ids::BURNING_MANAGER_AAA_ID);
    let zm = AAA::sovereign_account_id_system(aaa_ids::ZAP_MANAGER_AAA_ID);
    // Clear preexisting balances to ensure clean test state
    let pre_seeded_bm_native = Balances::free_balance(&bm);
    if pre_seeded_bm_native > crate::EXISTENTIAL_DEPOSIT {
      let _ = <Balances as Currency<crate::AccountId>>::transfer(
        &bm,
        &ALICE,
        pre_seeded_bm_native - crate::EXISTENTIAL_DEPOSIT,
        polkadot_sdk::frame_support::traits::ExistenceRequirement::KeepAlive,
      );
    }
    let pre_seeded_bm_foreign = crate::Assets::balance(super::common::ASSET_A, &bm);
    if pre_seeded_bm_foreign > 0 {
      let _ = <crate::Assets as FungiblesMutate<crate::AccountId>>::burn_from(
        super::common::ASSET_A,
        &bm,
        pre_seeded_bm_foreign,
        Preservation::Expendable,
        Precision::BestEffort,
        Fortitude::Force,
      );
    }
    let bm_fund_amount = 2 * primitives::ecosystem::params::PRECISION;
    let zm_fund_amount = 10 * primitives::ecosystem::params::PRECISION;
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&bm, bm_fund_amount);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&zm, zm_fund_amount);
    assert_ok!(super::common::mint_tokens(
      super::common::ASSET_A,
      &ALICE,
      &bm,
      bm_fund_amount
    ));
    assert_ok!(super::common::mint_tokens(
      super::common::ASSET_A,
      &ALICE,
      &zm,
      zm_fund_amount
    ));
    let price = 1_000_000_000_000u128;
    pallet_axial_router::EmaPrices::<Runtime>::insert(AssetKind::Native, foreign, price);
    pallet_axial_router::EmaPrices::<Runtime>::insert(foreign, AssetKind::Native, price);
    let burn_execution_plan =
      crate::configs::aaa_config::TmctolGenesisSystemAaas::build_burn_execution_plan(
        alloc::vec![foreign],
        dust,
      );
    let zap_execution_plan =
      crate::configs::aaa_config::TmctolGenesisSystemAaas::build_zap_execution_plan(
        foreign, lp_asset, dust,
      );
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::root(),
      aaa_ids::BURNING_MANAGER_AAA_ID,
      burn_execution_plan
    ));
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::root(),
      aaa_ids::ZAP_MANAGER_AAA_ID,
      zap_execution_plan
    ));
    // Explicitly trigger execution since we deposited funds before updating execution plans
    assert_ok!(AAA::manual_trigger(
      RuntimeOrigin::root(),
      aaa_ids::BURNING_MANAGER_AAA_ID
    ));
    assert_ok!(AAA::manual_trigger(
      RuntimeOrigin::root(),
      aaa_ids::ZAP_MANAGER_AAA_ID
    ));
    let issuance_before = Balances::total_issuance();
    let foreign_before_bm = crate::Assets::balance(super::common::ASSET_A, &bm);
    System::reset_events();
    for block in 11..=40 {
      System::set_block_number(block);
      AAA::on_initialize(block);
      AAA::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    }
    let foreign_after_bm = crate::Assets::balance(super::common::ASSET_A, &bm);
    assert!(
      foreign_after_bm < foreign_before_bm,
      "BM must swap foreign tokens"
    );
    let issuance_after = Balances::total_issuance();
    assert!(
      issuance_after < issuance_before,
      "Issuance must decrease after burn"
    );
    let bucket_a = AAA::sovereign_account_id_system(aaa_ids::TOL_BUCKET_A_AAA_ID);
    let bucket_a_lp = crate::Assets::balance(lp_asset_id, &bucket_a);
    assert!(
      bucket_a_lp > 0,
      "ZM must distribute LP tokens to TOL buckets"
    );
  });
}

#[test]
fn bucket_c_unwind_execution_plan_transfers_lp_components_to_treasury_c() {
  use polkadot_sdk::frame_support::traits::fungible::Inspect as NativeInspect;
  use polkadot_sdk::frame_support::traits::fungibles::{Inspect, Mutate};
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let foreign = AssetKind::Local(ASSET_A);
    let foreign_id = ASSET_A;
    // Get LP token ID from the created pool
    let (_, pool_info) = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::iter()
      .next()
      .expect("pool must exist after setup");
    let lp_asset_id = pool_info.lp_token;
    let lp_asset = AssetKind::Local(lp_asset_id);
    let bucket_c_id = aaa_ids::TOL_BUCKET_C_AAA_ID;
    let treasury_c_id = aaa_ids::TREASURY_C_AAA_ID;
    let bucket_sovereign = AAA::sovereign_account_id_system(bucket_c_id);
    let treasury_sovereign = AAA::sovereign_account_id_system(treasury_c_id);
    // Endow Bucket C with 10_000_000 LP tokens
    let initial_lp_amount = 10_000_000u128;
    assert_ok!(<crate::Assets as Mutate<crate::AccountId>>::mint_into(
      lp_asset_id,
      &bucket_sovereign,
      initial_lp_amount
    ));
    // Endow the treasury account with native ED so it doesn't fail on first native transfer
    let ed = crate::EXISTENTIAL_DEPOSIT;
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&treasury_sovereign, ed);
    // Also endow the Bucket C sovereign account with native ED so it can receive small liquidity removals
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&bucket_sovereign, ed);
    let dust = 100u128;
    let unwind_pct = Perbill::from_percent(1); // 1% unwind per cycle
    // Build the execution_plan and activate Bucket C
    let execution_plan =
      crate::configs::aaa_config::TmctolGenesisSystemAaas::build_bucket_unwind_execution_plan(
        lp_asset,
        foreign,
        dust,
        unwind_pct,
        treasury_c_id,
      );
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::root(),
      bucket_c_id,
      execution_plan
    ));
    // Manually trigger execution of the Bucket C unwind cycle
    assert_ok!(pallet_aaa::Pallet::<Runtime>::manual_trigger(
      RuntimeOrigin::root(),
      bucket_c_id
    ));
    System::set_block_number(System::block_number() + 1);
    AAA::on_idle(System::block_number(), Weight::MAX); // This will execute the pending manual trigger
    for event in System::events() {
      println!("Event: {:?}", event.event);
    }
    // 1. Verify 1% of LP was removed.
    let remaining_lp =
      <crate::Assets as Inspect<crate::AccountId>>::balance(lp_asset_id, &bucket_sovereign);
    assert_eq!(remaining_lp, 9_900_000u128);
    // 2. Verify Treasury C received Native and Foreign.
    let treasury_native =
      <Balances as NativeInspect<crate::AccountId>>::balance(&treasury_sovereign);
    let treasury_foreign =
      <crate::Assets as Inspect<crate::AccountId>>::balance(foreign_id, &treasury_sovereign);
    assert!(treasury_native > ed);
    assert!(treasury_foreign > 0);
    // 3. Verify the cycle succeeded entirely
    assert!(has_aaa_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          aaa_id: id,
          failed_steps: 0,
          ..
        } if *id == bucket_c_id
      )
    }));
  });
}

// --- BLDR Domain Integration Tests ---

#[test]
fn native_tmc_mint_routes_collateral_and_tokens_to_default_zap_manager_sink() {
  seeded_test_ext().execute_with(|| {
    let foreign_amount = 10 * primitives::ecosystem::params::PRECISION;
    let zap_manager = zap_manager_account();
    assert_ok!(TokenMintingCurve::create_curve(
      RuntimeOrigin::root(),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      primitives::ecosystem::params::PRECISION,
      0,
    ));
    let alice_native_before = Balances::free_balance(&ALICE);
    let alice_foreign_before = crate::Assets::balance(ASSET_A, &ALICE);
    let zap_native_before = Balances::free_balance(&zap_manager);
    let zap_foreign_before = crate::Assets::balance(ASSET_A, &zap_manager);
    let minted = TokenMintingCurve::mint_with_distribution(
      &ALICE,
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      foreign_amount,
    )
    .expect("native TMC mint must succeed");
    let user_allocation = primitives::ecosystem::params::TMC_USER_ALLOCATION.mul_floor(minted);
    let zap_allocation = minted.saturating_sub(user_allocation);
    assert_eq!(minted, foreign_amount);
    assert_eq!(
      Balances::free_balance(&ALICE),
      alice_native_before + user_allocation
    );
    assert_eq!(
      crate::Assets::balance(ASSET_A, &ALICE),
      alice_foreign_before - foreign_amount
    );
    assert_eq!(
      Balances::free_balance(&zap_manager),
      zap_native_before + zap_allocation
    );
    assert_eq!(
      crate::Assets::balance(ASSET_A, &zap_manager),
      zap_foreign_before + foreign_amount
    );
  });
}

#[test]
fn native_tmc_mint_rejects_wrong_collateral_without_touching_default_zap_manager_sink() {
  seeded_test_ext().execute_with(|| {
    let foreign_amount = 10 * primitives::ecosystem::params::PRECISION;
    let zap_manager = zap_manager_account();
    assert_ok!(TokenMintingCurve::create_curve(
      RuntimeOrigin::root(),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      primitives::ecosystem::params::PRECISION,
      0,
    ));
    let alice_native_before = Balances::free_balance(&ALICE);
    let alice_wrong_foreign_before = crate::Assets::balance(ASSET_B, &ALICE);
    let zap_native_before = Balances::free_balance(&zap_manager);
    let zap_wrong_foreign_before = crate::Assets::balance(ASSET_B, &zap_manager);
    System::reset_events();
    assert_noop!(
      TokenMintingCurve::mint_with_distribution(
        &ALICE,
        AssetKind::Native,
        AssetKind::Local(ASSET_B),
        foreign_amount,
      ),
      pallet_tmc::Error::<Runtime>::InvalidForeignAsset
    );
    assert_eq!(Balances::free_balance(&ALICE), alice_native_before);
    assert_eq!(
      crate::Assets::balance(ASSET_B, &ALICE),
      alice_wrong_foreign_before
    );
    assert_eq!(Balances::free_balance(&zap_manager), zap_native_before);
    assert_eq!(
      crate::Assets::balance(ASSET_B, &zap_manager),
      zap_wrong_foreign_before
    );
    assert!(
      System::events()
        .into_iter()
        .all(|record| { !matches!(record.event, crate::RuntimeEvent::TokenMintingCurve(_)) })
    );
  });
}

#[test]
fn bldr_tmc_mint_rejects_wrong_collateral_without_touching_splitter_sink() {
  use primitives::ecosystem::{aaa_ids, protocol_tokens};
  seeded_test_ext().execute_with(|| {
    let bldr_id = protocol_tokens::BLDR_ASSET_ID;
    let bldr_asset = AssetKind::Local(bldr_id);
    let wrong_collateral = AssetKind::Local(ASSET_A);
    let splitter_sov = AAA::sovereign_account_id_system(aaa_ids::BLDR_SPLITTER_AAA_ID);
    let mint_amount = 10 * primitives::ecosystem::params::PRECISION;
    let alice_native_before = Balances::free_balance(&ALICE);
    let alice_wrong_foreign_before = crate::Assets::balance(ASSET_A, &ALICE);
    let splitter_native_before = Balances::free_balance(&splitter_sov);
    let splitter_wrong_foreign_before = crate::Assets::balance(ASSET_A, &splitter_sov);
    let splitter_bldr_before = crate::Assets::balance(bldr_id, &splitter_sov);
    let alice_bldr_before = crate::Assets::balance(bldr_id, &ALICE);
    System::reset_events();
    assert_noop!(
      TokenMintingCurve::mint_with_distribution(&ALICE, bldr_asset, wrong_collateral, mint_amount),
      pallet_tmc::Error::<Runtime>::InvalidForeignAsset
    );
    assert_eq!(Balances::free_balance(&ALICE), alice_native_before);
    assert_eq!(
      crate::Assets::balance(ASSET_A, &ALICE),
      alice_wrong_foreign_before
    );
    assert_eq!(
      Balances::free_balance(&splitter_sov),
      splitter_native_before
    );
    assert_eq!(
      crate::Assets::balance(ASSET_A, &splitter_sov),
      splitter_wrong_foreign_before
    );
    assert_eq!(
      crate::Assets::balance(bldr_id, &splitter_sov),
      splitter_bldr_before
    );
    assert_eq!(crate::Assets::balance(bldr_id, &ALICE), alice_bldr_before);
    assert!(
      System::events()
        .into_iter()
        .all(|record| { !matches!(record.event, crate::RuntimeEvent::TokenMintingCurve(_)) })
    );
  });
}

#[test]
fn bldr_tmc_mint_routes_collateral_and_tokens_correctly() {
  use primitives::ecosystem::{aaa_ids, protocol_tokens};
  seeded_test_ext().execute_with(|| {
    let bldr_id = protocol_tokens::BLDR_ASSET_ID;
    let bldr_asset = AssetKind::Local(bldr_id);
    // BLDR TMC curve created at genesis
    assert!(crate::TokenMintingCurve::has_curve(bldr_asset));
    let curve = crate::TokenMintingCurve::get_curve(bldr_asset).unwrap();
    assert_eq!(curve.foreign_asset, AssetKind::Native);
    let splitter_sov = AAA::sovereign_account_id_system(aaa_ids::BLDR_SPLITTER_AAA_ID);
    // Mint BLDR via TMC directly to verify distribution
    let mint_amount = 10 * primitives::ecosystem::params::PRECISION;
    let alice_native_before = Balances::free_balance(&ALICE);
    assert_ok!(crate::TokenMintingCurve::mint_with_distribution(
      &ALICE,
      bldr_asset,
      AssetKind::Native,
      mint_amount,
    ));
    // 6. Verify distribution
    let alice_native_after = Balances::free_balance(&ALICE);
    assert!(
      alice_native_after < alice_native_before,
      "Alice must pay NTVE collateral"
    );
    let collateral_paid = alice_native_before - alice_native_after;
    assert_eq!(
      collateral_paid, mint_amount,
      "All collateral must be transferred"
    );
    let alice_bldr = crate::Assets::balance(bldr_id, &ALICE);
    let splitter_bldr = crate::Assets::balance(bldr_id, &splitter_sov);
    let total_minted = alice_bldr + splitter_bldr;
    assert!(alice_bldr > 0, "User must receive BLDR");
    assert!(splitter_bldr > 0, "Splitter must receive BLDR");
    // Verify 33/66 ratio (within Perbill rounding tolerance)
    let user_pct = Perbill::from_rational(alice_bldr, total_minted);
    let expected_pct = primitives::ecosystem::params::TMC_USER_ALLOCATION;
    let diff = if user_pct > expected_pct {
      user_pct.deconstruct() - expected_pct.deconstruct()
    } else {
      expected_pct.deconstruct() - user_pct.deconstruct()
    };
    assert!(diff < 1000, "User allocation must be ~33% (diff={})", diff);
    assert!(
      splitter_bldr > alice_bldr,
      "Splitter (66%) must receive more than user (33%)"
    );
    // 7. Verify collateral (NTVE) went to Splitter (single output sink)
    let splitter_native = Balances::free_balance(&splitter_sov);
    assert!(
      splitter_native > crate::EXISTENTIAL_DEPOSIT,
      "Splitter must receive NTVE collateral from TMC output"
    );
  });
}

#[test]
fn bldr_splitter_distributes_to_zm_and_treasury() {
  use polkadot_sdk::frame_support::traits::fungibles::Mutate as FungiblesMutate;
  use primitives::ecosystem::{aaa_ids, protocol_tokens};
  seeded_test_ext().execute_with(|| {
    let bldr_id = protocol_tokens::BLDR_ASSET_ID;
    let bldr_asset = AssetKind::Local(bldr_id);
    let dust = primitives::ecosystem::params::BURNING_MANAGER_DUST_THRESHOLD;
    let splitter_sov = AAA::sovereign_account_id_system(aaa_ids::BLDR_SPLITTER_AAA_ID);
    let bldr_zm_sov = AAA::sovereign_account_id_system(aaa_ids::BLDR_ZM_AAA_ID);
    let bldr_treasury_sov = AAA::sovereign_account_id_system(aaa_ids::BLDR_TREASURY_AAA_ID);
    // Fund splitter with BLDR tokens and NTVE collateral (simulating TMC output)
    let fund_amount = 100 * primitives::ecosystem::params::PRECISION;
    let ntve_collateral = 50 * primitives::ecosystem::params::PRECISION;
    assert_ok!(
      <crate::Assets as FungiblesMutate<crate::AccountId>>::mint_into(
        bldr_id,
        &splitter_sov,
        fund_amount,
      )
    );
    let _ =
      <Balances as Currency<crate::AccountId>>::deposit_creating(&splitter_sov, ntve_collateral);
    let zm_native_before = Balances::free_balance(&bldr_zm_sov);
    // Activate splitter execution_plan
    let execution_plan =
      crate::configs::aaa_config::TmctolGenesisSystemAaas::build_bldr_splitter_execution_plan(
        bldr_asset, dust,
      );
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::root(),
      aaa_ids::BLDR_SPLITTER_AAA_ID,
      execution_plan,
    ));
    assert_ok!(AAA::manual_trigger(
      RuntimeOrigin::root(),
      aaa_ids::BLDR_SPLITTER_AAA_ID,
    ));
    System::reset_events();
    for block in 11..=30 {
      System::set_block_number(block);
      AAA::on_initialize(block);
      AAA::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    }
    let zm_bldr = crate::Assets::balance(bldr_id, &bldr_zm_sov);
    let treasury_bldr = crate::Assets::balance(bldr_id, &bldr_treasury_sov);
    assert!(zm_bldr > 0, "BLDR ZM must receive BLDR from splitter");
    assert!(
      treasury_bldr > 0,
      "BLDR Treasury must receive BLDR from splitter"
    );
    // 50/50 split (within rounding tolerance)
    let total_distributed = zm_bldr + treasury_bldr;
    let diff = zm_bldr.abs_diff(treasury_bldr);
    assert!(diff <= 1, "BLDR split must be 50/50 (diff={})", diff);
    assert!(
      total_distributed >= fund_amount - 2,
      "All funded BLDR must be distributed (total={}, funded={})",
      total_distributed,
      fund_amount
    );
    // Verify NTVE collateral was forwarded to BLDR ZM
    let zm_native_after = Balances::free_balance(&bldr_zm_sov);
    assert!(
      zm_native_after > zm_native_before,
      "BLDR ZM must receive forwarded NTVE from Splitter"
    );
  });
}

// --- BLDR Full E2E: Router → TMC → Splitter → ZM → LP → Bucket A ---

#[test]
fn bldr_full_e2e_router_tmc_splitter_zm_bucket() {
  use polkadot_sdk::frame_support::traits::fungibles::Inspect as FungiblesInspect;
  use primitives::ecosystem::{aaa_ids, protocol_tokens};
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_axial_router_infrastructure());
    // BLDR TMC curve created at genesis (9.5), Splitter execution_plan active at genesis (9.6)
    let bldr_id = protocol_tokens::BLDR_ASSET_ID;
    let bldr_asset = AssetKind::Local(bldr_id);
    let dust = primitives::ecosystem::params::BURNING_MANAGER_DUST_THRESHOLD;
    let precision = primitives::ecosystem::params::PRECISION;
    let splitter_id = aaa_ids::BLDR_SPLITTER_AAA_ID;
    let zm_id = aaa_ids::BLDR_ZM_AAA_ID;
    let bucket_a_id = aaa_ids::BLDR_BUCKET_A_AAA_ID;
    let splitter_sov = AAA::sovereign_account_id_system(splitter_id);
    let zm_sov = AAA::sovereign_account_id_system(zm_id);
    let treasury_sov = AAA::sovereign_account_id_system(aaa_ids::BLDR_TREASURY_AAA_ID);
    let bucket_a_sov = AAA::sovereign_account_id_system(bucket_a_id);
    // 1. Create NTVE-BLDR pool so ZM can add liquidity
    super::common::setup_bldr_pool(1_000 * precision);
    // 2. Activate ZM execution_plan (AddLiquidity + LP → Bucket A)
    let lp_asset = super::common::get_pool_lp_asset(AssetKind::Native, bldr_asset);
    let zm_execution_plan =
      crate::configs::aaa_config::TmctolGenesisSystemAaas::build_bldr_zm_execution_plan(
        bldr_asset, lp_asset, dust,
      );
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::root(),
      zm_id,
      zm_execution_plan
    ));
    // 3. Set ZM timer schedule (Splitter schedule already active from genesis)
    assert_ok!(AAA::update_schedule(
      RuntimeOrigin::root(),
      zm_id,
      pallet_aaa::Schedule {
        trigger: pallet_aaa::Trigger::Timer {
          every_blocks: 1,
          probability: None,
        },
        cooldown_blocks: 0,
      },
      None,
    ));
    // 4. User mints BLDR via Router TMC
    let mint_amount = 10 * precision;
    let alice_native_before = Balances::free_balance(&ALICE);
    let alice_bldr_before =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &ALICE);
    assert_ok!(crate::AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      AssetKind::Native,
      bldr_asset,
      mint_amount,
      0,
      ALICE,
      100, // deadline
    ));
    // Verify user received BLDR and paid NTVE
    let alice_native_after = Balances::free_balance(&ALICE);
    let alice_bldr_after =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &ALICE);
    assert!(
      alice_native_after < alice_native_before,
      "Alice must pay NTVE"
    );
    assert!(
      alice_bldr_after > alice_bldr_before,
      "Alice must receive BLDR"
    );
    // Verify Splitter received both NTVE collateral and BLDR zap share
    let splitter_ntve = Balances::free_balance(&splitter_sov);
    let splitter_bldr =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &splitter_sov);
    assert!(
      splitter_ntve > crate::EXISTENTIAL_DEPOSIT,
      "Splitter must hold NTVE collateral"
    );
    assert!(splitter_bldr > 0, "Splitter must hold BLDR zap share");
    // 5. Run blocks to execute Splitter → ZM → Bucket A chain.
    // The queue scheduler should keep this chain progressing without starvation.
    System::reset_events();
    for block in 2..=40 {
      System::set_block_number(block);
      AAA::on_initialize(block);
      AAA::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    }
    // 6. Verify: Splitter distributed NTVE to ZM
    let zm_ntve = Balances::free_balance(&zm_sov);
    assert!(
      zm_ntve > crate::EXISTENTIAL_DEPOSIT,
      "ZM must have received NTVE from Splitter"
    );
    // 7. Verify: Splitter distributed BLDR to both ZM and Treasury
    let treasury_bldr =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &treasury_sov);
    assert!(treasury_bldr > 0, "Treasury must have received BLDR");
    // 8. Verify: Bucket A received LP tokens (ZM provisioned liquidity)
    if let Some(lp_id) = match lp_asset {
      AssetKind::Local(id) => Some(id),
      _ => None,
    } {
      let bucket_lp =
        <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(lp_id, &bucket_a_sov);
      assert!(
        bucket_lp > 0,
        "Bucket A must hold LP tokens from ZM liquidity provisioning"
      );
    }
    // 9. Verify: Splitter sovereign is drained (execution_plan forwarded everything)
    let splitter_bldr_final =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &splitter_sov);
    assert!(
      splitter_bldr_final < dust,
      "Splitter must forward all BLDR (remaining={}, dust={})",
      splitter_bldr_final,
      dust
    );
  });
}

// --- Treasury B: BLDR Buyback & Burn ---

#[test]
fn treasury_b_buyback_burns_bldr() {
  use polkadot_sdk::frame_support::traits::fungibles::Inspect as FungiblesInspect;
  use primitives::ecosystem::aaa_ids;
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_axial_router_infrastructure());
    // Use ASSET_A as buyback target (pool already exists from setup)
    let target_id = super::common::ASSET_A;
    let target_asset = AssetKind::Local(target_id);
    let dust = primitives::ecosystem::params::BURNING_MANAGER_DUST_THRESHOLD;
    let slippage = primitives::ecosystem::params::SYSTEM_AAA_MAX_SWAP_SLIPPAGE;
    let buyback_pct = primitives::ecosystem::params::TREASURY_B_BUYBACK_PCT;
    let treasury_b_id = aaa_ids::TREASURY_B_AAA_ID;
    let treasury_b_sov = AAA::sovereign_account_id_system(treasury_b_id);
    // Fund Treasury B with NTVE (large enough that 0.042% > dust threshold)
    let fund_amount = 10_000 * primitives::ecosystem::params::PRECISION;
    let _ =
      <Balances as Currency<crate::AccountId>>::deposit_creating(&treasury_b_sov, fund_amount);
    // Activate buyback execution_plan
    let execution_plan =
      crate::configs::aaa_config::TmctolGenesisSystemAaas::build_treasury_b_buyback_execution_plan(
        target_asset,
        buyback_pct,
        dust,
        slippage,
      );
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::root(),
      treasury_b_id,
      execution_plan,
    ));
    assert_ok!(AAA::update_schedule(
      RuntimeOrigin::root(),
      treasury_b_id,
      pallet_aaa::Schedule {
        trigger: pallet_aaa::Trigger::Timer {
          every_blocks: 10,
          probability: None,
        },
        cooldown_blocks: 5,
      },
      None,
    ));
    let target_supply_before =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::total_issuance(target_id);
    let native_before = Balances::free_balance(&treasury_b_sov);
    System::reset_events();
    for block in 11..=100 {
      System::set_block_number(block);
      AAA::on_initialize(block);
      AAA::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    }
    // Verify at least one cycle executed
    assert!(
      has_aaa_event(|event| {
        matches!(event, pallet_aaa::Event::CycleSummary { aaa_id, .. } if *aaa_id == treasury_b_id)
      }),
      "Treasury B must have at least one cycle execution"
    );
    // Verify: Treasury B spent NTVE
    let native_after = Balances::free_balance(&treasury_b_sov);
    assert!(
      native_after < native_before,
      "Treasury B must spend NTVE on buyback"
    );
    // Verify: target supply decreased (burned)
    let target_supply_after =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::total_issuance(target_id);
    assert!(
      target_supply_after < target_supply_before,
      "Target supply must decrease after burn (before={}, after={})",
      target_supply_before,
      target_supply_after
    );
    // Verify: Treasury B holds zero target tokens
    let treasury_target =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(target_id, &treasury_b_sov);
    assert_eq!(
      treasury_target, 0,
      "Treasury B must burn all acquired tokens"
    );
  });
}

// --- Bucket B Unwind → Treasury B ---

#[test]
fn bucket_b_unwinds_lp_and_feeds_treasury_b() {
  use primitives::ecosystem::aaa_ids;
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let foreign = AssetKind::Local(ASSET_A);
    let precision = primitives::ecosystem::params::PRECISION;
    let dust = primitives::ecosystem::params::BURNING_MANAGER_DUST_THRESHOLD;
    let bucket_b_id = aaa_ids::TOL_BUCKET_B_AAA_ID;
    let treasury_b_id = aaa_ids::TREASURY_B_AAA_ID;
    let bucket_b_sov = AAA::sovereign_account_id_system(bucket_b_id);
    let treasury_b_sov = AAA::sovereign_account_id_system(treasury_b_id);
    // 1. Get LP asset for NTVE-Foreign pool
    let lp_asset = super::common::get_pool_lp_asset(AssetKind::Native, foreign);
    let lp_id = match lp_asset {
      AssetKind::Local(id) => id,
      _ => panic!("LP must be Local"),
    };
    // 2. Seed Bucket B with LP tokens by adding liquidity from its sovereign
    let seed_amount = 100 * precision;
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&bucket_b_sov, seed_amount);
    assert_ok!(
      <crate::Assets as polkadot_sdk::frame_support::traits::fungibles::Mutate<
        crate::AccountId,
      >>::mint_into(ASSET_A, &bucket_b_sov, seed_amount)
    );
    assert_ok!(crate::AssetConversion::add_liquidity(
      RuntimeOrigin::signed(bucket_b_sov.clone()),
      Box::new(AssetKind::Native),
      Box::new(foreign),
      seed_amount / 2,
      seed_amount / 2,
      0,
      0,
      bucket_b_sov.clone(),
    ));
    let lp_before =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(lp_id, &bucket_b_sov);
    assert!(lp_before > 0, "Bucket B must hold LP tokens");
    // 3. Activate Bucket B unwind execution_plan
    let unwind_pct = Perbill::from_percent(10);
    let execution_plan =
      crate::configs::aaa_config::TmctolGenesisSystemAaas::build_bucket_unwind_execution_plan(
        lp_asset,
        foreign,
        dust,
        unwind_pct,
        treasury_b_id,
      );
    assert_ok!(AAA::update_execution_plan(
      RuntimeOrigin::root(),
      bucket_b_id,
      execution_plan,
    ));
    assert_ok!(AAA::update_schedule(
      RuntimeOrigin::root(),
      bucket_b_id,
      pallet_aaa::Schedule {
        trigger: pallet_aaa::Trigger::Timer {
          every_blocks: 1,
          probability: None,
        },
        cooldown_blocks: 0,
      },
      None,
    ));
    // 4. Record Treasury B balances before unwind
    let treasury_ntve_before = Balances::free_balance(&treasury_b_sov);
    let treasury_foreign_before =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(ASSET_A, &treasury_b_sov);
    // 5. Run blocks for unwind execution
    System::reset_events();
    for block in 2..=40 {
      System::set_block_number(block);
      AAA::on_initialize(block);
      AAA::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    }
    // 6. Verify: Bucket B LP decreased
    let lp_after =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(lp_id, &bucket_b_sov);
    assert!(
      lp_after < lp_before,
      "Bucket B LP must decrease (before={}, after={})",
      lp_before,
      lp_after,
    );
    // 7. Verify: Treasury B received NTVE
    let treasury_ntve_after = Balances::free_balance(&treasury_b_sov);
    assert!(
      treasury_ntve_after > treasury_ntve_before,
      "Treasury B must receive NTVE from unwind"
    );
    // 8. Verify: Treasury B received Foreign
    let treasury_foreign_after =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(ASSET_A, &treasury_b_sov);
    assert!(
      treasury_foreign_after > treasury_foreign_before,
      "Treasury B must receive Foreign from unwind"
    );
  });
}

// --- Router TMC Efficiency Arbitration ---

#[test]
fn router_selects_tmc_over_xyk_when_tmc_price_is_better() {
  use primitives::ecosystem::protocol_tokens;
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let bldr_id = protocol_tokens::BLDR_ASSET_ID;
    let bldr_asset = AssetKind::Local(bldr_id);
    let precision = primitives::ecosystem::params::PRECISION;
    // BLDR TMC curve exists from genesis; create NTVE-BLDR XYK pool with unfavorable price
    // TMC gives ~1:1 at low supply; seed XYK pool at 10:1 ratio (10 NTVE per BLDR)
    super::common::setup_bldr_pool(100 * precision);
    // Add more NTVE to pool to make XYK price unfavorable (many NTVE per BLDR)
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&ALICE, 5000 * precision);
    assert_ok!(crate::AssetConversion::add_liquidity(
      RuntimeOrigin::signed(ALICE),
      Box::new(AssetKind::Native),
      Box::new(bldr_asset),
      900 * precision,
      10 * precision,
      0,
      0,
      ALICE,
    ));
    // Swap via Router — should select TMC (better price at low supply)
    let mint_amount = precision;
    let quote =
      crate::AxialRouter::quote_exact_input(ALICE, AssetKind::Native, bldr_asset, mint_amount)
        .expect("direct-mint quote must exist");
    let splitter_sov = AAA::sovereign_account_id_system(aaa_ids::BLDR_SPLITTER_AAA_ID);
    let burning_manager_before = Balances::free_balance(&burning_manager_account());
    let splitter_native_before = Balances::free_balance(&splitter_sov);
    let splitter_bldr_before =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &splitter_sov);
    let alice_bldr_before =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &ALICE);
    assert_ok!(crate::AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      AssetKind::Native,
      bldr_asset,
      mint_amount,
      0,
      ALICE,
      100,
    ));
    let alice_bldr_after =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &ALICE);
    let splitter_native_after = Balances::free_balance(&splitter_sov);
    let splitter_bldr_after =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &splitter_sov);
    let received = alice_bldr_after.saturating_sub(alice_bldr_before);
    assert!(received > 0, "Must receive BLDR tokens");
    // TMC at low supply gives ~33% of collateral as user share
    // XYK at 10:1 ratio gives much less per NTVE
    // Verify user received a reasonable amount (TMC-like, not XYK-like)
    let tmc_expected_min = precision / 4; // at least 25% of collateral value
    assert!(
      received > tmc_expected_min,
      "Router should select TMC route (received={}, min_expected={})",
      received,
      tmc_expected_min,
    );
    // Verify mechanism: Router must have selected DirectMint
    let used_mechanism = System::events()
      .iter()
      .rev()
      .find_map(|r| {
        if let crate::RuntimeEvent::AxialRouter(pallet_axial_router::Event::SwapExecuted {
          mechanism,
          ..
        }) = &r.event
        {
          Some(mechanism.clone())
        } else {
          None
        }
      })
      .expect("SwapExecuted event must exist");
    assert_eq!(
      used_mechanism,
      pallet_axial_router::RouteMechanismKind::DirectMint,
      "Router must select TMC (DirectMint) when TMC price is better than XYK"
    );
    assert_eq!(
      Balances::free_balance(&burning_manager_account()) - burning_manager_before,
      quote.router_fee,
      "Router direct-mint path must route the native fee to the burning manager"
    );
    assert_eq!(
      splitter_native_after - splitter_native_before,
      quote.amount_after_fee,
      "BLDR splitter must receive the post-fee native collateral"
    );
    assert_eq!(
      received + splitter_bldr_after.saturating_sub(splitter_bldr_before),
      quote.amount_out,
      "User + splitter BLDR deltas must equal the router direct-mint output"
    );
  });
}

#[test]
fn router_selects_xyk_when_tmc_price_exceeds_xyk() {
  use polkadot_sdk::frame_support::traits::fungibles::Mutate as FungiblesMutate;
  use primitives::ecosystem::protocol_tokens;
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let bldr_id = protocol_tokens::BLDR_ASSET_ID;
    let bldr_asset = AssetKind::Local(bldr_id);
    let precision = primitives::ecosystem::params::PRECISION;
    // Inflate BLDR total_issuance via direct mint (bypassing TMC). This is valid
    // because TMC reads live `Assets::total_issuance()` for price calculation, so
    // inflated issuance raises the TMC spot price regardless of mint origin.
    assert_ok!(
      <crate::Assets as FungiblesMutate<crate::AccountId>>::mint_into(
        bldr_id,
        &ALICE,
        1_000_000 * precision,
      )
    );
    // Create XYK pool at favorable 1:1 ratio (small pool)
    super::common::setup_bldr_pool(100 * precision);
    // Now TMC price is high (supply inflated), XYK is 1:1
    let mint_amount = precision;
    let alice_bldr_before =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &ALICE);
    assert_ok!(crate::AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      AssetKind::Native,
      bldr_asset,
      mint_amount,
      0,
      ALICE,
      100,
    ));
    let alice_bldr_after =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &ALICE);
    let received = alice_bldr_after.saturating_sub(alice_bldr_before);
    assert!(received > 0, "Must receive BLDR tokens");
    // XYK at 1:1 gives ~1 BLDR per NTVE (minus fees)
    // TMC with inflated supply gives << 1 BLDR (33% user share of diminishing mint)
    // Router should prefer XYK
    let xyk_expected_min = precision / 2; // at least 50% of input from XYK
    assert!(
      received > xyk_expected_min,
      "Router should select XYK route when TMC price is high (received={}, min={})",
      received,
      xyk_expected_min,
    );
    // Verify mechanism: Router must have selected DirectXyk
    let used_mechanism = System::events()
      .iter()
      .rev()
      .find_map(|r| {
        if let crate::RuntimeEvent::AxialRouter(pallet_axial_router::Event::SwapExecuted {
          mechanism,
          ..
        }) = &r.event
        {
          Some(mechanism.clone())
        } else {
          None
        }
      })
      .expect("SwapExecuted event must exist");
    assert_eq!(
      used_mechanism,
      pallet_axial_router::RouteMechanismKind::DirectXyk,
      "Router must select XYK (DirectXyk) when TMC price is worse"
    );
  });
}

#[test]
fn router_multi_hop_foreign_to_bldr() {
  use primitives::ecosystem::protocol_tokens;
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let bldr_id = protocol_tokens::BLDR_ASSET_ID;
    let bldr_asset = AssetKind::Local(bldr_id);
    let foreign = AssetKind::Local(ASSET_A);
    let precision = primitives::ecosystem::params::PRECISION;
    // Create NTVE-BLDR XYK pool (needed for hop 2)
    super::common::setup_bldr_pool(100 * precision);
    // NTVE-Foreign pool already exists from setup_axial_router_infrastructure (hop 1)
    // No direct Foreign-BLDR pool → forces multi-hop: Foreign→NTVE→BLDR
    let swap_amount = precision;
    let alice_bldr_before =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &ALICE);
    System::reset_events();
    assert_ok!(crate::AxialRouter::swap(
      RuntimeOrigin::signed(ALICE),
      foreign,
      bldr_asset,
      swap_amount,
      0,
      ALICE,
      100,
    ));
    let alice_bldr_after =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &ALICE);
    let received = alice_bldr_after.saturating_sub(alice_bldr_before);
    assert!(received > 0, "Must receive BLDR from multi-hop swap");
    // Verify mechanism: must be MultiHopNative (Foreign→Native→BLDR)
    let used_mechanism = System::events()
      .iter()
      .rev()
      .find_map(|r| {
        if let crate::RuntimeEvent::AxialRouter(pallet_axial_router::Event::SwapExecuted {
          mechanism,
          ..
        }) = &r.event
        {
          Some(mechanism.clone())
        } else {
          None
        }
      })
      .expect("SwapExecuted event must exist");
    assert_eq!(
      used_mechanism,
      pallet_axial_router::RouteMechanismKind::MultiHopNative,
      "Router must use multi-hop (Foreign→NTVE→BLDR) when no direct pool exists"
    );
  });
}

#[test]
fn tol_bucket_drainage_pressure_all_four_buckets_is_bounded() {
  use polkadot_sdk::frame_support::{BoundedVec, traits::fungibles::Mutate};
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_axial_router_infrastructure());
    let (_, pool_info) = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::iter()
      .next()
      .expect("pool must exist after setup");
    let lp_asset_id = pool_info.lp_token;
    let lp_asset = AssetKind::Local(lp_asset_id);
    let bucket_ids = [
      aaa_ids::TOL_BUCKET_A_AAA_ID,
      aaa_ids::TOL_BUCKET_B_AAA_ID,
      aaa_ids::TOL_BUCKET_C_AAA_ID,
      aaa_ids::TOL_BUCKET_D_AAA_ID,
    ];
    let mut before_lp = alloc::vec::Vec::new();
    for bucket_id in bucket_ids {
      let bucket = AAA::sovereign_account_id_system(bucket_id);
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
        &bucket,
        crate::EXISTENTIAL_DEPOSIT,
      );
      assert_ok!(<crate::Assets as Mutate<crate::AccountId>>::mint_into(
        lp_asset_id,
        &bucket,
        1_000_000_000,
      ));
      let execution_plan: ExecutionPlanOf<Runtime> = alloc::vec![pallet_aaa::Step {
        conditions: BoundedVec::default(),
        task: Task::RemoveLiquidity {
          lp_asset,
          amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(10)),
        },
        on_error: StepErrorPolicy::AbortCycle,
      }]
      .try_into()
      .expect("execution_plan must fit");
      assert_ok!(AAA::update_execution_plan(
        RuntimeOrigin::root(),
        bucket_id,
        execution_plan
      ));
      assert_ok!(AAA::manual_trigger(RuntimeOrigin::root(), bucket_id));
      before_lp.push((bucket_id, crate::Assets::balance(lp_asset_id, &bucket)));
    }
    for block in 20..=40 {
      System::set_block_number(block);
      AAA::on_initialize(block);
      AAA::on_idle(block, Weight::MAX);
    }
    for (bucket_id, before) in before_lp {
      let bucket = AAA::sovereign_account_id_system(bucket_id);
      let after = crate::Assets::balance(lp_asset_id, &bucket);
      assert!(
        after < before,
        "Bucket {} LP should decrease under drainage pressure ({} -> {})",
        bucket_id,
        before,
        after
      );
      assert!(after > 0, "Bucket {} should retain non-zero LP", bucket_id);
    }
  });
}
