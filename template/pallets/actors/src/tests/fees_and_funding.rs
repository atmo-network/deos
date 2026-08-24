use super::*;

#[test]
fn actor_state_hold_prices_exact_contract_geometry_and_releases_on_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let dormant_owner_before = native_balance(&ALICE);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      None,
    ));
    let dormant = Actors::actor_state_hold(0).expect("Dormant User hold exists");
    assert!(dormant.breakdown.identity > 0);
    assert_eq!(dormant.breakdown.contract_head, 0);
    assert_eq!(dormant.breakdown.contract_body, 0);
    assert_eq!(dormant.breakdown.detector, 0);
    assert_eq!(dormant.breakdown.funding, 0);
    assert_eq!(dormant.breakdown.run, 0);
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), 0));
    assert!(Actors::actor_state_hold(0).is_none());
    assert_eq!(
      native_balance(&ALICE),
      dormant_owner_before.saturating_sub(TestActorCreationFee::get())
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    let one_step = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let five_steps = BoundedVec::try_from(
      (0..5)
        .map(|_| {
          make_step(Task::Transfer {
            to: BOB,
            asset: TestAsset::Native,
            amount: AmountResolution::Fixed(1),
          })
        })
        .collect::<Vec<_>>(),
    )
    .expect("five Steps fit");
    let five_step = create_user_with(
      CHARLIE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      five_steps,
    );
    let compact = Actors::actor_state_hold(one_step).expect("one-Step hold exists");
    let chunked = Actors::actor_state_hold(five_step).expect("five-Step hold exists");
    assert_eq!(compact.breakdown.contract_body, 0);
    assert!(compact.breakdown.run > 0);
    assert_eq!(chunked.breakdown.run, compact.breakdown.run);
    assert!(chunked.breakdown.contract_body > 0);
    assert!(chunked.breakdown.contract_head > 0);
    assert!(chunked.breakdown.funding > 0);
    let temporal = create_user_with(
      ALICE,
      Mutability::Mutable,
      at_time_schedule(10),
      None,
      crate::ContractSteps::<Test>::default(),
    );
    assert!(
      Actors::actor_state_hold(temporal)
        .expect("temporal hold exists")
        .breakdown
        .detector
        > 0
    );

    let system = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert!(Actors::actor_state_hold(system).is_none());
  });
}

#[test]
fn actor_state_hold_failure_and_lifecycle_deltas_are_atomic() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let underfunded_owner = 77u64;
    let initial = TestActorCreationFee::get().saturating_add(1);
    let _ = <Balances as Currency<AccountId>>::deposit_creating(&underfunded_owner, initial);
    let sink_before = native_balance(&TestFeeSink::get());
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(underfunded_owner),
        Mutability::Mutable,
        None,
      ),
      Error::<Test>::StateHoldUnavailable
    );
    assert_eq!(native_balance(&underfunded_owner), initial);
    assert_eq!(native_balance(&TestFeeSink::get()), sink_before);
    assert_eq!(Actors::next_actor_id(), 0);
    assert_eq!(Actors::actor_identity_count(), 0);

    let owner = 78u64;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(&owner, 1_000_000);
    let actor_id = create_user_with(
      owner,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let old_contract = Actors::actor_contract(actor_id).expect("one-Step Contract exists");
    let old_hold = Actors::actor_state_hold(actor_id).expect("one-Step hold exists");
    let free = native_balance(&owner);
    assert_ok!(<Balances as Currency<AccountId>>::transfer(
      &owner,
      &BOB,
      free.saturating_sub(1),
      polkadot_sdk::frame_support::traits::ExistenceRequirement::AllowDeath,
    ));
    let five_steps = BoundedVec::try_from(
      (0..5)
        .map(|_| {
          make_step(Task::Transfer {
            to: BOB,
            asset: TestAsset::Native,
            amount: AmountResolution::Fixed(1),
          })
        })
        .collect::<Vec<_>>(),
    )
    .expect("five Steps fit");
    let replacement = user_active_contract(manual_schedule(), None, five_steps)
      .expect("replacement Contract exists");
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_noop!(
      Actors::update_contract(RuntimeOrigin::signed(owner), actor_id, replacement.clone()),
      Error::<Test>::StateHoldUnavailable
    );
    assert_eq!(Actors::actor_contract(actor_id), Some(old_contract));
    assert_eq!(Actors::actor_state_hold(actor_id), Some(old_hold));

    let _ = <Balances as Currency<AccountId>>::deposit_creating(&owner, 1_000_000);
    assert_ok!(Actors::update_contract(
      RuntimeOrigin::signed(owner),
      actor_id,
      replacement,
    ));
    let active_hold = Actors::actor_state_hold(actor_id).expect("expanded hold exists");
    assert!(active_hold.breakdown.contract_body > 0);

    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(owner),
      actor_id,
    ));
    let dormant_hold = Actors::actor_state_hold(actor_id).expect("Dormant hold exists");
    assert!(dormant_hold.breakdown.identity > 0);
    assert_eq!(dormant_hold.breakdown.contract_head, 0);
    assert_eq!(dormant_hold.breakdown.contract_body, 0);
    assert_eq!(dormant_hold.breakdown.detector, 0);
    assert_eq!(dormant_hold.breakdown.funding, 0);
    assert_eq!(dormant_hold.breakdown.run, 0);
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(owner), actor_id));
    assert!(Actors::actor_state_hold(actor_id).is_none());
  });
}

#[test]
fn funding_ingress_preflights_positive_state_hold_delta_as_temporary() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let owner = 79u64;
    let _ = <Balances as Currency<AccountId>>::deposit_creating(&owner, 1_000_000);
    let tracked_steps = BoundedVec::try_from(vec![make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
    })])
    .expect("tracked Step fits");
    let actor_id = create_user_with(
      owner,
      Mutability::Mutable,
      manual_schedule(),
      None,
      tracked_steps,
    );
    let sovereign = sovereign_account(actor_id);
    let free = native_balance(&owner);
    assert_ok!(<Balances as Currency<AccountId>>::transfer(
      &owner,
      &BOB,
      free.saturating_sub(1),
      polkadot_sdk::frame_support::traits::ExistenceRequirement::AllowDeath,
    ));
    let failure = Actors::preflight_ingress(&crate::AddressEvent {
      destination: sovereign,
      source: Some(owner),
      asset: TestAsset::Native,
      amount: 1,
      provenance: Some(crate::FundingProvenance::Signed),
    })
    .expect_err("new funding entry needs additional owner hold capacity");
    assert_eq!(failure.retry, RetryClass::Temporary);
    assert_eq!(failure.error, Error::<Test>::StateHoldUnavailable.into());
    assert!(
      Actors::actor_funding(actor_id)
        .expect("funding state exists")
        .funding_accumulated
        .is_empty()
    );
  });
}

