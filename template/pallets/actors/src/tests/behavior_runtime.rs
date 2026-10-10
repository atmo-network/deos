use super::*;
use crate::{
  ActorAdmissionCertificateOf, ActorContractOf, ActorRef, AdmissionCertificateAuthorityProvider,
  AmountResolution, LoadedActorStep, Step,
};

// This fixture exercises the real transaction on a prepared source; public admission and physical
// policy resource qualification remain separate gates, and no production coefficient is supplied.
fn prepared_policy_source(
  steps: crate::ContractSteps<Test>,
  expression: crate::CloseWhenOf<Test>,
) -> (ActorRef, ActorContractOf<Test>) {
  prepared_policy_source_for_class(steps, expression, ActorType::System)
}

fn prepared_policy_source_for_class(
  steps: crate::ContractSteps<Test>,
  expression: crate::CloseWhenOf<Test>,
  actor_type: ActorType,
) -> (ActorRef, ActorContractOf<Test>) {
  prepared_policy_source_with_readiness(steps, expression, actor_type, true)
}

fn prepared_policy_source_with_readiness(
  steps: crate::ContractSteps<Test>,
  expression: crate::CloseWhenOf<Test>,
  actor_type: ActorType,
  ready: bool,
) -> (ActorRef, ActorContractOf<Test>) {
  prepared_policy_source_with_window(steps, expression, actor_type, ready, None)
}

fn prepared_policy_source_with_window(
  steps: crate::ContractSteps<Test>,
  expression: crate::CloseWhenOf<Test>,
  actor_type: ActorType,
  ready: bool,
  window: Option<crate::ScheduleWindow<u64>>,
) -> (ActorRef, ActorContractOf<Test>) {
  prepared_policy_source_with_targets(steps, expression, actor_type, ready, window, None)
}

fn prepared_policy_source_with_targets(
  steps: crate::ContractSteps<Test>,
  expression: crate::CloseWhenOf<Test>,
  actor_type: ActorType,
  ready: bool,
  window: Option<crate::ScheduleWindow<u64>>,
  auto_close_at_cycle_nonce: Option<u64>,
) -> (ActorRef, ActorContractOf<Test>) {
  let (actor, contract) = install_prepared_policy_source_with_targets(
    steps,
    expression,
    actor_type,
    window,
    auto_close_at_cycle_nonce,
  );
  age_fixture_control_clock(actor.actor_id);
  assert_eq!(
    Actors::load_actor_contract(actor.actor_id),
    Some(contract.clone())
  );
  if ready {
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor.actor_id
    ));
  }
  (actor, contract)
}

fn install_prepared_policy_source_with_targets(
  steps: crate::ContractSteps<Test>,
  expression: crate::CloseWhenOf<Test>,
  actor_type: ActorType,
  window: Option<crate::ScheduleWindow<u64>>,
  auto_close_at_cycle_nonce: Option<u64>,
) -> (ActorRef, ActorContractOf<Test>) {
  System::set_block_number(1);
  // Model the complete configured cleanup domain without enabling financial prices or freezing
  // the host semantics version. These fixture coordinates are never reference coefficients.
  set_prepared_behavior_lifecycle_weight(true);
  let actor_id = Actors::next_actor_id();
  let mut contract = match actor_type {
    ActorType::System => {
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        ALICE,
        Mutability::Mutable,
        None,
      ));
      system_active_contract(manual_schedule(), None, steps).unwrap()
    }
    ActorType::User => {
      prefund_active_user_creation(ALICE, &steps);
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        None,
      ));
      user_active_contract(manual_schedule(), None, steps).unwrap()
    }
  };
  contract.close_when = Some(expression);
  contract.window = window;
  contract.auto_close_at_cycle_nonce = auto_close_at_cycle_nonce;
  let Some(crate::ActorSemanticState::Dormant(record)) =
    crate::ActorSemanticStates::<Test>::get(actor_id)
  else {
    panic!("Dormant installation source");
  };
  let authority = <Test as crate::Config>::AdmissionCertificateAuthority::current().unwrap();
  let admission = ActorAdmissionCertificateOf::<Test>::new(
    contract.semantic_contract_id(),
    contract.body_commitment().unwrap(),
    contract.trigger.wake_qualification(&contract.window),
    authority.runtime_actor_semantics_version,
    authority.production_weight_identity,
    authority.body_geometry_version,
    authority.configured_bounds_commitment,
    authority.maximum_lifecycle_weight,
  );
  let hot = crate::ActorHotState {
    lifecycle: ActiveLifecycle::Active,
    cycle_state: CycleState::Idle,
    trigger_runtime_state: crate::TriggerRuntimeState::Stateless,
    unsuccessful_attempt_streak: 0,
    pending_signal: false,
    queue_ticket: None,
    wakeup_pointer: None,
    trigger_wakeup_pointer: None,
    terminal_at: None,
    schedule_anchor: 1,
    last_cycle_block: None,
  };
  assert_ok!(polkadot_sdk::frame_support::storage::with_transaction(
    || {
      let result = (|| {
        Actors::insert_active_actor_with_admission(
          actor_id,
          record.identity,
          hot,
          contract.clone(),
          crate::TriggerTransitionIntent::ActivateDormant,
          admission,
        )?;
        crate::ActiveActorCount::<Test>::mutate(|count| *count += 1);
        Actors::reconcile_actor_state_hold_with_authority(actor_id)
      })();
      match result {
        Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
        Err(error) => {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
        }
      }
    }
  ));
  (Actors::load_actor_ref(actor_id).unwrap(), contract)
}

#[test]
fn behavior_runtime_clean_run_deactivation_preserves_custody_and_retires_policy() {
  use polkadot_sdk::frame_support::traits::fungible::InspectHold;
  // Run the simple controls first and stop at the first attributable clean-Run counterexample.
  for (case, (clean, prefix)) in [(false, false), (true, false), (false, true), (true, true)]
    .into_iter()
    .enumerate()
  {
    new_test_ext().execute_with(|| {
      let steps: crate::ContractSteps<Test> = vec![transfer_step(None); 2].try_into().unwrap();
      let expression = single_metric(
        if clean { crate::BehaviorMetric::CycleCleanTotal }
        else { crate::BehaviorMetric::ActionSuccessTotal },
        if clean { 1 } else { 2 },
      );
      let slot = Actors::available_owner_slot(&ALICE, None).unwrap();
      let payer = Actors::sovereign_account_id(&ALICE, slot);
      // All funding precedes installation; no post-prefix balance restoration or clock mutation.
      assert_ok!(MockAssetOps::mint(&payer, TestAsset::Native, 1_000_000_000_000));
      assert_ok!(MockAssetOps::mint(&payer, TestAsset::Local(901), 77));
      let (actor, contract) = install_prepared_policy_source_with_targets(
        steps, expression, ActorType::User, None, None,
      );
      let id = actor.actor_id;
      assert_eq!(Actors::load_actor_contract(id), Some(contract));
      assert_eq!(Actors::actor_identity(id).unwrap().last_control_mutation_block, 1);
      assert!(!crate::ActorControlLocators::<Test>::contains_key(id));
      assert!(!crate::ActorUnsignaledControlCells::<Test>::contains_key(id));
      let bob_before = native_balance(&BOB);
      System::set_block_number(2);
      if prefix {
        assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
        System::set_block_number(3);
        let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
        assert_eq!(Actors::service_canonical_round_head(&mut meter, 3), Ok(crate::ServiceRoundEncounter::Eligible(actor)));
        let run = Actors::actor_run_state(id).unwrap();
        assert_eq!(run.cursor, 1);
        assert_eq!(run.cumulative_outcomes.committed_effectful_tasks, 1);
        assert_eq!(native_balance(&BOB), bob_before + 10);
      }
      let monitor = crate::ActorBehaviorMonitors::<Test>::get(id).unwrap();
      assert_eq!(monitor.clean_cycle.is_some(), clean && prefix);
      assert_eq!(Actors::actor_identity(id).unwrap().last_control_mutation_block, 1);
      System::set_block_number(4);
      let run_before = Actors::actor_run_state(id);
      let ledger = || (
        native_balance(&payer), MockAssetOps::balance(&payer, TestAsset::Local(901)),
        native_balance(&BOB), native_balance(&TestFeeSink::get()),
      );
      let balances = ledger();
      let reason: RuntimeHoldReason = crate::HoldReason::ActorState.into();
      let held_before = Balances::balance_on_hold(&reason, &ALICE);
      let owner_free = native_balance(&ALICE);
      let identities = crate::ActorIdentityCount::<Test>::get();
      let active = crate::ActiveActorCount::<Test>::get();
      let slots = crate::OwnerSlotBitmaps::<Test>::get(ALICE);
      let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      polkadot_sdk::frame_support::storage::with_transaction(|| {
        assert_ok!(Actors::deactivate_actor(RuntimeOrigin::signed(ALICE), id));
        assert!(matches!(crate::ActorSemanticStates::<Test>::get(id), Some(crate::ActorSemanticState::Dormant(_))));
        polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Ok::<_, DispatchError>(()))
      }).unwrap();
      assert_eq!(polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1), root,
        "late caller refusal restores the entire source, holds, events and paid prefix");
      assert_eq!(Actors::actor_run_state(id), run_before);
      assert_eq!(ledger(), balances);
      let result = Actors::deactivate_actor(RuntimeOrigin::signed(ALICE), id);
      if result.is_err() {
        assert_eq!(polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1), root,
          "any refusal restores full consensus state, events, holds and committed prefix");
        assert_eq!(Actors::actor_run_state(id), run_before);
        assert_eq!(ledger(), balances);
        assert_eq!(Balances::balance_on_hold(&reason, &ALICE), held_before);
      }
      assert!(result.is_ok(), "deactivation case={case}, clean={clean}, prefix={prefix}: {result:?}; rollback assertions passed");
      let Some(crate::ActorSemanticState::Dormant(dormant)) = crate::ActorSemanticStates::<Test>::get(id)
      else { panic!("successful deactivation retains Dormant identity") };
      assert_eq!(dormant.generation, actor.generation);
      assert_eq!(dormant.identity.sovereign_account, payer);
      assert_eq!(dormant.identity.last_control_mutation_block, 4);
      assert_eq!(dormant.identity.cycle_nonce, u64::from(prefix));
      assert_eq!(ledger(), balances);
      assert_eq!(crate::ActorIdentityCount::<Test>::get(), identities);
      assert_eq!(crate::ActiveActorCount::<Test>::get(), active - 1);
      assert_eq!(crate::OwnerSlotBitmaps::<Test>::get(ALICE), slots);
      assert_eq!(crate::SovereignIndex::<Test>::get(payer), Some(id));
      assert!(!crate::ActorContractHeads::<Test>::contains_key(id));
      assert!(crate::ActorContractTailChunks::<Test>::iter_prefix(id).next().is_none());
      assert!(!crate::ActorBehaviorPolicies::<Test>::contains_key(id));
      assert!(!crate::ActorBehaviorMonitors::<Test>::contains_key(id));
      assert!(!crate::ActorProcesses::<Test>::contains_key(id));
      assert!(!crate::ActorRunStateStore::<Test>::contains_key(id));
      assert!(!crate::ServiceNodes::<Test>::contains_key(id));
      assert!(!crate::DeadlineHandles::<Test>::contains_key(id));
      let held_after = Balances::balance_on_hold(&reason, &ALICE);
      assert!(held_after < held_before, "only Dormant geometry backing remains");
      assert_eq!(native_balance(&ALICE), owner_free + held_before - held_after);
      assert_eq!(System::events().iter().filter(|record| matches!(
        &record.event, RuntimeEvent::Actors(Event::CycleCancelled {
          actor_id, cycle_nonce: 1, reason: CancellationReason::Deactivated,
        }) if *actor_id == id
      )).count(), usize::from(prefix));
      assert_eq!(System::events().iter().filter(|record| matches!(
        &record.event, RuntimeEvent::Actors(Event::CycleSummary {
          actor_id, cycle_nonce: 1, result: CycleResult::Cancelled, outcomes,
        }) if *actor_id == id && run_before.as_ref().is_some_and(|run| run.cumulative_outcomes == *outcomes)
      )).count(), usize::from(prefix));
      assert_eq!(System::events().iter().filter(|record| matches!(
        &record.event, RuntimeEvent::Actors(Event::ActorDeactivated { actor_id }) if *actor_id == id
      )).count(), 1);
    });
  }
}

fn cleanup_policy_expression(case: u8) -> crate::CloseWhenOf<Test> {
  use crate::BehaviorMetric::*;
  let mut expression = match case {
    1 => single_metric(ActionSuccessTotal, 1),
    2 => single_metric(CycleCleanTotal, 1),
    3 => single_metric(ActionSuccessTotal, u32::MAX),
    _ => {
      let metrics = [
        PreconditionPassTotal,
        PreconditionPassStreak,
        PreconditionRejectTotal,
        PreconditionRejectStreak,
        ActionSuccessTotal,
        ActionSuccessStreak,
        ActionFailureTotal,
        ActionFailureStreak,
        CycleCompletedTotal,
        CycleCleanTotal,
      ];
      crate::CloseWhen {
        clauses: (0..4)
          .map(|clause| {
            (0..4)
              .map(|atom| crate::BehaviorAtom {
                metric: metrics[(clause * 4 + atom) % metrics.len()],
                threshold: u32::MAX,
              })
              .collect::<Vec<_>>()
              .try_into()
              .unwrap()
          })
          .collect::<Vec<_>>()
          .try_into()
          .unwrap(),
      }
    }
  };
  for clause in &mut expression.clauses {
    clause.sort_by_key(|atom| atom.metric);
  }
  expression.clauses.sort();
  expression.validate().unwrap();
  expression
}

#[test]
fn behavior_runtime_geometry_policy_authentication_preserves_source_and_prefix() {
  use polkadot_sdk::frame_support::storage::{TransactionOutcome, with_transaction};
  use polkadot_sdk::{sp_core::Blake2Hasher, sp_trie::recorder::Recorder};
  use std::collections::BTreeSet;
  let root = || polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
  for (case, prefix) in [
    (0, true),
    (1, false),
    (2, false),
    (2, true),
    (3, true),
    (4, true),
  ] {
    let mut ext = new_test_ext();
    let (actor, contract, payer, header) = ext.execute_with(|| {
      set_missing_behavior_inspection_weight(false);
      set_missing_behavior_control_weight(false);
      set_missing_behavior_close_weight(false);
      let steps: crate::ContractSteps<Test> = vec![
        make_step(Task::Transfer {
          to: BOB,
          asset: TestAsset::Native,
          amount: AmountResolution::Fixed(1),
        });
        5
      ]
      .try_into()
      .unwrap();
      let slot = Actors::available_owner_slot(&ALICE, None).unwrap();
      let payer = Actors::sovereign_account_id(&ALICE, slot);
      assert_ok!(MockAssetOps::mint(
        &payer,
        TestAsset::Native,
        1_000_000_000_000
      ));
      assert_ok!(MockAssetOps::mint(&payer, TestAsset::Local(901), 77));
      let (actor, contract) = if case == 0 {
        System::set_block_number(1);
        prefund_active_user_creation(ALICE, &steps);
        let id = Actors::next_actor_id();
        let contract = user_active_contract(manual_schedule(), None, steps).unwrap();
        assert_ok!(Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          Mutability::Mutable,
          Some(contract.clone())
        ));
        (Actors::load_actor_ref(id).unwrap(), contract)
      } else {
        install_prepared_policy_source_with_targets(
          steps,
          cleanup_policy_expression(case),
          ActorType::User,
          None,
          None,
        )
      };
      assert_eq!(sovereign_account(actor.actor_id), payer);
      let bob = native_balance(&BOB);
      System::set_block_number(2);
      if prefix {
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          actor.actor_id
        ));
        System::set_block_number(3);
        let payer_before = native_balance(&payer);
        let sink_before = native_balance(&TestFeeSink::get());
        let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
        assert_eq!(
          Actors::service_canonical_round_head(&mut meter, 3),
          Ok(crate::ServiceRoundEncounter::Eligible(actor))
        );
        let run = Actors::actor_run_state(actor.actor_id).unwrap();
        assert_eq!(run.cursor, 1);
        assert_eq!(run.cumulative_outcomes.committed_effectful_tasks, 1);
        assert_eq!(native_balance(&BOB), bob + 1);
        let paid = native_balance(&TestFeeSink::get()) - sink_before;
        assert!(
          paid > 0,
          "real User admission/attempt fees precede the read-only fragment"
        );
        assert_eq!(payer_before - native_balance(&payer), paid + 1);
      }
      assert_eq!(
        Actors::actor_identity(actor.actor_id)
          .unwrap()
          .last_control_mutation_block,
        1
      );
      set_missing_behavior_inspection_weight(true);
      set_missing_behavior_control_weight(true);
      set_missing_behavior_close_weight(true);
      let header = crate::ActorContractHeads::<Test>::get(actor.actor_id)
        .unwrap()
        .header;
      (actor, contract, payer, header)
    });
    let id = actor.actor_id;
    let snapshot = || {
      (
        crate::ActorSemanticStates::<Test>::get(id),
        crate::ActorProcesses::<Test>::get(id),
        crate::ActorRunStateStore::<Test>::get(id),
        crate::ActorBehaviorPolicies::<Test>::get(id),
        crate::ActorBehaviorMonitors::<Test>::get(id),
        crate::ActorStateHolds::<Test>::get(id),
        crate::OwnerSlotBitmaps::<Test>::get(ALICE),
        native_balance(&payer),
        <Test as crate::Config>::AssetOps::balance(&payer, TestAsset::Local(901)),
        native_balance(&BOB),
        native_balance(&TestFeeSink::get()),
      )
    };
    ext.execute_with(|| {
      let before = snapshot();
      let healthy = root();
      assert_eq!(
        Actors::benchmark_geometry_behavior_authentication(id, &header).as_ref(),
        Some(&contract.close_when)
      );
      assert_eq!(Actors::load_actor_contract(id).as_ref(), Some(&contract));
      if case != 0 {
        let policy = before.3.as_ref().unwrap();
        let context = Actors::behavior_control_weight_context(policy).unwrap();
        assert_eq!(context.requires_clean_cycle, matches!(case, 2 | 4));
        if case == 2 {
          assert_eq!(context.counter_slots, 0);
        }
        if case == 4 {
          assert_eq!(context.counter_slots, 10);
        }
        assert_eq!(
          before
            .4
            .as_ref()
            .is_some_and(|monitor| monitor.clean_cycle.is_some()),
          prefix && matches!(case, 2 | 4)
        );
        let mut mismatched = header.clone();
        mismatched.admission_identity[0] ^= 1;
        assert!(Actors::benchmark_geometry_behavior_authentication(id, &mismatched).is_none());
        assert_eq!(root(), healthy);
        for monitor_missing in [false, true] {
          if monitor_missing && case == 1 {
            continue;
          }
          with_transaction(|| {
            if monitor_missing {
              crate::ActorBehaviorMonitors::<Test>::remove(id);
            } else {
              crate::ActorBehaviorPolicies::<Test>::remove(id);
            }
            let missing = root();
            assert!(Actors::benchmark_geometry_behavior_authentication(id, &header).is_none());
            assert!(Actors::load_actor_contract(id).is_none());
            assert_eq!(root(), missing, "authentication refusal has no writes");
            TransactionOutcome::Rollback(Ok::<_, DispatchError>(()))
          })
          .unwrap();
          assert_eq!(root(), healthy);
          assert_eq!(
            snapshot(),
            before,
            "fault rollback retains the paid prefix and custody"
          );
        }
      }
      assert_eq!(root(), healthy);
      assert_eq!(snapshot(), before);
    });
    ext.commit_all().unwrap();
    let recorder = Recorder::<Blake2Hasher>::default();
    let actual = ext.execute_with_recorder(recorder.clone(), || {
      Actors::benchmark_geometry_behavior_authentication(id, &header)
    });
    assert_eq!(actual.as_ref(), Some(&contract.close_when));
    let reads = recorder
      .recorded_keys()
      .into_values()
      .flat_map(|keys| keys.into_keys())
      .map(|key| key.as_ref().to_vec())
      .collect::<BTreeSet<_>>();
    let mut expected = BTreeSet::new();
    if case != 0 {
      expected.insert(crate::ActorBehaviorPolicies::<Test>::hashed_key_for(id));
      expected.insert(crate::ActorSemanticStates::<Test>::hashed_key_for(id));
    }
    if case >= 2 {
      expected.insert(crate::ActorBehaviorMonitors::<Test>::hashed_key_for(id));
    }
    if prefix && matches!(case, 2 | 4) {
      expected.insert(crate::ActorRunStateStore::<Test>::hashed_key_for(id));
    }
    assert_eq!(
      reads, expected,
      "exact optional keys, not the broader Service snapshot"
    );
    assert!(
      ext.overlayed_changes().changes().next().is_none(),
      "read-only kernel"
    );
    ext.execute_with(|| assert_eq!(Actors::load_actor_contract(id).as_ref(), Some(&contract)));
  }
  set_missing_behavior_inspection_weight(false);
  set_missing_behavior_control_weight(false);
  set_missing_behavior_close_weight(false);
}

