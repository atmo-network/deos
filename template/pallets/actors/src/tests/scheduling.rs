use super::*;
use crate::{FundingProvenance, TriggerCauseProvenance};

#[test]
fn cancelled_run_auto_close_precedes_insufficient_next_pipeline_fee() {
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
    assert_ok!(Actors::cancel_run(RuntimeOrigin::signed(ALICE), actor_id));
    let cancelled =
      Actors::active_actor_view(actor_id).expect("deferred readiness survives cancel");
    assert_eq!(cancelled.cycle_nonce, 1);
    assert!(cancelled.pending_signal);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(
      Actors::actor_control_cell(actor_id).and_then(|(_, cell)| cell.eligible_at),
      Some(3)
    );
    Actors::execute_cycle_to_cutoff(Weight::MAX, Actors::queue_tail());
    assert_eq!(Actors::active_actor_view(actor_id), Some(cancelled));
    let sovereign = sovereign_account(actor_id);
    let excess = native_balance(&sovereign)
      .checked_sub(TestMinUserBalance::get())
      .expect("funded Actor covers the protected floor");
    deplete_user_sovereign(actor_id, excess);
    System::set_block_number(3);
    System::reset_events();
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(actor_id).is_none());
    assert_eq!(native_balance(&sovereign), TestMinUserBalance::get());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed { actor_id: id, reason: CloseReason::AutoCloseNonceReached }
        if *id == actor_id
    )));
    assert!(!has_actor_event(|event| matches!(
      event, Event::CycleStarted { actor_id: id, .. } if *id == actor_id
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
        let limits = crate::SimulationBudget {
          actor_control: Weight::from_parts(u64::MAX / 2, u64::MAX / 2),
          shared_economic: effect_capacity,
        }
        .checked_limits()
        .expect("independent component limits fit");
        let mut resources = crate::BlockResourceState::new(1);
        assert_ok!(resources.begin_prepass());
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
          assert_eq!(
            Actors::paged_head_entry().map(|(_, entry)| entry.actor_id),
            Some(actor_id)
          );
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
fn idle_weight_refusal_reads_only_header_and_user_fee_prerequisite() {
  for actor_type in [ActorType::System, ActorType::User] {
    let mut ext = new_test_ext();
    let (actor_id, cutoff, minimum_probe, header_key, native_key, payload_keys) =
      ext.execute_with(|| {
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
        let minimum_probe =
          <TestWeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1)
            .saturating_add(Actors::scheduler_actor_state_probe_weight_upper())
            .saturating_add(
              <TestWeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
                .max(<TestWeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page()),
            );
        (
          actor_id,
          Actors::queue_tail(),
          minimum_probe,
          crate::ActorContractHeads::<Test>::hashed_key_for(actor_id),
          polkadot_sdk::frame_system::Account::<Test>::hashed_key_for(sovereign_account(actor_id)),
          [
            crate::ActorFunding::<Test>::hashed_key_for(actor_id),
            crate::ActorRunHeads::<Test>::hashed_key_for(actor_id),
            crate::ActorRunPayloads::<Test>::hashed_key_for(actor_id),
          ],
        )
      });
    ext.commit_all().expect("commit fixture before recording");
    let before = ext.execute_with(|| polkadot_sdk::sp_io::storage::root(StateVersion::V1));
    ext.commit_all().expect("commit root calculation");
    for scarce in [
      Weight::from_parts(minimum_probe.ref_time(), u64::MAX),
      Weight::from_parts(u64::MAX, minimum_probe.proof_size()),
    ] {
      for domain in [
        None,
        Some(crate::BlockResourceDomain::ActorControl),
        Some(crate::BlockResourceDomain::ActorDrainEffect),
      ] {
        let recorder = polkadot_sdk::sp_trie::recorder::Recorder::<
          polkadot_sdk::sp_core::Blake2Hasher,
        >::default();
        ext.execute_with_recorder(recorder.clone(), || {
          if let Some(domain) = domain {
            let ample = Weight::from_parts(u64::MAX / 2, u64::MAX / 2);
            let shared_economic = if domain == crate::BlockResourceDomain::ActorDrainEffect {
              if scarce.ref_time() == u64::MAX {
                Weight::from_parts(ample.ref_time(), 0)
              } else {
                Weight::from_parts(0, ample.proof_size())
              }
            } else {
              ample
            };
            let limits = crate::SimulationBudget {
              actor_control: ample,
              shared_economic,
            }
            .checked_limits()
            .expect("independent resource lanes fit");
            let mut resources = crate::BlockResourceState::new(1);
            assert_ok!(resources.begin_prepass());
            assert_ok!(resources.open_external_phase());
            assert_ok!(resources.begin_drain());
            let pass = Actors::execute_cycle_to_cutoff_with_resources(
              Weight::MAX,
              cutoff,
              &mut resources,
              limits,
              crate::BlockResourceDomain::ActorDrainEffect,
              if domain == crate::BlockResourceDomain::ActorControl {
                scarce.min(ample)
              } else {
                ample
              },
            );
            assert_eq!(resources.outstanding_reservations(), 0);
            assert_eq!(resources.usage().actor_effect_used(), Weight::zero());
            assert_eq!(resources.usage().actor_control_used(), pass.consumed);
          } else {
            Actors::execute_cycle_to_cutoff(scarce, cutoff);
          }
        });
        let recorded = recorder.recorded_keys();
        let was_read = |key: &[u8]| {
          recorded
            .values()
            .any(|keys| keys.keys().any(|read| read.as_ref() == key))
        };
        assert!(
          was_read(&header_key),
          "terminal policy requires the Contract header"
        );
        assert_eq!(
          was_read(&native_key),
          actor_type == ActorType::User,
          "only User admission requires a native-fee balance read"
        );
        for key in &payload_keys {
          assert!(
            !was_read(key),
            "Weight-rejected Idle {actor_type:?} read payload {key:?}"
          );
        }
        ext.execute_with(|| {
          assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
          assert_eq!(
            Actors::paged_head_entry().map(|(_, entry)| entry.actor_id),
            Some(actor_id)
          );
        });
        ext.commit_all().expect("commit unchanged rejection state");
      }
    }
    ext.execute_with(|| {
      let recipient = native_balance(&BOB);
      Actors::execute_cycle_to_cutoff(Weight::MAX, cutoff);
      assert_eq!(native_balance(&BOB), recipient + 10);
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
fn running_fifo_head_avoids_cold_reads_before_eligibility_and_weight_admission() {
  let mut ext = new_test_ext();
  let (actor_id, due, cutoff, cold_keys) = ext.execute_with(|| {
    System::set_block_number(1);
    let steps = BoundedVec::try_from(vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      }),
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(2),
      }),
    ])
    .expect("two Steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    Actors::execute_cycle_to_cutoff(Weight::MAX, Actors::queue_tail());
    let run = Actors::actor_run_state(actor_id).expect("real Opening persists its successor");
    assert_eq!(run.cursor, 1);
    assert!(run.eligible_at > System::block_number());
    assert_eq!(
      Actors::paged_head_entry().map(|(_, entry)| entry.actor_id),
      Some(actor_id)
    );
    let cold_keys = [
      crate::ActorContractHeads::<Test>::hashed_key_for(actor_id),
      crate::ActorContractTailChunks::<Test>::hashed_key_for(actor_id, 0),
      crate::ActorFunding::<Test>::hashed_key_for(actor_id),
      crate::ActorRunPayloads::<Test>::hashed_key_for(actor_id),
    ];
    (actor_id, run.eligible_at, Actors::queue_tail(), cold_keys)
  });
  ext
    .commit_all()
    .expect("fixture overlay commits before read recording");
  let before = ext.execute_with(|| polkadot_sdk::sp_io::storage::root(StateVersion::V1));
  ext
    .commit_all()
    .expect("root calculation leaves no overlay authority");
  let recorder =
    polkadot_sdk::sp_trie::recorder::Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
  ext.execute_with_recorder(recorder.clone(), || {
    Actors::execute_cycle_to_cutoff(Weight::MAX, cutoff);
  });
  let recorded = recorder.recorded_keys();
  for key in &cold_keys {
    assert!(
      !recorded
        .values()
        .any(|keys| keys.keys().any(|read| read.as_ref() == key.as_slice())),
      "future head read cold key {key:?}"
    );
  }
  ext.execute_with(|| {
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
    assert_eq!(
      Actors::actor_run_state(actor_id).map(|run| run.cursor),
      Some(1)
    );
    System::set_block_number(due);
  });
  ext
    .commit_all()
    .expect("eligible fixture overlay commits before positive control");
  let (minimum_probe, eligible_root, step_resources) = ext.execute_with(|| {
    let probe = <TestWeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1)
      .saturating_add(Actors::scheduler_actor_state_probe_weight_upper())
      .saturating_add(
        <TestWeightInfo as crate::WeightInfo>::scheduler_paged_consume_preserve_page()
          .max(<TestWeightInfo as crate::WeightInfo>::scheduler_paged_consume_delete_page()),
      );
    let resources = Actors::actor_control_cell(actor_id)
      .expect("canonical Running primary")
      .1
      .resources;
    (
      probe,
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      resources,
    )
  });
  ext
    .commit_all()
    .expect("eligible root commits before recording");
  for scarce in [
    Weight::from_parts(minimum_probe.ref_time(), u64::MAX),
    Weight::from_parts(u64::MAX, minimum_probe.proof_size()),
  ] {
    for (domain, cleanup_only) in [
      (None, false),
      (Some(crate::BlockResourceDomain::ActorControl), false),
      (Some(crate::BlockResourceDomain::ActorBaseEffect), false),
      (Some(crate::BlockResourceDomain::ActorDrainEffect), false),
      (None, true),
      (Some(crate::BlockResourceDomain::ActorControl), true),
    ] {
      let scarce = if cleanup_only {
        let step = if domain.is_none() {
          step_resources.control.saturating_add(step_resources.effect)
        } else {
          step_resources.control
        };
        if scarce.ref_time() == u64::MAX {
          Weight::from_parts(
            u64::MAX,
            scarce.proof_size().saturating_add(step.proof_size()),
          )
        } else {
          Weight::from_parts(scarce.ref_time().saturating_add(step.ref_time()), u64::MAX)
        }
      } else {
        scarce
      };
      let recorder =
        polkadot_sdk::sp_trie::recorder::Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
      ext.execute_with_recorder(recorder.clone(), || {
        if let Some(domain) = domain {
          let limits = crate::SimulationBudget {
            actor_control: Weight::from_parts(u64::MAX / 2, u64::MAX / 2),
            shared_economic: Weight::from_parts(u64::MAX / 2, u64::MAX / 2),
          }
          .checked_limits()
          .expect("two ample lanes fit");
          let mut resources = crate::BlockResourceState::new(due);
          assert_ok!(resources.begin_prepass());
          let effect_domain = if domain == crate::BlockResourceDomain::ActorBaseEffect {
            let saturated = if scarce.ref_time() == u64::MAX {
              Weight::from_parts(0, limits.actor_base_turn().proof_size())
            } else {
              Weight::from_parts(limits.actor_base_turn().ref_time(), 0)
            };
            assert!(
              saturated
                .saturating_add(step_resources.effect)
                .all_lte(limits.shared_economic())
            );
            let mut prior = resources
              .reserve(limits, domain, saturated)
              .expect("prior Actor work fits its base turn exactly");
            assert_ok!(resources.settle(&mut prior, saturated));
            domain
          } else {
            assert_ok!(resources.open_external_phase());
            crate::BlockResourceDomain::ActorDrainEffect
          };
          if domain == crate::BlockResourceDomain::ActorDrainEffect {
            let saturated = if scarce.ref_time() == u64::MAX {
              Weight::from_parts(0, limits.shared_economic().proof_size())
            } else {
              Weight::from_parts(limits.shared_economic().ref_time(), 0)
            };
            let mut user = resources
              .reserve(limits, crate::BlockResourceDomain::UserDispatch, saturated)
              .expect("prior user work fits exactly");
            assert_ok!(resources.settle(&mut user, saturated));
          }
          if effect_domain == crate::BlockResourceDomain::ActorDrainEffect {
            assert_ok!(resources.begin_drain());
          }
          let prior_usage = resources.usage();
          let pass = Actors::execute_cycle_to_cutoff_with_resources(
            Weight::MAX,
            cutoff,
            &mut resources,
            limits,
            effect_domain,
            if domain == crate::BlockResourceDomain::ActorControl {
              scarce.min(limits.actor_control())
            } else {
              limits.actor_control()
            },
          );
          assert_eq!(resources.outstanding_reservations(), 0);
          assert_eq!(
            resources.usage().actor_effect_used(),
            prior_usage.actor_effect_used()
          );
          assert_eq!(resources.usage().actor_control_used(), pass.consumed);
          assert_eq!(
            resources.usage().user_dispatch_used(),
            prior_usage.user_dispatch_used()
          );
        } else {
          Actors::execute_cycle_to_cutoff(scarce, cutoff);
        }
      });
      let recorded = recorder.recorded_keys();
      for key in &cold_keys {
        assert!(
          !recorded
            .values()
            .any(|keys| keys.keys().any(|read| read.as_ref() == key.as_slice())),
          "Weight-rejected Running head read cold key {key:?}"
        );
      }
      ext.execute_with(|| {
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(StateVersion::V1),
          eligible_root
        );
      });
      ext
        .commit_all()
        .expect("refusal assertions stay outside the next recording");
    }
  }
  let recorder =
    polkadot_sdk::sp_trie::recorder::Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
  ext.execute_with_recorder(recorder.clone(), || {
    Actors::execute_cycle_to_cutoff(Weight::MAX, cutoff);
  });
  let recorded = recorder.recorded_keys();
  for key in &cold_keys {
    assert!(
      recorded
        .values()
        .any(|keys| keys.keys().any(|read| read.as_ref() == key.as_slice())),
      "eligible positive control did not record cold key {key:?}"
    );
  }
  ext.execute_with(|| {
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert_eq!(
      Actors::actor_identity(actor_id).map(|identity| identity.cycle_nonce),
      Some(1)
    );
  });
}

