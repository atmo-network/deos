#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepExecutionPlan<Identity, Hot, Run, Funding, Admission, Ticket, LoadedStep, Fee> {
  pub identity: Identity,
  pub hot: Hot,
  pub run: Option<Run>,
  pub funding: Funding,
  pub admission: Admission,
  pub ticket: Ticket,
  pub loaded_step: LoadedStep,
  pub maximum_fee: Fee,
}
