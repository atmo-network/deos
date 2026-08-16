use crate as pallet_deos_actors;
use frame::prelude::*;
use polkadot_sdk::{
  frame_support::{
    PalletId, construct_runtime,
    traits::{ConstU8, ConstU32, ConstU64, ConstU128, Get},
  },
  frame_system::EnsureRoot,
  sp_runtime::{
    BuildStorage, Perbill,
    traits::{BlakeTwo256, IdentityLookup},
  },
};

use alloc::vec;
use core::cell::RefCell;

use crate::{
  ActorType, AssetOps, DexOps, DexSwapOutcome, ExecutionContext, FeeCollector, LiquidityOps,
  StakingOps, TaskFailure,
};

type Block = polkadot_sdk::frame_system::mocking::MockBlock<Test>;
pub type AccountId = u64;
pub type Balance = u128;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CHARLIE: AccountId = 3;

#[derive(
  Clone,
  Copy,
  Debug,
  Default,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  Ord,
  PartialEq,
  PartialOrd,
  TypeInfo,
  MaxEncodedLen,
  serde::Serialize,
  serde::Deserialize,
)]
pub enum TestAsset {
  #[default]
  Native,
  Local(u32),
}

construct_runtime!(
  pub enum Test {
    System: polkadot_sdk::frame_system,
    Balances: polkadot_sdk::pallet_balances,
    Actors: pallet_deos_actors,
  }
);

impl polkadot_sdk::frame_system::Config for Test {
  type BaseCallFilter = polkadot_sdk::frame_support::traits::Everything;
  type BlockWeights = ();
  type BlockLength = ();
  type DbWeight = ();
  type RuntimeOrigin = RuntimeOrigin;
  type RuntimeCall = RuntimeCall;
  type Nonce = u64;
  type Hash = polkadot_sdk::sp_core::H256;
  type Hashing = BlakeTwo256;
  type AccountId = AccountId;
  type Lookup = IdentityLookup<Self::AccountId>;
  type Block = Block;
  type RuntimeEvent = RuntimeEvent;
  type BlockHashCount = polkadot_sdk::frame_support::traits::ConstU64<250>;
  type Version = ();
  type PalletInfo = PalletInfo;
  type AccountData = polkadot_sdk::pallet_balances::AccountData<Balance>;
  type OnNewAccount = ();
  type OnKilledAccount = ();
  type SystemWeightInfo = ();
  type SS58Prefix = ();
  type OnSetCode = ();
  type MaxConsumers = ConstU32<16>;
  type RuntimeTask = ();
  type ExtensionsWeightInfo = ();
  type SingleBlockMigrations = ();
  type MultiBlockMigrator = ();
  type PreInherents = ();
  type PostInherents = ();
  type PostTransactions = ();
}

impl polkadot_sdk::pallet_balances::Config for Test {
  type MaxLocks = ConstU32<50>;
  type MaxReserves = ();
  type ReserveIdentifier = [u8; 8];
  type Balance = Balance;
  type RuntimeEvent = RuntimeEvent;
  type DustRemoval = ();
  type ExistentialDeposit = ConstU128<1>;
  type AccountStore = System;
  type WeightInfo = ();
  type FreezeIdentifier = ();
  type MaxFreezes = ();
  type RuntimeHoldReason = RuntimeHoldReason;
  type RuntimeFreezeReason = RuntimeFreezeReason;
  type DoneSlashHandler = ();
}

pub struct ActorsPalletId;
impl Get<PalletId> for ActorsPalletId {
  fn get() -> PalletId {
    PalletId(*b"actors00")
  }
}

pub struct NativeAsset;
impl Get<TestAsset> for NativeAsset {
  fn get() -> TestAsset {
    TestAsset::Native
  }
}

thread_local! {
  static ASSET_BALANCES: RefCell<alloc::collections::BTreeMap<(AccountId, TestAsset), Balance>> =
    RefCell::new(alloc::collections::BTreeMap::new());

  static BURNED: RefCell<alloc::collections::BTreeMap<TestAsset, Balance>> =
    RefCell::new(alloc::collections::BTreeMap::new());

  static MINTED: RefCell<alloc::collections::BTreeMap<TestAsset, Balance>> =
    RefCell::new(alloc::collections::BTreeMap::new());

  static POOL_RESERVES: RefCell<alloc::collections::BTreeMap<(TestAsset, TestAsset), (Balance, Balance)>> =
    RefCell::new(alloc::collections::BTreeMap::new());

  static STAKED: RefCell<alloc::collections::BTreeMap<(AccountId, TestAsset), Balance>> =
    RefCell::new(alloc::collections::BTreeMap::new());

  static STAKING_SHARE_BALANCE_READS: RefCell<u32> = const { RefCell::new(0) };

  static UNSTAKED: RefCell<alloc::collections::BTreeMap<(AccountId, TestAsset), Balance>> =
    RefCell::new(alloc::collections::BTreeMap::new());

  static DONATED_LIQUIDITY: RefCell<alloc::collections::BTreeMap<(AccountId, TestAsset, TestAsset), (Balance, Balance)>> =
    RefCell::new(alloc::collections::BTreeMap::new());

  static GUARANTEED_ON_IDLE_WEIGHT: RefCell<polkadot_sdk::sp_weights::Weight> =
    RefCell::new(polkadot_sdk::sp_weights::Weight::MAX);
  static FEE_COLLECTIONS: RefCell<alloc::vec::Vec<Balance>> = RefCell::new(alloc::vec::Vec::new());
  static FAIL_CREATE_CHECKPOINT: RefCell<bool> = RefCell::new(false);
  static FAIL_FEE_SINK_TRANSFER: RefCell<bool> = RefCell::new(false);
  static FAIL_TRANSFER_TO: RefCell<Option<AccountId>> = RefCell::new(None);
  static CORRUPT_QUEUE_AFTER_TRANSFER: RefCell<bool> = RefCell::new(false);
  static ASSET_MINIMUM_BALANCE: RefCell<Balance> = RefCell::new(1);
  static OBSERVATIONS: RefCell<
    alloc::collections::BTreeMap<u32, crate::ScalarObservationState<u64>>,
  > = RefCell::new(alloc::collections::BTreeMap::new());
  static FAIL_DEX_AFTER_INPUT_TRANSFER: RefCell<bool> = RefCell::new(false);
  static TEMPORARY_DEX_FAILURE: RefCell<bool> = RefCell::new(false);
  static TEMPORARY_ADD_LIQUIDITY_FAILURE: RefCell<bool> = RefCell::new(false);
  static LAST_DEX_ACTORS_TYPE: RefCell<Option<ActorType>> = RefCell::new(None);
  static MAX_CONSECUTIVE_FAILURES: RefCell<u32> = RefCell::new(3);
  static FAIL_STAKING_OPS: RefCell<bool> = RefCell::new(false);
  static FAIL_STAKING_AFTER_BURN: RefCell<bool> = RefCell::new(false);
  static STAKING_SHARE_ASSET_AVAILABLE: RefCell<bool> = RefCell::new(true);
  static FAIL_LIQUIDITY_DONATION_OPS: RefCell<bool> = RefCell::new(false);
  static FAIL_LIQUIDITY_DONATION_AFTER_FIRST_BURN: RefCell<bool> = RefCell::new(false);
  static LP_PAIR_BY_TOKEN: RefCell<alloc::collections::BTreeMap<TestAsset, (TestAsset, TestAsset)>> =
    RefCell::new(alloc::collections::BTreeMap::new());
  static RESERVED_SOVEREIGN_ACCOUNTS: RefCell<alloc::collections::BTreeSet<AccountId>> =
    RefCell::new(alloc::collections::BTreeSet::new());
  #[cfg(feature = "runtime-benchmarks")]
  static BENCHMARK_INGRESS: RefCell<Option<(AccountId, AccountId, Balance)>> = RefCell::new(None);
  #[cfg(feature = "runtime-benchmarks")]
  static BENCHMARK_ASSET_OPS_INGRESS: RefCell<bool> = RefCell::new(false);
}

