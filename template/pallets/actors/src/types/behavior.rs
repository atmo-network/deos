use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use polkadot_sdk::frame_support::{
  BoundedVec, CloneNoBound, DebugNoBound, EqNoBound, PartialEqNoBound,
  traits::{ConstU32, Get},
};
use scale_info::TypeInfo;

/// Closed outcome metric catalog. Values are episode-local policy inputs, not lifetime telemetry.
#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  PartialEq,
  Ord,
  PartialOrd,
  TypeInfo,
  MaxEncodedLen,
)]
pub enum BehaviorMetric {
  #[codec(index = 0)]
  PreconditionPassTotal,
  #[codec(index = 1)]
  PreconditionPassStreak,
  #[codec(index = 2)]
  PreconditionRejectTotal,
  #[codec(index = 3)]
  PreconditionRejectStreak,
  #[codec(index = 4)]
  ActionSuccessTotal,
  #[codec(index = 5)]
  ActionSuccessStreak,
  #[codec(index = 6)]
  ActionFailureTotal,
  #[codec(index = 7)]
  ActionFailureStreak,
  #[codec(index = 8)]
  CycleCompletedTotal,
  #[codec(index = 9)]
  CycleCleanTotal,
}

#[derive(
  Clone,
  Copy,
  Debug,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  PartialEq,
  Ord,
  PartialOrd,
  TypeInfo,
  MaxEncodedLen,
)]
pub struct BehaviorAtom {
  pub metric: BehaviorMetric,
  pub threshold: u32,
}

/// Nonempty canonical DNF: outer clauses are OR, inner atoms are AND.
#[derive(
  CloneNoBound,
  DebugNoBound,
  EqNoBound,
  PartialEqNoBound,
  Decode,
  DecodeWithMemTracking,
  Encode,
  TypeInfo,
  MaxEncodedLen,
)]
#[scale_info(skip_type_params(MaxClauses, MaxAtomsPerClause))]
pub struct CloseWhen<MaxClauses: Get<u32>, MaxAtomsPerClause: Get<u32>> {
  pub clauses: BoundedVec<BoundedVec<BehaviorAtom, MaxAtomsPerClause>, MaxClauses>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BehaviorPolicyError {
  InvalidBounds,
  ExceedsBounds,
  EmptyExpression,
  EmptyClause,
  ZeroThreshold,
  NonCanonicalAtoms,
  NonCanonicalClauses,
  RedundantClause,
  PlanMismatch,
  StateMismatch,
  InvalidObservation,
  ReplacementRequired,
}

/// Borrowed validation witness; it cannot outlive or mutate the checked expression.
pub struct ValidatedCloseWhen<'a, MaxClauses: Get<u32>, MaxAtomsPerClause: Get<u32>> {
  expression: &'a CloseWhen<MaxClauses, MaxAtomsPerClause>,
  limits: (u32, u32),
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct BehaviorPolicyBinding {
  pub semantic_contract_id: [u8; 32],
  pub admission_identity: [u8; 32],
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub enum BehaviorTrackingMode {
  #[codec(index = 0)]
  CurrentOutcome,
  #[codec(index = 1)]
  Accumulated,
}

/// Derived bounded policy requirements, not an observation history or independent authority.
#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct BehaviorPolicyPlan {
  identity: [u8; 32],
  mode: BehaviorTrackingMode,
  metrics: BoundedVec<BehaviorAtom, ConstU32<10>>,
}

/// Cold authored-policy owner; every load verifies its derived plan against the Contract head.
#[derive(
  CloneNoBound,
  DebugNoBound,
  EqNoBound,
  PartialEqNoBound,
  Decode,
  DecodeWithMemTracking,
  Encode,
  TypeInfo,
  MaxEncodedLen,
)]
#[scale_info(skip_type_params(MaxClauses, MaxAtomsPerClause))]
pub struct ActorBehaviorPolicy<MaxClauses: Get<u32>, MaxAtomsPerClause: Get<u32>> {
  pub expression: CloseWhen<MaxClauses, MaxAtomsPerClause>,
  pub plan: BehaviorPolicyPlan,
  pub episode_started_at_generation: super::ActorGeneration,
}

