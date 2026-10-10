use super::*;
use crate::{ActorAdmissionCertificateOf, ActorContractOf};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use polkadot_sdk::frame_support::{
  Blake2_128Concat,
  traits::{ConstU32, StorageInfoTrait},
};
use polkadot_sdk::sp_core::Blake2Hasher;
use polkadot_sdk::sp_trie::recorder::Recorder;
use std::collections::BTreeSet;

type Expression = crate::CloseWhen<ConstU32<4>, ConstU32<4>>;

fn stage_behavior_geometry(
  expression: Expression,
) -> (
  ActorId,
  ActorContractOf<Test>,
  ActorAdmissionCertificateOf<Test>,
) {
  stage_behavior_geometry_for_class(expression, ActorType::System)
}

fn stage_behavior_geometry_for_class(
  expression: Expression,
  actor_type: ActorType,
) -> (
  ActorId,
  ActorContractOf<Test>,
  ActorAdmissionCertificateOf<Test>,
) {
  let actor_id = match actor_type {
    ActorType::System => create_system_with(ALICE, manual_schedule(), None, inert_contract_steps()),
    ActorType::User => create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    ),
  };
  let (mut contract, base) = Actors::load_admitted_contract_geometry(actor_id).unwrap();
  Actors::remove_admitted_contract_geometry_with_admission(actor_id, &base).unwrap();
  contract.close_when = Some(expression);
  // Raw geometry certifies source binding only; public policy admission stays unavailable.
  let certificate = ActorAdmissionCertificateOf::<Test>::new(
    contract.semantic_contract_id(),
    contract.body_commitment().unwrap(),
    contract.trigger.wake_qualification(&contract.window),
    base.runtime_actor_semantics_version,
    base.production_weight_identity,
    base.body_geometry_version,
    base.configured_bounds_commitment,
    base.maximum_lifecycle_weight,
  );
  crate::ActorSemanticStates::<Test>::mutate(actor_id, |state| {
    let Some(crate::ActorSemanticState::Active(record)) = state else {
      panic!("active fixture");
    };
    record.admission = certificate.clone();
  });
  assert!(Actors::insert_admitted_contract_geometry(
    actor_id,
    &contract,
    &certificate
  ));
  (actor_id, contract, certificate)
}

#[test]
fn behavior_eligibility_projection_preserves_required_only_state_and_read_only_roots() {
  for (metric, threshold, mode, monitor_required) in [
    (
      crate::BehaviorMetric::ActionSuccessTotal,
      1,
      crate::BehaviorTrackingMode::CurrentOutcome,
      false,
    ),
    (
      crate::BehaviorMetric::ActionSuccessTotal,
      2,
      crate::BehaviorTrackingMode::Accumulated,
      true,
    ),
    (
      crate::BehaviorMetric::CycleCleanTotal,
      1,
      crate::BehaviorTrackingMode::CurrentOutcome,
      true,
    ),
  ] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let (actor_id, _, _) = stage_behavior_geometry(super::behavior_policy::expression(vec![
        vec![crate::BehaviorAtom { metric, threshold }],
      ]));
      let policy = crate::ActorBehaviorPolicies::<Test>::get(actor_id).unwrap();
      let before = polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      let crate::ActorEligibility::Active(view) = eligibility(actor_id) else {
        panic!("Active behavior projection");
      };
      let projection = view.behavior_episode.expect("authored behavior episode");
      assert_eq!(
        projection.episode_started_at_generation,
        policy.episode_started_at_generation
      );
      assert_eq!(projection.tracking_mode, mode);
      assert_eq!(
        projection.requirements.as_slice(),
        &[crate::BehaviorAtom { metric, threshold }]
      );
      assert_eq!(projection.monitor.is_some(), monitor_required);
      if let Some(monitor) = projection.monitor {
        assert!(monitor.clean_cycle.is_none());
        let expected = if mode == crate::BehaviorTrackingMode::Accumulated {
          vec![crate::BehaviorMetricValue { metric, value: 0 }]
        } else {
          vec![]
        };
        assert_eq!(monitor.capped_values.as_slice(), expected.as_slice());
      }
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1),
        before
      );
    });
  }
}

