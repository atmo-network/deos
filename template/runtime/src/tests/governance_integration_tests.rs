use super::common::{
  ALICE, BOB, actor_fee_sink_account, create_test_asset, mint_tokens, new_test_ext,
};
use crate::configs::governance_config::{
  RuntimeProposalPayloadExecutor, RuntimeProposalPayloadPreimageProvider,
  StrategicRuntimeUpgradePayload, TacticalTreasuryInvoicePayload,
};
use crate::{
  Actors, Address, Assets, Balances, DeosRouter, Executive, Governance, Preimage, Runtime,
  RuntimeCall, RuntimeEvent, RuntimeOrigin, Signature, Staking, System, TxExtension,
  UncheckedExtrinsic,
};
use codec::{Decode, Encode, MaxEncodedLen};
use pallet_governance::{ProposalPayloadExecutor, ProposalPayloadPreimageProvider};
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

pub(super) fn approved_outcome(
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

fn invoice_cid() -> pallet_governance::CidBytesOf<Runtime> {
  let digest = cid::multihash::Multihash::<64>::wrap(0x12, &[7u8; 32]).unwrap();
  cid::Cid::new_v1(0x55, digest)
    .to_string()
    .into_bytes()
    .try_into()
    .unwrap()
}

fn invoice_treasury() -> crate::AccountId {
  Actors::sovereign_account_id_system(primitives::ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID)
}

fn note_treasury_preimage(signer: crate::AccountId) -> crate::Hash {
  let payload = TacticalTreasuryInvoicePayload {
    beneficiary: signer.clone(),
    payout_asset: TACTICAL_GOVERNANCE_DOMAIN,
    base_amount: 1,
    invoice_cid: invoice_cid(),
    treasury_account: invoice_treasury(),
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
    // Declared coefficients only: pending service now adds to every historical branch.
    // Physical qualification of the changed owners remains a separate gate.
    let pending =
      <GovernanceWeights as pallet_governance::WeightInfo>::service_pending_enactments(4);
    for branch in [
      one_empty_epoch.saturating_add(pending).saturating_add(
        <GovernanceWeights as pallet_governance::WeightInfo>::service_maturing_proposals(3),
      ),
      one_empty_epoch.saturating_add(pending),
      one_empty_epoch.saturating_add(pending).saturating_add(
        <GovernanceWeights as pallet_governance::WeightInfo>::service_finalized_proposal_outcomes(
          1024,
        ),
      ),
      one_empty_epoch.saturating_add(pending).saturating_add(
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
fn governance_executor_preflight_never_reads_rejected_byte_values() {
  use pallet_governance::{ProposalExecutionFailureReason as Failure, ProposalPayloadKind as Kind};
  use polkadot_sdk::frame_support::storage::StoragePrefixedMap;
  use polkadot_sdk::{pallet_preimage, sp_core::Blake2Hasher, sp_trie::recorder::Recorder};
  let value_prefix = pallet_preimage::PreimageFor::<Runtime>::final_prefix();
  let root = || polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
  let record = |ext: &mut polkadot_sdk::sp_io::TestExternalities, domain, kind, hash| {
    let recorder = Recorder::<Blake2Hasher>::default();
    let error = ext.execute_with_recorder(recorder.clone(), || {
      RuntimeProposalPayloadExecutor::execute(domain, 100, kind, hash).err()
    });
    let keys = recorder
      .recorded_keys()
      .into_values()
      .flat_map(|keys| keys.into_keys())
      .map(|key| key.as_ref().to_vec())
      .collect::<std::collections::BTreeSet<_>>();
    (error, keys)
  };
  for (kind, domain) in [
    (Kind::L1RootAction, PROTOCOL_GOVERNANCE_DOMAIN),
    (Kind::L2ParameterChange, PROTOCOL_GOVERNANCE_DOMAIN),
    (Kind::L2TreasurySpend, TACTICAL_GOVERNANCE_DOMAIN),
  ] {
    for wrong_domain in [false, true] {
      let mut ext = new_test_ext();
      let (hash, before) = ext.execute_with(|| {
        let ceiling =
          RuntimeProposalPayloadPreimageProvider::payload_length_ceiling(domain, kind).unwrap();
        let bytes = vec![0; ceiling as usize + 1];
        let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
        assert_ok!(Preimage::note_preimage(RuntimeOrigin::signed(ALICE), bytes));
        (hash, root())
      });
      ext.commit_all().unwrap();
      let selected_domain = if wrong_domain {
        if domain == PROTOCOL_GOVERNANCE_DOMAIN {
          TACTICAL_GOVERNANCE_DOMAIN
        } else {
          PROTOCOL_GOVERNANCE_DOMAIN
        }
      } else {
        domain
      };
      let (error, keys) = record(&mut ext, selected_domain, kind, hash);
      assert_eq!(
        error,
        Some(if wrong_domain {
          Failure::UnsupportedDomain
        } else {
          Failure::InvalidPreimage
        })
      );
      assert!(keys.contains(&pallet_preimage::RequestStatusFor::<Runtime>::hashed_key_for(hash)));
      assert!(
        keys.iter().all(|key| !key.starts_with(&value_prefix)),
        "preflight must not read rejected preimage value: kind={kind:?}, wrong_domain={wrong_domain}"
      );
      ext.execute_with(|| assert_eq!(root(), before));
    }
  }
  for kind in [Kind::L1RootAction, Kind::Intent, Kind::L2SignalToL1] {
    let mut ext = new_test_ext();
    let (hash, before) = ext.execute_with(|| {
      let bytes = vec![0; pallet_preimage::MAX_SIZE as usize];
      let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
      assert_ok!(Preimage::note_preimage(RuntimeOrigin::signed(ALICE), bytes));
      (hash, root())
    });
    ext.commit_all().unwrap();
    let (error, keys) = record(&mut ext, PROTOCOL_GOVERNANCE_DOMAIN, kind, hash);
    assert_eq!(
      error,
      Some(if kind == Kind::L1RootAction {
        Failure::InvalidPreimage
      } else {
        Failure::UnsupportedCall
      })
    );
    assert!(keys.iter().all(|key| !key.starts_with(&value_prefix)));
    ext.execute_with(|| assert_eq!(root(), before));
  }
  for requested in [false, true] {
    for (kind, domain) in [
      (Kind::L1RootAction, PROTOCOL_GOVERNANCE_DOMAIN),
      (Kind::L1RootAction, TACTICAL_GOVERNANCE_DOMAIN),
      (Kind::Intent, PROTOCOL_GOVERNANCE_DOMAIN),
    ] {
      let mut ext = new_test_ext();
      let hash = crate::Hash::repeat_byte(77);
      let before = ext.execute_with(|| {
        if requested {
          assert_ok!(Preimage::request_preimage(RuntimeOrigin::root(), hash));
        }
        root()
      });
      ext.commit_all().unwrap();
      let (error, keys) = record(&mut ext, domain, kind, hash);
      assert_eq!(error, Some(Failure::MissingPreimage));
      assert!(keys.iter().all(|key| !key.starts_with(&value_prefix)));
      ext.execute_with(|| assert_eq!(root(), before));
    }
  }
  let mut ext = new_test_ext();
  let target = crate::Hash::repeat_byte(7);
  let hash = ext.execute_with(|| {
    let bytes = StrategicRuntimeUpgradePayload { code_hash: target }.encode();
    let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
    assert_ok!(Preimage::note_preimage(RuntimeOrigin::signed(ALICE), bytes));
    hash
  });
  ext.commit_all().unwrap();
  let (error, keys) = record(
    &mut ext,
    PROTOCOL_GOVERNANCE_DOMAIN,
    Kind::L1RootAction,
    hash,
  );
  assert_eq!(error, None);
  assert!(
    keys.contains(&pallet_preimage::PreimageFor::<Runtime>::hashed_key_for((
      hash,
      StrategicRuntimeUpgradePayload::max_encoded_len() as u32
    )))
  );
  ext.execute_with(|| assert_eq!(System::authorized_upgrade().unwrap().code_hash(), &target));
}

#[test]
fn l1_root_action_admission_and_enactment_share_exact_decoder() {
  use pallet_governance::{
    ProposalExecutionDetail as Detail, ProposalExecutionFailureReason as Failure,
    ProposalPayloadKind as Kind, ProposalPreimageAdmissionError as Admission,
  };
  for case in 0..4 {
    new_test_ext().execute_with(|| {
      let previous = crate::Hash::repeat_byte(9);
      let target = crate::Hash::repeat_byte(7);
      assert_ok!(System::authorize_upgrade(RuntimeOrigin::root(), previous));
      let mut bytes = StrategicRuntimeUpgradePayload { code_hash: target }.encode();
      match case {
        0 => {}
        1 => bytes.push(0),
        2 => {
          bytes.pop();
        }
        3 => bytes.clear(),
        _ => unreachable!(),
      }
      let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
      let validated = RuntimeProposalPayloadPreimageProvider::validate_for_witness(
        PROTOCOL_GOVERNANCE_DOMAIN,
        Kind::L1RootAction,
        &hash,
        &bytes,
      );
      if case == 0 {
        assert!(validated.is_ok());
        assert_eq!(
          RuntimeProposalPayloadPreimageProvider::validate_for_witness(
            TACTICAL_GOVERNANCE_DOMAIN,
            Kind::L1RootAction,
            &hash,
            &bytes,
          ),
          Err(Admission::Incompatible)
        );
      } else {
        assert_eq!(
          validated,
          Err(if case == 1 {
            Admission::Oversized
          } else {
            Admission::Invalid
          })
        );
      }
      assert_ok!(Preimage::note_preimage(RuntimeOrigin::signed(ALICE), bytes));
      submit_root_action_proposal(100, hash);
      let root = || polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      let before_root = root();
      if case == 0 {
        assert!(matches!(
          RuntimeProposalPayloadExecutor::execute(
            TACTICAL_GOVERNANCE_DOMAIN,
            100,
            Kind::L1RootAction,
            hash,
          ),
          Err(Failure::UnsupportedDomain)
        ));
      } else {
        assert!(
          matches!(
            RuntimeProposalPayloadExecutor::execute(
              PROTOCOL_GOVERNANCE_DOMAIN,
              100,
              Kind::L1RootAction,
              hash,
            ),
            Err(Failure::InvalidPreimage)
          ),
          "malformed Root case={case} must not replace upgrade authorization"
        );
      }
      assert_eq!(root(), before_root);
      let sink = System::account(&actor_fee_sink_account());
      resolve_root_action_proposal(100);
      let detail = Governance::proposal_execution_detail(PROTOCOL_GOVERNANCE_DOMAIN, 100);
      if case == 0 {
        assert_eq!(
          detail,
          Some(Detail::Succeeded(
            pallet_governance::ProposalExecutionSuccessDetail::RuntimeUpgradeAuthorized {
              code_hash: target
            },
          ))
        );
        assert_eq!(System::authorized_upgrade().unwrap().code_hash(), &target);
      } else {
        assert_eq!(detail, Some(Detail::Failed(Failure::InvalidPreimage)));
        assert_eq!(System::authorized_upgrade().unwrap().code_hash(), &previous);
      }
      assert_eq!(System::account(&actor_fee_sink_account()), sink);
    });
  }
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
fn l2_router_fee_uses_shared_exact_decoder_without_mutating_actor_controls() {
  use pallet_governance::{
    ProposalExecutionReceipt, ProposalParameterChangeSurface as Surface,
    ProposalPayloadKind as Kind,
  };
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(Actors::set_user_creation_enabled(
      RuntimeOrigin::root(),
      false
    ));
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    let new_fee = polkadot_sdk::sp_runtime::Perbill::from_percent(1);
    let call: RuntimeCall =
      pallet_deos_router::Call::<Runtime>::update_router_fee { new_fee }.into();
    let bytes = call.encode();
    let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      bytes.clone()
    ));
    assert!(
      RuntimeProposalPayloadPreimageProvider::validate_for_witness(
        PROTOCOL_GOVERNANCE_DOMAIN,
        Kind::L2ParameterChange,
        &hash,
        &bytes,
      )
      .is_ok()
    );
    let receipt = RuntimeProposalPayloadExecutor::execute(
      PROTOCOL_GOVERNANCE_DOMAIN,
      0,
      Kind::L2ParameterChange,
      hash,
    )
    .unwrap_or_else(|reason| panic!("Router parameter execution failed: {reason:?}"));
    assert!(matches!(
      receipt,
      ProposalExecutionReceipt::ParameterChangeExecuted {
        surface: Surface::RouterFee,
      }
    ));
    assert_eq!(DeosRouter::router_fee(), new_fee);
    assert!(!Actors::user_creation_enabled());
    assert!(Actors::global_circuit_breaker());
  });
}

#[test]
fn l2_actor_controls_share_witness_and_direct_executor_allowlist() {
  use pallet_governance::{
    ProposalExecutionAuthority, ProposalExecutionReceipt,
    ProposalParameterChangeSurface as Surface, ProposalPayloadKind as Kind,
  };
  for frozen in [false, true] {
    for value in [false, true] {
      for creation in [false, true] {
        new_test_ext().execute_with(|| {
          System::set_block_number(1);
          assert_ok!(Actors::set_user_creation_enabled(
            RuntimeOrigin::root(),
            !value
          ));
          assert_ok!(Actors::set_global_circuit_breaker(
            RuntimeOrigin::root(),
            frozen
          ));
          let next_id = Actors::next_actor_id();
          let count = Actors::active_actor_count();
          let call: RuntimeCall = if creation {
            pallet_deos_actors::Call::<Runtime>::set_user_creation_enabled { enabled: value }.into()
          } else {
            pallet_deos_actors::Call::<Runtime>::set_global_circuit_breaker { paused: value }.into()
          };
          let bytes = call.encode();
          let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
          assert_ok!(Preimage::note_preimage(
            RuntimeOrigin::signed(ALICE),
            bytes.clone()
          ));
          let validated = RuntimeProposalPayloadPreimageProvider::validate_for_witness(
            PROTOCOL_GOVERNANCE_DOMAIN,
            Kind::L2ParameterChange,
            &hash,
            &bytes,
          )
          .expect("exact bounded protocol control is witnessable");
          assert_eq!(
            validated.execution_authority,
            ProposalExecutionAuthority::DomainParameters
          );
          let surface = if creation {
            Surface::ActorUserCreationEnabled
          } else {
            Surface::ActorGlobalCircuitBreaker
          };
          let receipt = RuntimeProposalPayloadExecutor::execute(
            PROTOCOL_GOVERNANCE_DOMAIN,
            0,
            Kind::L2ParameterChange,
            hash,
          )
          .unwrap_or_else(|reason| panic!("bounded control execution failed: {reason:?}"));
          assert!(matches!(
            receipt,
            ProposalExecutionReceipt::ParameterChangeExecuted { surface: actual }
              if actual == surface
          ));
          assert_eq!(
            Actors::user_creation_enabled(),
            if creation { value } else { !value }
          );
          assert_eq!(
            Actors::global_circuit_breaker(),
            if creation { frozen } else { value }
          );
          assert_eq!(Actors::next_actor_id(), next_id);
          assert_eq!(Actors::active_actor_count(), count);
          System::assert_last_event(RuntimeEvent::Actors(if creation {
            pallet_deos_actors::Event::UserCreationEnabledSet { enabled: value }
          } else {
            pallet_deos_actors::Event::GlobalCircuitBreakerSet { paused: value }
          }));
        });
      }
    }
  }
}

#[test]
fn l2_parameter_allowlist_refuses_wrong_domains_calls_and_nonexact_bytes_without_effects() {
  use pallet_governance::{
    ProposalExecutionFailureReason as Failure, ProposalPayloadKind as Kind,
    ProposalPreimageAdmissionError as Admission,
  };
  let control: RuntimeCall =
    pallet_deos_actors::Call::<Runtime>::set_user_creation_enabled { enabled: true }.into();
  let canonical = control.encode();
  let mut trailing = canonical.clone();
  trailing.push(0);
  let mut truncated = canonical.clone();
  truncated.pop();
  let unsupported_actor: RuntimeCall =
    pallet_deos_actors::Call::<Runtime>::set_active_actor_limit { new_limit: 1 }.into();
  let unsupported_system: RuntimeCall =
    frame_system::Call::<Runtime>::remark { remark: vec![] }.into();
  let fee_sink_debit: RuntimeCall =
    polkadot_sdk::pallet_balances::Call::<Runtime>::force_set_balance {
      who: MultiAddress::Id(actor_fee_sink_account()),
      new_free: 0,
    }
    .into();
  for (domain, bytes, admission, failure) in [
    (
      TACTICAL_GOVERNANCE_DOMAIN,
      canonical,
      Admission::Incompatible,
      Failure::UnsupportedDomain,
    ),
    (
      PROTOCOL_GOVERNANCE_DOMAIN,
      unsupported_actor.encode(),
      Admission::Incompatible,
      Failure::UnsupportedCall,
    ),
    (
      PROTOCOL_GOVERNANCE_DOMAIN,
      unsupported_system.encode(),
      Admission::Incompatible,
      Failure::UnsupportedCall,
    ),
    (
      PROTOCOL_GOVERNANCE_DOMAIN,
      trailing,
      Admission::Invalid,
      Failure::InvalidPreimage,
    ),
    (
      PROTOCOL_GOVERNANCE_DOMAIN,
      truncated,
      Admission::Invalid,
      Failure::InvalidPreimage,
    ),
    (
      PROTOCOL_GOVERNANCE_DOMAIN,
      fee_sink_debit.encode(),
      Admission::Oversized,
      Failure::InvalidPreimage,
    ),
  ] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      assert_ok!(Actors::set_global_circuit_breaker(
        RuntimeOrigin::root(),
        true
      ));
      let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
      assert_ok!(Preimage::note_preimage(
        RuntimeOrigin::signed(ALICE),
        bytes.clone()
      ));
      let before = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
      assert_eq!(
        RuntimeProposalPayloadPreimageProvider::validate_for_witness(
          domain,
          Kind::L2ParameterChange,
          &hash,
          &bytes
        )
        .err(),
        Some(admission)
      );
      assert_eq!(
        RuntimeProposalPayloadExecutor::execute(domain, 0, Kind::L2ParameterChange, hash).err(),
        Some(failure)
      );
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
        before
      );
    });
  }
}

