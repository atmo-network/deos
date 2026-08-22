//! Runtime Integrity Tests
//!
//! FRAME collects every pallet `#[pallet::integrity_test]` block into `AllPalletsWithSystem`.
//! Those assertions encode configuration invariants that hold only for one specific combination
//! of runtime constants and generated weights: derived bound equalities, non-zero page sizes, and
//! the requirement that one maximum automatic actor cleanup fits the guaranteed `on_idle` service
//! envelope.
//!
//! A parameter change or a weight regeneration can invalidate any of them, and a node only
//! discovers that at startup. Nothing else in this workspace executes them, so this module is the
//! gate that keeps those invariants inside ordinary local validation.

use super::common::new_test_ext;
use crate::AllPalletsWithSystem;
use polkadot_sdk::frame_support::traits::IntegrityTest;

/// Runs every pallet integrity assertion against the concrete DEOS runtime configuration.
#[test]
fn runtime_pallet_integrity_holds() {
  new_test_ext().execute_with(|| {
    AllPalletsWithSystem::integrity_test();
  });
}

#[test]
fn privileged_origin_inventory_rejects_signed_authority_escalation() {
  use crate::{Runtime, RuntimeOrigin};
  use polkadot_sdk::frame_support::traits::EnsureOrigin;

  macro_rules! assert_root_only {
    ($origin:ty, $name:literal) => {{
      assert!(
        <$origin as EnsureOrigin<RuntimeOrigin>>::try_origin(RuntimeOrigin::root()).is_ok(),
        "{} must accept Root",
        $name,
      );
      assert!(
        <$origin as EnsureOrigin<RuntimeOrigin>>::try_origin(RuntimeOrigin::signed(
          super::common::ALICE,
        ))
        .is_err(),
        "{} must reject signed authority",
        $name,
      );
    }};
  }

  assert_root_only!(
    <Runtime as pallet_deos_actors::Config>::SystemOrigin,
    "Actors System control"
  );
  assert_root_only!(
    <Runtime as pallet_deos_actors::Config>::GlobalBreakerOrigin,
    "Actors global breaker"
  );
  assert_root_only!(
    <Runtime as pallet_governance::Config>::AdminOrigin,
    "Governance administration"
  );
  assert_root_only!(
    <Runtime as pallet_deos_router::Config>::AdminOrigin,
    "DEOS Router administration"
  );
  assert_root_only!(
    <Runtime as pallet_staking::Config>::AdminOrigin,
    "DEOS Staking administration"
  );
  assert_root_only!(
    <Runtime as pallet_staking::Config>::SecurityRewardFundingOrigin,
    "security reward funding"
  );
  assert_root_only!(
    <Runtime as pallet_tmc::Config>::AdminOrigin,
    "TMC administration"
  );
  assert_root_only!(
    <Runtime as polkadot_sdk::pallet_assets::Config>::ForceOrigin,
    "pallet-assets force administration"
  );
  assert_root_only!(
    <Runtime as pallet_asset_registry::Config>::RegistryOrigin,
    "Asset Registry administration"
  );
  assert_root_only!(
    <Runtime as pallet_oracle::Config>::RegisterOrigin,
    "DEOS Oracle feed registration"
  );
  assert_root_only!(
    <Runtime as polkadot_sdk::pallet_preimage::Config>::ManagerOrigin,
    "preimage administration"
  );
  assert_root_only!(
    <Runtime as polkadot_sdk::pallet_xcm::Config>::AdminOrigin,
    "XCM administration"
  );
  assert_root_only!(
    <Runtime as polkadot_sdk::cumulus_pallet_xcmp_queue::Config>::ControllerOrigin,
    "XCMP queue control"
  );

  type AssetCreator = <Runtime as polkadot_sdk::pallet_assets::Config>::CreateOrigin;
  assert!(
    <AssetCreator as polkadot_sdk::frame_support::traits::EnsureOriginWithArg<
      RuntimeOrigin,
      u32,
    >>::try_origin(RuntimeOrigin::root(), &0)
    .is_ok(),
    "pallet-assets creation must accept Root",
  );
  assert!(
    <AssetCreator as polkadot_sdk::frame_support::traits::EnsureOriginWithArg<
      RuntimeOrigin,
      u32,
    >>::try_origin(RuntimeOrigin::signed(super::common::ALICE), &0)
    .is_err(),
    "pallet-assets creation must reject signed authority",
  );

  type CollatorUpdater = <Runtime as polkadot_sdk::pallet_collator_selection::Config>::UpdateOrigin;
  assert!(
    <CollatorUpdater as EnsureOrigin<RuntimeOrigin>>::try_origin(RuntimeOrigin::root()).is_ok(),
    "collator updates accept Root",
  );
  assert!(
    <CollatorUpdater as EnsureOrigin<RuntimeOrigin>>::try_origin(RuntimeOrigin::signed(
      super::common::ALICE,
    ))
    .is_err(),
    "collator updates reject signed authority outside the explicit relay StakingAdmin path",
  );

  type OraclePublisher = <Runtime as pallet_oracle::Config>::PublishOrigin;
  assert!(
    <OraclePublisher as EnsureOrigin<RuntimeOrigin>>::try_origin(RuntimeOrigin::signed(
      super::common::ALICE,
    ))
    .is_ok(),
    "Oracle publication is the explicitly bounded signed exception",
  );
  assert!(
    <OraclePublisher as EnsureOrigin<RuntimeOrigin>>::try_origin(RuntimeOrigin::root()).is_err(),
    "Oracle publication must still pass feed-level producer checks",
  );
}

#[cfg(feature = "try-runtime")]
#[test]
fn router_try_state_reconciles_lp_pairs_with_pool_and_asset_truth() {
  use super::common::{ALICE, create_pool, seeded_test_ext};
  use crate::{DeosRouter, Runtime, RuntimeOrigin, configs::AssetKind};
  use polkadot_sdk::frame_support::{assert_ok, traits::Hooks};
  use polkadot_sdk::pallet_asset_conversion::PoolLocator;

  seeded_test_ext().execute_with(|| {
    let pair = (
      AssetKind::Native,
      AssetKind::Local(primitives::ecosystem::protocol_tokens::BLDR_ASSET_ID),
    );
    assert_ok!(create_pool(RuntimeOrigin::signed(ALICE), pair.0, pair.1,));
    let pool_id = <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::pool_id(
      &pair.0, &pair.1,
    )
    .expect("fixture pool identity resolves");
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get(pool_id)
      .expect("fixture pool exists");
    assert_ok!(<DeosRouter as Hooks<crate::BlockNumber>>::try_state(1));

    pallet_deos_router::LpPairByTokenId::<Runtime>::mutate(|pairs| {
      pairs.remove(&pool.lp_token);
    });
    assert!(<DeosRouter as Hooks<crate::BlockNumber>>::try_state(1).is_err());
    assert_ok!(crate::configs::assets_config::register_pool_lp_pair(
      pair.0, pair.1,
    ));

    polkadot_sdk::pallet_assets::Asset::<Runtime>::remove(pool.lp_token);
    assert!(<DeosRouter as Hooks<crate::BlockNumber>>::try_state(1).is_err());
  });
}
