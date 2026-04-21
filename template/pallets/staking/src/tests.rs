use crate::{Error, Event, mock::*};
use polkadot_sdk::frame_support::{
  assert_noop, assert_ok,
  traits::{
    Hooks,
    fungibles::{Inspect, Mutate, metadata::Inspect as MetadataInspect},
  },
  weights::Weight,
};
use polkadot_sdk::sp_runtime::FixedU128;

fn advance_to_block(target: u64) {
  while System::block_number() < target {
    let current = System::block_number();
    let _ = Staking::on_idle(current, Weight::MAX);
    Staking::on_finalize(current);
    System::reset_events();
    let next = current.saturating_add(1);
    System::set_block_number(next);
    let _ = Staking::on_initialize(next);
  }
}

#[test]
fn staked_asset_id_resolution_matches_current_namespace_contract() {
  const TYPE_FOREIGN: AssetId = 0xF000_0000;
  const TYPE_STAKED: AssetId = 0x5000_0000;
  const TYPE_STAKED_FOREIGN: AssetId = 0x6000_0000;
  new_test_ext().execute_with(|| {
    assert_eq!(Staking::staked_asset_id(1), Some(TYPE_STAKED));
    assert_eq!(Staking::staked_asset_id(2), Some(TYPE_STAKED | 2));
    assert_eq!(
      Staking::staked_asset_id(TYPE_FOREIGN | 2),
      Some(TYPE_STAKED_FOREIGN | 2)
    );
  });
}

#[test]
fn live_native_staked_receipt_balance_reads_current_receipt_asset_balance() {
  const TYPE_STAKED: AssetId = 0x5000_0000;
  new_test_ext().execute_with(|| {
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      TYPE_STAKED,
      1,
      true,
      1
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(
      TYPE_STAKED,
      &1,
      42
    ));
    assert_eq!(Staking::live_native_staked_receipt_balance(&1), Some(42));
    assert_eq!(Staking::live_native_staked_receipt_balance(&2), Some(0));
  });
}

#[test]
fn staked_receipt_value_tracks_pool_share_price_from_live_receipt_balance() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    let pool_account = Staking::pool_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &pool_account,
      100,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_ok!(Staking::sync_pool(RuntimeOrigin::signed(99), 2));
    assert_eq!(Staking::staked_receipt_value(2, &1), Some(200));
  });
}

#[test]
fn query_surface_prefers_receipt_balances_once_receipts_exist() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      TYPE_STAKED_LOCAL,
      &1,
      &2,
      60,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_eq!(
      Staking::effective_share_balance_for_queries(2, &1),
      Some(40)
    );
    assert_eq!(Staking::stake_fraction(2, &1), Some((40, 100)));
    assert_eq!(Staking::stake_value(2, &1), Some(40));
  });
}

#[test]
fn mixed_legacy_and_receipt_shares_sum_in_query_surface() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    let pool_account = Staking::pool_account_for(2);
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      TYPE_STAKED_LOCAL,
      pool_account,
      true,
      1,
    ));
    crate::Pools::<Test>::insert(
      2,
      crate::PoolState {
        total_shares: 100,
        accounted_balance: 100,
        active_staker_count: 1,
      },
    );
    crate::Positions::<Test>::insert(2, 1, crate::StakePosition { shares: 60 });
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(
      2,
      &pool_account,
      100
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(
      TYPE_STAKED_LOCAL,
      &1,
      40
    ));
    assert_eq!(
      Staking::effective_share_balance_for_queries(2, &1),
      Some(100)
    );
    assert_eq!(Staking::stake_fraction(2, &1), Some((100, 100)));
    assert_eq!(Staking::stake_value(2, &1), Some(100));
  });
}

#[test]
fn legacy_pool_with_receipt_asset_mints_new_stakes_as_receipts() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    let pool_account = Staking::pool_account_for(2);
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      TYPE_STAKED_LOCAL,
      pool_account,
      true,
      1,
    ));
    crate::Pools::<Test>::insert(
      2,
      crate::PoolState {
        total_shares: 100,
        accounted_balance: 100,
        active_staker_count: 1,
      },
    );
    crate::Positions::<Test>::insert(2, 1, crate::StakePosition { shares: 100 });
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(
      2,
      &pool_account,
      100
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(2), 2, 50));
    let pool = Staking::pool(2).expect("pool must exist");
    assert_eq!(pool.total_shares, 150);
    assert_eq!(pool.accounted_balance, 150);
    assert_eq!(pool.active_staker_count, 1);
    assert_eq!(Staking::position(2, 2), None);
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(TYPE_STAKED_LOCAL, &2),
      50
    );
    assert_eq!(Staking::stake_value(2, &1), Some(100));
    assert_eq!(Staking::stake_value(2, &2), Some(50));
  });
}

