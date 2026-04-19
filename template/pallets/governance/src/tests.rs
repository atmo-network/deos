use crate::{
  ActiveProposalCounts, ActiveProposals, Error, Event, ExpiringAccountTouch, ExpiryBuckets,
  FinalizedProposalOutcome, ProposalCadenceMode, ProposalExecutionAuthority, ProposalMetadata,
  ProposalPayloadKind, ProposalPendingEnactmentAt, ProposalRejectionReason, ProposalTiming,
  ProposalUrgentAuthorizedAt, ProposalVoteKind, ProposalVotesByItem, RecentFinalizedProposal,
  mock::*,
};
use polkadot_sdk::frame_support::{BoundedVec, assert_noop, assert_ok, traits::Hooks};
use polkadot_sdk::sp_core::H256;
use polkadot_sdk::sp_runtime::FixedU128;

const DEFAULT_PROPOSER: u64 = 99;

fn submit_test_proposal(
  domain: u32,
  item_id: u32,
  proposer: u64,
) -> polkadot_sdk::sp_runtime::DispatchResult {
  Governance::submit_proposal(
    RuntimeOrigin::root(),
    domain,
    item_id,
    proposer,
    ProposalCadenceMode::Ordinary,
    ProposalPayloadKind::L2ParameterChange,
    Default::default(),
  )
}

fn submit_signed_intent_proposal(
  domain: u32,
  item_id: u32,
  proposer: u64,
) -> polkadot_sdk::sp_runtime::DispatchResult {
  Governance::submit_signed_proposal(
    RuntimeOrigin::signed(proposer),
    domain,
    item_id,
    ProposalCadenceMode::Ordinary,
    ProposalPayloadKind::Intent,
    Default::default(),
  )
}

#[test]
fn proposal_submit_resolve_and_reject_follow_active_lifecycle() {
  new_test_ext().execute_with(|| {
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_eq!(ActiveProposalCounts::<Test>::get(7), 1);
    assert!(ActiveProposals::<Test>::contains_key(7, 100));
    assert_eq!(Governance::active_proposal_ids(7).into_inner(), vec![100]);
    System::assert_last_event(RuntimeEvent::Governance(Event::ProposalSubmitted {
      domain: 7,
      item_id: 100,
      proposer: DEFAULT_PROPOSER,
      cadence_mode: ProposalCadenceMode::Ordinary,
      payload_kind: ProposalPayloadKind::L2ParameterChange,
      payload_hash: Default::default(),
      epoch: 1,
      active_count: 1,
    }));
    let winners =
      BoundedVec::try_from(vec![10u64, 11u64]).expect("proposal winners must fit configured bound");
    assert_ok!(Governance::resolve_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      winners,
    ));
    assert_eq!(ActiveProposalCounts::<Test>::get(7), 0);
    assert!(!ActiveProposals::<Test>::contains_key(7, 100));
    assert!(Governance::active_proposal_ids(7).is_empty());
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_rational(1u128, 6u128)
    );
    System::assert_last_event(RuntimeEvent::Governance(Event::ProposalResolved {
      domain: 7,
      item_id: 100,
      epoch: 1,
      winner_count: 2,
      active_count: 0,
    }));
    assert_ok!(submit_test_proposal(7, 101, DEFAULT_PROPOSER));
    assert_eq!(Governance::active_proposal_ids(7).into_inner(), vec![101]);
    assert_ok!(Governance::reject_proposal(RuntimeOrigin::root(), 7, 101));
    assert!(!ActiveProposals::<Test>::contains_key(7, 101));
    assert!(Governance::active_proposal_ids(7).is_empty());
    System::assert_last_event(RuntimeEvent::Governance(Event::ProposalRejected {
      domain: 7,
      item_id: 101,
      epoch: 1,
      reason: ProposalRejectionReason::AdminRejected,
      active_count: 0,
    }));
  });
}

#[test]
fn finalized_outcome_is_stored_and_later_expires() {
  new_test_ext().execute_with(|| {
    let winners =
      BoundedVec::try_from(vec![10u64, 11u64]).expect("proposal winners must fit configured bound");
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::resolve_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      winners,
    ));
    assert_eq!(
      Governance::finalized_proposal_outcome(7, 100),
      Some(FinalizedProposalOutcome::Resolved {
        epoch: 1,
        winner_count: 2,
      })
    );
    assert_eq!(
      Governance::recent_finalized_proposals(7).into_inner(),
      vec![RecentFinalizedProposal {
        item_id: 100,
        outcome: FinalizedProposalOutcome::Resolved {
          epoch: 1,
          winner_count: 2,
        },
      }]
    );
    System::set_block_number(4);
    Governance::on_initialize(4);
    assert!(Governance::finalized_proposal_outcome(7, 100).is_none());
    assert!(Governance::recent_finalized_proposals(7).is_empty());
  });
}

#[test]
fn rejected_outcome_is_stored_with_reason() {
  new_test_ext().execute_with(|| {
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::reject_proposal(RuntimeOrigin::root(), 7, 100));
    assert_eq!(
      Governance::finalized_proposal_outcome(7, 100),
      Some(FinalizedProposalOutcome::Rejected {
        epoch: 1,
        reason: ProposalRejectionReason::AdminRejected,
      })
    );
  });
}

#[test]
fn recent_finalized_proposals_are_sorted_newest_first_per_domain() {
  new_test_ext().execute_with(|| {
    let winners =
      BoundedVec::try_from(vec![10u64]).expect("proposal winners must fit configured bound");
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::resolve_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      winners.clone(),
    ));
    System::set_block_number(2);
    assert_ok!(submit_test_proposal(7, 101, DEFAULT_PROPOSER));
    assert_ok!(Governance::reject_proposal(RuntimeOrigin::root(), 7, 101));
    assert_ok!(submit_test_proposal(8, 200, DEFAULT_PROPOSER));
    assert_ok!(Governance::reject_proposal(RuntimeOrigin::root(), 8, 200));
    System::set_block_number(3);
    assert_ok!(submit_test_proposal(7, 102, DEFAULT_PROPOSER));
    assert_ok!(Governance::resolve_proposal(
      RuntimeOrigin::root(),
      7,
      102,
      winners,
    ));
    assert_eq!(
      Governance::recent_finalized_proposals(7).into_inner(),
      vec![
        RecentFinalizedProposal {
          item_id: 102,
          outcome: FinalizedProposalOutcome::Resolved {
            epoch: 3,
            winner_count: 1,
          },
        },
        RecentFinalizedProposal {
          item_id: 101,
          outcome: FinalizedProposalOutcome::Rejected {
            epoch: 2,
            reason: ProposalRejectionReason::AdminRejected,
          },
        },
        RecentFinalizedProposal {
          item_id: 100,
          outcome: FinalizedProposalOutcome::Resolved {
            epoch: 1,
            winner_count: 1,
          },
        },
      ]
    );
    assert_eq!(
      Governance::recent_finalized_proposals(8).into_inner(),
      vec![RecentFinalizedProposal {
        item_id: 200,
        outcome: FinalizedProposalOutcome::Rejected {
          epoch: 2,
          reason: ProposalRejectionReason::AdminRejected,
        },
      }]
    );
  });
}

#[test]
fn proposal_resolution_rejects_unknown_or_empty_winner_sets() {
  new_test_ext().execute_with(|| {
    let winners =
      BoundedVec::try_from(vec![10u64]).expect("proposal winners must fit configured bound");
    assert_noop!(
      Governance::resolve_proposal(RuntimeOrigin::root(), 7, 100, winners),
      Error::<Test>::ProposalNotActive
    );
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_noop!(
      Governance::resolve_proposal(RuntimeOrigin::root(), 7, 100, BoundedVec::default()),
      Error::<Test>::ProposalWinnerSetEmpty
    );
  });
}

#[test]
fn vote_resolution_without_ballots_rejects_the_proposal() {
  new_test_ext().execute_with(|| {
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    System::set_block_number(3);
    Governance::on_initialize(3);
    assert!(!ActiveProposals::<Test>::contains_key(7, 100));
    System::assert_last_event(RuntimeEvent::Governance(Event::ProposalRejected {
      domain: 7,
      item_id: 100,
      epoch: 3,
      reason: ProposalRejectionReason::NoVotes,
      active_count: 0,
    }));
  });
}

#[test]
fn active_proposal_cap_is_enforced_per_domain() {
  new_test_ext().execute_with(|| {
    for item_id in 0..16u32 {
      if item_id > 0 && item_id % 4 == 0 {
        System::set_block_number(System::block_number().saturating_add(1));
      }
      assert_ok!(submit_test_proposal(7, item_id, DEFAULT_PROPOSER));
    }
    assert_noop!(
      submit_test_proposal(7, 16, DEFAULT_PROPOSER),
      Error::<Test>::ActiveProposalCapReached
    );
    assert_eq!(ActiveProposalCounts::<Test>::get(7), 16);
  });
}

#[test]
fn on_initialize_auto_finalizes_matured_proposals() {
  new_test_ext().execute_with(|| {
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_eq!(Governance::proposal_maturity_bucket(3).len(), 1);
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(12),
      7,
      100,
      ProposalVoteKind::Nay,
    ));
    System::set_block_number(3);
    Governance::on_initialize(3);
    assert!(!ActiveProposals::<Test>::contains_key(7, 100));
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_rational(1u128, 6u128)
    );
    assert_eq!(
      Governance::reward_coefficient(7, 11),
      FixedU128::from_rational(1u128, 6u128)
    );
    assert_eq!(
      Governance::reward_coefficient(7, 12),
      FixedU128::from_inner(0)
    );
  });
}

