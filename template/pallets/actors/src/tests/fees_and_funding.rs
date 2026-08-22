use super::*;

#[test]
fn create_user_charges_creation_fee_and_emits_event() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let fee = TestActorCreationFee::get();
    let fee_sink = TestFeeSink::get();
    let owner_before = native_balance(&ALICE);
    let sink_before = native_balance(&fee_sink);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    let inst = Actors::active_actor_view(actor_id).expect("Actors must exist");
    assert_eq!(inst.actor_class, ActorClass::User { owner_slot: 0 });
    assert_eq!(native_balance(&ALICE), owner_before.saturating_sub(fee));
    assert_eq!(native_balance(&fee_sink), sink_before.saturating_add(fee));
    assert_eq!(OwnerSlotBitmaps::<Test>::get(ALICE)[0], 0b0000_0001);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorCreated {
          actor_id: id,
          owner,
          actor_class: ActorClass::User { owner_slot: 0 },
          mutability: Mutability::Mutable,
          initial_lifecycle: InitialLifecycle::Active,
          ..
        } if *id == actor_id && *owner == ALICE
      )
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::ActorActivated { actor_id: id } if *id == actor_id)
    }));
  });
}

#[test]
fn both_user_creation_calls_charge_dormant_admission_fee_before_identity_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let fee = TestActorCreationFee::get();
    let fee_sink = TestFeeSink::get();
    let sink_before = native_balance(&fee_sink);
    let alice_before = native_balance(&ALICE);
    let charlie_before = native_balance(&CHARLIE);

    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      None,
    ));
    assert_ok!(Actors::create_user_actor_at_slot(
      RuntimeOrigin::signed(CHARLIE),
      2,
      Mutability::Mutable,
      None,
    ));

    assert_eq!(native_balance(&ALICE), alice_before.saturating_sub(fee));
    assert_eq!(native_balance(&CHARLIE), charlie_before.saturating_sub(fee));
    assert_eq!(
      native_balance(&fee_sink),
      sink_before.saturating_add(fee.saturating_mul(2))
    );
    assert_eq!(Actors::owner_slot_bitmap(ALICE)[0], 0b0000_0001);
    assert_eq!(Actors::owner_slot_bitmap(CHARLIE)[0], 0b0000_0100);
    assert_eq!(Actors::actor_identity_count(), 2);
    assert_eq!(Actors::active_actor_count(), 0);
  });
}

#[test]
fn user_active_creation_requires_prefunded_sovereign_before_opening_fee() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // An unfunded User Active creation fails InsufficientBalance before the opening fee
    // or any identity/locator mutation (spec 7.1).
    let owner_before = native_balance(&ALICE);
    let sink_before = native_balance(&TestFeeSink::get());
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::InsufficientBalance
    );
    assert_eq!(Actors::next_actor_id(), 0);
    assert_eq!(Actors::actor_identity_count(), 0);
    assert_eq!(Actors::active_actor_count(), 0);
    assert_eq!(Actors::owner_slot_bitmap(ALICE), [0; 32]);
    assert_eq!(native_balance(&ALICE), owner_before);
    assert_eq!(native_balance(&TestFeeSink::get()), sink_before);
    assert!(fee_collections().is_empty());

    // Dormant creation remains unfunded and admits with no sovereign balance.
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      None,
    ));
    assert_eq!(native_balance(&Actors::sovereign_account_id(&ALICE, 0)), 0);
  });
}

#[test]
fn user_active_creation_prefunding_boundary_is_exact_floor_plus_envelope() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let plan = transfer_contract_steps(BOB, 1);
    let requirement = user_prefunding_requirement(&plan);
    // requirement - 1 still fails closed before any opening-fee mutation.
    fund_native_raw(
      &Actors::sovereign_account_id(&ALICE, 0),
      requirement.saturating_sub(1),
    );
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, plan.clone()),
      ),
      Error::<Test>::InsufficientBalance
    );
    assert_eq!(Actors::actor_identity_count(), 0);
    assert_eq!(Actors::owner_slot_bitmap(ALICE), [0; 32]);
    // Exactly floor + envelope admits.
    fund_native_raw(&Actors::sovereign_account_id(&ALICE, 0), 1);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, plan),
    ));
    assert_eq!(Actors::active_actor_count(), 1);
  });
}

#[test]
fn create_system_does_not_charge_creation_fee() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let owner_before = native_balance(&ALICE);
    let _ = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    assert_eq!(native_balance(&ALICE), owner_before);
  });
}

#[test]
fn attempt_fee_envelope_owns_step_and_suffix_bounds() {
  new_test_ext().execute_with(|| {
    let plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
      make_step(Task::Burn {
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
    ])
    .expect("two steps fit the shared bound");
    let envelope = Actors::attempt_fee_envelope(ActorType::User, &plan, 0)
      .expect("User plan has a checked fee envelope");
    assert_eq!(envelope.steps.len(), 2);
    for cursor in 0..=plan.len() {
      let suffix = Actors::attempt_fee_envelope(ActorType::User, &plan, cursor)
        .expect("User suffix has a checked fee envelope");
      let expected = suffix.steps.iter().fold(0u128, |total, step| {
        total
          .checked_add(step.total)
          .expect("small test fees cannot overflow")
      });
      assert_eq!(suffix.total, expected);
      if cursor == 1 {
        assert_eq!(suffix.steps.len(), 1);
        assert_eq!(suffix.total, envelope.steps[1].total);
      }
    }
    let system = Actors::attempt_fee_envelope(ActorType::System, &plan, 0)
      .expect("System plan has a checked fee envelope");
    assert_eq!(system.total, 0);
    assert!(system.steps.iter().all(|step| step.total == 0));
  });
}

#[test]
fn fee_envelope_composition_is_portable_and_checked() {
  type VectorBound = frame::traits::ConstU32<2>;
  let inputs = BoundedVec::<_, VectorBound>::try_from(vec![
    FeeEnvelopeInput {
      evaluation: 2u128,
      execution: 100,
    },
    FeeEnvelopeInput {
      evaluation: 5,
      execution: 7,
    },
  ])
  .expect("portable vector inputs fit");
  let user = compose_attempt_fee_envelope(ActorType::User, &inputs, 1)
    .expect("User portable suffix composes");
  assert_eq!(user.steps[0].total, 12);
  assert_eq!(user.total, 12);
  let system = compose_attempt_fee_envelope(ActorType::System, &inputs, 0)
    .expect("System portable suffix composes");
  assert_eq!(system.total, 0);
  assert!(matches!(
    compose_attempt_fee_envelope(ActorType::User, &inputs, 3),
    Err(FeeEnvelopeError::CursorOutOfBounds)
  ));
  let overflowing = BoundedVec::<_, VectorBound>::try_from(vec![FeeEnvelopeInput {
    evaluation: u128::MAX,
    execution: 1,
  }])
  .expect("overflow vector input fits");
  assert!(matches!(
    compose_attempt_fee_envelope(ActorType::User, &overflowing, 0),
    Err(FeeEnvelopeError::Overflow)
  ));
}

#[test]
fn fee_envelope_settlement_releases_reservation_and_prices_attempts() {
  type VectorBound = frame::traits::ConstU32<2>;
  let inputs = BoundedVec::<_, VectorBound>::try_from(vec![
    FeeEnvelopeInput {
      evaluation: 2u128,
      execution: 100,
    },
    FeeEnvelopeInput {
      evaluation: 5,
      execution: 7,
    },
  ])
  .expect("settlement vector inputs fit");
  let envelope =
    compose_attempt_fee_envelope(ActorType::User, &inputs, 0).expect("User envelope composes");
  let skipped = settle_attempt_fee_step(
    ActorType::User,
    envelope.total,
    &envelope.steps[0],
    FeeChargeKind::EvaluationOnly,
  )
  .expect("first User settlement has a reservation");
  assert_eq!(skipped.charged, 2);
  assert_eq!(skipped.reservation_remaining, 12);
  let attempted = settle_attempt_fee_step(
    ActorType::User,
    skipped.reservation_remaining,
    &envelope.steps[1],
    FeeChargeKind::Attempted,
  )
  .expect("second User settlement releases the suffix to zero");
  assert_eq!(attempted.charged, 12);
  assert_eq!(attempted.reservation_remaining, 0);
  assert!(matches!(
    settle_attempt_fee_step(
      ActorType::User,
      11,
      &envelope.steps[1],
      FeeChargeKind::Attempted,
    ),
    Err(FeeEnvelopeError::ReservationUnderflow)
  ));
  let system = settle_attempt_fee_step(
    ActorType::System,
    0,
    &envelope.steps[0],
    FeeChargeKind::Attempted,
  )
  .expect("System settlement remains fee-exempt");
  assert_eq!(system.charged, 0);
  assert_eq!(system.reservation_remaining, 0);
}