#[test]
fn legacy_position_holder_can_unstake_after_receipt_mode_starts() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    let pool_account = Staking::pool_account_for(2);
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      TYPE_STAKED_LOCAL,
      pool_account,
      true,
      1,
    ));
    crate::Pools::<Test>::insert(
      2,
      crate::PoolState {
        total_shares: 150,
        accounted_balance: 150,
        active_staker_count: 1,
      },
    );
    crate::Positions::<Test>::insert(2, 1, crate::StakePosition { shares: 100 });
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(
      2,
      &pool_account,
      150
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(
      TYPE_STAKED_LOCAL,
      &2,
      50
    ));
    let before = <Assets as Inspect<AccountId>>::balance(2, &1);
    assert_ok!(Staking::unstake(RuntimeOrigin::signed(1), 2, 40));
    let after = <Assets as Inspect<AccountId>>::balance(2, &1);
    assert_eq!(after - before, 40);
    assert_eq!(
      Staking::position(2, 1),
      Some(crate::StakePosition { shares: 60 })
    );
    assert_eq!(Staking::stake_value(2, &1), Some(60));
    assert_eq!(Staking::stake_value(2, &2), Some(50));
  });
}

#[test]
fn transferred_receipt_holder_can_unstake_via_receipt_balance() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(TYPE_STAKED_LOCAL, &1),
      100
    );
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      TYPE_STAKED_LOCAL,
      &1,
      &2,
      30,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    let before = <Assets as Inspect<AccountId>>::balance(2, &2);
    assert_ok!(Staking::unstake(RuntimeOrigin::signed(2), 2, 30));
    let after = <Assets as Inspect<AccountId>>::balance(2, &2);
    assert_eq!(after - before, 30);
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(TYPE_STAKED_LOCAL, &2),
      0
    );
    assert_eq!(Staking::stake_value(2, &1), Some(70));
    assert_eq!(Staking::stake_value(2, &2), None);
  });
}

#[test]
fn register_asset_creates_empty_pool() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    let pool = Staking::pool(2).expect("pool must exist");
    assert_eq!(pool.total_shares, 0);
    assert_eq!(pool.accounted_balance, 0);
    assert_eq!(pool.active_staker_count, 0);
    assert!(<Assets as Inspect<AccountId>>::asset_exists(
      TYPE_STAKED_LOCAL
    ));
    assert_eq!(
      <Assets as MetadataInspect<AccountId>>::name(TYPE_STAKED_LOCAL),
      b"Staked Asset 2".to_vec()
    );
    assert_eq!(
      <Assets as MetadataInspect<AccountId>>::symbol(TYPE_STAKED_LOCAL),
      b"stASSET2".to_vec()
    );
    assert_eq!(
      <Assets as MetadataInspect<AccountId>>::decimals(TYPE_STAKED_LOCAL),
      12
    );
    System::assert_last_event(RuntimeEvent::Staking(Event::StakingAssetRegistered {
      asset_id: 2,
      pool_account: Staking::pool_account_for(2),
      reward_account: Staking::reward_account_for(2),
    }));
  });
}

#[test]
fn reward_account_and_governance_domain_follow_distinct_runtime_configured_surface() {
  new_test_ext().execute_with(|| {
    assert_ne!(Staking::pool_account_for(2), Staking::reward_account_for(2));
    assert_eq!(Staking::reward_governance_domain(2), Some(2));
  });
}

#[test]
fn reward_coefficient_is_exported_through_runtime_configured_provider_surface() {
  new_test_ext().execute_with(|| {
    assert_eq!(
      Staking::reward_coefficient(2, &1),
      Some(FixedU128::from_rational(3u128, 10u128))
    );
    assert_eq!(
      Staking::reward_coefficient(2, &3),
      Some(FixedU128::from_rational(5u128, 10u128))
    );
  });
}

