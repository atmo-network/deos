#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use codec::{Decode, Encode};
use frame::prelude::{DecodeWithMemTracking, MaxEncodedLen, TypeInfo};
use frame::traits::StorageVersion;
use polkadot_sdk::frame_support::weights::Weight;

pub use pallet::*;

pub trait EpochProvider<Epoch> {
  fn current_epoch() -> Epoch;
}

impl<Epoch: Default> EpochProvider<Epoch> for () {
  fn current_epoch() -> Epoch {
    Default::default()
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProposalVoteContext<ItemId, Epoch> {
  pub item_id: ItemId,
  pub current_epoch: Epoch,
  pub submitted_epoch: Epoch,
  pub maturity_epoch: Epoch,
  pub vote_epoch: Epoch,
}

pub trait ProposalVoteWeightProvider<AccountId, DomainId, ItemId, Epoch> {
  fn vote_weight(
    domain: DomainId,
    context: &ProposalVoteContext<ItemId, Epoch>,
    account: &AccountId,
  ) -> u32;
}

#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum ProposalTrackFamily {
  Ordinary,
  Veto,
}

#[derive(
  Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub struct VotePowerCustodyPosition<Balance, Epoch> {
  pub amount: Balance,
  pub lock_until: Epoch,
}

pub trait VotePowerCustody<AccountId, DomainId, LockId, Balance> {
  fn lock_id(_domain: DomainId, _track: ProposalTrackFamily) -> Option<LockId> {
    None
  }

  fn target_amount(
    _domain: DomainId,
    _track: ProposalTrackFamily,
    _account: &AccountId,
    current_locked: Balance,
  ) -> Balance {
    current_locked
  }

  fn lock(
    _account: &AccountId,
    _lock_id: LockId,
    _amount: Balance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    Ok(())
  }

  fn unlock(
    _account: &AccountId,
    _lock_id: LockId,
    _amount: Balance,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    Ok(())
  }
}

impl<AccountId, DomainId, LockId, Balance> VotePowerCustody<AccountId, DomainId, LockId, Balance>
  for ()
{
}

#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum ProposalVotePowerProfile {
  DecliningDirectStake,
  DecliningVetoAsset,
  DecliningNativeStake,
  FlatUrgentDirectStake,
}

#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub struct GovernanceDomainPolicy {
  pub ordinary_power_profile: ProposalVotePowerProfile,
  pub protection_power_profile: ProposalVotePowerProfile,
}

pub trait GovernanceDomainPolicyProvider<DomainId> {
  fn policy(domain: DomainId) -> GovernanceDomainPolicy;
}

pub trait ProposalTrackPowerProfileProvider<DomainId, ItemId> {
  fn power_profile(
    domain: DomainId,
    item_id: ItemId,
    track: ProposalTrackFamily,
  ) -> ProposalVotePowerProfile;
}

#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum ProposalPrimaryTrackFamily {
  Binary,
  Invoice,
}

pub trait ProposalPrimaryTrackFamilyProvider<DomainId> {
  fn family(_domain: DomainId, _payload_kind: ProposalPayloadKind) -> ProposalPrimaryTrackFamily {
    ProposalPrimaryTrackFamily::Binary
  }
}

pub trait ProposalUrgentPolicyProvider<DomainId> {
  fn is_expeditable(_domain: DomainId, _payload_kind: ProposalPayloadKind) -> bool {
    false
  }
  fn executes_immediately_on_unanimous_pass(
    _domain: DomainId,
    _payload_kind: ProposalPayloadKind,
  ) -> bool {
    false
  }
}

#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum ProposalSubmissionAuthority {
  Signed,
  PrimaryEligibleSigned,
  AdminOnly,
}

#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum ProposalCapacityLane {
  General,
  Strategic,
}

#[derive(
  Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub struct ProposalAdmissionPolicyView<Balance> {
  pub authority: ProposalSubmissionAuthority,
  pub capacity_lane: ProposalCapacityLane,
  pub capacity_limit: u32,
  pub author_limit: u32,
  pub signed_preimage_required: bool,
  pub opening_fee: Option<Balance>,
}

pub trait ProposalSubmissionAuthorityProvider<DomainId> {
  fn authority(
    _domain: DomainId,
    _payload_kind: ProposalPayloadKind,
  ) -> ProposalSubmissionAuthority {
    ProposalSubmissionAuthority::AdminOnly
  }
}

#[derive(
  Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub struct AuthorizedRuntimeUpgrade<Hash> {
  pub code_hash: Hash,
  pub check_version: bool,
}

pub trait ProposalRuntimeUpgradeAuthorizationProvider<Hash> {
  fn authorized_upgrade() -> Option<AuthorizedRuntimeUpgrade<Hash>> {
    None
  }
}

pub trait ProposalPayloadPreimageNoteCostProvider<Balance> {
  fn note_cost(_payload_len: u32) -> Option<Balance> {
    None
  }
}

impl<AccountId, DomainId, ItemId, Epoch>
  ProposalVoteWeightProvider<AccountId, DomainId, ItemId, Epoch> for ()
{
  fn vote_weight(
    _domain: DomainId,
    _context: &ProposalVoteContext<ItemId, Epoch>,
    _account: &AccountId,
  ) -> u32 {
    1
  }
}

impl<DomainId> GovernanceDomainPolicyProvider<DomainId> for () {
  fn policy(_domain: DomainId) -> GovernanceDomainPolicy {
    GovernanceDomainPolicy {
      ordinary_power_profile: ProposalVotePowerProfile::DecliningDirectStake,
      protection_power_profile: ProposalVotePowerProfile::DecliningVetoAsset,
    }
  }
}

impl<DomainId, ItemId> ProposalTrackPowerProfileProvider<DomainId, ItemId> for () {
  fn power_profile(
    _domain: DomainId,
    _item_id: ItemId,
    track: ProposalTrackFamily,
  ) -> ProposalVotePowerProfile {
    match track {
      ProposalTrackFamily::Ordinary => ProposalVotePowerProfile::DecliningDirectStake,
      ProposalTrackFamily::Veto => ProposalVotePowerProfile::DecliningVetoAsset,
    }
  }
}

impl<DomainId> ProposalPrimaryTrackFamilyProvider<DomainId> for () {}

impl<DomainId> ProposalUrgentPolicyProvider<DomainId> for () {}

impl<DomainId> ProposalSubmissionAuthorityProvider<DomainId> for () {}

pub trait ProposalSubmissionEligibilityProvider<AccountId, DomainId> {
  fn has_primary_governance_power(_domain: DomainId, _account: &AccountId) -> bool {
    false
  }
}

impl<AccountId, DomainId> ProposalSubmissionEligibilityProvider<AccountId, DomainId> for () {}

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AccountId, DomainId, Hash, LockId, Balance> {
  fn prepare_primary_eligible_submitter(
    account: &AccountId,
  ) -> Result<(DomainId, Hash), polkadot_sdk::sp_runtime::DispatchError>;

  fn prepare_protection_voter(
    account: &AccountId,
  ) -> Result<DomainId, polkadot_sdk::sp_runtime::DispatchError>;

  fn prepare_vote_power_custody(
    account: &AccountId,
  ) -> Result<(LockId, Balance), polkadot_sdk::sp_runtime::DispatchError>;
}