#[test]
fn fee_native_protected_minimum_uses_the_configured_user_fee_native_floor() {
  assert_eq!(
    fee_native_protected_minimum(ActorType::User, true, 1u128, 50),
    50
  );
  assert_eq!(
    fee_native_protected_minimum(ActorType::User, true, 100u128, 50),
    50
  );
  assert_eq!(
    fee_native_protected_minimum(ActorType::User, false, 1u128, 50),
    1
  );
  assert_eq!(
    fee_native_protected_minimum(ActorType::System, true, 100u128, 50),
    100
  );
}

#[test]
fn user_cycle_weight_includes_one_fee_collection_per_step() {
  new_test_ext().execute_with(|| {
    let contract_steps = inert_contract_steps();
    let system_weight = Actors::compute_cycle_weight_upper(ActorType::System, &contract_steps);
    let user_weight = Actors::compute_cycle_weight_upper(ActorType::User, &contract_steps);
    assert_eq!(
      user_weight,
      system_weight.saturating_add(<TestWeightInfo as crate::WeightInfo>::fee_collection())
    );
  });
}

#[test]
fn system_sovereign_reattachment_creates_fresh_identity_with_same_custody() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let first_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let original_sovereign = sovereign_account(first_id);
    let _ = Balances::deposit_creating(&original_sovereign, 777);
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), first_id));
    assert_eq!(
      Actors::system_sovereigns(first_id),
      Some(SystemSovereignState::Vacant)
    );

    let fresh_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor_at_sovereign_id(
      RuntimeOrigin::root(),
      first_id,
      ALICE,
      Mutability::Mutable,
      system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
    ));
    let fresh = Actors::active_actor_view(fresh_id).expect("fresh System identity exists");
    assert_ne!(fresh_id, first_id);
    assert_eq!(fresh.sovereign_account, original_sovereign);
    assert_eq!(
      fresh.actor_class,
      ActorClass::System {
        sovereign_id: first_id
      }
    );
    assert_eq!(fresh.cycle_nonce, 0);
    assert_eq!(native_balance(&original_sovereign), 777);
    assert_eq!(
      Actors::system_sovereigns(first_id),
      Some(SystemSovereignState::Occupied(fresh_id))
    );
  });
}

#[test]
fn system_sovereign_reattachment_accepts_dormant_contract_input() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let first_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let sovereign = sovereign_account(first_id);
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), first_id));
    let fresh_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor_at_sovereign_id(
      RuntimeOrigin::root(),
      first_id,
      ALICE,
      Mutability::Mutable,
      None,
    ));
    let identity = Actors::actor_identities(fresh_id).expect("dormant identity exists");
    assert_eq!(identity.sovereign_account, sovereign);
    assert_eq!(
      identity.actor_class,
      ActorClass::System {
        sovereign_id: first_id
      }
    );
  });
}

#[test]
fn system_sovereign_reattachment_requires_allocated_vacant_locator() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_noop!(
      Actors::create_system_actor_at_sovereign_id(
        RuntimeOrigin::root(),
        42,
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::SystemSovereignUnknown
    );
    let occupied_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_noop!(
      Actors::create_system_actor_at_sovereign_id(
        RuntimeOrigin::root(),
        occupied_id,
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::SystemSovereignOccupied
    );
  });
}

#[test]
fn reserved_sovereign_account_rejects_creation_at_that_slot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // The host marks a derived sovereign account as reserved; creation at the slot whose
    // sovereign would alias it must fail closed with SovereignAccountCollision before any
    // identity or fee mutation.
    let slot = 4u8;
    let sovereign = Actors::sovereign_account_id(&ALICE, slot);
    set_reserved_sovereign_account(sovereign);
    let alice_before = native_balance(&ALICE);
    assert_noop!(
      Actors::create_user_actor_at_slot(
        RuntimeOrigin::signed(ALICE),
        slot,
        Mutability::Mutable,
        None,
      ),
      Error::<Test>::ReservedSovereignAccount
    );
    assert_eq!(native_balance(&ALICE), alice_before);
    assert_eq!(Actors::actor_identity_count(), 0);
    assert_eq!(
      Actors::owner_slot_bitmap(ALICE)[slot as usize / 8] & (1 << (slot % 8)),
      0
    );
  });
}

#[test]
fn create_user_at_slot_reuses_same_sovereign_after_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let target_slot = 3;
    let first_id = create_user_with_slot(
      ALICE,
      target_slot,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let first = Actors::active_actor_view(first_id).expect("first Actors exists");
    assert_eq!(first.actor_class.owner_slot(), Some(target_slot));
    assert_eq!(
      first.sovereign_account,
      Actors::sovereign_account_id(&ALICE, target_slot)
    );
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), first_id));
    let second_id = create_user_with_slot(
      ALICE,
      target_slot,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let second = Actors::active_actor_view(second_id).expect("second Actors exists");
    assert_eq!(second.actor_class.owner_slot(), Some(target_slot));
    assert_eq!(second.sovereign_account, first.sovereign_account);
  });
}

#[test]
fn creation_fee_route_failure_rolls_back_actor_creation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let active_before = Actors::active_actor_count();
    let owner_before = native_balance(&ALICE);
    set_fail_fee_sink_transfer(true);
    prefund_active_user_creation(ALICE, &transfer_contract_steps(BOB, 1));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::InsufficientFee
    );
    assert_noop!(
      Actors::create_user_actor_at_slot(
        RuntimeOrigin::signed(CHARLIE),
        2,
        Mutability::Mutable,
        None,
      ),
      Error::<Test>::InsufficientFee
    );
    set_fail_fee_sink_transfer(false);
    assert_eq!(Actors::active_actor_count(), active_before);
    assert_eq!(Actors::actor_identity_count(), 0);
    assert_eq!(Actors::next_actor_id(), 0);
    assert_eq!(Actors::owner_slot_bitmap(ALICE), [0; 32]);
    assert_eq!(Actors::owner_slot_bitmap(CHARLIE), [0; 32]);
    assert_eq!(native_balance(&ALICE), owner_before);
  });
}

#[test]
fn failed_pre_opening_weight_admission_preserves_latch_and_funding() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE,
    ));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    let queue_weight = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1)
      .saturating_add(Actors::scheduler_actor_probe_weight_upper())
      .saturating_add(
        <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
          .max(<<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page()),
      );
    // Only the RefTime dimension is exhausted; ProofSize remains unlimited.
    let ref_time_limit = queue_weight
      .ref_time()
      .saturating_add(Actors::attempt_weight_upper_bound(&instance, 0).ref_time())
      .saturating_sub(1);
    Actors::execute_cycle(Weight::from_parts(ref_time_limit, u64::MAX));
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(instance.pending_signal);
    assert_eq!(instance.cycle_nonce, 0);
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&100),
    );
  });
}

#[test]
fn cycle_closes_with_fee_budget_exhausted_when_unfunded() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let prefunded = user_prefunding_requirement(&transfer_contract_steps(BOB, 10));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    deplete_user_sovereign(actor_id, prefunded);
    fund_native(actor_id, TestMinUserBalance::get());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::FeeBudgetExhausted,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn balance_exhausted_takes_precedence_over_fee_budget_exhausted() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let prefunded = user_prefunding_requirement(&transfer_contract_steps(BOB, 10));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    deplete_user_sovereign(actor_id, prefunded);
    fund_native(actor_id, TestMinUserBalance::get() - 1);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::BalanceExhausted,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn fee_insufficiency_is_terminal_without_deferral_guard() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let prefunded = user_prefunding_requirement(&transfer_contract_steps(BOB, 10));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    deplete_user_sovereign(actor_id, prefunded);
    fund_native(actor_id, TestMinUserBalance::get());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
  });
}

#[test]
fn condition_skip_fee_route_failure_aborts_before_skip_event() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: Balance::MAX,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(step),
    );
    fund_native(actor_id, 1_000_000_000);
    let bob_before = native_balance(&BOB);
    let fee_sink_before = native_balance(&TestFeeSink::get());
    clear_fee_collections();
    set_fail_fee_sink_transfer(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    set_fail_fee_sink_transfer(false);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&TestFeeSink::get()), fee_sink_before);
    assert_eq!(fee_collections(), vec![Actors::compute_eval_fee(1)]);
    assert!(!has_actor_event(|event| {
      matches!(event, Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id)
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::StepSkipped { actor_id: id, step_index: 0, .. } if *id == actor_id)
    }));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor remains")
        .cycle_nonce,
      0
    );
  });
}

#[test]
fn combined_step_fee_route_failure_aborts_before_task_execution() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let task = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(task.clone())),
    );
    fund_native(actor_id, 1_000_000_000);
    let bob_before = native_balance(&BOB);
    let fee_sink_before = native_balance(&TestFeeSink::get());
    clear_fee_collections();
    set_fail_fee_sink_transfer(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    set_fail_fee_sink_transfer(false);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&TestFeeSink::get()), fee_sink_before);
    let expected = Actors::compute_eval_fee(0).saturating_add(TestWeightToFee::weight_to_fee(
      &Actors::weight_upper_bound(&task),
    ));
    assert_eq!(fee_collections(), vec![expected]);
    assert!(!has_actor_event(|event| {
      matches!(event, Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id)
    }));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor remains")
        .cycle_nonce,
      0
    );
  });
}

