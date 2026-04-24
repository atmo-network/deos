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

fn register_native_pool<T>() -> <T as Config>::AssetId
where
  T: Config
    + pallet_assets::Config<AssetId = <T as Config>::AssetId, Balance = <T as Config>::Balance>,
  <T as pallet_assets::Config>::AssetIdParameter: From<<T as Config>::AssetId> + Copy,
  <T as Config>::AssetId: From<u32>,
{
  let asset_id = T::NativeStakingAssetId::get();
  if !Pools::<T>::contains_key(asset_id) {
    register_pool::<T>(asset_id);
  }
  asset_id
}

fn benchmark_amount<T: Config>(value: u32) -> <T as Config>::Balance {
  <T as Config>::Balance::from(value)
}

fn benchmark_operator<T: Config>(name: &'static str) -> T::AccountId {
  account(name, 0, 0)
}

fn setup_collator_lp_lock<T>(
  caller: &T::AccountId,
  operator: &T::AccountId,
  amount: <T as Config>::Balance,
) -> <T as Config>::AssetId
where
  T: Config
    + pallet_assets::Config<AssetId = <T as Config>::AssetId, Balance = <T as Config>::Balance>,
  <T as pallet_assets::Config>::AssetIdParameter: From<<T as Config>::AssetId> + Copy,
  <T as Config>::AssetId: From<u32>,
{
  register_native_pool::<T>();
  T::NativeOperatorValidator::benchmark_prepare_valid_operator(operator);
  let lp_asset_id =
    <T as Config>::BenchmarkHelper::prepare_native_staking_lp(caller, amount + amount)
      .expect("benchmark helper must prepare native staking LP");
  Pallet::<T>::lock_native_lp_for_collator(
    RawOrigin::Signed(caller.clone()).into(),
    lp_asset_id,
    amount,
    operator.clone(),
  )
  .expect("benchmark native LP lock must succeed");
  lp_asset_id
}

