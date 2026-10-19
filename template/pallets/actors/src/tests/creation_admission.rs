use super::*;

fn policy_admission_contract(zero_step: bool) -> crate::ActorContractOf<Test> {
  let steps = if zero_step {
    BoundedVec::default()
  } else {
    transfer_contract_steps(BOB, 1)
  };
  let mut contract = user_active_contract(manual_schedule(), None, steps).unwrap();
  contract.close_when = Some(crate::CloseWhen {
    clauses: vec![
      vec![crate::BehaviorAtom {
        metric: crate::BehaviorMetric::CycleCompletedTotal,
        threshold: 2,
      }]
      .try_into()
      .unwrap(),
    ]
    .try_into()
    .unwrap(),
  });
  contract
}

#[test]
fn policy_admission_missing_control_or_inspection_is_not_arithmetic_overflow() {
  for class in [ActorType::User, ActorType::System] {
    for zero_step in [false, true] {
      for missing_control in [false, true] {
        new_test_ext().execute_with(|| {
          System::set_block_number(1);
          let contract = policy_admission_contract(zero_step);
          set_missing_behavior_control_weight(missing_control);
          set_missing_behavior_inspection_weight(!missing_control);
          let before = Actors::next_actor_id();
          let result = || match class {
            ActorType::User => Actors::create_user_actor(
              RuntimeOrigin::signed(ALICE),
              Mutability::Mutable,
              Some(contract.clone()),
            ),
            ActorType::System => Actors::create_system_actor(
              RuntimeOrigin::root(),
              ALICE,
              Mutability::Mutable,
              Some(contract.clone()),
            ),
          };
          assert_noop!(result(), Error::<Test>::BehaviorPolicyUnavailable);
          assert_eq!(Actors::next_actor_id(), before);
          assert!(fee_collections().is_empty());
        });
      }
    }
  }
}

#[test]
fn policy_admission_activation_and_replacement_preserve_the_existing_source() {
  for zero_step in [false, true] {
    for missing_control in [false, true] {
      new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let dormant = Actors::next_actor_id();
        assert_ok!(Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          Mutability::Mutable,
          None
        ));
        let active = create_user_with(
          ALICE,
          Mutability::Mutable,
          manual_schedule(),
          None,
          transfer_contract_steps(BOB, 1),
        );
        System::set_block_number(2);
        clear_fee_collections();
        set_missing_behavior_control_weight(missing_control);
        set_missing_behavior_inspection_weight(!missing_control);
        let contract = policy_admission_contract(zero_step);
        assert_noop!(
          Actors::activate_actor(RuntimeOrigin::signed(ALICE), dormant, contract.clone()),
          Error::<Test>::BehaviorPolicyUnavailable
        );
        assert_noop!(
          Actors::update_contract(RuntimeOrigin::signed(ALICE), active, contract),
          Error::<Test>::BehaviorPolicyUnavailable
        );
        assert!(fee_collections().is_empty());
      });
    }
  }
}

#[test]
fn policy_admission_keeps_checked_composition_overflow_distinct() {
  for zero_step in [false, true] {
    for maximum in [
      Weight::from_parts(u64::MAX, 0),
      Weight::from_parts(0, u64::MAX),
    ] {
      new_test_ext().execute_with(|| {
        System::set_block_number(1);
        set_behavior_control_maximum_override(Some(maximum));
        assert_noop!(
          Actors::create_user_actor(
            RuntimeOrigin::signed(ALICE),
            Mutability::Mutable,
            Some(policy_admission_contract(zero_step))
          ),
          Error::<Test>::AdmissionBoundOverflow
        );
        assert!(fee_collections().is_empty());
      });
    }
  }
}

#[test]
fn policy_admission_preserves_existing_complete_native_host_declarations() {
  for zero_step in [false, true] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let contract = policy_admission_contract(zero_step);
      prefund_active_user_creation(ALICE, &contract.steps);
      let id = Actors::next_actor_id();
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        Some(contract.clone())
      ));
      assert_eq!(Actors::load_actor_contract(id), Some(contract));
      #[cfg(feature = "try-runtime")]
      Actors::do_try_state().unwrap();
    });
  }
}

