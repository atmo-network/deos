use super::common::*;
use crate::{Assets, Governance, RuntimeEvent, RuntimeOrigin, Staking, System};
#[cfg(not(feature = "runtime-benchmarks"))]
use polkadot_sdk::frame_support::traits::fungibles::Mutate as FungiblesMutate;
use polkadot_sdk::frame_support::{
  assert_noop, assert_ok,
  traits::{
    Hooks,
    fungibles::{Inspect, metadata::Inspect as MetadataInspect},
  },
  weights::Weight,
};
use polkadot_sdk::sp_runtime::FixedU128;

fn advance_to_block(target: crate::BlockNumber) {
  while System::block_number() < target {
    let current = System::block_number();
    let _ = Staking::on_idle(current, Weight::MAX);
    Staking::on_finalize(current);
    System::reset_events();
    let next = current.saturating_add(1);
    System::set_block_number(next);
    let _ = Staking::on_initialize(next);
    let _ = Governance::on_initialize(next);
  }
}

fn governance_primary_open_epoch() -> crate::BlockNumber {
  crate::configs::governance_config::ProposalLeadInPeriod::get().saturating_add(1)
}

fn governance_primary_last_open_epoch() -> crate::BlockNumber {
  governance_primary_open_epoch()
    .saturating_add(crate::configs::governance_config::ProposalVotingPeriod::get())
    .saturating_sub(1)
}

fn governance_maturity_epoch() -> crate::BlockNumber {
  governance_primary_last_open_epoch().saturating_add(1)
}

fn governance_protection_last_open_epoch() -> crate::BlockNumber {
  crate::configs::governance_config::ProposalProtectionPeriod::get()
}

fn governance_protection_close_epoch() -> crate::BlockNumber {
  governance_protection_last_open_epoch().saturating_add(1)
}

fn jump_to_governance_epoch(target: crate::BlockNumber) {
  System::set_block_number(target);
}

fn service_governance_epoch(target: crate::BlockNumber) {
  pallet_governance::LastProcessedEpoch::<crate::Runtime>::put(target.saturating_sub(1));
  System::set_block_number(target);
  let _ = Governance::on_initialize(target);
}

fn record_winning_vote(domain: u32, item_id: u32, account: crate::AccountId) {
  assert_ok!(Governance::record_winning_vote(
    RuntimeOrigin::root(),
    domain,
    item_id,
    account,
  ));
}

fn record_winning_vote_batch(
  domain: u32,
  item_id: u32,
  accounts: alloc::vec::Vec<crate::AccountId>,
) {
  let accounts = polkadot_sdk::frame_support::BoundedVec::try_from(accounts)
    .expect("batch accounts must fit runtime bound");
  assert_ok!(Governance::record_winning_vote_batch(
    RuntimeOrigin::root(),
    domain,
    item_id,
    accounts,
  ));
}

fn submit_governance_proposal(domain: u32, item_id: u32) {
  assert_ok!(Governance::submit_proposal(
    RuntimeOrigin::root(),
    domain,
    item_id,
    ALICE,
    pallet_governance::ProposalCadenceMode::Ordinary,
    pallet_governance::ProposalPayloadKind::L2ParameterChange,
    Default::default(),
  ));
}

fn resolve_governance_proposal(
  domain: u32,
  item_id: u32,
  winners: alloc::vec::Vec<crate::AccountId>,
) {
  let winners = polkadot_sdk::frame_support::BoundedVec::try_from(winners)
    .expect("proposal winners must fit runtime bound");
  assert_ok!(Governance::resolve_proposal(
    RuntimeOrigin::root(),
    domain,
    item_id,
    winners,
  ));
}

fn reject_governance_proposal(domain: u32, item_id: u32) {
  assert_ok!(Governance::reject_proposal(
    RuntimeOrigin::root(),
    domain,
    item_id,
  ));
}

fn register_additional_staking_assets(start_index: u32, count: u32) {
  const TYPE_TEST: u32 = 0x2000_0000;
  for offset in 0..count {
    let asset_id = TYPE_TEST | (start_index + offset);
    assert_ok!(create_test_asset(asset_id, &ALICE));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      asset_id
    ));
  }
}

fn cast_governance_vote_kind(
  account: crate::AccountId,
  domain: u32,
  item_id: u32,
  vote: pallet_governance::ProposalVoteKind,
) {
  assert_ok!(Governance::cast_vote(
    RuntimeOrigin::signed(account),
    domain,
    item_id,
    vote,
  ));
}

fn cast_governance_vote(account: crate::AccountId, domain: u32, item_id: u32, aye: bool) {
  let vote = if aye {
    pallet_governance::ProposalVoteKind::Aye
  } else {
    pallet_governance::ProposalVoteKind::Nay
  };
  cast_governance_vote_kind(account, domain, item_id, vote);
}

fn prepare_weighted_governance_asset_stakes(
  asset_id: u32,
  stakes: &[(crate::AccountId, crate::Balance)],
) {
  assert_ok!(Staking::register_staking_asset(
    RuntimeOrigin::root(),
    asset_id,
  ));
  for (account, amount) in stakes {
    assert_ok!(Staking::stake(
      RuntimeOrigin::signed(account.clone()),
      asset_id,
      *amount,
    ));
  }
}

#[test]
fn reward_account_and_governance_domain_are_runtime_configured_per_asset_surface() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ne!(
      Staking::pool_account_for(ASSET_A),
      Staking::reward_account_for(ASSET_A)
    );
    assert_eq!(Staking::reward_governance_domain(ASSET_A), Some(ASSET_A));
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_inner(0))
    );
    record_winning_vote(ASSET_A, 100, BOB);
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
  });
}

#[test]
fn runtime_governance_zero_sum_eviction_clears_reward_coefficient_after_decay() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    record_winning_vote(ASSET_A, 100, BOB);
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
    System::set_block_number(4);
    Governance::on_initialize(4);
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_inner(0))
    );
  });
}

#[test]
fn runtime_governance_rejects_duplicate_item_within_live_window() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    record_winning_vote(ASSET_A, 100, BOB);
    assert_noop!(
      Governance::record_winning_vote(RuntimeOrigin::root(), ASSET_A, 100, BOB),
      pallet_governance::Error::<crate::Runtime>::DuplicateWinningVoteResolutionItem
    );
    System::set_block_number(2);
    Governance::on_initialize(2);
    assert_noop!(
      Governance::record_winning_vote(RuntimeOrigin::root(), ASSET_A, 100, BOB),
      pallet_governance::Error::<crate::Runtime>::DuplicateWinningVoteResolutionItem
    );
  });
}

#[test]
fn runtime_governance_rejects_re_ingesting_one_item_for_different_accounts() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    record_winning_vote_batch(ASSET_A, 100, alloc::vec![BOB, CHARLIE]);
    assert_noop!(
      Governance::record_winning_vote(RuntimeOrigin::root(), ASSET_A, 100, DAVE),
      pallet_governance::Error::<crate::Runtime>::DuplicateWinningVoteResolutionItem
    );
  });
}

#[test]
fn runtime_governance_proposal_resolution_feeds_reward_memory() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    submit_governance_proposal(ASSET_A, 100);
    assert_eq!(Governance::active_proposal_count(ASSET_A), 1);
    assert_eq!(
      Governance::active_proposal_ids(ASSET_A).into_inner(),
      alloc::vec![100]
    );
    resolve_governance_proposal(ASSET_A, 100, alloc::vec![BOB, CHARLIE]);
    assert_eq!(Governance::active_proposal_count(ASSET_A), 0);
    assert!(Governance::active_proposal_ids(ASSET_A).is_empty());
    assert_eq!(
      Governance::finalized_proposal_outcome(ASSET_A, 100),
      Some(pallet_governance::FinalizedProposalOutcome::Resolved {
        epoch: 1,
        winner_count: 2,
      })
    );
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &CHARLIE),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
  });
}

