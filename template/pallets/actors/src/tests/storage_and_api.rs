use super::*;
use crate::weights::WeightInfo as _;
use crate::{
  ActorContractHeads, ActorContractTailChunks, ActorCostQuoteError, ActorProcess, ActorProcesses,
  ActorRef, ActorSemanticExecutionProjection, ActorSemanticLoadError, ActorSemanticMutation,
  ActorSemanticMutationError, ActorSemanticProjectionError, ActorSemanticRecord,
  ActorSemanticState, ActorSemanticStates, ActorStepResourceEnvelope, ActorUnsignaledControlCells,
  ActorWaitingOccupancies, CanonicalOccurrenceError, CanonicalOccurrencePlan,
  CanonicalOccurrencePublication, CloseReason, CompletionPolicy, DeadlineHandle, DeadlineHandles,
  DeadlineHeader, DeadlineHeaders, DeadlineIndexLen, DeadlineIndexMutationError,
  DeadlineIndexPages, DeadlineIndexPositions, DeadlineMutationError, DeadlinePages,
  DependencyDueReviewError, DependencyDueReviewMutation, DependencyPlanMutation,
  DependencyPlanSource, DependencyPlans, DependencyPublicationError, DependencyPublicationMutation,
  DependencyRegistrationError, DependencyRegistrationFreePositions, DependencyRegistrationHandle,
  DependencyRegistrationHeaders, DependencyRegistrationMutation, DependencyRegistrationPages,
  DependencyRegistrationPosition, DependencyRegistrationPositions, DependencyRegistrations,
  DependencyReviewMutation, DependencyReviewWorkerError, DependencyRevisionError,
  DependencyRevisionMutation, DependencyRevisionState, DependencyRevisions, DependencyScanError,
  DependencyScanMutation, DependencyScanSourceError, DependencyScanSourceList,
  DependencyScanSourceListState, DependencyScanSourceMutation, DependencyScanSourceNode,
  DependencyScanSourceNodes, DependencySourceAllocator, DependencySourceAllocatorState,
  DependencySourceError, DependencySourceMutation, DependencySourceObservations,
  DependencyTimedReview, DependencyTimedReviewMutation, DependencyTimedReviews,
  DormantActorSemanticRecord, DueBlockDeadlineMutation, DueTickDeadlineMutation,
  LegacyProcessPlacement, LegacyProcessTransition, ObservationDependencySources, ParkEvidence,
  ParkNegativeReason, PendingCheckOwner, PendingCheckOwners, PendingDependencyEvent,
  PendingDependencyEvents, PendingDependencyReviews, PipelineMachineFeeStrategy,
  ProcessCompileError, ProcessDisableCause, ProcessDisablement, ProcessPublicationError,
  ProcessResidence, ProcessRevivalAuthority, ProcessStatus, ProcessTransitionError,
  ProcessTransitionObligation, ScalarObservationState, ServiceHeader, ServiceHeaderRecord,
  ServiceNode, ServiceNodes, ServicePublicationError, ServiceResidenceKind, ServiceRetirementError,
  ServiceRingMutationError, ServiceRoundEncounter, ServiceRoundError, SuspendedProcessBasis,
  UnsignaledProcessEvidence, apply_actor_semantic_mutation, compile_legacy_process,
  next_actor_generation, plan_canonical_occurrence, plan_legacy_process_transition,
  project_actor_semantic_execution,
};
use frame::traits::ConstU32;
use std::collections::BTreeMap;

#[test]
fn process_skeleton_uses_generation_bound_references() {
  assert_eq!(
    ActorRef {
      actor_id: 7,
      generation: 11
    }
    .encoded_size(),
    16
  );
  assert_ne!(
    ActorRef {
      actor_id: 7,
      generation: 11
    },
    ActorRef {
      actor_id: 7,
      generation: 12
    }
  );
}

#[test]
fn semantic_record_owns_only_non_derivable_state_and_projects_execution_geometry() {
  let record = ActorSemanticRecord {
    identity: 11u32,
    generation: 7,
    hot: 12u32,
    admission: 13u32,
  };
  assert_eq!(
    (
      record.identity,
      record.generation,
      record.hot,
      record.admission,
    ),
    (11, 7, 12, 13)
  );

  let resources = ActorStepResourceEnvelope {
    control: Weight::from_parts(17, 19),
    effect: Weight::from_parts(23, 29),
  };
  assert_eq!(
    project_actor_semantic_execution(CycleState::Idle, None::<(u32, u32)>, Some(resources)),
    Ok(ActorSemanticExecutionProjection {
      cursor: 0,
      eligible_at: None,
      resources,
    })
  );
  assert_eq!(
    project_actor_semantic_execution(CycleState::Running, Some((3, 31u32)), Some(resources)),
    Ok(ActorSemanticExecutionProjection {
      cursor: 3,
      eligible_at: Some(31),
      resources,
    })
  );
  assert_eq!(
    project_actor_semantic_execution(CycleState::Suspended, Some((4, 37u32)), Some(resources)),
    Ok(ActorSemanticExecutionProjection {
      cursor: 4,
      eligible_at: Some(37),
      resources,
    })
  );
  assert_eq!(
    project_actor_semantic_execution(CycleState::Idle, Some((0, 31u32)), Some(resources)),
    Err(ActorSemanticProjectionError::RunStateMismatch)
  );
  assert_eq!(
    project_actor_semantic_execution(CycleState::Running, None::<(u32, u32)>, Some(resources)),
    Err(ActorSemanticProjectionError::RunStateMismatch)
  );
  assert_eq!(
    project_actor_semantic_execution(CycleState::Running, Some((3, 31u32)), None),
    Err(ActorSemanticProjectionError::CurrentStepMissing)
  );
}

#[test]
fn semantic_generation_is_nonzero_and_fails_closed_at_exhaustion() {
  assert_eq!(next_actor_generation(0), Some(1));
  assert_eq!(next_actor_generation(41), Some(42));
  assert_eq!(next_actor_generation(u64::MAX), None);
}

#[test]
fn semantic_mutation_is_complete_compare_and_replace_without_placement_authority() {
  let dormant = ActorSemanticState::Dormant(DormantActorSemanticRecord {
    identity: 1u32,
    generation: 0,
  });
  let active = ActorSemanticState::Active(ActorSemanticRecord {
    identity: 1u32,
    generation: 1,
    hot: 4u32,
    admission: 5u32,
  });
  let stale = ActorSemanticState::Active(ActorSemanticRecord {
    identity: 9u32,
    generation: 9,
    hot: 9u32,
    admission: 9u32,
  });

  assert_eq!(
    apply_actor_semantic_mutation(None, &ActorSemanticMutation::Publish(dormant.clone()),),
    Ok(Some(dormant.clone()))
  );
  assert_eq!(
    apply_actor_semantic_mutation(
      Some(&dormant),
      &ActorSemanticMutation::Publish(dormant.clone()),
    ),
    Err(ActorSemanticMutationError::AlreadyPublished)
  );
  assert_eq!(
    apply_actor_semantic_mutation(
      Some(&dormant),
      &ActorSemanticMutation::Replace {
        expected: dormant.clone(),
        replacement: active.clone(),
      },
    ),
    Ok(Some(active.clone()))
  );
  assert_eq!(
    apply_actor_semantic_mutation(
      Some(&dormant),
      &ActorSemanticMutation::Replace {
        expected: stale.clone(),
        replacement: active.clone(),
      },
    ),
    Err(ActorSemanticMutationError::Stale)
  );
  assert_eq!(
    apply_actor_semantic_mutation(
      Some(&active),
      &ActorSemanticMutation::Replace {
        expected: active.clone(),
        replacement: dormant.clone(),
      },
    ),
    Ok(Some(dormant.clone()))
  );
  assert_eq!(
    apply_actor_semantic_mutation(
      Some(&dormant),
      &ActorSemanticMutation::Remove {
        expected: dormant.clone(),
      },
    ),
    Ok(None)
  );
  assert_eq!(
    apply_actor_semantic_mutation(None, &ActorSemanticMutation::Remove { expected: dormant },),
    Err(ActorSemanticMutationError::Missing)
  );
}

#[test]
fn legacy_process_compiler_maps_exact_placements_and_refuses_unsignaled_guessing() {
  let disabled = ProcessDisablement {
    cause: ProcessDisableCause::OwnerPaused,
    revival_authority: ProcessRevivalAuthority::Owner,
    basis: SuspendedProcessBasis::Running { eligible_at: 9u32 },
  };
  let parked = ParkEvidence {
    plan_identity: [3; 32],
    reason: ParkNegativeReason::PredicateFalse,
    review_at: Some(12u32),
  };
  let cases = [
    (
      LegacyProcessPlacement::Ready(ServiceResidenceKind::Live),
      ActorProcess {
        generation: 11,
        last_attempted: Some(7),
        status: ProcessStatus::Serving,
        residence: Some(ProcessResidence::Service(ServiceResidenceKind::Live)),
      },
    ),
    (
      LegacyProcessPlacement::Ready(ServiceResidenceKind::Pending),
      ActorProcess {
        generation: 11,
        last_attempted: Some(7),
        status: ProcessStatus::Serving,
        residence: Some(ProcessResidence::Service(ServiceResidenceKind::Pending)),
      },
    ),
    (
      LegacyProcessPlacement::Waiting {
        key: WakeupKey::Block(9),
        page: 2,
        slot: 3,
      },
      ActorProcess {
        generation: 11,
        last_attempted: Some(7),
        status: ProcessStatus::Serving,
        residence: Some(ProcessResidence::Deadline {
          key: WakeupKey::Block(9),
          page: 2,
          slot: 3,
        }),
      },
    ),
    (
      LegacyProcessPlacement::Waiting {
        key: WakeupKey::Tick(10),
        page: 4,
        slot: 5,
      },
      ActorProcess {
        generation: 11,
        last_attempted: Some(7),
        status: ProcessStatus::Serving,
        residence: Some(ProcessResidence::Deadline {
          key: WakeupKey::Tick(10),
          page: 4,
          slot: 5,
        }),
      },
    ),
    (
      LegacyProcessPlacement::Unsignaled(Some(UnsignaledProcessEvidence::Parked(parked))),
      ActorProcess {
        generation: 11,
        last_attempted: Some(7),
        status: ProcessStatus::Serving,
        residence: Some(ProcessResidence::Parked(parked)),
      },
    ),
    (
      LegacyProcessPlacement::Unsignaled(Some(UnsignaledProcessEvidence::Disabled(disabled))),
      ActorProcess {
        generation: 11,
        last_attempted: Some(7),
        status: ProcessStatus::Disabled(disabled),
        residence: None,
      },
    ),
  ];
  for (placement, expected) in cases {
    assert_eq!(compile_legacy_process(11, Some(7), placement), Ok(expected));
  }
  assert_eq!(
    compile_legacy_process::<u32>(11, None, LegacyProcessPlacement::Unsignaled(None)),
    Err(ProcessCompileError::AmbiguousUnsignaled)
  );
}

#[test]
fn canonical_occurrence_planner_publishes_idle_pending_and_preserves_busy_residence() {
  let parked = ParkEvidence {
    plan_identity: [3; 32],
    reason: ParkNegativeReason::PredicateFalse,
    review_at: Some(12u32),
  };
  let idle = ActorProcess {
    generation: 11,
    last_attempted: Some(7),
    status: ProcessStatus::Serving,
    residence: Some(ProcessResidence::Parked(parked)),
  };
  assert_eq!(
    plan_canonical_occurrence(CycleState::Idle, false, idle, 20),
    Ok(Some(CanonicalOccurrencePlan {
      process: ActorProcess {
        residence: Some(ProcessResidence::Service(ServiceResidenceKind::Pending)),
        ..idle
      },
      pending_signal: true,
      publication: CanonicalOccurrencePublication::PublishPending { eligible_from: 21 },
    }))
  );
  let disabled = ActorProcess {
    status: ProcessStatus::Disabled(ProcessDisablement {
      cause: ProcessDisableCause::Protocol,
      revival_authority: ProcessRevivalAuthority::Protocol,
      basis: SuspendedProcessBasis::Idle,
    }),
    residence: None,
    ..idle
  };
  assert_eq!(
    plan_canonical_occurrence(CycleState::Idle, false, disabled, 20),
    Ok(Some(CanonicalOccurrencePlan {
      process: ActorProcess {
        status: ProcessStatus::Serving,
        residence: Some(ProcessResidence::Service(ServiceResidenceKind::Pending)),
        ..disabled
      },
      pending_signal: true,
      publication: CanonicalOccurrencePublication::PublishPending { eligible_from: 21 },
    }))
  );

  for (cycle_state, residence) in [
    (
      CycleState::Running,
      ProcessResidence::Service(ServiceResidenceKind::Live),
    ),
    (
      CycleState::Suspended,
      ProcessResidence::Deadline {
        key: WakeupKey::Block(31),
        page: 2,
        slot: 3,
      },
    ),
  ] {
    let busy = ActorProcess {
      residence: Some(residence),
      ..idle
    };
    assert_eq!(
      plan_canonical_occurrence(cycle_state, false, busy, 20),
      Ok(Some(CanonicalOccurrencePlan {
        process: busy,
        pending_signal: true,
        publication: CanonicalOccurrencePublication::PreserveResidence,
      }))
    );
  }

  assert_eq!(
    plan_canonical_occurrence(CycleState::Idle, true, idle, 20),
    Ok(None)
  );
  assert_eq!(
    plan_canonical_occurrence(CycleState::Running, false, idle, 20),
    Err(CanonicalOccurrenceError::InvalidResidence)
  );
  assert_eq!(
    plan_canonical_occurrence(CycleState::Idle, false, idle, u32::MAX),
    Err(CanonicalOccurrenceError::BlockNumberOverflow)
  );
  assert_eq!(
    plan_canonical_occurrence(
      CycleState::Idle,
      false,
      ActorProcess {
        status: ProcessStatus::Retired(CloseReason::OwnerInitiated),
        residence: None,
        ..idle
      },
      20,
    ),
    Err(CanonicalOccurrenceError::InvalidProcess)
  );
}

#[test]
fn legacy_process_transition_planner_enforces_owner_obligations_and_typed_successors() {
  let current = ActorProcess {
    generation: 11,
    last_attempted: Some(7),
    status: ProcessStatus::Serving,
    residence: Some(ProcessResidence::Service(ServiceResidenceKind::Live)),
  };
  let waiting = LegacyProcessPlacement::Waiting {
    key: WakeupKey::Block(9u32),
    page: 2,
    slot: 3,
  };
  let disabled = ProcessDisablement {
    cause: ProcessDisableCause::OwnerPaused,
    revival_authority: ProcessRevivalAuthority::Owner,
    basis: SuspendedProcessBasis::Running { eligible_at: 9u32 },
  };

  assert_eq!(
    plan_legacy_process_transition(
      current,
      ProcessTransitionObligation::PublishTypedResidence,
      LegacyProcessTransition::Publish(waiting),
    ),
    Ok(ActorProcess {
      generation: 11,
      last_attempted: Some(7),
      status: ProcessStatus::Serving,
      residence: Some(ProcessResidence::Deadline {
        key: WakeupKey::Block(9),
        page: 2,
        slot: 3,
      }),
    })
  );
  assert_eq!(
    plan_legacy_process_transition(
      current,
      ProcessTransitionObligation::AtomicSuccessorOrRemoval,
      LegacyProcessTransition::Replace(Some(waiting)),
    ),
    compile_legacy_process(11, Some(7), waiting).map_err(ProcessTransitionError::Compile)
  );
  assert_eq!(
    plan_legacy_process_transition(
      current,
      ProcessTransitionObligation::PreserveProcess,
      LegacyProcessTransition::Preserve,
    ),
    Ok(current)
  );
  assert_eq!(
    plan_legacy_process_transition(
      current,
      ProcessTransitionObligation::CarrierOnly,
      LegacyProcessTransition::CarrierOnly,
    ),
    Ok(current)
  );
  assert_eq!(
    plan_legacy_process_transition(
      current,
      ProcessTransitionObligation::AtomicSuccessorOrRemoval,
      LegacyProcessTransition::Replace(None),
    ),
    Err(ProcessTransitionError::DetachWithoutSuccessor)
  );
  for obligation in [
    ProcessTransitionObligation::PublishTypedResidence,
    ProcessTransitionObligation::AtomicSuccessorOrRemoval,
  ] {
    let transition = match obligation {
      ProcessTransitionObligation::PublishTypedResidence => {
        LegacyProcessTransition::Publish(LegacyProcessPlacement::Unsignaled(None))
      }
      ProcessTransitionObligation::AtomicSuccessorOrRemoval => {
        LegacyProcessTransition::Replace(Some(LegacyProcessPlacement::Unsignaled(None)))
      }
      _ => unreachable!(),
    };
    assert_eq!(
      plan_legacy_process_transition(current, obligation, transition),
      Err(ProcessTransitionError::Compile(
        ProcessCompileError::AmbiguousUnsignaled
      ))
    );
  }
  assert_eq!(
    plan_legacy_process_transition(
      current,
      ProcessTransitionObligation::RetireOrDisable,
      LegacyProcessTransition::Disable(disabled),
    ),
    Ok(ActorProcess {
      generation: 11,
      last_attempted: Some(7),
      status: ProcessStatus::Disabled(disabled),
      residence: None,
    })
  );
  assert_eq!(
    plan_legacy_process_transition(
      current,
      ProcessTransitionObligation::AtomicSuccessorOrRemoval,
      LegacyProcessTransition::Retire(CloseReason::OwnerInitiated),
    ),
    Ok(ActorProcess {
      generation: 11,
      last_attempted: Some(7),
      status: ProcessStatus::Retired(CloseReason::OwnerInitiated),
      residence: None,
    })
  );
  assert_eq!(
    plan_legacy_process_transition(
      current,
      ProcessTransitionObligation::PreserveProcess,
      LegacyProcessTransition::CarrierOnly,
    ),
    Err(ProcessTransitionError::ObligationMismatch)
  );
  assert_eq!(
    plan_legacy_process_transition(
      ActorProcess::<u32> {
        generation: 11,
        last_attempted: None,
        status: ProcessStatus::Serving,
        residence: None,
      },
      ProcessTransitionObligation::PreserveProcess,
      LegacyProcessTransition::Preserve,
    ),
    Err(ProcessTransitionError::InvalidCurrentProcess)
  );
}

#[test]
fn process_publication_is_transaction_local_single_authority_and_rollback_safe() {
  new_test_ext().execute_with(|| {
    let current = ActorProcess {
      generation: 11,
      last_attempted: Some(7),
      status: ProcessStatus::Serving,
      residence: Some(ProcessResidence::Service(ServiceResidenceKind::Live)),
    };
    let waiting = LegacyProcessPlacement::Waiting {
      key: WakeupKey::Block(9),
      page: 2,
      slot: 3,
    };

    assert_eq!(
      Actors::publish_legacy_process_transition(
        900,
        current,
        ProcessTransitionObligation::PublishTypedResidence,
        LegacyProcessTransition::Publish(waiting),
      ),
      Err(ProcessPublicationError::TransactionRequired)
    );

    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let dual_authority = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_legacy_process_transition(
          actor_id,
          current,
          ProcessTransitionObligation::PublishTypedResidence,
          LegacyProcessTransition::Publish(waiting),
        ),
      )
    });
    assert_eq!(
      dual_authority,
      Err(ProcessPublicationError::LegacyAuthorityPresent)
    );
    assert!(!ActorProcesses::<Test>::contains_key(actor_id));

    let rejected = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_legacy_process_transition(
          902,
          current,
          ProcessTransitionObligation::PublishTypedResidence,
          LegacyProcessTransition::Publish(LegacyProcessPlacement::Unsignaled(None)),
        ),
      )
    });
    assert_eq!(
      rejected,
      Err(ProcessPublicationError::Transition(
        ProcessTransitionError::Compile(ProcessCompileError::AmbiguousUnsignaled)
      ))
    );
    assert!(!ActorProcesses::<Test>::contains_key(902));

    let published = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_legacy_process_transition(
          900,
          current,
          ProcessTransitionObligation::PublishTypedResidence,
          LegacyProcessTransition::Publish(waiting),
        ),
      )
    })
    .expect("typed process publication succeeds");
    assert_eq!(ActorProcesses::<Test>::get(900), Some(published));

    let mismatched = ActorProcess {
      generation: 12,
      ..published
    };
    let mismatch = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_legacy_process_transition(
          900,
          mismatched,
          ProcessTransitionObligation::PreserveProcess,
          LegacyProcessTransition::Preserve,
        ),
      )
    });
    assert_eq!(
      mismatch,
      Err(ProcessPublicationError::CurrentProcessMismatch)
    );
    assert_eq!(ActorProcesses::<Test>::get(900), Some(published));

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::publish_legacy_process_transition(
        901,
        current,
        ProcessTransitionObligation::PublishTypedResidence,
        LegacyProcessTransition::Publish(waiting),
      )
      .expect("transaction-local publication is initially visible");
      assert!(ActorProcesses::<Test>::contains_key(901));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert!(!ActorProcesses::<Test>::contains_key(901));
  });
}

#[test]
fn service_publication_atomically_owns_process_and_ring_insertion() {
  new_test_ext().execute_with(|| {
    let first = actor_ref(910, 4);
    publish_test_service_member(first, ServiceResidenceKind::Live, 1)
      .expect("empty publication succeeds");
    assert_eq!(ServiceHeader::<Test>::get().count, 1);
    assert_eq!(
      ActorProcesses::<Test>::get(first.actor_id).map(|process| process.residence),
      Some(Some(ProcessResidence::Service(ServiceResidenceKind::Live)))
    );
    assert!(ServiceNodes::<Test>::contains_key(first.actor_id));

    let second = actor_ref(911, 2);
    publish_test_service_member(second, ServiceResidenceKind::Pending, 1)
      .expect("populated publication succeeds");
    assert_eq!(ServiceHeader::<Test>::get().count, 2);
    assert!(ActorProcesses::<Test>::contains_key(second.actor_id));
    assert!(ServiceNodes::<Test>::contains_key(second.actor_id));

    ServiceHeader::<Test>::mutate(|header| header.cursor = None);
    let rejected = actor_ref(912, 1);
    assert_eq!(
      publish_test_service_member(rejected, ServiceResidenceKind::Live, 1),
      Err(ServicePublicationError::Ring(
        ServiceRingMutationError::CorruptRing
      ))
    );
    assert!(!ActorProcesses::<Test>::contains_key(rejected.actor_id));
    assert!(!ServiceNodes::<Test>::contains_key(rejected.actor_id));
  });
}

#[test]
fn canonical_service_semantics_load_and_mutate_without_legacy_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let ActorSemanticState::Active(record) =
      ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);

    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1)
      .expect("canonical Service carrier publishes");
    assert_eq!(
      Actors::load_service_actor_semantic_state(actor, ServiceResidenceKind::Live),
      Ok(record.clone())
    );

    let mut replacement = record.hot.clone();
    replacement.unsuccessful_attempt_streak = 7;
    assert_eq!(
      Actors::try_store_service_control_hot(
        actor,
        ServiceResidenceKind::Live,
        replacement.clone(),
      ),
      Ok(())
    );
    assert_eq!(Actors::load_control_hot(actor_id), Some(replacement));
    assert!(!ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!ActorUnsignaledControlCells::<Test>::contains_key(actor_id));
    DeadlineHandles::<Test>::insert(
      actor_id,
      DeadlineHandle {
        actor,
        key: WakeupKey::Block(2),
        page: 0,
        slot: 0,
      },
    );
    assert_eq!(
      Actors::load_canonical_actor_semantic_state(actor),
      Err(ActorSemanticLoadError::Corrupt),
      "a Service process cannot retain a second process carrier"
    );
  });
}

#[test]
fn canonical_service_parking_installs_destination_before_releasing_membership() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let ActorSemanticState::Active(record) =
      ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1)
      .expect("canonical Service carrier publishes");
    let source = 19;
    let desired = [DependencyPlanSource {
      source,
      observed_revision: 0,
    }];
    let owner = PendingCheckOwner {
      actor,
      plan_revision: 7,
    };
    let evidence = ParkEvidence {
      plan_identity: record.admission.admission_identity,
      reason: ParkNegativeReason::SourceUnavailable,
      review_at: None,
    };
    let node = ServiceNodes::<Test>::get(actor_id).expect("member exists");

    assert_eq!(
      Actors::transfer_service_member_to_park(
        actor_ref(actor_id, actor.generation + 1),
        ServiceResidenceKind::Live,
        owner.plan_revision,
        evidence.reason,
        evidence.review_at,
        &desired,
        None,
      ),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    assert_eq!(ServiceNodes::<Test>::get(actor_id), Some(node));
    assert!(!PendingCheckOwners::<Test>::contains_key(actor_id));
    assert!(DependencyPlans::<Test>::get(actor_id).is_empty());
    assert_eq!(Actors::load_control_hot(actor_id), Some(record.hot.clone()));

    let mut stale_run_record = record.clone();
    stale_run_record.hot.cycle_state = CycleState::Running;
    ActorSemanticStates::<Test>::insert(actor_id, ActorSemanticState::Active(stale_run_record));
    assert_eq!(
      Actors::transfer_service_member_to_park(
        actor,
        ServiceResidenceKind::Live,
        owner.plan_revision,
        evidence.reason,
        evidence.review_at,
        &desired,
        None,
      ),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    ActorSemanticStates::<Test>::insert(actor_id, ActorSemanticState::Active(record.clone()));

    let head = ActorContractHeads::<Test>::get(actor_id).expect("Contract head exists");
    let mut stale_step_head = head.clone();
    stale_step_head.first_step = None;
    ActorContractHeads::<Test>::insert(actor_id, stale_step_head);
    assert_eq!(
      Actors::transfer_service_member_to_park(
        actor,
        ServiceResidenceKind::Live,
        owner.plan_revision,
        evidence.reason,
        evidence.review_at,
        &desired,
        None,
      ),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    ActorContractHeads::<Test>::insert(actor_id, head.clone());

    let mut stale_resources_head = head.clone();
    stale_resources_head.first_step_resources = None;
    ActorContractHeads::<Test>::insert(actor_id, stale_resources_head);
    assert_eq!(
      Actors::transfer_service_member_to_park(
        actor,
        ServiceResidenceKind::Live,
        owner.plan_revision,
        evidence.reason,
        evidence.review_at,
        &desired,
        None,
      ),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    ActorContractHeads::<Test>::insert(actor_id, head);
    assert_eq!(ServiceNodes::<Test>::get(actor_id), Some(node));
    assert!(!PendingCheckOwners::<Test>::contains_key(actor_id));
    assert!(DependencyPlans::<Test>::get(actor_id).is_empty());

    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        exhausted: true,
        ..Default::default()
      },
    );
    assert_eq!(
      Actors::transfer_service_member_to_park(
        actor,
        ServiceResidenceKind::Live,
        owner.plan_revision,
        evidence.reason,
        evidence.review_at,
        &desired,
        None,
      ),
      Err(DependencyRegistrationError::SourceExhausted)
    );
    assert_eq!(ServiceNodes::<Test>::get(actor_id), Some(node));
    assert!(!PendingCheckOwners::<Test>::contains_key(actor_id));
    assert!(DependencyPlans::<Test>::get(actor_id).is_empty());
    assert_eq!(Actors::load_control_hot(actor_id), Some(record.hot.clone()));
    DependencyRevisions::<Test>::remove(source);

    let occupied_deadline = Actors::plan_deadline_destination(actor, WakeupKey::Block(2)).unwrap();
    DeadlineHandles::<Test>::insert(actor_id, occupied_deadline);
    assert_eq!(
      Actors::transfer_service_member_to_park(
        actor,
        ServiceResidenceKind::Live,
        owner.plan_revision,
        evidence.reason,
        evidence.review_at,
        &desired,
        Some(WakeupKey::Block(2)),
      ),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    assert_eq!(ServiceNodes::<Test>::get(actor_id), Some(node));
    assert!(!PendingCheckOwners::<Test>::contains_key(actor_id));
    assert!(DependencyPlans::<Test>::get(actor_id).is_empty());
    assert!(!DependencyTimedReviews::<Test>::contains_key(actor_id));
    assert_eq!(
      DeadlineHandles::<Test>::get(actor_id),
      Some(occupied_deadline)
    );
    DeadlineHandles::<Test>::remove(actor_id);

    assert_eq!(
      Actors::transfer_service_member_to_park(
        actor,
        ServiceResidenceKind::Live,
        owner.plan_revision,
        evidence.reason,
        evidence.review_at,
        &desired,
        None,
      ),
      Ok(DependencyPlanMutation {
        installed: 1,
        ..Default::default()
      })
    );
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(PendingCheckOwners::<Test>::get(actor_id), Some(owner));
    assert_eq!(DependencyPlans::<Test>::get(actor_id).len(), 1);
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).map(|process| process.residence),
      Some(Some(ProcessResidence::Parked(evidence)))
    );
    assert_eq!(Actors::load_control_hot(actor_id), Some(record.hot.clone()));
    assert_eq!(
      Actors::load_canonical_actor_semantic_state(actor),
      Ok((
        record,
        ActorProcess {
          generation: actor.generation,
          last_attempted: None,
          status: ProcessStatus::Serving,
          residence: Some(ProcessResidence::Parked(evidence)),
        },
      ))
    );

    let stale_evidence = ParkEvidence {
      plan_identity: [8; 32],
      ..evidence
    };
    assert_eq!(
      Actors::wake_parked_member_to_service(
        actor,
        ServiceResidenceKind::Live,
        owner,
        stale_evidence,
        2,
      ),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(PendingCheckOwners::<Test>::get(actor_id), Some(owner));
    assert_eq!(DependencyPlans::<Test>::get(actor_id).len(), 1);

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::publish_dependency_event(source),
        Ok(DependencyPublicationMutation::Begun(1))
      );
      assert_eq!(
        Actors::process_dependency_scan_member(source, 1, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    let pending = PendingDependencyEvents::<Test>::get(actor_id).expect("positive result pending");
    let stale_pending = PendingDependencyEvent {
      revision: 0,
      ..pending
    };
    assert_eq!(
      Actors::consume_negative_dependency_event_and_rearm(stale_pending, evidence, &desired, None,),
      Err(DependencyRegistrationError::PendingEventMismatch)
    );
    assert_eq!(
      Actors::consume_negative_dependency_event_and_rearm(pending, stale_evidence, &desired, None,),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    assert_eq!(
      PendingDependencyEvents::<Test>::get(actor_id),
      Some(pending)
    );
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(DependencyPlans::<Test>::get(actor_id).len(), 1);

    let successor = [DependencyPlanSource {
      source,
      observed_revision: pending.revision,
    }];
    assert_eq!(
      Actors::consume_negative_dependency_event_and_rearm(
        pending,
        evidence,
        &successor,
        Some(WakeupKey::Block(2)),
      ),
      Ok(DependencyPlanMutation {
        retained: 1,
        timed_review: DependencyTimedReviewMutation::Installed,
        ..Default::default()
      })
    );
    assert!(!PendingDependencyEvents::<Test>::contains_key(actor_id));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).map(|process| process.residence),
      Some(Some(ProcessResidence::Parked(evidence)))
    );
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));

    frame_system::Pallet::<Test>::set_block_number(2);
    let review = DependencyTimedReview {
      owner,
      deadline: WakeupKey::Block(2),
    };
    assert_eq!(
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::publish_due_dependency_review(review),
        )
      }),
      Ok(DependencyDueReviewMutation::Published)
    );
    let stale_review = DependencyTimedReview {
      deadline: WakeupKey::Block(1),
      ..review
    };
    assert_eq!(
      Actors::consume_negative_dependency_review_and_rearm(
        stale_review,
        evidence,
        &successor,
        None,
      ),
      Err(DependencyRegistrationError::PendingReviewMismatch)
    );
    assert_eq!(
      Actors::consume_negative_dependency_review_and_rearm(
        review,
        stale_evidence,
        &successor,
        None,
      ),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    DependencyRevisions::<Test>::mutate(source, |state| state.revision += 1);
    assert_eq!(
      Actors::consume_negative_dependency_review_and_rearm(review, evidence, &successor, None,),
      Err(DependencyRegistrationError::RevisionMismatch)
    );
    DependencyRevisions::<Test>::mutate(source, |state| state.revision -= 1);
    assert_eq!(
      Actors::consume_negative_dependency_review_and_rearm(
        review,
        evidence,
        &successor,
        Some(WakeupKey::Block(2)),
      ),
      Err(DependencyRegistrationError::DeadlineNotFuture)
    );
    assert_eq!(
      PendingDependencyReviews::<Test>::get(actor_id),
      Some(review)
    );
    assert_eq!(DependencyPlans::<Test>::get(actor_id).len(), 1);
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).map(|process| process.residence),
      Some(Some(ProcessResidence::Parked(evidence)))
    );
    assert_eq!(
      Actors::consume_negative_dependency_review_and_rearm(review, evidence, &successor, None,),
      Ok(DependencyPlanMutation {
        retained: 1,
        ..Default::default()
      })
    );
    assert!(!PendingDependencyReviews::<Test>::contains_key(actor_id));
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::complete_dependency_scan(source, pending.revision, 1),
        Ok(DependencyScanMutation::Completed)
      );
      assert_eq!(
        Actors::publish_dependency_event(source),
        Ok(DependencyPublicationMutation::Begun(2))
      );
      assert_eq!(
        Actors::process_dependency_scan_member(source, 2, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    let newer_pending =
      PendingDependencyEvents::<Test>::get(actor_id).expect("newer positive result pending");
    assert_eq!(
      Actors::consume_negative_dependency_event_and_rearm(pending, evidence, &successor, None,),
      Err(DependencyRegistrationError::PendingEventMismatch)
    );
    assert_eq!(
      PendingDependencyEvents::<Test>::get(actor_id),
      Some(newer_pending)
    );
    assert_eq!(DependencyPlans::<Test>::get(actor_id).len(), 1);

    assert_eq!(
      Actors::consume_positive_dependency_event_and_wake(
        newer_pending,
        ServiceResidenceKind::Live,
        evidence,
        2,
      ),
      Ok(())
    );
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).map(|process| process.residence),
      Some(Some(ProcessResidence::Service(ServiceResidenceKind::Live)))
    );
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
    assert!(!PendingCheckOwners::<Test>::contains_key(actor_id));
    assert!(DependencyPlans::<Test>::get(actor_id).is_empty());
    assert!(!DependencyRegistrations::<Test>::contains_key(
      source, actor_id
    ));

    assert_eq!(
      Actors::wake_parked_member_to_service(actor, ServiceResidenceKind::Live, owner, evidence, 2,),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
  });
}

#[test]
fn positive_due_review_wakes_only_the_exact_current_park_episode() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let ActorSemanticState::Active(record) =
      ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1).unwrap();
    let source = 23;
    let observed = [DependencyPlanSource {
      source,
      observed_revision: 0,
    }];
    let owner = PendingCheckOwner {
      actor,
      plan_revision: 9,
    };
    let evidence = ParkEvidence {
      plan_identity: record.admission.admission_identity,
      reason: ParkNegativeReason::SourceUnavailable,
      review_at: Some(2),
    };
    Actors::transfer_service_member_to_park(
      actor,
      ServiceResidenceKind::Live,
      owner.plan_revision,
      evidence.reason,
      evidence.review_at,
      &observed,
      Some(WakeupKey::Block(2)),
    )
    .unwrap();
    frame_system::Pallet::<Test>::set_block_number(2);
    let review = DependencyTimedReview {
      owner,
      deadline: WakeupKey::Block(2),
    };
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::publish_due_dependency_review(review),
        Ok(DependencyDueReviewMutation::Published)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    let stale_review = DependencyTimedReview {
      deadline: WakeupKey::Block(1),
      ..review
    };
    assert_eq!(
      Actors::consume_positive_dependency_review_and_wake(
        stale_review,
        evidence,
        &observed,
        ServiceResidenceKind::Live,
        2,
      ),
      Err(DependencyRegistrationError::PendingReviewMismatch)
    );
    let stale_evidence = ParkEvidence {
      plan_identity: [7; 32],
      ..evidence
    };
    assert_eq!(
      Actors::consume_positive_dependency_review_and_wake(
        review,
        stale_evidence,
        &observed,
        ServiceResidenceKind::Live,
        2,
      ),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    DependencyRevisions::<Test>::mutate(source, |state| state.revision += 1);
    assert_eq!(
      Actors::consume_positive_dependency_review_and_wake(
        review,
        evidence,
        &observed,
        ServiceResidenceKind::Live,
        2,
      ),
      Err(DependencyRegistrationError::RevisionMismatch)
    );
    assert_eq!(
      PendingDependencyReviews::<Test>::get(actor_id),
      Some(review)
    );
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    DependencyRevisions::<Test>::mutate(source, |state| {
      state.revision -= 1;
      state.exhausted = true;
    });
    assert_eq!(
      Actors::consume_positive_dependency_review_and_wake(
        review,
        evidence,
        &observed,
        ServiceResidenceKind::Live,
        2,
      ),
      Err(DependencyRegistrationError::RevisionMismatch)
    );
    DependencyRevisions::<Test>::mutate(source, |state| state.exhausted = false);

    assert_eq!(
      Actors::consume_positive_dependency_review_and_wake(
        review,
        evidence,
        &observed,
        ServiceResidenceKind::Live,
        2,
      ),
      Ok(())
    );
    assert!(!PendingDependencyReviews::<Test>::contains_key(actor_id));
    assert!(!PendingCheckOwners::<Test>::contains_key(actor_id));
    assert!(DependencyPlans::<Test>::get(actor_id).is_empty());
    assert!(!DependencyRegistrations::<Test>::contains_key(
      source, actor_id
    ));
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).map(|process| process.residence),
      Some(Some(ProcessResidence::Service(ServiceResidenceKind::Live)))
    );
    assert_eq!(
      Actors::consume_positive_dependency_review_and_wake(
        review,
        evidence,
        &observed,
        ServiceResidenceKind::Live,
        2,
      ),
      Err(DependencyRegistrationError::PendingReviewMissing)
    );
  });
}