#[test]
fn behavior_eligibility_projection_reports_saturation_without_exact_history() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let (actor_id, contract, certificate) =
      stage_behavior_geometry(super::behavior_policy::expression(vec![vec![
        crate::BehaviorAtom {
          metric: crate::BehaviorMetric::ActionSuccessTotal,
          threshold: 2,
        },
        crate::BehaviorAtom {
          metric: crate::BehaviorMetric::CycleCompletedTotal,
          threshold: 9,
        },
      ]]));
    let policy = crate::ActorBehaviorPolicies::<Test>::get(actor_id).unwrap();
    let bound = policy
      .plan
      .bind(
        &policy.expression,
        crate::BehaviorPolicyBinding {
          semantic_contract_id: contract.semantic_contract_id(),
          admission_identity: certificate.admission_identity,
        },
      )
      .unwrap();
    let episode = (
      b"DEOS_ACTOR_BEHAVIOR_EPISODE",
      actor_id,
      policy.episode_started_at_generation,
      contract.semantic_contract_id(),
    )
      .using_encoded(polkadot_sdk::sp_io::hashing::blake2_256);
    let mut monitor = crate::ActorBehaviorMonitors::<Test>::get(actor_id).unwrap();
    for _ in 0..5 {
      let reduced = bound
        .reduce(
          &monitor.state,
          episode,
          crate::BehaviorObservation {
            precondition: None,
            action: Some(true),
            completion: None,
          },
        )
        .unwrap();
      assert!(!reduced.satisfied);
      monitor.state = reduced.candidate;
    }
    crate::ActorBehaviorMonitors::<Test>::insert(actor_id, monitor);
    let crate::ActorEligibility::Active(view) = eligibility(actor_id) else {
      panic!("Active capped episode");
    };
    assert_eq!(
      view
        .behavior_episode
        .unwrap()
        .monitor
        .unwrap()
        .capped_values
        .as_slice(),
      &[
        crate::BehaviorMetricValue {
          metric: crate::BehaviorMetric::ActionSuccessTotal,
          value: 2
        },
        crate::BehaviorMetricValue {
          metric: crate::BehaviorMetric::CycleCompletedTotal,
          value: 0
        },
      ]
    );
  });
}

#[test]
fn behavior_eligibility_absent_policy_never_reads_optional_companions() {
  let mut ext = new_test_ext();
  let actor_id = ext.execute_with(|| {
    System::set_block_number(1);
    create_system_with(ALICE, manual_schedule(), None, inert_contract_steps())
  });
  ext
    .commit_all()
    .expect("canonical no-policy source commits");
  let recorder = Recorder::<Blake2Hasher>::default();
  ext.execute_with_recorder(recorder.clone(), || {
    let crate::ActorEligibility::Active(view) = eligibility(actor_id) else {
      panic!("Active no-policy Actor");
    };
    assert!(view.behavior_episode.is_none());
  });
  let keys: BTreeSet<Vec<u8>> = recorder
    .recorded_keys()
    .into_values()
    .flat_map(|keys| keys.into_keys().map(|key| key.as_ref().to_vec()))
    .collect();
  for key in [
    crate::ActorBehaviorPolicies::<Test>::hashed_key_for(actor_id),
    crate::ActorBehaviorMonitors::<Test>::hashed_key_for(actor_id),
  ] {
    assert!(!keys.contains(&key));
  }
}