#[test]
fn typed_ingress_absent_destination_is_balance_only() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let event = crate::AddressEvent {
      destination: BOB,
      source: Some(ALICE),
      asset: TestAsset::Native,
      amount: 100,
      provenance: Some(crate::FundingProvenance::Signed),
    };
    // A movement to a non-sovereign destination is balance-only: the typed
    // boundary accepts it without lifecycle, funding, trigger, or placement work.
    assert_ok!(Actors::preflight_ingress(&event));
    assert_ok!(Actors::notify_ingress(&event));
  });
}

#[test]
fn expired_ingress_remains_balance_only_and_closes_inline() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let window = ScheduleWindow { start: 1, end: 101 };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      Some(window),
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    let actor = sovereign_account(actor_id);
    let balance_before = native_balance(&actor);
    frame_system::Pallet::<Test>::set_block_number(102);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      1_000
    ));
    assert_eq!(native_balance(&actor), balance_before.saturating_add(1_000));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(Actors::actor_funding(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::WindowExpired,
      } if *id == actor_id
    )));
  });
}

#[test]
fn window_expired_takes_precedence_over_balance_exhausted() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let window = ScheduleWindow { start: 1, end: 101 };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(window),
      transfer_contract_steps(BOB, 1),
    );
    frame_system::Pallet::<Test>::set_block_number(102);
    assert_ok!(Actors::permissionless_sweep(
      RuntimeOrigin::signed(ALICE),
      actor_id,
    ));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::WindowExpired,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn stop_cycle_fee_failure_rolls_back_before_step_policy_or_stop() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    fund_native(actor_id, 100_000);
    let sink_before = native_balance(&TestFeeSink::get());
    set_fail_fee_sink_transfer(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStopped { actor_id: id, .. } if *id == actor_id
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleSummary { actor_id: id, .. } if *id == actor_id
    )));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor remains active")
        .unsuccessful_attempt_streak,
      0
    );
    assert_eq!(native_balance(&TestFeeSink::get()), sink_before);

    set_fail_fee_sink_transfer(false);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    let task: TaskOf<Test> = Task::StopCycle;
    let task_weight = Actors::weight_upper_bound(&task);
    let expected_fee = Actors::compute_eval_fee(0).saturating_add(
      <TestWeightToFee as polkadot_sdk::sp_weights::WeightToFee>::weight_to_fee(&task_weight),
    );
    assert_eq!(
      native_balance(&TestFeeSink::get()),
      sink_before.saturating_add(expected_fee)
    );
    assert_eq!(fee_collections().last(), Some(&expected_fee));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleStopped {
        actor_id: id,
        cycle_nonce: 1,
        step_index: 0,
      } if *id == actor_id
    )));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("successful actor remains active")
        .unsuccessful_attempt_streak,
      0
    );
  });
}

#[test]
fn simulation_projects_fee_collection_failure_as_interface_error_and_rolls_back() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = transfer_contract_steps(BOB, 10);
    let contract = user_active_contract(manual_schedule(), None, contract_steps.clone())
      .expect("direct Actor Contract");
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    fund_native(actor_id, 100_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let actor_before = Actors::active_actor_view(actor_id).expect("actor before simulation");
    let events_before = System::events();
    let sink_before = native_balance(&TestFeeSink::get());
    set_fail_fee_sink_transfer(true);

    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::User,
        Mutability::Mutable,
        contract,
        SimulationMode::FreshCurrentPlan,
      ),
      Err(SimulationError::FeeCollectionFailed)
    );

    set_fail_fee_sink_transfer(false);
    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(System::events(), events_before);
    assert_eq!(native_balance(&TestFeeSink::get()), sink_before);
  });
}

#[test]
fn retry_later_funding_unavailable_resumes_without_new_logical_run() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut contract_steps = transfer_contract_steps(BOB, 50);
    contract_steps[0].on_error = RETRY_LATER;
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    let bob_before = native_balance(&BOB);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let suspended = Actors::continuation_state(actor_id).expect("funding retry persists");
    assert_eq!(suspended.cursor, 0);
    assert_eq!(suspended.cumulative_outcomes.failed_steps, 0);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor remains")
        .unsuccessful_attempt_streak,
      1
    );

    fund_native_raw(&actor, 51);
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    let completed = Actors::active_actor_view(actor_id).expect("actor completes");
    assert_eq!(completed.cycle_nonce, 1);
    assert_eq!(completed.cycle_state, CycleState::Idle);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before + 50);
  });
}

#[test]
fn capped_exponential_balances_retry_pressure_and_recovery_at_maximum_occupancy() {
  const HORIZON: u32 = 64;
  const MAX_DELAY: u32 = 8;
  let max_occupancy = <<Test as crate::Config>::MaxActiveActors as Get<u32>>::get() as u64;

  let due_blocks = |fixed_delay: Option<u32>| {
    let mut blocks = Vec::new();
    let mut due = 0u32;
    let mut attempt = 0u32;
    while due <= HORIZON.saturating_add(MAX_DELAY) {
      let delay = fixed_delay.unwrap_or_else(|| Actors::retry_backoff_blocks(attempt));
      due = due.checked_add(delay).expect("bounded experiment horizon");
      blocks.push(due);
      attempt = attempt.checked_add(1).expect("bounded experiment attempts");
    }
    blocks
  };
  let metrics = |fixed_delay| {
    let due = due_blocks(fixed_delay);
    let attempts_per_actor = due.iter().filter(|block| **block <= HORIZON).count() as u64;
    let recovery_wait_sum = (1..=HORIZON)
      .map(|available_at| {
        due
          .iter()
          .copied()
          .find(|block| *block >= available_at)
          .expect("experiment extends through the capped delay")
          - available_at
      })
      .sum::<u32>();
    (
      attempts_per_actor.saturating_mul(max_occupancy),
      recovery_wait_sum,
      max_occupancy,
    )
  };

  let exponential = metrics(None);
  let fixed_one = metrics(Some(1));
  let fixed_four = metrics(Some(4));
  let fixed_eight = metrics(Some(8));
  let evidence: serde_json::Value = serde_json::from_str(include_str!(
    "../../tests/fixtures/retry-backoff-decision.v1.json"
  ))
  .expect("retry-backoff decision fixture parses");
  let expected = |policy: &str, metric: &str| {
    evidence[policy][metric]
      .as_u64()
      .expect("decision metric is an unsigned integer")
  };
  assert_eq!(
    exponential.0,
    expected("cappedExponential", "dueRetryObligations")
  );
  assert_eq!(
    u64::from(exponential.1),
    expected("cappedExponential", "recoveryWaitSum")
  );
  assert_eq!(fixed_one.0, expected("fixedDelay1", "dueRetryObligations"));
  assert_eq!(
    u64::from(fixed_one.1),
    expected("fixedDelay1", "recoveryWaitSum")
  );
  assert_eq!(fixed_four.0, expected("fixedDelay4", "dueRetryObligations"));
  assert_eq!(
    u64::from(fixed_four.1),
    expected("fixedDelay4", "recoveryWaitSum")
  );
  assert_eq!(
    fixed_eight.0,
    expected("fixedDelay8", "dueRetryObligations")
  );
  assert_eq!(
    u64::from(fixed_eight.1),
    expected("fixedDelay8", "recoveryWaitSum")
  );
  assert_eq!(evidence["decision"], "retain-capped-exponential");
  assert!(exponential.0 < fixed_four.0);
  assert!(exponential.1 < fixed_eight.1);
  assert_eq!(
    exponential.2, fixed_eight.2,
    "delay policy changes neither maximum occupancy nor FIFO cohort fairness"
  );
}

#[test]
fn funding_arrival_during_suspension_accumulates_for_the_next_cycle() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: None,
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      },
      on_error: RETRY_LATER,
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(step),
    );
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("suspended")
        .cycle_state,
      CycleState::Suspended
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSuspended {
        actor_id: id,
        cycle_nonce: 1,
        cursor: 0,
        reason: SuspensionReason::FundingUnavailable,
        ..
      } if *id == actor_id
    )));
    let retry_ticket = Actors::actor_hot(actor_id)
      .expect("suspended actor")
      .queue_ticket;

    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      7,
      &ALICE
    ));
    let funding = actor_funding(actor_id);
    assert_eq!(
      funding.funding_accumulated.get(&TestAsset::Native),
      Some(&7)
    );
    assert_eq!(
      Actors::actor_hot(actor_id)
        .expect("suspended actor")
        .queue_ticket,
      retry_ticket
    );
  });
}