#[test]
fn runtime_governance_recent_finalized_proposals_are_queryable() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    submit_governance_proposal(ASSET_A, 100);
    resolve_governance_proposal(ASSET_A, 100, alloc::vec![BOB]);
    advance_to_block(2);
    submit_governance_proposal(ASSET_A, 101);
    reject_governance_proposal(ASSET_A, 101);
    advance_to_block(3);
    submit_governance_proposal(ASSET_B, 200);
    reject_governance_proposal(ASSET_B, 200);
    assert_eq!(
      Governance::recent_finalized_proposals(ASSET_A).into_inner(),
      alloc::vec![
        pallet_governance::RecentFinalizedProposal {
          item_id: 101,
          outcome: pallet_governance::FinalizedProposalOutcome::Rejected {
            epoch: 2,
            reason: pallet_governance::ProposalRejectionReason::AdminRejected,
          },
        },
        pallet_governance::RecentFinalizedProposal {
          item_id: 100,
          outcome: pallet_governance::FinalizedProposalOutcome::Resolved {
            epoch: 1,
            winner_count: 1,
          },
        },
      ]
    );
    assert_eq!(
      Governance::recent_finalized_proposals(ASSET_B).into_inner(),
      alloc::vec![pallet_governance::RecentFinalizedProposal {
        item_id: 200,
        outcome: pallet_governance::FinalizedProposalOutcome::Rejected {
          epoch: 3,
          reason: pallet_governance::ProposalRejectionReason::AdminRejected,
        },
      }]
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_governance_auto_finalizes_matured_vote_resolution() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    prepare_weighted_governance_asset_stakes(ASSET_A, &[(BOB, 500), (CHARLIE, 150), (DAVE, 150)]);
    submit_governance_proposal(ASSET_A, 103);
    jump_to_governance_epoch(governance_primary_open_epoch());
    cast_governance_vote(BOB, ASSET_A, 103, true);
    cast_governance_vote(CHARLIE, ASSET_A, 103, false);
    cast_governance_vote(DAVE, ASSET_A, 103, false);
    service_governance_epoch(governance_maturity_epoch());
    assert_eq!(Governance::active_proposal_count(ASSET_A), 0);
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &CHARLIE),
      Some(FixedU128::from_inner(0))
    );
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &DAVE),
      Some(FixedU128::from_inner(0))
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_governance_force_resolve_bypasses_voting_window() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    prepare_weighted_governance_asset_stakes(ASSET_A, &[(BOB, 500), (CHARLIE, 150), (DAVE, 150)]);
    submit_governance_proposal(ASSET_A, 101);
    jump_to_governance_epoch(governance_primary_open_epoch());
    cast_governance_vote(BOB, ASSET_A, 101, true);
    cast_governance_vote(CHARLIE, ASSET_A, 101, false);
    cast_governance_vote(DAVE, ASSET_A, 101, false);
    assert_ok!(Governance::force_resolve_proposal_from_votes(
      RuntimeOrigin::root(),
      ASSET_A,
      101,
    ));
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &CHARLIE),
      Some(FixedU128::from_inner(0))
    );
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &DAVE),
      Some(FixedU128::from_inner(0))
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_governance_immediate_veto_cancels_proposal_without_reward_credit() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use primitives::{AssetKind, ecosystem::protocol_tokens};

    let veto_asset_id = protocol_tokens::VETO_ASSET_ID;
    assert!(<Assets as Inspect<_>>::asset_exists(veto_asset_id));
    assert_eq!(
      <Assets as MetadataInspect<_>>::name(veto_asset_id),
      b"Veto Governance Token".to_vec()
    );
    assert_eq!(
      <Assets as MetadataInspect<_>>::symbol(veto_asset_id),
      b"VETO".to_vec()
    );
    assert_eq!(<Assets as MetadataInspect<_>>::decimals(veto_asset_id), 12);
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset_id,
      &BOB,
      60
    ));
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset_id,
      &CHARLIE,
      40
    ));
    submit_governance_proposal(ASSET_A, 150);
    cast_governance_vote_kind(BOB, ASSET_A, 150, pallet_governance::ProposalVoteKind::Veto);
    assert_eq!(
      <Assets as Inspect<_>>::total_issuance(veto_asset_id),
      100,
      "runtime veto weight should resolve against live VETO issuance"
    );
    assert_eq!(
      primitives::get_well_known_metadata(AssetKind::Local(veto_asset_id))
        .expect("well-known metadata must exist")
        .symbol,
      b"VETO".to_vec()
    );
    assert_eq!(
      Governance::finalized_proposal_outcome(ASSET_A, 150),
      Some(pallet_governance::FinalizedProposalOutcome::VetoCancelled {
        epoch: 1,
        veto_weight: 420,
      })
    );
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_inner(0))
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_governance_sub_percent_veto_does_not_block_main_track_resolution() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use primitives::ecosystem::protocol_tokens;

    let maturity_epoch = governance_maturity_epoch();
    prepare_weighted_governance_asset_stakes(ASSET_A, &[(BOB, 500), (CHARLIE, 150)]);
    let veto_asset_id = protocol_tokens::VETO_ASSET_ID;
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset_id,
      &BOB,
      9
    ));
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset_id,
      &CHARLIE,
      991
    ));
    submit_governance_proposal(ASSET_A, 153);
    jump_to_governance_epoch(governance_primary_open_epoch());
    cast_governance_vote(BOB, ASSET_A, 153, true);
    cast_governance_vote(CHARLIE, ASSET_A, 153, false);
    cast_governance_vote_kind(BOB, ASSET_A, 153, pallet_governance::ProposalVoteKind::Veto);
    service_governance_epoch(maturity_epoch);
    assert_eq!(
      Governance::finalized_proposal_outcome(ASSET_A, 153),
      Some(pallet_governance::FinalizedProposalOutcome::Resolved {
        epoch: maturity_epoch,
        winner_count: 1,
      })
    );
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_governance_rejects_protection_vote_after_protection_window_close() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use primitives::ecosystem::protocol_tokens;

    let maturity_epoch = governance_maturity_epoch();
    prepare_weighted_governance_asset_stakes(ASSET_A, &[(BOB, 500), (CHARLIE, 150)]);
    let veto_asset_id = protocol_tokens::VETO_ASSET_ID;
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset_id,
      &BOB,
      50
    ));
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset_id,
      &CHARLIE,
      50
    ));
    submit_governance_proposal(ASSET_A, 154);
    jump_to_governance_epoch(governance_primary_open_epoch());
    cast_governance_vote(BOB, ASSET_A, 154, true);
    cast_governance_vote(CHARLIE, ASSET_A, 154, false);
    jump_to_governance_epoch(governance_protection_close_epoch());
    assert_noop!(
      Governance::cast_vote(
        RuntimeOrigin::signed(BOB),
        ASSET_A,
        154,
        pallet_governance::ProposalVoteKind::Veto,
      ),
      pallet_governance::Error::<crate::Runtime>::ProposalProtectionTrackClosed
    );
    assert_eq!(
      Governance::proposal_vote_tally(ASSET_A, 154)
        .expect("proposal must stay active after rejected late veto")
        .veto_voters,
      0
    );
    jump_to_governance_epoch(maturity_epoch);
    assert_ok!(Governance::resolve_proposal_from_votes(
      RuntimeOrigin::root(),
      ASSET_A,
      154,
    ));
    assert_eq!(
      Governance::finalized_proposal_outcome(ASSET_A, 154),
      Some(pallet_governance::FinalizedProposalOutcome::Resolved {
        epoch: maturity_epoch,
        winner_count: 1,
      })
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_governance_pass_can_replace_prior_veto_vote() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use primitives::ecosystem::protocol_tokens;

    let maturity_epoch = governance_maturity_epoch();
    prepare_weighted_governance_asset_stakes(ASSET_A, &[(BOB, 500)]);
    let veto_asset_id = protocol_tokens::VETO_ASSET_ID;
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset_id,
      &BOB,
      40
    ));
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset_id,
      &CHARLIE,
      60
    ));
    submit_governance_proposal(ASSET_A, 151);
    cast_governance_vote_kind(BOB, ASSET_A, 151, pallet_governance::ProposalVoteKind::Veto);
    cast_governance_vote_kind(BOB, ASSET_A, 151, pallet_governance::ProposalVoteKind::Pass);
    jump_to_governance_epoch(governance_primary_open_epoch());
    cast_governance_vote(BOB, ASSET_A, 151, true);
    let tally = Governance::proposal_vote_tally(ASSET_A, 151).expect("proposal must stay active");
    assert_eq!(tally.veto_voters, 0);
    assert_eq!(tally.pass_voters, 1);
    assert_eq!(tally.veto_weight, 0);
    assert_eq!(tally.pass_weight, 280);
    service_governance_epoch(maturity_epoch);
    assert_eq!(
      Governance::finalized_proposal_outcome(ASSET_A, 151),
      Some(pallet_governance::FinalizedProposalOutcome::Resolved {
        epoch: maturity_epoch,
        winner_count: 1,
      })
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_governance_vote_power_profiles_match_launch_policy() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    let bldr_id = primitives::ecosystem::protocol_tokens::BLDR_ASSET_ID;
    submit_governance_proposal(ASSET_A, 159);
    submit_governance_proposal(bldr_id, 160);
    assert_eq!(
      Governance::proposal_vote_power_profile(
        ASSET_A,
        159,
        pallet_governance::ProposalVoteKind::Aye,
      ),
      Some(pallet_governance::ProposalVotePowerProfile::DecliningDirectStake)
    );
    assert_eq!(
      Governance::proposal_vote_power_profile(
        ASSET_A,
        159,
        pallet_governance::ProposalVoteKind::Veto,
      ),
      Some(pallet_governance::ProposalVotePowerProfile::DecliningVetoAsset)
    );
    assert_eq!(
      Governance::proposal_vote_power_profile(
        bldr_id,
        160,
        pallet_governance::ProposalVoteKind::Aye,
      ),
      Some(pallet_governance::ProposalVotePowerProfile::DecliningDirectStake)
    );
    assert_eq!(
      Governance::proposal_vote_power_profile(
        bldr_id,
        160,
        pallet_governance::ProposalVoteKind::Veto,
      ),
      Some(pallet_governance::ProposalVotePowerProfile::DecliningNativeStake)
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_governance_domain_policy_view_matches_launch_hierarchy() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    let bldr_id = primitives::ecosystem::protocol_tokens::BLDR_ASSET_ID;
    assert_eq!(
      Governance::governance_domain_policy(ASSET_A),
      pallet_governance::GovernanceDomainPolicy {
        ordinary_power_profile: pallet_governance::ProposalVotePowerProfile::DecliningDirectStake,
        protection_power_profile: pallet_governance::ProposalVotePowerProfile::DecliningVetoAsset,
      }
    );
    assert_eq!(
      Governance::governance_domain_policy(bldr_id),
      pallet_governance::GovernanceDomainPolicy {
        ordinary_power_profile: pallet_governance::ProposalVotePowerProfile::DecliningDirectStake,
        protection_power_profile: pallet_governance::ProposalVotePowerProfile::DecliningNativeStake,
      }
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_governance_bldr_primary_track_keeps_declining_same_domain_weight() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use primitives::ecosystem::protocol_tokens;

    let bldr_id = protocol_tokens::BLDR_ASSET_ID;
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      bldr_id, &BOB, 100
    ));
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      bldr_id, &CHARLIE, 100
    ));
    prepare_weighted_governance_asset_stakes(bldr_id, &[(BOB, 50), (CHARLIE, 50)]);
    submit_governance_proposal(bldr_id, 160);
    jump_to_governance_epoch(governance_primary_open_epoch());
    cast_governance_vote(BOB, bldr_id, 160, true);
    jump_to_governance_epoch(governance_primary_last_open_epoch());
    cast_governance_vote(CHARLIE, bldr_id, 160, true);
    let tally = Governance::proposal_vote_tally(bldr_id, 160).expect("proposal must stay active");
    assert_eq!(tally.aye_voters, 2);
    assert_eq!(tally.aye_weight, 400);
    assert_eq!(tally.nay_weight, 0);
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_governance_bldr_protection_track_uses_declining_native_stake() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;

    let bldr_id = primitives::ecosystem::protocol_tokens::BLDR_ASSET_ID;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(mint_tokens(0, &ALICE, &CHARLIE, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(BOB), 40, ALICE));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(CHARLIE),
      60,
      ALICE
    ));
    submit_governance_proposal(bldr_id, 161);
    cast_governance_vote_kind(BOB, bldr_id, 161, pallet_governance::ProposalVoteKind::Pass);
    let early_tally =
      Governance::proposal_vote_tally(bldr_id, 161).expect("proposal must stay active");
    assert_eq!(early_tally.pass_weight, 280);
    jump_to_governance_epoch(governance_protection_last_open_epoch());
    cast_governance_vote_kind(BOB, bldr_id, 161, pallet_governance::ProposalVoteKind::Veto);
    let late_tally =
      Governance::proposal_vote_tally(bldr_id, 161).expect("proposal must stay active");
    assert_eq!(late_tally.pass_voters, 0);
    assert_eq!(late_tally.veto_voters, 1);
    assert_eq!(late_tally.pass_weight, 0);
    assert_eq!(late_tally.veto_weight, 40);
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_governance_bldr_native_protection_track_can_cancel_immediately() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;

    let bldr_id = primitives::ecosystem::protocol_tokens::BLDR_ASSET_ID;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(mint_tokens(0, &ALICE, &CHARLIE, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(BOB), 60, ALICE));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(CHARLIE),
      40,
      ALICE
    ));
    submit_governance_proposal(bldr_id, 162);
    cast_governance_vote_kind(BOB, bldr_id, 162, pallet_governance::ProposalVoteKind::Veto);
    assert_eq!(
      Governance::finalized_proposal_outcome(bldr_id, 162),
      Some(pallet_governance::FinalizedProposalOutcome::VetoCancelled {
        epoch: 1,
        veto_weight: 420,
      })
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_governance_declining_power_rewards_early_ordinary_votes() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    prepare_weighted_governance_asset_stakes(ASSET_A, &[(BOB, 50), (CHARLIE, 50)]);
    submit_governance_proposal(ASSET_A, 160);
    jump_to_governance_epoch(governance_primary_open_epoch());
    cast_governance_vote(BOB, ASSET_A, 160, true);
    jump_to_governance_epoch(governance_primary_last_open_epoch());
    cast_governance_vote(CHARLIE, ASSET_A, 160, true);
    let tally = Governance::proposal_vote_tally(ASSET_A, 160).expect("proposal must stay active");
    assert_eq!(tally.aye_voters, 2);
    assert_eq!(tally.aye_weight, 400);
    assert_eq!(tally.nay_weight, 0);
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_governance_veto_track_switch_reprices_to_late_weight() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use primitives::ecosystem::protocol_tokens;

    let veto_asset_id = protocol_tokens::VETO_ASSET_ID;
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset_id,
      &BOB,
      40
    ));
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset_id,
      &CHARLIE,
      60
    ));
    submit_governance_proposal(ASSET_A, 161);
    cast_governance_vote_kind(BOB, ASSET_A, 161, pallet_governance::ProposalVoteKind::Pass);
    let early_tally =
      Governance::proposal_vote_tally(ASSET_A, 161).expect("proposal must stay active");
    assert_eq!(early_tally.pass_weight, 280);
    jump_to_governance_epoch(governance_protection_last_open_epoch());
    cast_governance_vote_kind(BOB, ASSET_A, 161, pallet_governance::ProposalVoteKind::Veto);
    let late_tally =
      Governance::proposal_vote_tally(ASSET_A, 161).expect("proposal must stay active");
    assert_eq!(late_tally.pass_voters, 0);
    assert_eq!(late_tally.veto_voters, 1);
    assert_eq!(late_tally.pass_weight, 0);
    assert_eq!(late_tally.veto_weight, 40);
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_governance_pass_can_unblock_main_track_resolution() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use primitives::ecosystem::protocol_tokens;

    let maturity_epoch = governance_maturity_epoch();
    prepare_weighted_governance_asset_stakes(ASSET_A, &[(BOB, 500), (CHARLIE, 150)]);
    let veto_asset_id = protocol_tokens::VETO_ASSET_ID;
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset_id,
      &BOB,
      20
    ));
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset_id,
      &CHARLIE,
      30
    ));
    assert_ok!(<Assets as FungiblesMutate<_>>::mint_into(
      veto_asset_id,
      &DAVE,
      50
    ));
    submit_governance_proposal(ASSET_A, 152);
    jump_to_governance_epoch(governance_primary_open_epoch());
    cast_governance_vote(BOB, ASSET_A, 152, true);
    cast_governance_vote(CHARLIE, ASSET_A, 152, false);
    cast_governance_vote_kind(BOB, ASSET_A, 152, pallet_governance::ProposalVoteKind::Veto);
    cast_governance_vote_kind(
      CHARLIE,
      ASSET_A,
      152,
      pallet_governance::ProposalVoteKind::Pass,
    );
    service_governance_epoch(maturity_epoch);
    assert_eq!(
      Governance::finalized_proposal_outcome(ASSET_A, 152),
      Some(pallet_governance::FinalizedProposalOutcome::Resolved {
        epoch: maturity_epoch,
        winner_count: 1,
      })
    );
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_governance_vote_resolution_feeds_reward_memory() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    prepare_weighted_governance_asset_stakes(ASSET_A, &[(BOB, 500), (CHARLIE, 150), (DAVE, 150)]);
    submit_governance_proposal(ASSET_A, 101);
    jump_to_governance_epoch(governance_primary_open_epoch());
    cast_governance_vote(BOB, ASSET_A, 101, true);
    cast_governance_vote(CHARLIE, ASSET_A, 101, false);
    cast_governance_vote(DAVE, ASSET_A, 101, false);
    assert_noop!(
      Governance::resolve_proposal_from_votes(RuntimeOrigin::root(), ASSET_A, 101),
      pallet_governance::Error::<crate::Runtime>::ProposalVotingWindowStillOpen
    );
    service_governance_epoch(governance_maturity_epoch());
    assert_eq!(Governance::active_proposal_count(ASSET_A), 0);
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &CHARLIE),
      Some(FixedU128::from_inner(0))
    );
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &DAVE),
      Some(FixedU128::from_inner(0))
    );
  });
}