#[test]
fn behavior_eligibility_projection_rejects_foreign_or_missing_required_monitor() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let expression = super::behavior_policy::expression(vec![vec![crate::BehaviorAtom {
      metric: crate::BehaviorMetric::ActionSuccessTotal,
      threshold: 2,
    }]]);
    let (first, _, _) = stage_behavior_geometry(expression.clone());
    let (second, _, _) = stage_behavior_geometry(expression);
    let original = crate::ActorBehaviorMonitors::<Test>::get(second).unwrap();
    crate::ActorBehaviorMonitors::<Test>::insert(
      second,
      crate::ActorBehaviorMonitors::<Test>::get(first).unwrap(),
    );
    assert_eq!(
      Actors::actor_eligibility(second),
      Err(crate::ActorClassificationError::ActorInvariant)
    );
    crate::ActorBehaviorMonitors::<Test>::insert(second, original);
    assert!(Actors::actor_eligibility(second).is_ok());
    crate::ActorBehaviorMonitors::<Test>::remove(second);
    assert_eq!(
      Actors::actor_eligibility(second),
      Err(crate::ActorClassificationError::ActorInvariant)
    );
  });
}

#[test]
fn behavior_policy_geometry_recertification_preserves_episode_and_replacement_resets() {
  for replacement in [false, true] {
    new_test_ext().execute_with(|| {
      System::set_block_number(1);
      let (actor_id, mut contract, old_admission) =
        stage_behavior_geometry(close_when_action_and_normal_cycle());
      let old_policy = crate::ActorBehaviorPolicies::<Test>::get(actor_id).unwrap();
      let episode = (
        b"DEOS_ACTOR_BEHAVIOR_EPISODE",
        actor_id,
        old_policy.episode_started_at_generation,
        contract.semantic_contract_id(),
      )
        .using_encoded(polkadot_sdk::sp_io::hashing::blake2_256);
      let bound = old_policy
        .plan
        .bind(
          &old_policy.expression,
          crate::BehaviorPolicyBinding {
            semantic_contract_id: old_admission.semantic_contract_id,
            admission_identity: old_admission.admission_identity,
          },
        )
        .unwrap();
      let mut monitor = crate::ActorBehaviorMonitors::<Test>::get(actor_id).unwrap();
      monitor.state = bound
        .reduce(
          &monitor.state,
          episode,
          crate::BehaviorObservation {
            action: Some(true),
            ..Default::default()
          },
        )
        .unwrap()
        .candidate;
      crate::ActorBehaviorMonitors::<Test>::insert(actor_id, monitor);
      if replacement {
        contract.cooldown_blocks += 1;
      }
      let mut weight_identity = old_admission.production_weight_identity;
      weight_identity[0] ^= 1;
      let new_admission = ActorAdmissionCertificateOf::<Test>::new(
        contract.semantic_contract_id(),
        contract.body_commitment().unwrap(),
        contract.trigger.wake_qualification(&contract.window),
        old_admission.runtime_actor_semantics_version,
        weight_identity,
        old_admission.body_geometry_version,
        old_admission.configured_bounds_commitment,
        old_admission.maximum_lifecycle_weight,
      );
      let root = || polkadot_sdk::sp_io::storage::root(polkadot_sdk::sp_runtime::StateVersion::V1);
      let before = root();
      let rollback: Result<(), polkadot_sdk::sp_runtime::DispatchError> =
        polkadot_sdk::frame_support::storage::with_transaction(|| {
          assert!(Actors::replace_canonical_contract_geometry(
            actor_id,
            &contract,
            &new_admission
          ));
          polkadot_sdk::frame_support::storage::TransactionOutcome::Rollback(Ok(()))
        });
      assert_ok!(rollback);
      assert_eq!(root(), before);
      assert!(Actors::replace_canonical_contract_geometry(
        actor_id,
        &contract,
        &new_admission
      ));
      assert_eq!(
        Actors::load_contract_geometry_with_admission(actor_id, &new_admission),
        Some(contract.clone())
      );
      let policy = crate::ActorBehaviorPolicies::<Test>::get(actor_id).unwrap();
      let monitor = crate::ActorBehaviorMonitors::<Test>::get(actor_id).unwrap();
      let expected_generation = if replacement { 2 } else { 1 };
      assert_eq!(policy.episode_started_at_generation, expected_generation);
      let new_episode = (
        b"DEOS_ACTOR_BEHAVIOR_EPISODE",
        actor_id,
        expected_generation,
        contract.semantic_contract_id(),
      )
        .using_encoded(polkadot_sdk::sp_io::hashing::blake2_256);
      let bound = policy
        .plan
        .bind(
          &policy.expression,
          crate::BehaviorPolicyBinding {
            semantic_contract_id: new_admission.semantic_contract_id,
            admission_identity: new_admission.admission_identity,
          },
        )
        .unwrap();
      assert_eq!(
        bound.value(
          &monitor.state,
          new_episode,
          crate::BehaviorMetric::ActionSuccessTotal
        ),
        Ok(Some(u32::from(!replacement)))
      );
      assert_ne!(policy.plan.identity(), old_policy.plan.identity());
      assert!(Actors::load_contract_geometry_with_admission(actor_id, &old_admission).is_none());
    });
  }
}

