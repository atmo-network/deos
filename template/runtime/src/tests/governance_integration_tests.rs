use super::common::{
  ALICE, BOB, actor_fee_sink_account, create_test_asset, mint_tokens, new_test_ext,
};
use crate::configs::governance_config::{
  RuntimeProposalPayloadExecutor, StrategicRuntimeUpgradePayload, TacticalTreasuryFundingSource,
  TacticalTreasuryInvoicePayload,
};
use crate::{
  Actors, Address, Assets, Balances, DeosRouter, Executive, Governance, Preimage, Runtime,
  RuntimeCall, RuntimeEvent, RuntimeOrigin, Signature, Staking, System, TxExtension,
  UncheckedExtrinsic,
};
use codec::{Encode, MaxEncodedLen};
use pallet_governance::ProposalPayloadExecutor;
use polkadot_sdk::frame_support::traits::{
  fungibles::{Inspect as FungiblesInspect, Mutate as FungiblesMutate},
  tokens::Preservation,
};
use polkadot_sdk::frame_support::{assert_noop, assert_ok};
use polkadot_sdk::frame_support::{
  dispatch::GetDispatchInfo,
  traits::{Currency, GetStorageVersion, Hooks, StorageVersion},
};
use polkadot_sdk::frame_system;
use polkadot_sdk::sp_core::{
  Pair, sr25519,
  traits::{ReadRuntimeVersion, ReadRuntimeVersionExt},
};
use polkadot_sdk::sp_externalities::Externalities;
use polkadot_sdk::sp_runtime::{
  MultiAddress, generic, traits::Hash as _, transaction_validity::TransactionSource,
};

struct RejectRuntimeVersionRead;
impl ReadRuntimeVersion for RejectRuntimeVersionRead {
  fn read_runtime_version(
    &self,
    _wasm_code: &[u8],
    _ext: &mut dyn Externalities,
  ) -> Result<Vec<u8>, String> {
    Err("invalid runtime code".into())
  }
}

const PROTOCOL_GOVERNANCE_DOMAIN: u32 = 0;
const TACTICAL_GOVERNANCE_DOMAIN: u32 = primitives::ecosystem::protocol_tokens::BLDR_ASSET_ID;

#[test]
fn governance_0_7_22_storage_schema_is_a_fresh_genesis_baseline() {
  new_test_ext().execute_with(|| {
    let baseline = StorageVersion::new(4);
    assert_eq!(Governance::in_code_storage_version(), baseline);
    assert_eq!(Governance::on_chain_storage_version(), baseline);
  });
}

fn approved_outcome(
  approved_epoch: u32,
  winner_count: u32,
  enactment: pallet_governance::ProposalEnactmentOutcome<u32>,
) -> pallet_governance::FinalizedProposalOutcome<u32> {
  pallet_governance::FinalizedProposalOutcome::Approved {
    approval: pallet_governance::ProposalApproval {
      approved_epoch,
      winner_count,
    },
    enactment,
  }
}

fn submit_root_action_proposal(item_id: u32, payload_hash: crate::Hash) {
  assert_ok!(Governance::submit_proposal(
    RuntimeOrigin::root(),
    PROTOCOL_GOVERNANCE_DOMAIN,
    item_id,
    ALICE,
    pallet_governance::ProposalCadenceMode::Ordinary,
    pallet_governance::ProposalPayloadKind::L1RootAction,
    payload_hash,
  ));
}

fn note_advisory_preimage(signer: crate::AccountId, summary: &[u8]) -> crate::Hash {
  let payload = (
    Option::<crate::Hash>::None,
    summary.to_vec(),
    Option::<Vec<u8>>::None,
  )
    .encode();
  let hash = <Runtime as frame_system::Config>::Hashing::hash(&payload);
  assert_ok!(Preimage::note_preimage(
    RuntimeOrigin::signed(signer),
    payload
  ));
  hash
}

fn note_treasury_preimage(signer: crate::AccountId) -> crate::Hash {
  let payload = TacticalTreasuryInvoicePayload {
    beneficiary: signer.clone(),
    payout_asset: TACTICAL_GOVERNANCE_DOMAIN,
    base_amount: 1,
    funding_source: TacticalTreasuryFundingSource::BldrTreasury,
  }
  .encode();
  let hash = <Runtime as frame_system::Config>::Hashing::hash(&payload);
  assert_ok!(Preimage::note_preimage(
    RuntimeOrigin::signed(signer),
    payload
  ));
  hash
}

fn prepare_payload_witness(
  signer: crate::AccountId,
  domain: u32,
  payload_kind: pallet_governance::ProposalPayloadKind,
  payload_hash: crate::Hash,
) {
  use polkadot_sdk::frame_support::traits::PreimageProvider;
  let payload = <Preimage as PreimageProvider<crate::Hash>>::get_preimage(&payload_hash)
    .expect("witness preparation requires the exact noted payload bytes");
  let payload = pallet_governance::ProposalPayloadBytesOf::<Runtime>::try_from(payload)
    .expect("runtime test payload must fit the Governance call bound");
  assert_ok!(Governance::prepare_payload_admission_witness(
    RuntimeOrigin::signed(signer),
    domain,
    payload_kind,
    payload_hash,
    payload,
  ));
}

fn signed_extrinsic(
  signer: &sr25519::Pair,
  nonce: crate::Nonce,
  call: RuntimeCall,
) -> UncheckedExtrinsic {
  let now = System::block_number();
  if !Actors::block_resource_state().is_some_and(|state| state.ensure_block(now).is_ok()) {
    let _ = Actors::on_initialize(now);
  }
  let tx_ext = TxExtension::new((
    polkadot_sdk::frame_system::AuthorizeCall::<Runtime>::new(),
    polkadot_sdk::frame_system::CheckNonZeroSender::<Runtime>::new(),
    polkadot_sdk::frame_system::CheckSpecVersion::<Runtime>::new(),
    polkadot_sdk::frame_system::CheckTxVersion::<Runtime>::new(),
    polkadot_sdk::frame_system::CheckGenesis::<Runtime>::new(),
    polkadot_sdk::frame_system::CheckEra::<Runtime>::from(generic::Era::Immortal),
    polkadot_sdk::frame_system::CheckNonce::<Runtime>::from(nonce),
    polkadot_sdk::frame_system::CheckWeight::<Runtime>::new(),
    (
      crate::configs::resource_meter::BlockResourceMeterExtension,
      crate::configs::address_event_ingress::AddressEventIngressExtension,
    ),
    polkadot_sdk::pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(0),
    polkadot_sdk::frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
  ));
  let payload =
    generic::SignedPayload::new(call.clone(), tx_ext.clone()).expect("signed payload must encode");
  let signature = payload.using_encoded(|encoded| signer.sign(encoded));
  UncheckedExtrinsic::new_signed(
    call,
    Address::Id(crate::AccountId::from(signer.public())),
    Signature::Sr25519(signature),
    tx_ext,
  )
}

fn service_pending_enactment(domain: u32, item_id: u32) {
  let enactment_epoch =
    pallet_governance::ProposalPendingEnactmentAt::<Runtime>::get(domain, item_id)
      .expect("proposal must schedule pending enactment");
  pallet_governance::LastProcessedEpoch::<Runtime>::put(enactment_epoch.saturating_sub(1));
  System::set_block_number(enactment_epoch);
  Governance::on_initialize(enactment_epoch);
}

fn ordinary_primary_open_epoch() -> u32 {
  crate::configs::governance_config::ProposalLeadInPeriod::get().saturating_add(1)
}

fn ordinary_enactment_epoch(approved_epoch: u32) -> u32 {
  approved_epoch.saturating_add(crate::configs::governance_config::ProposalEnactmentDelay::get())
}

fn advance_to_primary_open() -> u32 {
  let primary_open_epoch = ordinary_primary_open_epoch();
  System::set_block_number(primary_open_epoch);
  primary_open_epoch
}

fn resolve_root_action_proposal(item_id: u32) {
  let winners = polkadot_sdk::frame_support::BoundedVec::try_from(vec![ALICE])
    .expect("proposal winners must fit runtime bound");
  assert_ok!(Governance::resolve_proposal(
    RuntimeOrigin::root(),
    PROTOCOL_GOVERNANCE_DOMAIN,
    item_id,
    winners,
  ));
  service_pending_enactment(PROTOCOL_GOVERNANCE_DOMAIN, item_id);
}

#[test]
fn governance_epoch_catch_up_services_one_chronological_epoch_per_runtime_block() {
  new_test_ext().execute_with(|| {
    type GovernanceWeights = crate::weights::pallet_governance::SubstrateWeight<Runtime>;
    let one_empty_epoch =
      <GovernanceWeights as pallet_governance::WeightInfo>::service_epoch_catch_up();
    let maximum_block = crate::configs::RuntimeBlockWeights::get().max_block;
    for branch in [
      one_empty_epoch.saturating_add(
        <GovernanceWeights as pallet_governance::WeightInfo>::service_maturing_proposals(3),
      ),
      one_empty_epoch.saturating_add(
        <GovernanceWeights as pallet_governance::WeightInfo>::service_pending_enactments(4),
      ),
      one_empty_epoch.saturating_add(
        <GovernanceWeights as pallet_governance::WeightInfo>::service_finalized_proposal_outcomes(
          1024,
        ),
      ),
      one_empty_epoch.saturating_add(
        <GovernanceWeights as pallet_governance::WeightInfo>::service_expiring_accounts(512),
      ),
    ] {
      assert!(
        branch.all_lte(maximum_block),
        "governance epoch service branch {branch:?} exceeds {maximum_block:?}"
      );
    }

    System::set_block_number(20);
    Governance::on_initialize(20);
    assert_eq!(pallet_governance::LastProcessedEpoch::<Runtime>::get(), 1);
    for expected in 2..=20 {
      Governance::on_initialize(20);
      assert_eq!(
        pallet_governance::LastProcessedEpoch::<Runtime>::get(),
        expected
      );
    }
  });
}