impl BehaviorPolicyPlan {
  pub fn mode(&self) -> BehaviorTrackingMode {
    self.mode
  }

  pub fn metrics(&self) -> &[BehaviorAtom] {
    &self.metrics
  }

  pub fn identity(&self) -> [u8; 32] {
    self.identity
  }

  pub fn cap(&self, metric: BehaviorMetric) -> Option<u32> {
    let atom = self.metrics.iter().find(|atom| atom.metric == metric)?;
    Some(atom.threshold)
  }

  pub fn requires_clean_cycle(&self) -> bool {
    self.cap(BehaviorMetric::CycleCleanTotal).is_some()
  }

  pub fn verify<MaxClauses: Get<u32>, MaxAtomsPerClause: Get<u32>>(
    &self,
    expression: &CloseWhen<MaxClauses, MaxAtomsPerClause>,
    binding: BehaviorPolicyBinding,
  ) -> Result<(), BehaviorPolicyError> {
    self.bind(expression, binding).map(|_| ())
  }

  pub fn bind<'a, MaxClauses: Get<u32>, MaxAtomsPerClause: Get<u32>>(
    &'a self,
    expression: &'a CloseWhen<MaxClauses, MaxAtomsPerClause>,
    binding: BehaviorPolicyBinding,
  ) -> Result<BoundBehaviorPolicy<'a, MaxClauses, MaxAtomsPerClause>, BehaviorPolicyError> {
    let expression = expression.validate()?;
    if *self != expression.compile(binding)? {
      return Err(BehaviorPolicyError::PlanMismatch);
    }
    Ok(BoundBehaviorPolicy {
      plan: self,
      expression,
      binding,
    })
  }
}

/// Verified source/plan context; the host remains responsible for authenticating binding inputs.
pub struct BoundBehaviorPolicy<'a, MaxClauses: Get<u32>, MaxAtomsPerClause: Get<u32>> {
  plan: &'a BehaviorPolicyPlan,
  expression: ValidatedCloseWhen<'a, MaxClauses, MaxAtomsPerClause>,
  binding: BehaviorPolicyBinding,
}

/// Whole-Step stream results plus an already qualified normal Cycle completion (true = clean).
#[derive(
  Clone,
  Copy,
  Debug,
  Default,
  Decode,
  DecodeWithMemTracking,
  Encode,
  Eq,
  PartialEq,
  TypeInfo,
  MaxEncodedLen,
)]
pub struct BehaviorObservation {
  pub precondition: Option<bool>,
  pub action: Option<bool>,
  pub completion: Option<bool>,
}

/// Transaction-local normal completion; clean-Cycle qualification has a separate owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NormalCycleCompletion {
  PlanEnd,
  EarlyReturn,
}

/// Current-Step evidence for strict clean-Cycle qualification, independent of Action streaks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CleanCycleStep {
  #[default]
  Neutral,
  EconomicSuccess,
  Disqualified,
}

/// Candidate flags for one Cycle; the eventual companion owner must authenticate Cycle scope.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct CleanCycleCandidate {
  possible: bool,
  economic_success: bool,
}

impl Default for CleanCycleCandidate {
  fn default() -> Self {
    Self {
      possible: true,
      economic_success: false,
    }
  }
}

impl CleanCycleCandidate {
  pub fn observe(mut self, step: CleanCycleStep) -> Self {
    match step {
      CleanCycleStep::Neutral => {}
      CleanCycleStep::EconomicSuccess => self.economic_success = true,
      CleanCycleStep::Disqualified => self.possible = false,
    }
    self
  }

  pub fn completion(self, boundary: Option<NormalCycleCompletion>) -> Option<bool> {
    boundary.map(|boundary| {
      boundary == NormalCycleCompletion::PlanEnd && self.possible && self.economic_success
    })
  }
}

