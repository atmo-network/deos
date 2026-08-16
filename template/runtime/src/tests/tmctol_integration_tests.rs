//! Integration tests for the TMCTOL economic standard on DEOS runtime.
//!
//! The `tmctol_` prefix is intentional: this module tests the TMCTOL standard
//! (TMC, TOL, Router, Splitter, Liquidity Actor, Bucket) running on top of the DEOS
//! runtime. The `tmctol_` module identity remains standard-specific because these
//! tests verify one concrete economy rather than the reusable DEOS kernel.

use super::common::{
  ALICE, ASSET_A, ASSET_B, add_liquidity, burn_actor_account, create_pool, get_pool_lp_asset,
  liquidity_actor_account, new_test_ext, publish_bidirectional_deos_router_observation,
  publish_deos_router_observation, seeded_test_ext, update_actor_contract_partial,
};
macro_rules! update_actor_contract_partial {
  ($origin:expr, $actor:expr, $value:expr $(,)?) => {
    update_actor_contract_partial($origin, $actor, $value)
  };
  ($origin:expr, $actor:expr, $first:expr, $second:expr $(,)?) => {
    update_actor_contract_partial($origin, $actor, ($first, $second))
  };
}

use crate::{Actors, Balances, Runtime, RuntimeOrigin, System, TokenMintingCurve};
use pallet_deos_actors::{
  ActorContract, ActorType, AmountResolution, AssetOps, CompletionPolicy, ContractSteps, DexOps,
  Event, ExecutionContext, FundingSourcePolicy, OutcomeTotals, StepErrorPolicy, Task,
};
use polkadot_sdk::frame_support::{
  assert_noop, assert_ok,
  traits::{
    Currency, Hooks,
    fungibles::{Inspect as FungiblesInspect, Mutate as FungiblesMutate},
  },
  weights::Weight,
};
use polkadot_sdk::sp_runtime::Perbill;
use primitives::ecosystem::{actor_ids, protocol_tokens};
use primitives::{AssetKind, GuaranteeStatus, TmctolConformanceStatus};

use super::actors_integration_tests::has_actor_event;

fn all_preconditions(
  predicates: alloc::vec::Vec<
    pallet_deos_actors::Predicate<AssetKind, u128, u32, primitives::OracleFeedId>,
  >,
) -> Option<pallet_deos_actors::PreconditionOf<Runtime>> {
  let clause = predicates
    .into_iter()
    .map(|predicate| pallet_deos_actors::TimedPredicate {
      timing: pallet_deos_actors::ObservationTiming::Current,
      predicate,
    })
    .collect::<alloc::vec::Vec<_>>()
    .try_into()
    .expect("runtime predicates fit");
  Some(pallet_deos_actors::Precondition {
    clauses: alloc::vec![clause].try_into().expect("runtime clause fits"),
  })
}

fn activate_dormant_system(
  actor_id: pallet_deos_actors::ActorId,
  steps: ContractSteps<Runtime>,
) -> polkadot_sdk::sp_runtime::DispatchResult {
  Actors::activate_actor(
    RuntimeOrigin::root(),
    actor_id,
    ActorContract {
      trigger: pallet_deos_actors::Trigger::immediate_manual_and_address_event(
        pallet_deos_actors::SourceFilter::Any,
        pallet_deos_actors::AssetFilter::Any,
      ),
      cooldown_blocks: primitives::ecosystem::params::SYSTEM_ACTORS_COOLDOWN_BLOCKS,
      window: None,
      steps,
      completion: pallet_deos_actors::CompletionPolicy::Persistent,
      funding: FundingSourcePolicy::RuntimePolicy,
      auto_close_at_cycle_nonce: None,
    },
  )
}

// --- Genesis System Actors ---

#[test]
fn tmctol_guarantee_state_reports_anchor_protection_without_pool_initialization() {
  new_test_ext().execute_with(|| {
    let state = crate::tmctol_read_model::TmctolReadModel::tmctol_guarantee_state();
    assert_eq!(state.tol_anchor.status, GuaranteeStatus::Satisfied);
    assert_eq!(state.bldr_anchor.status, GuaranteeStatus::Satisfied);
    assert_eq!(state.anchor_status, GuaranteeStatus::Satisfied);
    assert_eq!(state.tol_pool.status, GuaranteeStatus::NotInitialized);
    assert_eq!(state.bldr_pool.status, GuaranteeStatus::NotInitialized);
    assert_eq!(state.pool_status, GuaranteeStatus::NotInitialized);
    assert_eq!(
      state.zap_postconditions.status,
      GuaranteeStatus::NotInitialized
    );
    assert_eq!(state.zap_status, GuaranteeStatus::NotInitialized);
    assert_eq!(
      state.native_burn_liveness.status,
      GuaranteeStatus::Satisfied
    );
    assert!(state.native_burn_liveness.has_required_burn_step);
    assert!(!state.native_burn_liveness.requires_swap);
    assert_eq!(
      state.bldr_buyback_liveness.status,
      GuaranteeStatus::NotInitialized
    );
    assert_eq!(state.burn_liveness_status, GuaranteeStatus::NotInitialized);
    assert_eq!(
      state.native_floor_inputs.status,
      GuaranteeStatus::NotInitialized
    );
    assert_eq!(
      state.conformance_status,
      TmctolConformanceStatus::NotInitialized
    );
  });
}

#[test]
fn tmctol_guarantee_state_reports_bldr_anchor_pool_when_initialized() {
  seeded_test_ext().execute_with(|| {
    super::common::setup_bldr_pool(10 * crate::UNIT);
    let lp_asset = get_pool_lp_asset(
      AssetKind::Native,
      AssetKind::Local(protocol_tokens::BLDR_ASSET_ID),
    );
    let AssetKind::Local(lp_asset_id) = lp_asset else {
      panic!("pool LP asset must be local");
    };
    let bldr_anchor = Actors::sovereign_account_id_system(actor_ids::BLDR_BUCKET_A_ACTORS_ID);
    assert_ok!(
      <crate::Assets as FungiblesMutate<crate::AccountId>>::mint_into(
        lp_asset_id,
        &bldr_anchor,
        1_000,
      )
    );

    let state = crate::tmctol_read_model::TmctolReadModel::tmctol_guarantee_state();
    assert_eq!(state.bldr_anchor.status, GuaranteeStatus::Satisfied);
    assert_eq!(state.bldr_pool.status, GuaranteeStatus::Satisfied);
    assert_eq!(state.bldr_pool.lp_asset_id, Some(lp_asset_id));
    assert!(state.bldr_pool.reserve_a > 0);
    assert!(state.bldr_pool.reserve_b > 0);
    assert_eq!(state.bldr_pool.anchor_lp_balance, 1_000);
  });
}

#[test]
fn tmctol_guarantee_state_reports_bldr_buyback_liveness_when_configured() {
  seeded_test_ext().execute_with(|| {
    super::common::setup_bldr_pool(10 * crate::UNIT);
    let steps =
      crate::configs::actor_config::TmctolGenesisSystemActors::build_treasury_b_buyback_contract_steps(
        AssetKind::Local(protocol_tokens::BLDR_ASSET_ID),
        primitives::ecosystem::params::TREASURY_B_BUYBACK_PCT,
        primitives::ecosystem::params::BURN_ACTOR_DUST_THRESHOLD,
        primitives::ecosystem::params::SYSTEM_ACTORS_MAX_SWAP_SLIPPAGE,
      );
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::root(),
      actor_ids::TREASURY_B_ACTORS_ID,
      ActorContract {
        trigger: pallet_deos_actors::Trigger::immediate_manual_and_address_event(
          pallet_deos_actors::SourceFilter::Any,
          pallet_deos_actors::AssetFilter::Any,
        ),
        cooldown_blocks: 5,
        window: None,
        steps,
        completion: pallet_deos_actors::CompletionPolicy::Persistent,
        funding: FundingSourcePolicy::RuntimePolicy,
        auto_close_at_cycle_nonce: None,
      },
    ));

    let state = crate::tmctol_read_model::TmctolReadModel::tmctol_guarantee_state();
    assert_eq!(
      state.bldr_buyback_liveness.status,
      GuaranteeStatus::Satisfied
    );
    assert!(state.bldr_buyback_liveness.requires_swap);
    assert!(state.bldr_buyback_liveness.has_required_swap_step);
    assert!(state.bldr_buyback_liveness.has_required_burn_step);
  });
}

