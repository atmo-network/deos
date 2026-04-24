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
use polkadot_sdk::sp_arithmetic::FixedPointNumber;
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

fn setup_native_staking_lp_nomination(
  owner: crate::AccountId,
  operator: crate::AccountId,
  amount: u128,
) {
  use polkadot_sdk::pallet_asset_conversion::PoolLocator;
  let native_asset_id = 0;
  assert_ok!(mint_tokens(native_asset_id, &ALICE, &owner, 1_000));
  assert_ok!(Staking::stake_native(
    RuntimeOrigin::signed(owner.clone()),
    500
  ));
  let staked_asset_id = Staking::staked_asset_id(native_asset_id).expect("stNTVE must resolve");
  let base_asset = crate::configs::AssetKind::Local(native_asset_id);
  let staked_asset = crate::configs::AssetKind::Local(staked_asset_id);
  let _ = create_pool(
    RuntimeOrigin::signed(owner.clone()),
    base_asset,
    staked_asset,
  );
  if <Assets as Inspect<_>>::balance(staked_asset_id, &owner) >= 400
    && <Assets as Inspect<_>>::balance(native_asset_id, &owner) >= 400
  {
    let _ = add_liquidity(
      RuntimeOrigin::signed(owner.clone()),
      base_asset,
      staked_asset,
      400,
      400,
      1,
      1,
      &owner,
    );
  }
  let pool_id =
    <crate::Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::pool_id(
      &base_asset,
      &staked_asset,
    )
    .expect("NTVE/stNTVE pool id must resolve");
  let pool = polkadot_sdk::pallet_asset_conversion::Pools::<crate::Runtime>::get(&pool_id)
    .expect("NTVE/stNTVE pool must exist");
  assert_ok!(Staking::lock_native_lp_for_collator(
    RuntimeOrigin::signed(owner),
    pool.lp_token,
    amount,
    operator,
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
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    setup_native_staking_lp_nomination(BOB, ALICE, 20);
    setup_native_staking_lp_nomination(CHARLIE, ALICE, 30);
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
fn runtime_governance_bldr_native_vote_power_is_frozen_against_lp_reserve_changes() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;

    let bldr_id = primitives::ecosystem::protocol_tokens::BLDR_ASSET_ID;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    setup_native_staking_lp_nomination(BOB, ALICE, 20);
    submit_governance_proposal(bldr_id, 162);
    cast_governance_vote_kind(BOB, bldr_id, 162, pallet_governance::ProposalVoteKind::Pass);
    assert_noop!(
      Staking::request_unlock_native_lp(RuntimeOrigin::signed(BOB), ALICE, 1),
      pallet_staking::Error::<crate::Runtime>::NativeGovernanceLockActive
    );
    let early_tally =
      Governance::proposal_vote_tally(bldr_id, 162).expect("proposal must stay active");
    assert_eq!(early_tally.pass_weight, 280);
    let early_view = Governance::account_governance_power_view(bldr_id, 162, BOB)
      .expect("active proposal must expose account governance view");
    assert!(early_view.governance_lock_until.is_some());
    assert_eq!(
      early_view.protection_power_profile,
      pallet_governance::ProposalVotePowerProfile::DecliningNativeStake
    );
    assert_eq!(early_view.current_protection_raw_power, 40);
    let frozen_pass = early_view
      .frozen_protection_ballot
      .expect("pass ballot must be frozen");
    assert_eq!(frozen_pass.vote, pallet_governance::ProposalVoteKind::Pass);
    assert_eq!(frozen_pass.weight, 280);
    assert_eq!(frozen_pass.raw_power, 40);
    assert_ok!(mint_tokens(0, &ALICE, &CHARLIE, 200));
    assert_ok!(
      crate::configs::AssetConversionAdapter::donate_native_staking_liquidity_from_ntve(
        &CHARLIE,
        200,
        polkadot_sdk::sp_runtime::Perbill::zero(),
      ),
      (100, 100)
    );
    let later_tally =
      Governance::proposal_vote_tally(bldr_id, 162).expect("proposal must stay active");
    assert_eq!(later_tally.pass_weight, 280);
    let later_view = Governance::account_governance_power_view(bldr_id, 162, BOB)
      .expect("active proposal must expose account governance view");
    assert!(later_view.current_protection_raw_power > frozen_pass.raw_power);
    assert_eq!(later_view.frozen_protection_ballot, Some(frozen_pass));
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_governance_standalone_lp_lock_feeds_native_vote_power() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    use polkadot_sdk::pallet_asset_conversion::PoolLocator;

    let bldr_id = primitives::ecosystem::protocol_tokens::BLDR_ASSET_ID;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(BOB), 500));
    let staked_asset_id = Staking::staked_asset_id(0).expect("stNTVE must resolve");
    let base_asset = crate::configs::AssetKind::Local(0);
    let staked_asset = crate::configs::AssetKind::Local(staked_asset_id);
    assert_ok!(create_pool(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset,
    ));
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset,
      400,
      400,
      1,
      1,
      &BOB,
    ));
    let pool_id =
      <crate::Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::pool_id(
        &base_asset,
        &staked_asset,
      )
      .expect("NTVE/stNTVE pool id must resolve");
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<crate::Runtime>::get(&pool_id)
      .expect("NTVE/stNTVE pool must exist");
    assert_ok!(Staking::lock_native_lp_for_governance(
      RuntimeOrigin::signed(BOB),
      pool.lp_token,
      20,
    ));
    assert_eq!(Staking::account_native_lp_locked(BOB), 20);
    assert_eq!(Staking::account_native_collator_lp_locked(BOB), 0);
    assert_eq!(
      crate::configs::DelegationWeightedCollatorSessionManager::native_nomination_reward_base_weight(
        &BOB,
      ),
      0
    );
    submit_governance_proposal(bldr_id, 163);
    cast_governance_vote_kind(BOB, bldr_id, 163, pallet_governance::ProposalVoteKind::Pass);
    let tally = Governance::proposal_vote_tally(bldr_id, 163).expect("proposal must stay active");
    assert_eq!(tally.pass_weight, 280);
    assert_noop!(
      Staking::request_unlock_native_lp_for_governance(RuntimeOrigin::signed(BOB), 1),
      pallet_staking::Error::<crate::Runtime>::NativeGovernanceLockActive
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_governance_native_and_stntve_locks_feed_native_vote_power() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;

    let bldr_id = primitives::ecosystem::protocol_tokens::BLDR_ASSET_ID;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(BOB), 100));
    let staked_asset_id = Staking::staked_asset_id(0).expect("stNTVE must resolve");
    assert_ok!(Staking::lock_native_asset_for_governance(
      RuntimeOrigin::signed(BOB),
      0,
      20,
    ));
    assert_ok!(Staking::lock_native_asset_for_governance(
      RuntimeOrigin::signed(BOB),
      staked_asset_id,
      20,
    ));
    submit_governance_proposal(bldr_id, 164);
    cast_governance_vote_kind(BOB, bldr_id, 164, pallet_governance::ProposalVoteKind::Pass);
    let tally = Governance::proposal_vote_tally(bldr_id, 164).expect("proposal must stay active");
    assert_eq!(tally.pass_weight, 280);
    assert_noop!(
      Staking::request_unlock_native_asset_for_governance(
        RuntimeOrigin::signed(BOB),
        staked_asset_id,
        1,
      ),
      pallet_staking::Error::<crate::Runtime>::NativeGovernanceLockActive
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_native_reward_snapshot_uses_collator_locked_lp_base_weight() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;

    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    setup_native_staking_lp_nomination(BOB, ALICE, 20);
    record_winning_vote(0, 700, BOB);
    let base_weight =
      crate::configs::DelegationWeightedCollatorSessionManager::native_nomination_reward_base_weight(
        &BOB,
      );
    assert!(base_weight > 0);
    advance_to_block(2);
    assert_eq!(
      Staking::reward_active_weight(0, &BOB),
      Some(FixedU128::from_rational(1u128, 12u128).saturating_mul_int(base_weight))
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_claim_nomination_reward_pays_liquid_ntve() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;

    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &ALICE, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    setup_native_staking_lp_nomination(BOB, ALICE, 20);
    record_winning_vote(0, 702, BOB);
    let reward_account = Staking::reward_account_for(0);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      0,
      reward_account.clone().into(),
      84,
    ));
    advance_to_block(2);
    advance_to_block(3);
    let bob_balance_before = <Assets as Inspect<_>>::balance(0, &BOB);
    let bob_stntve_before = <Assets as Inspect<_>>::balance(
      Staking::staked_asset_id(0).expect("stNTVE must resolve"),
      &BOB,
    );
    assert_eq!(Staking::reward_claimable(0, 2, &BOB), Some(84));
    assert_eq!(
      Staking::native_nomination_reward_claimable(2, BOB),
      Some(84)
    );
    assert_ok!(Staking::claim_nomination_reward(
      RuntimeOrigin::signed(BOB),
      2
    ));
    assert_eq!(Staking::reward_claimed((0, 2), BOB), Some(84));
    assert_eq!(Staking::reward_liability_balance(0), 0);
    assert_eq!(
      <Assets as Inspect<_>>::balance(0, &BOB),
      bob_balance_before + 84
    );
    assert_eq!(
      <Assets as Inspect<_>>::balance(Staking::staked_asset_id(0).unwrap(), &BOB),
      bob_stntve_before
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_can_compound_liquid_nomination_reward_into_locked_lp() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;

    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &ALICE, 2_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    setup_native_staking_lp_nomination(BOB, ALICE, 20);
    record_winning_vote(0, 703, BOB);
    let reward_account = Staking::reward_account_for(0);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      0,
      reward_account.into(),
      840,
    ));
    advance_to_block(2);
    advance_to_block(3);
    let locked_before = Staking::account_native_collator_lp_locked(BOB);
    assert_ok!(Staking::claim_and_compound_nomination_reward(
      RuntimeOrigin::signed(BOB),
      2,
      ALICE,
    ));
    assert!(Staking::reward_claimed((0, 2), BOB).is_some());
    assert_eq!(Staking::reward_liability_balance(0), 0);
    assert!(Staking::account_native_collator_lp_locked(BOB) > locked_before);
    assert_eq!(
      Staking::operator_native_lp_locked(ALICE),
      Staking::account_native_collator_lp_locked(BOB)
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn runtime_native_governance_touch_bypasses_reward_event_scan() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    record_winning_vote(0, 701, BOB);
    System::reset_events();
    let ingress_weight = Staking::on_idle(System::block_number(), Weight::from_parts(100_000, 0));
    assert_eq!(ingress_weight.ref_time(), 0);
    let touched_accounts = pallet_staking::RewardEpochTouchedAccounts::<crate::Runtime>::get(1, 0);
    assert_eq!(touched_accounts.len(), 1);
    assert!(touched_accounts.contains(&BOB));
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
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    setup_native_staking_lp_nomination(BOB, ALICE, 30);
    setup_native_staking_lp_nomination(CHARLIE, ALICE, 20);
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
fn native_reward_account_transfer_is_ignored_by_event_ingress_and_reconciled_on_epoch_rollover() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &ALICE, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    let reward_account = Staking::reward_account_for(0);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(ALICE),
      0,
      reward_account.into(),
      75,
    ));
    advance_to_block(2);
    assert_eq!(Staking::reward_epoch_accrued(0, 1), 0);
    assert_eq!(Staking::reward_epoch_accrued(0, 2), 75);
    assert_eq!(Staking::reward_liability_balance(0), 75);
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
    let ingress_weight = <crate::configs::staking_config::RuntimeLegacyRewardSnapshotEventIngress as pallet_staking::RewardSnapshotEventIngress<crate::BlockNumber>>::ingest(
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
    let ingress_weight = <crate::configs::staking_config::RuntimeLegacyRewardSnapshotEventIngress as pallet_staking::RewardSnapshotEventIngress<crate::BlockNumber>>::ingest(
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
fn legacy_reward_event_ingress_aggregates_same_block_reward_inflows_under_finite_budget() {
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
fn legacy_reward_event_ingress_stops_at_tiny_on_idle_weight_budget() {
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
    let ingress_weight = Staking::on_idle(System::block_number(), Weight::from_parts(1_000, 0));
    assert_eq!(ingress_weight.ref_time(), 1_000);
    Staking::on_finalize(System::block_number());
    assert_eq!(Staking::reward_epoch_accrued(ASSET_A, 1), 0);
    assert_eq!(Staking::reward_liability_balance(ASSET_A), 0);
  });
}

#[test]
fn legacy_reward_event_ingress_stops_at_tiny_remaining_weight_budget() {
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
    let ingress_weight = <crate::configs::staking_config::RuntimeLegacyRewardSnapshotEventIngress as pallet_staking::RewardSnapshotEventIngress<crate::BlockNumber>>::ingest(
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
fn legacy_reward_event_ingress_records_governance_touches_under_finite_budget() {
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
    assert_eq!(touched_accounts.len(), 2);
    assert!(touched_accounts.contains(&BOB));
    assert!(touched_accounts.contains(&CHARLIE));
    assert_eq!(Staking::last_reward_ingress_truncated_epoch(), None);
  });
}

#[test]
fn legacy_reward_ingress_receipt_lookup_probe_stays_event_bound_with_many_pools() {
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
fn legacy_reward_ingress_governance_lookup_probe_stays_domain_bound_with_many_pools() {
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
fn legacy_reward_event_ingress_emits_truncation_signal_when_scan_cap_is_hit_under_finite_budget() {
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
fn native_receipt_transfer_is_ignored_by_reward_event_ingress() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(BOB), 120));
    advance_to_block(2);
    let staked_asset_id = Staking::staked_asset_id(0).expect("stNTVE must resolve");
    System::reset_events();
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(BOB),
      staked_asset_id,
      CHARLIE.into(),
      50,
    ));
    let ingress_weight = Staking::on_idle(System::block_number(), Weight::from_parts(100_000, 0));
    assert!(ingress_weight.ref_time() > 0);
    let touched_accounts = pallet_staking::RewardEpochTouchedAccounts::<crate::Runtime>::get(2, 0);
    assert_eq!(touched_accounts.len(), 0);
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
fn ntve_stntve_pool_direct_balanced_donation_increases_lp_value_without_minting_lp() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::pallet_asset_conversion::PoolLocator;
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(BOB), 500));
    let staked_asset_id = Staking::staked_asset_id(0).expect("staked asset id must resolve");
    let base_asset = crate::configs::AssetKind::Local(0);
    let staked_asset = crate::configs::AssetKind::Local(staked_asset_id);
    assert_ok!(create_pool(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset
    ));
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset,
      400,
      400,
      1,
      1,
      &BOB,
    ));
    let pool_id =
      <crate::Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::pool_id(
        &base_asset,
        &staked_asset,
      )
      .expect("NTVE/stNTVE pool id must resolve");
    let pool_account =
      <crate::Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::address(
        &pool_id,
      )
      .expect("NTVE/stNTVE pool account must resolve");
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<crate::Runtime>::get(&pool_id)
      .expect("NTVE/stNTVE pool must exist");
    let lp_supply_before =
      <crate::Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolAssets::total_issuance(
        pool.lp_token,
      );
    assert_eq!(<Assets as Inspect<_>>::balance(0, &pool_account), 400);
    assert_eq!(
      <Assets as Inspect<_>>::balance(staked_asset_id, &pool_account),
      400
    );
    assert_noop!(
      crate::configs::AssetConversionAdapter::donate_balanced_liquidity(
        &BOB,
        base_asset,
        staked_asset,
        40,
        20,
        polkadot_sdk::sp_runtime::Perbill::from_percent(1),
      ),
      polkadot_sdk::sp_runtime::DispatchError::Other("DonationRatioExceeded")
    );
    assert_ok!(
      crate::configs::AssetConversionAdapter::donate_balanced_liquidity(
        &BOB,
        base_asset,
        staked_asset,
        40,
        40,
        polkadot_sdk::sp_runtime::Perbill::zero(),
      )
    );
    let lp_supply_after =
      <crate::Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolAssets::total_issuance(
        pool.lp_token,
      );
    assert_eq!(lp_supply_after, lp_supply_before);
    assert_eq!(<Assets as Inspect<_>>::balance(0, &pool_account), 440);
    assert_eq!(
      <Assets as Inspect<_>>::balance(staked_asset_id, &pool_account),
      440
    );
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 80));
    assert_ok!(
      crate::configs::AssetConversionAdapter::donate_native_staking_liquidity_from_ntve(
        &BOB,
        80,
        polkadot_sdk::sp_runtime::Perbill::zero(),
      ),
      (40, 40)
    );
    let lp_supply_after_acquisition =
      <crate::Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolAssets::total_issuance(
        pool.lp_token,
      );
    assert_eq!(lp_supply_after_acquisition, lp_supply_before);
    assert_eq!(<Assets as Inspect<_>>::balance(0, &pool_account), 480);
    assert_eq!(
      <Assets as Inspect<_>>::balance(staked_asset_id, &pool_account),
      480
    );
  });
}