pub fn set_corrupt_queue_after_transfer(enabled: bool) {
  CORRUPT_QUEUE_AFTER_TRANSFER.with(|value| *value.borrow_mut() = enabled);
}

pub fn set_asset_minimum_balance(amount: Balance) {
  ASSET_MINIMUM_BALANCE.with(|value| *value.borrow_mut() = amount);
}

pub fn set_reserved_sovereign_account(account: AccountId) {
  RESERVED_SOVEREIGN_ACCOUNTS.with(|set| {
    set.borrow_mut().insert(account);
  });
}
pub fn set_observation(feed: u32, state: crate::ScalarObservationState<u64>) {
  OBSERVATIONS.with(|values| {
    values.borrow_mut().insert(feed, state);
  });
}

pub fn set_pool_reserves(
  asset_a: TestAsset,
  asset_b: TestAsset,
  reserve_a: Balance,
  reserve_b: Balance,
) {
  let key = if asset_a <= asset_b {
    (asset_a, asset_b)
  } else {
    (asset_b, asset_a)
  };
  let (ra, rb) = if asset_a <= asset_b {
    (reserve_a, reserve_b)
  } else {
    (reserve_b, reserve_a)
  };
  POOL_RESERVES.with(|p| p.borrow_mut().insert(key, (ra, rb)));
}

pub fn register_lp_pair(lp_asset: TestAsset, asset_a: TestAsset, asset_b: TestAsset) {
  LP_PAIR_BY_TOKEN.with(|pairs| pairs.borrow_mut().insert(lp_asset, (asset_a, asset_b)));
}

pub fn reset_mock_adapters() {
  ASSET_BALANCES.with(|b| b.borrow_mut().clear());
  BURNED.with(|b| b.borrow_mut().clear());
  MINTED.with(|b| b.borrow_mut().clear());
  POOL_RESERVES.with(|b| b.borrow_mut().clear());
  STAKED.with(|b| b.borrow_mut().clear());
  STAKING_SHARE_BALANCE_READS.with(|reads| *reads.borrow_mut() = 0);
  UNSTAKED.with(|b| b.borrow_mut().clear());
  DONATED_LIQUIDITY.with(|b| b.borrow_mut().clear());
  GUARANTEED_ON_IDLE_WEIGHT.with(|v| *v.borrow_mut() = polkadot_sdk::sp_weights::Weight::MAX);
  FEE_COLLECTIONS.with(|v| v.borrow_mut().clear());
  FAIL_CREATE_CHECKPOINT.with(|v| *v.borrow_mut() = false);
  FAIL_FEE_SINK_TRANSFER.with(|v| *v.borrow_mut() = false);
  FAIL_TRANSFER_TO.with(|v| *v.borrow_mut() = None);
  CORRUPT_QUEUE_AFTER_TRANSFER.with(|v| *v.borrow_mut() = false);
  ASSET_MINIMUM_BALANCE.with(|v| *v.borrow_mut() = 1);
  OBSERVATIONS.with(|values| values.borrow_mut().clear());
  FAIL_DEX_AFTER_INPUT_TRANSFER.with(|v| *v.borrow_mut() = false);
  TEMPORARY_DEX_FAILURE.with(|v| *v.borrow_mut() = false);
  TEMPORARY_ADD_LIQUIDITY_FAILURE.with(|v| *v.borrow_mut() = false);
  LAST_DEX_ACTORS_TYPE.with(|value| *value.borrow_mut() = None);
  MAX_CONSECUTIVE_FAILURES.with(|v| *v.borrow_mut() = 3);
  FAIL_STAKING_OPS.with(|v| *v.borrow_mut() = false);
  FAIL_STAKING_AFTER_BURN.with(|v| *v.borrow_mut() = false);
  STAKING_SHARE_ASSET_AVAILABLE.with(|v| *v.borrow_mut() = true);
  FAIL_LIQUIDITY_DONATION_OPS.with(|v| *v.borrow_mut() = false);
  FAIL_LIQUIDITY_DONATION_AFTER_FIRST_BURN.with(|v| *v.borrow_mut() = false);
  #[cfg(feature = "runtime-benchmarks")]
  {
    BENCHMARK_INGRESS.with(|event| *event.borrow_mut() = None);
    BENCHMARK_ASSET_OPS_INGRESS.with(|enabled| *enabled.borrow_mut() = false);
  }
}

