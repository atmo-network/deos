use crate::{
  Aggregation, Error, FeedLifecycle, Observation, ObservationState, OracleValue, ZeroPolicy,
  mock::{Oracle, RuntimeOrigin, System, Test, hook_calls, new_test_ext, set_hook_failure},
};
use codec::{Decode, Encode};
use polkadot_sdk::frame_support::{assert_noop, assert_ok, traits::StorageInfoTrait};

fn register(feed: u32, producer: u64, aggregation: Aggregation) {
  assert_ok!(Oracle::register_feed(
    RuntimeOrigin::root(),
    feed,
    producer,
    7,
    1,
    12,
    aggregation,
    ZeroPolicy::Reject,
    false,
  ));
}

#[test]
fn registration_pins_typed_semantics_and_bounds_cardinality() {
  new_test_ext().execute_with(|| {
    register(10, 1, Aggregation::LastValue);
    let config = Oracle::feeds(10).expect("feed exists");
    assert_eq!(config.producer, 1);
    assert_eq!(config.meaning, 7);
    assert_eq!(config.provenance, 1);
    assert_eq!(config.scale, 12);
    assert_eq!(config.lifecycle, FeedLifecycle::Active);
    assert_eq!(Oracle::feed_count(), 1);
    assert_eq!(Oracle::producer_feeds(1).as_slice(), &[10]);
    assert_noop!(
      Oracle::register_feed(
        RuntimeOrigin::root(),
        10,
        1,
        8,
        2,
        6,
        Aggregation::LastValue,
        ZeroPolicy::Allow,
        false,
      ),
      Error::<Test>::FeedAlreadyExists
    );
    register(11, 1, Aggregation::LastValue);
    assert_noop!(
      Oracle::register_feed(
        RuntimeOrigin::root(),
        12,
        1,
        7,
        1,
        12,
        Aggregation::LastValue,
        ZeroPolicy::Reject,
        false,
      ),
      Error::<Test>::ProducerCapacityReached
    );
  });
}

#[test]
fn global_feed_capacity_is_independent_from_producer_capacity() {
  new_test_ext().execute_with(|| {
    register(1, 1, Aggregation::LastValue);
    register(2, 1, Aggregation::LastValue);
    register(3, 2, Aggregation::LastValue);
    assert_noop!(
      Oracle::register_feed(
        RuntimeOrigin::root(),
        4,
        2,
        7,
        1,
        12,
        Aggregation::LastValue,
        ZeroPolicy::Reject,
        false,
      ),
      Error::<Test>::FeedCapacityReached
    );
    assert_eq!(Oracle::feed_ids().as_slice(), &[1, 2, 3]);
    assert_eq!(Oracle::producer_ids().as_slice(), &[1, 2]);
  });
}

#[test]
fn invalid_feed_configuration_fails_before_mutation() {
  new_test_ext().execute_with(|| {
    assert_noop!(
      Oracle::register_feed(
        RuntimeOrigin::root(),
        1,
        1,
        7,
        1,
        19,
        Aggregation::LastValue,
        ZeroPolicy::Reject,
        false,
      ),
      Error::<Test>::InvalidScale
    );
    assert_noop!(
      Oracle::register_feed(
        RuntimeOrigin::root(),
        1,
        1,
        7,
        1,
        12,
        Aggregation::Ema {
          half_life_blocks: 0
        },
        ZeroPolicy::Reject,
        false,
      ),
      Error::<Test>::InvalidHalfLife
    );
    assert_eq!(Oracle::feed_count(), 0);
    assert!(Oracle::producer_feeds(1).is_empty());
  });
}

#[test]
fn last_value_revision_and_freshness_are_explicit() {
  new_test_ext().execute_with(|| {
    register(1, 1, Aggregation::LastValue);
    assert_eq!(
      Oracle::observation_state(1, 1).expect("valid age"),
      ObservationState::Uninitialized
    );
    assert_noop!(
      Oracle::publish(RuntimeOrigin::signed(2), 1, 10),
      Error::<Test>::UnauthorizedProducer
    );
    assert_noop!(
      Oracle::publish(RuntimeOrigin::signed(1), 1, 0),
      Error::<Test>::ZeroRejected
    );
    assert_ok!(Oracle::publish(RuntimeOrigin::signed(1), 1, 10));
    let first = Oracle::observations(1).expect("initialized");
    assert_eq!((first.value, first.revision, first.updated_at), (10, 1, 1));
    System::set_block_number(2);
    assert_ok!(Oracle::publish(RuntimeOrigin::signed(1), 1, 10));
    let refresh = Oracle::observations(1).expect("refreshed");
    assert_eq!(
      (refresh.value, refresh.revision, refresh.updated_at),
      (10, 1, 2)
    );
    assert_ok!(Oracle::publish(RuntimeOrigin::signed(1), 1, 11));
    assert_eq!(Oracle::observations(1).expect("changed").revision, 2);
    System::set_block_number(3);
    assert!(matches!(
      Oracle::observation_state(1, 1).expect("boundary age"),
      ObservationState::Fresh(_)
    ));
    System::set_block_number(4);
    assert!(matches!(
      Oracle::observation_state(1, 1).expect("stale age"),
      ObservationState::Stale(_)
    ));
  });
}

