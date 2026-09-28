use super::*;
use crate::{
  ActorProcesses, FundingProvenance, ProcessResidence, ServiceNodes, ServiceResidenceKind,
  TriggerCauseProvenance,
};

#[test]
fn terminal_service_reserves_cleanup_before_any_step_mutation() {
  use crate::WeightInfo;
  for actor_type in [ActorType::User, ActorType::System] {
    for (zero_step, reason) in [
      (false, CloseReason::AutoCloseNonceReached),
      (true, CloseReason::AutoCloseNonceReached),
      (false, CloseReason::ProductiveCycleCompleted),
      (false, CloseReason::RetryAttemptsExhausted),
      (false, CloseReason::ConsecutiveFailures),
    ] {
      for pass_owned in [false, true] {
        for deficit in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
          new_test_ext().execute_with(|| {
            System::set_block_number(1);
            let failure = matches!(
              reason,
              CloseReason::RetryAttemptsExhausted | CloseReason::ConsecutiveFailures
            );
            if reason == CloseReason::ConsecutiveFailures {
              set_max_consecutive_failures(1);
            }
            let mut step = if failure {
              setup_temporary_retry_pool();
              set_temporary_dex_failure(true);
              let mut step = temporary_retry_swap_plan()[0].clone();
              step.on_error = StepErrorPolicy::AbortCycle;
              step
            } else {
              make_step(Task::Transfer {
                to: BOB,
                asset: TestAsset::Native,
                amount: AmountResolution::Fixed(10),
              })
            };
            if reason == CloseReason::RetryAttemptsExhausted {
              step.on_error = StepErrorPolicy::RetryLater { max_attempts: 2 };
            }
            let steps = if zero_step {
              BoundedVec::default()
            } else {
              contract_steps_with_step(step)
            };
            let mut contract = system_active_contract(manual_schedule(), None, steps).unwrap();
            match reason {
              CloseReason::AutoCloseNonceReached => contract.auto_close_at_cycle_nonce = Some(1),
              CloseReason::ProductiveCycleCompleted => {
                contract.completion = crate::CompletionPolicy::CloseAfterProductiveCycle;
              }
              _ => {}
            }
            let id = Actors::next_actor_id();
            if actor_type == ActorType::User {
              prefund_active_user_creation(ALICE, &contract.steps);
              assert_ok!(Actors::create_user_actor(
                RuntimeOrigin::signed(ALICE),
                Mutability::Mutable,
                Some(contract)
              ));
            } else {
              assert_ok!(Actors::create_system_actor(
                RuntimeOrigin::root(),
                ALICE,
                Mutability::Mutable,
                Some(contract)
              ));
            }
            fund_native(id, 1_000);
            assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
            System::set_block_number(2);
            if reason == CloseReason::RetryAttemptsExhausted {
              let mut setup =
                polkadot_sdk::frame_support::weights::WeightMeter::with_limit(Weight::MAX);
              assert_ok!(Actors::service_canonical_round_head(&mut setup, 2));
              assert_eq!(
                Actors::actor_run_state(id)
                  .unwrap()
                  .unsuccessful_attempts_at_cursor,
                1
              );
              System::set_block_number(3);
            }
            let now = System::block_number();
            polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
              assert_ok!(Actors::begin_service_round(now));
              polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
            });
            let (step_control, effect) = if zero_step {
              (
                TestWeightInfo::scheduler_inner_zero_step_complete(),
                Weight::zero(),
              )
            } else {
              let resources = Actors::load_current_step_from_storage(id, 0)
                .unwrap()
                .resources;
              (resources.control, resources.effect)
            };
            let inspection = TestWeightInfo::service_round_begin_populated()
              .saturating_add(TestWeightInfo::service_round_probe_eligible())
              .saturating_add(TestWeightInfo::scheduler_actor_state_probe());
            let suffix = TestWeightInfo::service_round_admit_eligible()
              .max(TestWeightInfo::service_member_retire_interior())
              .max(TestWeightInfo::service_member_retire_pair_cursor())
              .max(TestWeightInfo::service_member_retire_singleton());
            let full_control = inspection
              .saturating_add(step_control)
              .saturating_add(suffix)
              .saturating_add(Actors::close_dispatch_weight_upper());
            let root =
              polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
            for short in [true, false] {
              let control = if short {
                full_control.saturating_sub(deficit)
              } else {
                full_control
              };
              let limits =
                crate::BlockResourceLimits::new(control, effect.saturating_mul(2), effect, effect)
                  .unwrap();
              let mut state = crate::BlockResourceState::new(now);
              assert_ok!(
                state.begin_prepass(
                  limits
                    .into_budget()
                    .expect("synthetic limits define one budget")
                )
              );
              let consumed = if pass_owned {
                Actors::execute_cycle_to_cutoff_with_resources(
                  full_control.saturating_add(effect),
                  0,
                  &mut state,
                  limits,
                  crate::BlockResourceDomain::ActorBaseEffect,
                  control,
                )
                .consumed
              } else {
                let mut meter = polkadot_sdk::frame_support::weights::WeightMeter::with_limit(
                  full_control.saturating_add(effect),
                );
                let result = Actors::service_canonical_round_head_with_resources(
                  &mut meter,
                  now,
                  &mut state,
                  limits,
                  crate::BlockResourceDomain::ActorBaseEffect,
                );
                assert_eq!(result.is_err(), short);
                meter.consumed()
              };
              assert_eq!(state.outstanding_reservations(), 0);
              assert!(!state.optional_actor_work_halted());
              if short {
                assert_eq!(
                  polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
                  root,
                  "{actor_type:?} {reason:?} zero={zero_step} caller={pass_owned} {deficit:?}"
                );
                assert_eq!(consumed, inspection);
                assert_eq!(state.usage().actor_effect_used(), Weight::zero());
              } else {
                assert_eq!(
                  state.usage().actor_control_used(),
                  full_control,
                  "{actor_type:?} {reason:?} zero={zero_step} caller={pass_owned}"
                );
                assert_eq!(state.usage().actor_effect_used(), effect);
                assert_eq!(consumed, full_control.saturating_add(effect));
                assert!(Actors::actor_identity(id).is_none());
                assert!(!ActorProcesses::<Test>::contains_key(id));
                assert!(!ServiceNodes::<Test>::contains_key(id));
                assert!(!crate::ActorRunStateStore::<Test>::contains_key(id));
                #[cfg(feature = "try-runtime")]
                assert_ok!(Actors::do_try_state());
                assert!(has_actor_event(|event| matches!(
                  event,
                  Event::ActorClosed { actor_id, reason: actual }
                    if *actor_id == id && *actual == reason
                )));
                if !zero_step {
                  assert_eq!(
                    last_step_control_execution().unwrap().placement,
                    crate::StepControlPlacement::None
                  );
                }
              }
            }
          });
        }
      }
    }
  }
}

#[test]
fn cancelled_run_returns_idle_without_a_deferred_manual_cycle() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    setup_temporary_retry_pool();
    let mut contract = user_active_contract(manual_schedule(), None, temporary_retry_swap_plan())
      .expect("active retry Contract");
    contract.auto_close_at_cycle_nonce = Some(1);
    prefund_active_user_creation(ALICE, &contract.steps);
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      Some(contract)
    ));
    fund_native(actor_id, 1_000_000_000_000_000);
    set_temporary_dex_failure(true);
    System::set_block_number(2);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_some());

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(!Actors::pending_signal(actor_id));
    assert_ok!(Actors::cancel_run(RuntimeOrigin::signed(ALICE), actor_id));

    let cancelled = Actors::active_actor_view(actor_id).expect("cancelled Actor remains active");
    assert_eq!(cancelled.cycle_nonce, 1);
    assert_eq!(cancelled.cycle_state, CycleState::Idle);
    assert!(!cancelled.pending_signal);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!crate::ActorUnsignaledControlCells::<Test>::contains_key(
      actor_id
    ));
    assert!(crate::ActorProcesses::<Test>::contains_key(actor_id));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActorClosed { actor_id: id, .. } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn user_pipeline_insolvency_closes_before_effect_capacity_deferral() {
  for solvent in [false, true] {
    for effect_capacity in [
      Weight::from_parts(0, u64::MAX / 2),
      Weight::from_parts(u64::MAX / 2, 0),
    ] {
      new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let actor_id = create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          transfer_contract_steps(BOB, 10),
        );
        fund_native(actor_id, 1_000_000_000_000_000);
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          actor_id
        ));
        let sovereign = sovereign_account(actor_id);
        if !solvent {
          let excess = native_balance(&sovereign)
            .checked_sub(TestMinUserBalance::get())
            .expect("funded Actor covers the protected floor");
          deplete_user_sovereign(actor_id, excess);
        }
        let balance_before = native_balance(&sovereign);
        let recipient_before = native_balance(&BOB);
        let actor_before = Actors::active_actor_view(actor_id).expect("paid readiness");
        let process_before = crate::ActorProcesses::<Test>::get(actor_id);
        // The canonical occurrence is published as Pending Service at B+1, so the direct pass must
        // observe the next block for the head to be selectable.
        System::set_block_number(2);
        let limits = crate::SimulationBudget {
          actor_control: Weight::from_parts(u64::MAX / 2, u64::MAX / 2),
          shared_economic: effect_capacity,
        }
        .checked_limits()
        .expect("independent component limits fit");
        let mut resources = crate::BlockResourceState::new(2);
        assert_ok!(
          resources.begin_prepass(
            limits
              .into_budget()
              .expect("synthetic limits define one budget")
          )
        );
        assert_ok!(resources.open_external_phase());
        assert_ok!(resources.begin_drain());
        let pass = Actors::execute_cycle_to_cutoff_with_resources(
          Weight::MAX,
          Actors::queue_tail(),
          &mut resources,
          limits,
          crate::BlockResourceDomain::ActorDrainEffect,
          limits.actor_control(),
        );
        assert_eq!(resources.outstanding_reservations(), 0);
        assert_eq!(resources.usage().actor_effect_used(), Weight::zero());
        assert_eq!(
          pass.reconciled_domains(),
          Some((pass.consumed, Weight::zero()))
        );
        assert_eq!(native_balance(&BOB), recipient_before);
        assert_eq!(native_balance(&sovereign), balance_before);
        assert!(!has_actor_event(|event| matches!(
          event, Event::CycleStarted { actor_id: id, .. } if *id == actor_id
        )));
        if solvent {
          assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
          // The deferred effectful attempt rolls the canonical resident back unchanged; the legacy
          // paged head is not part of canonical publication.
          assert_eq!(crate::ActorProcesses::<Test>::get(actor_id), process_before);
          Actors::execute_cycle_to_cutoff(Weight::MAX, Actors::queue_tail());
          assert_eq!(
            Actors::active_actor_view(actor_id).map(|actor| actor.cycle_nonce),
            Some(1)
          );
          assert_eq!(native_balance(&BOB), recipient_before.saturating_add(10));
        } else {
          assert!(Actors::active_actor_view(actor_id).is_none());
          assert!(has_actor_event(|event| matches!(
            event,
            Event::ActorClosed { actor_id: id, reason: CloseReason::CycleAdmissionInsufficient }
              if *id == actor_id
          )));
        }
        #[cfg(feature = "try-runtime")]
        assert_ok!(Actors::do_try_state());
      });
    }
  }
}

#[test]
fn canonical_weight_refusal_reads_no_actor_cold_state() {
  for actor_type in [ActorType::System, ActorType::User] {
    let mut ext = new_test_ext();
    let (
      actor_id,
      process_key,
      service_key,
      semantic_key,
      contract_key,
      payload_key,
      selector,
      envelope,
    ) = ext.execute_with(|| {
      System::set_block_number(1);
      let actor_id = match actor_type {
        ActorType::System => create_system_with(
          ALICE,
          manual_schedule(),
          None,
          transfer_contract_steps(BOB, 10),
        ),
        ActorType::User => create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          transfer_contract_steps(BOB, 10),
        ),
      };
      fund_native(actor_id, 1_000_000_000_000_000);
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      // The occurrence published at block 1 is served at B+1.
      System::set_block_number(2);
      let selector = <TestWeightInfo as crate::WeightInfo>::service_round_begin_populated()
        .saturating_add(<TestWeightInfo as crate::WeightInfo>::service_round_probe_eligible());
      let inspection = selector
        .saturating_add(<TestWeightInfo as crate::WeightInfo>::scheduler_actor_state_probe());
      (
        actor_id,
        crate::ActorProcesses::<Test>::hashed_key_for(actor_id),
        crate::ServiceNodes::<Test>::hashed_key_for(actor_id),
        crate::ActorSemanticStates::<Test>::hashed_key_for(actor_id),
        crate::ActorContractHeads::<Test>::hashed_key_for(actor_id),
        crate::ActorRunStateStore::<Test>::hashed_key_for(actor_id),
        selector,
        inspection,
      )
    });
    ext.commit_all().expect("commit fixture before recording");
    let before = ext.execute_with(|| polkadot_sdk::sp_io::storage::root(StateVersion::V1));
    ext.commit_all().expect("commit root calculation");
    for (scarce, discovered) in [
      (selector.saturating_sub(Weight::from_parts(1, 0)), false),
      (selector.saturating_sub(Weight::from_parts(0, 1)), false),
      (envelope.saturating_sub(Weight::from_parts(1, 0)), true),
      (envelope.saturating_sub(Weight::from_parts(0, 1)), true),
    ] {
      let recorder =
        polkadot_sdk::sp_trie::recorder::Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
      ext.execute_with_recorder(recorder.clone(), || {
        let mut refused = WeightMeter::with_limit(scarce);
        assert_eq!(
          Actors::service_canonical_round_head(&mut refused, 2),
          Err(if discovered {
            crate::ServiceRoundError::InsufficientWeight
          } else {
            crate::ServiceRoundError::DiscoveryUnavailable
          })
        );
        assert_eq!(
          refused.consumed(),
          if discovered { selector } else { Weight::zero() }
        );
      });
      let recorded = recorder.recorded_keys();
      for key in [&process_key, &service_key] {
        assert_eq!(
          recorded
            .values()
            .any(|keys| keys.keys().any(|read| read.as_ref() == key.as_slice())),
          discovered,
          "topology reads require discovery admission"
        );
      }
      for key in [&semantic_key, &contract_key, &payload_key] {
        assert!(
          !recorded
            .values()
            .any(|keys| keys.keys().any(|read| read.as_ref() == key.as_slice())),
          "sub-envelope Weight refusal read cold key {key:?}"
        );
      }
      ext.execute_with(|| {
        assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
      });
      ext.commit_all().expect("commit unchanged refusal");
    }
    // Positive control: an admitted round does read the ring and process authority.
    let recorder =
      polkadot_sdk::sp_trie::recorder::Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
    ext.execute_with_recorder(recorder.clone(), || {
      let mut admitted = WeightMeter::with_limit(Weight::MAX);
      assert!(Actors::service_canonical_round_head(&mut admitted, 2).is_ok());
    });
    let recorded = recorder.recorded_keys();
    for key in [&process_key, &service_key] {
      assert!(
        recorded
          .values()
          .any(|keys| keys.keys().any(|read| read.as_ref() == key.as_slice())),
        "admitted round did not read {key:?}"
      );
    }
    ext.execute_with(|| {
      assert_eq!(
        Actors::active_actor_view(actor_id).map(|actor| actor.cycle_nonce),
        Some(1)
      );
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn suspended_expiry_and_breaker_precede_liability_and_effect_deferral() {
  for actor_type in [ActorType::System, ActorType::User] {
    for paused in [false, true] {
      for breaker in [false, true] {
        for effect_capacity in [
          Weight::from_parts(0, u64::MAX / 2),
          Weight::from_parts(u64::MAX / 2, 0),
        ] {
          new_test_ext().execute_with(|| {
            System::set_block_number(1);
            setup_temporary_retry_pool();
            let schedule = Schedule { trigger: Trigger::manual(), cooldown_blocks: 1 };
            let window = Some(ScheduleWindow { start: 1, end: 101 });
            let actor_id = match actor_type {
              ActorType::System => create_system_with(ALICE, schedule, window, temporary_retry_swap_plan()),
              ActorType::User => create_user_with(ALICE, Mutability::Mutable, schedule, window, temporary_retry_swap_plan()),
            };
            fund_native(actor_id, 1_000_000_000_000_000);
            set_temporary_dex_failure(true);
            assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id));
            run_idle(Weight::MAX);
            let run = Actors::actor_run_state(actor_id).expect("ordinary retry");
            // The occurrence published at block 1 is served at B+1, so a cooldown of one block
            // makes the retry eligible at block 3.
            assert_eq!(run.eligible_at, 3);
            System::set_block_number(2);
            if paused { assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id)); }
            if breaker { assert_ok!(Actors::set_global_circuit_breaker(RuntimeOrigin::root(), true)); }
            let sovereign = sovereign_account(actor_id);
            if actor_type == ActorType::User {
              let excess = native_balance(&sovereign).checked_sub(TestMinUserBalance::get()).expect("funded fee floor");
              deplete_user_sovereign(actor_id, excess);
            }
            assert!(crate::ActorProcesses::<Test>::contains_key(actor_id));
            assert!(!ActorControlLocators::<Test>::contains_key(actor_id));
            let custody = native_balance(&sovereign);
            clear_fee_collections();
            System::reset_events();
            System::set_block_number(102);
            // Window expiry for a paused Actor is owned by the canonical temporal deadline carrier.
            // Returning it to Service in B=102 makes it first executable in B+1=103.
            service_canonical_temporal_frontiers(102);
            System::set_block_number(103);
            let limits = crate::SimulationBudget {
              actor_control: Weight::from_parts(u64::MAX / 2, u64::MAX / 2),
              shared_economic: effect_capacity,
            }.checked_limits().expect("independent lanes fit");
            let mut resources = crate::BlockResourceState::new(102);
            assert_ok!(resources.begin_prepass(limits.into_budget().expect("synthetic limits define one budget")));
            assert_ok!(resources.open_external_phase());
            assert_ok!(resources.begin_drain());
            let pass = Actors::execute_cycle_to_cutoff_with_resources(Weight::MAX, Actors::queue_tail(), &mut resources, limits,
              crate::BlockResourceDomain::ActorDrainEffect, limits.actor_control());
            assert_eq!(resources.outstanding_reservations(), 0);
            assert_eq!(resources.usage().actor_effect_used(), Weight::zero());
            assert_eq!(resources.usage().actor_control_used(), pass.consumed);
            assert_eq!(native_balance(&sovereign), custody);
            assert!(fee_collections().is_empty());
            if breaker {
              assert!(Actors::active_actor_view(actor_id).is_some());
              assert_eq!(Actors::actor_run_state(actor_id).expect("breaker preserves retry").encode(), run.encode());
              assert!(!has_actor_event(|event| matches!(event, Event::ActorClosed { actor_id: id, .. } if *id == actor_id)));
            } else {
              assert!(Actors::active_actor_view(actor_id).is_none());
              assert!(Actors::actor_run_state(actor_id).is_none());
              assert!(has_actor_event(|event| matches!(event, Event::ActorClosed { actor_id: id, reason: CloseReason::WindowExpired } if *id == actor_id)));
            }
            #[cfg(feature = "try-runtime")]
            assert_ok!(Actors::do_try_state());
          });
        }
      }
    }
  }
}

#[test]
fn next_work_plan_types_unsignaled_process_authority_without_writes() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let state = Actors::active_actor_state(actor_id).expect("real suspended Actor");
    let actor = Actors::load_actor_ref(actor_id).expect("active generation-bound reference");
    let resources = fixture_step_resource_envelope(actor_id);
    let run_state = state.run_state.clone();
    let state = Actors::detach_actor_publication(actor, state, run_state.as_ref())
      .expect("detach canonical publication for pure process planning");
    let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);

    let mut paused = state.clone();
    paused.hot.lifecycle = crate::ActiveLifecycle::Paused;
    paused.contract.window = None;
    let expected_not_before = paused
      .run_state
      .as_ref()
      .expect("suspended Run authority")
      .eligible_at;
    assert_eq!(
      Actors::test_plan_next_work_source(&paused, paused.run_state.as_ref(), 0),
      Ok((
        crate::StepControlPlacement::None,
        None,
        Some(crate::ProcessDisablement {
          cause: crate::ProcessDisableCause::OwnerPaused,
          revival_authority: crate::ProcessRevivalAuthority::Owner,
          basis: crate::SuspendedProcessBasis::Suspended {
            not_before: expected_not_before,
          },
        }),
        None,
      ))
    );

    let mut unlatched = state.clone();
    unlatched.hot.lifecycle = crate::ActiveLifecycle::Active;
    unlatched.hot.cycle_state = crate::CycleState::Idle;
    unlatched.hot.pending_signal = false;
    unlatched.contract.window = None;
    assert_eq!(
      Actors::test_plan_next_work_source(&unlatched, None, 0),
      Ok((
        crate::StepControlPlacement::None,
        None,
        Some(crate::ProcessDisablement {
          cause: crate::ProcessDisableCause::Protocol,
          revival_authority: crate::ProcessRevivalAuthority::Protocol,
          basis: crate::SuspendedProcessBasis::Idle,
        }),
        None,
      ))
    );
    let (disabled, admission_round, deadline) =
      Actors::test_plan_process_destination(actor, &unlatched, None, 0)
        .expect("unlatched Actor has a complete disabled destination");
    assert_eq!(disabled.generation, actor.generation);
    assert!(matches!(
      disabled.status,
      crate::ProcessStatus::Disabled(crate::ProcessDisablement {
        cause: crate::ProcessDisableCause::Protocol,
        ..
      })
    ));
    assert_eq!(
      (disabled.residence, admission_round, deadline),
      (None, None, None)
    );

    let mut pending = unlatched;
    pending.hot.pending_signal = true;
    assert_eq!(
      Actors::test_plan_next_work_source(&pending, None, System::block_number()),
      Ok((
        crate::StepControlPlacement::Queue,
        None,
        None,
        Some(crate::ServiceResidenceKind::Pending),
      ))
    );
    let (service, admission_round, deadline) =
      Actors::test_plan_process_destination(actor, &pending, None, System::block_number())
        .expect("pending Actor has a complete Service destination");
    assert_eq!(
      (service.generation, service.status, service.residence),
      (
        actor.generation,
        crate::ProcessStatus::Serving,
        Some(crate::ProcessResidence::Service(
          crate::ServiceResidenceKind::Pending,
        )),
      )
    );
    assert_eq!(
      (admission_round, deadline),
      (Some(System::block_number()), None)
    );

    let (sleeping, admission_round, deadline) =
      Actors::test_plan_process_destination(actor, &state, state.run_state.as_ref(), 0)
        .expect("suspended Actor has a complete Deadline destination");
    let deadline = deadline.expect("exact Deadline handle");
    assert_eq!(admission_round, None);
    assert_eq!(deadline.actor, actor);
    assert_eq!(
      sleeping.residence,
      Some(crate::ProcessResidence::Deadline {
        key: deadline.key,
        page: deadline.page,
        slot: deadline.slot,
      })
    );

    let mut temporal = state.clone();
    temporal.contract.trigger = Trigger::Cadenced { every_ticks: 5 };
    temporal.hot.trigger_runtime_state = TriggerRuntimeState::Cadenced {
      anchor_tick: Some(0),
    };
    temporal.hot.trigger_wakeup_pointer = None;
    let (planned_hot, planned_process, _, process_deadline, trigger_deadline) =
      Actors::test_plan_actor_publication(
        actor,
        &temporal,
        temporal.run_state.as_ref(),
        resources,
        0,
      )
      .expect("process residence and temporal Trigger plan together");
    let process_deadline = process_deadline.expect("suspended process owns Block deadline");
    let trigger_deadline = trigger_deadline.expect("Cadenced Trigger owns Tick deadline");
    assert!(matches!(process_deadline.key, WakeupKey::Block(_)));
    assert!(matches!(trigger_deadline.key, WakeupKey::Tick(_)));
    assert_eq!(process_deadline.actor, trigger_deadline.actor);
    assert_eq!(
      planned_process.residence,
      Some(crate::ProcessResidence::Deadline {
        key: process_deadline.key,
        page: process_deadline.page,
        slot: process_deadline.slot,
      })
    );
    assert_eq!(
      planned_hot.trigger_wakeup_pointer,
      Some(crate::TriggerWakeupPointer {
        tick: match trigger_deadline.key {
          WakeupKey::Tick(tick) => tick,
          WakeupKey::Block(_) => unreachable!("Trigger deadline uses Tick clock"),
        },
        page_id: trigger_deadline.page,
        slot: u32::from(trigger_deadline.slot),
      })
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      before,
      "typed process and destination planning must remain storage-free"
    );

    crate::DeadlineHandles::<Test>::insert(actor_id, process_deadline);
    crate::TriggerDeadlineHandles::<Test>::insert(actor_id, trigger_deadline);
    assert_eq!(
      crate::DeadlineHandles::<Test>::get(actor_id),
      Some(process_deadline)
    );
    assert_eq!(
      crate::TriggerDeadlineHandles::<Test>::get(actor_id),
      Some(trigger_deadline),
      "independent reverse owners must preserve both simultaneous memberships"
    );
  });
}

#[test]
fn composite_publication_preflight_rejects_stale_resources_and_partial_canonical_authority() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let state = Actors::active_actor_state(actor_id).expect("real suspended Actor");
    let resources = fixture_step_resource_envelope(actor_id);
    let actor = crate::ActorRef {
      actor_id,
      generation: crate::ActorSemanticStates::<Test>::get(actor_id)
        .and_then(|semantic| match semantic {
          crate::ActorSemanticState::Active(record) => Some(record.generation),
          crate::ActorSemanticState::Dormant(_) => None,
        })
        .expect("active generation"),
    };
    // Canonical creation publishes the Actor immediately, so the pre-publication preflight is
    // reached by detaching that exact publication back to its semantic-only source.
    let state = Actors::detach_actor_publication(actor, state.clone(), state.run_state.as_ref())
      .expect("canonical publication detaches to its pre-publication source");

    assert_eq!(
      Actors::test_preflight_actor_publication(
        actor,
        &state,
        state.run_state.as_ref(),
        resources,
        0,
      ),
      Ok(())
    );
    let mut stale_resources = resources;
    stale_resources.effect = stale_resources
      .effect
      .saturating_add(Weight::from_parts(1, 0));
    assert_eq!(
      Actors::test_preflight_actor_publication(
        actor,
        &state,
        state.run_state.as_ref(),
        stale_resources,
        0,
      ),
      Err(crate::scheduler::EnqueueOutcome::CorruptedTopology)
    );

    crate::ActorProcesses::<Test>::insert(
      actor_id,
      crate::ActorProcess {
        generation: actor.generation,
        last_attempted: None,
        status: crate::ProcessStatus::Disabled(crate::ProcessDisablement {
          cause: crate::ProcessDisableCause::Protocol,
          revival_authority: crate::ProcessRevivalAuthority::Protocol,
          basis: crate::SuspendedProcessBasis::Idle,
        }),
        residence: None,
      },
    );
    assert_eq!(
      Actors::test_preflight_actor_publication(
        actor,
        &state,
        state.run_state.as_ref(),
        resources,
        0,
      ),
      Err(crate::scheduler::EnqueueOutcome::CorruptedTopology),
      "a partial canonical publication must fail before the transaction boundary"
    );
  });
}