#[test]
fn bounded_due_review_worker_admits_one_atomic_oracle_attempt() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let ActorSemanticState::Active(record) =
      ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1).unwrap();
    let source = 30;
    let feed = 8;
    ObservationDependencySources::<Test>::insert(feed, source);
    DependencySourceObservations::<Test>::insert(source, feed);
    set_observation(feed, ScalarObservationState::Unavailable);
    let owner = PendingCheckOwner {
      actor,
      plan_revision: 12,
    };
    let evidence = ParkEvidence {
      plan_identity: record.admission.admission_identity,
      reason: ParkNegativeReason::SourceUnavailable,
      review_at: Some(2),
    };
    Actors::transfer_service_member_to_park(
      actor,
      ServiceResidenceKind::Live,
      owner.plan_revision,
      evidence.reason,
      evidence.review_at,
      &[DependencyPlanSource {
        source,
        observed_revision: 0,
      }],
      Some(WakeupKey::Block(2)),
    )
    .unwrap();
    let review = DependencyTimedReview {
      owner,
      deadline: WakeupKey::Block(2),
    };
    let weight = <<Test as crate::Config>::WeightInfo as crate::weights::WeightInfo>::process_due_observation_availability_review();
    let mut meter = WeightMeter::with_limit(weight);
    assert_eq!(
      Actors::process_due_observation_availability_review(
        &mut meter,
        review,
        evidence,
        ServiceResidenceKind::Live,
        1,
        Some(WakeupKey::Block(3)),
      ),
      Err(DependencyReviewWorkerError::Publication(
        DependencyDueReviewError::NotDue
      ))
    );
    assert_eq!(meter.consumed(), weight);
    assert_eq!(DependencyTimedReviews::<Test>::get(actor_id), Some(review));
    assert!(!PendingDependencyReviews::<Test>::contains_key(actor_id));

    frame_system::Pallet::<Test>::set_block_number(2);
    let mut no_weight = WeightMeter::with_limit(Weight::zero());
    assert_eq!(
      Actors::process_due_observation_availability_review(
        &mut no_weight,
        review,
        evidence,
        ServiceResidenceKind::Live,
        2,
        Some(WakeupKey::Block(3)),
      ),
      Err(DependencyReviewWorkerError::InsufficientWeight)
    );
    assert_eq!(DependencyTimedReviews::<Test>::get(actor_id), Some(review));

    let mut admitted = WeightMeter::with_limit(weight);
    assert!(matches!(
      Actors::process_due_observation_availability_review(
        &mut admitted,
        review,
        evidence,
        ServiceResidenceKind::Live,
        2,
        Some(WakeupKey::Block(3)),
      ),
      Ok(DependencyReviewMutation::Rearmed(_))
    ));
    assert_eq!(
      DependencyTimedReviews::<Test>::get(actor_id),
      Some(DependencyTimedReview {
        owner,
        deadline: WakeupKey::Block(3),
      })
    );

    frame_system::Pallet::<Test>::set_block_number(3);
    let next = DependencyTimedReview {
      owner,
      deadline: WakeupKey::Block(3),
    };
    PendingDependencyEvents::<Test>::insert(
      actor_id,
      PendingDependencyEvent {
        owner,
        source,
        revision: 0,
      },
    );
    let mut occupied = WeightMeter::with_limit(weight);
    assert_eq!(
      Actors::process_due_observation_availability_review(
        &mut occupied,
        next,
        evidence,
        ServiceResidenceKind::Live,
        3,
        None,
      ),
      Err(DependencyReviewWorkerError::Publication(
        DependencyDueReviewError::DestinationOccupied
      ))
    );
    assert_eq!(DependencyTimedReviews::<Test>::get(actor_id), Some(next));
    assert!(!PendingDependencyReviews::<Test>::contains_key(actor_id));
    PendingDependencyEvents::<Test>::remove(actor_id);

    set_observation(feed, ScalarObservationState::Uninitialized);
    let mut refused = WeightMeter::with_limit(weight);
    assert_eq!(
      Actors::process_due_observation_availability_review(
        &mut refused,
        next,
        evidence,
        ServiceResidenceKind::Live,
        3,
        None,
      ),
      Err(DependencyReviewWorkerError::Interpretation(
        DependencyRegistrationError::SourceUninitialized
      ))
    );
    assert_eq!(DependencyTimedReviews::<Test>::get(actor_id), Some(next));
    assert!(!PendingDependencyReviews::<Test>::contains_key(actor_id));

    set_observation(
      feed,
      ScalarObservationState::Fresh {
        value: 1,
        observed_at: 3,
      },
    );
    let mut wake = WeightMeter::with_limit(weight);
    assert_eq!(
      Actors::process_due_observation_availability_review(
        &mut wake,
        next,
        evidence,
        ServiceResidenceKind::Live,
        3,
        None,
      ),
      Ok(DependencyReviewMutation::Woke)
    );
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
    assert!(!DependencyTimedReviews::<Test>::contains_key(actor_id));
  });
}

#[test]
fn due_review_deadline_traversal_is_weight_gated_and_atomic() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let ActorSemanticState::Active(record) =
      ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1).unwrap();
    let source = 31;
    let feed = 9;
    ObservationDependencySources::<Test>::insert(feed, source);
    DependencySourceObservations::<Test>::insert(source, feed);
    set_observation(feed, ScalarObservationState::Unavailable);
    let owner = PendingCheckOwner {
      actor,
      plan_revision: 13,
    };
    let evidence = ParkEvidence {
      plan_identity: record.admission.admission_identity,
      reason: ParkNegativeReason::SourceUnavailable,
      review_at: Some(2),
    };
    Actors::transfer_service_member_to_park(
      actor,
      ServiceResidenceKind::Live,
      owner.plan_revision,
      evidence.reason,
      evidence.review_at,
      &[DependencyPlanSource {
        source,
        observed_revision: 0,
      }],
      Some(WakeupKey::Block(2)),
    )
    .unwrap();
    let first_handle =
      DeadlineHandles::<Test>::get(actor_id).expect("parking publishes timed review deadline");
    assert_eq!(first_handle.key, WakeupKey::Block(2));

    frame_system::Pallet::<Test>::set_block_number(2);
    let mut no_weight = WeightMeter::with_limit(Weight::zero());
    assert_eq!(
      Actors::process_next_due_block_observation_availability_review(
        &mut no_weight,
        ServiceResidenceKind::Live,
        2,
        Some(WakeupKey::Block(3)),
      ),
      Err(DependencyReviewWorkerError::InsufficientWeight)
    );
    assert_eq!(DeadlineHandles::<Test>::get(actor_id), Some(first_handle));
    assert!(!PendingDependencyReviews::<Test>::contains_key(actor_id));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).map(|process| process.residence),
      Some(Some(ProcessResidence::Parked(evidence)))
    );

    let selector_weight = <<Test as crate::Config>::WeightInfo as crate::weights::WeightInfo>::classify_due_block_deadline();
    let review_weight = <<Test as crate::Config>::WeightInfo as crate::weights::WeightInfo>::process_due_observation_availability_review();
    let retry_weight = <<Test as crate::Config>::WeightInfo as crate::weights::WeightInfo>::return_due_block_deadline_to_service();
    let mut branch_refused =
      WeightMeter::with_limit(selector_weight.saturating_add(retry_weight));
    assert_eq!(
      Actors::process_next_due_block_deadline(
        &mut branch_refused,
        ServiceResidenceKind::Live,
        2,
        Some(WakeupKey::Block(3)),
      ),
      Err(DependencyReviewWorkerError::InsufficientWeight)
    );
    assert_eq!(DeadlineHandles::<Test>::get(actor_id), Some(first_handle));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).map(|process| process.residence),
      Some(Some(ProcessResidence::Parked(evidence)))
    );
    let weight = selector_weight.saturating_add(review_weight);
    let mut admitted = WeightMeter::with_limit(weight);
    assert!(matches!(
      Actors::process_next_due_block_deadline(
        &mut admitted,
        ServiceResidenceKind::Live,
        2,
        Some(WakeupKey::Block(3)),
      ),
      Ok(DueBlockDeadlineMutation::ReviewProcessed(
        current,
        DependencyReviewMutation::Rearmed(_)
      )) if current == actor
    ));
    let rearmed_handle = DeadlineHandles::<Test>::get(actor_id).expect("review stays indexed");
    assert_eq!(rearmed_handle.key, WakeupKey::Block(3));
    assert!(!PendingDependencyReviews::<Test>::contains_key(actor_id));

    frame_system::Pallet::<Test>::set_block_number(3);
    set_observation(feed, ScalarObservationState::Uninitialized);
    let mut refused = WeightMeter::with_limit(weight);
    assert_eq!(
      Actors::process_next_due_block_observation_availability_review(
        &mut refused,
        ServiceResidenceKind::Live,
        3,
        None,
      ),
      Err(DependencyReviewWorkerError::Interpretation(
        DependencyRegistrationError::SourceUninitialized
      ))
    );
    assert_eq!(DeadlineHandles::<Test>::get(actor_id), Some(rearmed_handle));
    assert!(!PendingDependencyReviews::<Test>::contains_key(actor_id));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).map(|process| process.residence),
      Some(Some(ProcessResidence::Parked(evidence)))
    );

    set_observation(
      feed,
      ScalarObservationState::Fresh {
        value: 1,
        observed_at: 3,
      },
    );
    let mut wake = WeightMeter::with_limit(weight);
    assert_eq!(
      Actors::process_next_due_block_observation_availability_review(
        &mut wake,
        ServiceResidenceKind::Live,
        3,
        None,
      ),
      Ok((actor, DependencyReviewMutation::Woke))
    );
    assert!(!DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
    assert!(!DependencyTimedReviews::<Test>::contains_key(actor_id));
  });
}

#[test]
fn due_tick_review_uses_its_own_frontier_and_preserves_refused_work() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(2);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let ActorSemanticState::Active(record) =
      ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1).unwrap();
    let source = 32;
    let feed = 10;
    ObservationDependencySources::<Test>::insert(feed, source);
    DependencySourceObservations::<Test>::insert(source, feed);
    set_observation(feed, ScalarObservationState::Unavailable);
    let evidence = ParkEvidence {
      plan_identity: record.admission.admission_identity,
      reason: ParkNegativeReason::SourceUnavailable,
      review_at: Some(2),
    };
    Actors::transfer_service_member_to_park(
      actor,
      ServiceResidenceKind::Live,
      14,
      evidence.reason,
      evidence.review_at,
      &[DependencyPlanSource { source, observed_revision: 0 }],
      Some(WakeupKey::Tick(7)),
    )
    .unwrap();
    let retained = DeadlineHandles::<Test>::get(actor_id).expect("Tick review is indexed");
    frame_system::Pallet::<Test>::set_block_number(7);
    assert!(matches!(Actors::classify_next_due_block_deadline(7), Err(DeadlineMutationError::MemberMissing)));
    assert!(matches!(Actors::classify_next_due_tick_deadline(6), Err(DeadlineMutationError::InvalidDestination)));

    let selector_weight = <<Test as crate::Config>::WeightInfo as crate::weights::WeightInfo>::classify_due_tick_deadline();
    let review_weight = <<Test as crate::Config>::WeightInfo as crate::weights::WeightInfo>::process_due_observation_availability_review();
    let mut selector_refused = WeightMeter::with_limit(Weight::zero());
    assert_eq!(
      Actors::process_next_due_tick_deadline(&mut selector_refused, ServiceResidenceKind::Live, 7, 7, Some(WakeupKey::Tick(8))),
      Err(DependencyReviewWorkerError::InsufficientWeight)
    );
    let mut branch_refused = WeightMeter::with_limit(selector_weight);
    assert_eq!(
      Actors::process_next_due_tick_deadline(&mut branch_refused, ServiceResidenceKind::Live, 7, 7, Some(WakeupKey::Tick(8))),
      Err(DependencyReviewWorkerError::InsufficientWeight)
    );
    assert_eq!(DeadlineHandles::<Test>::get(actor_id), Some(retained));
    assert!(!PendingDependencyReviews::<Test>::contains_key(actor_id));

    let block_selector = <<Test as crate::Config>::WeightInfo as crate::weights::WeightInfo>::classify_due_block_deadline();
    let retry_weight = <<Test as crate::Config>::WeightInfo as crate::weights::WeightInfo>::return_due_block_deadline_to_service();
    let complete = block_selector
      .saturating_add(retry_weight.max(review_weight))
      .saturating_add(selector_weight)
      .saturating_add(review_weight);
    let mut admitted = WeightMeter::with_limit(complete);
    let pass = Actors::service_due_deadline_frontiers(
      &mut admitted,
      ServiceResidenceKind::Live,
      7,
      7,
      None,
      Some(WakeupKey::Tick(8)),
    )
    .unwrap();
    assert_eq!(
      pass.block,
      Err(DependencyReviewWorkerError::Deadline(
        DeadlineMutationError::MemberMissing
      ))
    );
    assert!(matches!(
      pass.tick,
      Ok(DueTickDeadlineMutation::ReviewProcessed(current, DependencyReviewMutation::Rearmed(_))) if current == actor
    ));
    assert_eq!(DeadlineHandles::<Test>::get(actor_id).map(|handle| handle.key), Some(WakeupKey::Tick(8)));
    assert!(matches!(Actors::classify_next_due_block_deadline(7), Err(DeadlineMutationError::MemberMissing)));
  });
}

#[test]
fn mandatory_deadline_service_reserves_both_independent_frontiers() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(6);
    let mut actors = Vec::new();
    for (source, feed, deadline) in [
      (41, 11, WakeupKey::Block(7)),
      (42, 12, WakeupKey::Tick(7)),
    ] {
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      let ActorSemanticState::Active(record) =
        ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
      else {
        panic!("created Actor is active");
      };
      let actor = actor_ref(actor_id, record.generation);
      ActorControlLocators::<Test>::remove(actor_id);
      ActorUnsignaledControlCells::<Test>::remove(actor_id);
      publish_test_service_member(actor, ServiceResidenceKind::Live, 1).unwrap();
      ObservationDependencySources::<Test>::insert(feed, source);
      DependencySourceObservations::<Test>::insert(source, feed);
      set_observation(feed, ScalarObservationState::Unavailable);
      Actors::transfer_service_member_to_park(
        actor,
        ServiceResidenceKind::Live,
        u64::from(source),
        ParkNegativeReason::SourceUnavailable,
        Some(7),
        &[DependencyPlanSource { source, observed_revision: 0 }],
        Some(deadline),
      )
      .unwrap();
      set_observation(
        feed,
        ScalarObservationState::Fresh {
          value: 1,
          observed_at: 7,
        },
      );
      actors.push(actor);
    }
    frame_system::Pallet::<Test>::set_block_number(7);

    let block_selector = <<Test as crate::Config>::WeightInfo as crate::weights::WeightInfo>::classify_due_block_deadline();
    let tick_selector = <<Test as crate::Config>::WeightInfo as crate::weights::WeightInfo>::classify_due_tick_deadline();
    let review = <<Test as crate::Config>::WeightInfo as crate::weights::WeightInfo>::process_due_observation_availability_review();
    let retry = <<Test as crate::Config>::WeightInfo as crate::weights::WeightInfo>::return_due_block_deadline_to_service();
    let complete = block_selector
      .saturating_add(retry.max(review))
      .saturating_add(tick_selector)
      .saturating_add(review);
    let mut refused = WeightMeter::with_limit(complete.saturating_sub(Weight::from_parts(1, 0)));
    assert_eq!(
      Actors::service_due_deadline_frontiers(
        &mut refused,
        ServiceResidenceKind::Live,
        7,
        7,
        None,
        None,
      ),
      Err(DependencyReviewWorkerError::InsufficientWeight)
    );
    assert!(actors.iter().all(|actor| DeadlineHandles::<Test>::contains_key(actor.actor_id)));

    let mut admitted = WeightMeter::with_limit(complete);
    let pass = Actors::service_due_deadline_frontiers(
      &mut admitted,
      ServiceResidenceKind::Live,
      7,
      7,
      None,
      None,
    )
    .unwrap();
    assert_eq!(
      pass.block,
      Ok(DueBlockDeadlineMutation::ReviewProcessed(
        actors[0],
        DependencyReviewMutation::Woke,
      ))
    );
    assert_eq!(
      pass.tick,
      Ok(DueTickDeadlineMutation::ReviewProcessed(
        actors[1],
        DependencyReviewMutation::Woke,
      ))
    );
    assert!(actors.iter().all(|actor| !DeadlineHandles::<Test>::contains_key(actor.actor_id)));
  });
}

#[test]
fn on_idle_services_due_block_and_tick_frontiers_with_current_clocks() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(6);
    let mut actors = Vec::new();
    for (source, feed, deadline) in [(51, 21, WakeupKey::Block(7)), (52, 22, WakeupKey::Tick(7))] {
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      let ActorSemanticState::Active(record) =
        ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
      else {
        panic!("created Actor is active");
      };
      let actor = actor_ref(actor_id, record.generation);
      ActorControlLocators::<Test>::remove(actor_id);
      ActorUnsignaledControlCells::<Test>::remove(actor_id);
      publish_test_service_member(actor, ServiceResidenceKind::Live, 6).unwrap();
      ObservationDependencySources::<Test>::insert(feed, source);
      DependencySourceObservations::<Test>::insert(source, feed);
      set_observation(feed, ScalarObservationState::Unavailable);
      Actors::transfer_service_member_to_park(
        actor,
        ServiceResidenceKind::Live,
        u64::from(source),
        ParkNegativeReason::SourceUnavailable,
        Some(7),
        &[DependencyPlanSource {
          source,
          observed_revision: 0,
        }],
        Some(deadline),
      )
      .unwrap();
      set_observation(
        feed,
        ScalarObservationState::Fresh {
          value: 1,
          observed_at: 7,
        },
      );
      actors.push(actor);
    }
    frame_system::Pallet::<Test>::set_block_number(7);

    assert_eq!(Actors::on_idle(7, Weight::zero()), Weight::zero());
    assert!(
      actors
        .iter()
        .all(|actor| DeadlineHandles::<Test>::contains_key(actor.actor_id))
    );

    let consumed = Actors::on_idle(7, Weight::MAX);
    assert_ne!(consumed, Weight::zero());
    assert!(
      actors
        .iter()
        .all(|actor| ServiceNodes::<Test>::contains_key(actor.actor_id))
    );
    assert!(
      actors
        .iter()
        .all(|actor| !DeadlineHandles::<Test>::contains_key(actor.actor_id))
    );
  });
}

#[test]
fn due_review_interpreter_routes_one_snapshot_without_consuming_refusals() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let ActorSemanticState::Active(record) =
      ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1).unwrap();
    let source = 29;
    let feed = 7;
    ObservationDependencySources::<Test>::insert(feed, source);
    DependencySourceObservations::<Test>::insert(source, feed);
    set_observation(feed, ScalarObservationState::Unavailable);
    let initial = [DependencyPlanSource {
      source,
      observed_revision: 0,
    }];
    let owner = PendingCheckOwner {
      actor,
      plan_revision: 11,
    };
    let evidence = ParkEvidence {
      plan_identity: record.admission.admission_identity,
      reason: ParkNegativeReason::SourceUnavailable,
      review_at: Some(2),
    };
    Actors::transfer_service_member_to_park(
      actor,
      ServiceResidenceKind::Live,
      owner.plan_revision,
      evidence.reason,
      evidence.review_at,
      &initial,
      Some(WakeupKey::Block(2)),
    )
    .unwrap();
    frame_system::Pallet::<Test>::set_block_number(2);
    let first = DependencyTimedReview {
      owner,
      deadline: WakeupKey::Block(2),
    };
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::publish_due_dependency_review(first),
        Ok(DependencyDueReviewMutation::Published)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    assert_eq!(
      Actors::interpret_pending_observation_availability_review(
        first,
        evidence,
        ServiceResidenceKind::Live,
        2,
        Some(WakeupKey::Block(3)),
      ),
      Ok(DependencyReviewMutation::Rearmed(DependencyPlanMutation {
        retained: 1,
        timed_review: DependencyTimedReviewMutation::Installed,
        ..Default::default()
      }))
    );
    assert!(!PendingDependencyReviews::<Test>::contains_key(actor_id));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).map(|process| process.residence),
      Some(Some(ProcessResidence::Parked(evidence)))
    );

    frame_system::Pallet::<Test>::set_block_number(3);
    let second = DependencyTimedReview {
      owner,
      deadline: WakeupKey::Block(3),
    };
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::publish_due_dependency_review(second),
        Ok(DependencyDueReviewMutation::Published)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    let stale_evidence = ParkEvidence {
      plan_identity: [7; 32],
      ..evidence
    };
    assert_eq!(
      Actors::interpret_pending_observation_availability_review(
        second,
        stale_evidence,
        ServiceResidenceKind::Live,
        3,
        None,
      ),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );

    set_observation(feed, ScalarObservationState::Uninitialized);
    assert_eq!(
      Actors::interpret_pending_observation_availability_review(
        second,
        evidence,
        ServiceResidenceKind::Live,
        3,
        None,
      ),
      Err(DependencyRegistrationError::SourceUninitialized)
    );

    DependencySourceObservations::<Test>::remove(source);
    assert_eq!(
      Actors::interpret_pending_observation_availability_review(
        second,
        evidence,
        ServiceResidenceKind::Live,
        3,
        None,
      ),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    DependencySourceObservations::<Test>::insert(source, feed);
    ObservationDependencySources::<Test>::insert(feed, source + 1);
    assert_eq!(
      Actors::interpret_pending_observation_availability_review(
        second,
        evidence,
        ServiceResidenceKind::Live,
        3,
        None,
      ),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    ObservationDependencySources::<Test>::insert(feed, source);

    set_observation(
      feed,
      ScalarObservationState::Fresh {
        value: 1,
        observed_at: 3,
      },
    );
    race_observation_source_revision_once(source);
    assert_eq!(
      Actors::interpret_pending_observation_availability_review(
        second,
        evidence,
        ServiceResidenceKind::Live,
        3,
        None,
      ),
      Err(DependencyRegistrationError::RevisionMismatch)
    );
    assert_eq!(DependencyRevisions::<Test>::get(source).revision, 0);

    ServiceNodes::<Test>::insert(
      actor_id,
      ServiceNode {
        generation: actor.generation,
        previous: actor,
        next: actor,
        kind: ServiceResidenceKind::Live,
        eligible_from: 2,
        last_considered: 2,
      },
    );
    assert_eq!(
      Actors::interpret_pending_observation_availability_review(
        second,
        evidence,
        ServiceResidenceKind::Live,
        3,
        None,
      ),
      Err(DependencyRegistrationError::StoredPlanMismatch)
    );
    ServiceNodes::<Test>::remove(actor_id);

    assert_eq!(
      PendingDependencyReviews::<Test>::get(actor_id),
      Some(second)
    );
    assert_eq!(DependencyPlans::<Test>::get(actor_id).len(), 1);
    assert!(DependencyRegistrations::<Test>::contains_key(
      source, actor_id
    ));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).map(|process| process.residence),
      Some(Some(ProcessResidence::Parked(evidence)))
    );

    assert_eq!(
      Actors::interpret_pending_observation_availability_review(
        second,
        evidence,
        ServiceResidenceKind::Live,
        3,
        None,
      ),
      Ok(DependencyReviewMutation::Woke)
    );
    assert!(!PendingDependencyReviews::<Test>::contains_key(actor_id));
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
  });
}

#[test]
fn canonical_service_semantics_reject_stale_generation_and_residence_without_mutation() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let ActorSemanticState::Active(record) =
      ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1)
      .expect("canonical Service carrier publishes");

    let mut rejected = record.hot.clone();
    rejected.unsuccessful_attempt_streak = 9;
    let stale = actor_ref(actor_id, actor.generation + 1);
    for (reference, kind) in [
      (stale, ServiceResidenceKind::Live),
      (actor, ServiceResidenceKind::Pending),
    ] {
      assert_eq!(
        Actors::load_service_actor_semantic_state(reference, kind),
        Err(ActorSemanticLoadError::Corrupt)
      );
      assert_eq!(
        Actors::try_store_service_control_hot(reference, kind, rejected.clone()),
        Err(crate::scheduler::EnqueueOutcome::CorruptedTopology)
      );
      assert_eq!(Actors::load_control_hot(actor_id), Some(record.hot.clone()));
    }
  });
}

#[test]
fn retained_service_attempt_commits_semantics_before_advance_and_preserves_refusals() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(2);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let ActorSemanticState::Active(record) =
      ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1)
      .expect("canonical Service carrier publishes");
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::begin_service_round(2).expect("round begins");
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    let header_before_refusal = ServiceHeader::<Test>::get();
    let node_before_refusal = ServiceNodes::<Test>::get(actor_id).unwrap();
    let mut replacement = record.hot.clone();
    replacement.unsuccessful_attempt_streak = 11;
    assert_eq!(
      Actors::commit_retained_service_attempt(
        actor,
        ServiceResidenceKind::Pending,
        record.identity.clone(),
        replacement.clone(),
        2,
      ),
      Err(ServiceRoundError::ProcessResidenceMismatch)
    );
    assert_eq!(Actors::load_control_hot(actor_id), Some(record.hot));
    assert_eq!(ServiceHeader::<Test>::get(), header_before_refusal);
    assert_eq!(
      ServiceNodes::<Test>::get(actor_id),
      Some(node_before_refusal)
    );

    assert_eq!(
      Actors::commit_retained_service_attempt(
        actor,
        ServiceResidenceKind::Live,
        record.identity.clone(),
        replacement.clone(),
        2,
      ),
      Ok(())
    );
    assert_eq!(Actors::load_control_hot(actor_id), Some(replacement));
    assert_eq!(
      ServiceNodes::<Test>::get(actor_id).unwrap().last_considered,
      2
    );
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id)
        .unwrap()
        .last_attempted,
      Some(2)
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::consider_service_head(2),
        Ok(ServiceRoundEncounter::Closed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
  });
}

#[test]
fn canonical_zero_step_service_attempt_commits_semantics_before_ring_advance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(2);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, Default::default());
    let state = Actors::active_actor_state(actor_id).expect("active zero-Step state");
    let ActorSemanticState::Active(record) =
      ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists")
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    publish_test_service_member(actor, ServiceResidenceKind::Live, 1)
      .expect("canonical Service carrier publishes");
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::begin_service_round(2).expect("round begins");
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    Actors::execute_zero_step_on_service(
      actor,
      ServiceResidenceKind::Live,
      state,
      &record.admission,
      2,
      None,
    )
    .expect("canonical zero-Step attempt commits");
    let stored = Actors::load_service_actor_semantic_state(actor, ServiceResidenceKind::Live)
      .expect("retained semantic owner remains loadable");
    assert_eq!(stored.identity.cycle_nonce, 1);
    assert_eq!(stored.hot.last_cycle_block, Some(2));
    assert_eq!(
      ServiceNodes::<Test>::get(actor_id).unwrap().last_considered,
      2
    );
  });
}

#[test]
fn canonical_effectful_completion_commits_before_service_advance() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(2);
    let step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(1),
      amount: AmountResolution::Fixed(1),
    });
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![step]).expect("one-Step Contract"),
    );
    let sovereign = Actors::actor_identity(actor_id)
      .expect("active identity")
      .sovereign_account;
    set_asset_balance(&sovereign, TestAsset::Local(1), 10);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let admission_block = frame_system::Pallet::<Test>::block_number();
    Actors::on_idle(admission_block, Weight::MAX);
    let now = admission_block + 1;
    frame_system::Pallet::<Test>::set_block_number(now);
    Actors::execute_cycle(Weight::MAX);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 1);
    let stored = Actors::active_actor_state(actor_id)
      .expect("retained canonical process authority remains loadable");
    assert_eq!(stored.identity.cycle_nonce, 1);
    assert_eq!(stored.hot.last_cycle_block, Some(now));
    assert_eq!(
      ServiceNodes::<Test>::get(actor_id).unwrap().last_considered,
      now
    );
  });
}

#[test]
fn canonical_effectful_later_retry_moves_through_deadline_and_reenters_once() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(2);
    let mut step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(1),
      amount: AmountResolution::Fixed(1),
    });
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 3,
      },
      None,
      BoundedVec::try_from(vec![step]).expect("one-Step retry Contract"),
    );
    let sovereign = Actors::actor_identity(actor_id)
      .expect("active identity")
      .sovereign_account;
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let admission_block = frame_system::Pallet::<Test>::block_number();
    Actors::on_idle(admission_block, Weight::MAX);
    let now = admission_block + 1;
    frame_system::Pallet::<Test>::set_block_number(now);
    Actors::execute_cycle(Weight::MAX);
    let generation =
      match ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists") {
        ActorSemanticState::Active(record) => record.generation,
        ActorSemanticState::Dormant(_) => panic!("created Actor is active"),
      };
    let actor = actor_ref(actor_id, generation);
    let retry_at = now + 3;
    let retry_run = ActorRunStateStore::<Test>::get(actor_id).expect("retry Run remains");
    assert_eq!(retry_run.unsuccessful_attempts_at_cursor, 1);
    assert_eq!(retry_run.eligible_at, retry_at);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(
      DeadlineHandles::<Test>::get(actor_id),
      Some(DeadlineHandle {
        actor,
        key: WakeupKey::Block(retry_at),
        page: 0,
        slot: 0,
      })
    );
    assert!(matches!(
      Actors::return_next_due_block_deadline_to_service(ServiceResidenceKind::Live, now),
      Err(DeadlineMutationError::InvalidDestination)
    ));

    frame_system::Pallet::<Test>::set_block_number(retry_at);
    let selector_weight = <<Test as crate::Config>::WeightInfo as crate::weights::WeightInfo>::classify_due_block_deadline();
    let retry_weight = <<Test as crate::Config>::WeightInfo as crate::weights::WeightInfo>::return_due_block_deadline_to_service();
    let deadline_weight = selector_weight.saturating_add(retry_weight);
    let mut no_weight = WeightMeter::with_limit(Weight::zero());
    assert_eq!(
      Actors::process_next_due_block_deadline(
        &mut no_weight,
        ServiceResidenceKind::Live,
        retry_at,
        None,
      ),
      Err(DependencyReviewWorkerError::InsufficientWeight)
    );
    assert!(DeadlineHandles::<Test>::contains_key(actor_id));
    let mut branch_refused = WeightMeter::with_limit(selector_weight);
    assert_eq!(
      Actors::process_next_due_block_deadline(
        &mut branch_refused,
        ServiceResidenceKind::Live,
        retry_at,
        None,
      ),
      Err(DependencyReviewWorkerError::InsufficientWeight)
    );
    assert!(DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    let mut admitted = WeightMeter::with_limit(deadline_weight);
    assert_eq!(
      Actors::process_next_due_block_deadline(
        &mut admitted,
        ServiceResidenceKind::Live,
        retry_at,
        None,
      ),
      Ok(DueBlockDeadlineMutation::RetryReturned(actor))
    );
    assert!(!DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(matches!(
      Actors::return_next_due_block_deadline_to_service(ServiceResidenceKind::Live, retry_at),
      Err(DeadlineMutationError::MemberMissing)
    ));
    set_asset_balance(&sovereign, TestAsset::Local(1), 10);
    let stored_run = ActorRunStateStore::<Test>::get(actor_id).expect("retry Run stays stored");
    assert_eq!(stored_run.cursor, retry_run.cursor);
    assert_eq!(stored_run.eligible_at, retry_run.eligible_at);
    assert_eq!(
      stored_run.unsuccessful_attempts_at_cursor,
      retry_run.unsuccessful_attempts_at_cursor
    );
    Actors::execute_cycle(Weight::MAX);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 1);
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    Actors::execute_cycle(Weight::MAX);
    assert_eq!(
      asset_balance(&BOB, TestAsset::Local(1)),
      1,
      "the consumed due retry cannot execute twice"
    );
  });
}

#[test]
fn canonical_effectful_terminal_attempt_rolls_back_then_cleans_before_service_unlink() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(2);
    let step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(1),
      amount: AmountResolution::Fixed(1),
    });
    let actor_id = Actors::next_actor_id();
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      system_active_contract_with_completion(
        manual_schedule(),
        None,
        contract_steps_with_step(step),
        CompletionPolicy::CloseAfterProductiveCycle,
      ),
    ));
    let sovereign = Actors::actor_identity(actor_id)
      .expect("active identity")
      .sovereign_account;
    set_asset_balance(&sovereign, TestAsset::Local(1), 10);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let admission_block = frame_system::Pallet::<Test>::block_number();
    Actors::on_idle(admission_block, Weight::MAX);
    let now = admission_block + 1;
    frame_system::Pallet::<Test>::set_block_number(now);
    let generation =
      match ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists") {
        ActorSemanticState::Active(record) => record.generation,
        ActorSemanticState::Dormant(_) => panic!("created Actor is active"),
      };
    let actor = actor_ref(actor_id, generation);
    let service_header = ServiceHeader::<Test>::get();
    assert_eq!(service_header.cursor, Some(actor));

    let recipient_before = asset_balance(&BOB, TestAsset::Local(1));
    ServiceHeader::<Test>::mutate(|header| header.cursor = None);
    let refused = Actors::execute_cycle(Weight::MAX);
    assert!(refused.starved);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), recipient_before);
    assert!(ActorSemanticStates::<Test>::contains_key(actor_id));
    assert!(ServiceNodes::<Test>::contains_key(actor_id));

    ServiceHeader::<Test>::put(service_header);
    Actors::execute_cycle(Weight::MAX);
    assert_eq!(
      asset_balance(&BOB, TestAsset::Local(1)),
      recipient_before + 1
    );
    assert!(!ActorSemanticStates::<Test>::contains_key(actor_id));
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).map(|process| (process.status, process.residence)),
      Some((
        ProcessStatus::Retired(CloseReason::ProductiveCycleCompleted),
        None,
      ))
    );
    assert_eq!(ServiceHeader::<Test>::get(), ServiceHeaderRecord::default());
  });
}

#[test]
fn canonical_zero_step_terminal_attempt_cleans_before_service_unlink() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(2);
    let mut contract = system_active_contract(manual_schedule(), None, Default::default())
      .expect("zero-Step Contract");
    contract.auto_close_at_cycle_nonce = Some(1);
    let actor_id = Actors::next_actor_id();
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
    let admission_block = frame_system::Pallet::<Test>::block_number();
    Actors::on_idle(admission_block, Weight::MAX);
    let now = admission_block + 1;
    frame_system::Pallet::<Test>::set_block_number(now);
    let generation =
      match ActorSemanticStates::<Test>::get(actor_id).expect("semantic owner exists") {
        ActorSemanticState::Active(record) => record.generation,
        ActorSemanticState::Dormant(_) => panic!("created Actor is active"),
      };
    let actor = actor_ref(actor_id, generation);
    assert_eq!(ServiceHeader::<Test>::get().cursor, Some(actor));

    Actors::execute_cycle(Weight::MAX);
    assert!(!ActorSemanticStates::<Test>::contains_key(actor_id));
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).map(|process| (process.status, process.residence)),
      Some((
        ProcessStatus::Retired(CloseReason::AutoCloseNonceReached),
        None,
      ))
    );
    assert_eq!(ServiceHeader::<Test>::get(), ServiceHeaderRecord::default());
  });
}

#[test]
fn service_retirement_atomically_unlinks_each_topology_and_retires_the_process() {
  new_test_ext().execute_with(|| {
    let members = [
      actor_ref(920, 1),
      actor_ref(921, 1),
      actor_ref(922, 1),
      actor_ref(923, 1),
    ];
    for actor in members {
      publish_test_service_member(actor, ServiceResidenceKind::Live, 1)
        .expect("service publication succeeds");
    }

    for actor in [members[2], members[0], members[3], members[1]] {
      Actors::retire_service_member(actor, CloseReason::OwnerInitiated)
        .expect("interior, cursor, pair, and singleton retirement succeeds");
      assert!(!ServiceNodes::<Test>::contains_key(actor.actor_id));
      assert_eq!(
        ActorProcesses::<Test>::get(actor.actor_id)
          .map(|process| (process.status, process.residence)),
        Some((ProcessStatus::Retired(CloseReason::OwnerInitiated), None))
      );
    }
    assert_eq!(ServiceHeader::<Test>::get(), ServiceHeaderRecord::default());

    let corrupt = actor_ref(924, 1);
    publish_test_service_member(corrupt, ServiceResidenceKind::Live, 2)
      .expect("service publication succeeds");
    ServiceHeader::<Test>::mutate(|header| header.cursor = None);
    assert_eq!(
      Actors::retire_service_member(corrupt, CloseReason::OwnerInitiated),
      Err(ServiceRetirementError::Ring(
        ServiceRingMutationError::CorruptRing
      ))
    );
    assert!(ServiceNodes::<Test>::contains_key(corrupt.actor_id));
    assert_eq!(
      ActorProcesses::<Test>::get(corrupt.actor_id),
      Some(serving_process(corrupt, ServiceResidenceKind::Live))
    );
  });
}

#[test]
fn legacy_control_adapter_derives_ready_kind_and_rejects_malformed_or_ambiguous_cells() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let (unsignaled_location, unsignaled_cell) =
      Actors::actor_control_cell(actor_id).expect("Manual Actor starts Unsignaled");
    assert_eq!(
      Actors::compile_legacy_control_process(
        7,
        Some(5),
        unsignaled_location,
        &unsignaled_cell,
        None,
      ),
      Err(ProcessCompileError::AmbiguousUnsignaled)
    );
    let park = ParkEvidence {
      plan_identity: [4; 32],
      reason: ParkNegativeReason::SourceUnavailable,
      review_at: Some(9),
    };
    assert_eq!(
      Actors::compile_legacy_control_process(
        7,
        Some(5),
        unsignaled_location,
        &unsignaled_cell,
        Some(UnsignaledProcessEvidence::Parked(park)),
      ),
      Ok(ActorProcess {
        generation: 7,
        last_attempted: Some(5),
        status: ProcessStatus::Serving,
        residence: Some(ProcessResidence::Parked(park)),
      })
    );

    assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), actor_id));
    let (ready_location, ready_cell) =
      Actors::actor_control_cell(actor_id).expect("latched Manual Actor is Ready");
    assert_eq!(
      Actors::compile_legacy_control_process(7, Some(5), ready_location, &ready_cell, None),
      Ok(ActorProcess {
        generation: 7,
        last_attempted: Some(5),
        status: ProcessStatus::Serving,
        residence: Some(ProcessResidence::Service(ServiceResidenceKind::Pending)),
      })
    );

    let mut malformed = ready_cell;
    malformed.eligible_at = None;
    assert_eq!(
      Actors::compile_legacy_control_process(7, Some(5), ready_location, &malformed, None),
      Err(ProcessCompileError::MalformedControlCell)
    );
  });
}