#[test]
fn actor_run_state_hold_is_reserved_for_the_active_installed_lifetime() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut retry_step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(Balance::MAX),
    });
    retry_step.on_error = RETRY_LATER;
    let steps = BoundedVec::try_from(vec![retry_step]).expect("one retry Step fits");
    let actor_id = create_user_with(ALICE, Mutability::Mutable, manual_schedule(), None, steps);
    let installed_run_hold = Actors::actor_state_hold(actor_id)
      .expect("idle hold exists")
      .breakdown
      .run;
    assert!(installed_run_hold > 0);
    fund_native(actor_id, 1_000_000_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(3);
    run_idle(Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_some());
    assert_eq!(
      Actors::actor_state_hold(actor_id)
        .expect("Running hold exists")
        .breakdown
        .run,
      installed_run_hold
    );
    frame_system::Pallet::<Test>::set_block_number(4);
    assert_ok!(Actors::cancel_run(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(
      Actors::actor_state_hold(actor_id)
        .expect("active installed hold remains")
        .breakdown
        .run,
      installed_run_hold
    );
  });
}

#[test]
fn active_lifetime_run_hold_preserves_autonomous_opening_after_owner_spends_free_balance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut retry_step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(Balance::MAX),
    });
    retry_step.on_error = RETRY_LATER;
    let steps = BoundedVec::try_from(vec![retry_step]).expect("one retry Step fits");
    let actor_id = create_user_with(ALICE, Mutability::Mutable, manual_schedule(), None, steps);
    let reserved = Actors::actor_state_hold(actor_id)
      .expect("active installed hold exists")
      .breakdown
      .run;
    assert!(reserved > 0);
    fund_native(
      actor_id,
      user_prefunding_requirement(&inert_contract_steps()),
    );
    let owner_free = native_balance(&ALICE);
    assert_ok!(<Balances as Currency<AccountId>>::transfer(
      &ALICE,
      &BOB,
      owner_free.saturating_sub(1),
      polkadot_sdk::frame_support::traits::ExistenceRequirement::AllowDeath,
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(3);
    run_idle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_some());
    assert!(Actors::actor_run_state(actor_id).is_some());
    assert_eq!(
      Actors::actor_state_hold(actor_id)
        .expect("active installed hold remains")
        .breakdown
        .run,
      reserved
    );
  });
}

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
    let state_hold = actor_state_hold_total(actor_id);
    assert_eq!(
      native_balance(&ALICE),
      owner_before.saturating_sub(fee).saturating_sub(state_hold)
    );
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

    assert_eq!(
      native_balance(&ALICE),
      alice_before
        .saturating_sub(fee)
        .saturating_sub(actor_state_hold_total(0))
    );
    assert_eq!(
      native_balance(&CHARLIE),
      charlie_before
        .saturating_sub(fee)
        .saturating_sub(actor_state_hold_total(1))
    );
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
fn user_active_creation_charges_creation_fee_without_sovereign_service_prefunding() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let owner_before = native_balance(&ALICE);
    let sink_before = native_balance(&TestFeeSink::get());
    let creation_fee = TestActorCreationFee::get();
    let sovereign = Actors::sovereign_account_id(&ALICE, 0);
    assert_eq!(native_balance(&sovereign), 0);

    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
    ));

    let state_hold = actor_state_hold_total(0);
    assert_eq!(
      native_balance(&ALICE),
      owner_before - creation_fee - state_hold
    );
    assert_eq!(
      native_balance(&TestFeeSink::get()),
      sink_before + creation_fee
    );
    assert_eq!(native_balance(&sovereign), 0);
    assert_eq!(Actors::actor_identity_count(), 1);
    assert_eq!(Actors::active_actor_count(), 1);
  });
}

#[test]
fn user_active_creation_requires_no_sovereign_activation_prefunding() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
    ])
    .expect("two Steps fit");
    let sovereign = Actors::sovereign_account_id(&ALICE, 0);
    assert_eq!(native_balance(&sovereign), 0);

    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, plan),
    ));

    assert_eq!(native_balance(&sovereign), 0);
    let head = crate::ActorContractHeads::<Test>::get(0).expect("C6 head exists");
    assert!(
      head
        .header
        .pipeline_machine_envelope
        .pipeline_machine_fee_upper
        > 0
    );
    assert_eq!(Actors::active_actor_count(), 1);
  });
}

#[test]
fn zero_step_user_opening_charges_pipeline_but_no_action_fee() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      BoundedVec::default(),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    clear_fee_collections();
    let fee_sink_before = native_balance(&TestFeeSink::get());

    run_idle(Weight::MAX);

    let pipeline_fee = pipeline_opening_fee(&BoundedVec::default());
    assert_eq!(fee_collections(), vec![pipeline_fee]);
    assert_eq!(
      native_balance(&TestFeeSink::get()),
      fee_sink_before.saturating_add(pipeline_fee)
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::PipelineFeeCharged {
        actor_id: id,
        fee,
      } if *id == actor_id && *fee == pipeline_fee
    )));
    assert_eq!(
      Actors::actor_identities(actor_id)
        .expect("persistent zero-Step User remains")
        .cycle_nonce,
      1
    );
    assert!(ActorRunStateStore::<Test>::get(actor_id).is_none());
  });
}