#[test]
fn tmctol_guarantee_state_flags_broken_native_burn_plan_as_violation() {
  new_test_ext().execute_with(|| {
    pallet_deos_actors::ActorContracts::<Runtime>::mutate(actor_ids::BURN_ACTOR_ID, |maybe| {
      let contract = maybe.as_mut().expect("Burn Actor contract exists");
      contract.steps = alloc::vec![pallet_deos_actors::Step {
        precondition: None,
        task: pallet_deos_actors::Task::Transfer {
          to: ALICE,
          asset: AssetKind::Native,
          amount: AmountResolution::Fixed(0),
        },
        on_error: pallet_deos_actors::StepErrorPolicy::AbortCycle,
      }]
      .try_into()
      .expect("malformed burn plan fits");
    });

    let state = crate::tmctol_read_model::TmctolReadModel::tmctol_guarantee_state();
    assert_eq!(state.native_burn_liveness.status, GuaranteeStatus::Violated);
    assert_eq!(state.burn_liveness_status, GuaranteeStatus::Violated);
    assert_eq!(state.conformance_status, TmctolConformanceStatus::Violated);
  });
}

#[test]
fn tmctol_guarantee_state_reports_valid_zap_postconditions() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let foreign = AssetKind::Local(ASSET_A);
    let lp_asset = get_pool_lp_asset(AssetKind::Native, foreign);
    let steps = crate::configs::actor_config::TmctolGenesisSystemActors::build_zap_contract_steps(
      foreign,
      lp_asset,
      primitives::ecosystem::params::BURN_ACTOR_DUST_THRESHOLD,
    );
    assert_ok!(activate_dormant_system(
      actor_ids::LIQUIDITY_ACTOR_ACTORS_ID,
      steps,
    ));

    let state = crate::tmctol_read_model::TmctolReadModel::tmctol_guarantee_state();
    assert_eq!(state.zap_postconditions.status, GuaranteeStatus::Satisfied);
    assert_eq!(state.zap_status, GuaranteeStatus::Satisfied);
    assert_eq!(
      state.zap_postconditions.configured_foreign_asset,
      Some(foreign)
    );
    assert_eq!(state.zap_postconditions.configured_lp_asset, Some(lp_asset));
    assert!(state.zap_postconditions.has_add_liquidity_step);
    assert!(state.zap_postconditions.has_foreign_to_native_swap_step);
    assert!(state.zap_postconditions.has_lp_split_step);
    assert!(state.zap_postconditions.split_targets_all_buckets);
    assert!(state.zap_postconditions.split_shares_sum_to_one);
    assert!(state.zap_postconditions.split_shares_match_policy);
  });
}