#[test]
fn legacy_control_mutation_inventory_covers_every_raw_storage_owner() {
  #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
  enum TransactionBoundary {
    CallerTransactional,
    FunctionTransactional,
    InPlaceAtomicWrite,
  }
  type RequiredProcessTransition = ProcessTransitionObligation;
  #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
  enum PlannerIntent {
    Publish,
    Preserve,
    Replace,
    RetireOrDisable,
    CarrierOnly,
  }
  #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
  enum AtomicPublicationSite {
    MutationOwner,
    EveryDirectCaller,
    CarrierNoProcessPublication,
  }
  #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
  enum AtomicOutcome {
    SuccessorOrRollback,
    PreservedOrRollback,
    TerminalOrRollback,
    CarrierOnlyOrRollback,
  }
  type InventoryRow = (
    &'static str,
    &'static str,
    TransactionBoundary,
    RequiredProcessTransition,
    PlannerIntent,
    AtomicPublicationSite,
    AtomicOutcome,
  );

  const INVENTORY: &[InventoryRow] = &[
    (
      "execution.rs",
      "write_run_state",
      TransactionBoundary::FunctionTransactional,
      RequiredProcessTransition::PublishTypedResidence,
      PlannerIntent::Publish,
      AtomicPublicationSite::MutationOwner,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "lib.rs",
      "insert_unsignaled_control_authority",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::PublishTypedResidence,
      PlannerIntent::Publish,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "lib.rs",
      "replace_control_admission_for_transition",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::PreserveProcess,
      PlannerIntent::Preserve,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::PreservedOrRollback,
    ),
    (
      "scheduler.rs",
      "append_waiting_entry",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::CarrierOnly,
      PlannerIntent::CarrierOnly,
      AtomicPublicationSite::CarrierNoProcessPublication,
      AtomicOutcome::CarrierOnlyOrRollback,
    ),
    (
      "scheduler.rs",
      "consume_waiting_from_supplied_authority",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::AtomicSuccessorOrRemoval,
      PlannerIntent::Replace,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "scheduler.rs",
      "control_append_ready",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::PublishTypedResidence,
      PlannerIntent::Publish,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "scheduler.rs",
      "control_append_waiting",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::PublishTypedResidence,
      PlannerIntent::Publish,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "scheduler.rs",
      "control_finalize_underfunded_at_time",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::RetireOrDisable,
      PlannerIntent::RetireOrDisable,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::TerminalOrRollback,
    ),
    (
      "scheduler.rs",
      "control_normalize_ready_head",
      TransactionBoundary::FunctionTransactional,
      RequiredProcessTransition::CarrierOnly,
      PlannerIntent::CarrierOnly,
      AtomicPublicationSite::CarrierNoProcessPublication,
      AtomicOutcome::CarrierOnlyOrRollback,
    ),
    (
      "scheduler.rs",
      "control_remove_ready_primary",
      TransactionBoundary::FunctionTransactional,
      RequiredProcessTransition::AtomicSuccessorOrRemoval,
      PlannerIntent::Replace,
      AtomicPublicationSite::MutationOwner,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "scheduler.rs",
      "demote_ready_frame_to_unsignaled",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::PublishTypedResidence,
      PlannerIntent::Publish,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "scheduler.rs",
      "detach_primary_for_successor",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::AtomicSuccessorOrRemoval,
      PlannerIntent::Replace,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "scheduler.rs",
      "paged_consume_head_at_inner",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::AtomicSuccessorOrRemoval,
      PlannerIntent::Replace,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "scheduler.rs",
      "remove_primary_control_cell_inner",
      TransactionBoundary::InPlaceAtomicWrite,
      RequiredProcessTransition::AtomicSuccessorOrRemoval,
      PlannerIntent::Replace,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "scheduler.rs",
      "remove_waiting_entry",
      TransactionBoundary::InPlaceAtomicWrite,
      RequiredProcessTransition::CarrierOnly,
      PlannerIntent::CarrierOnly,
      AtomicPublicationSite::CarrierNoProcessPublication,
      AtomicOutcome::CarrierOnlyOrRollback,
    ),
    (
      "scheduler.rs",
      "restore_unsignaled_from_authority",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::PublishTypedResidence,
      PlannerIntent::Publish,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
    (
      "scheduler.rs",
      "store_primary_control_cell",
      TransactionBoundary::InPlaceAtomicWrite,
      RequiredProcessTransition::PreserveProcess,
      PlannerIntent::Preserve,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::PreservedOrRollback,
    ),
    (
      "scheduler.rs",
      "wakeup_substrate_drain_block_inner",
      TransactionBoundary::CallerTransactional,
      RequiredProcessTransition::AtomicSuccessorOrRemoval,
      PlannerIntent::Replace,
      AtomicPublicationSite::EveryDirectCaller,
      AtomicOutcome::SuccessorOrRollback,
    ),
  ];

  fn raw_mutation_owners(source: &str) -> BTreeSet<String> {
    let mut owner = None;
    let mut owners = BTreeSet::new();
    for line in source.lines() {
      let trimmed = line.trim_start();
      if let Some(rest) = trimmed
        .strip_prefix("fn ")
        .or_else(|| trimmed.strip_prefix("pub(crate) fn "))
      {
        owner = rest.split('(').next();
      }
      let mutates_control_storage = [
        "ActorUnsignaledControlCells::<T>::insert",
        "ActorUnsignaledControlCells::<T>::remove",
        "ActorReadyFrameChunks::<T>::insert",
        "ActorReadyFrameChunks::<T>::remove",
        "ActorWaitingFrameChunks::<T>::insert",
        "ActorWaitingFrameChunks::<T>::remove",
        "ActorControlLocators::<T>::insert",
        "ActorControlLocators::<T>::remove",
      ]
      .iter()
      .any(|needle| line.contains(needle));
      if mutates_control_storage {
        owners.insert(
          owner
            .expect("raw control mutation must be inside a named function")
            .into(),
        );
      }
    }
    owners
  }

  let sources = [
    ("lib.rs", include_str!("../lib.rs")),
    ("scheduler.rs", include_str!("../scheduler.rs")),
    ("execution.rs", include_str!("../execution.rs")),
  ];
  let actual = sources
    .into_iter()
    .flat_map(|(file, source)| {
      raw_mutation_owners(source)
        .into_iter()
        .map(move |owner| (file, owner))
    })
    .collect::<BTreeSet<_>>();
  let expected = INVENTORY
    .iter()
    .map(|(file, owner, ..)| (*file, (*owner).to_owned()))
    .collect::<BTreeSet<_>>();
  assert_eq!(
    actual, expected,
    "classify every new raw control mutation owner before process cutover"
  );
  assert_eq!(
    INVENTORY
      .iter()
      .map(|(_, _, boundary, ..)| *boundary)
      .collect::<BTreeSet<_>>(),
    BTreeSet::from([
      TransactionBoundary::CallerTransactional,
      TransactionBoundary::FunctionTransactional,
      TransactionBoundary::InPlaceAtomicWrite,
    ])
  );
  assert_eq!(
    INVENTORY
      .iter()
      .map(|(_, _, _, transition, ..)| *transition)
      .collect::<BTreeSet<_>>(),
    BTreeSet::from([
      RequiredProcessTransition::PublishTypedResidence,
      RequiredProcessTransition::PreserveProcess,
      RequiredProcessTransition::AtomicSuccessorOrRemoval,
      RequiredProcessTransition::RetireOrDisable,
      RequiredProcessTransition::CarrierOnly,
    ])
  );

  fn direct_callers(source: &str, callee: &str) -> BTreeSet<String> {
    let mut owner = None;
    let mut callers = BTreeSet::new();
    for line in source.lines() {
      let trimmed = line.trim_start();
      if let Some(rest) = trimmed
        .strip_prefix("fn ")
        .or_else(|| trimmed.strip_prefix("pub(crate) fn "))
      {
        owner = rest.split('(').next();
      }
      if line.contains(&format!("Self::{callee}("))
        && let Some(caller) = owner
        && caller != callee
      {
        callers.insert(caller.to_owned());
      }
    }
    callers
  }

  for (file, owner, boundary, obligation, intent, site, outcome) in INVENTORY {
    let expected_intent = match obligation {
      RequiredProcessTransition::PublishTypedResidence => PlannerIntent::Publish,
      RequiredProcessTransition::PreserveProcess => PlannerIntent::Preserve,
      RequiredProcessTransition::AtomicSuccessorOrRemoval => PlannerIntent::Replace,
      RequiredProcessTransition::RetireOrDisable => PlannerIntent::RetireOrDisable,
      RequiredProcessTransition::CarrierOnly => PlannerIntent::CarrierOnly,
    };
    assert_eq!(
      *intent, expected_intent,
      "planner intent drift for {file}::{owner}"
    );
    let expected_outcome = match obligation {
      RequiredProcessTransition::PublishTypedResidence
      | RequiredProcessTransition::AtomicSuccessorOrRemoval => AtomicOutcome::SuccessorOrRollback,
      RequiredProcessTransition::PreserveProcess => AtomicOutcome::PreservedOrRollback,
      RequiredProcessTransition::RetireOrDisable => AtomicOutcome::TerminalOrRollback,
      RequiredProcessTransition::CarrierOnly => AtomicOutcome::CarrierOnlyOrRollback,
    };
    assert_eq!(
      *outcome, expected_outcome,
      "atomic outcome drift for {file}::{owner}"
    );

    match site {
      AtomicPublicationSite::MutationOwner => {
        assert_eq!(*boundary, TransactionBoundary::FunctionTransactional);
        let source = sources
          .iter()
          .find_map(|(candidate, source)| (*candidate == *file).then_some(*source))
          .expect("inventory source exists");
        assert!(
          source.contains(&format!("fn {owner}(")) && source.contains("storage::with_transaction"),
          "mutation owner must retain its explicit transaction: {file}::{owner}"
        );
      }
      AtomicPublicationSite::EveryDirectCaller => {
        let callers = sources
          .iter()
          .flat_map(|(_, source)| direct_callers(source, owner))
          .collect::<BTreeSet<_>>();
        assert!(
          !callers.is_empty(),
          "caller-transactional owner must have a source-backed publication cohort: {file}::{owner}"
        );
      }
      AtomicPublicationSite::CarrierNoProcessPublication => {
        assert_eq!(*intent, PlannerIntent::CarrierOnly);
        assert_eq!(*outcome, AtomicOutcome::CarrierOnlyOrRollback);
      }
    }
  }
}

#[test]
fn canonical_service_cutover_waits_for_a_nonplacement_semantic_authority_owner() {
  let lib = include_str!("../lib.rs");
  let scheduler = include_str!("../scheduler.rs");
  let execution = include_str!("../execution.rs");
  let benchmarks = include_str!("../benchmarking.rs");
  let pallet_weights = include_str!("../weights.rs");
  let runtime_weights = include_str!("../../../../runtime/src/weights/pallet_deos_actors.rs");

  assert!(lib.contains("Self::insert_unsignaled_control_authority(actor_id, identity, hot"));
  assert!(lib.contains("ActorIdentities::<T>::remove(actor_id);"));
  assert!(lib.contains("Self::prime_initial_actor_schedule(actor_id)"));
  assert!(lib.contains("ActorSemanticStates::<T>::get(actor_id)"));

  // Lifecycle, service, observation and execution callers now enter through one storage-neutral
  // semantic loader. Only that loader compiles the placement-backed owner; production semantic
  // storage remains absent until the writer and Weight cutover can land atomically.
  for owner in [
    "load_actor_state_with_admission",
    "load_control_authority_with_authority",
    "load_actor_service_state_with_authority",
  ] {
    let marker = format!("    pub(crate) fn {owner}(");
    let start = lib
      .find(&marker)
      .unwrap_or_else(|| panic!("semantic loader caller disappeared: {owner}"));
    let body = &lib[start..];
    let end = body[marker.len()..]
      .find("\n    pub(crate) fn ")
      .map_or(body.len(), |offset| marker.len() + offset);
    assert!(
      body[..end].contains("Self::load_actor_semantic_state(actor_id)"),
      "semantic loader caller bypassed the storage-neutral boundary: {owner}"
    );
  }
  assert!(lib.contains("fn load_actor_semantic_state("));
  assert!(lib.contains("Self::load_frame_control_authority(actor_id)"));
  assert!(!scheduler.contains("Self::load_frame_actor_state(actor_id)"));
  assert!(execution.contains("Self::load_actor_state_for_frame_control(actor_id)"));

  // Initial semantic publication has one shared owner. Create and activate supply scalar identity,
  // freshly initialized hot state, and complete contract geometry to insert_active_actor; that
  // owner derives admission and the zero-Step/Step-0 resource envelope before publishing the
  // Unsignaled cell. The future cutover therefore needs no caller-specific record constructor.
  for dependency in [
    "Self::build_admission_certificate(&contract)",
    "Self::derive_step_resource_envelopes(&contract)",
    "T::WeightInfo::scheduler_inner_zero_step_complete()",
    "Self::insert_unsignaled_control_authority(actor_id, identity, hot, admission, resources,)",
    "Self::store_actor_contract(actor_id, contract)",
  ] {
    assert!(
      lib.contains("fn insert_active_actor(") && lib.contains(dependency),
      "initial semantic construction input drift: {dependency}"
    );
  }
  for caller in ["do_create_actor", "do_activate_actor"] {
    let marker = format!("    fn {caller}(");
    let start = lib
      .find(&marker)
      .unwrap_or_else(|| panic!("initial publication caller disappeared: {caller}"));
    let body = &lib[start..];
    let end = body[marker.len()..]
      .find("\n    fn ")
      .map_or(body.len(), |offset| marker.len() + offset);
    assert!(
      body[..end].contains("Self::insert_active_actor("),
      "supported initial publication must retain the shared constructor: {caller}"
    );
  }

  // Every projected field has one post-cutover source: semantic record for identity/hot/admission,
  // run state for cursor/eligibility, and contract geometry for current-Step resources. Placement
  // may retain only residence and reverse-handle evidence after this complete loader conversion.
  for dependency in [
    "ActorRunStateStore::<T>::get(actor_id)",
    "Self::load_current_step_from_geometry(",
    "ActorContractHeads::<T>::get(actor_id)?",
  ] {
    assert!(
      lib.contains("fn load_actor_service_state_with_head(") && lib.contains(dependency),
      "semantic execution projection input drift: {dependency}"
    );
  }

  // The complete post-cutover operation map is deliberately smaller than the caller set: shared
  // construction publishes one record; every semantic writer performs whole-record Replace;
  // deactivate/finalize remove the exact record; and residence-only movement performs no semantic
  // mutation. Production storage remains blocked until all mapped callers and composed weights
  // convert together.
  let semantic_operation_map = [
    ("insert_active_actor", "Publish"),
    ("store_frame_control_authority", "Replace"),
    ("replace_control_admission_for_transition", "Replace"),
    ("prepare_observation_ready_cell", "Replace"),
    ("update_existing_frame_control_identity", "Replace"),
    ("update_existing_frame_control_hot", "Replace"),
    ("consume_waiting_from_supplied_authority", "Replace"),
    ("write_run_state", "Replace"),
    ("do_deactivate_actor", "Replace"),
    ("finalize_actor_loaded_inner", "Remove"),
  ];
  assert_eq!(
    semantic_operation_map
      .iter()
      .filter(|(_, operation)| *operation == "Publish")
      .count(),
    1
  );
  assert_eq!(
    semantic_operation_map
      .iter()
      .filter(|(_, operation)| *operation == "Remove")
      .count(),
    1
  );
  for (owner, _) in semantic_operation_map {
    assert!(
      lib.contains(&format!("fn {owner}("))
        || scheduler.contains(&format!("fn {owner}("))
        || execution.contains(&format!("fn {owner}(")),
      "semantic operation owner disappeared: {owner}"
    );
  }
  for placement_only_owner in [
    "append_waiting_entry",
    "control_normalize_ready_head",
    "control_remove_ready_primary",
    "remove_waiting_entry",
  ] {
    assert!(
      scheduler.contains(&format!("fn {placement_only_owner}(")),
      "placement-only semantic no-op owner disappeared: {placement_only_owner}"
    );
    assert!(
      !semantic_operation_map
        .iter()
        .any(|(owner, _)| *owner == placement_only_owner),
      "placement-only owner must not acquire semantic write authority: {placement_only_owner}"
    );
  }

  // These production owners still rewrite semantic fields inside the legacy placement cell after
  // publication. Cutover must convert this complete writer closure atomically or it creates dual
  // semantic truth.
  for (source, owner, mutation) in [
    (
      lib,
      "store_frame_control_authority",
      "cell.identity = control_identity;",
    ),
    (
      lib,
      "store_frame_control_authority",
      "cell.hot = Self::control_hot_from_scalar(hot.clone());",
    ),
    (
      lib,
      "replace_control_admission_for_transition",
      "cell.admission = certificate.clone();",
    ),
    (
      lib,
      "replace_control_admission_for_transition",
      "cell.resources = resources;",
    ),
    (
      scheduler,
      "prepare_observation_ready_cell",
      "cell.cursor = state.run_head.as_ref().map_or(0, |run| run.cursor);",
    ),
    (
      scheduler,
      "update_existing_frame_control_identity",
      "cell.identity = Self::control_identity_from_scalar(identity.clone())",
    ),
    (
      scheduler,
      "update_existing_frame_control_hot",
      "cell.hot = Self::control_hot_from_scalar(hot.clone());",
    ),
    (
      scheduler,
      "consume_waiting_from_supplied_authority",
      "cell.cursor = 0;",
    ),
    (
      execution,
      "write_run_state",
      "cell.cursor = state.as_ref().map_or(0, |run| run.cursor);",
    ),
    (
      execution,
      "write_run_state",
      "cell.resources = step.resources;",
    ),
  ] {
    assert!(
      source.contains(&format!("fn {owner}(")) && source.contains(mutation),
      "semantic-owner cutover inventory drift for {owner}: {mutation}"
    );
  }
  // Both supported initial publication callers already provide the required outer rollback
  // boundary. The blocker is therefore the complete loader/writer conversion and composed weight,
  // not a missing transaction around create or activate.
  for (owner, transaction) in [
    (
      "do_create_actor",
      "polkadot_sdk::frame_support::storage::with_transaction(||",
    ),
    (
      "do_activate_actor",
      "polkadot_sdk::frame_support::storage::with_transaction(||",
    ),
  ] {
    let marker = format!("    fn {owner}(");
    let start = lib
      .find(&marker)
      .unwrap_or_else(|| panic!("initial publication owner disappeared: {owner}"));
    let body = &lib[start..];
    let end = body[marker.len()..]
      .find("\n    fn ")
      .map_or(body.len(), |offset| marker.len() + offset);
    assert!(
      body[..end].contains(transaction),
      "initial canonical publication needs an outer rollback boundary in {owner}"
    );
  }

  // Exact incremental storage composition at the cutover boundary. Publish, Replace and Remove
  // each require one semantic-record read plus one write. Initial Live publication must additionally
  // compose the generated populated-ring owner, whose benchmark already covers process publication,
  // legacy-absence checks, ring/header reads and process/node/header writes (10 reads, 6 writes).
  // The worst supported create/activate path therefore gains 11 reads and 7 writes before any
  // measured execution/proof contribution from the semantic record itself.
  let semantic_mutation_io = [("Publish", 1u64, 1u64), ("Replace", 1, 1), ("Remove", 1, 1)];
  assert_eq!(
    semantic_mutation_io
      .iter()
      .find(|(operation, _, _)| *operation == "Publish")
      .map(|(_, reads, writes)| (*reads + 10, *writes + 6)),
    Some((11, 7))
  );
  assert!(benchmarks.contains("fn service_member_publish_populated()"));
  assert!(pallet_weights.contains(
    "fn service_member_publish_populated() -> Weight {\n    Weight::from_parts(200_000_000, 32_000).saturating_add(T::DbWeight::get().reads_writes(10, 6))"
  ));

  // Existing create/activate/deactivate benchmarks and generated runtime bindings cannot account
  // for that composition: no production semantic map exists, those lifecycle benchmarks do not
  // execute service publication, and generated storage evidence cannot name the semantic owner.
  // Production publication is blocked until the lifecycle benchmarks exercise the composed path
  // and regenerate both pallet/runtime Weight bindings in the same atomic authority cutover.
  assert!(!lib.contains("pub type ActorSemanticRecords<T:"));
  let lifecycle_branch_matrix = [
    (
      "create_user_actor",
      "active creation with populated Contract geometry and an initially\n  // non-Live current-state detector residence",
      "must Publish semantic state",
    ),
    (
      "create_system_actor",
      "System active creation shares the populated, initially non-Live",
      "Publish active\n  // semantic state; no service publication",
    ),
    (
      "create_dormant_system_actor",
      "dormant creation uses Publish identity-only semantic state and no",
      "Publish identity-only semantic state",
    ),
    (
      "activate_actor",
      "activation must Replace dormant with active semantic state",
      "Replace dormant with active semantic state",
    ),
    (
      "deactivate_actor",
      "deactivation must Replace active with identity-only dormant semantic",
      "Replace active with identity-only dormant semantic",
    ),
  ];
  for (benchmark, branch_marker, cutover_operation) in lifecycle_branch_matrix {
    assert!(benchmarks.contains(&format!("fn {benchmark}()")));
    assert!(benchmarks.contains(branch_marker));
    assert!(benchmarks.contains(cutover_operation));
  }
  assert!(benchmarks.contains("terminal finalization instead removes the record"));
  assert!(benchmarks.contains("fn service_member_publish_populated()"));
  assert!(benchmarks.contains("fn scheduler_inner_zero_step_complete()"));

  // Dispatch Weight ownership follows the semantic branch rather than the public-call aliases.
  // User at-slot and System sovereign-id creation already have distinct generated owners; active
  // versus dormant System creation selects its owner from the optional Contract. Every path that
  // may terminally finalize composes the shared close upper bound, including scheduler-side close.
  for binding in [
    "T::WeightInfo::create_user_actor().max(T::WeightInfo::create_user_actor_crossing_new_page())",
    "T::WeightInfo::create_user_actor_at_slot().max(T::WeightInfo::create_user_actor_crossing_new_page())",
    "T::WeightInfo::create_system_actor()\n        .max(T::WeightInfo::create_user_actor_crossing_new_page())",
    "T::WeightInfo::create_dormant_system_actor()",
    "T::WeightInfo::create_system_actor_at_sovereign_id().max(T::WeightInfo::create_user_actor_crossing_new_page())",
    "T::WeightInfo::activate_actor()",
    "T::WeightInfo::deactivate_actor()",
    "Pallet::<T>::close_dispatch_weight_upper()",
  ] {
    assert!(
      lib.contains(binding),
      "lifecycle Weight owner drift: {binding}"
    );
  }
  for generated_owner in [
    "create_user_actor",
    "create_user_actor_at_slot",
    "create_system_actor",
    "create_system_actor_at_sovereign_id",
    "create_dormant_system_actor",
    "activate_actor",
    "deactivate_actor",
    "close_actor",
  ] {
    assert!(
      pallet_weights.contains(&format!("fn {generated_owner}() -> Weight")),
      "pallet Weight binding disappeared: {generated_owner}"
    );
    assert!(
      runtime_weights.contains(&format!("fn {generated_owner}() -> Weight")),
      "runtime Weight binding disappeared: {generated_owner}"
    );
  }
  for terminal_dispatch_owner in [
    "pause_actor",
    "resume_actor",
    "manual_trigger",
    "close_actor",
    "update_contract",
    "permissionless_sweep",
    "permissionless_sweep_many",
    "cancel_run",
  ] {
    let marker = format!("    pub fn {terminal_dispatch_owner}(");
    let start = lib
      .find(&marker)
      .unwrap_or_else(|| panic!("terminal dispatch owner disappeared: {terminal_dispatch_owner}"));
    let prefix = &lib[start.saturating_sub(320)..start];
    assert!(
      prefix.contains("close_dispatch_weight_upper()"),
      "terminal dispatch owner must price shared finalization: {terminal_dispatch_owner}"
    );
  }
  assert!(scheduler.contains("pub fn close_cleanup_weight_upper() -> Weight"));
  assert!(scheduler.contains("T::WeightInfo::close_actor()"));

  // A zero-Step active Contract still publishes complete active semantic state, but has no current
  // Step resource load. Initial Live publication, when selected by the new residence policy, must
  // compose the populated service owner; initially Sleeping/Parked/Unsignaled branches must not.
  // The existing zero-Step scheduler benchmark is executable evidence for the no-Step branch, while
  // the lifecycle comments above are an explicit guard against collapsing Dormant into zero-Step.
  assert!(benchmarks.contains("fn scheduler_inner_zero_step_complete()"));
  assert!(benchmarks.contains("This remains distinct from an active zero-Step Contract"));
  for generated in [pallet_weights, runtime_weights] {
    assert!(!generated.contains("Actors::ActorSemanticRecords"));
  }

  for source in [lib, scheduler, execution] {
    assert_eq!(
      source.matches("Self::publish_service_member(").count(),
      0,
      "production must not publish canonical service authority while the legacy control cell is the sole identity/hot/admission owner"
    );
  }
  assert_eq!(
    scheduler.matches("Self::retire_service_member(").count(),
    2,
    "only staged canonical zero-Step and effectful completion transactions may retire their consumed Service carriers"
  );
  for source in [lib, execution] {
    assert_eq!(
      source.matches("Self::retire_service_member(").count(),
      0,
      "other production paths must not retire canonical Service authority before publication cuts over"
    );
  }
}

#[test]
fn process_status_separates_park_from_revocation_and_retirement() {
  let parked: ActorProcess<u32> = ActorProcess {
    generation: 11,
    last_attempted: Some(7),
    status: ProcessStatus::Serving,
    residence: Some(ProcessResidence::Parked(ParkEvidence {
      plan_identity: [3; 32],
      reason: ParkNegativeReason::PredicateFalse,
      review_at: Some(9u32),
    })),
  };
  let disabled: ActorProcess<u32> = ActorProcess {
    generation: 11,
    last_attempted: Some(7),
    status: ProcessStatus::Disabled(ProcessDisablement {
      cause: ProcessDisableCause::OwnerPaused,
      revival_authority: ProcessRevivalAuthority::Owner,
      basis: SuspendedProcessBasis::Running { eligible_at: 9u32 },
    }),
    residence: None,
  };
  let retired: ActorProcess<u32> = ActorProcess {
    generation: 11,
    last_attempted: Some(7),
    status: ProcessStatus::Retired(CloseReason::OwnerInitiated),
    residence: None,
  };

  assert!(matches!(
    parked,
    ActorProcess {
      status: ProcessStatus::Serving,
      residence: Some(ProcessResidence::Parked(_)),
      ..
    }
  ));
  assert!(matches!(
    disabled,
    ActorProcess {
      status: ProcessStatus::Disabled(_),
      residence: None,
      ..
    }
  ));
  assert!(matches!(
    retired,
    ActorProcess {
      status: ProcessStatus::Retired(_),
      residence: None,
      ..
    }
  ));
}

#[derive(Default)]
struct ServiceRingOracle {
  header: ServiceHeaderRecord<u32>,
  nodes: BTreeMap<u64, ServiceNode<u32>>,
}

impl ServiceRingOracle {
  fn insert_tail(&mut self, actor: ActorRef) {
    assert!(!self.nodes.contains_key(&actor.actor_id));
    let cursor = self.header.cursor.unwrap_or(actor);
    let tail = self
      .nodes
      .get(&cursor.actor_id)
      .map_or(actor, |node| node.previous);
    let node = ServiceNode {
      generation: actor.generation,
      previous: tail,
      next: cursor,
      kind: ServiceResidenceKind::Live,
      eligible_from: 1,
      last_considered: 0,
    };
    if let Some(head) = self.nodes.get_mut(&cursor.actor_id) {
      head.previous = actor;
      self
        .nodes
        .get_mut(&tail.actor_id)
        .expect("tail exists")
        .next = actor;
    } else {
      self.header.cursor = Some(actor);
    }
    self.nodes.insert(actor.actor_id, node);
    self.header.count += 1;
    self.assert_valid();
  }

  fn remove(&mut self, actor: ActorRef) -> bool {
    let Some(node) = self.nodes.get(&actor.actor_id).copied() else {
      return false;
    };
    if node.generation != actor.generation {
      return false;
    }
    if self.header.count == 1 {
      self.header.cursor = None;
    } else {
      self
        .nodes
        .get_mut(&node.previous.actor_id)
        .expect("previous exists")
        .next = node.next;
      self
        .nodes
        .get_mut(&node.next.actor_id)
        .expect("next exists")
        .previous = node.previous;
      if self.header.cursor == Some(actor) {
        self.header.cursor = Some(node.next);
      }
    }
    self.nodes.remove(&actor.actor_id);
    self.header.count -= 1;
    self.assert_valid();
    true
  }

  fn assert_valid(&self) {
    assert_eq!(self.header.count as usize, self.nodes.len());
    let Some(start) = self.header.cursor else {
      assert!(self.nodes.is_empty());
      return;
    };
    let mut current = start;
    for _ in 0..self.header.count {
      let node = self.nodes.get(&current.actor_id).expect("member exists");
      assert_eq!(node.generation, current.generation);
      let next = self.nodes.get(&node.next.actor_id).expect("next exists");
      let previous = self
        .nodes
        .get(&node.previous.actor_id)
        .expect("previous exists");
      assert_eq!(next.previous, current);
      assert_eq!(previous.next, current);
      current = node.next;
    }
    assert_eq!(current, start);
  }
}

fn actor_ref(actor_id: u64, generation: u64) -> ActorRef {
  ActorRef {
    actor_id,
    generation,
  }
}

#[test]
fn service_ring_oracle_covers_empty_singleton_interior_cursor_and_wrap() {
  let mut ring = ServiceRingOracle::default();
  ring.assert_valid();
  for id in 1..=4 {
    ring.insert_tail(actor_ref(id, 1));
  }
  assert_eq!(ring.nodes[&4].next, actor_ref(1, 1));
  assert!(ring.remove(actor_ref(3, 1)));
  assert!(ring.remove(actor_ref(1, 1)));
  assert_eq!(ring.header.cursor, Some(actor_ref(2, 1)));
  assert!(ring.remove(actor_ref(4, 1)));
  assert!(ring.remove(actor_ref(2, 1)));
  assert_eq!(ring.header, ServiceHeaderRecord::default());
}

#[test]
fn service_ring_oracle_rejects_stale_generation_without_mutation() {
  let mut ring = ServiceRingOracle::default();
  ring.insert_tail(actor_ref(7, 3));
  assert!(!ring.remove(actor_ref(7, 2)));
  assert_eq!(ring.header.count, 1);
  assert_eq!(ring.header.cursor, Some(actor_ref(7, 3)));
}

fn serving_process(actor: ActorRef, kind: ServiceResidenceKind) -> ActorProcess<u64> {
  ActorProcess {
    generation: actor.generation,
    last_attempted: None,
    status: ProcessStatus::Serving,
    residence: Some(ProcessResidence::Service(kind)),
  }
}

#[test]
fn canonical_service_ring_is_transactional_generation_bound_and_structurally_complete() {
  new_test_ext().execute_with(|| {
    let first = actor_ref(100, 4);
    ActorProcesses::<Test>::insert(
      first.actor_id,
      serving_process(first, ServiceResidenceKind::Live),
    );
    assert_eq!(
      Actors::insert_service_member(first, ServiceResidenceKind::Live, 1),
      Err(ServiceRingMutationError::TransactionRequired)
    );

    let legacy_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let legacy_rejected = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::insert_service_member(actor_ref(legacy_id, 1), ServiceResidenceKind::Live, 1),
      )
    });
    assert_eq!(
      legacy_rejected,
      Err(ServiceRingMutationError::LegacyAuthorityPresent)
    );

    let members = [
      first,
      actor_ref(101, 5),
      actor_ref(102, 6),
      actor_ref(103, 7),
    ];
    for actor in members {
      ActorProcesses::<Test>::insert(
        actor.actor_id,
        serving_process(actor, ServiceResidenceKind::Live),
      );
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::insert_service_member(actor, ServiceResidenceKind::Live, 1),
        )
      })
      .expect("canonical insertion succeeds");
    }
    assert_eq!(ServiceHeader::<Test>::get().cursor, Some(first));
    assert_eq!(ServiceHeader::<Test>::get().count, 4);
    assert_eq!(ServiceNodes::<Test>::get(103).expect("tail").next, first);

    let stale = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::remove_service_member(actor_ref(102, 5)),
      )
    });
    assert_eq!(stale, Err(ServiceRingMutationError::StaleGeneration));
    assert_eq!(ServiceHeader::<Test>::get().count, 4);

    for actor in [members[2], members[0], members[3], members[1]] {
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::remove_service_member(actor),
        )
      })
      .expect("interior, cursor, pair, and singleton removal succeed");
    }
    assert_eq!(ServiceHeader::<Test>::get(), ServiceHeaderRecord::default());
    assert!(
      members
        .iter()
        .all(|actor| !ServiceNodes::<Test>::contains_key(actor.actor_id))
    );

    let rolled_back = actor_ref(110, 8);
    ActorProcesses::<Test>::insert(
      rolled_back.actor_id,
      serving_process(rolled_back, ServiceResidenceKind::Pending),
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::insert_service_member(rolled_back, ServiceResidenceKind::Pending, 2)
        .expect("staged insertion is visible");
      assert!(ServiceNodes::<Test>::contains_key(rolled_back.actor_id));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(ServiceHeader::<Test>::get(), ServiceHeaderRecord::default());
    assert!(!ServiceNodes::<Test>::contains_key(rolled_back.actor_id));
  });
}

#[test]
fn observation_dependency_sources_are_bijective_transactional_and_nonwrapping() {
  new_test_ext().execute_with(|| {
    assert_eq!(
      Actors::resolve_observation_dependency_source(41),
      Err(DependencySourceError::TransactionRequired)
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::resolve_observation_dependency_source(41),
        Ok(DependencySourceMutation::Allocated(0))
      );
      assert_eq!(
        Actors::resolve_observation_dependency_source(41),
        Ok(DependencySourceMutation::Existing(0))
      );
      assert_eq!(
        Actors::resolve_observation_dependency_source(42),
        Ok(DependencySourceMutation::Allocated(1))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(ObservationDependencySources::<Test>::get(41), Some(0));
    assert_eq!(DependencySourceObservations::<Test>::get(0), Some(41));

    let rolled_back = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      let outcome = Actors::resolve_observation_dependency_source(43);
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(outcome)
    });
    assert_eq!(rolled_back, Ok(DependencySourceMutation::Allocated(2)));
    assert_eq!(ObservationDependencySources::<Test>::get(43), None);
    assert_eq!(DependencySourceAllocatorState::<Test>::get().next, 2);

    DependencySourceObservations::<Test>::insert(2, 98);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::resolve_observation_dependency_source(43),
        Err(DependencySourceError::SourceOccupied)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    DependencySourceObservations::<Test>::remove(2);

    DependencySourceObservations::<Test>::remove(0);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::resolve_observation_dependency_source(41),
        Err(DependencySourceError::ReverseMissing)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    DependencySourceObservations::<Test>::insert(0, 99);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::resolve_observation_dependency_source(41),
        Err(DependencySourceError::ReverseMismatch)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    DependencySourceAllocatorState::<Test>::put(DependencySourceAllocator {
      next: u64::MAX,
      exhausted: false,
    });
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::resolve_observation_dependency_source(44),
        Ok(DependencySourceMutation::Allocated(u64::MAX))
      );
      assert!(DependencySourceAllocatorState::<Test>::get().exhausted);
      assert_eq!(
        Actors::resolve_observation_dependency_source(44),
        Ok(DependencySourceMutation::Existing(u64::MAX))
      );
      assert_eq!(
        Actors::resolve_observation_dependency_source(45),
        Err(DependencySourceError::Exhausted)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
  });
}

#[test]
fn observation_dependency_event_ingress_allocates_reuses_publishes_and_rolls_back() {
  new_test_ext().execute_with(|| {
    assert_eq!(
      <Actors as crate::DependencyEventIngress<u32>>::note_dependency_event(51),
      Err(DispatchError::Other(
        "dependency event requires transaction"
      ))
    );

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_ok!(<Actors as crate::DependencyEventIngress<u32>>::note_dependency_event(51));
      assert_ok!(<Actors as crate::DependencyEventIngress<u32>>::note_dependency_event(51));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(ObservationDependencySources::<Test>::get(51), Some(0));
    assert_eq!(DependencySourceObservations::<Test>::get(0), Some(51));
    assert_eq!(DependencyRevisions::<Test>::get(0).revision, 2);

    let rolled_back = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_ok!(<Actors as crate::DependencyEventIngress<u32>>::note_dependency_event(52));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(rolled_back, ());
    assert_eq!(ObservationDependencySources::<Test>::get(52), None);
    assert_eq!(DependencySourceAllocatorState::<Test>::get().next, 1);

    DependencyRevisions::<Test>::insert(
      1,
      DependencyRevisionState {
        exhausted: true,
        ..Default::default()
      },
    );
    let failed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      let outcome = <Actors as crate::DependencyEventIngress<u32>>::note_dependency_event(52);
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(outcome)
    });
    assert_eq!(
      failed,
      Err(DispatchError::Other("dependency revision exhausted"))
    );
    assert_eq!(ObservationDependencySources::<Test>::get(52), None);
    assert_eq!(DependencySourceAllocatorState::<Test>::get().next, 1);

    DependencySourceAllocatorState::<Test>::put(DependencySourceAllocator {
      next: u64::MAX,
      exhausted: true,
    });
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        <Actors as crate::DependencyEventIngress<u32>>::note_dependency_event(53),
        Err(DispatchError::Other("dependency source identity exhausted"))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(ObservationDependencySources::<Test>::get(53), None);
  });
}

#[test]
fn dependency_revision_is_nonwrapping_sticky_and_transactional() {
  new_test_ext().execute_with(|| {
    let source = 17;
    assert_eq!(
      Actors::revise_dependency_source(source),
      Err(DependencyRevisionError::TransactionRequired)
    );
    let first = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::revise_dependency_source(source),
      )
    });
    assert_eq!(first, Ok(DependencyRevisionMutation::Advanced(1)));

    let rolled_back = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      let outcome = Actors::revise_dependency_source(source);
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(outcome)
    });
    assert_eq!(rolled_back, Ok(DependencyRevisionMutation::Advanced(2)));
    assert_eq!(DependencyRevisions::<Test>::get(source).revision, 1);

    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        revision: u64::MAX,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    for _ in 0..2 {
      let outcome = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::revise_dependency_source(source),
        )
      });
      assert_eq!(outcome, Ok(DependencyRevisionMutation::Exhausted));
      assert_eq!(
        DependencyRevisions::<Test>::get(source),
        DependencyRevisionState {
          revision: u64::MAX,
          scan_target: None,
          scan_cursor: 0,
          scan_end: 0,
          exhausted: true,
        }
      );
    }
  });
}

