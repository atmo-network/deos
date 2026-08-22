extern crate alloc;

use crate as pallet_staking;
use polkadot_sdk::frame_support::{
  PalletId, construct_runtime, derive_impl,
  traits::{ConstU32, ConstU128, Get, Hooks},
};
use polkadot_sdk::frame_system::{self, EnsureRoot};
use polkadot_sdk::sp_runtime::{
  BuildStorage, FixedU128,
  testing::H256,
  traits::{BlakeTwo256, IdentityLookup},
};
use std::{cell::RefCell, collections::BTreeMap};

pub type AccountId = u64;
pub type AssetId = u32;
pub type Balance = u128;
type Block = frame_system::mocking::MockBlock<Test>;

thread_local! {
  static BENCHMARK_VALID_OPERATORS: RefCell<alloc::vec::Vec<AccountId>> = const { RefCell::new(alloc::vec![]) };
  static NATIVE_GOVERNANCE_LOCKS: RefCell<BTreeMap<AccountId, u64>> = const { RefCell::new(BTreeMap::new()) };
  static NATIVE_SECURITY_MODE: RefCell<pallet_staking::NativeSecurityMode> = const {
    RefCell::new(pallet_staking::NativeSecurityMode::LpBackedSelection)
  };
  static SECURITY_EPOCH: RefCell<pallet_staking::SecurityEpoch> = const { RefCell::new(0) };
  static GOVERNANCE_COEFFICIENTS: RefCell<BTreeMap<AccountId, FixedU128>> = const { RefCell::new(BTreeMap::new()) };
  static NATIVE_LP_VALUE_MULTIPLIER: RefCell<Balance> = const { RefCell::new(1) };
  static COMPOUND_FAILURE: RefCell<bool> = const { RefCell::new(false) };
  static COMPOUND_LP_OUT: RefCell<Balance> = const { RefCell::new(10) };
}

pub fn set_native_security_mode(mode: pallet_staking::NativeSecurityMode) {
  NATIVE_SECURITY_MODE.with(|current| *current.borrow_mut() = mode);
}

pub fn set_security_epoch(epoch: pallet_staking::SecurityEpoch) {
  SECURITY_EPOCH.with(|current| *current.borrow_mut() = epoch);
}

pub fn set_governance_coefficient(account: AccountId, coefficient: FixedU128) {
  GOVERNANCE_COEFFICIENTS.with(|coefficients| {
    coefficients.borrow_mut().insert(account, coefficient);
  });
}

pub fn set_native_lp_value_multiplier(multiplier: Balance) {
  NATIVE_LP_VALUE_MULTIPLIER.with(|current| *current.borrow_mut() = multiplier);
}

pub fn set_compound_failure(fail: bool) {
  COMPOUND_FAILURE.with(|current| *current.borrow_mut() = fail);
}

pub fn set_compound_lp_out(lp_out: Balance) {
  COMPOUND_LP_OUT.with(|current| *current.borrow_mut() = lp_out);
}

pub struct MockNativeSecurityModeProvider;
impl pallet_staking::NativeSecurityModeProvider for MockNativeSecurityModeProvider {
  fn mode() -> pallet_staking::NativeSecurityMode {
    NATIVE_SECURITY_MODE.with(|mode| *mode.borrow())
  }

  #[cfg(feature = "runtime-benchmarks")]
  fn benchmark_prepare_lp_backed_selection() {
    set_native_security_mode(pallet_staking::NativeSecurityMode::LpBackedSelection);
  }
}

construct_runtime!(
  pub struct Test {
    System: frame_system,
    Balances: polkadot_sdk::pallet_balances,
    Assets: polkadot_sdk::pallet_assets,
    Staking: pallet_staking,
  }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
  type Block = Block;
  type AccountId = AccountId;
  type Lookup = IdentityLookup<Self::AccountId>;
  type Hash = H256;
  type Hashing = BlakeTwo256;
  type AccountData = polkadot_sdk::pallet_balances::AccountData<Balance>;
}

impl polkadot_sdk::pallet_balances::Config for Test {
  type MaxLocks = ();
  type MaxReserves = ();
  type ReserveIdentifier = [u8; 8];
  type Balance = Balance;
  type DustRemoval = ();
  type RuntimeEvent = RuntimeEvent;
  type ExistentialDeposit = ConstU128<1>;
  type AccountStore = System;
  type WeightInfo = ();
  type FreezeIdentifier = ();
  type MaxFreezes = ();
  type RuntimeHoldReason = ();
  type RuntimeFreezeReason = ();
  type DoneSlashHandler = ();
}

