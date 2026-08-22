use super::*;

#[test]
fn active_dirty_list_rotates_fairly_and_repairs_cursor_on_removal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actors = [1u32, 2, 3].map(|feed| {
      create_system_with(
        ALICE,
        observation_schedule(vec![feed]),
        None,
        inert_contract_steps(),
      )
    });
    for feed in [1u32, 2, 3] {
      assert_ok!(Actors::note_observation_changed(feed, 1));
    }
    let list = Actors::dirty_observation_list();
    assert_eq!(
      (list.head, list.tail, list.cursor, list.count),
      (Some(1), Some(3), Some(1), 3)
    );
    assert_eq!(
      Actors::dirty_observation_feeds(2)
        .expect("middle dirty feed")
        .previous_dirty_feed,
      Some(1)
    );

    assert!(crate::Pallet::<Test>::do_fanout_dirty_observation_page().expect("first page"));
    assert!(Actors::dirty_observation_feeds(1).is_none());
    assert_eq!(Actors::dirty_observation_list().cursor, Some(2));
    assert!(Actors::pending_signal(actors[0]));

    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      actors[1]
    ));
    let repaired = Actors::dirty_observation_list();
    assert_eq!(
      (
        repaired.head,
        repaired.tail,
        repaired.cursor,
        repaired.count
      ),
      (Some(3), Some(3), Some(3), 1)
    );
    let last = Actors::dirty_observation_feeds(3).expect("last dirty feed");
    assert_eq!(
      (last.previous_dirty_feed, last.next_dirty_feed),
      (None, None)
    );

    assert!(!crate::Pallet::<Test>::do_fanout_dirty_observation_page().expect("last page"));
    assert_eq!(Actors::dirty_observation_list(), Default::default());
    assert!(Actors::pending_signal(actors[2]));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn reactivation_with_positive_nonce_uses_schedule_anchor_for_cooldown() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 10,
    };
    let actor_id = create_system_with(
      ALICE,
      schedule.clone(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    // First run: nonce 0 -> 1, last_cycle_block = Some(1).
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("active")
        .cycle_nonce,
      1
    );
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot").last_cycle_block,
      Some(1)
    );
    // Deactivate at block 5.
    frame_system::Pallet::<Test>::set_block_number(5);
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    // Reactivate at block 8 with the same contract: the fresh hot state has no
    // last_cycle_block, so the conservative schedule_anchor (8) anchors cooldown.
    frame_system::Pallet::<Test>::set_block_number(8);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      system_active_contract(schedule, None, transfer_contract_steps(BOB, 10))
        .expect("direct Actor Contract"),
    ));
    let instance = Actors::active_actor_view(actor_id).expect("reactivated");
    assert_eq!(instance.cycle_nonce, 1);
    assert_eq!(instance.schedule_anchor, 8);
    assert_eq!(instance.last_cycle_block, None);
    // A manual trigger at block 8 must NOT fire immediately: cooldown runs from the
    // anchor (8 + 10 = 18), not from block zero.
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("still active")
        .cycle_nonce,
      1,
      "reactivated actor must not become immediately eligible"
    );
    // At block 18 the cooldown has elapsed and the run proceeds.
    frame_system::Pallet::<Test>::set_block_number(18);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("active")
        .cycle_nonce,
      2
    );
  });
}

#[test]
fn create_admission_enforces_both_idle_weight_dimensions_before_charging() {
  new_test_ext().execute_with(|| {
    let contract_steps = transfer_contract_steps(BOB, 10);
    let required = Actors::contract_steps_admission_weight_upper(ActorType::User, &contract_steps);
    let fixed = <TestWeightInfo as crate::WeightInfo>::scheduler_on_idle_base()
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::materialization_coordinator_base())
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1))
      .saturating_add(TestWakeupWeightLimit::get())
      .saturating_add(TestCrossingWorkerWeightLimit::get())
      .saturating_add(TestObservationFanoutWeightLimit::get());
    let gross_required = required.saturating_add(fixed);
    set_guaranteed_on_idle_weight(gross_required);
    prefund_active_user_creation(ALICE, &contract_steps);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, contract_steps.clone()),
    ));
    let owner_before = native_balance(&BOB);
    set_guaranteed_on_idle_weight(Weight::from_parts(
      gross_required.ref_time(),
      gross_required.proof_size().saturating_sub(1),
    ));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(BOB),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, contract_steps),
      ),
      Error::<Test>::ContractStepsExceedOnIdleBudget
    );
    assert_eq!(native_balance(&BOB), owner_before);
  });
}

#[test]
fn plan_updates_reject_a_prospective_run_above_the_idle_budget() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let before = Actors::active_actor_view(actor_id).expect("Actors exists");
    let replacement = transfer_contract_steps(BOB, 10);
    let required = Actors::contract_steps_admission_weight_upper(ActorType::User, &replacement);
    set_guaranteed_on_idle_weight(Weight::from_parts(
      required.ref_time(),
      required.proof_size().saturating_sub(1),
    ));
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        replacement,
        crate::CompletionPolicy::Persistent,
      ),
      Error::<Test>::ContractStepsExceedOnIdleBudget
    );
    assert_eq!(Actors::active_actor_view(actor_id), Some(before));
  });
}

