use super::{
  behavior_policy::{expression, oracle_metric, oracle_policy},
  behavior_reference as oracle,
};
use crate::{
  BehaviorAtom, BehaviorMetric, BehaviorMonitorState, BehaviorObservation, BehaviorPolicyBinding,
  BehaviorPolicyError,
};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};

fn atom(metric: BehaviorMetric, threshold: u32) -> BehaviorAtom {
  BehaviorAtom { metric, threshold }
}

#[test]
fn clean_cycle_candidate_requires_plan_end_and_preserves_disqualification() {
  use crate::{CleanCycleCandidate, CleanCycleStep as Step, NormalCycleCompletion as End};
  for (trace, expected_clean) in [
    (vec![], false),
    (vec![Step::Neutral, Step::Neutral], false),
    (vec![Step::EconomicSuccess, Step::Neutral], true),
    (vec![Step::Neutral, Step::EconomicSuccess], true),
    (vec![Step::Disqualified, Step::EconomicSuccess], false),
    (
      vec![
        Step::EconomicSuccess,
        Step::Disqualified,
        Step::EconomicSuccess,
      ],
      false,
    ),
  ] {
    let current = trace
      .into_iter()
      .fold(CleanCycleCandidate::default(), |candidate, step| {
        candidate.observe(step)
      });
    assert_eq!(current.completion(None), None);
    assert_eq!(current.completion(Some(End::EarlyReturn)), Some(false));
    assert_eq!(current.completion(Some(End::PlanEnd)), Some(expected_clean));
    let discarded = current.observe(Step::Disqualified);
    assert_eq!(discarded.completion(Some(End::PlanEnd)), Some(false));
    assert_eq!(current.completion(Some(End::PlanEnd)), Some(expected_clean));
  }
}

fn binding() -> BehaviorPolicyBinding {
  BehaviorPolicyBinding {
    semantic_contract_id: [1; 32],
    admission_identity: [2; 32],
  }
}

fn all_metrics(
  threshold: u32,
) -> crate::CloseWhen<
  polkadot_sdk::frame_support::traits::ConstU32<4>,
  polkadot_sdk::frame_support::traits::ConstU32<4>,
> {
  use BehaviorMetric::*;
  expression(vec![
    vec![
      atom(PreconditionPassTotal, threshold),
      atom(PreconditionPassStreak, threshold),
      atom(PreconditionRejectTotal, threshold),
      atom(PreconditionRejectStreak, threshold),
    ],
    vec![
      atom(PreconditionPassTotal, threshold),
      atom(ActionSuccessTotal, threshold),
      atom(ActionSuccessStreak, threshold),
      atom(ActionFailureTotal, threshold),
    ],
    vec![
      atom(PreconditionPassStreak, threshold),
      atom(ActionFailureStreak, threshold),
      atom(CycleCompletedTotal, threshold),
      atom(CycleCleanTotal, threshold),
    ],
    vec![
      atom(PreconditionRejectTotal, threshold),
      atom(PreconditionRejectStreak, threshold),
      atom(ActionSuccessTotal, threshold),
      atom(ActionSuccessStreak, threshold),
    ],
  ])
}

#[test]
fn behavior_state_capped_stream_reducer_matches_independent_exact_history() {
  let policy = all_metrics(3);
  let plan = policy
    .validate()
    .expect("canonical")
    .compile(binding())
    .expect("ten metrics");
  let bound = plan.bind(&policy, binding()).expect("verified plan");
  let expected_policy = oracle_policy(&policy);
  let cases = [
    (
      oracle::Check::Pass,
      oracle::Action::Success,
      Some(true),
      Some(true),
      true,
    ),
    (
      oracle::Check::Pass,
      oracle::Action::Failure,
      Some(true),
      Some(false),
      false,
    ),
    (
      oracle::Check::Reject,
      oracle::Action::NotInvoked,
      Some(false),
      None,
      false,
    ),
    (
      oracle::Check::Error,
      oracle::Action::NotInvoked,
      None,
      None,
      false,
    ),
    (
      oracle::Check::Absent,
      oracle::Action::Success,
      None,
      Some(true),
      true,
    ),
    (
      oracle::Check::Absent,
      oracle::Action::NotInvoked,
      None,
      None,
      false,
    ),
  ];
  let mut compared = 0;
  for first in &cases {
    for second in &cases {
      for third in &cases {
        let mut state = bound.begin_episode([3; 32]).expect("sparse state");
        let mut trace = vec![];
        for (check, action, precondition, action_result, clean) in [first, second, third, third] {
          trace.push(oracle::Record::committed(oracle::Transition::Open));
          trace.push(oracle::Record::committed(oracle::Transition::Attempt {
            check: *check,
            action: *action,
            disposition: oracle::Disposition::Advance,
          }));
          // The fixture is exactly one economic Step, without retry or previous failures.
          // Cycle qualification is supplied independently, not inferred from oracle output.
          let proposed = bound
            .reduce(
              &state,
              [3; 32],
              BehaviorObservation {
                precondition: *precondition,
                action: *action_result,
                completion: Some(*clean),
              },
            )
            .expect("qualified stream results");
          let projection = oracle::project(&[oracle::StepKind::Economic], &trace, None);
          assert_eq!(proposed.satisfied, expected_policy.matches(&projection));
          state = proposed.candidate;
          for cap in plan.metrics() {
            let expected = u32::try_from(projection.value(oracle_metric(cap.metric)))
              .expect("four-Cycle trace")
              .min(cap.threshold);
            assert_eq!(
              bound.value(&state, [3; 32], cap.metric),
              Ok(Some(expected)),
              "{trace:?}, {cap:?}"
            );
            compared += 1;
          }
        }
      }
    }
  }
  assert_eq!(compared, 216 * 4 * 10);
}