#[test]
fn behavior_runtime_policy_only_cleanup_preserves_collateral_and_prefix() {
  use codec::{Encode, MaxEncodedLen};
  use polkadot_sdk::frame_support::storage::{TransactionOutcome, with_transaction};
  use polkadot_sdk::{sp_core::Blake2Hasher, sp_trie::recorder::Recorder};
  use std::collections::BTreeSet;

  fn source(case: u8) -> (ActorRef, ActorContractOf<Test>) {
    set_missing_behavior_inspection_weight(false);
    set_missing_behavior_control_weight(false);
    set_missing_behavior_close_weight(false);
    let steps: crate::ContractSteps<Test> = vec![
      make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Fixed(1),
      });
      5
    ]
    .try_into()
    .unwrap();
    let (actor, contract) = if case == 0 {
      System::set_block_number(1);
      prefund_active_user_creation(ALICE, &steps);
      let id = Actors::next_actor_id();
      let contract = user_active_contract(manual_schedule(), None, steps).unwrap();
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        Some(contract.clone()),
      ));
      (Actors::load_actor_ref(id).unwrap(), contract)
    } else {
      let expression = cleanup_policy_expression(case);
      prepared_policy_source_with_readiness(steps, expression, ActorType::User, false)
    };
    let payer = sovereign_account(actor.actor_id);
    <Test as crate::Config>::AssetOps::mint(&payer, TestAsset::Native, 1_000_000_000_000).unwrap();
    // P1 would close on a first success; other sources preserve a genuine committed Transfer.
    if case != 1 {
      System::set_block_number(System::block_number() + 1);
      assert_ok!(Actors::manual_trigger(
        RuntimeOrigin::signed(ALICE),
        actor.actor_id
      ));
      let recipient_before = native_balance(&BOB);
      let now = System::block_number() + 1;
      System::set_block_number(now);
      let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
      assert_eq!(
        Actors::service_canonical_round_head(&mut meter, now),
        Ok(crate::ServiceRoundEncounter::Eligible(actor))
      );
      assert_eq!(Actors::actor_run_state(actor.actor_id).unwrap().cursor, 1);
      assert_eq!(native_balance(&BOB), recipient_before + 1);
    }
    set_missing_behavior_inspection_weight(true);
    set_missing_behavior_control_weight(true);
    set_missing_behavior_close_weight(true);
    (actor, contract)
  }
  let root = || polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
  for case in 0..=4 {
    let mut ext = new_test_ext();
    let (actor, contract) = ext.execute_with(|| source(case));
    let id = actor.actor_id;
    let payer = ext.execute_with(|| sovereign_account(id));
    let snapshot = || {
      (
        crate::ActorSemanticStates::<Test>::get(id),
        crate::ActorProcesses::<Test>::get(id),
        crate::ActorContractHeads::<Test>::get(id),
        crate::ActorContractTailChunks::<Test>::iter_prefix(id).collect::<Vec<_>>(),
        crate::ActorRunStateStore::<Test>::get(id),
        crate::ActorFinancialWaits::<Test>::get(id),
        crate::ActorStateHolds::<Test>::get(id),
        crate::OwnerSlotBitmaps::<Test>::get(ALICE),
        native_balance(&payer),
        native_balance(&BOB),
        native_balance(&TestFeeSink::get()),
      )
    };
    let (monitor, collateral, expected) = ext.execute_with(|| {
      let policy = crate::ActorBehaviorPolicies::<Test>::get(id);
      let monitor = crate::ActorBehaviorMonitors::<Test>::contains_key(id);
      assert_eq!(monitor, case >= 2);
      if let Some(policy) = policy {
        let context = Actors::behavior_control_weight_context(&policy).unwrap();
        if case == 2 {
          assert_eq!(context.counter_slots, 0);
          assert!(context.requires_clean_cycle);
        }
        if case == 4 {
          assert_eq!(
            policy.encoded_size(),
            crate::ActorBehaviorPolicyOf::<Test>::max_encoded_len()
          );
          assert_eq!(
            crate::ActorBehaviorMonitors::<Test>::get(id)
              .unwrap()
              .encoded_size(),
            crate::ActorBehaviorMonitor::max_encoded_len()
          );
        }
      }
      let collateral = snapshot();
      assert!(!collateral.3.is_empty(), "ordinary tails must be present");
      let expected = with_transaction(|| {
        if case != 0 {
          crate::ActorBehaviorPolicies::<Test>::remove(id);
        }
        if monitor {
          crate::ActorBehaviorMonitors::<Test>::remove(id);
        }
        TransactionOutcome::Rollback(Ok::<_, DispatchError>(root()))
      })
      .unwrap();
      (monitor, collateral, expected)
    });
    ext.execute_with(|| {
      let before = root();
      with_transaction(|| {
        assert_eq!(
          Actors::benchmark_remove_behavior_state(id, case != 0),
          Some(())
        );
        assert_eq!(root(), expected);
        assert_eq!(snapshot(), collateral);
        TransactionOutcome::Rollback(Ok::<_, DispatchError>(()))
      })
      .unwrap();
      assert_eq!(root(), before);
      // The production geometry wrapper retains the same enclosing late-refusal rollback.
      with_transaction(|| {
        assert!(Actors::benchmark_remove_loaded_contract_geometry(id, contract.clone()).is_some());
        assert!(!crate::ActorContractHeads::<Test>::contains_key(id));
        assert!(
          crate::ActorContractTailChunks::<Test>::iter_prefix(id)
            .next()
            .is_none()
        );
        TransactionOutcome::Rollback(Ok::<_, DispatchError>(()))
      })
      .unwrap();
      assert_eq!(root(), before);
      if case != 0 {
        with_transaction(|| {
          crate::ActorBehaviorPolicies::<Test>::remove(id);
          let missing = root();
          assert_eq!(Actors::benchmark_remove_behavior_state(id, true), None);
          assert!(
            Actors::benchmark_remove_loaded_contract_geometry(id, contract.clone()).is_none()
          );
          assert_eq!(
            root(),
            missing,
            "missing policy refuses before even the ordinary head write"
          );
          TransactionOutcome::Rollback(Ok::<_, DispatchError>(()))
        })
        .unwrap();
        assert_eq!(root(), before);
      }
    });
    ext.commit_all().unwrap();
    let recorder = Recorder::<Blake2Hasher>::default();
    // Only the fragment executes under recording. Oracles and state/root scans stay outside it.
    assert_eq!(
      ext.execute_with_recorder(
        recorder.clone(),
        || Actors::benchmark_remove_behavior_state(id, case != 0)
      ),
      Some(())
    );
    let reads = recorder
      .recorded_keys()
      .into_values()
      .flat_map(|keys| keys.into_keys())
      .map(|key| key.as_ref().to_vec())
      .collect::<BTreeSet<_>>();
    let policy_key = crate::ActorBehaviorPolicies::<Test>::hashed_key_for(id);
    let monitor_key = crate::ActorBehaviorMonitors::<Test>::hashed_key_for(id);
    let expected_reads = if case == 0 {
      BTreeSet::new()
    } else {
      BTreeSet::from([policy_key.clone()])
    };
    assert_eq!(reads, expected_reads);
    let writes = ext
      .overlayed_changes()
      .changes()
      .map(|(key, _)| key.clone())
      .collect::<BTreeSet<_>>();
    let mut expected_writes = expected_reads;
    if monitor {
      expected_writes.insert(monitor_key);
    }
    assert_eq!(writes, expected_writes);
    ext.execute_with(|| {
      // The independent oracle ran at FRAME depth one; compare at that same depth.
      with_transaction(|| {
        assert_eq!(root(), expected);
        TransactionOutcome::Rollback(Ok::<_, DispatchError>(()))
      })
      .unwrap();
      assert_eq!(snapshot(), collateral);
    });
  }
  set_missing_behavior_inspection_weight(false);
  set_missing_behavior_control_weight(false);
  set_missing_behavior_close_weight(false);
}

// The oracle recomputes uncapped histories; only this bridge reads the production projection.
#[test]
fn behavior_runtime_suspended_service_fee_closes_before_another_attempt() {
  use polkadot_sdk::frame_support::traits::fungible::InspectHold;
  use polkadot_sdk::sp_runtime::traits::Zero;
  for rollback in [false, true] {
    new_test_ext().execute_with(|| {
      let first = make_step(Task::Transfer {
        to: BOB,
        asset: TestAsset::Native,
        amount: AmountResolution::Percent(Perbill::one()),
      });
      let mut next = make_step(Task::Transfer {
        to: CHARLIE,
        asset: TestAsset::Local(1),
        amount: AmountResolution::Fixed(1),
      });
      next.on_error = StepErrorPolicy::RetryLater {
        max_attempts: <Test as crate::Config>::MaxRetryAttempts::get(),
      };
      let mut steps = vec![first, next.clone()];
      steps.resize(
        <<Test as crate::Config>::MaxContractSteps as Get<u32>>::get() as usize,
        next,
      );
      let (actor, contract) = prepared_policy_source_with_readiness(
        steps.try_into().unwrap(),
        single_metric(crate::BehaviorMetric::ActionSuccessTotal, 2),
        ActorType::User,
        false,
      );
      let id = actor.actor_id;
      let payer = sovereign_account(id);
      <Test as crate::Config>::AssetOps::mint(&payer, TestAsset::Native, 1_000_000_000_000)
        .unwrap();
      <Test as crate::Config>::AssetOps::mint(&payer, TestAsset::Local(1), 100).unwrap();
      assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
      for expected in [CycleState::Running, CycleState::Suspended] {
        let now = System::block_number() + 1;
        System::set_block_number(now);
        let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
        assert_eq!(
          Actors::service_canonical_round_head(&mut meter, now),
          Ok(crate::ServiceRoundEncounter::Eligible(actor))
        );
        assert_eq!(Actors::actor_hot(id).unwrap().cycle_state, expected);
        assert!(!meter.consumed().is_zero());
      }
      let run = Actors::actor_run_state(id).unwrap();
      assert_eq!(run.cursor, 1);
      assert_eq!(run.unsuccessful_attempts_at_cursor, 1);
      assert_eq!(run.cumulative_outcomes.committed_effectful_tasks, 1);
      assert_eq!(
        run.last_step_outcome,
        Some(crate::StepOutcome::FundingUnavailable(
          crate::FundingUnavailableReason::ServiceFee
        ))
      );
      assert_eq!(
        asset_balance(&payer, TestAsset::Native),
        TestMinUserBalance::get()
      );
      assert_eq!(asset_balance(&payer, TestAsset::Local(1)), 100);
      let now = run.eligible_at;
      assert_eq!(now, System::block_number() + 1);
      System::set_block_number(now);
      let (bounded, _, _) = Actors::load_actor_service_state_with_authority(id).unwrap();
      assert_ne!(bounded.contract.steps, contract.steps);
      assert_eq!(bounded.contract.steps[0], contract.steps[1]);
      let before = crate::ActorSemanticStates::<Test>::get(id);
      let hold = crate::ActorStateHolds::<Test>::get(id).unwrap();
      assert_eq!(hold.owner, ALICE);
      let hold_reason: RuntimeHoldReason = crate::HoldReason::ActorState.into();
      let held = Balances::balance_on_hold(&hold_reason, &ALICE);
      let free = Balances::free_balance(ALICE);
      assert!(held > 0);
      let slot = crate::OwnerSlotBitmaps::<Test>::get(ALICE);
      let prefix_credit = asset_balance(&BOB, TestAsset::Native);
      let recipient = asset_balance(&CHARLIE, TestAsset::Local(1));
      let sink =
        <Test as crate::Config>::AssetOps::total_balance(&TestFeeSink::get(), TestAsset::Native);
      let events = System::events();
      let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      for missing in 0..3 {
        set_missing_behavior_inspection_weight(missing == 0);
        set_missing_behavior_control_weight(missing == 1);
        set_missing_behavior_close_weight(missing == 2);
        let mut refused = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
        assert!(Actors::service_canonical_round_head(&mut refused, now).is_err());
        assert!(!refused.consumed().is_zero());
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
          root
        );
      }
      set_missing_behavior_inspection_weight(false);
      set_missing_behavior_control_weight(false);
      set_missing_behavior_close_weight(false);
      let mut meter = polkadot_sdk::sp_weights::WeightMeter::with_limit(Weight::MAX);
      let result = if rollback {
        polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
          let result = Actors::service_canonical_round_head(&mut meter, now);
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(result)
        })
      } else {
        Actors::service_canonical_round_head(&mut meter, now)
      };
      assert_eq!(
        result,
        Ok(crate::ServiceRoundEncounter::TerminallyClosed(
          CloseReason::CycleAdmissionInsufficient
        ))
      );
      assert!(!meter.consumed().is_zero());
      assert_eq!(
        asset_balance(&payer, TestAsset::Native),
        TestMinUserBalance::get()
      );
      assert_eq!(asset_balance(&payer, TestAsset::Local(1)), 100);
      assert_eq!(asset_balance(&BOB, TestAsset::Native), prefix_credit);
      assert_eq!(asset_balance(&CHARLIE, TestAsset::Local(1)), recipient);
      assert_eq!(
        <Test as crate::Config>::AssetOps::total_balance(&TestFeeSink::get(), TestAsset::Native),
        sink
      );
      assert!(!crate::ActorFinancialWaits::<Test>::contains_key(id));
      if rollback {
        assert_eq!(crate::ActorSemanticStates::<Test>::get(id), before);
        assert_eq!(Actors::actor_run_state(id), Some(run));
        assert_eq!(crate::ActorStateHolds::<Test>::get(id), Some(hold));
        assert_eq!(crate::OwnerSlotBitmaps::<Test>::get(ALICE), slot);
        assert_eq!(Actors::load_actor_contract(id), Some(contract));
        assert_eq!(Balances::balance_on_hold(&hold_reason, &ALICE), held);
        assert_eq!(Balances::free_balance(ALICE), free);
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
          root
        );
      } else {
        assert!(!crate::ActorSemanticStates::<Test>::contains_key(id));
        assert!(!crate::ActorRunStateStore::<Test>::contains_key(id));
        assert!(!crate::ActorStateHolds::<Test>::contains_key(id));
        assert!(!crate::OwnerSlotBitmaps::<Test>::contains_key(ALICE));
        assert!(!crate::ServiceNodes::<Test>::contains_key(id));
        assert!(!crate::DeadlineHandles::<Test>::contains_key(id));
        assert_eq!(Balances::balance_on_hold(&hold_reason, &ALICE), 0);
        assert_eq!(Balances::free_balance(ALICE), free + held);
        let mut closes = 0;
        for record in System::events().iter().skip(events.len()) {
          match record.event {
            RuntimeEvent::Actors(Event::ActorClosed { actor_id, reason }) if actor_id == id => {
              assert_eq!(reason, CloseReason::CycleAdmissionInsufficient);
              closes += 1;
            }
            RuntimeEvent::Actors(
              Event::ActionFeeCharged { actor_id, .. }
              | Event::PipelineFeeCharged { actor_id, .. }
              | Event::CycleStarted { actor_id, .. }
              | Event::StepFailed { actor_id, .. }
              | Event::StepSkipped { actor_id, .. },
            ) if actor_id == id => panic!("no new attempt or payment"),
            _ => {}
          }
        }
        assert_eq!(closes, 1);
      }
    });
  }
}

fn assert_runtime_policy_trace(
  actor: ActorRef,
  steps: &[super::behavior_reference::StepKind],
  trace: &[super::behavior_reference::Record],
  expression: &crate::CloseWhenOf<Test>,
) -> bool {
  use super::{behavior_policy, behavior_reference as oracle};
  let expected = oracle::project(
    steps,
    trace,
    Some(&behavior_policy::oracle_policy(expression)),
  );
  assert_eq!(
    Actors::actor_identity(actor.actor_id).is_none(),
    expected.closed
  );
  if expected.closed {
    assert!(!crate::ActorBehaviorPolicies::<Test>::contains_key(
      actor.actor_id
    ));
    assert!(!crate::ActorBehaviorMonitors::<Test>::contains_key(
      actor.actor_id
    ));
    assert!(!crate::ActorRunStateStore::<Test>::contains_key(
      actor.actor_id
    ));
    assert_eq!(
      System::events()
        .iter()
        .filter(|record| matches!(
          record.event,
          RuntimeEvent::Actors(Event::ActorClosed {
            actor_id,
            reason: CloseReason::BehaviorPolicySatisfied,
            ..
          }) if actor_id == actor.actor_id
        ))
        .count(),
      1
    );
    return true;
  }
  let policy = crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id).unwrap();
  let monitor = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id);
  assert_eq!(
    monitor.is_some(),
    policy.plan.mode() == crate::BehaviorTrackingMode::Accumulated
      || policy.plan.requires_clean_cycle()
  );
  let authored_metrics: BTreeSet<_> = expression
    .clauses
    .iter()
    .flatten()
    .map(|atom| atom.metric)
    .collect();
  assert_eq!(
    policy
      .plan
      .metrics()
      .iter()
      .map(|slot| slot.metric)
      .collect::<BTreeSet<_>>(),
    authored_metrics
  );
  let Some(monitor) = monitor else {
    return false;
  };
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
  for slot in policy.plan.metrics() {
    let metric = slot.metric;
    if policy.plan.mode() == crate::BehaviorTrackingMode::CurrentOutcome {
      assert_eq!(bound.value(&monitor.state, episode, metric), Ok(None));
      continue;
    }
    let authored_cap = expression
      .clauses
      .iter()
      .flatten()
      .filter(|atom| atom.metric == metric)
      .map(|atom| u64::from(atom.threshold))
      .max()
      .unwrap();
    assert_eq!(
      bound.value(&monitor.state, episode, metric),
      Ok(Some(
        u32::try_from(
          expected
            .value(behavior_policy::oracle_metric(metric))
            .min(authored_cap)
        )
        .unwrap()
      )),
      "{metric:?}"
    );
  }
  false
}

