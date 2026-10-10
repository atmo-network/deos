use super::common::{
  ALICE, BOB, add_liquidity, burn_actor_account, create_pool, create_test_asset,
  deos_router_account, mint_tokens, new_test_ext,
};
use crate::{Assets, Balances, DeosRouter, Oracle, Runtime, RuntimeCall, RuntimeOrigin, System};
use alloc::boxed::Box;
use codec::Encode;
use pallet_oracle::{Aggregation, ObservationState, WeightInfo as _, ZeroPolicy};
use polkadot_sdk::frame_support::{
  assert_noop, assert_ok,
  dispatch::GetDispatchInfo,
  traits::{Currency, fungibles::Inspect as FungiblesInspect},
  weights::Weight,
};
use primitives::{AssetKind, OracleAggregationId, OracleFeedId, OracleMeaning, OracleProvenance};

fn directional_feed(asset_in: AssetKind, asset_out: AssetKind) -> OracleFeedId {
  crate::configs::oracle_config::deos_router_pool_feed(asset_in, asset_out)
}

#[test]
fn equal_refresh_and_rejected_producers_preserve_reactive_state() {
  new_test_ext().execute_with(|| {
    let producer = deos_router_account();
    let feed = directional_feed(AssetKind::Native, AssetKind::Local(7));
    assert_ok!(Oracle::register_feed(
      RuntimeOrigin::root(),
      feed,
      producer.clone(),
      feed.meaning(),
      OracleProvenance::DeosRouterPreExecutionReserves,
      feed.scale,
      Aggregation::LastValue,
      ZeroPolicy::Reject,
      false,
    ));
    let events_before_rejection = System::events();
    assert_noop!(
      Oracle::publish(RuntimeOrigin::signed(ALICE), feed, 1_000),
      pallet_oracle::Error::<Runtime>::UnauthorizedProducer
    );
    assert_eq!(Oracle::observations(feed), None);
    assert_eq!(System::events(), events_before_rejection);

    assert_ok!(Oracle::publish(
      RuntimeOrigin::signed(producer.clone()),
      feed,
      1_000,
    ));
    let first = Oracle::observations(feed).expect("first observation");
    let events_after_first = System::events();
    System::set_block_number(2);
    assert_ok!(Oracle::publish(
      RuntimeOrigin::signed(producer.clone()),
      feed,
      1_000,
    ));
    let refreshed = Oracle::observations(feed).expect("refreshed observation");
    assert_eq!(refreshed.value, first.value);
    assert_eq!(refreshed.revision, first.revision);
    assert_eq!(refreshed.updated_at, 2);
    assert!(System::events().len() > events_after_first.len());

    assert_ok!(Oracle::pause_feed(RuntimeOrigin::root(), feed));
    let paused_observation = Oracle::observations(feed);
    let events_before_paused = System::events();
    assert_noop!(
      Oracle::publish(RuntimeOrigin::signed(producer), feed, 2_000),
      pallet_oracle::Error::<Runtime>::FeedPaused
    );
    assert_eq!(Oracle::observations(feed), paused_observation);
    assert_eq!(System::events(), events_before_paused);
  });
}

#[test]
fn runtime_admits_and_publishes_typed_directional_feed() {
  new_test_ext().execute_with(|| {
    let producer = deos_router_account();
    let feed = directional_feed(AssetKind::Native, AssetKind::Local(7));
    assert_ok!(Oracle::register_feed(
      RuntimeOrigin::root(),
      feed,
      producer.clone(),
      OracleMeaning::DirectionalLocalPoolPrice {
        asset_in: feed.asset_in,
        asset_out: feed.asset_out,
        method: feed.method,
      },
      OracleProvenance::DeosRouterPreExecutionReserves,
      feed.scale,
      Aggregation::Ema {
        half_life_blocks: 100,
      },
      ZeroPolicy::Reject,
      false,
    ));
    assert_ok!(Oracle::publish(
      RuntimeOrigin::signed(producer),
      feed,
      1_000_000_000_000,
    ));
    assert!(matches!(
      Oracle::observation_state(feed, 1).expect("maximum age is valid"),
      ObservationState::Fresh(observation)
        if observation.value == 1_000_000_000_000 && observation.revision == 1
    ));
  });
}

