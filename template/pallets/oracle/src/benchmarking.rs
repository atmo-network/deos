use crate::*;
use polkadot_sdk::{
  frame_benchmarking::{account, v2::*},
  frame_support::traits::Get,
  frame_system::RawOrigin,
};

#[benchmarks(
  where
    T::FeedId: From<u32>,
    T::ProducerId: From<T::AccountId>,
    T::Meaning: Default,
    T::Provenance: Default,
)]
mod benches {
  use super::*;

  fn seed_feed<T: Config>(
    feed: T::FeedId,
    producer: T::ProducerId,
    aggregation: Aggregation,
    lifecycle: FeedLifecycle,
  ) where
    T::Meaning: Default,
    T::Provenance: Default,
  {
    let first_for_producer = !ProducerFeeds::<T>::contains_key(&producer);
    if first_for_producer {
      ProducerIds::<T>::mutate(|producers| {
        producers
          .try_push(producer.clone())
          .expect("benchmark producer capacity fits")
      });
    }
    ProducerFeeds::<T>::mutate(&producer, |feeds| {
      feeds.try_push(feed).expect("benchmark feed capacity fits")
    });
    FeedIds::<T>::mutate(|feeds| {
      feeds
        .try_push(feed)
        .expect("benchmark global capacity fits")
    });
    Feeds::<T>::insert(
      feed,
      FeedConfig {
        producer,
        meaning: T::Meaning::default(),
        provenance: T::Provenance::default(),
        scale: 0,
        aggregation,
        zero_policy: ZeroPolicy::Allow,
        lifecycle,
      },
    );
    FeedCount::<T>::mutate(|count| *count = count.saturating_add(1));
  }

  #[benchmark]
  fn register_feed_existing_producer() {
    let caller: T::AccountId = whitelisted_caller();
    let producer = T::ProducerId::from(caller);
    let producer_occupancy = T::MaxFeedsPerProducer::get()
      .saturating_sub(1)
      .min(T::MaxFeeds::get().saturating_sub(1));
    for index in 0..producer_occupancy {
      seed_feed::<T>(
        T::FeedId::from(index),
        producer.clone(),
        Aggregation::LastValue,
        FeedLifecycle::Active,
      );
    }
    let other_account: T::AccountId = account("other-producer", 0, 0);
    let other_producer = T::ProducerId::from(other_account);
    for index in producer_occupancy..T::MaxFeeds::get().saturating_sub(1) {
      seed_feed::<T>(
        T::FeedId::from(index),
        other_producer.clone(),
        Aggregation::LastValue,
        FeedLifecycle::Active,
      );
    }
    let feed = T::FeedId::from(T::MaxFeeds::get().saturating_sub(1));

    #[extrinsic_call]
    register_feed(
      RawOrigin::Root,
      feed,
      producer,
      T::Meaning::default(),
      T::Provenance::default(),
      0,
      Aggregation::LastValue,
      ZeroPolicy::Allow,
      false,
    );

    assert!(Feeds::<T>::contains_key(feed));
  }

  #[benchmark]
  fn register_feed_new_producer() {
    for index in 0..T::MaxFeeds::get().saturating_sub(1) {
      let account: T::AccountId = account("full-producer-index", index, 0);
      seed_feed::<T>(
        T::FeedId::from(index),
        T::ProducerId::from(account),
        Aggregation::LastValue,
        FeedLifecycle::Active,
      );
    }
    let caller: T::AccountId = whitelisted_caller();
    let producer = T::ProducerId::from(caller);
    let feed = T::FeedId::from(T::MaxFeeds::get().saturating_sub(1));

    #[extrinsic_call]
    register_feed(
      RawOrigin::Root,
      feed,
      producer,
      T::Meaning::default(),
      T::Provenance::default(),
      0,
      Aggregation::LastValue,
      ZeroPolicy::Allow,
      false,
    );

    assert!(Feeds::<T>::contains_key(feed));
  }

