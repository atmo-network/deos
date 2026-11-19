use super::*;
use polkadot_sdk::polkadot_parachain_primitives::primitives::HeadData;

fn expected() -> ValidationResult {
  ValidationResult {
    head_data: HeadData(vec![1]),
    new_validation_code: None,
    upward_messages: Default::default(),
    horizontal_messages: Default::default(),
    processed_downward_messages: 0,
    hrmp_watermark: 2,
  }
}

fn option_args(suffix: &[&str]) -> Vec<String> {
  ["wasm", "fixtures", "workers", "1"]
    .into_iter()
    .chain(suffix.iter().copied())
    .map(String::from)
    .collect()
}

#[test]
fn optional_trace_arguments_are_unambiguous() {
  let parse = |suffix: &[&str]| {
    let args = option_args(suffix);
    options(&args).map(|options| {
      assert!(!options.positive_only);
      (options.insecure, options.trace_path.map(String::from))
    })
  };
  assert_eq!(parse(&[]).unwrap(), (false, None));
  assert_eq!(
    parse(&["--cpu-trace", "output", "--insecure-local"]).unwrap(),
    (true, Some("output".into()))
  );
  for suffix in [
    vec!["--cpu-trace"],
    vec!["--cpu-trace", "--insecure-local"],
    vec!["--cpu-trace", "a", "--cpu-trace", "b"],
    vec!["--insecure-local", "--insecure-local"],
  ] {
    assert!(parse(&suffix).is_err());
  }
}

#[test]
fn case_selection_flag_is_explicit_and_unique() {
  assert_eq!(options(&option_args(&[])).unwrap(), Options::default());
  for suffix in [
    vec!["--positive-only"],
    vec![
      "--positive-only",
      "--cpu-trace",
      "trace path",
      "--insecure-local",
    ],
    vec![
      "--insecure-local",
      "--cpu-trace",
      "trace path",
      "--positive-only",
    ],
  ] {
    let args = option_args(&suffix);
    let parsed = options(&args).unwrap();
    assert!(parsed.positive_only);
    assert_eq!(parsed.insecure, suffix.contains(&"--insecure-local"));
    assert_eq!(
      parsed.trace_path,
      suffix.contains(&"--cpu-trace").then_some("trace path")
    );
  }
  for suffix in [
    vec!["--positive-only", "--positive-only"],
    vec!["--positive-only=true"],
    vec!["--positive-only", "true"],
    vec!["--unknown"],
  ] {
    assert!(options(&option_args(&suffix)).is_err());
  }
}

#[test]
fn case_selection_requires_one_fixture_and_freezes_its_name() {
  let directory = tempfile::tempdir().unwrap();
  let root = directory.path();
  assert_eq!(
    CaseSelection::new(root, false).unwrap(),
    CaseSelection::Full
  );
  assert!(CaseSelection::Full.presets(root).is_err());
  assert!(CaseSelection::new(root, true).is_err());
  let missing = root.join("missing");
  assert_eq!(
    CaseSelection::new(&missing, false).unwrap(),
    CaseSelection::Full
  );
  assert!(CaseSelection::new(&missing, true).is_err());
  fs::write(root.join("z.params.scale"), []).unwrap();
  fs::write(root.join("unrelated.txt"), []).unwrap();
  let positive = CaseSelection::new(root, true).unwrap();
  assert_eq!(positive, CaseSelection::PositiveOnly("z".into()));
  fs::write(root.join("a.params.scale"), []).unwrap();
  assert!(CaseSelection::new(root, true).is_err());
  assert_eq!(positive.presets(root).unwrap(), ["z"]);
  assert_eq!(CaseSelection::Full.presets(root).unwrap(), ["a", "z"]);
  fs::remove_file(root.join("z.params.scale")).unwrap();
  assert_eq!(positive.presets(root).unwrap(), ["z"]);
  assert!(Fixture::read(root, "z").is_err());
}

// Codec-only inputs for case planning, not cryptographically valid PVF candidates.
fn case_selection_fixture() -> Fixture {
  let parent = Header::new(
    0,
    H256::zero(),
    H256::zero(),
    H256::zero(),
    Default::default(),
  );
  let block = Block {
    header: Header::new(
      1,
      H256::zero(),
      H256::zero(),
      parent.hash(),
      generic::Digest {
        logs: vec![DigestItem::Seal(
          polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
          vec![1; 64],
        )],
      },
    ),
    extrinsics: Vec::new(),
  };
  let data = ParachainBlockData::new(
    vec![block.clone()],
    polkadot_sdk::sp_trie::CompactProof {
      encoded_nodes: vec![vec![7]],
    },
    None,
  );
  let pvd = PersistedValidationData {
    parent_head: HeadData(parent.encode()),
    relay_parent_number: 1,
    relay_parent_storage_root: H256::repeat_byte(2),
    max_pov_size: 10_485_760,
  };
  let mut expected = expected();
  expected.head_data = HeadData(block.header.encode());
  Fixture {
    params: ValidationParams {
      parent_head: pvd.parent_head.clone(),
      block_data: polkadot_sdk::polkadot_parachain_primitives::primitives::BlockData(data.encode()),
      relay_parent_number: pvd.relay_parent_number,
      relay_parent_storage_root: pvd.relay_parent_storage_root,
    },
    pvd,
    expected,
  }
}