#[test]
fn governance_payload_execution_authority_inventory_is_closed() {
  new_test_ext().execute_with(|| {
    let cases = [
      (
        PROTOCOL_GOVERNANCE_DOMAIN,
        90,
        pallet_governance::ProposalPayloadKind::L1RootAction,
        pallet_governance::ProposalExecutionAuthority::Root,
      ),
      (
        TACTICAL_GOVERNANCE_DOMAIN,
        91,
        pallet_governance::ProposalPayloadKind::L2TreasurySpend,
        pallet_governance::ProposalExecutionAuthority::DomainTreasury,
      ),
      (
        TACTICAL_GOVERNANCE_DOMAIN,
        92,
        pallet_governance::ProposalPayloadKind::L2ParameterChange,
        pallet_governance::ProposalExecutionAuthority::DomainParameters,
      ),
      (
        PROTOCOL_GOVERNANCE_DOMAIN,
        93,
        pallet_governance::ProposalPayloadKind::Intent,
        pallet_governance::ProposalExecutionAuthority::NonExecutable,
      ),
      (
        TACTICAL_GOVERNANCE_DOMAIN,
        94,
        pallet_governance::ProposalPayloadKind::L2SignalToL1,
        pallet_governance::ProposalExecutionAuthority::NonExecutable,
      ),
    ];
    for (domain, item_id, payload_kind, authority) in cases {
      System::set_block_number(item_id - 89);
      assert_ok!(Governance::submit_proposal(
        RuntimeOrigin::root(),
        domain,
        item_id,
        ALICE,
        pallet_governance::ProposalCadenceMode::Ordinary,
        payload_kind,
        crate::Hash::repeat_byte(item_id as u8),
      ));
      assert_eq!(
        Governance::proposal_execution_authority(domain, item_id),
        Some(authority),
      );
    }
  });
}

#[test]
fn l1_root_action_authorize_upgrade_executes_from_governance_preimage() {
  new_test_ext().execute_with(|| {
    let approved_epoch = 1;
    let executed_epoch = ordinary_enactment_epoch(approved_epoch);
    let code_hash = crate::Hash::repeat_byte(7);
    assert_noop!(
      crate::System::authorize_upgrade(RuntimeOrigin::signed(ALICE), code_hash),
      polkadot_sdk::sp_runtime::DispatchError::BadOrigin
    );
    let payload = StrategicRuntimeUpgradePayload { code_hash };
    let encoded_payload = payload.encode();
    let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&encoded_payload);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      encoded_payload,
    ));
    submit_root_action_proposal(100, payload_hash);
    resolve_root_action_proposal(100);
    assert_eq!(
      Governance::finalized_proposal_outcome(PROTOCOL_GOVERNANCE_DOMAIN, 100),
      Some(approved_outcome(
        approved_epoch,
        1,
        pallet_governance::ProposalEnactmentOutcome::Enacted {
          epoch: executed_epoch
        },
      ))
    );
    let authorized_upgrade = crate::System::authorized_upgrade();
    assert!(authorized_upgrade.is_some());
    assert_eq!(authorized_upgrade.unwrap().code_hash(), &code_hash);
    assert_eq!(
      Governance::authorized_runtime_upgrade(),
      Some(pallet_governance::AuthorizedRuntimeUpgrade {
        code_hash,
        check_version: true,
      })
    );
    assert_eq!(
      Governance::proposal_execution_detail(PROTOCOL_GOVERNANCE_DOMAIN, 100),
      Some(pallet_governance::ProposalExecutionDetail::Succeeded(
        pallet_governance::ProposalExecutionSuccessDetail::RuntimeUpgradeAuthorized { code_hash },
      ))
    );
    assert!(System::events().iter().any(|record| {
      record.event
        == RuntimeEvent::Governance(pallet_governance::Event::ProposalExecuted {
          domain: PROTOCOL_GOVERNANCE_DOMAIN,
          item_id: 100,
          approved_epoch,
          executed_epoch,
          authority: pallet_governance::ProposalExecutionAuthority::Root,
          payload_kind: pallet_governance::ProposalPayloadKind::L1RootAction,
        })
    }));
    assert!(System::events().iter().any(|record| {
      record.event
        == RuntimeEvent::Governance(pallet_governance::Event::ProposalRuntimeUpgradeAuthorized {
          domain: PROTOCOL_GOVERNANCE_DOMAIN,
          item_id: 100,
          approved_epoch,
          executed_epoch,
          code_hash,
        })
    }));
  });
}

#[test]
fn signed_witnessed_enactment_executes_only_the_bytes_committed_by_proposal_hash() {
  new_test_ext().execute_with(|| {
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), 0, 500));

    let committed_code_hash = crate::Hash::repeat_byte(41);
    let competing_code_hash = crate::Hash::repeat_byte(42);
    let committed_bytes = StrategicRuntimeUpgradePayload {
      code_hash: committed_code_hash,
    }
    .encode();
    let competing_bytes = StrategicRuntimeUpgradePayload {
      code_hash: competing_code_hash,
    }
    .encode();
    let committed_payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&committed_bytes);
    let competing_payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&competing_bytes);
    assert_ne!(committed_payload_hash, competing_payload_hash);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(BOB),
      committed_bytes,
    ));
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(BOB),
      competing_bytes,
    ));
    prepare_payload_witness(
      BOB,
      PROTOCOL_GOVERNANCE_DOMAIN,
      pallet_governance::ProposalPayloadKind::L1RootAction,
      committed_payload_hash,
    );
    assert_ok!(Governance::submit_signed_proposal(
      RuntimeOrigin::signed(BOB),
      PROTOCOL_GOVERNANCE_DOMAIN,
      111,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::L1RootAction,
      committed_payload_hash,
    ));
    assert_eq!(
      Governance::proposal_metadata(PROTOCOL_GOVERNANCE_DOMAIN, 111)
        .expect("signed proposal metadata must exist")
        .payload_hash,
      committed_payload_hash
    );

    resolve_root_action_proposal(111);

    let authorized_upgrade = crate::System::authorized_upgrade()
      .expect("the payload selected by the proposal hash must authorize an upgrade");
    assert_eq!(authorized_upgrade.code_hash(), &committed_code_hash);
    assert_ne!(authorized_upgrade.code_hash(), &competing_code_hash);
  });
}

#[test]
fn strategic_signed_ingress_requires_primary_power_not_veto_power() {
  new_test_ext().execute_with(|| {
    let code_hash = crate::Hash::repeat_byte(29);
    let encoded_payload = StrategicRuntimeUpgradePayload { code_hash }.encode();
    let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&encoded_payload);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      encoded_payload,
    ));
    prepare_payload_witness(
      ALICE,
      PROTOCOL_GOVERNANCE_DOMAIN,
      pallet_governance::ProposalPayloadKind::L1RootAction,
      payload_hash,
    );
    let veto_asset = primitives::ecosystem::protocol_tokens::VETO_ASSET_ID;
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset, &ALICE, 100,
    ));
    let alice_before = Balances::free_balance(ALICE);
    let events_before = System::events();
    assert_noop!(
      Governance::submit_signed_proposal(
        RuntimeOrigin::signed(ALICE),
        PROTOCOL_GOVERNANCE_DOMAIN,
        109,
        pallet_governance::ProposalCadenceMode::Ordinary,
        pallet_governance::ProposalPayloadKind::L1RootAction,
        payload_hash,
      ),
      pallet_governance::Error::<Runtime>::ProposalSubmitterNotPrimaryEligible
    );
    assert_eq!(Balances::free_balance(ALICE), alice_before);
    assert_eq!(System::events(), events_before);
    assert_eq!(
      Governance::active_proposal_count(PROTOCOL_GOVERNANCE_DOMAIN),
      0
    );

    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), 0, 500));
    let bob_before = Balances::free_balance(BOB);
    let fee_sink_before = Balances::free_balance(actor_fee_sink_account());
    assert_ok!(Governance::submit_signed_proposal(
      RuntimeOrigin::signed(BOB),
      PROTOCOL_GOVERNANCE_DOMAIN,
      109,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::L1RootAction,
      payload_hash,
    ));
    assert_eq!(
      Governance::proposal_author(PROTOCOL_GOVERNANCE_DOMAIN, 109),
      Some(BOB)
    );
    assert_eq!(
      Balances::free_balance(BOB),
      bob_before.saturating_sub(crate::configs::governance_config::ProposalOpeningFee::get())
    );
    assert_eq!(
      Balances::free_balance(actor_fee_sink_account()),
      fee_sink_before.saturating_add(crate::configs::governance_config::ProposalOpeningFee::get())
    );
  });
}

