use super::*;
use polkadot_sdk::sp_runtime::DispatchError;

#[test]
fn class_control_machine_flags_project_independently_without_writes() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    let mut resource = crate::BlockResourceState::new(1);
    resource.halt_optional_actor_work();
    crate::CurrentBlockResourceState::<Test>::put(resource);
    for (enabled, frozen) in [(false, false), (false, true), (true, true), (true, false)] {
      assert_ok!(Actors::set_user_creation_enabled(
        RuntimeOrigin::root(),
        enabled
      ));
      assert_ok!(Actors::set_global_circuit_breaker(
        RuntimeOrigin::root(),
        frozen
      ));
      let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      assert_eq!(
        Actors::machine_control_flags(),
        crate::ActorMachineControlFlags {
          user_creation_enabled: enabled,
          machine_frozen: frozen,
        }
      );
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        before
      );
      assert!(Actors::actor_hot(actor_id).unwrap().lifecycle.is_paused());
      assert!(
        Actors::block_resource_state()
          .unwrap()
          .optional_actor_work_halted()
      );
    }
  });
}

#[test]
fn frozen_ingress_refuses_active_consequences_temporarily_and_preserves_balance_only_cases() {
  for system in [false, true] {
    for schedule in [
      manual_schedule(),
      percentage_trigger_schedule(),
      timer_schedule(5),
    ] {
      new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let actor_id = if system {
          create_system_with(ALICE, schedule, None, inert_contract_steps())
        } else {
          create_user_with(
            ALICE,
            Mutability::Mutable,
            schedule,
            None,
            inert_contract_steps(),
          )
        };
        let dormant = Actors::next_actor_id();
        assert_ok!(Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          Mutability::Mutable,
          None
        ));
        let mut event = crate::AddressEvent {
          source: Some(BOB),
          destination: sovereign_account(actor_id),
          asset: TestAsset::Native,
          amount: 1,
          provenance: Some(crate::FundingProvenance::Signed),
        };
        assert_ok!(Actors::set_global_circuit_breaker(
          RuntimeOrigin::root(),
          true
        ));
        let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        for provenance in [
          Some(crate::FundingProvenance::Signed),
          Some(crate::FundingProvenance::InternalProtocol),
          Some(crate::FundingProvenance::Xcm),
          None,
        ] {
          event.provenance = provenance;
          for result in [
            Actors::preflight_ingress(&event),
            Actors::notify_ingress(&event),
          ] {
            assert_eq!(
              result,
              Err(crate::IngressFailure::temporary(
                Error::<Test>::GlobalCircuitBreakerActive
              ))
            );
          }
        }
        event.amount = 0;
        assert_ok!(Actors::preflight_ingress(&event));
        assert_ok!(Actors::notify_ingress(&event));
        event.amount = 1;
        event.source = Some(event.destination);
        assert_ok!(Actors::preflight_ingress(&event));
        assert_ok!(Actors::notify_ingress(&event));
        event.source = Some(BOB);
        for destination in [
          Actors::actor_identity(dormant).unwrap().sovereign_account,
          CHARLIE,
        ] {
          event.destination = destination;
          assert_ok!(Actors::preflight_ingress(&event));
          assert_ok!(Actors::notify_ingress(&event));
        }
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
          root
        );
      });
    }
  }
}

#[test]
fn frozen_ingress_commit_rechecks_stale_preflight_and_producer_rolls_back_movement() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      percentage_trigger_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_contract_steps(),
    );
    let event = crate::AddressEvent {
      source: Some(BOB),
      destination: sovereign_account(actor_id),
      asset: TestAsset::Native,
      amount: 100,
      provenance: Some(crate::FundingProvenance::Signed),
    };
    assert_ok!(Actors::preflight_ingress(&event));
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    System::set_block_number(102);
    let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_eq!(
      Actors::preflight_ingress(&event),
      Err(crate::IngressFailure::temporary(
        Error::<Test>::GlobalCircuitBreakerActive
      ))
    );
    let result = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_ok!(MockAssetOps::transfer(
        &BOB,
        &event.destination,
        event.asset,
        event.amount
      ));
      let result = Actors::notify_ingress(&event);
      match result {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    });
    assert_eq!(
      result,
      Err(crate::IngressFailure::temporary(
        Error::<Test>::GlobalCircuitBreakerActive
      ))
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root
    );
    assert!(
      Actors::active_actor_state(actor_id).is_some(),
      "expired Active identity cannot be destroyed by frozen ingress"
    );
  });
}

