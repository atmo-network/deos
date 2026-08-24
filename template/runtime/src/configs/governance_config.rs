use super::assets_config::AssetId;
use super::preimage_config::{PreimageBaseDeposit, PreimageByteDeposit};
use super::*;

use codec::{Decode, Encode, MaxEncodedLen};
use polkadot_sdk::frame_support::traits::UnfilteredDispatchable;
use polkadot_sdk::frame_support::{
  parameter_types,
  traits::{PreimageProvider, QueryPreimage},
};
use polkadot_sdk::frame_system::{EnsureRoot, RawOrigin};
#[allow(unused_imports)]
use polkadot_sdk::sp_runtime::traits::Hash as _;
use scale_info::TypeInfo;

parameter_types! {
  pub const WinningVoteLookbackEpochs: u32 = 3;
  pub const MaxWinningVotesPerEpoch: u16 = 4;
  pub const MaxWinningVoteItemsPerEpoch: u32 = 4;
  pub const MaxWinningVoteResolutionItemsPerEpoch: u32 = 64;
  pub const MaxWinningVoteAccountsPerCall: u32 = 256;
  pub const MaxActiveProposalsPerDomain: u32 = 128;
  pub const StrategicProposalReserve: u32 = 1;
  pub const MaxActiveProposalsPerAuthor: u32 = 16;
  pub const MaxMaturingProposalsPerEpoch: u32 = 4;
  pub const MaxPendingEnactmentsPerEpoch: u32 = 4;
  pub const ProposalVotingPeriod: BlockNumber = 7 * 24 * HOURS;
  pub const ProposalLeadInPeriod: BlockNumber = 3 * 24 * HOURS;
  pub const ProposalProtectionPeriod: BlockNumber = 7 * 24 * HOURS;
  pub const ProposalUrgentVotingPeriod: BlockNumber = 24 * HOURS;
  pub const ProposalEnactmentDelay: BlockNumber = 3 * 24 * HOURS;
  pub const ProposalOpeningFee: Balance = 10 * EXISTENTIAL_DEPOSIT;
  pub const PayloadAdmissionWitnessDeposit: Balance = EXISTENTIAL_DEPOSIT;
  pub ProposalFastTrackPassThreshold: polkadot_sdk::sp_runtime::Perbill =
    polkadot_sdk::sp_runtime::Perbill::from_percent(100);
  pub ProposalApprovalThreshold: polkadot_sdk::sp_runtime::Perbill =
    polkadot_sdk::sp_runtime::Perbill::from_percent(60);
  pub ProposalVetoThreshold: polkadot_sdk::sp_runtime::Perbill =
    polkadot_sdk::sp_runtime::Perbill::from_percent(50);
  pub ProposalVetoMinimumVetoTurnout: polkadot_sdk::sp_runtime::Perbill =
    polkadot_sdk::sp_runtime::Perbill::from_percent(1);
  pub const ProposalMinimumTurnout: u64 = 200;
  pub const MaxGovernanceEpochCatchUpPerBlock: u32 = 1;
  pub const MaxGovernanceMaturingProposalsPerBlock: u32 = 2;
  pub const MaxGovernancePendingEnactmentsPerBlock: u32 = 4;
  pub const MaxGovernanceFinalizedOutcomesPerBlock: u32 = 1024;
  pub const MaxGovernanceExpiringAccountsPerBlock: u32 = 512;
  pub const FinalizedProposalOutcomeRetentionEpochs: u32 = 16;
  pub const MaxFinalizedProposalOutcomesPerEpoch: u32 = 1024;
  pub const MaxRecentFinalizedProposalsPerDomain: u32 = 16 * 1024;
  pub const MaxExpiringAccountsPerEpoch: u32 = 1024;
}

parameter_types! {
  /// Worst reachable one-block Governance hook branch: catch-up plus one bounded service phase.
  pub GovernanceFixedWeight: Weight = {
    type W = crate::weights::pallet_governance::SubstrateWeight<Runtime>;
    let phase = [
      <W as pallet_governance::WeightInfo>::service_maturing_proposals(
        MaxGovernanceMaturingProposalsPerBlock::get(),
      ),
      <W as pallet_governance::WeightInfo>::service_pending_enactments(
        MaxGovernancePendingEnactmentsPerBlock::get(),
      ),
      <W as pallet_governance::WeightInfo>::service_finalized_proposal_outcomes(
        MaxGovernanceFinalizedOutcomesPerBlock::get(),
      ),
      <W as pallet_governance::WeightInfo>::service_expiring_accounts(
        MaxGovernanceExpiringAccountsPerBlock::get(),
      ),
    ]
    .into_iter()
    .fold(Weight::zero(), |left, right| {
      Weight::from_parts(
        left.ref_time().max(right.ref_time()),
        left.proof_size().max(right.proof_size()),
      )
    });
    <W as pallet_governance::WeightInfo>::service_epoch_catch_up().saturating_add(phase)
  };
}

pub struct RuntimeGovernanceEpochProvider;
impl pallet_governance::EpochProvider<BlockNumber> for RuntimeGovernanceEpochProvider {
  fn current_epoch() -> BlockNumber {
    crate::System::block_number()
  }
}

#[cfg_attr(feature = "runtime-benchmarks", allow(dead_code))]
fn declining_power_weight<ItemId>(
  base_weight: u128,
  context: &pallet_governance::ProposalVoteContext<ItemId, BlockNumber>,
) -> u128 {
  let voting_period = context
    .maturity_epoch
    .saturating_sub(context.submitted_epoch)
    .max(1);
  let clamped_vote_epoch = context
    .vote_epoch
    .max(context.submitted_epoch)
    .min(context.maturity_epoch);
  let elapsed = clamped_vote_epoch.saturating_sub(context.submitted_epoch);
  let voting_period_u128 = u128::from(voting_period);
  let elapsed_u128 = u128::from(elapsed).min(voting_period_u128);
  if elapsed_u128.saturating_mul(7) >= voting_period_u128.saturating_mul(6) {
    return base_weight;
  }
  let multiplier_numerator = voting_period_u128
    .saturating_mul(7)
    .saturating_sub(elapsed_u128.saturating_mul(7));
  base_weight
    .saturating_mul(multiplier_numerator)
    .saturating_div(voting_period_u128)
}

#[derive(Clone, Copy)]
enum RuntimeGovernanceTrackBacking {
  DirectStake,
  VetoAsset,
  NativeStake,
}