#[test]
fn signed_l1_root_action_survives_saturated_general_capacity_and_releases_reserve() {
  new_test_ext().execute_with(|| {
    let general_limit = crate::configs::governance_config::MaxActiveProposalsPerDomain::get()
      .saturating_sub(crate::configs::governance_config::StrategicProposalReserve::get());
    let author_limit = crate::configs::governance_config::MaxActiveProposalsPerAuthor::get();
    for item_id in 0..general_limit {
      if item_id > 0 && item_id % 4 == 0 {
        System::set_block_number(System::block_number().saturating_add(1));
      }
      let author_seed = 10u8.saturating_add((item_id / author_limit) as u8);
      assert_ok!(Governance::submit_proposal(
        RuntimeOrigin::root(),
        PROTOCOL_GOVERNANCE_DOMAIN,
        item_id,
        crate::AccountId::new([author_seed; 32]),
        pallet_governance::ProposalCadenceMode::Ordinary,
        pallet_governance::ProposalPayloadKind::Intent,
        crate::Hash::repeat_byte(item_id as u8),
      ));
    }
    assert_eq!(
      Governance::active_proposal_count(PROTOCOL_GOVERNANCE_DOMAIN),
      general_limit
    );

    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), 0, 500));
    let code_hash = crate::Hash::repeat_byte(247);
    let encoded_payload = StrategicRuntimeUpgradePayload { code_hash }.encode();
    let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&encoded_payload);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      encoded_payload,
    ));
    prepare_payload_witness(
      ALICE,
      PROTOCOL_GOVERNANCE_DOMAIN,
      pallet_governance::ProposalPayloadKind::L1RootAction,
      payload_hash,
    );
    let bob_before = Balances::free_balance(BOB);
    let fee_sink_before = Balances::free_balance(actor_fee_sink_account());
    assert_ok!(Governance::submit_signed_proposal(
      RuntimeOrigin::signed(BOB),
      PROTOCOL_GOVERNANCE_DOMAIN,
      1_000,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::L1RootAction,
      payload_hash,
    ));
    assert_eq!(
      Governance::active_proposal_count(PROTOCOL_GOVERNANCE_DOMAIN),
      general_limit.saturating_add(1)
    );
    assert_eq!(
      Balances::free_balance(BOB),
      bob_before.saturating_sub(crate::configs::governance_config::ProposalOpeningFee::get())
    );
    assert_eq!(
      Balances::free_balance(actor_fee_sink_account()),
      fee_sink_before.saturating_add(crate::configs::governance_config::ProposalOpeningFee::get())
    );

    resolve_root_action_proposal(1_000);
    assert_eq!(
      Governance::active_proposal_count(PROTOCOL_GOVERNANCE_DOMAIN),
      general_limit
    );
    assert_eq!(
      Governance::authorized_runtime_upgrade(),
      Some(pallet_governance::AuthorizedRuntimeUpgrade {
        code_hash,
        check_version: true,
      })
    );
    assert!(matches!(
      Governance::finalized_proposal_outcome(PROTOCOL_GOVERNANCE_DOMAIN, 1_000),
      Some(pallet_governance::FinalizedProposalOutcome::Approved {
        enactment: pallet_governance::ProposalEnactmentOutcome::Enacted { .. },
        ..
      })
    ));

    System::set_block_number(System::block_number().saturating_add(1));
    prepare_payload_witness(
      ALICE,
      PROTOCOL_GOVERNANCE_DOMAIN,
      pallet_governance::ProposalPayloadKind::L1RootAction,
      payload_hash,
    );
    assert_ok!(Governance::submit_signed_proposal(
      RuntimeOrigin::signed(BOB),
      PROTOCOL_GOVERNANCE_DOMAIN,
      1_001,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::L1RootAction,
      payload_hash,
    ));
    assert_eq!(
      Governance::active_proposal_count(PROTOCOL_GOVERNANCE_DOMAIN),
      general_limit.saturating_add(1)
    );
  });
}

#[test]
fn authorized_runtime_upgrade_can_be_applied_by_external_origin_after_governance_authorization() {
  let mut ext = new_test_ext();
  ext.register_extension(ReadRuntimeVersionExt::new(RejectRuntimeVersionRead));
  ext.execute_with(|| {
    let invalid_code = vec![1u8, 2, 3, 4];
    let code_hash = <Runtime as frame_system::Config>::Hashing::hash(&invalid_code);
    let encoded_payload = StrategicRuntimeUpgradePayload { code_hash }.encode();
    let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&encoded_payload);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      encoded_payload,
    ));
    submit_root_action_proposal(109, payload_hash);
    resolve_root_action_proposal(109);
    assert!(crate::System::authorized_upgrade().is_some());
    assert_ok!(crate::System::apply_authorized_upgrade(
      RuntimeOrigin::signed(BOB),
      invalid_code,
    ));
    assert!(crate::System::authorized_upgrade().is_none());
    assert!(System::events().iter().any(|record| {
      matches!(
        &record.event,
        RuntimeEvent::System(frame_system::Event::RejectedInvalidAuthorizedUpgrade {
          code_hash: rejected_hash,
          ..
        }) if *rejected_hash == code_hash
      )
    }));
  });
}

#[test]
fn l2_parameter_change_updates_router_fee_via_governance_executor() {
  new_test_ext().execute_with(|| {
    let approved_epoch = 1;
    let executed_epoch = ordinary_enactment_epoch(approved_epoch);
    let new_fee = polkadot_sdk::sp_runtime::Perbill::from_percent(1);
    let call: crate::RuntimeCall =
      pallet_deos_router::Call::<Runtime>::update_router_fee { new_fee }.into();
    let encoded_call = call.encode();
    let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&encoded_call);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      encoded_call,
    ));
    assert_ne!(DeosRouter::router_fee(), new_fee);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      PROTOCOL_GOVERNANCE_DOMAIN,
      102,
      ALICE,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::L2ParameterChange,
      payload_hash,
    ));
    resolve_root_action_proposal(102);
    assert_eq!(DeosRouter::router_fee(), new_fee);
    assert_eq!(
      Governance::finalized_proposal_outcome(PROTOCOL_GOVERNANCE_DOMAIN, 102),
      Some(approved_outcome(
        approved_epoch,
        1,
        pallet_governance::ProposalEnactmentOutcome::Enacted {
          epoch: executed_epoch
        },
      ))
    );
    assert!(System::events().iter().any(|record| {
      record.event
        == RuntimeEvent::Governance(pallet_governance::Event::ProposalParameterChangeExecuted {
          domain: PROTOCOL_GOVERNANCE_DOMAIN,
          item_id: 102,
          approved_epoch,
          executed_epoch,
          surface: pallet_governance::ProposalParameterChangeSurface::RouterFee,
        })
    }));
  });
}

#[test]
fn l2_parameter_change_rejects_router_fee_above_runtime_bound() {
  new_test_ext().execute_with(|| {
    let initial_fee = DeosRouter::router_fee();
    let new_fee = polkadot_sdk::sp_runtime::Perbill::from_percent(2);
    let call: crate::RuntimeCall =
      pallet_deos_router::Call::<Runtime>::update_router_fee { new_fee }.into();
    let encoded_call = call.encode();
    let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&encoded_call);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      encoded_call,
    ));
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      PROTOCOL_GOVERNANCE_DOMAIN,
      113,
      ALICE,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::L2ParameterChange,
      payload_hash,
    ));
    resolve_root_action_proposal(113);
    assert_eq!(DeosRouter::router_fee(), initial_fee);
    assert_eq!(
      Governance::proposal_execution_detail(PROTOCOL_GOVERNANCE_DOMAIN, 113),
      Some(pallet_governance::ProposalExecutionDetail::Failed(
        pallet_governance::ProposalExecutionFailureReason::DispatchFailed,
      ))
    );
  });
}

#[test]
fn l2_signal_to_l1_finalizes_with_explicit_advisory_kind() {
  new_test_ext().execute_with(|| {
    let approved_epoch = 1;
    let finalized_epoch = ordinary_enactment_epoch(approved_epoch);
    let payload_hash = crate::Hash::repeat_byte(17);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      TACTICAL_GOVERNANCE_DOMAIN,
      101,
      ALICE,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::L2SignalToL1,
      payload_hash,
    ));
    let winners = polkadot_sdk::frame_support::BoundedVec::try_from(vec![ALICE])
      .expect("proposal winners must fit runtime bound");
    assert_ok!(Governance::resolve_proposal(
      RuntimeOrigin::root(),
      TACTICAL_GOVERNANCE_DOMAIN,
      101,
      winners,
    ));
    service_pending_enactment(TACTICAL_GOVERNANCE_DOMAIN, 101);
    assert_eq!(
      Governance::finalized_proposal_outcome(TACTICAL_GOVERNANCE_DOMAIN, 101),
      Some(approved_outcome(
        approved_epoch,
        1,
        pallet_governance::ProposalEnactmentOutcome::AdvisoryFinalized {
          epoch: finalized_epoch,
        },
      ))
    );
    assert!(System::events().iter().any(|record| {
      record.event
        == RuntimeEvent::Governance(pallet_governance::Event::ProposalAdvisoryFinalized {
          domain: TACTICAL_GOVERNANCE_DOMAIN,
          item_id: 101,
          approved_epoch,
          finalized_epoch,
          payload_kind: pallet_governance::ProposalPayloadKind::L2SignalToL1,
        })
    }));
  });
}