#[test]
fn composite_publication_rehomes_an_existing_temporal_trigger_pointer() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let state = Actors::active_actor_state(actor_id).expect("real suspended Actor");
    let resources = fixture_step_resource_envelope(actor_id);
    let actor = Actors::load_actor_ref(actor_id).expect("active generation-bound reference");
    // Detach the canonical publication so the rehomed Trigger pointer is the only stale carrier.
    let run_state = state.run_state.clone();
    let mut state = Actors::detach_actor_publication(actor, state, run_state.as_ref())
      .expect("canonical publication detaches");
    state.hot.trigger_wakeup_pointer = Some(crate::TriggerWakeupPointer {
      tick: 9,
      page_id: 77,
      slot: 3,
    });
    crate::ActorSemanticStates::<Test>::mutate(actor_id, |semantic| {
      let Some(crate::ActorSemanticState::Active(record)) = semantic else {
        panic!("active semantic record");
      };
      record.hot = state.hot.clone();
    });

    Actors::test_publish_actor_publication(actor, &state, state.run_state.as_ref(), resources, 0)
      .expect("existing temporal authority is rehomed");

    let handle = crate::TriggerDeadlineHandles::<Test>::get(actor_id)
      .expect("canonical Trigger deadline owns the migrated tick");
    assert_eq!(handle.key, WakeupKey::Tick(9));
    assert_eq!(
      crate::ActorSemanticStates::<Test>::get(actor_id).and_then(|semantic| match semantic {
        crate::ActorSemanticState::Active(record) => record.hot.trigger_wakeup_pointer,
        crate::ActorSemanticState::Dormant(_) => None,
      }),
      Some(crate::TriggerWakeupPointer {
        tick: 9,
        page_id: handle.page,
        slot: u32::from(handle.slot),
      }),
      "semantic Trigger authority must point only at the new canonical residence"
    );
  });
}

#[test]
fn temporal_actors_share_one_deadline_heap_key_until_last_close() {
  for close_first_created in [true, false] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let before = crate::DeadlineIndexLen::<Test>::get(WakeupClock::Tick);
      let first = create_system_with(
        BOB,
        timer_schedule(5),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      let first_handle = crate::TriggerDeadlineHandles::<Test>::get(first)
        .expect("first temporal Actor owns a canonical deadline");
      let key = first_handle.key;
      let index = crate::DeadlineIndexPositions::<Test>::get(key).expect("bucket is indexed");
      assert_eq!(
        crate::DeadlineIndexLen::<Test>::get(WakeupClock::Tick),
        before + 1
      );
      let second = create_system_with(
        CHARLIE,
        timer_schedule(5),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      );
      let second_handle = crate::TriggerDeadlineHandles::<Test>::get(second)
        .expect("second temporal Actor shares the deadline bucket");
      assert_eq!(second_handle.key, key);
      assert_eq!(second_handle.page, first_handle.page);
      assert_ne!(second_handle.slot, first_handle.slot);
      assert_eq!(crate::DeadlineHeaders::<Test>::get(key).unwrap().count, 2);
      assert_eq!(
        crate::DeadlineIndexLen::<Test>::get(WakeupClock::Tick),
        before + 1
      );
      assert_eq!(crate::DeadlineIndexPositions::<Test>::get(key), Some(index));
      let (closed, survivor) = if close_first_created {
        (first_handle, second_handle)
      } else {
        (second_handle, first_handle)
      };
      assert_ok!(Actors::close_actor(
        RuntimeOrigin::root(),
        closed.actor.actor_id
      ));
      assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
        closed.actor.actor_id
      ));
      assert!(!ActorProcesses::<Test>::contains_key(closed.actor.actor_id));
      assert_eq!(crate::DeadlineHeaders::<Test>::get(key).unwrap().count, 1);
      assert_eq!(
        crate::TriggerDeadlineHandles::<Test>::get(survivor.actor.actor_id),
        Some(survivor)
      );
      let page = crate::DeadlinePages::<Test>::get(key, survivor.page).unwrap();
      assert_eq!(page.entries[usize::from(closed.slot)], None);
      assert_eq!(
        page.entries[usize::from(survivor.slot)],
        Some(survivor.actor)
      );
      assert_eq!(
        crate::DeadlineIndexLen::<Test>::get(WakeupClock::Tick),
        before + 1
      );
      assert_eq!(crate::DeadlineIndexPositions::<Test>::get(key), Some(index));
      assert_eq!(
        crate::DeadlineIndexPages::<Test>::get(WakeupClock::Tick, u64::from(index / 32)).unwrap()
          [(index % 32) as usize],
        key,
      );
      #[cfg(feature = "try-runtime")]
      Actors::do_try_state().expect("surviving generation retains complete deadline authority");
      assert_ok!(Actors::close_actor(
        RuntimeOrigin::root(),
        survivor.actor.actor_id
      ));
      assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
        survivor.actor.actor_id
      ));
      assert!(!ActorProcesses::<Test>::contains_key(
        survivor.actor.actor_id
      ));
      assert!(!crate::DeadlineHeaders::<Test>::contains_key(key));
      assert!(!crate::DeadlineIndexPositions::<Test>::contains_key(key));
      assert!(
        crate::DeadlinePages::<Test>::iter_prefix(key)
          .next()
          .is_none()
      );
      assert_eq!(
        crate::DeadlineIndexLen::<Test>::get(WakeupClock::Tick),
        before
      );
      assert!(!crate::ActorWaitingCursorIndices::<Test>::contains_key(key));
      assert!(!crate::ActorWaitingOccupancies::<Test>::contains_key(key));
      assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick), 0);
      #[cfg(feature = "try-runtime")]
      Actors::do_try_state().expect("last close releases the shared deadline key");
    });
  }
}

#[test]
fn canonical_cadenced_deadline_publishes_pending_service() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      timer_schedule(1),
      None,
      inert_contract_steps(),
    );
    let actor = Actors::load_actor_ref(actor_id).expect("active generation-bound reference");
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert_eq!(
      crate::TriggerDeadlineHandles::<Test>::get(actor_id).map(|handle| handle.key),
      Some(WakeupKey::Tick(2))
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    let pass = Actors::service_due_deadline_frontiers(
      &mut meter,
      crate::ServiceResidenceKind::Live,
      2,
      2,
      Some(WakeupKey::Block(3)),
    )
    .expect("canonical deadline frontiers are admitted");
    assert_eq!(
      pass.tick,
      Ok(crate::DueTickDeadlineMutation::TemporalTriggerProcessed(
        actor
      ))
    );
    assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
      actor_id
    ));
    assert!(matches!(
      crate::ActorProcesses::<Test>::get(actor_id),
      Some(crate::ActorProcess {
        status: crate::ProcessStatus::Serving,
        residence: Some(crate::ProcessResidence::Service(
          crate::ServiceResidenceKind::Pending
        )),
        ..
      })
    ));
    let hot = crate::ActorSemanticStates::<Test>::get(actor_id)
      .and_then(|semantic| match semantic {
        crate::ActorSemanticState::Active(record) => Some(record.hot),
        crate::ActorSemanticState::Dormant(_) => None,
      })
      .expect("Cadenced semantic state survives");
    assert!(hot.pending_signal);
    assert!(hot.trigger_wakeup_pointer.is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed { actor_id: id, .. } if *id == actor_id
    )));
  });
}

#[test]
fn temporal_deadline_dispatch_preserves_source_on_resource_refusal() {
  for schedule in [at_time_schedule(1), timer_schedule(1)] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let fee = if matches!(schedule.trigger, Trigger::AtTime { .. }) {
        at_time_trigger_fee()
      } else {
        cadenced_trigger_fee()
      };
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        schedule,
        None,
        inert_contract_steps(),
      );
      fund_native(actor_id, 1_000_000_000_000_000);
      let source = crate::TriggerDeadlineHandles::<Test>::get(actor_id).unwrap();
      let payer = sovereign_account(actor_id);
      System::set_block_number(2);
      clear_fee_collections();
      let balance = native_balance(&payer);
      let selector = <TestWeightInfo as crate::WeightInfo>::classify_due_tick_deadline();
      let branch = <TestWeightInfo as crate::WeightInfo>::at_time_trigger_occurrence()
        .max(<TestWeightInfo as crate::WeightInfo>::cadenced_trigger_occurrence());
      let complete = selector.saturating_add(branch);
      let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      for (limit, consumed) in [
        (
          Weight::from_parts(selector.ref_time() - 1, u64::MAX),
          Weight::zero(),
        ),
        (
          Weight::from_parts(u64::MAX, selector.proof_size() - 1),
          Weight::zero(),
        ),
        (
          Weight::from_parts(complete.ref_time() - 1, u64::MAX),
          selector,
        ),
        (
          Weight::from_parts(u64::MAX, complete.proof_size() - 1),
          selector,
        ),
      ] {
        let mut meter = WeightMeter::with_limit(limit);
        assert_eq!(
          Actors::process_next_due_tick_deadline(&mut meter, 2),
          Err(crate::DependencyReviewWorkerError::InsufficientWeight)
        );
        assert_eq!(meter.consumed(), consumed);
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
          root
        );
        assert_eq!(
          crate::TriggerDeadlineHandles::<Test>::get(actor_id),
          Some(source)
        );
        assert_eq!(native_balance(&payer), balance);
        assert!(fee_collections().is_empty());
      }
      // This proves the declared admission boundary, not sufficiency of retained coefficients.
      let mut meter = WeightMeter::with_limit(complete);
      assert_eq!(
        Actors::process_next_due_tick_deadline(&mut meter, 2),
        Ok(crate::DueTickDeadlineMutation::TemporalTriggerProcessed(
          source.actor
        ))
      );
      assert_eq!(meter.consumed(), complete);
      assert_eq!(fee_collections(), vec![fee]);
      assert_eq!(native_balance(&payer), balance - fee);
      assert!(Actors::pending_signal(actor_id));
      assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
        actor_id
      ));
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn temporal_deadline_transaction_restores_source_after_loading_refusal() {
  for schedule in [at_time_schedule(1), timer_schedule(1)] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        schedule,
        None,
        inert_contract_steps(),
      );
      let source = crate::TriggerDeadlineHandles::<Test>::get(actor_id).unwrap();
      let payer = sovereign_account(actor_id);
      System::set_block_number(2);
      clear_fee_collections();
      let balance = native_balance(&payer);
      let head = crate::ActorContractHeads::<Test>::take(actor_id).unwrap();
      let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      let mut meter = WeightMeter::with_limit(Weight::MAX);
      assert_eq!(
        Actors::process_next_due_tick_deadline(&mut meter, 2),
        Err(crate::DependencyReviewWorkerError::TemporalOccurrence)
      );
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        root,
        "failed authority loading restores earlier source removal and semantic writes"
      );
      assert_eq!(
        crate::TriggerDeadlineHandles::<Test>::get(actor_id),
        Some(source)
      );
      assert_eq!(native_balance(&payer), balance);
      assert!(fee_collections().is_empty());
      crate::ActorContractHeads::<Test>::insert(actor_id, head);
      assert_eq!(
        Actors::process_due_temporal_deadline(source.actor, 2),
        Ok(crate::DueTickDeadlineMutation::TemporalTriggerProcessed(
          source.actor
        ))
      );
      assert!(Actors::pending_signal(actor_id));
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn composite_publication_commits_process_and_temporal_trigger_deadlines_atomically() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let mut state = Actors::active_actor_state(actor_id).expect("real suspended Actor");
    let resources = fixture_step_resource_envelope(actor_id);
    let actor = crate::ActorRef {
      actor_id,
      generation: crate::ActorSemanticStates::<Test>::get(actor_id)
        .and_then(|semantic| match semantic {
          crate::ActorSemanticState::Active(record) => Some(record.generation),
          crate::ActorSemanticState::Dormant(_) => None,
        })
        .expect("active generation"),
    };
    state.contract.trigger = Trigger::Cadenced { every_ticks: 5 };
    state.hot.trigger_runtime_state = TriggerRuntimeState::Cadenced {
      anchor_tick: Some(0),
    };
    state.hot.trigger_wakeup_pointer = None;
    let admission = Actors::build_admission_certificate(&state.contract)
      .expect("temporal Contract remains admissible");
    let semantic = crate::ActorSemanticStates::<Test>::get(actor_id)
      .and_then(|semantic| match semantic {
        crate::ActorSemanticState::Active(mut record) => {
          record.hot = state.hot.clone();
          record.admission = admission;
          Some(record)
        }
        crate::ActorSemanticState::Dormant(_) => None,
      })
      .expect("active semantic record");
    crate::ActorSemanticStates::<Test>::insert(
      actor_id,
      crate::ActorSemanticState::Active(semantic),
    );
    // Re-derive the pre-publication source so the Cadenced successor owns both deadlines.
    let run_state = state.run_state.clone();
    let state = Actors::detach_actor_publication(actor, state, run_state.as_ref())
      .expect("canonical publication detaches");

    Actors::test_publish_actor_publication(actor, &state, state.run_state.as_ref(), resources, 0)
      .expect("complete canonical publication commits");

    let process_handle =
      crate::DeadlineHandles::<Test>::get(actor_id).expect("suspended process owns Block deadline");
    let trigger_handle = crate::TriggerDeadlineHandles::<Test>::get(actor_id)
      .expect("Cadenced Trigger owns independent Tick deadline");
    assert!(matches!(process_handle.key, WakeupKey::Block(_)));
    assert!(matches!(trigger_handle.key, WakeupKey::Tick(_)));
    assert_eq!(process_handle.actor, trigger_handle.actor);
    let hot = crate::ActorSemanticStates::<Test>::get(actor_id)
      .and_then(|semantic| match semantic {
        crate::ActorSemanticState::Active(record) => Some(record.hot),
        crate::ActorSemanticState::Dormant(_) => None,
      })
      .expect("semantic Hot remains active");
    assert_eq!(
      hot.trigger_wakeup_pointer,
      Some(crate::TriggerWakeupPointer {
        tick: match trigger_handle.key {
          WakeupKey::Tick(tick) => tick,
          WakeupKey::Block(_) => unreachable!("Trigger deadline uses Tick clock"),
        },
        page_id: trigger_handle.page,
        slot: u32::from(trigger_handle.slot),
      })
    );
  });
}

#[test]
fn temporal_trigger_deadline_removal_is_independent_and_transactional() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let mut state = Actors::active_actor_state(actor_id).expect("real suspended Actor");
    let resources = fixture_step_resource_envelope(actor_id);
    let actor = Actors::load_actor_ref(actor_id).expect("active generation-bound reference");
    state.contract.trigger = Trigger::Cadenced { every_ticks: 5 };
    state.hot.trigger_runtime_state = TriggerRuntimeState::Cadenced {
      anchor_tick: Some(0),
    };
    state.hot.trigger_wakeup_pointer = None;
    let admission = Actors::build_admission_certificate(&state.contract)
      .expect("temporal Contract remains admissible");
    crate::ActorSemanticStates::<Test>::mutate(actor_id, |semantic| {
      let Some(crate::ActorSemanticState::Active(record)) = semantic else {
        panic!("active semantic record");
      };
      record.hot = state.hot.clone();
      record.admission = admission;
    });
    let run_state = state.run_state.clone();
    let state = Actors::detach_actor_publication(actor, state, run_state.as_ref())
      .expect("canonical publication detaches");
    Actors::test_publish_actor_publication(actor, &state, state.run_state.as_ref(), resources, 0)
      .expect("complete canonical publication commits");

    let process_before = crate::ActorProcesses::<Test>::get(actor_id)
      .expect("suspended process remains independently resident");
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    let refused: Result<(), crate::DeadlineMutationError> =
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        Actors::remove_trigger_deadline_member(actor).expect("exact Trigger member removes");
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(
          crate::DeadlineMutationError::InvalidDestination,
        ))
      });
    assert_eq!(
      refused,
      Err(crate::DeadlineMutationError::InvalidDestination)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before,
      "a later caller refusal must restore the complete Trigger carrier root"
    );

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::remove_trigger_deadline_member(actor).expect("exact Trigger member removes");
      crate::ActorSemanticStates::<Test>::mutate(actor_id, |semantic| {
        let Some(crate::ActorSemanticState::Active(record)) = semantic else {
          panic!("active semantic record");
        };
        record.hot.trigger_wakeup_pointer = None;
      });
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
      actor_id
    ));
    assert_eq!(
      crate::ActorProcesses::<Test>::get(actor_id),
      Some(process_before)
    );
    assert!(crate::DeadlineHandles::<Test>::contains_key(actor_id));
  });
}

#[test]
fn canonical_occurrence_preserves_busy_process_residence() {
  new_test_ext().execute_with(|| {
    let actor_id = create_canonical_suspended_system_retry(1);
    let state = Actors::active_actor_state(actor_id).expect("real suspended Actor");
    let actor = Actors::load_actor_ref(actor_id).expect("active generation-bound reference");
    let sovereign = sovereign_account(actor_id);
    assert!(matches!(state.contract.trigger, Trigger::Manual));
    let process_before = crate::ActorProcesses::<Test>::get(actor_id)
      .expect("busy process has one canonical residence");
    let process_handle_before = crate::DeadlineHandles::<Test>::get(actor_id);

    assert_eq!(
      Actors::commit_canonical_trigger_occurrence_with_authority(
        actor,
        ActorType::System,
        &sovereign,
        crate::TriggerFeeBreakdown {
          trigger_family: TriggerFamily::Manual,
          trigger_fee: 0,
        },
        state,
        System::block_number(),
      ),
      Ok(crate::scheduler::ActivationOutcome::Latched)
    );
    assert_eq!(
      crate::ActorProcesses::<Test>::get(actor_id),
      Some(process_before)
    );
    assert_eq!(
      crate::DeadlineHandles::<Test>::get(actor_id),
      process_handle_before
    );
    assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
      actor_id
    ));
    assert!(matches!(
      crate::ActorSemanticStates::<Test>::get(actor_id),
      Some(crate::ActorSemanticState::Active(record))
        if record.hot.pending_signal && record.hot.trigger_wakeup_pointer.is_none()
    ));
  });
}

#[test]
fn canonical_lifecycle_transition_atomically_pauses_and_resumes_service_authority() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let mut state = Actors::active_actor_state(actor_id).expect("real suspended Actor");
    let resources = fixture_step_resource_envelope(actor_id);
    let actor = Actors::load_actor_ref(actor_id).expect("active generation-bound reference");
    state.contract.trigger = Trigger::Cadenced { every_ticks: 5 };
    state.hot.trigger_runtime_state = TriggerRuntimeState::Cadenced {
      anchor_tick: Some(0),
    };
    state.hot.trigger_wakeup_pointer = None;
    let admission = Actors::build_admission_certificate(&state.contract)
      .expect("temporal Contract remains admissible");
    crate::ActorSemanticStates::<Test>::mutate(actor_id, |semantic| {
      let Some(crate::ActorSemanticState::Active(record)) = semantic else {
        panic!("active semantic record");
      };
      record.hot = state.hot.clone();
      record.admission = admission;
    });
    let run_state = state.run_state.clone();
    let state = Actors::detach_actor_publication(actor, state, run_state.as_ref())
      .expect("canonical publication detaches");
    Actors::test_publish_actor_publication(actor, &state, state.run_state.as_ref(), resources, 0)
      .expect("complete canonical publication commits");

    let mut source = state.clone();
    source.hot = match crate::ActorSemanticStates::<Test>::get(actor_id) {
      Some(crate::ActorSemanticState::Active(record)) => record.hot,
      _ => panic!("canonical semantic source remains active"),
    };
    let mut successor = source.clone();
    successor.hot.lifecycle = crate::ActiveLifecycle::Paused;
    successor.hot.trigger_wakeup_pointer = None;
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    let invalid_resources = crate::ActorStepResourceEnvelope {
      control: polkadot_sdk::sp_weights::Weight::zero(),
      effect: polkadot_sdk::sp_weights::Weight::zero(),
    };
    assert_eq!(
      Actors::test_transition_actor_publication_to_successor(
        actor,
        &source,
        &successor,
        source.run_state.as_ref(),
        invalid_resources,
        1,
      ),
      Err(crate::scheduler::EnqueueOutcome::CorruptedTopology),
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before,
      "late publication refusal restores process and Trigger carriers",
    );

    Actors::test_transition_actor_publication_to_successor(
      actor,
      &source,
      &successor,
      source.run_state.as_ref(),
      resources,
      1,
    )
    .expect("canonical pause transition commits");
    assert!(!crate::DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
      actor_id
    ));
    assert!(!crate::ServiceNodes::<Test>::contains_key(actor_id));
    assert!(matches!(
      crate::ActorProcesses::<Test>::get(actor_id),
      Some(crate::ActorProcess {
        status: crate::ProcessStatus::Disabled(_),
        residence: None,
        ..
      })
    ));

    let paused = successor;
    let mut resumed = paused.clone();
    resumed.hot.lifecycle = crate::ActiveLifecycle::Active;
    resumed.hot.pending_signal = true;
    Actors::test_transition_actor_publication_to_successor(
      actor,
      &paused,
      &resumed,
      resumed.run_state.as_ref(),
      resources,
      2,
    )
    .expect("canonical resume and latched occurrence publish one Service residence");
    assert!(!crate::DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(crate::TriggerDeadlineHandles::<Test>::contains_key(
      actor_id
    ));
    assert!(crate::ServiceNodes::<Test>::contains_key(actor_id));
    assert!(matches!(
      crate::ActorProcesses::<Test>::get(actor_id),
      Some(crate::ActorProcess {
        status: crate::ProcessStatus::Serving,
        residence: Some(crate::ProcessResidence::Service(_)),
        ..
      })
    ));
    assert!(matches!(
      crate::ActorSemanticStates::<Test>::get(actor_id),
      Some(crate::ActorSemanticState::Active(record))
        if record.hot.lifecycle == crate::ActiveLifecycle::Active
          && record.hot.pending_signal
    ));
  });
}

#[test]
fn canonical_terminal_removal_closes_service_authority_transactionally() {
  new_test_ext().execute_with(|| {
    let actor_id = create_system_with(ALICE, manual_schedule(), None, BoundedVec::default());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let state = Actors::active_actor_state(actor_id).expect("real latched Actor");
    let resources = fixture_step_resource_envelope(actor_id);
    let actor = Actors::load_actor_ref(actor_id).expect("active generation-bound reference");
    let run_state = state.run_state.clone();
    let mut state = Actors::detach_actor_publication(actor, state, run_state.as_ref())
      .expect("canonical publication detaches");
    Actors::test_publish_actor_publication(actor, &state, state.run_state.as_ref(), resources, 1)
      .expect("complete canonical publication commits");
    state.hot = match crate::ActorSemanticStates::<Test>::get(actor_id) {
      Some(crate::ActorSemanticState::Active(record)) => record.hot,
      _ => panic!("canonical semantic source remains active"),
    };

    let active_count = crate::ActiveActorCount::<Test>::get();
    crate::ActiveActorCount::<Test>::put(0);
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_noop!(
      Actors::test_remove_actor_publication_and_finalize(
        actor,
        state.clone(),
        state.run_state.as_ref(),
        CloseReason::OwnerInitiated,
      ),
      crate::Error::<Test>::ActiveActorCountInvariant
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before,
      "late terminal refusal restores semantic, process, and both deadline owners",
    );
    crate::ActiveActorCount::<Test>::put(active_count);

    Actors::test_remove_actor_publication_and_finalize(
      actor,
      state.clone(),
      state.run_state.as_ref(),
      CloseReason::OwnerInitiated,
    )
    .expect("canonical terminal removal converges on ordinary finalization");
    assert!(!crate::ActorSemanticStates::<Test>::contains_key(actor_id));
    assert!(!crate::ActorProcesses::<Test>::contains_key(actor_id));
    assert!(!crate::DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
      actor_id
    ));
    assert!(!crate::SovereignIndex::<Test>::contains_key(
      state.identity.sovereign_account
    ));
  });
}

#[test]
fn composite_publication_rolls_back_every_owner_after_carrier_mutation_failures() {
  let corrupted = crate::scheduler::EnqueueOutcome::CorruptedTopology;

  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let mut state = Actors::active_actor_state(actor_id).expect("real suspended Actor");
    let resources = fixture_step_resource_envelope(actor_id);
    let actor = Actors::load_actor_ref(actor_id).expect("active generation-bound reference");
    state.hot.lifecycle = crate::ActiveLifecycle::Active;
    state.hot.cycle_state = crate::CycleState::Idle;
    state.hot.pending_signal = true;
    state.run_state = None;
    crate::ActorRunStateStore::<Test>::remove(actor_id);
    crate::ActorSemanticStates::<Test>::mutate(actor_id, |semantic| {
      let Some(crate::ActorSemanticState::Active(record)) = semantic else {
        panic!("active semantic record");
      };
      record.hot = state.hot.clone();
    });
    let run_state = state.run_state.clone();
    let state = Actors::detach_actor_publication(actor, state, run_state.as_ref())
      .expect("canonical publication detaches");
    crate::ServiceHeader::<Test>::mutate(|header| {
      header.count = 0;
      header.cursor = Some(actor);
    });
    let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);

    assert_eq!(
      Actors::test_publish_actor_publication(actor, &state, None, resources, 2),
      Err(corrupted),
      "malformed Service topology fails after semantic and process publication"
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      before,
      "Service insertion failure must restore semantic, process, ring, and legacy state"
    );
  });

  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let state = Actors::active_actor_state(actor_id).expect("real suspended Actor");
    let resources = fixture_step_resource_envelope(actor_id);
    let actor = Actors::load_actor_ref(actor_id).expect("active generation-bound reference");
    let (_, _, process_deadline) =
      Actors::test_plan_process_destination(actor, &state, state.run_state.as_ref(), 0)
        .expect("suspended process destination");
    let process_key = process_deadline.expect("process deadline handle").key;
    let run_state = state.run_state.clone();
    let state = Actors::detach_actor_publication(actor, state, run_state.as_ref())
      .expect("canonical publication detaches");
    crate::DeadlineIndexPositions::<Test>::insert(process_key, 0);
    let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);

    assert_eq!(
      Actors::test_publish_actor_publication(
        actor,
        &state,
        state.run_state.as_ref(),
        resources,
        0,
      ),
      Err(corrupted),
      "stale process Deadline index fails after page and reverse-handle insertion"
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      before,
      "process Deadline index failure must restore every publication owner"
    );
  });

  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let mut state = Actors::active_actor_state(actor_id).expect("real suspended Actor");
    let resources = fixture_step_resource_envelope(actor_id);
    let actor = Actors::load_actor_ref(actor_id).expect("active generation-bound reference");
    state.contract.trigger = Trigger::Cadenced { every_ticks: 5 };
    state.hot.trigger_runtime_state = TriggerRuntimeState::Cadenced {
      anchor_tick: Some(0),
    };
    state.hot.trigger_wakeup_pointer = None;
    let admission = Actors::build_admission_certificate(&state.contract)
      .expect("temporal Contract remains admissible");
    crate::ActorSemanticStates::<Test>::mutate(actor_id, |semantic| {
      let Some(crate::ActorSemanticState::Active(record)) = semantic else {
        panic!("active semantic record");
      };
      record.hot = state.hot.clone();
      record.admission = admission;
    });
    let run_state = state.run_state.clone();
    let state = Actors::detach_actor_publication(actor, state, run_state.as_ref())
      .expect("canonical publication detaches");
    let (_, _, _, _, trigger_deadline) = Actors::test_plan_actor_publication(
      actor,
      &state,
      state.run_state.as_ref(),
      resources,
      0,
    )
    .expect("composite temporal destination");
    let trigger_key = trigger_deadline
      .expect("temporal Trigger deadline handle")
      .key;
    crate::DeadlineIndexPositions::<Test>::insert(trigger_key, 0);
    let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);

    assert_eq!(
      Actors::test_publish_actor_publication(
        actor,
        &state,
        state.run_state.as_ref(),
        resources,
        0,
      ),
      Err(corrupted),
      "stale Trigger Deadline index fails after process Deadline publication"
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      before,
      "Trigger Deadline failure must also roll back the earlier process carrier"
    );
  });
}