#[test]
fn runtime_governance_vote_resolution_rejects_below_turnout_threshold() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    prepare_weighted_governance_asset_stakes(ASSET_A, &[(BOB, 10)]);
    submit_governance_proposal(ASSET_A, 102);
    jump_to_governance_epoch(governance_primary_open_epoch());
    cast_governance_vote(BOB, ASSET_A, 102, true);
    service_governance_epoch(governance_maturity_epoch());
    assert_eq!(Governance::active_proposal_count(ASSET_A), 0);
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_inner(0))
    );
    assert_eq!(Governance::active_proposal_count(ASSET_A), 0);
  });
}

#[test]
fn runtime_governance_batch_records_multiple_accounts_for_one_item() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    record_winning_vote_batch(ASSET_A, 100, alloc::vec![BOB, CHARLIE]);
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
    assert_eq!(
      Staking::reward_coefficient(ASSET_A, &CHARLIE),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
    assert_eq!(Governance::expiry_bucket(4).len(), 2);
  });
}

#[test]
fn prefunded_reward_account_can_record_reward_inflow_into_current_epoch() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 400));
    let reward_account = Staking::reward_account_for(ASSET_A);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      reward_account.clone().into(),
      75,
    ));
    advance_to_block(2);
    assert_eq!(Staking::reward_epoch_accrued(ASSET_A, 1), 75);
    assert_eq!(Staking::reward_liability_balance(ASSET_A), 75);
    assert_eq!(Staking::reward_epoch_total_weight(ASSET_A, 1), 0);
    assert_eq!(
      Staking::pool(ASSET_A)
        .expect("pool must exist")
        .accounted_balance,
      400
    );
    assert_eq!(Staking::stake_value(ASSET_A, &BOB), Some(400));
    let _ = reward_account;
  });
}

#[test]
fn direct_reward_ingress_weight_matches_aggregated_inflow_model() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 400));
    let reward_account = Staking::reward_account_for(ASSET_A);
    System::reset_events();
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      reward_account.clone().into(),
      30,
    ));
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      reward_account.clone().into(),
      45,
    ));
    let ingress_weight = <crate::configs::staking_config::RuntimeRewardSnapshotEventIngress as pallet_staking::RewardSnapshotEventIngress<crate::BlockNumber>>::ingest(
      System::block_number(),
      128,
      Weight::MAX,
    );
    assert_eq!(
      ingress_weight.ref_time(),
      crate::configs::staking_config::reward_ingress_expected_ref_time(3, 0, 1)
    );
    assert_eq!(Staking::reward_epoch_accrued(ASSET_A, 1), 75);
    assert_eq!(Staking::reward_liability_balance(ASSET_A), 75);
  });
}

#[test]
fn direct_reward_ingress_weight_matches_governance_touch_model() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 400));
    System::reset_events();
    record_winning_vote(ASSET_A, 100, BOB);
    let ingress_weight = <crate::configs::staking_config::RuntimeRewardSnapshotEventIngress as pallet_staking::RewardSnapshotEventIngress<crate::BlockNumber>>::ingest(
      System::block_number(),
      128,
      Weight::MAX,
    );
    assert_eq!(
      ingress_weight.ref_time(),
      crate::configs::staking_config::reward_ingress_expected_ref_time(1, 1, 0)
    );
    let touched_accounts =
      pallet_staking::RewardEpochTouchedAccounts::<crate::Runtime>::get(1, ASSET_A);
    assert_eq!(touched_accounts.len(), 1);
    assert!(touched_accounts.contains(&BOB));
  });
}