fn fixture_encoding(fixture: &Fixture) -> Vec<u8> {
  (&fixture.params, &fixture.pvd, &fixture.expected).encode()
}

#[test]
fn case_selection_full_preserves_order_flags_and_mutations() {
  let fixture = case_selection_fixture();
  let expected = [
    fixture.clone(),
    fixture.corrupt_seal().unwrap(),
    fixture.corrupt_parent().unwrap(),
    fixture.corrupt_proof().unwrap(),
  ];
  let cases = CaseSelection::Full.cases(fixture).unwrap();
  assert_eq!(
    cases
      .iter()
      .map(|(name, _, negative)| (*name, *negative))
      .collect::<Vec<_>>(),
    [
      ("valid", false),
      ("corrupt_seal", true),
      ("parent_root", true),
      ("compact_proof", true),
    ]
  );
  for ((_, actual, _), expected) in cases.iter().zip(expected) {
    assert_eq!(fixture_encoding(actual), fixture_encoding(&expected));
  }
}

#[test]
fn case_selection_positive_only_never_constructs_corruptions() {
  let mut fixture = case_selection_fixture();
  fixture.params.block_data.0.clear();
  assert!(fixture.corrupt_seal().is_err());
  assert!(fixture.corrupt_proof().is_err());
  fixture.pvd.parent_head.0.clear();
  assert!(fixture.corrupt_parent().is_err());
  assert!(CaseSelection::Full.cases(fixture.clone()).is_err());
  let encoded = fixture_encoding(&fixture);
  let cases = CaseSelection::PositiveOnly("one".into())
    .cases(fixture)
    .unwrap();
  assert_eq!(cases.len(), 1);
  assert_eq!(cases[0].0, "valid");
  assert!(!cases[0].2);
  assert_eq!(fixture_encoding(&cases[0].1), encoded);
}

#[test]
fn case_selection_scope_does_not_relabel_full_suite_or_cpu() {
  for record in [
    json!({"kind":"session", "format":"DEOS_PVF_CPU_TRACE_V1", "scope":"child CPU"}),
    json!({"kind":"complete", "acceptance":true}),
  ] {
    assert_eq!(CaseSelection::Full.annotate_scope(record.clone()), record);
    let annotated = CaseSelection::PositiveOnly("one".into()).annotate_scope(record.clone());
    assert_eq!(annotated["case_selection"], "positive_only_single_fixture");
    for (key, value) in record.as_object().unwrap() {
      assert_eq!(&annotated[key], value);
    }
  }
}

#[test]
fn only_exact_result_and_typed_worker_invalid_are_accepted() {
  let expected = expected();
  assert!(check_result(Ok(expected.clone()), &expected, false).is_ok());
  let mut changed = expected.clone();
  changed.hrmp_watermark += 1;
  assert!(check_result(Ok(changed), &expected, false).is_err());
  assert!(check_result(Ok(expected.clone()), &expected, true).is_err());
  assert!(
    check_result(
      Err(ValidationError::Invalid(InvalidCandidate::HardTimeout)),
      &expected,
      true
    )
    .is_err()
  );
  assert!(check_result(Err(ValidationError::ExecutionDeadline), &expected, true).is_err());
  assert!(
    check_result(
      Err(ValidationError::Invalid(
        InvalidCandidate::WorkerReportedInvalid("seal".into())
      )),
      &expected,
      true
    )
    .is_ok()
  );
}

fn proof_umbrella_check(
  proof: &polkadot_sdk::sp_trie::StorageProof,
  root: H256,
  keys: &[Vec<u8>],
) -> serde_json::Value {
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_trie::{LayoutV1, StorageProof, Trie, TrieDBBuilder, recorder::Recorder},
  };

  // The V1 null node is implicit in MemoryDB, even when omitted from the supplied proof.
  let universe = StorageProof::merge([proof.clone(), StorageProof::new([vec![0]])]);
  let upper = universe
    .iter_nodes()
    .try_fold(0usize, |sum, node| sum.checked_add(node.encoded_size()))
    .unwrap();
  let db = proof.to_memory_db::<Blake2Hasher>();
  let recorder = Recorder::<Blake2Hasher>::default();
  {
    let mut recording = recorder.as_trie_recorder(root);
    let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&db, &root)
      .with_recorder(&mut recording)
      .build();
    for key in keys {
      trie.get(key).expect("query must be covered by this proof");
    }
  }
  let first = recorder.estimate_encoded_size();
  {
    let mut recording = recorder.as_trie_recorder(root);
    let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&db, &root)
      .with_recorder(&mut recording)
      .build();
    for key in keys {
      trie.get(key).expect("repeated covered query");
    }
  }
  assert_eq!(first, recorder.estimate_encoded_size());
  let recorded = recorder.drain_storage_proof();
  let supplied = universe.into_nodes();
  assert!(recorded.iter_nodes().all(|node| supplied.contains(node)));
  assert!(first <= upper);
  if let Some(missing) = recorded.iter_nodes().next() {
    let mut incomplete = supplied.clone();
    assert!(incomplete.remove(missing));
    assert!(!recorded.iter_nodes().all(|node| incomplete.contains(node)));
  }
  json!({"decoded_nodes":proof.len(), "full_proof_scale_bytes":proof.encoded_size(),
    "node_universe_upper_bytes":upper, "native_recorded_bytes":first,
    "native_recorded_nodes":recorded.len(), "subset":true, "repeat_stable":true,
    "pvf_execution":false, "ref_time_upper":false, "production_price":false})
}