#[test]
fn behavior_state_discarded_candidate_rolls_back_without_losing_committed_prefix() {
  let policy = expression(vec![vec![atom(BehaviorMetric::ActionSuccessStreak, 2)]]);
  let plan = policy
    .validate()
    .expect("canonical")
    .compile(binding())
    .expect("one metric");
  let bound = plan.bind(&policy, binding()).expect("verified");
  let initial = bound.begin_episode([3; 32]).expect("state");
  let success = BehaviorObservation {
    action: Some(true),
    ..Default::default()
  };
  let committed = bound
    .reduce(&initial, [3; 32], success)
    .expect("first success");
  assert!(!committed.satisfied);
  let discarded = bound
    .reduce(&committed.candidate, [3; 32], success)
    .expect("candidate reaching threshold");
  assert!(discarded.satisfied);
  assert_eq!(
    bound.value(
      &committed.candidate,
      [3; 32],
      BehaviorMetric::ActionSuccessStreak
    ),
    Ok(Some(1))
  );
  for _ in 0..3 {
    let preserved = bound
      .reduce(&committed.candidate, [3; 32], Default::default())
      .expect("absent observations");
    assert_eq!(preserved.candidate, committed.candidate);
    assert!(!preserved.satisfied);
  }
  let failed = bound
    .reduce(
      &committed.candidate,
      [3; 32],
      BehaviorObservation {
        action: Some(false),
        ..Default::default()
      },
    )
    .expect("committed opposite");
  assert_eq!(
    bound.value(
      &failed.candidate,
      [3; 32],
      BehaviorMetric::ActionSuccessStreak
    ),
    Ok(Some(0))
  );
  assert_eq!(
    bound.value(&initial, [3; 32], BehaviorMetric::ActionSuccessStreak),
    Ok(Some(0))
  );
}

#[test]
fn behavior_state_current_outcome_has_no_counters_and_checks_whole_step() {
  let policy = expression(vec![vec![atom(BehaviorMetric::PreconditionPassTotal, 1)]]);
  let plan = policy
    .validate()
    .expect("canonical")
    .compile(binding())
    .expect("one metric");
  let bound = plan.bind(&policy, binding()).expect("verified");
  let initial = bound.begin_episode([3; 32]).expect("no counters");
  assert_eq!(initial.encoded_size(), 65);
  assert_eq!(
    bound.value(&initial, [3; 32], BehaviorMetric::PreconditionPassTotal),
    Ok(None)
  );
  let rejected = bound
    .reduce(
      &initial,
      [3; 32],
      BehaviorObservation {
        precondition: Some(false),
        ..Default::default()
      },
    )
    .expect("skip");
  assert!(!rejected.satisfied);
  assert_eq!(rejected.candidate, initial);
  let final_result = bound
    .reduce(
      &initial,
      [3; 32],
      BehaviorObservation {
        precondition: Some(true),
        action: Some(false),
        completion: None,
      },
    )
    .expect("passed predicate and failed permitted Action");
  assert!(final_result.satisfied);
  assert_eq!(final_result.candidate, initial);
}

