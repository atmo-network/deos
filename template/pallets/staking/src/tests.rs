use crate::{Error, Event, NativeLpLocks, OperatorNativeLpLocked, PendingNativeLpUnlocks, mock::*};
use polkadot_sdk::frame_support::{
  BoundedVec, assert_noop, assert_ok,
  traits::{
    Hooks,
    fungibles::{Inspect, Mutate, metadata::Inspect as MetadataInspect},
  },
};
use polkadot_sdk::sp_runtime::{FixedU128, traits::One};

fn empty_security_snapshot(
  epoch: crate::SecurityEpoch,
) -> crate::NativeSecurityEpochSnapshot<
  AccountId,
  Balance,
  MaxNativeSecurityParticipants,
  MaxNativeSecurityOperators,
> {
  crate::NativeSecurityEpochSnapshot {
    epoch,
    participants: Default::default(),
    eligible_operators: Default::default(),
    total_reward_weight: 0,
  }
}

fn advance_to_block(target: u64) {
  while System::block_number() < target {
    let current = System::block_number();
    Staking::on_finalize(current);
    System::reset_events();
    let next = current.saturating_add(1);
    System::set_block_number(next);
    let _ = Staking::on_initialize(next);
  }
}

#[test]
fn registration_requires_resolvable_receipt_identity() {
  new_test_ext().execute_with(|| {
    assert_ok!(Assets::force_create(RuntimeOrigin::root(), 99, 1, true, 1));
    assert_noop!(
      Staking::register_staking_asset(RuntimeOrigin::root(), 99),
      Error::<Test>::StakedAssetUnsupported
    );
    assert!(Staking::pool(99).is_none());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_reconciles_native_security_reward_liability_and_custody() {
  new_test_ext().execute_with(|| {
    let mut snapshot = empty_security_snapshot(0);
    snapshot
      .participants
      .try_push(crate::NativeSecurityAccountSnapshot {
        account: 1,
        conservative_native_value: 1,
        governance_coefficient: FixedU128::one(),
        reward_weight: 1,
      })
      .expect("participant fits");
    snapshot.total_reward_weight = 1;
    crate::ActiveNativeSecurityEpochSnapshot::<Test>::put(&snapshot);
    crate::NativeSecurityEpochSnapshots::<Test>::insert(0, snapshot);
    crate::NativeSecurityRewardPots::<Test>::insert(
      0,
      crate::NativeSecurityRewardPot {
        total_reward_weight: 1,
        credited: 10,
        claimed: 0,
        status: crate::NativeSecurityRewardPotStatus::Open,
      },
    );
    crate::NativeSecurityRewardLiability::<Test>::put(10);
    let reward_account = Staking::native_security_reward_account();
    let _ =
      <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::deposit_creating(
        &reward_account,
        11,
      );
    assert_ok!(Staking::do_try_state());

    crate::NativeSecurityRewardLiability::<Test>::put(9);
    assert!(Staking::do_try_state().is_err());
    crate::NativeSecurityRewardLiability::<Test>::put(10);
    let _ = <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::slash(
      &reward_account,
      2,
    );
    assert!(Staking::do_try_state().is_err());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_native_security_reward_claim_drift() {
  new_test_ext().execute_with(|| {
    crate::NativeSecurityEpochSnapshots::<Test>::insert(0, empty_security_snapshot(0));
    crate::NativeSecurityRewardPots::<Test>::insert(
      0,
      crate::NativeSecurityRewardPot {
        total_reward_weight: 0,
        credited: 0,
        claimed: 0,
        status: crate::NativeSecurityRewardPotStatus::Finalized,
      },
    );
    crate::NativeSecurityRewardClaims::<Test>::insert(0, 1, ());
    set_security_epoch(1);
    assert!(Staking::do_try_state().is_err());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_native_security_reward_retention_above_the_hard_bound() {
  new_test_ext().execute_with(|| {
    for epoch in 1..=SecurityRewardClaimHorizon::get() + 3 {
      crate::NativeSecurityEpochSnapshots::<Test>::insert(epoch, empty_security_snapshot(epoch));
      crate::NativeSecurityRewardPots::<Test>::insert(
        epoch,
        crate::NativeSecurityRewardPot {
          total_reward_weight: 0,
          credited: 0,
          claimed: 0,
          status: crate::NativeSecurityRewardPotStatus::Finalized,
        },
      );
    }
    set_security_epoch(SecurityRewardClaimHorizon::get() + 10);
    assert!(Staking::do_try_state().is_err());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_native_nomination_index_drift() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 20));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      10,
      99,
    ));
    assert_ok!(Staking::do_try_state());
    crate::NativeNominationOperators::<Test>::remove(1);
    assert!(Staking::do_try_state().is_err());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_native_nomination_aggregate_drift() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 20));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      10,
      99,
    ));
    assert_ok!(Staking::do_try_state());

    crate::OperatorNativeLpLocked::<Test>::insert(99, 9);
    assert!(Staking::do_try_state().is_err());
    crate::OperatorNativeLpLocked::<Test>::insert(99, 10);
    crate::AccountNativeLpLocked::<Test>::insert(1, 11);
    assert!(Staking::do_try_state().is_err());
    crate::AccountNativeLpLocked::<Test>::insert(1, 10);
    crate::TotalNativeLpLocked::<Test>::put(11);
    assert!(Staking::do_try_state().is_err());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_reconciles_pending_unlocks_with_physical_lp_custody() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 40));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      40,
      99,
    ));
    assert_ok!(Staking::request_unlock_native_lp(
      RuntimeOrigin::signed(1),
      99,
      15,
    ));
    assert_ok!(Staking::do_try_state());

    let lock_account = Staking::native_lp_lock_account();
    assert_ok!(<Assets as Mutate<AccountId>>::burn_from(
      LP_ASSET,
      &lock_account,
      1,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Expendable,
      polkadot_sdk::frame_support::traits::tokens::Precision::Exact,
      polkadot_sdk::frame_support::traits::tokens::Fortitude::Force,
    ));
    assert!(Staking::do_try_state().is_err());
  });
}

#[test]
fn native_security_boundary_diagnostic_is_one_overwritten_bounded_value() {
  new_test_ext().execute_with(|| {
    assert_eq!(Staking::last_native_security_boundary_diagnostic(), None);
    Staking::note_native_security_boundary(
      7,
      crate::NativeSecurityBoundaryOutcome::NotReady(
        crate::NativeSecurityReadiness::NativePoolMissing,
      ),
    );
    assert_eq!(
      Staking::last_native_security_boundary_diagnostic(),
      Some(crate::NativeSecurityBoundaryDiagnostic {
        planned_epoch: 7,
        outcome: crate::NativeSecurityBoundaryOutcome::NotReady(
          crate::NativeSecurityReadiness::NativePoolMissing,
        ),
      }),
    );
    Staking::note_native_security_boundary(8, crate::NativeSecurityBoundaryOutcome::SnapshotOpened);
    assert_eq!(
      Staking::last_native_security_boundary_diagnostic(),
      Some(crate::NativeSecurityBoundaryDiagnostic {
        planned_epoch: 8,
        outcome: crate::NativeSecurityBoundaryOutcome::SnapshotOpened,
      }),
    );
  });
}

#[test]
fn native_security_readiness_is_mode_aware_and_fail_closed() {
  new_test_ext().execute_with(|| {
    assert_eq!(
      Staking::native_security_readiness(),
      crate::NativeSecurityReadiness::NativePoolMissing
    );
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_eq!(
      Staking::native_security_readiness(),
      crate::NativeSecurityReadiness::LiquidityPoolMissing
    );
    set_native_security_mode(crate::NativeSecurityMode::TrustedSet);
    assert!(matches!(
      Staking::native_security_view(),
      Ok(crate::NativeSecurityView::TrustedSet { .. })
    ));
  });
}

fn native_security_view_error_name(error: crate::NativeSecurityViewError) -> &'static str {
  match error {
    crate::NativeSecurityViewError::RetentionBoundExceeded => "RetentionBoundExceeded",
    crate::NativeSecurityViewError::MultiplePlannedEpochs => "MultiplePlannedEpochs",
  }
}

#[test]
fn native_security_view_signature_and_errors_are_compiler_exhaustive() {
  let _: fn() -> Result<crate::NativeSecurityView, crate::NativeSecurityViewError> =
    Staking::native_security_view;
  assert_eq!(
    native_security_view_error_name(crate::NativeSecurityViewError::RetentionBoundExceeded),
    "RetentionBoundExceeded"
  );
  assert_eq!(
    native_security_view_error_name(crate::NativeSecurityViewError::MultiplePlannedEpochs),
    "MultiplePlannedEpochs"
  );
}

#[test]
fn native_security_view_owns_mode_readiness_epoch_plan_and_obligation_truth() {
  new_test_ext().execute_with(|| {
    let view = Staking::native_security_view().expect("empty bounded view is valid");
    assert!(matches!(
      view,
      crate::NativeSecurityView::LpBackedSelection {
        readiness: crate::NativeSecurityReadiness::NativePoolMissing,
        current_epoch: 0,
        planned_epoch: None,
        settlement_obligations_remain: false,
      }
    ));

    crate::NativeSecurityRewardPots::<Test>::insert(
      1,
      crate::NativeSecurityRewardPot {
        total_reward_weight: 0,
        credited: 0,
        claimed: 0,
        status: crate::NativeSecurityRewardPotStatus::Planned,
      },
    );
    let planned = Staking::native_security_view().expect("one planned epoch is valid");
    assert!(matches!(
      planned,
      crate::NativeSecurityView::LpBackedSelection {
        planned_epoch: Some(1),
        settlement_obligations_remain: false,
        ..
      }
    ));

    crate::NativeSecurityRewardPots::<Test>::mutate(1, |pot| {
      pot.as_mut().expect("pot exists").status = crate::NativeSecurityRewardPotStatus::Open;
    });
    let open = Staking::native_security_view().expect("one open epoch is valid");
    assert!(matches!(
      open,
      crate::NativeSecurityView::LpBackedSelection {
        planned_epoch: None,
        settlement_obligations_remain: true,
        ..
      }
    ));

    for epoch in [2, 3] {
      crate::NativeSecurityRewardPots::<Test>::insert(
        epoch,
        crate::NativeSecurityRewardPot {
          total_reward_weight: 0,
          credited: 0,
          claimed: 0,
          status: crate::NativeSecurityRewardPotStatus::Planned,
        },
      );
    }
    assert_eq!(
      Staking::native_security_view(),
      Err(crate::NativeSecurityViewError::MultiplePlannedEpochs)
    );

    for epoch in 2..=6 {
      crate::NativeSecurityRewardPots::<Test>::insert(
        epoch,
        crate::NativeSecurityRewardPot {
          total_reward_weight: 0,
          credited: 0,
          claimed: 0,
          status: crate::NativeSecurityRewardPotStatus::Finalized,
        },
      );
    }
    assert_eq!(
      Staking::native_security_view(),
      Err(crate::NativeSecurityViewError::RetentionBoundExceeded)
    );
  });
}

