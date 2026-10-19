use super::*;
use crate::weights::WeightInfo as _;
use crate::{
  ActorContractHeads, ActorContractTailChunks, ActorProcesses, ActorRef, ActorSemanticState,
  ActorSemanticStates, ActorStepResourceEnvelope, BlockResourceDomain, BlockResourceState,
  ServiceHeader, ServiceNodes, ServiceRoundEncounter,
};

fn prepare_head(class: ActorType, steps: crate::ContractSteps<Test>) -> (ActorRef, u64) {
  System::set_block_number(1);
  let id = match class {
    ActorType::User => create_user_with(ALICE, Mutability::Mutable, manual_schedule(), None, steps),
    ActorType::System => create_system_with(ALICE, manual_schedule(), None, steps),
  };
  fund_native(id, 1_000_000);
  assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), id));
  let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(id).unwrap() else {
    panic!("public creation installs Active authority");
  };
  let now = System::block_number() + 1;
  System::set_block_number(now);
  (
    ActorRef {
      actor_id: id,
      generation: record.generation,
    },
    now,
  )
}

fn root() -> Vec<u8> {
  polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1)
}

fn read_only_allowance(now: u64) -> Option<ActorStepResourceEnvelope> {
  let before = root();
  let header = ServiceHeader::<Test>::get();
  let fees = fee_collections();
  let result = Actors::service_head_allowance_diagnostic(now);
  assert_eq!(root(), before, "quote must preserve committed state");
  assert_eq!(
    ServiceHeader::<Test>::get(),
    header,
    "quote cannot open a round"
  );
  assert_eq!(
    fee_collections(),
    fees,
    "quote cannot invoke the fee collector"
  );
  result
}

#[test]
fn service_head_allowance_diagnostic_covers_canonical_encounters() {
  type W = <Test as crate::Config>::WeightInfo;
  for class in [ActorType::System, ActorType::User] {
    for stop in [false, true] {
      for disposition in 0..4 {
        new_test_ext().execute_with(|| {
          let step = if stop {
            make_step(Task::StopCycle)
          } else {
            transfer_contract_steps(BOB, 1)[0].clone()
          };
          let (actor, now) = prepare_head(class, contract_steps_with_step(step));
          let account = sovereign_account(actor.actor_id);
          match disposition {
            // A retained-unlatched native boundary, not a new public ingress route.
            1 => mutate_actor_hot_coherent(actor.actor_id, |hot| hot.pending_signal = false),
            2 => mutate_actor_hot_coherent(actor.actor_id, |hot| {
              hot.unsuccessful_attempt_streak =
                <Test as crate::Config>::MaxConsecutiveFailures::get();
            }),
            3 => deplete_user_sovereign(actor.actor_id, native_balance(&account)),
            _ => {}
          }
          let allowance = read_only_allowance(now).expect("ordinary eligible head");
          let loaded = Actors::load_current_step_service_state(actor.actor_id)
            .unwrap()
            .2;
          let resources = if stop {
            ActorStepResourceEnvelope {
              control: loaded
                .resources
                .control
                .checked_add(&loaded.resources.effect)
                .unwrap(),
              effect: Weight::zero(),
            }
          } else {
            loaded.resources
          };
          let suffix = W::service_round_admit_eligible()
            .max(W::service_member_retire_interior())
            .max(W::service_member_retire_pair_cursor())
            .max(W::service_member_retire_singleton());
          let expected_control = W::service_round_begin_populated()
            .checked_add(&W::service_round_probe_eligible())
            .unwrap()
            .checked_add(&W::scheduler_actor_state_probe())
            .unwrap()
            .checked_add(&resources.control)
            .unwrap()
            .checked_add(&W::close_actor())
            .unwrap()
            .checked_add(&suffix)
            .unwrap();
          assert_eq!(allowance.control, expected_control);
          assert_eq!(allowance.effect, resources.effect);
          assert!(ServiceHeader::<Test>::get().round_block < Some(now));

          let budget = <Test as crate::Config>::BlockResourceBudget::get();
          let mut state = BlockResourceState::new(now);
          assert_ok!(state.begin_prepass(budget));
          assert_ok!(state.open_external_phase());
          assert_ok!(state.begin_drain());
          let mut meter =
            WeightMeter::with_limit(allowance.control.checked_add(&allowance.effect).unwrap());
          let encounter = Actors::service_canonical_round_head_with_resources(
            &mut meter,
            now,
            &mut state,
            budget.limits(),
            BlockResourceDomain::ActorDrainEffect,
          )
          .expect("the protected existing admission must fit");
          match disposition {
            1 => assert_eq!(encounter, ServiceRoundEncounter::NoWork(actor)),
            2 => assert_eq!(
              encounter,
              ServiceRoundEncounter::TerminallyClosed(CloseReason::ConsecutiveFailures),
            ),
            3 if class == ActorType::User => assert_eq!(
              encounter,
              ServiceRoundEncounter::TerminallyClosed(CloseReason::CycleAdmissionInsufficient),
            ),
            _ => assert_eq!(encounter, ServiceRoundEncounter::Eligible(actor)),
          }
          assert!(
            state
              .usage()
              .actor_control_used()
              .all_lte(allowance.control)
          );
          assert!(state.usage().actor_effect_used().all_lte(allowance.effect));
          assert_eq!(state.outstanding_reservations(), 0);
        });
      }
    }
  }
}