#[test]
fn auto_finalization_defers_when_current_epoch_winner_cap_is_exhausted() {
  new_test_ext().execute_with(|| {
    for item_id in [100u32, 101u32, 102u32] {
      assert_ok!(submit_test_proposal(7, item_id, DEFAULT_PROPOSER));
      assert_ok!(Governance::cast_vote(
        RuntimeOrigin::signed(10),
        7,
        item_id,
        ProposalVoteKind::Aye,
      ));
      assert_ok!(Governance::cast_vote(
        RuntimeOrigin::signed(11),
        7,
        item_id,
        ProposalVoteKind::Aye,
      ));
    }
    System::set_block_number(3);
    Governance::on_initialize(3);
    assert!(ActiveProposals::<Test>::contains_key(7, 102));
    assert_eq!(Governance::proposal_maturity_bucket(4).len(), 1);
    System::assert_last_event(RuntimeEvent::Governance(
      Event::ProposalAutoFinalizationDeferred {
        domain: 7,
        item_id: 102,
        epoch: 3,
        rescheduled: true,
      },
    ));
    System::set_block_number(4);
    Governance::on_initialize(4);
    assert!(!ActiveProposals::<Test>::contains_key(7, 102));
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_rational(3u128, 6u128)
    );
    assert_eq!(
      Governance::reward_coefficient(7, 11),
      FixedU128::from_rational(3u128, 6u128)
    );
  });
}

#[test]
fn admin_can_requeue_unscheduled_auto_finalization_recovery() {
  new_test_ext().execute_with(|| {
    for item_id in [100u32, 101u32, 102u32] {
      assert_ok!(submit_test_proposal(7, item_id, DEFAULT_PROPOSER));
      assert_ok!(Governance::cast_vote(
        RuntimeOrigin::signed(10),
        7,
        item_id,
        ProposalVoteKind::Aye,
      ));
      assert_ok!(Governance::cast_vote(
        RuntimeOrigin::signed(11),
        7,
        item_id,
        ProposalVoteKind::Aye,
      ));
    }
    System::set_block_number(2);
    for item_id in [200u32, 201u32, 202u32, 203u32] {
      assert_ok!(submit_test_proposal(7, item_id, DEFAULT_PROPOSER));
    }
    System::set_block_number(3);
    Governance::on_initialize(3);
    assert!(ActiveProposals::<Test>::contains_key(7, 102));
    assert_eq!(Governance::proposal_maturity_bucket(4).len(), 4);
    assert!(
      Governance::proposal_maturity_bucket(4)
        .iter()
        .all(|entry| entry.item_id != 102)
    );
    System::assert_last_event(RuntimeEvent::Governance(
      Event::ProposalAutoFinalizationDeferred {
        domain: 7,
        item_id: 102,
        epoch: 3,
        rescheduled: false,
      },
    ));
    System::set_block_number(4);
    Governance::on_initialize(4);
    assert!(ActiveProposals::<Test>::contains_key(7, 102));
    assert_ok!(Governance::requeue_proposal_for_auto_finalization(
      RuntimeOrigin::root(),
      7,
      102,
    ));
    assert_eq!(Governance::proposal_maturity_bucket(5).len(), 1);
    System::assert_last_event(RuntimeEvent::Governance(
      Event::ProposalAutoFinalizationRequeued {
        domain: 7,
        item_id: 102,
        epoch: 4,
        maturity_epoch: 5,
      },
    ));
    System::set_block_number(5);
    Governance::on_initialize(5);
    assert!(!ActiveProposals::<Test>::contains_key(7, 102));
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_rational(3u128, 6u128)
    );
    assert_eq!(
      Governance::reward_coefficient(7, 11),
      FixedU128::from_rational(3u128, 6u128)
    );
  });
}

#[test]
fn proposal_query_helpers_report_weighted_tally_and_resolution_state() {
  new_test_ext().execute_with(|| {
    set_vote_weight(10, 5);
    set_vote_weight(11, 1);
    set_vote_weight(12, 1);
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      100,
      ProposalVoteKind::Nay,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(12),
      7,
      100,
      ProposalVoteKind::Nay,
    ));
    let tally = Governance::proposal_vote_tally(7, 100).expect("active proposal must expose tally");
    assert_eq!(tally.aye_voters, 1);
    assert_eq!(tally.nay_voters, 2);
    assert_eq!(tally.veto_voters, 0);
    assert_eq!(tally.aye_weight, 5);
    assert_eq!(tally.nay_weight, 2);
    assert_eq!(tally.veto_weight, 0);
    assert_eq!(
      Governance::proposal_primary_track_tally(7, 100),
      Some(crate::ProposalPrimaryTrackTally::Binary {
        aye_voters: 1,
        nay_voters: 2,
        aye_weight: 5,
        nay_weight: 2,
        turnout_weight: 7,
        leading_option: Some(crate::ProposalPrimaryTrackOption::Aye),
      })
    );
    assert_eq!(
      Governance::proposal_resolution_state(7, 100),
      Some(crate::ProposalResolutionState::VotingWindowOpen {
        current_epoch: 1,
        maturity_epoch: 3,
      })
    );
    assert_eq!(
      Governance::proposal_status(7, 100),
      Some(crate::ProposalStatus::Active(
        crate::ProposalResolutionState::VotingWindowOpen {
          current_epoch: 1,
          maturity_epoch: 3,
        }
      ))
    );
    assert_eq!(
      Governance::proposal_vote_power_profile(7, 100, ProposalVoteKind::Aye),
      Some(crate::ProposalVotePowerProfile::DecliningDirectStake)
    );
    assert_eq!(
      Governance::proposal_vote_power_profile(7, 100, ProposalVoteKind::Veto),
      Some(crate::ProposalVotePowerProfile::DecliningVetoAsset)
    );
    System::set_block_number(3);
    assert_eq!(
      Governance::proposal_resolution_state(7, 100),
      Some(crate::ProposalResolutionState::PassingAye)
    );
    assert_eq!(
      Governance::proposal_status(7, 100),
      Some(crate::ProposalStatus::Active(
        crate::ProposalResolutionState::PassingAye
      ))
    );
    assert_ok!(Governance::force_resolve_proposal_from_votes(
      RuntimeOrigin::root(),
      7,
      100,
    ));
    assert_eq!(
      Governance::proposal_status(7, 100),
      Some(crate::ProposalStatus::Finalized(
        FinalizedProposalOutcome::Resolved {
          epoch: 3,
          winner_count: 1,
        }
      ))
    );
    assert_eq!(
      Governance::proposal_vote_power_profile(7, 100, ProposalVoteKind::Aye),
      None
    );
    assert_eq!(
      Governance::retained_proposal_winning_primary_option(7, 100),
      Some(crate::ProposalPrimaryTrackOption::Aye)
    );
  });
}

#[test]
fn test_submission_helper_uses_ordinary_parameter_change_payload() {
  new_test_ext().execute_with(|| {
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_eq!(
      Governance::proposal_metadata(7, 100),
      Some(ProposalMetadata {
        cadence_mode: ProposalCadenceMode::Ordinary,
        payload_kind: ProposalPayloadKind::L2ParameterChange,
        payload_hash: Default::default(),
      })
    );
  });
}

#[test]
fn metadata_aware_submission_persists_payload_kind_and_hash() {
  new_test_ext().execute_with(|| {
    let payload_hash = polkadot_sdk::sp_core::H256::repeat_byte(7);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      DEFAULT_PROPOSER,
      ProposalCadenceMode::Fast,
      ProposalPayloadKind::L1RootAction,
      payload_hash,
    ));
    assert_eq!(
      Governance::proposal_metadata(7, 100),
      Some(ProposalMetadata {
        cadence_mode: ProposalCadenceMode::Fast,
        payload_kind: ProposalPayloadKind::L1RootAction,
        payload_hash,
      })
    );
    assert_eq!(
      Governance::proposal_execution_authority(7, 100),
      Some(ProposalExecutionAuthority::Root)
    );
  });
}

#[test]
fn proposal_payload_availability_reflects_provider_state() {
  new_test_ext().execute_with(|| {
    let payload_hash = polkadot_sdk::sp_core::H256::repeat_byte(9);
    set_payload_preimage_state(payload_hash, true, true);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      DEFAULT_PROPOSER,
      ProposalCadenceMode::Fast,
      ProposalPayloadKind::L1RootAction,
      payload_hash,
    ));
    assert_eq!(
      Governance::proposal_payload_availability(7, 100),
      Some(crate::ProposalPayloadAvailability {
        have_preimage: true,
        preimage_requested: true,
      })
    );
  });
}

#[test]
fn payload_hash_preimage_status_is_explicit_and_queryable() {
  new_test_ext().execute_with(|| {
    let payload_hash = polkadot_sdk::sp_core::H256::repeat_byte(19);
    set_payload_preimage_state_with_len(payload_hash, true, true, Some(12));
    assert_eq!(
      Governance::payload_hash_preimage_status(payload_hash),
      crate::PayloadHashPreimageStatus {
        have_preimage: true,
        preimage_requested: true,
        payload_len: Some(12),
      }
    );
    assert_eq!(
      Governance::payload_hash_preimage_status(polkadot_sdk::sp_core::H256::repeat_byte(20)),
      crate::PayloadHashPreimageStatus {
        have_preimage: false,
        preimage_requested: false,
        payload_len: None,
      }
    );
  });
}

#[test]
fn executable_payload_stays_resolved_when_executor_is_disabled() {
  new_test_ext().execute_with(|| {
    let payload_hash = polkadot_sdk::sp_core::H256::repeat_byte(10);
    set_payload_preimage_state(payload_hash, true, false);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      DEFAULT_PROPOSER,
      ProposalCadenceMode::Fast,
      ProposalPayloadKind::L1RootAction,
      payload_hash,
    ));
    let winners =
      BoundedVec::try_from(vec![10u64]).expect("proposal winners must fit configured bound");
    assert_ok!(Governance::resolve_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      winners,
    ));
    assert_eq!(
      Governance::finalized_proposal_outcome(7, 100),
      Some(FinalizedProposalOutcome::Resolved {
        epoch: 1,
        winner_count: 1,
      })
    );
  });
}