#[test]
fn lifecycle_controls_publication_without_reinterpreting_state() {
  new_test_ext().execute_with(|| {
    assert_noop!(
      Oracle::publish(RuntimeOrigin::signed(1), 99, 5),
      Error::<Test>::FeedNotFound
    );
    assert_noop!(
      Oracle::pause_feed(RuntimeOrigin::root(), 99),
      Error::<Test>::FeedNotFound
    );
    register(1, 1, Aggregation::LastValue);
    assert_ok!(Oracle::publish(RuntimeOrigin::signed(1), 1, 5));
    assert_ok!(Oracle::pause_feed(RuntimeOrigin::root(), 1));
    assert_noop!(
      Oracle::publish(RuntimeOrigin::signed(1), 1, 6),
      Error::<Test>::FeedPaused
    );
    assert!(matches!(
      Oracle::observation_state(1, 1).expect("paused state remains readable"),
      ObservationState::Fresh(_)
    ));
    assert_ok!(Oracle::resume_feed(RuntimeOrigin::root(), 1));
    assert_ok!(Oracle::deactivate_feed(RuntimeOrigin::root(), 1));
    assert_eq!(
      Oracle::observation_state(1, 1).expect("deactivated is unavailable"),
      ObservationState::Unavailable
    );
    assert_noop!(
      Oracle::resume_feed(RuntimeOrigin::root(), 1),
      Error::<Test>::InvalidLifecycleTransition
    );
  });
}

#[test]
fn ema_uses_elapsed_weighting_and_direct_initialization() {
  new_test_ext().execute_with(|| {
    register(
      1,
      1,
      Aggregation::Ema {
        half_life_blocks: 100,
      },
    );
    assert_ok!(Oracle::publish(RuntimeOrigin::signed(1), 1, 1_000));
    System::set_block_number(101);
    assert_ok!(Oracle::publish(RuntimeOrigin::signed(1), 1, 2_000));
    let observation = Oracle::observations(1).expect("EMA exists");
    assert_eq!(observation.value, 1_500);
    assert_eq!(observation.revision, 2);
  });
}

#[test]
fn changed_hook_is_transactional_and_equal_refresh_is_hook_free() {
  new_test_ext().execute_with(|| {
    register(1, 1, Aggregation::LastValue);
    let events_before = System::event_count();
    set_hook_failure(true);
    assert_eq!(
      Oracle::publish(RuntimeOrigin::signed(1), 1, 10),
      Err(polkadot_sdk::sp_runtime::DispatchError::Other(
        "ObservationHookRejected"
      ))
    );
    assert!(Oracle::observations(1).is_none());
    assert_eq!(System::event_count(), events_before);
    assert!(hook_calls().is_empty());

    set_hook_failure(false);
    assert_ok!(Oracle::publish(RuntimeOrigin::signed(1), 1, 10));
    assert_eq!(hook_calls(), vec![(1, 1)]);
    System::set_block_number(2);
    assert_ok!(Oracle::publish(RuntimeOrigin::signed(1), 1, 10));
    assert_eq!(hook_calls(), vec![(1, 1)]);
  });
}

#[test]
fn revision_overflow_fails_without_refresh_or_hook() {
  new_test_ext().execute_with(|| {
    register(1, 1, Aggregation::LastValue);
    crate::Observations::<Test>::insert(
      1,
      Observation {
        value: 10,
        updated_at: 1,
        revision: u64::MAX,
      },
    );
    assert_noop!(
      Oracle::publish(RuntimeOrigin::signed(1), 1, 11),
      Error::<Test>::RevisionOverflow
    );
    assert_eq!(
      Oracle::observations(1).expect("original observation remains"),
      Observation {
        value: 10,
        updated_at: 1,
        revision: u64::MAX,
      }
    );
    assert!(hook_calls().is_empty());
  });
}

