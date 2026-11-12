use super::*;
use crate::{ProposalPrimaryTrackOption, ProposalVotes, U256};

const ITEM: u32 = 100;
const VOTER: u64 = 10_000;

// Frozen value-domain fixtures: family-valid distinct accounts and legal individual
// primary power, not jointly backed public histories or physical resource prices.
fn prepare(
  invoice: bool,
) -> (
  u32,
  ProposalVotes<u64, u64, <Test as crate::Config>::MaxWinningVoteAccountsPerCall>,
) {
  let domain = if invoice { 43 } else { 7 };
  assert_ok!(Governance::submit_proposal(
    RuntimeOrigin::root(),
    domain,
    ITEM,
    DEFAULT_PROPOSER,
    ProposalCadenceMode::Ordinary,
    if invoice {
      ProposalPayloadKind::L2TreasurySpend
    } else {
      ProposalPayloadKind::L2ParameterChange
    },
    H256::default(),
  ));
  let maximum = <Test as crate::Config>::MaxWinningVoteAccountsPerCall::get();
  let power = U256::from(u128::MAX) * U256::from(7);
  set_vote_weight(VOTER, power);
  set_veto_total_issuance(100);
  let ballots = |direction: u64, primary: bool| {
    BoundedVec::try_from(
      (0..maximum)
        .map(|index| ProposalBallot {
          account: direction * u64::from(maximum) + u64::from(index) + 1,
          vote_epoch: 1,
          weight: if primary { power } else { U256::zero() },
          raw_power: if primary { power } else { U256::zero() },
        })
        .collect::<Vec<_>>(),
    )
    .unwrap()
  };
  let votes = ProposalVotes {
    ayes: if invoice {
      BoundedVec::default()
    } else {
      ballots(0, true)
    },
    nays: ballots(1, true),
    amplifies: if invoice {
      ballots(2, true)
    } else {
      BoundedVec::default()
    },
    approves: if invoice {
      ballots(3, true)
    } else {
      BoundedVec::default()
    },
    reduces: if invoice {
      ballots(4, true)
    } else {
      BoundedVec::default()
    },
    vetoes: ballots(5, false),
    passes: ballots(6, false),
  };
  let populated = if invoice { 6 } else { 4 };
  let ballot_width = votes.nays[0].encoded_size();
  assert_eq!(
    votes.encoded_size(),
    populated * (codec::Compact(maximum).encoded_size() + maximum as usize * ballot_width)
      + (7 - populated)
  );
  assert!(
    Governance::current_veto_cancellation(domain, ITEM, &votes, false)
      .unwrap()
      .is_none()
  );
  (domain, votes)
}

#[test]
fn maximum_family_initial_primary_dispatch_preserves_full_wide_value() {
  for invoice in [false, true] {
    new_test_ext().execute_with(|| {
      let (domain, mut votes) = prepare(invoice);
      let target = if invoice {
        &mut votes.reduces
      } else {
        &mut votes.ayes
      };
      target.pop().unwrap();
      let before = votes.encoded_size();
      ProposalVotesByItem::<Test>::insert(domain, ITEM, votes);
      let vote = if invoice {
        ProposalVoteKind::Reduce
      } else {
        ProposalVoteKind::Aye
      };
      assert_ok!(Governance::cast_vote(
        RuntimeOrigin::signed(VOTER),
        domain,
        ITEM,
        vote
      ));
      let after = ProposalVotesByItem::<Test>::get(domain, ITEM).unwrap();
      let target = if invoice { &after.reduces } else { &after.ayes };
      assert_eq!(target.len(), 256);
      let ballot = target.last().unwrap();
      assert_eq!(ballot.account, VOTER);
      assert_eq!(ballot.weight, U256::from(u128::MAX) * U256::from(7));
      assert_eq!(ballot.raw_power, ballot.weight);
      assert_eq!(after.encoded_size(), before + ballot.encoded_size());
      assert!(ActiveProposals::<Test>::contains_key(domain, ITEM));
      assert_eq!(
        ParticipationTotalsByAccount::<Test>::get(domain, VOTER).total_participations,
        1
      );
      assert_eq!(
        Governance::proposal_vote_tally(domain, ITEM)
          .unwrap()
          .turnout_weight,
        ballot.weight * U256::from(if invoice { 4 * 256 } else { 2 * 256 })
      );
    });
  }
}

