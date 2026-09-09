//! Dev-only full-block Wasm replay under the SDK import-with-proof-recording profile.
//!
//! The caller owns complete pre-state, correct state-layout lineage, block authoring, and artifact
//! provenance. This module neither authors blocks nor proves chain membership/finality. Its trie
//! proof sizes are not a full parachain PoV or registered FRAME ProofSize measurement.

use codec::{DecodeAll, Encode};
use polkadot_sdk::{
  sp_core::{
    Blake2Hasher, H256,
    storage::{ChildInfo, StateVersion, Storage, well_known_keys},
    traits::{CallContext, RuntimeCode, WrappedRuntimeCode},
  },
  sp_externalities::Extensions,
  sp_inherents::{CheckInherentsResult, InherentData},
  sp_io,
  sp_runtime::traits::{Block as BlockT, Header as HeaderT},
  sp_state_machine::{
    Backend, BasicExternalities, OverlayedChanges, StateMachine, StorageChanges, TrieBackend,
    TrieBackendBuilder, create_proof_check_backend,
  },
  sp_trie::{
    CompactProof, PrefixedMemoryDB, StorageProof,
    proof_size_extension::{ProofSizeExt, RecordingProofSizeProvider},
    recorder::Recorder,
  },
  sp_version::RuntimeVersion,
};
use sc_executor::{RuntimeVersionOf, WasmExecutor};
use std::{
  borrow::Cow,
  collections::{BTreeMap, BTreeSet},
  panic::AssertUnwindSafe,
};

type ReplayHostFunctions = (
  sp_io::SubstrateHostFunctions,
  cumulus_primitives_proof_size_hostfunction::storage_proof_size::HostFunctions,
);
type ReplayExecutor = WasmExecutor<ReplayHostFunctions>;
const EXECUTE_BLOCK: &str = "Core_execute_block";
const IMPORT_CONTEXT: CallContext = CallContext::Onchain { import: true };

#[derive(Debug)]
pub(crate) struct ProofArtifact {
  pub storage_proof: StorageProof,
  pub compact_proof: CompactProof,
  pub storage_proof_scale_bytes: usize,
  pub compact_proof_scale_bytes: usize,
  pub trie_node_bytes: usize,
  pub trie_node_count: usize,
}

/// Raw trie-node bytes without proof framing. Shared nodes count once; summing independent
/// key paths counts their repeated appearances without implying duplicate runtime execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct KeyProofOverlap {
  pub paths: BTreeMap<(H256, Vec<u8>), StorageProof>,
  pub per_key_node_bytes: usize,
  pub unique_node_bytes: usize,
  pub shared_node_bytes: usize,
  pub unattributed_node_bytes: usize,
}

#[derive(Debug)]
pub(crate) struct VerifiedWasmReplay {
  pub runtime_code_hash: H256,
  pub runtime_version: RuntimeVersion,
  pub block_hash: H256,
  pub pre_state_root: H256,
  pub post_state_root: H256,
  /// Exact SCALE unit returned by Core_execute_block, also returned by independent replay.
  pub execute_block_result: Vec<u8>,
  /// Nodes touched by execution/root calculation; excludes extra code/heap binding reads.
  pub execution_proof: ProofArtifact,
  /// Execution plus code/heap binding and inherent checks, independently replayed after compaction.
  pub verification_proof: ProofArtifact,
  /// Root-scoped key paths retained by the SDK recorder after execution and root calculation.
  pub execution_recorded_keys: BTreeSet<(H256, Vec<u8>)>,
  /// Independent key-path reconstruction from execution proof; never a FRAME Weight discount.
  pub execution_key_overlap: KeyProofOverlap,
  /// Raw nodes first recorded by host root verification after Core_execute_block returned.
  pub post_execute_root_node_bytes: usize,
  /// Root reconstruction from committed deltas using only execution proof nodes.
  pub committed_root_proof: StorageProof,
  /// Actual hostfunction observations; independently replayed in exactly the same order.
  pub proof_size_observations: Vec<usize>,
}

