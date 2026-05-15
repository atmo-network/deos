use super::common::{ALICE, BOB, new_test_ext};
use crate::configs::governance_config::{
  StrategicRuntimeUpgradePayload, TacticalTreasuryFundingSource, TacticalTreasuryInvoicePayload,
};
use crate::{
  AAA, Assets, AxialRouter, Balances, Governance, Preimage, Runtime, RuntimeEvent, RuntimeOrigin,
  System,
};
use codec::Encode;
use polkadot_sdk::frame_support::assert_ok;
use polkadot_sdk::frame_support::traits::{
  Hooks,
  fungibles::{Inspect as FungiblesInspect, Mutate as FungiblesMutate},
};
use polkadot_sdk::frame_system;
use polkadot_sdk::sp_core::traits::{ReadRuntimeVersion, ReadRuntimeVersionExt};
use polkadot_sdk::sp_externalities::Externalities;
use polkadot_sdk::sp_runtime::{MultiAddress, traits::Hash as _};

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

fn submit_signed_intent_proposal(item_id: u32, payload_hash: crate::Hash) {
  assert_ok!(Governance::submit_signed_proposal(
    RuntimeOrigin::signed(ALICE),
    PROTOCOL_GOVERNANCE_DOMAIN,
    item_id,
    pallet_governance::ProposalCadenceMode::Ordinary,
    pallet_governance::ProposalPayloadKind::Intent,
    payload_hash,
  ));
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
fn l1_root_action_authorize_upgrade_executes_from_governance_preimage() {
  new_test_ext().execute_with(|| {
    let approved_epoch = 1;
    let executed_epoch = ordinary_enactment_epoch(approved_epoch);
    let code_hash = crate::Hash::repeat_byte(7);
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
      Some(pallet_governance::FinalizedProposalOutcome::Enacted {
        approved_epoch,
        executed_epoch,
        winner_count: 1,
      })
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
      Some(pallet_governance::ProposalExecutionDetail::Executed {
        payload_kind: pallet_governance::ProposalPayloadKind::L1RootAction,
        authority: pallet_governance::ProposalExecutionAuthority::Root,
        executed_epoch,
        detail: pallet_governance::ProposalExecutionSuccessDetail::RuntimeUpgradeAuthorized {
          code_hash,
        },
      })
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
      pallet_axial_router::Call::<Runtime>::update_router_fee { new_fee }.into();
    let encoded_call = call.encode();
    let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&encoded_call);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      encoded_call,
    ));
    assert_ne!(AxialRouter::router_fee(), new_fee);
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
    assert_eq!(AxialRouter::router_fee(), new_fee);
    assert_eq!(
      Governance::finalized_proposal_outcome(PROTOCOL_GOVERNANCE_DOMAIN, 102),
      Some(pallet_governance::FinalizedProposalOutcome::Enacted {
        approved_epoch,
        executed_epoch,
        winner_count: 1,
      })
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
    let failed_epoch = ordinary_enactment_epoch(1);
    let initial_fee = AxialRouter::router_fee();
    let new_fee = polkadot_sdk::sp_runtime::Perbill::from_percent(2);
    let call: crate::RuntimeCall =
      pallet_axial_router::Call::<Runtime>::update_router_fee { new_fee }.into();
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
    assert_eq!(AxialRouter::router_fee(), initial_fee);
    assert_eq!(
      Governance::proposal_execution_detail(PROTOCOL_GOVERNANCE_DOMAIN, 113),
      Some(
        pallet_governance::ProposalExecutionDetail::ExecutionFailed {
          payload_kind: pallet_governance::ProposalPayloadKind::L2ParameterChange,
          authority: pallet_governance::ProposalExecutionAuthority::DomainParameters,
          failed_epoch,
          reason: pallet_governance::ProposalExecutionFailureReason::DispatchFailed,
        }
      )
    );
  });
}

