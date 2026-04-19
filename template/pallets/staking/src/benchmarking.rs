#![cfg(feature = "runtime-benchmarks")]

extern crate alloc;

use crate::*;
use frame::prelude::*;
use polkadot_sdk::frame_benchmarking::{account, v2::*};
use polkadot_sdk::frame_support::traits::fungibles::{Inspect, Mutate};
use polkadot_sdk::frame_system::RawOrigin;
use polkadot_sdk::pallet_assets;
use polkadot_sdk::sp_runtime::traits::{One, Zero};

fn benchmark_asset_id<T>() -> <T as Config>::AssetId
where
  T: Config
    + pallet_assets::Config<AssetId = <T as Config>::AssetId, Balance = <T as Config>::Balance>,
  <T as Config>::AssetId: From<u32>,
{
  77u32.into()
}

fn benchmark_owner<T: Config>() -> T::AccountId {
  account("staking-owner", 0, 0)
}

fn create_asset<T>(asset_id: <T as Config>::AssetId)
where
  T: Config
    + pallet_assets::Config<AssetId = <T as Config>::AssetId, Balance = <T as Config>::Balance>,
  <T as pallet_assets::Config>::AssetIdParameter: From<<T as Config>::AssetId> + Copy,
  <T as Config>::AssetId: From<u32>,
{
  if pallet_assets::Pallet::<T>::asset_exists(asset_id) {
    return;
  }
  let owner = benchmark_owner::<T>();
  pallet_assets::Pallet::<T>::force_create(
    RawOrigin::Root.into(),
    <T as pallet_assets::Config>::AssetIdParameter::from(asset_id),
    T::Lookup::unlookup(owner),
    true,
    <T as Config>::Balance::one(),
  )
  .expect("benchmark asset creation must succeed");
}

fn register_pool<T>(asset_id: <T as Config>::AssetId)
where
  T: Config
    + pallet_assets::Config<AssetId = <T as Config>::AssetId, Balance = <T as Config>::Balance>,
  <T as pallet_assets::Config>::AssetIdParameter: From<<T as Config>::AssetId> + Copy,
  <T as Config>::AssetId: From<u32>,
{
  create_asset::<T>(asset_id);
  Pallet::<T>::register_staking_asset(RawOrigin::Root.into(), asset_id)
    .expect("benchmark pool registration must succeed");
}

fn mint_to<T>(asset_id: <T as Config>::AssetId, who: &T::AccountId, amount: <T as Config>::Balance)
where
  T: Config
    + pallet_assets::Config<AssetId = <T as Config>::AssetId, Balance = <T as Config>::Balance>,
{
  <pallet_assets::Pallet<T> as Mutate<T::AccountId>>::mint_into(asset_id, who, amount)
    .expect("benchmark mint must succeed");
}

fn seed_legacy_pool_with_position<T>(
  asset_id: <T as Config>::AssetId,
  holder: &T::AccountId,
  shares: <T as Config>::Balance,
) where
  T: Config
    + pallet_assets::Config<AssetId = <T as Config>::AssetId, Balance = <T as Config>::Balance>,
  <T as pallet_assets::Config>::AssetIdParameter: From<<T as Config>::AssetId> + Copy,
  <T as Config>::AssetId: From<u32>,
{
  create_asset::<T>(asset_id);
  let pool_account = Pallet::<T>::pool_account_for(asset_id);
  Pools::<T>::insert(
    asset_id,
    PoolState {
      total_shares: shares,
      accounted_balance: shares,
      active_staker_count: 1,
    },
  );
  Positions::<T>::insert(asset_id, holder, StakePosition { shares });
  mint_to::<T>(asset_id, &pool_account, shares);
}

#[benchmarks(where
  T: pallet_assets::Config<AssetId = <T as Config>::AssetId, Balance = <T as Config>::Balance>,
  <T as Config>::AssetId: From<u32>,
  <T as Config>::RewardEpoch: From<u32>,
  <T as pallet_assets::Config>::AssetIdParameter: From<<T as Config>::AssetId> + Copy,
  BlockNumberFor<T>: From<u32>,
)]
mod benches {
  use super::*;