#[test]
fn native_security_epoch_snapshot_is_atomic_and_filters_ineligible_operators() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 100));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &2, 100));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      40,
      99,
    ));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      10,
      2,
    ));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(2),
      LP_ASSET,
      30,
      99,
    ));

    assert_ok!(Staking::open_native_security_epoch(7, &[99]));
    set_security_epoch(7);
    assert_ok!(Staking::activate_native_security_epoch(7));
    let snapshot = Staking::active_native_security_epoch_snapshot()
      .expect("planned boundary must activate atomically at session start");
    assert_eq!(snapshot.epoch, 7);
    assert_eq!(snapshot.eligible_operators.len(), 1);
    assert_eq!(snapshot.eligible_operators[0].operator, 99);
    assert_eq!(
      snapshot.eligible_operators[0].conservative_native_backing,
      70
    );
    assert_eq!(snapshot.participants.len(), 2);
    assert_eq!(snapshot.participants[0].account, 1);
    assert_eq!(snapshot.participants[0].conservative_native_value, 40);
    assert_eq!(
      snapshot.participants[0].governance_coefficient,
      FixedU128::from_rational(2u128, 10u128)
    );
    assert_eq!(snapshot.participants[0].reward_weight, 8);
    assert_eq!(snapshot.participants[1].account, 2);
    assert_eq!(snapshot.participants[1].conservative_native_value, 30);
    assert_eq!(
      snapshot.participants[1].governance_coefficient,
      FixedU128::from_rational(3u128, 10u128)
    );
    assert_eq!(snapshot.participants[1].reward_weight, 9);
    assert_eq!(snapshot.total_reward_weight, 17);
    let retained = Staking::native_security_epoch_snapshot(7)
      .expect("opened snapshot must be retained by session identity");
    assert_eq!(retained.epoch, snapshot.epoch);
    assert_eq!(retained.total_reward_weight, snapshot.total_reward_weight);
    assert_eq!(retained.participants, snapshot.participants);
    assert_eq!(retained.eligible_operators, snapshot.eligible_operators);
    assert_eq!(
      Staking::native_security_reward_pot(7),
      Some(crate::NativeSecurityRewardPot {
        total_reward_weight: 17,
        credited: 0,
        claimed: 0,
        status: crate::NativeSecurityRewardPotStatus::Open,
      })
    );
    assert_eq!(Staking::native_security_reward_liability(), 0);
  });
}

#[test]
fn governance_and_pool_changes_after_planning_affect_only_later_epoch() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 100));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      40,
      99,
    ));
    set_governance_coefficient(1, FixedU128::from_rational(1u128, 2u128));
    assert_ok!(Staking::open_native_security_epoch(0, &[99]));
    assert_ok!(Staking::activate_native_security_epoch(0));
    assert_ok!(Staking::open_native_security_epoch(1, &[99]));
    let planned = Staking::native_security_epoch_snapshot(1)
      .expect("future epoch freezes governance and pool state");
    assert_eq!(planned.participants[0].conservative_native_value, 40);
    assert_eq!(
      planned.participants[0].governance_coefficient,
      FixedU128::from_rational(1u128, 2u128)
    );
    assert_eq!(planned.participants[0].reward_weight, 20);

    set_governance_coefficient(1, FixedU128::from_rational(3u128, 4u128));
    set_native_lp_value_multiplier(2);
    assert_eq!(
      Staking::native_security_epoch_snapshot(1)
        .expect("frozen plan cannot reread live state")
        .participants,
      planned.participants
    );

    set_security_epoch(1);
    assert_ok!(Staking::activate_native_security_epoch(1));
    assert_ok!(Staking::open_native_security_epoch(2, &[99]));
    let later = Staking::native_security_epoch_snapshot(2)
      .expect("later plan observes changed governance and pool state");
    assert_eq!(later.participants[0].conservative_native_value, 80);
    assert_eq!(
      later.participants[0].governance_coefficient,
      FixedU128::from_rational(3u128, 4u128)
    );
    assert_eq!(later.participants[0].reward_weight, 60);
  });
}

#[test]
fn eligible_operator_changes_after_planning_affect_only_later_epoch() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 100));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      40,
      99,
    ));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      10,
      2,
    ));
    assert_ok!(Staking::open_native_security_epoch(0, &[99]));
    assert_ok!(Staking::activate_native_security_epoch(0));
    assert_ok!(Staking::open_native_security_epoch(1, &[99]));
    let planned =
      Staking::native_security_epoch_snapshot(1).expect("future epoch freezes eligible operators");
    assert_eq!(planned.eligible_operators[0].operator, 99);
    assert_eq!(planned.participants[0].conservative_native_value, 40);

    assert_eq!(
      Staking::native_security_epoch_snapshot(1)
        .expect("frozen eligibility cannot change")
        .eligible_operators,
      planned.eligible_operators
    );
    set_security_epoch(1);
    assert_ok!(Staking::activate_native_security_epoch(1));
    assert_ok!(Staking::open_native_security_epoch(2, &[2]));
    let later = Staking::native_security_epoch_snapshot(2)
      .expect("later epoch uses changed eligible operator set");
    assert_eq!(later.eligible_operators[0].operator, 2);
    assert_eq!(later.participants[0].conservative_native_value, 10);
  });
}

#[test]
fn finalized_security_reward_claims_survive_trusted_mode_and_conserve_liability() {
  new_test_ext().execute_with(|| {
    let mut snapshot = empty_security_snapshot(0);
    snapshot
      .participants
      .try_push(crate::NativeSecurityAccountSnapshot {
        account: 1,
        conservative_native_value: 40,
        governance_coefficient: FixedU128::one(),
        reward_weight: 40,
      })
      .expect("participant fits");
    snapshot
      .participants
      .try_push(crate::NativeSecurityAccountSnapshot {
        account: 2,
        conservative_native_value: 60,
        governance_coefficient: FixedU128::one(),
        reward_weight: 60,
      })
      .expect("participant fits");
    snapshot.total_reward_weight = 100;
    crate::NativeSecurityEpochSnapshots::<Test>::insert(0, snapshot);
    crate::NativeSecurityRewardPots::<Test>::insert(
      0,
      crate::NativeSecurityRewardPot {
        total_reward_weight: 100,
        credited: 101,
        claimed: 0,
        status: crate::NativeSecurityRewardPotStatus::Finalized,
      },
    );
    crate::NativeSecurityRewardLiability::<Test>::put(101);
    let reward_account = Staking::native_security_reward_account();
    let _ =
      <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::deposit_creating(
        &reward_account,
        101,
      );
    let account_before = Balances::free_balance(1);
    set_native_security_mode(crate::NativeSecurityMode::TrustedSet);
    assert_noop!(
      Staking::claim_and_compound_native_security_reward(RuntimeOrigin::signed(1), 0, 99, 1),
      Error::<Test>::NativeSecurityModeInactive
    );

    assert_ok!(Staking::claim_native_security_reward(
      RuntimeOrigin::signed(1),
      0
    ));

    assert_eq!(Balances::free_balance(1), account_before + 40);
    assert_eq!(Balances::free_balance(reward_account), 61);
    assert_eq!(Staking::native_security_reward_liability(), 61);
    assert_eq!(
      Staking::native_security_reward_pot(0)
        .expect("pot retained")
        .claimed,
      40
    );
    assert!(Staking::native_security_reward_claimed(0, 1).is_some());
    assert_noop!(
      Staking::claim_native_security_reward(RuntimeOrigin::signed(1), 0),
      Error::<Test>::NativeSecurityRewardAlreadyClaimed
    );
  });
}

#[test]
fn compound_security_reward_claims_lock_output_atomically() {
  new_test_ext().execute_with(|| {
    const LP_ASSET: AssetId = 0x7000_0001;
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    let mut snapshot = empty_security_snapshot(0);
    snapshot
      .participants
      .try_push(crate::NativeSecurityAccountSnapshot {
        account: 1,
        conservative_native_value: 1,
        governance_coefficient: FixedU128::one(),
        reward_weight: 1,
      })
      .expect("participant fits");
    snapshot.total_reward_weight = 1;
    crate::NativeSecurityEpochSnapshots::<Test>::insert(0, snapshot);
    crate::NativeSecurityRewardPots::<Test>::insert(
      0,
      crate::NativeSecurityRewardPot {
        total_reward_weight: 1,
        credited: 10,
        claimed: 0,
        status: crate::NativeSecurityRewardPotStatus::Finalized,
      },
    );
    crate::NativeSecurityRewardLiability::<Test>::put(10);
    let reward_account = Staking::native_security_reward_account();
    let _ =
      <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::deposit_creating(
        &reward_account,
        11,
      );
    set_compound_lp_out(10);

    assert_ok!(Staking::claim_and_compound_native_security_reward(
      RuntimeOrigin::signed(1),
      0,
      99,
      10,
    ));

    assert_eq!(Staking::native_security_reward_liability(), 0);
    assert!(Staking::native_security_reward_claimed(0, 1).is_some());
    assert_eq!(
      Staking::native_lp_lock(1, 99).expect("LP locked").amount,
      10
    );
    assert_eq!(
      Assets::balance(LP_ASSET, &Staking::native_lp_lock_account()),
      10
    );
  });
}

#[test]
fn compound_security_reward_claims_roll_back_every_effect() {
  new_test_ext().execute_with(|| {
    const LP_ASSET: AssetId = 0x7000_0001;
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    let mut snapshot = empty_security_snapshot(0);
    snapshot
      .participants
      .try_push(crate::NativeSecurityAccountSnapshot {
        account: 1,
        conservative_native_value: 1,
        governance_coefficient: FixedU128::one(),
        reward_weight: 1,
      })
      .expect("participant fits");
    snapshot.total_reward_weight = 1;
    crate::NativeSecurityEpochSnapshots::<Test>::insert(0, snapshot);
    crate::NativeSecurityRewardPots::<Test>::insert(
      0,
      crate::NativeSecurityRewardPot {
        total_reward_weight: 1,
        credited: 10,
        claimed: 0,
        status: crate::NativeSecurityRewardPotStatus::Finalized,
      },
    );
    crate::NativeSecurityRewardLiability::<Test>::put(10);
    let reward_account = Staking::native_security_reward_account();
    let _ =
      <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::deposit_creating(
        &reward_account,
        11,
      );

    set_compound_failure(true);
    assert!(
      Staking::claim_and_compound_native_security_reward(RuntimeOrigin::signed(1), 0, 99, 1,)
        .is_err()
    );
    set_compound_failure(false);
    assert_eq!(Staking::native_security_reward_liability(), 10);
    assert!(Staking::native_security_reward_claimed(0, 1).is_none());
    assert_eq!(
      Staking::native_security_reward_pot(0)
        .expect("pot retained")
        .claimed,
      0
    );
    assert_eq!(Balances::free_balance(reward_account), 11);

    set_compound_lp_out(9);
    assert_noop!(
      Staking::claim_and_compound_native_security_reward(RuntimeOrigin::signed(1), 0, 99, 10),
      Error::<Test>::InsufficientCompoundLpOutput
    );
    assert_eq!(Staking::native_security_reward_liability(), 10);
    assert!(Staking::native_security_reward_claimed(0, 1).is_none());
    assert_eq!(Assets::balance(LP_ASSET, &1), 0);
    assert!(Staking::native_lp_lock(1, 99).is_none());

    assert_noop!(
      Staking::claim_and_compound_native_security_reward(RuntimeOrigin::signed(1), 0, 1, 1),
      Error::<Test>::CannotNominateSelf
    );
    assert_noop!(
      Staking::claim_and_compound_native_security_reward(RuntimeOrigin::signed(1), 0, 77, 1),
      Error::<Test>::InvalidNativeOperatorTarget
    );
  });
}