#[test]
fn dependency_event_publication_coalesces_behind_one_fixed_scan() {
  new_test_ext().execute_with(|| {
    let source = 18;
    assert_eq!(
      Actors::publish_dependency_event(source),
      Err(DependencyRevisionError::TransactionRequired)
    );

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::publish_dependency_event(source),
        Ok(DependencyPublicationMutation::Begun(1))
      );
      assert_eq!(
        DependencyRevisions::<Test>::get(source),
        DependencyRevisionState {
          revision: 1,
          scan_target: Some(1),
          scan_cursor: 0,
          scan_end: 0,
          exhausted: false,
        }
      );
      assert_eq!(
        Actors::publish_dependency_event(source),
        Ok(DependencyPublicationMutation::Coalesced {
          revision: 2,
          active_target: 1,
        })
      );
      assert_eq!(
        Actors::complete_dependency_scan(source, 1, 0),
        Ok(DependencyScanMutation::HandedOff(2))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    let owner = PendingCheckOwner {
      actor: actor_ref(120, 4),
      plan_revision: 6,
    };
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::install_dependency_registration(source, owner, 2),
        Ok(DependencyRegistrationMutation::Installed)
      );
      assert_eq!(
        Actors::publish_dependency_event(source),
        Ok(DependencyPublicationMutation::Coalesced {
          revision: 3,
          active_target: 2,
        })
      );
      let fixed = DependencyRevisions::<Test>::get(source);
      assert_eq!(fixed.scan_end, 0);
      assert_eq!(
        Actors::complete_dependency_scan(source, 2, 0),
        Ok(DependencyScanMutation::HandedOff(3))
      );
      assert_eq!(DependencyRevisions::<Test>::get(source).scan_end, 1);
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    let retained = DependencyRevisions::<Test>::get(source);
    let rolled_back = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      let outcome = Actors::publish_dependency_event(source);
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(outcome)
    });
    assert_eq!(
      rolled_back,
      Ok(DependencyPublicationMutation::Coalesced {
        revision: 4,
        active_target: 3,
      })
    );
    assert_eq!(DependencyRevisions::<Test>::get(source), retained);

    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        revision: u64::MAX,
        scan_target: Some(9),
        scan_cursor: 4,
        scan_end: 7,
        exhausted: false,
      },
    );
    for _ in 0..2 {
      let outcome = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::publish_dependency_event(source),
        )
      });
      assert_eq!(outcome, Ok(DependencyPublicationMutation::Exhausted));
      assert_eq!(
        DependencyRevisions::<Test>::get(source),
        DependencyRevisionState {
          revision: u64::MAX,
          scan_target: Some(9),
          scan_cursor: 4,
          scan_end: 7,
          exhausted: true,
        }
      );
    }
  });
}

#[test]
fn dependency_publication_retains_empty_scan_source_across_coalescing_and_rollback() {
  new_test_ext().execute_with(|| {
    let source = 18;
    assert_eq!(
      Actors::publish_dependency_event_with_source_retention(source),
      Err(DependencyPublicationError::Revision(
        DependencyRevisionError::TransactionRequired
      ))
    );

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::publish_dependency_event_with_source_retention(source),
        Ok(DependencyPublicationMutation::Begun(1))
      );
      assert_eq!(
        Actors::publish_dependency_event_with_source_retention(source),
        Ok(DependencyPublicationMutation::Coalesced {
          revision: 2,
          active_target: 1,
        })
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(DependencyRevisions::<Test>::get(source).scan_end, 0);
    assert_eq!(DependencyScanSourceListState::<Test>::get().count, 1);

    let before_revision = DependencyRevisions::<Test>::get(19);
    let before_list = DependencyScanSourceListState::<Test>::get();
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      let outcome = Actors::publish_dependency_event_with_source_retention(19);
      assert_eq!(outcome, Ok(DependencyPublicationMutation::Begun(1)));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(DependencyRevisions::<Test>::get(19), before_revision);
    assert_eq!(DependencyScanSourceListState::<Test>::get(), before_list);
    assert!(!DependencyScanSourceNodes::<Test>::contains_key(19));
  });
}

#[test]
fn dependency_scan_source_carrier_preserves_exact_fair_membership_and_rollback() {
  new_test_ext().execute_with(|| {
    assert_eq!(
      Actors::insert_dependency_scan_source(18),
      Err(DependencyScanSourceError::TransactionRequired)
    );

    for source in [18, 19, 20] {
      DependencyRevisions::<Test>::mutate(source, |state| state.scan_target = Some(1));
    }
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      for source in [18, 19, 20] {
        assert_eq!(
          Actors::insert_dependency_scan_source(source),
          Ok(DependencyScanSourceMutation::Inserted)
        );
      }
      assert_eq!(
        Actors::insert_dependency_scan_source(19),
        Ok(DependencyScanSourceMutation::AlreadyActive)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    assert_eq!(
      DependencyScanSourceListState::<Test>::get(),
      DependencyScanSourceList {
        cursor: Some(18),
        count: 3,
      }
    );
    assert_eq!(
      DependencyScanSourceNodes::<Test>::get(18),
      Some(DependencyScanSourceNode {
        previous: 20,
        next: 19,
      })
    );

    let before = DependencyScanSourceListState::<Test>::get();
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      DependencyRevisions::<Test>::mutate(18, |state| state.scan_target = None);
      assert_eq!(
        Actors::remove_dependency_scan_source(18),
        Ok(DependencyScanSourceMutation::Removed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(DependencyScanSourceListState::<Test>::get(), before);
    assert!(DependencyScanSourceNodes::<Test>::contains_key(18));

    for source in [18, 20, 19] {
      DependencyRevisions::<Test>::mutate(source, |state| state.scan_target = None);
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        assert_eq!(
          Actors::remove_dependency_scan_source(source),
          Ok(DependencyScanSourceMutation::Removed)
        );
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
    }
    assert_eq!(
      DependencyScanSourceListState::<Test>::get(),
      DependencyScanSourceList::default()
    );
    assert_eq!(DependencyScanSourceNodes::<Test>::iter().count(), 0);

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::insert_dependency_scan_source(21),
        Err(DependencyScanSourceError::ScanInactive)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
  });
}

#[test]
fn dependency_scan_keeps_fixed_target_and_requires_exact_advance_authority() {
  new_test_ext().execute_with(|| {
    let source = 19;
    let owner = PendingCheckOwner {
      actor: actor_ref(121, 3),
      plan_revision: 5,
    };
    let handle = DependencyRegistrationHandle {
      actor: owner.actor,
      plan_revision: owner.plan_revision,
      acknowledged_revision: 1,
    };
    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        revision: 1,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::install_dependency_registration(source, owner, 1),
        Ok(DependencyRegistrationMutation::Installed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    assert_eq!(
      Actors::begin_dependency_scan(source),
      Err(DependencyScanError::TransactionRequired)
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::begin_dependency_scan(source),
        Ok(DependencyScanMutation::Begun(1))
      );
      assert_eq!(
        Actors::revise_dependency_source(source),
        Ok(DependencyRevisionMutation::Advanced(2))
      );
      assert_eq!(
        Actors::begin_dependency_scan(source),
        Err(DependencyScanError::ScanAlreadyActive)
      );
      assert_eq!(
        DependencyRevisions::<Test>::get(source).scan_target,
        Some(1)
      );
      assert_eq!(
        Actors::process_dependency_scan_member(source, 1, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      assert_eq!(
        DependencyRegistrations::<Test>::get(source, owner.actor.actor_id),
        Some(handle)
      );
      assert_eq!(
        Actors::complete_dependency_scan(source, 1, 1),
        Ok(DependencyScanMutation::HandedOff(2))
      );
      let state = DependencyRevisions::<Test>::get(source);
      assert_eq!(state.scan_target, Some(2));
      assert_eq!(state.scan_cursor, 0);
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    let retained = DependencyRevisions::<Test>::get(source);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      DependencyRegistrations::<Test>::remove(source, owner.actor.actor_id);
      assert_eq!(
        Actors::process_dependency_scan_member(source, 2, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(DependencyRevisions::<Test>::get(source), retained);
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, owner.actor.actor_id),
      Some(handle)
    );

    let mut exhausted = retained;
    exhausted.exhausted = true;
    DependencyRevisions::<Test>::insert(source, exhausted);
    let before = DependencyRevisions::<Test>::get(source);
    let refused = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::complete_dependency_scan(source, 2, 0),
      )
    });
    assert_eq!(refused, Err(DependencyScanError::SourceExhausted));
    assert_eq!(DependencyRevisions::<Test>::get(source), before);
  });
}

#[test]
fn dependency_scan_processes_one_destination_before_advancing() {
  new_test_ext().execute_with(|| {
    let source = 29;
    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        revision: 3,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    let owners: Vec<_> = (0..5)
      .map(|offset| PendingCheckOwner {
        actor: actor_ref(130 + offset, 2),
        plan_revision: 7,
      })
      .collect();
    for (index, owner) in owners.iter().copied().enumerate() {
      PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        assert_eq!(
          Actors::install_dependency_registration(
            source,
            owner,
            if index == 0 || index == 3 { 1 } else { 3 },
          ),
          Ok(DependencyRegistrationMutation::Installed)
        );
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
    }

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::begin_dependency_scan(source),
        Ok(DependencyScanMutation::Begun(3))
      );
      assert_eq!(
        Actors::revise_dependency_source(source),
        Ok(DependencyRevisionMutation::Advanced(4))
      );
      let third = DependencyRegistrations::<Test>::get(source, owners[2].actor.actor_id).unwrap();
      assert_eq!(
        Actors::replace_dependency_registration(source, third, owners[2], 4),
        Ok(DependencyRegistrationMutation::Replaced)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    let before = DependencyRevisions::<Test>::get(source);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::process_dependency_scan_member(source, 3, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      assert_eq!(
        DependencyRegistrations::<Test>::get(source, owners[0].actor.actor_id)
          .unwrap()
          .acknowledged_revision,
        3
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(DependencyRevisions::<Test>::get(source), before);
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, owners[0].actor.actor_id)
        .unwrap()
        .acknowledged_revision,
      1
    );
    assert!(!PendingDependencyEvents::<Test>::contains_key(
      owners[0].actor.actor_id
    ));

    let coalesced = PendingDependencyEvent {
      owner: owners[0],
      source: 77,
      revision: 9,
    };
    PendingDependencyEvents::<Test>::insert(owners[0].actor.actor_id, coalesced);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::process_dependency_scan_member(source, 3, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      assert_eq!(
        PendingDependencyEvents::<Test>::get(owners[0].actor.actor_id),
        Some(coalesced)
      );
      assert_eq!(
        Actors::process_dependency_scan_member(source, 3, 1),
        Ok(DependencyScanMutation::Advanced(2))
      );
      assert_eq!(
        Actors::process_dependency_scan_member(source, 3, 2),
        Ok(DependencyScanMutation::Advanced(3))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, owners[2].actor.actor_id)
        .unwrap()
        .acknowledged_revision,
      4
    );

    let wrong_generation = PendingCheckOwner {
      actor: actor_ref(owners[3].actor.actor_id, 3),
      ..owners[3]
    };
    let wrong_plan = PendingCheckOwner {
      plan_revision: 8,
      ..owners[3]
    };
    for wrong_owner in [wrong_generation, wrong_plan] {
      PendingCheckOwners::<Test>::insert(wrong_owner.actor.actor_id, wrong_owner);
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        assert_eq!(
          Actors::process_dependency_scan_member(source, 3, 3),
          Err(DependencyScanError::PendingAuthorityMismatch)
        );
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
      assert_eq!(DependencyRevisions::<Test>::get(source).scan_cursor, 3);
      assert_eq!(
        DependencyRegistrations::<Test>::get(source, owners[3].actor.actor_id)
          .unwrap()
          .acknowledged_revision,
        1
      );
    }
    PendingCheckOwners::<Test>::insert(owners[3].actor.actor_id, owners[3]);
    PendingDependencyEvents::<Test>::insert(
      owners[3].actor.actor_id,
      PendingDependencyEvent {
        owner: wrong_plan,
        source,
        revision: 3,
      },
    );
    let destination_refused =
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::process_dependency_scan_member(source, 3, 3),
        )
      });
    assert_eq!(
      destination_refused,
      Err(DependencyScanError::PendingDestinationMismatch)
    );
    assert_eq!(DependencyRevisions::<Test>::get(source).scan_cursor, 3);
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, owners[3].actor.actor_id)
        .unwrap()
        .acknowledged_revision,
      1
    );
    PendingDependencyEvents::<Test>::remove(owners[3].actor.actor_id);

    let stale_position =
      DependencyRegistrationPositions::<Test>::get(source, owners[4].actor.actor_id).unwrap();
    DependencyRegistrations::<Test>::remove(source, owners[4].actor.actor_id);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::process_dependency_scan_member(source, 3, 3),
        Ok(DependencyScanMutation::Advanced(4))
      );
      assert_eq!(
        Actors::process_dependency_scan_member(source, 3, 4),
        Ok(DependencyScanMutation::Advanced(5))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(DependencyRegistrationHeaders::<Test>::get(source).count, 4);
    assert_eq!(
      PendingDependencyEvents::<Test>::get(owners[3].actor.actor_id),
      Some(PendingDependencyEvent {
        owner: owners[3],
        source,
        revision: 3,
      })
    );
    assert_eq!(
      DependencyRegistrationPages::<Test>::get(source, stale_position.page)
        .unwrap()
        .entries[stale_position.slot as usize],
      None
    );
    assert!(!DependencyRegistrationPositions::<Test>::contains_key(
      source,
      owners[4].actor.actor_id
    ));
    assert_eq!(
      Actors::complete_dependency_scan(source, 3, 5),
      Err(DependencyScanError::TransactionRequired)
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::complete_dependency_scan(source, 3, 5),
        Ok(DependencyScanMutation::HandedOff(4))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
  });
}

#[test]
fn pending_check_owner_is_exactly_generation_and_plan_bound() {
  new_test_ext().execute_with(|| {
    let old = PendingCheckOwner {
      actor: actor_ref(119, 7),
      plan_revision: 3,
    };
    PendingCheckOwners::<Test>::insert(old.actor.actor_id, old);
    assert_eq!(
      PendingCheckOwners::<Test>::get(old.actor.actor_id),
      Some(old)
    );

    let replacement = PendingCheckOwner {
      actor: actor_ref(old.actor.actor_id, 8),
      plan_revision: old.plan_revision,
    };
    let revised_plan = PendingCheckOwner {
      actor: old.actor,
      plan_revision: 4,
    };
    assert_ne!(old, replacement);
    assert_ne!(old, revised_plan);
    assert_eq!(
      PendingCheckOwners::<Test>::get(old.actor.actor_id),
      Some(old)
    );
  });
}

#[test]
fn dependency_registration_boundaries_are_exact_idempotent_and_transactional() {
  new_test_ext().execute_with(|| {
    let source = 23;
    let old_owner = PendingCheckOwner {
      actor: actor_ref(123, 4),
      plan_revision: 7,
    };
    let old_handle = DependencyRegistrationHandle {
      actor: old_owner.actor,
      plan_revision: old_owner.plan_revision,
      acknowledged_revision: 2,
    };
    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        revision: 3,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    PendingCheckOwners::<Test>::insert(old_owner.actor.actor_id, old_owner);

    assert_eq!(
      Actors::install_dependency_registration(source, old_owner, 2),
      Err(DependencyRegistrationError::TransactionRequired)
    );
    let installed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::install_dependency_registration(source, old_owner, 2),
      )
    });
    assert_eq!(installed, Ok(DependencyRegistrationMutation::Installed));
    assert_eq!(
      DependencyRegistrationHeaders::<Test>::get(source).next_index,
      1
    );
    assert_eq!(DependencyRegistrationHeaders::<Test>::get(source).count, 1);
    assert_eq!(
      DependencyRegistrationPages::<Test>::get(source, 0)
        .unwrap()
        .entries[0],
      Some(old_handle)
    );
    assert!(DependencyRegistrationPositions::<Test>::contains_key(
      source,
      old_owner.actor.actor_id
    ));
    let overlap = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::install_dependency_registration(source, old_owner, 2),
      )
    });
    assert_eq!(overlap, Ok(DependencyRegistrationMutation::Unchanged));

    let stale_generation = PendingCheckOwner {
      actor: actor_ref(old_owner.actor.actor_id, 5),
      plan_revision: old_owner.plan_revision,
    };
    let stale_plan = PendingCheckOwner {
      actor: old_owner.actor,
      plan_revision: old_owner.plan_revision + 1,
    };
    for owner in [stale_generation, stale_plan] {
      let refused = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::replace_dependency_registration(source, old_handle, owner, 3),
        )
      });
      assert_eq!(
        refused,
        Err(DependencyRegistrationError::PendingOwnerMismatch)
      );
      assert_eq!(
        DependencyRegistrations::<Test>::get(source, old_owner.actor.actor_id),
        Some(old_handle)
      );
    }
    let future = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::replace_dependency_registration(source, old_handle, old_owner, 4),
      )
    });
    assert_eq!(future, Err(DependencyRegistrationError::RevisionFromFuture));
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, old_owner.actor.actor_id),
      Some(old_handle)
    );

    let revised_owner = PendingCheckOwner {
      actor: old_owner.actor,
      plan_revision: 8,
    };
    PendingCheckOwners::<Test>::insert(revised_owner.actor.actor_id, revised_owner);
    let replacement = DependencyRegistrationHandle {
      actor: revised_owner.actor,
      plan_revision: revised_owner.plan_revision,
      acknowledged_revision: 3,
    };
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::replace_dependency_registration(source, old_handle, revised_owner, 3),
        Ok(DependencyRegistrationMutation::Replaced)
      );
      assert_eq!(
        DependencyRegistrations::<Test>::get(source, old_owner.actor.actor_id),
        Some(replacement)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, old_owner.actor.actor_id),
      Some(old_handle)
    );

    let removed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::remove_dependency_registration(source, old_handle),
      )
    });
    assert_eq!(removed, Ok(DependencyRegistrationMutation::Removed));
    assert!(!DependencyRegistrations::<Test>::contains_key(
      source,
      old_owner.actor.actor_id
    ));
    assert!(!DependencyRegistrationPositions::<Test>::contains_key(
      source,
      old_owner.actor.actor_id
    ));
    assert_eq!(DependencyRegistrationHeaders::<Test>::get(source).count, 0);
    assert_eq!(
      DependencyRegistrationHeaders::<Test>::get(source).free_count,
      1
    );
    assert!(DependencyRegistrationFreePositions::<Test>::contains_key(
      source, 0
    ));
    assert_eq!(
      DependencyRegistrationPages::<Test>::get(source, 0)
        .unwrap()
        .entries[0],
      None
    );
  });
}

#[test]
fn negative_dependency_evaluation_commits_only_exact_current_snapshot() {
  new_test_ext().execute_with(|| {
    let source = 37;
    let owner = PendingCheckOwner {
      actor: actor_ref(237, 4),
      plan_revision: 6,
    };
    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        revision: 2,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);

    assert_eq!(
      Actors::commit_negative_dependency_evaluation(source, owner, 2),
      Err(DependencyRegistrationError::TransactionRequired)
    );
    let installed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_evaluation(source, owner, 2),
      )
    });
    assert_eq!(installed, Ok(DependencyRegistrationMutation::Installed));
    let original = DependencyRegistrationHandle {
      actor: owner.actor,
      plan_revision: owner.plan_revision,
      acknowledged_revision: 2,
    };
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, owner.actor.actor_id),
      Some(original)
    );
    let unchanged = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_evaluation(source, owner, 2),
      )
    });
    assert_eq!(unchanged, Ok(DependencyRegistrationMutation::Unchanged));

    for stale_owner in [
      PendingCheckOwner {
        actor: actor_ref(owner.actor.actor_id, owner.actor.generation + 1),
        plan_revision: owner.plan_revision,
      },
      PendingCheckOwner {
        actor: owner.actor,
        plan_revision: owner.plan_revision + 1,
      },
    ] {
      let refused = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::commit_negative_dependency_evaluation(source, stale_owner, 2),
        )
      });
      assert_eq!(
        refused,
        Err(DependencyRegistrationError::PendingOwnerMismatch)
      );
      assert_eq!(
        DependencyRegistrations::<Test>::get(source, owner.actor.actor_id),
        Some(original)
      );
    }

    DependencyRevisions::<Test>::mutate(source, |state| state.revision = 3);
    let raced = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_evaluation(source, owner, 2),
      )
    });
    assert_eq!(raced, Err(DependencyRegistrationError::RevisionMismatch));
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, owner.actor.actor_id),
      Some(original)
    );

    let revised_owner = PendingCheckOwner {
      actor: owner.actor,
      plan_revision: owner.plan_revision + 1,
    };
    PendingCheckOwners::<Test>::insert(revised_owner.actor.actor_id, revised_owner);
    let replacement = DependencyRegistrationHandle {
      actor: revised_owner.actor,
      plan_revision: revised_owner.plan_revision,
      acknowledged_revision: 3,
    };
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::commit_negative_dependency_evaluation(source, revised_owner, 3),
        Ok(DependencyRegistrationMutation::Replaced)
      );
      assert_eq!(
        DependencyRegistrations::<Test>::get(source, owner.actor.actor_id),
        Some(replacement)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, owner.actor.actor_id),
      Some(original)
    );

    DependencyRevisions::<Test>::mutate(source, |state| state.exhausted = true);
    let exhausted = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_evaluation(source, revised_owner, 3),
      )
    });
    assert_eq!(exhausted, Err(DependencyRegistrationError::SourceExhausted));
    assert_eq!(
      DependencyRegistrations::<Test>::get(source, owner.actor.actor_id),
      Some(original)
    );
  });
}

#[test]
fn complete_negative_dependency_plan_replaces_all_sources_atomically() {
  new_test_ext().execute_with(|| {
    let owner = PendingCheckOwner {
      actor: actor_ref(238, 4),
      plan_revision: 6,
    };
    for (source, revision) in [(41, 2), (42, 3), (43, 4), (44, 5)] {
      DependencyRevisions::<Test>::insert(
        source,
        DependencyRevisionState {
          revision,
          scan_target: None,
          scan_cursor: 0,
          scan_end: 0,
          exhausted: false,
        },
      );
    }
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
    let initial = [
      DependencyPlanSource {
        source: 41,
        observed_revision: 2,
      },
      DependencyPlanSource {
        source: 42,
        observed_revision: 3,
      },
    ];
    assert_eq!(
      Actors::commit_negative_dependency_plan(owner, &initial, None),
      Err(DependencyRegistrationError::TransactionRequired)
    );
    let installed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(owner, &initial, None),
      )
    });
    assert_eq!(
      installed,
      Ok(DependencyPlanMutation {
        installed: 2,
        ..Default::default()
      })
    );
    assert_eq!(DependencyPlans::<Test>::get(owner.actor.actor_id).len(), 2);

    let revised_owner = PendingCheckOwner {
      actor: owner.actor,
      plan_revision: 7,
    };
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, revised_owner);
    let revised = [
      DependencyPlanSource {
        source: 42,
        observed_revision: 3,
      },
      DependencyPlanSource {
        source: 43,
        observed_revision: 4,
      },
    ];
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::commit_negative_dependency_plan(revised_owner, &revised, None),
        Ok(DependencyPlanMutation {
          installed: 1,
          replaced: 1,
          removed: 1,
          ..Default::default()
        })
      );
      assert!(!DependencyRegistrations::<Test>::contains_key(
        41,
        owner.actor.actor_id
      ));
      assert!(DependencyRegistrations::<Test>::contains_key(
        43,
        owner.actor.actor_id
      ));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert!(DependencyRegistrations::<Test>::contains_key(
      41,
      owner.actor.actor_id
    ));
    assert!(!DependencyRegistrations::<Test>::contains_key(
      43,
      owner.actor.actor_id
    ));
    assert_eq!(DependencyPlans::<Test>::get(owner.actor.actor_id).len(), 2);

    let duplicate = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(
          revised_owner,
          &[
            DependencyPlanSource {
              source: 41,
              observed_revision: 2,
            },
            DependencyPlanSource {
              source: 41,
              observed_revision: 2,
            },
          ],
          None,
        ),
      )
    });
    let raced = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(
          revised_owner,
          &[DependencyPlanSource {
            source: 43,
            observed_revision: 3,
          }],
          None,
        ),
      )
    });
    for refused in [duplicate, raced] {
      assert!(matches!(
        refused,
        Err(DependencyRegistrationError::DuplicateSource)
          | Err(DependencyRegistrationError::RevisionMismatch)
      ));
      assert_eq!(DependencyPlans::<Test>::get(owner.actor.actor_id).len(), 2);
    }

    DependencyRegistrationHeaders::<Test>::mutate(44, |header| {
      let capacity: u32 = <Test as crate::Config>::MaxActiveActors::get();
      header.count = capacity;
      header.next_index = u64::from(capacity);
    });
    let capacity = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(
          revised_owner,
          &[DependencyPlanSource {
            source: 44,
            observed_revision: 5,
          }],
          None,
        ),
      )
    });
    assert_eq!(capacity, Err(DependencyRegistrationError::CapacityExceeded));
    assert!(DependencyRegistrations::<Test>::contains_key(
      41,
      owner.actor.actor_id
    ));
    assert!(DependencyRegistrations::<Test>::contains_key(
      42,
      owner.actor.actor_id
    ));

    let committed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(revised_owner, &revised, None),
      )
    });
    assert_eq!(
      committed,
      Ok(DependencyPlanMutation {
        installed: 1,
        replaced: 1,
        removed: 1,
        ..Default::default()
      })
    );
    let unchanged = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(revised_owner, &revised, None),
      )
    });
    assert_eq!(
      unchanged,
      Ok(DependencyPlanMutation {
        retained: 2,
        ..Default::default()
      })
    );
    assert!(!DependencyRegistrations::<Test>::contains_key(
      41,
      owner.actor.actor_id
    ));
    assert!(DependencyRegistrations::<Test>::contains_key(
      42,
      owner.actor.actor_id
    ));
    assert!(DependencyRegistrations::<Test>::contains_key(
      43,
      owner.actor.actor_id
    ));
  });
}

#[test]
fn complete_negative_dependency_plan_replaces_block_and_tick_review_atomically() {
  new_test_ext().execute_with(|| {
    System::set_block_number(10);
    let source = 45;
    let owner = PendingCheckOwner {
      actor: actor_ref(239, 4),
      plan_revision: 6,
    };
    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        revision: 2,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
    let desired = [DependencyPlanSource {
      source,
      observed_revision: 2,
    }];
    let block_review = WakeupKey::Block(20);
    let installed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(owner, &desired, Some(block_review)),
      )
    });
    assert_eq!(
      installed,
      Ok(DependencyPlanMutation {
        installed: 1,
        timed_review: DependencyTimedReviewMutation::Installed,
        ..Default::default()
      })
    );
    let original = DependencyTimedReview {
      owner,
      deadline: block_review,
    };
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(original)
    );

    let retained = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(owner, &desired, Some(block_review)),
      )
    });
    assert_eq!(
      retained,
      Ok(DependencyPlanMutation {
        retained: 1,
        timed_review: DependencyTimedReviewMutation::Retained,
        ..Default::default()
      })
    );

    let revised_owner = PendingCheckOwner {
      actor: owner.actor,
      plan_revision: 7,
    };
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, revised_owner);
    let tick_review = WakeupKey::Tick(30);
    let replaced = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(revised_owner, &desired, Some(tick_review)),
      )
    });
    assert_eq!(
      replaced,
      Ok(DependencyPlanMutation {
        replaced: 1,
        timed_review: DependencyTimedReviewMutation::Replaced,
        ..Default::default()
      })
    );
    let replacement = DependencyTimedReview {
      owner: revised_owner,
      deadline: tick_review,
    };
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(replacement)
    );

    for invalid in [WakeupKey::Block(10), WakeupKey::Tick(10)] {
      let refused = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::commit_negative_dependency_plan(revised_owner, &desired, Some(invalid)),
        )
      });
      assert_eq!(refused, Err(DependencyRegistrationError::DeadlineNotFuture));
      assert_eq!(
        DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
        Some(replacement)
      );
    }

    DependencyRevisions::<Test>::insert(
      46,
      DependencyRevisionState {
        revision: 1,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    DependencyRegistrationHeaders::<Test>::mutate(46, |header| {
      let capacity = <Test as crate::Config>::MaxActiveActors::get();
      header.count = capacity;
      header.next_index = u64::from(capacity);
    });
    let capacity = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(
          revised_owner,
          &[DependencyPlanSource {
            source: 46,
            observed_revision: 1,
          }],
          Some(WakeupKey::Block(40)),
        ),
      )
    });
    assert_eq!(capacity, Err(DependencyRegistrationError::CapacityExceeded));
    assert!(DependencyRegistrations::<Test>::contains_key(
      source,
      owner.actor.actor_id
    ));
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(replacement)
    );

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::commit_negative_dependency_plan(revised_owner, &desired, None),
        Ok(DependencyPlanMutation {
          retained: 1,
          timed_review: DependencyTimedReviewMutation::Removed,
          ..Default::default()
        })
      );
      assert!(!DependencyTimedReviews::<Test>::contains_key(
        owner.actor.actor_id
      ));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(replacement)
    );

    let stale = PendingCheckOwner {
      actor: actor_ref(owner.actor.actor_id, owner.actor.generation + 1),
      plan_revision: revised_owner.plan_revision,
    };
    let refused = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::commit_negative_dependency_plan(stale, &desired, None),
      )
    });
    assert_eq!(
      refused,
      Err(DependencyRegistrationError::PendingOwnerMismatch)
    );
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(replacement)
    );
  });
}

#[test]
fn due_dependency_review_publishes_exact_pending_authority_atomically() {
  new_test_ext().execute_with(|| {
    System::set_block_number(10);
    let owner = PendingCheckOwner {
      actor: actor_ref(240, 4),
      plan_revision: 6,
    };
    let block_review = DependencyTimedReview {
      owner,
      deadline: WakeupKey::Block(20),
    };
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
    DependencyTimedReviews::<Test>::insert(owner.actor.actor_id, block_review);

    assert_eq!(
      Actors::publish_due_dependency_review(block_review),
      Err(DependencyDueReviewError::TransactionRequired)
    );
    let early = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(block_review),
      )
    });
    assert_eq!(early, Err(DependencyDueReviewError::NotDue));
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(block_review)
    );

    System::set_block_number(20);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::publish_due_dependency_review(block_review),
        Ok(DependencyDueReviewMutation::Published)
      );
      assert!(!DependencyTimedReviews::<Test>::contains_key(
        owner.actor.actor_id
      ));
      assert_eq!(
        PendingDependencyReviews::<Test>::get(owner.actor.actor_id),
        Some(block_review)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(block_review)
    );
    assert!(!PendingDependencyReviews::<Test>::contains_key(
      owner.actor.actor_id
    ));

    let published = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(block_review),
      )
    });
    assert_eq!(published, Ok(DependencyDueReviewMutation::Published));
    let duplicate = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(block_review),
      )
    });
    assert_eq!(duplicate, Ok(DependencyDueReviewMutation::AlreadyPending));

    let tick_owner = PendingCheckOwner {
      actor: actor_ref(241, 2),
      plan_revision: 3,
    };
    let tick_review = DependencyTimedReview {
      owner: tick_owner,
      deadline: WakeupKey::Tick(10),
    };
    PendingCheckOwners::<Test>::insert(tick_owner.actor.actor_id, tick_owner);
    DependencyTimedReviews::<Test>::insert(tick_owner.actor.actor_id, tick_review);
    let tick_published = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(tick_review),
      )
    });
    assert_eq!(tick_published, Ok(DependencyDueReviewMutation::Published));

    let raced_owner = PendingCheckOwner {
      actor: actor_ref(242, 1),
      plan_revision: 1,
    };
    let stale = DependencyTimedReview {
      owner: raced_owner,
      deadline: WakeupKey::Block(19),
    };
    let replacement = DependencyTimedReview {
      owner: PendingCheckOwner {
        plan_revision: 2,
        ..raced_owner
      },
      deadline: WakeupKey::Block(20),
    };
    PendingCheckOwners::<Test>::insert(raced_owner.actor.actor_id, replacement.owner);
    DependencyTimedReviews::<Test>::insert(raced_owner.actor.actor_id, replacement);
    let stale_deadline = DependencyTimedReview {
      owner: replacement.owner,
      deadline: stale.deadline,
    };
    let replaced = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(stale_deadline),
      )
    });
    assert_eq!(replaced, Err(DependencyDueReviewError::ReviewMismatch));
    let raced = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(stale),
      )
    });
    assert_eq!(raced, Err(DependencyDueReviewError::PendingOwnerMismatch));
    assert_eq!(
      DependencyTimedReviews::<Test>::get(raced_owner.actor.actor_id),
      Some(replacement)
    );

    let occupied_owner = PendingCheckOwner {
      actor: actor_ref(243, 1),
      plan_revision: 1,
    };
    let occupied = DependencyTimedReview {
      owner: occupied_owner,
      deadline: WakeupKey::Block(20),
    };
    PendingCheckOwners::<Test>::insert(occupied_owner.actor.actor_id, occupied_owner);
    DependencyTimedReviews::<Test>::insert(occupied_owner.actor.actor_id, occupied);
    PendingDependencyReviews::<Test>::insert(
      occupied_owner.actor.actor_id,
      DependencyTimedReview {
        deadline: WakeupKey::Block(19),
        ..occupied
      },
    );
    let refused = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(occupied),
      )
    });
    assert_eq!(refused, Err(DependencyDueReviewError::DestinationOccupied));
    assert_eq!(
      DependencyTimedReviews::<Test>::get(occupied_owner.actor.actor_id),
      Some(occupied)
    );
  });
}

#[test]
fn dependency_pending_causes_share_one_actor_destination() {
  new_test_ext().execute_with(|| {
    System::set_block_number(10);
    let owner = PendingCheckOwner {
      actor: actor_ref(244, 3),
      plan_revision: 7,
    };
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
    let review = DependencyTimedReview {
      owner,
      deadline: WakeupKey::Block(10),
    };
    DependencyTimedReviews::<Test>::insert(owner.actor.actor_id, review);
    let event = PendingDependencyEvent {
      owner,
      source: 57,
      revision: 2,
    };
    PendingDependencyEvents::<Test>::insert(owner.actor.actor_id, event);

    let blocked_review = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(review),
      )
    });
    assert_eq!(
      blocked_review,
      Err(DependencyDueReviewError::DestinationOccupied)
    );
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(review)
    );
    assert_eq!(
      PendingDependencyEvents::<Test>::get(owner.actor.actor_id),
      Some(event)
    );
    assert!(!PendingDependencyReviews::<Test>::contains_key(
      owner.actor.actor_id
    ));

    PendingDependencyEvents::<Test>::remove(owner.actor.actor_id);
    let published_review = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::publish_due_dependency_review(review),
      )
    });
    assert_eq!(published_review, Ok(DependencyDueReviewMutation::Published));

    DependencyRevisions::<Test>::insert(
      57,
      DependencyRevisionState {
        revision: 1,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::commit_negative_dependency_plan(
          owner,
          &[DependencyPlanSource {
            source: 57,
            observed_revision: 1,
          }],
          None,
        ),
        Ok(DependencyPlanMutation {
          installed: 1,
          ..Default::default()
        })
      );
      assert_eq!(
        Actors::publish_dependency_event(57),
        Ok(DependencyPublicationMutation::Begun(2))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    let blocked_event = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::process_dependency_scan_member(57, 2, 0),
      )
    });
    assert_eq!(
      blocked_event,
      Err(DependencyScanError::PendingDestinationMismatch)
    );
    assert_eq!(DependencyRevisions::<Test>::get(57).scan_cursor, 0);
    assert_eq!(
      DependencyRegistrations::<Test>::get(57, owner.actor.actor_id)
        .unwrap()
        .acknowledged_revision,
      1
    );
    assert_eq!(
      PendingDependencyReviews::<Test>::get(owner.actor.actor_id),
      Some(review)
    );
    assert!(!PendingDependencyEvents::<Test>::contains_key(
      owner.actor.actor_id
    ));

    let consumed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_review(
          review,
          &[DependencyPlanSource {
            source: 57,
            observed_revision: 2,
          }],
          None,
        ),
      )
    });
    assert_eq!(
      consumed,
      Ok(DependencyPlanMutation {
        replaced: 1,
        ..Default::default()
      })
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::process_dependency_scan_member(57, 2, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      assert_eq!(
        Actors::complete_dependency_scan(57, 2, 1),
        Ok(DependencyScanMutation::Completed)
      );
      assert_eq!(
        Actors::publish_dependency_event(57),
        Ok(DependencyPublicationMutation::Begun(3))
      );
      assert_eq!(
        Actors::process_dependency_scan_member(57, 3, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert!(PendingDependencyEvents::<Test>::contains_key(
      owner.actor.actor_id
    ));
    assert!(!PendingDependencyReviews::<Test>::contains_key(
      owner.actor.actor_id
    ));
  });
}

#[test]
fn pending_dependency_event_installs_complete_successor_before_consumption() {
  new_test_ext().execute_with(|| {
    System::set_block_number(10);
    let owner = PendingCheckOwner {
      actor: actor_ref(244, 3),
      plan_revision: 7,
    };
    for (source, revision) in [(57, 2), (58, 4), (59, 1)] {
      DependencyRevisions::<Test>::insert(
        source,
        DependencyRevisionState {
          revision,
          scan_target: None,
          scan_cursor: 0,
          scan_end: 0,
          exhausted: false,
        },
      );
    }
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::commit_negative_dependency_plan(
          owner,
          &[DependencyPlanSource {
            source: 57,
            observed_revision: 2,
          }],
          None,
        ),
        Ok(DependencyPlanMutation {
          installed: 1,
          ..Default::default()
        })
      );
      assert_eq!(
        Actors::publish_dependency_event(57),
        Ok(DependencyPublicationMutation::Begun(3))
      );
      assert_eq!(
        Actors::process_dependency_scan_member(57, 3, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      assert_eq!(
        Actors::complete_dependency_scan(57, 3, 1),
        Ok(DependencyScanMutation::Completed)
      );
      assert_eq!(
        Actors::publish_dependency_event(57),
        Ok(DependencyPublicationMutation::Begun(4))
      );
      assert_eq!(
        Actors::process_dependency_scan_member(57, 4, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    let pending = PendingDependencyEvent {
      owner,
      source: 57,
      revision: 3,
    };
    assert_eq!(
      PendingDependencyEvents::<Test>::get(owner.actor.actor_id),
      Some(pending)
    );
    assert_eq!(
      DependencyPlans::<Test>::get(owner.actor.actor_id)[0]
        .handle
        .acknowledged_revision,
      4
    );
    let successor = [
      DependencyPlanSource {
        source: 57,
        observed_revision: 4,
      },
      DependencyPlanSource {
        source: 58,
        observed_revision: 4,
      },
    ];

    assert_eq!(
      Actors::consume_pending_dependency_event(pending, &successor, None),
      Err(DependencyRegistrationError::TransactionRequired)
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::consume_pending_dependency_event(pending, &successor, Some(WakeupKey::Tick(20)),),
        Ok(DependencyPlanMutation {
          retained: 1,
          installed: 1,
          timed_review: DependencyTimedReviewMutation::Installed,
          ..Default::default()
        })
      );
      assert!(!PendingDependencyEvents::<Test>::contains_key(
        owner.actor.actor_id
      ));
      assert!(DependencyRegistrations::<Test>::contains_key(
        58,
        owner.actor.actor_id
      ));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(
      PendingDependencyEvents::<Test>::get(owner.actor.actor_id),
      Some(pending)
    );
    assert!(!DependencyRegistrations::<Test>::contains_key(
      58,
      owner.actor.actor_id
    ));

    let mismatched = PendingDependencyEvent {
      revision: 4,
      ..pending
    };
    let mismatch = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_event(mismatched, &successor, None),
      )
    });
    assert_eq!(
      mismatch,
      Err(DependencyRegistrationError::PendingEventMismatch)
    );

    let invalid = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_event(pending, &successor, Some(WakeupKey::Block(10))),
      )
    });
    assert_eq!(invalid, Err(DependencyRegistrationError::DeadlineNotFuture));
    assert_eq!(
      PendingDependencyEvents::<Test>::get(owner.actor.actor_id),
      Some(pending)
    );

    let registration = DependencyRegistrations::<Test>::take(57, owner.actor.actor_id).unwrap();
    let registration_race =
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::consume_pending_dependency_event(pending, &successor, None),
        )
      });
    assert_eq!(
      registration_race,
      Err(DependencyRegistrationError::RegistrationMissing)
    );
    DependencyRegistrations::<Test>::insert(57, owner.actor.actor_id, registration);

    DependencyRegistrationHeaders::<Test>::mutate(59, |header| {
      let capacity = <Test as crate::Config>::MaxActiveActors::get();
      header.count = capacity;
      header.next_index = u64::from(capacity);
    });
    let capacity = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_event(
          pending,
          &[DependencyPlanSource {
            source: 59,
            observed_revision: 1,
          }],
          None,
        ),
      )
    });
    assert_eq!(capacity, Err(DependencyRegistrationError::CapacityExceeded));

    let raced_owner = PendingCheckOwner {
      plan_revision: owner.plan_revision + 1,
      ..owner
    };
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, raced_owner);
    let owner_race = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_event(pending, &successor, None),
      )
    });
    assert_eq!(
      owner_race,
      Err(DependencyRegistrationError::PendingOwnerMismatch)
    );
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);

    let committed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_event(pending, &successor, None),
      )
    });
    assert_eq!(
      committed,
      Ok(DependencyPlanMutation {
        retained: 1,
        installed: 1,
        ..Default::default()
      })
    );
    assert!(!PendingDependencyEvents::<Test>::contains_key(
      owner.actor.actor_id
    ));
    let replay = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_event(pending, &successor, None),
      )
    });
    assert_eq!(
      replay,
      Err(DependencyRegistrationError::PendingEventMissing)
    );
  });
}

