use super::assets_config::AssetId;
use super::*;

use alloc::{
  collections::{BTreeMap, BTreeSet},
  format,
};
use pallet_governance::Event as GovernanceEvent;
use polkadot_sdk::frame_support::traits::fungibles::metadata::Inspect as MetadataInspect;
use polkadot_sdk::{
  frame_support::{PalletId, parameter_types, weights::Weight},
  frame_system::EnsureRoot,
  pallet_assets::Event as AssetsEvent,
  sp_runtime::FixedU128,
};

parameter_types! {
  pub const StakingPalletId: PalletId = PalletId(*primitives::ecosystem::pallet_ids::STAKING_PALLET_ID);
  pub const NativeStakingAssetId: AssetId = 0;
  pub const MaxOperatorCommission: polkadot_sdk::sp_runtime::Perbill = polkadot_sdk::sp_runtime::Perbill::from_percent(50);
  pub const MaxRewardEventScanPerBlock: u32 = 128;
  pub const MaxRewardAccountsPerAssetEpoch: u32 = 256;
  pub const MaxClaimEpochsPerCall: u32 = 16;
}

pub struct RuntimeNativeBindingTargetValidator;
impl pallet_staking::NativeBindingTargetValidator<AccountId>
  for RuntimeNativeBindingTargetValidator
{
  fn is_valid_operator(account: &AccountId) -> bool {
    pallet_collator_selection::Invulnerables::<Runtime>::get().contains(account)
      || (PermissionlessCollatorsEnabled::get()
        && pallet_collator_selection::CandidateList::<Runtime>::get()
          .iter()
          .any(|candidate| &candidate.who == account))
  }

  #[cfg(feature = "runtime-benchmarks")]
  fn benchmark_prepare_valid_operator(account: &AccountId) {
    use polkadot_sdk::frame_support::BoundedVec;
    let mut invulnerables = pallet_collator_selection::Invulnerables::<Runtime>::get().into_inner();
    if invulnerables.contains(account) {
      return;
    }
    invulnerables.push(account.clone());
    pallet_collator_selection::Invulnerables::<Runtime>::put(BoundedVec::truncate_from(
      invulnerables,
    ));
  }
}

pub struct RuntimeStakedAssetIdResolver;
impl pallet_staking::StakedAssetIdResolver<AssetId> for RuntimeStakedAssetIdResolver {
  fn staked_asset_id(asset_id: AssetId) -> Option<AssetId> {
    let asset_kind = if asset_id == NativeStakingAssetId::get() {
      primitives::AssetKind::Native
    } else if (asset_id & primitives::MASK_TYPE) == primitives::TYPE_FOREIGN {
      primitives::AssetKind::Foreign(asset_id)
    } else {
      primitives::AssetKind::Local(asset_id)
    };
    match asset_kind.into_staked()? {
      primitives::AssetKind::Local(id) => Some(id),
      _ => None,
    }
  }
}

pub struct RuntimeStakedAssetLifecycle;
impl pallet_staking::StakedAssetLifecycle<AccountId, AssetId> for RuntimeStakedAssetLifecycle {
  fn register(
    asset_id: AssetId,
    staked_asset_id: AssetId,
    admin: &AccountId,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    let (name, symbol, decimals) = if asset_id == NativeStakingAssetId::get() {
      (b"Staked Native Token".to_vec(), b"stNTVE".to_vec(), 12)
    } else {
      let base_name = <crate::Assets as MetadataInspect<AccountId>>::name(asset_id);
      let base_symbol = <crate::Assets as MetadataInspect<AccountId>>::symbol(asset_id);
      let decimals = <crate::Assets as MetadataInspect<AccountId>>::decimals(asset_id);
      let name = if base_name.is_empty() {
        format!("Staked Asset {}", asset_id).into_bytes()
      } else {
        let mut value = b"Staked ".to_vec();
        value.extend(base_name);
        value
      };
      let symbol = if base_symbol.is_empty() {
        format!("st{}", asset_id).into_bytes()
      } else {
        let mut value = b"st".to_vec();
        value.extend(base_symbol);
        value
      };
      (name, symbol, decimals)
    };
    crate::Assets::force_create(
      RuntimeOrigin::root(),
      staked_asset_id,
      polkadot_sdk::sp_runtime::MultiAddress::Id(admin.clone()),
      true,
      1,
    )?;
    crate::Assets::force_set_metadata(
      RuntimeOrigin::root(),
      staked_asset_id,
      name,
      symbol,
      decimals,
      false,
    )
  }
}

pub struct RuntimeRewardGovernanceDomainResolver;
impl pallet_staking::RewardGovernanceDomainResolver<AssetId, AssetId>
  for RuntimeRewardGovernanceDomainResolver
{
  fn reward_governance_domain(asset_id: AssetId) -> Option<AssetId> {
    Some(asset_id)
  }
}

pub struct RuntimeRewardEpochProvider;
impl pallet_staking::RewardEpochProvider<BlockNumber> for RuntimeRewardEpochProvider {
  fn current_reward_epoch() -> BlockNumber {
    crate::System::block_number()
  }
}

pub struct RuntimeRewardCoefficientProvider;
impl pallet_staking::RewardCoefficientProvider<AccountId, AssetId>
  for RuntimeRewardCoefficientProvider
{
  fn reward_coefficient(domain: AssetId, account: &AccountId) -> FixedU128 {
    crate::Governance::reward_coefficient(domain, account.clone())
  }

  #[cfg(feature = "runtime-benchmarks")]
  fn benchmark_prepare_positive_coefficient(domain: AssetId, account: &AccountId) {
    let item_id = crate::System::block_number();
    let _ = crate::Governance::ingest_winning_vote_resolution(domain, item_id, account.clone());
  }
}

