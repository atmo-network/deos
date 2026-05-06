use super::common::{
  ALICE, BOB, INITIAL_BALANCE, aaa_fee_sink_account, add_liquidity, create_pool, new_test_ext,
};
use crate::{
  AAA, Assets, Balances, Runtime, RuntimeOrigin, Staking,
  configs::{AssetKind, RuntimeFeeSplit, aaa_config::TmctolGenesisSystemAaas},
};
use polkadot_sdk::frame_support::{
  assert_ok,
  traits::{
    Currency, Hooks, OnUnbalanced,
    fungible::Balanced,
    fungibles::Inspect,
    tokens::{Fortitude, Precision, Preservation},
  },
  weights::Weight,
};
use polkadot_sdk::pallet_asset_conversion::PoolLocator;

#[test]
fn transaction_fee_split_routes_to_fee_sink_when_author_is_unresolved() {
  new_test_ext().execute_with(|| {
    let fee_sink = aaa_fee_sink_account();
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
    RuntimeFeeSplit::on_unbalanced(credit);
    assert_eq!(Balances::free_balance(&fee_sink), sink_before + amount);
    assert_eq!(Balances::free_balance(&ALICE), INITIAL_BALANCE - amount);
  });
}

#[test]
fn fee_sink_actor_splits_phase1_native_flow_to_staking_and_lp_ingress() {
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
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(BOB), 500));
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
    assert_ok!(TmctolGenesisSystemAaas::activate_native_staking_lp_farming(
      1
    ));
    let fee_sink = aaa_fee_sink_account();
    let staking_pool = Staking::pool_account_for(native_asset_id);
    let lp_farmer = AAA::sovereign_account_id_system(
      primitives::ecosystem::aaa_ids::NATIVE_STAKING_LP_FARMER_AAA_ID,
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
    let _ = <Balances as Currency<_>>::deposit_creating(&fee_sink, amount);
    crate::System::set_block_number(2);
    let _ = AAA::on_initialize(2);
    let _ = AAA::on_idle(2, Weight::from_parts(u64::MAX, u64::MAX));
    for block in 3..=12 {
      crate::System::set_block_number(block);
      let _ = AAA::on_initialize(block);
      let _ = AAA::on_idle(block, Weight::from_parts(u64::MAX, u64::MAX));
    }
    let lp_supply_after =
      <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolAssets::total_issuance(
        pool.lp_token,
      );
    assert_eq!(lp_supply_after, lp_supply_before);
    assert!(Assets::balance(native_asset_id, &staking_pool) > pool_native_asset_before);
    assert_eq!(Balances::free_balance(&staking_pool), 0);
    assert!(Assets::balance(native_asset_id, &lp_farmer) <= 1);
    assert_eq!(Balances::free_balance(&lp_farmer), 0);
    assert!(Assets::balance(native_asset_id, &pool_account) > lp_pool_native_before);
    assert!(Assets::balance(staked_asset_id, &pool_account) > lp_pool_staked_before);
    assert_eq!(
      Balances::free_balance(&fee_sink),
      crate::EXISTENTIAL_DEPOSIT
    );
  });
}