#[test]
fn supplied_run_is_the_only_consumed_scheduling_authority() {
  for suspended in [false, true] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = if suspended {
        create_suspended_system_retry(1)
      } else {
        let steps = BoundedVec::try_from(vec![
          make_step(Task::Transfer {
            to: BOB,
            asset: TestAsset::Native,
            amount: AmountResolution::Fixed(1),
          }),
          make_step(Task::StopCycle),
        ])
        .expect("two steps fit");
        let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
        fund_native(actor_id, 10);
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          actor_id
        ));
        run_idle(Weight::MAX);
        actor_id
      };
      let state = Actors::active_actor_state(actor_id).expect("real active Run");
      assert_eq!(
        state.hot.cycle_state,
        if suspended {
          CycleState::Suspended
        } else {
          CycleState::Running
        }
      );
      let run = state
        .run_state
        .as_ref()
        .expect("real lifecycle Run")
        .clone();
      let (_, admission, _) =
        Actors::load_frame_actor_service_state(actor_id).expect("canonical service authority");
      let resources = fixture_step_resource_envelope(actor_id);
      let invoke = |supplied| {
        Actors::test_schedule_next_work_source(
          actor_id,
          &state,
          &admission,
          resources,
          supplied,
          run.eligible_at,
        )
      };
      let assert_read_only = |supplied, expected| {
        let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        assert_eq!(invoke(supplied), expected);
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
          before
        );
      };
      let queued = Ok((crate::StepControlPlacement::Queue, vec![actor_id]));
      let corrupt = Err(crate::scheduler::EnqueueOutcome::CorruptedTopology);
      assert_read_only(None, queued.clone());
      assert_read_only(Some(None), corrupt.clone());
      let mut stale = run.clone();
      if suspended {
        stale.eligible_at += 1;
      } else {
        stale.suspension = Some(crate::SuspensionReason::Temporary);
      }
      assert_read_only(Some(Some(&stale)), corrupt.clone());
      crate::ActorRunStateStore::<Test>::insert(actor_id, stale.clone());
      assert_read_only(Some(Some(&run)), queued.clone());
      assert_read_only(None, corrupt.clone());
      crate::ActorRunStateStore::<Test>::remove(actor_id);
      assert_read_only(Some(Some(&run)), queued);
      assert_read_only(None, corrupt);
    });
  }
}

#[test]
fn empty_on_idle_settles_housekeeping_into_actor_control() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let _ = Actors::on_initialize(1);
    run_prepass();
    let before = Actors::block_resource_state().expect("prepass opens resource state");
    let consumed = Actors::on_idle(1, Weight::MAX);
    let after = Actors::block_resource_state().expect("idle settlement retains resource state");
    assert_ne!(consumed, Weight::zero());
    assert_eq!(after.outstanding_reservations(), 0);
    assert_eq!(after.phase(), crate::BlockResourcePhase::Finalizable);
    let settled_housekeeping = after
      .usage()
      .actor_control_used()
      .saturating_sub(before.usage().actor_control_used());
    assert_ne!(settled_housekeeping, Weight::zero());
    assert!(settled_housekeeping.all_lte(consumed));
    assert_eq!(after.usage().actor_effect_used(), Weight::zero());
    assert_eq!(
      Actors::finalized_block_resource_telemetry().map(|snapshot| snapshot.block_number()),
      Some(1)
    );
    Actors::on_finalize(1);
  });
}

#[test]
fn optional_work_halt_preserves_mandatory_idle_finalization() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    run_prepass();
    let mut state = Actors::block_resource_state().expect("prepass opens resource state");
    state.halt_optional_actor_work();
    let usage_before = state.usage();
    crate::CurrentBlockResourceState::<Test>::put(state);

    let consumed = Actors::on_idle(1, Weight::MAX);
    let finalized = Actors::block_resource_state().expect("halted block still finalizes");
    let mandatory = <TestWeightInfo as crate::WeightInfo>::scheduler_on_idle_base()
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::block_resource_finalize());
    assert_eq!(consumed, mandatory);
    assert_eq!(finalized.phase(), crate::BlockResourcePhase::Finalizable);
    assert!(finalized.optional_actor_work_halted());
    assert_eq!(finalized.outstanding_reservations(), 0);
    assert_eq!(
      finalized.usage().actor_control_used(),
      usage_before.actor_control_used().saturating_add(mandatory)
    );
    assert_eq!(
      finalized.usage().actor_effect_used(),
      usage_before.actor_effect_used()
    );
    assert_eq!(
      Actors::finalized_block_resource_telemetry().map(|snapshot| (
        snapshot.block_number(),
        snapshot.optional_actor_work_halted()
      )),
      Some((1, true))
    );
    Actors::on_finalize(1);
  });
}

#[test]
fn payload_free_actor_prepass_inherent_is_required_and_canonical() {
  use polkadot_sdk::{frame_support::inherent::ProvideInherent, sp_inherents::InherentData};

  let missing = InherentData::new();
  assert!(<Actors as ProvideInherent>::create_inherent(&missing).is_none());
  assert!(
    <Actors as ProvideInherent>::is_inherent_required(&missing)
      .expect("required check is deterministic")
      .is_some()
  );

  let mut present = InherentData::new();
  crate::provide_actor_prepass_inherent_data(&mut present).expect("empty prepass data encodes");
  let call = <Actors as ProvideInherent>::create_inherent(&present)
    .expect("canonical empty data creates prepass");
  assert!(<Actors as ProvideInherent>::is_inherent(&call));
  assert!(<Actors as ProvideInherent>::check_inherent(&call, &present).is_ok());
  assert!(<Actors as ProvideInherent>::check_inherent(&call, &missing).is_err());

  let mut unsupported = InherentData::new();
  unsupported
    .put_data(
      <Actors as ProvideInherent>::INHERENT_IDENTIFIER,
      &crate::ActorPrepassInherentData {
        version: crate::ACTOR_PREPASS_INHERENT_VERSION.saturating_add(1),
      },
    )
    .expect("unsupported fixture encodes");
  assert!(<Actors as ProvideInherent>::create_inherent(&unsupported).is_none());
  assert!(<Actors as ProvideInherent>::check_inherent(&call, &unsupported).is_err());
}

#[test]
fn actor_prepass_declaration_covers_quiet_allocations_without_changing_with_fixed_work() {
  use polkadot_sdk::frame_support::dispatch::{DispatchClass, GetDispatchInfo};

  new_test_ext().execute_with(|| {
    let maximum = Weight::from_parts(1200, 2400);
    let quiet = crate::BlockResourceBudget::new_with_control_ratio(maximum, Weight::zero(), 1, 3)
      .expect("bounded zero-fixed allocation");
    let quiet_prepass = quiet
      .limits()
      .actor_control()
      .saturating_add(quiet.limits().actor_base_turn());
    let mut declaration = None;
    for fixed in [
      maximum / 4,
      Weight::zero(),
      Weight::from_parts(300, 0),
      Weight::from_parts(0, 600),
    ] {
      set_block_resource_budget(
        crate::BlockResourceBudget::new_with_control_ratio(maximum, fixed, 1, 3)
          .expect("fixed work fits the system quarter"),
      );
      let info = crate::Call::<Test>::actor_prepass {}.get_dispatch_info();
      assert_eq!(info.class, DispatchClass::Mandatory);
      assert!(quiet_prepass.all_lte(info.call_weight));
      assert!(info.call_weight.all_lte(maximum));
      if let Some(previous) = declaration {
        assert_eq!(info.call_weight, previous);
      }
      declaration = Some(info.call_weight);
      assert!(Actors::block_resource_state().is_none());
    }
  });
}

#[test]
fn prepass_freezes_host_settled_budget_once_across_all_phases() {
  use polkadot_sdk::frame_support::dispatch::GetDispatchInfo;
  let maximum = Weight::from_parts(2_000_000_000_000, 10_485_760);
  let maximum_fixed = maximum / 4;
  let configured =
    crate::BlockResourceBudget::new_with_control_ratio(maximum, maximum_fixed, 1, 3).unwrap();
  for (prefix, tail) in [
    (Weight::zero(), Weight::zero()),
    (maximum / 16, maximum / 16),
    (maximum / 8, maximum / 8),
    (
      Weight::from_parts(maximum.ref_time() / 8, 0),
      Weight::from_parts(0, maximum.proof_size() / 8),
    ),
  ] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      set_block_resource_budget(configured);
      let steps =
        BoundedVec::try_from(vec![transfer_contract_steps(BOB, 1)[0].clone(); 2]).unwrap();
      let ids = (0..2)
        .map(|_| {
          let id = create_system_with(ALICE, manual_schedule(), None, steps.clone());
          fund_native(id, 1_000);
          assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
          id
        })
        .collect::<Vec<_>>();
      let supplied =
        crate::BlockResourceBudget::from_settled_prefix(maximum, maximum_fixed, prefix, tail, 1, 3)
          .unwrap();
      set_prepass_resource_budget(2, Ok(supplied));
      assert_eq!(
        crate::Call::<Test>::actor_prepass {}
          .get_dispatch_info()
          .call_weight,
        maximum
      );
      assert_eq!(TestBlockResourceBudget::get(), configured);
      assert_eq!(
        prepass_resource_budget_reads(),
        0,
        "configuration and declaration do not consume block input"
      );
      System::set_block_number(2);
      Actors::on_initialize(2);
      run_prepass();
      assert_eq!(prepass_resource_budget_reads(), 1);
      let mut state = Actors::block_resource_state().unwrap();
      assert_eq!(state.budget(), Ok(supplied));
      assert_eq!(supplied.fixed_envelope(), prefix.saturating_add(tail));
      assert!(
        supplied
          .limits()
          .actor_control()
          .all_gte(configured.limits().actor_control())
      );
      assert!(
        supplied
          .limits()
          .shared_economic()
          .all_gte(configured.limits().shared_economic())
      );
      for id in &ids {
        assert_eq!(Actors::actor_run_state(*id).unwrap().cursor, 1);
      }
      let user_actual = Weight::from_parts(1_000, 1_000);
      let mut reservation = state
        .reserve(
          supplied.limits(),
          crate::BlockResourceDomain::UserDispatch,
          user_actual,
        )
        .unwrap();
      assert_ok!(state.settle(&mut reservation, user_actual));
      let before = state;
      for deficit in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
        let overflow = supplied
          .limits()
          .shared_economic()
          .checked_sub(&state.usage().actor_effect_used())
          .unwrap()
          .checked_sub(&user_actual)
          .unwrap()
          .saturating_add(deficit);
        assert_eq!(
          state.reserve(
            supplied.limits(),
            crate::BlockResourceDomain::UserDispatch,
            overflow
          ),
          Err(crate::BlockResourceError::LimitExceeded)
        );
        assert_eq!(state, before);
      }
      crate::CurrentBlockResourceState::<Test>::put(state);
      set_prepass_resource_budget(2, Err(crate::BlockResourceError::InvalidPhase));
      set_block_resource_budget(crate::BlockResourceBudget::fail_closed(Weight::MAX));
      assert!(Actors::on_idle(2, Weight::MAX).all_gt(Weight::zero()));
      assert_eq!(
        prepass_resource_budget_reads(),
        1,
        "Drain cannot reread or replace frozen input"
      );
      let snapshot = Actors::finalized_block_resource_telemetry().unwrap();
      assert_eq!(snapshot.budget(), supplied);
      assert_eq!(snapshot.usage().user_dispatch_used(), user_actual);
      assert!(!snapshot.optional_actor_work_halted());
      Actors::on_finalize(2);
      assert!(Actors::block_resource_state().is_none());
      assert_eq!(prepass_resource_budget_reads(), 1);
      for id in &ids {
        assert_eq!(
          Actors::actor_run_state(*id).unwrap().cursor,
          1,
          "Drain preserves Q1"
        );
      }

      set_block_resource_budget(configured);
      set_prepass_resource_budget(3, Ok(configured));
      System::set_block_number(3);
      Actors::on_initialize(3);
      run_prepass();
      assert_eq!(prepass_resource_budget_reads(), 2);
      assert_eq!(
        Actors::block_resource_state().unwrap().budget(),
        Ok(configured)
      );
      Actors::on_idle(3, Weight::MAX);
      Actors::on_finalize(3);
      for id in ids {
        assert_eq!(Actors::active_actor_view(id).unwrap().cycle_nonce, 1);
        assert!(Actors::actor_run_state(id).is_none());
      }
    });
  }
}

#[test]
fn prepass_rejects_host_budget_errors_before_scheduler_mutation() {
  let maximum = Weight::from_parts(2_000_000_000_000, 10_485_760);
  let maximum_fixed = maximum / 4;
  let configured =
    crate::BlockResourceBudget::new_with_control_ratio(maximum, maximum_fixed, 1, 3).unwrap();
  let mut cases = vec![
    (2, Err(crate::BlockResourceError::InvalidPhase)),
    (1, Ok(configured)),
  ];
  for deficit in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
    for wrong_maximum in [
      maximum.saturating_add(deficit),
      maximum.saturating_sub(deficit),
    ] {
      cases.push((
        2,
        crate::BlockResourceBudget::new_with_control_ratio(wrong_maximum, maximum_fixed, 1, 3),
      ));
    }
    cases.push((
      2,
      crate::BlockResourceBudget::new_with_control_ratio(
        maximum,
        maximum_fixed.saturating_add(deficit),
        1,
        3,
      ),
    ));
    cases.push((
      2,
      crate::BlockResourceBudget::from_settled_prefix(
        maximum,
        maximum_fixed,
        maximum_fixed,
        deficit,
        1,
        3,
      ),
    ));
  }
  for (block, candidate) in cases {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      set_block_resource_budget(configured);
      let id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      fund_native(id, 1_000);
      assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
      System::set_block_number(2);
      set_prepass_resource_budget(block, candidate);
      assert_noop!(
        Actors::actor_prepass(RuntimeOrigin::none()),
        Error::<Test>::ResourceProtocolFailed
      );
      assert_eq!(prepass_resource_budget_reads(), 1);
      assert!(Actors::block_resource_state().is_none());
      assert_eq!(Actors::active_actor_view(id).unwrap().cycle_nonce, 0);
      set_prepass_resource_budget(2, Ok(configured));
      assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
      assert_eq!(prepass_resource_budget_reads(), 2);
      assert_eq!(Actors::active_actor_view(id).unwrap().cycle_nonce, 1);
      polkadot_sdk::frame_support::assert_err!(
        Actors::actor_prepass(RuntimeOrigin::none()),
        Error::<Test>::PrepassDuplicateOrStale
      );
      assert_eq!(
        prepass_resource_budget_reads(),
        2,
        "duplicate admission never asks the host for a replacement"
      );
      assert_eq!(
        Actors::block_resource_state().unwrap().budget(),
        Ok(configured)
      );
      Actors::on_idle(2, Weight::MAX);
      Actors::on_finalize(2);
    });
  }
}

#[test]
fn actor_prepass_rejects_signed_origin_before_resource_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_noop!(
      Actors::actor_prepass(RuntimeOrigin::signed(ALICE)),
      DispatchError::BadOrigin
    );
    assert!(Actors::block_resource_state().is_none());
    assert!(Actors::prepass_execution_cutoff().is_none());
  });
}

#[test]
fn block_finalize_rejects_missing_drain_and_stale_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    Actors::on_initialize(1);
    run_prepass();
    assert!(std::panic::catch_unwind(|| Actors::on_finalize(1)).is_err());
  });
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    Actors::on_initialize(1);
    run_prepass();
    assert!(std::panic::catch_unwind(|| Actors::on_finalize(2)).is_err());
  });
}

#[test]
fn block_finalize_rejects_unsettled_reservation_and_missing_telemetry() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    Actors::on_initialize(1);
    run_prepass();
    let budget = TestBlockResourceBudget::get();
    let mut state = Actors::block_resource_state().expect("initialize opens resource state");
    state
      .reserve(
        budget.limits(),
        crate::BlockResourceDomain::UserDispatch,
        Weight::from_parts(1, 1),
      )
      .expect("external phase admits user reservation");
    crate::CurrentBlockResourceState::<Test>::put(state);
    assert!(std::panic::catch_unwind(|| Actors::on_finalize(1)).is_err());
  });
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    Actors::on_initialize(1);
    run_prepass();
    Actors::on_idle(1, Weight::MAX);
    crate::FinalizedBlockResourceTelemetry::<Test>::kill();
    assert!(std::panic::catch_unwind(|| Actors::on_finalize(1)).is_err());
  });
}

#[test]
fn block_finalize_consumes_the_one_pass_marker() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    Actors::on_initialize(1);
    run_prepass();
    Actors::on_idle(1, Weight::MAX);
    Actors::on_finalize(1);
    assert!(Actors::block_resource_state().is_none());
    assert!(std::panic::catch_unwind(|| Actors::on_finalize(1)).is_err());
  });
}

#[test]
fn drain_and_finalization_use_the_budget_frozen_by_prepass() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    Actors::on_initialize(1);
    run_prepass();
    let frozen = Actors::block_resource_state().unwrap().budget().unwrap();
    set_block_resource_budget(crate::BlockResourceBudget::fail_closed(Weight::MAX));
    assert_ne!(TestBlockResourceBudget::get(), frozen);
    assert!(Actors::on_idle(1, Weight::MAX).all_gt(Weight::zero()));
    let state = Actors::block_resource_state().unwrap();
    assert_eq!(state.budget(), Ok(frozen));
    assert_eq!(state.phase(), crate::BlockResourcePhase::Finalizable);
    let snapshot = Actors::finalized_block_resource_telemetry().unwrap();
    assert_eq!(snapshot.budget(), frozen);
    assert_eq!(snapshot.fixed_reserved(), frozen.fixed_envelope());
    Actors::on_finalize(1);
    assert!(Actors::block_resource_state().is_none());
  });
}

#[test]
fn exhausted_actor_control_prevents_on_idle_housekeeping_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let _ = Actors::on_initialize(1);
    run_prepass();
    let budget = TestBlockResourceBudget::get();
    let mut state = Actors::block_resource_state().expect("prepass opens resource state");
    let remaining = budget
      .limits()
      .actor_control()
      .checked_sub(&state.usage().actor_control_used())
      .expect("cutoff owner fits Actor Control"); // deos-bypass: panic-owner — runtime configuration test proves cutoff fit before this package fixture.
    let mut reservation = state
      .reserve(
        budget.limits(),
        crate::BlockResourceDomain::ActorControl,
        remaining,
      )
      .expect("remaining Actor Control is exactly reservable"); // deos-bypass: panic-owner — maximum equals the checked residual of the same limit and usage.
    assert_eq!(state.settle(&mut reservation, remaining), Ok(()));
    crate::CurrentBlockResourceState::<Test>::put(state);
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    assert_eq!(Actors::on_idle(1, Weight::MAX), Weight::zero());
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn mandatory_prepass_does_not_publish_legacy_ticket_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    crate::ActorReadyTail::<Test>::put(7);
    crate::PrepassExecutionCutoff::<Test>::put((0, 3));
    assert_eq!(Actors::on_initialize(1), Weight::zero());
    let prepass = Actors::actor_prepass(RuntimeOrigin::none()).expect("prepass succeeds");
    let empty_prepass = prepass
      .actual_weight
      .expect("prepass reports actual Weight");
    assert!(
      <TestWeightInfo as crate::WeightInfo>::scheduler_on_initialize_cutoff()
        .all_lte(empty_prepass)
    );
    assert_eq!(Actors::prepass_execution_cutoff(), Some((0, 3)));
    let state = Actors::block_resource_state().expect("resource state is opened once");
    assert_eq!(state.phase(), crate::BlockResourcePhase::ExternalPhase);
    assert_eq!(state.usage().actor_control_used(), empty_prepass);
    assert!(!state.optional_actor_work_halted());

    crate::ActorReadyTail::<Test>::put(9);
    assert_noop!(
      Actors::actor_prepass(RuntimeOrigin::none()),
      crate::Error::<Test>::PrepassDuplicateOrStale
    );
    assert_eq!(Actors::prepass_execution_cutoff(), Some((0, 3)));
    let duplicate = Actors::block_resource_state().expect("duplicate preserves state");
    assert_eq!(duplicate.usage(), state.usage());
    assert!(!duplicate.optional_actor_work_halted());

    Actors::on_idle(1, Weight::MAX);
    assert_eq!(Actors::prepass_execution_cutoff(), Some((0, 3)));
    assert_eq!(Actors::queue_head(), 0);
    assert_eq!(Actors::queue_tail(), 9);
    assert_eq!(Actors::queue_occupancy(), 0);
  });
}

#[test]
fn certified_ingress_maps_funding_provenance_to_cause_phase() {
  assert_eq!(
    Actors::test_trigger_cause_provenance(Some(&FundingProvenance::Signed)),
    TriggerCauseProvenance::ExternalPhase
  );
  assert_eq!(
    Actors::test_trigger_cause_provenance(Some(&FundingProvenance::Xcm)),
    TriggerCauseProvenance::Deferred
  );
  assert_eq!(
    Actors::test_trigger_cause_provenance(Some(&FundingProvenance::InternalProtocol)),
    TriggerCauseProvenance::Deferred
  );
  assert_eq!(
    Actors::test_trigger_cause_provenance(None),
    TriggerCauseProvenance::Deferred
  );
}

#[test]
fn queue_pair_preflight_reserves_consecutive_authority_without_mutating_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let first = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let second = create_system_with(
      BOB,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let next_ticket = Actors::next_queue_ticket();
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);

    assert_eq!(
      Actors::test_preflight_queue_pair(first, second),
      Ok([next_ticket, next_ticket + 1])
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before
    );
    assert_eq!(Actors::next_queue_ticket(), next_ticket);
    assert_eq!(Actors::queue_occupancy(), 0);
    assert_eq!(
      Actors::test_preflight_queue_pair(first, first),
      Err(crate::scheduler::EnqueueOutcome::AlreadyLive)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before,
      "duplicate aggregate authority must retain no mutation"
    );
  });
}

#[test]
fn queue_quartet_preflight_reserves_maximum_consecutive_authority_without_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actors = [ALICE, BOB, CHARLIE, 4].map(|owner| {
      create_system_with(
        owner,
        manual_schedule(),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      )
    });
    let next_ticket = Actors::next_queue_ticket();
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);

    assert_eq!(
      Actors::test_preflight_queue_quartet(actors),
      Ok([
        next_ticket,
        next_ticket + 1,
        next_ticket + 2,
        next_ticket + 3,
      ])
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before
    );
    assert_eq!(Actors::next_queue_ticket(), next_ticket);
    assert_eq!(Actors::queue_occupancy(), 0);
  });
}

#[test]
fn queue_quartet_commit_applies_one_aggregate_plan_exactly_once() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actors = [ALICE, BOB, CHARLIE, 4].map(|owner| {
      create_system_with(
        owner,
        manual_schedule(),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      )
    });

    Actors::test_reset_queue_append_commits();
    assert_eq!(Actors::test_commit_queue_quartet(actors), Ok(()));
    assert_eq!(Actors::test_queue_append_commits(), 1);
    assert_eq!(Actors::next_queue_ticket(), 4);
    assert_eq!(Actors::queue_occupancy(), 4);
    assert_eq!(Actors::queue_tail(), 4);
    assert_eq!(
      crate::ActorReadyFrameChunks::<Test>::get(0)
        .expect("first queue page")
        .iter()
        .filter(|cell| cell.is_some())
        .count(),
      4
    );
    for (index, actor_id) in actors.into_iter().enumerate() {
      let hot = Actors::actor_hot(actor_id).expect("queued actor");
      assert!(hot.pending_signal);
      assert_eq!(hot.queue_ticket, Some(index as u64));
    }
  });
}

#[test]
fn queue_pair_preflight_crosses_page_boundary_without_mutating_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut actors = Vec::new();
    for owner in 10_000..10_033 {
      actors.push(create_system_with(
        owner,
        manual_schedule(),
        None,
        contract_steps_with_step(make_step(Task::StopCycle)),
      ));
    }
    for actor_id in actors.iter(/* deos-bypass: bounded-iter */).take(31) {
      assert!(enqueue_latched_actor(*actor_id));
    }
    let next_ticket = Actors::next_queue_ticket();
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);

    assert_eq!(
      Actors::test_preflight_queue_pair(actors[31], actors[32]),
      Ok([next_ticket, next_ticket + 1])
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before
    );
    assert_eq!(Actors::next_queue_ticket(), next_ticket);
    assert_eq!(Actors::queue_occupancy(), 31);
    assert!(crate::ActorReadyFrameChunks::<Test>::get(1).is_none());
  });
}

#[test]
fn zero_step_opening_completes_without_step_or_run_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, BoundedVec::default());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    // Canonical publication gives the latched zero-Step Actor one generation-bound
    // `Service(Pending)` membership for the next block instead of a legacy ready ticket.
    assert!(crate::ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(Actors::service_header().count, 1);
    let (state, _, loaded_step) =
      Actors::load_frame_actor_service_state(actor_id).expect("zero-Step service state loads");
    assert!(loaded_step.is_none());
    let instance = Actors::derive_active_actor_view(state.identity, state.hot, state.contract);
    assert_eq!(
      Actors::classify_actor(actor_id, &instance)
        .expect("zero-Step Actor classifies")
        .execution_phase,
      ActorExecutionPhase::Ready
    );
    System::reset_events();

    // The canonical occurrence is ineligible in its own block, so the service pass runs at B+1.
    frame_system::Pallet::<Test>::set_block_number(2);
    let budget = TestBlockResourceBudget::get();
    let mut resource_state = crate::BlockResourceState::new(2);
    assert_eq!(resource_state.begin_prepass(budget), Ok(()));
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
    let (control, effect) = pass
      .reconciled_domains()
      .expect("zero-Step pass has complete control evidence"); // deos-bypass: panic-owner — zero-Step executes no Task effect and returns bounded control evidence.
    assert_eq!(effect, Weight::zero());
    assert_eq!(control, pass.consumed);
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert_eq!(resource_state.usage().actor_control_used(), control);
    assert_eq!(resource_state.usage().actor_effect_used(), Weight::zero());

    let identity = Actors::actor_identity(actor_id).expect("persistent zero-Step Actor remains");
    assert_eq!(identity.cycle_nonce, 1);
    assert!(ActorRunStateStore::<Test>::get(actor_id).is_none());
    assert!(
      Actors::actor_hot(actor_id)
        .is_some_and(|hot| { hot.cycle_state == CycleState::Idle && hot.queue_ticket.is_none() })
    );
    let actor_events = System::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(event) => Some(event),
        _ => None,
      })
      .collect::<Vec<_>>();
    assert_eq!(
      actor_events,
      vec![
        Event::CycleStarted {
          actor_id,
          cycle_nonce: 1,
        },
        Event::CycleSummary {
          actor_id,
          cycle_nonce: 1,
          result: CycleResult::Completed,
          outcomes: OutcomeTotals::default(),
        },
      ]
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_manual_zero_step_uses_only_canonical_control() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      BoundedVec::default(),
    );
    fund_native(actor_id, 1_000_000_000_000_000_000);
    let installed_hold =
      crate::ActorStateHolds::<Test>::get(actor_id).expect("User state hold is installed");
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    run_next_idle(Weight::MAX);

    let state = Actors::active_actor_state(actor_id).expect("frame successor remains active");
    assert_eq!(state.identity.cycle_nonce, 1);
    assert_eq!(state.hot.cycle_state, CycleState::Idle);
    assert!(!state.hot.pending_signal);
    assert!(state.hot.queue_ticket.is_none());
    assert!(state.run_state.is_none());
    assert_eq!(
      crate::ActorStateHolds::<Test>::get(actor_id),
      Some(installed_hold)
    );
    // Public creation already publishes canonical semantic/process authority, so no legacy control
    // locator or scalar hot cell survives the completed zero-Step cycle.
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(crate::ActorSemanticStates::<Test>::contains_key(actor_id));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary {
        actor_id: id,
        cycle_nonce: 1,
        result: CycleResult::Completed,
        ..
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_opening_stop_cycle_uses_only_canonical_control() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    fund_native(actor_id, 1_000_000_000_000_000_000);
    let installed_hold =
      crate::ActorStateHolds::<Test>::get(actor_id).expect("User state hold is installed");
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    run_next_idle(Weight::MAX);

    let state = Actors::active_actor_state(actor_id).expect("StopCycle successor remains active");
    assert_eq!(state.identity.cycle_nonce, 1);
    assert_eq!(state.hot.cycle_state, CycleState::Idle);
    assert!(!state.hot.pending_signal);
    assert!(state.run_state.is_none());
    assert_eq!(
      crate::ActorStateHolds::<Test>::get(actor_id),
      Some(installed_hold)
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleStopped {
        actor_id: id,
        cycle_nonce: 1,
        step_index: 0,
      } if *id == actor_id
    )));
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_none());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn zero_step_auto_close_one_is_atomic_and_has_no_run_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = Actors::next_actor_id();
    let mut contract = system_active_contract(manual_schedule(), None, BoundedVec::default())
      .expect("zero-Step Contract exists");
    contract.auto_close_at_cycle_nonce = Some(1);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      Some(contract),
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    System::reset_events();
    #[cfg(not(feature = "runtime-benchmarks"))]
    {}

    run_idle(Weight::MAX);

    assert!(Actors::actor_identity(actor_id).is_none());
    assert!(ActorRunStateStore::<Test>::get(actor_id).is_none());
    assert!(System::events().iter().any(|record| matches!(
      record.event,
      RuntimeEvent::Actors(Event::ActorClosed {
        actor_id: closed,
        reason: CloseReason::AutoCloseNonceReached,
        ..
      }) if closed == actor_id
    )));
  });
}