#[test]
fn batch_security_reward_claims_share_validation_and_roll_back_on_failure() {
  new_test_ext().execute_with(|| {
    for epoch in [0, 1] {
      let mut snapshot = empty_security_snapshot(epoch);
      snapshot
        .participants
        .try_push(crate::NativeSecurityAccountSnapshot {
          account: 1,
          conservative_native_value: 1,
          governance_coefficient: FixedU128::one(),
          reward_weight: 1,
        })
        .expect("participant fits");
      snapshot.total_reward_weight = 1;
      crate::NativeSecurityEpochSnapshots::<Test>::insert(epoch, snapshot);
      crate::NativeSecurityRewardPots::<Test>::insert(
        epoch,
        crate::NativeSecurityRewardPot {
          total_reward_weight: 1,
          credited: 10,
          claimed: 0,
          status: crate::NativeSecurityRewardPotStatus::Finalized,
        },
      );
    }
    crate::NativeSecurityRewardLiability::<Test>::put(20);
    let reward_account = Staking::native_security_reward_account();
    let _ =
      <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::deposit_creating(
        &reward_account,
        21,
      );
    set_security_epoch(2);
    let epochs =
      BoundedVec::<u32, MaxSecurityRewardClaimsPerCall>::try_from(vec![0, 1]).expect("batch fits");
    assert_ok!(Staking::claim_native_security_reward_batch(
      RuntimeOrigin::signed(1),
      epochs,
    ));
    assert_eq!(Staking::native_security_reward_liability(), 0);
    let event_liabilities = System::events()
      .iter()
      .filter_map(|record| match &record.event {
        RuntimeEvent::Staking(crate::Event::NativeSecurityRewardClaimed {
          outstanding_liability,
          ..
        }) => Some(*outstanding_liability),
        _ => None,
      })
      .collect::<Vec<_>>();
    assert_eq!(event_liabilities, vec![10, 0]);
    assert_eq!(
      Staking::native_security_reward_pot(0)
        .expect("pot retained")
        .claimed,
      10
    );
    assert_eq!(
      Staking::native_security_reward_pot(1)
        .expect("pot retained")
        .claimed,
      10
    );

    let duplicate =
      BoundedVec::<u32, MaxSecurityRewardClaimsPerCall>::try_from(vec![0, 0]).expect("batch fits");
    assert_noop!(
      Staking::claim_native_security_reward_batch(RuntimeOrigin::signed(2), duplicate),
      Error::<Test>::DuplicateSecurityRewardEpoch
    );
    assert!(Staking::native_security_reward_claimed(0, 2).is_none());
  });
}

#[test]
fn security_reward_claims_reject_open_zero_weight_zero_pot_and_expired_epochs() {
  new_test_ext().execute_with(|| {
    crate::NativeSecurityEpochSnapshots::<Test>::insert(0, empty_security_snapshot(0));
    crate::NativeSecurityRewardPots::<Test>::insert(
      0,
      crate::NativeSecurityRewardPot {
        total_reward_weight: 0,
        credited: 10,
        claimed: 0,
        status: crate::NativeSecurityRewardPotStatus::Finalized,
      },
    );
    crate::NativeSecurityRewardLiability::<Test>::put(10);
    let reward_account = Staking::native_security_reward_account();
    let _ =
      <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::deposit_creating(
        &reward_account,
        10,
      );
    assert_noop!(
      Staking::claim_native_security_reward(RuntimeOrigin::signed(1), 0),
      Error::<Test>::NoSecurityRewardClaimable
    );

    crate::NativeSecurityRewardPots::<Test>::mutate(0, |pot| {
      let pot = pot.as_mut().expect("pot exists");
      pot.total_reward_weight = 1;
      pot.credited = 0;
    });
    assert_noop!(
      Staking::claim_native_security_reward(RuntimeOrigin::signed(1), 0),
      Error::<Test>::NativeSecurityRewardZeroPot
    );
    crate::NativeSecurityRewardPots::<Test>::mutate(0, |pot| {
      let pot = pot.as_mut().expect("pot exists");
      pot.credited = 10;
      pot.status = crate::NativeSecurityRewardPotStatus::Open;
    });
    assert_noop!(
      Staking::claim_native_security_reward(RuntimeOrigin::signed(1), 0),
      Error::<Test>::NativeSecurityRewardPotNotFinalized
    );
    crate::NativeSecurityRewardPots::<Test>::mutate(0, |pot| {
      pot.as_mut().expect("pot exists").status = crate::NativeSecurityRewardPotStatus::Finalized;
    });
    set_security_epoch(SecurityRewardClaimHorizon::get() + 1);
    assert_noop!(
      Staking::claim_native_security_reward(RuntimeOrigin::signed(1), 0),
      Error::<Test>::NativeSecurityRewardEpochExpired
    );
    assert_eq!(Staking::native_security_reward_liability(), 10);
    assert_eq!(Balances::free_balance(reward_account), 10);
  });
}

#[test]
fn expiry_atomically_settles_in_trusted_mode_and_removes_state() {
  new_test_ext().execute_with(|| {
    let mut snapshot = empty_security_snapshot(0);
    for account in [1, 2, 3] {
      snapshot
        .participants
        .try_push(crate::NativeSecurityAccountSnapshot {
          account,
          conservative_native_value: 1,
          governance_coefficient: FixedU128::one(),
          reward_weight: 1,
        })
        .expect("participant fits");
    }
    snapshot.total_reward_weight = 3;
    crate::NativeSecurityEpochSnapshots::<Test>::insert(0, snapshot);
    crate::NativeSecurityRewardPots::<Test>::insert(
      0,
      crate::NativeSecurityRewardPot {
        total_reward_weight: 3,
        credited: 10,
        claimed: 9,
        status: crate::NativeSecurityRewardPotStatus::Finalized,
      },
    );
    for account in [1, 2, 3] {
      crate::NativeSecurityRewardClaims::<Test>::insert(0, account, ());
    }
    crate::NativeSecurityRewardLiability::<Test>::put(1);
    let reward_account = Staking::native_security_reward_account();
    let source = SecurityRewardFundingSource::get();
    let _ =
      <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::deposit_creating(
        &reward_account,
        2,
      );
    let source_before = Balances::free_balance(source);
    set_security_epoch(SecurityRewardClaimHorizon::get() + 1);
    set_native_security_mode(crate::NativeSecurityMode::TrustedSet);

    assert_ok!(Staking::expire_native_security_reward(
      RuntimeOrigin::signed(3),
      0
    ));
    assert_eq!(Balances::free_balance(source), source_before + 1);
    assert_eq!(Balances::free_balance(reward_account), 1);
    assert_eq!(Staking::native_security_reward_liability(), 0);
    assert!(Staking::native_security_epoch_snapshot(0).is_none());
    assert!(Staking::native_security_reward_pot(0).is_none());
    for account in [1, 2, 3] {
      assert!(Staking::native_security_reward_claimed(0, account).is_none());
    }
    assert_noop!(
      Staking::expire_native_security_reward(RuntimeOrigin::signed(3), 0),
      Error::<Test>::NativeSecurityEpochNotOpen
    );
    assert_eq!(Balances::free_balance(source), source_before + 1);
  });
}

#[test]
fn session_retention_settles_exactly_the_epoch_crossing_the_horizon() {
  new_test_ext().execute_with(|| {
    crate::NativeSecurityEpochSnapshots::<Test>::insert(0, empty_security_snapshot(0));
    crate::NativeSecurityRewardPots::<Test>::insert(
      0,
      crate::NativeSecurityRewardPot {
        total_reward_weight: 1,
        credited: 10,
        claimed: 0,
        status: crate::NativeSecurityRewardPotStatus::Finalized,
      },
    );
    crate::NativeSecurityRewardLiability::<Test>::put(10);
    let reward_account = Staking::native_security_reward_account();
    let _ =
      <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::deposit_creating(
        &reward_account,
        11,
      );
    set_native_security_mode(crate::NativeSecurityMode::TrustedSet);
    set_security_epoch(SecurityRewardClaimHorizon::get());
    assert_eq!(Staking::settle_due_native_security_reward(), Ok(None));
    assert!(Staking::native_security_reward_pot(0).is_some());

    set_security_epoch(SecurityRewardClaimHorizon::get() + 1);
    assert_eq!(Staking::settle_due_native_security_reward(), Ok(Some(0)));
    assert!(Staking::native_security_reward_pot(0).is_none());
    assert_eq!(Staking::native_security_reward_liability(), 0);
  });
}