#[test]
fn initial_placement_preserves_creation_and_reactivation_authority() {
  for schedule in [manual_schedule(), at_time_schedule(10), timer_schedule(10)] {
    for empty in [false, true] {
      new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(1);
        let window = matches!(schedule.trigger, Trigger::Manual).then_some(ScheduleWindow {
          start: 10,
          end: 10 + <<Test as crate::Config>::MinWindowLength as Get<u64>>::get(),
        });
        let steps = if empty { BoundedVec::default() } else { inert_contract_steps() };
        let contract = system_active_contract(schedule.clone(), window, steps);
        let actor_id = NextActorId::<Test>::get();
        for reactivation in [false, true] {
          if reactivation {
            frame_system::Pallet::<Test>::set_block_number(2);
            assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), actor_id));
            frame_system::Pallet::<Test>::set_block_number(3);
          }
          let install = || {
            if reactivation {
              Actors::activate_actor(RuntimeOrigin::root(), actor_id, contract.clone().expect("active Contract"))
            } else {
              Actors::create_system_actor(RuntimeOrigin::root(), ALICE, Mutability::Mutable, contract.clone())
            }
          };
          let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
          Actors::test_fail_wakeup_placement_with_capacity();
          assert_noop!(install(), Error::<Test>::QueueCapacityUnavailable);
          assert_eq!(polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1), before);
          assert_ok!(install());
          let (state, admission, step) = Actors::load_frame_actor_service_state(actor_id)
            .expect("complete installed authority");
          assert_eq!(step.is_some(), !empty);
          assert_eq!(state.hot.cycle_state, CycleState::Idle);
          assert!(!state.hot.pending_signal);
          assert!(state.hot.queue_ticket.is_none());
          assert!(state.run_state.is_none());
          let expected_key = if let Some(window) = window {
            assert!(state.hot.trigger_wakeup_pointer.is_none());
            let pointer = state.hot.wakeup_pointer.expect("terminal Block pointer");
            assert_eq!(pointer.block, WakeupKey::Block(window.end + 1));
            pointer.block
          } else {
            assert!(state.hot.wakeup_pointer.is_none());
            let pointer = state.hot.trigger_wakeup_pointer.expect("initial Tick pointer");
            assert_eq!(pointer.tick, if reactivation { 13 } else { 11 });
            WakeupKey::Tick(pointer.tick)
          };
          let (location, cell) = Actors::actor_control_cell(actor_id).expect("installed primary");
          assert!(matches!(location, crate::ActorControlLocation::Waiting { key, .. } if key == expected_key));
          assert_eq!(cell.admission, admission);
          #[cfg(feature = "try-runtime")]
          assert_ok!(Actors::do_try_state());
        }
      });
    }
  }
}

#[test]
fn temporal_replacement_publishes_exact_primary_and_preserves_failed_source() {
  for schedule in [at_time_schedule(10), timer_schedule(10)] {
    for empty in [false, true] {
      new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(1);
        let steps = if empty {
          BoundedVec::default()
        } else {
          inert_contract_steps()
        };
        let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
        let mut replacement = Actors::load_actor_contract(actor_id).expect("admitted Contract");
        replacement.trigger = schedule.trigger.clone();
        frame_system::Pallet::<Test>::set_block_number(2);
        let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        Actors::test_fail_wakeup_placement_with_capacity();
        assert_noop!(
          Actors::update_contract(RuntimeOrigin::root(), actor_id, replacement.clone()),
          Error::<Test>::QueueCapacityUnavailable
        );
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
          before
        );
        assert_ok!(Actors::update_contract(
          RuntimeOrigin::root(),
          actor_id,
          replacement
        ));
        let (state, admission, loaded_step) =
          Actors::load_frame_actor_service_state(actor_id).expect("complete temporal authority");
        assert_eq!(loaded_step.is_some(), !empty);
        let anchor = match state.hot.trigger_runtime_state {
          TriggerRuntimeState::AtTime { anchor_tick, .. }
          | TriggerRuntimeState::Cadenced { anchor_tick } => {
            anchor_tick.expect("anchored replacement")
          }
          _ => panic!("temporal Trigger"),
        };
        let pointer = state.hot.trigger_wakeup_pointer.expect("Tick pointer");
        assert_eq!(pointer.tick, anchor + 10);
        let (location, cell) = Actors::actor_control_cell(actor_id).expect("sole primary");
        let crate::ActorControlLocation::Waiting { key, page, slot } = location else {
          panic!("Tick primary");
        };
        assert_eq!(key, WakeupKey::Tick(pointer.tick));
        assert_eq!(page, pointer.page_id);
        assert_eq!(u32::from(slot), pointer.slot);
        assert_eq!(cell.admission, admission);
        assert_eq!(cell.cursor, 0);
        assert_eq!(
          cell.resources,
          loaded_step.map_or(
            crate::ActorStepResourceEnvelope {
              control: <<Test as crate::Config>::WeightInfo as crate::weights::WeightInfo>::scheduler_inner_zero_step_complete(),
              effect: Weight::zero(),
            },
            |loaded| loaded.resources
          )
        );
        let published =
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        assert_ok!(Actors::prime_frame_actor_schedule(actor_id));
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
          published
        );
        #[cfg(feature = "try-runtime")]
        assert_ok!(Actors::do_try_state());
      });
    }
  }
}

#[test]
fn retained_ingress_rejects_incomplete_authority_without_writes() {
  for enqueue in [false, true] {
    new_test_ext().execute_with(|| {
      let actor_id = create_suspended_system_retry(1);
      let state = Actors::active_actor_state(actor_id).expect("real suspended Actor");
      let invoke = || {
        if enqueue {
          Actors::try_paged_enqueue(actor_id)
        } else {
          Actors::try_wakeup_substrate_schedule_inner(actor_id, 10)
        }
      };
      let funding = crate::ActorFunding::<Test>::take(actor_id).expect("funding authority");
      let corrupt = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      assert_eq!(
        invoke(),
        Err(crate::scheduler::EnqueueOutcome::CorruptedTopology)
      );
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        corrupt
      );
      crate::ActorFunding::<Test>::insert(actor_id, funding);
      let payload = crate::ActorRunPayloads::<Test>::take(actor_id).expect("Run payload authority");
      let corrupt = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      assert_eq!(
        invoke(),
        Err(crate::scheduler::EnqueueOutcome::CorruptedTopology)
      );
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        corrupt
      );
      crate::ActorRunPayloads::<Test>::insert(actor_id, payload);
      let expected = if enqueue && state.hot.queue_ticket.is_some() {
        Err(crate::scheduler::EnqueueOutcome::AlreadyLive)
      } else {
        Ok(())
      };
      assert_eq!(invoke(), expected, "retained ingress enqueue={enqueue}");
      assert_eq!(
        Actors::actor_run_state(actor_id).encode(),
        state.run_state.encode()
      );
      assert!(Actors::active_actor_state(actor_id).is_some());
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn retained_wakeup_deferral_preserves_capacity_rollback_and_rejects_corruption() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    let state = Actors::active_actor_state(actor_id).expect("real suspended Actor");
    let (_, cell) = Actors::actor_control_cell(actor_id).expect("canonical primary");
    let invoke = || {
      Actors::test_schedule_next_work_source(
        actor_id,
        &state,
        &cell.admission,
        cell.resources,
        None,
        0,
      )
    };
    let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    Actors::test_fail_wakeup_placement_with_capacity();
    assert_eq!(
      invoke(),
      Err(crate::scheduler::EnqueueOutcome::WakeupCapacityExhausted)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      before
    );
    crate::ActorFunding::<Test>::remove(actor_id);
    let corrupt = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_eq!(
      invoke(),
      Err(crate::scheduler::EnqueueOutcome::CorruptedTopology)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      corrupt
    );
    crate::ActorFunding::<Test>::insert(actor_id, state.funding.clone());
    assert_eq!(invoke(), Ok((crate::StepControlPlacement::Wakeup, vec![])));
    assert_eq!(
      Actors::actor_run_state(actor_id).encode(),
      state.run_state.encode()
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
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
        Actors::on_idle(1, Weight::MAX);
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
      let (_, cell) = Actors::actor_control_cell(actor_id).expect("canonical primary");
      let invoke = |supplied| {
        Actors::test_schedule_next_work_source(
          actor_id,
          &state,
          &cell.admission,
          cell.resources,
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
    let cursor_before = Actors::materialization_family_cursor();

    assert_eq!(Actors::on_idle(1, Weight::MAX), Weight::zero());
    assert_eq!(Actors::materialization_family_cursor(), cursor_before);
  });
}

#[test]
fn block_initialize_freezes_the_pre_external_ticket_frontier() {
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
    assert_eq!(Actors::prepass_execution_cutoff(), Some((1, 7)));
    let state = Actors::block_resource_state().expect("resource state is opened once");
    assert_eq!(state.phase(), crate::BlockResourcePhase::ExternalPhase);
    assert_eq!(state.usage().actor_control_used(), empty_prepass);
    assert!(!state.optional_actor_work_halted());

    crate::ActorReadyTail::<Test>::put(9);
    assert_noop!(
      Actors::actor_prepass(RuntimeOrigin::none()),
      crate::Error::<Test>::PrepassDuplicateOrStale
    );
    assert_eq!(Actors::prepass_execution_cutoff(), Some((1, 7)));
    let duplicate = Actors::block_resource_state().expect("duplicate preserves state");
    assert_eq!(duplicate.usage(), state.usage());
    assert!(!duplicate.optional_actor_work_halted());
  });
}