#[test]
fn policy_admission_preserves_invalid_expression_and_absent_policy_semantics() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    set_missing_behavior_control_weight(true);
    set_missing_behavior_inspection_weight(true);
    let mut invalid = policy_admission_contract(false);
    invalid.close_when.as_mut().unwrap().clauses[0][0].threshold = 0;
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        Some(invalid)
      ),
      Error::<Test>::InvalidCloseWhen
    );
    assert!(fee_collections().is_empty());
    let mut ordinary = policy_admission_contract(false);
    ordinary.close_when = None;
    prefund_active_user_creation(ALICE, &ordinary.steps);
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      Some(ordinary)
    ));
  });
}

#[test]
fn user_creation_normalizes_raw_dnf_before_canonical_predicate_cap() {
  use polkadot_sdk::frame_support::traits::Get;
  #[derive(Clone, Copy)]
  enum Normalization {
    Deduplicate,
    Absorb,
    OverCanonicalCap,
    DuplicateClauses,
  }
  let clauses = <<Test as crate::Config>::MaxPreconditionClauses as Get<u32>>::get();
  let atoms = <<Test as crate::Config>::MaxPredicatesPerClause as Get<u32>>::get();
  let cap = <<Test as crate::Config>::MaxPredicatesPerStep as Get<u32>>::get();
  let steps = <<Test as crate::Config>::MaxContractSteps as Get<u32>>::get();
  assert_eq!((clauses, atoms, cap), (4, 4, 4));
  for exact_slot in [false, true] {
    for normalization in [
      Normalization::Deduplicate,
      Normalization::Absorb,
      Normalization::OverCanonicalCap,
      Normalization::DuplicateClauses,
    ] {
      new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let predicate = |threshold| Predicate::BlockNumberAbove { threshold };
        let raw: crate::PreconditionOf<Test> = Precondition {
          clauses: (0..clauses)
            .map(|clause| {
              (0..atoms)
                .map(|atom| match normalization {
                  Normalization::Deduplicate => predicate(clause + 1),
                  Normalization::Absorb if clause == 0 || atom == 0 => predicate(0),
                  Normalization::Absorb | Normalization::OverCanonicalCap => {
                    predicate(1 + clause * atoms + atom)
                  }
                  Normalization::DuplicateClauses => predicate(atom + 1),
                })
                .collect::<Vec<_>>()
                .try_into()
                .expect("raw clause fits its codec bound")
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("raw DNF fits its codec bound"),
        };
        assert_eq!(raw.predicate_count(), clauses * atoms);
        assert!(raw.predicate_count() > cap);
        let plan = vec![
          StepOf::<Test> {
            precondition: Some(raw.clone()),
            task: Task::StopCycle,
            on_error: StepErrorPolicy::AbortCycle,
          };
          steps as usize
        ]
        .try_into()
        .expect("maximum Step plan fits");
        let contract = user_active_contract(manual_schedule(), None, plan);
        let actor_id = Actors::next_actor_id();
        let create = || {
          if exact_slot {
            Actors::create_user_actor_at_slot(
              RuntimeOrigin::signed(ALICE),
              0,
              Mutability::Mutable,
              contract.clone(),
            )
          } else {
            Actors::create_user_actor(
              RuntimeOrigin::signed(ALICE),
              Mutability::Mutable,
              contract.clone(),
            )
          }
        };
        let expected_clauses = match normalization {
          Normalization::OverCanonicalCap => {
            assert_noop!(create(), Error::<Test>::AdmissionBoundOverflow);
            assert!(fee_collections().is_empty());
            assert_eq!(Actors::next_actor_id(), actor_id);
            return;
          }
          Normalization::DuplicateClauses => {
            assert_noop!(create(), Error::<Test>::InvalidPredicate);
            assert!(fee_collections().is_empty());
            assert_eq!(Actors::next_actor_id(), actor_id);
            return;
          }
          Normalization::Deduplicate => (0..clauses)
            .map(|clause| vec![predicate(clause + 1)])
            .collect::<Vec<_>>(),
          Normalization::Absorb => vec![vec![predicate(0)]],
        };
        assert_ok!(create());
        let stored = Actors::actor_contract(actor_id).expect("canonical Contract exists");
        assert_eq!(stored.steps.len(), steps as usize);
        for step in &stored.steps {
          let canonical = step.precondition.as_ref().expect("canonical DNF exists");
          assert!(canonical.predicate_count() <= cap);
          assert!(raw.encoded_size() > canonical.encoded_size());
          assert_eq!(canonical.clauses.len(), expected_clauses.len());
          for (actual, expected) in canonical.clauses.iter().zip(&expected_clauses) {
            assert_eq!(actual.as_slice(), expected.as_slice());
          }
        }
        assert_eq!(Actors::actor_run_state(actor_id), None);
        #[cfg(feature = "try-runtime")]
        Actors::do_try_state().expect("normalized creation preserves canonical authority");
      });
    }
  }
}