#[test]
fn session_retention_runs_four_claim_horizons_without_external_cleanup() {
  new_test_ext().execute_with(|| {
    let horizon = SecurityRewardClaimHorizon::get();
    let last_epoch = horizon.saturating_add(1).saturating_mul(4);
    let source = SecurityRewardFundingSource::get();
    let source_initial = Balances::free_balance(source);
    let reward_account = Staking::native_security_reward_account();
    let _ =
      <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::deposit_creating(
        &reward_account,
        <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::minimum_balance(),
      );
    assert_ok!(Staking::open_native_security_epoch(0, &[]));

    for epoch in 0..=last_epoch {
      set_security_epoch(epoch);
      let source_before_retention = Balances::free_balance(source);
      let settled =
        Staking::settle_due_native_security_reward().expect("session retention must remain live");
      if let Some(settled_epoch) = settled {
        assert_eq!(settled_epoch, epoch - horizon - 1);
        assert_eq!(Balances::free_balance(source), source_before_retention + 1);
        assert_eq!(Staking::settle_due_native_security_reward(), Ok(None));
      }
      assert_ok!(Staking::activate_native_security_epoch(epoch));
      assert_ok!(Staking::fund_native_security_reward(
        RuntimeOrigin::root(),
        1,
      ));
      assert_ok!(Staking::open_native_security_epoch(epoch + 1, &[]));
      assert!(
        crate::NativeSecurityRewardPots::<Test>::iter().count()
          <= horizon.saturating_add(2) as usize
      );
      assert_eq!(
        Staking::native_security_reward_liability(),
        epoch.min(horizon).saturating_add(1) as Balance
      );
    }

    assert_eq!(
      Balances::free_balance(source),
      source_initial - Staking::native_security_reward_liability()
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(Staking::do_try_state());
  });
}

#[test]
fn retention_recovers_the_oldest_missed_epoch_before_a_newer_due_epoch() {
  new_test_ext().execute_with(|| {
    for epoch in [0, 1] {
      crate::NativeSecurityEpochSnapshots::<Test>::insert(epoch, empty_security_snapshot(epoch));
      crate::NativeSecurityRewardPots::<Test>::insert(
        epoch,
        crate::NativeSecurityRewardPot {
          total_reward_weight: 1,
          credited: 10,
          claimed: 0,
          status: crate::NativeSecurityRewardPotStatus::Finalized,
        },
      );
    }
    crate::NativeSecurityRewardLiability::<Test>::put(20);
    set_native_security_mode(crate::NativeSecurityMode::TrustedSet);
    set_security_epoch(SecurityRewardClaimHorizon::get() + 2);
    assert_noop!(
      Staking::settle_due_native_security_reward(),
      Error::<Test>::NativeSecurityRewardAccountingOverflow
    );
    assert!(Staking::native_security_reward_pot(0).is_some());
    assert!(Staking::native_security_reward_pot(1).is_some());

    let reward_account = Staking::native_security_reward_account();
    let _ =
      <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::deposit_creating(
        &reward_account,
        21,
      );
    assert_eq!(Staking::settle_due_native_security_reward(), Ok(Some(0)));
    assert!(Staking::native_security_reward_pot(0).is_none());
    assert!(Staking::native_security_reward_pot(1).is_some());
    set_security_epoch(SecurityRewardClaimHorizon::get() + 3);
    assert_eq!(Staking::settle_due_native_security_reward(), Ok(Some(1)));
    assert!(Staking::native_security_reward_pot(1).is_none());
    assert_eq!(Staking::native_security_reward_liability(), 0);
  });
}

#[test]
fn planning_rejects_a_full_retention_window_or_an_existing_plan() {
  new_test_ext().execute_with(|| {
    for epoch in 1..=SecurityRewardClaimHorizon::get() + 2 {
      crate::NativeSecurityEpochSnapshots::<Test>::insert(epoch, empty_security_snapshot(epoch));
      crate::NativeSecurityRewardPots::<Test>::insert(
        epoch,
        crate::NativeSecurityRewardPot {
          total_reward_weight: 0,
          credited: 0,
          claimed: 0,
          status: crate::NativeSecurityRewardPotStatus::Finalized,
        },
      );
    }
    assert_noop!(
      Staking::open_native_security_epoch(9, &[]),
      Error::<Test>::NativeSecurityRetentionBlocked
    );

    crate::NativeSecurityEpochSnapshots::<Test>::remove(SecurityRewardClaimHorizon::get() + 2);
    crate::NativeSecurityRewardPots::<Test>::remove(SecurityRewardClaimHorizon::get() + 2);
    crate::NativeSecurityRewardPots::<Test>::mutate(1, |pot| {
      pot.as_mut().expect("pot exists").status = crate::NativeSecurityRewardPotStatus::Planned;
    });
    assert_noop!(
      Staking::open_native_security_epoch(9, &[]),
      Error::<Test>::NativeSecurityRetentionBlocked
    );
    assert_ok!(Staking::cancel_native_security_epoch_plan(1));
    assert!(Staking::native_security_epoch_snapshot(1).is_none());
    assert!(Staking::native_security_reward_pot(1).is_none());
  });
}

#[test]
fn expiry_returns_unsolicited_excess_without_crediting_it_to_rewards() {
  new_test_ext().execute_with(|| {
    crate::NativeSecurityEpochSnapshots::<Test>::insert(0, empty_security_snapshot(0));
    crate::NativeSecurityRewardPots::<Test>::insert(
      0,
      crate::NativeSecurityRewardPot {
        total_reward_weight: 1,
        credited: 10,
        claimed: 0,
        status: crate::NativeSecurityRewardPotStatus::Finalized,
      },
    );
    crate::NativeSecurityRewardLiability::<Test>::put(10);
    let reward_account = Staking::native_security_reward_account();
    let source = SecurityRewardFundingSource::get();
    let _ =
      <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::deposit_creating(
        &reward_account,
        16,
      );
    let source_before = Balances::free_balance(source);
    set_security_epoch(SecurityRewardClaimHorizon::get() + 1);

    assert_ok!(Staking::expire_native_security_reward(
      RuntimeOrigin::signed(3),
      0
    ));
    assert_eq!(Balances::free_balance(source), source_before + 15);
    assert_eq!(Balances::free_balance(reward_account), 1);
    assert_eq!(Staking::native_security_reward_liability(), 0);
  });
}

#[test]
fn expiry_fails_before_horizon_or_when_reward_custody_is_below_liability() {
  new_test_ext().execute_with(|| {
    crate::NativeSecurityEpochSnapshots::<Test>::insert(0, empty_security_snapshot(0));
    crate::NativeSecurityRewardPots::<Test>::insert(
      0,
      crate::NativeSecurityRewardPot {
        total_reward_weight: 1,
        credited: 10,
        claimed: 0,
        status: crate::NativeSecurityRewardPotStatus::Finalized,
      },
    );
    crate::NativeSecurityRewardLiability::<Test>::put(10);
    let source_before = Balances::free_balance(SecurityRewardFundingSource::get());

    set_security_epoch(SecurityRewardClaimHorizon::get());
    assert_noop!(
      Staking::expire_native_security_reward(RuntimeOrigin::signed(3), 0),
      Error::<Test>::NativeSecurityRewardExpiryInvalid
    );
    set_security_epoch(SecurityRewardClaimHorizon::get() + 1);
    assert_noop!(
      Staking::expire_native_security_reward(RuntimeOrigin::signed(3), 0),
      Error::<Test>::NativeSecurityRewardAccountingOverflow
    );
    assert_eq!(Staking::native_security_reward_liability(), 10);
    assert_eq!(
      Balances::free_balance(SecurityRewardFundingSource::get()),
      source_before
    );
    assert_eq!(
      Staking::native_security_reward_pot(0)
        .expect("pot retained")
        .status,
      crate::NativeSecurityRewardPotStatus::Finalized
    );
  });
}

#[test]
fn certified_security_reward_funding_creates_exact_pot_and_liability() {
  new_test_ext().execute_with(|| {
    crate::ActiveNativeSecurityEpochSnapshot::<Test>::put(empty_security_snapshot(0));
    crate::NativeSecurityEpochSnapshots::<Test>::insert(0, empty_security_snapshot(0));
    crate::NativeSecurityRewardPots::<Test>::insert(
      0,
      crate::NativeSecurityRewardPot {
        total_reward_weight: 0,
        credited: 0,
        claimed: 0,
        status: crate::NativeSecurityRewardPotStatus::Open,
      },
    );
    let source_before = Balances::free_balance(SecurityRewardFundingSource::get());
    let reward_account = Staking::native_security_reward_account();

    assert_ok!(Staking::fund_native_security_reward(
      RuntimeOrigin::root(),
      40,
    ));
    assert_ok!(Staking::fund_native_security_reward(
      RuntimeOrigin::root(),
      10,
    ));

    assert_eq!(
      Balances::free_balance(SecurityRewardFundingSource::get()),
      source_before - 50
    );
    assert_eq!(Balances::free_balance(reward_account), 50);
    assert_eq!(Staking::native_security_reward_liability(), 50);
    let pot = Staking::native_security_reward_pot(0).expect("current pot must remain open");
    assert_eq!(pot.credited, 50);
    assert_eq!(pot.claimed, 0);
    assert_eq!(pot.status, crate::NativeSecurityRewardPotStatus::Open);
  });
}

#[test]
fn unsolicited_security_reward_balance_never_creates_accounting_state() {
  new_test_ext().execute_with(|| {
    let reward_account = Staking::native_security_reward_account();
    let _ =
      <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::deposit_creating(
        &reward_account,
        25,
      );

    assert_eq!(Balances::free_balance(reward_account), 25);
    assert_eq!(Staking::native_security_reward_liability(), 0);
    assert_eq!(Staking::native_security_reward_pot(0), None);
    assert!(Staking::native_security_epoch_snapshot(0).is_none());
  });
}

#[test]
fn certified_funding_bridge_accepts_only_configured_fee_sink_source() {
  new_test_ext().execute_with(|| {
    crate::ActiveNativeSecurityEpochSnapshot::<Test>::put(empty_security_snapshot(0));
    crate::NativeSecurityEpochSnapshots::<Test>::insert(0, empty_security_snapshot(0));
    crate::NativeSecurityRewardPots::<Test>::insert(
      0,
      crate::NativeSecurityRewardPot {
        total_reward_weight: 0,
        credited: 0,
        claimed: 0,
        status: crate::NativeSecurityRewardPotStatus::Open,
      },
    );

    assert_noop!(
      Staking::preflight_native_security_reward_funding(&1, 0, 10),
      Error::<Test>::NativeSecurityRewardFundingUnavailable
    );
    assert_noop!(
      Staking::certify_native_security_reward_funding(&1, 0, 10),
      Error::<Test>::NativeSecurityRewardFundingUnavailable
    );
    assert_ok!(Staking::preflight_native_security_reward_funding(
      &SecurityRewardFundingSource::get(),
      0,
      10,
    ));
    assert_eq!(Staking::native_security_reward_liability(), 0);
    assert_eq!(
      Staking::native_security_reward_pot(0)
        .expect("pot remains")
        .credited,
      0
    );
  });
}

#[test]
fn security_reward_funding_rejects_uncertified_or_incoherent_state_without_mutation() {
  new_test_ext().execute_with(|| {
    crate::ActiveNativeSecurityEpochSnapshot::<Test>::put(empty_security_snapshot(0));
    crate::NativeSecurityEpochSnapshots::<Test>::insert(0, empty_security_snapshot(0));
    crate::NativeSecurityRewardPots::<Test>::insert(
      0,
      crate::NativeSecurityRewardPot {
        total_reward_weight: 0,
        credited: 0,
        claimed: 0,
        status: crate::NativeSecurityRewardPotStatus::Open,
      },
    );
    let source_before = Balances::free_balance(SecurityRewardFundingSource::get());
    let reward_account = Staking::native_security_reward_account();

    assert_noop!(
      Staking::fund_native_security_reward(RuntimeOrigin::signed(1), 10),
      polkadot_sdk::sp_runtime::DispatchError::BadOrigin
    );
    set_security_epoch(1);
    assert_noop!(
      Staking::fund_native_security_reward(RuntimeOrigin::root(), 10),
      Error::<Test>::NativeSecurityEpochNotOpen
    );
    set_security_epoch(0);
    set_native_security_mode(crate::NativeSecurityMode::TrustedSet);
    assert_noop!(
      Staking::fund_native_security_reward(RuntimeOrigin::root(), 10),
      Error::<Test>::NativeSecurityModeInactive
    );

    assert_eq!(
      Balances::free_balance(SecurityRewardFundingSource::get()),
      source_before
    );
    assert_eq!(Balances::free_balance(reward_account), 0);
    assert_eq!(Staking::native_security_reward_liability(), 0);
    assert_eq!(
      Staking::native_security_reward_pot(0)
        .expect("pot remains")
        .credited,
      0
    );
  });
}

#[test]
fn planning_future_epoch_does_not_change_active_epoch_or_current_funding() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      0x7000_0001,
      1,
      true,
      1,
    ));
    assert_ok!(Staking::open_native_security_epoch(0, &[]));
    assert_ok!(Staking::activate_native_security_epoch(0));
    assert_ok!(Staking::open_native_security_epoch(1, &[]));

    assert_eq!(
      Staking::active_native_security_epoch_snapshot()
        .expect("current epoch remains active")
        .epoch,
      0
    );
    assert_eq!(
      Staking::native_security_reward_pot(0)
        .expect("current pot remains")
        .status,
      crate::NativeSecurityRewardPotStatus::Open
    );
    assert_eq!(
      Staking::native_security_reward_pot(1)
        .expect("future pot is retained")
        .status,
      crate::NativeSecurityRewardPotStatus::Planned
    );
    assert_ok!(Staking::fund_native_security_reward(
      RuntimeOrigin::root(),
      10,
    ));
    assert_eq!(
      Staking::native_security_reward_pot(0)
        .expect("pot remains")
        .credited,
      10
    );
    assert_eq!(
      Staking::native_security_reward_pot(1)
        .expect("pot remains")
        .credited,
      0
    );
  });
}