pub fn staking_share_balance_reads() -> u32 {
  STAKING_SHARE_BALANCE_READS.with(|reads| *reads.borrow())
}

pub struct MockObservationProvider;
impl crate::ObservationProvider<u32, u64> for MockObservationProvider {
  fn observe(feed: &u32, _: u64, _: u32) -> crate::ScalarObservationState<u64> {
    OBSERVATIONS.with(|values| {
      values
        .borrow()
        .get(feed)
        .copied()
        .unwrap_or(crate::ScalarObservationState::Unavailable)
    })
  }
}

pub struct MockFeeCollector;
impl FeeCollector<AccountId, TestAsset, Balance> for MockFeeCollector {
  fn collect_fee(
    payer: &AccountId,
    fee_sink: &AccountId,
    native_asset: TestAsset,
    amount: Balance,
  ) -> DispatchResult {
    FEE_COLLECTIONS.with(|collections| collections.borrow_mut().push(amount));
    MockAssetOps::transfer(payer, fee_sink, native_asset, amount).map_err(|failure| failure.error)
  }
}

pub struct MockAssetOps;

impl AssetOps<AccountId, TestAsset, Balance> for MockAssetOps {
  fn transfer(
    from: &AccountId,
    to: &AccountId,
    asset: TestAsset,
    amount: Balance,
  ) -> Result<(), TaskFailure> {
    if FAIL_TRANSFER_TO.with(|target| *target.borrow() == Some(*to)) {
      return Err(DispatchError::Other("MockTransferTargetFailed").into());
    }
    match asset {
      TestAsset::Native => {
        if *to == TestFeeSink::get() && FAIL_FEE_SINK_TRANSFER.with(|v| *v.borrow()) {
          return Err(DispatchError::Other("MockFeeSinkTransferFailed").into());
        }
        use polkadot_sdk::frame_support::traits::Currency;
        <Balances as Currency<AccountId>>::transfer(
          from,
          to,
          amount,
          polkadot_sdk::frame_support::traits::ExistenceRequirement::AllowDeath,
        )?;
      }
      _ => ASSET_BALANCES.with(|b| {
        let mut map = b.borrow_mut();
        let src = map.get(&(*from, asset)).copied().unwrap_or(0);
        if src < amount {
          return Err(DispatchError::Token(
            polkadot_sdk::sp_runtime::TokenError::FundsUnavailable,
          ));
        }
        map.insert((*from, asset), src - amount);
        let dst = map.get(&(*to, asset)).copied().unwrap_or(0);
        map.insert((*to, asset), dst + amount);
        Ok(())
      })?,
    }
    if CORRUPT_QUEUE_AFTER_TRANSFER.with(|enabled| {
      let corrupt = *enabled.borrow();
      *enabled.borrow_mut() = false;
      corrupt
    }) {
      crate::QueueTail::<Test>::mutate(|tail| *tail = tail.saturating_add(1));
    }
    #[cfg(feature = "runtime-benchmarks")]
    if BENCHMARK_ASSET_OPS_INGRESS.with(|enabled| *enabled.borrow()) {
      if let Some(actor_id) = crate::SovereignIndex::<Test>::get(to) {
        crate::Pallet::<Test>::notify_address_event(actor_id, asset, amount, from)?;
      }
    }
    Ok(())
  }

  fn burn(who: &AccountId, asset: TestAsset, amount: Balance) -> Result<(), TaskFailure> {
    match asset {
      TestAsset::Native => {
        use polkadot_sdk::frame_support::traits::Currency;
        let (_, remainder) = <Balances as Currency<AccountId>>::slash(who, amount);
        if remainder > 0 {
          return Err(
            DispatchError::Token(polkadot_sdk::sp_runtime::TokenError::FundsUnavailable).into(),
          );
        }
        Ok(())
      }
      _ => ASSET_BALANCES
        .with(|b| -> Result<(), DispatchError> {
          let mut map = b.borrow_mut();
          let bal = map.get(&(*who, asset)).copied().unwrap_or(0);
          if bal < amount {
            return Err(DispatchError::Token(
              polkadot_sdk::sp_runtime::TokenError::FundsUnavailable,
            ));
          }
          map.insert((*who, asset), bal - amount);
          BURNED.with(|br| {
            let mut bm = br.borrow_mut();
            let prev = bm.get(&asset).copied().unwrap_or(0);
            bm.insert(asset, prev + amount);
          });
          Ok(())
        })
        .map_err(TaskFailure::from),
    }
  }

  fn mint(to: &AccountId, asset: TestAsset, amount: Balance) -> Result<(), TaskFailure> {
    match asset {
      TestAsset::Native => {
        use polkadot_sdk::frame_support::traits::Currency;
        let _ = <Balances as Currency<AccountId>>::deposit_creating(to, amount);
      }
      _ => ASSET_BALANCES.with(|b| {
        let mut map = b.borrow_mut();
        let bal = map.get(&(*to, asset)).copied().unwrap_or(0);
        map.insert((*to, asset), bal + amount);
        MINTED.with(|m| {
          let mut mm = m.borrow_mut();
          let prev = mm.get(&asset).copied().unwrap_or(0);
          mm.insert(asset, prev + amount);
        });
      }),
    }
    #[cfg(feature = "runtime-benchmarks")]
    if BENCHMARK_ASSET_OPS_INGRESS.with(|enabled| *enabled.borrow())
      && let Some(actor_id) = crate::SovereignIndex::<Test>::get(to)
    {
      crate::Pallet::<Test>::notify_address_event_without_source(actor_id, asset, amount)?;
    }
    Ok(())
  }