#[test]
fn user_creation_admission_is_fail_closed_and_genesis_owned() {
  use polkadot_sdk::sp_runtime::BuildStorage;
  for enabled in [false, true] {
    let storage = crate::GenesisConfig::<Test> {
      user_creation_enabled: enabled,
      ..Default::default()
    }
    .build_storage()
    .expect("Actors genesis builds");
    polkadot_sdk::sp_io::TestExternalities::new(storage).execute_with(|| {
      assert!(crate::UserCreationEnabled::<Test>::exists());
      assert_eq!(Actors::user_creation_enabled(), enabled);
      assert!(!Actors::global_circuit_breaker());
      crate::UserCreationEnabled::<Test>::kill();
      assert!(
        !Actors::user_creation_enabled(),
        "missing admission authority denies creation"
      );
      assert_noop!(
        Actors::create_user_actor(RuntimeOrigin::signed(ALICE), Mutability::Mutable, None),
        Error::<Test>::UserCreationDisabled
      );
    });
  }
  assert!(!crate::GenesisConfig::<Test>::default().user_creation_enabled);
}

#[test]
fn disabled_user_creation_rejects_every_identity_shape_before_payment_or_slot_validation() {
  for frozen in [false, true] {
    for mutability in [Mutability::Mutable, Mutability::Immutable] {
      for active in [false, true] {
        new_test_ext().execute_with(|| {
          System::set_block_number(1);
          GlobalCircuitBreaker::<Test>::put(frozen);
          assert_ok!(Actors::set_user_creation_enabled(
            RuntimeOrigin::root(),
            false
          ));
          let contract = active.then(|| {
            user_active_contract(manual_schedule(), None, inert_contract_steps())
              .expect("Active Contract")
          });
          let expected = || {
            if frozen {
              Error::<Test>::GlobalCircuitBreakerActive
            } else {
              Error::<Test>::UserCreationDisabled
            }
          };
          let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
          assert_noop!(
            Actors::create_user_actor(RuntimeOrigin::signed(ALICE), mutability, contract.clone()),
            expected()
          );
          for slot in [0, u8::MAX] {
            assert_noop!(
              Actors::create_user_actor_at_slot(
                RuntimeOrigin::signed(ALICE),
                slot,
                mutability,
                contract.clone()
              ),
              expected()
            );
          }
          assert!(fee_collections().is_empty());
          assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
        });
      }
    }
  }
}

#[test]
fn disabled_user_creation_preserves_existing_service_activation_and_system_creation() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let active = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    fund_native(active, 1_000_000_000_000_000);
    let dormant = Actors::next_actor_id();
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(ALICE),
      Mutability::Mutable,
      None
    ));
    assert_ok!(Actors::set_user_creation_enabled(
      RuntimeOrigin::root(),
      false
    ));
    let bob_before = native_balance(&BOB);
    assert_ok!(Actors::manual_trigger(RuntimeOrigin::signed(ALICE), active));
    System::set_block_number(2);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::signed(ALICE),
      dormant,
      user_active_contract(manual_schedule(), None, inert_contract_steps()).expect("Contract")
    ));
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None
    ));
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      user_active_contract(manual_schedule(), None, inert_contract_steps())
    ));
    run_canonical_block_at(2, Weight::MAX);
    assert_eq!(native_balance(&BOB), bob_before + 10);
    assert_eq!(Actors::actor_identity(active).unwrap().cycle_nonce, 1);
    assert!(Actors::active_actor_state(dormant).is_some());
    assert!(!Actors::user_creation_enabled());
    System::set_block_number(3);
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      dormant
    ));
    System::set_block_number(4);
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), dormant));
    assert_noop!(
      Actors::create_user_actor_at_slot(RuntimeOrigin::signed(ALICE), 1, Mutability::Mutable, None),
      Error::<Test>::UserCreationDisabled
    );
    assert_ok!(Actors::set_user_creation_enabled(
      RuntimeOrigin::root(),
      true
    ));
    assert_ok!(Actors::create_user_actor_at_slot(
      RuntimeOrigin::signed(ALICE),
      1,
      Mutability::Mutable,
      None
    ));
    let reenabled = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    assert!(Actors::active_actor_state(reenabled).is_some());
  });
}