#[test]
fn dormant_identity_owns_no_scheduler_state_and_round_trips_activation() {
  new_test_ext().execute_with(|| {
    use polkadot_sdk::frame_support::traits::{Currency, Hooks};
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      None,
    ));
    let actor_id = 0;
    let identity = Actors::actor_identity(actor_id).expect("dormant identity exists");
    assert_eq!(Actors::actor_identity_count(), 1);
    assert_eq!(Actors::active_actor_count(), 0);
    assert!(Actors::active_actor_view(actor_id).is_none());
    System::reset_events();
    for block in 2..=5 {
      System::set_block_number(block);
      let _ = <Actors as Hooks<MockBlockNumber>>::on_idle(block, Weight::MAX);
    }
    assert!(System::events().iter().all(|record| !matches!(
      record.event,
      RuntimeEvent::Actors(Event::CycleStarted { actor_id: id, .. })
        | RuntimeEvent::Actors(Event::CycleSummary { actor_id: id, .. }) if id == actor_id
    )));
    let preserved = 777;
    let _ =
      <Balances as Currency<AccountId>>::deposit_creating(&identity.sovereign_account, preserved);
    assert_noop!(
      Actors::activate_actor(
        RuntimeOrigin::signed(ALICE),
        actor_id,
        ActorContract {
          funding: FundingSourcePolicy::AnyVerifiedIngress,
          ..user_active_contract(
            manual_schedule(),
            None,
            contract_steps_with_step(make_step(Task::Mint {
              asset: TestAsset::Native,
              amount: AmountResolution::Fixed(1),
            })),
          )
          .expect("direct Actor Contract")
        },
      ),
      Error::<Test>::MintNotAllowedForUserActor
    );
    assert!(Actors::actor_identity(actor_id).is_some());
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(Actors::active_actor_count(), 0);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      ActorContract {
        funding: FundingSourcePolicy::AnyVerifiedIngress,
        ..user_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 10))
          .expect("direct Actor Contract")
      },
    ));
    assert!(Actors::actor_identity(actor_id).is_some());
    let _activated = Actors::active_actor_view(actor_id).expect("active Actor Contract exists");
    assert_eq!(
      Actors::load_actor_contract(actor_id)
        .expect("active Actor Contract")
        .funding,
      FundingSourcePolicy::AnyVerifiedIngress
    );
    assert_eq!(Actors::actor_identity_count(), 1);
    assert_eq!(Actors::active_actor_count(), 1);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(Actors::actor_identity(actor_id).is_some());
    assert_eq!(Actors::actor_identity_count(), 1);
    assert_eq!(Actors::active_actor_count(), 0);
    assert_eq!(native_balance(&identity.sovereign_account), preserved);
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(Actors::actor_identity(actor_id).is_none());
    assert_eq!(Actors::actor_identity_count(), 0);
    assert_eq!(Actors::owner_slot_bitmap(ALICE), [0; 32]);
    assert_eq!(native_balance(&identity.sovereign_account), preserved);
  });
}

#[test]
fn on_idle_never_consumes_above_the_runtime_reserve() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let reserve = <TestWeightInfo as crate::WeightInfo>::scheduler_on_idle_base();
    set_guaranteed_on_idle_weight(reserve);

    let used = Actors::on_idle(1, Weight::MAX);

    assert!(used.all_lte(reserve));
    assert_eq!(
      Actors::actor_identity(actor_id)
        .expect("actor identity remains")
        .cycle_nonce,
      0,
    );
  });
}

#[test]
fn create_rejects_zero_cadence() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::Cadenced { every_ticks: 0 },
      cooldown_blocks: 0,
    };
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
fn user_pause_resume_churn_is_limited_to_one_queue_mutation_per_block() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_eq!(Actors::service_header().count, 1);
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(
      Actors::active_actor_view(actor_id)
        .expect("paused actor")
        .lifecycle
        .is_paused()
    );
    assert_noop!(
      Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ControlMutationRateLimited
    );
    assert_eq!(
      Actors::service_header().count,
      0,
      "rate-limited resume must not republish service residence"
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(Actors::service_header().count, 1);
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ControlMutationRateLimited
    );
    assert_eq!(
      Actors::service_header().count,
      1,
      "rate-limited pause must preserve service residence"
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn successful_manual_execution_preserves_canonical_control() {
  for steps in [inert_contract_steps(), BoundedVec::default()] {
    let predicate_stop = !steps.is_empty();
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);

      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      frame_system::Pallet::<Test>::set_block_number(2);
      let pass = Actors::execute_cycle(Weight::MAX);
      assert!(!pass.starved);
      assert_ne!(pass.consumed, Weight::zero());

      if predicate_stop {
        assert!(
          has_actor_event(|event| matches!(
            event,
            Event::StepSkipped {
              actor_id: id,
              step_index: 0,
              reason: StepSkippedReason::PreconditionFalse,
              ..
            } if *id == actor_id
          )),
          "canonical manual execution events: {:?}",
          System::events(),
        );
        assert!(!has_actor_event(|event| matches!(
          event,
          Event::CycleStopped { actor_id: id, .. } if *id == actor_id
        )));
      }
      assert!(has_actor_event(|event| matches!(
        event,
        Event::CycleSummary { actor_id: id, .. } if *id == actor_id
      )));
      assert!(!ActorIdentities::<Test>::contains_key(actor_id));
      let active = Actors::active_actor_state(actor_id)
        .expect("canonical process authority retains the active Actor");
      assert_eq!(active.hot.cycle_state, CycleState::Idle);
      assert!(Actors::actor_control_cell(actor_id).is_none());
      assert!(crate::ActorProcesses::<Test>::contains_key(actor_id));
      assert!(crate::ServiceNodes::<Test>::contains_key(actor_id));
      #[cfg(feature = "try-runtime")]
      assert_ok!(crate::Pallet::<Test>::do_try_state());
    });
  }
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn mandatory_hook_preserves_canonical_running_successor_and_q1() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
      make_step(Task::StopCycle),
    ])
    .expect("two Steps fit");
    let actor_id = create_user_with(ALICE, Mutability::Mutable, manual_schedule(), None, steps);
    fund_native(actor_id, 1_000_000_000_000_000_000);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    Actors::on_idle(1, Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());

    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .unwrap_or_else(|| {
          panic!(
            "middle-Step Run survives with canonical authority; events={:?}",
            System::events(),
          )
        })
        .cursor,
      1,
    );

    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert_eq!(
      Actors::active_actor_state(actor_id)
        .expect("canonical process authority retains the Running Actor")
        .hot
        .cycle_state,
      CycleState::Running,
    );
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(crate::ActorProcesses::<Test>::contains_key(actor_id));
    assert!(crate::ServiceNodes::<Test>::contains_key(actor_id));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("same-round retry retains the Running successor")
        .cursor,
      1,
    );
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStopped {
        actor_id: id,
        step_index: 1,
        ..
      } if *id == actor_id
    )));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert_eq!(
      Actors::active_actor_state(actor_id)
        .expect("same-round refusal retains canonical Running authority")
        .hot
        .cycle_state,
      CycleState::Running,
    );
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(crate::ActorProcesses::<Test>::contains_key(actor_id));
    assert!(crate::ServiceNodes::<Test>::contains_key(actor_id));

    frame_system::Pallet::<Test>::set_block_number(3);
    run_idle(Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert_eq!(
      Actors::active_actor_state(actor_id)
        .expect("canonical process authority retains the completed Actor")
        .hot
        .cycle_state,
      CycleState::Idle,
    );
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(crate::ActorProcesses::<Test>::contains_key(actor_id));
    assert!(crate::ServiceNodes::<Test>::contains_key(actor_id));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn mandatory_hook_preserves_public_retry_prefix_and_canonical_residence() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut retry = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(77),
      amount: AmountResolution::Fixed(10),
    });
    retry.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
      retry,
    ])
    .expect("prefix and retry Steps fit");
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 2,
      },
      None,
      steps,
    );
    fund_native(actor_id, 1_000_000_000_000_000_000);
    let sovereign = sovereign_account(actor_id);
    let recipient_before = native_balance(&BOB);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    Actors::on_idle(1, Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());

    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    let resident_node = crate::ServiceNodes::<Test>::get(actor_id)
      .expect("successful prefix retains the canonical Service node");
    assert_eq!(native_balance(&BOB), recipient_before + 1);
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("prefix commits before the retry Step")
        .cursor,
      1,
    );
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));

    run_idle(Weight::MAX);
    assert_eq!(
      crate::ServiceNodes::<Test>::get(actor_id),
      Some(resident_node)
    );
    assert_eq!(native_balance(&BOB), recipient_before + 1);

    frame_system::Pallet::<Test>::set_block_number(3);
    run_idle(Weight::MAX);
    let suspended = Actors::actor_run_state(actor_id)
      .expect("missing tracked input produces a genuine suspended Run");
    assert_eq!(suspended.cursor, 1);
    assert_eq!(
      suspended.suspension,
      Some(crate::SuspensionReason::FundingUnavailable)
    );
    assert_eq!(suspended.eligible_at, 5);
    assert!(!crate::ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(
      crate::DeadlineHandles::<Test>::get(actor_id).map(|handle| handle.key),
      Some(WakeupKey::Block(5))
    );
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert_eq!(native_balance(&BOB), recipient_before + 1);

    set_asset_balance(&sovereign, TestAsset::Local(77), 1_000);
    run_canonical_block_at(5, Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_some());
    run_canonical_block_at(6, Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(!crate::DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(crate::ServiceNodes::<Test>::contains_key(actor_id));
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert_eq!(native_balance(&BOB), recipient_before + 1);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(77)), 10);
    assert_eq!(asset_balance(&sovereign, TestAsset::Local(77)), 990);
    assert_eq!(
      Actors::active_actor_state(actor_id)
        .expect("recovered Actor remains active")
        .hot
        .cycle_state,
      CycleState::Idle,
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn public_user_v1_trace_preserves_prefix_retries_non_adjacent_and_recomputes() {
  #[derive(Debug, PartialEq)]
  struct TurnLedgerRow {
    block: u64,
    logical_reads: &'static [&'static str],
    logical_writes: &'static [&'static str],
    membership_inserts: u32,
    membership_removes: u32,
    successor_publications: u32,
    reserved_control: Weight,
    reserved_effect: Weight,
    settled_control: Weight,
    settled_effect: Weight,
    fees: Balance,
    useful_effects: &'static [&'static str],
  }

  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let retry_task = Task::SwapIn {
      asset_in: TestAsset::Native,
      asset_out: TestAsset::Local(77),
      amount_in: AmountResolution::Percent(Perbill::from_percent(50)),
      slippage_tolerance: Perbill::one(),
    };
    let steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
      StepOf::<Test> {
        precondition: None,
        task: retry_task.clone(),
        on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
      },
      make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Local(77),
        amount: AmountResolution::Fixed(1),
      }),
    ])
    .expect("integrated V1 Steps fit");
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 2,
      },
      None,
      steps,
    );
    fund_native(actor_id, 1_000_000);
    let actor = Actors::load_actor_ref(actor_id).expect("active generation-bound Actor");
    let step_resources = Actors::derive_step_resource_envelopes(
      &Actors::actor_contract(actor_id).expect("integrated Contract remains admitted"),
    )
    .expect("integrated Contract resources remain derivable");
    let sovereign = sovereign_account(actor_id);
    let bob_before = native_balance(&BOB);
    let charlie_before = asset_balance(&CHARLIE, TestAsset::Local(77));
    let pool_before = native_balance(&u64::MAX);
    let mut ledger = Vec::new();
    let mut event_cursor = System::events().len();
    let mut record_turn = |block,
                           logical_reads,
                           logical_writes,
                           membership_inserts,
                           membership_removes,
                           successor_publications,
                           reserved: crate::ActorStepResourceEnvelope,
                           useful_effects| {
      let events = System::events();
      let fees = events[event_cursor..]
        .iter()
        .filter_map(|record| match &record.event {
          RuntimeEvent::Actors(Event::PipelineFeeCharged { fee, .. })
          | RuntimeEvent::Actors(Event::ActionFeeCharged { fee, .. }) => Some(*fee),
          _ => None,
        })
        .fold(0u128, Balance::saturating_add);
      event_cursor = events.len();
      let resources = crate::CurrentBlockResourceState::<Test>::get()
        .expect("every serviced turn retains reconciled resource state");
      assert_eq!(resources.outstanding_reservations(), 0);
      ledger.push(TurnLedgerRow {
        block,
        logical_reads,
        logical_writes,
        membership_inserts,
        membership_removes,
        successor_publications,
        reserved_control: reserved.control,
        reserved_effect: reserved.effect,
        settled_control: resources.usage().actor_control_used(),
        settled_effect: resources.usage().actor_effect_used(),
        fees,
        useful_effects,
      });
    };

    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));

    frame_system::Pallet::<Test>::set_block_number(2);
    run_prepass();
    run_idle(Weight::MAX);
    record_turn(
      2,
      &["semantic", "service", "contract[0]", "balances"],
      &["semantic", "process", "run", "balances", "events"],
      0,
      0,
      0,
      step_resources[0],
      &["transfer-native"],
    );
    assert_eq!(native_balance(&BOB), bob_before + 1);
    let after_prefix = Actors::actor_run_state(actor_id).expect("prefix commits");
    assert_eq!(
      (
        after_prefix.cursor,
        after_prefix.unsuccessful_attempts_at_cursor
      ),
      (1, 0)
    );
    assert_eq!(
      crate::ServiceNodes::<Test>::get(actor_id)
        .expect("running Actor has one Service residence")
        .generation,
      actor.generation
    );
    assert!(!crate::DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));

    frame_system::Pallet::<Test>::set_block_number(3);
    run_prepass();
    run_idle(Weight::MAX);
    record_turn(
      3,
      &[
        "semantic",
        "service",
        "process",
        "run",
        "contract[1]",
        "dex",
        "balances",
      ],
      &[
        "semantic", "process", "run", "service", "deadline", "fees", "events",
      ],
      1,
      1,
      1,
      step_resources[1],
      &[],
    );
    let suspended = Actors::actor_run_state(actor_id).expect("temporary attempt suspends");
    assert_eq!(
      (suspended.cursor, suspended.unsuccessful_attempts_at_cursor),
      (1, 1)
    );
    assert_eq!(
      suspended.suspension,
      Some(crate::SuspensionReason::Temporary)
    );
    assert_eq!(suspended.eligible_at, 5);
    assert!(!crate::ServiceNodes::<Test>::contains_key(actor_id));
    let deadline = crate::DeadlineHandles::<Test>::get(actor_id)
      .expect("non-adjacent retry owns one Deadline residence");
    assert_eq!((deadline.actor, deadline.key), (actor, WakeupKey::Block(5)));
    assert_eq!(native_balance(&u64::MAX), pool_before);
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));

    fund_native(actor_id, 10_000_000);
    let before_recovery = native_balance(&sovereign);
    let action_fee = <TestWeightToFee as polkadot_sdk::sp_weights::WeightToFee>::weight_to_fee(
      &Actors::weight_upper_bound(&retry_task),
    );
    let expected_in = Perbill::from_percent(50).mul_floor(
      before_recovery
        .saturating_sub(action_fee)
        .saturating_sub(TestMinUserBalance::get()),
    );
    set_temporary_dex_failure(false);

    frame_system::Pallet::<Test>::set_block_number(5);
    Actors::on_initialize(5);
    run_prepass();
    run_idle(Weight::MAX);
    record_turn(
      5,
      &["deadline", "process", "semantic"],
      &["deadline", "service", "process", "semantic"],
      1,
      1,
      1,
      crate::ActorStepResourceEnvelope {
        control: Weight::zero(),
        effect: Weight::zero(),
      },
      &[],
    );
    assert!(crate::ServiceNodes::<Test>::contains_key(actor_id));
    assert!(!crate::DeadlineHandles::<Test>::contains_key(actor_id));
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("recovery is deferred to B+1")
        .cursor,
      1
    );

    frame_system::Pallet::<Test>::set_block_number(6);
    run_prepass();
    run_idle(Weight::MAX);
    record_turn(
      6,
      &[
        "semantic",
        "service",
        "process",
        "run",
        "contract[1]",
        "dex",
        "balances",
      ],
      &["semantic", "process", "run", "balances", "fees", "events"],
      0,
      0,
      0,
      step_resources[1],
      &["swap"],
    );
    let recovered = Actors::actor_run_state(actor_id).expect("recovered Step commits");
    assert_eq!(
      (recovered.cursor, recovered.unsuccessful_attempts_at_cursor),
      (2, 0)
    );
    assert_eq!(
      native_balance(&u64::MAX),
      pool_before.saturating_add(expected_in)
    );
    assert_eq!(
      asset_balance(&CHARLIE, TestAsset::Local(77)),
      charlie_before
    );

    frame_system::Pallet::<Test>::set_block_number(7);
    run_prepass();
    run_idle(Weight::MAX);
    record_turn(
      7,
      &[
        "semantic",
        "service",
        "process",
        "run",
        "contract[2]",
        "balances",
      ],
      &["semantic", "process", "run", "balances", "fees", "events"],
      0,
      0,
      0,
      step_resources[2],
      &["transfer-local"],
    );
    assert_eq!(ledger.len(), 5);
    assert_eq!(
      ledger.iter().map(|row| row.block).collect::<Vec<_>>(),
      vec![2, 3, 5, 6, 7]
    );
    assert!(
      ledger
        .iter()
        .all(|row| !row.logical_reads.is_empty() && !row.logical_writes.is_empty())
    );
    assert!(
      ledger
        .iter()
        .all(|row| row.settled_control != Weight::zero())
    );
    assert!(
      ledger
        .iter()
        .filter(|row| !row.useful_effects.is_empty())
        .all(|row| {
          row.reserved_control != Weight::zero()
            && row.reserved_effect != Weight::zero()
            && row.settled_effect != Weight::zero()
            && row.settled_effect.all_lte(row.reserved_effect)
            && row.fees > 0
        })
    );
    let interior = ledger
      .iter()
      .find(|row| row.block == 6)
      .expect("interior resident turn");
    assert_eq!(
      (
        interior.membership_inserts,
        interior.membership_removes,
        interior.successor_publications
      ),
      (0, 0, 0)
    );
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(
      asset_balance(&CHARLIE, TestAsset::Local(77)),
      charlie_before + 1
    );
    assert_eq!(
      Actors::active_actor_state(actor_id)
        .expect("persistent Actor completes Idle")
        .hot
        .cycle_state,
      CycleState::Idle,
    );
    assert_eq!(
      crate::ServiceNodes::<Test>::get(actor_id)
        .expect("Idle manual Actor has one canonical Service residence")
        .generation,
      actor.generation
    );
    assert!(!crate::DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn mandatory_hook_resolves_percent_against_current_balance_between_adjacent_steps() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let percentage = AmountResolution::Percent(Perbill::from_percent(50));
    let steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Local(77),
        amount: percentage,
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Local(77),
        amount: percentage,
      }),
    ])
    .expect("two percent Steps fit");
    let actor_id = create_user_with(ALICE, Mutability::Mutable, manual_schedule(), None, steps);
    fund_native(actor_id, 1_000_000_000_000_000_000);
    let sovereign = sovereign_account(actor_id);
    set_asset_balance(&sovereign, TestAsset::Local(77), 1_000);
    let bob_before = asset_balance(&BOB, TestAsset::Local(77));

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    // The first Step resolves half of the current tracked balance after its action fee.
    assert_eq!(asset_balance(&BOB, TestAsset::Local(77)), bob_before + 499);
    assert_eq!(asset_balance(&sovereign, TestAsset::Local(77)), 501);
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("first Step commits a running cursor")
        .cursor,
      1,
    );
    assert!(crate::ServiceNodes::<Test>::contains_key(actor_id));
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));

    // A normal external credit between Steps raises the later resolution's current input.
    set_asset_balance(&sovereign, TestAsset::Local(77), 400);
    frame_system::Pallet::<Test>::set_block_number(3);
    run_idle(Weight::MAX);
    assert_eq!(
      asset_balance(&BOB, TestAsset::Local(77)),
      bob_before + 499 + 450
    );
    assert_eq!(asset_balance(&sovereign, TestAsset::Local(77)), 451);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(crate::ServiceNodes::<Test>::contains_key(actor_id));
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn mandatory_hook_reresolves_percent_on_a_recovered_retry_attempt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let task = Task::SwapIn {
      asset_in: TestAsset::Native,
      asset_out: TestAsset::Local(77),
      amount_in: AmountResolution::Percent(Perbill::from_percent(50)),
      slippage_tolerance: Perbill::one(),
    };
    let retry_step = StepOf::<Test> {
      precondition: None,
      task: task.clone(),
      on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
    };
    let steps = BoundedVec::try_from(vec![retry_step]).expect("retry Step fits");
    let actor_id = create_user_with(ALICE, Mutability::Mutable, manual_schedule(), None, steps);
    fund_native(actor_id, 1_000_000);
    let sovereign = sovereign_account(actor_id);
    let pool_before = native_balance(&u64::MAX);

    // A genuine temporary effect failure suspends the Run without committing a resolved effect.
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    let suspended = Actors::actor_run_state(actor_id).expect("temporary failure suspends the Run");
    assert_eq!(
      suspended.suspension,
      Some(crate::SuspensionReason::Temporary)
    );
    assert_eq!(native_balance(&u64::MAX), pool_before);

    // A normal external credit between Attempts raises the later resolution's current input.
    fund_native(actor_id, 10_000_000);
    let before_recovery = native_balance(&sovereign);
    let action_fee = <TestWeightToFee as polkadot_sdk::sp_weights::WeightToFee>::weight_to_fee(
      &Actors::weight_upper_bound(&task),
    );
    let expected_in = Perbill::from_percent(50).mul_floor(
      before_recovery
        .saturating_sub(action_fee)
        .saturating_sub(TestMinUserBalance::get()),
    );
    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(3);
    run_idle(Weight::MAX);

    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(
      native_balance(&u64::MAX),
      pool_before.saturating_add(expected_in),
      "the recovered Attempt re-resolves Percent against the current tracked balance"
    );
    assert!(crate::ServiceNodes::<Test>::contains_key(actor_id));
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_expired_manual_activation_closes_from_retained_unsignaled_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      BoundedVec::default(),
    );
    fund_native(actor_id, 1_000_000_000_000_000_000);
    assert!(crate::ActorStateHolds::<Test>::contains_key(actor_id));

    frame_system::Pallet::<Test>::set_block_number(102);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(!Actors::active_actor_exists(actor_id));
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!crate::ActorStateHolds::<Test>::contains_key(actor_id));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(!Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::WindowExpired,
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn manual_middle_step_preserves_canonical_control() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
      make_step(Task::StopCycle),
    ])
    .expect("two Steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    fund_native(actor_id, 10);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    // Canonical publication makes the occurrence ineligible in its own block, so the first Step
    // executes at B+1.
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::execute_cycle(Weight::MAX);
    let run = Actors::actor_run_state(actor_id).expect("middle-Step Run survives");
    assert_eq!(run.cursor, 1);
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(!ActorControlLocators::<Test>::contains_key(actor_id));

    frame_system::Pallet::<Test>::set_block_number(3);
    Actors::execute_cycle(Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary { actor_id: id, .. } if *id == actor_id
    )));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(!ActorControlLocators::<Test>::contains_key(actor_id));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_paused_ready_pop_uses_only_canonical_control() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000_000_000_000_000_000);
    let installed_hold =
      crate::ActorStateHolds::<Test>::get(actor_id).expect("User state hold is installed");
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    let paused_before =
      Actors::active_actor_state(actor_id).expect("pause atom retains canonical authority");
    assert!(paused_before.hot.lifecycle.is_paused());
    System::reset_events();

    run_next_idle(Weight::MAX);

    let state = Actors::active_actor_state(actor_id).expect("paused authority remains active");
    assert!(state.hot.lifecycle.is_paused());
    assert!(state.hot.pending_signal);
    assert!(state.hot.queue_ticket.is_none());
    assert_eq!(
      crate::ActorStateHolds::<Test>::get(actor_id),
      Some(installed_hold)
    );
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_circuit_breaker_skip_uses_only_canonical_control() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    let before = Actors::actor_hot(actor_id).expect("canonical Ready owner");
    let header_before = Actors::service_header();

    // Canonical publication latches the occurrence for B+1, so the breaker refusal is observed at
    // the next block rather than in the same block as the trigger.
    frame_system::Pallet::<Test>::set_block_number(2);
    let consumed = Actors::execute_cycle(Weight::MAX).consumed;
    let expected = <TestWeightInfo as crate::WeightInfo>::service_round_begin_populated()
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::service_round_probe_eligible())
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::scheduler_actor_state_probe());
    assert_eq!(consumed, expected);
    assert_eq!(Actors::actor_hot(actor_id), Some(before));
    assert_eq!(Actors::service_header().count, header_before.count);
    assert_eq!(Actors::service_header().cursor, header_before.cursor);
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));

    let state = Actors::active_actor_state(actor_id).expect("breaker-skipped authority remains");
    assert!(state.hot.pending_signal);
    assert!(state.hot.queue_ticket.is_none());
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).and_then(|process| process.residence),
      Some(ProcessResidence::Service(ServiceResidenceKind::Pending))
    );
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn manual_trigger_survives_paused_queue_pop_and_resume() {
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
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    let budget = TestBlockResourceBudget::get();
    let mut resource_state = crate::BlockResourceState::new(1);
    assert_eq!(resource_state.begin_prepass(budget), Ok(()));
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
      pass.reconciled_domains(),
      Some((pass.consumed, Weight::zero()))
    );
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert_eq!(resource_state.usage().actor_control_used(), pass.consumed);
    assert_eq!(resource_state.usage().actor_effect_used(), Weight::zero());
    let paused = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(paused.pending_signal);
    assert_eq!(paused.cycle_nonce, 0);
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    run_idle(Weight::MAX);
    let resumed = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(!resumed.pending_signal);
    assert_eq!(resumed.cycle_nonce, 1);
  });
}

fn prepare_due_deadline_frontiers() -> (Vec<u64>, Vec<u64>) {
  System::set_block_number(1);
  let mut sleepers = Vec::new();
  for _ in 0..2 {
    let mut step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(77),
      amount: AmountResolution::Fixed(10),
    });
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 2 };
    let id = create_system_with(
      ALICE,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 2,
      },
      None,
      BoundedVec::try_from(vec![step]).unwrap(),
    );
    fund_native(id, 1_000);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
    sleepers.push(id);
  }
  System::set_block_number(2);
  run_prepass();
  for id in &sleepers {
    assert_eq!(
      Actors::deadline_handles(id).unwrap().key,
      WakeupKey::Block(4)
    );
    assert_eq!(Actors::actor_run_state(*id).unwrap().cursor, 0);
  }
  Actors::on_idle(2, Weight::MAX);
  Actors::on_finalize(2);
  let timers = (0..2)
    .map(|_| create_system_with(ALICE, at_time_schedule(2), None, BoundedVec::default()))
    .collect::<Vec<_>>();
  (sleepers, timers)
}