fn proof_umbrella_cache_constructor_check(
  compact: &polkadot_sdk::sp_trie::CompactProof,
  root: H256,
  keys: &[Vec<u8>],
  cold: &serde_json::Value,
) -> Vec<serde_json::Value> {
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_trie::{
      LayoutV1, NodeCodec, StorageProof, Trie, TrieDBBuilder,
      cache::{CacheSize, SharedTrieCache},
      recorder::Recorder,
    },
  };

  let (proof, decoded_root) = compact.to_storage_proof::<Blake2Hasher>(&root).unwrap();
  assert_eq!(decoded_root, root);
  let universe = StorageProof::merge([proof.clone(), StorageProof::new([vec![0]])]).into_nodes();
  let upper = universe
    .iter()
    .try_fold(0usize, |sum, node| sum.checked_add(node.encoded_size()))
    .unwrap();
  assert_eq!(json!(upper), cold["node_universe_upper_bytes"]);
  let (compact_db, compact_root) = compact.to_memory_db::<Blake2Hasher>(&root).unwrap();
  assert_eq!(compact_root, root);
  let mut expected_values = None;
  let mut reports = Vec::new();
  for (constructor, db) in [
    ("storage_proof", proof.to_memory_db::<Blake2Hasher>()),
    ("compact_proof", compact_db),
  ] {
    let cache = SharedTrieCache::<Blake2Hasher>::new(CacheSize::new(64 * 1024), None);
    let warm_values = {
      let local = cache.local_cache_untrusted();
      let mut node_cache = local.as_trie_db_cache(root);
      let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&db, &root)
        .with_cache(&mut node_cache)
        .build();
      keys
        .iter()
        .map(|key| trie.get(key).unwrap())
        .collect::<Vec<_>>()
    };
    if let Some(expected) = expected_values.as_ref() {
      assert_eq!(&warm_values, expected);
    } else {
      expected_values = Some(warm_values.clone());
    }
    let cached_root = cache
      .peek_node(&root)
      .expect("warm root must survive fixture cache");
    let encoded_root = cached_root.to_encoded::<NodeCodec<Blake2Hasher>>();
    assert!(universe.contains(&encoded_root));
    let recorder = Recorder::<Blake2Hasher>::default();
    assert_eq!(recorder.estimate_encoded_size(), 0);
    {
      let local = cache.local_cache_untrusted();
      let mut node_cache = local.as_trie_db_cache(root);
      let mut recording = recorder.as_trie_recorder(root);
      let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&db, &root)
        .with_cache(&mut node_cache)
        .with_recorder(&mut recording)
        .build();
      for (key, expected) in keys.iter().zip(&warm_values) {
        assert_eq!(&trie.get(key).unwrap(), expected);
      }
    }
    let first = recorder.estimate_encoded_size();
    {
      let local = cache.local_cache_untrusted();
      let mut node_cache = local.as_trie_db_cache(root);
      let mut recording = recorder.as_trie_recorder(root);
      let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&db, &root)
        .with_cache(&mut node_cache)
        .with_recorder(&mut recording)
        .build();
      for (key, expected) in keys.iter().zip(&warm_values) {
        assert_eq!(&trie.get(key).unwrap(), expected);
      }
    }
    assert_eq!(first, recorder.estimate_encoded_size());
    let recorded = recorder.drain_storage_proof();
    assert!(recorded.iter_nodes().all(|node| universe.contains(node)));
    assert!(recorded.iter_nodes().any(|node| node == &encoded_root));
    assert_eq!(json!(first), cold["native_recorded_bytes"]);
    assert!(first as u64 <= cold["node_universe_upper_bytes"].as_u64().unwrap());
    reports.push(
      json!({"constructor":constructor, "decoded_nodes":proof.len(),
      "warm_root_present":true, "reencoded_root_in_universe":true,
      "fresh_native_recorded_bytes":first, "fresh_native_recorded_nodes":recorded.len(),
      "universe_upper_bytes":cold["node_universe_upper_bytes"], "subset":true,
      "cold_warm_counter_equal":true, "duplicate_counter_stable":true,
      "constructor_values_equal":true, "actual_pvf_recorder_executed":false}),
    );
  }
  reports
}