#[test]
fn executable_payload_executes_immediately_when_executor_is_enabled() {
  new_test_ext().execute_with(|| {
    let payload_hash = polkadot_sdk::sp_core::H256::repeat_byte(11);
    set_payload_preimage_state(payload_hash, true, false);
    set_payload_executor_enabled(true);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      DEFAULT_PROPOSER,
      ProposalCadenceMode::Fast,
      ProposalPayloadKind::L1RootAction,
      payload_hash,
    ));
    let winners =
      BoundedVec::try_from(vec![10u64]).expect("proposal winners must fit configured bound");
    assert_ok!(Governance::resolve_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      winners,
    ));
    assert_eq!(
      Governance::finalized_proposal_outcome(7, 100),
      Some(FinalizedProposalOutcome::Enacted {
        approved_epoch: 1,
        executed_epoch: 1,
        winner_count: 1,
      })
    );
    assert_eq!(
      Governance::proposal_execution_detail(7, 100),
      Some(crate::ProposalExecutionDetail::Executed {
        payload_kind: ProposalPayloadKind::L1RootAction,
        authority: ProposalExecutionAuthority::Root,
        executed_epoch: 1,
        detail: crate::ProposalExecutionSuccessDetail::Generic,
      })
    );
    let events = System::events()
      .into_iter()
      .map(|record| record.event)
      .collect::<Vec<_>>();
    assert!(
      events.contains(&RuntimeEvent::Governance(Event::ProposalExecuted {
        domain: 7,
        item_id: 100,
        approved_epoch: 1,
        executed_epoch: 1,
        authority: ProposalExecutionAuthority::Root,
        payload_kind: ProposalPayloadKind::L1RootAction,
      }))
    );
  });
}

#[test]
fn executable_payload_without_preimage_fails_when_executor_is_enabled() {
  new_test_ext().execute_with(|| {
    let payload_hash = polkadot_sdk::sp_core::H256::repeat_byte(12);
    set_payload_executor_enabled(true);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      DEFAULT_PROPOSER,
      ProposalCadenceMode::Fast,
      ProposalPayloadKind::L1RootAction,
      payload_hash,
    ));
    let winners =
      BoundedVec::try_from(vec![10u64]).expect("proposal winners must fit configured bound");
    assert_ok!(Governance::resolve_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      winners,
    ));
    assert_eq!(
      Governance::finalized_proposal_outcome(7, 100),
      Some(FinalizedProposalOutcome::ExecutionFailed {
        approved_epoch: 1,
        failed_epoch: 1,
        winner_count: 1,
      })
    );
    assert_eq!(
      Governance::proposal_execution_detail(7, 100),
      Some(crate::ProposalExecutionDetail::ExecutionFailed {
        payload_kind: ProposalPayloadKind::L1RootAction,
        authority: ProposalExecutionAuthority::Root,
        failed_epoch: 1,
        reason: crate::ProposalExecutionFailureReason::MissingPreimage,
      })
    );
    let events = System::events()
      .into_iter()
      .map(|record| record.event)
      .collect::<Vec<_>>();
    assert!(
      events.contains(&RuntimeEvent::Governance(Event::ProposalExecutionFailed {
        domain: 7,
        item_id: 100,
        approved_epoch: 1,
        failed_epoch: 1,
        authority: ProposalExecutionAuthority::Root,
        payload_kind: ProposalPayloadKind::L1RootAction,
        reason: crate::ProposalExecutionFailureReason::MissingPreimage,
      }))
    );
  });
}

#[test]
fn advisory_payload_finalizes_without_dispatch() {
  new_test_ext().execute_with(|| {
    let payload_hash = polkadot_sdk::sp_core::H256::repeat_byte(13);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      DEFAULT_PROPOSER,
      ProposalCadenceMode::Fast,
      ProposalPayloadKind::Intent,
      payload_hash,
    ));
    let winners =
      BoundedVec::try_from(vec![10u64]).expect("proposal winners must fit configured bound");
    assert_ok!(Governance::resolve_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      winners,
    ));
    assert_eq!(
      Governance::finalized_proposal_outcome(7, 100),
      Some(FinalizedProposalOutcome::AdvisoryFinalized {
        approved_epoch: 1,
        finalized_epoch: 1,
        winner_count: 1,
      })
    );
    assert_eq!(
      Governance::proposal_execution_detail(7, 100),
      Some(crate::ProposalExecutionDetail::AdvisoryFinalized {
        payload_kind: ProposalPayloadKind::Intent,
        finalized_epoch: 1,
      })
    );
    let events = System::events()
      .into_iter()
      .map(|record| record.event)
      .collect::<Vec<_>>();
    assert!(events.contains(&RuntimeEvent::Governance(
      Event::ProposalAdvisoryFinalized {
        domain: 7,
        item_id: 100,
        approved_epoch: 1,
        finalized_epoch: 1,
        payload_kind: ProposalPayloadKind::Intent,
      }
    )));
  });
}

#[test]
fn l2_signal_to_l1_payload_finalization_emits_advisory_kind() {
  new_test_ext().execute_with(|| {
    let payload_hash = polkadot_sdk::sp_core::H256::repeat_byte(23);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      7,
      101,
      DEFAULT_PROPOSER,
      ProposalCadenceMode::Fast,
      ProposalPayloadKind::L2SignalToL1,
      payload_hash,
    ));
    let winners =
      BoundedVec::try_from(vec![10u64]).expect("proposal winners must fit configured bound");
    assert_ok!(Governance::resolve_proposal(
      RuntimeOrigin::root(),
      7,
      101,
      winners,
    ));
    assert_eq!(
      Governance::finalized_proposal_outcome(7, 101),
      Some(FinalizedProposalOutcome::AdvisoryFinalized {
        approved_epoch: 1,
        finalized_epoch: 1,
        winner_count: 1,
      })
    );
    let events = System::events()
      .into_iter()
      .map(|record| record.event)
      .collect::<Vec<_>>();
    assert!(events.contains(&RuntimeEvent::Governance(
      Event::ProposalAdvisoryFinalized {
        domain: 7,
        item_id: 101,
        approved_epoch: 1,
        finalized_epoch: 1,
        payload_kind: ProposalPayloadKind::L2SignalToL1,
      }
    )));
  });
}

#[test]
fn executable_payload_records_execution_failed_when_executor_errors() {
  new_test_ext().execute_with(|| {
    let payload_hash = polkadot_sdk::sp_core::H256::repeat_byte(15);
    set_payload_preimage_state(payload_hash, true, false);
    set_payload_executor_enabled(true);
    set_payload_execution_result(payload_hash, false);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      DEFAULT_PROPOSER,
      ProposalCadenceMode::Fast,
      ProposalPayloadKind::L1RootAction,
      payload_hash,
    ));
    let winners =
      BoundedVec::try_from(vec![10u64]).expect("proposal winners must fit configured bound");
    assert_ok!(Governance::resolve_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      winners,
    ));
    assert_eq!(
      Governance::finalized_proposal_outcome(7, 100),
      Some(FinalizedProposalOutcome::ExecutionFailed {
        approved_epoch: 1,
        failed_epoch: 1,
        winner_count: 1,
      })
    );
    assert_eq!(
      Governance::proposal_execution_detail(7, 100),
      Some(crate::ProposalExecutionDetail::ExecutionFailed {
        payload_kind: ProposalPayloadKind::L1RootAction,
        authority: ProposalExecutionAuthority::Root,
        failed_epoch: 1,
        reason: crate::ProposalExecutionFailureReason::DispatchFailed,
      })
    );
    let events = System::events()
      .into_iter()
      .map(|record| record.event)
      .collect::<Vec<_>>();
    assert!(
      events.contains(&RuntimeEvent::Governance(Event::ProposalExecutionFailed {
        domain: 7,
        item_id: 100,
        approved_epoch: 1,
        failed_epoch: 1,
        authority: ProposalExecutionAuthority::Root,
        payload_kind: ProposalPayloadKind::L1RootAction,
        reason: crate::ProposalExecutionFailureReason::DispatchFailed,
      }))
    );
  });
}

#[test]
fn pending_enactment_executes_when_due_and_executor_is_enabled() {
  new_test_ext().execute_with(|| {
    let payload_hash = polkadot_sdk::sp_core::H256::repeat_byte(14);
    set_payload_preimage_state(payload_hash, true, false);
    set_payload_executor_enabled(true);
    ProposalEnactmentDelay::set(2);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      DEFAULT_PROPOSER,
      ProposalCadenceMode::Fast,
      ProposalPayloadKind::L1RootAction,
      payload_hash,
    ));
    let winners =
      BoundedVec::try_from(vec![10u64]).expect("proposal winners must fit configured bound");
    assert_ok!(Governance::resolve_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      winners,
    ));
    assert_eq!(
      Governance::proposal_status(7, 100),
      Some(crate::ProposalStatus::PendingEnactment {
        outcome: FinalizedProposalOutcome::Resolved {
          epoch: 1,
          winner_count: 1,
        },
        enactment_epoch: 3,
      })
    );
    System::set_block_number(3);
    Governance::on_initialize(3);
    assert_eq!(
      Governance::finalized_proposal_outcome(7, 100),
      Some(FinalizedProposalOutcome::Enacted {
        approved_epoch: 1,
        executed_epoch: 3,
        winner_count: 1,
      })
    );
    assert!(!ProposalPendingEnactmentAt::<Test>::contains_key(7, 100));
  });
}