#[test]
fn prepass_materialization_stays_behind_the_frozen_cutoff_until_next_block() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(1), None, inert_contract_steps());
    run_prepass();
    Actors::on_idle(1, Weight::MAX);
    Actors::on_finalize(1);

    frame_system::Pallet::<Test>::set_block_number(2);
    let cutoff = Actors::next_queue_ticket();
    run_prepass();
    assert_eq!(Actors::prepass_execution_cutoff(), Some((2, cutoff)));
    let deferred = Actors::active_actor_view(actor_id).expect("timer Actor remains active");
    assert_eq!(deferred.cycle_nonce, 0);
    assert!(deferred.pending_signal);
    assert!(deferred.queue_ticket.is_some_and(|ticket| ticket >= cutoff));
    Actors::on_idle(2, Weight::MAX);
    Actors::on_finalize(2);

    frame_system::Pallet::<Test>::set_block_number(3);
    run_prepass();
    assert_eq!(
      Actors::active_actor_view(actor_id).map(|actor| actor.cycle_nonce),
      Some(1)
    );
  });
}

#[test]
fn hook_order_fixture_exposes_external_ticket_after_prepass_cutoff() {
  new_test_ext().execute_with(|| {
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let before_external = Actors::next_queue_ticket();

    let consumed = run_actor_hook_order_with_external(
      1,
      || {
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          actor_id
        ));
      },
      Weight::MAX,
    );

    assert_eq!(
      Actors::prepass_execution_cutoff(),
      Some((1, before_external))
    );
    assert!(
      Actors::prepass_execution_cutoff()
        .is_some_and(|(_, cutoff)| Actors::next_queue_ticket() > cutoff)
    );
    let hot = Actors::actor_hot(actor_id).expect("Actor remains pending for the next block");
    assert!(hot.pending_signal);
    assert!(hot.queue_ticket.is_some());
    assert_eq!(
      Actors::active_actor_view(actor_id).map(|actor| actor.cycle_nonce),
      Some(0)
    );
    assert!(
      <TestWeightInfo as crate::WeightInfo>::scheduler_on_initialize_cutoff().all_lte(consumed)
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    run_prepass();
    Actors::on_idle(2, Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id).map(|actor| actor.cycle_nonce),
      Some(1)
    );
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
fn activation_preflight_selects_prime_schedule_without_mutating_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);

    assert_eq!(Actors::test_activation_plan_kind(actor_id), Ok(4));
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before
    );
    let actor = Actors::active_actor_view(actor_id).expect("active actor");
    assert!(!actor.pending_signal);
    assert!(actor.queue_ticket.is_none());
  });
}

#[test]
fn ready_activation_plan_commits_its_frozen_destination_from_canonical_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let loaded = if cfg!(feature = "runtime-benchmarks") {
      Actors::load_actor_state(actor_id)
    } else {
      Actors::load_frame_actor_state(actor_id)
    };
    let crate::LoadedActorStateOf::Active(state) = loaded else {
      panic!("active Actor state");
    };
    let plan = Actors::preflight_activation_loaded(actor_id, state)
      .expect("ready activation preflight succeeds");
    assert!(matches!(
      &plan.action,
      crate::scheduler::ActivationAction::EnqueueReady(Ok(_))
    ));

    assert_eq!(
      Actors::commit_activation_plan(plan),
      Ok(crate::scheduler::ActivationOutcome::Latched)
    );
    let hot = Actors::actor_hot(actor_id).expect("committed hot authority");
    assert!(hot.pending_signal);
    assert!(hot.queue_ticket.is_some());
    assert_eq!(hot.unsuccessful_attempt_streak, 0);
    let (location, _, frame_hot, _) = Actors::load_frame_control_authority(actor_id)
      .expect("ready destination owns frame authority");
    assert!(matches!(
      location,
      crate::ActorControlLocation::Ready { .. }
    ));
    assert_eq!(frame_hot, hot);
  });
}

#[test]
fn deferred_activation_preserves_source_on_failure_and_publishes_exact_waiting() {
  for saturated in [false, true] {
    for empty in [false, true] {
      new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(1);
        let window = (!saturated).then_some(ScheduleWindow {
          start: 10,
          end: 10 + <<Test as crate::Config>::MinWindowLength as Get<u64>>::get(),
        });
        let steps = if empty {
          BoundedVec::default()
        } else {
          inert_contract_steps()
        };
        let actor_id = create_system_with(ALICE, manual_schedule(), window, steps);
        if saturated {
          seed_saturated_tombstone_queue();
        }
        let state = Actors::active_actor_state(actor_id).expect("canonical activation source");
        let plan = Actors::preflight_activation_loaded(actor_id, state).expect("valid preflight");
        if saturated {
          assert!(matches!(
            plan.action,
            crate::scheduler::ActivationAction::EnqueueReady(Err(
              crate::EnqueueOutcome::CapacityUnavailable
            ))
          ));
        } else {
          assert!(matches!(
            plan.action,
            crate::scheduler::ActivationAction::PrimeSchedule(Ok(
              crate::scheduler::PrimeSchedulePlan::BlockWakeup(10)
            ))
          ));
        }
        let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        Actors::test_fail_wakeup_placement_with_capacity();
        assert_noop!(
          Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor_id),
          Error::<Test>::QueueCapacityUnavailable
        );
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
          before
        );
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          actor_id
        ));
        let hot = Actors::actor_hot(actor_id).expect("published activation");
        assert!(hot.pending_signal);
        assert!(hot.queue_ticket.is_none());
        assert_eq!(
          hot.wakeup_pointer.expect("Waiting pointer").block,
          WakeupKey::Block(if saturated { 2 } else { 10 })
        );
        assert!(matches!(
          Actors::actor_control_cell(actor_id)
            .expect("canonical primary")
            .0,
          crate::ActorControlLocation::Waiting { .. }
        ));
        #[cfg(feature = "try-runtime")]
        assert_ok!(Actors::do_try_state());
      });
    }
  }
}

#[test]
fn activation_preflight_selects_exact_block_wakeup_without_mutating_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 10,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    mutate_actor_hot_coherent(actor_id, |hot| hot.last_cycle_block = Some(1));
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);

    assert_eq!(Actors::test_activation_plan_kind(actor_id), Ok(5));
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before
    );
    let actor = Actors::active_actor_view(actor_id).expect("active actor");
    assert!(!actor.pending_signal);
    assert!(actor.queue_ticket.is_none());
    assert!(actor.wakeup_pointer.is_none());
  });
}

#[test]
fn cadenced_activation_preflight_carries_queue_authority_without_mutating_state() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: Trigger::cadenced(10),
        cooldown_blocks: 0,
      },
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let actor_before = Actors::active_actor_view(actor_id).expect("active actor");
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);

    assert_eq!(Actors::test_activation_plan_kind(actor_id), Ok(2));
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before
    );
    let actor_after = Actors::active_actor_view(actor_id).expect("active actor");
    assert_eq!(actor_after, actor_before);
    assert!(actor_after.queue_ticket.is_none());
  });
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
fn queue_cohort_preflight_rejects_more_than_runtime_maximum_without_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let cohort_cap = <<Test as crate::Config>::MaxCrossingActorsPerBlock as Get<u32>>::get();
    let actors = (0..=cohort_cap)
      .map(|offset| {
        create_system_with(
          20_000 + u64::from(offset),
          manual_schedule(),
          None,
          contract_steps_with_step(make_step(Task::StopCycle)),
        )
      })
      .collect();
    let root_before =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);

    assert_eq!(
      Actors::test_preflight_queue_over_cap(actors),
      Err(crate::scheduler::EnqueueOutcome::CapacityUnavailable)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root_before
    );
    assert_eq!(Actors::next_queue_ticket(), 0);
    assert_eq!(Actors::queue_occupancy(), 0);
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
    assert_eq!(Actors::queue_occupancy(), 1);
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

    Actors::on_idle(1, Weight::MAX);

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
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Unsignaled)
    ));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());
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

    Actors::on_idle(1, Weight::MAX);

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
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Unsignaled)
    ));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());
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
    assert!(Actors::actor_funding(actor_id).is_none());
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
    assert_eq!(Actors::queue_tail(), 1);
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert!(
      Actors::actor_hot(actor_id)
        .expect("paused actor")
        .queue_ticket
        .is_none()
    );
    assert_noop!(
      Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ControlMutationRateLimited
    );
    assert_eq!(
      Actors::queue_tail(),
      1,
      "rate-limited resume must not append"
    );

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::signed(ALICE), actor_id));
    assert_eq!(Actors::queue_tail(), 2);
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id),
      Error::<Test>::ControlMutationRateLimited
    );
    assert_eq!(
      Actors::queue_tail(),
      2,
      "rate-limited pause must not create a tombstone"
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
      run_idle(Weight::MAX);

      if predicate_stop {
        assert!(has_actor_event(|event| matches!(
          event,
          Event::StepSkipped {
            actor_id: id,
            step_index: 0,
            reason: StepSkippedReason::PreconditionFalse,
            ..
          } if *id == actor_id
        )));
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
      assert!(Actors::actor_hot(actor_id).is_some());
      assert!(Actors::actor_control_cell(actor_id).is_some());
      #[cfg(feature = "try-runtime")]
      assert_ok!(crate::Pallet::<Test>::do_try_state());
    });
  }
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn canonical_execution_preserves_running_successor_and_q1() {
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
    Actors::on_idle(1, Weight::MAX);
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
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());
    Actors::on_idle(1, Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id)
        .expect("same-block retry retains the Running successor")
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
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());

    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    run_prepass();
    Actors::on_idle(2, Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());
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
    Actors::on_idle(1, Weight::MAX);
    let run = Actors::actor_run_state(actor_id).expect("middle-Step Run survives");
    assert_eq!(run.cursor, 1);
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());

    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary { actor_id: id, .. } if *id == actor_id
    )));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());
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
    System::reset_events();

    Actors::on_idle(1, Weight::MAX);

    let state = Actors::active_actor_state(actor_id).expect("paused authority remains active");
    assert!(state.hot.lifecycle.is_paused());
    assert!(state.hot.pending_signal);
    assert!(state.hot.queue_ticket.is_none());
    assert_eq!(
      crate::ActorStateHolds::<Test>::get(actor_id),
      Some(installed_hold)
    );
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Unsignaled)
    ));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());
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

    Actors::execute_cycle(Weight::MAX);

    let state = Actors::active_actor_state(actor_id).expect("breaker-skipped authority remains");
    assert!(state.hot.pending_signal);
    assert!(state.hot.queue_ticket.is_some());
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Ready { .. })
    ));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());
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

