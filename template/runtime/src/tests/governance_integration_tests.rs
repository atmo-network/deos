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

fn advisory_preimage_bytes(summary: &[u8]) -> Vec<u8> {
  (
    Option::<crate::Hash>::None,
    summary.to_vec(),
    Option::<Vec<u8>>::None,
  )
    .encode()
}

fn note_advisory_preimage(signer: crate::AccountId, summary: &[u8]) -> crate::Hash {
  let payload = advisory_preimage_bytes(summary);
  let hash = <Runtime as frame_system::Config>::Hashing::hash(&payload);
  assert_ok!(Preimage::note_preimage(
    RuntimeOrigin::signed(signer),
    payload
  ));
  hash
}

/// Data-only packet for the two-domain Intent cleanup chronology. No signing or dispatch.
pub(super) fn same_expiry_signed_intent_packet() -> [RuntimeCall; 5] {
  use pallet_governance::{ProposalCadenceMode, ProposalPayloadKind};
  let preimage = advisory_preimage_bytes(b"same expiry domains");
  let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&preimage);
  let payload: pallet_governance::ProposalPayloadBytesOf<Runtime> = preimage
    .clone()
    .try_into()
    .expect("fixed advisory payload fits the runtime bound");
  let witness = |domain| {
    RuntimeCall::Governance(pallet_governance::Call::prepare_payload_admission_witness {
      domain,
      payload_kind: ProposalPayloadKind::Intent,
      payload_hash,
      payload: payload.clone(),
    })
  };
  let submit = |domain| {
    RuntimeCall::Governance(pallet_governance::Call::submit_signed_proposal {
      domain,
      item_id: 110,
      cadence_mode: ProposalCadenceMode::Ordinary,
      payload_kind: ProposalPayloadKind::Intent,
      payload_hash,
    })
  };
  [
    RuntimeCall::Preimage(polkadot_sdk::pallet_preimage::Call::note_preimage { bytes: preimage }),
    witness(101),
    submit(101),
    witness(102),
    submit(102),
  ]
}