#[test]
fn mandatory_prepass_services_both_deadline_clocks_before_external_dispatch() {
  new_test_ext().execute_with(|| {
    let (sleepers, timers) = prepare_due_deadline_frontiers();
    let runs = sleepers
      .iter()
      .copied()
      .map(Actors::actor_run_state)
      .collect::<Vec<_>>();
    let custody = sleepers
      .iter()
      .map(|id| native_balance(&sovereign_account(*id)))
      .collect::<Vec<_>>();
    let residents = (0..4)
      .map(|_| {
        let id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
        assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
        id
      })
      .collect::<Vec<_>>();
    System::set_block_number(4);
    run_prepass();
    assert_eq!(
      Actors::block_resource_state().unwrap().phase(),
      crate::BlockResourcePhase::ExternalPhase
    );
    assert!(
      Actors::deadline_handles(sleepers[0]).is_none(),
      "Block return belongs to Prepass, not on_idle"
    );
    assert!(
      Actors::trigger_deadline_handles(timers[0]).is_none(),
      "Tick detection belongs to Prepass, not on_idle"
    );
    for id in [sleepers[0], timers[0]] {
      assert_eq!(Actors::service_nodes(id).unwrap().eligible_from, 5);
      assert_eq!(Actors::active_actor_view(id).unwrap().cycle_nonce, 0);
    }
    assert!(Actors::deadline_handles(sleepers[1]).is_some());
    assert!(Actors::trigger_deadline_handles(timers[1]).is_some());
    assert_eq!(
      residents
        .iter()
        .filter(|id| Actors::active_actor_view(**id).unwrap().cycle_nonce == 1)
        .count(),
      3
    );
    assert_eq!(
      sleepers
        .iter()
        .copied()
        .map(Actors::actor_run_state)
        .collect::<Vec<_>>(),
      runs
    );
    assert_eq!(
      sleepers
        .iter()
        .map(|id| native_balance(&sovereign_account(*id)))
        .collect::<Vec<_>>(),
      custody
    );
    Actors::on_idle(4, Weight::MAX);
    assert!(
      Actors::deadline_handles(sleepers[1]).is_some(),
      "Drain cannot duplicate the mandatory deadline quantum"
    );
    assert!(Actors::trigger_deadline_handles(timers[1]).is_some());
    assert_eq!(Actors::active_actor_view(timers[0]).unwrap().cycle_nonce, 0);
    assert_eq!(
      Actors::block_resource_state()
        .unwrap()
        .outstanding_reservations(),
      0
    );
    Actors::on_finalize(4);
  });
}

#[test]
fn mandatory_prepass_deadline_reservation_refuses_each_dimension_before_mutation() {
  use crate::weights::WeightInfo;
  for shortfall in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
    new_test_ext().execute_with(|| {
      let (sleepers, timers) = prepare_due_deadline_frontiers();
      System::set_block_number(4);
      type W = <Test as crate::Config>::WeightInfo;
      let finalization = W::scheduler_on_idle_base().saturating_add(W::block_resource_finalize());
      let minimum = W::scheduler_on_initialize_cutoff()
        .saturating_add(Actors::deadline_service_weight_upper())
        .saturating_add(W::dependency_scan_source_probe())
        .saturating_add(
          W::process_dependency_scan_unit().max(W::process_dependency_scan_completion_unit()),
        )
        .saturating_add(finalization);
      let refused = minimum.checked_sub(&shortfall).unwrap();
      set_block_resource_budget(
        crate::BlockResourceBudget::new_with_control_ratio(
          refused.saturating_mul(3),
          Weight::zero(),
          1,
          3,
        )
        .unwrap(),
      );
      assert_eq!(
        TestBlockResourceBudget::get().limits().actor_control(),
        refused
      );
      assert_noop!(
        Actors::actor_prepass(RuntimeOrigin::none()),
        crate::Error::<Test>::ResourceProtocolFailed
      );
      assert!(Actors::deadline_handles(sleepers[0]).is_some());
      assert!(Actors::trigger_deadline_handles(timers[0]).is_some());
      set_block_resource_budget(
        crate::BlockResourceBudget::new_with_control_ratio(
          minimum.saturating_mul(3),
          Weight::zero(),
          1,
          3,
        )
        .unwrap(),
      );
      run_prepass();
      assert!(Actors::deadline_handles(sleepers[0]).is_none());
      assert!(Actors::trigger_deadline_handles(timers[0]).is_none());
      let state = Actors::block_resource_state().unwrap();
      assert!(
        state
          .usage()
          .actor_control_used()
          .saturating_add(finalization)
          .all_lte(minimum)
      );
      assert_eq!(state.outstanding_reservations(), 0);
      Actors::on_idle(4, Weight::MAX);
      Actors::on_finalize(4);
    });
  }
}

#[test]
fn mandatory_prepass_pass_admits_effectful_service_without_double_reserving_control() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(crate::ActorControlLocators::<Test>::get(actor_id).is_none());
    frame_system::Pallet::<Test>::set_block_number(2);
    let budget = TestBlockResourceBudget::get();
    let mut resource_state = crate::BlockResourceState::new(2);
    assert_eq!(resource_state.begin_prepass(budget), Ok(()));
    // The enclosing pass owns the ActorControl envelope; the canonical Service round must admit
    // the effectful head without reserving that control a second time.
    Actors::execute_cycle_to_cutoff_with_resources(
      Weight::MAX,
      Actors::next_queue_ticket(),
      &mut resource_state,
      budget.limits(),
      crate::BlockResourceDomain::ActorBaseEffect,
      budget.limits().actor_control(),
    );
    let executed = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(executed.cycle_nonce, 1);
    assert!(!executed.pending_signal);
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert!(resource_state.usage().actor_effect_used() != Weight::zero());
    assert!(crate::ActorControlLocators::<Test>::get(actor_id).is_none());
  });
}

#[test]
fn service_discovery_refuses_before_round_mutation_in_each_dimension() {
  use crate::WeightInfo;
  for pass_owned in [false, true] {
    for deficit in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
      for (with_actor, closed) in [(false, false), (true, false), (true, true)] {
        new_test_ext().execute_with(|| {
          System::set_block_number(1);
          if with_actor {
            let id = create_system_with(
              ALICE,
              manual_schedule(),
              None,
              transfer_contract_steps(BOB, 1),
            );
            fund_native(id, 1_000);
            assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
          }
          System::set_block_number(2);
          if closed {
            let mut setup =
              polkadot_sdk::frame_support::weights::WeightMeter::with_limit(Weight::MAX);
            assert_ok!(Actors::service_canonical_round_head(&mut setup, 2));
          }
          let selector = TestWeightInfo::service_round_begin_populated()
            .saturating_add(TestWeightInfo::service_round_probe_eligible());
          let short = selector.saturating_sub(deficit);
          let root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
          let mut state = crate::BlockResourceState::new(2);
          assert_ok!(state.begin_prepass(TestBlockResourceBudget::get()));
          let pass = if pass_owned {
            Actors::execute_cycle_to_cutoff_with_resources(
              Weight::MAX,
              0,
              &mut state,
              TestBlockResourceBudget::get().limits(),
              crate::BlockResourceDomain::ActorBaseEffect,
              short,
            )
          } else {
            Actors::execute_cycle(short)
          };
          assert_eq!(
            polkadot_sdk::sp_io::storage::root(StateVersion::V1),
            root,
            "unadmitted discovery must not open a round"
          );
          assert_eq!(pass.consumed, Weight::zero());
          assert!(
            !pass.starved,
            "uninspected work is not evidence of starvation"
          );
          assert!(!pass.starvation_observed);
          assert_eq!(state.usage().actor_control_used(), Weight::zero());
          assert_eq!(state.usage().actor_effect_used(), Weight::zero());
          assert_eq!(state.outstanding_reservations(), 0);
          assert!(!state.optional_actor_work_halted());
        });
      }
    }
  }
}

#[test]
fn service_discovery_classifies_without_admitting_actor_loading() {
  use crate::WeightInfo;
  for pass_owned in [false, true] {
    for (with_actor, closed) in [(false, false), (true, false), (true, true)] {
      new_test_ext().execute_with(|| {
        System::set_block_number(1);
        if with_actor {
          let id = create_system_with(
            ALICE,
            manual_schedule(),
            None,
            transfer_contract_steps(BOB, 1),
          );
          fund_native(id, 1_000);
          assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
        }
        System::set_block_number(2);
        if closed {
          let mut setup =
            polkadot_sdk::frame_support::weights::WeightMeter::with_limit(Weight::MAX);
          assert_ok!(Actors::service_canonical_round_head(&mut setup, 2));
        }
        let selector = TestWeightInfo::service_round_begin_populated()
          .saturating_add(TestWeightInfo::service_round_probe_eligible());
        let root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
        let mut state = crate::BlockResourceState::new(2);
        assert_ok!(state.begin_prepass(TestBlockResourceBudget::get()));
        let pass = if pass_owned {
          Actors::execute_cycle_to_cutoff_with_resources(
            Weight::MAX,
            0,
            &mut state,
            TestBlockResourceBudget::get().limits(),
            crate::BlockResourceDomain::ActorBaseEffect,
            selector,
          )
        } else {
          Actors::execute_cycle(selector)
        };
        assert_eq!(
          pass.consumed, selector,
          "one admitted discovery owns its work"
        );
        assert_eq!(pass.starved, with_actor && !closed);
        assert!(pass.starvation_observed);
        if with_actor {
          assert_eq!(
            polkadot_sdk::sp_io::storage::root(StateVersion::V1),
            root,
            "a refused head or closed round preserves exact authority"
          );
        } else {
          assert_eq!(Actors::service_header().round_block, Some(2));
          assert_eq!(Actors::service_header().count, 0);
        }
        assert_eq!(
          state.usage().actor_control_used(),
          if pass_owned { selector } else { Weight::zero() }
        );
        assert_eq!(state.usage().actor_effect_used(), Weight::zero());
        assert_eq!(state.outstanding_reservations(), 0);
        assert!(!state.optional_actor_work_halted());
      });
    }
  }
}

#[test]
fn service_discovery_stops_on_defensive_attempt_marker() {
  use crate::WeightInfo;
  for pass_owned in [false, true] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      fund_native(id, 1_000);
      assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
      System::set_block_number(2);
      let mut setup = WeightMeter::with_limit(Weight::MAX);
      assert_ok!(Actors::service_canonical_round_head(&mut setup, 2));
      // Corrupt only the consideration marker after a real attempt. The independent process
      // marker must still prevent replay; this is a defensive witness, not normal publication.
      crate::ServiceNodes::<Test>::mutate(id, |node| node.as_mut().unwrap().last_considered = 1);
      let root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
      let selector = TestWeightInfo::service_round_begin_populated()
        .saturating_add(TestWeightInfo::service_round_probe_eligible());
      let mut state = crate::BlockResourceState::new(2);
      assert_ok!(state.begin_prepass(TestBlockResourceBudget::get()));
      let pass = if pass_owned {
        Actors::execute_cycle_to_cutoff_with_resources(
          Weight::MAX,
          0,
          &mut state,
          TestBlockResourceBudget::get().limits(),
          crate::BlockResourceDomain::ActorBaseEffect,
          selector,
        )
      } else {
        Actors::execute_cycle(selector)
      };
      assert_eq!(pass.consumed, selector);
      assert!(pass.starvation_observed && !pass.starved);
      assert_eq!(state.outstanding_reservations(), 0);
      assert!(!state.optional_actor_work_halted());
      assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), root);
    });
  }
}

#[test]
fn pass_owned_control_matches_direct_service_actual_accounting() {
  for actor_type in [ActorType::User, ActorType::System] {
    for zero_step in [false, true] {
      let run = |pass_owned| {
        new_test_ext().execute_with(|| {
          System::set_block_number(1);
          let steps = if zero_step {
            BoundedVec::default()
          } else {
            transfer_contract_steps(BOB, 1)
          };
          let id = match actor_type {
            ActorType::User => {
              create_user_with(ALICE, Mutability::Mutable, manual_schedule(), None, steps)
            }
            ActorType::System => create_system_with(ALICE, manual_schedule(), None, steps),
          };
          fund_native(id, 1_000);
          assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
          System::set_block_number(2);
          let limits = TestBlockResourceBudget::get().limits();
          let mut state = crate::BlockResourceState::new(2);
          assert_ok!(state.begin_prepass(TestBlockResourceBudget::get()));
          let consumed = if pass_owned {
            Actors::execute_cycle_to_cutoff_with_resources(
              Weight::MAX,
              0,
              &mut state,
              limits,
              crate::BlockResourceDomain::ActorBaseEffect,
              limits.actor_control(),
            )
            .consumed
          } else {
            let mut meter =
              polkadot_sdk::frame_support::weights::WeightMeter::with_limit(Weight::MAX);
            assert_ok!(Actors::service_canonical_round_head_with_resources(
              &mut meter,
              2,
              &mut state,
              limits,
              crate::BlockResourceDomain::ActorBaseEffect
            ));
            // Compare the same work: the pass also discovers the now-closed round.
            assert_eq!(
              Actors::service_canonical_round_head_with_resources(
                &mut meter,
                2,
                &mut state,
                limits,
                crate::BlockResourceDomain::ActorBaseEffect,
              ),
              Ok(crate::ServiceRoundEncounter::Closed)
            );
            meter.consumed()
          };
          assert_eq!(Actors::active_actor_view(id).unwrap().cycle_nonce, 1);
          assert!(!state.optional_actor_work_halted());
          assert_eq!(state.outstanding_reservations(), 0);
          assert_eq!(
            consumed,
            state
              .usage()
              .actor_control_used()
              .saturating_add(state.usage().actor_effect_used())
          );
          (
            consumed,
            state.usage().actor_control_used(),
            state.usage().actor_effect_used(),
          )
        })
      };
      assert_eq!(
        run(true),
        run(false),
        "pass reservation changes ownership, not actual work: {actor_type:?}, zero_step={zero_step}"
      );
    }
  }
}

#[test]
fn pass_owned_control_cannot_borrow_effect_capacity_or_bypass_a_refused_head() {
  use crate::WeightInfo;
  for proof_shortfall in [false, true] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let ids = (0..2)
        .map(|_| {
          let id = create_system_with(
            ALICE,
            manual_schedule(),
            None,
            transfer_contract_steps(BOB, 1),
          );
          fund_native(id, 1_000);
          assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
          id
        })
        .collect::<Vec<_>>();
      System::set_block_number(2);
      let actor = crate::ActorRef {
        actor_id: ids[0],
        generation: Actors::service_nodes(ids[0]).unwrap().generation,
      };
      let semantic =
        Actors::load_service_actor_semantic_state(actor, crate::ServiceResidenceKind::Pending)
          .unwrap();
      let resources = Actors::load_actor_service_state_with_control(
        ids[0],
        semantic.identity,
        semantic.hot,
        semantic.admission,
      )
      .and_then(|(_, _, step)| step)
      .unwrap()
      .resources;
      let inspection = TestWeightInfo::service_round_begin_populated()
        .saturating_add(TestWeightInfo::service_round_probe_eligible())
        .saturating_add(TestWeightInfo::scheduler_actor_state_probe());
      let suffix = TestWeightInfo::service_round_admit_eligible()
        .max(TestWeightInfo::service_member_retire_interior())
        .max(TestWeightInfo::service_member_retire_pair_cursor())
        .max(TestWeightInfo::service_member_retire_singleton());
      let complete_control = inspection
        .saturating_add(resources.control)
        .saturating_add(suffix);
      let control_limit = complete_control.saturating_sub(if proof_shortfall {
        Weight::from_parts(0, 1)
      } else {
        Weight::from_parts(1, 0)
      });
      assert!(inspection.all_lte(control_limit));
      assert_ok!(
        polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
            Actors::begin_service_round(2),
          )
        })
      );
      let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      let mut state = crate::BlockResourceState::new(2);
      assert_ok!(state.begin_prepass(TestBlockResourceBudget::get()));
      let pass = Actors::execute_cycle_to_cutoff_with_resources(
        complete_control.saturating_add(resources.effect),
        0,
        &mut state,
        TestBlockResourceBudget::get().limits(),
        crate::BlockResourceDomain::ActorBaseEffect,
        control_limit,
      );
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        root
      );
      assert!(pass.starved);
      assert_eq!(pass.consumed, inspection);
      assert_eq!(state.usage().actor_control_used(), inspection);
      assert_eq!(state.usage().actor_effect_used(), Weight::zero());
      assert_eq!(state.outstanding_reservations(), 0);
      assert!(
        !state.optional_actor_work_halted(),
        "ordinary capacity refusal is not corrupt accounting"
      );
    });
  }
}

#[test]
fn paired_service_refusal_recovers_without_progress_or_fee_loss() {
  use crate::{BlockResourceDomain as Domain, WeightInfo};
  for (drain, control_shortfall) in [(false, false), (true, false), (false, true), (true, true)] {
    for deficit in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
      for running in [false, true] {
        new_test_ext().execute_with(|| {
          // Synthetic settled usage isolates the admission boundary, not block throughput.
          let budget = crate::BlockResourceBudget::new_with_control_ratio(
            Weight::from_parts(2_000_000_000_000, 10_485_760),
            Weight::zero(),
            1,
            3,
          )
          .unwrap();
          let limits = budget.limits();
          System::set_block_number(1);
          let steps =
            BoundedVec::try_from(vec![transfer_contract_steps(BOB, 1)[0].clone(); 3]).unwrap();
          let ids = (0..2)
            .map(|_| {
              let id = create_user_with(
                ALICE,
                Mutability::Mutable,
                manual_schedule(),
                None,
                steps.clone(),
              );
              fund_native(id, 10_000);
              assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
              assert_eq!(Actors::service_nodes(id).unwrap().eligible_from, 2);
              id
            })
            .collect::<Vec<_>>();
          System::set_block_number(2);
          if running {
            let mut opening = crate::BlockResourceState::new(2);
            assert_ok!(opening.begin_prepass(budget));
            Actors::execute_cycle_to_cutoff_with_resources(
              Weight::MAX,
              0,
              &mut opening,
              limits,
              Domain::ActorBaseEffect,
              limits.actor_control(),
            );
            for id in &ids {
              assert_eq!(Actors::actor_run_state(*id).unwrap().cursor, 1);
            }
            assert_eq!(opening.outstanding_reservations(), 0);
            System::set_block_number(3);
          }
          let now = System::block_number();
          let resources = Actors::load_current_step_from_storage(ids[0], u32::from(running))
            .unwrap()
            .resources;
          let inspection = TestWeightInfo::service_round_begin_populated()
            .saturating_add(TestWeightInfo::service_round_probe_eligible())
            .saturating_add(TestWeightInfo::scheduler_actor_state_probe());
          let suffix = TestWeightInfo::service_round_admit_eligible()
            .max(TestWeightInfo::service_member_retire_interior())
            .max(TestWeightInfo::service_member_retire_pair_cursor())
            .max(TestWeightInfo::service_member_retire_singleton());
          let complete_control = inspection
            .saturating_add(resources.control)
            .saturating_add(suffix);
          assert!(complete_control.all_lte(limits.actor_control()));
          assert!(resources.effect.all_lte(limits.actor_base_turn()));
          let domain = if drain {
            Domain::ActorDrainEffect
          } else {
            Domain::ActorBaseEffect
          };
          let mut state = crate::BlockResourceState::new(now);
          assert_ok!(state.begin_prepass(budget));
          if drain {
            assert_ok!(state.open_external_phase());
            assert_ok!(state.begin_drain());
          }
          let (charged_domain, capacity, required) = if control_shortfall {
            (
              Domain::ActorControl,
              limits.actor_control(),
              complete_control,
            )
          } else {
            (
              domain,
              if drain {
                limits.shared_economic()
              } else {
                limits.actor_base_turn()
              },
              resources.effect,
            )
          };
          let remaining = required.checked_sub(&deficit).unwrap();
          let prior_usage = capacity.checked_sub(&remaining).unwrap();
          let mut prior = state.reserve(limits, charged_domain, prior_usage).unwrap();
          assert_ok!(state.settle(&mut prior, prior_usage));
          let before = state.usage();
          let control = if control_shortfall {
            remaining
          } else {
            limits.actor_control()
          };
          assert!(inspection.all_lte(control));
          assert_ok!(
            polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
              polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
                Actors::begin_service_round(now),
              )
            })
          );
          System::reset_events();
          clear_fee_collections();
          let root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
          let recipient = native_balance(&BOB);
          let pass = Actors::execute_cycle_to_cutoff_with_resources(
            Weight::MAX,
            0,
            &mut state,
            limits,
            domain,
            control,
          );
          assert!(pass.starved);
          assert_eq!(pass.consumed, inspection);
          // Covers ring, process, Run, pending signal, attempt markers, holds, custody and events.
          assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), root);
          assert!(fee_collections().is_empty());
          assert_eq!(
            state.usage().actor_effect_used(),
            before.actor_effect_used()
          );
          assert_eq!(
            state.usage().actor_control_used(),
            before.actor_control_used().saturating_add(inspection)
          );
          assert_eq!(state.outstanding_reservations(), 0);
          assert!(!state.optional_actor_work_halted());
          assert_eq!(Actors::service_header().cursor.unwrap().actor_id, ids[0]);

          if !drain {
            assert_ok!(state.open_external_phase());
            assert_ok!(state.begin_drain());
          }
          if drain || control_shortfall {
            // A fully spent shared pool or Control can recover only in a later block.
            assert_ok!(state.finish_drain());
            assert_ok!(state.finalized_snapshot());
            System::set_block_number(now + 1);
            state = crate::BlockResourceState::new(now + 1);
            assert_ok!(state.begin_prepass(budget));
          }
          let recovery_domain = if drain || control_shortfall {
            Domain::ActorBaseEffect
          } else {
            Domain::ActorDrainEffect
          };
          let control_left = limits
            .actor_control()
            .checked_sub(&state.usage().actor_control_used())
            .unwrap();
          let recovered = Actors::execute_cycle_to_cutoff_with_resources(
            Weight::MAX,
            0,
            &mut state,
            limits,
            recovery_domain,
            control_left,
          );
          assert!(!recovered.starved);
          assert_eq!(state.outstanding_reservations(), 0);
          assert!(!state.optional_actor_work_halted());
          let transfers = || {
            System::events()
              .into_iter()
              .filter_map(|record| match record.event {
                RuntimeEvent::Actors(Event::TransferExecuted { actor_id, .. }) => Some(actor_id),
                _ => None,
              })
              .collect::<Vec<_>>()
          };
          assert_eq!(
            transfers(),
            ids,
            "the refused head executes once before its successor"
          );
          assert_eq!(native_balance(&BOB), recipient + 2);
          assert!(!fee_collections().is_empty());
          for id in &ids {
            let run = Actors::actor_run_state(*id).unwrap();
            assert_eq!(run.cursor, u32::from(running) + 1);
            assert_eq!(run.eligible_at, System::block_number() + 1);
            let node = Actors::service_nodes(*id).unwrap();
            assert_eq!(
              node.eligible_from, 2,
              "progress retains the original ring admission"
            );
            assert_eq!(node.last_considered, System::block_number());
          }
          let effect_after = state.usage().actor_effect_used();
          let control_left = limits
            .actor_control()
            .checked_sub(&state.usage().actor_control_used())
            .unwrap();
          Actors::execute_cycle_to_cutoff_with_resources(
            Weight::MAX,
            0,
            &mut state,
            limits,
            recovery_domain,
            control_left,
          );
          assert_eq!(
            transfers(),
            ids,
            "another pass cannot execute a second Step in this block"
          );
          assert_eq!(state.usage().actor_effect_used(), effect_after);
          assert_eq!(state.outstanding_reservations(), 0);
          #[cfg(feature = "try-runtime")]
          assert_ok!(Actors::do_try_state());
        });
      }
    }
  }
}

#[test]
fn paired_service_business_failure_consumes_attempt_and_allows_successor() {
  use crate::BlockResourceDomain as Domain;
  for drain in [false, true] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      set_max_consecutive_failures(3);
      let mut failed_step = transfer_contract_steps(BOB, 1)[0].clone();
      failed_step.on_error = StepErrorPolicy::AbortCycle;
      let first = create_user_with(
        ALICE,
        Mutability::Mutable,
        manual_schedule(),
        None,
        contract_steps_with_step(failed_step),
      );
      let second = create_user_with(
        ALICE,
        Mutability::Mutable,
        manual_schedule(),
        None,
        transfer_contract_steps(ALICE, 1),
      );
      for id in [first, second] {
        fund_native(id, 10_000);
        assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
      }
      System::set_block_number(2);
      System::reset_events();
      clear_fee_collections();
      set_fail_transfer_to(Some(BOB));
      let recipient = native_balance(&BOB);
      let budget = TestBlockResourceBudget::get();
      let limits = budget.limits();
      let mut state = crate::BlockResourceState::new(2);
      assert_ok!(state.begin_prepass(budget));
      if drain {
        assert_ok!(state.open_external_phase());
        assert_ok!(state.begin_drain());
      }
      let pass = Actors::execute_cycle_to_cutoff_with_resources(
        Weight::MAX,
        0,
        &mut state,
        limits,
        if drain {
          Domain::ActorDrainEffect
        } else {
          Domain::ActorBaseEffect
        },
        limits.actor_control(),
      );
      set_fail_transfer_to(None);
      assert!(!pass.starved);
      assert!(!state.optional_actor_work_halted());
      assert_eq!(state.outstanding_reservations(), 0);
      assert_ne!(state.usage().actor_effect_used(), Weight::zero());
      assert_eq!(native_balance(&BOB), recipient);
      assert!(!fee_collections().is_empty());
      let outcomes = System::events()
        .into_iter()
        .filter_map(|record| match record.event {
          RuntimeEvent::Actors(Event::StepFailed {
            actor_id, error, ..
          }) => {
            // DispatchError's static diagnostic string is not SCALE-persisted in events.
            assert!(matches!(error, DispatchError::Other(_)));
            Some((actor_id, false))
          }
          RuntimeEvent::Actors(Event::TransferExecuted { actor_id, .. }) => Some((actor_id, true)),
          _ => None,
        })
        .collect::<Vec<_>>();
      assert_eq!(outcomes, vec![(first, false), (second, true)]);
      for id in [first, second] {
        let hot = Actors::actor_hot(id).unwrap();
        assert_eq!(Actors::active_actor_view(id).unwrap().cycle_nonce, 1);
        assert_eq!(hot.cycle_state, CycleState::Idle);
        assert!(!hot.pending_signal);
        assert!(Actors::actor_run_state(id).is_none());
      }
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn global_fifo_eventually_services_system_actor_after_many_users() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let user_count = 32u32;
    for i in 0..user_count {
      let owner: AccountId = 10_000 + i as AccountId;
      let _ = <Balances as frame::traits::Currency<AccountId>>::deposit_creating(
        &owner,
        TEST_INITIAL_BALANCE,
      );
      let user_id = create_user_with(
        owner,
        Mutability::Mutable,
        timer_schedule(1),
        None,
        inert_contract_steps(),
      );
      fund_native(user_id, 1_000);
    }
    let system_id = create_system_with(ALICE, timer_schedule(1), None, inert_contract_steps());
    // One temporal member per block plus B+1 Service covers the finite due population.
    for block in 2..=u64::from(user_count) + 3 {
      run_canonical_block_at(block, Weight::MAX);
    }
    let system = Actors::active_actor_view(system_id).expect("system Actors exists");
    assert!(
      system.cycle_nonce >= 1,
      "system actor must execute after the finite due population (nonce={})",
      system.cycle_nonce,
    );
  });
}

