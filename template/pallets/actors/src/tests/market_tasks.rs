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
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(10));
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
    frame_system::Pallet::<Test>::set_block_number(2);
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
    frame_system::Pallet::<Test>::set_block_number(2);
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
    frame_system::Pallet::<Test>::set_block_number(2);
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
    let mut add_step = make_step(Task::AddLiquidity {
      asset_a: TestAsset::Local(1),
      asset_b: TestAsset::Local(2),
      amount_a: AmountResolution::Fixed(4),
      amount_b: AmountResolution::Fixed(4),
      min_lp_out: 5,
    });
    add_step.on_error = StepErrorPolicy::RetryLater { max_attempts: 2 };
    let add_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(add_step),
    );
    fund_native(add_id, 1_000);
    let add_actor = sovereign_account(add_id);
    set_asset_balance(&add_actor, TestAsset::Local(1), 10);
    set_asset_balance(&add_actor, TestAsset::Local(2), 10);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), add_id));
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::LiquidityAdded { actor_id, .. } if *actor_id == add_id
    )));
    assert_eq!(asset_balance(&add_actor, TestAsset::Local(1)), 10);
    assert_eq!(asset_balance(&add_actor, TestAsset::Local(2)), 10);
    let run = Actors::actor_run_state(add_id).expect("dynamic LP output rejection suspends");
    assert_eq!(run.cursor, 0);
    assert_eq!(run.unsuccessful_attempts_at_cursor, 1);
    assert!(matches!(
      run.last_step_outcome,
      Some(StepOutcome::Failed(TaskFailure {
        retry: RetryClass::Temporary,
        ..
      }))
    ));

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
    frame_system::Pallet::<Test>::set_block_number(3);
    run_idle(Weight::MAX);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::LiquidityRemoved { actor_id, .. } if *actor_id == remove_id
    )));
  });
}

#[test]
fn liquidity_tasks_reject_adapter_outcomes_outside_authored_bounds() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_invalid_liquidity_outcome(true);

    let tasks = [
      Task::AddLiquidity {
        asset_a: TestAsset::Local(41),
        asset_b: TestAsset::Local(42),
        amount_a: AmountResolution::Fixed(10),
        amount_b: AmountResolution::Fixed(10),
        min_lp_out: 1,
      },
      Task::RemoveLiquidity {
        lp_asset: TestAsset::Local(43),
        asset_a: TestAsset::Local(41),
        asset_b: TestAsset::Local(42),
        lp_amount: AmountResolution::Fixed(10),
        min_amount_a: 1,
        min_amount_b: 1,
      },
      Task::DonateLiquidity {
        asset_a: TestAsset::Local(41),
        asset_b: TestAsset::Local(42),
        max_amount_a: AmountResolution::Fixed(10),
        max_ratio_error: Perbill::zero(),
      },
    ];
    register_lp_pair(
      TestAsset::Local(43),
      TestAsset::Local(41),
      TestAsset::Local(42),
    );

    for (index, task) in tasks.into_iter().enumerate() {
      let base = 1 + index as u64 * 2;
      frame_system::Pallet::<Test>::set_block_number(base);
      Actors::on_initialize(base);
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        contract_steps_with_step(make_step(task)),
      );
      let actor = sovereign_account(actor_id);
      fund_native(actor_id, 1_000);
      for asset in [
        TestAsset::Local(41),
        TestAsset::Local(42),
        TestAsset::Local(43),
      ] {
        set_asset_balance(&actor, asset, 100);
      }
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      frame_system::Pallet::<Test>::set_block_number(base + 1);
      Actors::on_initialize(base + 1);
      run_idle(Weight::MAX);
      assert!(
        has_actor_event(|event| matches!(
          event,
          Event::StepFailed { actor_id: id, retry_class: RetryClass::Permanent, .. }
            if *id == actor_id
        )),
        "liquidity task {index} accepted an adapter outcome outside its authored bounds"
      );
      assert!(!has_actor_event(|event| matches!(
        event,
        Event::LiquidityAdded { actor_id: id, .. }
          | Event::LiquidityRemoved { actor_id: id, .. }
          | Event::LiquidityDonated { actor_id: id, .. }
          if *id == actor_id
      )));
    }
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
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(3);
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(4);
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(5);
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
    frame_system::Pallet::<Test>::set_block_number(2);
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
    frame_system::Pallet::<Test>::set_block_number(2);
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
fn zero_amount_resolutions_and_identical_market_assets_are_rejected() {
  new_test_ext().execute_with(|| {
    for amount in [
      AmountResolution::Fixed(0),
      AmountResolution::Percent(Perbill::zero()),
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
        None,
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
    let self_leg_plan = contract_steps_with_step(make_step(Task::Transfer {
      to: dormant.sovereign_account,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
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
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(3);
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
        amount: AmountResolution::Percent(Perbill::one()),
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
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(3);
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
    frame_system::Pallet::<Test>::set_block_number(2);
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
    frame_system::Pallet::<Test>::set_block_number(2);
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
fn unstake_percentage_resolves_against_current_staking_shares() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(8);
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Unstake {
        asset,
        shares: AmountResolution::Percent(Perbill::from_percent(25)),
      }),
      make_step(Task::Unstake {
        asset,
        shares: AmountResolution::Percent(Perbill::from_percent(50)),
      }),
    ])
    .expect("system execution plan fits");
    let actor_id = create_system_with(ALICE, percentage_trigger_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 100);
    signal_percentage_trigger(actor_id, asset);
    // Canonical AddressEvent ingress publishes one Pending Service obligation at B+1, and each
    // committed Step sets the adjacent-round successor eligible one block later.
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 75);
    assert_eq!(unstaked_shares(actor, asset), 25);
    frame_system::Pallet::<Test>::set_block_number(3);
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 38);
    assert_eq!(unstaked_shares(actor, asset), 62);
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
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(3);
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
    frame_system::Pallet::<Test>::set_block_number(2);
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
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(3);
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
    frame_system::Pallet::<Test>::set_block_number(2);
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
      max_amount_a: AmountResolution::Percent(Perbill::from_percent(50)),
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
    frame_system::Pallet::<Test>::set_block_number(2);
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
      max_amount_a: AmountResolution::Percent(Perbill::from_percent(100)),
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
    frame_system::Pallet::<Test>::set_block_number(2);
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
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(3);
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
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(3);
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
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(3);
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
    frame_system::Pallet::<Test>::set_block_number(2);
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
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(3);
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
fn user_swap_then_cold_storage_transfer() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let cold_wallet: AccountId = 9999;
    let schedule = manual_schedule();
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
          amount: AmountResolution::Percent(Perbill::from_percent(80)),
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
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(3);
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

fn run_cycle_balance_block(now: MockBlockNumber) {
  frame_system::Pallet::<Test>::set_block_number(now);
  Actors::on_initialize(now);
  run_prepass();
  Actors::on_idle(now, Weight::MAX);
}

fn cycle_balance_distribution_steps() -> crate::ContractSteps<Test> {
  vec![
    make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::CaptureCycleBalance {
        percent: Perbill::from_percent(10),
        share: Perbill::from_percent(50),
      },
    }),
    make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::CycleBalancePercent(Perbill::from_percent(50)),
    }),
  ]
  .try_into()
  .unwrap()
}

