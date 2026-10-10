//! Fixed-corpus hole-prefix inputs for the closed retention construction.
mod actor_retention_corpus;
#[path = "actor_retention_hole_domain/birth.rs"]
mod birth;
#[path = "actor_retention_fixture/inherents.rs"]
mod inherent_data;
#[path = "actor_retention_hole_domain/inherent_lengths.rs"]
mod inherent_lengths;
#[path = "actor_retention_hole_domain/signed_envelope.rs"]
mod signed_envelope;
#[path = "actor_retention_hole_domain/signed_pair.rs"]
mod signed_pair;

use actor_retention_corpus::{INPUT_SHA256, PAGES, WATCHES, digest, fixed_pages};
use codec::{Encode, MaxEncodedLen};
use deos_runtime::Runtime;
use pallet_deos_actors::{
  DependencyRegistrationFreeIndices as Indices, DependencyRegistrationFreePositions as Positions,
  DependencyRegistrationPosition,
};
use polkadot_sdk::{
  sp_core::Blake2Hasher,
  sp_trie::{LayoutV1, TrieLayout},
};
use serde_json::json;

const CAP: usize = 63;

fn nibble<const N: usize>(key: &[u8; N], depth: usize) -> u8 {
  assert!(depth < 2 * N);
  if depth.is_multiple_of(2) {
    key[depth / 2] >> 4
  } else {
    key[depth / 2] & 15
  }
}

// Each absent lookup follows a prefix of some extant terminal path, or stops at a
// missing child. A smaller coherent key set removes branches/reduces fanout. Thus
// maximization over complete-image terminal paths also bounds absence lookups.
fn branch_union<const N: usize>(keys: &[[u8; N]], mut depth: usize) -> Vec<usize> {
  assert!(!keys.is_empty());
  if keys.len() == 1 {
    return vec![0, 0];
  }
  while nibble(&keys[0], depth) == nibble(keys.last().unwrap(), depth) {
    depth += 1;
    assert!(depth < 2 * N, "Concat suffixes must remain unique");
  }
  let mut costs = vec![0];
  let mut start = 0;
  let mut children = 0;
  while start < keys.len() {
    let digit = nibble(&keys[start], depth);
    let end = start + keys[start..].partition_point(|key| nibble(key, depth) == digit);
    let child = branch_union(&keys[start..end], depth + 1);
    let mut next = vec![0; (costs.len() + child.len() - 1).min(CAP + 1)];
    for (left, left_cost) in costs
      .iter() // deos-bypass: bounded-iter -- at most CAP + 1 path-count cells
      .enumerate()
    {
      for (right, right_cost) in child
        .iter() // deos-bypass: bounded-iter -- at most CAP + 1 path-count cells
        .enumerate()
        .take(next.len() - left)
      {
        next[left + right] = next[left + right].max(left_cost + right_cost);
      }
    }
    costs = next;
    start = end;
    children += 1;
  }
  assert!((2..=16).contains(&children));
  // Header, bitmap, SCALE-vector overhead and odd-nibble rounding; no branch values.
  for cost in &mut costs[1..] {
    *cost += 7 + 33 * children;
  }
  costs
}

fn curve<const N: usize>(mut keys: Vec<[u8; N]>) -> Vec<usize> {
  keys.sort_unstable();
  assert!(keys.windows(2).all(|pair| pair[0] != pair[1]));
  let costs = branch_union(&keys, 0);
  assert_eq!(costs.len(), CAP + 1);
  assert!(costs.windows(2).all(|pair| pair[0] <= pair[1]));
  costs
}

// This is the closed recipe's checked allocator endpoint, not the runtime identity cap.
const ACTOR_ID_END: u64 = 9_989;

fn runtime_namespace_stems() -> std::collections::BTreeSet<Vec<u8>> {
  use codec::Decode;
  use polkadot_sdk::frame_support::__private::metadata::{
    RuntimeMetadata, RuntimeMetadataPrefixed,
  };
  let encoded = Runtime::metadata_at_version(16).unwrap();
  let mut input = &encoded[..];
  let RuntimeMetadataPrefixed(_, RuntimeMetadata::V16(metadata)) =
    RuntimeMetadataPrefixed::decode(&mut input).unwrap()
  else {
    panic!("native metadata must be v16")
  };
  assert!(input.is_empty());
  let mut stems = std::collections::BTreeSet::new();
  for key in [
    polkadot_sdk::sp_core::storage::well_known_keys::CODE,
    polkadot_sdk::sp_core::storage::well_known_keys::PENDING_CODE,
    polkadot_sdk::sp_core::storage::well_known_keys::HEAP_PAGES,
    polkadot_sdk::sp_core::storage::well_known_keys::EXTRINSIC_INDEX,
    polkadot_sdk::sp_core::storage::well_known_keys::INTRABLOCK_ENTROPY,
    polkadot_sdk::frame_support::storage::transactional::TRANSACTION_LEVEL_KEY,
    b":c",
  ] {
    assert!(key.len() <= 32);
    assert!(stems.insert(key.to_vec()));
  }
  let mut storage_count = 0;
  for pallet in &metadata.pallets {
    assert!(
      stems.insert(
        polkadot_sdk::frame_support::storage::storage_prefix(
          pallet.name.as_bytes(),
          polkadot_sdk::frame_support::traits::STORAGE_VERSION_STORAGE_KEY_POSTFIX,
        )
        .to_vec(),
      )
    );
    if let Some(storage) = &pallet.storage {
      for entry in &storage.entries {
        assert!(
          stems.insert(
            polkadot_sdk::frame_support::storage::storage_prefix(
              storage.prefix.as_bytes(),
              entry.name.as_bytes(),
            )
            .to_vec(),
          )
        );
        storage_count += 1;
      }
    }
  }
  assert_eq!(storage_count, 239);
  assert!(
    stems.insert(
      polkadot_sdk::frame_support::storage::storage_prefix(b"XcmpQueue", b"InboundXcmpStatus")
        .to_vec(),
    )
  );
  assert!(stems.len() <= 512);
  stems
}

