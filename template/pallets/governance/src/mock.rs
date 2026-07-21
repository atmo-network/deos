use crate as pallet_governance;
use polkadot_sdk::frame_support::{
  construct_runtime, derive_impl, parameter_types, traits::ConstU16, traits::ConstU32,
  traits::ConstU64,
};
use polkadot_sdk::frame_system::{self, EnsureRoot};
use polkadot_sdk::pallet_balances;
use polkadot_sdk::sp_runtime::{
  BuildStorage,
  testing::H256,
  traits::{BlakeTwo256, IdentityLookup},
};
use std::{cell::RefCell, collections::BTreeMap};

pub type AccountId = u64;
pub type DomainId = u32;
pub type Epoch = u64;
type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
  pub struct Test {
    System: frame_system,
    Balances: pallet_balances,
    Governance: pallet_governance,
  }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
  type Block = Block;
  type AccountId = AccountId;
  type Lookup = IdentityLookup<Self::AccountId>;
  type Hash = H256;
  type Hashing = BlakeTwo256;
  type AccountData = pallet_balances::AccountData<u128>;
}

thread_local! {
  static PROPOSAL_VOTE_WEIGHTS: RefCell<BTreeMap<AccountId, u32>> = RefCell::new(BTreeMap::new());
  static PROPOSAL_VOTE_CONTEXTS: RefCell<Vec<(DomainId, u32, Epoch, Epoch, Epoch, Epoch, AccountId)>> = const { RefCell::new(Vec::new()) };
  static VETO_VOTE_WEIGHTS: RefCell<BTreeMap<AccountId, u64>> = RefCell::new(BTreeMap::new());
  static VETO_TOTAL_ISSUANCE: RefCell<u64> = const { RefCell::new(0) };
  static PAYLOAD_PREIMAGE_STATES: RefCell<BTreeMap<H256, (bool, bool, Option<u32>)>> = RefCell::new(BTreeMap::new());
  static AUTHORIZED_RUNTIME_UPGRADE: RefCell<Option<pallet_governance::AuthorizedRuntimeUpgrade<H256>>> = const { RefCell::new(None) };
  static PAYLOAD_EXECUTOR_ENABLED: RefCell<bool> = const { RefCell::new(false) };
  static PAYLOAD_EXECUTION_RESULTS: RefCell<BTreeMap<H256, bool>> = RefCell::new(BTreeMap::new());
}

pub struct MockEpochProvider;
impl pallet_governance::EpochProvider<Epoch> for MockEpochProvider {
  fn current_epoch() -> Epoch {
    System::block_number()
  }
}

pub struct MockProposalVoteWeightProvider;
impl pallet_governance::ProposalVoteWeightProvider<AccountId, DomainId, u32, Epoch>
  for MockProposalVoteWeightProvider
{
  fn vote_weight(
    domain: DomainId,
    context: &pallet_governance::ProposalVoteContext<u32, Epoch>,
    account: &AccountId,
  ) -> u32 {
    PROPOSAL_VOTE_CONTEXTS.with(|contexts| {
      contexts.borrow_mut().push((
        domain,
        context.item_id,
        context.current_epoch,
        context.submitted_epoch,
        context.maturity_epoch,
        context.vote_epoch,
        *account,
      ));
    });
    PROPOSAL_VOTE_WEIGHTS.with(|weights| *weights.borrow().get(account).unwrap_or(&1))
  }
}

pub struct MockGovernanceDomainPolicyProvider;
impl pallet_governance::GovernanceDomainPolicyProvider<DomainId>
  for MockGovernanceDomainPolicyProvider
{
  fn policy(_domain: DomainId) -> pallet_governance::GovernanceDomainPolicy {
    pallet_governance::GovernanceDomainPolicy {
      ordinary_power_profile: pallet_governance::ProposalVotePowerProfile::DecliningDirectStake,
      protection_power_profile: pallet_governance::ProposalVotePowerProfile::DecliningVetoAsset,
    }
  }
}