#[test]
fn test_submission_helper_derives_domain_parameter_authority() {
  new_test_ext().execute_with(|| {
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_eq!(
      Governance::proposal_execution_authority(7, 100),
      Some(ProposalExecutionAuthority::DomainParameters)
    );
  });
}

#[test]
fn proposal_timing_reports_current_ordinary_schedule() {
  new_test_ext().execute_with(|| {
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_eq!(
      Governance::proposal_timing(7, 100),
      Some(ProposalTiming {
        submitted_epoch: 1,
        protection_open_epoch: 1,
        protection_close_epoch: 3,
        ordinary_primary_open_epoch: 1,
        ordinary_primary_close_epoch: 3,
        urgent_primary_open_epoch: None,
        urgent_primary_close_epoch: None,
        effective_primary_open_epoch: 1,
        effective_primary_close_epoch: 3,
        pending_enactment_epoch: None,
      })
    );
  });
}

#[test]
fn proposal_timing_reports_scaffolded_urgent_and_enactment_epochs() {
  new_test_ext().execute_with(|| {
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    ProposalUrgentAuthorizedAt::<Test>::insert(7, 100, 2);
    ProposalPendingEnactmentAt::<Test>::insert(7, 100, 5);
    assert_eq!(
      Governance::proposal_timing(7, 100),
      Some(ProposalTiming {
        submitted_epoch: 1,
        protection_open_epoch: 1,
        protection_close_epoch: 3,
        ordinary_primary_open_epoch: 1,
        ordinary_primary_close_epoch: 3,
        urgent_primary_open_epoch: Some(2),
        urgent_primary_close_epoch: Some(3),
        effective_primary_open_epoch: 2,
        effective_primary_close_epoch: 3,
        pending_enactment_epoch: Some(5),
      })
    );
  });
}

#[test]
fn proposal_primary_track_family_is_explicit_even_before_invoice_activation() {
  new_test_ext().execute_with(|| {
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      43,
      101,
      DEFAULT_PROPOSER,
      ProposalCadenceMode::Ordinary,
      ProposalPayloadKind::L2TreasurySpend,
      Default::default(),
    ));
    assert_eq!(
      Governance::proposal_primary_track_family(7, 100),
      Some(crate::ProposalPrimaryTrackFamily::Binary)
    );
    assert_eq!(
      Governance::proposal_primary_track_family(43, 101),
      Some(crate::ProposalPrimaryTrackFamily::Invoice)
    );
    assert_eq!(Governance::proposal_primary_track_family(7, 999), None);
  });
}

#[test]
fn invoice_family_accepts_invoice_primary_votes_and_rejects_binary_aye() {
  new_test_ext().execute_with(|| {
    set_vote_weight(10, 5);
    set_vote_weight(11, 5);
    set_vote_weight(12, 5);
    set_vote_weight(13, 2);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      43,
      102,
      DEFAULT_PROPOSER,
      ProposalCadenceMode::Ordinary,
      ProposalPayloadKind::L2TreasurySpend,
      Default::default(),
    ));
    assert_noop!(
      Governance::cast_vote(RuntimeOrigin::signed(10), 43, 102, ProposalVoteKind::Aye),
      Error::<Test>::ProposalVoteKindNotAllowedForPrimaryTrackFamily
    );
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      43,
      102,
      ProposalVoteKind::Amplify,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      43,
      102,
      ProposalVoteKind::Approve,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(12),
      43,
      102,
      ProposalVoteKind::Reduce,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(13),
      43,
      102,
      ProposalVoteKind::Nay,
    ));
    let votes =
      ProposalVotesByItem::<Test>::get(43, 102).expect("invoice proposal votes must exist");
    assert_eq!(votes.amplifies.len(), 1);
    assert_eq!(votes.approves.len(), 1);
    assert_eq!(votes.reduces.len(), 1);
    assert_eq!(votes.nays.len(), 1);
    assert!(votes.ayes.is_empty());
    assert_eq!(
      Governance::proposal_primary_track_tally(43, 102),
      Some(crate::ProposalPrimaryTrackTally::Invoice {
        amplify_voters: 1,
        approve_voters: 1,
        reduce_voters: 1,
        nay_voters: 1,
        amplify_weight: 5,
        approve_weight: 5,
        reduce_weight: 5,
        nay_weight: 2,
        positive_weight: 15,
        turnout_weight: 17,
        leading_positive_option: Some(crate::ProposalPrimaryTrackOption::Reduce),
        leading_positive_weight: 5,
      })
    );
  });
}

#[test]
fn invoice_family_resolution_uses_lowest_scalar_tie_break_for_positive_winner() {
  new_test_ext().execute_with(|| {
    set_vote_weight(10, 5);
    set_vote_weight(11, 5);
    set_vote_weight(12, 5);
    set_vote_weight(13, 2);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      43,
      103,
      DEFAULT_PROPOSER,
      ProposalCadenceMode::Ordinary,
      ProposalPayloadKind::L2TreasurySpend,
      Default::default(),
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      43,
      103,
      ProposalVoteKind::Amplify,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      43,
      103,
      ProposalVoteKind::Approve,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(12),
      43,
      103,
      ProposalVoteKind::Reduce,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(13),
      43,
      103,
      ProposalVoteKind::Nay,
    ));
    System::set_block_number(3);
    assert_eq!(
      Governance::proposal_resolution_state(43, 103),
      Some(crate::ProposalResolutionState::PassingReduce)
    );
    assert_ok!(Governance::resolve_proposal_from_votes(
      RuntimeOrigin::root(),
      43,
      103,
    ));
    assert_eq!(
      Governance::finalized_proposal_outcome(43, 103),
      Some(FinalizedProposalOutcome::Resolved {
        epoch: 3,
        winner_count: 1,
      })
    );
    assert_eq!(
      Governance::retained_proposal_winning_primary_option(43, 103),
      Some(crate::ProposalPrimaryTrackOption::Reduce)
    );
    assert_eq!(
      Governance::govxp_counters(43, 10).total_winning_participations,
      0
    );
    assert_eq!(
      Governance::govxp_counters(43, 11).total_winning_participations,
      0
    );
    assert_eq!(
      Governance::govxp_counters(43, 12).total_winning_participations,
      1
    );
  });
}

#[test]
fn invoice_family_resolution_rejects_when_positive_ratio_misses_threshold() {
  new_test_ext().execute_with(|| {
    set_vote_weight(10, 3);
    set_vote_weight(11, 2);
    set_vote_weight(12, 4);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      43,
      104,
      DEFAULT_PROPOSER,
      ProposalCadenceMode::Ordinary,
      ProposalPayloadKind::L2TreasurySpend,
      Default::default(),
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      43,
      104,
      ProposalVoteKind::Amplify,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      43,
      104,
      ProposalVoteKind::Approve,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(12),
      43,
      104,
      ProposalVoteKind::Nay,
    ));
    System::set_block_number(3);
    assert_eq!(
      Governance::proposal_resolution_state(43, 104),
      Some(crate::ProposalResolutionState::Rejected {
        reason: ProposalRejectionReason::ApprovalThresholdNotMet,
      })
    );
    assert_ok!(Governance::resolve_proposal_from_votes(
      RuntimeOrigin::root(),
      43,
      104,
    ));
    assert_eq!(
      Governance::finalized_proposal_outcome(43, 104),
      Some(FinalizedProposalOutcome::Rejected {
        epoch: 3,
        reason: ProposalRejectionReason::ApprovalThresholdNotMet,
      })
    );
  });
}

#[test]
fn submission_authority_opening_fee_and_preimage_note_cost_are_explicit_and_queryable() {
  new_test_ext().execute_with(|| {
    assert_eq!(Governance::authorized_runtime_upgrade(), None);
    set_authorized_runtime_upgrade(Some(crate::AuthorizedRuntimeUpgrade {
      code_hash: polkadot_sdk::sp_core::H256::repeat_byte(31),
      check_version: true,
    }));
    assert_eq!(
      Governance::authorized_runtime_upgrade(),
      Some(crate::AuthorizedRuntimeUpgrade {
        code_hash: polkadot_sdk::sp_core::H256::repeat_byte(31),
        check_version: true,
      })
    );
    assert_eq!(
      Governance::proposal_submission_authority(44, ProposalPayloadKind::Intent),
      crate::ProposalSubmissionAuthority::Signed
    );
    assert_eq!(
      Governance::proposal_opening_fee(44, ProposalPayloadKind::Intent),
      Some(10)
    );
    assert_eq!(Governance::payload_preimage_note_cost(0), Some(2));
    assert_eq!(Governance::payload_preimage_note_cost(7), Some(9));
    assert_eq!(
      Governance::proposal_submission_authority(43, ProposalPayloadKind::L2SignalToL1),
      crate::ProposalSubmissionAuthority::Signed
    );
    assert_eq!(
      Governance::proposal_opening_fee(43, ProposalPayloadKind::L2SignalToL1),
      Some(10)
    );
    assert_eq!(
      Governance::proposal_submission_authority(7, ProposalPayloadKind::L2ParameterChange),
      crate::ProposalSubmissionAuthority::AdminOnly
    );
    assert_eq!(
      Governance::proposal_opening_fee(7, ProposalPayloadKind::L2ParameterChange),
      None
    );
  });
}

#[test]
fn signed_submission_burns_opening_fee_and_records_proposer() {
  new_test_ext().execute_with(|| {
    let proposer = 10u64;
    let balance_before = Balances::free_balance(proposer);
    assert_ok!(submit_signed_intent_proposal(44, 105, proposer));
    assert_eq!(Governance::proposal_author(44, 105), Some(proposer));
    assert_eq!(
      Balances::free_balance(proposer),
      balance_before.saturating_sub(10)
    );
    assert!(System::events().iter().any(|record| {
      record.event
        == RuntimeEvent::Governance(Event::ProposalOpeningFeeBurned {
          domain: 44,
          item_id: 105,
          proposer,
          amount: 10,
        })
    }));
  });
}

