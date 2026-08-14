use super::assets_config::AssetId;
use super::*;

use alloc::{boxed::Box, format};
use polkadot_sdk::frame_support::traits::fungibles::metadata::Inspect as MetadataInspect;
use polkadot_sdk::{
  frame_support::{PalletId, parameter_types},
  frame_system::EnsureRoot,
  pallet_asset_conversion::PoolLocator,
  sp_core::U256,
  sp_runtime::{DispatchError, FixedU128, PerThing, Perbill, traits::Zero},
};
parameter_types! {
  pub const StakingPalletId: PalletId = PalletId(*primitives::ecosystem::pallet_ids::STAKING_PALLET_ID);
  pub const NativeStakingAssetId: AssetId = 0;
  pub const NativeGovernanceDomainId: AssetId = 0;
  pub SecurityRewardFundingSource: AccountId = crate::Actors::sovereign_account_id_system(
    primitives::ecosystem::actor_ids::FEE_SINK_ACTORS_ID,
  );
  pub const MaxNativeSecurityParticipants: u32 = 100;
  pub const MaxNativeSecurityOperators: u32 = 100;
  pub const MaxNominationsPerAccount: u32 = 16;
  pub const NativeLpUnlockDelay: BlockNumber = 7 * 24 * HOURS;
  pub const SecurityRewardClaimHorizon: u32 = 12;
  pub const MaxSecurityRewardClaimsPerCall: u32 = 12;
  pub NativeSecurityCompoundMaxRatioDeviation: Perbill = Perbill::from_percent(1);
}

pub struct RuntimeNativeSecurityModeProvider;
impl pallet_staking::NativeSecurityModeProvider for RuntimeNativeSecurityModeProvider {
  fn mode() -> pallet_staking::NativeSecurityMode {
    #[cfg(feature = "runtime-benchmarks")]
    return pallet_staking::NativeSecurityMode::LpBackedSelection;
    #[cfg(not(feature = "runtime-benchmarks"))]
    pallet_staking::NativeSecurityMode::TrustedSet
  }

  #[cfg(feature = "runtime-benchmarks")]
  fn benchmark_prepare_lp_backed_selection() {}
}