/// Replays an already authored complete block; any malformed input, trap, missing proof node,
/// invalid block, non-unit result, host observation mismatch, or post-root mismatch fails closed.
pub(crate) fn replay_block(
  pre_state: Storage,
  wasm: &[u8],
  encoded_block: &[u8],
  inherent_data: &InherentData,
) -> Result<VerifiedWasmReplay, String> {
  std::panic::catch_unwind(AssertUnwindSafe(|| {
    replay_block_inner(pre_state, wasm, encoded_block, inherent_data)
  }))
  .map_err(|_| "Wasm replay or proof backend panicked".to_owned())?
}

fn replay_block_inner(
  pre_state: Storage,
  wasm: &[u8],
  encoded_block: &[u8],
  inherent_data: &InherentData,
) -> Result<VerifiedWasmReplay, String> {
  if pre_state.top.get(well_known_keys::CODE).map(Vec::as_slice) != Some(wasm) {
    return Err("Supplied Wasm does not equal pre-state :code".into());
  }
  let block = crate::Block::decode_all(&mut &encoded_block[..])
    .map_err(|error| format!("Complete block decode: {error}"))?;
  if block.encode() != encoded_block {
    return Err("Block encoding is not canonical".into());
  }
  let heap_pages = decode_heap_pages(pre_state.top.get(well_known_keys::HEAP_PAGES))?;
  let code_fetcher = WrappedRuntimeCode(Cow::Borrowed(wasm));
  let runtime_code_hash = H256::from(sp_io::hashing::blake2_256(wasm));
  let runtime_code = RuntimeCode {
    code_fetcher: &code_fetcher,
    heap_pages,
    hash: runtime_code_hash.encode(),
  };
  let executor = ReplayExecutor::builder().build();
  // Version discovery is isolated from the measured execution overlay and recorder.
  let runtime_version = executor
    .runtime_version(
      &mut BasicExternalities::new(pre_state.clone()),
      &runtime_code,
    )
    .map_err(|error| format!("Exact Wasm version: {error}"))?;
  let state_version = runtime_version.state_version();
  let expected_post_root = *block.header().state_root();
  let backend: TrieBackend<PrefixedMemoryDB<Blake2Hasher>, Blake2Hasher> =
    (pre_state, state_version).into();
  let pre_state_root = *backend.root();

  // Bind code/heap and validate inherent data without charging these verification reads to the
  // execution proof-size hostfunction. Actual block execution starts with a separate recorder.
  let binding_backend = TrieBackendBuilder::wrap(&backend)
    .with_recorder(Recorder::<Blake2Hasher>::default())
    .build();
  verify_code_binding(&binding_backend, wasm, heap_pages)?;
  let inherent_check = check_inherents(
    &binding_backend,
    &executor,
    &runtime_code,
    &block,
    inherent_data,
  )?;
  let binding_proof = binding_backend
    .extract_proof()
    .ok_or_else(|| "Code binding recorder was absent".to_owned())?;

  let recorder = Recorder::<Blake2Hasher>::default();
  let proving_backend = TrieBackendBuilder::wrap(&backend)
    .with_recorder(recorder.clone())
    .build();
  let observations = RecordingProofSizeProvider::new(recorder.clone());
  let mut extensions = Extensions::default();
  extensions.register(ProofSizeExt::new(observations.clone()));
  let mut overlay = OverlayedChanges::<Blake2Hasher>::default();
  let result = StateMachine::new(
    &proving_backend,
    &mut overlay,
    &executor,
    EXECUTE_BLOCK,
    encoded_block,
    &mut extensions,
    &runtime_code,
    IMPORT_CONTEXT,
  )
  .set_parent_hash(*block.header().parent_hash())
  .execute()
  .map_err(|error| format!("Full-state Core_execute_block: {error}"))?;
  require_unit(&result)?;
  let execute_proof = recorder.to_storage_proof();
  let storage_changes = overlay
    .drain_storage_changes(&proving_backend, state_version)
    .map_err(|error| format!("Full-state post root: {error}"))?;
  let post_state_root = storage_changes.transaction_storage_root;
  if post_state_root != expected_post_root {
    return Err("Full-state post root differs from the authored header".into());
  }
  let execution_recorded_keys = recorded_trie_keys(&recorder);
  let execution_key_overlap = key_proof_overlap(&recorder)?;
  let execution_proof = proving_backend
    .extract_proof()
    .ok_or_else(|| "Execution recorder was absent".to_owned())?;
  let execute_nodes = execute_proof.iter_nodes().collect::<BTreeSet<_>>();
  let execution_nodes = execution_proof.iter_nodes().collect::<BTreeSet<_>>();
  if !execute_nodes.is_subset(&execution_nodes) {
    return Err("Host root verification removed execution proof nodes".into());
  }
  let post_execute_root_node_bytes = execution_nodes
    .difference(&execute_nodes)
    .map(|node| node.len())
    .sum();
  let committed_root_proof = reconstruct_committed_root_proof(
    pre_state_root,
    &storage_changes,
    &execution_proof,
    state_version,
  )?;
  let verification_proof = StorageProof::merge([execution_proof.clone(), binding_proof]);
  let verification_proof = proof_artifact(verification_proof, pre_state_root)?;
  let (decoded_proof, decoded_root) = verification_proof
    .compact_proof
    .to_storage_proof::<Blake2Hasher>(Some(&pre_state_root))
    .map_err(|error| format!("Compact proof decode: {error:?}"))?;
  if decoded_root != pre_state_root {
    return Err("Compact proof changed the pre-state root".into());
  }

  // execution_proof_check uses Offchain in SDK 0.53. Use its same proof-only backend constructor
  // with the actual Onchain import context and fresh ProofSizeExt instead of changing conditions.
  let proof_backend = create_proof_check_backend::<Blake2Hasher>(pre_state_root, decoded_proof)
    .map_err(|error| format!("Proof backend: {error}"))?;
  verify_code_binding(&proof_backend, wasm, heap_pages)?;
  let verifier = ReplayExecutor::builder().build();
  if check_inherents(
    &proof_backend,
    &verifier,
    &runtime_code,
    &block,
    inherent_data,
  )? != inherent_check
  {
    return Err("Proof-only inherent validation differs".into());
  }
  let verifier_recorder = Recorder::<Blake2Hasher>::default();
  let checking_backend = TrieBackendBuilder::wrap(&proof_backend)
    .with_recorder(verifier_recorder.clone())
    .build();
  let verifier_observations = RecordingProofSizeProvider::new(verifier_recorder.clone());
  let mut verifier_extensions = Extensions::default();
  verifier_extensions.register(ProofSizeExt::new(verifier_observations.clone()));
  let mut verifier_overlay = OverlayedChanges::<Blake2Hasher>::default();
  let checked_result = StateMachine::new(
    &checking_backend,
    &mut verifier_overlay,
    &verifier,
    EXECUTE_BLOCK,
    encoded_block,
    &mut verifier_extensions,
    &runtime_code,
    IMPORT_CONTEXT,
  )
  .set_parent_hash(*block.header().parent_hash())
  .execute()
  .map_err(|error| format!("Proof-only Core_execute_block: {error}"))?;
  require_unit(&checked_result)?;
  let checked_root = verifier_overlay
    .drain_storage_changes(&checking_backend, state_version)
    .map_err(|error| format!("Proof-only post root: {error}"))?
    .transaction_storage_root;
  let proof_size_observations = observations.recorded_estimations();
  if checked_result != result
    || checked_root != post_state_root
    || verifier_observations.recorded_estimations() != proof_size_observations
    || recorded_trie_keys(&verifier_recorder) != execution_recorded_keys
  {
    return Err(
      "Proof-only result, post root, proof-size observations, or recorded keys differ".into(),
    );
  }
  Ok(VerifiedWasmReplay {
    runtime_code_hash,
    runtime_version,
    block_hash: block.header().hash(),
    pre_state_root,
    post_state_root,
    execute_block_result: result,
    execution_proof: proof_artifact(execution_proof, pre_state_root)?,
    verification_proof,
    execution_recorded_keys,
    execution_key_overlap,
    post_execute_root_node_bytes,
    committed_root_proof,
    proof_size_observations,
  })
}