/// Capped slot values in the bound plan's metric order; never exact lifetime telemetry.
#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct BehaviorMonitorState {
  plan_identity: [u8; 32],
  episode: [u8; 32],
  values: BoundedVec<u32, ConstU32<10>>,
}

/// Mutable optional owner, allocated only for accumulated tracking or clean-Cycle qualification.
#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct ActorBehaviorMonitor {
  pub state: BehaviorMonitorState,
  pub clean_cycle: Option<BehaviorCleanCycle>,
}

/// One authenticated capped value; totals and streaks are not exact lifetime telemetry.
#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct BehaviorMetricValue {
  pub metric: BehaviorMetric,
  pub value: u32,
}

#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct BehaviorMonitorProjection {
  pub capped_values: BoundedVec<BehaviorMetricValue, ConstU32<10>>,
  pub clean_cycle: Option<BehaviorCleanCycle>,
}

/// Current bound episode and referenced requirements; a stateless policy has no monitor.
#[derive(
  Clone, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct BehaviorEpisodeProjection {
  pub episode_started_at_generation: super::ActorGeneration,
  pub tracking_mode: BehaviorTrackingMode,
  pub requirements: BoundedVec<BehaviorAtom, ConstU32<10>>,
  pub monitor: Option<BehaviorMonitorProjection>,
}

/// Source-qualified snapshot used only inside the enclosing Actor transaction.
pub(crate) struct LoadedBehaviorPolicy<Policy> {
  pub actor: super::ActorRef,
  pub binding: BehaviorPolicyBinding,
  pub policy: Policy,
  pub monitor: Option<ActorBehaviorMonitor>,
}

pub(crate) struct BehaviorPolicyCandidate<Policy> {
  pub source: LoadedBehaviorPolicy<Policy>,
  pub monitor: Option<ActorBehaviorMonitor>,
  pub satisfied: bool,
}