#[test]
fn reward_event_ingress_aggregates_same_block_reward_inflows_under_finite_budget() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 400));
    let reward_account = Staking::reward_account_for(ASSET_A);
    System::reset_events();
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      reward_account.clone().into(),
      30,
    ));
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      reward_account.clone().into(),
      45,
    ));
    let ingress_weight = Staking::on_idle(System::block_number(), Weight::from_parts(100_000, 0));
    assert!(ingress_weight.ref_time() > 0);
    Staking::on_finalize(System::block_number());
    assert_eq!(Staking::reward_epoch_accrued(ASSET_A, 1), 75);
    assert_eq!(Staking::reward_liability_balance(ASSET_A), 75);
    let reward_events = System::events()
      .into_iter()
      .filter(|record| {
        matches!(
          record.event,
          RuntimeEvent::Staking(pallet_staking::Event::RewardInflowRecorded {
            asset_id,
            epoch: 1,
            amount: 75,
            ..
          }) if asset_id == ASSET_A
        )
      })
      .count();
    assert_eq!(reward_events, 1);
  });
}

#[test]
fn reward_event_ingress_respects_remaining_weight_budget() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 400));
    let reward_account = Staking::reward_account_for(ASSET_A);
    System::reset_events();
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      reward_account.clone().into(),
      30,
    ));
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      reward_account.clone().into(),
      45,
    ));
    let ingress_weight = Staking::on_idle(System::block_number(), Weight::from_parts(7_000, 0));
    assert!(ingress_weight.ref_time() > 0);
    Staking::on_finalize(System::block_number());
    assert_eq!(Staking::reward_epoch_accrued(ASSET_A, 1), 0);
    assert_eq!(Staking::reward_liability_balance(ASSET_A), 0);
    assert_eq!(Staking::last_reward_ingress_truncated_epoch(), Some(1));
    assert!(System::events().into_iter().any(|record| {
      matches!(
        record.event,
        RuntimeEvent::Staking(pallet_staking::Event::RewardIngressTruncated { epoch: 1, .. })
      )
    }));
  });
}

#[test]
fn reward_event_ingress_stops_at_tiny_remaining_weight_budget() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 400));
    let reward_account = Staking::reward_account_for(ASSET_A);
    System::reset_events();
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      reward_account.into(),
      30,
    ));
    let ingress_weight = <crate::configs::staking_config::RuntimeRewardSnapshotEventIngress as pallet_staking::RewardSnapshotEventIngress<crate::BlockNumber>>::ingest(
      System::block_number(),
      crate::configs::staking_config::MaxRewardEventScanPerBlock::get() as usize,
      Weight::from_parts(1_000, 0),
    );
    assert_eq!(ingress_weight.ref_time(), 1_000);
    assert_eq!(Staking::reward_epoch_accrued(ASSET_A, 1), 0);
    assert_eq!(Staking::reward_liability_balance(ASSET_A), 0);
  });
}

#[test]
fn native_delegation_event_ingress_stops_at_tiny_remaining_weight_budget() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(BOB),
      400,
      ALICE
    ));
    let staked_asset_id = Staking::staked_asset_id(0).expect("staked asset id must resolve");
    System::reset_events();
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(BOB),
      staked_asset_id,
      CHARLIE.into(),
      50,
    ));
    let ingress = <crate::configs::staking_config::RuntimeNativeDelegationEventIngress as pallet_staking::NativeDelegationEventIngress<crate::AccountId>>::ingress(
      crate::configs::staking_config::MaxRewardEventScanPerBlock::get() as usize,
      Weight::from_parts(1_000, 0),
    );
    assert_eq!(ingress.weight.ref_time(), 1_000);
    assert!(ingress.truncated);
    assert!(ingress.touched_accounts.is_empty());
  });
}

#[test]
fn native_delegation_cache_repair_respects_remaining_weight_budget() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(BOB),
      400,
      ALICE
    ));
    let _ = Staking::on_idle(System::block_number(), Weight::MAX);
    assert_eq!(Staking::cached_delegated_native_backing(&ALICE), 400);
    System::reset_events();
    pallet_staking::NativeDelegationCacheDirty::<crate::Runtime>::put(true);
    pallet_staking::NativeDelegationRepairPhaseState::<crate::Runtime>::put(
      pallet_staking::NativeDelegationRepairPhase::ClearingCache,
    );
    pallet_staking::NativeDelegationRepairCursor::<crate::Runtime>::kill();
    let ingress_weight = Staking::on_idle(System::block_number(), Weight::from_parts(7_999, 0));
    assert_eq!(ingress_weight.ref_time(), 0);
    assert!(pallet_staking::NativeDelegationCacheDirty::<crate::Runtime>::get());
    assert_eq!(
      pallet_staking::NativeDelegationRepairPhaseState::<crate::Runtime>::get(),
      Some(pallet_staking::NativeDelegationRepairPhase::ClearingCache)
    );
  });
}

#[test]
fn reward_event_ingress_records_governance_touches_under_finite_budget() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 400));
    System::reset_events();
    record_winning_vote(ASSET_A, 100, BOB);
    record_winning_vote(ASSET_A, 101, CHARLIE);
    let ingress_weight = Staking::on_idle(System::block_number(), Weight::from_parts(7_000, 0));
    assert!(ingress_weight.ref_time() > 0);
    let touched_accounts =
      pallet_staking::RewardEpochTouchedAccounts::<crate::Runtime>::get(1, ASSET_A);
    assert_eq!(touched_accounts.len(), 1);
    assert!(touched_accounts.contains(&BOB));
    assert_eq!(Staking::last_reward_ingress_truncated_epoch(), Some(1));
    assert!(System::events().into_iter().any(|record| {
      matches!(
        record.event,
        RuntimeEvent::Staking(pallet_staking::Event::RewardIngressTruncated { epoch: 1, .. })
      )
    }));
  });
}

#[test]
fn reward_ingress_receipt_lookup_probe_stays_event_bound_with_many_pools() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    register_additional_staking_assets(10, 8);
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 400));
    let staked_asset_id = Staking::staked_asset_id(ASSET_A).expect("staked asset id must resolve");
    System::reset_events();
    crate::configs::staking_config::reset_reward_ingress_lookup_probes();
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(BOB),
      staked_asset_id,
      CHARLIE.into(),
      50,
    ));
    let ingress_weight = Staking::on_idle(System::block_number(), Weight::from_parts(100_000, 0));
    assert!(ingress_weight.ref_time() > 0);
    assert_eq!(
      crate::configs::staking_config::reward_ingress_receipt_base_lookup_probe_count(),
      2
    );
    let touched_accounts =
      pallet_staking::RewardEpochTouchedAccounts::<crate::Runtime>::get(1, ASSET_A);
    assert_eq!(touched_accounts.len(), 2);
    assert!(touched_accounts.contains(&BOB));
    assert!(touched_accounts.contains(&CHARLIE));
  });
}

#[test]
fn reward_ingress_governance_lookup_probe_stays_domain_bound_with_many_pools() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    register_additional_staking_assets(20, 8);
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    System::reset_events();
    crate::configs::staking_config::reset_reward_ingress_lookup_probes();
    record_winning_vote(ASSET_A, 100, BOB);
    let ingress_weight = Staking::on_idle(System::block_number(), Weight::from_parts(100_000, 0));
    assert!(ingress_weight.ref_time() > 0);
    assert_eq!(
      crate::configs::staking_config::reward_ingress_governance_domain_lookup_probe_count(),
      1
    );
    let touched_accounts =
      pallet_staking::RewardEpochTouchedAccounts::<crate::Runtime>::get(1, ASSET_A);
    assert_eq!(touched_accounts.len(), 1);
    assert!(touched_accounts.contains(&BOB));
  });
}

#[test]
fn reward_event_ingress_emits_truncation_signal_when_scan_cap_is_hit_under_finite_budget() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    let reward_account = Staking::reward_account_for(ASSET_A);
    System::reset_events();
    for _ in 0..129u32 {
      assert_ok!(Assets::transfer(
        RuntimeOrigin::signed(ALICE),
        ASSET_A,
        reward_account.clone().into(),
        1,
      ));
    }
    let ingress_weight = Staking::on_idle(System::block_number(), Weight::from_parts(1_000_000, 0));
    assert!(ingress_weight.ref_time() > 0);
    Staking::on_finalize(System::block_number());
    assert_eq!(Staking::reward_epoch_accrued(ASSET_A, 1), 127);
    assert_eq!(Staking::reward_liability_balance(ASSET_A), 127);
    assert_eq!(Staking::last_reward_ingress_truncated_epoch(), Some(1));
    System::assert_has_event(RuntimeEvent::Staking(
      pallet_staking::Event::RewardIngressTruncated {
        epoch: 1,
        scanned: 129,
        max_scan: 128,
      },
    ));
  });
}

#[test]
fn bootstrap_reward_snapshot_materializes_live_holder_weight_in_runtime() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 120));
    record_winning_vote(ASSET_A, 100, BOB);
    assert_eq!(Staking::reward_active_weight(ASSET_A, &BOB), None);
    assert_ok!(Staking::bootstrap_reward_snapshot(
      RuntimeOrigin::root(),
      ASSET_A,
      BOB,
    ));
    let snapshot =
      Staking::reward_active_weight_snapshot(ASSET_A, &BOB).expect("reward snapshot must exist");
    assert_eq!(snapshot.effective_from_epoch, 1);
    assert_eq!(snapshot.shares, 120);
    assert_eq!(
      snapshot.coefficient,
      FixedU128::from_rational(1u128, 12u128)
    );
    assert_eq!(snapshot.weight, 9);
    assert_eq!(Staking::reward_active_total_weight(ASSET_A), 9);
  });
}