pub struct MockProposalTrackPowerProfileProvider;
impl pallet_governance::ProposalTrackPowerProfileProvider<DomainId, u32>
  for MockProposalTrackPowerProfileProvider
{
  fn power_profile(
    domain: DomainId,
    _item_id: u32,
    track: pallet_governance::ProposalTrackFamily,
  ) -> pallet_governance::ProposalVotePowerProfile {
    let policy =
      <MockGovernanceDomainPolicyProvider as pallet_governance::GovernanceDomainPolicyProvider<
        DomainId,
      >>::policy(domain);
    match track {
      pallet_governance::ProposalTrackFamily::Ordinary => policy.ordinary_power_profile,
      pallet_governance::ProposalTrackFamily::Veto => policy.protection_power_profile,
    }
  }
}

pub struct MockProposalPrimaryTrackFamilyProvider;
impl pallet_governance::ProposalPrimaryTrackFamilyProvider<DomainId>
  for MockProposalPrimaryTrackFamilyProvider
{
  fn family(
    domain: DomainId,
    payload_kind: pallet_governance::ProposalPayloadKind,
  ) -> pallet_governance::ProposalPrimaryTrackFamily {
    if domain == 43 && payload_kind == pallet_governance::ProposalPayloadKind::L2TreasurySpend {
      return pallet_governance::ProposalPrimaryTrackFamily::Invoice;
    }
    pallet_governance::ProposalPrimaryTrackFamily::Binary
  }
}

pub struct MockProposalUrgentPolicyProvider;
impl pallet_governance::ProposalUrgentPolicyProvider<DomainId>
  for MockProposalUrgentPolicyProvider
{
  fn is_expeditable(
    domain: DomainId,
    payload_kind: pallet_governance::ProposalPayloadKind,
  ) -> bool {
    domain == 42 && payload_kind == pallet_governance::ProposalPayloadKind::L1RootAction
  }

  fn executes_immediately_on_unanimous_pass(
    domain: DomainId,
    payload_kind: pallet_governance::ProposalPayloadKind,
  ) -> bool {
    domain == 42 && payload_kind == pallet_governance::ProposalPayloadKind::L1RootAction
  }
}

pub struct MockProposalSubmissionAuthorityProvider;
impl pallet_governance::ProposalSubmissionAuthorityProvider<DomainId>
  for MockProposalSubmissionAuthorityProvider
{
  fn authority(
    domain: DomainId,
    payload_kind: pallet_governance::ProposalPayloadKind,
  ) -> pallet_governance::ProposalSubmissionAuthority {
    if domain == 44 && payload_kind == pallet_governance::ProposalPayloadKind::Intent {
      return pallet_governance::ProposalSubmissionAuthority::Signed;
    }
    if domain == 43 && payload_kind == pallet_governance::ProposalPayloadKind::L2SignalToL1 {
      return pallet_governance::ProposalSubmissionAuthority::Signed;
    }
    pallet_governance::ProposalSubmissionAuthority::AdminOnly
  }
}

pub struct MockProposalRuntimeUpgradeAuthorizationProvider;
impl pallet_governance::ProposalRuntimeUpgradeAuthorizationProvider<H256>
  for MockProposalRuntimeUpgradeAuthorizationProvider
{
  fn authorized_upgrade() -> Option<pallet_governance::AuthorizedRuntimeUpgrade<H256>> {
    AUTHORIZED_RUNTIME_UPGRADE.with(|authorization| authorization.borrow().clone())
  }
}

pub struct MockProposalPayloadPreimageNoteCostProvider;
impl pallet_governance::ProposalPayloadPreimageNoteCostProvider<u128>
  for MockProposalPayloadPreimageNoteCostProvider
{
  fn note_cost(payload_len: u32) -> Option<u128> {
    Some(2 + u128::from(payload_len))
  }
}