#[derive(Clone, Copy)]
struct RuntimeGovernanceDomainPolicy {
  primary_track: RuntimeGovernanceTrackBacking,
  protection_track: RuntimeGovernanceTrackBacking,
}

fn governance_domain_policy(domain: AssetId) -> RuntimeGovernanceDomainPolicy {
  if domain == primitives::ecosystem::protocol_tokens::BLDR_ASSET_ID {
    RuntimeGovernanceDomainPolicy {
      primary_track: RuntimeGovernanceTrackBacking::DirectStake,
      protection_track: RuntimeGovernanceTrackBacking::NativeStake,
    }
  } else {
    RuntimeGovernanceDomainPolicy {
      primary_track: RuntimeGovernanceTrackBacking::DirectStake,
      protection_track: RuntimeGovernanceTrackBacking::VetoAsset,
    }
  }
}

parameter_types! {
  pub const GovernanceVotePowerCustodyPalletId: polkadot_sdk::frame_support::PalletId =
    polkadot_sdk::frame_support::PalletId(*primitives::ecosystem::pallet_ids::GOVERNANCE_CUSTODY_PALLET_ID);
}

pub fn governance_vote_power_custody_account() -> AccountId {
  use polkadot_sdk::sp_runtime::traits::AccountIdConversion;
  GovernanceVotePowerCustodyPalletId::get().into_account_truncating()
}

pub struct RuntimeVotePowerCustody;

#[cfg(test)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum VotePowerCustodyFault {
  LockAfterTransfer,
  UnlockAfterTransfer,
}

#[cfg(test)]
std::thread_local! {
  static VOTE_POWER_CUSTODY_FAULT: core::cell::Cell<Option<VotePowerCustodyFault>> = const { core::cell::Cell::new(None) };
}

#[cfg(test)]
pub(crate) fn set_vote_power_custody_fault(fault: Option<VotePowerCustodyFault>) {
  VOTE_POWER_CUSTODY_FAULT.with(|configured| configured.set(fault));
}

impl pallet_governance::VotePowerCustody<AccountId, AssetId, AssetId, Balance>
  for RuntimeVotePowerCustody
{
  fn lock_id(domain: AssetId, track: pallet_governance::ProposalTrackFamily) -> Option<AssetId> {
    let backing = match track {
      pallet_governance::ProposalTrackFamily::Ordinary => {
        governance_domain_policy(domain).primary_track
      }
      pallet_governance::ProposalTrackFamily::Veto => {
        governance_domain_policy(domain).protection_track
      }
    };
    match backing {
      RuntimeGovernanceTrackBacking::DirectStake => crate::Staking::staked_asset_id(domain),
      RuntimeGovernanceTrackBacking::VetoAsset => {
        Some(primitives::ecosystem::protocol_tokens::VETO_ASSET_ID)
      }
      RuntimeGovernanceTrackBacking::NativeStake => None,
    }
  }

  fn target_amount(
    domain: AssetId,
    track: pallet_governance::ProposalTrackFamily,
    account: &AccountId,
    current_locked: Balance,
  ) -> Result<Balance, polkadot_sdk::sp_runtime::DispatchError> {
    let Some(lock_id) = Self::lock_id(domain, track) else {
      return Ok(current_locked);
    };
    current_locked
      .checked_add(crate::Assets::balance(lock_id, account))
      .ok_or(polkadot_sdk::sp_runtime::DispatchError::Arithmetic(
        polkadot_sdk::sp_runtime::ArithmeticError::Overflow,
      ))
  }

  fn lock(
    account: &AccountId,
    lock_id: AssetId,
    amount: Balance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    use polkadot_sdk::frame_support::traits::{fungibles::Mutate, tokens::Preservation};
    <crate::Assets as Mutate<AccountId>>::transfer(
      lock_id,
      account,
      &governance_vote_power_custody_account(),
      amount,
      Preservation::Expendable,
    )?;
    #[cfg(test)]
    if VOTE_POWER_CUSTODY_FAULT
      .with(|configured| configured.get() == Some(VotePowerCustodyFault::LockAfterTransfer))
    {
      return Err(polkadot_sdk::sp_runtime::DispatchError::Other(
        "Forced custody lock failure after transfer",
      ));
    }
    Ok(())
  }

  fn unlock(
    account: &AccountId,
    lock_id: AssetId,
    amount: Balance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    use polkadot_sdk::frame_support::traits::{fungibles::Mutate, tokens::Preservation};
    <crate::Assets as Mutate<AccountId>>::transfer(
      lock_id,
      &governance_vote_power_custody_account(),
      account,
      amount,
      Preservation::Expendable,
    )?;
    #[cfg(test)]
    if VOTE_POWER_CUSTODY_FAULT
      .with(|configured| configured.get() == Some(VotePowerCustodyFault::UnlockAfterTransfer))
    {
      return Err(polkadot_sdk::sp_runtime::DispatchError::Other(
        "Forced custody unlock failure after transfer",
      ));
    }
    Ok(())
  }

  fn custodied_amount(lock_id: AssetId) -> Option<Balance> {
    Some(crate::Assets::balance(
      lock_id,
      &governance_vote_power_custody_account(),
    ))
  }
}

fn governance_track_power_profile_for_backing(
  backing: RuntimeGovernanceTrackBacking,
) -> pallet_governance::ProposalVotePowerProfile {
  match backing {
    RuntimeGovernanceTrackBacking::DirectStake => {
      pallet_governance::ProposalVotePowerProfile::DecliningDirectStake
    }
    RuntimeGovernanceTrackBacking::VetoAsset => {
      pallet_governance::ProposalVotePowerProfile::DecliningVetoAsset
    }
    RuntimeGovernanceTrackBacking::NativeStake => {
      pallet_governance::ProposalVotePowerProfile::DecliningNativeStake
    }
  }
}

fn proposal_has_urgent_authorization(domain: AssetId, item_id: u32) -> bool {
  crate::Governance::proposal_urgent_authorized_at(domain, item_id).is_some()
}

fn governance_track_power_profile(
  domain: AssetId,
  item_id: u32,
  track: pallet_governance::ProposalTrackFamily,
) -> pallet_governance::ProposalVotePowerProfile {
  let policy = governance_domain_policy(domain);
  match track {
    pallet_governance::ProposalTrackFamily::Ordinary => {
      if proposal_has_urgent_authorization(domain, item_id) {
        return pallet_governance::ProposalVotePowerProfile::FlatUrgentDirectStake;
      }
      governance_track_power_profile_for_backing(policy.primary_track)
    }
    pallet_governance::ProposalTrackFamily::Veto => {
      governance_track_power_profile_for_backing(policy.protection_track)
    }
  }
}