#[test]
fn queued_actor_is_preserved_when_proof_budget_cannot_admit_probe() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let scan_weight =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(
        1,
      );
    let budget = TestBlockResourceBudget::get();
    let mut resource_state = crate::BlockResourceState::new(1);
    assert_eq!(resource_state.begin_prepass(), Ok(()));
    assert_eq!(resource_state.open_external_phase(), Ok(()));
    assert_eq!(resource_state.begin_drain(), Ok(()));
    let pass = Actors::execute_cycle_to_cutoff_with_resources(
      Weight::from_parts(
        u64::MAX,
        scan_weight
          .proof_size()
          .saturating_add(Actors::scheduler_actor_probe_weight_upper().proof_size())
          .saturating_sub(1),
      ),
      Actors::next_queue_ticket(),
      &mut resource_state,
      budget.limits(),
      crate::BlockResourceDomain::ActorDrainEffect,
      budget.limits().actor_control(),
    );
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert_eq!(resource_state.usage().actor_control_used(), pass.consumed);
    assert_eq!(resource_state.usage().actor_effect_used(), Weight::zero());
    let instance = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert!(instance.pending_signal);
    assert_eq!(instance.cycle_nonce, 0);
    assert!(
      Actors::actor_hot(actor_id)
        .expect("queued actor")
        .queue_ticket
        .is_some()
    );
  });
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
    // With MaxExecutionsPerBlock=3 and mixed User/System contention,
    // run enough blocks for the bounded queue to service the System actor.
    for block in 2..=20 {
      frame_system::Pallet::<Test>::set_block_number(block);
      run_idle(Weight::MAX);
    }
    let system = Actors::active_actor_view(system_id).expect("system Actors exists");
    assert!(
      system.cycle_nonce >= 1,
      "system actor must execute at least once over 20 blocks (nonce={})",
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
      frame_system::Pallet::<Test>::set_block_number(block);
      run_idle(Weight::MAX);
    }
    let system = Actors::active_actor_view(system_id).expect("system Actors exists");
    assert!(system.cycle_nonce >= 1);
  });
}

#[test]
fn paged_enqueue_coalesces_without_a_per_block_insertion_cap() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let total = 32u32 + 7;
    for _ in 0..total {
      let id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
    }
    assert_eq!(
      Actors::queue_tail().saturating_sub(Actors::queue_head()),
      u64::from(total)
    );
    assert_eq!(Actors::wakeup_cursor_len(), 0);
  });
}

#[test]
fn tombstone_drain_rolls_back_on_live_occupancy_exceeding_span() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(enqueue_latched_actor(actor_id));
    assert_eq!(Actors::paged_invalidate(actor_id), Some(0));
    restore_structural_queue_tombstone(actor_id);
    crate::ActorReadyOccupancy::<Test>::put(2);
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    assert_eq!(
      Actors::paged_drain_tombstones(Actors::queue_tail(), 1),
      Err(crate::EnqueueOutcome::CorruptedTopology),
    );

    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
  });
}

#[test]
fn tombstone_drain_rolls_back_on_cross_page_corruption() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let page_size = 32u32;
    for _ in 0..=page_size {
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert!(enqueue_latched_actor(actor_id));
      assert!(Actors::paged_invalidate(actor_id).is_some());
      restore_structural_queue_tombstone(actor_id);
    }
    crate::ActorReadyFrameChunks::<Test>::remove(1);
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    assert_eq!(
      Actors::paged_drain_tombstones(Actors::queue_tail(), page_size + 1),
      Err(crate::EnqueueOutcome::CorruptedTopology),
    );

    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
    assert!(
      crate::ActorReadyFrameChunks::<Test>::get(0).is_some(),
      "first page deletion rolls back"
    );
  });
}

#[test]
fn tombstone_drain_missing_current_page_is_blocked_without_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(enqueue_latched_actor(actor_id));
    assert!(Actors::paged_invalidate(actor_id).is_some());
    restore_structural_queue_tombstone(actor_id);
    crate::ActorReadyFrameChunks::<Test>::remove(0);
    let cutoff = Actors::queue_tail();
    let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    let (kind, entry, scanned) = Actors::test_head_discovery(cutoff, 1, 0, Weight::MAX);

    assert_eq!((kind, entry, scanned), (3, None, 0));
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
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
fn try_state_rejects_frame_and_physical_queue_mismatch() {
  for corruption in 0u8..3 {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      let other = create_system_with(BOB, manual_schedule(), None, inert_contract_steps());
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      match corruption {
        0 => crate::ActorControlLocators::<Test>::insert(
          actor_id,
          crate::ActorControlLocation::Ready { ticket: 1 },
        ),
        1 => crate::ActorReadyFrameChunks::<Test>::mutate(0, |maybe| {
          maybe.as_mut().expect("queue page")[0]
            .as_mut()
            .expect("live cell")
            .actor_id = other;
        }),
        2 => {
          crate::ActorReadyFrameChunks::<Test>::mutate(0, |maybe| {
            let page = maybe.as_mut().expect("queue page");
            let first = page.get_mut(0).expect("first queue slot").take();
            let second = ::core::mem::replace(page.get_mut(1).expect("second queue slot"), first);
            *page.get_mut(0).expect("first queue slot") = second;
          });
          crate::ActorReadyTail::<Test>::put(2);
        }
        _ => unreachable!(),
      }
      assert!(
        crate::Pallet::<Test>::do_try_state().is_err(),
        "corruption case {corruption}"
      );
    });
  }
}

#[test]
fn live_occupancy_and_tombstone_span_are_independently_bounded() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut actors = Vec::new();
    for _ in 0..5 {
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
      actors.push(actor_id);
    }
    assert_eq!(Actors::combined_queue_occupancy(), 5);
    // Tombstones retain physical span but no longer contribute to live occupancy.
    for actor_id in &actors[0..4] {
      assert!(Actors::paged_invalidate(*actor_id).is_some());
      restore_structural_queue_tombstone(*actor_id);
    }
    assert_eq!(
      Actors::combined_queue_occupancy(),
      1,
      "invalidation removes exactly four live cells"
    );
    assert_eq!(Actors::queue_tail() - Actors::queue_head(), 5);
    let cutoff = Actors::next_queue_ticket();
    let drained = Actors::paged_drain_tombstones(cutoff, 10).expect("valid queue topology");
    assert_eq!(drained.tombstones_skipped, 4);
    assert_eq!(
      Actors::combined_queue_occupancy(),
      1,
      "draining tombstones does not decrement live occupancy"
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn paged_queue_uses_one_live_actor_ticket_and_lazy_invalidation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());

    assert!(enqueue_latched_actor(actor_id));
    assert!(Actors::paged_enqueue(actor_id));
    assert_eq!(Actors::queue_head(), 0);
    assert_eq!(Actors::queue_tail(), 1);
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot state").queue_ticket,
      Some(0)
    );
    assert_eq!(
      crate::ActorReadyFrameChunks::<Test>::get(0)
        .expect("head page")
        .iter()
        .filter(|cell| cell.is_some())
        .count(),
      1
    );

    assert_eq!(Actors::paged_invalidate(actor_id), Some(0));
    restore_structural_queue_tombstone(actor_id);
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot state").queue_ticket,
      None
    );
    assert!(crate::ActorReadyFrameChunks::<Test>::get(0).expect("tombstone page")[0].is_none());
    let drained = Actors::paged_drain_tombstones(Actors::next_queue_ticket(), 1)
      .expect("invalidated head drains as a tombstone");
    assert_eq!(drained.tombstones_skipped, 1);
    assert_eq!(Actors::queue_head(), 1);
    assert_eq!(Actors::queue_tail(), 1);
    assert!(crate::ActorReadyFrameChunks::<Test>::get(0).is_none());
  });
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

fn consume_structural_ready_head(ticket: u64) {
  let (_, entry) = Actors::paged_head_entry().expect("live structural head");
  let (_, mut cell) = Actors::actor_control_cell(entry.actor_id).expect("source primary");
  assert_eq!(cell.hot.cycle_state, CycleState::Idle);
  assert!(Actors::paged_consume_head(ticket));
  assert!(Actors::actor_control_cell(entry.actor_id).is_none());
  cell.hot.pending_signal = false;
  cell.eligible_at = None;
  crate::ActorUnsignaledControlCells::<Test>::insert(entry.actor_id, cell);
  crate::ActorControlLocators::<Test>::insert(
    entry.actor_id,
    crate::ActorControlLocation::Unsignaled,
  );
}

