extern crate alloc;

use crate::{types::BenchmarkHelper, *};
use polkadot_sdk::frame_benchmarking::v2::*;
use polkadot_sdk::frame_support::traits::{
  fungible::Inspect as NativeInspect, fungibles::Inspect as FungiblesInspect,
};
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
    let bm_account = T::BurningManagerAccount::get();
    let caller_native_before = T::Currency::balance(&caller);
    let caller_foreign_before = match from {
      AssetKind::Local(id) | AssetKind::Foreign(id) => T::Assets::balance(id, &caller),
      AssetKind::Native => 0,
    };
    let bm_foreign_before = match from {
      AssetKind::Local(id) | AssetKind::Foreign(id) => T::Assets::balance(id, &bm_account),
      AssetKind::Native => 0,
    };
    let expected_fee = Pallet::<T>::calculate_router_fee(amount_in);

    #[extrinsic_call]
    swap(
      RawOrigin::Signed(caller.clone()),
      from,
      to,
      amount_in,
      min_amount_out,
      recipient,
      deadline,
    );

    let caller_native_after = T::Currency::balance(&caller);
    let caller_foreign_after = match from {
      AssetKind::Local(id) | AssetKind::Foreign(id) => T::Assets::balance(id, &caller),
      AssetKind::Native => 0,
    };
    let bm_foreign_after = match from {
      AssetKind::Local(id) | AssetKind::Foreign(id) => T::Assets::balance(id, &bm_account),
      AssetKind::Native => 0,
    };
    assert!(caller_native_after > caller_native_before);
    assert_eq!(
      caller_foreign_before.saturating_sub(caller_foreign_after),
      amount_in
    );
    assert_eq!(
      bm_foreign_after.saturating_sub(bm_foreign_before),
      expected_fee
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