#[test]
fn service_head_allowance_diagnostic_current_fragment_and_rollback_are_fresh() {
  for class in [ActorType::System, ActorType::User] {
    for suspended in [false, true] {
      new_test_ext().execute_with(|| {
        setup_temporary_retry_pool();
        let mut retry = make_step(Task::SwapIn {
          asset_in: TestAsset::Native,
          asset_out: TestAsset::Local(77),
          amount_in: AmountResolution::Fixed(10),
          slippage_tolerance: Perbill::one(),
        });
        retry.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
        let mut steps = vec![transfer_contract_steps(BOB, 1)[0].clone(); 8];
        steps[1] = retry;
        let (actor, first) = prepare_head(class, steps.try_into().unwrap());
        run_canonical_block_at(first, Weight::MAX);
        if suspended {
          set_temporary_dex_failure(true);
          run_canonical_block_at(first + 1, Weight::MAX);
          assert_eq!(
            Actors::actor_hot(actor.actor_id).unwrap().cycle_state,
            CycleState::Suspended
          );
        } else {
          assert_eq!(
            Actors::actor_hot(actor.actor_id).unwrap().cycle_state,
            CycleState::Running
          );
        }
        let now = System::block_number() + 1;
        System::set_block_number(now);
        assert_eq!(Actors::actor_run_state(actor.actor_id).unwrap().cursor, 1);
        let original = read_only_allowance(now).unwrap();
        // Deliberate read-isolation falsifier, not a valid published missing-tail history.
        let unreached = ActorContractTailChunks::<Test>::take(actor.actor_id, 1).unwrap();
        assert_eq!(read_only_allowance(now), Some(original));
        let current = ActorContractTailChunks::<Test>::take(actor.actor_id, 0).unwrap();
        assert_eq!(read_only_allowance(now), None);
        ActorContractTailChunks::<Test>::insert(actor.actor_id, 0, current);
        for delta in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
          let before = root();
          polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
            ActorContractTailChunks::<Test>::mutate(actor.actor_id, 0, |chunk| {
              let resources = &mut chunk.as_mut().unwrap().step_resources[0];
              resources.control = resources.control.checked_add(&delta).unwrap();
            });
            let fresh = read_only_allowance(now).unwrap();
            assert_eq!(fresh.control, original.control.checked_add(&delta).unwrap());
            polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
          });
          assert_eq!(root(), before);
          assert_eq!(read_only_allowance(now), Some(original));
        }
        ActorContractTailChunks::<Test>::insert(actor.actor_id, 1, unreached);
      });
    }
  }
}