#[test]
fn frozen_creation_rejects_both_classes_before_contract_and_locator_validation() {
  for mutability in [Mutability::Mutable, Mutability::Immutable] {
    for active in [false, true] {
      new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let occupied_id =
          create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
        let contract = active.then(|| {
          let mut contract = user_active_contract(manual_schedule(), None, inert_contract_steps())
            .expect("Active Contract");
          contract.cooldown_blocks = u32::MAX;
          contract
        });
        assert_ok!(Actors::set_global_circuit_breaker(
          RuntimeOrigin::root(),
          true
        ));
        let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
        assert_noop!(
          Actors::create_user_actor(RuntimeOrigin::signed(ALICE), mutability, contract.clone()),
          Error::<Test>::GlobalCircuitBreakerActive
        );
        for slot in [0, u8::MAX] {
          assert_noop!(
            Actors::create_user_actor_at_slot(
              RuntimeOrigin::signed(ALICE),
              slot,
              mutability,
              contract.clone()
            ),
            Error::<Test>::GlobalCircuitBreakerActive
          );
        }
        assert_noop!(
          Actors::create_system_actor(RuntimeOrigin::root(), ALICE, mutability, contract.clone()),
          Error::<Test>::GlobalCircuitBreakerActive
        );
        for locator in [occupied_id, u64::MAX] {
          assert_noop!(
            Actors::create_system_actor_at_sovereign_id(
              RuntimeOrigin::root(),
              locator,
              ALICE,
              mutability,
              contract.clone()
            ),
            Error::<Test>::GlobalCircuitBreakerActive
          );
        }
        assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
      });
    }
  }
}

#[test]
fn frozen_creation_preserves_origin_authentication_before_the_guard() {
  new_test_ext().execute_with(|| {
    assert_ok!(Actors::set_global_circuit_breaker(
      RuntimeOrigin::root(),
      true
    ));
    assert_noop!(
      Actors::create_user_actor(RuntimeOrigin::root(), Mutability::Mutable, None),
      DispatchError::BadOrigin
    );
    assert_noop!(
      Actors::create_user_actor_at_slot(RuntimeOrigin::none(), 0, Mutability::Mutable, None),
      DispatchError::BadOrigin
    );
    assert_noop!(
      Actors::create_system_actor(
        RuntimeOrigin::signed(ALICE),
        ALICE,
        Mutability::Mutable,
        None
      ),
      DispatchError::BadOrigin
    );
    assert_noop!(
      Actors::create_system_actor_at_sovereign_id(
        RuntimeOrigin::signed(ALICE),
        u64::MAX,
        ALICE,
        Mutability::Mutable,
        None
      ),
      DispatchError::BadOrigin
    );
  });
}

#[test]
fn frozen_creation_preserves_vacant_user_slot_and_system_locator_custody_until_thaw() {
  for system in [false, true] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let actor_id = Actors::next_actor_id();
      if system {
        assert_ok!(Actors::create_system_actor(
          RuntimeOrigin::root(),
          ALICE,
          Mutability::Mutable,
          None
        ));
      } else {
        assert_ok!(Actors::create_user_actor_at_slot(
          RuntimeOrigin::signed(ALICE),
          0,
          Mutability::Mutable,
          None
        ));
      }
      let custody = Actors::actor_identity(actor_id)
        .expect("Dormant identity")
        .sovereign_account;
      let _ = Balances::deposit_creating(&custody, 1_000);
      System::set_block_number(2);
      assert_ok!(Actors::close_actor(
        if system {
          RuntimeOrigin::root()
        } else {
          RuntimeOrigin::signed(ALICE)
        },
        actor_id
      ));
      let balance = native_balance(&custody);
      let next_id = Actors::next_actor_id();
      assert_ok!(Actors::set_global_circuit_breaker(
        RuntimeOrigin::root(),
        true
      ));
      let before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
      let recreate = || {
        if system {
          Actors::create_system_actor_at_sovereign_id(
            RuntimeOrigin::root(),
            actor_id,
            ALICE,
            Mutability::Mutable,
            None,
          )
        } else {
          Actors::create_user_actor_at_slot(
            RuntimeOrigin::signed(ALICE),
            0,
            Mutability::Mutable,
            None,
          )
        }
      };
      assert_noop!(recreate(), Error::<Test>::GlobalCircuitBreakerActive);
      assert_eq!(Actors::next_actor_id(), next_id);
      assert_eq!(native_balance(&custody), balance);
      assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), before);
      assert_ok!(Actors::set_global_circuit_breaker(
        RuntimeOrigin::root(),
        false
      ));
      assert_ok!(recreate());
      let identity = Actors::actor_identity(next_id).expect("fresh identity after thaw");
      assert_ne!(next_id, actor_id);
      assert_eq!(identity.sovereign_account, custody);
      assert_eq!(identity.cycle_nonce, 0);
      assert_eq!(native_balance(&custody), balance);
      assert_eq!(Actors::actor_run_state(next_id), None);
    });
  }
}