#[test]
fn manual_trigger_charges_occurrence_before_pipeline_opening() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let sovereign = sovereign_account(actor_id);
    let sovereign_before = native_balance(&sovereign);
    let sink_before = native_balance(&TestFeeSink::get());
    clear_fee_collections();

    let post = Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id)
      .expect("funded Manual occurrence commits");

    let fee = manual_trigger_fee();
    assert_eq!(
      post.pays_fee,
      polkadot_sdk::frame_support::dispatch::Pays::No
    );
    assert_eq!(fee_collections(), vec![fee]);
    assert_eq!(native_balance(&sovereign), sovereign_before - fee);
    assert_eq!(native_balance(&TestFeeSink::get()), sink_before + fee);
    let instance = Actors::active_actor_view(actor_id).expect("Actor remains");
    assert!(instance.pending_signal);
    assert_eq!(instance.cycle_nonce, 0, "Trigger does not open a Pipeline");
    assert!(ActorRunStateStore::<Test>::get(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed {
        actor_id: id,
        trigger_family: TriggerFamily::Manual,
        fee: charged,
      } if *id == actor_id && *charged == fee
    )));

    let pipeline_fee = pipeline_opening_fee(&instance.steps);
    frame_system::Pallet::<Test>::set_block_number(3);
    run_idle(Weight::MAX);
    assert_eq!(fee_collections().get(1), Some(&pipeline_fee));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::PipelineFeeCharged {
        actor_id: id,
        fee,
      } if *id == actor_id && *fee == pipeline_fee
    )));
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("persistent Actor remains")
        .cycle_nonce,
      1
    );
  });
}

#[test]
fn repeated_pending_manual_occurrence_is_latched_without_trigger_fee() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    fund_native(actor_id, manual_trigger_fee());
    clear_fee_collections();

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ticket = Actors::actor_hot(actor_id)
      .expect("Actor hot state")
      .queue_ticket;
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    assert_eq!(fee_collections(), vec![manual_trigger_fee()]);
    let hot = Actors::actor_hot(actor_id).expect("Actor hot state");
    assert!(hot.pending_signal);
    assert_eq!(
      hot.queue_ticket, ticket,
      "coalescing creates no second ticket"
    );
    assert_eq!(
      System::events()
        .iter()
        .filter(|record| matches!(
          &record.event,
          RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
            actor_id: id,
            trigger_family: TriggerFamily::Manual,
            ..
          }) if *id == actor_id
        ))
        .count(),
      1
    );

    run_idle(Weight::MAX);
    assert_eq!(
      System::events()
        .iter()
        .filter(|record| matches!(
          &record.event,
          RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id: id, .. })
            if *id == actor_id
        ))
        .count(),
      1
    );
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("persistent Actor remains")
        .cycle_nonce,
      1
    );
  });
}

#[test]
fn busy_manual_occurrence_charges_and_latches_only_the_future_pipeline() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
    ])
    .expect("two Steps fit");
    let actor_id = create_user_with(ALICE, Mutability::Mutable, manual_schedule(), None, plan);
    fund_native(actor_id, 1_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    Actors::on_idle(1, Weight::MAX);
    let run_before = ActorRunStateStore::<Test>::get(actor_id).expect("Pipeline is Running");
    assert_eq!(run_before.cursor, 1);
    clear_fee_collections();
    frame_system::Pallet::<Test>::reset_events();

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    assert_eq!(fee_collections(), vec![manual_trigger_fee()]);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::PipelineFeeCharged { actor_id: id, .. } if *id == actor_id
    )));
    let hot = Actors::actor_hot(actor_id).expect("Actor hot state");
    assert_eq!(hot.cycle_state, CycleState::Running);
    assert!(hot.pending_signal);
    let run_after = ActorRunStateStore::<Test>::get(actor_id).expect("Pipeline remains Running");
    assert_eq!(run_after.cursor, run_before.cursor);
    assert_eq!(run_after.cycle_nonce, run_before.cycle_nonce);
    assert_eq!(
      run_after.contract_authority.semantic_contract_id,
      run_before.contract_authority.semantic_contract_id
    );
  });
}

#[test]
fn underfunded_manual_occurrence_creates_no_fee_readiness_or_apoptosis() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let sovereign = sovereign_account(actor_id);
    let balance = native_balance(&sovereign);
    deplete_user_sovereign(actor_id, balance - TestMinUserBalance::get());
    clear_fee_collections();

    assert_noop!(
      Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::InsufficientFee
    );

    assert!(fee_collections().is_empty());
    let hot = Actors::actor_hot(actor_id).expect("process remains live");
    assert!(!hot.pending_signal);
    assert!(hot.queue_ticket.is_none());
    assert!(Actors::active_actor_view(actor_id).is_some());
  });
}

#[test]
fn manual_trigger_collection_failure_rolls_back_readiness_and_fee_movement() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let sovereign = sovereign_account(actor_id);
    let sovereign_before = native_balance(&sovereign);
    let sink_before = native_balance(&TestFeeSink::get());
    set_fail_fee_sink_transfer(true);

    assert_noop!(
      Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::InsufficientFee
    );
    set_fail_fee_sink_transfer(false);

    assert_eq!(native_balance(&sovereign), sovereign_before);
    assert_eq!(native_balance(&TestFeeSink::get()), sink_before);
    let hot = Actors::actor_hot(actor_id).expect("process remains live");
    assert!(!hot.pending_signal);
    assert!(hot.queue_ticket.is_none());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn address_event_charges_occurrence_before_pipeline_opening() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      percentage_trigger_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let sovereign = sovereign_account(actor_id);
    let sovereign_before = native_balance(&sovereign);
    clear_fee_collections();

    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      1,
      &ALICE,
    ));

    let fee = address_event_trigger_fee();
    assert_eq!(fee_collections(), vec![fee]);
    assert_eq!(native_balance(&sovereign), sovereign_before - fee);
    let instance = Actors::active_actor_view(actor_id).expect("Actor remains");
    assert!(instance.pending_signal);
    assert_eq!(instance.cycle_nonce, 0);
    assert!(ActorRunStateStore::<Test>::get(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed {
        actor_id: id,
        trigger_family: TriggerFamily::AddressEvent,
        fee: charged,
      } if *id == actor_id && *charged == fee
    )));
  });
}

#[test]
fn repeated_pending_address_event_is_latched_without_trigger_fee() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      percentage_trigger_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    fund_native(actor_id, address_event_trigger_fee());
    clear_fee_collections();

    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      1,
      &ALICE,
    ));
    let ticket = Actors::actor_hot(actor_id)
      .expect("Actor hot state")
      .queue_ticket;
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      1,
      &ALICE,
    ));

    assert_eq!(fee_collections(), vec![address_event_trigger_fee()]);
    let hot = Actors::actor_hot(actor_id).expect("Actor hot state");
    assert!(hot.pending_signal);
    assert_eq!(hot.queue_ticket, ticket);
    assert_eq!(
      System::events()
        .iter()
        .filter(|record| matches!(
          &record.event,
          RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
            actor_id: id,
            trigger_family: TriggerFamily::AddressEvent,
            ..
          }) if *id == actor_id
        ))
        .count(),
      1
    );
  });
}