#[test]
fn note_reward_inflow_tracks_epoch_accrual_and_liability_after_prefunding_reward_account() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    let reward_account = Staking::reward_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &reward_account,
      40,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_ok!(Staking::note_reward_inflow(2, 40));
    assert_eq!(Staking::reward_epoch_accrued(2, 1), 40);
    assert_eq!(Staking::reward_liability_balance(2), 40);
    assert_eq!(Staking::reward_epoch_total_weight(2, 1), 0);
    System::assert_last_event(RuntimeEvent::Staking(Event::RewardInflowRecorded {
      asset_id: 2,
      reward_account,
      epoch: 1,
      amount: 40,
    }));
  });
}

#[test]
fn reward_weight_snapshot_uses_one_epoch_lag_after_stake() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_eq!(Staking::reward_active_weight(2, &1), None);
    advance_to_block(2);
    assert_eq!(Staking::reward_active_total_weight(2), 30);
    assert_eq!(
      Staking::reward_active_weight_snapshot(2, &1),
      Some(crate::RewardWeightSnapshot {
        effective_from_epoch: 2,
        shares: 100,
        coefficient: FixedU128::from_rational(3u128, 10u128),
        weight: 30,
      })
    );
  });
}

#[test]
fn receipt_transfer_changes_reward_weight_only_next_epoch() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    advance_to_block(2);
    assert_eq!(Staking::reward_active_total_weight(2), 30);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      TYPE_STAKED_LOCAL,
      &1,
      &2,
      40,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    let reward_account = Staking::reward_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &reward_account,
      10,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_eq!(Staking::reward_active_weight(2, &1), Some(30));
    assert_eq!(Staking::reward_active_weight(2, &2), None);
    advance_to_block(3);
    assert_eq!(Staking::reward_epoch_accrued(2, 2), 10);
    assert_eq!(Staking::reward_epoch_total_weight(2, 2), 30);
    assert_eq!(Staking::reward_liability_balance(2), 10);
    assert_eq!(Staking::reward_active_total_weight(2), 34);
    assert_eq!(
      Staking::reward_active_weight_snapshot(2, &1),
      Some(crate::RewardWeightSnapshot {
        effective_from_epoch: 3,
        shares: 60,
        coefficient: FixedU128::from_rational(3u128, 10u128),
        weight: 18,
      })
    );
    assert_eq!(
      Staking::reward_active_weight_snapshot(2, &2),
      Some(crate::RewardWeightSnapshot {
        effective_from_epoch: 3,
        shares: 40,
        coefficient: FixedU128::from_rational(4u128, 10u128),
        weight: 16,
      })
    );
  });
}

#[test]
fn bootstrap_reward_snapshot_materializes_active_weight_for_live_holder() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_eq!(Staking::reward_active_weight(2, &1), None);
    assert_ok!(Staking::bootstrap_reward_snapshot(
      RuntimeOrigin::root(),
      2,
      1,
    ));
    assert_eq!(Staking::reward_active_total_weight(2), 30);
    System::assert_last_event(RuntimeEvent::Staking(Event::RewardSnapshotBootstrapped {
      asset_id: 2,
      account: 1,
      epoch: 1,
      weight: 30,
    }));
  });
}

#[test]
fn bootstrap_reward_snapshot_rejects_after_epoch_denominator_is_fixed() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(Staking::bootstrap_reward_snapshot(
      RuntimeOrigin::root(),
      2,
      1,
    ));
    let reward_account = Staking::reward_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &reward_account,
      10,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_ok!(Staking::note_reward_inflow(2, 10));
    assert_noop!(
      Staking::bootstrap_reward_snapshot(RuntimeOrigin::root(), 2, 1),
      Error::<Test>::RewardEpochWeightFrozen
    );
  });
}

#[test]
fn claim_reward_auto_compounds_same_asset_into_new_receipts() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(Staking::bootstrap_reward_snapshot(
      RuntimeOrigin::root(),
      2,
      1,
    ));
    let reward_account = Staking::reward_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &reward_account,
      40,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_ok!(Staking::note_reward_inflow(2, 40));
    advance_to_block(2);
    assert_ok!(Staking::claim_reward(RuntimeOrigin::signed(1), 2, 1));
    assert_eq!(Staking::reward_claimed((2, 1), 1), Some(40));
    assert_eq!(Staking::reward_liability_balance(2), 0);
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(2, &reward_account),
      0
    );
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(TYPE_STAKED_LOCAL, &1),
      140
    );
    assert_eq!(Staking::stake_value(2, &1), Some(140));
    System::assert_last_event(RuntimeEvent::Staking(Event::RewardClaimed {
      asset_id: 2,
      account: 1,
      epoch: 1,
      reward_amount: 40,
      minted_shares: 40,
    }));
  });
}