#[test]
fn behavior_runtime_installation_requires_complete_lifecycle_authority_before_publication() {
  for actor_type in [ActorType::User, ActorType::System] {
    // Success, missing Close owner, independent lifecycle dimensions, and late caller rollback.
    for refusal in 0..5 {
      new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let actor_id = Actors::next_actor_id();
        match actor_type {
          ActorType::User => {
            assert_ok!(Actors::create_user_actor(
              RuntimeOrigin::signed(ALICE),
              Mutability::Mutable,
              None,
            ));
          }
          ActorType::System => {
            assert_ok!(Actors::create_system_actor(
              RuntimeOrigin::root(),
              ALICE,
              Mutability::Mutable,
              None,
            ));
          }
        }
        let Some(crate::ActorSemanticState::Dormant(record)) =
          crate::ActorSemanticStates::<Test>::get(actor_id)
        else {
          panic!("Dormant source");
        };
        let mut contract = user_active_contract(
          manual_schedule(),
          None,
          vec![transfer_step(None), transfer_step(None)]
            .try_into()
            .unwrap(),
        )
        .unwrap();
        contract.close_when = Some(super::behavior_policy::expression(vec![vec![
          crate::BehaviorAtom {
            metric: crate::BehaviorMetric::ActionSuccessTotal,
            threshold: 1,
          },
          crate::BehaviorAtom {
            metric: crate::BehaviorMetric::CycleCompletedTotal,
            threshold: 1,
          },
        ]]));
        let cleanup = Actors::close_control_weight_for_contract(&contract).unwrap();
        assert!(cleanup.ref_time() > 0 && cleanup.proof_size() > 0);
        let mut authority =
          <Test as crate::Config>::AdmissionCertificateAuthority::current().unwrap();
        authority.maximum_lifecycle_weight = match refusal {
          2 => Weight::from_parts(cleanup.ref_time() - 1, cleanup.proof_size()),
          3 => Weight::from_parts(cleanup.ref_time(), cleanup.proof_size() - 1),
          _ => cleanup,
        };
        set_admission_authority_override(Some(authority));
        if refusal == 0 {
          assert_noop!(
            Actors::activate_actor(RuntimeOrigin::signed(ALICE), actor_id, contract.clone()),
            Error::<Test>::BehaviorPolicyUnavailable
          );
          assert!(Actors::build_admission_certificate(&contract).is_none());
        }
        let admission = ActorAdmissionCertificateOf::<Test>::new(
          contract.semantic_contract_id(),
          contract.body_commitment().unwrap(),
          contract.trigger.wake_qualification(&contract.window),
          authority.runtime_actor_semantics_version,
          authority.production_weight_identity,
          authority.body_geometry_version,
          authority.configured_bounds_commitment,
          authority.maximum_lifecycle_weight,
        );
        let hot = crate::ActorHotState {
          lifecycle: ActiveLifecycle::Active,
          cycle_state: CycleState::Idle,
          trigger_runtime_state: crate::TriggerRuntimeState::Stateless,
          unsuccessful_attempt_streak: 0,
          pending_signal: false,
          queue_ticket: None,
          wakeup_pointer: None,
          trigger_wakeup_pointer: None,
          terminal_at: None,
          schedule_anchor: 1,
          last_cycle_block: None,
        };
        let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        set_missing_behavior_close_weight(refusal == 1);
        let result = polkadot_sdk::frame_support::storage::with_transaction(|| {
          let before_installation =
            polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
          let result = (|| {
            Actors::insert_active_actor_with_admission(
              actor_id,
              record.identity.clone(),
              hot,
              contract.clone(),
              crate::TriggerTransitionIntent::ActivateDormant,
              admission,
            )?;
            crate::ActiveActorCount::<Test>::mutate(|count| *count += 1);
            Actors::reconcile_actor_state_hold_with_authority(actor_id)?;
            if refusal == 4 {
              return Err(polkadot_sdk::sp_runtime::DispatchError::Other(
                "late policy installation refusal",
              ));
            }
            Ok(())
          })();
          match result {
            Ok(()) => polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Ok(())),
            Err(error) => {
              if (1..4).contains(&refusal) {
                assert_eq!(
                  polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
                  before_installation,
                  "resource refusal must precede publication, not rely on caller rollback"
                );
              }
              polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Err(error))
            }
          }
        });
        if refusal != 0 {
          if refusal < 4 {
            assert_eq!(result, Err(Error::<Test>::BehaviorPolicyUnavailable.into()));
          } else {
            assert!(result.is_err());
          }
          assert_eq!(
            polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
            before
          );
          return;
        }
        assert_ok!(result);
        assert_eq!(Actors::load_actor_contract(actor_id), Some(contract));
        assert!(crate::ActorBehaviorPolicies::<Test>::contains_key(actor_id));
        assert!(crate::ActorBehaviorMonitors::<Test>::contains_key(actor_id));
        let sovereign = sovereign_account(actor_id);
        fund_native(actor_id, 10_000);
        let bob = native_balance(&BOB);
        assert_ok!(Actors::manual_trigger(
          RuntimeOrigin::signed(ALICE),
          actor_id
        ));
        let sink = native_balance(&TestFeeSink::get());
        let custody = native_balance(&sovereign);
        let pipeline = crate::ActorContractHeads::<Test>::get(actor_id)
          .unwrap()
          .header
          .pipeline_machine_envelope
          .pipeline_machine_fee_upper;
        let action_fee = if actor_type == ActorType::User {
          100
        } else {
          0
        };
        run_canonical_round_at(2, Weight::MAX);
        assert_eq!(Actors::actor_run_state(actor_id).unwrap().cursor, 1);
        assert_eq!(native_balance(&BOB), bob + 10);
        run_canonical_round_at(3, Weight::MAX);
        assert!(Actors::actor_identity(actor_id).is_none());
        assert!(!crate::ActorBehaviorPolicies::<Test>::contains_key(
          actor_id
        ));
        assert!(!crate::ActorBehaviorMonitors::<Test>::contains_key(
          actor_id
        ));
        assert_eq!(native_balance(&BOB), bob + 20);
        assert_eq!(
          native_balance(&sovereign),
          custody - 20 - pipeline - 2 * action_fee
        );
        assert_eq!(
          native_balance(&TestFeeSink::get()),
          sink + pipeline + 2 * action_fee
        );
      });
    }
  }
}

#[test]
fn behavior_runtime_scalar_terminals_match_oracle_in_both_tracking_modes() {
  use super::{behavior_policy, behavior_reference as oracle};
  use crate::{BehaviorMetric::*, TaskEffectWeightProvider};
  let mut stop_refusals = [[0u32; 3]; 2];
  for metric in [
    PreconditionPassTotal,
    PreconditionPassStreak,
    PreconditionRejectTotal,
    PreconditionRejectStreak,
    ActionSuccessTotal,
    ActionSuccessStreak,
    ActionFailureTotal,
    ActionFailureStreak,
    CycleCompletedTotal,
    CycleCleanTotal,
  ] {
    for threshold in [1, 2] {
      for actor_type in [ActorType::User, ActorType::System] {
        new_test_ext().execute_with(|| {
          set_max_consecutive_failures(100);
          let expression = single_metric(metric, threshold);
          let conditional = || Precondition {
            clauses: vec![
              vec![Predicate::BalanceAbove {
                asset: TestAsset::Native,
                threshold: 20_000,
              }]
              .try_into()
              .unwrap(),
            ]
            .try_into()
            .unwrap(),
          };
          let cycle_metric = matches!(metric, CycleCompletedTotal | CycleCleanTotal);
          let steps = if cycle_metric {
            vec![
              transfer_step(Some(conditional())),
              Step {
                precondition: Some(conditional()),
                task: Task::StopCycle,
                on_error: StepErrorPolicy::ContinueNextStep,
              },
              transfer_step(None),
            ]
          } else {
            vec![transfer_step(Some(conditional())), transfer_step(None)]
          };
          let kinds = if cycle_metric {
            vec![
              oracle::StepKind::Economic,
              oracle::StepKind::StopCycle,
              oracle::StepKind::Economic,
            ]
          } else {
            vec![oracle::StepKind::Economic; 2]
          };
          let (actor, _) = prepared_policy_source_for_class(
            steps.try_into().unwrap(),
            expression.clone(),
            actor_type,
          );
          let policy = crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id).unwrap();
          assert_eq!(
            policy.plan.mode(),
            if threshold == 1 {
              crate::BehaviorTrackingMode::CurrentOutcome
            } else {
              crate::BehaviorTrackingMode::Accumulated
            }
          );
          let sovereign = sovereign_account(actor.actor_id);
          let machine_fee = crate::ActorContractHeads::<Test>::get(actor.actor_id)
            .unwrap()
            .header
            .pipeline_machine_envelope
            .pipeline_machine_fee_upper;
          let action_fee =
            <Test as crate::Config>::WeightToFee::weight_to_fee(&Weight::from_parts(1, 0));
          let oracle_policy = behavior_policy::oracle_policy(&expression);
          let mut trace = vec![];
          let mut cursor = 0;
          let mut cycle = 0;
          let mut opening = true;
          let mut now = 2;
          let mut closed = false;
          let mut transfers = 0;
          let bob_initial = native_balance(&BOB);
          for _ in 0..20 {
            let economic = kinds[cursor] == oracle::StepKind::Economic;
            let has_check = cursor == 0 || cycle_metric && cursor == 1;
            let positive = cycle == 0 || cycle >= 2;
            let passes = if !has_check {
              true
            } else {
              match metric {
                PreconditionPassTotal | PreconditionPassStreak => positive,
                PreconditionRejectTotal | PreconditionRejectStreak => !positive,
                ActionSuccessTotal | ActionSuccessStreak | ActionFailureTotal
                | ActionFailureStreak => false,
                CycleCompletedTotal | CycleCleanTotal if cursor == 0 => cycle != 1,
                CycleCompletedTotal | CycleCleanTotal => cycle == 0,
              }
            };
            let fails = economic
              && passes
              && match metric {
                ActionSuccessTotal | ActionSuccessStreak => !positive,
                ActionFailureTotal | ActionFailureStreak => positive,
                CycleCleanTotal => cycle == 2 && cursor == 2,
                _ => false,
              };
            if cycle_metric && cycle == 0 {
              // Preserve the paid prefix for actual-settlement probes at the returning StopCycle.
              if opening {
                let shortfall = 30_000u128.saturating_sub(native_balance(&sovereign));
                if shortfall > 0 {
                  fund_native(actor.actor_id, shortfall);
                }
              }
              assert!(passes && native_balance(&sovereign) > 20_000);
            } else {
              assert_ok!(Balances::force_set_balance(
                RuntimeOrigin::root(),
                sovereign,
                if passes { 30_000 } else { 10_000 }
              ));
            }
            set_fail_transfer_to(fails.then_some(BOB));
            if opening && !trace.is_empty() {
              let sink = native_balance(&TestFeeSink::get());
              let custody = native_balance(&sovereign);
              assert_ok!(Actors::manual_trigger(
                RuntimeOrigin::signed(ALICE),
                actor.actor_id
              ));
              let fee = if actor_type == ActorType::User {
                manual_trigger_fee()
              } else {
                0
              };
              assert_eq!(native_balance(&TestFeeSink::get()), sink + fee);
              assert_eq!(native_balance(&sovereign), custody - fee);
            }
            let returns = !economic && passes;
            let attempt = oracle::Transition::Attempt {
              check: if !has_check {
                oracle::Check::Absent
              } else if passes {
                oracle::Check::Pass
              } else {
                oracle::Check::Reject
              },
              action: if !economic || !passes {
                oracle::Action::NotInvoked
              } else if fails {
                oracle::Action::Failure
              } else {
                oracle::Action::Success
              },
              disposition: if returns {
                oracle::Disposition::Return
              } else {
                oracle::Disposition::Advance
              },
            };
            let mut next_trace = trace.clone();
            if opening {
              next_trace.push(oracle::Record::committed(oracle::Transition::Open));
            }
            next_trace.push(oracle::Record::committed(attempt.clone()));
            let should_close = oracle::project(&kinds, &next_trace, Some(&oracle_policy)).closed;
            if should_close || returns {
              System::set_block_number(now);
              begin_prepared_round(now);
              let before =
                polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
              let refusals = if returns {
                let admission = Actors::load_control_admission(actor.actor_id).unwrap();
                let maximum = Actors::load_current_step_with_admission(
                  actor.actor_id,
                  cursor as u32,
                  &admission,
                )
                .unwrap()
                .resources
                .control;
                let run = Actors::actor_run_state(actor.actor_id).unwrap();
                assert_eq!(run.cursor, 1);
                assert_eq!(run.cumulative_outcomes.committed_effectful_tasks, 1);
                assert_eq!(run.last_committed_step_block.unwrap() + 1, now);
                vec![
                  None,
                  Some(Weight::from_parts(
                    maximum.ref_time().checked_add(1).unwrap(),
                    maximum.proof_size(),
                  )),
                  Some(Weight::from_parts(
                    maximum.ref_time(),
                    maximum.proof_size().checked_add(1).unwrap(),
                  )),
                ]
              } else {
                vec![Some(Weight::MAX)]
              };
              for (refusal_index, actual) in refusals.into_iter().enumerate() {
                set_missing_step_control_actual_weight(actual.is_none());
                set_step_control_actual_weight_override(actual);
                let result = execute_prepared_step(actor, now);
                set_missing_step_control_actual_weight(false);
                set_step_control_actual_weight_override(None);
                let Err(error) = result else {
                  panic!("late actual-control refusal must roll back: {actual:?}");
                };
                assert_eq!(
                  error.cause,
                  crate::scheduler::AttemptTransactionError::Invariant
                );
                if returns {
                  stop_refusals[usize::from(should_close)][refusal_index] += 1;
                  let execution = last_step_control_execution().unwrap();
                  assert_eq!(execution.phase, crate::StepControlPhase::Running);
                  assert_eq!(execution.outcome, crate::StepControlOutcome::Completed);
                  assert_eq!(execution.task_effect, crate::TaskEffectExecution::Invoked);
                  assert!(!execution.action_fee_collected);
                  let effect = <Test as crate::Config>::TaskEffectWeight::actual_effect_weight(
                    &Task::StopCycle,
                    crate::TaskEffectExecution::Invoked,
                  )
                  .unwrap();
                  assert_eq!(error.actual_effect_weight, Some(effect));
                }
                assert_eq!(
                  polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
                  before
                );
                trace.push(oracle::Record::rolled_back(attempt.clone()));
                assert!(!assert_runtime_policy_trace(
                  actor,
                  &kinds,
                  &trace,
                  &expression
                ));
              }
              now += 1;
            }
            let custody = native_balance(&sovereign);
            let bob = native_balance(&BOB);
            let sink = native_balance(&TestFeeSink::get());
            run_canonical_round_at(now, Weight::MAX);
            let transferred = if economic && passes && !fails { 10 } else { 0 };
            transfers += transferred;
            let fee = if actor_type == ActorType::User {
              (if opening { machine_fee } else { 0 })
                + if economic && passes { action_fee } else { 0 }
            } else {
              0
            };
            assert_eq!(native_balance(&sovereign), custody - transferred - fee);
            assert_eq!(native_balance(&BOB), bob + transferred);
            assert_eq!(native_balance(&TestFeeSink::get()), sink + fee);
            trace = next_trace;
            closed = assert_runtime_policy_trace(actor, &kinds, &trace, &expression);
            assert_eq!(closed, should_close);
            if closed {
              break;
            }
            let ends_cycle = returns || cursor + 1 == kinds.len();
            if ends_cycle {
              cycle += 1;
              cursor = 0;
            } else {
              cursor += 1;
            }
            let state = Actors::active_actor_state(actor.actor_id).unwrap();
            assert_eq!(state.identity.cycle_nonce, cycle);
            assert_eq!(
              state.hot.cycle_state,
              if ends_cycle {
                CycleState::Idle
              } else {
                CycleState::Running
              }
            );
            if !ends_cycle {
              assert_eq!(state.run_state.as_ref().unwrap().cursor as usize, cursor);
            }
            opening = ends_cycle;
            now += 1;
          }
          assert!(closed, "{metric:?}, threshold={threshold}, {actor_type:?}");
          assert_eq!(native_balance(&BOB), bob_initial + transfers);
          set_fail_transfer_to(None);
        });
      }
    }
  }
  // Early StopCycle increments completion, not plan-end-only CycleClean: only the
  // threshold-one CycleCompleted policy closes here; the other three policies retain Idle.
  assert_eq!(stop_refusals, [[6, 6, 6], [2, 2, 2]]);
}

#[test]
fn behavior_runtime_seeded_native_traces_match_independent_oracle_and_payment() {
  use super::{behavior_policy, behavior_reference as oracle};
  use proptest::{
    prelude::*,
    test_runner::{Config as PropertyConfig, RngSeed, TestRunner},
  };
  let strategy = (
    proptest::collection::vec(0u8..8, 1..=4),
    proptest::collection::vec(any::<u8>(), 16..=48),
    2u32..=5,
    2u32..=4,
    1u32..=3,
  );
  let mut runner = TestRunner::new(PropertyConfig {
    cases: 32,
    rng_seed: RngSeed::Fixed(0xde05_b027),
    failure_persistence: None,
    ..PropertyConfig::default()
  });
  runner
    .run(
      &strategy,
      |(shapes, decisions, success_total, success_streak, clean_total)| {
        for actor_type in [ActorType::User, ActorType::System] {
          new_test_ext().execute_with(|| {
            // Keep the independent global-failure terminal outside this finite policy trace.
            set_max_consecutive_failures(100);
            use crate::BehaviorMetric::*;
            let clause = |metrics: &[(crate::BehaviorMetric, u32)]| {
              metrics
                .iter()
                .map(|(metric, threshold)| crate::BehaviorAtom {
                  metric: *metric,
                  threshold: *threshold,
                })
                .collect()
            };
            let expression = behavior_policy::expression(vec![
              clause(&[
                (PreconditionPassTotal, 100),
                (PreconditionPassStreak, 100),
                (PreconditionRejectTotal, 100),
                (PreconditionRejectStreak, 100),
              ]),
              clause(&[
                (ActionSuccessTotal, success_total),
                (ActionSuccessStreak, success_streak),
              ]),
              clause(&[
                (ActionFailureTotal, success_total),
                (ActionFailureStreak, success_streak + 1),
              ]),
              clause(&[(CycleCompletedTotal, 3), (CycleCleanTotal, clean_total)]),
            ]);
            let steps: Vec<_> = shapes
              .iter()
              .map(|shape| {
                let precondition = (shape & 2 != 0).then(|| Precondition {
                  clauses: vec![
                    vec![Predicate::BalanceAbove {
                      asset: TestAsset::Native,
                      threshold: 20_000,
                    }]
                    .try_into()
                    .unwrap(),
                  ]
                  .try_into()
                  .unwrap(),
                });
                let mut step = transfer_step(precondition);
                if shape & 1 != 0 {
                  step.task = Task::StopCycle;
                }
                if shape & 4 != 0 {
                  step.on_error = StepErrorPolicy::AbortCycle;
                }
                step
              })
              .collect();
            let kinds: Vec<_> = shapes
              .iter()
              .map(|shape| {
                if shape & 1 != 0 {
                  oracle::StepKind::StopCycle
                } else {
                  oracle::StepKind::Economic
                }
              })
              .collect();
            let (actor, _) = prepared_policy_source_for_class(
              steps.try_into().unwrap(),
              expression.clone(),
              actor_type,
            );
            let sovereign = sovereign_account(actor.actor_id);
            let machine_fee = crate::ActorContractHeads::<Test>::get(actor.actor_id)
              .unwrap()
              .header
              .pipeline_machine_envelope
              .pipeline_machine_fee_upper;
            let action_fee =
              <Test as crate::Config>::WeightToFee::weight_to_fee(&Weight::from_parts(1, 0));
            let mut trace = vec![];
            let mut cursor = 0;
            let mut opening = true;
            let mut cycle_nonce = 0;
            let mut now = 2;
            for decision in &decisions {
              let shape = shapes[cursor];
              let funded = if decision & 1 == 0 { 10_000 } else { 30_000 };
              assert_ok!(Balances::force_set_balance(
                RuntimeOrigin::root(),
                sovereign,
                funded
              ));
              let conditional = shape & 2 != 0;
              let passes = !conditional || decision & 1 != 0;
              let economic = shape & 1 == 0;
              let fails = economic && passes && decision & 2 != 0;
              set_fail_transfer_to(fails.then_some(BOB));
              let returns = passes && !economic;
              let aborts = fails && shape & 4 != 0;
              let disposition = if returns {
                oracle::Disposition::Return
              } else if aborts {
                oracle::Disposition::Abort
              } else {
                oracle::Disposition::Advance
              };
              let attempt = oracle::Transition::Attempt {
                check: if !conditional {
                  oracle::Check::Absent
                } else if passes {
                  oracle::Check::Pass
                } else {
                  oracle::Check::Reject
                },
                action: if !economic || !passes {
                  oracle::Action::NotInvoked
                } else if fails {
                  oracle::Action::Failure
                } else {
                  oracle::Action::Success
                },
                disposition,
              };
              let ends_cycle = returns || aborts || cursor + 1 == shapes.len();
              if opening && !trace.is_empty() {
                let custody = native_balance(&sovereign);
                let sink = native_balance(&TestFeeSink::get());
                assert_ok!(Actors::manual_trigger(
                  RuntimeOrigin::signed(ALICE),
                  actor.actor_id
                ));
                let trigger_fee = if actor_type == ActorType::User {
                  manual_trigger_fee()
                } else {
                  0
                };
                assert_eq!(native_balance(&sovereign), custody - trigger_fee);
                assert_eq!(native_balance(&TestFeeSink::get()), sink + trigger_fee);
              }
              if decision & 8 != 0 {
                System::set_block_number(now);
                begin_prepared_round(now);
                let before =
                  polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
                set_step_control_actual_weight_override(Some(Weight::MAX));
                let Err(error) = execute_prepared_step(actor, now) else {
                  panic!("late control mismatch must roll back");
                };
                assert_eq!(
                  error.cause,
                  crate::scheduler::AttemptTransactionError::Invariant
                );
                set_step_control_actual_weight_override(None);
                assert_eq!(
                  polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
                  before
                );
                trace.push(oracle::Record::rolled_back(attempt.clone()));
                assert!(!assert_runtime_policy_trace(
                  actor,
                  &kinds,
                  &trace,
                  &expression
                ));
                now += 1;
              }
              let custody = native_balance(&sovereign);
              let bob = native_balance(&BOB);
              let sink = native_balance(&TestFeeSink::get());
              run_canonical_round_at(now, Weight::MAX);
              let transferred = if economic && passes && !fails { 10 } else { 0 };
              let charged = if actor_type == ActorType::User {
                (if opening { machine_fee } else { 0 })
                  + if economic && passes { action_fee } else { 0 }
              } else {
                0
              };
              assert_eq!(native_balance(&BOB), bob + transferred);
              assert_eq!(native_balance(&sovereign), custody - transferred - charged);
              assert_eq!(native_balance(&TestFeeSink::get()), sink + charged);
              if opening {
                trace.push(oracle::Record::committed(oracle::Transition::Open));
              }
              trace.push(oracle::Record::committed(attempt));
              if assert_runtime_policy_trace(actor, &kinds, &trace, &expression) {
                break;
              }
              if ends_cycle {
                cycle_nonce += 1;
                cursor = 0;
              } else {
                cursor += 1;
              }
              let state = Actors::active_actor_state(actor.actor_id).unwrap();
              assert_eq!(state.identity.cycle_nonce, cycle_nonce);
              assert_eq!(
                state.hot.cycle_state,
                if ends_cycle {
                  CycleState::Idle
                } else {
                  CycleState::Running
                }
              );
              if !ends_cycle {
                assert_eq!(state.run_state.as_ref().unwrap().cursor as usize, cursor);
              }
              opening = ends_cycle;
              now += 1;
            }
            set_fail_transfer_to(None);
          });
        }
        Ok(())
      },
    )
    .unwrap();
}