#[test]
fn busy_address_event_charges_and_latches_only_the_future_pipeline() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
    ])
    .expect("two Steps fit");
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      percentage_trigger_schedule(),
      None,
      plan,
    );
    fund_native(actor_id, 1_000_000);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      1,
      &ALICE,
    ));
    Actors::on_idle(1, Weight::MAX);
    let run_before = ActorRunStateStore::<Test>::get(actor_id).expect("Pipeline is Running");
    clear_fee_collections();
    frame_system::Pallet::<Test>::reset_events();

    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      1,
      &ALICE,
    ));

    assert_eq!(fee_collections(), vec![address_event_trigger_fee()]);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::PipelineFeeCharged { actor_id: id, .. } if *id == actor_id
    )));
    let hot = Actors::actor_hot(actor_id).expect("Actor hot state");
    assert_eq!(hot.cycle_state, CycleState::Running);
    assert!(hot.pending_signal);
    let run_after = ActorRunStateStore::<Test>::get(actor_id).expect("Pipeline remains Running");
    assert_eq!(run_after.cursor, run_before.cursor);
    assert_eq!(run_after.cycle_nonce, run_before.cycle_nonce);
  });
}

#[test]
fn underfunded_address_event_advances_without_fee_readiness_or_apoptosis() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      percentage_trigger_schedule(),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    let sovereign = sovereign_account(actor_id);
    let balance = native_balance(&sovereign);
    deplete_user_sovereign(actor_id, balance - TestMinUserBalance::get());
    clear_fee_collections();

    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      1,
      &ALICE,
    ));

    assert!(fee_collections().is_empty());
    assert_eq!(native_balance(&sovereign), TestMinUserBalance::get());
    let hot = Actors::actor_hot(actor_id).expect("process remains live");
    assert!(!hot.pending_signal);
    assert!(hot.queue_ticket.is_none());
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&1)
    );
  });
}

#[test]
fn address_event_collection_failure_preserves_source_progress_without_readiness() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      percentage_trigger_schedule(),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::one()),
      })),
    );
    let sovereign = sovereign_account(actor_id);
    let sovereign_before = native_balance(&sovereign);
    let sink_before = native_balance(&TestFeeSink::get());
    set_fail_fee_sink_transfer(true);

    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      1,
      &ALICE,
    ));
    set_fail_fee_sink_transfer(false);

    assert_eq!(native_balance(&sovereign), sovereign_before);
    assert_eq!(native_balance(&TestFeeSink::get()), sink_before);
    let hot = Actors::actor_hot(actor_id).expect("process remains live");
    assert!(!hot.pending_signal);
    assert!(hot.queue_ticket.is_none());
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&1)
    );
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed { actor_id: id, .. } if *id == actor_id
    )));
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
fn exact_slot_recovery_contract_accesses_residual_user_custody() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let target_slot = 3;
    let asset = TestAsset::Local(7);
    let recovery_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset,
      amount: AmountResolution::Fixed(333),
    }));
    let first_id = create_user_with_slot(
      ALICE,
      target_slot,
      Mutability::Mutable,
      manual_schedule(),
      None,
      recovery_steps.clone(),
    );
    let first = Actors::active_actor_view(first_id).expect("first actor exists");
    assert_eq!(first.actor_class.owner_slot(), Some(target_slot));
    assert_eq!(
      first.sovereign_account,
      Actors::sovereign_account_id(&ALICE, target_slot)
    );
    set_asset_balance(&first.sovereign_account, asset, 777);
    let native_before_close = native_balance(&first.sovereign_account);
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), first_id));
    assert_eq!(
      native_balance(&first.sovereign_account),
      native_before_close
    );
    assert_eq!(asset_balance(&first.sovereign_account, asset), 777);

    let second_id = create_user_with_slot(
      ALICE,
      target_slot,
      Mutability::Mutable,
      manual_schedule(),
      None,
      recovery_steps,
    );
    let second = Actors::active_actor_view(second_id).expect("recovery actor exists");
    assert_ne!(second_id, first_id);
    assert_eq!(second.actor_class.owner_slot(), Some(target_slot));
    assert_eq!(second.sovereign_account, first.sovereign_account);
    assert_eq!(asset_balance(&second.sovereign_account, asset), 777);

    let bob_before = asset_balance(&BOB, asset);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      second_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&BOB, asset), bob_before + 333);
    assert_eq!(asset_balance(&second.sovereign_account, asset), 444);
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
    let queue_weight = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1)
      .saturating_add(Actors::scheduler_actor_probe_weight_upper())
      .saturating_add(
        <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
          .max(<<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page()),
      );
    // Only the RefTime dimension is exhausted; ProofSize remains unlimited.
    let resources = Actors::load_current_step_from_storage(actor_id, 0)
      .expect("current Step resources exist")
      .resources;
    let ref_time_limit = queue_weight
      .ref_time()
      .saturating_add(resources.control.saturating_add(resources.effect).ref_time())
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
fn non_invoked_task_releases_effect_weight_after_maximum_admission() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(StepOf::<Test> {
        precondition: all_conditions(vec![Predicate::BlockNumberBelow { threshold: 0 }]),
        task: Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(10),
        },
        on_error: StepErrorPolicy::AbortCycle,
      }),
    );
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    let resources = Actors::load_current_step_from_storage(actor_id, 0)
      .expect("current Step resources exist")
      .resources;
    assert_ne!(resources.effect, Weight::zero());
    let scan = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1);
    let probe = Actors::scheduler_actor_probe_weight_upper();
    let consume = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
      .max(<<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page());

    let bob_before = native_balance(&BOB);
    let budget = TestBlockResourceBudget::get();
    let mut resource_state = crate::BlockResourceState::new(1);
    assert_eq!(resource_state.begin_prepass(), Ok(()));
    assert_eq!(resource_state.open_external_phase(), Ok(()));
    assert_eq!(resource_state.begin_drain(), Ok(()));
    let pass = Actors::execute_cycle_to_cutoff_with_resources(
      Weight::MAX,
      Actors::next_queue_ticket(),
      &mut resource_state,
      budget.limits(),
      crate::BlockResourceDomain::ActorDrainEffect,
      budget.limits().actor_control(),
    );

    assert_eq!(
      pass.consumed,
      scan
        .saturating_mul(2)
        .saturating_add(probe)
        .saturating_add(consume)
        .saturating_add(resources.control),
    );
    assert_eq!(
      pass.reconciled_domains(),
      Some((pass.consumed, Weight::zero()))
    );
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert_eq!(resource_state.usage().actor_control_used(), pass.consumed);
    assert_eq!(resource_state.usage().actor_effect_used(), Weight::zero());
    assert_eq!(native_balance(&BOB), bob_before);
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
fn successful_actor_pass_separates_actual_effect_from_control() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let task = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(StepOf::<Test> {
        precondition: all_conditions(vec![Predicate::BlockNumberAbove { threshold: 0 }]),
        task: task.clone(),
        on_error: StepErrorPolicy::AbortCycle,
      }),
    );
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    let budget = TestBlockResourceBudget::get();
    let mut resource_state = crate::BlockResourceState::new(1);
    assert_eq!(resource_state.begin_prepass(), Ok(()));
    assert_eq!(resource_state.open_external_phase(), Ok(()));
    assert_eq!(resource_state.begin_drain(), Ok(()));
    let pass = Actors::execute_cycle_to_cutoff_with_resources(
      Weight::MAX,
      Actors::next_queue_ticket(),
      &mut resource_state,
      budget.limits(),
      crate::BlockResourceDomain::ActorDrainEffect,
      budget.limits().actor_control(),
    );
    let expected_effect =
      <MockTaskEffectWeight as crate::TaskEffectWeightProvider<RuntimeTask>>::actual_effect_weight(
        &task,
        crate::TaskEffectExecution::Invoked,
      )
      .expect("mock invoked effect has actual evidence"); // deos-bypass: panic-owner — mock provider defines total actual evidence for every invoked task.
    assert_eq!(pass.effect_consumed, expected_effect);
    let (control, effect) = pass
      .reconciled_domains()
      .expect("successful pass has complete effect evidence"); // deos-bypass: panic-owner — successful commit returns typed actual control/effect evidence.
    assert_eq!(effect, expected_effect);
    assert_eq!(control.saturating_add(effect), pass.consumed);
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert_eq!(resource_state.usage().actor_control_used(), control);
    assert_eq!(resource_state.usage().actor_effect_used(), effect);
  });
}