#[test]
fn directional_feed_identity_does_not_alias_reverse_direction() {
  let forward = directional_feed(AssetKind::Native, AssetKind::Local(7));
  let reverse = forward.reverse();
  assert_eq!(
    reverse,
    directional_feed(AssetKind::Local(7), AssetKind::Native)
  );
  assert_ne!(forward, reverse);
  assert_ne!(forward.encode(), reverse.encode());
  assert_ne!(
    forward,
    OracleFeedId {
      aggregation: OracleAggregationId::LastValue,
      ..forward
    }
  );
  assert_ne!(
    forward,
    OracleFeedId {
      scale: 11,
      ..forward
    }
  );
}

#[test]
fn pool_registration_admits_both_directional_feeds_once() {
  new_test_ext().execute_with(|| {
    let asset_a = AssetKind::Native;
    let asset_b = AssetKind::Local(7);
    assert_ok!(create_test_asset(7, &ALICE));
    assert_ok!(create_pool(RuntimeOrigin::signed(ALICE), asset_a, asset_b,));

    let producer = deos_router_account();
    let forward = directional_feed(asset_a, asset_b);
    let reverse = forward.reverse();
    let forward_config = pallet_oracle::Feeds::<Runtime>::get(forward)
      .expect("pool registration admits the forward feed");
    let reverse_config = pallet_oracle::Feeds::<Runtime>::get(reverse)
      .expect("pool registration admits the reverse feed");
    assert_eq!(forward_config.meaning, forward.meaning());
    assert_eq!(reverse_config.meaning, reverse.meaning());
    assert_eq!(forward_config.producer, producer);
    assert_eq!(reverse_config.producer, producer);
    assert_eq!(pallet_oracle::FeedIds::<Runtime>::decode_len(), Some(2));

    assert_ok!(crate::configs::assets_config::register_pool_lp_pair(
      asset_a, asset_b,
    ));
    assert_eq!(pallet_oracle::FeedIds::<Runtime>::decode_len(), Some(2));

    assert_ok!(Oracle::publish(
      RuntimeOrigin::signed(producer.clone()),
      forward,
      2_000_000_000_000,
    ));
    assert_ok!(Oracle::publish(
      RuntimeOrigin::signed(producer),
      reverse,
      500_000_000_000,
    ));
    assert!(matches!(
      Oracle::observation_state(forward, 1).expect("maximum age is valid"),
      ObservationState::Fresh(observation) if observation.value == 2_000_000_000_000
    ));
    assert!(matches!(
      Oracle::observation_state(reverse, 1).expect("maximum age is valid"),
      ObservationState::Fresh(observation) if observation.value == 500_000_000_000
    ));
  });
}

#[test]
fn oracle_publish_declares_the_subscriber_independent_actor_hook_weight() {
  let feed = directional_feed(AssetKind::Native, AssetKind::Local(7));
  let call = RuntimeCall::Oracle(pallet_oracle::Call::publish {
    feed,
    sample: 1_000_000_000_000,
  });
  let oracle_branch =
    crate::weights::pallet_oracle::SubstrateWeight::<Runtime>::publish_ema_changed()
      .max(
        crate::weights::pallet_oracle::SubstrateWeight::<Runtime>::publish_ema_changed_primary_first(),
      )
      .max(
        crate::weights::pallet_oracle::SubstrateWeight::<Runtime>::publish_ema_changed_primary_existing(),
      )
      .max(
        crate::weights::pallet_oracle::SubstrateWeight::<Runtime>::publish_ema_changed_secondary_first(),
      )
      .max(
        crate::weights::pallet_oracle::SubstrateWeight::<Runtime>::publish_ema_changed_secondary_existing(),
      )
      .max(
        crate::weights::pallet_oracle::SubstrateWeight::<Runtime>::publish_ema_changed_combined(),
      )
      .max(
        crate::weights::pallet_oracle::SubstrateWeight::<Runtime>::publish_ema_changed_secondary_capacity(),
      )
      .max(crate::weights::pallet_oracle::SubstrateWeight::<Runtime>::publish_ema_refresh())
      .max(crate::weights::pallet_oracle::SubstrateWeight::<Runtime>::publish_last_value());
  assert_eq!(call.get_dispatch_info().call_weight, oracle_branch);
  assert!(oracle_branch.ref_time() > 0);
  assert!(oracle_branch.proof_size() > 0);
}