pub struct MockVetoVotePowerProvider;
impl pallet_governance::VetoVotePowerProvider<AccountId, DomainId, u32, Epoch>
  for MockVetoVotePowerProvider
{
  fn vote_weight(
    _domain: DomainId,
    _context: &pallet_governance::ProposalVoteContext<u32, Epoch>,
    account: &AccountId,
  ) -> u64 {
    VETO_VOTE_WEIGHTS.with(|weights| *weights.borrow().get(account).unwrap_or(&0))
  }

  fn raw_vote_weight(_domain: DomainId, account: &AccountId) -> u64 {
    VETO_VOTE_WEIGHTS.with(|weights| *weights.borrow().get(account).unwrap_or(&0))
  }

  fn total_issuance(_domain: DomainId) -> u64 {
    VETO_TOTAL_ISSUANCE.with(|total| *total.borrow())
  }
}

pub struct MockProposalPayloadPreimageProvider;
impl pallet_governance::ProposalPayloadPreimageProvider<H256>
  for MockProposalPayloadPreimageProvider
{
  fn have_preimage(hash: &H256) -> bool {
    PAYLOAD_PREIMAGE_STATES.with(|states| {
      states
        .borrow()
        .get(hash)
        .map(|state| state.0)
        .unwrap_or(false)
    })
  }

  fn preimage_requested(hash: &H256) -> bool {
    PAYLOAD_PREIMAGE_STATES.with(|states| {
      states
        .borrow()
        .get(hash)
        .map(|state| state.1)
        .unwrap_or(false)
    })
  }

  fn preimage_len(hash: &H256) -> Option<u32> {
    PAYLOAD_PREIMAGE_STATES.with(|states| states.borrow().get(hash).and_then(|state| state.2))
  }
}

pub struct MockProposalPayloadExecutor;
impl pallet_governance::ProposalPayloadExecutor<AccountId, DomainId, u32, H256>
  for MockProposalPayloadExecutor
{
  fn can_execute(payload_kind: pallet_governance::ProposalPayloadKind) -> bool {
    PAYLOAD_EXECUTOR_ENABLED.with(|enabled| {
      *enabled.borrow() && payload_kind == pallet_governance::ProposalPayloadKind::L1RootAction
    })
  }

  fn execute(
    _domain: DomainId,
    _item_id: u32,
    _payload_kind: pallet_governance::ProposalPayloadKind,
    payload_hash: H256,
  ) -> Result<
    pallet_governance::ProposalExecutionReceipt<AccountId, DomainId, H256>,
    pallet_governance::ProposalExecutionFailureReason,
  > {
    let success = PAYLOAD_EXECUTION_RESULTS
      .with(|results| results.borrow().get(&payload_hash).copied().unwrap_or(true));
    if success {
      Ok(pallet_governance::ProposalExecutionReceipt::Generic)
    } else {
      Err(pallet_governance::ProposalExecutionFailureReason::DispatchFailed)
    }
  }
}

parameter_types! {
  pub const ExistentialDeposit: u128 = 1;
  pub const ProposalOpeningFee: u128 = 10;
  pub const ProposalFeeRecipient: AccountId = 99;
  pub ProposalApprovalThreshold: polkadot_sdk::sp_runtime::Perbill =
    polkadot_sdk::sp_runtime::Perbill::from_percent(60);
  pub ProposalFastTrackPassThreshold: polkadot_sdk::sp_runtime::Perbill =
    polkadot_sdk::sp_runtime::Perbill::from_percent(50);
  pub ProposalVetoThreshold: polkadot_sdk::sp_runtime::Perbill =
    polkadot_sdk::sp_runtime::Perbill::from_percent(50);
  pub ProposalVetoMinimumVetoTurnout: polkadot_sdk::sp_runtime::Perbill =
    polkadot_sdk::sp_runtime::Perbill::from_percent(1);
  pub static ProposalLeadInPeriod: Epoch = 0;
  pub static ProposalProtectionPeriod: Epoch = 2;
  pub static ProposalUrgentVotingPeriod: Epoch = 1;
  pub static ProposalEnactmentDelay: Epoch = 0;
}