#[test]
fn winning_holder_claims_reward_via_same_asset_auto_compound_in_runtime() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &CHARLIE, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 400));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(CHARLIE), ASSET_A, 400));
    record_winning_vote(ASSET_A, 100, BOB);
    assert_ok!(Staking::bootstrap_reward_snapshot(
      RuntimeOrigin::root(),
      ASSET_A,
      BOB,
    ));
    assert_ok!(Staking::bootstrap_reward_snapshot(
      RuntimeOrigin::root(),
      ASSET_A,
      CHARLIE,
    ));
    let reward_account = Staking::reward_account_for(ASSET_A);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      reward_account.clone().into(),
      80,
    ));
    advance_to_block(2);
    assert_noop!(
      Staking::claim_reward(RuntimeOrigin::signed(CHARLIE), ASSET_A, 1),
      pallet_staking::Error::<crate::Runtime>::NoRewardClaimable
    );
    assert_ok!(Staking::claim_reward(
      RuntimeOrigin::signed(BOB),
      ASSET_A,
      1
    ));
    let staked_asset_id = Staking::staked_asset_id(ASSET_A).expect("staked asset id must resolve");
    assert_eq!(Staking::reward_claimed((ASSET_A, 1), BOB), Some(80));
    assert_eq!(Staking::reward_liability_balance(ASSET_A), 0);
    assert_eq!(<Assets as Inspect<_>>::balance(ASSET_A, &reward_account), 0);
    assert_eq!(<Assets as Inspect<_>>::balance(staked_asset_id, &BOB), 480);
    assert_eq!(Staking::stake_value(ASSET_A, &BOB), Some(480));
    assert_eq!(Staking::stake_value(ASSET_A, &CHARLIE), Some(400));
  });
}

#[test]
fn winning_holder_can_batch_claim_multiple_closed_epochs_in_runtime() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    let epochs = polkadot_sdk::frame_support::BoundedVec::try_from(alloc::vec![1u32, 2u32])
      .expect("batch epochs must fit runtime bound");
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 400));
    record_winning_vote(ASSET_A, 100, BOB);
    assert_ok!(Staking::bootstrap_reward_snapshot(
      RuntimeOrigin::root(),
      ASSET_A,
      BOB,
    ));
    let reward_account = Staking::reward_account_for(ASSET_A);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      reward_account.clone().into(),
      80,
    ));
    assert!(Staking::note_reward_touch(ASSET_A, &BOB));
    advance_to_block(2);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      reward_account.clone().into(),
      20,
    ));
    advance_to_block(3);
    assert_ok!(Staking::claim_reward_batch(
      RuntimeOrigin::signed(BOB),
      ASSET_A,
      epochs,
    ));
    let staked_asset_id = Staking::staked_asset_id(ASSET_A).expect("staked asset id must resolve");
    assert_eq!(Staking::reward_claimed((ASSET_A, 1), BOB), Some(80));
    assert_eq!(Staking::reward_claimed((ASSET_A, 2), BOB), Some(20));
    assert_eq!(Staking::reward_liability_balance(ASSET_A), 0);
    assert_eq!(<Assets as Inspect<_>>::balance(ASSET_A, &reward_account), 0);
    assert_eq!(<Assets as Inspect<_>>::balance(staked_asset_id, &BOB), 500);
    assert_eq!(Staking::stake_value(ASSET_A, &BOB), Some(500));
  });
}

#[test]
fn truncated_reward_epoch_blocks_runtime_claims() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 400));
    record_winning_vote(ASSET_A, 100, BOB);
    assert_ok!(Staking::bootstrap_reward_snapshot(
      RuntimeOrigin::root(),
      ASSET_A,
      BOB,
    ));
    let reward_account = Staking::reward_account_for(ASSET_A);
    System::reset_events();
    for _ in 0..129u32 {
      assert_ok!(Assets::transfer(
        RuntimeOrigin::signed(ALICE),
        ASSET_A,
        reward_account.clone().into(),
        1,
      ));
    }
    advance_to_block(2);
    assert_noop!(
      Staking::claim_reward(RuntimeOrigin::signed(BOB), ASSET_A, 1),
      pallet_staking::Error::<crate::Runtime>::RewardEpochIncomplete
    );
  });
}

#[test]
fn reward_snapshot_transfer_changes_weight_only_next_epoch_in_runtime() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 120));
    record_winning_vote(ASSET_A, 100, BOB);
    record_winning_vote(ASSET_A, 101, CHARLIE);
    advance_to_block(2);
    let staked_asset_id = Staking::staked_asset_id(ASSET_A).expect("staked asset id must resolve");
    assert_eq!(Staking::reward_active_weight(ASSET_A, &BOB), Some(9));
    assert_eq!(Staking::reward_active_weight(ASSET_A, &CHARLIE), None);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(BOB),
      staked_asset_id,
      CHARLIE.into(),
      50,
    ));
    let reward_account = Staking::reward_account_for(ASSET_A);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      reward_account.clone().into(),
      10,
    ));
    advance_to_block(3);
    assert_eq!(Staking::reward_epoch_accrued(ASSET_A, 2), 10);
    assert_eq!(Staking::reward_epoch_total_weight(ASSET_A, 2), 9);
    assert_eq!(Staking::reward_liability_balance(ASSET_A), 10);
    assert_eq!(Staking::reward_active_total_weight(ASSET_A), 9);
    let bob_snapshot = Staking::reward_active_weight_snapshot(ASSET_A, &BOB)
      .expect("bob reward snapshot must exist");
    assert_eq!(bob_snapshot.effective_from_epoch, 3);
    assert_eq!(bob_snapshot.shares, 70);
    assert_eq!(
      bob_snapshot.coefficient,
      FixedU128::from_rational(1u128, 12u128)
    );
    assert_eq!(bob_snapshot.weight, 5);
    let charlie_snapshot = Staking::reward_active_weight_snapshot(ASSET_A, &CHARLIE)
      .expect("charlie reward snapshot must exist");
    assert_eq!(charlie_snapshot.effective_from_epoch, 3);
    assert_eq!(charlie_snapshot.shares, 50);
    assert_eq!(
      charlie_snapshot.coefficient,
      FixedU128::from_rational(1u128, 12u128)
    );
    assert_eq!(charlie_snapshot.weight, 4);
  });
}

#[test]
fn governance_changes_affect_reward_snapshot_only_from_next_epoch() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 120));
    advance_to_block(2);
    let snapshot =
      Staking::reward_active_weight_snapshot(ASSET_A, &BOB).expect("reward snapshot must exist");
    assert_eq!(snapshot.effective_from_epoch, 2);
    assert_eq!(snapshot.shares, 120);
    assert_eq!(snapshot.coefficient, FixedU128::from_inner(0));
    assert_eq!(snapshot.weight, 0);
    record_winning_vote(ASSET_A, 100, BOB);
    assert_eq!(Staking::reward_active_weight(ASSET_A, &BOB), Some(0));
    let reward_account = Staking::reward_account_for(ASSET_A);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      ASSET_A,
      reward_account.clone().into(),
      10,
    ));
    advance_to_block(3);
    assert_eq!(Staking::reward_epoch_accrued(ASSET_A, 2), 10);
    assert_eq!(Staking::reward_epoch_total_weight(ASSET_A, 2), 0);
    assert_eq!(Staking::reward_liability_balance(ASSET_A), 10);
    let snapshot =
      Staking::reward_active_weight_snapshot(ASSET_A, &BOB).expect("reward snapshot must exist");
    assert_eq!(snapshot.effective_from_epoch, 3);
    assert_eq!(snapshot.shares, 120);
    assert_eq!(
      snapshot.coefficient,
      FixedU128::from_rational(1u128, 12u128)
    );
    assert_eq!(snapshot.weight, 9);
    assert_eq!(Staking::reward_active_total_weight(ASSET_A), 9);
  });
}

#[test]
fn registering_staking_pool_creates_staked_receipt_asset_with_metadata() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    const TYPE_STAKED: u32 = 0x5000_0000;
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    let staked_asset_id = Staking::staked_asset_id(ASSET_A).expect("staked asset id must resolve");
    assert_eq!(staked_asset_id, TYPE_STAKED | 1);
    assert!(<Assets as Inspect<_>>::asset_exists(staked_asset_id));
    assert_eq!(
      <Assets as MetadataInspect<_>>::name(staked_asset_id),
      format!("Staked Asset {}", ASSET_A).into_bytes()
    );
    assert_eq!(
      <Assets as MetadataInspect<_>>::symbol(staked_asset_id),
      format!("st{}", ASSET_A).into_bytes()
    );
    assert_eq!(<Assets as MetadataInspect<_>>::decimals(staked_asset_id), 0);
  });
}

#[test]
fn registering_native_staking_pool_creates_stntve_receipt_asset_with_metadata() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    const TYPE_STAKED: u32 = 0x5000_0000;
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert!(<Assets as Inspect<_>>::asset_exists(TYPE_STAKED));
    assert_eq!(
      <Assets as MetadataInspect<_>>::name(TYPE_STAKED),
      b"Staked Native Token".to_vec()
    );
    assert_eq!(
      <Assets as MetadataInspect<_>>::symbol(TYPE_STAKED),
      b"stNTVE".to_vec()
    );
    assert_eq!(<Assets as MetadataInspect<_>>::decimals(TYPE_STAKED), 12);
  });
}

#[test]
fn registering_foreign_staking_pool_creates_dedicated_foreign_receipt_asset() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    const TYPE_STAKED_FOREIGN: u32 = 0x6000_0000;
    assert_ok!(Assets::force_set_metadata(
      RuntimeOrigin::root(),
      ASSET_FOREIGN,
      b"Foreign Dollar".to_vec(),
      b"FUSD".to_vec(),
      12,
      false,
    ));
    assert_ok!(mint_tokens(ASSET_FOREIGN, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_FOREIGN,
    ));
    let staked_asset_id =
      Staking::staked_asset_id(ASSET_FOREIGN).expect("staked asset id must resolve");
    assert_eq!(staked_asset_id, TYPE_STAKED_FOREIGN | 1);
    assert!(<Assets as Inspect<_>>::asset_exists(staked_asset_id));
    assert_eq!(
      <Assets as MetadataInspect<_>>::name(staked_asset_id),
      b"Staked Foreign Dollar".to_vec()
    );
    assert_eq!(
      <Assets as MetadataInspect<_>>::symbol(staked_asset_id),
      b"stFUSD".to_vec()
    );
    assert_ok!(Staking::stake(
      RuntimeOrigin::signed(BOB),
      ASSET_FOREIGN,
      400,
    ));
    assert_eq!(<Assets as Inspect<_>>::balance(staked_asset_id, &BOB), 400);
    assert_eq!(Staking::stake_value(ASSET_FOREIGN, &BOB), Some(400));
  });
}