#[test]
fn service_head_allowance_diagnostic_refuses_local_faults_and_unsupported_heads() {
  new_test_ext().execute_with(|| assert_eq!(read_only_allowance(1), None));
  for class in [ActorType::System, ActorType::User] {
    new_test_ext().execute_with(|| {
      let (_, now) = prepare_head(class, Default::default());
      assert_eq!(
        read_only_allowance(now),
        None,
        "zero-Step remains ordinary Service-owned"
      );
    });
  }
  for fault in 0..23 {
    new_test_ext().execute_with(|| {
      let (actor, now) = prepare_head(ActorType::User, transfer_contract_steps(BOB, 1));
      let id = actor.actor_id;
      assert!(read_only_allowance(now).is_some());
      match fault {
        0 => ServiceNodes::<Test>::mutate(id, |v| v.as_mut().unwrap().last_considered = now),
        1 => ServiceNodes::<Test>::mutate(id, |v| v.as_mut().unwrap().eligible_from = now + 1),
        2 => ServiceHeader::<Test>::mutate(|v| v.round_block = Some(now + 1)),
        3 => ActorProcesses::<Test>::mutate(id, |v| v.as_mut().unwrap().last_attempted = Some(now)),
        4 => ServiceNodes::<Test>::mutate(id, |v| v.as_mut().unwrap().generation += 1),
        5 => ActorSemanticStates::<Test>::mutate(id, |v| {
          let Some(ActorSemanticState::Active(record)) = v else {
            unreachable!()
          };
          record.generation += 1;
        }),
        6 => ActorContractHeads::<Test>::mutate(id, |v| {
          v.as_mut().unwrap().header.body_commitment[0] ^= 1;
        }),
        7 => ActorSemanticStates::<Test>::mutate(id, |v| {
          let Some(ActorSemanticState::Active(record)) = v else {
            unreachable!()
          };
          record.admission.production_weight_identity[0] ^= 1;
        }),
        8 => ActorContractHeads::<Test>::mutate(id, |v| {
          v.as_mut().unwrap().first_step_resources = None
        }),
        9 => ActorContractHeads::<Test>::mutate(id, |v| {
          v.as_mut().unwrap().header.has_close_when = true
        }),
        10 => ActorContractHeads::<Test>::mutate(id, |v| {
          v.as_mut().unwrap().header.on_unfunded = crate::OnUnfunded::AwaitFunding;
        }),
        11 => GlobalCircuitBreaker::<Test>::put(true),
        12 => ServiceNodes::<Test>::mutate(id, |v| v.as_mut().unwrap().next.generation += 1),
        13 => ActorContractHeads::<Test>::mutate(id, |v| {
          v.as_mut()
            .unwrap()
            .first_step_resources
            .as_mut()
            .unwrap()
            .control = Weight::MAX;
        }),
        14 => mutate_actor_hot_coherent(id, |hot| hot.cycle_state = CycleState::Running),
        15 => ServiceHeader::<Test>::mutate(|v| v.count = 0),
        16 => ActorProcesses::<Test>::mutate(id, |v| v.as_mut().unwrap().generation += 1),
        17 => ActorContractHeads::<Test>::mutate(id, |v| {
          v.as_mut().unwrap().header.trigger = Trigger::cadenced(1);
        }),
        18 => ActorContractHeads::<Test>::mutate(id, |v| {
          let maximum: u32 = <Test as crate::Config>::MaxContractSteps::get();
          v.as_mut().unwrap().header.step_count = maximum + 1;
        }),
        19 => {}
        20 => ServiceHeader::<Test>::mutate(|v| v.cursor.as_mut().unwrap().generation += 1),
        21 => ActorSemanticStates::<Test>::remove(id),
        22 => ActorProcesses::<Test>::mutate(id, |v| v.as_mut().unwrap().residence = None),
        _ => unreachable!(),
      }
      assert_eq!(
        read_only_allowance(if fault == 19 { now + 1 } else { now }),
        None
      );
    });
  }
}