#[cfg(feature = "runtime-benchmarks")]
impl<AccountId, DomainId, Hash, LockId, Balance>
  BenchmarkHelper<AccountId, DomainId, Hash, LockId, Balance> for ()
{
  fn prepare_primary_eligible_submitter(
    _account: &AccountId,
  ) -> Result<(DomainId, Hash), polkadot_sdk::sp_runtime::DispatchError> {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "GovernanceBenchmarkHelperNotConfigured",
    ))
  }

  fn prepare_protection_voter(
    _account: &AccountId,
  ) -> Result<DomainId, polkadot_sdk::sp_runtime::DispatchError> {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "GovernanceBenchmarkHelperNotConfigured",
    ))
  }

  fn prepare_vote_power_custody(
    _account: &AccountId,
  ) -> Result<(LockId, Balance), polkadot_sdk::sp_runtime::DispatchError> {
    Err(polkadot_sdk::sp_runtime::DispatchError::Other(
      "GovernanceBenchmarkHelperNotConfigured",
    ))
  }
}

impl<Hash> ProposalRuntimeUpgradeAuthorizationProvider<Hash> for () {}

impl<Balance> ProposalPayloadPreimageNoteCostProvider<Balance> for () {}

pub trait VetoVotePowerProvider<AccountId, DomainId, ItemId, Epoch> {
  fn vote_weight(
    domain: DomainId,
    context: &ProposalVoteContext<ItemId, Epoch>,
    account: &AccountId,
  ) -> u64;
  fn raw_vote_weight(domain: DomainId, account: &AccountId) -> u64;
  fn total_issuance(domain: DomainId) -> u64;
}

pub trait ProposalPayloadPreimageProvider<Hash, DomainId> {
  fn have_preimage(hash: &Hash) -> bool;
  fn preimage_requested(hash: &Hash) -> bool;
  fn preimage_len(_hash: &Hash) -> Option<u32> {
    None
  }
  fn validate_for_submission(
    _domain: DomainId,
    _payload_kind: ProposalPayloadKind,
    hash: &Hash,
  ) -> Result<(), ProposalPreimageAdmissionError> {
    if Self::have_preimage(hash) {
      Ok(())
    } else {
      Err(ProposalPreimageAdmissionError::Missing)
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProposalPreimageAdmissionError {
  Missing,
  Oversized,
  Invalid,
  Incompatible,
}

#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum ProposalParameterChangeSurface {
  RouterFee,
}

#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum ProposalTreasurySpendSettlementKind {
  InvoiceScalarTransfer,
}

#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum ProposalTreasurySpendScalar {
  Amplify,
  Approve,
  Reduce,
}

#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum ProposalExecutionFailureReason {
  MissingPreimage,
  InvalidPreimage,
  UnsupportedDomain,
  UnsupportedCall,
  MissingWinningPrimaryOption,
  DispatchFailed,
}

#[derive(
  Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum ProposalExecutionSuccessDetail<AccountId, DomainId, Hash> {
  Generic,
  RuntimeUpgradeAuthorized {
    code_hash: Hash,
  },
  ParameterChangeExecuted {
    surface: ProposalParameterChangeSurface,
  },
  TreasurySpendExecuted {
    funding_source: AccountId,
    beneficiary: AccountId,
    payout_asset: DomainId,
    base_amount: u128,
    scalar: ProposalTreasurySpendScalar,
    final_amount: u128,
    settlement_kind: ProposalTreasurySpendSettlementKind,
  },
}

#[derive(
  Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum ProposalExecutionDetail<AccountId, DomainId, Hash> {
  Succeeded(ProposalExecutionSuccessDetail<AccountId, DomainId, Hash>),
  Failed(ProposalExecutionFailureReason),
}

pub enum ProposalExecutionReceipt<AccountId, DomainId, Hash> {
  Generic,
  RuntimeUpgradeAuthorized {
    code_hash: Hash,
  },
  ParameterChangeExecuted {
    surface: ProposalParameterChangeSurface,
  },
  TreasurySpendExecuted {
    funding_source: AccountId,
    beneficiary: AccountId,
    payout_asset: DomainId,
    base_amount: u128,
    scalar: ProposalTreasurySpendScalar,
    final_amount: u128,
    settlement_kind: ProposalTreasurySpendSettlementKind,
  },
}

pub trait ProposalPayloadExecutor<AccountId, DomainId, ItemId, Hash> {
  fn can_execute(_payload_kind: ProposalPayloadKind) -> bool {
    false
  }
  fn execute(
    _domain: DomainId,
    _item_id: ItemId,
    _payload_kind: ProposalPayloadKind,
    _payload_hash: Hash,
  ) -> Result<ProposalExecutionReceipt<AccountId, DomainId, Hash>, ProposalExecutionFailureReason>
  {
    Ok(ProposalExecutionReceipt::Generic)
  }
}

impl<AccountId, DomainId, ItemId, Epoch> VetoVotePowerProvider<AccountId, DomainId, ItemId, Epoch>
  for ()
{
  fn vote_weight(
    _domain: DomainId,
    _context: &ProposalVoteContext<ItemId, Epoch>,
    _account: &AccountId,
  ) -> u64 {
    0
  }

  fn raw_vote_weight(_domain: DomainId, _account: &AccountId) -> u64 {
    0
  }

  fn total_issuance(_domain: DomainId) -> u64 {
    0
  }
}

impl<Hash, DomainId> ProposalPayloadPreimageProvider<Hash, DomainId> for () {
  fn have_preimage(_hash: &Hash) -> bool {
    false
  }

  fn preimage_requested(_hash: &Hash) -> bool {
    false
  }
}

impl<AccountId, DomainId, ItemId, Hash> ProposalPayloadExecutor<AccountId, DomainId, ItemId, Hash>
  for ()
{
}

pub trait WeightInfo {
  fn record_winning_vote() -> Weight;
  fn record_winning_vote_batch(accounts: u32) -> Weight;
  fn submit_proposal() -> Weight;
  fn submit_signed_proposal() -> Weight;
  fn cast_vote() -> Weight;
  fn unlock_vote_power() -> Weight;
  fn resolve_proposal(accounts: u32) -> Weight;
  fn resolve_proposal_from_votes(accounts: u32) -> Weight;
  fn force_resolve_proposal_from_votes(accounts: u32) -> Weight;
  fn reject_proposal() -> Weight;
  fn requeue_proposal_for_auto_finalization() -> Weight;
  fn service_maturing_proposals(entries: u32) -> Weight;
  fn service_finalized_proposal_outcomes(entries: u32) -> Weight;
  fn service_expiring_accounts(entries: u32) -> Weight;
}

impl WeightInfo for () {
  fn record_winning_vote() -> Weight {
    Weight::zero()
  }

  fn record_winning_vote_batch(_accounts: u32) -> Weight {
    Weight::zero()
  }