#[test]
fn pending_dependency_review_installs_complete_successor_before_consumption() {
  new_test_ext().execute_with(|| {
    System::set_block_number(10);
    let owner = PendingCheckOwner {
      actor: actor_ref(244, 3),
      plan_revision: 7,
    };
    for (source, revision) in [(47, 2), (48, 4)] {
      DependencyRevisions::<Test>::insert(
        source,
        DependencyRevisionState {
          revision,
          scan_target: None,
          scan_cursor: 0,
          scan_end: 0,
          exhausted: false,
        },
      );
    }
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);
    let initial = [DependencyPlanSource {
      source: 47,
      observed_revision: 2,
    }];
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::commit_negative_dependency_plan(owner, &initial, None),
        Ok(DependencyPlanMutation {
          installed: 1,
          ..Default::default()
        })
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    let pending = DependencyTimedReview {
      owner,
      deadline: WakeupKey::Block(10),
    };
    PendingDependencyReviews::<Test>::insert(owner.actor.actor_id, pending);
    let successor = [DependencyPlanSource {
      source: 48,
      observed_revision: 4,
    }];

    assert_eq!(
      Actors::consume_pending_dependency_review(pending, &successor, None),
      Err(DependencyRegistrationError::TransactionRequired)
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::consume_pending_dependency_review(pending, &successor, None),
        Ok(DependencyPlanMutation {
          installed: 1,
          removed: 1,
          ..Default::default()
        })
      );
      assert!(!PendingDependencyReviews::<Test>::contains_key(
        owner.actor.actor_id
      ));
      assert!(DependencyRegistrations::<Test>::contains_key(
        48,
        owner.actor.actor_id
      ));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(
      PendingDependencyReviews::<Test>::get(owner.actor.actor_id),
      Some(pending)
    );
    assert!(DependencyRegistrations::<Test>::contains_key(
      47,
      owner.actor.actor_id
    ));
    assert!(!DependencyRegistrations::<Test>::contains_key(
      48,
      owner.actor.actor_id
    ));

    let invalid = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_review(pending, &successor, Some(WakeupKey::Block(10))),
      )
    });
    assert_eq!(invalid, Err(DependencyRegistrationError::DeadlineNotFuture));
    assert_eq!(
      PendingDependencyReviews::<Test>::get(owner.actor.actor_id),
      Some(pending)
    );

    DependencyRevisions::<Test>::mutate(48, |state| state.revision = 5);
    let revision_race = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_review(pending, &successor, None),
      )
    });
    assert_eq!(
      revision_race,
      Err(DependencyRegistrationError::RevisionMismatch)
    );
    DependencyRevisions::<Test>::mutate(48, |state| state.revision = 4);

    DependencyRevisions::<Test>::insert(
      49,
      DependencyRevisionState {
        revision: 1,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    DependencyRegistrationHeaders::<Test>::mutate(49, |header| {
      let capacity = <Test as crate::Config>::MaxActiveActors::get();
      header.count = capacity;
      header.next_index = u64::from(capacity);
    });
    let capacity = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_review(
          pending,
          &[DependencyPlanSource {
            source: 49,
            observed_revision: 1,
          }],
          None,
        ),
      )
    });
    assert_eq!(capacity, Err(DependencyRegistrationError::CapacityExceeded));
    assert_eq!(
      PendingDependencyReviews::<Test>::get(owner.actor.actor_id),
      Some(pending)
    );
    assert!(DependencyRegistrations::<Test>::contains_key(
      47,
      owner.actor.actor_id
    ));

    let raced_owner = PendingCheckOwner {
      plan_revision: owner.plan_revision + 1,
      ..owner
    };
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, raced_owner);
    let raced = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_review(pending, &successor, None),
      )
    });
    assert_eq!(
      raced,
      Err(DependencyRegistrationError::PendingOwnerMismatch)
    );
    assert_eq!(
      PendingDependencyReviews::<Test>::get(owner.actor.actor_id),
      Some(pending)
    );
    PendingCheckOwners::<Test>::insert(owner.actor.actor_id, owner);

    let stale = DependencyTimedReview {
      deadline: WakeupKey::Block(9),
      ..pending
    };
    let mismatched = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_review(stale, &successor, None),
      )
    });
    assert_eq!(
      mismatched,
      Err(DependencyRegistrationError::PendingReviewMismatch)
    );

    let committed = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_review(pending, &successor, Some(WakeupKey::Tick(20))),
      )
    });
    assert_eq!(
      committed,
      Ok(DependencyPlanMutation {
        installed: 1,
        removed: 1,
        timed_review: DependencyTimedReviewMutation::Installed,
        ..Default::default()
      })
    );
    assert!(!PendingDependencyReviews::<Test>::contains_key(
      owner.actor.actor_id
    ));
    assert_eq!(
      DependencyTimedReviews::<Test>::get(owner.actor.actor_id),
      Some(DependencyTimedReview {
        owner,
        deadline: WakeupKey::Tick(20),
      })
    );
    let replay = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::consume_pending_dependency_review(pending, &successor, None),
      )
    });
    assert_eq!(
      replay,
      Err(DependencyRegistrationError::PendingReviewMissing)
    );
    assert!(DependencyRegistrations::<Test>::contains_key(
      48,
      owner.actor.actor_id
    ));
  });
}

#[test]
fn dependency_pages_keep_fixed_cursor_authority_across_fragmentation_and_new_members() {
  new_test_ext().execute_with(|| {
    let source = 29;
    DependencyRevisions::<Test>::insert(
      source,
      DependencyRevisionState {
        revision: 1,
        scan_target: None,
        scan_cursor: 0,
        scan_end: 0,
        exhausted: false,
      },
    );
    let mut handles = Vec::new();
    for actor_id in 200..234 {
      let owner = PendingCheckOwner {
        actor: actor_ref(actor_id, 1),
        plan_revision: 1,
      };
      PendingCheckOwners::<Test>::insert(actor_id, owner);
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        assert_eq!(
          Actors::install_dependency_registration(source, owner, 1),
          Ok(DependencyRegistrationMutation::Installed)
        );
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
      handles.push(DependencyRegistrationHandle {
        actor: owner.actor,
        plan_revision: 1,
        acknowledged_revision: 1,
      });
    }
    assert_eq!(
      DependencyRegistrationHeaders::<Test>::get(source).next_index,
      34
    );
    assert!(DependencyRegistrationPages::<Test>::contains_key(source, 0));
    assert!(DependencyRegistrationPages::<Test>::contains_key(source, 1));

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::begin_dependency_scan(source),
        Ok(DependencyScanMutation::Begun(1))
      );
      assert_eq!(
        Actors::process_dependency_scan_member(source, 1, 0),
        Ok(DependencyScanMutation::Advanced(1))
      );
      assert_eq!(
        Actors::remove_dependency_registration(source, handles[0]),
        Ok(DependencyRegistrationMutation::Removed)
      );
      let ahead_owner = PendingCheckOwner {
        actor: handles[33].actor,
        plan_revision: 2,
      };
      PendingCheckOwners::<Test>::insert(ahead_owner.actor.actor_id, ahead_owner);
      assert_eq!(
        Actors::replace_dependency_registration(source, handles[33], ahead_owner, 1),
        Ok(DependencyRegistrationMutation::Replaced)
      );
      handles[33].plan_revision = 2;

      let new_owner = PendingCheckOwner {
        actor: actor_ref(234, 1),
        plan_revision: 1,
      };
      PendingCheckOwners::<Test>::insert(new_owner.actor.actor_id, new_owner);
      assert_eq!(
        Actors::install_dependency_registration(source, new_owner, 1),
        Ok(DependencyRegistrationMutation::Installed)
      );
      assert_eq!(DependencyRevisions::<Test>::get(source).scan_end, 34);

      for cursor in 1..34 {
        assert_eq!(
          Actors::process_dependency_scan_member(source, 1, cursor),
          Ok(DependencyScanMutation::Advanced(cursor + 1))
        );
      }
      assert_eq!(
        Actors::process_dependency_scan_member(source, 1, 34),
        Err(DependencyScanError::ScanComplete)
      );
      assert_eq!(
        Actors::complete_dependency_scan(source, 1, 34),
        Ok(DependencyScanMutation::Completed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(
      DependencyRegistrationHeaders::<Test>::get(source).next_index,
      35
    );
    assert_eq!(DependencyRegistrationHeaders::<Test>::get(source).count, 34);
    assert_eq!(
      DependencyRegistrationPositions::<Test>::get(source, handles[33].actor.actor_id)
        .unwrap()
        .page,
      1
    );

    let reused_owner = PendingCheckOwner {
      actor: actor_ref(235, 1),
      plan_revision: 1,
    };
    PendingCheckOwners::<Test>::insert(reused_owner.actor.actor_id, reused_owner);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::install_dependency_registration(source, reused_owner, 1),
        Ok(DependencyRegistrationMutation::Installed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(
      DependencyRegistrationPositions::<Test>::get(source, reused_owner.actor.actor_id),
      Some(DependencyRegistrationPosition { page: 0, slot: 0 })
    );
    assert_eq!(
      DependencyRegistrationHeaders::<Test>::get(source).next_index,
      35
    );
    assert_eq!(
      DependencyRegistrationHeaders::<Test>::get(source).free_count,
      0
    );
  });
}

#[test]
fn canonical_service_round_preserves_markers_cursor_and_blocked_head() {
  new_test_ext().execute_with(|| {
    let members = [actor_ref(120, 1), actor_ref(121, 1), actor_ref(122, 1)];
    for actor in members {
      ActorProcesses::<Test>::insert(
        actor.actor_id,
        serving_process(actor, ServiceResidenceKind::Live),
      );
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        Actors::insert_service_member(actor, ServiceResidenceKind::Live, 4)
          .expect("same-block admission succeeds");
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
    }
    assert_eq!(ServiceHeader::<Test>::get().round_block, Some(4));
    assert!(members.iter().all(|actor| {
      let node = ServiceNodes::<Test>::get(actor.actor_id).expect("member");
      node.eligible_from == 5 && node.last_considered == 4
    }));

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::begin_service_round(5).expect("next round begins");
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::advance_service_head(members[0], 5).expect("staged attempt advances");
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(ServiceHeader::<Test>::get().cursor, Some(members[0]));
    assert_eq!(
      ActorProcesses::<Test>::get(members[0].actor_id)
        .unwrap()
        .last_attempted,
      None
    );

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::consider_service_head(5),
        Ok(ServiceRoundEncounter::Eligible(members[0]))
      );
      Actors::advance_service_head(members[0], 5).expect("first consideration advances");
      assert_eq!(
        ActorProcesses::<Test>::get(members[0].actor_id)
          .unwrap()
          .last_attempted,
        Some(5)
      );
      assert_eq!(ServiceHeader::<Test>::get().cursor, Some(members[1]));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::advance_service_head(members[1], 5).expect("second consideration advances");
      Actors::advance_service_head(members[2], 5).expect("third consideration wraps");
      assert_eq!(
        Actors::consider_service_head(5),
        Ok(ServiceRoundEncounter::Closed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
  });
}

#[test]
fn canonical_service_idle_no_work_head_advances_without_an_attempt() {
  new_test_ext().execute_with(|| {
    let members = [actor_ref(130, 1), actor_ref(131, 1)];
    for actor in members {
      ActorProcesses::<Test>::insert(
        actor.actor_id,
        serving_process(actor, ServiceResidenceKind::Live),
      );
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        Actors::insert_service_member(actor, ServiceResidenceKind::Live, 4)
          .expect("same-block admission succeeds");
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
    }

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::begin_service_round(5).expect("next round begins");
      assert_eq!(
        Actors::consider_service_head(5),
        Ok(ServiceRoundEncounter::Eligible(members[0]))
      );
      Actors::advance_idle_service_head(members[0], 5)
        .expect("an Idle retained resident advances without executing a Step");
      let node = ServiceNodes::<Test>::get(members[0].actor_id).expect("retained member");
      assert_eq!(node.last_considered, 5);
      assert_eq!(
        ActorProcesses::<Test>::get(members[0].actor_id)
          .unwrap()
          .last_attempted,
        None,
        "a no-work encounter records no attempt"
      );
      assert_eq!(ServiceHeader::<Test>::get().cursor, Some(members[1]));
      assert_eq!(
        Actors::consider_service_head(5),
        Ok(ServiceRoundEncounter::Eligible(members[1]))
      );
      Actors::advance_idle_service_head(members[1], 5).expect("second no-work resident advances");
      assert_eq!(ServiceHeader::<Test>::get().cursor, Some(members[0]));
      assert_eq!(
        Actors::consider_service_head(5),
        Ok(ServiceRoundEncounter::Closed),
        "the round closes once every resident has been considered exactly once"
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
  });
}

#[test]
fn mandatory_service_frontier_dispatches_the_selected_zero_step_kind() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(4);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, Default::default());
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    ActorControlLocators::<Test>::remove(actor_id);
    ActorUnsignaledControlCells::<Test>::remove(actor_id);
    frame_system::Pallet::<Test>::set_block_number(5);
    let before_header = ServiceHeader::<Test>::get();
    let before_process = ActorProcesses::<Test>::get(actor_id).unwrap();
    let selector = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible());
    let zero_step = <Test as crate::Config>::WeightInfo::scheduler_inner_zero_step_complete()
      .saturating_add(
        <Test as crate::Config>::WeightInfo::service_round_admit_eligible().max(
          <Test as crate::Config>::WeightInfo::service_member_retire_interior()
            .max(<Test as crate::Config>::WeightInfo::service_member_retire_pair_cursor())
            .max(<Test as crate::Config>::WeightInfo::service_member_retire_singleton()),
        ),
      );
    let complete = selector.saturating_add(zero_step);
    let mut refused = WeightMeter::with_limit(complete.saturating_sub(Weight::from_parts(1, 0)));
    assert_eq!(
      Actors::service_canonical_round_head(&mut refused, 5),
      Err(ServiceRoundError::InsufficientWeight)
    );
    assert_eq!(refused.consumed(), Weight::zero());
    assert_eq!(ServiceHeader::<Test>::get(), before_header);
    assert_eq!(ActorProcesses::<Test>::get(actor_id), Some(before_process));

    let mut admitted = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head(&mut admitted, 5),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );
    assert_eq!(admitted.consumed(), complete);
    let stored = Actors::load_service_actor_semantic_state(actor, ServiceResidenceKind::Pending)
      .expect("retained zero-Step Actor remains canonical");
    assert_eq!(stored.identity.cycle_nonce, 1);
    assert_eq!(stored.hot.last_cycle_block, Some(5));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id)
        .unwrap()
        .last_attempted,
      Some(5)
    );
  });
}

#[test]
fn mandatory_service_frontier_pre_admits_and_executes_one_effectful_head() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(4);
    let step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(1),
      amount: AmountResolution::Fixed(1),
    });
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![step]).unwrap(),
    );
    let sovereign = Actors::actor_identity(actor_id).unwrap().sovereign_account;
    set_asset_balance(&sovereign, TestAsset::Local(1), 10);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    let resources = Actors::load_current_step_service_state(actor_id)
      .unwrap()
      .2
      .resources;
    ActorControlLocators::<Test>::remove(actor_id);
    frame_system::Pallet::<Test>::set_block_number(5);
    let before_header = ServiceHeader::<Test>::get();
    let before_process = ActorProcesses::<Test>::get(actor_id).unwrap();
    let recipient_before = asset_balance(&BOB, TestAsset::Local(1));
    let selector = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible());
    let suffix = <Test as crate::Config>::WeightInfo::service_round_admit_eligible().max(
      <Test as crate::Config>::WeightInfo::service_member_retire_interior()
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_pair_cursor())
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_singleton()),
    );
    let complete = selector
      .saturating_add(resources.control)
      .saturating_add(resources.effect)
      .saturating_add(suffix);
    let mut refused = WeightMeter::with_limit(complete.saturating_sub(Weight::from_parts(1, 0)));
    assert_eq!(
      Actors::service_canonical_round_head(&mut refused, 5),
      Err(ServiceRoundError::InsufficientWeight)
    );
    assert_eq!(refused.consumed(), Weight::zero());
    assert_eq!(ServiceHeader::<Test>::get(), before_header);
    assert_eq!(ActorProcesses::<Test>::get(actor_id), Some(before_process));
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), recipient_before);

    let budget = <Test as crate::Config>::BlockResourceBudget::get();
    let mut resource_state = crate::BlockResourceState::new(5);
    assert_ok!(resource_state.begin_prepass());
    assert_ok!(resource_state.open_external_phase());
    assert_ok!(resource_state.begin_drain());
    let resource_before = resource_state;
    let mut resource_refused = resource_state;
    let mut exhausted = resource_refused
      .reserve(
        budget.limits(),
        crate::BlockResourceDomain::ActorControl,
        budget.limits().actor_control(),
      )
      .expect("test exhausts ActorControl");
    assert_ok!(resource_refused.settle(&mut exhausted, budget.limits().actor_control(),));
    let resource_refused_before = resource_refused;
    let mut admitted = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut admitted,
        5,
        &mut resource_refused,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Err(ServiceRoundError::ResourceUnavailable)
    );
    assert_eq!(admitted.consumed(), Weight::zero());
    assert_eq!(resource_refused, resource_refused_before);
    assert_eq!(ServiceHeader::<Test>::get(), before_header);
    assert_eq!(ActorProcesses::<Test>::get(actor_id), Some(before_process));
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), recipient_before);

    let mut admitted = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut admitted,
        5,
        &mut resource_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );
    assert!(admitted.consumed().all_lte(complete));
    assert_ne!(resource_state.usage(), resource_before.usage());
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert_eq!(
      asset_balance(&BOB, TestAsset::Local(1)),
      recipient_before + 1
    );
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id)
        .unwrap()
        .last_attempted,
      Some(5)
    );
  });
}

#[test]
fn mandatory_service_commits_abort_cycle_failure_and_retains_service_residence() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(4);
    let step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(1),
      amount: AmountResolution::Fixed(1),
    });
    assert_eq!(step.on_error, StepErrorPolicy::AbortCycle);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![step]).unwrap(),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    let resources = Actors::load_current_step_service_state(actor_id)
      .unwrap()
      .2
      .resources;
    ActorControlLocators::<Test>::remove(actor_id);
    frame_system::Pallet::<Test>::set_block_number(5);
    let selector = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible());
    let suffix = <Test as crate::Config>::WeightInfo::service_round_admit_eligible().max(
      <Test as crate::Config>::WeightInfo::service_member_retire_interior()
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_pair_cursor())
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_singleton()),
    );
    let complete = selector
      .saturating_add(resources.control)
      .saturating_add(resources.effect)
      .saturating_add(suffix);
    let budget = <Test as crate::Config>::BlockResourceBudget::get();
    let mut resource_state = crate::BlockResourceState::new(5);
    assert_ok!(resource_state.begin_prepass());
    assert_ok!(resource_state.open_external_phase());
    assert_ok!(resource_state.begin_drain());
    let resource_before = resource_state;
    let mut meter = WeightMeter::with_limit(complete);

    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut meter,
        5,
        &mut resource_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );

    assert!(meter.consumed().all_lte(complete));
    assert_ne!(resource_state.usage(), resource_before.usage());
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
    let stored = Actors::load_service_actor_semantic_state(actor, ServiceResidenceKind::Pending)
      .expect("aborted cycle remains a retained Service resident");
    assert_eq!(stored.hot.cycle_state, CycleState::Idle);
    assert_eq!(stored.hot.unsuccessful_attempt_streak, 0);
    assert_eq!(stored.hot.last_cycle_block, Some(5));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id)
        .unwrap()
        .last_attempted,
      Some(5)
    );
  });
}

#[test]
fn mandatory_service_commits_permanent_retry_failure_without_parking() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(4);
    let asset_out = TestAsset::Local(91);
    setup_pool(TestAsset::Native, asset_out, 1_000_000, 1);
    set_asset_balance(&u64::MAX, asset_out, 1);
    let mut step = make_step(Task::SwapIn {
      asset_in: TestAsset::Native,
      asset_out,
      amount_in: AmountResolution::Fixed(1),
      slippage_tolerance: Perbill::one(),
    });
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![step]).unwrap(),
    );
    fund_native(actor_id, 10);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    let resources = Actors::load_current_step_service_state(actor_id)
      .unwrap()
      .2
      .resources;
    ActorControlLocators::<Test>::remove(actor_id);
    frame_system::Pallet::<Test>::set_block_number(5);
    let selector = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible());
    let suffix = <Test as crate::Config>::WeightInfo::service_round_admit_eligible().max(
      <Test as crate::Config>::WeightInfo::service_member_retire_interior()
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_pair_cursor())
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_singleton()),
    );
    let complete = selector
      .saturating_add(resources.control)
      .saturating_add(resources.effect)
      .saturating_add(suffix);
    let budget = <Test as crate::Config>::BlockResourceBudget::get();
    let mut resource_state = crate::BlockResourceState::new(5);
    assert_ok!(resource_state.begin_prepass());
    assert_ok!(resource_state.open_external_phase());
    assert_ok!(resource_state.begin_drain());
    let resource_before = resource_state;
    let mut meter = WeightMeter::with_limit(complete);

    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut meter,
        5,
        &mut resource_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );

    assert!(meter.consumed().all_lte(complete));
    assert_ne!(resource_state.usage(), resource_before.usage());
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    assert!(!DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
    let stored = Actors::load_service_actor_semantic_state(actor, ServiceResidenceKind::Pending)
      .expect("permanent retry failure remains a retained Service resident");
    assert_eq!(stored.hot.cycle_state, CycleState::Idle);
    assert_eq!(stored.hot.unsuccessful_attempt_streak, 1);
    assert_eq!(stored.hot.last_cycle_block, Some(5));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id)
        .unwrap()
        .last_attempted,
      Some(5)
    );
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepFailed {
        actor_id: id,
        retry_class: RetryClass::Permanent,
        ..
      } if *id == actor_id
    )));
  });
}

#[test]
fn mandatory_service_closes_locally_exhausted_retry_and_removes_residence() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(4);
    setup_temporary_retry_pool();
    set_temporary_dex_failure(true);
    let mut step = make_step(Task::SwapIn {
      asset_in: TestAsset::Native,
      asset_out: TestAsset::Local(77),
      amount_in: AmountResolution::Fixed(10),
      slippage_tolerance: Perbill::one(),
    });
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 2 };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![step]).unwrap(),
    );
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    let resources = Actors::load_current_step_service_state(actor_id)
      .unwrap()
      .2
      .resources;
    ActorControlLocators::<Test>::remove(actor_id);
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).unwrap().residence,
      Some(ProcessResidence::Service(ServiceResidenceKind::Pending))
    );
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
    let selector = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible());
    let suffix = <Test as crate::Config>::WeightInfo::service_round_admit_eligible().max(
      <Test as crate::Config>::WeightInfo::service_member_retire_interior()
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_pair_cursor())
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_singleton()),
    );
    let complete = selector
      .saturating_add(resources.control)
      .saturating_add(resources.effect)
      .saturating_add(suffix);
    let budget = <Test as crate::Config>::BlockResourceBudget::get();
    let mut first_state = crate::BlockResourceState::new(5);
    assert_ok!(first_state.begin_prepass());
    assert_ok!(first_state.open_external_phase());
    assert_ok!(first_state.begin_drain());
    let mut first_meter = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut first_meter,
        5,
        &mut first_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );
    assert_eq!(first_state.outstanding_reservations(), 0);
    assert_eq!(
      ActorRunStateStore::<Test>::get(actor_id)
        .expect("first temporary failure retains its Run")
        .unsuccessful_attempts_at_cursor,
      1
    );
    assert!(ServiceNodes::<Test>::contains_key(actor_id));

    frame_system::Pallet::<Test>::set_block_number(6);
    let before_header = ServiceHeader::<Test>::get();
    let before_semantic = ActorSemanticStates::<Test>::get(actor_id).unwrap();
    let before_process = ActorProcesses::<Test>::get(actor_id).unwrap();
    let before_run = ActorRunStateStore::<Test>::get(actor_id).unwrap();
    let before_run_coordinates = (
      before_run.cycle_nonce,
      before_run.cursor,
      before_run.eligible_at,
      before_run.unsuccessful_attempts_at_cursor,
      before_run.cumulative_outcomes,
    );
    let mut resource_refused = crate::BlockResourceState::new(6);
    assert_ok!(resource_refused.begin_prepass());
    assert_ok!(resource_refused.open_external_phase());
    assert_ok!(resource_refused.begin_drain());
    let mut exhausted = resource_refused
      .reserve(
        budget.limits(),
        crate::BlockResourceDomain::ActorControl,
        budget.limits().actor_control(),
      )
      .expect("test exhausts ActorControl");
    assert_ok!(resource_refused.settle(&mut exhausted, budget.limits().actor_control()));
    let resource_refused_before = resource_refused;
    let mut refused_meter = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut refused_meter,
        6,
        &mut resource_refused,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Err(ServiceRoundError::ResourceUnavailable)
    );
    assert_eq!(refused_meter.consumed(), Weight::zero());
    assert_eq!(resource_refused, resource_refused_before);
    assert_eq!(ServiceHeader::<Test>::get(), before_header);
    assert_eq!(
      ActorSemanticStates::<Test>::get(actor_id),
      Some(before_semantic)
    );
    assert_eq!(ActorProcesses::<Test>::get(actor_id), Some(before_process));
    let retained_run = ActorRunStateStore::<Test>::get(actor_id).unwrap();
    assert_eq!(
      (
        retained_run.cycle_nonce,
        retained_run.cursor,
        retained_run.eligible_at,
        retained_run.unsuccessful_attempts_at_cursor,
        retained_run.cumulative_outcomes,
      ),
      before_run_coordinates
    );

    let mut resource_state = crate::BlockResourceState::new(6);
    assert_ok!(resource_state.begin_prepass());
    assert_ok!(resource_state.open_external_phase());
    assert_ok!(resource_state.begin_drain());
    let resource_before = resource_state;
    let mut meter = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut meter,
        6,
        &mut resource_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );

    assert!(meter.consumed().all_lte(complete));
    assert_ne!(resource_state.usage(), resource_before.usage());
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert!(!ActorSemanticStates::<Test>::contains_key(actor_id));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id)
        .expect("terminal attempt evidence remains queryable")
        .last_attempted,
      Some(5)
    );
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    assert!(!ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(ServiceHeader::<Test>::get().count, 0);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::RetryAttemptsExhausted,
      } if *id == actor_id
    )));
  });
}

#[test]
fn mandatory_service_closes_at_global_failure_limit_and_rolls_back_refusal() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(4);
    set_max_consecutive_failures(2);
    setup_temporary_retry_pool();
    set_temporary_dex_failure(true);
    let mut step = make_step(Task::SwapIn {
      asset_in: TestAsset::Native,
      asset_out: TestAsset::Local(77),
      amount_in: AmountResolution::Fixed(10),
      slippage_tolerance: Perbill::one(),
    });
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![step]).unwrap(),
    );
    fund_native(actor_id, 100);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    let resources = Actors::load_current_step_service_state(actor_id)
      .unwrap()
      .2
      .resources;
    ActorControlLocators::<Test>::remove(actor_id);
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id).unwrap().residence,
      Some(ProcessResidence::Service(ServiceResidenceKind::Pending))
    );
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
    let selector = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible());
    let suffix = <Test as crate::Config>::WeightInfo::service_round_admit_eligible().max(
      <Test as crate::Config>::WeightInfo::service_member_retire_interior()
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_pair_cursor())
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_singleton()),
    );
    let complete = selector
      .saturating_add(resources.control)
      .saturating_add(resources.effect)
      .saturating_add(suffix);
    let budget = <Test as crate::Config>::BlockResourceBudget::get();
    let mut first_state = crate::BlockResourceState::new(5);
    assert_ok!(first_state.begin_prepass());
    assert_ok!(first_state.open_external_phase());
    assert_ok!(first_state.begin_drain());
    let mut first_meter = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut first_meter,
        5,
        &mut first_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );
    assert_eq!(first_state.outstanding_reservations(), 0);
    let first_run = ActorRunStateStore::<Test>::get(actor_id)
      .expect("first failure remains below the global limit");
    assert_eq!(first_run.unsuccessful_attempts_at_cursor, 1);
    let ActorSemanticState::Active(first_semantic) =
      ActorSemanticStates::<Test>::get(actor_id).expect("first failure retains semantic state")
    else {
      panic!("first failure keeps active semantics");
    };
    assert_eq!(first_semantic.hot.unsuccessful_attempt_streak, 1);

    frame_system::Pallet::<Test>::set_block_number(6);
    let before_header = ServiceHeader::<Test>::get();
    let before_semantic = ActorSemanticStates::<Test>::get(actor_id).unwrap();
    let before_process = ActorProcesses::<Test>::get(actor_id).unwrap();
    let before_run = ActorRunStateStore::<Test>::get(actor_id).unwrap();
    let before_run_coordinates = (
      before_run.cycle_nonce,
      before_run.cursor,
      before_run.eligible_at,
      before_run.unsuccessful_attempts_at_cursor,
      before_run.cumulative_outcomes,
    );
    let mut resource_refused = crate::BlockResourceState::new(6);
    assert_ok!(resource_refused.begin_prepass());
    assert_ok!(resource_refused.open_external_phase());
    assert_ok!(resource_refused.begin_drain());
    let mut exhausted = resource_refused
      .reserve(
        budget.limits(),
        crate::BlockResourceDomain::ActorControl,
        budget.limits().actor_control(),
      )
      .expect("test exhausts ActorControl");
    assert_ok!(resource_refused.settle(&mut exhausted, budget.limits().actor_control()));
    let resource_refused_before = resource_refused;
    let mut refused_meter = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut refused_meter,
        6,
        &mut resource_refused,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Err(ServiceRoundError::ResourceUnavailable)
    );
    assert_eq!(refused_meter.consumed(), Weight::zero());
    assert_eq!(resource_refused, resource_refused_before);
    assert_eq!(ServiceHeader::<Test>::get(), before_header);
    assert_eq!(
      ActorSemanticStates::<Test>::get(actor_id),
      Some(before_semantic)
    );
    assert_eq!(ActorProcesses::<Test>::get(actor_id), Some(before_process));
    let retained_run = ActorRunStateStore::<Test>::get(actor_id).unwrap();
    assert_eq!(
      (
        retained_run.cycle_nonce,
        retained_run.cursor,
        retained_run.eligible_at,
        retained_run.unsuccessful_attempts_at_cursor,
        retained_run.cumulative_outcomes,
      ),
      before_run_coordinates
    );

    let mut resource_state = crate::BlockResourceState::new(6);
    assert_ok!(resource_state.begin_prepass());
    assert_ok!(resource_state.open_external_phase());
    assert_ok!(resource_state.begin_drain());
    let resource_before = resource_state;
    let mut meter = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut meter,
        6,
        &mut resource_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );

    assert!(meter.consumed().all_lte(complete));
    assert_ne!(resource_state.usage(), resource_before.usage());
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert!(!ActorSemanticStates::<Test>::contains_key(actor_id));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id)
        .expect("terminal global-exhaustion evidence remains queryable")
        .last_attempted,
      Some(5)
    );
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    assert!(!ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(!DeadlineHandles::<Test>::contains_key(actor_id));
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(ServiceHeader::<Test>::get().count, 0);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::ActorClosed {
        actor_id: id,
        reason: CloseReason::ConsecutiveFailures,
      } if *id == actor_id
    )));
  });
}

#[test]
fn mandatory_service_continues_after_failed_step_without_repeating_the_prefix() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(4);
    setup_temporary_retry_pool();
    set_temporary_dex_failure(true);
    let failed = StepOf::<Test> {
      precondition: None,
      task: Task::SwapIn {
        asset_in: TestAsset::Native,
        asset_out: TestAsset::Local(77),
        amount_in: AmountResolution::Fixed(10),
        slippage_tolerance: Perbill::one(),
      },
      on_error: StepErrorPolicy::ContinueNextStep,
    };
    let completed = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(2),
      amount: AmountResolution::Fixed(1),
    });
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![failed, completed]).unwrap(),
    );
    let sovereign = Actors::actor_identity(actor_id).unwrap().sovereign_account;
    fund_native(actor_id, 100);
    set_asset_balance(&sovereign, TestAsset::Local(2), 10);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    let first_resources = Actors::load_current_step_service_state(actor_id)
      .unwrap()
      .2
      .resources;
    ActorControlLocators::<Test>::remove(actor_id);
    frame_system::Pallet::<Test>::set_block_number(5);
    let budget = <Test as crate::Config>::BlockResourceBudget::get();
    let selector = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible());
    let suffix = <Test as crate::Config>::WeightInfo::service_round_admit_eligible().max(
      <Test as crate::Config>::WeightInfo::service_member_retire_interior()
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_pair_cursor())
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_singleton()),
    );
    let first_complete = selector
      .saturating_add(first_resources.control)
      .saturating_add(first_resources.effect)
      .saturating_add(suffix);
    let mut first_state = crate::BlockResourceState::new(5);
    assert_ok!(first_state.begin_prepass());
    assert_ok!(first_state.open_external_phase());
    assert_ok!(first_state.begin_drain());
    let mut first_meter = WeightMeter::with_limit(first_complete);

    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut first_meter,
        5,
        &mut first_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );
    assert_eq!(asset_balance(&sovereign, TestAsset::Local(77)), 0);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(2)), 0);
    assert_eq!(first_state.outstanding_reservations(), 0);
    let running = ActorRunStateStore::<Test>::get(actor_id).expect("cycle remains Running");
    assert_eq!(running.cursor, 1);
    assert_eq!(running.cumulative_outcomes.failed_steps, 1);
    assert!(ServiceNodes::<Test>::contains_key(actor_id));

    frame_system::Pallet::<Test>::set_block_number(6);
    let semantic = Actors::load_service_actor_semantic_state(actor, ServiceResidenceKind::Pending)
      .expect("continued Actor remains in canonical Service");
    let second_resources = Actors::load_actor_service_state_with_control(
      actor_id,
      semantic.identity,
      semantic.hot,
      semantic.admission,
    )
    .and_then(|(_, _, loaded)| loaded)
    .expect("continued Step loads from canonical authority")
    .resources;
    let second_complete = selector
      .saturating_add(second_resources.control)
      .saturating_add(second_resources.effect)
      .saturating_add(suffix);
    let mut second_state = crate::BlockResourceState::new(6);
    assert_ok!(second_state.begin_prepass());
    assert_ok!(second_state.open_external_phase());
    assert_ok!(second_state.begin_drain());
    let mut second_meter = WeightMeter::with_limit(second_complete);

    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut second_meter,
        6,
        &mut second_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );
    assert_eq!(asset_balance(&sovereign, TestAsset::Local(77)), 0);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(2)), 1);
    assert_eq!(second_state.outstanding_reservations(), 0);
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    assert_eq!(
      ActorProcesses::<Test>::get(actor_id)
        .unwrap()
        .last_attempted,
      Some(6)
    );
  });
}