#[test]
fn service_head_allowance_diagnostic_rejects_wait_before_optional_authentication() {
  let mut ext = new_test_ext();
  let (actor, now, before) = ext.execute_with(|| {
    let (actor, now) = prepare_head(ActorType::User, transfer_contract_steps(BOB, 1));
    assert!(read_only_allowance(now).is_some());
    // An unsupported process-kind fault must refuse before any episode authentication.
    ActorProcesses::<Test>::mutate(actor.actor_id, |process| {
      process.as_mut().unwrap().status = crate::ProcessStatus::AwaitingFunding;
    });
    (actor, now, root())
  });
  ext.commit_all().unwrap();
  let recorder =
    polkadot_sdk::sp_trie::recorder::Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
  ext.execute_with_recorder(recorder.clone(), || {
    assert_eq!(Actors::service_head_allowance_diagnostic(now), None);
  });
  let process_key = ActorProcesses::<Test>::hashed_key_for(actor.actor_id);
  assert!(recorder.recorded_keys().values().any(|keys| {
    keys
      .keys()
      .any(|recorded| recorded.as_ref() == process_key.as_slice())
  }));
  let forbidden = [
    crate::ActorFinancialWaits::<Test>::hashed_key_for(actor.actor_id),
    crate::ActorBehaviorPolicies::<Test>::hashed_key_for(actor.actor_id),
    crate::ActorBehaviorMonitors::<Test>::hashed_key_for(actor.actor_id),
    ActorContractHeads::<Test>::hashed_key_for(actor.actor_id),
    ActorRunStateStore::<Test>::hashed_key_for(actor.actor_id),
    ActorContractTailChunks::<Test>::hashed_key_for(actor.actor_id, 0),
  ];
  for key in forbidden {
    assert!(
      !recorder.recorded_keys().values().any(|keys| {
        keys
          .keys()
          .any(|recorded| recorded.as_ref() == key.as_slice())
      }),
      "unsupported process must not load optional/Contract/Run state"
    );
  }
  ext.execute_with(|| assert_eq!(root(), before));
}

#[test]
fn service_head_allowance_diagnostic_tracks_cursor_and_reciprocal_peers() {
  new_test_ext().execute_with(|| {
    let (first, _) = prepare_head(ActorType::User, transfer_contract_steps(BOB, 1));
    let second_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      second_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(second_id).unwrap()
    else {
      unreachable!()
    };
    let second = ActorRef {
      actor_id: second_id,
      generation: record.generation,
    };
    let now = System::block_number() + 1;
    System::set_block_number(now);
    let original = read_only_allowance(now).unwrap();
    assert!(original.effect.ref_time() > 0);
    ServiceHeader::<Test>::mutate(|header| header.count = 3);
    assert_eq!(
      read_only_allowance(now),
      None,
      "two-neighbor alias requires count two"
    );
    ServiceHeader::<Test>::mutate(|header| header.count = 2);
    let peer = ServiceNodes::<Test>::get(second_id).unwrap();
    ServiceNodes::<Test>::mutate(second_id, |v| v.as_mut().unwrap().next = second);
    assert_eq!(read_only_allowance(now), None);
    ServiceNodes::<Test>::insert(second_id, peer);
    assert_eq!(read_only_allowance(now), Some(original));
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_ok!(Actors::begin_service_round(now));
      assert_ok!(Actors::advance_service_head(first, now));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(ServiceHeader::<Test>::get().cursor, Some(second));
    assert_eq!(read_only_allowance(now).unwrap().effect, Weight::zero());
  });
}

// These token prices are conditional model inputs, not elapsed time, generated Weight or bounds
// on real host callbacks. No production caller or benchmark consumes this driver.
#[derive(Clone, Copy)]
struct QuantumModel {
  entry: Weight,
  guard: Weight,
  outer: Weight,
}

impl QuantumModel {
  fn tokens() -> Self {
    Self {
      entry: Weight::from_parts(7, 11),
      guard: Weight::from_parts(13, 17),
      outer: Weight::from_parts(19, 23),
    }
  }
}

#[derive(Debug, PartialEq)]
enum QuantumOutcome {
  BootstrapRefused,
  UnsupportedHead,
  UnitRefused,
  RolledBack,
  Committed,
}

#[derive(Debug)]
struct QuantumReport {
  outcome: QuantumOutcome,
  queries: u8,
  worker: Weight,
  productive: bool,
  post_supported: bool,
}

fn model_finalization() -> Weight {
  type W = <Test as crate::Config>::WeightInfo;
  W::scheduler_on_idle_base()
    .checked_add(&W::block_resource_finalize())
    .unwrap()
}

fn model_service_floor(state: &BlockResourceState<u64>) -> Weight {
  state
    .budget()
    .unwrap()
    .limits()
    .actor_control()
    .checked_sub(&Actors::mandatory_actor_control_weight_upper().unwrap())
    .unwrap()
}

fn sum_weights(weights: &[Weight]) -> Weight {
  weights
    .iter()
    .try_fold(Weight::zero(), |sum, weight| sum.checked_add(weight))
    .expect("finite model inputs do not overflow")
}

fn model_charge(state: &mut BlockResourceState<u64>, amount: Weight) {
  let limits = state.budget().unwrap().limits();
  let mut reservation = state
    .reserve(limits, BlockResourceDomain::ActorControl, amount)
    .unwrap();
  assert_ok!(state.settle(&mut reservation, amount));
}