#[test]
fn test_weight_fallback_equals_reference_interface_for_all_classes() {
  type Reference = crate::weights::SubstrateWeight<Test>;
  macro_rules! same {
    ($($method:ident),+ $(,)?) => {$({
      assert_eq!(
        <TestWeightInfo as crate::WeightInfo>::$method(),
        <Reference as crate::WeightInfo>::$method(),
        stringify!($method),
      );
    })+};
  }
  same!(
    create_user_actor,
    create_user_actor_at_slot,
    create_system_actor,
    create_system_actor_at_sovereign_id,
    create_dormant_system_actor,
    activate_actor,
    deactivate_actor,
    pause_actor,
    resume_actor,
    manual_trigger,
    observation_change_ingress,
    observation_fanout_base,
    observation_fanout_page,
    close_actor,
    fee_collection,
    cycle_orchestration,
    task_transfer,
    task_burn,
    task_mint,
    task_stop_cycle,
    xcm_asset_deposit,
    task_add_liquidity,
    task_donate_liquidity,
    task_remove_liquidity,
    task_stake,
    task_unstake,
    task_dex_exact_in,
    task_dex_exact_out,
    scheduler_on_idle_base,
    scheduler_paged_append_existing_page,
    scheduler_paged_append_new_page,
    scheduler_wakeup_append_existing_page,
    scheduler_wakeup_append_new_page,
    scheduler_wakeup_replace_exact,
    scheduler_wakeup_invalidate_middle_page,
    scheduler_wakeup_drain_partial_page,
    scheduler_wakeup_drain_full_page,
    scheduler_wakeup_drain_dense_boundary,
    scheduler_wakeup_drain_stale_page,
    scheduler_wakeup_cursor_insert,
    scheduler_wakeup_cursor_pop_min,
    scheduler_wakeup_cursor_remove_exact,
    scheduler_wakeup_cursor_worker_partial,
    scheduler_wakeup_cursor_worker_remove,
    scheduler_wakeup_cursor_worker_future,
    scheduler_paged_consume_preserve_page,
    scheduler_paged_consume_delete_page,
    scheduler_actor_state_probe,
    transaction_extension_ingress_base,
    transaction_extension_ingress_notify,
    continuation_retry,
    continuation_complete,
    continuation_cancel,
    update_contract,
    set_global_circuit_breaker,
    set_active_actor_limit,
    permissionless_sweep,
  );
  macro_rules! same_at {
    ($method:ident, $($value:expr),+ $(,)?) => {$({
      assert_eq!(
        <TestWeightInfo as crate::WeightInfo>::$method($value),
        <Reference as crate::WeightInfo>::$method($value),
        concat!(stringify!($method), " parameterized"),
      );
    })+};
  }
  same_at!(predicate_set_evaluation, 0, 1, 8);
  same_at!(task_split_transfer, 0, 1, 8);
  same_at!(step_orchestration, 0, 1, 8);
  same_at!(scheduler_paged_tombstone_drain, 0, 1, 10_000);
  same_at!(scheduler_paged_mixed_scan, 0, 1, 10_000);
  same_at!(scheduler_paged_execute_cheap, 0, 1, 1_000);
  same_at!(scheduler_paged_execute_cheap_mixed, 0, 1, 1_000);
  same_at!(funding_snapshot_open, 0, 1, 10);
  same_at!(continuation_suspend, 0, 1, 16);
  same_at!(continuation_suffix_admission, 0, 1, 8);
  same_at!(permissionless_sweep_many, 0, 1, 5);
}

#[test]
fn fresh_system_creation_cannot_capture_a_vacant_locator() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // A vacant locator exists after close. A fresh (non-reattachment) creation always
    // derives its own unregistered next id, so it can never capture the vacant locator's
    // registry entry; reattachment remains the only path authorized to regain control.
    let first_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), first_id));
    assert_eq!(
      Actors::system_sovereigns(first_id),
      Some(SystemSovereignState::Vacant)
    );
    let next_before = Actors::next_actor_id();
    let fresh_id = create_system_with(
      BOB,
      manual_schedule(),
      None,
      transfer_contract_steps(CHARLIE, 1),
    );
    assert_eq!(fresh_id, next_before);
    assert_ne!(fresh_id, first_id);
    assert_eq!(
      Actors::system_sovereigns(first_id),
      Some(SystemSovereignState::Vacant),
      "fresh creation must not capture the vacant locator"
    );
    assert_eq!(
      Actors::system_sovereigns(fresh_id),
      Some(SystemSovereignState::Occupied(fresh_id))
    );
  });
}

#[test]
fn create_atomicity_checkpoint_failure_rolls_back_all_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    set_fail_create_checkpoint(true);
    let actor_id = Actors::next_actor_id();
    let active_before = Actors::active_actor_count();
    let expected_sovereign = Actors::sovereign_account_id(&ALICE, 0);
    let owner_before = native_balance(&ALICE);
    let sink_before = native_balance(&TestFeeSink::get());
    prefund_active_user_creation(ALICE, &transfer_contract_steps(BOB, 1));
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1)),
      ),
      DispatchError::Other("AtomicityCreateCheckpointFailed")
    );
    assert_eq!(Actors::next_actor_id(), actor_id);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(SovereignIndex::<Test>::get(&expected_sovereign), None);
    assert_eq!(OwnerSlotBitmaps::<Test>::get(ALICE), [0; 32]);
    assert_eq!(Actors::active_actor_count(), active_before);
    assert_eq!(ActorHot::<Test>::iter_keys().count() as u32, active_before);
    assert_eq!(native_balance(&ALICE), owner_before);
    assert_eq!(native_balance(&TestFeeSink::get()), sink_before);
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn control_transition_reuses_existing_transaction_at_depth_limit() {
  fn run_nested(depth: u32, actor_id: ActorId) -> polkadot_sdk::sp_runtime::DispatchResult {
    if depth == 0 {
      return Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id);
    }
    polkadot_sdk::frame_support::storage::with_transaction(|| {
      match run_nested(depth - 1, actor_id) {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    })
  }

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );

    assert_ok!(run_nested(
      u32::from(polkadot_sdk::frame_support::storage::transactional::TRANSACTIONAL_LIMIT) - 2,
      actor_id,
    ));
    assert!(Actors::pending_signal(actor_id));
  });

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();

    assert_noop!(
      run_nested(
        u32::from(polkadot_sdk::frame_support::storage::transactional::TRANSACTIONAL_LIMIT) - 1,
        actor_id,
      ),
      Error::<Test>::SchedulerIndexExhausted
    );
    assert!(!Actors::pending_signal(actor_id));
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn deactivation_checkpoint_failure_rolls_back_state_and_event() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let events_before = System::events();
    set_fail_create_checkpoint(true);

    assert_noop!(
      Actors::deactivate_actor(RuntimeOrigin::signed(ALICE), actor_id),
      DispatchError::Other("AtomicityCreateCheckpointFailed")
    );
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert_eq!(Actors::active_actor_count(), 1);
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn activation_checkpoint_failure_rolls_back_state_and_event() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      None,
    ));
    let actor_id = Actors::next_actor_id() - 1;
    frame_system::Pallet::<Test>::set_block_number(2);
    let identity_before = ActorIdentities::<Test>::get(actor_id);
    fund_native_raw(
      &identity_before
        .as_ref()
        .expect("dormant identity exists")
        .sovereign_account,
      user_prefunding_requirement(&transfer_contract_steps(BOB, 1)),
    );
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    set_fail_create_checkpoint(true);

    assert_noop!(
      Actors::activate_actor(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 1))
          .expect("direct Actor Contract"),
      ),
      DispatchError::Other("AtomicityCreateCheckpointFailed")
    );

    assert_eq!(ActorIdentities::<Test>::get(actor_id), identity_before);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(Actors::active_actor_count(), 0);
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn create_rejects_empty_whitelist_filter() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let empty_whitelist: BoundedVec<AccountId, <Test as crate::Config>::MaxWhitelistSize> =
      BoundedVec::default();
    let schedule =
      on_address_event_schedule(SourceFilter::Whitelist(empty_whitelist), AssetFilter::Any);
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
fn whitelist_size_is_bounded_by_runtime_type_limit() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_whitelist = <<Test as crate::Config>::MaxWhitelistSize as Get<u32>>::get() as usize;
    let within_limit = (0..max_whitelist)
      .map(|offset| 50u64.saturating_add(offset as u64))
      .collect::<Vec<_>>();
    let above_limit = (0..max_whitelist.saturating_add(1))
      .map(|offset| 50u64.saturating_add(offset as u64))
      .collect::<Vec<_>>();
    assert!(
      BoundedVec::<AccountId, <Test as crate::Config>::MaxWhitelistSize>::try_from(within_limit)
        .is_ok()
    );
    assert!(
      BoundedVec::<AccountId, <Test as crate::Config>::MaxWhitelistSize>::try_from(above_limit)
        .is_err()
    );
  });
}