fn actor_suffix_owner(
  name: &str,
  prefix_bytes: usize,
  image: &[[u8; 24]],
  key_for: impl Fn(u64) -> Vec<u8>,
  storage_info: &[polkadot_sdk::frame_support::traits::StorageInfo],
  stems: &std::collections::BTreeSet<Vec<u8>>,
  costs: &[usize],
) -> serde_json::Value {
  let prefix = key_for(0)[..prefix_bytes].to_vec();
  assert_eq!(
    &prefix[..32],
    polkadot_sdk::frame_support::storage::storage_prefix(b"Actors", name.as_bytes())
  );
  for actor in 0..ACTOR_ID_END {
    let key = key_for(actor);
    assert_eq!(key.len(), prefix_bytes + 24);
    assert_eq!(&key[..prefix_bytes], prefix);
    assert_eq!(&key[prefix_bytes..], image[actor as usize]);
  }
  assert_eq!(storage_info.len(), 1);
  let info = &storage_info[0];
  assert_eq!(info.prefix, prefix[..32]);
  assert_eq!(info.storage_name, name.as_bytes());
  let size = info.max_size.unwrap();
  assert!(size < u32::MAX);
  // StorageInfo excludes the 32-byte namespace, but includes every hashed key suffix.
  let value_bytes = usize::try_from(size).unwrap() - (prefix_bytes + 24 - 32);
  let value_blob = value_bytes + codec::Compact(u32::try_from(value_bytes).unwrap()).encoded_size();
  let depths = stems
    .iter() // deos-bypass: bounded-iter -- at most 512 guarded runtime namespace stems
    .filter_map(|stem| {
      let matching = stem
        .iter() // deos-bypass: bounded-iter -- at most 32 bytes per guarded namespace stem
        .flat_map(|byte| [byte >> 4, byte & 15])
        .zip(prefix[..32].iter().flat_map(|byte| [byte >> 4, byte & 15])) // deos-bypass: bounded-iter -- fixed 32-byte namespace prefix
        .take_while(|(left, right)| left == right)
        .count();
      (matching < 64).then_some(matching)
    })
    .collect::<std::collections::BTreeSet<_>>();
  let global_nodes = depths.len() + 1;
  let source_nodes = 2 * (prefix_bytes - 32);
  let inputs = [1, 2, 4, 32, 63].map(|queries| {
    // Branch/leaf overhead owns odd-partial rounding; floor-partial bytes sum
    // to at most 24 per selected path. Payloads are charged even when inline.
    let read = (global_nodes + source_nodes) * 650
      + costs[queries]
      + queries * (24 + 37 + value_blob)
      + 1284
      + 5;
    // At most one sibling is needed per removed terminal; an additional foreign
    // sibling covers final destruction of this complete protected prefix.
    json!({"queries": queries, "readInputBytes": read,
      "readAndAllSelectedDeletedInputBytes": read + queries * 558 + 1284})
  });
  json!({"owner": name, "prefixBytes": prefix_bytes,
    "keyBytes": prefix_bytes + 24, "checkedActorIds": ACTOR_ID_END,
    "maxEncodedValueBytes": value_bytes, "maxValueBlobBytes": value_blob,
    "globalAncestorNodes": global_nodes, "sourceAncestorNodes": source_nodes,
    "conditionalPrefixInputs": inputs})
}