// The parent admits/refusal-prices this capability while retaining U/F. The driver receives it
// prepaid: inability even to enter is a parent-owner gap, not free work or a production algorithm.
fn model_entry(
  state: &mut BlockResourceState<u64>,
  prices: QuantumModel,
) -> crate::BlockResourceReservation {
  state
    .reserve(
      state.budget().unwrap().limits(),
      BlockResourceDomain::ActorControl,
      sum_weights(&[
        prices.entry,
        model_service_floor(state),
        model_finalization(),
      ]),
    )
    .unwrap()
}

fn conditional_quantum(
  now: u64,
  tick: u64,
  state: &mut BlockResourceState<u64>,
  mut entry: crate::BlockResourceReservation,
  prices: QuantumModel,
) -> QuantumReport {
  use polkadot_sdk::frame_support::storage::{TransactionOutcome, with_transaction_unchecked};
  let limits = state.budget().unwrap().limits();
  assert_ok!(state.settle(&mut entry, prices.entry));
  let mut report = QuantumReport {
    outcome: QuantumOutcome::BootstrapRefused,
    queries: 0,
    worker: Weight::zero(),
    productive: false,
    post_supported: false,
  };
  let Ok(mut guard) = state.reserve(
    limits,
    BlockResourceDomain::ActorControl,
    sum_weights(&[
      prices.guard,
      model_service_floor(state),
      model_finalization(),
    ]),
  ) else {
    return report;
  };
  report.queries += 1;
  // Retain only scalar allowances, never loaded Actor authority across the worker/callback.
  let head = Actors::service_head_allowance_diagnostic(now);
  assert_ok!(state.settle(&mut guard, prices.guard));
  let Some(head) = head else {
    report.outcome = QuantumOutcome::UnsupportedHead;
    return report;
  };
  let worker_maximum = Actors::deadline_service_weight_upper();
  let Ok(mut unit) = state.reserve_actor_step(
    limits,
    BlockResourceDomain::ActorBaseEffect,
    sum_weights(&[
      prices.outer,
      worker_maximum,
      prices.guard,
      head.control,
      model_finalization(),
    ]),
    head.effect,
  ) else {
    report.outcome = QuantumOutcome::UnitRefused;
    return report;
  };
  let mut meter = WeightMeter::with_limit(worker_maximum);
  report.outcome = with_transaction_unchecked(|| {
    // Bounded peer metadata belongs to the model's outer token assumption, not a physical price.
    // Capture before the worker: a callback must not turn earlier corruption into "absence".
    let declared_empty = |clock| {
      crate::DeadlineIndexLen::<Test>::get(clock) == 0
        && crate::DeadlineIndexPages::<Test>::get(clock, 0).is_none()
    };
    let block_empty = declared_empty(WakeupClock::Block);
    let tick_empty = declared_empty(WakeupClock::Tick);
    let pass = Actors::service_due_deadline_frontiers(
      &mut meter,
      crate::ServiceResidenceKind::Live,
      now,
      tick,
      Some(WakeupKey::Block(now.checked_add(1).unwrap())),
    )
    .expect("complete worker envelope was admitted");
    report.worker = meter.consumed();
    report.queries += 1;
    let fresh = Actors::service_head_allowance_diagnostic(now);
    report.post_supported = fresh.is_some();
    // This is Rust-local state, deliberately not restored by storage rollback.
    assert_ok!(state.settle_actor_step(
      &mut unit,
      sum_weights(&[prices.outer, meter.consumed(), prices.guard]),
      Weight::zero(),
    ));
    // Missing storage is not alone evidence of an empty clock. Future/invalid destinations and
    // nonempty missing peers refuse the unit; this local check does not certify the whole namespace.
    let absent =
      crate::DependencyReviewWorkerError::Deadline(crate::DeadlineMutationError::MemberMissing);
    let success = (pass.block.is_ok() || pass.tick.is_ok())
      && (pass.block.is_ok() || (pass.block == Err(absent) && block_empty))
      && (pass.tick.is_ok() || (pass.tick == Err(absent) && tick_empty));
    report.productive = success;
    let fits = fresh.is_some_and(|head| {
      let mut admission = *state;
      admission
        .reserve_actor_step(
          limits,
          BlockResourceDomain::ActorBaseEffect,
          sum_weights(&[head.control, model_finalization()]),
          head.effect,
        )
        .is_ok()
    });
    if success && fits {
      TransactionOutcome::Commit(QuantumOutcome::Committed)
    } else {
      TransactionOutcome::Rollback(QuantumOutcome::RolledBack)
    }
  });
  report
}