pub struct RuntimeGovernanceDomainPolicyProvider;
impl pallet_governance::GovernanceDomainPolicyProvider<AssetId>
  for RuntimeGovernanceDomainPolicyProvider
{
  fn policy(domain: AssetId) -> pallet_governance::GovernanceDomainPolicy {
    let policy = governance_domain_policy(domain);
    pallet_governance::GovernanceDomainPolicy {
      ordinary_power_profile: governance_track_power_profile_for_backing(policy.primary_track),
      protection_power_profile: governance_track_power_profile_for_backing(policy.protection_track),
    }
  }
}

#[cfg_attr(feature = "runtime-benchmarks", allow(dead_code))]
fn transferable_vote_power_balance(asset_id: AssetId, account: &AccountId) -> Balance {
  crate::Assets::balance(asset_id, account).saturating_add(
    crate::Governance::vote_power_custody(account, asset_id)
      .map(|position| position.amount)
      .unwrap_or_default(),
  )
}

fn direct_stake_vote_power(domain: AssetId, account: &AccountId) -> Balance {
  let Some(staked_asset_id) = crate::Staking::staked_asset_id(domain) else {
    return 0;
  };
  let shares = transferable_vote_power_balance(staked_asset_id, account);
  let Some(pool) = crate::Staking::pool(domain) else {
    return 0;
  };
  if pool.total_shares == 0 {
    return 0;
  }
  (sp_core::U256::from(shares).saturating_mul(sp_core::U256::from(pool.accounted_balance))
    / sp_core::U256::from(pool.total_shares))
  .try_into()
  .unwrap_or(Balance::MAX)
}

fn track_base_weight(
  backing: RuntimeGovernanceTrackBacking,
  domain: AssetId,
  account: &AccountId,
) -> u128 {
  match backing {
    RuntimeGovernanceTrackBacking::DirectStake => direct_stake_vote_power(domain, account),
    RuntimeGovernanceTrackBacking::VetoAsset => {
      let asset_id = primitives::ecosystem::protocol_tokens::VETO_ASSET_ID;
      if !<crate::Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<AccountId>>::asset_exists(asset_id) {
        return 0;
      }
      transferable_vote_power_balance(asset_id, account)
    }
    RuntimeGovernanceTrackBacking::NativeStake => {
      DelegationWeightedCollatorSessionManager::conservative_native_lp_value_or_zero(
        crate::Staking::account_native_lp_locked(account),
      )
      .saturating_add(native_governance_asset_vote_power(account))
    }
  }
}

#[cfg_attr(feature = "runtime-benchmarks", allow(dead_code))]
fn ordinary_track_base_weight(domain: AssetId, account: &AccountId) -> u128 {
  track_base_weight(
    governance_domain_policy(domain).primary_track,
    domain,
    account,
  )
}

#[cfg_attr(feature = "runtime-benchmarks", allow(dead_code))]
fn native_staking_asset_id() -> AssetId {
  <<crate::Runtime as pallet_staking::Config>::NativeStakingAssetId as polkadot_sdk::frame_support::traits::Get<AssetId>>::get()
}

#[cfg_attr(feature = "runtime-benchmarks", allow(dead_code))]
fn protection_track_base_weight(domain: AssetId, account: &AccountId) -> u128 {
  track_base_weight(
    governance_domain_policy(domain).protection_track,
    domain,
    account,
  )
}

#[cfg_attr(feature = "runtime-benchmarks", allow(dead_code))]
fn native_governance_asset_vote_power(account: &AccountId) -> Balance {
  let native_asset_id = native_staking_asset_id();
  let native_power = crate::Staking::native_governance_asset_locked(account, native_asset_id);
  let Some(staked_asset_id) = crate::Staking::staked_asset_id(native_asset_id) else {
    return native_power;
  };
  native_power.saturating_add(staked_receipt_governance_power(
    crate::Staking::native_governance_asset_locked(account, staked_asset_id),
  ))
}

#[cfg_attr(feature = "runtime-benchmarks", allow(dead_code))]
fn total_native_governance_asset_vote_power() -> Balance {
  let native_asset_id = native_staking_asset_id();
  let native_power = crate::Staking::total_native_governance_asset_locked(native_asset_id);
  let Some(staked_asset_id) = crate::Staking::staked_asset_id(native_asset_id) else {
    return native_power;
  };
  native_power.saturating_add(staked_receipt_governance_power(
    crate::Staking::total_native_governance_asset_locked(staked_asset_id),
  ))
}

fn staked_receipt_governance_power(shares: Balance) -> Balance {
  if shares == 0 {
    return 0;
  }
  let Some(pool) = crate::Staking::pool(native_staking_asset_id()) else {
    return 0;
  };
  if pool.total_shares == 0 {
    return 0;
  }
  let result = sp_core::U256::from(shares)
    .saturating_mul(sp_core::U256::from(pool.accounted_balance))
    .checked_div(sp_core::U256::from(pool.total_shares))
    .unwrap_or_default();
  result.try_into().unwrap_or(Balance::MAX)
}

#[cfg_attr(feature = "runtime-benchmarks", allow(dead_code))]
fn track_total_issuance(backing: RuntimeGovernanceTrackBacking, domain: AssetId) -> u128 {
  match backing {
    RuntimeGovernanceTrackBacking::DirectStake => crate::Staking::pool(domain)
      .map(|pool| pool.accounted_balance)
      .unwrap_or_default(),
    RuntimeGovernanceTrackBacking::VetoAsset => {
      let asset_id = primitives::ecosystem::protocol_tokens::VETO_ASSET_ID;
      if !<crate::Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<AccountId>>::asset_exists(asset_id) {
        return 0;
      }
      <crate::Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<AccountId>>::total_issuance(asset_id)
    }
    RuntimeGovernanceTrackBacking::NativeStake => {
      DelegationWeightedCollatorSessionManager::conservative_native_lp_value_or_zero(
        crate::Staking::total_native_lp_locked(),
      )
      .saturating_add(total_native_governance_asset_vote_power())
    }
  }
}

#[cfg_attr(feature = "runtime-benchmarks", allow(dead_code))]
fn protection_track_total_issuance(domain: AssetId) -> u128 {
  track_total_issuance(governance_domain_policy(domain).protection_track, domain)
}