#[test]
fn suspended_cycle_freezes_its_funding_snapshot_and_preserves_new_accumulation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_in = TestAsset::Local(5);
    let asset_out = TestAsset::Local(6);
    setup_pool(asset_in, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let step = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in,
        asset_out,
        amount_in: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
        slippage_tolerance: Perbill::one(),
      },
      on_error: RETRY_LATER,
    };
    let actor_id = create_system_with(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      contract_steps_with_step(step),
    );
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_in, 100);
    assert_ok!(Actors::notify_address_event(
      actor_id, asset_in, 100, &ALICE
    ));
    set_temporary_dex_failure(true);
    run_idle(Weight::MAX);
    let continuation = Actors::continuation_state(actor_id).expect("suspended");
    assert!(continuation.opening_snapshot.is_empty());
    assert_eq!(continuation.funding_snapshot.get(&asset_in), Some(&100));

    set_asset_balance(&actor, asset_in, 30);
    assert_ok!(Actors::notify_address_event(actor_id, asset_in, 30, &ALICE));
    assert_eq!(
      actor_funding(actor_id).funding_accumulated.get(&asset_in),
      Some(&30)
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::continuation_state(actor_id)
        .expect("still suspended")
        .funding_snapshot
        .get(&asset_in),
      Some(&100)
    );
    assert_eq!(
      actor_funding(actor_id).funding_accumulated.get(&asset_in),
      Some(&30)
    );

    set_asset_balance(&actor, asset_in, 100);
    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(4);
    run_idle(Weight::MAX);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert_eq!(
      actor_funding(actor_id).funding_accumulated.get(&asset_in),
      Some(&30)
    );
  });
}

#[test]
fn update_contract_prunes_stale_funding_accumulators() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let initial_contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      initial_contract_steps,
    );
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      100
    ));
    assert!(
      actor_funding(actor_id)
        .funding_accumulated
        .contains_key(&TestAsset::Native)
    );
    let replacement = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(1),
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      replacement,
      crate::CompletionPolicy::Persistent,
    ));
    let funding_after = actor_funding(actor_id);
    assert!(
      !funding_after
        .funding_accumulated
        .contains_key(&TestAsset::Native)
    );
    assert!(
      funding_after
        .funding_tracked_assets
        .contains(&TestAsset::Local(1))
    );
  });
}

#[test]
fn permissionless_sweep_closes_user_below_min_balance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let prefunded = user_prefunding_requirement(&transfer_contract_steps(BOB, 1));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    deplete_user_sovereign(actor_id, prefunded);
    assert_ok!(Actors::permissionless_sweep(
      RuntimeOrigin::signed(BOB),
      actor_id
    ));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::BalanceExhausted,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn user_resolution_skip_charges_only_eval_fee() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfCurrent(Perbill::from_parts(1)),
    }));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 1_000);
    let before = native_balance(&actor);
    clear_fee_collections();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let after = native_balance(&actor);
    assert_eq!(after, before.saturating_sub(Actors::compute_eval_fee(0)));
    assert_eq!(fee_collections(), vec![Actors::compute_eval_fee(0)]);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::ResolutionSkipped,
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn condition_skip_charges_one_evaluation_fee() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: Balance::MAX,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(step),
    );
    fund_native(actor_id, 1_000);
    clear_fee_collections();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(fee_collections(), vec![Actors::compute_eval_fee(1)]);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepSkipped {
        actor_id: id,
        reason: StepSkippedReason::PreconditionFalse,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn balance_conditions_never_read_staking_share_surfaces() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let plan = contract_steps_with_step(StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 0,
      }]),
      task: Task::StopCycle,
      on_error: StepErrorPolicy::AbortCycle,
    });
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    fund_native(actor_id, 10);

    assert_eq!(staking_share_balance_reads(), 0);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(staking_share_balance_reads(), 0);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        result: CycleResult::Completed,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn true_balance_condition_can_precede_funding_unavailable_resolution() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let task = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(StepOf::<Test> {
        precondition: all_conditions(vec![Predicate::BalanceAbove {
          asset: TestAsset::Native,
          threshold: 1,
        }]),
        task,
        on_error: StepErrorPolicy::ContinueNextStep,
      }),
    );
    fund_native(actor_id, 1_000);
    clear_fee_collections();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(fee_collections(), vec![Actors::compute_eval_fee(1)]);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepSkipped {
        actor_id: id,
        reason: StepSkippedReason::FundingUnavailable,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn executable_task_charges_eval_and_execution_fees() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let task = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    };
    let contract_steps = contract_steps_with_step(make_step(task.clone()));
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 1_000);
    let actor_before = native_balance(&actor);
    let fee_sink_before = native_balance(&TestFeeSink::get());
    let task_weight = Actors::weight_upper_bound(&task);
    assert!(task_weight.ref_time() > 0);
    let expected_fee =
      Actors::compute_eval_fee(0).saturating_add(TestWeightToFee::weight_to_fee(&task_weight));
    let instance = Actors::active_actor_view(actor_id).expect("user actor");
    assert_eq!(Actors::attempt_fee_upper_bound(&instance, 0), expected_fee);
    clear_fee_collections();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&actor),
      actor_before.saturating_sub(expected_fee).saturating_sub(1)
    );
    assert_eq!(
      native_balance(&TestFeeSink::get()),
      fee_sink_before.saturating_add(expected_fee)
    );
    assert_eq!(fee_collections(), vec![expected_fee]);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::CycleSummary {
          actor_id: id,
          outcomes: OutcomeTotals {
            executed_steps: 1,
            precondition_skips: 0,
            skipped_resolution: 0,
            skipped_funding_unavailable: 0,
            failed_steps: 0,
            ..
          },
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn adapter_failure_retains_one_combined_step_fee() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let foreign = TestAsset::Local(77);
    let pool_account: AccountId = u64::MAX;
    setup_pool(TestAsset::Native, foreign, 10_000, 10_000);
    fund_native_raw(&pool_account, 10_000);
    set_asset_balance(&pool_account, foreign, 10_000);
    let task = Task::SwapIn {
      asset_in: TestAsset::Native,
      asset_out: foreign,
      amount_in: AmountResolution::Fixed(100),
      slippage_tolerance: Perbill::from_percent(10),
    };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(task.clone())),
    );
    let actor = sovereign_account(actor_id);
    fund_native_raw(&actor, 1_000);
    let actor_before = native_balance(&actor);
    let pool_before = native_balance(&pool_account);
    let expected = Actors::compute_eval_fee(0).saturating_add(TestWeightToFee::weight_to_fee(
      &Actors::weight_upper_bound(&task),
    ));
    clear_fee_collections();
    set_fail_dex_after_input_transfer(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    set_fail_dex_after_input_transfer(false);
    assert_eq!(fee_collections(), vec![expected]);
    assert_eq!(
      native_balance(&actor),
      actor_before.saturating_sub(expected)
    );
    assert_eq!(native_balance(&pool_account), pool_before);
    assert!(has_actor_event(|event| {
      matches!(event, Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id)
    }));
  });
}

#[test]
fn cycle_summary_fee_fairness_property_matrix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let cases = [
      (1_000u128, Perbill::from_parts(1), true),
      (1_000u128, Perbill::from_percent(10), false),
      (10_000u128, Perbill::from_percent(50), false),
    ];
    let eval_fee = Actors::compute_eval_fee(0);
    for (idx, (funding, pct, expect_skip)) in cases.into_iter().enumerate() {
      let task = Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfCurrent(pct),
      };
      let contract_steps = contract_steps_with_step(make_step(task.clone()));
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        manual_schedule(),
        None,
        contract_steps,
      );
      fund_native(actor_id, funding);
      let fee_sink_before = native_balance(&TestFeeSink::get());
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      run_idle(Weight::MAX);
      let fee_sink_after = native_balance(&TestFeeSink::get());
      let fee_delta = fee_sink_after.saturating_sub(fee_sink_before);
      let summary = frame_system::Pallet::<Test>::events()
        .into_iter()
        .rev()
        .find_map(|record| match record.event {
          RuntimeEvent::Actors(Event::CycleSummary {
            actor_id: id,
            outcomes,
            ..
          }) if id == actor_id => Some((
            outcomes.executed_steps,
            outcomes.precondition_skips,
            outcomes.skipped_resolution,
            outcomes.skipped_funding_unavailable,
            outcomes.failed_steps,
          )),
          _ => None,
        })
        .expect("CycleSummary must be emitted");
      if expect_skip {
        assert_eq!(summary.0, 0);
        assert_eq!(summary.1, 0);
        assert_eq!(summary.2, 1);
        assert_eq!(summary.3, 0);
        assert_eq!(summary.4, 0);
        assert_eq!(fee_delta, eval_fee);
      } else {
        let exec_fee = <TestWeightToFee as polkadot_sdk::sp_weights::WeightToFee>::weight_to_fee(
          &Actors::weight_upper_bound(&task),
        );
        assert_eq!(summary.0, 1);
        assert_eq!(summary.1, 0);
        assert_eq!(summary.2, 0);
        assert_eq!(summary.3, 0);
        assert_eq!(summary.4, 0);
        assert_eq!(fee_delta, eval_fee.saturating_add(exec_fee));
      }
      frame_system::Pallet::<Test>::set_block_number((idx as u64).saturating_add(2));
    }
  });
}

