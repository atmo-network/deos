use super::*;

#[test]
fn unit_asset_adapter_fails_closed_for_every_mutation() {
  type UnsupportedAssetOps = ();
  assert_eq!(
    <UnsupportedAssetOps as AssetOps<AccountId, TestAsset, Balance>>::transfer(
      &ALICE,
      &BOB,
      TestAsset::Native,
      1,
    ),
    Err(TaskFailure {
      error: DispatchError::Other("AssetOps not configured"),
      retry: RetryClass::Permanent,
    })
  );
  assert_eq!(
    <UnsupportedAssetOps as AssetOps<AccountId, TestAsset, Balance>>::burn(
      &ALICE,
      TestAsset::Native,
      1,
    ),
    Err(TaskFailure {
      error: DispatchError::Other("AssetOps not configured"),
      retry: RetryClass::Permanent,
    })
  );
  assert_eq!(
    <UnsupportedAssetOps as AssetOps<AccountId, TestAsset, Balance>>::mint(
      &ALICE,
      TestAsset::Native,
      1,
    ),
    Err(TaskFailure {
      error: DispatchError::Other("AssetOps not configured"),
      retry: RetryClass::Permanent,
    })
  );
  assert_eq!(
    <UnsupportedAssetOps as AssetOps<AccountId, TestAsset, Balance>>::preflight_transfer(
      &ALICE,
      &BOB,
      TestAsset::Native,
      1,
    ),
    Err(TaskFailure {
      error: DispatchError::Other("AssetOps not configured"),
      retry: RetryClass::Permanent,
    })
  );
}

#[test]
fn transfer_burn_and_mint_use_independent_weight_classes() {
  new_test_ext().execute_with(|| {
    let transfer = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    };
    let burn = Task::Burn {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    };
    let mint = Task::Mint {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    };
    assert_eq!(
      Actors::weight_upper_bound(&transfer),
      <TestWeightInfo as crate::WeightInfo>::task_transfer()
    );
    assert_eq!(
      Actors::weight_upper_bound(&burn),
      <TestWeightInfo as crate::WeightInfo>::task_burn()
    );
    assert_eq!(
      Actors::weight_upper_bound(&mint),
      <TestWeightInfo as crate::WeightInfo>::task_mint()
    );
  });
}

#[test]
fn create_rejects_empty_asset_whitelist_filter() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let empty_assets: BoundedVec<TestAsset, <Test as crate::Config>::MaxWhitelistSize> =
      BoundedVec::default();
    let schedule =
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Whitelist(empty_assets));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(schedule, None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::InvalidTriggerConfiguration
    );
  });
}

#[test]
fn split_transfer_rejects_share_sum_above_one() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let legs = BoundedVec::try_from(vec![
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(60),
      },
      SplitLeg {
        to: CHARLIE,
        share: Perbill::from_percent(50),
      },
    ])
    .expect("legs fit");
    let contract_steps = contract_steps_with_step(make_step(Task::SplitTransfer {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(100),
      legs,
    }));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, contract_steps),
      ),
      Error::<Test>::InvalidSplitTransfer
    );
  });
}

#[test]
fn split_transfer_rejects_duplicate_recipients() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let legs = BoundedVec::try_from(vec![
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(50),
      },
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(50),
      },
    ])
    .expect("legs fit");
    let contract_steps = contract_steps_with_step(make_step(Task::SplitTransfer {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(100),
      legs,
    }));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, contract_steps),
      ),
      Error::<Test>::InvalidSplitTransfer
    );
  });
}

#[test]
fn split_transfer_leg_count_is_bounded_by_runtime_type_limit() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_legs = <<Test as crate::Config>::MaxSplitTransferLegs as Get<u32>>::get() as usize;
    let within_limit = (0..max_legs)
      .map(|offset| SplitLeg {
        to: 10u64.saturating_add(offset as u64),
        share: Perbill::from_percent(1),
      })
      .collect::<Vec<_>>();
    let above_limit = (0..max_legs.saturating_add(1))
      .map(|offset| SplitLeg {
        to: 10u64.saturating_add(offset as u64),
        share: Perbill::from_percent(1),
      })
      .collect::<Vec<_>>();
    assert!(SplitTransferLegsOf::<Test>::try_from(within_limit).is_ok());
    assert!(SplitTransferLegsOf::<Test>::try_from(above_limit).is_err());
  });
}

#[test]
fn split_transfer_normalization_conserves_u128_max_without_overflow() {
  let total = u128::MAX;
  let partitions = [
    vec![Perbill::from_parts(1), Perbill::from_parts(999_999_999)],
    vec![Perbill::from_percent(50), Perbill::from_percent(50)],
    vec![Perbill::from_percent(25); 4],
    vec![Perbill::from_parts(125_000_000); 8],
  ];
  for shares in partitions {
    let distributed = shares
      .iter()
      .try_fold(0u128, |sum, share| sum.checked_add(share.mul_floor(total)))
      .expect("a validated share partition cannot overflow its total");
    let retained = total
      .checked_sub(distributed)
      .expect("floored normalized legs cannot exceed their total");
    assert_eq!(
      distributed.checked_add(retained),
      Some(total),
      "distribution and retained remainder conserve the resolved total"
    );
    assert!(shares.iter().all(|share| share.mul_floor(total) != 0));
  }
}