#[test]
fn scale_and_storage_contract_are_explicit() {
  assert_eq!(Aggregation::LastValue.encode(), vec![0]);
  assert_eq!(
    Aggregation::Ema {
      half_life_blocks: 0x0102_0304,
    }
    .encode(),
    vec![1, 4, 3, 2, 1]
  );
  assert_eq!(ZeroPolicy::Allow.encode(), vec![0]);
  assert_eq!(ZeroPolicy::Reject.encode(), vec![1]);
  assert_eq!(FeedLifecycle::Active.encode(), vec![0]);
  assert_eq!(FeedLifecycle::Paused.encode(), vec![1]);
  assert_eq!(FeedLifecycle::Deactivated.encode(), vec![2]);
  assert_eq!(
    Aggregation::decode(&mut &Aggregation::LastValue.encode()[..]).expect("aggregation decodes"),
    Aggregation::LastValue
  );

  let storage = Oracle::storage_info();
  let names = storage
    .into_iter()
    .map(|entry| String::from_utf8(entry.storage_name).expect("storage name is UTF-8"))
    .collect::<Vec<_>>();
  assert_eq!(
    names,
    vec![
      "FeedCount".to_owned(),
      "FeedIds".to_owned(),
      "Feeds".to_owned(),
      "ProducerIds".to_owned(),
      "ProducerFeeds".to_owned(),
      "Observations".to_owned(),
    ]
  );
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_reconciles_bounded_forward_and_reverse_indexes() {
  new_test_ext().execute_with(|| {
    register(1, 1, Aggregation::LastValue);
    assert_ok!(Oracle::do_try_state());
    crate::FeedCount::<Test>::put(2);
    assert!(Oracle::do_try_state().is_err());
  });
}

#[test]
fn ema_matches_router_elapsed_and_rounding_vectors() {
  new_test_ext().execute_with(|| {
    register(
      1,
      1,
      Aggregation::Ema {
        half_life_blocks: 100,
      },
    );
    for (elapsed, expected) in [
      (1u64, 1_009_900_990u128),
      (10, 1_090_909_090),
      (100, 1_500_000_000),
    ] {
      crate::Observations::<Test>::insert(
        1,
        Observation {
          value: 1_000_000_000,
          updated_at: 1,
          revision: 7,
        },
      );
      System::set_block_number(1 + elapsed);
      assert_ok!(Oracle::publish(RuntimeOrigin::signed(1), 1, 2_000_000_000,));
      let observation = Oracle::observations(1).expect("EMA vector publishes");
      assert_eq!((observation.value, observation.revision), (expected, 8));
    }

    crate::Observations::<Test>::insert(
      1,
      Observation {
        value: 1_000_000_000,
        updated_at: 0,
        revision: 9,
      },
    );
    System::set_block_number(1);
    assert_ok!(Oracle::publish(RuntimeOrigin::signed(1), 1, 1_000_000_000,));
    let equal = Oracle::observations(1).expect("equal EMA refreshes");
    assert_eq!(
      (equal.value, equal.revision, equal.updated_at),
      (1_000_000_000, 9, 1)
    );
  });
}

#[test]
fn ema_extreme_values_remain_bounded_without_saturation() {
  new_test_ext().execute_with(|| {
    register(
      1,
      1,
      Aggregation::Ema {
        half_life_blocks: u32::MAX,
      },
    );
    assert_ok!(Oracle::publish(RuntimeOrigin::signed(1), 1, u128::MAX));
    System::set_block_number(u64::MAX);
    assert_ok!(Oracle::publish(RuntimeOrigin::signed(1), 1, u128::MAX));
    let observation = Oracle::observations(1).expect("EMA remains initialized");
    assert_eq!(observation.value, u128::MAX - 1);
    assert_eq!(observation.revision, 2);
  });
}

#[test]
fn zero_can_be_an_initialized_value_when_the_feed_allows_it() {
  new_test_ext().execute_with(|| {
    assert_ok!(Oracle::register_feed(
      RuntimeOrigin::root(),
      1,
      1,
      7,
      1,
      12,
      Aggregation::LastValue,
      ZeroPolicy::Allow,
      false,
    ));
    let zero: OracleValue = 0;
    assert_ok!(Oracle::publish(RuntimeOrigin::signed(1), 1, zero));
    let observation = Oracle::observations(1).expect("zero initializes storage");
    assert_eq!((observation.value, observation.revision), (0, 1));
  });
}