#[test]
fn l2_treasury_spend_transfers_bldr_from_bldr_treasury_account() {
  new_test_ext().execute_with(|| {
    let treasury_account = Actors::sovereign_account_id_system(
      primitives::ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID,
    );
    if !<Assets as FungiblesInspect<_>>::asset_exists(TACTICAL_GOVERNANCE_DOMAIN) {
      assert_ok!(Assets::force_create(
        RuntimeOrigin::root(),
        TACTICAL_GOVERNANCE_DOMAIN,
        MultiAddress::Id(ALICE),
        true,
        1,
      ));
    }
    let spend_amount = 25 * crate::EXISTENTIAL_DEPOSIT;
    assert_ok!(
      <crate::Assets as FungiblesMutate<crate::AccountId>>::mint_into(
        TACTICAL_GOVERNANCE_DOMAIN,
        &treasury_account,
        spend_amount.saturating_mul(2),
      )
    );
    let payload = TacticalTreasuryInvoicePayload {
      beneficiary: BOB,
      payout_asset: TACTICAL_GOVERNANCE_DOMAIN,
      base_amount: spend_amount,
      funding_source: TacticalTreasuryFundingSource::BldrTreasury,
    };
    let encoded_payload = payload.encode();
    let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&encoded_payload);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      encoded_payload,
    ));
    let bob_before = Assets::balance(TACTICAL_GOVERNANCE_DOMAIN, &BOB);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      TACTICAL_GOVERNANCE_DOMAIN,
      103,
      ALICE,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::L2TreasurySpend,
      payload_hash,
    ));
    let approved_epoch = advance_to_primary_open();
    let executed_epoch = ordinary_enactment_epoch(approved_epoch);
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(ALICE),
      TACTICAL_GOVERNANCE_DOMAIN,
      103,
      pallet_governance::ProposalVoteKind::Approve,
    ));
    assert_ok!(Governance::resolve_proposal(
      RuntimeOrigin::root(),
      TACTICAL_GOVERNANCE_DOMAIN,
      103,
      polkadot_sdk::frame_support::BoundedVec::try_from(vec![ALICE])
        .expect("proposal winners must fit runtime bound"),
    ));
    service_pending_enactment(TACTICAL_GOVERNANCE_DOMAIN, 103);
    assert_eq!(
      Governance::finalized_proposal_outcome(TACTICAL_GOVERNANCE_DOMAIN, 103),
      Some(approved_outcome(
        approved_epoch,
        1,
        pallet_governance::ProposalEnactmentOutcome::Enacted {
          epoch: executed_epoch
        },
      ))
    );
    assert_eq!(
      Assets::balance(TACTICAL_GOVERNANCE_DOMAIN, &BOB),
      bob_before.saturating_add(spend_amount)
    );
    assert_eq!(
      Governance::proposal_execution_detail(TACTICAL_GOVERNANCE_DOMAIN, 103),
      Some(pallet_governance::ProposalExecutionDetail::Succeeded(
        pallet_governance::ProposalExecutionSuccessDetail::TreasurySpendExecuted {
          funding_source: treasury_account.clone(),
          beneficiary: BOB,
          payout_asset: TACTICAL_GOVERNANCE_DOMAIN,
          base_amount: spend_amount,
          scalar: pallet_governance::ProposalTreasurySpendScalar::Approve,
          final_amount: spend_amount,
          settlement_kind:
            pallet_governance::ProposalTreasurySpendSettlementKind::InvoiceScalarTransfer,
        },
      ))
    );
    assert_eq!(
      Governance::retained_proposal_winning_primary_option(TACTICAL_GOVERNANCE_DOMAIN, 103,),
      Some(pallet_governance::ProposalPrimaryTrackOption::Approve)
    );
    assert!(System::events().iter().any(|record| {
      record.event
        == RuntimeEvent::Governance(pallet_governance::Event::ProposalTreasurySpendExecuted {
          domain: TACTICAL_GOVERNANCE_DOMAIN,
          item_id: 103,
          approved_epoch,
          executed_epoch,
          funding_source: treasury_account.clone(),
          beneficiary: BOB,
          payout_asset: TACTICAL_GOVERNANCE_DOMAIN,
          base_amount: spend_amount,
          scalar: pallet_governance::ProposalTreasurySpendScalar::Approve,
          final_amount: spend_amount,
          settlement_kind:
            pallet_governance::ProposalTreasurySpendSettlementKind::InvoiceScalarTransfer,
        })
    }));
  });
}

#[test]
fn l2_treasury_spend_transfers_non_bldr_asset_from_same_treasury_account() {
  new_test_ext().execute_with(|| {
    let treasury_account = Actors::sovereign_account_id_system(
      primitives::ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID,
    );
    let foreign_asset = 0x2000_0001u32;
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      foreign_asset,
      MultiAddress::Id(ALICE),
      true,
      1,
    ));
    assert_ok!(
      <crate::Assets as FungiblesMutate<crate::AccountId>>::mint_into(
        foreign_asset,
        &treasury_account,
        100 * crate::EXISTENTIAL_DEPOSIT,
      )
    );
    let payload = TacticalTreasuryInvoicePayload {
      beneficiary: BOB,
      payout_asset: foreign_asset,
      base_amount: 10 * crate::EXISTENTIAL_DEPOSIT,
      funding_source: TacticalTreasuryFundingSource::BldrTreasury,
    };
    let encoded_payload = payload.encode();
    let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&encoded_payload);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      encoded_payload,
    ));
    let bob_before = Assets::balance(foreign_asset, &BOB);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      TACTICAL_GOVERNANCE_DOMAIN,
      104,
      ALICE,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::L2TreasurySpend,
      payload_hash,
    ));
    let approved_epoch = advance_to_primary_open();
    let executed_epoch = ordinary_enactment_epoch(approved_epoch);
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(ALICE),
      TACTICAL_GOVERNANCE_DOMAIN,
      104,
      pallet_governance::ProposalVoteKind::Approve,
    ));
    assert_ok!(Governance::resolve_proposal(
      RuntimeOrigin::root(),
      TACTICAL_GOVERNANCE_DOMAIN,
      104,
      polkadot_sdk::frame_support::BoundedVec::try_from(vec![ALICE])
        .expect("proposal winners must fit runtime bound"),
    ));
    service_pending_enactment(TACTICAL_GOVERNANCE_DOMAIN, 104);
    assert_eq!(
      Governance::finalized_proposal_outcome(TACTICAL_GOVERNANCE_DOMAIN, 104),
      Some(approved_outcome(
        approved_epoch,
        1,
        pallet_governance::ProposalEnactmentOutcome::Enacted {
          epoch: executed_epoch
        },
      ))
    );
    assert_eq!(
      Assets::balance(foreign_asset, &BOB),
      bob_before.saturating_add(10 * crate::EXISTENTIAL_DEPOSIT)
    );
    assert!(System::events().iter().any(|record| {
      record.event
        == RuntimeEvent::Governance(pallet_governance::Event::ProposalTreasurySpendExecuted {
          domain: TACTICAL_GOVERNANCE_DOMAIN,
          item_id: 104,
          approved_epoch,
          executed_epoch,
          funding_source: treasury_account.clone(),
          beneficiary: BOB,
          payout_asset: foreign_asset,
          base_amount: 10 * crate::EXISTENTIAL_DEPOSIT,
          scalar: pallet_governance::ProposalTreasurySpendScalar::Approve,
          final_amount: 10 * crate::EXISTENTIAL_DEPOSIT,
          settlement_kind:
            pallet_governance::ProposalTreasurySpendSettlementKind::InvoiceScalarTransfer,
        })
    }));
  });
}

#[test]
fn l2_treasury_spend_fails_without_winning_primary_option_reason() {
  new_test_ext().execute_with(|| {
    let treasury_account = Actors::sovereign_account_id_system(
      primitives::ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID,
    );
    if !<Assets as FungiblesInspect<_>>::asset_exists(TACTICAL_GOVERNANCE_DOMAIN) {
      assert_ok!(Assets::force_create(
        RuntimeOrigin::root(),
        TACTICAL_GOVERNANCE_DOMAIN,
        MultiAddress::Id(ALICE),
        true,
        1,
      ));
    }
    let spend_amount = 25 * crate::EXISTENTIAL_DEPOSIT;
    assert_ok!(
      <crate::Assets as FungiblesMutate<crate::AccountId>>::mint_into(
        TACTICAL_GOVERNANCE_DOMAIN,
        &treasury_account,
        spend_amount.saturating_mul(2),
      )
    );
    let payload = TacticalTreasuryInvoicePayload {
      beneficiary: BOB,
      payout_asset: TACTICAL_GOVERNANCE_DOMAIN,
      base_amount: spend_amount,
      funding_source: TacticalTreasuryFundingSource::BldrTreasury,
    };
    let encoded_payload = payload.encode();
    let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&encoded_payload);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      encoded_payload,
    ));
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      TACTICAL_GOVERNANCE_DOMAIN,
      108,
      ALICE,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::L2TreasurySpend,
      payload_hash,
    ));
    assert_ok!(Governance::resolve_proposal(
      RuntimeOrigin::root(),
      TACTICAL_GOVERNANCE_DOMAIN,
      108,
      polkadot_sdk::frame_support::BoundedVec::try_from(vec![ALICE])
        .expect("proposal winners must fit runtime bound"),
    ));
    service_pending_enactment(TACTICAL_GOVERNANCE_DOMAIN, 108);
    assert_eq!(
      Governance::finalized_proposal_outcome(TACTICAL_GOVERNANCE_DOMAIN, 108),
      Some(approved_outcome(
        1,
        1,
        pallet_governance::ProposalEnactmentOutcome::ExecutionFailed {
          epoch: ordinary_enactment_epoch(1),
        },
      ))
    );
    assert_eq!(
      Governance::proposal_execution_detail(TACTICAL_GOVERNANCE_DOMAIN, 108),
      Some(pallet_governance::ProposalExecutionDetail::Failed(
        pallet_governance::ProposalExecutionFailureReason::MissingWinningPrimaryOption,
      ))
    );
  });
}