#[test]
fn split_transfer_executes_and_remainder_is_retained() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let total = 101u128;
    let legs = BoundedVec::try_from(vec![
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(50),
      },
      SplitLeg {
        to: CHARLIE,
        share: Perbill::from_percent(50),
      },
    ])
    .expect("legs fit");
    let contract_steps = contract_steps_with_step(make_step(Task::SplitTransfer {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(total),
      legs,
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 1_000);
    let actor = sovereign_account(actor_id);
    let actor_before = native_balance(&actor);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(native_balance(&CHARLIE), charlie_before.saturating_add(50));
    assert_eq!(native_balance(&actor), actor_before.saturating_sub(100));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::SplitTransferExecuted {
          actor_id: id,
          total: emitted_total,
          distributed,
          retained,
          legs: 2,
          effective_legs: 2,
          ..
        } if *id == actor_id
          && *emitted_total == total
          && *distributed == 100
          && *retained == 1
      )
    }));
  });
}

#[test]
fn split_transfer_rejects_five_ineligible_legs_atomically_then_retries() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_asset_minimum_balance(11);
    let asset = TestAsset::Local(11);
    let recipients = [ALICE, BOB, CHARLIE, 4, 5, 6, 7, 8];
    let legs = recipients
      .iter()
      .map(|to| SplitLeg {
        to: *to,
        share: Perbill::from_parts(125_000_000),
      })
      .collect::<Vec<_>>()
      .try_into()
      .expect("eight legs fit");
    let mut step = make_step(Task::SplitTransfer {
      asset,
      amount: AmountResolution::Fixed(80),
      legs,
    });
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 2 };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 1_000);
    let actor_before = asset_balance(&actor, asset);
    let recipient_balances = recipients.map(|recipient| asset_balance(&recipient, asset));

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(asset_balance(&actor, asset), actor_before);
    assert_eq!(
      recipients.map(|recipient| asset_balance(&recipient, asset)),
      recipient_balances
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepFailed { actor_id: id, error, .. }
        if *id == actor_id
          && *error == Error::<Test>::RecipientDepositUnavailable.into()
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::SplitTransferExecuted { actor_id: id, .. } if *id == actor_id
    )));
    let continuation = Actors::actor_run_state(actor_id).expect("temporary rejection suspends");
    assert_eq!(continuation.cursor, 0);
    assert_eq!(continuation.unsuccessful_attempts_at_cursor, 1);

    for recipient in recipients {
      set_asset_balance(&recipient, asset, 1);
    }
    let retry_balances = recipients.map(|recipient| asset_balance(&recipient, asset));
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);

    assert_eq!(asset_balance(&actor, asset), actor_before - 80);
    for (recipient, before) in recipients.iter().zip(retry_balances) {
      assert_eq!(asset_balance(recipient, asset), before + 10);
    }
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::SplitTransferExecuted {
        actor_id: id,
        total: 80,
        distributed: 80,
        retained: 0,
        legs: 8,
        effective_legs: 8,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn split_transfer_late_leg_failure_rolls_back_every_leg() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let legs = BoundedVec::try_from(vec![
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(50),
      },
      SplitLeg {
        to: CHARLIE,
        share: Perbill::from_percent(50),
      },
    ])
    .expect("legs fit");
    let contract_steps = contract_steps_with_step(make_step(Task::SplitTransfer {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(100),
      legs,
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 1_000);
    let actor = sovereign_account(actor_id);
    let actor_before = native_balance(&actor);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    set_fail_transfer_to(Some(CHARLIE));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    set_fail_transfer_to(None);
    assert_eq!(native_balance(&actor), actor_before);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert!(has_actor_event(|event| {
      matches!(event, Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id)
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::SplitTransferExecuted { actor_id: id, .. } if *id == actor_id)
    }));
  });
}

#[test]
fn all_zero_split_transfer_total_is_an_explicit_resolution_skip() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let legs = BoundedVec::try_from(vec![
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(50),
      },
      SplitLeg {
        to: CHARLIE,
        share: Perbill::from_percent(50),
      },
    ])
    .expect("legs fit");
    let contract_steps = contract_steps_with_step(make_step(Task::SplitTransfer {
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
      legs,
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    let actor_before = native_balance(&actor);
    let bob_before = native_balance(&BOB);
    let charlie_before = native_balance(&CHARLIE);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    // No silent zero-leg transfer: the all-zero total resolves as a skip with no balance read,
    // preflight, or SplitTransferExecuted event, and the cycle continues.
    assert_eq!(native_balance(&actor), actor_before);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert!(!has_actor_event(|event| {
      matches!(event, Event::SplitTransferExecuted { actor_id: id, .. } if *id == actor_id)
    }));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepSkipped { actor_id: id, reason: StepSkippedReason::ResolutionSkipped, .. }
        if *id == actor_id
    )));
  });
}