#[test]
fn percentage_of_last_funding_consumes_each_cycle_open_snapshot() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let bob_before = native_balance(&BOB);
    let actor = sovereign_account(actor_id);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      100
    ));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&100)
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 1);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(native_balance(&actor), 50);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(CHARLIE),
      actor_id,
      TestAsset::Native,
      200
    ));
    assert_eq!(native_balance(&actor), 250);
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&200)
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 2);
    let inst = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(inst.cycle_nonce, 2);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(150));
    assert!(actor_funding(actor_id).funding_accumulated.is_empty());
  });
}

#[test]
fn system_keeps_running_on_last_funding_exhaustion_and_accepts_refill() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      100
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 1);
    assert_eq!(native_balance(&actor), 50);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 2);
    assert_eq!(native_balance(&actor), 50);
    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 3);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == actor_id
      )
    }));
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(instance.lifecycle, ActiveLifecycle::Active);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(CHARLIE),
      actor_id,
      TestAsset::Native,
      80
    ));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&80)
    );
  });
}

#[test]
fn user_closes_when_the_full_fee_envelope_cannot_fit_above_the_floor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      500
    ));
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    // After the first run transfers 250 and charges the fee envelope (~101), the
    // remaining balance cannot fit the full envelope above MinUserBalance, so the
    // second run is NOT admitted and the actor closes with FeeBudgetExhausted
    // instead of running with a step skip (spec 5.2.1).
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::ActorClosed {
          actor_id: id,
          reason: CloseReason::FeeBudgetExhausted,
          ..
        } if *id == actor_id
      )
    }));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(250));
  });
}

#[test]
fn swap_out_absolute_input_is_a_cap_not_a_balance_gate() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_in = TestAsset::Local(1);
    let asset_out = TestAsset::Local(2);
    set_pool_reserves(asset_in, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::SwapOut {
        asset_out,
        amount_out: AmountResolution::Fixed(100),
        asset_in,
        input_limit: InputLimit::Absolute(1_000),
        slippage_tolerance: Perbill::zero(),
      })),
    );
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_in, 200);
    let input_before = asset_balance(&actor, asset_in);
    let output_before = asset_balance(&actor, asset_out);

    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);

    let input_spent = input_before.saturating_sub(asset_balance(&actor, asset_in));
    assert!(input_spent > 0 && input_spent <= 200);
    assert!(asset_balance(&actor, asset_out) >= output_before.saturating_add(100));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::SwapExecuted { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn default_funding_policies_authorize_system_runtime_sources_but_only_user_owner() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps_sys = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let contract_steps_usr = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let system_actor = create_system_with(ALICE, manual_schedule(), None, contract_steps_sys);
    let user_actor = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_usr,
    );
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      system_actor,
      TestAsset::Native,
      100
    ));
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      user_actor,
      TestAsset::Native,
      100
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::notify_address_event(
      system_actor,
      TestAsset::Native,
      500,
      &CHARLIE
    ));
    assert_ok!(Actors::notify_address_event(
      user_actor,
      TestAsset::Native,
      500,
      &CHARLIE
    ));
    assert_ok!(Actors::notify_address_event(
      user_actor,
      TestAsset::Native,
      25,
      &ALICE
    ));
    assert_ok!(Actors::notify_internal_address_event(
      user_actor,
      TestAsset::Native,
      30,
      &ALICE
    ));
    assert_ok!(Actors::notify_xcm_address_event(
      user_actor,
      TestAsset::Native,
      35,
      &ALICE
    ));
    let sys_inst = actor_funding(system_actor);
    assert!(matches!(
      ActorContracts::<Test>::get(system_actor)
        .expect("system Actor Contract")
        .funding,
      FundingSourcePolicy::RuntimePolicy
    ));
    assert_eq!(
      sys_inst.funding_accumulated.get(&TestAsset::Native),
      Some(&600)
    );
    let user_inst = actor_funding(user_actor);
    assert!(matches!(
      ActorContracts::<Test>::get(user_actor)
        .expect("user Actor Contract")
        .funding,
      FundingSourcePolicy::OwnerOnly
    ));
    assert_eq!(
      user_inst.funding_accumulated.get(&TestAsset::Native),
      Some(&125)
    );
  });
}

#[test]
fn trigger_source_and_funding_provenance_are_evaluated_independently() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      on_address_event_schedule(SourceFilter::OwnerOnly, AssetFilter::Any),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::notify_internal_address_event(
      actor_id,
      TestAsset::Native,
      25,
      &ALICE,
    ));
    assert!(
      Actors::actor_hot(actor_id)
        .expect("actor hot state")
        .pending_signal,
      "verified source must satisfy trigger filtering independently"
    );
    assert!(
      actor_funding(actor_id).funding_accumulated.is_empty(),
      "InternalProtocol provenance must not satisfy OwnerOnly funding"
    );
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::FundingAccumulated { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn untracked_credit_can_trigger_without_allocating_funding_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    frame_system::Pallet::<Test>::reset_events();

    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Local(99),
      25,
      &ALICE,
    ));

    assert!(Actors::actor_hot(actor_id).is_some_and(|hot| hot.pending_signal));
    assert!(actor_funding(actor_id).funding_accumulated.is_empty());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::FundingAccumulated { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn one_ingress_matching_the_single_source_mutates_funding_once() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let trigger = Trigger::address_event(SourceFilter::Any, AssetFilter::Any);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      Schedule {
        trigger,
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );

    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE
    ));

    let hot = Actors::actor_hot(actor_id).expect("actor hot state");
    assert!(hot.pending_signal);
    assert!(hot.queue_ticket.is_some());
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&100)
    );
    assert_eq!(
      frame_system::Pallet::<Test>::events()
        .into_iter()
        .filter(|record| matches!(
          record.event,
          RuntimeEvent::Actors(Event::FundingAccumulated { actor_id: id, .. }) if id == actor_id
        ))
        .count(),
      1
    );
  });
}

#[test]
fn identical_authoritative_transfers_remain_distinct_funding_events() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    let first_ticket = Actors::actor_hot(actor_id)
      .expect("actor hot state")
      .queue_ticket;
    assert!(first_ticket.is_some());
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    assert_eq!(
      Actors::actor_hot(actor_id)
        .expect("actor hot state")
        .queue_ticket,
      first_ticket,
      "an already-pending signal must retain one live FIFO ticket"
    );
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&200)
    );
  });
}

#[test]
fn direct_notification_reports_funding_overflow_without_partial_readiness_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    crate::ActorFunding::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("user actor funding")
        .funding_accumulated
        .try_insert(TestAsset::Native, u128::MAX)
        .expect("funding accumulator fits");
    });
    assert_noop!(
      Actors::notify_address_event(actor_id, TestAsset::Native, 1, &ALICE),
      Error::<Test>::FundingAccumulatorOverflow
    );
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&u128::MAX)
    );
    assert!(!Actors::actor_hot(actor_id).is_some_and(|hot| hot.pending_signal));
  });
}

#[test]
fn signed_allowlist_accepts_only_verified_listed_signers_for_funding() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    let allowed = BoundedBTreeSet::try_from([CHARLIE].into_iter().collect::<BTreeSet<_>>())
      .expect("one funding signer fits");
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      FundingSourcePolicy::SignedAllowlist(allowed),
    ));
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &CHARLIE
    ));
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      900,
      &BOB
    ));
    assert_ok!(Actors::notify_internal_address_event(
      actor_id,
      TestAsset::Native,
      700,
      &CHARLIE
    ));
    assert_ok!(Actors::notify_address_event_without_source(
      actor_id,
      TestAsset::Native,
      500
    ));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&100)
    );
  });
}

#[test]
fn immutable_user_cannot_update_contract_funding() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Immutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        FundingSourcePolicy::AnyVerifiedIngress
      ),
      Error::<Test>::ImmutableActor
    );
  });
}

#[test]
fn multi_asset_funding_accumulators_are_independent() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // ContractSteps with TWO assets using PercentageOfLastFunding
    let step1 = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    });
    let step2 = make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Local(1),
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(25)),
    });
    let contract_steps = BoundedVec::try_from(vec![step1, step2]).expect("contract_steps fits");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    // Verify both assets are tracked
    let inst = actor_funding(actor_id);
    assert!(inst.funding_tracked_assets.contains(&TestAsset::Native));
    assert!(inst.funding_tracked_assets.contains(&TestAsset::Local(1)));
    assert_eq!(inst.funding_tracked_assets.len(), 2);
    // Transfer Native first
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      1000
    ));
    let inst = actor_funding(actor_id);
    assert_eq!(
      inst.funding_accumulated.get(&TestAsset::Native),
      Some(&1000)
    );
    assert!(inst.funding_accumulated.get(&TestAsset::Local(1)).is_none());
    // Fund Local(1) separately
    frame_system::Pallet::<Test>::set_block_number(2);
    <crate::mock::MockAssetOps as crate::adapters::AssetOps<AccountId, TestAsset, Balance>>::mint(
      &ALICE,
      TestAsset::Local(1),
      400,
    )
    .unwrap();
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Local(1),
      400
    ));
    let inst = actor_funding(actor_id);
    assert_eq!(
      inst.funding_accumulated.get(&TestAsset::Native),
      Some(&1000)
    );
    assert_eq!(
      inst.funding_accumulated.get(&TestAsset::Local(1)),
      Some(&400)
    );
    // Another Native transfer accumulates only the Native asset.
    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      500
    ));
    let inst = actor_funding(actor_id);
    assert_eq!(
      inst.funding_accumulated.get(&TestAsset::Native),
      Some(&1500)
    );
    assert_eq!(
      inst.funding_accumulated.get(&TestAsset::Local(1)),
      Some(&400)
    );
  });
}

