use crate::*;
use polkadot_sdk::frame_benchmarking::v2::*;
use polkadot_sdk::frame_system::RawOrigin;
use polkadot_sdk::staging_xcm::latest::{Junction::Parachain, Junctions::X1, Location};
use primitives::assets::CurrencyMetadata;

fn test_location(id: u32) -> Location {
  Location::new(1, X1([Parachain(id)].into()))
}

fn test_metadata() -> CurrencyMetadata {
  CurrencyMetadata {
    name: b"Test Asset".to_vec(),
    symbol: b"TST".to_vec(),
    decimals: 12,
  }
}

#[benchmarks(
  where
    <T as polkadot_sdk::pallet_assets::Config>::AssetId: Into<u32> + Copy + From<u32>,
    <T as polkadot_sdk::pallet_assets::Config>::AssetIdParameter:
      From<<T as polkadot_sdk::pallet_assets::Config>::AssetId> + Copy,
    T::Balance: From<u32>,
)]
mod benches {
  use super::*;

  #[benchmark]
  fn register_foreign_asset() {
    let location = test_location(2000);
    let metadata = test_metadata();
    let min_balance: T::Balance = 1u32.into();

    #[extrinsic_call]
    register_foreign_asset(RawOrigin::Root, location, metadata, min_balance, false);
  }

  #[benchmark]
  fn register_foreign_asset_with_id() {
    let location = test_location(3000);
    let metadata = test_metadata();
    let min_balance: T::Balance = 1u32.into();
    let asset_id: T::AssetId = (primitives::assets::TYPE_FOREIGN | 99u32).into();

    #[extrinsic_call]
    register_foreign_asset_with_id(
      RawOrigin::Root,
      location,
      asset_id,
      metadata,
      min_balance,
      false,
    );
  }

  #[benchmark]
  fn link_existing_asset() {
    let pre_location = test_location(4000);
    let pre_metadata = test_metadata();
    let min_balance: T::Balance = 1u32.into();
    let asset_id: T::AssetId = (primitives::assets::TYPE_FOREIGN | 42u32).into();
    // Pre-register with explicit foreign-masked ID
    pallet::Pallet::<T>::register_foreign_asset_with_id(
      RawOrigin::Root.into(),
      pre_location.clone(),
      asset_id,
      pre_metadata,
      min_balance,
      false,
    )
    .expect("pre-registration failed");

    let link_location = test_location(4001);
    // Remove the old mapping so we can link fresh
    ForeignAssetMapping::<T>::remove(&pre_location);

    #[extrinsic_call]
    link_existing_asset(RawOrigin::Root, link_location, asset_id);
  }

  #[benchmark]
  fn migrate_location_key() {
    let old_location = test_location(5000);
    let metadata = test_metadata();
    let min_balance: T::Balance = 1u32.into();
    pallet::Pallet::<T>::register_foreign_asset(
      RawOrigin::Root.into(),
      old_location.clone(),
      metadata,
      min_balance,
      false,
    )
    .expect("pre-registration failed");

    let new_location = test_location(5001);

    #[extrinsic_call]
    migrate_location_key(RawOrigin::Root, old_location, new_location);
  }

  #[cfg(test)]
  use crate::mock::{Test, new_test_ext};
  #[cfg(test)]
  impl_benchmark_test_suite!(Pallet, new_test_ext(), Test);
}