fn actor_prefix_witness(owner: &serde_json::Value) {
  use pallet_deos_actors::{ActorContractHeads, ActorSemanticStates};
  use polkadot_sdk::sp_trie::{
    MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder, delta_trie_root,
  };
  type Layout = LayoutV1<Blake2Hasher>;
  assert_eq!(owner["owner"], "ActorContractHead");
  let value_bytes = owner["maxEncodedValueBytes"].as_u64().unwrap() as usize;
  for (case, rows_count, present_count, bound_index) in [
    ("full", ACTOR_ID_END, 62u64, 4usize),
    ("prefix_last", 1, 1, 1),
  ] {
    // These are codec witnesses with bounded synthetic bytes, not admitted Actor records.
    let mut rows = (0..rows_count)
      .map(|actor| {
        let mut value = vec![7; value_bytes];
        value[..8].copy_from_slice(&actor.to_le_bytes());
        (ActorContractHeads::<Runtime>::hashed_key_for(actor), value)
      })
      .collect::<Vec<_>>();
    let peer = ActorSemanticStates::<Runtime>::hashed_key_for(0u64);
    rows.push((
      peer.clone(),
      vec![9; pallet_deos_actors::ActorSemanticStateOf::<Runtime>::max_encoded_len()],
    ));
    rows.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    let mut database = MemoryDB::<Blake2Hasher>::default();
    let root = Layout::trie_build(&mut database, rows.clone());
    let mut recorder = Recorder::<Layout>::new();
    let absent = ActorContractHeads::<Runtime>::hashed_key_for(u64::MAX);
    {
      let trie = TrieDBBuilder::<Layout>::new(&database, &root)
        .with_recorder(&mut recorder)
        .build();
      for actor in 0..present_count {
        assert_eq!(
          trie
            .get(&ActorContractHeads::<Runtime>::hashed_key_for(actor))
            .unwrap()
            .unwrap()
            .len(),
          value_bytes
        );
      }
      assert_eq!(trie.get(&absent).unwrap(), None);
    }
    let mut records = recorder.drain();
    let read_proof = StorageProof::new(
      records
        .iter() // deos-bypass: bounded-iter -- at most 63 selected reads in a fixed 9989-key V1 image
        .map(|record| record.data.clone()),
    );
    let read_bytes = read_proof.encoded_size();
    let read_bound = owner["conditionalPrefixInputs"][bound_index]["readInputBytes"]
      .as_u64()
      .unwrap() as usize;
    assert!(read_bytes <= read_bound);
    let delta = (0..present_count)
      .map(|actor| {
        (
          ActorContractHeads::<Runtime>::hashed_key_for(actor),
          None::<Vec<u8>>,
        )
      })
      .collect::<Vec<_>>();
    let post = delta_trie_root::<Layout, _, _, _, _, _>(
      &mut database,
      root,
      delta.clone(),
      Some(&mut recorder),
      None,
    )
    .unwrap();
    rows.retain(|(key, _)| {
      key == &peer
        || !delta
          .iter() // deos-bypass: bounded-iter -- at most 62 selected terminal deletions
          .any(|(removed, _)| removed == key)
    });
    assert_eq!(post, Layout::trie_root(rows));
    records.extend(recorder.drain());
    let proof = StorageProof::new(records.into_iter().map(|record| record.data));
    let net_bytes = proof.encoded_size();
    let net_bound =
      owner["conditionalPrefixInputs"][bound_index]["readAndAllSelectedDeletedInputBytes"]
        .as_u64()
        .unwrap() as usize;
    assert!(net_bytes <= net_bound);
    let mut replay = proof.into_memory_db::<Blake2Hasher>();
    let trie = TrieDBBuilder::<Layout>::new(&replay, &root).build();
    assert_eq!(trie.get(&absent).unwrap(), None);
    drop(trie);
    assert_eq!(
      delta_trie_root::<Layout, _, _, _, _, _>(&mut replay, root, delta, None, None).unwrap(),
      post
    );
    println!(
      "{}",
      json!({"scope": "conditional_actor_prefix_codec_witness", "case": case,
        "protectedRows": rows_count, "presentReads": present_count, "absenceReads": 1,
        "valueBytes": value_bytes, "recordedReadBytes": read_bytes, "readBoundBytes": read_bound,
        "recordedNetDeleteBytes": net_bytes, "netDeleteBoundBytes": net_bound,
        "netRootReplay": true}),
    );
  }
}

