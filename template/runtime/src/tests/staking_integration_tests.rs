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
    Staking::on_finalize(current);
    System::reset_events();
    let next = current.saturating_add(1);
    System::set_block_number(next);
    let _ = Staking::on_initialize(next);
    let _ = Governance::on_initialize(next);
  }
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn lp_backed_security_path_composes_sessions_funding_claim_expiry_and_cleanup() {
  use polkadot_sdk::frame_support::traits::Currency;
  use polkadot_sdk::pallet_collator_selection::CandidateInfo;
  use polkadot_sdk::pallet_session::SessionManager;

  new_test_ext().execute_with(|| {
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    setup_native_staking_lp_nomination(BOB, ALICE, 20);
    assert_ok!(Governance::record_winning_vote(
      RuntimeOrigin::root(),
      0,
      900,
      BOB,
    ));
    polkadot_sdk::pallet_collator_selection::CandidateList::<crate::Runtime>::put(
      polkadot_sdk::frame_support::BoundedVec::try_from(alloc::vec![CandidateInfo {
        who: ALICE,
        deposit: 1,
      }])
      .expect("candidate fits"),
    );
    polkadot_sdk::pallet_collator_selection::DesiredCandidates::<crate::Runtime>::put(1);
    assert_eq!(
      Staking::native_security_readiness(),
      pallet_staking::NativeSecurityReadiness::Ready
    );
    let selected = <crate::configs::DelegationWeightedCollatorSessionManager as SessionManager<
      crate::AccountId,
    >>::new_session(0)
    .expect("ready LP-backed session plans");
    assert_eq!(selected, alloc::vec![ALICE]);
    let planned = Staking::native_security_epoch_snapshot(0).expect("planned snapshot");
    assert_eq!(planned.eligible_operators[0].operator, ALICE);
    assert_eq!(planned.participants[0].account, BOB);
    <crate::configs::DelegationWeightedCollatorSessionManager as SessionManager<
      crate::AccountId,
    >>::start_session(0);
    assert_eq!(
      Staking::native_security_reward_pot(0)
        .expect("open reward pot")
        .status,
      pallet_staking::NativeSecurityRewardPotStatus::Open
    );

    let source = crate::configs::staking_config::SecurityRewardFundingSource::get();
    let reward: crate::Balance = 1_000;
    let _ = crate::Balances::deposit_creating(&source, reward.saturating_mul(2));
    assert_ok!(Staking::fund_native_security_reward(
      RuntimeOrigin::root(),
      reward,
    ));
    assert_eq!(Staking::native_security_reward_liability(), reward);

    assert!(
      <crate::configs::DelegationWeightedCollatorSessionManager as SessionManager<
        crate::AccountId,
      >>::new_session(1)
      .is_some()
    );
    polkadot_sdk::pallet_session::CurrentIndex::<crate::Runtime>::put(1);
    <crate::configs::DelegationWeightedCollatorSessionManager as SessionManager<
      crate::AccountId,
    >>::start_session(1);
    assert_eq!(
      Staking::native_security_reward_pot(0)
        .expect("finalized reward pot")
        .status,
      pallet_staking::NativeSecurityRewardPotStatus::Finalized
    );
    let bob_before = crate::Balances::free_balance(&BOB);
    assert_ok!(Staking::claim_native_security_reward(
      RuntimeOrigin::signed(BOB),
      0,
    ));
    assert!(crate::Balances::free_balance(&BOB) > bob_before);
    assert!(Staking::native_security_reward_claimed(0, BOB).is_some());

    assert_ok!(Staking::fund_native_security_reward(
      RuntimeOrigin::root(),
      reward,
    ));
    assert!(
      <crate::configs::DelegationWeightedCollatorSessionManager as SessionManager<
        crate::AccountId,
      >>::new_session(2)
      .is_some()
    );
    polkadot_sdk::pallet_session::CurrentIndex::<crate::Runtime>::put(2);
    <crate::configs::DelegationWeightedCollatorSessionManager as SessionManager<
      crate::AccountId,
    >>::start_session(2);
    let expiry_epoch = 1u32
      .saturating_add(crate::configs::staking_config::SecurityRewardClaimHorizon::get())
      .saturating_add(1);
    polkadot_sdk::pallet_session::CurrentIndex::<crate::Runtime>::put(expiry_epoch);
    assert_ok!(Staking::expire_native_security_reward(
      RuntimeOrigin::signed(CHARLIE),
      1,
    ));
    assert_eq!(
      Staking::native_security_reward_pot(1)
        .expect("expired pot retained for bounded cleanup")
        .status,
      pallet_staking::NativeSecurityRewardPotStatus::Expired
    );
    assert_ok!(Staking::cleanup_expired_native_security_reward(
      RuntimeOrigin::signed(CHARLIE),
      1,
    ));
    assert!(Staking::native_security_epoch_snapshot(1).is_none());
    assert!(Staking::native_security_reward_pot(1).is_none());
  });
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn compound_path_proves_exact_reward_reserve_issuance_custody_and_backing_deltas() {
  use polkadot_sdk::frame_support::traits::{Currency, fungibles::Inspect};

  new_test_ext().execute_with(|| {
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    setup_native_staking_lp_nomination(BOB, ALICE, 10);
    polkadot_sdk::pallet_collator_selection::Invulnerables::<crate::Runtime>::put(
      polkadot_sdk::frame_support::BoundedVec::try_from(alloc::vec![ALICE])
        .expect("single invulnerable fits"),
    );
    let reward = 1_000;
    let epoch = 0;
    polkadot_sdk::pallet_session::CurrentIndex::<crate::Runtime>::put(1);
    let participant = pallet_staking::NativeSecurityAccountSnapshot {
      account: BOB,
      conservative_native_value: 1,
      governance_coefficient: FixedU128::from_u32(1),
      reward_weight: 1,
    };
    let snapshot = pallet_staking::NativeSecurityEpochSnapshot {
      epoch,
      participants: polkadot_sdk::frame_support::BoundedVec::try_from(alloc::vec![participant])
        .expect("single participant fits"),
      eligible_operators: Default::default(),
      total_reward_weight: 1,
    };
    pallet_staking::NativeSecurityEpochSnapshots::<crate::Runtime>::insert(epoch, snapshot);
    pallet_staking::NativeSecurityRewardPots::<crate::Runtime>::insert(
      epoch,
      pallet_staking::NativeSecurityRewardPot {
        total_reward_weight: 1,
        credited: reward,
        claimed: 0,
        status: pallet_staking::NativeSecurityRewardPotStatus::Finalized,
      },
    );
    pallet_staking::NativeSecurityRewardLiability::<crate::Runtime>::put(reward);
    let reward_account = Staking::native_security_reward_account();
    let _ = crate::Balances::deposit_creating(&reward_account, reward + crate::EXISTENTIAL_DEPOSIT);
    let prior_position =
      Staking::native_lp_lock(BOB, ALICE).expect("nomination fixture creates prior lock");
    let prior_lock = prior_position.amount;
    let lp_asset_id = prior_position.lp_asset_id;
    let staked_asset_id = Staking::staked_asset_id(0).expect("registered staked asset exists");
    let wallet_lp_before = <Assets as Inspect<_>>::balance(lp_asset_id, &BOB);
    let bob_native_before = crate::Balances::free_balance(&BOB);
    let reward_custody_before = crate::Balances::free_balance(&reward_account);
    let native_issuance_before = <Assets as Inspect<_>>::total_issuance(0);
    let staked_issuance_before = <Assets as Inspect<_>>::total_issuance(staked_asset_id);
    let lp_issuance_before = <Assets as Inspect<_>>::total_issuance(lp_asset_id);
    let (_, reserve_native_before, reserve_staked_before, _) =
      crate::configs::AssetConversionAdapter::native_staking_liquidity_pool_read_model()
        .expect("canonical native staking pool exists");
    let staking_pool_before = pallet_staking::Pools::<crate::Runtime>::get(0)
      .expect("registered native staking pool exists");
    let operator_backing_before =
      crate::configs::DelegationWeightedCollatorSessionManager::collator_backing_value(&ALICE);

    assert_ok!(Staking::claim_and_compound_native_security_reward(
      RuntimeOrigin::signed(BOB),
      epoch,
      ALICE,
      1,
    ));

    Staking::native_security_reward_claimed(epoch, BOB)
      .expect("compound consumes the frozen claim exactly once");
    assert_eq!(Staking::native_security_reward_liability(), 0);
    assert_eq!(
      Staking::native_security_reward_pot(epoch)
        .expect("finalized pot remains retained")
        .claimed,
      reward,
    );
    assert_eq!(
      crate::Balances::free_balance(&reward_account),
      reward_custody_before - reward,
    );
    assert_eq!(
      crate::Balances::free_balance(&BOB),
      bob_native_before,
      "the claimed native reward is consumed entirely by compound",
    );

    let lock = Staking::native_lp_lock(BOB, ALICE).expect("compound LP remains locked");
    let lp_out = lock.amount - prior_lock;
    assert!(lp_out > 0);
    assert_eq!(
      <Assets as Inspect<_>>::balance(lp_asset_id, &BOB),
      wallet_lp_before,
      "newly minted compound LP must move entirely into nomination custody",
    );
    assert_eq!(
      <Assets as Inspect<_>>::total_issuance(lp_asset_id) - lp_issuance_before,
      lp_out,
    );
    let (_, reserve_native_after, reserve_staked_after, _) =
      crate::configs::AssetConversionAdapter::native_staking_liquidity_pool_read_model()
        .expect("canonical native staking pool remains available");
    let staking_pool_after = pallet_staking::Pools::<crate::Runtime>::get(0)
      .expect("native staking pool remains available");
    let native_reserve_delta = reserve_native_after - reserve_native_before;
    let staked_reserve_delta = reserve_staked_after - reserve_staked_before;
    let stake_delta = staking_pool_after.accounted_balance - staking_pool_before.accounted_balance;
    let share_delta = staking_pool_after.total_shares - staking_pool_before.total_shares;
    assert_eq!(native_reserve_delta + stake_delta, reward);
    assert_eq!(staked_reserve_delta, share_delta);
    assert_eq!(
      <Assets as Inspect<_>>::total_issuance(0) - native_issuance_before,
      reward,
    );
    assert_eq!(
      <Assets as Inspect<_>>::total_issuance(staked_asset_id) - staked_issuance_before,
      share_delta,
    );
    assert!(
      crate::configs::DelegationWeightedCollatorSessionManager::collator_backing_value(&ALICE)
        > operator_backing_before,
    );
  });
}

#[test]
fn native_security_reward_compound_adapter_mints_canonical_lp_with_bounds() {
  use pallet_staking::NativeSecurityRewardCompound;
  use polkadot_sdk::frame_support::traits::fungibles::Inspect;

  new_test_ext().execute_with(|| {
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    setup_native_staking_lp_nomination(BOB, ALICE, 10);
    let (lp_asset_id, _, _, _) =
      crate::configs::AssetConversionAdapter::native_staking_liquidity_pool_read_model()
        .expect("canonical pool exists");
    let lp_before = <Assets as Inspect<_>>::balance(lp_asset_id, &BOB);
    let native_before = crate::Balances::free_balance(&BOB);

    let (actual_lp_asset_id, lp_out) =
      <crate::configs::staking_config::RuntimeNativeSecurityRewardCompound as NativeSecurityRewardCompound<
        crate::AccountId,
        u32,
        u128,
      >>::compound(&BOB, 1_000, 1)
      .expect("balanced compound succeeds");

    assert_eq!(actual_lp_asset_id, lp_asset_id);
    assert!(lp_out > 0);
    assert_eq!(
      <Assets as Inspect<_>>::balance(lp_asset_id, &BOB),
      lp_before + lp_out
    );
    assert_eq!(crate::Balances::free_balance(&BOB), native_before - 1_000);
  });
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

#[cfg(not(feature = "runtime-benchmarks"))]
fn governance_protection_last_open_epoch() -> crate::BlockNumber {
  crate::configs::governance_config::ProposalProtectionPeriod::get()
}

#[cfg(not(feature = "runtime-benchmarks"))]
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
  let lock_account = Staking::native_lp_lock_account();
  if System::providers(&lock_account) == 0 {
    let _ = System::inc_providers(&lock_account);
  }
  let owner_lp_before = <Assets as Inspect<_>>::balance(pool.lp_token, &owner);
  assert!(owner_lp_before >= amount);
  assert_ok!(Assets::transfer(
    RuntimeOrigin::signed(owner.clone()),
    pool.lp_token,
    lock_account.into(),
    amount,
  ));
  let is_new_participant =
    pallet_staking::NativeNominationOperators::<crate::Runtime>::get(&owner).is_empty();
  pallet_staking::NativeLpLocks::<crate::Runtime>::insert(
    &owner,
    &operator,
    pallet_staking::NativeLpLock {
      lp_asset_id: pool.lp_token,
      amount,
    },
  );
  pallet_staking::NativeNominationOperators::<crate::Runtime>::mutate(&owner, |operators| {
    if !operators.contains(&operator) {
      operators
        .try_push(operator.clone())
        .expect("test nomination must fit");
    }
  });
  if is_new_participant {
    pallet_staking::NativeSecurityParticipants::<crate::Runtime>::mutate(|participants| {
      participants
        .try_push(owner.clone())
        .expect("test participant must fit");
    });
  }
  pallet_staking::OperatorNativeLpLocked::<crate::Runtime>::mutate(&operator, |total| {
    *total = total.saturating_add(amount);
  });
  pallet_staking::AccountNativeLpLocked::<crate::Runtime>::mutate(&owner, |total| {
    *total = total.saturating_add(amount);
  });
  pallet_staking::AccountNativeCollatorLpLocked::<crate::Runtime>::mutate(&owner, |total| {
    *total = total.saturating_add(amount);
  });
  pallet_staking::TotalNativeLpLocked::<crate::Runtime>::mutate(|total| {
    *total = total.saturating_add(amount);
  });
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
fn governance_participation_coefficient_remains_runtime_configured_per_domain() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_eq!(
      Staking::governance_participation_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_inner(0))
    );
    record_winning_vote(ASSET_A, 100, BOB);
    assert_eq!(
      Staking::governance_participation_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
  });
}

