use codec::Encode;
use polkadot_sdk::{
  sp_core::{Blake2Hasher, H256},
  sp_trie::{
    LayoutV1, NodeCodec, StorageProof, Trie, TrieDBBuilder,
    cache::{CacheSize, SharedTrieCache},
  },
};
use std::collections::{BTreeMap, BTreeSet};

fn hash(bytes: &[u8]) -> H256 {
  H256::from(polkadot_sdk::sp_io::hashing::blake2_256(bytes))
}

// Test-only certificate; input membership and weights do not authenticate execution origin.
struct InputBudget {
  owners: BTreeMap<H256, u32>,
  total: u32,
}

impl InputBudget {
  fn from_weights(owners: BTreeMap<H256, u32>) -> Result<Self, &'static str> {
    let total = owners
      .values()
      .try_fold(0u32, |sum, weight| sum.checked_add(*weight))
      .ok_or("input budget exceeds wasm32")?;
    Ok(Self { owners, total })
  }

  fn from_proof(proof: &StorageProof) -> Result<Self, &'static str> {
    let universe = StorageProof::merge([proof.clone(), StorageProof::new([vec![0]])]);
    let mut owners = BTreeMap::new();
    for supplied in universe.iter_nodes() {
      let original_hash = hash(supplied);
      // Raw owner size is a candidate budget, not assumed decoder domination.
      // certify must reject any first-seen re-encoding that exceeds this budget.
      let weight =
        u32::try_from(supplied.encoded_size()).map_err(|_| "input owner exceeds wasm32")?;
      owners
        .entry(original_hash)
        .and_modify(|old: &mut u32| *old = (*old).max(weight))
        .or_insert(weight);
    }
    Self::from_weights(owners)
  }

  // Events carry the recorder's original hash, not a hash recomputed from normalized bytes.
  // This mirrors first-seen size accounting, not the private Cumulus recorder itself.
  fn certify(&self, events: impl IntoIterator<Item = (H256, usize)>) -> Result<u32, &'static str> {
    let mut seen = BTreeSet::new();
    let mut total = 0u32;
    for (original_hash, encoded_weight) in events {
      if !seen.insert(original_hash) {
        continue;
      }
      let maximum = self
        .owners
        .get(&original_hash)
        .ok_or("unknown original hash")?;
      if encoded_weight > *maximum as usize {
        return Err("input owner weight exceeded");
      }
      total = total
        .checked_add(encoded_weight as u32)
        .ok_or("recorded sum exceeds wasm32")?;
    }
    Ok(total)
  }
}

#[test]
fn implicit_null_is_one_hash_owner() {
  let empty = InputBudget::from_proof(&StorageProof::empty()).unwrap();
  let explicit = InputBudget::from_proof(&StorageProof::new([vec![0]])).unwrap();
  assert_eq!(empty.owners, explicit.owners);
  assert_eq!(empty.total, 2);
  assert_eq!(empty.certify([(hash(&[0]), 2)]), Ok(2));
  assert_eq!(empty.certify([]), Ok(0));
}

#[test]
fn normalized_nodes_keep_the_original_hash_budget() {
  use polkadot_sdk::sp_trie::{MemoryDB, TrieDBMutBuilder, TrieMut, recorder::Recorder};

  let key = vec![0x12; 30];
  let value = vec![7; 31];
  let mut db = MemoryDB::<Blake2Hasher>::default();
  let mut root = H256::default();
  {
    let mut trie = TrieDBMutBuilder::<LayoutV1<Blake2Hasher>>::new(&mut db, &mut root).build();
    trie.insert(&key, &value).unwrap();
  }
  let recorder = Recorder::<Blake2Hasher>::default();
  {
    let mut recording = recorder.as_trie_recorder(root);
    let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&db, &root)
      .with_recorder(&mut recording)
      .build();
    assert_eq!(trie.get(&key).unwrap(), Some(value));
  }
  let nodes = recorder.drain_storage_proof().into_nodes();
  assert_eq!(nodes.len(), 1);
  let canonical = nodes.into_iter().next().unwrap();
  assert_eq!(canonical.len(), 63);
  let mut trailing = canonical.clone();
  trailing.push(0xff);
  let owner = hash(&trailing);
  let input = StorageProof::new([trailing.clone()]);
  let input_db = input.to_memory_db::<Blake2Hasher>();
  let cache = SharedTrieCache::<Blake2Hasher>::new(CacheSize::new(64 * 1024), None);
  {
    let local = cache.local_cache_untrusted();
    let mut node_cache = local.as_trie_db_cache(owner);
    let trie = TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&input_db, &owner)
      .with_cache(&mut node_cache)
      .build();
    assert_eq!(trie.get(&key).unwrap(), Some(vec![7; 31]));
  }
  let normalized = cache
    .peek_node(&owner)
    .unwrap()
    .to_encoded::<NodeCodec<Blake2Hasher>>();
  assert_eq!(normalized, canonical);
  let budget = InputBudget::from_proof(&input).unwrap();
  assert_ne!(owner, hash(&normalized));
  assert_eq!(budget.owners[&owner], trailing.encoded_size() as u32);
  assert_eq!(
    budget.certify([(owner, canonical.encoded_size())]),
    Ok(canonical.encoded_size() as u32)
  );
  assert_eq!(
    budget.certify([(hash(&canonical), canonical.encoded_size())]),
    Err("unknown original hash")
  );
  assert_eq!(
    budget.certify([(owner, trailing.encoded_size() + 1)]),
    Err("input owner weight exceeded")
  );
  assert_eq!(
    budget.certify([(owner, canonical.encoded_size()), (owner, usize::MAX)]),
    Ok(canonical.encoded_size() as u32)
  );
}