fn prepare_quantum_prefix(class: ActorType) -> (ActorRef, [u64; 2], u64, BlockResourceState<u64>) {
  let (protected, _) = prepare_head(class, transfer_contract_steps(BOB, 1));
  let sources = [0; 2].map(|_| {
    let actor = create_user_with(
      ALICE,
      Mutability::Mutable,
      at_time_schedule(1),
      None,
      inert_contract_steps(),
    );
    fund_native(actor, 1_000_000);
    actor
  });
  let now = System::block_number() + 1;
  System::set_block_number(now);
  let budget = <Test as crate::Config>::BlockResourceBudget::get();
  let mut state = BlockResourceState::new(now);
  assert_ok!(state.begin_prepass(budget));
  let mut reservation = state
    .reserve(
      budget.limits(),
      BlockResourceDomain::ActorControl,
      Actors::deadline_service_weight_upper(),
    )
    .unwrap();
  let mut meter = WeightMeter::with_limit(Actors::deadline_service_weight_upper());
  let first = Actors::service_due_deadline_frontiers(
    &mut meter,
    crate::ServiceResidenceKind::Live,
    now,
    Actors::current_scheduler_tick().unwrap(),
    Some(WakeupKey::Block(now + 1)),
  )
  .unwrap();
  assert!(
    matches!(first.tick, Ok(crate::DueTickDeadlineMutation::TemporalTriggerProcessed(actor)) if actor.actor_id == sources[0])
  );
  assert_ok!(state.settle(&mut reservation, meter.consumed()));
  assert!(Actors::actor_hot(sources[0]).unwrap().pending_signal);
  assert!(!Actors::actor_hot(sources[1]).unwrap().pending_signal);
  assert_eq!(ServiceHeader::<Test>::get().cursor, Some(protected));
  clear_fee_collections();
  (protected, sources, now, state)
}

fn attempt_quantum(now: u64, state: &mut BlockResourceState<u64>) -> QuantumReport {
  let prices = QuantumModel::tokens();
  let entry = model_entry(state, prices);
  conditional_quantum(
    now,
    Actors::current_scheduler_tick().unwrap(),
    state,
    entry,
    prices,
  )
}

fn ordinary_service_remains(protected: ActorRef, now: u64, state: &mut BlockResourceState<u64>) {
  assert!(!state.optional_actor_work_halted());
  assert_eq!(state.outstanding_reservations(), 0);
  let limits = state.budget().unwrap().limits();
  assert_eq!(
    Actors::service_canonical_round_head_with_resources(
      &mut WeightMeter::with_limit(Weight::MAX),
      now,
      state,
      limits,
      BlockResourceDomain::ActorBaseEffect,
    ),
    Ok(ServiceRoundEncounter::Eligible(protected)),
  );
  let mut finalization = state
    .reserve(
      limits,
      BlockResourceDomain::ActorControl,
      model_finalization(),
    )
    .expect("finalization capacity survives");
  assert_ok!(state.settle(&mut finalization, Weight::zero()));
  assert_eq!(state.outstanding_reservations(), 0);
}

#[test]
fn service_head_allowance_quantum_rejects_missing_nonempty_peer_index() {
  new_test_ext().execute_with(|| {
    let (protected, sources, now, mut state) = prepare_quantum_prefix(ActorType::System);
    assert_eq!(crate::DeadlineIndexLen::<Test>::get(WakeupClock::Block), 0);
    assert!(crate::DeadlineIndexPages::<Test>::get(WakeupClock::Block, 0).is_none());
    // Deliberately inconsistent topology, not a claimed public setup history.
    crate::DeadlineIndexLen::<Test>::insert(WakeupClock::Block, 1);
    let before = root();
    let used = state.usage().actor_control_used();
    let report = attempt_quantum(now, &mut state);
    assert_eq!(report.outcome, QuantumOutcome::RolledBack);
    assert_eq!(
      root(),
      before,
      "a missing nonempty peer is not certified absence"
    );
    assert!(Actors::actor_hot(sources[0]).unwrap().pending_signal);
    assert!(!Actors::actor_hot(sources[1]).unwrap().pending_signal);
    assert!(state.usage().actor_control_used().all_gt(used));
    assert_eq!(state.outstanding_reservations(), 0);
    crate::DeadlineIndexLen::<Test>::remove(WakeupClock::Block);
    ordinary_service_remains(protected, now, &mut state);
  });
}