#[test]
fn claim_reward_rejects_open_or_already_claimed_epoch() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(Staking::bootstrap_reward_snapshot(
      RuntimeOrigin::root(),
      2,
      1,
    ));
    let reward_account = Staking::reward_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &reward_account,
      40,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_ok!(Staking::note_reward_inflow(2, 40));
    assert_noop!(
      Staking::claim_reward(RuntimeOrigin::signed(1), 2, 1),
      Error::<Test>::RewardEpochStillOpen
    );
    advance_to_block(2);
    assert_ok!(Staking::claim_reward(RuntimeOrigin::signed(1), 2, 1));
    assert_noop!(
      Staking::claim_reward(RuntimeOrigin::signed(1), 2, 1),
      Error::<Test>::RewardAlreadyClaimed
    );
  });
}

#[test]
fn claim_reward_batch_claims_multiple_closed_epochs() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    let epochs = polkadot_sdk::frame_support::BoundedVec::try_from(vec![1u64, 2u64])
      .expect("batch epochs must fit runtime bound");
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(Staking::bootstrap_reward_snapshot(
      RuntimeOrigin::root(),
      2,
      1,
    ));
    let reward_account = Staking::reward_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &reward_account,
      40,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_ok!(Staking::note_reward_inflow(2, 40));
    assert!(Staking::note_reward_touch(2, &1));
    advance_to_block(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &reward_account,
      60,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_ok!(Staking::note_reward_inflow(2, 60));
    advance_to_block(3);
    assert_ok!(Staking::claim_reward_batch(
      RuntimeOrigin::signed(1),
      2,
      epochs,
    ));
    assert_eq!(Staking::reward_claimed((2, 1), 1), Some(40));
    assert_eq!(Staking::reward_claimed((2, 2), 1), Some(60));
    assert_eq!(Staking::reward_liability_balance(2), 0);
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(2, &reward_account),
      0
    );
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(TYPE_STAKED_LOCAL, &1),
      200
    );
    assert_eq!(Staking::stake_value(2, &1), Some(200));
  });
}

#[test]
fn claim_reward_batch_rejects_duplicate_epochs() {
  new_test_ext().execute_with(|| {
    let epochs = polkadot_sdk::frame_support::BoundedVec::try_from(vec![1u64, 1u64])
      .expect("batch epochs must fit runtime bound");
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(Staking::bootstrap_reward_snapshot(
      RuntimeOrigin::root(),
      2,
      1,
    ));
    let reward_account = Staking::reward_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &reward_account,
      40,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_ok!(Staking::note_reward_inflow(2, 40));
    advance_to_block(2);
    assert_noop!(
      Staking::claim_reward_batch(RuntimeOrigin::signed(1), 2, epochs),
      Error::<Test>::DuplicateRewardEpoch
    );
  });
}

#[test]
fn claim_reward_rejects_truncated_epoch() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(Staking::bootstrap_reward_snapshot(
      RuntimeOrigin::root(),
      2,
      1,
    ));
    let reward_account = Staking::reward_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &reward_account,
      40,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_ok!(Staking::note_reward_inflow(2, 40));
    Staking::note_reward_ingress_truncated(1, 129, 128);
    advance_to_block(2);
    assert_noop!(
      Staking::claim_reward(RuntimeOrigin::signed(1), 2, 1),
      Error::<Test>::RewardEpochIncomplete
    );
  });
}

