use super::common::{
  ALICE, INITIAL_BALANCE, actor_fee_sink_account, new_test_ext, set_consensus_timestamp,
};
#[cfg(not(feature = "runtime-benchmarks"))]
use super::common::{BOB, add_liquidity, create_pool};
use crate::{
  Actors, Balances, RuntimeOrigin, Staking,
  configs::{AssetKind, RuntimeFeeCollector, actor_config::TmctolFeeCollector},
};
#[cfg(not(feature = "runtime-benchmarks"))]
use crate::{
  Assets, Runtime,
  configs::actor_config::{TmctolAssetOps, TmctolGenesisSystemActors},
};
#[cfg(not(feature = "runtime-benchmarks"))]
use pallet_deos_actors::AssetOps;
use pallet_deos_actors::{FeeCollector, TriggerRuntimeState, WakeupKey};
#[cfg(not(feature = "runtime-benchmarks"))]
use polkadot_sdk::frame_support::traits::fungibles::Inspect;
use polkadot_sdk::frame_support::{
  assert_ok,
  traits::{
    Hooks, OnUnbalanced,
    fungible::Balanced,
    tokens::{Fortitude, Precision, Preservation},
  },
  weights::Weight,
};
#[cfg(not(feature = "runtime-benchmarks"))]
use polkadot_sdk::pallet_asset_conversion::PoolLocator;

fn run_at_cadence_tick(key: WakeupKey<crate::BlockNumber>) {
  let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
  let key = if Actors::actor_hot(fee_sink_id).is_some_and(|hot| {
    matches!(
      hot.trigger_runtime_state,
      TriggerRuntimeState::Cadenced { anchor_tick: None }
    )
  }) {
    initialize_genesis_fee_sink_cadence(1_000)
  } else {
    key
  };
  let WakeupKey::Tick(tick) = key else {
    panic!("cadenced actor must own a timestamp-tick wakeup");
  };
  set_consensus_timestamp(
    tick.saturating_mul(primitives::ecosystem::params::ACTOR_CADENCE_TICK_MILLIS),
  );
  let block = crate::System::block_number().saturating_add(1);
  crate::System::set_block_number(block);
  let _ = Actors::on_initialize(block);
  let _ = Actors::on_idle(block, Weight::MAX);
  let eligible_block = block.saturating_add(1);
  crate::System::set_block_number(eligible_block);
  let _ = Actors::on_initialize(eligible_block);
  let _ = Actors::on_idle(eligible_block, Weight::MAX);
}

fn cadence_key(actor_id: pallet_deos_actors::ActorId) -> WakeupKey<crate::BlockNumber> {
  WakeupKey::Tick(
    Actors::actor_hot(actor_id)
      .and_then(|hot| hot.trigger_wakeup_pointer)
      .expect("Cadenced Trigger deadline exists")
      .tick,
  )
}

fn initialize_genesis_fee_sink_cadence(timestamp_millis: u64) -> WakeupKey<crate::BlockNumber> {
  let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
  set_consensus_timestamp(timestamp_millis);
  let block = crate::System::block_number().saturating_add(1);
  crate::System::set_block_number(block);
  let _ = Actors::on_initialize(block);
  let _ = Actors::on_idle(block, Weight::MAX);
  cadence_key(fee_sink_id)
}

#[test]
fn runtime_fee_collector_routes_the_full_credit_to_fee_sink() {
  new_test_ext().execute_with(|| {
    let fee_sink = actor_fee_sink_account();
    let amount = 1_000_000_000_000u128;
    let sink_before = Balances::free_balance(&fee_sink);
    let credit = <Balances as Balanced<_>>::withdraw(
      &ALICE,
      amount,
      Precision::Exact,
      Preservation::Preserve,
      Fortitude::Polite,
    )
    .expect("Alice has enough balance for fee withdrawal");
    RuntimeFeeCollector::on_unbalanced(credit);
    assert_eq!(Balances::free_balance(&fee_sink), sink_before + amount);
    assert_eq!(Balances::free_balance(&ALICE), INITIAL_BALANCE - amount);
  });
}