  fn balance(who: &AccountId, asset: TestAsset) -> Balance {
    match asset {
      TestAsset::Native => {
        use polkadot_sdk::frame_support::traits::{
          fungible::Inspect as NativeInspect,
          tokens::{Fortitude, Preservation},
        };
        <Balances as NativeInspect<AccountId>>::reducible_balance(
          who,
          Preservation::Expendable,
          Fortitude::Polite,
        )
      }
      _ => ASSET_BALANCES.with(|b| b.borrow().get(&(*who, asset)).copied().unwrap_or(0)),
    }
  }

  fn minimum_balance(_asset: TestAsset) -> Balance {
    ASSET_MINIMUM_BALANCE.with(|value| *value.borrow())
  }

  fn preflight_transfer(
    from: &AccountId,
    to: &AccountId,
    asset: TestAsset,
    amount: Balance,
  ) -> Result<(), TaskFailure> {
    if amount == 0 {
      return Ok(());
    }
    if asset == TestAsset::Native {
      use polkadot_sdk::frame_support::traits::{
        fungible::Inspect as NativeInspect, tokens::Provenance,
      };
      <Balances as NativeInspect<AccountId>>::can_withdraw(from, amount)
        .into_result(false)
        .map_err(TaskFailure::permanent)?;
      return <Balances as NativeInspect<AccountId>>::can_deposit(to, amount, Provenance::Extant)
        .into_result()
        .map_err(|_| TaskFailure::temporary(crate::Error::<Test>::RecipientDepositUnavailable));
    }
    let source = ASSET_BALANCES.with(|b| b.borrow().get(&(*from, asset)).copied().unwrap_or(0));
    if source < amount {
      return Err(TaskFailure::permanent(DispatchError::Token(
        polkadot_sdk::sp_runtime::TokenError::FundsUnavailable,
      )));
    }
    let recipient = ASSET_BALANCES.with(|b| b.borrow().get(&(*to, asset)).copied().unwrap_or(0));
    if recipient == 0 && amount < Self::minimum_balance(asset) {
      return Err(TaskFailure::temporary(
        crate::Error::<Test>::RecipientDepositUnavailable,
      ));
    }
    Ok(())
  }
}

pub fn staked_balance(who: AccountId, asset: TestAsset) -> Balance {
  STAKED.with(|s| s.borrow().get(&(who, asset)).copied().unwrap_or(0))
}

pub fn unstaked_shares(who: AccountId, asset: TestAsset) -> Balance {
  UNSTAKED.with(|s| s.borrow().get(&(who, asset)).copied().unwrap_or(0))
}

pub fn donated_liquidity(
  who: AccountId,
  asset_a: TestAsset,
  asset_b: TestAsset,
) -> (Balance, Balance) {
  DONATED_LIQUIDITY.with(|d| {
    d.borrow()
      .get(&(who, asset_a, asset_b))
      .copied()
      .unwrap_or((0, 0))
  })
}

pub fn set_fail_transfer_to(target: Option<AccountId>) {
  FAIL_TRANSFER_TO.with(|value| *value.borrow_mut() = target);
}

pub fn set_fail_dex_after_input_transfer(value: bool) {
  FAIL_DEX_AFTER_INPUT_TRANSFER.with(|v| *v.borrow_mut() = value);
}

pub fn set_temporary_dex_failure(value: bool) {
  TEMPORARY_DEX_FAILURE.with(|v| *v.borrow_mut() = value);
}

pub fn set_temporary_add_liquidity_failure(value: bool) {
  TEMPORARY_ADD_LIQUIDITY_FAILURE.with(|v| *v.borrow_mut() = value);
}

pub fn last_dex_actor_type() -> Option<ActorType> {
  LAST_DEX_ACTORS_TYPE.with(|value| *value.borrow())
}

pub fn set_max_consecutive_failures(value: u32) {
  MAX_CONSECUTIVE_FAILURES.with(|maximum| *maximum.borrow_mut() = value);
}

pub fn set_fail_staking_ops(value: bool) {
  FAIL_STAKING_OPS.with(|v| *v.borrow_mut() = value);
}

pub fn set_fail_staking_after_burn(value: bool) {
  FAIL_STAKING_AFTER_BURN.with(|v| *v.borrow_mut() = value);
}

pub fn set_staking_share_asset_available(value: bool) {
  STAKING_SHARE_ASSET_AVAILABLE.with(|v| *v.borrow_mut() = value);
}

pub fn set_fail_liquidity_donation_ops(value: bool) {
  FAIL_LIQUIDITY_DONATION_OPS.with(|v| *v.borrow_mut() = value);
}

pub fn set_fail_liquidity_donation_after_first_burn(value: bool) {
  FAIL_LIQUIDITY_DONATION_AFTER_FIRST_BURN.with(|v| *v.borrow_mut() = value);
}

pub struct MockDexOps;

impl DexOps<AccountId, TestAsset, Balance> for MockDexOps {
  fn swap_exact_in(
    context: ExecutionContext<'_, AccountId>,
    asset_in: TestAsset,
    asset_out: TestAsset,
    amount_in: Balance,
    slippage_tolerance: Perbill,
  ) -> Result<DexSwapOutcome<Balance>, TaskFailure> {
    let who = context.actor;
    LAST_DEX_ACTORS_TYPE.with(|value| *value.borrow_mut() = Some(context.actor_type));
    let (ri, ro) = Self::get_reserves(asset_in, asset_out)?;
    let amount_out = amount_in.saturating_mul(ro) / (ri.saturating_add(amount_in));
    let quote = amount_in.saturating_mul(ro) / ri.saturating_add(amount_in);
    let min_out = (Perbill::one() - slippage_tolerance).mul_floor(quote);
    if amount_out < min_out {
      return Err(DispatchError::Other("SlippageExceeded").into());
    }
    MockAssetOps::transfer(who, &u64::MAX, asset_in, amount_in)?;
    if TEMPORARY_DEX_FAILURE.with(|v| *v.borrow()) {
      return Err(TaskFailure::temporary(DispatchError::Other(
        "TemporaryDexCapacity",
      )));
    }
    if FAIL_DEX_AFTER_INPUT_TRANSFER.with(|v| *v.borrow()) {
      return Err(DispatchError::Other("MockDexAfterInputTransferFailed").into());
    }
    MockAssetOps::transfer(&u64::MAX, who, asset_out, amount_out)?;
    Ok(DexSwapOutcome {
      total_amount_in: amount_in,
      recipient_amount_out: amount_out,
    })
  }