#[test]
fn proof_umbrella_decoded_universe_and_optional_closed_inputs() {
  use polkadot_sdk::{
    sp_core::{Blake2Hasher, Pair},
    sp_trie::{
      LayoutV1, MemoryDB, StorageProof, Trie, TrieDBBuilder, TrieDBMutBuilder, TrieMut,
      empty_trie_root, recorder::Recorder,
    },
  };

  let empty = StorageProof::empty();
  let empty_report = proof_umbrella_check(
    &empty,
    empty_trie_root::<LayoutV1<Blake2Hasher>>(),
    &[b"absent".to_vec()],
  );
  println!("PROOF_UMBRELLA_SYNTHETIC empty {empty_report}");
  let empty_root = empty_trie_root::<LayoutV1<Blake2Hasher>>();
  let empty_compact = empty.to_compact_proof::<Blake2Hasher>(empty_root).unwrap();
  for report in proof_umbrella_cache_constructor_check(
    &empty_compact,
    empty_root,
    &[b"absent".to_vec()],
    &empty_report,
  ) {
    println!("PROOF_UMBRELLA_CACHE empty {report}");
  }

  for (label, values) in [
    ("inline", vec![vec![7; 1], vec![8; 32]]),
    (
      "hashed",
      vec![vec![7; 33], vec![8; 64], vec![9; 64], vec![10; 64]],
    ),
  ] {
    let mut db = MemoryDB::<Blake2Hasher>::default();
    let mut root = H256::default();
    let keys = (1..=values.len())
      .map(|n| vec![(n * 16) as u8])
      .collect::<Vec<_>>();
    {
      let mut trie = TrieDBMutBuilder::<LayoutV1<Blake2Hasher>>::new(&mut db, &mut root).build();
      for (key, value) in keys.iter().zip(&values) {
        trie.insert(key, value).unwrap();
      }
    }
    let source = Recorder::<Blake2Hasher>::default();
    {
      let mut recording = source.as_trie_recorder(root);
      let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&db, &root)
        .with_recorder(&mut recording)
        .build();
      for (key, value) in keys.iter().zip(&values) {
        assert_eq!(trie.get(key).unwrap().as_ref(), Some(value));
      }
      assert!(trie.get(&[0xff]).unwrap().is_none());
    }
    let proof = source.drain_storage_proof();
    let compact = proof.to_compact_proof::<Blake2Hasher>(root).unwrap();
    let (decoded, decoded_root) = compact.to_storage_proof::<Blake2Hasher>(&root).unwrap();
    assert_eq!(root, decoded_root);
    assert!(
      compact
        .to_storage_proof::<Blake2Hasher>(&H256::repeat_byte(0xff))
        .is_err()
    );
    if label == "hashed" {
      assert!(
        decoded.encoded_size() > compact.encoded_size(),
        "compact bytes are not the reconstructed recorder universe"
      );
    }
    let report = proof_umbrella_check(&decoded, root, &keys);
    for cache_report in proof_umbrella_cache_constructor_check(&compact, root, &keys, &report) {
      println!("PROOF_UMBRELLA_CACHE {label} {cache_report}");
    }
    println!(
      "PROOF_UMBRELLA_SYNTHETIC {label} compact_bytes={} {report}",
      compact.encoded_size()
    );
  }

  let Some(path) = std::env::var_os("DEOS_PVF_PROOF_UMBRELLA_FIXTURES") else {
    println!("PROOF_UMBRELLA_ARTIFACTS checked=0 reason=not_requested");
    return;
  };
  let owner = polkadot_sdk::sp_core::sr25519::Pair::from_string("//Alice", None)
    .unwrap()
    .public();
  let account: &[u8] = owner.as_ref();
  let key = [
    polkadot_sdk::sp_io::hashing::twox_128(b"System").as_slice(),
    polkadot_sdk::sp_io::hashing::twox_128(b"Account").as_slice(),
    polkadot_sdk::sp_io::hashing::blake2_128(account).as_slice(),
    account,
  ]
  .concat();
  for name in [
    "development.complete",
    "development.inspection",
    "local_testnet.complete",
    "local_testnet.inspection",
  ] {
    let fixture = Fixture::read(Path::new(&path), name).unwrap();
    let parent = Header::decode_all(&mut fixture.params.parent_head.0.as_slice()).unwrap();
    let data =
      ParachainBlockData::<Block>::decode_all(&mut fixture.params.block_data.0.as_slice()).unwrap();
    assert_eq!(data.blocks().len(), 1);
    let (proof, root) = data
      .proof()
      .to_storage_proof::<Blake2Hasher>(parent.state_root())
      .unwrap();
    let report = proof_umbrella_check(&proof, root, std::slice::from_ref(&key));
    for cache_report in proof_umbrella_cache_constructor_check(
      data.proof(),
      root,
      std::slice::from_ref(&key),
      &report,
    ) {
      println!("PROOF_UMBRELLA_CACHE {name} {cache_report}");
    }
    println!(
      "PROOF_UMBRELLA_ARTIFACT {name} compact_bytes={} {report}",
      data.proof().encoded_size()
    );
  }
}

