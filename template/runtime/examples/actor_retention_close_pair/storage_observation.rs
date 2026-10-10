//! Native storage observations outside runtime recording; no resource bound is inferred.

use super::{Blake2Hasher, TestExternalities, digest, recorded_keys};
use polkadot_sdk::{
  frame_support::traits::StorageInfoTrait, sp_genesis_builder::PresetId, sp_state_machine::Backend,
  sp_trie::recorder::Recorder,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn key_image(keys: &BTreeSet<Vec<u8>>) -> Vec<String> {
  assert!(
    keys.len() < 10_000,
    "the fixed five-extrinsic key image is bounded"
  );
  keys
    .iter() // deos-bypass: bounded-iter -- the guarded fixed five-extrinsic backend-key image
    .map(|key| polkadot_sdk::sp_core::hexdisplay::HexDisplay::from(key).to_string())
    .collect()
}

#[derive(Default)]
struct Population {
  entries: usize,
  maximum_key_bytes: usize,
  maximum_value_bytes: usize,
}

pub(super) fn record_pre_state(state: &TestExternalities, preset: &PresetId) {
  println!("pair pre-state storage inventory begins: preset={preset}");
  let infos = deos_runtime::AllPalletsWithSystem::storage_info();
  assert!(infos.len() < 512);
  let mut names = BTreeMap::new();
  let mut populations = BTreeMap::<Vec<u8>, Population>::new();
  for info in infos {
    let owner = format!(
      "{}::{}",
      String::from_utf8(info.pallet_name).unwrap(),
      String::from_utf8(info.storage_name).unwrap(),
    );
    assert!(names.insert(info.prefix.clone(), owner).is_none());
    populations.insert(info.prefix, Population::default());
  }
  let mut total = 0usize;
  // The fixed four-source corpus has fewer than three million main-trie entries.
  for entry in state.backend.pairs(Default::default()).unwrap() {
    let (key, value) = entry.unwrap();
    total += 1;
    assert!(
      total < 3_000_000,
      "inventory is limited to the fixed corpus"
    );
    let prefix = key[..key.len().min(32)].to_vec();
    let population = populations.entry(prefix).or_default();
    population.entries += 1;
    population.maximum_key_bytes = population.maximum_key_bytes.max(key.len());
    population.maximum_value_bytes = population.maximum_value_bytes.max(value.len());
  }
  let rows = populations
    .into_iter() // deos-bypass: bounded-iter -- compiled namespaces plus the fixed-corpus trie prefixes
    .map(|(prefix, population)| {
      let prefix_hex = polkadot_sdk::sp_core::hexdisplay::HexDisplay::from(&prefix).to_string();
      json!({"owner": names.get(&prefix), "prefix": prefix_hex,
        "entries": population.entries, "maximumKeyBytes": population.maximum_key_bytes,
        "maximumValueBytes": population.maximum_value_bytes})
    })
    .collect::<Vec<_>>();
  println!(
    "{}",
    json!({"scope": "native_close_pre_state_population", "preset": preset,
      "entries": total, "namespaces": rows,
      "scopeLimit": "One synthetic prepared parent, outside Recorder; no horizon or universal population proof."})
  );
}

pub(super) fn record_delta(
  state: &TestExternalities,
  preset: &PresetId,
  caller: &str,
  recorder: &Recorder<Blake2Hasher>,
) -> Value {
  let overlay = state.overlayed_changes();
  assert!(overlay.children().next().is_none());
  assert!(overlay.changes().count() < 10_000);
  // Materialize a clone so observing Append values cannot change the executed overlay.
  let mut copy = overlay.clone();
  let rows = copy
    .changes_mut() // deos-bypass: bounded-iter -- guarded fixed-block overlay, outside Recorder
    .map(|(key, entry)| {
      let original = state.backend.storage(key).unwrap();
      let value = entry.value();
      json!({"key": polkadot_sdk::sp_core::hexdisplay::HexDisplay::from(key).to_string(),
        "originalPresent": original.is_some(), "resultPresent": value.is_some(),
        "originalValueBytes": original.as_ref().map(Vec::len),
        "resultValueBytes": value.map(Vec::len),
        "originalValueSha256": original.as_ref().map(|bytes| digest(bytes)),
        "resultValueSha256": value.map(|bytes| digest(bytes)),
        "unchangedValue": original.as_ref() == value})
    })
    .collect::<Vec<_>>();
  let delta = json!(rows);
  println!(
    "{}",
    json!({"scope": "native_close_complete_storage_footprint", "preset": preset,
      "caller": caller, "recordedBackendKeys": key_image(&recorded_keys(recorder)),
      "netOverlay": delta, "scopeLimit": "Final overlay and Recorder key image; transient writes, physical cost and universal admission are separate."})
  );
  delta
}