  fn swap_exact_out(
    context: ExecutionContext<'_, AccountId>,
    asset_in: TestAsset,
    asset_out: TestAsset,
    amount_out: Balance,
    max_amount_in: Balance,
    slippage_tolerance: Perbill,
  ) -> Result<DexSwapOutcome<Balance>, TaskFailure> {
    let who = context.actor;
    LAST_DEX_ACTORS_TYPE.with(|value| *value.borrow_mut() = Some(context.actor_type));
    let (ri, ro) = Self::get_reserves(asset_in, asset_out)?;
    if amount_out >= ro {
      return Err(DispatchError::Other("InsufficientPoolLiquidity").into());
    }
    let numerator = ri.saturating_mul(amount_out);
    let denominator = ro.saturating_sub(amount_out);
    let amount_in = numerator
      .checked_div(denominator)
      .ok_or(DispatchError::Other("DivisionByZero"))?
      .saturating_add(1);
    let quoted_max_in = amount_in.saturating_add(slippage_tolerance.mul_ceil(amount_in));
    if quoted_max_in > max_amount_in {
      return Err(DispatchError::Other("ExactOutInputCapacityExceeded").into());
    }
    MockAssetOps::transfer(who, &u64::MAX, asset_in, amount_in)?;
    if TEMPORARY_DEX_FAILURE.with(|v| *v.borrow()) {
      return Err(TaskFailure::temporary(DispatchError::Other(
        "TemporaryDexCapacity",
      )));
    }
    if FAIL_DEX_AFTER_INPUT_TRANSFER.with(|v| *v.borrow()) {
      return Err(DispatchError::Other("MockDexAfterInputTransferFailed").into());
    }
    MockAssetOps::transfer(&u64::MAX, who, asset_out, amount_out)?;
    Ok(DexSwapOutcome {
      total_amount_in: amount_in,
      recipient_amount_out: amount_out,
    })
  }
}

impl MockDexOps {
  fn get_reserves(
    asset_in: TestAsset,
    asset_out: TestAsset,
  ) -> Result<(Balance, Balance), DispatchError> {
    let key = if asset_in <= asset_out {
      (asset_in, asset_out)
    } else {
      (asset_out, asset_in)
    };
    POOL_RESERVES.with(|p| {
      let map = p.borrow();
      let (ra, rb) = map
        .get(&key)
        .copied()
        .ok_or(DispatchError::Other("NoPool"))?;
      if asset_in <= asset_out {
        Ok((ra, rb))
      } else {
        Ok((rb, ra))
      }
    })
  }
}

pub struct MockStakingOps;

impl StakingOps<AccountId, TestAsset, Balance> for MockStakingOps {
  fn stake(who: &AccountId, asset: TestAsset, amount: Balance) -> Result<(), TaskFailure> {
    if FAIL_STAKING_OPS.with(|v| *v.borrow()) {
      return Err(DispatchError::Other("MockStakingOpsFailed").into());
    }
    MockAssetOps::burn(who, asset, amount)?;
    if FAIL_STAKING_AFTER_BURN.with(|v| *v.borrow()) {
      return Err(DispatchError::Other("MockStakingAfterBurnFailed").into());
    }
    STAKED.with(|s| {
      let mut map = s.borrow_mut();
      let current = map.get(&(*who, asset)).copied().unwrap_or(0);
      map.insert((*who, asset), current.saturating_add(amount));
    });
    Ok(())
  }

  fn unstake(who: &AccountId, asset: TestAsset, shares: Balance) -> Result<(), TaskFailure> {
    if FAIL_STAKING_OPS.with(|v| *v.borrow()) {
      return Err(DispatchError::Other("MockStakingOpsFailed").into());
    }
    MockAssetOps::burn(who, asset, shares)?;
    if FAIL_STAKING_AFTER_BURN.with(|v| *v.borrow()) {
      return Err(DispatchError::Other("MockStakingAfterBurnFailed").into());
    }
    UNSTAKED.with(|s| {
      let mut map = s.borrow_mut();
      let current = map.get(&(*who, asset)).copied().unwrap_or(0);
      map.insert((*who, asset), current.saturating_add(shares));
    });
    Ok(())
  }

  fn share_balance(who: &AccountId, asset: TestAsset) -> Balance {
    STAKING_SHARE_BALANCE_READS.with(|reads| {
      let next = reads.borrow().saturating_add(1);
      *reads.borrow_mut() = next;
    });
    MockAssetOps::balance(who, asset)
  }

  fn share_asset(asset: TestAsset) -> Option<TestAsset> {
    if !STAKING_SHARE_ASSET_AVAILABLE.with(|value| *value.borrow())
      || asset == TestAsset::Local(u32::MAX)
    {
      None
    } else {
      Some(asset)
    }
  }
}

pub struct MockLiquidityOps;

impl LiquidityOps<AccountId, TestAsset, Balance> for MockLiquidityOps {
  fn lp_assets(lp_asset: TestAsset) -> Option<(TestAsset, TestAsset)> {
    LP_PAIR_BY_TOKEN.with(|pairs| pairs.borrow().get(&lp_asset).copied())
  }

  fn add_liquidity(
    _who: &AccountId,
    _asset_a: TestAsset,
    _asset_b: TestAsset,
    amount_a: Balance,
    amount_b: Balance,
    min_lp_out: Balance,
  ) -> Result<(Balance, Balance, Balance), TaskFailure> {
    if TEMPORARY_ADD_LIQUIDITY_FAILURE.with(|v| *v.borrow()) {
      return Err(TaskFailure::temporary(DispatchError::Other(
        "TemporaryAddLiquidityCapacity",
      )));
    }
    let lp_minted = integer_sqrt(amount_a.saturating_mul(amount_b));
    if lp_minted < min_lp_out {
      return Err(DispatchError::Other("MinimumLpOutputNotMet").into());
    }
    Ok((amount_a, amount_b, lp_minted))
  }