#[test]
fn pool_feed_cardinality_is_explicitly_bounded() {
  let maximum = crate::configs::oracle_config::DEOS_ROUTER_MAX_ORACLE_POOL_PAIRS;
  assert_eq!(
    10u128.pow(u32::from(
      crate::configs::oracle_config::DEOS_ROUTER_ORACLE_SCALE
    )),
    crate::configs::deos_router_config::DeosRouterPrecision::get()
  );
  assert!(
    maximum.saturating_mul(2) <= crate::configs::oracle_config::OracleMaxFeedsPerProducer::get()
  );
  assert!(
    maximum.saturating_add(1).saturating_mul(2)
      > crate::configs::oracle_config::OracleMaxFeedsPerProducer::get()
  );
}

#[test]
fn pair_registration_rejects_before_partial_mutation_when_capacity_is_full() {
  new_test_ext().execute_with(|| {
    let producer = deos_router_account();
    for index in 0..1_000 {
      let feed = directional_feed(AssetKind::Local(10_000 + index), AssetKind::Native);
      assert_ok!(Oracle::register_feed(
        RuntimeOrigin::root(),
        feed,
        producer.clone(),
        feed.meaning(),
        OracleProvenance::DeosRouterPreExecutionReserves,
        feed.scale,
        Aggregation::Ema {
          half_life_blocks: 100,
        },
        ZeroPolicy::Reject,
        false,
      ));
    }
    assert_ok!(create_test_asset(7, &ALICE));
    let asset_a = AssetKind::Native;
    let asset_b = AssetKind::Local(7);
    assert_ok!(crate::AssetConversion::create_pool(
      RuntimeOrigin::signed(ALICE),
      Box::new(asset_a),
      Box::new(asset_b),
    ));
    let pool = crate::pallet_asset_conversion::Pools::<Runtime>::get((asset_a, asset_b))
      .expect("pool exists before bounded index admission");
    let forward = directional_feed(asset_a, asset_b);
    let reverse = forward.reverse();

    assert_noop!(
      crate::configs::assets_config::register_pool_lp_pair(asset_a, asset_b),
      polkadot_sdk::sp_runtime::DispatchError::Other("DEOS Router pool feed capacity reached")
    );
    assert_eq!(pallet_oracle::FeedIds::<Runtime>::decode_len(), Some(1_000));
    assert!(!pallet_oracle::Feeds::<Runtime>::contains_key(forward));
    assert!(!pallet_oracle::Feeds::<Runtime>::contains_key(reverse));
    assert_eq!(crate::DeosRouter::lp_pair_by_token_id(pool.lp_token), None);
  });
}

#[test]
fn pair_registration_rolls_back_first_direction_when_reverse_identity_collides() {
  new_test_ext().execute_with(|| {
    let asset_a = AssetKind::Native;
    let asset_b = AssetKind::Local(7);
    let forward = directional_feed(asset_a, asset_b);
    let reverse = forward.reverse();
    assert_ok!(Oracle::register_feed(
      RuntimeOrigin::root(),
      reverse,
      BOB,
      reverse.meaning(),
      OracleProvenance::DeosRouterPreExecutionReserves,
      reverse.scale,
      Aggregation::Ema {
        half_life_blocks: 100,
      },
      ZeroPolicy::Reject,
      false,
    ));
    assert_ok!(create_test_asset(7, &ALICE));
    assert_ok!(crate::AssetConversion::create_pool(
      RuntimeOrigin::signed(ALICE),
      Box::new(asset_a),
      Box::new(asset_b),
    ));
    let pool = crate::pallet_asset_conversion::Pools::<Runtime>::get((asset_a, asset_b))
      .expect("pool exists before collision test");

    assert_noop!(
      crate::configs::assets_config::register_pool_lp_pair(asset_a, asset_b),
      polkadot_sdk::sp_runtime::DispatchError::Other("Oracle feed identity collision")
    );
    assert_eq!(pallet_oracle::FeedIds::<Runtime>::decode_len(), Some(1));
    assert!(!pallet_oracle::Feeds::<Runtime>::contains_key(forward));
    assert!(pallet_oracle::Feeds::<Runtime>::contains_key(reverse));
    assert_eq!(crate::DeosRouter::lp_pair_by_token_id(pool.lp_token), None);
  });
}