pub struct RuntimeProposalVoteWeightProvider;
impl pallet_governance::ProposalVoteWeightProvider<AccountId, AssetId, u32, BlockNumber>
  for RuntimeProposalVoteWeightProvider
{
  fn vote_weight(
    domain: AssetId,
    context: &pallet_governance::ProposalVoteContext<u32, BlockNumber>,
    account: &AccountId,
  ) -> u32 {
    #[cfg(feature = "runtime-benchmarks")]
    {
      let _ = (domain, context, account);
      return 1;
    }
    #[cfg(not(feature = "runtime-benchmarks"))]
    {
      let base_weight = ordinary_track_base_weight(domain, account);
      if proposal_has_urgent_authorization(domain, context.item_id) {
        return base_weight.min(u128::from(u32::MAX)) as u32;
      }
      declining_power_weight(base_weight, context).min(u128::from(u32::MAX)) as u32
    }
  }
}

pub struct RuntimeProposalTrackPowerProfileProvider;
impl pallet_governance::ProposalTrackPowerProfileProvider<AssetId, u32>
  for RuntimeProposalTrackPowerProfileProvider
{
  fn power_profile(
    domain: AssetId,
    item_id: u32,
    track: pallet_governance::ProposalTrackFamily,
  ) -> pallet_governance::ProposalVotePowerProfile {
    governance_track_power_profile(domain, item_id, track)
  }
}

pub struct RuntimeProposalPrimaryTrackFamilyProvider;
impl pallet_governance::ProposalPrimaryTrackFamilyProvider<AssetId>
  for RuntimeProposalPrimaryTrackFamilyProvider
{
  fn family(
    domain: AssetId,
    payload_kind: pallet_governance::ProposalPayloadKind,
  ) -> pallet_governance::ProposalPrimaryTrackFamily {
    if domain == tactical_governance_domain()
      && payload_kind == pallet_governance::ProposalPayloadKind::L2TreasurySpend
    {
      return pallet_governance::ProposalPrimaryTrackFamily::Invoice;
    }
    pallet_governance::ProposalPrimaryTrackFamily::Binary
  }
}

pub struct RuntimeProposalUrgentPolicyProvider;
impl pallet_governance::ProposalUrgentPolicyProvider<AssetId>
  for RuntimeProposalUrgentPolicyProvider
{
  fn is_expeditable(domain: AssetId, payload_kind: pallet_governance::ProposalPayloadKind) -> bool {
    domain == protocol_governance_domain()
      && payload_kind == pallet_governance::ProposalPayloadKind::L1RootAction
  }

  fn executes_immediately_on_unanimous_pass(
    domain: AssetId,
    payload_kind: pallet_governance::ProposalPayloadKind,
  ) -> bool {
    domain == protocol_governance_domain()
      && payload_kind == pallet_governance::ProposalPayloadKind::L1RootAction
  }
}

pub struct RuntimeProposalSubmissionAuthorityProvider;
impl pallet_governance::ProposalSubmissionAuthorityProvider<AssetId>
  for RuntimeProposalSubmissionAuthorityProvider
{
  fn authority(
    domain: AssetId,
    payload_kind: pallet_governance::ProposalPayloadKind,
  ) -> pallet_governance::ProposalSubmissionAuthority {
    if domain == protocol_governance_domain()
      && matches!(
        payload_kind,
        pallet_governance::ProposalPayloadKind::L1RootAction
          | pallet_governance::ProposalPayloadKind::Intent
      )
    {
      return pallet_governance::ProposalSubmissionAuthority::PrimaryEligibleSigned;
    }
    if payload_kind == pallet_governance::ProposalPayloadKind::Intent {
      return pallet_governance::ProposalSubmissionAuthority::Signed;
    }
    if domain == tactical_governance_domain()
      && matches!(
        payload_kind,
        pallet_governance::ProposalPayloadKind::L2SignalToL1
          | pallet_governance::ProposalPayloadKind::L2TreasurySpend
      )
    {
      return pallet_governance::ProposalSubmissionAuthority::Signed;
    }
    pallet_governance::ProposalSubmissionAuthority::AdminOnly
  }
}

pub struct RuntimeProposalSubmissionEligibilityProvider;
impl pallet_governance::ProposalSubmissionEligibilityProvider<AccountId, AssetId>
  for RuntimeProposalSubmissionEligibilityProvider
{
  fn has_primary_governance_power(domain: AssetId, account: &AccountId) -> bool {
    domain == protocol_governance_domain() && ordinary_track_base_weight(domain, account) > 0
  }
}

#[cfg(feature = "runtime-benchmarks")]
pub struct RuntimeGovernanceBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_governance::BenchmarkHelper<AccountId, AssetId, Hash, AssetId, Balance>
  for RuntimeGovernanceBenchmarkHelper
{
  fn prepare_primary_eligible_submitter(
    account: &AccountId,
  ) -> Result<(AssetId, Hash, Vec<u8>), polkadot_sdk::sp_runtime::DispatchError> {
    use polkadot_sdk::frame_support::traits::{Currency, fungibles::Mutate};
    let domain = protocol_governance_domain();
    if !<crate::Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<AccountId>>::asset_exists(domain) {
      crate::Assets::force_create(
        RuntimeOrigin::root(),
        domain,
        account.clone().into(),
        true,
        1,
      )?;
    }
    if crate::Staking::pool(domain).is_none() {
      crate::Staking::register_staking_asset(RuntimeOrigin::root(), domain)?;
    }
    let amount = primitives::ecosystem::params::PRECISION.saturating_mul(100);
    <crate::Assets as Mutate<AccountId>>::mint_into(domain, account, amount)?;
    let _ = <crate::Balances as Currency<AccountId>>::deposit_creating(account, amount);
    crate::Staking::stake(RuntimeOrigin::signed(account.clone()), domain, amount / 2)?;
    let payload = StrategicRuntimeUpgradePayload {
      code_hash: Hash::default(),
    }
    .encode();
    let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&payload);
    crate::Preimage::note_preimage(RuntimeOrigin::signed(account.clone()), payload.clone())
      .map_err(|error| error.error)?;
    Ok((domain, payload_hash, payload))
  }

  fn prepare_maximum_payload_witness(
    account: &AccountId,
  ) -> Result<
    (
      AssetId,
      pallet_governance::ProposalPayloadKind,
      Hash,
      Vec<u8>,
    ),
    polkadot_sdk::sp_runtime::DispatchError,
  > {
    use polkadot_sdk::frame_support::traits::Currency;
    let _ = <crate::Balances as Currency<AccountId>>::deposit_creating(
      account,
      primitives::ecosystem::params::PRECISION,
    );
    let payload = (
      Some(Hash::repeat_byte(1)),
      alloc::vec![b'a'; 128],
      Some(alloc::vec![b'b'; 96]),
    )
      .encode();
    debug_assert_eq!(payload.len(), 262);
    let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&payload);
    crate::Preimage::note_preimage(RuntimeOrigin::signed(account.clone()), payload.clone())
      .map_err(|error| error.error)?;
    Ok((
      protocol_governance_domain(),
      pallet_governance::ProposalPayloadKind::Intent,
      payload_hash,
      payload,
    ))
  }

  fn prepare_protection_voter(
    account: &AccountId,
  ) -> Result<AssetId, polkadot_sdk::sp_runtime::DispatchError> {
    use polkadot_sdk::frame_support::traits::fungibles::{Inspect, Mutate};
    let domain = protocol_governance_domain();
    let asset_id = primitives::ecosystem::protocol_tokens::VETO_ASSET_ID;
    if !<crate::Assets as Inspect<AccountId>>::asset_exists(asset_id) {
      crate::Assets::force_create(
        RuntimeOrigin::root(),
        asset_id,
        account.clone().into(),
        true,
        1,
      )?;
    }
    <crate::Assets as Mutate<AccountId>>::mint_into(asset_id, account, 100)?;
    Ok(domain)
  }

  fn prepare_vote_power_custody(
    account: &AccountId,
  ) -> Result<(AssetId, Balance), polkadot_sdk::sp_runtime::DispatchError> {
    use polkadot_sdk::frame_support::traits::fungibles::{Inspect, Mutate};
    let asset_id = primitives::ecosystem::protocol_tokens::VETO_ASSET_ID;
    if !<crate::Assets as Inspect<AccountId>>::asset_exists(asset_id) {
      crate::Assets::force_create(
        RuntimeOrigin::root(),
        asset_id,
        account.clone().into(),
        true,
        1,
      )?;
    }
    let amount = 100;
    <crate::Assets as Mutate<AccountId>>::mint_into(
      asset_id,
      &governance_vote_power_custody_account(),
      amount,
    )?;
    Ok((asset_id, amount))
  }
}