  #[benchmark]
  fn register_staking_asset() {
    let asset_id = benchmark_asset_id::<T>();
    create_asset::<T>(asset_id);
    #[extrinsic_call]
    register_staking_asset(RawOrigin::Root, asset_id);
    assert!(Pools::<T>::contains_key(asset_id));
  }

  #[benchmark]
  fn initialize_staked_asset() {
    let asset_id = benchmark_asset_id::<T>();
    let holder: T::AccountId = account("legacy-holder", 0, 0);
    let shares = <T as Config>::Balance::from(100u32);
    seed_legacy_pool_with_position::<T>(asset_id, &holder, shares);
    let staked_asset_id = Pallet::<T>::staked_asset_id(asset_id)
      .expect("benchmark asset id must resolve receipt asset");
    #[extrinsic_call]
    initialize_staked_asset(RawOrigin::Root, asset_id);
    assert!(<pallet_assets::Pallet<T> as Inspect<T::AccountId>>::asset_exists(staked_asset_id));
  }

  #[benchmark]
  fn convert_position_to_receipt() {
    let asset_id = benchmark_asset_id::<T>();
    let holder: T::AccountId = whitelisted_caller();
    let shares = <T as Config>::Balance::from(100u32);
    seed_legacy_pool_with_position::<T>(asset_id, &holder, shares);
    Pallet::<T>::initialize_staked_asset(RawOrigin::Root.into(), asset_id)
      .expect("benchmark staked asset initialization must succeed");
    let staked_asset_id = Pallet::<T>::staked_asset_id(asset_id)
      .expect("benchmark asset id must resolve receipt asset");
    #[extrinsic_call]
    convert_position_to_receipt(RawOrigin::Signed(holder.clone()), asset_id);
    assert!(Positions::<T>::get(asset_id, holder.clone()).is_none());
    assert_eq!(
      Pools::<T>::get(asset_id)
        .expect("pool must exist after conversion")
        .active_staker_count,
      0
    );
    assert_eq!(
      <pallet_assets::Pallet<T> as Inspect<T::AccountId>>::balance(staked_asset_id, &holder),
      shares
    );
  }

  #[benchmark]
  fn sync_pool() {
    let asset_id = benchmark_asset_id::<T>();
    register_pool::<T>(asset_id);
    let caller: T::AccountId = account("sync-caller", 0, 0);
    let pool_account = Pallet::<T>::pool_account_for(asset_id);
    let inflow = <T as Config>::Balance::from(100u32);
    mint_to::<T>(asset_id, &pool_account, inflow);
    #[extrinsic_call]
    sync_pool(RawOrigin::Signed(caller), asset_id);
    assert_eq!(
      Pools::<T>::get(asset_id)
        .expect("pool must exist")
        .accounted_balance,
      inflow
    );
  }

  #[benchmark]
  fn stake() {
    let asset_id = benchmark_asset_id::<T>();
    register_pool::<T>(asset_id);
    let caller: T::AccountId = whitelisted_caller();
    let amount = <T as Config>::Balance::from(100u32);
    let staked_asset_id = Pallet::<T>::staked_asset_id(asset_id)
      .expect("benchmark asset id must resolve receipt asset");
    mint_to::<T>(asset_id, &caller, amount + <T as Config>::Balance::one());
    #[extrinsic_call]
    stake(RawOrigin::Signed(caller.clone()), asset_id, amount);
    assert!(Positions::<T>::get(asset_id, caller.clone()).is_none());
    assert_eq!(
      <pallet_assets::Pallet<T> as Inspect<T::AccountId>>::balance(staked_asset_id, &caller),
      amount
    );
  }