#[test]
fn planned_epoch_cannot_be_funded_before_session_activation() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      0x7000_0001,
      1,
      true,
      1,
    ));
    assert_ok!(Staking::open_native_security_epoch(1, &[]));
    set_security_epoch(1);

    assert_noop!(
      Staking::fund_native_security_reward(RuntimeOrigin::root(), 10),
      Error::<Test>::NativeSecurityEpochNotOpen
    );
    assert_eq!(Staking::native_security_reward_liability(), 0);
    assert_eq!(
      Staking::native_security_reward_pot(1)
        .expect("planned pot remains")
        .status,
      crate::NativeSecurityRewardPotStatus::Planned
    );
  });
}

#[test]
fn opening_next_security_epoch_finalizes_prior_reward_pot() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      0x7000_0001,
      1,
      true,
      1,
    ));
    assert_ok!(Staking::open_native_security_epoch(0, &[]));
    assert_ok!(Staking::activate_native_security_epoch(0));
    assert_ok!(Staking::open_native_security_epoch(1, &[]));
    set_security_epoch(1);
    assert_ok!(Staking::activate_native_security_epoch(1));

    assert_eq!(
      Staking::native_security_reward_pot(0)
        .expect("prior pot must remain retained")
        .status,
      crate::NativeSecurityRewardPotStatus::Finalized
    );
    assert_eq!(
      Staking::native_security_reward_pot(1)
        .expect("next pot must be open")
        .status,
      crate::NativeSecurityRewardPotStatus::Open
    );
  });
}

#[test]
fn failed_native_security_epoch_open_preserves_prior_snapshot() {
  new_test_ext().execute_with(|| {
    let prior = empty_security_snapshot(6);
    crate::ActiveNativeSecurityEpochSnapshot::<Test>::put(prior);
    set_native_security_mode(crate::NativeSecurityMode::TrustedSet);

    assert_noop!(
      Staking::open_native_security_epoch(7, &[]),
      Error::<Test>::NativeSecurityModeInactive
    );
    let retained = Staking::active_native_security_epoch_snapshot()
      .expect("failed opening must preserve the prior snapshot");
    assert_eq!(retained.epoch, 6);
    assert!(retained.participants.is_empty());
    assert!(retained.eligible_operators.is_empty());
    assert_eq!(retained.total_reward_weight, 0);
  });
}

#[test]
fn invalid_operator_snapshot_open_preserves_prior_snapshot() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    crate::OperatorNativeLpLocked::<Test>::insert(55, 10);
    crate::ActiveNativeSecurityEpochSnapshot::<Test>::put(empty_security_snapshot(6));

    assert_noop!(
      Staking::open_native_security_epoch(7, &[55]),
      Error::<Test>::InvalidNativeOperatorTarget
    );
    assert_eq!(
      Staking::active_native_security_epoch_snapshot()
        .expect("failed opening must preserve the prior snapshot")
        .epoch,
      6
    );
  });
}

#[test]
fn lock_native_lp_for_collator_moves_lp_into_lock_account() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 100));
    let lock_account = Staking::native_lp_lock_account();
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      40,
      99,
    ));
    assert_eq!(<Assets as Inspect<AccountId>>::balance(LP_ASSET, &1), 60);
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(LP_ASSET, &lock_account),
      40
    );
    assert_eq!(
      NativeLpLocks::<Test>::get(1, 99)
        .expect("lock must exist")
        .amount,
      40
    );
    assert_eq!(OperatorNativeLpLocked::<Test>::get(99), 40);
    assert_eq!(Staking::account_native_lp_locked(1), 40);
    assert_eq!(Staking::native_locked_lp_position(1).collator_locked_lp, 40);
    assert_eq!(Staking::total_native_lp_locked(), 40);
    assert_eq!(Staking::native_security_participants().as_slice(), &[1]);
    assert_eq!(Staking::native_nomination_operators(1).as_slice(), &[99]);
    System::assert_last_event(RuntimeEvent::Staking(Event::NativeLpLocked {
      account: 1,
      operator: 99,
      lp_asset_id: LP_ASSET,
      amount: 40,
      total_locked: 40,
    }));
  });
}

#[test]
fn lock_native_lp_rejects_invalid_lp_asset() {
  new_test_ext().execute_with(|| {
    assert_noop!(
      Staking::lock_native_lp_for_collator(RuntimeOrigin::signed(1), 2, 10, 99),
      Error::<Test>::InvalidNativeLpAsset
    );
  });
}

#[test]
fn widened_mul_div_reward_weight_and_unlock_deadline_fail_closed_at_boundaries() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_eq!(
      Staking::mul_div_floor(u128::MAX, u128::MAX, u128::MAX),
      Some(u128::MAX)
    );
    assert_eq!(Staking::mul_div_floor(u128::MAX, u128::MAX, 1), None);
    assert_eq!(Staking::mul_div_floor(1, 1, 0), None);
    assert_eq!(
      Staking::reward_weight_from_snapshot(u128::MAX, FixedU128::one(),)
        .expect("identity coefficient remains exact"),
      u128::MAX,
    );
    assert!(
      Staking::reward_weight_from_snapshot(u128::MAX, FixedU128::from_inner(u128::MAX),).is_err()
    );

    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 100));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      40,
      99,
    ));
    System::set_block_number(u64::MAX);
    assert_noop!(
      Staking::request_unlock_native_lp(RuntimeOrigin::signed(1), 99, 15),
      polkadot_sdk::sp_runtime::ArithmeticError::Overflow,
    );
    assert_eq!(
      NativeLpLocks::<Test>::get(1, 99)
        .expect("overflow preserves the live lock")
        .amount,
      40,
    );
    assert!(PendingNativeLpUnlocks::<Test>::get(1, 99).is_none());
  });
}

#[test]
fn unlock_after_planning_preserves_planned_rights_and_changes_only_later_epoch() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 100));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      40,
      99,
    ));
    assert_ok!(Staking::open_native_security_epoch(0, &[99]));
    assert_ok!(Staking::activate_native_security_epoch(0));
    assert_ok!(Staking::open_native_security_epoch(1, &[99]));
    let planned_before = Staking::native_security_epoch_snapshot(1)
      .expect("next epoch rights must be frozen before unlock");

    assert_ok!(Staking::request_unlock_native_lp(
      RuntimeOrigin::signed(1),
      99,
      40,
    ));
    let planned_after = Staking::native_security_epoch_snapshot(1)
      .expect("unlock must preserve already planned rights");
    assert_eq!(planned_after.participants, planned_before.participants);
    assert_eq!(
      planned_after.total_reward_weight,
      planned_before.total_reward_weight
    );
    assert_eq!(Staking::operator_native_lp_locked(99), 0);

    set_security_epoch(1);
    assert_ok!(Staking::activate_native_security_epoch(1));
    assert_eq!(
      Staking::active_native_security_epoch_snapshot()
        .expect("planned rights become active unchanged")
        .participants,
      planned_before.participants
    );
    assert_ok!(Staking::open_native_security_epoch(2, &[]));
    assert!(
      Staking::native_security_epoch_snapshot(2)
        .expect("later epoch must reflect immediate backing removal")
        .participants
        .is_empty()
    );
  });
}

#[test]
fn native_lp_unlock_lifecycle_releases_after_delay() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 100));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      40,
      99,
    ));
    assert_ok!(Staking::request_unlock_native_lp(
      RuntimeOrigin::signed(1),
      99,
      15,
    ));
    let pending = PendingNativeLpUnlocks::<Test>::get(1, 99).expect("pending unlock must exist");
    assert_eq!(pending.amount, 15);
    assert_eq!(pending.unlock_block, 4);
    assert_eq!(
      NativeLpLocks::<Test>::get(1, 99)
        .expect("lock remains")
        .amount,
      25
    );
    assert_eq!(OperatorNativeLpLocked::<Test>::get(99), 25);
    assert_eq!(Staking::account_native_lp_locked(1), 25);
    assert_eq!(Staking::native_locked_lp_position(1).collator_locked_lp, 25);
    assert_eq!(Staking::total_native_lp_locked(), 25);
    assert_noop!(
      Staking::withdraw_unlocked_native_lp(RuntimeOrigin::signed(1), 99),
      Error::<Test>::NativeLpUnlockNotReady
    );
    advance_to_block(4);
    assert_ok!(Staking::withdraw_unlocked_native_lp(
      RuntimeOrigin::signed(1),
      99,
    ));
    assert!(PendingNativeLpUnlocks::<Test>::get(1, 99).is_none());
    assert_eq!(Staking::native_security_participants().as_slice(), &[1]);
    assert_eq!(Staking::native_nomination_operators(1).as_slice(), &[99]);
    assert_eq!(<Assets as Inspect<AccountId>>::balance(LP_ASSET, &1), 75);
  });
}

#[test]
fn repeated_unlock_requests_accumulate_custody_and_extend_maturity() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 100));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      40,
      99,
    ));
    assert_ok!(Staking::request_unlock_native_lp(
      RuntimeOrigin::signed(1),
      99,
      10,
    ));
    advance_to_block(2);
    assert_ok!(Staking::request_unlock_native_lp(
      RuntimeOrigin::signed(1),
      99,
      15,
    ));

    let pending = Staking::pending_native_lp_unlock(1, 99).expect("pending unlock retained");
    assert_eq!(pending.amount, 25);
    assert_eq!(pending.unlock_block, 5);
    assert_eq!(
      Staking::native_lp_lock(1, 99)
        .expect("active remainder")
        .amount,
      15
    );
    assert_eq!(Staking::operator_native_lp_locked(99), 15);
    advance_to_block(4);
    assert_noop!(
      Staking::withdraw_unlocked_native_lp(RuntimeOrigin::signed(1), 99),
      Error::<Test>::NativeLpUnlockNotReady
    );
    advance_to_block(5);
    assert_ok!(Staking::withdraw_unlocked_native_lp(
      RuntimeOrigin::signed(1),
      99,
    ));
    assert_eq!(<Assets as Inspect<AccountId>>::balance(LP_ASSET, &1), 85);
  });
}

