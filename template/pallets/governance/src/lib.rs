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
  AdminOnly,
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

pub trait ProposalPayloadPreimageProvider<Hash> {
  fn have_preimage(hash: &Hash) -> bool;
  fn preimage_requested(hash: &Hash) -> bool;
  fn preimage_len(_hash: &Hash) -> Option<u32> {
    None
  }
}

#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum ProposalParameterChangeSurface {
  RouterFee,
  TrackedAsset,
}

#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
)]
pub enum ProposalTreasurySpendSettlementKind {
  DirectTransfer,
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
  UnsupportedTarget,
  UnsupportedPayloadKind,
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
pub enum ProposalExecutionDetail<AccountId, DomainId, Hash, Epoch> {
  Executed {
    payload_kind: ProposalPayloadKind,
    authority: ProposalExecutionAuthority,
    executed_epoch: Epoch,
    detail: ProposalExecutionSuccessDetail<AccountId, DomainId, Hash>,
  },
  ExecutionFailed {
    payload_kind: ProposalPayloadKind,
    authority: ProposalExecutionAuthority,
    failed_epoch: Epoch,
    reason: ProposalExecutionFailureReason,
  },
  AdvisoryFinalized {
    payload_kind: ProposalPayloadKind,
    finalized_epoch: Epoch,
  },
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

impl<Hash> ProposalPayloadPreimageProvider<Hash> for () {
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
  fn cast_vote() -> Weight;
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