#[test]
fn behavior_policy_monitor_rejects_another_actor_and_future_episode_authority() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let (first, _, _) = stage_behavior_geometry(close_when_action_and_normal_cycle());
    let (second, _, admission) = stage_behavior_geometry(close_when_action_and_normal_cycle());
    let monitor = crate::ActorBehaviorMonitors::<Test>::get(second).unwrap();
    crate::ActorBehaviorMonitors::<Test>::insert(
      second,
      crate::ActorBehaviorMonitors::<Test>::get(first).unwrap(),
    );
    assert!(Actors::load_contract_geometry_with_admission(second, &admission).is_none());
    crate::ActorBehaviorMonitors::<Test>::insert(second, monitor);
    assert!(Actors::load_contract_geometry_with_admission(second, &admission).is_some());
    crate::ActorBehaviorPolicies::<Test>::mutate(second, |policy| {
      policy.as_mut().unwrap().episode_started_at_generation = 2;
    });
    assert!(Actors::load_contract_geometry_with_admission(second, &admission).is_none());
  });
}

#[test]
fn behavior_policy_required_only_monitors_and_holds_use_actual_geometry() {
  for (metric, threshold, has_monitor, clean) in [
    (crate::BehaviorMetric::ActionSuccessTotal, 1, false, false),
    (crate::BehaviorMetric::ActionSuccessTotal, 2, true, false),
    (crate::BehaviorMetric::CycleCleanTotal, 1, true, true),
  ] {
    let mut ext = new_test_ext();
    let (actor_id, contract, certificate) = ext.execute_with(|| {
      System::set_block_number(1);
      let expression =
        super::behavior_policy::expression(vec![vec![crate::BehaviorAtom { metric, threshold }]]);
      let (actor_id, contract, certificate) =
        stage_behavior_geometry_for_class(expression, ActorType::User);
      let before = Actors::actor_state_hold(actor_id).unwrap();
      let policy = crate::ActorBehaviorPolicies::<Test>::get(actor_id).unwrap();
      let control_geometry = Actors::behavior_control_weight_context(&policy).unwrap();
      let monitor = crate::ActorBehaviorMonitors::<Test>::get(actor_id);
      assert_eq!(monitor.is_some(), has_monitor);
      let mut bytes = policy.encoded_size();
      if let Some(monitor) = monitor {
        assert!(monitor.clean_cycle.is_none());
        bytes += monitor.encoded_size();
        if clean {
          bytes += crate::BehaviorCleanCycle::max_encoded_len();
        }
      }
      assert_eq!(crate::CleanCycleCandidate::max_encoded_len(), 2);
      assert_eq!(crate::BehaviorCleanCycle::max_encoded_len(), 10);
      assert_eq!(crate::ActorBehaviorMonitor::max_encoded_len(), 116);
      assert_eq!(
        control_geometry.policy_bytes as usize,
        policy.encoded_size()
      );
      assert_eq!(
        control_geometry.monitor_bytes as usize,
        bytes - policy.encoded_size()
      );
      assert_eq!(control_geometry.counter_slots, u32::from(threshold > 1));
      assert_eq!(control_geometry.requires_clean_cycle, clean);
      assert_ok!(Actors::reconcile_actor_state_hold_with_authority(actor_id));
      let after = Actors::actor_state_hold(actor_id).unwrap();
      let base: Balance = <Test as crate::Config>::ActorStateHoldBase::get();
      let per_byte: Balance = <Test as crate::Config>::ActorStateHoldPerByte::get();
      assert_eq!(before.breakdown.contract_body, 0);
      assert_eq!(
        after.breakdown.contract_head,
        before.breakdown.contract_head
      );
      assert_eq!(after.breakdown.run, before.breakdown.run);
      assert_eq!(
        after.breakdown.contract_body,
        base + bytes as Balance * per_byte
      );
      (actor_id, contract, certificate)
    });
    ext
      .commit_all()
      .expect("staged geometry commits before trie recording");
    let recorder = Recorder::<Blake2Hasher>::default();
    ext.execute_with_recorder(recorder.clone(), || {
      assert_eq!(
        Actors::load_contract_geometry_with_admission(actor_id, &certificate),
        Some(contract.clone())
      );
    });
    let keys: BTreeSet<Vec<u8>> = recorder
      .recorded_keys()
      .into_values()
      .flat_map(|keys| keys.into_keys().map(|key| key.as_ref().to_vec()))
      .collect();
    let mut expected = BTreeSet::from([
      crate::ActorContractHeads::<Test>::hashed_key_for(actor_id),
      crate::ActorBehaviorPolicies::<Test>::hashed_key_for(actor_id),
      crate::ActorSemanticStates::<Test>::hashed_key_for(actor_id),
    ]);
    if has_monitor {
      expected.insert(crate::ActorBehaviorMonitors::<Test>::hashed_key_for(
        actor_id,
      ));
    }
    assert_eq!(keys, expected);
  }
}

