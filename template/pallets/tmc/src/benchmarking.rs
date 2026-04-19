#![cfg(feature = "runtime-benchmarks")]

use super::*;
use polkadot_sdk::frame_benchmarking::v2::*;
use polkadot_sdk::frame_system::RawOrigin;
use primitives::AssetKind;

#[benchmarks]
mod benches {
  use super::*;

  #[benchmark]
  fn create_curve() {
    let token_asset = AssetKind::Local(1);
    let foreign_asset = AssetKind::Local(2);
    let initial_price: u128 = 1000;
    let slope: u128 = 1;

    #[extrinsic_call]
    create_curve(
      RawOrigin::Root,
      token_asset,
      foreign_asset,
      initial_price,
      slope,
    );

    assert!(TokenCurves::<T>::contains_key(token_asset));
  }

  impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
