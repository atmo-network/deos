//! Test/benchmark-only tariff candidates, never installed authority or a live fee route.

use polkadot_sdk::{sp_runtime::DispatchError, sp_weights::Weight};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PipelineTariffCandidate<Balance> {
  /// Fee-only declaration: System zero does not mean zero physical admission work.
  pub control_weight: Weight,
  pub pipeline_machine_fee: Balance,
  pub candidate_identity: [u8; 32],
}

/// Plain Rust diagnostics, deliberately absent from pallet metadata and economic failure policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PipelineTariffCandidateError {
  InvalidContract(DispatchError),
  NonCanonicalSteps,
  UnsupportedUnfundedPolicy,
  AuthorityUnavailable,
  TariffUnavailable,
}