#[test]
fn valid_actual_control_replaces_the_maximum_in_pass_consumption() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(StepOf::<Test> {
        precondition: all_conditions(vec![Predicate::BlockNumberBelow { threshold: 0 }]),
        task: Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(10),
        },
        on_error: StepErrorPolicy::AbortCycle,
      }),
    );
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    let resources = Actors::load_current_step_from_storage(actor_id, 0)
      .expect("current Step resources exist")
      .resources;
    let actual_control = resources
      .control
      .checked_sub(&Weight::from_parts(1, 1))
      .expect("mock control maximum is nonzero");
    set_step_control_actual_weight_override(Some(actual_control));
    let scan = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1);
    let probe = Actors::scheduler_actor_probe_weight_upper();
    let consume = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
      .max(<<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page());

    let pass = Actors::execute_cycle(Weight::MAX);

    assert_eq!(
      pass.consumed,
      scan
        .saturating_mul(2)
        .saturating_add(probe)
        .saturating_add(consume)
        .saturating_add(actual_control),
    );
  });
}

#[test]
fn valid_zero_actual_control_charges_pipeline_but_no_action_fee() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(StepOf::<Test> {
        precondition: all_conditions(vec![Predicate::BlockNumberBelow { threshold: 0 }]),
        task: Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(10),
        },
        on_error: StepErrorPolicy::AbortCycle,
      }),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    set_step_control_actual_weight_override(Some(Weight::zero()));
    clear_fee_collections();
    let actor_before = native_balance(&sovereign_account(actor_id));

    let pass = Actors::execute_cycle(Weight::MAX);

    assert!(!pass.starved);
    let pipeline_fee = pipeline_opening_fee(
      &Actors::active_actor_view(actor_id)
        .expect("Actor remains")
        .steps,
    );
    assert_eq!(fee_collections(), vec![pipeline_fee]);
    assert_eq!(
      native_balance(&sovereign_account(actor_id)),
      actor_before.saturating_sub(pipeline_fee)
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepSkipped {
        actor_id: id,
        reason: StepSkippedReason::PreconditionFalse,
        ..
      } if *id == actor_id
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActionFeeCharged { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn valid_zero_actual_effect_releases_effect_fee_after_invocation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    let resources = Actors::load_current_step_from_storage(actor_id, 0)
      .expect("current Step resources exist")
      .resources;
    let expected_action_fee = Actors::step_fee_for_resources(
      ActorType::User,
      crate::ActorStepResourceEnvelope {
        control: resources.control,
        effect: Weight::zero(),
      },
    )
    .expect("actual Action fee is bounded")
    .total_fee;
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    set_task_effect_actual_weight_override(Some(Weight::zero()));
    clear_fee_collections();
    let bob_before = native_balance(&BOB);

    let pass = Actors::execute_cycle(Weight::MAX);

    assert!(!pass.starved);
    assert_eq!(expected_action_fee, 0);
    let pipeline_fee = pipeline_opening_fee(
      &Actors::active_actor_view(actor_id)
        .expect("Actor remains")
        .steps,
    );
    assert_eq!(fee_collections(), vec![pipeline_fee]);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(10));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActionFeeCharged {
        actor_id: id,
        cycle_nonce: 1,
        step_index: 0,
        actual_effect_weight,
        fee: 0,
      } if *id == actor_id && *actual_effect_weight == Weight::zero()
    )));
  });
}

fn assert_missing_actual_weight_rolls_back_without_fee_collection(missing_control: bool) {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    if missing_control {
      set_missing_step_control_actual_weight(true);
    } else {
      set_missing_task_effect_actual_weight(true);
    }
    frame_system::Pallet::<Test>::reset_events();
    clear_fee_collections();
    let bob_before = native_balance(&BOB);
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);

    let budget = TestBlockResourceBudget::get();
    let mut resource_state = crate::BlockResourceState::new(1);
    assert_eq!(resource_state.begin_prepass(), Ok(()));
    assert_eq!(resource_state.open_external_phase(), Ok(()));
    assert_eq!(resource_state.begin_drain(), Ok(()));
    let pass = Actors::execute_cycle_to_cutoff_with_resources(
      Weight::MAX,
      Actors::next_queue_ticket(),
      &mut resource_state,
      budget.limits(),
      crate::BlockResourceDomain::ActorDrainEffect,
      budget.limits().actor_control(),
    );

    assert!(pass.starved);
    assert_eq!(pass.reconciled_domains(), None);
    assert!(resource_state.optional_actor_work_halted());
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before,
    );
    assert_eq!(native_balance(&BOB), bob_before);
    let pipeline_fee = pipeline_opening_fee(
      &Actors::active_actor_view(actor_id)
        .expect("Actor remains")
        .steps,
    );
    assert_eq!(fee_collections(), vec![pipeline_fee]);
    assert!(frame_system::Pallet::<Test>::events().is_empty());
  });
}

