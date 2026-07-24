use super::assets_config::AssetId;
use super::*;

#[cfg(feature = "runtime-benchmarks")]
use alloc::boxed::Box;
use alloc::{
  collections::{BTreeMap, BTreeSet},
  format,
};
use pallet_governance::Event as GovernanceEvent;
use polkadot_sdk::frame_support::traits::fungibles::metadata::Inspect as MetadataInspect;
use polkadot_sdk::{
  frame_support::{PalletId, parameter_types, weights::Weight},
  frame_system::EnsureRoot,
  pallet_asset_conversion::PoolLocator,
  pallet_assets::Event as AssetsEvent,
  sp_runtime::FixedU128,
};
#[cfg(test)]
use std::cell::Cell;

parameter_types! {
  pub const StakingPalletId: PalletId = PalletId(*primitives::ecosystem::pallet_ids::STAKING_PALLET_ID);
  pub const NativeStakingAssetId: AssetId = 0;
  pub const MaxOperatorCommission: polkadot_sdk::sp_runtime::Perbill = polkadot_sdk::sp_runtime::Perbill::from_percent(50);
  pub const MaxRewardEventScanPerBlock: u32 = 128;
  pub const MaxRewardRolloverAssetsPerBlock: u32 = 32;
  pub const MaxRewardAccountsPerAssetEpoch: u32 = 256;
  pub const MaxRewardAssetsPerGovernanceDomain: u32 = 16;
  pub const MaxClaimEpochsPerCall: u32 = 16;
  pub const NativeLpUnlockDelay: BlockNumber = 7 * 24 * HOURS;
}

pub(crate) const REWARD_INGRESS_EVENT_SCAN_WEIGHT_REF_TIME: u64 = 1_000;
pub(crate) const REWARD_INGRESS_ACCOUNT_TOUCH_WEIGHT_REF_TIME: u64 = 2_000;
pub(crate) const REWARD_INGRESS_RECORD_WEIGHT_REF_TIME: u64 = 4_000;

#[cfg(test)]
thread_local! {
  static REWARD_INGRESS_RECEIPT_BASE_LOOKUP_COUNT: Cell<u32> = const { Cell::new(0) };
  static REWARD_INGRESS_GOVERNANCE_DOMAIN_LOOKUP_COUNT: Cell<u32> = const { Cell::new(0) };
}