#[test]
fn frozen_balance_dependency_publication_rolls_back_its_producer_without_revision_or_scan() {
  new_test_ext().execute_with(|| {
    let asset = TestAsset::Native;
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::resolve_balance_dependency_source(asset),
      )
    })
    .unwrap();
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    let result = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_ok!(MockAssetOps::transfer(&ALICE, &BOB, asset, 10));
      let result =
        <Actors as crate::BalanceTransitionIngress<TestAsset>>::note_balance_transition(asset);
      match result {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    });
    assert_eq!(
      result,
      Err(Error::<Test>::GlobalCircuitBreakerActive.into())
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      let result = <Actors as crate::BalanceTransitionIngress<TestAsset>>::note_balance_transition(
        TestAsset::Local(9),
      );
      assert_ok!(result);
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
  });
}

#[test]
fn machine_freeze_prepass_retains_due_frontiers_and_thaw_waits_for_next_block() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_system_with_mutability(
      ALICE,
      Mutability::Mutable,
      timer_schedule(5),
      None,
      inert_contract_steps(),
    );
    let window_actor = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_contract_steps(),
    );
    let window_state = crate::ActorSemanticStates::<Test>::get(window_actor);
    let window_deadline = crate::DeadlineHandles::<Test>::get(window_actor);
    let semantic = crate::ActorSemanticStates::<Test>::get(actor_id);
    let process = crate::ActorProcesses::<Test>::get(actor_id);
    let tick = crate::TriggerDeadlineHandles::<Test>::get(actor_id);
    let deadline = crate::DeadlineHandles::<Test>::get(actor_id);
    let service = crate::ServiceHeader::<Test>::get();
    let dependency = crate::DependencyScanSourceListState::<Test>::get();
    let fees = fee_collections();
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    System::set_block_number(102);
    assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
    let state = Actors::block_resource_state().unwrap();
    assert_eq!(state.phase(), crate::BlockResourcePhase::ExternalPhase);
    assert!(state.optional_actor_work_halted());
    assert_eq!(state.usage().actor_effect_used(), Weight::zero());
    assert_eq!(crate::ActorSemanticStates::<Test>::get(actor_id), semantic);
    assert_eq!(crate::ActorProcesses::<Test>::get(actor_id), process);
    assert_eq!(crate::TriggerDeadlineHandles::<Test>::get(actor_id), tick);
    assert_eq!(crate::DeadlineHandles::<Test>::get(actor_id), deadline);
    assert_eq!(
      crate::ActorSemanticStates::<Test>::get(window_actor),
      window_state
    );
    assert_eq!(
      crate::DeadlineHandles::<Test>::get(window_actor),
      window_deadline
    );
    assert_eq!(crate::ServiceHeader::<Test>::get(), service);
    assert_eq!(
      crate::DependencyScanSourceListState::<Test>::get(),
      dependency
    );
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      false
    ));
    Actors::on_idle(102, Weight::MAX);
    assert_eq!(
      Actors::block_resource_state().unwrap().phase(),
      crate::BlockResourcePhase::Finalizable
    );
    assert_eq!(crate::ActorSemanticStates::<Test>::get(actor_id), semantic);
    assert_eq!(crate::ActorProcesses::<Test>::get(actor_id), process);
    assert_eq!(crate::TriggerDeadlineHandles::<Test>::get(actor_id), tick);
    assert_eq!(crate::DeadlineHandles::<Test>::get(actor_id), deadline);
    assert_eq!(
      crate::ActorSemanticStates::<Test>::get(window_actor),
      window_state
    );
    assert_eq!(
      crate::DeadlineHandles::<Test>::get(window_actor),
      window_deadline
    );
    assert_eq!(crate::ServiceHeader::<Test>::get(), service);
    assert_eq!(
      crate::DependencyScanSourceListState::<Test>::get(),
      dependency
    );
    assert_eq!(fee_collections(), fees);
    Actors::on_finalize(102);
    run_canonical_block_at(103, Weight::MAX);
    assert_eq!(
      active_eligibility(window_actor).terminal_reason,
      Some(CloseReason::WindowExpired)
    );
    assert_eq!(Actors::active_actor_view(actor_id).unwrap().cycle_nonce, 0);
    assert!(Actors::active_actor_view(actor_id).unwrap().pending_signal);
    // Frontier return retains the normal B+1 boundary; the expired window is not reset.
    run_canonical_block_at(104, Weight::MAX);
    assert!(
      Actors::active_actor_state(window_actor).is_none(),
      "original absolute window closes after thaw without extending it"
    );
    assert_eq!(Actors::active_actor_view(actor_id).unwrap().cycle_nonce, 1);
    #[cfg(feature = "try-runtime")]
    Actors::do_try_state().expect("retained due authority is reclaimed by its ordinary owner");
  });
}