// This scenario requires the production vote-power provider; benchmarking replaces it with 1.
#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn signed_l2_actor_admission_enacts_after_real_ordinary_delay_on_declared_presets() {
  exercise_signed_l2_actor_admission(false);
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn signed_l2_actor_admission_matches_complete_prefix_governance_chronology() {
  exercise_signed_l2_actor_admission(true);
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn signed_l2_actor_admission_then_parked_churn_imports_complete_blocks() {
  let pair = sr25519::Pair::from_string("//Alice", None).unwrap();
  for preset in crate::genesis_config_presets::preset_names() {
    let (storage, parent) = super::production_block_replay::signed_l2_admission_from_genesis(
      &pair,
      &preset,
      PROTOCOL_GOVERNANCE_DOMAIN,
    );
    polkadot_sdk::sp_io::TestExternalities::new(storage).execute_with(|| {
      super::production_block_replay::replay_signed_parked_user_epoch_churn(&pair, parent);
    });
    eprintln!("Signed governance/churn complete: {preset}");
  }
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn exercise_signed_l2_actor_admission(prepare_one_call_per_block: bool) {
  use pallet_governance::{ProposalEnactmentOutcome as Outcome, ProposalPayloadKind as Kind};
  let pair = sr25519::Pair::from_string("//Alice", None).unwrap();
  let signer = crate::AccountId::from(pair.public());
  for preset in crate::genesis_config_presets::preset_names() {
    let storage = super::production_block_replay::preset_genesis_storage(&[], &preset);
    polkadot_sdk::sp_io::TestExternalities::new(storage).execute_with(|| {
      System::set_block_number(1);
      Governance::on_initialize(1);
      assert!(!Actors::user_creation_enabled());
      assert!(!Actors::global_circuit_breaker());
      let system_count = Actors::active_actor_count();
      let system_next = Actors::next_actor_id();
      assert_noop!(
        Actors::create_user_actor(
          RuntimeOrigin::signed(signer.clone()),
          pallet_deos_actors::Mutability::Mutable,
          None
        ),
        pallet_deos_actors::Error::<Runtime>::UserCreationDisabled
      );

      // Use the preset's existing asset, registration and endowment, without genesis overrides.
      let prepare_governance_block = |block| {
        if prepare_one_call_per_block {
          System::set_block_number(block);
          Governance::on_initialize(block);
        }
      };
      let mut nonce = 0;
      let mut apply = |call: RuntimeCall| {
        super::actors_integration_tests::ensure_actor_prepass_context();
        let extrinsic = signed_extrinsic(&pair, nonce, call);
        if !Actors::block_resource_state()
          .is_some_and(|state| state.ensure_block(System::block_number()).is_ok())
        {
          assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
        }
        assert!(matches!(Executive::apply_extrinsic(extrinsic), Ok(Ok(()))));
        nonce += 1;
      };
      apply(RuntimeCall::Staking(pallet_staking::Call::stake {
        asset_id: 0,
        amount: 1_000,
      }));
      let call: RuntimeCall =
        pallet_deos_actors::Call::<Runtime>::set_user_creation_enabled { enabled: true }.into();
      let bytes = call.encode();
      let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
      prepare_governance_block(2);
      apply(RuntimeCall::Preimage(
        polkadot_sdk::pallet_preimage::Call::note_preimage {
          bytes: bytes.clone(),
        },
      ));
      prepare_governance_block(3);
      apply(RuntimeCall::Governance(
        pallet_governance::Call::prepare_payload_admission_witness {
          domain: PROTOCOL_GOVERNANCE_DOMAIN,
          payload_kind: Kind::L2ParameterChange,
          payload_hash: hash,
          payload: bytes.try_into().unwrap(),
        },
      ));
      prepare_governance_block(4);
      apply(RuntimeCall::Governance(
        pallet_governance::Call::submit_signed_proposal {
          domain: PROTOCOL_GOVERNANCE_DOMAIN,
          item_id: 120,
          cadence_mode: pallet_governance::ProposalCadenceMode::Ordinary,
          payload_kind: Kind::L2ParameterChange,
          payload_hash: hash,
        },
      ));
      assert_eq!(
        Governance::proposal_author(PROTOCOL_GOVERNANCE_DOMAIN, 120),
        Some(signer.clone())
      );
      let timing = Governance::proposal_timing(PROTOCOL_GOVERNANCE_DOMAIN, 120).unwrap();
      let open = timing.ordinary_primary_open_epoch;
      let close = timing.ordinary_primary_close_epoch;
      let delay = crate::configs::governance_config::ProposalEnactmentDelay::get();
      assert_eq!(delay, 43_200);
      let due = close.checked_add(delay).unwrap();
      if prepare_one_call_per_block {
        assert_eq!(close, 144_004);
        assert_eq!(due, 187_204);
      }
      for block in System::block_number() + 1..=open {
        System::set_block_number(block);
        Governance::on_initialize(block);
      }
      apply(RuntimeCall::Governance(
        pallet_governance::Call::cast_vote {
          domain: PROTOCOL_GOVERNANCE_DOMAIN,
          item_id: 120,
          vote: pallet_governance::ProposalVoteKind::Aye,
        },
      ));
      assert!(
        Governance::proposal_vote_tally(PROTOCOL_GOVERNANCE_DOMAIN, 120)
          .unwrap()
          .aye_weight
          >= 200
      );
      for block in open + 1..due {
        System::set_block_number(block);
        Governance::on_initialize(block);
        assert!(!Actors::user_creation_enabled());
        if block >= close {
          assert_eq!(
            Governance::finalized_proposal_outcome(PROTOCOL_GOVERNANCE_DOMAIN, 120),
            Some(approved_outcome(close, 1, Outcome::NotAttempted))
          );
          assert_eq!(
            pallet_governance::ProposalPendingEnactmentAt::<Runtime>::get(
              PROTOCOL_GOVERNANCE_DOMAIN,
              120
            ),
            Some(due)
          );
          assert!(Governance::finalized_proposal_outcome_expiry_bucket(block).is_empty());
        }
      }
      System::set_block_number(due);
      Governance::on_initialize(due);
      assert!(Actors::user_creation_enabled());
      assert!(!Actors::global_circuit_breaker());
      assert_eq!(Actors::active_actor_count(), system_count);
      assert_eq!(Actors::next_actor_id(), system_next);
      assert_eq!(
        Governance::finalized_proposal_outcome(PROTOCOL_GOVERNANCE_DOMAIN, 120),
        Some(approved_outcome(close, 1, Outcome::Enacted { epoch: due }))
      );
      assert!(pallet_governance::PendingEnactments::<Runtime>::get().is_empty());
      assert!(pallet_governance::ActiveProposals::<Runtime>::iter_keys().next().is_none());
      assert!(pallet_governance::ProposalMaturityBuckets::<Runtime>::iter().all(|(_, bucket)| bucket.is_empty()));
      assert!(pallet_governance::ExpiryBuckets::<Runtime>::iter().all(|(_, bucket)| bucket.is_empty()));
      assert_eq!(pallet_governance::CurrentEpochServicePhase::<Runtime>::get(), pallet_governance::EpochServicePhase::Maturing);
      assert_eq!(pallet_governance::LastProcessedEpoch::<Runtime>::get(), due - 1);
      assert!(System::events().iter().any(|record| matches!(record.event,
        RuntimeEvent::Governance(pallet_governance::Event::ProposalParameterChangeExecuted {
          domain: PROTOCOL_GOVERNANCE_DOMAIN, item_id: 120,
          surface: pallet_governance::ProposalParameterChangeSurface::ActorUserCreationEnabled,
          executed_epoch, ..
        }) if executed_epoch == due)));
      apply(RuntimeCall::Actors(
        pallet_deos_actors::Call::create_user_actor {
          mutability: pallet_deos_actors::Mutability::Mutable,
          contract: None,
        },
      ));
      assert_eq!(Actors::next_actor_id(), system_next + 1);
      let retention =
        crate::configs::governance_config::FinalizedProposalOutcomeRetentionEpochs::get();
      let expiry = due + retention;
      assert_eq!(
        Governance::finalized_proposal_outcome_expiry_bucket(expiry).len(),
        1
      );
      for block in due + 1..=expiry {
        System::set_block_number(block);
        Governance::on_initialize(block);
      }
      // Chronological history cleanup may need additional opportunities, never an early deadline.
      for block in expiry + 1..=expiry + 4 {
        if Governance::finalized_proposal_outcome(PROTOCOL_GOVERNANCE_DOMAIN, 120).is_none() {
          break;
        }
        System::set_block_number(block);
        Governance::on_initialize(block);
      }
      assert!(Governance::finalized_proposal_outcome(PROTOCOL_GOVERNANCE_DOMAIN, 120).is_none());
      assert!(Governance::proposal_metadata(PROTOCOL_GOVERNANCE_DOMAIN, 120).is_none());
      assert!(Governance::recent_finalized_proposals(PROTOCOL_GOVERNANCE_DOMAIN).is_empty());
      assert!(Actors::user_creation_enabled());
      // Complete the empty successor phase left by terminal-history cleanup.
      let finish_phase_block = System::block_number() + 1;
      System::set_block_number(finish_phase_block);
      Governance::on_initialize(finish_phase_block);
      assert!(pallet_governance::PendingEnactments::<Runtime>::get().is_empty());
      assert!(pallet_governance::ProposalPendingEnactmentAt::<Runtime>::iter_keys().next().is_none());
      assert!(pallet_governance::ActiveProposals::<Runtime>::iter_keys().next().is_none());
      assert!(pallet_governance::ProposalMaturityBuckets::<Runtime>::iter().all(|(_, bucket)| bucket.is_empty()));
      assert!(pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::iter().all(|(_, bucket)| bucket.is_empty()));
      assert!(pallet_governance::ExpiryBuckets::<Runtime>::iter().all(|(_, bucket)| bucket.is_empty()));
      assert_eq!(pallet_governance::CurrentEpochServicePhase::<Runtime>::get(), pallet_governance::EpochServicePhase::Maturing);
      let cursor_lag = System::block_number()
        .checked_sub(pallet_governance::LastProcessedEpoch::<Runtime>::get())
        .unwrap();
      let system_prefix = polkadot_sdk::sp_io::hashing::twox_128(b"System");
      let cursor_key = pallet_governance::LastProcessedEpoch::<Runtime>::hashed_key();
      let preserved_state = || {
        let mut storage = super::production_block_replay::current_top_storage();
        storage.top.retain(|key, _| !key.starts_with(&system_prefix) && key.as_slice() != cursor_key.as_slice());
        (
          storage.top,
          frame_system::Account::<Runtime>::iter().collect::<std::collections::BTreeMap<_, _>>(),
        )
      };
      let state_before = preserved_state();
      let quiet_start = System::block_number();
      for block in quiet_start + 1..=quiet_start + 32 {
        System::set_block_number(block);
        Governance::on_initialize(block);
        assert_eq!(pallet_governance::LastProcessedEpoch::<Runtime>::get(), block - cursor_lag);
        assert_eq!(preserved_state(), state_before);
      }
      println!("Construction Governance: preset={preset} prepare_one_call_per_block={prepare_one_call_per_block} due={due} quiet_start={quiet_start} quiet_blocks=32 cursor_lag={cursor_lag}");
    });
  }
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn signed_l2_breaker_freeze_and_thaw_enact_on_declared_presets_without_root_recovery() {
  exercise_signed_l2_breaker_freeze_and_thaw(false);
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn signed_l2_breaker_thaw_imports_complete_blocks_on_declared_presets() {
  exercise_signed_l2_breaker_freeze_and_thaw(true);
}

#[cfg(not(feature = "runtime-benchmarks"))]
fn exercise_signed_l2_breaker_freeze_and_thaw(complete_thaw_blocks: bool) {
  use pallet_governance::{ProposalEnactmentOutcome as Outcome, ProposalPayloadKind as Kind};
  let pair = sr25519::Pair::from_string("//Alice", None).unwrap();
  let signer = crate::AccountId::from(pair.public());
  for preset in crate::genesis_config_presets::preset_names() {
    let storage = super::production_block_replay::preset_genesis_storage(&[], &preset);
    polkadot_sdk::sp_io::TestExternalities::new(storage).execute_with(|| {
      System::set_block_number(1);
      Governance::on_initialize(1);
      assert!(!Actors::user_creation_enabled());
      assert!(!Actors::global_circuit_breaker());
      let system_count = Actors::active_actor_count();
      let system_next = Actors::next_actor_id();
      let mut nonce = 0;
      let mut apply = |call: RuntimeCall| {
        super::actors_integration_tests::ensure_actor_prepass_context();
        let extrinsic = signed_extrinsic(&pair, nonce, call);
        if !Actors::block_resource_state()
          .is_some_and(|state| state.ensure_block(System::block_number()).is_ok())
        {
          assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
        }
        let result = Executive::apply_extrinsic(extrinsic);
        assert!(
          matches!(result, Ok(Ok(()))),
          "signed call failed: {result:?}"
        );
        nonce += 1;
      };
      apply(RuntimeCall::Staking(pallet_staking::Call::stake {
        asset_id: 0,
        amount: 1_000,
      }));
      for (item_id, paused) in [(121, true), (122, false)] {
        let before = !paused;
        assert_eq!(Actors::global_circuit_breaker(), before);
        let sink = actor_fee_sink_account();
        let fees_before = Balances::free_balance(&sink);
        let call: RuntimeCall =
          pallet_deos_actors::Call::<Runtime>::set_global_circuit_breaker { paused }.into();
        let bytes = call.encode();
        let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
        apply(RuntimeCall::Preimage(
          polkadot_sdk::pallet_preimage::Call::note_preimage {
            bytes: bytes.clone(),
          },
        ));
        apply(RuntimeCall::Governance(
          pallet_governance::Call::prepare_payload_admission_witness {
            domain: PROTOCOL_GOVERNANCE_DOMAIN,
            payload_kind: Kind::L2ParameterChange,
            payload_hash: hash,
            payload: bytes.try_into().unwrap(),
          },
        ));
        apply(RuntimeCall::Governance(
          pallet_governance::Call::submit_signed_proposal {
            domain: PROTOCOL_GOVERNANCE_DOMAIN,
            item_id,
            cadence_mode: pallet_governance::ProposalCadenceMode::Ordinary,
            payload_kind: Kind::L2ParameterChange,
            payload_hash: hash,
          },
        ));
        assert_eq!(Actors::global_circuit_breaker(), before);
        assert!(
          Balances::free_balance(&sink) > fees_before,
          "Governance and transaction fees remain ledger-only while the breaker is set"
        );
        assert_eq!(
          Governance::proposal_author(PROTOCOL_GOVERNANCE_DOMAIN, item_id),
          Some(signer.clone())
        );
        let timing = Governance::proposal_timing(PROTOCOL_GOVERNANCE_DOMAIN, item_id).unwrap();
        let open = timing.ordinary_primary_open_epoch;
        let close = timing.ordinary_primary_close_epoch;
        for block in System::block_number() + 1..=open {
          System::set_block_number(block);
          Governance::on_initialize(block);
          assert_eq!(Actors::global_circuit_breaker(), before);
        }
        apply(RuntimeCall::Governance(
          pallet_governance::Call::cast_vote {
            domain: PROTOCOL_GOVERNANCE_DOMAIN,
            item_id,
            vote: pallet_governance::ProposalVoteKind::Aye,
          },
        ));
        let mut approved = None;
        // Retained historical phases may delay maturity service; keep the real cursor intact.
        for block in open + 1..=close + 8 {
          System::set_block_number(block);
          Governance::on_initialize(block);
          assert_eq!(Actors::global_circuit_breaker(), before);
          assert!(!Actors::user_creation_enabled());
          if let Some(pallet_governance::FinalizedProposalOutcome::Approved {
            approval,
            enactment: Outcome::NotAttempted,
          }) = Governance::finalized_proposal_outcome(PROTOCOL_GOVERNANCE_DOMAIN, item_id)
          {
            approved = Some(approval.approved_epoch);
            break;
          }
        }
        let approved = approved.expect("ordinary maturity obtains a bounded service opportunity");
        assert!(approved >= close);
        let delay = crate::configs::governance_config::ProposalEnactmentDelay::get();
        assert_eq!(delay, 43_200);
        let due = approved.checked_add(delay).unwrap();
        assert_eq!(
          pallet_governance::ProposalPendingEnactmentAt::<Runtime>::get(
            PROTOCOL_GOVERNANCE_DOMAIN,
            item_id
          ),
          Some(due)
        );
        let native_end = if complete_thaw_blocks && !paused {
          due - 1
        } else {
          due
        };
        for block in System::block_number() + 1..native_end {
          System::set_block_number(block);
          Governance::on_initialize(block);
          assert_eq!(Actors::global_circuit_breaker(), before);
          assert!(!Actors::user_creation_enabled());
          assert_eq!(
            pallet_governance::ProposalPendingEnactmentAt::<Runtime>::get(
              PROTOCOL_GOVERNANCE_DOMAIN,
              item_id
            ),
            Some(due)
          );
        }
        if complete_thaw_blocks && !paused {
          super::production_block_replay::replay_pending_governance_thaw_blocks(
            &pair,
            due,
            PROTOCOL_GOVERNANCE_DOMAIN,
            item_id,
          );
          return;
        }
        System::set_block_number(due);
        Governance::on_initialize(due);
        assert_eq!(Actors::global_circuit_breaker(), paused);
        assert!(!Actors::user_creation_enabled());
        assert_eq!(Actors::active_actor_count(), system_count);
        assert_eq!(Actors::next_actor_id(), system_next);
        assert_eq!(
          Governance::finalized_proposal_outcome(PROTOCOL_GOVERNANCE_DOMAIN, item_id),
          Some(approved_outcome(
            approved,
            1,
            Outcome::Enacted { epoch: due }
          ))
        );
        assert!(matches!(
          Governance::proposal_execution_detail(PROTOCOL_GOVERNANCE_DOMAIN, item_id),
          Some(pallet_governance::ProposalExecutionDetail::Succeeded(
            pallet_governance::ProposalExecutionSuccessDetail::ParameterChangeExecuted {
              surface: pallet_governance::ProposalParameterChangeSurface::ActorGlobalCircuitBreaker,
            }
          ))
        ));
        assert!(pallet_governance::PendingEnactments::<Runtime>::get().is_empty());
        if paused {
          assert_noop!(
            Actors::create_user_actor(
              RuntimeOrigin::signed(signer.clone()),
              pallet_deos_actors::Mutability::Mutable,
              None
            ),
            pallet_deos_actors::Error::<Runtime>::GlobalCircuitBreakerActive
          );
        }
      }
      assert_noop!(
        Actors::create_user_actor(
          RuntimeOrigin::signed(signer.clone()),
          pallet_deos_actors::Mutability::Mutable,
          None
        ),
        pallet_deos_actors::Error::<Runtime>::UserCreationDisabled
      );
    });
  }
}

#[test]
fn l2_signed_submission_requires_primary_power_and_keeps_general_fee_admission_while_frozen() {
  use pallet_governance::{
    ProposalCapacityLane, ProposalPayloadKind as Kind, ProposalSubmissionAuthority,
  };
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(Actors::set_user_creation_enabled(
      RuntimeOrigin::root(),
      false
    ));
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    let call: RuntimeCall =
      pallet_deos_actors::Call::<Runtime>::set_user_creation_enabled { enabled: true }.into();
    let bytes = call.encode();
    let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
    assert_ok!(Preimage::note_preimage(RuntimeOrigin::signed(ALICE), bytes));
    prepare_payload_witness(
      ALICE,
      PROTOCOL_GOVERNANCE_DOMAIN,
      Kind::L2ParameterChange,
      hash,
    );
    let policy = Governance::proposal_admission_policy_view(
      PROTOCOL_GOVERNANCE_DOMAIN,
      Kind::L2ParameterChange,
    );
    assert_eq!(
      policy.authority,
      ProposalSubmissionAuthority::PrimaryEligibleSigned
    );
    assert_eq!(policy.capacity_lane, ProposalCapacityLane::General);
    let fee = crate::configs::governance_config::ProposalOpeningFee::get();
    assert_eq!(policy.opening_fee, Some(fee));
    let veto_asset = primitives::ecosystem::protocol_tokens::VETO_ASSET_ID;
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset, &ALICE, 100
    ));
    assert_noop!(
      Governance::submit_signed_proposal(
        RuntimeOrigin::signed(ALICE),
        PROTOCOL_GOVERNANCE_DOMAIN,
        119,
        pallet_governance::ProposalCadenceMode::Ordinary,
        Kind::L2ParameterChange,
        hash
      ),
      pallet_governance::Error::<Runtime>::ProposalSubmitterNotPrimaryEligible
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), 0, 500));
    let payer_before = Balances::free_balance(BOB);
    let sink_before = Balances::free_balance(actor_fee_sink_account());
    assert_ok!(Governance::submit_signed_proposal(
      RuntimeOrigin::signed(BOB),
      PROTOCOL_GOVERNANCE_DOMAIN,
      119,
      pallet_governance::ProposalCadenceMode::Ordinary,
      Kind::L2ParameterChange,
      hash
    ));
    assert_eq!(Balances::free_balance(BOB), payer_before - fee);
    assert_eq!(
      Balances::free_balance(actor_fee_sink_account()),
      sink_before + fee
    );
    assert_eq!(
      Governance::proposal_author(PROTOCOL_GOVERNANCE_DOMAIN, 119),
      Some(BOB)
    );
    assert!(
      !Actors::user_creation_enabled(),
      "submission has no parameter effect"
    );
    assert!(Actors::global_circuit_breaker());
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

fn approve_capacity_invoice(
  asset: u32,
  base: u128,
  item: u32,
  vote: pallet_governance::ProposalVoteKind,
) -> u32 {
  let payload = TacticalTreasuryInvoicePayload {
    beneficiary: BOB,
    payout_asset: asset,
    base_amount: base,
    invoice_cid: invoice_cid(),
    treasury_account: invoice_treasury(),
  };
  approve_invoice_preimage(payload.encode(), item, vote)
}

fn approve_invoice_preimage(
  bytes: Vec<u8>,
  item: u32,
  vote: pallet_governance::ProposalVoteKind,
) -> u32 {
  let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
  if <Preimage as polkadot_sdk::frame_support::traits::PreimageProvider<crate::Hash>>::get_preimage(
    &hash,
  )
  .is_none()
  {
    assert_ok!(Preimage::note_preimage(RuntimeOrigin::signed(ALICE), bytes));
  }
  assert_ok!(Governance::submit_proposal(
    RuntimeOrigin::root(),
    TACTICAL_GOVERNANCE_DOMAIN,
    item,
    ALICE,
    pallet_governance::ProposalCadenceMode::Ordinary,
    pallet_governance::ProposalPayloadKind::L2TreasurySpend,
    hash
  ));
  let approved = advance_to_primary_open();
  assert_ok!(Governance::cast_vote(
    RuntimeOrigin::signed(ALICE),
    TACTICAL_GOVERNANCE_DOMAIN,
    item,
    vote
  ));
  assert_ok!(Governance::resolve_proposal(
    RuntimeOrigin::root(),
    TACTICAL_GOVERNANCE_DOMAIN,
    item,
    polkadot_sdk::frame_support::BoundedVec::try_from(vec![ALICE]).unwrap()
  ));
  approved
}

#[test]
fn l2_treasury_spend_admission_and_enactment_share_exact_decoder() {
  use pallet_governance::{
    ProposalExecutionDetail as Detail, ProposalExecutionFailureReason as Failure,
    ProposalPayloadKind as Kind, ProposalPreimageAdmissionError as Admission,
  };
  for case in 0..5 {
    new_test_ext().execute_with(|| {
      let treasury = Actors::sovereign_account_id_system(
        primitives::ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID,
      );
      let asset = 0x2000_0042u32;
      assert_ok!(Assets::force_create(
        RuntimeOrigin::root(),
        asset,
        MultiAddress::Id(ALICE),
        true,
        1
      ));
      assert_ok!(<Assets as FungiblesMutate<crate::AccountId>>::mint_into(
        asset, &treasury, 300
      ));
      let mut bytes = TacticalTreasuryInvoicePayload {
        beneficiary: BOB,
        payout_asset: asset,
        base_amount: 100,
        invoice_cid: invoice_cid(),
        treasury_account: invoice_treasury(),
      }
      .encode();
      match case {
        0 => {}
        1 => bytes.push(0),
        2 => {
          bytes.pop();
        }
        3 => {
          bytes[1] = b'Q';
        }
        4 => bytes.clear(),
        _ => unreachable!(),
      }
      let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
      let validated = RuntimeProposalPayloadPreimageProvider::validate_for_witness(
        TACTICAL_GOVERNANCE_DOMAIN,
        Kind::L2TreasurySpend,
        &hash,
        &bytes,
      );
      if case == 0 {
        assert!(validated.is_ok());
        assert_eq!(
          RuntimeProposalPayloadPreimageProvider::validate_for_witness(
            PROTOCOL_GOVERNANCE_DOMAIN,
            Kind::L2TreasurySpend,
            &hash,
            &bytes,
          ),
          Err(Admission::Incompatible)
        );
      } else {
        assert_eq!(validated, Err(Admission::Invalid));
      }
      approve_invoice_preimage(bytes, 106, pallet_governance::ProposalVoteKind::Approve);
      let ledger = || {
        (
          Assets::balance(asset, &treasury),
          Assets::balance(asset, &BOB),
          Assets::total_issuance(asset),
          System::account(&actor_fee_sink_account()),
        )
      };
      let before = ledger();
      let root = || polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      let before_root = root();
      if case == 0 {
        assert!(matches!(
          RuntimeProposalPayloadExecutor::execute(
            PROTOCOL_GOVERNANCE_DOMAIN,
            106,
            Kind::L2TreasurySpend,
            hash,
          ),
          Err(Failure::UnsupportedDomain)
        ));
        assert_eq!(root(), before_root);
      } else {
        assert!(
          matches!(
            RuntimeProposalPayloadExecutor::execute(
              TACTICAL_GOVERNANCE_DOMAIN,
              106,
              Kind::L2TreasurySpend,
              hash,
            ),
            Err(Failure::InvalidPreimage)
          ),
          "malformed case={case} must refuse before any payout"
        );
        assert_eq!(root(), before_root);
        assert_eq!(ledger(), before);
      }
      service_pending_enactment(TACTICAL_GOVERNANCE_DOMAIN, 106);
      let detail = Governance::proposal_execution_detail(TACTICAL_GOVERNANCE_DOMAIN, 106);
      if case == 0 {
        assert!(matches!(detail, Some(Detail::Succeeded(_))));
        assert_eq!(
          (
            Assets::balance(asset, &treasury),
            Assets::balance(asset, &BOB)
          ),
          (200, 100)
        );
        assert_eq!(Assets::total_issuance(asset), before.2);
        assert_eq!(System::account(&actor_fee_sink_account()), before.3);
      } else {
        assert_eq!(detail, Some(Detail::Failed(Failure::InvalidPreimage)));
        assert_eq!(ledger(), before);
      }
    });
  }
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn pending_benchmark_fixtures_preserve_ordinary_delay_on_reference_host() {
  for refusal in [None, Some(false), Some(true)] {
    new_test_ext().execute_with(|| {
      let before_service = || {
        let now = System::block_number();
        let delay = <Runtime as pallet_governance::Config>::ProposalEnactmentDelay::get();
        assert_eq!(now, 1 + delay);
        let members = pallet_governance::PendingEnactments::<Runtime>::get();
        let visits = <Runtime as pallet_governance::Config>::MaxPendingEnactmentsPerBlock::get();
        let capacity = <Runtime as pallet_governance::Config>::MaxPendingEnactments::get();
        assert_eq!(
          members.len(),
          if refusal.is_some() { capacity } else { visits } as usize
        );
        let mut due_count = 0;
        for member in members {
          let record =
            pallet_governance::FinalizedProposals::<Runtime>::get(member.domain, member.item_id)
              .unwrap();
          let pallet_governance::FinalizedProposalOutcome::Approved {
            approval,
            enactment: pallet_governance::ProposalEnactmentOutcome::NotAttempted,
          } = record.outcome
          else {
            panic!("source must be approved and pending")
          };
          let due =
            Governance::proposal_pending_enactment_at(member.domain, member.item_id).unwrap();
          assert_eq!(due, approval.approved_epoch + delay);
          assert!(approval.approved_epoch <= now);
          due_count += u32::from(due <= now);
        }
        assert_eq!(due_count, visits);
      };
      pallet_governance::Pallet::<Runtime>::verify_pending_enactment_benchmark(
        refusal,
        &before_service,
      )
      .unwrap();
    });
  }
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn finalized_cleanup_benchmark_verifies_reference_maximum() {
  new_test_ext().execute_with(|| {
    assert_eq!(
      <Runtime as pallet_governance::Config>::MaxRecentFinalizedProposalsPerDomain::get(),
      16_384
    );
    assert_eq!(
      <Runtime as pallet_governance::Config>::MaxFinalizedProposalOutcomesPerBlock::get(),
      1_024
    );
    assert_eq!(
      <Runtime as pallet_governance::Config>::MaxCidBytes::get(),
      96
    );
    pallet_governance::Pallet::<Runtime>::verify_maximum_finalized_cleanup_benchmark().unwrap();
    let domain = TACTICAL_GOVERNANCE_DOMAIN;
    assert_eq!(
      pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(domain).len(),
      15_360
    );
    let record = pallet_governance::FinalizedProposals::<Runtime>::get(domain, 3_024).unwrap();
    assert_eq!(
      record.encoded_size(),
      pallet_governance::FinalizedProposalRecord::<
        crate::AccountId,
        u32,
        crate::Hash,
        u32,
        pallet_governance::CidBytesOf<Runtime>,
      >::max_encoded_len()
    );
  });
}

#[test]
fn maximum_cid_invoice_pending_batch_preserves_capacity_prefix_and_future_member() {
  use pallet_governance::{
    ProposalExecutionDetail as Detail, ProposalExecutionFailureReason as Failure,
    ProposalExecutionSuccessDetail as Success,
  };
  new_test_ext().execute_with(|| {
    let visits = <Runtime as pallet_governance::Config>::MaxPendingEnactmentsPerBlock::get();
    assert_eq!(visits, 4);
    let domain = TACTICAL_GOVERNANCE_DOMAIN;
    let treasury = invoice_treasury();
    if !<Assets as FungiblesInspect<crate::AccountId>>::asset_exists(domain) {
      assert_ok!(Assets::force_create(RuntimeOrigin::root(), domain, ALICE.into(), true, 1));
    }
    let cid = cid::Cid::new_v1(0x55, cid::multihash::Multihash::<64>::wrap(0, &[7u8; 55]).unwrap()).to_string().into_bytes();
    assert_eq!(cid.len(), 96);
    let kind = pallet_governance::ProposalPayloadKind::L2TreasurySpend;
    let mut rows = Vec::new();
    for index in 0..=visits {
      if index == visits { System::set_block_number(2); }
      let beneficiary = crate::AccountId::new([10 + index as u8; 32]);
      let invoice_cid: pallet_governance::CidBytesOf<Runtime> = cid.clone().try_into().unwrap();
      let bytes = TacticalTreasuryInvoicePayload { invoice_cid: invoice_cid.clone(), treasury_account: treasury.clone(), beneficiary: beneficiary.clone(), payout_asset: domain, base_amount: 100 }.encode();
      assert_eq!(bytes.len(), TacticalTreasuryInvoicePayload::max_encoded_len());
      let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
      let expected = Success::TreasurySpendExecuted { invoice_cid, treasury_account: treasury.clone(), beneficiary: beneficiary.clone(), payout_asset: domain, base_amount: 100, scalar: pallet_governance::ProposalTreasurySpendScalar::Amplify, target_amount: 200, actual_amount: 150, capacity_limited: true, settlement_kind: pallet_governance::ProposalTreasurySpendSettlementKind::InvoiceScalarTransfer };
      assert!(rows.iter().all(|(_, _, prior_hash, _)| prior_hash != &hash));
      assert_ok!(Preimage::note_preimage(RuntimeOrigin::signed(ALICE), bytes));
      let item = 200 + index;
      assert_ok!(Governance::submit_proposal(RuntimeOrigin::root(), domain, item, ALICE, pallet_governance::ProposalCadenceMode::Ordinary, kind, hash));
      rows.push((item, beneficiary, hash, expected));
    }
    let primary = advance_to_primary_open();
    for (index, (item, _, _, _)) in rows.iter().enumerate() {
      if index == visits as usize { System::set_block_number(primary + 1); }
      assert_ok!(Governance::cast_vote(RuntimeOrigin::signed(ALICE), domain, *item, pallet_governance::ProposalVoteKind::Amplify));
      assert_ok!(Governance::resolve_proposal(RuntimeOrigin::root(), domain, *item, vec![ALICE].try_into().unwrap()));
    }
    assert_eq!(Assets::balance(domain, &treasury), 0);
    assert_ok!(<Assets as FungiblesMutate<crate::AccountId>>::mint_into(domain, &treasury, 550));
    let issuance = Assets::total_issuance(domain);
    let fee_sink = System::account(&actor_fee_sink_account());
    let actor = pallet_deos_actors::SovereignIndex::<Runtime>::get(&treasury).unwrap();
    let identity = Actors::actor_identity(actor);
    let members = pallet_governance::PendingEnactments::<Runtime>::get();
    assert_eq!(members.len(), (visits + 1) as usize);
    let due = Governance::proposal_pending_enactment_at(domain, rows[0].0).unwrap();
    let future = rows.last().unwrap();
    let future_due = Governance::proposal_pending_enactment_at(domain, future.0).unwrap();
    assert_eq!(future_due, due + 1);
    for (item, _, _, _) in rows.iter().take(visits as usize) {
      assert_eq!(Governance::proposal_pending_enactment_at(domain, *item), Some(due));
    }
    let future_record = pallet_governance::FinalizedProposals::<Runtime>::get(domain, future.0);
    let future_metadata = pallet_governance::ProposalMetadataByItem::<Runtime>::get(domain, future.0);
    service_pending_enactment(domain, rows[0].0);
    for (index, (item, beneficiary, _, expected)) in rows.iter().take(visits as usize).enumerate() {
      assert!(Governance::proposal_pending_enactment_at(domain, *item).is_none());
      let record = pallet_governance::FinalizedProposals::<Runtime>::get(domain, item).unwrap();
      assert!(matches!(&record.outcome, pallet_governance::FinalizedProposalOutcome::Approved { approval, .. } if approval.approved_epoch == primary && approval.winner_count == 1));
      if index < 3 {
        let amount = if index == 2 { 150 } else { 200 };
        let mut receipt = expected.clone();
        if let Success::TreasurySpendExecuted { actual_amount, capacity_limited, .. } = &mut receipt { *actual_amount = amount; *capacity_limited = amount < 200; }
        assert_eq!(record.execution_detail, Some(Detail::Succeeded(receipt)));
        assert_eq!(record.encoded_size(), pallet_governance::FinalizedProposalRecord::<crate::AccountId, u32, crate::Hash, u32, pallet_governance::CidBytesOf<Runtime>>::max_encoded_len());
        assert_eq!(Assets::balance(domain, beneficiary), amount);
      } else {
        assert_eq!(record.execution_detail, Some(Detail::Failed(Failure::InsufficientTreasuryCapacity)));
        assert_eq!(Assets::balance(domain, beneficiary), 0);
      }
    }
    assert_eq!(Assets::balance(domain, &treasury), 0);
    assert_eq!(Assets::total_issuance(domain), issuance);
    assert_eq!(System::account(&actor_fee_sink_account()), fee_sink);
    assert_eq!(Actors::actor_identity(actor), identity);
    assert_eq!(pallet_governance::PendingEnactments::<Runtime>::get().as_slice(), &members[visits as usize..]);
    assert_eq!(pallet_governance::PendingEnactmentCursor::<Runtime>::get(), 0);
    let expiry = due + <Runtime as pallet_governance::Config>::FinalizedProposalOutcomeRetentionEpochs::get();
    let cleanup = pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(expiry);
    assert_eq!(cleanup.as_slice(), &members[..visits as usize]);
    assert_eq!(Governance::proposal_pending_enactment_at(domain, future.0), Some(future_due));
    assert_eq!(pallet_governance::FinalizedProposals::<Runtime>::get(domain, future.0), future_record);
    assert_eq!(pallet_governance::ProposalMetadataByItem::<Runtime>::get(domain, future.0), future_metadata);
    assert_eq!(Assets::balance(domain, &future.1), 0);
    assert_ok!(<Assets as FungiblesMutate<crate::AccountId>>::mint_into(domain, &treasury, 200));
    let funded_issuance = Assets::total_issuance(domain);
    service_pending_enactment(domain, future.0);
    let mut receipt = future.3.clone();
    if let Success::TreasurySpendExecuted { actual_amount, capacity_limited, .. } = &mut receipt { *actual_amount = 200; *capacity_limited = false; }
    assert_eq!(Governance::proposal_execution_detail(domain, future.0), Some(Detail::Succeeded(receipt)));
    assert!(pallet_governance::PendingEnactments::<Runtime>::get().is_empty());
    assert!(!pallet_governance::PendingEnactmentCursor::<Runtime>::exists());
    assert!(Governance::proposal_pending_enactment_at(domain, future.0).is_none());
    assert_eq!(Assets::balance(domain, &future.1), 200);
    assert_eq!(Assets::balance(domain, &treasury), 0);
    assert_eq!(Assets::total_issuance(domain), funded_issuance);
    assert_eq!(System::account(&actor_fee_sink_account()), fee_sink);
    assert_eq!(Actors::actor_identity(actor), identity);
  });
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn finalized_invoice_benchmark_template_matches_real_enactment() {
  use pallet_governance::BenchmarkHelper as _;
  new_test_ext().execute_with(|| {
    let previous = pallet_deos_actors::SovereignIndex::<Runtime>::get(invoice_treasury()).unwrap();
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), previous));
    let (domain, kind, hash, expected) =
      <Runtime as pallet_governance::Config>::BenchmarkHelper::prepare_maximum_finalized_execution(
        &BOB,
      )
      .unwrap();
    assert_eq!(
      kind,
      pallet_governance::ProposalPayloadKind::L2TreasurySpend
    );
    let pallet_governance::ProposalExecutionSuccessDetail::TreasurySpendExecuted {
      ref invoice_cid,
      ref treasury_account,
      ref beneficiary,
      payout_asset,
      base_amount,
      ..
    } = expected
    else {
      panic!("maximum reference receipt must be an Invoice")
    };
    assert_eq!(invoice_cid.len(), 96);
    assert_ne!(
      pallet_deos_actors::SovereignIndex::<Runtime>::get(treasury_account),
      Some(previous)
    );
    assert_eq!(Assets::balance(payout_asset, treasury_account), 0);
    let beneficiary_before = Assets::balance(payout_asset, &BOB);
    assert_ok!(<Assets as FungiblesMutate<crate::AccountId>>::mint_into(
      payout_asset,
      treasury_account,
      150
    ));
    let bytes = TacticalTreasuryInvoicePayload {
      invoice_cid: invoice_cid.clone(),
      treasury_account: treasury_account.clone(),
      beneficiary: beneficiary.clone(),
      payout_asset,
      base_amount,
    }
    .encode();
    assert_eq!(
      <Runtime as frame_system::Config>::Hashing::hash(&bytes),
      hash
    );
    approve_invoice_preimage(bytes, 106, pallet_governance::ProposalVoteKind::Amplify);
    let issuance = Assets::total_issuance(payout_asset);
    let fee_sink = System::account(&actor_fee_sink_account());
    service_pending_enactment(domain, 106);
    let record = pallet_governance::FinalizedProposals::<Runtime>::get(domain, 106).unwrap();
    assert_eq!(
      record.execution_detail,
      Some(pallet_governance::ProposalExecutionDetail::Succeeded(
        expected.clone()
      ))
    );
    assert_eq!(
      record.encoded_size(),
      pallet_governance::FinalizedProposalRecord::<
        crate::AccountId,
        u32,
        crate::Hash,
        u32,
        pallet_governance::CidBytesOf<Runtime>,
      >::max_encoded_len()
    );
    assert_eq!(
      Assets::balance(payout_asset, &BOB),
      beneficiary_before + 150
    );
    assert_eq!(Assets::balance(payout_asset, treasury_account), 0);
    assert_eq!(Assets::total_issuance(payout_asset), issuance);
    assert_eq!(System::account(&actor_fee_sink_account()), fee_sink);
  });
}

#[test]
fn l2_treasury_spend_invoice_codec_matches_browser_source_vector() {
  let cid = invoice_cid();
  assert_eq!(
    cid.as_slice(),
    b"bafkreiaha4dqobyha4dqobyha4dqobyha4dqobyha4dqobyha4dqobyha4"
  );
  let payload = TacticalTreasuryInvoicePayload {
    invoice_cid: cid.clone(),
    treasury_account: crate::AccountId::new([0x11; 32]),
    beneficiary: crate::AccountId::new([0x22; 32]),
    payout_asset: 42,
    base_amount: 100,
  };
  let mut expected = vec![0xec];
  expected.extend_from_slice(&cid);
  expected.extend_from_slice(&[0x11; 32]);
  expected.extend_from_slice(&[0x22; 32]);
  expected.extend_from_slice(&42u32.to_le_bytes());
  expected.extend_from_slice(&100u128.to_le_bytes());
  assert_eq!(payload.encode(), expected);
  let mut cursor = expected.as_slice();
  assert!(TacticalTreasuryInvoicePayload::decode(&mut cursor).unwrap() == payload);
  assert!(cursor.is_empty());
  assert_eq!(TacticalTreasuryInvoicePayload::max_encoded_len(), 182);
  let long_cid = cid::Cid::new_v1(
    0x55,
    cid::multihash::Multihash::<64>::wrap(0, &[7u8; 36]).unwrap(),
  )
  .to_string()
  .into_bytes();
  assert_eq!(long_cid.len(), 65);
  let long = TacticalTreasuryInvoicePayload {
    invoice_cid: long_cid.try_into().unwrap(),
    ..payload
  };
  assert_eq!(&long.encode()[..2], &[5, 1]);
  let max_cid = cid::Cid::new_v1(
    0x55,
    cid::multihash::Multihash::<64>::wrap(0, &[7u8; 55]).unwrap(),
  )
  .to_string()
  .into_bytes();
  assert_eq!(max_cid.len(), 96);
  let maximum = TacticalTreasuryInvoicePayload {
    invoice_cid: max_cid.try_into().unwrap(),
    treasury_account: invoice_treasury(),
    ..long
  }
  .encode();
  assert_eq!(
    maximum.len(),
    TacticalTreasuryInvoicePayload::max_encoded_len()
  );
  new_test_ext().execute_with(|| {
    assert!(
      RuntimeProposalPayloadPreimageProvider::validate_for_witness(
        TACTICAL_GOVERNANCE_DOMAIN,
        pallet_governance::ProposalPayloadKind::L2TreasurySpend,
        &<Runtime as frame_system::Config>::Hashing::hash(&maximum),
        &maximum
      )
      .is_ok()
    );
  });
}

#[test]
fn l2_treasury_spend_rejects_noncanonical_invoice_identity() {
  use pallet_governance::{ProposalExecutionFailureReason as Failure, ProposalPayloadKind as Kind};
  let valid = String::from_utf8(invoice_cid().into_inner()).unwrap();
  let digest = cid::multihash::Multihash::<64>::wrap(0x12, &[7u8; 32]).unwrap();
  let v0 = cid::Cid::new_v0(digest).unwrap().to_string();
  let canonical = cid::Cid::try_from(valid.as_str()).unwrap();
  let mut trailing_bytes = canonical.to_bytes();
  trailing_bytes.push(0);
  let trailing = cid::multibase::encode(cid::multibase::Base::Base32Lower, trailing_bytes);
  let path = format!("/ipfs/{valid}");
  // Upstream accepts these aliases; canonical equality, not parsing alone, must reject them.
  for alias in [&path, &trailing] {
    assert_eq!(cid::Cid::try_from(alias.as_str()).unwrap(), canonical);
    assert_ne!(alias, &canonical.to_string());
  }
  for cid in [
    Vec::new(),
    path.into_bytes(),
    trailing.into_bytes(),
    format!("ipfs://{valid}").into_bytes(),
    valid.to_uppercase().into_bytes(),
    v0.into_bytes(),
    vec![0xff],
    vec![b'b'],
  ] {
    new_test_ext().execute_with(|| {
      let bytes = (
        cid,
        invoice_treasury(),
        BOB,
        TACTICAL_GOVERNANCE_DOMAIN,
        1u128,
      )
        .encode();
      let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
      assert_ok!(Preimage::note_preimage(
        RuntimeOrigin::signed(ALICE),
        bytes.clone()
      ));
      assert_noop!(
        Governance::prepare_payload_admission_witness(
          RuntimeOrigin::signed(ALICE),
          TACTICAL_GOVERNANCE_DOMAIN,
          Kind::L2TreasurySpend,
          hash,
          bytes.try_into().unwrap()
        ),
        pallet_governance::Error::<Runtime>::ProposalPreimageInvalid
      );
      let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      assert!(matches!(
        RuntimeProposalPayloadExecutor::execute(
          TACTICAL_GOVERNANCE_DOMAIN,
          106,
          Kind::L2TreasurySpend,
          hash
        ),
        Err(Failure::InvalidPreimage)
      ));
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        before
      );
    });
  }
  new_test_ext().execute_with(|| {
    let bytes = (
      vec![b'b'; crate::configs::governance_config::MaxCidBytes::get() as usize + 1],
      invoice_treasury(),
      BOB,
      TACTICAL_GOVERNANCE_DOMAIN,
      1u128,
    )
      .encode();
    let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      bytes.clone()
    ));
    assert_noop!(
      Governance::prepare_payload_admission_witness(
        RuntimeOrigin::signed(ALICE),
        TACTICAL_GOVERNANCE_DOMAIN,
        Kind::L2TreasurySpend,
        hash,
        bytes.try_into().unwrap()
      ),
      pallet_governance::Error::<Runtime>::ProposalPreimageOversized
    );
  });
}