pub struct RuntimeNativeOperatorValidator;
impl pallet_staking::NativeOperatorValidator<AccountId> for RuntimeNativeOperatorValidator {
  fn is_valid_operator(account: &AccountId) -> bool {
    if pallet_collator_selection::Invulnerables::<Runtime>::get().contains(account) {
      return true;
    }
    <RuntimeNativeSecurityModeProvider as pallet_staking::NativeSecurityModeProvider>::mode()
      == pallet_staking::NativeSecurityMode::LpBackedSelection
      && pallet_collator_selection::CandidateList::<Runtime>::get()
        .iter() // deos-bypass: bounded-iter — collator-selection MaxCandidates
        .any(|candidate| &candidate.who == account)
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

  #[cfg(feature = "runtime-benchmarks")]
  fn benchmark_prepare_snapshot_operator(account: &AccountId) {
    use polkadot_sdk::frame_support::BoundedVec;
    use polkadot_sdk::pallet_collator_selection::CandidateInfo;
    let mut candidates = pallet_collator_selection::CandidateList::<Runtime>::get().into_inner();
    if candidates
      .iter() // deos-bypass: bounded-iter — runtime-benchmarks only, collator-selection MaxCandidates
      .any(|candidate| &candidate.who == account)
    {
      return;
    }
    candidates.push(CandidateInfo {
      who: account.clone(),
      deposit: Balance::default(),
    });
    pallet_collator_selection::CandidateList::<Runtime>::put(BoundedVec::truncate_from(candidates));
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

pub struct RuntimeSecurityEpochProvider;
impl pallet_staking::SecurityEpochProvider for RuntimeSecurityEpochProvider {
  fn current_security_epoch() -> pallet_staking::SecurityEpoch {
    crate::Session::current_index()
  }
}

pub struct RuntimeGovernanceParticipationCoefficientProvider;
impl pallet_staking::GovernanceParticipationCoefficientProvider<AccountId, AssetId>
  for RuntimeGovernanceParticipationCoefficientProvider
{
  fn governance_participation_coefficient(domain: AssetId, account: &AccountId) -> FixedU128 {
    crate::Governance::governance_participation_coefficient(domain, account.clone())
  }

  #[cfg(feature = "runtime-benchmarks")]
  fn benchmark_prepare_positive_coefficient(domain: AssetId, account: &AccountId) {
    let item_id = crate::System::block_number();
    let _ = crate::Governance::ingest_winning_vote_resolution(domain, item_id, account.clone());
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

  fn set_security_epoch(epoch: pallet_staking::SecurityEpoch) {
    pallet_session::CurrentIndex::<Runtime>::put(epoch);
  }

  fn fund_native_account(account: &AccountId, amount: Balance) {
    crate::Balances::force_set_balance(
      crate::RuntimeOrigin::root(),
      account.clone().into(),
      amount,
    )
    .expect("benchmark native account funding must succeed");
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

pub struct RuntimeNativeSecurityRewardCompound;
impl pallet_staking::NativeSecurityRewardCompound<AccountId, AssetId, Balance>
  for RuntimeNativeSecurityRewardCompound
{
  fn compound(
    account: &AccountId,
    reward: Balance,
    min_lp_out: Balance,
  ) -> Result<(AssetId, Balance), DispatchError> {
    use polkadot_sdk::frame_support::traits::{
      Currency,
      fungibles::{Inspect, Mutate},
    };

    let native_asset_id = NativeStakingAssetId::get();
    let staked_asset_id = crate::Staking::staked_asset_id(native_asset_id)
      .ok_or(DispatchError::Other("StakedAssetUnavailable"))?;
    let base_asset = primitives::AssetKind::Local(native_asset_id);
    let staked_asset = primitives::AssetKind::Local(staked_asset_id);
    let (lp_asset_id, reserve_native, reserve_staked, _) =
      crate::configs::AssetConversionAdapter::native_staking_liquidity_pool_read_model().ok_or(
        DispatchError::Other("NativeStakingLiquidityPoolUnavailable"),
      )?;
    let staking_pool = pallet_staking::Pools::<Runtime>::get(native_asset_id)
      .ok_or(DispatchError::Other("NativeStakingPoolUnavailable"))?;
    if reward.is_zero()
      || reserve_native.is_zero()
      || reserve_staked.is_zero()
      || staking_pool.accounted_balance.is_zero()
      || staking_pool.total_shares.is_zero()
    {
      return Err(DispatchError::Other(
        "NativeSecurityCompoundStateUnavailable",
      ));
    }
    let numerator = U256::from(reserve_staked)
      .checked_mul(U256::from(reward))
      .and_then(|value| value.checked_mul(U256::from(staking_pool.accounted_balance)))
      .ok_or(DispatchError::Other("NativeSecurityCompoundOverflow"))?;
    let denominator = U256::from(reserve_staked)
      .checked_mul(U256::from(staking_pool.accounted_balance))
      .and_then(|left| {
        U256::from(reserve_native)
          .checked_mul(U256::from(staking_pool.total_shares))
          .and_then(|right| left.checked_add(right))
      })
      .ok_or(DispatchError::Other("NativeSecurityCompoundOverflow"))?;
    let stake_amount: Balance = numerator
      .checked_div(denominator)
      .ok_or(DispatchError::Other("NativeSecurityCompoundOverflow"))?
      .try_into()
      .map_err(|_| DispatchError::Other("NativeSecurityCompoundOverflow"))?;
    let native_liquidity = reward
      .checked_sub(stake_amount)
      .ok_or(DispatchError::Other("NativeSecurityCompoundOverflow"))?;
    if stake_amount.is_zero() || native_liquidity.is_zero() {
      return Err(DispatchError::Other("NativeSecurityCompoundAmountTooSmall"));
    }

    let (_, unslashed) = <crate::Balances as Currency<AccountId>>::slash(account, reward);
    if !unslashed.is_zero() {
      return Err(DispatchError::Other(
        "NativeSecurityCompoundNativeUnavailable",
      ));
    }
    <crate::Assets as Mutate<AccountId>>::mint_into(native_asset_id, account, reward)?;
    let staked_before = <crate::Assets as Inspect<AccountId>>::balance(staked_asset_id, account);
    crate::Staking::stake_native(crate::RuntimeOrigin::signed(account.clone()), stake_amount)?;
    let staked_out = <crate::Assets as Inspect<AccountId>>::balance(staked_asset_id, account)
      .checked_sub(staked_before)
      .ok_or(DispatchError::Other("NativeSecurityCompoundOverflow"))?;
    if staked_out.is_zero() {
      return Err(DispatchError::Other("NativeSecurityCompoundAmountTooSmall"));
    }
    let left = U256::from(native_liquidity)
      .checked_mul(U256::from(reserve_staked))
      .ok_or(DispatchError::Other("NativeSecurityCompoundOverflow"))?;
    let right = U256::from(staked_out)
      .checked_mul(U256::from(reserve_native))
      .ok_or(DispatchError::Other("NativeSecurityCompoundOverflow"))?;
    let difference = left.abs_diff(right);
    let allowed = NativeSecurityCompoundMaxRatioDeviation::get() * left.max(right);
    if difference > allowed {
      return Err(DispatchError::Other("NativeSecurityCompoundRatioExceeded"));
    }
    let lp_before = <crate::Assets as Inspect<AccountId>>::balance(lp_asset_id, account);
    let min_native =
      NativeSecurityCompoundMaxRatioDeviation::get().left_from_one() * native_liquidity;
    let min_staked = NativeSecurityCompoundMaxRatioDeviation::get().left_from_one() * staked_out;
    crate::AssetConversion::add_liquidity(
      crate::RuntimeOrigin::signed(account.clone()),
      Box::new(base_asset),
      Box::new(staked_asset),
      native_liquidity,
      staked_out,
      min_native,
      min_staked,
      account.clone(),
    )?;
    let lp_out = <crate::Assets as Inspect<AccountId>>::balance(lp_asset_id, account)
      .checked_sub(lp_before)
      .ok_or(DispatchError::Other("NativeSecurityCompoundOverflow"))?;
    if lp_out < min_lp_out {
      return Err(DispatchError::Other("NativeSecurityCompoundMinimumNotMet"));
    }
    Ok((lp_asset_id, lp_out))
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
    DelegationWeightedCollatorSessionManager::try_conservative_native_lp_value(locked_lp)
  }

  fn native_security_topology_readiness() -> Option<pallet_staking::NativeSecurityReadiness> {
    DelegationWeightedCollatorSessionManager::native_security_topology_readiness()
  }
}

impl pallet_staking::Config for Runtime {
  type AdminOrigin = EnsureRoot<AccountId>;
  type AssetId = AssetId;
  type NativeStakingAssetId = NativeStakingAssetId;
  type NativeCurrency = crate::Balances;
  type SecurityRewardFundingOrigin = EnsureRoot<AccountId>;
  type SecurityRewardFundingSource = SecurityRewardFundingSource;
  type GovernanceDomainId = AssetId;
  type NativeGovernanceDomainId = NativeGovernanceDomainId;
  type NativeOperatorValidator = RuntimeNativeOperatorValidator;
  type NativeStakingLpAssetValidator = RuntimeNativeStakingLpAssetValidator;
  type NativeLpAssetNamespaceInitializer = RuntimeNativeLpAssetNamespaceInitializer;
  type NativeGovernanceLockProvider = RuntimeNativeGovernanceLockProvider;
  type NativeSecurityModeProvider = RuntimeNativeSecurityModeProvider;
  type StakedAssetIdResolver = RuntimeStakedAssetIdResolver;
  type StakedAssetLifecycle = RuntimeStakedAssetLifecycle;
  type SecurityEpochProvider = RuntimeSecurityEpochProvider;
  type GovernanceParticipationCoefficientProvider =
    RuntimeGovernanceParticipationCoefficientProvider;
  type NativeStakingReadModelProvider = RuntimeNativeStakingReadModelProvider;
  type NativeSecurityRewardCompound = RuntimeNativeSecurityRewardCompound;
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = RuntimeStakingBenchmarkHelper;
  type MaxNativeSecurityParticipants = MaxNativeSecurityParticipants;
  type MaxNativeSecurityOperators = MaxNativeSecurityOperators;
  type MaxNominationsPerAccount = MaxNominationsPerAccount;
  type NativeLpUnlockDelay = NativeLpUnlockDelay;
  type SecurityRewardClaimHorizon = SecurityRewardClaimHorizon;
  type MaxSecurityRewardClaimsPerCall = MaxSecurityRewardClaimsPerCall;
  type Balance = Balance;
  type Assets = crate::Assets;
  type PalletId = StakingPalletId;
  type WeightInfo = crate::weights::pallet_staking::SubstrateWeight<Runtime>;
}