#[test]
fn signed_submission_rejects_admin_only_payload_kind() {
  new_test_ext().execute_with(|| {
    assert_noop!(
      Governance::submit_signed_proposal(
        RuntimeOrigin::signed(10),
        7,
        106,
        ProposalCadenceMode::Ordinary,
        ProposalPayloadKind::L2ParameterChange,
        Default::default(),
      ),
      Error::<Test>::ProposalSubmissionNotAllowedForSignedOrigin
    );
  });
}

#[test]
fn signed_submission_rejects_when_opening_fee_balance_is_insufficient() {
  new_test_ext().execute_with(|| {
    assert_noop!(
      submit_signed_intent_proposal(44, 107, 500),
      Error::<Test>::InsufficientProposalOpeningFeeBalance
    );
    assert!(!ActiveProposals::<Test>::contains_key(44, 107));
  });
}

#[test]
fn signed_submission_rolls_back_opening_fee_when_proposal_creation_fails() {
  new_test_ext().execute_with(|| {
    let proposer = 10u64;
    let balance_before = Balances::free_balance(proposer);
    assert_ok!(submit_signed_intent_proposal(44, 108, proposer));
    let balance_after_success = Balances::free_balance(proposer);
    assert_noop!(
      submit_signed_intent_proposal(44, 108, proposer),
      Error::<Test>::ProposalAlreadyActive
    );
    assert_eq!(Balances::free_balance(proposer), balance_after_success);
    assert_eq!(balance_before.saturating_sub(balance_after_success), 10);
  });
}

#[test]
fn proposal_urgent_eligibility_reflects_configured_policy_surface() {
  new_test_ext().execute_with(|| {
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      42,
      100,
      DEFAULT_PROPOSER,
      ProposalCadenceMode::Ordinary,
      ProposalPayloadKind::L1RootAction,
      Default::default(),
    ));
    assert_ok!(submit_test_proposal(7, 101, DEFAULT_PROPOSER));
    assert_eq!(Governance::proposal_urgent_eligibility(42, 100), Some(true));
    assert_eq!(Governance::proposal_urgent_eligibility(7, 101), Some(false));
    assert_eq!(Governance::proposal_urgent_eligibility(7, 999), None);
  });
}

#[test]
fn expeditable_pass_threshold_authorizes_urgent_fast_track_exactly_once() {
  new_test_ext().execute_with(|| {
    ProposalLeadInPeriod::set(2);
    set_veto_total_issuance(100);
    set_veto_vote_weight(10, 60);
    set_veto_vote_weight(11, 70);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      42,
      100,
      DEFAULT_PROPOSER,
      ProposalCadenceMode::Ordinary,
      ProposalPayloadKind::L1RootAction,
      Default::default(),
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      42,
      100,
      ProposalVoteKind::Pass,
    ));
    assert_eq!(ProposalUrgentAuthorizedAt::<Test>::get(42, 100), Some(1));
    assert!(System::events().iter().any(|record| {
      record.event
        == RuntimeEvent::Governance(Event::ProposalUrgentAuthorized {
          domain: 42,
          item_id: 100,
          authorization_epoch: 1,
          pass_weight: 60,
          total_protection_supply: 100,
        })
    }));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      42,
      100,
      ProposalVoteKind::Pass,
    ));
    assert_eq!(ProposalUrgentAuthorizedAt::<Test>::get(42, 100), Some(1));
    assert_eq!(
      System::events()
        .iter()
        .filter(|record| matches!(
          record.event,
          RuntimeEvent::Governance(Event::ProposalUrgentAuthorized {
            domain: 42,
            item_id: 100,
            ..
          })
        ))
        .count(),
      1
    );
  });
}

#[test]
fn unanimous_pass_executes_root_action_immediately() {
  new_test_ext().execute_with(|| {
    let payload_hash = H256::repeat_byte(42);
    set_payload_preimage_state(payload_hash, true, false);
    set_payload_executor_enabled(true);
    set_veto_total_issuance(100);
    set_veto_vote_weight(10, 100);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      42,
      150,
      DEFAULT_PROPOSER,
      ProposalCadenceMode::Ordinary,
      ProposalPayloadKind::L1RootAction,
      payload_hash,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      42,
      150,
      ProposalVoteKind::Pass,
    ));
    assert_eq!(ProposalUrgentAuthorizedAt::<Test>::get(42, 150), None);
    assert_eq!(Governance::active_proposal(42, 150), None);
    assert_eq!(
      Governance::finalized_proposal_outcome(42, 150),
      Some(FinalizedProposalOutcome::Enacted {
        approved_epoch: 1,
        executed_epoch: 1,
        winner_count: 0,
      })
    );
    assert!(System::events().iter().any(|record| {
      record.event
        == RuntimeEvent::Governance(Event::ProposalUrgentAuthorized {
          domain: 42,
          item_id: 150,
          authorization_epoch: 1,
          pass_weight: 100,
          total_protection_supply: 100,
        })
    }));
    assert!(System::events().iter().any(|record| {
      record.event
        == RuntimeEvent::Governance(Event::ProposalExecuted {
          domain: 42,
          item_id: 150,
          approved_epoch: 1,
          executed_epoch: 1,
          authority: ProposalExecutionAuthority::Root,
          payload_kind: ProposalPayloadKind::L1RootAction,
        })
    }));
  });
}

#[test]
fn urgent_authorization_opens_primary_early_and_shortens_resolution_window() {
  new_test_ext().execute_with(|| {
    ProposalLeadInPeriod::set(2);
    set_veto_total_issuance(100);
    set_veto_vote_weight(10, 60);
    assert_ok!(Governance::submit_proposal(
      RuntimeOrigin::root(),
      42,
      101,
      DEFAULT_PROPOSER,
      ProposalCadenceMode::Ordinary,
      ProposalPayloadKind::L1RootAction,
      Default::default(),
    ));
    assert_noop!(
      Governance::cast_vote(RuntimeOrigin::signed(11), 42, 101, ProposalVoteKind::Aye),
      Error::<Test>::ProposalPrimaryTrackNotOpen
    );
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      42,
      101,
      ProposalVoteKind::Pass,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      42,
      101,
      ProposalVoteKind::Aye,
    ));
    System::set_block_number(2);
    assert!(matches!(
      Governance::proposal_resolution_state(42, 101),
      Some(crate::ProposalResolutionState::Rejected { .. })
        | Some(crate::ProposalResolutionState::PassingAye)
    ));
  });
}

#[test]
fn ordinary_votes_are_rejected_during_lead_in_while_protection_votes_still_work() {
  new_test_ext().execute_with(|| {
    ProposalLeadInPeriod::set(2);
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_noop!(
      Governance::cast_vote(RuntimeOrigin::signed(10), 7, 100, ProposalVoteKind::Aye),
      Error::<Test>::ProposalPrimaryTrackNotOpen
    );
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Pass,
    ));
    System::set_block_number(3);
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
  });
}

#[test]
fn proposal_status_reports_pending_enactment_until_delay_expires() {
  new_test_ext().execute_with(|| {
    ProposalEnactmentDelay::set(2);
    let winners = BoundedVec::try_from(vec![10u64]).expect("single winner must fit");
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::resolve_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      winners,
    ));
    assert_eq!(
      Governance::proposal_status(7, 100),
      Some(crate::ProposalStatus::PendingEnactment {
        outcome: FinalizedProposalOutcome::Resolved {
          epoch: 1,
          winner_count: 1,
        },
        enactment_epoch: 3,
      })
    );
    System::set_block_number(3);
    assert_eq!(
      Governance::proposal_status(7, 100),
      Some(crate::ProposalStatus::Finalized(
        FinalizedProposalOutcome::Resolved {
          epoch: 1,
          winner_count: 1,
        }
      ))
    );
  });
}

#[test]
fn urgent_authorized_resolution_bypasses_final_protection_gate_and_enactment_delay() {
  new_test_ext().execute_with(|| {
    ProposalLeadInPeriod::set(2);
    ProposalEnactmentDelay::set(5);
    set_veto_total_issuance(100);
    set_veto_vote_weight(12, 40);
    set_vote_weight(11, 5);
    assert_ok!(submit_test_proposal(7, 102, DEFAULT_PROPOSER));
    ProposalUrgentAuthorizedAt::<Test>::insert(7, 102, 1);
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      102,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(12),
      7,
      102,
      ProposalVoteKind::Veto,
    ));
    System::set_block_number(2);
    assert_ok!(Governance::resolve_proposal_from_votes(
      RuntimeOrigin::root(),
      7,
      102,
    ));
    assert_eq!(ProposalPendingEnactmentAt::<Test>::get(7, 102), None);
    assert_eq!(
      Governance::finalized_proposal_outcome(7, 102),
      Some(FinalizedProposalOutcome::Resolved {
        epoch: 2,
        winner_count: 1,
      })
    );
  });
}

#[test]
fn vote_weight_provider_changes_winner_selection() {
  new_test_ext().execute_with(|| {
    set_vote_weight(10, 5);
    set_vote_weight(11, 1);
    set_vote_weight(12, 1);
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      100,
      ProposalVoteKind::Nay,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(12),
      7,
      100,
      ProposalVoteKind::Nay,
    ));
    System::set_block_number(3);
    assert_ok!(Governance::resolve_proposal_from_votes(
      RuntimeOrigin::root(),
      7,
      100,
    ));
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_rational(1u128, 6u128)
    );
    assert_eq!(
      Governance::reward_coefficient(7, 11),
      FixedU128::from_inner(0)
    );
    assert_eq!(
      Governance::reward_coefficient(7, 12),
      FixedU128::from_inner(0)
    );
  });
}