#[test]
fn proof_umbrella_generated_nodes_require_positive_transaction_origin() {
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_runtime::StateVersion,
    sp_state_machine::{Backend, TrieBackendBuilder},
    sp_trie::{
      LayoutV1, MemoryDB, StorageProof, Trie, TrieDBBuilder, TrieDBMutBuilder, TrieMut,
      recorder::{IgnoredNodes, Recorder},
    },
  };

  let keys = [vec![0x10], vec![0x20], vec![0x30], vec![0x40]];
  let values = [vec![7; 1], vec![8; 64], vec![9; 2], vec![10; 64]];
  let mut db = MemoryDB::<Blake2Hasher>::default();
  let mut root = H256::default();
  {
    let mut trie = TrieDBMutBuilder::<LayoutV1<Blake2Hasher>>::new(&mut db, &mut root).build();
    for (key, value) in keys.iter().zip(&values) {
      trie.insert(key, value).unwrap();
    }
  }
  let initial = Recorder::<Blake2Hasher>::default();
  {
    let mut recording = initial.as_trie_recorder(root);
    let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&db, &root)
      .with_recorder(&mut recording)
      .build();
    for (key, expected) in keys.iter().zip(&values) {
      assert_eq!(trie.get(key).unwrap().as_ref(), Some(expected));
    }
  }
  let input = initial.drain_storage_proof();
  let universe = StorageProof::merge([input.clone(), StorageProof::new([vec![0]])]).into_nodes();
  let upper = universe
    .iter()
    .try_fold(0usize, |sum, node| sum.checked_add(node.encoded_size()))
    .unwrap();
  let original_db = input.to_memory_db::<Blake2Hasher>();
  let replacements = [vec![11; 32], vec![12; 128]];
  let root_recorder = Recorder::<Blake2Hasher>::default();
  let backend = TrieBackendBuilder::<_, Blake2Hasher>::new(&original_db, root)
    .with_recorder(root_recorder.clone())
    .build();
  let delta = keys[..2]
    .iter()
    .zip(&replacements)
    .map(|(key, value)| (key.as_slice(), Some(value.as_slice())));
  let (changed_root, transaction) = backend.storage_root(delta, StateVersion::V1);
  assert_ne!(root, changed_root);
  let root_phase_bytes = root_recorder.estimate_encoded_size();
  let root_phase_proof = root_recorder.to_storage_proof();
  assert!(
    root_phase_bytes > 0,
    "root-generation recording must be observable"
  );
  assert!(root_phase_bytes <= upper);
  assert!(
    root_phase_proof
      .iter_nodes()
      .all(|node| universe.contains(node))
  );
  let mut positive_transaction = transaction.clone();
  let positive = StorageProof::new(
    positive_transaction
      .drain()
      .into_iter()
      .filter_map(|(_, (value, count))| (count > 0).then_some(value)),
  );
  assert!(!positive.is_empty());
  let generated_outside_input = positive
    .iter_nodes()
    .filter(|node| !universe.contains(*node))
    .collect::<Vec<_>>();
  assert!(!generated_outside_input.is_empty());
  assert!(
    root_phase_proof
      .iter_nodes()
      .all(|node| !generated_outside_input.contains(&node))
  );
  let ignored = IgnoredNodes::from_memory_db::<Blake2Hasher, _>(transaction);
  assert!(ignored.is_ignored(&changed_root));
  let new_blob_hash = H256::from(polkadot_sdk::sp_io::hashing::blake2_256(&replacements[1]));
  let old_blob_hash = H256::from(polkadot_sdk::sp_io::hashing::blake2_256(&values[3]));
  assert!(ignored.is_ignored(&new_blob_hash));
  assert!(
    !ignored.is_ignored(&old_blob_hash),
    "untouched input must not be ignored"
  );
  let changed_db =
    StorageProof::merge([input.clone(), positive.clone()]).to_memory_db::<Blake2Hasher>();
  let expected = [&replacements[0], &replacements[1], &values[2], &values[3]];
  let read = |recorder: &Recorder<Blake2Hasher>| {
    let mut recording = recorder.as_trie_recorder(changed_root);
    let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&changed_db, &changed_root)
      .with_recorder(&mut recording)
      .build();
    for (key, value) in keys.iter().zip(expected) {
      assert_eq!(trie.get(key).unwrap().as_ref(), Some(value));
    }
  };

  let unmasked = Recorder::<Blake2Hasher>::default();
  read(&unmasked);
  let unmasked_bytes = unmasked.estimate_encoded_size();
  let new_nodes = unmasked
    .to_storage_proof()
    .iter_nodes()
    .filter(|node| !universe.contains(*node))
    .count();
  assert!(new_nodes > 0);
  assert!(
    unmasked_bytes > upper,
    "new-state recording defeats original-input bytes"
  );
  let masked = Recorder::<Blake2Hasher>::with_ignored_nodes(ignored);
  read(&masked);
  let masked_bytes = masked.estimate_encoded_size();
  assert!(
    masked_bytes > 0,
    "the control must not ignore every input node"
  );
  assert!(masked_bytes <= upper);
  let recorded = masked.to_storage_proof();
  assert!(recorded.iter_nodes().all(|node| universe.contains(node)));
  assert!(recorded.iter_nodes().any(|node| node == &values[3]));
  let key_state = masked
    .recorded_keys()
    .get(&changed_root)
    .and_then(|by_key| by_key.get(keys[1].as_slice()))
    .map(|state| format!("{state:?}"))
    .unwrap_or_else(|| "None".into());
  read(&masked);
  assert_eq!(masked_bytes, masked.estimate_encoded_size());
  println!(
    "PROOF_UMBRELLA_GENERATED {}",
    json!({
      "input_nodes":input.len(), "input_universe_upper_bytes":upper,
      "positive_transaction_nodes":positive.len(), "changed_root":true,
      "root_phase_recorded_bytes":root_phase_bytes, "root_phase_recorded_nodes":root_phase_proof.len(),
      "root_phase_subset_of_input":true, "root_phase_generated_overlap":false,
      "positive_nodes_outside_input":generated_outside_input.len(),
      "unmasked_recorded_bytes":unmasked_bytes, "unmasked_new_nodes":new_nodes,
      "unmasked_exceeds_input_upper":true, "masked_recorded_bytes":masked_bytes,
      "masked_nodes":recorded.len(), "masked_subset_of_input":true,
      "untouched_input_blob_charged":true, "duplicate_byte_counter_stable":true,
      "masked_new_blob_key_state":key_state, "actual_size_only_recorder_executed":false,
      "runtime_legal_underfit":false, "actor_block_pvf_executed":false, "ref_time_upper":false
    })
  );
}