#[test]
fn cycle_balance_distribution_preserves_original_volume_across_ingress() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      cycle_balance_distribution_steps(),
    );
    fund_native(id, 1_002);
    let bob = native_balance(&BOB);
    let charlie = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
    run_cycle_balance_block(2);
    let run = Actors::actor_run_state(id).unwrap();
    assert_eq!(
      run.balance_snapshot,
      Some(crate::CycleBalanceSnapshot {
        asset: TestAsset::Native,
        balance: 100
      })
    );
    assert_eq!(native_balance(&BOB), bob + 50);
    assert_eq!(native_balance(&CHARLIE), charlie);
    fund_native(id, 1_000);
    run_cycle_balance_block(3);
    assert_eq!(native_balance(&CHARLIE), charlie + 50);
    assert_eq!(native_balance(&sovereign_account(id)), 1_902);
    assert!(Actors::actor_run_state(id).is_none());
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
    run_cycle_balance_block(4);
    assert_eq!(
      Actors::actor_run_state(id)
        .unwrap()
        .balance_snapshot
        .unwrap()
        .balance,
      190
    );
  });
}

#[test]
fn cycle_balance_skipped_capture_never_recaptures_later_ingress() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut steps = cycle_balance_distribution_steps();
    steps[0].precondition = all_conditions(vec![Predicate::BalanceAbove {
      asset: TestAsset::Native,
      threshold: 2_000,
    }]);
    let id = create_system_with(ALICE, manual_schedule(), None, steps);
    fund_native(id, 1_001);
    let charlie = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
    run_cycle_balance_block(2);
    assert!(
      Actors::actor_run_state(id)
        .unwrap()
        .balance_snapshot
        .is_none()
    );
    fund_native(id, 2_000);
    run_cycle_balance_block(3);
    assert_eq!(native_balance(&CHARLIE), charlie);
    assert!(Actors::actor_run_state(id).is_none());
  });
}