pub struct RuntimeProposalRuntimeUpgradeAuthorizationProvider;
impl pallet_governance::ProposalRuntimeUpgradeAuthorizationProvider<Hash>
  for RuntimeProposalRuntimeUpgradeAuthorizationProvider
{
  fn authorized_upgrade() -> Option<pallet_governance::AuthorizedRuntimeUpgrade<Hash>> {
    let authorization = crate::System::authorized_upgrade()?;
    pallet_governance::AuthorizedRuntimeUpgrade::<Hash>::decode(&mut &authorization.encode()[..])
      .ok()
  }
}

pub struct RuntimeProposalPayloadPreimageNoteCostProvider;
impl pallet_governance::ProposalPayloadPreimageNoteCostProvider<Balance>
  for RuntimeProposalPayloadPreimageNoteCostProvider
{
  fn note_cost(payload_len: u32) -> Option<Balance> {
    Some(
      PreimageBaseDeposit::get()
        .saturating_add(PreimageByteDeposit::get().saturating_mul(Balance::from(payload_len))),
    )
  }
}

const MAX_NORMALIZED_PROTECTION_POWER: u64 = u64::MAX / 7;

fn normalize_protection_power(power: u128, total_issuance: u128) -> u64 {
  if total_issuance == 0 {
    return 0;
  }
  let bounded_power = power.min(total_issuance);
  if total_issuance <= u128::from(MAX_NORMALIZED_PROTECTION_POWER) {
    return bounded_power as u64;
  }
  (sp_core::U256::from(bounded_power) * sp_core::U256::from(MAX_NORMALIZED_PROTECTION_POWER)
    / sp_core::U256::from(total_issuance))
  .as_u64()
}

fn normalize_protection_total(total_issuance: u128) -> u64 {
  total_issuance.min(u128::from(MAX_NORMALIZED_PROTECTION_POWER)) as u64
}

pub struct RuntimeVetoVotePowerProvider;
impl pallet_governance::VetoVotePowerProvider<AccountId, AssetId, u32, BlockNumber>
  for RuntimeVetoVotePowerProvider
{
  fn vote_weight(
    domain: AssetId,
    context: &pallet_governance::ProposalVoteContext<u32, BlockNumber>,
    account: &AccountId,
  ) -> u64 {
    declining_power_weight(u128::from(Self::raw_vote_weight(domain, account)), context)
      .min(u128::from(u64::MAX)) as u64
  }

  fn raw_vote_weight(domain: AssetId, account: &AccountId) -> u64 {
    normalize_protection_power(
      protection_track_base_weight(domain, account),
      protection_track_total_issuance(domain),
    )
  }

  fn total_issuance(domain: AssetId) -> u64 {
    normalize_protection_total(protection_track_total_issuance(domain))
  }
}

fn validate_advisory_payload(
  bytes: &[u8],
) -> Result<(), pallet_governance::ProposalPreimageAdmissionError> {
  use pallet_governance::ProposalPreimageAdmissionError as Error;
  let mut input = bytes;
  let _referenced_payload_hash = Option::<Hash>::decode(&mut input).map_err(|_| Error::Invalid)?;
  let summary = Vec::<u8>::decode(&mut input).map_err(|_| Error::Invalid)?;
  let doc_cid = Option::<Vec<u8>>::decode(&mut input).map_err(|_| Error::Invalid)?;
  if !input.is_empty()
    || summary.is_empty()
    || summary.len() > 128
    || core::str::from_utf8(&summary).is_err()
    || doc_cid.as_ref().is_some_and(|cid| cid.len() > 96)
  {
    return Err(Error::Invalid);
  }
  Ok(())
}