#[test]
fn paged_queue_crosses_and_reclaims_page_boundaries_without_prefix_rewrites() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut actors = Vec::new();
    for _ in 0..33 {
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert!(enqueue_latched_actor(actor_id));
      actors.push(actor_id);
    }
    assert_eq!(Actors::queue_tail(), 33);
    assert_eq!(
      crate::ActorReadyFrameChunks::<Test>::get(0)
        .expect("full first page")
        .iter()
        .filter(|cell| cell.is_some())
        .count(),
      32
    );
    assert_eq!(
      crate::ActorReadyFrameChunks::<Test>::get(1)
        .expect("partial second page")
        .iter()
        .filter(|cell| cell.is_some())
        .count(),
      1
    );

    for (ticket, actor_id) in actors.iter().take(32).copied().enumerate() {
      let (position, entry) = Actors::paged_head_entry().expect("queue head exists");
      assert_eq!(
        (position, entry.ticket, entry.actor_id),
        (ticket as u64, ticket as u64, actor_id)
      );
      consume_structural_ready_head(ticket as u64);
    }
    assert_eq!(Actors::queue_head(), 32);
    assert!(crate::ActorReadyFrameChunks::<Test>::get(0).is_none());
    assert_eq!(
      crate::ActorReadyFrameChunks::<Test>::get(1)
        .expect("remaining head page")
        .iter()
        .filter(|cell| cell.is_some())
        .count(),
      1
    );

    consume_structural_ready_head(32);
    assert_eq!(Actors::queue_head(), 33);
    assert_eq!(Actors::queue_tail(), 33);
    assert!(crate::ActorReadyFrameChunks::<Test>::get(1).is_none());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn paged_queue_replacement_ticket_leaves_old_entry_as_tombstone() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_a = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let actor_b = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(enqueue_latched_actor(actor_a));
    assert_eq!(Actors::paged_invalidate(actor_a), Some(0));
    restore_structural_queue_tombstone(actor_a);
    assert!(enqueue_latched_actor(actor_b));
    assert!(enqueue_latched_actor(actor_a));

    assert_eq!(
      Actors::actor_hot(actor_a)
        .expect("actor A hot")
        .queue_ticket,
      Some(2)
    );
    assert_eq!(
      Actors::actor_hot(actor_b)
        .expect("actor B hot")
        .queue_ticket,
      Some(1)
    );
    assert_eq!(Actors::queue_head(), 0);
    assert!(crate::ActorReadyFrameChunks::<Test>::get(0).expect("tombstone page")[0].is_none());
    assert!(Actors::paged_head_entry().is_none());
    let drained = Actors::paged_drain_tombstones(Actors::next_queue_ticket(), 1)
      .expect("replacement head drains as a tombstone");
    assert_eq!(drained.tombstones_skipped, 1);
    assert_eq!(
      Actors::actor_hot(actor_a)
        .expect("actor A hot")
        .queue_ticket,
      Some(2)
    );
    let (position, entry) = Actors::paged_head_entry().expect("queue head exists");
    assert_eq!((position, entry.ticket, entry.actor_id), (1, 1, actor_b));
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn paged_tombstone_drain_is_scan_bounded_and_reclaims_multiple_pages() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut actors = Vec::new();
    for _ in 0..65 {
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert!(enqueue_latched_actor(actor_id));
      actors.push(actor_id);
    }
    for actor_id in actors {
      assert!(Actors::paged_invalidate(actor_id).is_some());
      restore_structural_queue_tombstone(actor_id);
    }

    let cutoff = Actors::queue_tail();
    let first = Actors::paged_drain_tombstones(cutoff, 10).expect("valid first drain");
    assert_eq!(first.entries_scanned, 10);
    assert_eq!(first.tombstones_skipped, 10);
    assert_eq!(first.pages_touched, 1);
    assert_eq!(first.pages_deleted, 0);
    assert_eq!(Actors::queue_head(), 10);

    let rest = Actors::paged_drain_tombstones(cutoff, 55).expect("valid remaining drain");
    assert_eq!(rest.entries_scanned, 55);
    assert_eq!(rest.tombstones_skipped, 55);
    assert_eq!(rest.pages_touched, 3);
    assert_eq!(rest.pages_deleted, 3);
    assert_eq!(Actors::queue_head(), 65);
    assert_eq!(Actors::queue_tail(), 65);
    assert!(crate::ActorReadyFrameChunks::<Test>::get(0).is_none());
    assert!(crate::ActorReadyFrameChunks::<Test>::get(1).is_none());
    assert!(crate::ActorReadyFrameChunks::<Test>::get(2).is_none());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn mandatory_prepass_reclaims_saturated_fifo_without_domain_spill() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    seed_saturated_tombstone_queue();
    let cutoff = Actors::queue_tail();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    frame_system::Pallet::<Test>::set_block_number(2);
    run_prepass();
    assert!(Actors::queue_head() > 0);
    let actor = Actors::active_actor_view(actor_id).expect("deferred Actor is materialized");
    assert_eq!(actor.queue_ticket, Some(cutoff));
    assert!(actor.pending_signal);
    assert_eq!(actor.cycle_nonce, 0);
    assert_eq!(Actors::prepass_execution_cutoff(), Some((2, cutoff)));
    let state = Actors::block_resource_state().expect("prepass resource state exists");
    assert_eq!(state.usage().actor_effect_used(), Weight::zero());
    assert_eq!(state.outstanding_reservations(), 0);
    assert_eq!(state.phase(), crate::BlockResourcePhase::ExternalPhase);
  });
}

#[test]
fn saturated_tombstone_queue_reclaims_head_before_ingress_and_recovers_deferred_work() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let capacity = <<Test as crate::Config>::MaxQueueLength as Get<u32>>::get();
    for page_id in 0..capacity.div_ceil(32) {
      crate::ActorReadyFrameChunks::<Test>::insert(
        u64::from(page_id),
        BoundedVec::try_from(vec![None; 32]).expect("canonical tombstone page"),
      );
    }
    crate::ActorReadyHead::<Test>::put(0);
    crate::ActorReadyTail::<Test>::put(u64::from(capacity));
    crate::ActorReadyOccupancy::<Test>::put(0);

    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    let cleanup_budget =
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_on_idle_base()
        .saturating_add(
          <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::materialization_coordinator_base(),
        )
        .saturating_add(
          <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::block_resource_finalize(),
        )
        .saturating_add(
        <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(
          1,
        ),
      );
    Actors::on_idle(1, cleanup_budget);
    assert_eq!(
      Actors::queue_head(),
      1,
      "saturated stale head must make progress before ingress"
    );
    assert_eq!(Actors::queue_tail(), u64::from(capacity));

    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      false
    ));
    Actors::on_idle(2, Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("deferred actor survives")
        .cycle_nonce,
      1
    );
    assert_eq!(Actors::queue_head(), Actors::queue_tail());
    assert_eq!(scheduled_wakeup_block(actor_id), None);
  });
}

#[test]
fn queue_ticket_exhaustion_closes_through_the_unified_sink() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      on_address_event_schedule(SourceFilter::Any, AssetFilter::Any),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    let sovereign = sovereign_account(actor_id);
    // Monotonic ticket namespace at the ceiling closes through the single
    // scheduler-exhaustion terminal owner.
    crate::ActorReadyHead::<Test>::put(u64::MAX);
    crate::ActorReadyTail::<Test>::put(u64::MAX);
    let actor_before = native_balance(&sovereign);
    assert_ok!(Actors::notify_address_event(
      actor_id,
      TestAsset::Native,
      100,
      &ALICE
    ));
    assert_eq!(native_balance(&sovereign), actor_before);
    assert!(Actors::active_actor_view(actor_id).is_none());
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
fn stale_close_entry_drains_as_tombstone_before_recreated_slot_runs() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    // Create a User actor at slot 3, trigger it into the FIFO, then close it while queued.
    let first = create_user_with_slot(
      ALICE,
      3,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    fund_native(first, 1_000_000_000_000_000);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), first));
    assert!(
      Actors::actor_hot(first)
        .expect("queued actor")
        .queue_ticket
        .is_some(),
      "closed actor is physically queued"
    );
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), first));
    assert!(Actors::actor_hot(first).is_none(), "actor is closed");

    // Recreate at the same slot; the stale queue entry must not signal the fresh identity.
    let second = create_user_with_slot(
      ALICE,
      3,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    fund_native(second, 1_000_000_000_000_000);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), second));
    let second_ticket = Actors::actor_hot(second)
      .and_then(|hot| hot.queue_ticket)
      .expect("recreated actor has its own ticket");
    assert_ne!(
      second_ticket, 0,
      "fresh ticket must differ from the stale one"
    );

    // The stale head is a tombstone (actor closed, ticket cleared) and drains in physical order.
    let cutoff = Actors::next_queue_ticket();
    let drained = Actors::paged_drain_tombstones(cutoff, 10).expect("valid stale drain");
    assert_eq!(drained.tombstones_skipped, 1, "stale entry is a tombstone");
    assert_eq!(Actors::queue_head(), 1);

    // No CycleStarted for the recreated actor from the stale entry; the live head is the fresh
    // actor only after the fresh trigger.
    assert!(Actors::actor_hot(second).is_some_and(|hot| hot.pending_signal));
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
    let started: Vec<_> = frame_system::Pallet::<Test>::events()
      .into_iter()
      .filter_map(|record| match record.event {
        RuntimeEvent::Actors(Event::CycleStarted { actor_id, .. }) => Some(actor_id),
        _ => None,
      })
      .collect();
    assert_eq!(started, vec![second], "only the recreated actor executes");
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn paged_tombstone_drain_stops_at_live_head_and_honors_cutoff() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let stale = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let live = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let appended_after_cutoff =
      create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), stale));
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), live));
    let cutoff = Actors::queue_tail();
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      appended_after_cutoff
    ));
    assert_eq!(Actors::paged_invalidate(stale), Some(0));
    restore_structural_queue_tombstone(stale);
    assert_eq!(Actors::paged_invalidate(appended_after_cutoff), Some(2));
    restore_structural_queue_tombstone(appended_after_cutoff);

    let drained = Actors::paged_drain_tombstones(cutoff, 100).expect("valid cutoff drain");
    assert_eq!(drained.entries_scanned, 2);
    assert_eq!(drained.tombstones_skipped, 1);
    assert_eq!(drained.pages_touched, 1);
    assert_eq!(Actors::queue_head(), 1);
    assert_eq!(Actors::queue_tail(), 3);
    assert_eq!(
      Actors::actor_hot(live).expect("live actor").queue_ticket,
      Some(1)
    );

    let (_, mut live_cell) = Actors::actor_control_cell(live).expect("live primary before consume");
    assert!(Actors::paged_consume_head(1));
    assert!(Actors::actor_control_cell(live).is_none());
    live_cell.hot.pending_signal = false;
    live_cell.eligible_at = None;
    crate::ActorUnsignaledControlCells::<Test>::insert(live, live_cell);
    crate::ActorControlLocators::<Test>::insert(live, crate::ActorControlLocation::Unsignaled);
    let after_live = Actors::paged_drain_tombstones(cutoff, 100).expect("valid post-live drain");
    assert_eq!(
      after_live.entries_scanned, 0,
      "ticket 2 is beyond the captured cutoff"
    );
    assert_eq!(Actors::queue_head(), 2);
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn paged_scheduler_preserves_the_unexecuted_fifo_suffix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let max_exec: u32 = <Test as crate::Config>::MaxExecutionsPerBlock::get();
    let total = max_exec + 2;
    let mut ids = Vec::new();
    for _ in 0..total {
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      ids.push(actor_id);
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor_id
      ));
    }
    frame_system::Pallet::<Test>::set_block_number(2);
    run_idle(Weight::MAX);
    assert_eq!(Actors::queue_tail().saturating_sub(Actors::queue_head()), 2);
    assert_eq!(
      Actors::paged_head_entry().map(|(_, entry)| entry.actor_id),
      Some(ids[max_exec as usize])
    );
    frame_system::Pallet::<Test>::set_block_number(3);
    run_idle(Weight::MAX);
    assert_eq!(Actors::queue_head(), Actors::queue_tail());
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
    // Two manual triggers in the same block latch one pending_signal and one FIFO ticket;
    // the post-worker cutoff enforces executions(A, B) <= 1.
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert_eq!(
      Actors::actor_hot(actor_id)
        .and_then(|hot| hot.queue_ticket)
        .expect("one live ticket"),
      0
    );
    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
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
    assert_eq!(
      Actors::queue_head(),
      Actors::queue_tail(),
      "FIFO fully consumed"
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
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
    assert_eq!(scheduled_wakeup_block(actor_id), Some(3));
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
    frame_system::Pallet::<Test>::set_block_number(3);
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
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);

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

    frame_system::Pallet::<Test>::set_block_number(2);
    frame_system::Pallet::<Test>::reset_events();
    run_idle(Weight::MAX);
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

    assert_eq!(Actors::next_queue_ticket(), 4);
    let tickets: Vec<_> = crate::ActorReadyFrameChunks::<Test>::get(0)
      .expect("canonical queue page")
      .into_iter()
      .enumerate()
      .filter_map(|(slot, cell)| cell.map(|_| slot as u64))
      .collect();
    assert_eq!(tickets, vec![0, 1, 2, 3]);
    assert_eq!(Actors::queue_occupancy(), 4);
  });
}