impl polkadot_sdk::pallet_assets::Config for Test {
  type RuntimeEvent = RuntimeEvent;
  type Balance = Balance;
  type AssetId = AssetId;
  type AssetIdParameter = AssetId;
  type Currency = Balances;
  type CreateOrigin = polkadot_sdk::frame_support::traits::AsEnsureOriginWithArg<
    frame_system::EnsureSigned<Self::AccountId>,
  >;
  type ForceOrigin = EnsureRoot<Self::AccountId>;
  type AssetDeposit = ConstU128<1>;
  type AssetAccountDeposit = ConstU128<1>;
  type MetadataDepositBase = ConstU128<1>;
  type MetadataDepositPerByte = ConstU128<1>;
  type ApprovalDeposit = ConstU128<1>;
  type StringLimit = ConstU32<50>;
  type Freezer = ();
  type Extra = ();
  type ReserveData = ();
  type CallbackHandle = ();
  type WeightInfo = ();
  type RemoveItemsLimit = ConstU32<10>;
  type Holder = ();
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = ();
}

pub struct StakingPalletId;
impl Get<PalletId> for StakingPalletId {
  fn get() -> PalletId {
    PalletId(*b"stkngtst")
  }
}

pub struct NativeLpLockAccount;
impl Get<AccountId> for NativeLpLockAccount {
  fn get() -> AccountId {
    9_998
  }
}

pub struct NativeSecurityRewardAccount;
impl Get<AccountId> for NativeSecurityRewardAccount {
  fn get() -> AccountId {
    9_999
  }
}

pub struct MockNativeOperatorValidator;
impl pallet_staking::NativeOperatorValidator<AccountId> for MockNativeOperatorValidator {
  fn is_valid_operator(account: &AccountId) -> bool {
    matches!(*account, 2 | 99 | 100)
      || BENCHMARK_VALID_OPERATORS.with(|operators| operators.borrow().contains(account))
  }

  #[cfg(feature = "runtime-benchmarks")]
  fn benchmark_prepare_valid_operator(account: &AccountId) {
    BENCHMARK_VALID_OPERATORS.with(|operators| {
      let mut operators = operators.borrow_mut();
      if !operators.contains(account) {
        operators.push(*account);
      }
    });
  }
}

polkadot_sdk::frame_support::parameter_types! {
  pub const NativeGovernanceDomainId: u32 = 1;
  pub const SecurityRewardFundingSource: AccountId = 3;
  pub const MaxNativeSecurityParticipants: u32 = 3;
  pub const MaxNativeSecurityOperators: u32 = 3;
  pub const MaxNominationsPerAccount: u32 = 2;
  pub const NativeLpUnlockDelay: u64 = 3;
  pub const SecurityRewardClaimHorizon: u32 = 3;
  pub const MaxSecurityRewardClaimsPerCall: u32 = 3;
}

pub struct MockNativeStakingLpAssetValidator;
impl pallet_staking::NativeStakingLpAssetValidator<AssetId> for MockNativeStakingLpAssetValidator {
  fn is_valid_native_staking_lp_asset(asset_id: AssetId) -> bool {
    asset_id == 0x7000_0001
  }
}

pub struct MockNativeStakingReadModelProvider;
impl pallet_staking::NativeStakingReadModelProvider<AssetId, Balance>
  for MockNativeStakingReadModelProvider
{
  fn native_staking_liquidity_pool() -> Option<(AssetId, Balance, Balance, Balance)> {
    <Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<AccountId>>::asset_exists(
      0x7000_0001,
    )
    .then_some((0x7000_0001, 1, 1, 1))
  }

  fn native_lp_value(locked_lp: Balance) -> Option<Balance> {
    Some(
      NATIVE_LP_VALUE_MULTIPLIER.with(|multiplier| locked_lp.saturating_mul(*multiplier.borrow())),
    )
  }
}

pub struct MockNativeSecurityRewardCompound;
impl pallet_staking::NativeSecurityRewardCompound<AccountId, AssetId, Balance>
  for MockNativeSecurityRewardCompound
{
  fn compound(
    account: &AccountId,
    _reward: Balance,
    _min_lp_out: Balance,
  ) -> Result<(AssetId, Balance), polkadot_sdk::sp_runtime::DispatchError> {
    if COMPOUND_FAILURE.with(|fail| *fail.borrow()) {
      return Err(polkadot_sdk::sp_runtime::DispatchError::Other(
        "MockCompoundFailure",
      ));
    }
    let lp_out = COMPOUND_LP_OUT.with(|amount| *amount.borrow());
    <Assets as polkadot_sdk::frame_support::traits::fungibles::Mutate<AccountId>>::mint_into(
      0x7000_0001,
      account,
      lp_out,
    )?;
    Ok((0x7000_0001, lp_out))
  }
}