#[test]
fn reward_touch_overflow_marks_epoch_truncated_and_blocks_claims() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));

    let epoch = 1u64;
    let asset_id = 2u32;

    let mut accounts = polkadot_sdk::frame_support::BoundedVec::new();
    for i in 1u64..=256u64 {
      accounts.try_push(i).expect("must fit within 256");
    }
    crate::RewardEpochTouchedAccounts::<Test>::insert(epoch, asset_id, accounts);

    let result = Staking::note_reward_touch(asset_id, &257);
    assert!(!result);
    assert!(crate::RewardTruncatedEpochs::<Test>::contains_key(epoch));
    System::assert_has_event(RuntimeEvent::Staking(
      Event::RewardTouchedAccountsOverflow {
        epoch,
        asset_id,
        account: 257,
      },
    ));

    assert_ok!(Staking::bootstrap_reward_snapshot(
      RuntimeOrigin::root(),
      asset_id,
      1,
    ));
    let reward_account = Staking::reward_account_for(asset_id);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      asset_id,
      &1,
      &reward_account,
      40,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_ok!(Staking::note_reward_inflow(asset_id, 40));
    advance_to_block(2);
    assert_noop!(
      Staking::claim_reward(RuntimeOrigin::signed(1), asset_id, epoch),
      Error::<Test>::RewardEpochIncomplete
    );
  });
}

#[test]
fn register_asset_rejects_preexisting_staked_asset_id_collision() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      TYPE_STAKED_LOCAL,
      1,
      true,
      1
    ));
    assert_noop!(
      Staking::register_staking_asset(RuntimeOrigin::root(), 2),
      Error::<Test>::StakedAssetIdCollision
    );
  });
}

#[test]
fn foreign_asset_uses_dedicated_receipt_namespace() {
  const TYPE_FOREIGN: AssetId = 0xF000_0000;
  const TYPE_STAKED_FOREIGN: AssetId = 0x6000_0000;
  let foreign_asset = TYPE_FOREIGN | 2;
  let foreign_receipt = TYPE_STAKED_FOREIGN | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      foreign_asset,
      1,
      true,
      1
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(
      foreign_asset,
      &1,
      500
    ));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      foreign_asset,
    ));
    assert!(<Assets as Inspect<AccountId>>::asset_exists(
      foreign_receipt
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), foreign_asset, 200,));
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(foreign_receipt, &1),
      200
    );
    assert_eq!(Staking::stake_value(foreign_asset, &1), Some(200));
  });
}

#[test]
fn initialize_staked_asset_backfills_legacy_pool_receipt_class() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    let pool_account = Staking::pool_account_for(2);
    crate::Pools::<Test>::insert(
      2,
      crate::PoolState {
        total_shares: 100,
        accounted_balance: 100,
        active_staker_count: 1,
      },
    );
    crate::Positions::<Test>::insert(2, 1, crate::StakePosition { shares: 100 });
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(
      2,
      &pool_account,
      100
    ));
    assert!(!<Assets as Inspect<AccountId>>::asset_exists(
      TYPE_STAKED_LOCAL
    ));
    assert_ok!(Staking::initialize_staked_asset(RuntimeOrigin::root(), 2));
    assert!(<Assets as Inspect<AccountId>>::asset_exists(
      TYPE_STAKED_LOCAL
    ));
    assert_eq!(
      <Assets as MetadataInspect<AccountId>>::name(TYPE_STAKED_LOCAL),
      b"Staked Asset 2".to_vec()
    );
    System::assert_last_event(RuntimeEvent::Staking(Event::StakedAssetInitialized {
      asset_id: 2,
      staked_asset_id: TYPE_STAKED_LOCAL,
      pool_account,
    }));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(2), 2, 50));
    assert_eq!(Staking::position(2, 2), None);
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(TYPE_STAKED_LOCAL, &2),
      50
    );
    assert_eq!(Staking::stake_value(2, &1), Some(100));
    assert_eq!(Staking::stake_value(2, &2), Some(50));
  });
}

#[test]
fn initialize_staked_asset_rejects_existing_receipt_class() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    crate::Pools::<Test>::insert(
      2,
      crate::PoolState {
        total_shares: 0,
        accounted_balance: 0,
        active_staker_count: 0,
      },
    );
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      TYPE_STAKED_LOCAL,
      1,
      true,
      1
    ));
    assert_noop!(
      Staking::initialize_staked_asset(RuntimeOrigin::root(), 2),
      Error::<Test>::StakedAssetAlreadyInitialized
    );
  });
}