pub struct RuntimeProposalPayloadPreimageProvider;
impl pallet_governance::ProposalPayloadPreimageProvider<Hash, AssetId>
  for RuntimeProposalPayloadPreimageProvider
{
  fn have_preimage(hash: &Hash) -> bool {
    <crate::Preimage as PreimageProvider<Hash>>::have_preimage(hash)
  }

  fn preimage_requested(hash: &Hash) -> bool {
    <crate::Preimage as PreimageProvider<Hash>>::preimage_requested(hash)
  }

  fn preimage_len(hash: &Hash) -> Option<u32> {
    <crate::Preimage as QueryPreimage>::len(hash)
  }

  fn validate_for_witness(
    domain: AssetId,
    payload_kind: pallet_governance::ProposalPayloadKind,
    hash: &Hash,
    bytes: &[u8],
  ) -> Result<
    pallet_governance::ValidatedProposalPayload,
    pallet_governance::ProposalPreimageAdmissionError,
  > {
    use pallet_governance::ProposalPreimageAdmissionError as Error;
    if <Runtime as frame_system::Config>::Hashing::hash(bytes) != *hash {
      return Err(Error::Invalid);
    }
    let (execution_authority, compatibility) = Self::current_compatibility(domain, payload_kind)?;
    if bytes.len() > proposal_payload_length_ceiling(payload_kind) {
      return Err(Error::Oversized);
    }
    let mut input = bytes;
    match payload_kind {
      pallet_governance::ProposalPayloadKind::L1RootAction => {
        if domain != protocol_governance_domain() {
          return Err(Error::Incompatible);
        }
        let _payload =
          StrategicRuntimeUpgradePayload::decode(&mut input).map_err(|_| Error::Invalid)?;
      }
      pallet_governance::ProposalPayloadKind::L2ParameterChange => {
        let call = RuntimeCall::decode(&mut input).map_err(|_| Error::Invalid)?;
        if !matches!(
          call,
          RuntimeCall::DeosRouter(pallet_deos_router::Call::update_router_fee { .. })
        ) || domain != protocol_governance_domain()
        {
          return Err(Error::Incompatible);
        }
      }
      pallet_governance::ProposalPayloadKind::L2TreasurySpend => {
        if domain != tactical_governance_domain() {
          return Err(Error::Incompatible);
        }
        let _payload =
          TacticalTreasuryInvoicePayload::decode(&mut input).map_err(|_| Error::Invalid)?;
      }
      pallet_governance::ProposalPayloadKind::Intent
      | pallet_governance::ProposalPayloadKind::L2SignalToL1 => {
        validate_advisory_payload(bytes)?;
        input = &[];
      }
    }
    if !input.is_empty() {
      return Err(Error::Invalid);
    }
    let payload_len = u32::try_from(bytes.len()).map_err(|_| Error::Oversized)?;
    Ok(pallet_governance::ValidatedProposalPayload {
      payload_len,
      execution_authority,
      compatibility,
    })
  }

  fn current_compatibility(
    domain: AssetId,
    payload_kind: pallet_governance::ProposalPayloadKind,
  ) -> Result<
    (
      pallet_governance::ProposalExecutionAuthority,
      pallet_governance::ProposalPayloadCompatibility,
    ),
    pallet_governance::ProposalPreimageAdmissionError,
  > {
    use pallet_governance::{
      ProposalExecutionAuthority as Authority, ProposalPayloadKind as Kind,
      ProposalPreimageAdmissionError as Error,
    };
    let authority = match payload_kind {
      Kind::L1RootAction if domain == protocol_governance_domain() => Authority::Root,
      Kind::L2ParameterChange if domain == protocol_governance_domain() => {
        Authority::DomainParameters
      }
      Kind::L2TreasurySpend if domain == tactical_governance_domain() => Authority::DomainTreasury,
      Kind::Intent => Authority::NonExecutable,
      Kind::L2SignalToL1 if domain == tactical_governance_domain() => Authority::NonExecutable,
      _ => return Err(Error::Incompatible),
    };
    Ok((
      authority,
      pallet_governance::ProposalPayloadCompatibility {
        schema_version: 1,
        runtime_spec_version: Some(crate::VERSION.spec_version),
      },
    ))
  }

  fn payload_length_ceiling(
    domain: AssetId,
    payload_kind: pallet_governance::ProposalPayloadKind,
  ) -> Result<u32, pallet_governance::ProposalPreimageAdmissionError> {
    Self::current_compatibility(domain, payload_kind)?;
    u32::try_from(proposal_payload_length_ceiling(payload_kind))
      .map_err(|_| pallet_governance::ProposalPreimageAdmissionError::Oversized)
  }
}

fn proposal_payload_length_ceiling(payload_kind: pallet_governance::ProposalPayloadKind) -> usize {
  use pallet_governance::ProposalPayloadKind as Kind;
  match payload_kind {
    Kind::L1RootAction => StrategicRuntimeUpgradePayload::max_encoded_len(),
    Kind::L2ParameterChange => 6,
    Kind::L2TreasurySpend => TacticalTreasuryInvoicePayload::max_encoded_len(),
    Kind::Intent | Kind::L2SignalToL1 => 262,
  }
}

fn protocol_governance_domain() -> AssetId {
  native_staking_asset_id()
}

fn tactical_governance_domain() -> AssetId {
  primitives::ecosystem::protocol_tokens::BLDR_ASSET_ID
}

fn governance_treasury_account(domain: AssetId) -> Option<AccountId> {
  if domain == tactical_governance_domain() {
    return Some(crate::Actors::sovereign_account_id_system(
      primitives::ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID,
    ));
  }
  None
}

#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct StrategicRuntimeUpgradePayload {
  pub code_hash: Hash,
}

#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub enum TacticalTreasuryFundingSource {
  BldrTreasury,
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct TacticalTreasuryInvoicePayload {
  pub beneficiary: AccountId,
  pub payout_asset: AssetId,
  pub base_amount: u128,
  pub funding_source: TacticalTreasuryFundingSource,
}

fn tactical_treasury_account_for_invoice(
  domain: AssetId,
  funding_source: TacticalTreasuryFundingSource,
) -> Option<AccountId> {
  match funding_source {
    TacticalTreasuryFundingSource::BldrTreasury => governance_treasury_account(domain),
  }
}

fn invoice_scalar_for_winning_option(
  winning_option: pallet_governance::ProposalPrimaryTrackOption,
) -> Option<(pallet_governance::ProposalTreasurySpendScalar, u128, u128)> {
  match winning_option {
    pallet_governance::ProposalPrimaryTrackOption::Amplify => Some((
      pallet_governance::ProposalTreasurySpendScalar::Amplify,
      2,
      1,
    )),
    pallet_governance::ProposalPrimaryTrackOption::Approve => Some((
      pallet_governance::ProposalTreasurySpendScalar::Approve,
      1,
      1,
    )),
    pallet_governance::ProposalPrimaryTrackOption::Reduce => {
      Some((pallet_governance::ProposalTreasurySpendScalar::Reduce, 1, 2))
    }
    _ => None,
  }
}