#[test]
fn cycle_balance_late_failure_preserves_prefix_and_clears_snapshot_on_abort() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      cycle_balance_distribution_steps(),
    );
    fund_native(id, 1_001);
    let bob = native_balance(&BOB);
    let charlie = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
    run_cycle_balance_block(2);
    set_fail_transfer_to(Some(CHARLIE));
    run_cycle_balance_block(3);
    assert_eq!(native_balance(&BOB), bob + 50);
    assert_eq!(native_balance(&CHARLIE), charlie);
    assert!(Actors::actor_run_state(id).is_none());
  });
}

#[test]
fn cycle_balance_contract_rejects_missing_duplicate_cross_asset_and_excess_capture() {
  for case in 0..5 {
    new_test_ext().execute_with(|| {
      let mut steps = cycle_balance_distribution_steps();
      match case {
        0 => {
          let first = steps[0].clone();
          steps[0] = steps[1].clone();
          steps[1] = first;
        }
        1 => steps[1] = steps[0].clone(),
        2 => {
          if let Task::Transfer { asset, .. } = &mut steps[1].task {
            *asset = TestAsset::Local(1);
          }
        }
        3 => {
          if let Task::Transfer { amount, .. } = &mut steps[1].task {
            *amount = AmountResolution::CycleBalancePercent(Perbill::from_percent(60));
          }
        }
        _ => {
          steps[0].task = Task::Burn {
            asset: TestAsset::Native,
            amount: AmountResolution::CaptureCycleBalance {
              percent: Perbill::one(),
              share: Perbill::from_percent(50),
            },
          }
        }
      }
      assert_noop!(
        Actors::create_system_actor(
          RuntimeOrigin::root(),
          ALICE,
          Mutability::Mutable,
          system_active_contract(manual_schedule(), None, steps)
        ),
        Error::<Test>::InvalidAmountResolution
      );
    });
  }
}

#[test]
fn cycle_balance_close_removes_snapshot_and_preserves_custody() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      cycle_balance_distribution_steps(),
    );
    fund_native(id, 1_001);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
    run_cycle_balance_block(2);
    assert!(
      Actors::actor_run_state(id)
        .unwrap()
        .balance_snapshot
        .is_some()
    );
    let custody = native_balance(&sovereign_account(id));
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), id));
    assert!(Actors::actor_run_state(id).is_none());
    assert_eq!(
      native_balance(&Actors::sovereign_account_id_system(id)),
      custody
    );
  });
}

#[test]
fn cycle_balance_retry_keeps_snapshot_and_never_repeats_committed_prefix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut steps = cycle_balance_distribution_steps();
    steps[1].on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let id = create_system_with(ALICE, manual_schedule(), None, steps);
    fund_native(id, 1_001);
    let bob = native_balance(&BOB);
    let charlie = native_balance(&CHARLIE);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
    run_cycle_balance_block(2);
    set_temporary_transfer_failure_to(Some(CHARLIE));
    run_cycle_balance_block(3);
    let run = Actors::actor_run_state(id).unwrap();
    assert_eq!(run.balance_snapshot.as_ref().unwrap().balance, 100);
    assert_eq!(
      Actors::active_actor_view(id).unwrap().cycle_state,
      CycleState::Suspended
    );
    fund_native(id, 1_000);
    set_temporary_transfer_failure_to(None);
    run_cycle_balance_block(run.eligible_at);
    assert_eq!(native_balance(&BOB), bob + 50);
    assert_eq!(native_balance(&CHARLIE), charlie + 50);
    assert!(Actors::actor_run_state(id).is_none());
  });
}

#[test]
fn cycle_balance_capture_retry_preserves_the_first_attempt_volume() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut steps = cycle_balance_distribution_steps();
    steps[0].on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let id = create_system_with(ALICE, manual_schedule(), None, steps);
    fund_native(id, 1_001);
    let bob = native_balance(&BOB);
    let charlie = native_balance(&CHARLIE);
    set_temporary_transfer_failure_to(Some(BOB));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
    run_cycle_balance_block(2);
    let run = Actors::actor_run_state(id).unwrap();
    assert_eq!(run.cursor, 0);
    assert_eq!(run.balance_snapshot.as_ref().unwrap().balance, 100);
    assert_eq!(native_balance(&BOB), bob);
    fund_native(id, 1_000);
    set_temporary_transfer_failure_to(None);
    run_cycle_balance_block(run.eligible_at);
    assert_eq!(native_balance(&BOB), bob + 50);
    run_cycle_balance_block(run.eligible_at + 1);
    assert_eq!(native_balance(&CHARLIE), charlie + 50);
    assert!(Actors::actor_run_state(id).is_none());
  });
}
