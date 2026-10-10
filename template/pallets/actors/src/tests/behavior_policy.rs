use super::behavior_reference as oracle;
use crate::{
  BehaviorAtom, BehaviorMetric, BehaviorPolicyBinding, BehaviorPolicyError, BehaviorPolicyPlan,
  BehaviorTrackingMode, CloseWhen,
};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use polkadot_sdk::frame_support::traits::ConstU32;

type Expression = CloseWhen<ConstU32<4>, ConstU32<4>>;

polkadot_sdk::frame_support::parameter_types! {
  pub static PolicyClauseLimit: u32 = 4;
  pub static PolicyAtomLimit: u32 = 4;
}

fn atom(metric: BehaviorMetric, threshold: u32) -> BehaviorAtom {
  BehaviorAtom { metric, threshold }
}

pub(super) fn expression(clauses: Vec<Vec<BehaviorAtom>>) -> Expression {
  Expression {
    clauses: clauses
      .into_iter()
      .map(|clause| clause.try_into().expect("fixture clause is bounded"))
      .collect::<Vec<_>>()
      .try_into()
      .expect("fixture expression is bounded"),
  }
}

fn binding() -> BehaviorPolicyBinding {
  BehaviorPolicyBinding {
    semantic_contract_id: [1; 32],
    admission_identity: [2; 32],
  }
}

pub(super) fn oracle_metric(metric: BehaviorMetric) -> oracle::Metric {
  match metric {
    BehaviorMetric::PreconditionPassTotal => oracle::Metric::PreconditionPassTotal,
    BehaviorMetric::PreconditionPassStreak => oracle::Metric::PreconditionPassStreak,
    BehaviorMetric::PreconditionRejectTotal => oracle::Metric::PreconditionRejectTotal,
    BehaviorMetric::PreconditionRejectStreak => oracle::Metric::PreconditionRejectStreak,
    BehaviorMetric::ActionSuccessTotal => oracle::Metric::ActionSuccessTotal,
    BehaviorMetric::ActionSuccessStreak => oracle::Metric::ActionSuccessStreak,
    BehaviorMetric::ActionFailureTotal => oracle::Metric::ActionFailureTotal,
    BehaviorMetric::ActionFailureStreak => oracle::Metric::ActionFailureStreak,
    BehaviorMetric::CycleCompletedTotal => oracle::Metric::CycleCompletedTotal,
    BehaviorMetric::CycleCleanTotal => oracle::Metric::CycleCleanTotal,
  }
}

pub(super) fn oracle_policy(expression: &Expression) -> oracle::Policy {
  oracle::Policy::new(
    expression
      .clauses
      .iter()
      .map(|clause| {
        clause
          .iter()
          .map(|atom| oracle::Atom {
            metric: oracle_metric(atom.metric),
            threshold: u64::from(atom.threshold),
          })
          .collect()
      })
      .collect(),
  )
  .expect("positive nonempty fixture")
}

#[test]
fn behavior_policy_rejects_empty_zero_unordered_duplicate_and_redundant_forms() {
  use BehaviorMetric::{ActionFailureTotal as Failure, ActionSuccessTotal as Success};
  let cases = [
    (vec![], BehaviorPolicyError::EmptyExpression),
    (vec![vec![]], BehaviorPolicyError::EmptyClause),
    (
      vec![vec![atom(Success, 0)]],
      BehaviorPolicyError::ZeroThreshold,
    ),
    (
      vec![vec![atom(Failure, 1), atom(Success, 1)]],
      BehaviorPolicyError::NonCanonicalAtoms,
    ),
    (
      vec![vec![atom(Success, 1), atom(Success, 1)]],
      BehaviorPolicyError::NonCanonicalAtoms,
    ),
    (
      vec![vec![atom(Success, 1), atom(Success, 2)]],
      BehaviorPolicyError::NonCanonicalAtoms,
    ),
    (
      vec![vec![atom(Failure, 1)], vec![atom(Success, 1)]],
      BehaviorPolicyError::NonCanonicalClauses,
    ),
    (
      vec![vec![atom(Success, 1)], vec![atom(Success, 1)]],
      BehaviorPolicyError::NonCanonicalClauses,
    ),
    (
      vec![vec![atom(Success, 1)], vec![atom(Success, 2)]],
      BehaviorPolicyError::RedundantClause,
    ),
    (
      vec![
        vec![atom(Success, 2)],
        vec![atom(Success, 2), atom(Failure, 1)],
      ],
      BehaviorPolicyError::RedundantClause,
    ),
  ];
  for (clauses, error) in cases {
    assert_eq!(expression(clauses).validate().err(), Some(error));
  }
  let incomparable = expression(vec![
    vec![atom(Success, 1), atom(Failure, 2)],
    vec![atom(Success, 2), atom(Failure, 1)],
  ]);
  assert!(incomparable.validate().is_ok());
}