#[test]
fn create_rejects_timer_delay_above_max() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_delay = u32::try_from(<Test as crate::Config>::MaxCadenceDelayTicks::get())
      .expect("test cadence horizon fits u32");
    let schedule = timer_schedule(max_delay.saturating_add(1));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(schedule, None, transfer_contract_steps(BOB, 1)),
      ),
      Error::<Test>::ExecutionDelayTooLong
    );
  });
}

#[test]
fn address_event_waits_through_cooldown_without_second_signal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::address_event(SourceFilter::Any, AssetFilter::Any),
      cooldown_blocks: 5,
    };
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      schedule,
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 2_000);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      1
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert!(Actors::actor_hot(actor_id).is_some_and(|hot| hot.pending_signal));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(6));
    frame_system::Pallet::<Test>::set_block_number(6);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      2
    );
    assert!(!Actors::actor_hot(actor_id).is_some_and(|hot| hot.pending_signal));
  });
}

#[test]
fn weight_deferral_is_silent_when_both_dimensions_are_exhausted() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    let queue_weight = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1)
      .saturating_add(Actors::scheduler_actor_probe_weight_upper())
      .saturating_add(
        <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
          .max(<<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page()),
      );
    // Both dimensions are exhausted one unit below the full cycle envelope.
    let limit = Weight::from_parts(
      queue_weight
        .ref_time()
        .saturating_add(Actors::attempt_weight_upper_bound(&instance, 0).ref_time())
        .saturating_sub(1),
      queue_weight
        .proof_size()
        .saturating_add(Actors::attempt_weight_upper_bound(&instance, 0).proof_size())
        .saturating_sub(1),
    );
    Actors::execute_cycle(limit);
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(instance.pending_signal);
    assert_eq!(instance.cycle_nonce, 0);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. } | Event::CycleSummary { actor_id: id, .. }
        if *id == actor_id
    )));
  });
}

#[test]
fn live_head_consume_rolls_back_on_missing_or_malformed_head_page() {
  for malformed in [false, true] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert!(Actors::paged_enqueue(actor_id));
      if malformed {
        QueuePages::<Test>::insert(0, BoundedVec::default());
      } else {
        QueuePages::<Test>::remove(0);
      }
      let events_before = System::events();
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

      assert_eq!(
        Actors::paged_consume_head_at(0),
        Err(crate::EnqueueOutcome::CorruptedTopology)
      );

      assert_eq!(System::events(), events_before);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        root_before
      );
      assert_eq!(
        Actors::actor_hot(actor_id).expect("hot").queue_ticket,
        Some(0)
      );
    });
  }
}

#[test]
fn typed_ingress_zero_movement_creates_no_ingress() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, 10),
    );
    let sovereign = sovereign_account(actor_id);
    let event = crate::AddressEvent {
      destination: sovereign,
      source: Some(ALICE),
      asset: TestAsset::Native,
      amount: 0,
      provenance: Some(crate::FundingProvenance::Signed),
    };
    assert_ok!(Actors::notify_ingress(&event));
    let hot = Actors::actor_hot(actor_id).expect("hot state");
    assert!(
      !hot.pending_signal,
      "zero movement must not latch readiness (spec 5.3)"
    );
    assert!(hot.queue_ticket.is_none(), "zero movement must not enqueue");
    let funding = crate::ActorFunding::<Test>::get(actor_id).expect("funding state");
    assert!(
      funding.funding_accumulated.is_empty(),
      "zero movement must not accumulate funding"
    );
  });
}

#[test]
fn strict_head_of_line_heavy_head_deferral_preserves_follower_order() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // Heavy head first: three transfer steps make its cycle envelope strictly larger than the
    // single-step followers behind it.
    let step = |amount| {
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(amount),
      })
    };
    let heavy_plan = BoundedVec::try_from(vec![step(1), step(2), step(3)]).expect("plan fits");
    let head = create_system_with(ALICE, manual_schedule(), None, heavy_plan);
    fund_native(head, 10_000);
    let light_a = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    fund_native(light_a, 10_000);
    let light_b = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    fund_native(light_b, 10_000);
    for actor_id in [head, light_a, light_b] {
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
    }
    let tickets: Vec<_> = [head, light_a, light_b]
      .into_iter()
      .map(|id| {
        Actors::actor_hot(id)
          .and_then(|hot| hot.queue_ticket)
          .expect("triggered actor is queued")
      })
      .collect();
    assert_eq!(
      tickets,
      vec![0, 1, 2],
      "physical FIFO order is head, light A, light B"
    );

    // Constrained remainder: admits the head's probes and consume but not the head's full cycle
    // admission, while the lighter followers would fit. The pass must stop at the head.
    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(starvation_blocked_budget(head));

    let head_inst = Actors::active_actor_view(head).expect("head survives deferral");
    assert_eq!(
      head_inst.cycle_nonce, 0,
      "head attempt is deferred, not admitted"
    );
    assert_eq!(head_inst.pending_signal, true);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. } | Event::CycleSummary { actor_id: id, .. }
        if *id == head
    )));
    for (id, ticket) in [(light_a, 1), (light_b, 2)] {
      let inst = Actors::active_actor_view(id).expect("follower survives");
      assert_eq!(
        inst.cycle_nonce, 0,
        "follower never admitted behind the head"
      );
      assert!(
        Actors::actor_hot(id).is_some_and(|hot| hot.queue_ticket == Some(ticket)),
        "follower retains its exact physical ticket"
      );
    }
    assert!(
      !has_actor_event(|event| matches!(
        event,
        Event::CycleStarted { actor_id: id, .. } if *id == light_a || *id == light_b
      )),
      "no follower attempt starts behind an unadmitted head"
    );

    // Conforming full envelope: the head advances first, then followers in exact ticket order.
    frame_system::Pallet::<Test>::set_block_number(3);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let started: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(Event::CycleStarted { actor_id, .. }) => Some(actor_id),
        _ => None,
      })
      .collect();
    assert_eq!(started, vec![head, light_a, light_b]);
    for id in [head, light_a, light_b] {
      assert_eq!(
        Actors::active_actor_view(id)
          .expect("actor executed")
          .cycle_nonce,
        1
      );
    }
  });
}

