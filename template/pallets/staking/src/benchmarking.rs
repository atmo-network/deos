#![cfg(feature = "runtime-benchmarks")]

extern crate alloc;

use crate::*;
use frame::prelude::*;
use polkadot_sdk::frame_benchmarking::{account, v2::*};
use polkadot_sdk::frame_support::traits::{
  Currency,
  fungibles::{Inspect, Mutate},
};
use polkadot_sdk::frame_system::RawOrigin;
use polkadot_sdk::pallet_assets;
use polkadot_sdk::sp_runtime::traits::{One, Saturating, Zero};

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

fn prepare_lp_backed_selection<T: Config>() {
  T::NativeSecurityModeProvider::benchmark_prepare_lp_backed_selection();
  assert_eq!(
    T::NativeSecurityModeProvider::mode(),
    NativeSecurityMode::LpBackedSelection,
    "native-security benchmarks require the LP-backed branch"
  );
}

fn empty_security_snapshot<T: Config>(epoch: SecurityEpoch) -> NativeSecurityEpochSnapshotOf<T> {
  NativeSecurityEpochSnapshot {
    epoch,
    participants: Default::default(),
    eligible_operators: Default::default(),
    total_reward_weight: Zero::zero(),
  }
}

fn setup_finalized_reward<T: Config>(
  caller: &T::AccountId,
  epoch: SecurityEpoch,
  amount: T::Balance,
) {
  prepare_lp_backed_selection::<T>();
  T::BenchmarkHelper::set_security_epoch(epoch.saturating_add(1));
  let participant = NativeSecurityAccountSnapshot {
    account: caller.clone(),
    conservative_native_value: T::Balance::one(),
    governance_coefficient: polkadot_sdk::sp_runtime::FixedU128::one(),
    reward_weight: T::Balance::one(),
  };
  let mut participants = BoundedVec::default();
  participants
    .try_push(participant)
    .expect("benchmark participant must fit");
  let snapshot = NativeSecurityEpochSnapshot {
    epoch,
    participants,
    eligible_operators: Default::default(),
    total_reward_weight: T::Balance::one(),
  };
  NativeSecurityEpochSnapshots::<T>::insert(epoch, snapshot);
  NativeSecurityRewardPots::<T>::insert(
    epoch,
    NativeSecurityRewardPot {
      total_reward_weight: T::Balance::one(),
      credited: amount,
      claimed: Zero::zero(),
      status: NativeSecurityRewardPotStatus::Finalized,
    },
  );
  NativeSecurityRewardLiability::<T>::put(amount);
  let caller_balance = amount
    .saturating_add(T::NativeCurrency::minimum_balance())
    .saturating_add(T::NativeCurrency::minimum_balance());
  <T as Config>::BenchmarkHelper::fund_native_account(caller, caller_balance);
  assert!(T::NativeCurrency::free_balance(caller) >= caller_balance);
  let reward_account = Pallet::<T>::native_security_reward_account();
  let reward_balance = amount.saturating_add(T::NativeCurrency::minimum_balance());
  <T as Config>::BenchmarkHelper::fund_native_account(&reward_account, reward_balance);
  assert!(T::NativeCurrency::free_balance(&reward_account) >= reward_balance);
  T::NativeCurrency::transfer(
    &reward_account,
    caller,
    amount,
    polkadot_sdk::frame_support::traits::ExistenceRequirement::KeepAlive,
  )
  .expect("benchmark reward transfer probe must succeed");
  T::NativeCurrency::transfer(
    caller,
    &reward_account,
    amount,
    polkadot_sdk::frame_support::traits::ExistenceRequirement::KeepAlive,
  )
  .expect("benchmark reward transfer probe reset must succeed");
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
  prepare_lp_backed_selection::<T>();
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

#[benchmarks(where
  T: pallet_assets::Config<AssetId = <T as Config>::AssetId, Balance = <T as Config>::Balance>,
  <T as Config>::AssetId: From<u32>,
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
  fn lock_native_lp_for_collator() {
    let caller: T::AccountId = whitelisted_caller();
    let operator = benchmark_operator::<T>("native-operator");
    let amount = benchmark_amount::<T>(40);
    prepare_lp_backed_selection::<T>();
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
  fn fund_native_security_reward() {
    prepare_lp_backed_selection::<T>();
    let epoch = T::SecurityEpochProvider::current_security_epoch();
    let snapshot = NativeSecurityEpochSnapshot {
      epoch,
      participants: Default::default(),
      eligible_operators: Default::default(),
      total_reward_weight: <T as Config>::Balance::one(),
    };
    ActiveNativeSecurityEpochSnapshot::<T>::put(&snapshot);
    NativeSecurityEpochSnapshots::<T>::insert(epoch, snapshot);
    NativeSecurityRewardPots::<T>::insert(
      epoch,
      NativeSecurityRewardPot {
        total_reward_weight: <T as Config>::Balance::one(),
        credited: Zero::zero(),
        claimed: Zero::zero(),
        status: NativeSecurityRewardPotStatus::Open,
      },
    );
    let amount = benchmark_amount::<T>(1);
    let source = T::SecurityRewardFundingSource::get();
    let _ = T::NativeCurrency::deposit_creating(
      &source,
      amount.saturating_add(T::NativeCurrency::minimum_balance()),
    );
    #[extrinsic_call]
    fund_native_security_reward(RawOrigin::Root, amount);
    assert_eq!(NativeSecurityRewardLiability::<T>::get(), amount);
  }

  #[benchmark]
  fn claim_native_security_reward() {
    let caller: T::AccountId = account("reward-claimant", 0, 0);
    let epoch = 0;
    let amount = T::NativeCurrency::minimum_balance().saturating_mul(benchmark_amount::<T>(1_000));
    setup_finalized_reward::<T>(&caller, epoch, amount);
    #[block]
    {
      assert!(
        T::NativeCurrency::free_balance(&caller) >= amount,
        "benchmark claimant funding disappeared before measurement"
      );
      let reward_account = Pallet::<T>::native_security_reward_account();
      assert!(
        T::NativeCurrency::free_balance(&reward_account) >= amount,
        "benchmark reward custody funding disappeared before measurement"
      );
      Pallet::<T>::claim_native_security_reward(RawOrigin::Signed(caller.clone()).into(), epoch)
        .expect("benchmark liquid claim must succeed");
    }
    assert!(NativeSecurityRewardClaims::<T>::contains_key(
      epoch, &caller
    ));
    assert!(NativeSecurityRewardLiability::<T>::get().is_zero());
  }

  #[benchmark]
  fn claim_native_security_reward_batch(c: Linear<1, 12>) {
    let caller: T::AccountId = account("reward-batch-claimant", 0, 0);
    let amount = T::NativeCurrency::minimum_balance().saturating_mul(benchmark_amount::<T>(1_000));
    prepare_lp_backed_selection::<T>();
    <T as Config>::BenchmarkHelper::set_security_epoch(c + 1);
    let epoch_count = c.min(T::MaxSecurityRewardClaimsPerCall::get());
    let mut epochs = BoundedVec::default();
    for epoch in 0..epoch_count {
      setup_finalized_reward::<T>(&caller, epoch, amount);
      epochs.try_push(epoch).expect("benchmark epoch must fit");
    }
    let total = amount.saturating_mul(epoch_count.into());
    NativeSecurityRewardLiability::<T>::put(total);
    let reward_account = Pallet::<T>::native_security_reward_account();
    <T as Config>::BenchmarkHelper::fund_native_account(
      &reward_account,
      total.saturating_add(T::NativeCurrency::minimum_balance()),
    );
    #[extrinsic_call]
    claim_native_security_reward_batch(RawOrigin::Signed(caller.clone()), epochs);
    assert!(NativeSecurityRewardLiability::<T>::get().is_zero());
  }

  #[benchmark]
  fn claim_and_compound_native_security_reward() {
    let caller: T::AccountId = whitelisted_caller();
    let operator = benchmark_operator::<T>("compound-operator");
    let epoch = 0;
    let amount = benchmark_amount::<T>(1_000_000);
    prepare_lp_backed_selection::<T>();
    register_native_pool::<T>();
    T::NativeOperatorValidator::benchmark_prepare_valid_operator(&operator);
    let _ = <T as Config>::BenchmarkHelper::prepare_native_staking_lp(&caller, amount)
      .expect("benchmark helper must prepare compound liquidity");
    setup_finalized_reward::<T>(&caller, epoch, amount);
    #[extrinsic_call]
    claim_and_compound_native_security_reward(
      RawOrigin::Signed(caller.clone()),
      epoch,
      operator.clone(),
      <T as Config>::Balance::one(),
    );
    assert!(NativeSecurityRewardClaims::<T>::contains_key(
      epoch, &caller
    ));
    assert!(NativeLpLocks::<T>::contains_key(&caller, &operator));
  }

  #[benchmark]
  fn expire_native_security_reward() {
    let caller: T::AccountId = whitelisted_caller();
    let epoch = 0;
    let amount = benchmark_amount::<T>(1_000);
    setup_finalized_reward::<T>(&caller, epoch, amount);
    for participant_index in 0..T::MaxNativeSecurityParticipants::get() {
      let participant: T::AccountId = account("expired-participant", participant_index, 0);
      NativeSecurityRewardClaims::<T>::insert(epoch, participant, ());
    }
    <T as Config>::BenchmarkHelper::set_security_epoch(
      T::SecurityRewardClaimHorizon::get().saturating_add(1),
    );
    #[extrinsic_call]
    expire_native_security_reward(RawOrigin::Signed(caller), epoch);
    assert!(!NativeSecurityRewardPots::<T>::contains_key(epoch));
    assert!(!NativeSecurityEpochSnapshots::<T>::contains_key(epoch));
  }

  #[benchmark(pov_mode = Measured)]
  fn settle_due_native_security_reward(r: Linear<1, 14>) {
    let caller: T::AccountId = whitelisted_caller();
    let amount = benchmark_amount::<T>(1_000);
    setup_finalized_reward::<T>(&caller, 0, amount);
    for participant_index in 0..T::MaxNativeSecurityParticipants::get() {
      let participant: T::AccountId = account("retained-participant", participant_index, 0);
      NativeSecurityRewardClaims::<T>::insert(0, participant, ());
    }
    let retained_epochs = r.min(T::SecurityRewardClaimHorizon::get().saturating_add(2));
    for epoch in 1..retained_epochs {
      NativeSecurityEpochSnapshots::<T>::insert(epoch, empty_security_snapshot::<T>(epoch));
      NativeSecurityRewardPots::<T>::insert(
        epoch,
        NativeSecurityRewardPot {
          total_reward_weight: Zero::zero(),
          credited: Zero::zero(),
          claimed: Zero::zero(),
          status: NativeSecurityRewardPotStatus::Finalized,
        },
      );
    }
    <T as Config>::BenchmarkHelper::set_security_epoch(
      T::SecurityRewardClaimHorizon::get().saturating_add(1),
    );
    #[block]
    {
      Pallet::<T>::settle_due_native_security_reward()
        .expect("oldest due reward must settle atomically");
    }
    assert!(!NativeSecurityRewardPots::<T>::contains_key(0));
  }

  #[benchmark]
  fn cancel_native_security_epoch_plan() {
    let epoch = 7;
    NativeSecurityEpochSnapshots::<T>::insert(epoch, empty_security_snapshot::<T>(epoch));
    NativeSecurityRewardPots::<T>::insert(
      epoch,
      NativeSecurityRewardPot {
        total_reward_weight: Zero::zero(),
        credited: Zero::zero(),
        claimed: Zero::zero(),
        status: NativeSecurityRewardPotStatus::Planned,
      },
    );
    #[block]
    {
      Pallet::<T>::cancel_native_security_epoch_plan(epoch)
        .expect("unactivated plan must cancel atomically");
    }
    assert!(!NativeSecurityRewardPots::<T>::contains_key(epoch));
  }

  #[benchmark]
  fn contract_native_security_obligations() {
    let active_epoch = 0;
    let planned_epoch = 1;
    let active = empty_security_snapshot::<T>(active_epoch);
    ActiveNativeSecurityEpochSnapshot::<T>::put(&active);
    NativeSecurityEpochSnapshots::<T>::insert(active_epoch, active);
    NativeSecurityRewardPots::<T>::insert(
      active_epoch,
      NativeSecurityRewardPot {
        total_reward_weight: Zero::zero(),
        credited: <T as Config>::Balance::one(),
        claimed: Zero::zero(),
        status: NativeSecurityRewardPotStatus::Open,
      },
    );
    NativeSecurityEpochSnapshots::<T>::insert(
      planned_epoch,
      empty_security_snapshot::<T>(planned_epoch),
    );
    NativeSecurityRewardPots::<T>::insert(
      planned_epoch,
      NativeSecurityRewardPot {
        total_reward_weight: Zero::zero(),
        credited: Zero::zero(),
        claimed: Zero::zero(),
        status: NativeSecurityRewardPotStatus::Planned,
      },
    );
    <T as Config>::BenchmarkHelper::set_security_epoch(planned_epoch);
    #[block]
    {
      Pallet::<T>::do_contract_native_security_obligations()
        .expect("trusted-mode obligations must contract atomically");
    }
    assert!(ActiveNativeSecurityEpochSnapshot::<T>::get().is_none());
    assert_eq!(
      NativeSecurityRewardPots::<T>::get(active_epoch)
        .expect("active pot becomes claimable")
        .status,
      NativeSecurityRewardPotStatus::Finalized
    );
    assert!(!NativeSecurityRewardPots::<T>::contains_key(planned_epoch));
  }

  #[benchmark(pov_mode = Measured)]
  fn open_native_security_epoch(p: Linear<1, 100>, r: Linear<1, 13>) {
    prepare_lp_backed_selection::<T>();
    register_native_pool::<T>();
    let amount = benchmark_amount::<T>(1);
    let governance_domain = T::NativeGovernanceDomainId::get();
    let mut eligible_operators = alloc::vec::Vec::new();
    <T as Config>::BenchmarkHelper::set_security_epoch(0);
    let retained_epochs = r.min(T::SecurityRewardClaimHorizon::get().saturating_add(1));
    for retained_index in 0..retained_epochs {
      let epoch = 1_000u32.saturating_add(retained_index);
      NativeSecurityEpochSnapshots::<T>::insert(epoch, empty_security_snapshot::<T>(epoch));
      NativeSecurityRewardPots::<T>::insert(
        epoch,
        NativeSecurityRewardPot {
          total_reward_weight: Zero::zero(),
          credited: Zero::zero(),
          claimed: Zero::zero(),
          status: NativeSecurityRewardPotStatus::Finalized,
        },
      );
    }
    let participant_count = p.min(T::MaxNativeSecurityParticipants::get());
    for participant_index in 0..participant_count {
      let participant: T::AccountId = account("security-participant", participant_index, 0);
      let operator: T::AccountId = account("security-operator", participant_index, 0);
      T::GovernanceParticipationCoefficientProvider::benchmark_prepare_positive_coefficient(
        governance_domain,
        &participant,
      );
      T::NativeOperatorValidator::benchmark_prepare_snapshot_operator(&operator);
      let lp_asset_id =
        <T as Config>::BenchmarkHelper::prepare_native_staking_lp(&participant, amount + amount)
          .expect("benchmark helper must prepare native staking LP");
      Pallet::<T>::lock_native_lp_for_collator(
        RawOrigin::Signed(participant).into(),
        lp_asset_id,
        amount,
        operator.clone(),
      )
      .expect("benchmark native LP lock must succeed");
      eligible_operators.push(operator);
    }
    #[block]
    {
      Pallet::<T>::open_native_security_epoch(7, &eligible_operators)
        .expect("benchmark security snapshot must open atomically");
    }
    let snapshot = NativeSecurityEpochSnapshots::<T>::get(7)
      .expect("benchmark planned security snapshot must exist");
    assert_eq!(snapshot.epoch, 7);
    assert_eq!(snapshot.participants.len(), participant_count as usize);
    assert_eq!(
      snapshot.eligible_operators.len(),
      participant_count as usize
    );
    assert_eq!(
      NativeSecurityRewardPots::<T>::get(7)
        .expect("benchmark planned security pot must exist")
        .status,
      NativeSecurityRewardPotStatus::Planned,
    );
  }

  #[cfg(test)]
  use crate::mock::{Test, new_test_ext};
  #[cfg(test)]
  impl_benchmark_test_suite!(Pallet, new_test_ext(), Test);
}