#[test]
fn maximum_family_protection_replacement_and_cancellation_dispatch_are_distinct() {
  for invoice in [false, true] {
    for (replacement, cancel) in [(false, true), (true, true), (true, false)] {
      for vote in [ProposalVoteKind::Veto, ProposalVoteKind::Pass] {
        if cancel && vote == ProposalVoteKind::Pass {
          continue;
        }
        new_test_ext().execute_with(|| {
          let (domain, mut votes) = prepare(invoice);
          let maximum: u32 = <Test as crate::Config>::MaxWinningVoteAccountsPerCall::get();
          let target = if vote == ProposalVoteKind::Veto { &mut votes.vetoes } else { &mut votes.passes };
          target.pop().unwrap();
          if replacement {
            let source = if vote == ProposalVoteKind::Veto { &mut votes.passes } else { &mut votes.vetoes };
            source.last_mut().unwrap().account = VOTER;
          }
          let before = votes.encoded_size();
          ProposalVotesByItem::<Test>::insert(domain, ITEM, &votes);
          set_veto_vote_weight(VOTER, if cancel { 100 } else { 1 });
          assert_ok!(Governance::cast_vote(RuntimeOrigin::signed(VOTER), domain, ITEM, vote));
          assert_eq!(ParticipationTotalsByAccount::<Test>::get(domain, VOTER).total_participations, u64::from(!replacement));
          assert!(System::events().iter().any(|record| matches!(
            &record.event,
            RuntimeEvent::Governance(Event::ProposalVoteCast { account, replaced_vote, .. })
              if *account == VOTER && *replaced_vote == if replacement {
                Some(if vote == ProposalVoteKind::Veto { ProposalVoteKind::Pass } else { ProposalVoteKind::Veto })
              } else { None }
          )));
          if cancel {
            assert!(!ActiveProposals::<Test>::contains_key(domain, ITEM));
            assert!(!ProposalVotesByItem::<Test>::contains_key(domain, ITEM));
            assert!(matches!(FinalizedProposals::<Test>::get(domain, ITEM).unwrap().outcome,
              FinalizedProposalOutcome::VetoCancelled { finalized_epoch: 1, veto_weight } if veto_weight == U256::from(100)));
            assert_eq!(crate::ActiveProposalIdsByDomain::<Test>::get(domain).len(), 0);
            assert_eq!(ParticipationTotalsByAccount::<Test>::get(domain, VOTER).winning_participations, 1);
          } else {
            let after = ProposalVotesByItem::<Test>::get(domain, ITEM).unwrap();
            let (target, source) = if vote == ProposalVoteKind::Veto { (&after.vetoes, &after.passes) } else { (&after.passes, &after.vetoes) };
            assert_eq!(target.len(), maximum as usize);
            assert_eq!(source.len(), (maximum - 1) as usize);
            assert_eq!(target.last().unwrap().account, VOTER);
            assert_eq!(after.encoded_size(), before);
            assert!(ActiveProposals::<Test>::contains_key(domain, ITEM));
            assert!(!FinalizedProposals::<Test>::contains_key(domain, ITEM));
          }
        });
      }
    }
  }
}