#[test]
fn proposal_vote_weight_provider_receives_time_aware_context() {
  new_test_ext().execute_with(|| {
    set_vote_weight(10, 5);
    set_vote_weight(11, 1);
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      100,
      ProposalVoteKind::Nay,
    ));
    let tally = Governance::proposal_vote_tally(7, 100).expect("active proposal must expose tally");
    assert_eq!(tally.aye_weight, 5);
    assert_eq!(tally.nay_weight, 1);
    let contexts = take_vote_weight_contexts();
    assert_eq!(contexts.len(), 2);
    for (domain, item_id, current_epoch, submitted_epoch, maturity_epoch, vote_epoch, account) in
      contexts
    {
      assert_eq!(domain, 7);
      assert_eq!(item_id, 100);
      assert_eq!(current_epoch, 1);
      assert_eq!(submitted_epoch, 1);
      assert_eq!(maturity_epoch, 3);
      assert_eq!(vote_epoch, 1);
      assert!(matches!(account, 10 | 11));
    }
    System::set_block_number(3);
    let _ = Governance::proposal_vote_tally(7, 100).expect("active proposal must expose tally");
    let contexts = take_vote_weight_contexts();
    assert_eq!(contexts.len(), 2);
    for (_, _, current_epoch, submitted_epoch, maturity_epoch, vote_epoch, account) in contexts {
      assert_eq!(current_epoch, 3);
      assert_eq!(submitted_epoch, 1);
      assert_eq!(maturity_epoch, 3);
      assert_eq!(vote_epoch, 1);
      assert!(matches!(account, 10 | 11));
    }
  });
}

#[test]
fn force_resolve_from_votes_bypasses_voting_window() {
  new_test_ext().execute_with(|| {
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(12),
      7,
      100,
      ProposalVoteKind::Nay,
    ));
    assert_ok!(Governance::force_resolve_proposal_from_votes(
      RuntimeOrigin::root(),
      7,
      100,
    ));
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_rational(1u128, 6u128)
    );
    assert_eq!(
      Governance::reward_coefficient(7, 11),
      FixedU128::from_rational(1u128, 6u128)
    );
    assert_eq!(
      Governance::reward_coefficient(7, 12),
      FixedU128::from_inner(0)
    );
  });
}

#[test]
fn vote_cast_and_resolution_from_votes_credit_the_winning_side() {
  new_test_ext().execute_with(|| {
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(12),
      7,
      100,
      ProposalVoteKind::Nay,
    ));
    let votes = ProposalVotesByItem::<Test>::get(7, 100).expect("proposal votes must exist");
    assert_eq!(votes.ayes.len(), 2);
    assert_eq!(votes.nays.len(), 1);
    assert_noop!(
      Governance::resolve_proposal_from_votes(RuntimeOrigin::root(), 7, 100),
      Error::<Test>::ProposalVotingWindowStillOpen
    );
    System::set_block_number(3);
    assert_ok!(Governance::resolve_proposal_from_votes(
      RuntimeOrigin::root(),
      7,
      100,
    ));
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_rational(1u128, 6u128)
    );
    assert_eq!(
      Governance::reward_coefficient(7, 11),
      FixedU128::from_rational(1u128, 6u128)
    );
    assert_eq!(
      Governance::reward_coefficient(7, 12),
      FixedU128::from_inner(0)
    );
  });
}

#[test]
fn vote_cast_rejects_duplicates_and_tied_resolution_rejects_without_rewards() {
  new_test_ext().execute_with(|| {
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_noop!(
      Governance::cast_vote(RuntimeOrigin::signed(10), 7, 100, ProposalVoteKind::Nay),
      Error::<Test>::ProposalVoteAlreadyCast
    );
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      100,
      ProposalVoteKind::Nay,
    ));
    System::set_block_number(3);
    assert_ok!(Governance::resolve_proposal_from_votes(
      RuntimeOrigin::root(),
      7,
      100,
    ));
    assert!(!ActiveProposals::<Test>::contains_key(7, 100));
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_inner(0)
    );
    assert_eq!(
      Governance::reward_coefficient(7, 11),
      FixedU128::from_inner(0)
    );
    System::assert_last_event(RuntimeEvent::Governance(Event::ProposalRejected {
      domain: 7,
      item_id: 100,
      epoch: 3,
      reason: ProposalRejectionReason::VoteTie,
      active_count: 0,
    }));
  });
}

#[test]
fn vote_resolution_rejects_when_turnout_is_below_runtime_minimum() {
  new_test_ext().execute_with(|| {
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    System::set_block_number(3);
    assert_ok!(Governance::resolve_proposal_from_votes(
      RuntimeOrigin::root(),
      7,
      100,
    ));
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_inner(0)
    );
    assert!(!ActiveProposals::<Test>::contains_key(7, 100));
    System::assert_last_event(RuntimeEvent::Governance(Event::ProposalRejected {
      domain: 7,
      item_id: 100,
      epoch: 3,
      reason: ProposalRejectionReason::TurnoutBelowMinimum,
      active_count: 0,
    }));
  });
}

#[test]
fn vote_resolution_rejects_when_no_side_meets_approval_threshold() {
  new_test_ext().execute_with(|| {
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    for account in [10u64, 11u64, 12u64, 13u64] {
      assert_ok!(Governance::cast_vote(
        RuntimeOrigin::signed(account),
        7,
        100,
        ProposalVoteKind::Aye,
      ));
    }
    for account in [14u64, 15u64, 16u64] {
      assert_ok!(Governance::cast_vote(
        RuntimeOrigin::signed(account),
        7,
        100,
        ProposalVoteKind::Nay,
      ));
    }
    System::set_block_number(3);
    assert_ok!(Governance::resolve_proposal_from_votes(
      RuntimeOrigin::root(),
      7,
      100,
    ));
    for account in [10u64, 11u64, 12u64, 13u64, 14u64, 15u64, 16u64] {
      assert_eq!(
        Governance::reward_coefficient(7, account),
        FixedU128::from_inner(0)
      );
    }
    System::assert_last_event(RuntimeEvent::Governance(Event::ProposalRejected {
      domain: 7,
      item_id: 100,
      epoch: 3,
      reason: ProposalRejectionReason::ApprovalThresholdNotMet,
      active_count: 0,
    }));
  });
}

#[test]
fn winning_vote_recording_increases_reward_coefficient_and_enforces_epoch_cap() {
  new_test_ext().execute_with(|| {
    assert_ok!(Governance::record_winning_vote(
      RuntimeOrigin::root(),
      7,
      100,
      10,
    ));
    assert_ok!(Governance::record_winning_vote(
      RuntimeOrigin::root(),
      7,
      101,
      10,
    ));
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_rational(2u128, 6u128)
    );
    assert_eq!(
      Governance::govxp_counters(7, 10),
      crate::GovXpCounters {
        rolling_winning_participation: 2,
        total_participations: 2,
        total_winning_participations: 2,
        total_authored_proposals: 0,
        total_successful_authored_proposals: 0,
      }
    );
    assert_noop!(
      Governance::record_winning_vote(RuntimeOrigin::root(), 7, 102, 10),
      Error::<Test>::EpochVoteCapReached
    );
  });
}

#[test]
fn same_item_cannot_be_recorded_twice_within_live_window() {
  new_test_ext().execute_with(|| {
    assert_ok!(Governance::record_winning_vote(
      RuntimeOrigin::root(),
      7,
      100,
      10,
    ));
    assert_noop!(
      Governance::record_winning_vote(RuntimeOrigin::root(), 7, 100, 10),
      Error::<Test>::DuplicateWinningVoteResolutionItem
    );
    System::set_block_number(2);
    Governance::on_initialize(2);
    assert_noop!(
      Governance::record_winning_vote(RuntimeOrigin::root(), 7, 100, 10),
      Error::<Test>::DuplicateWinningVoteResolutionItem
    );
  });
}

#[test]
fn same_item_cannot_be_re_ingested_for_different_accounts_within_live_window() {
  new_test_ext().execute_with(|| {
    let accounts =
      BoundedVec::try_from(vec![10u64, 11u64]).expect("batch accounts must fit configured bound");
    assert_ok!(Governance::record_winning_vote_batch(
      RuntimeOrigin::root(),
      7,
      100,
      accounts,
    ));
    assert_noop!(
      Governance::record_winning_vote(RuntimeOrigin::root(), 7, 100, 12),
      Error::<Test>::DuplicateWinningVoteResolutionItem
    );
  });
}

#[test]
fn batch_records_multiple_accounts_for_same_item() {
  new_test_ext().execute_with(|| {
    let accounts = polkadot_sdk::frame_support::BoundedVec::try_from(vec![10u64, 11u64])
      .expect("batch accounts must fit configured bound");
    assert_ok!(Governance::record_winning_vote_batch(
      RuntimeOrigin::root(),
      7,
      100,
      accounts,
    ));
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_rational(1u128, 6u128)
    );
    assert_eq!(
      Governance::reward_coefficient(7, 11),
      FixedU128::from_rational(1u128, 6u128)
    );
    assert_eq!(Governance::expiry_bucket(4).len(), 2);
  });
}

#[test]
fn batch_rejects_duplicate_accounts() {
  new_test_ext().execute_with(|| {
    let accounts =
      BoundedVec::try_from(vec![10u64, 10u64]).expect("batch accounts must fit configured bound");
    assert_noop!(
      Governance::record_winning_vote_batch(RuntimeOrigin::root(), 7, 100, accounts),
      Error::<Test>::DuplicateWinningVoteAccount
    );
  });
}