#[test]
fn full_unlock_then_new_nomination_keeps_pending_custody_separate() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 100));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      40,
      99,
    ));
    assert_ok!(Staking::request_unlock_native_lp(
      RuntimeOrigin::signed(1),
      99,
      40,
    ));
    assert!(Staking::native_lp_lock(1, 99).is_none());
    assert!(Staking::native_security_participants().is_empty());

    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      10,
      99,
    ));
    assert_eq!(
      Staking::native_lp_lock(1, 99)
        .expect("new active position")
        .amount,
      10
    );
    assert_eq!(
      Staking::pending_native_lp_unlock(1, 99)
        .expect("old custody remains")
        .amount,
      40
    );
    assert_eq!(Staking::total_native_lp_locked(), 10);
    advance_to_block(4);
    assert_ok!(Staking::withdraw_unlocked_native_lp(
      RuntimeOrigin::signed(1),
      99,
    ));
    assert_eq!(
      Staking::native_lp_lock(1, 99)
        .expect("new lock survives withdrawal")
        .amount,
      10
    );
    assert_eq!(Staking::operator_native_lp_locked(99), 10);
    assert_eq!(<Assets as Inspect<AccountId>>::balance(LP_ASSET, &1), 90);
  });
}

#[test]
fn trusted_mode_contracts_open_and_planned_state_without_losing_liability() {
  new_test_ext().execute_with(|| {
    let reward_account = Staking::native_security_reward_account();
    let _ =
      <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::deposit_creating(
        &reward_account,
        <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::minimum_balance(),
      );
    assert_ok!(Staking::open_native_security_epoch(0, &[]));
    assert_ok!(Staking::activate_native_security_epoch(0));
    assert_ok!(Staking::fund_native_security_reward(
      RuntimeOrigin::root(),
      10,
    ));
    assert_ok!(Staking::open_native_security_epoch(1, &[]));
    set_security_epoch(1);
    set_native_security_mode(crate::NativeSecurityMode::TrustedSet);

    assert_ok!(Staking::contract_native_security_obligations_for_trusted_mode());
    assert!(Staking::active_native_security_epoch_snapshot().is_none());
    assert_eq!(
      Staking::native_security_reward_pot(0)
        .expect("open obligation becomes finalized")
        .status,
      crate::NativeSecurityRewardPotStatus::Finalized
    );
    assert!(Staking::native_security_reward_pot(1).is_none());
    assert_eq!(Staking::native_security_reward_liability(), 10);

    set_security_epoch(SecurityRewardClaimHorizon::get() + 1);
    assert_eq!(Staking::settle_due_native_security_reward(), Ok(Some(0)));
    assert_eq!(Staking::native_security_reward_liability(), 0);
  });
}

#[test]
fn lp_backed_to_trusted_transition_preserves_every_retained_obligation() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 40));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      40,
      99,
    ));
    assert_ok!(Staking::request_unlock_native_lp(
      RuntimeOrigin::signed(1),
      99,
      10,
    ));

    let mut finalized = empty_security_snapshot(0);
    for (account, reward_weight) in [(1, 40), (2, 60)] {
      finalized
        .participants
        .try_push(crate::NativeSecurityAccountSnapshot {
          account,
          conservative_native_value: reward_weight,
          governance_coefficient: FixedU128::one(),
          reward_weight,
        })
        .expect("participant fits");
    }
    finalized.total_reward_weight = 100;
    crate::NativeSecurityEpochSnapshots::<Test>::insert(0, finalized);
    crate::NativeSecurityRewardPots::<Test>::insert(
      0,
      crate::NativeSecurityRewardPot {
        total_reward_weight: 100,
        credited: 101,
        claimed: 0,
        status: crate::NativeSecurityRewardPotStatus::Finalized,
      },
    );
    let active = empty_security_snapshot(1);
    crate::ActiveNativeSecurityEpochSnapshot::<Test>::put(&active);
    crate::NativeSecurityEpochSnapshots::<Test>::insert(1, active);
    crate::NativeSecurityRewardPots::<Test>::insert(
      1,
      crate::NativeSecurityRewardPot {
        total_reward_weight: 0,
        credited: 50,
        claimed: 0,
        status: crate::NativeSecurityRewardPotStatus::Open,
      },
    );
    crate::NativeSecurityEpochSnapshots::<Test>::insert(2, empty_security_snapshot(2));
    crate::NativeSecurityRewardPots::<Test>::insert(
      2,
      crate::NativeSecurityRewardPot {
        total_reward_weight: 0,
        credited: 0,
        claimed: 0,
        status: crate::NativeSecurityRewardPotStatus::Planned,
      },
    );
    crate::NativeSecurityRewardLiability::<Test>::put(151);
    let reward_account = Staking::native_security_reward_account();
    let _ =
      <Balances as polkadot_sdk::frame_support::traits::Currency<AccountId>>::deposit_creating(
        &reward_account,
        159,
      );
    set_security_epoch(2);
    assert_ok!(Staking::claim_native_security_reward(
      RuntimeOrigin::signed(1),
      0,
    ));
    assert_eq!(Staking::native_security_reward_liability(), 111);

    set_native_security_mode(crate::NativeSecurityMode::TrustedSet);
    assert_ok!(Staking::contract_native_security_obligations_for_trusted_mode());
    assert!(Staking::active_native_security_epoch_snapshot().is_none());
    assert!(Staking::native_security_reward_pot(2).is_none());
    assert_eq!(
      Staking::native_security_reward_pot(1)
        .expect("open pot becomes finalized")
        .status,
      crate::NativeSecurityRewardPotStatus::Finalized
    );
    assert_eq!(Staking::native_security_reward_liability(), 111);
    assert_noop!(
      Staking::claim_and_compound_native_security_reward(RuntimeOrigin::signed(2), 0, 99, 1),
      Error::<Test>::NativeSecurityModeInactive
    );
    assert_ok!(Staking::claim_native_security_reward(
      RuntimeOrigin::signed(2),
      0,
    ));
    assert_eq!(Staking::native_security_reward_liability(), 51);

    let unlock_block = Staking::pending_native_lp_unlock(1, 99)
      .expect("pending custody exit survives mode contraction")
      .unlock_block;
    System::set_block_number(unlock_block);
    assert_ok!(Staking::withdraw_unlocked_native_lp(
      RuntimeOrigin::signed(1),
      99,
    ));
    assert_eq!(Assets::balance(LP_ASSET, &1), 10);
    assert_eq!(
      Staking::native_lp_lock(1, 99)
        .expect("active lock remains")
        .amount,
      30
    );

    let source = SecurityRewardFundingSource::get();
    let source_before = Balances::free_balance(source);
    set_security_epoch(SecurityRewardClaimHorizon::get() + 1);
    assert_eq!(Staking::settle_due_native_security_reward(), Ok(Some(0)));
    assert_eq!(Balances::free_balance(source), source_before + 8);
    assert_eq!(Staking::native_security_reward_liability(), 50);
    set_security_epoch(SecurityRewardClaimHorizon::get() + 2);
    assert_eq!(Staking::settle_due_native_security_reward(), Ok(Some(1)));
    assert_eq!(Balances::free_balance(source), source_before + 58);
    assert_eq!(Staking::native_security_reward_liability(), 0);
  });
}

#[test]
fn trusted_mode_stops_new_nomination_but_preserves_unlock_and_withdrawal() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 100));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      40,
      99,
    ));
    set_native_security_mode(crate::NativeSecurityMode::TrustedSet);
    assert_noop!(
      Staking::lock_native_lp_for_collator(RuntimeOrigin::signed(1), LP_ASSET, 10, 99),
      Error::<Test>::NativeSecurityModeInactive
    );
    assert_noop!(
      Staking::redelegate_native_lp(RuntimeOrigin::signed(1), 99, 2, 10),
      Error::<Test>::NativeSecurityModeInactive
    );
    assert_ok!(Staking::request_unlock_native_lp(
      RuntimeOrigin::signed(1),
      99,
      40,
    ));
    assert_eq!(Staking::operator_native_lp_locked(99), 0);
    assert_eq!(Staking::native_locked_lp_position(1).collator_locked_lp, 0);
    assert_eq!(Staking::total_native_lp_locked(), 0);
    assert!(Staking::native_security_participants().is_empty());
    assert!(Staking::native_nomination_operators(1).is_empty());
    advance_to_block(4);
    assert_ok!(Staking::withdraw_unlocked_native_lp(
      RuntimeOrigin::signed(1),
      99,
    ));
    assert_eq!(<Assets as Inspect<AccountId>>::balance(LP_ASSET, &1), 100);
  });
}

#[test]
fn native_lp_unlock_respects_account_governance_lock_horizon() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 100));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      40,
      99,
    ));
    crate::mock::set_native_governance_lock(1, 4);
    assert_noop!(
      Staking::request_unlock_native_lp(RuntimeOrigin::signed(1), 99, 15),
      Error::<Test>::NativeGovernanceLockActive
    );
    assert_eq!(Staking::account_native_lp_locked(1), 40);
    advance_to_block(4);
    assert_ok!(Staking::request_unlock_native_lp(
      RuntimeOrigin::signed(1),
      99,
      15,
    ));
    assert_eq!(Staking::account_native_lp_locked(1), 25);
  });
}

#[test]
fn native_governance_lp_lock_unlock_lifecycle_updates_vote_power_aggregates() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 100));
    assert_ok!(Staking::lock_native_lp_for_governance(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      40,
    ));
    let lock_account = Staking::native_lp_lock_account();
    assert_eq!(<Assets as Inspect<AccountId>>::balance(LP_ASSET, &1), 60);
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(LP_ASSET, &lock_account),
      40
    );
    assert_eq!(
      Staking::native_governance_lp_lock(1)
        .expect("governance lock must exist")
        .amount,
      40
    );
    assert_eq!(Staking::account_native_lp_locked(1), 40);
    assert_eq!(Staking::native_locked_lp_position(1).collator_locked_lp, 0);
    assert_eq!(Staking::total_native_lp_locked(), 40);
    assert_ok!(Staking::request_unlock_native_lp_for_governance(
      RuntimeOrigin::signed(1),
      15,
    ));
    assert_eq!(Staking::account_native_lp_locked(1), 25);
    assert_eq!(Staking::native_locked_lp_position(1).collator_locked_lp, 0);
    assert_eq!(Staking::total_native_lp_locked(), 25);
    assert_eq!(
      Staking::pending_native_governance_lp_unlock(1)
        .expect("pending unlock must exist")
        .amount,
      15
    );
    advance_to_block(4);
    assert_ok!(Staking::withdraw_unlocked_native_lp_for_governance(
      RuntimeOrigin::signed(1),
    ));
    assert_eq!(<Assets as Inspect<AccountId>>::balance(LP_ASSET, &1), 75);
  });
}