#[test]
fn behavior_policy_invalid_host_bounds_fail_before_expression_admission() {
  let zero = CloseWhen::<ConstU32<0>, ConstU32<4>> {
    clauses: Default::default(),
  };
  assert_eq!(
    zero.validate().err(),
    Some(BehaviorPolicyError::InvalidBounds)
  );
  let overflow = CloseWhen::<ConstU32<{ u32::MAX }>, ConstU32<2>> {
    clauses: Default::default(),
  };
  assert_eq!(
    overflow.validate().err(),
    Some(BehaviorPolicyError::InvalidBounds)
  );
}

#[test]
fn behavior_policy_revalidates_values_after_host_limits_shrink() {
  let policy = CloseWhen::<PolicyClauseLimit, PolicyAtomLimit> {
    clauses: vec![
      vec![
        atom(BehaviorMetric::PreconditionPassTotal, 1),
        atom(BehaviorMetric::ActionSuccessTotal, 1),
      ]
      .try_into()
      .expect("two atoms"),
      vec![
        atom(BehaviorMetric::PreconditionRejectTotal, 1),
        atom(BehaviorMetric::ActionFailureTotal, 1),
      ]
      .try_into()
      .expect("two atoms"),
    ]
    .try_into()
    .expect("two clauses"),
  };
  assert!(policy.validate().is_ok());
  for (clauses, atoms) in [(1, 4), (4, 1)] {
    PolicyClauseLimit::set(clauses);
    PolicyAtomLimit::set(atoms);
    let error = policy.validate().err();
    PolicyClauseLimit::set(4);
    PolicyAtomLimit::set(4);
    assert_eq!(error, Some(BehaviorPolicyError::ExceedsBounds));
  }
}

#[test]
fn behavior_policy_codec_rejects_unknown_metrics_and_overlimit_vectors() {
  for discriminant in [10_u8, 255] {
    assert!(BehaviorMetric::decode(&mut &[discriminant][..]).is_err());
    let bytes = [4, 4, discriminant, 1, 0, 0, 0];
    assert!(Expression::decode(&mut &bytes[..]).is_err());
  }
  let over_clauses = vec![vec![atom(BehaviorMetric::ActionSuccessTotal, 1)]; 5].encode();
  assert!(Expression::decode(&mut &over_clauses[..]).is_err());
  let over_atoms = vec![vec![atom(BehaviorMetric::ActionSuccessTotal, 1); 5]].encode();
  assert!(Expression::decode(&mut &over_atoms[..]).is_err());
}

#[test]
fn behavior_policy_encoding_bounds_are_attained_without_weight_fields() {
  fn requires_memory_tracking<T: DecodeWithMemTracking>() {}
  requires_memory_tracking::<Expression>();
  let metrics = [
    BehaviorMetric::PreconditionPassTotal,
    BehaviorMetric::PreconditionPassStreak,
    BehaviorMetric::PreconditionRejectTotal,
    BehaviorMetric::PreconditionRejectStreak,
    BehaviorMetric::ActionSuccessTotal,
    BehaviorMetric::ActionSuccessStreak,
    BehaviorMetric::ActionFailureTotal,
    BehaviorMetric::ActionFailureStreak,
    BehaviorMetric::CycleCompletedTotal,
    BehaviorMetric::CycleCleanTotal,
  ];
  for (index, metric) in metrics.into_iter().enumerate() {
    let discriminant = u8::try_from(index).expect("ten metrics");
    assert_eq!(metric.encode(), vec![discriminant]);
    assert_eq!(BehaviorMetric::decode(&mut &[discriminant][..]), Ok(metric));
  }
  assert_eq!(BehaviorMetric::max_encoded_len(), 1);
  assert_eq!(BehaviorAtom::max_encoded_len(), 5);
  let maximum = expression(vec![
    vec![
      atom(metrics[0], u32::MAX),
      atom(metrics[1], u32::MAX),
      atom(metrics[2], u32::MAX),
      atom(metrics[3], u32::MAX),
    ],
    vec![
      atom(metrics[0], u32::MAX),
      atom(metrics[1], u32::MAX),
      atom(metrics[2], u32::MAX),
      atom(metrics[4], u32::MAX),
    ],
    vec![
      atom(metrics[0], u32::MAX),
      atom(metrics[1], u32::MAX),
      atom(metrics[2], u32::MAX),
      atom(metrics[5], u32::MAX),
    ],
    vec![
      atom(metrics[0], u32::MAX),
      atom(metrics[1], u32::MAX),
      atom(metrics[2], u32::MAX),
      atom(metrics[6], u32::MAX),
    ],
  ]);
  assert!(maximum.validate().is_ok());
  assert_eq!(maximum.encoded_size(), 85);
  assert_eq!(Expression::max_encoded_len(), maximum.encoded_size());
  assert_eq!(Option::<Expression>::max_encoded_len(), 86);
  assert_eq!(Expression::decode(&mut &maximum.encode()[..]), Ok(maximum));
}