#[test]
fn split_transfer_rounding_skips_zero_distribution_and_allows_one_effective_leg() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let make_plan = |amount, first_share, second_share| {
      contract_steps_with_step(make_step(Task::SplitTransfer {
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(amount),
        legs: BoundedVec::try_from(vec![
          SplitLeg {
            to: BOB,
            share: first_share,
          },
          SplitLeg {
            to: CHARLIE,
            share: second_share,
          },
        ])
        .expect("two legs fit"),
      }))
    };
    let skipped = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      make_plan(1, Perbill::from_percent(50), Perbill::from_percent(50)),
    );
    fund_native(skipped, 10);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      skipped
    ));
    run_idle(Weight::MAX);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::SplitTransferExecuted { actor_id, .. } if *actor_id == skipped
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepSkipped {
        actor_id,
        reason: StepSkippedReason::ResolutionSkipped,
        ..
      } if *actor_id == skipped
    )));

    let one_effective = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      make_plan(2, Perbill::from_percent(60), Perbill::from_percent(40)),
    );
    fund_native(one_effective, 10);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      one_effective,
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::SplitTransferExecuted {
        actor_id,
        distributed: 1,
        effective_legs: 1,
        ..
      } if *actor_id == one_effective
    )));
  });
}

#[test]
fn on_address_event_asset_filter_is_enforced() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_whitelist = BoundedVec::try_from(vec![TestAsset::Local(7)]).expect("fits");
    let schedule =
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Whitelist(asset_whitelist));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule,
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Local(7),
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(10));
  });
}

#[test]
fn cadence_rearm_capacity_failure_rolls_back_pipeline_opening() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      timer_schedule(1),
      None,
      transfer_contract_steps(BOB, 10),
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    let mut wakeup_meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut wakeup_meter);
    let ready = Actors::actor_hot(actor_id).expect("Cadenced Actor remains active after detection");
    assert!(ready.pending_signal);
    assert!(ready.queue_ticket.is_some());
    assert!(ready.trigger_wakeup_pointer.is_none());
    let bob_before = native_balance(&BOB);
    System::reset_events();
    crate::WakeupCursorLen::<Test>::insert(
      WakeupClock::Tick,
      <<Test as crate::Config>::MaxActiveActors as Get<u32>>::get(),
    );

    let _ = Actors::execute_cycle(Weight::MAX);

    assert_eq!(native_balance(&BOB), bob_before);
    let state = Actors::active_actor_state(actor_id).expect("Cadenced Actor remains active");
    assert_eq!(state.identity.cycle_nonce, 0);
    assert!(state.hot.pending_signal);
    assert!(state.hot.trigger_wakeup_pointer.is_none());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActorClosed { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn user_actor_rejects_mint_task_on_create() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Mint {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    }));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, contract_steps),
      ),
      Error::<Test>::MintNotAllowedForUserActor
    );
  });
}

#[test]
fn create_accepts_swap_in_with_slippage_tolerance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::SwapIn {
      asset_in: TestAsset::Native,
      asset_out: TestAsset::Local(1),
      amount_in: AmountResolution::Fixed(10),
      slippage_tolerance: Perbill::from_percent(5),
    }));
    prefund_active_user_creation(ALICE, &contract_steps);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, contract_steps),
    ));
  });
}

#[test]
fn dex_adapter_receives_authoritative_actor_type() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      temporary_retry_swap_plan(),
    );
    fund_native(actor_id, 1_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(last_dex_actor_type(), Some(ActorType::User));
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, temporary_retry_swap_plan());
    fund_native(actor_id, 1_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(last_dex_actor_type(), Some(ActorType::System));
  });
}

#[test]
fn full_slippage_cannot_accept_zero_swap_output() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_out = TestAsset::Local(91);
    setup_pool(TestAsset::Native, asset_out, 1_000_000, 1);
    set_asset_balance(&u64::MAX, asset_out, 1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out,
        amount_in: AmountResolution::Fixed(1),
        slippage_tolerance: Perbill::one(),
      })),
    );
    fund_native(actor_id, 10);
    frame_system::Pallet::<Test>::reset_events();

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert!(!has_actor_event(|event| matches!(
      event,
      Event::SwapExecuted { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepFailed {
        actor_id: id,
        retry_class: RetryClass::Permanent,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn create_rejects_swap_out_with_zero_input_cap() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::SwapOut {
      asset_out: TestAsset::Local(2),
      amount_out: AmountResolution::Fixed(10),
      asset_in: TestAsset::Local(1),
      input_limit: InputLimit::Absolute(0),
      slippage_tolerance: Perbill::zero(),
    }));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, contract_steps),
      ),
      Error::<Test>::InvalidTradeBound
    );
  });
}

#[test]
fn create_rejects_zero_liquidity_output_bounds() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let add_plan = contract_steps_with_step(make_step(Task::AddLiquidity {
      asset_a: TestAsset::Local(1),
      asset_b: TestAsset::Local(2),
      amount_a: AmountResolution::Fixed(10),
      amount_b: AmountResolution::Fixed(10),
      min_lp_out: 0,
    }));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, add_plan),
      ),
      Error::<Test>::InvalidTradeBound
    );

    let remove_plan = contract_steps_with_step(make_step(Task::RemoveLiquidity {
      lp_asset: TestAsset::Local(3),
      asset_a: TestAsset::Local(3),
      asset_b: TestAsset::Local(3),
      lp_amount: AmountResolution::Fixed(10),
      min_amount_a: 1,
      min_amount_b: 0,
    }));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, remove_plan),
      ),
      Error::<Test>::InvalidTradeBound
    );
  });
}

