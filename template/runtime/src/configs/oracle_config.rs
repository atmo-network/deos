use crate::{AccountId, Oracle, Runtime, RuntimeOrigin};
use pallet_deos_actors::{
  ObservationTransition, ObservationTransitionIngress, TriggerCauseProvenance,
};
use pallet_oracle::{Aggregation, FeedConfig, FeedLifecycle, ZeroPolicy};
use polkadot_sdk::{
  frame_support::{ensure, parameter_types, transactional},
  frame_system::{EnsureRoot, EnsureSigned},
  sp_runtime::{DispatchError, DispatchResult, traits::AccountIdConversion},
};
use primitives::{
  AssetKind, LocalPoolObservationMethod, OracleAggregationId, OracleFeedId, OracleMeaning,
  OracleProvenance, ecosystem,
};

use super::deos_router_config::{DeosRouterEmaHalfLife, RouterPalletId};

pub const DEOS_ROUTER_ORACLE_SCALE: u8 = 12;
pub const DEOS_ROUTER_MAX_ORACLE_POOL_PAIRS: u32 = 500;

/// Closed runtime inventory of publishers certified to create Actors observation ingress.
pub const ACTORS_OBSERVATION_PUBLISHER_INVENTORY: &[&str] = &["DEOS Oracle::OnObservationChanged"];

pub const fn deos_router_pool_feed(asset_in: AssetKind, asset_out: AssetKind) -> OracleFeedId {
  OracleFeedId::directional_local_pool_price(
    asset_in,
    asset_out,
    LocalPoolObservationMethod::PreExecutionSpot,
    OracleAggregationId::Ema {
      half_life_blocks: ecosystem::params::EMA_HALF_LIFE_BLOCKS,
    },
    DEOS_ROUTER_ORACLE_SCALE,
  )
}

fn expected_deos_router_feed(
  feed: OracleFeedId,
) -> FeedConfig<AccountId, OracleMeaning, OracleProvenance> {
  FeedConfig {
    producer: RouterPalletId::get().into_account_truncating(),
    meaning: feed.meaning(),
    provenance: OracleProvenance::DeosRouterPreExecutionReserves,
    scale: DEOS_ROUTER_ORACLE_SCALE,
    aggregation: Aggregation::Ema {
      half_life_blocks: DeosRouterEmaHalfLife::get(),
    },
    zero_policy: ZeroPolicy::Reject,
    lifecycle: FeedLifecycle::Active,
  }
}

pub(crate) fn preflight_deos_router_pool_feeds(
  asset_a: AssetKind,
  asset_b: AssetKind,
) -> DispatchResult {
  ensure!(
    asset_a.is_valid_market_pair(asset_b),
    DispatchError::Other("Invalid oracle pool assets")
  );
  let forward = deos_router_pool_feed(asset_a, asset_b);
  let reverse = forward.reverse();
  let producer: AccountId = RouterPalletId::get().into_account_truncating();
  let current = pallet_oracle::ProducerFeeds::<Runtime>::get(&producer).len() as u32;
  let forward_existing = pallet_oracle::Feeds::<Runtime>::get(forward);
  let reverse_existing = pallet_oracle::Feeds::<Runtime>::get(reverse);
  let missing =
    u32::from(forward_existing.is_none()).saturating_add(u32::from(reverse_existing.is_none()));
  ensure!(
    current.saturating_add(missing) <= DEOS_ROUTER_MAX_ORACLE_POOL_PAIRS.saturating_mul(2),
    DispatchError::Other("DEOS Router pool feed capacity reached")
  );
  if let Some(existing) = forward_existing {
    ensure!(
      existing == expected_deos_router_feed(forward),
      DispatchError::Other("Oracle feed identity collision")
    );
  }
  if let Some(existing) = reverse_existing {
    ensure!(
      existing == expected_deos_router_feed(reverse),
      DispatchError::Other("Oracle feed identity collision")
    );
  }
  Ok(())
}

#[transactional]
pub(crate) fn ensure_deos_router_pool_feeds(
  asset_a: AssetKind,
  asset_b: AssetKind,
) -> DispatchResult {
  preflight_deos_router_pool_feeds(asset_a, asset_b)?;
  let forward = deos_router_pool_feed(asset_a, asset_b);
  ensure_deos_router_feed(forward)?;
  ensure_deos_router_feed(forward.reverse())
}

fn ensure_deos_router_feed(feed: OracleFeedId) -> DispatchResult {
  let expected = expected_deos_router_feed(feed);
  let producer = expected.producer.clone();
  let aggregation = expected.aggregation;
  if let Some(existing) = pallet_oracle::Feeds::<Runtime>::get(feed) {
    ensure!(
      existing == expected,
      DispatchError::Other("Oracle feed identity collision")
    );
    return Ok(());
  }
  Oracle::register_feed(
    RuntimeOrigin::root(),
    feed,
    producer,
    feed.meaning(),
    OracleProvenance::DeosRouterPreExecutionReserves,
    DEOS_ROUTER_ORACLE_SCALE,
    aggregation,
    ZeroPolicy::Reject,
    false,
  )
}