#[test]
fn behavior_runtime_reconfiguration_preserves_or_resets_committed_episode() {
  use super::{behavior_policy, behavior_reference as oracle};
  for phase in [CycleState::Idle, CycleState::Running, CycleState::Suspended] {
    for replacement in [false, true] {
      for actor_type in [ActorType::User, ActorType::System] {
        new_test_ext().execute_with(|| {
        let expression = behavior_policy::expression(vec![vec![
          crate::BehaviorAtom { metric: crate::BehaviorMetric::ActionSuccessTotal, threshold: 6 },
          crate::BehaviorAtom { metric: crate::BehaviorMetric::CycleCleanTotal, threshold: 3 },
        ]]);
        let second = if phase == CycleState::Suspended {
          Step { task: Task::AddLiquidity {
            asset_a: TestAsset::Native, asset_b: TestAsset::Local(1),
            amount_a: AmountResolution::Fixed(10), amount_b: AmountResolution::Fixed(10),
            min_lp_out: 1,
          }, precondition: None, on_error: StepErrorPolicy::RetryLater { max_attempts: 3 } }
        } else { transfer_step(None) };
        let (actor, mut contract) = prepared_policy_source_for_class(
          vec![transfer_step(None), second].try_into().unwrap(),
          expression.clone(), actor_type,
        );
        fund_native(actor.actor_id, 10_000);
        let sovereign = sovereign_account(actor.actor_id);
        if phase == CycleState::Suspended {
          assert_ok!(MockAssetOps::mint(&sovereign, TestAsset::Local(1), 100));
        }
        let kinds = [oracle::StepKind::Economic; 2];
        let successful = || oracle::Record::committed(oracle::Transition::Attempt {
          check: oracle::Check::Absent, action: oracle::Action::Success,
          disposition: oracle::Disposition::Advance,
        });
        let mut trace = vec![oracle::Record::committed(oracle::Transition::Open)];
        let bob_initial = native_balance(&BOB);
        run_canonical_round_at(2, Weight::MAX);
        trace.push(successful());
        assert!(!assert_runtime_policy_trace(actor, &kinds, &trace, &expression));
        let run = Actors::actor_run_state(actor.actor_id).unwrap();
        assert_eq!(run.cursor, 1);
        System::set_block_number(3);
        let root = || polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        let before = root();
        assert_ok!(Actors::update_contract(RuntimeOrigin::signed(ALICE), actor.actor_id, contract.clone()));
        assert_eq!(root(), before);
        assert_eq!(Actors::actor_run_state(actor.actor_id), Some(run));
        if phase == CycleState::Idle {
          run_canonical_round_at(3, Weight::MAX);
          trace.push(successful());
        } else if phase == CycleState::Suspended {
          set_temporary_add_liquidity_failure(true);
          run_canonical_round_at(3, Weight::MAX);
          trace.push(oracle::Record::committed(oracle::Transition::Attempt {
            check: oracle::Check::Absent, action: oracle::Action::Failure,
            disposition: oracle::Disposition::Retry,
          }));
          run_canonical_round_at(4, Weight::MAX);
          trace.push(oracle::Record::committed(oracle::Transition::Attempt {
            check: oracle::Check::Absent, action: oracle::Action::Failure,
            disposition: oracle::Disposition::Retry,
          }));
          System::set_block_number(5);
        }
        let state = Actors::active_actor_state(actor.actor_id).unwrap();
        assert_eq!(state.hot.cycle_state, phase);
        let cancellation_state = state.clone();
        let open_run = state.run_state;
        let retry = crate::DeadlineHandles::<Test>::get(actor.actor_id);
        assert_eq!(retry.is_some(), phase == CycleState::Suspended);
        let prefix_credit = if phase == CycleState::Idle { 20 } else { 10 };
        assert!(!assert_runtime_policy_trace(actor, &kinds, &trace, &expression));
        assert_eq!(native_balance(&BOB), bob_initial + prefix_credit);
        let old_policy = crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id).unwrap();
        let old_admission = Actors::load_control_admission(actor.actor_id).unwrap();
        let old_header = crate::ActorContractHeads::<Test>::get(actor.actor_id).unwrap().header;
        let old_source = Actors::load_step_behavior_policy(actor.actor_id, &old_header).unwrap().unwrap();
        let balances = (native_balance(&sovereign), native_balance(&BOB), native_balance(&TestFeeSink::get()));
        let held = Actors::actor_state_hold(actor.actor_id);
        set_admission_semantics_version(2);
        // Exact equality remains a no-op, even when the host authority is newer. It does not
        // silently re-certify an existing Contract or charge another machine activation.
        let before = root();
        assert_ok!(Actors::update_contract(RuntimeOrigin::signed(ALICE), actor.actor_id, contract.clone()));
        assert_eq!(root(), before);
        assert_eq!(Actors::load_control_admission(actor.actor_id), Some(old_admission.clone()));
        assert!(Actors::load_frame_actor_service_state(actor.actor_id).is_none());
        if replacement {
          contract.steps[0].task = Task::Transfer {
            to: BOB, asset: TestAsset::Native, amount: AmountResolution::Fixed(11),
          };
        }
        let authority = <MockAdmissionCertificateAuthority as crate::AdmissionCertificateAuthorityProvider>::current().unwrap();
        let admission = ActorAdmissionCertificateOf::<Test>::new(
          contract.semantic_contract_id(), contract.body_commitment().unwrap(),
          contract.trigger.wake_qualification(&contract.window),
          authority.runtime_actor_semantics_version, authority.production_weight_identity,
          authority.body_geometry_version, authority.configured_bounds_commitment,
          authority.maximum_lifecycle_weight,
        );
        assert_ne!(admission.admission_identity, old_admission.admission_identity);
        let publish = |candidate: &ActorAdmissionCertificateOf<Test>| -> Result<ActorRef, DispatchError> {
          assert_eq!(Actors::cancel_run_internal_loaded(
            actor.actor_id, &cancellation_state.identity, CancellationReason::ContractReplaced, None,
            crate::execution::LoadedCancellationContext::RetainedFrame {
              admission: old_admission.clone(), state: cancellation_state.clone(),
            },
          )?, phase != CycleState::Idle);
          let before_store = root();
          let stored = Actors::store_actor_contract_with_admission(actor.actor_id, contract.clone(), candidate);
          if stored.is_err() {
            assert_eq!(root(), before_store, "lifecycle refusal must precede Contract publication");
          }
          stored?;
          if actor_type == ActorType::User {
            Actors::reconcile_actor_state_hold_with_authority(actor.actor_id)?;
          }
          Ok(Actors::load_actor_ref(actor.actor_id).unwrap())
        };
        let before = root();
        // Self-consistent hashes are not authority: wake semantics and the complete host snapshot
        // must match the supplied Contract and current provider before touching residence.
        for invalid in 0..3 {
          let foreign = ActorAdmissionCertificateOf::<Test>::new(
            contract.semantic_contract_id(), contract.body_commitment().unwrap(),
            if invalid == 0 {
              crate::TriggerOf::<Test>::AtTime { after_ticks: 1 }.wake_qualification(&contract.window)
            } else {
              contract.trigger.wake_qualification(&contract.window)
            },
            authority.runtime_actor_semantics_version + u32::from(invalid == 1),
            authority.production_weight_identity, authority.body_geometry_version,
            authority.configured_bounds_commitment,
            if invalid == 2 { Weight::MAX } else { authority.maximum_lifecycle_weight },
          );
          assert!(foreign.has_valid_identity());
          let refused: Result<ActorRef, DispatchError> = polkadot_sdk::frame_support::storage::with_transaction(|| {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(publish(&foreign))
          });
          assert_eq!(refused, Err(Error::<Test>::ActorInvariant.into()));
          assert_eq!(root(), before);
          assert_eq!(Actors::actor_run_state(actor.actor_id), open_run);
          assert_eq!(crate::DeadlineHandles::<Test>::get(actor.actor_id), retry);
          assert_eq!(Actors::actor_state_hold(actor.actor_id), held);
          assert!(!assert_runtime_policy_trace(actor, &kinds, &trace, &expression));
        }
        let cleanup = Actors::close_control_weight_for_contract(&contract).unwrap();
        for maximum in [
          Weight::from_parts(cleanup.ref_time() - 1, cleanup.proof_size()),
          Weight::from_parts(cleanup.ref_time(), cleanup.proof_size() - 1),
        ] {
          let mut short_authority = authority;
          short_authority.maximum_lifecycle_weight = maximum;
          set_admission_authority_override(Some(short_authority));
          let short = ActorAdmissionCertificateOf::<Test>::new(
            contract.semantic_contract_id(), contract.body_commitment().unwrap(),
            contract.trigger.wake_qualification(&contract.window),
            short_authority.runtime_actor_semantics_version, short_authority.production_weight_identity,
            short_authority.body_geometry_version, short_authority.configured_bounds_commitment,
            maximum,
          );
          let refused: Result<ActorRef, DispatchError> = polkadot_sdk::frame_support::storage::with_transaction(|| {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(publish(&short))
          });
          set_admission_authority_override(None);
          assert_eq!(refused, Err(Error::<Test>::BehaviorPolicyUnavailable.into()));
          assert_eq!(root(), before);
          assert_eq!(Actors::actor_run_state(actor.actor_id), open_run);
          assert_eq!(crate::DeadlineHandles::<Test>::get(actor.actor_id), retry);
          assert_eq!(Actors::actor_state_hold(actor.actor_id), held);
          assert!(!assert_runtime_policy_trace(actor, &kinds, &trace, &expression));
        }
        let rollback: Result<ActorRef, DispatchError> = polkadot_sdk::frame_support::storage::with_transaction(|| {
          let result = publish(&admission);
          if let Ok(next) = &result {
            assert_eq!(next.generation, actor.generation + 1);
          }
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(result)
        });
        assert_ok!(rollback);
        assert_eq!(root(), before);
        assert_eq!(Actors::actor_run_state(actor.actor_id), open_run);
        assert_eq!(crate::DeadlineHandles::<Test>::get(actor.actor_id), retry);
        assert_eq!(Actors::actor_state_hold(actor.actor_id), held);
        assert!(!assert_runtime_policy_trace(actor, &kinds, &trace, &expression));
        // Exercise the real public replacement caller without opening public policy admission.
        // Removing a prepared policy is admissible; the forced outer rollback restores the
        // old Contract, Run, retry residence, monitor, hold, events and committed balances.
        let public_removal: Result<(), DispatchError> = polkadot_sdk::frame_support::storage::with_transaction(|| {
          let mut without_policy = contract.clone();
          without_policy.close_when = None;
          let result = Actors::update_contract(RuntimeOrigin::signed(ALICE), actor.actor_id, without_policy);
          if result.is_ok() {
            assert!(Actors::actor_run_state(actor.actor_id).is_none());
            assert!(!crate::DeadlineHandles::<Test>::contains_key(actor.actor_id));
            assert!(!crate::ActorBehaviorPolicies::<Test>::contains_key(actor.actor_id));
            assert_eq!(Actors::load_actor_ref(actor.actor_id).unwrap().generation, actor.generation + 1);
            assert_eq!((native_balance(&sovereign), native_balance(&BOB), native_balance(&TestFeeSink::get())), balances);
          }
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(result)
        });
        assert_ok!(public_removal);
        assert_eq!(root(), before);
        assert_eq!(Actors::actor_run_state(actor.actor_id), open_run);
        assert_eq!(crate::DeadlineHandles::<Test>::get(actor.actor_id), retry);
        let next = polkadot_sdk::frame_support::storage::with_transaction(|| {
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(publish(&admission))
        }).unwrap();
        assert_eq!(next.generation, actor.generation + 1);
        assert_eq!(Actors::load_actor_contract(actor.actor_id), Some(contract.clone()));
        assert_eq!(Actors::load_control_admission(actor.actor_id), Some(admission.clone()));
        assert!(Actors::actor_run_state(actor.actor_id).is_none());
        assert!(!crate::DeadlineHandles::<Test>::contains_key(actor.actor_id));
        assert_eq!(Actors::actor_identity(actor.actor_id).unwrap().cycle_nonce, 1);
        let cancellations = System::events().into_iter().filter(|record| matches!(
          &record.event, RuntimeEvent::Actors(Event::CycleCancelled {
            actor_id, cycle_nonce: 1, reason: CancellationReason::ContractReplaced,
          }) if *actor_id == actor.actor_id
        )).count();
        let summaries = System::events().into_iter().filter(|record| matches!(
          &record.event, RuntimeEvent::Actors(Event::CycleSummary {
            actor_id, cycle_nonce: 1, result: CycleResult::Cancelled, outcomes,
          }) if *actor_id == actor.actor_id && open_run.as_ref().is_some_and(|run| *outcomes == run.cumulative_outcomes)
        )).count();
        assert_eq!((cancellations, summaries), if phase == CycleState::Idle { (0, 0) } else { (1, 1) });
        assert!(crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id).unwrap().clean_cycle.is_none());
        assert_eq!(Actors::actor_state_hold(actor.actor_id), held);
        assert_eq!((native_balance(&sovereign), native_balance(&BOB), native_balance(&TestFeeSink::get())), balances);
        assert!(!Actors::validate_step_behavior_source(&old_source));
        assert!(Actors::load_canonical_actor_semantic_state(actor).is_err());
        let policy = crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id).unwrap();
        assert_ne!(policy.plan.identity(), old_policy.plan.identity());
        assert_eq!(policy.episode_started_at_generation, if replacement { next.generation } else { old_policy.episode_started_at_generation });
        if phase != CycleState::Idle { trace.push(oracle::Record::committed(oracle::Transition::Cancel)); }
        if replacement { trace.push(oracle::Record::committed(oracle::Transition::Replace(kinds.to_vec()))); }
        else { trace.push(oracle::Record::committed(oracle::Transition::Preserve(oracle::PreservedTransition::Recertification))); }
        assert!(!assert_runtime_policy_trace(next, &kinds, &trace, &expression));
        set_temporary_add_liquidity_failure(false);
        let mut now = System::block_number() + 1;
        let mut recipient_delta = prefix_credit;
        let required_cycles = if replacement || phase != CycleState::Idle { 3 } else { 2 };
        for cycle in 0..required_cycles {
          assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), next.actor_id));
          trace.push(oracle::Record::committed(oracle::Transition::Open));
          for cursor in 0..2 {
            let first_amount = if replacement { 11 } else { 10 };
            recipient_delta += if cursor == 0 { first_amount } else if phase == CycleState::Suspended { 0 } else { 10 };
            run_canonical_round_at(now, Weight::MAX);
            trace.push(successful());
            let closed = assert_runtime_policy_trace(next, &kinds, &trace, &expression);
            assert_eq!(closed, cycle + 1 == required_cycles && cursor == 1);
            assert_eq!(native_balance(&BOB), bob_initial + recipient_delta);
            now += 1;
          }
        }
        assert!(Actors::actor_identity(next.actor_id).is_none());
        #[cfg(feature = "try-runtime")]
        assert_ok!(Actors::do_try_state());
      });
      }
    }
  }
}

fn single_metric(metric: crate::BehaviorMetric, threshold: u32) -> crate::CloseWhenOf<Test> {
  super::behavior_policy::expression(vec![vec![crate::BehaviorAtom { metric, threshold }]])
}

fn begin_prepared_round(now: u64) {
  polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
    polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(Actors::begin_service_round(
      now,
    ))
  })
  .unwrap();
}

fn transfer_step(precondition: Option<crate::PreconditionOf<Test>>) -> StepOf<Test> {
  Step {
    precondition,
    task: Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(10),
    },
    on_error: StepErrorPolicy::ContinueNextStep,
  }
}