#[cfg(feature = "runtime-benchmarks")]
pub struct MockBenchmarkHelper;
#[cfg(feature = "runtime-benchmarks")]
impl pallet_staking::BenchmarkHelper<AccountId, AssetId, Balance> for MockBenchmarkHelper {
  fn prepare_native_staking_lp(
    account: &AccountId,
    amount: Balance,
  ) -> Result<AssetId, polkadot_sdk::sp_runtime::DispatchError> {
    const LP_ASSET: AssetId = 0x7000_0001;
    if !<Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<AccountId>>::asset_exists(
      LP_ASSET,
    ) {
      Assets::force_create(RuntimeOrigin::root(), LP_ASSET, 1, true, 1)?;
    }
    <Assets as polkadot_sdk::frame_support::traits::fungibles::Mutate<AccountId>>::mint_into(
      LP_ASSET, account, amount,
    )?;
    Ok(LP_ASSET)
  }

  fn prepare_native_governance_asset(
    account: &AccountId,
    amount: Balance,
  ) -> Result<AssetId, polkadot_sdk::sp_runtime::DispatchError> {
    if !<Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<AccountId>>::asset_exists(
      1,
    ) {
      Assets::force_create(RuntimeOrigin::root(), 1, 1, true, 1)?;
    }
    <Assets as polkadot_sdk::frame_support::traits::fungibles::Mutate<AccountId>>::mint_into(
      1, account, amount,
    )?;
    Ok(1)
  }

  fn set_security_epoch(epoch: pallet_staking::SecurityEpoch) {
    set_security_epoch(epoch);
  }

  fn fund_native_account(account: &AccountId, amount: Balance) {
    let _ =
      <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::deposit_creating(
        account, amount,
      );
  }
}

pub struct MockNativeGovernanceLockProvider;
impl pallet_staking::NativeGovernanceLockProvider<AccountId, u64>
  for MockNativeGovernanceLockProvider
{
  fn lock_until(account: &AccountId) -> Option<u64> {
    NATIVE_GOVERNANCE_LOCKS.with(|locks| locks.borrow().get(account).copied())
  }
}

pub fn set_native_governance_lock(account: AccountId, lock_until: u64) {
  NATIVE_GOVERNANCE_LOCKS.with(|locks| {
    locks.borrow_mut().insert(account, lock_until);
  });
}

pub struct MockStakedAssetIdResolver;
impl pallet_staking::StakedAssetIdResolver<AssetId> for MockStakedAssetIdResolver {
  fn staked_asset_id(asset_id: AssetId) -> Option<AssetId> {
    if asset_id == 99 {
      return None;
    }
    const TYPE_FOREIGN: AssetId = 0xF000_0000;
    const TYPE_STAKED: AssetId = 0x5000_0000;
    const TYPE_STAKED_FOREIGN: AssetId = 0x6000_0000;
    if asset_id == 1 {
      return Some(TYPE_STAKED);
    }
    if (asset_id & TYPE_FOREIGN) == TYPE_FOREIGN {
      return Some(TYPE_STAKED_FOREIGN | (asset_id & 0x0FFF_FFFF));
    }
    Some(TYPE_STAKED | asset_id)
  }
}

pub struct MockStakedAssetLifecycle;
impl pallet_staking::StakedAssetLifecycle<AccountId, AssetId> for MockStakedAssetLifecycle {
  fn register(
    asset_id: AssetId,
    staked_asset_id: AssetId,
    admin: &AccountId,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    let (name, symbol, decimals) = match asset_id {
      1 => (b"Staked Native Token".to_vec(), b"stNATIVE".to_vec(), 12),
      2 => (b"Staked Asset 2".to_vec(), b"stASSET2".to_vec(), 12),
      _ => (
        format!("Staked Asset {asset_id}").into_bytes(),
        format!("stASSET{asset_id}").into_bytes(),
        12,
      ),
    };
    Assets::force_create(RuntimeOrigin::root(), staked_asset_id, *admin, true, 1)?;
    Assets::force_set_metadata(
      RuntimeOrigin::root(),
      staked_asset_id,
      name,
      symbol,
      decimals,
      false,
    )
  }
}