#[test]
fn canonical_tombstones_cannot_bypass_the_oldest_live_head() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let scan = <TestWeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1);
    let old_user = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      old_user
    ));
    for _ in 0..3 {
      let tombstone = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        tombstone
      ));
      assert!(Actors::paged_invalidate(tombstone).is_some());
      restore_structural_queue_tombstone(tombstone);
    }
    let later_system = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      later_system
    ));
    let cutoff = Actors::next_queue_ticket();
    let (state, entry, _) = Actors::test_head_discovery(cutoff, 1, 0, scan);
    assert_eq!(state, 1);
    assert_eq!(entry.map(|entry| entry.actor_id), Some(old_user));

    assert_eq!(Actors::paged_invalidate(old_user), Some(0));
    restore_structural_queue_tombstone(old_user);
    let (state, entry, scanned) = Actors::test_head_discovery(cutoff, 5, 0, scan.saturating_mul(5));
    assert_eq!((state, scanned), (1, 5));
    assert_eq!(entry.map(|entry| entry.actor_id), Some(later_system));
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
    let mut wakeup_meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut wakeup_meter);
    assert!(Actors::actor_hot(actor_id).is_some_and(|hot| hot.trigger_wakeup_pointer.is_none()));
    let bob_before = native_balance(&BOB);

    let _ = Actors::execute_cycle(Weight::MAX);

    assert_eq!(native_balance(&BOB), bob_before + 10);
    let state = Actors::active_actor_state(actor_id).expect("Actor remains active");
    assert_eq!(state.identity.cycle_nonce, 1);
    assert!(state.hot.trigger_wakeup_pointer.is_some());
  });
}

#[test]
fn scheduler_close_rolls_back_on_fifo_topology_corruption() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      transfer_contract_steps(BOB, 10),
    );
    fund_native(actor_id, 1_000);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    frame_system::Pallet::<Test>::set_block_number(102);
    Actors::test_corrupt_queue_before_close_consume();
    let actor_before = Actors::active_actor_view(actor_id).expect("actor");
    let events_before = System::events();
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

    let _ = Actors::execute_cycle(Weight::MAX);

    assert_eq!(Actors::active_actor_view(actor_id), Some(actor_before));
    assert_eq!(System::events(), events_before);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before
    );
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
    frame_system::Pallet::<Test>::set_block_number(cadence_due);
    run_idle(Weight::MAX);

    let hot = Actors::actor_hot(actor_id).expect("suspended cadence actor");
    assert_eq!(
      hot.trigger_wakeup_pointer.map(|pointer| pointer.tick),
      Some(cadence_due + 100)
    );
    assert!(hot.queue_ticket.is_some());
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
    run_idle(Weight::MAX);
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
    run_idle(Weight::MAX);
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
fn cancellation_requeues_a_signal_latched_for_the_next_logical_run() {
  new_test_ext().execute_with(|| {
    let actor_id = create_suspended_system_retry(1);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    assert!(Actors::pending_signal(actor_id));
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      inert_contract_steps(),
      crate::CompletionPolicy::Persistent,
    ));
    let cancelled = Actors::active_actor_view(actor_id).expect("cancelled actor remains");
    assert_eq!(cancelled.cycle_state, CycleState::Idle);
    assert!(cancelled.pending_signal);
    assert!(cancelled.queue_ticket.is_some());

    for block in 2..=4 {
      frame_system::Pallet::<Test>::set_block_number(block);
      Actors::on_initialize(block);
      run_prepass();
      Actors::on_idle(block, Weight::MAX);
      if Actors::active_actor_view(actor_id).is_some_and(|actor| actor.cycle_nonce == 2) {
        break;
      }
    }
    let completed = Actors::active_actor_view(actor_id).expect("next logical cycle completes");
    assert_eq!(completed.cycle_nonce, 2);
    assert!(!completed.pending_signal);
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
fn shared_materialization_remainder_follows_rotated_first_family_after_reserving_minima() {
  new_test_ext().execute_with(|| {
    let shared = Actors::materialization_weight_limit();
    let minima = [
      Actors::materialization_family_minimum(0),
      Actors::materialization_family_minimum(1),
      Actors::materialization_family_minimum(2),
    ];
    let lendable = shared
      .saturating_sub(minima[0])
      .saturating_sub(minima[1])
      .saturating_sub(minima[2]);
    assert!(lendable.ref_time() > 0 && lendable.proof_size() > 0);
    assert_eq!(
      Actors::materialization_family_budget(
        0,
        0,
        lendable,
        crate::MaterializationMinimumReservation::Unavailable,
      ),
      lendable
    );

    for cursor in 0u8..3 {
      let first = Actors::materialization_family_budget(
        cursor,
        0,
        shared,
        crate::MaterializationMinimumReservation::ReserveAllFamilies,
      );
      assert_eq!(first, minima[usize::from(cursor)].saturating_add(lendable));
      let next = cursor.saturating_add(1) % 3;
      let after_first = shared.saturating_sub(first);
      assert_eq!(
        Actors::materialization_family_budget(
          cursor,
          1,
          after_first,
          crate::MaterializationMinimumReservation::ReserveAllFamilies,
        ),
        minima[usize::from(next)]
      );
    }
  });
}

#[test]
fn empty_materialization_families_charge_only_their_measured_probes_and_yield() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let expected = <TestWeightInfo as crate::WeightInfo>::scheduler_on_idle_base()
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::materialization_coordinator_base())
      .saturating_add(
        <TestWeightInfo as crate::WeightInfo>::scheduler_wakeup_cursor_worker_future()
          .saturating_mul(2),
      )
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::crossing_worker_base())
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::observation_fanout_base())
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::block_resource_finalize())
      .saturating_add(<TestWeightInfo as crate::WeightInfo>::scheduler_paged_tombstone_drain(1));

    assert_eq!(Actors::on_idle(1, Weight::MAX), expected);
    assert_eq!(Actors::materialization_family_cursor(), 1);
    assert_eq!(Actors::queue_occupancy(), 0);
    assert_eq!(Actors::crossing_pending_feed_list().count, 0);
    assert_eq!(Actors::dirty_observation_list().count, 0);
  });
}

#[test]
fn mixed_materialization_ticket_order_is_reproducible_from_cursor_and_block_state() {
  assert_eq!(
    mixed_materialization_ticket_trace(),
    mixed_materialization_ticket_trace()
  );
}

