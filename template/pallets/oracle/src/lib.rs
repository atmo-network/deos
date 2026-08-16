//! Bounded typed scalar observation oracle.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;
pub use weights::WeightInfo;

pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use polkadot_sdk::frame_support::{pallet_prelude::DispatchResult, weights::Weight};
use scale_info::TypeInfo;

pub type OracleValue = u128;
pub type Revision = u64;

#[derive(
  Clone, Copy, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, Debug, TypeInfo,
)]
pub enum Aggregation {
  LastValue,
  Ema { half_life_blocks: u32 },
}

#[derive(
  Clone, Copy, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, Debug, TypeInfo,
)]
pub enum ZeroPolicy {
  Allow,
  Reject,
}

#[derive(
  Clone, Copy, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, Debug, TypeInfo,
)]
pub enum FeedLifecycle {
  Active,
  Paused,
  Deactivated,
}

#[derive(
  Clone, Decode, DecodeWithMemTracking, Encode, Debug, Eq, MaxEncodedLen, PartialEq, TypeInfo,
)]
pub struct FeedConfig<ProducerId, Meaning, Provenance> {
  pub producer: ProducerId,
  pub meaning: Meaning,
  pub provenance: Provenance,
  pub scale: u8,
  pub aggregation: Aggregation,
  pub zero_policy: ZeroPolicy,
  pub lifecycle: FeedLifecycle,
}

#[derive(
  Clone, Copy, Decode, DecodeWithMemTracking, Encode, Eq, MaxEncodedLen, PartialEq, Debug, TypeInfo,
)]
pub struct Observation<BlockNumber> {
  pub value: OracleValue,
  pub updated_at: BlockNumber,
  pub revision: Revision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, TypeInfo)]
pub enum ObservationState<BlockNumber> {
  Unavailable,
  Uninitialized,
  Fresh(Observation<BlockNumber>),
  Stale(Observation<BlockNumber>),
}

pub trait OnObservationChanged<FeedId> {
  fn on_observation_changed(feed: FeedId, revision: Revision) -> DispatchResult;
  fn weight() -> Weight;
}

impl<FeedId> OnObservationChanged<FeedId> for () {
  fn on_observation_changed(_: FeedId, _: Revision) -> DispatchResult {
    Ok(())
  }

  fn weight() -> Weight {
    Weight::zero()
  }
}

pub trait ObservationSink<ProducerId, FeedId> {
  fn publish(producer: ProducerId, feed: FeedId, sample: OracleValue) -> DispatchResult;
}

#[frame::pallet]
pub mod pallet {
  use super::*;
  use frame::prelude::*;
  use polkadot_sdk::{
    frame_support::{
      ensure,
      traits::{EnsureOrigin, Get},
      transactional,
    },
    sp_arithmetic::Perbill,
    sp_runtime::traits::{SaturatedConversion, Zero},
  };

  pub type FeedConfigOf<T> =
    FeedConfig<<T as Config>::ProducerId, <T as Config>::Meaning, <T as Config>::Provenance>;
  pub type ObservationOf<T> = Observation<BlockNumberFor<T>>;
  pub type ObservationStateOf<T> = ObservationState<BlockNumberFor<T>>;

  #[pallet::config]
  pub trait Config: frame_system::Config {
    type FeedId: Parameter + Member + MaxEncodedLen + Copy + Ord;
    type ProducerId: Parameter + Member + MaxEncodedLen + Clone + Ord;
    type Meaning: Parameter + Member + MaxEncodedLen + Clone;
    type Provenance: Parameter + Member + MaxEncodedLen + Clone;
    type RegisterOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    type PublishOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::ProducerId>;
    type OnObservationChanged: super::OnObservationChanged<Self::FeedId>;
    #[pallet::constant]
    type MaxFeeds: Get<u32>;
    #[pallet::constant]
    type MaxFeedsPerProducer: Get<u32>;
    #[pallet::constant]
    type MaxScale: Get<u8>;
    type WeightInfo: super::WeightInfo;
  }

  #[pallet::pallet]
  #[pallet::storage_version(STORAGE_VERSION)]
  pub struct Pallet<T>(_);

  const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