fn actor_domain() {
  use pallet_deos_actors::*;
  use polkadot_sdk::frame_support::traits::StorageInfoTrait;
  type Layout = LayoutV1<Blake2Hasher>;
  assert!(!Layout::USE_EXTENSION);
  assert_eq!(Layout::MAX_INLINE_VALUE, Some(33));
  assert_eq!(ActorId::max_encoded_len(), 8);
  let keys = (0..ACTOR_ID_END)
    .map(|actor| {
      let key = ActorSemanticStates::<Runtime>::hashed_key_for(actor);
      assert_eq!(key.len(), 56);
      let suffix = <[u8; 24]>::try_from(&key[32..]).unwrap();
      assert_eq!(&suffix[16..], actor.encode());
      assert_eq!(
        &suffix[..16],
        polkadot_sdk::sp_io::hashing::blake2_128(&actor.encode())
      );
      suffix
    })
    .collect::<Vec<_>>();
  let costs = curve(keys.clone());
  let stems = runtime_namespace_stems();
  let mut owners = Vec::new();
  macro_rules! map_owner {
    ($name:literal, $map:ident) => {
      owners.push(actor_suffix_owner(
        $name,
        32,
        &keys,
        |actor| $map::<Runtime>::hashed_key_for(actor),
        &$map::<Runtime>::storage_info(),
        &stems,
        &costs,
      ));
    };
  }
  map_owner!("ActorSemanticStates", ActorSemanticStates);
  map_owner!("ActorIdentities", ActorIdentities);
  map_owner!("ActorContractHead", ActorContractHeads);
  map_owner!("ActorProcesses", ActorProcesses);
  map_owner!("ActorRunState", ActorRunStateStore);
  map_owner!("ActorStateHolds", ActorStateHolds);
  map_owner!("ActorControlLocators", ActorControlLocators);
  map_owner!("ActorUnsignaledControlCells", ActorUnsignaledControlCells);
  map_owner!("DeadlineHandles", DeadlineHandles);
  map_owner!("TriggerDeadlineHandles", TriggerDeadlineHandles);
  map_owner!("PendingCheckOwners", PendingCheckOwners);
  map_owner!("DependencyPlans", DependencyPlans);
  map_owner!("ParkedBalanceEpisodes", ParkedBalanceEpisodes);
  map_owner!("DependencyTimedReviews", DependencyTimedReviews);
  map_owner!("PendingDependencyEvents", PendingDependencyEvents);
  map_owner!("PendingDependencyReviews", PendingDependencyReviews);
  map_owner!("ServiceNodes", ServiceNodes);
  owners.push(actor_suffix_owner(
    "DependencyRegistrations",
    56,
    &keys,
    |actor| DependencyRegistrations::<Runtime>::hashed_key_for(u64::MAX, actor),
    &DependencyRegistrations::<Runtime>::storage_info(),
    &stems,
    &costs,
  ));
  owners.push(actor_suffix_owner(
    "DependencyRegistrationPositions",
    56,
    &keys,
    |actor| DependencyRegistrationPositions::<Runtime>::hashed_key_for(u64::MAX, actor),
    &DependencyRegistrationPositions::<Runtime>::storage_info(),
    &stems,
    &costs,
  ));
  let image_digest = digest(&keys.encode());
  println!(
    "{}",
    json!({"scope": "conditional_recipe_actor_id_branch_input",
      "inputSha256": INPUT_SHA256,
      "actorsLibSha256": digest(include_bytes!("../../pallets/actors/src/lib.rs")),
      "cargoLockSha256": digest(include_bytes!("../../Cargo.lock")),
      "actorIdEndExclusive": ACTOR_ID_END, "suffixBytes": 24,
      "scaleSuffixImageSha256": image_digest, "maxSelectedPaths": CAP,
      "runtimeNamespaceStemCount": stems.len(),
      "namespaceStemsSha256": digest(&stems.encode()),
      "leafAndRoundingOverheadBytes": 37, "typedDeletionSiblingBytes": 558,
      "foreignSiblingBytes": 1284, "ancestorNodeBytes": 650,
      "branchHeaderBitmapVectorReferenceBytes": costs, "checkedOwners": owners}),
  );
  actor_prefix_witness(&owners[2]);
  println!(
    "Conditional Actor-prefix read/net-delete inputs complete; whole-call selection, complete FRAME/Close, public-prefix/repetition, host CPU and binding remain open."
  );
}

fn absence_witness(pages: &[u64]) {
  use polkadot_sdk::sp_trie::{
    MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder,
  };
  type Layout = LayoutV1<Blake2Hasher>;
  let source = u64::MAX;
  let prefix =
    Indices::<Runtime>::hashed_key_for(source, DependencyRegistrationPosition { page: 0, slot: 1 })
      [..56]
      .to_vec();
  let mut suffixes = Vec::with_capacity(31 * PAGES);
  for page in pages {
    for slot in 1..32 {
      let key = Indices::<Runtime>::hashed_key_for(
        source,
        DependencyRegistrationPosition { page: *page, slot },
      );
      assert_eq!(&key[..56], prefix);
      suffixes.push(<[u8; 25]>::try_from(&key[56..]).unwrap());
    }
  }
  let costs = curve(suffixes.clone());
  suffixes.sort_unstable();
  let bound = (5 + 48) * 650 + costs[32] + 32 * 39 + 1284 + 5;
  let queries = (0..32)
    .map(|slot| {
      Indices::<Runtime>::hashed_key_for(
        source,
        DependencyRegistrationPosition {
          page: u64::MAX,
          slot,
        },
      )
    })
    .collect::<Vec<_>>();
  // A full image, a compressed subset and an empty protected prefix use actual V1
  // recording/replay. The peer is typed but outside the protected source prefix.
  for (case, stride) in [("full", 1), ("sparse", 97), ("empty", usize::MAX)] {
    let mut rows = Vec::new();
    if case != "empty" {
      for (offset, suffix) in suffixes
        .iter() // deos-bypass: bounded-iter -- 31 * PAGES fixed corpus suffixes
        .step_by(stride)
        .enumerate()
      {
        let mut key = prefix.clone();
        key.extend_from_slice(suffix);
        rows.push((key, (offset as u64).encode()));
      }
    }
    let protected_rows = rows.len();
    rows.push((
      Indices::<Runtime>::hashed_key_for(
        source - 1,
        DependencyRegistrationPosition { page: 0, slot: 1 },
      ),
      0u64.encode(),
    ));
    rows.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    let mut database = MemoryDB::<Blake2Hasher>::default();
    let root = Layout::trie_build(&mut database, rows);
    let mut recorder = Recorder::<Layout>::new();
    {
      let trie = TrieDBBuilder::<Layout>::new(&database, &root)
        .with_recorder(&mut recorder)
        .build();
      for key in &queries {
        assert_eq!(trie.get(key).unwrap(), None);
      }
    }
    let proof = StorageProof::new(recorder.drain().into_iter().map(|record| record.data));
    let bytes = proof.encoded_size();
    assert!(bytes <= bound);
    let replay = proof.into_memory_db::<Blake2Hasher>();
    let trie = TrieDBBuilder::<Layout>::new(&replay, &root).build();
    for key in &queries {
      assert_eq!(trie.get(key).unwrap(), None);
    }
    println!(
      "{}",
      json!({"scope": "typed_corpus_absence_trie_witness", "case": case,
        "protectedRows": protected_rows, "absenceQueries": queries.len(),
        "recordedEncodedProofBytes": bytes, "conditionalReadBoundBytes": bound,
        "proofReplay": true}),
    );
  }
}