#[test]
fn record_winning_vote_rolls_back_when_expiry_bucket_is_full() {
  new_test_ext().execute_with(|| {
    let mut bucket = BoundedVec::default();
    for index in 0..1024u32 {
      assert!(
        bucket
          .try_push(ExpiringAccountTouch {
            domain: 7,
            account: u64::from(index).saturating_add(1_000),
          })
          .is_ok()
      );
    }
    ExpiryBuckets::<Test>::insert(4, bucket);
    assert_noop!(
      Governance::record_winning_vote(RuntimeOrigin::root(), 7, 100, 10),
      Error::<Test>::ExpiryBucketFull
    );
    assert!(Governance::winning_vote_window(7, 10).is_none());
  });
}

#[test]
fn batch_rolls_back_when_a_later_account_fails() {
  new_test_ext().execute_with(|| {
    assert_ok!(Governance::record_winning_vote(
      RuntimeOrigin::root(),
      7,
      100,
      10,
    ));
    assert_ok!(Governance::record_winning_vote(
      RuntimeOrigin::root(),
      7,
      101,
      10,
    ));
    let accounts =
      BoundedVec::try_from(vec![11u64, 10u64]).expect("batch accounts must fit configured bound");
    assert_noop!(
      Governance::record_winning_vote_batch(RuntimeOrigin::root(), 7, 102, accounts),
      Error::<Test>::EpochVoteCapReached
    );
    assert!(Governance::winning_vote_window(7, 11).is_none());
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_rational(2u128, 6u128)
    );
  });
}

#[test]
fn direct_ingest_helper_rolls_back_when_expiry_bucket_is_full() {
  new_test_ext().execute_with(|| {
    let mut bucket = BoundedVec::default();
    for index in 0..1024u32 {
      assert!(
        bucket
          .try_push(ExpiringAccountTouch {
            domain: 7,
            account: u64::from(index).saturating_add(2_000),
          })
          .is_ok()
      );
    }
    ExpiryBuckets::<Test>::insert(4, bucket);
    assert_noop!(
      Governance::ingest_winning_vote_resolution(7, 100, 10),
      Error::<Test>::ExpiryBucketFull
    );
    assert!(Governance::winning_vote_window(7, 10).is_none());
  });
}

#[test]
fn direct_batch_ingest_helper_rolls_back_when_a_later_account_fails() {
  new_test_ext().execute_with(|| {
    assert_ok!(Governance::ingest_winning_vote_resolution(7, 100, 10));
    assert_ok!(Governance::ingest_winning_vote_resolution(7, 101, 10));
    let accounts =
      BoundedVec::try_from(vec![11u64, 10u64]).expect("batch accounts must fit configured bound");
    assert_noop!(
      Governance::ingest_winning_vote_resolution_batch(7, 102, accounts),
      Error::<Test>::EpochVoteCapReached
    );
    assert!(Governance::winning_vote_window(7, 11).is_none());
  });
}

#[test]
fn expiry_bucket_deduplicates_same_epoch_account_touches() {
  new_test_ext().execute_with(|| {
    assert_ok!(Governance::record_winning_vote(
      RuntimeOrigin::root(),
      7,
      100,
      10,
    ));
    assert_ok!(Governance::record_winning_vote(
      RuntimeOrigin::root(),
      7,
      101,
      10,
    ));
    assert_eq!(Governance::expiry_bucket(4).len(), 1);
  });
}

#[test]
fn expired_zero_sum_window_is_evicted_from_storage() {
  new_test_ext().execute_with(|| {
    assert_ok!(Governance::record_winning_vote(
      RuntimeOrigin::root(),
      7,
      100,
      10,
    ));
    assert!(Governance::winning_vote_window(7, 10).is_some());
    System::set_block_number(4);
    Governance::on_initialize(4);
    assert!(Governance::winning_vote_window(7, 10).is_none());
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_inner(0)
    );
    System::assert_last_event(RuntimeEvent::Governance(Event::WinningVoteWindowEvicted {
      domain: 7,
      account: 10,
      epoch: 4,
    }));
  });
}

#[test]
fn newer_winning_vote_survives_after_older_one_expires() {
  new_test_ext().execute_with(|| {
    assert_ok!(Governance::record_winning_vote(
      RuntimeOrigin::root(),
      7,
      100,
      10,
    ));
    System::set_block_number(2);
    Governance::on_initialize(2);
    assert_ok!(Governance::record_winning_vote(
      RuntimeOrigin::root(),
      7,
      101,
      10,
    ));
    System::set_block_number(4);
    Governance::on_initialize(4);
    assert!(Governance::winning_vote_window(7, 10).is_some());
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_rational(1u128, 6u128)
    );
  });
}

#[test]
fn proposal_authorship_counters_track_created_and_successful_proposals() {
  new_test_ext().execute_with(|| {
    let proposer = 42u64;
    let winners =
      BoundedVec::try_from(vec![10u64]).expect("proposal winners must fit configured bound");
    assert_ok!(submit_test_proposal(7, 100, proposer,));
    assert_eq!(
      Governance::govxp_counters(7, proposer),
      crate::GovXpCounters {
        rolling_winning_participation: 0,
        total_participations: 0,
        total_winning_participations: 0,
        total_authored_proposals: 1,
        total_successful_authored_proposals: 0,
      }
    );
    assert_ok!(Governance::resolve_proposal(
      RuntimeOrigin::root(),
      7,
      100,
      winners,
    ));
    assert_eq!(
      Governance::govxp_counters(7, proposer),
      crate::GovXpCounters {
        rolling_winning_participation: 0,
        total_participations: 0,
        total_winning_participations: 0,
        total_authored_proposals: 1,
        total_successful_authored_proposals: 1,
      }
    );
    assert_ok!(submit_test_proposal(7, 101, proposer,));
    assert_ok!(Governance::reject_proposal(RuntimeOrigin::root(), 7, 101));
    assert_eq!(
      Governance::govxp_counters(7, proposer),
      crate::GovXpCounters {
        rolling_winning_participation: 0,
        total_participations: 0,
        total_winning_participations: 0,
        total_authored_proposals: 2,
        total_successful_authored_proposals: 1,
      }
    );
    set_veto_vote_weight(10, 60);
    set_veto_total_issuance(100);
    assert_ok!(submit_test_proposal(7, 102, proposer,));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      102,
      ProposalVoteKind::Veto,
    ));
    assert_eq!(
      Governance::govxp_counters(7, proposer),
      crate::GovXpCounters {
        rolling_winning_participation: 0,
        total_participations: 0,
        total_winning_participations: 0,
        total_authored_proposals: 3,
        total_successful_authored_proposals: 1,
      }
    );
  });
}

#[test]
fn ordinary_and_veto_track_votes_can_coexist_for_same_account() {
  new_test_ext().execute_with(|| {
    set_vote_weight(10, 5);
    set_veto_vote_weight(10, 40);
    set_veto_total_issuance(100);
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Veto,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Pass,
    ));
    let tally = Governance::proposal_vote_tally(7, 100).expect("active proposal must expose tally");
    assert_eq!(tally.aye_voters, 1);
    assert_eq!(tally.veto_voters, 0);
    assert_eq!(tally.pass_voters, 1);
    assert_eq!(tally.aye_weight, 5);
    assert_eq!(tally.veto_weight, 0);
    assert_eq!(tally.pass_weight, 40);
    assert_noop!(
      Governance::cast_vote(RuntimeOrigin::signed(10), 7, 100, ProposalVoteKind::Nay),
      Error::<Test>::ProposalVoteAlreadyCast
    );
    assert_noop!(
      Governance::cast_vote(RuntimeOrigin::signed(10), 7, 100, ProposalVoteKind::Pass),
      Error::<Test>::ProposalVoteAlreadyCast
    );
  });
}

#[test]
fn first_vote_counts_total_participation_once_per_proposal() {
  new_test_ext().execute_with(|| {
    set_veto_vote_weight(10, 40);
    set_veto_total_issuance(100);
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_eq!(
      Governance::govxp_counters(7, 10),
      crate::GovXpCounters {
        rolling_winning_participation: 0,
        total_participations: 1,
        total_winning_participations: 0,
        total_authored_proposals: 0,
        total_successful_authored_proposals: 0,
      }
    );
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Pass,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Veto,
    ));
    assert_eq!(
      Governance::govxp_counters(7, 10),
      crate::GovXpCounters {
        rolling_winning_participation: 0,
        total_participations: 1,
        total_winning_participations: 0,
        total_authored_proposals: 0,
        total_successful_authored_proposals: 0,
      }
    );
  });
}

#[test]
fn resolved_vote_counts_cumulative_total_and_winning_participation() {
  new_test_ext().execute_with(|| {
    set_vote_weight(10, 2);
    set_vote_weight(11, 1);
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      100,
      ProposalVoteKind::Nay,
    ));
    System::set_block_number(3);
    assert_ok!(Governance::resolve_proposal_from_votes(
      RuntimeOrigin::root(),
      7,
      100,
    ));
    assert_eq!(
      Governance::govxp_counters(7, 10),
      crate::GovXpCounters {
        rolling_winning_participation: 1,
        total_participations: 1,
        total_winning_participations: 1,
        total_authored_proposals: 0,
        total_successful_authored_proposals: 0,
      }
    );
    assert_eq!(
      Governance::govxp_counters(7, 11),
      crate::GovXpCounters {
        rolling_winning_participation: 0,
        total_participations: 1,
        total_winning_participations: 0,
        total_authored_proposals: 0,
        total_successful_authored_proposals: 0,
      }
    );
  });
}