#[test]
#[cfg(not(feature = "runtime-benchmarks"))]
fn source_less_runtime_fee_credit_is_processed_by_fee_sink_cadence() {
  new_test_ext().execute_with(|| {
    let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let fee_sink = actor_fee_sink_account();
    let staking_pool = Staking::pool_account_for(0);
    let staking_liquidity_actor = Actors::sovereign_account_id_system(
      primitives::ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
    );
    let amount = 1_000_000_000_000u128;
    let pool_before = Balances::free_balance(&staking_pool);
    let liquidity_before = Balances::free_balance(&staking_liquidity_actor);
    let credit = <Balances as Balanced<_>>::withdraw(
      &ALICE,
      amount,
      Precision::Exact,
      Preservation::Preserve,
      Fortitude::Polite,
    )
    .expect("Alice has enough balance for fee withdrawal");
    RuntimeFeeCollector::on_unbalanced(credit);
    assert!(!Actors::pending_signal(fee_sink_id));
    let cadence_block = cadence_key(fee_sink_id);

    run_at_cadence_tick(cadence_block);

    let pool_delta = Balances::free_balance(&staking_pool).saturating_sub(pool_before);
    let liquidity_delta =
      Balances::free_balance(&staking_liquidity_actor).saturating_sub(liquidity_before);
    assert_eq!(
      pool_delta.saturating_add(liquidity_delta),
      primitives::ecosystem::params::FEE_SINK_BUFFER_PCT.mul_floor(amount)
    );
    assert_eq!(
      Balances::free_balance(&fee_sink),
      crate::EXISTENTIAL_DEPOSIT.saturating_add(
        amount
          .saturating_sub(primitives::ecosystem::params::FEE_SINK_BUFFER_PCT.mul_floor(amount),),
      )
    );
  });
}

#[test]
fn fee_sink_cadence_anchors_at_first_consensus_timestamp_and_never_executes_early() {
  new_test_ext().execute_with(|| {
    let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let initial = cadence_key(fee_sink_id);
    assert_eq!(initial, WakeupKey::Tick(0));
    assert!(matches!(
      Actors::actor_hot(fee_sink_id)
        .expect("Fee Sink hot state exists")
        .trigger_runtime_state,
      TriggerRuntimeState::Cadenced { anchor_tick: None }
    ));

    let due = initialize_genesis_fee_sink_cadence(1_000);
    assert_eq!(due, WakeupKey::Tick(122));
    assert_eq!(
      Actors::active_actor_state(fee_sink_id)
        .expect("Fee Sink remains active")
        .identity
        .cycle_nonce,
      0,
    );

    set_consensus_timestamp(60_999);
    crate::System::set_block_number(10_000);
    let _ = Actors::on_initialize(10_000);
    polkadot_sdk::cumulus_pallet_parachain_system::ValidationData::<crate::Runtime>::put(
      polkadot_sdk::cumulus_primitives_core::PersistedValidationData::default(),
    );
    assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
    let _ = Actors::on_idle(10_000, Weight::MAX);
    assert_eq!(
      Actors::active_actor_state(fee_sink_id)
        .expect("Fee Sink remains active")
        .identity
        .cycle_nonce,
      0,
    );

    set_consensus_timestamp(61_000);
    crate::System::set_block_number(10_001);
    let _ = Actors::on_initialize(10_001);
    pallet_deos_actors::CurrentBlockResourceState::<crate::Runtime>::kill();
    assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
    let _ = Actors::on_idle(10_001, Weight::MAX);
    assert_eq!(
      Actors::active_actor_state(fee_sink_id)
        .expect("Fee Sink remains active")
        .identity
        .cycle_nonce,
      0,
    );

    crate::System::set_block_number(10_002);
    let _ = Actors::on_initialize(10_002);
    pallet_deos_actors::CurrentBlockResourceState::<crate::Runtime>::kill();
    assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
    let _ = Actors::on_idle(10_002, Weight::MAX);
    assert_eq!(
      Actors::active_actor_state(fee_sink_id)
        .expect("Fee Sink remains active")
        .identity
        .cycle_nonce,
      1,
    );
  });
}

#[test]
fn repeated_low_volume_fee_sink_distributions_preserve_anchors_without_failures() {
  new_test_ext().execute_with(|| {
    let fee_sink = actor_fee_sink_account();
    let staking_pool = Staking::pool_account_for(0);
    let staking_liquidity_actor = Actors::sovereign_account_id_system(
      primitives::ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
    );
    let anchor = crate::EXISTENTIAL_DEPOSIT;
    assert_eq!(Balances::free_balance(&fee_sink), anchor);
    assert_eq!(Balances::free_balance(&staking_pool), anchor);
    assert_eq!(Balances::free_balance(&staking_liquidity_actor), anchor);

    for _ in 0..3 {
      assert_ok!(TmctolFeeCollector::collect_fee(
        &ALICE,
        &fee_sink,
        AssetKind::Native,
        2,
      ));
      let cadence_block = cadence_key(primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID);
      run_at_cadence_tick(cadence_block);
    }

    assert_eq!(Balances::free_balance(&fee_sink), anchor + 6);
    assert_eq!(Balances::free_balance(&staking_pool), anchor);
    assert_eq!(Balances::free_balance(&staking_liquidity_actor), anchor);
    let actor = Actors::active_actor_state(primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID)
      .expect("Fee Sink actor remains active");
    assert_eq!(actor.identity.cycle_nonce, 3);
    assert_eq!(actor.hot.unsuccessful_attempt_streak, 0);
  });
}