fn runtime_storage_catalog() -> serde_json::Value {
  use codec::Decode;
  use polkadot_sdk::frame_support::{
    __private::metadata::{RuntimeMetadata, RuntimeMetadataPrefixed, v16::StorageEntryType},
    traits::{STORAGE_VERSION_STORAGE_KEY_POSTFIX, StorageInfoTrait},
  };
  let encoded = Runtime::metadata_at_version(16).unwrap();
  let mut input = &encoded[..];
  let RuntimeMetadataPrefixed(_, RuntimeMetadata::V16(metadata)) =
    RuntimeMetadataPrefixed::decode(&mut input).unwrap()
  else {
    panic!("native metadata must be v16")
  };
  assert!(input.is_empty());
  assert_eq!(metadata.pallets.len(), 27);
  let info = deos_runtime::AllPalletsWithSystem::storage_info();
  let mut entries = Vec::new();
  let mut versions = Vec::new();
  let mut stems = std::collections::BTreeSet::new();
  for pallet in &metadata.pallets {
    let prefix = polkadot_sdk::frame_support::storage::storage_prefix(
      pallet.name.as_bytes(),
      STORAGE_VERSION_STORAGE_KEY_POSTFIX,
    );
    assert!(stems.insert(prefix.to_vec()));
    versions
      .push(json!({"owner": format!("{}.FrameStorageVersion", pallet.name), "prefix": prefix}));
    if let Some(storage) = &pallet.storage {
      for entry in &storage.entries {
        let prefix = polkadot_sdk::frame_support::storage::storage_prefix(
          storage.prefix.as_bytes(),
          entry.name.as_bytes(),
        );
        assert!(stems.insert(prefix.to_vec()));
        let matched = info
          .iter() // deos-bypass: bounded-iter -- native metadata and StorageInfo contain fixed compiled declarations
          .filter(|item| item.prefix == prefix)
          .collect::<Vec<_>>();
        assert_eq!(matched.len(), 1);
        let hashers = match &entry.ty {
          StorageEntryType::Plain(_) => 0,
          StorageEntryType::Map { hashers, .. } => hashers.len(),
        };
        entries.push(json!({"owner": format!("{}.{}", pallet.name, entry.name),
          "storagePrefix": storage.prefix, "prefix": prefix, "mapHashers": hashers,
          "modifier": format!("{:?}", entry.modifier), "maxValues": matched[0].max_values,
          "maxSize": matched[0].max_size}));
      }
    }
  }
  assert_eq!(entries.len(), 239);
  let complete_stems = runtime_namespace_stems();
  let outside_metadata = complete_stems
    .difference(&stems)
    .cloned()
    .collect::<Vec<_>>(); // deos-bypass: bounded-iter -- runtime_namespace_stems guards at most 512 compiled stems
  assert_eq!(outside_metadata.len(), 8);
  assert_eq!(
    complete_stems.len(),
    entries.len() + versions.len() + outside_metadata.len()
  );
  json!({"scope": "current_native_runtime_storage_catalog",
    "cargoLockSha256": digest(include_bytes!("../../Cargo.lock")),
    "metadataSha256": digest(&encoded), "metadataVersion": 16,
    "typedStorageOwners": entries, "storageVersionOwners": versions,
    "nonmetadataStems": outside_metadata, "namespaceStemsSha256": digest(&complete_stems.encode()),
    "writerReachabilityProved": false, "productionMetadataBinding": false})
}