#[test]
fn user_creation_admission_capacity_refuses_before_payment_or_deadline_publication() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    assert_ok!(Actors::set_active_actor_limit(RuntimeOrigin::root(), 32));
    let window = ScheduleWindow { start: 1, end: 200 };
    for _ in 0..31 {
      create_system_with(
        ALICE,
        manual_schedule(),
        Some(window),
        inert_contract_steps(),
      );
    }
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_user_actor_at_slot(
      RuntimeOrigin::signed(ALICE),
      0,
      Mutability::Mutable,
      user_active_contract(manual_schedule(), Some(window), inert_contract_steps()),
    ));
    let key = WakeupKey::Block(201);
    let header = crate::DeadlineHeaders::<Test>::get(key).expect("public full C32 bucket");
    assert_eq!(
      (header.count, header.page_count, header.first_vacant_page),
      (32, 1, None)
    );
    assert_eq!(crate::ActiveActorCount::<Test>::get(), 32);
    assert!(Actors::actor_identity(actor_id).is_some());
    let fees = fee_collections();
    let next_id = Actors::next_actor_id();
    for end in [200, 201] {
      let contract = user_active_contract(
        manual_schedule(),
        Some(ScheduleWindow { start: 1, end }),
        inert_contract_steps(),
      );
      assert_noop!(
        Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          Mutability::Mutable,
          contract.clone()
        ),
        Error::<Test>::ActiveActorCapacityExceeded
      );
      assert_noop!(
        Actors::create_user_actor_at_slot(
          RuntimeOrigin::signed(ALICE),
          1,
          Mutability::Mutable,
          contract
        ),
        Error::<Test>::ActiveActorCapacityExceeded
      );
    }
    assert_eq!(fee_collections(), fees);
    assert_eq!(Actors::next_actor_id(), next_id);
    assert_eq!(crate::DeadlineHeaders::<Test>::get(key), Some(header));
    assert!(!crate::DeadlineHeaders::<Test>::contains_key(
      WakeupKey::Block(202)
    ));
    #[cfg(feature = "try-runtime")]
    Actors::do_try_state().expect("public mixed-class capacity boundary is canonical");
  });
}