  #[pallet::storage]
  #[pallet::getter(fn feed_ids)]
  pub type FeedIds<T: Config> = StorageValue<_, BoundedVec<T::FeedId, T::MaxFeeds>, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn feeds)]
  pub type Feeds<T: Config> =
    StorageMap<_, Blake2_128Concat, T::FeedId, FeedConfigOf<T>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn producer_ids)]
  pub type ProducerIds<T: Config> =
    StorageValue<_, BoundedVec<T::ProducerId, T::MaxFeeds>, ValueQuery>;

  #[pallet::storage]
  #[pallet::getter(fn producer_feeds)]
  pub type ProducerFeeds<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::ProducerId,
    BoundedVec<T::FeedId, T::MaxFeedsPerProducer>,
    ValueQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn observations)]
  pub type Observations<T: Config> =
    StorageMap<_, Blake2_128Concat, T::FeedId, ObservationOf<T>, OptionQuery>;

  #[pallet::hooks]
  impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    fn integrity_test() {
      assert!(T::MaxFeeds::get() > 0, "MaxFeeds must be nonzero");
      assert!(
        T::MaxFeedsPerProducer::get() > 0 && T::MaxFeedsPerProducer::get() <= T::MaxFeeds::get(),
        "MaxFeedsPerProducer must be nonzero and no greater than MaxFeeds"
      );
    }

    #[cfg(feature = "try-runtime")]
    fn try_state(_n: BlockNumberFor<T>) -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      Self::do_try_state()
    }
  }

  #[pallet::event]
  #[pallet::generate_deposit(pub(super) fn deposit_event)]
  pub enum Event<T: Config> {
    FeedRegistered {
      feed: T::FeedId,
      producer: T::ProducerId,
    },
    FeedPaused {
      feed: T::FeedId,
    },
    FeedResumed {
      feed: T::FeedId,
    },
    FeedDeactivated {
      feed: T::FeedId,
    },
    ObservationPublished {
      feed: T::FeedId,
      value: OracleValue,
      revision: Revision,
    },
    ObservationRefreshed {
      feed: T::FeedId,
      value: OracleValue,
      revision: Revision,
    },
  }

  #[pallet::error]
  pub enum Error<T> {
    FeedAlreadyExists,
    FeedNotFound,
    FeedCapacityReached,
    ProducerCapacityReached,
    InvalidScale,
    InvalidHalfLife,
    UnauthorizedProducer,
    FeedPaused,
    FeedDeactivated,
    InvalidLifecycleTransition,
    ZeroRejected,
    ArithmeticOverflow,
    RevisionOverflow,
    InvalidMaximumAge,
  }

  #[pallet::call]
  impl<T: Config> Pallet<T> {
    #[pallet::call_index(0)]
    #[pallet::weight(
      T::WeightInfo::register_feed_existing_producer()
        .max(T::WeightInfo::register_feed_new_producer())
    )]
    #[transactional]
    pub fn register_feed(
      origin: OriginFor<T>,
      feed: T::FeedId,
      producer: T::ProducerId,
      meaning: T::Meaning,
      provenance: T::Provenance,
      scale: u8,
      aggregation: Aggregation,
      zero_policy: ZeroPolicy,
      start_paused: bool,
    ) -> DispatchResult {
      T::RegisterOrigin::ensure_origin(origin)?;
      ensure!(
        !Feeds::<T>::contains_key(feed),
        Error::<T>::FeedAlreadyExists
      );
      ensure!(
        (FeedIds::<T>::decode_len().unwrap_or(0) as u32) < T::MaxFeeds::get(),
        Error::<T>::FeedCapacityReached
      );
      ensure!(scale <= T::MaxScale::get(), Error::<T>::InvalidScale);
      if let Aggregation::Ema { half_life_blocks } = aggregation {
        ensure!(half_life_blocks > 0, Error::<T>::InvalidHalfLife);
      }
      let first_for_producer = !ProducerFeeds::<T>::contains_key(&producer);
      if first_for_producer {
        ensure!(
          (ProducerIds::<T>::decode_len().unwrap_or(0) as u32) < T::MaxFeeds::get(),
          Error::<T>::FeedCapacityReached
        );
      }
      ProducerFeeds::<T>::try_mutate(&producer, |feeds| {
        feeds
          .try_push(feed)
          .map_err(|_| Error::<T>::ProducerCapacityReached)
      })?;
      FeedIds::<T>::try_mutate(|feeds| {
        feeds
          .try_push(feed)
          .map_err(|_| Error::<T>::FeedCapacityReached)
      })?;
      if first_for_producer {
        ProducerIds::<T>::try_mutate(|producers| {
          producers
            .try_push(producer.clone())
            .map_err(|_| Error::<T>::FeedCapacityReached)
        })?;
      }
      let lifecycle = if start_paused {
        FeedLifecycle::Paused
      } else {
        FeedLifecycle::Active
      };
      Feeds::<T>::insert(
        feed,
        FeedConfig {
          producer: producer.clone(),
          meaning,
          provenance,
          scale,
          aggregation,
          zero_policy,
          lifecycle,
        },
      );
      Self::deposit_event(Event::FeedRegistered { feed, producer });
      Ok(())
    }

    #[pallet::call_index(1)]
    #[pallet::weight(T::WeightInfo::pause_feed())]
    pub fn pause_feed(origin: OriginFor<T>, feed: T::FeedId) -> DispatchResult {
      T::RegisterOrigin::ensure_origin(origin)?;
      Self::set_lifecycle(feed, FeedLifecycle::Active, FeedLifecycle::Paused)?;
      Self::deposit_event(Event::FeedPaused { feed });
      Ok(())
    }

    #[pallet::call_index(2)]
    #[pallet::weight(T::WeightInfo::resume_feed())]
    pub fn resume_feed(origin: OriginFor<T>, feed: T::FeedId) -> DispatchResult {
      T::RegisterOrigin::ensure_origin(origin)?;
      Self::set_lifecycle(feed, FeedLifecycle::Paused, FeedLifecycle::Active)?;
      Self::deposit_event(Event::FeedResumed { feed });
      Ok(())
    }

    #[pallet::call_index(3)]
    #[pallet::weight(T::WeightInfo::deactivate_feed())]
    pub fn deactivate_feed(origin: OriginFor<T>, feed: T::FeedId) -> DispatchResult {
      T::RegisterOrigin::ensure_origin(origin)?;
      Feeds::<T>::try_mutate(feed, |maybe| {
        let config = maybe.as_mut().ok_or(Error::<T>::FeedNotFound)?;
        ensure!(
          config.lifecycle != FeedLifecycle::Deactivated,
          Error::<T>::InvalidLifecycleTransition
        );
        config.lifecycle = FeedLifecycle::Deactivated;
        Ok::<_, DispatchError>(())
      })?;
      Self::deposit_event(Event::FeedDeactivated { feed });
      Ok(())
    }

    #[pallet::call_index(4)]
    #[pallet::weight(
      T::WeightInfo::publish_ema_changed()
        .max(T::WeightInfo::publish_ema_refresh())
        .max(T::WeightInfo::publish_last_value())
        .saturating_add(T::OnObservationChanged::weight())
    )]
    pub fn publish(origin: OriginFor<T>, feed: T::FeedId, sample: OracleValue) -> DispatchResult {
      let producer = T::PublishOrigin::ensure_origin(origin)?;
      Self::publish_from(producer, feed, sample)
    }
  }

  impl<T: Config> Pallet<T> {
    fn set_lifecycle(
      feed: T::FeedId,
      expected: FeedLifecycle,
      next: FeedLifecycle,
    ) -> DispatchResult {
      Feeds::<T>::try_mutate(feed, |maybe| {
        let config = maybe.as_mut().ok_or(Error::<T>::FeedNotFound)?;
        ensure!(
          config.lifecycle == expected,
          Error::<T>::InvalidLifecycleTransition
        );
        config.lifecycle = next;
        Ok::<_, DispatchError>(())
      })
    }

    #[transactional]
    pub fn publish_from(
      producer: T::ProducerId,
      feed: T::FeedId,
      sample: OracleValue,
    ) -> DispatchResult {
      let config = Feeds::<T>::get(feed).ok_or(Error::<T>::FeedNotFound)?;
      ensure!(
        config.producer == producer,
        Error::<T>::UnauthorizedProducer
      );
      match config.lifecycle {
        FeedLifecycle::Active => {}
        FeedLifecycle::Paused => return Err(Error::<T>::FeedPaused.into()),
        FeedLifecycle::Deactivated => return Err(Error::<T>::FeedDeactivated.into()),
      }
      if config.zero_policy == ZeroPolicy::Reject {
        ensure!(sample > 0, Error::<T>::ZeroRejected);
      }
      let now = frame_system::Pallet::<T>::block_number();
      let previous = Observations::<T>::get(feed);
      let value = match (config.aggregation, previous) {
        (_, None) | (Aggregation::LastValue, Some(_)) => sample,
        (Aggregation::Ema { half_life_blocks }, Some(observation)) => {
          let elapsed: u128 = now
            .saturating_sub(observation.updated_at)
            .saturated_into::<u128>()
            .max(1);
          let denominator = u128::from(half_life_blocks)
            .checked_add(elapsed)
            .ok_or(Error::<T>::ArithmeticOverflow)?;
          let alpha = Perbill::from_rational(elapsed, denominator);
          alpha
            .mul_floor(sample)
            .checked_add((Perbill::one() - alpha).mul_floor(observation.value))
            .ok_or(Error::<T>::ArithmeticOverflow)?
        }
      };
      let (revision, changed) = match previous {
        None => (1, true),
        Some(observation) if observation.value != value => (
          observation
            .revision
            .checked_add(1)
            .ok_or(Error::<T>::RevisionOverflow)?,
          true,
        ),
        Some(observation) => (observation.revision, false),
      };
      if changed {
        T::OnObservationChanged::on_observation_changed(feed, revision)?;
      }
      Observations::<T>::insert(
        feed,
        Observation {
          value,
          updated_at: now,
          revision,
        },
      );
      if changed {
        Self::deposit_event(Event::ObservationPublished {
          feed,
          value,
          revision,
        });
      } else {
        Self::deposit_event(Event::ObservationRefreshed {
          feed,
          value,
          revision,
        });
      }
      Ok(())
    }

    pub fn observation_state(
      feed: T::FeedId,
      max_age_blocks: BlockNumberFor<T>,
    ) -> Result<ObservationStateOf<T>, DispatchError> {
      ensure!(!max_age_blocks.is_zero(), Error::<T>::InvalidMaximumAge);
      let Some(config) = Feeds::<T>::get(feed) else {
        return Ok(ObservationState::Unavailable);
      };
      if config.lifecycle == FeedLifecycle::Deactivated {
        return Ok(ObservationState::Unavailable);
      }
      let Some(observation) = Observations::<T>::get(feed) else {
        return Ok(ObservationState::Uninitialized);
      };
      let age = frame_system::Pallet::<T>::block_number().saturating_sub(observation.updated_at);
      if age <= max_age_blocks {
        Ok(ObservationState::Fresh(observation))
      } else {
        Ok(ObservationState::Stale(observation))
      }
    }
  }

  #[cfg(feature = "try-runtime")]
  impl<T: Config> Pallet<T> {
    pub(crate) fn do_try_state() -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      use alloc::collections::BTreeSet;
      use polkadot_sdk::sp_runtime::TryRuntimeError;

      let feed_ids = FeedIds::<T>::get();
      let feed_count = feed_ids.len() as u32;
      let mut seen_feeds = BTreeSet::new();
      for feed in feed_ids.into_iter() {
        if !seen_feeds.insert(feed) {
          return Err(TryRuntimeError::Other("FeedIds contains a duplicate"));
        }
        let config = Feeds::<T>::get(feed)
          .ok_or(TryRuntimeError::Other("FeedIds references an absent feed"))?;
        if !ProducerFeeds::<T>::get(&config.producer).contains(&feed) {
          return Err(TryRuntimeError::Other(
            "Feed is absent from its producer reverse index",
          ));
        }
        if let Some(observation) = Observations::<T>::get(feed) {
          if observation.revision == 0 {
            return Err(TryRuntimeError::Other(
              "Initialized observation has revision zero",
            ));
          }
        }
      }

      let mut seen_producers = BTreeSet::new();
      let mut indexed_feeds = 0u32;
      for producer in ProducerIds::<T>::get().into_iter() {
        if !seen_producers.insert(producer.clone()) {
          return Err(TryRuntimeError::Other("ProducerIds contains a duplicate"));
        }
        let feeds = ProducerFeeds::<T>::get(&producer);
        if feeds.is_empty() {
          return Err(TryRuntimeError::Other("Producer index is empty"));
        }
        indexed_feeds = indexed_feeds.saturating_add(feeds.len() as u32);
        for feed in feeds.into_iter() {
          let config = Feeds::<T>::get(feed).ok_or(TryRuntimeError::Other(
            "Producer index references an absent feed",
          ))?;
          if config.producer != producer {
            return Err(TryRuntimeError::Other("Producer reverse index mismatch"));
          }
        }
      }
      if indexed_feeds != feed_count {
        return Err(TryRuntimeError::Other(
          "Producer reverse-index cardinality mismatch",
        ));
      }
      Ok(())
    }
  }

  impl<T: Config> ObservationSink<T::ProducerId, T::FeedId> for Pallet<T> {
    fn publish(producer: T::ProducerId, feed: T::FeedId, sample: OracleValue) -> DispatchResult {
      Self::publish_from(producer, feed, sample)
    }
  }
}