#[test]
fn proof_umbrella_overlay_keeps_original_backend_node_origin() {
  use polkadot_sdk::{
    sp_core::{
      Blake2Hasher,
      storage::{Storage, well_known_keys::CODE},
    },
    sp_runtime::StateVersion,
    sp_state_machine::{Backend, TestExternalities},
    sp_trie::{StorageProof, recorder::Recorder},
  };

  let keys = [vec![0x10], vec![0x20], vec![0x30], vec![0x40]];
  let values = [vec![7; 1], vec![8; 64], vec![9; 2], vec![10; 64]];
  let storage = Storage {
    top: keys.iter().cloned().zip(values.iter().cloned()).collect(),
    ..Default::default()
  };
  let mut ext =
    TestExternalities::<Blake2Hasher>::new_with_state_version(storage, StateVersion::V1);
  let original_root = *ext.backend.root();
  let initial = Recorder::<Blake2Hasher>::default();
  ext.execute_with_recorder(initial.clone(), || {
    for (key, value) in keys.iter().zip(&values) {
      assert_eq!(
        polkadot_sdk::sp_io::storage::get(key).as_deref(),
        Some(value.as_slice())
      );
    }
    assert_eq!(
      polkadot_sdk::sp_io::storage::get(CODE).as_deref(),
      Some(&[][..])
    );
  });
  let input = initial.drain_storage_proof();
  let universe = StorageProof::merge([input.clone(), StorageProof::new([vec![0]])]).into_nodes();
  let upper = universe
    .iter()
    .try_fold(0usize, |sum, node| sum.checked_add(node.encoded_size()))
    .unwrap();
  let replacements = [vec![11; 32], vec![12; 128]];
  let expected_root = ext
    .backend
    .storage_root(
      keys[..2]
        .iter()
        .zip(&replacements)
        .map(|(key, value)| (key.as_slice(), Some(value.as_slice()))),
      StateVersion::V1,
    )
    .0;
  assert_ne!(original_root, expected_root);

  let recorder = Recorder::<Blake2Hasher>::default();
  let observations = ext.execute_with_recorder(recorder.clone(), || {
    for (key, value) in keys[..2].iter().zip(&replacements) {
      polkadot_sdk::sp_io::storage::set(key, value);
    }
    let after_writes = recorder.estimate_encoded_size();
    assert_eq!(after_writes, 0);
    let root = polkadot_sdk::sp_io::storage::root(StateVersion::V1);
    assert_eq!(root, expected_root.encode());
    let after_root = recorder.estimate_encoded_size();
    assert!(after_root > 0);
    for (key, value) in keys[..2].iter().zip(&replacements) {
      assert_eq!(
        polkadot_sdk::sp_io::storage::get(key).as_deref(),
        Some(value.as_slice())
      );
    }
    let after_overlay_reads = recorder.estimate_encoded_size();
    assert_eq!(after_overlay_reads, after_root);
    assert_eq!(polkadot_sdk::sp_io::storage::root(StateVersion::V1), root);
    let after_repeat_root = recorder.estimate_encoded_size();
    assert_eq!(after_repeat_root, after_root);
    for (key, value) in keys[2..].iter().zip(&values[2..]) {
      assert_eq!(
        polkadot_sdk::sp_io::storage::get(key).as_deref(),
        Some(value.as_slice())
      );
    }
    let after_input_reads = recorder.estimate_encoded_size();
    assert!(
      after_input_reads > after_root,
      "untouched input must remain charged"
    );
    json!({
      "after_writes":after_writes, "after_root":after_root,
      "after_overlay_reads":after_overlay_reads, "after_repeat_root":after_repeat_root,
      "after_input_reads":after_input_reads
    })
  });
  assert_eq!(*ext.backend.root(), original_root);
  for (key, value) in keys.iter().zip(&values) {
    assert_eq!(ext.backend.storage(key).unwrap().as_ref(), Some(value));
  }
  let recorded = recorder.to_storage_proof();
  let bytes = recorder.estimate_encoded_size();
  assert!(recorded.iter_nodes().all(|node| universe.contains(node)));
  assert!(recorded.iter_nodes().any(|node| node == &values[3]));
  assert!(bytes <= upper);
  println!(
    "PROOF_UMBRELLA_OVERLAY {}",
    json!({
      "input_nodes":input.len(), "input_universe_upper_bytes":upper,
      "constructor_adds_empty_code":true, "recorded_bytes":bytes, "recorded_nodes":recorded.len(),
      "recorded_subset_of_input":true, "backend_root_and_values_unchanged":true,
      "changed_root_matches_independent_delta":true, "overlay_new_values_visible":true,
      "repeated_root_counter_stable":true, "untouched_input_blob_charged":true,
      "observations":observations, "actual_size_only_recorder_executed":false,
      "actor_block_pvf_executed":false, "ref_time_upper":false
    })
  );
}

