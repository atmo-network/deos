use crate as pallet_aaa;
use frame::prelude::*;
use polkadot_sdk::{
  frame_support::{
    PalletId, construct_runtime,
    traits::{ConstU8, ConstU32, ConstU128, Get},
  },
  frame_system::EnsureRoot,
  sp_runtime::{
    BuildStorage, Perbill,
    traits::{BlakeTwo256, IdentityLookup},
  },
};

use alloc::vec;
use core::cell::RefCell;

use crate::{AssetOps, DexOps, FeeCollector, LiquidityDonationOps, StakingOps};

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
    AAA: pallet_aaa,
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

pub struct AaaPalletId;
impl Get<PalletId> for AaaPalletId {
  fn get() -> PalletId {
    PalletId(*b"aaactor0")
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

  static UNSTAKED: RefCell<alloc::collections::BTreeMap<(AccountId, TestAsset), Balance>> =
    RefCell::new(alloc::collections::BTreeMap::new());

  static DONATED_LIQUIDITY: RefCell<alloc::collections::BTreeMap<(AccountId, TestAsset, TestAsset), (Balance, Balance)>> =
    RefCell::new(alloc::collections::BTreeMap::new());

  static GUARANTEED_ON_IDLE_WEIGHT: RefCell<polkadot_sdk::sp_weights::Weight> =
    RefCell::new(polkadot_sdk::sp_weights::Weight::MAX);
  static FAIL_CREATE_CHECKPOINT: RefCell<bool> = RefCell::new(false);
  static FAIL_CLOSE_CHECKPOINT: RefCell<bool> = RefCell::new(false);
  static FAIL_FEE_SINK_TRANSFER: RefCell<bool> = RefCell::new(false);
  static FAIL_DEX_AFTER_INPUT_TRANSFER: RefCell<bool> = RefCell::new(false);
  static FAIL_STAKING_OPS: RefCell<bool> = RefCell::new(false);
  static FAIL_STAKING_AFTER_BURN: RefCell<bool> = RefCell::new(false);
  static FAIL_LIQUIDITY_DONATION_OPS: RefCell<bool> = RefCell::new(false);
  static FAIL_LIQUIDITY_DONATION_AFTER_FIRST_BURN: RefCell<bool> = RefCell::new(false);
  #[cfg(feature = "runtime-benchmarks")]
  static BENCHMARK_INGRESS: RefCell<Option<(AccountId, AccountId, Balance)>> = RefCell::new(None);
  #[cfg(feature = "runtime-benchmarks")]
  static BENCHMARK_ASSET_OPS_INGRESS: RefCell<bool> = RefCell::new(false);
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

pub fn reset_mock_adapters() {
  ASSET_BALANCES.with(|b| b.borrow_mut().clear());
  BURNED.with(|b| b.borrow_mut().clear());
  MINTED.with(|b| b.borrow_mut().clear());
  POOL_RESERVES.with(|b| b.borrow_mut().clear());
  STAKED.with(|b| b.borrow_mut().clear());
  UNSTAKED.with(|b| b.borrow_mut().clear());
  DONATED_LIQUIDITY.with(|b| b.borrow_mut().clear());
  GUARANTEED_ON_IDLE_WEIGHT.with(|v| *v.borrow_mut() = polkadot_sdk::sp_weights::Weight::MAX);
  FAIL_CREATE_CHECKPOINT.with(|v| *v.borrow_mut() = false);
  FAIL_CLOSE_CHECKPOINT.with(|v| *v.borrow_mut() = false);
  FAIL_FEE_SINK_TRANSFER.with(|v| *v.borrow_mut() = false);
  FAIL_DEX_AFTER_INPUT_TRANSFER.with(|v| *v.borrow_mut() = false);
  FAIL_STAKING_OPS.with(|v| *v.borrow_mut() = false);
  FAIL_STAKING_AFTER_BURN.with(|v| *v.borrow_mut() = false);
  FAIL_LIQUIDITY_DONATION_OPS.with(|v| *v.borrow_mut() = false);
  FAIL_LIQUIDITY_DONATION_AFTER_FIRST_BURN.with(|v| *v.borrow_mut() = false);
  #[cfg(feature = "runtime-benchmarks")]
  {
    BENCHMARK_INGRESS.with(|event| *event.borrow_mut() = None);
    BENCHMARK_ASSET_OPS_INGRESS.with(|enabled| *enabled.borrow_mut() = false);
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
    MockAssetOps::transfer(payer, fee_sink, native_asset, amount)
  }
}

pub struct MockAssetOps;

impl AssetOps<AccountId, TestAsset, Balance> for MockAssetOps {
  fn transfer(
    from: &AccountId,
    to: &AccountId,
    asset: TestAsset,
    amount: Balance,
  ) -> Result<(), DispatchError> {
    match asset {
      TestAsset::Native => {
        if *to == TestFeeSink::get() && FAIL_FEE_SINK_TRANSFER.with(|v| *v.borrow()) {
          return Err(DispatchError::Other("MockFeeSinkTransferFailed"));
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
    #[cfg(feature = "runtime-benchmarks")]
    if BENCHMARK_ASSET_OPS_INGRESS.with(|enabled| *enabled.borrow()) {
      if let Some(aaa_id) = crate::SovereignIndex::<Test>::get(to) {
        crate::Pallet::<Test>::notify_address_event(aaa_id, asset, amount, from)?;
      }
    }
    Ok(())
  }

  fn burn(who: &AccountId, asset: TestAsset, amount: Balance) -> Result<(), DispatchError> {
    match asset {
      TestAsset::Native => {
        use polkadot_sdk::frame_support::traits::Currency;
        let (_, remainder) = <Balances as Currency<AccountId>>::slash(who, amount);
        if remainder > 0 {
          return Err(DispatchError::Token(
            polkadot_sdk::sp_runtime::TokenError::FundsUnavailable,
          ));
        }
        Ok(())
      }
      _ => ASSET_BALANCES.with(|b| {
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
      }),
    }
  }

  fn mint(to: &AccountId, asset: TestAsset, amount: Balance) -> Result<(), DispatchError> {
    match asset {
      TestAsset::Native => {
        use polkadot_sdk::frame_support::traits::Currency;
        let _ = <Balances as Currency<AccountId>>::deposit_creating(to, amount);
        Ok(())
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
        Ok(())
      }),
    }
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
    1
  }

  fn can_deposit(who: &AccountId, asset: TestAsset, amount: Balance) -> bool {
    if amount == 0 {
      return true;
    }
    let current = match asset {
      TestAsset::Native => {
        use polkadot_sdk::frame_support::traits::Currency;
        <Balances as Currency<AccountId>>::total_balance(who)
      }
      _ => ASSET_BALANCES.with(|b| b.borrow().get(&(*who, asset)).copied().unwrap_or(0)),
    };
    if current != 0 {
      return true;
    }
    amount >= Self::minimum_balance(asset)
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

pub fn set_fail_dex_after_input_transfer(value: bool) {
  FAIL_DEX_AFTER_INPUT_TRANSFER.with(|v| *v.borrow_mut() = value);
}

pub fn set_fail_staking_ops(value: bool) {
  FAIL_STAKING_OPS.with(|v| *v.borrow_mut() = value);
}

pub fn set_fail_staking_after_burn(value: bool) {
  FAIL_STAKING_AFTER_BURN.with(|v| *v.borrow_mut() = value);
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
    who: &AccountId,
    asset_in: TestAsset,
    asset_out: TestAsset,
    amount_in: Balance,
    slippage_tolerance: Perbill,
  ) -> Result<Balance, DispatchError> {
    let (ri, ro) = Self::get_reserves(asset_in, asset_out)?;
    let amount_out = amount_in.saturating_mul(ro) / (ri.saturating_add(amount_in));
    let quote = amount_in.saturating_mul(ro) / ri.saturating_add(amount_in);
    let min_out = (Perbill::one() - slippage_tolerance).mul_floor(quote);
    if amount_out < min_out {
      return Err(DispatchError::Other("SlippageExceeded"));
    }
    MockAssetOps::transfer(who, &u64::MAX, asset_in, amount_in)?;
    if FAIL_DEX_AFTER_INPUT_TRANSFER.with(|v| *v.borrow()) {
      return Err(DispatchError::Other("MockDexAfterInputTransferFailed"));
    }
    MockAssetOps::transfer(&u64::MAX, who, asset_out, amount_out)?;
    Ok(amount_out)
  }

  fn swap_exact_out(
    who: &AccountId,
    asset_in: TestAsset,
    asset_out: TestAsset,
    amount_out: Balance,
    max_amount_in: Balance,
    slippage_tolerance: Perbill,
  ) -> Result<Balance, DispatchError> {
    let (ri, ro) = Self::get_reserves(asset_in, asset_out)?;
    if amount_out >= ro {
      return Err(DispatchError::Other("InsufficientPoolLiquidity"));
    }
    let numerator = ri.saturating_mul(amount_out);
    let denominator = ro.saturating_sub(amount_out);
    let amount_in = numerator
      .checked_div(denominator)
      .ok_or(DispatchError::Other("DivisionByZero"))?
      .saturating_add(1);
    let quoted_max_in = amount_in.saturating_add(slippage_tolerance.mul_ceil(amount_in));
    if quoted_max_in > max_amount_in {
      return Err(DispatchError::Other("ExactOutInputCapacityExceeded"));
    }
    MockAssetOps::transfer(who, &u64::MAX, asset_in, amount_in)?;
    if FAIL_DEX_AFTER_INPUT_TRANSFER.with(|v| *v.borrow()) {
      return Err(DispatchError::Other("MockDexAfterInputTransferFailed"));
    }
    MockAssetOps::transfer(&u64::MAX, who, asset_out, amount_out)?;
    Ok(amount_in)
  }

  fn add_liquidity(
    _who: &AccountId,
    _asset_a: TestAsset,
    _asset_b: TestAsset,
    amount_a: Balance,
    amount_b: Balance,
  ) -> Result<(Balance, Balance, Balance), DispatchError> {
    let lp_minted = integer_sqrt(amount_a.saturating_mul(amount_b));
    Ok((amount_a, amount_b, lp_minted))
  }

  fn remove_liquidity(
    _who: &AccountId,
    _lp_asset: TestAsset,
    lp_amount: Balance,
  ) -> Result<(Balance, Balance), DispatchError> {
    let half = lp_amount / 2;
    Ok((half, half))
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
  fn stake(who: &AccountId, asset: TestAsset, amount: Balance) -> Result<(), DispatchError> {
    if FAIL_STAKING_OPS.with(|v| *v.borrow()) {
      return Err(DispatchError::Other("MockStakingOpsFailed"));
    }
    MockAssetOps::burn(who, asset, amount)?;
    if FAIL_STAKING_AFTER_BURN.with(|v| *v.borrow()) {
      return Err(DispatchError::Other("MockStakingAfterBurnFailed"));
    }
    STAKED.with(|s| {
      let mut map = s.borrow_mut();
      let current = map.get(&(*who, asset)).copied().unwrap_or(0);
      map.insert((*who, asset), current.saturating_add(amount));
    });
    Ok(())
  }

  fn unstake(who: &AccountId, asset: TestAsset, shares: Balance) -> Result<(), DispatchError> {
    if FAIL_STAKING_OPS.with(|v| *v.borrow()) {
      return Err(DispatchError::Other("MockStakingOpsFailed"));
    }
    MockAssetOps::burn(who, asset, shares)?;
    if FAIL_STAKING_AFTER_BURN.with(|v| *v.borrow()) {
      return Err(DispatchError::Other("MockStakingAfterBurnFailed"));
    }
    UNSTAKED.with(|s| {
      let mut map = s.borrow_mut();
      let current = map.get(&(*who, asset)).copied().unwrap_or(0);
      map.insert((*who, asset), current.saturating_add(shares));
    });
    Ok(())
  }

  fn share_balance(who: &AccountId, asset: TestAsset) -> Balance {
    MockAssetOps::balance(who, asset)
  }

  fn share_asset(asset: TestAsset) -> Option<TestAsset> {
    if asset == TestAsset::Local(u32::MAX) {
      None
    } else {
      Some(asset)
    }
  }
}

pub struct MockLiquidityDonationOps;

impl LiquidityDonationOps<AccountId, TestAsset, Balance> for MockLiquidityDonationOps {
  fn donate_liquidity(
    who: &AccountId,
    asset_a: TestAsset,
    asset_b: TestAsset,
    amount: Balance,
    _max_ratio_error: Perbill,
  ) -> Result<(Balance, Balance), DispatchError> {
    if FAIL_LIQUIDITY_DONATION_OPS.with(|v| *v.borrow()) {
      return Err(DispatchError::Other("MockLiquidityDonationOpsFailed"));
    }
    if MockAssetOps::balance(who, asset_a) < amount || MockAssetOps::balance(who, asset_b) < amount
    {
      return Err(DispatchError::Token(
        polkadot_sdk::sp_runtime::TokenError::FundsUnavailable,
      ));
    }
    MockAssetOps::burn(who, asset_a, amount)?;
    if FAIL_LIQUIDITY_DONATION_AFTER_FIRST_BURN.with(|v| *v.borrow()) {
      return Err(DispatchError::Other(
        "MockLiquidityDonationAfterFirstBurnFailed",
      ));
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
impl crate::BenchmarkHelper<AccountId, TestAsset, Balance> for MockBenchmarkHelper {
  fn setup_add_liquidity(
    owner: &AccountId,
  ) -> Result<(TestAsset, TestAsset, Balance, Balance), DispatchError> {
    let asset_a = TestAsset::Local(1);
    let asset_b = TestAsset::Local(2);
    let amount = 1_000_000;
    MockAssetOps::mint(owner, asset_a, amount)?;
    MockAssetOps::mint(owner, asset_b, amount)?;
    Ok((asset_a, asset_b, amount, amount))
  }

  fn setup_donate_liquidity(
    owner: &AccountId,
  ) -> Result<(TestAsset, TestAsset, Balance), DispatchError> {
    let asset_a = TestAsset::Local(1);
    let asset_b = TestAsset::Local(2);
    let amount = 1_000_000;
    MockAssetOps::mint(owner, asset_a, amount)?;
    MockAssetOps::mint(owner, asset_b, amount)?;
    Ok((asset_a, asset_b, amount))
  }

  fn setup_stake(owner: &AccountId) -> Result<(TestAsset, Balance), DispatchError> {
    let asset = TestAsset::Local(1);
    let amount = 1_000_000;
    MockAssetOps::mint(owner, asset, amount)?;
    Ok((asset, amount))
  }

  fn setup_unstake(owner: &AccountId) -> Result<(TestAsset, Balance), DispatchError> {
    let asset = TestAsset::Local(1);
    let shares = 1_000_000;
    MockAssetOps::mint(owner, asset, shares)?;
    Ok((asset, shares))
  }

  fn setup_swap_exact_in(
    owner: &AccountId,
  ) -> Result<(TestAsset, TestAsset, Balance), DispatchError> {
    let asset_in = TestAsset::Local(1);
    let asset_out = TestAsset::Local(2);
    let amount_in = 1_000;
    set_pool_reserves(asset_in, asset_out, 1_000_000, 1_000_000);
    MockAssetOps::mint(owner, asset_in, amount_in)?;
    MockAssetOps::mint(&u64::MAX, asset_out, 1_000_000)?;
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
    MockAssetOps::mint(owner, asset_in, max_amount_in)?;
    MockAssetOps::mint(&u64::MAX, asset_out, 1_000_000)?;
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

  fn run_address_event_ingress(recipient: &AccountId) -> bool {
    let event = BENCHMARK_INGRESS.with(|pending| *pending.borrow());
    let Some((event_recipient, source, amount)) = event else {
      return false;
    };
    if event_recipient != *recipient {
      return false;
    }
    let Some(aaa_id) = crate::SovereignIndex::<Test>::get(recipient) else {
      return false;
    };
    crate::Pallet::<Test>::notify_address_event(aaa_id, TestAsset::Native, amount, &source)
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
    MockAssetOps::mint(recipient, TestAsset::Native, amount)?;
    if let Some(aaa_id) = crate::SovereignIndex::<Test>::get(recipient) {
      crate::Pallet::<Test>::notify_xcm_address_event(aaa_id, TestAsset::Native, amount, source)?;
    }
    Ok(())
  }

  fn clear_address_event_ingress_events() {
    BENCHMARK_INGRESS.with(|event| *event.borrow_mut() = None);
  }

  fn run_compatibility_address_event_ingress() -> polkadot_sdk::sp_weights::Weight {
    let _ = crate::Pallet::<Test>::drain_address_event_overflow(1);
    polkadot_sdk::sp_weights::Weight::zero()
  }

  fn setup_remove_liquidity_max_k(
    owner: &AccountId,
    _max_scan: u32,
  ) -> Result<(TestAsset, Balance), DispatchError> {
    let lp_asset = TestAsset::Local(1);
    let lp_amount = 1_000_000u128;
    MockAssetOps::mint(owner, lp_asset, lp_amount)?;
    Ok((lp_asset, lp_amount))
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

pub struct TestStepBaseFee;
impl Get<Balance> for TestStepBaseFee {
  fn get() -> Balance {
    1
  }
}

pub struct TestConditionReadFee;
impl Get<Balance> for TestConditionReadFee {
  fn get() -> Balance {
    1
  }
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

pub struct TestAaaCreationFee;
impl Get<Balance> for TestAaaCreationFee {
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

pub struct TestGuaranteedOnIdleWeight;
impl Get<polkadot_sdk::sp_weights::Weight> for TestGuaranteedOnIdleWeight {
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
    3
  }
}

pub struct TestMinUserBalance;
impl Get<Balance> for TestMinUserBalance {
  fn get() -> Balance {
    50
  }
}

pub struct TestMaxSweepPerBlock;
impl Get<u32> for TestMaxSweepPerBlock {
  fn get() -> u32 {
    3
  }
}

pub struct MockFundingAuthority;

impl crate::adapters::FundingAuthority<AccountId> for MockFundingAuthority {
  fn allows(_: crate::AaaId, _: &AccountId, _: &crate::FundingProvenance<AccountId>) -> bool {
    true
  }
}

impl pallet_aaa::Config for Test {
  type AssetId = TestAsset;
  type Balance = Balance;
  type NativeAssetId = NativeAsset;
  type AssetOps = MockAssetOps;
  type FundingAuthority = MockFundingAuthority;
  type DexOps = MockDexOps;
  type StakingOps = MockStakingOps;
  type LiquidityDonationOps = MockLiquidityDonationOps;
  type MinWindowLength = frame::traits::ConstU64<100>;
  type PalletId = AaaPalletId;
  type SystemOrigin = EnsureRoot<AccountId>;
  type GlobalBreakerOrigin = EnsureRoot<AccountId>;
  type MaxExecutionPlanSteps = ConstU32<10>;
  type MaxUserExecutionPlanSteps = ConstU32<3>;
  type MaxSystemExecutionPlanSteps = ConstU32<10>;
  type MaxFundingTrackedAssets = ConstU32<10>;
  type MaxIngressOverflowQueue = ConstU32<256>;
  type MaxConditionsPerStep = ConstU32<4>;
  type MaxOwnerSlots = ConstU8<8>;
  type MaxExecutionsPerBlock = ConstU32<3>;
  type MaxQueueLength = ConstU32<1024>;
  type QueuePageSize = ConstU32<32>;
  type WakeupPageSize = ConstU32<32>;
  type MaxQueueEntriesScannedPerBlock = ConstU32<1024>;
  type MaxWakeupsPerBlock = ConstU32<64>;
  type MaxSweepPerBlock = TestMaxSweepPerBlock;
  type MaxWhitelistSize = ConstU32<16>;
  type MaxSplitTransferLegs = ConstU32<8>;
  type MaxAdapterScan = ConstU32<64>;
  type MaxExecutionDelayBlocks = TestMaxExecutionDelayBlocks;
  type MaxTimerJitterBlocks = ConstU32<64>;
  type MaxIdleStarvationBlocks = TestMaxIdleStarvationBlocks;
  type GuaranteedOnIdleWeight = TestGuaranteedOnIdleWeight;
  type MaxAutoCloseNonceHorizon = TestMaxAutoCloseNonceHorizon;
  type MaxActiveActors = ConstU32<10_000>;
  type MaxActorIdentities = ConstU32<10_000>;
  type StepBaseFee = TestStepBaseFee;
  type ConditionReadFee = TestConditionReadFee;
  type AaaCreationFee = TestAaaCreationFee;
  type WeightToFee = TestWeightToFee;
  type TaskWeightInfo = ();
  type AtomicityHook = MockAtomicityHook;
  type AddressEventIngressHook = ();
  type FeeSink = TestFeeSink;
  type FeeCollector = MockFeeCollector;
  type MaxConsecutiveFailures = TestMaxConsecutiveFailures;
  type MinUserBalance = TestMinUserBalance;
  type WeightInfo = ();
  type GenesisSystemAaas = ();
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

pub fn set_fail_close_checkpoint(value: bool) {
  FAIL_CLOSE_CHECKPOINT.with(|v| *v.borrow_mut() = value);
}

pub fn set_fail_fee_sink_transfer(value: bool) {
  FAIL_FEE_SINK_TRANSFER.with(|v| *v.borrow_mut() = value);
}

pub struct MockAtomicityHook;

impl crate::AtomicityHook for MockAtomicityHook {
  fn on_create_checkpoint(_aaa_id: u64) -> DispatchResult {
    let should_fail = FAIL_CREATE_CHECKPOINT.with(|v| *v.borrow());
    if should_fail {
      return Err(DispatchError::Other("AtomicityCreateCheckpointFailed"));
    }
    Ok(())
  }

  fn on_close_checkpoint(_aaa_id: u64) -> DispatchResult {
    let should_fail = FAIL_CLOSE_CHECKPOINT.with(|v| *v.borrow());
    if should_fail {
      return Err(DispatchError::Other("AtomicityCloseCheckpointFailed"));
    }
    Ok(())
  }
}