#[test]
fn l1_root_action_fails_with_missing_preimage_reason() {
  new_test_ext().execute_with(|| {
    let failed_epoch = ordinary_enactment_epoch(1);
    let payload_hash = crate::Hash::repeat_byte(29);
    submit_root_action_proposal(108, payload_hash);
    resolve_root_action_proposal(108);
    assert_eq!(
      Governance::finalized_proposal_outcome(PROTOCOL_GOVERNANCE_DOMAIN, 108),
      Some(approved_outcome(
        1,
        1,
        pallet_governance::ProposalEnactmentOutcome::ExecutionFailed {
          epoch: failed_epoch
        },
      ))
    );
    assert_eq!(
      Governance::proposal_execution_detail(PROTOCOL_GOVERNANCE_DOMAIN, 108),
      Some(pallet_governance::ProposalExecutionDetail::Failed(
        pallet_governance::ProposalExecutionFailureReason::MissingPreimage,
      ))
    );
    assert!(System::events().iter().any(|record| {
      record.event
        == RuntimeEvent::Governance(pallet_governance::Event::ProposalExecutionFailed {
          domain: PROTOCOL_GOVERNANCE_DOMAIN,
          item_id: 108,
          approved_epoch: 1,
          failed_epoch,
          authority: pallet_governance::ProposalExecutionAuthority::Root,
          payload_kind: pallet_governance::ProposalPayloadKind::L1RootAction,
          reason: pallet_governance::ProposalExecutionFailureReason::MissingPreimage,
        })
    }));
  });
}

#[test]
fn l1_root_action_rejects_invalid_upgrade_payload_bytes() {
  new_test_ext().execute_with(|| {
    let failed_epoch = ordinary_enactment_epoch(1);
    let encoded_payload = vec![1u8, 2, 3, 4];
    let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&encoded_payload);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      encoded_payload,
    ));
    submit_root_action_proposal(101, payload_hash);
    resolve_root_action_proposal(101);
    assert_eq!(
      Governance::finalized_proposal_outcome(PROTOCOL_GOVERNANCE_DOMAIN, 101),
      Some(approved_outcome(
        1,
        1,
        pallet_governance::ProposalEnactmentOutcome::ExecutionFailed {
          epoch: failed_epoch
        },
      ))
    );
    assert!(System::events().iter().any(|record| {
      record.event
        == RuntimeEvent::Governance(pallet_governance::Event::ProposalExecutionFailed {
          domain: PROTOCOL_GOVERNANCE_DOMAIN,
          item_id: 101,
          approved_epoch: 1,
          failed_epoch,
          authority: pallet_governance::ProposalExecutionAuthority::Root,
          payload_kind: pallet_governance::ProposalPayloadKind::L1RootAction,
          reason: pallet_governance::ProposalExecutionFailureReason::InvalidPreimage,
        })
    }));
  });
}

#[test]
fn runtime_executor_reaches_domain_and_call_failures_from_valid_preimages() {
  new_test_ext().execute_with(|| {
    let upgrade_payload = StrategicRuntimeUpgradePayload {
      code_hash: crate::Hash::repeat_byte(42),
    }
    .encode();
    let upgrade_hash = <Runtime as frame_system::Config>::Hashing::hash(&upgrade_payload);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      upgrade_payload,
    ));
    assert_eq!(
      RuntimeProposalPayloadExecutor::execute(
        TACTICAL_GOVERNANCE_DOMAIN,
        200,
        pallet_governance::ProposalPayloadKind::L1RootAction,
        upgrade_hash,
      )
      .err(),
      Some(pallet_governance::ProposalExecutionFailureReason::UnsupportedDomain)
    );

    let unsupported_call = RuntimeCall::System(frame_system::Call::remark {
      remark: vec![1, 2, 3],
    })
    .encode();
    let call_hash = <Runtime as frame_system::Config>::Hashing::hash(&unsupported_call);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      unsupported_call,
    ));
    assert_eq!(
      RuntimeProposalPayloadExecutor::execute(
        PROTOCOL_GOVERNANCE_DOMAIN,
        201,
        pallet_governance::ProposalPayloadKind::L2ParameterChange,
        call_hash,
      )
      .err(),
      Some(pallet_governance::ProposalExecutionFailureReason::UnsupportedCall)
    );
    assert_eq!(
      RuntimeProposalPayloadExecutor::execute(
        PROTOCOL_GOVERNANCE_DOMAIN,
        202,
        pallet_governance::ProposalPayloadKind::Intent,
        call_hash,
      )
      .err(),
      Some(pallet_governance::ProposalExecutionFailureReason::UnsupportedCall),
      "direct adapter invocation of an advisory payload fails closed without panic"
    );
  });
}

#[test]
fn ordinary_governance_timing_matches_public_cadence_on_current_line() {
  new_test_ext().execute_with(|| {
    submit_root_action_proposal(110, crate::Hash::repeat_byte(12));
    assert_eq!(
      Governance::proposal_timing(PROTOCOL_GOVERNANCE_DOMAIN, 110),
      Some(pallet_governance::ProposalTiming {
        submitted_epoch: 1,
        protection_open_epoch: 1,
        protection_close_epoch: 1 + 7 * 24 * crate::HOURS,
        ordinary_primary_open_epoch: 1 + 3 * 24 * crate::HOURS,
        ordinary_primary_close_epoch: 1 + 10 * 24 * crate::HOURS,
        urgent_primary_open_epoch: None,
        urgent_primary_close_epoch: None,
        effective_primary_open_epoch: 1 + 3 * 24 * crate::HOURS,
        effective_primary_close_epoch: 1 + 10 * 24 * crate::HOURS,
        pending_enactment_epoch: None,
      })
    );
  });
}

#[test]
fn urgent_policy_is_runtime_upgrade_only_on_current_launch_line() {
  new_test_ext().execute_with(|| {
    submit_root_action_proposal(107, crate::Hash::repeat_byte(9));
    assert_eq!(
      Governance::proposal_primary_track_family(PROTOCOL_GOVERNANCE_DOMAIN, 107),
      Some(pallet_governance::ProposalPrimaryTrackFamily::Binary)
    );
    assert_eq!(
      Governance::proposal_urgent_eligibility(PROTOCOL_GOVERNANCE_DOMAIN, 107),
      Some(true)
    );
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      TACTICAL_GOVERNANCE_DOMAIN,
      108,
      ALICE,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::L2TreasurySpend,
      crate::Hash::repeat_byte(10),
    ));
    assert_eq!(
      Governance::proposal_primary_track_family(TACTICAL_GOVERNANCE_DOMAIN, 108),
      Some(pallet_governance::ProposalPrimaryTrackFamily::Invoice)
    );
    assert_eq!(
      Governance::proposal_urgent_eligibility(TACTICAL_GOVERNANCE_DOMAIN, 108),
      Some(false)
    );
  });
}

#[test]
fn submission_authority_opening_fee_and_preimage_cost_status_are_explicit_on_current_launch_line() {
  new_test_ext().execute_with(|| {
    let noted_payload = vec![1u8, 2, 3, 4, 5];
    let noted_payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&noted_payload);
    let requested_payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&[9u8, 9, 9]);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      noted_payload.clone(),
    ));
    assert_ok!(Preimage::request_preimage(
      RuntimeOrigin::root(),
      requested_payload_hash,
    ));
    let protocol_intent = Governance::proposal_admission_policy_view(
      PROTOCOL_GOVERNANCE_DOMAIN,
      pallet_governance::ProposalPayloadKind::Intent,
    );
    assert_eq!(
      protocol_intent.authority,
      pallet_governance::ProposalSubmissionAuthority::PrimaryEligibleSigned
    );
    assert_eq!(
      protocol_intent.opening_fee,
      Some(10 * crate::EXISTENTIAL_DEPOSIT)
    );
    assert_eq!(
      Governance::payload_hash_preimage_status(noted_payload_hash),
      pallet_governance::PayloadHashPreimageStatus {
        have_preimage: true,
        preimage_requested: false,
        payload_len: Some(noted_payload.len() as u32),
      }
    );
    assert_eq!(
      Governance::payload_hash_preimage_status(requested_payload_hash),
      pallet_governance::PayloadHashPreimageStatus {
        have_preimage: false,
        preimage_requested: true,
        payload_len: None,
      }
    );
    assert_eq!(
      Governance::payload_preimage_note_cost(0),
      Some(crate::EXISTENTIAL_DEPOSIT)
    );
    assert_eq!(
      Governance::payload_preimage_note_cost(5),
      Some(crate::EXISTENTIAL_DEPOSIT + 5 * (10 * crate::MICRO_UNIT))
    );
    let protocol_root = Governance::proposal_admission_policy_view(
      PROTOCOL_GOVERNANCE_DOMAIN,
      pallet_governance::ProposalPayloadKind::L1RootAction,
    );
    assert_eq!(
      protocol_root.authority,
      pallet_governance::ProposalSubmissionAuthority::PrimaryEligibleSigned
    );
    assert_eq!(
      protocol_root.opening_fee,
      Some(10 * crate::EXISTENTIAL_DEPOSIT)
    );
    assert_eq!(
      Governance::proposal_admission_policy_view(
        TACTICAL_GOVERNANCE_DOMAIN,
        pallet_governance::ProposalPayloadKind::L1RootAction,
      )
      .authority,
      pallet_governance::ProposalSubmissionAuthority::AdminOnly
    );
    let tactical_signal = Governance::proposal_admission_policy_view(
      TACTICAL_GOVERNANCE_DOMAIN,
      pallet_governance::ProposalPayloadKind::L2SignalToL1,
    );
    assert_eq!(
      tactical_signal.authority,
      pallet_governance::ProposalSubmissionAuthority::Signed
    );
    assert_eq!(
      tactical_signal.opening_fee,
      Some(10 * crate::EXISTENTIAL_DEPOSIT)
    );
    let tactical_treasury = Governance::proposal_admission_policy_view(
      TACTICAL_GOVERNANCE_DOMAIN,
      pallet_governance::ProposalPayloadKind::L2TreasurySpend,
    );
    assert_eq!(
      tactical_treasury.authority,
      pallet_governance::ProposalSubmissionAuthority::Signed
    );
    assert_eq!(
      tactical_treasury.opening_fee,
      Some(10 * crate::EXISTENTIAL_DEPOSIT)
    );
  });
}