fn reconstruct_committed_root_proof(
  pre_state_root: H256,
  changes: &StorageChanges<Blake2Hasher>,
  execution_proof: &StorageProof,
  state_version: StateVersion,
) -> Result<StorageProof, String> {
  let backend = create_proof_check_backend::<Blake2Hasher>(pre_state_root, execution_proof.clone())
    .map_err(|error| format!("Committed-root proof backend: {error}"))?;
  let recorder = Recorder::<Blake2Hasher>::default();
  let proving_backend = TrieBackendBuilder::wrap(&backend)
    .with_recorder(recorder.clone())
    .build();
  let children = changes
    .child_storage_changes
    .iter()
    .map(|(key, delta)| (ChildInfo::new_default(key), delta))
    .collect::<Vec<_>>();
  let (root, _) = proving_backend.full_storage_root(
    changes
      .main_storage_changes
      .iter()
      .map(|(key, value)| (key.as_slice(), value.as_deref())),
    children.iter().map(|(info, delta)| {
      (
        info,
        delta
          .iter()
          .map(|(key, value)| (key.as_slice(), value.as_deref())),
      )
    }),
    state_version,
  );
  if root != changes.transaction_storage_root {
    return Err("Committed delta proof did not reproduce the post-state root".into());
  }
  let proof = recorder.to_storage_proof();
  let execution_nodes = execution_proof.iter_nodes().collect::<BTreeSet<_>>();
  if proof
    .iter_nodes()
    .any(|node| !execution_nodes.contains(node))
  {
    return Err("Committed root reconstruction escaped the execution proof".into());
  }
  Ok(proof)
}