#[test]
fn convert_position_to_receipt_moves_legacy_shares_into_stxxx() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    let pool_account = Staking::pool_account_for(2);
    crate::Pools::<Test>::insert(
      2,
      crate::PoolState {
        total_shares: 100,
        accounted_balance: 100,
        active_staker_count: 1,
      },
    );
    crate::Positions::<Test>::insert(2, 1, crate::StakePosition { shares: 100 });
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(
      2,
      &pool_account,
      100
    ));
    assert_ok!(Staking::initialize_staked_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::convert_position_to_receipt(
      RuntimeOrigin::signed(1),
      2,
    ));
    assert_eq!(Staking::position(2, 1), None);
    assert_eq!(
      Staking::pool(2)
        .expect("pool must exist")
        .active_staker_count,
      0
    );
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(TYPE_STAKED_LOCAL, &1),
      100
    );
    assert_eq!(Staking::stake_value(2, &1), Some(100));
    System::assert_last_event(RuntimeEvent::Staking(Event::LegacyPositionConverted {
      asset_id: 2,
      account: 1,
      converted_shares: 100,
    }));
  });
}

#[test]
fn convert_position_to_receipt_requires_initialized_staked_asset() {
  new_test_ext().execute_with(|| {
    crate::Pools::<Test>::insert(
      2,
      crate::PoolState {
        total_shares: 100,
        accounted_balance: 100,
        active_staker_count: 1,
      },
    );
    crate::Positions::<Test>::insert(2, 1, crate::StakePosition { shares: 100 });
    assert_noop!(
      Staking::convert_position_to_receipt(RuntimeOrigin::signed(1), 2),
      Error::<Test>::StakedAssetNotInitialized
    );
  });
}

#[test]
fn first_stake_mints_equal_shares() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    let pool = Staking::pool(2).expect("pool must exist");
    assert!(Staking::position(2, 1).is_none());
    assert_eq!(pool.total_shares, 100);
    assert_eq!(pool.accounted_balance, 100);
    assert_eq!(pool.active_staker_count, 0);
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(TYPE_STAKED_LOCAL, &1),
      100
    );
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(2, &Staking::pool_account_for(2)),
      100
    );
  });
}

#[test]
fn second_stake_into_non_empty_pool_mints_proportional_shares() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(2), 2, 50));
    let pool = Staking::pool(2).expect("pool must exist");
    assert_eq!(pool.total_shares, 150);
    assert_eq!(pool.accounted_balance, 150);
    assert!(Staking::position(2, 1).is_none());
    assert!(Staking::position(2, 2).is_none());
    assert_eq!(Staking::stake_value(2, &1), Some(100));
    assert_eq!(Staking::stake_value(2, &2), Some(50));
  });
}

#[test]
fn external_inflow_increases_all_holders_proportionally_after_sync() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(2), 2, 100));
    let pool_account = Staking::pool_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &pool_account,
      100,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_ok!(Staking::sync_pool(RuntimeOrigin::signed(99), 2));
    let pool = Staking::pool(2).expect("pool must exist");
    assert_eq!(pool.total_shares, 200);
    assert_eq!(pool.accounted_balance, 300);
    assert_eq!(Staking::stake_value(2, &1), Some(150));
    assert_eq!(Staking::stake_value(2, &2), Some(150));
  });
}

#[test]
fn partial_unstake_burns_shares_and_returns_correct_underlying() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(2), 2, 100));
    let pool_account = Staking::pool_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &pool_account,
      100,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_ok!(Staking::sync_pool(RuntimeOrigin::signed(99), 2));
    let before = <Assets as Inspect<AccountId>>::balance(2, &1);
    assert_ok!(Staking::unstake(RuntimeOrigin::signed(1), 2, 50));
    let after = <Assets as Inspect<AccountId>>::balance(2, &1);
    assert_eq!(after - before, 75);
    let pool = Staking::pool(2).expect("pool must exist");
    assert_eq!(pool.total_shares, 150);
    assert_eq!(pool.accounted_balance, 225);
    assert!(Staking::position(2, 1).is_none());
  });
}

#[test]
fn transferred_receipt_holder_can_unstake_without_legacy_position() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      TYPE_STAKED_LOCAL,
      &1,
      &2,
      40,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_eq!(Staking::position(2, 2), None);
    let before = <Assets as Inspect<AccountId>>::balance(2, &2);
    assert_ok!(Staking::unstake(RuntimeOrigin::signed(2), 2, 40));
    let after = <Assets as Inspect<AccountId>>::balance(2, &2);
    assert_eq!(after - before, 40);
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(TYPE_STAKED_LOCAL, &2),
      0
    );
  });
}

