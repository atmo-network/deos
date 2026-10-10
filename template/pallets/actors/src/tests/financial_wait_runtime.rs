use super::behavior_reference as oracle;
use super::*;
use crate::{
  ActorAdmissionCertificateOf, ActorFinancialWaits, ActorProcesses, ActorRef, ActorSemanticState,
  ActorSemanticStates, AdmissionCertificateAuthorityProvider, DeadlineHandles,
  FinancialWaitEpisode, FinancialWaitPublicationError, ProcessResidence, ProcessStatus,
  ServiceResidenceKind, UnpaidActorService, weights::WeightInfo,
};

#[derive(Clone, Copy, Debug)]
enum Source {
  AtTime,
  Opening,
  Running,
  Suspended,
}

fn prepared_source(source: Source) -> (ActorRef, UnpaidActorService) {
  prepared_source_with_native_tail(source, false)
}

fn prepared_source_with_native_tail(
  source: Source,
  native_tail: bool,
) -> (ActorRef, UnpaidActorService) {
  prepared_source_with_options(source, native_tail, Mutability::Mutable, false, false)
}

fn prepared_source_with_options(
  source: Source,
  native_tail: bool,
  mutability: Mutability,
  zero_step: bool,
  false_first_predicate: bool,
) -> (ActorRef, UnpaidActorService) {
  prepared_source_with_funding(
    source,
    native_tail,
    mutability,
    zero_step,
    false_first_predicate,
    None,
    None,
    false,
  )
}

fn prepared_source_with_funding(
  source: Source,
  native_tail: bool,
  mutability: Mutability,
  zero_step: bool,
  false_first_predicate: bool,
  funding: Option<crate::FundingSourcePolicyOf<Test>>,
  window: Option<crate::ScheduleWindow<u64>>,
  cadenced: bool,
) -> (ActorRef, UnpaidActorService) {
  prepared_source_with_length(
    source,
    native_tail,
    mutability,
    zero_step,
    false_first_predicate,
    funding,
    window,
    cadenced,
    2,
  )
}

fn prepared_source_with_length(
  source: Source,
  native_tail: bool,
  mutability: Mutability,
  zero_step: bool,
  false_first_predicate: bool,
  funding: Option<crate::FundingSourcePolicyOf<Test>>,
  window: Option<crate::ScheduleWindow<u64>>,
  cadenced: bool,
  step_count: usize,
) -> (ActorRef, UnpaidActorService) {
  set_prepared_financial_wait_weights(true);
  set_prepared_financial_wait_due_weight(true);
  set_prepared_financial_wait_service_weight(true);
  set_financial_return_backing(TestFinancialReturnBacking::Prepaid);
  System::set_block_number(1);
  let mut first = make_step(Task::Transfer {
    to: BOB,
    asset: if matches!(source, Source::Opening) && native_tail {
      TestAsset::Native
    } else {
      TestAsset::Local(1)
    },
    amount: AmountResolution::Fixed(7),
  });
  if false_first_predicate {
    first.precondition = all_conditions(vec![Predicate::BlockNumberAbove {
      threshold: u32::MAX,
    }]);
  }
  let second = if matches!(source, Source::Suspended) {
    temporary_retry_swap_plan()[0].clone()
  } else if native_tail {
    make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(7),
    })
  } else {
    first.clone()
  };
  let schedule = if cadenced {
    Schedule {
      trigger: Trigger::cadenced(1),
      cooldown_blocks: 0,
    }
  } else if matches!(source, Source::AtTime) {
    Schedule {
      trigger: Trigger::AtTime { after_ticks: 1 },
      cooldown_blocks: 0,
    }
  } else {
    manual_schedule()
  };
  let steps = if zero_step {
    vec![]
  } else {
    let tail = first.clone();
    let mut steps = vec![first, second];
    steps.resize(step_count, tail);
    steps
  };
  let actor_id = create_user_with(
    ALICE,
    mutability,
    schedule,
    window,
    steps.try_into().unwrap(),
  );
  let sovereign = sovereign_account(actor_id);
  set_asset_balance(&sovereign, TestAsset::Local(1), 100);
  if matches!(source, Source::Running | Source::Suspended) {
    fund_native(actor_id, if step_count > 2 { 100_000 } else { 1_000 });
  }
  if cadenced {
    fund_native(actor_id, 1_000);
    System::set_block_number(2);
    service_canonical_temporal_frontiers(2);
    assert!(Actors::actor_hot(actor_id).unwrap().pending_signal);
  } else if matches!(source, Source::Running | Source::Suspended) {
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
  }
  if matches!(source, Source::Running | Source::Suspended) {
    run_canonical_block_at(if cadenced { 3 } else { 2 }, Weight::MAX);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 7);
    assert_eq!(
      Actors::actor_hot(actor_id).unwrap().cycle_state,
      CycleState::Running
    );
  }
  if matches!(source, Source::Suspended) {
    setup_temporary_retry_pool();
    set_temporary_dex_failure(true);
    run_canonical_block_at(if cadenced { 4 } else { 3 }, Weight::MAX);
    assert_eq!(
      Actors::actor_hot(actor_id).unwrap().cycle_state,
      CycleState::Suspended
    );
  }
  if matches!(source, Source::AtTime) {
    // Reach the owed point without dispatching the existing Close insolvency disposition.
    System::set_block_number(2);
  }
  let (mut contract, base) = Actors::load_admitted_contract_geometry(actor_id).unwrap();
  Actors::remove_admitted_contract_geometry_with_admission(actor_id, &base).unwrap();
  contract.on_unfunded = crate::OnUnfunded::AwaitFunding;
  if let Some(funding) = funding {
    contract.funding = funding;
  }
  // Prepared source binding only. Public creation/replacement continues to reject AwaitFunding.
  let admission = ActorAdmissionCertificateOf::<Test>::new(
    contract.semantic_contract_id(),
    contract.body_commitment().unwrap(),
    contract.trigger.wake_qualification(&contract.window),
    base.runtime_actor_semantics_version,
    base.production_weight_identity,
    base.body_geometry_version,
    base.configured_bounds_commitment,
    base.maximum_lifecycle_weight,
  );
  let mut actor = ActorRef {
    actor_id,
    generation: 0,
  };
  ActorSemanticStates::<Test>::mutate(actor_id, |state| {
    let Some(ActorSemanticState::Active(record)) = state else {
      panic!("active source");
    };
    actor.generation = record.generation;
    record.admission = admission.clone();
  });
  if matches!(source, Source::AtTime) {
    set_prepared_at_time_backing(true);
    assert_ok!(polkadot_sdk::frame_support::storage::with_transaction(
      || {
        match Actors::secure_at_time_installation_backing(&ALICE, ActorType::User, actor, &contract)
        {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      }
    ));
  }
  assert!(Actors::insert_admitted_contract_geometry(
    actor_id, &contract, &admission
  ));
  ActorRunStateStore::<Test>::mutate(actor_id, |run| {
    if let Some(run) = run {
      run.contract_authority = ActorRunAuthority {
        semantic_contract_id: admission.semantic_contract_id,
        body_commitment: admission.body_commitment,
        admission_identity: admission.admission_identity,
        pipeline_service_identity: crate::pipeline_service_identity(admission.admission_identity),
      };
    }
  });
  assert_ok!(Actors::reconcile_actor_state_hold_with_authority(actor_id));
  if matches!(source, Source::Opening) && !cadenced {
    let fee = Actors::trigger_fee_for_actor(
      actor_id,
      ActorType::User,
      crate::OnUnfunded::AwaitFunding,
      crate::TriggerFamily::Manual,
      <Test as crate::Config>::WeightInfo::manual_trigger(),
    )
    .unwrap()
    .trigger_fee;
    let required = fee + TestMinUserBalance::get();
    let available = asset_balance(&sovereign, TestAsset::Native);
    if available < required {
      fund_native(actor_id, required - available);
    }
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
  }
  let nonce = Actors::actor_identity(actor_id).unwrap().cycle_nonce + 1;
  let unpaid = match source {
    Source::AtTime => UnpaidActorService::AtTimeTrigger {
      due_tick: match crate::TriggerDeadlineHandles::<Test>::get(actor_id)
        .unwrap()
        .key
      {
        WakeupKey::Tick(tick) => tick,
        WakeupKey::Block(_) => panic!("AtTime owns a Tick source"),
      },
    },
    Source::Opening => UnpaidActorService::PipelineOpening { cycle_nonce: nonce },
    Source::Running => UnpaidActorService::RunningAction {
      cycle_nonce: nonce,
      cursor: ActorRunStateStore::<Test>::get(actor_id).unwrap().cursor,
    },
    Source::Suspended => {
      let run = ActorRunStateStore::<Test>::get(actor_id).unwrap();
      UnpaidActorService::SuspendedAction {
        cycle_nonce: nonce,
        cursor: run.cursor,
        unsuccessful_attempts_at_cursor: run.unsuccessful_attempts_at_cursor,
      }
    }
  };
  set_native_transfer_lock(&sovereign, native_balance(&sovereign));
  assert_eq!(asset_balance(&sovereign, TestAsset::Native), 0);
  (actor, unpaid)
}

fn root() -> Vec<u8> {
  polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1)
}

#[test]
fn financial_wait_runtime_idle_waits_without_an_inspection_or_new_payment() {
  new_test_ext().execute_with(|| {
    // Prepared admission only: the public AwaitFunding gate remains closed. A zero-Step Manual
    // Cycle isolates machine work from Task effects, retries, producer hints and policy reads.
    let (actor, _) =
      prepared_source_with_options(Source::Opening, false, Mutability::Mutable, true, false);
    let sovereign = sovereign_account(actor.actor_id);
    set_native_transfer_lock(&sovereign, 0);
    fund_native(actor.actor_id, 100_000);
    System::set_block_number(2);
    assert_ok!(Actors::service_canonical_round_head(
      &mut WeightMeter::with_limit(Weight::MAX),
      2,
    ));
    let semantic = ActorSemanticStates::<Test>::get(actor.actor_id).unwrap();
    let hot = Actors::actor_hot(actor.actor_id).unwrap();
    assert_eq!(hot.cycle_state, CycleState::Idle);
    assert!(!hot.pending_signal);
    assert_eq!(
      Actors::actor_identity(actor.actor_id).unwrap().cycle_nonce,
      1
    );
    assert!(!ActorRunStateStore::<Test>::contains_key(actor.actor_id));
    assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
      actor.actor_id
    ));
    assert!(!DeadlineHandles::<Test>::contains_key(actor.actor_id));
    let process = ActorProcesses::<Test>::get(actor.actor_id).unwrap();
    assert!(
      matches!(process.status, ProcessStatus::Disabled(disablement)
      if disablement.cause == crate::ProcessDisableCause::Protocol
        && disablement.revival_authority == crate::ProcessRevivalAuthority::Protocol
        && disablement.basis == crate::SuspendedProcessBasis::Idle)
    );
    assert_eq!(process.residence, None);
    assert_eq!(process.last_attempted, Some(2));
    let fees = fee_collections();
    let custody = (native_balance(&sovereign), native_balance(&ALICE));
    let selector = TestWeightInfo::service_round_begin_populated()
      + TestWeightInfo::service_round_probe_eligible();
    // Empty-ring housekeeping is global; neither quiet block admits Actor-specific inspection.
    for now in [3, 4] {
      System::set_block_number(now);
      System::reset_events();
      let mut meter = WeightMeter::with_limit(Weight::MAX);
      let (encounter, attempt) = Actors::service_canonical_round_head_inner(
        &mut meter,
        now,
        None,
        crate::BlockResourceDomain::ActorBaseEffect,
        None,
      )
      .unwrap();
      assert_eq!(encounter, crate::ServiceRoundEncounter::Empty);
      assert!(attempt.is_none());
      assert_eq!(meter.consumed(), selector);
      assert_eq!(
        ActorSemanticStates::<Test>::get(actor.actor_id),
        Some(semantic.clone())
      );
      assert_eq!(ActorProcesses::<Test>::get(actor.actor_id), Some(process));
      assert!(!crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
      assert_eq!(fee_collections(), fees);
      assert_eq!(
        (native_balance(&sovereign), native_balance(&ALICE)),
        custody
      );
      assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
      assert!(System::events().is_empty());
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    }
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor.actor_id
    ));
    assert!(Actors::actor_hot(actor.actor_id).unwrap().pending_signal);
    assert_eq!(fee_collections().len(), fees.len() + 1);
    assert_eq!(
      crate::ServiceNodes::<Test>::get(actor.actor_id)
        .unwrap()
        .eligible_from,
      5
    );
    assert_eq!(
      Actors::service_canonical_round_head(&mut WeightMeter::with_limit(Weight::MAX), 4),
      Ok(crate::ServiceRoundEncounter::Closed)
    );
    System::set_block_number(5);
    assert_ok!(Actors::service_canonical_round_head(
      &mut WeightMeter::with_limit(Weight::MAX),
      5
    ));
    assert_eq!(
      Actors::actor_identity(actor.actor_id).unwrap().cycle_nonce,
      2
    );
    assert_eq!(fee_collections().len(), fees.len() + 2);
    assert!(!crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn financial_wait_runtime_owner_replacement_retires_wait_and_noop_preserves_it() {
  for version in [1, 2] {
    for tentative in [false, true] {
      for (source, monitored) in [
        (Source::AtTime, false),
        (Source::Opening, false),
        (Source::Running, false),
        (Source::Suspended, false),
        (Source::Running, true),
      ] {
        new_test_ext().execute_with(|| {
      let (actor, episode) = if monitored {
        let actor = prepared_joint_source(true);
        run_canonical_block_at(2, Weight::MAX);
        let sovereign = sovereign_account(actor.actor_id);
        set_native_transfer_lock(&sovereign, native_balance(&sovereign));
        run_canonical_block_at(3, Weight::MAX);
        (actor, ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap())
      } else {
        parked_source(source)
      };
      set_prepared_financial_wait_service_weight(true);
      set_prepared_financial_wait_ingress_weight(true);
      if tentative {
        System::set_block_number(9);
        assert_ok!(deliver_hint(
          &hint_event(actor, ALICE, TestAsset::Native, episode.record().unpaid_liability() + TestMinUserBalance::get()),
          &mut WeightMeter::with_limit(Weight::MAX),
        ));
        assert_eq!(ActorFinancialWaits::<Test>::get(actor.actor_id), Some(episode));
        assert!(Actors::load_canonical_actor_semantic_state(actor).unwrap().1.residence
          .is_some_and(|residence| matches!(residence, ProcessResidence::Service(_))));
      }
      System::set_block_number(10);
      let contract = Actors::load_actor_contract(actor.actor_id).unwrap();
      let run = ActorRunStateStore::<Test>::get(actor.actor_id);
      let process = crate::ActorProcesses::<Test>::get(actor.actor_id);
      let monitor = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id);
      set_admission_semantics_version(version);
      if version == 2 {
        let (semantic, _) = Actors::load_canonical_actor_semantic_state(actor).unwrap();
        let before = root();
        assert!(deliver_hint(
          &hint_event(actor, ALICE, TestAsset::Native, episode.record().unpaid_liability() + TestMinUserBalance::get()),
          &mut WeightMeter::with_limit(Weight::MAX),
        ).is_err());
        assert_eq!(root(), before);
        if tentative {
          let refused = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(
              Actors::prepare_financial_service_recovery(actor, &semantic),
            )
          });
          assert!(refused.is_err());
          assert_eq!(root(), before);
        } else {
          let refused = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
            System::set_block_number(episode.record().expiry_tick());
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(
              Actors::process_financial_wait_due(
                actor, episode.record().expiry_tick(), &mut WeightMeter::with_limit(Weight::MAX),
              ),
            )
          });
          assert_eq!(refused, Err(crate::FinancialWaitDueError::InvalidSource));
          assert_eq!(root(), before);
        }
      }
      let before = root();
      assert_ok!(Actors::update_contract(RuntimeOrigin::signed(ALICE), actor.actor_id, contract.clone()));
      assert_eq!(root(), before);
      let balances = (native_balance(&sovereign_account(actor.actor_id)), native_balance(&BOB));
      let fees = fee_collections();
      let mut replacement = contract;
      replacement.on_unfunded = crate::OnUnfunded::Close;
      replacement.close_when = None;
      let before = root();
      set_fail_create_checkpoint(true);
      assert_eq!(Actors::update_contract(RuntimeOrigin::signed(ALICE), actor.actor_id, replacement.clone()), Err(DispatchError::Other("AtomicityCreateCheckpointFailed")), "{source:?} version={version} tentative={tentative} monitored={monitored} rollback");
      assert_eq!(root(), before);
      assert_eq!(ActorFinancialWaits::<Test>::get(actor.actor_id), Some(episode));
      assert_eq!(ActorRunStateStore::<Test>::get(actor.actor_id), run);
      assert_eq!(crate::ActorProcesses::<Test>::get(actor.actor_id), process);
      assert_eq!(crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id), monitor);
      set_fail_create_checkpoint(false);
      assert_eq!(Actors::update_contract(RuntimeOrigin::signed(ALICE), actor.actor_id, replacement.clone()), Ok(()), "{source:?}");
      assert_eq!(Actors::load_actor_contract(actor.actor_id), Some(replacement));
      assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
      assert!(!ActorRunStateStore::<Test>::contains_key(actor.actor_id));
      assert!(ActorSemanticStates::<Test>::get(actor.actor_id).is_some_and(|state| matches!(state, ActorSemanticState::Active(record) if record.generation > actor.generation)));
      assert_eq!((native_balance(&sovereign_account(actor.actor_id)), native_balance(&BOB)), balances);
      assert_eq!(fee_collections(), fees);
      let mut meter = WeightMeter::with_limit(Weight::MAX);
      assert!(Actors::process_financial_wait_due(actor, episode.record().expiry_tick(), &mut meter).is_err());
      assert!(Actors::actor_identity(actor.actor_id).is_some());
      assert!(!has_actor_event(|event| matches!(event, Event::FinancialWaitRecovered { receipt } if *receipt == due_receipt(actor, episode))));
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
      }
    }
  }
}

#[test]
fn financial_wait_runtime_recertification_preserves_owed_boundary_and_paid_progress() {
  for (source, monitored) in [
    (Source::AtTime, false),
    (Source::Opening, false),
    (Source::Running, false),
    (Source::Suspended, false),
    (Source::Running, true),
  ] {
    for tentative in [false, true] {
      new_test_ext().execute_with(|| {
        let (actor, episode) = if monitored {
          let actor = prepared_joint_source(true);
          run_canonical_block_at(2, Weight::MAX);
          let sovereign = sovereign_account(actor.actor_id);
          set_native_transfer_lock(&sovereign, native_balance(&sovereign));
          run_canonical_block_at(3, Weight::MAX);
          (
            actor,
            ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap(),
          )
        } else {
          parked_source(source)
        };
        set_prepared_financial_wait_ingress_weight(true);
        if tentative {
          System::set_block_number(9);
          assert_ok!(deliver_hint(
            &hint_event(
              actor,
              ALICE,
              TestAsset::Native,
              episode.record().unpaid_liability()
                + TestMinUserBalance::get()
                + u128::from(monitored) * 7
            ),
            &mut WeightMeter::with_limit(Weight::MAX),
          ));
        }
        let contract = Actors::load_actor_contract(actor.actor_id).unwrap();
        set_admission_semantics_version(2);
        let authority = MockAdmissionCertificateAuthority::current().unwrap();
        let certificate = ActorAdmissionCertificateOf::<Test>::new(
          contract.semantic_contract_id(),
          contract.body_commitment().unwrap(),
          contract.trigger.wake_qualification(&contract.window),
          authority.runtime_actor_semantics_version,
          authority.production_weight_identity,
          authority.body_geometry_version,
          authority.configured_bounds_commitment,
          authority.maximum_lifecycle_weight,
        );
        let before = root();
        assert!(!Actors::replace_canonical_contract_geometry(
          actor.actor_id,
          &contract,
          &certificate,
        ));
        assert_eq!(root(), before);
        let run = ActorRunStateStore::<Test>::get(actor.actor_id);
        let monitor = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id);
        let policy = crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id);
        let process = ActorProcesses::<Test>::get(actor.actor_id);
        let deadline = DeadlineHandles::<Test>::get(actor.actor_id);
        let hold = Actors::actor_state_hold(actor.actor_id);
        let sovereign = sovereign_account(actor.actor_id);
        let balances = (
          native_balance(&sovereign_account(actor.actor_id)),
          native_balance(&BOB),
        );
        let fees = fee_collections();
        let events = System::events().encode();
        assert!(
          Actors::store_actor_contract_with_admission(
            actor.actor_id,
            contract.clone(),
            &certificate
          )
          .is_err()
        );
        assert_eq!(root(), before);
        let rolled_back: Result<(), DispatchError> =
          polkadot_sdk::frame_support::storage::with_transaction(|| {
            let result = Actors::store_actor_contract_with_admission(
              actor.actor_id,
              contract.clone(),
              &certificate,
            );
            assert_ok!(&result);
            assert_ok!(Actors::load_canonical_actor_semantic_state(actor));
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(result)
          });
        assert_ok!(rolled_back);
        assert_eq!(root(), before);
        let result: Result<(), DispatchError> =
          polkadot_sdk::frame_support::storage::with_transaction(|| {
            let result =
              Actors::store_actor_contract_with_admission(actor.actor_id, contract, &certificate);
            if result.is_ok() {
              polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(result)
            } else {
              polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(result)
            }
          });
        assert_ok!(result);
        let rebound = ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap();
        assert_eq!(rebound.original_identity(), episode.original_identity());
        assert_eq!(
          rebound.record().expiry_tick(),
          episode.record().expiry_tick()
        );
        assert_eq!(Actors::load_actor_ref(actor.actor_id), Some(actor));
        assert_eq!(
          rebound.record().entered_at_millis(),
          episode.record().entered_at_millis()
        );
        assert_eq!(
          rebound.record().unpaid_liability(),
          episode.record().unpaid_liability()
        );
        assert_eq!(
          rebound.record().unpaid_service(),
          episode.record().unpaid_service()
        );
        assert_eq!(
          rebound.record().scope().behavior_episode,
          episode.record().scope().behavior_episode
        );
        assert_eq!(
          rebound
            .record()
            .scope()
            .contract_authority
            .admission_identity,
          certificate.admission_identity
        );
        let mut expected_run = run;
        if let Some(run) = &mut expected_run {
          run.contract_authority = rebound.record().scope().contract_authority;
        }
        assert_eq!(
          ActorRunStateStore::<Test>::get(actor.actor_id),
          expected_run
        );
        assert_eq!(ActorProcesses::<Test>::get(actor.actor_id), process);
        assert_eq!(DeadlineHandles::<Test>::get(actor.actor_id), deadline);
        assert_eq!(Actors::actor_state_hold(actor.actor_id), hold);
        assert_eq!(
          (
            native_balance(&sovereign_account(actor.actor_id)),
            native_balance(&BOB)
          ),
          balances
        );
        assert_eq!(fee_collections(), fees);
        assert_eq!(System::events().encode(), events);
        if let (Some(old_policy), Some(old_monitor)) = (policy, monitor) {
          let new_policy = crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id).unwrap();
          let new_monitor = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id).unwrap();
          let behavior_episode = rebound.record().scope().behavior_episode.unwrap();
          let old_bound = old_policy
            .plan
            .bind(
              &old_policy.expression,
              crate::BehaviorPolicyBinding {
                semantic_contract_id: episode
                  .record()
                  .scope()
                  .contract_authority
                  .semantic_contract_id,
                admission_identity: episode
                  .record()
                  .scope()
                  .contract_authority
                  .admission_identity,
              },
            )
            .unwrap();
          let new_bound = new_policy
            .plan
            .bind(
              &new_policy.expression,
              crate::BehaviorPolicyBinding {
                semantic_contract_id: certificate.semantic_contract_id,
                admission_identity: certificate.admission_identity,
              },
            )
            .unwrap();
          for slot in old_policy.plan.metrics() {
            assert_eq!(
              new_bound.value(&new_monitor.state, behavior_episode, slot.metric),
              old_bound.value(&old_monitor.state, behavior_episode, slot.metric)
            );
          }
          assert_eq!(new_monitor.clean_cycle, old_monitor.clean_cycle);
          assert_eq!(
            new_policy.episode_started_at_generation,
            old_policy.episode_started_at_generation
          );
        }
        #[cfg(feature = "try-runtime")]
        assert_ok!(Actors::do_try_state());
        System::reset_events();
        if tentative {
          run_canonical_block_at(10, Weight::MAX);
          assert!(has_actor_event(
            |event| matches!(event, Event::FinancialWaitRecovered { receipt }
            if *receipt == due_receipt(actor, episode))
          ));
          if monitored {
            assert_eq!(native_balance(&BOB), balances.1 + 7);
          }
        } else {
          System::set_block_number(episode.record().expiry_tick());
          assert_ok!(Actors::process_financial_wait_due(
            actor,
            episode.record().expiry_tick(),
            &mut WeightMeter::with_limit(Weight::MAX)
          ));
          assert!(Actors::actor_identity(actor.actor_id).is_none());
          assert_eq!((native_balance(&sovereign), native_balance(&BOB)), balances);
          assert!(has_actor_event(|event| matches!(
            event,
            Event::ActorClosed {
              reason: CloseReason::FundingGraceExpired,
              ..
            }
          )));
        }
      });
    }
  }
}

#[test]
fn financial_wait_runtime_recertification_requires_current_certificate_and_resources() {
  for refusal in 0..4 {
    new_test_ext().execute_with(|| {
      let (actor, episode) = parked_source(Source::Running);
      let contract = Actors::load_actor_contract(actor.actor_id).unwrap();
      set_admission_semantics_version(2);
      let authority = MockAdmissionCertificateAuthority::current().unwrap();
      let certificate = ActorAdmissionCertificateOf::<Test>::new(
        contract.semantic_contract_id(),
        contract.body_commitment().unwrap(),
        contract.trigger.wake_qualification(&contract.window),
        authority.runtime_actor_semantics_version + u32::from(refusal == 0),
        authority.production_weight_identity,
        authority.body_geometry_version,
        authority.configured_bounds_commitment,
        authority.maximum_lifecycle_weight,
      );
      if refusal == 1 {
        set_prepared_financial_wait_service_weight(false);
      }
      if refusal == 2 {
        crate::ActorContractHeads::<Test>::mutate(actor.actor_id, |head| {
          head
            .as_mut()
            .unwrap()
            .first_step_resources
            .as_mut()
            .unwrap()
            .control
            .saturating_accrue(Weight::from_parts(1, 0));
        });
      }
      if refusal == 3 {
        crate::ActorStateHolds::<Test>::mutate(actor.actor_id, |hold| {
          hold.as_mut().unwrap().owner = BOB;
        });
      }
      let before = root();
      let result: Result<(), DispatchError> =
        polkadot_sdk::frame_support::storage::with_transaction(|| {
          let result =
            Actors::store_actor_contract_with_admission(actor.actor_id, contract, &certificate);
          if result.is_ok() {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(result)
          } else {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(result)
          }
        });
      assert!(result.is_err());
      if refusal == 3 {
        assert_eq!(result, Err(Error::<Test>::StateHoldInvariant.into()));
      }
      assert_eq!(root(), before);
      assert_eq!(
        ActorFinancialWaits::<Test>::get(actor.actor_id),
        Some(episode)
      );
    });
  }
}