  fn remove_liquidity(
    _who: &AccountId,
    lp_asset: TestAsset,
    asset_a: TestAsset,
    asset_b: TestAsset,
    lp_amount: Balance,
    min_amount_a: Balance,
    min_amount_b: Balance,
  ) -> Result<(Balance, Balance), TaskFailure> {
    // Bind the ordered pair for the LP token so the event exposes it and later
    // steps cannot reinterpret the admitted binding.
    LP_PAIR_BY_TOKEN.with(|pairs| pairs.borrow_mut().insert(lp_asset, (asset_a, asset_b)));
    let half = lp_amount / 2;
    if half < min_amount_a || half < min_amount_b {
      return Err(DispatchError::Other("MinimumLiquidityOutputNotMet").into());
    }
    Ok((half, half))
  }

  fn donate_liquidity(
    who: &AccountId,
    asset_a: TestAsset,
    asset_b: TestAsset,
    max_amount_a: Balance,
    max_amount_b: Balance,
    _max_ratio_error: Perbill,
  ) -> Result<(Balance, Balance), TaskFailure> {
    if FAIL_LIQUIDITY_DONATION_OPS.with(|v| *v.borrow()) {
      return Err(DispatchError::Other("MockLiquidityOpsFailed").into());
    }
    let amount = max_amount_a.min(max_amount_b);
    if MockAssetOps::balance(who, asset_a) < amount || MockAssetOps::balance(who, asset_b) < amount
    {
      return Err(
        DispatchError::Token(polkadot_sdk::sp_runtime::TokenError::FundsUnavailable).into(),
      );
    }
    MockAssetOps::burn(who, asset_a, amount)?;
    if FAIL_LIQUIDITY_DONATION_AFTER_FIRST_BURN.with(|v| *v.borrow()) {
      return Err(DispatchError::Other("MockLiquidityDonationAfterFirstBurnFailed").into());
    }
    MockAssetOps::burn(who, asset_b, amount)?;
    DONATED_LIQUIDITY.with(|d| {
      let mut map = d.borrow_mut();
      let (current_a, current_b) = map
        .get(&(*who, asset_a, asset_b))
        .copied()
        .unwrap_or((0, 0));
      map.insert(
        (*who, asset_a, asset_b),
        (
          current_a.saturating_add(amount),
          current_b.saturating_add(amount),
        ),
      );
    });
    Ok((amount, amount))
  }
}

#[cfg(feature = "runtime-benchmarks")]
pub struct MockBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl crate::BenchmarkHelper<AccountId, TestAsset, Balance, u32> for MockBenchmarkHelper {
  fn setup_add_liquidity(
    owner: &AccountId,
  ) -> Result<(TestAsset, TestAsset, Balance, Balance), DispatchError> {
    let asset_a = TestAsset::Local(1);
    let asset_b = TestAsset::Local(2);
    let amount = 1_000_000;
    MockAssetOps::mint(owner, asset_a, amount).map_err(|failure| failure.error)?;
    MockAssetOps::mint(owner, asset_b, amount).map_err(|failure| failure.error)?;
    Ok((asset_a, asset_b, amount, amount))
  }

  fn setup_donate_liquidity(
    owner: &AccountId,
  ) -> Result<(TestAsset, TestAsset, Balance), DispatchError> {
    let asset_a = TestAsset::Local(1);
    let asset_b = TestAsset::Local(2);
    let amount = 1_000_000;
    MockAssetOps::mint(owner, asset_a, amount).map_err(|failure| failure.error)?;
    MockAssetOps::mint(owner, asset_b, amount).map_err(|failure| failure.error)?;
    Ok((asset_a, asset_b, amount))
  }

  fn setup_stake(owner: &AccountId) -> Result<(TestAsset, Balance), DispatchError> {
    let asset = TestAsset::Local(1);
    let amount = 1_000_000;
    MockAssetOps::mint(owner, asset, amount).map_err(|failure| failure.error)?;
    Ok((asset, amount))
  }

  fn setup_unstake(owner: &AccountId) -> Result<(TestAsset, Balance), DispatchError> {
    let asset = TestAsset::Local(1);
    let shares = 1_000_000;
    MockAssetOps::mint(owner, asset, shares).map_err(|failure| failure.error)?;
    Ok((asset, shares))
  }

  fn setup_swap_exact_in(
    owner: &AccountId,
  ) -> Result<(TestAsset, TestAsset, Balance), DispatchError> {
    let asset_in = TestAsset::Local(1);
    let asset_out = TestAsset::Local(2);
    let amount_in = 1_000;
    set_pool_reserves(asset_in, asset_out, 1_000_000, 1_000_000);
    MockAssetOps::mint(owner, asset_in, amount_in).map_err(|failure| failure.error)?;
    MockAssetOps::mint(&u64::MAX, asset_out, 1_000_000).map_err(|failure| failure.error)?;
    Ok((asset_in, asset_out, amount_in))
  }

  fn setup_swap_exact_out(
    owner: &AccountId,
  ) -> Result<(TestAsset, TestAsset, Balance, Balance), DispatchError> {
    let asset_in = TestAsset::Local(1);
    let asset_out = TestAsset::Local(2);
    let amount_out = 1_000;
    let max_amount_in = 2_000;
    set_pool_reserves(asset_in, asset_out, 1_000_000, 1_000_000);
    MockAssetOps::mint(owner, asset_in, max_amount_in).map_err(|failure| failure.error)?;
    MockAssetOps::mint(&u64::MAX, asset_out, 1_000_000).map_err(|failure| failure.error)?;
    Ok((asset_in, asset_out, amount_out, max_amount_in))
  }

  fn funding_assets(max: u32) -> alloc::vec::Vec<TestAsset> {
    (0..max)
      .map(|index| {
        if index == 0 {
          TestAsset::Native
        } else {
          TestAsset::Local(index)
        }
      })
      .collect()
  }