#[test]
fn behavior_policy_evaluation_visits_every_atom_even_after_false_and_true_clauses() {
  let expression = expression(vec![
    vec![
      atom(BehaviorMetric::PreconditionPassTotal, 1),
      atom(BehaviorMetric::ActionSuccessTotal, 1),
    ],
    vec![
      atom(BehaviorMetric::PreconditionRejectTotal, 1),
      atom(BehaviorMetric::ActionFailureTotal, 1),
    ],
  ]);
  let checked = expression.validate().expect("canonical DNF");
  for value in [0, 1] {
    let mut visited = vec![];
    assert_eq!(
      checked.matches(|metric| {
        visited.push(metric);
        value
      }),
      value == 1
    );
    assert_eq!(
      visited,
      vec![
        BehaviorMetric::PreconditionPassTotal,
        BehaviorMetric::ActionSuccessTotal,
        BehaviorMetric::PreconditionRejectTotal,
        BehaviorMetric::ActionFailureTotal,
      ]
    );
  }
}

#[test]
fn behavior_policy_differential_dnf_matches_independent_committed_trace_oracle() {
  let metrics = [
    BehaviorMetric::PreconditionPassTotal,
    BehaviorMetric::ActionFailureStreak,
    BehaviorMetric::CycleCompletedTotal,
  ];
  let mut clauses = vec![];
  for (index, metric) in metrics.into_iter().enumerate() {
    for threshold in 1..=2 {
      clauses.push(vec![atom(metric, threshold)]);
    }
    for other in &metrics[index + 1..] {
      for threshold in 1..=2 {
        for other_threshold in 1..=2 {
          clauses.push(vec![atom(metric, threshold), atom(*other, other_threshold)]);
        }
      }
    }
  }
  clauses.sort();
  let mut expressions = vec![];
  for (index, clause) in clauses.iter().enumerate() {
    expressions.push(expression(vec![clause.clone()]));
    for other in &clauses[index + 1..] {
      let candidate = expression(vec![clause.clone(), other.clone()]);
      if candidate.validate().is_ok() {
        expressions.push(candidate);
      }
    }
  }
  let plans: Vec<_> = expressions
    .iter()
    .map(|expression| {
      expression
        .validate()
        .expect("canonical expression")
        .compile(binding())
        .expect("closed catalog")
    })
    .collect();
  let outcomes = [
    (oracle::Check::Pass, oracle::Action::Success),
    (oracle::Check::Pass, oracle::Action::Failure),
    (oracle::Check::Absent, oracle::Action::Failure),
    (oracle::Check::Reject, oracle::Action::NotInvoked),
    (oracle::Check::Error, oracle::Action::NotInvoked),
    (oracle::Check::Absent, oracle::Action::NotInvoked),
  ];
  let mut compared = 0;
  for first in &outcomes {
    for second in &outcomes {
      for third in &outcomes {
        let mut trace = vec![];
        for (check, action) in [first, second, third] {
          trace.push(oracle::Record::committed(oracle::Transition::Open));
          trace.push(oracle::Record::committed(oracle::Transition::Attempt {
            check: *check,
            action: *action,
            disposition: oracle::Disposition::Advance,
          }));
        }
        let projection = oracle::project(&[oracle::StepKind::Economic], &trace, None);
        for (expression, plan) in expressions.iter().zip(&plans) {
          let checked = expression.validate().expect("selected canonical policy");
          let expected = oracle_policy(expression).matches(&projection);
          let actual = checked.matches(|metric| {
            u32::try_from(projection.value(oracle_metric(metric))).expect("three-Cycle trace")
          });
          assert_eq!(actual, expected, "{expression:?}, {trace:?}");
          let capped = checked.matches(|metric| {
            u32::try_from(projection.value(oracle_metric(metric)))
              .expect("three-Cycle trace")
              .min(
                plan
                  .cap(metric)
                  .expect("every referenced metric has one cap"),
              )
          });
          assert_eq!(capped, expected, "capped {expression:?}, {trace:?}");
          compared += 1;
        }
      }
    }
  }
  assert!(compared > 0, "differential selection is nonempty");
}

