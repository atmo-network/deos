//! Test-only outcome oracle. Replays committed traces and recomputes exact metrics from
//! observation histories; it is not the production sparse/capped monitor or a scheduler.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Metric {
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Atom {
  pub metric: Metric,
  pub threshold: u64,
}

#[derive(Clone, Debug)]
pub(super) struct Policy(Vec<Vec<Atom>>);

impl Policy {
  pub fn new(clauses: Vec<Vec<Atom>>) -> Result<Self, &'static str> {
    if clauses.is_empty() || clauses.iter().any(Vec::is_empty) {
      return Err("empty expression or clause");
    }
    if clauses.iter().flatten().any(|atom| atom.threshold == 0) {
      return Err("zero threshold");
    }
    Ok(Self(clauses))
  }

  pub(super) fn matches(&self, projection: &Projection) -> bool {
    self.0.iter().any(|clause| {
      clause
        .iter()
        .all(|atom| projection.value(atom.metric) >= atom.threshold)
    })
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StepKind {
  Economic,
  StopCycle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Check {
  Absent,
  Pass,
  Reject,
  Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Action {
  NotInvoked,
  Success,
  Failure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Disposition {
  Advance,
  Retry,
  Abort,
  Return,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PreservedTransition {
  CapacityRefusal,
  AwaitFunding,
  TopUp,
  FundedRecovery,
  SystemPause,
  SystemResume,
  NoOpReplacement,
  Recertification,
}

#[derive(Clone, Debug)]
pub(super) enum Transition {
  Open,
  Attempt {
    check: Check,
    action: Action,
    disposition: Disposition,
  },
  Cancel,
  Preserve(PreservedTransition),
  Replace(Vec<StepKind>),
  Deactivate,
  Activate,
}

#[derive(Clone, Debug)]
pub(super) struct Record {
  pub transition: Transition,
  pub committed: bool,
}

impl Record {
  pub fn committed(transition: Transition) -> Self {
    Self {
      transition,
      committed: true,
    }
  }

  pub fn rolled_back(transition: Transition) -> Self {
    Self {
      transition,
      committed: false,
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Cycle {
  cursor: usize,
  clean_possible: bool,
  has_successful_action: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Projection {
  preconditions: Vec<bool>,
  actions: Vec<bool>,
  completions: Vec<bool>,
  cycle: Option<Cycle>,
  pub closed: bool,
}

impl Projection {
  pub fn value(&self, metric: Metric) -> u64 {
    let total = |stream: &[bool], outcome| {
      u64::try_from(stream.iter().filter(|result| **result == outcome).count())
        .expect("finite oracle trace")
    };
    let streak = |stream: &[bool], outcome| {
      u64::try_from(
        stream
          .iter()
          .rev()
          .take_while(|result| **result == outcome)
          .count(),
      )
      .expect("finite oracle trace")
    };
    match metric {
      Metric::PreconditionPassTotal => total(&self.preconditions, true),
      Metric::PreconditionPassStreak => streak(&self.preconditions, true),
      Metric::PreconditionRejectTotal => total(&self.preconditions, false),
      Metric::PreconditionRejectStreak => streak(&self.preconditions, false),
      Metric::ActionSuccessTotal => total(&self.actions, true),
      Metric::ActionSuccessStreak => streak(&self.actions, true),
      Metric::ActionFailureTotal => total(&self.actions, false),
      Metric::ActionFailureStreak => streak(&self.actions, false),
      Metric::CycleCompletedTotal => u64::try_from(self.completions.len()).expect("finite trace"),
      Metric::CycleCleanTotal => total(&self.completions, true),
    }
  }

  fn complete(&mut self, reached_plan_end: bool) {
    let cycle = self.cycle.take().expect("completion owns one open Cycle");
    self
      .completions
      .push(reached_plan_end && cycle.clean_possible && cycle.has_successful_action);
  }
}

pub(super) fn project(
  initial_steps: &[StepKind],
  records: &[Record],
  policy: Option<&Policy>,
) -> Projection {
  let mut steps = initial_steps.to_vec();
  let mut projection = Projection::default();
  for record in records.iter().filter(|record| record.committed) {
    assert!(
      !projection.closed,
      "a closed Actor cannot consume further transitions"
    );
    let evaluate_policy = match &record.transition {
      Transition::Open => {
        assert!(projection.cycle.is_none(), "one current Cycle only");
        projection.cycle = Some(Cycle {
          cursor: 0,
          clean_possible: true,
          has_successful_action: false,
        });
        if steps.is_empty() {
          projection.complete(true);
        }
        steps.is_empty()
      }
      Transition::Attempt {
        check,
        action,
        disposition,
      } => {
        let cycle = projection
          .cycle
          .as_mut()
          .expect("attempt owns an open Cycle");
        let kind = steps[cycle.cursor];
        if matches!(check, Check::Reject | Check::Error) {
          assert_eq!(*action, Action::NotInvoked);
        }
        if kind == StepKind::StopCycle {
          assert_eq!(*action, Action::NotInvoked, "StopCycle is not an Action");
        }
        match check {
          Check::Pass => projection.preconditions.push(true),
          Check::Reject => projection.preconditions.push(false),
          Check::Error => cycle.clean_possible = false,
          Check::Absent => {}
        }
        match action {
          Action::Success => {
            projection.actions.push(true);
            cycle.has_successful_action = true;
          }
          Action::Failure => {
            projection.actions.push(false);
            cycle.clean_possible = false;
          }
          Action::NotInvoked => {}
        }
        match disposition {
          Disposition::Advance => {
            if kind == StepKind::Economic && *action != Action::Success {
              cycle.clean_possible = false;
            }
            cycle.cursor += 1;
            if cycle.cursor == steps.len() {
              projection.complete(true);
            }
          }
          Disposition::Retry => {}
          Disposition::Abort => projection.cycle = None,
          Disposition::Return => {
            assert_eq!(kind, StepKind::StopCycle);
            assert!(matches!(check, Check::Absent | Check::Pass));
            let reached_plan_end = cycle.cursor + 1 == steps.len();
            projection.complete(reached_plan_end);
          }
        }
        true
      }
      Transition::Cancel => {
        projection.cycle = None;
        false
      }
      Transition::Preserve(reason) => {
        // Enumerated inputs intentionally have no observation in either stream.
        match reason {
          PreservedTransition::CapacityRefusal
          | PreservedTransition::AwaitFunding
          | PreservedTransition::TopUp
          | PreservedTransition::FundedRecovery
          | PreservedTransition::SystemPause
          | PreservedTransition::SystemResume
          | PreservedTransition::NoOpReplacement
          | PreservedTransition::Recertification => {}
        }
        false
      }
      Transition::Replace(replacement) => {
        steps = replacement.clone();
        projection = Projection::default();
        false
      }
      Transition::Deactivate | Transition::Activate => {
        projection = Projection::default();
        false
      }
    };
    if evaluate_policy && policy.is_some_and(|policy| policy.matches(&projection)) {
      projection.closed = true;
      projection.cycle = None;
    }
  }
  projection
}

fn attempt(check: Check, action: Action, disposition: Disposition) -> Record {
  Record::committed(Transition::Attempt {
    check,
    action,
    disposition,
  })
}

fn opened() -> Record {
  Record::committed(Transition::Open)
}

fn rule(metric: Metric, threshold: u64) -> Policy {
  Policy::new(vec![vec![Atom { metric, threshold }]]).expect("valid fixture policy")
}

#[test]
fn behavior_reference_stream_metrics_observe_absence_opposites_and_rollback() {
  let streams = [
    (
      Metric::PreconditionPassTotal,
      Metric::PreconditionPassStreak,
      Metric::PreconditionRejectTotal,
      Metric::PreconditionRejectStreak,
      true,
    ),
    (
      Metric::ActionSuccessTotal,
      Metric::ActionSuccessStreak,
      Metric::ActionFailureTotal,
      Metric::ActionFailureStreak,
      false,
    ),
  ];
  for (positive_total, positive_streak, negative_total, negative_streak, is_check) in streams {
    let positive = if is_check {
      attempt(Check::Pass, Action::NotInvoked, Disposition::Advance)
    } else {
      attempt(Check::Absent, Action::Success, Disposition::Advance)
    };
    let negative = if is_check {
      attempt(Check::Reject, Action::NotInvoked, Disposition::Advance)
    } else {
      attempt(Check::Absent, Action::Failure, Disposition::Advance)
    };
    let mut trace = vec![opened(), positive.clone(), opened(), positive.clone()];
    let before = project(&[StepKind::Economic], &trace, None);
    assert_eq!(before.value(positive_total), 2);
    assert_eq!(before.value(positive_streak), 2);
    assert_eq!(before.value(negative_total), 0);
    assert_eq!(before.value(negative_streak), 0);
    trace.push(Record::rolled_back(Transition::Open));
    trace.push(Record::rolled_back(negative.transition.clone()));
    for reason in [
      PreservedTransition::CapacityRefusal,
      PreservedTransition::AwaitFunding,
      PreservedTransition::TopUp,
      PreservedTransition::FundedRecovery,
      PreservedTransition::SystemPause,
      PreservedTransition::SystemResume,
      PreservedTransition::NoOpReplacement,
      PreservedTransition::Recertification,
    ] {
      trace.push(Record::committed(Transition::Preserve(reason)));
    }
    assert_eq!(project(&[StepKind::Economic], &trace, None), before);
    trace.extend([opened(), negative.clone(), opened(), negative]);
    let after = project(&[StepKind::Economic], &trace, None);
    assert_eq!(after.value(positive_total), 2);
    assert_eq!(after.value(positive_streak), 0);
    assert_eq!(after.value(negative_total), 2);
    assert_eq!(after.value(negative_streak), 2);
    trace.extend([
      opened(),
      attempt(Check::Error, Action::NotInvoked, Disposition::Advance),
    ]);
    let error = project(&[StepKind::Economic], &trace, None);
    for metric in [
      positive_total,
      positive_streak,
      negative_total,
      negative_streak,
    ] {
      assert_eq!(error.value(metric), after.value(metric));
    }
    trace.extend([opened(), positive]);
    let reset = project(&[StepKind::Economic], &trace, None);
    assert_eq!(reset.value(positive_total), 3);
    assert_eq!(reset.value(positive_streak), 1);
    assert_eq!(reset.value(negative_total), 2);
    assert_eq!(reset.value(negative_streak), 0);
  }
}

#[test]
fn behavior_reference_completion_matrix_distinguishes_return_skip_retry_and_empty() {
  let success = || attempt(Check::Absent, Action::Success, Disposition::Advance);
  let stop = || attempt(Check::Pass, Action::NotInvoked, Disposition::Return);
  let cases = [
    (
      vec![StepKind::Economic; 2],
      vec![opened(), success(), success()],
      1,
      1,
    ),
    (
      vec![StepKind::Economic; 2],
      vec![
        opened(),
        success(),
        attempt(Check::Reject, Action::NotInvoked, Disposition::Advance),
      ],
      1,
      0,
    ),
    (
      vec![StepKind::Economic],
      vec![
        opened(),
        attempt(Check::Pass, Action::Failure, Disposition::Retry),
        success(),
      ],
      1,
      0,
    ),
    (
      vec![StepKind::Economic, StepKind::StopCycle, StepKind::Economic],
      vec![opened(), success(), stop()],
      1,
      0,
    ),
    (
      vec![StepKind::Economic, StepKind::StopCycle],
      vec![opened(), success(), stop()],
      1,
      1,
    ),
    (vec![], vec![opened()], 1, 0),
    (vec![StepKind::StopCycle], vec![opened(), stop()], 1, 0),
    (
      vec![StepKind::Economic],
      vec![
        opened(),
        attempt(Check::Pass, Action::Failure, Disposition::Abort),
      ],
      0,
      0,
    ),
    (
      vec![StepKind::Economic],
      vec![opened(), Record::committed(Transition::Cancel)],
      0,
      0,
    ),
    (
      vec![StepKind::Economic],
      vec![
        opened(),
        attempt(Check::Pass, Action::NotInvoked, Disposition::Retry),
        success(),
      ],
      1,
      1,
    ),
  ];
  for (steps, trace, completed, clean) in cases {
    let result = project(&steps, &trace, None);
    assert_eq!(
      result.value(Metric::CycleCompletedTotal),
      completed,
      "{trace:?}"
    );
    assert_eq!(result.value(Metric::CycleCleanTotal), clean, "{trace:?}");
    assert!(result.cycle.is_none());
  }
}

#[test]
fn behavior_reference_conditional_return_false_reaches_tail_true_completes_once() {
  let steps = [StepKind::Economic, StepKind::StopCycle, StepKind::Economic];
  for (check, disposition, completed) in [
    (Check::Reject, Disposition::Advance, 0),
    (Check::Pass, Disposition::Return, 1),
  ] {
    let trace = [
      opened(),
      attempt(Check::Absent, Action::Success, Disposition::Advance),
      attempt(check, Action::NotInvoked, disposition),
    ];
    let result = project(&steps, &trace, None);
    assert_eq!(result.value(Metric::CycleCompletedTotal), completed);
    assert_eq!(result.value(Metric::ActionSuccessTotal), 1);
    assert_eq!(result.value(Metric::ActionFailureTotal), 0);
    if completed == 0 {
      assert_eq!(result.cycle.expect("remaining tail").cursor, 2);
    } else {
      assert!(result.cycle.is_none());
    }
  }
}

#[test]
fn behavior_reference_two_step_conditional_transfer_needs_no_return_step() {
  let policy = rule(Metric::CycleCompletedTotal, 1);
  for (check, action, successes, clean) in [
    (Check::Reject, Action::NotInvoked, 1, 0),
    (Check::Pass, Action::Success, 2, 1),
  ] {
    let trace = [
      opened(),
      attempt(Check::Absent, Action::Success, Disposition::Advance),
      attempt(check, action, Disposition::Advance),
    ];
    let result = project(&[StepKind::Economic; 2], &trace, Some(&policy));
    assert!(result.closed);
    assert_eq!(result.value(Metric::ActionSuccessTotal), successes);
    assert_eq!(result.value(Metric::CycleCompletedTotal), 1);
    assert_eq!(result.value(Metric::CycleCleanTotal), clean);
  }
}

#[test]
fn behavior_reference_twenty_successes_and_five_failures_close_at_exact_thresholds() {
  let success_policy = rule(Metric::ActionSuccessTotal, 20);
  let mut trace = vec![];
  for count in 1..=20 {
    trace.extend([
      opened(),
      attempt(Check::Pass, Action::Success, Disposition::Advance),
    ]);
    let result = project(&[StepKind::Economic], &trace, Some(&success_policy));
    assert_eq!(result.value(Metric::ActionSuccessTotal), count);
    assert_eq!(result.closed, count == 20);
  }
  let failure_policy = rule(Metric::ActionFailureStreak, 5);
  let mut trace = vec![];
  for count in 1..=5 {
    trace.extend([
      opened(),
      attempt(Check::Pass, Action::Failure, Disposition::Advance),
    ]);
    let result = project(&[StepKind::Economic; 2], &trace, Some(&failure_policy));
    assert_eq!(result.value(Metric::ActionFailureStreak), count);
    assert_eq!(result.closed, count == 5);
    if count < 5 {
      trace.extend([
        Record::committed(Transition::Preserve(PreservedTransition::AwaitFunding)),
        Record::committed(Transition::Preserve(PreservedTransition::TopUp)),
        Record::committed(Transition::Preserve(PreservedTransition::FundedRecovery)),
        attempt(Check::Reject, Action::NotInvoked, Disposition::Advance),
      ]);
    } else {
      assert_eq!(result.value(Metric::CycleCompletedTotal), 4);
    }
  }
}

#[test]
fn behavior_reference_streak_and_historical_completion_do_not_prove_clean_cycle() {
  let steps = [StepKind::Economic; 2];
  let trace = [
    opened(),
    attempt(Check::Absent, Action::Success, Disposition::Advance),
    attempt(Check::Reject, Action::NotInvoked, Disposition::Advance),
    opened(),
    attempt(Check::Absent, Action::Success, Disposition::Advance),
  ];
  let result = project(&steps, &trace, None);
  assert_eq!(result.value(Metric::ActionSuccessStreak), 2);
  assert_eq!(result.value(Metric::CycleCompletedTotal), 1);
  assert_eq!(result.value(Metric::CycleCleanTotal), 0);
  assert_eq!(result.cycle.expect("second Cycle is incomplete").cursor, 1);
  let misleading = Policy::new(vec![vec![
    Atom {
      metric: Metric::CycleCompletedTotal,
      threshold: 1,
    },
    Atom {
      metric: Metric::ActionSuccessStreak,
      threshold: 2,
    },
  ]])
  .expect("valid but different semantics");
  assert!(project(&steps, &trace, Some(&misleading)).closed);
  assert!(!project(&steps, &trace, Some(&rule(Metric::CycleCleanTotal, 1))).closed);
}

#[test]
fn behavior_reference_policy_runs_after_action_and_before_successor() {
  let steps = [StepKind::Economic; 2];
  let trace = [
    opened(),
    attempt(Check::Pass, Action::Success, Disposition::Advance),
  ];
  for metric in [Metric::PreconditionPassTotal, Metric::ActionSuccessTotal] {
    let result = project(&steps, &trace, Some(&rule(metric, 1)));
    assert!(result.closed);
    assert_eq!(result.value(Metric::ActionSuccessTotal), 1);
    assert_eq!(result.value(Metric::CycleCompletedTotal), 0);
  }
}

#[test]
fn behavior_reference_late_rollback_preserves_committed_prefix_and_observations() {
  let steps = [StepKind::Economic; 2];
  let mut trace = vec![
    opened(),
    attempt(Check::Pass, Action::Success, Disposition::Advance),
  ];
  let before = project(&steps, &trace, Some(&rule(Metric::ActionSuccessTotal, 2)));
  for action in [Action::Success, Action::Failure] {
    trace.push(Record::rolled_back(Transition::Attempt {
      check: Check::Pass,
      action,
      disposition: Disposition::Advance,
    }));
    assert_eq!(
      project(&steps, &trace, Some(&rule(Metric::ActionSuccessTotal, 2))),
      before
    );
  }
  trace.push(attempt(Check::Pass, Action::Success, Disposition::Advance));
  let after = project(&steps, &trace, Some(&rule(Metric::ActionSuccessTotal, 2)));
  assert!(after.closed);
  assert_eq!(after.value(Metric::CycleCompletedTotal), 1);
  assert_eq!(after.value(Metric::CycleCleanTotal), 1);
}

#[test]
fn behavior_reference_wait_and_funded_resumption_do_not_reset_cycle_or_streak() {
  let steps = [StepKind::Economic; 2];
  let mut trace = vec![
    opened(),
    attempt(Check::Pass, Action::Success, Disposition::Advance),
  ];
  let before = project(&steps, &trace, None);
  for reason in [
    PreservedTransition::AwaitFunding,
    PreservedTransition::TopUp,
    PreservedTransition::FundedRecovery,
    PreservedTransition::CapacityRefusal,
  ] {
    trace.push(Record::committed(Transition::Preserve(reason)));
    assert_eq!(project(&steps, &trace, None), before);
  }
  trace.push(attempt(Check::Pass, Action::Success, Disposition::Advance));
  let resumed = project(&steps, &trace, None);
  assert_eq!(resumed.value(Metric::ActionSuccessStreak), 2);
  assert_eq!(resumed.value(Metric::CycleCleanTotal), 1);
}

#[test]
fn behavior_reference_semantic_replacement_and_reactivation_start_new_episodes() {
  let steps = [StepKind::Economic; 2];
  let prefix = [
    opened(),
    attempt(Check::Pass, Action::Success, Disposition::Advance),
  ];
  for reset in [
    vec![Record::committed(Transition::Replace(steps.to_vec()))],
    vec![
      Record::committed(Transition::Deactivate),
      Record::committed(Transition::Activate),
    ],
  ] {
    let mut trace = prefix.to_vec();
    trace.extend(reset);
    assert_eq!(project(&steps, &trace, None), Projection::default());
    trace.extend([
      opened(),
      attempt(Check::Pass, Action::Success, Disposition::Advance),
    ]);
    let restarted = project(&steps, &trace, None);
    assert_eq!(restarted.value(Metric::ActionSuccessTotal), 1);
    assert_eq!(restarted.value(Metric::CycleCompletedTotal), 0);
  }
}

#[test]
fn behavior_reference_dnf_atoms_mature_independently_without_history_telemetry_claims() {
  let policy = Policy::new(vec![
    vec![
      Atom {
        metric: Metric::ActionSuccessTotal,
        threshold: 2,
      },
      Atom {
        metric: Metric::ActionFailureTotal,
        threshold: 1,
      },
    ],
    vec![Atom {
      metric: Metric::CycleCompletedTotal,
      threshold: 4,
    }],
  ])
  .expect("bounded fixture DNF");
  let trace = [
    opened(),
    attempt(Check::Pass, Action::Success, Disposition::Advance),
    opened(),
    attempt(Check::Pass, Action::Failure, Disposition::Advance),
    opened(),
    attempt(Check::Absent, Action::Success, Disposition::Advance),
  ];
  assert!(!project(&[StepKind::Economic], &trace[..4], Some(&policy)).closed);
  assert!(project(&[StepKind::Economic], &trace, Some(&policy)).closed);
  for value in [0_u64, 1, 2, 3, 100, u64::MAX] {
    for cap in [1_u64, 2, 5, u64::MAX] {
      for threshold in [1, cap] {
        assert_eq!(value >= threshold, value.min(cap) >= threshold);
      }
    }
  }
  assert!(Policy::new(vec![]).is_err());
  assert!(Policy::new(vec![vec![]]).is_err());
  assert!(
    Policy::new(vec![vec![Atom {
      metric: Metric::ActionSuccessTotal,
      threshold: 0
    }]])
    .is_err()
  );
}