#[test]
fn liquidity_tasks_fail_before_effects_when_output_minima_are_unmet() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let add_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::AddLiquidity {
        asset_a: TestAsset::Local(1),
        asset_b: TestAsset::Local(2),
        amount_a: AmountResolution::Fixed(4),
        amount_b: AmountResolution::Fixed(4),
        min_lp_out: 5,
      })),
    );
    fund_native(add_id, 1_000);
    let add_actor = sovereign_account(add_id);
    set_asset_balance(&add_actor, TestAsset::Local(1), 10);
    set_asset_balance(&add_actor, TestAsset::Local(2), 10);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), add_id));
    run_idle(Weight::MAX);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::LiquidityAdded { actor_id, .. } if *actor_id == add_id
    )));

    let remove_id = create_user_with(
      BOB,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::RemoveLiquidity {
        lp_asset: TestAsset::Local(3),
        asset_a: TestAsset::Local(3),
        asset_b: TestAsset::Local(3),
        lp_amount: AmountResolution::Fixed(10),
        min_amount_a: 6,
        min_amount_b: 6,
      })),
    );
    fund_native(remove_id, 1_000);
    let remove_actor = sovereign_account(remove_id);
    set_asset_balance(&remove_actor, TestAsset::Local(3), 20);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(BOB),
      remove_id
    ));
    run_idle(Weight::MAX);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::LiquidityRemoved { actor_id, .. } if *actor_id == remove_id
    )));
  });
}

#[test]
fn market_tasks_dispatch_their_resolved_task_local_amounts_without_a_system_cap() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let swap_in = TestAsset::Local(81);
    let swap_out = TestAsset::Local(82);
    set_pool_reserves(swap_in, swap_out, 1_000, 1_000);
    set_asset_balance(&u64::MAX, swap_out, 1_000);
    let plan = BoundedVec::try_from(vec![
      make_step(Task::SwapIn {
        asset_in: swap_in,
        asset_out: swap_out,
        amount_in: AmountResolution::Fixed(100),
        slippage_tolerance: Perbill::one(),
      }),
      make_step(Task::AddLiquidity {
        asset_a: TestAsset::Local(83),
        asset_b: TestAsset::Local(84),
        amount_a: AmountResolution::Fixed(100),
        amount_b: AmountResolution::Fixed(100),
        min_lp_out: 1,
      }),
      make_step(Task::RemoveLiquidity {
        lp_asset: TestAsset::Local(85),
        asset_a: TestAsset::Local(83),
        asset_b: TestAsset::Local(84),
        lp_amount: AmountResolution::Fixed(100),
        min_amount_a: 1,
        min_amount_b: 1,
      }),
      make_step(Task::DonateLiquidity {
        asset_a: TestAsset::Local(86),
        asset_b: TestAsset::Local(87),
        max_amount_a: AmountResolution::Fixed(100),
        max_ratio_error: Perbill::zero(),
      }),
    ])
    .expect("four market tasks fit System plan");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 1_000);
    for asset in [
      swap_in,
      TestAsset::Local(83),
      TestAsset::Local(84),
      TestAsset::Local(85),
      TestAsset::Local(86),
      TestAsset::Local(87),
    ] {
      set_asset_balance(&actor, asset, 101);
    }

    // The remove-liquidity step's LP token maps to the ordered pair created by the
    // add-liquidity step; the mock pair registry honors the host-owned binding.
    register_lp_pair(
      TestAsset::Local(85),
      TestAsset::Local(83),
      TestAsset::Local(84),
    );

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::SwapExecuted { actor_id: id, amount_in: 100, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::LiquidityAdded { actor_id: id, lp_minted: 100, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::LiquidityRemoved { actor_id: id, amount_a: 50, amount_b: 50, .. }
        if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::LiquidityDonated { actor_id: id, max_amount_a: 100, amount_a: 100, amount_b: 100, .. }
        if *id == actor_id
    )));
  });
}

#[test]
fn create_accepts_swap_out_with_optional_absolute_cap() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::SwapOut {
      asset_out: TestAsset::Local(2),
      amount_out: AmountResolution::Fixed(10),
      asset_in: TestAsset::Local(1),
      input_limit: InputLimit::Absolute(100),
      slippage_tolerance: Perbill::from_percent(5),
    }));
    prefund_active_user_creation(ALICE, &contract_steps);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, contract_steps),
    ));
  });
}

#[test]
fn swap_out_live_market_mode_uses_preservable_capacity_and_emits_swap_event() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_in = TestAsset::Local(1);
    let asset_out = TestAsset::Local(2);
    set_pool_reserves(asset_in, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::SwapOut {
        asset_out,
        amount_out: AmountResolution::Fixed(100),
        asset_in,
        input_limit: InputLimit::LiveQuote,
        slippage_tolerance: Perbill::from_percent(0),
      })),
    );
    fund_native(actor_id, 10_000);
    let sovereign = sovereign_account(actor_id);
    set_asset_balance(&sovereign, asset_in, 1_000);
    let out_before = asset_balance(&sovereign, asset_out);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let out_after = asset_balance(&sovereign, asset_out);
    assert!(out_after >= out_before.saturating_add(100));
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::SwapExecuted {
          actor_id: id,
          asset_in: in_asset,
          asset_out: out_asset,
          amount_in,
          amount_out,
          ..
        } if *id == actor_id
          && *in_asset == asset_in
          && *out_asset == asset_out
          && *amount_in > 0
          && *amount_out >= 100
      )
    }));
  });
}