#[test]
fn router_producer_matches_deterministic_ema_vectors() {
  new_test_ext().execute_with(|| {
    let asset_in = AssetKind::Native;
    let asset_out = AssetKind::Local(7);
    let feed = directional_feed(asset_in, asset_out);
    assert_ok!(crate::configs::oracle_config::ensure_deos_router_pool_feeds(
      asset_in, asset_out,
    ));
    System::set_block_number(1);
    assert_ok!(
      <crate::configs::deos_router_config::PriceOracleImpl<Runtime> as pallet_deos_router::PriceOracle<crate::Balance>>::update_ema_price(
        asset_in,
        asset_out,
        1_000_000_000,
      )
    );
    for (block, expected) in [
      (2, 1_009_900_990),
      (12, 1_099_909_990),
      (112, 1_549_954_995),
    ] {
      System::set_block_number(block);
      assert_ok!(
        <crate::configs::deos_router_config::PriceOracleImpl<Runtime> as pallet_deos_router::PriceOracle<crate::Balance>>::update_ema_price(
          asset_in,
          asset_out,
          2_000_000_000,
        )
      );
      assert_eq!(Oracle::observations(feed).map(|value| value.value), Some(expected));
    }
  });
}

#[test]
fn failed_swap_rolls_back_oracle_fee_event_and_pool_effects() {
  new_test_ext().execute_with(|| {
    const OUTPUT_ASSET: u32 = 9_001;
    const OUTPUT_MINIMUM: u128 = 1_000_000_000_000_000;
    const LIQUIDITY: u128 = OUTPUT_MINIMUM * 10;
    let asset_in = AssetKind::Native;
    let asset_out = AssetKind::Local(OUTPUT_ASSET);
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      OUTPUT_ASSET,
      ALICE.into(),
      true,
      OUTPUT_MINIMUM,
    ));
    assert_ok!(mint_tokens(
      OUTPUT_ASSET,
      &ALICE,
      &ALICE,
      LIQUIDITY.saturating_mul(2),
    ));
    assert_ok!(create_pool(
      RuntimeOrigin::signed(ALICE),
      asset_in,
      asset_out,
    ));
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &ALICE,
      LIQUIDITY.saturating_mul(2),
    );
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(ALICE),
      asset_in,
      asset_out,
      LIQUIDITY,
      LIQUIDITY,
      1,
      OUTPUT_MINIMUM,
      &ALICE,
    ));
    System::set_block_number(1);
    let feed = directional_feed(asset_in, asset_out);
    let pool_before =
      crate::AssetConversion::get_reserves(asset_in, asset_out).expect("pool reserves exist");
    let alice_before = <Balances as Currency<crate::AccountId>>::free_balance(&ALICE);
    let burn_before = <Balances as Currency<crate::AccountId>>::free_balance(&burn_actor_account());
    let recipient_before =
      <Assets as FungiblesInspect<crate::AccountId>>::balance(OUTPUT_ASSET, &BOB);
    let events_before = System::events();

    assert!(
      DeosRouter::execute_swap_for(&ALICE, asset_in, asset_out, 1_000_000_000_000, 0, &BOB,)
        .is_err()
    );
    assert_eq!(Oracle::observations(feed), None);
    assert_eq!(
      <Balances as Currency<crate::AccountId>>::free_balance(&ALICE),
      alice_before
    );
    assert_eq!(
      <Balances as Currency<crate::AccountId>>::free_balance(&burn_actor_account()),
      burn_before
    );
    assert_eq!(
      <Assets as FungiblesInspect<crate::AccountId>>::balance(OUTPUT_ASSET, &BOB),
      recipient_before
    );
    assert_eq!(
      crate::AssetConversion::get_reserves(asset_in, asset_out).expect("pool remains readable"),
      pool_before
    );
    assert_eq!(System::events(), events_before);
  });
}

#[test]
fn runtime_binds_generated_oracle_weights_and_stable_pallet_index() {
  let feed = directional_feed(AssetKind::Native, AssetKind::Local(7));
  let call = RuntimeCall::Oracle(pallet_oracle::Call::<Runtime>::pause_feed { feed });
  assert_eq!(call.encode()[0], 52);
  assert_eq!(
    call.get_dispatch_info().call_weight,
    crate::weights::pallet_oracle::SubstrateWeight::<Runtime>::pause_feed()
  );
  assert_ne!(call.get_dispatch_info().call_weight, Weight::zero());
  assert_eq!(
    crate::weights::pallet_oracle::SubstrateWeight::<Runtime>::register_feed_new_producer()
      .proof_size(),
    45_174,
    "runtime weight must charge the accepted measured ProofSize above the generated estimate"
  );
}