#[test]
fn materialization_family_cursor_rotates_deterministically_and_corruption_fails_closed() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let fixed = <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::scheduler_on_idle_base(
    )
    .saturating_add(
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::materialization_coordinator_base(
      ),
    )
    .saturating_add(
      <<Test as crate::Config>::WeightInfo as crate::WeightInfo>::block_resource_finalize(),
    );
    for expected in [1u8, 2, 0] {
      let used = Actors::on_idle(1, Weight::MAX);
      assert!(fixed.all_lte(used));
      assert_eq!(Actors::materialization_family_cursor(), expected);
    }

    crate::MaterializationFamilyCursor::<Test>::put(3);
    let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_eq!(Actors::on_idle(1, Weight::MAX), fixed);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      root_before,
      "invalid coordinator state must not run or rewrite any materialization family"
    );
    #[cfg(feature = "try-runtime")]
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
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
    run_idle(starvation_blocked_budget(actor_id));
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
    for block in 2..=(threshold + 2) {
      frame_system::Pallet::<Test>::set_block_number(block as u64);
      run_idle(starvation_blocked_budget(actor_id));
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
    for block in 1..=threshold {
      frame_system::Pallet::<Test>::set_block_number(u64::from(block));
      run_idle(starvation_blocked_budget(actor_id));
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
      run_idle(starvation_blocked_budget(actor_id));
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
    for block in 1..=threshold {
      frame_system::Pallet::<Test>::set_block_number(block as u64);
      run_idle(starvation_blocked_budget(actor_id));
    }
    GlobalCircuitBreaker::<Test>::put(true);
    frame_system::Pallet::<Test>::set_block_number(threshold.saturating_add(1) as u64);
    run_idle(starvation_blocked_budget(actor_id));
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
    run_idle(starvation_blocked_budget(actor_id));
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
fn breaker_preserves_lower_level_unlatched_terminal_ready_authority() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      Some(ScheduleWindow { start: 2, end: 102 }),
      transfer_contract_steps(BOB, 1),
    );
    fund_native(actor_id, 1_000);
    assert_eq!(scheduled_wakeup_block(actor_id), Some(103));
    assert!(
      !Actors::actor_hot(actor_id)
        .expect("untriggered Actor")
        .pending_signal
    );

    System::set_block_number(103);
    // Exercise the canonical lower-level consume/publication contract. The high-level
    // due worker intentionally substitutes WindowExpired close before enqueue instead.
    assert_ok!(polkadot_sdk::frame_support::storage::with_transaction(
      || {
        let (due, stats) = Actors::wakeup_substrate_drain_key(WakeupKey::Block(103), 1);
        assert_eq!(stats.entries_scanned, 1);
        assert_eq!(due.len(), 1);
        let (due_actor, state, admission, loaded_step) =
          due.into_iter().next().expect("due source");
        assert_eq!(due_actor, actor_id);
        let plan = Actors::preflight_paged_enqueue_authority(
          actor_id,
          state.hot,
          &state.identity,
          state.run_state.as_ref(),
          &admission,
          loaded_step.expect("authored current Step").resources,
        )
        .expect("complete consumed terminal authority admits Ready");
        assert_ok!(Actors::commit_paged_enqueue(plan));
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Ok::<(), DispatchError>(()),
        )
      }
    ));
    let (source_location, source_cell) = Actors::actor_control_cell(actor_id)
      .expect("lower-level due publication retains terminal primary");
    let crate::ActorControlLocation::Ready { ticket } = source_location else {
      panic!("canonical queue publication must materialize Ready");
    };
    assert_eq!(source_cell.hot.cycle_state, CycleState::Idle);
    assert!(!source_cell.hot.pending_signal);
    assert_eq!(source_cell.hot.terminal_at, Some(103));
    assert_eq!(crate::ActorReadyOccupancy::<Test>::get(), 1);
    assert_eq!(Actors::queue_head(), ticket);
    assert_eq!(Actors::queue_tail(), ticket + 1);
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    System::reset_events();
    let custody = (native_balance(&ALICE), native_balance(&BOB));

    Actors::execute_cycle(Weight::MAX);

    let (location, cell) = Actors::actor_control_cell(actor_id)
      .expect("breaker must retain exactly one valid terminal primary");
    assert_eq!(cell.hot.cycle_state, CycleState::Idle);
    assert!(!cell.hot.pending_signal);
    assert_eq!(cell.hot.terminal_at, Some(103));
    assert_eq!(cell.identity.cycle_nonce, source_cell.identity.cycle_nonce);
    assert!(Actors::actor_run_state(actor_id).is_none());
    let crate::ActorControlLocation::Waiting { key, page, slot } = location else {
      panic!("breaker Skip must preserve terminal work through its window wakeup");
    };
    assert_eq!(key, WakeupKey::Block(103));
    let pointer = cell.hot.wakeup_pointer.expect("terminal service pointer");
    assert_eq!(
      (pointer.block, pointer.page_id, pointer.slot),
      (key, page, u32::from(slot))
    );
    assert_eq!(crate::ActorWaitingOccupancies::<Test>::get(key), 1);
    assert_eq!(crate::ActorReadyOccupancy::<Test>::get(), 0);
    assert_eq!(Actors::queue_head(), ticket + 1);
    assert_eq!(Actors::queue_tail(), ticket + 1);
    assert!(!crate::ActorUnsignaledControlCells::<Test>::contains_key(
      actor_id
    ));
    assert_eq!((native_balance(&ALICE), native_balance(&BOB)), custody);
    assert!(!has_actor_event(|event| matches!(event,
      Event::ActorClosed { actor_id: id, .. } | Event::CycleStarted { actor_id: id, .. }
        if *id == actor_id
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
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
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut meter);

    assert_eq!(fee_collections(), vec![at_time_trigger_fee()]);
    let hot = Actors::actor_hot(actor_id).expect("AtTime Actor remains active");
    assert!(hot.pending_signal);
    assert!(hot.queue_ticket.is_some());
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
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(20, &mut meter);
    assert_eq!(fee_collections(), vec![at_time_trigger_fee()]);
  });
}

#[test]
fn busy_at_time_occurrence_charges_and_preserves_independent_run_service() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let steps = BoundedVec::try_from(vec![
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
    .expect("two-Step Contract fits");
    let actor_id = create_user_with(ALICE, Mutability::Mutable, at_time_schedule(2), None, steps);
    fund_native(actor_id, 1_000_000);
    assert_eq!(
      Actors::request_activation(actor_id),
      Ok(crate::scheduler::ActivationOutcome::Latched)
    );
    Actors::execute_cycle(Weight::MAX);
    let run_before = ActorRunStateStore::<Test>::get(actor_id).expect("Pipeline is Running");
    clear_fee_collections();

    frame_system::Pallet::<Test>::set_block_number(3);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(3, &mut meter);

    assert_eq!(fee_collections(), vec![at_time_trigger_fee()]);
    let hot = Actors::actor_hot(actor_id).expect("busy AtTime Actor remains active");
    assert_eq!(hot.cycle_state, CycleState::Running);
    assert!(hot.pending_signal);
    assert!(hot.trigger_wakeup_pointer.is_none());
    assert!(matches!(
      hot.trigger_runtime_state,
      TriggerRuntimeState::AtTime { consumed: true, .. }
    ));
    let run_after = ActorRunStateStore::<Test>::get(actor_id).expect("Pipeline remains Running");
    assert_eq!(run_after.cursor, run_before.cursor);
    assert_eq!(run_after.cycle_nonce, run_before.cycle_nonce);
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
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut meter);

    assert!(fee_collections().is_empty());
    assert!(!Actors::active_actor_exists(actor_id));
    assert_eq!(native_balance(&sovereign), custody_before);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::TriggerAdmissionInsufficient,
      } if *id == actor_id
    )));
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
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut meter);

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

    frame_system::Pallet::<Test>::set_block_number(2);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut meter);
    Actors::execute_cycle(Weight::MAX);

    let state = Actors::active_actor_state(actor_id).expect("AtTime successor remains active");
    assert_eq!(state.identity.cycle_nonce, 1);
    assert!(matches!(
      state.hot.trigger_runtime_state,
      TriggerRuntimeState::AtTime { consumed: true, .. }
    ));
    assert!(matches!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Unsignaled)
    ));
    assert!(!ActorIdentities::<Test>::contains_key(actor_id));
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn at_time_collection_failure_rolls_back_consumption_and_retains_wakeup() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      at_time_schedule(1),
      None,
      inert_contract_steps(),
    );
    let sovereign = sovereign_account(actor_id);
    let before = native_balance(&sovereign);
    set_fail_fee_sink_transfer(true);

    frame_system::Pallet::<Test>::set_block_number(2);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut meter);
    set_fail_fee_sink_transfer(false);

    assert_eq!(native_balance(&sovereign), before);
    let hot = Actors::actor_hot(actor_id).expect("process remains active");
    assert!(!hot.pending_signal);
    assert!(hot.queue_ticket.is_none());
    assert!(hot.trigger_wakeup_pointer.is_some());
    assert!(matches!(
      hot.trigger_runtime_state,
      TriggerRuntimeState::AtTime {
        consumed: false,
        ..
      }
    ));
  });
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
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut meter);
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
fn uninitialized_genesis_cadence_reanchors_from_frame_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(1), None, inert_contract_steps());
    assert_eq!(scheduled_wakeup_block(actor_id), Some(2));
    mutate_actor_hot_coherent(actor_id, |hot| {
      hot.trigger_runtime_state = TriggerRuntimeState::Cadenced { anchor_tick: None };
    });

    frame_system::Pallet::<Test>::set_block_number(100);
    Actors::on_idle(100, Weight::MAX);

    let (_, identity, hot, _) = Actors::load_frame_control_authority(actor_id)
      .expect("reanchored Cadenced frame authority exists");
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
    assert!(Actors::actor_control_cell(actor_id).is_some());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
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

    frame_system::Pallet::<Test>::set_block_number(100);
    Actors::on_idle(100, Weight::MAX);

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
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut meter);
    let first = Actors::actor_hot(actor_id).expect("Cadenced Actor remains active");
    let placement = first.queue_ticket;
    assert!(first.pending_signal);
    assert!(placement.is_some());
    assert!(first.trigger_wakeup_pointer.is_none());

    frame_system::Pallet::<Test>::set_block_number(3);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(3, &mut meter);

    let fee = cadenced_trigger_fee();
    assert_eq!(fee_collections(), vec![fee]);
    let second = Actors::actor_hot(actor_id).expect("Cadenced Actor remains active");
    assert!(second.pending_signal);
    assert_eq!(second.queue_ticket, placement);
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
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut meter);

    let (location, _, frame_hot, _) = Actors::load_frame_control_authority(actor_id)
      .expect("AtTime occurrence retains canonical primary");
    assert!(matches!(
      location,
      crate::ActorControlLocation::Ready { .. }
    ));
    assert!(frame_hot.pending_signal);
    assert!(
      matches!(
        frame_hot.trigger_runtime_state,
        TriggerRuntimeState::AtTime { consumed: true, .. }
      ),
      "{:?}",
      frame_hot.trigger_runtime_state
    );
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
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut meter);

    let (location, _, frame_hot, _) = Actors::load_frame_control_authority(actor_id)
      .expect("Cadenced occurrence retains canonical primary");
    assert!(matches!(
      location,
      crate::ActorControlLocation::Ready { .. }
    ));
    assert!(frame_hot.pending_signal);
    assert!(matches!(
      frame_hot.trigger_runtime_state,
      TriggerRuntimeState::Cadenced {
        anchor_tick: Some(1)
      }
    ));
    assert!(frame_hot.trigger_wakeup_pointer.is_none());
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
fn paused_temporal_occurrence_restores_unsignaled_frame_with_canonical_control() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, timer_schedule(1), None, inert_contract_steps());
    run_idle(Weight::MAX);
    assert_ok!(Actors::pause_actor(RuntimeOrigin::signed(ALICE), actor_id));
    frame_system::Pallet::<Test>::set_block_number(6);

    crate::NextWakeupClock::<Test>::put(WakeupClock::Tick);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    let stats = Actors::drain_overdue_wakeups_cursor(6, &mut meter);
    assert_eq!(stats.entries_scanned, 1);
    assert_eq!(stats.ready_entries, 1);
    assert!(!crate::WakeupWorkerFaultState::<Test>::exists());

    assert_eq!(
      crate::ActorControlLocators::<Test>::get(actor_id),
      Some(crate::ActorControlLocation::Unsignaled)
    );
    let (_, _, hot, _) = Actors::load_frame_control_authority(actor_id)
      .expect("paused temporal Unsignaled frame authority exists");
    assert!(hot.lifecycle.is_paused());
    assert!(hot.pending_signal);
    assert_eq!(hot.queue_ticket, None);
    assert!(ActorIdentities::<Test>::get(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());
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
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut meter);
    assert!(Actors::pending_signal(actor_id));
    Actors::execute_cycle(Weight::MAX);

    let crate::LoadedActorStateOf::Active(state) =
      Actors::load_actor_state_for_frame_control(actor_id)
    else {
      panic!("rearmed Cadenced frame state is active");
    };
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
      Some(3)
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
    assert_eq!(
      crate::ActorWaitingOccupancies::<Test>::get(WakeupKey::Tick(3)),
      1
    );
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[test]
fn busy_cadenced_occurrence_charges_and_preserves_independent_run_service() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let steps = BoundedVec::try_from(vec![
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
    .expect("two-Step Contract fits");
    let actor_id = create_user_with(ALICE, Mutability::Mutable, timer_schedule(1), None, steps);
    fund_native(actor_id, 1_000_000);
    frame_system::Pallet::<Test>::set_block_number(2);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut meter);
    Actors::execute_cycle(Weight::MAX);
    let run_before = ActorRunStateStore::<Test>::get(actor_id).expect("Pipeline is Running");
    clear_fee_collections();

    frame_system::Pallet::<Test>::set_block_number(3);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(3, &mut meter);

    assert_eq!(fee_collections(), vec![cadenced_trigger_fee()]);
    let hot = Actors::actor_hot(actor_id).expect("busy Cadenced Actor remains active");
    assert_eq!(hot.cycle_state, CycleState::Running);
    assert!(hot.pending_signal);
    assert!(hot.trigger_wakeup_pointer.is_none());
    let run_after = ActorRunStateStore::<Test>::get(actor_id).expect("Pipeline remains Running");
    assert_eq!(run_after.cursor, run_before.cursor);
    assert_eq!(run_after.cycle_nonce, run_before.cycle_nonce);
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn busy_cadenced_occurrence_preserves_frame_service_with_canonical_control() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let steps = BoundedVec::try_from(vec![
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
    .expect("two-Step Contract fits");
    let actor_id = create_user_with(ALICE, Mutability::Mutable, timer_schedule(1), None, steps);
    fund_native(actor_id, 1_000_000);
    frame_system::Pallet::<Test>::set_block_number(2);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut meter);
    Actors::execute_cycle(Weight::MAX);
    let run_before = ActorRunStateStore::<Test>::get(actor_id).expect("Pipeline is Running");
    clear_fee_collections();

    frame_system::Pallet::<Test>::set_block_number(3);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    let stats = Actors::drain_overdue_wakeups_cursor(3, &mut meter);

    assert_eq!(stats.entries_scanned, 1);
    assert_eq!(stats.ready_entries, 1);
    assert!(!crate::WakeupWorkerFaultState::<Test>::exists());
    assert_eq!(fee_collections(), vec![cadenced_trigger_fee()]);
    let (_, _, hot, _) = Actors::load_frame_control_authority(actor_id)
      .expect("busy Cadenced frame authority remains active");
    assert_eq!(hot.cycle_state, CycleState::Running);
    assert!(hot.pending_signal);
    assert!(hot.trigger_wakeup_pointer.is_none());
    let run_after = ActorRunStateStore::<Test>::get(actor_id).expect("Pipeline remains Running");
    assert_eq!(run_after.cursor, run_before.cursor);
    assert_eq!(run_after.cycle_nonce, run_before.cycle_nonce);
    assert!(ActorIdentities::<Test>::get(actor_id).is_none());
    assert!(Actors::actor_hot(actor_id).is_some());
    assert!(Actors::actor_control_cell(actor_id).is_some());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
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

    frame_system::Pallet::<Test>::set_block_number(2);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut meter);

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
  });
}