#[test]
fn swap_out_never_spends_above_explicit_input_cap() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_in = TestAsset::Local(1);
    let asset_out = TestAsset::Local(2);
    set_pool_reserves(asset_in, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::SwapOut {
        asset_out,
        amount_out: AmountResolution::Fixed(100),
        asset_in,
        input_limit: InputLimit::Absolute(50),
        slippage_tolerance: Perbill::zero(),
      })),
    );
    fund_native(actor_id, 10_000);
    let sovereign = sovereign_account(actor_id);
    set_asset_balance(&sovereign, asset_in, 1_000);
    let input_before = asset_balance(&sovereign, asset_in);
    let output_before = asset_balance(&sovereign, asset_out);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(asset_balance(&sovereign, asset_in), input_before);
    assert_eq!(asset_balance(&sovereign, asset_out), output_before);
    assert!(has_actor_event(|event| {
      matches!(event, Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id)
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
    }));
  });
}

#[test]
fn ordinary_transfer_updates_accumulator_without_resuming_paused_system_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(100)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    ActorHot::<Test>::mutate(actor_id, |maybe| {
      let hot = maybe.as_mut().expect("Actors hot state exists");
      hot.lifecycle = ActiveLifecycle::Paused;
    });
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      123
    ));
    let updated = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(updated.lifecycle, ActiveLifecycle::Paused);
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&123)
    );
    assert!(!has_actor_event(|event| {
      matches!(event, Event::ActorResumed { actor_id: id } if *id == actor_id)
    }));
  });
}

#[test]
fn zero_amount_resolutions_and_identical_market_assets_are_rejected() {
  new_test_ext().execute_with(|| {
    for amount in [
      AmountResolution::Fixed(0),
      AmountResolution::PercentageOfCurrent(Perbill::zero()),
      AmountResolution::PercentageAtOpening(Perbill::zero()),
      AmountResolution::PercentageOfLastFunding(Perbill::zero()),
    ] {
      let plan = contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount,
      }));
      assert_noop!(
        Actors::create_system_actor(
          RuntimeOrigin::root(),
          ALICE,
          Mutability::Mutable,
          system_active_contract(manual_schedule(), None, plan),
        ),
        Error::<Test>::InvalidAmountResolution
      );
    }

    let zero_nested_amount_tasks = vec![
      Task::SwapOut {
        asset_out: TestAsset::Local(77),
        amount_out: AmountResolution::Fixed(0),
        asset_in: TestAsset::Native,
        input_limit: InputLimit::LiveQuote,
        slippage_tolerance: Perbill::one(),
      },
      Task::AddLiquidity {
        asset_a: TestAsset::Native,
        asset_b: TestAsset::Local(77),
        amount_a: AmountResolution::Fixed(1),
        amount_b: AmountResolution::Fixed(0),
        min_lp_out: 1,
      },
      Task::RemoveLiquidity {
        lp_asset: TestAsset::Local(99),
        asset_a: TestAsset::Local(99),
        asset_b: TestAsset::Local(99),
        lp_amount: AmountResolution::Fixed(0),
        min_amount_a: 1,
        min_amount_b: 1,
      },
      Task::Unstake {
        asset: TestAsset::Local(77),
        shares: AmountResolution::Fixed(0),
      },
    ];
    for task in zero_nested_amount_tasks {
      assert_noop!(
        Actors::create_system_actor(
          RuntimeOrigin::root(),
          ALICE,
          Mutability::Mutable,
          system_active_contract(
            manual_schedule(),
            None,
            contract_steps_with_step(make_step(task)),
          ),
        ),
        Error::<Test>::InvalidAmountResolution
      );
    }

    let identical_asset_tasks = vec![
      Task::SwapIn {
        asset_in: TestAsset::Native,
        amount_in: AmountResolution::Fixed(1),
        asset_out: TestAsset::Native,
        slippage_tolerance: Perbill::one(),
      },
      Task::AddLiquidity {
        asset_a: TestAsset::Native,
        asset_b: TestAsset::Native,
        amount_a: AmountResolution::Fixed(1),
        amount_b: AmountResolution::Fixed(1),
        min_lp_out: 1,
      },
      Task::DonateLiquidity {
        asset_a: TestAsset::Native,
        asset_b: TestAsset::Native,
        max_amount_a: AmountResolution::Fixed(1),
        max_ratio_error: Perbill::one(),
      },
    ];
    for task in identical_asset_tasks {
      assert_noop!(
        Actors::create_system_actor(
          RuntimeOrigin::root(),
          ALICE,
          Mutability::Mutable,
          system_active_contract(
            manual_schedule(),
            None,
            contract_steps_with_step(make_step(task)),
          ),
        ),
        Error::<Test>::InvalidTradeBound
      );
    }
  });
}