#[test]
fn governance_can_initialize_and_convert_legacy_runtime_pool_to_receipts() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &CHARLIE, 1_000));
    let pool_account = Staking::pool_account_for(ASSET_A);
    pallet_staking::Pools::<crate::Runtime>::insert(
      ASSET_A,
      pallet_staking::PoolState {
        total_shares: 400,
        accounted_balance: 400,
        active_staker_count: 1,
      },
    );
    pallet_staking::Positions::<crate::Runtime>::insert(
      ASSET_A,
      BOB,
      pallet_staking::StakePosition { shares: 400 },
    );
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(BOB),
      ASSET_A,
      pool_account.clone().into(),
      400,
    ));
    let staked_asset_id = Staking::staked_asset_id(ASSET_A).expect("staked asset id must resolve");
    assert!(!<Assets as Inspect<_>>::asset_exists(staked_asset_id));
    assert_ok!(Staking::initialize_staked_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert!(<Assets as Inspect<_>>::asset_exists(staked_asset_id));
    assert_ok!(Staking::convert_position_to_receipt(
      RuntimeOrigin::signed(BOB),
      ASSET_A,
    ));
    assert_eq!(Staking::position(ASSET_A, BOB), None);
    assert_eq!(<Assets as Inspect<_>>::balance(staked_asset_id, &BOB), 400);
    assert_ok!(Staking::stake(RuntimeOrigin::signed(CHARLIE), ASSET_A, 200));
    assert_eq!(Staking::position(ASSET_A, CHARLIE), None);
    assert_eq!(
      <Assets as Inspect<_>>::balance(staked_asset_id, &CHARLIE),
      200
    );
    assert_eq!(Staking::stake_value(ASSET_A, &BOB), Some(400));
    assert_eq!(Staking::stake_value(ASSET_A, &CHARLIE), Some(200));
    System::assert_has_event(RuntimeEvent::Staking(
      pallet_staking::Event::StakedAssetInitialized {
        asset_id: ASSET_A,
        staked_asset_id,
        pool_account,
      },
    ));
    System::assert_has_event(RuntimeEvent::Staking(
      pallet_staking::Event::LegacyPositionConverted {
        asset_id: ASSET_A,
        account: BOB,
        converted_shares: 400,
      },
    ));
  });
}

#[test]
fn staking_pool_registers_and_stakes_local_asset() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 400));
    let pool = Staking::pool(ASSET_A).expect("pool must exist");
    assert_eq!(pool.total_shares, 400);
    assert_eq!(pool.accounted_balance, 400);
    assert!(Staking::position(ASSET_A, BOB).is_none());
    assert_eq!(Staking::stake_value(ASSET_A, &BOB), Some(400));
    System::assert_has_event(RuntimeEvent::Staking(pallet_staking::Event::Staked {
      asset_id: ASSET_A,
      account: BOB,
      amount_in: 400,
      minted_shares: 400,
    }));
  });
}

#[test]
fn transferred_staking_receipt_holder_can_exit_in_runtime() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 400));
    let staked_asset_id = Staking::staked_asset_id(ASSET_A).expect("staked asset id must resolve");
    let before = <Assets as Inspect<_>>::balance(ASSET_A, &CHARLIE);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(BOB),
      staked_asset_id,
      CHARLIE.into(),
      150,
    ));
    assert_ok!(Staking::unstake(
      RuntimeOrigin::signed(CHARLIE),
      ASSET_A,
      150
    ));
    assert_eq!(
      <Assets as Inspect<_>>::balance(staked_asset_id, &CHARLIE),
      0
    );
    assert_eq!(
      <Assets as Inspect<_>>::balance(ASSET_A, &CHARLIE) - before,
      150
    );
  });
}

#[test]
fn stxxx_pair_pool_can_be_created_and_funded_without_special_protocol_role() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 400));
    let staked_asset_id = Staking::staked_asset_id(ASSET_A).expect("staked asset id must resolve");
    let staked_asset = crate::configs::AssetKind::Local(staked_asset_id);
    let base_asset = crate::configs::AssetKind::Local(ASSET_A);
    let before_base = <Assets as Inspect<_>>::balance(ASSET_A, &BOB);
    let before_staked = <Assets as Inspect<_>>::balance(staked_asset_id, &BOB);
    assert_ok!(create_pool(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset
    ));
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset,
      399,
      399,
      1,
      1,
      &BOB,
    ));
    assert_eq!(
      <Assets as Inspect<_>>::balance(ASSET_A, &BOB),
      before_base - 399
    );
    assert_eq!(
      <Assets as Inspect<_>>::balance(staked_asset_id, &BOB),
      before_staked - 399
    );
  });
}

#[test]
fn external_inflow_sync_increases_runtime_stake_value() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &CHARLIE, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 500));
    let pool_account = Staking::pool_account_for(ASSET_A);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(CHARLIE),
      ASSET_A,
      pool_account.clone().into(),
      500,
    ));
    assert_ok!(Staking::sync_pool(RuntimeOrigin::signed(DAVE), ASSET_A));
    let pool = Staking::pool(ASSET_A).expect("pool must exist");
    assert_eq!(pool.total_shares, 500);
    assert_eq!(pool.accounted_balance, 1_000);
    assert_eq!(Staking::stake_value(ASSET_A, &BOB), Some(1_000));
    assert_eq!(
      <Assets as Inspect<_>>::balance(ASSET_A, &pool_account),
      1_000
    );
    assert_eq!(Staking::reward_liability_balance(ASSET_A), 0);
    assert_eq!(
      Staking::reward_epoch_accrued(ASSET_A, System::block_number()),
      0
    );
  });
}

#[test]
fn governance_can_recover_unowned_prefunded_runtime_pool() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &CHARLIE, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    let pool_account = Staking::pool_account_for(ASSET_A);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(CHARLIE),
      ASSET_A,
      pool_account.clone().into(),
      500,
    ));
    assert_noop!(
      Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 100),
      pallet_staking::Error::<crate::Runtime>::PoolHasUnownedBalance
    );
    let beneficiary_before = <Assets as Inspect<_>>::balance(ASSET_A, &DAVE);
    assert_ok!(Staking::recover_unowned_pool(
      RuntimeOrigin::root(),
      ASSET_A,
      DAVE
    ));
    assert_eq!(<Assets as Inspect<_>>::balance(ASSET_A, &pool_account), 0);
    assert_eq!(
      <Assets as Inspect<_>>::balance(ASSET_A, &DAVE) - beneficiary_before,
      500
    );
    assert_eq!(
      Staking::pool(ASSET_A)
        .expect("pool must exist")
        .accounted_balance,
      0
    );
    System::assert_has_event(RuntimeEvent::Staking(
      pallet_staking::Event::UnownedPoolRecovered {
        asset_id: ASSET_A,
        beneficiary: DAVE,
        amount: 500,
      },
    ));
  });
}

#[test]
fn native_binding_aggregates_runtime_backing() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(mint_tokens(0, &ALICE, &CHARLIE, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_noop!(
      Staking::bind_native(RuntimeOrigin::signed(BOB), DAVE),
      pallet_staking::Error::<crate::Runtime>::InvalidBindingTarget
    );
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(BOB),
      400,
      ALICE
    ));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(CHARLIE),
      300,
      ALICE
    ));
    assert_eq!(Staking::delegated_native_backing(&ALICE), 700);
    System::assert_has_event(RuntimeEvent::Staking(
      pallet_staking::Event::NativeBindingSet {
        account: CHARLIE,
        operator: ALICE,
      },
    ));
  });
}

#[test]
fn runtime_generic_native_stake_requires_operator() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_noop!(
      Staking::stake(RuntimeOrigin::signed(BOB), 0, 400),
      pallet_staking::Error::<crate::Runtime>::NativeStakeRequiresOperator
    );
  });
}

#[test]
fn runtime_native_backing_follows_live_stntve_transfer() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(BOB),
      400,
      ALICE
    ));
    let staked_asset_id = Staking::staked_asset_id(0).expect("staked asset id must resolve");
    assert_eq!(Staking::delegated_native_backing(&ALICE), 400);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(BOB),
      staked_asset_id,
      CHARLIE.into(),
      150,
    ));
    assert_eq!(Staking::delegated_native_backing(&ALICE), 250);
  });
}

#[test]
fn runtime_native_query_surface_follows_stntve_transfer() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(BOB),
      400,
      ALICE
    ));
    let staked_asset_id = Staking::staked_asset_id(0).expect("staked asset id must resolve");
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(BOB),
      staked_asset_id,
      CHARLIE.into(),
      150,
    ));
    assert_eq!(Staking::native_stake_value(&BOB), Some(250));
    assert_eq!(
      Staking::delegated_native_stake_value(&BOB),
      Some((ALICE, 250))
    );
    assert_eq!(Staking::native_stake_value(&CHARLIE), Some(150));
    assert_eq!(Staking::passive_native_stake_value(&CHARLIE), Some(150));
    assert_eq!(Staking::delegated_native_stake_value(&CHARLIE), None);
  });
}

#[test]
fn passive_stntve_holder_can_bind_native_backing() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(BOB),
      400,
      ALICE
    ));
    let staked_asset_id = Staking::staked_asset_id(0).expect("staked asset id must resolve");
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(BOB),
      staked_asset_id,
      CHARLIE.into(),
      150,
    ));
    assert_eq!(Staking::delegated_native_backing(&ALICE), 250);
    assert_eq!(Staking::delegated_native_stake_value(&CHARLIE), None);
    assert_ok!(Staking::bind_native(RuntimeOrigin::signed(CHARLIE), ALICE));
    assert_eq!(Staking::native_binding(CHARLIE), Some(ALICE));
    assert_eq!(
      Staking::delegated_native_stake_value(&CHARLIE),
      Some((ALICE, 150))
    );
    assert_eq!(Staking::delegated_native_backing(&ALICE), 400);
    System::assert_has_event(RuntimeEvent::Staking(
      pallet_staking::Event::NativeBindingSet {
        account: CHARLIE,
        operator: ALICE,
      },
    ));
  });
}