#[test]
fn behavior_policy_cold_geometry_authenticates_and_destroys_its_companion() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let (mut contract, base) = Actors::load_admitted_contract_geometry(actor_id).unwrap();
    Actors::remove_admitted_contract_geometry_with_admission(actor_id, &base).unwrap();
    contract.close_when = Some(close_when_action_and_normal_cycle());
    // Raw geometry exercises cold binding with native model prices, without public admission.
    let certificate = crate::ActorAdmissionCertificateOf::<Test>::new(
      contract.semantic_contract_id(),
      contract.body_commitment().unwrap(),
      contract.trigger.wake_qualification(&contract.window),
      base.runtime_actor_semantics_version,
      base.production_weight_identity,
      base.body_geometry_version,
      base.configured_bounds_commitment,
      base.maximum_lifecycle_weight,
    );
    crate::ActorSemanticStates::<Test>::mutate(actor_id, |state| {
      let Some(crate::ActorSemanticState::Active(record)) = state else {
        panic!("active fixture");
      };
      record.admission = certificate.clone();
    });
    assert!(Actors::insert_admitted_contract_geometry(
      actor_id,
      &contract,
      &certificate
    ));
    let policy = crate::ActorBehaviorPolicies::<Test>::get(actor_id).unwrap();
    assert_eq!(policy.episode_started_at_generation, 1);
    let monitor = crate::ActorBehaviorMonitors::<Test>::get(actor_id).unwrap();
    crate::ActorBehaviorMonitors::<Test>::remove(actor_id);
    assert!(Actors::load_contract_geometry_with_admission(actor_id, &certificate).is_none());
    crate::ActorBehaviorMonitors::<Test>::insert(actor_id, monitor);
    assert_eq!(
      Actors::load_contract_geometry_with_admission(actor_id, &certificate),
      Some(contract.clone())
    );
    assert!(Actors::load_current_step_with_admission(actor_id, 0, &certificate).is_some());
    set_missing_behavior_control_weight(true);
    assert!(Actors::load_current_step_with_admission(actor_id, 0, &certificate).is_none());
    set_missing_behavior_control_weight(false);
    assert!(Actors::build_admission_certificate(&contract).is_none());

    crate::ActorBehaviorPolicies::<Test>::remove(actor_id);
    assert!(Actors::load_contract_geometry_with_admission(actor_id, &certificate).is_none());
    let mut forged = policy.clone();
    forged.expression.clauses[0][0].threshold += 1;
    crate::ActorBehaviorPolicies::<Test>::insert(actor_id, forged);
    assert!(Actors::load_contract_geometry_with_admission(actor_id, &certificate).is_none());
    crate::ActorBehaviorPolicies::<Test>::insert(actor_id, policy.clone());
    let mut stale_head = crate::ActorContractHeads::<Test>::get(actor_id).unwrap();
    stale_head.header.admission_identity[0] ^= 1;
    assert!(Actors::reconstruct_contract_geometry(actor_id, stale_head, &[]).is_none());
    assert_eq!(
      Actors::remove_admitted_contract_geometry_with_admission(actor_id, &certificate),
      Some(contract)
    );
    assert!(!crate::ActorBehaviorPolicies::<Test>::contains_key(
      actor_id
    ));
    assert!(!crate::ActorBehaviorMonitors::<Test>::contains_key(
      actor_id
    ));
    assert!(!crate::ActorContractHeads::<Test>::contains_key(actor_id));

    let absent = user_active_contract(manual_schedule(), None, inert_contract_steps()).unwrap();
    let admission = Actors::build_admission_certificate(&absent).unwrap();
    assert!(Actors::insert_admitted_contract_geometry(
      actor_id, &absent, &admission
    ));
    crate::ActorBehaviorPolicies::<Test>::insert(actor_id, policy.clone());
    // An absent selector never consults even a corrupt optional owner; try-state detects orphans.
    assert_eq!(
      Actors::load_contract_geometry_with_admission(actor_id, &admission),
      Some(absent.clone())
    );
    assert_eq!(
      Actors::remove_admitted_contract_geometry_with_admission(actor_id, &admission),
      Some(absent)
    );
    assert_eq!(
      crate::ActorBehaviorPolicies::<Test>::get(actor_id),
      Some(policy)
    );
  });
}