#[test]
fn schedule_anchor_is_the_canonical_initial_timer_anchor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(7);
    let actor_id = create_system_with(ALICE, timer_schedule(20), None, inert_contract_steps());
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(instance.schedule_anchor, 7);
    assert_eq!(instance.last_cycle_block, None);
    assert_eq!(scheduled_wakeup_block(actor_id), Some(27));
  });
}

#[test]
fn terminal_membership_rolls_back_atomically_with_failed_schedule_replacement() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_contract_steps(),
    );
    // Corrupt the existing wakeup cursor so the replacement placement must roll back; the
    // terminal membership update and the whole control transaction revert together.
    WakeupBuckets::<Test>::mutate(WakeupKey::Block(102), |maybe| {
      maybe.as_mut().expect("bucket").cursor_index = None;
    });
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert!(
      update_contract_partial!(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        timer_schedule(5),
        Some(ScheduleWindow { start: 1, end: 201 }),
      )
      .is_err()
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot").terminal_at,
      Some(102),
      "terminal membership stays at the original window terminal"
    );
  });
}

#[test]
fn matching_runtime_simulation_reports_stop_and_rolls_everything_back() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = BoundedVec::try_from(vec![
      make_step(Task::StopCycle),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(5),
      }),
    ])
    .expect("two steps fit");
    let contract = system_active_contract(manual_schedule(), None, contract_steps.clone())
      .expect("direct Actor Contract");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let events_before = System::events();
    let actor_before = Actors::active_actor_view(actor_id).expect("actor exists");

    let result = Actors::simulate_current_contract(
      actor_id,
      ActorType::System,
      Mutability::Mutable,
      contract,
      SimulationMode::FreshCurrentPlan,
    )
    .expect("ready current plan simulates");

    assert_eq!(result.status, AttemptDisposition::Completed);
    assert_eq!(result.cumulative_outcomes.executed_steps, 1);
    assert_eq!(
      result.steps,
      vec![SimulationStepRecord {
        step_index: 0,
        outcome: StepOutcome::Stopped,
      }]
    );
    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(System::events(), events_before);
    assert!(Actors::continuation_state(actor_id).is_none());
  });
}

#[test]
fn canonical_head_discovery_distinguishes_empty_head_and_blocked() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let scan = <TestWeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1);
    assert_eq!(Actors::test_head_discovery(0, 1, 0, scan), (0, None, 0));

    let system = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::paged_enqueue(system));
    let cutoff = Actors::next_queue_ticket();
    let (state, entry, scanned) = Actors::test_head_discovery(cutoff, 1, 0, scan);
    assert_eq!((state, scanned), (1, 1));
    assert_eq!(entry.map(|entry| entry.actor_id), Some(system));

    assert_eq!(
      Actors::test_head_discovery(cutoff, 1, 1, scan),
      (4, None, 1),
      "an exhausted scan ceiling is a silent pass exhaustion"
    );
    assert_eq!(
      Actors::test_head_discovery(cutoff, 1, 0, Weight::zero()),
      (2, None, 0),
      "an unadmitted live-head probe is a weight stall"
    );
    QueueTail::<Test>::mutate(|tail| *tail = tail.saturating_add(1));
    assert_eq!(
      Actors::test_head_discovery(cutoff, 1, 0, scan),
      (3, None, 0),
      "defensive topology rejection is an invariant stall"
    );
    assert!(Actors::execute_cycle(Weight::MAX).starved);
  });
}

#[test]
fn signal_during_suspension_latches_a_later_logical_run() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let schedule = on_address_event_schedule(SourceFilter::Any, AssetFilter::Any);
    let actor_id = create_system_with(ALICE, schedule, None, temporary_retry_swap_plan());
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      1,
      &ALICE
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("suspended")
        .cycle_nonce,
      1
    );
    assert!(!Actors::pending_signal(actor_id));
    let retry_ticket = Actors::actor_hot(actor_id)
      .expect("suspended actor")
      .queue_ticket;
    assert!(retry_ticket.is_some());

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      1,
      &ALICE
    ));
    assert!(Actors::pending_signal(actor_id));
    assert_eq!(
      Actors::actor_hot(actor_id)
        .expect("suspended actor")
        .queue_ticket,
      retry_ticket
    );
    set_temporary_dex_failure(false);
    run_idle(Weight::MAX);

    let after_retry = Actors::active_actor_view(actor_id).expect("retry completes");
    assert_eq!(after_retry.cycle_nonce, 1);
    assert_eq!(after_retry.cycle_state, CycleState::Idle);
    assert!(after_retry.pending_signal);
    assert!(after_retry.queue_ticket.is_some());

    frame_system::Pallet::<Test>::set_block_number(3);
    run_idle(Weight::MAX);
    let after_next_run = Actors::active_actor_view(actor_id).expect("later run completes");
    assert_eq!(after_next_run.cycle_nonce, 2);
    assert!(!after_next_run.pending_signal);
  });
}