#[test]
fn mandatory_service_routes_later_retry_through_preplanned_block_deadline() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(1),
      amount: AmountResolution::Fixed(1),
    });
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 3,
      },
      None,
      BoundedVec::try_from(vec![step]).unwrap(),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let actor = actor_ref(actor_id, record.generation);
    let resources = Actors::load_current_step_service_state(actor_id)
      .unwrap()
      .2
      .resources;
    ActorControlLocators::<Test>::remove(actor_id);
    frame_system::Pallet::<Test>::set_block_number(2);
    let before_header = ServiceHeader::<Test>::get();
    let before_process = ActorProcesses::<Test>::get(actor_id).unwrap();
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    let retry_key = WakeupKey::Block(5);
    DeadlineHeaders::<Test>::insert(
      retry_key,
      DeadlineHeader {
        first_page: 0,
        last_page: 0,
        next_page: 1,
        page_count: 1,
        count: 1,
      },
    );
    let selector = <Test as crate::Config>::WeightInfo::service_round_begin_populated()
      .saturating_add(<Test as crate::Config>::WeightInfo::service_round_probe_eligible());
    let suffix = <Test as crate::Config>::WeightInfo::service_round_admit_eligible().max(
      <Test as crate::Config>::WeightInfo::service_member_retire_interior()
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_pair_cursor())
        .max(<Test as crate::Config>::WeightInfo::service_member_retire_singleton()),
    );
    let complete = selector
      .saturating_add(resources.control)
      .saturating_add(resources.effect)
      .saturating_add(suffix);
    let budget = <Test as crate::Config>::BlockResourceBudget::get();
    let mut resource_state = crate::BlockResourceState::new(2);
    assert_ok!(resource_state.begin_prepass());
    assert_ok!(resource_state.open_external_phase());
    assert_ok!(resource_state.begin_drain());
    let resource_before = resource_state;
    let mut meter = WeightMeter::with_limit(complete);
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut meter,
        2,
        &mut resource_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Err(ServiceRoundError::ProcessResidenceMismatch)
    );
    assert_eq!(meter.consumed(), Weight::zero());
    assert_eq!(resource_state, resource_before);
    assert_eq!(ServiceHeader::<Test>::get(), before_header);
    assert_eq!(ActorProcesses::<Test>::get(actor_id), Some(before_process));
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    assert!(!DeadlineHandles::<Test>::contains_key(actor_id));

    DeadlineHeaders::<Test>::remove(retry_key);
    assert_eq!(
      Actors::service_canonical_round_head_with_resources(
        &mut meter,
        2,
        &mut resource_state,
        budget.limits(),
        crate::BlockResourceDomain::ActorDrainEffect,
      ),
      Ok(ServiceRoundEncounter::Eligible(actor))
    );
    assert!(meter.consumed().all_lte(complete));
    assert_ne!(resource_state.usage(), resource_before.usage());
    assert_eq!(resource_state.outstanding_reservations(), 0);
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(
      DeadlineHandles::<Test>::get(actor_id).map(|handle| handle.key),
      Some(retry_key)
    );
    assert!(matches!(
      ActorProcesses::<Test>::get(actor_id).unwrap().residence,
      Some(ProcessResidence::Deadline { key, .. }) if key == retry_key
    ));
    let run = ActorRunStateStore::<Test>::get(actor_id).expect("retry Run remains canonical");
    assert_eq!(run.unsuccessful_attempts_at_cursor, 1);
    assert_eq!(run.eligible_at, 5);
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
  });
}

#[test]
fn on_idle_recovers_later_retry_and_completes_once_in_fresh_drain() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let mut step = make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Local(1),
      amount: AmountResolution::Fixed(1),
    });
    step.on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let actor_id = create_system_with(
      ALICE,
      Schedule {
        trigger: Trigger::manual(),
        cooldown_blocks: 3,
      },
      None,
      BoundedVec::try_from(vec![step]).unwrap(),
    );
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let ActorSemanticState::Active(_record) = ActorSemanticStates::<Test>::get(actor_id).unwrap()
    else {
      panic!("created Actor is active");
    };
    let sovereign = Actors::actor_identity(actor_id).unwrap().sovereign_account;
    ActorControlLocators::<Test>::remove(actor_id);
    frame_system::Pallet::<Test>::set_block_number(2);

    let open_resource_block = |now| {
      let mut state = crate::BlockResourceState::new(now);
      state.begin_prepass().unwrap();
      state.open_external_phase().unwrap();
      crate::CurrentBlockResourceState::<Test>::put(state);
    };
    open_resource_block(2);
    assert_ne!(Actors::on_idle(2, Weight::MAX), Weight::zero());
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 0);
    assert!(!ServiceNodes::<Test>::contains_key(actor_id));
    assert_eq!(
      DeadlineHandles::<Test>::get(actor_id).map(|handle| handle.key),
      Some(WakeupKey::Block(5))
    );
    assert_eq!(
      ActorRunStateStore::<Test>::get(actor_id)
        .unwrap()
        .unsuccessful_attempts_at_cursor,
      1
    );
    assert_eq!(
      crate::CurrentBlockResourceState::<Test>::get()
        .unwrap()
        .phase(),
      crate::BlockResourcePhase::Finalizable
    );

    set_asset_balance(&sovereign, TestAsset::Local(1), 10);
    frame_system::Pallet::<Test>::set_block_number(5);
    open_resource_block(5);
    assert_ne!(Actors::on_idle(5, Weight::MAX), Weight::zero());
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 1);
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
    assert!(!DeadlineHandles::<Test>::contains_key(actor_id));

    frame_system::Pallet::<Test>::set_block_number(6);
    open_resource_block(6);
    assert_ne!(Actors::on_idle(6, Weight::MAX), Weight::zero());
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 1);
    assert!(ServiceNodes::<Test>::contains_key(actor_id));
    assert!(!ActorRunStateStore::<Test>::contains_key(actor_id));
    let state = crate::CurrentBlockResourceState::<Test>::get().unwrap();
    assert_eq!(state.phase(), crate::BlockResourcePhase::Finalizable);
    assert_ne!(state.usage(), crate::BlockResourceUsage::default());
    assert_eq!(state.outstanding_reservations(), 0);

    assert_eq!(Actors::on_idle(6, Weight::MAX), Weight::zero());
    assert_eq!(asset_balance(&BOB, TestAsset::Local(1)), 1);
  });
}

#[test]
fn canonical_service_round_matches_the_independent_semantic_trace() {
  new_test_ext().execute_with(|| {
    let members = [actor_ref(120, 1), actor_ref(121, 1), actor_ref(122, 1)];
    for actor in members {
      ActorProcesses::<Test>::insert(
        actor.actor_id,
        serving_process(actor, ServiceResidenceKind::Live),
      );
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        Actors::insert_service_member(actor, ServiceResidenceKind::Live, 0).unwrap();
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
    }

    let mut block = 0;
    for row in include_str!("../../tests/fixtures/current_state_round_trace_v1.tsv").lines() {
      if row.starts_with('#') || row.is_empty() {
        continue;
      }
      let mut fields = row.split('\t');
      let next_block = fields.next().unwrap().parse::<u64>().unwrap();
      let expected = fields.next().unwrap().parse::<u64>().unwrap();
      let action = fields.next().unwrap();
      assert!(fields.next().is_none());
      if next_block != block {
        polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
          Actors::begin_service_round(next_block).unwrap();
          polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
        });
        block = next_block;
      }
      let actor = members[(expected - 120) as usize];
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        assert_eq!(
          Actors::consider_service_head(block),
          Ok(ServiceRoundEncounter::Eligible(actor))
        );
        if action == "admit" {
          Actors::advance_service_head(actor, block).unwrap();
        } else {
          assert!(matches!(action, "refuse_ref_time" | "refuse_proof_size"));
        }
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
    }
  });
}

#[test]
fn canonical_service_ring_appends_partial_round_admission_behind_current_residents() {
  new_test_ext().execute_with(|| {
    let [a, b, c, d] = [120, 121, 122, 123].map(|id| actor_ref(id, 1));
    for actor in [a, b, c] {
      ActorProcesses::<Test>::insert(
        actor.actor_id,
        serving_process(actor, ServiceResidenceKind::Live),
      );
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        Actors::insert_service_member(actor, ServiceResidenceKind::Live, 0).unwrap();
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
    }
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::begin_service_round(1).unwrap();
      assert_eq!(
        Actors::consider_service_head(1),
        Ok(ServiceRoundEncounter::Eligible(a))
      );
      Actors::advance_service_head(a, 1).unwrap();
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    ActorProcesses::<Test>::insert(d.actor_id, serving_process(d, ServiceResidenceKind::Live));
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::insert_service_member(d, ServiceResidenceKind::Live, 1).unwrap();
      Actors::begin_service_round(2).unwrap();
      for expected in [b, c, a, d] {
        assert_eq!(
          Actors::consider_service_head(2),
          Ok(ServiceRoundEncounter::Eligible(expected))
        );
        Actors::advance_service_head(expected, 2).unwrap();
      }
      assert_eq!(
        Actors::consider_service_head(2),
        Ok(ServiceRoundEncounter::Closed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
  });
}

#[test]
fn canonical_service_round_handles_empty_removal_interruption_and_faults() {
  new_test_ext().execute_with(|| {
    assert_eq!(
      Actors::begin_service_round(1),
      Err(ServiceRoundError::TransactionRequired)
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::begin_service_round(1).expect("empty round begins");
      assert_eq!(
        Actors::consider_service_head(1),
        Ok(ServiceRoundEncounter::Empty)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    let first = actor_ref(130, 2);
    let second = actor_ref(131, 2);
    for actor in [first, second] {
      ActorProcesses::<Test>::insert(
        actor.actor_id,
        serving_process(actor, ServiceResidenceKind::Live),
      );
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        Actors::insert_service_member(actor, ServiceResidenceKind::Live, 1).unwrap();
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
      });
    }
    ActorProcesses::<Test>::mutate(first.actor_id, |process| {
      process.as_mut().unwrap().last_attempted = Some(2);
    });
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::begin_service_round(2).unwrap();
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::consider_service_head(2),
        Ok(ServiceRoundEncounter::AlreadyAttempted(first))
      );
      Actors::converge_attempted_service_head(first, 2).unwrap();
      assert_eq!(ServiceHeader::<Test>::get().cursor, Some(second));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(ServiceHeader::<Test>::get().cursor, Some(first));
    assert_eq!(
      ServiceNodes::<Test>::get(first.actor_id)
        .unwrap()
        .last_considered,
      1
    );

    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::converge_attempted_service_head(first, 2).unwrap();
      Actors::remove_service_member(second).expect("cursor removal selects successor");
      assert_eq!(ServiceHeader::<Test>::get().cursor, Some(first));
      assert_eq!(
        Actors::consider_service_head(2),
        Ok(ServiceRoundEncounter::Closed)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });

    ServiceNodes::<Test>::mutate(first.actor_id, |node| {
      let node = node.as_mut().unwrap();
      node.eligible_from = 9;
      node.last_considered = 1;
    });
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::consider_service_head(2),
        Err(ServiceRoundError::FutureMemberUnmarked)
      );
      assert_eq!(ServiceHeader::<Test>::get().cursor, Some(first));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });

    ServiceNodes::<Test>::mutate(first.actor_id, |node| {
      node.as_mut().unwrap().generation = 3;
    });
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::consider_service_head(2),
        Err(ServiceRoundError::StaleGeneration)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });

    ServiceNodes::<Test>::mutate(first.actor_id, |node| {
      let node = node.as_mut().unwrap();
      node.generation = 2;
      node.eligible_from = 2;
    });
    ActorProcesses::<Test>::mutate(first.actor_id, |process| {
      process.as_mut().unwrap().last_attempted = Some(3);
    });
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::consider_service_head(2),
        Err(ServiceRoundError::AttemptFromFuture)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
  });
}

#[test]
fn canonical_service_admission_rejects_overflow_and_defers_replacement() {
  new_test_ext().execute_with(|| {
    let actor = actor_ref(140, 1);
    ActorProcesses::<Test>::insert(
      actor.actor_id,
      serving_process(actor, ServiceResidenceKind::Live),
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      assert_eq!(
        Actors::insert_service_member(actor, ServiceResidenceKind::Live, u64::MAX),
        Err(ServiceRingMutationError::BlockNumberOverflow)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(ServiceHeader::<Test>::get(), ServiceHeaderRecord::default());
    assert!(!ServiceNodes::<Test>::contains_key(actor.actor_id));

    let replacement = actor_ref(actor.actor_id, 2);
    ActorProcesses::<Test>::insert(
      replacement.actor_id,
      serving_process(replacement, ServiceResidenceKind::Live),
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::insert_service_member(replacement, ServiceResidenceKind::Live, 8).unwrap();
      assert_eq!(
        Actors::consider_service_head(8),
        Ok(ServiceRoundEncounter::Closed)
      );
      let node = ServiceNodes::<Test>::get(replacement.actor_id).unwrap();
      assert_eq!(
        (node.generation, node.eligible_from, node.last_considered),
        (2, 9, 8)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
  });
}

fn deadline_process(handle: DeadlineHandle<u64>) -> ActorProcess<u64> {
  ActorProcess {
    generation: handle.actor.generation,
    last_attempted: None,
    status: ProcessStatus::Serving,
    residence: Some(ProcessResidence::Deadline {
      key: handle.key,
      page: handle.page,
      slot: handle.slot,
    }),
  }
}

#[test]
fn canonical_deadline_carrier_covers_fragmentation_full_pages_move_and_rollback() {
  new_test_ext().execute_with(|| {
    let key = WakeupKey::Block(20);
    let first = DeadlineHandle {
      actor: actor_ref(200, 3),
      key,
      page: 0,
      slot: 0,
    };
    ActorProcesses::<Test>::insert(first.actor.actor_id, deadline_process(first));
    assert_eq!(
      Actors::insert_deadline_member(first),
      Err(DeadlineMutationError::TransactionRequired)
    );
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::insert_deadline_member(first),
      )
    })
    .expect("first deadline insertion succeeds");

    for slot in 1..32u8 {
      let handle = DeadlineHandle {
        actor: actor_ref(200 + u64::from(slot), 3),
        key,
        page: 0,
        slot,
      };
      ActorProcesses::<Test>::insert(handle.actor.actor_id, deadline_process(handle));
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::insert_deadline_member(handle),
        )
      })
      .expect("page fill succeeds");
    }
    assert_eq!(
      DeadlinePages::<Test>::get(key, 0)
        .expect("page")
        .live_entries,
      32
    );
    let planned_actor = actor_ref(240, 4);
    assert_eq!(
      Actors::plan_deadline_destination(planned_actor, key),
      Ok(DeadlineHandle {
        actor: planned_actor,
        key,
        page: 1,
        slot: 0,
      })
    );

    let removed = DeadlineHandle {
      actor: actor_ref(207, 3),
      key,
      page: 0,
      slot: 7,
    };
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::remove_deadline_member(removed.actor),
      )
    })
    .expect("interior removal succeeds");
    assert_eq!(
      Actors::plan_deadline_destination(planned_actor, key),
      Ok(DeadlineHandle {
        actor: planned_actor,
        key,
        page: 0,
        slot: 7,
      })
    );
    let replacement = DeadlineHandle {
      actor: planned_actor,
      key,
      page: 0,
      slot: 7,
    };
    ActorProcesses::<Test>::insert(replacement.actor.actor_id, deadline_process(replacement));
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::insert_deadline_member(replacement),
      )
    })
    .expect("fragmented slot is reusable");

    let destination = DeadlineHandle {
      actor: first.actor,
      key: WakeupKey::Tick(30),
      page: 0,
      slot: 4,
    };
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::move_deadline_member(first.actor, destination),
      )
    })
    .expect("cross-bucket move succeeds");
    assert_eq!(
      DeadlineHandles::<Test>::get(first.actor.actor_id),
      Some(destination)
    );
    assert_eq!(
      DeadlineHeaders::<Test>::get(key)
        .expect("source header")
        .count,
      31
    );

    let before = DeadlineHandles::<Test>::get(first.actor.actor_id);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      let invalid = DeadlineHandle {
        actor: first.actor,
        key: WakeupKey::Tick(40),
        page: 9,
        slot: 0,
      };
      assert_eq!(
        Actors::move_deadline_member(first.actor, invalid),
        Err(DeadlineMutationError::InvalidDestination)
      );
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert_eq!(DeadlineHandles::<Test>::get(first.actor.actor_id), before);
  });
}

fn install_indexed_deadline_bucket(key: WakeupKey<u64>, actor_id: u64) -> DeadlineHandle<u64> {
  let handle = DeadlineHandle {
    actor: actor_ref(actor_id, 1),
    key,
    page: 0,
    slot: 0,
  };
  ActorProcesses::<Test>::insert(actor_id, deadline_process(handle));
  polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
    Actors::insert_deadline_member(handle).expect("deadline bucket and index insertion succeeds");
    polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
  });
  handle
}

#[test]
fn canonical_deadline_index_covers_order_pages_stale_state_and_rollback() {
  new_test_ext().execute_with(|| {
    let mut handles = BTreeMap::new();
    for deadline in (1..=65u64).rev() {
      let key = WakeupKey::Block(deadline);
      handles.insert(key, install_indexed_deadline_bucket(key, 1_000 + deadline));
    }
    for deadline in [9, 3, 12] {
      let key = WakeupKey::Tick(deadline);
      handles.insert(key, install_indexed_deadline_bucket(key, 2_000 + deadline));
    }

    assert_eq!(DeadlineIndexLen::<Test>::get(WakeupClock::Block), 65);
    assert_eq!(DeadlineIndexLen::<Test>::get(WakeupClock::Tick), 3);
    assert_eq!(
      DeadlineIndexPages::<Test>::get(WakeupClock::Block, 0)
        .unwrap()
        .len(),
      32
    );
    assert_eq!(
      DeadlineIndexPages::<Test>::get(WakeupClock::Block, 1)
        .unwrap()
        .len(),
      32
    );
    assert_eq!(
      DeadlineIndexPages::<Test>::get(WakeupClock::Block, 2)
        .unwrap()
        .len(),
      1
    );
    assert_eq!(
      DeadlineIndexPages::<Test>::get(WakeupClock::Block, 0).unwrap()[0],
      WakeupKey::Block(1)
    );
    assert_eq!(
      DeadlineIndexPages::<Test>::get(WakeupClock::Tick, 0).unwrap()[0],
      WakeupKey::Tick(3)
    );

    for (key, index) in DeadlineIndexPositions::<Test>::iter() {
      let page = DeadlineIndexPages::<Test>::get(key.clock(), u64::from(index / 32)).unwrap();
      assert_eq!(page[(index % 32) as usize], key);
      if index > 0 {
        let parent = (index - 1) / 2;
        let parent_page =
          DeadlineIndexPages::<Test>::get(key.clock(), u64::from(parent / 32)).unwrap();
        assert!(parent_page[(parent % 32) as usize] <= key);
      }
      polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
        polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
          Actors::update_deadline_index(key),
        )
      })
      .expect("clustered and overdue key remains structurally valid");
    }

    let removed_key = WakeupKey::Block(1);
    let removed = handles[&removed_key];
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::remove_deadline_member(removed.actor)
        .expect("empty bucket removal repairs the deadline index");
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
    assert_eq!(DeadlineIndexLen::<Test>::get(WakeupClock::Block), 64);
    assert_eq!(
      DeadlineIndexPages::<Test>::get(WakeupClock::Block, 0).unwrap()[0],
      WakeupKey::Block(2)
    );
    assert!(!DeadlineIndexPages::<Test>::contains_key(
      WakeupClock::Block,
      2
    ));

    let stale_key = WakeupKey::Block(10);
    let original = DeadlineIndexPositions::<Test>::get(stale_key).unwrap();
    DeadlineIndexPositions::<Test>::insert(stale_key, 999);
    let stale = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::update_deadline_index(stale_key),
      )
    });
    assert_eq!(stale, Err(DeadlineIndexMutationError::StaleIndex));
    DeadlineIndexPositions::<Test>::insert(stale_key, original);

    let rollback_key = WakeupKey::Block(0);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      install_indexed_deadline_bucket(rollback_key, 3_000);
      assert_eq!(DeadlineIndexPositions::<Test>::get(rollback_key), Some(0));
      polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(())
    });
    assert!(!DeadlineHeaders::<Test>::contains_key(rollback_key));
    assert!(!DeadlineIndexPositions::<Test>::contains_key(rollback_key));
    assert_eq!(
      DeadlineIndexPages::<Test>::get(WakeupClock::Block, 0).unwrap()[0],
      WakeupKey::Block(2)
    );
  });
}

#[test]
fn canonical_deadline_index_rejects_missing_headers_legacy_authority_and_early_remove() {
  new_test_ext().execute_with(|| {
    let missing = WakeupKey::Block(40);
    assert_eq!(
      Actors::insert_deadline_index(missing),
      Err(DeadlineIndexMutationError::TransactionRequired)
    );
    let missing_result = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::insert_deadline_index(missing),
      )
    });
    assert_eq!(
      missing_result,
      Err(DeadlineIndexMutationError::HeaderMissing)
    );

    let key = WakeupKey::Block(41);
    let handle = install_indexed_deadline_bucket(key, 4_000);
    let header = DeadlineHeaders::<Test>::get(key).unwrap();
    DeadlineHeaders::<Test>::mutate(key, |stored| stored.as_mut().unwrap().count = 0);
    let corrupt = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::update_deadline_index(key),
      )
    });
    assert_eq!(corrupt, Err(DeadlineIndexMutationError::CorruptHeader));
    DeadlineHeaders::<Test>::insert(key, header);

    let early_remove = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::remove_deadline_index(key),
      )
    });
    assert_eq!(early_remove, Err(DeadlineIndexMutationError::CorruptHeader));

    ActorWaitingOccupancies::<Test>::insert(key, 1);
    let legacy = polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(
        Actors::remove_deadline_index(key),
      )
    });
    assert_eq!(
      legacy,
      Err(DeadlineIndexMutationError::LegacyAuthorityPresent)
    );
    assert!(DeadlineIndexPositions::<Test>::contains_key(key));
    ActorWaitingOccupancies::<Test>::remove(key);
    polkadot_sdk::frame_support::storage::with_transaction_unchecked(|| {
      Actors::remove_deadline_member(handle.actor).expect("bucket and index remove together");
      polkadot_sdk::frame_support::storage::TransactionOutcome::Commit(())
    });
  });
}

fn test_pipeline_machine_envelope() -> crate::PipelineMachineEnvelope<Balance> {
  crate::PipelineMachineEnvelope {
    pipeline_machine_fee_upper: 11,
    cleanup_fee_upper: 22,
  }
}

fn geometry_certificate(
  contract: &RuntimeActorContract,
) -> crate::ActorAdmissionCertificateOf<Test> {
  crate::ActorAdmissionCertificate::new(
    contract.semantic_contract_id(),
    contract.body_commitment().expect("body commitment"),
    contract.trigger.wake_qualification(&contract.window),
    1,
    [4u8; 32],
    1,
    [6u8; 32],
    Weight::from_parts(77, 88),
  )
}

#[test]
fn actor_identity_resolves_lifecycle_owner_and_fails_closed_on_missing_primary() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 2),
    );
    let identity = Actors::actor_identity(actor_id).expect("active identity exists");
    let primary = crate::ActorUnsignaledControlCells::<Test>::take(actor_id)
      .expect("Manual Actor starts unsignaled");
    assert!(Actors::actor_identity(actor_id).is_none());
    crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, primary);
    assert_eq!(Actors::actor_identity(actor_id), Some(identity.clone()));
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let dormant = Actors::actor_identity(actor_id).expect("dormant identity exists");
    assert_eq!(dormant.sovereign_account, identity.sovereign_account);
    assert_eq!(dormant.owner, identity.owner);
    assert!(Actors::actor_identity(actor_id + 1).is_none());
  });
}

#[test]
fn public_api_error_signatures_use_shared_typed_cores() {
  let _: fn(ActorId) -> Result<ActorEligibility<u32, u64>, ActorClassificationError> =
    Actors::actor_eligibility;
  let _: fn(ActorId) -> Result<crate::ActorCostQuote<Balance>, ActorCostQuoteError> =
    Actors::actor_cost_quote;
  let _: fn(
    ActorId,
    ActorType,
    Mutability,
    RuntimeActorContract,
    SimulationMode,
    crate::SimulationBudget,
  ) -> Result<crate::SimulationResult, SimulationError> = Actors::simulate_current_contract;

  let classification_cases = [
    (
      ActorClassificationError::ActorInvariant,
      Error::<Test>::ActorInvariant,
    ),
    (
      ActorClassificationError::RunInvariant,
      Error::<Test>::ActorRunInvariant,
    ),
    (
      ActorClassificationError::ComputationOverflow,
      Error::<Test>::ComputationOverflow,
    ),
  ];
  for (core, dispatch) in classification_cases {
    assert_eq!(
      Actors::classification_dispatch_error(core).encode(),
      dispatch.encode()
    );
  }
}

#[test]
fn actor_cost_quote_keeps_fee_boundaries_and_state_hold_provenance_separate() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 2),
    );
    let quote = Actors::actor_cost_quote(actor_id).expect("active User cost quote exists");
    assert_eq!(quote.actor_type, ActorType::User);
    assert_eq!(quote.creation_fee, 10);
    let trigger = quote
      .prospective_trigger_fee
      .expect("active Trigger quote exists");
    assert_eq!(trigger.trigger_family, TriggerFamily::Manual);
    assert_eq!(
      trigger.maximum_weight,
      <TestWeightInfo as crate::WeightInfo>::manual_trigger()
    );
    assert_eq!(trigger.fee, manual_trigger_fee());
    assert_ne!(trigger.production_weight_identity, [0; 32]);
    let pipeline = quote
      .prospective_pipeline_fee
      .expect("active Pipeline quote exists");
    assert_eq!(
      pipeline.strategy,
      PipelineMachineFeeStrategy::UpfrontBounded
    );
    assert_eq!(
      pipeline.total_fee,
      pipeline.pipeline_machine_fee + pipeline.cleanup_fee
    );
    assert_eq!(
      pipeline.production_weight_identity,
      crate::AdmissionCertificateAuthority::compose_production_weight_identity([41; 32], [42; 32])
    );
    let loaded =
      Actors::load_current_step_from_storage(actor_id, 0).expect("current Step resources exist");
    assert_eq!(
      quote.maximum_next_action_fee.maximum_effect_weight,
      loaded.resources.effect
    );
    assert_eq!(
      quote.maximum_next_action_fee.maximum_effect_fee,
      TestWeightToFee::weight_to_fee(&loaded.resources.effect)
    );
    let hold = crate::ActorStateHolds::<Test>::get(actor_id)
      .expect("User hold record exists")
      .breakdown;
    let expected_hold_total = [
      hold.identity,
      hold.contract_head,
      hold.contract_body,
      hold.detector,
      hold.run,
    ]
    .into_iter()
    .sum::<Balance>();
    assert_eq!(quote.actor_state_hold.total, expected_hold_total);
    assert!(!quote.actor_state_hold.exempt);
    assert_eq!(quote.actor_state_hold.base_per_component, 1);
    assert_eq!(quote.actor_state_hold.per_encoded_byte, 1);

    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(BOB),
      Mutability::Mutable,
      None,
    ));
    let dormant_id = Actors::next_actor_id() - 1;
    let dormant = Actors::actor_cost_quote(dormant_id).expect("Dormant quote exists");
    assert!(dormant.prospective_trigger_fee.is_none());
    assert!(dormant.prospective_pipeline_fee.is_none());
    assert_eq!(dormant.maximum_next_action_fee.maximum_effect_fee, 0);
    assert!(dormant.actor_state_hold.total > 0);

    let system_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let system = Actors::actor_cost_quote(system_id).expect("System quote exists");
    assert_eq!(system.actor_type, ActorType::System);
    assert_eq!(system.creation_fee, 0);
    assert_eq!(system.prospective_trigger_fee.expect("Trigger").fee, 0);
    assert_eq!(
      system.prospective_pipeline_fee.expect("Pipeline").total_fee,
      0
    );
    assert_eq!(system.maximum_next_action_fee.maximum_effect_fee, 0);
    assert!(system.actor_state_hold.exempt);
    assert_eq!(system.actor_state_hold.total, 0);

    assert_eq!(
      Actors::actor_cost_quote(u64::MAX),
      Err(ActorCostQuoteError::ActorNotFound)
    );
  });
}

#[test]
fn contract_header_extracts_only_hot_metadata_and_runtime_bindings() {
  let contract = system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 2))
    .expect("active Contract");
  let semantic_contract_id = [1u8; 32];
  let body_commitment = [2u8; 32];
  let admission_identity = [3u8; 32];

  let header = contract
    .try_header(
      semantic_contract_id,
      body_commitment,
      admission_identity,
      test_pipeline_machine_envelope(),
    )
    .expect("bounded Contract produces a header");

  assert_eq!(header.trigger, contract.trigger);
  assert_eq!(header.cooldown_blocks, contract.cooldown_blocks);
  assert_eq!(header.window, contract.window);
  assert_eq!(header.funding, contract.funding);
  assert_eq!(header.completion, contract.completion);
  assert_eq!(
    header.auto_close_at_cycle_nonce,
    contract.auto_close_at_cycle_nonce
  );
  assert_eq!(
    header.step_count,
    u32::try_from(contract.steps.len()).expect("bounded Step count fits u32")
  );
  assert_eq!(header.semantic_contract_id, semantic_contract_id);
  assert_eq!(header.body_commitment, body_commitment);
  assert_eq!(header.admission_identity, admission_identity);
  assert_eq!(
    header.pipeline_machine_envelope,
    test_pipeline_machine_envelope()
  );
}

#[test]
fn semantic_contract_id_uses_fixed_domain_and_authored_field_order() {
  let contract = system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 2))
    .expect("active Contract");
  assert_eq!(crate::ACTOR_CONTRACT_HASH_DOMAIN, *b"DEOS_ACTOR_CONTRACT");
  let expected = (
    crate::ACTOR_CONTRACT_HASH_DOMAIN,
    (
      &contract.trigger,
      contract.cooldown_blocks,
      &contract.window,
      &contract.funding,
      contract.completion,
      contract.auto_close_at_cycle_nonce,
    ),
    &contract.steps,
  )
    .using_encoded(frame::hashing::blake2_256);
  assert_eq!(contract.semantic_contract_id(), expected);

  let mut changed = contract.clone();
  changed.cooldown_blocks = changed.cooldown_blocks.saturating_add(1);
  assert_ne!(
    contract.semantic_contract_id(),
    changed.semantic_contract_id()
  );
}

#[test]
fn body_commitment_uses_fixed_domain_and_exact_ordered_indexes() {
  let steps = BoundedVec::try_from(vec![
    make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    }),
    make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(2),
    }),
  ])
  .expect("two Steps fit");
  let contract = system_active_contract(manual_schedule(), None, steps).expect("active Contract");
  assert_eq!(crate::ACTOR_BODY_HASH_DOMAIN, *b"DEOS_ACTOR_BODY");
  let indexed_steps = vec![(0u32, &contract.steps[0]), (1u32, &contract.steps[1])];
  let expected = (crate::ACTOR_BODY_HASH_DOMAIN, indexed_steps.as_slice())
    .using_encoded(frame::hashing::blake2_256);
  assert_eq!(contract.body_commitment(), Some(expected));

  let mut reordered = contract.clone();
  reordered.steps =
    BoundedVec::try_from(vec![contract.steps[1].clone(), contract.steps[0].clone()])
      .expect("reordered Steps fit");
  assert_ne!(contract.body_commitment(), reordered.body_commitment());
}

#[test]
fn current_step_service_state_does_not_load_unreached_tail_chunks() {
  new_test_ext().execute_with(|| {
    let steps = BoundedVec::try_from(
      (0..8)
        .map(|_| make_step(Task::StopCycle))
        .collect::<Vec<_>>(),
    )
    .expect("eight Steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    ActorContractTailChunks::<Test>::remove(actor_id, 0);

    let (state, admission, loaded_step) =
      Actors::load_current_step_service_state(actor_id).expect("Step 0 needs no tail chunk");
    assert_eq!(state.hot.cycle_state, CycleState::Idle);
    assert_eq!(state.contract.steps.len(), 8);
    assert_eq!(loaded_step.cursor, 0);
    assert_eq!(
      loaded_step.resources,
      ActorContractHeads::<Test>::get(actor_id)
        .expect("canonical head exists")
        .first_step_resources
        .expect("nonempty Contract has inline Step resources"),
      "the loaded Step keeps exact fragment authority"
    );
    assert!(admission.has_valid_identity());
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
    System::reset_events();
    Actors::on_idle(1, Weight::MAX);
    assert!(Actors::actor_hot(actor_id).is_some_and(|hot| !hot.pending_signal));
    assert!(Actors::actor_run_state(actor_id).is_none());
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleStarted { actor_id: id, .. } if *id == actor_id
    )));
    assert!(has_actor_event(|event| matches!(
      event,
      Event::CycleSummary { actor_id: id, result: CycleResult::Completed, outcomes, .. }
        if *id == actor_id && outcomes.executed_steps == 1 && outcomes.committed_effectful_tasks == 0
    )));
    assert!(!ActorContractTailChunks::<Test>::contains_key(actor_id, 0));
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
  });
}

#[test]
fn run_head_and_immutable_payload_remain_coherent_across_progress() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let step = inert_contract_steps()[0].clone();
    let steps =
      BoundedVec::try_from(vec![step.clone(), step.clone(), step]).expect("three Steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    Actors::on_idle(1, Weight::MAX);
    let first_head = crate::ActorRunHeads::<Test>::get(actor_id).expect("run head exists");
    let first_payload = crate::ActorRunPayloads::<Test>::get(actor_id).expect("run payload exists");

    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    Actors::execute_cycle(Weight::MAX);
    let second_head = crate::ActorRunHeads::<Test>::get(actor_id).expect("run head persists");
    let second_payload =
      crate::ActorRunPayloads::<Test>::get(actor_id).expect("run payload persists");
    assert_eq!(second_head.cursor, 2);
    assert_eq!(
      second_head.payload_commitment,
      first_head.payload_commitment
    );
    assert_eq!(second_payload.encode(), first_payload.encode());

    crate::ActorRunHeads::<Test>::mutate(actor_id, |maybe| {
      maybe.as_mut().expect("run head exists").payload_commitment[0] ^= 1;
    });
    assert!(ActorRunStateStore::<Test>::get(actor_id).is_none());
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
  });
}

#[test]
fn current_fragment_resources_are_authoritative_without_certificate_duplication() {
  new_test_ext().execute_with(|| {
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    ActorContractHeads::<Test>::mutate(actor_id, |maybe| {
      let head = maybe.as_mut().expect("canonical head exists");
      let resources = head
        .first_step_resources
        .as_mut()
        .expect("nonempty Contract has inline Step resources");
      resources.control = resources.control.saturating_add(Weight::from_parts(1, 0));
    });
    let (_, _, loaded) = Actors::load_current_step_service_state(actor_id)
      .expect("current fragment remains authoritative");
    assert_eq!(
      loaded.resources.control,
      ActorContractHeads::<Test>::get(actor_id)
        .expect("canonical head exists")
        .first_step_resources
        .expect("nonempty Contract has inline Step resources")
        .control
    );
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Active(_)
    ));
  });
}

#[test]
fn running_execution_and_post_placement_ignore_unreached_tail_chunks() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let inert = inert_contract_steps()[0].clone();
    let steps = BoundedVec::try_from((0..8).map(|_| inert.clone()).collect::<Vec<_>>())
      .expect("eight Steps fit");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, steps);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    Actors::on_idle(1, Weight::MAX);
    assert_eq!(
      Actors::actor_run_state(actor_id).map(|run| run.cursor),
      Some(1)
    );

    ActorContractTailChunks::<Test>::remove(actor_id, 1);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
    frame_system::Pallet::<Test>::set_block_number(2);
    Actors::on_initialize(2);
    assert!(Actors::actor_hot(actor_id).is_some_and(|hot| hot.queue_ticket.is_some()));
    let (_, queued) = Actors::paged_head_entry().expect("successor is queued");
    assert_eq!(queued.actor_id, actor_id);
    assert_eq!(queued.eligible_at, 2);
    assert!(Actors::load_current_step_service_state(actor_id).is_some());
    Actors::execute_cycle(Weight::MAX);
    assert!(has_actor_event(|event| matches!(
      event,
      Event::StepSkipped { actor_id: id, step_index: 1, .. } if *id == actor_id
    )));
    let run = Actors::actor_run_state(actor_id).expect("Running suffix remains live");
    assert_eq!(run.cursor, 2);
    assert_eq!(run.last_committed_step_block, Some(2));
    assert!(Actors::actor_hot(actor_id).is_some_and(|hot| {
      hot.cycle_state == CycleState::Running
        && (hot.queue_ticket.is_some() || hot.wakeup_pointer.is_some())
    }));
  });
}