// The existing head is a common byte carrier, not the future authored Contract ABI.
#[test]
fn behavior_policy_absent_cold_reconstruction_never_reads_optional_owner() {
  let mut ext = new_test_ext();
  let (actor_id, certificate, contract) = ext.execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let (contract, certificate) = Actors::load_admitted_contract_geometry(actor_id).unwrap();
    (actor_id, certificate, contract)
  });
  ext
    .commit_all()
    .expect("published Contract commits before trie recording");
  let recorder = Recorder::<Blake2Hasher>::default();
  ext.execute_with_recorder(recorder.clone(), || {
    assert_eq!(
      Actors::load_contract_geometry_with_admission(actor_id, &certificate),
      Some(contract.clone())
    );
  });
  let keys: BTreeSet<Vec<u8>> = recorder
    .recorded_keys()
    .into_values()
    .flat_map(|keys| keys.into_keys().map(|key| key.as_ref().to_vec()))
    .collect();
  let mut expected = BTreeSet::from([crate::ActorContractHeads::<Test>::hashed_key_for(actor_id)]);
  for chunk_index in 0..(contract.steps.len() as u32)
    .saturating_sub(1)
    .div_ceil(crate::MAX_STEPS_PER_TAIL_CHUNK)
  {
    expected.insert(crate::ActorContractTailChunks::<Test>::hashed_key_for(
      actor_id,
      chunk_index,
    ));
  }
  assert_eq!(keys, expected);
  assert!(
    !keys.contains(&crate::ActorBehaviorPolicies::<Test>::hashed_key_for(
      actor_id
    ))
  );
}

// Candidate inline carriers remain comparison evidence, never runtime storage owners.
#[derive(Clone, Decode, DecodeWithMemTracking, Encode, MaxEncodedLen)]
struct InlinePolicyHead {
  head: crate::ActorContractHeadOf<Test>,
  close_when: Option<Expression>,
}