  fn setup_predicate_assets(
    _owner: &AccountId,
    max: u32,
  ) -> Result<alloc::vec::Vec<TestAsset>, DispatchError> {
    Ok(Self::funding_assets(max))
  }

  fn setup_observation_feeds(max: u32) -> Result<alloc::vec::Vec<u32>, DispatchError> {
    for feed in 1..=max {
      set_observation(
        feed,
        crate::ScalarObservationState::Fresh {
          value: 1,
          observed_at: frame_system::Pallet::<Test>::block_number(),
        },
      );
    }
    Ok((1..=max).collect())
  }

  fn enable_asset_ops_ingress() {
    BENCHMARK_ASSET_OPS_INGRESS.with(|enabled| *enabled.borrow_mut() = true);
  }

  fn setup_address_event_ingress(
    recipient: &AccountId,
    source: &AccountId,
    amount: Balance,
  ) -> DispatchResult {
    BENCHMARK_INGRESS.with(|event| {
      *event.borrow_mut() = Some((*recipient, *source, amount));
    });
    Ok(())
  }

  fn run_address_event_ingress(
    recipient: &AccountId,
    _source: &AccountId,
    _amount: Balance,
  ) -> bool {
    let event = BENCHMARK_INGRESS.with(|pending| *pending.borrow());
    let Some((event_recipient, source, amount)) = event else {
      return false;
    };
    if event_recipient != *recipient {
      return false;
    }
    let Some(actor_id) = crate::SovereignIndex::<Test>::get(recipient) else {
      return false;
    };
    crate::Pallet::<Test>::notify_address_event(actor_id, TestAsset::Native, amount, &source)
      .expect("mock benchmark ingress must succeed");
    true
  }

  fn setup_xcm_asset_deposit() -> DispatchResult {
    Ok(())
  }

  fn run_xcm_asset_deposit(
    recipient: &AccountId,
    source: &AccountId,
    amount: Balance,
  ) -> DispatchResult {
    MockAssetOps::mint(recipient, TestAsset::Native, amount).map_err(|failure| failure.error)?;
    if let Some(actor_id) = crate::SovereignIndex::<Test>::get(recipient) {
      crate::Pallet::<Test>::notify_xcm_address_event(actor_id, TestAsset::Native, amount, source)?;
    }
    Ok(())
  }

  fn setup_remove_liquidity(
    owner: &AccountId,
  ) -> Result<(TestAsset, TestAsset, TestAsset, Balance), DispatchError> {
    let lp_asset = TestAsset::Local(1);
    let asset_a = TestAsset::Local(2);
    let asset_b = TestAsset::Local(3);
    let lp_amount = 1_000_000u128;
    MockAssetOps::mint(owner, lp_asset, lp_amount).map_err(|failure| failure.error)?;
    LP_PAIR_BY_TOKEN.with(|pairs| pairs.borrow_mut().insert(lp_asset, (asset_a, asset_b)));
    Ok((lp_asset, asset_a, asset_b, lp_amount))
  }
}

fn integer_sqrt(n: u128) -> u128 {
  if n == 0 {
    return 0;
  }
  let mut x = n;
  let mut y = x.div_ceil(2);
  while y < x {
    x = y;
    y = (x + n / x) / 2;
  }
  x
}

pub struct TestWeightToFee;
impl polkadot_sdk::sp_weights::WeightToFee for TestWeightToFee {
  type Balance = Balance;
  fn weight_to_fee(_weight: &polkadot_sdk::sp_weights::Weight) -> Self::Balance {
    100
  }
}

pub struct TestFeeSink;
impl Get<AccountId> for TestFeeSink {
  fn get() -> AccountId {
    999
  }
}

pub struct TestActorCreationFee;
impl Get<Balance> for TestActorCreationFee {
  fn get() -> Balance {
    10
  }
}

pub struct TestMaxExecutionDelayBlocks;
impl Get<u64> for TestMaxExecutionDelayBlocks {
  fn get() -> u64 {
    5_000
  }
}

pub struct TestMaxIdleStarvationBlocks;
impl Get<u32> for TestMaxIdleStarvationBlocks {
  fn get() -> u32 {
    2
  }
}

pub struct TestObservationFanoutWeightLimit;
impl Get<polkadot_sdk::sp_weights::Weight> for TestObservationFanoutWeightLimit {
  fn get() -> polkadot_sdk::sp_weights::Weight {
    polkadot_sdk::sp_weights::Weight::from_parts(1_000_000_000_000, 100_000_000)
  }
}

pub struct TestWakeupWeightLimit;
impl Get<polkadot_sdk::sp_weights::Weight> for TestWakeupWeightLimit {
  fn get() -> polkadot_sdk::sp_weights::Weight {
    polkadot_sdk::sp_weights::Weight::from_parts(1_000_000_000_000, 100_000_000)
  }
}

pub struct TestActorOnIdleReserve;
impl Get<polkadot_sdk::sp_weights::Weight> for TestActorOnIdleReserve {
  fn get() -> polkadot_sdk::sp_weights::Weight {
    GUARANTEED_ON_IDLE_WEIGHT.with(|v| *v.borrow())
  }
}

pub fn set_guaranteed_on_idle_weight(weight: polkadot_sdk::sp_weights::Weight) {
  GUARANTEED_ON_IDLE_WEIGHT.with(|v| *v.borrow_mut() = weight);
}

pub struct TestMaxAutoCloseNonceHorizon;
impl Get<u64> for TestMaxAutoCloseNonceHorizon {
  fn get() -> u64 {
    10_000
  }
}

pub struct TestMaxConsecutiveFailures;
impl Get<u32> for TestMaxConsecutiveFailures {
  fn get() -> u32 {
    MAX_CONSECUTIVE_FAILURES.with(|maximum| *maximum.borrow())
  }
}

pub struct TestMinUserBalance;
impl Get<Balance> for TestMinUserBalance {
  fn get() -> Balance {
    50
  }
}

pub struct TestMaxSweepBatch;
impl Get<u32> for TestMaxSweepBatch {
  fn get() -> u32 {
    3
  }
}