#[test]
fn runtime_governance_zero_sum_eviction_clears_governance_participation_coefficient_after_decay() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    record_winning_vote(ASSET_A, 100, BOB);
    assert_eq!(
      Staking::governance_participation_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
    System::set_block_number(4);
    Governance::on_initialize(4);
    assert_eq!(
      Staking::governance_participation_coefficient(ASSET_A, &BOB),
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
      Staking::governance_participation_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
    assert_eq!(
      Staking::governance_participation_coefficient(ASSET_A, &CHARLIE),
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
      Staking::governance_participation_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
    assert_eq!(
      Staking::governance_participation_coefficient(ASSET_A, &CHARLIE),
      Some(FixedU128::from_inner(0))
    );
    assert_eq!(
      Staking::governance_participation_coefficient(ASSET_A, &DAVE),
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
      Staking::governance_participation_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
    assert_eq!(
      Staking::governance_participation_coefficient(ASSET_A, &CHARLIE),
      Some(FixedU128::from_inner(0))
    );
    assert_eq!(
      Staking::governance_participation_coefficient(ASSET_A, &DAVE),
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
      Staking::governance_participation_coefficient(ASSET_A, &BOB),
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
      Staking::governance_participation_coefficient(ASSET_A, &BOB),
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
        100,
        polkadot_sdk::sp_runtime::Perbill::zero(),
      ),
      (100, 100)
    );
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 20));
    assert_ok!(Staking::lock_native_asset_for_governance(
      RuntimeOrigin::signed(BOB),
      0,
      20,
    ));
    record_winning_vote(bldr_id, 702, BOB);
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
fn trusted_security_path_composes_liquid_receipt_donation_governance_and_exit() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::pallet_asset_conversion::PoolLocator;

    assert_eq!(
      Staking::native_security_mode(),
      pallet_staking::NativeSecurityMode::TrustedSet
    );
    assert_eq!(
      Staking::native_security_capabilities(),
      pallet_staking::NativeSecurityCapabilities {
        new_nominations: false,
        redelegation: false,
        candidate_selection: false,
        reward_funding: false,
        reward_claims: false,
        reward_compound: false,
        custody_exit: true,
      }
    );
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(mint_tokens(0, &ALICE, &BOB, 1_000));
    assert_ok!(mint_tokens(0, &ALICE, &CHARLIE, 200));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    assert_ok!(Staking::stake_native(RuntimeOrigin::signed(BOB), 500));
    let staked_asset_id = Staking::staked_asset_id(0).expect("stNTVE must resolve");
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(BOB),
      staked_asset_id,
      DAVE.into(),
      50,
    ));
    assert_eq!(<Assets as Inspect<_>>::balance(staked_asset_id, &DAVE), 50);

    let native = crate::configs::AssetKind::Local(0);
    let staked = crate::configs::AssetKind::Local(staked_asset_id);
    assert_ok!(create_pool(RuntimeOrigin::signed(BOB), native, staked));
    assert_ok!(add_liquidity(
      RuntimeOrigin::signed(BOB),
      native,
      staked,
      400,
      400,
      1,
      1,
      &BOB,
    ));
    let pool_id =
      <crate::Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::pool_id(
        &native, &staked,
      )
      .expect("native staking pool id");
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<crate::Runtime>::get(&pool_id)
      .expect("native staking pool");
    let lp_supply_before = <Assets as Inspect<_>>::total_issuance(pool.lp_token);
    assert_ok!(Staking::lock_native_lp_for_governance(
      RuntimeOrigin::signed(BOB),
      pool.lp_token,
      20,
    ));

    let domain = primitives::ecosystem::protocol_tokens::BLDR_ASSET_ID;
    submit_governance_proposal(domain, 165);
    cast_governance_vote_kind(BOB, domain, 165, pallet_governance::ProposalVoteKind::Pass);
    let power_view = Governance::account_governance_power_view(domain, 165, BOB)
      .expect("active proposal power view");
    let governance_unlock = power_view
      .governance_lock_until
      .expect("vote extends governance custody");
    let frozen = power_view.frozen_protection_ballot.expect("frozen ballot");
    assert_noop!(
      Staking::request_unlock_native_lp_for_governance(RuntimeOrigin::signed(BOB), 20),
      pallet_staking::Error::<crate::Runtime>::NativeGovernanceLockActive
    );
    assert_ok!(
      crate::configs::AssetConversionAdapter::donate_native_staking_liquidity_from_ntve(
        &CHARLIE,
        200,
        100,
        polkadot_sdk::sp_runtime::Perbill::zero(),
      ),
      (100, 100)
    );
    assert_eq!(
      <Assets as Inspect<_>>::total_issuance(pool.lp_token),
      lp_supply_before
    );
    assert_eq!(
      Governance::account_governance_power_view(domain, 165, BOB)
        .expect("updated power view")
        .frozen_protection_ballot,
      Some(frozen)
    );

    reject_governance_proposal(domain, 165);
    System::set_block_number(governance_unlock);
    assert_ok!(Staking::request_unlock_native_lp_for_governance(
      RuntimeOrigin::signed(BOB),
      20,
    ));
    assert_noop!(
      Staking::withdraw_unlocked_native_lp_for_governance(RuntimeOrigin::signed(BOB)),
      pallet_staking::Error::<crate::Runtime>::NativeLpUnlockNotReady
    );
    System::set_block_number(
      System::block_number()
        .saturating_add(crate::configs::staking_config::NativeLpUnlockDelay::get()),
    );
    assert_ok!(Staking::withdraw_unlocked_native_lp_for_governance(
      RuntimeOrigin::signed(BOB),
    ));
    let shares_before = <Assets as Inspect<_>>::balance(staked_asset_id, &BOB);
    let native_before = <Assets as Inspect<_>>::balance(0, &BOB);
    assert_ok!(Staking::unstake(RuntimeOrigin::signed(BOB), 0, 10));
    assert_eq!(
      <Assets as Inspect<_>>::balance(staked_asset_id, &BOB),
      shares_before - 10
    );
    assert!(<Assets as Inspect<_>>::balance(0, &BOB) > native_before);
    assert_eq!(Staking::account_native_collator_lp_locked(BOB), 0);
    assert_eq!(Staking::operator_native_lp_locked(ALICE), 0);
    assert_eq!(
      Staking::native_security_readiness(),
      pallet_staking::NativeSecurityReadiness::Inactive
    );
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
fn runtime_governance_participation_does_not_touch_staking_snapshots() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    assert_ok!(create_test_asset(0, &ALICE));
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 0));
    record_winning_vote(0, 701, BOB);
    assert_eq!(
      Governance::governance_participation_coefficient(0, BOB),
      FixedU128::from_rational(1u128, 12u128)
    );
    System::reset_events();
    assert_eq!(
      Staking::on_idle(System::block_number(), Weight::MAX),
      Weight::zero()
    );
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
      Staking::governance_participation_coefficient(ASSET_A, &BOB),
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
      Staking::governance_participation_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
    assert_eq!(
      Staking::governance_participation_coefficient(ASSET_A, &CHARLIE),
      Some(FixedU128::from_inner(0))
    );
    assert_eq!(
      Staking::governance_participation_coefficient(ASSET_A, &DAVE),
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
      Staking::governance_participation_coefficient(ASSET_A, &BOB),
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
      Staking::governance_participation_coefficient(ASSET_A, &BOB),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
    assert_eq!(
      Staking::governance_participation_coefficient(ASSET_A, &CHARLIE),
      Some(FixedU128::from_rational(1u128, 12u128))
    );
    assert_eq!(Governance::expiry_bucket(4).len(), 2);
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
        40,
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
fn trusted_mode_rejects_new_collator_lp_nomination_without_custody_mutation() {
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
    assert_noop!(
      Staking::lock_native_lp_for_collator(RuntimeOrigin::signed(BOB), pool.lp_token, 10, ALICE,),
      pallet_staking::Error::<crate::Runtime>::NativeSecurityModeInactive
    );
    assert_eq!(
      <Assets as Inspect<_>>::balance(pool.lp_token, &BOB),
      bob_lp_before
    );
    assert_eq!(
      <Assets as Inspect<_>>::balance(pool.lp_token, &lock_account),
      0
    );
    assert!(Staking::native_lp_lock(BOB, ALICE).is_none());
    assert_eq!(Staking::operator_native_lp_locked(ALICE), 0);
    assert_eq!(Staking::account_native_collator_lp_locked(BOB), 0);
    assert_eq!(
      Staking::native_security_readiness(),
      pallet_staking::NativeSecurityReadiness::Inactive
    );
    assert_eq!(
      Staking::native_security_capabilities(),
      pallet_staking::NativeSecurityCapabilities {
        new_nominations: false,
        redelegation: false,
        candidate_selection: false,
        reward_funding: false,
        reward_claims: false,
        reward_compound: false,
        custody_exit: true,
      }
    );
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
fn external_inflow_sync_preserves_share_vault_yield_without_claim_state() {
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
fn native_security_topology_rejects_duplicate_candidates_and_requires_backed_operator() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    use polkadot_sdk::frame_support::BoundedVec;
    use polkadot_sdk::pallet_collator_selection::CandidateInfo;

    polkadot_sdk::pallet_collator_selection::CandidateList::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![
        CandidateInfo {
          who: ALICE,
          deposit: 10,
        },
        CandidateInfo {
          who: ALICE,
          deposit: 20,
        },
      ])
      .expect("duplicate candidate fixture must remain bounded"),
    );
    assert_eq!(
      crate::configs::DelegationWeightedCollatorSessionManager::native_security_topology_readiness(
      ),
      Some(pallet_staking::NativeSecurityReadiness::CandidateSetInconsistent),
    );

    polkadot_sdk::pallet_collator_selection::CandidateList::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![CandidateInfo {
        who: ALICE,
        deposit: 10,
      }])
      .expect("single candidate fixture must remain bounded"),
    );
    pallet_staking::NativeSecurityParticipants::<crate::Runtime>::put(
      BoundedVec::try_from(alloc::vec![BOB]).expect("single participant must fit"),
    );
    pallet_staking::NativeNominationOperators::<crate::Runtime>::insert(
      BOB,
      BoundedVec::try_from(alloc::vec![ALICE]).expect("single nomination must fit"),
    );
    assert_eq!(
      crate::configs::DelegationWeightedCollatorSessionManager::native_security_topology_readiness(
      ),
      Some(pallet_staking::NativeSecurityReadiness::EligibleOperatorSetEmpty),
      "candidate deposits cannot establish LP-backed operator eligibility",
    );
    pallet_staking::OperatorNativeLpLocked::<crate::Runtime>::insert(ALICE, 1);
    assert_eq!(
      crate::configs::DelegationWeightedCollatorSessionManager::native_security_topology_readiness(
      ),
      Some(pallet_staking::NativeSecurityReadiness::Ready),
    );
  });
}

