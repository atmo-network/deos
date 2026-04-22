#![cfg(feature = "runtime-benchmarks")]

use super::*;
use polkadot_sdk::frame_benchmarking::v2::*;
use polkadot_sdk::frame_support::traits::{
  fungible::Inspect as NativeInspect, fungibles::Inspect as FungiblesInspect,
};
use polkadot_sdk::frame_system::RawOrigin;
use polkadot_sdk::sp_runtime::traits::UniqueSaturatedInto;
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
    T::BenchmarkHelper::create_asset(1).expect("foreign asset must be creatable");
    T::BenchmarkHelper::create_asset(2).expect("token asset must be creatable");

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

  #[benchmark]
  fn mint_with_distribution() {
    let caller: T::AccountId = whitelisted_caller();
    let token_asset_id = 2u32;
    let token_asset = AssetKind::Local(token_asset_id);
    let foreign_asset = AssetKind::Native;
    let precision: u128 =
      <T::Balance as UniqueSaturatedInto<u128>>::unique_saturated_into(T::Precision::get());
    let foreign_amount = precision.saturating_mul(10);
    let output = T::MintOutputResolver::output_account(token_asset);
    T::BenchmarkHelper::create_asset(token_asset_id).expect("token asset must be creatable");
    T::BenchmarkHelper::mint_native(&caller, foreign_amount.saturating_mul(2))
      .expect("caller native funding must succeed");
    T::BenchmarkHelper::mint_native(&output, precision)
      .expect("output native funding must succeed");
    Pallet::<T>::create_curve(
      RawOrigin::Root.into(),
      token_asset,
      foreign_asset,
      precision,
      0,
    )
    .expect("curve creation must succeed in benchmark setup");
    let caller_native_before = T::Currency::balance(&caller);
    let caller_token_before = T::Assets::balance(token_asset_id, &caller);
    let output_native_before = T::Currency::balance(&output);
    let output_token_before = T::Assets::balance(token_asset_id, &output);

    #[block]
    {
      Pallet::<T>::mint_with_distribution(&caller, token_asset, foreign_asset, foreign_amount)
        .expect("mint_with_distribution must succeed in benchmark");
    }

    let caller_native_after = T::Currency::balance(&caller);
    let caller_token_after = T::Assets::balance(token_asset_id, &caller);
    let output_native_after = T::Currency::balance(&output);
    let output_token_after = T::Assets::balance(token_asset_id, &output);
    assert_eq!(
      caller_native_before.saturating_sub(caller_native_after),
      foreign_amount
    );
    assert!(caller_token_after > caller_token_before);
    assert_eq!(
      output_native_after.saturating_sub(output_native_before),
      foreign_amount
    );
    assert!(output_token_after > output_token_before);
  }

  impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
