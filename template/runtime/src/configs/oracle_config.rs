use crate::{AccountId, Oracle, Runtime, RuntimeOrigin};
use pallet_deos_actors::ObservationChangeIngress;
use pallet_oracle::{Aggregation, FeedConfig, FeedLifecycle, ZeroPolicy};
use polkadot_sdk::{
  frame_support::{ensure, parameter_types, transactional, weights::Weight},
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
    asset_a != asset_b,
    DispatchError::Other("Identical oracle feed assets")
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
  ) -> DispatchResult {
    <crate::Actors as ObservationChangeIngress<OracleFeedId>>::note_observation_changed(
      feed, revision,
    )
  }

  fn weight() -> Weight {
    crate::Actors::observation_change_ingress_weight()
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
  type MaxFeeds = OracleMaxFeeds;
  type MaxFeedsPerProducer = OracleMaxFeedsPerProducer;
  type MaxScale = OracleMaxScale;
  type WeightInfo = crate::weights::pallet_oracle::SubstrateWeight<Runtime>;
}