#[test]
fn explicit_cancellation_preserves_committed_effects_and_emits_terminal_summary() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let plan = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(10),
      }),
      StepOf::<Test> {
        precondition: None,
        task: Task::SwapIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
          slippage_tolerance: Perbill::one(),
        },
        on_error: RETRY_LATER,
      },
    ])
    .expect("two-step plan fits");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    let actor = sovereign_account(actor_id);
    fund_native(actor_id, 100);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      20,
      &ALICE
    ));
    let bob_before = native_balance(&BOB);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      7,
      &ALICE
    ));
    let actor_before_cancel = native_balance(&actor);
    let failures_before_cancel = Actors::active_actor_view(actor_id)
      .expect("suspended actor")
      .unsuccessful_attempt_streak;

    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    assert_ok!(Actors::cancel_continuation(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(Actors::continuation_state(actor_id).is_none());
    let actor_state = Actors::active_actor_view(actor_id).expect("cancelled actor remains");
    assert_eq!(actor_state.cycle_state, CycleState::Idle);
    assert_eq!(actor_state.cycle_nonce, 1);
    assert_eq!(
      actor_state.unsuccessful_attempt_streak,
      failures_before_cancel
    );
    assert!(actor_state.queue_ticket.is_none());
    assert!(
      Actors::actor_hot(actor_id)
        .expect("cancelled actor hot state")
        .wakeup_pointer
        .is_none()
    );
    assert_eq!(native_balance(&actor), actor_before_cancel);
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&7)
    );
    let events: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect();
    assert_eq!(events.len(), 2);
    assert!(matches!(
      events[0],
      Event::CycleCancelled {
        actor_id: id,
        cycle_nonce: 1,
        reason: CancellationReason::Explicit,
      } if id == actor_id
    ));
    assert!(matches!(
      events[1],
      Event::CycleSummary {
        actor_id: id,
        cycle_nonce: 1,
        result: CycleResult::Cancelled,
        outcomes: OutcomeTotals { executed_steps: 1, failed_steps: 1, .. },
        ..
      } if id == actor_id
    ));
    assert_noop!(
      Actors::cancel_continuation(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ContinuationNotFound
    );
  });
}

#[test]
fn cancellation_is_mutable_only() {
  new_test_ext().execute_with(|| {
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Immutable,
      system_active_contract(timer_schedule(1), None, inert_contract_steps()),
    ));
    assert_noop!(
      Actors::cancel_continuation(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ImmutableActor
    );
  });
}

#[test]
fn permissionless_sweep_many_ignores_missing_ids() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let prefunded = user_prefunding_requirement(&transfer_contract_steps(BOB, 1));
    let user_a = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    deplete_user_sovereign(user_a, prefunded);
    let missing_id = user_a.saturating_add(10_000);
    let sweep_ids: BoundedVec<u64, <Test as crate::Config>::MaxSweepBatch> =
      BoundedVec::try_from(vec![user_a, missing_id]).expect("batch fits");
    assert_ok!(Actors::permissionless_sweep_many(
      RuntimeOrigin::signed(CHARLIE),
      sweep_ids,
    ));
    assert!(Actors::active_actor_view(user_a).is_none());
    assert!(has_actor_event(|event| {
      matches!(
        event,
        Event::SweepBatchProcessed {
          requested: 2,
          closed: 1,
          alive: 0,
          missing: 1,
        }
      )
    }));
  });
}

#[test]
fn on_initialize_is_a_zero_weight_noop() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_eq!(Actors::on_initialize(2), Weight::zero());
    let inst = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(inst.cycle_nonce, 0);
    assert!(!has_actor_event(|event| {
      matches!(
        event,
        Event::CycleStarted {
          actor_id: id,
          cycle_nonce: 1,
        } if *id == actor_id
      )
    }));
  });
}

#[test]
fn breaker_keeps_explicit_repair_sweep_operational() {
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
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    assert_ok!(Actors::permissionless_sweep(
      RuntimeOrigin::signed(BOB),
      actor_id,
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
fn any_verified_ingress_accepts_each_verified_context_field_but_not_all_none() {
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
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress
    ));
    assert_ok!(Actors::notify_internal_address_event(
      actor_id,
      TestAsset::Native,
      40,
      &CHARLIE
    ));
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      30,
      &BOB
    ));
    assert_ok!(Actors::notify_xcm_address_event(
      actor_id,
      TestAsset::Native,
      20,
      &CHARLIE
    ));
    assert_ok!(Actors::notify_address_event_without_source(
      actor_id,
      TestAsset::Native,
      1_000
    ));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&90)
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ContractUpdated { actor_id: id } if *id == actor_id
    )));

    crate::ActorFunding::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("funding state")
        .funding_accumulated
        .get_mut(&TestAsset::Native)
        .map(|accumulated| *accumulated = u128::MAX);
    });
    assert_noop!(
      Actors::preflight_funding_event(actor_id, TestAsset::Native, 1, Some(&ALICE), None,),
      Error::<Test>::FundingAccumulatorOverflow
    );
    assert_noop!(
      Actors::preflight_funding_event(
        actor_id,
        TestAsset::Native,
        1,
        None,
        Some(&crate::FundingProvenance::Xcm),
      ),
      Error::<Test>::FundingAccumulatorOverflow
    );
    assert_ok!(Actors::preflight_funding_event(
      actor_id,
      TestAsset::Native,
      1,
      None,
      None,
    ));
  });
}

#[test]
fn any_verified_ingress_third_party_shapes_basis_only_with_real_delivered_value() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
      })),
    );
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      FundingSourcePolicy::AnyVerifiedIngress,
    ));
    let actor = sovereign_account(actor_id);
    let delivered = 100;
    let actor_before = native_balance(&actor);
    let bob_before = native_balance(&BOB);
    assert_ok!(MockAssetOps::transfer(
      &CHARLIE,
      &actor,
      TestAsset::Native,
      delivered,
    ));
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      delivered,
      &CHARLIE,
    ));
    assert_eq!(
      actor_funding(actor_id)
        .funding_accumulated
        .get(&TestAsset::Native),
      Some(&delivered),
    );

    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);

    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
    assert_eq!(native_balance(&actor), actor_before.saturating_add(50));
  });
}