fn execute_prepared_step(
  actor: ActorRef,
  now: u64,
) -> Result<crate::scheduler::StepCommitEvidence, crate::scheduler::StepRollback> {
  System::set_block_number(now);
  begin_prepared_round(now);
  let state = Actors::active_actor_state(actor.actor_id).unwrap();
  let admission = Actors::load_control_admission(actor.actor_id).unwrap();
  let cursor = state.run_state.as_ref().map_or(0, |run| run.cursor);
  let head = crate::ActorContractHeads::<Test>::get(actor.actor_id).unwrap();
  let resources = if cursor == 0 {
    head.first_step_resources.unwrap()
  } else {
    let tail_cursor = cursor - 1;
    let chunk = crate::ActorContractTailChunks::<Test>::get(
      actor.actor_id,
      tail_cursor / crate::MAX_STEPS_PER_TAIL_CHUNK,
    )
    .unwrap();
    chunk.step_resources[(tail_cursor % crate::MAX_STEPS_PER_TAIL_CHUNK) as usize]
  };
  let loaded = LoadedActorStep {
    cursor,
    step: state.contract.steps[cursor as usize].clone(),
    resources,
  };
  let fee = Actors::maximum_current_action_fee(
    state.identity.actor_class.actor_type(),
    &loaded.step,
    loaded.resources,
  )
  .unwrap();
  let plan = Actors::build_canonical_current_step_plan(
    actor.actor_id,
    state.identity.clone(),
    state.hot.clone(),
    state.run_state.clone(),
    admission.clone(),
    loaded,
    fee,
  )
  .unwrap();
  let kind = crate::ServiceNodes::<Test>::get(actor.actor_id)
    .unwrap()
    .kind;
  let deadline = if plan
    .loaded_step
    .step
    .on_error
    .retry_max_attempts()
    .is_some()
  {
    let attempts = state
      .run_state
      .as_ref()
      .map_or(0, |run| run.unsuccessful_attempts_at_cursor)
      + 1;
    let due = Actors::suspension_eligible_at(
      state.contract.cooldown_blocks,
      state.contract.window,
      now,
      attempts,
    )
    .unwrap();
    (due != now + 1).then_some(crate::WakeupKey::Block(due))
  } else {
    None
  };
  Actors::execute_effectful_step_on_service_with_deadline(
    actor, kind, state, plan, &admission, now, deadline,
  )
}

#[test]
fn behavior_runtime_missing_close_authority_preserves_opening_and_committed_prefix() {
  for committed_prefix in [false, true] {
    new_test_ext().execute_with(|| {
      let (actor, contract) = prepared_policy_source(
        BoundedVec::try_from(vec![transfer_step(None), transfer_step(None)]).unwrap(),
        single_metric(crate::BehaviorMetric::ActionSuccessTotal, 2),
      );
      let sovereign = sovereign_account(actor.actor_id);
      assert_ok!(MockAssetOps::mint(&sovereign, TestAsset::Native, 100));
      let qualified = Actors::close_control_weight_for_contract(&contract).unwrap();
      assert!(Actors::close_dispatch_weight_upper().all_lte(qualified));
      assert_ne!(qualified, Actors::close_dispatch_weight_upper());
      if committed_prefix {
        execute_prepared_step(actor, 2)
          .unwrap_or_else(|error| panic!("step refused: {:?}", error.cause));
      }
      let now = if committed_prefix { 3 } else { 2 };
      System::set_block_number(now);
      begin_prepared_round(now);
      let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      let bob_before = native_balance(&BOB);
      let state = Actors::active_actor_state(actor.actor_id).unwrap();
      set_missing_behavior_close_weight(true);
      assert!(Actors::close_control_weight_for_contract(&contract).is_none());
      assert_eq!(
        Actors::service_terminal_control_upper(&state, None),
        Weight::MAX
      );
      let Err(error) = execute_prepared_step(actor, now) else {
        panic!("missing policy Close authority must refuse before effects");
      };
      assert_eq!(
        error.cause,
        crate::scheduler::AttemptTransactionError::Invariant
      );
      assert!(error.actual_effect_weight.is_none());
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        before
      );
      assert_eq!(native_balance(&BOB), bob_before);
      set_missing_behavior_close_weight(false);
      let repaired = execute_prepared_step(actor, now)
        .unwrap_or_else(|error| panic!("step refused: {:?}", error.cause));
      assert_eq!(
        repaired.attempt.as_ref().unwrap().is_closed(),
        committed_prefix
      );
      assert_eq!(native_balance(&BOB), bob_before + 10);
    });
  }
}

#[test]
fn behavior_runtime_missing_close_authority_rolls_back_complete_destruction() {
  new_test_ext().execute_with(|| {
    let (actor, _) = prepared_policy_source_for_class(
      BoundedVec::try_from(vec![transfer_step(None), transfer_step(None)]).unwrap(),
      single_metric(crate::BehaviorMetric::ActionSuccessTotal, 2),
      ActorType::User,
    );
    let sovereign = sovereign_account(actor.actor_id);
    fund_native(actor.actor_id, 1_000);
    let state = Actors::active_actor_state(actor.actor_id).unwrap();
    let custody = native_balance(&sovereign);
    let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    set_missing_behavior_close_weight(true);
    assert_noop!(
      Actors::remove_actor_publication_and_finalize(
        actor,
        state.clone(),
        None,
        CloseReason::BehaviorPolicySatisfied,
      ),
      Error::<Test>::BehaviorPolicyUnavailable
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      before
    );
    set_missing_behavior_close_weight(false);
    assert_ok!(Actors::remove_actor_publication_and_finalize(
      actor,
      state,
      None,
      CloseReason::BehaviorPolicySatisfied,
    ));
    assert!(Actors::actor_identity(actor.actor_id).is_none());
    assert!(!crate::ActorBehaviorPolicies::<Test>::contains_key(
      actor.actor_id
    ));
    assert!(!crate::ActorBehaviorMonitors::<Test>::contains_key(
      actor.actor_id
    ));
    assert_eq!(native_balance(&sovereign), custody);
  });
}

#[test]
fn behavior_runtime_missing_close_authority_refuses_zero_step_before_pipeline_fee() {
  new_test_ext().execute_with(|| {
    let (actor, contract) = prepared_policy_source_for_class(
      BoundedVec::default(),
      single_metric(crate::BehaviorMetric::CycleCompletedTotal, 1),
      ActorType::User,
    );
    fund_native(actor.actor_id, 1_000);
    System::set_block_number(2);
    begin_prepared_round(2);
    let state = Actors::active_actor_state(actor.actor_id).unwrap();
    let admission = Actors::load_control_admission(actor.actor_id).unwrap();
    let kind = crate::ServiceNodes::<Test>::get(actor.actor_id)
      .unwrap()
      .kind;
    let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    set_missing_behavior_close_weight(true);
    assert!(Actors::zero_step_resource_envelope(&contract).is_some());
    let priced_head = crate::ActorContractHeads::<Test>::get(actor.actor_id).unwrap();
    let (recomputed, _) = Actors::decompose_admitted_contract_geometry(
      actor.actor_id,
      ActorType::User,
      &contract,
      &admission,
    )
    .expect("Pipeline pricing is independent of destruction authority");
    assert_eq!(
      recomputed.header.pipeline_machine_envelope,
      priced_head.header.pipeline_machine_envelope
    );
    assert!(Actors::close_control_weight_for_contract(&contract).is_none());
    assert!(matches!(
      Actors::execute_zero_step_on_service(actor, kind, state.clone(), &admission, 2),
      Err(crate::scheduler::AttemptTransactionError::Invariant)
    ));
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      before
    );
    set_missing_behavior_close_weight(false);
    let repaired = Actors::execute_zero_step_on_service(actor, kind, state, &admission, 2).unwrap();
    assert!(repaired.attempt.as_ref().unwrap().is_closed());
  });
}

#[test]
fn behavior_runtime_disabled_terminal_simulation_admits_complete_close_and_executes_rollback() {
  use crate::WeightInfo;
  use polkadot_sdk::frame_support::traits::{fungible::MutateHold, tokens::Precision};
  new_test_ext().execute_with(|| {
    let (actor, contract) = prepared_policy_source_with_readiness(
      BoundedVec::try_from(vec![transfer_step(None)]).unwrap(),
      single_metric(crate::BehaviorMetric::ActionSuccessTotal, 2),
      ActorType::User,
      false,
    );
    crate::ActorSemanticStates::<Test>::mutate(actor.actor_id, |state| {
      let Some(crate::ActorSemanticState::Active(record)) = state else {
        panic!("active source");
      };
      record.identity.cycle_nonce = u64::MAX;
    });
    fund_native(actor.actor_id, 1_000);
    assert!(crate::ServiceNodes::<Test>::get(actor.actor_id).is_none());
    let upper = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible())
      .saturating_add(Actors::close_control_weight_for_contract(&contract).unwrap());
    let mut budget = ample_simulation_budget();
    budget.actor_control = upper;
    let simulate = |budget| {
      Actors::simulate_current_contract(
        actor.actor_id,
        ActorType::User,
        Mutability::Mutable,
        contract.clone(),
        crate::SimulationMode::FreshCurrentPlan,
        budget,
      )
    };
    let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    for delta in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
      let mut refused = budget;
      refused.actor_control = upper.checked_sub(&delta).unwrap();
      assert_eq!(
        simulate(refused),
        Err(crate::SimulationError::ResourceDeferred)
      );
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        root
      );
    }
    set_missing_behavior_close_weight(true);
    assert_eq!(
      simulate(budget),
      Err(crate::SimulationError::ResourceDeferred)
    );
    set_missing_behavior_close_weight(false);
    let result = simulate(budget).unwrap();
    assert_eq!(
      result.status,
      crate::AttemptDisposition::Closed(CloseReason::CycleNonceExhausted)
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      root
    );
    let reason: RuntimeHoldReason = crate::HoldReason::ActorState.into();
    assert_ok!(Balances::release(&reason, &ALICE, 1, Precision::Exact));
    assert!(Actors::active_actor_state(actor.actor_id).is_some());
    let corrupt_root =
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
    assert_eq!(
      simulate(budget),
      Err(crate::SimulationError::Classification(
        crate::ActorClassificationError::ActorInvariant
      ))
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
      corrupt_root
    );
  });
}

#[test]
fn behavior_runtime_policy_detachment_rejects_wrong_body_generation_and_host_authority() {
  for defect in 0..3 {
    new_test_ext().execute_with(|| {
      let (actor, _) = prepared_policy_source(
        BoundedVec::try_from(vec![transfer_step(None)]).unwrap(),
        single_metric(crate::BehaviorMetric::ActionSuccessTotal, 2),
      );
      let mut supplied_actor = actor;
      let mut source = Actors::active_actor_state(actor.actor_id).unwrap();
      match defect {
        0 => source.contract.steps[0].task = Task::StopCycle,
        1 => supplied_actor.generation += 1,
        _ => set_admission_semantics_version(2),
      }
      assert_noop!(
        Actors::remove_actor_publication_and_finalize(
          supplied_actor,
          source,
          None,
          CloseReason::BehaviorPolicySatisfied,
        ),
        Error::<Test>::ActorInvariant
      );
      assert!(crate::ActorBehaviorPolicies::<Test>::contains_key(
        actor.actor_id
      ));
      assert!(crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
    });
  }
}

#[test]
fn behavior_runtime_canonical_service_selects_real_steps_and_closes_after_accumulated_success() {
  new_test_ext().execute_with(|| {
    let mut second = transfer_step(None);
    second.task = Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(20),
    };
    let (actor, _) = prepared_policy_source(
      BoundedVec::try_from(vec![transfer_step(None), second]).unwrap(),
      single_metric(crate::BehaviorMetric::ActionSuccessTotal, 2),
    );
    let sovereign = sovereign_account(actor.actor_id);
    assert_ok!(MockAssetOps::mint(&sovereign, TestAsset::Native, 100));
    let bob_before = native_balance(&BOB);
    run_canonical_round_at(2, Weight::MAX);
    assert_eq!(native_balance(&sovereign), 90);
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(Actors::actor_run_state(actor.actor_id).unwrap().cursor, 1);
    run_canonical_round_at(3, Weight::MAX);
    assert_eq!(native_balance(&sovereign), 70);
    assert_eq!(native_balance(&BOB), bob_before + 30);
    assert!(Actors::actor_identity(actor.actor_id).is_none());
    assert!(!crate::ActorBehaviorMonitors::<Test>::contains_key(
      actor.actor_id
    ));
  });
}

#[test]
fn behavior_runtime_service_loader_requires_policy_prices_and_preserves_absent_policy_ports() {
  new_test_ext().execute_with(|| {
    let (actor, _) = prepared_policy_source(
      BoundedVec::try_from(vec![transfer_step(None)]).unwrap(),
      single_metric(crate::BehaviorMetric::ActionSuccessTotal, 2),
    );
    let none = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(Actors::load_actor_service_state_with_authority(actor.actor_id).is_some());
    set_missing_behavior_control_weight(true);
    assert!(Actors::load_actor_service_state_with_authority(actor.actor_id).is_none());
    assert!(Actors::load_actor_service_state_with_authority(none).is_some());
    set_missing_behavior_control_weight(false);
    set_missing_behavior_close_weight(true);
    assert!(Actors::load_actor_service_state_with_authority(actor.actor_id).is_none());
    assert!(Actors::load_actor_service_state_with_authority(none).is_some());
    set_missing_behavior_close_weight(false);
    assert!(Actors::load_actor_service_state_with_authority(actor.actor_id).is_some());
  });
}

#[test]
fn behavior_runtime_canonical_zero_step_and_clean_completion_use_policy_authority() {
  for zero_step in [false, true] {
    new_test_ext().execute_with(|| {
      let (steps, metric, actor_type) = if zero_step {
        (
          BoundedVec::default(),
          crate::BehaviorMetric::CycleCompletedTotal,
          ActorType::User,
        )
      } else {
        (
          BoundedVec::try_from(vec![transfer_step(None), transfer_step(None)]).unwrap(),
          crate::BehaviorMetric::CycleCleanTotal,
          ActorType::System,
        )
      };
      let (actor, _) =
        prepared_policy_source_for_class(steps, single_metric(metric, 1), actor_type);
      fund_native(actor.actor_id, 1_000);
      run_canonical_round_at(2, Weight::MAX);
      if !zero_step {
        assert!(
          crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id)
            .unwrap()
            .clean_cycle
            .is_some()
        );
        run_canonical_round_at(3, Weight::MAX);
      }
      assert!(Actors::actor_identity(actor.actor_id).is_none());
      assert!(!crate::ActorBehaviorPolicies::<Test>::contains_key(
        actor.actor_id
      ));
      assert!(!crate::ActorBehaviorMonitors::<Test>::contains_key(
        actor.actor_id
      ));
    });
  }
}

#[test]
fn behavior_runtime_opening_stop_cycle_ignores_unreached_tail_geometry() {
  for actor_type in [ActorType::User, ActorType::System] {
    new_test_ext().execute_with(|| {
      let mut steps = vec![transfer_step(None); 9];
      steps[0].task = Task::StopCycle;
      let (actor, _) = prepared_policy_source_for_class(
        BoundedVec::try_from(steps).unwrap(),
        single_metric(crate::BehaviorMetric::CycleCompletedTotal, 2),
        actor_type,
      );
      fund_native(actor.actor_id, 1_000);
      let sovereign = sovereign_account(actor.actor_id);
      let custody = native_balance(&sovereign);
      let sink = native_balance(&TestFeeSink::get());
      let pipeline = Actors::pipeline_fee_for_actor(actor.actor_id, actor_type)
        .unwrap()
        .total_fee;
      // Read-isolation falsifier, not an admitted corrupt Contract: restore the untouched tail
      // after the attempt. Its absence must not affect an unconditional current StopCycle.
      let tail = crate::ActorContractTailChunks::<Test>::take(actor.actor_id, 1).unwrap();
      assert!(Actors::load_actor_service_state_with_authority(actor.actor_id).is_some());
      run_canonical_round_at(2, Weight::MAX);
      let (record, _) = Actors::load_canonical_actor_semantic_state(actor).unwrap();
      assert_eq!(record.identity.cycle_nonce, 1);
      assert_eq!(record.hot.cycle_state, CycleState::Idle);
      assert!(!record.hot.pending_signal);
      assert!(!crate::ActorRunStateStore::<Test>::contains_key(
        actor.actor_id
      ));
      assert_eq!(native_balance(&sovereign), custody - pipeline);
      assert_eq!(native_balance(&TestFeeSink::get()), sink + pipeline);
      crate::ActorContractTailChunks::<Test>::insert(actor.actor_id, 1, tail);
      assert!(Actors::load_actor_contract(actor.actor_id).is_some());
      #[cfg(feature = "try-runtime")]
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn behavior_runtime_stop_cycle_intrinsic_preserves_fees_custody_and_rollback() {
  for actor_type in [ActorType::User, ActorType::System] {
    for (closes, rollback) in [(false, false), (false, true), (true, false), (true, true)] {
      new_test_ext().execute_with(|| {
        let mut steps = vec![transfer_step(None); 9];
        steps[0].task = Task::StopCycle;
        let (actor, _) = prepared_policy_source_for_class(
          BoundedVec::try_from(steps).unwrap(),
          single_metric(
            crate::BehaviorMetric::CycleCompletedTotal,
            if closes { 1 } else { 2 },
          ),
          actor_type,
        );
        fund_native(actor.actor_id, 1_000);
        System::set_block_number(2);
        begin_prepared_round(2);
        let (state, admission, loaded) =
          Actors::load_actor_service_state_with_authority(actor.actor_id).unwrap();
        let loaded = loaded.unwrap();
        let effect_limit = loaded.resources.effect;
        let fee =
          Actors::maximum_current_action_fee(actor_type, &loaded.step, loaded.resources).unwrap();
        assert_eq!(fee.total_fee, 0, "StopCycle has no Action fee");
        let plan = Actors::build_canonical_current_step_plan(
          actor.actor_id,
          state.identity.clone(),
          state.hot.clone(),
          state.run_state.clone(),
          admission.clone(),
          loaded,
          fee,
        )
        .unwrap();
        let sovereign = state.identity.sovereign_account;
        let custody = native_balance(&sovereign);
        let hold_before = crate::ActorStateHolds::<Test>::get(actor.actor_id);
        let policy_before = crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id);
        let monitor_before = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id);
        let Some(crate::ProcessResidence::Service(kind)) =
          crate::ActorProcesses::<Test>::get(actor.actor_id)
            .unwrap()
            .residence
        else {
          panic!("prepared source has Service residence")
        };
        let sink = native_balance(&TestFeeSink::get());
        let pipeline = Actors::pipeline_fee_for_actor(actor.actor_id, actor_type)
          .unwrap()
          .total_fee;
        set_missing_behavior_control_weight(true);
        set_missing_behavior_close_weight(true);
        set_missing_behavior_inspection_weight(true);
        let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
          let (state, reason, effect) =
            Actors::benchmark_stop_cycle_control(actor.actor_id, state, plan, &admission, 2)
              .unwrap();
          assert!(effect.all_lte(effect_limit));
          if closes {
            assert_eq!(reason, Some(crate::CloseReason::BehaviorPolicySatisfied));
            assert_ok!(Actors::benchmark_finalize_actor_from_consumed_state(
              actor.actor_id,
              state,
              admission,
              reason.unwrap(),
            ));
            assert_ok!(Actors::retire_service_member(actor));
          } else {
            assert_eq!(reason, None);
            assert_eq!(state.hot.cycle_state, CycleState::Idle);
            assert!(state.run_state.is_none());
            assert_ok!(Actors::publish_idle_cycle_completion(
              actor, kind, &state, 2
            ));
          }
          if rollback {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
          } else {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
          }
        });
        if rollback {
          assert_eq!(
            polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
            root
          );
        } else {
          if closes {
            assert!(Actors::actor_identity(actor.actor_id).is_none());
            assert!(!crate::ActorProcesses::<Test>::contains_key(actor.actor_id));
            assert!(!crate::ActorBehaviorPolicies::<Test>::contains_key(
              actor.actor_id
            ));
            assert!(!crate::ActorBehaviorMonitors::<Test>::contains_key(
              actor.actor_id
            ));
          } else {
            let crate::ActorSemanticState::Active(record) =
              crate::ActorSemanticStates::<Test>::get(actor.actor_id).unwrap()
            else {
              panic!("nonterminal StopCycle retains Active authority")
            };
            assert_eq!(record.identity.cycle_nonce, 1);
            assert_eq!(record.hot.cycle_state, CycleState::Idle);
            assert!(!record.hot.pending_signal);
            assert!(!crate::ActorRunStateStore::<Test>::contains_key(
              actor.actor_id
            ));
            assert!(!crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
            assert!(crate::ActorProcesses::<Test>::contains_key(actor.actor_id));
            assert_eq!(
              crate::ActorStateHolds::<Test>::get(actor.actor_id),
              hold_before
            );
            assert_eq!(
              crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id),
              policy_before
            );
            assert_ne!(
              crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id),
              monitor_before
            );
          }
          assert_eq!(native_balance(&sovereign), custody - pipeline);
          assert_eq!(native_balance(&TestFeeSink::get()), sink + pipeline);
          #[cfg(feature = "try-runtime")]
          {
            // Global repricing needs the creation-time model ports. Intrinsic execution and
            // publication above ran with all optional policy prices absent.
            set_missing_behavior_control_weight(false);
            set_missing_behavior_close_weight(false);
            set_missing_behavior_inspection_weight(false);
            assert_ok!(Actors::do_try_state());
          }
        }
      });
    }
  }
}