#[test]
fn native_governance_lp_unlock_respects_account_governance_lock_horizon() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 100));
    assert_ok!(Staking::lock_native_lp_for_governance(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      40,
    ));
    crate::mock::set_native_governance_lock(1, 4);
    assert_noop!(
      Staking::request_unlock_native_lp_for_governance(RuntimeOrigin::signed(1), 15),
      Error::<Test>::NativeGovernanceLockActive
    );
    assert_eq!(Staking::account_native_lp_locked(1), 40);
    advance_to_block(4);
    assert_ok!(Staking::request_unlock_native_lp_for_governance(
      RuntimeOrigin::signed(1),
      15,
    ));
    assert_eq!(Staking::account_native_lp_locked(1), 25);
  });
}

#[test]
fn native_governance_asset_lock_unlock_lifecycle_updates_aggregates() {
  new_test_ext().execute_with(|| {
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(1, &1, 100));
    let balance_before = <Assets as Inspect<AccountId>>::balance(1, &1);
    assert_ok!(Staking::lock_native_asset_for_governance(
      RuntimeOrigin::signed(1),
      1,
      40,
    ));
    let lock_account = Staking::native_lp_lock_account();
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(1, &lock_account),
      40
    );
    assert_eq!(Staking::native_governance_asset_locked(1, 1), 40);
    assert_eq!(Staking::total_native_governance_asset_locked(1), 40);
    assert_ok!(Staking::request_unlock_native_asset_for_governance(
      RuntimeOrigin::signed(1),
      1,
      15,
    ));
    assert_eq!(Staking::native_governance_asset_locked(1, 1), 25);
    assert_eq!(Staking::total_native_governance_asset_locked(1), 25);
    assert_eq!(
      Staking::pending_native_governance_asset_unlock(1, 1)
        .expect("pending unlock must exist")
        .amount,
      15
    );
    advance_to_block(4);
    assert_ok!(Staking::withdraw_unlocked_native_asset_for_governance(
      RuntimeOrigin::signed(1),
      1,
    ));
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(1, &1),
      balance_before - 25
    );
  });
}

#[test]
fn native_governance_asset_unlock_respects_account_governance_lock_horizon() {
  new_test_ext().execute_with(|| {
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(1, &1, 100));
    assert_ok!(Staking::lock_native_asset_for_governance(
      RuntimeOrigin::signed(1),
      1,
      40,
    ));
    crate::mock::set_native_governance_lock(1, 4);
    assert_noop!(
      Staking::request_unlock_native_asset_for_governance(RuntimeOrigin::signed(1), 1, 15),
      Error::<Test>::NativeGovernanceLockActive
    );
    assert_eq!(Staking::native_governance_asset_locked(1, 1), 40);
    advance_to_block(4);
    assert_ok!(Staking::request_unlock_native_asset_for_governance(
      RuntimeOrigin::signed(1),
      1,
      15,
    ));
    assert_eq!(Staking::native_governance_asset_locked(1, 1), 25);
  });
}

#[test]
fn redelegation_after_planning_preserves_frozen_epoch_and_changes_later_epoch() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 100));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      40,
      99,
    ));
    assert_ok!(Staking::open_native_security_epoch(0, &[99]));
    assert_ok!(Staking::activate_native_security_epoch(0));
    assert_ok!(Staking::open_native_security_epoch(1, &[99]));
    let planned_before =
      Staking::native_security_epoch_snapshot(1).expect("next epoch must freeze source operator");

    assert_ok!(Staking::redelegate_native_lp(
      RuntimeOrigin::signed(1),
      99,
      100,
      40,
    ));
    assert_eq!(
      Staking::native_security_epoch_snapshot(1)
        .expect("redelegation cannot rewrite planned epoch")
        .eligible_operators,
      planned_before.eligible_operators
    );
    assert_eq!(Staking::operator_native_lp_locked(99), 0);
    assert_eq!(Staking::operator_native_lp_locked(100), 40);
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(LP_ASSET, &Staking::native_lp_lock_account(),),
      40
    );

    set_security_epoch(1);
    assert_ok!(Staking::activate_native_security_epoch(1));
    assert_ok!(Staking::open_native_security_epoch(2, &[100]));
    let later = Staking::native_security_epoch_snapshot(2)
      .expect("later epoch must use redelegated operator");
    assert_eq!(later.eligible_operators.len(), 1);
    assert_eq!(later.eligible_operators[0].operator, 100);
    assert_eq!(later.participants[0].conservative_native_value, 40);
    assert_eq!(Staking::account_native_lp_locked(1), 40);
    assert_eq!(Staking::native_locked_lp_position(1).collator_locked_lp, 40);
    assert_eq!(Staking::total_native_lp_locked(), 40);
  });
}

#[test]
fn native_lp_redelegate_moves_backing_between_operators() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(LP_ASSET, &1, 100));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      40,
      99,
    ));
    assert_ok!(Staking::redelegate_native_lp(
      RuntimeOrigin::signed(1),
      99,
      100,
      15,
    ));
    assert_eq!(
      NativeLpLocks::<Test>::get(1, 99)
        .expect("source lock remains")
        .amount,
      25
    );
    assert_eq!(
      NativeLpLocks::<Test>::get(1, 100)
        .expect("target lock exists")
        .amount,
      15
    );
    assert_eq!(OperatorNativeLpLocked::<Test>::get(99), 25);
    assert_eq!(OperatorNativeLpLocked::<Test>::get(100), 15);
    assert_eq!(Staking::account_native_lp_locked(1), 40);
    assert_eq!(Staking::native_locked_lp_position(1).collator_locked_lp, 40);
    assert_eq!(Staking::total_native_lp_locked(), 40);
    assert_eq!(Staking::native_security_participants().as_slice(), &[1]);
    assert_eq!(
      Staking::native_nomination_operators(1).as_slice(),
      &[99, 100]
    );
    assert_ok!(Staking::redelegate_native_lp(
      RuntimeOrigin::signed(1),
      99,
      100,
      25,
    ));
    assert_eq!(Staking::native_nomination_operators(1).as_slice(), &[100]);
  });
}

#[test]
fn native_nomination_indexes_enforce_bounds_before_transfer() {
  const LP_ASSET: AssetId = 0x7000_0001;
  new_test_ext().execute_with(|| {
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      LP_ASSET,
      1,
      true,
      1,
    ));
    for account in 1..=4 {
      assert_ok!(<Assets as Mutate<AccountId>>::mint_into(
        LP_ASSET, &account, 30
      ));
    }
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      10,
      99,
    ));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(1),
      LP_ASSET,
      10,
      100,
    ));
    let balance_before = <Assets as Inspect<AccountId>>::balance(LP_ASSET, &1);
    assert_noop!(
      Staking::lock_native_lp_for_collator(RuntimeOrigin::signed(1), LP_ASSET, 1, 2),
      Error::<Test>::NativeNominationLimitReached
    );
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(LP_ASSET, &1),
      balance_before
    );
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(2),
      LP_ASSET,
      10,
      99,
    ));
    assert_ok!(Staking::lock_native_lp_for_collator(
      RuntimeOrigin::signed(3),
      LP_ASSET,
      10,
      99,
    ));
    let fourth_before = <Assets as Inspect<AccountId>>::balance(LP_ASSET, &4);
    assert_noop!(
      Staking::lock_native_lp_for_collator(RuntimeOrigin::signed(4), LP_ASSET, 10, 99),
      Error::<Test>::NativeSecurityParticipantLimitReached
    );
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(LP_ASSET, &4),
      fourth_before
    );
  });
}

#[test]
fn staked_asset_id_resolution_matches_current_namespace_contract() {
  const TYPE_FOREIGN: AssetId = 0xF000_0000;
  const TYPE_STAKED: AssetId = 0x5000_0000;
  const TYPE_STAKED_FOREIGN: AssetId = 0x6000_0000;
  new_test_ext().execute_with(|| {
    assert_eq!(Staking::staked_asset_id(1), Some(TYPE_STAKED));
    assert_eq!(Staking::staked_asset_id(2), Some(TYPE_STAKED | 2));
    assert_eq!(
      Staking::staked_asset_id(TYPE_FOREIGN | 2),
      Some(TYPE_STAKED_FOREIGN | 2)
    );
  });
}

#[test]
fn live_native_staked_receipt_balance_reads_current_receipt_asset_balance() {
  const TYPE_STAKED: AssetId = 0x5000_0000;
  new_test_ext().execute_with(|| {
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      TYPE_STAKED,
      1,
      true,
      1
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(
      TYPE_STAKED,
      &1,
      42
    ));
    assert_eq!(Staking::live_native_staked_receipt_balance(&1), Some(42));
    assert_eq!(Staking::live_native_staked_receipt_balance(&2), Some(0));
  });
}

#[test]
fn staked_receipt_value_tracks_pool_share_price_from_live_receipt_balance() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    let pool_account = Staking::pool_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &pool_account,
      100,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_ok!(Staking::sync_pool(RuntimeOrigin::signed(99), 2));
    assert_eq!(Staking::staked_receipt_value(2, &1), Some(200));
  });
}

#[test]
fn query_surface_prefers_receipt_balances_once_receipts_exist() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      TYPE_STAKED_LOCAL,
      &1,
      &2,
      60,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_eq!(
      Staking::effective_share_balance_for_queries(2, &1),
      Some(40)
    );
    assert_eq!(Staking::stake_fraction(2, &1), Some((40, 100)));
    assert_eq!(Staking::stake_value(2, &1), Some(40));
  });
}

#[test]
fn transferred_receipt_holder_can_unstake_via_receipt_balance() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(TYPE_STAKED_LOCAL, &1),
      100
    );
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      TYPE_STAKED_LOCAL,
      &1,
      &2,
      30,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    let before = <Assets as Inspect<AccountId>>::balance(2, &2);
    assert_ok!(Staking::unstake(RuntimeOrigin::signed(2), 2, 30));
    let after = <Assets as Inspect<AccountId>>::balance(2, &2);
    assert_eq!(after - before, 30);
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(TYPE_STAKED_LOCAL, &2),
      0
    );
    assert_eq!(Staking::stake_value(2, &1), Some(70));
    assert_eq!(Staking::stake_value(2, &2), None);
  });
}

