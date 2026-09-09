use pallet_deos_actors::{
  ActorType, FeeAssetClass, FeeChargeKind, FeeEnvelopeInput, compose_attempt_fee_envelope,
  fee_native_protected_minimum, settle_attempt_fee_step,
};
use polkadot_sdk::frame_support::{BoundedVec, traits::ConstU32};
use serde::Serialize;
use std::{env, fs, path::Path};

use frame::hashing::sha2_256;

type Inputs = BoundedVec<FeeEnvelopeInput<u128>, ConstU32<4>>;

fn hex(bytes: &[u8]) -> String {
  bytes.iter().map(|byte| format!("{byte:02x}")).collect() // deos-bypass: bounded-iter -- fixed SHA-256 digest
}

/// Hash a caller-supplied runtime evidence file into a hex identity. The metadata and
/// Actors weights files identify the embedding runtime's artifacts; a missing file
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

fn actor_type_name(actor_type: ActorType) -> &'static str {
  match actor_type {
    ActorType::User => "User",
    ActorType::System => "System",
  }
}

fn charge_kind_name(charge_kind: FeeChargeKind) -> &'static str {
  match charge_kind {
    FeeChargeKind::EvaluationOnly => "EvaluationOnly",
    FeeChargeKind::Attempted => "Attempted",
  }
}

fn vector(actor_type: ActorType, cursor: usize) -> FeeEnvelopeVector {
  let inputs = inputs();
  let envelope = compose_attempt_fee_envelope(actor_type, &inputs, cursor)
    .expect("vector input has a valid checked envelope");
  FeeEnvelopeVector {
    actor_type: actor_type_name(actor_type),
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
  actor_type: ActorType,
  inputs: Inputs,
  cursor: usize,
  charge_kinds: &[FeeChargeKind],
) -> FeeSettlementCase {
  let envelope = compose_attempt_fee_envelope(actor_type, &inputs, cursor)
    .expect("settlement vector input has a checked envelope");
  assert_eq!(envelope.steps.len(), charge_kinds.len());
  let initial_reservation = envelope.total;
  let mut reservation = initial_reservation;
  let mut charges = Vec::with_capacity(envelope.steps.len());
  let mut reservation_remaining = Vec::with_capacity(envelope.steps.len());
  for (index, step) in envelope.steps.into_iter().enumerate() {
    let settlement = settle_attempt_fee_step(actor_type, reservation, &step, charge_kinds[index])
      .expect("settlement vector preserves its reservation");
    reservation = settlement.reservation_remaining;
    charges.push(settlement.charged.to_string());
    reservation_remaining.push(reservation.to_string());
  }
  assert_eq!(reservation, 0);
  FeeSettlementCase {
    name,
    actor_type: actor_type_name(actor_type),
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
  actor_type: ActorType,
  is_fee_native: bool,
  asset_minimum: u128,
  min_user_balance: u128,
) -> FeeFloorCase {
  FeeFloorCase {
    name,
    actor_type: actor_type_name(actor_type),
    is_fee_native,
    asset_minimum: asset_minimum.to_string(),
    min_user_balance: min_user_balance.to_string(),
    protected_minimum: fee_native_protected_minimum(
      actor_type,
      if is_fee_native {
        FeeAssetClass::FeeNative
      } else {
        FeeAssetClass::Other
      },
      asset_minimum,
      min_user_balance,
    )
    .to_string(),
  }
}

fn manifest(metadata: &Path, weights: &Path) -> FeeEnvelopeVectors {
  FeeEnvelopeVectors {
    format: "deos.actor.fee-envelope-vectors",
    format_version: 2,
    metadata_sha256: file_identity(metadata),
    weight_sha256: file_identity(weights),
    vectors: vec![
      vector(ActorType::User, 0),
      vector(ActorType::User, 1),
      vector(ActorType::User, 3),
      vector(ActorType::System, 0),
    ],
    settlement_cases: vec![
      settlement_case(
        "releaseToZero",
        ActorType::User,
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
        ActorType::User,
        rollback_inputs(),
        0,
        &[FeeChargeKind::Attempted],
      ),
      settlement_case(
        "systemFeeExemption",
        ActorType::System,
        rollback_inputs(),
        0,
        &[FeeChargeKind::Attempted],
      ),
    ],
    floor_cases: vec![
      floor_case("userFeeNative", ActorType::User, true, 1, 50),
      floor_case("userNonFeeNative", ActorType::User, false, 1, 50),
      floor_case("systemFeeNative", ActorType::System, true, 100, 50),
    ],
  }
}

fn main() {
  let args = env::args().skip(1).collect::<Vec<_>>();
  if matches!(args.as_slice(), [flag] if flag == "--help" || flag == "-h") {
    println!("{USAGE}");
    return;
  }
  let (metadata, weights, output) = parse_args(&args).unwrap_or_else(|error| panic!("{error}"));
  let rendered = serde_json::to_string(&manifest(Path::new(metadata), Path::new(weights)))
    .expect("vectors serialize")
    + "\n";
  match output {
    [] => print!("{rendered}"),
    [path] => {
      fs::write(Path::new(path), rendered).expect("fee-envelope vector artifact is writable");
    }
    [flag, path] if flag == "--check" => {
      let actual = fs::read_to_string(Path::new(path)).expect("vector artifact is readable");
      assert_eq!(actual, rendered, "fee-envelope vector artifact is stale");
    }
    _ => unreachable!("output arguments were validated before reading artifacts"),
  }
}

const USAGE: &str = "usage: cargo run -p pallet-deos-actors --example fee_envelope_vectors -- --metadata METADATA_PATH --weights PRODUCTION_WEIGHTS_PATH [OUTPUT_PATH | --check OUTPUT_PATH]\nBoth evidence inputs are required. Omit OUTPUT_PATH to print JSON; --check compares exact bytes. Relative paths resolve from the current working directory.";

fn parse_args(args: &[String]) -> Result<(&str, &str, &[String]), &'static str> {
  let [metadata_flag, metadata, weights_flag, weights, output @ ..] = args else {
    return Err(USAGE);
  };
  if metadata_flag != "--metadata"
    || weights_flag != "--weights"
    || metadata.is_empty()
    || weights.is_empty()
  {
    return Err(USAGE);
  }
  match output {
    [] => {}
    [path] if !path.is_empty() && !path.starts_with('-') => {}
    [flag, path] if flag == "--check" && !path.is_empty() => {}
    _ => return Err(USAGE),
  }
  Ok((metadata, weights, output))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn evidence_paths_are_explicit_and_output_modes_are_bounded() {
    let inputs = [
      "--metadata",
      "host/metadata.scale",
      "--weights",
      "host/weights.rs",
    ];
    for output in [
      &[][..],
      &["vectors.json"][..],
      &["--check", "vectors.json"][..],
    ] {
      let args = inputs
        .iter()
        .chain(output)
        .map(|arg| (*arg).to_owned())
        .collect::<Vec<_>>();
      let (metadata, weights, actual_output) = parse_args(&args).expect("explicit paths accepted");
      assert_eq!(metadata, "host/metadata.scale");
      assert_eq!(weights, "host/weights.rs");
      assert_eq!(actual_output, output);
    }
    for invalid in [
      vec![],
      vec!["--check", "vectors.json"],
      vec!["--metadata", "file"],
      vec!["--metadata", "file", "--weights", "weights", "--check"],
      vec!["--metadata", "", "--weights", "weights"],
    ] {
      let args = invalid.into_iter().map(str::to_owned).collect::<Vec<_>>();
      assert!(parse_args(&args).is_err());
    }
  }
}