#[cfg(feature = "runtime-benchmarks")]
pub struct OraclePublicationBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_oracle::PublicationBenchmarkHelper<OracleFeedId> for OraclePublicationBenchmarkHelper {
  fn prepare_changed_hook(
    feed: OracleFeedId,
    topology: pallet_oracle::ChangedHookBenchmarkTopology,
  ) -> DispatchResult {
    if matches!(
      topology,
      pallet_oracle::ChangedHookBenchmarkTopology::PrimaryFirst
        | pallet_oracle::ChangedHookBenchmarkTopology::PrimaryExisting
        | pallet_oracle::ChangedHookBenchmarkTopology::Combined
    ) {
      pallet_deos_actors::ObservationSubscriberCount::<Runtime>::insert(feed, 1);
    }
    if matches!(
      topology,
      pallet_oracle::ChangedHookBenchmarkTopology::SecondaryFirst
        | pallet_oracle::ChangedHookBenchmarkTopology::SecondaryExisting
        | pallet_oracle::ChangedHookBenchmarkTopology::Combined
    ) {
      pallet_deos_actors::CrossingFeedMembershipCount::<Runtime>::insert(feed, 1);
    }
    if topology == pallet_oracle::ChangedHookBenchmarkTopology::PrimaryExisting {
      <crate::Actors as ObservationTransitionIngress<OracleFeedId>>::note_observation_transition(
        feed,
        ObservationTransition {
          revision: 1,
          previous: None,
          current: 1_000_000_000,
        },
        TriggerCauseProvenance::Deferred,
      )?;
    }
    if topology == pallet_oracle::ChangedHookBenchmarkTopology::SecondaryExisting {
      <crate::Actors as ObservationTransitionIngress<OracleFeedId>>::note_observation_transition(
        feed,
        ObservationTransition {
          revision: 1,
          previous: None,
          current: 1_000_000_000,
        },
        TriggerCauseProvenance::Deferred,
      )?;
    }
    Ok(())
  }

  fn prepare_secondary_capacity_edge(
    feed: OracleFeedId,
  ) -> Result<(pallet_oracle::Revision, pallet_oracle::OracleValue, bool), DispatchError> {
    pallet_deos_actors::CrossingFeedMembershipCount::<Runtime>::insert(feed, 1);
    let mut current = 1_000_000_000u128;
    <crate::Actors as ObservationTransitionIngress<OracleFeedId>>::note_observation_transition(
      feed,
      ObservationTransition {
        revision: 1,
        previous: None,
        current,
      },
      TriggerCauseProvenance::Deferred,
    )?;
    let capacity = <<Runtime as pallet_deos_actors::Config>::MaxCrossingTransitionsPerFeed as polkadot_sdk::frame_support::traits::Get<u32>>::get();
    for offset in 0..capacity {
      let previous = current;
      current = current.checked_add(1).ok_or(DispatchError::Arithmetic(
        polkadot_sdk::sp_runtime::ArithmeticError::Overflow,
      ))?;
      <crate::Actors as ObservationTransitionIngress<OracleFeedId>>::note_observation_transition(
        feed,
        ObservationTransition {
          revision: u64::from(offset).saturating_add(2),
          previous: Some(previous),
          current,
        },
        TriggerCauseProvenance::Deferred,
      )?;
    }
    Ok((u64::from(capacity).saturating_add(1), current, true))
  }
}

pub struct ActorObservationChangeIngress;

impl ActorObservationChangeIngress {
  pub const fn certified_publisher_inventory() -> &'static [&'static str] {
    ACTORS_OBSERVATION_PUBLISHER_INVENTORY
  }
}

impl pallet_oracle::OnObservationChanged<OracleFeedId> for ActorObservationChangeIngress {
  fn on_observation_changed(
    feed: OracleFeedId,
    revision: pallet_oracle::Revision,
    previous: Option<pallet_oracle::OracleValue>,
    current: pallet_oracle::OracleValue,
    cause_provenance: pallet_oracle::ObservationCauseProvenance,
  ) -> DispatchResult {
    <crate::Actors as ObservationTransitionIngress<OracleFeedId>>::note_observation_transition(
      feed,
      ObservationTransition {
        revision,
        previous,
        current,
      },
      match cause_provenance {
        pallet_oracle::ObservationCauseProvenance::ExternalPhase => {
          TriggerCauseProvenance::ExternalPhase
        }
        pallet_oracle::ObservationCauseProvenance::Deferred => TriggerCauseProvenance::Deferred,
      },
    )
  }
}

parameter_types! {
  pub const OracleMaxFeeds: u32 = 1_024;
  pub const OracleMaxFeedsPerProducer: u32 = 1_001;
  pub const OracleMaxScale: u8 = 18;
}

impl pallet_oracle::Config for Runtime {
  type FeedId = OracleFeedId;
  type ProducerId = AccountId;
  type Meaning = OracleMeaning;
  type Provenance = OracleProvenance;
  type RegisterOrigin = EnsureRoot<AccountId>;
  type PublishOrigin = EnsureSigned<AccountId>;
  type OnObservationChanged = ActorObservationChangeIngress;
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = OraclePublicationBenchmarkHelper;
  type MaxFeeds = OracleMaxFeeds;
  type MaxFeedsPerProducer = OracleMaxFeedsPerProducer;
  type MaxScale = OracleMaxScale;
  type WeightInfo = crate::weights::pallet_oracle::SubstrateWeight<Runtime>;
}