#[test]
fn l2_treasury_spend_rejects_foreign_payload_treasury() {
  use pallet_governance::{ProposalExecutionFailureReason as Failure, ProposalPayloadKind as Kind};
  for source in [
    ALICE,
    Actors::sovereign_account_id_system(primitives::ecosystem::actor_ids::BLDR_ANCHOR_ACTORS_ID),
    Actors::sovereign_account_id_system(primitives::ecosystem::actor_ids::TREASURY_B_ACTORS_ID),
  ] {
    new_test_ext().execute_with(|| {
      let asset = 0x2000_0042;
      assert_ok!(Assets::force_create(
        RuntimeOrigin::root(),
        asset,
        MultiAddress::Id(ALICE),
        true,
        1
      ));
      for who in [&source, &invoice_treasury()] {
        assert_ok!(<Assets as FungiblesMutate<crate::AccountId>>::mint_into(
          asset, who, 300
        ));
      }
      let bytes = TacticalTreasuryInvoicePayload {
        invoice_cid: invoice_cid(),
        treasury_account: source.clone(),
        beneficiary: BOB,
        payout_asset: asset,
        base_amount: 100,
      }
      .encode();
      let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
      assert_ok!(Preimage::note_preimage(
        RuntimeOrigin::signed(ALICE),
        bytes.clone()
      ));
      assert_noop!(
        Governance::prepare_payload_admission_witness(
          RuntimeOrigin::signed(ALICE),
          TACTICAL_GOVERNANCE_DOMAIN,
          Kind::L2TreasurySpend,
          hash,
          bytes.clone().try_into().unwrap()
        ),
        pallet_governance::Error::<Runtime>::ProposalPreimageIncompatible
      );
      approve_invoice_preimage(bytes, 106, pallet_governance::ProposalVoteKind::Approve);
      let ledger = || {
        (
          Assets::balance(asset, &source),
          Assets::balance(asset, &invoice_treasury()),
          Assets::balance(asset, &BOB),
          Assets::total_issuance(asset),
          System::account(&actor_fee_sink_account()),
        )
      };
      let before = ledger();
      service_pending_enactment(TACTICAL_GOVERNANCE_DOMAIN, 106);
      assert_eq!(
        Governance::proposal_execution_detail(TACTICAL_GOVERNANCE_DOMAIN, 106),
        Some(pallet_governance::ProposalExecutionDetail::Failed(
          Failure::UnsupportedDomain
        ))
      );
      assert_eq!(ledger(), before);
    });
  }
}