#[cfg(feature = "runtime-benchmarks")]
#[test]
fn behavior_runtime_transfer_policy_preserves_action_fees_and_rollback() {
  for actor_type in [ActorType::User, ActorType::System] {
    for (closes, rollback) in [(false, false), (false, true), (true, false), (true, true)] {
      new_test_ext().execute_with(|| {
        let (actor, _) = prepared_policy_source_for_class(
          BoundedVec::try_from(vec![transfer_step(None); 9]).unwrap(),
          single_metric(
            crate::BehaviorMetric::ActionSuccessTotal,
            if closes { 1 } else { 2 },
          ),
          actor_type,
        );
        fund_native(actor.actor_id, 1_000);
        System::set_block_number(2);
        begin_prepared_round(2);
        let (state, admission, loaded) =
          Actors::load_actor_service_state_with_authority(actor.actor_id).unwrap();
        let loaded = loaded.unwrap();
        let step = loaded.step.clone();
        let effect_limit = loaded.resources.effect;
        let fee = Actors::maximum_current_action_fee(actor_type, &step, loaded.resources).unwrap();
        let plan = Actors::build_canonical_current_step_plan(
          actor.actor_id,
          state.identity.clone(),
          state.hot.clone(),
          state.run_state.clone(),
          admission.clone(),
          loaded,
          fee,
        )
        .unwrap();
        let identity_before = state.identity.clone();
        let policy_before = crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id);
        let monitor_before = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id);
        let hold_before = crate::ActorStateHolds::<Test>::get(actor.actor_id);
        let Some(crate::ProcessResidence::Service(kind)) =
          crate::ActorProcesses::<Test>::get(actor.actor_id)
            .unwrap()
            .residence
        else {
          panic!("prepared source has Service residence")
        };
        let sovereign = state.identity.sovereign_account;
        let custody = native_balance(&sovereign);
        let recipient = native_balance(&BOB);
        let sink = native_balance(&TestFeeSink::get());
        let pipeline = Actors::pipeline_fee_for_actor(actor.actor_id, actor_type)
          .unwrap()
          .total_fee;
        set_missing_behavior_control_weight(true);
        set_missing_behavior_close_weight(true);
        set_missing_behavior_inspection_weight(true);
        let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        let effect = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
          let (state, reason, effect, requires_clean) =
            Actors::benchmark_transfer_control(actor.actor_id, state, plan, &admission, 2).unwrap();
          assert!(
            !requires_clean,
            "scalar ActionSuccess policy has no clean-Cycle state"
          );
          if closes {
            assert_eq!(reason, Some(crate::CloseReason::BehaviorPolicySatisfied));
            assert_ok!(Actors::benchmark_finalize_actor_from_consumed_state(
              actor.actor_id,
              state,
              admission,
              reason.unwrap(),
            ));
            assert_ok!(Actors::retire_service_member(actor));
          } else {
            assert_eq!(reason, None);
            assert_eq!(state.hot.cycle_state, CycleState::Running);
            assert_ok!(Actors::try_store_service_control_state(
              actor,
              kind,
              state.identity.clone(),
              state.hot.clone(),
            ));
            assert_ok!(Actors::advance_service_head(actor, 2));
          }
          // Match the production suffix: Action collection follows placement/destruction.
          // Optional actual-control qualification remains outside this intrinsic witness.
          let actual_fee = Actors::maximum_current_action_fee(
            actor_type,
            &step,
            crate::ActorStepResourceEnvelope {
              control: Weight::zero(),
              effect,
            },
          )
          .unwrap();
          if actor_type == ActorType::User && actual_fee.total_fee != 0 {
            assert_ok!(Actors::collect_user_step_fee(
              &sovereign,
              actual_fee.total_fee
            ));
          }
          Actors::deposit_action_fee_receipt(actor.actor_id, 1, 0, effect, actual_fee.effect_fee);
          if rollback {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(effect)
          } else {
            polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(effect)
          }
        });
        assert_ne!(effect, Weight::zero());
        assert!(effect.all_lte(effect_limit));
        let action = Actors::maximum_current_action_fee(
          actor_type,
          &step,
          crate::ActorStepResourceEnvelope {
            control: Weight::zero(),
            effect,
          },
        )
        .unwrap()
        .total_fee;
        assert_eq!(action == 0, actor_type == ActorType::System);
        if rollback {
          assert_eq!(
            polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
            root
          );
          assert_eq!(native_balance(&sovereign), custody);
          assert_eq!(native_balance(&BOB), recipient);
          assert_eq!(native_balance(&TestFeeSink::get()), sink);
        } else {
          if closes {
            assert!(Actors::actor_identity(actor.actor_id).is_none());
            assert!(!crate::ActorProcesses::<Test>::contains_key(actor.actor_id));
            assert!(!crate::ActorStateHolds::<Test>::contains_key(
              actor.actor_id
            ));
            assert!(!crate::ActorBehaviorPolicies::<Test>::contains_key(
              actor.actor_id
            ));
            assert!(!crate::ActorBehaviorMonitors::<Test>::contains_key(
              actor.actor_id
            ));
          } else {
            let crate::ActorSemanticState::Active(record) =
              crate::ActorSemanticStates::<Test>::get(actor.actor_id).unwrap()
            else {
              panic!("successful nonterminal Transfer retains Active authority")
            };
            assert_eq!(record.identity, identity_before);
            assert_eq!(record.hot.cycle_state, CycleState::Running);
            assert!(!record.hot.pending_signal);
            let run = crate::ActorRunStateStore::<Test>::get(actor.actor_id).unwrap();
            assert_eq!((run.cursor, run.cycle_nonce, run.eligible_at), (1, 1, 3));
            assert_eq!(run.cumulative_outcomes.committed_effectful_tasks, 1);
            assert_eq!(
              crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id),
              policy_before
            );
            assert_ne!(
              crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id),
              monitor_before
            );
            // Active installation already holds maximum Run capacity; opening cannot grow it.
            assert_eq!(
              crate::ActorStateHolds::<Test>::get(actor.actor_id),
              hold_before
            );
            assert_eq!(
              crate::ServiceNodes::<Test>::get(actor.actor_id)
                .unwrap()
                .last_considered,
              2
            );
            assert_eq!(
              crate::ActorProcesses::<Test>::get(actor.actor_id)
                .unwrap()
                .last_attempted,
              Some(2)
            );
            let encounter =
              polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
                polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
                  Actors::consider_service_head(2),
                )
              });
            assert_eq!(encounter, Ok(crate::ServiceRoundEncounter::Closed));
            System::set_block_number(3);
            begin_prepared_round(3);
            let encounter =
              polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
                polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
                  Actors::consider_service_head(3),
                )
              });
            assert_eq!(encounter, Ok(crate::ServiceRoundEncounter::Eligible(actor)));
          }
          assert_eq!(native_balance(&sovereign), custody - 10 - pipeline - action);
          assert_eq!(native_balance(&BOB), recipient + 10);
          assert_eq!(
            native_balance(&TestFeeSink::get()),
            sink + pipeline + action
          );
          let receipt = RuntimeEvent::Actors(crate::Event::ActionFeeCharged {
            actor_id: actor.actor_id,
            cycle_nonce: 1,
            step_index: 0,
            actual_effect_weight: effect,
            fee: action,
          });
          assert_eq!(
            System::events()
              .iter()
              .filter(|event| event.event == receipt)
              .count(),
            1,
            "invoked Actions retain a receipt, including the System zero-fee receipt"
          );
          #[cfg(feature = "try-runtime")]
          {
            // Reprice retained state only after execution/publication/fee assertions without ports.
            set_missing_behavior_control_weight(false);
            set_missing_behavior_close_weight(false);
            set_missing_behavior_inspection_weight(false);
            assert_ok!(Actors::do_try_state());
          }
        }
      });
    }
  }
}

#[test]
fn behavior_runtime_running_policy_service_ignores_unreached_tail_geometry() {
  new_test_ext().execute_with(|| {
    let (actor, _) = prepared_policy_source(
      BoundedVec::try_from(vec![transfer_step(None); 9]).unwrap(),
      single_metric(crate::BehaviorMetric::ActionSuccessTotal, 20),
    );
    let sovereign = sovereign_account(actor.actor_id);
    fund_native(actor.actor_id, 1_000);
    run_canonical_round_at(2, Weight::MAX);
    crate::ActorContractTailChunks::<Test>::remove(actor.actor_id, 1);
    assert!(Actors::load_actor_service_state_with_authority(actor.actor_id).is_some());
    let custody = native_balance(&sovereign);
    run_canonical_round_at(3, Weight::MAX);
    assert_eq!(
      crate::ActorRunStateStore::<Test>::get(actor.actor_id)
        .unwrap()
        .cursor,
      2
    );
    assert_eq!(native_balance(&sovereign), custody - 10);
  });
}

#[test]
fn behavior_runtime_service_inspection_is_admitted_before_cold_reads_and_retained_on_refusal() {
  use crate::WeightInfo;
  for missing_price in [false, true] {
    for insufficient_probe in [false, true] {
      for pass_owned in [false, true] {
        new_test_ext().execute_with(|| {
        let (actor, _) = prepared_policy_source(
          BoundedVec::try_from(vec![transfer_step(None)]).unwrap(),
          single_metric(crate::BehaviorMetric::ActionSuccessTotal, 2),
        );
        System::set_block_number(2);
        let base = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
          .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible())
          .saturating_add(<Test as crate::Config>::WeightInfo::scheduler_actor_state_probe());
        let extra = <MockStepControlWeight as crate::StepControlWeightProvider<StepOf<Test>>>::maximum_behavior_service_inspection_weight(
          Actors::behavior_service_inspection_bounds().unwrap(),
        ).unwrap();
        let complete = base.checked_add(&extra).unwrap();
        // A malformed companion distinguishes pre-read refusal from attempted cold decoding.
        crate::ActorBehaviorPolicies::<Test>::remove(actor.actor_id);
        let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        set_missing_behavior_inspection_weight(missing_price);
        let limits = ample_simulation_budget().checked_limits().unwrap();
        let mut resources = crate::BlockResourceState::new(2);
        assert_ok!(resources.begin_prepass(limits.into_budget().unwrap()));
        let mut meter = WeightMeter::with_limit(if insufficient_probe {
          complete.checked_sub(&Weight::from_parts(1, 0)).unwrap()
        } else { complete });
        let result = if pass_owned {
          let mut parent = resources.reserve(limits, crate::BlockResourceDomain::ActorControl, complete).unwrap();
          let result = Actors::service_canonical_round_head_with_reserved_control(
            &mut meter, 2, &mut resources, limits, crate::BlockResourceDomain::ActorBaseEffect, complete,
          );
          assert_ok!(resources.settle(&mut parent, meter.consumed()));
          result
        } else {
          Actors::service_canonical_round_head_with_resources(
            &mut meter, 2, &mut resources, limits, crate::BlockResourceDomain::ActorBaseEffect,
          )
        };
        let expected = if missing_price {
          crate::ServiceRoundError::ResourceUnavailable
        } else if insufficient_probe {
          crate::ServiceRoundError::InsufficientWeight
        } else {
          crate::ServiceRoundError::ProcessResidenceMismatch
        };
        assert_eq!(result, Err(expected));
        assert_eq!(meter.consumed(), if missing_price || insufficient_probe { base } else { complete });
        assert_eq!(resources.usage().actor_control_used(), meter.consumed());
        assert_eq!(resources.outstanding_reservations(), 0);
        assert_eq!(polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1), root);
      });
      }
    }
  }
}

#[test]
fn behavior_runtime_inspection_refusal_reads_no_optional_companion_key() {
  use crate::WeightInfo;
  use polkadot_sdk::sp_core::Blake2Hasher;
  use polkadot_sdk::sp_trie::recorder::Recorder;
  for admitted in [false, true] {
    let mut ext = new_test_ext();
    let (actor, complete) = ext.execute_with(|| {
      let (actor, _) = prepared_policy_source(
        BoundedVec::try_from(vec![transfer_step(None)]).unwrap(),
        single_metric(crate::BehaviorMetric::ActionSuccessTotal, 2),
      );
      System::set_block_number(2);
      let base = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
        .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible())
        .saturating_add(<Test as crate::Config>::WeightInfo::scheduler_actor_state_probe());
      let extra = <MockStepControlWeight as crate::StepControlWeightProvider<StepOf<Test>>>::maximum_behavior_service_inspection_weight(
        Actors::behavior_service_inspection_bounds().unwrap(),
      ).unwrap();
      (actor, base.checked_add(&extra).unwrap())
    });
    ext.commit_all().unwrap();
    let recorder = Recorder::<Blake2Hasher>::default();
    ext.execute_with_recorder(recorder.clone(), || {
      let mut meter = WeightMeter::with_limit(if admitted {
        complete
      } else {
        complete.checked_sub(&Weight::from_parts(0, 1)).unwrap()
      });
      assert!(Actors::service_canonical_round_head(&mut meter, 2).is_err());
    });
    let keys: std::collections::BTreeSet<Vec<u8>> = recorder
      .recorded_keys()
      .into_values()
      .flat_map(|keys| keys.into_keys().map(|key| key.as_ref().to_vec()))
      .collect();
    assert_eq!(
      keys.contains(&crate::ActorBehaviorPolicies::<Test>::hashed_key_for(
        actor.actor_id
      )),
      admitted
    );
    assert!(
      !keys.contains(&crate::ActorBehaviorMonitors::<Test>::hashed_key_for(
        actor.actor_id
      ))
    );
  }
}

#[test]
fn behavior_runtime_missing_control_authority_refuses_before_opening_or_action() {
  for committed_prefix in [false, true] {
    new_test_ext().execute_with(|| {
      let (actor, _) = prepared_policy_source(
        BoundedVec::try_from(vec![transfer_step(None), transfer_step(None)]).unwrap(),
        single_metric(crate::BehaviorMetric::ActionSuccessTotal, 2),
      );
      let sovereign = sovereign_account(actor.actor_id);
      assert_ok!(MockAssetOps::mint(&sovereign, TestAsset::Native, 100));
      if committed_prefix {
        execute_prepared_step(actor, 2)
          .unwrap_or_else(|error| panic!("step refused: {:?}", error.cause));
      }
      let now = if committed_prefix { 3 } else { 2 };
      System::set_block_number(now);
      begin_prepared_round(now);
      let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      let bob_before = native_balance(&BOB);
      set_missing_behavior_control_weight(true);
      let Err(error) = execute_prepared_step(actor, now) else {
        panic!("missing policy control authority must refuse");
      };
      assert_eq!(
        error.cause,
        crate::scheduler::AttemptTransactionError::Invariant
      );
      assert!(error.actual_effect_weight.is_none());
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        before
      );
      assert_eq!(native_balance(&BOB), bob_before);
      set_missing_behavior_control_weight(false);
      let repaired = execute_prepared_step(actor, now)
        .unwrap_or_else(|error| panic!("step refused: {:?}", error.cause));
      assert_eq!(
        repaired.attempt.as_ref().unwrap().is_closed(),
        committed_prefix
      );
      assert_eq!(native_balance(&BOB), bob_before + 10);
    });
  }
}

#[test]
fn behavior_runtime_pass_threshold_closes_after_its_permitted_action_and_rolls_back_late_refusal() {
  for late_refusal in [false, true] {
    new_test_ext().execute_with(|| {
      let predicate = all_conditions(vec![crate::Predicate::BalanceAbove {
        asset: TestAsset::Native,
        threshold: 0,
      }]);
      let steps =
        BoundedVec::try_from(vec![transfer_step(predicate), transfer_step(None)]).unwrap();
      let (actor, _) = prepared_policy_source(
        steps,
        single_metric(crate::BehaviorMetric::PreconditionPassTotal, 1),
      );
      let sovereign = sovereign_account(actor.actor_id);
      assert_ok!(MockAssetOps::mint(&sovereign, TestAsset::Native, 100));
      System::set_block_number(2);
      begin_prepared_round(2);
      let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      let bob_before = native_balance(&BOB);
      if late_refusal {
        set_step_control_actual_weight_override(Some(Weight::MAX));
      }
      let result = execute_prepared_step(actor, 2);
      if late_refusal {
        let Err(error) = result else {
          panic!("late control refusal must roll back");
        };
        assert_eq!(
          error.cause,
          crate::scheduler::AttemptTransactionError::Invariant
        );
        assert!(error.actual_effect_weight.is_some());
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
          before
        );
        assert_eq!(native_balance(&BOB), bob_before);
        assert!(crate::ActorBehaviorPolicies::<Test>::contains_key(
          actor.actor_id
        ));
      } else {
        let evidence = result.unwrap_or_else(|error| panic!("step refused: {:?}", error.cause));
        assert!(evidence.attempt.as_ref().unwrap().is_closed());
        assert!(has_actor_event(
          |event| matches!(event, Event::ActorClosed { actor_id: id,
          reason: crate::CloseReason::BehaviorPolicySatisfied, .. } if *id == actor.actor_id)
        ));
        assert_eq!(native_balance(&BOB), bob_before + 10);
        assert_eq!(native_balance(&sovereign), 90);
        assert!(!crate::ActorBehaviorPolicies::<Test>::contains_key(
          actor.actor_id
        ));
        assert!(!crate::ActorBehaviorMonitors::<Test>::contains_key(
          actor.actor_id
        ));
        assert!(!crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
      }
    });
  }
}