#[derive(
  Clone, Copy, Debug, Decode, DecodeWithMemTracking, Encode, Eq, PartialEq, TypeInfo, MaxEncodedLen,
)]
pub struct BehaviorCleanCycle {
  pub cycle_nonce: u64,
  pub candidate: CleanCycleCandidate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BehaviorReduction {
  pub candidate: BehaviorMonitorState,
  pub satisfied: bool,
}

impl<MaxClauses: Get<u32>, MaxAtomsPerClause: Get<u32>>
  BoundBehaviorPolicy<'_, MaxClauses, MaxAtomsPerClause>
{
  pub fn begin_episode(
    &self,
    episode: [u8; 32],
  ) -> Result<BehaviorMonitorState, BehaviorPolicyError> {
    let mut values: BoundedVec<u32, ConstU32<10>> = Default::default();
    if self.plan.mode == BehaviorTrackingMode::Accumulated {
      for _ in &self.plan.metrics {
        values
          .try_push(0)
          .map_err(|_| BehaviorPolicyError::ExceedsBounds)?;
      }
    }
    Ok(BehaviorMonitorState {
      plan_identity: self.plan.identity,
      episode,
      values,
    })
  }

  pub fn validate_state(
    &self,
    state: &BehaviorMonitorState,
    episode: [u8; 32],
  ) -> Result<(), BehaviorPolicyError> {
    let slots = if self.plan.mode == BehaviorTrackingMode::Accumulated {
      self.plan.metrics.len()
    } else {
      0
    };
    if state.plan_identity != self.plan.identity
      || state.episode != episode
      || state.values.len() != slots
    {
      return Err(BehaviorPolicyError::StateMismatch);
    }
    for (value, cap) in state.values.iter().zip(&self.plan.metrics) {
      if *value > cap.threshold {
        return Err(BehaviorPolicyError::StateMismatch);
      }
    }
    Ok(())
  }

  fn metric_index(&self, metric: BehaviorMetric) -> Option<usize> {
    let metrics = &self.plan.metrics;
    metrics.iter().position(|entry| entry.metric == metric)
  }

  pub fn value(
    &self,
    state: &BehaviorMonitorState,
    episode: [u8; 32],
    metric: BehaviorMetric,
  ) -> Result<Option<u32>, BehaviorPolicyError> {
    self.validate_state(state, episode)?;
    if self.plan.mode == BehaviorTrackingMode::CurrentOutcome {
      return Ok(None);
    }
    Ok(self.metric_index(metric).map(|index| state.values[index]))
  }

  pub fn reduce(
    &self,
    state: &BehaviorMonitorState,
    episode: [u8; 32],
    observation: BehaviorObservation,
  ) -> Result<BehaviorReduction, BehaviorPolicyError> {
    self.validate_state(state, episode)?;
    if observation.precondition == Some(false) && observation.action.is_some()
      || observation.action == Some(false) && observation.completion == Some(true)
    {
      return Err(BehaviorPolicyError::InvalidObservation);
    }
    let mut candidate = state.clone();
    if self.plan.mode == BehaviorTrackingMode::Accumulated {
      for (index, cap) in self.plan.metrics.iter().enumerate() {
        candidate.values[index] = advance_behavior_metric(
          cap.metric,
          candidate.values[index],
          cap.threshold,
          observation,
        );
      }
    }
    let satisfied = self.expression.matches(|metric| {
      if self.plan.mode == BehaviorTrackingMode::CurrentOutcome {
        advance_behavior_metric(metric, 0, 1, observation)
      } else {
        self
          .metric_index(metric)
          .map_or(0, |index| candidate.values[index])
      }
    });
    Ok(BehaviorReduction {
      candidate,
      satisfied,
    })
  }

  pub fn rebind_episode(
    &self,
    previous: &Self,
    state: &BehaviorMonitorState,
    episode: [u8; 32],
  ) -> Result<BehaviorMonitorState, BehaviorPolicyError> {
    previous.validate_state(state, episode)?;
    if self.binding.semantic_contract_id != previous.binding.semantic_contract_id
      || self.expression.expression != previous.expression.expression
      || self.plan.metrics != previous.plan.metrics
      || self.plan.mode != previous.plan.mode
    {
      return Err(BehaviorPolicyError::ReplacementRequired);
    }
    let mut candidate = state.clone();
    candidate.plan_identity = self.plan.identity;
    self.validate_state(&candidate, episode)?;
    Ok(candidate)
  }
}

fn advance_behavior_metric(
  metric: BehaviorMetric,
  previous: u32,
  cap: u32,
  observation: BehaviorObservation,
) -> u32 {
  use BehaviorMetric::*;
  let increment = || previous.saturating_add(1).min(cap);
  let total = |stream: Option<bool>, expected| {
    if stream == Some(expected) {
      increment()
    } else {
      previous
    }
  };
  let streak = |stream: Option<bool>, expected| match stream {
    Some(result) if result == expected => increment(),
    Some(_) => 0,
    None => previous,
  };
  match metric {
    PreconditionPassTotal => total(observation.precondition, true),
    PreconditionPassStreak => streak(observation.precondition, true),
    PreconditionRejectTotal => total(observation.precondition, false),
    PreconditionRejectStreak => streak(observation.precondition, false),
    ActionSuccessTotal => total(observation.action, true),
    ActionSuccessStreak => streak(observation.action, true),
    ActionFailureTotal => total(observation.action, false),
    ActionFailureStreak => streak(observation.action, false),
    CycleCompletedTotal => {
      if observation.completion.is_some() {
        increment()
      } else {
        previous
      }
    }
    CycleCleanTotal => total(observation.completion, true),
  }
}

impl<MaxClauses: Get<u32>, MaxAtomsPerClause: Get<u32>> CloseWhen<MaxClauses, MaxAtomsPerClause> {
  pub fn validate(
    &self,
  ) -> Result<ValidatedCloseWhen<'_, MaxClauses, MaxAtomsPerClause>, BehaviorPolicyError> {
    let max_clauses = MaxClauses::get();
    let max_atoms = MaxAtomsPerClause::get();
    if max_clauses == 0 || max_atoms == 0 || max_clauses.checked_mul(max_atoms).is_none() {
      return Err(BehaviorPolicyError::InvalidBounds);
    }
    if self.clauses.len() > max_clauses as usize {
      return Err(BehaviorPolicyError::ExceedsBounds);
    }
    if self.clauses.is_empty() {
      return Err(BehaviorPolicyError::EmptyExpression);
    }
    for clause in &self.clauses {
      if clause.len() > max_atoms as usize {
        return Err(BehaviorPolicyError::ExceedsBounds);
      }
      if clause.is_empty() {
        return Err(BehaviorPolicyError::EmptyClause);
      }
      if clause.iter().any(|atom| atom.threshold == 0) {
        return Err(BehaviorPolicyError::ZeroThreshold);
      }
      if clause
        .windows(2)
        .any(|pair| pair[0].metric >= pair[1].metric)
      {
        return Err(BehaviorPolicyError::NonCanonicalAtoms);
      }
    }
    if self
      .clauses
      .windows(2)
      .any(|pair| pair[0].as_slice() >= pair[1].as_slice())
    {
      return Err(BehaviorPolicyError::NonCanonicalClauses);
    }
    for (strong_index, strong) in self.clauses.iter().enumerate() {
      for (weak_index, weak) in self.clauses.iter().enumerate() {
        if strong_index != weak_index && clause_implies(strong, weak) {
          return Err(BehaviorPolicyError::RedundantClause);
        }
      }
    }
    Ok(ValidatedCloseWhen {
      expression: self,
      limits: (max_clauses, max_atoms),
    })
  }
}