#[test]
fn global_fifo_services_system_actor_when_it_is_the_only_ready_work() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let system_id = create_system_with(ALICE, timer_schedule(1), None, inert_contract_steps());
    for block in 2..=4 {
      run_canonical_block_at(block, Weight::MAX);
    }
    let system = Actors::active_actor_view(system_id).expect("system Actors exists");
    assert!(system.cycle_nonce >= 1);
  });
}

#[test]
fn enqueue_rolls_back_on_span_occupancy_mismatch() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    crate::ActorReadyTail::<Test>::put(1);
    crate::ActorReadyOccupancy::<Test>::put(0);
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    assert_eq!(
      Actors::try_paged_enqueue(actor_id),
      Err(crate::EnqueueOutcome::CorruptedTopology)
    );

    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
  });
}

#[test]
fn enqueue_rolls_back_on_missing_or_malformed_tail_page() {
  for malformed in [false, true] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let mut actors = Vec::new();
      for _ in 0..33 {
        let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
        assert!(enqueue_latched_actor(actor_id));
        actors.push(actor_id);
      }
      let candidate = create_system_with(BOB, manual_schedule(), None, inert_contract_steps());
      if malformed {
        crate::ActorReadyFrameChunks::<Test>::mutate(1, |maybe_page| {
          maybe_page.as_mut().expect("tail page").truncate(1);
        });
      } else {
        crate::ActorReadyFrameChunks::<Test>::remove(1);
      }
      let events_before = System::events();
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

      assert_eq!(
        Actors::try_paged_enqueue(candidate),
        Err(crate::EnqueueOutcome::CorruptedTopology)
      );

      assert_eq!(System::events(), events_before);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        root_before
      );
      assert_eq!(actors.len(), 33);
    });
  }
}

#[cfg(feature = "try-runtime")]
#[test]
fn canonical_queue_try_state_rejects_a_malformed_page_width() {
  new_test_ext().execute_with(|| {
    crate::ActorReadyFrameChunks::<Test>::insert(
      0,
      BoundedVec::try_from(vec![None]).expect("malformed short page fits"),
    );
    crate::ActorReadyHead::<Test>::put(0);
    crate::ActorReadyTail::<Test>::put(2);
    crate::ActorReadyOccupancy::<Test>::put(0);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn queue_cohort_preflight_ticket_exhaustion_is_read_only() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    // The canonical tail is the sole non-resetting ticket allocator.
    crate::ActorReadyHead::<Test>::put(u64::MAX);
    crate::ActorReadyTail::<Test>::put(u64::MAX);
    crate::ActorReadyOccupancy::<Test>::put(0);

    let actor_before = native_balance(&sovereign_account(actor_id));
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    let mut hot = Actors::actor_hot(actor_id).expect("valid source authority");
    hot.pending_signal = true;
    assert_eq!(
      Actors::preflight_paged_enqueue_cohort_with_authority(vec![(actor_id, hot)]).map(|_| ()),
      Err(crate::EnqueueOutcome::TicketExhausted)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
    assert_eq!(native_balance(&sovereign_account(actor_id)), actor_before);
  });
}

#[test]
fn repeated_trigger_same_block_yields_one_ticket_and_one_execution() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000_000_000_000_000);
    // Two manual triggers in the same block coalesce into one latched pending signal and one
    // generation-bound `Service(Pending)` membership in the single service ring; the post-worker
    // cutoff enforces executions(A, B) <= 1 per block.
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(crate::ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(Actors::service_header().count, 1);
    assert!(
      Actors::actor_hot(actor_id)
        .expect("latched actor")
        .queue_ticket
        .is_none()
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    Actors::execute_cycle(Weight::MAX);
    let started = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter(|record| {
        matches!(
          record.event,
          RuntimeEvent::Actors(Event::CycleStarted { actor_id: id, .. }) if id == actor_id
        )
      })
      .count();
    assert_eq!(started, 1, "exactly one CycleStarted per actor per block");
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("actor")
        .cycle_nonce,
      1
    );
    // The committed prefix publishes exactly one successor `Service(Pending)` for the next block.
    assert!(crate::ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(Actors::service_header().count, 1);
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn repeated_scheduler_pass_same_block_preserves_one_committed_step_turn() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut steps = transfer_contract_steps(BOB, 10).into_inner();
    steps.extend(transfer_contract_steps(BOB, 20).into_inner());
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(steps).expect("two Steps fit"),
    );
    fund_native(actor_id, 1_000_000_000_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let recipient_before = MockAssetOps::balance(&BOB, TestAsset::Native);

    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::execute_cycle(Weight::MAX);
    let first_run = Actors::actor_run_state(actor_id).expect("successor remains live");
    assert_eq!(first_run.cursor, 1);
    assert_eq!(first_run.last_committed_step_block, Some(2));
    assert_eq!(
      MockAssetOps::balance(&BOB, TestAsset::Native),
      recipient_before + 10
    );

    // The committed prefix publishes exactly one successor `Service(Pending)` for the next block.
    assert!(crate::ServiceNodes::<Test>::contains_key(actor_id));

    Actors::execute_cycle(Weight::MAX);
    let same_block_run = Actors::actor_run_state(actor_id).expect("successor remains live");
    assert_eq!(same_block_run.cursor, first_run.cursor);
    assert_eq!(
      same_block_run.last_committed_step_block,
      first_run.last_committed_step_block
    );
    assert!(
      crate::ServiceNodes::<Test>::contains_key(actor_id),
      "same-block refusal preserves exact successor authority"
    );
    assert_eq!(
      MockAssetOps::balance(&BOB, TestAsset::Native),
      recipient_before + 10
    );

    frame_system::Pallet::<Test>::set_block_number(3);
    Actors::execute_cycle(Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(
      MockAssetOps::balance(&BOB, TestAsset::Native),
      recipient_before + 30
    );
  });
}

#[test]
fn simulation_and_scheduler_reject_the_same_protected_fee_floor_boundary() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract_steps = transfer_contract_steps(BOB, 10);
    let contract = user_active_contract(manual_schedule(), None, contract_steps.clone())
      .expect("direct Actor Contract");
    let prefunded = user_prefunding_requirement(&contract_steps);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      contract_steps,
    );
    deplete_user_sovereign(actor_id, prefunded);
    let attempt_fee = Actors::maximum_contract_step_fee(ActorType::User, &contract.steps, 0)
      .expect("current-Step fee is bounded")
      .total_fee;
    let raw_balance = attempt_fee.max(TestMinUserBalance::get());
    fund_native(actor_id, raw_balance.saturating_add(manual_trigger_fee()));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(
      raw_balance >= attempt_fee,
      "raw balance covers the attempt envelope"
    );
    assert!(
      raw_balance.saturating_sub(TestMinUserBalance::get()) < attempt_fee,
      "balance above the protected floor does not cover the attempt envelope",
    );
    let actor_before = Actors::active_actor_view(actor_id).expect("actor before simulation");
    let events_before = System::events();

    // The canonical occurrence is eligible at B+1, so the viability projection must be taken at
    // that eligible block rather than in the publication block.
    frame_system::Pallet::<Test>::set_block_number(2);
    let hot = Actors::actor_hot(actor_id).expect("pending canonical head");
    assert!(hot.pending_signal);
    assert_eq!(hot.cycle_state, CycleState::Idle);
    let result = Actors::simulate_current_contract(
      actor_id,
      ActorType::User,
      Mutability::Mutable,
      contract,
      SimulationMode::FreshCurrentPlan,
      ample_simulation_budget(),
    )
    .expect("terminal viability projects as a closed simulation");
    assert_eq!(
      result.status,
      AttemptDisposition::Closed(CloseReason::CycleAdmissionInsufficient)
    );
    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(System::events(), events_before);

    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::CycleAdmissionInsufficient,
      } if *id == actor_id
    )));
  });
}

#[test]
fn scheduler_retries_manual_continuation_after_cooldown_without_new_signal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let schedule = Schedule {
      trigger: Trigger::manual(),
      cooldown_blocks: 2,
    };
    let actor_id = create_system_with(ALICE, schedule, None, temporary_retry_swap_plan());
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    // The occurrence published at block 1 is served at B+1, so a two-block cooldown makes the
    // retry eligible at block 4.
    assert_eq!(scheduled_wakeup_block(actor_id), Some(4));
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("suspended")
        .unsuccessful_attempts_at_cursor,
      1
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("still suspended")
        .unsuccessful_attempts_at_cursor,
      1
    );

    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(4);
    Actors::on_initialize(4);
    run_prepass();
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("due retry waits for B+1 after Service reentry")
        .unsuccessful_attempts_at_cursor,
      1
    );
    frame_system::Pallet::<Test>::set_block_number(5);
    run_prepass();
    run_idle(Weight::MAX);
    let completed = Actors::active_actor_view(actor_id).expect("actor completes");
    assert_eq!(completed.cycle_nonce, 1);
    assert_eq!(completed.cycle_state, CycleState::Idle);
    assert!(Actors::actor_run_state(actor_id).is_none());
  });
}

#[test]
fn canonical_fifo_executes_global_ticket_order_across_actor_types() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let user_a = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let system_a = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let user_b = create_user_with(
      BOB,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let system_b = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    fund_native(user_a, 1_000_000_000_000_000);
    fund_native(user_b, 1_000_000_000_000_000);

    for (owner, actor_id) in [
      (ALICE, user_a),
      (ALICE, system_a),
      (BOB, user_b),
      (ALICE, system_b),
    ] {
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(owner),
        actor_id
      ));
    }
    // Canonical publication makes every latched Actor ineligible in its own block, so the first
    // service pass runs at B+1. One pass is bounded by `MaxExecutionsPerBlock` and preserves the
    // single global publication order across User and System Actor types.
    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    Actors::execute_cycle(Weight::MAX);

    let started: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(Event::CycleStarted { actor_id, .. }) => Some(actor_id),
        _ => None,
      })
      .collect();
    assert_eq!(started, vec![user_a, system_a, user_b]);
    assert_eq!(
      Actors::active_actor_view(system_b)
        .expect("fourth FIFO actor remains")
        .cycle_nonce,
      0
    );

    frame_system::Pallet::<Test>::set_block_number(3);
    frame_system::Pallet::<Test>::reset_events();
    Actors::execute_cycle(Weight::MAX);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id, .. } if *actor_id == system_b
    )));
  });
}

#[test]
fn canonical_fifo_uses_one_physical_ticket_sequence() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let system_a = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let user_a = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let system_b = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let user_b = create_user_with(
      BOB,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );

    let triggered = [system_a, user_a, system_b, user_b];
    for (owner, actor_id) in [
      (ALICE, system_a),
      (ALICE, user_a),
      (ALICE, system_b),
      (BOB, user_b),
    ] {
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(owner),
        actor_id
      ));
    }

    // Canonical publication gives every latched Actor exactly one generation-bound
    // `Service(Pending)` membership in the single persistent service ring; no legacy ready-frame
    // ticket or scalar allocator is mirrored.
    let header = Actors::service_header();
    assert_eq!(header.count, 4);
    for actor_id in triggered {
      let hot = Actors::actor_hot(actor_id).expect("latched Actor");
      assert!(hot.pending_signal);
      assert!(hot.queue_ticket.is_none());
      assert!(ServiceNodes::<Test>::contains_key(actor_id));
      assert_eq!(
        ActorProcesses::<Test>::get(actor_id).and_then(|process| process.residence),
        Some(ProcessResidence::Service(ServiceResidenceKind::Pending))
      );
    }

    // The ring is one circular physical sequence in publication order.
    let first = header.cursor.expect("non-empty ring cursor");
    let mut order = vec![first.actor_id];
    let mut current = ServiceNodes::<Test>::get(first.actor_id)
      .expect("cursor member")
      .next;
    while current != first {
      order.push(current.actor_id);
      current = ServiceNodes::<Test>::get(current.actor_id)
        .expect("ring member")
        .next;
    }
    assert_eq!(order, triggered);
  });
}

#[test]
fn pipeline_opening_rearms_cadence_from_current_tick() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      timer_schedule(1),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    frame_system::Pallet::<Test>::set_block_number(2);
    service_canonical_temporal_frontiers(2);
    assert!(Actors::actor_hot(actor_id).is_some_and(|hot| hot.trigger_wakeup_pointer.is_none()));
    let bob_before = native_balance(&BOB);

    run_next_idle(Weight::MAX);

    assert_eq!(native_balance(&BOB), bob_before + 10);
    let state = Actors::active_actor_state(actor_id).expect("Actor remains active");
    assert_eq!(state.identity.cycle_nonce, 1);
    assert!(state.hot.trigger_wakeup_pointer.is_some());
  });
}

#[test]
fn run_retry_preserves_independent_external_timer_cadence() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let schedule = Schedule {
      trigger: Trigger::cadenced(100),
      cooldown_blocks: 0,
    };
    let actor_id = create_system_with(ALICE, schedule, None, temporary_retry_swap_plan());
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);

    let cadence_due = scheduled_wakeup_block(actor_id).expect("cadenced wakeup");
    run_canonical_block_at(cadence_due, Weight::MAX);
    run_canonical_block_at(cadence_due + 1, Weight::MAX);

    let hot = Actors::actor_hot(actor_id).expect("suspended cadence actor");
    assert_eq!(
      hot.trigger_wakeup_pointer.map(|pointer| pointer.tick),
      Some(cadence_due + 100)
    );
    // The suspended retry keeps its structural residence in the canonical process/Service carrier
    // while the external cadence keeps its own Trigger deadline, so no legacy queue ticket exists.
    assert!(crate::ActorProcesses::<Test>::contains_key(actor_id));
    assert!(crate::ServiceNodes::<Test>::contains_key(actor_id));
    assert!(crate::TriggerDeadlineHandles::<Test>::contains_key(
      actor_id
    ));
    assert!(hot.queue_ticket.is_none());
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("suspended")
        .unsuccessful_attempts_at_cursor,
      1
    );
  });
}

#[test]
fn pause_and_breaker_gate_scheduler_owned_retry() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    setup_temporary_retry_pool();
    let actor_id = create_system_with(ALICE, manual_schedule(), None, temporary_retry_swap_plan());
    fund_native(actor_id, 100);
    set_temporary_dex_failure(true);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("suspended")
        .unsuccessful_attempts_at_cursor,
      1
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("paused")
        .unsuccessful_attempts_at_cursor,
      1
    );

    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    // Drive exactly one block: the canonical breaker refusal retains placement without the
    // helper's multi-block continuation loop advancing into future rounds.
    Actors::on_initialize(3);
    run_prepass();
    Actors::on_idle(3, Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("breaker gated")
        .unsuccessful_attempts_at_cursor,
      1
    );

    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      false
    ));
    set_temporary_dex_failure(false);
    frame_system::Pallet::<Test>::set_block_number(4);
    Actors::on_initialize(4);
    run_prepass();
    Actors::on_idle(4, Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("completed")
        .cycle_state,
      CycleState::Idle
    );
  });
}

#[test]
fn cancellation_does_not_requeue_an_ignored_busy_manual_signal() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(!Actors::pending_signal(actor_id));
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      inert_contract_steps(),
      crate::CompletionPolicy::Persistent,
    ));
    let cancelled = Actors::active_actor_view(actor_id).expect("cancelled actor remains");
    assert_eq!(cancelled.cycle_state, CycleState::Idle);
    assert_eq!(cancelled.cycle_nonce, 1);
    assert!(!cancelled.pending_signal);
    assert!(cancelled.queue_ticket.is_none());
  });
}

#[test]
fn semantic_control_origins_share_one_queue_churn_clock() {
  new_test_ext().execute_with(|| {
    let plan_id = create_suspended_system_retry(1);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      plan_id,
      inert_contract_steps(),
      crate::CompletionPolicy::Persistent,
    ));
    assert_noop!(
      update_contract_partial!(
        RuntimeOrigin::signed(ALICE),
        plan_id,
        timer_schedule(2),
        None,
      ),
      Error::<Test>::ControlMutationRateLimited
    );

    let policy_id = create_suspended_system_retry(2);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::signed(ALICE),
      policy_id,
      FundingSourcePolicy::AnyVerifiedIngress,
    ));
    assert_noop!(
      Actors::deactivate_actor(RuntimeOrigin::root(), policy_id),
      Error::<Test>::ControlMutationRateLimited
    );

    let schedule_id = create_suspended_system_retry(3);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      schedule_id,
      timer_schedule(2),
      None,
    ));
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::signed(ALICE), schedule_id),
      Error::<Test>::ControlMutationRateLimited
    );

    let cancel_id = create_suspended_system_retry(4);
    assert_ok!(Actors::pause_actor(RuntimeOrigin::root(), cancel_id));
    assert_noop!(
      Actors::cancel_run(RuntimeOrigin::signed(ALICE), cancel_id),
      Error::<Test>::ControlMutationRateLimited
    );

    frame_system::Pallet::<Test>::set_block_number(5);
    let dormant_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None,
    ));
    frame_system::Pallet::<Test>::set_block_number(6);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      dormant_id,
      system_active_contract(manual_schedule(), None, inert_contract_steps())
        .expect("direct Actor Contract"),
    ));
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::root(), dormant_id),
      Error::<Test>::ControlMutationRateLimited
    );
  });
}

#[test]
#[ignore = "10,000-identity production profile; run explicitly in release mode"]
fn maximum_dormant_identity_population_adds_no_idle_scan() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    run_prepass();
    let empty_idle = Actors::on_idle(1, Weight::MAX);
    Actors::on_finalize(1);

    let maximum = <Test as crate::Config>::MaxActorIdentities::get();
    let first_dormant = Actors::next_actor_id();
    for _ in 0..maximum {
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        None,
      ));
    }
    assert_eq!(crate::ActorIdentityCount::<Test>::get(), maximum);
    assert_eq!(crate::ActiveActorCount::<Test>::get(), 0);

    System::set_block_number(2);
    run_prepass();
    let saturated_dormant_idle = Actors::on_idle(2, Weight::MAX);
    Actors::on_finalize(2);
    assert_eq!(saturated_dormant_idle, empty_idle);

    System::set_block_number(3);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      first_dormant,
      system_active_contract(manual_schedule(), None, inert_contract_steps())
        .expect("bounded activation Contract"),
    ));
    assert_eq!(crate::ActiveActorCount::<Test>::get(), 1);
    assert_eq!(crate::ActorIdentityCount::<Test>::get(), maximum);
  });
}

#[test]
fn zero_on_idle_budget_performs_no_storage_or_telemetry_work() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    IdleStarvationState::<Test>::put(IdleStarvationPhase::Alerted {
      consecutive_blocks: 1,
    });
    let event_count = frame_system::Pallet::<Test>::event_count();
    let used = Actors::on_idle(1, Weight::zero());
    assert_eq!(used, Weight::zero());
    assert_eq!(
      IdleStarvationState::<Test>::get(),
      IdleStarvationPhase::Alerted {
        consecutive_blocks: 1,
      }
    );
    assert_eq!(frame_system::Pallet::<Test>::event_count(), event_count);
  });
}

#[test]
fn bounded_idle_scan_preserves_healthy_pass_exit() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let ids = (0..2)
      .map(|_| {
        let id = create_system_with(ALICE, manual_schedule(), None, BoundedVec::default());
        assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
        id
      })
      .collect::<Vec<_>>();
    System::set_block_number(2);
    Actors::execute_cycle(Weight::MAX);
    for id in &ids {
      let state = Actors::active_actor_view(*id).unwrap();
      assert_eq!(state.cycle_nonce, 1);
      assert!(!state.pending_signal);
    }
    TestMaxQueueEntriesScannedPerBlock::set(1);
    System::set_block_number(3);
    let pass = Actors::execute_cycle(Weight::MAX);
    assert!(
      pass.starvation_observed,
      "an admitted bounded scan retains the healthy cap-exit policy"
    );
    assert!(!pass.starved);
    assert_eq!(Actors::service_nodes(ids[0]).unwrap().last_considered, 3);
    assert_eq!(Actors::service_nodes(ids[1]).unwrap().last_considered, 2);
    assert_eq!(
      crate::ActorProcesses::<Test>::get(ids[0])
        .unwrap()
        .last_attempted,
      Some(2)
    );
  });
}

#[test]
fn uninspected_drain_preserves_starvation_evidence() {
  use crate::WeightInfo;
  for remove_head in [false, true] {
    for deficit in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
      new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let id = create_system_with(
          ALICE,
          manual_schedule(),
          None,
          transfer_contract_steps(BOB, 1),
        );
        fund_native(id, 1_000);
        assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
        let threshold = TestMaxIdleStarvationBlocks::get();
        for block in 2..=u64::from(threshold) + 1 {
          System::set_block_number(block);
          run_drain_only(starvation_blocked_budget(id));
        }
        let alerted = IdleStarvationPhase::Alerted {
          consecutive_blocks: threshold,
        };
        assert_eq!(IdleStarvationState::<Test>::get(), alerted);
        let now = u64::from(threshold) + 2;
        System::set_block_number(now);
        if remove_head {
          assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), id));
        }
        let open_external = |block| {
          let mut state = crate::BlockResourceState::new(block);
          assert_ok!(state.begin_prepass(TestBlockResourceBudget::get()));
          assert_ok!(state.open_external_phase());
          crate::CurrentBlockResourceState::<Test>::put(state);
        };
        open_external(now);
        let header = Actors::service_header();
        let events = System::events();
        let fixed = TestWeightInfo::scheduler_on_idle_base()
          .saturating_add(TestWeightInfo::block_resource_finalize());
        let selector = TestWeightInfo::service_round_begin_populated()
          .saturating_add(TestWeightInfo::service_round_probe_eligible());
        assert_eq!(
          Actors::on_idle(now, fixed.saturating_add(selector).saturating_sub(deficit)),
          fixed
        );
        assert_eq!(Actors::service_header(), header);
        assert_eq!(
          IdleStarvationState::<Test>::get(),
          alerted,
          "no discovery means neither an additional starvation block nor recovery"
        );
        assert_eq!(System::events(), events);
        let state = Actors::block_resource_state().unwrap();
        assert_eq!(state.phase(), crate::BlockResourcePhase::Finalizable);
        assert_eq!(state.outstanding_reservations(), 0);
        assert!(!state.optional_actor_work_halted());
        Actors::on_finalize(now);
        System::set_block_number(now + 1);
        open_external(now + 1);
        Actors::on_idle(now + 1, Weight::MAX);
        assert!(
          !IdleStarvationState::<Test>::exists(),
          "paid service or paid absence establishes recovery"
        );
      });
    }
  }
}

#[test]
fn drain_admitted_progress_survives_a_sub_discovery_remainder() {
  use crate::WeightInfo;
  for deficit in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        transfer_contract_steps(BOB, 1),
      );
      fund_native(id, 1_000);
      assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
      System::set_block_number(2);
      let mut state = crate::BlockResourceState::new(2);
      assert_ok!(state.begin_prepass(TestBlockResourceBudget::get()));
      assert_ok!(state.open_external_phase());
      crate::CurrentBlockResourceState::<Test>::put(state);
      let resources = Actors::load_current_step_from_storage(id, 0)
        .unwrap()
        .resources;
      let selector = TestWeightInfo::service_round_begin_populated()
        .saturating_add(TestWeightInfo::service_round_probe_eligible());
      let suffix = TestWeightInfo::service_round_admit_eligible()
        .max(TestWeightInfo::service_member_retire_interior())
        .max(TestWeightInfo::service_member_retire_pair_cursor())
        .max(TestWeightInfo::service_member_retire_singleton());
      let control = selector
        .saturating_add(TestWeightInfo::scheduler_actor_state_probe())
        .saturating_add(resources.control)
        .saturating_add(suffix);
      let fixed = TestWeightInfo::scheduler_on_idle_base()
        .saturating_add(TestWeightInfo::block_resource_finalize());
      let complete = fixed
        .saturating_add(control)
        .saturating_add(resources.effect);
      let before = native_balance(&BOB);
      let consumed = Actors::on_idle(2, complete.saturating_add(selector).saturating_sub(deficit));
      assert_eq!(consumed, complete);
      assert_eq!(native_balance(&BOB), before + 1);
      assert_eq!(Actors::active_actor_view(id).unwrap().cycle_nonce, 1);
      assert!(!IdleStarvationState::<Test>::exists());
      let state = Actors::block_resource_state().unwrap();
      assert_eq!(
        state.usage().actor_control_used(),
        fixed.saturating_add(control)
      );
      assert_eq!(state.usage().actor_effect_used(), resources.effect);
      assert_eq!(state.outstanding_reservations(), 0);
      assert!(!state.optional_actor_work_halted());
      Actors::on_finalize(2);
    });
  }
}

#[test]
fn starvation_emits_observability_event_once_without_control_effects() {
  new_test_ext().execute_with(|| {
    let threshold = TestMaxIdleStarvationBlocks::get();
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
    let queue_ticket = Actors::actor_hot(actor_id)
      .expect("queued actor")
      .queue_ticket;
    assert!(!GlobalCircuitBreaker::<Test>::get());
    // The canonical occurrence published by `manual_trigger` is eligible at B+1, so the first
    // weight-blocked Drain observation is the next block.
    frame_system::Pallet::<Test>::set_block_number(2);
    run_drain_only(starvation_blocked_budget(actor_id));
    assert_eq!(
      IdleStarvationState::<Test>::get(),
      IdleStarvationPhase::Starving {
        consecutive_blocks: 1,
      }
    );
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::IdleStarvationDetected { .. } | Event::IdleStarvationRecovered { .. }
    )));
    for block in 3..=(threshold + 3) {
      frame_system::Pallet::<Test>::set_block_number(block as u64);
      run_drain_only(starvation_blocked_budget(actor_id));
    }
    let detections = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(Event::IdleStarvationDetected { consecutive_blocks }) => {
          Some(consecutive_blocks)
        }
        _ => None,
      })
      .collect::<std::vec::Vec<_>>();
    assert_eq!(detections, vec![threshold]);
    assert_eq!(
      IdleStarvationState::<Test>::get(),
      IdleStarvationPhase::Alerted {
        consecutive_blocks: threshold + 2,
      }
    );
    assert!(
      Actors::active_actor_view(actor_id).is_some(),
      "live head survives"
    );
    assert_eq!(
      Actors::actor_hot(actor_id).expect("live head").queue_ticket,
      queue_ticket,
    );
    assert!(!GlobalCircuitBreaker::<Test>::get());
  });
}

#[test]
fn proof_size_exhaustion_counts_as_idle_starvation() {
  new_test_ext().execute_with(|| {
    let threshold = TestMaxIdleStarvationBlocks::get();
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
    for block in 2..=(threshold + 1) {
      frame_system::Pallet::<Test>::set_block_number(u64::from(block));
      run_drain_only(starvation_blocked_budget(actor_id));
    }
    assert_eq!(
      IdleStarvationState::<Test>::get(),
      IdleStarvationPhase::Alerted {
        consecutive_blocks: threshold,
      }
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::IdleStarvationDetected { consecutive_blocks } if *consecutive_blocks == threshold
    )));
  });
}

#[test]
fn starvation_requires_live_fifo_work_and_clears_without_work() {
  new_test_ext().execute_with(|| {
    let threshold = TestMaxIdleStarvationBlocks::get();
    assert!(!IdleStarvationState::<Test>::exists());
    frame_system::Pallet::<Test>::set_block_number(1);
    run_idle(Weight::MAX);
    assert!(!IdleStarvationState::<Test>::exists());
    // An empty queue with an exhausted budget must never starve: no live FIFO work exists.
    for block in 1..=(threshold + 2) {
      frame_system::Pallet::<Test>::set_block_number(block as u64);
      run_idle(starvation_observation_weight());
    }
    assert!(!IdleStarvationState::<Test>::exists());
  });
}