#[test]
fn signed_intent_submission_collects_opening_fee_and_records_signer_as_proposer() {
  new_test_ext().execute_with(|| {
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), 0, 500));
    let fee_sink = actor_fee_sink_account();
    let fee_sink_before = Balances::free_balance(&fee_sink);
    let payload_hash = note_advisory_preimage(BOB, b"protocol intent");
    prepare_payload_witness(
      BOB,
      PROTOCOL_GOVERNANCE_DOMAIN,
      pallet_governance::ProposalPayloadKind::Intent,
      payload_hash,
    );
    let balance_before = Balances::free_balance(BOB);
    assert_ok!(Governance::submit_signed_proposal(
      RuntimeOrigin::signed(BOB),
      PROTOCOL_GOVERNANCE_DOMAIN,
      110,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::Intent,
      payload_hash,
    ));
    assert_eq!(
      Governance::proposal_author(PROTOCOL_GOVERNANCE_DOMAIN, 110),
      Some(BOB)
    );
    assert_eq!(
      Balances::free_balance(BOB),
      balance_before
        .saturating_sub(10 * crate::EXISTENTIAL_DEPOSIT)
        .saturating_add(crate::configs::governance_config::PayloadAdmissionWitnessDeposit::get())
    );
    assert!(System::events().iter().any(|record| {
      record.event
        == RuntimeEvent::Governance(pallet_governance::Event::ProposalOpeningFeeCollected {
          domain: PROTOCOL_GOVERNANCE_DOMAIN,
          item_id: 110,
          proposer: BOB,
          amount: 10 * crate::EXISTENTIAL_DEPOSIT,
        })
    }));
    assert_eq!(
      Balances::free_balance(&fee_sink),
      fee_sink_before.saturating_add(10 * crate::EXISTENTIAL_DEPOSIT)
    );
  });
}

#[test]
fn runtime_payload_witness_enforces_exact_advisory_bound_and_domain_contract() {
  new_test_ext().execute_with(|| {
    let maximum_payload = (
      Some(crate::Hash::repeat_byte(1)),
      vec![b'a'; 128],
      Some(vec![b'b'; 96]),
    )
      .encode();
    assert_eq!(maximum_payload.len(), 262);
    let maximum_hash = <Runtime as frame_system::Config>::Hashing::hash(&maximum_payload);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      maximum_payload.clone(),
    ));
    let maximum_payload =
      pallet_governance::ProposalPayloadBytesOf::<Runtime>::try_from(maximum_payload)
        .expect("maximum Governance payload must fit the call bound");
    assert_ok!(Governance::prepare_payload_admission_witness(
      RuntimeOrigin::signed(ALICE),
      PROTOCOL_GOVERNANCE_DOMAIN,
      pallet_governance::ProposalPayloadKind::Intent,
      maximum_hash,
      maximum_payload.clone(),
    ));
    assert_eq!(
      Governance::payload_admission_witness(
        maximum_hash,
        (
          PROTOCOL_GOVERNANCE_DOMAIN,
          pallet_governance::ProposalPayloadKind::Intent,
        ),
      ),
      Some(pallet_governance::ProposalPayloadAdmissionWitness {
        domain: PROTOCOL_GOVERNANCE_DOMAIN,
        payload_kind: pallet_governance::ProposalPayloadKind::Intent,
        payload_len: 262,
        execution_authority: pallet_governance::ProposalExecutionAuthority::NonExecutable,
        compatibility: pallet_governance::ProposalPayloadCompatibility {
          schema_version: 1,
          runtime_spec_version: Some(crate::VERSION.spec_version),
        },
        depositor: ALICE,
        deposit: crate::configs::governance_config::PayloadAdmissionWitnessDeposit::get(),
      })
    );

    let mut different_payload = maximum_payload.clone().into_inner();
    different_payload[35] = b'c';
    let different_hash = <Runtime as frame_system::Config>::Hashing::hash(&different_payload);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      different_payload,
    ));
    assert_noop!(
      Governance::prepare_payload_admission_witness(
        RuntimeOrigin::signed(ALICE),
        PROTOCOL_GOVERNANCE_DOMAIN,
        pallet_governance::ProposalPayloadKind::Intent,
        different_hash,
        maximum_payload.clone(),
      ),
      pallet_governance::Error::<Runtime>::ProposalPreimageInvalid
    );

    let oversized_payload = vec![2u8; StrategicRuntimeUpgradePayload::max_encoded_len() + 1];
    assert_eq!(oversized_payload.len(), 33);
    let oversized_hash = <Runtime as frame_system::Config>::Hashing::hash(&oversized_payload);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      oversized_payload.clone(),
    ));
    let oversized_payload =
      pallet_governance::ProposalPayloadBytesOf::<Runtime>::try_from(oversized_payload)
        .expect("kind-specific oversized payload still fits the global call bound");
    assert_noop!(
      Governance::prepare_payload_admission_witness(
        RuntimeOrigin::signed(ALICE),
        PROTOCOL_GOVERNANCE_DOMAIN,
        pallet_governance::ProposalPayloadKind::L1RootAction,
        oversized_hash,
        oversized_payload,
      ),
      pallet_governance::Error::<Runtime>::ProposalPreimageOversized
    );
    assert_noop!(
      Governance::prepare_payload_admission_witness(
        RuntimeOrigin::signed(ALICE),
        17,
        pallet_governance::ProposalPayloadKind::L1RootAction,
        maximum_hash,
        maximum_payload,
      ),
      pallet_governance::Error::<Runtime>::ProposalPreimageIncompatible
    );
  });
}

#[test]
fn maximum_signed_governance_proposal_passes_real_check_weight_validation() {
  new_test_ext().execute_with(|| {
    let signer_pair = sr25519::Pair::from_seed(&[73u8; 32]);
    let signer = crate::AccountId::from(signer_pair.public());
    let _ = <Balances as Currency<crate::AccountId>>::deposit_creating(
      &signer,
      1_000_000_000_000_000_000,
    );
    assert_ok!(create_test_asset(0, &signer));
    assert_ok!(mint_tokens(0, &signer, &signer, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake(
      RuntimeOrigin::signed(signer.clone()),
      0,
      500,
    ));
    let payload = StrategicRuntimeUpgradePayload {
      code_hash: crate::Hash::repeat_byte(74),
    }
    .encode();
    assert_eq!(
      payload.len(),
      StrategicRuntimeUpgradePayload::max_encoded_len()
    );
    let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&payload);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(signer.clone()),
      payload,
    ));
    prepare_payload_witness(
      signer.clone(),
      PROTOCOL_GOVERNANCE_DOMAIN,
      pallet_governance::ProposalPayloadKind::L1RootAction,
      payload_hash,
    );
    let call = RuntimeCall::Governance(pallet_governance::Call::submit_signed_proposal {
      domain: PROTOCOL_GOVERNANCE_DOMAIN,
      item_id: 113,
      cadence_mode: pallet_governance::ProposalCadenceMode::Ordinary,
      payload_kind: pallet_governance::ProposalPayloadKind::L1RootAction,
      payload_hash,
    });
    let dispatch_info = call.get_dispatch_info();
    let block_weights = crate::configs::RuntimeBlockWeights::get();
    let class_limits = block_weights.get(dispatch_info.class);
    let max_extrinsic = class_limits
      .max_extrinsic
      .expect("signed Governance class must define max_extrinsic");
    assert!(dispatch_info.call_weight.all_lte(max_extrinsic));
    let extrinsic = signed_extrinsic(&signer_pair, 0, call);
    let extrinsic_weight = extrinsic.get_dispatch_info().total_weight();
    assert!(
      extrinsic_weight.all_lte(
        crate::configs::BlockResourceBudgetValue::get()
          .limits()
          .shared_economic()
      ),
      "extrinsic {extrinsic_weight:?} must fit shared {:?}",
      crate::configs::BlockResourceBudgetValue::get()
        .limits()
        .shared_economic()
    );
    let now = System::block_number();
    let validation_block = now.saturating_add(1);
    let mut validation_state = pallet_deos_actors::BlockResourceState::new(validation_block);
    validation_state
      .begin_prepass()
      .expect("validation phase opens");
    validation_state
      .open_external_phase()
      .expect("validation external phase opens");
    pallet_deos_actors::CurrentBlockResourceState::<Runtime>::put(validation_state);
    assert_ok!(Executive::validate_transaction(
      TransactionSource::External,
      extrinsic.clone(),
      System::block_hash(0),
    ));
    pallet_deos_actors::CurrentBlockResourceState::<Runtime>::kill();
    polkadot_sdk::pallet_timestamp::Now::<Runtime>::put(1);
    polkadot_sdk::cumulus_pallet_parachain_system::ValidationData::<Runtime>::put(
      polkadot_sdk::cumulus_primitives_core::PersistedValidationData::default(),
    );
    let _ = Actors::on_initialize(now);
    assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
    assert_ok!(Executive::apply_extrinsic(extrinsic));
    let resource_state =
      Actors::block_resource_state().expect("resource meter remains authoritative");
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert!(
      resource_state.usage().user_dispatch_used()
        != polkadot_sdk::frame_support::weights::Weight::zero()
    );
    assert_eq!(
      resource_state.phase(),
      pallet_deos_actors::BlockResourcePhase::ExternalPhase
    );
    assert_eq!(
      Governance::proposal_author(PROTOCOL_GOVERNANCE_DOMAIN, 113),
      Some(signer),
    );
  });
}