#[test]
fn rebinding_native_stake_moves_backing_between_operators() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE, CHARLIE]).expect("invulnerables must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(BOB),
      400,
      ALICE
    ));
    assert_eq!(Staking::delegated_native_backing(&ALICE), 400);
    assert_eq!(Staking::delegated_native_backing(&CHARLIE), 0);
    assert_ok!(Staking::bind_native(RuntimeOrigin::signed(BOB), CHARLIE));
    assert_eq!(Staking::native_binding(BOB), Some(CHARLIE));
    assert_eq!(Staking::delegated_native_backing(&ALICE), 0);
    assert_eq!(Staking::delegated_native_backing(&CHARLIE), 400);
    System::assert_has_event(RuntimeEvent::Staking(
      pallet_staking::Event::NativeBindingSet {
        account: BOB,
        operator: CHARLIE,
      },
    ));
  });
}

#[test]
fn rebinding_updates_runtime_ranking_cache_without_waiting_for_on_idle() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    use polkadot_sdk::pallet_collator_selection::CandidateInfo;

    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE, CHARLIE]).expect("invulnerables must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(BOB),
      400,
      ALICE
    ));
    let _ = Staking::on_idle(System::block_number(), Weight::MAX);
    assert_eq!(Staking::cached_delegated_native_backing(&ALICE), 400);
    assert_eq!(Staking::cached_delegated_native_backing(&CHARLIE), 0);
    assert_ok!(Staking::bind_native(RuntimeOrigin::signed(BOB), CHARLIE));
    assert_eq!(Staking::cached_delegated_native_backing(&ALICE), 0);
    assert_eq!(Staking::cached_delegated_native_backing(&CHARLIE), 400);
    let ranked =
      crate::configs::DelegationWeightedCollatorSessionManager::rank_candidates(alloc::vec![
        CandidateInfo {
          who: ALICE,
          deposit: 10,
        },
        CandidateInfo {
          who: CHARLIE,
          deposit: 10,
        },
      ]);
    let ranked_accounts = ranked
      .into_iter()
      .map(|candidate| candidate.who)
      .collect::<alloc::vec::Vec<_>>();
    assert_eq!(ranked_accounts, alloc::vec![CHARLIE, ALICE]);
  });
}

#[test]
fn clear_native_binding_makes_stntve_passive_again() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(BOB),
      400,
      ALICE
    ));
    assert_eq!(Staking::delegated_native_backing(&ALICE), 400);
    assert_ok!(Staking::clear_native_binding(RuntimeOrigin::signed(BOB)));
    assert_eq!(Staking::native_binding(BOB), None);
    assert_eq!(Staking::delegated_native_backing(&ALICE), 0);
    assert_eq!(Staking::passive_native_stake_value(&BOB), Some(400));
    assert_eq!(Staking::delegated_native_stake_value(&BOB), None);
    System::assert_has_event(RuntimeEvent::Staking(
      pallet_staking::Event::NativeBindingCleared { account: BOB },
    ));
  });
}

#[test]
fn runtime_native_stake_helpers_distinguish_passive_and_delegated_positions() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(mint_tokens(0, &ALICE, &CHARLIE, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(BOB),
      400,
      ALICE
    ));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(CHARLIE),
      300,
      ALICE
    ));
    let staked_asset_id = Staking::staked_asset_id(0).expect("staked asset id must resolve");
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(CHARLIE),
      staked_asset_id,
      DAVE.into(),
      120,
    ));
    assert_eq!(Staking::native_stake_value(&BOB), Some(400));
    assert_eq!(Staking::passive_native_stake_value(&BOB), None);
    assert_eq!(
      Staking::delegated_native_stake_value(&BOB),
      Some((ALICE, 400))
    );
    assert_eq!(
      Staking::stake_exposure(0, &BOB),
      Some(pallet_staking::StakeExposure {
        total_value: 400,
        passive_value: 0,
        delegated_value: 400,
        delegated_operator: Some(ALICE),
      })
    );
    assert_eq!(Staking::native_stake_value(&CHARLIE), Some(180));
    assert_eq!(
      Staking::delegated_native_stake_value(&CHARLIE),
      Some((ALICE, 180))
    );
    assert_eq!(Staking::native_stake_value(&DAVE), Some(120));
    assert_eq!(Staking::passive_native_stake_value(&DAVE), Some(120));
    assert_eq!(Staking::delegated_native_stake_value(&DAVE), None);
  });
}

#[test]
fn runtime_stake_value_follows_receipt_transfer_for_local_asset() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 400));
    let staked_asset_id = Staking::staked_asset_id(ASSET_A).expect("staked asset id must resolve");
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(BOB),
      staked_asset_id,
      CHARLIE.into(),
      150,
    ));
    assert_eq!(Staking::stake_value(ASSET_A, &BOB), Some(250));
    assert_eq!(Staking::stake_value(ASSET_A, &CHARLIE), Some(150));
  });
}

#[test]
fn runtime_non_native_stake_exposure_stays_passive() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(mint_tokens(ASSET_A, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(BOB),
      400,
      ALICE
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), ASSET_A, 250));
    assert_eq!(Staking::passive_stake_value(ASSET_A, &BOB), Some(250));
    assert_eq!(Staking::delegated_stake_value(ASSET_A, &BOB), None);
    assert_eq!(
      Staking::stake_exposure(ASSET_A, &BOB),
      Some(pallet_staking::StakeExposure {
        total_value: 250,
        passive_value: 250,
        delegated_value: 0,
        delegated_operator: None,
      })
    );
  });
}

#[test]
fn non_native_staking_is_ignored_by_native_security_queries() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      ASSET_A
    ));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(BOB),
      200,
      ALICE
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(CHARLIE), ASSET_A, 900));
    assert_ok!(Staking::bind_native(RuntimeOrigin::signed(CHARLIE), ALICE));
    assert_eq!(Staking::delegated_native_backing(&ALICE), 200);
    assert_eq!(Staking::native_stake_value(&CHARLIE), None);
    assert_eq!(Staking::passive_native_stake_value(&CHARLIE), None);
    assert_eq!(Staking::delegated_native_stake_value(&CHARLIE), None);
  });
}

#[test]
fn operator_commission_is_bounded_in_runtime() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::sp_runtime::Perbill;
    assert_eq!(Staking::operator_commission(ALICE), Perbill::zero());
    assert_ok!(Staking::set_operator_commission(
      RuntimeOrigin::signed(ALICE),
      Perbill::from_percent(25),
    ));
    assert_eq!(
      Staking::operator_commission(ALICE),
      Perbill::from_percent(25)
    );
    assert_noop!(
      Staking::set_operator_commission(RuntimeOrigin::signed(ALICE), Perbill::from_percent(51),),
      pallet_staking::Error::<crate::Runtime>::CommissionExceedsMaximum
    );
  });
}

#[test]
fn native_binding_rejects_permissionless_candidates_while_trusted_phase_is_active() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    use polkadot_sdk::pallet_collator_selection::CandidateInfo;

    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    polkadot_sdk::pallet_collator_selection::CandidateList::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![CandidateInfo {
        who: CHARLIE,
        deposit: 10,
      }])
      .expect("single candidate must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_noop!(
      Staking::stake_native(RuntimeOrigin::signed(BOB), 400, CHARLIE),
      pallet_staking::Error::<crate::Runtime>::InvalidBindingTarget
    );
  });
}

#[test]
fn session_manager_ranking_cache_follows_transfer_after_on_idle() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    use polkadot_sdk::pallet_collator_selection::CandidateInfo;

    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE, CHARLIE]).expect("invulnerables must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(mint_tokens(0, &ALICE, &EVE, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(BOB),
      400,
      ALICE
    ));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(EVE),
      300,
      CHARLIE
    ));
    let _ = Staking::on_idle(System::block_number(), Weight::MAX);
    let initial_ranked =
      crate::configs::DelegationWeightedCollatorSessionManager::rank_candidates(alloc::vec![
        CandidateInfo {
          who: ALICE,
          deposit: 10,
        },
        CandidateInfo {
          who: CHARLIE,
          deposit: 10,
        },
      ]);
    let initial_accounts = initial_ranked
      .into_iter()
      .map(|candidate| candidate.who)
      .collect::<alloc::vec::Vec<_>>();
    assert_eq!(initial_accounts, alloc::vec![ALICE, CHARLIE]);
    let staked_asset_id = Staking::staked_asset_id(0).expect("staked asset id must resolve");
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(BOB),
      staked_asset_id,
      DAVE.into(),
      400,
    ));
    let _ = Staking::on_idle(System::block_number(), Weight::MAX);
    let refreshed_ranked =
      crate::configs::DelegationWeightedCollatorSessionManager::rank_candidates(alloc::vec![
        CandidateInfo {
          who: ALICE,
          deposit: 10,
        },
        CandidateInfo {
          who: CHARLIE,
          deposit: 10,
        },
      ]);
    let refreshed_accounts = refreshed_ranked
      .into_iter()
      .map(|candidate| candidate.who)
      .collect::<alloc::vec::Vec<_>>();
    assert_eq!(refreshed_accounts, alloc::vec![CHARLIE, ALICE]);
  });
}

#[test]
fn zero_native_exposure_retires_runtime_binding_after_on_idle() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(BOB),
      400,
      ALICE
    ));
    let _ = Staking::on_idle(System::block_number(), Weight::MAX);
    let staked_asset_id = Staking::staked_asset_id(0).expect("staked asset id must resolve");
    System::reset_events();
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(BOB),
      staked_asset_id,
      CHARLIE.into(),
      400,
    ));
    let _ = Staking::on_idle(System::block_number(), Weight::MAX);
    assert_eq!(Staking::native_binding(BOB), None);
    assert_eq!(Staking::delegated_native_stake_value(&BOB), None);
    assert_eq!(Staking::passive_native_stake_value(&BOB), None);
    System::assert_has_event(RuntimeEvent::Staking(
      pallet_staking::Event::NativeBindingCleared { account: BOB },
    ));
  });
}