#[test]
fn identity_control_clock_tracks_creation_and_survives_deactivation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(7);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract(manual_schedule(), None, inert_contract_steps()),
    ));
    let actor_id = Actors::next_actor_id().saturating_sub(1);
    assert_eq!(
      Actors::actor_identities(actor_id)
        .expect("created identity")
        .last_control_mutation_block,
      7
    );
    frame_system::Pallet::<Test>::set_block_number(8);
    assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), actor_id));
    assert_eq!(
      Actors::actor_identities(actor_id)
        .expect("dormant identity")
        .last_control_mutation_block,
      8
    );
  });
}

#[test]
fn canonical_control_replacements_are_exact_noops_before_rate_limiting() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    let before = Actors::active_actor_view(actor_id).expect("actor exists");
    let funding_before = actor_funding(actor_id);
    let funding_policy_before = ActorContracts::<Test>::get(actor_id)
      .expect("active Actor Contract")
      .funding;
    System::reset_events();
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      funding_policy_before,
    ));
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      RuntimeSchedule {
        trigger: before.trigger.clone(),
        cooldown_blocks: before.cooldown_blocks,
      },
      before.window,
    ));
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      before.steps.clone(),
      before.completion,
    ));
    assert_eq!(Actors::active_actor_view(actor_id), Some(before));
    assert_eq!(actor_funding(actor_id), funding_before);
    assert!(System::events().is_empty());
  });
}

#[test]
fn stale_tracked_snapshot_remains_valid_until_overwritten() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::PercentageOfLastFunding(Perbill::from_percent(50)),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let bob_before = native_balance(&BOB);
    assert_ok!(ordinary_transfer_to_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      TestAsset::Native,
      100
    ));
    frame_system::Pallet::<Test>::set_block_number(25);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle_until_cycle_nonce(actor_id, 1);
    assert_eq!(native_balance(&BOB), bob_before.saturating_add(50));
  });
}

#[test]
fn global_circuit_breaker_blocks_creation() {
  new_test_ext().execute_with(|| {
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 10)),
      ),
      Error::<Test>::GlobalCircuitBreakerActive
    );
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 10)),
      ),
      Error::<Test>::GlobalCircuitBreakerActive
    );
  });
}

#[test]
fn future_schedule_targets_accept_exact_boundary_and_reject_overflow_without_mutation() {
  new_test_ext().execute_with(|| {
    let max = u64::MAX;
    frame_system::Pallet::<Test>::set_block_number(max - 5_000);
    let exact_cooldown = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 5_000,
    };
    let actor_id = create_system_with(ALICE, exact_cooldown, None, inert_contract_steps());
    assert!(Actors::active_actor_view(actor_id).is_some());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });

  new_test_ext().execute_with(|| {
    let max = u64::MAX;
    frame_system::Pallet::<Test>::set_block_number(max - 4_999);
    let overflowing_cooldown = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 5_000,
    };
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(overflowing_cooldown, None, inert_contract_steps()),
      ),
      Error::<Test>::SchedulerIndexExhausted
    );
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });

  new_test_ext().execute_with(|| {
    let max = u64::MAX;
    frame_system::Pallet::<Test>::set_block_number(max - 4);
    let actor_id = create_system_with(ALICE, timer_schedule(4), None, inert_contract_steps());
    assert!(Actors::active_actor_view(actor_id).is_some());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });

  new_test_ext().execute_with(|| {
    let max = u64::MAX;
    frame_system::Pallet::<Test>::set_block_number(max - 3);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract(timer_schedule(4), None, inert_contract_steps()),
    ));
  });
}

#[test]
fn optional_bounded_dnf_is_canonical_and_mode_distinct() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(5);
    let observed = TestAsset::Local(99);
    set_asset_balance(&ALICE, observed, 100);
    let unconditional: Option<crate::PreconditionOf<Test>> = None;
    assert!(unconditional.is_none());

    let all = all_conditions(vec![
      Predicate::BalanceAbove {
        asset: observed,
        threshold: 50,
      },
      Predicate::BlockNumberBelow { threshold: 10 },
      Predicate::BalanceBelow {
        asset: observed,
        threshold: 200,
      },
      Predicate::BlockNumberAbove { threshold: 1 },
    ]);
    let any = any_conditions(vec![
      Predicate::BalanceAbove {
        asset: observed,
        threshold: 1_000,
      },
      Predicate::BlockNumberBelow { threshold: 1 },
      Predicate::BalanceBelow {
        asset: observed,
        threshold: 200,
      },
      Predicate::BlockNumberAbove { threshold: 10 },
    ]);
    assert_eq!(all.as_ref().expect("all precondition").predicate_count(), 4);
    assert_eq!(any.as_ref().expect("any precondition").predicate_count(), 4);
    assert_eq!(
      Actors::evaluate_precondition(all.as_ref().expect("all precondition"), &ALICE, 0),
      Ok(true)
    );
    assert_eq!(
      Actors::evaluate_precondition(any.as_ref().expect("any precondition"), &ALICE, 0),
      Ok(true)
    );
    assert_ne!(all.encode(), any.encode());

    let all_false = all_conditions(vec![
      Predicate::BalanceAbove {
        asset: observed,
        threshold: 1_000,
      },
      Predicate::BlockNumberAbove { threshold: 10 },
    ]);
    let any_false = any_conditions(vec![
      Predicate::BalanceAbove {
        asset: observed,
        threshold: 1_000,
      },
      Predicate::BlockNumberAbove { threshold: 10 },
    ]);
    assert_eq!(
      Actors::evaluate_precondition(all_false.as_ref().expect("all precondition"), &ALICE, 0),
      Ok(false)
    );
    assert_eq!(
      Actors::evaluate_precondition(any_false.as_ref().expect("any precondition"), &ALICE, 0),
      Ok(false)
    );
  });
}