#[test]
fn self_transfer_rejection_covers_create_update_and_activation_paths() {
  new_test_ext().execute_with(|| {
    let system_id = Actors::next_actor_id();
    let system_sovereign = Actors::sovereign_account_id_system(system_id);
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(
          manual_schedule(),
          None,
          transfer_contract_steps(system_sovereign, 1),
        ),
      ),
      Error::<Test>::SelfTransferNotAllowed
    );

    let user_sovereign = Actors::sovereign_account_id(&ALICE, 0);
    assert_noop!(
      Actors::create_user_actor_at_slot(
        RuntimeOrigin::signed(ALICE),
        0,
        Mutability::Mutable,
        user_active_contract(
          manual_schedule(),
          None,
          transfer_contract_steps(user_sovereign, 1),
        ),
      ),
      Error::<Test>::SelfTransferNotAllowed
    );
    assert_eq!(fee_collections(), Vec::<Balance>::new());
    assert_eq!(Actors::owner_slot_bitmap(ALICE), [0; 32]);

    let active_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let active_sovereign = sovereign_account(active_id);
    let before = Actors::active_actor_view(active_id).expect("active actor");
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::root(),
        active_id,
        transfer_contract_steps(active_sovereign, 1),
        crate::CompletionPolicy::Persistent,
      ),
      Error::<Test>::SelfTransferNotAllowed
    );
    assert_eq!(Actors::active_actor_view(active_id), Some(before));

    let dormant_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None,
    ));
    let dormant = Actors::actor_identities(dormant_id).expect("dormant identity");
    let self_leg_plan = contract_steps_with_step(make_step(Task::SplitTransfer {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
      legs: BoundedVec::try_from(vec![
        SplitLeg {
          to: dormant.sovereign_account,
          share: Perbill::from_percent(50),
        },
        SplitLeg {
          to: BOB,
          share: Perbill::from_percent(50),
        },
      ])
      .expect("two legs fit"),
    }));
    assert_noop!(
      Actors::activate_actor(
        RuntimeOrigin::signed(ALICE),
        dormant_id,
        system_active_contract(manual_schedule(), None, self_leg_plan)
          .expect("direct Actor Contract"),
      ),
      Error::<Test>::SelfTransferNotAllowed
    );
    assert!(Actors::actor_identities(dormant_id).is_some());
    assert!(Actors::active_actor_view(dormant_id).is_none());
  });
}

#[test]
fn dex_adapter_late_failure_rolls_back_input_transfer() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_in = TestAsset::Native;
    let asset_out = TestAsset::Local(77);
    set_pool_reserves(asset_in, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in,
        asset_out,
        amount_in: AmountResolution::Fixed(40),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let contract_steps = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 120);
    set_fail_dex_after_input_transfer(true);
    let charlie_before = native_balance(&CHARLIE);
    let pool_native_before = native_balance(&u64::MAX);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 110);
    assert_eq!(asset_balance(&actor, asset_out), 0);
    assert_eq!(native_balance(&u64::MAX), pool_native_before);
    assert_eq!(asset_balance(&u64::MAX, asset_out), 10_000);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(!has_actor_event(|e| matches!(
      e,
      Event::SwapExecuted { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 1, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn preserve_spend_keeps_sufficient_asset_minimum() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(7);
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset,
        amount: AmountResolution::Fixed(10),
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset,
        amount: AmountResolution::AllAvailable,
      }),
    ])
    .expect("system execution plan fits");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 10);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 1);
    assert_eq!(asset_balance(&BOB, asset), 9);
  });
}

#[test]
fn stake_task_delegates_to_staking_adapter() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(7);
    let contract_steps = contract_steps_with_step(make_step(Task::Stake {
      asset,
      amount: AmountResolution::Fixed(120),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 200);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 80);
    assert_eq!(staked_balance(actor, asset), 120);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StakeExecuted { actor_id: id, asset: event_asset, amount, .. }
        if *id == actor_id && *event_asset == asset && *amount == 120
    )));
  });
}

#[test]
fn unstake_task_delegates_to_staking_adapter() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(8);
    let contract_steps = contract_steps_with_step(make_step(Task::Unstake {
      asset,
      shares: AmountResolution::Fixed(50),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 75);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 25);
    assert_eq!(unstaked_shares(actor, asset), 50);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::UnstakeExecuted { actor_id: id, asset: event_asset, shares, .. }
        if *id == actor_id && *event_asset == asset && *shares == 50
    )));
  });
}

#[test]
fn unstake_dynamic_modes_resolve_against_staking_shares() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(8);
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Unstake {
        asset,
        shares: AmountResolution::PercentageOfCurrent(Perbill::from_percent(25)),
      }),
      make_step(Task::Unstake {
        asset,
        shares: AmountResolution::PercentageAtOpening(Perbill::from_percent(50)),
      }),
    ])
    .expect("system execution plan fits");
    let actor_id = create_system_with(ALICE, percentage_trigger_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 100);
    signal_percentage_trigger(actor_id, asset);
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 25);
    assert_eq!(unstaked_shares(actor, asset), 75);
  });
}