#[test]
fn signed_tactical_l2_signal_submission_collects_opening_fee_and_records_signer() {
  new_test_ext().execute_with(|| {
    let payload_hash = note_advisory_preimage(ALICE, b"tactical signal");
    prepare_payload_witness(
      ALICE,
      TACTICAL_GOVERNANCE_DOMAIN,
      pallet_governance::ProposalPayloadKind::L2SignalToL1,
      payload_hash,
    );
    let balance_before = Balances::free_balance(ALICE);
    assert_ok!(Governance::submit_signed_proposal(
      RuntimeOrigin::signed(ALICE),
      TACTICAL_GOVERNANCE_DOMAIN,
      111,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::L2SignalToL1,
      payload_hash,
    ));
    assert_eq!(
      Governance::proposal_author(TACTICAL_GOVERNANCE_DOMAIN, 111),
      Some(ALICE)
    );
    assert_eq!(
      Balances::free_balance(ALICE),
      balance_before
        .saturating_sub(10 * crate::EXISTENTIAL_DEPOSIT)
        .saturating_add(crate::configs::governance_config::PayloadAdmissionWitnessDeposit::get())
    );
  });
}

#[test]
fn signed_tactical_treasury_submission_collects_opening_fee_and_records_signer() {
  new_test_ext().execute_with(|| {
    let payload_hash = note_treasury_preimage(ALICE);
    prepare_payload_witness(
      ALICE,
      TACTICAL_GOVERNANCE_DOMAIN,
      pallet_governance::ProposalPayloadKind::L2TreasurySpend,
      payload_hash,
    );
    let balance_before = Balances::free_balance(ALICE);
    assert_ok!(Governance::submit_signed_proposal(
      RuntimeOrigin::signed(ALICE),
      TACTICAL_GOVERNANCE_DOMAIN,
      112,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::L2TreasurySpend,
      payload_hash,
    ));
    assert_eq!(
      Governance::proposal_author(TACTICAL_GOVERNANCE_DOMAIN, 112),
      Some(ALICE)
    );
    assert_eq!(
      Balances::free_balance(ALICE),
      balance_before
        .saturating_sub(10 * crate::EXISTENTIAL_DEPOSIT)
        .saturating_add(crate::configs::governance_config::PayloadAdmissionWitnessDeposit::get())
    );
  });
}

#[test]
fn unanimous_veto_pass_executes_runtime_upgrade_immediately() {
  new_test_ext().execute_with(|| {
    let code_hash = crate::Hash::repeat_byte(21);
    let encoded_payload = StrategicRuntimeUpgradePayload { code_hash }.encode();
    let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&encoded_payload);
    let veto_asset = primitives::ecosystem::protocol_tokens::VETO_ASSET_ID;
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      encoded_payload,
    ));
    if !<Assets as FungiblesInspect<_>>::asset_exists(veto_asset) {
      assert_ok!(Assets::force_create(
        RuntimeOrigin::root(),
        veto_asset,
        MultiAddress::Id(ALICE),
        true,
        1,
      ));
    }
    assert_ok!(
      <crate::Assets as FungiblesMutate<crate::AccountId>>::mint_into(veto_asset, &ALICE, 100,)
    );
    submit_root_action_proposal(112, payload_hash);
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(ALICE),
      PROTOCOL_GOVERNANCE_DOMAIN,
      112,
      pallet_governance::ProposalVoteKind::Pass,
    ));
    assert_eq!(
      Governance::finalized_proposal_outcome(PROTOCOL_GOVERNANCE_DOMAIN, 112),
      Some(approved_outcome(
        1,
        0,
        pallet_governance::ProposalEnactmentOutcome::Enacted { epoch: 1 },
      ))
    );
    assert_eq!(
      Governance::authorized_runtime_upgrade(),
      Some(pallet_governance::AuthorizedRuntimeUpgrade {
        code_hash,
        check_version: true,
      })
    );
    assert!(System::events().iter().any(|record| {
      record.event
        == RuntimeEvent::Governance(pallet_governance::Event::ProposalUrgentAuthorized {
          domain: PROTOCOL_GOVERNANCE_DOMAIN,
          item_id: 112,
          authorization_epoch: 1,
          pass_weight: 100,
          total_protection_supply: 100,
        })
    }));
  });
}

#[test]
fn governance_custody_late_lock_and_unlock_failures_restore_exact_root() {
  new_test_ext().execute_with(|| {
    use crate::configs::governance_config::RuntimeVotePowerCustody;
    use crate::configs::governance_config::{
      VotePowerCustodyFault, governance_vote_power_custody_account,
      set_vote_power_custody_fault,
    };

    let veto_asset = primitives::ecosystem::protocol_tokens::VETO_ASSET_ID;
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset, &ALICE, 40,
    ));
    assert_eq!(
      <RuntimeVotePowerCustody as pallet_governance::VotePowerCustody<
        crate::AccountId,
        u32,
        u32,
        crate::Balance,
      >>::target_amount(
        PROTOCOL_GOVERNANCE_DOMAIN,
        pallet_governance::ProposalTrackFamily::Veto,
        &ALICE,
        u128::MAX,
      ),
      Err(polkadot_sdk::sp_runtime::DispatchError::Arithmetic(
        polkadot_sdk::sp_runtime::ArithmeticError::Overflow,
      ))
    );
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset, &BOB, 60,
    ));
    submit_root_action_proposal(125, crate::Hash::repeat_byte(25));
    System::reset_events();
    set_vote_power_custody_fault(Some(VotePowerCustodyFault::LockAfterTransfer));
    let root_before_lock =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_noop!(
      Governance::cast_vote(
        RuntimeOrigin::signed(ALICE),
        PROTOCOL_GOVERNANCE_DOMAIN,
        125,
        pallet_governance::ProposalVoteKind::Veto,
      ),
      polkadot_sdk::sp_runtime::DispatchError::Other(
        "Forced custody lock failure after transfer"
      )
    );
    set_vote_power_custody_fault(None);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before_lock,
      "lock fault restores voter/custody ledgers, ballot sets, locks, participation, events, and proposal state"
    );

    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(ALICE),
      PROTOCOL_GOVERNANCE_DOMAIN,
      125,
      pallet_governance::ProposalVoteKind::Veto,
    ));
    let position = Governance::vote_power_custody(ALICE, veto_asset)
      .expect("successful vote creates custody position");
    System::set_block_number(position.lock_until);
    System::reset_events();
    set_vote_power_custody_fault(Some(VotePowerCustodyFault::UnlockAfterTransfer));
    let root_before_unlock =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_noop!(
      Governance::unlock_vote_power(RuntimeOrigin::signed(ALICE), veto_asset),
      polkadot_sdk::sp_runtime::DispatchError::Other(
        "Forced custody unlock failure after transfer"
      )
    );
    set_vote_power_custody_fault(None);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before_unlock,
      "unlock fault restores voter/custody ledgers, aggregate position, events, and locks"
    );
    assert_eq!(Assets::balance(veto_asset, &ALICE), 0);
    assert_eq!(
      Assets::balance(veto_asset, &governance_vote_power_custody_account()),
      40
    );
  });
}

#[test]
fn transferable_veto_power_is_custodied_reused_and_increased_without_revote_amplification() {
  new_test_ext().execute_with(|| {
    let veto_asset = primitives::ecosystem::protocol_tokens::VETO_ASSET_ID;
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset, &ALICE, 40,
    ));
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset, &BOB, 60,
    ));
    for item_id in 120..=122 {
      submit_root_action_proposal(item_id, crate::Hash::repeat_byte(item_id as u8));
    }

    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(ALICE),
      PROTOCOL_GOVERNANCE_DOMAIN,
      120,
      pallet_governance::ProposalVoteKind::Veto,
    ));
    let custody = crate::configs::governance_config::governance_vote_power_custody_account();
    assert_eq!(Assets::balance(veto_asset, &ALICE), 0);
    assert_eq!(Assets::balance(veto_asset, &custody), 40);
    assert_eq!(
      Governance::account_governance_power_view(PROTOCOL_GOVERNANCE_DOMAIN, 120, ALICE)
        .expect("first proposal remains active")
        .frozen_protection_ballot
        .expect("first veto ballot is frozen")
        .raw_power,
      40
    );

    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(ALICE),
      PROTOCOL_GOVERNANCE_DOMAIN,
      121,
      pallet_governance::ProposalVoteKind::Veto,
    ));
    assert_eq!(Assets::balance(veto_asset, &custody), 40);
    assert_eq!(
      Governance::account_governance_power_view(PROTOCOL_GOVERNANCE_DOMAIN, 121, ALICE)
        .expect("second proposal remains active")
        .frozen_protection_ballot
        .expect("second veto ballot is frozen")
        .raw_power,
      40
    );

    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset, &ALICE, 20,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(ALICE),
      PROTOCOL_GOVERNANCE_DOMAIN,
      122,
      pallet_governance::ProposalVoteKind::Veto,
    ));
    let position = Governance::vote_power_custody(ALICE, veto_asset)
      .expect("transferable vote power remains in one aggregate source position");
    assert_eq!(position.amount, 60);
    assert_eq!(Assets::balance(veto_asset, &ALICE), 0);
    assert_eq!(Assets::balance(veto_asset, &custody), 60);
    assert_noop!(
      Governance::unlock_vote_power(RuntimeOrigin::signed(ALICE), veto_asset),
      pallet_governance::Error::<Runtime>::VotePowerCustodyLockActive
    );

    System::set_block_number(position.lock_until);
    assert_ok!(Governance::unlock_vote_power(
      RuntimeOrigin::signed(ALICE),
      veto_asset,
    ));
    assert!(Governance::vote_power_custody(ALICE, veto_asset).is_none());
    assert_eq!(Assets::balance(veto_asset, &ALICE), 60);
    assert_eq!(Assets::balance(veto_asset, &custody), 0);
  });
}