#[test]
fn tmctol_guarantee_state_flags_malformed_zap_postconditions() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let foreign = AssetKind::Local(ASSET_A);
    let lp_asset = get_pool_lp_asset(AssetKind::Native, foreign);
    let malformed_plan: ContractSteps<Runtime> = alloc::vec![
      pallet_deos_actors::Step {
        precondition: None,
        task: Task::AddLiquidity {
          asset_a: AssetKind::Native,
          asset_b: foreign,
          amount_a: AmountResolution::AllAvailable,
          amount_b: AmountResolution::AllAvailable,
          min_lp_out: 1,
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      pallet_deos_actors::Step {
        precondition: None,
        task: Task::SwapIn {
          asset_in: foreign,
          asset_out: AssetKind::Native,
          amount_in: AmountResolution::AllAvailable,
          slippage_tolerance: primitives::ecosystem::params::SYSTEM_ACTORS_MAX_SWAP_SLIPPAGE,
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      pallet_deos_actors::Step {
        precondition: None,
        task: Task::SplitTransfer {
          asset: lp_asset,
          amount: AmountResolution::AllAvailable,
          legs: alloc::vec![
            pallet_deos_actors::SplitLeg {
              to: Actors::sovereign_account_id_system(actor_ids::TOL_BUCKET_A_ACTORS_ID),
              share: Perbill::from_percent(40),
            },
            pallet_deos_actors::SplitLeg {
              to: Actors::sovereign_account_id_system(actor_ids::TOL_BUCKET_B_ACTORS_ID),
              share: Perbill::from_percent(20),
            },
            pallet_deos_actors::SplitLeg {
              to: Actors::sovereign_account_id_system(actor_ids::TOL_BUCKET_C_ACTORS_ID),
              share: Perbill::from_percent(20),
            },
            pallet_deos_actors::SplitLeg {
              to: Actors::sovereign_account_id_system(actor_ids::TOL_BUCKET_D_ACTORS_ID),
              share: Perbill::from_percent(20),
            },
          ]
          .try_into()
          .expect("split legs fit"),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
    ]
    .try_into()
    .expect("malformed zap plan still fits runtime bounds");
    assert_ok!(activate_dormant_system(
      actor_ids::LIQUIDITY_ACTOR_ACTORS_ID,
      malformed_plan,
    ));

    let state = crate::tmctol_read_model::TmctolReadModel::tmctol_guarantee_state();
    assert_eq!(state.zap_postconditions.status, GuaranteeStatus::Violated);
    assert_eq!(state.zap_status, GuaranteeStatus::Violated);
    assert!(state.zap_postconditions.split_targets_all_buckets);
    assert!(state.zap_postconditions.split_shares_sum_to_one);
    assert!(!state.zap_postconditions.split_shares_match_policy);
    assert_eq!(state.conformance_status, TmctolConformanceStatus::Violated);
  });
}

#[test]
fn tmctol_guarantee_state_flags_anchor_mutation_as_violation() {
  new_test_ext().execute_with(|| {
    let actor_id = actor_ids::TOL_BUCKET_A_ACTORS_ID;
    pallet_deos_actors::ActorIdentities::<Runtime>::insert(
      actor_id,
      pallet_deos_actors::ActorIdentity {
        sovereign_account: Actors::sovereign_account_id_system(actor_id),
        owner: ALICE,
        actor_class: pallet_deos_actors::ActorClass::System {
          sovereign_id: actor_id,
        },
        mutability: pallet_deos_actors::Mutability::Mutable,
        cycle_nonce: 0,
        last_control_mutation_block: 0,
      },
    );

    let state = crate::tmctol_read_model::TmctolReadModel::tmctol_guarantee_state();
    assert_eq!(state.tol_anchor.status, GuaranteeStatus::Violated);
    assert_eq!(state.anchor_status, GuaranteeStatus::Violated);
    assert_eq!(state.conformance_status, TmctolConformanceStatus::Violated);
  });
}

#[test]
fn genesis_burn_actor_has_deterministic_sovereign_and_correct_state() {
  new_test_ext().execute_with(|| {
    let actor_id = actor_ids::BURN_ACTOR_ID;
    let instance = Actors::active_actor_state(actor_id).expect("Burn Actor must exist at genesis");
    let expected_sovereign = Actors::sovereign_account_id_system(actor_id);
    assert_eq!(instance.identity.sovereign_account, expected_sovereign);
    assert_eq!(
      instance.identity.actor_class,
      pallet_deos_actors::ActorClass::System {
        sovereign_id: actor_id,
      }
    );
    assert_eq!(
      instance.identity.mutability,
      pallet_deos_actors::Mutability::Mutable
    );
    assert_eq!(
      instance.hot.lifecycle,
      pallet_deos_actors::ActiveLifecycle::Active
    );
    assert_eq!(instance.hot.unsuccessful_attempt_streak, 0);
    assert!(!instance.hot.pending_signal);
    assert_eq!(
      Actors::next_actor_id(),
      actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID + 1
    );
    assert_eq!(
      pallet_deos_actors::SovereignIndex::<Runtime>::get(&expected_sovereign),
      Some(actor_id)
    );
  });
}

#[test]
fn genesis_burn_actor_sovereign_is_stable_across_rebuilds() {
  let sovereign_a =
    new_test_ext().execute_with(|| Actors::sovereign_account_id_system(actor_ids::BURN_ACTOR_ID));
  let sovereign_b =
    new_test_ext().execute_with(|| Actors::sovereign_account_id_system(actor_ids::BURN_ACTOR_ID));
  assert_eq!(sovereign_a, sovereign_b);
}

#[test]
fn genesis_value_driven_contracts_use_omnivorous_address_event_triggers() {
  new_test_ext().execute_with(|| {
    for actor_id in [
      actor_ids::BURN_ACTOR_ID,
      actor_ids::FEE_SINK_ACTORS_ID,
      actor_ids::BLDR_SPLITTER_ACTORS_ID,
    ] {
      let instance = Actors::active_actor_state(actor_id).expect("genesis active actor exists");
      assert!(
        instance.contract.trigger.address_event_source_enabled(),
        "value-driven actor {actor_id} must react to verified inbound value without polling"
      );
    }
  });
}

// --- Burn Actor ---

#[test]
fn burn_actor_burns_native_on_address_event() {
  seeded_test_ext().execute_with(|| {
    let bm = Actors::sovereign_account_id_system(actor_ids::BURN_ACTOR_ID);
    let deposit = 50 * crate::EXISTENTIAL_DEPOSIT;
    assert_ok!(<crate::configs::actor_config::TmctolAssetOps as AssetOps<
      crate::AccountId,
      AssetKind,
      crate::Balance,
    >>::transfer(&ALICE, &bm, AssetKind::Native, deposit));
    let issuance_before = Balances::total_issuance();
    let bm_balance_before = Balances::free_balance(&bm);
    assert!(bm_balance_before > 0);
    System::set_block_number(11);
    Actors::on_initialize(11);
    Actors::on_idle(11, Weight::from_parts(u64::MAX, u64::MAX));
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
fn burn_actor_skips_burn_when_signaled_balance_is_below_dust() {
  new_test_ext().execute_with(|| {
    let bm = Actors::sovereign_account_id_system(actor_ids::BURN_ACTOR_ID);
    assert_ok!(<crate::configs::actor_config::TmctolAssetOps as AssetOps<
      crate::AccountId,
      AssetKind,
      crate::Balance,
    >>::mint(
      &bm, AssetKind::Native, crate::EXISTENTIAL_DEPOSIT
    ));
    let issuance_before = Balances::total_issuance();
    System::set_block_number(11);
    Actors::on_initialize(11);
    Actors::on_idle(11, Weight::from_parts(u64::MAX, u64::MAX));
    let issuance_after = Balances::total_issuance();
    assert_eq!(
      issuance_before, issuance_after,
      "No burn when balance is below dust threshold"
    );
  });
}

#[test]
fn router_oracle_burn_success_path_commits_once_without_scheduler_or_reward_residue() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let burn_actor_id = actor_ids::BURN_ACTOR_ID;
    let burn_actor = Actors::sovereign_account_id_system(burn_actor_id);
    for block in 11..=30 {
      System::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    }
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &burn_actor,
      crate::EXISTENTIAL_DEPOSIT,
    );
    let asset_in = AssetKind::Native;
    let asset_out = AssetKind::Local(super::common::ASSET_A);
    let feed = crate::configs::oracle_config::deos_router_pool_feed(asset_in, asset_out);
    let swap_amount = 500 * primitives::ecosystem::params::PRECISION;
    let quote = crate::DeosRouter::quote_exact_input(ALICE, asset_in, asset_out, swap_amount)
      .expect("seeded direct pool has a finalized quote");
    let alice_native_before = Balances::free_balance(&ALICE);
    let alice_output_before = crate::Assets::balance(super::common::ASSET_A, &ALICE);
    let burn_balance_before = Balances::free_balance(&burn_actor);
    let issuance_before = Balances::total_issuance();
    let observation_revision_before = crate::Oracle::observations(feed)
      .map(|observation| observation.revision)
      .unwrap_or_default();
    let reward_liability_before = crate::Staking::native_security_reward_liability();
    let reward_custody_before =
      Balances::free_balance(crate::Staking::native_security_reward_account());
    let burn_before = Actors::active_actor_state(burn_actor_id).expect("Burn Actor exists");
    let target_cycle_nonce = burn_before.identity.cycle_nonce.saturating_add(1);
    assert!(burn_before.contract.trigger.address_event_source_enabled());
    assert_eq!(Actors::dirty_observation_feed_count(), 0);

    System::set_block_number(31);
    assert_ok!(crate::DeosRouter::swap(
      RuntimeOrigin::signed(ALICE),
      asset_in,
      asset_out,
      swap_amount,
      quote.amount_out,
      ALICE,
      System::block_number() + 100,
    ));
    assert_eq!(
      Balances::free_balance(&ALICE),
      alice_native_before - swap_amount
    );
    assert_eq!(
      crate::Assets::balance(super::common::ASSET_A, &ALICE),
      alice_output_before + quote.amount_out,
    );
    assert_eq!(
      Balances::free_balance(&burn_actor),
      burn_balance_before + quote.router_fee,
    );
    let observation = crate::Oracle::observations(feed).expect("swap publishes pool observation");
    assert_eq!(observation.revision, observation_revision_before + 1);
    let issuance_after_swap = Balances::total_issuance();
    assert_eq!(
      issuance_after_swap, issuance_before,
      "the swap transfers native input without changing currency issuance",
    );

    let max_wait_blocks = burn_before.contract.cooldown_blocks.saturating_add(2);
    for offset in 1..=max_wait_blocks {
      let block = 31u32.saturating_add(offset);
      System::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
      if Actors::active_actor_state(burn_actor_id)
        .is_some_and(|state| state.identity.cycle_nonce == target_cycle_nonce)
      {
        break;
      }
    }
    let burn_after = Actors::active_actor_state(burn_actor_id).expect("Burn Actor remains active");
    assert_eq!(burn_after.identity.cycle_nonce, target_cycle_nonce);
    assert_eq!(
      Balances::free_balance(&burn_actor),
      crate::EXISTENTIAL_DEPOSIT,
      "Burn Actor preserves only its persistent native anchor",
    );
    let exact_burn = burn_balance_before + quote.router_fee - crate::EXISTENTIAL_DEPOSIT;
    assert_eq!(
      Balances::total_issuance(),
      issuance_after_swap - exact_burn,
      "the ingress cycle burns the exact available balance once",
    );
    assert_eq!(
      System::events()
        .into_iter()
        .filter(|record| matches!(
          &record.event,
          crate::RuntimeEvent::Actors(Event::CycleStarted { actor_id, cycle_nonce })
            if *actor_id == burn_actor_id && *cycle_nonce == target_cycle_nonce
        ))
        .count(),
      1,
    );
    let hot = Actors::actor_hot(burn_actor_id).expect("Burn Actor hot state exists");
    assert!(hot.queue_ticket.is_none());
    assert!(hot.wakeup_pointer.is_none());
    assert!(Actors::continuation_state(burn_actor_id).is_none());
    assert_eq!(Actors::dirty_observation_feed_count(), 0);
    assert_eq!(
      crate::Staking::native_security_reward_liability(),
      reward_liability_before,
    );
    assert_eq!(
      Balances::free_balance(crate::Staking::native_security_reward_account()),
      reward_custody_before,
    );
  });
}

#[test]
fn burn_actor_swaps_foreign_to_native_then_burns_via_updated_plan() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let bm_id = actor_ids::BURN_ACTOR_ID;
    let bm = Actors::sovereign_account_id_system(bm_id);
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
    assert_ok!(publish_bidirectional_deos_router_observation(
      AssetKind::Local(super::common::ASSET_A),
      AssetKind::Native,
      price,
    ));
    let dust = primitives::ecosystem::params::BURN_ACTOR_DUST_THRESHOLD;
    let new_steps: ContractSteps<Runtime> = alloc::vec![
      pallet_deos_actors::Step {
        precondition: all_preconditions(alloc::vec![pallet_deos_actors::Predicate::BalanceAbove {
          asset: AssetKind::Local(super::common::ASSET_A),
          threshold: dust,
        },]),
        task: Task::SwapIn {
          asset_in: AssetKind::Local(super::common::ASSET_A),
          asset_out: AssetKind::Native,
          amount_in: AmountResolution::AllAvailable,
          slippage_tolerance: Perbill::from_percent(5),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      pallet_deos_actors::Step {
        precondition: all_preconditions(alloc::vec![pallet_deos_actors::Predicate::BalanceAbove {
          asset: AssetKind::Native,
          threshold: dust,
        },]),
        task: Task::Burn {
          asset: AssetKind::Native,
          amount: AmountResolution::AllAvailable,
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
    ]
    .try_into()
    .unwrap();
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      bm_id,
      (new_steps, CompletionPolicy::Persistent,)
    ));
    let foreign_amount = 2 * primitives::ecosystem::params::PRECISION;
    assert_ok!(<crate::configs::actor_config::TmctolAssetOps as AssetOps<
      crate::AccountId,
      AssetKind,
      crate::Balance,
    >>::mint(
      &bm,
      AssetKind::Local(super::common::ASSET_A),
      foreign_amount,
    ));
    let foreign_before = crate::Assets::balance(super::common::ASSET_A, &bm);
    let issuance_before = Balances::total_issuance();
    for block in 11..=30 {
      System::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
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
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let bm = Actors::sovereign_account_id_system(actor_ids::BURN_ACTOR_ID);
    let price = 1_000_000_000_000u128;
    assert_ok!(publish_bidirectional_deos_router_observation(
      AssetKind::Local(super::common::ASSET_A),
      AssetKind::Native,
      price,
    ));
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
    let result = <Runtime as pallet_deos_actors::Config>::DexOps::swap_exact_in(
      ExecutionContext::new(&bm, ActorType::System),
      AssetKind::Local(super::common::ASSET_A),
      AssetKind::Native,
      foreign_amount,
      Perbill::from_percent(50),
    );
    assert!(
      result.is_ok(),
      "Foreign→Native swap must succeed: {result:?}"
    );
    assert!(
      result.unwrap().recipient_amount_out > 0,
      "Must receive native tokens"
    );
  });
}

#[test]
fn dexops_normal_swap_succeeds() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let bm = Actors::sovereign_account_id_system(actor_ids::BURN_ACTOR_ID);
    let amount = primitives::ecosystem::params::PRECISION;
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&bm, amount * 10);
    let price = 1_000_000_000_000u128;
    assert_ok!(publish_bidirectional_deos_router_observation(
      AssetKind::Native,
      AssetKind::Local(super::common::ASSET_A),
      price,
    ));
    use pallet_deos_actors::DexOps;
    let result = <Runtime as pallet_deos_actors::Config>::DexOps::swap_exact_in(
      ExecutionContext::new(&bm, ActorType::System),
      AssetKind::Native,
      AssetKind::Local(super::common::ASSET_A),
      amount,
      Perbill::from_percent(50),
    );
    assert!(result.is_ok(), "Normal swap must succeed: {result:?}");
    assert!(result.unwrap().recipient_amount_out > 0);
  });
}

