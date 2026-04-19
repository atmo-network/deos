extern crate alloc;

use crate::{types::BenchmarkHelper, *};
use polkadot_sdk::frame_benchmarking::v2::*;
use polkadot_sdk::frame_system::RawOrigin;
use polkadot_sdk::sp_runtime::traits::SaturatedConversion;
use primitives::AssetKind;

#[benchmarks]
mod benches {
  use super::*;

  #[benchmark]
  fn swap() {
    let caller: T::AccountId = whitelisted_caller();
    let from = AssetKind::Local(1);
    let to = T::NativeAsset::get();
    let amount_in = T::MinSwapForeign::get().saturating_mul(1000u32.into());
    let min_amount_out = 1u128;
    let recipient = caller.clone();
    let deadline = 10000u32.into();

    // Setup assets and liquidity (fund 2x to keep ED buffer after transfers)
    T::BenchmarkHelper::create_asset(from).expect("Failed to create asset");
    let fund_amount: u128 = 1_000_000_000_000_000_000;
    let liquidity_amount: u128 = 100_000_000_000_000_000;
    T::BenchmarkHelper::mint_asset(to, &caller, fund_amount.saturated_into())
      .expect("Failed to mint native");
    T::BenchmarkHelper::mint_asset(from, &caller, fund_amount.saturated_into())
      .expect("Failed to mint foreign");

    T::BenchmarkHelper::create_pool(to, from).expect("Failed to create pool");
    T::BenchmarkHelper::add_liquidity(
      &caller,
      to,
      from,
      liquidity_amount.saturated_into(),
      liquidity_amount.saturated_into(),
    )
    .expect("Failed to add liquidity");

    // Fund BM account so fee routing doesn't fail
    let bm_account = T::BurningManagerAccount::get();
    T::BenchmarkHelper::mint_asset(to, &bm_account, fund_amount.saturated_into())
      .expect("Failed to fund BM account");

    #[extrinsic_call]
    swap(
      RawOrigin::Signed(caller),
      from,
      to,
      amount_in,
      min_amount_out,
      recipient,
      deadline,
    );
  }

  #[benchmark]
  fn add_tracked_asset() {
    let asset = AssetKind::Local(100);

    #[extrinsic_call]
    add_tracked_asset(RawOrigin::Root, asset);

    assert!(TrackedAssets::<T>::get().contains(&asset));
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