pub struct RuntimeRewardSnapshotEventIngress;
impl pallet_staking::RewardSnapshotEventIngress<BlockNumber> for RuntimeRewardSnapshotEventIngress {
  fn ingest(epoch: BlockNumber, max_scan: usize) -> Weight {
    const EVENT_SCAN_WEIGHT_REF_TIME: u64 = 1_000;
    const ACCOUNT_TOUCH_WEIGHT_REF_TIME: u64 = 2_000;
    const REWARD_RECORD_WEIGHT_REF_TIME: u64 = 4_000;
    fn reward_base_asset_id(asset_id: AssetId) -> Option<AssetId> {
      pallet_staking::Pools::<Runtime>::iter_keys()
        .find(|base_asset_id| crate::Staking::staked_asset_id(*base_asset_id) == Some(asset_id))
    }
    fn reward_inflow_asset_id(asset_id: AssetId, recipient: &AccountId) -> Option<AssetId> {
      if !pallet_staking::Pools::<Runtime>::contains_key(asset_id) {
        return None;
      }
      if crate::Staking::reward_account_for(asset_id) != *recipient {
        return None;
      }
      Some(asset_id)
    }
    let mut scanned = 0u64;
    let mut touched = 0u64;
    let mut recorded_reward_inflows = 0u64;
    let mut truncated = false;
    let mut pending_reward_touches: BTreeSet<(AssetId, AccountId)> = BTreeSet::new();
    let mut pending_reward_inflows: BTreeMap<AssetId, Balance> = BTreeMap::new();
    for record in crate::System::read_events_no_consensus().take(max_scan.saturating_add(1)) {
      scanned = scanned.saturating_add(1);
      if scanned > max_scan as u64 {
        truncated = true;
        break;
      }
      match &record.event {
        crate::RuntimeEvent::Assets(AssetsEvent::Transferred {
          asset_id,
          from,
          to,
          amount,
        })
        | crate::RuntimeEvent::Assets(AssetsEvent::TransferredApproved {
          asset_id,
          owner: from,
          destination: to,
          amount,
          ..
        }) => {
          if let Some(reward_asset_id) = reward_inflow_asset_id(*asset_id, to) {
            pending_reward_inflows
              .entry(reward_asset_id)
              .and_modify(|value| *value = value.saturating_add(*amount))
              .or_insert(*amount);
          }
          let Some(base_asset_id) = reward_base_asset_id(*asset_id) else {
            continue;
          };
          pending_reward_touches.insert((base_asset_id, from.clone()));
          pending_reward_touches.insert((base_asset_id, to.clone()));
        }
        crate::RuntimeEvent::Assets(
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
            pending_reward_inflows
              .entry(reward_asset_id)
              .and_modify(|value| *value = value.saturating_add(*amount))
              .or_insert(*amount);
          }
          let Some(base_asset_id) = reward_base_asset_id(*asset_id) else {
            continue;
          };
          pending_reward_touches.insert((base_asset_id, owner.clone()));
        }
        crate::RuntimeEvent::Assets(
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
          let Some(base_asset_id) = reward_base_asset_id(*asset_id) else {
            continue;
          };
          pending_reward_touches.insert((base_asset_id, owner.clone()));
        }
        crate::RuntimeEvent::Governance(
          GovernanceEvent::WinningVoteRecorded {
            domain, account, ..
          }
          | GovernanceEvent::WinningVoteWindowEvicted {
            domain, account, ..
          },
        ) => {
          for asset_id in pallet_staking::Pools::<Runtime>::iter_keys() {
            if crate::Staking::reward_governance_domain(asset_id) == Some(*domain) {
              pending_reward_touches.insert((asset_id, account.clone()));
            }
          }
        }
        _ => {}
      }
    }
    if truncated {
      crate::Staking::note_reward_ingress_truncated(epoch, scanned as u32, max_scan as u32);
    }
    for (asset_id, amount) in pending_reward_inflows {
      recorded_reward_inflows = recorded_reward_inflows.saturating_add(u64::from(
        crate::Staking::note_reward_inflow(asset_id, amount).is_ok(),
      ));
    }
    for (asset_id, account) in pending_reward_touches {
      touched = touched.saturating_add(u64::from(crate::Staking::note_reward_touch(
        asset_id, &account,
      )));
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

impl pallet_staking::Config for Runtime {
  type AdminOrigin = EnsureRoot<AccountId>;
  type AssetId = AssetId;
  type NativeStakingAssetId = NativeStakingAssetId;
  type GovernanceDomainId = AssetId;
  type RewardEpoch = BlockNumber;
  type NativeBindingTargetValidator = RuntimeNativeBindingTargetValidator;
  type StakedAssetIdResolver = RuntimeStakedAssetIdResolver;
  type StakedAssetLifecycle = RuntimeStakedAssetLifecycle;
  type RewardGovernanceDomainResolver = RuntimeRewardGovernanceDomainResolver;
  type RewardEpochProvider = RuntimeRewardEpochProvider;
  type RewardCoefficientProvider = RuntimeRewardCoefficientProvider;
  type RewardSnapshotEventIngress = RuntimeRewardSnapshotEventIngress;
  type MaxOperatorCommission = MaxOperatorCommission;
  type MaxRewardEventScanPerBlock = MaxRewardEventScanPerBlock;
  type MaxRewardAccountsPerAssetEpoch = MaxRewardAccountsPerAssetEpoch;
  type MaxClaimEpochsPerCall = MaxClaimEpochsPerCall;
  type Balance = Balance;
  type Assets = crate::Assets;
  type PalletId = StakingPalletId;
  type WeightInfo = crate::weights::pallet_staking::SubstrateWeight<Runtime>;
}