#[test]
fn sovereign_account_collision_rejected() {
  new_test_ext().execute_with(|| {
    let contract_steps = transfer_contract_steps(BOB, 10);
    let _actor_id = Actors::next_actor_id();
    let sovereign = Actors::sovereign_account_id(&ALICE, 0);
    SovereignIndex::<Test>::insert(&sovereign, 999u64);
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, contract_steps),
      ),
      Error::<Test>::SovereignAccountCollision
    );
  });
}

#[test]
fn funding_policy_replacement_preserves_placement_without_continuation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(10), None, inert_contract_steps());
    let before = Actors::actor_hot(actor_id).expect("actor hot state exists");
    assert!(before.wakeup_pointer.is_some());
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress,
    ));
    let after = Actors::actor_hot(actor_id).expect("actor hot state exists");
    assert_eq!(after.queue_ticket, before.queue_ticket);
    assert_eq!(after.wakeup_pointer, before.wakeup_pointer);
  });
}

#[test]
fn missing_tracked_snapshot_resolves_to_funding_unavailable() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == actor_id
      )
    }));
    assert!(Actors::active_actor_view(actor_id).is_some());
  });
}

#[test]
fn zero_snapshot_resolves_to_funding_unavailable() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    crate::ActorFunding::<Test>::mutate(actor_id, |maybe| {
      let funding = maybe.as_mut().expect("actor funding exists");
      funding
        .funding_accumulated
        .try_insert(TestAsset::Native, 0)
        .expect("snapshot must fit");
    });
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn burn_last_funding_overspend_resolves_to_funding_unavailable() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Burn {
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 100);
    crate::ActorFunding::<Test>::mutate(actor_id, |maybe| {
      let funding = maybe.as_mut().expect("actor funding exists");
      funding
        .funding_accumulated
        .try_insert(TestAsset::Native, 200)
        .expect("snapshot must fit");
    });
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepSkipped {
        actor_id: id,
        step_index: 0,
        reason: StepSkippedReason::FundingUnavailable,
        ..
      } if *id == actor_id
    )));
    assert_eq!(native_balance(&actor), 100);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors remains active")
        .unsuccessful_attempt_streak,
      0
    );
  });
}

#[test]
fn overspend_resolves_to_funding_unavailable_for_system_without_pause() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1_000_000),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == actor_id
      )
    }));
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(instance.lifecycle, ActiveLifecycle::Active);
    let actor = sovereign_account(actor_id);
    assert_eq!(
      native_balance(&actor),
      100,
      "balance stays unchanged on FundingUnavailable"
    );
  });
}

#[test]
fn overspend_resolves_to_funding_unavailable_for_user_without_closing() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1_000_000),
    }));
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == actor_id
      )
    }));
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert_eq!(
      native_balance(&actor),
      1_000u128.saturating_sub(Actors::compute_eval_fee(0))
    );
  });
}

#[test]
fn funding_unavailable_releases_exec_fee_reservation_for_later_step_spend() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1_000_000),
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(650),
      }),
    ])
    .expect("execution plan must fit");
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    let actor = sovereign_account(actor_id);
    let bob_before = native_balance(&BOB);
    fund_native(actor_id, 1_000);
    clear_fee_collections();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          step_index: 0,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == actor_id
      )
    }));
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(650));
    let transfer = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(650),
    };
    let expected_attempt_fee = Actors::compute_eval_fee(0).saturating_add(
      TestWeightToFee::weight_to_fee(&Actors::weight_upper_bound(&transfer)),
    );
    assert_eq!(
      fee_collections(),
      vec![Actors::compute_eval_fee(0), expected_attempt_fee]
    );
    assert_eq!(
      native_balance(&actor),
      50,
      "later step can spend the execution-fee reservation released by the funding skip"
    );
  });
}

#[test]
fn failed_executable_step_charges_eval_and_exec_fee_without_refund() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::SwapIn {
      asset_in: TestAsset::Native,
      asset_out: TestAsset::Local(99),
      amount_in: AmountResolution::Fixed(10),
      slippage_tolerance: Perbill::zero(),
    }));
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let expected_fee = Actors::compute_eval_fee(0).saturating_add(TestWeightToFee::weight_to_fee(
      &Actors::weight_upper_bound(&Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(99),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::zero(),
      }),
    ));
    assert_eq!(
      native_balance(&actor),
      1_000u128.saturating_sub(expected_fee),
      "failed executable path should charge exactly eval+exec fee with no refund"
    );
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepFailed {
          actor_id: id,
          step_index: 0,
          ..
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn condition_fee_depends_only_on_total_atomic_count() {
  let forward = all_conditions(vec![
    Predicate::BlockNumberAbove { threshold: 1 },
    Predicate::BlockNumberBelow { threshold: 10 },
  ]);
  let reverse = any_conditions(vec![
    Predicate::BlockNumberBelow { threshold: 10 },
    Predicate::BlockNumberAbove { threshold: 1 },
  ]);
  let forward = forward.expect("bounded precondition");
  let reverse = reverse.expect("bounded precondition");
  assert_eq!(forward.predicate_count(), reverse.predicate_count());
  assert_eq!(
    Actors::compute_eval_fee(forward.predicate_count()),
    Actors::compute_eval_fee(reverse.predicate_count())
  );
}

#[test]
fn condition_balance_above_skips_when_below() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 1_000,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let actor = sovereign_account(actor_id);
    assert_eq!(
      native_balance(&actor),
      100,
      "transfer skipped — balance below threshold"
    );
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepSkipped {
        actor_id: id,
        step_index: 0,
        reason: StepSkippedReason::PreconditionFalse,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn condition_balance_above_executes_when_above() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 50,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&BOB),
      bob_before + 10,
      "transfer executed — balance above threshold"
    );
  });
}

#[test]
fn preserve_spend_keeps_native_minimum_across_fixed_percentage_split_and_all_balance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let split_legs: SplitTransferLegsOf<Test> = BoundedVec::try_from(vec![
      SplitLeg {
        to: BOB,
        share: Perbill::from_percent(50),
      },
      SplitLeg {
        to: CHARLIE,
        share: Perbill::from_percent(50),
      },
    ])
    .expect("two split legs fit");
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(100),
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfCurrent(Perbill::one()),
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageAtOpening(Perbill::one()),
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      }),
      make_step(Task::SplitTransfer {
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(100),
        legs: split_legs,
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::AllAvailable,
      }),
    ])
    .expect("system execution plan fits");
    let actor_id = create_system_with(ALICE, percentage_trigger_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    crate::ActorFunding::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("System Actors funding exists")
        .funding_accumulated
        .try_insert(TestAsset::Native, 100)
        .expect("tracked snapshot fits");
    });
    let actor = sovereign_account(actor_id);
    let bob_before = native_balance(&BOB);
    signal_percentage_trigger(actor_id, TestAsset::Native);
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 1);
    assert_eq!(native_balance(&BOB), bob_before + 99);
    let funding_skips = frame_system::Pallet::<Test>::events()
      .iter()
      .filter(|record| {
        matches!(
          &record.event,
          RuntimeEvent::Actors(Event::StepSkipped {
            actor_id: id,
            reason: StepSkippedReason::FundingUnavailable,
            ..
          }) if *id == actor_id
        )
      })
      .count();
    assert_eq!(funding_skips, 4);
    let resolution_skips = frame_system::Pallet::<Test>::events()
      .iter()
      .filter(|record| {
        matches!(
          &record.event,
          RuntimeEvent::Actors(Event::StepSkipped {
            actor_id: id,
            reason: StepSkippedReason::ResolutionSkipped,
            ..
          }) if *id == actor_id
        )
      })
      .count();
    assert_eq!(resolution_skips, 1);
  });
}

#[test]
fn percentage_of_current_uses_native_preservable_balance_as_its_base() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfCurrent(Perbill::one()),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let actor = sovereign_account(actor_id);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 1);
    assert_eq!(native_balance(&BOB), bob_before + 99);
    assert!(frame_system::Pallet::<Test>::events().iter().any(|record| {
      matches!(
        &record.event,
        RuntimeEvent::Actors(Event::TransferExecuted {
          actor_id: id,
          amount: 99,
          ..
        }) if *id == actor_id
      )
    }));
  });
}