#[test]
fn machine_freeze_after_prepass_preserves_committed_prefix_and_halts_drain_after_thaw() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    });
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      vec![step.clone(), step].try_into().unwrap(),
    );
    fund_native(actor_id, 1_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    System::set_block_number(2);
    assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
    let semantic = crate::ActorSemanticStates::<Test>::get(actor_id);
    let process = crate::ActorProcesses::<Test>::get(actor_id);
    let run = Actors::actor_run_state(actor_id).unwrap();
    assert_eq!(run.cursor, 1);
    let fees = fee_collections();
    let balance = native_balance(&BOB);
    let effects = Actors::block_resource_state()
      .unwrap()
      .usage()
      .actor_effect_used();
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      false
    ));
    assert!(
      Actors::block_resource_state()
        .unwrap()
        .optional_actor_work_halted()
    );
    Actors::on_idle(2, Weight::MAX);
    assert_eq!(crate::ActorSemanticStates::<Test>::get(actor_id), semantic);
    assert_eq!(crate::ActorProcesses::<Test>::get(actor_id), process);
    assert_eq!(Actors::actor_run_state(actor_id), Some(run));
    assert_eq!(
      Actors::block_resource_state()
        .unwrap()
        .usage()
        .actor_effect_used(),
      effects
    );
    assert_eq!(fee_collections(), fees);
    assert_eq!(native_balance(&BOB), balance);
    Actors::on_finalize(2);
    run_canonical_block_at(3, Weight::MAX);
    assert_eq!(native_balance(&BOB), balance + 10);
    assert_eq!(Actors::active_actor_view(actor_id).unwrap().cycle_nonce, 1);
    assert!(Actors::actor_run_state(actor_id).is_none());
    #[cfg(feature = "try-runtime")]
    Actors::do_try_state().expect("one committed Cycle continues once in the next block");
  });
}