#[test]
fn native_security_epoch_tracks_session_not_block_cadence() {
  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    System::set_block_number(42);
    polkadot_sdk::pallet_session::CurrentIndex::<crate::Runtime>::put(7);
    assert_eq!(Staking::current_security_epoch(), 7);

    System::set_block_number(99);
    assert_eq!(Staking::current_security_epoch(), 7);
    polkadot_sdk::pallet_session::CurrentIndex::<crate::Runtime>::put(8);
    assert_eq!(Staking::current_security_epoch(), 8);
  });
}

#[test]
fn security_epoch_identity_is_shared_by_runtime_planning_funding_and_claim_views() {
  use pallet_staking::SecurityEpochProvider;

  let mut ext = seeded_test_ext();
  ext.execute_with(|| {
    polkadot_sdk::pallet_session::CurrentIndex::<crate::Runtime>::put(7);
    let provider_epoch =
      <crate::configs::staking_config::RuntimeSecurityEpochProvider as SecurityEpochProvider>::current_security_epoch();
    assert_eq!(provider_epoch, 7);
    assert_eq!(Staking::current_security_epoch(), provider_epoch);

    let diagnostic = pallet_staking::NativeSecurityBoundaryDiagnostic {
      planned_epoch: provider_epoch,
      readiness: pallet_staking::NativeSecurityReadiness::Inactive,
    };
    pallet_staking::LastNativeSecurityBoundaryDiagnostic::<crate::Runtime>::put(diagnostic);
    assert_eq!(
      Staking::last_native_security_boundary_diagnostic()
        .expect("bounded diagnostic exists")
        .planned_epoch,
      Staking::current_security_epoch(),
    );

    System::set_block_number(999);
    assert_eq!(Staking::current_security_epoch(), provider_epoch);
  });
}