fn treasury_control_contract(
  trigger: pallet_deos_actors::TriggerOf<Runtime>,
) -> pallet_deos_actors::ActorContractOf<Runtime> {
  pallet_deos_actors::ActorContract {
    trigger,
    cooldown_blocks: 0,
    window: None,
    steps: Default::default(),
    funding: pallet_deos_actors::FundingSourcePolicy::RuntimePolicy,
    close_when: None,
    on_unfunded: pallet_deos_actors::OnUnfunded::Close,
    parked_balance_activation: None,
    auto_close_at_cycle_nonce: None,
  }
}

#[test]
fn l2_treasury_spend_rechecks_live_treasury_authority() {
  use pallet_deos_actors::Mutability;
  use pallet_governance::{ProposalExecutionDetail as Detail, ProposalPayloadKind as Kind};
  for immutable in [false, true] {
    for stage in 0..3 {
      new_test_ext().execute_with(|| {
        let sovereign_id = primitives::ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID;
        let treasury = Actors::sovereign_account_id_system(sovereign_id);
        let actor_id = pallet_deos_actors::SovereignIndex::<Runtime>::get(&treasury).unwrap();
        let owner = Actors::actor_identity(actor_id).unwrap().owner;
        let asset = 0x2000_0042u32;
        assert_ok!(Assets::force_create(
          RuntimeOrigin::root(),
          asset,
          MultiAddress::Id(ALICE),
          true,
          1
        ));
        assert_ok!(<Assets as FungiblesMutate<crate::AccountId>>::mint_into(
          asset, &treasury, 300
        ));
        let bytes = TacticalTreasuryInvoicePayload {
          beneficiary: BOB,
          payout_asset: asset,
          base_amount: 100,
          invoice_cid: invoice_cid(),
          treasury_account: invoice_treasury(),
        }
        .encode();
        let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
        if stage == 2 {
          approve_capacity_invoice(
            asset,
            100,
            106,
            pallet_governance::ProposalVoteKind::Approve,
          );
        } else {
          assert_ok!(Preimage::note_preimage(
            RuntimeOrigin::signed(ALICE),
            bytes.clone()
          ));
        }
        if stage == 1 {
          prepare_payload_witness(
            ALICE,
            TACTICAL_GOVERNANCE_DOMAIN,
            Kind::L2TreasurySpend,
            hash,
          );
        }
        assert_ok!(Actors::close_actor(RuntimeOrigin::root(), actor_id));
        assert_eq!(
          Assets::balance(asset, &treasury),
          300,
          "close retains custody"
        );
        if immutable {
          assert_ok!(Actors::create_system_actor_at_sovereign_id(
            RuntimeOrigin::root(),
            sovereign_id,
            owner.clone(),
            Mutability::Immutable,
            Some(treasury_control_contract(
              pallet_deos_actors::Trigger::AtTime { after_ticks: 1000 }
            )),
          ));
        }
        let mut before = (
          Assets::balance(asset, &treasury),
          Assets::balance(asset, &BOB),
          Assets::total_issuance(asset),
          System::account(&actor_fee_sink_account()),
        );
        match stage {
          0 => {
            assert_noop!(
              Governance::prepare_payload_admission_witness(
                RuntimeOrigin::signed(ALICE),
                TACTICAL_GOVERNANCE_DOMAIN,
                Kind::L2TreasurySpend,
                hash,
                pallet_governance::ProposalPayloadBytesOf::<Runtime>::try_from(bytes).unwrap(),
              ),
              pallet_governance::Error::<Runtime>::ProposalPreimageIncompatible
            );
          }
          1 => {
            assert_noop!(
              Governance::submit_signed_proposal(
                RuntimeOrigin::signed(ALICE),
                TACTICAL_GOVERNANCE_DOMAIN,
                106,
                pallet_governance::ProposalCadenceMode::Ordinary,
                Kind::L2TreasurySpend,
                hash,
              ),
              pallet_governance::Error::<Runtime>::ProposalPreimageIncompatible
            );
            if !immutable {
              let fresh = Actors::next_actor_id();
              assert_ok!(Actors::create_system_actor_at_sovereign_id(
                RuntimeOrigin::root(),
                sovereign_id,
                owner,
                Mutability::Mutable,
                None,
              ));
              assert_ne!(fresh, actor_id);
              assert_eq!(
                Actors::actor_identity(fresh).unwrap().sovereign_account,
                treasury
              );
              before.3.data.free += crate::configs::governance_config::ProposalOpeningFee::get();
              assert_ok!(Governance::submit_signed_proposal(
                RuntimeOrigin::signed(ALICE),
                TACTICAL_GOVERNANCE_DOMAIN,
                106,
                pallet_governance::ProposalCadenceMode::Ordinary,
                Kind::L2TreasurySpend,
                hash,
              ));
            }
          }
          2 => {
            service_pending_enactment(TACTICAL_GOVERNANCE_DOMAIN, 106);
            assert_eq!(
              Governance::proposal_execution_detail(TACTICAL_GOVERNANCE_DOMAIN, 106),
              Some(Detail::Failed(
                pallet_governance::ProposalExecutionFailureReason::DispatchFailed
              )),
              "inactive treasury authority must not permit payout"
            );
          }
          _ => unreachable!(),
        }
        assert_eq!(
          (
            Assets::balance(asset, &treasury),
            Assets::balance(asset, &BOB),
            Assets::total_issuance(asset),
            System::account(&actor_fee_sink_account())
          ),
          before
        );
      });
    }
  }
}

