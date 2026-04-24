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
use scale_info::TypeInfo;

parameter_types! {
  pub const WinningVoteLookbackEpochs: u32 = 3;
  pub const MaxWinningVotesPerEpoch: u16 = 4;
  pub const MaxWinningVoteItemsPerEpoch: u32 = 4;
  pub const MaxWinningVoteResolutionItemsPerEpoch: u32 = 64;
  pub const MaxWinningVoteAccountsPerCall: u32 = 256;
  pub const MaxActiveProposalsPerDomain: u32 = 128;
  pub const MaxMaturingProposalsPerEpoch: u32 = 4;
  pub const MaxPendingEnactmentsPerEpoch: u32 = 4;
  pub const ProposalVotingPeriod: BlockNumber = 7 * 24 * HOURS;
  pub const ProposalLeadInPeriod: BlockNumber = 3 * 24 * HOURS;
  pub const ProposalProtectionPeriod: BlockNumber = 7 * 24 * HOURS;
  pub const ProposalUrgentVotingPeriod: BlockNumber = 24 * HOURS;
  pub const ProposalEnactmentDelay: BlockNumber = 3 * 24 * HOURS;
  pub const ProposalOpeningFee: Balance = 10 * EXISTENTIAL_DEPOSIT;
  pub ProposalFastTrackPassThreshold: polkadot_sdk::sp_runtime::Perbill =
    polkadot_sdk::sp_runtime::Perbill::from_percent(100);
  pub ProposalApprovalThreshold: polkadot_sdk::sp_runtime::Perbill =
    polkadot_sdk::sp_runtime::Perbill::from_percent(60);
  pub ProposalVetoThreshold: polkadot_sdk::sp_runtime::Perbill =
    polkadot_sdk::sp_runtime::Perbill::from_percent(50);
  pub ProposalVetoMinimumVetoTurnout: polkadot_sdk::sp_runtime::Perbill =
    polkadot_sdk::sp_runtime::Perbill::from_percent(1);
  pub const ProposalMinimumTurnout: u64 = 200;
  pub const FinalizedProposalOutcomeRetentionEpochs: u32 = 16;
  pub const MaxFinalizedProposalOutcomesPerEpoch: u32 = 1024;
  pub const MaxRecentFinalizedProposalsPerDomain: u32 = 16 * 1024;
  pub const MaxExpiringAccountsPerEpoch: u32 = 1024;
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
fn track_base_weight(
  backing: RuntimeGovernanceTrackBacking,
  domain: AssetId,
  account: &AccountId,
) -> u128 {
  match backing {
    RuntimeGovernanceTrackBacking::DirectStake => {
      crate::Staking::stake_value(domain, account).unwrap_or_default()
    }
    RuntimeGovernanceTrackBacking::VetoAsset => {
      let asset_id = primitives::ecosystem::protocol_tokens::VETO_ASSET_ID;
      if !<crate::Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<AccountId>>::asset_exists(asset_id) {
        return 0;
      }
      <crate::Assets as polkadot_sdk::frame_support::traits::fungibles::Inspect<AccountId>>::balance(
        asset_id, account,
      )
    }
    RuntimeGovernanceTrackBacking::NativeStake => {
      DelegationWeightedCollatorSessionManager::conservative_native_lp_value(
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
      DelegationWeightedCollatorSessionManager::conservative_native_lp_value(
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

pub struct RuntimeWinningVoteRewardTouchHandler;
impl pallet_governance::WinningVoteRewardTouchHandler<AccountId, AssetId>
  for RuntimeWinningVoteRewardTouchHandler
{
  fn note_winning_vote_recorded(domain: AssetId, account: &AccountId) {
    if domain == native_staking_asset_id() {
      let _ = crate::Staking::note_reward_touch(native_staking_asset_id(), account);
    }
  }

  fn note_winning_vote_evicted(domain: AssetId, account: &AccountId) {
    if domain == native_staking_asset_id() {
      let _ = crate::Staking::note_reward_touch(native_staking_asset_id(), account);
    }
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

pub struct RuntimeVetoVotePowerProvider;
impl pallet_governance::VetoVotePowerProvider<AccountId, AssetId, u32, BlockNumber>
  for RuntimeVetoVotePowerProvider
{
  fn vote_weight(
    domain: AssetId,
    context: &pallet_governance::ProposalVoteContext<u32, BlockNumber>,
    account: &AccountId,
  ) -> u64 {
    #[cfg(feature = "runtime-benchmarks")]
    {
      let _ = (domain, context, account);
      return 1;
    }
    #[cfg(not(feature = "runtime-benchmarks"))]
    {
      declining_power_weight(u128::from(Self::raw_vote_weight(domain, account)), context)
        .min(u128::from(u64::MAX)) as u64
    }
  }

  fn raw_vote_weight(domain: AssetId, account: &AccountId) -> u64 {
    #[cfg(feature = "runtime-benchmarks")]
    {
      let _ = (domain, account);
      return 1;
    }
    #[cfg(not(feature = "runtime-benchmarks"))]
    {
      protection_track_base_weight(domain, account).min(u128::from(u64::MAX)) as u64
    }
  }

  fn total_issuance(domain: AssetId) -> u64 {
    #[cfg(feature = "runtime-benchmarks")]
    {
      let _ = domain;
      return 1;
    }
    #[cfg(not(feature = "runtime-benchmarks"))]
    {
      protection_track_total_issuance(domain).min(u128::from(u64::MAX)) as u64
    }
  }
}

pub struct RuntimeProposalPayloadPreimageProvider;
impl pallet_governance::ProposalPayloadPreimageProvider<Hash>
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
}

fn protocol_governance_domain() -> AssetId {
  native_staking_asset_id()
}

fn tactical_governance_domain() -> AssetId {
  primitives::ecosystem::protocol_tokens::BLDR_ASSET_ID
}

fn governance_treasury_account(domain: AssetId) -> Option<AccountId> {
  if domain == tactical_governance_domain() {
    return Some(crate::AAA::sovereign_account_id_system(
      primitives::ecosystem::aaa_ids::BLDR_TREASURY_AAA_ID,
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
    TacticalTreasuryFundingSource::BldrTreasury if domain == tactical_governance_domain() => {
      governance_treasury_account(domain)
    }
    _ => None,
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
          RuntimeCall::AxialRouter(pallet_axial_router::Call::add_tracked_asset { asset })
            if domain == protocol_governance_domain() =>
          {
            crate::AxialRouter::apply_add_tracked_asset(asset)
              .map(
                |_| pallet_governance::ProposalExecutionReceipt::ParameterChangeExecuted {
                  surface: pallet_governance::ProposalParameterChangeSurface::TrackedAsset,
                },
              )
              .map_err(|_| pallet_governance::ProposalExecutionFailureReason::DispatchFailed)
          }
          RuntimeCall::AxialRouter(pallet_axial_router::Call::update_router_fee { new_fee })
            if domain == protocol_governance_domain() =>
          {
            crate::AxialRouter::apply_router_fee_update(new_fee)
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
            .ok_or(pallet_governance::ProposalExecutionFailureReason::UnsupportedTarget)?;
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
      _ => Err(pallet_governance::ProposalExecutionFailureReason::UnsupportedPayloadKind),
    }
  }
}

impl pallet_governance::Config for Runtime {
  type AdminOrigin = EnsureRoot<AccountId>;
  type Currency = Balances;
  type ProposalOpeningFee = ProposalOpeningFee;
  type DomainId = AssetId;
  type WinningVoteItemId = u32;
  type Epoch = BlockNumber;
  type EpochProvider = RuntimeGovernanceEpochProvider;
  type WinningVoteLookbackEpochs = WinningVoteLookbackEpochs;
  type MaxWinningVotesPerEpoch = MaxWinningVotesPerEpoch;
  type MaxWinningVoteItemsPerEpoch = MaxWinningVoteItemsPerEpoch;
  type MaxWinningVoteResolutionItemsPerEpoch = MaxWinningVoteResolutionItemsPerEpoch;
  type MaxWinningVoteAccountsPerCall = MaxWinningVoteAccountsPerCall;
  type MaxActiveProposalsPerDomain = MaxActiveProposalsPerDomain;
  type MaxMaturingProposalsPerEpoch = MaxMaturingProposalsPerEpoch;
  type MaxPendingEnactmentsPerEpoch = MaxPendingEnactmentsPerEpoch;
  type ProposalVotingPeriod = ProposalVotingPeriod;
  type ProposalLeadInPeriod = ProposalLeadInPeriod;
  type ProposalProtectionPeriod = ProposalProtectionPeriod;
  type ProposalUrgentVotingPeriod = ProposalUrgentVotingPeriod;
  type ProposalEnactmentDelay = ProposalEnactmentDelay;
  type ProposalFastTrackPassThreshold = ProposalFastTrackPassThreshold;
  type ProposalApprovalThreshold = ProposalApprovalThreshold;
  type ProposalApprovalCeiling = ProposalApprovalThreshold;
  type ProposalVetoThreshold = ProposalVetoThreshold;
  type ProposalVetoMinimumVetoTurnout = ProposalVetoMinimumVetoTurnout;
  type ProposalMinimumTurnout = ProposalMinimumTurnout;
  type ProposalTurnoutCeiling = ProposalMinimumTurnout;
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
  type ProposalRuntimeUpgradeAuthorizationProvider =
    RuntimeProposalRuntimeUpgradeAuthorizationProvider;
  type ProposalPayloadPreimageNoteCostProvider = RuntimeProposalPayloadPreimageNoteCostProvider;
  type VetoVotePowerProvider = RuntimeVetoVotePowerProvider;
  type ProposalPayloadPreimageProvider = RuntimeProposalPayloadPreimageProvider;
  type ProposalPayloadExecutor = RuntimeProposalPayloadExecutor;
  type WinningVoteRewardTouchHandler = RuntimeWinningVoteRewardTouchHandler;
  type WeightInfo = crate::weights::pallet_governance::SubstrateWeight<Runtime>;
}

#[cfg(test)]
mod tests {
  use super::declining_power_weight;

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
  fn declining_power_starts_at_seven_x() {
    assert_eq!(declining_power_weight(10, &context(0, 7, 0)), 70);
  }

  #[test]
  fn declining_power_reaches_one_x_at_day_six_and_stays_flat() {
    assert_eq!(declining_power_weight(10, &context(0, 7, 6)), 10);
    assert_eq!(declining_power_weight(10, &context(0, 7, 7)), 10);
  }
}