#[test]
fn runtime_can_lock_ntve_stntve_lp_for_collator() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::pallet_asset_conversion::PoolLocator;
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      polkadot_sdk::frame_support::BoundedVec::try_from(alloc::vec![ALICE])
        .expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(BOB), 500));
    let staked_asset_id = Staking::staked_asset_id(0).expect("staked asset id must resolve");
    let base_asset = crate::configs::AssetKind::Local(0);
    let staked_asset = crate::configs::AssetKind::Local(staked_asset_id);
    assert_ok!(create_pool(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset,
    ));
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(BOB),
      base_asset,
      staked_asset,
      400,
      400,
      1,
      1,
      &BOB,
    ));
    let pool_id =
      <crate::Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::pool_id(
        &base_asset,
        &staked_asset,
      )
      .expect("NTVE/stNTVE pool id must resolve");
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<crate::Runtime>::get(&pool_id)
      .expect("NTVE/stNTVE pool must exist");
    let lock_account = Staking::native_lp_lock_account();
    let bob_lp_before = <Assets as Inspect<_>>::balance(pool.lp_token, &BOB);
    assert!(bob_lp_before >= 10);
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(BOB),
      pool.lp_token,
      10,
      ALICE,
    ));
    assert_eq!(
      <Assets as Inspect<_>>::balance(pool.lp_token, &BOB),
      bob_lp_before - 10
    );
    assert_eq!(
      <Assets as Inspect<_>>::balance(pool.lp_token, &lock_account),
      10
    );
    assert_eq!(
      Staking::native_lp_lock(BOB, ALICE)
        .expect("lock must exist")
        .amount,
      10
    );
    assert_eq!(Staking::operator_native_lp_locked(ALICE), 10);
    assert_eq!(Staking::account_native_collator_lp_locked(BOB), 10);
    assert!(
      crate::configs::DelegationWeightedCollatorSessionManager::native_nomination_reward_base_weight(
        &BOB,
      ) > 0
    );
    assert!(
      crate::configs::DelegationWeightedCollatorSessionManager::conservative_native_lp_backing_value(
        &ALICE,
      ) > 0
    );
    let ranked = crate::configs::DelegationWeightedCollatorSessionManager::rank_candidates(
      alloc::vec![
        polkadot_sdk::pallet_collator_selection::CandidateInfo {
          who: CHARLIE,
          deposit: 100,
        },
        polkadot_sdk::pallet_collator_selection::CandidateInfo {
          who: ALICE,
          deposit: 1,
        },
      ],
    );
    assert_eq!(ranked.first().map(|candidate| candidate.who.clone()), Some(ALICE));
    System::assert_last_event(RuntimeEvent::Staking(
      pallet_staking::Event::NativeLpLocked {
        account: BOB,
        operator: ALICE,
        lp_asset_id: pool.lp_token,
        amount: 10,
        total_locked: 10,
      },
    ));
  });
}