#[test]
fn l2_treasury_spend_keeps_mutable_treasury_lifecycle_independent() {
  for active in [false, true] {
    new_test_ext().execute_with(|| {
      let treasury = Actors::sovereign_account_id_system(
        primitives::ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID,
      );
      let actor_id = pallet_deos_actors::SovereignIndex::<Runtime>::get(&treasury).unwrap();
      if active {
        assert_ok!(Actors::activate_actor(
          RuntimeOrigin::root(),
          actor_id,
          treasury_control_contract(pallet_deos_actors::Trigger::Manual)
        ));
      }
      let asset = 0x2000_0042u32;
      assert_ok!(Assets::force_create(
        RuntimeOrigin::root(),
        asset,
        MultiAddress::Id(ALICE),
        true,
        1
      ));
      assert_ok!(<Assets as FungiblesMutate<crate::AccountId>>::mint_into(
        asset, &treasury, 300
      ));
      approve_capacity_invoice(
        asset,
        100,
        106,
        pallet_governance::ProposalVoteKind::Approve,
      );
      let lifecycle = || {
        (
          Actors::actor_identity(actor_id),
          Actors::actor_contract(actor_id),
          Actors::actor_run_state(actor_id),
          Actors::actor_processes(actor_id),
        )
      };
      let before = lifecycle();
      let sink = System::account(&actor_fee_sink_account());
      service_pending_enactment(TACTICAL_GOVERNANCE_DOMAIN, 106);
      assert!(matches!(
        Governance::proposal_execution_detail(TACTICAL_GOVERNANCE_DOMAIN, 106),
        Some(pallet_governance::ProposalExecutionDetail::Succeeded(_))
      ));
      assert_eq!(
        (
          Assets::balance(asset, &treasury),
          Assets::balance(asset, &BOB),
          Assets::total_issuance(asset)
        ),
        (200, 100, 300)
      );
      assert_eq!(lifecycle(), before);
      assert_eq!(System::account(&actor_fee_sink_account()), sink);
    });
  }
}