#[test]
fn user_creation_admission_clock_targets_separate_block_exhaustion_from_tick_range() {
  use polkadot_sdk::frame_support::traits::Get;
  let block_max = u64::MAX;
  for cooldown in [1, 2] {
    new_test_ext().execute_with(|| {
      System::set_block_number(block_max - 1);
      let schedule = RuntimeSchedule {
        trigger: Trigger::manual(),
        cooldown_blocks: cooldown,
      };
      let contract = user_active_contract(schedule, None, inert_contract_steps());
      if cooldown == 2 {
        assert_noop!(
          Actors::create_user_actor(RuntimeOrigin::signed(ALICE), Mutability::Mutable, contract),
          Error::<Test>::SchedulerIndexExhausted
        );
        assert!(fee_collections().is_empty());
      } else {
        assert_ok!(Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          Mutability::Mutable,
          contract
        ));
        #[cfg(feature = "try-runtime")]
        Actors::do_try_state().expect("exact Block cooldown target is representable");
      }
    });
  }
  for end in [block_max - 1, block_max] {
    new_test_ext().execute_with(|| {
      System::set_block_number(block_max - 1);
      let contract = user_active_contract(
        manual_schedule(),
        Some(ScheduleWindow {
          start: end - 99,
          end,
        }),
        inert_contract_steps(),
      );
      let id = Actors::next_actor_id();
      if end == block_max {
        assert_noop!(
          Actors::create_user_actor(RuntimeOrigin::signed(ALICE), Mutability::Mutable, contract),
          Error::<Test>::InvalidScheduleWindow
        );
        assert!(fee_collections().is_empty());
      } else {
        assert_ok!(Actors::create_user_actor(
          RuntimeOrigin::signed(ALICE),
          Mutability::Mutable,
          contract
        ));
        assert_eq!(
          crate::DeadlineHandles::<Test>::get(id)
            .expect("exact terminal handle")
            .key,
          WakeupKey::Block(block_max)
        );
        #[cfg(feature = "try-runtime")]
        Actors::do_try_state().expect("exact window terminal is canonical");
      }
    });
  }
  new_test_ext().execute_with(|| {
    System::set_block_number(block_max);
    assert_noop!(
      Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(manual_schedule(), None, inert_contract_steps()),
      ),
      Error::<Test>::SchedulerIndexExhausted
    );
    assert!(fee_collections().is_empty());
  });
  let delay = <<Test as crate::Config>::MaxTemporalDelayTicks as Get<u64>>::get();
  let millis = <<Test as crate::Config>::CadenceTickMillis as Get<u64>>::get();
  let floor = crate::scheduler_tick_floor(u64::MAX, millis).expect("positive tick duration");
  let anchor = crate::scheduler_tick_ceil(u64::MAX, millis).expect("timestamp ceiling fits");
  let target = anchor
    .checked_add(delay)
    .expect("configured full temporal delay fits even at maximum timestamp");
  assert_eq!(
    crate::first_cadence_due_tick(u64::MAX, millis, delay),
    Some(target)
  );
  assert_eq!(
    crate::next_cadence_due_tick(anchor, delay, floor),
    Some(target)
  );
  assert_eq!(
    crate::first_cadence_due_tick(u64::MAX - 1, 1, 1),
    Some(u64::MAX)
  );
  assert_eq!(crate::first_cadence_due_tick(u64::MAX, 1, 1), None);
  assert_eq!(
    crate::next_cadence_due_tick(u64::MAX - 1, 1, u64::MAX),
    None
  );
  for trigger in [
    Trigger::AtTime { after_ticks: delay },
    Trigger::Cadenced { every_ticks: delay },
  ] {
    new_test_ext().execute_with(|| {
      let schedule = RuntimeSchedule {
        trigger,
        cooldown_blocks: 0,
      };
      System::set_block_number(block_max - 1);
      let id = Actors::next_actor_id();
      assert_ok!(Actors::create_user_actor(
        RuntimeOrigin::signed(ALICE),
        Mutability::Mutable,
        user_active_contract(schedule, None, inert_contract_steps()),
      ));
      assert_eq!(
        Actors::active_actor_view(id)
          .expect("temporal Actor")
          .temporal_anchor_tick,
        Some(anchor)
      );
      assert_eq!(
        crate::TriggerDeadlineHandles::<Test>::get(id)
          .expect("temporal handle")
          .key,
        WakeupKey::Tick(target)
      );
      assert!(!crate::DeadlineHandles::<Test>::contains_key(id));
      assert_eq!(Actors::actor_run_state(id), None);
      #[cfg(feature = "try-runtime")]
      Actors::do_try_state().expect("configured timestamp extrema preserve canonical topology");
    });
  }
}

#[test]
fn user_creation_admission_page_ids_follow_retained_bucket_churn() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let window = ScheduleWindow { start: 1, end: 200 };
    let guards = (0..32)
      .map(|_| {
        create_system_with(
          ALICE,
          manual_schedule(),
          Some(window),
          inert_contract_steps(),
        )
      })
      .collect::<Vec<_>>();
    let key = WakeupKey::Block(201);
    let original = crate::DeadlinePages::<Test>::get(key, 0).expect("retained full page");
    for page_id in 1..=3u64 {
      let id = Actors::next_actor_id();
      assert_ok!(Actors::create_user_actor_at_slot(
        RuntimeOrigin::signed(ALICE),
        0,
        Mutability::Mutable,
        user_active_contract(manual_schedule(), Some(window), inert_contract_steps()),
      ));
      let handle = crate::DeadlineHandles::<Test>::get(id).expect("new-page member");
      assert_eq!((handle.key, handle.page, handle.slot), (key, page_id, 0));
      let header = crate::DeadlineHeaders::<Test>::get(key).expect("two-page header");
      assert_eq!(
        (header.count, header.page_count, header.next_page),
        (33, 2, page_id + 1)
      );
      System::set_block_number(page_id + 1);
      assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), id));
      let header = crate::DeadlineHeaders::<Test>::get(key).expect("retained bucket");
      assert_eq!(
        (header.count, header.page_count, header.next_page),
        (32, 1, page_id + 1)
      );
      assert_eq!((header.last_page, header.first_vacant_page), (0, None));
      assert!(!crate::DeadlinePages::<Test>::contains_key(key, page_id));
      assert_eq!(
        crate::DeadlinePages::<Test>::get(key, 0),
        Some(original.clone())
      );
      #[cfg(feature = "try-runtime")]
      Actors::do_try_state().expect("public page churn preserves all retained authority");
    }
    for guard in guards {
      assert_ok!(Actors::close_actor(RuntimeOrigin::root(), guard));
    }
    assert!(!crate::DeadlineHeaders::<Test>::contains_key(key));
    assert!(!crate::DeadlineIndexPositions::<Test>::contains_key(key));
    let id = Actors::next_actor_id();
    assert_ok!(Actors::create_user_actor_at_slot(
      RuntimeOrigin::signed(ALICE),
      0,
      Mutability::Mutable,
      user_active_contract(manual_schedule(), Some(window), inert_contract_steps()),
    ));
    let header = crate::DeadlineHeaders::<Test>::get(key).expect("new bucket epoch");
    assert_eq!(
      (header.count, header.page_count, header.next_page),
      (1, 1, 1)
    );
    assert_eq!(
      crate::DeadlineHandles::<Test>::get(id)
        .expect("new first member")
        .page,
      0
    );
    #[cfg(feature = "try-runtime")]
    Actors::do_try_state().expect("empty-bucket reset has no retained peer obligation");
  });
}