impl pallet_balances::Config for Test {
  type Balance = u128;
  type RuntimeEvent = RuntimeEvent;
  type DustRemoval = ();
  type ExistentialDeposit = ExistentialDeposit;
  type AccountStore = System;
  type WeightInfo = ();
  type MaxLocks = ConstU32<0>;
  type MaxReserves = ConstU32<0>;
  type ReserveIdentifier = [u8; 8];
  type FreezeIdentifier = ();
  type MaxFreezes = ConstU32<0>;
  type RuntimeHoldReason = ();
  type RuntimeFreezeReason = ();
  type DoneSlashHandler = ();
}

impl pallet_governance::Config for Test {
  type AdminOrigin = EnsureRoot<AccountId>;
  type Currency = Balances;
  type ProposalOpeningFee = ProposalOpeningFee;
  type ProposalFeeRecipient = ProposalFeeRecipient;
  type DomainId = DomainId;
  type WinningVoteItemId = u32;
  type Epoch = Epoch;
  type EpochProvider = MockEpochProvider;
  type WinningVoteLookbackEpochs = ConstU32<3>;
  type MaxWinningVotesPerEpoch = ConstU16<2>;
  type MaxWinningVoteItemsPerEpoch = ConstU32<2>;
  type MaxWinningVoteResolutionItemsPerEpoch = ConstU32<16>;
  type MaxWinningVoteAccountsPerCall = ConstU32<256>;
  type MaxActiveProposalsPerDomain = ConstU32<16>;
  type MaxMaturingProposalsPerEpoch = ConstU32<4>;
  type MaxPendingEnactmentsPerEpoch = ConstU32<4>;
  type ProposalVotingPeriod = ConstU64<2>;
  type ProposalLeadInPeriod = ProposalLeadInPeriod;
  type ProposalProtectionPeriod = ProposalProtectionPeriod;
  type ProposalUrgentVotingPeriod = ProposalUrgentVotingPeriod;
  type ProposalEnactmentDelay = ProposalEnactmentDelay;
  type ProposalFastTrackPassThreshold = ProposalFastTrackPassThreshold;
  type ProposalApprovalThreshold = ProposalApprovalThreshold;
  type ProposalApprovalCeiling = ProposalApprovalThreshold;
  type ProposalVetoThreshold = ProposalVetoThreshold;
  type ProposalVetoMinimumVetoTurnout = ProposalVetoMinimumVetoTurnout;
  type ProposalMinimumTurnout = ConstU64<2>;
  type ProposalTurnoutCeiling = ConstU64<2>;
  type ProposalConfirmPeriod = ConstU64<0>;
  type FinalizedProposalOutcomeRetentionEpochs = ConstU32<3>;
  type MaxFinalizedProposalOutcomesPerEpoch = ConstU32<1024>;
  type MaxRecentFinalizedProposalsPerDomain = ConstU32<3072>;
  type MaxExpiringAccountsPerEpoch = ConstU32<1024>;
  type ProposalVoteWeightProvider = MockProposalVoteWeightProvider;
  type GovernanceDomainPolicyProvider = MockGovernanceDomainPolicyProvider;
  type ProposalTrackPowerProfileProvider = MockProposalTrackPowerProfileProvider;
  type ProposalPrimaryTrackFamilyProvider = MockProposalPrimaryTrackFamilyProvider;
  type ProposalUrgentPolicyProvider = MockProposalUrgentPolicyProvider;
  type ProposalSubmissionAuthorityProvider = MockProposalSubmissionAuthorityProvider;
  type ProposalRuntimeUpgradeAuthorizationProvider =
    MockProposalRuntimeUpgradeAuthorizationProvider;
  type ProposalPayloadPreimageNoteCostProvider = MockProposalPayloadPreimageNoteCostProvider;
  type VetoVotePowerProvider = MockVetoVotePowerProvider;
  type ProposalPayloadPreimageProvider = MockProposalPayloadPreimageProvider;
  type ProposalPayloadExecutor = MockProposalPayloadExecutor;
  type WinningVoteRewardTouchHandler = ();
  type WeightInfo = ();
}