#[test]
fn l2_treasury_spend_rejects_source_dust_roundup() {
  new_test_ext().execute_with(|| {
    let treasury = Actors::sovereign_account_id_system(
      primitives::ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID,
    );
    let asset = 0x2000_0042u32;
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      asset,
      MultiAddress::Id(ALICE),
      true,
      10
    ));
    assert_ok!(<Assets as FungiblesMutate<crate::AccountId>>::mint_into(
      asset, &treasury, 105
    ));
    approve_capacity_invoice(
      asset,
      100,
      106,
      pallet_governance::ProposalVoteKind::Approve,
    );
    let ledger = || {
      (
        Assets::balance(asset, &treasury),
        Assets::balance(asset, &BOB),
        Assets::total_issuance(asset),
        System::account(&actor_fee_sink_account()),
      )
    };
    let before = ledger();
    service_pending_enactment(TACTICAL_GOVERNANCE_DOMAIN, 106);
    assert!(
      matches!(
        Governance::proposal_execution_detail(TACTICAL_GOVERNANCE_DOMAIN, 106),
        Some(pallet_governance::ProposalExecutionDetail::Failed(
          pallet_governance::ProposalExecutionFailureReason::DispatchFailed
        ))
      ),
      "a 100-unit invoice must not transfer the extra5 units of source dust"
    );
    assert_eq!(ledger(), before);
  });
}