#[test]
fn veto_cancellation_counts_veto_side_in_cumulative_winning_participation_only() {
  new_test_ext().execute_with(|| {
    set_veto_vote_weight(10, 60);
    set_veto_total_issuance(100);
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Veto,
    ));
    assert_eq!(
      Governance::govxp_counters(7, 10),
      crate::GovXpCounters {
        rolling_winning_participation: 0,
        total_participations: 1,
        total_winning_participations: 1,
        total_authored_proposals: 0,
        total_successful_authored_proposals: 0,
      }
    );
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_inner(0)
    );
  });
}

#[test]
fn pass_gate_winners_count_toward_cumulative_winning_participation() {
  new_test_ext().execute_with(|| {
    set_vote_weight(10, 2);
    set_veto_vote_weight(11, 30);
    set_veto_vote_weight(12, 20);
    set_veto_total_issuance(100);
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      100,
      ProposalVoteKind::Pass,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(12),
      7,
      100,
      ProposalVoteKind::Veto,
    ));
    System::set_block_number(3);
    assert_ok!(Governance::resolve_proposal_from_votes(
      RuntimeOrigin::root(),
      7,
      100,
    ));
    assert_eq!(
      Governance::govxp_counters(7, 10),
      crate::GovXpCounters {
        rolling_winning_participation: 1,
        total_participations: 1,
        total_winning_participations: 1,
        total_authored_proposals: 0,
        total_successful_authored_proposals: 0,
      }
    );
    assert_eq!(
      Governance::govxp_counters(7, 11),
      crate::GovXpCounters {
        rolling_winning_participation: 0,
        total_participations: 1,
        total_winning_participations: 1,
        total_authored_proposals: 0,
        total_successful_authored_proposals: 0,
      }
    );
    assert_eq!(
      Governance::govxp_counters(7, 12),
      crate::GovXpCounters {
        rolling_winning_participation: 0,
        total_participations: 1,
        total_winning_participations: 0,
        total_authored_proposals: 0,
        total_successful_authored_proposals: 0,
      }
    );
  });
}

#[test]
fn veto_votes_are_tracked_separately_in_tally() {
  new_test_ext().execute_with(|| {
    set_vote_weight(10, 5);
    set_vote_weight(11, 2);
    set_veto_vote_weight(12, 45);
    set_veto_vote_weight(13, 30);
    set_veto_total_issuance(100);
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      100,
      ProposalVoteKind::Nay,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(12),
      7,
      100,
      ProposalVoteKind::Veto,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(13),
      7,
      100,
      ProposalVoteKind::Pass,
    ));
    let tally = Governance::proposal_vote_tally(7, 100).expect("active proposal must expose tally");
    assert_eq!(tally.aye_voters, 1);
    assert_eq!(tally.nay_voters, 1);
    assert_eq!(tally.veto_voters, 1);
    assert_eq!(tally.pass_voters, 1);
    assert_eq!(tally.aye_weight, 5);
    assert_eq!(tally.nay_weight, 2);
    assert_eq!(tally.veto_weight, 45);
    assert_eq!(tally.pass_weight, 30);
    assert_eq!(tally.turnout_weight, 7);
    assert_eq!(tally.veto_turnout_weight, 75);
  });
}

#[test]
fn immediate_veto_cancels_proposal_without_reward_credit() {
  new_test_ext().execute_with(|| {
    set_veto_vote_weight(10, 51);
    set_veto_total_issuance(100);
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Veto,
    ));
    assert!(!ActiveProposals::<Test>::contains_key(7, 100));
    assert!(Governance::active_proposal_ids(7).is_empty());
    assert_eq!(
      Governance::finalized_proposal_outcome(7, 100),
      Some(FinalizedProposalOutcome::VetoCancelled {
        epoch: 1,
        veto_weight: 51,
      })
    );
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_inner(0)
    );
    System::assert_last_event(RuntimeEvent::Governance(Event::ProposalVetoCancelled {
      domain: 7,
      item_id: 100,
      epoch: 1,
      veto_weight: 51,
      pass_weight: 0,
      mode: crate::VetoCancellationMode::ImmediateThreshold,
      active_count: 0,
    }));
  });
}

#[test]
fn sub_percent_veto_does_not_activate_final_veto_gate() {
  new_test_ext().execute_with(|| {
    set_vote_weight(10, 5);
    set_vote_weight(11, 2);
    set_veto_vote_weight(12, 9);
    set_veto_total_issuance(1000);
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      100,
      ProposalVoteKind::Nay,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(12),
      7,
      100,
      ProposalVoteKind::Veto,
    ));
    System::set_block_number(3);
    Governance::on_initialize(3);
    assert_eq!(
      Governance::finalized_proposal_outcome(7, 100),
      Some(FinalizedProposalOutcome::Resolved {
        epoch: 3,
        winner_count: 1,
      })
    );
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_rational(1u128, 6u128)
    );
  });
}

#[test]
fn one_percent_veto_can_activate_final_veto_gate() {
  new_test_ext().execute_with(|| {
    set_vote_weight(10, 5);
    set_vote_weight(11, 2);
    set_veto_vote_weight(12, 10);
    set_veto_total_issuance(1000);
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      100,
      ProposalVoteKind::Nay,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(12),
      7,
      100,
      ProposalVoteKind::Veto,
    ));
    System::set_block_number(3);
    Governance::on_initialize(3);
    assert_eq!(
      Governance::finalized_proposal_outcome(7, 100),
      Some(FinalizedProposalOutcome::VetoCancelled {
        epoch: 3,
        veto_weight: 10,
      })
    );
  });
}

#[test]
fn protection_votes_after_protection_window_close_are_rejected() {
  new_test_ext().execute_with(|| {
    set_vote_weight(10, 5);
    set_vote_weight(11, 2);
    set_veto_vote_weight(12, 50);
    set_veto_total_issuance(100);
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      100,
      ProposalVoteKind::Nay,
    ));
    System::set_block_number(3);
    assert_noop!(
      Governance::cast_vote(RuntimeOrigin::signed(12), 7, 100, ProposalVoteKind::Veto),
      Error::<Test>::ProposalProtectionTrackClosed
    );
    assert_noop!(
      Governance::cast_vote(RuntimeOrigin::signed(12), 7, 100, ProposalVoteKind::Pass),
      Error::<Test>::ProposalProtectionTrackClosed
    );
    assert_ok!(Governance::resolve_proposal_from_votes(
      RuntimeOrigin::root(),
      7,
      100,
    ));
    assert_eq!(
      Governance::finalized_proposal_outcome(7, 100),
      Some(FinalizedProposalOutcome::Resolved {
        epoch: 3,
        winner_count: 1,
      })
    );
  });
}

#[test]
fn veto_track_blocks_at_maturity_without_immediate_threshold() {
  new_test_ext().execute_with(|| {
    set_vote_weight(10, 5);
    set_vote_weight(11, 2);
    set_veto_vote_weight(12, 50);
    set_veto_total_issuance(100);
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      100,
      ProposalVoteKind::Nay,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(12),
      7,
      100,
      ProposalVoteKind::Veto,
    ));
    assert_eq!(
      Governance::proposal_resolution_state(7, 100),
      Some(crate::ProposalResolutionState::VotingWindowOpen {
        current_epoch: 1,
        maturity_epoch: 3,
      })
    );
    System::set_block_number(3);
    Governance::on_initialize(3);
    assert_eq!(
      Governance::finalized_proposal_outcome(7, 100),
      Some(FinalizedProposalOutcome::VetoCancelled {
        epoch: 3,
        veto_weight: 50,
      })
    );
    System::assert_last_event(RuntimeEvent::Governance(Event::ProposalVetoCancelled {
      domain: 7,
      item_id: 100,
      epoch: 3,
      veto_weight: 50,
      pass_weight: 0,
      mode: crate::VetoCancellationMode::TrackOutcome,
      active_count: 0,
    }));
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_inner(0)
    );
  });
}

#[test]
fn veto_track_tie_blocks_proposal_at_maturity() {
  new_test_ext().execute_with(|| {
    set_vote_weight(10, 5);
    set_vote_weight(11, 2);
    set_veto_vote_weight(12, 20);
    set_veto_vote_weight(13, 20);
    set_veto_total_issuance(100);
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      100,
      ProposalVoteKind::Nay,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(12),
      7,
      100,
      ProposalVoteKind::Veto,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(13),
      7,
      100,
      ProposalVoteKind::Pass,
    ));
    System::set_block_number(3);
    Governance::on_initialize(3);
    assert_eq!(
      Governance::finalized_proposal_outcome(7, 100),
      Some(FinalizedProposalOutcome::VetoCancelled {
        epoch: 3,
        veto_weight: 20,
      })
    );
  });
}

#[test]
fn pass_outweighing_veto_allows_main_track_resolution() {
  new_test_ext().execute_with(|| {
    set_vote_weight(10, 5);
    set_vote_weight(11, 2);
    set_veto_vote_weight(12, 20);
    set_veto_vote_weight(13, 30);
    set_veto_total_issuance(100);
    assert_ok!(submit_test_proposal(7, 100, DEFAULT_PROPOSER));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(10),
      7,
      100,
      ProposalVoteKind::Aye,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(11),
      7,
      100,
      ProposalVoteKind::Nay,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(12),
      7,
      100,
      ProposalVoteKind::Veto,
    ));
    assert_ok!(Governance::cast_vote(
      RuntimeOrigin::signed(13),
      7,
      100,
      ProposalVoteKind::Pass,
    ));
    System::set_block_number(3);
    Governance::on_initialize(3);
    assert_eq!(
      Governance::finalized_proposal_outcome(7, 100),
      Some(FinalizedProposalOutcome::Resolved {
        epoch: 3,
        winner_count: 1,
      })
    );
    assert_eq!(
      Governance::reward_coefficient(7, 10),
      FixedU128::from_rational(1u128, 6u128)
    );
  });
}