#[test]
fn l2_parameter_change_adds_tracked_asset_via_governance_executor() {
  new_test_ext().execute_with(|| {
    let approved_epoch = 1;
    let executed_epoch = ordinary_enactment_epoch(approved_epoch);
    let asset = primitives::AssetKind::Local(0x2000_0010);
    let call: crate::RuntimeCall =
      pallet_axial_router::Call::<Runtime>::add_tracked_asset { asset }.into();
    let encoded_call = call.encode();
    let payload_hash = <Runtime as frame_system::Config>::Hashing::hash(&encoded_call);
    assert_ok!(Preimage::note_preimage(
      RuntimeOrigin::signed(ALICE),
      encoded_call,
    ));
    assert!(!pallet_axial_router::TrackedAssets::<Runtime>::get().contains(&asset));
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      PROTOCOL_GOVERNANCE_DOMAIN,
      106,
      ALICE,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::L2ParameterChange,
      payload_hash,
    ));
    resolve_root_action_proposal(106);
    assert!(pallet_axial_router::TrackedAssets::<Runtime>::get().contains(&asset));
    assert!(System::events().iter().any(|record| {
      record.event
        == RuntimeEvent::Governance(pallet_governance::Event::ProposalParameterChangeExecuted {
          domain: PROTOCOL_GOVERNANCE_DOMAIN,
          item_id: 106,
          approved_epoch,
          executed_epoch,
          surface: pallet_governance::ProposalParameterChangeSurface::TrackedAsset,
        })
    }));
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
      Some(
        pallet_governance::FinalizedProposalOutcome::AdvisoryFinalized {
          approved_epoch,
          finalized_epoch,
          winner_count: 1,
        }
      )
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
    let treasury_account =
      AAA::sovereign_account_id_system(primitives::ecosystem::aaa_ids::BLDR_TREASURY_AAA_ID);
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
      Some(pallet_governance::FinalizedProposalOutcome::Enacted {
        approved_epoch,
        executed_epoch,
        winner_count: 1,
      })
    );
    assert_eq!(
      Assets::balance(TACTICAL_GOVERNANCE_DOMAIN, &BOB),
      bob_before.saturating_add(spend_amount)
    );
    assert_eq!(
      Governance::proposal_execution_detail(TACTICAL_GOVERNANCE_DOMAIN, 103),
      Some(pallet_governance::ProposalExecutionDetail::Executed {
        payload_kind: pallet_governance::ProposalPayloadKind::L2TreasurySpend,
        authority: pallet_governance::ProposalExecutionAuthority::DomainTreasury,
        executed_epoch,
        detail: pallet_governance::ProposalExecutionSuccessDetail::TreasurySpendExecuted {
          funding_source: treasury_account.clone(),
          beneficiary: BOB,
          payout_asset: TACTICAL_GOVERNANCE_DOMAIN,
          base_amount: spend_amount,
          scalar: pallet_governance::ProposalTreasurySpendScalar::Approve,
          final_amount: spend_amount,
          settlement_kind:
            pallet_governance::ProposalTreasurySpendSettlementKind::InvoiceScalarTransfer,
        },
      })
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
    let treasury_account =
      AAA::sovereign_account_id_system(primitives::ecosystem::aaa_ids::BLDR_TREASURY_AAA_ID);
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
      Some(pallet_governance::FinalizedProposalOutcome::Enacted {
        approved_epoch,
        executed_epoch,
        winner_count: 1,
      })
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
    let treasury_account =
      AAA::sovereign_account_id_system(primitives::ecosystem::aaa_ids::BLDR_TREASURY_AAA_ID);
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
      Some(
        pallet_governance::FinalizedProposalOutcome::ExecutionFailed {
          approved_epoch: 1,
          failed_epoch: ordinary_enactment_epoch(1),
          winner_count: 1,
        }
      )
    );
    assert_eq!(
      Governance::proposal_execution_detail(TACTICAL_GOVERNANCE_DOMAIN, 108),
      Some(
        pallet_governance::ProposalExecutionDetail::ExecutionFailed {
          payload_kind: pallet_governance::ProposalPayloadKind::L2TreasurySpend,
          authority: pallet_governance::ProposalExecutionAuthority::DomainTreasury,
          failed_epoch: ordinary_enactment_epoch(1),
          reason: pallet_governance::ProposalExecutionFailureReason::MissingWinningPrimaryOption,
        }
      )
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
      Some(
        pallet_governance::FinalizedProposalOutcome::ExecutionFailed {
          approved_epoch: 1,
          failed_epoch,
          winner_count: 1,
        }
      )
    );
    assert_eq!(
      Governance::proposal_execution_detail(PROTOCOL_GOVERNANCE_DOMAIN, 108),
      Some(
        pallet_governance::ProposalExecutionDetail::ExecutionFailed {
          payload_kind: pallet_governance::ProposalPayloadKind::L1RootAction,
          authority: pallet_governance::ProposalExecutionAuthority::Root,
          failed_epoch,
          reason: pallet_governance::ProposalExecutionFailureReason::MissingPreimage,
        }
      )
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
      Some(
        pallet_governance::FinalizedProposalOutcome::ExecutionFailed {
          approved_epoch: 1,
          failed_epoch,
          winner_count: 1,
        }
      )
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
    assert_eq!(
      Governance::proposal_submission_authority(
        PROTOCOL_GOVERNANCE_DOMAIN,
        pallet_governance::ProposalPayloadKind::Intent,
      ),
      pallet_governance::ProposalSubmissionAuthority::Signed
    );
    assert_eq!(
      Governance::proposal_opening_fee(
        PROTOCOL_GOVERNANCE_DOMAIN,
        pallet_governance::ProposalPayloadKind::Intent,
      ),
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
    assert_eq!(
      Governance::proposal_submission_authority(
        PROTOCOL_GOVERNANCE_DOMAIN,
        pallet_governance::ProposalPayloadKind::L1RootAction,
      ),
      pallet_governance::ProposalSubmissionAuthority::AdminOnly
    );
    assert_eq!(
      Governance::proposal_opening_fee(
        PROTOCOL_GOVERNANCE_DOMAIN,
        pallet_governance::ProposalPayloadKind::L1RootAction,
      ),
      None
    );
    assert_eq!(
      Governance::proposal_submission_authority(
        TACTICAL_GOVERNANCE_DOMAIN,
        pallet_governance::ProposalPayloadKind::L2SignalToL1,
      ),
      pallet_governance::ProposalSubmissionAuthority::Signed
    );
    assert_eq!(
      Governance::proposal_opening_fee(
        TACTICAL_GOVERNANCE_DOMAIN,
        pallet_governance::ProposalPayloadKind::L2SignalToL1,
      ),
      Some(10 * crate::EXISTENTIAL_DEPOSIT)
    );
    assert_eq!(
      Governance::proposal_submission_authority(
        TACTICAL_GOVERNANCE_DOMAIN,
        pallet_governance::ProposalPayloadKind::L2TreasurySpend,
      ),
      pallet_governance::ProposalSubmissionAuthority::Signed
    );
    assert_eq!(
      Governance::proposal_opening_fee(
        TACTICAL_GOVERNANCE_DOMAIN,
        pallet_governance::ProposalPayloadKind::L2TreasurySpend,
      ),
      Some(10 * crate::EXISTENTIAL_DEPOSIT)
    );
  });
}