#[test]
fn admission_canonicalizes_dnf_and_equivalent_update_is_exact_noop() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let first = TimedPredicate {
      timing: ObservationTiming::Current,
      predicate: Predicate::BlockNumberAbove { threshold: 0 },
    };
    let second = TimedPredicate {
      timing: ObservationTiming::Current,
      predicate: Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 0,
      },
    };
    let raw_precondition = || {
      Some(Precondition {
        clauses: BoundedVec::try_from(vec![
          BoundedVec::try_from(vec![second, first, second]).expect("predicates fit"),
        ])
        .expect("clause fits"),
      })
    };
    let raw_plan = || {
      contract_steps_with_step(StepOf::<Test> {
        precondition: raw_precondition(),
        task: Task::StopCycle,
        on_error: StepErrorPolicy::AbortCycle,
      })
    };
    let actor_id = create_system_with(ALICE, manual_schedule(), None, raw_plan());
    let stored = Actors::actor_contract(actor_id).expect("contract exists");
    assert_eq!(
      stored.steps[0]
        .precondition
        .as_ref()
        .expect("stored precondition")
        .predicate_count(),
      2
    );
    let clauses = &stored.steps[0]
      .precondition
      .as_ref()
      .expect("stored precondition")
      .clauses;
    assert!(clauses[0][0].encode() < clauses[0][1].encode());

    let event_count = frame_system::Pallet::<Test>::event_count();
    let control_block = ActorIdentities::<Test>::get(actor_id)
      .expect("identity exists")
      .last_control_mutation_block;
    assert_ok!(Actors::update_contract(
      RuntimeOrigin::root(),
      actor_id,
      ActorContract {
        steps: raw_plan(),
        ..stored.clone()
      },
    ));
    assert_eq!(frame_system::Pallet::<Test>::event_count(), event_count);
    assert_eq!(
      ActorIdentities::<Test>::get(actor_id)
        .expect("identity exists")
        .last_control_mutation_block,
      control_block
    );

    let duplicate_clauses = Some(Precondition {
      clauses: BoundedVec::try_from(vec![
        BoundedVec::try_from(vec![first, second]).expect("first clause fits"),
        BoundedVec::try_from(vec![second, first, second]).expect("second clause fits"),
      ])
      .expect("clauses fit"),
    });
    let duplicate_plan = contract_steps_with_step(StepOf::<Test> {
      precondition: duplicate_clauses,
      task: Task::StopCycle,
      on_error: StepErrorPolicy::AbortCycle,
    });
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(manual_schedule(), None, duplicate_plan),
      ),
      Error::<Test>::InvalidPredicate
    );
  });
}

#[test]
fn admission_absorbs_exact_dnf_superset_clause() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let first = TimedPredicate {
      timing: ObservationTiming::Current,
      predicate: Predicate::BlockNumberAbove { threshold: 0 },
    };
    let second = TimedPredicate {
      timing: ObservationTiming::Current,
      predicate: Predicate::BlockNumberBelow { threshold: 10 },
    };
    let absorbed = Some(Precondition {
      clauses: BoundedVec::try_from(vec![
        BoundedVec::try_from(vec![first]).expect("subset fits"),
        BoundedVec::try_from(vec![second, first]).expect("superset fits"),
      ])
      .expect("clauses fit"),
    });
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(StepOf::<Test> {
        precondition: absorbed,
        task: Task::StopCycle,
        on_error: StepErrorPolicy::AbortCycle,
      }),
    );
    let stored = Actors::actor_contract(actor_id).expect("contract exists");
    let clauses = &stored.steps[0]
      .precondition
      .as_ref()
      .expect("stored precondition")
      .clauses;
    assert_eq!(clauses.len(), 1);
    assert_eq!(clauses[0].as_slice(), &[first]);
  });
}

#[test]
fn above_and_below_conditions_are_strict_at_equality() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(5);
    let asset = TestAsset::Local(99);
    set_asset_balance(&ALICE, asset, 100);
    set_observation(
      1,
      crate::ScalarObservationState::Fresh {
        value: 50,
        observed_at: 5,
      },
    );
    let equality_boundaries = [
      all_conditions(vec![Predicate::BalanceAbove {
        asset,
        threshold: 100,
      }]),
      all_conditions(vec![Predicate::BalanceBelow {
        asset,
        threshold: 100,
      }]),
      all_conditions(vec![Predicate::BlockNumberAbove { threshold: 5 }]),
      all_conditions(vec![Predicate::BlockNumberBelow { threshold: 5 }]),
      all_conditions(vec![Predicate::ObservationAbove {
        feed: 1,
        threshold: 50,
        max_age_blocks: 10,
      }]),
      all_conditions(vec![Predicate::ObservationBelow {
        feed: 1,
        threshold: 50,
        max_age_blocks: 10,
      }]),
    ];
    for conditions in equality_boundaries {
      assert_eq!(
        Actors::evaluate_precondition(
          conditions.as_ref().expect("bounded precondition"),
          &ALICE,
          0
        ),
        Ok(false)
      );
    }
  });
}

#[test]
fn condition_block_number_above_skips_before_threshold() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(5);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BlockNumberAbove { threshold: 10 }]),
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
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepSkipped { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
  });
}

#[test]
fn condition_block_number_below_skips_after_threshold() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(20);
    let step = StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BlockNumberBelow { threshold: 10 }]),
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
    assert!(has_actor_event(|e| matches!(
      e,
      Event::StepSkipped { actor_id: id, step_index: 0, .. } if *id == actor_id
    )));
  });
}

#[test]
fn mint_on_unfunded_account_creates_tokens() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = contract_steps_with_step(make_step(Task::Mint {
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1000),
    }));
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    let actor = sovereign_account(actor_id);
    assert_eq!(native_balance(&actor), 0);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(native_balance(&actor), 1000);
  });
}

#[test]
fn timer_always_executes_on_interval() {
  new_test_ext().execute_with(|| {
    let schedule = timer_schedule(5);
    let contract_steps = transfer_contract_steps(BOB, 10);
    let actor_id = create_system_with(ALICE, schedule, None, contract_steps);
    fund_native(actor_id, 1000);
    // Track executions via cycle_nonce changes (each cycle increments nonce)
    let mut last_cycle_nonce = 0u64;
    let mut execution_count = 0usize;
    for block in 2..22 {
      frame_system::Pallet::<Test>::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::MAX);
      if let Some(inst) = Actors::active_actor_view(actor_id) {
        if inst.cycle_nonce > last_cycle_nonce {
          execution_count += 1;
          last_cycle_nonce = inst.cycle_nonce;
        }
      }
    }
    assert!(
      execution_count >= 2,
      "timer should execute every 5 blocks, got {} executions",
      execution_count
    );
  });
}