#[test]
fn deos_router_price_deviation_breakpoint_is_bound_to_pool_depth() {
  let reserve = super::common::LIQUIDITY_AMOUNT;
  let fair_price = primitives::ecosystem::params::PRECISION;
  // With equal reserves, a direct XYK quote has normalized price R / (R + x).
  // The 20% guard flips at x > 0.25R. User swaps pay a 0.5% router fee first,
  // so reserve / 5 stays below the guard and reserve / 3 exceeds it.
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    assert_ok!(publish_deos_router_observation(
      AssetKind::Native,
      AssetKind::Local(super::common::ASSET_A),
      fair_price,
    ));
    assert_ok!(crate::DeosRouter::swap(
      RuntimeOrigin::signed(ALICE),
      AssetKind::Native,
      AssetKind::Local(super::common::ASSET_A),
      reserve / 5,
      0,
      ALICE,
      100,
    ));
  });
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    assert_ok!(publish_deos_router_observation(
      AssetKind::Native,
      AssetKind::Local(super::common::ASSET_A),
      fair_price,
    ));
    assert_noop!(
      crate::DeosRouter::swap(
        RuntimeOrigin::signed(ALICE),
        AssetKind::Native,
        AssetKind::Local(super::common::ASSET_A),
        reserve / 3,
        0,
        ALICE,
        100,
      ),
      pallet_deos_router::Error::<Runtime>::PriceDeviationExceeded
    );
  });
}

#[test]
fn oracle_deviation_rejects_swap_via_dexops() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let bm = Actors::sovereign_account_id_system(actor_ids::BURN_ACTOR_ID);
    let amount = 10 * primitives::ecosystem::params::PRECISION;
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&bm, amount);
    let deviated_price = 10_000_000_000_000u128;
    assert_ok!(publish_deos_router_observation(
      AssetKind::Native,
      AssetKind::Local(super::common::ASSET_A),
      deviated_price,
    ));
    let result = <Runtime as pallet_deos_actors::Config>::DexOps::swap_exact_in(
      ExecutionContext::new(&bm, ActorType::System),
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
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let bm = Actors::sovereign_account_id_system(actor_ids::BURN_ACTOR_ID);
    let amount = primitives::ecosystem::params::PRECISION;
    assert_ok!(<crate::configs::actor_config::TmctolAssetOps as AssetOps<
      crate::AccountId,
      AssetKind,
      crate::Balance,
    >>::mint(&bm, AssetKind::Native, amount * 10));
    let price = 1_000_000_000_000u128;
    assert_ok!(publish_bidirectional_deos_router_observation(
      AssetKind::Native,
      AssetKind::Local(super::common::ASSET_A),
      price,
    ));
    let steps: ContractSteps<Runtime> = alloc::vec![pallet_deos_actors::Step {
      precondition: None,
      task: Task::SwapIn {
        asset_in: AssetKind::Native,
        asset_out: AssetKind::Local(super::common::ASSET_A),
        amount_in: AmountResolution::Fixed(amount),
        slippage_tolerance: Perbill::from_percent(5),
      },
      on_error: StepErrorPolicy::AbortCycle,
    },]
    .try_into()
    .unwrap();
    let actor_id = actor_ids::BURN_ACTOR_ID;
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      (steps, CompletionPolicy::Persistent,)
    ));
    let balance_before = crate::Assets::balance(super::common::ASSET_A, &bm);
    System::set_block_number(11);
    Actors::on_initialize(11);
    Actors::on_idle(11, Weight::from_parts(u64::MAX, u64::MAX));
    let balance_after = crate::Assets::balance(super::common::ASSET_A, &bm);
    assert!(
      balance_after > balance_before,
      "Swap with 5% slippage tolerance must succeed under fair conditions"
    );
    assert!(has_actor_event(|event| {
      matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
    }));
  });
}

#[test]
fn swap_without_pool_fails_contract_steps() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let bm_id = actor_ids::BURN_ACTOR_ID;
    let bm = Actors::sovereign_account_id_system(bm_id);
    let steps: ContractSteps<Runtime> = alloc::vec![pallet_deos_actors::Step {
      precondition: None,
      task: Task::SwapIn {
        asset_in: AssetKind::Native,
        asset_out: AssetKind::Local(ASSET_A),
        amount_in: AmountResolution::Fixed(1_000_000_000_000),
        slippage_tolerance: Perbill::from_percent(5),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }]
    .try_into()
    .unwrap();
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      bm_id,
      (steps, CompletionPolicy::Persistent,)
    ));
    assert_ok!(<crate::configs::actor_config::TmctolAssetOps as AssetOps<
      crate::AccountId,
      AssetKind,
      crate::Balance,
    >>::mint(
      &bm,
      AssetKind::Native,
      100 * primitives::ecosystem::params::PRECISION,
    ));
    System::set_block_number(11);
    Actors::on_initialize(11);
    Actors::on_idle(11, Weight::from_parts(u64::MAX, u64::MAX));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id,
          cycle_nonce: 1,
          outcomes,
          ..
        } if *actor_id == bm_id && outcomes.failed_steps >= 1
      )
    }));
  });
}

// --- Liquidity Actor ContractSteps ---