pub struct MockSecurityEpochProvider;
impl pallet_staking::SecurityEpochProvider for MockSecurityEpochProvider {
  fn current_security_epoch() -> pallet_staking::SecurityEpoch {
    SECURITY_EPOCH.with(|current| *current.borrow())
  }
}

pub struct MockGovernanceParticipationCoefficientProvider;
impl pallet_staking::GovernanceParticipationCoefficientProvider<AccountId, u32>
  for MockGovernanceParticipationCoefficientProvider
{
  fn governance_participation_coefficient(domain: u32, account: &AccountId) -> FixedU128 {
    GOVERNANCE_COEFFICIENTS.with(|coefficients| {
      coefficients
        .borrow()
        .get(account)
        .copied()
        .unwrap_or_else(|| {
          FixedU128::from_rational(u128::from(domain) + u128::from(*account), 10u128)
        })
    })
  }
}

impl pallet_staking::Config for Test {
  type AdminOrigin = EnsureRoot<AccountId>;
  type AssetId = AssetId;
  type NativeStakingAssetId = ConstU32<1>;
  type NativeCurrency = Balances;
  type SecurityRewardFundingOrigin = EnsureRoot<AccountId>;
  type SecurityRewardFundingSource = SecurityRewardFundingSource;
  type GovernanceDomainId = u32;
  type NativeGovernanceDomainId = NativeGovernanceDomainId;
  type NativeOperatorValidator = MockNativeOperatorValidator;
  type NativeStakingLpAssetValidator = MockNativeStakingLpAssetValidator;
  type NativeLpAssetNamespaceInitializer = ();
  type NativeGovernanceLockProvider = MockNativeGovernanceLockProvider;
  type NativeSecurityModeProvider = MockNativeSecurityModeProvider;
  type StakedAssetIdResolver = MockStakedAssetIdResolver;
  type StakedAssetLifecycle = MockStakedAssetLifecycle;
  type SecurityEpochProvider = MockSecurityEpochProvider;
  type GovernanceParticipationCoefficientProvider = MockGovernanceParticipationCoefficientProvider;
  type NativeStakingReadModelProvider = MockNativeStakingReadModelProvider;
  type NativeSecurityRewardCompound = MockNativeSecurityRewardCompound;
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = MockBenchmarkHelper;
  type MaxNativeSecurityParticipants = MaxNativeSecurityParticipants;
  type MaxNativeSecurityOperators = MaxNativeSecurityOperators;
  type MaxNominationsPerAccount = MaxNominationsPerAccount;
  type NativeLpUnlockDelay = NativeLpUnlockDelay;
  type SecurityRewardClaimHorizon = SecurityRewardClaimHorizon;
  type MaxSecurityRewardClaimsPerCall = MaxSecurityRewardClaimsPerCall;
  type Balance = Balance;
  type Assets = Assets;
  type PalletId = StakingPalletId;
  type NativeLpLockAccount = NativeLpLockAccount;
  type NativeSecurityRewardAccount = NativeSecurityRewardAccount;
  type WeightInfo = ();
}

pub fn new_test_ext() -> polkadot_sdk::sp_io::TestExternalities {
  let mut storage = frame_system::GenesisConfig::<Test>::default()
    .build_storage()
    .unwrap();
  polkadot_sdk::pallet_balances::GenesisConfig::<Test> {
    balances: vec![(1, 1_000), (2, 1_000), (3, 1_000), (99, 1_000)],
    dev_accounts: None,
  }
  .assimilate_storage(&mut storage)
  .unwrap();
  polkadot_sdk::pallet_assets::GenesisConfig::<Test> {
    assets: alloc::vec![(1, 1, true, 1), (2, 1, true, 1)],
    metadata: alloc::vec![],
    accounts: alloc::vec![
      (1, 1, 1_000),
      (1, 2, 1_000),
      (1, 3, 1_000),
      (2, 1, 1_000),
      (2, 2, 1_000),
      (2, 3, 1_000),
      (2, 99, 1_000),
    ],
    reserves: alloc::vec![],
    next_asset_id: None,
  }
  .assimilate_storage(&mut storage)
  .unwrap();
  let mut ext: polkadot_sdk::sp_io::TestExternalities = storage.into();
  ext.execute_with(|| {
    set_native_security_mode(pallet_staking::NativeSecurityMode::LpBackedSelection);
    set_security_epoch(0);
    GOVERNANCE_COEFFICIENTS.with(|coefficients| coefficients.borrow_mut().clear());
    set_native_lp_value_multiplier(1);
    System::set_block_number(1);
    let _ = Staking::on_initialize(1);
  });
  ext
}