#[test]
fn user_creation_admission_page_id_endpoint_preserves_paid_prefix_rollback() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let window = ScheduleWindow { start: 1, end: 200 };
    let removable_guard = Actors::next_actor_id();
    for _ in 0..32 {
      create_system_with(
        ALICE,
        manual_schedule(),
        Some(window),
        inert_contract_steps(),
      );
    }
    let key = WakeupKey::Block(201);
    // Synthetic scalar endpoint only; public churn above supplies the causal owner,
    // not evidence of a lifetime containing u64::MAX page allocations.
    crate::DeadlineHeaders::<Test>::mutate(key, |header| {
      header.as_mut().expect("public bucket exists").next_page = u64::MAX - 1;
    });
    let id = Actors::next_actor_id();
    assert_ok!(Actors::create_user_actor_at_slot(
      RuntimeOrigin::signed(ALICE),
      0,
      Mutability::Mutable,
      user_active_contract(manual_schedule(), Some(window), inert_contract_steps()),
    ));
    assert_eq!(
      crate::DeadlineHandles::<Test>::get(id)
        .expect("last representable successor")
        .page,
      u64::MAX - 1
    );
    assert_eq!(
      crate::DeadlineHeaders::<Test>::get(key)
        .expect("endpoint header")
        .next_page,
      u64::MAX
    );
    #[cfg(feature = "try-runtime")]
    Actors::do_try_state().expect("sparse high page id has complete current authority");
    System::set_block_number(2);
    assert_ok!(Actors::close_actor(RuntimeOrigin::signed(ALICE), id));
    assert_eq!(crate::ActiveActorCount::<Test>::get(), 32);
    let header = crate::DeadlineHeaders::<Test>::get(key).expect("retained endpoint header");
    assert_eq!(
      (header.count, header.page_count, header.next_page),
      (32, 1, u64::MAX)
    );
    assert_eq!(header.first_vacant_page, None);
    for exact_slot in [false, true] {
      let root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
      let next_id = Actors::next_actor_id();
      let owner_balance = native_balance(&ALICE);
      let attempted_collections = fee_collections().len();
      let contract = user_active_contract(manual_schedule(), Some(window), inert_contract_steps());
      let result = if exact_slot {
        Actors::create_user_actor_at_slot(
          RuntimeOrigin::signed(ALICE),
          0,
          Mutability::Mutable,
          contract,
        )
      } else {
        Actors::create_user_actor(RuntimeOrigin::signed(ALICE), Mutability::Mutable, contract)
      };
      assert_eq!(result, Err(Error::<Test>::SchedulerIndexExhausted.into()));
      assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), root);
      assert_eq!(native_balance(&ALICE), owner_balance);
      assert_eq!(Actors::next_actor_id(), next_id);
      assert_eq!(
        fee_collections().len(),
        attempted_collections + 1,
        "test-only collector spy observes rolled-back collection work"
      );
      assert_eq!(crate::DeadlineHeaders::<Test>::get(key), Some(header));
      assert!(!crate::DeadlinePages::<Test>::contains_key(key, u64::MAX));
      #[cfg(feature = "try-runtime")]
      Actors::do_try_state().expect("endpoint refusal preserves every retained peer");
    }
    System::set_block_number(3);
    assert_ok!(Actors::close_actor(RuntimeOrigin::root(), removable_guard));
    let reused = Actors::next_actor_id();
    assert_ok!(Actors::create_user_actor_at_slot(
      RuntimeOrigin::signed(ALICE),
      0,
      Mutability::Mutable,
      user_active_contract(manual_schedule(), Some(window), inert_contract_steps()),
    ));
    assert_eq!(
      crate::DeadlineHandles::<Test>::get(reused)
        .expect("retained vacancy member")
        .page,
      0
    );
    let header =
      crate::DeadlineHeaders::<Test>::get(key).expect("exhausted append namespace remains");
    assert_eq!(
      (header.count, header.page_count, header.next_page),
      (32, 1, u64::MAX)
    );
    #[cfg(feature = "try-runtime")]
    Actors::do_try_state().expect("retained vacancy reuse allocates no new page id");
  });
}