#[test]
fn zap_contract_steps_builder_produces_valid_3_step_contract_steps() {
  use primitives::ecosystem::actor_ids;
  seeded_test_ext().execute_with(|| {
    let foreign = AssetKind::Local(ASSET_A);
    let lp_asset = AssetKind::Local(999);
    let dust = primitives::ecosystem::params::BURN_ACTOR_DUST_THRESHOLD;
    let steps = crate::configs::actor_config::TmctolGenesisSystemActors::build_zap_contract_steps(
      foreign, lp_asset, dust,
    );
    assert_eq!(steps.len(), 3, "Liquidity Actor steps must have 3 steps");
    assert!(matches!(steps[0].task, Task::AddLiquidity { .. }));
    assert_eq!(
      steps[0]
        .precondition
        .as_ref()
        .expect("AddLiquidity has a precondition")
        .predicate_count(),
      2,
      "AddLiquidity needs dual dust guard"
    );
    if let Task::SwapIn {
      asset_in,
      asset_out,
      ..
    } = &steps[1].task
    {
      assert_eq!(*asset_in, foreign);
      assert_eq!(*asset_out, AssetKind::Native);
    } else {
      panic!("Step 2 must be SwapIn");
    }
    if let Task::SplitTransfer { asset, legs, .. } = &steps[2].task {
      assert_eq!(*asset, lp_asset);
      assert_eq!(legs.len(), 4, "Must split to 4 TOL buckets");
      assert_eq!(
        legs[0].to,
        Actors::sovereign_account_id_system(actor_ids::TOL_BUCKET_A_ACTORS_ID)
      );
      assert_eq!(
        legs[1].to,
        Actors::sovereign_account_id_system(actor_ids::TOL_BUCKET_B_ACTORS_ID)
      );
      assert_eq!(
        legs[2].to,
        Actors::sovereign_account_id_system(actor_ids::TOL_BUCKET_C_ACTORS_ID)
      );
      assert_eq!(
        legs[3].to,
        Actors::sovereign_account_id_system(actor_ids::TOL_BUCKET_D_ACTORS_ID)
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
fn zap_contract_steps_tightens_slippage_as_native_depth_grows() {
  seeded_test_ext().execute_with(|| {
    let dust = primitives::ecosystem::params::BURN_ACTOR_DUST_THRESHOLD;
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
      crate::configs::actor_config::TmctolGenesisSystemActors::build_zap_contract_steps(
        shallow_foreign,
        shallow_lp,
        dust,
      );
    let deep_plan =
      crate::configs::actor_config::TmctolGenesisSystemActors::build_zap_contract_steps(
        deep_foreign,
        deep_lp,
        dust,
      );
    let shallow_slippage = match &shallow_plan[1].task {
      Task::SwapIn {
        slippage_tolerance, ..
      } => *slippage_tolerance,
      _ => panic!("Step 2 must be SwapIn"),
    };
    let deep_slippage = match &deep_plan[1].task {
      Task::SwapIn {
        slippage_tolerance, ..
      } => *slippage_tolerance,
      _ => panic!("Step 2 must be SwapIn"),
    };
    assert_eq!(
      shallow_slippage,
      primitives::ecosystem::params::LIQUIDITY_ACTOR_MAX_SWAP_SLIPPAGE
    );
    assert_eq!(deep_slippage, Perbill::from_parts(6_000_000));
    assert!(deep_slippage < shallow_slippage);
  });
}

#[test]
fn zap_contract_steps_uses_max_slippage_when_pool_depth_is_unavailable() {
  seeded_test_ext().execute_with(|| {
    let foreign = AssetKind::Local(ASSET_A);
    assert_eq!(
      crate::configs::actor_config::TmctolGenesisSystemActors::resolve_zap_slippage_tolerance(
        foreign
      ),
      primitives::ecosystem::params::LIQUIDITY_ACTOR_MAX_SWAP_SLIPPAGE
    );
  });
}

#[test]
fn zap_contract_steps_e2e_adds_liquidity_and_splits_lp_to_buckets() {
  use primitives::ecosystem::actor_ids;
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let liquidity_actor = Actors::sovereign_account_id_system(actor_ids::LIQUIDITY_ACTOR_ACTORS_ID);
    let liquidity_actor_id = actor_ids::LIQUIDITY_ACTOR_ACTORS_ID;
    let foreign = AssetKind::Local(ASSET_A);
    let pre_seeded = Balances::free_balance(&liquidity_actor);
    if pre_seeded > 0 {
      let _ = <Balances as Currency<crate::AccountId>>::transfer(
        &liquidity_actor,
        &ALICE,
        pre_seeded - crate::EXISTENTIAL_DEPOSIT,
        polkadot_sdk::frame_support::traits::ExistenceRequirement::KeepAlive,
      );
    }
    let pre_seeded_foreign = crate::Assets::balance(ASSET_A, &liquidity_actor);
    if pre_seeded_foreign > 0 {
      use polkadot_sdk::frame_support::traits::fungibles::Mutate;
      let _ = <crate::Assets as Mutate<crate::AccountId>>::transfer(
        ASSET_A,
        &liquidity_actor,
        &ALICE,
        pre_seeded_foreign,
        polkadot_sdk::frame_support::traits::tokens::Preservation::Expendable,
      );
    }
    let fund_amount = 10 * primitives::ecosystem::params::PRECISION;
    let (_, pool_info) = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::iter()
      .next()
      .expect("pool must exist after setup");
    let lp_asset_id = pool_info.lp_token;
    let lp_asset = AssetKind::Local(lp_asset_id);
    let dust = primitives::ecosystem::params::BURN_ACTOR_DUST_THRESHOLD;
    let steps = crate::configs::actor_config::TmctolGenesisSystemActors::build_zap_contract_steps(
      foreign, lp_asset, dust,
    );
    assert_ok!(activate_dormant_system(liquidity_actor_id, steps));
    assert_ok!(<crate::configs::actor_config::TmctolAssetOps as AssetOps<
      crate::AccountId,
      AssetKind,
      crate::Balance,
    >>::mint(
      &liquidity_actor, AssetKind::Native, fund_amount
    ));
    assert_ok!(<crate::configs::actor_config::TmctolAssetOps as AssetOps<
      crate::AccountId,
      AssetKind,
      crate::Balance,
    >>::mint(&liquidity_actor, foreign, fund_amount));
    let price = 1_000_000_000_000u128;
    assert_ok!(publish_bidirectional_deos_router_observation(
      AssetKind::Native,
      foreign,
      price,
    ));
    System::reset_events();
    for block in 11..=30 {
      System::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    }
    let bucket_a = Actors::sovereign_account_id_system(actor_ids::TOL_BUCKET_A_ACTORS_ID);
    let bucket_b = Actors::sovereign_account_id_system(actor_ids::TOL_BUCKET_B_ACTORS_ID);
    let bucket_c = Actors::sovereign_account_id_system(actor_ids::TOL_BUCKET_C_ACTORS_ID);
    let bucket_d = Actors::sovereign_account_id_system(actor_ids::TOL_BUCKET_D_ACTORS_ID);
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
    let liquidity_actor_lp_remaining = crate::Assets::balance(lp_asset_id, &liquidity_actor);
    assert!(
      liquidity_actor_lp_remaining < dust,
      "Liquidity Actor sovereign LP must be below dust after distribution"
    );
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          outcomes: OutcomeTotals { failed_steps: 0, .. },
          ..
        } if *id == liquidity_actor_id
      )
    }));
  });
}

// --- Foreign-Asset Actor Activation ---

#[test]
fn burn_and_liquidity_actor_activation_for_first_foreign_asset() {
  use polkadot_sdk::frame_support::traits::fungibles::Mutate as FungiblesMutate;
  use polkadot_sdk::frame_support::traits::tokens::{Fortitude, Precision, Preservation};
  use primitives::ecosystem::actor_ids;
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let foreign = AssetKind::Local(super::common::ASSET_A);
    let (_, pool_info) = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::iter()
      .next()
      .expect("pool must exist after setup");
    let lp_asset_id = pool_info.lp_token;
    let lp_asset = AssetKind::Local(lp_asset_id);
    let dust = primitives::ecosystem::params::BURN_ACTOR_DUST_THRESHOLD;
    let bm = Actors::sovereign_account_id_system(actor_ids::BURN_ACTOR_ID);
    let liquidity_actor = Actors::sovereign_account_id_system(actor_ids::LIQUIDITY_ACTOR_ACTORS_ID);
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
    let liquidity_actor_fund_amount = 10 * primitives::ecosystem::params::PRECISION;
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(&bm, bm_fund_amount);
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &liquidity_actor,
      liquidity_actor_fund_amount,
    );
    assert_ok!(super::common::mint_tokens(
      super::common::ASSET_A,
      &ALICE,
      &bm,
      bm_fund_amount
    ));
    assert_ok!(super::common::mint_tokens(
      super::common::ASSET_A,
      &ALICE,
      &liquidity_actor,
      liquidity_actor_fund_amount
    ));
    let price = 1_000_000_000_000u128;
    assert_ok!(publish_bidirectional_deos_router_observation(
      AssetKind::Native,
      foreign,
      price,
    ));
    let burn_contract_steps =
      crate::configs::actor_config::TmctolGenesisSystemActors::build_burn_contract_steps(
        alloc::vec![foreign],
        dust,
      );
    let zap_contract_steps =
      crate::configs::actor_config::TmctolGenesisSystemActors::build_zap_contract_steps(
        foreign, lp_asset, dust,
      );
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      actor_ids::BURN_ACTOR_ID,
      (burn_contract_steps, CompletionPolicy::Persistent,)
    ));
    assert_ok!(activate_dormant_system(
      actor_ids::LIQUIDITY_ACTOR_ACTORS_ID,
      zap_contract_steps,
    ));
    // Explicitly trigger execution since we deposited funds before updating execution plans
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::root(),
      actor_ids::BURN_ACTOR_ID
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::root(),
      actor_ids::LIQUIDITY_ACTOR_ACTORS_ID
    ));
    let issuance_before = Balances::total_issuance();
    let foreign_before_bm = crate::Assets::balance(super::common::ASSET_A, &bm);
    System::reset_events();
    for block in 11..=40 {
      System::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
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
    let bucket_a = Actors::sovereign_account_id_system(actor_ids::TOL_BUCKET_A_ACTORS_ID);
    let bucket_a_lp = crate::Assets::balance(lp_asset_id, &bucket_a);
    assert!(
      bucket_a_lp > 0,
      "Liquidity Actor must distribute LP tokens to TOL buckets"
    );
  });
}