#[test]
fn service_head_allowance_quantum_bootstrap_prepays_before_query() {
  for class in [ActorType::System, ActorType::User] {
    for delta in [
      Weight::from_parts(1, 0),
      Weight::from_parts(0, 1),
      Weight::zero(),
    ] {
      new_test_ext().execute_with(|| {
        let (protected, _, now, mut state) = prepare_quantum_prefix(class);
        let prices = QuantumModel::tokens();
        let required = sum_weights(&[
          prices.entry,
          prices.guard,
          model_service_floor(&state),
          model_finalization(),
        ]);
        let available = state
          .budget()
          .unwrap()
          .limits()
          .actor_control()
          .checked_sub(&state.usage().actor_control_used())
          .unwrap();
        model_charge(
          &mut state,
          available
            .checked_sub(&required.checked_sub(&delta).unwrap())
            .unwrap(),
        );
        let before = root();
        let used = state.usage().actor_control_used();
        let report = attempt_quantum(now, &mut state);
        if delta == Weight::zero() {
          assert!(report.queries > 0, "exact bootstrap must admit the query");
        } else {
          assert_eq!(report.outcome, QuantumOutcome::BootstrapRefused);
          assert_eq!(report.queries, 0);
          assert_eq!(
            state.usage().actor_control_used(),
            used.checked_add(&prices.entry).unwrap()
          );
          assert_eq!(root(), before);
          assert!(fee_collections().is_empty());
        }
        ordinary_service_remains(protected, now, &mut state);
      });
    }
  }
}

#[test]
fn service_head_allowance_quantum_paired_effect_refusal_is_local() {
  for delta in [Weight::from_parts(1, 0), Weight::from_parts(0, 1)] {
    new_test_ext().execute_with(|| {
      let (protected, _, now, mut state) = prepare_quantum_prefix(ActorType::System);
      let prices = QuantumModel::tokens();
      let head = read_only_allowance(now).unwrap();
      let limits = state.budget().unwrap().limits();
      // Synthetic already committed base effects, under the same frozen Prepass allocation.
      let consumed = limits
        .actor_base_turn()
        .checked_sub(&head.effect.checked_sub(&delta).unwrap())
        .unwrap();
      let mut prior = state
        .reserve(limits, BlockResourceDomain::ActorBaseEffect, consumed)
        .unwrap();
      assert_ok!(state.settle(&mut prior, consumed));
      let before = root();
      let used = state.usage().actor_control_used();
      let report = attempt_quantum(now, &mut state);
      assert_eq!(report.outcome, QuantumOutcome::UnitRefused);
      assert_eq!(report.queries, 1);
      assert_eq!(
        state.usage().actor_control_used(),
        sum_weights(&[used, prices.entry, prices.guard])
      );
      assert_eq!(root(), before);
      assert_eq!(ServiceHeader::<Test>::get().cursor, Some(protected));
      assert_eq!(state.outstanding_reservations(), 0);
      assert!(!state.optional_actor_work_halted());
    });
  }
}