fn recorded_trie_keys(recorder: &Recorder<Blake2Hasher>) -> BTreeSet<(H256, Vec<u8>)> {
  recorder
    .recorded_keys()
    .into_iter()
    .flat_map(|(root, keys)| {
      keys
        .into_keys()
        .map(move |key| (root, key.as_ref().to_vec()))
    })
    .collect()
}

fn key_proof_overlap(recorder: &Recorder<Blake2Hasher>) -> Result<KeyProofOverlap, String> {
  let proof = recorder.to_storage_proof();
  let proof_nodes = proof.iter_nodes().collect::<BTreeSet<_>>();
  let db = proof.to_memory_db::<Blake2Hasher>();
  let mut uses = BTreeMap::<Vec<u8>, usize>::new();
  let mut per_key_node_bytes = 0;
  let mut paths = BTreeMap::new();
  for (root, keys) in recorder.recorded_keys() {
    for (key, access) in keys {
      let key_recorder = Recorder::<Blake2Hasher>::default();
      let backend = TrieBackendBuilder::new(&db, root)
        .with_recorder(key_recorder.clone())
        .build();
      backend
        .storage_hash(&key)
        .map_err(|error| format!("Recorded key hash path: {error}"))?;
      let hash_accesses = key_recorder.recorded_keys();
      if hash_accesses.get(&root).and_then(|keys| keys.get(&key)) != Some(&access) {
        backend
          .storage(&key)
          .map_err(|error| format!("Recorded key value path: {error}"))?;
      }
      let replayed_accesses = key_recorder.recorded_keys();
      if replayed_accesses.get(&root).and_then(|keys| keys.get(&key)) != Some(&access) {
        return Err("Reconstructed key path changed the recorded access kind".into());
      }
      let key_proof = key_recorder.to_storage_proof();
      for node in key_proof.iter_nodes() {
        if !proof_nodes.contains(node) {
          return Err("Reconstructed key path contains a node outside execution proof".into());
        }
        per_key_node_bytes += node.len();
        *uses.entry(node.clone()).or_default() += 1;
      }
      paths.insert((root, key.as_ref().to_vec()), key_proof);
    }
  }
  let unique_node_bytes = uses.keys().map(Vec::len).sum::<usize>();
  let shared_node_bytes = uses
    .iter()
    .filter(|(_, count)| **count > 1)
    .map(|(node, _)| node.len())
    .sum();
  let execution_node_bytes = proof.iter_nodes().map(Vec::len).sum::<usize>();
  Ok(KeyProofOverlap {
    paths,
    per_key_node_bytes,
    unique_node_bytes,
    shared_node_bytes,
    unattributed_node_bytes: execution_node_bytes
      .checked_sub(unique_node_bytes)
      .ok_or("Key-path union exceeds execution proof")?,
  })
}