#[test]
fn cadenced_collection_failure_advances_deadline_without_readiness() {
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
    let before = native_balance(&sovereign);
    set_fail_fee_sink_transfer(true);

    frame_system::Pallet::<Test>::set_block_number(2);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(2, &mut meter);
    set_fail_fee_sink_transfer(false);

    assert_eq!(native_balance(&sovereign), before);
    let hot = Actors::actor_hot(actor_id).expect("process remains active");
    assert!(!hot.pending_signal);
    assert!(hot.queue_ticket.is_none());
    assert_eq!(
      hot.trigger_wakeup_pointer.map(|pointer| pointer.tick),
      Some(3)
    );
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::TriggerOccurrenceProcessed { actor_id: id, .. } if *id == actor_id
    )));
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
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_idle(2, Weight::MAX);
    let after_first = Actors::active_actor_view(actor_id).expect("Actors exists");
    assert_eq!(after_first.cycle_nonce, 1);
    assert!(after_first.queue_ticket.is_none());
    assert_eq!(scheduled_wakeup_block(actor_id), Some(3));
    for block in 3..=6 {
      frame_system::Pallet::<Test>::set_block_number(block);
      Actors::on_initialize(block);
      run_prepass();
      Actors::on_idle(block, Weight::MAX);
      let actor = Actors::active_actor_view(actor_id).expect("Actors exists");
      if block % 2 == 1 {
        assert!(actor.pending_signal);
        assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick), 0);
      } else {
        assert!(!actor.pending_signal);
        assert_eq!(crate::WakeupCursorLen::<Test>::get(WakeupClock::Tick), 1);
      }
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
    frame_system::Pallet::<Test>::set_block_number(6);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      0
    );
    assert_eq!(scheduled_wakeup_block(actor_id), None);
    let paused = Actors::actor_hot(actor_id).expect("paused actor");
    assert!(paused.pending_signal);
    assert!(paused.queue_ticket.is_none());
    frame_system::Pallet::<Test>::set_block_number(7);
    assert_ok!(Actors::resume_actor(RuntimeOrigin::root(), actor_id));
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      1
    );
    assert_eq!(scheduled_wakeup_block(actor_id), Some(8));
    frame_system::Pallet::<Test>::set_block_number(8);
    run_idle(Weight::MAX);
    assert_eq!(
      Actors::active_actor_view(actor_id)
        .expect("Actors exists")
        .cycle_nonce,
      2
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
    // Run one block: both actors must execute despite 2000-wide ID gap
    System::set_block_number(2);
    System::reset_events();
    Actors::on_idle(2, Weight::from_parts(u64::MAX, u64::MAX));
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
    assert!(Actors::active_actor_view(id3).is_some());
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
    System::set_block_number(3);
    run_idle(Weight::MAX);
    assert!(Actors::active_actor_view(id3).is_none());
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

#[test]
fn admission_time_close_reasons_require_complete_queue_and_cleanup_budget() {
  let reasons = [
    CloseReason::WindowExpired,
    CloseReason::CycleAdmissionInsufficient,
    CloseReason::CycleNonceExhausted,
    CloseReason::ConsecutiveFailures,
    CloseReason::AutoCloseNonceReached,
  ];
  for reason in reasons {
    assert_scheduler_close_requires_atomic_budget(reason, Weight::from_parts(1, 0));
    assert_scheduler_close_requires_atomic_budget(reason, Weight::from_parts(0, 1));
  }
}

#[test]
fn scheduler_fails_closed_without_consuming_a_corrupt_live_head() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let hot_before = Actors::actor_hot(actor_id).expect("queued actor");
    let head_before = Actors::queue_head();
    let events_before = frame_system::Pallet::<Test>::events();

    ActorFunding::<Test>::remove(actor_id);
    let corrupt_root =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_eq!(
      Actors::try_paged_enqueue(actor_id),
      Err(crate::EnqueueOutcome::CorruptedTopology)
    );
    assert_eq!(
      Actors::try_paged_invalidate(actor_id),
      Err(crate::EnqueueOutcome::CorruptedTopology)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      corrupt_root
    );
    Actors::execute_cycle(Weight::MAX);

    assert_eq!(Actors::queue_head(), head_before);
    assert_eq!(Actors::actor_hot(actor_id), Some(hot_before));
    assert_eq!(frame_system::Pallet::<Test>::events(), events_before);
    assert!(
      Actors::actor_identity(actor_id).is_none(),
      "full classification rejects corruption"
    );
    assert_eq!(
      Actors::actor_control_cell(actor_id)
        .expect("retained primary")
        .1
        .identity
        .cycle_nonce,
      0
    );
  });
}

#[test]
fn mixed_fifo_stops_at_corrupt_actor_without_touching_valid_suffix() {
  for corrupt_index in 0usize..3 {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actors = (0..3)
        .map(|_| create_system_with(ALICE, manual_schedule(), None, inert_contract_steps()))
        .collect::<Vec<_>>();
      for actor_id in &actors {
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          *actor_id
        ));
      }
      let sovereign_accounts = actors
        .iter()
        .map(|actor_id| {
          Actors::actor_identity(*actor_id)
            .expect("identity")
            .sovereign_account
        })
        .collect::<Vec<_>>();
      ActorFunding::<Test>::remove(actors[corrupt_index]);
      let suffix_hot = actors[corrupt_index..]
        .iter()
        .map(|actor_id| Actors::actor_hot(*actor_id).expect("queued actor"))
        .collect::<Vec<_>>();
      let suffix_balances = sovereign_accounts[corrupt_index..]
        .iter()
        .map(native_balance)
        .collect::<Vec<_>>();
      frame_system::Pallet::<Test>::reset_events();

      Actors::execute_cycle(Weight::MAX);

      assert_eq!(Actors::queue_head(), corrupt_index as u64);
      for (index, actor_id) in actors.iter().enumerate() {
        let (_, cell) = Actors::actor_control_cell(*actor_id).expect("primary remains");
        assert_eq!(cell.identity.cycle_nonce, u64::from(index < corrupt_index));
        assert_eq!(
          Actors::actor_identity(*actor_id).is_none(),
          index == corrupt_index
        );
      }
      for (offset, actor_id) in actors[corrupt_index..].iter().enumerate() {
        assert_eq!(
          Actors::actor_hot(*actor_id),
          Some(suffix_hot[offset].clone())
        );
        assert_eq!(
          native_balance(&sovereign_accounts[corrupt_index + offset]),
          suffix_balances[offset]
        );
      }
      let actor_events = System::events()
        .into_iter()
        .filter(|record| matches!(record.event, RuntimeEvent::Actors(..)))
        .count();
      assert_eq!(actor_events, corrupt_index.saturating_mul(3));
      #[cfg(feature = "try-runtime")]
      assert!(crate::Pallet::<Test>::do_try_state().is_err());
    });
  }
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn eligibility_projection_uses_primary_authority_and_rejects_missing_primary() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::WaitingSignal
    );

    Actors::remove_primary_control_cell_inner(actor_id).expect("primary removal succeeds");
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
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    Actors::drain_overdue_wakeups_cursor(21, &mut meter);
    assert_eq!(
      active_eligibility(actor_id).execution_phase,
      ActorExecutionPhase::Ready
    );
    Actors::execute_cycle(Weight::MAX);
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