fn setup_native_reward_claim<T>(
  caller: &T::AccountId,
  operator: &T::AccountId,
  reward_amount: <T as Config>::Balance,
) -> <T as Config>::RewardEpoch
where
  T: Config
    + pallet_assets::Config<AssetId = <T as Config>::AssetId, Balance = <T as Config>::Balance>,
  <T as pallet_assets::Config>::AssetIdParameter: From<<T as Config>::AssetId> + Copy,
  <T as Config>::AssetId: From<u32>,
  <T as Config>::RewardEpoch: From<u32>,
  BlockNumberFor<T>: From<u32>,
{
  let asset_id = register_native_pool::<T>();
  let governance_domain = Pallet::<T>::reward_governance_domain(asset_id)
    .expect("benchmark native asset must resolve governance domain");
  T::RewardCoefficientProvider::benchmark_prepare_positive_coefficient(governance_domain, caller);
  mint_to::<T>(asset_id, caller, benchmark_amount::<T>(500));
  let _ = Pallet::<T>::stake_native(
    RawOrigin::Signed(caller.clone()).into(),
    benchmark_amount::<T>(100),
  );
  setup_collator_lp_lock::<T>(caller, operator, benchmark_amount::<T>(100));
  let claim_epoch = T::RewardEpochProvider::current_reward_epoch();
  Pallet::<T>::bootstrap_reward_snapshot(RawOrigin::Root.into(), asset_id, caller.clone())
    .expect("benchmark reward bootstrap must succeed");
  let reward_account = Pallet::<T>::reward_account_for(asset_id);
  mint_to::<T>(asset_id, &reward_account, reward_amount);
  Pallet::<T>::note_reward_inflow(asset_id, reward_amount)
    .expect("benchmark reward inflow must succeed");
  frame_system::Pallet::<T>::set_block_number(2u32.into());
  claim_epoch
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
  fn set_operator_commission() {
    let caller: T::AccountId = whitelisted_caller();
    let commission = T::MaxOperatorCommission::get();
    #[extrinsic_call]
    set_operator_commission(RawOrigin::Signed(caller.clone()), commission);
    assert_eq!(OperatorCommissions::<T>::get(&caller), commission);
  }

  #[benchmark]
  fn lock_native_lp_for_collator() {
    let caller: T::AccountId = whitelisted_caller();
    let operator = benchmark_operator::<T>("native-operator");
    let amount = benchmark_amount::<T>(40);
    register_native_pool::<T>();
    T::NativeOperatorValidator::benchmark_prepare_valid_operator(&operator);
    let lp_asset_id =
      <T as Config>::BenchmarkHelper::prepare_native_staking_lp(&caller, amount + amount)
        .expect("benchmark helper must prepare native staking LP");
    Pallet::<T>::lock_native_lp_for_collator(
      RawOrigin::Signed(caller.clone()).into(),
      lp_asset_id,
      amount,
      operator.clone(),
    )
    .expect("benchmark existing native LP lock setup must succeed");
    #[extrinsic_call]
    lock_native_lp_for_collator(
      RawOrigin::Signed(caller.clone()),
      lp_asset_id,
      amount,
      operator.clone(),
    );
    assert_eq!(
      AccountNativeCollatorLpLocked::<T>::get(&caller),
      amount + amount
    );
  }

  #[benchmark]
  fn request_unlock_native_lp() {
    let caller: T::AccountId = whitelisted_caller();
    let operator = benchmark_operator::<T>("native-operator");
    let amount = benchmark_amount::<T>(15);
    setup_collator_lp_lock::<T>(&caller, &operator, amount + amount);
    #[extrinsic_call]
    request_unlock_native_lp(RawOrigin::Signed(caller.clone()), operator.clone(), amount);
    assert!(PendingNativeLpUnlocks::<T>::contains_key(
      &caller, &operator
    ));
  }

  #[benchmark]
  fn withdraw_unlocked_native_lp() {
    let caller: T::AccountId = whitelisted_caller();
    let operator = benchmark_operator::<T>("native-operator");
    let amount = benchmark_amount::<T>(15);
    setup_collator_lp_lock::<T>(&caller, &operator, amount);
    Pallet::<T>::request_unlock_native_lp(
      RawOrigin::Signed(caller.clone()).into(),
      operator.clone(),
      amount,
    )
    .expect("benchmark native LP unlock request must succeed");
    frame_system::Pallet::<T>::set_block_number(T::NativeLpUnlockDelay::get() + 2u32.into());
    #[extrinsic_call]
    withdraw_unlocked_native_lp(RawOrigin::Signed(caller.clone()), operator.clone());
    assert!(!PendingNativeLpUnlocks::<T>::contains_key(
      &caller, &operator
    ));
  }

  #[benchmark]
  fn redelegate_native_lp() {
    let caller: T::AccountId = whitelisted_caller();
    let from_operator = benchmark_operator::<T>("from-operator");
    let to_operator = benchmark_operator::<T>("to-operator");
    let amount = benchmark_amount::<T>(15);
    setup_collator_lp_lock::<T>(&caller, &from_operator, amount + amount);
    T::NativeOperatorValidator::benchmark_prepare_valid_operator(&to_operator);
    #[extrinsic_call]
    redelegate_native_lp(
      RawOrigin::Signed(caller.clone()),
      from_operator.clone(),
      to_operator.clone(),
      amount,
    );
    assert!(NativeLpLocks::<T>::contains_key(&caller, &to_operator));
  }

  #[benchmark]
  fn lock_native_lp_for_governance() {
    let caller: T::AccountId = whitelisted_caller();
    let amount = benchmark_amount::<T>(40);
    register_native_pool::<T>();
    let lp_asset_id =
      <T as Config>::BenchmarkHelper::prepare_native_staking_lp(&caller, amount + amount)
        .expect("benchmark helper must prepare native staking LP");
    Pallet::<T>::lock_native_lp_for_governance(
      RawOrigin::Signed(caller.clone()).into(),
      lp_asset_id,
      amount,
    )
    .expect("benchmark existing governance LP lock setup must succeed");
    #[extrinsic_call]
    lock_native_lp_for_governance(RawOrigin::Signed(caller.clone()), lp_asset_id, amount);
    assert_eq!(
      NativeGovernanceLpLocks::<T>::get(&caller).map(|lock| lock.amount),
      Some(amount + amount)
    );
  }

  #[benchmark]
  fn request_unlock_native_lp_for_governance() {
    let caller: T::AccountId = whitelisted_caller();
    let amount = benchmark_amount::<T>(15);
    register_native_pool::<T>();
    let lp_asset_id =
      <T as Config>::BenchmarkHelper::prepare_native_staking_lp(&caller, amount + amount)
        .expect("benchmark helper must prepare native staking LP");
    Pallet::<T>::lock_native_lp_for_governance(
      RawOrigin::Signed(caller.clone()).into(),
      lp_asset_id,
      amount + amount,
    )
    .expect("benchmark governance LP lock setup must succeed");
    #[extrinsic_call]
    request_unlock_native_lp_for_governance(RawOrigin::Signed(caller.clone()), amount);
    assert!(PendingNativeGovernanceLpUnlocks::<T>::contains_key(&caller));
  }

  #[benchmark]
  fn withdraw_unlocked_native_lp_for_governance() {
    let caller: T::AccountId = whitelisted_caller();
    let amount = benchmark_amount::<T>(15);
    register_native_pool::<T>();
    let lp_asset_id = <T as Config>::BenchmarkHelper::prepare_native_staking_lp(&caller, amount)
      .expect("benchmark helper must prepare native staking LP");
    Pallet::<T>::lock_native_lp_for_governance(
      RawOrigin::Signed(caller.clone()).into(),
      lp_asset_id,
      amount,
    )
    .expect("benchmark governance LP lock setup must succeed");
    Pallet::<T>::request_unlock_native_lp_for_governance(
      RawOrigin::Signed(caller.clone()).into(),
      amount,
    )
    .expect("benchmark governance LP unlock request must succeed");
    frame_system::Pallet::<T>::set_block_number(T::NativeLpUnlockDelay::get() + 2u32.into());
    #[extrinsic_call]
    withdraw_unlocked_native_lp_for_governance(RawOrigin::Signed(caller.clone()));
    assert!(!PendingNativeGovernanceLpUnlocks::<T>::contains_key(
      &caller
    ));
  }

  #[benchmark]
  fn lock_native_asset_for_governance() {
    let caller: T::AccountId = whitelisted_caller();
    let amount = benchmark_amount::<T>(40);
    register_native_pool::<T>();
    let asset_id =
      <T as Config>::BenchmarkHelper::prepare_native_governance_asset(&caller, amount + amount)
        .expect("benchmark helper must prepare native governance asset");
    Pallet::<T>::lock_native_asset_for_governance(
      RawOrigin::Signed(caller.clone()).into(),
      asset_id,
      amount,
    )
    .expect("benchmark existing governance asset lock setup must succeed");
    #[extrinsic_call]
    lock_native_asset_for_governance(RawOrigin::Signed(caller.clone()), asset_id, amount);
    assert_eq!(
      NativeGovernanceAssetLocked::<T>::get(&caller, asset_id),
      amount + amount
    );
  }

  #[benchmark]
  fn request_unlock_native_asset_for_governance() {
    let caller: T::AccountId = whitelisted_caller();
    let amount = benchmark_amount::<T>(15);
    register_native_pool::<T>();
    let asset_id =
      <T as Config>::BenchmarkHelper::prepare_native_governance_asset(&caller, amount + amount)
        .expect("benchmark helper must prepare native governance asset");
    Pallet::<T>::lock_native_asset_for_governance(
      RawOrigin::Signed(caller.clone()).into(),
      asset_id,
      amount + amount,
    )
    .expect("benchmark governance asset lock setup must succeed");
    #[extrinsic_call]
    request_unlock_native_asset_for_governance(RawOrigin::Signed(caller.clone()), asset_id, amount);
    assert!(PendingNativeGovernanceAssetUnlocks::<T>::contains_key(
      &caller, asset_id
    ));
  }

  #[benchmark]
  fn withdraw_unlocked_native_asset_for_governance() {
    let caller: T::AccountId = whitelisted_caller();
    let amount = benchmark_amount::<T>(15);
    register_native_pool::<T>();
    let asset_id = <T as Config>::BenchmarkHelper::prepare_native_governance_asset(&caller, amount)
      .expect("benchmark helper must prepare native governance asset");
    Pallet::<T>::lock_native_asset_for_governance(
      RawOrigin::Signed(caller.clone()).into(),
      asset_id,
      amount,
    )
    .expect("benchmark governance asset lock setup must succeed");
    Pallet::<T>::request_unlock_native_asset_for_governance(
      RawOrigin::Signed(caller.clone()).into(),
      asset_id,
      amount,
    )
    .expect("benchmark governance asset unlock request must succeed");
    frame_system::Pallet::<T>::set_block_number(T::NativeLpUnlockDelay::get() + 2u32.into());
    #[extrinsic_call]
    withdraw_unlocked_native_asset_for_governance(RawOrigin::Signed(caller.clone()), asset_id);
    assert!(!PendingNativeGovernanceAssetUnlocks::<T>::contains_key(
      &caller, asset_id
    ));
  }

  #[benchmark]
  fn claim_nomination_reward() {
    let caller: T::AccountId = whitelisted_caller();
    let operator = benchmark_operator::<T>("native-operator");
    let reward_amount = benchmark_amount::<T>(25);
    let claim_epoch = setup_native_reward_claim::<T>(&caller, &operator, reward_amount);
    #[extrinsic_call]
    claim_nomination_reward(RawOrigin::Signed(caller.clone()), claim_epoch);
    assert_eq!(
      RewardClaims::<T>::get((T::NativeStakingAssetId::get(), claim_epoch), &caller),
      Some(reward_amount)
    );
  }

  #[benchmark]
  fn claim_and_compound_nomination_reward() {
    let caller: T::AccountId = whitelisted_caller();
    let operator = benchmark_operator::<T>("native-operator");
    let reward_amount = benchmark_amount::<T>(1_000_000_000);
    let claim_epoch = setup_native_reward_claim::<T>(&caller, &operator, reward_amount);
    #[extrinsic_call]
    claim_and_compound_nomination_reward(
      RawOrigin::Signed(caller.clone()),
      claim_epoch,
      operator.clone(),
    );
    assert!(RewardClaims::<T>::contains_key(
      (T::NativeStakingAssetId::get(), claim_epoch),
      &caller
    ));
  }

  #[benchmark]
  fn claim_nomination_reward_batch(n: Linear<1, 16>) {
    let caller: T::AccountId = whitelisted_caller();
    let operator = benchmark_operator::<T>("native-operator");
    let asset_id = register_native_pool::<T>();
    let governance_domain = Pallet::<T>::reward_governance_domain(asset_id)
      .expect("benchmark native asset must resolve governance domain");
    T::RewardCoefficientProvider::benchmark_prepare_positive_coefficient(
      governance_domain,
      &caller,
    );
    mint_to::<T>(asset_id, &caller, benchmark_amount::<T>(500));
    let _ = Pallet::<T>::stake_native(
      RawOrigin::Signed(caller.clone()).into(),
      benchmark_amount::<T>(100),
    );
    setup_collator_lp_lock::<T>(&caller, &operator, benchmark_amount::<T>(100));
    let reward_amount = benchmark_amount::<T>(25);
    let reward_account = Pallet::<T>::reward_account_for(asset_id);
    let mut epochs = alloc::vec::Vec::new();
    for epoch_index in 1..=n {
      let claim_epoch: T::RewardEpoch = epoch_index.into();
      frame_system::Pallet::<T>::set_block_number(epoch_index.into());
      T::RewardCoefficientProvider::benchmark_prepare_positive_coefficient(
        governance_domain,
        &caller,
      );
      Pallet::<T>::bootstrap_reward_snapshot(RawOrigin::Root.into(), asset_id, caller.clone())
        .expect("benchmark reward bootstrap must succeed");
      mint_to::<T>(asset_id, &reward_account, reward_amount);
      Pallet::<T>::note_reward_inflow(asset_id, reward_amount)
        .expect("benchmark reward inflow must succeed");
      epochs.push(claim_epoch);
      frame_system::Pallet::<T>::set_block_number((epoch_index + 1).into());
    }
    let epochs: BoundedVec<T::RewardEpoch, T::MaxClaimEpochsPerCall> = epochs
      .try_into()
      .expect("benchmark epoch batch must respect runtime bound");
    #[extrinsic_call]
    claim_nomination_reward_batch(RawOrigin::Signed(caller.clone()), epochs.clone());
    for claim_epoch in epochs {
      assert!(RewardClaims::<T>::contains_key(
        (asset_id, claim_epoch),
        &caller
      ));
    }
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