#[test]
fn full_exit_removes_position_cleanly() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(Staking::unstake(RuntimeOrigin::signed(1), 2, 100));
    let pool = Staking::pool(2).expect("pool must exist");
    assert_eq!(pool.total_shares, 0);
    assert_eq!(pool.accounted_balance, 0);
    assert_eq!(pool.active_staker_count, 0);
    assert!(Staking::position(2, 1).is_none());
  });
}

#[test]
fn first_stake_rejects_unowned_prefunded_pool() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    let pool_account = Staking::pool_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &pool_account,
      100,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_noop!(
      Staking::stake(RuntimeOrigin::signed(1), 2, 100),
      Error::<Test>::PoolHasUnownedBalance
    );
  });
}

#[test]
fn recover_unowned_pool_transfers_balance_and_unblocks_first_stake() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    let pool_account = Staking::pool_account_for(2);
    let beneficiary_before = <Assets as Inspect<AccountId>>::balance(2, &99);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &pool_account,
      100,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_noop!(
      Staking::stake(RuntimeOrigin::signed(1), 2, 100),
      Error::<Test>::PoolHasUnownedBalance
    );
    assert_ok!(Staking::recover_unowned_pool(RuntimeOrigin::root(), 2, 99));
    let pool = Staking::pool(2).expect("pool must exist");
    assert_eq!(pool.total_shares, 0);
    assert_eq!(pool.accounted_balance, 0);
    assert_eq!(<Assets as Inspect<AccountId>>::balance(2, &pool_account), 0);
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(2, &99) - beneficiary_before,
      100
    );
    System::assert_has_event(RuntimeEvent::Staking(Event::UnownedPoolRecovered {
      asset_id: 2,
      beneficiary: 99,
      amount: 100,
    }));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert!(Staking::position(2, 1).is_none());
    assert_eq!(Staking::stake_value(2, &1), Some(100));
  });
}

#[test]
fn recover_unowned_pool_ignores_stale_legacy_position_count_after_receipt_exit() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      TYPE_STAKED_LOCAL,
      &1,
      &2,
      100,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Expendable,
    ));
    assert_ok!(Staking::unstake(RuntimeOrigin::signed(2), 2, 100));
    assert_eq!(Staking::pool(2).expect("pool must exist").total_shares, 0);
    assert!(Staking::position(2, 1).is_none());
    let pool_account = Staking::pool_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &pool_account,
      25,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    let beneficiary_before = <Assets as Inspect<AccountId>>::balance(2, &1);
    assert_ok!(Staking::recover_unowned_pool(RuntimeOrigin::root(), 2, 1));
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(2, &1) - beneficiary_before,
      25
    );
  });
}

#[test]
fn recover_unowned_pool_rejects_non_empty_pool() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_noop!(
      Staking::recover_unowned_pool(RuntimeOrigin::root(), 2, 99),
      Error::<Test>::PoolNotEmpty
    );
  });
}

#[test]
fn native_binding_tracks_operator_backing_from_native_stake() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(1), 100, 99));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(2), 50, 99));
    assert_eq!(Staking::native_binding(1), Some(99));
    assert_eq!(Staking::native_binding(2), Some(99));
    assert_eq!(Staking::delegated_native_backing(&99), 150);
    System::assert_has_event(RuntimeEvent::Staking(Event::NativeBindingSet {
      account: 2,
      operator: 99,
    }));
  });
}

#[test]
fn clearing_native_binding_removes_operator_backing() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(1), 100, 99));
    assert_eq!(Staking::delegated_native_backing(&99), 100);
    assert_ok!(Staking::clear_native_binding(RuntimeOrigin::signed(1)));
    assert_eq!(Staking::native_binding(1), None);
    assert_eq!(Staking::delegated_native_backing(&99), 0);
    System::assert_has_event(RuntimeEvent::Staking(Event::NativeBindingCleared {
      account: 1,
    }));
  });
}

#[test]
fn delegated_native_backing_follows_live_receipt_balance_after_transfer() {
  const TYPE_STAKED: AssetId = 0x5000_0000;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(1), 100, 99));
    assert_eq!(Staking::delegated_native_backing(&99), 100);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      TYPE_STAKED,
      &1,
      &2,
      30,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_eq!(Staking::delegated_native_backing(&99), 70);
  });
}