#[test]
fn behavior_runtime_retry_preserves_paid_prefix_and_attempt_fees() {
  use crate::TaskEffectWeightProvider;
  for actor_type in [ActorType::User, ActorType::System] {
    for suspended in [false, true] {
      for rollback in [false, true] {
        new_test_ext().execute_with(|| {
          let retry = Step {
            precondition: None,
            task: Task::AddLiquidity {
              asset_a: TestAsset::Local(901), asset_b: TestAsset::Local(902),
              amount_a: AmountResolution::Percent(Perbill::from_percent(50)),
              amount_b: AmountResolution::Percent(Perbill::from_percent(50)), min_lp_out: 1_000,
            },
            on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
          };
          let mut steps = vec![transfer_step(None); <<Test as crate::Config>::MaxContractSteps as Get<u32>>::get() as usize];
          steps[1] = retry.clone();
          let (actor, _) = prepared_policy_source_for_class(steps.try_into().unwrap(),
            single_metric(crate::BehaviorMetric::ActionSuccessTotal, u32::MAX), actor_type);
          let payer = sovereign_account(actor.actor_id);
          assert_ok!(MockAssetOps::mint(&payer, TestAsset::Native, 1_000_000));
          for asset in [TestAsset::Local(901), TestAsset::Local(902)] {
            assert_ok!(MockAssetOps::mint(&payer, asset, 1_000));
          }
          let recipient_before = native_balance(&BOB);
          execute_prepared_step(actor, 2).unwrap_or_else(|error| panic!("Opening refused: {:?}", error.cause));
          assert_eq!(native_balance(&BOB), recipient_before + 10);
          if suspended {
            execute_prepared_step(actor, 3).unwrap_or_else(|error| panic!("retry preparation refused: {:?}", error.cause));
            assert_eq!(Actors::actor_run_state(actor.actor_id).unwrap().eligible_at, 4);
            for asset in [TestAsset::Local(901), TestAsset::Local(902)] {
              assert_ok!(MockAssetOps::mint(&payer, asset, 3_000));
            }
          }
          assert_eq!(Actors::active_actor_state(actor.actor_id).unwrap().contract.steps[1], retry);
          let now = if suspended { 4 } else { 3 };
          let prior_run = Actors::actor_run_state(actor.actor_id).unwrap();
          assert_eq!(prior_run.cursor, 1);
          assert_eq!(prior_run.cumulative_outcomes.committed_effectful_tasks, 1);
          let held = crate::ActorStateHolds::<Test>::get(actor.actor_id);
          let payer_before = native_balance(&payer);
          let inputs = [TestAsset::Local(901), TestAsset::Local(902)].map(|asset| MockAssetOps::balance(&payer, asset));
          let receipt_counts = || {
            let mut counts = (0, 0);
            for record in System::events() {
              match record.event {
                RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id, .. }) if actor_id == actor.actor_id => counts.0 += 1,
                RuntimeEvent::Actors(Event::ActionFeeCharged { actor_id, .. }) if actor_id == actor.actor_id => counts.1 += 1,
                _ => {},
              }
            }
            counts
          };
          let counts = receipt_counts();
          let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
          let fee = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
            let evidence = execute_prepared_step(actor, now).unwrap_or_else(|error| panic!("attempt refused: {:?}", error.cause));
            let expected = <Test as crate::Config>::TaskEffectWeight::actual_effect_weight(&retry.task, crate::TaskEffectExecution::Invoked).unwrap();
            assert_eq!(evidence.actual_effect_weight, expected);
            assert_ne!(expected, Weight::zero());
            let outcome = Actors::actor_run_state(actor.actor_id).unwrap().last_step_outcome.unwrap();
            if suspended { assert!(matches!(outcome, crate::StepOutcome::Executed)); }
            else { assert!(matches!(outcome, crate::StepOutcome::Failed(failure) if failure.retry == RetryClass::Temporary && failure.error.encode() == DispatchError::Other("MinimumLpOutputNotMet").encode())); }
            let run = Actors::actor_run_state(actor.actor_id).unwrap();
            assert_eq!(run.cursor, if suspended { 2 } else { 1 });
            assert_eq!(run.cycle_nonce, prior_run.cycle_nonce);
            assert_eq!(run.eligible_at, now + 1);
            assert_eq!(run.cumulative_outcomes.committed_effectful_tasks, if suspended { 2 } else { 1 });
            assert_eq!(run.unsuccessful_attempts_at_cursor, if suspended { 0 } else { 1 });
            let state = Actors::active_actor_state(actor.actor_id).unwrap();
            assert_eq!(state.hot.cycle_state, if suspended { CycleState::Running } else { CycleState::Suspended });
            let fee = Actors::maximum_current_action_fee(actor_type, &retry,
              crate::ActorStepResourceEnvelope { control: Weight::zero(), effect: expected }).unwrap().total_fee;
            assert_eq!(native_balance(&payer), payer_before - fee);
            assert_eq!(receipt_counts(), (counts.0, counts.1 + 1));
            if rollback { polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(fee) }
            else { polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(fee) }
          });
          assert_eq!(native_balance(&BOB), recipient_before + 10);
          assert_eq!(crate::ActorStateHolds::<Test>::get(actor.actor_id), held);
          assert_eq!(Actors::active_actor_state(actor.actor_id).unwrap().contract.steps[1], retry);
          if rollback {
            assert_eq!(native_balance(&payer), payer_before);
            assert_eq!(receipt_counts(), counts);
            assert_eq!(Actors::actor_run_state(actor.actor_id), Some(prior_run));
            assert_eq!(polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1), root);
          } else {
            assert_eq!(native_balance(&payer), payer_before - fee);
          }
          if rollback || !suspended {
            assert_eq!([TestAsset::Local(901), TestAsset::Local(902)].map(|asset| MockAssetOps::balance(&payer, asset)), inputs);
          }
        });
      }
    }
  }
}

#[test]
fn behavior_runtime_late_retry_and_exhaustion_preserve_committed_boundaries() {
  for actor_type in [ActorType::User, ActorType::System] {
    for terminal in [false, true] {
      for rollback in [false, true] {
        new_test_ext().execute_with(|| {
          let retry = Step {
            precondition: None,
            task: Task::AddLiquidity {
              asset_a: TestAsset::Local(901),
              asset_b: TestAsset::Local(902),
              amount_a: AmountResolution::Percent(Perbill::from_percent(50)),
              amount_b: AmountResolution::Percent(Perbill::from_percent(50)),
              min_lp_out: 1_000,
            },
            on_error: StepErrorPolicy::RetryLater { max_attempts: 3 },
          };
          let mut steps = vec![
            transfer_step(None);
            <<Test as crate::Config>::MaxContractSteps as Get<u32>>::get()
              as usize
          ];
          steps[1] = retry.clone();
          let (actor, _) = prepared_policy_source_for_class(
            steps.try_into().unwrap(),
            single_metric(crate::BehaviorMetric::ActionSuccessTotal, u32::MAX),
            actor_type,
          );
          let id = actor.actor_id;
          let payer = sovereign_account(id);
          assert_ok!(MockAssetOps::mint(&payer, TestAsset::Native, 1_000_000));
          for asset in [TestAsset::Local(901), TestAsset::Local(902)] {
            assert_ok!(MockAssetOps::mint(&payer, asset, 1_000));
          }
          let recipient = native_balance(&BOB);
          execute_prepared_step(actor, 2)
            .unwrap_or_else(|error| panic!("prefix refused: {:?}", error.cause));
          execute_prepared_step(actor, 3)
            .unwrap_or_else(|error| panic!("first failure refused: {:?}", error.cause));
          assert_eq!(Actors::actor_run_state(id).unwrap().eligible_at, 4);
          if terminal {
            execute_prepared_step(actor, 4)
              .unwrap_or_else(|error| panic!("late retry refused: {:?}", error.cause));
            assert_eq!(Actors::actor_run_state(id).unwrap().eligible_at, 6);
            assert!(crate::DeadlineHandles::<Test>::contains_key(id));
            assert!(!crate::ServiceNodes::<Test>::contains_key(id));
            let run = Actors::actor_run_state(id).unwrap();
            let balance = native_balance(&payer);
            System::set_block_number(6);
            assert_ok!(Actors::return_due_deadline_member_to_service(
              actor,
              crate::ServiceResidenceKind::Live,
              6
            ));
            assert_eq!(Actors::actor_run_state(id), Some(run));
            assert_eq!(native_balance(&payer), balance);
            assert!(!crate::DeadlineHandles::<Test>::contains_key(id));
            assert_eq!(crate::ServiceNodes::<Test>::get(id).unwrap().eligible_from, 7);
          }
          let now = if terminal { 7 } else { 4 };
          let prefix = Actors::actor_run_state(id).unwrap();
          let holds = crate::ActorStateHolds::<Test>::get(id);
          let balance = native_balance(&payer);
          let counts = || {
            let mut result = (0, 0, 0);
            for event in System::events() {
              match event.event {
                RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id, .. })
                  if actor_id == id =>
                {
                  result.0 += 1
                }
                RuntimeEvent::Actors(Event::ActionFeeCharged { actor_id, .. })
                  if actor_id == id =>
                {
                  result.1 += 1
                }
                RuntimeEvent::Actors(Event::ActorClosed {
                  actor_id,
                  reason: CloseReason::RetryAttemptsExhausted,
                }) if actor_id == id => result.2 += 1,
                _ => {}
              }
            }
            result
          };
          let receipts = counts();
          assert_eq!(receipts.0, usize::from(actor_type == ActorType::User));
          assert_eq!(receipts.1, if terminal { 3 } else { 2 });
          let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
          polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
            let result = execute_prepared_step(actor, now)
              .unwrap_or_else(|error| panic!("selected attempt refused: {:?}; actor={actor_type:?} terminal={terminal} rollback={rollback} now={now}", error.cause));
            let fee = Actors::maximum_current_action_fee(
              actor_type,
              &retry,
              crate::ActorStepResourceEnvelope {
                control: Weight::zero(),
                effect: result.actual_effect_weight,
              },
            )
            .unwrap()
            .total_fee;
            assert_ne!(result.actual_effect_weight, Weight::zero());
            assert_eq!(native_balance(&payer), balance - fee);
            assert_eq!(
              counts(),
              (
                receipts.0,
                receipts.1 + 1,
                receipts.2 + usize::from(terminal)
              )
            );
            if terminal {
              assert!(Actors::actor_identity(id).is_none());
              assert!(Actors::actor_run_state(id).is_none());
              assert!(!crate::ServiceNodes::<Test>::contains_key(id));
              assert!(!crate::DeadlineHandles::<Test>::contains_key(id));
              assert!(!crate::ActorStateHolds::<Test>::contains_key(id));
            } else {
              let run = Actors::actor_run_state(id).unwrap();
              assert_eq!(run.cursor, 1);
              assert_eq!(run.eligible_at, 6);
              assert_eq!(run.unsuccessful_attempts_at_cursor, 2);
              assert_eq!(run.cumulative_outcomes.committed_effectful_tasks, 1);
              assert!(crate::DeadlineHandles::<Test>::contains_key(id));
              assert!(!crate::ServiceNodes::<Test>::contains_key(id));
              assert_eq!(crate::ActorStateHolds::<Test>::get(id), holds);
            }
            if rollback {
              polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
            } else {
              polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
            }
          });
          assert_eq!(native_balance(&BOB), recipient + 10);
          for asset in [TestAsset::Local(901), TestAsset::Local(902)] {
            assert_eq!(MockAssetOps::balance(&payer, asset), 1_000);
          }
          if rollback {
            assert_eq!(Actors::actor_run_state(id), Some(prefix));
            assert_eq!(crate::ActorStateHolds::<Test>::get(id), holds);
            assert_eq!(native_balance(&payer), balance);
            assert_eq!(counts(), receipts);
            assert!(crate::ServiceNodes::<Test>::contains_key(id));
            assert!(!crate::DeadlineHandles::<Test>::contains_key(id));
            assert_eq!(
              polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
              root
            );
          }
        });
      }
    }
  }
}

#[test]
fn behavior_runtime_clean_flags_follow_committed_steps_and_reject_economic_skip() {
  for skip_tail in [false, true] {
    new_test_ext().execute_with(|| {
      let tail_predicate = if skip_tail {
        all_conditions(vec![crate::Predicate::BalanceAbove {
          asset: TestAsset::Native,
          threshold: 1_000,
        }])
      } else {
        None
      };
      let steps =
        BoundedVec::try_from(vec![transfer_step(None), transfer_step(tail_predicate)]).unwrap();
      let (actor, _) = prepared_policy_source(
        steps,
        single_metric(crate::BehaviorMetric::CycleCleanTotal, 1),
      );
      let sovereign = sovereign_account(actor.actor_id);
      assert_ok!(MockAssetOps::mint(&sovereign, TestAsset::Native, 100));
      let first = execute_prepared_step(actor, 2)
        .unwrap_or_else(|error| panic!("step refused: {:?}", error.cause));
      assert!(!first.attempt.as_ref().unwrap().is_closed());
      assert_eq!(
        Actors::active_actor_state(actor.actor_id)
          .unwrap()
          .run_state
          .unwrap()
          .cursor,
        1
      );
      let monitor = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id).unwrap();
      let clean = monitor.clean_cycle.unwrap();
      assert_eq!(clean.cycle_nonce, 1);
      let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      let crate::ActorEligibility::Active(view) = eligibility(actor.actor_id) else {
        panic!("Active committed clean-Cycle prefix");
      };
      let projection = view.behavior_episode.unwrap();
      let projected_monitor = projection.monitor.unwrap();
      assert!(projected_monitor.capped_values.is_empty());
      assert_eq!(projected_monitor.clean_cycle, Some(clean));
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        before
      );
      assert_eq!(
        clean
          .candidate
          .completion(Some(crate::NormalCycleCompletion::PlanEnd)),
        Some(true)
      );
      let second = execute_prepared_step(actor, 3)
        .unwrap_or_else(|error| panic!("step refused: {:?}", error.cause));
      assert_eq!(
        second.attempt.as_ref().unwrap().normal_cycle_completion,
        Some(crate::NormalCycleCompletion::PlanEnd)
      );
      if skip_tail {
        assert!(!second.attempt.as_ref().unwrap().is_closed());
        assert_eq!(
          Actors::active_actor_state(actor.actor_id)
            .unwrap()
            .identity
            .cycle_nonce,
          1
        );
        assert!(
          crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id)
            .unwrap()
            .clean_cycle
            .is_none()
        );
        assert_eq!(native_balance(&sovereign), 90);
      } else {
        assert!(second.attempt.as_ref().unwrap().is_closed());
        assert!(Actors::actor_identity(actor.actor_id).is_none());
        assert_eq!(native_balance(&sovereign), 80);
      }
    });
  }
}

#[test]
fn behavior_runtime_accumulated_count_commits_only_with_its_action_and_close() {
  use super::behavior_reference as oracle;
  for (actor_type, collector_refusal, require_normal_completion) in [
    (ActorType::User, false, false),
    (ActorType::User, true, false),
    (ActorType::System, false, false),
    (ActorType::User, false, true),
    (ActorType::User, true, true),
    (ActorType::System, false, true),
  ] {
    new_test_ext().execute_with(|| {
      let step_count = if require_normal_completion { 2 } else { 3 };
      let steps = BoundedVec::try_from(vec![transfer_step(None); step_count]).unwrap();
      let expression = if require_normal_completion {
        // Same conjunction as reference runtime productive-close fixtures; reaching Action success
        // alone must not close before plan end. Model prices do not qualify public admission.
        super::behavior_policy::expression(vec![vec![
          crate::BehaviorAtom {
            metric: crate::BehaviorMetric::ActionSuccessTotal,
            threshold: 1,
          },
          crate::BehaviorAtom {
            metric: crate::BehaviorMetric::CycleCompletedTotal,
            threshold: 1,
          },
        ]])
      } else {
        single_metric(crate::BehaviorMetric::ActionSuccessTotal, 2)
      };
      let (actor, contract) =
        prepared_policy_source_for_class(steps, expression.clone(), actor_type);
      let sovereign = sovereign_account(actor.actor_id);
      fund_native(actor.actor_id, 10_000);
      let custody = native_balance(&sovereign);
      let bob = native_balance(&BOB);
      let sink = native_balance(&TestFeeSink::get());
      let pipeline_fee = crate::ActorContractHeads::<Test>::get(actor.actor_id)
        .unwrap()
        .header
        .pipeline_machine_envelope
        .pipeline_machine_fee_upper;
      let action_fee = if actor_type == ActorType::User {
        100
      } else {
        0
      };
      let simulate = |mode| {
        let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        let result = Actors::simulate_current_contract(
          actor.actor_id,
          actor_type,
          Mutability::Mutable,
          contract.clone(),
          mode,
          ample_simulation_budget(),
        );
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
          root
        );
        result
      };
      System::set_block_number(2);
      begin_prepared_round(2);
      let opening = simulate(SimulationMode::FreshCurrentPlan).unwrap();
      assert_eq!(opening.status, crate::AttemptDisposition::Continued);
      assert_eq!(opening.start_cursor, 0);
      assert_eq!(opening.steps.len(), 1);
      assert_eq!(opening.steps[0].outcome, crate::StepOutcome::Executed);
      clear_fee_collections();
      let first = execute_prepared_step(actor, 2)
        .unwrap_or_else(|error| panic!("step refused: {:?}", error.cause));
      assert!(!first.attempt.as_ref().unwrap().is_closed());
      let run = Actors::actor_run_state(actor.actor_id).unwrap();
      assert_eq!(opening.run_cursor, Some(run.cursor));
      assert_eq!(opening.cycle_nonce, run.cycle_nonce);
      assert_eq!(opening.cumulative_outcomes, run.cumulative_outcomes);
      let attempt = oracle::Transition::Attempt {
        check: oracle::Check::Absent,
        action: oracle::Action::Success,
        disposition: oracle::Disposition::Advance,
      };
      let mut trace = vec![
        oracle::Record::committed(oracle::Transition::Open),
        oracle::Record::committed(attempt.clone()),
      ];
      let kinds = vec![oracle::StepKind::Economic; step_count];
      assert!(!assert_runtime_policy_trace(
        actor,
        &kinds,
        &trace,
        &expression
      ));
      let monitor = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id).unwrap();
      assert_eq!(
        fee_collections(),
        if actor_type == ActorType::User {
          vec![pipeline_fee, action_fee]
        } else {
          vec![]
        }
      );
      assert!(monitor.clean_cycle.is_none());
      assert_eq!(
        native_balance(&sovereign),
        custody - 10 - pipeline_fee - action_fee
      );
      assert_eq!(native_balance(&BOB), bob + 10);
      assert_eq!(
        native_balance(&TestFeeSink::get()),
        sink + pipeline_fee + action_fee
      );
      System::set_block_number(3);
      begin_prepared_round(3);
      let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      clear_fee_collections();
      if collector_refusal {
        set_fail_fee_sink_transfer_from(Some(sovereign));
      } else {
        set_step_control_actual_weight_override(Some(Weight::MAX));
      }
      assert_eq!(
        simulate(SimulationMode::CurrentRun),
        Err(if collector_refusal {
          crate::SimulationError::FeeCollectionFailed
        } else {
          crate::SimulationError::Classification(crate::ActorClassificationError::ActorInvariant)
        })
      );
      clear_fee_collections();
      let Err(error) = execute_prepared_step(actor, 3) else {
        panic!("late control or collector refusal must roll back");
      };
      assert_eq!(
        error.cause,
        if collector_refusal {
          crate::scheduler::AttemptTransactionError::FeeCollection
        } else {
          crate::scheduler::AttemptTransactionError::Invariant
        }
      );
      assert!(error.actual_effect_weight.is_some());
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        before
      );
      assert_eq!(
        crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id),
        Some(monitor)
      );
      assert_eq!(
        native_balance(&sovereign),
        custody - 10 - pipeline_fee - action_fee
      );
      assert_eq!(native_balance(&BOB), bob + 10);
      assert_eq!(
        native_balance(&TestFeeSink::get()),
        sink + pipeline_fee + action_fee
      );
      assert_eq!(
        fee_collections(),
        if collector_refusal {
          vec![action_fee]
        } else {
          vec![]
        }
      );
      trace.push(oracle::Record::rolled_back(attempt.clone()));
      assert!(!assert_runtime_policy_trace(
        actor,
        &kinds,
        &trace,
        &expression
      ));
      set_step_control_actual_weight_override(None);
      set_fail_fee_sink_transfer_from(None);
      let terminal = simulate(SimulationMode::CurrentRun).unwrap();
      assert_eq!(
        terminal.status,
        crate::AttemptDisposition::Closed(CloseReason::BehaviorPolicySatisfied)
      );
      assert_eq!(terminal.start_cursor, 1);
      assert_eq!(terminal.steps.len(), 1);
      assert_eq!(terminal.steps[0].outcome, crate::StepOutcome::Executed);
      clear_fee_collections();
      let second = execute_prepared_step(actor, 3)
        .unwrap_or_else(|error| panic!("step refused: {:?}", error.cause));
      assert!(second.attempt.as_ref().unwrap().is_closed());
      assert!(terminal.run_cursor.is_none());
      assert_eq!(terminal.cumulative_outcomes.executed_steps, 2);
      assert_eq!(terminal.cumulative_outcomes.committed_effectful_tasks, 2);
      assert_eq!(
        second.attempt.as_ref().unwrap().normal_cycle_completion,
        require_normal_completion.then_some(crate::NormalCycleCompletion::PlanEnd)
      );
      trace.push(oracle::Record::committed(attempt));
      assert!(assert_runtime_policy_trace(
        actor,
        &kinds,
        &trace,
        &expression
      ));
      assert!(!crate::ActorBehaviorMonitors::<Test>::contains_key(
        actor.actor_id
      ));
      assert_eq!(
        native_balance(&sovereign),
        custody - 20 - pipeline_fee - 2 * action_fee
      );
      assert_eq!(native_balance(&BOB), bob + 20);
      assert_eq!(
        native_balance(&TestFeeSink::get()),
        sink + pipeline_fee + 2 * action_fee
      );
      assert_eq!(
        fee_collections(),
        if actor_type == ActorType::User {
          vec![action_fee]
        } else {
          vec![]
        }
      );
    });
  }
}