pub struct MockFundingAuthority;

impl crate::adapters::FundingAuthority<AccountId> for MockFundingAuthority {
  fn permits(
    _: crate::ActorId,
    _: &AccountId,
    _: Option<&AccountId>,
    _: Option<&crate::FundingProvenance>,
  ) -> bool {
    true
  }
}

pub struct MockSovereignAccountPolicy;

impl crate::adapters::SovereignAccountPolicy<AccountId> for MockSovereignAccountPolicy {
  fn is_reserved(account: &AccountId) -> bool {
    RESERVED_SOVEREIGN_ACCOUNTS.with(|set| set.borrow().contains(account))
  }
}

impl pallet_deos_actors::Config for Test {
  type AssetId = TestAsset;
  type Balance = Balance;
  type FeeNativeAssetId = NativeAsset;
  type AssetOps = MockAssetOps;
  type ObservationFeedId = u32;
  type ObservationProvider = MockObservationProvider;
  type FundingAuthority = MockFundingAuthority;
  type SovereignAccountPolicy = MockSovereignAccountPolicy;
  type DexOps = MockDexOps;
  type StakingOps = MockStakingOps;
  type LiquidityOps = MockLiquidityOps;
  type MinWindowLength = frame::traits::ConstU64<100>;
  type PalletId = ActorsPalletId;
  type SystemOrigin = EnsureRoot<AccountId>;
  type GlobalBreakerOrigin = EnsureRoot<AccountId>;
  type MaxContractSteps = ConstU32<8>;
  type MaxFundingTrackedAssets = ConstU32<10>;
  type MaxOpeningSnapshotEntries = ConstU32<16>;
  type MaxOpeningPredicateResults = ConstU32<32>;
  type MaxPreconditionClauses = ConstU32<4>;
  type MaxPredicatesPerClause = ConstU32<4>;
  type MaxPredicatesPerStep = ConstU32<4>;
  type MaxOwnerSlots = ConstU8<255>;
  type MaxExecutionsPerBlock = ConstU32<3>;
  type MaxQueueLength = ConstU32<1024>;
  type QueuePageSize = ConstU32<32>;
  type WakeupPageSize = ConstU32<32>;
  type ObservationPageSize = ConstU32<16>;
  type MaxQueueEntriesScannedPerBlock = ConstU32<1024>;
  type MaxObservationFanoutPagesPerBlock = ConstU32<64>;
  type ObservationFanoutWeightLimit = TestObservationFanoutWeightLimit;
  type WakeupWeightLimit = TestWakeupWeightLimit;
  type MaxWakeupsPerBlock = ConstU32<64>;
  type MaxSweepBatch = TestMaxSweepBatch;
  type MaxWhitelistSize = ConstU32<16>;
  type MaxTriggerSources = ConstU32<4>;
  type MaxSplitTransferLegs = ConstU32<8>;
  type TargetBlockTime = ConstU64<63_116>;
  type MaxExecutionDelayBlocks = TestMaxExecutionDelayBlocks;
  type MaxIdleStarvationBlocks = TestMaxIdleStarvationBlocks;
  type ActorOnIdleReserve = TestActorOnIdleReserve;
  type MaxAutoCloseNonceHorizon = TestMaxAutoCloseNonceHorizon;
  type MaxActiveActors = ConstU32<10_000>;
  type MaxActorIdentities = ConstU32<10_000>;
  type MaxSystemSovereigns = ConstU32<10_000>;
  type ActorCreationFee = TestActorCreationFee;
  type WeightToFee = TestWeightToFee;
  type FeeSink = TestFeeSink;
  type FeeCollector = MockFeeCollector;
  type MaxConsecutiveFailures = TestMaxConsecutiveFailures;
  type MaxRetryAttempts = ConstU32<10>;
  type MinUserBalance = TestMinUserBalance;
  type WeightInfo = crate::weights::TestWeightInfo;
  type GenesisSystemActors = ();
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = MockBenchmarkHelper;
}

pub const TEST_INITIAL_BALANCE: Balance = 10_000_000_000_000;

pub fn new_test_ext() -> polkadot_sdk::sp_io::TestExternalities {
  let mut t = polkadot_sdk::frame_system::GenesisConfig::<Test>::default()
    .build_storage()
    .unwrap();

  polkadot_sdk::pallet_balances::GenesisConfig::<Test> {
    balances: vec![
      (ALICE, TEST_INITIAL_BALANCE),
      (BOB, TEST_INITIAL_BALANCE),
      (CHARLIE, TEST_INITIAL_BALANCE),
      (0, TEST_INITIAL_BALANCE),
      (255, TEST_INITIAL_BALANCE),
      (999, 1), // FeeSink ED
    ],
    dev_accounts: None,
  }
  .assimilate_storage(&mut t)
  .unwrap();

  crate::GenesisConfig::<Test>::default()
    .assimilate_storage(&mut t)
    .unwrap();

  let mut ext = polkadot_sdk::sp_io::TestExternalities::new(t);
  ext.execute_with(|| {
    reset_mock_adapters();
  });
  ext
}

pub fn set_fail_create_checkpoint(value: bool) {
  FAIL_CREATE_CHECKPOINT.with(|v| *v.borrow_mut() = value);
}

pub fn fee_collections() -> alloc::vec::Vec<Balance> {
  FEE_COLLECTIONS.with(|collections| collections.borrow().clone())
}

pub fn clear_fee_collections() {
  FEE_COLLECTIONS.with(|collections| collections.borrow_mut().clear());
}

pub fn set_fail_fee_sink_transfer(value: bool) {
  FAIL_FEE_SINK_TRANSFER.with(|v| *v.borrow_mut() = value);
}

pub(crate) fn control_atomicity_checkpoint(_actor_id: u64) -> DispatchResult {
  let should_fail = FAIL_CREATE_CHECKPOINT.with(|v| *v.borrow());
  if should_fail {
    return Err(DispatchError::Other("AtomicityCreateCheckpointFailed"));
  }
  Ok(())
}