#[test]
fn starvation_recovery_is_observable_once_and_healthy_idle_stays_sparse() {
  new_test_ext().execute_with(|| {
    let threshold = TestMaxIdleStarvationBlocks::get();
    assert!(!IdleStarvationState::<Test>::exists());
    frame_system::Pallet::<Test>::set_block_number(1);
    run_idle(Weight::MAX);
    assert!(!IdleStarvationState::<Test>::exists());
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
    for block in 2..=(threshold + 1) {
      frame_system::Pallet::<Test>::set_block_number(block as u64);
      run_drain_only(starvation_blocked_budget(actor_id));
    }
    assert_eq!(
      IdleStarvationState::<Test>::get(),
      IdleStarvationPhase::Alerted {
        consecutive_blocks: threshold,
      }
    );
    frame_system::Pallet::<Test>::set_block_number(threshold.saturating_add(2) as u64);
    run_idle(Weight::MAX);
    assert!(!IdleStarvationState::<Test>::exists());
    let recoveries = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(Event::IdleStarvationRecovered { consecutive_blocks }) => {
          Some(consecutive_blocks)
        }
        _ => None,
      })
      .collect::<std::vec::Vec<_>>();
    assert_eq!(recoveries, vec![threshold]);
    frame_system::Pallet::<Test>::set_block_number(threshold.saturating_add(3) as u64);
    run_idle(Weight::MAX);
    assert!(!IdleStarvationState::<Test>::exists());
    assert_eq!(
      frame_system::Pallet::<Test>::events()
        .into_iter()
        .filter(|record| matches!(
          record.event,
          RuntimeEvent::Actors(Event::IdleStarvationRecovered { .. })
        ))
        .count(),
      1
    );
  });
}

#[test]
fn breaker_freezes_starvation_count_without_recovery_event() {
  new_test_ext().execute_with(|| {
    let threshold = TestMaxIdleStarvationBlocks::get();
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
    for block in 2..=(threshold + 1) {
      frame_system::Pallet::<Test>::set_block_number(block as u64);
      run_drain_only(starvation_blocked_budget(actor_id));
    }
    GlobalCircuitBreaker::<Test>::put(true);
    frame_system::Pallet::<Test>::set_block_number(threshold.saturating_add(1) as u64);
    run_drain_only(starvation_blocked_budget(actor_id));
    assert_eq!(
      IdleStarvationState::<Test>::get(),
      IdleStarvationPhase::Alerted {
        consecutive_blocks: threshold,
      }
    );
    let recovery_count = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter(|record| {
        matches!(
          record.event,
          RuntimeEvent::Actors(Event::IdleStarvationRecovered { .. })
        )
      })
      .count();
    frame_system::Pallet::<Test>::set_block_number(threshold.saturating_add(2) as u64);
    run_drain_only(starvation_blocked_budget(actor_id));
    assert_eq!(
      IdleStarvationState::<Test>::get(),
      IdleStarvationPhase::Alerted {
        consecutive_blocks: threshold,
      }
    );
    assert_eq!(
      frame_system::Pallet::<Test>::events()
        .into_iter()
        .filter(|record| matches!(
          record.event,
          RuntimeEvent::Actors(Event::IdleStarvationRecovered { .. })
        ))
        .count(),
      recovery_count
    );
  });
}

#[test]
fn breaker_defers_pipeline_admission_apoptosis_without_partial_events() {
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
    fund_native(actor_id, 60 + manual_trigger_fee());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let instance = Actors::active_actor_view(actor_id).expect("breaker keeps actor pending");
    assert_eq!(instance.cycle_nonce, 0);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. }
        | Event::CycleSummary { actor_id: id, .. }
        | Event::ActorClosed { actor_id: id, .. }
        if *id == actor_id
    )));
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      false
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::CycleAdmissionInsufficient,
      } if *id == actor_id
    )));
  });
}

#[test]
fn breaker_defers_scheduler_owned_window_expiry_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      transfer_contract_steps(BOB, 1),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    frame_system::Pallet::<Test>::set_block_number(102);
    frame_system::Pallet::<Test>::reset_events();
    let _ = Actors::execute_cycle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_some());
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActorClosed { actor_id: id, .. } if *id == actor_id
    )));
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      false
    ));
    frame_system::Pallet::<Test>::set_block_number(103);
    frame_system::Pallet::<Test>::reset_events();
    let _ = Actors::execute_cycle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
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
fn at_time_occurrence_charges_once_consumes_deadline_and_latches_readiness() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      at_time_schedule(1),
      None,
      inert_contract_steps(),
    );
    clear_fee_collections();

    frame_system::Pallet::<Test>::set_block_number(2);
    service_canonical_temporal_frontiers(2);

    assert_eq!(fee_collections(), vec![at_time_trigger_fee()]);
    let hot = Actors::actor_hot(actor_id).expect("AtTime Actor remains active");
    assert!(hot.pending_signal);
    assert!(hot.queue_ticket.is_none());
    assert!(hot.trigger_wakeup_pointer.is_none());
    assert!(matches!(
      hot.trigger_runtime_state,
      TriggerRuntimeState::AtTime { consumed: true, .. }
    ));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed {
        actor_id: id,
        trigger_family: TriggerFamily::AtTime,
        ..
      } if *id == actor_id
    )));

    frame_system::Pallet::<Test>::set_block_number(20);
    service_canonical_temporal_frontiers(20);
    assert_eq!(fee_collections(), vec![at_time_trigger_fee()]);
  });
}

#[test]
fn temporal_occurrence_refuses_selector_and_schedule_mismatched_wake_qualification() {
  for (schedule, mismatched_trigger, mismatched_window) in [
    (at_time_schedule(1), RuntimeTrigger::at_time(2), None),
    (timer_schedule(1), RuntimeTrigger::cadenced(2), None),
    (
      at_time_schedule(1),
      RuntimeTrigger::at_time(1),
      Some(crate::ScheduleWindow { start: 2, end: 20 }),
    ),
  ] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, schedule, None, inert_contract_steps());
      let (state, admission, loaded_step) =
        Actors::load_frame_actor_service_state(actor_id).expect("temporal authority exists");
      let replacement = crate::ActorAdmissionCertificate::new(
        admission.semantic_contract_id,
        admission.body_commitment,
        mismatched_trigger.wake_qualification(&mismatched_window),
        admission.runtime_actor_semantics_version,
        admission.production_weight_identity,
        admission.body_geometry_version,
        admission.configured_bounds_commitment,
        admission.maximum_lifecycle_weight,
      );
      let hot_before = Actors::actor_hot(actor_id).expect("temporal Actor exists");
      let events_before = System::events();
      let balances_before = (native_balance(&ALICE), native_balance(&BOB));

      assert_eq!(
        Actors::process_due_temporal_occurrence_loaded(
          actor_id,
          state,
          replacement,
          loaded_step,
          2,
        ),
        Err(DispatchError::Other(
          "temporal wake qualification is corrupt"
        )),
      );
      assert_eq!(Actors::actor_hot(actor_id), Some(hot_before));
      assert_eq!(System::events(), events_before);
      assert_eq!(
        (native_balance(&ALICE), native_balance(&BOB)),
        balances_before
      );
    });
  }
}

#[test]
fn latched_at_time_replacement_does_not_retrigger_running_pipeline() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let steps = BoundedVec::try_from(
      (0..6)
        .map(|_| {
          make_step(Task::Transfer {
            to: BOB,
            asset: TestAsset::Native,
            amount: AmountResolution::Fixed(1),
          })
        })
        .collect::<Vec<_>>(),
    )
    .expect("six-Step Contract fits");
    let actor_id = create_user_with(ALICE, Mutability::Mutable, manual_schedule(), None, steps);
    fund_native(actor_id, 1_000_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    // The latched manual occurrence survives the schedule replacement and opens the Pipeline at
    // its B+1 eligible block, retaining the replacement AtTime deadline for the next tick.
    let mut replacement = Actors::actor_contract(actor_id).expect("admitted Contract");
    replacement.trigger = RuntimeTrigger::at_time(2);
    assert_ok!(Actors::update_contract(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      replacement
    ));
    run_next_idle(Weight::MAX);
    assert!(
      crate::TriggerDeadlineHandles::<Test>::get(actor_id).is_none(),
      "opening a preserved latch releases the replacement AtTime one-shot deadline"
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
    let run_before = ActorRunStateStore::<Test>::get(actor_id).expect("Pipeline is Running");
    clear_fee_collections();
    System::reset_events();

    // A later temporal service pass must observe no leftover one-shot occurrence and must not
    // re-trigger the Running Pipeline.
    frame_system::Pallet::<Test>::set_block_number(4);
    service_canonical_temporal_frontiers(4);

    assert!(fee_collections().is_empty());
    let hot = Actors::actor_hot(actor_id).expect("busy AtTime Actor remains active");
    assert_eq!(hot.cycle_state, CycleState::Running);
    assert!(!hot.pending_signal);
    assert!(hot.trigger_wakeup_pointer.is_none());
    assert!(matches!(
      hot.trigger_runtime_state,
      TriggerRuntimeState::AtTime { consumed: true, .. }
    ));
    let run_after = ActorRunStateStore::<Test>::get(actor_id).expect("Pipeline remains Running");
    assert_eq!(run_after.cursor, run_before.cursor);
    assert_eq!(run_after.cycle_nonce, run_before.cycle_nonce);
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn underfunded_at_time_occurrence_selects_prepaid_custody_neutral_apoptosis() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Immutable,
      at_time_schedule(1),
      None,
      inert_contract_steps(),
    );
    let sovereign = sovereign_account(actor_id);
    let balance = native_balance(&sovereign);
    deplete_user_sovereign(actor_id, balance - TestMinUserBalance::get());
    let custody_before = native_balance(&sovereign);
    clear_fee_collections();

    frame_system::Pallet::<Test>::set_block_number(2);
    service_canonical_temporal_frontiers(2);

    assert!(fee_collections().is_empty());
    assert!(!Actors::active_actor_exists(actor_id));
    assert!(!ActorProcesses::<Test>::contains_key(actor_id));
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert!(!crate::DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
      actor_id
    ));
    assert_eq!(native_balance(&sovereign), custody_before);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::TriggerAdmissionInsufficient,
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_underfunded_at_time_closes_from_consumed_wakeup_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Immutable,
      at_time_schedule(1),
      None,
      inert_contract_steps(),
    );
    let sovereign = sovereign_account(actor_id);
    let balance = native_balance(&sovereign);
    deplete_user_sovereign(actor_id, balance - TestMinUserBalance::get());
    let custody_before = native_balance(&sovereign);
    clear_fee_collections();

    frame_system::Pallet::<Test>::set_block_number(2);
    service_canonical_temporal_frontiers(2);

    assert!(fee_collections().is_empty());
    assert!(!Actors::active_actor_exists(actor_id));
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!crate::ActorStateHolds::<Test>::contains_key(actor_id));
    assert_eq!(native_balance(&sovereign), custody_before);
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(!Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::TriggerAdmissionInsufficient,
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn frame_only_zero_step_at_time_uses_only_canonical_control() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, at_time_schedule(1), None, BoundedVec::default());

    // B+1 canonical temporal service: block 2 processes the due AtTime deadline and publishes the
    // Pending Service member; block 3 executes the zero-Step cycle. Canonical publication leaves no
    // legacy control locator or scalar hot cell behind.
    run_next_idle(Weight::MAX);
    run_next_idle(Weight::MAX);

    let state = Actors::active_actor_state(actor_id).expect("AtTime successor remains active");
    assert_eq!(state.identity.cycle_nonce, 1);
    assert!(matches!(
      state.hot.trigger_runtime_state,
      TriggerRuntimeState::AtTime { consumed: true, .. }
    ));
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_none());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn temporal_collection_failure_preserves_exact_source_and_retries_once() {
  for schedule in [at_time_schedule(1), timer_schedule(1)] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let one_shot = matches!(schedule.trigger, Trigger::AtTime { .. });
      let fee = if one_shot {
        at_time_trigger_fee()
      } else {
        cadenced_trigger_fee()
      };
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        schedule,
        None,
        inert_contract_steps(),
      );
      let sovereign = sovereign_account(actor_id);
      let balance = native_balance(&sovereign);
      let sink_balance = native_balance(&TestFeeSink::get());
      let source = crate::TriggerDeadlineHandles::<Test>::get(actor_id).unwrap();
      let hot = Actors::actor_hot(actor_id).unwrap();
      System::set_block_number(2);
      let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      set_fail_fee_sink_transfer(true);
      let mut meter = WeightMeter::with_limit(Weight::MAX);
      assert_eq!(
        Actors::process_next_due_tick_deadline(&mut meter, 2),
        Err(crate::DependencyReviewWorkerError::TemporalOccurrence)
      );
      set_fail_fee_sink_transfer(false);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        root,
        "collector failure restores extraction, cadence rearm, holds and readiness together"
      );
      assert_eq!(
        crate::TriggerDeadlineHandles::<Test>::get(actor_id),
        Some(source)
      );
      assert_eq!(Actors::actor_hot(actor_id), Some(hot));
      assert_eq!(native_balance(&sovereign), balance);
      assert_eq!(native_balance(&TestFeeSink::get()), sink_balance);
      clear_fee_collections();
      assert_eq!(
        Actors::process_next_due_tick_deadline(&mut meter, 2),
        Ok(crate::DueTickDeadlineMutation::TemporalTriggerProcessed(
          source.actor
        ))
      );
      assert_eq!(fee_collections(), vec![fee]);
      assert_eq!(native_balance(&sovereign), balance - fee);
      assert_eq!(native_balance(&TestFeeSink::get()), sink_balance + fee);
      let hot = Actors::actor_hot(actor_id).unwrap();
      assert!(hot.pending_signal && hot.trigger_wakeup_pointer.is_none());
      if one_shot {
        assert!(matches!(
          hot.trigger_runtime_state,
          TriggerRuntimeState::AtTime { consumed: true, .. }
        ));
      }
      assert_eq!(
        Actors::process_next_due_tick_deadline(&mut meter, 2),
        Err(crate::DependencyReviewWorkerError::Deadline(
          crate::DeadlineMutationError::MemberMissing
        ))
      );
      assert_eq!(fee_collections(), vec![fee]);
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn immutable_zero_step_at_time_closes_at_authored_cycle_nonce() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let steps = crate::ContractSteps::<Test>::default();
    prefund_active_user_creation(ALICE, &steps);
    let mut contract = user_active_contract(at_time_schedule(1), None, steps)
      .expect("direct zero-Step Actor Contract");
    contract.auto_close_at_cycle_nonce = Some(1);
    let actor_id = Actors::next_actor_id();
    let owner_before = native_balance(&ALICE);
    let sink_before = native_balance(&TestFeeSink::get());
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Immutable,
      Some(contract),
    ));
    assert_eq!(
      native_balance(&ALICE),
      owner_before
        .saturating_sub(TestActorCreationFee::get())
        .saturating_sub(actor_state_hold_total(actor_id))
    );
    assert_eq!(
      native_balance(&TestFeeSink::get()),
      sink_before.saturating_add(TestActorCreationFee::get())
    );
    let slot = Actors::active_actor_view(actor_id)
      .and_then(|actor| actor.actor_class.owner_slot())
      .expect("immutable User slot exists");
    let sovereign = Actors::sovereign_account_id(&ALICE, slot);
    let residual_asset = TestAsset::Local(9);
    set_asset_balance(&sovereign, residual_asset, 919);
    assert_noop!(
      Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ImmutableActor
    );
    clear_fee_collections();

    frame_system::Pallet::<Test>::set_block_number(2);
    service_canonical_temporal_frontiers(2);
    frame_system::Pallet::<Test>::set_block_number(3);
    Actors::execute_cycle(Weight::MAX);

    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(asset_balance(&sovereign, residual_asset), 919);
    assert_eq!(
      fee_collections(),
      vec![
        at_time_trigger_fee(),
        pipeline_opening_fee(&crate::ContractSteps::<Test>::default()),
      ]
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::AutoCloseNonceReached,
      } if *id == actor_id
    )));
    let fresh_id = create_user_with_slot(
      ALICE,
      slot,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    assert_eq!(sovereign_account(fresh_id), sovereign);
    assert_eq!(asset_balance(&sovereign, residual_asset), 919);
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn uninitialized_genesis_cadence_reanchors_canonically() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(1), None, inert_contract_steps());
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    mutate_actor_hot_coherent(actor_id, |hot| {
      hot.trigger_runtime_state = TriggerRuntimeState::Cadenced { anchor_tick: None };
    });

    run_canonical_block_at(100, Weight::MAX);

    let (identity, hot, _) = Actors::load_control_authority_with_authority(actor_id)
      .expect("canonical semantic authority exists");
    assert_eq!(identity.cycle_nonce, 0);
    assert!(!hot.pending_signal);
    assert!(matches!(
      hot.trigger_runtime_state,
      TriggerRuntimeState::Cadenced {
        anchor_tick: Some(100)
      }
    ));
    assert_eq!(
      hot.trigger_wakeup_pointer.map(|pointer| pointer.tick),
      Some(101)
    );
    assert!(ActorIdentities::<Test>::get(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!crate::ActorUnsignaledControlCells::<Test>::contains_key(
      actor_id
    ));
    assert_eq!(
      crate::TriggerDeadlineHandles::<Test>::get(actor_id).map(|handle| handle.key),
      Some(crate::WakeupKey::Tick(101))
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn temporal_bootstrap_rearm_respects_independent_resource_dimensions() {
  for schedule in [at_time_schedule(1), timer_schedule(1)] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let actor_id = create_system_with(ALICE, schedule.clone(), None, inert_contract_steps());
      mutate_actor_hot_coherent(actor_id, |hot| {
        hot.trigger_runtime_state = match schedule.trigger {
          Trigger::AtTime { .. } => TriggerRuntimeState::AtTime {
            anchor_tick: None,
            consumed: false,
          },
          Trigger::Cadenced { .. } => TriggerRuntimeState::Cadenced { anchor_tick: None },
          _ => unreachable!("fixture is temporal"),
        };
      });
      let source = crate::TriggerDeadlineHandles::<Test>::get(actor_id).unwrap();
      let process = ActorProcesses::<Test>::get(actor_id).unwrap().encode();
      let identity = Actors::actor_identity(actor_id).unwrap();
      System::set_block_number(100);
      System::reset_events();
      clear_fee_collections();
      let selector = <TestWeightInfo as crate::WeightInfo>::classify_due_tick_deadline();
      let branch = <TestWeightInfo as crate::WeightInfo>::at_time_trigger_occurrence()
        .max(<TestWeightInfo as crate::WeightInfo>::cadenced_trigger_occurrence());
      let complete = selector.saturating_add(branch);
      let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      for limit in [
        Weight::from_parts(complete.ref_time() - 1, u64::MAX),
        Weight::from_parts(u64::MAX, complete.proof_size() - 1),
      ] {
        let mut meter = WeightMeter::with_limit(limit);
        assert_eq!(
          Actors::process_next_due_tick_deadline(&mut meter, 100),
          Err(crate::DependencyReviewWorkerError::InsufficientWeight)
        );
        assert_eq!(meter.consumed(), selector);
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1,),
          root
        );
        assert_eq!(
          crate::TriggerDeadlineHandles::<Test>::get(actor_id),
          Some(source)
        );
      }

      let mut meter = WeightMeter::with_limit(complete);
      assert_eq!(
        Actors::process_next_due_tick_deadline(&mut meter, 100),
        Ok(crate::DueTickDeadlineMutation::TemporalTriggerProcessed(
          source.actor
        ))
      );
      assert_eq!(meter.consumed(), complete);
      let hot = Actors::actor_hot(actor_id).unwrap();
      assert!(!hot.pending_signal);
      assert_eq!(
        hot.trigger_wakeup_pointer.map(|pointer| pointer.tick),
        Some(101)
      );
      assert!(matches!(
        hot.trigger_runtime_state,
        TriggerRuntimeState::AtTime {
          anchor_tick: Some(100),
          consumed: false
        } | TriggerRuntimeState::Cadenced {
          anchor_tick: Some(100)
        }
      ));
      assert_eq!(
        ActorProcesses::<Test>::get(actor_id).unwrap().encode(),
        process
      );
      assert_eq!(Actors::actor_identity(actor_id), Some(identity));
      assert!(fee_collections().is_empty());
      assert!(System::events().is_empty());
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn uninitialized_genesis_cadence_reanchors_in_benchmark_fixture() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(1), None, inert_contract_steps());
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    mutate_actor_hot_coherent(actor_id, |hot| {
      hot.trigger_runtime_state = TriggerRuntimeState::Cadenced { anchor_tick: None };
    });

    run_canonical_block_at(100, Weight::MAX);

    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(instance.cycle_nonce, 0);
    assert!(!instance.pending_signal);
    assert_eq!(instance.temporal_anchor_tick, Some(100));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(101));
  });
}

#[test]
fn cadenced_latch_disables_detection_until_pipeline_opening() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      timer_schedule(1),
      None,
      inert_contract_steps(),
    );
    clear_fee_collections();

    frame_system::Pallet::<Test>::set_block_number(2);
    service_canonical_temporal_frontiers(2);
    let first = Actors::actor_hot(actor_id).expect("Cadenced Actor remains active");
    assert!(first.pending_signal);
    assert!(first.queue_ticket.is_none());
    assert!(first.trigger_wakeup_pointer.is_none());
    assert_eq!(
      crate::ActorProcesses::<Test>::get(actor_id).and_then(|process| process.residence),
      Some(crate::ProcessResidence::Service(
        crate::ServiceResidenceKind::Pending
      ))
    );

    frame_system::Pallet::<Test>::set_block_number(3);
    service_canonical_temporal_frontiers(3);

    let fee = cadenced_trigger_fee();
    assert_eq!(fee_collections(), vec![fee]);
    let second = Actors::actor_hot(actor_id).expect("Cadenced Actor remains active");
    assert!(second.pending_signal);
    assert!(second.queue_ticket.is_none());
    assert!(second.trigger_wakeup_pointer.is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed {
        actor_id: id,
        trigger_family: TriggerFamily::Cadenced,
        ..
      } if *id == actor_id
    )));
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn at_time_occurrence_uses_primary_pending_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, at_time_schedule(1), None, inert_contract_steps());
    assert!(!Actors::pending_signal(actor_id));

    frame_system::Pallet::<Test>::set_block_number(2);
    service_canonical_temporal_frontiers(2);

    let state = Actors::active_actor_state(actor_id).expect("AtTime pending authority exists");
    let frame_hot = state.hot;
    assert!(frame_hot.pending_signal);
    assert!(
      matches!(
        frame_hot.trigger_runtime_state,
        TriggerRuntimeState::AtTime { consumed: true, .. }
      ),
      "{:?}",
      frame_hot.trigger_runtime_state
    );
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(Actors::actor_control_cell(actor_id).is_none());
    let projected_hot = Actors::actor_hot(actor_id).expect("canonical hot projection exists");
    assert!(projected_hot.pending_signal);
    assert_eq!(
      projected_hot.trigger_runtime_state,
      frame_hot.trigger_runtime_state
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed {
        actor_id: id,
        trigger_family: TriggerFamily::AtTime,
        ..
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn cadenced_occurrence_uses_primary_pending_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(1), None, inert_contract_steps());
    assert!(!Actors::pending_signal(actor_id));

    frame_system::Pallet::<Test>::set_block_number(2);
    service_canonical_temporal_frontiers(2);

    let state = Actors::active_actor_state(actor_id).expect("Cadenced pending authority exists");
    let frame_hot = state.hot;
    assert!(frame_hot.pending_signal);
    assert!(matches!(
      frame_hot.trigger_runtime_state,
      TriggerRuntimeState::Cadenced {
        anchor_tick: Some(1)
      }
    ));
    assert!(frame_hot.trigger_wakeup_pointer.is_none());
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(Actors::actor_control_cell(actor_id).is_none());
    let projected_hot = Actors::actor_hot(actor_id).expect("canonical hot projection exists");
    assert_eq!(projected_hot.pending_signal, frame_hot.pending_signal);
    assert_eq!(
      projected_hot.trigger_runtime_state,
      frame_hot.trigger_runtime_state
    );
    assert_eq!(
      projected_hot.trigger_wakeup_pointer,
      frame_hot.trigger_wakeup_pointer
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed {
        actor_id: id,
        trigger_family: TriggerFamily::Cadenced,
        ..
      } if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn cadenced_rearm_uses_frozen_opening_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(1), None, inert_contract_steps());
    frame_system::Pallet::<Test>::set_block_number(2);
    service_canonical_temporal_frontiers(2);
    assert!(Actors::pending_signal(actor_id));
    // Opening happens at B+1, so the next cadence frontier starts strictly after that tick.
    run_next_idle(Weight::MAX);

    let state = Actors::active_actor_state(actor_id).expect("rearmed Cadenced state is active");
    assert_eq!(state.identity.cycle_nonce, 1);
    assert!(!state.hot.pending_signal);
    assert!(state.hot.queue_ticket.is_none());
    assert!(matches!(
      state.hot.trigger_runtime_state,
      TriggerRuntimeState::Cadenced {
        anchor_tick: Some(1)
      }
    ));
    assert_eq!(
      state.hot.trigger_wakeup_pointer.map(|pointer| pointer.tick),
      Some(4)
    );
    let projected_hot = Actors::actor_hot(actor_id).expect("canonical hot projection exists");
    assert_eq!(
      projected_hot.trigger_runtime_state,
      state.hot.trigger_runtime_state
    );
    assert_eq!(
      projected_hot.trigger_wakeup_pointer,
      state.hot.trigger_wakeup_pointer
    );
    // The rearm is owned by the canonical Trigger deadline carrier, not the legacy waiting
    // reference substrate that production `on_idle` no longer drains for temporal triggers.
    let handle = Actors::trigger_deadline_handles(actor_id)
      .expect("canonical Trigger deadline member is registered");
    assert_eq!(handle.key, WakeupKey::Tick(4));
    assert_eq!(
      u32::from(handle.slot),
      projected_hot
        .trigger_wakeup_pointer
        .expect("projected pointer exists")
        .slot
    );
    assert!(!ActorControlLocators::<Test>::contains_key(actor_id));
    assert_eq!(
      crate::ActorWaitingOccupancies::<Test>::get(WakeupKey::Tick(4)),
      0
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

fn prepare_busy_cadenced_actor(actor_type: ActorType, failed_attempts: u32) -> ActorId {
  System::set_block_number(1);
  let mut steps = BoundedVec::try_from(
    (0..6)
      .map(|_| {
        make_step(Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(1),
        })
      })
      .collect::<Vec<_>>(),
  )
  .expect("six-Step Contract fits");
  if failed_attempts > 0 {
    steps[0] = StepOf::<Test> {
      precondition: None,
      task: Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(u128::from(u64::MAX)),
      },
      on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
    };
  }
  let actor_id = match actor_type {
    ActorType::User => create_user_with(ALICE, Mutability::Mutable, timer_schedule(5), None, steps),
    ActorType::System => create_system_with(ALICE, timer_schedule(5), None, steps),
  };
  fund_native(actor_id, 1_000_000);
  System::set_block_number(6);
  service_canonical_temporal_frontiers(6);
  System::set_block_number(7);
  Actors::execute_cycle(Weight::MAX);
  if failed_attempts == 2 {
    System::set_block_number(8);
    Actors::execute_cycle(Weight::MAX);
  }
  let run = Actors::actor_run_state(actor_id).expect("ordinary Opening retains a Run");
  assert_eq!(run.unsuccessful_attempts_at_cursor, failed_attempts);
  let hot = Actors::actor_hot(actor_id).unwrap();
  assert_eq!(
    hot.cycle_state,
    if failed_attempts == 0 {
      CycleState::Running
    } else {
      CycleState::Suspended
    }
  );
  assert!(!hot.pending_signal);
  assert_eq!(
    crate::TriggerDeadlineHandles::<Test>::get(actor_id)
      .unwrap()
      .key,
    WakeupKey::Tick(11)
  );
  let residence = ActorProcesses::<Test>::get(actor_id).unwrap().residence;
  if failed_attempts == 2 {
    assert!(matches!(
      residence,
      Some(ProcessResidence::Deadline {
        key: WakeupKey::Block(10),
        ..
      })
    ));
  } else {
    assert_eq!(
      residence,
      Some(ProcessResidence::Service(ServiceResidenceKind::Live))
    );
  }
  #[cfg(feature = "try-runtime")]
  Actors::do_try_state()
    .unwrap_or_else(|error| panic!("busy {actor_type:?}/{failed_attempts}: {error:?}"));
  actor_id
}