#[test]
fn behavior_runtime_zero_step_source_authentication_is_read_only() {
  #[derive(Clone, Copy, Debug)]
  enum Mismatch {
    Generation,
    Residence,
    Identity,
    Hot,
    Admission,
    Header,
    Body,
  }
  for actor_type in [ActorType::User, ActorType::System] {
    for windowed in [false, true] {
      new_test_ext().execute_with(|| {
        let (actor, _) = prepared_policy_source_with_window(
          BoundedVec::default(),
          single_metric(crate::BehaviorMetric::CycleCompletedTotal, 2),
          actor_type,
          true,
          windowed.then_some(crate::ScheduleWindow {
            start: 1,
            end: 1_000,
          }),
        );
        fund_native(actor.actor_id, 1_000);
        System::set_block_number(2);
        begin_prepared_round(2);
        let state = Actors::active_actor_state(actor.actor_id).unwrap();
        let admission = Actors::load_control_admission(actor.actor_id).unwrap();
        let kind = crate::ServiceNodes::<Test>::get(actor.actor_id)
          .unwrap()
          .kind;
        let semantic = Actors::load_service_actor_semantic_state(actor, kind).unwrap();
        let root = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        assert_eq!(
          Actors::authenticate_zero_step_service_source(actor, kind, &state, &admission).unwrap(),
          semantic,
        );
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
          root
        );
        for mismatch in [
          Mismatch::Generation,
          Mismatch::Residence,
          Mismatch::Identity,
          Mismatch::Hot,
          Mismatch::Admission,
          Mismatch::Header,
          Mismatch::Body,
        ] {
          // Change only supplied arguments: retained consensus source and readiness stay intact.
          let mut supplied_actor = actor;
          let mut supplied_kind = kind;
          let mut supplied_state = state.clone();
          let mut supplied_admission = admission.clone();
          match mismatch {
            Mismatch::Generation => supplied_actor.generation += 1,
            Mismatch::Residence => {
              supplied_kind = match kind {
                crate::ServiceResidenceKind::Live => crate::ServiceResidenceKind::Pending,
                crate::ServiceResidenceKind::Pending => crate::ServiceResidenceKind::Live,
              }
            }
            Mismatch::Identity => supplied_state.identity.cycle_nonce += 1,
            Mismatch::Hot => supplied_state.hot.pending_signal = !state.hot.pending_signal,
            Mismatch::Admission => supplied_admission.admission_identity[0] ^= 1,
            Mismatch::Header => supplied_state.contract.auto_close_at_cycle_nonce = Some(100),
            Mismatch::Body => supplied_state
              .contract
              .steps
              .try_push(transfer_step(None))
              .unwrap(),
          }
          assert_eq!(
            Actors::authenticate_zero_step_service_source(
              supplied_actor,
              supplied_kind,
              &supplied_state,
              &supplied_admission,
            )
            .unwrap_err(),
            crate::scheduler::AttemptTransactionError::Invariant,
            "{mismatch:?}",
          );
          assert_eq!(
            polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
            root
          );
          assert_eq!(
            Actors::execute_zero_step_on_service(
              supplied_actor,
              supplied_kind,
              supplied_state,
              &supplied_admission,
              2,
            )
            .err()
            .expect("caller mismatch must refuse"),
            crate::scheduler::AttemptTransactionError::Invariant,
            "{mismatch:?}",
          );
          assert_eq!(
            polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
            root
          );
        }
        // No repair, new occurrence or new round is needed after caller rejection.
        let evidence =
          Actors::execute_zero_step_on_service(actor, kind, state, &admission, 2).unwrap();
        assert_eq!(evidence.actual_effect_weight, Weight::zero());
        assert!(!evidence.attempt.unwrap().is_closed());
      });
    }
  }
}

#[test]
fn behavior_runtime_zero_step_uses_policy_pipeline_price_and_atomic_actual_settlement() {
  use super::behavior_reference::{Record, Transition};
  #[derive(Clone, Copy, Debug)]
  enum EvidenceCase {
    Valid,
    MissingMaximum,
    MissingActual,
    RefTimeExcess,
    ProofSizeExcess,
  }
  for (closes, windowed) in [(true, false), (false, false), (true, true), (false, true)] {
    for case in [
      EvidenceCase::Valid,
      EvidenceCase::MissingMaximum,
      EvidenceCase::MissingActual,
      EvidenceCase::RefTimeExcess,
      EvidenceCase::ProofSizeExcess,
    ] {
      new_test_ext().execute_with(|| {
        let expression = single_metric(
          crate::BehaviorMetric::CycleCompletedTotal, if closes { 1 } else { 2 },
        );
        let (actor, contract) = prepared_policy_source_with_window(
          BoundedVec::default(), expression.clone(), ActorType::User, true,
          windowed.then_some(crate::ScheduleWindow { start: 1, end: 1_000 }),
        );
        let sovereign = sovereign_account(actor.actor_id);
        fund_native(actor.actor_id, 1_000);
        let resources = Actors::zero_step_resource_envelope(&contract).unwrap();
        let placement = Actors::idle_window_placement_weight(&contract);
        let host_maximum = resources.control.checked_sub(&placement).unwrap();
        let priced_control = resources.control
          .checked_add(&Actors::policy_service_inspection_weight(&contract).unwrap()).unwrap();
        let fee = <Test as crate::Config>::WeightToFee::weight_to_fee(&priced_control);
        let head = crate::ActorContractHeads::<Test>::get(actor.actor_id).unwrap();
        assert_eq!(head.header.pipeline_machine_envelope.pipeline_machine_fee_upper, fee);
        System::set_block_number(2);
        begin_prepared_round(2);
        let state = Actors::active_actor_state(actor.actor_id).unwrap();
        let owner = state.identity.owner;
        let admission = Actors::load_control_admission(actor.actor_id).unwrap();
        let kind = crate::ServiceNodes::<Test>::get(actor.actor_id).unwrap().kind;
        let hold = crate::ActorStateHolds::<Test>::get(actor.actor_id);
        let slots = crate::OwnerSlotBitmaps::<Test>::get(owner);
        let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
        let custody_before = native_balance(&sovereign);
        let sink_before = native_balance(&TestFeeSink::get());
        let actual_override = match case {
          EvidenceCase::RefTimeExcess => Some(Weight::from_parts(
            host_maximum.ref_time().checked_add(1).unwrap(), host_maximum.proof_size(),
          )),
          EvidenceCase::ProofSizeExcess => Some(Weight::from_parts(
            host_maximum.ref_time(), host_maximum.proof_size().checked_add(1).unwrap(),
          )),
          _ => None,
        };
        if let Some(actual) = actual_override {
          assert!(!actual.all_lte(host_maximum));
          if closes && windowed {
            // This would fit the combined envelope: unused placement cannot subsidize
            // an oversized host result after the Actor has already been destroyed.
            assert!(actual.all_lte(resources.control));
          }
        }
        set_missing_behavior_control_weight(matches!(case, EvidenceCase::MissingMaximum));
        set_missing_step_control_actual_weight(matches!(case, EvidenceCase::MissingActual));
        set_step_control_actual_weight_override(actual_override);
        let result = Actors::execute_zero_step_on_service(actor, kind, state, &admission, 2);
        set_missing_behavior_control_weight(false);
        set_missing_step_control_actual_weight(false);
        set_step_control_actual_weight_override(None);
        let mut trace = Vec::new();
        let evidence = if !matches!(case, EvidenceCase::Valid) {
          let Err(cause) = result else { panic!("invalid evidence must refuse: {case:?}") };
          assert_eq!(cause, crate::scheduler::AttemptTransactionError::Invariant);
          assert_eq!(polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1), before);
          assert_eq!(native_balance(&sovereign), custody_before);
          assert_eq!(native_balance(&TestFeeSink::get()), sink_before);
          assert_eq!(crate::ActorStateHolds::<Test>::get(actor.actor_id), hold);
          assert_eq!(crate::OwnerSlotBitmaps::<Test>::get(owner), slots);
          assert_eq!(Actors::load_actor_contract(actor.actor_id), Some(contract.clone()));
          assert_eq!(Actors::load_control_admission(actor.actor_id), Some(admission.clone()));
          trace.push(Record::rolled_back(Transition::Open));
          // Repair the same paid activation; no second Trigger or additional readiness fee.
          let state = Actors::active_actor_state(actor.actor_id).unwrap();
          Actors::execute_zero_step_on_service(actor, kind, state, &admission, 2).unwrap()
        } else { result.unwrap() };
        trace.push(Record::committed(Transition::Open));
        assert_eq!(assert_runtime_policy_trace(actor, &[], &trace, &expression), closes);
        assert_eq!(evidence.attempt.as_ref().unwrap().is_closed(), closes);
        let unused_placement = if closes { placement } else { Weight::zero() };
        assert_eq!(evidence.actual_control_weight, resources.control.checked_sub(&unused_placement).unwrap());
        assert_eq!(evidence.actual_effect_weight, Weight::zero());
        assert_eq!(native_balance(&sovereign), custody_before - fee);
        assert_eq!(native_balance(&TestFeeSink::get()), sink_before + fee);
        assert!(!crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
        assert_eq!(crate::DeadlineHandles::<Test>::get(actor.actor_id).map(|h| h.key),
          (!closes && windowed).then_some(crate::WakeupKey::Block(1_001)));
        let events = System::events();
        assert_eq!(events.iter().filter(|record| matches!(record.event,
          RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id, .. }) if actor_id == actor.actor_id
        )).count(), 1);
        assert_eq!(events.iter().filter(|record| matches!(record.event,
          RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed { actor_id, .. }) if actor_id == actor.actor_id
        )).count(), 1);
        assert!(!events.iter().any(|record| matches!(record.event,
          RuntimeEvent::Actors(Event::ActionFeeCharged { actor_id, .. }) if actor_id == actor.actor_id
        )));
      });
    }
  }
}

#[test]
fn behavior_runtime_zero_step_normal_completion_uses_the_same_policy_reducer() {
  use super::{behavior_policy, behavior_reference as oracle};
  use crate::{BehaviorMetric::*, BehaviorTrackingMode};
  #[derive(Clone, Copy, Debug)]
  enum Case {
    NoMonitorFalse,
    NoMonitorClose,
    CleanOnlyFalse,
    CompletedOrClean,
    Accumulated,
    AccumulatedClean,
    PolicyBeforeNonce,
    NonceOnly,
  }
  let completed_or_clean = |threshold| {
    let mut expression = single_metric(CycleCompletedTotal, threshold);
    expression
      .clauses
      .try_push(single_metric(CycleCleanTotal, 1).clauses[0].clone())
      .unwrap();
    expression
  };
  for actor_type in [ActorType::User, ActorType::System] {
    for case in [
      Case::NoMonitorFalse,
      Case::NoMonitorClose,
      Case::CleanOnlyFalse,
      Case::CompletedOrClean,
      Case::Accumulated,
      Case::AccumulatedClean,
      Case::PolicyBeforeNonce,
      Case::NonceOnly,
    ] {
      new_test_ext().execute_with(|| {
        let expression = match case {
          Case::NoMonitorFalse => single_metric(ActionSuccessTotal, 1),
          Case::NoMonitorClose => single_metric(CycleCompletedTotal, 1),
          Case::CleanOnlyFalse | Case::NonceOnly => single_metric(CycleCleanTotal, 1),
          Case::CompletedOrClean | Case::PolicyBeforeNonce => completed_or_clean(1),
          Case::Accumulated => single_metric(CycleCompletedTotal, 2),
          Case::AccumulatedClean => completed_or_clean(2),
        };
        expression.validate().unwrap();
        let accumulated = matches!(case, Case::Accumulated | Case::AccumulatedClean);
        let clean = !matches!(case, Case::NoMonitorFalse | Case::NoMonitorClose | Case::Accumulated);
        let nonce = matches!(case, Case::PolicyBeforeNonce | Case::NonceOnly).then_some(1);
        let (actor, contract) = prepared_policy_source_with_targets(
          BoundedVec::default(), expression.clone(), actor_type, true, None, nonce,
        );
        // One additive funding act before the first Opening, never a post-Cycle balance reset.
        fund_native(actor.actor_id, 10_000);
        let sovereign = sovereign_account(actor.actor_id);
        let owner = Actors::actor_identity(actor.actor_id).unwrap().owner;
        let hold = crate::ActorStateHolds::<Test>::get(actor.actor_id);
        let slots = crate::OwnerSlotBitmaps::<Test>::get(owner);
        let admission = Actors::load_control_admission(actor.actor_id).unwrap();
        let pipeline = Actors::pipeline_fee_for_actor(actor.actor_id, actor_type).unwrap().total_fee;
        assert_eq!(pipeline == 0, actor_type == ActorType::System);
        let mut trace = Vec::new();
        for cycle in 1..=2u64 {
          if cycle > 1 {
            System::set_block_number(3);
            assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(owner), actor.actor_id));
          }
          let now = cycle * 2;
          System::set_block_number(now);
          begin_prepared_round(now);
          let state = Actors::active_actor_state(actor.actor_id).unwrap();
          assert_eq!(state.identity.cycle_nonce, cycle - 1);
          assert!(state.hot.pending_signal);
          assert!(state.run_state.is_none());
          let policy = crate::ActorBehaviorPolicies::<Test>::get(actor.actor_id).unwrap();
          assert_eq!(policy.plan.mode(), if accumulated { BehaviorTrackingMode::Accumulated } else { BehaviorTrackingMode::CurrentOutcome });
          assert_eq!(policy.plan.requires_clean_cycle(), clean);
          let monitor = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id);
          assert_eq!(monitor.is_some(), accumulated || clean);
          if let Some(monitor) = &monitor {
            assert!(monitor.clean_cycle.is_none());
            let bound = policy.plan.bind(&policy.expression, crate::BehaviorPolicyBinding {
              semantic_contract_id: admission.semantic_contract_id,
              admission_identity: admission.admission_identity,
            }).unwrap();
            let episode = (b"DEOS_ACTOR_BEHAVIOR_EPISODE", actor.actor_id,
              policy.episode_started_at_generation, admission.semantic_contract_id)
              .using_encoded(polkadot_sdk::sp_io::hashing::blake2_256);
            if !accumulated {
              // CurrentOutcome clean monitors authenticate a zero-slot state, not busy flags.
              assert_eq!(monitor.state, bound.begin_episode(episode).unwrap());
            }
          }
          let payer_before = native_balance(&sovereign);
          let sink_before = native_balance(&TestFeeSink::get());
          let kind = crate::ServiceNodes::<Test>::get(actor.actor_id).unwrap().kind;
          let evidence = Actors::execute_zero_step_on_service(actor, kind, state, &admission, now).unwrap();
          assert_eq!(evidence.actual_effect_weight, Weight::zero());
          let attempt = evidence.attempt.unwrap();
          assert_eq!(attempt.normal_cycle_completion, Some(crate::NormalCycleCompletion::PlanEnd));
          assert_eq!(attempt.clean_cycle_step, crate::CleanCycleStep::Neutral);
          assert!(has_actor_event(|event| matches!(event,
            Event::CycleSummary { actor_id, cycle_nonce, result: crate::CycleResult::Completed, outcomes }
              if *actor_id == actor.actor_id && *cycle_nonce == cycle && *outcomes == crate::OutcomeTotals::default()
          )));
          assert_eq!(native_balance(&sovereign), payer_before - pipeline);
          assert_eq!(native_balance(&TestFeeSink::get()), sink_before + pipeline);
          trace.push(oracle::Record::committed(oracle::Transition::Open));
          let projected = oracle::project(&[], &trace, Some(&behavior_policy::oracle_policy(&expression)));
          assert_eq!(projected.value(oracle::Metric::CycleCleanTotal), 0);
          assert_eq!(projected.value(oracle::Metric::CycleCompletedTotal), cycle);
          let expected_reason = if projected.closed {
            Some(CloseReason::BehaviorPolicySatisfied)
          } else if nonce.is_some_and(|target| cycle >= target) {
            Some(CloseReason::AutoCloseNonceReached)
          } else { None };
          assert_eq!(attempt.is_closed(), expected_reason.is_some(), "{case:?}");
          if let Some(reason) = expected_reason {
            assert!(has_actor_event(|event| matches!(event,
              Event::ActorClosed { actor_id, reason: actual } if *actor_id == actor.actor_id && *actual == reason
            )));
            assert!(Actors::actor_identity(actor.actor_id).is_none());
            assert!(!crate::ActorBehaviorMonitors::<Test>::contains_key(actor.actor_id));
            assert!(!crate::ActorStateHolds::<Test>::contains_key(actor.actor_id));
            if actor_type == ActorType::User {
              assert_ne!(crate::OwnerSlotBitmaps::<Test>::get(owner), slots);
            } else {
              assert_eq!(crate::OwnerSlotBitmaps::<Test>::get(owner), slots);
            }
            if projected.closed { assert!(assert_runtime_policy_trace(actor, &[], &trace, &expression)); }
          } else {
            assert!(!assert_runtime_policy_trace(actor, &[], &trace, &expression));
            let after = crate::ActorBehaviorMonitors::<Test>::get(actor.actor_id);
            assert!(after.as_ref().is_none_or(|monitor| monitor.clean_cycle.is_none()));
            if !accumulated {
              assert_eq!(after, monitor, "current-outcome Idle stores no history or busy clean flags");
            }
            assert_eq!(Actors::load_actor_contract(actor.actor_id), Some(contract.clone()));
            assert_eq!(Actors::load_control_admission(actor.actor_id), Some(admission.clone()));
            assert_eq!(crate::ActorStateHolds::<Test>::get(actor.actor_id), hold);
            assert_eq!(crate::OwnerSlotBitmaps::<Test>::get(owner), slots);
          }
          assert!(!crate::ServiceNodes::<Test>::contains_key(actor.actor_id));
          assert!(!crate::ActorRunStateStore::<Test>::contains_key(actor.actor_id));
          let events = System::events();
          assert_eq!(events.iter().filter(|record| matches!(record.event,
            RuntimeEvent::Actors(Event::PipelineFeeCharged { actor_id, .. }) if actor_id == actor.actor_id
          )).count(), if actor_type == ActorType::User { cycle as usize } else { 0 });
          assert_eq!(events.iter().filter(|record| matches!(record.event,
            RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed { actor_id, .. }) if actor_id == actor.actor_id
          )).count(), cycle as usize);
          assert!(!events.iter().any(|record| matches!(record.event,
            RuntimeEvent::Actors(Event::ActionFeeCharged { actor_id, .. }) if actor_id == actor.actor_id
          )));
          if expected_reason.is_some() { break; }
        }
      });
    }
  }
}