#[test]
fn stake_adapter_failure_can_continue_next_step() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(13);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::Stake {
        asset,
        amount: AmountResolution::Fixed(40),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let contract_steps = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 100);
    fund_native(actor_id, 20);
    set_fail_staking_ops(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 100);
    assert_eq!(staked_balance(actor, asset), 0);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 1, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn unstake_adapter_failure_aborts_cycle_without_partial_effects() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(14);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::Unstake {
        asset,
        shares: AmountResolution::Fixed(40),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let skipped_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let contract_steps = BoundedVec::try_from(vec![failing_step, skipped_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 100);
    fund_native(actor_id, 20);
    set_fail_staking_ops(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 100);
    assert_eq!(unstaked_shares(actor, asset), 0);
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 0, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn unstake_adapter_late_failure_rolls_back_partial_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Native;
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::Unstake {
        asset,
        shares: AmountResolution::Fixed(40),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let contract_steps = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 120);
    set_fail_staking_after_burn(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 110);
    assert_eq!(unstaked_shares(actor, asset), 0);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(!has_actor_event(|e| matches!(
      e,
      Event::UnstakeExecuted { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 1, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn donate_liquidity_task_delegates_to_liquidity_donation_adapter() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_a = TestAsset::Local(9);
    let asset_b = TestAsset::Local(10);
    let contract_steps = contract_steps_with_step(make_step(Task::DonateLiquidity {
      asset_a,
      asset_b,
      max_amount_a: AmountResolution::Fixed(40),
      max_ratio_error: Perbill::from_percent(1),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_a, 100);
    set_asset_balance(&actor, asset_b, 90);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset_a), 60);
    assert_eq!(asset_balance(&actor, asset_b), 50);
    assert_eq!(donated_liquidity(actor, asset_a, asset_b), (40, 40));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::LiquidityDonated {
        actor_id: id,
        asset_a: event_asset_a,
        asset_b: event_asset_b,
        max_amount_a,
        amount_a,
        amount_b,
        ..
      } if *id == actor_id
        && *event_asset_a == asset_a
        && *event_asset_b == asset_b
        && *max_amount_a == 40
        && *amount_a == 40
        && *amount_b == 40
    )));
  });
}

#[test]
fn donate_liquidity_percentage_resolves_only_against_asset_a() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_a = TestAsset::Local(73);
    let asset_b = TestAsset::Local(74);
    let contract_steps = contract_steps_with_step(make_step(Task::DonateLiquidity {
      asset_a,
      asset_b,
      max_amount_a: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
      max_ratio_error: Perbill::from_percent(1),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_a, 101);
    set_asset_balance(&actor, asset_b, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(donated_liquidity(actor, asset_a, asset_b), (50, 50));
    assert_eq!(asset_balance(&actor, asset_a), 51);
    assert_eq!(asset_balance(&actor, asset_b), 50);
  });
}

#[test]
fn donate_liquidity_asset_b_debit_is_capped_at_preservable_capacity() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // The resolved asset-a amount exceeds the preservable asset-b capacity, so the generic
    // contract must cap the paired debit at the smaller max_amount_b and report exact used
    // amounts without overdrawing the b-side or the fee-native protected floor.
    let asset_a = TestAsset::Local(81);
    let asset_b = TestAsset::Local(82);
    let contract_steps = contract_steps_with_step(make_step(Task::DonateLiquidity {
      asset_a,
      asset_b,
      max_amount_a: AmountResolution::PercentageOfCurrent(Perbill::from_percent(100)),
      max_ratio_error: Perbill::from_percent(1),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_a, 100);
    // Asset b holds less than the resolved a-side, and its protected floor keeps part reserved.
    set_asset_balance(&actor, asset_b, 60);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let (used_a, used_b) = donated_liquidity(actor, asset_a, asset_b);
    assert!(
      used_b <= 60,
      "asset-b debit must respect its preservable capacity"
    );
    assert_eq!(
      used_a, used_b,
      "balanced mock donation caps at the smaller side"
    );
    assert_eq!(asset_balance(&actor, asset_a), 100 - used_a);
    assert_eq!(asset_balance(&actor, asset_b), 60 - used_b);
  });
}