#[test]
fn trusted_security_mode_ignores_candidates_and_reports_capabilities() {
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
    assert_eq!(
      Staking::native_security_mode(),
      pallet_staking::NativeSecurityMode::TrustedSet
    );
    let collators = <crate::configs::DelegationWeightedCollatorSessionManager as SessionManager<
      crate::AccountId,
    >>::new_session(0)
    .expect("session manager must return a collator set");
    assert_eq!(collators, alloc::vec![ALICE]);
    assert_eq!(Staking::last_native_security_boundary_diagnostic(), None);
    System::set_block_number(99);
    let _ = Staking::on_initialize(System::block_number());
    assert_eq!(
      Staking::last_native_security_boundary_diagnostic(),
      None,
      "ordinary block hooks cannot create session-bound diagnostics",
    );
  });
}

#[test]
fn session_manager_ranks_larger_candidate_set_by_backing_then_account() {
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
      alloc::vec![ALICE, BOB, CHARLIE, DAVE, EVE, faythe]
    );
    let top_three = ranked_accounts
      .into_iter()
      .take(3)
      .collect::<alloc::vec::Vec<_>>();
    assert_eq!(top_three, alloc::vec![ALICE, BOB, CHARLIE]);
  });
}

#[test]
fn session_manager_top_n_boundary_prefers_account_order_on_equal_backing() {
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
fn session_manager_never_uses_candidate_deposit_as_security_backing() {
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
    assert_eq!(ranked_accounts, alloc::vec![ALICE, BOB, CHARLIE, DAVE]);
  });
}
