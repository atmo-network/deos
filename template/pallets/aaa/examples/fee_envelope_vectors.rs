use pallet_aaa::{
  AaaType, FeeChargeKind, FeeEnvelopeInput, compose_attempt_fee_envelope,
  fee_native_protected_minimum, settle_attempt_fee_step,
};
use polkadot_sdk::frame_support::{BoundedVec, traits::ConstU32};
use serde::Serialize;
use std::{env, fs, path::Path};

use frame::hashing::sha2_256;

type Inputs = BoundedVec<FeeEnvelopeInput<u128>, ConstU32<4>>;

fn hex(bytes: &[u8]) -> String {
  bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Hash a committed runtime evidence file into a hex identity. The metadata and
/// AAA weights files are the final release candidate's artifacts; a missing file
/// fails closed so the fee-envelope binding cannot silently unbind.
fn file_identity(path: &Path) -> String {
  let bytes = fs::read(path).unwrap_or_else(|error| {
    panic!(
      "fee-envelope evidence file {} is unreadable: {error}",
      path.display()
    )
  });
  hex(&sha2_256(&bytes))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FeeEnvelopeVectors {
  format: &'static str,
  format_version: u32,
  metadata_sha256: String,
  weight_sha256: String,
  vectors: Vec<FeeEnvelopeVector>,
  settlement_cases: Vec<FeeSettlementCase>,
  floor_cases: Vec<FeeFloorCase>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FeeEnvelopeVector {
  actor_type: &'static str,
  cursor: usize,
  inputs: Vec<FeeInput>,
  steps: Vec<FeeStep>,
  total: String,
}

#[derive(Serialize)]
struct FeeInput {
  evaluation: String,
  execution: String,
}

#[derive(Serialize)]
struct FeeStep {
  evaluation: String,
  execution: String,
  total: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FeeSettlementCase {
  name: &'static str,
  actor_type: &'static str,
  cursor: usize,
  inputs: Vec<FeeInput>,
  initial_reservation: String,
  charge_kinds: Vec<&'static str>,
  charges: Vec<String>,
  reservation_remaining: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FeeFloorCase {
  name: &'static str,
  actor_type: &'static str,
  is_fee_native: bool,
  asset_minimum: String,
  min_user_balance: String,
  protected_minimum: String,
}

fn inputs() -> Inputs {
  BoundedVec::try_from(vec![
    FeeEnvelopeInput {
      evaluation: 2,
      execution: 100,
    },
    FeeEnvelopeInput {
      evaluation: 5,
      execution: 7,
    },
    FeeEnvelopeInput {
      evaluation: 0,
      execution: 33,
    },
  ])
  .expect("fee-envelope vector inputs fit")
}

fn rollback_inputs() -> Inputs {
  BoundedVec::try_from(vec![FeeEnvelopeInput {
    evaluation: 2,
    execution: 100,
  }])
  .expect("rollback vector input fits")
}

fn rendered_inputs(inputs: Inputs) -> Vec<FeeInput> {
  inputs
    .into_iter()
    .map(|input| FeeInput {
      evaluation: input.evaluation.to_string(),
      execution: input.execution.to_string(),
    })
    .collect()
}

fn actor_type_name(aaa_type: AaaType) -> &'static str {
  match aaa_type {
    AaaType::User => "User",
    AaaType::System => "System",
  }
}

fn charge_kind_name(charge_kind: FeeChargeKind) -> &'static str {
  match charge_kind {
    FeeChargeKind::EvaluationOnly => "EvaluationOnly",
    FeeChargeKind::Attempted => "Attempted",
  }
}

fn vector(aaa_type: AaaType, cursor: usize) -> FeeEnvelopeVector {
  let inputs = inputs();
  let envelope = compose_attempt_fee_envelope(aaa_type, &inputs, cursor)
    .expect("vector input has a valid checked envelope");
  FeeEnvelopeVector {
    actor_type: actor_type_name(aaa_type),
    cursor,
    inputs: rendered_inputs(inputs),
    steps: envelope
      .steps
      .into_iter()
      .map(|step| FeeStep {
        evaluation: step.evaluation.to_string(),
        execution: step.execution.to_string(),
        total: step.total.to_string(),
      })
      .collect(),
    total: envelope.total.to_string(),
  }
}

fn settlement_case(
  name: &'static str,
  aaa_type: AaaType,
  inputs: Inputs,
  cursor: usize,
  charge_kinds: &[FeeChargeKind],
) -> FeeSettlementCase {
  let envelope = compose_attempt_fee_envelope(aaa_type, &inputs, cursor)
    .expect("settlement vector input has a checked envelope");
  assert_eq!(envelope.steps.len(), charge_kinds.len());
  let initial_reservation = envelope.total;
  let mut reservation = initial_reservation;
  let mut charges = Vec::with_capacity(envelope.steps.len());
  let mut reservation_remaining = Vec::with_capacity(envelope.steps.len());
  for (index, step) in envelope.steps.into_iter().enumerate() {
    let settlement = settle_attempt_fee_step(aaa_type, reservation, &step, charge_kinds[index])
      .expect("settlement vector preserves its reservation");
    reservation = settlement.reservation_remaining;
    charges.push(settlement.charged.to_string());
    reservation_remaining.push(reservation.to_string());
  }
  assert_eq!(reservation, 0);
  FeeSettlementCase {
    name,
    actor_type: actor_type_name(aaa_type),
    cursor,
    inputs: rendered_inputs(inputs),
    initial_reservation: initial_reservation.to_string(),
    charge_kinds: charge_kinds
      .into_iter()
      .map(|charge_kind| charge_kind_name(*charge_kind))
      .collect(),
    charges,
    reservation_remaining,
  }
}

fn floor_case(
  name: &'static str,
  aaa_type: AaaType,
  is_fee_native: bool,
  asset_minimum: u128,
  min_user_balance: u128,
) -> FeeFloorCase {
  FeeFloorCase {
    name,
    actor_type: actor_type_name(aaa_type),
    is_fee_native,
    asset_minimum: asset_minimum.to_string(),
    min_user_balance: min_user_balance.to_string(),
    protected_minimum: fee_native_protected_minimum(
      aaa_type,
      is_fee_native,
      asset_minimum,
      min_user_balance,
    )
    .to_string(),
  }
}

fn manifest() -> FeeEnvelopeVectors {
  FeeEnvelopeVectors {
    format: "deos.aaa.fee-envelope-vectors",
    format_version: 2,
    metadata_sha256: file_identity(Path::new("../web-client/.papi/metadata/deos.scale")),
    weight_sha256: file_identity(Path::new("runtime/src/weights/pallet_aaa.rs")),
    vectors: vec![
      vector(AaaType::User, 0),
      vector(AaaType::User, 1),
      vector(AaaType::User, 3),
      vector(AaaType::System, 0),
    ],
    settlement_cases: vec![
      settlement_case(
        "releaseToZero",
        AaaType::User,
        inputs(),
        0,
        &[
          FeeChargeKind::EvaluationOnly,
          FeeChargeKind::Attempted,
          FeeChargeKind::EvaluationOnly,
        ],
      ),
      settlement_case(
        "attemptPricedRollback",
        AaaType::User,
        rollback_inputs(),
        0,
        &[FeeChargeKind::Attempted],
      ),
      settlement_case(
        "systemFeeExemption",
        AaaType::System,
        rollback_inputs(),
        0,
        &[FeeChargeKind::Attempted],
      ),
    ],
    floor_cases: vec![
      floor_case("userFeeNative", AaaType::User, true, 1, 50),
      floor_case("userNonFeeNative", AaaType::User, false, 1, 50),
      floor_case("systemFeeNative", AaaType::System, true, 100, 50),
    ],
  }
}

fn main() {
  let rendered = serde_json::to_string(&manifest()).expect("vectors serialize") + "\n";
  let args = env::args().skip(1).collect::<Vec<_>>();
  match args.as_slice() {
    [] => print!("{rendered}"),
    [flag, path] if flag == "--check" => {
      let actual = fs::read_to_string(Path::new(path)).expect("vector artifact is readable");
      assert_eq!(actual, rendered, "fee-envelope vector artifact is stale");
    }
    _ => {
      panic!("usage: cargo run -p pallet-deos-aaa --example fee_envelope_vectors -- [--check PATH]")
    }
  }
}