#[derive(Clone, Decode, DecodeWithMemTracking, Encode, MaxEncodedLen)]
struct CompanionPolicyHead {
  head: crate::ActorContractHeadOf<Test>,
  has_close_when: bool,
}

#[derive(Decode, DecodeWithMemTracking, Encode, MaxEncodedLen)]
struct InlineMonitoredRun {
  run: crate::ActorRunStateOf<Test>,
  monitor: Option<crate::BehaviorMonitorState>,
}

#[polkadot_sdk::frame_support::storage_alias]
type InlinePolicyHeads = StorageMap<Actors, Blake2_128Concat, ActorId, InlinePolicyHead>;
#[polkadot_sdk::frame_support::storage_alias]
type CompanionPolicyHeads = StorageMap<Actors, Blake2_128Concat, ActorId, CompanionPolicyHead>;
#[polkadot_sdk::frame_support::storage_alias]
type PolicyCompanions = StorageMap<Actors, Blake2_128Concat, ActorId, Expression>;
#[polkadot_sdk::frame_support::storage_alias]
type MonitorCompanions = StorageMap<Actors, Blake2_128Concat, ActorId, crate::BehaviorMonitorState>;

pub(super) fn maximum_expression() -> Expression {
  use crate::BehaviorMetric::*;
  let clauses = [
    [
      PreconditionPassTotal,
      PreconditionPassStreak,
      PreconditionRejectTotal,
      PreconditionRejectStreak,
    ],
    [
      PreconditionPassTotal,
      ActionSuccessTotal,
      ActionSuccessStreak,
      ActionFailureTotal,
    ],
    [
      PreconditionPassStreak,
      ActionFailureStreak,
      CycleCompletedTotal,
      CycleCleanTotal,
    ],
    [
      PreconditionRejectTotal,
      PreconditionRejectStreak,
      ActionSuccessTotal,
      ActionSuccessStreak,
    ],
  ];
  super::behavior_policy::expression(
    clauses
      .into_iter()
      .map(|clause| {
        clause
          .into_iter()
          .map(|metric| crate::BehaviorAtom {
            metric,
            threshold: u32::MAX,
          })
          .collect()
      })
      .collect(),
  )
}

#[test]
fn behavior_storage_geometry_separates_actual_head_hold_from_declared_head_ceiling() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    let head = crate::ActorContractHeads::<Test>::get(actor_id).unwrap();
    let inline = InlinePolicyHead {
      head: head.clone(),
      close_when: None,
    };
    let companion = CompanionPolicyHead {
      head: head.clone(),
      has_close_when: false,
    };
    assert_eq!(inline.encoded_size(), head.encoded_size() + 1);
    assert_eq!(companion.encoded_size(), inline.encoded_size());
    let inline_ceiling = InlinePolicyHeads::storage_info()[0].max_size.unwrap();
    let companion_ceiling = CompanionPolicyHeads::storage_info()[0].max_size.unwrap();
    assert_eq!(
      inline_ceiling - companion_ceiling,
      Expression::max_encoded_len() as u32
    );
    assert_eq!(Expression::max_encoded_len(), 85);
    assert_eq!(
      CompanionPolicyHead::max_encoded_len(),
      crate::ActorContractHeadOf::<Test>::max_encoded_len() + 1
    );
    let policy = maximum_expression();
    assert!(policy.validate().is_ok());
    assert_eq!(policy.encoded_size(), Expression::max_encoded_len());
    let present = InlinePolicyHead {
      head,
      close_when: Some(policy),
    };
    assert_eq!(present.encoded_size() - inline.encoded_size(), 85);
    eprintln!(
      "head: absent_actual={}, inline_metadata={}, companion_metadata={}, policy_width=85",
      inline.encoded_size(),
      inline_ceiling,
      companion_ceiling
    );
  });
}

