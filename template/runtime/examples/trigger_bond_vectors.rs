use std::{env, fs, path::Path};

use deos_runtime::{Balance, Runtime};
use pallet_deos_actors::{
  TriggerOf,
  types::{AssetFilter, CrossingDirection, SourceFilter, Trigger},
};
use serde_json::json;

fn hex(bytes: &[u8]) -> String {
  bytes.iter().map(|byte| format!("{byte:02x}")).collect() // deos-bypass: bounded-iter -- fixed SHA-256 digest
}

fn file_identity(path: &Path) -> String {
  let bytes = fs::read(path)
    .unwrap_or_else(|error| panic!("failed to read identity input {}: {error}", path.display()));
  hex(&polkadot_sdk::sp_io::hashing::sha2_256(&bytes))
}

fn amount(trigger: TriggerOf<Runtime>) -> Balance {
  deos_runtime::actors_trigger_state_bond(&trigger)
}

fn main() {
  let mut args = env::args().skip(1).collect::<Vec<_>>();
  let check = args.first().is_some_and(|arg| arg == "--check");
  if check {
    args.remove(0);
  }
  assert_eq!(
    args.len(),
    1,
    "usage: trigger_bond_vectors [--check] <output>"
  );
  let output = args.remove(0);
  let output = Path::new(&output);
  let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let metadata = repository.join("web-client/.papi/metadata/deos.scale");
  let weights = repository.join("template/runtime/src/weights/pallet_deos_actors.rs");
  let feed = deos_runtime::actors_trigger_bond_vector_feed();
  let vectors = [
    ("Manual", amount(Trigger::manual())),
    (
      "AddressEvent",
      amount(Trigger::address_event(SourceFilter::Any, AssetFilter::Any)),
    ),
    (
      "ObservationChange",
      amount(Trigger::observation_change(feed)),
    ),
    (
      "ObservationCrossing",
      amount(Trigger::observation_crossing(
        feed,
        CrossingDirection::Rising,
        2,
        1,
      )),
    ),
    ("Cadenced", amount(Trigger::cadenced(1))),
  ];
  let value = json!({
    "formatVersion": 1,
    "metadataSha256": file_identity(&metadata),
    "actorsWeightSha256": file_identity(&weights),
    "vectors": vectors.into_iter().map(|(trigger_family, amount)| json!({
      "triggerFamily": trigger_family,
      "amount": amount.to_string(),
    })).collect::<Vec<_>>(),
  });
  let encoded = format!(
    "{}\n",
    serde_json::to_string_pretty(&value).expect("JSON encoding succeeds")
  );
  if check {
    let current = fs::read_to_string(output)
      .unwrap_or_else(|error| panic!("failed to read {}: {error}", output.display()));
    assert_eq!(current, encoded, "Trigger bond vectors are stale");
  } else {
    if let Some(parent) = output.parent() {
      fs::create_dir_all(parent).expect("output parent creation succeeds");
    }
    fs::write(output, encoded).expect("Trigger bond vector write succeeds");
  }
}