#[test]
fn maximum_family_full_destination_replacement_refuses_without_losing_source_ballot() {
  for invoice in [false, true] {
    for vote in [ProposalVoteKind::Veto, ProposalVoteKind::Pass] {
      new_test_ext().execute_with(|| {
        let (domain, mut votes) = prepare(invoice);
        let source = if vote == ProposalVoteKind::Veto {
          &mut votes.passes
        } else {
          &mut votes.vetoes
        };
        source.last_mut().unwrap().account = VOTER;
        ProposalVotesByItem::<Test>::insert(domain, ITEM, &votes);
        set_veto_vote_weight(VOTER, 1);
        assert_noop!(
          Governance::cast_vote(RuntimeOrigin::signed(VOTER), domain, ITEM, vote),
          Error::<Test>::ProposalVoteSetFull
        );
        assert_eq!(
          ProposalVotesByItem::<Test>::get(domain, ITEM)
            .unwrap()
            .encode(),
          votes.encode()
        );
      });
    }
  }
}

#[test]
fn maximum_family_cancellation_retention_refusal_restores_complete_vote_state() {
  for invoice in [false, true] {
    new_test_ext().execute_with(|| {
      let (domain, mut votes) = prepare(invoice);
      votes.vetoes.pop().unwrap();
      votes.passes.last_mut().unwrap().account = VOTER;
      ProposalVotesByItem::<Test>::insert(domain, ITEM, &votes);
      set_veto_vote_weight(VOTER, 100);
      let retention: u32 = <Test as crate::Config>::FinalizedProposalOutcomeRetentionEpochs::get();
      let expiry = 1 + u64::from(retention);
      let capacity: u32 = <Test as crate::Config>::MaxFinalizedProposalOutcomesPerEpoch::get();
      crate::FinalizedProposalOutcomeExpiryBuckets::<Test>::insert(
        expiry,
        BoundedVec::try_from(
          (0..capacity)
            .map(|item_id| crate::FinalizedProposalTouch {
              domain,
              item_id: item_id + 1000,
            })
            .collect::<Vec<_>>(),
        )
        .unwrap(),
      );
      assert_noop!(
        Governance::cast_vote(
          RuntimeOrigin::signed(VOTER),
          domain,
          ITEM,
          ProposalVoteKind::Veto
        ),
        Error::<Test>::FinalizedProposalOutcomeExpiryBucketFull
      );
      assert_eq!(
        ProposalVotesByItem::<Test>::get(domain, ITEM)
          .unwrap()
          .encode(),
        votes.encode()
      );
      assert!(ActiveProposals::<Test>::contains_key(domain, ITEM));
      assert!(!FinalizedProposals::<Test>::contains_key(domain, ITEM));
    });
  }
}

#[test]
fn maximum_family_resolution_dispatch_preserves_invoice_tie_rule_and_binary_rejection() {
  for invoice in [false, true] {
    new_test_ext().execute_with(|| {
      let (domain, votes) = prepare(invoice);
      ProposalVotesByItem::<Test>::insert(domain, ITEM, votes);
      System::set_block_number(3);
      assert_ok!(Governance::resolve_proposal_from_votes(
        RuntimeOrigin::root(),
        domain,
        ITEM
      ));
      assert!(!ActiveProposals::<Test>::contains_key(domain, ITEM));
      assert!(!ProposalVotesByItem::<Test>::contains_key(domain, ITEM));
      let result = FinalizedProposals::<Test>::get(domain, ITEM).unwrap();
      if invoice {
        assert!(matches!(
          result.outcome,
          FinalizedProposalOutcome::Approved { .. }
        ));
        assert_eq!(
          crate::ProposalWinningPrimaryOptionByItem::<Test>::get(domain, ITEM),
          Some(ProposalPrimaryTrackOption::Reduce)
        );
        for account in 4 * 256 + 1..=5 * 256 {
          assert_eq!(
            ParticipationTotalsByAccount::<Test>::get(domain, account).winning_participations,
            1
          );
        }
      } else {
        assert!(matches!(
          result.outcome,
          FinalizedProposalOutcome::Rejected {
            reason: ProposalRejectionReason::VoteTie,
            ..
          }
        ));
      }
    });
  }
}