fn check_inherents<B: Backend<Blake2Hasher>>(
  backend: &B,
  executor: &ReplayExecutor,
  runtime_code: &RuntimeCode,
  block: &crate::Block,
  inherent_data: &InherentData,
) -> Result<Vec<u8>, String> {
  let mut overlay = OverlayedChanges::<Blake2Hasher>::default();
  let mut extensions = Extensions::default();
  let result = StateMachine::new(
    backend,
    &mut overlay,
    executor,
    "BlockBuilder_check_inherents",
    &(block, inherent_data).encode(),
    &mut extensions,
    runtime_code,
    IMPORT_CONTEXT,
  )
  .set_parent_hash(*block.header().parent_hash())
  .execute()
  .map_err(|error| format!("Wasm inherent validation: {error}"))?;
  let checked = CheckInherentsResult::decode_all(&mut &result[..])
    .map_err(|error| format!("Inherent validation result decode: {error}"))?;
  if !checked.ok() || !overlay.is_empty() {
    return Err("Wasm inherent validation rejected input or mutated state".into());
  }
  Ok(result)
}

fn require_unit(result: &[u8]) -> Result<(), String> {
  <()>::decode_all(&mut &result[..])
    .map_err(|error| format!("Core_execute_block returned non-unit bytes: {error}"))
}

fn decode_heap_pages(bytes: Option<&Vec<u8>>) -> Result<Option<u64>, String> {
  bytes
    .map(|bytes| {
      u64::decode_all(&mut &bytes[..]).map_err(|error| format!("Invalid :heappages: {error}"))
    })
    .transpose()
}

fn verify_code_binding<B: Backend<Blake2Hasher>>(
  backend: &B,
  wasm: &[u8],
  expected_heap_pages: Option<u64>,
) -> Result<(), String> {
  let stored_code = backend
    .storage(well_known_keys::CODE)
    .map_err(|_| "Cannot read proof-bound :code".to_owned())?;
  let stored_heap_pages = backend
    .storage(well_known_keys::HEAP_PAGES)
    .map_err(|_| "Cannot read proof-bound :heappages".to_owned())?;
  if stored_code.as_deref() != Some(wasm)
    || decode_heap_pages(stored_heap_pages.as_ref())? != expected_heap_pages
  {
    return Err("Proof-bound code or heap differs from the replay executor".into());
  }
  Ok(())
}

fn proof_artifact(proof: StorageProof, root: H256) -> Result<ProofArtifact, String> {
  let compact_proof = proof
    .to_compact_proof::<Blake2Hasher>(root)
    .map_err(|error| format!("Storage proof compaction: {error:?}"))?;
  let trie_node_bytes = proof.iter_nodes().try_fold(0usize, |total, node| {
    total
      .checked_add(node.len())
      .ok_or("Proof byte count overflow")
  })?;
  Ok(ProofArtifact {
    storage_proof_scale_bytes: proof.encoded_size(),
    compact_proof_scale_bytes: compact_proof.encoded_size(),
    trie_node_bytes,
    trie_node_count: proof.len(),
    storage_proof: proof,
    compact_proof,
  })
}