  #[benchmark]
  fn unstake() {
    let asset_id = benchmark_asset_id::<T>();
    register_pool::<T>(asset_id);
    let caller: T::AccountId = whitelisted_caller();
    let amount = <T as Config>::Balance::from(100u32);
    let burn = <T as Config>::Balance::from(40u32);
    let staked_asset_id = Pallet::<T>::staked_asset_id(asset_id)
      .expect("benchmark asset id must resolve receipt asset");
    mint_to::<T>(asset_id, &caller, amount + <T as Config>::Balance::one());
    Pallet::<T>::stake(RawOrigin::Signed(caller.clone()).into(), asset_id, amount)
      .expect("benchmark stake setup must succeed");
    #[extrinsic_call]
    unstake(RawOrigin::Signed(caller.clone()), asset_id, burn);
    assert!(Positions::<T>::get(asset_id, caller.clone()).is_none());
    assert_eq!(
      <pallet_assets::Pallet<T> as Inspect<T::AccountId>>::balance(staked_asset_id, &caller),
      amount - burn
    );
  }

  #[benchmark]
  fn recover_unowned_pool() {
    let asset_id = benchmark_asset_id::<T>();
    register_pool::<T>(asset_id);
    let beneficiary: T::AccountId = account("recovery-beneficiary", 0, 0);
    let pool_account = Pallet::<T>::pool_account_for(asset_id);
    let recoverable = <T as Config>::Balance::from(100u32);
    mint_to::<T>(asset_id, &pool_account, recoverable);
    #[extrinsic_call]
    recover_unowned_pool(RawOrigin::Root, asset_id, beneficiary.clone());
    assert_eq!(
      Pools::<T>::get(asset_id)
        .expect("pool must exist after recovery")
        .accounted_balance,
      Zero::zero()
    );
    assert_eq!(
      <pallet_assets::Pallet<T> as Inspect<T::AccountId>>::balance(asset_id, &pool_account),
      Zero::zero()
    );
  }

  #[benchmark]
  fn bind_native() {
    let binder: T::AccountId = whitelisted_caller();
    let operator: T::AccountId = account("delegation-operator", 0, 0);
    T::NativeBindingTargetValidator::benchmark_prepare_valid_operator(&operator);
    #[extrinsic_call]
    bind_native(RawOrigin::Signed(binder.clone()), operator.clone());
    assert_eq!(NativeBindings::<T>::get(&binder), Some(operator));
  }

  #[benchmark]
  fn set_operator_commission() {
    let caller: T::AccountId = whitelisted_caller();
    let commission = T::MaxOperatorCommission::get();
    #[extrinsic_call]
    set_operator_commission(RawOrigin::Signed(caller.clone()), commission);
    assert_eq!(OperatorCommissions::<T>::get(&caller), commission);
  }

  #[benchmark]
  fn clear_native_binding() {
    let binder: T::AccountId = whitelisted_caller();
    let operator: T::AccountId = account("delegation-operator", 0, 0);
    T::NativeBindingTargetValidator::benchmark_prepare_valid_operator(&operator);
    Pallet::<T>::bind_native(RawOrigin::Signed(binder.clone()).into(), operator)
      .expect("benchmark native binding setup must succeed");
    #[extrinsic_call]
    clear_native_binding(RawOrigin::Signed(binder.clone()));
    assert!(NativeBindings::<T>::get(&binder).is_none());
  }

  #[benchmark]
  fn bootstrap_reward_snapshot() {
    let asset_id = benchmark_asset_id::<T>();
    register_pool::<T>(asset_id);
    let holder: T::AccountId = account("reward-holder", 0, 0);
    let amount = <T as Config>::Balance::from(100u32);
    mint_to::<T>(asset_id, &holder, amount + <T as Config>::Balance::one());
    Pallet::<T>::stake(RawOrigin::Signed(holder.clone()).into(), asset_id, amount)
      .expect("benchmark stake setup must succeed");
    let governance_domain = Pallet::<T>::reward_governance_domain(asset_id)
      .expect("benchmark asset must resolve governance domain");
    T::RewardCoefficientProvider::benchmark_prepare_positive_coefficient(
      governance_domain,
      &holder,
    );
    #[extrinsic_call]
    bootstrap_reward_snapshot(RawOrigin::Root, asset_id, holder.clone());
    assert!(RewardActiveWeightSnapshots::<T>::contains_key(
      asset_id, holder
    ));
  }