#[test]
fn behavior_state_recertification_preserves_episode_replacement_requires_reset() {
  let policy = expression(vec![vec![atom(BehaviorMetric::ActionSuccessTotal, 20)]]);
  let old_plan = policy
    .validate()
    .expect("canonical")
    .compile(binding())
    .expect("plan");
  let old = old_plan.bind(&policy, binding()).expect("verified");
  let initial = old.begin_episode([3; 32]).expect("state");
  let committed = old
    .reduce(
      &initial,
      [3; 32],
      BehaviorObservation {
        action: Some(true),
        ..Default::default()
      },
    )
    .expect("success")
    .candidate;
  let new_binding = BehaviorPolicyBinding {
    admission_identity: [4; 32],
    ..binding()
  };
  let new_plan = policy
    .validate()
    .expect("same canonical policy")
    .compile(new_binding)
    .expect("new certificate context");
  let new = new_plan.bind(&policy, new_binding).expect("verified");
  assert_eq!(
    new.reduce(&committed, [3; 32], Default::default()),
    Err(BehaviorPolicyError::StateMismatch)
  );
  let rebound = new
    .rebind_episode(&old, &committed, [3; 32])
    .expect("technical re-certification");
  assert_eq!(
    new.value(&rebound, [3; 32], BehaviorMetric::ActionSuccessTotal),
    Ok(Some(1))
  );
  assert_eq!(
    old.rebind_episode(&old, &committed, [3; 32]),
    Ok(committed.clone())
  );
  let replacement_binding = BehaviorPolicyBinding {
    semantic_contract_id: [5; 32],
    ..new_binding
  };
  let replacement_plan = policy
    .validate()
    .expect("same policy on different Contract")
    .compile(replacement_binding)
    .expect("plan");
  let replacement = replacement_plan
    .bind(&policy, replacement_binding)
    .expect("verified");
  assert_eq!(
    replacement.rebind_episode(&new, &rebound, [3; 32]),
    Err(BehaviorPolicyError::ReplacementRequired)
  );
  let reset = replacement
    .begin_episode([6; 32])
    .expect("new semantic episode");
  assert_eq!(
    replacement.value(&reset, [6; 32], BehaviorMetric::ActionSuccessTotal),
    Ok(Some(0))
  );
  assert_eq!(
    replacement.value(&reset, [3; 32], BehaviorMetric::ActionSuccessTotal),
    Err(BehaviorPolicyError::StateMismatch)
  );
}

#[test]
fn behavior_state_rejects_bad_scope_shape_caps_and_impossible_current_results() {
  let policy = expression(vec![vec![atom(BehaviorMetric::ActionSuccessTotal, 20)]]);
  let plan = policy
    .validate()
    .expect("canonical")
    .compile(binding())
    .expect("plan");
  let bound = plan.bind(&policy, binding()).expect("verified");
  let initial = bound.begin_episode([3; 32]).expect("state");
  assert_eq!(initial.encoded_size(), 69);
  let bytes = initial.encode();
  for offset in [0, 32] {
    let mut forged = bytes.clone();
    forged[offset] ^= 1;
    let state = BehaviorMonitorState::decode(&mut &forged[..]).expect("decodable wrong scope");
    assert_eq!(
      bound.reduce(&state, [3; 32], Default::default()),
      Err(BehaviorPolicyError::StateMismatch)
    );
  }
  let mut excessive = bytes.clone();
  excessive[65..69].copy_from_slice(&21_u32.to_le_bytes());
  let state = BehaviorMonitorState::decode(&mut &excessive[..]).expect("decodable excessive count");
  assert_eq!(
    bound.reduce(&state, [3; 32], Default::default()),
    Err(BehaviorPolicyError::StateMismatch)
  );
  let mut missing = bytes[..65].to_vec();
  missing[64] = 0;
  let state = BehaviorMonitorState::decode(&mut &missing[..]).expect("decodable missing slot");
  assert_eq!(
    bound.reduce(&state, [3; 32], Default::default()),
    Err(BehaviorPolicyError::StateMismatch)
  );
  for observation in [
    BehaviorObservation {
      precondition: Some(false),
      action: Some(true),
      completion: None,
    },
    BehaviorObservation {
      precondition: None,
      action: Some(false),
      completion: Some(true),
    },
  ] {
    assert_eq!(
      bound.reduce(&initial, [3; 32], observation),
      Err(BehaviorPolicyError::InvalidObservation)
    );
  }
  assert_eq!(initial.encode(), bytes);
}