#[test]
fn donate_liquidity_asset_b_cap_keeps_capped_task_continuing() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_a = TestAsset::Local(11);
    let asset_b = TestAsset::Local(12);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::DonateLiquidity {
        asset_a,
        asset_b,
        max_amount_a: AmountResolution::Fixed(40),
        max_ratio_error: Perbill::from_percent(1),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let contract_steps = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_a, 100);
    set_asset_balance(&actor, asset_b, 10);
    fund_native(actor_id, 20);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    // The asset-b debit cap caps the balanced donation at the smaller preservable side instead
    // of overdrawing; the capped task succeeds and the cycle continues to the next step.
    assert_eq!(asset_balance(&actor, asset_a), 91);
    assert_eq!(asset_balance(&actor, asset_b), 1);
    assert_eq!(donated_liquidity(actor, asset_a, asset_b), (9, 9));
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::LiquidityDonated { actor_id: id, amount_a: 9, amount_b: 9, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::TransferExecuted { actor_id: id, to, amount, .. }
        if *id == actor_id && *to == CHARLIE && *amount == 10
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 2, failed_steps: 0, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn add_liquidity_late_failure_rolls_back_partial_debit() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_b = TestAsset::Local(18);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::AddLiquidity {
        asset_a: TestAsset::Native,
        asset_b,
        amount_a: AmountResolution::Fixed(40),
        amount_b: AmountResolution::Fixed(40),
        min_lp_out: 1,
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap(),
    );
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 100);
    set_asset_balance(&actor, asset_b, 100);
    let actor_native_before = native_balance(&actor);
    let charlie_before = native_balance(&CHARLIE);
    set_fail_add_liquidity_after_first_debit(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), actor_native_before - 10);
    assert_eq!(asset_balance(&actor, asset_b), 100);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::LiquidityAdded { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn remove_liquidity_late_failure_rolls_back_partial_credit() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let lp_asset = TestAsset::Local(30);
    let asset_b = TestAsset::Local(31);
    register_lp_pair(lp_asset, TestAsset::Native, asset_b);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::RemoveLiquidity {
        lp_asset,
        asset_a: TestAsset::Native,
        asset_b,
        lp_amount: AmountResolution::Fixed(40),
        min_amount_a: 1,
        min_amount_b: 1,
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap(),
    );
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 100);
    set_asset_balance(&actor, lp_asset, 100);
    let actor_native_before = native_balance(&actor);
    let charlie_before = native_balance(&CHARLIE);
    set_fail_remove_liquidity_after_first_credit(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), actor_native_before - 10);
    assert_eq!(asset_balance(&actor, lp_asset), 100);
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::LiquidityRemoved { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn donate_liquidity_adapter_failure_aborts_cycle_without_partial_effects() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_a = TestAsset::Local(15);
    let asset_b = TestAsset::Local(16);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::DonateLiquidity {
        asset_a,
        asset_b,
        max_amount_a: AmountResolution::Fixed(40),
        max_ratio_error: Perbill::from_percent(1),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let skipped_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let contract_steps = BoundedVec::try_from(vec![failing_step, skipped_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_a, 100);
    set_asset_balance(&actor, asset_b, 100);
    fund_native(actor_id, 20);
    set_fail_liquidity_donation_ops(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset_a), 100);
    assert_eq!(asset_balance(&actor, asset_b), 100);
    assert_eq!(donated_liquidity(actor, asset_a, asset_b), (0, 0));
    assert_eq!(native_balance(&CHARLIE), charlie_before);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 0, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn donate_liquidity_adapter_late_failure_rolls_back_partial_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_a = TestAsset::Native;
    let asset_b = TestAsset::Local(19);
    let failing_step = StepOf::<Test> {
      precondition: None,
      task: Task::DonateLiquidity {
        asset_a,
        asset_b,
        max_amount_a: AmountResolution::Fixed(40),
        max_ratio_error: Perbill::from_percent(1),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let succeeding_step = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let contract_steps = BoundedVec::try_from(vec![failing_step, succeeding_step]).unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_b, 100);
    fund_native(actor_id, 120);
    set_fail_liquidity_donation_after_first_burn(true);
    let charlie_before = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 110);
    assert_eq!(asset_balance(&actor, asset_b), 100);
    assert_eq!(donated_liquidity(actor, asset_a, asset_b), (0, 0));
    assert_eq!(native_balance(&CHARLIE), charlie_before + 10);
    assert!(!has_actor_event(|e| matches!(
      e,
      Event::LiquidityDonated { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|e| matches!(
      e,
      Event::CycleSummary {
        actor_id: id,
        outcomes: OutcomeTotals { executed_steps: 1, failed_steps: 1, .. },
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn user_dca_swap_then_cold_storage_transfer() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let cold_wallet: AccountId = 9999;
    let schedule = timer_schedule(5);
    let foreign = TestAsset::Local(1);
    // Seed mock AMM pool for swap
    setup_pool(foreign, TestAsset::Native, 10_000, 10_000);
    let pool_account: AccountId = u64::MAX;
    set_asset_balance(&pool_account, foreign, 10_000);
    fund_native_raw(&pool_account, 10_000);
    // ContractSteps: SwapIn(foreign → native) → Transfer(native → cold wallet)
    let contract_steps = BoundedVec::try_from(vec![
      StepOf::<Test> {
        precondition: all_conditions(vec![Predicate::BalanceAbove {
          asset: foreign,
          threshold: 50,
        }]),
        task: Task::SwapIn {
          asset_in: foreign,
          asset_out: TestAsset::Native,
          amount_in: AmountResolution::Fixed(100),
          slippage_tolerance: Perbill::from_percent(10),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
      StepOf::<Test> {
        precondition: all_conditions(vec![Predicate::BalanceAbove {
          asset: TestAsset::Native,
          threshold: 10,
        }]),
        task: Task::Transfer {
          to: cold_wallet,
          asset: TestAsset::Native,
          amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(80)),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
    ])
    .unwrap();
    let actor_id = create_user_with(ALICE, Mutability::Mutable, schedule, None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, foreign, 1000);
    fund_native(actor_id, 5000);
    let cold_before = native_balance(&cold_wallet);
    frame_system::Pallet::<Test>::set_block_number(6);
    Actors::on_initialize(6);
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(7);
    Actors::on_initialize(7);
    run_idle(Weight::MAX);
    assert!(
      native_balance(&cold_wallet) > cold_before,
      "Cold storage should receive native after swap + transfer"
    );
    assert!(
      has_actor_event(|e| matches!(
        e,
        Event::SwapExecuted { actor_id: id, .. } if *id == actor_id
      )),
      "Swap should be executed"
    );
  });
}

#[test]
fn multi_asset_contract_steps_tracks_all_referenced_assets() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let foreign_a = TestAsset::Local(1);
    let foreign_b = TestAsset::Local(2);
    // ContractSteps references both assets via PercentageOfLastFunding
    let contract_steps = BoundedVec::try_from(vec![
      StepOf::<Test> {
        precondition: None,
        task: Task::Transfer {
          to: BOB,
          asset: foreign_a,
          amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(10)),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
      StepOf::<Test> {
        precondition: None,
        task: Task::Transfer {
          to: CHARLIE,
          asset: foreign_b,
          amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(20)),
        },
        on_error: StepErrorPolicy::AbortCycle,
      },
    ])
    .unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let funding = actor_funding(actor_id);
    // Verify both assets are tracked
    assert!(funding.funding_tracked_assets.contains(&foreign_a));
    assert!(funding.funding_tracked_assets.contains(&foreign_b));
  });
}