#[test]
fn current_step_plan_builds_only_from_coherent_opening_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    crate::ActorReadyHead::<Test>::put(9);
    crate::ActorReadyTail::<Test>::put(9);
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(ALICE),
      actor_id
    ));
    let identity = Actors::actor_identity(actor_id).expect("identity exists");
    let hot = Actors::actor_hot(actor_id).expect("hot state exists");
    assert_eq!(hot.queue_ticket, Some(9));
    let admission = Actors::actor_control_cell(actor_id)
      .map(|(_, cell)| cell.admission)
      .expect("canonical admission certificate exists");
    let head = ActorContractHeads::<Test>::get(actor_id).expect("canonical head exists");
    let loaded_step = Actors::load_current_step_from_geometry(actor_id, &head, &admission, 0, None)
      .expect("Step 0 loads");
    let ticket = crate::ActorStepTicket {
      actor_id,
      cycle_nonce: identity.cycle_nonce + 1,
      cursor: 0,
      ticket: 9,
      eligible_at: 1,
      contract_commitment: crate::ActorContractCommitment {
        semantic_contract_id: admission.semantic_contract_id,
        body_commitment: admission.body_commitment,
      },
    };
    assert_eq!(
      Actors::build_actor_step_ticket(actor_id, 9, 1, &identity, &hot, None, &admission,),
      Some(ticket)
    );
    let mut stale_hot = hot.clone();
    stale_hot.queue_ticket = Some(10);
    assert!(
      Actors::build_actor_step_ticket(actor_id, 9, 1, &identity, &stale_hot, None, &admission,)
        .is_none()
    );
    let user_fee = Actors::maximum_current_step_fee(ActorType::User, loaded_step.resources)
      .expect("User current-Step fee is representable");
    assert_eq!(
      user_fee.total_fee,
      user_fee.control_fee + user_fee.effect_fee
    );
    let maximum_fee = Actors::maximum_current_step_fee(ActorType::System, loaded_step.resources)
      .expect("System current-Step fee is representable");
    assert_eq!(maximum_fee.total_fee, 0);
    let storage_plan = Actors::load_current_step_plan_from_storage(ticket)
      .expect("storage-backed Opening plan builds");
    assert_eq!(storage_plan.ticket.actor_id, ticket.actor_id);
    assert_eq!(storage_plan.ticket.cycle_nonce, ticket.cycle_nonce);
    assert_eq!(storage_plan.ticket.cursor, ticket.cursor);
    assert_eq!(storage_plan.ticket.eligible_at, ticket.eligible_at);
    assert_eq!(
      storage_plan.ticket.contract_commitment,
      ticket.contract_commitment
    );
    assert_eq!(storage_plan.loaded_step, loaded_step);
    assert_eq!(storage_plan.maximum_fee, maximum_fee);
    let mut future_ticket = ticket;
    future_ticket.eligible_at = 2;
    assert!(Actors::load_current_step_plan_from_storage(future_ticket).is_none());
    let plan = Actors::build_current_step_plan(
      actor_id,
      identity.clone(),
      hot.clone(),
      None,
      admission.clone(),
      ticket,
      loaded_step.clone(),
      maximum_fee.clone(),
    )
    .expect("coherent Opening plan builds");
    assert_eq!(plan.loaded_step, loaded_step);
    assert_eq!(plan.maximum_fee, maximum_fee);

    let mut running_hot = plan.hot.clone();
    running_hot.cycle_state = CycleState::Running;
    let running = RuntimeActorRunState {
      contract_authority: run_contract_authority(actor_id),
      cycle_nonce: plan.ticket.cycle_nonce,
      cursor: plan.ticket.cursor,
      unsuccessful_attempts_at_cursor: 0,
      last_attempt_block: 0,
      last_committed_step_block: Some(0),
      eligible_at: plan.ticket.eligible_at,
      opening_snapshot: Default::default(),
      cumulative_outcomes: Default::default(),
      last_step_outcome: None,
      suspension: None,
    };
    let running_ticket = Actors::build_actor_step_ticket(
      actor_id,
      9,
      running.eligible_at,
      &plan.identity,
      &running_hot,
      Some(&running),
      &plan.admission,
    )
    .expect("coherent legacy Running ticket builds");
    assert!(
      Actors::build_actor_step_ticket(
        actor_id,
        9,
        running.eligible_at + 1,
        &plan.identity,
        &running_hot,
        Some(&running),
        &plan.admission,
      )
      .is_none()
    );
    assert!(
      Actors::build_current_step_plan(
        actor_id,
        plan.identity.clone(),
        running_hot.clone(),
        Some(running.clone()),
        plan.admission.clone(),
        running_ticket,
        plan.loaded_step.clone(),
        plan.maximum_fee.clone(),
      )
      .is_some()
    );
    let mut stale_run = running.clone();
    stale_run.contract_authority.body_commitment[0] ^= 1;
    assert!(
      Actors::build_actor_step_ticket(
        actor_id,
        9,
        stale_run.eligible_at,
        &plan.identity,
        &running_hot,
        Some(&stale_run),
        &plan.admission,
      )
      .is_none()
    );
    assert!(
      Actors::build_current_step_plan(
        actor_id,
        plan.identity.clone(),
        running_hot.clone(),
        Some(stale_run),
        plan.admission.clone(),
        running_ticket,
        plan.loaded_step.clone(),
        plan.maximum_fee.clone(),
      )
      .is_none()
    );
    let mut suspended_hot = running_hot.clone();
    suspended_hot.cycle_state = CycleState::Suspended;
    let mut suspended = running.clone();
    suspended.last_step_outcome = Some(StepOutcome::FundingUnavailable);
    suspended.suspension = Some(SuspensionReason::FundingUnavailable);
    assert!(
      Actors::build_current_step_plan(
        actor_id,
        plan.identity.clone(),
        suspended_hot,
        Some(suspended),
        plan.admission.clone(),
        running_ticket,
        plan.loaded_step.clone(),
        plan.maximum_fee.clone(),
      )
      .is_some()
    );
    let mut incoherent_suspension = running;
    incoherent_suspension.suspension = Some(SuspensionReason::Temporary);
    assert!(
      Actors::build_current_step_plan(
        actor_id,
        plan.identity.clone(),
        running_hot,
        Some(incoherent_suspension),
        plan.admission.clone(),
        running_ticket,
        plan.loaded_step.clone(),
        plan.maximum_fee.clone(),
      )
      .is_none()
    );

    let mut stale_ticket = ticket;
    stale_ticket.cycle_nonce += 1;
    assert!(
      Actors::build_current_step_plan(
        actor_id,
        identity,
        hot,
        None,
        admission,
        stale_ticket,
        plan.loaded_step,
        plan.maximum_fee,
      )
      .is_none()
    );
  });
}

#[test]
fn step_ticket_binds_run_cursor_fifo_eligibility_and_contract_commitment() {
  let ticket = crate::ActorStepTicket {
    actor_id: 7,
    cycle_nonce: 8,
    cursor: 2,
    ticket: 9,
    eligible_at: 10u64,
    contract_commitment: crate::ActorContractCommitment {
      semantic_contract_id: [1u8; 32],
      body_commitment: [2u8; 32],
    },
  };
  let decoded =
    crate::ActorStepTicket::decode(&mut ticket.encode().as_slice()).expect("Step ticket decodes");
  assert_eq!(decoded, ticket);
  assert_eq!(decoded.ticket, 9);
  assert_eq!(decoded.cursor, 2);
  assert_eq!(decoded.eligible_at, 10);
  assert!(decoded.matches(
    7,
    8,
    2,
    9,
    &10,
    &crate::ActorContractCommitment {
      semantic_contract_id: [1u8; 32],
      body_commitment: [2u8; 32],
    },
  ));
  assert!(!decoded.matches(7, 8, 3, 9, &10, &decoded.contract_commitment,));
}

#[test]
fn admission_identity_binds_every_runtime_owned_domain_field() {
  let qualification = manual_schedule()
    .trigger
    .wake_qualification(&None::<crate::ScheduleWindow<MockBlockNumber>>);
  let certificate: crate::ActorAdmissionCertificateOf<Test> = crate::ActorAdmissionCertificate::new(
    [1u8; 32],
    [2u8; 32],
    qualification,
    3,
    [4u8; 32],
    5,
    [6u8; 32],
    Weight::from_parts(77, 88),
  );
  assert!(certificate.has_valid_identity());
  assert!(certificate.authorizes_wake(qualification));
  let mut wrong_family = qualification;
  wrong_family.family = crate::TriggerFamily::AddressEvent;
  assert!(!certificate.authorizes_wake(wrong_family));
  let mut wrong_selector = qualification;
  wrong_selector.selector_commitment[0] ^= 1;
  assert!(!certificate.authorizes_wake(wrong_selector));
  let mut stale = certificate.clone();
  stale.wake_qualification = wrong_selector;
  assert!(!stale.has_valid_identity());
  let mut stale = certificate.clone();
  stale.body_geometry_version = 6;
  assert!(!stale.has_valid_identity());
  let mut stale = certificate.clone();
  stale.production_weight_identity[0] ^= 1;
  assert!(!stale.has_valid_identity());
  let mut stale = certificate;
  stale.maximum_lifecycle_weight = Weight::from_parts(78, 88);
  assert!(!stale.has_valid_identity());
}

#[test]
fn admission_certificate_encoding_is_independent_of_resource_ceiling() {
  type SmallResources = BoundedVec<crate::ActorStepResourceEnvelope, ConstU32<1>>;
  type LargeResources = BoundedVec<crate::ActorStepResourceEnvelope, ConstU32<32>>;
  let qualification = manual_schedule()
    .trigger
    .wake_qualification(&None::<crate::ScheduleWindow<MockBlockNumber>>);
  let small: crate::ActorAdmissionCertificate<SmallResources> =
    crate::ActorAdmissionCertificate::new(
      [1u8; 32],
      [2u8; 32],
      qualification,
      3,
      [4u8; 32],
      5,
      [6u8; 32],
      Weight::from_parts(77, 88),
    );
  let large: crate::ActorAdmissionCertificate<LargeResources> =
    crate::ActorAdmissionCertificate::new(
      [1u8; 32],
      [2u8; 32],
      qualification,
      3,
      [4u8; 32],
      5,
      [6u8; 32],
      Weight::from_parts(77, 88),
    );
  assert_eq!(small.encode(), large.encode());
  assert_eq!(small.admission_identity, large.admission_identity);
}

#[test]
fn admission_certificate_builder_composes_compact_host_authority() {
  new_test_ext().execute_with(|| {
    let contract = system_active_contract(
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![make_step(Task::StopCycle), make_step(Task::StopCycle)])
        .expect("two Steps fit"),
    )
    .expect("active Contract");
    let certificate =
      Actors::build_admission_certificate(&contract).expect("host authority exists");
    assert!(certificate.has_valid_identity());
    assert_eq!(
      certificate.semantic_contract_id,
      contract.semantic_contract_id()
    );
    assert_eq!(
      certificate.body_commitment,
      contract.body_commitment().expect("body commitment")
    );
    assert_eq!(certificate.runtime_actor_semantics_version, 1);
    assert_eq!(
      certificate.production_weight_identity,
      crate::AdmissionCertificateAuthority::compose_production_weight_identity([41; 32], [42; 32])
    );
    assert_eq!(certificate.body_geometry_version, 1);
    assert_eq!(certificate.configured_bounds_commitment, [6; 32]);
    assert_eq!(
      certificate.maximum_lifecycle_weight,
      Weight::from_parts(77, 88)
    );
    assert_eq!(
      certificate.marker,
      std::marker::PhantomData::<crate::ActorAdmissionResourcesOf<Test>>
    );
  });
}

#[test]
fn production_contract_load_requires_its_certified_wake_qualification() {
  new_test_ext().execute_with(|| {
    let contract = system_active_contract(
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits"),
    )
    .expect("active Contract");
    let matching = Actors::build_admission_certificate(&contract).expect("host authority exists");
    assert!(Actors::admission_authorizes_contract_wake(
      &matching, &contract
    ));

    let temporal_contract = system_active_contract(
      at_time_schedule(10),
      None,
      BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits"),
    )
    .expect("temporal Contract");
    assert!(
      !Actors::admission_authorizes_contract_wake(&matching, &temporal_contract),
      "a valid Manual certificate cannot authorize a temporal production Contract load",
    );

    let windowed_contract = system_active_contract(
      manual_schedule(),
      Some(crate::ScheduleWindow { start: 2, end: 20 }),
      BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits"),
    )
    .expect("windowed Contract");
    assert!(
      !Actors::admission_authorizes_contract_wake(&matching, &windowed_contract),
      "a valid unwindowed certificate cannot authorize different schedule boundaries",
    );

    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    crate::ActorUnsignaledControlCells::<Test>::mutate(actor_id, |stored| {
      let cell = stored.as_mut().expect("Unsignaled authority exists");
      let old = &cell.admission;
      let replacement = crate::ActorAdmissionCertificate::new(
        old.semantic_contract_id,
        old.body_commitment,
        temporal_contract
          .trigger
          .wake_qualification(&temporal_contract.window),
        old.runtime_actor_semantics_version,
        old.production_weight_identity,
        old.body_geometry_version,
        old.configured_bounds_commitment,
        old.maximum_lifecycle_weight,
      );
      cell.pipeline_service_identity =
        crate::pipeline_service_identity(replacement.admission_identity);
      cell.admission = replacement;
    });
    let replacement_identity = crate::ActorUnsignaledControlCells::<Test>::get(actor_id)
      .expect("mutated authority exists")
      .admission
      .admission_identity;
    crate::ActorContractHeads::<Test>::mutate(actor_id, |stored| {
      stored
        .as_mut()
        .expect("Contract head exists")
        .header
        .admission_identity = replacement_identity;
    });
    assert!(matches!(
      Actors::load_actor_state_for_frame_control(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
  });
}

#[test]
fn step_resource_derivation_binds_current_predicate_and_amount_geometry() {
  let steps = BoundedVec::try_from(vec![StepOf::<Test> {
    precondition: all_conditions(vec![Predicate::BalanceAbove {
      asset: TestAsset::Native,
      threshold: 1,
    }]),
    task: Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Percent(Perbill::from_percent(50)),
    },
    on_error: StepErrorPolicy::AbortCycle,
  }])
  .expect("one Step fits");
  let contract = system_active_contract(manual_schedule(), None, steps).expect("active Contract");
  let resources = Actors::derive_step_resource_envelopes(&contract)
    .expect("configured resource providers admit Contract");
  assert_eq!(resources.len(), 1);
  assert_eq!(
    resources[0].control,
    Weight::from_parts(100_000_012, 100_023),
    "control base plus current predicate and amount evaluation units",
  );
}

#[test]
fn opening_control_envelope_charges_only_authored_tail_chunks() {
  let one_step = system_active_contract(
    manual_schedule(),
    None,
    BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits"),
  )
  .expect("one-Step Contract exists");
  let eight_steps = system_active_contract(
    manual_schedule(),
    None,
    BoundedVec::try_from(
      (0..8)
        .map(|_| make_step(Task::StopCycle))
        .collect::<Vec<_>>(),
    )
    .expect("eight Steps fit"),
  )
  .expect("eight-Step Contract exists");

  let one =
    Actors::derive_step_resource_envelopes(&one_step).expect("one-Step resources exist")[0].control;
  let eight = Actors::derive_step_resource_envelopes(&eight_steps)
    .expect("eight-Step resources exist")[0]
    .control;

  assert_eq!(eight.ref_time().saturating_sub(one.ref_time()), 2);
  assert_eq!(eight.proof_size(), one.proof_size());
}

#[test]
fn unconfigured_resource_weight_ports_fail_closed() {
  assert_eq!(
    <() as crate::AdmissionCertificateAuthorityProvider>::current(),
    None
  );
  let step = make_step(Task::StopCycle);
  assert_eq!(
    <() as crate::StepControlWeightProvider<RuntimeStep>>::production_weight_identity(),
    None
  );
  assert_eq!(
    <() as crate::StepControlWeightProvider<_>>::maximum_control_weight(
      crate::StepControlWeightContext {
        cursor: 0,
        steps_in_fragment: 1,
        opening_tail_chunks: 0,
        predicate_evaluation_units: 0,
        opening_snapshot_entries: 0,
      },
      &step,
    ),
    None
  );
  assert_eq!(
    <() as crate::StepControlWeightProvider<RuntimeStep>>::actual_control_weight(
      crate::StepControlWeightContext {
        cursor: 0,
        steps_in_fragment: 1,
        opening_tail_chunks: 0,
        predicate_evaluation_units: 0,
        opening_snapshot_entries: 0,
      },
      &step,
      Weight::from_parts(1, 1),
      crate::StepControlExecution {
        task_effect: crate::TaskEffectExecution::NotInvoked,
        action_fee_collected: false,
        phase: crate::StepControlPhase::Opening,
        outcome: crate::StepControlOutcome::Completed,
        placement: crate::StepControlPlacement::None,
      },
    ),
    None
  );
  let task =
    Task::<TestAsset, Balance, AccountId, <Test as crate::Config>::MaxSplitTransferLegs>::StopCycle;
  assert_eq!(
    <() as crate::TaskEffectWeightProvider<RuntimeTask>>::production_weight_identity(),
    None
  );
  assert_eq!(
    <() as crate::TaskEffectWeightProvider<_>>::maximum_effect_weight(&task),
    None
  );
  assert_eq!(
    <() as crate::TaskEffectWeightProvider<_>>::actual_effect_weight(
      &task,
      crate::TaskEffectExecution::NotInvoked,
    ),
    None
  );
}

#[test]
fn step_control_weight_context_matches_c6_head_and_tail_geometry() {
  assert_eq!(
    Actors::step_control_weight_context(1, 0, 7, 8),
    Some(crate::StepControlWeightContext {
      cursor: 0,
      steps_in_fragment: 1,
      opening_tail_chunks: 0,
      predicate_evaluation_units: 7,
      opening_snapshot_entries: 8,
    })
  );
  assert_eq!(
    Actors::step_control_weight_context(12, 0, 7, 8)
      .expect("twelve-Step head context exists")
      .opening_tail_chunks,
    3,
  );
  for (cursor, steps_in_fragment) in [(1, 4), (4, 4), (5, 4), (8, 4), (9, 3), (11, 3)] {
    assert_eq!(
      Actors::step_control_weight_context(12, cursor, 7, 8),
      Some(crate::StepControlWeightContext {
        cursor,
        steps_in_fragment,
        opening_tail_chunks: 0,
        predicate_evaluation_units: 7,
        opening_snapshot_entries: 0,
      })
    );
  }
  assert_eq!(Actors::step_control_weight_context(0, 0, 0, 0), None);
  assert_eq!(Actors::step_control_weight_context(12, 12, 0, 0), None);
  assert_eq!(Actors::step_control_weight_context(13, 0, 0, 0), None);
}

#[test]
fn current_step_resource_admission_requires_both_weight_components() {
  let resources = crate::ActorStepResourceEnvelope {
    control: Weight::from_parts(11, 22),
    effect: Weight::from_parts(33, 44),
  };
  let control = WeightMeter::with_limit(resources.control);
  let effect = WeightMeter::with_limit(resources.effect);
  assert!(Actors::current_step_resources_fit(
    &control, &effect, resources
  ));
  let short_control = WeightMeter::with_limit(Weight::from_parts(10, 22));
  assert!(!Actors::current_step_resources_fit(
    &short_control,
    &effect,
    resources,
  ));
  let short_effect_proof = WeightMeter::with_limit(Weight::from_parts(33, 43));
  assert!(!Actors::current_step_resources_fit(
    &control,
    &short_effect_proof,
    resources,
  ));
}

#[test]
fn step_resource_envelope_keeps_control_and_effect_weight_separate() {
  let envelope = crate::ActorStepResourceEnvelope {
    control: Weight::from_parts(11, 22),
    effect: Weight::from_parts(33, 44),
  };
  assert_eq!(envelope.control, Weight::from_parts(11, 22));
  assert_eq!(envelope.effect, Weight::from_parts(33, 44));
  assert_eq!(
    crate::ActorStepResourceEnvelope::decode(&mut envelope.encode().as_slice()),
    Ok(envelope)
  );
}

#[test]
fn user_pipeline_machine_envelope_prices_control_retries_and_cleanup_only() {
  new_test_ext().execute_with(|| {
    let mut steps = BoundedVec::try_from(vec![
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
    steps[1].on_error = StepErrorPolicy::RetryLater { max_attempts: 3 };
    let contract =
      system_active_contract(manual_schedule(), None, steps.clone()).expect("bounded Contract");
    let resources =
      Actors::derive_step_resource_envelopes(&contract).expect("Step resources derive");
    let first_fee = Actors::maximum_current_step_fee(ActorType::User, resources[0])
      .expect("first Action fee fits");
    let second_fee = Actors::maximum_current_step_fee(ActorType::User, resources[1])
      .expect("second Action fee fits");
    let envelope = Actors::derive_pipeline_machine_envelope(ActorType::User, &steps, &resources)
      .expect("Pipeline Machine envelope fits");

    assert_eq!(
      envelope.pipeline_machine_fee_upper,
      TestWeightToFee::weight_to_fee(&resources[0].control)
        + TestWeightToFee::weight_to_fee(&resources[1].control) * 3
    );
    assert_eq!(first_fee.control_fee, 0);
    assert_eq!(second_fee.control_fee, 0);
    assert!(first_fee.effect_fee > 0);
    assert!(second_fee.effect_fee > 0);
    assert_eq!(
      envelope.cleanup_fee_upper,
      TestWeightToFee::weight_to_fee(&<TestWeightInfo as crate::WeightInfo>::close_actor())
    );
    assert!(
      envelope
        .pipeline_machine_fee_upper
        .checked_add(envelope.cleanup_fee_upper)
        .is_some()
    );
  });
}

#[test]
fn zero_step_pipeline_machine_envelope_prices_generated_control_and_cleanup() {
  new_test_ext().execute_with(|| {
    let steps = crate::ContractSteps::<Test>::default();
    let contract =
      system_active_contract(manual_schedule(), None, steps.clone()).expect("zero-Step Contract");
    let resources =
      Actors::derive_step_resource_envelopes(&contract).expect("empty resources derive");
    let user = Actors::derive_pipeline_machine_envelope(ActorType::User, &steps, &resources)
      .expect("zero-Step User Pipeline Machine envelope fits");
    assert_eq!(
      user.pipeline_machine_fee_upper,
      TestWeightToFee::weight_to_fee(
        &<TestWeightInfo as crate::WeightInfo>::scheduler_inner_zero_step_complete(),
      )
    );
    assert_eq!(
      user.cleanup_fee_upper,
      TestWeightToFee::weight_to_fee(&<TestWeightInfo as crate::WeightInfo>::close_actor())
    );
    assert!(user.pipeline_machine_fee_upper > 0);
    let system = Actors::derive_pipeline_machine_envelope(ActorType::System, &steps, &resources)
      .expect("zero-Step System envelope fits");
    assert_eq!(system.pipeline_machine_fee_upper, 0);
    assert_eq!(system.cleanup_fee_upper, 0);
  });
}

#[test]
fn pipeline_machine_envelope_absorbs_stop_cycle_control_effect() {
  new_test_ext().execute_with(|| {
    let steps = BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits");
    let contract =
      system_active_contract(manual_schedule(), None, steps.clone()).expect("bounded Contract");
    let resources =
      Actors::derive_step_resource_envelopes(&contract).expect("Step resources derive");
    let action_fee = Actors::maximum_current_action_fee(ActorType::User, &steps[0], resources[0])
      .expect("StopCycle Action fee fits");
    let envelope = Actors::derive_pipeline_machine_envelope(ActorType::User, &steps, &resources)
      .expect("Pipeline Machine envelope fits");

    assert_eq!(
      envelope.pipeline_machine_fee_upper,
      TestWeightToFee::weight_to_fee(&resources[0].control.saturating_add(resources[0].effect))
    );
    assert_eq!(action_fee.total_fee, 0);
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_pipeline_machine_envelope_drift() {
  new_test_ext().execute_with(|| {
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    ActorContractHeads::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("C6 head exists")
        .header
        .pipeline_machine_envelope
        .pipeline_machine_fee_upper = 1;
    });

    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_actor_state_hold_geometry_drift() {
  new_test_ext().execute_with(|| {
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    crate::ActorStateHolds::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("User Actor hold exists")
        .breakdown
        .contract_head = 1;
    });
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn step_chunk_binds_authority_and_contiguous_first_index() {
  let chunk = crate::ActorStepChunk {
    authority: crate::ActorBodyAuthority {
      actor_id: 7u64,
      semantic_contract_id: [1u8; 32],
      body_commitment: [2u8; 32],
      admission_identity: [3u8; 32],
    },
    first_step_index: 1,
    steps: vec![11u8, 12u8, 13u8, 14u8],
    step_resources: vec![
      crate::ActorStepResourceEnvelope {
        control: Weight::zero(),
        effect: Weight::zero(),
      };
      4
    ],
  };
  assert!(chunk.matches(&7, &[1u8; 32], &[2u8; 32], &[3u8; 32], 1));
  assert!(!chunk.matches(&8, &[1u8; 32], &[2u8; 32], &[3u8; 32], 1));
  assert!(!chunk.matches(&7, &[1u8; 32], &[2u8; 32], &[3u8; 32], 2));
  assert_eq!(chunk.steps.len(), 4);
}

#[test]
fn contract_geometry_decomposition_is_gap_free_and_head_only_for_one_step() {
  new_test_ext().execute_with(|| {
    for step_count in [1u32, 8] {
      let steps = BoundedVec::try_from(
        (0..step_count)
          .map(|_| make_step(Task::StopCycle))
          .collect::<Vec<_>>(),
      )
      .expect("bounded Steps fit");
      let contract =
        system_active_contract(manual_schedule(), None, steps).expect("active Contract");
      let certificate = crate::ActorAdmissionCertificate::new(
        contract.semantic_contract_id(),
        contract.body_commitment().expect("body commitment"),
        contract.trigger.wake_qualification(&contract.window),
        1,
        [4u8; 32],
        1,
        [6u8; 32],
        Weight::from_parts(77, 88),
      );
      let (head, chunks) =
        Actors::decompose_admitted_contract_geometry(7, ActorType::System, &contract, &certificate)
          .expect("admitted Contract geometry decomposes");
      assert_eq!(
        head.header.admission_identity,
        certificate.admission_identity
      );
      assert_eq!(
        head.header.pipeline_machine_envelope,
        crate::PipelineMachineEnvelope {
          pipeline_machine_fee_upper: 0,
          cleanup_fee_upper: 0,
        }
      );
      let loaded_first = Actors::load_current_step_from_geometry(7, &head, &certificate, 0, None)
        .expect("Step 0 loads from the head only");
      assert_eq!(loaded_first.step, contract.steps[0]);
      let ticket = crate::ActorStepTicket {
        actor_id: 7,
        cycle_nonce: 1,
        cursor: 0,
        ticket: 9,
        eligible_at: 1u64,
        contract_commitment: crate::ActorContractCommitment {
          semantic_contract_id: certificate.semantic_contract_id,
          body_commitment: certificate.body_commitment,
        },
      };
      assert!(Actors::validate_loaded_step_authority(
        7,
        9,
        &certificate,
        &ticket,
        &loaded_first,
      ));
      let mut stale_ticket = ticket;
      stale_ticket.cursor = 1;
      assert!(!Actors::validate_loaded_step_authority(
        7,
        9,
        &certificate,
        &stale_ticket,
        &loaded_first,
      ));
      if step_count > 1 {
        let cursor = step_count - 1;
        let chunk_index = cursor.saturating_sub(1) / 4;
        let chunk = chunks
          .iter(/* deos-bypass: bounded-iter */)
          .find(|(index, _)| *index == chunk_index)
          .expect("current tail chunk exists");
        let loaded = Actors::load_current_step_from_geometry(
          7,
          &head,
          &certificate,
          cursor,
          Some((chunk.0, &chunk.1)),
        )
        .expect("tail Step loads from exactly one chunk");
        assert_eq!(loaded.step, contract.steps[cursor as usize]);
        assert!(
          Actors::load_current_step_from_geometry(
            8,
            &head,
            &certificate,
            cursor,
            Some((chunk.0, &chunk.1)),
          )
          .is_none()
        );
      }
      let mut stale_certificate = certificate.clone();
      stale_certificate.body_geometry_version += 1;
      assert!(
        Actors::decompose_admitted_contract_geometry(
          7,
          ActorType::User,
          &contract,
          &stale_certificate,
        )
        .is_none()
      );
      assert_eq!(head.first_step, Some(contract.steps[0].clone()));
      assert_eq!(
        Actors::reconstruct_contract_geometry(7, head.clone(), &chunks),
        Some(contract.clone())
      );
      if !chunks.is_empty() {
        assert_eq!(
          Actors::reconstruct_contract_geometry(8, head.clone(), &chunks),
          None
        );
        let mut stale_range = chunks.clone();
        stale_range[0].1.first_step_index = 2;
        assert_eq!(
          Actors::reconstruct_contract_geometry(7, head.clone(), &stale_range),
          None
        );
      }
      assert_eq!(
        chunks.len(),
        usize::try_from(step_count.saturating_sub(1).div_ceil(4)).unwrap()
      );
      let reconstructed = head
        .first_step
        .into_iter()
        .chain(
          chunks
            .iter()
            .flat_map(|(_, chunk)| chunk.steps.iter().cloned()),
        )
        .collect::<Vec<_>>();
      assert_eq!(reconstructed.as_slice(), contract.steps.as_slice());
      for (chunk_index, chunk) in chunks {
        assert_eq!(chunk.first_step_index, 1 + chunk_index * 4);
        assert!(!chunk.steps.is_empty());
        assert!(chunk.steps.len() <= 4);
        assert!(chunk.matches(
          &7,
          &head.header.semantic_contract_id,
          &head.header.body_commitment,
          &head.header.admission_identity,
          chunk.first_step_index,
        ));
      }
    }
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn contract_replacement_updates_frame_admission_and_current_step_resources_in_place() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      contract_steps_with_step(make_step(Task::StopCycle)),
    );
    let (_, before) = Actors::load_primary_control_cell(actor_id).expect("initial primary exists");
    let replacement =
      system_active_contract(manual_schedule(), None, transfer_contract_steps(BOB, 7))
        .expect("replacement Contract");

    assert_ok!(Actors::store_actor_contract(actor_id, replacement));

    let (_, after) =
      Actors::load_primary_control_cell(actor_id).expect("replacement primary exists");
    let projected_admission = Actors::actor_control_cell(actor_id)
      .map(|(_, cell)| cell.admission)
      .expect("canonical admission projection exists");
    let loaded_step = Actors::load_current_step_from_storage(actor_id, after.cursor)
      .expect("replacement current Step loads");
    assert_ne!(
      after.admission.admission_identity,
      before.admission.admission_identity
    );
    assert_eq!(after.admission, projected_admission);
    assert_eq!(after.resources, loaded_step.resources);
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn dormant_activation_and_trigger_replacements_preserve_strict_frame_loading() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None,
    ));
    let actor_id = 0;
    frame_system::Pallet::<Test>::set_block_number(2);
    assert_ok!(Actors::activate_actor(
      RuntimeOrigin::root(),
      actor_id,
      system_active_contract(manual_schedule(), None, inert_contract_steps())
        .expect("Manual Contract"),
    ));
    assert!(matches!(
      Actors::load_frame_actor_state(actor_id),
      crate::LoadedActorStateOf::Active(_)
    ));

    frame_system::Pallet::<Test>::set_block_number(3);
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      timer_schedule(2),
      None,
    ));
    assert!(matches!(
      Actors::load_frame_actor_state(actor_id),
      crate::LoadedActorStateOf::Active(_)
    ));

    frame_system::Pallet::<Test>::set_block_number(4);
    assert_ok!(Actors::pause_actor(RuntimeOrigin::root(), actor_id));
    assert!(matches!(
      Actors::load_frame_actor_state(actor_id),
      crate::LoadedActorStateOf::Active(_)
    ));

    frame_system::Pallet::<Test>::set_block_number(5);
    let address_schedule = Schedule {
      trigger: Trigger::address_event(SourceFilter::Any, AssetFilter::Any),
      cooldown_blocks: 0,
    };
    assert_ok!(update_contract_partial!(
      RuntimeOrigin::root(),
      actor_id,
      address_schedule,
      None,
    ));
    assert!(matches!(
      Actors::load_frame_actor_state(actor_id),
      crate::LoadedActorStateOf::Active(_)
    ));
    assert_eq!(Actors::test_activation_plan_kind(actor_id), Ok(3));
  });
}

#[test]
fn admitted_contract_storage_loads_one_current_fragment_and_replaces_exact_tail() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let contract = system_active_contract(
      manual_schedule(),
      None,
      BoundedVec::try_from(
        (0..8)
          .map(|_| make_step(Task::StopCycle))
          .collect::<Vec<_>>(),
      )
      .expect("eight Steps fit"),
    )
    .expect("active Contract");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, contract.steps.clone());
    let certificate = Actors::actor_control_cell(actor_id)
      .expect("live primary owns admission")
      .1
      .admission;
    assert_eq!(
      Actors::remove_admitted_contract_geometry(actor_id),
      Some(contract.clone())
    );
    assert!(Actors::insert_admitted_contract_geometry(
      actor_id,
      &contract,
      &certificate,
    ));
    assert!(!Actors::insert_admitted_contract_geometry(
      actor_id,
      &contract,
      &certificate,
    ));
    assert_eq!(
      Actors::load_admitted_contract_geometry(actor_id),
      Some((contract.clone(), certificate.clone()))
    );
    assert_eq!(
      Actors::load_current_step_from_storage(actor_id, 0)
        .expect("head Step loads")
        .step,
      contract.steps[0]
    );
    assert_eq!(
      Actors::load_current_step_from_storage(actor_id, 7)
        .expect("one tail chunk loads current Step")
        .step,
      contract.steps[7]
    );
    assert!(ActorContractTailChunks::<Test>::contains_key(actor_id, 0));
    assert!(ActorContractTailChunks::<Test>::contains_key(actor_id, 1));

    let replacement = system_active_contract(
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits"),
    )
    .expect("replacement Contract");
    let replacement_certificate =
      Actors::build_admission_certificate(&replacement).expect("replacement host admission");
    assert!(Actors::replace_admitted_contract_geometry(
      actor_id,
      &replacement,
      &replacement_certificate,
    ));
    assert!(!ActorContractTailChunks::<Test>::contains_key(actor_id, 0));
    assert!(!ActorContractTailChunks::<Test>::contains_key(actor_id, 1));
    assert!(Actors::load_current_step_from_storage(actor_id, 1).is_none());
    let primary_before = Actors::actor_control_cell(actor_id).expect("replacement primary");
    assert_eq!(
      Actors::remove_admitted_contract_geometry(actor_id),
      Some(replacement)
    );
    assert!(!ActorContractHeads::<Test>::contains_key(actor_id));
    assert_eq!(Actors::actor_control_cell(actor_id), Some(primary_before));
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Corrupt
    ));
    assert!(Actors::remove_admitted_contract_geometry(actor_id).is_none());
  });
}

#[test]
fn admitted_contract_storage_fails_closed_before_mutation_on_stale_authority() {
  new_test_ext().execute_with(|| {
    let actor_id = 78;
    let contract = system_active_contract(
      manual_schedule(),
      None,
      BoundedVec::try_from(vec![make_step(Task::StopCycle)]).expect("one Step fits"),
    )
    .expect("active Contract");
    let mut stale_certificate = geometry_certificate(&contract);
    stale_certificate.body_geometry_version += 1;
    assert!(!Actors::insert_admitted_contract_geometry(
      actor_id,
      &contract,
      &stale_certificate,
    ));
    assert!(!ActorContractHeads::<Test>::contains_key(actor_id));
    assert!(!Actors::actor_control_cell(actor_id).is_some());
  });
}

#[test]
fn inline_first_head_and_lazy_tail_have_unique_ordered_ownership() {
  let steps = BoundedVec::try_from(vec![
    make_step(Task::Transfer {
      to: BOB,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(1),
    }),
    make_step(Task::Transfer {
      to: CHARLIE,
      asset: TestAsset::Native,
      amount: AmountResolution::Fixed(2),
    }),
  ])
  .expect("two Steps fit");
  let contract = system_active_contract(manual_schedule(), None, steps).expect("active Contract");
  let semantic_contract_id = contract.semantic_contract_id();
  let body_commitment = contract.body_commitment().expect("bounded body commitment");
  let admission_identity = [3u8; 32];
  let header = contract
    .try_header(
      semantic_contract_id,
      body_commitment,
      admission_identity,
      test_pipeline_machine_envelope(),
    )
    .expect("bounded header");
  let resource = crate::ActorStepResourceEnvelope {
    control: Weight::from_parts(11, 22),
    effect: Weight::from_parts(33, 44),
  };
  let head = crate::ActorContractHead {
    header,
    first_step: Some(contract.steps[0].clone()),
    first_step_resources: Some(resource),
  };
  let tail = crate::ActorStepChunk {
    authority: crate::ActorBodyAuthority {
      actor_id: 7u64,
      semantic_contract_id,
      body_commitment,
      admission_identity,
    },
    first_step_index: 1,
    steps: BoundedVec::<_, ConstU32<4>>::try_from(vec![contract.steps[1].clone()])
      .expect("one tail Step fits"),
    step_resources: BoundedVec::<_, ConstU32<4>>::try_from(vec![resource])
      .expect("one tail resource fits"),
  };

  assert_eq!(head.header.step_count, 2);
  assert_eq!(head.first_step, Some(contract.steps[0].clone()));
  assert_eq!(tail.steps[0], contract.steps[1]);
  assert!(tail.matches(
    &7,
    &semantic_contract_id,
    &body_commitment,
    &admission_identity,
    1,
  ));
  assert!(!tail.matches(
    &8,
    &semantic_contract_id,
    &body_commitment,
    &admission_identity,
    1,
  ));
  assert!(!tail.matches(
    &7,
    &semantic_contract_id,
    &body_commitment,
    &admission_identity,
    0,
  ));
  let reconstructed = BoundedVec::<_, <Test as crate::Config>::MaxContractSteps>::try_from(vec![
    head.first_step.expect("nonempty Contract has inline Step"),
    tail.steps[0].clone(),
  ])
  .expect("reconstructed Steps fit");
  assert_eq!(reconstructed, contract.steps);
}

#[test]
#[ignore = "retained-state Contract geometry profile"]
fn profile_contract_geometry_state_footprint() {
  for step_count in [1u32, 8] {
    let steps = BoundedVec::try_from(
      (0..step_count)
        .map(|_| make_step(Task::StopCycle))
        .collect::<Vec<_>>(),
    )
    .expect("profile Steps fit");
    let contract = system_active_contract(manual_schedule(), None, steps).expect("active Contract");
    let semantic_contract_id = contract.semantic_contract_id();
    let body_commitment = contract.body_commitment().expect("bounded body commitment");
    let admission_identity = [3u8; 32];
    let header = contract
      .try_header(
        semantic_contract_id,
        body_commitment,
        admission_identity,
        test_pipeline_machine_envelope(),
      )
      .expect("bounded header");
    let authority = crate::ActorBodyAuthority {
      actor_id: 7u64,
      semantic_contract_id,
      body_commitment,
      admission_identity,
    };
    let resource = crate::ActorStepResourceEnvelope {
      control: Weight::zero(),
      effect: Weight::zero(),
    };
    let head = crate::ActorContractHead {
      header,
      first_step: Some(contract.steps[0].clone()),
      first_step_resources: Some(resource),
    };
    let chunks = contract.steps.as_slice()[1..]
      .chunks(4)
      .enumerate()
      .map(|(chunk_index, steps)| crate::ActorStepChunk {
        authority: authority.clone(),
        first_step_index: 1u32.saturating_add(
          u32::try_from(chunk_index)
            .expect("bounded chunk index fits u32")
            .saturating_mul(4),
        ),
        steps: steps.to_vec(),
        step_resources: vec![resource; steps.len()],
      })
      .collect::<Vec<_>>();
    let b0_bytes = contract.encode().len();
    let head_bytes = head.encode().len();
    let chunk_bytes = chunks
      .iter()
      .map(|chunk| chunk.encode().len())
      .sum::<usize>();
    let max_chunk_bytes = chunks
      .iter()
      .map(|chunk| chunk.encode().len())
      .max()
      .unwrap_or_default();
    println!(
      "CONTRACT_GEOMETRY_FOOTPRINT step_count={step_count} monolithic_bytes={b0_bytes} chunked_total_bytes={} head_bytes={head_bytes} max_chunk_bytes={max_chunk_bytes}",
      head_bytes.saturating_add(chunk_bytes),
    );
  }
}