#[test]
fn register_asset_creates_empty_pool() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    let pool = Staking::pool(2).expect("pool must exist");
    assert_eq!(pool.total_shares, 0);
    assert_eq!(pool.accounted_balance, 0);
    assert!(<Assets as Inspect<AccountId>>::asset_exists(
      TYPE_STAKED_LOCAL
    ));
    assert_eq!(
      Staking::base_asset_for_staked_asset(TYPE_STAKED_LOCAL),
      Some(2)
    );
    assert_eq!(
      Staking::live_base_asset_for_staked_asset(TYPE_STAKED_LOCAL),
      Some(2)
    );
    assert_eq!(
      <Assets as MetadataInspect<AccountId>>::name(TYPE_STAKED_LOCAL),
      b"Staked Asset 2".to_vec()
    );
    assert_eq!(
      <Assets as MetadataInspect<AccountId>>::symbol(TYPE_STAKED_LOCAL),
      b"stASSET2".to_vec()
    );
    assert_eq!(
      <Assets as MetadataInspect<AccountId>>::decimals(TYPE_STAKED_LOCAL),
      12
    );
    System::assert_last_event(RuntimeEvent::Staking(Event::StakingAssetRegistered {
      asset_id: 2,
      pool_account: Staking::pool_account_for(2),
    }));
  });
}

#[test]
fn register_asset_rejects_preexisting_staked_asset_id_collision() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      TYPE_STAKED_LOCAL,
      1,
      true,
      1
    ));
    assert_noop!(
      Staking::register_staking_asset(RuntimeOrigin::root(), 2),
      Error::<Test>::StakedAssetIdCollision
    );
  });
}

#[test]
fn foreign_asset_uses_dedicated_receipt_namespace() {
  const TYPE_FOREIGN: AssetId = 0xF000_0000;
  const TYPE_STAKED_FOREIGN: AssetId = 0x6000_0000;
  let foreign_asset = TYPE_FOREIGN | 2;
  let foreign_receipt = TYPE_STAKED_FOREIGN | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Assets::force_create(
      RuntimeOrigin::root(),
      foreign_asset,
      1,
      true,
      1
    ));
    assert_ok!(<Assets as Mutate<AccountId>>::mint_into(
      foreign_asset,
      &1,
      500
    ));
    assert_ok!(Staking::register_staking_asset(
      RuntimeOrigin::root(),
      foreign_asset,
    ));
    assert!(<Assets as Inspect<AccountId>>::asset_exists(
      foreign_receipt
    ));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), foreign_asset, 200,));
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(foreign_receipt, &1),
      200
    );
    assert_eq!(Staking::stake_value(foreign_asset, &1), Some(200));
  });
}

#[test]
fn failed_receipt_mint_rolls_back_stake_collateral_and_pool_state() {
  const STAKED_ASSET: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Assets::force_asset_status(
      RuntimeOrigin::root(),
      STAKED_ASSET,
      1,
      1,
      1,
      1,
      1,
      true,
      true,
    ));
    let account_before = <Assets as Inspect<AccountId>>::balance(2, &1);
    let pool_account = Staking::pool_account_for(2);
    let pool_balance_before = <Assets as Inspect<AccountId>>::balance(2, &pool_account);
    let pool_before = Staking::pool(2).expect("pool exists");
    let events_before = System::event_count();

    assert_eq!(
      Staking::stake(RuntimeOrigin::signed(1), 2, 100),
      Err(polkadot_sdk::pallet_assets::Error::<Test>::AssetNotLive.into())
    );

    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(2, &1),
      account_before
    );
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(2, &pool_account),
      pool_balance_before
    );
    assert_eq!(Staking::pool(2), Some(pool_before));
    assert_eq!(<Assets as Inspect<AccountId>>::balance(STAKED_ASSET, &1), 0);
    assert_eq!(System::event_count(), events_before);
  });
}

#[test]
fn failed_backing_transfer_rolls_back_unstake_receipt_burn_and_pool_state() {
  const STAKED_ASSET: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(Assets::force_asset_status(
      RuntimeOrigin::root(),
      2,
      1,
      1,
      1,
      1,
      1,
      true,
      true,
    ));
    let receipt_before = <Assets as Inspect<AccountId>>::balance(STAKED_ASSET, &1);
    let account_before = <Assets as Inspect<AccountId>>::balance(2, &1);
    let pool_account = Staking::pool_account_for(2);
    let pool_balance_before = <Assets as Inspect<AccountId>>::balance(2, &pool_account);
    let pool_before = Staking::pool(2).expect("pool exists");
    let events_before = System::event_count();

    assert_eq!(
      Staking::unstake(RuntimeOrigin::signed(1), 2, 50),
      Err(polkadot_sdk::sp_runtime::DispatchError::Token(
        polkadot_sdk::sp_runtime::TokenError::Frozen
      ))
    );

    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(STAKED_ASSET, &1),
      receipt_before
    );
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(2, &1),
      account_before
    );
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(2, &pool_account),
      pool_balance_before
    );
    assert_eq!(Staking::pool(2), Some(pool_before));
    assert_eq!(System::event_count(), events_before);
  });
}

#[test]
fn first_stake_mints_equal_shares() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    let pool = Staking::pool(2).expect("pool must exist");
    assert_eq!(pool.total_shares, 100);
    assert_eq!(pool.accounted_balance, 100);
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(TYPE_STAKED_LOCAL, &1),
      100
    );
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(2, &Staking::pool_account_for(2)),
      100
    );
  });
}

#[test]
fn second_stake_into_non_empty_pool_mints_proportional_shares() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(2), 2, 50));
    let pool = Staking::pool(2).expect("pool must exist");
    assert_eq!(pool.total_shares, 150);
    assert_eq!(pool.accounted_balance, 150);
    assert_eq!(Staking::stake_value(2, &1), Some(100));
    assert_eq!(Staking::stake_value(2, &2), Some(50));
  });
}

#[test]
fn external_inflow_increases_all_holders_proportionally_after_sync_without_reward_state() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(2), 2, 100));
    let pool_account = Staking::pool_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &pool_account,
      100,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_ok!(Staking::sync_pool(RuntimeOrigin::signed(99), 2));
    let pool = Staking::pool(2).expect("pool must exist");
    assert_eq!(pool.total_shares, 200);
    assert_eq!(pool.accounted_balance, 300);
    assert_eq!(Staking::stake_value(2, &1), Some(150));
    assert_eq!(Staking::stake_value(2, &2), Some(150));
  });
}

#[test]
fn partial_unstake_burns_shares_and_returns_correct_underlying() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(2), 2, 100));
    let pool_account = Staking::pool_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &pool_account,
      100,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_ok!(Staking::sync_pool(RuntimeOrigin::signed(99), 2));
    let before = <Assets as Inspect<AccountId>>::balance(2, &1);
    assert_ok!(Staking::unstake(RuntimeOrigin::signed(1), 2, 50));
    let after = <Assets as Inspect<AccountId>>::balance(2, &1);
    assert_eq!(after - before, 75);
    let pool = Staking::pool(2).expect("pool must exist");
    assert_eq!(pool.total_shares, 150);
    assert_eq!(pool.accounted_balance, 225);
  });
}

#[test]
fn transferred_receipt_holder_can_unstake() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      TYPE_STAKED_LOCAL,
      &1,
      &2,
      40,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    let before = <Assets as Inspect<AccountId>>::balance(2, &2);
    assert_ok!(Staking::unstake(RuntimeOrigin::signed(2), 2, 40));
    let after = <Assets as Inspect<AccountId>>::balance(2, &2);
    assert_eq!(after - before, 40);
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(TYPE_STAKED_LOCAL, &2),
      0
    );
  });
}

#[test]
fn full_receipt_exit_clears_pool_totals() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(Staking::unstake(RuntimeOrigin::signed(1), 2, 100));
    let pool = Staking::pool(2).expect("pool must exist");
    assert_eq!(pool.total_shares, 0);
    assert_eq!(pool.accounted_balance, 0);
  });
}

#[test]
fn first_stake_rejects_unowned_prefunded_pool() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    let pool_account = Staking::pool_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &pool_account,
      100,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_noop!(
      Staking::stake(RuntimeOrigin::signed(1), 2, 100),
      Error::<Test>::PoolHasUnownedBalance
    );
  });
}

#[test]
fn recover_unowned_pool_transfers_balance_and_unblocks_first_stake() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    let pool_account = Staking::pool_account_for(2);
    let beneficiary_before = <Assets as Inspect<AccountId>>::balance(2, &99);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &pool_account,
      100,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_noop!(
      Staking::stake(RuntimeOrigin::signed(1), 2, 100),
      Error::<Test>::PoolHasUnownedBalance
    );
    assert_ok!(Staking::recover_unowned_pool(RuntimeOrigin::root(), 2, 99));
    let pool = Staking::pool(2).expect("pool must exist");
    assert_eq!(pool.total_shares, 0);
    assert_eq!(pool.accounted_balance, 0);
    assert_eq!(<Assets as Inspect<AccountId>>::balance(2, &pool_account), 0);
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(2, &99) - beneficiary_before,
      100
    );
    System::assert_has_event(RuntimeEvent::Staking(Event::UnownedPoolRecovered {
      asset_id: 2,
      beneficiary: 99,
      amount: 100,
    }));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_eq!(Staking::stake_value(2, &1), Some(100));
  });
}

#[test]
fn recover_unowned_pool_accepts_prefunding_after_full_receipt_exit() {
  const TYPE_STAKED_LOCAL: AssetId = 0x5000_0000 | 2;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      TYPE_STAKED_LOCAL,
      &1,
      &2,
      100,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Expendable,
    ));
    assert_ok!(Staking::unstake(RuntimeOrigin::signed(2), 2, 100));
    assert_eq!(Staking::pool(2).expect("pool must exist").total_shares, 0);
    let pool_account = Staking::pool_account_for(2);
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      2,
      &1,
      &pool_account,
      25,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    let beneficiary_before = <Assets as Inspect<AccountId>>::balance(2, &1);
    assert_ok!(Staking::recover_unowned_pool(RuntimeOrigin::root(), 2, 1));
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(2, &1) - beneficiary_before,
      25
    );
  });
}

#[test]
fn recover_unowned_pool_rejects_non_empty_pool() {
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 2));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 2, 100));
    assert_noop!(
      Staking::recover_unowned_pool(RuntimeOrigin::root(), 2, 99),
      Error::<Test>::PoolNotEmpty
    );
  });
}

#[test]
fn generic_stake_mints_liquid_native_receipt_without_binding() {
  const TYPE_STAKED: AssetId = 0x5000_0000;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 1, 100));
    assert_eq!(
      <Assets as Inspect<AccountId>>::balance(TYPE_STAKED, &1),
      100
    );
  });
}

#[test]
fn generic_stake_value_tracks_transferable_native_receipts() {
  const TYPE_STAKED: AssetId = 0x5000_0000;
  new_test_ext().execute_with(|| {
    assert_ok!(Staking::register_staking_asset(RuntimeOrigin::root(), 1));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(1), 1, 100));
    assert_ok!(Staking::stake(RuntimeOrigin::signed(2), 1, 50));
    assert_ok!(<Assets as Mutate<AccountId>>::transfer(
      TYPE_STAKED,
      &2,
      &3,
      20,
      polkadot_sdk::frame_support::traits::tokens::Preservation::Protect,
    ));
    assert_eq!(Staking::stake_value(1, &1), Some(100));
    assert_eq!(Staking::stake_value(1, &2), Some(30));
    assert_eq!(Staking::stake_value(1, &3), Some(20));
  });
}