#[test]
fn l2_treasury_spend_base_floor_enforces_scalar_minimums() {
  use pallet_governance::ProposalVoteKind::{Amplify, Approve, Reduce};
  for (vote, base, capacity, actual) in [
    (Amplify, 100, 99, None),
    (Amplify, 100, 100, Some(100)),
    (Amplify, 100, 199, Some(199)),
    (Amplify, 100, 250, Some(200)),
    (Approve, 100, 99, None),
    (Approve, 100, 100, Some(100)),
    (Approve, 100, 150, Some(100)),
    (Reduce, 100, 49, None),
    (Reduce, 100, 50, Some(50)),
    (Reduce, 101, 200, Some(50)),
    (Amplify, u128::MAX, 250, None),
  ] {
    new_test_ext().execute_with(|| {
      let treasury = Actors::sovereign_account_id_system(
        primitives::ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID,
      );
      let asset = 0x2000_0042u32;
      assert_ok!(Assets::force_create(
        RuntimeOrigin::root(),
        asset,
        MultiAddress::Id(ALICE),
        true,
        1
      ));
      assert_ok!(<Assets as FungiblesMutate<crate::AccountId>>::mint_into(
        asset, &treasury, capacity
      ));
      approve_capacity_invoice(asset, base, 106, vote);
      let sink = System::account(&actor_fee_sink_account());
      service_pending_enactment(TACTICAL_GOVERNANCE_DOMAIN, 106);
      assert_eq!(Assets::balance(asset, &BOB), actual.unwrap_or(0));
      assert_eq!(
        Assets::balance(asset, &treasury),
        capacity - actual.unwrap_or(0)
      );
      assert_eq!(Assets::total_issuance(asset), capacity);
      assert_eq!(System::account(&actor_fee_sink_account()), sink);
      match actual {
        Some(amount) => {
          let (scalar, target_amount) = match vote {
            Amplify => (
              pallet_governance::ProposalTreasurySpendScalar::Amplify,
              base * 2,
            ),
            Approve => (
              pallet_governance::ProposalTreasurySpendScalar::Approve,
              base,
            ),
            Reduce => (
              pallet_governance::ProposalTreasurySpendScalar::Reduce,
              base / 2,
            ),
            _ => unreachable!(),
          };
          assert_eq!(
            Governance::proposal_execution_detail(TACTICAL_GOVERNANCE_DOMAIN, 106),
            Some(pallet_governance::ProposalExecutionDetail::Succeeded(
              pallet_governance::ProposalExecutionSuccessDetail::TreasurySpendExecuted {
                invoice_cid: invoice_cid(),
                treasury_account: treasury.clone(),
                beneficiary: BOB,
                payout_asset: asset,
                base_amount: base,
                scalar,
                target_amount,
                actual_amount: amount,
                capacity_limited: amount < target_amount,
                settlement_kind:
                  pallet_governance::ProposalTreasurySpendSettlementKind::InvoiceScalarTransfer,
              }
            ))
          );
        }
        None => {
          let reason = if vote == Amplify && base.checked_mul(2).is_none() {
            pallet_governance::ProposalExecutionFailureReason::DispatchFailed
          } else {
            pallet_governance::ProposalExecutionFailureReason::InsufficientTreasuryCapacity
          };
          assert_eq!(
            Governance::proposal_execution_detail(TACTICAL_GOVERNANCE_DOMAIN, 106),
            Some(pallet_governance::ProposalExecutionDetail::Failed(reason))
          );
          assert!(System::events().iter().any(|record| matches!(record.event,
            RuntimeEvent::Governance(pallet_governance::Event::ProposalExecutionFailed {
              reason: observed, ..
            }) if observed == reason)));
        }
      }
    });
  }
}

#[test]
fn l2_treasury_spend_competing_invoices_use_enactment_capacity() {
  new_test_ext().execute_with(|| {
    let treasury = Actors::sovereign_account_id_system(primitives::ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID);
    let asset = 0x2000_0042u32;
    assert_ok!(Assets::force_create(RuntimeOrigin::root(), asset, MultiAddress::Id(ALICE), true, 1));
    assert_ok!(<Assets as FungiblesMutate<crate::AccountId>>::mint_into(asset, &treasury, 300));
    let payload = TacticalTreasuryInvoicePayload {
      beneficiary: BOB, payout_asset: asset, base_amount: 100,
      invoice_cid: invoice_cid(),
    treasury_account: invoice_treasury(),
    }.encode();
    let hash = <Runtime as frame_system::Config>::Hashing::hash(&payload);
    assert_ok!(Preimage::note_preimage(RuntimeOrigin::signed(ALICE), payload));
    let votes = [(106, pallet_governance::ProposalVoteKind::Amplify),
      (107, pallet_governance::ProposalVoteKind::Amplify),
      (108, pallet_governance::ProposalVoteKind::Approve)];
    for (item, _) in votes {
      assert_ok!(Governance::submit_proposal(RuntimeOrigin::root(), TACTICAL_GOVERNANCE_DOMAIN, item, ALICE,
        pallet_governance::ProposalCadenceMode::Ordinary, pallet_governance::ProposalPayloadKind::L2TreasurySpend, hash));
    }
    advance_to_primary_open();
    for (item, vote) in votes {
      assert_ok!(Governance::cast_vote(RuntimeOrigin::signed(ALICE), TACTICAL_GOVERNANCE_DOMAIN, item, vote));
      assert_ok!(Governance::resolve_proposal(RuntimeOrigin::root(), TACTICAL_GOVERNANCE_DOMAIN, item,
        polkadot_sdk::frame_support::BoundedVec::try_from(vec![ALICE]).unwrap()));
    }
    assert_eq!(Assets::balance(asset, &treasury), 300, "approval creates no balance reservation");
    let sink = System::account(&actor_fee_sink_account());
    service_pending_enactment(TACTICAL_GOVERNANCE_DOMAIN, 106);
    for (item, amount) in [(106, 200), (107, 100)] {
      assert!(matches!(Governance::proposal_execution_detail(TACTICAL_GOVERNANCE_DOMAIN, item),
        Some(pallet_governance::ProposalExecutionDetail::Succeeded(
          pallet_governance::ProposalExecutionSuccessDetail::TreasurySpendExecuted { actual_amount, .. }
        )) if actual_amount == amount));
    }
    assert_eq!(Governance::proposal_execution_detail(TACTICAL_GOVERNANCE_DOMAIN, 108),
      Some(pallet_governance::ProposalExecutionDetail::Failed(
        pallet_governance::ProposalExecutionFailureReason::InsufficientTreasuryCapacity)));
    assert_eq!(Assets::balance(asset, &BOB), 300);
    assert_eq!(Assets::balance(asset, &treasury), 0);
    assert_eq!(Assets::total_issuance(asset), 300);
    assert_eq!(System::account(&actor_fee_sink_account()), sink);
  });
}