pub struct RuntimeProposalPayloadExecutor;
impl pallet_governance::ProposalPayloadExecutor<AccountId, AssetId, u32, Hash>
  for RuntimeProposalPayloadExecutor
{
  fn can_execute(payload_kind: pallet_governance::ProposalPayloadKind) -> bool {
    matches!(
      payload_kind,
      pallet_governance::ProposalPayloadKind::L1RootAction
        | pallet_governance::ProposalPayloadKind::L2ParameterChange
        | pallet_governance::ProposalPayloadKind::L2TreasurySpend
    )
  }

  fn execute(
    domain: AssetId,
    item_id: u32,
    payload_kind: pallet_governance::ProposalPayloadKind,
    payload_hash: Hash,
  ) -> Result<
    pallet_governance::ProposalExecutionReceipt<AccountId, AssetId, Hash>,
    pallet_governance::ProposalExecutionFailureReason,
  > {
    let Some(bytes) = <crate::Preimage as PreimageProvider<Hash>>::get_preimage(&payload_hash)
    else {
      return Err(pallet_governance::ProposalExecutionFailureReason::MissingPreimage);
    };
    match payload_kind {
      pallet_governance::ProposalPayloadKind::L1RootAction => {
        if domain != protocol_governance_domain() {
          return Err(pallet_governance::ProposalExecutionFailureReason::UnsupportedDomain);
        }
        let payload = StrategicRuntimeUpgradePayload::decode(&mut &bytes[..])
          .map_err(|_| pallet_governance::ProposalExecutionFailureReason::InvalidPreimage)?;
        RuntimeCall::System(frame_system::Call::authorize_upgrade {
          code_hash: payload.code_hash,
        })
        .dispatch_bypass_filter(RawOrigin::Root.into())
        .map(
          |_| pallet_governance::ProposalExecutionReceipt::RuntimeUpgradeAuthorized {
            code_hash: payload.code_hash,
          },
        )
        .map_err(|_| pallet_governance::ProposalExecutionFailureReason::DispatchFailed)
      }
      pallet_governance::ProposalPayloadKind::L2ParameterChange => {
        let call = RuntimeCall::decode(&mut &bytes[..])
          .map_err(|_| pallet_governance::ProposalExecutionFailureReason::InvalidPreimage)?;
        match call {
          RuntimeCall::DeosRouter(pallet_deos_router::Call::update_router_fee { new_fee })
            if domain == protocol_governance_domain() =>
          {
            crate::DeosRouter::apply_router_fee_update(new_fee)
              .map(
                |_| pallet_governance::ProposalExecutionReceipt::ParameterChangeExecuted {
                  surface: pallet_governance::ProposalParameterChangeSurface::RouterFee,
                },
              )
              .map_err(|_| pallet_governance::ProposalExecutionFailureReason::DispatchFailed)
          }
          _ => Err(pallet_governance::ProposalExecutionFailureReason::UnsupportedCall),
        }
      }
      pallet_governance::ProposalPayloadKind::L2TreasurySpend => {
        if domain != tactical_governance_domain() {
          return Err(pallet_governance::ProposalExecutionFailureReason::UnsupportedDomain);
        }
        let payload = TacticalTreasuryInvoicePayload::decode(&mut &bytes[..])
          .map_err(|_| pallet_governance::ProposalExecutionFailureReason::InvalidPreimage)?;
        let winning_option = crate::Governance::proposal_winning_primary_option(domain, item_id)
          .ok_or(pallet_governance::ProposalExecutionFailureReason::MissingWinningPrimaryOption)?;
        let (scalar, numerator, denominator) = invoice_scalar_for_winning_option(winning_option)
          .ok_or(pallet_governance::ProposalExecutionFailureReason::MissingWinningPrimaryOption)?;
        let final_amount = payload
          .base_amount
          .checked_mul(numerator)
          .and_then(|amount| amount.checked_div(denominator))
          .ok_or(pallet_governance::ProposalExecutionFailureReason::DispatchFailed)?;
        let treasury_account =
          tactical_treasury_account_for_invoice(domain, payload.funding_source)
            .ok_or(pallet_governance::ProposalExecutionFailureReason::DispatchFailed)?;
        RuntimeCall::Assets(pallet_assets::Call::transfer {
          id: payload.payout_asset,
          target: polkadot_sdk::sp_runtime::MultiAddress::Id(payload.beneficiary.clone()),
          amount: final_amount,
        })
        .dispatch_bypass_filter(RawOrigin::Signed(treasury_account.clone()).into())
        .map(
          |_| pallet_governance::ProposalExecutionReceipt::TreasurySpendExecuted {
            funding_source: treasury_account,
            beneficiary: payload.beneficiary,
            payout_asset: payload.payout_asset,
            base_amount: payload.base_amount,
            scalar,
            final_amount,
            settlement_kind:
              pallet_governance::ProposalTreasurySpendSettlementKind::InvoiceScalarTransfer,
          },
        )
        .map_err(|_| pallet_governance::ProposalExecutionFailureReason::DispatchFailed)
      }
      pallet_governance::ProposalPayloadKind::Intent
      | pallet_governance::ProposalPayloadKind::L2SignalToL1 => {
        Err(pallet_governance::ProposalExecutionFailureReason::UnsupportedCall)
      }
    }
  }
}