#[test]
fn frozen_public_controls_reject_before_noop_expiry_payment_or_publication() {
  for system in [false, true] {
    for expired in [false, true] {
      new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let steps = inert_contract_steps();
        let window = Some(ScheduleWindow { start: 1, end: 101 });
        let actor_id = if system {
          create_system_with_mutability(
            ALICE,
            Mutability::Mutable,
            manual_schedule(),
            window,
            steps,
          )
        } else {
          create_user_with(ALICE, Mutability::Mutable, manual_schedule(), window, steps)
        };
        fund_native(actor_id, 1_000_000);
        let dormant = Actors::next_actor_id();
        assert_ok!(Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          Mutability::Mutable,
          None
        ));
        let contract = Actors::active_actor_state(actor_id).unwrap().contract;
        System::set_block_number(if expired { 102 } else { 2 });
        assert_ok!(Actors::set_global_circuit_breaker(
          RuntimeOrigin::root(),
          true
        ));
        let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        let fees = fee_collections();
        assert_noop!(
          Actors::update_contract(RuntimeOrigin::signed(ALICE), actor_id, contract.clone()),
          Error::<Test>::GlobalCircuitBreakerActive
        );
        assert_noop!(
          Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id),
          Error::<Test>::GlobalCircuitBreakerActive
        );
        assert_noop!(
          Actors::cancel_run(RuntimeOrigin::signed(ALICE), actor_id),
          Error::<Test>::GlobalCircuitBreakerActive
        );
        assert_noop!(
          Actors::deactivate_actor(RuntimeOrigin::signed(ALICE), actor_id),
          Error::<Test>::GlobalCircuitBreakerActive
        );
        assert_noop!(
          Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id),
          Error::<Test>::GlobalCircuitBreakerActive
        );
        assert_noop!(
          Actors::close_actor(RuntimeOrigin::signed(ALICE), dormant),
          Error::<Test>::GlobalCircuitBreakerActive
        );
        assert_noop!(
          Actors::activate_actor(RuntimeOrigin::signed(ALICE), dormant, contract),
          Error::<Test>::GlobalCircuitBreakerActive
        );
        assert_noop!(
          Actors::permissionless_sweep(RuntimeOrigin::signed(BOB), actor_id),
          Error::<Test>::GlobalCircuitBreakerActive
        );
        for ids in [vec![], vec![actor_id, dormant, u64::MAX]] {
          assert_noop!(
            Actors::permissionless_sweep_many(RuntimeOrigin::signed(BOB), ids.try_into().unwrap()),
            Error::<Test>::GlobalCircuitBreakerActive
          );
        }
        assert_noop!(
          Actors::set_active_actor_limit(
            RuntimeOrigin::root(),
            Actors::effective_active_actor_limit()
          ),
          Error::<Test>::GlobalCircuitBreakerActive
        );
        assert_noop!(
          Actors::set_active_actor_limit(RuntimeOrigin::root(), 0),
          Error::<Test>::GlobalCircuitBreakerActive
        );
        if system {
          assert_noop!(
            Actors::pause_actor(RuntimeOrigin::root(), actor_id),
            Error::<Test>::GlobalCircuitBreakerActive
          );
          assert_noop!(
            Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id),
            Error::<Test>::GlobalCircuitBreakerActive
          );
        }
        assert_eq!(fee_collections(), fees);
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
          before
        );
        assert_ok!(Actors::set_user_creation_enabled(
          RuntimeOrigin::root(),
          false
        ));
        assert_ok!(Actors::set_global_circuit_breaker(
          RuntimeOrigin::root(),
          false
        ));
        assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), dormant));
        #[cfg(feature = "try-runtime")]
        Actors::do_try_state().expect("freeze refusal preserves live and dormant authority");
      });
    }
  }
}