  fn submit_proposal() -> Weight {
    Weight::zero()
  }

  fn submit_signed_proposal() -> Weight {
    Weight::zero()
  }

  fn cast_vote() -> Weight {
    Weight::zero()
  }

  fn unlock_vote_power() -> Weight {
    Weight::zero()
  }

  fn resolve_proposal(_accounts: u32) -> Weight {
    Weight::zero()
  }

  fn resolve_proposal_from_votes(_accounts: u32) -> Weight {
    Weight::zero()
  }

  fn force_resolve_proposal_from_votes(_accounts: u32) -> Weight {
    Weight::zero()
  }

  fn reject_proposal() -> Weight {
    Weight::zero()
  }

  fn requeue_proposal_for_auto_finalization() -> Weight {
    Weight::zero()
  }

  fn service_maturing_proposals(_entries: u32) -> Weight {
    Weight::zero()
  }

  fn service_finalized_proposal_outcomes(_entries: u32) -> Weight {
    Weight::zero()
  }

  fn service_expiring_accounts(_entries: u32) -> Weight {
    Weight::zero()
  }
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod epoch_service;
mod proposal_execution;
mod proposal_resolution;
mod reward_memory;

const STORAGE_VERSION: StorageVersion = StorageVersion::new(3);

#[frame::pallet]
pub mod pallet {
  use crate::{
    EpochProvider as _, GovernanceDomainPolicyProvider as _,
    ProposalPayloadPreimageNoteCostProvider as _, ProposalPayloadPreimageProvider as _,
    ProposalRuntimeUpgradeAuthorizationProvider as _, VotePowerCustody as _, WeightInfo as _,
  };
  use codec::{Decode, Encode};
  use frame::prelude::*;
  use polkadot_sdk::frame_support::{
    traits::{Currency, ExistenceRequirement},
    transactional,
  };
  use polkadot_sdk::sp_runtime::{
    FixedU128, Perbill,
    traits::{AtLeast32BitUnsigned, Zero},
  };

  pub type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

  #[pallet::config]
  pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
    type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    type Currency: Currency<Self::AccountId>;
    #[pallet::constant]
    type ProposalOpeningFee: Get<BalanceOf<Self>>;
    type ProposalFeeRecipient: Get<Self::AccountId>;
    type DomainId: Parameter + MaxEncodedLen + Member + Copy + Ord + TypeInfo;
    type VotePowerLockId: Parameter + MaxEncodedLen + Member + Copy + Ord + TypeInfo;
    type VotePowerCustody: crate::VotePowerCustody<
        Self::AccountId,
        Self::DomainId,
        Self::VotePowerLockId,
        BalanceOf<Self>,
      >;
    type WinningVoteItemId: Parameter + MaxEncodedLen + Member + Copy + Ord + TypeInfo;
    type Epoch: Parameter
      + MaxEncodedLen
      + Member
      + Copy
      + Ord
      + TypeInfo
      + AtLeast32BitUnsigned
      + Default;
    type EpochProvider: crate::EpochProvider<Self::Epoch>;
    #[pallet::constant]
    type WinningVoteLookbackEpochs: Get<u32>;
    #[pallet::constant]
    type MaxWinningVotesPerEpoch: Get<u16>;
    #[pallet::constant]
    type MaxWinningVoteItemsPerEpoch: Get<u32>;
    #[pallet::constant]
    type MaxWinningVoteResolutionItemsPerEpoch: Get<u32>;
    #[pallet::constant]
    type MaxWinningVoteAccountsPerCall: Get<u32>;
    #[pallet::constant]
    type MaxActiveProposalsPerDomain: Get<u32>;
    /// Capacity withheld from general proposals for protocol-domain strategic ingress.
    #[pallet::constant]
    type StrategicProposalReserve: Get<u32>;
    /// Maximum proposals one author may keep active in one domain.
    #[pallet::constant]
    type MaxActiveProposalsPerAuthor: Get<u32>;
    #[pallet::constant]
    type MaxMaturingProposalsPerEpoch: Get<u32>;
    #[pallet::constant]
    type MaxPendingEnactmentsPerEpoch: Get<u32>;
    #[pallet::constant]
    type ProposalVotingPeriod: Get<Self::Epoch>;
    #[pallet::constant]
    type ProposalLeadInPeriod: Get<Self::Epoch>;
    #[pallet::constant]
    type ProposalProtectionPeriod: Get<Self::Epoch>;
    #[pallet::constant]
    type ProposalUrgentVotingPeriod: Get<Self::Epoch>;
    #[pallet::constant]
    type ProposalEnactmentDelay: Get<Self::Epoch>;
    #[pallet::constant]
    type ProposalFastTrackPassThreshold: Get<Perbill>;
    #[pallet::constant]
    type ProposalApprovalThreshold: Get<Perbill>;
    #[pallet::constant]
    type ProposalVetoThreshold: Get<Perbill>;
    #[pallet::constant]
    type ProposalVetoMinimumVetoTurnout: Get<Perbill>;
    #[pallet::constant]
    type ProposalMinimumTurnout: Get<u64>;
    /// Confirm period: number of epochs a passing proposal must sustain approval
    /// before finalization. Zero disables confirm (immediate finalization at maturity).
    #[pallet::constant]
    type ProposalConfirmPeriod: Get<Self::Epoch>;
    #[pallet::constant]
    type FinalizedProposalOutcomeRetentionEpochs: Get<u32>;
    #[pallet::constant]
    type MaxFinalizedProposalOutcomesPerEpoch: Get<u32>;
    #[pallet::constant]
    type MaxRecentFinalizedProposalsPerDomain: Get<u32>;
    #[pallet::constant]
    type MaxExpiringAccountsPerEpoch: Get<u32>;
    type ProposalVoteWeightProvider: crate::ProposalVoteWeightProvider<
        Self::AccountId,
        Self::DomainId,
        Self::WinningVoteItemId,
        Self::Epoch,
      >;
    type GovernanceDomainPolicyProvider: crate::GovernanceDomainPolicyProvider<Self::DomainId>;
    type ProposalTrackPowerProfileProvider: crate::ProposalTrackPowerProfileProvider<Self::DomainId, Self::WinningVoteItemId>;
    type ProposalPrimaryTrackFamilyProvider: crate::ProposalPrimaryTrackFamilyProvider<Self::DomainId>;
    type ProposalUrgentPolicyProvider: crate::ProposalUrgentPolicyProvider<Self::DomainId>;
    type ProposalSubmissionAuthorityProvider: crate::ProposalSubmissionAuthorityProvider<Self::DomainId>;
    type ProposalSubmissionEligibilityProvider: crate::ProposalSubmissionEligibilityProvider<Self::AccountId, Self::DomainId>;
    type ProposalRuntimeUpgradeAuthorizationProvider: crate::ProposalRuntimeUpgradeAuthorizationProvider<Self::Hash>;
    type ProposalPayloadPreimageNoteCostProvider: crate::ProposalPayloadPreimageNoteCostProvider<BalanceOf<Self>>;
    type VetoVotePowerProvider: crate::VetoVotePowerProvider<
        Self::AccountId,
        Self::DomainId,
        Self::WinningVoteItemId,
        Self::Epoch,
      >;
    type ProposalPayloadPreimageProvider: crate::ProposalPayloadPreimageProvider<Self::Hash, Self::DomainId>;
    type ProposalPayloadExecutor: crate::ProposalPayloadExecutor<
        Self::AccountId,
        Self::DomainId,
        Self::WinningVoteItemId,
        Self::Hash,
      >;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper: crate::BenchmarkHelper<
        Self::AccountId,
        Self::DomainId,
        Self::Hash,
        Self::VotePowerLockId,
        BalanceOf<Self>,
      >;
    type WeightInfo: crate::WeightInfo;
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  #[scale_info(skip_type_params(MaxWinningVoteItemsPerEpoch))]
  pub struct WinningVoteEpochSlot<ItemId, MaxWinningVoteItemsPerEpoch: Get<u32>> {
    pub item_ids: BoundedVec<ItemId, MaxWinningVoteItemsPerEpoch>,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  #[scale_info(skip_type_params(MaxWinningVoteLookbackEpochs, MaxWinningVoteItemsPerEpoch))]
  pub struct WinningVoteWindow<
    Epoch,
    ItemId,
    MaxWinningVoteLookbackEpochs: Get<u32>,
    MaxWinningVoteItemsPerEpoch: Get<u32>,
  > {
    pub last_epoch: Epoch,
    pub epochs: BoundedVec<
      WinningVoteEpochSlot<ItemId, MaxWinningVoteItemsPerEpoch>,
      MaxWinningVoteLookbackEpochs,
    >,
    pub rolling_sum: u32,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  #[scale_info(skip_type_params(
    MaxWinningVoteLookbackEpochs,
    MaxWinningVoteResolutionItemsPerEpoch
  ))]
  pub struct WinningVoteResolutionWindow<
    Epoch,
    ItemId,
    MaxWinningVoteLookbackEpochs: Get<u32>,
    MaxWinningVoteResolutionItemsPerEpoch: Get<u32>,
  > {
    pub last_epoch: Epoch,
    pub epochs: BoundedVec<
      WinningVoteEpochSlot<ItemId, MaxWinningVoteResolutionItemsPerEpoch>,
      MaxWinningVoteLookbackEpochs,
    >,
  }