#[test]
fn behavior_storage_geometry_inline_monitor_taxes_reserved_run_without_a_policy() {
  new_test_ext().execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_user_with(
      ALICE,
      Mutability::Mutable,
      manual_schedule(),
      None,
      inert_contract_steps(),
    );
    assert!(Actors::actor_run_state(actor_id).is_none());
    let capacity = crate::ActorRunStateOf::<Test>::max_encoded_len();
    let base: Balance = <Test as crate::Config>::ActorStateHoldBase::get();
    let per_byte: Balance = <Test as crate::Config>::ActorStateHoldPerByte::get();
    assert_eq!(
      Actors::actor_state_hold(actor_id).unwrap().breakdown.run,
      base + capacity as Balance * per_byte
    );
    let inline_capacity = InlineMonitoredRun::max_encoded_len();
    assert_eq!(
      inline_capacity - capacity,
      Option::<crate::BehaviorMonitorState>::max_encoded_len()
    );
    assert_eq!(inline_capacity - capacity, 106);
    assert_eq!(
      base + inline_capacity as Balance * per_byte - (base + capacity as Balance * per_byte),
      106 * per_byte
    );
    eprintln!(
      "run: base_capacity={capacity}, inline_capacity={inline_capacity}, no_policy_monitor_tax=106"
    );
  });
}

#[test]
fn behavior_storage_geometry_companion_lookup_records_only_selected_keys() {
  let mut ext = new_test_ext();
  ext.execute_with(|| {
    System::set_block_number(1);
    let actor_id = create_system_with(ALICE, manual_schedule(), None, inert_contract_steps());
    let head = crate::ActorContractHeads::<Test>::get(actor_id).unwrap();
    CompanionPolicyHeads::insert(
      100,
      CompanionPolicyHead {
        head: head.clone(),
        has_close_when: false,
      },
    );
    CompanionPolicyHeads::insert(
      101,
      CompanionPolicyHead {
        head: head.clone(),
        has_close_when: true,
      },
    );
    let policy = maximum_expression();
    let plan = policy
      .validate()
      .unwrap()
      .compile(crate::BehaviorPolicyBinding {
        semantic_contract_id: head.header.semantic_contract_id,
        admission_identity: head.header.admission_identity,
      })
      .unwrap();
    let bound = plan
      .bind(
        &policy,
        crate::BehaviorPolicyBinding {
          semantic_contract_id: head.header.semantic_contract_id,
          admission_identity: head.header.admission_identity,
        },
      )
      .unwrap();
    let monitor = bound
      .begin_episode([7; 32])
      .expect("bounded diagnostic episode");
    assert_eq!(plan.encoded_size(), 84);
    assert_eq!(monitor.encoded_size(), 105);
    PolicyCompanions::insert(101, &policy);
    MonitorCompanions::insert(101, &monitor);
  });
  ext
    .commit_all()
    .expect("candidate carriers commit before trie recording");
  for actor_id in [100, 101] {
    let recorder = Recorder::<Blake2Hasher>::default();
    ext.execute_with_recorder(recorder.clone(), || {
      let head = CompanionPolicyHeads::get(actor_id).unwrap();
      if head.has_close_when {
        assert!(PolicyCompanions::get(actor_id).unwrap().validate().is_ok());
        assert!(MonitorCompanions::get(actor_id).is_some());
      }
    });
    let keys: BTreeSet<Vec<u8>> = recorder
      .recorded_keys()
      .into_values()
      .flat_map(|keys| keys.into_keys().map(|key| key.as_ref().to_vec()))
      .collect();
    let mut expected = BTreeSet::from([CompanionPolicyHeads::hashed_key_for(actor_id)]);
    if actor_id == 101 {
      expected.insert(PolicyCompanions::hashed_key_for(actor_id));
      expected.insert(MonitorCompanions::hashed_key_for(actor_id));
    }
    assert_eq!(keys, expected);
    eprintln!(
      "companion: policy_present={}, recorded_keys={}",
      actor_id == 101,
      keys.len()
    );
  }
}