#[test]
fn behavior_plan_deduplicates_metrics_and_selects_largest_referenced_caps() {
  use BehaviorMetric::{ActionFailureTotal as Failure, ActionSuccessTotal as Success};
  let policy = expression(vec![
    vec![atom(Success, 2), atom(Failure, 3)],
    vec![atom(Success, 4), atom(Failure, 1)],
  ]);
  let plan = policy
    .validate()
    .expect("incomparable clauses")
    .compile(binding())
    .expect("ten metrics");
  assert_eq!(plan.metrics(), &[atom(Success, 4), atom(Failure, 3)]);
  assert_eq!(plan.mode(), BehaviorTrackingMode::Accumulated);
  assert_eq!(plan.cap(BehaviorMetric::PreconditionPassTotal), None);
  assert!(!plan.requires_clean_cycle());
  assert_eq!(plan.verify(&policy, binding()), Ok(()));
}

#[test]
fn behavior_plan_current_outcome_mode_requires_only_singleton_threshold_one_clauses() {
  use BehaviorMetric::{
    ActionSuccessTotal as Success, CycleCleanTotal as Clean, PreconditionPassTotal as Pass,
  };
  let cases = [
    (
      vec![vec![atom(Success, 1)]],
      BehaviorTrackingMode::CurrentOutcome,
      false,
    ),
    (
      vec![vec![atom(Pass, 1)], vec![atom(Success, 1)]],
      BehaviorTrackingMode::CurrentOutcome,
      false,
    ),
    (
      vec![vec![atom(Clean, 1)]],
      BehaviorTrackingMode::CurrentOutcome,
      true,
    ),
    (
      vec![vec![atom(Success, 2)]],
      BehaviorTrackingMode::Accumulated,
      false,
    ),
    (
      vec![vec![atom(Pass, 1), atom(Success, 1)]],
      BehaviorTrackingMode::Accumulated,
      false,
    ),
    (
      vec![vec![atom(Success, 1), atom(Clean, 1)]],
      BehaviorTrackingMode::Accumulated,
      true,
    ),
  ];
  for (clauses, mode, clean) in cases {
    let plan = expression(clauses)
      .validate()
      .expect("canonical")
      .compile(binding())
      .expect("ten metrics");
    assert_eq!(plan.mode(), mode);
    assert_eq!(plan.requires_clean_cycle(), clean);
  }
}

#[test]
fn behavior_plan_identity_binds_expression_semantics_admission_and_limits() {
  let policy = CloseWhen::<PolicyClauseLimit, PolicyAtomLimit> {
    clauses: vec![
      vec![atom(BehaviorMetric::ActionSuccessTotal, 20)]
        .try_into()
        .expect("one atom"),
    ]
    .try_into()
    .expect("one clause"),
  };
  let checked = policy.validate().expect("four-by-four limit snapshot");
  let plan = checked.compile(binding()).expect("ten metrics");
  for changed in [
    BehaviorPolicyBinding {
      semantic_contract_id: [3; 32],
      ..binding()
    },
    BehaviorPolicyBinding {
      admission_identity: [3; 32],
      ..binding()
    },
  ] {
    assert_eq!(
      plan.verify(&policy, changed),
      Err(BehaviorPolicyError::PlanMismatch)
    );
  }
  let changed = CloseWhen::<PolicyClauseLimit, PolicyAtomLimit> {
    clauses: vec![
      vec![atom(BehaviorMetric::ActionSuccessTotal, 21)]
        .try_into()
        .expect("one atom"),
    ]
    .try_into()
    .expect("one clause"),
  };
  assert_eq!(
    plan.verify(&changed, binding()),
    Err(BehaviorPolicyError::PlanMismatch)
  );
  PolicyClauseLimit::set(3);
  let fresh = policy
    .validate()
    .expect("one clause still fits")
    .compile(binding())
    .expect("ten metrics");
  let verification = plan.verify(&policy, binding());
  let original_snapshot = checked
    .compile(binding())
    .expect("immutable captured limits");
  PolicyClauseLimit::set(4);
  assert_ne!(fresh.identity(), plan.identity());
  assert_eq!(verification, Err(BehaviorPolicyError::PlanMismatch));
  assert_eq!(original_snapshot, plan);
  assert_eq!(plan.verify(&policy, binding()), Ok(()));
}

