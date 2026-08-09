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

use super::axial_router_config::{AxialRouterEmaHalfLife, AxialRouterPalletId};

pub const AXIAL_ROUTER_ORACLE_SCALE: u8 = 12;
pub const AXIAL_ROUTER_MAX_ORACLE_POOL_PAIRS: u32 = 500;

/// Closed runtime inventory of publishers certified to create Actors observation ingress.
pub const ACTORS_OBSERVATION_PUBLISHER_INVENTORY: &[&str] = &["DEOS Oracle::OnObservationChanged"];

pub const fn axial_router_pool_feed(asset_in: AssetKind, asset_out: AssetKind) -> OracleFeedId {
  OracleFeedId::directional_local_pool_price(
    asset_in,
    asset_out,
    LocalPoolObservationMethod::PreExecutionSpot,
    OracleAggregationId::Ema {
      half_life_blocks: ecosystem::params::EMA_HALF_LIFE_BLOCKS,
    },
    AXIAL_ROUTER_ORACLE_SCALE,
  )
}

#[transactional]
pub(crate) fn ensure_axial_router_pool_feeds(
  asset_a: AssetKind,
  asset_b: AssetKind,
) -> DispatchResult {
  ensure!(
    asset_a != asset_b,
    DispatchError::Other("Identical oracle feed assets")
  );
  let forward = axial_router_pool_feed(asset_a, asset_b);
  let reverse = forward.reverse();
  let producer: AccountId = AxialRouterPalletId::get().into_account_truncating();
  let current = pallet_oracle::ProducerFeeds::<Runtime>::get(&producer).len() as u32;
  let missing = u32::from(!pallet_oracle::Feeds::<Runtime>::contains_key(forward)).saturating_add(
    u32::from(!pallet_oracle::Feeds::<Runtime>::contains_key(reverse)),
  );
  ensure!(
    current.saturating_add(missing) <= AXIAL_ROUTER_MAX_ORACLE_POOL_PAIRS.saturating_mul(2),
    DispatchError::Other("DEOS Router pool feed capacity reached")
  );
  ensure_axial_router_feed(forward)?;
  ensure_axial_router_feed(reverse)
}

fn ensure_axial_router_feed(feed: OracleFeedId) -> DispatchResult {
  let producer: AccountId = AxialRouterPalletId::get().into_account_truncating();
  let aggregation = Aggregation::Ema {
    half_life_blocks: AxialRouterEmaHalfLife::get(),
  };
  let expected = FeedConfig {
    producer: producer.clone(),
    meaning: feed.meaning(),
    provenance: OracleProvenance::AxialRouterPreExecutionReserves,
    scale: AXIAL_ROUTER_ORACLE_SCALE,
    aggregation,
    zero_policy: ZeroPolicy::Reject,
    lifecycle: FeedLifecycle::Active,
  };
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
    OracleProvenance::AxialRouterPreExecutionReserves,
    AXIAL_ROUTER_ORACLE_SCALE,
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
