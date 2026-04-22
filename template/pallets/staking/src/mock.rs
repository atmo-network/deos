extern crate alloc;

use crate as pallet_staking;
use polkadot_sdk::frame_system::{self, EnsureRoot};
use polkadot_sdk::sp_runtime::{
  BuildStorage, FixedU128,
  testing::H256,
  traits::{BlakeTwo256, IdentityLookup},
};
use polkadot_sdk::{
  frame_support::{
    PalletId, construct_runtime, derive_impl,
    traits::{ConstU32, ConstU128, Get, Hooks},
    weights::Weight,
  },
  pallet_assets::Event as AssetsEvent,
};
use std::{
  cell::RefCell,
  collections::{BTreeMap, BTreeSet},
};

pub type AccountId = u64;
pub type AssetId = u32;
pub type Balance = u128;
type Block = frame_system::mocking::MockBlock<Test>;

thread_local! {
  static BENCHMARK_VALID_OPERATORS: RefCell<alloc::vec::Vec<AccountId>> = const { RefCell::new(alloc::vec![]) };
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

pub struct MockNativeBindingTargetValidator;
impl pallet_staking::NativeBindingTargetValidator<AccountId> for MockNativeBindingTargetValidator {
  fn is_valid_operator(account: &AccountId) -> bool {
    matches!(*account, 99)
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
  pub const MaxOperatorCommission: polkadot_sdk::sp_runtime::Perbill = polkadot_sdk::sp_runtime::Perbill::from_percent(50);
  pub const MaxRewardEventScanPerBlock: u32 = 128;
  pub const MaxRewardAccountsPerAssetEpoch: u32 = 256;
  pub const MaxRewardAssetsPerGovernanceDomain: u32 = 16;
  pub const MaxClaimEpochsPerCall: u32 = 16;
}

pub struct MockStakedAssetIdResolver;
impl pallet_staking::StakedAssetIdResolver<AssetId> for MockStakedAssetIdResolver {
  fn staked_asset_id(asset_id: AssetId) -> Option<AssetId> {
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

pub struct MockRewardGovernanceDomainResolver;
impl pallet_staking::RewardGovernanceDomainResolver<AssetId, u32>
  for MockRewardGovernanceDomainResolver
{
  fn reward_governance_domain(asset_id: AssetId) -> Option<u32> {
    Some(asset_id)
  }
}

pub struct MockRewardEpochProvider;
impl pallet_staking::RewardEpochProvider<u64> for MockRewardEpochProvider {
  fn current_reward_epoch() -> u64 {
    System::block_number()
  }
}

pub struct MockRewardCoefficientProvider;
impl pallet_staking::RewardCoefficientProvider<AccountId, u32> for MockRewardCoefficientProvider {
  fn reward_coefficient(domain: u32, account: &AccountId) -> FixedU128 {
    FixedU128::from_rational(u128::from(domain) + u128::from(*account), 10u128)
  }
}

pub struct MockNativeDelegationEventIngress;
impl pallet_staking::NativeDelegationEventIngress<AccountId> for MockNativeDelegationEventIngress {
  fn ingress(
    max_scan: usize,
    remaining_weight: Weight,
  ) -> pallet_staking::NativeDelegationIngress<AccountId> {
    const EVENT_SCAN_WEIGHT_REF_TIME: u64 = 1_000;
    const ACCOUNT_REFRESH_WEIGHT_REF_TIME: u64 = 2_000;
    let native_staked_asset_id = pallet_staking::Pallet::<Test>::staked_asset_id(
      <Test as pallet_staking::Config>::NativeStakingAssetId::get(),
    );
    let Some(native_staked_asset_id) = native_staked_asset_id else {
      return pallet_staking::NativeDelegationIngress {
        touched_accounts: alloc::vec::Vec::new(),
        weight: Weight::zero(),
        truncated: false,
      };
    };
    let max_ref_time = remaining_weight.ref_time();
    let mut scanned = 0u64;
    let mut truncated = false;
    let mut touched_accounts = BTreeSet::new();
    for record in System::read_events_no_consensus().take(max_scan.saturating_add(1)) {
      let next_scanned = scanned.saturating_add(1);
      let scan_only_ref_time = next_scanned
        .saturating_mul(EVENT_SCAN_WEIGHT_REF_TIME)
        .saturating_add(
          (touched_accounts.len() as u64).saturating_mul(ACCOUNT_REFRESH_WEIGHT_REF_TIME),
        );
      if scan_only_ref_time > max_ref_time {
        break;
      }
      scanned = next_scanned;
      if scanned > max_scan as u64 {
        truncated = true;
        break;
      }
      let projected_touches = match &record.event {
        RuntimeEvent::Assets(AssetsEvent::Transferred {
          asset_id,
          from,
          to,
          amount: _,
        }) if *asset_id == native_staked_asset_id => {
          let mut projected_touches = touched_accounts.len();
          projected_touches =
            projected_touches.saturating_add(usize::from(!touched_accounts.contains(from)));
          projected_touches.saturating_add(usize::from(!touched_accounts.contains(to)))
        }
        RuntimeEvent::Assets(
          AssetsEvent::Issued {
            asset_id,
            owner,
            amount: _,
          }
          | AssetsEvent::Deposited {
            asset_id,
            who: owner,
            amount: _,
          },
        ) if *asset_id == native_staked_asset_id => touched_accounts
          .len()
          .saturating_add(usize::from(!touched_accounts.contains(owner))),
        RuntimeEvent::Assets(
          AssetsEvent::Burned {
            asset_id,
            owner,
            balance: _,
          }
          | AssetsEvent::Withdrawn {
            asset_id,
            who: owner,
            amount: _,
          },
        ) if *asset_id == native_staked_asset_id => touched_accounts
          .len()
          .saturating_add(usize::from(!touched_accounts.contains(owner))),
        RuntimeEvent::Staking(
          pallet_staking::Event::NativeBindingSet { account, .. }
          | pallet_staking::Event::NativeBindingCleared { account },
        ) => touched_accounts
          .len()
          .saturating_add(usize::from(!touched_accounts.contains(account))),
        _ => touched_accounts.len(),
      };
      let projected_ref_time = scanned
        .saturating_mul(EVENT_SCAN_WEIGHT_REF_TIME)
        .saturating_add((projected_touches as u64).saturating_mul(ACCOUNT_REFRESH_WEIGHT_REF_TIME));
      if projected_ref_time > max_ref_time {
        truncated = true;
        break;
      }
      match &record.event {
        RuntimeEvent::Assets(AssetsEvent::Transferred {
          asset_id,
          from,
          to,
          amount: _,
        }) if *asset_id == native_staked_asset_id => {
          touched_accounts.insert(*from);
          touched_accounts.insert(*to);
        }
        RuntimeEvent::Assets(
          AssetsEvent::Issued {
            asset_id,
            owner,
            amount: _,
          }
          | AssetsEvent::Deposited {
            asset_id,
            who: owner,
            amount: _,
          },
        ) if *asset_id == native_staked_asset_id => {
          touched_accounts.insert(*owner);
        }
        RuntimeEvent::Assets(
          AssetsEvent::Burned {
            asset_id,
            owner,
            balance: _,
          }
          | AssetsEvent::Withdrawn {
            asset_id,
            who: owner,
            amount: _,
          },
        ) if *asset_id == native_staked_asset_id => {
          touched_accounts.insert(*owner);
        }
        RuntimeEvent::Staking(
          pallet_staking::Event::NativeBindingSet { account, .. }
          | pallet_staking::Event::NativeBindingCleared { account },
        ) => {
          touched_accounts.insert(*account);
        }
        _ => {}
      }
    }
    let touched_accounts = touched_accounts.into_iter().collect::<alloc::vec::Vec<_>>();
    pallet_staking::NativeDelegationIngress {
      weight: Weight::from_parts(
        scanned
          .saturating_mul(EVENT_SCAN_WEIGHT_REF_TIME)
          .saturating_add(
            (touched_accounts.len() as u64).saturating_mul(ACCOUNT_REFRESH_WEIGHT_REF_TIME),
          ),
        0,
      ),
      touched_accounts,
      truncated,
    }
  }
}

pub struct MockRewardSnapshotEventIngress;
impl pallet_staking::RewardSnapshotEventIngress<u64> for MockRewardSnapshotEventIngress {
  fn ingest(epoch: u64, max_scan: usize, remaining_weight: Weight) -> Weight {
    const EVENT_SCAN_WEIGHT_REF_TIME: u64 = 1_000;
    const ACCOUNT_TOUCH_WEIGHT_REF_TIME: u64 = 2_000;
    const REWARD_RECORD_WEIGHT_REF_TIME: u64 = 4_000;
    fn reward_inflow_asset_id(asset_id: AssetId, recipient: &AccountId) -> Option<AssetId> {
      if !pallet_staking::Pools::<Test>::contains_key(asset_id) {
        return None;
      }
      if pallet_staking::Pallet::<Test>::reward_account_for(asset_id) != *recipient {
        return None;
      }
      Some(asset_id)
    }
    let max_ref_time = remaining_weight.ref_time();
    let mut scanned = 0u64;
    let mut touched = 0u64;
    let mut recorded_reward_inflows = 0u64;
    let mut truncated = false;
    let mut pending_reward_touches: BTreeSet<(AssetId, AccountId)> = BTreeSet::new();
    let mut pending_reward_inflows: BTreeMap<AssetId, Balance> = BTreeMap::new();
    for record in System::read_events_no_consensus().take(max_scan.saturating_add(1)) {
      let next_scanned = scanned.saturating_add(1);
      let scan_only_ref_time = next_scanned
        .saturating_mul(EVENT_SCAN_WEIGHT_REF_TIME)
        .saturating_add(
          (pending_reward_touches.len() as u64).saturating_mul(ACCOUNT_TOUCH_WEIGHT_REF_TIME),
        )
        .saturating_add(
          (pending_reward_inflows.len() as u64).saturating_mul(REWARD_RECORD_WEIGHT_REF_TIME),
        );
      if scan_only_ref_time > max_ref_time {
        break;
      }
      scanned = next_scanned;
      if scanned > max_scan as u64 {
        truncated = true;
        break;
      }
      let mut projected_touches = pending_reward_touches.len();
      let mut projected_inflows = pending_reward_inflows.len();
      match &record.event {
        RuntimeEvent::Assets(AssetsEvent::Transferred {
          asset_id,
          from,
          to,
          amount,
        }) => {
          if let Some(reward_asset_id) = reward_inflow_asset_id(*asset_id, to) {
            projected_inflows = projected_inflows.saturating_add(usize::from(
              !pending_reward_inflows.contains_key(&reward_asset_id),
            ));
          }
          if let Some(base_asset_id) =
            pallet_staking::Pools::<Test>::iter_keys().find(|base_asset_id| {
              pallet_staking::Pallet::<Test>::staked_asset_id(*base_asset_id) == Some(*asset_id)
            })
          {
            projected_touches = projected_touches.saturating_add(usize::from(
              !pending_reward_touches.contains(&(base_asset_id, *from)),
            ));
            projected_touches = projected_touches.saturating_add(usize::from(
              !pending_reward_touches.contains(&(base_asset_id, *to)),
            ));
          }
          let projected_ref_time = scanned
            .saturating_mul(EVENT_SCAN_WEIGHT_REF_TIME)
            .saturating_add(
              (projected_touches as u64).saturating_mul(ACCOUNT_TOUCH_WEIGHT_REF_TIME),
            )
            .saturating_add(
              (projected_inflows as u64).saturating_mul(REWARD_RECORD_WEIGHT_REF_TIME),
            );
          if projected_ref_time > max_ref_time {
            truncated = true;
            break;
          }
          if let Some(reward_asset_id) = reward_inflow_asset_id(*asset_id, to) {
            pending_reward_inflows
              .entry(reward_asset_id)
              .and_modify(|value| *value = value.saturating_add(*amount))
              .or_insert(*amount);
          }
          let Some(base_asset_id) =
            pallet_staking::Pools::<Test>::iter_keys().find(|base_asset_id| {
              pallet_staking::Pallet::<Test>::staked_asset_id(*base_asset_id) == Some(*asset_id)
            })
          else {
            continue;
          };
          pending_reward_touches.insert((base_asset_id, *from));
          pending_reward_touches.insert((base_asset_id, *to));
        }
        RuntimeEvent::Assets(
          AssetsEvent::Issued {
            asset_id,
            owner,
            amount,
          }
          | AssetsEvent::Deposited {
            asset_id,
            who: owner,
            amount,
          },
        ) => {
          if let Some(reward_asset_id) = reward_inflow_asset_id(*asset_id, owner) {
            projected_inflows = projected_inflows.saturating_add(usize::from(
              !pending_reward_inflows.contains_key(&reward_asset_id),
            ));
          }
          if let Some(base_asset_id) =
            pallet_staking::Pools::<Test>::iter_keys().find(|base_asset_id| {
              pallet_staking::Pallet::<Test>::staked_asset_id(*base_asset_id) == Some(*asset_id)
            })
          {
            projected_touches = projected_touches.saturating_add(usize::from(
              !pending_reward_touches.contains(&(base_asset_id, *owner)),
            ));
          }
          let projected_ref_time = scanned
            .saturating_mul(EVENT_SCAN_WEIGHT_REF_TIME)
            .saturating_add(
              (projected_touches as u64).saturating_mul(ACCOUNT_TOUCH_WEIGHT_REF_TIME),
            )
            .saturating_add(
              (projected_inflows as u64).saturating_mul(REWARD_RECORD_WEIGHT_REF_TIME),
            );
          if projected_ref_time > max_ref_time {
            truncated = true;
            break;
          }
          if let Some(reward_asset_id) = reward_inflow_asset_id(*asset_id, owner) {
            pending_reward_inflows
              .entry(reward_asset_id)
              .and_modify(|value| *value = value.saturating_add(*amount))
              .or_insert(*amount);
          }
          let Some(base_asset_id) =
            pallet_staking::Pools::<Test>::iter_keys().find(|base_asset_id| {
              pallet_staking::Pallet::<Test>::staked_asset_id(*base_asset_id) == Some(*asset_id)
            })
          else {
            continue;
          };
          pending_reward_touches.insert((base_asset_id, *owner));
        }
        RuntimeEvent::Assets(
          AssetsEvent::Burned {
            asset_id,
            owner,
            balance: _,
          }
          | AssetsEvent::Withdrawn {
            asset_id,
            who: owner,
            amount: _,
          },
        ) => {
          if let Some(base_asset_id) =
            pallet_staking::Pools::<Test>::iter_keys().find(|base_asset_id| {
              pallet_staking::Pallet::<Test>::staked_asset_id(*base_asset_id) == Some(*asset_id)
            })
          {
            projected_touches = projected_touches.saturating_add(usize::from(
              !pending_reward_touches.contains(&(base_asset_id, *owner)),
            ));
          }
          let projected_ref_time = scanned
            .saturating_mul(EVENT_SCAN_WEIGHT_REF_TIME)
            .saturating_add(
              (projected_touches as u64).saturating_mul(ACCOUNT_TOUCH_WEIGHT_REF_TIME),
            )
            .saturating_add(
              (projected_inflows as u64).saturating_mul(REWARD_RECORD_WEIGHT_REF_TIME),
            );
          if projected_ref_time > max_ref_time {
            truncated = true;
            break;
          }
          let Some(base_asset_id) =
            pallet_staking::Pools::<Test>::iter_keys().find(|base_asset_id| {
              pallet_staking::Pallet::<Test>::staked_asset_id(*base_asset_id) == Some(*asset_id)
            })
          else {
            continue;
          };
          pending_reward_touches.insert((base_asset_id, *owner));
        }
        _ => {
          let projected_ref_time = scanned
            .saturating_mul(EVENT_SCAN_WEIGHT_REF_TIME)
            .saturating_add(
              (projected_touches as u64).saturating_mul(ACCOUNT_TOUCH_WEIGHT_REF_TIME),
            )
            .saturating_add(
              (projected_inflows as u64).saturating_mul(REWARD_RECORD_WEIGHT_REF_TIME),
            );
          if projected_ref_time > max_ref_time {
            truncated = true;
            break;
          }
        }
      }
    }
    if truncated {
      pallet_staking::Pallet::<Test>::note_reward_ingress_truncated(
        epoch,
        scanned as u32,
        max_scan as u32,
      );
    }
    for (asset_id, amount) in pending_reward_inflows {
      recorded_reward_inflows = recorded_reward_inflows.saturating_add(u64::from(
        pallet_staking::Pallet::<Test>::note_reward_inflow(asset_id, amount).is_ok(),
      ));
    }
    for (asset_id, account) in pending_reward_touches {
      touched = touched.saturating_add(u64::from(
        pallet_staking::Pallet::<Test>::note_reward_touch(asset_id, &account),
      ));
    }
    Weight::from_parts(
      scanned
        .saturating_mul(EVENT_SCAN_WEIGHT_REF_TIME)
        .saturating_add(touched.saturating_mul(ACCOUNT_TOUCH_WEIGHT_REF_TIME))
        .saturating_add(recorded_reward_inflows.saturating_mul(REWARD_RECORD_WEIGHT_REF_TIME)),
      0,
    )
  }
}

impl pallet_staking::Config for Test {
  type AdminOrigin = EnsureRoot<AccountId>;
  type AssetId = AssetId;
  type NativeStakingAssetId = ConstU32<1>;
  type GovernanceDomainId = u32;
  type RewardEpoch = u64;
  type NativeBindingTargetValidator = MockNativeBindingTargetValidator;
  type StakedAssetIdResolver = MockStakedAssetIdResolver;
  type StakedAssetLifecycle = MockStakedAssetLifecycle;
  type RewardGovernanceDomainResolver = MockRewardGovernanceDomainResolver;
  type RewardEpochProvider = MockRewardEpochProvider;
  type RewardCoefficientProvider = MockRewardCoefficientProvider;
  type RewardSnapshotEventIngress = MockRewardSnapshotEventIngress;
  type NativeDelegationEventIngress = MockNativeDelegationEventIngress;
  type MaxOperatorCommission = MaxOperatorCommission;
  type MaxRewardEventScanPerBlock = MaxRewardEventScanPerBlock;
  type MaxRewardAccountsPerAssetEpoch = MaxRewardAccountsPerAssetEpoch;
  type MaxRewardAssetsPerGovernanceDomain = MaxRewardAssetsPerGovernanceDomain;
  type MaxClaimEpochsPerCall = MaxClaimEpochsPerCall;
  type Balance = Balance;
  type Assets = Assets;
  type PalletId = StakingPalletId;
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
    System::set_block_number(1);
    let _ = Staking::on_initialize(1);
  });
  ext
}