#[test]
fn user_all_available_preserves_current_floor_but_no_future_cycle_fee() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let task = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::AllAvailable,
    };
    let contract_steps = contract_steps_with_step(make_step(task));
    let fee = Actors::attempt_fee_envelope(ActorType::User, &contract_steps, 0)
      .expect("User fee envelope")
      .total;
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    let actor = sovereign_account(actor_id);
    let floor = TestMinUserBalance::get();
    let initial = floor.saturating_add(fee).saturating_add(50);
    fund_native(actor_id, initial);
    let bob_before = native_balance(&BOB);
    clear_fee_collections();

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(native_balance(&actor), floor);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(fee_collections(), vec![fee]);

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::FeeBudgetExhausted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn user_fee_admission_boundary_is_exact_floor_plus_envelope() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    }));
    let fee = Actors::attempt_fee_envelope(ActorType::User, &contract_steps, 0)
      .expect("User fee envelope")
      .total;
    let floor = TestMinUserBalance::get();

    // floor + envelope - 1: the complete envelope cannot fit above the floor, so
    // the User attempt is NOT admitted and the actor closes with FeeBudgetExhausted
    // even though the raw balance covers the envelope (spec 5.2.1).
    let prefunded = user_prefunding_requirement(&contract_steps);
    let short = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps.clone(),
    );
    deplete_user_sovereign(short, prefunded);
    fund_native(short, floor.saturating_add(fee).saturating_sub(1));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), short));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(short).is_none());
    assert!(has_actor_event(|e| matches!(
      e,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::FeeBudgetExhausted,
        ..
      } if *id == short
    )));

    // floor + envelope: the envelope fits exactly above the floor and the run admits.
    let prefunded = user_prefunding_requirement(&contract_steps);
    let exact = create_user_with(
      BOB,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(exact, prefunded);
    fund_native(exact, floor.saturating_add(fee));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(BOB), exact));
    run_idle(Weight::MAX);
    // The attempt is admitted (the envelope fits exactly above the floor); the
    // transfer itself is floor-limited, so assert admission via the nonce advance.
    assert_eq!(
      Actors::active_actor_view(exact)
        .expect("active")
        .cycle_nonce,
      1
    );
  });
}

#[test]
fn user_swap_out_native_input_cap_preserves_fee_native_floor_after_failed_attempt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_out = TestAsset::Local(88);
    let task = Task::SwapOut {
      asset_out,
      amount_out: AmountResolution::Fixed(98),
      asset_in: TestAsset::Native,
      input_limit: InputLimit::LiveQuote,
      slippage_tolerance: Perbill::zero(),
    };
    set_pool_reserves(TestAsset::Native, asset_out, 10_000, 10_000);
    set_asset_balance(&u64::MAX, asset_out, 10_000);
    let contract_steps = contract_steps_with_step(make_step(task));
    let fee_envelope =
      Actors::attempt_fee_envelope(ActorType::User, &contract_steps, 0).expect("User fee envelope");
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    let actor = sovereign_account(actor_id);
    let floor = TestMinUserBalance::get();
    let initial = floor.saturating_add(fee_envelope.total).saturating_add(50);
    fund_native(actor_id, initial);
    clear_fee_collections();

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(
      native_balance(&actor),
      initial.saturating_sub(fee_envelope.total)
    );
    assert!(native_balance(&actor) >= floor);
    assert_eq!(fee_collections(), vec![fee_envelope.total]);
    assert!(has_actor_event(|event| {
      matches!(event, Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id)
    }));
    assert!(!has_actor_event(|event| {
      matches!(event, Event::SwapExecuted { actor_id: id, .. } if *id == actor_id)
    }));
  });
}

#[test]
fn percentage_of_current_uses_sufficient_asset_preservable_balance_as_its_base() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(7);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset,
      amount: AmountResolution::PercentageOfCurrent(Perbill::one()),
    }));
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
fn burn_all_balance_preserves_the_asset_minimum() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Burn {
      asset: TestAsset::Native,
      amount: AmountResolution::AllAvailable,
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 500);
    let actor = sovereign_account(actor_id);
    assert_eq!(native_balance(&actor), 500);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&actor),
      1,
      "Burn(AllAvailable) must preserve the asset minimum"
    );
    assert!(has_actor_event(|e| matches!(
      e,
      Event::BurnExecuted { actor_id: id, asset: TestAsset::Native, amount: 499, .. } if *id == actor_id
    )));
  });
}

#[test]
fn stake_preserve_spend_keeps_asset_minimum_balance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(70);
    let contract_steps = contract_steps_with_step(make_step(Task::Stake {
      asset,
      amount: AmountResolution::Fixed(99),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 1);
    assert_eq!(staked_balance(actor, asset), 99);
  });
}

#[test]
fn add_liquidity_uses_funding_unavailable_precedence_across_amount_fields() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset_a = TestAsset::Local(71);
    let asset_b = TestAsset::Local(72);
    let contract_steps = contract_steps_with_step(make_step(Task::AddLiquidity {
      asset_a,
      asset_b,
      amount_a: AmountResolution::PercentageOfCurrent(Perbill::from_percent(1)),
      amount_b: AmountResolution::Fixed(50),
      min_lp_out: 1,
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset_a, 2);
    set_asset_balance(&actor, asset_b, 50);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::StepSkipped {
          actor_id: id,
          reason: StepSkippedReason::FundingUnavailable,
          ..
        } if *id == actor_id
      )
    }));
    assert_eq!(asset_balance(&actor, asset_a), 2);
    assert_eq!(asset_balance(&actor, asset_b), 50);
  });
}

#[test]
fn unstake_all_balance_withdraws_all_staking_shares() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(8);
    let contract_steps = contract_steps_with_step(make_step(Task::Unstake {
      asset,
      shares: AmountResolution::AllAvailable,
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    set_asset_balance(&actor, asset, 80);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 0);
    assert_eq!(unstaked_shares(actor, asset), 80);
  });
}

#[test]
fn unstake_last_funding_tracks_transferable_share_asset() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(8);
    let contract_steps = contract_steps_with_step(make_step(Task::Unstake {
      asset,
      shares: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    set_asset_balance(&ALICE, asset, 100);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      asset,
      100,
    ));
    let actor = sovereign_account(actor_id);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&actor, asset), 50);
    assert_eq!(unstaked_shares(actor, asset), 50);
  });
}

#[test]
fn unstake_last_funding_rejects_position_without_transferable_share_asset() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Unstake {
      asset: TestAsset::Local(u32::MAX),
      shares: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    }));
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, contract_steps),
      ),
      Error::<Test>::InvalidAmountResolution
    );
  });
}

#[test]
fn unstake_last_funding_fails_closed_if_share_mapping_disappears_mid_lifetime() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let asset = TestAsset::Local(8);
    let contract_steps = contract_steps_with_step(make_step(Task::Unstake {
      asset,
      shares: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    set_asset_balance(&ALICE, asset, 100);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      asset,
      100,
    ));
    let actor = sovereign_account(actor_id);
    set_staking_share_asset_available(false);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);

    assert_eq!(asset_balance(&actor, asset), 100);
    assert_eq!(unstaked_shares(actor, asset), 0);
    assert!(Actors::continuation_state(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepFailed { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
  });
}

#[test]
fn donate_liquidity_user_fee_native_b_side_preserves_protected_floor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // A User actor donates with the fee-native asset as the paired asset-b side. The pallet
    // resolves the asset-b debit cap as the preservable capacity, so the paired donation must
    // never consume the fee-native protected floor (MinUserBalance = 50).
    let asset_a = TestAsset::Local(90);
    let asset_b = TestAsset::Native;
    let contract_steps = contract_steps_with_step(make_step(Task::DonateLiquidity {
      asset_a,
      asset_b,
      max_amount_a: AmountResolution::PercentageOfCurrent(Perbill::from_percent(100)),
      max_ratio_error: Perbill::from_percent(1),
    }));
    let fee = Actors::attempt_fee_envelope(ActorType::User, &contract_steps, 0)
      .expect("User fee envelope")
      .total;
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    let actor = sovereign_account(actor_id);
    let floor = TestMinUserBalance::get();
    set_asset_balance(&actor, asset_a, 1_000);
    // Fund the floor plus the reserved User fee plus a spendable donation budget.
    fund_native(actor_id, floor.saturating_add(fee).saturating_add(60));
    clear_fee_collections();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let (used_a, used_b) = donated_liquidity(actor, asset_a, asset_b);
    assert!(
      used_b <= 60,
      "asset-b fee-native debit must respect its preservable capacity"
    );
    assert_eq!(
      native_balance(&actor),
      floor
        .saturating_add(fee)
        .saturating_add(60)
        .saturating_sub(used_b)
        .saturating_sub(fee),
      "the fee-native protected floor is preserved by the asset-b cap"
    );
    assert!(native_balance(&actor) >= floor);
    assert_eq!(
      used_a, used_b,
      "balanced mock donation caps at the smaller side"
    );
  });
}