  fn cast_vote() -> Weight {
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

const STORAGE_VERSION: StorageVersion = StorageVersion::new(11);

#[frame::pallet]
pub mod pallet {
  use crate::{
    EpochProvider as _, GovernanceDomainPolicyProvider as _, ProposalPayloadExecutor as _,
    ProposalPayloadPreimageNoteCostProvider as _, ProposalPayloadPreimageProvider as _,
    ProposalPrimaryTrackFamilyProvider as _, ProposalRuntimeUpgradeAuthorizationProvider as _,
    ProposalSubmissionAuthorityProvider as _, ProposalTrackPowerProfileProvider as _,
    ProposalUrgentPolicyProvider as _, ProposalVoteWeightProvider as _, STORAGE_VERSION,
    VetoVotePowerProvider as _, WeightInfo as _,
  };
  use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec::Vec,
  };
  use codec::{Decode, Encode};
  use frame::prelude::*;
  use polkadot_sdk::frame_support::{traits::Currency, transactional};
  use polkadot_sdk::sp_runtime::{
    FixedU128, Perbill,
    traits::{AtLeast32BitUnsigned, SaturatedConversion, Zero},
  };

  type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

  #[pallet::config]
  pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
    type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    type Currency: Currency<Self::AccountId>;
    #[pallet::constant]
    type ProposalOpeningFee: Get<BalanceOf<Self>>;
    type DomainId: Parameter + MaxEncodedLen + Member + Copy + Ord + TypeInfo;
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
    /// Approval ceiling for adaptive curves. When equal to `ProposalApprovalThreshold`,
    /// the curve degenerates to a flat threshold (current behavior). When greater,
    /// approval requirement starts at ceiling and decays to the floor over the voting window.
    #[pallet::constant]
    type ProposalApprovalCeiling: Get<Perbill>;
    #[pallet::constant]
    type ProposalVetoThreshold: Get<Perbill>;
    #[pallet::constant]
    type ProposalVetoMinimumVetoTurnout: Get<Perbill>;
    #[pallet::constant]
    type ProposalMinimumTurnout: Get<u64>;
    /// Turnout ceiling for adaptive curves. When equal to `ProposalMinimumTurnout`,
    /// the curve degenerates to a flat threshold (current behavior). When greater,
    /// turnout requirement starts at ceiling and decays to the floor over the voting window.
    #[pallet::constant]
    type ProposalTurnoutCeiling: Get<u64>;
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
    type ProposalRuntimeUpgradeAuthorizationProvider: crate::ProposalRuntimeUpgradeAuthorizationProvider<Self::Hash>;
    type ProposalPayloadPreimageNoteCostProvider: crate::ProposalPayloadPreimageNoteCostProvider<BalanceOf<Self>>;
    type VetoVotePowerProvider: crate::VetoVotePowerProvider<
        Self::AccountId,
        Self::DomainId,
        Self::WinningVoteItemId,
        Self::Epoch,
      >;
    type ProposalPayloadPreimageProvider: crate::ProposalPayloadPreimageProvider<Self::Hash>;
    type ProposalPayloadExecutor: crate::ProposalPayloadExecutor<
        Self::AccountId,
        Self::DomainId,
        Self::WinningVoteItemId,
        Self::Hash,
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
  pub enum ProposalStatus<Epoch> {
    Active(ProposalResolutionState<Epoch>),
    PendingEnactment {
      outcome: FinalizedProposalOutcome<Epoch>,
      enactment_epoch: Epoch,
    },
    Finalized(FinalizedProposalOutcome<Epoch>),
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub enum FinalizedProposalOutcome<Epoch> {
    Resolved {
      epoch: Epoch,
      winner_count: u32,
    },
    Rejected {
      epoch: Epoch,
      reason: ProposalRejectionReason,
    },
    VetoCancelled {
      epoch: Epoch,
      veto_weight: u64,
    },
    Enacted {
      approved_epoch: Epoch,
      executed_epoch: Epoch,
      winner_count: u32,
    },
    ExecutionFailed {
      approved_epoch: Epoch,
      failed_epoch: Epoch,
      winner_count: u32,
    },
    AdvisoryFinalized {
      approved_epoch: Epoch,
      finalized_epoch: Epoch,
      winner_count: u32,
    },
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct RecentFinalizedProposal<ItemId, Epoch> {
    pub item_id: ItemId,
    pub outcome: FinalizedProposalOutcome<Epoch>,
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
  pub struct ProposalPayloadAvailability {
    pub have_preimage: bool,
    pub preimage_requested: bool,
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
  struct VetoCancellation {
    veto_weight: u64,
    pass_weight: u64,
    mode: VetoCancellationMode,
  }

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  struct UrgentAuthorization {
    pass_weight: u64,
    total_protection_supply: u64,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct MaturingProposalTouch<DomainId, ItemId> {
    pub domain: DomainId,
    pub item_id: ItemId,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  pub struct FinalizedProposalTouch<DomainId, ItemId> {
    pub domain: DomainId,
    pub item_id: ItemId,
  }

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
  #[pallet::getter(fn active_proposal_count)]
  pub type ActiveProposalCounts<T: Config> =
    StorageMap<_, Blake2_128Concat, T::DomainId, u32, ValueQuery>;

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
  #[pallet::getter(fn finalized_proposal_outcome)]
  pub type FinalizedProposalOutcomes<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::DomainId,
    Blake2_128Concat,
    T::WinningVoteItemId,
    FinalizedProposalOutcome<T::Epoch>,
    OptionQuery,
  >;

  #[pallet::storage]
  pub type ProposalExecutionDetails<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::DomainId,
    Blake2_128Concat,
    T::WinningVoteItemId,
    crate::ProposalExecutionDetail<T::AccountId, T::DomainId, T::Hash, T::Epoch>,
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
    ProposalOpeningFeeBurned {
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
    InsufficientProposalOpeningFeeBalance,
    ProposalVoteSetFull,
    ProposalProtectionTrackClosed,
    ActiveProposalCapReached,
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
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  #[scale_info(skip_type_params(MaxProposalVotesPerDirection))]
  struct LegacyProposalVotesV1<AccountId, MaxProposalVotesPerDirection: Get<u32>> {
    ayes: BoundedVec<AccountId, MaxProposalVotesPerDirection>,
    nays: BoundedVec<AccountId, MaxProposalVotesPerDirection>,
    vetoes: BoundedVec<AccountId, MaxProposalVotesPerDirection>,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  #[scale_info(skip_type_params(MaxProposalVotesPerDirection))]
  struct LegacyProposalVotesV2<AccountId, MaxProposalVotesPerDirection: Get<u32>> {
    ayes: BoundedVec<AccountId, MaxProposalVotesPerDirection>,
    nays: BoundedVec<AccountId, MaxProposalVotesPerDirection>,
    vetoes: BoundedVec<AccountId, MaxProposalVotesPerDirection>,
    passes: BoundedVec<AccountId, MaxProposalVotesPerDirection>,
  }

  #[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo,
  )]
  #[scale_info(skip_type_params(MaxProposalVotesPerDirection))]
  struct LegacyProposalVotesV3<AccountId, Epoch, MaxProposalVotesPerDirection: Get<u32>> {
    ayes: BoundedVec<ProposalBallot<AccountId, Epoch>, MaxProposalVotesPerDirection>,
    nays: BoundedVec<ProposalBallot<AccountId, Epoch>, MaxProposalVotesPerDirection>,
    vetoes: BoundedVec<ProposalBallot<AccountId, Epoch>, MaxProposalVotesPerDirection>,
    passes: BoundedVec<ProposalBallot<AccountId, Epoch>, MaxProposalVotesPerDirection>,
  }

  #[pallet::hooks]
  impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
    fn on_runtime_upgrade() -> Weight {
      let on_chain_version = StorageVersion::get::<Pallet<T>>();
      if on_chain_version >= STORAGE_VERSION {
        return Weight::zero();
      }
      let migration_epoch = T::EpochProvider::current_epoch();
      let inflate_ballots =
        |accounts: BoundedVec<T::AccountId, T::MaxWinningVoteAccountsPerCall>| {
          accounts.into_iter().fold(
            BoundedVec::<
              ProposalBallot<T::AccountId, T::Epoch>,
              T::MaxWinningVoteAccountsPerCall,
            >::default(),
            |mut ballots, account| {
              let push_result = ballots.try_push(ProposalBallot {
                account,
                vote_epoch: migration_epoch,
              });
              if push_result.is_err() {
                panic!("migration ballots must preserve the bounded vote set")
              }
              ballots
            },
          )
        };
      let mut translated = 0u64;
      if on_chain_version == StorageVersion::new(2) {
        ProposalVotesByItem::<T>::translate::<
          LegacyProposalVotesV2<T::AccountId, T::MaxWinningVoteAccountsPerCall>,
          _,
        >(|_, _, legacy_votes| {
          translated = translated.saturating_add(1);
          Some(ProposalVotes {
            ayes: inflate_ballots(legacy_votes.ayes),
            nays: inflate_ballots(legacy_votes.nays),
            amplifies: BoundedVec::default(),
            approves: BoundedVec::default(),
            reduces: BoundedVec::default(),
            vetoes: inflate_ballots(legacy_votes.vetoes),
            passes: inflate_ballots(legacy_votes.passes),
          })
        });
      } else if on_chain_version < StorageVersion::new(3) {
        ProposalVotesByItem::<T>::translate::<
          LegacyProposalVotesV1<T::AccountId, T::MaxWinningVoteAccountsPerCall>,
          _,
        >(|_, _, legacy_votes| {
          translated = translated.saturating_add(1);
          Some(ProposalVotes {
            ayes: inflate_ballots(legacy_votes.ayes),
            nays: inflate_ballots(legacy_votes.nays),
            amplifies: BoundedVec::default(),
            approves: BoundedVec::default(),
            reduces: BoundedVec::default(),
            vetoes: inflate_ballots(legacy_votes.vetoes),
            passes: BoundedVec::default(),
          })
        });
      } else if on_chain_version < StorageVersion::new(10) {
        ProposalVotesByItem::<T>::translate::<
          LegacyProposalVotesV3<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
          _,
        >(|_, _, legacy_votes| {
          translated = translated.saturating_add(1);
          Some(ProposalVotes {
            ayes: legacy_votes.ayes,
            nays: legacy_votes.nays,
            amplifies: BoundedVec::default(),
            approves: BoundedVec::default(),
            reduces: BoundedVec::default(),
            vetoes: legacy_votes.vetoes,
            passes: legacy_votes.passes,
          })
        });
      }
      let mut migrated_active_proposals = 0u64;
      let mut active_proposal_index_writes = 0u64;
      if on_chain_version < StorageVersion::new(5) {
        let mut active_indexes = BTreeMap::<
          T::DomainId,
          BoundedVec<T::WinningVoteItemId, T::MaxActiveProposalsPerDomain>,
        >::new();
        for (domain, item_id, _) in ActiveProposals::<T>::iter() {
          migrated_active_proposals = migrated_active_proposals.saturating_add(1);
          let item_ids = active_indexes.entry(domain).or_default();
          if item_ids.iter().all(|existing| *existing != item_id) {
            let push_result = item_ids.try_push(item_id);
            if push_result.is_err() {
              panic!("migration active proposal index must fit configured domain cap")
            }
          }
        }
        for (domain, item_ids) in active_indexes {
          ActiveProposalIdsByDomain::<T>::insert(domain, item_ids);
          active_proposal_index_writes = active_proposal_index_writes.saturating_add(1);
        }
      }
      STORAGE_VERSION.put::<Pallet<T>>();
      T::DbWeight::get().reads_writes(
        translated
          .saturating_add(migrated_active_proposals)
          .saturating_add(1),
        translated
          .saturating_add(active_proposal_index_writes)
          .saturating_add(1),
      )
    }

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
      Self::submit_active_proposal(
        domain,
        item_id,
        proposer,
        ProposalMetadata {
          cadence_mode,
          payload_kind,
          payload_hash,
        },
      )
    }

    #[pallet::call_index(3)]
    #[pallet::weight(
      T::WeightInfo::submit_proposal().saturating_add(T::DbWeight::get().reads_writes(2, 2))
    )]
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
      ensure!(
        T::ProposalSubmissionAuthorityProvider::authority(domain, payload_kind)
          == crate::ProposalSubmissionAuthority::Signed,
        Error::<T>::ProposalSubmissionNotAllowedForSignedOrigin
      );
      let opening_fee = T::ProposalOpeningFee::get();
      if !opening_fee.is_zero() {
        let (_, remainder) = T::Currency::slash(&proposer, opening_fee);
        ensure!(
          remainder.is_zero(),
          Error::<T>::InsufficientProposalOpeningFeeBalance
        );
        Self::deposit_event(Event::ProposalOpeningFeeBurned {
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
  }

  impl<T: Config> Pallet<T> {
    #[transactional]
    pub fn submit_active_proposal(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      proposer: T::AccountId,
      metadata: ProposalMetadata<T::Hash>,
    ) -> DispatchResult {
      let current_epoch = T::EpochProvider::current_epoch();
      let maturity_epoch = Self::proposal_maturity_epoch(current_epoch)?;
      let active_count = ActiveProposalCounts::<T>::try_mutate(domain, |active_count| {
        ensure!(
          !ActiveProposals::<T>::contains_key(domain, item_id),
          Error::<T>::ProposalAlreadyActive
        );
        ensure!(
          *active_count < T::MaxActiveProposalsPerDomain::get(),
          Error::<T>::ActiveProposalCapReached
        );
        *active_count = active_count.saturating_add(1);
        Ok::<u32, DispatchError>(*active_count)
      })?;
      ActiveProposals::<T>::insert(
        domain,
        item_id,
        ActiveProposal {
          submitted_epoch: current_epoch,
        },
      );
      ProposalAuthorsByItem::<T>::insert(domain, item_id, proposer.clone());
      ProposalMetadataByItem::<T>::insert(domain, item_id, metadata.clone());
      Self::note_authored_proposal(domain, &proposer);
      Self::insert_active_proposal_id(domain, item_id)?;
      Self::schedule_proposal_maturity_at(maturity_epoch, domain, item_id)?;
      Self::deposit_event(Event::ProposalSubmitted {
        domain,
        item_id,
        proposer,
        cadence_mode: metadata.cadence_mode,
        payload_kind: metadata.payload_kind,
        payload_hash: metadata.payload_hash,
        epoch: current_epoch,
        active_count,
      });
      Ok(())
    }

    fn add_epochs(base_epoch: T::Epoch, delta: T::Epoch) -> Result<T::Epoch, DispatchError> {
      let result_epoch_u32 = base_epoch
        .saturated_into::<u32>()
        .checked_add(delta.saturated_into::<u32>())
        .ok_or(Error::<T>::EpochArithmeticOverflow)?;
      Ok(result_epoch_u32.saturated_into())
    }

    fn proposal_maturity_epoch(submitted_epoch: T::Epoch) -> Result<T::Epoch, DispatchError> {
      let voting_period = T::ProposalVotingPeriod::get().saturated_into::<u32>();
      ensure!(voting_period > 0, Error::<T>::ZeroProposalVotingPeriod);
      Self::proposal_ordinary_primary_close_epoch(submitted_epoch)
    }

    fn proposal_ordinary_primary_open_epoch(
      submitted_epoch: T::Epoch,
    ) -> Result<T::Epoch, DispatchError> {
      Self::add_epochs(submitted_epoch, T::ProposalLeadInPeriod::get())
    }

    fn proposal_ordinary_primary_close_epoch(
      submitted_epoch: T::Epoch,
    ) -> Result<T::Epoch, DispatchError> {
      let primary_open_epoch = Self::proposal_ordinary_primary_open_epoch(submitted_epoch)?;
      Self::add_epochs(primary_open_epoch, T::ProposalVotingPeriod::get())
    }

    fn proposal_effective_primary_open_epoch(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      submitted_epoch: T::Epoch,
    ) -> Result<T::Epoch, DispatchError> {
      Ok(
        ProposalUrgentAuthorizedAt::<T>::get(domain, item_id)
          .unwrap_or(Self::proposal_ordinary_primary_open_epoch(submitted_epoch)?),
      )
    }

    fn proposal_effective_primary_close_epoch(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      submitted_epoch: T::Epoch,
    ) -> Result<T::Epoch, DispatchError> {
      if let Some(urgent_primary_open_epoch) = ProposalUrgentAuthorizedAt::<T>::get(domain, item_id)
      {
        return Self::proposal_urgent_primary_close_epoch(urgent_primary_open_epoch);
      }
      Self::proposal_ordinary_primary_close_epoch(submitted_epoch)
    }

    fn proposal_protection_close_epoch(
      submitted_epoch: T::Epoch,
    ) -> Result<T::Epoch, DispatchError> {
      Self::add_epochs(submitted_epoch, T::ProposalProtectionPeriod::get())
    }

    fn proposal_urgent_primary_close_epoch(
      urgent_primary_open_epoch: T::Epoch,
    ) -> Result<T::Epoch, DispatchError> {
      Self::add_epochs(
        urgent_primary_open_epoch,
        T::ProposalUrgentVotingPeriod::get(),
      )
    }

    fn do_proposal_timing(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<ProposalTiming<T::Epoch>> {
      let proposal = ActiveProposals::<T>::get(domain, item_id)?;
      let submitted_epoch = proposal.submitted_epoch;
      let protection_open_epoch = submitted_epoch;
      let protection_close_epoch = Self::proposal_protection_close_epoch(submitted_epoch).ok()?;
      let ordinary_primary_open_epoch =
        Self::proposal_ordinary_primary_open_epoch(submitted_epoch).ok()?;
      let ordinary_primary_close_epoch =
        Self::proposal_ordinary_primary_close_epoch(submitted_epoch).ok()?;
      let urgent_primary_open_epoch = ProposalUrgentAuthorizedAt::<T>::get(domain, item_id);
      let urgent_primary_close_epoch = urgent_primary_open_epoch.and_then(|urgent_open_epoch| {
        Self::proposal_urgent_primary_close_epoch(urgent_open_epoch).ok()
      });
      let effective_primary_open_epoch =
        urgent_primary_open_epoch.unwrap_or(ordinary_primary_open_epoch);
      let effective_primary_close_epoch =
        urgent_primary_close_epoch.unwrap_or(ordinary_primary_close_epoch);
      let pending_enactment_epoch = ProposalPendingEnactmentAt::<T>::get(domain, item_id);
      Some(ProposalTiming {
        submitted_epoch,
        protection_open_epoch,
        protection_close_epoch,
        ordinary_primary_open_epoch,
        ordinary_primary_close_epoch,
        urgent_primary_open_epoch,
        urgent_primary_close_epoch,
        effective_primary_open_epoch,
        effective_primary_close_epoch,
        pending_enactment_epoch,
      })
    }

    fn proposal_execution_authority_for_payload_kind(
      payload_kind: ProposalPayloadKind,
    ) -> ProposalExecutionAuthority {
      match payload_kind {
        ProposalPayloadKind::L1RootAction => ProposalExecutionAuthority::Root,
        ProposalPayloadKind::L2TreasurySpend => ProposalExecutionAuthority::DomainTreasury,
        ProposalPayloadKind::L2ParameterChange => ProposalExecutionAuthority::DomainParameters,
        ProposalPayloadKind::Intent => ProposalExecutionAuthority::NonExecutable,
        ProposalPayloadKind::L2SignalToL1 => ProposalExecutionAuthority::NonExecutable,
      }
    }

    fn do_proposal_execution_authority(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<ProposalExecutionAuthority> {
      ProposalMetadataByItem::<T>::get(domain, item_id)
        .map(|metadata| Self::proposal_execution_authority_for_payload_kind(metadata.payload_kind))
    }

    fn do_proposal_payload_availability(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<ProposalPayloadAvailability> {
      let metadata = ProposalMetadataByItem::<T>::get(domain, item_id)?;
      Some(ProposalPayloadAvailability {
        have_preimage: T::ProposalPayloadPreimageProvider::have_preimage(&metadata.payload_hash),
        preimage_requested: T::ProposalPayloadPreimageProvider::preimage_requested(
          &metadata.payload_hash,
        ),
      })
    }

    fn do_proposal_primary_track_family(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<crate::ProposalPrimaryTrackFamily> {
      ProposalMetadataByItem::<T>::get(domain, item_id).map(|metadata| {
        T::ProposalPrimaryTrackFamilyProvider::family(domain, metadata.payload_kind)
      })
    }

    fn do_proposal_urgent_eligibility(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<bool> {
      ProposalMetadataByItem::<T>::get(domain, item_id).map(|metadata| {
        T::ProposalUrgentPolicyProvider::is_expeditable(domain, metadata.payload_kind)
      })
    }

    /// Voting-window progress clamped to `[0, Perbill::one()]`.
    fn voting_progress(
      current_epoch: T::Epoch,
      submitted_epoch: T::Epoch,
      maturity_epoch: T::Epoch,
    ) -> Perbill {
      let current = current_epoch.saturated_into::<u32>();
      let start = submitted_epoch.saturated_into::<u32>();
      let end = maturity_epoch.saturated_into::<u32>();
      let window = end.saturating_sub(start);
      if window == 0 {
        return Perbill::one();
      }
      let elapsed = current.saturating_sub(start).min(window);
      Perbill::from_rational(elapsed, window)
    }

    /// Adaptive approval threshold: decays linearly from ceiling to floor over the voting window.
    fn approval_threshold_at(
      current_epoch: T::Epoch,
      submitted_epoch: T::Epoch,
      maturity_epoch: T::Epoch,
    ) -> Perbill {
      let floor = T::ProposalApprovalThreshold::get();
      let ceiling = T::ProposalApprovalCeiling::get();
      if ceiling <= floor {
        return floor;
      }
      let progress = Self::voting_progress(current_epoch, submitted_epoch, maturity_epoch);
      let spread = ceiling.saturating_sub(floor);
      let decay = progress.mul_floor(spread.deconstruct());
      Perbill::from_parts(ceiling.deconstruct().saturating_sub(decay))
    }

    /// Adaptive turnout threshold: decays linearly from ceiling to floor over the voting window.
    fn turnout_threshold_at(
      current_epoch: T::Epoch,
      submitted_epoch: T::Epoch,
      maturity_epoch: T::Epoch,
    ) -> u64 {
      let floor = T::ProposalMinimumTurnout::get();
      let ceiling = T::ProposalTurnoutCeiling::get();
      if ceiling <= floor {
        return floor;
      }
      let progress = Self::voting_progress(current_epoch, submitted_epoch, maturity_epoch);
      let spread = ceiling.saturating_sub(floor);
      let decay = progress.mul_floor(spread);
      ceiling.saturating_sub(decay)
    }

    fn schedule_proposal_maturity_at(
      maturity_epoch: T::Epoch,
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> DispatchResult {
      ProposalMaturityBuckets::<T>::try_mutate(maturity_epoch, |bucket| -> DispatchResult {
        let exists = bucket
          .iter()
          .any(|entry| entry.domain == domain && entry.item_id == item_id);
        if !exists {
          bucket
            .try_push(MaturingProposalTouch { domain, item_id })
            .map_err(|_| Error::<T>::ProposalMaturityBucketFull)?;
        }
        Ok(())
      })
    }

    fn insert_active_proposal_id(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> DispatchResult {
      ActiveProposalIdsByDomain::<T>::try_mutate(domain, |item_ids| -> DispatchResult {
        if item_ids.iter().all(|existing| *existing != item_id) {
          item_ids
            .try_push(item_id)
            .map_err(|_| Error::<T>::ActiveProposalCapReached)?;
        }
        Ok(())
      })
    }

    fn remove_active_proposal_id(domain: T::DomainId, item_id: T::WinningVoteItemId) {
      ActiveProposalIdsByDomain::<T>::mutate(domain, |item_ids| {
        if let Some(position) = item_ids.iter().position(|existing| *existing == item_id) {
          item_ids.remove(position);
        }
      });
    }

    pub fn requeue_active_proposal_for_auto_finalization(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> DispatchResult {
      let proposal =
        ActiveProposals::<T>::get(domain, item_id).ok_or(Error::<T>::ProposalNotActive)?;
      let current_epoch = T::EpochProvider::current_epoch();
      let natural_maturity_epoch = Self::proposal_maturity_epoch(proposal.submitted_epoch)?;
      let target_epoch_u32 = current_epoch
        .saturated_into::<u32>()
        .saturating_add(1)
        .max(natural_maturity_epoch.saturated_into::<u32>());
      let maturity_epoch: T::Epoch = target_epoch_u32.saturated_into();
      Self::schedule_proposal_maturity_at(maturity_epoch, domain, item_id)?;
      Self::deposit_event(Event::ProposalAutoFinalizationRequeued {
        domain,
        item_id,
        epoch: current_epoch,
        maturity_epoch,
      });
      Ok(())
    }

    #[transactional]
    pub fn cast_active_proposal_vote(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      account: T::AccountId,
      vote: ProposalVoteKind,
    ) -> DispatchResult {
      let proposal =
        ActiveProposals::<T>::get(domain, item_id).ok_or(Error::<T>::ProposalNotActive)?;
      let current_epoch = T::EpochProvider::current_epoch();
      let protection_close_epoch = Self::proposal_protection_close_epoch(proposal.submitted_epoch)?;
      let primary_track_family = Self::do_proposal_primary_track_family(domain, item_id)
        .ok_or(Error::<T>::ProposalNotActive)?;
      let (
        aye_count,
        nay_count,
        veto_count,
        pass_count,
        replaced_vote,
        immediate_cancellation,
        urgent_authorization,
        urgent_executes_immediately,
        is_first_participation,
      ) = ProposalVotesByItem::<T>::try_mutate(
        domain,
        item_id,
        |maybe_votes| -> Result<
          (
            u32,
            u32,
            u32,
            u32,
            Option<ProposalVoteKind>,
            Option<VetoCancellation>,
            Option<UrgentAuthorization>,
            bool,
            bool,
          ),
          DispatchError,
        > {
          let mut votes = maybe_votes.take().unwrap_or(ProposalVotes {
            ayes: BoundedVec::default(),
            nays: BoundedVec::default(),
            amplifies: BoundedVec::default(),
            approves: BoundedVec::default(),
            reduces: BoundedVec::default(),
            vetoes: BoundedVec::default(),
            passes: BoundedVec::default(),
          });
          let ballot = ProposalBallot {
            account: account.clone(),
            vote_epoch: current_epoch,
          };
          let ordinary_track_vote_exists =
            Self::proposal_ballots_contain_account(&votes.ayes, &account)
              || Self::proposal_ballots_contain_account(&votes.nays, &account)
              || Self::proposal_ballots_contain_account(&votes.amplifies, &account)
              || Self::proposal_ballots_contain_account(&votes.approves, &account)
              || Self::proposal_ballots_contain_account(&votes.reduces, &account);
          let veto_track_vote_exists =
            Self::proposal_ballots_contain_account(&votes.vetoes, &account)
              || Self::proposal_ballots_contain_account(&votes.passes, &account);
          let is_first_participation = !ordinary_track_vote_exists && !veto_track_vote_exists;
          let mut replaced_vote = None;
          match vote {
            ProposalVoteKind::Aye => {
              ensure!(
                primary_track_family == crate::ProposalPrimaryTrackFamily::Binary,
                Error::<T>::ProposalVoteKindNotAllowedForPrimaryTrackFamily
              );
              ensure!(
                current_epoch.saturated_into::<u32>()
                  >= Self::proposal_effective_primary_open_epoch(
                    domain,
                    item_id,
                    proposal.submitted_epoch,
                  )?
                  .saturated_into::<u32>(),
                Error::<T>::ProposalPrimaryTrackNotOpen
              );
              ensure!(
                !ordinary_track_vote_exists,
                Error::<T>::ProposalVoteAlreadyCast
              );
              votes
                .ayes
                .try_push(ballot)
                .map_err(|_| Error::<T>::ProposalVoteSetFull)?;
            }
            ProposalVoteKind::Nay => {
              ensure!(
                current_epoch.saturated_into::<u32>()
                  >= Self::proposal_effective_primary_open_epoch(
                    domain,
                    item_id,
                    proposal.submitted_epoch,
                  )?
                  .saturated_into::<u32>(),
                Error::<T>::ProposalPrimaryTrackNotOpen
              );
              ensure!(
                !ordinary_track_vote_exists,
                Error::<T>::ProposalVoteAlreadyCast
              );
              votes
                .nays
                .try_push(ballot)
                .map_err(|_| Error::<T>::ProposalVoteSetFull)?;
            }
            ProposalVoteKind::Amplify => {
              ensure!(
                primary_track_family == crate::ProposalPrimaryTrackFamily::Invoice,
                Error::<T>::ProposalVoteKindNotAllowedForPrimaryTrackFamily
              );
              ensure!(
                current_epoch.saturated_into::<u32>()
                  >= Self::proposal_effective_primary_open_epoch(
                    domain,
                    item_id,
                    proposal.submitted_epoch,
                  )?
                  .saturated_into::<u32>(),
                Error::<T>::ProposalPrimaryTrackNotOpen
              );
              ensure!(
                !ordinary_track_vote_exists,
                Error::<T>::ProposalVoteAlreadyCast
              );
              votes
                .amplifies
                .try_push(ballot)
                .map_err(|_| Error::<T>::ProposalVoteSetFull)?;
            }
            ProposalVoteKind::Approve => {
              ensure!(
                primary_track_family == crate::ProposalPrimaryTrackFamily::Invoice,
                Error::<T>::ProposalVoteKindNotAllowedForPrimaryTrackFamily
              );
              ensure!(
                current_epoch.saturated_into::<u32>()
                  >= Self::proposal_effective_primary_open_epoch(
                    domain,
                    item_id,
                    proposal.submitted_epoch,
                  )?
                  .saturated_into::<u32>(),
                Error::<T>::ProposalPrimaryTrackNotOpen
              );
              ensure!(
                !ordinary_track_vote_exists,
                Error::<T>::ProposalVoteAlreadyCast
              );
              votes
                .approves
                .try_push(ballot)
                .map_err(|_| Error::<T>::ProposalVoteSetFull)?;
            }
            ProposalVoteKind::Reduce => {
              ensure!(
                primary_track_family == crate::ProposalPrimaryTrackFamily::Invoice,
                Error::<T>::ProposalVoteKindNotAllowedForPrimaryTrackFamily
              );
              ensure!(
                current_epoch.saturated_into::<u32>()
                  >= Self::proposal_effective_primary_open_epoch(
                    domain,
                    item_id,
                    proposal.submitted_epoch,
                  )?
                  .saturated_into::<u32>(),
                Error::<T>::ProposalPrimaryTrackNotOpen
              );
              ensure!(
                !ordinary_track_vote_exists,
                Error::<T>::ProposalVoteAlreadyCast
              );
              votes
                .reduces
                .try_push(ballot)
                .map_err(|_| Error::<T>::ProposalVoteSetFull)?;
            }
            ProposalVoteKind::Veto => {
              ensure!(
                !Self::proposal_protection_track_is_closed(current_epoch, protection_close_epoch,),
                Error::<T>::ProposalProtectionTrackClosed
              );
              if let Some(position) = votes
                .passes
                .iter()
                .position(|existing| existing.account == account)
              {
                votes.passes.remove(position);
                replaced_vote = Some(ProposalVoteKind::Pass);
              } else {
                ensure!(
                  !votes
                    .vetoes
                    .iter()
                    .any(|existing| existing.account == account),
                  Error::<T>::ProposalVoteAlreadyCast
                );
              }
              votes
                .vetoes
                .try_push(ballot)
                .map_err(|_| Error::<T>::ProposalVoteSetFull)?;
            }
            ProposalVoteKind::Pass => {
              ensure!(
                !Self::proposal_protection_track_is_closed(current_epoch, protection_close_epoch,),
                Error::<T>::ProposalProtectionTrackClosed
              );
              if let Some(position) = votes
                .vetoes
                .iter()
                .position(|existing| existing.account == account)
              {
                votes.vetoes.remove(position);
                replaced_vote = Some(ProposalVoteKind::Veto);
              } else {
                ensure!(
                  !votes
                    .passes
                    .iter()
                    .any(|existing| existing.account == account),
                  Error::<T>::ProposalVoteAlreadyCast
                );
              }
              votes
                .passes
                .try_push(ballot)
                .map_err(|_| Error::<T>::ProposalVoteSetFull)?;
            }
          }
          let aye_count = votes.ayes.len() as u32;
          let nay_count = votes.nays.len() as u32;
          let veto_count = votes.vetoes.len() as u32;
          let pass_count = votes.passes.len() as u32;
          let immediate_cancellation =
            Self::current_veto_cancellation(domain, item_id, &votes, false);
          let urgent_authorization = if immediate_cancellation.is_none() {
            Self::current_urgent_authorization(domain, item_id, &votes)
          } else {
            None
          };
          let urgent_executes_immediately = if urgent_authorization.is_some() {
            Self::urgent_fast_track_executes_immediately(domain, item_id, &votes)
          } else {
            false
          };
          *maybe_votes = Some(votes);
          Ok((
            aye_count,
            nay_count,
            veto_count,
            pass_count,
            replaced_vote,
            immediate_cancellation,
            urgent_authorization,
            urgent_executes_immediately,
            is_first_participation,
          ))
        },
      )?;
      if is_first_participation {
        Self::note_total_participation(domain, &account);
      }
      Self::deposit_event(Event::ProposalVoteCast {
        domain,
        item_id,
        account,
        vote,
        replaced_vote,
        vote_epoch: current_epoch,
        aye_count,
        nay_count,
        veto_count,
        pass_count,
      });
      if let Some(cancellation) = immediate_cancellation {
        return Self::veto_cancel_active_proposal(domain, item_id, cancellation);
      }
      if let Some(urgent_authorization) = urgent_authorization {
        Self::authorize_urgent_fast_track(domain, item_id, current_epoch, urgent_authorization);
        if urgent_executes_immediately {
          return Self::resolve_active_proposal_without_winners(domain, item_id);
        }
      }
      // Reset confirm timer if a new vote changes the passing state during confirm
      if T::ProposalConfirmPeriod::get().saturated_into::<u32>() > 0
        && ProposalConfirmStartedAt::<T>::contains_key(domain, item_id)
      {
        match Self::proposal_resolution_state(domain, item_id) {
          Some(ProposalResolutionState::Confirming { .. }) => {
            // Still passing — keep confirm timer running
          }
          _ => {
            // No longer passing — reset confirm, re-schedule maturity
            let reset_epoch = T::EpochProvider::current_epoch();
            ProposalConfirmStartedAt::<T>::remove(domain, item_id);
            let next_epoch_u32 = reset_epoch.saturated_into::<u32>().saturating_add(1);
            let _ =
              Self::schedule_proposal_maturity_at(next_epoch_u32.saturated_into(), domain, item_id);
            Self::deposit_event(Event::ProposalConfirmReset {
              domain,
              item_id,
              epoch: reset_epoch,
            });
          }
        }
      }
      Ok(())
    }

    fn proposal_vote_context(
      item_id: T::WinningVoteItemId,
      submitted_epoch: T::Epoch,
      current_epoch: T::Epoch,
      maturity_epoch: T::Epoch,
      vote_epoch: T::Epoch,
    ) -> crate::ProposalVoteContext<T::WinningVoteItemId, T::Epoch> {
      crate::ProposalVoteContext {
        item_id,
        current_epoch,
        submitted_epoch,
        maturity_epoch,
        vote_epoch,
      }
    }

    fn proposal_ordinary_weighting_window(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<(T::Epoch, T::Epoch, T::Epoch)> {
      let proposal = ActiveProposals::<T>::get(domain, item_id)?;
      let current_epoch = T::EpochProvider::current_epoch();
      let primary_open_epoch =
        Self::proposal_effective_primary_open_epoch(domain, item_id, proposal.submitted_epoch)
          .ok()?;
      let primary_close_epoch =
        Self::proposal_effective_primary_close_epoch(domain, item_id, proposal.submitted_epoch)
          .ok()?;
      Some((current_epoch, primary_open_epoch, primary_close_epoch))
    }

    fn proposal_protection_weighting_window(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<(T::Epoch, T::Epoch, T::Epoch)> {
      let proposal = ActiveProposals::<T>::get(domain, item_id)?;
      let current_epoch = T::EpochProvider::current_epoch();
      let protection_close_epoch =
        Self::proposal_protection_close_epoch(proposal.submitted_epoch).ok()?;
      Some((
        current_epoch,
        proposal.submitted_epoch,
        protection_close_epoch,
      ))
    }

    fn proposal_vote_weight_sum(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      current_epoch: T::Epoch,
      submitted_epoch: T::Epoch,
      maturity_epoch: T::Epoch,
      ballots: &BoundedVec<
        ProposalBallot<T::AccountId, T::Epoch>,
        T::MaxWinningVoteAccountsPerCall,
      >,
    ) -> u64 {
      ballots.iter().fold(0u64, |sum, ballot| {
        let context = Self::proposal_vote_context(
          item_id,
          submitted_epoch,
          current_epoch,
          maturity_epoch,
          ballot.vote_epoch,
        );
        sum.saturating_add(u64::from(T::ProposalVoteWeightProvider::vote_weight(
          domain,
          &context,
          &ballot.account,
        )))
      })
    }

    fn proposal_veto_weight_sum(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      current_epoch: T::Epoch,
      submitted_epoch: T::Epoch,
      maturity_epoch: T::Epoch,
      ballots: &BoundedVec<
        ProposalBallot<T::AccountId, T::Epoch>,
        T::MaxWinningVoteAccountsPerCall,
      >,
    ) -> u64 {
      ballots.iter().fold(0u64, |sum, ballot| {
        let context = Self::proposal_vote_context(
          item_id,
          submitted_epoch,
          current_epoch,
          maturity_epoch,
          ballot.vote_epoch,
        );
        sum.saturating_add(T::VetoVotePowerProvider::vote_weight(
          domain,
          &context,
          &ballot.account,
        ))
      })
    }

    fn proposal_raw_protection_weight_sum(
      domain: T::DomainId,
      ballots: &BoundedVec<
        ProposalBallot<T::AccountId, T::Epoch>,
        T::MaxWinningVoteAccountsPerCall,
      >,
    ) -> u64 {
      ballots.iter().fold(0u64, |sum, ballot| {
        sum.saturating_add(T::VetoVotePowerProvider::raw_vote_weight(
          domain,
          &ballot.account,
        ))
      })
    }

    fn proposal_raw_veto_weight_sum(
      domain: T::DomainId,
      ballots: &BoundedVec<
        ProposalBallot<T::AccountId, T::Epoch>,
        T::MaxWinningVoteAccountsPerCall,
      >,
    ) -> u64 {
      Self::proposal_raw_protection_weight_sum(domain, ballots)
    }

    fn proposal_veto_track_weights(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      current_epoch: T::Epoch,
      submitted_epoch: T::Epoch,
      maturity_epoch: T::Epoch,
      votes: &ProposalVotes<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
    ) -> (u64, u64) {
      (
        Self::proposal_veto_weight_sum(
          domain,
          item_id,
          current_epoch,
          submitted_epoch,
          maturity_epoch,
          &votes.vetoes,
        ),
        Self::proposal_veto_weight_sum(
          domain,
          item_id,
          current_epoch,
          submitted_epoch,
          maturity_epoch,
          &votes.passes,
        ),
      )
    }

    fn proposal_protection_track_is_closed(
      current_epoch: T::Epoch,
      protection_close_epoch: T::Epoch,
    ) -> bool {
      current_epoch.saturated_into::<u32>() >= protection_close_epoch.saturated_into::<u32>()
    }

    fn veto_weight_strictly_exceeds_threshold(veto_weight: u64, total_veto_issuance: u64) -> bool {
      if veto_weight == 0 || total_veto_issuance == 0 {
        return false;
      }
      let threshold_parts = u128::from(T::ProposalVetoThreshold::get().deconstruct());
      let veto_parts = u128::from(veto_weight).saturating_mul(1_000_000_000u128);
      let threshold_weight = u128::from(total_veto_issuance).saturating_mul(threshold_parts);
      veto_parts > threshold_weight
    }

    fn veto_weight_meets_minimum_turnout(veto_weight: u64, total_veto_issuance: u64) -> bool {
      if veto_weight == 0 || total_veto_issuance == 0 {
        return false;
      }
      let threshold_parts = u128::from(T::ProposalVetoMinimumVetoTurnout::get().deconstruct());
      if threshold_parts == 0 {
        return true;
      }
      let veto_parts = u128::from(veto_weight).saturating_mul(1_000_000_000u128);
      let threshold_weight = u128::from(total_veto_issuance).saturating_mul(threshold_parts);
      veto_parts >= threshold_weight
    }

    fn pass_weight_meets_fast_track_threshold(
      pass_weight: u64,
      total_protection_supply: u64,
    ) -> bool {
      if pass_weight == 0 || total_protection_supply == 0 {
        return false;
      }
      let threshold = T::ProposalFastTrackPassThreshold::get();
      if threshold == Perbill::one() {
        return pass_weight == total_protection_supply;
      }
      let threshold_parts = u128::from(threshold.deconstruct());
      let pass_parts = u128::from(pass_weight).saturating_mul(1_000_000_000u128);
      let threshold_weight = u128::from(total_protection_supply).saturating_mul(threshold_parts);
      pass_parts > threshold_weight
    }

    fn current_veto_cancellation(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      votes: &ProposalVotes<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
      allow_track_outcome: bool,
    ) -> Option<VetoCancellation> {
      let raw_veto_weight = Self::proposal_raw_veto_weight_sum(domain, &votes.vetoes);
      let total_veto_issuance = T::VetoVotePowerProvider::total_issuance(domain);
      let (current_epoch, protection_open_epoch, protection_close_epoch) =
        Self::proposal_protection_weighting_window(domain, item_id)?;
      let (veto_weight, pass_weight) = Self::proposal_veto_track_weights(
        domain,
        item_id,
        current_epoch,
        protection_open_epoch,
        protection_close_epoch,
        votes,
      );
      if Self::veto_weight_strictly_exceeds_threshold(raw_veto_weight, total_veto_issuance) {
        return Some(VetoCancellation {
          veto_weight,
          pass_weight,
          mode: VetoCancellationMode::ImmediateThreshold,
        });
      }
      if allow_track_outcome
        && Self::veto_weight_meets_minimum_turnout(raw_veto_weight, total_veto_issuance)
        && veto_weight >= pass_weight
      {
        return Some(VetoCancellation {
          veto_weight,
          pass_weight,
          mode: VetoCancellationMode::TrackOutcome,
        });
      }
      None
    }

    fn proposal_is_urgent_authorized(domain: T::DomainId, item_id: T::WinningVoteItemId) -> bool {
      ProposalUrgentAuthorizedAt::<T>::contains_key(domain, item_id)
    }

    fn current_urgent_authorization(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      votes: &ProposalVotes<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
    ) -> Option<UrgentAuthorization> {
      if Self::proposal_is_urgent_authorized(domain, item_id) {
        return None;
      }
      let metadata = ProposalMetadataByItem::<T>::get(domain, item_id)?;
      if !T::ProposalUrgentPolicyProvider::is_expeditable(domain, metadata.payload_kind) {
        return None;
      }
      let pass_weight = Self::proposal_raw_protection_weight_sum(domain, &votes.passes);
      let total_protection_supply = T::VetoVotePowerProvider::total_issuance(domain);
      if !Self::pass_weight_meets_fast_track_threshold(pass_weight, total_protection_supply) {
        return None;
      }
      Some(UrgentAuthorization {
        pass_weight,
        total_protection_supply,
      })
    }

    fn urgent_fast_track_executes_immediately(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      votes: &ProposalVotes<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
    ) -> bool {
      let Some(metadata) = ProposalMetadataByItem::<T>::get(domain, item_id) else {
        return false;
      };
      if !T::ProposalUrgentPolicyProvider::executes_immediately_on_unanimous_pass(
        domain,
        metadata.payload_kind,
      ) {
        return false;
      }
      let pass_weight = Self::proposal_raw_protection_weight_sum(domain, &votes.passes);
      let total_protection_supply = T::VetoVotePowerProvider::total_issuance(domain);
      total_protection_supply > 0 && pass_weight == total_protection_supply
    }

    fn authorize_urgent_fast_track(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      authorization_epoch: T::Epoch,
      urgent_authorization: UrgentAuthorization,
    ) {
      ProposalUrgentAuthorizedAt::<T>::insert(domain, item_id, authorization_epoch);
      Self::deposit_event(Event::ProposalUrgentAuthorized {
        domain,
        item_id,
        authorization_epoch,
        pass_weight: urgent_authorization.pass_weight,
        total_protection_supply: urgent_authorization.total_protection_supply,
      });
    }

    fn proposal_has_any_votes(
      votes: &ProposalVotes<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
    ) -> bool {
      !votes.ayes.is_empty()
        || !votes.nays.is_empty()
        || !votes.amplifies.is_empty()
        || !votes.approves.is_empty()
        || !votes.reduces.is_empty()
        || !votes.vetoes.is_empty()
        || !votes.passes.is_empty()
    }

    fn proposal_ballots_contain_account(
      ballots: &BoundedVec<
        ProposalBallot<T::AccountId, T::Epoch>,
        T::MaxWinningVoteAccountsPerCall,
      >,
      account: &T::AccountId,
    ) -> bool {
      for ballot in ballots {
        if ballot.account == *account {
          return true;
        }
      }
      false
    }

    fn collect_unique_accounts<I>(accounts: I) -> Vec<T::AccountId>
    where
      I: IntoIterator<Item = T::AccountId>,
    {
      let mut seen_accounts = BTreeSet::new();
      let mut unique_accounts = Vec::new();
      for account in accounts {
        if seen_accounts.insert(account.encode()) {
          unique_accounts.push(account);
        }
      }
      unique_accounts
    }

    fn note_winning_participation_batch(
      domain: T::DomainId,
      accounts: impl IntoIterator<Item = T::AccountId>,
    ) {
      for account in Self::collect_unique_accounts(accounts) {
        Self::note_winning_participation(domain, &account);
      }
    }

    fn note_pass_winning_participation(
      domain: T::DomainId,
      votes: &ProposalVotes<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
    ) {
      for ballot in &votes.passes {
        Self::note_winning_participation(domain, &ballot.account);
      }
    }

    fn infer_winning_primary_option_from_winners(
      votes: &ProposalVotes<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
      winners: &BoundedVec<T::AccountId, T::MaxWinningVoteAccountsPerCall>,
    ) -> Option<ProposalPrimaryTrackOption> {
      if winners.is_empty() {
        return None;
      }
      let winners_match = |ballots: &BoundedVec<
        ProposalBallot<T::AccountId, T::Epoch>,
        T::MaxWinningVoteAccountsPerCall,
      >| {
        for winner in winners {
          if !Self::proposal_ballots_contain_account(ballots, winner) {
            return false;
          }
        }
        true
      };
      if winners_match(&votes.ayes) {
        return Some(ProposalPrimaryTrackOption::Aye);
      }
      if winners_match(&votes.amplifies) {
        return Some(ProposalPrimaryTrackOption::Amplify);
      }
      if winners_match(&votes.approves) {
        return Some(ProposalPrimaryTrackOption::Approve);
      }
      if winners_match(&votes.reduces) {
        return Some(ProposalPrimaryTrackOption::Reduce);
      }
      if winners_match(&votes.nays) {
        return Some(ProposalPrimaryTrackOption::Nay);
      }
      None
    }

    fn record_finalized_proposal_outcome(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      outcome: FinalizedProposalOutcome<T::Epoch>,
      current_epoch: T::Epoch,
    ) -> DispatchResult {
      FinalizedProposalOutcomes::<T>::insert(domain, item_id, outcome);
      let retention = T::FinalizedProposalOutcomeRetentionEpochs::get();
      let expiry_epoch_u32 = current_epoch
        .saturated_into::<u32>()
        .checked_add(retention)
        .ok_or(Error::<T>::EpochArithmeticOverflow)?;
      let expiry_epoch: T::Epoch = expiry_epoch_u32.saturated_into();
      FinalizedProposalOutcomeExpiryBuckets::<T>::try_mutate(
        expiry_epoch,
        |bucket| -> DispatchResult {
          let exists = bucket
            .iter()
            .any(|entry| entry.domain == domain && entry.item_id == item_id);
          if !exists {
            bucket
              .try_push(FinalizedProposalTouch { domain, item_id })
              .map_err(|_| Error::<T>::FinalizedProposalOutcomeExpiryBucketFull)?;
          }
          Ok(())
        },
      )
    }

    fn schedule_pending_enactment_if_needed(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      finalized_epoch: T::Epoch,
    ) -> Result<bool, DispatchError> {
      let enactment_delay = T::ProposalEnactmentDelay::get();
      if enactment_delay.saturated_into::<u32>() == 0 {
        ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
        return Ok(false);
      }
      let enactment_epoch = Self::add_epochs(finalized_epoch, enactment_delay)?;
      let touch = FinalizedProposalTouch { domain, item_id };
      ProposalPendingEnactmentAt::<T>::insert(domain, item_id, enactment_epoch);
      PendingEnactmentBuckets::<T>::try_mutate(enactment_epoch, |bucket| -> DispatchResult {
        if !bucket.contains(&touch) {
          bucket
            .try_push(touch.clone())
            .map_err(|_| Error::<T>::PendingEnactmentBucketFull)?;
        }
        Ok(())
      })?;
      Self::deposit_event(Event::ProposalEnactmentScheduled {
        domain,
        item_id,
        finalized_epoch,
        enactment_epoch,
      });
      Ok(true)
    }

    fn set_finalized_proposal_outcome(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      outcome: FinalizedProposalOutcome<T::Epoch>,
    ) {
      FinalizedProposalOutcomes::<T>::insert(domain, item_id, outcome);
    }

    fn set_proposal_execution_detail(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      detail: crate::ProposalExecutionDetail<T::AccountId, T::DomainId, T::Hash, T::Epoch>,
    ) {
      ProposalExecutionDetails::<T>::insert(domain, item_id, detail);
    }

    fn maybe_execute_proposal_payload(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      approved_epoch: T::Epoch,
      winner_count: u32,
      execution_epoch: T::Epoch,
    ) -> DispatchResult {
      let metadata = ProposalMetadataByItem::<T>::get(domain, item_id)
        .ok_or(Error::<T>::ProposalMetadataMissing)?;
      match metadata.payload_kind {
        ProposalPayloadKind::Intent | ProposalPayloadKind::L2SignalToL1 => {
          Self::set_finalized_proposal_outcome(
            domain,
            item_id,
            FinalizedProposalOutcome::AdvisoryFinalized {
              approved_epoch,
              finalized_epoch: execution_epoch,
              winner_count,
            },
          );
          ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
          Self::set_proposal_execution_detail(
            domain,
            item_id,
            crate::ProposalExecutionDetail::AdvisoryFinalized {
              payload_kind: metadata.payload_kind,
              finalized_epoch: execution_epoch,
            },
          );
          Self::deposit_event(Event::ProposalAdvisoryFinalized {
            domain,
            item_id,
            approved_epoch,
            finalized_epoch: execution_epoch,
            payload_kind: metadata.payload_kind,
          });
          Ok(())
        }
        _ => {
          if !T::ProposalPayloadExecutor::can_execute(metadata.payload_kind) {
            return Ok(());
          }
          let authority =
            Self::proposal_execution_authority_for_payload_kind(metadata.payload_kind);
          if !T::ProposalPayloadPreimageProvider::have_preimage(&metadata.payload_hash) {
            Self::set_finalized_proposal_outcome(
              domain,
              item_id,
              FinalizedProposalOutcome::ExecutionFailed {
                approved_epoch,
                failed_epoch: execution_epoch,
                winner_count,
              },
            );
            ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
            Self::set_proposal_execution_detail(
              domain,
              item_id,
              crate::ProposalExecutionDetail::ExecutionFailed {
                payload_kind: metadata.payload_kind,
                authority,
                failed_epoch: execution_epoch,
                reason: crate::ProposalExecutionFailureReason::MissingPreimage,
              },
            );
            Self::deposit_event(Event::ProposalExecutionFailed {
              domain,
              item_id,
              approved_epoch,
              failed_epoch: execution_epoch,
              authority,
              payload_kind: metadata.payload_kind,
              reason: crate::ProposalExecutionFailureReason::MissingPreimage,
            });
            return Ok(());
          }
          match T::ProposalPayloadExecutor::execute(
            domain,
            item_id,
            metadata.payload_kind,
            metadata.payload_hash,
          ) {
            Ok(receipt) => {
              Self::set_finalized_proposal_outcome(
                domain,
                item_id,
                FinalizedProposalOutcome::Enacted {
                  approved_epoch,
                  executed_epoch: execution_epoch,
                  winner_count,
                },
              );
              ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
              let execution_detail = match receipt {
                crate::ProposalExecutionReceipt::Generic => {
                  crate::ProposalExecutionSuccessDetail::Generic
                }
                crate::ProposalExecutionReceipt::RuntimeUpgradeAuthorized { code_hash } => {
                  Self::deposit_event(Event::ProposalRuntimeUpgradeAuthorized {
                    domain,
                    item_id,
                    approved_epoch,
                    executed_epoch: execution_epoch,
                    code_hash,
                  });
                  crate::ProposalExecutionSuccessDetail::RuntimeUpgradeAuthorized { code_hash }
                }
                crate::ProposalExecutionReceipt::ParameterChangeExecuted { surface } => {
                  Self::deposit_event(Event::ProposalParameterChangeExecuted {
                    domain,
                    item_id,
                    approved_epoch,
                    executed_epoch: execution_epoch,
                    surface,
                  });
                  crate::ProposalExecutionSuccessDetail::ParameterChangeExecuted { surface }
                }
                crate::ProposalExecutionReceipt::TreasurySpendExecuted {
                  funding_source,
                  beneficiary,
                  payout_asset,
                  base_amount,
                  scalar,
                  final_amount,
                  settlement_kind,
                } => {
                  Self::deposit_event(Event::ProposalTreasurySpendExecuted {
                    domain,
                    item_id,
                    approved_epoch,
                    executed_epoch: execution_epoch,
                    funding_source: funding_source.clone(),
                    beneficiary: beneficiary.clone(),
                    payout_asset,
                    base_amount,
                    scalar,
                    final_amount,
                    settlement_kind,
                  });
                  crate::ProposalExecutionSuccessDetail::TreasurySpendExecuted {
                    funding_source,
                    beneficiary,
                    payout_asset,
                    base_amount,
                    scalar,
                    final_amount,
                    settlement_kind,
                  }
                }
              };
              Self::set_proposal_execution_detail(
                domain,
                item_id,
                crate::ProposalExecutionDetail::Executed {
                  payload_kind: metadata.payload_kind,
                  authority,
                  executed_epoch: execution_epoch,
                  detail: execution_detail,
                },
              );
              Self::deposit_event(Event::ProposalExecuted {
                domain,
                item_id,
                approved_epoch,
                executed_epoch: execution_epoch,
                authority,
                payload_kind: metadata.payload_kind,
              });
              Ok(())
            }
            Err(reason) => {
              Self::set_finalized_proposal_outcome(
                domain,
                item_id,
                FinalizedProposalOutcome::ExecutionFailed {
                  approved_epoch,
                  failed_epoch: execution_epoch,
                  winner_count,
                },
              );
              ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
              Self::set_proposal_execution_detail(
                domain,
                item_id,
                crate::ProposalExecutionDetail::ExecutionFailed {
                  payload_kind: metadata.payload_kind,
                  authority,
                  failed_epoch: execution_epoch,
                  reason,
                },
              );
              Self::deposit_event(Event::ProposalExecutionFailed {
                domain,
                item_id,
                approved_epoch,
                failed_epoch: execution_epoch,
                authority,
                payload_kind: metadata.payload_kind,
                reason,
              });
              Ok(())
            }
          }
        }
      }
    }

    fn resolve_or_reject_from_current_votes(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      votes: ProposalVotes<T::AccountId, T::Epoch, T::MaxWinningVoteAccountsPerCall>,
    ) -> DispatchResult {
      let urgent_authorized = Self::proposal_is_urgent_authorized(domain, item_id);
      if let Some(cancellation) =
        Self::current_veto_cancellation(domain, item_id, &votes, !urgent_authorized)
      {
        return Self::veto_cancel_active_proposal(domain, item_id, cancellation);
      }
      let (current_epoch, primary_open_epoch, primary_close_epoch) =
        Self::proposal_ordinary_weighting_window(domain, item_id)
          .ok_or(Error::<T>::ProposalNotActive)?;
      let (protection_current_epoch, protection_open_epoch, protection_close_epoch) =
        Self::proposal_protection_weighting_window(domain, item_id)
          .ok_or(Error::<T>::ProposalNotActive)?;
      let family = Self::do_proposal_primary_track_family(domain, item_id)
        .ok_or(Error::<T>::ProposalNotActive)?;
      let aye_weight = Self::proposal_vote_weight_sum(
        domain,
        item_id,
        current_epoch,
        primary_open_epoch,
        primary_close_epoch,
        &votes.ayes,
      );
      let nay_weight = Self::proposal_vote_weight_sum(
        domain,
        item_id,
        current_epoch,
        primary_open_epoch,
        primary_close_epoch,
        &votes.nays,
      );
      let amplify_weight = Self::proposal_vote_weight_sum(
        domain,
        item_id,
        current_epoch,
        primary_open_epoch,
        primary_close_epoch,
        &votes.amplifies,
      );
      let approve_weight = Self::proposal_vote_weight_sum(
        domain,
        item_id,
        current_epoch,
        primary_open_epoch,
        primary_close_epoch,
        &votes.approves,
      );
      let reduce_weight = Self::proposal_vote_weight_sum(
        domain,
        item_id,
        current_epoch,
        primary_open_epoch,
        primary_close_epoch,
        &votes.reduces,
      );
      let veto_weight = Self::proposal_veto_weight_sum(
        domain,
        item_id,
        protection_current_epoch,
        protection_open_epoch,
        protection_close_epoch,
        &votes.vetoes,
      );
      let pass_weight = Self::proposal_veto_weight_sum(
        domain,
        item_id,
        protection_current_epoch,
        protection_open_epoch,
        protection_close_epoch,
        &votes.passes,
      );
      let turnout = match family {
        crate::ProposalPrimaryTrackFamily::Binary => aye_weight.saturating_add(nay_weight),
        crate::ProposalPrimaryTrackFamily::Invoice => amplify_weight
          .saturating_add(approve_weight)
          .saturating_add(reduce_weight)
          .saturating_add(nay_weight),
      };
      if turnout == 0 {
        return Self::reject_active_proposal(domain, item_id, ProposalRejectionReason::NoVotes);
      }
      if turnout
        < Self::turnout_threshold_at(current_epoch, primary_open_epoch, primary_close_epoch)
      {
        return Self::reject_active_proposal(
          domain,
          item_id,
          ProposalRejectionReason::TurnoutBelowMinimum,
        );
      }
      let approval_threshold =
        Self::approval_threshold_at(current_epoch, primary_open_epoch, primary_close_epoch);
      match family {
        crate::ProposalPrimaryTrackFamily::Binary => {
          if aye_weight == nay_weight {
            return Self::reject_active_proposal(domain, item_id, ProposalRejectionReason::VoteTie);
          }
          let aye_approval = Perbill::from_rational(aye_weight, turnout);
          let nay_approval = Perbill::from_rational(nay_weight, turnout);
          if aye_approval >= approval_threshold {
            if pass_weight > veto_weight {
              Self::note_pass_winning_participation(domain, &votes);
            }
            let winners = votes.ayes.into_iter().fold(
              BoundedVec::<T::AccountId, T::MaxWinningVoteAccountsPerCall>::default(),
              |mut winners, ballot| {
                let push_result = winners.try_push(ballot.account);
                if push_result.is_err() {
                  panic!("winner projection must preserve the bounded vote set")
                }
                winners
              },
            );
            return Self::resolve_active_proposal(
              domain,
              item_id,
              winners,
              Some(ProposalPrimaryTrackOption::Aye),
            );
          }
          if nay_approval >= approval_threshold {
            if pass_weight > veto_weight {
              Self::note_pass_winning_participation(domain, &votes);
            }
            let winners = votes.nays.into_iter().fold(
              BoundedVec::<T::AccountId, T::MaxWinningVoteAccountsPerCall>::default(),
              |mut winners, ballot| {
                let push_result = winners.try_push(ballot.account);
                if push_result.is_err() {
                  panic!("winner projection must preserve the bounded vote set")
                }
                winners
              },
            );
            return Self::resolve_active_proposal(
              domain,
              item_id,
              winners,
              Some(ProposalPrimaryTrackOption::Nay),
            );
          }
          Self::reject_active_proposal(
            domain,
            item_id,
            ProposalRejectionReason::ApprovalThresholdNotMet,
          )
        }
        crate::ProposalPrimaryTrackFamily::Invoice => {
          let positive_weight = amplify_weight
            .saturating_add(approve_weight)
            .saturating_add(reduce_weight);
          if positive_weight == nay_weight {
            return Self::reject_active_proposal(domain, item_id, ProposalRejectionReason::VoteTie);
          }
          if positive_weight < nay_weight {
            return Self::reject_active_proposal(
              domain,
              item_id,
              ProposalRejectionReason::ApprovalThresholdNotMet,
            );
          }
          let positive_approval = Perbill::from_rational(positive_weight, turnout);
          if positive_approval < approval_threshold {
            return Self::reject_active_proposal(
              domain,
              item_id,
              ProposalRejectionReason::ApprovalThresholdNotMet,
            );
          }
          if pass_weight > veto_weight {
            Self::note_pass_winning_participation(domain, &votes);
          }
          let (leading_positive_option, _) =
            Self::invoice_leading_positive_weights(amplify_weight, approve_weight, reduce_weight);
          let winners = match leading_positive_option {
            Some(ProposalPrimaryTrackOption::Amplify) => votes.amplifies.into_iter().fold(
              BoundedVec::<T::AccountId, T::MaxWinningVoteAccountsPerCall>::default(),
              |mut winners, ballot| {
                let push_result = winners.try_push(ballot.account);
                if push_result.is_err() {
                  panic!("winner projection must preserve the bounded vote set")
                }
                winners
              },
            ),
            Some(ProposalPrimaryTrackOption::Approve) => votes.approves.into_iter().fold(
              BoundedVec::<T::AccountId, T::MaxWinningVoteAccountsPerCall>::default(),
              |mut winners, ballot| {
                let push_result = winners.try_push(ballot.account);
                if push_result.is_err() {
                  panic!("winner projection must preserve the bounded vote set")
                }
                winners
              },
            ),
            Some(ProposalPrimaryTrackOption::Reduce) => votes.reduces.into_iter().fold(
              BoundedVec::<T::AccountId, T::MaxWinningVoteAccountsPerCall>::default(),
              |mut winners, ballot| {
                let push_result = winners.try_push(ballot.account);
                if push_result.is_err() {
                  panic!("winner projection must preserve the bounded vote set")
                }
                winners
              },
            ),
            _ => {
              return Self::reject_active_proposal(
                domain,
                item_id,
                ProposalRejectionReason::ApprovalThresholdNotMet,
              );
            }
          };
          Self::resolve_active_proposal(domain, item_id, winners, leading_positive_option)
        }
      }
    }

    #[transactional]
    fn resolve_active_proposal_without_winners(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> DispatchResult {
      ensure!(
        ActiveProposals::<T>::contains_key(domain, item_id),
        Error::<T>::ProposalNotActive
      );
      let winner_count = 0;
      let urgent_authorized = Self::proposal_is_urgent_authorized(domain, item_id);
      ActiveProposals::<T>::remove(domain, item_id);
      ProposalConfirmStartedAt::<T>::remove(domain, item_id);
      ProposalUrgentAuthorizedAt::<T>::remove(domain, item_id);
      let proposer = ProposalAuthorsByItem::<T>::take(domain, item_id);
      Self::remove_active_proposal_id(domain, item_id);
      ProposalVotesByItem::<T>::remove(domain, item_id);
      ProposalWinningPrimaryOptionByItem::<T>::remove(domain, item_id);
      let current_epoch = T::EpochProvider::current_epoch();
      if let Some(proposer) = proposer {
        Self::note_successful_authored_proposal(domain, &proposer);
      }
      Self::record_finalized_proposal_outcome(
        domain,
        item_id,
        FinalizedProposalOutcome::Resolved {
          epoch: current_epoch,
          winner_count,
        },
        current_epoch,
      )?;
      let enactment_scheduled = if urgent_authorized {
        ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
        false
      } else {
        Self::schedule_pending_enactment_if_needed(domain, item_id, current_epoch)?
      };
      if !enactment_scheduled {
        Self::maybe_execute_proposal_payload(
          domain,
          item_id,
          current_epoch,
          winner_count,
          current_epoch,
        )?;
      }
      let active_count = ActiveProposalCounts::<T>::mutate(domain, |active_count| {
        *active_count = active_count.saturating_sub(1);
        *active_count
      });
      Self::deposit_event(Event::ProposalResolved {
        domain,
        item_id,
        epoch: current_epoch,
        winner_count,
        active_count,
      });
      Ok(())
    }

    #[transactional]
    pub fn resolve_active_proposal(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      winners: BoundedVec<T::AccountId, T::MaxWinningVoteAccountsPerCall>,
      winning_primary_option: Option<ProposalPrimaryTrackOption>,
    ) -> DispatchResult {
      ensure!(!winners.is_empty(), Error::<T>::ProposalWinnerSetEmpty);
      ensure!(
        ActiveProposals::<T>::contains_key(domain, item_id),
        Error::<T>::ProposalNotActive
      );
      let winner_count = winners.len() as u32;
      let votes = ProposalVotesByItem::<T>::get(domain, item_id);
      let count_total_participation = votes
        .as_ref()
        .map(Self::proposal_has_any_votes)
        .map(|has_votes| !has_votes)
        .unwrap_or(true);
      Self::ingest_winning_vote_resolution_batch_internal(
        domain,
        item_id,
        winners,
        count_total_participation,
      )?;
      let urgent_authorized = Self::proposal_is_urgent_authorized(domain, item_id);
      ActiveProposals::<T>::remove(domain, item_id);
      ProposalConfirmStartedAt::<T>::remove(domain, item_id);
      ProposalUrgentAuthorizedAt::<T>::remove(domain, item_id);
      let proposer = ProposalAuthorsByItem::<T>::take(domain, item_id);
      Self::remove_active_proposal_id(domain, item_id);
      ProposalVotesByItem::<T>::remove(domain, item_id);
      if let Some(winning_primary_option) = winning_primary_option {
        ProposalWinningPrimaryOptionByItem::<T>::insert(domain, item_id, winning_primary_option);
      } else {
        ProposalWinningPrimaryOptionByItem::<T>::remove(domain, item_id);
      }
      let current_epoch = T::EpochProvider::current_epoch();
      if let Some(proposer) = proposer {
        Self::note_successful_authored_proposal(domain, &proposer);
      }
      Self::record_finalized_proposal_outcome(
        domain,
        item_id,
        FinalizedProposalOutcome::Resolved {
          epoch: current_epoch,
          winner_count,
        },
        current_epoch,
      )?;
      let enactment_scheduled = if urgent_authorized {
        ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
        false
      } else {
        Self::schedule_pending_enactment_if_needed(domain, item_id, current_epoch)?
      };
      if !enactment_scheduled {
        Self::maybe_execute_proposal_payload(
          domain,
          item_id,
          current_epoch,
          winner_count,
          current_epoch,
        )?;
      }
      let active_count = ActiveProposalCounts::<T>::mutate(domain, |active_count| {
        *active_count = active_count.saturating_sub(1);
        *active_count
      });
      Self::deposit_event(Event::ProposalResolved {
        domain,
        item_id,
        epoch: current_epoch,
        winner_count,
        active_count,
      });
      Ok(())
    }

    #[transactional]
    pub fn resolve_active_proposal_from_votes(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> DispatchResult {
      Self::resolve_active_proposal_from_votes_with_policy(domain, item_id, true)
    }

    #[transactional]
    fn resolve_active_proposal_from_votes_with_policy(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      enforce_voting_window: bool,
    ) -> DispatchResult {
      match Self::proposal_resolution_state(domain, item_id) {
        None => Err(Error::<T>::ProposalNotActive.into()),
        Some(ProposalResolutionState::VotingWindowOpen { .. }) if enforce_voting_window => {
          Err(Error::<T>::ProposalVotingWindowStillOpen.into())
        }
        Some(ProposalResolutionState::VotingWindowOpen { .. }) => {
          let votes = ProposalVotesByItem::<T>::get(domain, item_id).unwrap_or(ProposalVotes {
            ayes: BoundedVec::default(),
            nays: BoundedVec::default(),
            amplifies: BoundedVec::default(),
            approves: BoundedVec::default(),
            reduces: BoundedVec::default(),
            vetoes: BoundedVec::default(),
            passes: BoundedVec::default(),
          });
          Self::resolve_or_reject_from_current_votes(domain, item_id, votes)
        }
        Some(ProposalResolutionState::VetoPassing {
          veto_weight,
          pass_weight,
          mode,
        }) => Self::veto_cancel_active_proposal(
          domain,
          item_id,
          VetoCancellation {
            veto_weight,
            pass_weight,
            mode,
          },
        ),
        Some(ProposalResolutionState::PassingAye)
        | Some(ProposalResolutionState::PassingAmplify)
        | Some(ProposalResolutionState::PassingApprove)
        | Some(ProposalResolutionState::PassingReduce)
        | Some(ProposalResolutionState::PassingNay)
        | Some(ProposalResolutionState::Confirming { .. })
        | Some(ProposalResolutionState::Rejected { .. }) => {
          let votes = ProposalVotesByItem::<T>::get(domain, item_id).unwrap_or(ProposalVotes {
            ayes: BoundedVec::default(),
            nays: BoundedVec::default(),
            amplifies: BoundedVec::default(),
            approves: BoundedVec::default(),
            reduces: BoundedVec::default(),
            vetoes: BoundedVec::default(),
            passes: BoundedVec::default(),
          });
          Self::resolve_or_reject_from_current_votes(domain, item_id, votes)
        }
      }
    }

    #[transactional]
    pub fn reject_active_proposal(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      reason: ProposalRejectionReason,
    ) -> DispatchResult {
      ensure!(
        ActiveProposals::<T>::take(domain, item_id).is_some(),
        Error::<T>::ProposalNotActive
      );
      ProposalAuthorsByItem::<T>::remove(domain, item_id);
      ProposalConfirmStartedAt::<T>::remove(domain, item_id);
      ProposalUrgentAuthorizedAt::<T>::remove(domain, item_id);
      ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
      ProposalWinningPrimaryOptionByItem::<T>::remove(domain, item_id);
      Self::remove_active_proposal_id(domain, item_id);
      ProposalVotesByItem::<T>::remove(domain, item_id);
      let current_epoch = T::EpochProvider::current_epoch();
      Self::record_finalized_proposal_outcome(
        domain,
        item_id,
        FinalizedProposalOutcome::Rejected {
          epoch: current_epoch,
          reason,
        },
        current_epoch,
      )?;
      let active_count = ActiveProposalCounts::<T>::mutate(domain, |active_count| {
        *active_count = active_count.saturating_sub(1);
        *active_count
      });
      Self::deposit_event(Event::ProposalRejected {
        domain,
        item_id,
        epoch: current_epoch,
        reason,
        active_count,
      });
      Ok(())
    }

    #[transactional]
    fn veto_cancel_active_proposal(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      cancellation: VetoCancellation,
    ) -> DispatchResult {
      ensure!(
        ActiveProposals::<T>::take(domain, item_id).is_some(),
        Error::<T>::ProposalNotActive
      );
      ProposalAuthorsByItem::<T>::remove(domain, item_id);
      ProposalConfirmStartedAt::<T>::remove(domain, item_id);
      ProposalUrgentAuthorizedAt::<T>::remove(domain, item_id);
      ProposalPendingEnactmentAt::<T>::remove(domain, item_id);
      ProposalWinningPrimaryOptionByItem::<T>::remove(domain, item_id);
      Self::remove_active_proposal_id(domain, item_id);
      let votes = ProposalVotesByItem::<T>::get(domain, item_id);
      if let Some(votes) = &votes {
        Self::note_winning_participation_batch(
          domain,
          votes.vetoes.iter().map(|ballot| ballot.account.clone()),
        );
      }
      ProposalVotesByItem::<T>::remove(domain, item_id);
      let current_epoch = T::EpochProvider::current_epoch();
      Self::record_finalized_proposal_outcome(
        domain,
        item_id,
        FinalizedProposalOutcome::VetoCancelled {
          epoch: current_epoch,
          veto_weight: cancellation.veto_weight,
        },
        current_epoch,
      )?;
      let active_count = ActiveProposalCounts::<T>::mutate(domain, |active_count| {
        *active_count = active_count.saturating_sub(1);
        *active_count
      });
      Self::deposit_event(Event::ProposalVetoCancelled {
        domain,
        item_id,
        epoch: current_epoch,
        veto_weight: cancellation.veto_weight,
        pass_weight: cancellation.pass_weight,
        mode: cancellation.mode,
        active_count,
      });
      Ok(())
    }

    fn ingest_winning_vote_resolution_batch_internal(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      accounts: BoundedVec<T::AccountId, T::MaxWinningVoteAccountsPerCall>,
      count_total_participation: bool,
    ) -> DispatchResult {
      let lookback = T::WinningVoteLookbackEpochs::get();
      ensure!(lookback > 0, Error::<T>::ZeroLookbackWindow);
      let current_epoch = T::EpochProvider::current_epoch();
      let mut seen_accounts = BTreeSet::new();
      for account in &accounts {
        ensure!(
          seen_accounts.insert(account.encode()),
          Error::<T>::DuplicateWinningVoteAccount
        );
      }
      Self::record_winning_vote_resolution_item(domain, item_id, current_epoch)?;
      for account in accounts {
        if count_total_participation {
          Self::note_total_participation(domain, &account);
        }
        Self::note_winning_participation(domain, &account);
        Self::record_winning_vote_for_account(domain, item_id, account, current_epoch)?;
      }
      Ok(())
    }

    #[transactional]
    pub fn ingest_winning_vote_resolution(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      account: T::AccountId,
    ) -> DispatchResult {
      let accounts = BoundedVec::try_from(Vec::from([account]))
        .expect("single winning-vote account must fit configured bound");
      Self::ingest_winning_vote_resolution_batch_internal(domain, item_id, accounts, true)
    }

    #[transactional]
    pub fn ingest_winning_vote_resolution_batch(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      accounts: BoundedVec<T::AccountId, T::MaxWinningVoteAccountsPerCall>,
    ) -> DispatchResult {
      Self::ingest_winning_vote_resolution_batch_internal(domain, item_id, accounts, true)
    }

    fn do_reward_coefficient(domain: T::DomainId, account: &T::AccountId) -> FixedU128 {
      let Some(mut window) = WinningVoteWindows::<T>::get(domain, account) else {
        return FixedU128::from_inner(0);
      };
      let lookback = T::WinningVoteLookbackEpochs::get();
      let max_votes = T::MaxWinningVotesPerEpoch::get();
      if lookback == 0 || max_votes == 0 {
        return FixedU128::from_inner(0);
      }
      Self::rotate_window_to(&mut window, T::EpochProvider::current_epoch());
      if window.rolling_sum == 0 {
        return FixedU128::from_inner(0);
      }
      FixedU128::from_rational(
        u128::from(window.rolling_sum),
        u128::from(lookback) * u128::from(max_votes),
      )
    }

    fn do_govxp_counters(domain: T::DomainId, account: &T::AccountId) -> GovXpCounters {
      let rolling_winning_participation = WinningVoteWindows::<T>::get(domain, account)
        .map(|mut window| {
          Self::rotate_window_to(&mut window, T::EpochProvider::current_epoch());
          window.rolling_sum
        })
        .unwrap_or(0);
      let participation_totals = ParticipationTotalsByAccount::<T>::get(domain, account);
      let authorship_totals = ProposalAuthorshipTotalsByAccount::<T>::get(domain, account);
      GovXpCounters {
        rolling_winning_participation,
        total_participations: participation_totals.total_participations,
        total_winning_participations: participation_totals.winning_participations,
        total_authored_proposals: authorship_totals.authored_proposals,
        total_successful_authored_proposals: authorship_totals.successful_authored_proposals,
      }
    }

    fn track_family_for_vote_kind(vote: ProposalVoteKind) -> crate::ProposalTrackFamily {
      match vote {
        ProposalVoteKind::Aye
        | ProposalVoteKind::Nay
        | ProposalVoteKind::Amplify
        | ProposalVoteKind::Approve
        | ProposalVoteKind::Reduce => crate::ProposalTrackFamily::Ordinary,
        ProposalVoteKind::Veto | ProposalVoteKind::Pass => crate::ProposalTrackFamily::Veto,
      }
    }

    fn finalized_outcome_epoch(outcome: &FinalizedProposalOutcome<T::Epoch>) -> T::Epoch {
      match outcome {
        FinalizedProposalOutcome::Resolved { epoch, .. }
        | FinalizedProposalOutcome::Rejected { epoch, .. }
        | FinalizedProposalOutcome::VetoCancelled { epoch, .. } => *epoch,
        FinalizedProposalOutcome::Enacted { executed_epoch, .. } => *executed_epoch,
        FinalizedProposalOutcome::ExecutionFailed { failed_epoch, .. } => *failed_epoch,
        FinalizedProposalOutcome::AdvisoryFinalized {
          finalized_epoch, ..
        } => *finalized_epoch,
      }
    }

    fn do_recent_finalized_proposals(
      domain: T::DomainId,
    ) -> BoundedVec<
      RecentFinalizedProposal<T::WinningVoteItemId, T::Epoch>,
      T::MaxRecentFinalizedProposalsPerDomain,
    > {
      let mut proposals = FinalizedProposalOutcomes::<T>::iter_prefix(domain)
        .map(|(item_id, outcome)| RecentFinalizedProposal { item_id, outcome })
        .collect::<Vec<_>>();
      proposals.sort_by(|left, right| {
        Self::finalized_outcome_epoch(&right.outcome)
          .cmp(&Self::finalized_outcome_epoch(&left.outcome))
          .then_with(|| left.item_id.cmp(&right.item_id))
      });
      proposals
        .into_iter()
        .fold(BoundedVec::default(), |mut bounded, proposal| {
          let push_result = bounded.try_push(proposal);
          if push_result.is_err() {
            panic!("recent finalized proposal query bound must cover retained outcomes")
          }
          bounded
        })
    }

    fn do_proposal_vote_power_profile(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      vote: ProposalVoteKind,
    ) -> Option<crate::ProposalVotePowerProfile> {
      ActiveProposals::<T>::contains_key(domain, item_id).then(|| {
        T::ProposalTrackPowerProfileProvider::power_profile(
          domain,
          item_id,
          Self::track_family_for_vote_kind(vote),
        )
      })
    }

    fn invoice_leading_positive_weights(
      amplify_weight: u64,
      approve_weight: u64,
      reduce_weight: u64,
    ) -> (Option<ProposalPrimaryTrackOption>, u64) {
      let mut leading_option = None;
      let mut leading_weight = 0u64;
      if amplify_weight > 0 {
        leading_option = Some(ProposalPrimaryTrackOption::Amplify);
        leading_weight = amplify_weight;
      }
      if approve_weight > 0 && approve_weight >= leading_weight {
        leading_option = Some(ProposalPrimaryTrackOption::Approve);
        leading_weight = approve_weight;
      }
      if reduce_weight > 0 && reduce_weight >= leading_weight {
        leading_option = Some(ProposalPrimaryTrackOption::Reduce);
        leading_weight = reduce_weight;
      }
      (leading_option, leading_weight)
    }

    fn invoice_leading_positive_option(
      tally: &ProposalVoteTally,
    ) -> (Option<ProposalPrimaryTrackOption>, u64) {
      Self::invoice_leading_positive_weights(
        tally.amplify_weight,
        tally.approve_weight,
        tally.reduce_weight,
      )
    }

    fn do_proposal_primary_track_tally(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<ProposalPrimaryTrackTally> {
      let family = Self::do_proposal_primary_track_family(domain, item_id)?;
      let tally = Self::do_proposal_vote_tally(domain, item_id)?;
      match family {
        crate::ProposalPrimaryTrackFamily::Binary => {
          let leading_option = if tally.aye_weight > tally.nay_weight {
            Some(ProposalPrimaryTrackOption::Aye)
          } else if tally.nay_weight > tally.aye_weight {
            Some(ProposalPrimaryTrackOption::Nay)
          } else {
            None
          };
          Some(ProposalPrimaryTrackTally::Binary {
            aye_voters: tally.aye_voters,
            nay_voters: tally.nay_voters,
            aye_weight: tally.aye_weight,
            nay_weight: tally.nay_weight,
            turnout_weight: tally.turnout_weight,
            leading_option,
          })
        }
        crate::ProposalPrimaryTrackFamily::Invoice => {
          let positive_weight = tally
            .amplify_weight
            .saturating_add(tally.approve_weight)
            .saturating_add(tally.reduce_weight);
          let (leading_positive_option, leading_positive_weight) =
            Self::invoice_leading_positive_option(&tally);
          Some(ProposalPrimaryTrackTally::Invoice {
            amplify_voters: tally.amplify_voters,
            approve_voters: tally.approve_voters,
            reduce_voters: tally.reduce_voters,
            nay_voters: tally.nay_voters,
            amplify_weight: tally.amplify_weight,
            approve_weight: tally.approve_weight,
            reduce_weight: tally.reduce_weight,
            nay_weight: tally.nay_weight,
            positive_weight,
            turnout_weight: tally.turnout_weight,
            leading_positive_option,
            leading_positive_weight,
          })
        }
      }
    }

    fn do_proposal_vote_tally(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<ProposalVoteTally> {
      let (current_epoch, primary_open_epoch, primary_close_epoch) =
        Self::proposal_ordinary_weighting_window(domain, item_id)?;
      let (protection_current_epoch, protection_open_epoch, protection_close_epoch) =
        Self::proposal_protection_weighting_window(domain, item_id)?;
      let votes = ProposalVotesByItem::<T>::get(domain, item_id).unwrap_or(ProposalVotes {
        ayes: BoundedVec::default(),
        nays: BoundedVec::default(),
        amplifies: BoundedVec::default(),
        approves: BoundedVec::default(),
        reduces: BoundedVec::default(),
        vetoes: BoundedVec::default(),
        passes: BoundedVec::default(),
      });
      let aye_weight = Self::proposal_vote_weight_sum(
        domain,
        item_id,
        current_epoch,
        primary_open_epoch,
        primary_close_epoch,
        &votes.ayes,
      );
      let nay_weight = Self::proposal_vote_weight_sum(
        domain,
        item_id,
        current_epoch,
        primary_open_epoch,
        primary_close_epoch,
        &votes.nays,
      );
      let amplify_weight = Self::proposal_vote_weight_sum(
        domain,
        item_id,
        current_epoch,
        primary_open_epoch,
        primary_close_epoch,
        &votes.amplifies,
      );
      let approve_weight = Self::proposal_vote_weight_sum(
        domain,
        item_id,
        current_epoch,
        primary_open_epoch,
        primary_close_epoch,
        &votes.approves,
      );
      let reduce_weight = Self::proposal_vote_weight_sum(
        domain,
        item_id,
        current_epoch,
        primary_open_epoch,
        primary_close_epoch,
        &votes.reduces,
      );
      let veto_weight = Self::proposal_veto_weight_sum(
        domain,
        item_id,
        protection_current_epoch,
        protection_open_epoch,
        protection_close_epoch,
        &votes.vetoes,
      );
      let pass_weight = Self::proposal_veto_weight_sum(
        domain,
        item_id,
        protection_current_epoch,
        protection_open_epoch,
        protection_close_epoch,
        &votes.passes,
      );
      let turnout_weight = aye_weight
        .saturating_add(nay_weight)
        .saturating_add(amplify_weight)
        .saturating_add(approve_weight)
        .saturating_add(reduce_weight);
      let veto_turnout_weight = veto_weight.saturating_add(pass_weight);
      Some(ProposalVoteTally {
        aye_voters: votes.ayes.len() as u32,
        nay_voters: votes.nays.len() as u32,
        amplify_voters: votes.amplifies.len() as u32,
        approve_voters: votes.approves.len() as u32,
        reduce_voters: votes.reduces.len() as u32,
        veto_voters: votes.vetoes.len() as u32,
        pass_voters: votes.passes.len() as u32,
        aye_weight,
        nay_weight,
        amplify_weight,
        approve_weight,
        reduce_weight,
        veto_weight,
        pass_weight,
        turnout_weight,
        veto_turnout_weight,
      })
    }

    pub fn proposal_resolution_state(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<ProposalResolutionState<T::Epoch>> {
      let proposal = ActiveProposals::<T>::get(domain, item_id)?;
      let current_epoch = T::EpochProvider::current_epoch();
      let maturity_epoch =
        Self::proposal_effective_primary_close_epoch(domain, item_id, proposal.submitted_epoch)
          .ok()?;
      let tally = Self::do_proposal_vote_tally(domain, item_id)?;
      let votes = ProposalVotesByItem::<T>::get(domain, item_id).unwrap_or(ProposalVotes {
        ayes: BoundedVec::default(),
        nays: BoundedVec::default(),
        amplifies: BoundedVec::default(),
        approves: BoundedVec::default(),
        reduces: BoundedVec::default(),
        vetoes: BoundedVec::default(),
        passes: BoundedVec::default(),
      });
      if let Some(cancellation) = Self::current_veto_cancellation(domain, item_id, &votes, false) {
        return Some(ProposalResolutionState::VetoPassing {
          veto_weight: cancellation.veto_weight,
          pass_weight: cancellation.pass_weight,
          mode: cancellation.mode,
        });
      }
      if current_epoch.saturated_into::<u32>() < maturity_epoch.saturated_into::<u32>() {
        return Some(ProposalResolutionState::VotingWindowOpen {
          current_epoch,
          maturity_epoch,
        });
      }
      let urgent_authorized = Self::proposal_is_urgent_authorized(domain, item_id);
      if let Some(cancellation) =
        Self::current_veto_cancellation(domain, item_id, &votes, !urgent_authorized)
      {
        return Some(ProposalResolutionState::VetoPassing {
          veto_weight: cancellation.veto_weight,
          pass_weight: cancellation.pass_weight,
          mode: cancellation.mode,
        });
      }
      if tally.turnout_weight == 0 {
        return Some(ProposalResolutionState::Rejected {
          reason: ProposalRejectionReason::NoVotes,
        });
      }
      if tally.aye_weight == tally.nay_weight {
        return Some(ProposalResolutionState::Rejected {
          reason: ProposalRejectionReason::VoteTie,
        });
      }
      if tally.turnout_weight
        < Self::turnout_threshold_at(
          current_epoch,
          Self::proposal_effective_primary_open_epoch(domain, item_id, proposal.submitted_epoch)
            .ok()?,
          maturity_epoch,
        )
      {
        return Some(ProposalResolutionState::Rejected {
          reason: ProposalRejectionReason::TurnoutBelowMinimum,
        });
      }
      let approval_threshold = Self::approval_threshold_at(
        current_epoch,
        Self::proposal_effective_primary_open_epoch(domain, item_id, proposal.submitted_epoch)
          .ok()?,
        maturity_epoch,
      );
      let passing_state = match Self::do_proposal_primary_track_family(domain, item_id)? {
        crate::ProposalPrimaryTrackFamily::Binary => {
          if tally.aye_weight == tally.nay_weight {
            return Some(ProposalResolutionState::Rejected {
              reason: ProposalRejectionReason::VoteTie,
            });
          }
          let aye_approval = Perbill::from_rational(tally.aye_weight, tally.turnout_weight);
          let nay_approval = Perbill::from_rational(tally.nay_weight, tally.turnout_weight);
          if aye_approval >= approval_threshold {
            ProposalResolutionState::PassingAye
          } else if nay_approval >= approval_threshold {
            ProposalResolutionState::PassingNay
          } else {
            return Some(ProposalResolutionState::Rejected {
              reason: ProposalRejectionReason::ApprovalThresholdNotMet,
            });
          }
        }
        crate::ProposalPrimaryTrackFamily::Invoice => {
          let positive_weight = tally
            .amplify_weight
            .saturating_add(tally.approve_weight)
            .saturating_add(tally.reduce_weight);
          if positive_weight == tally.nay_weight {
            return Some(ProposalResolutionState::Rejected {
              reason: ProposalRejectionReason::VoteTie,
            });
          }
          if positive_weight < tally.nay_weight {
            return Some(ProposalResolutionState::Rejected {
              reason: ProposalRejectionReason::ApprovalThresholdNotMet,
            });
          }
          let positive_approval = Perbill::from_rational(positive_weight, tally.turnout_weight);
          if positive_approval < approval_threshold {
            return Some(ProposalResolutionState::Rejected {
              reason: ProposalRejectionReason::ApprovalThresholdNotMet,
            });
          }
          match Self::invoice_leading_positive_option(&tally).0 {
            Some(ProposalPrimaryTrackOption::Amplify) => ProposalResolutionState::PassingAmplify,
            Some(ProposalPrimaryTrackOption::Approve) => ProposalResolutionState::PassingApprove,
            Some(ProposalPrimaryTrackOption::Reduce) => ProposalResolutionState::PassingReduce,
            _ => {
              return Some(ProposalResolutionState::Rejected {
                reason: ProposalRejectionReason::ApprovalThresholdNotMet,
              });
            }
          }
        }
      };
      if let Some(confirm_started) = ProposalConfirmStartedAt::<T>::get(domain, item_id) {
        let confirm_end_u32 = confirm_started
          .saturated_into::<u32>()
          .saturating_add(T::ProposalConfirmPeriod::get().saturated_into::<u32>());
        return Some(ProposalResolutionState::Confirming {
          confirm_started_epoch: confirm_started,
          confirm_end_epoch: confirm_end_u32.saturated_into(),
        });
      }
      Some(passing_state)
    }

    fn do_proposal_status(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<ProposalStatus<T::Epoch>> {
      if let Some(resolution_state) = Self::proposal_resolution_state(domain, item_id) {
        return Some(ProposalStatus::Active(resolution_state));
      }
      let outcome = Self::finalized_proposal_outcome(domain, item_id)?;
      if let Some(enactment_epoch) = ProposalPendingEnactmentAt::<T>::get(domain, item_id) {
        if T::EpochProvider::current_epoch().saturated_into::<u32>()
          < enactment_epoch.saturated_into::<u32>()
        {
          return Some(ProposalStatus::PendingEnactment {
            outcome,
            enactment_epoch,
          });
        }
      }
      Some(ProposalStatus::Finalized(outcome))
    }

    fn record_winning_vote_resolution_item(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      current_epoch: T::Epoch,
    ) -> DispatchResult {
      WinningVoteResolutionWindows::<T>::try_mutate(domain, |maybe_window| -> DispatchResult {
        let mut window = maybe_window
          .take()
          .unwrap_or_else(|| Self::fresh_resolution_window(current_epoch));
        Self::rotate_resolution_window_to(&mut window, current_epoch);
        let item_already_resolved = window.epochs.iter().any(|slot| {
          slot
            .item_ids
            .iter()
            .any(|existing_item_id| *existing_item_id == item_id)
        });
        ensure!(
          !item_already_resolved,
          Error::<T>::DuplicateWinningVoteResolutionItem
        );
        let slot_index = Self::slot_index(current_epoch);
        let epoch_slot = window
          .epochs
          .get_mut(slot_index)
          .expect("fresh resolution window always has full lookback width");
        epoch_slot
          .item_ids
          .try_push(item_id)
          .map_err(|_| Error::<T>::WinningVoteResolutionItemSetFull)?;
        *maybe_window = Some(window);
        Ok(())
      })
    }

    fn note_total_participation(domain: T::DomainId, account: &T::AccountId) {
      ParticipationTotalsByAccount::<T>::mutate(domain, account, |totals| {
        totals.total_participations = totals.total_participations.saturating_add(1);
      });
    }

    fn note_winning_participation(domain: T::DomainId, account: &T::AccountId) {
      ParticipationTotalsByAccount::<T>::mutate(domain, account, |totals| {
        totals.winning_participations = totals.winning_participations.saturating_add(1);
      });
    }

    fn note_authored_proposal(domain: T::DomainId, account: &T::AccountId) {
      ProposalAuthorshipTotalsByAccount::<T>::mutate(domain, account, |totals| {
        totals.authored_proposals = totals.authored_proposals.saturating_add(1);
      });
    }

    fn note_successful_authored_proposal(domain: T::DomainId, account: &T::AccountId) {
      ProposalAuthorshipTotalsByAccount::<T>::mutate(domain, account, |totals| {
        totals.successful_authored_proposals =
          totals.successful_authored_proposals.saturating_add(1);
      });
    }

    fn record_winning_vote_for_account(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
      account: T::AccountId,
      current_epoch: T::Epoch,
    ) -> DispatchResult {
      let mut epoch_count = 0u16;
      let mut rolling_sum = 0u32;
      WinningVoteWindows::<T>::try_mutate(domain, &account, |maybe_window| -> DispatchResult {
        let mut window = maybe_window
          .take()
          .unwrap_or_else(|| Self::fresh_window(current_epoch));
        Self::rotate_window_to(&mut window, current_epoch);
        let item_already_counted = window.epochs.iter().any(|slot| {
          slot
            .item_ids
            .iter()
            .any(|existing_item_id| *existing_item_id == item_id)
        });
        ensure!(!item_already_counted, Error::<T>::DuplicateWinningVoteItem);
        let slot_index = Self::slot_index(current_epoch);
        let epoch_slot = window
          .epochs
          .get_mut(slot_index)
          .expect("fresh window always has full lookback width");
        ensure!(
          epoch_slot.item_ids.len() < usize::from(T::MaxWinningVotesPerEpoch::get()),
          Error::<T>::EpochVoteCapReached
        );
        epoch_slot
          .item_ids
          .try_push(item_id)
          .map_err(|_| Error::<T>::WinningVoteItemSetFull)?;
        epoch_count = epoch_slot.item_ids.len() as u16;
        window.rolling_sum = window.rolling_sum.saturating_add(1);
        rolling_sum = window.rolling_sum;
        *maybe_window = Some(window);
        Ok(())
      })?;
      Self::schedule_expiry(domain, &account, current_epoch)?;
      Self::deposit_event(Event::WinningVoteRecorded {
        domain,
        item_id,
        account,
        epoch: current_epoch,
        epoch_count,
        rolling_sum,
      });
      Ok(())
    }

    fn fresh_window(
      current_epoch: T::Epoch,
    ) -> WinningVoteWindow<
      T::Epoch,
      T::WinningVoteItemId,
      T::WinningVoteLookbackEpochs,
      T::MaxWinningVoteItemsPerEpoch,
    > {
      let mut epochs = BoundedVec::default();
      for _ in 0..T::WinningVoteLookbackEpochs::get() {
        let push_result = epochs.try_push(WinningVoteEpochSlot {
          item_ids: BoundedVec::default(),
        });
        if push_result.is_err() {
          panic!("fresh window slot vector fits configured lookback")
        }
      }
      WinningVoteWindow {
        last_epoch: current_epoch,
        epochs,
        rolling_sum: 0,
      }
    }

    fn fresh_resolution_window(
      current_epoch: T::Epoch,
    ) -> WinningVoteResolutionWindow<
      T::Epoch,
      T::WinningVoteItemId,
      T::WinningVoteLookbackEpochs,
      T::MaxWinningVoteResolutionItemsPerEpoch,
    > {
      let mut epochs = BoundedVec::default();
      for _ in 0..T::WinningVoteLookbackEpochs::get() {
        let push_result = epochs.try_push(WinningVoteEpochSlot {
          item_ids: BoundedVec::default(),
        });
        if push_result.is_err() {
          panic!("fresh resolution window slot vector fits configured lookback")
        }
      }
      WinningVoteResolutionWindow {
        last_epoch: current_epoch,
        epochs,
      }
    }

    fn slot_index(epoch: T::Epoch) -> usize {
      let lookback = T::WinningVoteLookbackEpochs::get();
      if lookback == 0 {
        return 0;
      }
      (epoch.saturated_into::<u32>() % lookback) as usize
    }

    fn rotate_window_to(
      window: &mut WinningVoteWindow<
        T::Epoch,
        T::WinningVoteItemId,
        T::WinningVoteLookbackEpochs,
        T::MaxWinningVoteItemsPerEpoch,
      >,
      current_epoch: T::Epoch,
    ) {
      let lookback = T::WinningVoteLookbackEpochs::get();
      if lookback == 0 {
        window.last_epoch = current_epoch;
        window.rolling_sum = 0;
        for epoch_slot in window.epochs.iter_mut() {
          epoch_slot.item_ids.clear();
        }
        return;
      }
      let last_epoch = window.last_epoch.saturated_into::<u32>();
      let current_epoch_u32 = current_epoch.saturated_into::<u32>();
      if current_epoch_u32 <= last_epoch {
        window.last_epoch = current_epoch;
        return;
      }
      let delta = current_epoch_u32.saturating_sub(last_epoch);
      if delta >= lookback {
        for epoch_slot in window.epochs.iter_mut() {
          epoch_slot.item_ids.clear();
        }
        window.rolling_sum = 0;
        window.last_epoch = current_epoch;
        return;
      }
      let old_expired_epoch = last_epoch.saturating_sub(lookback);
      let new_expired_epoch = current_epoch_u32.saturating_sub(lookback);
      for expired_epoch in old_expired_epoch.saturating_add(1)..=new_expired_epoch {
        let slot_index = (expired_epoch % lookback) as usize;
        let epoch_slot = window
          .epochs
          .get_mut(slot_index)
          .expect("slot index always stays within lookback width");
        window.rolling_sum = window
          .rolling_sum
          .saturating_sub(epoch_slot.item_ids.len() as u32);
        epoch_slot.item_ids.clear();
      }
      window.last_epoch = current_epoch;
    }

    fn rotate_resolution_window_to(
      window: &mut WinningVoteResolutionWindow<
        T::Epoch,
        T::WinningVoteItemId,
        T::WinningVoteLookbackEpochs,
        T::MaxWinningVoteResolutionItemsPerEpoch,
      >,
      current_epoch: T::Epoch,
    ) {
      let lookback = T::WinningVoteLookbackEpochs::get();
      if lookback == 0 {
        window.last_epoch = current_epoch;
        for epoch_slot in window.epochs.iter_mut() {
          epoch_slot.item_ids.clear();
        }
        return;
      }
      let last_epoch = window.last_epoch.saturated_into::<u32>();
      let current_epoch_u32 = current_epoch.saturated_into::<u32>();
      if current_epoch_u32 <= last_epoch {
        window.last_epoch = current_epoch;
        return;
      }
      let delta = current_epoch_u32.saturating_sub(last_epoch);
      if delta >= lookback {
        for epoch_slot in window.epochs.iter_mut() {
          epoch_slot.item_ids.clear();
        }
        window.last_epoch = current_epoch;
        return;
      }
      let old_expired_epoch = last_epoch.saturating_sub(lookback);
      let new_expired_epoch = current_epoch_u32.saturating_sub(lookback);
      for expired_epoch in old_expired_epoch.saturating_add(1)..=new_expired_epoch {
        let slot_index = (expired_epoch % lookback) as usize;
        let epoch_slot = window
          .epochs
          .get_mut(slot_index)
          .expect("slot index always stays within lookback width");
        epoch_slot.item_ids.clear();
      }
      window.last_epoch = current_epoch;
    }

    fn schedule_expiry(
      domain: T::DomainId,
      account: &T::AccountId,
      current_epoch: T::Epoch,
    ) -> DispatchResult {
      let lookback = T::WinningVoteLookbackEpochs::get();
      ensure!(lookback > 0, Error::<T>::ZeroLookbackWindow);
      let expiry_epoch_u32 = current_epoch
        .saturated_into::<u32>()
        .checked_add(lookback)
        .ok_or(Error::<T>::EpochArithmeticOverflow)?;
      let expiry_epoch: T::Epoch = expiry_epoch_u32.saturated_into();
      ExpiryBuckets::<T>::try_mutate(expiry_epoch, |bucket| -> DispatchResult {
        let exists = bucket
          .iter()
          .any(|entry| entry.domain == domain && entry.account == *account);
        if !exists {
          bucket
            .try_push(ExpiringAccountTouch {
              domain,
              account: account.clone(),
            })
            .map_err(|_| Error::<T>::ExpiryBucketFull)?;
        }
        Ok(())
      })
    }

    fn service_current_epoch(current_epoch: T::Epoch) -> Weight {
      let last_processed_epoch = LastProcessedEpoch::<T>::get().saturated_into::<u32>();
      let current_epoch_u32 = current_epoch.saturated_into::<u32>();
      if current_epoch_u32 <= last_processed_epoch {
        return Weight::zero();
      }
      let maturing_weight = Self::service_maturing_proposals(last_processed_epoch, current_epoch);
      let pending_enactment_weight =
        Self::service_pending_enactments(last_processed_epoch, current_epoch);
      let finalized_weight =
        Self::service_finalized_proposal_outcomes(last_processed_epoch, current_epoch);
      let expiring_weight = Self::service_expiring_accounts(last_processed_epoch, current_epoch);
      LastProcessedEpoch::<T>::put(current_epoch);
      maturing_weight
        .saturating_add(pending_enactment_weight)
        .saturating_add(finalized_weight)
        .saturating_add(expiring_weight)
    }

    fn service_maturing_proposals(last_processed_epoch: u32, current_epoch: T::Epoch) -> Weight {
      let current_epoch_u32 = current_epoch.saturated_into::<u32>();
      let confirm_period = T::ProposalConfirmPeriod::get().saturated_into::<u32>();
      let mut processed_entries = 0u32;
      for epoch_u32 in last_processed_epoch.saturating_add(1)..=current_epoch_u32 {
        let epoch: T::Epoch = epoch_u32.saturated_into();
        let bucket = ProposalMaturityBuckets::<T>::take(epoch);
        processed_entries = processed_entries.saturating_add(bucket.len() as u32);
        for touch in bucket {
          if !ActiveProposals::<T>::contains_key(touch.domain, touch.item_id) {
            ProposalConfirmStartedAt::<T>::remove(touch.domain, touch.item_id);
            continue;
          }
          if confirm_period == 0 {
            if Self::resolve_active_proposal_from_votes(touch.domain, touch.item_id).is_ok() {
              continue;
            }
          } else if ProposalConfirmStartedAt::<T>::contains_key(touch.domain, touch.item_id) {
            // Confirm-end: proposal sustained approval for the full confirm period
            ProposalConfirmStartedAt::<T>::remove(touch.domain, touch.item_id);
            if Self::resolve_active_proposal_from_votes(touch.domain, touch.item_id).is_ok() {
              continue;
            }
          } else {
            // First maturity: check if currently passing → enter confirm
            match Self::proposal_resolution_state(touch.domain, touch.item_id) {
              Some(ProposalResolutionState::PassingAye)
              | Some(ProposalResolutionState::PassingAmplify)
              | Some(ProposalResolutionState::PassingApprove)
              | Some(ProposalResolutionState::PassingReduce)
              | Some(ProposalResolutionState::PassingNay) => {
                let confirm_end_epoch_u32 = epoch_u32.saturating_add(confirm_period);
                ProposalConfirmStartedAt::<T>::insert(touch.domain, touch.item_id, epoch);
                let rescheduled = Self::schedule_proposal_maturity_at(
                  confirm_end_epoch_u32.saturated_into(),
                  touch.domain,
                  touch.item_id,
                )
                .is_ok();
                if rescheduled {
                  Self::deposit_event(Event::ProposalConfirmStarted {
                    domain: touch.domain,
                    item_id: touch.item_id,
                    confirm_started_epoch: epoch,
                    confirm_end_epoch: confirm_end_epoch_u32.saturated_into(),
                  });
                }
                continue;
              }
              Some(ProposalResolutionState::VetoPassing { .. }) => {
                if Self::resolve_active_proposal_from_votes(touch.domain, touch.item_id).is_ok() {
                  continue;
                }
              }
              _ => {
                if Self::resolve_active_proposal_from_votes(touch.domain, touch.item_id).is_ok() {
                  continue;
                }
              }
            }
          }
          let next_epoch_u32 = epoch_u32.saturating_add(1);
          let rescheduled = Self::schedule_proposal_maturity_at(
            next_epoch_u32.saturated_into(),
            touch.domain,
            touch.item_id,
          )
          .is_ok();
          Self::deposit_event(Event::ProposalAutoFinalizationDeferred {
            domain: touch.domain,
            item_id: touch.item_id,
            epoch: current_epoch,
            rescheduled,
          });
        }
      }
      T::WeightInfo::service_maturing_proposals(processed_entries)
    }

    fn service_pending_enactments(last_processed_epoch: u32, current_epoch: T::Epoch) -> Weight {
      let current_epoch_u32 = current_epoch.saturated_into::<u32>();
      let mut processed_entries = 0u32;
      for epoch_u32 in last_processed_epoch.saturating_add(1)..=current_epoch_u32 {
        let epoch: T::Epoch = epoch_u32.saturated_into();
        let bucket = PendingEnactmentBuckets::<T>::take(epoch);
        processed_entries = processed_entries.saturating_add(bucket.len() as u32);
        for touch in bucket {
          let Some(enactment_epoch) =
            ProposalPendingEnactmentAt::<T>::get(touch.domain, touch.item_id)
          else {
            continue;
          };
          if enactment_epoch != epoch {
            continue;
          }
          let Some(outcome) = FinalizedProposalOutcomes::<T>::get(touch.domain, touch.item_id)
          else {
            ProposalPendingEnactmentAt::<T>::remove(touch.domain, touch.item_id);
            continue;
          };
          let (approved_epoch, winner_count) = match outcome {
            FinalizedProposalOutcome::Resolved {
              epoch,
              winner_count,
            } => (epoch, winner_count),
            _ => {
              ProposalPendingEnactmentAt::<T>::remove(touch.domain, touch.item_id);
              continue;
            }
          };
          let execution_attempt = Self::maybe_execute_proposal_payload(
            touch.domain,
            touch.item_id,
            approved_epoch,
            winner_count,
            current_epoch,
          );
          if execution_attempt.is_err()
            || ProposalPendingEnactmentAt::<T>::contains_key(touch.domain, touch.item_id)
          {
            let next_epoch_u32 = epoch_u32.saturating_add(1);
            let next_epoch: T::Epoch = next_epoch_u32.saturated_into();
            let next_touch = FinalizedProposalTouch {
              domain: touch.domain,
              item_id: touch.item_id,
            };
            let _ = PendingEnactmentBuckets::<T>::try_mutate(
              next_epoch,
              |next_bucket| -> DispatchResult {
                if !next_bucket.contains(&next_touch) {
                  next_bucket
                    .try_push(next_touch.clone())
                    .map_err(|_| Error::<T>::PendingEnactmentBucketFull)?;
                }
                Ok(())
              },
            );
          }
        }
      }
      T::WeightInfo::service_finalized_proposal_outcomes(processed_entries)
    }

    fn service_finalized_proposal_outcomes(
      last_processed_epoch: u32,
      current_epoch: T::Epoch,
    ) -> Weight {
      let current_epoch_u32 = current_epoch.saturated_into::<u32>();
      let mut processed_entries = 0u32;
      for epoch_u32 in last_processed_epoch.saturating_add(1)..=current_epoch_u32 {
        let epoch: T::Epoch = epoch_u32.saturated_into();
        let bucket = FinalizedProposalOutcomeExpiryBuckets::<T>::take(epoch);
        processed_entries = processed_entries.saturating_add(bucket.len() as u32);
        for touch in bucket {
          FinalizedProposalOutcomes::<T>::remove(touch.domain, touch.item_id);
          ProposalExecutionDetails::<T>::remove(touch.domain, touch.item_id);
          ProposalMetadataByItem::<T>::remove(touch.domain, touch.item_id);
          ProposalPendingEnactmentAt::<T>::remove(touch.domain, touch.item_id);
          ProposalWinningPrimaryOptionByItem::<T>::remove(touch.domain, touch.item_id);
          ProposalUrgentAuthorizedAt::<T>::remove(touch.domain, touch.item_id);
        }
      }
      T::WeightInfo::service_finalized_proposal_outcomes(processed_entries)
    }

    fn service_expiring_accounts(last_processed_epoch: u32, current_epoch: T::Epoch) -> Weight {
      let current_epoch_u32 = current_epoch.saturated_into::<u32>();
      let mut processed_entries = 0u32;
      for epoch_u32 in last_processed_epoch.saturating_add(1)..=current_epoch_u32 {
        let epoch: T::Epoch = epoch_u32.saturated_into();
        let bucket = ExpiryBuckets::<T>::take(epoch);
        processed_entries = processed_entries.saturating_add(bucket.len() as u32);
        for touch in bucket {
          let evicted =
            WinningVoteWindows::<T>::mutate_exists(touch.domain, &touch.account, |maybe_window| {
              let Some(window) = maybe_window.as_mut() else {
                return false;
              };
              Self::rotate_window_to(window, current_epoch);
              if window.rolling_sum == 0 {
                *maybe_window = None;
                return true;
              }
              false
            });
          if evicted {
            Self::deposit_event(Event::WinningVoteWindowEvicted {
              domain: touch.domain,
              account: touch.account,
              epoch: current_epoch,
            });
          }
        }
      }
      T::WeightInfo::service_expiring_accounts(processed_entries)
    }
  }

  #[pallet::view_functions]
  impl<T: Config> Pallet<T> {
    pub fn reward_coefficient(domain: T::DomainId, account: T::AccountId) -> FixedU128 {
      Self::do_reward_coefficient(domain, &account)
    }

    pub fn govxp_counters(domain: T::DomainId, account: T::AccountId) -> GovXpCounters {
      Self::do_govxp_counters(domain, &account)
    }

    pub fn recent_finalized_proposals(
      domain: T::DomainId,
    ) -> BoundedVec<
      RecentFinalizedProposal<T::WinningVoteItemId, T::Epoch>,
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

    pub fn proposal_submission_authority(
      domain: T::DomainId,
      payload_kind: ProposalPayloadKind,
    ) -> crate::ProposalSubmissionAuthority {
      T::ProposalSubmissionAuthorityProvider::authority(domain, payload_kind)
    }

    pub fn proposal_opening_fee(
      domain: T::DomainId,
      payload_kind: ProposalPayloadKind,
    ) -> Option<BalanceOf<T>> {
      (T::ProposalSubmissionAuthorityProvider::authority(domain, payload_kind)
        == crate::ProposalSubmissionAuthority::Signed)
        .then(T::ProposalOpeningFee::get)
    }

    pub fn proposal_payload_availability(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<ProposalPayloadAvailability> {
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

    pub fn proposal_execution_detail(
      domain: T::DomainId,
      item_id: T::WinningVoteItemId,
    ) -> Option<crate::ProposalExecutionDetail<T::AccountId, T::DomainId, T::Hash, T::Epoch>> {
      ProposalExecutionDetails::<T>::get(domain, item_id)
    }
  }
}