#[test]
fn bucket_lp_transfer_then_treasury_remove_liquidity_fits_production_budget() {
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let foreign = AssetKind::Local(ASSET_A);
    let lp_asset = super::common::get_pool_lp_asset(AssetKind::Native, foreign);
    let lp_id = match lp_asset {
      AssetKind::Local(id) => id,
      _ => panic!("LP must be local"),
    };
    let bucket_id = actor_ids::TOL_BUCKET_C_ACTORS_ID;
    let treasury_id = actor_ids::TREASURY_C_ACTORS_ID;
    let bucket = Actors::sovereign_account_id_system(bucket_id);
    let treasury = Actors::sovereign_account_id_system(treasury_id);
    let initial_lp = 10_000_000u128;
    let independently_supplied_lp = 500_000u128;
    assert_ok!(
      <crate::Assets as FungiblesMutate<crate::AccountId>>::mint_into(lp_id, &bucket, initial_lp,)
    );
    // Treasury admission is asset-specific, not sender-specific: model the configured LP already
    // present from an independent source before the paired Bucket transfer executes.
    assert_ok!(
      <crate::Assets as FungiblesMutate<crate::AccountId>>::mint_into(
        lp_id,
        &treasury,
        independently_supplied_lp,
      )
    );
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &bucket,
      crate::EXISTENTIAL_DEPOSIT,
    );
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &treasury,
      crate::EXISTENTIAL_DEPOSIT,
    );

    let bucket_plan =
      crate::configs::actor_config::TmctolGenesisSystemActors::build_bucket_lp_transfer_contract_steps(
        lp_asset,
        100,
        Perbill::from_percent(10),
        treasury_id,
      );
    let treasury_plan =
      crate::configs::actor_config::TmctolGenesisSystemActors::build_treasury_lp_unwind_contract_steps(
        lp_asset, 100,
      );
    assert_ok!(activate_dormant_system(bucket_id, bucket_plan));
    assert_ok!(activate_dormant_system(treasury_id, treasury_plan));
    System::set_block_number(System::block_number().saturating_add(1));
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      bucket_id,
      (pallet_deos_actors::Trigger::immediate_manual(), 0, None),
    ));
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      treasury_id,
      (pallet_deos_actors::Trigger::cadenced_always(1), 0, None),
    ));

    assert_eq!(
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(lp_id, &treasury),
      independently_supplied_lp
    );
    let treasury_native_before = Balances::free_balance(&treasury);
    let treasury_foreign_before =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(ASSET_A, &treasury);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), bucket_id));
    let budget = <<Runtime as pallet_deos_actors::Config>::ActorOnIdleReserve as
      polkadot_sdk::frame_support::traits::Get<Weight>>::get();
    for block in 2..=8 {
      System::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, budget);
    }

    let bucket_lp = <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(lp_id, &bucket);
    let treasury_lp =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(lp_id, &treasury);
    assert!(bucket_lp < initial_lp);
    assert_eq!(treasury_lp, 1);
    assert!(Balances::free_balance(&treasury) > treasury_native_before);
    assert!(
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(ASSET_A, &treasury)
        > treasury_foreign_before
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary { actor_id, outcomes: OutcomeTotals { failed_steps: 0, .. }, .. }
        if *actor_id == bucket_id || *actor_id == treasury_id
    )));
  });
}

// --- BLDR Domain Integration Tests ---

#[test]
fn native_tmc_mint_routes_collateral_and_tokens_to_default_liquidity_actor_sink() {
  seeded_test_ext().execute_with(|| {
    let foreign_amount = 10 * primitives::ecosystem::params::PRECISION;
    let liquidity_actor_id = actor_ids::LIQUIDITY_ACTOR_ACTORS_ID;
    let liquidity_actor = liquidity_actor_account();
    let steps = ContractSteps::<Runtime>::try_from(vec![pallet_deos_actors::StepOf::<Runtime> {
      precondition: None,
      task: Task::Transfer {
        to: ALICE,
        asset: AssetKind::Native,
        amount: AmountResolution::Fixed(1),
      },
      on_error: StepErrorPolicy::AbortCycle,
    }])
    .expect("liquidity actor test plan fits");
    assert_ok!(activate_dormant_system(liquidity_actor_id, steps));
    assert!(
      !Actors::actor_hot(liquidity_actor_id)
        .expect("liquidity actor hot state")
        .pending_signal
    );
    assert_ok!(TokenMintingCurve::create_curve(
      RuntimeOrigin::root(),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      primitives::ecosystem::params::PRECISION,
      0,
    ));
    let alice_native_before = Balances::free_balance(&ALICE);
    let alice_foreign_before = crate::Assets::balance(ASSET_A, &ALICE);
    let liquidity_actor_native_before = Balances::free_balance(&liquidity_actor);
    let liquidity_actor_foreign_before = crate::Assets::balance(ASSET_A, &liquidity_actor);
    let minted = TokenMintingCurve::mint_with_distribution(
      &ALICE,
      &ALICE,
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      foreign_amount,
    )
    .expect("native TMC mint must succeed");
    let user_allocation = primitives::ecosystem::params::TMC_USER_ALLOCATION.mul_floor(minted);
    let liquidity_actor_allocation = minted.saturating_sub(user_allocation);
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
      Balances::free_balance(&liquidity_actor),
      liquidity_actor_native_before + liquidity_actor_allocation
    );
    assert_eq!(
      crate::Assets::balance(ASSET_A, &liquidity_actor),
      liquidity_actor_foreign_before + foreign_amount
    );
    let hot = Actors::actor_hot(liquidity_actor_id).expect("liquidity actor hot state");
    assert!(
      hot.pending_signal,
      "TMC distribution must notify the liquidity actor directly"
    );
    assert!(
      hot.queue_ticket.is_some() || hot.wakeup_pointer.is_some(),
      "direct TMC ingress must retain exact scheduler readiness"
    );
  });
}