#[test]
fn removed_amount_resolution_scale_forms_are_rejected() {
  for discriminant in [2_u8, 3, 4] {
    assert!(AmountResolution::<u128>::decode(&mut &[discriminant][..]).is_err());
  }
}

#[test]
fn public_reachability_inventory_is_closed_and_canonical() {
  assert_variant_names::<RuntimeTask>(&[
    "Transfer",
    "SplitTransfer",
    "SwapIn",
    "SwapOut",
    "AddLiquidity",
    "RemoveLiquidity",
    "Burn",
    "Mint",
    "Stake",
    "DonateLiquidity",
    "Unstake",
    "StopCycle",
  ]);
  assert_variant_names::<AmountResolution<u128>>(&["Fixed", "Percent"]);
  assert_variant_names::<InputLimit<u128>>(&["LiveQuote", "Absolute"]);
  assert_variant_names::<Predicate<TestAsset, u128, u32, u32>>(&[
    "BalanceAbove",
    "BalanceBelow",
    "BalanceEquals",
    "BalanceNotEquals",
    "BlockNumberAbove",
    "BlockNumberBelow",
    "ObservationAbove",
    "ObservationBelow",
    "ObservationEquals",
    "ObservationNotEquals",
  ]);
  assert_variant_names::<crate::PredicateError>(&["InvalidObservation"]);
  assert_variant_names::<RuntimeSourceFilter>(&["Any", "OwnerOnly", "Whitelist"]);
  assert_variant_names::<RuntimeAssetFilter>(&["Any", "Whitelist"]);
  assert_variant_names::<RuntimeTrigger>(&[
    "Manual",
    "AddressEvent",
    "ObservationChange",
    "ObservationCrossing",
    "AtTime",
    "Cadenced",
  ]);
  assert_variant_names::<Trigger<AccountId, TestAsset, <Test as crate::Config>::MaxWhitelistSize>>(
    &[
      "Manual",
      "AddressEvent",
      "ObservationChange",
      "ObservationCrossing",
      "AtTime",
      "Cadenced",
    ],
  );
  assert_variant_names::<ActorType>(&["User", "System"]);
  assert_variant_names::<ActorClass>(&["User", "System"]);
  assert_variant_names::<Mutability>(&["Mutable", "Immutable"]);
  assert_variant_names::<crate::CompletionPolicy>(&["Persistent", "CloseAfterProductiveCycle"]);
  assert_variant_names::<ActiveLifecycle>(&["Active", "Paused"]);
  assert_variant_names::<CycleState>(&["Idle", "Running", "Suspended"]);
  assert_variant_names::<AttemptDisposition>(&[
    "Completed",
    "Continued",
    "Failed",
    "Suspended",
    "Closed",
  ]);
  assert_variant_names::<StepOutcome>(&[
    "Executed",
    "Stopped",
    "Skipped",
    "FundingUnavailable",
    "Failed",
  ]);
  assert_variant_names::<OpeningSurface<TestAsset>>(&[
    "PreservableAsset",
    "TargetAsset",
    "StakingShares",
  ]);
  assert_variant_names::<CloseReason>(&[
    "OwnerInitiated",
    "CycleAdmissionInsufficient",
    "TriggerAdmissionInsufficient",
    "ConsecutiveFailures",
    "WindowExpired",
    "CycleNonceExhausted",
    "AutoCloseNonceReached",
    "RetryAttemptsExhausted",
    "ProductiveCycleCompleted",
    "SchedulerIndexExhausted",
  ]);
  assert_variant_names::<StepErrorPolicy>(&["AbortCycle", "ContinueNextStep", "RetryLater"]);
  assert_variant_names::<SuspensionReason>(&["FundingUnavailable", "Temporary"]);
  assert_variant_names::<CancellationReason>(&[
    "Explicit",
    "ContractReplaced",
    "Deactivated",
    "Closing",
  ]);
  assert_variant_names::<StepSkippedReason>(&[
    "PreconditionFalse",
    "ResolutionSkipped",
    "FundingUnavailable",
  ]);
  assert_variant_names::<FundingSourcePolicy<AccountId, <Test as crate::Config>::MaxWhitelistSize>>(
    &[
      "OwnerOnly",
      "SignedAllowlist",
      "RuntimePolicy",
      "AnyVerifiedIngress",
    ],
  );
  assert_variant_names::<crate::FundingProvenance>(&["Signed", "InternalProtocol", "Xcm"]);
  assert_variant_names::<RetryClass>(&["Permanent", "Temporary"]);
  assert_variant_names::<crate::ScalarObservationState<u64>>(&[
    "Unavailable",
    "Uninitialized",
    "Fresh",
    "Stale",
  ]);
  assert_variant_names::<ActorEligibility<u32, u64>>(&["NotRegistered", "Dormant", "Active"]);
  assert_variant_names::<SimulationMode>(&["FreshCurrentPlan", "CurrentRun"]);
  assert_variant_names::<SimulationError>(&[
    "TransactionDepthExceeded",
    "Classification",
    "ActorNotFound",
    "TypeMismatch",
    "MutabilityMismatch",
    "InvalidContract",
    "InvalidBudget",
    "ContractMismatch",
    "ModeCycleStateMismatch",
    "GlobalCircuitBreaker",
    "Paused",
    "NotReady",
    "ResourceDeferred",
    "FeeCollectionFailed",
  ]);
}

#[test]
fn actor_storage_schema_is_explicit() {
  let storage_info = Actors::storage_info();
  assert!(
    storage_info
      .iter()
      .all(|entry| entry.pallet_name == b"Actors")
  );
  let metadata = Actors::storage_metadata();
  assert_eq!(metadata.prefix, "Actors");
  let actual_shapes: alloc::vec::Vec<_> = metadata
    .entries
    .iter()
    .map(|entry| {
      let optional = matches!(entry.modifier, StorageEntryModifierIR::Optional);
      let is_blake_map = match &entry.ty {
        StorageEntryTypeIR::Plain(_) => false,
        StorageEntryTypeIR::Map { hashers, .. } => {
          assert!(
            hashers
              .iter()
              .all(|hasher| *hasher == StorageHasherIR::Blake2_128Concat)
          );
          true
        }
      };
      (entry.name, optional, is_blake_map)
    })
    .collect();
  assert_eq!(
    actual_shapes,
    [
      ("NextActorId", false, false),
      ("ActorContractHead", true, true),
      ("ActorActivationAuthority", true, true),
      ("ActorContractTailChunk", true, true),
      ("ActorRunHead", true, true),
      ("ActorRunPayload", true, true),
      ("ActorIdentities", true, true),
      ("ActorSemanticStates", true, true),
      ("ActorProcesses", true, true),
      ("ServiceHeader", false, false),
      ("ServiceNodes", true, true),
      ("DependencySourceAllocatorState", false, false),
      ("ObservationDependencySources", true, true),
      ("DependencySourceObservations", true, true),
      ("DependencyRevisions", false, true),
      ("DependencyScanSourceListState", false, false),
      ("DependencyScanSourceNodes", true, true),
      ("PendingCheckOwners", true, true),
      ("DependencyRegistrationHeaders", false, true),
      ("DependencyRegistrationPages", true, true),
      ("DependencyRegistrationFreePositions", true, true),
      ("DependencyRegistrationPositions", true, true),
      ("DependencyRegistrations", true, true),
      ("DependencyPlans", false, true),
      ("DependencyTimedReviews", true, true),
      ("PendingDependencyEvents", true, true),
      ("PendingDependencyReviews", true, true),
      ("DeadlineHeaders", true, true),
      ("DeadlinePages", true, true),
      ("DeadlineHandles", true, true),
      ("TriggerDeadlineHandles", true, true),
      ("DeadlineIndexPages", true, true),
      ("DeadlineIndexPositions", true, true),
      ("DeadlineIndexLen", false, true),
      ("ActorIdentityCount", false, false),
      ("ActorStateHolds", true, true),
      ("ActiveActorCount", false, false),
      ("SystemSovereigns", true, true),
      ("SystemSovereignCount", false, false),
      ("PrepassExecutionCutoff", true, false),
      ("CurrentBlockResourceState", true, false),
      ("FinalizedBlockResourceTelemetry", true, false),
      ("ActorUnsignaledControlCells", true, true),
      ("ActorReadyFrameChunks", true, true),
      ("ActorWaitingFrameChunks", true, true),
      ("ActorControlLocators", true, true),
      ("ActorReadyHead", false, false),
      ("ActorReadyTail", false, false),
      ("ActorReadyOccupancy", false, false),
      ("ActorWaitingHeads", false, true),
      ("ActorWaitingTails", false, true),
      ("ActorWaitingOccupancies", false, true),
      ("ActorWaitingCursorIndices", true, true),
      ("WakeupCursorPages", true, true),
      ("WakeupCursorLen", false, true),
      ("NextWakeupClock", false, false),
      ("WakeupWorkerFaultState", true, false),
      ("OwnerSlotBitmaps", false, true),
      ("SovereignIndex", true, true),
      ("ActiveActorLimit", false, false),
      ("IndexedTriggerDetectionDisabled", true, true),
      ("ActorObservationFeeds", true, true),
      ("ObservationSubscriptionSlot", true, true),
      ("ObservationSubscriptionSlotOwner", true, true),
      ("NextObservationSubscriptionSlot", false, false),
      ("ObservationFreeSlotLen", false, false),
      ("ObservationFreeSlotPages", true, true),
      ("ObservationSubscriberPages", true, true),
      ("ObservationSubscriberPageLists", true, true),
      ("ObservationSubscriberCount", false, true),
      ("ObservationSubscriptionCount", false, false),
      ("ObservationIngressRevisions", true, true),
      ("DirtyObservationFeeds", true, true),
      ("DirtyObservationListState", false, false),
      ("ObservationFanoutWorkerFaultState", true, false),
      ("CrossingMemberships", true, true),
      ("CrossingMemberPages", true, true),
      ("CrossingLeafStates", true, true),
      ("CrossingRadixNodes", true, true),
      ("CrossingFeedMembershipCount", false, true),
      ("CrossingUserFeedMembershipCount", false, true),
      ("CrossingTransitionQueues", true, true),
      ("CrossingPendingFeeds", true, true),
      ("CrossingPendingFeedListState", false, false),
      ("CrossingRangeCursors", true, true),
      ("CrossingWorkerFaultState", true, false),
      ("MaterializationFamilyCursor", false, false),
      ("GlobalCircuitBreaker", false, false),
      ("IdleStarvationState", false, false),
    ]
  );

  assert_eq!(
    storage_info
      .iter()
      .map(|entry| ::core::str::from_utf8(&entry.storage_name).expect("storage name is UTF-8"))
      .collect::<Vec<_>>(),
    actual_shapes
      .iter()
      .map(|(name, _, _)| *name)
      .collect::<Vec<_>>(),
    "storage info and complete canonical metadata must agree without exclusions"
  );

  let entry = |name: &str| {
    metadata
      .entries
      .iter()
      .find(|entry| entry.name == name)
      .expect("declared storage entry exists")
  };
  assert_map_storage_types::<u64, crate::ActorContractHeadOf<Test>>(entry("ActorContractHead"));
  assert_map_storage_types::<(u64, u32), crate::ActorStepChunkOf<Test>>(entry(
    "ActorContractTailChunk",
  ));
  assert_map_storage_types::<u64, crate::ActorStateHoldRecordOf<Test>>(entry("ActorStateHolds"));
  assert_map_storage_types::<u64, ()>(entry("IndexedTriggerDetectionDisabled"));

  assert_plain_storage_type::<u64>(entry("NextActorId"));
  assert_map_storage_types::<u64, crate::ActorContractHeadOf<Test>>(entry("ActorContractHead"));

  assert_map_storage_types::<u64, crate::ActorActivationAuthorityOf<Test>>(entry(
    "ActorActivationAuthority",
  ));
  assert_map_storage_types::<u64, crate::ActorControlCellOf<Test>>(entry(
    "ActorUnsignaledControlCells",
  ));
  assert_map_storage_types::<u64, crate::ActorControlChunkOf<Test>>(entry("ActorReadyFrameChunks"));
  assert_map_storage_types::<(WakeupKey<MockBlockNumber>, u64), crate::ActorWaitingPageOf<Test>>(
    entry("ActorWaitingFrameChunks"),
  );
  assert_map_storage_types::<u64, crate::ActorControlLocation<MockBlockNumber>>(entry(
    "ActorControlLocators",
  ));
  assert_plain_storage_type::<u64>(entry("ActorReadyHead"));
  assert_plain_storage_type::<u64>(entry("ActorReadyTail"));
  assert_plain_storage_type::<u32>(entry("ActorReadyOccupancy"));
  assert_map_storage_types::<WakeupKey<MockBlockNumber>, u64>(entry("ActorWaitingHeads"));
  assert_map_storage_types::<WakeupKey<MockBlockNumber>, u64>(entry("ActorWaitingTails"));
  assert_map_storage_types::<WakeupKey<MockBlockNumber>, u32>(entry("ActorWaitingOccupancies"));
  assert_map_storage_types::<WakeupKey<MockBlockNumber>, u32>(entry("ActorWaitingCursorIndices"));
  assert_plain_storage_type::<(MockBlockNumber, u64)>(entry("PrepassExecutionCutoff"));
  assert_plain_storage_type::<crate::BlockResourceState<MockBlockNumber>>(entry(
    "CurrentBlockResourceState",
  ));
  assert_plain_storage_type::<crate::FinalizedBlockResourceSnapshot<MockBlockNumber>>(entry(
    "FinalizedBlockResourceTelemetry",
  ));

  let mut registry = scale_info::Registry::new();
  let contract_type =
    registry.register_type(&scale_info::meta_type::<crate::ActorContractOf<Test>>());
  let (_, contract) = registry
    .types()
    .find(|(symbol, _)| symbol.id == contract_type.id)
    .expect("Actor Contract type is registered");
  let scale_info::TypeDef::Composite(contract_fields) = &contract.type_def else {
    panic!("Actor Contract metadata must be composite");
  };
  assert_eq!(
    contract_fields
      .fields
      .iter()
      .map(|field| field.name.as_deref().expect("named Actor Contract field"))
      .collect::<Vec<_>>(),
    [
      "trigger",
      "cooldown_blocks",
      "window",
      "steps",
      "funding",
      "completion",
      "auto_close_at_cycle_nonce"
    ]
  );
  assert_map_storage_types::<u64, crate::ActorRunHeadOf<Test>>(entry("ActorRunHead"));
  assert_map_storage_types::<u64, crate::ActorRunPayloadOf<Test>>(entry("ActorRunPayload"));
  let run_type = registry.register_type(&scale_info::meta_type::<RuntimeActorRunState>());
  let (_, run_state) = registry
    .types()
    .find(|(symbol, _)| symbol.id == run_type.id)
    .expect("Actor run type is registered");
  let scale_info::TypeDef::Composite(run_fields) = &run_state.type_def else {
    panic!("Actor run metadata must be composite");
  };
  assert_eq!(
    run_fields
      .fields
      .iter()
      .map(|field| field.name.as_deref().expect("named Actor run field"))
      .collect::<Vec<_>>(),
    [
      "contract_authority",
      "cycle_nonce",
      "cursor",
      "unsuccessful_attempts_at_cursor",
      "last_attempt_block",
      "last_committed_step_block",
      "eligible_at",
      "opening_snapshot",
      "cumulative_outcomes",
      "last_step_outcome",
      "suspension"
    ]
  );
  assert_map_storage_types::<u64, crate::ActorIdentityOf<Test>>(entry("ActorIdentities"));
  assert_plain_storage_type::<DependencySourceAllocator>(entry("DependencySourceAllocatorState"));
  assert_map_storage_types::<u32, u64>(entry("ObservationDependencySources"));
  assert_map_storage_types::<u64, u32>(entry("DependencySourceObservations"));
  assert_plain_storage_type::<u32>(entry("ActorIdentityCount"));
  assert_plain_storage_type::<u32>(entry("ActiveActorCount"));
  assert_map_storage_types::<u64, SystemSovereignState>(entry("SystemSovereigns"));
  assert_plain_storage_type::<u32>(entry("SystemSovereignCount"));
  assert_map_storage_types::<(WakeupClock, u64), crate::WakeupCursorPageOf<Test>>(entry(
    "WakeupCursorPages",
  ));
  assert_map_storage_types::<WakeupClock, u32>(entry("WakeupCursorLen"));
  assert_plain_storage_type::<WakeupClock>(entry("NextWakeupClock"));
  assert_plain_storage_type::<crate::WakeupWorkerFault<MockBlockNumber>>(entry(
    "WakeupWorkerFaultState",
  ));
  assert_map_storage_types::<AccountId, [u8; 32]>(entry("OwnerSlotBitmaps"));
  assert_map_storage_types::<AccountId, u64>(entry("SovereignIndex"));
  assert_plain_storage_type::<u32>(entry("ActiveActorLimit"));
  assert_map_storage_types::<u64, crate::ActorObservationFeedsOf<Test>>(entry(
    "ActorObservationFeeds",
  ));
  assert_map_storage_types::<u64, u32>(entry("ObservationSubscriptionSlot"));
  assert_map_storage_types::<u32, u64>(entry("ObservationSubscriptionSlotOwner"));
  assert_plain_storage_type::<u32>(entry("NextObservationSubscriptionSlot"));
  assert_plain_storage_type::<u32>(entry("ObservationFreeSlotLen"));
  assert_map_storage_types::<u32, crate::ObservationFreeSlotPageOf<Test>>(entry(
    "ObservationFreeSlotPages",
  ));
  assert_map_storage_types::<(u32, u32), crate::ObservationSubscriberPageOf<Test>>(entry(
    "ObservationSubscriberPages",
  ));
  assert_map_storage_types::<u32, ObservationSubscriberPageList>(entry(
    "ObservationSubscriberPageLists",
  ));
  assert_map_storage_types::<u32, u32>(entry("ObservationSubscriberCount"));
  assert_plain_storage_type::<u32>(entry("ObservationSubscriptionCount"));
  assert_map_storage_types::<u32, u64>(entry("ObservationIngressRevisions"));
  assert_map_storage_types::<
    u32,
    crate::types::DirtyObservationState<
      u32,
      polkadot_sdk::frame_system::pallet_prelude::BlockNumberFor<Test>,
    >,
  >(entry("DirtyObservationFeeds"));
  assert_plain_storage_type::<crate::types::DirtyObservationList<u32>>(entry(
    "DirtyObservationListState",
  ));
  assert_plain_storage_type::<crate::ObservationFanoutWorkerFault<u32>>(entry(
    "ObservationFanoutWorkerFaultState",
  ));
  assert_map_storage_types::<u64, crate::CrossingMembershipLocatorOf<Test>>(entry(
    "CrossingMemberships",
  ));
  assert_map_storage_types::<
    (crate::CrossingLeafKeyOf<Test>, u32),
    crate::CrossingMemberPageOf<Test>,
  >(entry("CrossingMemberPages"));
  assert_map_storage_types::<crate::CrossingLeafKeyOf<Test>, crate::CrossingLeafState>(entry(
    "CrossingLeafStates",
  ));
  assert_map_storage_types::<crate::CrossingRadixNodeKeyOf<Test>, u16>(entry("CrossingRadixNodes"));
  assert_map_storage_types::<u32, u32>(entry("CrossingFeedMembershipCount"));
  assert_map_storage_types::<u32, u32>(entry("CrossingUserFeedMembershipCount"));
  assert_map_storage_types::<u32, crate::CrossingTransitionQueueOf<Test>>(entry(
    "CrossingTransitionQueues",
  ));
  assert_map_storage_types::<u32, crate::CrossingPendingFeedState<u32>>(entry(
    "CrossingPendingFeeds",
  ));
  assert_plain_storage_type::<crate::CrossingPendingFeedList<u32>>(entry(
    "CrossingPendingFeedListState",
  ));
  assert_map_storage_types::<u32, crate::CrossingRangeCursor>(entry("CrossingRangeCursors"));
  assert_plain_storage_type::<crate::CrossingWorkerFault<u32>>(entry("CrossingWorkerFaultState"));
  assert_plain_storage_type::<u8>(entry("MaterializationFamilyCursor"));
  assert_plain_storage_type::<bool>(entry("GlobalCircuitBreaker"));
  assert_plain_storage_type::<IdleStarvationPhase>(entry("IdleStarvationState"));
}

#[test]
fn fresh_genesis_baseline_carries_no_migration_ceremony() {
  new_test_ext().execute_with(|| {
    // Genesis writes the current storage version directly; no legacy reader,
    // dual write, queue-merge bridge, or migration cursor exists (COMPAT-STORAGE).
    use polkadot_sdk::frame_support::traits::GetStorageVersion;
    let on_chain =
      polkadot_sdk::frame_support::traits::StorageVersion::get::<crate::Pallet<Test>>();
    let in_code = crate::Pallet::<Test>::in_code_storage_version();
    assert_eq!(
      on_chain, in_code,
      "genesis baseline must equal the current storage version"
    );
    // No queue-merge or migration-cursor storage exists on the schema surface.
    let storage_info = Actors::storage_info();
    let names: alloc::vec::Vec<_> = storage_info
      .iter()
      .map(|entry| ::core::str::from_utf8(&entry.storage_name).expect("UTF-8"))
      .collect();
    assert!(
      names.iter().all(|name| {
        !name.starts_with("Legacy")
          && !name.starts_with("Migration")
          && !name.contains("Merge")
          && !name.contains("OnRuntimeUpgrade")
      }),
      "fresh baseline must not carry migration-ceremony storage: {names:?}"
    );
    // The embedding fixture independently starts from the same fresh schema.
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn owner_slot_bitmap_try_state_rejects_invalid_and_orphaned_bits() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 1),
    );
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    OwnerSlotBitmaps::<Test>::mutate(ALICE, |bitmap| bitmap[31] |= 0b1000_0000);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
    OwnerSlotBitmaps::<Test>::mutate(ALICE, |bitmap| bitmap[31] &= 0b0111_1111);
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    OwnerSlotBitmaps::<Test>::insert(CHARLIE, [1; 32]);
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn live_head_consume_rolls_back_on_occupancy_or_span_corruption() {
  let excessive_span = u64::from(<<Test as crate::Config>::MaxQueueLength as Get<u32>>::get()) + 1;
  for (tail, occupancy) in [(1u64, 0u32), (excessive_span, 1u32)] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      assert!(enqueue_latched_actor(actor_id));
      crate::ActorReadyTail::<Test>::put(tail);
      crate::ActorReadyOccupancy::<Test>::put(occupancy);
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
fn reverse_index_corruption_matrix_fails_closed_for_system_close() {
  for dormant in [false, true] {
    for corruption in 0u8..5 {
      new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(1);
        let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
        let other = create_system_with(BOB, manual_schedule(), None, inert_contract_steps());
        if dormant {
          assert_ok!(Actors::deactivate_actor(RuntimeOrigin::root(), actor_id));
          frame_system::Pallet::<Test>::set_block_number(2);
        }
        let identity = Actors::actor_identity(actor_id).expect("system identity");
        let sovereign_id = match identity.actor_class {
          ActorClass::System { sovereign_id } => sovereign_id,
          _ => unreachable!(),
        };
        match corruption {
          0 => SovereignIndex::<Test>::remove(&identity.sovereign_account),
          1 => SovereignIndex::<Test>::insert(&identity.sovereign_account, other),
          2 => crate::SystemSovereigns::<Test>::remove(sovereign_id),
          3 => crate::SystemSovereigns::<Test>::insert(sovereign_id, SystemSovereignState::Vacant),
          4 => crate::SystemSovereigns::<Test>::insert(
            sovereign_id,
            SystemSovereignState::Occupied(other),
          ),
          _ => unreachable!(),
        }
        let events_before = System::events();
        let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

        assert!(Actors::close_actor(RuntimeOrigin::root(), actor_id).is_err());
        assert_eq!(System::events(), events_before);
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(StateVersion::V1),
          root_before,
          "dormant={dormant}, corruption={corruption}"
        );
        #[cfg(feature = "try-runtime")]
        assert!(
          crate::Pallet::<Test>::do_try_state().is_err(),
          "dormant={dormant}, corruption={corruption}"
        );
      });
    }
  }
}

#[test]
fn reverse_index_corruption_matrix_fails_closed_for_user_close() {
  for corruption in 0u8..3 {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_user_with(
        ALICE,
        Mutability::Mutable,
        manual_schedule(),
        None,
        inert_contract_steps(),
      );
      let other = create_user_with(
        BOB,
        Mutability::Mutable,
        manual_schedule(),
        None,
        inert_contract_steps(),
      );
      let identity = Actors::actor_identity(actor_id).expect("user identity");
      match corruption {
        0 => SovereignIndex::<Test>::remove(&identity.sovereign_account),
        1 => SovereignIndex::<Test>::insert(&identity.sovereign_account, other),
        2 => OwnerSlotBitmaps::<Test>::remove(ALICE),
        _ => unreachable!(),
      }
      let events_before = System::events();
      let root_before = polkadot_sdk::sp_io::storage::root(StateVersion::V1);

      assert!(Actors::close_actor(RuntimeOrigin::signed(ALICE), actor_id).is_err());
      assert_eq!(System::events(), events_before);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(StateVersion::V1),
        root_before,
        "corruption={corruption}"
      );
      #[cfg(feature = "try-runtime")]
      assert!(crate::Pallet::<Test>::do_try_state().is_err());
    });
  }
}

#[test]
fn system_locator_corruption_surfaces_one_invariant_error_on_close() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      None,
      transfer_contract_steps(BOB, 10),
    );
    // Corrupt the locator truth: the live actor's entry no longer points at it.
    crate::SystemSovereigns::<Test>::insert(actor_id, SystemSovereignState::Vacant);
    assert_noop!(
      Actors::close_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::SystemSovereignInvariant
    );
  });
}

#[test]
fn temporal_membership_try_state_rejects_terminal_at_drift() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(
      ALICE,
      manual_schedule(),
      Some(ScheduleWindow { start: 1, end: 101 }),
      inert_contract_steps(),
    );
    assert_eq!(
      Actors::actor_hot(actor_id).expect("hot").terminal_at,
      Some(102)
    );
    #[cfg(feature = "try-runtime")]
    {
      assert_ok!(crate::Pallet::<Test>::do_try_state());
      // Terminal membership is derived from the schedule window: any `terminal_at` that is not
      // the exact window terminal (or absent without a window) must fail try_state.
      mutate_primary_control_cell(actor_id, |cell| cell.hot.terminal_at = Some(999));
      assert_eq!(
        crate::Pallet::<Test>::do_try_state().map_err(|error| format!("{error:?}")),
        Err("Other(\"ActorControl primary cannot restore its semantic identity\")".into())
      );
      mutate_primary_control_cell(actor_id, |cell| cell.hot.terminal_at = None);
      assert!(crate::Pallet::<Test>::do_try_state().is_err());
      mutate_primary_control_cell(actor_id, |cell| cell.hot.terminal_at = Some(102));
      assert_ok!(crate::Pallet::<Test>::do_try_state());
    }
  });
}

#[test]
fn temporal_membership_try_state_rejects_unconsumed_at_time_without_pointer() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, at_time_schedule(10), None, inert_contract_steps());
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());
    Actors::trigger_wakeup_substrate_invalidate_inner(actor_id)
      .expect("AtTime pointer is coherent")
      .expect("AtTime pointer exists");
    #[cfg(feature = "try-runtime")]
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn temporal_membership_try_state_rejects_page_slot_pointing_at_different_actor() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let other = create_system_with(BOB, manual_schedule(), None, inert_contract_steps());
    assert!(schedule_latched_service_wakeup(actor_id, 10));
    assert!(schedule_latched_service_wakeup(other, 10));
    // A physical slot whose entry addresses an actor that owns a different pointer in the
    // same clock domain is corruption.
    crate::ActorWaitingFrameChunks::<Test>::mutate((WakeupKey::Block(10), 0), |maybe| {
      let page = maybe.as_mut().expect("wakeup page");
      let crate::ActorWaitingEntry::Primary(cell) =
        page.entries[0].as_mut().expect("occupied temporal slot")
      else {
        panic!("service wakeup owns the primary");
      };
      cell.actor_id = other;
    });
    #[cfg(feature = "try-runtime")]
    assert_eq!(
      crate::Pallet::<Test>::do_try_state().map_err(|error| format!("{error:?}")),
      Err("Other(\"ActorControl frame topology is corrupt\")".into())
    );
  });
}

#[cfg(not(feature = "runtime-benchmarks"))]
#[test]
fn canonical_loader_rejects_contract_admission_disagreement() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let crate::LoadedActorStateOf::Active(frame_state) =
      Actors::load_actor_state_for_frame_control(actor_id)
    else {
      panic!("canonical frame state must remain active");
    };
    assert!(!frame_state.hot.pending_signal);
    assert_eq!(frame_state.hot.unsuccessful_attempt_streak, 0);
    assert!(matches!(
      frame_state.hot.trigger_runtime_state,
      TriggerRuntimeState::Stateless
    ));
    assert_eq!(frame_state.identity.cycle_nonce, 0);
    let projected = Actors::active_actor_state(actor_id).expect("frame-owned projection is active");
    assert!(!projected.hot.pending_signal);
    assert_eq!(projected.hot.unsuccessful_attempt_streak, 0);
    assert_eq!(projected.identity.cycle_nonce, 0);

    let (service_state, service_admission, loaded_step) =
      Actors::load_frame_actor_service_state(actor_id).expect("frame service state remains live");
    assert_eq!(service_state.identity.cycle_nonce, 0);
    assert!(!service_state.hot.pending_signal);
    assert!(service_admission.has_valid_identity());
    assert_eq!(loaded_step.expect("current Step remains loaded").cursor, 0);
    #[cfg(feature = "try-runtime")]
    assert_ok!(crate::Pallet::<Test>::do_try_state());

    ActorContractHeads::<Test>::mutate(actor_id, |maybe| {
      maybe
        .as_mut()
        .expect("Contract head remains live")
        .header
        .body_commitment[0] ^= 1;
    });
    assert!(matches!(
      Actors::load_frame_actor_state(actor_id),
      crate::LoadedActorStateOf::Corrupt
    ));
    assert!(Actors::active_actor_state(actor_id).is_none());
  });
}

#[cfg(all(feature = "try-runtime", not(feature = "runtime-benchmarks")))]
#[test]
fn try_state_rejects_orphan_primary_without_locator() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    crate::ActorControlLocators::<Test>::remove(actor_id);
    assert!(crate::ActorUnsignaledControlCells::<Test>::contains_key(
      actor_id
    ));
    assert!(!crate::ActorControlLocators::<Test>::contains_key(actor_id));
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn canonical_loader_distinguishes_absence_dormancy_active_and_corruption() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let missing_id = Actors::next_actor_id();
    assert!(matches!(
      Actors::load_actor_state(missing_id),
      LoadedActorStateOf::NotRegistered
    ));

    assert_ok!(Actors::create_system_actor(
      RuntimeOrigin::root(),
      ALICE,
      Mutability::Mutable,
      None,
    ));
    assert!(matches!(
      Actors::load_actor_state(missing_id),
      LoadedActorStateOf::Dormant(_)
    ));

    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Active(_)
    ));
    assert!(!Actors::pending_signal(actor_id));
    ActorContractHeads::<Test>::remove(actor_id);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Corrupt
    ));
    assert!(!Actors::pending_signal(actor_id));
    assert_noop!(
      Actors::write_run_state(actor_id, None),
      Error::<Test>::ActorInvariant
    );
    assert_noop!(
      Actors::pause_actor(RuntimeOrigin::root(), actor_id),
      Error::<Test>::ActorInvariant
    );

    crate::ActorControlLocators::<Test>::remove(actor_id);
    assert!(matches!(
      Actors::load_actor_state(actor_id),
      LoadedActorStateOf::Corrupt
    ));
  });
}

#[test]
fn canonical_loader_classifies_every_primary_partition_presence_mask() {
  for mask in 0u8..16 {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      let semantic =
        crate::ActorSemanticStates::<Test>::take(actor_id).expect("semantic authority fixture");
      let cell =
        crate::ActorUnsignaledControlCells::<Test>::take(actor_id).expect("primary fixture");
      let locator = crate::ActorControlLocators::<Test>::take(actor_id).expect("locator fixture");
      let head = ActorContractHeads::<Test>::take(actor_id).expect("Contract head fixture");
      if mask & 0b0001 != 0 {
        crate::ActorSemanticStates::<Test>::insert(actor_id, semantic);
      }
      if mask & 0b0010 != 0 {
        crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell);
      }
      if mask & 0b0100 != 0 {
        crate::ActorControlLocators::<Test>::insert(actor_id, locator);
      }
      if mask & 0b1000 != 0 {
        ActorContractHeads::<Test>::insert(actor_id, head);
      }
      let loaded = Actors::load_actor_state(actor_id);
      match mask {
        0 => assert!(matches!(loaded, LoadedActorStateOf::NotRegistered)),
        15 => assert!(matches!(loaded, LoadedActorStateOf::Active(_))),
        _ => assert!(
          matches!(loaded, LoadedActorStateOf::Corrupt),
          "mask {mask:04b}"
        ),
      }
    });
  }
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_contract_tail_keys_outside_the_admitted_geometry() {
  for orphan in [false, true] {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let step = transfer_contract_steps(BOB, 1)[0].clone();
      let actor_id = create_system_with(
        ALICE,
        manual_schedule(),
        None,
        BoundedVec::try_from(vec![step.clone(), step]).expect("two Steps fit"),
      );
      assert_ok!(Actors::do_try_state());
      let chunk = ActorContractTailChunks::<Test>::get(actor_id, 0).expect("admitted tail");
      let key = if orphan {
        (actor_id + 100, 0)
      } else {
        (actor_id, 1)
      };
      ActorContractTailChunks::<Test>::insert(key.0, key.1, chunk);
      assert!(
        Actors::do_try_state().is_err(),
        "unowned tail {key:?} was accepted"
      );
      ActorContractTailChunks::<Test>::remove(key.0, key.1);
      assert_ok!(Actors::do_try_state());
    });
  }
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_orphan_activation_authority() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let head = ActorContractHeads::<Test>::get(actor_id).expect("admitted Contract");
    assert_ok!(Actors::do_try_state());
    let orphan_id = actor_id + 100;
    crate::ActorActivationAuthorities::<Test>::insert(
      orphan_id,
      crate::ActorActivationAuthority {
        feed: 1,
        cooldown_blocks: head.header.cooldown_blocks,
        window: head.header.window,
        auto_close_at_cycle_nonce: head.header.auto_close_at_cycle_nonce,
        semantic_contract_id: head.header.semantic_contract_id,
        body_commitment: head.header.body_commitment,
        admission_identity: head.header.admission_identity,
      },
    );
    assert!(
      Actors::do_try_state().is_err(),
      "orphan activation authority was accepted"
    );
    crate::ActorActivationAuthorities::<Test>::remove(orphan_id);
    assert_ok!(Actors::do_try_state());
  });
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_every_incomplete_primary_partition_presence_mask() {
  for mask in 0u8..7 {
    new_test_ext().execute_with(|| {
      frame_system::Pallet::<Test>::set_block_number(1);
      let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
      let cell =
        crate::ActorUnsignaledControlCells::<Test>::take(actor_id).expect("primary fixture");
      let locator = crate::ActorControlLocators::<Test>::take(actor_id).expect("locator fixture");
      let head = ActorContractHeads::<Test>::take(actor_id).expect("Contract head fixture");
      if mask & 0b0001 != 0 {
        crate::ActorUnsignaledControlCells::<Test>::insert(actor_id, cell);
      }
      if mask & 0b0010 != 0 {
        crate::ActorControlLocators::<Test>::insert(actor_id, locator);
      }
      if mask & 0b0100 != 0 {
        ActorContractHeads::<Test>::insert(actor_id, head);
      }
      assert!(
        crate::Pallet::<Test>::do_try_state().is_err(),
        "mask {mask:04b}"
      );
    });
  }
}

#[cfg(feature = "try-runtime")]
#[test]
fn try_state_rejects_system_reverse_index_outside_derived_account() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let original = Actors::actor_identity(actor_id).expect("System identity fixture");
    let replacement_account = 999_998;
    crate::SovereignIndex::<Test>::remove(original.sovereign_account);
    crate::SovereignIndex::<Test>::insert(replacement_account, actor_id);
    assert_eq!(
      Actors::actor_identity(actor_id)
        .expect("identity derives its account")
        .sovereign_account,
      original.sovereign_account
    );
    assert!(crate::Pallet::<Test>::do_try_state().is_err());
  });
}

#[test]
fn eligibility_projection_rejects_partial_active_partitions() {
  new_test_ext().execute_with(|| {
    frame_system::Pallet::<Test>::set_block_number(1);
    let plan = inert_contract_steps();
    let expected_contract =
      system_active_contract(manual_schedule(), None, plan.clone()).expect("direct Actor Contract");
    let actor_id = create_system_with(ALICE, manual_schedule(), None, plan);
    ActorContractHeads::<Test>::remove(actor_id);
    assert_eq!(
      Actors::actor_eligibility(actor_id),
      Err(ActorClassificationError::ActorInvariant)
    );
    assert_eq!(
      Actors::simulate_current_contract(
        actor_id,
        ActorType::System,
        Mutability::Mutable,
        expected_contract,
        SimulationMode::FreshCurrentPlan,
        ample_simulation_budget(),
      ),
      Err(SimulationError::Classification(
        ActorClassificationError::ActorInvariant
      ))
    );
  });
}