fn main() {
  let args = std::env::args().collect::<Vec<_>>();
  if args.len() == 2 && args[1] == "--storage-catalog" {
    polkadot_sdk::sp_io::TestExternalities::new_empty().execute_with(|| {
      println!("{}", runtime_storage_catalog());
    });
    return;
  }
  if args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") {
    println!(
      "Usage: actor_retention_hole_domain --storage-catalog\nReports every current native metadata storage owner, its StorageInfo and nonmetadata stems without corpus enumeration, block execution or production metadata binding."
    );
    println!(
      "Fixed-corpus hole-prefix diagnostic.\nUsage: actor_retention_hole_domain FIXED_PAGE_JSON [--absence-witness|--actor-domain|--signed-pair-input|--birth-input|--frame-kernel-input|--quiet-hook-input|--parachain-input|--inherent-length-input|--session-successor-witness|--session-input|--session-successor-interval|--economic-effect-input]\nDefault enumerates finite typed hash images and maximizes up to 63 terminal-path unions. --absence-witness separately records/replays 32 absent reads in full/sparse/empty first-source V1 tries. Bounds reads of arbitrary absent keys inside a coherent subset of the same protected source prefix. The forward offsets include the provisional retirement slot; reverse keys contain only permanent corpus pages/slots 1..31. --actor-domain separately checks all Actor IDs 0..9989 under nineteen typed owners and reports branch unions and conditional per-prefix read/net-delete inputs with typed values and guarded ancestors. Its range requires the closed recipe allocator premise and the declared coherent namespace/key inventory. --signed-pair-input selects the fixed recipe's distinct one-StopCycle Manual Actors, four retained scanning sources and Block-only review/heap roles; it reports Actors-owned read/net-delete inputs, with the separately qualified hole-table read supplied explicitly as an external input, plus twenty currency/FRAME storage roles after Executive initialization, Events reset and consensus inherents. --birth-input reports the additional typed reads, coded declaration and conservative signed length for ordinary Manual/Parked creation replacing activation; it also reports maximum-width Close-target codecs, whose shared-read reuse requires the separately audited initial Parked publication path. --frame-kernel-input separately reports eighteen direct System/Executive initialization/finalization roles for at most five extrinsics, with three empty-EventTopics V1 prefix recording/replay witnesses. --quiet-hook-input separately reports twenty-eight ordinary nonrotating consensus, Timestamp and quiet Governance/message hook storage roles under fixed TrustedSet and empty-state premises. --parachain-input separately reports thirty-five Parachain inherent/finalization, consensus callback and empty outbound roles, plus bounded synthetic V1 overwrite/deletion root replays. --inherent-length-input reuses the selected constructor and reports a seven-key relay-trie/SCALE length upper with finite Compact/header witnesses; it creates and encodes three inherents without applying blocks. --session-successor-witness compares empty prefixed seek and global next-key geometry under two bounded synthetic neighbor spines. --session-input separately reports sixteen scheduled TrustedSet Session roles and a source-bounded global-successor input with one possible overlay skip. --session-successor-interval bounds the global seek through the live genesis Asset(0) key, native metadata widths and both named presets; it preserves the original closed-recipe conditions. --economic-effect-input separately reports twenty-two ledger, predicate, Dormant recipient and active LP-source publication roles for the four producers and TrustedSet Fee Sink. Actor scheduler bodies, full PVF/host composition, physical admission and repetition remain separate. This is conditional input arithmetic, not a history replay, public chronology, whole FRAME/Close proof, physical Weight, tariff or production binding."
    );
    return;
  }
  if args.get(2).is_some_and(|arg| arg == "--birth-input") {
    assert_eq!(args.len(), 3);
    let _ = fixed_pages(&args[1]);
    polkadot_sdk::sp_io::TestExternalities::new_empty().execute_with(|| {
      let stems = runtime_namespace_stems();
      println!(
        "{}",
        json!({"scope": "conditional_recipe_manual_birth_input",
        "inputSha256": INPUT_SHA256,
        "cargoLockSha256": digest(include_bytes!("../../Cargo.lock")),
        "namespaceStemsSha256": digest(&stems.encode()),
        "namespaceStems": stems.iter().map(|key| key.iter().map(|byte| format!("{byte:02x}")).collect::<String>()).collect::<Vec<_>>(), // deos-bypass: bounded-iter -- at most 512 guarded namespace stems
        "birth": birth::input(&stems)})
      );
    });
    return;
  }
  let witness = args.get(2).is_some_and(|arg| arg == "--absence-witness");
  let actors = args.get(2).is_some_and(|arg| arg == "--actor-domain");
  let pair = args.get(2).is_some_and(|arg| arg == "--signed-pair-input");
  let kernel = args.get(2).is_some_and(|arg| arg == "--frame-kernel-input");
  let quiet = args.get(2).is_some_and(|arg| arg == "--quiet-hook-input");
  let para = args.get(2).is_some_and(|arg| arg == "--parachain-input");

  if args
    .get(2)
    .is_some_and(|arg| arg == "--inherent-length-input")
  {
    assert_eq!(args.len(), 3);
    let _ = fixed_pages(&args[1]);
    polkadot_sdk::sp_io::TestExternalities::new_empty().execute_with(|| {
      println!(
        "{}",
        json!({"scope": "conditional_recipe_inherent_encoded_length",
          "inputSha256": INPUT_SHA256,
          "cargoLockSha256": digest(include_bytes!("../../Cargo.lock")),
          "inherentLengths": inherent_lengths::input()})
      );
    });
    return;
  }

  if args
    .get(2)
    .is_some_and(|arg| arg == "--session-successor-interval")
  {
    assert_eq!(args.len(), 3);
    let _ = fixed_pages(&args[1]);
    polkadot_sdk::sp_io::TestExternalities::new_empty().execute_with(|| {
      let stems = runtime_namespace_stems();
      let input = signed_envelope::session_successor_interval(&stems);
      println!(
        "{}",
        json!({"scope": "conditional_recipe_session_successor_interval",
          "inputSha256": INPUT_SHA256,
          "cargoLockSha256": digest(include_bytes!("../../Cargo.lock")),
          "namespaceStemsSha256": digest(&stems.encode()),
          "namespaceStems": stems.iter().map(|key| key.iter().map(|byte| format!("{byte:02x}")).collect::<String>()).collect::<Vec<_>>(), // deos-bypass: bounded-iter -- at most 512 namespace stems of at most 32 bytes each
          "successorInterval": input})
      );
    });
    return;
  }
  let economic = args
    .get(2)
    .is_some_and(|arg| arg == "--economic-effect-input");
  if economic {
    assert_eq!(args.len(), 3);
    let _ = fixed_pages(&args[1]);
    polkadot_sdk::sp_io::TestExternalities::new_empty().execute_with(|| {
      let stems = runtime_namespace_stems();
      let input = signed_envelope::economic_effect_input(&stems);
      println!(
        "{}",
        json!({"scope": "conditional_recipe_economic_effect_input",
          "inputSha256": INPUT_SHA256,
          "cargoLockSha256": digest(include_bytes!("../../Cargo.lock")),
          "namespaceStemsSha256": digest(&stems.encode()),
          "namespaceStems": stems.iter().map(|key| key.iter().map(|byte| format!("{byte:02x}")).collect::<String>()).collect::<Vec<_>>(), // deos-bypass: bounded-iter -- at most 512 namespace stems of at most 32 bytes each
          "economicEffects": input})
      );
    });
    return;
  }
  let rotation = args.get(2).is_some_and(|arg| arg == "--session-input");
  if rotation {
    assert_eq!(args.len(), 3);
    let _ = fixed_pages(&args[1]);
    polkadot_sdk::sp_io::TestExternalities::new_empty().execute_with(|| {
      let stems = runtime_namespace_stems();
      let input = signed_envelope::session_input(&stems);
      println!(
        "{}",
        json!({"scope": "conditional_recipe_session_input",
          "inputSha256": INPUT_SHA256,
          "cargoLockSha256": digest(include_bytes!("../../Cargo.lock")),
          "namespaceStemsSha256": digest(&stems.encode()),
          "namespaceStems": stems.iter().map(|key| key.iter().map(|byte| format!("{byte:02x}")).collect::<String>()).collect::<Vec<_>>(), // deos-bypass: bounded-iter -- at most 512 namespace stems of at most 32 bytes each
          "session": input})
      );
    });
    return;
  }
  let sess = args
    .get(2)
    .is_some_and(|arg| arg == "--session-successor-witness");
  if sess {
    assert_eq!(args.len(), 3);
    let _ = fixed_pages(&args[1]);
    polkadot_sdk::sp_io::TestExternalities::new_empty().execute_with(|| {
      let stems = runtime_namespace_stems();
      let witness = signed_envelope::session_successor_witness(&stems);
      println!(
        "{}",
        json!({"scope": "synthetic_session_successor_witness",
          "inputSha256": INPUT_SHA256,
          "cargoLockSha256": digest(include_bytes!("../../Cargo.lock")),
          "namespaceStemsSha256": digest(&stems.encode()),
          "namespaceStems": stems.iter().map(|key| key.iter().map(|byte| format!("{byte:02x}")).collect::<String>()).collect::<Vec<_>>(), // deos-bypass: bounded-iter -- at most 512 namespace stems of at most 32 bytes each
          "successor": witness})
      );
    });
    return;
  }
  assert!(
    args.len() == 2 || (args.len() == 3 && (witness || actors || pair || kernel || quiet || para)),
    "see --help"
  );
  let pages = fixed_pages(&args[1]);
  if para {
    polkadot_sdk::sp_io::TestExternalities::new_empty().execute_with(|| {
      let stems = runtime_namespace_stems();
      let input = signed_envelope::parachain_input(&stems);
      println!(
        "{}",
        json!({"scope": "conditional_recipe_parachain_input",
          "inputSha256": INPUT_SHA256,
          "cargoLockSha256": digest(include_bytes!("../../Cargo.lock")),
          "namespaceStemsSha256": digest(&stems.encode()),
          "namespaceStems": stems.iter().map(|key| key.iter().map(|byte| format!("{byte:02x}")).collect::<String>()).collect::<Vec<_>>(), // deos-bypass: bounded-iter -- at most 512 namespace stems of at most 32 bytes each
          "parachain": input})
      );
    });
    return;
  }
  if quiet {
    polkadot_sdk::sp_io::TestExternalities::new_empty().execute_with(|| {
      let stems = runtime_namespace_stems();
      let input = signed_envelope::quiet_hooks_input(&stems);
      println!(
        "{}",
        json!({"scope": "conditional_recipe_quiet_hook_input",
          "inputSha256": INPUT_SHA256,
          "cargoLockSha256": digest(include_bytes!("../../Cargo.lock")),
          "namespaceStemsSha256": digest(&stems.encode()),
          "namespaceStems": stems.iter().map(|key| key.iter().map(|byte| format!("{byte:02x}")).collect::<String>()).collect::<Vec<_>>(), // deos-bypass: bounded-iter -- at most 512 namespace stems of at most 32 bytes each
          "hooks": input})
      );
    });
    return;
  }
  if kernel {
    polkadot_sdk::sp_io::TestExternalities::new_empty().execute_with(|| {
      let stems = runtime_namespace_stems();
      let input = signed_envelope::frame_kernel_input(&stems);
      println!(
        "{}",
        json!({"scope": "conditional_recipe_frame_kernel_input",
          "inputSha256": INPUT_SHA256,
          "cargoLockSha256": digest(include_bytes!("../../Cargo.lock")),
          "namespaceStemsSha256": digest(&stems.encode()),
          "namespaceStems": stems.iter().map(|key| key.iter().map(|byte| format!("{byte:02x}")).collect::<String>()).collect::<Vec<_>>(), // deos-bypass: bounded-iter -- at most 512 namespace stems of at most 32 bytes each
          "kernel": input})
      );
    });
    return;
  }
  if pair {
    polkadot_sdk::sp_io::TestExternalities::new_empty().execute_with(|| signed_pair::input(&pages));
    return;
  }
  if actors {
    polkadot_sdk::sp_io::TestExternalities::new_empty().execute_with(actor_domain);
    return;
  }
  if witness {
    absence_witness(&pages[0]);
    return;
  }
  type Layout = LayoutV1<Blake2Hasher>;
  assert!(!Layout::USE_EXTENSION);
  assert_eq!(Layout::MAX_INLINE_VALUE, Some(33));
  assert_eq!(DependencyRegistrationPosition::max_encoded_len(), 9);
  assert_eq!(u64::max_encoded_len(), 8);
  println!(
    "scope=fixed_corpus_hole_key_domain input_sha256={INPUT_SHA256} actors_lib_sha256={} cargo_lock_sha256={} permanent_holes_per_source={}",
    digest(include_bytes!("../../pallets/actors/src/lib.rs")),
    digest(include_bytes!("../../Cargo.lock")),
    31 * PAGES,
  );
  let source = u64::MAX;
  let prefix = Positions::<Runtime>::hashed_key_for(source, 0)[..56].to_vec();
  let forward = curve(
    (0..=31 * PAGES)
      .map(|offset| {
        let key = Positions::<Runtime>::hashed_key_for(source, offset as u64);
        assert_eq!(key.len(), 80);
        assert_eq!(&key[..56], prefix);
        <[u8; 24]>::try_from(&key[56..]).unwrap()
      })
      .collect(),
  );
  println!(
    "forward image complete: offsets={} cap={CAP}",
    31 * PAGES + 1
  );
  let mut reverse = Vec::new();
  for (index, pages) in pages
    .iter() // deos-bypass: bounded-iter -- four hash-bound corpus sources
    .enumerate()
  {
    let prefix = Indices::<Runtime>::hashed_key_for(
      source,
      DependencyRegistrationPosition { page: 0, slot: 1 },
    )[..56]
      .to_vec();
    let mut keys = Vec::with_capacity(31 * PAGES);
    for page in pages {
      for slot in 1..32 {
        let key = Indices::<Runtime>::hashed_key_for(
          source,
          DependencyRegistrationPosition { page: *page, slot },
        );
        assert_eq!(key.len(), 81);
        assert_eq!(&key[..56], prefix);
        keys.push(<[u8; 25]>::try_from(&key[56..]).unwrap());
      }
    }
    reverse.push(curve(keys));
    println!(
      "reverse image complete: source={index} keys={} cap={CAP}",
      31 * PAGES
    );
  }
  assert_eq!(reverse.len(), WATCHES);
  let shared_source_ancestors = (5 + 48 * WATCHES) * 650 + 5;
  for queries in [1, 32, 63] {
    let forward_read = shared_source_ancestors + WATCHES * (forward[queries] + queries * 39 + 1284);
    let reverse_read = shared_source_ancestors
      + reverse
        .iter() // deos-bypass: bounded-iter -- four source-domain curves
        .map(|costs| costs[queries] + queries * 39 + 1284)
        .sum::<usize>();
    println!(
      "{}",
      json!({
        "scope": "conditional_fixed_corpus_hole_prefix_input", "queriesPerSource": queries,
        "forwardBranchBytesPerSource": forward[queries],
        "reverseBranchBytesPerSource": reverse.iter(/* deos-bypass: bounded-iter -- four source-domain curves */).map(|c| c[queries]).collect::<Vec<_>>(),
        "forwardReadBytes": forward_read, "reverseReadBytes": reverse_read,
        "bothReadBytes": forward_read + reverse_read,
        "bothReadAnd31TypedDeletionsBytes": forward_read + reverse_read + WATCHES * 31 * (558 + 559),
      })
    );
  }
  println!(
    "Conditional key-domain inputs complete; public-prefix/repetition, whole FRAME/Close, host CPU and binding remain open."
  );
}