#[test]
fn same_expiry_signed_intent_packet_has_exact_codec_without_state() {
  // Intentionally no Externalities: packet construction must not repair or read runtime state.
  let payload = advisory_preimage_bytes(b"same expiry domains");
  let mut expected_payload = vec![0, 19 << 2];
  expected_payload.extend_from_slice(b"same expiry domains");
  expected_payload.push(0);
  assert_eq!(payload, expected_payload);
  assert_eq!(payload.len(), 22);
  let hash = <Runtime as frame_system::Config>::Hashing::hash(&payload);
  assert_eq!(
    format!("{hash:x}"),
    "9d987885cf7efc3014333ef6cc9d88bd36c0e3c2d2e177464f1c5085a8ef0a52"
  );
  let packet = same_expiry_signed_intent_packet();
  let mut expected_note = vec![45, 0, 22 << 2];
  expected_note.extend_from_slice(&payload);
  assert_eq!(packet[0].encode(), expected_note);
  for (index, domain) in [(1usize, 101u32), (3, 102)] {
    let mut witness = vec![44, 11];
    witness.extend_from_slice(&domain.to_le_bytes());
    witness.push(3);
    witness.extend_from_slice(hash.as_bytes());
    witness.push(22 << 2);
    witness.extend_from_slice(&payload);
    assert_eq!(packet[index].encode(), witness);
    let mut submit = vec![44, 3];
    submit.extend_from_slice(&domain.to_le_bytes());
    submit.extend_from_slice(&110u32.to_le_bytes());
    submit.extend_from_slice(&[0, 3]);
    submit.extend_from_slice(hash.as_bytes());
    assert_eq!(packet[index + 1].encode(), submit);
  }
  assert_eq!(
    packet.each_ref().map(Encode::encoded_size),
    [25, 62, 44, 62, 44]
  );
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
fn pending_full_topology_prepares_a_lifted_epoch_for_the_actual_hook() {
  type W = crate::weights::pallet_governance::SubstrateWeight<Runtime>;
  let delay = <Runtime as pallet_governance::Config>::ProposalEnactmentDelay::get();
  assert!(delay > 0);
  for invalid in [delay - 1, delay] {
    new_test_ext().execute_with(|| {
      let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      assert!(matches!(
        Governance::prepare_full_pending_enactment_hook(invalid, false, None),
        Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Skip)
      ));
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        before
      );
    });
  }
  new_test_ext().execute_with(|| {
    let retention =
      <Runtime as pallet_governance::Config>::FinalizedProposalOutcomeRetentionEpochs::get();
    assert!(retention > 0);
    let overflow_epoch = u32::MAX - retention + 1;
    let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert!(Governance::prepare_full_pending_enactment_hook(overflow_epoch, false, None).is_err());
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      before
    );
  });
  for refused in [false, true] {
    new_test_ext().execute_with(|| {
      let now = 1 + delay + 17;
      // Prepared pending-only history context, not a public history or maximal joint phase.
      pallet_governance::LastProcessedEpoch::<Runtime>::put(now);
      let verify_pending_topology = Governance::prepare_full_pending_enactment_hook(now, refused, None).unwrap();
      let capacity = <Runtime as pallet_governance::Config>::MaxPendingEnactments::get();
      let visits = <Runtime as pallet_governance::Config>::MaxPendingEnactmentsPerBlock::get();
      let members = pallet_governance::PendingEnactments::<Runtime>::get();
      assert_eq!(members.len(), capacity as usize);
      assert_eq!(System::block_number(), now);
      assert_eq!(pallet_governance::PendingEnactmentCursor::<Runtime>::get(), capacity - visits);
      assert!(!pallet_deos_actors::UserCreationEnabled::<Runtime>::get(), "preparation must not dispatch the pending effect");
      for (index, member) in members.iter().enumerate() {
        let due = if index >= (capacity - visits) as usize { now } else { now + 1 };
        let expected_approval = due - delay;
        let record = pallet_governance::FinalizedProposals::<Runtime>::get(member.domain, member.item_id).unwrap();
        assert!(matches!(record.outcome, pallet_governance::FinalizedProposalOutcome::Approved { approval, enactment: pallet_governance::ProposalEnactmentOutcome::NotAttempted } if approval.approved_epoch == expected_approval));
        assert!(expected_approval <= now);
        assert_eq!(Governance::proposal_pending_enactment_at(member.domain, member.item_id), Some(due));
      }
      let charged = Governance::on_initialize(now);
      assert_eq!(charged, <W as pallet_governance::WeightInfo>::service_pending_enactments(visits));
      assert_eq!(pallet_governance::LastProcessedEpoch::<Runtime>::get(), now);
      assert_eq!(pallet_deos_actors::UserCreationEnabled::<Runtime>::get(), !refused);
      verify_pending_topology(&[]).unwrap();
      println!("GOVERNANCE_FULL_PENDING_HOOK_SOURCE_V1 {}", serde_json::json!({
        "refused": refused, "epoch": now, "dueApprovalEpoch": now - delay,
        "capacity": capacity, "visits": visits, "preparedWithoutService": true,
        "actualHook": true, "historyAlreadyCompletedAtCurrentEpoch": true,
        "syntheticNotPublicHistoryOrJointMaximum": true,
        "notPhysicalTimingProofOrProductionPrice": true
      }));
    });
  }
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn pending_and_maximum_cleanup_share_one_hook_without_fixture_collision() {
  new_test_ext().execute_with(|| {
    type W = crate::weights::pallet_governance::SubstrateWeight<Runtime>;
    let (cleanup_domain, expiry) = Governance::prepare_maximum_finalized_cleanup_hook().unwrap();
    let now = expiry + 1;
    let before = pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(cleanup_domain);
    let due = pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(expiry);
    let future = pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::iter()
      .filter(|(epoch, _)| *epoch > expiry)
      .collect::<Vec<_>>();
    assert_eq!(before.len(), 16_384);
    assert_eq!(due.len(), 1_024);
    pallet_governance::LastProcessedEpoch::<Runtime>::put(expiry - 1);
    let verify_pending_topology =
      Governance::prepare_full_pending_enactment_hook(now, false, None).unwrap();
    let members = pallet_governance::PendingEnactments::<Runtime>::get();
    assert_eq!(members.len(), 128);
    assert!(members.iter().all(|touch| touch.domain != cleanup_domain));
    let retention =
      <Runtime as pallet_governance::Config>::FinalizedProposalOutcomeRetentionEpochs::get();
    assert!(future.iter().all(|(epoch, _)| *epoch != now + retention));
    #[cfg(feature = "try-runtime")]
    assert_ok!(<Governance as polkadot_sdk::frame_support::traits::Hooks<
      crate::BlockNumber,
    >>::try_state(now));
    let charged = Governance::on_initialize(now);
    let visits = <Runtime as pallet_governance::Config>::MaxPendingEnactmentsPerBlock::get();
    let n = <Runtime as pallet_governance::Config>::MaxFinalizedProposalOutcomesPerBlock::get();
    let expected = <W as pallet_governance::WeightInfo>::service_pending_enactments(visits)
      .saturating_add(<W as pallet_governance::WeightInfo>::service_epoch_catch_up())
      .saturating_add(<W as pallet_governance::WeightInfo>::service_finalized_proposal_outcomes(n));
    assert_eq!(charged, expected);
    verify_pending_topology(&[]).unwrap();
    assert_eq!(
      pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(cleanup_domain).as_slice(),
      &before[n as usize..]
    );
    for touch in due {
      assert!(
        !pallet_governance::FinalizedProposals::<Runtime>::contains_key(
          touch.domain,
          touch.item_id
        )
      );
      assert!(
        !pallet_governance::ProposalMetadataByItem::<Runtime>::contains_key(
          touch.domain,
          touch.item_id
        )
      );
    }
    for (epoch, bucket) in future {
      assert_eq!(
        pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(epoch),
        bucket
      );
    }
    assert!(
      pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(expiry).is_empty()
    );
    assert_eq!(
      pallet_governance::LastProcessedEpoch::<Runtime>::get(),
      expiry - 1
    );
    assert_eq!(
      pallet_governance::CurrentEpochServicePhase::<Runtime>::get(),
      pallet_governance::EpochServicePhase::RewardExpiry
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(<Governance as polkadot_sdk::frame_support::traits::Hooks<
      crate::BlockNumber,
    >>::try_state(now));
    println!(
      "GOVERNANCE_JOINT_PENDING_CLEANUP_HOOK_SOURCE_V1 {}",
      serde_json::json!({
        "epoch": now, "oldExpiry": expiry, "newTerminalExpiry": now + retention,
        "pendingBefore": members.len(), "pendingVisits": visits,
        "oldIndexBefore": before.len(), "oldCleanup": n,
        "distinctPendingAndCleanupDomains": true,
        "syntheticNotPublicHistoryOrJointMaximum": true,
        "notPhysicalTimingProofOrProductionPrice": true
      })
    );
  });
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn finalized_cleanup_benchmark_verifies_reference_maximum() {
  use polkadot_sdk::frame_benchmarking::{Recording, benchmarking};
  use polkadot_sdk::frame_support::storage::StoragePrefixedMap;
  #[derive(Default)]
  struct CleanupRecording {
    starts: u32,
    counts: Option<(u32, u32, u32, u32)>,
    prefix_rows: Vec<(Vec<u8>, u32, u32, bool)>,
  }
  impl Recording for CleanupRecording {
    fn start(&mut self) {
      assert_eq!(self.starts, 0);
      self.starts += 1;
      benchmarking::set_whitelist(Vec::new());
      benchmarking::commit_db();
      benchmarking::reset_read_write_count();
    }
    fn stop(&mut self) {
      assert!(self.counts.is_none());
      // Flush actual SDK bookkeeping before verification can add diagnostic reads.
      benchmarking::commit_db();
      self.counts = Some(benchmarking::read_write_count());
      self.prefix_rows = benchmarking::get_read_and_written_keys();
    }
  }
  super::common::with_benchmarking_host(
    super::common::benchmarking_storage(new_test_ext()),
    |_| {
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
      let mut recording = CleanupRecording::default();
      Governance::verify_maximum_finalized_cleanup_benchmark(&mut recording).unwrap();
      assert_eq!(recording.starts, 1);
      let n = <Runtime as pallet_governance::Config>::MaxFinalizedProposalOutcomesPerBlock::get();
      let (reads, _, writes, _) = recording.counts.unwrap();
      assert_eq!(reads, 2);
      assert_eq!(writes, 4 * n + 2);
      assert!(
        recording
          .prefix_rows
          .iter()
          .all(|(_, _, _, whitelisted)| !whitelisted)
      );
      // The SDK aggregates unique full-key accesses into storage-prefix rows.
      for prefix in [
        pallet_governance::FinalizedProposals::<Runtime>::final_prefix(),
        pallet_governance::ProposalMetadataByItem::<Runtime>::final_prefix(),
        pallet_governance::ProposalWinningPrimaryOptionByItem::<Runtime>::final_prefix(),
        pallet_governance::ProposalUrgentAuthorizedAt::<Runtime>::final_prefix(),
      ] {
        let family = recording
          .prefix_rows
          .iter()
          .filter(|(key, _, _, _)| key == &prefix)
          .collect::<Vec<_>>();
        assert_eq!(family.len(), 1);
        assert_eq!((family[0].1, family[0].2), (0, n));
      }
      let pending_prefix = pallet_governance::ProposalPendingEnactmentAt::<Runtime>::final_prefix();
      assert!(
        recording
          .prefix_rows
          .iter()
          .all(|(key, _, _, _)| key != &pending_prefix)
      );
      assert_eq!(recording.prefix_rows.len(), 6);
      for prefix in [
        pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::final_prefix(),
        pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::final_prefix(),
      ] {
        let row = recording
          .prefix_rows
          .iter()
          .find(|(key, _, _, _)| key == &prefix)
          .unwrap();
        assert_eq!((row.1, row.2), (1, 1));
      }
      let db = <Runtime as frame_system::Config>::DbWeight::get();
      let write_model = db.writes(u64::from(writes));
      let quarter = crate::MAXIMUM_BLOCK_WEIGHT / 4;
      assert!(write_model.ref_time() < quarter.ref_time());
      assert_eq!(write_model.proof_size(), 0);
      println!(
        "GOVERNANCE_CLEANUP_SDK_FOOTPRINT n={n} reads={reads} unique_writes={writes} write_model_ps={} quarter_ps={} physical_bound=false",
        write_model.ref_time(),
        quarter.ref_time()
      );
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
    },
  );
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn finalized_cleanup_diverse_benchmark_preserves_shared_population() {
  for n in [2, 1_024] {
    new_test_ext().execute_with(|| {
      Governance::verify_diverse_finalized_cleanup_benchmark(n).unwrap();
      let mut remaining = 0;
      for offset in 0..n {
        let domain = 101 + offset;
        let ids = pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(domain);
        let before = 16_384 / n + u32::from(offset < 16_384 % n);
        assert_eq!(ids.len(), (before - 1) as usize);
        remaining += ids.len();
        assert!(!pallet_governance::FinalizedProposals::<Runtime>::contains_key(domain, offset));
      }
      assert_eq!(remaining, (16_384 - n) as usize);
      assert!(!pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::contains_key(101 + n));
      println!(
        "GOVERNANCE_DIVERSE_BENCHMARK_SOURCE_V2 n={n} total=16384 domains={n} remaining={remaining}"
      );
    });
  }
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn complete_cleanup_benchmark_verifies_reference_hook_phases() {
  for phase in 0..=1 {
    for n in [1, 1024] {
      new_test_ext().execute_with(|| {
        Governance::verify_complete_finalized_cleanup_benchmark(n, phase).unwrap();
      });
    }
  }
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
#[ignore = "opt-in reference maximum cleanup fixture; native source conformance only"]
fn complete_diverse_cleanup_benchmark_verifies_reference_hook_phases() {
  for n in [3, 1024] {
    for phase in 0..=1 {
      new_test_ext().execute_with(|| {
        Governance::verify_complete_diverse_finalized_cleanup_benchmark(n, phase).unwrap();
        let remaining = (0..n)
          .map(|offset| {
            let ids = pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(101 + offset);
            assert!(!ids.contains(&offset));
            ids.len()
          })
          .sum::<usize>();
        assert_eq!(remaining, 16384 - n as usize);
        assert!(!pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::contains_key(101 + n));
      });
    }
  }
}

#[cfg(all(feature = "runtime-benchmarks", feature = "try-runtime"))]
#[test]
#[ignore = "opt-in joint pending/full-index prepared source; not physical pricing or history"]
fn independent_full_indices_and_pending_share_old_epoch_hook() {
  new_test_ext().execute_with(|| {
    type W = crate::weights::pallet_governance::SubstrateWeight<Runtime>;
    let (prepared_clock, expiry, check_cleanup) = Governance::prepare_independent_full_indices_cleanup_component().unwrap();
    let delay = <Runtime as pallet_governance::Config>::ProposalEnactmentDelay::get();
    let now = delay.checked_add(18).unwrap();
    assert!(prepared_clock < now);
    let check_pending = Governance::prepare_full_pending_enactment_hook(now, false, None).unwrap();
    let pending = pallet_governance::PendingEnactments::<Runtime>::get();
    let capacity = <Runtime as pallet_governance::Config>::MaxPendingEnactments::get();
    let visits = <Runtime as pallet_governance::Config>::MaxPendingEnactmentsPerBlock::get();
    let terminal_expiry = now + <Runtime as pallet_governance::Config>::FinalizedProposalOutcomeRetentionEpochs::get();
    assert_eq!((now, expiry), (43218, 17));
    assert_eq!((capacity, visits), (128, 4));
    assert_eq!(pending.len(), capacity as usize);
    assert!(pending.iter().all(|touch| ![101, 102].contains(&touch.domain)));
    assert_eq!(pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(7).len(), 1020);
    assert_eq!(pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(terminal_expiry).len(), 1020);
    assert_eq!(System::block_number(), now);
    assert_eq!(pallet_governance::LastProcessedEpoch::<Runtime>::get(), expiry - 1);
    assert_eq!(pallet_governance::CurrentEpochServicePhase::<Runtime>::get(), pallet_governance::EpochServicePhase::FinalizedOutcome);
    assert!(!pallet_deos_actors::UserCreationEnabled::<Runtime>::get());
    assert_ok!(<Governance as Hooks<crate::BlockNumber>>::try_state(now));
    let charged = Governance::on_initialize(now);
    check_cleanup().unwrap();
    check_pending(&[]).unwrap();
    assert_eq!(charged, <W as pallet_governance::WeightInfo>::service_pending_enactments(visits)
      .saturating_add(<W as pallet_governance::WeightInfo>::service_epoch_catch_up())
      .saturating_add(<W as pallet_governance::WeightInfo>::service_finalized_proposal_outcomes(2)));
    assert_eq!(pallet_governance::PendingEnactments::<Runtime>::get().len(), 124);
    assert_eq!(pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(terminal_expiry).len(), 1024);
    assert!(pallet_deos_actors::UserCreationEnabled::<Runtime>::get());
    assert_ok!(<Governance as Hooks<crate::BlockNumber>>::try_state(now));
    println!("GOVERNANCE_FULL_INDICES_PENDING_SOURCE_V1 {}", serde_json::json!({
      "clock": now, "oldExpiry": expiry, "completedCursor": expiry - 1, "debt": now - (expiry - 1),
      "cleanupDomains": 2, "cleanupDue": 2, "cleanupPopulation": 32768,
      "pendingBefore": capacity, "pendingVisits": visits, "pendingAfter": 124,
      "dueApprovalEpoch": now - delay, "futureApprovalEpoch": now + 1 - delay,
      "newTerminalExpiry": terminal_expiry, "destinationBeforeAfter": [1020,1024],
      "cleanupAndPendingComponentVerifiers": true, "tryStateBeforeAfter": true,
      "actualUserCreationEffect": true,
      "preparedNotPublicHistoryJointMaximumPhysicalPriceOrPoV": true
    }));
  });
}

#[cfg(all(feature = "runtime-benchmarks", feature = "try-runtime"))]
#[test]
#[ignore = "opt-in joint refusal exact-state model; not invoked-effect rollback or physical pricing"]
fn independent_full_indices_refusal_preserves_pending_and_commits_cleanup() {
  use super::common::committed_fixture_storage;
  use polkadot_sdk::{
    sp_core::{Blake2Hasher, H256},
    sp_trie::{LayoutV1, TrieConfiguration},
  };
  let mut source = new_test_ext();
  let (now, expiry, due, check_cleanup) = source.execute_with(|| {
    let (_, expiry, check_cleanup) =
      Governance::prepare_independent_full_indices_cleanup_component().unwrap();
    let now = <Runtime as pallet_governance::Config>::ProposalEnactmentDelay::get()
      .checked_add(18)
      .unwrap();
    // This verifier's standalone full-root domain excludes committed cleanup. The joint
    // complete-state model below replaces it only here, without weakening that API.
    let _pending_only_check =
      Governance::prepare_full_pending_enactment_hook(now, true, None).unwrap();
    let destination =
      now + <Runtime as pallet_governance::Config>::FinalizedProposalOutcomeRetentionEpochs::get();
    assert_eq!(
      pallet_governance::PendingEnactments::<Runtime>::get().len(),
      128
    );
    assert_eq!(
      pallet_governance::PendingEnactmentCursor::<Runtime>::get(),
      124
    );
    assert_eq!(
      pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(destination).len(),
      1024
    );
    assert!(!pallet_deos_actors::UserCreationEnabled::<Runtime>::get());
    assert_ok!(<Governance as Hooks<crate::BlockNumber>>::try_state(now));
    (
      now,
      expiry,
      pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(expiry),
      check_cleanup,
    )
  });
  let before = committed_fixture_storage(source);
  let mut expected = before.clone();
  assert_eq!(due.len(), 2);
  for touch in due {
    assert!([101, 102].contains(&touch.domain));
    let index_key =
      pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::hashed_key_for(touch.domain);
    let mut ids = Vec::<u32>::decode(&mut &expected.top[&index_key][..]).unwrap();
    assert_eq!(ids.len(), 16384);
    assert_eq!(ids.remove(0), touch.item_id);
    expected.top.insert(index_key, ids.encode());
    assert!(
      expected
        .top
        .remove(
          &pallet_governance::FinalizedProposals::<Runtime>::hashed_key_for(
            touch.domain,
            touch.item_id
          )
        )
        .is_some()
    );
    // This selected NoVotes source has no retained metadata/winning/urgency values.
    for key in [
      pallet_governance::ProposalMetadataByItem::<Runtime>::hashed_key_for(
        touch.domain,
        touch.item_id,
      ),
      pallet_governance::ProposalWinningPrimaryOptionByItem::<Runtime>::hashed_key_for(
        touch.domain,
        touch.item_id,
      ),
      pallet_governance::ProposalUrgentAuthorizedAt::<Runtime>::hashed_key_for(
        touch.domain,
        touch.item_id,
      ),
    ] {
      assert!(!expected.top.contains_key(&key));
    }
  }
  assert!(
    expected
      .top
      .remove(&pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<
        Runtime,
      >::hashed_key_for(expiry))
      .is_some()
  );
  expected.top.insert(
    pallet_governance::CurrentEpochServicePhase::<Runtime>::hashed_key().to_vec(),
    pallet_governance::EpochServicePhase::RewardExpiry.encode(),
  );
  expected.top.insert(
    pallet_governance::PendingEnactmentCursor::<Runtime>::hashed_key().to_vec(),
    0u32.encode(),
  );
  let changed = before
    .top
    .keys()
    .chain(expected.top.keys())
    .collect::<std::collections::BTreeSet<_>>()
    .into_iter()
    .filter(|key| before.top.get(*key) != expected.top.get(*key))
    .count();
  assert_eq!(changed, 7);
  type Layout = LayoutV1<Blake2Hasher>;
  let expected_root = Layout::trie_root(expected.top.clone());
  assert_ne!(
    expected_root,
    Layout::trie_root(before.top.clone()),
    "committed cleanup must not be rolled back"
  );
  let mut actual = polkadot_sdk::sp_io::TestExternalities::new(before);
  let actual_root = actual.execute_with(|| {
    let charged = Governance::on_initialize(now);
    check_cleanup().unwrap();
    assert_eq!(
      pallet_governance::PendingEnactments::<Runtime>::get().len(),
      128
    );
    assert_eq!(
      pallet_governance::PendingEnactmentCursor::<Runtime>::get(),
      0
    );
    assert_eq!(
      pallet_governance::LastProcessedEpoch::<Runtime>::get(),
      expiry - 1
    );
    assert!(!pallet_deos_actors::UserCreationEnabled::<Runtime>::get());
    assert_ok!(<Governance as Hooks<crate::BlockNumber>>::try_state(now));
    type W = crate::weights::pallet_governance::SubstrateWeight<Runtime>;
    assert_eq!(
      charged,
      <W as pallet_governance::WeightInfo>::service_pending_enactments(4)
        .saturating_add(<W as pallet_governance::WeightInfo>::service_epoch_catch_up())
        .saturating_add(
          <W as pallet_governance::WeightInfo>::service_finalized_proposal_outcomes(2)
        )
    );
    H256::from_slice(&polkadot_sdk::sp_io::storage::root(
      crate::VERSION.state_version(),
    ))
  });
  assert_eq!(actual_root, expected_root);
  let actual = committed_fixture_storage(actual);
  assert!(
    actual.top == expected.top,
    "exact combined-state expectation, with no masked namespace"
  );
  println!(
    "GOVERNANCE_FULL_INDICES_PENDING_REFUSAL_V1 {}",
    serde_json::json!({
      "clock": now, "oldExpiry": expiry, "completedCursor": expiry - 1,
      "pendingBeforeAfter": [128,128], "pendingCursorBeforeAfter": [124,0],
      "destinationBeforeAfter": [1024,1024], "cleanupDue": 2, "cleanupPopulation": 32768,
      "exactModeledDeltaKeys": changed, "completeTopAndRootMatch": true,
      "preEffectCapacityRefusalNotInvokedEffectRollback": true,
      "preparedNotPublicHistoryMaximumPoVCpuPriceOrHostAcceptance": true
    })
  );
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn full_index_value_classes_metadata_is_extra_only() {
  use polkadot_sdk::frame_benchmarking::{BenchmarkParameter, Benchmarking};
  let name = b"on_initialize_full_index_value_classes";
  assert!(
    !<Governance as Benchmarking>::benchmarks(false)
      .iter()
      .any(|b| b.name == name)
  );
  let metadata = <Governance as Benchmarking>::benchmarks(true);
  let entries = metadata
    .iter()
    .filter(|b| b.name == name)
    .collect::<Vec<_>>();
  assert_eq!(entries.len(), 1);
  assert_eq!(entries[0].components, vec![(BenchmarkParameter::q, 0, 1)]);
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
#[ignore = "opt-in ordinary SDK returned counters; incidental timings ignored, no Wasm or price"]
fn full_index_value_classes_sdk_return_separates_optional_verification() {
  use polkadot_sdk::frame_benchmarking::{BenchmarkParameter, Benchmarking};
  let mut returned = Vec::new();
  for verify in [false, true] {
    let observed = super::common::with_benchmarking_host(
      super::common::benchmarking_storage(new_test_ext()),
      |_| {
        // Use the real generated dispatcher/BenchmarkRecording, including final commit/wipe.
        let mut rows = <Governance as Benchmarking>::run_benchmark(
          b"on_initialize_full_index_value_classes",
          &[(BenchmarkParameter::q, 1)],
          &[vec![7; 32].into()],
          verify,
          1,
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        let row = rows.remove(0);
        assert_eq!(row.components, vec![(BenchmarkParameter::q, 1)]);
        assert!(row.proof_size >= 131080);
        let counts = (row.reads, row.repeat_reads, row.writes, row.repeat_writes);
        // Do not read, record or compare either incidental SDK timing field.
        (row.proof_size, counts)
      },
    );
    assert_eq!(
      observed.1,
      if verify {
        (32806, 4, 12, 0)
      } else {
        (7, 2, 12, 0)
      }
    );
    println!(
      "GOVERNANCE_VALUE_CLASS_SDK_RETURN_V1 {}",
      serde_json::json!({
        "verify": verify, "selectorQ": 1, "sdkInternalRepeats": 1,
        "returnedProofBytes": observed.0, "returnedCounters": observed.1,
        "ordinaryNativeDispatcherNotStopCallback": true,
        "incidentalTimingsIgnoredNotPhysicalWasmOrPrice": true
      })
    );
    returned.push(observed);
  }
  // Optional post-body verification changes returned counters, not the saved end proof.
  assert_eq!(returned[0].0, returned[1].0);
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
#[ignore = "opt-in v2 value-class boundary; no clock, Wasm or pricing"]
fn full_index_value_classes_v2_records_absolute_body_before_verification() {
  use polkadot_sdk::{
    frame_benchmarking::{Recording, benchmarking},
    sp_runtime::traits::BlakeTwo256,
    sp_state_machine::Backend,
    sp_trie::{StorageProof, recorder::Recorder},
  };
  struct Boundary {
    recorder: Recorder<BlakeTwo256>,
    starts: u32,
    values: Vec<Vec<u8>>,
    proof: Option<StorageProof>,
    absolute: Option<u32>,
    counts: Option<(u32, u32, u32, u32)>,
  }
  impl Recording for Boundary {
    fn start(&mut self) {
      assert_eq!(self.starts, 0);
      self.starts += 1;
      assert!(benchmarking::get_whitelist().is_empty());
      assert_eq!(System::block_number(), 33);
      self.values = [101, 102]
        .map(|d| pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(d).encode())
        .into();
      self.values.sort();
      self.values.dedup();
      benchmarking::commit_db();
      benchmarking::reset_read_write_count();
      assert!(self.recorder.to_storage_proof().is_empty());
      assert_eq!(benchmarking::proof_size(), Some(0));
    }
    fn stop(&mut self) {
      assert!(self.proof.is_none());
      self.proof = Some(self.recorder.to_storage_proof());
      self.absolute = benchmarking::proof_size();
      // Commit bookkeeping only after the end-of-body proof snapshot.
      benchmarking::commit_db();
      self.counts = Some(benchmarking::read_write_count());
    }
  }
  for q in [0, 1] {
    super::common::with_benchmarking_host(
      super::common::benchmarking_storage(new_test_ext()),
      |host| {
        // The selector, not the test recorder, must remove caller-supplied discounts.
        benchmarking::set_whitelist(vec![vec![7; 32].into()]);
        let mut boundary = Boundary {
          recorder: host.recorder().unwrap(),
          starts: 0,
          values: Vec::new(),
          proof: None,
          absolute: None,
          counts: None,
        };
        Governance::verify_full_index_value_classes_benchmark(q, &mut boundary).unwrap();
        assert_eq!(boundary.starts, 1);
        assert_eq!(boundary.values.len(), q as usize + 1);
        let proof = boundary.proof.unwrap();
        for value in &boundary.values {
          assert_eq!(value.len(), 65540);
          assert_eq!(proof.iter_nodes().filter(|node| *node == value).count(), 1);
        }
        let mass = (q + 1) * 65540;
        assert!(boundary.absolute.unwrap() >= mass);
        assert_eq!(boundary.counts, Some((7, 2, 12, 0)));
        assert!(host.get_whitelist().is_empty());
        println!(
          "GOVERNANCE_VALUE_CLASS_V2_BOUNDARY_V1 {}",
          serde_json::json!({
            "q": q, "starts": boundary.starts, "baseline": 0,
            "absoluteCompactBodyProofBytes": boundary.absolute, "rawBodyProofBytes": proof.encoded_size(),
            "requiredPayloadBytes": mass, "postCommitCounts": boundary.counts,
            "snapshotBeforeCommitAndVerification": true, "timerCalled": false,
            "nativeV2NotWasmOrPrice": true
          })
        );
      },
    );
  }
}

#[cfg(all(feature = "runtime-benchmarks", feature = "try-runtime"))]
#[test]
#[ignore = "opt-in matched native SDK value-class source; no timing or physical pricing"]
fn full_index_value_classes_reach_native_sdk_body_recorder() {
  use super::common::{committed_fixture_storage, with_benchmarking_host};
  use polkadot_sdk::{
    frame_benchmarking::benchmarking,
    sp_core::{Blake2Hasher, Hasher},
    sp_state_machine::Backend,
    sp_trie::{LayoutV1, StorageProof, Trie, TrieConfiguration, TrieDBBuilder},
  };
  use std::collections::BTreeSet;
  let keys =
    [101, 102].map(pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::hashed_key_for);
  for distinct in [false, true] {
    let mut source = new_test_ext();
    let (now, expiry, check_hook) = source.execute_with(|| {
      let prepared = Governance::prepare_independent_full_indices_value_contrast(distinct).unwrap();
      assert_ok!(<Governance as Hooks<crate::BlockNumber>>::try_state(
        prepared.0
      ));
      prepared
    });
    let parent = committed_fixture_storage(source);
    let root = LayoutV1::<Blake2Hasher>::trie_root(parent.top.clone());
    let values = keys
      .each_ref()
      .map(|key| parent.top.get(key).unwrap().clone());
    assert!(values.iter().all(|value| value.len() == 65540));
    assert_eq!(values[0] == values[1], !distinct);
    let hashes = values
      .iter()
      .map(|value| Blake2Hasher::hash(value))
      .collect::<BTreeSet<_>>();
    assert_eq!(hashes.len(), if distinct { 2 } else { 1 });
    with_benchmarking_host(parent, |host| {
      let recorder = host.recorder().unwrap();
      assert!(host.get_whitelist().is_empty());
      assert!(recorder.to_storage_proof().is_empty());
      assert_eq!(host.proof_size(), Some(0));
      assert_eq!((System::block_number(), expiry), (33, 17));
      // Discard the context-check read so capture begins from the declared original root.
      benchmarking::commit_db();
      benchmarking::reset_read_write_count();
      assert!(recorder.to_storage_proof().is_empty());
      assert_eq!(host.proof_size(), Some(0));
      let charged = Governance::on_initialize(now);
      let proof = recorder.to_storage_proof();
      let absolute = host.proof_size().unwrap();
      let compact = proof.to_compact_proof::<Blake2Hasher>(root).unwrap();
      assert_eq!(absolute as usize, compact.encoded_size());
      for value in &values {
        assert_eq!(proof.iter_nodes().filter(|node| *node == value).count(), 1);
      }
      let (replay, replay_root) = compact.to_memory_db::<Blake2Hasher>(&root).unwrap();
      assert_eq!(replay_root, root);
      let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&replay, &root).build();
      for (key, value) in keys.iter().zip(&values) {
        assert_eq!(trie.get(key).unwrap().as_ref(), Some(value));
      }
      let missing = StorageProof::new(
        proof
          .iter_nodes()
          .filter(|node| *node != &values[0])
          .cloned(),
      )
      .into_memory_db::<Blake2Hasher>();
      let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&missing, &root).build();
      assert!(trie.get(&keys[0]).is_err());
      if distinct {
        assert_eq!(trie.get(&keys[1]).unwrap().as_ref(), Some(&values[1]));
      } else {
        assert!(trie.get(&keys[1]).is_err());
      }
      // Post-body commit/counters and verification are outside the captured proof scope.
      benchmarking::commit_db();
      let counters = host.read_write_count();
      check_hook().unwrap();
      assert_ok!(<Governance as Hooks<crate::BlockNumber>>::try_state(now));
      type W = crate::weights::pallet_governance::SubstrateWeight<Runtime>;
      assert_eq!(
        charged,
        <W as pallet_governance::WeightInfo>::service_pending_enactments(0)
          .saturating_add(<W as pallet_governance::WeightInfo>::service_epoch_catch_up())
          .saturating_add(
            <W as pallet_governance::WeightInfo>::service_finalized_proposal_outcomes(2)
          )
      );
      println!(
        "GOVERNANCE_MATCHED_VALUE_CLASS_SOURCE_V1 {}",
        serde_json::json!({
          "distinct": distinct, "domains": 2, "due": 2, "population": 32768, "retainVisits": 32768,
          "clock": now, "oldExpiry": expiry, "requiredOriginalValueHashes": hashes.len(),
          "payloadMassBytes": hashes.len() * values[0].len(), "proofBaseline": 0,
          "bodyRawProofBytes": proof.encoded_size(), "bodyAbsoluteCompactProofBytes": absolute,
          "postCommitCounters": [counters.0,counters.1,counters.2,counters.3],
          "proofBeforeCommitVerificationOrRootDiagnostics": true,
          "nativePreparedNotWasmTimingDominationHistoryRootMaximumOrPrice": true
        })
      );
    });
  }
}

#[cfg(all(feature = "runtime-benchmarks", feature = "try-runtime"))]
#[test]
#[ignore = "opt-in independent full-index source; no physical timing or admitted history"]
fn independent_full_indices_cleanup_hook_has_consistent_debt() {
  new_test_ext().execute_with(|| {
    let (now, expiry, check_cleanup) = Governance::prepare_independent_full_indices_cleanup_hook().unwrap();
    assert_eq!((now, expiry), (33, 17));
    assert_eq!(System::block_number(), now);
    assert_eq!(pallet_governance::LastProcessedEpoch::<Runtime>::get(), 16);
    for domain in [101, 102] {
      assert_eq!(pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(domain).len(), 16384);
      assert!(matches!(pallet_governance::FinalizedProposals::<Runtime>::get(domain, 2000 + 16383).unwrap().outcome,
        pallet_governance::FinalizedProposalOutcome::Rejected { finalized_epoch, .. } if finalized_epoch <= now));
    }
    assert!(pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::iter().all(|(epoch, _)| epoch > 16));
    assert_ok!(<Governance as Hooks<crate::BlockNumber>>::try_state(now));
    let charged = Governance::on_initialize(now);
    check_cleanup().unwrap();
    assert_ok!(<Governance as Hooks<crate::BlockNumber>>::try_state(now));
    assert_eq!([101, 102].iter().map(|domain| pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(domain).len()).sum::<usize>(), 32766);
    println!("GOVERNANCE_INDEPENDENT_FULL_INDICES_SOURCE_V1 {}", serde_json::json!({
      "domains": 2, "due": 2, "indexIdsPerDomain": 16384, "initialPopulation": 32768,
      "clock": now, "selectedExpiry": expiry, "completedCursor": 16, "debt": now - 16,
      "equalIndexValues": true, "tryStateBeforeAfter": true,
      "declaredWeightNotPhysicalCost": [charged.ref_time(), charged.proof_size()],
      "preparedNotPublicHistoryExecutiveAdmissionOrPhysicalPrice": true
    }));
  });
}

#[cfg(all(feature = "runtime-benchmarks", feature = "try-runtime"))]
#[test]
#[ignore = "opt-in full-index original-input/delta transfer; not physical pricing"]
fn independent_full_indices_original_input_replays_whole_delta() {
  use super::{common::committed_fixture_storage, h1_storage_trace};
  use polkadot_sdk::{
    sp_core::{Blake2Hasher, H256, Hasher},
    sp_trie::{
      EMPTY_PREFIX, HashDBT, LayoutV1, MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration,
      TrieDBBuilder, delta_trie_root,
    },
  };
  use std::collections::BTreeSet;
  type Layout = LayoutV1<Blake2Hasher>;
  let mut source = new_test_ext();
  let (now, _, check_cleanup) = source.execute_with(|| {
    let prepared = Governance::prepare_independent_full_indices_cleanup_hook().unwrap();
    assert_ok!(<Governance as Hooks<crate::BlockNumber>>::try_state(
      prepared.0
    ));
    prepared
  });
  let before_root =
    source.execute_with(|| polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()));
  let before = committed_fixture_storage(source);
  let mut plain = polkadot_sdk::sp_io::TestExternalities::new(before.clone());
  let plain_weight = plain.execute_with(|| Governance::on_initialize(now));
  let plain = committed_fixture_storage(plain);
  let mut observed = polkadot_sdk::sp_io::TestExternalities::new(before.clone());
  let ((observed_weight, access), runtime_root) = h1_storage_trace::recorded(|| {
    observed.execute_with(|| {
      let result = h1_storage_trace::point_access_keys("governance-cleanup", || {
        Governance::on_initialize(now)
      });
      check_cleanup().unwrap();
      assert_ok!(<Governance as Hooks<crate::BlockNumber>>::try_state(now));
      (
        result,
        polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
      )
    })
  });
  let after = committed_fixture_storage(observed);
  let (keys, written) = (access.all, access.written);
  assert_eq!(plain_weight, observed_weight);
  assert_eq!(plain.top, after.top, "observer preserves complete TOP");
  let all_keys = before
    .top
    .keys()
    .chain(after.top.keys())
    .collect::<BTreeSet<_>>();
  let delta = all_keys
    .into_iter()
    .filter(|key| before.top.get(*key) != after.top.get(*key))
    .map(|key| (key.clone(), after.top.get(key).cloned()))
    .collect::<Vec<_>>();
  assert!(
    delta.iter().all(|(key, _)| written.contains(key)),
    "every net mutation has an observed write owner"
  );
  assert!(keys.contains(
    polkadot_sdk::frame_support::storage::storage_prefix(b"System", b"Number").as_slice()
  ));
  let mut original = MemoryDB::<Blake2Hasher>::default();
  let root = Layout::trie_build(&mut original, before.top.clone());
  assert_eq!(root, H256::from_slice(&before_root));
  assert_eq!(root, Layout::trie_root(before.top.clone()));
  let mut recorder = Recorder::<Layout>::new();
  {
    let trie = TrieDBBuilder::<Layout>::new(&original, &root)
      .with_recorder(&mut recorder)
      .build();
    for key in &keys {
      assert_eq!(trie.get(key).unwrap(), before.top.get(key).cloned());
    }
  }
  let input_nodes = recorder
    .drain()
    .into_iter()
    .map(|node| node.data)
    .collect::<BTreeSet<_>>();
  let input = StorageProof::new(input_nodes.iter().cloned());
  let mut lookup_only = input.to_memory_db::<Blake2Hasher>();
  let lookup_only_sufficient =
    delta_trie_root::<Layout, _, _, _, _, _>(&mut lookup_only, root, delta.clone(), None, None)
      .is_ok();
  let mut working = original.clone();
  let mut mutation_recorder = Recorder::<Layout>::new();
  let post_root = delta_trie_root::<Layout, _, _, _, _, _>(
    &mut working,
    root,
    delta.clone(),
    Some(&mut mutation_recorder),
    None,
  )
  .unwrap();
  assert_eq!(post_root, H256::from_slice(&runtime_root));
  assert_eq!(post_root, Layout::trie_root(after.top.clone()));
  let mut repair_inputs = BTreeSet::new();
  let mut computed_nodes = 0usize;
  for node in mutation_recorder
    .drain()
    .into_iter()
    .map(|record| record.data)
  {
    if input_nodes.contains(&node) {
      continue;
    }
    if HashDBT::get(&original, &Blake2Hasher::hash(&node), EMPTY_PREFIX).as_ref() == Some(&node) {
      repair_inputs.insert(node);
    } else {
      computed_nodes += 1;
    }
  }
  let witness = StorageProof::new(
    input_nodes
      .iter()
      .cloned()
      .chain(repair_inputs.iter().cloned()),
  );
  assert!(
    witness.iter_nodes().all(|node| HashDBT::get(
      &original,
      &Blake2Hasher::hash(node),
      EMPTY_PREFIX
    )
    .as_ref()
      == Some(node)),
    "no computed/new node is supplied as original input"
  );
  let compact = witness.to_compact_proof::<Blake2Hasher>(root).unwrap();
  let (mut replay, replay_root) = compact.to_memory_db::<Blake2Hasher>(&root).unwrap();
  assert_eq!(replay_root, root);
  {
    let trie = TrieDBBuilder::<Layout>::new(&replay, &root).build();
    for key in &keys {
      assert_eq!(trie.get(key).unwrap(), before.top.get(key).cloned());
    }
  }
  assert_eq!(
    delta_trie_root::<Layout, _, _, _, _, _>(&mut replay, root, delta.clone(), None, None).unwrap(),
    post_root
  );
  let index_keys =
    [101, 102].map(pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::hashed_key_for);
  let shared_value = before.top.get(&index_keys[0]).unwrap();
  assert_eq!(Some(shared_value), before.top.get(&index_keys[1]));
  assert_eq!(
    witness
      .iter_nodes()
      .filter(|node| *node == shared_value)
      .count(),
    1
  );
  let missing = StorageProof::new(
    witness
      .iter_nodes()
      .filter(|node| *node != shared_value)
      .cloned(),
  )
  .into_memory_db::<Blake2Hasher>();
  let trie = TrieDBBuilder::<Layout>::new(&missing, &root).build();
  for key in &index_keys {
    assert!(trie.get(key).is_err());
  }
  println!(
    "GOVERNANCE_FULL_INDEX_DELTA_TRANSFER_V1 {}",
    serde_json::json!({
      "domains": 2, "initialPopulation": 32768, "clock": now,
      "pointKeys": keys.len(), "netDeltaKeys": delta.len(),
      "originalInputNodes": input_nodes.len(), "extraOriginalRepairNodes": repair_inputs.len(),
      "excludedComputedNodes": computed_nodes, "lookupOnlyDeltaSucceeded": lookup_only_sufficient,
      "rawWitnessBytes": witness.encoded_size(), "compactWitnessBytes": compact.encoded_size(),
      "plainTopAndWeightPreserved": true, "independentPostRootMatches": true,
      "explicitSharedValueAbsenceFailsReads": true,
      "inputSupersetNotMinimalProofCpuPVFPoVOrHistory": true
    })
  );
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn finalized_cleanup_hook_sdk_footprint_keeps_wrapper_owners() {
  use polkadot_sdk::frame_benchmarking::benchmarking;
  let mut source = new_test_ext();
  let (domain, expiry) =
    source.execute_with(|| Governance::prepare_maximum_finalized_cleanup_hook().unwrap());
  let parent = super::common::benchmarking_storage(source);
  for phase in [
    pallet_governance::EpochServicePhase::Maturing,
    pallet_governance::EpochServicePhase::FinalizedOutcome,
  ] {
    super::common::with_benchmarking_host(parent.clone(), |_| {
      System::set_block_number(expiry);
      pallet_governance::LastProcessedEpoch::<Runtime>::put(expiry - 1);
      pallet_governance::CurrentEpochServicePhase::<Runtime>::put(phase);
      assert!(pallet_governance::PendingEnactments::<Runtime>::get().is_empty());
      benchmarking::set_whitelist(Vec::new());
      benchmarking::commit_db();
      benchmarking::reset_read_write_count();
      let charged = Governance::on_initialize(expiry);
      // Commit the actual SDK bookkeeping before any verification read; no timer/PoV query.
      benchmarking::commit_db();
      let counts = benchmarking::read_write_count();
      let rows = benchmarking::get_read_and_written_keys();
      let prefix = |pallet: &[u8], item: &[u8]| {
        polkadot_sdk::frame_support::storage::storage_prefix(pallet, item).to_vec()
      };
      let maturity = prefix(b"Governance", b"ProposalMaturityBuckets");
      let initial_maturing = phase == pallet_governance::EpochServicePhase::Maturing;
      let expected_reads = if initial_maturing { 7 } else { 6 };
      assert_eq!((counts.0, counts.2), (expected_reads, 4099));
      assert_eq!(rows.len(), if initial_maturing { 11 } else { 10 });
      assert!(rows.iter().all(|(_, _, _, whitelisted)| !whitelisted));
      for (pallet, item, writes) in [
        (b"System".as_slice(), b"Number".as_slice(), 0),
        (b"Governance".as_slice(), b"PendingEnactments".as_slice(), 0),
        (
          b"Governance".as_slice(),
          b"LastProcessedEpoch".as_slice(),
          0,
        ),
        (
          b"Governance".as_slice(),
          b"CurrentEpochServicePhase".as_slice(),
          1,
        ),
      ] {
        let key = prefix(pallet, item);
        let row = rows.iter().find(|row| row.0 == key).unwrap();
        assert_eq!((row.1, row.2), (1, writes));
      }
      let maturity_row = rows.iter().find(|row| row.0 == maturity);
      if initial_maturing {
        let row = maturity_row.unwrap();
        assert_eq!((row.1, row.2), (1, 0));
      } else {
        assert!(maturity_row.is_none());
      }
      assert_eq!(
        pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(domain).len(),
        15360
      );
      assert!(
        pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(expiry).is_empty()
      );
      assert_eq!(
        pallet_governance::CurrentEpochServicePhase::<Runtime>::get(),
        pallet_governance::EpochServicePhase::RewardExpiry
      );
      assert_eq!(
        pallet_governance::LastProcessedEpoch::<Runtime>::get(),
        expiry - 1
      );
      println!(
        "GOVERNANCE_CLEANUP_HOOK_SDK_FOOTPRINT_V1 {}",
        serde_json::json!({
          "initialPhase": format!("{phase:?}"), "readWriteCount": counts,
          "prefixRows": rows.len(), "charged": [charged.ref_time(), charged.proof_size()],
          "emptyPendingOnly": true, "actualHookNotHelper": true,
          "nativeSdkCountersNotIOOrPhysicalPrice": true,
          "noTimingOrProofSizeQuery": true, "notExecutiveOrPVFAcceptance": true
        })
      );
    });
  }
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
#[ignore = "opt-in large-index logical trace; not physical timing"]
fn finalized_cleanup_index_logical_volume_preserves_plain_hook() {
  use super::h1_storage_trace;
  let mut source = new_test_ext();
  let (domain, expiry) =
    source.execute_with(|| Governance::prepare_maximum_finalized_cleanup_hook().unwrap());
  let parent = super::common::benchmarking_storage(source);
  let n = 1024usize;
  let m = 16384usize;
  let encoded_len = |len: usize| codec::Compact(len as u32).encoded_size() + 4 * len;
  let execute = |observe| {
    polkadot_sdk::sp_io::TestExternalities::new(parent.clone()).execute_with(|| {
      System::set_block_number(expiry);
      pallet_governance::LastProcessedEpoch::<Runtime>::put(expiry - 1);
      pallet_governance::CurrentEpochServicePhase::<Runtime>::put(
        pallet_governance::EpochServicePhase::FinalizedOutcome,
      );
      assert!(pallet_governance::PendingEnactments::<Runtime>::get().is_empty());
      assert!(pallet_governance::ProposalMaturityBuckets::<Runtime>::get(expiry).is_empty());
      let before = pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(domain);
      let due = pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(expiry);
      assert_eq!(before.len(), m);
      assert_eq!(before.encoded_size(), encoded_len(m));
      assert_eq!(due.len(), n);
      assert_eq!(
        due.iter().map(|touch| touch.item_id).collect::<Vec<_>>(),
        before[..n]
      );
      let pre_root = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
      let key = pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::hashed_key_for(domain);
      let (charged, operations) = if observe {
        h1_storage_trace::key_operations("governance-cleanup", &key, || {
          Governance::on_initialize(expiry)
        })
      } else {
        (Governance::on_initialize(expiry), Vec::new())
      };
      let post_root = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
      assert_eq!(
        pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(domain).as_slice(),
        &before[n..]
      );
      assert!(
        pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(expiry).is_empty()
      );
      assert_eq!(
        pallet_governance::CurrentEpochServicePhase::<Runtime>::get(),
        pallet_governance::EpochServicePhase::RewardExpiry
      );
      assert_eq!(
        pallet_governance::LastProcessedEpoch::<Runtime>::get(),
        expiry - 1
      );
      (pre_root, post_root, charged, operations)
    })
  };
  let plain = execute(false);
  let observed = h1_storage_trace::recorded(|| execute(true));
  assert_eq!(
    (&plain.0, &plain.1, plain.2),
    (&observed.0, &observed.1, observed.2)
  );
  assert!(plain.3.is_empty());
  assert_eq!(observed.3.len(), 2 * n);
  let mut returned = 0usize;
  let mut submitted = 0usize;
  for (j, pair) in observed.3.chunks_exact(2).enumerate() {
    assert_eq!(pair[0], ("Get".into(), encoded_len(m - j), false));
    assert_eq!(pair[1], ("Put".into(), encoded_len(m - j - 1), false));
    returned += pair[0].1;
    submitted += pair[1].1;
  }
  let predicate_visits = (0..n).map(|j| m - j).sum::<usize>();
  assert_eq!(
    (returned, submitted, predicate_visits),
    (65_015_810, 65_011_712, 16_253_440)
  );
  println!(
    "GOVERNANCE_INDEX_LOGICAL_WORK_V1 {}",
    serde_json::json!({
      "due": n, "initialIndexIds": m, "keys": 1,
      "logicalGets": n, "logicalPuts": n,
      "returnedValueBytes": returned, "submittedValueBytes": submitted,
      "sourceRetainPredicateVisits": predicate_visits,
      "originalValueBytes": encoded_len(m), "finalValueBytes": encoded_len(m - n),
      "plainRootAndWeightPreserved": true, "setupAndVerificationExcluded": true,
      "notCpuBackendIOProofPriceOrAdmittedHistory": true
    })
  );
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn finalized_cleanup_initialization_exposes_system_quarter_mismatch() {
  new_test_ext().execute_with(|| {
    type W = crate::weights::pallet_governance::SubstrateWeight<Runtime>;
    let (domain, expiry) = Governance::prepare_maximum_finalized_cleanup_hook().unwrap();
    let n = <Runtime as pallet_governance::Config>::MaxFinalizedProposalOutcomesPerBlock::get();
    let before = pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(domain);
    let future = pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::iter()
      .filter(|(epoch, _)| *epoch > expiry)
      .collect::<Vec<_>>();
    assert_eq!(before.len(), 16_384);
    assert_eq!(n, 1_024);
    assert!(pallet_governance::PendingEnactments::<Runtime>::get().is_empty());
    let issuance = Balances::total_issuance();
    let alice = System::account(ALICE);
    let bob = System::account(BOB);
    System::set_block_number(expiry);
    pallet_governance::LastProcessedEpoch::<Runtime>::put(expiry - 1);
    let charged = Governance::on_initialize(expiry);
    let expected = <W as pallet_governance::WeightInfo>::service_pending_enactments(0)
      .saturating_add(<W as pallet_governance::WeightInfo>::service_epoch_catch_up())
      .saturating_add(<W as pallet_governance::WeightInfo>::service_finalized_proposal_outcomes(n));
    assert_eq!(charged, expected);
    let quarter = crate::MAXIMUM_BLOCK_WEIGHT / 4;
    let index_bytes = before.encoded_size();
    assert_eq!(index_bytes, 65_540);
    assert!(
      charged.proof_size() < index_bytes as u64,
      "known owner coverage gap: full retained index is read by cleanup"
    );
    assert!(
      charged.ref_time() > quarter.ref_time(),
      "known configured policy violation"
    );
    assert!(charged.proof_size() <= quarter.proof_size());
    assert!(charged.all_lte(crate::configs::governance_config::GovernanceFixedWeight::get()));
    assert_eq!(
      pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(domain).as_slice(),
      &before[n as usize..]
    );
    for item in before.iter().take(n as usize) {
      assert!(!pallet_governance::FinalizedProposals::<Runtime>::contains_key(domain, item));
      assert!(!pallet_governance::ProposalMetadataByItem::<Runtime>::contains_key(domain, item));
    }
    for (epoch, bucket) in future {
      assert_eq!(
        pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(epoch),
        bucket
      );
    }
    assert!(
      pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(expiry).is_empty()
    );
    assert_eq!(
      pallet_governance::CurrentEpochServicePhase::<Runtime>::get(),
      pallet_governance::EpochServicePhase::RewardExpiry
    );
    assert_eq!(Balances::total_issuance(), issuance);
    assert_eq!(System::account(ALICE), alice);
    assert_eq!(System::account(BOB), bob);
    println!(
      "GOVERNANCE_FIXED_CLEANUP_CALLER_V1 {}",
      serde_json::json!({
        "syntheticBenchmarkState": true, "notPhysicalTiming": true, "due": n,
        "remaining": before.len() - n as usize, "expiry": expiry,
        "retainedIndexEncodedBytes": index_bytes, "codecBytesAreNotMeasuredProof": true,
        "charged": [charged.ref_time(), charged.proof_size()],
        "quarter": [quarter.ref_time(), quarter.proof_size()]
      })
    );
  });
}

#[test]
fn finalized_cleanup_due_bucket_admits_distinct_domain_indices() {
  new_test_ext().execute_with(|| {
    let domains = [101, 102];
    let item = 7;
    let hash = note_advisory_preimage(ALICE, b"domain-diverse cleanup source");
    for (birth, item) in [(1, 7), (2, 8)] {
      System::set_block_number(birth);
      for domain in domains {
        assert_ok!(Governance::submit_proposal(
          RuntimeOrigin::root(),
          domain,
          item,
          ALICE,
          pallet_governance::ProposalCadenceMode::Ordinary,
          pallet_governance::ProposalPayloadKind::Intent,
          hash,
        ));
        assert_ok!(Governance::resolve_proposal(
          RuntimeOrigin::root(),
          domain,
          item,
          vec![ALICE].try_into().unwrap(),
        ));
        assert_eq!(
          pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(domain).as_slice(),
          if birth == 1 { &[7][..] } else { &[7, 8][..] }
        );
        assert!(pallet_governance::FinalizedProposals::<Runtime>::contains_key(domain, item));
      }
    }
    let due = Governance::proposal_pending_enactment_at(domains[0], item).unwrap();
    assert_eq!(
      Governance::proposal_pending_enactment_at(domains[0], 8),
      Some(due + 1)
    );
    assert_eq!(
      Governance::proposal_pending_enactment_at(domains[1], item),
      Some(due)
    );
    System::set_block_number(due);
    Governance::on_initialize(due);
    for domain in domains {
      assert!(Governance::proposal_pending_enactment_at(domain, item).is_none());
    }
    System::set_block_number(due + 1);
    Governance::on_initialize(due + 1);
    let buckets = pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::iter()
      .filter(|(_, entries)| entries.iter().any(|e| domains.contains(&e.domain)))
      .collect::<Vec<_>>();
    assert_eq!(buckets.len(), 2, "adjacent public finalized expiry cohorts");
    let expiry = buckets.iter().map(|(epoch, _)| *epoch).min().unwrap();
    let due_bucket =
      pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(expiry);
    let future_bucket =
      pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(expiry + 1);
    for domain in domains {
      assert!(
        due_bucket
          .iter()
          .any(|e| e.domain == domain && e.item_id == item)
      );
    }
    assert_ne!(
      pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::hashed_key_for(domains[0]),
      pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::hashed_key_for(domains[1])
    );
    let future = domains.map(|domain| {
      (
        pallet_governance::FinalizedProposals::<Runtime>::get(domain, 8),
        pallet_governance::ProposalMetadataByItem::<Runtime>::get(domain, 8),
      )
    });
    let balances = (
      Balances::total_issuance(),
      System::account(ALICE),
      System::account(BOB),
    );
    // Explicit maintenance-cursor fixture, not a claim of full public block history.
    pallet_governance::LastProcessedEpoch::<Runtime>::put(expiry - 1);
    System::set_block_number(expiry);
    let charged = Governance::on_initialize(expiry);
    type W = crate::weights::pallet_governance::SubstrateWeight<Runtime>;
    assert_eq!(
      charged,
      <W as pallet_governance::WeightInfo>::service_pending_enactments(0)
        .saturating_add(<W as pallet_governance::WeightInfo>::service_epoch_catch_up())
        .saturating_add(
          <W as pallet_governance::WeightInfo>::service_finalized_proposal_outcomes(2)
        )
    );
    for (index, domain) in domains.into_iter().enumerate() {
      assert_eq!(
        pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(domain).as_slice(),
        &[8]
      );
      assert!(!pallet_governance::FinalizedProposals::<Runtime>::contains_key(domain, item));
      assert!(!pallet_governance::ProposalMetadataByItem::<Runtime>::contains_key(domain, item));
      assert_eq!(
        pallet_governance::FinalizedProposals::<Runtime>::get(domain, 8),
        future[index].0
      );
      assert_eq!(
        pallet_governance::ProposalMetadataByItem::<Runtime>::get(domain, 8),
        future[index].1
      );
    }
    assert!(
      pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(expiry).is_empty()
    );
    assert_eq!(
      pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(expiry + 1),
      future_bucket
    );
    assert_eq!(
      (
        Balances::total_issuance(),
        System::account(ALICE),
        System::account(BOB)
      ),
      balances
    );
    type Index = polkadot_sdk::frame_support::BoundedVec<
      u32,
      <Runtime as pallet_governance::Config>::MaxRecentFinalizedProposalsPerDomain,
    >;
    type Bucket = polkadot_sdk::frame_support::BoundedVec<
      pallet_governance::FinalizedProposalTouch<u32, u32>,
      <Runtime as pallet_governance::Config>::MaxFinalizedProposalOutcomesPerEpoch,
    >;
    let index_value_upper = Index::max_encoded_len();
    let bucket_value_upper = Bucket::max_encoded_len();
    let two_domain_value_upper = bucket_value_upper
      .checked_add(index_value_upper.checked_mul(2).unwrap())
      .unwrap();
    println!(
      "GOVERNANCE_CLEANUP_DOMAIN_SOURCE_V1 {}",
      serde_json::json!({
        "publicRootProposalAndResolution": true, "domains": domains, "item": item,
        "expiry": expiry, "retainedIndexKeys": 2,
        "syntheticCleanupCursor": true, "removed": 2, "futureRetained": 2,
        "maximumIndexValueBytes": index_value_upper, "maximumBucketValueBytes": bucket_value_upper,
        "twoDomainValueUpperNotProofOrReachableLower": two_domain_value_upper,
        "charged": [charged.ref_time(), charged.proof_size()],
        "notMaximumJointGeometryOrMeasuredProof": true
      })
    );
  });
}

#[test]
fn cleanup_co_read_estimate_does_not_collapse_distinct_keys() {
  use polkadot_sdk::frame_support::traits::StorageInfoTrait;
  let information = <Governance as StorageInfoTrait>::storage_info();
  let metadata = |name: &[u8]| {
    information
      .iter()
      .find(|entry| entry.storage_name == name)
      .unwrap()
  };
  let bucket = metadata(b"FinalizedProposalOutcomeExpiryBuckets");
  let index = metadata(b"FinalizedProposalIdsByDomain");
  assert_eq!(bucket.max_size, Some(8_214));
  assert_eq!(index.max_size, Some(65_560));
  assert_eq!(bucket.max_values, None);
  assert_eq!(index.max_values, None);
  for (name, size) in [
    (b"FinalizedProposals".as_slice(), 274),
    (b"ProposalMetadataByItem".as_slice(), 74),
    (b"ProposalWinningPrimaryOptionByItem".as_slice(), 41),
    (b"ProposalUrgentAuthorizedAt".as_slice(), 44),
  ] {
    assert_eq!(metadata(name).max_size, Some(size));
  }
  assert_eq!(
    pallet_governance::FinalizedProposals::<Runtime>::hashed_key_for(101, 110).len(),
    72
  );
  assert_eq!(
    pallet_governance::ProposalMetadataByItem::<Runtime>::hashed_key_for(101, 110).len(),
    72
  );
  assert_eq!(
    pallet_governance::ProposalWinningPrimaryOptionByItem::<Runtime>::hashed_key_for(101, 110)
      .len(),
    72
  );
  assert_eq!(
    pallet_governance::ProposalUrgentAuthorizedAt::<Runtime>::hashed_key_for(101, 110).len(),
    72
  );
  // Conditional SDK model only: five map layers at default1M entries plus two prefix layers.
  // Neither default cardinality nor balanced trie depth is certified by these metadata rows.
  let path_estimate = 7u64 * 15 * 33;
  let bucket_estimate = u64::from(bucket.max_size.unwrap()) + path_estimate;
  let index_estimate = u64::from(index.max_size.unwrap()) + path_estimate;
  let composed = |distinct_domains: u64| {
    bucket_estimate
      .checked_add(index_estimate.checked_mul(distinct_domains).unwrap())
      .unwrap()
  };
  assert_eq!((bucket_estimate, index_estimate), (11_679, 69_025));
  assert_eq!(composed(1), 80_704);
  assert_eq!(composed(2), 149_729);
  assert!(
    bucket_estimate.max(index_estimate) < 74_428,
    "rejected generated max-prefix estimate misses retained recorded read proof"
  );
  assert!(
    composed(1) >= 74_428,
    "finite observed coverage is not a production upper"
  );
  println!(
    "GOVERNANCE_CLEANUP_CONDITIONAL_PROOF_V1 {}",
    serde_json::json!({
      "assumedMapEntries": 1_000_000, "assumedMapDepth": 5, "prefixLayers": 2,
      "mapCardinalityMetadataBounded": false,
      "perBucket": bucket_estimate, "perIndex": index_estimate,
      "oneDomain": composed(1), "twoDomains": composed(2),
      "notProductionBound": true
    })
  );
}

#[test]
fn cleanup_equal_domain_indices_share_proof_value_not_logical_work() {
  use polkadot_sdk::{
    frame_support::BoundedVec,
    sp_core::Blake2Hasher,
    sp_trie::{
      LayoutV1, MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder,
      TrieLayout,
    },
  };
  type Layout = LayoutV1<Blake2Hasher>;
  type Index =
    BoundedVec<u32, <Runtime as pallet_governance::Config>::MaxRecentFinalizedProposalsPerDomain>;
  assert_eq!(Layout::MAX_INLINE_VALUE, Some(33));
  let maximum = <Runtime as pallet_governance::Config>::MaxRecentFinalizedProposalsPerDomain::get();
  assert_eq!(maximum, 16384);
  let index: Index = (2000..2000 + maximum)
    .collect::<Vec<_>>()
    .try_into()
    .unwrap();
  let keys =
    [101u32, 102].map(pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::hashed_key_for);
  assert_ne!(keys[0], keys[1]);
  assert!(keys.iter().all(|key| key.len() == 52));
  let population = 2 * u64::from(maximum);
  let mut sizes = Vec::new();
  for shared in [true, false] {
    let mut other = index.clone();
    if !shared {
      other[maximum as usize - 1] += 1;
    }
    let values = [index.encode(), other.encode()];
    assert_eq!(values[0].len(), 65540);
    assert_eq!(values[0].len(), values[1].len());
    let entries = keys
      .iter()
      .cloned()
      .zip(values.iter().cloned())
      .collect::<Vec<_>>();
    let mut database = MemoryDB::<Blake2Hasher>::default();
    let root = Layout::trie_build(&mut database, entries.clone());
    assert_eq!(root, Layout::trie_root(entries));
    let mut recorder = Recorder::<Layout>::new();
    {
      let trie = TrieDBBuilder::<Layout>::new(&database, &root)
        .with_recorder(&mut recorder)
        .build();
      for (key, value) in keys.iter().zip(&values) {
        assert_eq!(trie.get(key).unwrap().as_ref(), Some(value));
      }
    }
    let proof = StorageProof::new(recorder.drain().into_iter().map(|node| node.data));
    let payloads = proof
      .iter_nodes()
      .filter(|node| values.contains(node))
      .count();
    assert_eq!(payloads, if shared { 1 } else { 2 });
    let compact = proof.to_compact_proof::<Blake2Hasher>(root).unwrap();
    assert_eq!(
      compact
        .encoded_nodes
        .iter()
        .filter(|node| values.contains(node))
        .count(),
      payloads
    );
    let (replay, replay_root) = compact.to_memory_db::<Blake2Hasher>(&root).unwrap();
    assert_eq!(replay_root, root);
    let trie = TrieDBBuilder::<Layout>::new(&replay, &replay_root).build();
    let mut visits = 0u64;
    for (key, value) in keys.iter().zip(&values) {
      let bytes = trie.get(key).unwrap().unwrap();
      assert_eq!(&bytes, value);
      let mut ids = Index::decode(&mut &bytes[..]).unwrap().into_inner();
      ids.retain(|id| {
        visits += 1;
        *id != 2000
      });
      assert_eq!(ids.len(), maximum as usize - 1);
    }
    assert_eq!(visits, population, "source filtering is still per domain");
    let without_value = StorageProof::new(
      proof
        .iter_nodes()
        .filter(|node| **node != values[0])
        .cloned(),
    )
    .into_memory_db::<Blake2Hasher>();
    let incomplete = TrieDBBuilder::<Layout>::new(&without_value, &root).build();
    assert!(incomplete.get(&keys[0]).is_err());
    if shared {
      assert!(incomplete.get(&keys[1]).is_err());
      assert!(
        (compact.encoded_size() as u64) < 4 * population,
        "per-domain 4S is not a proof-size floor"
      );
    } else {
      assert_eq!(incomplete.get(&keys[1]).unwrap().as_ref(), Some(&values[1]));
    }
    sizes.push((proof.encoded_size(), compact.encoded_size()));
    println!(
      "GOVERNANCE_INDEX_VALUE_ALIAS_V1 {}",
      serde_json::json!({
        "shared": shared, "domains": 2, "population": population,
        "uniqueValueBlobs": payloads, "valueBytes": values[0].len(),
        "rawProofBytes": proof.encoded_size(), "compactProofBytes": compact.encoded_size(),
        "sourceRetainVisits": visits, "requiredValueAbsenceChecked": true,
        "inputProjectionNotGovernanceStatePoVHistoryCpuOrPrice": true
      })
    );
  }
  assert!(sizes[0].0 < sizes[1].0 && sizes[0].1 < sizes[1].1);
}

#[test]
fn cleanup_required_external_value_mass_counts_hash_classes_not_domains() {
  use polkadot_sdk::{
    frame_support::BoundedVec,
    sp_core::{Blake2Hasher, Hasher},
    sp_trie::{
      LayoutV1, MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder,
      TrieLayout,
    },
  };
  use std::collections::BTreeMap;
  type Layout = LayoutV1<Blake2Hasher>;
  type Index =
    BoundedVec<u32, <Runtime as pallet_governance::Config>::MaxRecentFinalizedProposalsPerDomain>;
  assert_eq!(Layout::MAX_INLINE_VALUE, Some(33));
  let maximum = <Runtime as pallet_governance::Config>::MaxRecentFinalizedProposalsPerDomain::get();
  assert_eq!(maximum, 16384);
  let mut entries = Vec::new();
  let mut by_hash = BTreeMap::new();
  for ordinal in 0u32..41 {
    let class = if ordinal == 40 { 0 } else { ordinal };
    let start = 2000 + class * maximum;
    let ids: Index = (start..start + maximum)
      .collect::<Vec<_>>()
      .try_into()
      .unwrap();
    let value = ids.encode();
    assert_eq!(value.len(), 65540);
    let hash = Blake2Hasher::hash(&value);
    if let Some(previous) = by_hash.insert(hash, value.clone()) {
      assert_eq!(previous, value);
    }
    let key =
      pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::hashed_key_for(101 + ordinal);
    assert_eq!(key.len(), 52);
    entries.push((key, value));
  }
  assert_eq!(entries.len(), 41);
  assert_eq!(by_hash.len(), 40);
  assert_eq!(entries[0].1, entries[40].1);
  let absence_values = [entries[0].1.clone(), entries[39].1.clone()];
  // trie_build requires full storage-key order, not numeric domain order.
  entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
  let mass = by_hash
    .values()
    .map(|value| value.len() as u64)
    .sum::<u64>();
  assert_eq!(mass, 2621600);
  assert!(
    mass
      < entries
        .iter()
        .map(|(_, value)| value.len() as u64)
        .sum::<u64>()
  );
  let quarter = (crate::MAXIMUM_BLOCK_WEIGHT / 4).proof_size();
  assert_eq!(mass.checked_sub(quarter), Some(160));
  let mut database = MemoryDB::<Blake2Hasher>::default();
  let root = Layout::trie_build(&mut database, entries.clone());
  assert_eq!(root, Layout::trie_root(entries.clone()));
  let mut recorder = Recorder::<Layout>::new();
  {
    let trie = TrieDBBuilder::<Layout>::new(&database, &root)
      .with_recorder(&mut recorder)
      .build();
    for (key, value) in &entries {
      assert_eq!(trie.get(key).unwrap().as_ref(), Some(value));
    }
  }
  let proof = StorageProof::new(recorder.drain().into_iter().map(|node| node.data));
  let compact = proof.to_compact_proof::<Blake2Hasher>(root).unwrap();
  for value in by_hash.values() {
    assert_eq!(proof.iter_nodes().filter(|node| *node == value).count(), 1);
    assert_eq!(
      compact
        .encoded_nodes
        .iter()
        .filter(|node| *node == value)
        .count(),
      1
    );
  }
  assert!(proof.encoded_size() as u64 >= mass);
  assert!(compact.encoded_size() as u64 >= mass);
  let (replay, replay_root) = compact.to_memory_db::<Blake2Hasher>(&root).unwrap();
  assert_eq!(replay_root, root);
  let trie = TrieDBBuilder::<Layout>::new(&replay, &root).build();
  for (key, value) in &entries {
    let actual = trie.get(key).unwrap().unwrap();
    assert_eq!(&actual, value);
    let mut input = &actual[..];
    assert_eq!(Index::decode(&mut input).unwrap().len(), maximum as usize);
    assert!(input.is_empty());
  }
  // One representative per multiplicity class: shared blob and private blob.
  for absent in absence_values {
    let missing = StorageProof::new(proof.iter_nodes().filter(|node| *node != &absent).cloned())
      .into_memory_db::<Blake2Hasher>();
    let trie = TrieDBBuilder::<Layout>::new(&missing, &root).build();
    for (key, value) in &entries {
      if value == &absent {
        assert!(trie.get(key).is_err());
      } else {
        assert_eq!(trie.get(key).unwrap().as_ref(), Some(value));
      }
    }
  }
  println!(
    "GOVERNANCE_REQUIRED_EXTERNAL_MASS_V1 {}",
    serde_json::json!({
      "domains": entries.len(), "logicalPopulation": u64::from(maximum) * entries.len() as u64,
      "distinctRequiredValueHashes": by_hash.len(), "valueBytes": 65540,
      "distinctPayloadMassBytes": mass, "rawProofBytes": proof.encoded_size(), "compactProofBytes": compact.encoded_size(),
      "numericQuarterProofBytes": quarter, "payloadExcessOverNumericQuarter": mass - quarter,
      "rawCompactRoundTripAndRequiredAbsence": true,
      "projectionNotGovernanceStateHistoryRecorderDeltaNetworkPoVCpuPriceOrImpossibility": true
    })
  );
}

#[test]
fn cleanup_index_logical_work_bound_covers_domain_and_population_partitions() {
  // Source/logical operands only: never a production Weight or physical-cost estimator.
  fn upper(n: u32, d: u32, population: u64, maximum: u32) -> Option<(u128, u128, u128)> {
    let (n, d, s, m) = (
      u128::from(n),
      u128::from(d),
      u128::from(population),
      u128::from(maximum),
    );
    if d == 0 || m == 0 || d > n || n > s || s > d * m {
      return None;
    }
    let (q, r) = (n / d, n % d);
    let balanced_shrink = d * q * (q - 1) / 2 + r * q;
    let k = n - d + 1;
    let visits = (n * m - balanced_shrink).min(k * (k + 1) / 2 + d - 1 + k * (s - n));
    let first_prefix = codec::Compact(maximum).encoded_size() as u128;
    let later_prefix = codec::Compact(maximum - 1).encoded_size() as u128;
    Some((
      visits,
      4 * visits + d * first_prefix + (n - d) * later_prefix,
      4 * (visits - n) + n * later_prefix,
    ))
  }

  fn check_partitions(
    maximum: u32,
    left: u32,
    parts: &mut Vec<(u32, u32)>,
    checked: &mut usize,
    corner_maximum: &mut u128,
  ) {
    if left != 0 {
      for m in 1..=maximum {
        for n in 1..=m {
          parts.push((n, m));
          check_partitions(maximum, left - 1, parts, checked, corner_maximum);
          parts.pop();
        }
      }
      return;
    }
    let n = parts.iter().map(|(n, _)| n).sum::<u32>();
    let d = parts.len() as u32;
    let population = parts.iter().map(|(_, m)| u64::from(*m)).sum::<u64>();
    let mut actual = (0u128, 0u128, 0u128);
    for &(due, members) in parts.iter() {
      let mut ids = (0..members).collect::<Vec<_>>();
      for expired in 0..due {
        actual.1 += ids.encoded_size() as u128;
        ids.retain(|id| {
          actual.0 += 1;
          *id != expired
        });
        if !ids.is_empty() {
          actual.2 += ids.encoded_size() as u128;
        }
      }
    }
    let mut previous = (0, 0, 0);
    for s in population..=u64::from(d) * u64::from(maximum) {
      let bound = upper(n, d, s, maximum).unwrap();
      assert!(actual.0 <= bound.0 && actual.1 <= bound.1 && actual.2 <= bound.2);
      assert!(previous.0 <= bound.0 && previous.1 <= bound.1 && previous.2 <= bound.2);
      previous = bound;
    }
    if maximum == 4 && n == 4 && d == 2 && population == 7 {
      *corner_maximum = (*corner_maximum).max(actual.0);
    }
    *checked += 1;
  }

  let (mut checked, mut corner_maximum) = (0, 0);
  for maximum in 1..=5 {
    for domains in 1..=3 {
      check_partitions(
        maximum,
        domains,
        &mut Vec::new(),
        &mut checked,
        &mut corner_maximum,
      );
    }
  }
  assert_eq!(checked, 5025);
  assert_eq!(corner_maximum, 12);
  assert_eq!(
    upper(4, 2, 7, 4).unwrap().0,
    14,
    "upper is not a tight/attained maximum"
  );
  for (n, d, s, m) in [
    (0, 0, 0, 1),
    (1, 0, 1, 1),
    (1, 2, 2, 1),
    (2, 1, 1, 2),
    (1, 1, 2, 1),
    (1, 1, 1, 0),
  ] {
    assert!(upper(n, d, s, m).is_none());
  }

  // Compact-prefix transitions, including deletion instead of encoding an empty Vec.
  for (members, due) in [(1u32, 1), (63, 2), (64, 2), (16383, 2), (16384, 2)] {
    let mut ids = (0..members).collect::<Vec<_>>();
    let (mut returned, mut submitted) = (0u128, 0u128);
    for expired in 0..due {
      returned += ids.encoded_size() as u128;
      ids.retain(|id| *id != expired);
      if !ids.is_empty() {
        submitted += ids.encoded_size() as u128;
      }
    }
    let bound = upper(due, 1, u64::from(members), members).unwrap();
    assert!(returned <= bound.1 && submitted <= bound.2);
  }

  assert_eq!(
    <Runtime as pallet_governance::Config>::MaxRecentFinalizedProposalsPerDomain::get(),
    16384
  );
  assert_eq!(
    <Runtime as pallet_governance::Config>::MaxFinalizedProposalOutcomesPerBlock::get(),
    1024
  );
  assert_eq!(
    <Runtime as pallet_governance::Config>::WinningVoteItemId::max_encoded_len(),
    4
  );
  // Consume closed large-profile facts without replaying any hook, observer or timer.
  assert_eq!(
    upper(1024, 1, 16384, 16384),
    Some((16_253_440, 65_015_810, 65_011_712))
  );
  assert_eq!(
    upper(1024, 1024, 16384, 16384),
    Some((16_384, 69_632, 63_488))
  );
  assert_eq!(
    upper(1024, 1024, 17536, 16384),
    Some((17_536, 74_240, 68_096))
  );
  assert_eq!(
    upper(1024, 1024, 16_777_216, 16384),
    Some((16_777_216, 67_112_960, 67_106_816))
  );
  let largest = u32::MAX;
  let full = u64::from(largest) * u64::from(largest);
  assert_eq!(
    upper(largest, largest, full, largest),
    Some((
      u128::from(full),
      4 * u128::from(full) + 5 * u128::from(largest),
      4 * u128::from(full) + u128::from(largest)
    ))
  );
}

#[test]
fn cleanup_ordinary_input_union_keeps_delete_paths_and_distinct_index_values() {
  use polkadot_sdk::frame_support::traits::StorageInfoTrait;
  assert_eq!(
    <Runtime as pallet_governance::Config>::MaxFinalizedProposalOutcomesPerEpoch::get(),
    1024
  );
  assert_eq!(
    <Runtime as pallet_governance::Config>::MaxFinalizedProposalOutcomesPerBlock::get(),
    1024
  );
  let information = <Governance as StorageInfoTrait>::storage_info();
  let families = [
    (b"FinalizedProposalOutcomeExpiryBuckets".as_slice(), 52u64),
    (b"FinalizedProposalIdsByDomain".as_slice(), 52),
    (b"FinalizedProposals".as_slice(), 72),
    (b"ProposalMetadataByItem".as_slice(), 72),
    (b"ProposalWinningPrimaryOptionByItem".as_slice(), 72),
    (b"ProposalUrgentAuthorizedAt".as_slice(), 72),
  ];
  let value_bytes = families.map(|(name, key_bytes)| {
    let info = information
      .iter()
      .find(|info| info.storage_name == name)
      .unwrap();
    u64::from(info.max_size.unwrap())
      .checked_sub(key_bytes - 32)
      .unwrap()
  });
  assert_eq!(value_bytes, [8194, 65540, 234, 34, 1, 4]);
  let blobs = value_bytes.map(|bytes| {
    if bytes > 32 {
      bytes + codec::Compact(bytes as u32).encoded_size() as u64
    } else {
      0
    }
  });
  assert_eq!(blobs, [8196, 65544, 236, 35, 0, 0]);
  // Two original external values in this maximum fixture, not an incremental recorder delta.
  let co_read_value_entries = blobs[0] + blobs[1];
  assert_eq!(co_read_value_entries, 73_740);
  assert!(69_025 < co_read_value_entries);
  let system_information = <System as StorageInfoTrait>::storage_info();
  let number = system_information
    .iter()
    .find(|info| info.storage_name == b"Number")
    .unwrap();
  assert_eq!((number.prefix.len(), number.max_size), (32, Some(4)));
  for (name, size) in [
    (b"LastProcessedEpoch".as_slice(), 4),
    (b"CurrentEpochServicePhase".as_slice(), 1),
    (b"PendingEnactments".as_slice(), 1026),
  ] {
    let info = information
      .iter()
      .find(|info| info.storage_name == name)
      .unwrap();
    assert_eq!((info.prefix.len(), info.max_size), (32, Some(size)));
  }
  assert_eq!(
    pallet_governance::ProposalMaturityBuckets::<Runtime>::hashed_key_for(1).len(),
    52
  );
  // Empty pending/maturity values are one byte even though their generic maxima are larger.
  assert_eq!(codec::Compact(0u32).encoded_size(), 1);
  let wrapper_delta = |initial_maturing: bool| {
    let keys = if initial_maturing {
      &[32u64, 32, 32, 32, 52][..]
    } else {
      &[32u64; 4][..]
    };
    let count = keys.len() as u64;
    let suffix = keys.iter().map(|bytes| 2 * (bytes - 32)).sum::<u64>();
    // The existing ordinary-key/foreign/repair premises apply at the same backend base.
    let read = (65 * count + suffix) * 650 + count * 1284;
    (count, read, read + count * 1284)
  };
  assert_eq!(wrapper_delta(false), (4, 174_136, 179_272));
  assert_eq!(wrapper_delta(true), (5, 243_670, 250_090));
  let project = |n: u64, d: u64| {
    assert!((1..=1024).contains(&n) && (1..=n).contains(&d));
    let counts = [1u64, d, n, n, n, n];
    // Reuse the ordinary H256/V1 owner: generic namespace paths and one foreign terminal.
    let paths = counts.iter().sum::<u64>();
    let suffix_positions = families
      .iter()
      .zip(counts)
      .map(|((_, key_bytes), count)| 2 * (key_bytes - 32) * count)
      .sum::<u64>();
    let payload = blobs
      .iter()
      .zip(counts)
      .map(|(bytes, count)| bytes * count)
      .sum::<u64>();
    let read_superset = (6 * 65 + suffix_positions) * 650 + paths * 1284 + payload + 5;
    // Every path is conservatively allowed deletion, including optional absences/updates.
    let repair = paths * 1284;
    assert_eq!(paths, 1 + d + 4 * n);
    assert_eq!(payload, 8196 + 65544 * d + 271 * n);
    assert_eq!(read_superset, 288985 + 92828 * d + 213407 * n);
    assert_eq!(read_superset + repair, 290269 + 94112 * d + 218543 * n);
    (paths, payload, read_superset, read_superset + repair)
  };
  assert_eq!(project(1, 1), (6, 74011, 595220, 602924));
  let one_domain = project(1024, 1);
  let distinct_domains = project(1024, 1024);
  assert_eq!((one_domain.2, one_domain.3), (218910581, 224172413));
  for (phase, expected) in [
    (false, (219_084_717, 224_351_685)),
    (true, (219_154_251, 224_422_503)),
  ] {
    let wrapper = wrapper_delta(phase);
    assert_eq!(
      (one_domain.2 + wrapper.1, one_domain.3 + wrapper.2),
      expected
    );
  }
  assert_eq!(
    (distinct_domains.2, distinct_domains.3),
    (313873625, 320448989)
  );
  let next_domain = project(2, 2);
  let same_domain = project(2, 1);
  assert_eq!(next_domain.2 - same_domain.2, 92828);
  assert_eq!(next_domain.3 - same_domain.3, 94112);
  let pending = u64::from(<Runtime as pallet_governance::Config>::MaxPendingEnactments::get());
  let per_epoch =
    u64::from(<Runtime as pallet_governance::Config>::MaxFinalizedProposalOutcomesPerEpoch::get());
  let per_domain =
    u64::from(<Runtime as pallet_governance::Config>::MaxRecentFinalizedProposalsPerDomain::get());
  let retention = u64::from(
    <Runtime as pallet_governance::Config>::FinalizedProposalOutcomeRetentionEpochs::get(),
  );
  assert_eq!(
    (pending, per_epoch, per_domain, retention),
    (128, 1024, 16384, 16)
  );
  assert_eq!(4 * per_domain + 8, blobs[1]);
  // Parameters describe one canonical backend base, never a newer cursor over older values.
  let joint_members = |domains: u64, debt: u64| {
    assert!((1..=1024).contains(&domains) && debt <= u64::from(u32::MAX));
    (domains * per_domain).min(pending + per_epoch * (retention + debt))
  };
  assert_eq!(joint_members(2, 0), 16512);
  assert!(
    joint_members(2, 0) > per_epoch * retention,
    "pending discovery slots are independent of terminal retention"
  );
  let joint_index_blob = |domains, debt| 4 * joint_members(domains, debt) + 8 * domains;
  assert_eq!(joint_index_blob(1, 1), blobs[1]);
  assert_eq!(joint_index_blob(1024, 1), 78336);
  let replace_index_only = |n, domains, debt| {
    let original = project(n, domains);
    let replacement = joint_index_blob(domains, debt);
    assert!(replacement <= blobs[1] * domains);
    (
      original.2 - blobs[1] * domains + replacement,
      original.3 - blobs[1] * domains + replacement,
    )
  };
  let joint_at_one = replace_index_only(1024, 1024, 1);
  assert_eq!(joint_at_one, (246834905, 253410269));
  let full_product_debt = (per_domain * 1024 - pending).div_ceil(per_epoch) - retention;
  assert_eq!(full_product_debt, 16368);
  assert!(joint_members(1024, full_product_debt - 1) < per_domain * 1024);
  assert_eq!(joint_members(1024, full_product_debt), per_domain * 1024);
  assert_eq!(
    replace_index_only(1024, 1024, full_product_debt),
    (distinct_domains.2, distinct_domains.3)
  );
  assert_eq!(joint_members(1024, u64::from(u32::MAX)), per_domain * 1024);
  assert_eq!(
    core::any::TypeId::of::<<Runtime as pallet_governance::Config>::DomainId>(),
    core::any::TypeId::of::<u32>()
  );
  let active_per_domain =
    u64::from(<Runtime as pallet_governance::Config>::MaxActiveProposalsPerDomain::get());
  assert_eq!(active_per_domain, 128);
  let active_global = active_per_domain * (u64::from(u32::MAX) + 1);
  assert_eq!(active_global, 549755813888);
  assert_eq!(
    core::any::TypeId::of::<<Runtime as pallet_governance::Config>::WinningVoteItemId>(),
    core::any::TypeId::of::<u32>()
  );
  let item_keys = [
    [
      pallet_governance::FinalizedProposals::<Runtime>::hashed_key_for(101, 110),
      pallet_governance::FinalizedProposals::<Runtime>::hashed_key_for(101, 111),
      pallet_governance::FinalizedProposals::<Runtime>::hashed_key_for(102, 110),
    ],
    [
      pallet_governance::ProposalMetadataByItem::<Runtime>::hashed_key_for(101, 110),
      pallet_governance::ProposalMetadataByItem::<Runtime>::hashed_key_for(101, 111),
      pallet_governance::ProposalMetadataByItem::<Runtime>::hashed_key_for(102, 110),
    ],
    [
      pallet_governance::ProposalWinningPrimaryOptionByItem::<Runtime>::hashed_key_for(101, 110),
      pallet_governance::ProposalWinningPrimaryOptionByItem::<Runtime>::hashed_key_for(101, 111),
      pallet_governance::ProposalWinningPrimaryOptionByItem::<Runtime>::hashed_key_for(102, 110),
    ],
    [
      pallet_governance::ProposalUrgentAuthorizedAt::<Runtime>::hashed_key_for(101, 110),
      pallet_governance::ProposalUrgentAuthorizedAt::<Runtime>::hashed_key_for(101, 111),
      pallet_governance::ProposalUrgentAuthorizedAt::<Runtime>::hashed_key_for(102, 110),
    ],
  ];
  for [first, same_domain, other_domain] in item_keys {
    assert_eq!(
      (first.len(), same_domain.len(), other_domain.len()),
      (72, 72, 72)
    );
    assert_eq!(&first[..52], &same_domain[..52]);
    assert_ne!(&first[..52], &other_domain[..52]);
    assert_ne!(&first[52..], &same_domain[52..]);
  }
  let forest = |n: u64, domains: u64, debt: u64, active: u64, group_domains: bool| {
    let previous = replace_index_only(n, domains, debt);
    assert!(active <= active_global);
    let finalized = pending + per_epoch * (retention + debt);
    // Whole-namespace populations, not only selected domains or expiring records.
    let populations = [
      retention + debt,
      finalized,
      finalized,
      active + finalized,
      finalized,
      active,
    ];
    let counts = [1u64, domains, n, n, n, n];
    let mut nodes = 0u64;
    for (((_, key_bytes), queries), population) in families.iter().zip(counts).zip(populations) {
      let separate_branches = 2 * queries * (key_bytes - 32);
      // Share only each complete hashed-domain prefix, never across storage namespaces.
      let branches = if group_domains && *key_bytes == 72 {
        2 * domains * (52 - 32) + 2 * queries * (key_bytes - 52)
      } else {
        separate_branches
      };
      assert!(branches <= separate_branches);
      let references = (16 * branches).min(population + branches + queries);
      // Reuse the nonprefix fixed-key forest owner; no hashed-key distribution assumption.
      let shared =
        7 * branches + queries * key_bytes + 33 * references + queries * (key_bytes + 37);
      assert!(shared <= 650 * branches);
      nodes += 65 * 650 + shared + 1284 * queries;
    }
    let payload = 8196 + 271 * n + joint_index_blob(domains, debt);
    let read = nodes + payload + 5;
    let net_delete = read + 1284 * (1 + domains + 4 * n);
    assert!(read <= previous.0 && net_delete <= previous.1);
    (read, net_delete, populations)
  };
  let no_active_forest = forest(1024, 1024, 1, 0, false);
  let broad_active_forest = forest(1024, 1024, 1, active_global, false);
  assert_eq!(
    (no_active_forest.0, no_active_forest.1),
    (25310312, 31885676)
  );
  assert_eq!(
    (broad_active_forest.0, broad_active_forest.1),
    (105764840, 112340204)
  );
  assert_eq!(
    broad_active_forest.2[3] - broad_active_forest.2[2],
    active_global
  );
  assert_eq!(broad_active_forest.2[5], active_global);
  assert!(
    broad_active_forest.0 > no_active_forest.0,
    "active metadata/urgency cannot inherit the finalized-only population"
  );
  let broad_debt_forest = forest(1024, 1024, u64::from(u32::MAX), active_global, false);
  assert_eq!(
    (broad_debt_forest.0, broad_debt_forest.1),
    (272361326, 278936690)
  );
  let grouped_single = forest(1024, 1, 1, active_global, true);
  let separate_single = forest(1024, 1, 1, active_global, false);
  assert_eq!((grouped_single.0, grouped_single.1), (55006793, 60268625));
  assert!(grouped_single.0 < separate_single.0);
  assert_eq!(
    grouped_single.1 - grouped_single.0,
    separate_single.1 - separate_single.0
  );
  let grouped_distinct = forest(1024, 1024, 1, active_global, true);
  assert_eq!(
    grouped_distinct, broad_active_forest,
    "one identity per domain shares no additional path"
  );
  assert_eq!(
    forest(1024, 1024, u64::from(u32::MAX), active_global, true),
    broad_debt_forest
  );
  println!(
    "GOVERNANCE_CLEANUP_ORDINARY_INPUT_UNION_V1 {}",
    serde_json::json!({
      "valueBytes": value_bytes, "externalBlobBytes": blobs,
      "maximumBucketAndIndexOriginalValueEntries": co_read_value_entries,
      "wrapperDeltaFinalizedOutcome": wrapper_delta(false),
      "wrapperDeltaMaturing": wrapper_delta(true),
      "valueEntryMassIsNotAnIncrementalRecorderDeltaFloor": true,
      "oneEntryOneDomain": project(1, 1), "maximumEntriesOneDomain": one_domain,
      "coarseDistinctDomainCorner": distinct_domains,
      "jointIndexBlobAtDebtOne": joint_index_blob(1024, 1),
      "jointReadAndDeleteInputAtDebtOne": joint_at_one,
      "fullTypedProductRestoredAtDebt": full_product_debt,
      "forestNoActiveAtDebtOne": no_active_forest,
      "forestBroadActiveAtDebtOne": broad_active_forest,
      "forestBroadDebtAndActive": broad_debt_forest,
      "groupedSingleDomainAtDebtOne": grouped_single,
      "groupedDistinctDomainsAtDebtOne": grouped_distinct,
      "domainPrefixBytes": 52,
      "groupingRetainsForeignValuesAndDeletionOwners": true,
      "populationOrder": ["expiryBuckets", "domainIndices", "finalized", "metadata", "winningOption", "urgency"],
      "activeGlobalTypedUpperNotAttainment": active_global,
      "debtIsDescriptiveParameterNotQualifiedCapOrGetter": true,
      "selectedPathsIncludeDeleteOnlyFamilies": true,
      "ordinarySourceAndHostConditionsRequired": true,
      "notFullHookHostCostPoVOrPhysicalPrice": true,
      "upperExcessDoesNotProveAttainmentOrImpossibility": true
    })
  );
}

#[test]
fn cleanup_present_key_path_bound_excludes_absence() {
  use polkadot_sdk::{
    sp_core::{Blake2Hasher, H256},
    sp_trie::{
      LayoutV1, MemoryDB, Trie, TrieDBBuilder, TrieDBMutBuilder, TrieLayout, TrieMut,
      recorder::Recorder,
    },
  };
  type Layout = LayoutV1<Blake2Hasher>;
  fn observe(
    db: &MemoryDB<Blake2Hasher>,
    root: H256,
    key: &[u8],
    value: &[u8],
  ) -> (bool, usize, usize) {
    let recorder = Recorder::<Blake2Hasher>::default();
    let present = {
      let mut sink = recorder.as_trie_recorder(root);
      TrieDBBuilder::<Layout>::new(db, &root)
        .with_recorder(&mut sink)
        .build()
        .get(key)
        .unwrap()
        .is_some()
    };
    let bytes = recorder.estimate_encoded_size();
    let proof = recorder.drain_storage_proof();
    (
      present,
      bytes,
      proof
        .iter_nodes()
        .filter(|node| node.as_slice() != value)
        .map(|node| node.len())
        .max()
        .unwrap(),
    )
  }
  assert_eq!(
    crate::VERSION.state_version(),
    polkadot_sdk::sp_core::storage::StateVersion::V1
  );
  assert!(!Layout::USE_EXTENSION);
  assert_eq!(Layout::MAX_INLINE_VALUE, Some(33));
  let key = pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::hashed_key_for(17);
  assert_eq!(key.len(), 52);
  assert_eq!(
    pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::hashed_key_for(101).len(),
    52
  );
  let nibbles = key.len() * 2;
  // Present-key path: <=104 consumed nibbles + terminal; every partial is within that key.
  // V1 node upper: header2 + partial52 + bitmap2 + value33 + sixteen child references33.
  let node_upper = 2 + key.len() + 2 + 33 + 16 * 33;
  let node_record_upper = vec![0u8; node_upper].encoded_size();
  let value = vec![7u8; 8_194];
  let present_only_upper = (nibbles + 1) * node_record_upper + value.encoded_size();
  let mut results = Vec::new();
  for absent in [false, true] {
    let mut db = MemoryDB::<Blake2Hasher>::default();
    let mut root = H256::default();
    {
      let mut trie = TrieDBMutBuilder::<Layout>::new(&mut db, &mut root).build();
      for position in 0..nibbles {
        let shift = if position % 2 == 0 { 4 } else { 0 };
        let old = (key[position / 2] >> shift) & 15;
        for nibble in 0u8..16 {
          if nibble == old {
            continue;
          }
          let mut sibling = key.clone();
          sibling[position / 2] = (sibling[position / 2] & !(15 << shift)) | (nibble << shift);
          trie.insert(&sibling, &[9u8; 32]).unwrap();
        }
      }
      if absent {
        let mut longer = key.clone();
        longer.resize(20_000, 3);
        trie.insert(&longer, &[9u8; 32]).unwrap();
      } else {
        trie.insert(&key, &value).unwrap();
      }
    }
    let (present, recorded, largest_node) = observe(&db, root, &key, &value);
    assert_eq!(present, !absent);
    if present {
      assert!(largest_node <= node_upper);
      assert!(recorded <= present_only_upper);
    } else {
      assert!(
        largest_node > node_upper,
        "absence does not bound terminal partial by queried key length"
      );
      assert!(
        recorded > present_only_upper,
        "counterexample to applying the present-key envelope to absence"
      );
    }
    if present {
      let mut descendant = key.clone();
      descendant.resize(20_000, 3);
      {
        let mut trie = TrieDBMutBuilder::<Layout>::from_existing(&mut db, &mut root).build();
        trie.insert(&descendant, &[9u8; 32]).unwrap();
      }
      let (still_present, before_delete, _) = observe(&db, root, &key, &value);
      assert!(still_present);
      assert!(before_delete <= present_only_upper);
      let parent_root = root;
      let recorder = Recorder::<Blake2Hasher>::default();
      {
        let mut sink = recorder.as_trie_recorder(parent_root);
        let mut trie = TrieDBMutBuilder::<Layout>::from_existing(&mut db, &mut root)
          .with_recorder(&mut sink)
          .build();
        assert!(trie.remove(&key).unwrap().is_some());
        trie.commit();
      }
      let deletion_bytes = recorder.estimate_encoded_size();
      let proof = recorder.drain_storage_proof();
      let largest = proof.iter_nodes().map(|node| node.len()).max().unwrap();
      assert_ne!(root, parent_root);
      assert!(
        largest > node_upper,
        "deletion may fetch an off-path descendant to compress the branch"
      );
      assert!(
        deletion_bytes > present_only_upper,
        "present-read bound is not a deletion/commit bound"
      );
      let trie = TrieDBBuilder::<Layout>::new(&db, &root).build();
      assert!(trie.get(&key).unwrap().is_none());
      assert_eq!(trie.get(&descendant).unwrap(), Some(vec![9u8; 32]));
      println!(
        "GOVERNANCE_TRIE_PRESENT_DELETE_SCOPE_V1 {}",
        serde_json::json!({
          "syntheticTrieNotRuntimeHistory": true, "presentReadBytes": before_delete,
          "presentReadUpper": present_only_upper, "deleteCommitBytes": deletion_bytes,
          "largestDeleteNode": largest, "globalKeyLengthBoundNotEstablished": true,
          "notProductionProofBound": true
        })
      );
    }
    results.push(serde_json::json!({"present": present, "recordedBytes": recorded, "largestStructuralNode": largest_node}));
  }
  println!(
    "GOVERNANCE_TRIE_PATH_SCOPE_V1 {}",
    serde_json::json!({
      "syntheticTrieNotRuntimeHistory": true, "keyBytes": key.len(), "siblingKeys": 15 * nibbles,
      "nodeUpper": node_upper, "nodeRecordUpper": node_record_upper,
      "presentOnlyReadUpper": present_only_upper,
      "excludes": "absence, deletion/commit witnesses, child tries, full storage-proof/container/network PoV", "results": results
    })
  );
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
fn full_pending_protocol_hook_matches_prefreeze_context_and_receipts() {
  use pallet_governance::{
    ProposalExecutionDetail as Detail, ProposalExecutionSuccessDetail as Success,
    ProposalParameterChangeSurface as Surface, ProposalPayloadKind as Kind,
  };
  new_test_ext().execute_with(|| {
    type W = crate::weights::pallet_governance::SubstrateWeight<Runtime>;
    let domain = PROTOCOL_GOVERNANCE_DOMAIN;
    let code_hash = crate::Hash::repeat_byte(23);
    let new_fee = polkadot_sdk::sp_runtime::Perbill::from_percent(1);
    let capacity = <Runtime as pallet_governance::Config>::MaxPendingEnactments::get();
    let visits = <Runtime as pallet_governance::Config>::MaxPendingEnactmentsPerBlock::get();
    assert_eq!((capacity, visits), (128, 4));
    assert!(System::authorized_upgrade().is_none());
    assert_ne!(DeosRouter::router_fee(), new_fee);
    assert!(!Actors::global_circuit_breaker());
    assert!(Actors::block_resource_state().is_none());
    Actors::apply_user_creation_enabled(false);
    let sources = [
      (
        Kind::L1RootAction,
        StrategicRuntimeUpgradePayload { code_hash }.encode(),
        Success::RuntimeUpgradeAuthorized { code_hash },
      ),
      (
        Kind::L2ParameterChange,
        RuntimeCall::DeosRouter(pallet_deos_router::Call::update_router_fee { new_fee }).encode(),
        Success::ParameterChangeExecuted {
          surface: Surface::RouterFee,
        },
      ),
      (
        Kind::L2ParameterChange,
        RuntimeCall::Actors(pallet_deos_actors::Call::set_user_creation_enabled { enabled: true })
          .encode(),
        Success::ParameterChangeExecuted {
          surface: Surface::ActorUserCreationEnabled,
        },
      ),
      (
        Kind::L2ParameterChange,
        RuntimeCall::Actors(pallet_deos_actors::Call::set_global_circuit_breaker { paused: true })
          .encode(),
        Success::ParameterChangeExecuted {
          surface: Surface::ActorGlobalCircuitBreaker,
        },
      ),
    ];
    let mut rows = Vec::new();
    for (kind, bytes, expected) in sources {
      let hash = <Runtime as frame_system::Config>::Hashing::hash(&bytes);
      assert!(rows.iter().all(|(_, prior, _)| prior != &hash));
      assert!(
        RuntimeProposalPayloadPreimageProvider::validate_for_witness(domain, kind, &hash, &bytes)
          .is_ok()
      );
      assert_ok!(Preimage::note_preimage(RuntimeOrigin::signed(ALICE), bytes));
      rows.push((kind, hash, expected));
    }
    let (cleanup_domain, old_expiry) =
      Governance::prepare_maximum_finalized_cleanup_hook().unwrap();
    assert_ne!(cleanup_domain, domain);
    let old_ids = pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(cleanup_domain);
    assert_eq!(old_ids.len(), 16_384);
    let now = old_expiry + 1;
    pallet_governance::LastProcessedEpoch::<Runtime>::put(old_expiry - 1);
    let verify_pending_topology = Governance::prepare_full_pending_enactment_hook(
      now,
      false,
      Some((domain, rows[0].0, rows[0].1, None)),
    )
    .unwrap();
    let members = pallet_governance::PendingEnactments::<Runtime>::get();
    assert_eq!(members.len(), capacity as usize);
    let first_due = (capacity - visits) as usize;
    for (member, (kind, hash, _)) in members[first_due..].iter().zip(&rows) {
      assert_eq!(member.domain, domain);
      pallet_governance::ProposalMetadataByItem::<Runtime>::mutate(
        domain,
        member.item_id,
        |metadata| {
          let metadata = metadata.as_mut().unwrap();
          metadata.payload_kind = *kind;
          metadata.payload_hash = *hash;
        },
      );
      assert_eq!(
        Governance::proposal_pending_enactment_at(domain, member.item_id),
        Some(now)
      );
      assert_eq!(
        Governance::proposal_execution_detail(domain, member.item_id),
        None
      );
    }
    assert!(
      Actors::block_resource_state().is_none(),
      "no synthetic post-freeze state in this initialization source"
    );
    let next_actor = Actors::next_actor_id();
    let actor_count = Actors::active_actor_count();
    let fee_sink = System::account(&actor_fee_sink_account());
    #[cfg(feature = "try-runtime")]
    assert_ok!(<Governance as Hooks<crate::BlockNumber>>::try_state(now));
    let charged = Governance::on_initialize(now);
    let n = <Runtime as pallet_governance::Config>::MaxFinalizedProposalOutcomesPerBlock::get();
    let expected_charge = <W as pallet_governance::WeightInfo>::service_pending_enactments(visits)
      .saturating_add(<W as pallet_governance::WeightInfo>::service_epoch_catch_up())
      .saturating_add(<W as pallet_governance::WeightInfo>::service_finalized_proposal_outcomes(n));
    assert_eq!(charged, expected_charge);
    verify_pending_topology(&[]).unwrap();
    for (member, (_, _, expected)) in members[first_due..].iter().zip(&rows) {
      assert_eq!(
        Governance::proposal_execution_detail(domain, member.item_id),
        Some(Detail::Succeeded(expected.clone()))
      );
    }
    assert_eq!(
      System::authorized_upgrade().unwrap().code_hash(),
      &code_hash
    );
    assert_eq!(
      Governance::authorized_runtime_upgrade(),
      Some(pallet_governance::AuthorizedRuntimeUpgrade {
        code_hash,
        check_version: true
      })
    );
    assert_eq!(DeosRouter::router_fee(), new_fee);
    assert!(Actors::user_creation_enabled());
    assert!(Actors::global_circuit_breaker());
    assert!(Actors::block_resource_state().is_none());
    assert_eq!(Actors::next_actor_id(), next_actor);
    assert_eq!(Actors::active_actor_count(), actor_count);
    assert_eq!(System::account(&actor_fee_sink_account()), fee_sink);
    assert_eq!(
      pallet_governance::PendingEnactments::<Runtime>::get().as_slice(),
      &members[..first_due]
    );
    assert_eq!(
      pallet_governance::LastProcessedEpoch::<Runtime>::get(),
      old_expiry - 1
    );
    assert_eq!(
      pallet_governance::CurrentEpochServicePhase::<Runtime>::get(),
      pallet_governance::EpochServicePhase::RewardExpiry
    );
    assert_eq!(
      pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::get(cleanup_domain).as_slice(),
      &old_ids[n as usize..]
    );
    assert!(
      pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(old_expiry)
        .is_empty()
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(<Governance as Hooks<crate::BlockNumber>>::try_state(now));
    println!(
      "GOVERNANCE_FULL_PENDING_PROTOCOL_HOOK_SOURCE_V1 {}",
      serde_json::json!({
        "pendingBefore": capacity, "visited": visits, "distinctDuePreimages": rows.len(),
        "effects": ["upgradeAuthorization", "routerFee", "userCreation", "globalCircuitBreaker"],
        "currentResourceStateAbsentBeforeAndAfter": true, "actualHook": true,
        "oldCleanup": n, "oldIndexBefore": old_ids.len(), "oldExpiry": old_expiry,
        "distinctPendingAndCleanupDomains": true, "futurePending": first_due,
        "syntheticNotPublicHistoryOrJointMaximum": true,
        "notPostfreezeHaltFailureRollbackOrPhysicalPrice": true
      })
    );
  });
}

#[cfg(feature = "runtime-benchmarks")]
fn verify_full_pending_invoice_hook(missing_and_capacity_failure: bool, predecessor_source: bool) {
  use pallet_governance::{
    BenchmarkHelper as _, ProposalExecutionDetail as Detail,
    ProposalExecutionFailureReason as Failure, ProposalExecutionSuccessDetail as Success,
  };
  use polkadot_sdk::sp_runtime::Either;
  new_test_ext().execute_with(|| {
    type W = crate::weights::pallet_governance::SubstrateWeight<Runtime>;
    let visits = <Runtime as pallet_governance::Config>::MaxPendingEnactmentsPerBlock::get();
    let capacity = <Runtime as pallet_governance::Config>::MaxPendingEnactments::get();
    assert_eq!((capacity, visits), (128, 4));
    let domain = TACTICAL_GOVERNANCE_DOMAIN;
    let kind = pallet_governance::ProposalPayloadKind::L2TreasurySpend;
    let mut rows = Vec::new();
    for index in 0..visits {
      let beneficiary = crate::AccountId::new([60 + index as u8; 32]);
      let (prepared_domain, prepared_kind, hash, receipt) = <Runtime as pallet_governance::Config>::BenchmarkHelper::prepare_maximum_finalized_execution(&beneficiary).unwrap();
      assert_eq!((prepared_domain, prepared_kind), (domain, kind));
      let Success::TreasurySpendExecuted { invoice_cid, treasury_account, payout_asset, base_amount, .. } = &receipt else {
        panic!("maximum reference receipt must be an Invoice")
      };
      assert_eq!(invoice_cid.len(), 96);
      let bytes = TacticalTreasuryInvoicePayload { invoice_cid: invoice_cid.clone(), treasury_account: treasury_account.clone(), beneficiary, payout_asset: *payout_asset, base_amount: *base_amount }.encode();
      assert_eq!(bytes.len(), TacticalTreasuryInvoicePayload::max_encoded_len());
      assert_eq!(<Runtime as frame_system::Config>::Hashing::hash(&bytes), hash);
      assert!(rows.iter().all(|(previous, _)| previous != &hash));
      assert_ok!(Preimage::note_preimage(RuntimeOrigin::signed(ALICE), bytes));
      rows.push((hash, receipt));
    }
    let now = 1 + <Runtime as pallet_governance::Config>::ProposalEnactmentDelay::get() + 17;
    let preparation_epoch = if predecessor_source { now - 1 } else { now };
    pallet_governance::LastProcessedEpoch::<Runtime>::put(preparation_epoch);
    if !missing_and_capacity_failure {
      let missing = crate::Hash::repeat_byte(0xaf);
      assert!(!RuntimeProposalPayloadPreimageProvider::have_preimage(&missing));
      for (payload_kind, hash) in [(pallet_governance::ProposalPayloadKind::Intent, rows[0].0), (kind, missing)] {
        let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        assert!(matches!(Governance::prepare_full_pending_enactment_hook(now, false, Some((domain, payload_kind, hash, None))), Err(polkadot_sdk::frame_benchmarking::BenchmarkError::Skip)));
        assert_eq!(polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1), before);
      }
    }
    let user_creation = pallet_deos_actors::UserCreationEnabled::<Runtime>::get();
    let payload_source = Some((domain, kind, rows[0].0, Some(pallet_governance::ProposalPrimaryTrackOption::Amplify)));
    let verify_pending_topology = if predecessor_source {
      Either::Left(Governance::prepare_full_pending_enactment_parent(now, payload_source).unwrap())
    } else {
      Either::Right(Governance::prepare_full_pending_enactment_hook(now, false, payload_source).unwrap())
    };
    assert_eq!(System::block_number(), preparation_epoch);
    if predecessor_source {
      let retention = <Runtime as pallet_governance::Config>::FinalizedProposalOutcomeRetentionEpochs::get();
      assert!(pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(now + retention).is_empty());
      let peers = pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::get(preparation_epoch + retention);
      assert_eq!(peers.len(), 1020);
      for touch in peers {
        assert!(matches!(pallet_governance::FinalizedProposals::<Runtime>::get(touch.domain, touch.item_id).unwrap().outcome, pallet_governance::FinalizedProposalOutcome::Rejected { finalized_epoch, .. } if finalized_epoch == preparation_epoch));
      }
    }
    assert_eq!(pallet_deos_actors::UserCreationEnabled::<Runtime>::get(), user_creation, "supplied payload must not invoke the default User-admission setup");
    let members = pallet_governance::PendingEnactments::<Runtime>::get();
    assert_eq!(members.len(), capacity as usize);
    let first_due = (capacity - visits) as usize;
    for member in &members {
      assert_eq!(member.domain, domain);
      let record = pallet_governance::FinalizedProposals::<Runtime>::get(domain, member.item_id).unwrap();
      assert!(matches!(record.outcome, pallet_governance::FinalizedProposalOutcome::Approved { approval, .. } if approval.approved_epoch <= preparation_epoch));
      assert_eq!(Governance::proposal_winning_primary_option(domain, member.item_id), Some(pallet_governance::ProposalPrimaryTrackOption::Amplify));
    }
    for (member, (hash, _)) in members[first_due..].iter().zip(&rows) {
      pallet_governance::ProposalMetadataByItem::<Runtime>::mutate(domain, member.item_id, |metadata| metadata.as_mut().unwrap().payload_hash = *hash);
      assert_eq!(Governance::proposal_pending_enactment_at(domain, member.item_id), Some(now));
      assert_eq!(Governance::proposal_execution_detail(domain, member.item_id), None);
    }
    if missing_and_capacity_failure {
      assert!(!RuntimeProposalPayloadPreimageProvider::preimage_requested(&rows[0].0));
      assert_ok!(Preimage::unnote_preimage(RuntimeOrigin::signed(ALICE), rows[0].0));
      assert!(!RuntimeProposalPayloadPreimageProvider::have_preimage(&rows[0].0));
    }
    let expected_failures = if missing_and_capacity_failure {
      vec![(members[first_due].item_id, Failure::MissingPreimage), (members[first_due + 3].item_id, Failure::InsufficientTreasuryCapacity)]
    } else { Vec::new() };
    let payouts = if missing_and_capacity_failure { [0, 200, 200, 0] } else { [200, 200, 200, 150] };
    let treasury = invoice_treasury();
    assert_eq!(Assets::balance(domain, &treasury), 0);
    assert_ok!(<Assets as FungiblesMutate<crate::AccountId>>::mint_into(domain, &treasury, if missing_and_capacity_failure { 400 } else { 750 }));
    let issuance = Assets::total_issuance(domain);
    let fee_sink = System::account(&actor_fee_sink_account());
    let actor = pallet_deos_actors::SovereignIndex::<Runtime>::get(&treasury).unwrap();
    let identity = Actors::actor_identity(actor);
    #[cfg(feature = "try-runtime")]
    assert_ok!(<Governance as Hooks<crate::BlockNumber>>::try_state(preparation_epoch));
    if predecessor_source {
      System::set_block_number(now);
    }
    let charged = Governance::on_initialize(now);
    let expected_charge = <W as pallet_governance::WeightInfo>::service_pending_enactments(visits)
      .saturating_add(if predecessor_source { <W as pallet_governance::WeightInfo>::service_epoch_catch_up() } else { polkadot_sdk::frame_support::weights::Weight::zero() });
    assert_eq!(charged, expected_charge);
    match verify_pending_topology {
      Either::Left(verify) => verify(&expected_failures),
      Either::Right(verify) => verify(&expected_failures),
    }.unwrap();
    for (index, (member, (_, expected))) in members[first_due..].iter().zip(&rows).enumerate() {
      let actual = payouts[index];
      let failure = expected_failures.iter().find_map(|(item_id, reason)| (*item_id == member.item_id).then_some(*reason));
      if let Some(reason) = failure {
        assert_eq!(Governance::proposal_execution_detail(domain, member.item_id), Some(Detail::Failed(reason)));
        let Success::TreasurySpendExecuted { beneficiary, .. } = expected else { panic!("source receipt must remain an Invoice") };
        assert_eq!(Assets::balance(domain, beneficiary), 0);
        continue;
      }
      let mut expected = expected.clone();
      let Success::TreasurySpendExecuted { actual_amount, capacity_limited, beneficiary, .. } = &mut expected else { panic!("source receipt must remain an Invoice") };
      *actual_amount = actual;
      *capacity_limited = actual < 200;
      assert_eq!(Assets::balance(domain, beneficiary), actual);
      let record = pallet_governance::FinalizedProposals::<Runtime>::get(domain, member.item_id).unwrap();
      assert_eq!(record.execution_detail, Some(Detail::Succeeded(expected)));
      assert_eq!(record.encoded_size(), pallet_governance::FinalizedProposalRecord::<crate::AccountId, u32, crate::Hash, u32, pallet_governance::CidBytesOf<Runtime>>::max_encoded_len());
    }
    assert_eq!(Assets::balance(domain, &treasury), 0);
    assert_eq!(Assets::total_issuance(domain), issuance);
    assert_eq!(System::account(&actor_fee_sink_account()), fee_sink);
    assert_eq!(Actors::actor_identity(actor), identity);
    assert_eq!(pallet_governance::PendingEnactments::<Runtime>::get().as_slice(), &members[..first_due]);
    assert_eq!(pallet_governance::LastProcessedEpoch::<Runtime>::get(), now);
    #[cfg(feature = "try-runtime")]
    assert_ok!(<Governance as Hooks<crate::BlockNumber>>::try_state(now));
    let marker = if missing_and_capacity_failure { "GOVERNANCE_FULL_PENDING_INVOICE_FAILURE_HOOK_SOURCE_V1" } else { "GOVERNANCE_FULL_PENDING_INVOICE_HOOK_SOURCE_V1" };
    println!("{marker} {}", serde_json::json!({
      "pendingBefore": capacity, "visited": visits, "distinctDuePreimages": rows.len(),
      "cidBytes": 96, "payloadBytes": TacticalTreasuryInvoicePayload::max_encoded_len(),
      "actualPayouts": payouts, "futurePending": first_due, "expectedFailures": expected_failures.len(),
      "actualHook": true, "historyAlreadyCompletedAtCurrentEpoch": !predecessor_source,
      "predecessorClockSource": predecessor_source, "preparationEpoch": preparation_epoch,
      "syntheticNotPublicHistoryOrJointMaximum": true,
      "notMutationBeforeErrorOrLateBookkeepingRollbackEvidence": true,
      "notPhysicalTimingProofOrProductionPrice": true
    }));
  });
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn full_pending_invoice_hook_binds_distinct_maximum_payloads_and_receipts() {
  verify_full_pending_invoice_hook(false, false);
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn full_pending_invoice_hook_commits_missing_and_capacity_failures() {
  verify_full_pending_invoice_hook(true, false);
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn full_pending_invoice_parent_source_preserves_predecessor_chronology() {
  verify_full_pending_invoice_hook(true, true);
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
fn governance_admin_terminal_ingress_rejects_signed_origin_without_state_change() {
  use polkadot_sdk::frame_support::traits::{Authorize, UnfilteredDispatchable};
  new_test_ext().execute_with(|| {
    let calls = [
      pallet_governance::Call::resolve_proposal {
        domain: 101,
        item_id: 110,
        winners: Default::default(),
      },
      pallet_governance::Call::reject_proposal {
        domain: 101,
        item_id: 110,
      },
      pallet_governance::Call::resolve_proposal_from_votes {
        domain: 101,
        item_id: 110,
      },
      pallet_governance::Call::force_resolve_proposal_from_votes {
        domain: 101,
        item_id: 110,
      },
    ];
    for call in calls {
      let call = RuntimeCall::Governance(call);
      assert!(call.authorize(TransactionSource::External).is_none());
      for origin in [
        RuntimeOrigin::signed(ALICE),
        RuntimeOrigin::none(),
        frame_system::RawOrigin::Authorized.into(),
      ] {
        let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        let error = call.clone().dispatch_bypass_filter(origin).unwrap_err();
        assert_eq!(
          error.error,
          polkadot_sdk::sp_runtime::DispatchError::BadOrigin
        );
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
          before
        );
      }
    }
  });
}

#[test]
fn signed_governance_vote_retains_declared_weight_through_transaction_extensions() {
  let storage = super::production_block_replay::preset_genesis_storage(
    &[],
    &polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into(),
  );
  // Native per-extrinsic execution with inherent placeholders, not complete block acceptance.
  polkadot_sdk::sp_io::TestExternalities::new(storage).execute_with(|| {
    let signer = polkadot_sdk::sp_keyring::Sr25519Keyring::Alice.pair();
    System::set_block_number(1);
    polkadot_sdk::pallet_timestamp::Now::<Runtime>::put(1);
    polkadot_sdk::cumulus_pallet_parachain_system::ValidationData::<Runtime>::put(
      polkadot_sdk::cumulus_primitives_core::PersistedValidationData::default(),
    );
    let _ = Actors::on_initialize(1);
    assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
    for nonce in 0..2 {
      let call = RuntimeCall::Governance(pallet_governance::Call::cast_vote {
        domain: 101,
        item_id: 110,
        vote: pallet_governance::ProposalVoteKind::Approve,
      });
      let call_info = call.get_dispatch_info();
      let extrinsic = signed_extrinsic(&signer, nonce, call);
      let info = extrinsic.get_dispatch_info();
      let charged = frame_system::calculate_consumed_extrinsic_weight::<RuntimeCall>(
        &crate::configs::RuntimeBlockWeights::get(),
        &info,
        extrinsic.encoded_size(),
      );
      let frame_before = *System::block_weight().get(info.class);
      let resource_before = Actors::block_resource_state()
        .unwrap()
        .usage()
        .user_dispatch_used();
      assert_eq!(
        Executive::apply_extrinsic(extrinsic),
        Ok(Err(
          pallet_governance::Error::<Runtime>::ProposalNotActive.into()
        ))
      );
      let resource_after = Actors::block_resource_state().unwrap();
      assert_eq!(
        System::block_weight()
          .get(info.class)
          .saturating_sub(frame_before),
        charged
      );
      assert_eq!(
        resource_after
          .usage()
          .user_dispatch_used()
          .saturating_sub(resource_before),
        charged
      );
      assert_eq!(resource_after.outstanding_reservations(), 0);
      assert!(charged.ref_time() >= call_info.call_weight.ref_time());
      assert!(Governance::finalized_proposal_outcome(101, 110).is_none());
    }
  });
}

#[test]
fn declared_governance_vote_count_ceiling_uses_sdk_admission() {
  use polkadot_sdk::frame_support::{dispatch::DispatchClass, weights::Weight};
  use polkadot_sdk::sp_runtime::transaction_validity::InvalidTransaction;
  let call = RuntimeCall::Governance(pallet_governance::Call::cast_vote {
    domain: 101,
    item_id: 110,
    vote: pallet_governance::ProposalVoteKind::Approve,
  });
  let info = call.get_dispatch_info();
  let weights = crate::configs::RuntimeBlockWeights::get();
  assert_eq!(info.class, DispatchClass::Normal);
  assert_eq!(weights.get(info.class).max_total, Some(weights.max_block));
  assert_eq!(weights.get(info.class).reserved, Some(Weight::zero()));
  assert_eq!(info.call_weight.ref_time(), 35_111_357_000);
  assert_eq!(weights.max_block.ref_time(), 2_000_000_000_000);
  let ceiling = weights.max_block.ref_time() / info.call_weight.ref_time();
  assert_eq!(ceiling, 56);
  // Admission projection: omit every earlier base, extension, byte and proof charge.
  polkadot_sdk::sp_io::TestExternalities::default().execute_with(|| {
    for prior_calls in [ceiling - 1, ceiling] {
      let prior_ref_time = info.call_weight.ref_time() * prior_calls;
      let mut used = frame_system::ConsumedWeight::default();
      used.accrue(Weight::from_parts(prior_ref_time, 0), info.class);
      frame_system::BlockWeight::<Runtime>::put(&used);
      let admitted = frame_system::CheckWeight::<Runtime>::do_prepare(
        &info,
        call.encoded_size(),
        call.encoded_size() as u32,
      );
      if prior_calls == ceiling - 1 {
        assert!(admitted.is_ok());
      } else {
        assert_eq!(admitted, Err(InvalidTransaction::ExhaustsResources.into()));
        assert_eq!(System::block_weight(), used);
        assert!(prior_ref_time + info.call_weight.ref_time() > weights.max_block.ref_time());
        assert!(
          info.call_weight.proof_size() + call.encoded_size() as u64
            <= weights.max_block.proof_size()
        );
      }
    }
  });
}

#[test]
fn signed_intent_admission_is_not_limited_to_named_asset_domains() {
  new_test_ext().execute_with(|| {
    let kind = pallet_governance::ProposalPayloadKind::Intent;
    let hash = note_advisory_preimage(ALICE, b"non-protocol intent");
    let fee_sink = actor_fee_sink_account();
    for domain in [101, 102, u32::MAX] {
      assert_ne!(domain, PROTOCOL_GOVERNANCE_DOMAIN);
      assert_ne!(domain, TACTICAL_GOVERNANCE_DOMAIN);
      assert!(!Assets::asset_exists(domain));
      assert!(Staking::pool(domain).is_none());
      let policy = Governance::proposal_admission_policy_view(domain, kind);
      assert_eq!(
        policy.authority,
        pallet_governance::ProposalSubmissionAuthority::Signed
      );
      prepare_payload_witness(ALICE, domain, kind, hash);
      let fee_before = Balances::free_balance(&fee_sink);
      assert_ok!(Governance::submit_signed_proposal(
        RuntimeOrigin::signed(ALICE),
        domain,
        110,
        pallet_governance::ProposalCadenceMode::Ordinary,
        kind,
        hash,
      ));
      assert_eq!(Governance::proposal_author(domain, 110), Some(ALICE));
      assert_eq!(
        Governance::active_proposal_ids(domain).into_inner(),
        vec![110]
      );
      assert!(Governance::payload_admission_witness(hash, (domain, kind)).is_none());
      assert_eq!(
        Balances::free_balance(&fee_sink),
        fee_before + policy.opening_fee.unwrap()
      );
      assert!(!pallet_governance::FinalizedProposals::<Runtime>::contains_key(domain, 110));
    }
  });
}

#[test]
fn same_expiry_domain_indices_follow_signed_intent_on_development_preset() {
  use pallet_governance::{FinalizedProposalOutcome as Outcome, ProposalRejectionReason as Reason};
  let storage = super::production_block_replay::preset_genesis_storage(
    &[],
    &polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into(),
  );
  // Empty CODE is a native placeholder; this drives Governance hooks, not Executive blocks.
  polkadot_sdk::sp_io::TestExternalities::new(storage).execute_with(|| {
    let signer = polkadot_sdk::sp_keyring::Sr25519Keyring::Alice.to_account_id();
    let veto = primitives::ecosystem::protocol_tokens::VETO_ASSET_ID;
    assert!(Assets::asset_exists(veto));
    assert_eq!(Assets::total_issuance(veto), 0);
    assert!(<Runtime as pallet_governance::Config>::MaxMaturingProposalsPerBlock::get() >= 2);
    assert!(<Runtime as pallet_governance::Config>::MaxFinalizedProposalOutcomesPerBlock::get() >= 2);
    System::set_block_number(1);
    let _ = Governance::on_initialize(1);
    assert_eq!(pallet_governance::LastProcessedEpoch::<Runtime>::get(), 1);
    let kind = pallet_governance::ProposalPayloadKind::Intent;
    let hash = note_advisory_preimage(signer.clone(), b"same expiry domains");
    let domains = [101, 102];
    let fee_sink = actor_fee_sink_account();
    let fees_before = Balances::free_balance(&fee_sink);
    for domain in domains {
      assert_ne!(domain, PROTOCOL_GOVERNANCE_DOMAIN);
      assert_ne!(domain, TACTICAL_GOVERNANCE_DOMAIN);
      assert!(!Assets::asset_exists(domain));
      prepare_payload_witness(signer.clone(), domain, kind, hash);
      assert_ok!(Governance::submit_signed_proposal(
        RuntimeOrigin::signed(signer.clone()), domain, 110,
        pallet_governance::ProposalCadenceMode::Ordinary, kind, hash,
      ));
      assert_eq!(Governance::proposal_author(domain, 110), Some(signer.clone()));
      assert_eq!(Governance::active_proposal_ids(domain).into_inner(), vec![110]);
    }
    let opening_fee = <Runtime as pallet_governance::Config>::ProposalOpeningFee::get();
    assert_eq!(Balances::free_balance(&fee_sink), fees_before + 2 * opening_fee);
    let maturity = 1u32
      .checked_add(<Runtime as pallet_governance::Config>::ProposalLeadInPeriod::get()).unwrap()
      .checked_add(<Runtime as pallet_governance::Config>::ProposalVotingPeriod::get()).unwrap();
    let retention: u32 = <Runtime as pallet_governance::Config>::FinalizedProposalOutcomeRetentionEpochs::get();
    let expiry = maturity.checked_add(retention).unwrap();
    // Allow the control turns following the two nonempty phases, not a cursor reposition.
    let end = expiry.checked_add(3).unwrap();
    let ledger = (System::account(&signer), System::account(&fee_sink), Balances::total_issuance());
    let mut cleaned_at = None;
    for block in 2..=end {
      System::set_block_number(block);
      let _ = Governance::on_initialize(block);
      if block == maturity - 1 {
        assert_eq!(pallet_governance::LastProcessedEpoch::<Runtime>::get(), block);
        for domain in domains {
          assert_eq!(Governance::active_proposal_ids(domain).into_inner(), vec![110]);
          assert!(Governance::finalized_proposal_outcome(domain, 110).is_none());
        }
      }
      if block == maturity {
        let bucket = Governance::finalized_proposal_outcome_expiry_bucket(expiry);
        assert_eq!(bucket.len(), 2);
        for domain in domains {
          assert_eq!(Governance::finalized_proposal_outcome(domain, 110), Some(Outcome::Rejected {
            finalized_epoch: maturity, reason: Reason::NoVotes,
          }));
          assert!(bucket.iter().any(|touch| touch.domain == domain && touch.item_id == 110));
          assert_eq!(Governance::finalized_proposal_ids(domain).into_inner(), vec![110]);
          assert!(Governance::proposal_pending_enactment_at(domain, 110).is_none());
        }
        assert!(pallet_governance::PendingEnactments::<Runtime>::get().is_empty());
      }
      if block >= maturity {
        let retained = domains.iter().filter(|domain| pallet_governance::FinalizedProposals::<Runtime>::contains_key(**domain, 110)).count();
        assert!(retained == 0 || retained == domains.len());
        if block < expiry {
          assert_eq!(retained, domains.len());
          assert!(pallet_governance::FinalizedProposalOutcomeExpiryBuckets::<Runtime>::contains_key(expiry));
          for domain in domains {
            assert!(pallet_governance::FinalizedProposalIdsByDomain::<Runtime>::contains_key(domain));
            assert!(pallet_governance::ProposalMetadataByItem::<Runtime>::contains_key(domain, 110));
            assert!(!pallet_governance::ProposalWinningPrimaryOptionByItem::<Runtime>::contains_key(domain, 110));
            assert!(!pallet_governance::ProposalUrgentAuthorizedAt::<Runtime>::contains_key(domain, 110));
          }
        }
        if retained == 0 {
          assert!(block >= expiry);
          cleaned_at.get_or_insert(block);
        }
      }
    }
    let cleaned_at = cleaned_at.expect("ordinary hook progression must service both expired indices");
    for domain in domains {
      assert!(Governance::active_proposal_ids(domain).is_empty());
      assert!(Governance::finalized_proposal_ids(domain).is_empty());
      assert!(!pallet_governance::ProposalMetadataByItem::<Runtime>::contains_key(domain, 110));
      assert!(Governance::proposal_pending_enactment_at(domain, 110).is_none());
    }
    assert!(Governance::finalized_proposal_outcome_expiry_bucket(expiry).is_empty());
    assert_eq!((System::account(&signer), System::account(&fee_sink), Balances::total_issuance()), ledger);
    assert_eq!(Assets::total_issuance(veto), 0);
    println!("GOVERNANCE_SIGNED_DOMAIN_HISTORY domains=2 maturity={maturity} expiry={expiry} cleaned_at={cleaned_at} root_resolution=false cursor_reset=false executive_replay=false");
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