  #[benchmark]
  fn pause_feed() {
    let caller: T::AccountId = whitelisted_caller();
    let feed = T::FeedId::from(1);
    seed_feed::<T>(
      feed,
      T::ProducerId::from(caller),
      Aggregation::LastValue,
      FeedLifecycle::Active,
    );

    #[extrinsic_call]
    pause_feed(RawOrigin::Root, feed);

    assert_eq!(
      Feeds::<T>::get(feed).expect("feed exists").lifecycle,
      FeedLifecycle::Paused
    );
  }

  #[benchmark]
  fn resume_feed() {
    let caller: T::AccountId = whitelisted_caller();
    let feed = T::FeedId::from(1);
    seed_feed::<T>(
      feed,
      T::ProducerId::from(caller),
      Aggregation::LastValue,
      FeedLifecycle::Paused,
    );

    #[extrinsic_call]
    resume_feed(RawOrigin::Root, feed);

    assert_eq!(
      Feeds::<T>::get(feed).expect("feed exists").lifecycle,
      FeedLifecycle::Active
    );
  }

  #[benchmark]
  fn deactivate_feed() {
    let caller: T::AccountId = whitelisted_caller();
    let feed = T::FeedId::from(1);
    seed_feed::<T>(
      feed,
      T::ProducerId::from(caller),
      Aggregation::LastValue,
      FeedLifecycle::Active,
    );

    #[extrinsic_call]
    deactivate_feed(RawOrigin::Root, feed);

    assert_eq!(
      Feeds::<T>::get(feed).expect("feed exists").lifecycle,
      FeedLifecycle::Deactivated
    );
  }

  #[benchmark]
  fn publish_last_value() {
    let caller: T::AccountId = whitelisted_caller();
    let feed = T::FeedId::from(1);
    seed_feed::<T>(
      feed,
      T::ProducerId::from(caller.clone()),
      Aggregation::LastValue,
      FeedLifecycle::Active,
    );

    #[extrinsic_call]
    publish(RawOrigin::Signed(caller), feed, 1);

    assert_eq!(
      Observations::<T>::get(feed)
        .expect("observation exists")
        .revision,
      1
    );
  }

  #[benchmark]
  fn publish_ema_changed() {
    let caller: T::AccountId = whitelisted_caller();
    let feed = T::FeedId::from(1);
    seed_feed::<T>(
      feed,
      T::ProducerId::from(caller.clone()),
      Aggregation::Ema {
        half_life_blocks: 100,
      },
      FeedLifecycle::Active,
    );
    Observations::<T>::insert(
      feed,
      Observation {
        value: 1_000_000_000,
        updated_at: polkadot_sdk::frame_system::Pallet::<T>::block_number(),
        revision: 1,
      },
    );

    #[extrinsic_call]
    publish(RawOrigin::Signed(caller), feed, 2_000_000_000);

    assert_eq!(
      Observations::<T>::get(feed)
        .expect("observation exists")
        .revision,
      2
    );
  }

  #[benchmark]
  fn publish_ema_refresh() {
    let caller: T::AccountId = whitelisted_caller();
    let feed = T::FeedId::from(1);
    seed_feed::<T>(
      feed,
      T::ProducerId::from(caller.clone()),
      Aggregation::Ema {
        half_life_blocks: 100,
      },
      FeedLifecycle::Active,
    );
    Observations::<T>::insert(
      feed,
      Observation {
        value: 1_000_000_000,
        updated_at: polkadot_sdk::frame_system::Pallet::<T>::block_number(),
        revision: 1,
      },
    );

    #[extrinsic_call]
    publish(RawOrigin::Signed(caller), feed, 1_000_000_000);

    assert_eq!(
      Observations::<T>::get(feed)
        .expect("observation exists")
        .revision,
      1
    );
  }

  #[cfg(test)]
  use crate::mock::{Test, new_test_ext};
  #[cfg(test)]
  impl_benchmark_test_suite!(Pallet, new_test_ext(), Test);
}