#[test]
fn condition_balance_below_skips_when_above() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceBelow {
        asset: TestAsset::Native,
        threshold: 50,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepSkipped { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
  });
}

#[test]
fn condition_balance_below_executes_when_below() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceBelow {
        asset: TestAsset::Native,
        threshold: 200,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before + 10);
  });
}

#[test]
fn condition_balance_equals_matches_exact() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceEquals {
        asset: TestAsset::Native,
        threshold: 100,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      native_balance(&BOB),
      bob_before + 10,
      "executes when balance == threshold"
    );
  });
}

#[test]
fn condition_balance_equals_skips_when_not_equal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceEquals {
        asset: TestAsset::Native,
        threshold: 999,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepSkipped { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
  });
}

#[test]
fn condition_balance_not_equals_executes_when_different() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceNotEquals {
        asset: TestAsset::Native,
        threshold: 999,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before + 10);
  });
}

#[test]
fn condition_balance_not_equals_skips_when_equal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceNotEquals {
        asset: TestAsset::Native,
        threshold: 100,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepSkipped { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
  });
}

#[test]
fn insufficient_fee_closes_cycle_immediately() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    }));
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    // Fund above MinUserBalance(50) but below fee threshold (eval=1 + exec=100 = 101)
    fund_native(actor_id, 60);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|e| matches!(
      e,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::FeeBudgetExhausted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn condition_sees_spendable_not_raw_balance_for_user_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let threshold = 350;
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    fund_native(actor_id, 400);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    // The raw balance exceeds the threshold, but the fee reservation leaves less spendable.
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepSkipped { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
  });
}

#[test]
fn condition_sees_full_balance_for_system_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // System Actors: reserved=0, so spendable == raw
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 150,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(50),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 200);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    // Must execute: spendable(200) > threshold(150)
    assert_eq!(native_balance(&BOB), bob_before + 50);
  });
}

#[test]
fn user_condition_combines_adapter_lock_with_reserved_fee_budget() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 60,
      }]),
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      },
      on_error: StepErrorPolicy::AbortCycle,
    };
    let contract_steps = contract_steps_with_step(step);
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    fund_native(actor_id, 300);
    let sovereign = Actors::active_actor_view(actor_id)
      .expect("user actor")
      .sovereign_account;
    // A transfer lock reduces the native reducible balance below what the full fee
    // envelope needs above MinUserBalance, so the User attempt is not admitted and
    // the actor closes with FeeBudgetExhausted (spec 5.2.1) rather than running.
    set_native_transfer_lock(&sovereign, 150);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::FeeBudgetExhausted,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn user_portfolio_rebalancer_both_directions() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let foreign = TestAsset::Local(1);
    let schedule = timer_schedule(5);
    // Step 0: If native > 5000 (spendable), transfer 20% native to BOB
    // Step 1: If spendable native < 500 AND foreign > 500, transfer 50% foreign to CHARLIE
    // Full two-step attempt fee envelope ≈ 2*(1+100) = 202
    // BalanceBelow checks spendable = raw - fee_reserve
    let contract_steps = BoundedVec::try_from(vec![
      StepOf::<Test> {
        precondition: all_conditions(vec![Predicate::BalanceAbove {
          asset: TestAsset::Native,
          threshold: 5000,
        }]),
        task: Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(20)),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      StepOf::<Test> {
        precondition: all_conditions(vec![
          Predicate::BalanceBelow {
            asset: TestAsset::Native,
            threshold: 500,
          },
          Predicate::BalanceAbove {
            asset: foreign,
            threshold: 500,
          },
        ]),
        task: Task::Transfer {
          to: CHARLIE,
          asset: foreign,
          amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(50)),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
    ])
    .unwrap();
    let actor_id = create_user_with(ALICE, Mutability::Mutable, schedule, None, contract_steps);
    let actor = sovereign_account(actor_id);
    // First evaluation: native high → step 0 fires, step 1 skipped
    fund_native(actor_id, 10000);
    set_asset_balance(&actor, foreign, 2000);
    frame_system::Pallet::<Test>::set_block_number(6);
    Actors::on_initialize(6);
    Actors::on_idle(6, Weight::MAX);
    assert!(
      has_actor_event(|e| matches!(
        e,
        Event::TransferExecuted { actor_id: id, asset: TestAsset::Native, to, .. }
        if *id == actor_id && *to == BOB
      )),
      "Step 0 should execute when spendable native > 5000"
    );
    // Second evaluation: slash native so spendable < 500, keep raw > fee_reserve (202)
    // Set raw native to 600: spendable = 600 - 202 = 398 < 500 ✓
    let actor_native = native_balance(&actor);
    let _ = <Balances as Currency<AccountId>>::slash(&actor, actor_native.saturating_sub(600));
    let charlie_before = asset_balance(&CHARLIE, foreign);
    frame_system::Pallet::<Test>::set_block_number(11);
    Actors::on_initialize(11);
    Actors::on_idle(11, Weight::MAX);
    assert!(
      asset_balance(&CHARLIE, foreign) > charlie_before,
      "Step 1 should execute when spendable native < 500 AND foreign > 500"
    );
  });
}

#[test]
fn funding_ingress_mutates_only_actor_funding_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    let instance_before = Actors::active_actor_view(actor_id).expect("Actors exists");
    let hot_before = Actors::actor_hot(actor_id).expect("actor hot state exists");
    let contract_before = Actors::actor_contract(actor_id).expect("actor contract exists");
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      100,
    ));
    assert_eq!(Actors::active_actor_view(actor_id), Some(instance_before));
    assert_eq!(Actors::actor_hot(actor_id), Some(hot_before));
    assert_eq!(Actors::actor_contract(actor_id), Some(contract_before));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&100)
    );
  });
}

#[test]
fn funding_accumulator_isolated_per_asset() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let foreign = TestAsset::Local(1);
    let contract_steps = contract_steps_with_step(StepOf::<Test> {
      precondition: None,
      task: Task::Transfer {
        to: BOB,
        asset: foreign,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
      },
      on_error: StepErrorPolicy::AbortCycle,
    });
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    // Mint on ALICE before the ordinary transfer to the sovereign account
    set_asset_balance(&ALICE, foreign, 1000);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      foreign,
      1000
    ));
    // Verify accumulation.
    let funding = actor_funding(actor_id);
    assert_eq!(funding.funding_accumulated.get(&foreign), Some(&1000));
    // Native should not be tracked (not referenced by PercentageOfLastFunding)
    assert!(!funding.funding_tracked_assets.contains(&TestAsset::Native));
  });
}

#[test]
fn percentage_of_last_funding_multi_asset() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let foreign_a = TestAsset::Local(1);
    let foreign_b = TestAsset::Local(2);
    let contract_steps = BoundedVec::try_from(vec![
      StepOf::<Test> {
        precondition: None,
        task: Task::Transfer {
          to: BOB,
          asset: foreign_a,
          amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(10)),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
      StepOf::<Test> {
        precondition: None,
        task: Task::Transfer {
          to: CHARLIE,
          asset: foreign_b,
          amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(20)),
        },
        on_error: StepErrorPolicy::ContinueNextStep,
      },
    ])
    .unwrap();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    // Mint both assets on ALICE before ordinary transfers to the sovereign account
    set_asset_balance(&ALICE, foreign_a, 1000);
    set_asset_balance(&ALICE, foreign_b, 500);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      foreign_a,
      1000
    ));
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      foreign_b,
      500
    ));
    // Execute
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    // Verify transfers: 10% of 1000 = 100 to BOB, 20% of 500 = 100 to CHARLIE
    assert_eq!(asset_balance(&BOB, foreign_a), 100);
    assert_eq!(asset_balance(&CHARLIE, foreign_b), 100);
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_reconciles_system_sovereign_and_reverse_index_ownership() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let _ = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });

  for corruption in 0u8..4 {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      let identity = ActorIdentities::<Test>::get(actor_id).expect("System identity fixture");
      let ActorClass::System { sovereign_id } = identity.actor_class else {
        panic!("fixture must create a System actor");
      };
      match corruption {
        0 => crate::SystemSovereignCount::<Test>::mutate(|count| *count = count.saturating_add(1)),
        1 => {
          crate::SystemSovereigns::<Test>::insert(
            sovereign_id.saturating_add(1_000),
            SystemSovereignState::Occupied(actor_id),
          );
          crate::SystemSovereignCount::<Test>::mutate(|count| *count = count.saturating_add(1));
        }
        2 => crate::SystemSovereigns::<Test>::insert(sovereign_id, SystemSovereignState::Vacant),
        3 => {
          crate::SovereignIndex::<Test>::insert(999_999, actor_id);
        }
        _ => unreachable!(),
      }
      assert!(
        crate::Pallet::<Test>::do_try_state().is_err(),
        "System sovereign corruption case {corruption} must fail",
      );
    });
  }
}