#[test]
fn user_creation_setter_has_privileged_authority_and_preserves_existing_actors() {
  for initial in [false, true] {
    for enabled in [false, true] {
      for frozen in [false, true] {
        new_test_ext().execute_with(|| {
          System::set_block_number(1);
          let actor_id = create_user_with(
            ALICE,
            Mutability::Mutable,
            manual_schedule(),
            None,
            transfer_contract_steps(BOB, 10),
          );
          fund_native(actor_id, 1_000_000);
          assert_ok!(Actors::manual_trigger(
            RuntimeOrigin::signed(ALICE),
            actor_id
          ));
          let before_actor = Actors::active_actor_state(actor_id).expect("existing paid readiness");
          let before_process = crate::ActorProcesses::<Test>::get(actor_id);
          let before_hold = crate::ActorStateHolds::<Test>::get(actor_id);
          let before_custody = native_balance(&sovereign_account(actor_id));
          let before_next = Actors::next_actor_id();
          let before_count = Actors::active_instance_count();
          let before_fees = fee_collections();
          crate::UserCreationEnabled::<Test>::put(initial);
          GlobalCircuitBreaker::<Test>::put(frozen);
          for origin in [
            RuntimeOrigin::signed(ALICE),
            RuntimeOrigin::signed(BOB),
            RuntimeOrigin::none(),
          ] {
            let root =
              polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
            assert_noop!(
              Actors::set_user_creation_enabled(origin, enabled),
              DispatchError::BadOrigin
            );
            assert_eq!(
              polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
              root
            );
          }
          assert_ok!(Actors::set_user_creation_enabled(
            RuntimeOrigin::root(),
            enabled
          ));
          assert_eq!(Actors::user_creation_enabled(), enabled);
          assert_eq!(Actors::global_circuit_breaker(), frozen);
          System::assert_last_event(Event::<Test>::UserCreationEnabledSet { enabled }.into());
          assert_eq!(Actors::active_actor_state(actor_id), Some(before_actor));
          assert_eq!(crate::ActorProcesses::<Test>::get(actor_id), before_process);
          assert_eq!(crate::ActorStateHolds::<Test>::get(actor_id), before_hold);
          assert_eq!(native_balance(&sovereign_account(actor_id)), before_custody);
          assert_eq!(Actors::next_actor_id(), before_next);
          assert_eq!(Actors::active_instance_count(), before_count);
          assert_eq!(fee_collections(), before_fees);
          #[cfg(feature = "try-runtime")]
          Actors::do_try_state().expect("setter changes no Actor partition or residence");
        });
      }
    }
  }
}

#[test]
fn system_temporal_pause_preserves_clocks_without_detector_membership() {
  for trigger in [Trigger::at_time(5), Trigger::cadenced(5)] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let actor_id = Actors::next_actor_id();
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(
          RuntimeSchedule {
            trigger: trigger.clone(),
            cooldown_blocks: 0
          },
          None,
          inert_contract_steps(),
        ),
      ));
      let actor = Actors::load_actor_ref(actor_id).unwrap();
      let mut identity = Actors::actor_identity(actor_id).unwrap();
      let mut hot = Actors::actor_hot(actor_id).unwrap();
      let contract = Actors::actor_contract(actor_id).unwrap();
      let hold = crate::ActorStateHolds::<Test>::get(actor_id);
      let custody = native_balance(&sovereign_account(actor_id));
      assert_eq!(
        crate::TriggerDeadlineHandles::<Test>::get(actor_id)
          .unwrap()
          .key,
        WakeupKey::Tick(6)
      );
      System::set_block_number(2);
      assert_ok!(Actors::pause_actor(RuntimeOrigin::root(), actor_id));
      identity.last_control_mutation_block = 2;
      hot.lifecycle = crate::ActiveLifecycle::Paused;
      hot.trigger_wakeup_pointer = None;
      assert_eq!(Actors::actor_identity(actor_id), Some(identity));
      assert_eq!(Actors::actor_hot(actor_id), Some(hot.clone()));
      assert_eq!(Actors::actor_contract(actor_id), Some(contract.clone()));
      assert_eq!(Actors::load_actor_ref(actor_id), Some(actor));
      assert_eq!(crate::ActorStateHolds::<Test>::get(actor_id), hold);
      assert_eq!(native_balance(&sovereign_account(actor_id)), custody);
      assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
        actor_id
      ));
      assert!(Actors::actor_run_state(actor_id).is_none());
      let process = crate::ActorProcesses::<Test>::get(actor_id).unwrap();
      assert!(matches!(process.status, crate::ProcessStatus::Disabled(_)));
      assert!(process.residence.is_none());
      #[cfg(feature = "try-runtime")]
      Actors::do_try_state().expect("paused temporal detector has no physical membership");
      System::set_block_number(3);
      assert_ok!(Actors::resume_actor(RuntimeOrigin::root(), actor_id));
      let resumed = Actors::actor_hot(actor_id).unwrap();
      assert_eq!(resumed.trigger_runtime_state, hot.trigger_runtime_state);
      assert_eq!(resumed.pending_signal, hot.pending_signal);
      assert_eq!(
        crate::TriggerDeadlineHandles::<Test>::get(actor_id)
          .unwrap()
          .key,
        WakeupKey::Tick(6)
      );
      assert_eq!(Actors::actor_contract(actor_id), Some(contract));
      assert_eq!(Actors::load_actor_ref(actor_id), Some(actor));
      assert_eq!(crate::ActorStateHolds::<Test>::get(actor_id), hold);
      assert_eq!(native_balance(&sovereign_account(actor_id)), custody);
      #[cfg(feature = "try-runtime")]
      Actors::do_try_state().expect("resume republishes the preserved temporal clock");
    });
  }
}

