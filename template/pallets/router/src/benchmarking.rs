extern crate alloc;

use crate::{types::BenchmarkHelper, *};
use polkadot_sdk::frame_benchmarking::v2::*;
use polkadot_sdk::frame_system::RawOrigin;
use polkadot_sdk::sp_runtime::traits::SaturatedConversion;
use primitives::AssetKind;

#[benchmarks]
mod benches {
  use super::*;

  fn fund<T: Config>(caller: &T::AccountId, assets: &[AssetKind]) {
    let amount: u128 = 1_000_000_000_000_000_000;
    for asset in assets {
      T::BenchmarkHelper::create_asset(*asset).expect("asset creation must succeed");
      T::BenchmarkHelper::mint_asset(*asset, caller, amount.saturated_into())
        .expect("asset funding must succeed");
    }
  }

  fn add_pool<T: Config>(caller: &T::AccountId, asset_a: AssetKind, asset_b: AssetKind) {
    let liquidity: u128 = 100_000_000_000_000_000;
    T::BenchmarkHelper::create_pool(asset_a, asset_b).expect("pool creation must succeed");
    T::BenchmarkHelper::add_liquidity(
      caller,
      asset_a,
      asset_b,
      liquidity.saturated_into(),
      liquidity.saturated_into(),
    )
    .expect("liquidity setup must succeed");
  }

  #[benchmark]
  fn direct_xyk_exact_input() {
    let caller: T::AccountId = whitelisted_caller();
    let from = AssetKind::Local(11);
    let to = T::NativeAsset::get();
    fund::<T>(&caller, &[from, to]);
    add_pool::<T>(&caller, from, to);
    let amount_in = T::MinSwapForeign::get().saturating_mul(1000u32.into());

    #[block]
    {
      let outcome = Pallet::<T>::execute_swap_for(&caller, from, to, amount_in, 1, &caller)
        .expect("direct exact-input execution must succeed");
      assert_eq!(outcome.weight_class, RouteWeightClass::ExactInputDirectXyk);
    }
  }

  #[benchmark]
  fn direct_mint_exact_input() {
    let caller: T::AccountId = whitelisted_caller();
    let from = AssetKind::Local(12);
    let to = AssetKind::Local(13);
    fund::<T>(&caller, &[from, to]);
    T::BenchmarkHelper::create_tmc_curve(to, from).expect("curve creation must succeed");
    let amount_in = T::MinSwapForeign::get().saturating_mul(1000u32.into());

    #[block]
    {
      let outcome = Pallet::<T>::execute_swap_for(&caller, from, to, amount_in, 1, &caller)
        .expect("mint exact-input execution must succeed");
      assert_eq!(outcome.weight_class, RouteWeightClass::ExactInputDirectMint);
    }
  }

  #[benchmark]
  fn native_anchored_exact_input() {
    let caller: T::AccountId = whitelisted_caller();
    let from = AssetKind::Local(14);
    let to = AssetKind::Local(15);
    let native = T::NativeAsset::get();
    fund::<T>(&caller, &[from, native, to]);
    add_pool::<T>(&caller, from, native);
    add_pool::<T>(&caller, native, to);
    let amount_in = T::MinSwapForeign::get().saturating_mul(1000u32.into());

    #[block]
    {
      let outcome = Pallet::<T>::execute_swap_for(&caller, from, to, amount_in, 1, &caller)
        .expect("Native-anchored exact-input execution must succeed");
      assert_eq!(
        outcome.weight_class,
        RouteWeightClass::ExactInputNativeAnchoredXyk
      );
    }
  }

  #[benchmark]
  fn direct_xyk_exact_output() {
    let caller: T::AccountId = whitelisted_caller();
    let from = AssetKind::Local(16);
    let to = T::NativeAsset::get();
    fund::<T>(&caller, &[from, to]);
    add_pool::<T>(&caller, from, to);

    #[block]
    {
      let outcome = Pallet::<T>::execute_exact_out_for(&caller, from, to, 1, u128::MAX, &caller)
        .expect("direct exact-output execution must succeed");
      assert_eq!(outcome.weight_class, RouteWeightClass::ExactOutputDirectXyk);
    }
  }

  #[benchmark]
  fn native_anchored_exact_output() {
    let caller: T::AccountId = whitelisted_caller();
    let from = AssetKind::Local(17);
    let to = AssetKind::Local(18);
    let native = T::NativeAsset::get();
    fund::<T>(&caller, &[from, native, to]);
    add_pool::<T>(&caller, from, native);
    add_pool::<T>(&caller, native, to);

    #[block]
    {
      let outcome = Pallet::<T>::execute_exact_out_for(&caller, from, to, 1, u128::MAX, &caller)
        .expect("Native-anchored exact-output execution must succeed");
      assert_eq!(
        outcome.weight_class,
        RouteWeightClass::ExactOutputNativeAnchoredXyk
      );
    }
  }

  #[benchmark]
  fn update_router_fee() {
    let new_fee = polkadot_sdk::sp_runtime::Perbill::from_percent(1);

    #[extrinsic_call]
    update_router_fee(RawOrigin::Root, new_fee);
  }

  #[cfg(test)]
  use crate::mock::{Test, new_test_ext};
  #[cfg(test)]
  impl_benchmark_test_suite!(Pallet, new_test_ext(), Test);
}