  #[benchmark]
  fn claim_reward() {
    let asset_id = benchmark_asset_id::<T>();
    register_pool::<T>(asset_id);
    let holder: T::AccountId = whitelisted_caller();
    let amount = <T as Config>::Balance::from(100u32);
    let claim_reward_amount = <T as Config>::Balance::from(25u32);
    mint_to::<T>(asset_id, &holder, amount + <T as Config>::Balance::one());
    Pallet::<T>::stake(RawOrigin::Signed(holder.clone()).into(), asset_id, amount)
      .expect("benchmark stake setup must succeed");
    let governance_domain = Pallet::<T>::reward_governance_domain(asset_id)
      .expect("benchmark asset must resolve governance domain");
    T::RewardCoefficientProvider::benchmark_prepare_positive_coefficient(
      governance_domain,
      &holder,
    );
    let claim_epoch = T::RewardEpochProvider::current_reward_epoch();
    Pallet::<T>::bootstrap_reward_snapshot(RawOrigin::Root.into(), asset_id, holder.clone())
      .expect("benchmark reward bootstrap must succeed");
    let reward_account = Pallet::<T>::reward_account_for(asset_id);
    mint_to::<T>(asset_id, &reward_account, claim_reward_amount);
    Pallet::<T>::note_reward_inflow(asset_id, claim_reward_amount)
      .expect("benchmark reward inflow must succeed");
    frame_system::Pallet::<T>::set_block_number(2u32.into());
    #[extrinsic_call]
    claim_reward(RawOrigin::Signed(holder.clone()), asset_id, claim_epoch);
    assert!(RewardClaims::<T>::contains_key(
      (asset_id, claim_epoch),
      holder
    ));
  }

  #[benchmark]
  fn claim_reward_batch(n: Linear<1, 16>) {
    let asset_id = benchmark_asset_id::<T>();
    register_pool::<T>(asset_id);
    let holder: T::AccountId = whitelisted_caller();
    let amount = <T as Config>::Balance::from(100u32);
    let claim_reward_amount = <T as Config>::Balance::from(25u32);
    mint_to::<T>(asset_id, &holder, amount + <T as Config>::Balance::one());
    Pallet::<T>::stake(RawOrigin::Signed(holder.clone()).into(), asset_id, amount)
      .expect("benchmark stake setup must succeed");
    let governance_domain = Pallet::<T>::reward_governance_domain(asset_id)
      .expect("benchmark asset must resolve governance domain");
    T::RewardCoefficientProvider::benchmark_prepare_positive_coefficient(
      governance_domain,
      &holder,
    );
    let reward_account = Pallet::<T>::reward_account_for(asset_id);
    let mut epochs = alloc::vec::Vec::new();
    for epoch_index in 1..=n {
      let claim_epoch: T::RewardEpoch = epoch_index.into();
      frame_system::Pallet::<T>::set_block_number(epoch_index.into());
      T::RewardCoefficientProvider::benchmark_prepare_positive_coefficient(
        governance_domain,
        &holder,
      );
      Pallet::<T>::bootstrap_reward_snapshot(RawOrigin::Root.into(), asset_id, holder.clone())
        .expect("benchmark reward bootstrap must succeed");
      mint_to::<T>(asset_id, &reward_account, claim_reward_amount);
      Pallet::<T>::note_reward_inflow(asset_id, claim_reward_amount)
        .expect("benchmark reward inflow must succeed");
      epochs.push(claim_epoch);
      let next_block = epoch_index + 1;
      frame_system::Pallet::<T>::set_block_number(next_block.into());
    }
    let epochs: BoundedVec<T::RewardEpoch, T::MaxClaimEpochsPerCall> = epochs
      .try_into()
      .expect("benchmark epoch batch must respect runtime bound");
    #[extrinsic_call]
    claim_reward_batch(RawOrigin::Signed(holder.clone()), asset_id, epochs.clone());
    for claim_epoch in epochs {
      assert!(RewardClaims::<T>::contains_key(
        (asset_id, claim_epoch),
        holder.clone()
      ));
    }
  }

  #[cfg(test)]
  use crate::mock::{Test, new_test_ext};
  #[cfg(test)]
  impl_benchmark_test_suite!(Pallet, new_test_ext(), Test);
}