#[test]
fn committed_root_proof_covers_top_and_child_deltas_and_rejects_invalid_evidence() {
  let state_version = crate::VERSION.state_version();
  let storage = Storage {
    top: [(b"a".to_vec(), vec![1; 64]), (b"b".to_vec(), vec![2; 64])].into(),
    ..Default::default()
  };
  let backend: TrieBackend<PrefixedMemoryDB<Blake2Hasher>, Blake2Hasher> =
    (storage, state_version).into();
  let recorder = Recorder::<Blake2Hasher>::default();
  let proving_backend = TrieBackendBuilder::wrap(&backend)
    .with_recorder(recorder.clone())
    .build();
  let mut overlay = OverlayedChanges::<Blake2Hasher>::default();
  overlay.set_storage(b"a".to_vec(), Some(vec![3; 64]));
  overlay.set_storage(b"b".to_vec(), None);
  overlay.set_storage(b"c".to_vec(), Some(vec![4; 64]));
  overlay.set_child_storage(
    &ChildInfo::new_default(b"child"),
    b"a".to_vec(),
    Some(vec![5; 64]),
  );
  let mut changes = overlay
    .drain_storage_changes(&proving_backend, state_version)
    .unwrap();
  assert_eq!(changes.child_storage_changes.len(), 1);
  let proof = recorder.to_storage_proof();
  assert_eq!(
    reconstruct_committed_root_proof(*backend.root(), &changes, &proof, state_version).unwrap(),
    proof,
  );
  assert!(
    reconstruct_committed_root_proof(
      *backend.root(),
      &changes,
      &StorageProof::empty(),
      state_version
    )
    .is_err()
  );
  changes.transaction_storage_root = H256::zero();
  assert!(
    reconstruct_committed_root_proof(*backend.root(), &changes, &proof, state_version).is_err()
  );
}

#[test]
fn recorded_keys_retain_trie_root_identity() {
  let recorder = Recorder::<Blake2Hasher>::default();
  let key = b"shared-key".to_vec();
  let mut expected = BTreeSet::new();
  for value in [vec![1; 64], vec![2; 64]] {
    let storage = Storage {
      top: [(key.clone(), value.clone())].into(),
      ..Default::default()
    };
    let backend: TrieBackend<PrefixedMemoryDB<Blake2Hasher>, Blake2Hasher> =
      (storage, crate::VERSION.state_version()).into();
    expected.insert((*backend.root(), key.clone()));
    let proving_backend = TrieBackendBuilder::wrap(&backend)
      .with_recorder(recorder.clone())
      .build();
    for _ in 0..2 {
      assert_eq!(proving_backend.storage(&key).unwrap(), Some(value.clone()));
    }
  }
  assert_eq!(expected.len(), 2, "fixture roots must be distinct");
  assert_eq!(recorded_trie_keys(&recorder), expected);
  let overlap = key_proof_overlap(&recorder).expect("both roots must be proof-reconstructible");
  assert_eq!(overlap.unattributed_node_bytes, 0);
  assert_eq!(overlap.per_key_node_bytes, overlap.unique_node_bytes);
}