#[test]
fn session_manager_ignores_candidates_while_permissionless_collators_are_disabled() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    use polkadot_sdk::pallet_collator_selection::CandidateInfo;
    use polkadot_sdk::pallet_session::SessionManager;

    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    polkadot_sdk::pallet_collator_selection::DesiredCandidates::<crate::Runtime>::put(1);
    polkadot_sdk::pallet_collator_selection::CandidateList::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![CandidateInfo {
        who: CHARLIE,
        deposit: 10,
      }])
      .expect("single candidate must fit"),
    );
    let collators = <crate::configs::DelegationWeightedCollatorSessionManager as SessionManager<
      crate::AccountId,
    >>::new_session(0)
    .expect("session manager must return a collator set");
    assert_eq!(collators, alloc::vec![ALICE]);
  });
}

#[test]
fn session_manager_ranks_larger_candidate_set_by_backing_deposit_and_account() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    use polkadot_sdk::pallet_collator_selection::CandidateInfo;

    let faythe = crate::AccountId::new([6u8; 32]);
    let grace = crate::AccountId::new([7u8; 32]);
    let heidi = crate::AccountId::new([8u8; 32]);
    let ivan = crate::AccountId::new([9u8; 32]);

    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE, BOB, CHARLIE, DAVE, EVE, faythe.clone()])
        .expect("invulnerables must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(mint_tokens(0, &ALICE, &EVE, 1_000));
    assert_ok!(mint_tokens(0, &ALICE, &grace, 1_000));
    assert_ok!(mint_tokens(0, &ALICE, &heidi, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(BOB),
      500,
      ALICE
    ));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(EVE), 500, DAVE));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(grace),
      300,
      BOB
    ));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(heidi),
      300,
      CHARLIE
    ));
    let _ = Staking::on_idle(System::block_number(), Weight::MAX);
    let ranked =
      crate::configs::DelegationWeightedCollatorSessionManager::rank_candidates(alloc::vec![
        CandidateInfo {
          who: faythe.clone(),
          deposit: 1,
        },
        CandidateInfo {
          who: EVE,
          deposit: 100,
        },
        CandidateInfo {
          who: DAVE,
          deposit: 10,
        },
        CandidateInfo {
          who: CHARLIE,
          deposit: 40,
        },
        CandidateInfo {
          who: BOB,
          deposit: 40,
        },
        CandidateInfo {
          who: ALICE,
          deposit: 20,
        },
      ]);
    let ranked_accounts = ranked
      .into_iter()
      .map(|candidate| candidate.who)
      .collect::<alloc::vec::Vec<_>>();
    assert_eq!(
      ranked_accounts,
      alloc::vec![ALICE, DAVE, BOB, CHARLIE, EVE, faythe]
    );
    let top_three = ranked_accounts
      .into_iter()
      .take(3)
      .collect::<alloc::vec::Vec<_>>();
    assert_eq!(top_three, alloc::vec![ALICE, DAVE, BOB]);
    assert_eq!(Staking::cached_delegated_native_backing(&ALICE), 500);
    assert_eq!(Staking::cached_delegated_native_backing(&DAVE), 500);
    assert_eq!(Staking::cached_delegated_native_backing(&BOB), 300);
    assert_eq!(Staking::cached_delegated_native_backing(&CHARLIE), 300);
    assert_eq!(Staking::cached_delegated_native_backing(&EVE), 0);
    assert_eq!(Staking::cached_delegated_native_backing(&ivan), 0);
  });
}

#[test]
fn rebinding_can_flip_top_n_membership_in_larger_candidate_set_without_waiting_for_on_idle() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    use polkadot_sdk::pallet_collator_selection::CandidateInfo;

    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE, BOB, CHARLIE, DAVE]).expect("invulnerables must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(mint_tokens(0, &ALICE, &EVE, 1_000));
    assert_ok!(mint_tokens(
      0,
      &ALICE,
      &crate::AccountId::new([6u8; 32]),
      1_000
    ));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(BOB),
      500,
      ALICE
    ));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(EVE), 400, BOB));
    let grace = crate::AccountId::new([6u8; 32]);
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(grace),
      300,
      CHARLIE
    ));
    let _ = Staking::on_idle(System::block_number(), Weight::MAX);
    let rank_candidates = || {
      crate::configs::DelegationWeightedCollatorSessionManager::rank_candidates(alloc::vec![
        CandidateInfo {
          who: DAVE,
          deposit: 10,
        },
        CandidateInfo {
          who: CHARLIE,
          deposit: 10,
        },
        CandidateInfo {
          who: BOB,
          deposit: 10,
        },
        CandidateInfo {
          who: ALICE,
          deposit: 10,
        },
      ])
      .into_iter()
      .map(|candidate| candidate.who)
      .collect::<alloc::vec::Vec<_>>()
    };
    assert_eq!(
      rank_candidates()
        .into_iter()
        .take(3)
        .collect::<alloc::vec::Vec<_>>(),
      alloc::vec![ALICE, BOB, CHARLIE]
    );
    assert_ok!(Staking::bind_native(RuntimeOrigin::signed(EVE), DAVE));
    assert_eq!(Staking::cached_delegated_native_backing(&BOB), 0);
    assert_eq!(Staking::cached_delegated_native_backing(&DAVE), 400);
    assert_eq!(
      rank_candidates()
        .into_iter()
        .take(3)
        .collect::<alloc::vec::Vec<_>>(),
      alloc::vec![ALICE, DAVE, CHARLIE]
    );
  });
}

#[test]
fn session_manager_top_n_boundary_prefers_account_order_on_equal_backing_and_deposit() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    use polkadot_sdk::pallet_collator_selection::CandidateInfo;

    let faythe = crate::AccountId::new([6u8; 32]);
    let grace = crate::AccountId::new([7u8; 32]);
    let heidi = crate::AccountId::new([8u8; 32]);
    let ivan = crate::AccountId::new([9u8; 32]);

    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE, BOB, CHARLIE, DAVE]).expect("invulnerables must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &faythe, 1_000));
    assert_ok!(mint_tokens(0, &ALICE, &grace, 1_000));
    assert_ok!(mint_tokens(0, &ALICE, &heidi, 1_000));
    assert_ok!(mint_tokens(0, &ALICE, &ivan, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(faythe),
      200,
      ALICE
    ));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(grace),
      200,
      BOB
    ));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(heidi),
      200,
      CHARLIE
    ));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(ivan),
      200,
      DAVE
    ));
    let _ = Staking::on_idle(System::block_number(), Weight::MAX);
    let ranked =
      crate::configs::DelegationWeightedCollatorSessionManager::rank_candidates(alloc::vec![
        CandidateInfo {
          who: DAVE,
          deposit: 10,
        },
        CandidateInfo {
          who: CHARLIE,
          deposit: 10,
        },
        CandidateInfo {
          who: BOB,
          deposit: 10,
        },
        CandidateInfo {
          who: ALICE,
          deposit: 10,
        },
      ]);
    let ranked_accounts = ranked
      .into_iter()
      .map(|candidate| candidate.who)
      .collect::<alloc::vec::Vec<_>>();
    assert_eq!(ranked_accounts, alloc::vec![ALICE, BOB, CHARLIE, DAVE]);
    let top_two = ranked_accounts
      .into_iter()
      .take(2)
      .collect::<alloc::vec::Vec<_>>();
    assert_eq!(top_two, alloc::vec![ALICE, BOB]);
  });
}

#[test]
fn ranking_probe_stays_candidate_bound_after_cache_refresh_with_many_bindings() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    use polkadot_sdk::pallet_collator_selection::CandidateInfo;

    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE, BOB, CHARLIE]).expect("invulnerables must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    for seed in 6u8..30u8 {
      let delegator = crate::AccountId::new([seed; 32]);
      let operator = match seed % 3 {
        0 => ALICE,
        1 => BOB,
        _ => CHARLIE,
      };
      assert_ok!(mint_tokens(0, &ALICE, &delegator, 101));
      assert_ok!(Staking::stake_native(
        RuntimeOrigin::signed(delegator),
        100,
        operator,
      ));
    }
    let _ = Staking::on_idle(System::block_number(), Weight::MAX);
    crate::configs::DelegationWeightedCollatorSessionManager::reset_ranking_backing_lookup_probe();
    let ranked = crate::configs::DelegationWeightedCollatorSessionManager::rank_candidates(
      alloc::vec![
        CandidateInfo {
          who: ALICE,
          deposit: 10,
        },
        CandidateInfo {
          who: BOB,
          deposit: 10,
        },
        CandidateInfo {
          who: CHARLIE,
          deposit: 10,
        },
      ],
    );
    let ranked_accounts = ranked
      .into_iter()
      .map(|candidate| candidate.who)
      .collect::<alloc::vec::Vec<_>>();
    assert_eq!(ranked_accounts.len(), 3);
    assert_eq!(
      crate::configs::DelegationWeightedCollatorSessionManager::ranking_backing_lookup_probe_count(),
      3,
      "clean cached ranking should perform one backing lookup per candidate, not per delegator binding",
    );
  });
}

#[test]
fn session_manager_ranks_candidates_by_backing_then_deposit_then_account() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    use polkadot_sdk::pallet_collator_selection::CandidateInfo;

    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE, BOB, CHARLIE, DAVE]).expect("invulnerables must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(mint_tokens(0, &ALICE, &EVE, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(BOB),
      400,
      CHARLIE
    ));
    assert_ok!(Staking::stake_native(
      RuntimeOrigin::signed(EVE),
      400,
      ALICE
    ));
    let _ = Staking::on_idle(System::block_number(), Weight::MAX);
    let ranked =
      crate::configs::DelegationWeightedCollatorSessionManager::rank_candidates(alloc::vec![
        CandidateInfo {
          who: DAVE,
          deposit: 30,
        },
        CandidateInfo {
          who: CHARLIE,
          deposit: 50,
        },
        CandidateInfo {
          who: BOB,
          deposit: 30,
        },
        CandidateInfo {
          who: ALICE,
          deposit: 30,
        },
      ]);
    let ranked_accounts = ranked
      .into_iter()
      .map(|candidate| candidate.who)
      .collect::<alloc::vec::Vec<_>>();
    assert_eq!(ranked_accounts, alloc::vec![CHARLIE, ALICE, BOB, DAVE]);
  });
}