#[test]
fn pending_service_opening_promotes_in_place_and_rejects_stale_kind() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let mut actors = Vec::new();
    for actor_type in [ActorType::User, ActorType::System, ActorType::User] {
      let steps = BoundedVec::try_from(vec![
        make_step(Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(1),
        });
        2
      ])
      .unwrap();
      let actor_id = match actor_type {
        ActorType::User => {
          create_user_with(ALICE, Mutability::Mutable, manual_schedule(), None, steps)
        }
        ActorType::System => create_system_with(ALICE, manual_schedule(), None, steps),
      };
      fund_native(actor_id, 1_000_000);
      let origin = if actor_type == ActorType::User {
        RuntimeOrigin::signed(ALICE)
      } else {
        RuntimeOrigin::root()
      };
      assert_ok!(Actors::manual_trigger(origin, actor_id));
      actors.push((actor_id, ServiceNodes::<Test>::get(actor_id).unwrap()));
    }
    // Capture topology only after all admissions have established peer links.
    for (actor_id, node) in &mut actors {
      *node = ServiceNodes::<Test>::get(actor_id).unwrap();
    }
    System::set_block_number(2);
    Actors::execute_cycle(Weight::MAX);
    for (actor_id, before) in actors {
      assert_eq!(before.kind, ServiceResidenceKind::Pending);
      let after = ServiceNodes::<Test>::get(actor_id).unwrap();
      assert_eq!(after.kind, ServiceResidenceKind::Live);
      assert_eq!(
        (
          after.generation,
          after.previous,
          after.next,
          after.eligible_from
        ),
        (
          before.generation,
          before.previous,
          before.next,
          before.eligible_from
        )
      );
      assert_eq!(
        ActorProcesses::<Test>::get(actor_id).unwrap().residence,
        Some(ProcessResidence::Service(ServiceResidenceKind::Live))
      );
      let state = Actors::active_actor_state(actor_id).unwrap();
      assert_eq!(state.hot.cycle_state, CycleState::Running);
      assert_eq!(state.run_state.as_ref().unwrap().cursor, 1);
      let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      assert!(
        Actors::try_store_service_control_state(
          Actors::load_actor_ref(actor_id).unwrap(),
          ServiceResidenceKind::Pending,
          state.identity,
          state.hot,
        )
        .is_err()
      );
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        root
      );
    }
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn pending_service_opening_hold_refusal_restores_readiness_and_effects() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      });
      2
    ])
    .unwrap();
    let actor_id = create_user_with(ALICE, Mutability::Mutable, timer_schedule(1), None, steps);
    fund_native(actor_id, 1_000_000);
    System::set_block_number(2);
    service_canonical_temporal_frontiers(2);
    let before = crate::ActorSemanticStates::<Test>::get(actor_id);
    let node = ServiceNodes::<Test>::get(actor_id).unwrap();
    let hold = actor_state_hold_total(actor_id);
    let custody = (
      native_balance(&sovereign_account(actor_id)),
      native_balance(&BOB),
      native_balance(&TestFeeSink::get()),
    );
    let owner_free = Balances::free_balance(ALICE);
    assert_ok!(Balances::force_set_balance(
      RuntimeOrigin::root(),
      ALICE,
      <Test as polkadot_sdk::pallet_balances::Config>::ExistentialDeposit::get()
    ));
    System::set_block_number(3);
    Actors::execute_cycle(Weight::MAX);
    assert_eq!(crate::ActorSemanticStates::<Test>::get(actor_id), before);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
      actor_id
    ));
    assert_eq!(
      ServiceNodes::<Test>::get(actor_id).unwrap().kind,
      ServiceResidenceKind::Pending
    );
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).unwrap().residence,
      Some(ProcessResidence::Service(ServiceResidenceKind::Pending))
    );
    assert_eq!(
      (
        native_balance(&sovereign_account(actor_id)),
        native_balance(&BOB),
        native_balance(&TestFeeSink::get())
      ),
      custody
    );
    assert_eq!(actor_state_hold_total(actor_id), hold);
    assert_ok!(Balances::force_set_balance(
      RuntimeOrigin::root(),
      ALICE,
      owner_free
    ));
    System::set_block_number(4);
    Actors::execute_cycle(Weight::MAX);
    assert_eq!(Actors::actor_run_state(actor_id).unwrap().cursor, 1);
    let after = ServiceNodes::<Test>::get(actor_id).unwrap();
    assert_eq!(after.kind, ServiceResidenceKind::Live);
    assert_eq!(
      (
        after.generation,
        after.previous,
        after.next,
        after.eligible_from
      ),
      (
        node.generation,
        node.previous,
        node.next,
        node.eligible_from
      )
    );
    assert!(
      actor_state_hold_total(actor_id) > hold,
      "Opening holds the newly rearmed cadence detector"
    );
    assert_eq!(
      native_balance(&BOB),
      custody.1 + 1,
      "the refused effect commits only once on recovery"
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn cadenced_occurrence_wakes_parked_balance_and_removes_review_deadline() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      timer_schedule(5),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let mut contract = Actors::actor_contract(actor_id).unwrap();
    contract.parked_balance_activation = Some(
      crate::ParkedBalanceActivationOf::<Test>::try_from_rules(vec![crate::ParkedBalanceRule {
        asset: TestAsset::Local(9),
        authored_min_delta: 100,
      }])
      .unwrap(),
    );
    assert_ok!(Actors::update_contract(
      RuntimeOrigin::signed(ALICE),
      actor_id,
      contract,
    ));
    fund_native(actor_id, 1_000_000_000_000_000);
    let actor = Actors::load_actor_ref(actor_id).unwrap();
    System::set_block_number(6);
    service_canonical_temporal_frontiers(6);
    System::set_block_number(7);
    Actors::execute_cycle(Weight::MAX);
    let evidence = match ActorProcesses::<Test>::get(actor_id).unwrap().residence {
      Some(ProcessResidence::Parked(evidence)) => evidence,
      residence => panic!("completed cadence must be Parked, got {residence:?}"),
    };
    assert_eq!(evidence.review_at, Some(8));
    assert_eq!(
      crate::TriggerDeadlineHandles::<Test>::get(actor_id)
        .unwrap()
        .key,
      WakeupKey::Tick(11)
    );
    assert_eq!(
      crate::DeadlineHandles::<Test>::get(actor_id).unwrap().key,
      WakeupKey::Block(8)
    );
    assert!(
      Actors::actor_hot(actor_id)
        .unwrap()
        .wakeup_pointer
        .is_none()
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());

    System::set_block_number(8);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    assert!(matches!(Actors::process_next_due_block_deadline(
      &mut meter, ServiceResidenceKind::Pending, 8, Some(WakeupKey::Block(100)),
    ), Ok(crate::DueBlockDeadlineMutation::ReviewProcessed(
      processed, crate::DependencyReviewMutation::Rearmed(_),
    )) if processed == actor));
    assert!(matches!(
      ActorProcesses::<Test>::get(actor_id).and_then(|process| process.residence),
      Some(ProcessResidence::Parked(_))
    ));
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());

    System::set_block_number(11);
    let trigger_source = crate::TriggerDeadlineHandles::<Test>::get(actor_id).unwrap();
    let review_source = crate::DeadlineHandles::<Test>::get(actor_id).unwrap();
    let sovereign = sovereign_account(actor_id);
    let balances = (
      native_balance(&sovereign),
      native_balance(&TestFeeSink::get()),
    );
    let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    clear_fee_collections();
    set_fail_fee_sink_transfer(true);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    assert_eq!(
      Actors::process_next_due_tick_deadline(&mut meter, 11),
      Err(crate::DependencyReviewWorkerError::TemporalOccurrence)
    );
    set_fail_fee_sink_transfer(false);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1,),
      root,
      "late collector refusal restores cadence and Park authority"
    );
    assert_eq!(
      crate::TriggerDeadlineHandles::<Test>::get(actor_id),
      Some(trigger_source)
    );
    assert_eq!(
      crate::DeadlineHandles::<Test>::get(actor_id),
      Some(review_source)
    );
    assert_eq!(
      (
        native_balance(&sovereign),
        native_balance(&TestFeeSink::get())
      ),
      balances
    );
    clear_fee_collections();
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    assert_eq!(
      Actors::process_next_due_tick_deadline(&mut meter, 11),
      Ok(crate::DueTickDeadlineMutation::TemporalTriggerProcessed(
        actor
      ))
    );
    assert!(!crate::DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
      actor_id
    ));
    assert!(!crate::ParkedBalanceEpisodes::<Test>::contains_key(
      actor_id
    ));
    assert!(!crate::PendingCheckOwners::<Test>::contains_key(actor_id));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).unwrap().residence,
      Some(ProcessResidence::Service(ServiceResidenceKind::Pending))
    );
    let hot = Actors::actor_hot(actor_id).unwrap();
    assert!(hot.pending_signal && hot.trigger_wakeup_pointer.is_none());
    assert_eq!(fee_collections(), vec![cadenced_trigger_fee()]);
    assert_eq!(
      native_balance(&sovereign),
      balances.0 - cadenced_trigger_fee()
    );
    assert_eq!(
      native_balance(&TestFeeSink::get()),
      balances.1 + cadenced_trigger_fee()
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn busy_cadenced_rearm_refuses_each_resource_dimension_before_mutation() {
  for failed_attempts in 0..=2 {
    new_test_ext().execute_with(|| {
      let actor_id = prepare_busy_cadenced_actor(ActorType::User, failed_attempts);
      let actor = Actors::load_actor_ref(actor_id).unwrap();
      let source = crate::TriggerDeadlineHandles::<Test>::get(actor_id).unwrap();
      assert_eq!(
        Actors::classify_next_due_tick_deadline(11),
        Ok(crate::DueBlockDeadlineBranch::TemporalTriggerBusy(actor))
      );
      let selector = <TestWeightInfo as crate::WeightInfo>::classify_due_tick_deadline();
      let branch = Actors::cadenced_busy_rearm_weight_upper();
      let complete = selector.saturating_add(branch);
      let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      for (limit, consumed) in [
        (
          Weight::from_parts(selector.ref_time() - 1, u64::MAX),
          Weight::zero(),
        ),
        (
          Weight::from_parts(u64::MAX, selector.proof_size() - 1),
          Weight::zero(),
        ),
        (
          Weight::from_parts(complete.ref_time() - 1, u64::MAX),
          selector,
        ),
        (
          Weight::from_parts(u64::MAX, complete.proof_size() - 1),
          selector,
        ),
      ] {
        let mut meter = WeightMeter::with_limit(limit);
        assert_eq!(
          Actors::process_next_due_tick_deadline(&mut meter, 11),
          Err(crate::DependencyReviewWorkerError::InsufficientWeight)
        );
        assert_eq!(meter.consumed(), consumed);
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
          root
        );
        assert_eq!(
          crate::TriggerDeadlineHandles::<Test>::get(actor_id),
          Some(source)
        );
      }
    });
  }
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_carrier_deduplicates_process_and_trigger_slots() {
  new_test_ext().execute_with(|| {
    let actor_id = prepare_busy_cadenced_actor(ActorType::System, 2);
    let process = crate::DeadlineHandles::<Test>::get(actor_id).unwrap();
    let trigger = crate::TriggerDeadlineHandles::<Test>::get(actor_id).unwrap();
    assert_ne!(process.key, trigger.key);
    assert!(
      Actors::actor_hot(actor_id)
        .unwrap()
        .wakeup_pointer
        .is_none()
    );
    assert_ok!(Actors::do_try_state());
    let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      // Corrupt the already-produced result: individually coherent handles cannot share a slot.
      crate::DeadlineHandles::<Test>::insert(actor_id, trigger);
      ActorProcesses::<Test>::mutate(actor_id, |process| {
        process.as_mut().unwrap().residence = Some(ProcessResidence::Deadline {
          key: trigger.key,
          page: trigger.page,
          slot: trigger.slot,
        });
      });
      let corrupted =
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      assert_eq!(
        Actors::do_try_state(),
        Err(polkadot_sdk::sp_runtime::TryRuntimeError::Other(
          "multiple Deadline handles own one physical slot"
        ))
      );
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        corrupted
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      before
    );
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn busy_cadenced_occurrence_rearms_without_fees_or_future_cycle() {
  for actor_type in [ActorType::User, ActorType::System] {
    for failed_attempts in 0..=2 {
      for funded in [false, true] {
        new_test_ext().execute_with(|| {
          let actor_id = prepare_busy_cadenced_actor(actor_type, failed_attempts);
          let identity = Actors::actor_identity(actor_id).unwrap();
          if !funded {
            deplete_user_sovereign(
              actor_id,
              native_balance(&identity.sovereign_account) - TestMinUserBalance::get(),
            );
          }
          let run = Actors::actor_run_state(actor_id).unwrap().encode();
          let mut expected_hot = Actors::actor_hot(actor_id).unwrap();
          let publication = (
            ActorProcesses::<Test>::get(actor_id),
            ServiceNodes::<Test>::get(actor_id),
            crate::ServiceHeader::<Test>::get(),
            crate::DeadlineHandles::<Test>::get(actor_id),
          );
          let balances = (
            native_balance(&identity.sovereign_account),
            native_balance(&BOB),
            native_balance(&TestFeeSink::get()),
          );
          let actor = Actors::load_actor_ref(actor_id).unwrap();
          clear_fee_collections();
          System::reset_events();
          set_fail_fee_sink_transfer(true);
          // Section 2.3 owns current-state coalescing: busy service buys no deferred Cycle.
          // A missed cadence also advances straight to the next aligned future point.
          for (now, next) in [(11, 16), (23, 26)] {
            System::set_block_number(now);
            let mut meter = WeightMeter::with_limit(Weight::MAX);
            assert_eq!(
              Actors::process_next_due_tick_deadline(&mut meter, now),
              Ok(crate::DueTickDeadlineMutation::TemporalTriggerProcessed(
                actor
              ))
            );
            assert!(
              fee_collections().is_empty(),
              "busy service never invokes the collector"
            );
            let source = crate::TriggerDeadlineHandles::<Test>::get(actor_id).unwrap();
            assert_eq!((source.actor, source.key), (actor, WakeupKey::Tick(next)));
            expected_hot.trigger_wakeup_pointer = Some(crate::TriggerWakeupPointer {
              tick: next,
              page_id: source.page,
              slot: u32::from(source.slot),
            });
            assert_eq!(Actors::actor_hot(actor_id), Some(expected_hot.clone()));
            assert_eq!(Actors::actor_identity(actor_id), Some(identity.clone()));
            assert_eq!(Actors::actor_run_state(actor_id).unwrap().encode(), run);
            assert_eq!(
              (
                ActorProcesses::<Test>::get(actor_id),
                ServiceNodes::<Test>::get(actor_id),
                crate::ServiceHeader::<Test>::get(),
                crate::DeadlineHandles::<Test>::get(actor_id),
              ),
              publication
            );
            assert_eq!(
              (
                native_balance(&identity.sovereign_account),
                native_balance(&BOB),
                native_balance(&TestFeeSink::get())
              ),
              balances
            );
            assert!(
              System::events().is_empty(),
              "unexpected busy-work events: {:?}",
              System::events()
            );
            #[cfg(feature = "try-runtime")]
            assert_ok!(Actors::do_try_state());
          }
          set_fail_fee_sink_transfer(false);
        });
      }
    }
  }
}

#[test]
fn underfunded_cadenced_occurrence_advances_without_fee_readiness_or_apoptosis() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      timer_schedule(1),
      None,
      inert_contract_steps(),
    );
    let sovereign = sovereign_account(actor_id);
    let balance = native_balance(&sovereign);
    deplete_user_sovereign(actor_id, balance - TestMinUserBalance::get());
    clear_fee_collections();
    System::reset_events();
    set_fail_fee_sink_transfer(true);
    let identity = Actors::actor_identity(actor_id).unwrap();
    let process = ActorProcesses::<Test>::get(actor_id).unwrap().encode();
    let service = ServiceNodes::<Test>::get(actor_id).encode();

    frame_system::Pallet::<Test>::set_block_number(2);
    service_canonical_temporal_frontiers(2);

    assert!(fee_collections().is_empty());
    let hot = Actors::actor_hot(actor_id).expect("underfunded process remains active");
    assert!(!hot.pending_signal);
    assert!(hot.queue_ticket.is_none());
    assert!(hot.wakeup_pointer.is_none());
    assert_eq!(
      hot.trigger_wakeup_pointer.map(|pointer| pointer.tick),
      Some(3)
    );
    assert_eq!(native_balance(&sovereign), TestMinUserBalance::get());
    assert_eq!(Actors::actor_identity(actor_id), Some(identity));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).unwrap().encode(),
      process
    );
    assert_eq!(ServiceNodes::<Test>::get(actor_id).encode(), service);
    assert!(System::events().is_empty());
    set_fail_fee_sink_transfer(false);
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn next_block_cadence_rearms_after_each_deferred_opening_without_late_fifo_tickets() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(1), None, inert_contract_steps());
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    Actors::on_idle(1, Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      0
    );

    // Each cadence occurrence publishes one Pending Service for B+1. The deferred opening executes
    // the cycle, re-arms the next deadline, and never materializes a legacy FIFO queue ticket.
    frame_system::Pallet::<Test>::set_block_number(2);
    service_canonical_temporal_frontiers(2);
    let latched = Actors::actor_hot(actor_id).expect("Actors exists");
    assert!(latched.pending_signal);
    assert!(latched.queue_ticket.is_none());
    assert!(!ActorControlLocators::<Test>::contains_key(actor_id));

    run_next_idle(Weight::MAX);
    let after_first = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(after_first.cycle_nonce, 1);
    assert!(after_first.queue_ticket.is_none());
    assert!(!Actors::pending_signal(actor_id));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(4));

    for block in [4, 6] {
      frame_system::Pallet::<Test>::set_block_number(block);
      service_canonical_temporal_frontiers(block);
      run_next_idle(Weight::MAX);
      let actor = Actors::active_actor_view(actor_id).expect("Actors exists");
      assert!(actor.queue_ticket.is_none());
      assert!(!actor.pending_signal);
    }
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      3
    );
  });
}

#[test]
fn paused_timer_waits_for_resume_without_queue_churn_or_signal_loss() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let schedule = Schedule {
      trigger: Trigger::cadenced(1),
      cooldown_blocks: 0,
    };
    let actor_id = create_system_with(ALICE, schedule, None, inert_contract_steps());
    run_idle(Weight::MAX);
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    assert_ok!(Actors::pause_actor(RuntimeOrigin::root(), actor_id));

    // Pause releases the canonical Trigger deadline and leaves no queue ticket or Service
    // residence behind, so time passing while paused performs no cycle work.
    assert_eq!(scheduled_wakeup_block(actor_id), None);
    let paused = Actors::actor_hot(actor_id).expect("paused actor");
    assert!(paused.lifecycle.is_paused());
    assert!(paused.queue_ticket.is_none());
    frame_system::Pallet::<Test>::set_block_number(6);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      0
    );
    assert_eq!(scheduled_wakeup_block(actor_id), None);

    // Resume re-arms the cadence from the current tick, so the deferred opening still occurs.
    frame_system::Pallet::<Test>::set_block_number(7);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::root(), actor_id));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(8));
    frame_system::Pallet::<Test>::set_block_number(8);
    service_canonical_temporal_frontiers(8);
    run_next_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      1
    );
  });
}

#[test]
fn timer_validation_uses_the_independent_exact_temporal_horizon() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_delay = u32::try_from(<Test as crate::Config>::MaxTemporalDelayTicks::get())
      .expect("test temporal horizon fits u32");
    assert!(
      u64::from(max_delay) > <Test as crate::Config>::MaxExecutionDelayBlocks::get(),
      "cadence ticks and consensus blocks have independent horizons"
    );
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract(timer_schedule(max_delay), None, inert_contract_steps()),
    ));
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        system_active_contract(
          timer_schedule(max_delay.saturating_add(1)),
          None,
          inert_contract_steps(),
        ),
      ),
      Error::<Test>::ExecutionDelayTooLong
    );
  });
}

#[test]
fn scheduler_ignores_sparse_id_gaps() {
  // Sparse Actors IDs must not create a scheduler "shadow zone".
  // Create Actors at ID 0, bump NextActorId to 2000 (huge gap), create Actors at ID 2000.
  // Both must execute in the first block.
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let schedule = timer_schedule(1);
    let contract_steps = inert_contract_steps();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract(schedule.clone(), None, contract_steps.clone()),
    ));
    let sov_0 = Actors::sovereign_account_id_system(0);
    let _ = Balances::deposit_creating(&sov_0, 1_000_000);
    // Bump NextActorId to create 2000-wide gap
    crate::pallet::NextActorId::<Test>::put(2000u64);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract(schedule, None, contract_steps),
    ));
    let sov_2000 = Actors::sovereign_account_id_system(2000);
    let _ = Balances::deposit_creating(&sov_2000, 1_000_000);
    assert_eq!(Actors::next_actor_id(), 2001);
    assert!(Actors::active_actor_view(0).is_some());
    assert!(Actors::active_actor_view(2000).is_some());
    // Run one canonical service block: both actors must execute despite 2000-wide ID gap.
    // Canonical publication serves the occurrence at B+1 through the mandatory prepass and
    // Drain phase, so the block boundary and hooks must be driven canonically.
    System::reset_events();
    run_next_idle(Weight::from_parts(u64::MAX, u64::MAX));
    let executed: alloc::vec::Vec<_> = System::events()
      .iter()
      .filter_map(|r| {
        if let RuntimeEvent::Actors(Event::CycleSummary { actor_id, .. }) = &r.event {
          Some(*actor_id)
        } else {
          None
        }
      })
      .collect();
    assert!(
      executed.contains(&0),
      "ID 0 must execute despite sparse Actors IDs"
    );
    assert!(
      executed.contains(&2000),
      "ID 2000 must execute despite sparse Actors IDs"
    );
  });
}

#[test]
fn scheduler_continues_after_in_loop_close_and_executes_following_ready_actors() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let close_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    deplete_user_sovereign(
      close_id,
      user_prefunding_requirement(&inert_contract_steps()),
    );
    fund_native(
      close_id,
      TestMinUserBalance::get().saturating_add(manual_trigger_fee()),
    );
    let live_id_1 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let live_id_2 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    fund_native(live_id_1, 1_000);
    fund_native(live_id_2, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      close_id
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      live_id_1
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      live_id_2
    ));
    System::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(close_id).is_none());
    assert_eq!(
      Actors::active_actor_view(live_id_1)
        .expect("live actor")
        .cycle_nonce,
      1
    );
    assert_eq!(
      Actors::active_actor_view(live_id_2)
        .expect("live actor")
        .cycle_nonce,
      1
    );
  });
}

#[test]
fn queue_progress_handles_adjacent_removal() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let id0 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let id1 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let id2 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let id3 = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    deplete_user_sovereign(id3, user_prefunding_requirement(&inert_contract_steps()));
    fund_native(
      id3,
      TestMinUserBalance::get().saturating_add(manual_trigger_fee()),
    );
    fund_native(id0, 1_000);
    fund_native(id1, 1_000);
    fund_native(id2, 1_000);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id0));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id1));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id2));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id3));
    System::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(id0)
        .expect("id0 live")
        .cycle_nonce,
      1
    );
    assert_eq!(
      Actors::active_actor_view(id1)
        .expect("id1 live")
        .cycle_nonce,
      1
    );
    assert_eq!(
      Actors::active_actor_view(id2)
        .expect("id2 executed")
        .cycle_nonce,
      1
    );
    // The insolvent adjacent tail is closed by the same Service round that advances the funded
    // prefix, so adjacent removal never stalls the remaining queue.
    assert!(Actors::active_actor_view(id3).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed { actor_id, reason: CloseReason::CycleAdmissionInsufficient }
        if *actor_id == id3
    )));
  });
}

#[test]
fn queue_progress_matrix_keeps_progress_and_coverage() {
  for funded_mask in 1u8..=7u8 {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let ids = [
        create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          inert_contract_steps(),
        ),
        create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          inert_contract_steps(),
        ),
        create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          inert_contract_steps(),
        ),
      ];
      for (idx, actor_id) in ids.iter().enumerate() {
        deplete_user_sovereign(
          *actor_id,
          user_prefunding_requirement(&inert_contract_steps()),
        );
        if (funded_mask & (1 << idx)) != 0 {
          fund_native(*actor_id, 1_000);
        } else {
          fund_native(
            *actor_id,
            TestMinUserBalance::get().saturating_add(manual_trigger_fee()),
          );
        }
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          *actor_id
        ));
      }
      System::set_block_number(2);
      run_idle(Weight::MAX);
      let expected_started = ids
        .iter()
        .enumerate()
        .filter(|(idx, _)| (funded_mask & (1 << idx)) != 0)
        .count() as u32;
      let started = frame_system::Pallet::<Test>::events()
        .iter()
        .filter(|record| {
          matches!(
            record.event,
            RuntimeEvent::Actors(Event::CycleStarted { .. })
          )
        })
        .count() as u32;
      assert_eq!(started, expected_started);
      for (idx, actor_id) in ids.iter().enumerate() {
        if (funded_mask & (1 << idx)) != 0 {
          assert_eq!(
            Actors::active_actor_view(*actor_id)
              .expect("funded actor")
              .cycle_nonce,
            1
          );
        } else {
          assert!(Actors::active_actor_view(*actor_id).is_none());
        }
      }
    });
  }
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn eligibility_projection_rejects_missing_canonical_contract_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingSignal
    );

    crate::ActorContractHeads::<Test>::remove(actor_id);
    assert_eq!(
      Actors::actor_eligibility(actor_id),
      Err(crate::ActorClassificationError::ActorInvariant)
    );
  });
}

#[test]
fn eligibility_projection_reports_exact_at_time_gate_and_consumption() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, at_time_schedule(20), None, inert_contract_steps());

    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingCadenceTick(21)
    );
    frame_system::Pallet::<Test>::set_block_number(21);
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingCadenceTick(21)
    );
    service_canonical_temporal_frontiers(21);
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::Ready
    );
    run_next_idle(Weight::MAX);
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingSignal
    );
  });
}

#[test]
fn eligibility_projection_reports_exact_cadence_gate_without_actor_phase() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let cadence = 20u32;
    let actor_id = create_system_with(ALICE, timer_schedule(cadence), None, inert_contract_steps());

    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingCadenceTick(21)
    );

    frame_system::Pallet::<Test>::set_block_number(11);
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingCadenceTick(21)
    );

    frame_system::Pallet::<Test>::set_block_number(21);
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingCadenceTick(21),
      "a due but unmaterialized tick remains the exact live obligation"
    );
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        system_active_contract(timer_schedule(cadence), None, inert_contract_steps())
          .expect("expected Actor Contract"),
        SimulationMode::FreshCurrentPlan,
        ample_simulation_budget(),
      ),
      Err(SimulationError::NotReady),
      "simulation must not synthesize readiness before scheduler materialization"
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn canonical_in_place_hot_mutation_requires_no_legacy_primary() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    assert!(!ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!crate::ActorUnsignaledControlCells::<Test>::contains_key(
      actor_id
    ));
    let before = Actors::actor_hot(actor_id).expect("canonical semantic Hot owner");
    Actors::try_mutate_control_hot_with_authority(actor_id, Error::<Test>::ActorNotFound, |hot| {
      hot.pending_signal = !before.pending_signal;
      Ok(())
    })
    .expect("canonical in-place Hot mutation commits through the semantic owner");
    let after = Actors::actor_hot(actor_id).expect("canonical semantic Hot owner");
    assert_eq!(after.pending_signal, !before.pending_signal);
    assert!(Actors::actor_control_cell(actor_id).is_none());
    assert!(!ActorControlLocators::<Test>::contains_key(actor_id));
  });
}