#[test]
fn proof_umbrella_decoder_normalization_uses_original_hash_owner_weight() {
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_trie::{
      LayoutV1, MemoryDB, NodeCodec, StorageProof, Trie, TrieDBBuilder, TrieDBMutBuilder, TrieMut,
      cache::{CacheSize, SharedTrieCache},
      recorder::Recorder,
    },
  };

  let key = vec![0x12; 30];
  let value = vec![7; 31];
  let mut db = MemoryDB::<Blake2Hasher>::default();
  let mut canonical_root = H256::default();
  {
    let mut trie =
      TrieDBMutBuilder::<LayoutV1<Blake2Hasher>>::new(&mut db, &mut canonical_root).build();
    trie.insert(&key, &value).unwrap();
  }
  let initial = Recorder::<Blake2Hasher>::default();
  {
    let mut recording = initial.as_trie_recorder(canonical_root);
    let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&db, &canonical_root)
      .with_recorder(&mut recording)
      .build();
    assert_eq!(trie.get(&key).unwrap(), Some(value.clone()));
  }
  let input = initial.drain_storage_proof();
  assert_eq!(input.len(), 1);
  let canonical = input.into_nodes().into_iter().next().unwrap();
  assert_eq!(canonical.len(), 63);
  let mut trailing = canonical.clone();
  trailing.push(0xff);

  for (name, supplied) in [("canonical", canonical.clone()), ("trailing", trailing)] {
    let root = H256::from(polkadot_sdk::sp_io::hashing::blake2_256(&supplied));
    let proof = StorageProof::new([supplied.clone()]);
    let universe = StorageProof::merge([proof.clone(), StorageProof::new([vec![0]])]).into_nodes();
    let upper = universe
      .iter()
      .try_fold(0usize, |sum, node| sum.checked_add(node.encoded_size()))
      .unwrap();
    let owner_weight = supplied.encoded_size();
    let proof_db = proof.to_memory_db::<Blake2Hasher>();
    let cold = Recorder::<Blake2Hasher>::default();
    {
      let mut recording = cold.as_trie_recorder(root);
      let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&proof_db, &root)
        .with_recorder(&mut recording)
        .build();
      assert_eq!(trie.get(&key).unwrap(), Some(value.clone()));
    }
    assert_eq!(cold.estimate_encoded_size(), owner_weight);
    let cache = SharedTrieCache::<Blake2Hasher>::new(CacheSize::new(64 * 1024), None);
    {
      let local = cache.local_cache_untrusted();
      let mut node_cache = local.as_trie_db_cache(root);
      let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&proof_db, &root)
        .with_cache(&mut node_cache)
        .build();
      assert_eq!(trie.get(&key).unwrap(), Some(value.clone()));
    }
    let normalized = cache
      .peek_node(&root)
      .unwrap()
      .to_encoded::<NodeCodec<Blake2Hasher>>();
    assert_eq!(normalized, canonical);
    let normalized_weight = normalized.encoded_size();
    assert!(normalized_weight <= owner_weight);
    let exact_membership = universe.contains(&normalized);
    let normalized_hash_matches =
      H256::from(polkadot_sdk::sp_io::hashing::blake2_256(&normalized)) == root;
    assert_eq!(exact_membership, name == "canonical");
    assert_eq!(normalized_hash_matches, name == "canonical");
    let warm = Recorder::<Blake2Hasher>::default();
    {
      let local = cache.local_cache_untrusted();
      let mut node_cache = local.as_trie_db_cache(root);
      let mut recording = warm.as_trie_recorder(root);
      let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&proof_db, &root)
        .with_cache(&mut node_cache)
        .with_recorder(&mut recording)
        .build();
      assert_eq!(trie.get(&key).unwrap(), Some(value.clone()));
    }
    assert_eq!(warm.estimate_encoded_size(), normalized_weight);
    assert!(warm.estimate_encoded_size() <= upper);
    let recorded = warm.to_storage_proof();
    assert_eq!(recorded.len(), 1);
    assert!(recorded.iter_nodes().any(|node| node == &normalized));
    println!(
      "PROOF_UMBRELLA_NORMALIZATION {name} {}",
      json!({
        "supplied_bytes":supplied.len(), "normalized_bytes":normalized.len(),
        "original_owner_weight":owner_weight, "normalized_weight":normalized_weight,
        "cold_counter":cold.estimate_encoded_size(), "warm_counter":warm.estimate_encoded_size(),
        "input_universe_upper":upper, "exact_membership":exact_membership,
        "normalized_hash_matches_reported_root":normalized_hash_matches,
        "owner_weight_dominates":true, "decode_lookup_value_equal":true,
        "synthetically_chosen_root":true, "canonical_chain_root_reachability_proven":false,
        "actual_size_only_recorder_executed":false, "actor_block_pvf_executed":false,
        "runtime_legal_underfit":false, "ref_time_upper":false
      })
    );
  }
}