#[test]
fn missing_actual_effect_weight_rolls_back_without_fee_collection() {
  assert_missing_actual_weight_rolls_back_without_fee_collection(false);
}

#[test]
fn missing_actual_control_weight_rolls_back_without_fee_collection() {
  assert_missing_actual_weight_rolls_back_without_fee_collection(true);
}

#[test]
fn greater_than_reserved_actual_effect_weight_rolls_back_the_complete_attempt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    set_task_effect_actual_weight_override(Some(Weight::from_parts(34, 44)));
    frame_system::Pallet::<Test>::reset_events();
    let bob_before = native_balance(&BOB);
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);

    let pass = Actors::execute_cycle(Weight::MAX);

    assert!(pass.starved);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before,
    );
    assert_eq!(native_balance(&BOB), bob_before);
    assert!(frame_system::Pallet::<Test>::events().is_empty());
    assert!(Actors::actor_hot(actor_id).is_some_and(|hot| hot.queue_ticket.is_some()));
  });
}

#[test]
fn greater_than_reserved_actual_control_weight_rolls_back_the_complete_attempt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let reserved = Actors::load_current_step_from_storage(actor_id, 0)
      .expect("current Step resources exist")
      .resources
      .control;
    set_step_control_actual_weight_override(Some(Weight::from_parts(
      reserved.ref_time().saturating_add(1),
      reserved.proof_size(),
    )));
    frame_system::Pallet::<Test>::reset_events();
    let bob_before = native_balance(&BOB);
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);

    let pass = Actors::execute_cycle(Weight::MAX);

    assert!(pass.starved);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before,
    );
    assert_eq!(native_balance(&BOB), bob_before);
    assert!(frame_system::Pallet::<Test>::events().is_empty());
    assert!(Actors::actor_hot(actor_id).is_some_and(|hot| hot.queue_ticket.is_some()));
  });
}