#[test]
fn financial_wait_runtime_current_incompatible_authority_cannot_recertify_wait() {
  for tentative in [false, true] {
    for resource in 0..5 {
      new_test_ext().execute_with(|| {
        let (actor, episode) = hint_source(Source::Running);
        if tentative {
          assert_ok!(deliver_hint(
            &hint_event(
              actor,
              ALICE,
              TestAsset::Native,
              episode.record().unpaid_liability() + TestMinUserBalance::get()
            ),
            &mut WeightMeter::with_limit(Weight::MAX),
          ));
        }
        let contract = Actors::load_actor_contract(actor.actor_id).unwrap();
        let mut authority = MockAdmissionCertificateAuthority::current().unwrap();
        authority.runtime_actor_semantics_version += 1;
        match resource {
          0 => authority.body_geometry_version += 1,
          1 => {
            authority.maximum_lifecycle_weight =
              authority.maximum_lifecycle_weight + Weight::from_parts(1, 0)
          }
          2 => {
            authority.maximum_lifecycle_weight =
              authority.maximum_lifecycle_weight - Weight::from_parts(1, 0)
          }
          3 => {
            authority.maximum_lifecycle_weight =
              authority.maximum_lifecycle_weight + Weight::from_parts(0, 1)
          }
          4 => {
            authority.maximum_lifecycle_weight =
              authority.maximum_lifecycle_weight - Weight::from_parts(0, 1)
          }
          _ => unreachable!(),
        }
        set_admission_authority_override(Some(authority));
        let certificate = ActorAdmissionCertificateOf::<Test>::new(
          contract.semantic_contract_id(),
          contract.body_commitment().unwrap(),
          contract.trigger.wake_qualification(&contract.window),
          authority.runtime_actor_semantics_version,
          authority.production_weight_identity,
          authority.body_geometry_version,
          authority.configured_bounds_commitment,
          authority.maximum_lifecycle_weight,
        );
        assert!(certificate.has_valid_identity());
        let before = root();
        let fees = fee_collections();
        let backing = financial_return_backings();
        let result: Result<(), DispatchError> =
          polkadot_sdk::frame_support::storage::with_transaction(|| {
            let result = Actors::store_actor_contract_with_admission(
              actor.actor_id,
              contract.clone(),
              &certificate,
            );
            if result.is_ok() {
              polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(result)
            } else {
              polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(result)
            }
          });
        assert_eq!(result, Err(Error::<Test>::AwaitFundingUnavailable.into()));
        assert_eq!(root(), before);
        assert_eq!(fee_collections(), fees);
        assert_eq!(financial_return_backings(), backing);
        assert_eq!(
          ActorFinancialWaits::<Test>::get(actor.actor_id),
          Some(episode)
        );
        // The new certificate is genuinely current; rejection belongs to compatibility.
        assert_eq!(
          MockAdmissionCertificateAuthority::current(),
          Some(authority)
        );
        set_admission_authority_override(None);
        #[cfg(feature = "try-runtime")]
        assert_ok!(Actors::do_try_state());
      });
    }
  }
}

#[test]
fn financial_wait_runtime_clock_change_preserves_deadline_and_tentative_service() {
  for source in [
    Source::AtTime,
    Source::Opening,
    Source::Running,
    Source::Suspended,
  ] {
    for tentative in [false, true] {
      new_test_ext().execute_with(|| {
        set_cadence_tick_millis(60_000);
        let (actor, unpaid) = prepared_source(source);
        System::set_block_number(340); // MockTime: 170,000 ms, past AtTime in both clocks.
        let episode = Actors::publish_prepared_financial_wait(
          actor,
          unpaid,
          &mut WeightMeter::with_limit(Weight::MAX),
        )
        .unwrap();
        set_prepared_financial_wait_ingress_weight(true);
        if tentative {
          assert_ok!(deliver_hint(
            &hint_event(
              actor,
              ALICE,
              TestAsset::Native,
              episode.record().unpaid_liability() + TestMinUserBalance::get()
            ),
            &mut WeightMeter::with_limit(Weight::MAX),
          ));
        }
        let (semantic, process) = Actors::load_canonical_actor_semantic_state(actor).unwrap();
        assert_eq!(
          matches!(process.residence, Some(ProcessResidence::Service(_))),
          tentative
        );
        let record = episode.record();
        let reinterpreted = crate::FinancialWaitRecord::new(
          record.scope(),
          unpaid,
          record.unpaid_liability(),
          record.entered_at_millis(),
          60_001,
          TestMaxTemporalDelayTicks::get(),
        )
        .unwrap();
        assert_eq!(record.expiry_tick(), reinterpreted.expiry_tick());
        // Reach the later apparent endpoint by scalar time, without replaying blocks.
        System::set_block_number((record.expiry_tick() * 60_001).div_ceil(500));
        let contract = Actors::load_actor_contract(actor.actor_id).unwrap();
        let sovereign = semantic.identity.sovereign_account;
        let event = hint_event(
          actor,
          ALICE,
          TestAsset::Native,
          record.unpaid_liability() + TestMinUserBalance::get(),
        );
        let before = root();
        let fees = fee_collections();
        let backing = financial_return_backings();
        let local = (
          asset_balance(&sovereign, TestAsset::Local(1)),
          asset_balance(&BOB, TestAsset::Local(1)),
        );
        set_cadence_tick_millis(60_001);
        assert!(Actors::load_canonical_actor_semantic_state(actor).is_err());
        let mut ingress = WeightMeter::with_limit(Weight::MAX);
        assert!(deliver_hint(&event, &mut ingress).is_err());
        assert_ne!(ingress.consumed(), Weight::from_parts(0, 0));
        assert_eq!(root(), before);
        let mut service = WeightMeter::with_limit(Weight::MAX);
        if tentative {
          assert_eq!(
            Actors::service_canonical_round_head_inner(
              &mut service,
              System::block_number(),
              None,
              crate::BlockResourceDomain::ActorBaseEffect,
              None,
            )
            .err(),
            Some(crate::ServiceRoundError::ProcessResidenceMismatch)
          );
        } else {
          assert!(
            Actors::process_next_due_tick_deadline(&mut service, record.expiry_tick(),).is_err()
          );
        }
        assert_ne!(service.consumed(), Weight::from_parts(0, 0));
        assert_eq!(root(), before);
        let mut authority = MockAdmissionCertificateAuthority::current().unwrap();
        authority.runtime_actor_semantics_version += 1;
        set_admission_authority_override(Some(authority));
        let certificate = ActorAdmissionCertificateOf::<Test>::new(
          contract.semantic_contract_id(),
          contract.body_commitment().unwrap(),
          contract.trigger.wake_qualification(&contract.window),
          authority.runtime_actor_semantics_version,
          authority.production_weight_identity,
          authority.body_geometry_version,
          authority.configured_bounds_commitment,
          authority.maximum_lifecycle_weight,
        );
        let result: Result<(), DispatchError> =
          polkadot_sdk::frame_support::storage::with_transaction(|| {
            let result =
              Actors::store_actor_contract_with_admission(actor.actor_id, contract, &certificate);
            if result.is_ok() {
              polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(result)
            } else {
              polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(result)
            }
          });
        assert!(result.is_err());
        assert_eq!(root(), before);
        assert_eq!(fee_collections(), fees);
        assert_eq!(financial_return_backings(), backing);
        assert_eq!(
          (
            asset_balance(&sovereign, TestAsset::Local(1)),
            asset_balance(&BOB, TestAsset::Local(1))
          ),
          local
        );
        set_admission_authority_override(None);
        set_cadence_tick_millis(60_000);
        assert_eq!(
          Actors::load_canonical_actor_semantic_state(actor).unwrap(),
          (semantic, process)
        );
        assert_eq!(
          ActorFinancialWaits::<Test>::get(actor.actor_id),
          Some(episode)
        );
        assert_eq!(root(), before);
        #[cfg(feature = "try-runtime")]
        assert_ok!(Actors::do_try_state());
        set_cadence_tick_millis(500);
      });
    }
  }
}

#[test]
fn financial_wait_runtime_public_sweep_preserves_wait_and_independent_window_terminal() {
  for batch in [false, true] {
    for (source, expired) in [
      (Source::AtTime, false),
      (Source::Opening, false),
      (Source::Opening, true),
      (Source::Running, false),
      (Source::Running, true),
      (Source::Suspended, false),
      (Source::Suspended, true),
    ] {
      new_test_ext().execute_with(|| {
        let (actor, unpaid) = prepared_source_with_funding(
          source, false, Mutability::Mutable, false, false, None,
          if matches!(source, Source::AtTime) { None } else { Some(crate::ScheduleWindow { start: 1, end: 101 }) }, false,
        );
        let episode = Actors::publish_prepared_financial_wait(actor, unpaid, &mut WeightMeter::with_limit(Weight::MAX)).unwrap();
        System::set_block_number(if expired { 102 } else { 10 });
        let sovereign = sovereign_account(actor.actor_id);
        let balances = (native_balance(&sovereign), native_balance(&BOB));
        let local = (asset_balance(&sovereign, TestAsset::Local(1)), asset_balance(&BOB, TestAsset::Local(1)));
        let fees = fee_collections();
        let semantic = ActorSemanticStates::<Test>::get(actor.actor_id);
        let process = ActorProcesses::<Test>::get(actor.actor_id);
        let deadline = DeadlineHandles::<Test>::get(actor.actor_id);
        let hold = Actors::actor_state_hold(actor.actor_id);
        let events = System::events();
        let before = root();
        let contract = Actors::load_actor_contract(actor.actor_id).unwrap();
        let run = ActorRunStateStore::<Test>::get(actor.actor_id);
        let mode = if run.is_some() { crate::SimulationMode::CurrentRun } else { crate::SimulationMode::FreshCurrentPlan };
        let simulate = |budget| {
          let result = Actors::simulate_current_contract(
            actor.actor_id,
            ActorType::User,
            Mutability::Mutable,
            contract.clone(),
            mode,
            budget,
          );
          assert_eq!(root(), before);
          assert_eq!(fee_collections(), fees);
          assert_eq!((asset_balance(&sovereign, TestAsset::Local(1)), asset_balance(&BOB, TestAsset::Local(1))), local);
          result
        };
        if expired {
          let mut budget = ample_simulation_budget();
          budget.actor_control = Weight::zero();
          assert_eq!(simulate(budget), Err(crate::SimulationError::ResourceDeferred));
          let terminal = simulate(ample_simulation_budget()).unwrap();
          assert_eq!(terminal.status, crate::AttemptDisposition::Closed(CloseReason::WindowExpired));
          assert!(terminal.steps.is_empty());
          assert!(terminal.run_cursor.is_none());
          assert_eq!(
            (terminal.cycle_nonce, terminal.start_cursor, terminal.cumulative_outcomes),
            run.as_ref().map_or(
              (0, 0, crate::OutcomeTotals::default()),
              |run| (run.cycle_nonce, run.cursor, run.cumulative_outcomes),
            )
          );
        } else {
          assert_eq!(simulate(ample_simulation_budget()), Err(crate::SimulationError::NotReady));
        }
        if batch {
          assert_ok!(Actors::permissionless_sweep_many(RuntimeOrigin::signed(BOB), vec![actor.actor_id].try_into().unwrap()));
        } else {
          assert_ok!(Actors::permissionless_sweep(RuntimeOrigin::signed(BOB), actor.actor_id));
        }
        if expired {
          assert!(Actors::actor_identity(actor.actor_id).is_none());
          assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
          assert!(!ActorRunStateStore::<Test>::contains_key(actor.actor_id));
          assert!(!DeadlineHandles::<Test>::contains_key(actor.actor_id));
          assert!(has_actor_event(|event| matches!(event, Event::ActorClosed { actor_id, reason: CloseReason::WindowExpired } if *actor_id == actor.actor_id)));
        } else {
          assert_eq!(ActorSemanticStates::<Test>::get(actor.actor_id), semantic);
          assert_eq!(ActorProcesses::<Test>::get(actor.actor_id), process);
          assert_eq!(DeadlineHandles::<Test>::get(actor.actor_id), deadline);
          assert_eq!(Actors::actor_state_hold(actor.actor_id), hold);
          assert_eq!(ActorFinancialWaits::<Test>::get(actor.actor_id), Some(episode));
          if !batch { assert_eq!(root(), before); }
        }
        assert_eq!((native_balance(&sovereign), native_balance(&BOB)), balances);
        assert_eq!((asset_balance(&sovereign, TestAsset::Local(1)), asset_balance(&BOB, TestAsset::Local(1))), local);
        assert_eq!(fee_collections(), fees);
        let after_events = System::events();
        assert!(after_events.starts_with(&events));
        assert!(!after_events[events.len()..].iter().any(|record| matches!(
          record.event,
          RuntimeEvent::Actors(Event::ActionFeeCharged { .. } | Event::FinancialWaitRecovered { .. })
        )));
        #[cfg(feature = "try-runtime")]
        assert_ok!(Actors::do_try_state());
      });
    }
  }
}

fn prepared_joint_source(clean: bool) -> ActorRef {
  let mut atoms = vec![crate::BehaviorAtom {
    metric: crate::BehaviorMetric::ActionSuccessTotal,
    threshold: 2,
  }];
  if clean {
    atoms.push(crate::BehaviorAtom {
      metric: crate::BehaviorMetric::CycleCleanTotal,
      threshold: 1,
    });
  }
  let step = make_step(Task::Transfer {
    to: BOB,
    asset: TestAsset::Native,
    amount: AmountResolution::Fixed(7),
  });
  prepared_joint_source_with_contract(
    vec![step.clone(), step].try_into().unwrap(),
    super::behavior_policy::expression(vec![atoms]),
    false,
  )
}

fn prepared_joint_source_with_contract(
  steps: crate::ContractSteps<Test>,
  expression: crate::CloseWhenOf<Test>,
  with_funding_window: bool,
) -> ActorRef {
  let (actor, _) = prepared_source_with_funding(
    Source::Opening,
    true,
    Mutability::Mutable,
    false,
    false,
    None,
    with_funding_window.then_some(crate::ScheduleWindow {
      start: 0,
      end: u64::MAX - 1,
    }),
    false,
  );
  let (mut contract, base) = Actors::load_admitted_contract_geometry(actor.actor_id).unwrap();
  Actors::remove_admitted_contract_geometry_with_admission(actor.actor_id, &base).unwrap();
  contract.steps = steps;
  contract.close_when = Some(expression);
  if with_funding_window {
    let whitelist_size: u32 = <Test as crate::Config>::MaxWhitelistSize::get();
    contract.funding = crate::FundingSourcePolicy::SignedAllowlist(
      (100..100 + u64::from(whitelist_size))
        .collect::<std::collections::BTreeSet<_>>()
        .try_into()
        .unwrap(),
    );
  }
  let admission = ActorAdmissionCertificateOf::<Test>::new(
    contract.semantic_contract_id(),
    contract.body_commitment().unwrap(),
    contract.trigger.wake_qualification(&contract.window),
    base.runtime_actor_semantics_version,
    base.production_weight_identity,
    base.body_geometry_version,
    base.configured_bounds_commitment,
    base.maximum_lifecycle_weight,
  );
  ActorSemanticStates::<Test>::mutate(actor.actor_id, |state| {
    let Some(ActorSemanticState::Active(record)) = state else {
      panic!("prepared joint Active source");
    };
    record.admission = admission.clone();
  });
  assert!(Actors::insert_admitted_contract_geometry(
    actor.actor_id,
    &contract,
    &admission
  ));
  assert_ok!(Actors::reconcile_actor_state_hold_with_authority(
    actor.actor_id
  ));
  set_prepared_financial_wait_service_weight(true);
  set_prepared_financial_wait_ingress_weight(true);
  set_native_transfer_lock(&sovereign_account(actor.actor_id), 0);
  fund_native(actor.actor_id, 10_000);
  actor
}

const JOINT_METRICS: [(crate::BehaviorMetric, oracle::Metric); 10] = [
  (
    crate::BehaviorMetric::PreconditionPassTotal,
    oracle::Metric::PreconditionPassTotal,
  ),
  (
    crate::BehaviorMetric::PreconditionPassStreak,
    oracle::Metric::PreconditionPassStreak,
  ),
  (
    crate::BehaviorMetric::PreconditionRejectTotal,
    oracle::Metric::PreconditionRejectTotal,
  ),
  (
    crate::BehaviorMetric::PreconditionRejectStreak,
    oracle::Metric::PreconditionRejectStreak,
  ),
  (
    crate::BehaviorMetric::ActionSuccessTotal,
    oracle::Metric::ActionSuccessTotal,
  ),
  (
    crate::BehaviorMetric::ActionSuccessStreak,
    oracle::Metric::ActionSuccessStreak,
  ),
  (
    crate::BehaviorMetric::ActionFailureTotal,
    oracle::Metric::ActionFailureTotal,
  ),
  (
    crate::BehaviorMetric::ActionFailureStreak,
    oracle::Metric::ActionFailureStreak,
  ),
  (
    crate::BehaviorMetric::CycleCompletedTotal,
    oracle::Metric::CycleCompletedTotal,
  ),
  (
    crate::BehaviorMetric::CycleCleanTotal,
    oracle::Metric::CycleCleanTotal,
  ),
];

fn assert_joint_trace(actor: ActorRef, trace: &[oracle::Record]) {
  let projection = oracle::project(&[oracle::StepKind::Economic], trace, None);
  let policy = crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id).unwrap_or_else(|| {
    panic!(
      "joint Actor closed before its expected terminal: {:?}",
      System::events()
    )
  });
  let monitor = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id).unwrap();
  let admission = Actors::load_control_admission(actor.actor_id).unwrap();
  let episode = (
    b"DEOS_ACTOR_BEHAVIOR_EPISODE",
    actor.actor_id,
    policy.episode_started_at_generation,
    admission.semantic_contract_id,
  )
    .using_encoded(polkadot_sdk::sp_io::hashing::blake2_256);
  let bound = policy
    .plan
    .bind(
      &policy.expression,
      crate::BehaviorPolicyBinding {
        semantic_contract_id: admission.semantic_contract_id,
        admission_identity: admission.admission_identity,
      },
    )
    .unwrap();
  for (metric, reference) in JOINT_METRICS {
    assert_eq!(
      bound.value(&monitor.state, episode, metric),
      Ok(Some(u32::try_from(projection.value(reference)).unwrap())),
      "{metric:?}",
    );
  }
}