/// Only same-metric threshold implication is considered; there is no general boolean minimizer.
fn clause_implies<MaxAtoms: Get<u32>>(
  strong: &BoundedVec<BehaviorAtom, MaxAtoms>,
  weak: &BoundedVec<BehaviorAtom, MaxAtoms>,
) -> bool {
  weak.iter().all(|required| {
    strong.iter().any(|provided| {
      provided.metric == required.metric && provided.threshold >= required.threshold
    })
  })
}

impl<MaxClauses: Get<u32>, MaxAtomsPerClause: Get<u32>>
  ValidatedCloseWhen<'_, MaxClauses, MaxAtomsPerClause>
{
  pub fn compile(
    &self,
    binding: BehaviorPolicyBinding,
  ) -> Result<BehaviorPolicyPlan, BehaviorPolicyError> {
    let mut metrics: BoundedVec<BehaviorAtom, ConstU32<10>> = Default::default();
    let mut current_outcome_only = true;
    for clause in &self.expression.clauses {
      current_outcome_only &= clause.len() == 1;
      for atom in clause {
        current_outcome_only &= atom.threshold == 1;
        if let Some(existing) = metrics.iter_mut().find(|entry| entry.metric == atom.metric) {
          existing.threshold = existing.threshold.max(atom.threshold);
        } else {
          metrics
            .try_push(*atom)
            .map_err(|_| BehaviorPolicyError::ExceedsBounds)?;
        }
      }
    }
    metrics.sort_by_key(|atom| atom.metric);
    let mode = if current_outcome_only {
      BehaviorTrackingMode::CurrentOutcome
    } else {
      BehaviorTrackingMode::Accumulated
    };
    let identity = (
      *b"DEOS_ACTOR_BEHAVIOR_PLAN",
      binding,
      self.limits,
      self.expression,
    )
      .using_encoded(frame::hashing::blake2_256);
    Ok(BehaviorPolicyPlan {
      identity,
      mode,
      metrics,
    })
  }

  /// Visit every authored atom in canonical order, without value-dependent short-circuiting.
  /// The caller supplies stable metric values; no observations or storage are read here.
  pub fn matches(&self, mut value: impl FnMut(BehaviorMetric) -> u32) -> bool {
    let mut expression_matches = false;
    for clause in &self.expression.clauses {
      let mut clause_matches = true;
      for atom in clause {
        clause_matches &= value(atom.metric) >= atom.threshold;
      }
      expression_matches |= clause_matches;
    }
    expression_matches
  }
}