  #[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
  )]
  pub struct ParticipationTotals {
    pub total_participations: u64,
    pub winning_participations: u64,
  }

  #[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
  )]
  pub struct ProposalAuthorshipTotals {
    pub authored_proposals: u64,
    pub successful_authored_proposals: u64,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct GovXpCounters {
    pub rolling_winning_participation: u32,
    pub total_participations: u64,
    pub total_winning_participations: u64,
    pub total_authored_proposals: u64,
    pub total_successful_authored_proposals: u64,
  }

  #[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
  )]
  pub enum ProposalVoteKind {
    Aye,
    Nay,
    Amplify,
    Approve,
    Reduce,
    Veto,
    Pass,
  }

  #[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
  )]
  pub enum VetoCancellationMode {
    ImmediateThreshold,
    TrackOutcome,
  }

  #[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
  )]
  pub enum ProposalRejectionReason {
    AdminRejected,
    NoVotes,
    VoteTie,
    TurnoutBelowMinimum,
    ApprovalThresholdNotMet,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct ProposalVoteTally {
    pub aye_voters: u32,
    pub nay_voters: u32,
    pub amplify_voters: u32,
    pub approve_voters: u32,
    pub reduce_voters: u32,
    pub veto_voters: u32,
    pub pass_voters: u32,
    pub aye_weight: u64,
    pub nay_weight: u64,
    pub amplify_weight: u64,
    pub approve_weight: u64,
    pub reduce_weight: u64,
    pub veto_weight: u64,
    pub pass_weight: u64,
    pub turnout_weight: u64,
    pub veto_turnout_weight: u64,
  }

  #[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
  )]
  pub enum ProposalPrimaryTrackOption {
    Aye,
    Nay,
    Amplify,
    Approve,
    Reduce,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub enum ProposalPrimaryTrackTally {
    Binary {
      aye_voters: u32,
      nay_voters: u32,
      aye_weight: u64,
      nay_weight: u64,
      turnout_weight: u64,
      leading_option: Option<ProposalPrimaryTrackOption>,
    },
    Invoice {
      amplify_voters: u32,
      approve_voters: u32,
      reduce_voters: u32,
      nay_voters: u32,
      amplify_weight: u64,
      approve_weight: u64,
      reduce_weight: u64,
      nay_weight: u64,
      positive_weight: u64,
      turnout_weight: u64,
      leading_positive_option: Option<ProposalPrimaryTrackOption>,
      leading_positive_weight: u64,
    },
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub enum ProposalResolutionState<Epoch> {
    VotingWindowOpen {
      current_epoch: Epoch,
      maturity_epoch: Epoch,
    },
    VetoPassing {
      veto_weight: u64,
      pass_weight: u64,
      mode: VetoCancellationMode,
    },
    PassingAye,
    PassingAmplify,
    PassingApprove,
    PassingReduce,
    PassingNay,
    Confirming {
      confirm_started_epoch: Epoch,
      confirm_end_epoch: Epoch,
    },
    Rejected {
      reason: ProposalRejectionReason,
    },
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct ProposalIdentity<DomainId, ItemId> {
    pub domain: DomainId,
    pub item_id: ItemId,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct ProposalApproval<Epoch> {
    pub approved_epoch: Epoch,
    pub winner_count: u32,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub enum ProposalEnactmentOutcome<Epoch> {
    NotAttempted,
    Enacted { epoch: Epoch },
    ExecutionFailed { epoch: Epoch },
    AdvisoryFinalized { epoch: Epoch },
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub enum FinalizedProposalOutcome<Epoch> {
    Approved {
      approval: ProposalApproval<Epoch>,
      enactment: ProposalEnactmentOutcome<Epoch>,
    },
    Rejected {
      finalized_epoch: Epoch,
      reason: ProposalRejectionReason,
    },
    VetoCancelled {
      finalized_epoch: Epoch,
      veto_weight: u64,
    },
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct FinalizedProposalRecord<AccountId, DomainId, Hash, Epoch> {
    pub outcome: FinalizedProposalOutcome<Epoch>,
    pub execution_detail: Option<crate::ProposalExecutionDetail<AccountId, DomainId, Hash>>,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub enum ProposalStatus<Epoch> {
    Active(ProposalResolutionState<Epoch>),
    PendingEnactment {
      approval: ProposalApproval<Epoch>,
      enactment_epoch: Epoch,
    },
    Finalized(FinalizedProposalOutcome<Epoch>),
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct RecentFinalizedProposal<AccountId, DomainId, ItemId, Hash, Epoch> {
    pub identity: ProposalIdentity<DomainId, ItemId>,
    pub finalization: FinalizedProposalRecord<AccountId, DomainId, Hash, Epoch>,
  }

  #[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
  )]
  pub enum ProposalCadenceMode {
    Ordinary,
    Fast,
  }

  #[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
  )]
  pub enum ProposalPayloadKind {
    L1RootAction,
    L2TreasurySpend,
    L2ParameterChange,
    Intent,
    L2SignalToL1,
  }

  #[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
  )]
  pub enum ProposalExecutionAuthority {
    Root,
    DomainTreasury,
    DomainParameters,
    NonExecutable,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct ProposalMetadata<Hash> {
    pub cadence_mode: ProposalCadenceMode,
    pub payload_kind: ProposalPayloadKind,
    pub payload_hash: Hash,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct PayloadHashPreimageStatus {
    pub have_preimage: bool,
    pub preimage_requested: bool,
    pub payload_len: Option<u32>,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct ActiveProposal<Epoch> {
    pub submitted_epoch: Epoch,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct ProposalTiming<Epoch> {
    pub submitted_epoch: Epoch,
    pub protection_open_epoch: Epoch,
    pub protection_close_epoch: Epoch,
    pub ordinary_primary_open_epoch: Epoch,
    pub ordinary_primary_close_epoch: Epoch,
    pub urgent_primary_open_epoch: Option<Epoch>,
    pub urgent_primary_close_epoch: Option<Epoch>,
    pub effective_primary_open_epoch: Epoch,
    pub effective_primary_close_epoch: Epoch,
    pub pending_enactment_epoch: Option<Epoch>,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct ProposalBallot<AccountId, Epoch> {
    pub account: AccountId,
    pub vote_epoch: Epoch,
    pub weight: u64,
    pub raw_power: u64,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct GovernanceLock<Epoch> {
    pub lock_until: Epoch,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct ProposalFrozenBallot<Epoch> {
    pub vote: ProposalVoteKind,
    pub vote_epoch: Epoch,
    pub weight: u64,
    pub raw_power: u64,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct AccountGovernancePowerView<Epoch> {
    pub governance_lock_until: Option<Epoch>,
    pub ordinary_power_profile: crate::ProposalVotePowerProfile,
    pub protection_power_profile: crate::ProposalVotePowerProfile,
    pub current_ordinary_weight: u64,
    pub current_protection_weight: u64,
    pub current_protection_raw_power: u64,
    pub frozen_ordinary_ballot: Option<ProposalFrozenBallot<Epoch>>,
    pub frozen_protection_ballot: Option<ProposalFrozenBallot<Epoch>>,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  #[scale_info(skip_type_params(MaxProposalVotesPerDirection))]
  pub struct ProposalVotes<AccountId, Epoch, MaxProposalVotesPerDirection: Get<u32>> {
    pub ayes: BoundedVec<ProposalBallot<AccountId, Epoch>, MaxProposalVotesPerDirection>,
    pub nays: BoundedVec<ProposalBallot<AccountId, Epoch>, MaxProposalVotesPerDirection>,
    pub amplifies: BoundedVec<ProposalBallot<AccountId, Epoch>, MaxProposalVotesPerDirection>,
    pub approves: BoundedVec<ProposalBallot<AccountId, Epoch>, MaxProposalVotesPerDirection>,
    pub reduces: BoundedVec<ProposalBallot<AccountId, Epoch>, MaxProposalVotesPerDirection>,
    pub vetoes: BoundedVec<ProposalBallot<AccountId, Epoch>, MaxProposalVotesPerDirection>,
    pub passes: BoundedVec<ProposalBallot<AccountId, Epoch>, MaxProposalVotesPerDirection>,
  }

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub struct VetoCancellation {
    pub veto_weight: u64,
    pub pass_weight: u64,
    pub mode: VetoCancellationMode,
  }

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub struct UrgentAuthorization {
    pub pass_weight: u64,
    pub total_protection_supply: u64,
  }

  pub type MaturingProposalTouch<DomainId, ItemId> = ProposalIdentity<DomainId, ItemId>;
  pub type FinalizedProposalTouch<DomainId, ItemId> = ProposalIdentity<DomainId, ItemId>;

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  #[scale_info(skip_type_params(MaxExpiringAccountsPerEpoch))]
  pub struct ExpiringAccountTouch<DomainId, AccountId> {
    pub domain: DomainId,
    pub account: AccountId,
  }

  #[pallet::pallet]
  #[pallet::storage_version(crate::STORAGE_VERSION)]
  pub struct Pallet<T>(_);

  #[pallet::storage]
  #[pallet::getter(fn winning_vote_window)]
  pub type WinningVoteWindows<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::DomainId,
    Blake2_128Concat,
    T::AccountId,
    WinningVoteWindow<
      T::Epoch,
      T::WinningVoteItemId,
      T::WinningVoteLookbackEpochs,
      T::MaxWinningVoteItemsPerEpoch,
    >,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn participation_totals)]
  pub type ParticipationTotalsByAccount<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::DomainId,
    Blake2_128Concat,
    T::AccountId,
    ParticipationTotals,
    ValueQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn proposal_authorship_totals)]
  pub type ProposalAuthorshipTotalsByAccount<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::DomainId,
    Blake2_128Concat,
    T::AccountId,
    ProposalAuthorshipTotals,
    ValueQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn winning_vote_resolution_window)]
  pub type WinningVoteResolutionWindows<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::DomainId,
    WinningVoteResolutionWindow<
      T::Epoch,
      T::WinningVoteItemId,
      T::WinningVoteLookbackEpochs,
      T::MaxWinningVoteResolutionItemsPerEpoch,
    >,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn active_proposal)]
  pub type ActiveProposals<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::DomainId,
    Blake2_128Concat,
    T::WinningVoteItemId,
    ActiveProposal<T::Epoch>,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn proposal_author)]
  pub type ProposalAuthorsByItem<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::DomainId,
    Blake2_128Concat,
    T::WinningVoteItemId,
    T::AccountId,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn proposal_metadata)]
  pub type ProposalMetadataByItem<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::DomainId,
    Blake2_128Concat,
    T::WinningVoteItemId,
    ProposalMetadata<T::Hash>,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn proposal_votes)]
  pub type ProposalVotesByItem<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::DomainId,
    Blake2_128Concat,
    T::WinningVoteItemId,
    ProposalVotes<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn governance_lock)]
  pub type GovernanceLocks<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AccountId, GovernanceLock<T::Epoch>, OptionQuery>;

  #[pallet::storage]
  #[pallet::getter(fn vote_power_custody)]
  pub type VotePowerCustodyByAccount<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    Blake2_128Concat,
    T::VotePowerLockId,
    crate::VotePowerCustodyPosition<BalanceOf<T>, T::Epoch>,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn active_proposal_ids)]
  pub type ActiveProposalIdsByDomain<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::DomainId,
    BoundedVec<T::WinningVoteItemId, T::MaxActiveProposalsPerDomain>,
    ValueQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn proposal_maturity_bucket)]
  pub type ProposalMaturityBuckets<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::Epoch,
    BoundedVec<
      MaturingProposalTouch<T::DomainId, T::WinningVoteItemId>,
      T::MaxMaturingProposalsPerEpoch,
    >,
    ValueQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn finalized_proposal)]
  pub type FinalizedProposals<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::DomainId,
    Blake2_128Concat,
    T::WinningVoteItemId,
    FinalizedProposalRecord<T::AccountId, T::DomainId, T::Hash, T::Epoch>,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn proposal_winning_primary_option)]
  pub type ProposalWinningPrimaryOptionByItem<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::DomainId,
    Blake2_128Concat,
    T::WinningVoteItemId,
    crate::ProposalPrimaryTrackOption,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn proposal_urgent_authorized_at)]
  pub type ProposalUrgentAuthorizedAt<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::DomainId,
    Blake2_128Concat,
    T::WinningVoteItemId,
    T::Epoch,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn proposal_pending_enactment_at)]
  pub type ProposalPendingEnactmentAt<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::DomainId,
    Blake2_128Concat,
    T::WinningVoteItemId,
    T::Epoch,
    OptionQuery,
  >;

  #[pallet::storage]
  pub type PendingEnactmentBuckets<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::Epoch,
    BoundedVec<
      FinalizedProposalTouch<T::DomainId, T::WinningVoteItemId>,
      T::MaxPendingEnactmentsPerEpoch,
    >,
    ValueQuery,
  >;

  /// Epoch at which a proposal entered the confirm period.
  /// Present only when `ProposalConfirmPeriod > 0` and the proposal is sustaining approval.
  #[pallet::storage]
  pub type ProposalConfirmStartedAt<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::DomainId,
    Blake2_128Concat,
    T::WinningVoteItemId,
    T::Epoch,
    OptionQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn finalized_proposal_outcome_expiry_bucket)]
  pub type FinalizedProposalOutcomeExpiryBuckets<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::Epoch,
    BoundedVec<
      FinalizedProposalTouch<T::DomainId, T::WinningVoteItemId>,
      T::MaxFinalizedProposalOutcomesPerEpoch,
    >,
    ValueQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn expiry_bucket)]
  pub type ExpiryBuckets<T: Config> = StorageMap<
    _,
    Blake2_128Concat,
    T::Epoch,
    BoundedVec<ExpiringAccountTouch<T::DomainId, T::AccountId>, T::MaxExpiringAccountsPerEpoch>,
    ValueQuery,
  >;

  #[pallet::storage]
  #[pallet::getter(fn last_processed_epoch)]
  pub type LastProcessedEpoch<T: Config> = StorageValue<_, T::Epoch, ValueQuery>;

  #[pallet::event]
  #[pallet::generate_deposit(pub(super) fn deposit_event)]
  pub enum Event<T: Config> {
    ProposalOpeningFeeCollected {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      proposer: T::AccountId,
      amount: BalanceOf<T>,
    },
    ProposalSubmitted {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      proposer: T::AccountId,
      cadence_mode: ProposalCadenceMode,
      payload_kind: ProposalPayloadKind,
      payload_hash: T::Hash,
      epoch: T::Epoch,
      active_count: u32,
    },
    ProposalVoteCast {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      account: T::AccountId,
      vote: ProposalVoteKind,
      replaced_vote: Option<ProposalVoteKind>,
      vote_epoch: T::Epoch,
      aye_count: u32,
      nay_count: u32,
      veto_count: u32,
      pass_count: u32,
    },
    GovernanceLockExtended {
      account: T::AccountId,
      previous_lock_until: Option<T::Epoch>,
      new_lock_until: T::Epoch,
    },
    VotePowerCustodyUpdated {
      account: T::AccountId,
      lock_id: T::VotePowerLockId,
      previous_amount: BalanceOf<T>,
      new_amount: BalanceOf<T>,
      lock_until: T::Epoch,
    },
    VotePowerCustodyReleased {
      account: T::AccountId,
      lock_id: T::VotePowerLockId,
      amount: BalanceOf<T>,
    },
    ProposalUrgentAuthorized {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      authorization_epoch: T::Epoch,
      pass_weight: u64,
      total_protection_supply: u64,
    },
    ProposalAutoFinalizationDeferred {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      epoch: T::Epoch,
      rescheduled: bool,
    },
    ProposalAutoFinalizationRequeued {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      epoch: T::Epoch,
      maturity_epoch: T::Epoch,
    },
    ProposalConfirmStarted {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      confirm_started_epoch: T::Epoch,
      confirm_end_epoch: T::Epoch,
    },
    ProposalConfirmReset {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      epoch: T::Epoch,
    },
    ProposalResolved {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      epoch: T::Epoch,
      winner_count: u32,
      active_count: u32,
    },
    ProposalEnactmentScheduled {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      finalized_epoch: T::Epoch,
      enactment_epoch: T::Epoch,
    },
    ProposalExecuted {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      approved_epoch: T::Epoch,
      executed_epoch: T::Epoch,
      authority: ProposalExecutionAuthority,
      payload_kind: ProposalPayloadKind,
    },
    ProposalRuntimeUpgradeAuthorized {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      approved_epoch: T::Epoch,
      executed_epoch: T::Epoch,
      code_hash: T::Hash,
    },
    ProposalParameterChangeExecuted {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      approved_epoch: T::Epoch,
      executed_epoch: T::Epoch,
      surface: crate::ProposalParameterChangeSurface,
    },
    ProposalTreasurySpendExecuted {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      approved_epoch: T::Epoch,
      executed_epoch: T::Epoch,
      funding_source: T::AccountId,
      beneficiary: T::AccountId,
      payout_asset: T::DomainId,
      base_amount: u128,
      scalar: crate::ProposalTreasurySpendScalar,
      final_amount: u128,
      settlement_kind: crate::ProposalTreasurySpendSettlementKind,
    },
    ProposalExecutionFailed {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      approved_epoch: T::Epoch,
      failed_epoch: T::Epoch,
      authority: ProposalExecutionAuthority,
      payload_kind: ProposalPayloadKind,
      reason: crate::ProposalExecutionFailureReason,
    },
    ProposalAdvisoryFinalized {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      approved_epoch: T::Epoch,
      finalized_epoch: T::Epoch,
      payload_kind: ProposalPayloadKind,
    },
    ProposalRejected {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      epoch: T::Epoch,
      reason: ProposalRejectionReason,
      active_count: u32,
    },
    ProposalVetoCancelled {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      epoch: T::Epoch,
      veto_weight: u64,
      pass_weight: u64,
      mode: VetoCancellationMode,
      active_count: u32,
    },
    WinningVoteRecorded {
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      account: T::AccountId,
      epoch: T::Epoch,
      epoch_count: u16,
      rolling_sum: u32,
    },
    WinningVoteWindowEvicted {
      domain: T::DomainId,
      account: T::AccountId,
      epoch: T::Epoch,
    },
  }

  #[pallet::error]
  pub enum Error<T> {
    ZeroLookbackWindow,
    ProposalAlreadyActive,
    ProposalNotActive,
    ProposalWinnerSetEmpty,
    ProposalVotingWindowStillOpen,
    ProposalPrimaryTrackNotOpen,
    ProposalVoteAlreadyCast,
    ProposalVoteKindNotAllowedForPrimaryTrackFamily,
    ProposalSubmissionNotAllowedForSignedOrigin,
    ProposalSubmitterNotPrimaryEligible,
    ProposalPreimageMissing,
    ProposalPreimageOversized,
    ProposalPreimageInvalid,
    ProposalPreimageIncompatible,
    InsufficientProposalOpeningFeeBalance,
    ProposalVoteSetFull,
    ProposalProtectionTrackClosed,
    VotePowerCustodyTargetDecreased,
    VotePowerCustodyNotFound,
    VotePowerCustodyLockActive,
    ActiveProposalCapReached,
    ActiveProposalAuthorCapReached,
    ActiveProposalAuthorMissing,
    ActiveProposalCountUnderflow,
    ProposalMaturityBucketFull,
    PendingEnactmentBucketFull,
    FinalizedProposalOutcomeExpiryBucketFull,
    ZeroProposalVotingPeriod,
    EpochVoteCapReached,
    DuplicateWinningVoteItem,
    DuplicateWinningVoteResolutionItem,
    DuplicateWinningVoteAccount,
    WinningVoteItemSetFull,
    WinningVoteResolutionItemSetFull,
    ExpiryBucketFull,
    EpochArithmeticOverflow,
    ProposalMetadataMissing,
    FinalizedProposalMissing,
  }

  #[pallet::hooks]
  impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
      Self::service_current_epoch(T::EpochProvider::current_epoch())
    }

    fn integrity_test() {
      let required_recent_finalized_capacity = T::FinalizedProposalOutcomeRetentionEpochs::get()
        .saturating_mul(T::MaxFinalizedProposalOutcomesPerEpoch::get());
      assert!(
        T::MaxRecentFinalizedProposalsPerDomain::get() >= required_recent_finalized_capacity,
        "MaxRecentFinalizedProposalsPerDomain must cover the full retained finalized-outcome horizon"
      );
      assert!(
        T::StrategicProposalReserve::get() > 0
          && T::StrategicProposalReserve::get() < T::MaxActiveProposalsPerDomain::get(),
        "StrategicProposalReserve must reserve a nonzero strict subset of domain capacity"
      );
      assert!(
        T::MaxActiveProposalsPerAuthor::get() > 0
          && T::MaxActiveProposalsPerAuthor::get()
            < T::MaxActiveProposalsPerDomain::get()
              .saturating_sub(T::StrategicProposalReserve::get()),
        "MaxActiveProposalsPerAuthor must be a nonzero strict subset of general domain capacity"
      );
    }
  }

  #[pallet::call]
  impl<T: Config> Pallet<T> {
    #[pallet::call_index(0)]
    #[pallet::weight(T::WeightInfo::record_winning_vote())]
    pub fn record_winning_vote(
      origin: OriginFor<T>,
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      account: T::AccountId,
    ) -> DispatchResult {
      T::AdminOrigin::ensure_origin(origin)?;
      Self::ingest_winning_vote_resolution(domain, item_id, account)
    }

    #[pallet::call_index(1)]
    #[pallet::weight(T::WeightInfo::record_winning_vote_batch(accounts.len() as u32))]
    pub fn record_winning_vote_batch(
      origin: OriginFor<T>,
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      accounts: BoundedVec<T::AccountId, T::MaxWinningVoteAccountsPerCall>,
    ) -> DispatchResult {
      T::AdminOrigin::ensure_origin(origin)?;
      Self::ingest_winning_vote_resolution_batch(domain, item_id, accounts)
    }

    #[pallet::call_index(2)]
    #[pallet::weight(T::WeightInfo::submit_proposal())]
    #[transactional]
    pub fn submit_proposal(
      origin: OriginFor<T>,
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      proposer: T::AccountId,
      cadence_mode: ProposalCadenceMode,
      payload_kind: ProposalPayloadKind,
      payload_hash: T::Hash,
    ) -> DispatchResult {
      T::AdminOrigin::ensure_origin(origin)?;
      let admission = Self::classify_proposal_admission(
        domain,
        item_id,
        &proposer,
        payload_kind,
        &payload_hash,
        false,
      )?;
      Self::submit_active_proposal(
        domain,
        item_id,
        proposer,
        ProposalMetadata {
          cadence_mode,
          payload_kind,
          payload_hash,
        },
        admission,
      )
    }

    #[pallet::call_index(3)]
    #[pallet::weight(T::WeightInfo::submit_signed_proposal())]
    #[transactional]
    pub fn submit_signed_proposal(
      origin: OriginFor<T>,
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      cadence_mode: ProposalCadenceMode,
      payload_kind: ProposalPayloadKind,
      payload_hash: T::Hash,
    ) -> DispatchResult {
      let proposer = ensure_signed(origin)?;
      let admission = Self::classify_proposal_admission(
        domain,
        item_id,
        &proposer,
        payload_kind,
        &payload_hash,
        true,
      )?;
      let opening_fee = admission.opening_fee.unwrap_or_default();
      if !opening_fee.is_zero() {
        T::Currency::transfer(
          &proposer,
          &T::ProposalFeeRecipient::get(),
          opening_fee,
          ExistenceRequirement::KeepAlive,
        )
        .map_err(|_| Error::<T>::InsufficientProposalOpeningFeeBalance)?;
        Self::deposit_event(Event::ProposalOpeningFeeCollected {
          domain,
          item_id,
          proposer: proposer.clone(),
          amount: opening_fee,
        });
      }
      Self::submit_active_proposal(
        domain,
        item_id,
        proposer,
        ProposalMetadata {
          cadence_mode,
          payload_kind,
          payload_hash,
        },
        admission,
      )
    }

    #[pallet::call_index(4)]
    #[pallet::weight(T::WeightInfo::resolve_proposal(winners.len() as u32))]
    pub fn resolve_proposal(
      origin: OriginFor<T>,
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      winners: BoundedVec<T::AccountId, T::MaxWinningVoteAccountsPerCall>,
    ) -> DispatchResult {
      T::AdminOrigin::ensure_origin(origin)?;
      let winning_primary_option = ProposalVotesByItem::<T>::get(domain, item_id)
        .as_ref()
        .and_then(|votes| Self::infer_winning_primary_option_from_winners(votes, &winners));
      Self::resolve_active_proposal(domain, item_id, winners, winning_primary_option)
    }

    #[pallet::call_index(5)]
    #[pallet::weight(T::WeightInfo::reject_proposal())]
    pub fn reject_proposal(
      origin: OriginFor<T>,
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> DispatchResult {
      T::AdminOrigin::ensure_origin(origin)?;
      Self::reject_active_proposal(domain, item_id, ProposalRejectionReason::AdminRejected)
    }

    #[pallet::call_index(6)]
    #[pallet::weight(T::WeightInfo::cast_vote())]
    pub fn cast_vote(
      origin: OriginFor<T>,
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      vote: ProposalVoteKind,
    ) -> DispatchResult {
      let account = ensure_signed(origin)?;
      Self::cast_active_proposal_vote(domain, item_id, account, vote)
    }

    #[pallet::call_index(7)]
    #[pallet::weight(T::WeightInfo::resolve_proposal_from_votes(
      T::MaxWinningVoteAccountsPerCall::get()
    ))]
    pub fn resolve_proposal_from_votes(
      origin: OriginFor<T>,
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> DispatchResult {
      T::AdminOrigin::ensure_origin(origin)?;
      Self::resolve_active_proposal_from_votes(domain, item_id)
    }

    #[pallet::call_index(8)]
    #[pallet::weight(T::WeightInfo::requeue_proposal_for_auto_finalization())]
    pub fn requeue_proposal_for_auto_finalization(
      origin: OriginFor<T>,
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> DispatchResult {
      T::AdminOrigin::ensure_origin(origin)?;
      Self::requeue_active_proposal_for_auto_finalization(domain, item_id)
    }

    #[pallet::call_index(9)]
    #[pallet::weight(T::WeightInfo::force_resolve_proposal_from_votes(
      T::MaxWinningVoteAccountsPerCall::get()
    ))]
    pub fn force_resolve_proposal_from_votes(
      origin: OriginFor<T>,
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> DispatchResult {
      T::AdminOrigin::ensure_origin(origin)?;
      Self::resolve_active_proposal_from_votes_with_policy(domain, item_id, false)
    }

    #[pallet::call_index(10)]
    #[pallet::weight(T::WeightInfo::unlock_vote_power())]
    #[transactional]
    pub fn unlock_vote_power(origin: OriginFor<T>, lock_id: T::VotePowerLockId) -> DispatchResult {
      let account = ensure_signed(origin)?;
      let position = VotePowerCustodyByAccount::<T>::get(&account, lock_id)
        .ok_or(Error::<T>::VotePowerCustodyNotFound)?;
      ensure!(
        T::EpochProvider::current_epoch() >= position.lock_until,
        Error::<T>::VotePowerCustodyLockActive
      );
      T::VotePowerCustody::unlock(&account, lock_id, position.amount)?;
      VotePowerCustodyByAccount::<T>::remove(&account, lock_id);
      Self::deposit_event(Event::VotePowerCustodyReleased {
        account,
        lock_id,
        amount: position.amount,
      });
      Ok(())
    }
  }

  #[pallet::view_functions]
  impl<T: Config> Pallet<T> {
    pub fn governance_participation_coefficient(
      domain: T::DomainId,
      account: T::AccountId,
    ) -> FixedU128 {
      Self::do_governance_participation_coefficient(domain, &account)
    }

    pub fn govxp_counters(domain: T::DomainId, account: T::AccountId) -> GovXpCounters {
      Self::do_govxp_counters(domain, &account)
    }

    pub fn recent_finalized_proposals(
      domain: T::DomainId,
    ) -> BoundedVec<
      RecentFinalizedProposal<T::AccountId, T::DomainId, T::WinningVoteItemId, T::Hash, T::Epoch>,
      T::MaxRecentFinalizedProposalsPerDomain,
    > {
      Self::do_recent_finalized_proposals(domain)
    }

    pub fn governance_domain_policy(domain: T::DomainId) -> crate::GovernanceDomainPolicy {
      T::GovernanceDomainPolicyProvider::policy(domain)
    }

    pub fn proposal_vote_power_profile(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      vote: ProposalVoteKind,
    ) -> Option<crate::ProposalVotePowerProfile> {
      Self::do_proposal_vote_power_profile(domain, item_id, vote)
    }

    pub fn proposal_vote_tally(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<ProposalVoteTally> {
      Self::do_proposal_vote_tally(domain, item_id)
    }

    pub fn account_governance_power_view(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      account: T::AccountId,
    ) -> Option<AccountGovernancePowerView<T::Epoch>> {
      Self::do_account_governance_power_view(domain, item_id, account)
    }

    pub fn active_proposal_count(domain: T::DomainId) -> u32 {
      ActiveProposalIdsByDomain::<T>::decode_len(domain).unwrap_or(0) as u32
    }

    pub fn proposal_primary_track_tally(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<ProposalPrimaryTrackTally> {
      Self::do_proposal_primary_track_tally(domain, item_id)
    }

    pub fn retained_proposal_winning_primary_option(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<ProposalPrimaryTrackOption> {
      ProposalWinningPrimaryOptionByItem::<T>::get(domain, item_id)
    }

    pub fn proposal_status(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<ProposalStatus<T::Epoch>> {
      Self::do_proposal_status(domain, item_id)
    }

    pub fn proposal_execution_authority(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<ProposalExecutionAuthority> {
      Self::do_proposal_execution_authority(domain, item_id)
    }

    pub fn authorized_runtime_upgrade() -> Option<crate::AuthorizedRuntimeUpgrade<T::Hash>> {
      T::ProposalRuntimeUpgradeAuthorizationProvider::authorized_upgrade()
    }

    pub fn proposal_admission_policy_view(
      domain: T::DomainId,
      payload_kind: ProposalPayloadKind,
    ) -> crate::ProposalAdmissionPolicyView<BalanceOf<T>> {
      Self::proposal_admission_policy(domain, payload_kind)
    }

    pub fn proposal_payload_availability(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<PayloadHashPreimageStatus> {
      Self::do_proposal_payload_availability(domain, item_id)
    }

    pub fn payload_hash_preimage_status(payload_hash: T::Hash) -> PayloadHashPreimageStatus {
      PayloadHashPreimageStatus {
        have_preimage: T::ProposalPayloadPreimageProvider::have_preimage(&payload_hash),
        preimage_requested: T::ProposalPayloadPreimageProvider::preimage_requested(&payload_hash),
        payload_len: T::ProposalPayloadPreimageProvider::preimage_len(&payload_hash),
      }
    }

    pub fn payload_preimage_note_cost(payload_len: u32) -> Option<BalanceOf<T>> {
      T::ProposalPayloadPreimageNoteCostProvider::note_cost(payload_len)
    }

    pub fn proposal_timing(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<ProposalTiming<T::Epoch>> {
      Self::do_proposal_timing(domain, item_id)
    }

    pub fn proposal_primary_track_family(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<crate::ProposalPrimaryTrackFamily> {
      Self::do_proposal_primary_track_family(domain, item_id)
    }

    pub fn proposal_urgent_eligibility(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<bool> {
      Self::do_proposal_urgent_eligibility(domain, item_id)
    }

    pub fn finalized_proposal_outcome(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<FinalizedProposalOutcome<T::Epoch>> {
      FinalizedProposals::<T>::get(domain, item_id).map(|record| record.outcome)
    }

    pub fn proposal_execution_detail(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<crate::ProposalExecutionDetail<T::AccountId, T::DomainId, T::Hash>> {
      FinalizedProposals::<T>::get(domain, item_id)?.execution_detail
    }
  }
}