#[test]
fn financial_wait_runtime_joint_failure_wait_retry_and_skip_match_committed_trace() {
  new_test_ext().execute_with(|| {
    // Only the first clean Cycle can close in this short trace. The other clauses retain all
    // stream metrics at unreachable thresholds for comparison with the exact-history oracle.
    let mut clauses = JOINT_METRICS[..9].chunks(4).map(|metrics| {
      metrics.iter().map(|(metric, _)| crate::BehaviorAtom {
        metric: *metric,
        threshold: 100,
      }).collect::<Vec<_>>()
    }).collect::<Vec<_>>();
    clauses.push(vec![crate::BehaviorAtom {
      metric: crate::BehaviorMetric::CycleCleanTotal,
      threshold: 1,
    }]);
    let mut steps = temporary_retry_swap_plan();
    steps[0].precondition = all_conditions(vec![Predicate::BalanceAbove {
      asset: TestAsset::Local(1),
      threshold: 50,
    }]);
    let actor = prepared_joint_source_with_contract(
      steps,
      super::behavior_policy::expression(clauses),
      false,
    );
    let sovereign = sovereign_account(actor.actor_id);
    setup_temporary_retry_pool();
    let head = crate::ActorContractHeads::<Test>::get(actor.actor_id).unwrap();
    let balance = native_balance(&sovereign);
    let pool_native = native_balance(&u64::MAX);
    let sink = native_balance(&TestFeeSink::get());
    set_temporary_dex_failure(true);
    System::reset_events();
    run_canonical_block_at(2, Weight::MAX);
    assert_eq!(last_dex_actor_type(), Some(ActorType::User));
    assert!(has_actor_event(|event| matches!(event, Event::StepFailed {
      actor_id, retry_class: RetryClass::Temporary, ..
    } if *actor_id == actor.actor_id)));
    assert_eq!(native_balance(&sovereign), balance - head.header.pipeline_machine_envelope.pipeline_machine_fee_upper - 100);
    assert_eq!(native_balance(&u64::MAX), pool_native);
    assert_eq!(native_balance(&TestFeeSink::get()), sink + head.header.pipeline_machine_envelope.pipeline_machine_fee_upper + 100);
    let mut trace = vec![
      oracle::Record::committed(oracle::Transition::Open),
      oracle::Record::committed(oracle::Transition::Attempt {
        check: oracle::Check::Pass,
        action: oracle::Action::Failure,
        disposition: oracle::Disposition::Retry,
      }),
    ];
    assert_joint_trace(actor, &trace);
    let failed_run = ActorRunStateStore::<Test>::get(actor.actor_id).unwrap();
    assert_eq!(failed_run.unsuccessful_attempts_at_cursor, 1);
    let failed_monitor = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id).unwrap();
    assert_eq!(failed_monitor.clean_cycle.unwrap().candidate.completion(Some(crate::NormalCycleCompletion::PlanEnd)), Some(false));
    let due = failed_run.eligible_at;
    assert!(due > 2);
    set_native_transfer_lock(&sovereign, native_balance(&sovereign));
    System::reset_events();
    run_canonical_block_at(due, Weight::MAX);
    let episode = ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap();
    assert_eq!(episode.record().unpaid_service(), UnpaidActorService::SuspendedAction {
      cycle_nonce: 1, cursor: 0, unsuccessful_attempts_at_cursor: 1,
    });
    assert_eq!(ActorRunStateStore::<Test>::get(actor.actor_id), Some(failed_run));
    assert_eq!(crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id), Some(failed_monitor.clone()));
    assert!(!has_actor_event(|event| matches!(event, Event::StepFailed { .. } | Event::StepSkipped { .. } | Event::ActionFeeCharged { .. } | Event::CycleSummary { .. })));
    trace.push(oracle::Record::committed(oracle::Transition::Preserve(oracle::PreservedTransition::AwaitFunding)));
    assert_joint_trace(actor, &trace);
    System::set_block_number(due + 2);
    assert_ok!(deliver_hint(&hint_event(actor, ALICE, TestAsset::Native, episode.record().unpaid_liability() + TestMinUserBalance::get() + 10), &mut WeightMeter::with_limit(Weight::MAX)));
    assert_eq!(crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id), Some(failed_monitor));
    assert_eq!(ActorFinancialWaits::<Test>::get(actor.actor_id), Some(episode));
    trace.push(oracle::Record::committed(oracle::Transition::Preserve(oracle::PreservedTransition::TopUp)));
    assert_joint_trace(actor, &trace);
    let sink = native_balance(&TestFeeSink::get());
    set_temporary_dex_failure(false);
    System::reset_events();
    run_canonical_block_at(due + 3, Weight::MAX);
    trace.push(oracle::Record::committed(oracle::Transition::Attempt {
      check: oracle::Check::Pass,
      action: oracle::Action::Success,
      disposition: oracle::Disposition::Advance,
    }));
    assert_joint_trace(actor, &trace);
    assert_eq!(Actors::actor_identity(actor.actor_id).unwrap().cycle_nonce, 1);
    assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    assert!(!ActorRunStateStore::<Test>::contains_key(actor.actor_id));
    assert_eq!(native_balance(&TestFeeSink::get()), sink + 100);
    assert!(has_actor_event(|event| matches!(event, Event::FinancialWaitRecovered { receipt } if *receipt == due_receipt(actor, episode))));
    assert!(!has_actor_event(|event| matches!(event, Event::PipelineFeeCharged { .. } | Event::ActorClosed { .. })));

    // A later false predicate resets its opposite streak but contributes no Action result.
    set_native_transfer_lock(&sovereign, 0);
    fund_native(actor.actor_id, 20_000);
    assert_ok!(MockAssetOps::burn(
      &sovereign,
      TestAsset::Local(1),
      asset_balance(&sovereign, TestAsset::Local(1)),
    ));
    assert_eq!(asset_balance(&sovereign, TestAsset::Local(1)), 0);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor.actor_id));
    run_canonical_block_at(due + 4, Weight::MAX);
    trace.push(oracle::Record::committed(oracle::Transition::Open));
    trace.push(oracle::Record::committed(oracle::Transition::Attempt {
      check: oracle::Check::Reject,
      action: oracle::Action::NotInvoked,
      disposition: oracle::Disposition::Advance,
    }));
    assert_joint_trace(actor, &trace);
    assert_eq!(Actors::actor_identity(actor.actor_id).unwrap().cycle_nonce, 2);
    assert!(crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id).unwrap().clean_cycle.is_none());
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());

    set_asset_balance(&sovereign, TestAsset::Local(1), 100);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor.actor_id));
    System::reset_events();
    run_canonical_block_at(due + 5, Weight::MAX);
    trace.push(oracle::Record::committed(oracle::Transition::Open));
    trace.push(oracle::Record::committed(oracle::Transition::Attempt {
      check: oracle::Check::Pass,
      action: oracle::Action::Success,
      disposition: oracle::Disposition::Advance,
    }));
    let policy = oracle::Policy::new(vec![vec![oracle::Atom { metric: oracle::Metric::CycleCleanTotal, threshold: 1 }]]).unwrap();
    assert!(oracle::project(&[oracle::StepKind::Economic], &trace, Some(&policy)).closed);
    assert!(Actors::actor_identity(actor.actor_id).is_none());
    assert!(!crate::ActorBehaviorMonitors::<Test>::contains_key(actor.actor_id));
    assert!(has_actor_event(|event| matches!(event, Event::ActorClosed { actor_id, reason: CloseReason::BehaviorPolicySatisfied } if *actor_id == actor.actor_id)));
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn financial_wait_runtime_joint_optional_records_attain_codec_ceilings_before_close() {
  use polkadot_sdk::frame_support::traits::{
    fungible::{InspectHold, MutateHold},
    tokens::Precision,
  };
  for with_funding_window in [false, true] {
    new_test_ext().execute_with(|| {
      let max_steps: u32 = <Test as crate::Config>::MaxContractSteps::get();
      let mut steps = temporary_retry_swap_plan();
      if with_funding_window {
        let tail = StepOf::<Test> {
          precondition: all_conditions(
            (1..=4)
              .map(|index| Predicate::BalanceBelow {
                asset: TestAsset::Local(index),
                threshold: u128::MAX,
              })
              .collect(),
          ),
          task: Task::AddLiquidity { asset_a: TestAsset::Local(1), asset_b: TestAsset::Local(2), amount_a: AmountResolution::Fixed(u128::MAX), amount_b: AmountResolution::Fixed(u128::MAX), min_lp_out: u128::MAX },
          on_error: StepErrorPolicy::AbortCycle,
        };
        for _ in 1..max_steps {
          steps.try_push(tail.clone()).unwrap();
        }
      }
      let actor = prepared_joint_source_with_contract(
        steps.clone(),
        super::behavior_geometry::maximum_expression(),
        with_funding_window,
      );
      let head_before = crate::ActorContractHeads::<Test>::get(actor.actor_id).unwrap();
      let tail_count = (steps.len() - 1).div_ceil(crate::MAX_STEPS_PER_TAIL_CHUNK as usize);
      let tails_before = (0..tail_count as u32)
        .map(|index| crate::ActorContractTailChunks::<Test>::get(actor.actor_id, index).unwrap())
        .collect::<Vec<_>>();
      if with_funding_window {
        assert_eq!(steps.len(), max_steps as usize);
        assert_eq!(tail_count, 3);
        assert_eq!(
          head_before.header.funding.encoded_size(),
          crate::FundingSourcePolicyOf::<Test>::max_encoded_len()
        );
        assert!(head_before.header.window.is_some());
      }
      let sovereign = sovereign_account(actor.actor_id);
      fund_native(actor.actor_id, 100_000);
      setup_temporary_retry_pool();
      let hold_before_cycle = Actors::actor_state_hold(actor.actor_id).unwrap();
      set_temporary_dex_failure(true);
      run_canonical_block_at(2, Weight::MAX);
      let run = ActorRunStateStore::<Test>::get(actor.actor_id).unwrap();
      assert_eq!(
        Actors::actor_hot(actor.actor_id).unwrap().cycle_state,
        CycleState::Suspended
      );
      assert_eq!(run.unsuccessful_attempts_at_cursor, 1);
      let monitor_before = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id).unwrap();
      set_native_transfer_lock(&sovereign, native_balance(&sovereign));
      run_canonical_block_at(run.eligible_at, Weight::MAX);
      let policy = crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id).unwrap();
      let monitor = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id).unwrap();
      let wait = ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap();
      assert_eq!(Actors::actor_state_hold(actor.actor_id), Some(hold_before_cycle),
        "installed hold already backs clean flags and maximum financial record capacity");
      assert_eq!(
        monitor, monitor_before,
        "unpaid retry creates no observation"
      );
      assert_eq!(policy.plan.metrics().len(), 10);
      assert!(
        policy
          .plan
          .metrics()
          .iter()
          .all(|atom| atom.threshold == u32::MAX)
      );
      assert_eq!(
        policy.encoded_size(),
        crate::ActorBehaviorPolicyOf::<Test>::max_encoded_len()
      );
      assert_eq!(
        monitor.encoded_size(),
        crate::ActorBehaviorMonitor::max_encoded_len()
      );
      assert_eq!(
        wait.encoded_size(),
        FinancialWaitEpisode::<Balance>::max_encoded_len()
      );
      assert_eq!(
        wait.record().unpaid_service(),
        UnpaidActorService::SuspendedAction {
          cycle_nonce: run.cycle_nonce,
          cursor: run.cursor,
          unsuccessful_attempts_at_cursor: run.unsuccessful_attempts_at_cursor,
        }
      );
      let before = root();
      let crate::ActorEligibility::Active(view) = Actors::actor_eligibility(actor.actor_id).unwrap()
      else {
        panic!("joint maximum records have an authenticated Active projection");
      };
      assert_eq!(root(), before);
      let values = view
        .behavior_episode
        .unwrap()
        .monitor
        .unwrap()
        .capped_values;
      assert_eq!(values.len(), 10);
      assert!(
        values.iter(/* deos-bypass: bounded-iter -- ten-metric projection bound. */).all(|counter| counter.value <= 1),
        "small committed counts already occupy every fixed-width counter slot"
      );
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
      let custody = [TestAsset::Native, TestAsset::Local(1), TestAsset::Local(2), TestAsset::Local(77)]
        .map(|asset| MockAssetOps::total_balance(&sovereign, asset));
      let reason: RuntimeHoldReason = crate::HoldReason::ActorState.into();
      let held = <Balances as InspectHold<AccountId>>::balance_on_hold(&reason, &ALICE);
      <Balances as MutateHold<AccountId>>::release(&reason, &ALICE, held, Precision::Exact).unwrap();
      let failed_root = root();
      assert!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor.actor_id).is_err());
      assert_eq!(
        root(),
        failed_root,
        "late hold refusal restores all maximum-width owners"
      );
      assert_eq!(
        crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id),
        Some(policy.clone())
      );
      assert_eq!(
        crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id),
        Some(monitor.clone())
      );
      assert_eq!(ActorFinancialWaits::<Test>::get(actor.actor_id), Some(wait));
      assert_eq!(
        crate::ActorContractHeads::<Test>::get(actor.actor_id),
        Some(head_before.clone())
      );
      for (index, tail) in tails_before.iter().enumerate() {
        assert_eq!(
          crate::ActorContractTailChunks::<Test>::get(actor.actor_id, index as u32),
          Some(tail.clone())
        );
      }
      <Balances as MutateHold<AccountId>>::hold(&reason, &ALICE, held).unwrap();
      assert_ok!(Actors::close_actor(
        RuntimeOrigin::signed(ALICE),
        actor.actor_id
      ));
      assert!(!crate::ActorBehaviorPolicies::<Test>::contains_key(
        actor.actor_id
      ));
      assert!(!crate::ActorBehaviorMonitors::<Test>::contains_key(
        actor.actor_id
      ));
      assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
      assert!(!ActorRunStateStore::<Test>::contains_key(actor.actor_id));
      assert!(!ActorProcesses::<Test>::contains_key(actor.actor_id));
      assert!(!DeadlineHandles::<Test>::contains_key(actor.actor_id));
      assert_eq!(Actors::actor_state_hold(actor.actor_id), None);
      assert!(!crate::ActorContractHeads::<Test>::contains_key(actor.actor_id));
      for index in 0..tail_count as u32 {
        assert!(!crate::ActorContractTailChunks::<Test>::contains_key(actor.actor_id, index));
      }
      assert_eq!(
        [TestAsset::Native, TestAsset::Local(1), TestAsset::Local(2), TestAsset::Local(77)]
          .map(|asset| MockAssetOps::total_balance(&sovereign, asset)),
        custody
      );
      println!(
        "joint optional codec bytes: policy={}, monitor={}, financial={}, steps={}, head={}, tails={}",
        policy.encoded_size(),
        monitor.encoded_size(),
        wait.encoded_size(),
        steps.len(),
        head_before.encoded_size(),
        tails_before.iter().map(Encode::encoded_size).sum::<usize>()
      );
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_joint_missing_policy_authority_preserves_the_wait() {
  for missing_inspection in [false, true] {
    new_test_ext().execute_with(|| {
      let actor = prepared_joint_source(true);
      let sovereign = sovereign_account(actor.actor_id);
      run_canonical_block_at(2, Weight::MAX);
      set_native_transfer_lock(&sovereign, native_balance(&sovereign));
      run_canonical_block_at(3, Weight::MAX);
      let episode = ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap();
      let monitor = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id).unwrap();
      System::set_block_number(4);
      assert_ok!(deliver_hint(
        &hint_event(
          actor,
          ALICE,
          TestAsset::Native,
          episode.record().unpaid_liability() + TestMinUserBalance::get() + 7
        ),
        &mut WeightMeter::with_limit(Weight::MAX)
      ));
      System::set_block_number(5);
      set_missing_behavior_inspection_weight(missing_inspection);
      set_missing_behavior_close_weight(!missing_inspection);
      let before = root();
      let fees = fee_collections();
      let mut meter = WeightMeter::with_limit(Weight::MAX);
      assert!(Actors::service_canonical_round_head(&mut meter, 5).is_err());
      assert_eq!(root(), before);
      assert_eq!(fee_collections(), fees);
      assert_eq!(
        ActorFinancialWaits::<Test>::get(actor.actor_id),
        Some(episode)
      );
      assert_eq!(
        crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id),
        Some(monitor)
      );
      assert!(meter.consumed().ref_time() > 0);
      set_missing_behavior_inspection_weight(false);
      set_missing_behavior_close_weight(false);
      assert_ok!(Actors::service_canonical_round_head(
        &mut WeightMeter::with_limit(Weight::MAX),
        5
      ));
      assert!(Actors::actor_identity(actor.actor_id).is_none());
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_at_time_installation_backs_passive_entry_and_original_trigger() {
  new_test_ext().execute_with(|| {
    let sink = native_balance(&TestFeeSink::get());
    let (actor, unpaid) = prepared_source(Source::AtTime);
    let contract = Actors::load_actor_contract(actor.actor_id).unwrap();
    assert_eq!(native_balance(&TestFeeSink::get()), sink + TestActorCreationFee::get() + 400);
    assert_eq!(polkadot_sdk::sp_io::storage::get(&at_time_backing_key(actor, contract.semantic_contract_id())).unwrap().as_ref(), &400_u128.encode());
    let sink = native_balance(&TestFeeSink::get());
    let fees = fee_collections();
    service_canonical_temporal_frontiers(2);
    let episode = ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap();
    assert_eq!(episode.record().unpaid_service(), unpaid);
    assert_eq!(episode.record().unpaid_liability(), 600);
    assert_eq!(native_balance(&TestFeeSink::get()), sink);
    assert_eq!(fee_collections(), fees);
    set_prepared_financial_wait_ingress_weight(true);
    System::set_block_number(3);
    assert_ok!(deliver_hint(&hint_event(actor, ALICE, TestAsset::Native, episode.record().unpaid_liability() + TestMinUserBalance::get()), &mut WeightMeter::with_limit(Weight::MAX)));
    assert_eq!(native_balance(&TestFeeSink::get()), sink);
    assert_eq!(ActorFinancialWaits::<Test>::get(actor.actor_id), Some(episode));
    run_canonical_block_at(4, Weight::MAX);
    assert_eq!(native_balance(&TestFeeSink::get()), sink + episode.record().unpaid_liability());
    assert!(has_actor_event(|event| matches!(event, Event::FinancialWaitRecovered { receipt } if *receipt == due_receipt(actor, episode))));
    assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    assert!(Actors::actor_hot(actor.actor_id).unwrap().pending_signal);
    run_canonical_block_at(5, Weight::MAX);
    assert_eq!(ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap().record().unpaid_service(), UnpaidActorService::PipelineOpening { cycle_nonce: 1 });
    assert_eq!(native_balance(&TestFeeSink::get()), sink + episode.record().unpaid_liability());
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn financial_wait_runtime_at_time_installation_requires_host_and_fixed_prices() {
  new_test_ext().execute_with(|| {
    let (actor, _) = prepared_source(Source::AtTime);
    let contract = Actors::load_actor_contract(actor.actor_id).unwrap();
    let before = root();
    let fees = fee_collections();
    set_prepared_at_time_backing(false);
    assert!(
      Actors::secure_at_time_installation_backing(&ALICE, ActorType::User, actor, &contract)
        .is_err()
    );
    for missing in 0..3 {
      set_prepared_at_time_backing(true);
      set_prepared_financial_wait_weights(missing != 0);
      set_prepared_financial_wait_due_weight(missing != 1);
      set_prepared_financial_wait_service_weight(missing != 2);
      assert_noop!(
        Actors::secure_at_time_installation_backing(&ALICE, ActorType::User, actor, &contract),
        Error::<Test>::AdmissionBoundOverflow
      );
    }
    let mut close = contract.clone();
    close.on_unfunded = crate::OnUnfunded::Close;
    let mut manual = contract.clone();
    manual.trigger = Trigger::Manual;
    assert_ok!(Actors::secure_at_time_installation_backing(
      &ALICE,
      ActorType::User,
      actor,
      &close
    ));
    assert_ok!(Actors::secure_at_time_installation_backing(
      &ALICE,
      ActorType::User,
      actor,
      &manual
    ));
    assert_ok!(Actors::secure_at_time_installation_backing(
      &ALICE,
      ActorType::System,
      actor,
      &contract
    ));
    assert_ok!(Actors::update_contract(
      RuntimeOrigin::signed(ALICE),
      actor.actor_id,
      contract
    ));
    assert_eq!(root(), before);
    assert_eq!(fee_collections(), fees);
  });
}

#[test]
fn financial_wait_runtime_at_time_active_installation_authenticates_scope_and_rolls_back_backing() {
  for refusal in 0..7 {
    new_test_ext().execute_with(|| {
      set_prepared_financial_wait_weights(true);
      set_prepared_financial_wait_due_weight(true);
      set_prepared_financial_wait_service_weight(true);
      System::set_block_number(1);
      let actor_id = Actors::next_actor_id();
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        None
      ));
      let Some(ActorSemanticState::Dormant(record)) = ActorSemanticStates::<Test>::get(actor_id)
      else {
        panic!("Dormant source");
      };
      let mut contract = user_active_contract(
        Schedule {
          trigger: Trigger::AtTime { after_ticks: 1 },
          cooldown_blocks: 0,
        },
        None,
        vec![make_step(Task::Transfer {
          to: BOB,
          asset: TestAsset::Local(1),
          amount: AmountResolution::Fixed(7),
        })]
        .try_into()
        .unwrap(),
      )
      .unwrap();
      contract.on_unfunded = crate::OnUnfunded::AwaitFunding;
      let successor = ActorRef {
        actor_id,
        generation: record.generation + 1,
      };
      let authority = <Test as crate::Config>::AdmissionCertificateAuthority::current().unwrap();
      let mut admission = ActorAdmissionCertificateOf::<Test>::new(
        contract.semantic_contract_id(),
        contract.body_commitment().unwrap(),
        if refusal == 2 {
          manual_schedule()
            .trigger
            .wake_qualification(&contract.window)
        } else {
          contract.trigger.wake_qualification(&contract.window)
        },
        authority.runtime_actor_semantics_version + u32::from(refusal == 3),
        authority.production_weight_identity,
        authority.body_geometry_version,
        authority.configured_bounds_commitment,
        authority.maximum_lifecycle_weight,
      );
      if refusal == 1 {
        admission.semantic_contract_id = [91; 32];
      }
      let hot = crate::ActorHotState {
        lifecycle: ActiveLifecycle::Active,
        cycle_state: CycleState::Idle,
        trigger_runtime_state: crate::TriggerRuntimeState::AtTime {
          anchor_tick: Some(2),
          consumed: false,
        },
        unsuccessful_attempt_streak: 0,
        pending_signal: false,
        queue_ticket: None,
        wakeup_pointer: None,
        trigger_wakeup_pointer: None,
        terminal_at: None,
        schedule_anchor: 2,
        last_cycle_block: None,
      };
      System::set_block_number(2);
      let key = at_time_backing_key(successor, contract.semantic_contract_id());
      assert!(!polkadot_sdk::sp_io::storage::exists(&key));
      let before = root();
      let sink = native_balance(&TestFeeSink::get());
      set_prepared_at_time_backing(refusal != 0);
      if refusal == 4 {
        set_prepared_financial_wait_due_weight(false);
      }
      if refusal == 6 {
        assert_noop!(
          Actors::activate_actor(RuntimeOrigin::signed(ALICE), actor_id, contract.clone()),
          Error::<Test>::AwaitFundingUnavailable
        );
      }
      let result = polkadot_sdk::frame_support::storage::with_transaction(|| {
        let result = (|| {
          Actors::insert_active_actor_with_admission(
            actor_id,
            record.identity.clone(),
            hot.clone(),
            contract.clone(),
            crate::TriggerTransitionIntent::ActivateDormant,
            admission.clone(),
          )?;
          crate::ActiveActorCount::<Test>::mutate(|count| *count += 1);
          Actors::reconcile_actor_state_hold_with_authority(actor_id)?;
          if refusal == 5 {
            return Err(polkadot_sdk::sp_runtime::DispatchError::Other(
              "late installation refusal",
            ));
          }
          Ok(())
        })();
        match result {
          Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
          Err(error) => {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
          }
        }
      });
      if refusal < 6 {
        assert!(result.is_err());
        assert_eq!(root(), before);
        assert_eq!(native_balance(&TestFeeSink::get()), sink);
        assert!(!polkadot_sdk::sp_io::storage::exists(&key));
      } else {
        assert_ok!(result);
        assert_eq!(native_balance(&TestFeeSink::get()), sink + 400);
        assert!(polkadot_sdk::sp_io::storage::exists(&key));
        assert_eq!(Actors::load_actor_ref(actor_id), Some(successor));
        assert_eq!(
          Actors::load_actor_contract(actor_id),
          Some(contract.clone())
        );
        assert_eq!(
          crate::TriggerDeadlineHandles::<Test>::get(actor_id)
            .unwrap()
            .key,
          WakeupKey::Tick(3)
        );
        assert!(actor_state_hold_total(actor_id) > 0);
        System::set_block_number(3);
        service_canonical_temporal_frontiers(3);
        let episode = ActorFinancialWaits::<Test>::get(actor_id).unwrap();
        assert_eq!(episode.record().scope().actor, successor);
        assert_eq!(
          episode.record().unpaid_service(),
          UnpaidActorService::AtTimeTrigger { due_tick: 3 }
        );
        assert_eq!(native_balance(&TestFeeSink::get()), sink + 400);
        assert_eq!(
          asset_balance(&record.identity.sovereign_account, TestAsset::Native),
          0
        );
        assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
      }
      set_prepared_financial_wait_due_weight(true);
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_paid_trigger_backs_opening_without_an_entry_payment() {
  new_test_ext().execute_with(|| {
    let sink = native_balance(&TestFeeSink::get());
    let (actor, unpaid) = prepared_source(Source::Opening);
    let sovereign = sovereign_account(actor.actor_id);
    let head = crate::ActorContractHeads::<Test>::get(actor.actor_id).unwrap();
    let trigger = Actors::trigger_fee_for_head(
      ActorType::User, &head, crate::TriggerFamily::Manual,
      <Test as crate::Config>::WeightInfo::manual_trigger(),
    ).unwrap();
    assert_eq!(trigger.trigger_fee, 600);
    assert_eq!(Actors::actor_cost_quote(actor.actor_id).unwrap().prospective_trigger_fee.unwrap().fee, trigger.trigger_fee);
    assert_eq!(native_balance(&TestFeeSink::get()), sink + TestActorCreationFee::get() + trigger.trigger_fee);
    assert!(has_actor_event(|event| matches!(event, Event::TriggerOccurrenceProcessed {
      actor_id, trigger_family: crate::TriggerFamily::Manual, fee: 600,
    } if *actor_id == actor.actor_id)));
    set_native_transfer_lock(&sovereign, native_balance(&sovereign));
    let fees = fee_collections();
    let sink = native_balance(&TestFeeSink::get());
    System::reset_events();
    run_canonical_block_at(2, Weight::MAX);
    let opening_wait = ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap();
    assert_eq!(opening_wait.record().unpaid_service(), unpaid);
    assert_eq!(opening_wait.record().unpaid_liability(), head.header.pipeline_machine_envelope.pipeline_machine_fee_upper);
    assert_eq!(native_balance(&TestFeeSink::get()), sink);
    assert_eq!(fee_collections(), fees);
    let before = root();
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), actor.actor_id));
    assert_eq!(root(), before);
    assert_eq!(fee_collections(), fees);
    set_prepared_financial_wait_ingress_weight(true);
    System::set_block_number(3);
    assert_ok!(deliver_hint(&hint_event(
      actor, ALICE, TestAsset::Native,
      opening_wait.record().unpaid_liability() + TestMinUserBalance::get(),
    ), &mut WeightMeter::with_limit(Weight::MAX)));
    run_canonical_block_at(4, Weight::MAX);
    assert_eq!(native_balance(&TestFeeSink::get()), sink + opening_wait.record().unpaid_liability());
    let action_wait = ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap();
    assert_eq!(action_wait.record().unpaid_service(), UnpaidActorService::RunningAction { cycle_nonce: 1, cursor: 0 });
    assert_ne!(action_wait.original_identity(), opening_wait.original_identity());
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
    assert!(has_actor_event(|event| matches!(event, Event::FinancialWaitRecovered { receipt } if *receipt == due_receipt(actor, opening_wait))));
    assert!(!has_actor_event(|event| matches!(event, Event::TriggerOccurrenceProcessed { .. } | Event::ActionFeeCharged { .. } | Event::StepSkipped { .. } | Event::StepFailed { .. })));
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn financial_wait_runtime_tariff_backs_initial_service_and_distinct_wait_recovery() {
  for stop in [false, true] {
    new_test_ext().execute_with(|| {
      let (actor, _) = prepared_source(Source::Opening);
      let mut contract = Actors::load_actor_contract(actor.actor_id).unwrap();
      contract.steps = vec![make_step(if stop {
        Task::StopCycle
      } else {
        Task::Transfer {
          to: BOB,
          asset: TestAsset::Local(1),
          amount: AmountResolution::Fixed(7),
        }
      })]
      .try_into()
      .unwrap();
      let resources = Actors::derive_step_resource_envelopes(&contract).unwrap();
      let price = Actors::derive_pipeline_machine_envelope(
        ActorType::User,
        crate::OnUnfunded::AwaitFunding,
        &contract.steps,
        &resources,
        Weight::zero(),
        Weight::zero(),
      )
      .unwrap();
      // Readiness already backs the first financial inspection. Pipeline control remains
      // separate from an Action's additional control visit, entry, due and recovery work.
      assert_eq!(
        price.pipeline_machine_fee_upper,
        if stop { 100 } else { 500 }
      );
      let close = Actors::derive_pipeline_machine_envelope(
        ActorType::User,
        crate::OnUnfunded::Close,
        &contract.steps,
        &resources,
        Weight::zero(),
        Weight::zero(),
      )
      .unwrap();
      assert_eq!(close.pipeline_machine_fee_upper, 100);
    });
  }
}

#[test]
fn financial_wait_runtime_trigger_reserve_requires_prices_and_preserves_base_quotes() {
  for zero_step in [false, true] {
    new_test_ext().execute_with(|| {
      let (actor, _) = prepared_source_with_options(
        Source::Opening,
        false,
        Mutability::Mutable,
        zero_step,
        false,
      );
      let head = crate::ActorContractHeads::<Test>::get(actor.actor_id).unwrap();
      let price = <Test as crate::Config>::WeightInfo::manual_trigger();
      assert_eq!(
        Actors::trigger_fee_for_head(ActorType::User, &head, crate::TriggerFamily::Manual, price)
          .unwrap()
          .trigger_fee,
        600
      );
      let mut close = head.clone();
      close.header.on_unfunded = crate::OnUnfunded::Close;
      for missing in 0..3 {
        set_prepared_financial_wait_weights(missing != 0);
        set_prepared_financial_wait_due_weight(missing != 1);
        set_prepared_financial_wait_service_weight(missing != 2);
        assert!(matches!(
          Actors::trigger_fee_for_head(ActorType::User, &head, crate::TriggerFamily::Manual, price),
          Err(Error::<Test>::AdmissionBoundOverflow)
        ));
        assert_eq!(
          Actors::trigger_fee_for_head(
            ActorType::User,
            &close,
            crate::TriggerFamily::Manual,
            price
          )
          .unwrap()
          .trigger_fee,
          100
        );
        assert_eq!(
          Actors::trigger_fee_for_head(
            ActorType::System,
            &head,
            crate::TriggerFamily::Manual,
            price
          )
          .unwrap()
          .trigger_fee,
          0
        );
      }
      set_prepared_financial_wait_weights(true);
      set_prepared_financial_wait_due_weight(true);
      set_prepared_financial_wait_service_weight(true);
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_paid_pipeline_collects_wait_backing_once() {
  new_test_ext().execute_with(|| {
    let actor = prepared_joint_source(true);
    let sovereign = sovereign_account(actor.actor_id);
    let head = crate::ActorContractHeads::<Test>::get(actor.actor_id).unwrap();
    let (contract, admission) = Actors::load_admitted_contract_geometry(actor.actor_id).unwrap();
    let before_geometry = root();
    set_missing_behavior_close_weight(true);
    let (recomputed, _) = Actors::decompose_admitted_contract_geometry(
      actor.actor_id, ActorType::User, &contract, &admission,
    ).expect("wait-backed Pipeline pricing excludes destruction authority");
    assert_eq!(recomputed.header.pipeline_machine_envelope, head.header.pipeline_machine_envelope);
    assert!(Actors::close_control_weight_for_contract(&contract).is_none());
    assert_eq!(root(), before_geometry);
    set_missing_behavior_close_weight(false);
    let before = native_balance(&sovereign);
    let sink = native_balance(&TestFeeSink::get());
    let readiness = Actors::trigger_fee_for_head(
      ActorType::User,
      &head,
      crate::TriggerFamily::Manual,
      <Test as crate::Config>::WeightInfo::manual_trigger(),
    ).unwrap().trigger_fee;
    assert_eq!(readiness, 600);
    assert_eq!(fee_collections().last(), Some(&readiness), "Useful readiness already collected first Opening inspection backing");
    let pipeline_fee = head.header.pipeline_machine_envelope.pipeline_machine_fee_upper;
    assert_eq!(pipeline_fee, 1_000);
    run_canonical_block_at(2, Weight::MAX);
    assert_eq!(native_balance(&sovereign), before - pipeline_fee - 100 - 7);
    assert_eq!(native_balance(&TestFeeSink::get()), sink + pipeline_fee + 100);
    set_native_transfer_lock(&sovereign, native_balance(&sovereign));
    run_canonical_block_at(3, Weight::MAX);
    let episode = ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap();
    assert_eq!(episode.record().unpaid_liability(), 100);
    System::set_block_number(4);
    assert_ok!(deliver_hint(&hint_event(actor, ALICE, TestAsset::Native, 100 + TestMinUserBalance::get() + 7), &mut WeightMeter::with_limit(Weight::MAX)));
    let sink = native_balance(&TestFeeSink::get());
    run_canonical_block_at(5, Weight::MAX);
    assert_eq!(native_balance(&TestFeeSink::get()), sink + 100);
    assert_eq!(System::events().iter().filter(|record| matches!(&record.event, RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id, .. }) if *actor_id == actor.actor_id)).count(), 1);
    assert!(Actors::actor_identity(actor.actor_id).is_none());
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn financial_wait_runtime_joint_clean_cycle_survives_pristine_and_repeated_action_waits() {
  new_test_ext().execute_with(|| {
    let actor = prepared_joint_source(true);
    let sovereign = sovereign_account(actor.actor_id);
    let bob = native_balance(&BOB);
    let head = crate::ActorContractHeads::<Test>::get(actor.actor_id).unwrap();
    let capacity = head.header.pipeline_machine_envelope.pipeline_machine_fee_upper + TestMinUserBalance::get();
    set_native_transfer_lock(&sovereign, native_balance(&sovereign) - capacity);
    let pristine_monitor = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id).unwrap();
    run_canonical_block_at(2, Weight::MAX);
    let first = ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap();
    assert_eq!(first.record().unpaid_service(), UnpaidActorService::RunningAction { cycle_nonce: 1, cursor: 0 });
    assert_eq!(ActorRunStateStore::<Test>::get(actor.actor_id).unwrap().cursor, 0);
    assert_eq!(crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id), Some(pristine_monitor));
    assert_eq!(native_balance(&BOB), bob);
    System::set_block_number(3);
    assert_ok!(deliver_hint(&hint_event(actor, ALICE, TestAsset::Native, first.record().unpaid_liability() + 7), &mut WeightMeter::with_limit(Weight::MAX)));
    run_canonical_block_at(4, Weight::MAX);
    assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    assert_eq!(native_balance(&BOB), bob + 7);
    let monitor = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id).unwrap();
    assert!(monitor.clean_cycle.is_some());
    let paid_run = ActorRunStateStore::<Test>::get(actor.actor_id).unwrap();
    let mut invalid = monitor.clone();
    invalid.clean_cycle = None;
    crate::ActorBehaviorMonitors::<Test>::insert(actor.actor_id, invalid);
    let before = root();
    assert!(matches!(Actors::load_actor_state(actor.actor_id), LoadedActorStateOf::Corrupt));
    assert_eq!(root(), before);
    crate::ActorBehaviorMonitors::<Test>::insert(actor.actor_id, monitor.clone());
    run_canonical_block_at(5, Weight::MAX);
    let second = ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap();
    assert_eq!(second.record().unpaid_service(), UnpaidActorService::RunningAction { cycle_nonce: 1, cursor: 1 });
    assert_ne!(second.original_identity(), first.original_identity());
    assert_eq!(second.record().scope().behavior_episode, first.record().scope().behavior_episode);
    assert!(second.record().expiry_tick() > first.record().expiry_tick());
    assert_eq!(crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id), Some(monitor));
    assert_eq!(ActorRunStateStore::<Test>::get(actor.actor_id), Some(paid_run));
    System::set_block_number(6);
    assert_ok!(deliver_hint(&hint_event(actor, ALICE, TestAsset::Native, second.record().unpaid_liability() + 7), &mut WeightMeter::with_limit(Weight::MAX)));
    run_canonical_block_at(7, Weight::MAX);
    assert_eq!(native_balance(&BOB), bob + 14);
    assert!(Actors::actor_identity(actor.actor_id).is_none());
    assert!(!crate::ActorBehaviorMonitors::<Test>::contains_key(actor.actor_id));
    assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    assert!(has_actor_event(|event| matches!(event, Event::ActorClosed { actor_id, reason: CloseReason::BehaviorPolicySatisfied } if *actor_id == actor.actor_id)));
    assert_eq!(System::events().iter().filter(|record| matches!(&record.event, RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id, .. }) if *actor_id == actor.actor_id)).count(), 1);
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn financial_wait_runtime_joint_freeze_preserves_monitor_and_committed_prefix() {
  for funded in [false, true] {
    new_test_ext().execute_with(|| {
      let actor = prepared_joint_source(true);
      let contract = Actors::actor_contract(actor.actor_id).unwrap();
      assert!(
        Actors::close_control_weight_for_contract(&contract)
          .unwrap()
          .all_lte(
            MockAdmissionCertificateAuthority::current()
              .unwrap()
              .maximum_lifecycle_weight
          )
      );
      let sovereign = sovereign_account(actor.actor_id);
      let bob = native_balance(&BOB);
      let head = crate::ActorContractHeads::<Test>::get(actor.actor_id).unwrap();
      let capacity = head
        .header
        .pipeline_machine_envelope
        .pipeline_machine_fee_upper
        + TestMinUserBalance::get()
        + 100
        + 7;
      set_native_transfer_lock(&sovereign, native_balance(&sovereign) - capacity);
      run_canonical_block_at(2, Weight::MAX);
      assert_eq!(native_balance(&BOB), bob + 7);
      run_canonical_block_at(3, Weight::MAX);
      let episode = ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap();
      assert_eq!(
        episode.record().unpaid_service(),
        UnpaidActorService::RunningAction {
          cycle_nonce: 1,
          cursor: 1,
        }
      );
      let snapshot = || {
        (
          ActorSemanticStates::<Test>::get(actor.actor_id),
          ActorProcesses::<Test>::get(actor.actor_id),
          ActorRunStateStore::<Test>::get(actor.actor_id),
          ActorFinancialWaits::<Test>::get(actor.actor_id),
          crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id),
          crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id),
          DeadlineHandles::<Test>::get(actor.actor_id),
          Actors::actor_state_hold(actor.actor_id),
        )
          .encode()
      };
      let original = snapshot();
      let monitor = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id);
      let policy = crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id).encode();
      let paid_run = ActorRunStateStore::<Test>::get(actor.actor_id);
      let fees = fee_collections();
      if funded {
        make_service_solvent(actor, episode);
        fund_native(actor.actor_id, 7);
      }
      let native = native_balance(&sovereign);
      assert_ok!(Actors::set_global_circuit_breaker(
        RuntimeOrigin::root(),
        true
      ));
      let now = episode.record().expiry_tick() + 1;
      System::set_block_number(now);
      System::reset_events();
      assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
      assert_eq!(snapshot(), original);
      assert_eq!(native_balance(&sovereign), native);
      assert_eq!(native_balance(&BOB), bob + 7);
      assert_eq!(fee_collections(), fees);
      assert_ok!(Actors::set_global_circuit_breaker(
        RuntimeOrigin::root(),
        false
      ));
      Actors::on_idle(now, Weight::MAX);
      assert_eq!(snapshot(), original);
      assert_eq!(native_balance(&BOB), bob + 7);
      Actors::on_finalize(now);
      run_canonical_block_at(now + 1, Weight::MAX);
      if funded {
        assert_eq!(
          crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id),
          monitor
        );
        assert_eq!(
          crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id).encode(),
          policy
        );
        assert_eq!(ActorRunStateStore::<Test>::get(actor.actor_id), paid_run);
        assert_eq!(
          ActorFinancialWaits::<Test>::get(actor.actor_id),
          Some(episode)
        );
        assert_eq!(native_balance(&BOB), bob + 7);
        assert_eq!(fee_collections(), fees);
        run_canonical_block_at(now + 2, Weight::MAX);
        assert_eq!(native_balance(&BOB), bob + 14);
        assert_eq!(fee_collections().len(), fees.len() + 1);
        assert!(has_actor_event(|event| matches!(event,
          Event::FinancialWaitRecovered { receipt } if *receipt == due_receipt(actor, episode))));
        assert!(has_actor_event(|event| matches!(event,
          Event::ActorClosed { actor_id, reason: CloseReason::BehaviorPolicySatisfied }
            if *actor_id == actor.actor_id)));
        assert!(!has_actor_event(|event| matches!(
          event,
          Event::PipelineFeeCharged { .. } | Event::CycleStarted { .. }
        )));
      } else {
        assert_eq!(native_balance(&sovereign), native);
        assert_eq!(native_balance(&BOB), bob + 7);
        assert_eq!(fee_collections(), fees);
        assert!(has_actor_event(|event| matches!(event,
          Event::ActorClosed { actor_id, reason: CloseReason::FundingGraceExpired }
            if *actor_id == actor.actor_id)));
        assert!(!has_actor_event(|event| matches!(
          event,
          Event::FinancialWaitRecovered { .. } | Event::ActionFeeCharged { .. }
        )));
      }
      assert!(Actors::actor_identity(actor.actor_id).is_none());
      assert!(!ActorRunStateStore::<Test>::contains_key(actor.actor_id));
      assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
      assert!(!crate::ActorBehaviorPolicies::<Test>::contains_key(
        actor.actor_id
      ));
      assert!(!crate::ActorBehaviorMonitors::<Test>::contains_key(
        actor.actor_id
      ));
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_joint_monitor_preserves_prefix_clean_cycle_and_terminal_rollback() {
  for clean in [false, true] {
    for late_refusal in [false, true] {
      new_test_ext().execute_with(|| {
        let actor = prepared_joint_source(clean);
        let sovereign = sovereign_account(actor.actor_id);
        let bob = native_balance(&BOB);
        run_canonical_block_at(2, Weight::MAX);
        assert_eq!(native_balance(&BOB), bob + 7);
        let monitor = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id).unwrap();
        assert_eq!(monitor.clean_cycle.is_some(), clean);
        let paid_run = ActorRunStateStore::<Test>::get(actor.actor_id).unwrap();
        let fees = fee_collections();
        set_native_transfer_lock(&sovereign, native_balance(&sovereign));
        System::reset_events();
        run_canonical_block_at(3, Weight::MAX);
        let episode = ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap();
        let behavior_episode = episode.record().scope().behavior_episode.unwrap();
        let policy = crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id).unwrap();
        let admission = Actors::load_control_admission(actor.actor_id).unwrap();
        let bound = policy.plan.bind(&policy.expression, crate::BehaviorPolicyBinding {
          semantic_contract_id: admission.semantic_contract_id,
          admission_identity: admission.admission_identity,
        }).unwrap();
        assert_eq!(bound.value(&monitor.state, behavior_episode, crate::BehaviorMetric::ActionSuccessTotal), Ok(Some(1)));
        if clean {
          assert_eq!(bound.value(&monitor.state, behavior_episode, crate::BehaviorMetric::CycleCleanTotal), Ok(Some(0)));
        }
        assert_eq!(episode.record().unpaid_service(), UnpaidActorService::RunningAction { cycle_nonce: 1, cursor: 1 });
        assert_eq!(crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id), Some(monitor.clone()));
        assert_eq!(ActorRunStateStore::<Test>::get(actor.actor_id), Some(paid_run));
        assert_eq!(native_balance(&BOB), bob + 7);
        assert_eq!(fee_collections(), fees);
        assert!(!has_actor_event(|event| matches!(event, Event::StepSkipped { .. } | Event::ActionFeeCharged { .. } | Event::CycleSummary { .. })));
        System::set_block_number(4);
        let event = hint_event(actor, ALICE, TestAsset::Native, episode.record().unpaid_liability() + TestMinUserBalance::get() + 7);
        assert_ok!(deliver_hint(&event, &mut WeightMeter::with_limit(Weight::MAX)));
        assert_eq!(crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id), Some(monitor.clone()));
        assert_eq!(ActorFinancialWaits::<Test>::get(actor.actor_id), Some(episode));
        System::set_block_number(5);
        System::reset_events();
        if late_refusal {
          set_fail_fee_sink_transfer(true);
          let before = root();
          let balances = (native_balance(&sovereign), native_balance(&BOB));
          let mut meter = WeightMeter::with_limit(Weight::MAX);
          assert_eq!(Actors::service_canonical_round_head(&mut meter, 5), Err(crate::ServiceRoundError::FeeCollection));
          assert_eq!(root(), before);
          assert_eq!((native_balance(&sovereign), native_balance(&BOB)), balances);
          assert_eq!(crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id), Some(monitor));
          assert_eq!(ActorFinancialWaits::<Test>::get(actor.actor_id), Some(episode));
          assert!(meter.consumed().ref_time() > 0);
          set_fail_fee_sink_transfer(false);
        }
        assert_ok!(Actors::service_canonical_round_head(&mut WeightMeter::with_limit(Weight::MAX), 5));
        assert_eq!(native_balance(&BOB), bob + 14);
        assert!(Actors::actor_identity(actor.actor_id).is_none());
        assert!(!crate::ActorBehaviorMonitors::<Test>::contains_key(actor.actor_id));
        assert!(!crate::ActorBehaviorPolicies::<Test>::contains_key(actor.actor_id));
        assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
        assert!(has_actor_event(|event| matches!(event, Event::ActorClosed { actor_id, reason: CloseReason::BehaviorPolicySatisfied } if *actor_id == actor.actor_id)));
        assert!(has_actor_event(|event| matches!(event, Event::FinancialWaitRecovered { receipt } if *receipt == due_receipt(actor, episode))));
        assert!(!has_actor_event(|event| matches!(event, Event::PipelineFeeCharged { .. } | Event::CycleStarted { .. })));
        assert_eq!(fee_collections().len(), fees.len() + 1 + usize::from(late_refusal));
        #[cfg(feature = "try-runtime")]
        assert_ok!(Actors::do_try_state());
      });
    }
  }
}

fn entry_weight() -> Weight {
  MockAdmissionCertificateAuthority::financial_wait_entry_weight(
    FinancialWaitEpisode::<Balance>::max_encoded_len() as u32,
  )
  .unwrap()
}

fn parked_source(source: Source) -> (ActorRef, FinancialWaitEpisode<Balance>) {
  let (actor, unpaid) = prepared_source(source);
  let episode = Actors::publish_prepared_financial_wait(
    actor,
    unpaid,
    &mut WeightMeter::with_limit(Weight::MAX),
  )
  .unwrap();
  set_prepared_financial_wait_due_weight(true);
  (actor, episode)
}

fn due_weight() -> Weight {
  let (due, cleanup) = Actors::financial_wait_due_weights().unwrap();
  due + cleanup
}

fn due_receipt(
  actor: ActorRef,
  episode: FinancialWaitEpisode<Balance>,
) -> crate::FinancialWaitReceipt {
  crate::FinancialWaitReceipt {
    actor,
    episode_identity: episode.original_identity(),
    unpaid_service: episode.record().unpaid_service(),
  }
}

fn make_service_solvent(actor: ActorRef, episode: FinancialWaitEpisode<Balance>) {
  let sovereign = sovereign_account(actor.actor_id);
  set_native_transfer_lock(&sovereign, 0);
  fund_native(
    actor.actor_id,
    episode.record().unpaid_liability() + TestMinUserBalance::get(),
  );
}

fn funded_source_service(source: Source) -> (ActorRef, FinancialWaitEpisode<Balance>, u64) {
  let (actor, unpaid) = prepared_source_with_native_tail(source, true);
  funded_service(actor, unpaid)
}

fn funded_service(
  actor: ActorRef,
  unpaid: UnpaidActorService,
) -> (ActorRef, FinancialWaitEpisode<Balance>, u64) {
  let episode = Actors::publish_prepared_financial_wait(
    actor,
    unpaid,
    &mut WeightMeter::with_limit(Weight::MAX),
  )
  .unwrap();
  set_prepared_financial_wait_due_weight(true);
  set_prepared_financial_wait_service_weight(true);
  make_service_solvent(actor, episode);
  let due = episode.record().expiry_tick();
  System::set_block_number(due);
  assert_ok!(Actors::process_financial_wait_due(
    actor,
    due,
    &mut WeightMeter::with_limit(Weight::MAX)
  ));
  System::set_block_number(due + 1);
  (actor, episode, due + 1)
}

#[test]
fn financial_wait_runtime_cadenced_recovery_needs_no_new_owner_hold() {
  for source in [Source::Opening, Source::Running, Source::Suspended] {
    new_test_ext().execute_with(|| {
      let (actor, unpaid) = prepared_source_with_funding(
        source,
        false,
        Mutability::Mutable,
        false,
        false,
        None,
        None,
        true,
      );
      let installed_hold = Actors::actor_state_hold(actor.actor_id).unwrap();
      let prefix = asset_balance(&BOB, TestAsset::Local(1));
      let (actor, episode, now) = funded_service(actor, unpaid);
      assert_ok!(Balances::transfer_all(
        RuntimeOrigin::signed(ALICE),
        CHARLIE,
        false
      ));
      assert_eq!(Balances::free_balance(ALICE), 1);
      fund_native(actor.actor_id, 10_000);
      set_temporary_dex_failure(false);
      System::reset_events();
      run_canonical_block_at(now, Weight::MAX);
      assert!(
        !ActorFinancialWaits::<Test>::contains_key(actor.actor_id),
        "funded service must commit even when the owner has no unheld reserve"
      );
      assert_eq!(
        Actors::actor_state_hold(actor.actor_id),
        Some(installed_hold)
      );
      let state = Actors::active_actor_state(actor.actor_id).unwrap();
      assert_eq!(state.hot.trigger_wakeup_pointer.unwrap().tick, now + 1);
      assert!(has_actor_event(|event| matches!(event,
        Event::FinancialWaitRecovered { receipt } if *receipt == due_receipt(actor, episode))));
      assert_eq!(
        asset_balance(&BOB, TestAsset::Local(1)),
        prefix
          + if matches!(source, Source::Suspended) {
            0
          } else {
            7
          }
      );
      assert_eq!(Balances::free_balance(ALICE), 1);
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_cadenced_recovery_rearms_from_current_time_without_catchup() {
  for source in [Source::Opening, Source::Running, Source::Suspended] {
    new_test_ext().execute_with(|| {
      let (actor, unpaid) = prepared_source_with_funding(
        source,
        false,
        Mutability::Mutable,
        false,
        false,
        None,
        None,
        true,
      );
      let anchor = Actors::actor_hot(actor.actor_id).unwrap().trigger_runtime_state.temporal_anchor_tick();
      let identity = Actors::actor_identity(actor.actor_id).unwrap();
      let prefix = asset_balance(&BOB, TestAsset::Local(1));
      let (actor, episode, service_block) = funded_service(actor, unpaid);
      fund_native(actor.actor_id, 10_000);
      set_temporary_dex_failure(false);
      assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(actor.actor_id));
      assert_eq!(ActorFinancialWaits::<Test>::get(actor.actor_id), Some(episode));
      System::reset_events();
      run_canonical_block_at(service_block, Weight::MAX);
      assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
      let state = Actors::active_actor_state(actor.actor_id).unwrap();
      assert_eq!(state.hot.trigger_runtime_state.temporal_anchor_tick(), anchor);
      assert_eq!(state.identity.cycle_nonce, identity.cycle_nonce + u64::from(!matches!(source, Source::Opening)));
      let next_tick = service_block + 1;
      assert_eq!(state.hot.trigger_wakeup_pointer.unwrap().tick, next_tick);
      assert_eq!(crate::TriggerDeadlineHandles::<Test>::get(actor.actor_id).unwrap().key, WakeupKey::Tick(next_tick));
      assert!(!state.hot.pending_signal);
      assert_eq!(has_actor_event(|event| matches!(event, Event::CycleStarted { .. })), matches!(source, Source::Opening));
      assert!(has_actor_event(|event| matches!(event, Event::FinancialWaitRecovered { receipt } if *receipt == due_receipt(actor, episode))));
      assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), prefix + if matches!(source, Source::Suspended) { 0 } else { 7 });
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
      // One ordinary cadence point can resume future cycles; no missed point is replayed.
      System::reset_events();
      run_canonical_block_at(next_tick, Weight::MAX);
      run_canonical_block_at(next_tick + 1, Weight::MAX);
      if matches!(source, Source::Opening) {
        run_canonical_block_at(next_tick + 2, Weight::MAX);
      }
      assert_eq!(Actors::actor_identity(actor.actor_id).unwrap().cycle_nonce, identity.cycle_nonce + 1);
      assert_eq!(Actors::actor_run_state(actor.actor_id).unwrap().cycle_nonce, identity.cycle_nonce + 2);
      assert_eq!(System::events().iter().filter(|record| matches!(&record.event, RuntimeEvent::Actors(Event::CycleStarted { actor_id, .. }) if *actor_id == actor.actor_id)).count(), 1);
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

fn hint_source(source: Source) -> (ActorRef, FinancialWaitEpisode<Balance>) {
  let result = parked_source(source);
  set_prepared_financial_wait_service_weight(true);
  set_prepared_financial_wait_ingress_weight(true);
  result
}

#[test]
fn financial_wait_runtime_eligibility_preserves_original_at_time_debt_and_residence() {
  for source in [
    Source::AtTime,
    Source::Opening,
    Source::Running,
    Source::Suspended,
  ] {
    new_test_ext().execute_with(|| {
      let (actor, episode) = hint_source(source);
      let sovereign = sovereign_account(actor.actor_id);
      let original_custody = native_balance(&sovereign);
      let debt = episode.record().unpaid_liability();
      let minimum = TestMinUserBalance::get();
      set_native_transfer_lock(&sovereign, 0);
      for (custody, spendable, shortfall) in [
        (minimum.saturating_sub(1), 0, debt),
        (minimum, 0, debt),
        (minimum + debt - 1, debt - 1, 1),
        (minimum + debt, debt, 0),
        (minimum + debt + 1, debt + 1, 0),
        (1_u128 << 100, (1_u128 << 100) - minimum, 0),
      ] {
        let _ = <Balances as Currency<AccountId>>::make_free_balance_be(&sovereign, custody);
        let before = root();
        let crate::ActorEligibility::Active(view) = eligibility(actor.actor_id) else {
          panic!("Active financial wait at every funding boundary");
        };
        let wait = view.financial_wait.unwrap();
        assert_eq!(wait.unpaid_liability, debt);
        assert_eq!(wait.current_service_spendability, spendable);
        assert_eq!(wait.funding_shortfall, shortfall);
        assert_eq!(
          ActorFinancialWaits::<Test>::get(actor.actor_id),
          Some(episode)
        );
        assert_eq!(root(), before);
      }
      let _ = <Balances as Currency<AccountId>>::make_free_balance_be(&sovereign, original_custody);
      set_native_transfer_lock(&sovereign, original_custody);
      let before = root();
      let crate::ActorEligibility::Active(view) = eligibility(actor.actor_id) else {
        panic!("Active financial wait");
      };
      let expected_wait = crate::FinancialWaitProjection {
        unpaid_service: episode.record().unpaid_service(),
        unpaid_liability: debt,
        current_service_spendability: 0,
        funding_shortfall: debt,
        entered_at_millis: episode.record().entered_at_millis(),
        expiry_tick: episode.record().expiry_tick(),
      };
      assert_eq!(view.financial_wait, Some(expected_wait));
      assert_eq!(
        view.residence,
        crate::ActorActivationResidence::Deadline(WakeupKey::Tick(episode.record().expiry_tick()))
      );
      if matches!(source, Source::AtTime) {
        assert_eq!(
          view.eligibility.execution_phase,
          ActorExecutionPhase::WaitingCadenceTick(match episode.record().unpaid_service() {
            UnpaidActorService::AtTimeTrigger { due_tick } => due_tick,
            _ => panic!("original AtTime obligation"),
          })
        );
      }
      assert_eq!(root(), before);
      let event = hint_event(
        actor,
        ALICE,
        TestAsset::Native,
        episode.record().unpaid_liability() + TestMinUserBalance::get() + 1_000,
      );
      assert_ok!(deliver_hint(
        &event,
        &mut WeightMeter::with_limit(Weight::MAX)
      ));
      let before = root();
      let crate::ActorEligibility::Active(view) = eligibility(actor.actor_id) else {
        panic!("Active financial Service");
      };
      let Some(ProcessResidence::Service(kind)) = ActorProcesses::<Test>::get(actor.actor_id)
        .unwrap()
        .residence
      else {
        panic!("financial Service residence");
      };
      assert_eq!(
        view.residence,
        crate::ActorActivationResidence::Service(kind)
      );
      let mut funded_wait = expected_wait;
      funded_wait.current_service_spendability =
        MockAssetOps::balance(&sovereign, TestAsset::Native).saturating_sub(minimum);
      funded_wait.funding_shortfall = 0;
      assert_eq!(view.financial_wait, Some(funded_wait));
      assert_eq!(
        ActorFinancialWaits::<Test>::get(actor.actor_id),
        Some(episode)
      );
      assert_eq!(root(), before);
    });
  }
}

#[test]
fn financial_wait_runtime_cadenced_recovery_admits_namespace_cleanup_before_action() {
  for refusal in 0..3 {
    new_test_ext().execute_with(|| {
      let (actor, unpaid) = prepared_source_with_funding(
        Source::Running, true, Mutability::Mutable, false, false, None, None, true,
      );
      let (actor, episode, now) = funded_service(actor, unpaid);
      let destination = WakeupKey::Tick(now + 1);
      for _ in 0..32 {
        create_system_with(ALICE, timer_schedule(1), None, inert_contract_steps());
      }
      crate::DeadlineHeaders::<Test>::mutate(destination, |header| {
        let header = header.as_mut().unwrap();
        assert!(header.first_vacant_page.is_none());
        header.next_page = u64::MAX;
      });
      let (state, _, step) = Actors::load_current_step_service_state(actor.actor_id).unwrap();
      let terminal = Actors::service_terminal_control_upper(&state, Some(&step.step));
      assert_eq!(terminal, Actors::close_control_weight_for_contract(&state.contract).unwrap());
      let inspection = TestWeightInfo::service_round_begin_populated()
        + TestWeightInfo::service_round_probe_eligible()
        + TestWeightInfo::scheduler_actor_state_probe()
        + MockAdmissionCertificateAuthority::financial_wait_service_weight(FinancialWaitEpisode::<Balance>::max_encoded_len() as u32).unwrap();
      let suffix = TestWeightInfo::service_round_admit_eligible().max(
        TestWeightInfo::service_member_retire_interior()
          .max(TestWeightInfo::service_member_retire_pair_cursor())
          .max(TestWeightInfo::service_member_retire_singleton()),
      );
      let complete = inspection + step.resources.control + entry_weight() + terminal + suffix + step.resources.effect;
      let limit = match refusal {
        0 => complete - Weight::from_parts(1, 0),
        1 => complete - Weight::from_parts(0, 1),
        _ => complete,
      };
      System::reset_events();
      let before = root();
      let sovereign = sovereign_account(actor.actor_id);
      let balances = (native_balance(&sovereign), native_balance(&BOB));
      let fees = fee_collections();
      let mut meter = WeightMeter::with_limit(limit);
      let result = Actors::service_canonical_round_head(&mut meter, now);
      assert_eq!((native_balance(&sovereign), native_balance(&BOB)), balances);
      assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 7);
      assert_eq!(fee_collections(), fees);
      if refusal < 2 {
        assert!(result.is_err());
        assert_eq!(root(), before);
        assert_eq!(meter.consumed(), inspection
          - MockAdmissionCertificateAuthority::financial_wait_service_weight(
            FinancialWaitEpisode::<Balance>::max_encoded_len() as u32,
          ).unwrap());
        assert_eq!(ActorFinancialWaits::<Test>::get(actor.actor_id), Some(episode));
      } else {
        assert_ok!(result);
        assert!(Actors::actor_identity(actor.actor_id).is_none());
        assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
        assert!(has_actor_event(|event| matches!(event, Event::ActorClosed { actor_id, reason: CloseReason::SchedulerIndexExhausted } if *actor_id == actor.actor_id)));
        assert!(!has_actor_event(|event| matches!(event, Event::FinancialWaitRecovered { .. } | Event::ActionFeeCharged { .. })));
      }
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

fn hint_event(
  actor: ActorRef,
  source: AccountId,
  asset: TestAsset,
  amount: Balance,
) -> crate::AddressEvent<AccountId, TestAsset, Balance> {
  crate::AddressEvent {
    destination: sovereign_account(actor.actor_id),
    source: Some(source),
    asset,
    amount,
    provenance: Some(crate::FundingProvenance::Signed),
  }
}

/// Native post-movement producer model. Its extra complete maximum is explicitly admitted and
/// retained on rollback; it does not establish production pricing or ordinary transaction fees.
fn deliver_hint(
  event: &crate::AddressEvent<AccountId, TestAsset, Balance>,
  meter: &mut WeightMeter,
) -> Result<(), crate::IngressFailure> {
  let weight = Actors::financial_wait_ingress_weight_upper().ok_or(
    crate::IngressFailure::temporary(Error::<Test>::AwaitFundingUnavailable),
  )?;
  if !meter.can_consume(weight) {
    return Err(crate::IngressFailure::temporary(
      Error::<Test>::QueueCapacityUnavailable,
    ));
  }
  meter.consume(weight);
  polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
    let result = (|| {
      Actors::preflight_ingress(event)?;
      MockAssetOps::transfer(
        &event.source.unwrap(),
        &event.destination,
        event.asset,
        event.amount,
      )
      .map_err(|failure| crate::IngressFailure {
        error: failure.error,
        retry: failure.retry,
      })?;
      Actors::notify_ingress(event)
    })();
    match result {
      Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
      Err(error) => polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error)),
    }
  })
}

#[test]
fn financial_wait_runtime_funded_ingress_requires_fresh_return_backing() {
  new_test_ext().execute_with(|| {
    let (actor, episode) = hint_source(Source::Running);
    set_financial_return_backing(TestFinancialReturnBacking::Unavailable);
    let event = hint_event(
      actor,
      ALICE,
      TestAsset::Native,
      episode.record().unpaid_liability() + TestMinUserBalance::get(),
    );
    let before = root();
    let bound = Actors::financial_wait_ingress_weight_upper().unwrap();
    let mut meter = WeightMeter::with_limit(bound);
    assert_eq!(
      deliver_hint(&event, &mut meter),
      Err(crate::IngressFailure::temporary(
        Error::<Test>::AwaitFundingUnavailable
      ))
    );
    assert_eq!(root(), before);
    assert_eq!(meter.consumed(), bound);
    assert_eq!(
      ActorFinancialWaits::<Test>::get(actor.actor_id),
      Some(episode)
    );
    assert!(financial_return_backings().is_empty());
  });
}

#[test]
fn financial_wait_runtime_return_backing_cannot_debit_waiting_custody() {
  new_test_ext().execute_with(|| {
    let (actor, episode) = hint_source(Source::Running);
    set_financial_return_backing(TestFinancialReturnBacking::ActorCustody);
    let event = hint_event(
      actor,
      ALICE,
      TestAsset::Native,
      episode.record().unpaid_liability() + TestMinUserBalance::get(),
    );
    let before = root();
    assert!(deliver_hint(&event, &mut WeightMeter::with_limit(Weight::MAX)).is_err());
    assert_eq!(root(), before);
    assert_eq!(
      ActorFinancialWaits::<Test>::get(actor.actor_id),
      Some(episode)
    );
    assert!(financial_return_backings().is_empty());
  });
}

#[test]
fn financial_wait_runtime_unfunded_hint_and_due_do_not_require_return_backing() {
  new_test_ext().execute_with(|| {
    let (actor, episode) = hint_source(Source::Running);
    set_financial_return_backing(TestFinancialReturnBacking::Unavailable);
    assert_ok!(deliver_hint(
      &hint_event(actor, ALICE, TestAsset::Native, 1),
      &mut WeightMeter::with_limit(Weight::MAX)
    ));
    assert!(financial_return_backings().is_empty());
    assert!(DeadlineHandles::<Test>::contains_key(actor.actor_id));
    make_service_solvent(actor, episode);
    let due = episode.record().expiry_tick();
    System::set_block_number(due);
    assert_ok!(Actors::process_financial_wait_due(
      actor,
      due,
      &mut WeightMeter::with_limit(Weight::MAX)
    ));
    assert!(financial_return_backings().is_empty());
    assert!(crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
    assert_eq!(
      ActorFinancialWaits::<Test>::get(actor.actor_id),
      Some(episode)
    );
  });
}

#[test]
fn financial_wait_runtime_certified_hint_returns_each_boundary_and_commits_only_owed_work() {
  for source in [
    Source::AtTime,
    Source::Opening,
    Source::Running,
    Source::Suspended,
  ] {
    new_test_ext().execute_with(|| {
      let (actor, episode) = hint_source(source);
      let now = System::block_number();
      let run = ActorRunStateStore::<Test>::get(actor.actor_id);
      let hot = Actors::actor_hot(actor.actor_id).unwrap();
      let contract = Actors::load_actor_contract(actor.actor_id).unwrap();
      let mode = if run.is_some() {
        crate::SimulationMode::CurrentRun
      } else {
        crate::SimulationMode::FreshCurrentPlan
      };
      let simulate = || {
        assert_eq!(
          ActorProcesses::<Test>::get(actor.actor_id).unwrap().status,
          ProcessStatus::AwaitingFunding
        );
        let before = root();
        let result = Actors::simulate_current_contract(
          actor.actor_id,
          ActorType::User,
          Mutability::Mutable,
          contract.clone(),
          mode,
          ample_simulation_budget(),
        );
        assert_eq!(root(), before);
        result
      };
      assert_eq!(simulate(), Err(crate::SimulationError::NotReady));
      let fees = fee_collections().len();
      let bob = asset_balance(&BOB, TestAsset::Local(1));
      let event = hint_event(
        actor,
        ALICE,
        TestAsset::Native,
        episode.record().unpaid_liability() + TestMinUserBalance::get() + 1_000,
      );
      let mut meter =
        WeightMeter::with_limit(Actors::financial_wait_ingress_weight_upper().unwrap());
      System::reset_events();
      assert_ok!(deliver_hint(&event, &mut meter));
      assert_eq!(
        meter.consumed(),
        Actors::financial_wait_ingress_weight_upper().unwrap()
      );
      assert_eq!(
        ActorFinancialWaits::<Test>::get(actor.actor_id),
        Some(episode)
      );
      assert_eq!(ActorRunStateStore::<Test>::get(actor.actor_id), run);
      assert_eq!(Actors::actor_hot(actor.actor_id), Some(hot));
      assert_eq!(fee_collections().len(), fees);
      assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), bob);
      assert!(!DeadlineHandles::<Test>::contains_key(actor.actor_id));
      let node = crate::ServiceNodes::<Test>::get(actor.actor_id).unwrap();
      assert_eq!(node.eligible_from, now + 1);
      assert_eq!(
        node.kind,
        if matches!(source, Source::Opening) {
          ServiceResidenceKind::Pending
        } else {
          ServiceResidenceKind::Live
        }
      );
      assert!(!has_actor_event(|event| matches!(
        event,
        Event::FinancialWaitRecovered { .. }
          | Event::TriggerOccurrenceProcessed { .. }
          | Event::CycleStarted { .. }
          | Event::ActionFeeCharged { .. }
      )));
      assert_ok!(Actors::service_canonical_round_head(
        &mut WeightMeter::with_limit(Weight::MAX),
        now
      ));
      assert_eq!(
        ActorFinancialWaits::<Test>::get(actor.actor_id),
        Some(episode)
      );
      System::set_block_number(now + 1);
      assert_eq!(simulate(), Err(crate::SimulationError::NotReady));
      assert_eq!(fee_collections().len(), fees);
      assert_eq!(
        ActorFinancialWaits::<Test>::get(actor.actor_id),
        Some(episode)
      );
      assert_eq!(ActorRunStateStore::<Test>::get(actor.actor_id), run);
      assert_ok!(Actors::service_canonical_round_head(
        &mut WeightMeter::with_limit(Weight::MAX),
        now + 1
      ));
      assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
      assert_eq!(
        fee_collections().len(),
        fees
          + if matches!(source, Source::Opening) {
            2
          } else {
            1
          }
      );
      assert!(has_actor_event(
        |event| matches!(event, Event::FinancialWaitRecovered { receipt }
        if *receipt == due_receipt(actor, episode))
      ));
      if matches!(source, Source::AtTime) {
        assert!(Actors::pending_signal(actor.actor_id));
        assert!(!ActorRunStateStore::<Test>::contains_key(actor.actor_id));
      }
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_hints_use_current_balance_authorization_and_original_expiry() {
  new_test_ext().execute_with(|| {
    let (actor, episode) = hint_source(Source::Opening);
    let source = DeadlineHandles::<Test>::get(actor.actor_id);
    let hot = Actors::actor_hot(actor.actor_id);
    let fees = fee_collections();
    let required = episode.record().unpaid_liability() + TestMinUserBalance::get();
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    assert_ok!(deliver_hint(
      &hint_event(actor, ALICE, TestAsset::Native, required - 1),
      &mut meter
    ));
    assert_eq!(DeadlineHandles::<Test>::get(actor.actor_id), source);
    set_asset_balance(&ALICE, TestAsset::Local(2), 2);
    assert_ok!(deliver_hint(
      &hint_event(actor, ALICE, TestAsset::Local(2), 1),
      &mut meter
    ));
    assert_eq!(
      asset_balance(&sovereign_account(actor.actor_id), TestAsset::Local(2)),
      1
    );
    assert_eq!(DeadlineHandles::<Test>::get(actor.actor_id), source);
    // Rejected funding remains custody. A later authorized hint still checks the whole current
    // spendable balance, with no source-history accumulator or deposit-delta readiness threshold.
    assert_ok!(deliver_hint(
      &hint_event(actor, BOB, TestAsset::Native, 1),
      &mut meter
    ));
    assert_eq!(DeadlineHandles::<Test>::get(actor.actor_id), source);
    assert_ok!(Actors::notify_address_event_without_source(
      actor.actor_id,
      TestAsset::Native,
      1
    ));
    assert_eq!(DeadlineHandles::<Test>::get(actor.actor_id), source);
    assert_eq!(
      ActorFinancialWaits::<Test>::get(actor.actor_id),
      Some(episode)
    );
    assert_eq!(Actors::actor_hot(actor.actor_id), hot);
    assert_ok!(deliver_hint(
      &hint_event(actor, ALICE, TestAsset::Native, 1),
      &mut meter
    ));
    let node = crate::ServiceNodes::<Test>::get(actor.actor_id).unwrap();
    assert!(!DeadlineHandles::<Test>::contains_key(actor.actor_id));
    System::set_block_number(System::block_number() + 1);
    assert_ok!(deliver_hint(
      &hint_event(actor, ALICE, TestAsset::Native, 1),
      &mut meter
    ));
    assert_eq!(crate::ServiceNodes::<Test>::get(actor.actor_id), Some(node));
    assert_eq!(
      ActorFinancialWaits::<Test>::get(actor.actor_id),
      Some(episode)
    );
    assert_eq!(fee_collections(), fees);
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn financial_wait_runtime_hint_funding_policy_authenticates_source_and_provenance() {
  for policy in 0..4 {
    for authorized in [false, true] {
      new_test_ext().execute_with(|| {
        let funding = match policy {
          0 => FundingSourcePolicy::OwnerOnly,
          1 => FundingSourcePolicy::SignedAllowlist(
            alloc::collections::BTreeSet::from([BOB])
              .try_into()
              .unwrap(),
          ),
          2 => FundingSourcePolicy::RuntimePolicy,
          3 => FundingSourcePolicy::AnyVerifiedIngress,
          _ => unreachable!(),
        };
        let (actor, unpaid) = prepared_source_with_funding(
          Source::Opening,
          false,
          Mutability::Mutable,
          false,
          false,
          Some(funding),
          None,
          false,
        );
        let episode = Actors::publish_prepared_financial_wait(
          actor,
          unpaid,
          &mut WeightMeter::with_limit(Weight::MAX),
        )
        .unwrap();
        set_prepared_financial_wait_service_weight(true);
        set_prepared_financial_wait_ingress_weight(true);
        let required = episode.record().unpaid_liability() + TestMinUserBalance::get();
        let source = if policy == 0 && authorized {
          ALICE
        } else {
          BOB
        };
        if policy < 2 {
          let mut event = hint_event(actor, source, TestAsset::Native, required);
          if policy == 1 && !authorized {
            event.provenance = Some(crate::FundingProvenance::InternalProtocol);
          }
          assert_ok!(deliver_hint(
            &event,
            &mut WeightMeter::with_limit(Weight::MAX)
          ));
        } else {
          // Source-less protocol ingress uses its typed provenance. Ledger-only funding plus
          // the unverified all-None alias supplies no authenticated hint even with an allowing host.
          fund_native(actor.actor_id, required);
          assert_ok!(Actors::notify_ingress(&crate::AddressEvent {
            destination: sovereign_account(actor.actor_id),
            source: None,
            asset: TestAsset::Native,
            amount: required,
            provenance: authorized.then_some(crate::FundingProvenance::InternalProtocol),
          }));
        }
        assert_eq!(
          crate::ServiceNodes::<Test>::contains_key(actor.actor_id),
          authorized
        );
        assert_eq!(
          DeadlineHandles::<Test>::contains_key(actor.actor_id),
          !authorized
        );
        assert_eq!(
          ActorFinancialWaits::<Test>::get(actor.actor_id),
          Some(episode)
        );
        #[cfg(feature = "try-runtime")]
        assert_ok!(Actors::do_try_state());
      });
    }
  }
}

#[test]
fn financial_wait_runtime_hint_refusals_preserve_producer_ledger_and_original_source() {
  for refusal in 0..7 {
    new_test_ext().execute_with(|| {
      let (actor, episode) = hint_source(Source::Opening);
      set_financial_return_backing(TestFinancialReturnBacking::SignedProducer);
      let event = hint_event(
        actor,
        ALICE,
        TestAsset::Native,
        episode.record().unpaid_liability() + TestMinUserBalance::get(),
      );
      let complete = Actors::financial_wait_ingress_weight_upper().unwrap();
      let mut limit = complete;
      match refusal {
        0 => limit = Weight::from_parts(complete.ref_time() - 1, u64::MAX),
        1 => limit = Weight::from_parts(u64::MAX, complete.proof_size() - 1),
        2 => set_prepared_financial_wait_ingress_weight(false),
        3 => ActorProcesses::<Test>::mutate(actor.actor_id, |process| {
          process.as_mut().unwrap().generation += 1;
        }),
        4 => crate::ServiceHeader::<Test>::mutate(|header| {
          header.round_block = Some(System::block_number() + 1);
        }),
        5 => System::set_block_number(u64::MAX),
        6 => set_fail_fee_sink_transfer(true),
        _ => unreachable!(),
      }
      let before = root();
      let fees = fee_collections();
      let mut meter = WeightMeter::with_limit(limit);
      let result = deliver_hint(&event, &mut meter);
      assert!(result.is_err(), "refusal {refusal}");
      assert_eq!(
        meter.consumed(),
        if refusal < 3 {
          Weight::zero()
        } else {
          complete
        }
      );
      assert_eq!(root(), before, "refusal {refusal}");
      assert_eq!(fee_collections(), fees);
      assert!(financial_return_backings().is_empty());
      assert_eq!(
        ActorFinancialWaits::<Test>::get(actor.actor_id),
        Some(episode)
      );
      assert!(DeadlineHandles::<Test>::contains_key(actor.actor_id));
      if refusal == 2 {
        assert_eq!(
          Actors::preflight_ingress(&event),
          Err(crate::IngressFailure::temporary(
            Error::<Test>::AwaitFundingUnavailable
          ))
        );
      }
    });
  }
}

#[test]
fn financial_wait_runtime_hint_preserves_independent_window_close_and_credited_custody() {
  new_test_ext().execute_with(|| {
    let (actor, unpaid) = prepared_source_with_funding(
      Source::Opening,
      false,
      Mutability::Mutable,
      false,
      false,
      None,
      Some(crate::ScheduleWindow { start: 1, end: 101 }),
      false,
    );
    let episode = Actors::publish_prepared_financial_wait(
      actor,
      unpaid,
      &mut WeightMeter::with_limit(Weight::MAX),
    )
    .unwrap();
    set_prepared_financial_wait_service_weight(true);
    set_prepared_financial_wait_ingress_weight(true);
    let event = hint_event(
      actor,
      ALICE,
      TestAsset::Native,
      episode.record().unpaid_liability() + TestMinUserBalance::get(),
    );
    let balance = native_balance(&event.destination);
    let fees = fee_collections();
    System::set_block_number(102);
    assert!(System::block_number() < episode.record().expiry_tick());
    assert_ok!(deliver_hint(
      &event,
      &mut WeightMeter::with_limit(Weight::MAX)
    ));
    assert!(Actors::actor_identity(actor.actor_id).is_none());
    assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    assert_eq!(native_balance(&event.destination), balance + event.amount);
    assert_eq!(fee_collections(), fees);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        reason: CloseReason::WindowExpired,
        ..
      }
    )));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::FinancialWaitRecovered { .. }
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn financial_wait_runtime_ingress_qualification_precedes_cold_wait_reads() {
  let mut ext = new_test_ext();
  let (event, wait_key, contract_key) = ext.execute_with(|| {
    let (actor, episode) = hint_source(Source::Opening);
    let event = hint_event(
      actor,
      ALICE,
      TestAsset::Native,
      episode.record().unpaid_liability() + TestMinUserBalance::get(),
    );
    set_prepared_financial_wait_ingress_weight(false);
    (
      event,
      ActorFinancialWaits::<Test>::hashed_key_for(actor.actor_id),
      crate::ActorContractHeads::<Test>::hashed_key_for(actor.actor_id),
    )
  });
  ext.commit_all().unwrap();
  let recorder =
    polkadot_sdk::sp_trie::recorder::Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
  ext.execute_with_recorder(recorder.clone(), || {
    assert_eq!(
      Actors::preflight_ingress(&event),
      Err(crate::IngressFailure::temporary(
        Error::<Test>::AwaitFundingUnavailable
      ))
    );
  });
  for key in [&wait_key, &contract_key] {
    assert!(
      !recorder
        .recorded_keys()
        .values()
        .any(|keys| keys.keys().any(|read| read.as_ref() == key.as_slice()))
    );
  }
  let recorder =
    polkadot_sdk::sp_trie::recorder::Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
  ext.execute_with_recorder(recorder.clone(), || {
    set_prepared_financial_wait_ingress_weight(true);
    assert_ok!(Actors::preflight_ingress(&event));
  });
  for key in [&wait_key, &contract_key] {
    assert!(
      recorder
        .recorded_keys()
        .values()
        .any(|keys| keys.keys().any(|read| read.as_ref() == key.as_slice()))
    );
  }
}

#[test]
fn financial_wait_runtime_frozen_hint_rechecks_preflight_and_rolls_back_native_movement() {
  new_test_ext().execute_with(|| {
    let (actor, episode) = hint_source(Source::Opening);
    let event = hint_event(
      actor,
      ALICE,
      TestAsset::Native,
      episode.record().unpaid_liability() + TestMinUserBalance::get(),
    );
    assert_ok!(Actors::preflight_ingress(&event));
    crate::GlobalCircuitBreaker::<Test>::put(true);
    let before = root();
    let result = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_ok!(MockAssetOps::transfer(
        &ALICE,
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
    assert_eq!(root(), before);
    assert_eq!(
      ActorFinancialWaits::<Test>::get(actor.actor_id),
      Some(episode)
    );
    set_prepared_financial_wait_ingress_weight(false);
    let self_event = hint_event(actor, event.destination, TestAsset::Native, 1);
    assert_ok!(Actors::preflight_ingress(&self_event));
    assert_ok!(Actors::notify_ingress(&self_event));
    assert_ok!(Actors::notify_address_event(
      actor.actor_id,
      TestAsset::Native,
      1,
      &event.destination
    ));
    assert_ok!(Actors::notify_ingress(&crate::AddressEvent {
      amount: 0,
      ..event
    }));
    assert_eq!(root(), before);
  });
}

#[test]
fn financial_wait_runtime_freeze_preserves_overdue_deadline_and_tentative_service() {
  for source in [
    Source::AtTime,
    Source::Opening,
    Source::Running,
    Source::Suspended,
  ] {
    for service_resident in [false, true] {
      for funded in [false, true] {
        new_test_ext().execute_with(|| {
          let (actor, episode) = hint_source(source);
          let sovereign = sovereign_account(actor.actor_id);
          if service_resident {
            assert_ok!(deliver_hint(
              &hint_event(actor, ALICE, TestAsset::Native,
                episode.record().unpaid_liability() + TestMinUserBalance::get() + 1_000),
              &mut WeightMeter::with_limit(Actors::financial_wait_ingress_weight_upper().unwrap()),
            ));
            assert!(crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
          }
          if funded {
            make_service_solvent(actor, episode);
            fund_native(actor.actor_id, 1_000);
          } else {
            set_native_transfer_lock(&sovereign, native_balance(&sovereign));
          }
          let snapshot = || (
            ActorSemanticStates::<Test>::get(actor.actor_id),
            ActorProcesses::<Test>::get(actor.actor_id),
            Actors::actor_contract(actor.actor_id),
            crate::ActorContractHeads::<Test>::get(actor.actor_id),
            ActorRunStateStore::<Test>::get(actor.actor_id),
            ActorFinancialWaits::<Test>::get(actor.actor_id),
            DeadlineHandles::<Test>::get(actor.actor_id),
            crate::TriggerDeadlineHandles::<Test>::get(actor.actor_id),
            crate::ServiceNodes::<Test>::get(actor.actor_id),
            Actors::actor_state_hold(actor.actor_id),
          ).encode();
          let original = snapshot();
          let native = native_balance(&sovereign);
          let custody = asset_balance(&sovereign, TestAsset::Local(1));
          let bob = asset_balance(&BOB, TestAsset::Local(1));
          let fees = fee_collections();
          assert_ok!(Actors::set_global_circuit_breaker(RuntimeOrigin::root(), true));
          let now = episode.record().expiry_tick() + 17;
          System::set_block_number(now);
          System::reset_events();
          assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
          let resources = Actors::block_resource_state().unwrap();
          assert!(resources.optional_actor_work_halted());
          assert_eq!(resources.usage().actor_effect_used(), Weight::zero());
          assert_eq!(snapshot(), original);
          assert_eq!(native_balance(&sovereign), native);
          assert_eq!(asset_balance(&sovereign, TestAsset::Local(1)), custody);
          assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), bob);
          assert_eq!(fee_collections(), fees);
          assert!(!has_actor_event(|event| matches!(event,
            Event::ActorClosed { .. } | Event::FinancialWaitRecovered { .. }
            | Event::ActionFeeCharged { .. } | Event::CycleStarted { .. })));
          assert_ok!(Actors::set_global_circuit_breaker(RuntimeOrigin::root(), false));
          Actors::on_idle(now, Weight::MAX);
          assert_eq!(snapshot(), original);
          assert_eq!(fee_collections(), fees);
          Actors::on_finalize(now);
          System::reset_events();
          run_canonical_block_at(now + 1, Weight::MAX);
          if funded {
            if !service_resident {
              assert_eq!(ActorFinancialWaits::<Test>::get(actor.actor_id), Some(episode));
              assert_eq!(crate::ServiceNodes::<Test>::get(actor.actor_id).unwrap().eligible_from,
                now + 2);
              assert_eq!(fee_collections(), fees);
              run_canonical_block_at(now + 2, Weight::MAX);
            }
            assert!(Actors::actor_identity(actor.actor_id).is_some());
            assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
            assert!(has_actor_event(|event| matches!(event,
              Event::FinancialWaitRecovered { receipt } if *receipt == due_receipt(actor, episode))));
            assert!(!has_actor_event(|event| matches!(event,
              Event::ActorClosed { reason: CloseReason::FundingGraceExpired, .. })));
          } else {
            assert!(Actors::actor_identity(actor.actor_id).is_none());
            assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
            assert!(!DeadlineHandles::<Test>::contains_key(actor.actor_id));
            assert!(!crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
            assert_eq!(native_balance(&sovereign), native);
            assert_eq!(asset_balance(&sovereign, TestAsset::Local(1)), custody);
            assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), bob);
            assert_eq!(fee_collections(), fees);
            assert!(has_actor_event(|event| matches!(event,
              Event::ActorClosed { actor_id, reason: CloseReason::FundingGraceExpired }
                if *actor_id == actor.actor_id)));
            assert!(!has_actor_event(|event| matches!(event,
              Event::FinancialWaitRecovered { .. } | Event::ActionFeeCharged { .. })));
          }
          #[cfg(feature = "try-runtime")]
          assert_ok!(Actors::do_try_state());
        });
      }
    }
  }
}

#[test]
fn financial_wait_runtime_unpaid_at_time_enters_once_without_consuming_or_charging_trigger() {
  for mutability in [Mutability::Mutable, Mutability::Immutable] {
    for zero_step in [false, true] {
      new_test_ext().execute_with(|| {
        let (actor, unpaid) =
          prepared_source_with_options(Source::AtTime, false, mutability, zero_step, false);
        set_prepared_financial_wait_service_weight(true);
        let UnpaidActorService::AtTimeTrigger { due_tick } = unpaid else {
          panic!("AtTime source");
        };
        let mut hot = Actors::actor_hot(actor.actor_id).unwrap();
        hot.trigger_wakeup_pointer = None;
        let fees = fee_collections();
        let sovereign = sovereign_account(actor.actor_id);
        let balance = native_balance(&sovereign);
        let selector = TestWeightInfo::classify_due_tick_deadline();
        let occurrence = TestWeightInfo::at_time_trigger_occurrence()
          .max(TestWeightInfo::cadenced_trigger_occurrence());
        let complete = selector + Actors::temporal_deadline_weight_upper(occurrence);
        System::reset_events();
        let mut meter = WeightMeter::with_limit(complete);
        assert_eq!(
          Actors::process_next_due_tick_deadline(&mut meter, due_tick),
          Ok(crate::DueTickDeadlineMutation::TemporalTriggerProcessed(
            actor
          ))
        );
        assert_eq!(meter.consumed(), selector + occurrence + entry_weight());
        let episode = ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap();
        assert_eq!(episode.record().unpaid_service(), unpaid);
        assert_eq!(Actors::actor_hot(actor.actor_id), Some(hot));
        assert_eq!(native_balance(&sovereign), balance);
        assert_eq!(fee_collections(), fees);
        assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
        assert!(!ActorRunStateStore::<Test>::contains_key(actor.actor_id));
        assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
          actor.actor_id
        ));
        assert_eq!(
          crate::DeadlineHandles::<Test>::get(actor.actor_id)
            .unwrap()
            .key,
          WakeupKey::Tick(episode.record().expiry_tick())
        );
        assert_eq!(
          ActorProcesses::<Test>::get(actor.actor_id).unwrap().status,
          ProcessStatus::AwaitingFunding
        );
        assert!(has_actor_event(
          |event| matches!(event, Event::FinancialWaitEntered { receipt }
          if *receipt == due_receipt(actor, episode))
        ));
        assert!(!has_actor_event(|event| matches!(
          event,
          Event::CycleStarted { .. } | Event::StepSkipped { .. } | Event::ActionFeeCharged { .. }
        )));
        let before = root();
        let mut second = WeightMeter::with_limit(complete);
        assert!(Actors::process_next_due_tick_deadline(&mut second, due_tick).is_err());
        assert_eq!(second.consumed(), selector);
        assert_eq!(root(), before);
        #[cfg(feature = "try-runtime")]
        assert_ok!(Actors::do_try_state());
      });
    }
  }
}

#[test]
fn financial_wait_runtime_at_time_entry_admits_wait_and_cleanup_before_source_mutation() {
  new_test_ext().execute_with(|| {
    let (actor, unpaid) = prepared_source(Source::AtTime);
    set_prepared_financial_wait_service_weight(true);
    let UnpaidActorService::AtTimeTrigger { due_tick } = unpaid else {
      panic!("AtTime source");
    };
    let selector = TestWeightInfo::classify_due_tick_deadline();
    let occurrence = TestWeightInfo::at_time_trigger_occurrence()
      .max(TestWeightInfo::cadenced_trigger_occurrence());
    let complete = selector + Actors::temporal_deadline_weight_upper(occurrence);
    let before = root();
    let fees = fee_collections();
    for limit in [
      Weight::from_parts(complete.ref_time() - 1, u64::MAX),
      Weight::from_parts(u64::MAX, complete.proof_size() - 1),
    ] {
      let mut meter = WeightMeter::with_limit(limit);
      assert_eq!(
        Actors::process_next_due_tick_deadline(&mut meter, due_tick),
        Err(crate::DependencyReviewWorkerError::InsufficientWeight)
      );
      assert_eq!(meter.consumed(), selector);
      assert_eq!(root(), before);
      assert_eq!(fee_collections(), fees);
    }
    assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    assert!(crate::TriggerDeadlineHandles::<Test>::contains_key(
      actor.actor_id
    ));
  });
}

#[test]
fn financial_wait_runtime_at_time_entry_late_hold_authority_refusal_restores_source_and_keeps_work()
{
  new_test_ext().execute_with(|| {
    let (actor, unpaid) = prepared_source(Source::AtTime);
    set_prepared_financial_wait_service_weight(true);
    let UnpaidActorService::AtTimeTrigger { due_tick } = unpaid else {
      panic!("AtTime source");
    };
    // Entry retains detector capacity and therefore performs no currency release. Corrupt the
    // hold owner to exercise its remaining fallible reconciliation after provisional publication.
    crate::ActorStateHolds::<Test>::mutate(actor.actor_id, |hold| {
      hold.as_mut().unwrap().owner = BOB;
    });
    let selector = TestWeightInfo::classify_due_tick_deadline();
    let occurrence = TestWeightInfo::at_time_trigger_occurrence()
      .max(TestWeightInfo::cadenced_trigger_occurrence());
    let complete = selector + Actors::temporal_deadline_weight_upper(occurrence);
    let before = root();
    let fees = fee_collections();
    let mut meter = WeightMeter::with_limit(complete);
    assert!(Actors::process_next_due_tick_deadline(&mut meter, due_tick).is_err());
    assert_eq!(meter.consumed(), complete);
    assert_eq!(root(), before);
    assert_eq!(fee_collections(), fees);
    assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    assert!(crate::TriggerDeadlineHandles::<Test>::contains_key(
      actor.actor_id
    ));
  });
}

#[test]
fn financial_wait_runtime_at_time_entry_preserves_funded_occurrence_and_independent_terminal() {
  for terminal in [false, true] {
    new_test_ext().execute_with(|| {
      let (actor, unpaid) = prepared_source(Source::AtTime);
      set_prepared_financial_wait_service_weight(true);
      let UnpaidActorService::AtTimeTrigger { due_tick } = unpaid else {
        panic!("AtTime source");
      };
      let sovereign = sovereign_account(actor.actor_id);
      if terminal {
        ActorSemanticStates::<Test>::mutate(actor.actor_id, |state| {
          let Some(ActorSemanticState::Active(record)) = state else {
            panic!("active source");
          };
          record.identity.cycle_nonce = u64::MAX;
        });
      } else {
        set_native_transfer_lock(&sovereign, 0);
        fund_native(actor.actor_id, 1_000);
      }
      let before = native_balance(&sovereign);
      let fees = fee_collections().len();
      let selector = TestWeightInfo::classify_due_tick_deadline();
      let occurrence = TestWeightInfo::at_time_trigger_occurrence()
        .max(TestWeightInfo::cadenced_trigger_occurrence());
      let complete = selector + Actors::temporal_deadline_weight_upper(occurrence);
      let mut meter = WeightMeter::with_limit(complete);
      assert_ok!(Actors::process_next_due_tick_deadline(&mut meter, due_tick));
      assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
      assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
        actor.actor_id
      ));
      if terminal {
        assert_eq!(meter.consumed(), complete - entry_weight());
        assert!(Actors::actor_identity(actor.actor_id).is_none());
        assert_eq!(native_balance(&sovereign), before);
        assert_eq!(fee_collections().len(), fees);
        assert!(has_actor_event(|event| matches!(
          event,
          Event::ActorClosed {
            reason: CloseReason::CycleNonceExhausted,
            ..
          }
        )));
      } else {
        assert_eq!(meter.consumed(), selector + occurrence);
        let hot = Actors::actor_hot(actor.actor_id).unwrap();
        assert!(hot.pending_signal);
        assert!(matches!(
          hot.trigger_runtime_state,
          TriggerRuntimeState::AtTime { consumed: true, .. }
        ));
        assert_eq!(fee_collections().len(), fees + 1);
        assert_eq!(native_balance(&sovereign), before - fee_collections()[fees]);
      }
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_at_time_entry_reaches_funded_recovery_or_expiry_through_tick_worker() {
  for funded in [false, true] {
    new_test_ext().execute_with(|| {
      let (actor, unpaid) = prepared_source(Source::AtTime);
      set_prepared_financial_wait_service_weight(true);
      let UnpaidActorService::AtTimeTrigger { due_tick } = unpaid else {
        panic!("AtTime source");
      };
      assert_ok!(Actors::process_next_due_tick_deadline(
        &mut WeightMeter::with_limit(Weight::MAX),
        due_tick
      ));
      let episode = ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap();
      set_prepared_financial_wait_due_weight(true);
      set_prepared_financial_wait_service_weight(true);
      if funded {
        make_service_solvent(actor, episode);
      }
      let sovereign = sovereign_account(actor.actor_id);
      let balance = native_balance(&sovereign);
      let fees = fee_collections().len();
      let expiry = episode.record().expiry_tick();
      System::set_block_number(expiry);
      assert_eq!(
        Actors::process_next_due_tick_deadline(&mut WeightMeter::with_limit(Weight::MAX), expiry),
        Ok(if funded {
          crate::DueTickDeadlineMutation::FinancialWaitReturned(due_receipt(actor, episode))
        } else {
          crate::DueTickDeadlineMutation::FinancialWaitClosed(due_receipt(actor, episode))
        })
      );
      assert_eq!(native_balance(&sovereign), balance);
      assert_eq!(fee_collections().len(), fees);
      if funded {
        assert_eq!(
          ActorFinancialWaits::<Test>::get(actor.actor_id),
          Some(episode)
        );
        System::set_block_number(expiry + 1);
        assert_ok!(Actors::service_canonical_round_head(
          &mut WeightMeter::with_limit(Weight::MAX),
          expiry + 1
        ));
        assert_eq!(
          native_balance(&sovereign),
          balance - episode.record().unpaid_liability()
        );
        assert_eq!(fee_collections().len(), fees + 1);
        assert_eq!(
          ActorProcesses::<Test>::get(actor.actor_id)
            .unwrap()
            .residence,
          Some(ProcessResidence::Service(ServiceResidenceKind::Pending))
        );
        assert!(Actors::pending_signal(actor.actor_id));
        assert!(has_actor_event(
          |event| matches!(event, Event::FinancialWaitRecovered { receipt }
          if *receipt == due_receipt(actor, episode))
        ));
      } else {
        assert!(Actors::actor_identity(actor.actor_id).is_none());
        assert!(has_actor_event(|event| matches!(
          event,
          Event::ActorClosed {
            reason: CloseReason::FundingGraceExpired,
            ..
          }
        )));
      }
      assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
      assert!(!ActorRunStateStore::<Test>::contains_key(actor.actor_id));
      assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_service_enters_the_exact_unpaid_boundary_without_a_step_result() {
  for source in [Source::Opening, Source::Running, Source::Suspended] {
    new_test_ext().execute_with(|| {
      let (actor, unpaid) = prepared_source_with_native_tail(source, true);
      set_prepared_financial_wait_service_weight(true);
      let mut now = System::block_number() + 1;
      if let Some(ProcessResidence::Deadline {
        key: WakeupKey::Block(due),
        ..
      }) = ActorProcesses::<Test>::get(actor.actor_id)
        .unwrap()
        .residence
      {
        System::set_block_number(due);
        assert_ok!(Actors::return_due_deadline_member_to_service(
          actor,
          ServiceResidenceKind::Live,
          due
        ));
        now = due + 1;
      }
      if let Some(run) = ActorRunStateStore::<Test>::get(actor.actor_id) {
        now = now.max(run.eligible_at);
      }
      System::set_block_number(now);
      let original_run = ActorRunStateStore::<Test>::get(actor.actor_id);
      let original_hot = Actors::actor_hot(actor.actor_id).unwrap();
      let fees = fee_collections();
      let sovereign = sovereign_account(actor.actor_id);
      let native = native_balance(&sovereign);
      System::reset_events();
      let (_, attempt) = Actors::service_canonical_round_head_inner(
        &mut WeightMeter::with_limit(Weight::MAX),
        now,
        None,
        crate::BlockResourceDomain::ActorBaseEffect,
        None,
      )
      .unwrap();
      assert!(attempt.is_none());
      let episode = ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap();
      assert_eq!(episode.record().unpaid_service(), unpaid);
      assert_eq!(
        ActorRunStateStore::<Test>::get(actor.actor_id),
        original_run
      );
      assert_eq!(Actors::actor_hot(actor.actor_id), Some(original_hot));
      assert_eq!(native_balance(&sovereign), native);
      assert_eq!(fee_collections(), fees);
      assert!(!has_actor_event(|event| matches!(
        event,
        Event::StepSkipped { .. }
          | Event::StepFailed { .. }
          | Event::TransferExecuted { .. }
          | Event::ActionFeeCharged { .. }
          | Event::CycleSuspended { .. }
          | Event::CycleContinued { .. }
          | Event::CycleStarted { .. }
      )));
      assert!(has_actor_event(
        |event| matches!(event, Event::FinancialWaitEntered { receipt }
        if *receipt == due_receipt(actor, episode))
      ));
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_opening_keeps_paid_cycle_at_step_zero_and_never_charges_it_again() {
  new_test_ext().execute_with(|| {
    let (actor, original, now) = funded_source_service(Source::Opening);
    let sovereign = sovereign_account(actor.actor_id);
    let capacity = original.record().unpaid_liability() + TestMinUserBalance::get();
    set_native_transfer_lock(&sovereign, native_balance(&sovereign) - capacity);
    let fees = fee_collections().len();
    let before = native_balance(&sovereign);
    let bob = native_balance(&BOB);
    System::reset_events();
    let (_, attempt) = Actors::service_canonical_round_head_inner(
      &mut WeightMeter::with_limit(Weight::MAX),
      now,
      None,
      crate::BlockResourceDomain::ActorBaseEffect,
      None,
    )
    .unwrap();
    assert!(attempt.is_none());
    assert_eq!(
      native_balance(&sovereign),
      before - original.record().unpaid_liability()
    );
    assert_eq!(native_balance(&BOB), bob);
    assert_eq!(fee_collections().len(), fees + 1);
    let action_wait = ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap();
    assert_eq!(
      action_wait.record().unpaid_service(),
      UnpaidActorService::RunningAction {
        cycle_nonce: 1,
        cursor: 0
      }
    );
    assert_ne!(
      action_wait.original_identity(),
      original.original_identity()
    );
    assert!(action_wait.record().expiry_tick() > original.record().expiry_tick());
    let run = ActorRunStateStore::<Test>::get(actor.actor_id).unwrap();
    assert!(run.running_is_coherent());
    assert_eq!(run.cursor, 0);
    assert_eq!(run.unsuccessful_attempts_at_cursor, 0);
    assert_eq!(run.last_committed_step_block, None);
    assert_eq!(run.last_step_outcome, None);
    assert_eq!(run.cumulative_outcomes, OutcomeTotals::default());
    assert_eq!(
      Actors::actor_identity(actor.actor_id).unwrap().cycle_nonce,
      0
    );
    assert!(!Actors::actor_hot(actor.actor_id).unwrap().pending_signal);
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::TransferExecuted { .. }
        | Event::StepSkipped { .. }
        | Event::StepFailed { .. }
        | Event::ActionFeeCharged { .. }
        | Event::CycleSummary { .. }
        | Event::CycleSuspended { .. }
        | Event::CycleContinued { .. }
    )));
    assert!(has_actor_event(
      |event| matches!(event, Event::FinancialWaitRecovered { receipt }
      if *receipt == due_receipt(actor, original))
    ));
    assert!(has_actor_event(
      |event| matches!(event, Event::FinancialWaitEntered { receipt }
      if *receipt == due_receipt(actor, action_wait))
    ));
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
    make_service_solvent(actor, action_wait);
    let due = action_wait.record().expiry_tick();
    System::set_block_number(due);
    assert_ok!(Actors::process_financial_wait_due(
      actor,
      due,
      &mut WeightMeter::with_limit(Weight::MAX)
    ));
    System::set_block_number(due + 1);
    System::reset_events();
    let fees = fee_collections().len();
    assert_ok!(Actors::service_canonical_round_head(
      &mut WeightMeter::with_limit(Weight::MAX),
      due + 1
    ));
    assert_eq!(native_balance(&BOB), bob + 7);
    assert_eq!(fee_collections().len(), fees + 1);
    assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    assert_eq!(
      ActorRunStateStore::<Test>::get(actor.actor_id)
        .unwrap()
        .cursor,
      1
    );
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::PipelineFeeCharged { .. } | Event::CycleStarted { .. }
    )));
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn financial_wait_runtime_pristine_paid_run_rejects_fabricated_progress_or_attempts() {
  new_test_ext().execute_with(|| {
    let (actor, original, now) = funded_source_service(Source::Opening);
    let sovereign = sovereign_account(actor.actor_id);
    let capacity = original.record().unpaid_liability() + TestMinUserBalance::get();
    set_native_transfer_lock(&sovereign, native_balance(&sovereign) - capacity);
    assert_ok!(Actors::service_canonical_round_head(
      &mut WeightMeter::with_limit(Weight::MAX),
      now
    ));
    let pristine = ActorRunStateStore::<Test>::get(actor.actor_id).unwrap();
    for corruption in 0..8 {
      let mut invalid = pristine.clone();
      match corruption {
        0 => invalid.cycle_nonce = 0,
        1 => invalid.cursor = 1,
        2 => invalid.unsuccessful_attempts_at_cursor = 1,
        3 => invalid.last_step_outcome = Some(StepOutcome::Executed),
        4 => invalid.cumulative_outcomes.executed_steps = 1,
        5 => invalid.suspension = Some(SuspensionReason::FundingUnavailable),
        6 => invalid.last_attempt_block = invalid.eligible_at,
        _ => invalid.last_committed_step_block = Some(now),
      }
      assert!(!invalid.running_is_coherent());
      ActorRunStateStore::<Test>::insert(actor.actor_id, invalid);
      assert!(matches!(
        Actors::load_actor_state(actor.actor_id),
        LoadedActorStateOf::Corrupt
      ));
    }
    ActorRunStateStore::<Test>::insert(actor.actor_id, pristine);
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn financial_wait_runtime_opening_funded_action_and_false_predicate_commit_the_original_boundary() {
  for false_predicate in [false, true] {
    new_test_ext().execute_with(|| {
      let (actor, unpaid) = prepared_source_with_options(
        Source::Opening,
        true,
        Mutability::Mutable,
        false,
        false_predicate,
      );
      let (actor, original, now) = funded_service(actor, unpaid);
      let sovereign = sovereign_account(actor.actor_id);
      let action = Actors::load_current_step_with_admission(
        actor.actor_id,
        0,
        &match ActorSemanticStates::<Test>::get(actor.actor_id).unwrap() {
          ActorSemanticState::Active(record) => record.admission,
          _ => panic!("prepared Opening"),
        },
      )
      .unwrap();
      let action_fee =
        Actors::maximum_current_action_fee(ActorType::User, &action.step, action.resources)
          .unwrap()
          .total_fee;
      if !false_predicate {
        fund_native(actor.actor_id, action_fee + 7);
      }
      let fees = fee_collections().len();
      let before = native_balance(&sovereign);
      let bob = native_balance(&BOB);
      System::reset_events();
      let (_, attempt) = Actors::service_canonical_round_head_inner(
        &mut WeightMeter::with_limit(Weight::MAX),
        now,
        None,
        crate::BlockResourceDomain::ActorBaseEffect,
        None,
      )
      .unwrap();
      assert!(attempt.is_some());
      assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
      assert_eq!(
        ActorRunStateStore::<Test>::get(actor.actor_id)
          .unwrap()
          .cursor,
        1
      );
      assert_eq!(
        fee_collections().len(),
        fees + if false_predicate { 1 } else { 2 }
      );
      assert_eq!(
        native_balance(&sovereign),
        before
          - original.record().unpaid_liability()
          - if false_predicate { 0 } else { action_fee + 7 }
      );
      assert_eq!(
        native_balance(&BOB),
        bob + if false_predicate { 0 } else { 7 }
      );
      assert!(has_actor_event(
        |event| matches!(event, Event::FinancialWaitRecovered { receipt }
        if *receipt == due_receipt(actor, original))
      ));
      assert!(!has_actor_event(|event| matches!(
        event,
        Event::FinancialWaitEntered { .. }
      )));
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_zero_step_opening_recovers_and_completes_without_an_action_fee() {
  new_test_ext().execute_with(|| {
    let (actor, unpaid) =
      prepared_source_with_options(Source::Opening, false, Mutability::Mutable, true, false);
    let (actor, original, now) = funded_service(actor, unpaid);
    let fees = fee_collections().len();
    System::reset_events();
    assert_ok!(Actors::service_canonical_round_head(
      &mut WeightMeter::with_limit(Weight::MAX),
      now
    ));
    assert_eq!(fee_collections().len(), fees + 1);
    assert_eq!(
      Actors::actor_identity(actor.actor_id).unwrap().cycle_nonce,
      1
    );
    assert!(!ActorRunStateStore::<Test>::contains_key(actor.actor_id));
    assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActionFeeCharged { .. }
    )));
    assert!(has_actor_event(
      |event| matches!(event, Event::FinancialWaitRecovered { receipt }
      if *receipt == due_receipt(actor, original))
    ));
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn financial_wait_runtime_opening_late_control_and_collector_refusals_restore_original_authority() {
  for late_control in [false, true] {
    new_test_ext().execute_with(|| {
      let (actor, original, now) = funded_source_service(Source::Opening);
      fund_native(actor.actor_id, 1_000);
      if late_control {
        set_step_control_actual_weight_override(Some(Weight::MAX));
      } else {
        set_fail_fee_sink_transfer(true);
      }
      let before = root();
      let bob = native_balance(&BOB);
      assert!(
        Actors::service_canonical_round_head(&mut WeightMeter::with_limit(Weight::MAX), now)
          .is_err()
      );
      assert_eq!(root(), before);
      assert_eq!(native_balance(&BOB), bob);
      assert_eq!(
        ActorFinancialWaits::<Test>::get(actor.actor_id),
        Some(original)
      );
      assert!(!ActorRunStateStore::<Test>::contains_key(actor.actor_id));
      assert!(Actors::actor_hot(actor.actor_id).unwrap().pending_signal);
      set_step_control_actual_weight_override(None);
      set_fail_fee_sink_transfer(false);
      assert_ok!(Actors::service_canonical_round_head(
        &mut WeightMeter::with_limit(Weight::MAX),
        now
      ));
      assert_eq!(native_balance(&BOB), bob + 7);
    });
  }
}

#[test]
fn financial_wait_runtime_at_time_recovery_charges_only_original_trigger_and_defers_opening() {
  new_test_ext().execute_with(|| {
    let (actor, episode, now) = funded_source_service(Source::AtTime);
    let fees = fee_collections().len();
    let sovereign = sovereign_account(actor.actor_id);
    let balance = native_balance(&sovereign);
    System::reset_events();
    assert_ok!(Actors::service_canonical_round_head(
      &mut WeightMeter::with_limit(Weight::MAX),
      now,
    ));
    assert_eq!(
      native_balance(&sovereign),
      balance - episode.record().unpaid_liability()
    );
    assert_eq!(fee_collections().len(), fees + 1);
    assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    let hot = Actors::actor_hot(actor.actor_id).unwrap();
    assert_eq!(hot.cycle_state, CycleState::Idle);
    assert!(hot.pending_signal);
    assert!(matches!(
      hot.trigger_runtime_state,
      TriggerRuntimeState::AtTime { consumed: true, .. }
    ));
    assert!(!ActorRunStateStore::<Test>::contains_key(actor.actor_id));
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
    assert_eq!(
      ActorProcesses::<Test>::get(actor.actor_id)
        .unwrap()
        .residence,
      Some(ProcessResidence::Service(ServiceResidenceKind::Pending))
    );
    assert_eq!(
      crate::ServiceNodes::<Test>::get(actor.actor_id)
        .unwrap()
        .eligible_from,
      now + 1
    );
    assert!(!System::events().iter().any(|event| matches!(
      event.event,
      RuntimeEvent::Actors(Event::CycleStarted { .. } | Event::ActionFeeCharged { .. })
    )));
    assert!(System::events().iter().any(|event| matches!(event.event,
      RuntimeEvent::Actors(Event::FinancialWaitRecovered { receipt })
        if receipt == due_receipt(actor, episode))));
    assert_ok!(Actors::service_canonical_round_head(
      &mut WeightMeter::with_limit(Weight::MAX),
      now,
    ));
    assert_eq!(fee_collections().len(), fees + 1);
  });
}

#[test]
fn financial_wait_runtime_at_time_collector_refusal_restores_original_wait_and_retries_once() {
  new_test_ext().execute_with(|| {
    let (actor, episode, now) = funded_source_service(Source::AtTime);
    set_fail_fee_sink_transfer(true);
    let before = root();
    assert_eq!(
      Actors::service_canonical_round_head(&mut WeightMeter::with_limit(Weight::MAX), now,),
      Err(crate::ServiceRoundError::FeeCollection)
    );
    assert_eq!(root(), before);
    assert_eq!(
      ActorFinancialWaits::<Test>::get(actor.actor_id),
      Some(episode)
    );
    set_fail_fee_sink_transfer(false);
    assert_ok!(Actors::service_canonical_round_head(
      &mut WeightMeter::with_limit(Weight::MAX),
      now,
    ));
    assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    assert!(Actors::actor_hot(actor.actor_id).unwrap().pending_signal);
  });
}

#[test]
fn financial_wait_runtime_at_time_recovery_preserves_immutable_and_zero_step_contracts() {
  for zero_step in [false, true] {
    new_test_ext().execute_with(|| {
      let (actor, unpaid) = prepared_source_with_options(
        Source::AtTime,
        false,
        Mutability::Immutable,
        zero_step,
        false,
      );
      let (actor, episode, now) = funded_service(actor, unpaid);
      let fees = fee_collections().len();
      let before = root();
      assert!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor.actor_id).is_err());
      assert_eq!(root(), before);
      assert_ok!(Actors::service_canonical_round_head(
        &mut WeightMeter::with_limit(Weight::MAX),
        now,
      ));
      assert_eq!(fee_collections().len(), fees + 1);
      assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
      assert!(Actors::actor_hot(actor.actor_id).unwrap().pending_signal);
      assert!(!ActorRunStateStore::<Test>::contains_key(actor.actor_id));
      assert_eq!(
        Actors::actor_identity(actor.actor_id).unwrap().mutability,
        Mutability::Immutable
      );
      let after = root();
      assert!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor.actor_id).is_err());
      assert_eq!(root(), after);
      assert!(matches!(
        episode.record().unpaid_service(),
        UnpaidActorService::AtTimeTrigger { .. }
      ));
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_at_time_recovery_admits_trigger_and_cleanup_in_both_dimensions() {
  for proof in [false, true] {
    new_test_ext().execute_with(|| {
      let (actor, episode, now) = funded_source_service(Source::AtTime);
      let inspection = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
        + <Test as crate::Config>::WeightInfo::service_round_probe_eligible()
        + <Test as crate::Config>::WeightInfo::scheduler_actor_state_probe()
        + MockAdmissionCertificateAuthority::financial_wait_service_weight(FinancialWaitEpisode::<
          Balance,
        >::max_encoded_len(
        ) as u32)
        .unwrap();
      let LoadedActorStateOf::Active(state) = Actors::load_actor_state(actor.actor_id) else {
        panic!("prepared financial source");
      };
      let complete = inspection
        + <Test as crate::Config>::WeightInfo::at_time_trigger_occurrence()
        + Actors::close_control_weight_for_contract(&state.contract).unwrap();
      let limit = if proof {
        Weight::from_parts(complete.ref_time(), complete.proof_size() - 1)
      } else {
        Weight::from_parts(complete.ref_time() - 1, complete.proof_size())
      };
      let before = root();
      let fees = fee_collections();
      let mut meter = WeightMeter::with_limit(limit);
      assert!(Actors::service_canonical_round_head(&mut meter, now).is_err());
      assert_eq!(root(), before);
      assert_eq!(fee_collections(), fees);
      assert_eq!(
        meter.consumed(),
        inspection
          - MockAdmissionCertificateAuthority::financial_wait_service_weight(
            FinancialWaitEpisode::<Balance>::max_encoded_len() as u32,
          )
          .unwrap()
      );
      assert_eq!(
        ActorFinancialWaits::<Test>::get(actor.actor_id),
        Some(episode)
      );
    });
  }
}

#[test]
fn financial_wait_runtime_service_recovers_only_the_unpaid_action_without_replaying_prefix() {
  for source in [Source::Running, Source::Suspended] {
    new_test_ext().execute_with(|| {
      let (actor, episode, now) = funded_source_service(source);
      set_temporary_dex_failure(false);
      let fees = fee_collections();
      let bob_native = native_balance(&BOB);
      let hold = Actors::actor_state_hold(actor.actor_id);
      System::reset_events();
      let mut meter = WeightMeter::with_limit(Weight::MAX);
      assert_ok!(Actors::service_canonical_round_head(&mut meter, now));
      assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
      assert_eq!(
        ActorProcesses::<Test>::get(actor.actor_id).unwrap().status,
        ProcessStatus::Disabled(crate::ProcessDisablement {
          cause: crate::ProcessDisableCause::Protocol,
          revival_authority: crate::ProcessRevivalAuthority::Protocol,
          basis: crate::SuspendedProcessBasis::Idle,
        })
      );
      assert_eq!(
        Actors::actor_identity(actor.actor_id).unwrap().cycle_nonce,
        1
      );
      assert_eq!(
        Actors::actor_hot(actor.actor_id).unwrap().cycle_state,
        CycleState::Idle
      );
      assert!(!ActorRunStateStore::<Test>::contains_key(actor.actor_id));
      assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 7);
      if matches!(source, Source::Running) {
        assert_eq!(native_balance(&BOB), bob_native + 7);
      } else {
        assert!(asset_balance(&sovereign_account(actor.actor_id), TestAsset::Local(77)) > 0);
      }
      assert_eq!(fee_collections().len(), fees.len() + 1);
      assert_eq!(Actors::actor_state_hold(actor.actor_id), hold);
      assert!(!has_actor_event(|event| matches!(
        event,
        Event::CycleStarted { .. }
      )));
      assert!(has_actor_event(
        |event| matches!(event, Event::FinancialWaitRecovered { receipt }
        if *receipt == due_receipt(actor, episode))
      ));
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_service_actual_retry_failure_retires_the_paid_boundary() {
  new_test_ext().execute_with(|| {
    let (actor, episode, now) = funded_source_service(Source::Suspended);
    let run = ActorRunStateStore::<Test>::get(actor.actor_id).unwrap();
    let fees = fee_collections();
    assert_ok!(Actors::service_canonical_round_head(
      &mut WeightMeter::with_limit(Weight::MAX),
      now
    ));
    assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    let successor = ActorRunStateStore::<Test>::get(actor.actor_id).unwrap();
    assert_eq!(successor.cycle_nonce, run.cycle_nonce);
    assert_eq!(successor.cursor, run.cursor);
    assert_eq!(
      successor.unsuccessful_attempts_at_cursor,
      run.unsuccessful_attempts_at_cursor + 1
    );
    assert_eq!(
      ActorProcesses::<Test>::get(actor.actor_id).unwrap().status,
      ProcessStatus::Serving
    );
    assert_eq!(fee_collections().len(), fees.len() + 1);
    assert!(has_actor_event(
      |event| matches!(event, Event::FinancialWaitRecovered { receipt }
      if *receipt == due_receipt(actor, episode))
    ));
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 7);
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn financial_wait_runtime_service_late_refusals_restore_episode_effects_and_paid_prefix() {
  for (collector, cadenced) in [(false, false), (true, false), (false, true), (true, true)] {
    new_test_ext().execute_with(|| {
      let (actor, episode, now) = if cadenced {
        let (actor, unpaid) = prepared_source_with_funding(
          Source::Running,
          true,
          Mutability::Mutable,
          false,
          false,
          None,
          None,
          true,
        );
        funded_service(actor, unpaid)
      } else {
        funded_source_service(Source::Running)
      };
      if collector {
        set_fail_fee_sink_transfer(true);
      } else {
        set_step_control_actual_weight_override(Some(Weight::MAX));
      }
      let before = root();
      let native = native_balance(&sovereign_account(actor.actor_id));
      let bob = native_balance(&BOB);
      let fees = fee_collections();
      let run = ActorRunStateStore::<Test>::get(actor.actor_id)
        .unwrap()
        .encode();
      let mut meter = WeightMeter::with_limit(Weight::MAX);
      assert!(Actors::service_canonical_round_head(&mut meter, now).is_err());
      assert_eq!(root(), before);
      assert_eq!(native_balance(&sovereign_account(actor.actor_id)), native);
      assert_eq!(native_balance(&BOB), bob);
      assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 7);
      if collector {
        // The thread-local mock log records attempted collection, including rejected calls.
        let observed = fee_collections();
        assert_eq!(&observed[..fees.len()], fees.as_slice());
        assert_eq!(observed.len(), fees.len() + 1);
      } else {
        assert_eq!(fee_collections(), fees);
      }
      assert_eq!(
        ActorRunStateStore::<Test>::get(actor.actor_id)
          .unwrap()
          .encode(),
        run
      );
      assert_eq!(
        ActorFinancialWaits::<Test>::get(actor.actor_id),
        Some(episode)
      );
      assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
        actor.actor_id
      ));
      assert!(meter.consumed().ref_time() > 300);
      set_fail_fee_sink_transfer(false);
      set_step_control_actual_weight_override(None);
      assert_ok!(Actors::service_canonical_round_head(
        &mut WeightMeter::with_limit(Weight::MAX),
        now
      ));
      assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
      assert_eq!(native_balance(&BOB), bob + 7);
      if cadenced {
        assert_eq!(
          crate::TriggerDeadlineHandles::<Test>::get(actor.actor_id)
            .unwrap()
            .key,
          WakeupKey::Tick(now + 1)
        );
      }
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_service_funds_lost_after_due_restore_close_without_an_action() {
  for source in [Source::AtTime, Source::Running, Source::Suspended] {
    new_test_ext().execute_with(|| {
      let (actor, episode, now) = funded_source_service(source);
      let sovereign = sovereign_account(actor.actor_id);
      set_native_transfer_lock(&sovereign, native_balance(&sovereign));
      let native = native_balance(&sovereign);
      let bob = native_balance(&BOB);
      let fees = fee_collections();
      System::reset_events();
      assert_ok!(Actors::service_canonical_round_head(
        &mut WeightMeter::with_limit(Weight::MAX),
        now
      ));
      assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
      assert!(Actors::actor_identity(actor.actor_id).is_none());
      assert_eq!(native_balance(&sovereign), native);
      assert_eq!(native_balance(&BOB), bob);
      assert_eq!(
        asset_balance(&BOB, TestAsset::Local(1)),
        if matches!(source, Source::AtTime) {
          0
        } else {
          7
        }
      );
      assert_eq!(fee_collections(), fees);
      assert!(!has_actor_event(|event| matches!(
        event,
        Event::FinancialWaitRecovered { .. }
      )));
      assert!(has_actor_event(
        |event| matches!(event, Event::ActorClosed {
      actor_id, reason: CloseReason::FundingGraceExpired } if *actor_id == actor.actor_id)
      ));
      assert_eq!(episode.record().expiry_tick(), now - 1);
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_repark_reauthenticates_source_inside_atomic_publication() {
  for source in [
    Source::AtTime,
    Source::Opening,
    Source::Running,
    Source::Suspended,
  ] {
    for rollback in [false, true] {
      let mut ext = new_test_ext();
      let (actor, episode, process, semantic, head, run, before, keys, fees, events) = ext
        .execute_with(|| {
          let (actor, episode) = hint_source(source);
          set_native_transfer_lock(&sovereign_account(actor.actor_id), 0);
          assert_ok!(deliver_hint(
            &hint_event(
              actor,
              ALICE,
              TestAsset::Native,
              episode.record().unpaid_liability() + TestMinUserBalance::get(),
            ),
            &mut WeightMeter::with_limit(Weight::MAX),
          ));
          assert_eq!(crate::ServiceHeader::<Test>::get().count, 1);
          assert!(!DeadlineHandles::<Test>::contains_key(actor.actor_id));
          (
            actor,
            episode,
            ActorProcesses::<Test>::get(actor.actor_id).unwrap(),
            ActorSemanticStates::<Test>::get(actor.actor_id).unwrap(),
            crate::ActorContractHeads::<Test>::get(actor.actor_id).unwrap(),
            ActorRunStateStore::<Test>::get(actor.actor_id),
            root(),
            [
              ActorSemanticStates::<Test>::hashed_key_for(actor.actor_id),
              crate::ActorContractHeads::<Test>::hashed_key_for(actor.actor_id),
              ActorFinancialWaits::<Test>::hashed_key_for(actor.actor_id),
              ActorRunStateStore::<Test>::hashed_key_for(actor.actor_id),
            ],
            fee_collections(),
            System::event_count(),
          )
        });
      ext.commit_all().unwrap();
      let recorder =
        polkadot_sdk::sp_trie::recorder::Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
      ext.execute_with_recorder(recorder.clone(), || {
        polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
          assert_ok!(Actors::test_repark_financial_service(actor, &episode));
          if rollback {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
          } else {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
          }
        });
      });
      // Observe only the actual publication call; setup and verification are outside the recorder.
      for key in keys {
        assert!(
          recorder
            .recorded_keys()
            .values()
            .any(|keys| keys.keys().any(|read| read.as_ref() == key.as_slice()))
        );
      }
      ext.execute_with(|| {
        assert_eq!(
          ActorFinancialWaits::<Test>::get(actor.actor_id),
          Some(episode)
        );
        assert_eq!(
          ActorSemanticStates::<Test>::get(actor.actor_id),
          Some(semantic)
        );
        assert_eq!(
          crate::ActorContractHeads::<Test>::get(actor.actor_id),
          Some(head)
        );
        assert_eq!(ActorRunStateStore::<Test>::get(actor.actor_id), run);
        assert_eq!(fee_collections(), fees);
        assert_eq!(System::event_count(), events);
        if rollback {
          assert_eq!(ActorProcesses::<Test>::get(actor.actor_id), Some(process));
          assert!(crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
          assert!(!DeadlineHandles::<Test>::contains_key(actor.actor_id));
          assert_eq!(root(), before);
        } else {
          let handle = DeadlineHandles::<Test>::get(actor.actor_id).unwrap();
          assert_eq!(
            (handle.key, handle.page, handle.slot),
            (WakeupKey::Tick(episode.record().expiry_tick()), 0, 0)
          );
          let mut expected = process;
          expected.residence = Some(ProcessResidence::Deadline {
            key: handle.key,
            page: handle.page,
            slot: handle.slot,
          });
          assert_eq!(ActorProcesses::<Test>::get(actor.actor_id), Some(expected));
          assert!(!crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
          assert_eq!(crate::ServiceHeader::<Test>::get().count, 0);
        }
        #[cfg(feature = "try-runtime")]
        assert_ok!(Actors::do_try_state());
      });
    }
  }
}

#[test]
fn financial_wait_runtime_stale_repark_authenticates_complete_retained_body() {
  for source in [Source::Running, Source::Suspended] {
    // Current authority reads the cursor; stale authority authenticates every retained tail.
    // A missing cold tail is corruption, not an admitted predecessor.
    for mode in 0..3 {
      for rollback in [false, true] {
        if mode == 2 && rollback {
          continue;
        }
        let mut ext = new_test_ext();
        let (
          actor,
          episode,
          process,
          semantic,
          head,
          run,
          tails,
          before,
          custody,
          fees,
          backing,
          events,
        ) = ext.execute_with(|| {
          let (actor, unpaid) = prepared_source_with_length(
            source,
            false,
            Mutability::Mutable,
            false,
            false,
            None,
            None,
            false,
            12,
          );
          let episode = Actors::publish_prepared_financial_wait(
            actor,
            unpaid,
            &mut WeightMeter::with_limit(Weight::MAX),
          )
          .unwrap();
          set_prepared_financial_wait_ingress_weight(true);
          set_native_transfer_lock(&sovereign_account(actor.actor_id), 0);
          assert_ok!(deliver_hint(
            &hint_event(
              actor,
              ALICE,
              TestAsset::Native,
              episode.record().unpaid_liability() + TestMinUserBalance::get()
            ),
            &mut WeightMeter::with_limit(Weight::MAX),
          ));
          let head = crate::ActorContractHeads::<Test>::get(actor.actor_id).unwrap();
          assert_eq!(head.header.step_count, 12);
          let tails = (0..3)
            .map(|chunk| {
              crate::ActorContractTailChunks::<Test>::get(actor.actor_id, chunk).unwrap()
            })
            .collect::<Vec<_>>();
          let run = ActorRunStateStore::<Test>::get(actor.actor_id).unwrap();
          assert_eq!(run.cursor, 1);
          assert_eq!(run.cumulative_outcomes.committed_effectful_tasks, 1);
          assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 7);
          let sovereign = sovereign_account(actor.actor_id);
          if mode != 0 {
            let mut authority = MockAdmissionCertificateAuthority::current().unwrap();
            authority.runtime_actor_semantics_version += 1;
            set_admission_authority_override(Some(authority));
          }
          if mode == 2 {
            crate::ActorContractTailChunks::<Test>::remove(actor.actor_id, 2);
          }
          (
            actor,
            episode,
            ActorProcesses::<Test>::get(actor.actor_id).unwrap(),
            ActorSemanticStates::<Test>::get(actor.actor_id).unwrap(),
            head,
            run,
            tails,
            root(),
            (
              native_balance(&sovereign),
              asset_balance(&sovereign, TestAsset::Local(1)),
              asset_balance(&BOB, TestAsset::Local(1)),
            ),
            fee_collections(),
            financial_return_backings(),
            System::event_count(),
          )
        });
        ext.commit_all().unwrap();
        let recorder = polkadot_sdk::sp_trie::recorder::Recorder::<
          polkadot_sdk::sp_core::Blake2Hasher,
        >::default();
        ext.execute_with_recorder(recorder.clone(), || {
          polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
            let result = Actors::test_repark_financial_service(actor, &episode);
            if mode == 2 {
              assert!(matches!(
                result,
                Err(crate::FinancialWaitDueError::Service(_))
              ));
              polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
            } else {
              assert_ok!(result);
              if rollback {
                polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
              } else {
                polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
              }
            }
          });
        });
        // Recorder reads belong solely to the suffix, including the absent final-tail lookup.
        for chunk in 0..3 {
          let key = crate::ActorContractTailChunks::<Test>::hashed_key_for(actor.actor_id, chunk);
          let read = recorder.recorded_keys().values().any(|keys| {
            keys
              .keys()
              .any(|recorded| recorded.as_ref() == key.as_slice())
          });
          assert_eq!(read, chunk == 0 || mode != 0);
        }
        ext.execute_with(|| {
          let ActorSemanticState::Active(record) = &semantic else {
            panic!("retained source is active");
          };
          let sovereign = record.identity.sovereign_account;
          assert_eq!(
            ActorFinancialWaits::<Test>::get(actor.actor_id),
            Some(episode)
          );
          assert_eq!(
            ActorSemanticStates::<Test>::get(actor.actor_id),
            Some(semantic)
          );
          assert_eq!(
            crate::ActorContractHeads::<Test>::get(actor.actor_id),
            Some(head)
          );
          assert_eq!(ActorRunStateStore::<Test>::get(actor.actor_id), Some(run));
          assert_eq!(
            (
              native_balance(&sovereign),
              asset_balance(&sovereign, TestAsset::Local(1)),
              asset_balance(&BOB, TestAsset::Local(1))
            ),
            custody
          );
          assert_eq!(fee_collections(), fees);
          assert_eq!(financial_return_backings(), backing);
          assert_eq!(System::event_count(), events);
          if rollback || mode == 2 {
            assert_eq!(ActorProcesses::<Test>::get(actor.actor_id), Some(process));
            assert!(crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
            assert!(!DeadlineHandles::<Test>::contains_key(actor.actor_id));
            assert_eq!(root(), before);
          } else {
            let handle = DeadlineHandles::<Test>::get(actor.actor_id).unwrap();
            assert_eq!(handle.key, WakeupKey::Tick(episode.record().expiry_tick()));
            let mut expected = process;
            expected.residence = Some(ProcessResidence::Deadline {
              key: handle.key,
              page: handle.page,
              slot: handle.slot,
            });
            assert_eq!(ActorProcesses::<Test>::get(actor.actor_id), Some(expected));
            assert!(!crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
            assert_eq!(crate::ServiceHeader::<Test>::get().count, 0);
          }
          for (chunk, tail) in tails.into_iter().enumerate() {
            if mode == 2 && chunk == 2 {
              assert!(!crate::ActorContractTailChunks::<Test>::contains_key(
                actor.actor_id,
                2
              ));
              // Repair diagnostic corruption before checking the complete canonical state.
              crate::ActorContractTailChunks::<Test>::insert(actor.actor_id, 2, tail);
            } else {
              assert_eq!(
                crate::ActorContractTailChunks::<Test>::get(actor.actor_id, chunk as u32),
                Some(tail)
              );
            }
          }
          set_admission_authority_override(None);
          #[cfg(feature = "try-runtime")]
          assert_ok!(Actors::do_try_state());
        });
      }
    }
  }
}

#[test]
fn financial_wait_runtime_service_reparks_before_grace_without_renewing_the_episode() {
  for source in [
    Source::AtTime,
    Source::Opening,
    Source::Running,
    Source::Suspended,
  ] {
    new_test_ext().execute_with(|| {
      let (actor, episode) = hint_source(source);
      set_financial_return_backing(TestFinancialReturnBacking::SignedProducer);
      let backing_fee =
        Actors::financial_wait_return_fee(&Actors::load_actor_contract(actor.actor_id).unwrap())
          .unwrap();
      let sovereign = sovereign_account(actor.actor_id);
      let run = ActorRunStateStore::<Test>::get(actor.actor_id);
      let hot = Actors::actor_hot(actor.actor_id).unwrap();
      let fees = fee_collections();
      let bob = asset_balance(&BOB, TestAsset::Local(1));
      let ingress = Actors::financial_wait_ingress_weight_upper().unwrap();
      let inspection = TestWeightInfo::service_round_begin_populated()
        + TestWeightInfo::service_round_probe_eligible()
        + TestWeightInfo::scheduler_actor_state_probe()
        + MockAdmissionCertificateAuthority::financial_wait_service_weight(FinancialWaitEpisode::<
          Balance,
        >::max_encoded_len(
        ) as u32)
        .unwrap();
      for visit in 0..2 {
        let now = System::block_number() + 1;
        System::set_block_number(now);
        set_native_transfer_lock(&sovereign, 0);
        let amount = if visit == 0 {
          episode.record().unpaid_liability() + TestMinUserBalance::get() + 1_000
        } else {
          1
        };
        let mut producer = WeightMeter::with_limit(ingress + ingress);
        let payer = native_balance(&ALICE);
        let sink = native_balance(&TestFeeSink::get());
        let custody = native_balance(&sovereign);
        assert_ok!(deliver_hint(
          &hint_event(actor, ALICE, TestAsset::Native, amount),
          &mut producer,
        ));
        assert_eq!(native_balance(&ALICE), payer - amount - backing_fee);
        assert_eq!(native_balance(&TestFeeSink::get()), sink + backing_fee);
        assert_eq!(native_balance(&sovereign), custody + amount);
        assert_eq!(
          financial_return_backings(),
          vec![(due_receipt(actor, episode), backing_fee); visit + 1]
        );
        let node = crate::ServiceNodes::<Test>::get(actor.actor_id).unwrap();
        assert_eq!(node.eligible_from, now + 1);
        assert_eq!(
          node.kind,
          if matches!(source, Source::Opening) {
            ServiceResidenceKind::Pending
          } else {
            ServiceResidenceKind::Live
          }
        );
        assert_ok!(deliver_hint(
          &hint_event(actor, ALICE, TestAsset::Native, 1),
          &mut producer,
        ));
        assert_eq!(producer.consumed(), ingress + ingress);
        assert_eq!(native_balance(&ALICE), payer - amount - backing_fee - 1);
        assert_eq!(native_balance(&TestFeeSink::get()), sink + backing_fee);
        assert_eq!(financial_return_backings().len(), visit + 1);
        assert_eq!(crate::ServiceNodes::<Test>::get(actor.actor_id), Some(node));
        set_native_transfer_lock(&sovereign, native_balance(&sovereign));
        System::set_block_number(now + 1);
        System::reset_events();
        let mut service = WeightMeter::with_limit(Weight::MAX);
        assert_ok!(Actors::service_canonical_round_head(&mut service, now + 1));
        assert_eq!(service.consumed(), inspection);
        assert_eq!(
          ActorFinancialWaits::<Test>::get(actor.actor_id),
          Some(episode)
        );
        assert_eq!(
          DeadlineHandles::<Test>::get(actor.actor_id).unwrap().key,
          WakeupKey::Tick(episode.record().expiry_tick())
        );
        assert!(!crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
        assert_eq!(ActorRunStateStore::<Test>::get(actor.actor_id), run);
        assert_eq!(Actors::actor_hot(actor.actor_id), Some(hot.clone()));
        assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), bob);
        assert_eq!(fee_collections(), fees);
        assert!(!has_actor_event(|event| matches!(
          event,
          Event::FinancialWaitEntered { .. }
            | Event::FinancialWaitRecovered { .. }
            | Event::TriggerOccurrenceProcessed { .. }
            | Event::CycleStarted { .. }
            | Event::StepSkipped { .. }
            | Event::StepFailed { .. }
            | Event::ActionFeeCharged { .. }
            | Event::TransferExecuted { .. }
        )));
        #[cfg(feature = "try-runtime")]
        assert_ok!(Actors::do_try_state());
      }
      set_native_transfer_lock(&sovereign, 0);
      let now = System::block_number();
      assert_ok!(deliver_hint(
        &hint_event(actor, ALICE, TestAsset::Native, 1),
        &mut WeightMeter::with_limit(ingress),
      ));
      assert_eq!(
        financial_return_backings(),
        vec![(due_receipt(actor, episode), backing_fee); 3]
      );
      System::set_block_number(now + 1);
      assert_ok!(Actors::service_canonical_round_head(
        &mut WeightMeter::with_limit(Weight::MAX),
        now + 1,
      ));
      assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
      assert!(has_actor_event(|event| matches!(event,
        Event::FinancialWaitRecovered { receipt } if *receipt == due_receipt(actor, episode))));
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_retained_capacity_refusals_preserve_return_backing() {
  new_test_ext().execute_with(|| {
    let (actor, episode) = hint_source(Source::AtTime);
    set_financial_return_backing(TestFinancialReturnBacking::SignedProducer);
    assert_ok!(deliver_hint(
      &hint_event(
        actor,
        ALICE,
        TestAsset::Native,
        episode.record().unpaid_liability() + TestMinUserBalance::get(),
      ),
      &mut WeightMeter::with_limit(Weight::MAX),
    ));
    let contract = Actors::load_actor_contract(actor.actor_id).unwrap();
    let inspection = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      + <Test as crate::Config>::WeightInfo::service_round_probe_eligible()
      + <Test as crate::Config>::WeightInfo::scheduler_actor_state_probe()
      + MockAdmissionCertificateAuthority::financial_wait_service_weight(FinancialWaitEpisode::<
        Balance,
      >::max_encoded_len()
        as u32)
      .unwrap();
    let common = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      + <Test as crate::Config>::WeightInfo::service_round_probe_eligible()
      + <Test as crate::Config>::WeightInfo::scheduler_actor_state_probe();
    assert!(inspection.ref_time() > 0);
    assert_eq!(
      financial_return_backings(),
      vec![(
        due_receipt(actor, episode),
        Actors::financial_wait_return_fee(&contract).unwrap()
      )]
    );
    let backing = financial_return_backings();
    let node = crate::ServiceNodes::<Test>::get(actor.actor_id).unwrap();
    let balances = (
      native_balance(&ALICE),
      native_balance(&TestFeeSink::get()),
      native_balance(&sovereign_account(actor.actor_id)),
    );
    let fees = fee_collections();
    for now in [
      node.eligible_from,
      node.eligible_from + 1,
      node.eligible_from + 2,
      episode.record().expiry_tick(),
      episode.record().expiry_tick() + 1,
    ] {
      System::set_block_number(now);
      let before = root();
      let mut meter = WeightMeter::with_limit(inspection);
      assert_eq!(
        Actors::service_canonical_round_head(&mut meter, now),
        Err(crate::ServiceRoundError::InsufficientWeight)
      );
      assert_eq!(meter.consumed(), common);
      assert_eq!(root(), before);
      assert_eq!(
        ActorFinancialWaits::<Test>::get(actor.actor_id),
        Some(episode)
      );
      assert_eq!(crate::ServiceNodes::<Test>::get(actor.actor_id), Some(node));
      assert_eq!(financial_return_backings(), backing);
      assert_eq!(fee_collections(), fees);
      assert_eq!(
        (
          native_balance(&ALICE),
          native_balance(&TestFeeSink::get()),
          native_balance(&sovereign_account(actor.actor_id))
        ),
        balances
      );
    }
    // Solvency preserves the owed service after grace. Capacity refusal now happens before the
    // additional financial inspection, so these visits cannot spend its one-return allowance.
    // Common Service discovery/classification still need their ordinary payment qualification.
  });
}

#[test]
fn financial_wait_runtime_capacity_refusal_precedes_cold_financial_inspection() {
  for source in [
    Source::AtTime,
    Source::Opening,
    Source::Running,
    Source::Suspended,
  ] {
    let mut ext = new_test_ext();
    let (actor, now, before, limit, common) = ext.execute_with(|| {
      let (actor, _, now) = funded_source_service(source);
      let common = TestWeightInfo::service_round_begin_populated()
        + TestWeightInfo::service_round_probe_eligible()
        + TestWeightInfo::scheduler_actor_state_probe();
      let financial =
        MockAdmissionCertificateAuthority::financial_wait_service_weight(FinancialWaitEpisode::<
          Balance,
        >::max_encoded_len(
        ) as u32)
        .unwrap();
      (actor, now, root(), common + financial, common)
    });
    ext.commit_all().unwrap();
    let wait_key = ActorFinancialWaits::<Test>::hashed_key_for(actor.actor_id);
    let recorder =
      polkadot_sdk::sp_trie::recorder::Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
    ext.execute_with_recorder(recorder.clone(), || {
      let mut meter = WeightMeter::with_limit(limit);
      assert_eq!(
        Actors::service_canonical_round_head(&mut meter, now),
        Err(crate::ServiceRoundError::InsufficientWeight),
      );
      assert_eq!(meter.consumed(), common, "source={source:?}");
    });
    assert!(
      !recorder
        .recorded_keys()
        .values()
        .any(|keys| { keys.keys().any(|key| key.as_ref() == wait_key.as_slice()) }),
      "source={source:?}: a capacity refusal must precede the optional financial read",
    );
    ext.execute_with(|| assert_eq!(root(), before));
  }
}

#[test]
fn financial_wait_runtime_admitted_collector_refusals_need_separate_backing_qualification() {
  new_test_ext().execute_with(|| {
    let (actor, episode) = hint_source(Source::AtTime);
    set_financial_return_backing(TestFinancialReturnBacking::SignedProducer);
    assert_ok!(deliver_hint(
      &hint_event(
        actor,
        ALICE,
        TestAsset::Native,
        episode.record().unpaid_liability() + TestMinUserBalance::get()
      ),
      &mut WeightMeter::with_limit(Weight::MAX),
    ));
    let node = crate::ServiceNodes::<Test>::get(actor.actor_id).unwrap();
    let backing = financial_return_backings();
    assert_eq!(backing.len(), 1);
    let mut attempted_collections = fee_collections();
    let inspection = TestWeightInfo::service_round_begin_populated()
      + TestWeightInfo::service_round_probe_eligible()
      + TestWeightInfo::scheduler_actor_state_probe()
      + MockAdmissionCertificateAuthority::financial_wait_service_weight(FinancialWaitEpisode::<
        Balance,
      >::max_encoded_len()
        as u32)
      .unwrap();
    set_fail_fee_sink_transfer(true);
    for now in [
      node.eligible_from,
      node.eligible_from + 1,
      episode.record().expiry_tick(),
      episode.record().expiry_tick() + 1,
    ] {
      System::set_block_number(now);
      let before = root();
      let mut meter = WeightMeter::with_limit(Weight::MAX);
      assert_eq!(
        Actors::service_canonical_round_head(&mut meter, now),
        Err(crate::ServiceRoundError::FeeCollection)
      );
      assert!(meter.consumed().all_gte(inspection));
      assert_eq!(root(), before);
      assert_eq!(crate::ServiceNodes::<Test>::get(actor.actor_id), Some(node));
      assert_eq!(
        ActorFinancialWaits::<Test>::get(actor.actor_id),
        Some(episode)
      );
      assert_eq!(financial_return_backings(), backing);
      // The native mock logs invocation outside consensus storage; the root proves no debit.
      attempted_collections.push(episode.record().unpaid_liability());
      assert_eq!(fee_collections(), attempted_collections);
    }
    // Early capacity admission cannot bound fully admitted, noncommitting collector work.
    // One return payment is not a qualification for repeated post-admission failure.
  });
}

#[test]
fn financial_wait_runtime_resource_refusal_precedes_cold_financial_inspection() {
  for source in [Source::AtTime, Source::Running] {
    for drain in [false, true] {
      for effect_refusal in [false, true] {
        if effect_refusal && matches!(source, Source::AtTime) {
          continue;
        }
        for proof in [false, true] {
          let mut ext = new_test_ext();
          let (actor, now, before) = ext.execute_with(|| {
            let (actor, _, now) = funded_source_service(source);
            (actor, now, root())
          });
          ext.commit_all().unwrap();
          let recorder = polkadot_sdk::sp_trie::recorder::Recorder::<
            polkadot_sdk::sp_core::Blake2Hasher,
          >::default();
          ext.execute_with_recorder(recorder.clone(), || {
            let budget = crate::BlockResourceBudget::new_with_control_ratio(
              Weight::from_parts(2_000_000_000_000, 10_485_760),
              Weight::zero(),
              1,
              3,
            )
            .unwrap();
            let limits = budget.limits();
            let mut ledger = crate::BlockResourceState::new(now);
            assert_ok!(ledger.begin_prepass(budget));
            let effect_domain = if drain {
              assert_ok!(ledger.open_external_phase());
              assert_ok!(ledger.begin_drain());
              crate::BlockResourceDomain::ActorDrainEffect
            } else {
              crate::BlockResourceDomain::ActorBaseEffect
            };
            let common = TestWeightInfo::service_round_begin_populated()
              + TestWeightInfo::service_round_probe_eligible()
              + TestWeightInfo::scheduler_actor_state_probe();
            let financial = MockAdmissionCertificateAuthority::financial_wait_service_weight(
              FinancialWaitEpisode::<Balance>::max_encoded_len() as u32,
            )
            .unwrap();
            let (domain, ceiling, remaining) = if effect_refusal {
              (
                effect_domain,
                if drain {
                  limits.shared_economic()
                } else {
                  limits.actor_base_turn()
                },
                if proof {
                  Weight::from_parts(33, 43)
                } else {
                  Weight::from_parts(32, 44)
                },
              )
            } else {
              (
                crate::BlockResourceDomain::ActorControl,
                limits.actor_control(),
                common + financial,
              )
            };
            let prefill = if proof {
              Weight::from_parts(0, ceiling.proof_size() - remaining.proof_size())
            } else {
              Weight::from_parts(ceiling.ref_time() - remaining.ref_time(), 0)
            };
            let mut reservation = ledger.reserve(limits, domain, prefill).unwrap();
            assert_ok!(ledger.settle(&mut reservation, prefill));
            let usage = ledger.usage();
            let mut meter = WeightMeter::with_limit(Weight::MAX);
            assert_eq!(
              Actors::service_canonical_round_head_with_resources(
                &mut meter,
                now,
                &mut ledger,
                limits,
                effect_domain,
              ),
              Err(crate::ServiceRoundError::ResourceUnavailable)
            );
            assert_eq!(meter.consumed(), common);
            assert_eq!(ledger.outstanding_reservations(), 0);
            assert_eq!(
              ledger.usage().actor_control_used(),
              usage.actor_control_used() + common
            );
            assert_eq!(
              ledger.usage().actor_effect_used(),
              usage.actor_effect_used()
            );
            assert_eq!(root(), before);
          });
          let key = ActorFinancialWaits::<Test>::hashed_key_for(actor.actor_id);
          assert!(
            !recorder
              .recorded_keys()
              .values()
              .any(|keys| { keys.keys().any(|read| read.as_ref() == key.as_slice()) })
          );
        }
      }
    }
  }
}

#[test]
fn financial_wait_runtime_control_only_recovery_does_not_require_effect_capacity() {
  for (source, zero_step, policy_stop) in [
    (Source::AtTime, false, false),
    (Source::Opening, true, false),
    (Source::Opening, false, true),
  ] {
    for drain in [false, true] {
      new_test_ext().execute_with(|| {
        let (actor, unpaid) = if policy_stop {
          let actor = prepared_joint_source_with_contract(
            vec![make_step(Task::StopCycle)].try_into().unwrap(),
            super::behavior_policy::expression(vec![vec![crate::BehaviorAtom {
              metric: crate::BehaviorMetric::CycleCompletedTotal,
              threshold: 2,
            }]]),
            false,
          );
          let sovereign = sovereign_account(actor.actor_id);
          set_native_transfer_lock(&sovereign, native_balance(&sovereign));
          let cycle_nonce = Actors::actor_identity(actor.actor_id).unwrap().cycle_nonce + 1;
          (actor, UnpaidActorService::PipelineOpening { cycle_nonce })
        } else {
          prepared_source_with_options(source, false, Mutability::Mutable, zero_step, false)
        };
        let (actor, _, now) = funded_service(actor, unpaid);
        let budget = crate::BlockResourceBudget::new_with_control_ratio(
          Weight::from_parts(2_000_000_000_000, 10_485_760),
          Weight::zero(),
          1,
          3,
        )
        .unwrap();
        let limits = budget.limits();
        let mut ledger = crate::BlockResourceState::new(now);
        assert_ok!(ledger.begin_prepass(budget));
        let domain = if drain {
          assert_ok!(ledger.open_external_phase());
          assert_ok!(ledger.begin_drain());
          crate::BlockResourceDomain::ActorDrainEffect
        } else {
          crate::BlockResourceDomain::ActorBaseEffect
        };
        let all_effect = if drain {
          limits.shared_economic()
        } else {
          limits.actor_base_turn()
        };
        let mut effect = ledger.reserve(limits, domain, all_effect).unwrap();
        assert_ok!(ledger.settle(&mut effect, all_effect));
        let mut meter = WeightMeter::with_limit(Weight::MAX);
        assert_ok!(Actors::service_canonical_round_head_with_resources(
          &mut meter,
          now,
          &mut ledger,
          limits,
          domain,
        ));
        assert_eq!(ledger.usage().actor_effect_used(), all_effect);
        assert_eq!(ledger.outstanding_reservations(), 0);
        assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
      });
    }
  }
}

#[test]
fn financial_wait_runtime_service_inspection_requires_both_dimensions_and_host_qualification() {
  for refusal in 0..3 {
    new_test_ext().execute_with(|| {
      let (actor, episode, now) = funded_source_service(Source::Running);
      let base = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
        + <Test as crate::Config>::WeightInfo::service_round_probe_eligible()
        + <Test as crate::Config>::WeightInfo::scheduler_actor_state_probe();
      let additional =
        MockAdmissionCertificateAuthority::financial_wait_service_weight(FinancialWaitEpisode::<
          Balance,
        >::max_encoded_len(
        ) as u32)
        .unwrap();
      let complete = base + additional;
      let limit = match refusal {
        0 => {
          set_prepared_financial_wait_service_weight(false);
          Weight::MAX
        }
        1 => Weight::from_parts(complete.ref_time() - 1, complete.proof_size()),
        _ => Weight::from_parts(complete.ref_time(), complete.proof_size() - 1),
      };
      let before = root();
      let fees = fee_collections();
      let mut meter = WeightMeter::with_limit(limit);
      assert!(Actors::service_canonical_round_head(&mut meter, now).is_err());
      assert_eq!(root(), before);
      assert_eq!(fee_collections(), fees);
      assert_eq!(meter.consumed(), base);
      assert_eq!(
        ActorFinancialWaits::<Test>::get(actor.actor_id),
        Some(episode)
      );
    });
  }
}

#[test]
fn financial_wait_runtime_policy_zero_step_qualifies_before_optional_reads() {
  let mut ext = new_test_ext();
  let (actor, now, before) = ext.execute_with(|| {
    let actor = prepared_joint_source_with_contract(
      crate::ContractSteps::<Test>::default(),
      super::behavior_policy::expression(vec![vec![crate::BehaviorAtom {
        metric: crate::BehaviorMetric::CycleCompletedTotal,
        threshold: 2,
      }]]),
      false,
    );
    let sovereign = sovereign_account(actor.actor_id);
    set_native_transfer_lock(&sovereign, native_balance(&sovereign));
    let nonce = Actors::actor_identity(actor.actor_id).unwrap().cycle_nonce + 1;
    let (actor, _, now) = funded_service(
      actor,
      UnpaidActorService::PipelineOpening { cycle_nonce: nonce },
    );
    (actor, now, root())
  });
  ext.commit_all().unwrap();
  let recorder =
    polkadot_sdk::sp_trie::recorder::Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
  ext.execute_with_recorder(recorder.clone(), || {
    set_missing_behavior_control_weight(true);
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    assert_eq!(
      Actors::service_canonical_round_head(&mut meter, now),
      Err(crate::ServiceRoundError::ResourceUnavailable)
    );
    assert_eq!(
      meter.consumed(),
      TestWeightInfo::service_round_begin_populated()
        + TestWeightInfo::service_round_probe_eligible()
        + TestWeightInfo::scheduler_actor_state_probe()
    );
    assert_eq!(root(), before);
  });
  for key in [
    ActorFinancialWaits::<Test>::hashed_key_for(actor.actor_id),
    crate::ActorBehaviorPolicies::<Test>::hashed_key_for(actor.actor_id),
    crate::ActorBehaviorMonitors::<Test>::hashed_key_for(actor.actor_id),
  ] {
    assert!(
      !recorder
        .recorded_keys()
        .values()
        .any(|keys| { keys.keys().any(|read| read.as_ref() == key.as_slice()) })
    );
  }
  ext.execute_with(|| {
    set_missing_behavior_control_weight(false);
    assert_ok!(Actors::service_canonical_round_head(
      &mut WeightMeter::with_limit(Weight::MAX),
      now,
    ));
    assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    assert_eq!(
      Actors::actor_identity(actor.actor_id).unwrap().cycle_nonce,
      1
    );
    assert!(!has_actor_event(|event| matches!(
      event,
      Event::ActionFeeCharged { .. }
    )));
  });
}

#[test]
fn financial_wait_runtime_caller_control_reservation_is_not_booked_twice() {
  for source in [Source::AtTime, Source::Running] {
    for refused in [false, true] {
      new_test_ext().execute_with(|| {
        let (actor, _, now) = funded_source_service(source);
        let budget = crate::BlockResourceBudget::new_with_control_ratio(
          Weight::from_parts(2_000_000_000_000, 10_485_760),
          Weight::zero(),
          1,
          3,
        )
        .unwrap();
        let limits = budget.limits();
        let mut ledger = crate::BlockResourceState::new(now);
        assert_ok!(ledger.begin_prepass(budget));
        let mut outer = ledger
          .reserve(
            limits,
            crate::BlockResourceDomain::ActorControl,
            limits.actor_control(),
          )
          .unwrap();
        let common = TestWeightInfo::service_round_begin_populated()
          + TestWeightInfo::service_round_probe_eligible()
          + TestWeightInfo::scheduler_actor_state_probe();
        let remaining = if refused {
          common
            + MockAdmissionCertificateAuthority::financial_wait_service_weight(
              FinancialWaitEpisode::<Balance>::max_encoded_len() as u32,
            )
            .unwrap()
        } else {
          limits.actor_control()
        };
        let before = root();
        let mut meter = WeightMeter::with_limit(Weight::MAX);
        let result = Actors::service_canonical_round_head_with_reserved_control(
          &mut meter,
          now,
          &mut ledger,
          limits,
          crate::BlockResourceDomain::ActorBaseEffect,
          remaining,
        );
        assert_eq!(ledger.outstanding_reservations(), 1);
        if refused {
          assert_eq!(result, Err(crate::ServiceRoundError::InsufficientWeight));
          assert_eq!(meter.consumed(), common);
          assert_eq!(root(), before);
        } else {
          assert_ok!(result);
          assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
        }
        let actual_control = meter.consumed() - ledger.usage().actor_effect_used();
        assert_ok!(ledger.settle(&mut outer, actual_control));
        assert_eq!(ledger.outstanding_reservations(), 0);
        assert_eq!(
          ledger.usage().actor_control_used() + ledger.usage().actor_effect_used(),
          meter.consumed()
        );
      });
    }
  }
}

#[test]
fn financial_wait_runtime_service_settles_control_and_effect_once_in_base_and_drain() {
  for source in [Source::AtTime, Source::Opening, Source::Running] {
    for drain in [false, true] {
      for refused in [false, true] {
        new_test_ext().execute_with(|| {
          let (actor, episode, now) = funded_source_service(source);
          if matches!(source, Source::Opening) {
            let sovereign = sovereign_account(actor.actor_id);
            let capacity = episode.record().unpaid_liability() + TestMinUserBalance::get();
            set_native_transfer_lock(&sovereign, native_balance(&sovereign) - capacity);
          }
          let budget = crate::BlockResourceBudget::new_with_control_ratio(
            Weight::from_parts(2_000_000_000_000, 10_485_760),
            Weight::zero(),
            1,
            3,
          )
          .unwrap();
          let mut ledger = crate::BlockResourceState::new(now);
          assert_ok!(ledger.begin_prepass(budget));
          let domain = if drain {
            assert_ok!(ledger.open_external_phase());
            assert_ok!(ledger.begin_drain());
            crate::BlockResourceDomain::ActorDrainEffect
          } else {
            crate::BlockResourceDomain::ActorBaseEffect
          };
          if refused {
            set_fail_fee_sink_transfer(true);
          }
          let before = root();
          let mut meter = WeightMeter::with_limit(Weight::MAX);
          let result = Actors::service_canonical_round_head_with_resources(
            &mut meter,
            now,
            &mut ledger,
            budget.limits(),
            domain,
          );
          assert_eq!(result.is_err(), refused);
          assert_eq!(ledger.outstanding_reservations(), 0);
          assert_eq!(
            ledger.usage().actor_control_used() + ledger.usage().actor_effect_used(),
            meter.consumed()
          );
          if matches!(source, Source::AtTime) || matches!(source, Source::Opening) && !refused {
            assert_eq!(ledger.usage().actor_effect_used(), Weight::zero());
          } else {
            assert!(ledger.usage().actor_effect_used().ref_time() > 0);
          }
          if refused {
            assert_eq!(root(), before);
            assert_eq!(
              ActorFinancialWaits::<Test>::get(actor.actor_id),
              Some(episode)
            );
          } else if matches!(source, Source::Opening) {
            let action_wait = ActorFinancialWaits::<Test>::get(actor.actor_id).unwrap();
            assert_ne!(action_wait.original_identity(), episode.original_identity());
            assert_eq!(
              action_wait.record().unpaid_service(),
              UnpaidActorService::RunningAction {
                cycle_nonce: 1,
                cursor: 0
              }
            );
          } else {
            assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
          }
        });
      }
    }
  }
}

#[test]
fn financial_wait_runtime_due_preserves_independent_window_terminal_before_solvency() {
  for source in [Source::Opening, Source::Running] {
    for mutability in [Mutability::Mutable, Mutability::Immutable] {
      for (funded, via_tick) in [(false, false), (true, false), (false, true), (true, true)] {
        new_test_ext().execute_with(|| {
          let (actor, unpaid) = prepared_source_with_funding(
            source,
            true,
            mutability,
            false,
            false,
            None,
            Some(crate::ScheduleWindow { start: 1, end: 101 }),
            false,
          );
          let episode = Actors::publish_prepared_financial_wait(
            actor, unpaid, &mut WeightMeter::with_limit(Weight::MAX),
          ).unwrap();
          set_prepared_financial_wait_due_weight(true);
          if funded {
            make_service_solvent(actor, episode);
          }
          let sovereign = sovereign_account(actor.actor_id);
          let native = native_balance(&sovereign);
          let bob = native_balance(&BOB);
          let prefix = asset_balance(&BOB, TestAsset::Local(1));
          let fees = fee_collections();
          let expiry = episode.record().expiry_tick();
          System::set_block_number(expiry);
          System::reset_events();
          let complete = due_weight() + if via_tick { TestWeightInfo::classify_due_tick_deadline() } else { Weight::zero() };
          let mut meter = WeightMeter::with_limit(complete);
          let result = if via_tick {
            Actors::process_next_due_tick_deadline(&mut meter, expiry).unwrap()
          } else {
            Actors::process_financial_wait_due(actor, expiry, &mut meter).unwrap()
          };
          assert_eq!(result, crate::DueTickDeadlineMutation::FinancialWaitClosed(due_receipt(actor, episode)));
          assert_eq!(meter.consumed(), complete);
          assert!(has_actor_event(|event| matches!(event, Event::ActorClosed { actor_id, reason: CloseReason::WindowExpired } if *actor_id == actor.actor_id)));
          assert!(!has_actor_event(|event| matches!(event, Event::FinancialWaitRecovered { .. } | Event::ActionFeeCharged { .. } | Event::CycleStarted { .. })));
          assert!(Actors::actor_identity(actor.actor_id).is_none());
          assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
          assert!(!DeadlineHandles::<Test>::contains_key(actor.actor_id));
          assert!(!crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
          assert!(!ActorRunStateStore::<Test>::contains_key(actor.actor_id));
          assert_eq!(native_balance(&sovereign), native);
          assert_eq!(native_balance(&BOB), bob);
          assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), prefix);
          assert_eq!(fee_collections(), fees);
          #[cfg(feature = "try-runtime")]
          assert_ok!(Actors::do_try_state());
        });
      }
    }
  }
}

#[test]
fn financial_wait_runtime_due_closes_each_insolvent_source_without_custody_effects() {
  for source in [
    Source::AtTime,
    Source::Opening,
    Source::Running,
    Source::Suspended,
  ] {
    new_test_ext().execute_with(|| {
      let (actor, episode) = parked_source(source);
      let sovereign = sovereign_account(actor.actor_id);
      let native = native_balance(&sovereign);
      let foreign = asset_balance(&sovereign, TestAsset::Local(1));
      let bob = asset_balance(&BOB, TestAsset::Local(1));
      let fees = fee_collections();
      let expiry = episode.record().expiry_tick();
      System::set_block_number(expiry);
      let mut meter = WeightMeter::with_limit(due_weight());
      assert_eq!(
        Actors::process_financial_wait_due(actor, expiry, &mut meter),
        Ok(crate::DueTickDeadlineMutation::FinancialWaitClosed(
          due_receipt(actor, episode)
        )),
      );
      assert_eq!(meter.consumed(), due_weight());
      assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
      assert!(!ActorProcesses::<Test>::contains_key(actor.actor_id));
      assert!(!ActorSemanticStates::<Test>::contains_key(actor.actor_id));
      assert!(!DeadlineHandles::<Test>::contains_key(actor.actor_id));
      assert!(!crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
      assert!(!ActorRunStateStore::<Test>::contains_key(actor.actor_id));
      assert!(Actors::actor_state_hold(actor.actor_id).is_none());
      assert_eq!(native_balance(&sovereign), native);
      assert_eq!(asset_balance(&sovereign, TestAsset::Local(1)), foreign);
      assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), bob);
      assert_eq!(fee_collections(), fees);
      assert!(has_actor_event(|event| matches!(event,
        Event::ActorClosed { actor_id, reason: CloseReason::FundingGraceExpired }
          if *actor_id == actor.actor_id)));
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_funded_due_returns_b_plus_one_without_repricing_or_new_episode() {
  for source in [
    Source::AtTime,
    Source::Opening,
    Source::Running,
    Source::Suspended,
  ] {
    for late in [0, 17] {
      new_test_ext().execute_with(|| {
        let (actor, episode) = parked_source(source);
        make_service_solvent(actor, episode);
        let identity = Actors::actor_identity(actor.actor_id).unwrap();
        let hot = Actors::actor_hot(actor.actor_id).unwrap();
        let hold = Actors::actor_state_hold(actor.actor_id);
        let run = ActorRunStateStore::<Test>::get(actor.actor_id).map(|run| run.encode());
        let native = native_balance(&identity.sovereign_account);
        let fees = fee_collections();
        let now = episode.record().expiry_tick() + late;
        System::set_block_number(now);
        let mut meter = WeightMeter::with_limit(due_weight());
        assert_eq!(
          Actors::process_financial_wait_due(actor, now, &mut meter),
          Ok(crate::DueTickDeadlineMutation::FinancialWaitReturned(
            due_receipt(actor, episode)
          )),
        );
        assert_eq!(
          meter.consumed(),
          Actors::financial_wait_due_weights().unwrap().0
        );
        assert_eq!(
          ActorFinancialWaits::<Test>::get(actor.actor_id),
          Some(episode)
        );
        assert_eq!(Actors::actor_identity(actor.actor_id), Some(identity));
        assert_eq!(Actors::actor_hot(actor.actor_id), Some(hot));
        assert_eq!(Actors::actor_state_hold(actor.actor_id), hold);
        assert_eq!(
          ActorRunStateStore::<Test>::get(actor.actor_id).map(|run| run.encode()),
          run
        );
        assert_eq!(native_balance(&sovereign_account(actor.actor_id)), native);
        assert_eq!(fee_collections(), fees);
        assert!(!DeadlineHandles::<Test>::contains_key(actor.actor_id));
        assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
          actor.actor_id
        ));
        let node = crate::ServiceNodes::<Test>::get(actor.actor_id).unwrap();
        assert_eq!(node.eligible_from, now + 1);
        assert_eq!(
          node.kind,
          if matches!(source, Source::Opening) {
            ServiceResidenceKind::Pending
          } else {
            ServiceResidenceKind::Live
          }
        );
        let process = ActorProcesses::<Test>::get(actor.actor_id).unwrap();
        assert_eq!(process.status, ProcessStatus::AwaitingFunding);
        assert_eq!(
          process.residence,
          Some(ProcessResidence::Service(node.kind))
        );
        assert!(Actors::load_canonical_actor_semantic_state(actor).is_ok());
        #[cfg(feature = "try-runtime")]
        assert_ok!(Actors::do_try_state());
      });
    }
  }
}

#[test]
fn financial_wait_runtime_due_uses_actual_time_and_never_closes_early() {
  new_test_ext().execute_with(|| {
    let (actor, episode) = parked_source(Source::Running);
    let expiry = episode.record().expiry_tick();
    System::set_block_number(expiry - 1);
    let before = root();
    let mut meter = WeightMeter::with_limit(due_weight());
    assert_eq!(
      Actors::process_financial_wait_due(actor, expiry, &mut meter),
      Err(crate::FinancialWaitDueError::NotDue),
    );
    assert_eq!(root(), before);
    assert_eq!(meter.consumed(), due_weight());
    System::set_block_number(expiry);
    let before = root();
    assert_eq!(
      Actors::process_financial_wait_due(
        actor,
        expiry - 1,
        &mut WeightMeter::with_limit(Weight::MAX)
      ),
      Err(crate::FinancialWaitDueError::NotDue),
    );
    assert_eq!(root(), before);
  });
}

#[test]
fn financial_wait_runtime_due_requires_the_complete_debt_above_protected_minimum() {
  for funded in [false, true] {
    new_test_ext().execute_with(|| {
      let (actor, episode) = parked_source(Source::Opening);
      let amount =
        episode.record().unpaid_liability() + TestMinUserBalance::get() - Balance::from(!funded);
      // Keep the old buffer locked: only this new arrival is spendable.
      fund_native(actor.actor_id, amount);
      assert_eq!(
        asset_balance(&sovereign_account(actor.actor_id), TestAsset::Native),
        amount
      );
      let now = episode.record().expiry_tick();
      System::set_block_number(now);
      let result =
        Actors::process_financial_wait_due(actor, now, &mut WeightMeter::with_limit(due_weight()));
      assert_eq!(
        result,
        Ok(if funded {
          crate::DueTickDeadlineMutation::FinancialWaitReturned(due_receipt(actor, episode))
        } else {
          crate::DueTickDeadlineMutation::FinancialWaitClosed(due_receipt(actor, episode))
        })
      );
      if funded {
        assert_eq!(
          ActorFinancialWaits::<Test>::get(actor.actor_id),
          Some(episode)
        );
        let before = root();
        assert_eq!(
          Actors::process_financial_wait_due(
            actor,
            now,
            &mut WeightMeter::with_limit(due_weight())
          ),
          Err(crate::FinancialWaitDueError::InvalidSource),
        );
        assert_eq!(root(), before);
      }
    });
  }
}

#[test]
fn financial_wait_runtime_due_refusals_restore_authority_and_keep_incurred_weight() {
  for refusal in 0..8 {
    new_test_ext().execute_with(|| {
      let (mut actor, episode) = parked_source(Source::Opening);
      make_service_solvent(actor, episode);
      System::set_block_number(episode.record().expiry_tick());
      let complete = due_weight();
      let mut limit = complete;
      match refusal {
        0 => set_prepared_financial_wait_due_weight(false),
        1 => limit = Weight::from_parts(complete.ref_time() - 1, complete.proof_size()),
        2 => limit = Weight::from_parts(complete.ref_time(), complete.proof_size() - 1),
        3 => GlobalCircuitBreaker::<Test>::put(true),
        4 => actor.generation += 1,
        5 => crate::ServiceHeader::<Test>::mutate(|header| header.count = u32::MAX),
        6 => ActorFinancialWaits::<Test>::remove(actor.actor_id),
        // The funded Opening still owes its authenticated complete Contract at due inspection.
        7 => crate::ActorContractTailChunks::<Test>::remove(actor.actor_id, 0),
        _ => unreachable!(),
      }
      let before = root();
      let fees = fee_collections();
      let mut meter = WeightMeter::with_limit(limit);
      assert!(
        Actors::process_financial_wait_due(actor, episode.record().expiry_tick(), &mut meter)
          .is_err()
      );
      assert_eq!(root(), before, "refusal {refusal}");
      assert_eq!(fee_collections(), fees);
      assert_eq!(
        meter.consumed(),
        if refusal < 3 {
          Weight::zero()
        } else {
          complete
        }
      );
    });
  }
}

#[test]
fn financial_wait_runtime_late_due_cleanup_refusal_restores_entire_wait() {
  use polkadot_sdk::frame_support::traits::{
    fungible::{InspectHold, MutateHold},
    tokens::Precision,
  };
  new_test_ext().execute_with(|| {
    let (actor, episode) = parked_source(Source::Suspended);
    let reason: RuntimeHoldReason = crate::HoldReason::ActorState.into();
    let held = <Balances as InspectHold<AccountId>>::balance_on_hold(&reason, &ALICE);
    <Balances as MutateHold<AccountId>>::release(&reason, &ALICE, held, Precision::Exact).unwrap();
    System::set_block_number(episode.record().expiry_tick());
    let before = root();
    let fees = fee_collections();
    let mut meter = WeightMeter::with_limit(due_weight());
    assert_eq!(
      Actors::process_financial_wait_due(actor, episode.record().expiry_tick(), &mut meter),
      Err(crate::FinancialWaitDueError::TerminalCleanup),
    );
    assert_eq!(root(), before);
    assert_eq!(fee_collections(), fees);
    assert_eq!(meter.consumed(), due_weight());
    assert_eq!(
      ActorFinancialWaits::<Test>::get(actor.actor_id),
      Some(episode)
    );
  });
}

#[test]
fn financial_wait_runtime_shared_tick_worker_uses_financial_branch_and_retains_close_authority() {
  new_test_ext().execute_with(|| {
    let (actor, episode) = parked_source(Source::Running);
    make_service_solvent(actor, episode);
    let now = episode.record().expiry_tick();
    System::set_block_number(now);
    let fees = fee_collections();
    let mut meter = WeightMeter::with_limit(Weight::MAX);
    assert_eq!(
      Actors::process_next_due_tick_deadline(&mut meter, now),
      Ok(crate::DueTickDeadlineMutation::FinancialWaitReturned(
        due_receipt(actor, episode)
      )),
    );
    assert_eq!(
      meter.consumed(),
      <Test as crate::Config>::WeightInfo::classify_due_tick_deadline()
        + Actors::financial_wait_due_weights().unwrap().0
    );
    let sovereign = sovereign_account(actor.actor_id);
    let native = native_balance(&sovereign);
    assert_ok!(Actors::close_actor(
      RuntimeOrigin::signed(ALICE),
      actor.actor_id
    ));
    assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    assert!(!ActorProcesses::<Test>::contains_key(actor.actor_id));
    assert!(!crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
    assert_eq!(native_balance(&sovereign), native);
    assert_eq!(fee_collections(), fees);
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}

#[test]
fn financial_wait_runtime_entry_preserves_each_owed_source_and_paid_prefix() {
  for source in [
    Source::AtTime,
    Source::Opening,
    Source::Running,
    Source::Suspended,
  ] {
    new_test_ext().execute_with(|| {
      let (actor, unpaid) = prepared_source(source);
      let identity = Actors::actor_identity(actor.actor_id).unwrap();
      let hot = Actors::actor_hot(actor.actor_id).unwrap();
      let run = ActorRunStateStore::<Test>::get(actor.actor_id).map(|run| run.encode());
      let custody = native_balance(&identity.sovereign_account);
      let foreign = asset_balance(&identity.sovereign_account, TestAsset::Local(1));
      let bob = asset_balance(&BOB, TestAsset::Local(1));
      let fees = fee_collections();
      let mut meter = WeightMeter::with_limit(entry_weight());
      let episode = Actors::publish_prepared_financial_wait(actor, unpaid, &mut meter).unwrap();
      assert_eq!(meter.consumed(), entry_weight());
      assert_eq!(episode.record().unpaid_service(), unpaid);
      assert_eq!(
        episode.record().entered_at_millis(),
        System::block_number() * 500
      );
      assert_eq!(
        episode.record().expiry_tick(),
        System::block_number() + 4_838_400
      );
      assert_eq!(
        ActorFinancialWaits::<Test>::get(actor.actor_id),
        Some(episode)
      );
      let mut expected_hot = hot;
      expected_hot.trigger_wakeup_pointer = None;
      assert_eq!(Actors::actor_hot(actor.actor_id), Some(expected_hot));
      assert_eq!(
        Actors::actor_identity(actor.actor_id),
        Some(identity.clone())
      );
      assert_eq!(
        ActorRunStateStore::<Test>::get(actor.actor_id).map(|run| run.encode()),
        run
      );
      assert_eq!(native_balance(&identity.sovereign_account), custody);
      assert_eq!(
        asset_balance(&identity.sovereign_account, TestAsset::Local(1)),
        foreign
      );
      assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), bob);
      assert_eq!(fee_collections(), fees);
      assert!(!crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
      assert!(!crate::TriggerDeadlineHandles::<Test>::contains_key(
        actor.actor_id
      ));
      let process = ActorProcesses::<Test>::get(actor.actor_id).unwrap();
      let handle = DeadlineHandles::<Test>::get(actor.actor_id).unwrap();
      assert_eq!(process.status, ProcessStatus::AwaitingFunding);
      assert_eq!(handle.key, WakeupKey::Tick(episode.record().expiry_tick()));
      assert_eq!(
        process.residence,
        Some(ProcessResidence::Deadline {
          key: handle.key,
          page: handle.page,
          slot: handle.slot,
        })
      );
      assert!(Actors::load_canonical_actor_semantic_state(actor).is_ok());
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[test]
fn financial_wait_runtime_entry_refusals_preserve_complete_root_and_charge_incurred_work() {
  for refusal in 0..6 {
    new_test_ext().execute_with(|| {
      let (mut actor, unpaid) = prepared_source(Source::Opening);
      let mut limit = entry_weight();
      match refusal {
        0 => set_prepared_financial_wait_weights(false),
        1 => limit = Weight::from_parts(limit.ref_time(), limit.proof_size() - 1),
        2 => crate::GlobalCircuitBreaker::<Test>::put(true),
        3 => actor.generation += 1,
        4 => {
          // A late index publication refusal occurs after source detachment and destination writes.
          crate::DeadlineIndexLen::<Test>::insert(WakeupClock::Tick, u32::MAX);
        }
        5 => {
          <Balances as LockableCurrency<AccountId>>::remove_lock(
            *b"actlock0",
            &sovereign_account(actor.actor_id),
          );
          let head = crate::ActorContractHeads::<Test>::get(actor.actor_id).unwrap();
          fund_native(
            actor.actor_id,
            head
              .header
              .pipeline_machine_envelope
              .pipeline_machine_fee_upper,
          );
        }
        _ => unreachable!(),
      }
      let before = root();
      let fees = fee_collections();
      let mut meter = WeightMeter::with_limit(limit);
      let result = Actors::publish_prepared_financial_wait(actor, unpaid, &mut meter);
      assert!(result.is_err(), "refusal {refusal}: {result:?}");
      if refusal <= 1 {
        assert_eq!(meter.consumed(), Weight::zero());
      } else {
        assert_eq!(meter.consumed(), entry_weight());
      }
      assert_eq!(root(), before, "refusal {refusal}");
      assert_eq!(fee_collections(), fees);
      assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    });
  }
}

#[test]
fn financial_wait_runtime_original_episode_is_not_replaced_or_dispatched_as_retry() {
  new_test_ext().execute_with(|| {
    let (actor, unpaid) = prepared_source(Source::Suspended);
    let episode = Actors::publish_prepared_financial_wait(
      actor,
      unpaid,
      &mut WeightMeter::with_limit(Weight::MAX),
    )
    .unwrap();
    for block in [
      10,
      episode.record().expiry_tick(),
      episode.record().expiry_tick() + 1,
    ] {
      System::set_block_number(block);
      let before = root();
      assert_eq!(
        Actors::publish_prepared_financial_wait(
          actor,
          unpaid,
          &mut WeightMeter::with_limit(Weight::MAX)
        ),
        Err(FinancialWaitPublicationError::AlreadyWaiting)
      );
      assert_eq!(root(), before);
    }
    let expiry = episode.record().expiry_tick();
    assert_eq!(
      Actors::classify_next_due_tick_deadline(expiry),
      Ok(crate::DueBlockDeadlineBranch::FinancialWait(actor))
    );
    let before = root();
    set_prepared_financial_wait_due_weight(false);
    assert!(
      Actors::process_next_due_tick_deadline(&mut WeightMeter::with_limit(Weight::MAX), expiry)
        .is_err()
    );
    assert_eq!(root(), before);
    assert_eq!(
      ActorFinancialWaits::<Test>::get(actor.actor_id),
      Some(episode)
    );
  });
}

#[test]
fn financial_wait_runtime_late_hold_authority_refusal_restores_temporal_source() {
  new_test_ext().execute_with(|| {
    let (actor, unpaid) = prepared_source(Source::AtTime);
    crate::ActorStateHolds::<Test>::mutate(actor.actor_id, |hold| {
      hold.as_mut().unwrap().owner = BOB;
    });
    let before = root();
    let fees = fee_collections();
    let mut meter = WeightMeter::with_limit(entry_weight());
    assert_eq!(
      Actors::publish_prepared_financial_wait(actor, unpaid, &mut meter),
      Err(FinancialWaitPublicationError::StateHold)
    );
    assert_eq!(root(), before);
    assert_eq!(meter.consumed(), entry_weight());
    assert_eq!(fee_collections(), fees);
    assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    assert!(crate::TriggerDeadlineHandles::<Test>::contains_key(
      actor.actor_id
    ));
  });
}

#[test]
fn financial_wait_runtime_companion_and_process_must_have_one_current_owner() {
  for version in [1, 2] {
    for corruption in 0..4 {
      new_test_ext().execute_with(|| {
        let (actor, unpaid) = prepared_source(Source::Opening);
        let episode = Actors::publish_prepared_financial_wait(
          actor,
          unpaid,
          &mut WeightMeter::with_limit(Weight::MAX),
        )
        .unwrap();
        set_admission_semantics_version(version);
        match corruption {
          0 => ActorFinancialWaits::<Test>::remove(actor.actor_id),
          1 => {
            let mut encoded = episode.encode();
            *encoded.last_mut().unwrap() ^= 1;
            let changed = FinancialWaitEpisode::<Balance>::decode(&mut &encoded[..]).unwrap();
            ActorFinancialWaits::<Test>::insert(actor.actor_id, changed);
          }
          2 => ActorProcesses::<Test>::mutate(actor.actor_id, |process| {
            process.as_mut().unwrap().status = ProcessStatus::Serving;
          }),
          3 => ActorFinancialWaits::<Test>::insert(actor.actor_id + 1, episode),
          _ => unreachable!(),
        }
        let before = root();
        if corruption <= 1 {
          assert!(Actors::load_canonical_actor_semantic_state(actor).is_err());
        }
        #[cfg(feature = "try-runtime")]
        assert!(Actors::do_try_state().is_err());
        assert_eq!(root(), before);
      });
    }
  }
}

#[test]
fn financial_wait_runtime_close_removes_optional_authority_without_custody_effects() {
  new_test_ext().execute_with(|| {
    let (actor, unpaid) = prepared_source(Source::Running);
    Actors::publish_prepared_financial_wait(
      actor,
      unpaid,
      &mut WeightMeter::with_limit(Weight::MAX),
    )
    .unwrap();
    let sovereign = sovereign_account(actor.actor_id);
    let total = MockAssetOps::total_balance(&sovereign, TestAsset::Native);
    let foreign = asset_balance(&sovereign, TestAsset::Local(1));
    System::set_block_number(4);
    assert_ok!(Actors::close_actor(
      RuntimeOrigin::signed(ALICE),
      actor.actor_id
    ));
    assert!(!ActorFinancialWaits::<Test>::contains_key(actor.actor_id));
    assert!(!ActorProcesses::<Test>::contains_key(actor.actor_id));
    assert!(!DeadlineHandles::<Test>::contains_key(actor.actor_id));
    assert_eq!(
      MockAssetOps::total_balance(&sovereign, TestAsset::Native),
      total
    );
    assert_eq!(asset_balance(&sovereign, TestAsset::Local(1)), foreign);
    #[cfg(feature = "try-runtime")]
    assert_ok!(Actors::do_try_state());
  });
}