#[test]
fn condition_skip_pipeline_fee_failure_aborts_before_skip_event() {
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
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let fee_sink_before = native_balance(&TestFeeSink::get());
    let pipeline_fee = pipeline_opening_fee(
      &Actors::active_actor_view(actor_id)
        .expect("Actor remains")
        .steps,
    );
    clear_fee_collections();
    set_fail_fee_sink_transfer(true);
    run_idle(Weight::MAX);
    set_fail_fee_sink_transfer(false);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&TestFeeSink::get()), fee_sink_before);
    assert_eq!(fee_collections(), vec![pipeline_fee]);
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
fn pipeline_fee_route_failure_aborts_before_task_execution() {
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
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let fee_sink_before = native_balance(&TestFeeSink::get());
    let pipeline_fee = pipeline_opening_fee(
      &Actors::active_actor_view(actor_id)
        .expect("Actor remains")
        .steps,
    );
    clear_fee_collections();
    set_fail_fee_sink_transfer(true);
    run_idle(Weight::MAX);
    set_fail_fee_sink_transfer(false);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&TestFeeSink::get()), fee_sink_before);
    assert_eq!(fee_collections(), vec![pipeline_fee]);
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
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let sink_before = native_balance(&TestFeeSink::get());
    set_fail_fee_sink_transfer(true);
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

    let expected_fee = pipeline_opening_fee(
      &Actors::active_actor_view(actor_id)
        .expect("Actor remains")
        .steps,
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
    let suspended = Actors::actor_run_state(actor_id).expect("funding retry persists");
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
    assert!(Actors::actor_run_state(actor_id).is_none());
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
    let suspended = Actors::active_actor_view(actor_id).expect("suspended");
    assert_eq!(suspended.cycle_state, CycleState::Suspended);
    assert_eq!(suspended.cycle_nonce, 0);
    let run_state = Actors::actor_run_state(actor_id).expect("funding suspension persists");
    assert_eq!(run_state.cycle_nonce, 1);
    assert!(run_state.funding_snapshot.is_empty());
    assert_eq!(
      run_state.last_step_outcome,
      Some(StepOutcome::FundingUnavailable)
    );
    assert_eq!(
      run_state.suspension,
      Some(SuspensionReason::FundingUnavailable)
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
    assert!(
      Actors::actor_run_state(actor_id)
        .expect("later funding does not mutate the open snapshot")
        .funding_snapshot
        .is_empty()
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
    let continuation = Actors::actor_run_state(actor_id).expect("suspended");
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
      Actors::actor_run_state(actor_id)
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
    assert!(Actors::actor_run_state(actor_id).is_none());
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
fn permissionless_sweep_preserves_user_below_future_pipeline_minimum() {
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
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActorClosed { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn user_resolution_skip_charges_pipeline_but_no_action_fee() {
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
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let before = native_balance(&actor);
    clear_fee_collections();
    run_idle(Weight::MAX);
    let after = native_balance(&actor);
    let pipeline_fee = pipeline_opening_fee(
      &Actors::active_actor_view(actor_id)
        .expect("Actor remains")
        .steps,
    );
    assert_eq!(after, before.saturating_sub(pipeline_fee));
    assert_eq!(fee_collections(), vec![pipeline_fee]);
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
fn condition_skip_charges_pipeline_but_no_action_fee() {
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
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    clear_fee_collections();
    run_idle(Weight::MAX);
    let pipeline_fee = pipeline_opening_fee(
      &Actors::active_actor_view(actor_id)
        .expect("Actor remains")
        .steps,
    );
    assert_eq!(fee_collections(), vec![pipeline_fee]);
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
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    clear_fee_collections();
    run_idle(Weight::MAX);
    let pipeline_fee = pipeline_opening_fee(
      &Actors::active_actor_view(actor_id)
        .expect("Actor remains")
        .steps,
    );
    assert_eq!(fee_collections(), vec![pipeline_fee]);
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
fn executable_task_charges_pipeline_and_independent_action_fee() {
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
    let task_weight = Actors::weight_upper_bound(&task);
    assert!(task_weight.ref_time() > 0);
    let expected_action_fee = TestWeightToFee::weight_to_fee(&task_weight);
    let instance = Actors::active_actor_view(actor_id).expect("user actor");
    let current_fee =
      Actors::maximum_contract_step_fee(instance.actor_class.actor_type(), &instance.steps, 0)
        .expect("current-Step fee is bounded")
        .total_fee;
    assert_eq!(current_fee, expected_action_fee);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let actor_before = native_balance(&actor);
    let fee_sink_before = native_balance(&TestFeeSink::get());
    clear_fee_collections();
    run_idle(Weight::MAX);
    let pipeline_fee = pipeline_opening_fee(
      &Actors::active_actor_view(actor_id)
        .expect("Actor remains")
        .steps,
    );
    assert_eq!(
      native_balance(&actor),
      actor_before
        .saturating_sub(pipeline_fee)
        .saturating_sub(expected_action_fee)
        .saturating_sub(1)
    );
    assert_eq!(
      native_balance(&TestFeeSink::get()),
      fee_sink_before
        .saturating_add(pipeline_fee)
        .saturating_add(expected_action_fee)
    );
    assert_eq!(fee_collections(), vec![pipeline_fee, expected_action_fee]);
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
fn adapter_failure_retains_pipeline_and_one_action_fee() {
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
    let pool_before = native_balance(&pool_account);
    let expected_action_fee = TestWeightToFee::weight_to_fee(&Actors::weight_upper_bound(&task));
    set_fail_dex_after_input_transfer(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let actor_before = native_balance(&actor);
    clear_fee_collections();
    run_idle(Weight::MAX);
    set_fail_dex_after_input_transfer(false);
    let pipeline_fee = pipeline_opening_fee(
      &Actors::active_actor_view(actor_id)
        .expect("Actor remains")
        .steps,
    );
    assert_eq!(fee_collections(), vec![pipeline_fee, expected_action_fee]);
    assert_eq!(
      native_balance(&actor),
      actor_before
        .saturating_sub(pipeline_fee)
        .saturating_sub(expected_action_fee)
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
    for (idx, (funding, pct, expect_skip)) in cases.into_iter().enumerate() {
      let task = Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfCurrent(pct),
      };
      let contract_steps = contract_steps_with_step(make_step(task.clone()));
      let pipeline_fee = pipeline_opening_fee(&contract_steps);
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        manual_schedule(),
        None,
        contract_steps,
      );
      fund_native(actor_id, funding);
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      let fee_sink_before = native_balance(&TestFeeSink::get());
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
        assert_eq!(fee_delta, pipeline_fee);
      } else {
        let exec_fee = <TestWeightToFee as polkadot_sdk::sp_weights::WeightToFee>::weight_to_fee(
          &Actors::weight_upper_bound(&task),
        );
        assert_eq!(summary.0, 1);
        assert_eq!(summary.1, 0);
        assert_eq!(summary.2, 0);
        assert_eq!(summary.3, 0);
        assert_eq!(summary.4, 0);
        assert_eq!(fee_delta, pipeline_fee.saturating_add(exec_fee));
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
fn paid_readiness_closes_only_when_pipeline_charge_cannot_fit_above_floor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let prefunded = user_prefunding_requirement(&contract_steps);
    let pipeline_fee = pipeline_opening_fee(&contract_steps);
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
    fund_native(actor_id, manual_trigger_fee().saturating_add(pipeline_fee));
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    // The first Pipeline and Action leave exactly enough for another Trigger occurrence,
    // but not enough to admit another complete Pipeline above MinUserBalance.
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
          reason: CloseReason::CycleAdmissionInsufficient,
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
      Actors::load_actor_contract(system_actor)
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
      Actors::load_actor_contract(user_actor)
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
    assert!(before.trigger_wakeup_pointer.is_some());
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress,
    ));
    let after = Actors::actor_hot(actor_id).expect("actor hot state exists");
    assert_eq!(after.queue_ticket, before.queue_ticket);
    assert_eq!(after.wakeup_pointer, before.wakeup_pointer);
    assert_eq!(after.trigger_wakeup_pointer, before.trigger_wakeup_pointer);
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
    let pipeline_fee = pipeline_opening_fee(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 1_000 + manual_trigger_fee() + pipeline_fee);
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
    assert_eq!(native_balance(&actor), 1_000);
  });
}

#[test]
fn funding_unavailable_releases_action_fee_reservation_for_later_step_spend() {
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
    let pipeline_fee = pipeline_opening_fee(&contract_steps);
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
    fund_native(actor_id, 1_000 + manual_trigger_fee() + pipeline_fee);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    clear_fee_collections();
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
    let expected_attempt_fee =
      TestWeightToFee::weight_to_fee(&Actors::weight_upper_bound(&transfer));
    assert_eq!(fee_collections(), vec![pipeline_fee, expected_attempt_fee]);
    assert_eq!(
      native_balance(&actor),
      250,
      "later Step spends custody released by the non-invoked first Action reservation"
    );
  });
}

#[test]
fn failed_invoked_action_charges_effect_fee_without_refund() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let task = Task::SwapIn {
      asset_in: TestAsset::Native,
      asset_out: TestAsset::Local(99),
      amount_in: AmountResolution::Fixed(10),
      slippage_tolerance: Perbill::zero(),
    };
    let contract_steps = contract_steps_with_step(make_step(task.clone()));
    let prefunded = user_prefunding_requirement(&contract_steps);
    let pipeline_fee = pipeline_opening_fee(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 1_000 + manual_trigger_fee() + pipeline_fee);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    let expected_weight =
      <MockTaskEffectWeight as crate::TaskEffectWeightProvider<RuntimeTask>>::actual_effect_weight(
        &task,
        crate::TaskEffectExecution::Invoked,
      )
      .expect("invoked Action actual Weight exists");
    let expected_fee = TestWeightToFee::weight_to_fee(&expected_weight);
    assert_eq!(
      native_balance(&actor),
      1_000u128.saturating_sub(expected_fee),
      "failed invoked Action charges exactly its effect fee with no refund"
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
    let receipt = System::events()
      .iter()
      .find_map(|record| match &record.event {
        RuntimeEvent::Actors(Event::ActionFeeCharged {
          actor_id,
          cycle_nonce,
          step_index,
          actual_effect_weight,
          fee,
        }) => Some((
          *actor_id,
          *cycle_nonce,
          *step_index,
          *actual_effect_weight,
          *fee,
        )),
        _ => None,
      });
    assert_eq!(
      receipt,
      Some((actor_id, 1, 0, expected_weight, expected_fee))
    );
  });
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
fn user_all_available_preserves_floor_and_underfunded_future_trigger_keeps_process() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let task = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::AllAvailable,
    };
    let contract_steps = contract_steps_with_step(make_step(task));
    let fee = Actors::attempt_fee_envelope(ActorType::User, &contract_steps, 0)
      .expect("User Action fee envelope")
      .total;
    let pipeline_fee = pipeline_opening_fee(&contract_steps);
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
    let pipeline_balance = floor.saturating_add(fee).saturating_add(50);
    fund_native(
      actor_id,
      pipeline_balance
        .saturating_add(manual_trigger_fee())
        .saturating_add(pipeline_fee),
    );
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    clear_fee_collections();
    run_idle(Weight::MAX);

    assert_eq!(native_balance(&actor), floor);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(fee_collections(), vec![pipeline_fee, fee]);

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_noop!(
      Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::InsufficientFee
    );
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
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
    let pipeline_fee = pipeline_opening_fee(&contract_steps);
    let floor = TestMinUserBalance::get();

    // Floor + Pipeline charge - 1 pays the Trigger occurrence but cannot consume the
    // resulting readiness into a Cycle.
    let prefunded = user_prefunding_requirement(&contract_steps);
    let short = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps.clone(),
    );
    deplete_user_sovereign(short, prefunded);
    fund_native(
      short,
      floor
        .saturating_add(pipeline_fee)
        .saturating_sub(1)
        .saturating_add(manual_trigger_fee()),
    );
    let short_sovereign = sovereign_account(short);
    clear_fee_collections();
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), short));
    assert_eq!(fee_collections(), vec![manual_trigger_fee()]);
    let short_after_trigger = native_balance(&short_sovereign);
    run_idle(Weight::MAX);
    assert_eq!(fee_collections(), vec![manual_trigger_fee()]);
    assert_eq!(native_balance(&short_sovereign), short_after_trigger);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::PipelineFeeCharged { actor_id, .. } if *actor_id == short
    )));
    assert!(Actors::active_actor_view(short).is_none());
    assert!(has_actor_event(|e| matches!(
      e,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::CycleAdmissionInsufficient,
        ..
      } if *id == short
    )));

    // floor + Pipeline charge admits exactly and leaves the protected floor for Actions.
    let prefunded = user_prefunding_requirement(&contract_steps);
    let exact = create_user_with(
      BOB,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(exact, prefunded);
    fund_native(
      exact,
      floor
        .saturating_add(pipeline_fee)
        .saturating_add(manual_trigger_fee()),
    );
    clear_fee_collections();
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(BOB), exact));
    run_idle(Weight::MAX);
    assert_eq!(fee_collections(), vec![manual_trigger_fee(), pipeline_fee]);
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
    let fee_envelope = Actors::attempt_fee_envelope(ActorType::User, &contract_steps, 0)
      .expect("User Action fee envelope");
    let pipeline_fee = pipeline_opening_fee(&contract_steps);
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
    let pipeline_balance = floor.saturating_add(fee_envelope.total).saturating_add(50);
    fund_native(
      actor_id,
      pipeline_balance
        .saturating_add(manual_trigger_fee())
        .saturating_add(pipeline_fee),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    clear_fee_collections();
    run_idle(Weight::MAX);

    assert_eq!(
      native_balance(&actor),
      pipeline_balance.saturating_sub(fee_envelope.total)
    );
    assert!(native_balance(&actor) >= floor);
    assert_eq!(fee_collections(), vec![pipeline_fee, fee_envelope.total]);
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
    assert!(Actors::actor_run_state(actor_id).is_none());
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
      .expect("User Action fee envelope")
      .total;
    let pipeline_fee = pipeline_opening_fee(&contract_steps);
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
    // Fund one Trigger occurrence, then the protected floor, current Action fee, and donation.
    let pipeline_balance = floor.saturating_add(fee).saturating_add(60);
    fund_native(
      actor_id,
      pipeline_balance
        .saturating_add(manual_trigger_fee())
        .saturating_add(pipeline_fee),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    clear_fee_collections();
    run_idle(Weight::MAX);
    let (used_a, used_b) = donated_liquidity(actor, asset_a, asset_b);
    assert!(
      used_b <= 60,
      "asset-b fee-native debit must respect its preservable capacity"
    );
    assert_eq!(
      native_balance(&actor),
      pipeline_balance.saturating_sub(used_b).saturating_sub(fee),
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
fn user_pipeline_admission_combines_adapter_lock_with_machine_charge() {
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
    let residual_asset = TestAsset::Local(8);
    set_asset_balance(&sovereign, residual_asset, 808);
    // A transfer lock leaves enough reducible balance for the Trigger occurrence but not
    // the complete Pipeline charge above MinUserBalance, so no Step is attempted.
    set_native_transfer_lock(&sovereign, 150);
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let native_after_trigger = native_balance(&sovereign);
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(native_balance(&sovereign), native_after_trigger);
    assert_eq!(asset_balance(&sovereign, residual_asset), 808);
    assert!(has_actor_event(|e| matches!(
      e,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::CycleAdmissionInsufficient,
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
    frame_system::Pallet::<Test>::set_block_number(7);
    Actors::on_initialize(7);
    Actors::on_idle(7, Weight::MAX);
    assert!(
      has_actor_event(|e| matches!(
        e,
        Event::TransferExecuted { actor_id: id, asset: TestAsset::Native, to, .. }
        if *id == actor_id && *to == BOB
      )),
      "Step 0 should execute when spendable native > 5000"
    );
    frame_system::Pallet::<Test>::set_block_number(7);
    Actors::on_idle(7, Weight::MAX);
    // Second evaluation: slash native so spendable < 500 while preserving Pipeline admission.
    // Raw 600 pays the complete Pipeline at Opening; the current Action reserve leaves < 500.
    let actor_native = native_balance(&actor);
    let _ = <Balances as Currency<AccountId>>::slash(&actor, actor_native.saturating_sub(600));
    let charlie_before = asset_balance(&CHARLIE, foreign);
    frame_system::Pallet::<Test>::set_block_number(11);
    Actors::on_initialize(11);
    Actors::on_idle(11, Weight::MAX);
    frame_system::Pallet::<Test>::set_block_number(12);
    Actors::on_idle(12, Weight::MAX);
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