pub fn set_vote_weight(account: AccountId, weight: u32) {
  PROPOSAL_VOTE_WEIGHTS.with(|weights| {
    weights.borrow_mut().insert(account, weight);
  });
}

pub fn take_vote_weight_contexts() -> Vec<(DomainId, u32, Epoch, Epoch, Epoch, Epoch, AccountId)> {
  PROPOSAL_VOTE_CONTEXTS.with(|contexts| std::mem::take(&mut *contexts.borrow_mut()))
}

pub fn set_veto_vote_weight(account: AccountId, weight: u64) {
  VETO_VOTE_WEIGHTS.with(|weights| {
    weights.borrow_mut().insert(account, weight);
  });
}

pub fn set_veto_total_issuance(total_issuance: u64) {
  VETO_TOTAL_ISSUANCE.with(|total| {
    *total.borrow_mut() = total_issuance;
  });
}

pub fn set_authorized_runtime_upgrade(
  authorization: Option<pallet_governance::AuthorizedRuntimeUpgrade<H256>>,
) {
  AUTHORIZED_RUNTIME_UPGRADE.with(|value| *value.borrow_mut() = authorization);
}

pub fn set_payload_preimage_state(hash: H256, have_preimage: bool, preimage_requested: bool) {
  set_payload_preimage_state_with_len(
    hash,
    have_preimage,
    preimage_requested,
    have_preimage.then_some(32),
  );
}

pub fn set_payload_preimage_state_with_len(
  hash: H256,
  have_preimage: bool,
  preimage_requested: bool,
  payload_len: Option<u32>,
) {
  PAYLOAD_PREIMAGE_STATES.with(|states| {
    states
      .borrow_mut()
      .insert(hash, (have_preimage, preimage_requested, payload_len));
  });
}

pub fn set_payload_executor_enabled(enabled: bool) {
  PAYLOAD_EXECUTOR_ENABLED.with(|value| *value.borrow_mut() = enabled);
}

pub fn set_payload_execution_result(hash: H256, success: bool) {
  PAYLOAD_EXECUTION_RESULTS.with(|results| {
    results.borrow_mut().insert(hash, success);
  });
}

pub fn new_test_ext() -> polkadot_sdk::sp_io::TestExternalities {
  let mut storage = frame_system::GenesisConfig::<Test>::default()
    .build_storage()
    .unwrap();
  pallet_balances::GenesisConfig::<Test> {
    balances: (1u64..=128u64)
      .map(|account| (account, 1_000u128))
      .collect(),
    dev_accounts: None,
  }
  .assimilate_storage(&mut storage)
  .unwrap();
  let mut ext: polkadot_sdk::sp_io::TestExternalities = storage.into();
  ext.execute_with(|| {
    PROPOSAL_VOTE_WEIGHTS.with(|weights| weights.borrow_mut().clear());
    PROPOSAL_VOTE_CONTEXTS.with(|contexts| contexts.borrow_mut().clear());
    VETO_VOTE_WEIGHTS.with(|weights| weights.borrow_mut().clear());
    VETO_TOTAL_ISSUANCE.with(|total| *total.borrow_mut() = 0);
    PAYLOAD_PREIMAGE_STATES.with(|states| states.borrow_mut().clear());
    AUTHORIZED_RUNTIME_UPGRADE.with(|authorization| authorization.borrow_mut().take());
    PAYLOAD_EXECUTOR_ENABLED.with(|enabled| *enabled.borrow_mut() = false);
    PAYLOAD_EXECUTION_RESULTS.with(|results| results.borrow_mut().clear());
    ProposalLeadInPeriod::set(0);
    ProposalProtectionPeriod::set(2);
    ProposalUrgentVotingPeriod::set(1);
    ProposalEnactmentDelay::set(0);
    System::set_block_number(1);
  });
  ext
}