impl pallet_governance::Config for Runtime {
  type AdminOrigin = EnsureRoot<AccountId>;
  type Currency = Balances;
  type ProposalOpeningFee = ProposalOpeningFee;
  type PayloadAdmissionWitnessDeposit = PayloadAdmissionWitnessDeposit;
  type ProposalFeeRecipient = crate::configs::actor_config::ActorFeeRecipient;
  type DomainId = AssetId;
  type VotePowerLockId = AssetId;
  type VotePowerCustody = RuntimeVotePowerCustody;
  type WinningVoteItemId = u32;
  type Epoch = BlockNumber;
  type EpochProvider = RuntimeGovernanceEpochProvider;
  type MaxEpochCatchUpPerBlock = MaxGovernanceEpochCatchUpPerBlock;
  type MaxMaturingProposalsPerBlock = MaxGovernanceMaturingProposalsPerBlock;
  type MaxPendingEnactmentsPerBlock = MaxGovernancePendingEnactmentsPerBlock;
  type MaxFinalizedProposalOutcomesPerBlock = MaxGovernanceFinalizedOutcomesPerBlock;
  type MaxExpiringAccountsPerBlock = MaxGovernanceExpiringAccountsPerBlock;
  type WinningVoteLookbackEpochs = WinningVoteLookbackEpochs;
  type MaxWinningVotesPerEpoch = MaxWinningVotesPerEpoch;
  type MaxWinningVoteItemsPerEpoch = MaxWinningVoteItemsPerEpoch;
  type MaxWinningVoteResolutionItemsPerEpoch = MaxWinningVoteResolutionItemsPerEpoch;
  type MaxWinningVoteAccountsPerCall = MaxWinningVoteAccountsPerCall;
  type MaxProposalPayloadBytes = ConstU32<262>;
  type MaxActiveProposalsPerDomain = MaxActiveProposalsPerDomain;
  type StrategicProposalReserve = StrategicProposalReserve;
  type MaxActiveProposalsPerAuthor = MaxActiveProposalsPerAuthor;
  type MaxMaturingProposalsPerEpoch = MaxMaturingProposalsPerEpoch;
  type MaxPendingEnactmentsPerEpoch = MaxPendingEnactmentsPerEpoch;
  type ProposalVotingPeriod = ProposalVotingPeriod;
  type ProposalLeadInPeriod = ProposalLeadInPeriod;
  type ProposalProtectionPeriod = ProposalProtectionPeriod;
  type ProposalUrgentVotingPeriod = ProposalUrgentVotingPeriod;
  type ProposalEnactmentDelay = ProposalEnactmentDelay;
  type ProposalFastTrackPassThreshold = ProposalFastTrackPassThreshold;
  type ProposalApprovalThreshold = ProposalApprovalThreshold;
  type ProposalVetoThreshold = ProposalVetoThreshold;
  type ProposalVetoMinimumVetoTurnout = ProposalVetoMinimumVetoTurnout;
  type ProposalMinimumTurnout = ProposalMinimumTurnout;
  type ProposalConfirmPeriod = ConstU32<0>;
  type FinalizedProposalOutcomeRetentionEpochs = FinalizedProposalOutcomeRetentionEpochs;
  type MaxFinalizedProposalOutcomesPerEpoch = MaxFinalizedProposalOutcomesPerEpoch;
  type MaxRecentFinalizedProposalsPerDomain = MaxRecentFinalizedProposalsPerDomain;
  type MaxExpiringAccountsPerEpoch = MaxExpiringAccountsPerEpoch;
  type ProposalVoteWeightProvider = RuntimeProposalVoteWeightProvider;
  type GovernanceDomainPolicyProvider = RuntimeGovernanceDomainPolicyProvider;
  type ProposalTrackPowerProfileProvider = RuntimeProposalTrackPowerProfileProvider;
  type ProposalPrimaryTrackFamilyProvider = RuntimeProposalPrimaryTrackFamilyProvider;
  type ProposalUrgentPolicyProvider = RuntimeProposalUrgentPolicyProvider;
  type ProposalSubmissionAuthorityProvider = RuntimeProposalSubmissionAuthorityProvider;
  type ProposalSubmissionEligibilityProvider = RuntimeProposalSubmissionEligibilityProvider;
  type ProposalRuntimeUpgradeAuthorizationProvider =
    RuntimeProposalRuntimeUpgradeAuthorizationProvider;
  type ProposalPayloadPreimageNoteCostProvider = RuntimeProposalPayloadPreimageNoteCostProvider;
  type VetoVotePowerProvider = RuntimeVetoVotePowerProvider;
  type ProposalPayloadPreimageProvider = RuntimeProposalPayloadPreimageProvider;
  type ProposalPayloadExecutor = RuntimeProposalPayloadExecutor;
  #[cfg(feature = "runtime-benchmarks")]
  type BenchmarkHelper = RuntimeGovernanceBenchmarkHelper;
  type WeightInfo = crate::weights::pallet_governance::SubstrateWeight<Runtime>;
}

#[cfg(test)]
mod tests {
  use super::{
    MAX_NORMALIZED_PROTECTION_POWER, declining_power_weight, normalize_protection_power,
    normalize_protection_total,
  };

  fn context(
    submitted_epoch: u32,
    maturity_epoch: u32,
    vote_epoch: u32,
  ) -> pallet_governance::ProposalVoteContext<u32, u32> {
    pallet_governance::ProposalVoteContext {
      item_id: 1,
      current_epoch: vote_epoch,
      submitted_epoch,
      maturity_epoch,
      vote_epoch,
    }
  }

  #[test]
  fn protection_power_normalization_preserves_boundary_ratios_without_u64_saturation() {
    let cap = u128::from(MAX_NORMALIZED_PROTECTION_POWER);
    let total = cap * 100;
    let majority = total * 51 / 100;
    let minority = total / 100;

    assert_eq!(
      normalize_protection_total(total),
      MAX_NORMALIZED_PROTECTION_POWER
    );
    assert!(normalize_protection_power(majority, total) > MAX_NORMALIZED_PROTECTION_POWER / 2);
    assert!(normalize_protection_power(minority, total) < MAX_NORMALIZED_PROTECTION_POWER / 2);
    assert_eq!(
      normalize_protection_power(total, total),
      MAX_NORMALIZED_PROTECTION_POWER
    );
    assert_eq!(
      MAX_NORMALIZED_PROTECTION_POWER.saturating_mul(7),
      u64::MAX - (u64::MAX % 7),
      "the normalization cap must reserve the full declining-power multiplier"
    );
  }

  #[test]
  fn protection_power_normalization_is_proportional_at_the_u128_supply_boundary() {
    let total = u128::MAX;
    let powers = [1, total / 3, total / 2, total];
    let mut previous = 0;
    for power in powers {
      let normalized = normalize_protection_power(power, total);
      let expected = (polkadot_sdk::sp_core::U256::from(power)
        * polkadot_sdk::sp_core::U256::from(MAX_NORMALIZED_PROTECTION_POWER)
        / polkadot_sdk::sp_core::U256::from(total))
      .as_u64();
      assert_eq!(normalized, expected);
      assert!(normalized >= previous);
      assert!(normalized.checked_mul(7).is_some());
      previous = normalized;
    }
    assert_eq!(previous, normalize_protection_total(total));
  }

  #[test]
  fn protection_power_normalization_keeps_representable_units_exact() {
    assert_eq!(normalize_protection_total(1_000), 1_000);
    assert_eq!(normalize_protection_power(510, 1_000), 510);
  }

  #[test]
  fn declining_power_starts_at_seven_x() {
    assert_eq!(declining_power_weight(10, &context(0, 7, 0)), 70);
  }

  #[test]
  fn declining_power_reaches_one_x_at_day_six_and_stays_flat() {
    assert_eq!(declining_power_weight(10, &context(0, 7, 6)), 10);
    assert_eq!(declining_power_weight(10, &context(0, 7, 7)), 10);
  }
}