#[test]
fn user_pause_resume_reject_before_active_noop_or_expiry_substitution() {
  for mutability in [Mutability::Mutable, Mutability::Immutable] {
    for expired in [false, true] {
      for breaker in [false, true] {
        new_test_ext().execute_with(|| {
          System::set_block_number(1);
          let actor_id = create_user_with(
            ALICE,
            mutability,
            manual_schedule(),
            Some(ScheduleWindow { start: 1, end: 101 }),
            transfer_contract_steps(BOB, 10),
          );
          fund_native(actor_id, 1_000_000);
          System::set_block_number(if expired { 102 } else { 2 });
          GlobalCircuitBreaker::<Test>::put(breaker);
          if expired {
            assert_eq!(
              active_eligibility(actor_id).terminal_reason,
              Some(CloseReason::WindowExpired)
            );
          }
          let before =
            polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
          let actor_before = Actors::active_actor_state(actor_id).expect("live User authority");
          let hold_before = crate::ActorStateHolds::<Test>::get(actor_id);
          let custody_before = native_balance(&sovereign_account(actor_id));
          assert_noop!(
            Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id),
            DispatchError::BadOrigin
          );
          assert_noop!(
            Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id),
            DispatchError::BadOrigin
          );
          assert_noop!(
            Actors::pause_actor(RuntimeOrigin::signed(BOB), actor_id),
            Error::<Test>::NotOwner
          );
          assert_noop!(
            Actors::resume_actor(RuntimeOrigin::root(), actor_id),
            Error::<Test>::NotGovernance
          );
          assert_eq!(Actors::active_actor_state(actor_id), Some(actor_before));
          assert_eq!(crate::ActorStateHolds::<Test>::get(actor_id), hold_before);
          assert_eq!(native_balance(&sovereign_account(actor_id)), custody_before);
          assert_eq!(
            polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
            before
          );
        });
      }
    }
  }
}

#[test]
fn user_pause_rejection_preserves_running_and_suspended_progress() {
  for suspended in [false, true] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      setup_temporary_retry_pool();
      let steps = if suspended {
        temporary_retry_swap_plan()
      } else {
        let step = make_step(Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(10),
        });
        BoundedVec::try_from(vec![step.clone(), step]).expect("two economic Steps")
      };
      let actor_id = create_user_with(ALICE, Mutability::Mutable, manual_schedule(), None, steps);
      fund_native(actor_id, 1_000_000_000_000_000);
      set_temporary_dex_failure(suspended);
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      run_canonical_block_at(2, Weight::MAX);
      assert_eq!(
        Actors::actor_hot(actor_id)
          .expect("active User")
          .cycle_state,
        if suspended {
          CycleState::Suspended
        } else {
          CycleState::Running
        }
      );
      let progress = Actors::actor_run_state(actor_id).expect("committed Run");
      let actor = Actors::active_actor_state(actor_id).expect("canonical actor");
      let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      assert_noop!(
        Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id),
        DispatchError::BadOrigin
      );
      assert_noop!(
        Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id),
        DispatchError::BadOrigin
      );
      assert_eq!(Actors::actor_run_state(actor_id), Some(progress));
      assert_eq!(Actors::active_actor_state(actor_id), Some(actor));
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        before
      );
    });
  }
}