#[test]
fn l2_treasury_spend_capacity_and_transfer_refusals_preserve_payout_ledger() {
  for source_frozen in [true, false] {
    new_test_ext().execute_with(|| {
      let treasury = Actors::sovereign_account_id_system(
        primitives::ecosystem::actor_ids::BLDR_TREASURY_ACTORS_ID,
      );
      let asset = 0x2000_0042u32;
      assert_ok!(Assets::force_create(
        RuntimeOrigin::root(),
        asset,
        MultiAddress::Id(ALICE),
        source_frozen,
        1
      ));
      assert_ok!(<Assets as FungiblesMutate<crate::AccountId>>::mint_into(
        asset, &treasury, 150
      ));
      approve_capacity_invoice(
        asset,
        100,
        106,
        pallet_governance::ProposalVoteKind::Amplify,
      );
      if source_frozen {
        assert_ok!(Assets::freeze(
          RuntimeOrigin::signed(ALICE),
          asset,
          MultiAddress::Id(treasury.clone())
        ));
      } else {
        // Ordinary Root ledger mutation removes the non-sufficient receiver's provider.
        assert_ok!(Balances::force_set_balance(
          RuntimeOrigin::root(),
          MultiAddress::Id(BOB),
          0
        ));
        assert_eq!(System::account(&BOB).providers, 0);
        assert_eq!(
          <Assets as FungiblesInspect<crate::AccountId>>::reducible_balance(
            asset,
            &treasury,
            Preservation::Expendable,
            polkadot_sdk::frame_support::traits::tokens::Fortitude::Polite
          ),
          150
        );
      }
      let ledger = || {
        (
          Assets::balance(asset, &treasury),
          Assets::balance(asset, &BOB),
          Assets::total_issuance(asset),
          System::account(&actor_fee_sink_account()),
        )
      };
      let before = ledger();
      service_pending_enactment(TACTICAL_GOVERNANCE_DOMAIN, 106);
      assert_eq!(
        Governance::proposal_execution_detail(TACTICAL_GOVERNANCE_DOMAIN, 106),
        Some(pallet_governance::ProposalExecutionDetail::Failed(
          if source_frozen {
            pallet_governance::ProposalExecutionFailureReason::InsufficientTreasuryCapacity
          } else {
            pallet_governance::ProposalExecutionFailureReason::DispatchFailed
          }
        ))
      );
      assert_eq!(
        ledger(),
        before,
        "refusal transfers nothing and preserves issuance/Fee Sink"
      );
    });
  }
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
      invoice_cid: invoice_cid(),
      treasury_account: invoice_treasury(),
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
    let fee_sink = actor_fee_sink_account();
    assert_ne!(treasury_account, fee_sink);
    let fee_sink_before = System::account(&fee_sink);
    assert!(fee_sink_before.data.free >= crate::EXISTENTIAL_DEPOSIT);
    service_pending_enactment(TACTICAL_GOVERNANCE_DOMAIN, 103);
    assert_eq!(System::account(&fee_sink), fee_sink_before);
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
          invoice_cid: invoice_cid(),
          treasury_account: treasury_account.clone(),
          beneficiary: BOB,
          payout_asset: TACTICAL_GOVERNANCE_DOMAIN,
          base_amount: spend_amount,
          scalar: pallet_governance::ProposalTreasurySpendScalar::Approve,
          target_amount: spend_amount,
          capacity_limited: false,
          actual_amount: spend_amount,
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
          invoice_cid: invoice_cid(),
          treasury_account: treasury_account.clone(),
          beneficiary: BOB,
          payout_asset: TACTICAL_GOVERNANCE_DOMAIN,
          base_amount: spend_amount,
          scalar: pallet_governance::ProposalTreasurySpendScalar::Approve,
          target_amount: spend_amount,
          capacity_limited: false,
          actual_amount: spend_amount,
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
      invoice_cid: invoice_cid(),
      treasury_account: invoice_treasury(),
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
    let fee_sink = actor_fee_sink_account();
    assert_ne!(treasury_account, fee_sink);
    let fee_sink_before = System::account(&fee_sink);
    assert!(fee_sink_before.data.free >= crate::EXISTENTIAL_DEPOSIT);
    service_pending_enactment(TACTICAL_GOVERNANCE_DOMAIN, 104);
    assert_eq!(System::account(&fee_sink), fee_sink_before);
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
          invoice_cid: invoice_cid(),
          treasury_account: treasury_account.clone(),
          beneficiary: BOB,
          payout_asset: foreign_asset,
          base_amount: 10 * crate::EXISTENTIAL_DEPOSIT,
          scalar: pallet_governance::ProposalTreasurySpendScalar::Approve,
          target_amount: 10 * crate::EXISTENTIAL_DEPOSIT,
          capacity_limited: false,
          actual_amount: 10 * crate::EXISTENTIAL_DEPOSIT,
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
      invoice_cid: invoice_cid(),
      treasury_account: invoice_treasury(),
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
fn fresh_genesis_preimage_tickets_preserve_fee_sink() {
  use polkadot_sdk::frame_support::{
    storage::storage_prefix,
    traits::{QueryPreimage, StorePreimage, fungible::InspectHold},
  };
  use polkadot_sdk::pallet_preimage::{RequestStatus, RequestStatusFor};
  use polkadot_sdk::sp_io::{TestExternalities, storage};
  use polkadot_sdk::sp_keyring::Sr25519Keyring;

  let payer = Sr25519Keyring::Alice.to_account_id();
  let other = Sr25519Keyring::Bob.to_account_id();
  let sink = actor_fee_sink_account();
  let reason =
    crate::RuntimeHoldReason::Preimage(polkadot_sdk::pallet_preimage::HoldReason::Preimage);
  let legacy_prefix = storage_prefix(b"Preimage", b"StatusFor");
  let bytes = vec![0xa5; 32];
  let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
  let deposit = crate::configs::preimage_config::PreimageBaseDeposit::get()
    + crate::configs::preimage_config::PreimageByteDeposit::get() * bytes.len() as u128;
  assert_ne!(payer, sink);
  assert_ne!(other, sink);
  for preset in crate::genesis_config_presets::preset_names() {
    let genesis = super::production_block_replay::preset_genesis_storage(&[], &preset);
    TestExternalities::new(genesis).execute_with(|| {
      let sink_before = System::account(&sink);
      let issuance_before = Balances::total_issuance();
      let payer_before = Balances::free_balance(&payer);
      let preserved = || {
        // Inspect raw keys so malformed legacy entries cannot look like an empty map.
        assert!(storage::get(&legacy_prefix).is_none());
        assert!(
          storage::next_key(&legacy_prefix).is_none_or(|key| !key.starts_with(&legacy_prefix))
        );
        assert_eq!(System::account(&sink), sink_before);
        assert_eq!(Balances::balance_on_hold(&reason, &sink), 0);
        assert_eq!(Balances::total_issuance(), issuance_before);
      };
      preserved();
      assert!(RequestStatusFor::<Runtime>::get(hash).is_none());
      assert_ok!(Preimage::note_preimage(
        RuntimeOrigin::signed(payer.clone()),
        bytes.clone(),
      ));
      let ticket = RequestStatusFor::<Runtime>::get(hash).unwrap();
      assert!(matches!(
        &ticket,
        RequestStatus::Unrequested { ticket: (owner, _), len }
          if owner == &payer && *len == bytes.len() as u32
      ));
      assert_eq!(Balances::free_balance(&payer), payer_before - deposit);
      assert_eq!(Balances::balance_on_hold(&reason, &payer), deposit);
      preserved();
      assert_noop!(
        Preimage::note_preimage(RuntimeOrigin::signed(other.clone()), bytes.clone()),
        polkadot_sdk::pallet_preimage::Error::<Runtime>::AlreadyNoted
      );
      assert_ok!(Preimage::ensure_updated(
        RuntimeOrigin::signed(other.clone()),
        vec![hash],
      ));
      assert_eq!(RequestStatusFor::<Runtime>::get(hash), Some(ticket));
      assert_eq!(
        <Preimage as QueryPreimage>::len(&hash),
        Some(bytes.len() as u32)
      );
      assert_eq!(
        <Preimage as QueryPreimage>::fetch(&hash, None)
          .unwrap()
          .as_ref(),
        bytes.as_slice()
      );
      assert!(!<Preimage as QueryPreimage>::is_requested(&hash));
      preserved();
      <Preimage as QueryPreimage>::request(&hash);
      <Preimage as QueryPreimage>::request(&hash);
      assert!(matches!(
        RequestStatusFor::<Runtime>::get(hash),
        Some(RequestStatus::Requested { maybe_ticket: Some((owner, _)), count: 2, .. })
          if owner == payer
      ));
      assert_eq!(Balances::balance_on_hold(&reason, &payer), deposit);
      preserved();
      assert_ok!(Preimage::unrequest_preimage(RuntimeOrigin::root(), hash));
      <Preimage as QueryPreimage>::unrequest(&hash);
      assert_noop!(
        Preimage::unnote_preimage(RuntimeOrigin::signed(other.clone()), hash),
        polkadot_sdk::pallet_preimage::Error::<Runtime>::NotAuthorized
      );
      assert_ok!(Preimage::unnote_preimage(
        RuntimeOrigin::signed(payer.clone()),
        hash
      ));
      assert_eq!(Balances::free_balance(&payer), payer_before);
      assert_eq!(Balances::balance_on_hold(&reason, &payer), 0);
      assert!(RequestStatusFor::<Runtime>::get(hash).is_none());
      preserved();
      let system_hash = <Preimage as StorePreimage>::note(bytes.clone().into()).unwrap();
      assert_eq!(system_hash, hash);
      assert!(matches!(
        RequestStatusFor::<Runtime>::get(hash),
        Some(RequestStatus::Requested {
          maybe_ticket: None,
          count: 1,
          ..
        })
      ));
      <Preimage as StorePreimage>::unnote(&hash);
      assert_eq!(Balances::free_balance(&payer), payer_before);
      assert!(RequestStatusFor::<Runtime>::get(hash).is_none());
      preserved();
    });
  }
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
      .begin_prepass(crate::configs::BlockResourceBudgetValue::get())
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
  for maximum in [false, true] {
    new_test_ext().execute_with(|| {
      let domain = TACTICAL_GOVERNANCE_DOMAIN;
      let kind = pallet_governance::ProposalPayloadKind::L2TreasurySpend;
      let invoice_cid = if maximum {
        cid::Cid::new_v1(
          0x55,
          cid::multihash::Multihash::<64>::wrap(0, &[7u8; 55]).unwrap(),
        )
        .to_string()
        .into_bytes()
        .try_into()
        .unwrap()
      } else {
        invoice_cid()
      };
      assert_eq!(invoice_cid.len(), if maximum { 96 } else { 59 });
      let treasury = invoice_treasury();
      let payload = TacticalTreasuryInvoicePayload {
        invoice_cid,
        treasury_account: treasury.clone(),
        beneficiary: ALICE,
        payout_asset: domain,
        base_amount: 1,
      }
      .encode();
      let len = payload.len();
      assert_eq!(
        len,
        if maximum {
          TacticalTreasuryInvoicePayload::max_encoded_len()
        } else {
          144
        }
      );
      let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&payload);
      assert_ok!(Preimage::note_preimage(
        RuntimeOrigin::signed(ALICE),
        payload
      ));
      let free_before = Balances::free_balance(ALICE);
      let reserved_before = Balances::reserved_balance(ALICE);
      let issuance = Balances::total_issuance();
      let asset_issuance = Assets::total_issuance(domain);
      let custody = Assets::balance(domain, &treasury);
      let actor = pallet_deos_actors::SovereignIndex::<Runtime>::get(&treasury).unwrap();
      let identity = Actors::actor_identity(actor);
      let fee_sink = actor_fee_sink_account();
      let sink_before = Balances::free_balance(&fee_sink);
      let deposit = crate::configs::governance_config::PayloadAdmissionWitnessDeposit::get();
      let opening_fee = crate::configs::governance_config::ProposalOpeningFee::get();
      prepare_payload_witness(ALICE, domain, kind, payload_hash);
      assert_eq!(
        Governance::payload_admission_witness(payload_hash, (domain, kind)),
        Some(pallet_governance::ProposalPayloadAdmissionWitness {
          domain,
          payload_kind: kind,
          payload_len: len as u32,
          execution_authority: pallet_governance::ProposalExecutionAuthority::DomainTreasury,
          compatibility: pallet_governance::ProposalPayloadCompatibility {
            schema_version: 1,
            runtime_spec_version: Some(crate::VERSION.spec_version),
          },
          depositor: ALICE,
          deposit,
        })
      );
      assert_eq!(Balances::free_balance(ALICE), free_before - deposit);
      assert_eq!(Balances::reserved_balance(ALICE), reserved_before + deposit);
      assert_eq!(Balances::free_balance(&fee_sink), sink_before);
      assert_ok!(Governance::submit_signed_proposal(
        RuntimeOrigin::signed(ALICE),
        domain,
        112,
        pallet_governance::ProposalCadenceMode::Ordinary,
        kind,
        payload_hash,
      ));
      assert_eq!(Governance::proposal_author(domain, 112), Some(ALICE));
      assert_eq!(
        pallet_governance::ProposalMetadataByItem::<Runtime>::get(domain, 112),
        Some(pallet_governance::ProposalMetadata {
          cadence_mode: pallet_governance::ProposalCadenceMode::Ordinary,
          payload_kind: kind,
          payload_hash,
        })
      );
      assert!(Governance::payload_admission_witness(payload_hash, (domain, kind)).is_none());
      assert_eq!(Balances::free_balance(ALICE), free_before - opening_fee);
      assert_eq!(Balances::reserved_balance(ALICE), reserved_before);
      assert_eq!(Balances::free_balance(&fee_sink), sink_before + opening_fee);
      assert!(System::events().iter().any(|record| record.event
        == RuntimeEvent::Governance(pallet_governance::Event::ProposalOpeningFeeCollected {
          domain,
          item_id: 112,
          proposer: ALICE,
          amount: opening_fee,
        })));
      assert_eq!(Balances::total_issuance(), issuance);
      assert_eq!(Assets::total_issuance(domain), asset_issuance);
      assert_eq!(Assets::balance(domain, &treasury), custody);
      assert_eq!(Actors::actor_identity(actor), identity);
      assert_noop!(
        Governance::submit_signed_proposal(
          RuntimeOrigin::signed(ALICE),
          domain,
          113,
          pallet_governance::ProposalCadenceMode::Ordinary,
          kind,
          payload_hash,
        ),
        pallet_governance::Error::<Runtime>::ProposalPreimageWitnessMissing
      );
    });
  }
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
