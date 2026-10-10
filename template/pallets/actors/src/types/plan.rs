#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepExecutionPlan<Identity, Hot, Run, Admission, Ticket, LoadedStep, Fee> {
  pub identity: Identity,
  pub hot: Hot,
  pub run: Option<Run>,
  pub admission: Admission,
  pub ticket: Ticket,
  pub loaded_step: LoadedStep,
  pub maximum_fee: Fee,
  /// Transient outcome emitted by the current Step owner, never persisted as another Run owner.
  pub last_step_outcome: Option<super::lifecycle::StepOutcome>,
  /// Unpaid service selected before invocation; this carries no Step outcome or observation.
  pub financial_wait: Option<super::financial_wait::UnpaidActorService>,
  /// Candidate stream observations; only the enclosing transition can commit them.
  pub behavior_observation: super::behavior::BehaviorObservation,
  pub normal_cycle_completion: Option<super::behavior::NormalCycleCompletion>,
  pub clean_cycle_step: super::behavior::CleanCycleStep,
}