#[test]
fn proof_umbrella_old_root_alias_can_charge_generated_payload() {
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_runtime::StateVersion,
    sp_state_machine::{Backend, TrieBackendBuilder},
    sp_trie::{
      LayoutV1, MemoryDB, StorageProof, Trie, TrieDBBuilder, TrieDBMutBuilder, TrieMut,
      cache::{CacheSize, SharedTrieCache},
      recorder::Recorder,
    },
  };

  let keys = [vec![0x10], vec![0x20]];
  let a = vec![7; 64];
  let b = vec![8; 1024];
  let b_hash = H256::from(polkadot_sdk::sp_io::hashing::blake2_256(&b));
  let mut full_db = MemoryDB::<Blake2Hasher>::default();
  let mut root = H256::default();
  {
    let mut trie = TrieDBMutBuilder::<LayoutV1<Blake2Hasher>>::new(&mut full_db, &mut root).build();
    trie.insert(&keys[0], &a).unwrap();
    trie.insert(&keys[1], &b).unwrap();
  }
  let initial = Recorder::<Blake2Hasher>::default();
  {
    let mut recording = initial.as_trie_recorder(root);
    let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&full_db, &root)
      .with_recorder(&mut recording)
      .build();
    assert_eq!(trie.get(&keys[0]).unwrap(), Some(a.clone()));
    assert_eq!(trie.get_hash(&keys[1]).unwrap(), Some(b_hash));
  }
  let input = initial.drain_storage_proof();
  assert!(!input.iter_nodes().any(|node| node == &b));
  let universe = StorageProof::merge([input.clone(), StorageProof::new([vec![0]])]).into_nodes();
  let upper = universe
    .iter()
    .try_fold(0usize, |sum, node| sum.checked_add(node.encoded_size()))
    .unwrap();
  let proof_db = input.to_memory_db::<Blake2Hasher>();
  let plain_missing = || {
    let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&proof_db, &root).build();
    assert_eq!(trie.get_hash(&keys[1]).unwrap(), Some(b_hash));
    assert!(trie.get(&keys[1]).is_err());
  };
  plain_missing();

  let cache = SharedTrieCache::<Blake2Hasher>::new(CacheSize::new(64 * 1024), None);
  assert!(cache.peek_node(&b_hash).is_none());
  let recorder = Recorder::<Blake2Hasher>::default();
  let changed_root;
  {
    let backend = TrieBackendBuilder::<_, Blake2Hasher>::new(&proof_db, root)
      .with_recorder(recorder.clone())
      .with_cache(cache.local_cache_untrusted())
      .build();
    let (new_root, mut transaction) = backend.storage_root(
      std::iter::once((keys[0].as_slice(), Some(b.as_slice()))),
      StateVersion::V1,
    );
    changed_root = new_root;
    assert_ne!(changed_root, root);
    assert_eq!(*backend.root(), root);
    assert!(
      transaction
        .drain()
        .into_values()
        .any(|(value, count)| count > 0 && value == b)
    );
  }
  let after_root = recorder.estimate_encoded_size();
  assert!(after_root <= upper);
  let generated = cache
    .peek_node(&b_hash)
    .expect("root commit must publish the generated blob");
  assert_eq!(
    generated.data().map(|data| data.as_ref()),
    Some(b.as_slice())
  );
  plain_missing();
  {
    let backend = TrieBackendBuilder::<_, Blake2Hasher>::new(&proof_db, root)
      .with_recorder(recorder.clone())
      .with_cache(cache.local_cache_untrusted())
      .build();
    assert_eq!(*backend.root(), root);
    assert_eq!(backend.storage(&keys[1]).unwrap(), Some(b.clone()));
    assert_eq!(*backend.root(), root);
  }
  let after_alias = recorder.estimate_encoded_size();
  let recorded = recorder.to_storage_proof();
  assert!(recorded.iter_nodes().any(|node| node == &b));
  assert!(!universe.contains(&b));
  assert!(
    after_alias > upper,
    "a generated cached alias defeats input-only U"
  );
  assert!(after_alias >= after_root.checked_add(b.encoded_size()).unwrap());
  plain_missing();
  println!(
    "PROOF_UMBRELLA_OLD_ROOT_ALIAS {}",
    json!({
      "input_nodes":input.len(), "input_universe_upper":upper,
      "blob_bytes":b.len(), "blob_encoded_weight":b.encoded_size(),
      "after_root":after_root, "after_old_root_alias":after_alias,
      "input_omits_blob":true, "hash_only_input_path_present":true,
      "generated_blob_cached":true, "positive_transaction_contains_blob":true,
      "plain_db_missing_before_and_after":true, "base_root_unchanged":true,
      "transaction_applied":false, "new_recorded_blob_outside_input":true,
      "input_only_counter_upper_refuted":true, "native_local_cache_drop_publishes":true,
      "actual_private_pvf_cache_or_size_only_executed":false,
      "deos_inspection_reachability_proven":false, "runtime_legal_underfit":false,
      "actor_block_pvf_executed":false, "ref_time_upper":false
    })
  );
}