#[cfg(test)]
pub fn reset_reward_ingress_lookup_probes() {
  REWARD_INGRESS_RECEIPT_BASE_LOOKUP_COUNT.with(|count| count.set(0));
  REWARD_INGRESS_GOVERNANCE_DOMAIN_LOOKUP_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub fn reward_ingress_receipt_base_lookup_probe_count() -> u32 {
  REWARD_INGRESS_RECEIPT_BASE_LOOKUP_COUNT.with(Cell::get)
}

#[cfg(test)]
pub fn reward_ingress_governance_domain_lookup_probe_count() -> u32 {
  REWARD_INGRESS_GOVERNANCE_DOMAIN_LOOKUP_COUNT.with(Cell::get)
}

pub(crate) fn reward_ingress_expected_ref_time(
  scanned: u64,
  touched: u64,
  recorded_inflows: u64,
) -> u64 {
  scanned
    .saturating_mul(REWARD_INGRESS_EVENT_SCAN_WEIGHT_REF_TIME)
    .saturating_add(touched.saturating_mul(REWARD_INGRESS_ACCOUNT_TOUCH_WEIGHT_REF_TIME))
    .saturating_add(recorded_inflows.saturating_mul(REWARD_INGRESS_RECORD_WEIGHT_REF_TIME))
}

#[cfg(test)]
fn note_reward_ingress_receipt_base_lookup() {
  REWARD_INGRESS_RECEIPT_BASE_LOOKUP_COUNT.with(|count| count.set(count.get().saturating_add(1)));
}

#[cfg(test)]
fn note_reward_ingress_governance_domain_lookup() {
  REWARD_INGRESS_GOVERNANCE_DOMAIN_LOOKUP_COUNT
    .with(|count| count.set(count.get().saturating_add(1)));
}

pub struct RuntimeNativeOperatorValidator;
impl pallet_staking::NativeOperatorValidator<AccountId> for RuntimeNativeOperatorValidator {
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

pub struct RuntimeNativeLpAssetNamespaceInitializer;
impl pallet_staking::NativeLpAssetNamespaceInitializer
  for RuntimeNativeLpAssetNamespaceInitializer
{
  fn ensure_namespace() {
    crate::configs::AssetConversionAdapter::ensure_lp_asset_namespace();
  }
}

pub struct RuntimeNativeStakingLpAssetValidator;
impl pallet_staking::NativeStakingLpAssetValidator<AssetId>
  for RuntimeNativeStakingLpAssetValidator
{
  fn is_valid_native_staking_lp_asset(asset_id: AssetId) -> bool {
    let native_asset_id = NativeStakingAssetId::get();
    let Some(staked_asset_id) = crate::Staking::staked_asset_id(native_asset_id) else {
      return false;
    };
    let base_asset = primitives::AssetKind::Local(native_asset_id);
    let staked_asset = primitives::AssetKind::Local(staked_asset_id);
    let Ok(pool_id) = <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(
      &base_asset,
      &staked_asset,
    ) else {
      return false;
    };
    polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get(pool_id)
      .map(|pool| pool.lp_token == asset_id)
      .unwrap_or(false)
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

pub struct RuntimeRewardBaseWeightProvider;
impl pallet_staking::RewardBaseWeightProvider<AccountId, AssetId, Balance>
  for RuntimeRewardBaseWeightProvider
{
  fn reward_base_weight(asset_id: AssetId, account: &AccountId) -> Option<Balance> {
    if asset_id != NativeStakingAssetId::get() {
      return None;
    }
    Some(DelegationWeightedCollatorSessionManager::native_nomination_reward_base_weight(account))
  }
}

pub struct RuntimeLegacyRewardSnapshotEventIngress;
impl pallet_staking::RewardSnapshotEventIngress<BlockNumber>
  for RuntimeLegacyRewardSnapshotEventIngress
{
  fn ingest(epoch: BlockNumber, max_scan: usize, remaining_weight: Weight) -> Weight {
    fn reward_base_asset_id(asset_id: AssetId) -> Option<AssetId> {
      #[cfg(test)]
      note_reward_ingress_receipt_base_lookup();
      let base_asset_id = crate::Staking::live_base_asset_for_staked_asset(asset_id)?;
      if base_asset_id == NativeStakingAssetId::get() {
        return None;
      }
      Some(base_asset_id)
    }
    fn reward_assets_for_governance_domain(domain: AssetId) -> alloc::vec::Vec<AssetId> {
      #[cfg(test)]
      note_reward_ingress_governance_domain_lookup();
      crate::Staking::reward_assets_for_governance_domain(domain)
        .into_inner()
        .into_iter()
        .filter(|asset_id| *asset_id != NativeStakingAssetId::get())
        .collect()
    }
    fn reward_inflow_asset_id(asset_id: AssetId, recipient: &AccountId) -> Option<AssetId> {
      if asset_id == NativeStakingAssetId::get() {
        return None;
      }
      if !pallet_staking::Pools::<Runtime>::contains_key(asset_id) {
        return None;
      }
      if crate::Staking::reward_account_for(asset_id) != *recipient {
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
    for record in crate::System::read_events_no_consensus().take(max_scan.saturating_add(1)) {
      let next_scanned = scanned.saturating_add(1);
      let scan_only_ref_time = reward_ingress_expected_ref_time(
        next_scanned,
        pending_reward_touches.len() as u64,
        pending_reward_inflows.len() as u64,
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
            projected_inflows = projected_inflows.saturating_add(usize::from(
              !pending_reward_inflows.contains_key(&reward_asset_id),
            ));
          }
          if let Some(base_asset_id) = reward_base_asset_id(*asset_id) {
            projected_touches = projected_touches.saturating_add(usize::from(
              !pending_reward_touches.contains(&(base_asset_id, from.clone())),
            ));
            projected_touches = projected_touches.saturating_add(usize::from(
              !pending_reward_touches.contains(&(base_asset_id, to.clone())),
            ));
          }
          let projected_ref_time = reward_ingress_expected_ref_time(
            scanned,
            projected_touches as u64,
            projected_inflows as u64,
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
            projected_inflows = projected_inflows.saturating_add(usize::from(
              !pending_reward_inflows.contains_key(&reward_asset_id),
            ));
          }
          if let Some(base_asset_id) = reward_base_asset_id(*asset_id) {
            projected_touches = projected_touches.saturating_add(usize::from(
              !pending_reward_touches.contains(&(base_asset_id, owner.clone())),
            ));
          }
          let projected_ref_time = reward_ingress_expected_ref_time(
            scanned,
            projected_touches as u64,
            projected_inflows as u64,
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
          if let Some(base_asset_id) = reward_base_asset_id(*asset_id) {
            projected_touches = projected_touches.saturating_add(usize::from(
              !pending_reward_touches.contains(&(base_asset_id, owner.clone())),
            ));
          }
          let projected_ref_time = reward_ingress_expected_ref_time(
            scanned,
            projected_touches as u64,
            projected_inflows as u64,
          );
          if projected_ref_time > max_ref_time {
            truncated = true;
            break;
          }
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
          let reward_assets = reward_assets_for_governance_domain(*domain);
          projected_touches =
            reward_assets
              .clone()
              .into_iter()
              .fold(projected_touches, |count, asset_id| {
                count.saturating_add(usize::from(
                  !pending_reward_touches.contains(&(asset_id, account.clone())),
                ))
              });
          let projected_ref_time = reward_ingress_expected_ref_time(
            scanned,
            projected_touches as u64,
            projected_inflows as u64,
          );
          if projected_ref_time > max_ref_time {
            truncated = true;
            break;
          }
          reward_assets.into_iter().for_each(|asset_id| {
            pending_reward_touches.insert((asset_id, account.clone()));
          });
        }
        _ => {
          let projected_ref_time = reward_ingress_expected_ref_time(
            scanned,
            projected_touches as u64,
            projected_inflows as u64,
          );
          if projected_ref_time > max_ref_time {
            truncated = true;
            break;
          }
        }
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
      reward_ingress_expected_ref_time(scanned, touched, recorded_reward_inflows),
      0,
    )
  }
}

#[cfg(feature = "runtime-benchmarks")]
pub struct RuntimeStakingBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_staking::BenchmarkHelper<AccountId, AssetId, Balance>
  for RuntimeStakingBenchmarkHelper
{
  fn prepare_native_staking_lp(
    account: &AccountId,
    amount: Balance,
  ) -> Result<AssetId, polkadot_sdk::sp_runtime::DispatchError> {
    use polkadot_sdk::frame_support::traits::Currency;
    let native_asset_id = NativeStakingAssetId::get();
    let owner = account.clone();
    if !<crate::Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<AccountId>>::asset_exists(native_asset_id) {
      crate::Assets::force_create(
        crate::RuntimeOrigin::root(),
        native_asset_id,
        owner.clone().into(),
        true,
        1,
      )?;
    }
    let _ = crate::Staking::register_staking_asset(crate::RuntimeOrigin::root(), native_asset_id);
    let staked_asset_id = crate::Staking::staked_asset_id(native_asset_id).ok_or(
      polkadot_sdk::sp_runtime::DispatchError::Other("MissingStakedAsset"),
    )?;
    let liquidity_seed = amount.saturating_mul(1_000).max(1_000_000_000_000);
    let mint_amount = liquidity_seed.saturating_mul(4);
    <crate::Assets as polkadot_sdk::frame_support::traits::fungibles::Mutate<AccountId>>::mint_into(
      native_asset_id,
      account,
      mint_amount,
    )?;
    let _ = crate::Balances::deposit_creating(account, mint_amount);
    let _ = crate::Staking::stake_native(
      crate::RuntimeOrigin::signed(account.clone()),
      mint_amount / 2,
    )?;
    crate::configs::AssetConversionAdapter::ensure_lp_asset_namespace();
    let base_asset = primitives::AssetKind::Local(native_asset_id);
    let staked_asset = primitives::AssetKind::Local(staked_asset_id);
    let _ = crate::AssetConversion::create_pool(
      crate::RuntimeOrigin::signed(account.clone()),
      Box::new(base_asset),
      Box::new(staked_asset),
    );
    crate::configs::assets_config::register_pool_lp_pair(base_asset, staked_asset)?;
    crate::AssetConversion::add_liquidity(
      crate::RuntimeOrigin::signed(account.clone()),
      Box::new(base_asset),
      Box::new(staked_asset),
      liquidity_seed,
      liquidity_seed,
      0,
      0,
      account.clone(),
    )?;
    let pool_id = <Runtime as pallet_asset_conversion::Config>::PoolLocator::pool_id(
      &base_asset,
      &staked_asset,
    )
    .map_err(|_| {
      polkadot_sdk::sp_runtime::DispatchError::Other("NativeStakingPoolIdUnavailable")
    })?;
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get(pool_id).ok_or(
      polkadot_sdk::sp_runtime::DispatchError::Other("MissingNativeStakingPool"),
    )?;
    Ok(pool.lp_token)
  }

  fn prepare_native_governance_asset(
    account: &AccountId,
    amount: Balance,
  ) -> Result<AssetId, polkadot_sdk::sp_runtime::DispatchError> {
    let native_asset_id = NativeStakingAssetId::get();
    if !<crate::Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<AccountId>>::asset_exists(native_asset_id) {
      crate::Assets::force_create(
        crate::RuntimeOrigin::root(),
        native_asset_id,
        account.clone().into(),
        true,
        1,
      )?;
    }
    let _ = crate::Staking::register_staking_asset(crate::RuntimeOrigin::root(), native_asset_id);
    <crate::Assets as polkadot_sdk::frame_support::traits::fungibles::Mutate<AccountId>>::mint_into(
      native_asset_id,
      account,
      amount,
    )?;
    Ok(native_asset_id)
  }
}

pub struct RuntimeNativeGovernanceLockProvider;
impl pallet_staking::NativeGovernanceLockProvider<AccountId, BlockNumber>
  for RuntimeNativeGovernanceLockProvider
{
  fn lock_until(account: &AccountId) -> Option<BlockNumber> {
    crate::Governance::governance_lock(account).map(|lock| lock.lock_until)
  }
}

pub struct RuntimeNativeNominationRewardCompounder;
impl pallet_staking::NativeNominationRewardCompounder<AccountId, Balance>
  for RuntimeNativeNominationRewardCompounder
{
  fn compound(
    account: &AccountId,
    operator: &AccountId,
    amount: Balance,
  ) -> Result<Balance, polkadot_sdk::sp_runtime::DispatchError> {
    crate::configs::AssetConversionAdapter::compound_native_nomination_reward_to_locked_lp(
      account, operator, amount,
    )
  }
}

pub struct RuntimeNativeStakingReadModelProvider;
impl pallet_staking::NativeStakingReadModelProvider<AssetId, Balance>
  for RuntimeNativeStakingReadModelProvider
{
  fn native_staking_liquidity_pool() -> Option<(AssetId, Balance, Balance, Balance)> {
    crate::configs::AssetConversionAdapter::native_staking_liquidity_pool_read_model()
  }

  fn native_lp_value(locked_lp: Balance) -> Option<Balance> {
    Some(DelegationWeightedCollatorSessionManager::conservative_native_lp_value(locked_lp))
  }
}

impl pallet_staking::Config for Runtime {
  type AdminOrigin = EnsureRoot<AccountId>;
  type AssetId = AssetId;
  type NativeStakingAssetId = NativeStakingAssetId;
  type GovernanceDomainId = AssetId;
  type RewardEpoch = BlockNumber;
  type NativeOperatorValidator = RuntimeNativeOperatorValidator;
  type NativeStakingLpAssetValidator = RuntimeNativeStakingLpAssetValidator;
  type NativeLpAssetNamespaceInitializer = RuntimeNativeLpAssetNamespaceInitializer;
  type NativeGovernanceLockProvider = RuntimeNativeGovernanceLockProvider;
  type StakedAssetIdResolver = RuntimeStakedAssetIdResolver;
  type StakedAssetLifecycle = RuntimeStakedAssetLifecycle;
  type RewardGovernanceDomainResolver = RuntimeRewardGovernanceDomainResolver;
  type RewardEpochProvider = RuntimeRewardEpochProvider;
  type RewardCoefficientProvider = RuntimeRewardCoefficientProvider;
  type RewardBaseWeightProvider = RuntimeRewardBaseWeightProvider;
  type NativeNominationRewardCompounder = RuntimeNativeNominationRewardCompounder;
  type NativeStakingReadModelProvider = RuntimeNativeStakingReadModelProvider;
  type RewardSnapshotEventIngress = RuntimeLegacyRewardSnapshotEventIngress;
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = RuntimeStakingBenchmarkHelper;
  type MaxOperatorCommission = MaxOperatorCommission;
  type MaxRewardEventScanPerBlock = MaxRewardEventScanPerBlock;
  type MaxRewardRolloverAssetsPerBlock = MaxRewardRolloverAssetsPerBlock;
  type MaxRewardAccountsPerAssetEpoch = MaxRewardAccountsPerAssetEpoch;
  type MaxRewardAssetsPerGovernanceDomain = MaxRewardAssetsPerGovernanceDomain;
  type MaxClaimEpochsPerCall = MaxClaimEpochsPerCall;
  type NativeLpUnlockDelay = NativeLpUnlockDelay;
  type Balance = Balance;
  type Assets = crate::Assets;
  type PalletId = StakingPalletId;
  type WeightInfo = crate::weights::pallet_staking::SubstrateWeight<Runtime>;
}