#[test]
fn key_proof_overlap_preserves_access_kind_and_shared_nodes() {
  let shared_value = vec![7; 64];
  let hash_only_value = vec![9; 128];
  let storage = Storage {
    top: [
      (b"prefix/a".to_vec(), shared_value.clone()),
      (b"prefix/b".to_vec(), shared_value.clone()),
      (b"prefix/c".to_vec(), hash_only_value.clone()),
      (b"prefix/d".to_vec(), vec![3; 8]),
    ]
    .into(),
    ..Default::default()
  };
  let backend: TrieBackend<PrefixedMemoryDB<Blake2Hasher>, Blake2Hasher> =
    (storage, crate::VERSION.state_version()).into();
  let recorder = Recorder::<Blake2Hasher>::default();
  let proving_backend = TrieBackendBuilder::wrap(&backend)
    .with_recorder(recorder.clone())
    .build();
  for key in [b"prefix/a", b"prefix/b", b"prefix/d", b"prefix/z"] {
    proving_backend.storage(key).unwrap();
  }
  proving_backend.storage_hash(b"prefix/c").unwrap();
  let proof = recorder.to_storage_proof();
  assert!(!proof.iter_nodes().any(|node| *node == hash_only_value));
  assert_eq!(recorded_trie_keys(&recorder).len(), 5);
  let overlap = key_proof_overlap(&recorder).expect("recorded key paths must reconstruct");
  assert_eq!(overlap.unattributed_node_bytes, 0);
  assert_eq!(
    overlap.unique_node_bytes,
    proof.iter_nodes().map(Vec::len).sum::<usize>(),
  );
  assert!(overlap.shared_node_bytes >= shared_value.len());
  assert!(overlap.per_key_node_bytes > overlap.unique_node_bytes);
  proving_backend.storage(b"prefix/a").unwrap();
  assert_eq!(key_proof_overlap(&recorder).unwrap(), overlap);
}

#[test]
fn key_proof_overlap_keeps_iterator_nodes_unattributed() {
  let storage = Storage {
    top: [(b"key".to_vec(), vec![1; 64])].into(),
    ..Default::default()
  };
  let backend: TrieBackend<PrefixedMemoryDB<Blake2Hasher>, Blake2Hasher> =
    (storage, crate::VERSION.state_version()).into();
  let recorder = Recorder::<Blake2Hasher>::default();
  let proving_backend = TrieBackendBuilder::wrap(&backend)
    .with_recorder(recorder.clone())
    .build();
  assert_eq!(
    proving_backend.next_storage_key(b"").unwrap(),
    Some(b"key".to_vec())
  );
  assert!(recorded_trie_keys(&recorder).is_empty());
  let overlap = key_proof_overlap(&recorder).unwrap();
  assert_eq!(overlap.per_key_node_bytes, 0);
  assert_eq!(overlap.unique_node_bytes, 0);
  assert_eq!(overlap.shared_node_bytes, 0);
  assert!(overlap.unattributed_node_bytes > 0);
  assert_eq!(
    overlap.unattributed_node_bytes,
    recorder
      .to_storage_proof()
      .iter_nodes()
      .map(Vec::len)
      .sum::<usize>(),
  );
}

#[test]
fn replay_rejects_unbound_wasm_before_execution() {
  let mut storage = Storage::default();
  storage.top.insert(well_known_keys::CODE.to_vec(), vec![1]);
  assert_eq!(
    replay_block(storage, &[2], &[], &InherentData::new()).expect_err("different code must fail"),
    "Supplied Wasm does not equal pre-state :code"
  );
}

#[test]
fn replay_rejects_malformed_block_before_loading_wasm() {
  // Deliberately invalid Wasm proves rejection does not instantiate untrusted code.
  let wasm = b"not a Wasm module";
  let mut storage = Storage::default();
  storage
    .top
    .insert(well_known_keys::CODE.to_vec(), wasm.to_vec());
  let block = crate::Block {
    header: crate::Header::new(
      0,
      Default::default(),
      Default::default(),
      Default::default(),
      Default::default(),
    ),
    extrinsics: Vec::new(),
  };
  let mut trailing_bytes = block.encode();
  trailing_bytes.push(0);
  let mut noncanonical_length = block.encode();
  assert_eq!(noncanonical_length.pop(), Some(0));
  // Empty extrinsics use one-byte Compact(0); a two-byte zero is noncanonical SCALE.
  noncanonical_length.extend_from_slice(&[1, 0]);
  for bytes in [Vec::new(), trailing_bytes, noncanonical_length] {
    let error = replay_block(storage.clone(), wasm, &bytes, &InherentData::new())
      .expect_err("malformed or noncanonical block bytes must fail before Wasm loading");
    assert!(error.starts_with("Complete block decode:"), "{error}");
  }
}