#[test]
fn user_creation_admission_deadline_failure_partition_preserves_capacity() {
  use crate::scheduler::EnqueueOutcome;
  use crate::{DeadlineIndexMutationError, DeadlineMutationError, DependencyRegistrationError};
  for (error, placement, dependency) in [
    (
      DeadlineMutationError::NamespaceExhausted,
      EnqueueOutcome::SchedulerIndexExhausted,
      DependencyRegistrationError::DeadlineNamespaceExhausted,
    ),
    (
      DeadlineMutationError::CapacityExceeded,
      EnqueueOutcome::WakeupCapacityExhausted,
      DependencyRegistrationError::CapacityExceeded,
    ),
    (
      DeadlineMutationError::PageFull,
      EnqueueOutcome::WakeupCapacityExhausted,
      DependencyRegistrationError::StoredPlanMismatch,
    ),
    (
      DeadlineMutationError::CorruptCarrier,
      EnqueueOutcome::CorruptedTopology,
      DependencyRegistrationError::StoredPlanMismatch,
    ),
  ] {
    assert_eq!(EnqueueOutcome::from(error), placement);
    assert_eq!(DependencyRegistrationError::from(error), dependency);
  }
  let index_capacity = DeadlineMutationError::from(DeadlineIndexMutationError::CapacityExceeded);
  assert_eq!(index_capacity, DeadlineMutationError::CapacityExceeded);
  assert_eq!(
    EnqueueOutcome::from(index_capacity),
    EnqueueOutcome::WakeupCapacityExhausted
  );
  new_test_ext().execute_with(|| {
    assert_eq!(
      Actors::placement_error(EnqueueOutcome::SchedulerIndexExhausted),
      Error::<Test>::SchedulerIndexExhausted.into()
    );
    assert_eq!(
      Actors::placement_error(EnqueueOutcome::WakeupCapacityExhausted),
      Error::<Test>::QueueCapacityUnavailable.into()
    );
  });
}

#[test]
fn user_creation_admission_namespace_publication_is_class_and_clock_neutral() {
  for system in [false, true] {
    for temporal in [false, true] {
      new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let schedule = if temporal {
          timer_schedule(100)
        } else {
          manual_schedule()
        };
        let window = (!temporal).then_some(ScheduleWindow { start: 1, end: 200 });
        for _ in 0..32 {
          create_system_with(ALICE, schedule.clone(), window, inert_contract_steps());
        }
        let key = if temporal {
          WakeupKey::Tick(101)
        } else {
          WakeupKey::Block(201)
        };
        // Endpoint geometry only; the separately qualified public churn retains causality.
        crate::DeadlineHeaders::<Test>::mutate(key, |header| {
          header.as_mut().expect("public full bucket").next_page = u64::MAX;
        });
        let root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
        let collections = fee_collections().len();
        let result = if system {
          Actors::create_system_actor(
            RuntimeOrigin::root(),
            ALICE,
            Mutability::Mutable,
            system_active_contract(schedule, window, inert_contract_steps()),
          )
        } else {
          Actors::create_user_actor(
            RuntimeOrigin::signed(ALICE),
            Mutability::Mutable,
            user_active_contract(schedule, window, inert_contract_steps()),
          )
        };
        assert_eq!(result, Err(Error::<Test>::SchedulerIndexExhausted.into()));
        assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), root);
        assert_eq!(fee_collections().len(), collections + usize::from(!system));
        assert!(!crate::DeadlinePages::<Test>::contains_key(key, u64::MAX));
        #[cfg(feature = "try-runtime")]
        Actors::do_try_state().expect("both publication owners preserve retained peers");
      });
    }
  }
}