#[test]
fn timer_jitter_removal_evidence_is_machine_readable_and_decisive() {
  let evidence: serde_json::Value = serde_json::from_str(include_str!(
    "../../tests/fixtures/timer-jitter-decision.v1.json"
  ))
  .expect("timer-jitter decision fixture parses");
  assert_eq!(evidence["profile"]["actorIds"], "1..=10000");
  assert_eq!(evidence["zeroPhase"]["tailServiceBlock"], 7_429);
  assert_eq!(evidence["historicalJitter"]["tailServiceBlock"], 7_429);
  assert_eq!(evidence["zeroPhase"]["passExhaustedBlocks"], 6_666);
  assert_eq!(evidence["historicalJitter"]["passExhaustedBlocks"], 6_666);
  assert_eq!(evidence["decision"], "remove-timer-jitter");
}

#[test]
fn user_copybook_savings() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let savings: AccountId = 8888;
    let schedule = timer_schedule(10);
    // Transfer 5% of current native balance to savings
    let contract_steps = contract_steps_with_step(StepOf::<Test> {
      precondition: all_conditions(vec![Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 100,
      }]),
      task: Task::Transfer {
        to: savings,
        asset: TestAsset::Native,
        amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(5)),
      },
      on_error: StepErrorPolicy::AbortCycle,
    });
    let actor_id = create_user_with(ALICE, Mutability::Mutable, schedule, None, contract_steps);
    fund_native(actor_id, 10000);
    let initial_savings = native_balance(&savings);
    // Execute multiple cycles
    for block in 2..32 {
      frame_system::Pallet::<Test>::set_block_number(block);
      Actors::on_initialize(block);
      Actors::on_idle(block, Weight::MAX);
    }
    assert!(
      native_balance(&savings) > initial_savings,
      "Savings should accumulate"
    );
  });
}

#[test]
fn percentage_modes_excluding_total_supply_remain_supported() {
  new_test_ext().execute_with(|| {
    let asset = TestAsset::Local(42);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::Transfer {
        to: BOB,
        asset,
        amount: AmountResolution::PercentageOfCurrent(Perbill::from_percent(10)),
      })),
    );
    let sovereign = sovereign_account(actor_id);
    set_asset_balance(&sovereign, asset, 1_000);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);
    assert_eq!(asset_balance(&BOB, asset), 99);
    assert_eq!(asset_balance(&sovereign, asset), 901);
  });
}

#[test]
fn system_immutable_rejects_runtime_control_paths_even_for_root() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = inert_contract_steps();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Immutable,
      system_active_contract(timer_schedule(1), None, contract_steps.clone()),
    ));
    let actor_id = Actors::next_actor_id().saturating_sub(1);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .mutability,
      Mutability::Immutable
    );
    assert_noop!(
      update_contract_partial!(RuntimeOrigin::root(), actor_id, timer_schedule(2), None),
      Error::<Test>::ImmutableActor
    );
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::root(),
        actor_id,
        contract_steps.clone(),
        crate::CompletionPolicy::CloseAfterProductiveCycle,
      ),
      Error::<Test>::ImmutableActor
    );
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ImmutableActor
    );
    assert_noop!(
      Actors::resume_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ImmutableActor
    );
    assert_noop!(
      Actors::manual_trigger(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ImmutableActor
    );
    assert_noop!(
      Actors::close_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ImmutableActor
    );
    assert_noop!(
      Actors::create_system_actor_at_sovereign_id(
        RuntimeOrigin::root(),
        actor_id,
        ALICE,
        Mutability::Immutable,
        system_active_contract(manual_schedule(), None, inert_contract_steps()),
      ),
      Error::<Test>::SystemSovereignOccupied
    );
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert_eq!(
      Actors::system_sovereigns(actor_id),
      Some(SystemSovereignState::Occupied(actor_id))
    );
  });
}

#[test]
fn system_immutable_indefinite_commitment_survives_breaker_mitigation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Immutable,
      system_active_contract(timer_schedule(10), None, inert_contract_steps()),
    ));
    let actor_id = Actors::next_actor_id().saturating_sub(1);
    let before = Actors::active_actor_view(actor_id).expect("immutable actor");
    assert!(Actors::attempt_weight_upper_bound(&before, 0).ref_time() > 0);
    assert_eq!(
      Actors::system_sovereigns(actor_id),
      Some(SystemSovereignState::Occupied(actor_id)),
    );

    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    frame_system::Pallet::<Test>::set_block_number(100);
    run_idle(Weight::MAX);

    let after = Actors::active_actor_view(actor_id).expect("breaker cannot remove commitment");
    assert_eq!(after.sovereign_account, before.sovereign_account);
    assert_eq!(after.steps, before.steps);
    assert_eq!(after.cycle_nonce, 0);
    assert_eq!(
      Actors::system_sovereigns(actor_id),
      Some(SystemSovereignState::Occupied(actor_id)),
    );
    assert_noop!(
      Actors::close_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ImmutableActor
    );
  });
}

#[test]
fn fresh_current_plan_simulation_returns_runtime_trace_and_rolls_back_every_write() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = transfer_contract_steps(BOB, 10);
    let expected_contract = system_active_contract(manual_schedule(), None, contract_steps.clone())
      .expect("direct Actor Contract");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract_steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    let actor_before = Actors::active_actor_view(actor_id).expect("actor exists");
    let actor_balance_before = native_balance(&actor_before.sovereign_account);
    let bob_before = native_balance(&BOB);
    let event_count_before = frame_system::Pallet::<Test>::event_count();
    let result = Actors::simulate_current_contract(
      actor_id,
      ActorType::System,
      Mutability::Mutable,
      expected_contract,
      SimulationMode::FreshCurrentPlan,
    )
    .expect("ready current plan simulates");

    assert_eq!(result.status, AttemptDisposition::Completed);
    assert_eq!(result.cycle_nonce, 1);
    assert_eq!(result.start_cursor, 0);
    assert_eq!(result.continuation_cursor, None);
    assert_eq!(result.unsuccessful_attempts_at_cursor, None);
    assert_eq!(result.cumulative_outcomes.executed_steps, 1);
    assert_eq!(
      result.steps.as_slice(),
      &[SimulationStepRecord {
        step_index: 0,
        outcome: StepOutcome::Executed,
      }]
    );
    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(native_balance(&BOB), bob_before);
    assert_eq!(
      native_balance(
        &Actors::active_actor_view(actor_id)
          .expect("actor remains")
          .sovereign_account
      ),
      actor_balance_before
    );
    assert_eq!(
      frame_system::Pallet::<Test>::event_count(),
      event_count_before
    );
    assert!(Actors::continuation_state(actor_id).is_none());
  });
}