#[test]
fn service_head_allowance_quantum_commits_readiness_and_preserves_prior_units() {
  for class in [ActorType::System, ActorType::User] {
    new_test_ext().execute_with(|| {
      let (protected, sources, now, mut state) = prepare_quantum_prefix(class);
      let prefix = ActorSemanticStates::<Test>::get(sources[0]);
      let prefix_balance = native_balance(&sovereign_account(sources[0]));
      let source_account = sovereign_account(sources[1]);
      let balance = native_balance(&source_account);
      let sink = native_balance(&TestFeeSink::get());
      let first = attempt_quantum(now, &mut state);
      assert_eq!(first.outcome, QuantumOutcome::Committed);
      assert_eq!(first.queries, 2);
      assert!(first.worker.all_gt(Weight::zero()));
      assert_eq!(fee_collections(), vec![at_time_trigger_fee()]);
      assert_eq!(
        native_balance(&source_account),
        balance - at_time_trigger_fee()
      );
      assert_eq!(
        native_balance(&TestFeeSink::get()),
        sink + at_time_trigger_fee()
      );
      assert!(Actors::actor_hot(sources[1]).unwrap().pending_signal);
      assert_eq!(Actors::actor_identity(sources[1]).unwrap().cycle_nonce, 0);
      assert!(ServiceNodes::<Test>::get(sources[1]).unwrap().eligible_from > now);
      assert_eq!(ServiceHeader::<Test>::get().cursor, Some(protected));
      assert_eq!(ActorSemanticStates::<Test>::get(sources[0]), prefix);
      assert_eq!(
        native_balance(&sovereign_account(sources[0])),
        prefix_balance
      );
      let committed = root();
      let used = state.usage().actor_control_used();
      let later = attempt_quantum(now, &mut state);
      assert_eq!(later.outcome, QuantumOutcome::RolledBack);
      assert_eq!(later.queries, 2);
      assert!(!later.productive);
      assert_eq!(root(), committed, "both earlier units and fees survive");
      assert_eq!(fee_collections(), vec![at_time_trigger_fee()]);
      assert_eq!(
        state.usage().actor_control_used(),
        sum_weights(&[
          used,
          QuantumModel::tokens().entry,
          QuantumModel::tokens().guard,
          QuantumModel::tokens().outer,
          later.worker,
          QuantumModel::tokens().guard,
        ])
      );
      ordinary_service_remains(protected, now, &mut state);
    });
  }
}

#[test]
fn service_head_allowance_quantum_reloads_after_actual_collector_callback() {
  new_test_ext().execute_with(|| {
    let (protected, _, now, mut state) = prepare_quantum_prefix(ActorType::System);
    let before = root();
    let used = state.usage().actor_control_used();
    set_fee_collection_hook(move || {
      assert_ok!(Actors::close_actor(
        RuntimeOrigin::root(),
        protected.actor_id
      ));
      assert_ne!(ServiceHeader::<Test>::get().cursor, Some(protected));
    });
    let report = attempt_quantum(now, &mut state);
    assert_eq!(report.outcome, QuantumOutcome::RolledBack);
    assert_eq!(report.queries, 2);
    assert!(
      report.productive,
      "the worker succeeds before fresh-head refusal"
    );
    assert!(!report.post_supported);
    assert_eq!(
      root(),
      before,
      "callback close, ingress and fee all roll back together"
    );
    assert_eq!(
      fee_collections(),
      vec![at_time_trigger_fee()],
      "attempt observer is not a committed debit"
    );
    assert_eq!(
      state.usage().actor_control_used(),
      sum_weights(&[
        used,
        QuantumModel::tokens().entry,
        QuantumModel::tokens().guard,
        QuantumModel::tokens().outer,
        report.worker,
        QuantumModel::tokens().guard,
      ])
    );
    ordinary_service_remains(protected, now, &mut state);
  });
}

#[test]
fn service_head_allowance_quantum_fee_failure_keeps_incurred_control() {
  new_test_ext().execute_with(|| {
    let (protected, sources, now, mut state) = prepare_quantum_prefix(ActorType::System);
    let before = root();
    let used = state.usage().actor_control_used();
    set_fail_fee_sink_transfer_from(Some(sovereign_account(sources[1])));
    let report = attempt_quantum(now, &mut state);
    set_fail_fee_sink_transfer_from(None);
    assert_eq!(report.outcome, QuantumOutcome::RolledBack);
    assert_eq!(report.queries, 2);
    assert!(!report.productive);
    assert!(
      report.post_supported,
      "fee failure is not a head-refusal test"
    );
    assert_eq!(root(), before);
    type W = <Test as crate::Config>::WeightInfo;
    assert!(report.worker.all_gt(sum_weights(&[
      W::classify_due_block_deadline(),
      W::classify_due_tick_deadline()
    ])));
    assert_eq!(
      state.usage().actor_control_used(),
      sum_weights(&[
        used,
        QuantumModel::tokens().entry,
        QuantumModel::tokens().guard,
        QuantumModel::tokens().outer,
        report.worker,
        QuantumModel::tokens().guard,
      ])
    );
    ordinary_service_remains(protected, now, &mut state);
  });
}