#[test]
fn signed_intent_submission_burns_opening_fee_and_records_signer_as_proposer() {
  new_test_ext().execute_with(|| {
    let balance_before = Balances::free_balance(ALICE);
    submit_signed_intent_proposal(110, crate::Hash::repeat_byte(12));
    assert_eq!(
      Governance::proposal_author(PROTOCOL_GOVERNANCE_DOMAIN, 110),
      Some(ALICE)
    );
    assert_eq!(
      Balances::free_balance(ALICE),
      balance_before.saturating_sub(10 * crate::EXISTENTIAL_DEPOSIT)
    );
    assert!(System::events().iter().any(|record| {
      record.event
        == RuntimeEvent::Governance(pallet_governance::Event::ProposalOpeningFeeBurned {
          domain: PROTOCOL_GOVERNANCE_DOMAIN,
          item_id: 110,
          proposer: ALICE,
          amount: 10 * crate::EXISTENTIAL_DEPOSIT,
        })
    }));
  });
}

#[test]
fn signed_tactical_l2_signal_submission_burns_opening_fee_and_records_signer() {
  new_test_ext().execute_with(|| {
    let balance_before = Balances::free_balance(ALICE);
    assert_ok!(Governance::submit_signed_proposal(
      RuntimeOrigin::signed(ALICE),
      TACTICAL_GOVERNANCE_DOMAIN,
      111,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::L2SignalToL1,
      crate::Hash::repeat_byte(13),
    ));
    assert_eq!(
      Governance::proposal_author(TACTICAL_GOVERNANCE_DOMAIN, 111),
      Some(ALICE)
    );
    assert_eq!(
      Balances::free_balance(ALICE),
      balance_before.saturating_sub(10 * crate::EXISTENTIAL_DEPOSIT)
    );
  });
}

#[test]
fn signed_tactical_treasury_submission_burns_opening_fee_and_records_signer() {
  new_test_ext().execute_with(|| {
    let balance_before = Balances::free_balance(ALICE);
    assert_ok!(Governance::submit_signed_proposal(
      RuntimeOrigin::signed(ALICE),
      TACTICAL_GOVERNANCE_DOMAIN,
      112,
      pallet_governance::ProposalCadenceMode::Ordinary,
      pallet_governance::ProposalPayloadKind::L2TreasurySpend,
      crate::Hash::repeat_byte(14),
    ));
    assert_eq!(
      Governance::proposal_author(TACTICAL_GOVERNANCE_DOMAIN, 112),
      Some(ALICE)
    );
    assert_eq!(
      Balances::free_balance(ALICE),
      balance_before.saturating_sub(10 * crate::EXISTENTIAL_DEPOSIT)
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
      Some(pallet_governance::FinalizedProposalOutcome::Enacted {
        approved_epoch: 1,
        executed_epoch: 1,
        winner_count: 0,
      })
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