#[test]
fn behavior_state_codec_ceiling_and_u32_saturation_are_representation_witnesses() {
  fn requires_memory_tracking<T: DecodeWithMemTracking>() {}
  requires_memory_tracking::<BehaviorMonitorState>();
  let policy = all_metrics(u32::MAX);
  let plan = policy
    .validate()
    .expect("canonical")
    .compile(binding())
    .expect("ten metrics");
  let bound = plan.bind(&policy, binding()).expect("verified");
  let initial = bound.begin_episode([3; 32]).expect("ten zero-valued slots");
  assert_eq!(initial.encoded_size(), 105);
  assert_eq!(
    BehaviorMonitorState::max_encoded_len(),
    initial.encoded_size()
  );
  assert_eq!(
    BehaviorMonitorState::decode(&mut &initial.encode()[..]),
    Ok(initial)
  );
  let policy = expression(vec![vec![atom(
    BehaviorMetric::ActionSuccessStreak,
    u32::MAX,
  )]]);
  let plan = policy
    .validate()
    .expect("canonical")
    .compile(binding())
    .expect("one metric");
  let bound = plan.bind(&policy, binding()).expect("verified");
  let mut bytes = bound.begin_episode([3; 32]).expect("one slot").encode();
  // Restored scalar extreme only: not a reachable lifetime/fee/physical-bound witness.
  bytes[65..69].copy_from_slice(&u32::MAX.to_le_bytes());
  let maximum = BehaviorMonitorState::decode(&mut &bytes[..]).expect("scalar extreme");
  let saturated = bound
    .reduce(
      &maximum,
      [3; 32],
      BehaviorObservation {
        action: Some(true),
        ..Default::default()
      },
    )
    .expect("no overflow");
  assert_eq!(
    bound.value(
      &saturated.candidate,
      [3; 32],
      BehaviorMetric::ActionSuccessStreak
    ),
    Ok(Some(u32::MAX))
  );
  assert!(saturated.satisfied);
  let reset = bound
    .reduce(
      &maximum,
      [3; 32],
      BehaviorObservation {
        action: Some(false),
        ..Default::default()
      },
    )
    .expect("opposite resets even capped maximum");
  assert_eq!(
    bound.value(
      &reset.candidate,
      [3; 32],
      BehaviorMetric::ActionSuccessStreak
    ),
    Ok(Some(0))
  );
}

#[test]
fn behavior_state_normal_and_clean_completions_mature_independently() {
  let policy = expression(vec![vec![
    atom(BehaviorMetric::CycleCompletedTotal, 5),
    atom(BehaviorMetric::CycleCleanTotal, 2),
  ]]);
  let plan = policy
    .validate()
    .expect("canonical")
    .compile(binding())
    .expect("two metrics");
  let bound = plan.bind(&policy, binding()).expect("verified");
  let mut state = bound.begin_episode([3; 32]).expect("two slots");
  for (completion, completed, clean, satisfied) in [
    (Some(true), 1, 1, false),
    (None, 1, 1, false),
    (Some(false), 2, 1, false),
    (Some(true), 3, 2, false),
    (Some(false), 4, 2, false),
    (Some(false), 5, 2, true),
  ] {
    let result = bound
      .reduce(
        &state,
        [3; 32],
        BehaviorObservation {
          completion,
          ..Default::default()
        },
      )
      .expect("qualified completion or no observation");
    assert_eq!(result.satisfied, satisfied);
    state = result.candidate;
    assert_eq!(
      bound.value(&state, [3; 32], BehaviorMetric::CycleCompletedTotal),
      Ok(Some(completed))
    );
    assert_eq!(
      bound.value(&state, [3; 32], BehaviorMetric::CycleCleanTotal),
      Ok(Some(clean))
    );
  }
}

#[test]
fn behavior_state_policy_replacement_with_same_caps_cannot_reuse_an_episode() {
  let conjunction = expression(vec![vec![
    atom(BehaviorMetric::PreconditionPassTotal, 2),
    atom(BehaviorMetric::ActionSuccessTotal, 3),
  ]]);
  let disjunction = expression(vec![
    vec![atom(BehaviorMetric::PreconditionPassTotal, 2)],
    vec![atom(BehaviorMetric::ActionSuccessTotal, 3)],
  ]);
  let old_plan = conjunction
    .validate()
    .expect("canonical AND")
    .compile(binding())
    .expect("two caps");
  let new_plan = disjunction
    .validate()
    .expect("canonical OR")
    .compile(binding())
    .expect("same two caps");
  assert_eq!(old_plan.metrics(), new_plan.metrics());
  assert_eq!(old_plan.mode(), new_plan.mode());
  let old = old_plan
    .bind(&conjunction, binding())
    .expect("verified old source");
  let new = new_plan
    .bind(&disjunction, binding())
    .expect("verified new source");
  let state = old.begin_episode([3; 32]).expect("episode");
  // Even a caller asserting an unchanged semantic ID cannot bypass source comparison.
  assert_eq!(
    new.rebind_episode(&old, &state, [3; 32]),
    Err(BehaviorPolicyError::ReplacementRequired)
  );
}