#[test]
fn stake_native_stakes_and_binds_in_one_step() {
  const TYPE_STAKED: AssetId = 0x5000_0000;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(1), 100, 99));
    assert_eq!(Staking::native_binding(1), Some(99));
    assert_eq!(Staking::delegated_native_backing(&99), 100);
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(TYPE_STAKED, &1),
      100
    );
  });
}

#[test]
fn generic_native_stake_requires_operator() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_noop!(
      Staking::stake(RuntimeOrigin::signed(1), 1, 100),
      Error::<Test>::NativeStakeRequiresOperator
    );
  });
}

#[test]
fn native_binding_rejects_self_binding() {
  new_test_ext().execute_with(|| {
    assert_noop!(
      Staking::bind_native(RuntimeOrigin::signed(1), 1),
      Error::<Test>::CannotBindToSelf
    );
  });
}

#[test]
fn native_binding_rejects_invalid_operator_target() {
  new_test_ext().execute_with(|| {
    assert_noop!(
      Staking::bind_native(RuntimeOrigin::signed(1), 2),
      Error::<Test>::InvalidBindingTarget
    );
  });
}

#[test]
fn operator_commission_can_be_set_within_maximum() {
  use polkadot_sdk::sp_runtime::Perbill;
  new_test_ext().execute_with(|| {
    assert_eq!(Staking::operator_commission(1), Perbill::zero());
    assert_ok!(Staking::set_operator_commission(
      RuntimeOrigin::signed(1),
      Perbill::from_percent(25),
    ));
    assert_eq!(Staking::operator_commission(1), Perbill::from_percent(25));
    System::assert_last_event(RuntimeEvent::Staking(Event::OperatorCommissionSet {
      operator: 1,
      commission: Perbill::from_percent(25),
    }));
  });
}

#[test]
fn operator_commission_rejects_above_maximum() {
  use polkadot_sdk::sp_runtime::Perbill;
  new_test_ext().execute_with(|| {
    assert_noop!(
      Staking::set_operator_commission(RuntimeOrigin::signed(1), Perbill::from_percent(51),),
      Error::<Test>::CommissionExceedsMaximum
    );
    assert_eq!(Staking::operator_commission(1), Perbill::zero());
  });
}

#[test]
fn operator_commission_can_be_updated() {
  use polkadot_sdk::sp_runtime::Perbill;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::set_operator_commission(
      RuntimeOrigin::signed(1),
      Perbill::from_percent(10),
    ));
    assert_ok!(Staking::set_operator_commission(
      RuntimeOrigin::signed(1),
      Perbill::from_percent(50),
    ));
    assert_eq!(Staking::operator_commission(1), Perbill::from_percent(50));
  });
}

#[test]
fn native_stake_helpers_distinguish_passive_and_delegated_positions() {
  const TYPE_STAKED: AssetId = 0x5000_0000;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(1), 100, 99));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(2), 50, 99));
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      TYPE_STAKED,
      &2,
      &3,
      20,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_eq!(Staking::native_stake_value(&1), Some(100));
    assert_eq!(Staking::passive_native_stake_value(&1), None);
    assert_eq!(Staking::delegated_native_stake_value(&1), Some((99, 100)));
    assert_eq!(
      Staking::stake_exposure(1, &1),
      Some(crate::StakeExposure {
        total_value: 100,
        passive_value: 0,
        delegated_value: 100,
        delegated_operator: Some(99),
      })
    );
    assert_eq!(Staking::native_stake_value(&2), Some(30));
    assert_eq!(Staking::passive_native_stake_value(&2), None);
    assert_eq!(Staking::delegated_native_stake_value(&2), Some((99, 30)));
    assert_eq!(Staking::native_stake_value(&3), Some(20));
    assert_eq!(Staking::passive_native_stake_value(&3), Some(20));
    assert_eq!(Staking::delegated_native_stake_value(&3), None);
  });
}

#[test]
fn non_native_stake_exposure_stays_passive_even_when_native_position_is_delegated() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(1), 100, 99));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 250));
    assert_eq!(Staking::passive_stake_value(2, &1), Some(250));
    assert_eq!(Staking::delegated_stake_value(2, &1), None);
    assert_eq!(
      Staking::stake_exposure(2, &1),
      Some(crate::StakeExposure {
        total_value: 250,
        passive_value: 250,
        delegated_value: 0,
        delegated_operator: None,
      })
    );
  });
}