#[test]
fn behavior_plan_codec_decode_does_not_authorize_forged_derived_fields() {
  let policy = expression(vec![vec![atom(BehaviorMetric::ActionSuccessTotal, 20)]]);
  let plan = policy
    .validate()
    .expect("canonical")
    .compile(binding())
    .expect("ten metrics");
  let bytes = plan.encode();
  assert_eq!(bytes.len(), 39);
  for (offset, byte) in [(0, bytes[0] ^ 1), (32, 0), (34, 0), (35, 0)] {
    let mut forged = bytes.clone();
    forged[offset] = byte;
    let decoded =
      BehaviorPolicyPlan::decode(&mut &forged[..]).expect("representation is decodable");
    assert_eq!(
      decoded.verify(&policy, binding()),
      Err(BehaviorPolicyError::PlanMismatch)
    );
  }
  let mut unknown_mode = bytes.clone();
  unknown_mode[32] = 2;
  assert!(BehaviorPolicyPlan::decode(&mut &unknown_mode[..]).is_err());
  let mut overflow = bytes[..33].to_vec();
  overflow.extend(vec![atom(BehaviorMetric::ActionSuccessTotal, 1); 11].encode());
  assert!(BehaviorPolicyPlan::decode(&mut &overflow[..]).is_err());
}

#[test]
fn behavior_plan_all_metric_encoding_maximum_and_hash_input_are_attained() {
  fn requires_memory_tracking<T: DecodeWithMemTracking>() {}
  requires_memory_tracking::<BehaviorPolicyPlan>();
  use BehaviorMetric::*;
  let policy = expression(vec![
    vec![
      atom(PreconditionPassTotal, u32::MAX),
      atom(PreconditionPassStreak, u32::MAX),
      atom(PreconditionRejectTotal, u32::MAX),
      atom(PreconditionRejectStreak, u32::MAX),
    ],
    vec![
      atom(PreconditionPassTotal, u32::MAX),
      atom(ActionSuccessTotal, u32::MAX),
      atom(ActionSuccessStreak, u32::MAX),
      atom(ActionFailureTotal, u32::MAX),
    ],
    vec![
      atom(PreconditionPassStreak, u32::MAX),
      atom(ActionFailureStreak, u32::MAX),
      atom(CycleCompletedTotal, u32::MAX),
      atom(CycleCleanTotal, u32::MAX),
    ],
    vec![
      atom(PreconditionRejectTotal, u32::MAX),
      atom(PreconditionRejectStreak, u32::MAX),
      atom(ActionSuccessTotal, u32::MAX),
      atom(ActionSuccessStreak, u32::MAX),
    ],
  ]);
  let plan = policy
    .validate()
    .expect("distinct four-atom clauses")
    .compile(binding())
    .expect("closed ten-metric catalog");
  assert_eq!(plan.metrics().len(), 10);
  assert!(plan.metrics().iter().all(|atom| atom.threshold == u32::MAX));
  assert!(plan.requires_clean_cycle());
  assert_eq!(plan.encoded_size(), 84);
  assert_eq!(BehaviorPolicyPlan::max_encoded_len(), plan.encoded_size());
  assert_eq!(BehaviorPolicyBinding::max_encoded_len(), 64);
  let preimage = (
    *b"DEOS_ACTOR_BEHAVIOR_PLAN",
    binding(),
    (4_u32, 4_u32),
    &policy,
  )
    .encode();
  assert_eq!(
    preimage.len(),
    b"DEOS_ACTOR_BEHAVIOR_PLAN".len() + 64 + 8 + policy.encoded_size()
  );
  assert_eq!(preimage.len(), 181);
  assert_eq!(plan.identity(), frame::hashing::blake2_256(&preimage));
  assert_eq!(
    BehaviorPolicyPlan::decode(&mut &plan.encode()[..]),
    Ok(plan)
  );
}