#[test]
fn native_tmc_mint_rejects_wrong_collateral_without_touching_default_liquidity_actor_sink() {
  seeded_test_ext().execute_with(|| {
    let foreign_amount = 10 * primitives::ecosystem::params::PRECISION;
    let liquidity_actor = liquidity_actor_account();
    assert_ok!(TokenMintingCurve::create_curve(
      RuntimeOrigin::root(),
      AssetKind::Native,
      AssetKind::Local(ASSET_A),
      primitives::ecosystem::params::PRECISION,
      0,
    ));
    let alice_native_before = Balances::free_balance(&ALICE);
    let alice_wrong_foreign_before = crate::Assets::balance(ASSET_B, &ALICE);
    let liquidity_actor_native_before = Balances::free_balance(&liquidity_actor);
    let liquidity_actor_wrong_foreign_before = crate::Assets::balance(ASSET_B, &liquidity_actor);
    System::reset_events();
    assert_noop!(
      TokenMintingCurve::mint_with_distribution(
        &ALICE,
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
    assert_eq!(
      Balances::free_balance(&liquidity_actor),
      liquidity_actor_native_before
    );
    assert_eq!(
      crate::Assets::balance(ASSET_B, &liquidity_actor),
      liquidity_actor_wrong_foreign_before
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
  use primitives::ecosystem::{actor_ids, protocol_tokens};
  seeded_test_ext().execute_with(|| {
    let bldr_id = protocol_tokens::BLDR_ASSET_ID;
    let bldr_asset = AssetKind::Local(bldr_id);
    let wrong_collateral = AssetKind::Local(ASSET_A);
    let splitter_sov = Actors::sovereign_account_id_system(actor_ids::BLDR_SPLITTER_ACTORS_ID);
    let mint_amount = 10 * primitives::ecosystem::params::PRECISION;
    let alice_native_before = Balances::free_balance(&ALICE);
    let alice_wrong_foreign_before = crate::Assets::balance(ASSET_A, &ALICE);
    let splitter_native_before = Balances::free_balance(&splitter_sov);
    let splitter_wrong_foreign_before = crate::Assets::balance(ASSET_A, &splitter_sov);
    let splitter_bldr_before = crate::Assets::balance(bldr_id, &splitter_sov);
    let alice_bldr_before = crate::Assets::balance(bldr_id, &ALICE);
    System::reset_events();
    assert_noop!(
      TokenMintingCurve::mint_with_distribution(
        &ALICE,
        &ALICE,
        bldr_asset,
        wrong_collateral,
        mint_amount,
      ),
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
  use primitives::ecosystem::{actor_ids, protocol_tokens};
  seeded_test_ext().execute_with(|| {
    let bldr_id = protocol_tokens::BLDR_ASSET_ID;
    let bldr_asset = AssetKind::Local(bldr_id);
    // BLDR TMC curve created at genesis
    assert!(crate::TokenMintingCurve::has_curve(bldr_asset));
    let curve = crate::TokenMintingCurve::get_curve(bldr_asset).unwrap();
    assert_eq!(curve.foreign_asset, AssetKind::Native);
    let splitter_sov = Actors::sovereign_account_id_system(actor_ids::BLDR_SPLITTER_ACTORS_ID);
    let bldr_liquidity_sov =
      Actors::sovereign_account_id_system(actor_ids::BLDR_LIQUIDITY_ACTOR_ID);
    // Mint BLDR via TMC directly to verify distribution
    let mint_amount = 10 * primitives::ecosystem::params::PRECISION;
    let alice_native_before = Balances::free_balance(&ALICE);
    assert_ok!(crate::TokenMintingCurve::mint_with_distribution(
      &ALICE,
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
    // 7. Verify collateral (NTVE) went directly to the BLDR Liquidity Actor
    let bldr_liquidity_native = Balances::free_balance(&bldr_liquidity_sov);
    assert!(
      bldr_liquidity_native > crate::EXISTENTIAL_DEPOSIT,
      "BLDR Liquidity Actor must receive NTVE collateral from TMC output"
    );
    assert_eq!(
      Balances::free_balance(&splitter_sov),
      crate::EXISTENTIAL_DEPOSIT,
      "BLDR Splitter must receive only the minted BLDR share"
    );
  });
}

#[test]
fn bldr_splitter_distributes_to_liquidity_and_treasury() {
  use polkadot_sdk::frame_support::traits::fungibles::Mutate as FungiblesMutate;
  use primitives::ecosystem::{actor_ids, protocol_tokens};
  seeded_test_ext().execute_with(|| {
    let bldr_id = protocol_tokens::BLDR_ASSET_ID;
    let bldr_asset = AssetKind::Local(bldr_id);
    let dust = primitives::ecosystem::params::BURN_ACTOR_DUST_THRESHOLD;
    let splitter_sov = Actors::sovereign_account_id_system(actor_ids::BLDR_SPLITTER_ACTORS_ID);
    let bldr_liquidity_sov =
      Actors::sovereign_account_id_system(actor_ids::BLDR_LIQUIDITY_ACTOR_ID);
    let bldr_treasury_sov = Actors::sovereign_account_id_system(actor_ids::BLDR_TREASURY_ACTORS_ID);
    // Fund splitter with the minted BLDR share (simulating TMC output)
    let fund_amount = 100 * primitives::ecosystem::params::PRECISION;
    assert_ok!(
      <crate::Assets as FungiblesMutate<crate::AccountId>>::mint_into(
        bldr_id,
        &splitter_sov,
        fund_amount,
      )
    );
    // Activate splitter steps
    let steps =
      crate::configs::actor_config::TmctolGenesisSystemActors::build_bldr_splitter_contract_steps(
        bldr_asset, dust,
      );
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      actor_ids::BLDR_SPLITTER_ACTORS_ID,
      (steps, CompletionPolicy::Persistent,)
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::root(),
      actor_ids::BLDR_SPLITTER_ACTORS_ID,
    ));
    System::reset_events();
    for block in 11..=30 {
      System::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    }
    let liquidity_bldr = crate::Assets::balance(bldr_id, &bldr_liquidity_sov);
    let treasury_bldr = crate::Assets::balance(bldr_id, &bldr_treasury_sov);
    assert!(
      liquidity_bldr > 0,
      "BLDR Liquidity Actor must receive BLDR from splitter"
    );
    assert!(
      treasury_bldr > 0,
      "BLDR Treasury must receive BLDR from splitter"
    );
    // 50/50 split (within rounding tolerance)
    let total_distributed = liquidity_bldr + treasury_bldr;
    let diff = liquidity_bldr.abs_diff(treasury_bldr);
    assert!(diff <= 1, "BLDR split must be 50/50 (diff={})", diff);
    assert!(
      total_distributed >= fund_amount - 2,
      "All funded BLDR must be distributed (total={}, funded={})",
      total_distributed,
      fund_amount
    );
  });
}

// --- BLDR Full E2E: Router → TMC → Splitter → Liquidity Actor → LP → Bucket A ---

#[test]
fn bldr_full_e2e_router_tmc_splitter_liquidity_bucket() {
  use polkadot_sdk::frame_support::traits::fungibles::Inspect as FungiblesInspect;
  use primitives::ecosystem::{actor_ids, protocol_tokens};
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    // BLDR TMC curve created at genesis (9.5), Splitter steps active at genesis (9.6)
    let bldr_id = protocol_tokens::BLDR_ASSET_ID;
    let bldr_asset = AssetKind::Local(bldr_id);
    let dust = primitives::ecosystem::params::BURN_ACTOR_DUST_THRESHOLD;
    let precision = primitives::ecosystem::params::PRECISION;
    let splitter_id = actor_ids::BLDR_SPLITTER_ACTORS_ID;
    let liquidity_id = actor_ids::BLDR_LIQUIDITY_ACTOR_ID;
    let bucket_a_id = actor_ids::BLDR_BUCKET_A_ACTORS_ID;
    let splitter_sov = Actors::sovereign_account_id_system(splitter_id);
    let liquidity_sov = Actors::sovereign_account_id_system(liquidity_id);
    let treasury_sov = Actors::sovereign_account_id_system(actor_ids::BLDR_TREASURY_ACTORS_ID);
    let bucket_a_sov = Actors::sovereign_account_id_system(bucket_a_id);
    // 1. Create the NTVE-BLDR pool so the Liquidity Actor can provision it. Seed it against
    // BLDR buys so the router exercises the TMC mint path rather than XYK.
    super::common::setup_bldr_pool_with_reserves(900 * precision, 10 * precision);
    // 2. Activate the BLDR Liquidity Actor execution plan (AddLiquidity + LP → Bucket A).
    let lp_asset = super::common::get_pool_lp_asset(AssetKind::Native, bldr_asset);
    let liquidity_contract_steps =
      crate::configs::actor_config::TmctolGenesisSystemActors::build_bldr_liquidity_contract_steps(
        bldr_asset, lp_asset, dust,
      );
    assert_ok!(activate_dormant_system(
      liquidity_id,
      liquidity_contract_steps
    ));
    // 3. Keep BLDR liquidity ingress-driven with no cooldown for the chained test.
    System::set_block_number(System::block_number().saturating_add(1));
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      liquidity_id,
      (
        pallet_deos_actors::Trigger::immediate_manual_and_address_event(
          pallet_deos_actors::SourceFilter::Any,
          pallet_deos_actors::AssetFilter::Any,
        ),
        0,
        None,
      ),
    ));
    // 4. User mints BLDR via Router TMC
    let mint_amount = 10 * precision;
    let alice_native_before = Balances::free_balance(&ALICE);
    let alice_bldr_before =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &ALICE);
    assert_ok!(crate::DeosRouter::swap(
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
    // Verify TMC routed collateral directly to the Liquidity Actor and the minted share to Splitter
    let splitter_bldr =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &splitter_sov);
    assert!(
      Balances::free_balance(&liquidity_sov) > crate::EXISTENTIAL_DEPOSIT,
      "BLDR Liquidity Actor must hold NTVE collateral"
    );
    assert!(splitter_bldr > 0, "Splitter must hold BLDR zap share");
    // 5. Run blocks to execute the Splitter → Liquidity Actor → Bucket A chain.
    // The queue scheduler should keep this chain progressing without starvation.
    System::reset_events();
    for block in 2..=40 {
      System::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    }
    // 6. Verify: Splitter distributed BLDR to both Liquidity Actor and Treasury. The Actor may
    // consume received NTVE immediately into LP, so the durable final proof is
    // Bucket A receiving LP plus the Splitter sovereign draining below dust.
    let treasury_bldr =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &treasury_sov);
    assert!(treasury_bldr > 0, "Treasury must have received BLDR");
    // 8. Verify: Bucket A received LP tokens provisioned by the Liquidity Actor.
    if let Some(lp_id) = match lp_asset {
      AssetKind::Local(id) => Some(id),
      _ => None,
    } {
      let bucket_lp =
        <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(lp_id, &bucket_a_sov);
      assert!(
        bucket_lp > 0,
        "Bucket A must hold LP tokens from BLDR Liquidity Actor provisioning"
      );
    }
    // 9. Verify: Splitter distributed its complete BLDR input below dust
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
  use primitives::ecosystem::actor_ids;
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    // Use ASSET_A as buyback target (pool already exists from setup)
    let target_id = super::common::ASSET_A;
    let target_asset = AssetKind::Local(target_id);
    let dust = primitives::ecosystem::params::BURN_ACTOR_DUST_THRESHOLD;
    let slippage = primitives::ecosystem::params::SYSTEM_ACTORS_MAX_SWAP_SLIPPAGE;
    let buyback_pct = primitives::ecosystem::params::TREASURY_B_BUYBACK_PCT;
    let treasury_b_id = actor_ids::TREASURY_B_ACTORS_ID;
    let treasury_b_sov = Actors::sovereign_account_id_system(treasury_b_id);
    // Fund Treasury B with NTVE (large enough that 0.042% > dust threshold)
    let fund_amount = 10_000 * primitives::ecosystem::params::PRECISION;
    let _ =
      <Balances as Currency<crate::AccountId>>::deposit_creating(&treasury_b_sov, fund_amount);
    // Activate buyback steps
    let steps =
      crate::configs::actor_config::TmctolGenesisSystemActors::build_treasury_b_buyback_contract_steps(
        target_asset,
        buyback_pct,
        dust,
        slippage,
      );
    assert_ok!(activate_dormant_system(treasury_b_id, steps));
    System::set_block_number(System::block_number().saturating_add(1));
    assert_ok!(update_actor_contract_partial!(
      RuntimeOrigin::root(),
      treasury_b_id,
      (pallet_deos_actors::Trigger::cadenced_always(10), 5, None),
    ));
    let target_supply_before =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::total_issuance(target_id);
    let native_before = Balances::free_balance(&treasury_b_sov);
    System::reset_events();
    for block in 11..=100 {
      System::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    }
    // Verify at least one cycle executed
    assert!(
      has_actor_event(|event| {
        matches!(event, pallet_deos_actors::Event::CycleSummary { actor_id, .. } if *actor_id == treasury_b_id)
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
    // Verify: PreserveSpend retains the target asset minimum after buyback burn.
    let treasury_target =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(target_id, &treasury_b_sov);
    assert_eq!(
      treasury_target, 1,
      "Treasury B must preserve the target asset minimum"
    );
  });
}

// --- Router TMC Efficiency Arbitration ---

#[test]
fn router_selects_tmc_over_xyk_when_tmc_price_is_better() {
  use primitives::ecosystem::protocol_tokens;
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let bldr_id = protocol_tokens::BLDR_ASSET_ID;
    let bldr_asset = AssetKind::Local(bldr_id);
    let precision = primitives::ecosystem::params::PRECISION;
    // BLDR TMC curve exists from genesis; create NTVE-BLDR XYK pool with an
    // unfavorable price. The skew must beat the TMC recipient allocation, not
    // total curve emission.
    super::common::setup_bldr_pool_with_reserves(900 * precision, 10 * precision);
    // Swap via Router — should select TMC (better price at low supply)
    let mint_amount = precision;
    let quote =
      crate::DeosRouter::quote_exact_input(ALICE, AssetKind::Native, bldr_asset, mint_amount)
        .expect("direct-mint quote must exist");
    let splitter_sov = Actors::sovereign_account_id_system(actor_ids::BLDR_SPLITTER_ACTORS_ID);
    let liquidity_sov = Actors::sovereign_account_id_system(actor_ids::BLDR_LIQUIDITY_ACTOR_ID);
    let burn_actor_before = Balances::free_balance(&burn_actor_account());
    let liquidity_native_before = Balances::free_balance(&liquidity_sov);
    let splitter_bldr_before =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &splitter_sov);
    let alice_bldr_before =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &ALICE);
    assert_ok!(crate::DeosRouter::swap(
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
    let liquidity_native_after = Balances::free_balance(&liquidity_sov);
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
        if let crate::RuntimeEvent::DeosRouter(pallet_deos_router::Event::SwapExecuted {
          outcome,
          ..
        }) = &r.event
        {
          Some(outcome.family)
        } else {
          None
        }
      })
      .expect("SwapExecuted event must exist");
    assert_eq!(
      used_mechanism,
      pallet_deos_router::RouteFamily::DirectMint,
      "Router must select TMC (DirectMint) when TMC price is better than XYK"
    );
    assert_eq!(
      Balances::free_balance(&burn_actor_account()) - burn_actor_before,
      quote.router_fee,
      "Router direct-mint path must route the native fee to the Burn Actor"
    );
    assert_eq!(
      liquidity_native_after - liquidity_native_before,
      quote.amount_after_fee,
      "BLDR Liquidity Actor must receive the post-fee native collateral"
    );
    assert_eq!(
      received, quote.amount_out,
      "Router direct-mint output must be the recipient BLDR delta"
    );
    assert!(
      splitter_bldr_after > splitter_bldr_before,
      "Protocol sink must still receive the non-recipient BLDR allocation"
    );
  });
}

#[test]
fn router_selects_xyk_when_tmc_price_exceeds_xyk() {
  use polkadot_sdk::frame_support::traits::fungibles::Mutate as FungiblesMutate;
  use primitives::ecosystem::protocol_tokens;
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
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
    assert_ok!(crate::DeosRouter::swap(
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
        if let crate::RuntimeEvent::DeosRouter(pallet_deos_router::Event::SwapExecuted {
          outcome,
          ..
        }) = &r.event
        {
          Some(outcome.family)
        } else {
          None
        }
      })
      .expect("SwapExecuted event must exist");
    assert_eq!(
      used_mechanism,
      pallet_deos_router::RouteFamily::DirectXyk,
      "Router must select XYK (DirectXyk) when TMC price is worse"
    );
  });
}