#[test]
fn veto_pass_replacement_reuses_one_position_and_extends_only_to_the_maximum_horizon() {
  new_test_ext().execute_with(|| {
    let veto_asset = primitives::ecosystem::protocol_tokens::VETO_ASSET_ID;
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset, &ALICE, 40,
    ));
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset, &BOB, 60,
    ));
    submit_root_action_proposal(126, crate::Hash::repeat_byte(26));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(ALICE),
      PROTOCOL_GOVERNANCE_DOMAIN,
      126,
      pallet_governance::ProposalVoteKind::Veto,
    ));
    let first_position = Governance::vote_power_custody(ALICE, veto_asset)
      .expect("first protection ballot creates aggregate custody");
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(ALICE),
      PROTOCOL_GOVERNANCE_DOMAIN,
      126,
      pallet_governance::ProposalVoteKind::Pass,
    ));
    let votes = Governance::proposal_votes(PROTOCOL_GOVERNANCE_DOMAIN, 126)
      .expect("replacement ballot remains stored");
    assert!(votes.vetoes.is_empty());
    assert_eq!(votes.passes.len(), 1);
    assert_eq!(
      Governance::vote_power_custody(ALICE, veto_asset),
      Some(first_position.clone())
    );
    assert_noop!(
      Governance::cast_vote(
        RuntimeOrigin::signed(ALICE),
        PROTOCOL_GOVERNANCE_DOMAIN,
        126,
        pallet_governance::ProposalVoteKind::Pass,
      ),
      pallet_governance::Error::<Runtime>::ProposalVoteAlreadyCast
    );

    System::set_block_number(2);
    submit_root_action_proposal(127, crate::Hash::repeat_byte(27));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(ALICE),
      PROTOCOL_GOVERNANCE_DOMAIN,
      127,
      pallet_governance::ProposalVoteKind::Pass,
    ));
    let extended_position = Governance::vote_power_custody(ALICE, veto_asset)
      .expect("second proposal reuses aggregate custody");
    assert_eq!(extended_position.amount, 40);
    assert!(extended_position.lock_until > first_position.lock_until);
    assert_eq!(
      Assets::balance(
        veto_asset,
        &crate::configs::governance_config::governance_vote_power_custody_account(),
      ),
      40
    );
  });
}

#[test]
fn ordinary_custody_keeps_multiple_domain_lock_ids_independent() {
  new_test_ext().execute_with(|| {
    for domain in [PROTOCOL_GOVERNANCE_DOMAIN, TACTICAL_GOVERNANCE_DOMAIN] {
      if !<Assets as FungiblesInspect<_>>::asset_exists(domain) {
        assert_ok!(create_test_asset(domain, &ALICE));
      }
      assert_ok!(mint_tokens(domain, &ALICE, &ALICE, 100));
      assert_ok!(Staking::register_staking_asset(
        RuntimeOrigin::root(),
        domain
      ));
      assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), domain, 60));
    }
    submit_root_action_proposal(128, crate::Hash::repeat_byte(28));
    let tactical_hash = note_treasury_preimage(ALICE);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      TACTICAL_GOVERNANCE_DOMAIN,
      129,
      ALICE,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::L2TreasurySpend,
      tactical_hash,
    ));
    advance_to_primary_open();
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(ALICE),
      PROTOCOL_GOVERNANCE_DOMAIN,
      128,
      pallet_governance::ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(ALICE),
      TACTICAL_GOVERNANCE_DOMAIN,
      129,
      pallet_governance::ProposalVoteKind::Approve,
    ));
    let protocol_receipt =
      Staking::staked_asset_id(PROTOCOL_GOVERNANCE_DOMAIN).expect("protocol receipt exists");
    let tactical_receipt =
      Staking::staked_asset_id(TACTICAL_GOVERNANCE_DOMAIN).expect("tactical receipt exists");
    assert_ne!(protocol_receipt, tactical_receipt);
    assert_eq!(
      Governance::vote_power_custody(ALICE, protocol_receipt)
        .expect("protocol position exists")
        .amount,
      60
    );
    assert_eq!(
      Governance::vote_power_custody(ALICE, tactical_receipt)
        .expect("tactical position exists")
        .amount,
      60
    );
    let custody = crate::configs::governance_config::governance_vote_power_custody_account();
    assert_eq!(Assets::balance(protocol_receipt, &custody), 60);
    assert_eq!(Assets::balance(tactical_receipt, &custody), 60);
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn governance_try_state_reconciles_live_ballot_horizons_and_host_custody() {
  new_test_ext().execute_with(|| {
    use polkadot_sdk::frame_support::traits::Hooks;

    let veto_asset = primitives::ecosystem::protocol_tokens::VETO_ASSET_ID;
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset, &ALICE, 40,
    ));
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset, &BOB, 60,
    ));
    submit_root_action_proposal(130, crate::Hash::repeat_byte(30));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(ALICE),
      PROTOCOL_GOVERNANCE_DOMAIN,
      130,
      pallet_governance::ProposalVoteKind::Veto,
    ));
    assert_ok!(<Governance as Hooks<crate::BlockNumber>>::try_state(1));
    let original_position =
      Governance::vote_power_custody(ALICE, veto_asset).expect("valid aggregate position exists");

    pallet_governance::VotePowerCustodyByAccount::<Runtime>::mutate(
      ALICE,
      veto_asset,
      |position| {
        position.as_mut().expect("position exists").lock_until = 0;
      },
    );
    assert!(<Governance as Hooks<crate::BlockNumber>>::try_state(1).is_err());

    pallet_governance::VotePowerCustodyByAccount::<Runtime>::remove(ALICE, veto_asset);
    assert!(<Governance as Hooks<crate::BlockNumber>>::try_state(1).is_err());

    pallet_governance::VotePowerCustodyByAccount::<Runtime>::insert(
      ALICE,
      veto_asset,
      pallet_governance::VotePowerCustodyPosition {
        amount: original_position.amount + 1,
        lock_until: original_position.lock_until,
      },
    );
    assert!(<Governance as Hooks<crate::BlockNumber>>::try_state(1).is_err());
  });
}

#[test]
fn transferable_staking_receipt_power_is_custodied_reused_and_increased() {
  new_test_ext().execute_with(|| {
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &ALICE, 100));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), 0, 60));
    let receipt = Staking::staked_asset_id(0).expect("staking receipt is registered");
    submit_root_action_proposal(123, crate::Hash::repeat_byte(23));
    submit_root_action_proposal(124, crate::Hash::repeat_byte(24));
    advance_to_primary_open();

    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(ALICE),
      PROTOCOL_GOVERNANCE_DOMAIN,
      123,
      pallet_governance::ProposalVoteKind::Aye,
    ));
    let custody = crate::configs::governance_config::governance_vote_power_custody_account();
    assert_eq!(Assets::balance(receipt, &ALICE), 0);
    assert_eq!(Assets::balance(receipt, &custody), 60);
    assert!(
      <Assets as FungiblesMutate<_>>::transfer(receipt, &ALICE, &BOB, 1, Preservation::Expendable,)
        .is_err(),
      "a frozen receipt cannot move and vote again through another account"
    );

    assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), 0, 20));
    assert_eq!(Assets::balance(receipt, &ALICE), 20);
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(ALICE),
      PROTOCOL_GOVERNANCE_DOMAIN,
      124,
      pallet_governance::ProposalVoteKind::Aye,
    ));
    let position = Governance::vote_power_custody(ALICE, receipt)
      .expect("receipt power remains in one aggregate source position");
    assert_eq!(position.amount, 80);
    assert_eq!(Assets::balance(receipt, &ALICE), 0);
    assert_eq!(Assets::balance(receipt, &custody), 80);
    assert_eq!(
      Governance::account_governance_power_view(PROTOCOL_GOVERNANCE_DOMAIN, 124, ALICE)
        .expect("second proposal remains active")
        .frozen_ordinary_ballot
        .expect("second primary ballot is frozen")
        .raw_power,
      if cfg!(feature = "runtime-benchmarks") {
        1
      } else {
        7 * 80
      }
    );

    System::set_block_number(position.lock_until);
    assert_ok!(Governance::unlock_vote_power(
      RuntimeOrigin::signed(ALICE),
      receipt,
    ));
    assert_eq!(Assets::balance(receipt, &ALICE), 80);
    assert_eq!(Assets::balance(receipt, &custody), 0);
  });
}

#[test]
fn ordinary_track_profile_switches_to_flat_urgent_after_authorization() {
  new_test_ext().execute_with(|| {
    submit_root_action_proposal(109, crate::Hash::repeat_byte(11));
    assert_eq!(
      Governance::proposal_vote_power_profile(
        PROTOCOL_GOVERNANCE_DOMAIN,
        109,
        pallet_governance::ProposalVoteKind::Aye,
      ),
      Some(pallet_governance::ProposalVotePowerProfile::DecliningDirectStake)
    );
    pallet_governance::ProposalUrgentAuthorizedAt::<Runtime>::insert(
      PROTOCOL_GOVERNANCE_DOMAIN,
      109,
      1,
    );
    assert_eq!(
      Governance::proposal_vote_power_profile(
        PROTOCOL_GOVERNANCE_DOMAIN,
        109,
        pallet_governance::ProposalVoteKind::Aye,
      ),
      Some(pallet_governance::ProposalVotePowerProfile::FlatUrgentDirectStake)
    );
  });
}