#[test]
fn owner_sum_has_an_explicit_wasm32_boundary() {
  // Synthetic weights test arithmetic, not a multi-gigabyte witness or admitted state.
  let a = H256::repeat_byte(1);
  let b = H256::repeat_byte(2);
  let budget = InputBudget::from_weights([(a, u32::MAX - 1), (b, 1)].into()).unwrap();
  assert_eq!(budget.total, u32::MAX);
  assert_eq!(
    budget.certify([(b, 1), (a, (u32::MAX - 1) as usize), (b, usize::MAX)]),
    Ok(u32::MAX)
  );
  assert!(InputBudget::from_weights([(a, u32::MAX), (b, 1)].into()).is_err());
}

#[test]
fn overlay_root_is_certified_but_promoted_backend_is_a_new_epoch() {
  use polkadot_sdk::{
    sp_core::storage::{Storage, well_known_keys::CODE},
    sp_runtime::StateVersion,
    sp_state_machine::{Backend, TestExternalities},
    sp_trie::recorder::Recorder,
  };

  let a = vec![0x10];
  let b = vec![0x20];
  let original = vec![1; 64];
  let replacement = vec![3; 256];
  let storage = Storage {
    top: [(a.clone(), original.clone()), (b.clone(), vec![2; 64])].into(),
    ..Default::default()
  };
  let mut ext =
    TestExternalities::<Blake2Hasher>::new_with_state_version(storage, StateVersion::V1);
  let old_root = *ext.backend.root();
  let initial = Recorder::<Blake2Hasher>::default();
  ext.execute_with_recorder(initial.clone(), || {
    for key in [&a[..], &b[..], CODE] {
      assert!(polkadot_sdk::sp_io::storage::get(key).is_some());
    }
  });
  let budget = InputBudget::from_proof(&initial.drain_storage_proof()).unwrap();
  let (expected_root, _) = ext.backend.storage_root(
    [
      (a.as_slice(), Some(replacement.as_slice())),
      (b.as_slice(), None),
    ]
    .into_iter(),
    StateVersion::V1,
  );
  assert_ne!(expected_root, old_root);
  let recorder = Recorder::<Blake2Hasher>::default();
  ext.execute_with_recorder(recorder.clone(), || {
    polkadot_sdk::sp_io::storage::set(&a, &replacement);
    polkadot_sdk::sp_io::storage::clear(&b);
    assert_eq!(recorder.estimate_encoded_size(), 0);
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(StateVersion::V1),
      expected_root.encode()
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::get(&a).as_deref(),
      Some(replacement.as_slice())
    );
  });
  assert_eq!(*ext.backend.root(), old_root);
  // The native canonical constructor preserves encoding, so recomputing these hashes
  // is valid here. Normalizing input needs the original event hash, as tested above.
  let nodes = recorder.to_storage_proof();
  let certified = budget
    .certify(
      nodes
        .iter_nodes()
        .map(|node| (hash(node), node.encoded_size())),
    )
    .unwrap();
  assert_eq!(certified as usize, recorder.estimate_encoded_size());
  assert!(certified <= budget.total);
  assert_eq!(ext.backend.storage(&a).unwrap(), Some(original));
  ext.commit_all().unwrap();
  assert_eq!(*ext.backend.root(), expected_root);
  let promoted = Recorder::<Blake2Hasher>::default();
  ext.execute_with_recorder(promoted.clone(), || {
    assert_eq!(
      polkadot_sdk::sp_io::storage::get(&a).as_deref(),
      Some(replacement.as_slice())
    );
  });
  assert_eq!(
    budget.certify(
      promoted
        .to_storage_proof()
        .iter_nodes()
        .map(|node| (hash(node), node.encoded_size()))
    ),
    Err("unknown original hash")
  );
  println!(
    "PROOF_INPUT_BUDGET native_overlay_certified={certified} upper={} promoted_epoch_refused=true actual_cumulus_recorder=false pvf_execution=false ref_time_upper=false",
    budget.total
  );
}