#[test]
fn router_multi_hop_foreign_to_bldr() {
  use primitives::ecosystem::protocol_tokens;
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let bldr_id = protocol_tokens::BLDR_ASSET_ID;
    let bldr_asset = AssetKind::Local(bldr_id);
    let foreign = AssetKind::Local(ASSET_A);
    let precision = primitives::ecosystem::params::PRECISION;
    // Create NTVE-BLDR XYK pool (needed for hop 2)
    super::common::setup_bldr_pool(100 * precision);
    // NTVE-Foreign pool already exists from setup_deos_router_infrastructure (hop 1)
    // No direct Foreign-BLDR pool → forces multi-hop: Foreign→NTVE→BLDR
    let swap_amount = precision;
    let alice_bldr_before =
      <crate::Assets as FungiblesInspect<crate::AccountId>>::balance(bldr_id, &ALICE);
    System::reset_events();
    assert_ok!(crate::DeosRouter::swap(
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
        if let crate::RuntimeEvent::DeosRouter(pallet_deos_router::Event::SwapExecuted {
          outcome,
          ..
        }) = &r.event
        {
          Some(outcome.family)
        } else {
          None
        }
      })
      .expect("SwapExecuted event must exist");
    assert_eq!(
      used_mechanism,
      pallet_deos_router::RouteFamily::NativeAnchoredXyk,
      "Router must use multi-hop (Foreign→NTVE→BLDR) when no direct pool exists"
    );
  });
}

#[test]
fn tol_bucket_drainage_pressure_respects_anchor_immutability() {
  use polkadot_sdk::frame_support::{assert_noop, traits::fungibles::Mutate};
  seeded_test_ext().execute_with(|| {
    assert_ok!(super::common::setup_deos_router_infrastructure());
    let (pool_key, pool_info) = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::iter()
      .next()
      .expect("pool must exist after setup");
    let lp_asset_id = pool_info.lp_token;
    let lp_asset = AssetKind::Local(lp_asset_id);
    let (asset_a, asset_b) = pool_key;
    let bucket_ids = [
      actor_ids::TOL_BUCKET_A_ACTORS_ID,
      actor_ids::TOL_BUCKET_B_ACTORS_ID,
      actor_ids::TOL_BUCKET_C_ACTORS_ID,
      actor_ids::TOL_BUCKET_D_ACTORS_ID,
    ];
    let mut before_lp = alloc::vec::Vec::new();
    for bucket_id in bucket_ids {
      let bucket = Actors::sovereign_account_id_system(bucket_id);
      let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
        &bucket,
        crate::EXISTENTIAL_DEPOSIT,
      );
      assert_ok!(<crate::Assets as Mutate<crate::AccountId>>::mint_into(
        lp_asset_id,
        &bucket,
        1_000_000_000,
      ));
      let steps: ContractSteps<Runtime> = alloc::vec![pallet_deos_actors::Step {
        precondition: None,
        task: Task::RemoveLiquidity {
          lp_asset,
          asset_a,
          asset_b,
          lp_amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(10)),
          min_amount_a: 1,
          min_amount_b: 1,
        },
        on_error: StepErrorPolicy::AbortCycle,
      }]
      .try_into()
      .expect("steps must fit");
      if bucket_id == actor_ids::TOL_BUCKET_A_ACTORS_ID {
        assert_noop!(
          update_actor_contract_partial!(
            RuntimeOrigin::root(),
            bucket_id,
            (steps, CompletionPolicy::Persistent,)
          ),
          pallet_deos_actors::Error::<Runtime>::ActorNotFound
        );
      } else {
        assert_ok!(activate_dormant_system(bucket_id, steps));
        assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), bucket_id));
      }
      before_lp.push((bucket_id, crate::Assets::balance(lp_asset_id, &bucket)));
    }
    for block in 20..=40 {
      System::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::MAX);
    }
    for (bucket_id, before) in before_lp {
      let bucket = Actors::sovereign_account_id_system(bucket_id);
      let after = crate::Assets::balance(lp_asset_id, &bucket);
      if bucket_id == actor_ids::TOL_BUCKET_A_ACTORS_ID {
        assert_eq!(after, before, "Bucket A LP principal must remain anchored");
      } else {
        assert!(
          after < before,
          "Bucket {} LP should decrease under drainage pressure ({} -> {})",
          bucket_id,
          before,
          after
        );
        assert!(after > 0, "Bucket {} should retain non-zero LP", bucket_id);
      }
    }
  });
}