#[test]
#[cfg(not(feature = "runtime-benchmarks"))]
fn fee_sink_threshold_admits_exactly_one_ed_per_permissioned_leg() {
  new_test_ext().execute_with(|| {
    let fee_sink_id = primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID;
    let fee_sink = actor_fee_sink_account();
    let staking_pool = Staking::pool_account_for(0);
    let staking_liquidity_actor = Actors::sovereign_account_id_system(
      primitives::ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
    );
    let anchor = crate::EXISTENTIAL_DEPOSIT;
    let amount = anchor.saturating_mul(20);
    assert_ok!(TmctolFeeCollector::collect_fee(
      &ALICE,
      &fee_sink,
      AssetKind::Native,
      amount,
    ));
    let cadence_block = cadence_key(fee_sink_id);
    run_at_cadence_tick(cadence_block);

    assert_eq!(Balances::free_balance(&staking_pool), anchor * 2);
    assert_eq!(Balances::free_balance(&staking_liquidity_actor), anchor * 2);
    assert_eq!(Balances::free_balance(&fee_sink), anchor + 18 * anchor);
  });
}

#[test]
#[cfg(not(feature = "runtime-benchmarks"))]
fn fee_sink_actor_splits_trusted_set_native_flow_to_staking_and_lp_ingress() {
  new_test_ext().execute_with(|| {
    let native_asset_id = 0;
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      native_asset_id,
      ALICE.into(),
      true,
      1,
    ));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      native_asset_id,
    ));
    assert_ok!(Assets::mint(
      RuntimeOrigin::signed(ALICE),
      native_asset_id,
      BOB.into(),
      1_000,
    ));
    assert_ok!(Staking::stake(
      RuntimeOrigin::signed(BOB),
      native_asset_id,
      500
    ));
    let staked_asset_id = Staking::staked_asset_id(native_asset_id).expect("stNTVE must resolve");
    let base_asset = AssetKind::Local(native_asset_id);
    let staked_asset = AssetKind::Local(staked_asset_id);
    assert_ok!(create_pool(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset
    ));
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset,
      400,
      400,
      1,
      1,
      &BOB,
    ));
    assert_ok!(TmctolGenesisSystemActors::activate_native_staking_liquidity_actor(1));
    let fee_sink = actor_fee_sink_account();
    let staking_pool = Staking::pool_account_for(native_asset_id);
    let staking_liquidity_actor = Actors::sovereign_account_id_system(
      primitives::ecosystem::actor_ids::NATIVE_STAKING_LIQUIDITY_ACTOR_ID,
    );
    let pool_id = <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::pool_id(
      &base_asset,
      &staked_asset,
    )
    .expect("NTVE/stNTVE pool id must resolve");
    let pool_account =
      <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::address(&pool_id)
        .expect("NTVE/stNTVE pool account must resolve");
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get(&pool_id)
      .expect("NTVE/stNTVE pool must exist");
    let lp_supply_before =
      <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolAssets::total_issuance(
        pool.lp_token,
      );
    let amount = 1_000_000_000_000u128;
    let pool_native_asset_before = Assets::balance(native_asset_id, &staking_pool);
    let lp_pool_native_before = Assets::balance(native_asset_id, &pool_account);
    let lp_pool_staked_before = Assets::balance(staked_asset_id, &pool_account);
    assert_ok!(<TmctolAssetOps as AssetOps<
      crate::AccountId,
      AssetKind,
      crate::Balance,
    >>::mint(&fee_sink, AssetKind::Native, amount));
    let cadence = cadence_key(primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID);
    run_at_cadence_tick(cadence);
    let followup_block = crate::System::block_number().saturating_add(1);
    crate::System::set_block_number(followup_block);
    let _ = Actors::on_initialize(followup_block);
    let _ = Actors::on_idle(followup_block, Weight::MAX);
    let lp_supply_after =
      <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolAssets::total_issuance(
        pool.lp_token,
      );
    assert_eq!(lp_supply_after, lp_supply_before);
    assert!(Assets::balance(native_asset_id, &staking_pool) > pool_native_asset_before);
    assert_eq!(
      Balances::free_balance(&staking_pool),
      crate::EXISTENTIAL_DEPOSIT
    );
    assert!(Assets::balance(native_asset_id, &staking_liquidity_actor) <= 1);
    assert_eq!(
      Balances::free_balance(&staking_liquidity_actor),
      crate::EXISTENTIAL_DEPOSIT
    );
    assert!(Assets::balance(native_asset_id, &pool_account) > lp_pool_native_before);
    assert!(Assets::balance(staked_asset_id, &pool_account) > lp_pool_staked_before);
    assert_eq!(
      Balances::free_balance(&fee_sink),
      crate::EXISTENTIAL_DEPOSIT.saturating_add(
        amount
          .saturating_sub(primitives::ecosystem::params::FEE_SINK_BUFFER_PCT.mul_floor(amount),),
      )
    );
  });
}