#[test]
fn runtime_native_staking_read_model_exposes_bounded_surfaces() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      polkadot_sdk::frame_support::BoundedVec::try_from(alloc::vec![ALICE])
        .expect("single invulnerable must fit"),
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    setup_native_staking_lp_nomination(BOB, ALICE, 20);
    let pool = Staking::native_staking_liquidity_pool().expect("pool read model must exist");
    assert_eq!(pool.native_asset_id, 0);
    assert_eq!(pool.staked_asset_id, Staking::staked_asset_id(0).unwrap());
    assert_eq!(pool.reserve_native, 400);
    assert_eq!(pool.reserve_staked, 400);
    assert_eq!(pool.lp_total_issuance, 400);
    assert_eq!(
      Staking::native_staking_exchange_rate(),
      Some(FixedU128::from_rational(1u128, 1u128))
    );
    let position = Staking::native_locked_lp_position(BOB);
    assert_eq!(position.total_locked_lp, 20);
    assert_eq!(position.collator_locked_lp, 20);
    assert_eq!(position.governance_locked_lp, 0);
    assert_eq!(position.conservative_native_value, Some(40));
    let collator_position = Staking::native_collator_lp_position(BOB, ALICE);
    assert_eq!(collator_position.lp_asset_id, Some(pool.lp_asset_id));
    assert_eq!(collator_position.locked_lp, 20);
    assert_eq!(collator_position.pending_unlock_lp, 0);
    assert_eq!(collator_position.pending_unlock_block, None);
    assert_eq!(collator_position.conservative_native_value, Some(40));
    assert_ok!(Staking::request_unlock_native_lp(
      RuntimeOrigin::signed(BOB),
      ALICE,
      5
    ));
    let unlock_block = System::block_number()
      .saturating_add(crate::configs::staking_config::NativeLpUnlockDelay::get());
    let collator_position = Staking::native_collator_lp_position(BOB, ALICE);
    assert_eq!(collator_position.locked_lp, 15);
    assert_eq!(collator_position.pending_unlock_lp, 5);
    assert_eq!(collator_position.pending_unlock_block, Some(unlock_block));
    assert_ok!(Staking::lock_native_lp_for_governance(
      RuntimeOrigin::signed(BOB),
      pool.lp_asset_id,
      10
    ));
    assert_ok!(Staking::request_unlock_native_lp_for_governance(
      RuntimeOrigin::signed(BOB),
      4
    ));
    assert_ok!(Staking::lock_native_asset_for_governance(
      RuntimeOrigin::signed(BOB),
      0,
      50
    ));
    assert_ok!(Staking::request_unlock_native_asset_for_governance(
      RuntimeOrigin::signed(BOB),
      0,
      20
    ));
    let governance_position = Staking::native_governance_custody_position(BOB, 0);
    assert_eq!(governance_position.lp_asset_id, Some(pool.lp_asset_id));
    assert_eq!(governance_position.governance_locked_lp, 6);
    assert_eq!(governance_position.pending_governance_lp_unlock, 4);
    assert_eq!(
      governance_position.pending_governance_lp_unlock_block,
      Some(unlock_block)
    );
    assert_eq!(governance_position.asset_id, 0);
    assert_eq!(governance_position.asset_locked, 30);
    assert_eq!(governance_position.pending_asset_unlock, 20);
    assert_eq!(
      governance_position.pending_asset_unlock_block,
      Some(unlock_block)
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
fn runtime_native_stake_helpers_treat_stntve_as_passive_liquid_receipt() {
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
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(BOB), 400));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(CHARLIE), 300));
    let staked_asset_id = Staking::staked_asset_id(0).expect("staked asset id must resolve");
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(CHARLIE),
      staked_asset_id,
      DAVE.into(),
      120,
    ));
    assert_eq!(Staking::native_stake_value(&BOB), Some(400));
    assert_eq!(Staking::passive_native_stake_value(&BOB), Some(400));
    assert_eq!(Staking::delegated_native_stake_value(&BOB), None);
    assert_eq!(
      Staking::stake_exposure(0, &BOB),
      Some(pallet_staking::StakeExposure {
        total_value: 400,
        passive_value: 400,
        delegated_value: 0,
        delegated_operator: None,
      })
    );
    assert_eq!(Staking::native_stake_value(&CHARLIE), Some(180));
    assert_eq!(Staking::passive_native_stake_value(&CHARLIE), Some(180));
    assert_eq!(Staking::delegated_native_stake_value(&CHARLIE), None);
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
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(BOB), 400));
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
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(BOB), 200));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(CHARLIE), ASSET_A, 900));
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
fn stntve_transfer_no_longer_changes_session_ranking_after_lp_cutover() {
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
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(BOB), 400));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(EVE), 300));
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
    assert_eq!(refreshed_accounts, alloc::vec![ALICE, CHARLIE]);
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
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(BOB), 500));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(EVE), 500));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(grace), 300));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(heidi), 300));
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
      alloc::vec![EVE, BOB, CHARLIE, ALICE, DAVE, faythe]
    );
    let top_three = ranked_accounts
      .into_iter()
      .take(3)
      .collect::<alloc::vec::Vec<_>>();
    assert_eq!(top_three, alloc::vec![EVE, BOB, CHARLIE]);
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
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(faythe), 200));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(grace), 200));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(heidi), 200));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(ivan), 200));
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
fn ranking_probe_stays_candidate_bound_with_many_stntve_holders() {
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
      assert_ok!(mint_tokens(0, &ALICE, &delegator, 101));
      assert_ok!(Staking::stake_native(RuntimeOrigin::signed(delegator), 100));
    }
    let _ = Staking::on_idle(System::block_number(), Weight::MAX);
    crate::configs::DelegationWeightedCollatorSessionManager::reset_ranking_backing_lookup_probe();
    let ranked =
      crate::configs::DelegationWeightedCollatorSessionManager::rank_candidates(alloc::vec![
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
      ]);
    let ranked_accounts = ranked
      .into_iter()
      .map(|candidate| candidate.who)
      .collect::<alloc::vec::Vec<_>>();
    assert_eq!(ranked_accounts.len(), 3);
    assert_eq!(
      crate::configs::DelegationWeightedCollatorSessionManager::ranking_backing_lookup_probe_count(
      ),
      3,
      "ranking should perform one backing lookup per candidate, not per unrelated stNTVE holder",
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
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(BOB), 400));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(EVE), 400));
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
