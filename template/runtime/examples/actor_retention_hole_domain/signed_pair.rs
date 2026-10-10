//! Conditional storage inputs for the fixed recipe's two distinct temporary Actors.
//! Source-call selection is reviewed separately from these codec arithmetic inputs.
use super::{ACTOR_ID_END, CAP, actor_suffix_owner, branch_union, curve, runtime_namespace_stems};
use crate::actor_retention_corpus::{INPUT_SHA256, PAGES, WATCHES, digest};
use codec::Encode;
use deos_runtime::Runtime;
use pallet_deos_actors::*;
use polkadot_sdk::frame_support::traits::{StorageInfo, StorageInfoTrait};
use serde_json::{Value, json};
use std::collections::BTreeSet;

// The key-width argument belongs to StorageInfo's maximum key, including Tick variants.
// The protected prefix/key below belongs to this Block-only selected query role.
struct Geometry {
  prefix_bytes: usize,
  max_key_bytes: usize,
  queries: usize,
  deletions: usize,
  finite_branch_bytes: Option<usize>,
  added_terminal_bytes: usize,
}

fn row(
  key: Vec<u8>,
  infos: Vec<StorageInfo>,
  stems: &BTreeSet<Vec<u8>>,
  geometry: Geometry,
) -> Value {
  let Geometry {
    prefix_bytes,
    max_key_bytes,
    queries,
    deletions,
    finite_branch_bytes,
    added_terminal_bytes,
  } = geometry;
  assert_eq!(infos.len(), 1);
  let info = &infos[0];
  assert_eq!(info.prefix, key[..32]);
  assert!((32..=81).contains(&max_key_bytes));
  assert!((32..=key.len()).contains(&prefix_bytes));
  assert!(key.len() <= max_key_bytes && queries <= CAP && deletions <= queries);
  let maximum = info.max_size.unwrap();
  assert!(maximum < u32::MAX);
  let value_bytes = maximum as usize - (max_key_bytes - 32);
  let value_blob = value_bytes + codec::Compact(value_bytes as u32).encoded_size();
  let depths = stems
    .iter() // deos-bypass: bounded-iter -- at most 512 guarded runtime namespace stems
    .filter_map(|stem| {
      let matching = stem
        .iter() // deos-bypass: bounded-iter -- at most 32 bytes per namespace stem
        .flat_map(|byte| [byte >> 4, byte & 15])
        .zip(key[..32].iter().flat_map(|byte| [byte >> 4, byte & 15])) // deos-bypass: bounded-iter -- fixed 32-byte namespace prefix
        .take_while(|(left, right)| left == right)
        .count();
      (matching < 64).then_some(matching)
    })
    .collect::<BTreeSet<_>>();
  let global_nodes = depths.len() + 1;
  let source_nodes = 2 * (prefix_bytes - 32);
  let suffix_bytes = key.len() - prefix_bytes;
  let (read, deletion_extra, method) = if let Some(branches) = finite_branch_bytes {
    assert!(matches!(suffix_bytes, 24 | 48));
    (
      (global_nodes + source_nodes) * 650
        + branches
        + added_terminal_bytes
        + queries * (suffix_bytes + 37 + value_blob)
        + 1284
        + 5,
      // Joined 48-byte roles retain permanent rows in every source. Deletion
      // therefore collapses only inside that source's 24-byte suffix, where the
      // qualified 558-byte sibling allowance applies; no entire source is removed.
      deletions * 558 + usize::from(deletions > 0) * 1284,
      "finite_equal_width_suffix_image",
    )
  } else {
    assert_eq!(added_terminal_bytes, 0);
    // Reuse the package guard's path-node and foreign/collapse allowances. Selected
    // Block queries have a 5-byte WakeupKey payload, while stored Tick keys may have
    // 9. Their concatenated variant byte differs before the end of a Block key, so
    // no Block key is a proper prefix of a Tick key, including hash collisions.
    // DeadlinePages protects this entire Block key before the fixed u64 page suffix.
    // StorageInfo still uses the maximum key to derive V; foreign nodes retain the
    // unchanged longer-key guard. No future Block hash image is enumerated.
    (
      (global_nodes + source_nodes + 2 * queries * suffix_bytes) * 650
        + queries * (1284 + value_blob)
        + 5,
      deletions * 1284,
      "guarded_selected_key_typed_prefix",
    )
  };
  json!({"owner": String::from_utf8(info.storage_name.clone()).unwrap(),
    "method": method, "prefixBytes": prefix_bytes, "maxKeyBytes": max_key_bytes,
    "selectedSuffixBytes": suffix_bytes,
    "queries": queries, "deletions": deletions,
    "maxEncodedValueBytes": value_bytes, "maxValueBlobBytes": value_blob,
    "globalAncestorNodes": global_nodes, "sourceAncestorNodes": source_nodes,
    "finiteBranchBytes": finite_branch_bytes, "addedTerminalBytes": added_terminal_bytes,
    "readInputBytes": read, "readAndNetDeleteInputBytes": read + deletion_extra})
}

pub fn input(pages: &[Vec<u64>]) {
  assert_eq!(pages.len(), WATCHES);
  assert_eq!(<Runtime as Config>::MaxActiveActors::get(), 10_000);
  let stems = runtime_namespace_stems();
  let actors = (0..ACTOR_ID_END)
    .map(|actor| {
      let key = ActorSemanticStates::<Runtime>::hashed_key_for(actor);
      <[u8; 24]>::try_from(&key[32..]).unwrap()
    })
    .collect::<Vec<_>>();
  let actor_costs = curve(actors.clone());
  let mut owners = Vec::new();
  macro_rules! actor {
    ($map:ident) => {{
      let infos = $map::<Runtime>::storage_info();
      let name = String::from_utf8(infos[0].storage_name.clone()).unwrap();
      let mut input = actor_suffix_owner(
        &name,
        32,
        &actors,
        |actor| $map::<Runtime>::hashed_key_for(actor),
        &infos,
        &stems,
        &actor_costs,
      );
      input["queries"] = json!(2);
      input["deletions"] = json!(2);
      input["readInputBytes"] = input["conditionalPrefixInputs"][1]["readInputBytes"].clone();
      input["readAndNetDeleteInputBytes"] =
        input["conditionalPrefixInputs"][1]["readAndAllSelectedDeletedInputBytes"].clone();
      input
        .as_object_mut()
        .unwrap()
        .remove("conditionalPrefixInputs");
      owners.push(input);
    }};
  }
  actor!(ActorSemanticStates);
  actor!(ActorIdentities);
  actor!(ActorContractHeads);
  actor!(ActorProcesses);
  actor!(ActorRunStateStore);
  actor!(ActorStateHolds);
  actor!(ActorControlLocators);
  actor!(ActorUnsignaledControlCells);
  actor!(DeadlineHandles);
  actor!(TriggerDeadlineHandles);
  actor!(PendingCheckOwners);
  actor!(DependencyPlans);
  actor!(ParkedBalanceEpisodes);
  actor!(DependencyTimedReviews);
  actor!(PendingDependencyEvents);
  actor!(PendingDependencyReviews);
  actor!(ServiceNodes);
  macro_rules! finite {
    ($map:ident, $key:expr, $prefix:expr, $queries:expr, $deletes:expr, $cost:expr, $extra:expr) => {{
      let key = $key;
      owners.push(row(
        key.clone(),
        $map::<Runtime>::storage_info(),
        &stems,
        Geometry {
          prefix_bytes: $prefix,
          max_key_bytes: key.len(),
          queries: $queries,
          deletions: $deletes,
          finite_branch_bytes: Some($cost),
          added_terminal_bytes: $extra,
        },
      ));
    }};
  }
  let mut page_images = Vec::new();
  let mut registration_image = Vec::new();
  let mut page_image = Vec::new();
  let mut extra_pages = Vec::new();
  for (source, pages) in pages
    .iter() // deos-bypass: bounded-iter -- four hash-bound source page images
    .enumerate()
  {
    let source = source as u64;
    for actor in 0..ACTOR_ID_END {
      let key = DependencyRegistrations::<Runtime>::hashed_key_for(source, actor);
      let position = DependencyRegistrationPositions::<Runtime>::hashed_key_for(source, actor);
      assert_eq!(&key[32..], &position[32..]);
      registration_image.push(<[u8; 48]>::try_from(&key[32..]).unwrap());
    }
    let image = pages
      .iter() // deos-bypass: bounded-iter -- 9968 pages in one exact hash-bound source
      .map(|page| {
        let key = DependencyRegistrationPages::<Runtime>::hashed_key_for(source, *page);
        page_image.push(<[u8; 48]>::try_from(&key[32..]).unwrap());
        <[u8; 24]>::try_from(&key[56..]).unwrap()
      })
      .collect::<Vec<_>>();
    // Adding an equal-width terminal either adds a reference (<=33) or splits one
    // compressed edge into one two-child branch (<=7+2*33=73). Old references are
    // already charged at 33. Each selected new path maps to an old terminal path;
    // q representatives bound the old contribution by C(q), including absent reads.
    // The permanent corpus remains present; at most two temporary page IDs are added.
    for page in [u64::MAX - 1, u64::MAX] {
      let key = DependencyRegistrationPages::<Runtime>::hashed_key_for(source, page);
      extra_pages.push(<[u8; 48]>::try_from(&key[32..]).unwrap());
    }
    page_images.push(json!({"source": source, "permanentPages": PAGES,
      "scaleSuffixImageSha256": digest(&image.encode())}));
  }
  // Sources 0..3 remain the complete recipe domain. Joining their images accounts
  // for namespace and source ancestors once instead of charging an arbitrary
  // 48-node source path separately for each source. No new source is allocated.
  let registration_costs = curve(registration_image.clone());
  finite!(
    DependencyRegistrations,
    DependencyRegistrations::<Runtime>::hashed_key_for(0u64, 0u64),
    32,
    8,
    4,
    registration_costs[8],
    0
  );
  finite!(
    DependencyRegistrationPositions,
    DependencyRegistrationPositions::<Runtime>::hashed_key_for(0u64, 0u64),
    32,
    8,
    4,
    registration_costs[8],
    0
  );
  let page_costs = curve(page_image.clone());
  let mut augmented = page_image.clone();
  augmented.extend(extra_pages);
  let added_costs = curve(augmented);
  assert!(
    added_costs
      .iter() // deos-bypass: bounded-iter -- 64 branch-union cells
      .zip(&page_costs)
      .all(|(new, old)| *new <= *old + 73 * 8)
  );
  finite!(
    DependencyRegistrationPages,
    DependencyRegistrationPages::<Runtime>::hashed_key_for(0u64, 0u64),
    32,
    24,
    4,
    page_costs[24],
    73 * 8
  );
  let mut source_image = (0u64..WATCHES as u64)
    .map(|source| {
      let key = DependencyRevisions::<Runtime>::hashed_key_for(source);
      <[u8; 24]>::try_from(&key[32..]).unwrap()
    })
    .collect::<Vec<_>>();
  source_image.sort_unstable();
  let source_costs = branch_union(&source_image, 0);
  assert_eq!(source_costs.len(), WATCHES + 1);
  finite!(
    DependencyRegistrationHeaders,
    DependencyRegistrationHeaders::<Runtime>::hashed_key_for(0u64),
    32,
    4,
    0,
    source_costs[4],
    0
  );
  finite!(
    DependencyRevisions,
    DependencyRevisions::<Runtime>::hashed_key_for(0u64),
    32,
    4,
    0,
    source_costs[4],
    0
  );
  finite!(
    DependencySourceBalances,
    DependencySourceBalances::<Runtime>::hashed_key_for(0u64),
    32,
    4,
    0,
    source_costs[4],
    0
  );
  macro_rules! generic {
    ($map:ident, $key:expr, $prefix:expr, $max_key:expr, $queries:expr, $deletes:expr) => {
      owners.push(row(
        $key,
        $map::<Runtime>::storage_info(),
        &stems,
        Geometry {
          prefix_bytes: $prefix,
          max_key_bytes: $max_key,
          queries: $queries,
          deletions: $deletes,
          finite_branch_bytes: None,
          added_terminal_bytes: 0,
        },
      ));
    };
  }
  generic!(
    BalanceDependencySources,
    BalanceDependencySources::<Runtime>::hashed_key_for(primitives::AssetKind::Local(0)),
    32,
    53,
    4,
    0
  );
  let block = WakeupKey::Block(0u32);
  generic!(
    DeadlineHeaders,
    DeadlineHeaders::<Runtime>::hashed_key_for(block),
    32,
    57,
    2,
    1
  );
  generic!(
    ActorWaitingOccupancies,
    ActorWaitingOccupancies::<Runtime>::hashed_key_for(block),
    32,
    57,
    2,
    0
  );
  generic!(
    ActorWaitingCursorIndices,
    ActorWaitingCursorIndices::<Runtime>::hashed_key_for(block),
    32,
    57,
    2,
    0
  );
  generic!(
    DeadlineIndexPositions,
    DeadlineIndexPositions::<Runtime>::hashed_key_for(block),
    32,
    57,
    31,
    1
  );
  for (block, deletions) in [(0u32, 1), (1, 0)] {
    generic!(
      DeadlinePages,
      DeadlinePages::<Runtime>::hashed_key_for(WakeupKey::Block(block), 0u64),
      53,
      81,
      1,
      deletions
    );
  }
  let heap_pages = <Runtime as Config>::MaxActiveActors::get().div_ceil(32);
  let heap_image = (0..u64::from(heap_pages))
    .map(|page| {
      let key = DeadlineIndexPages::<Runtime>::hashed_key_for(WakeupClock::Block, page);
      <[u8; 24]>::try_from(&key[49..]).unwrap()
    })
    .collect::<Vec<_>>();
  let heap_costs = curve(heap_image.clone());
  finite!(
    DeadlineIndexPages,
    DeadlineIndexPages::<Runtime>::hashed_key_for(WakeupClock::Block, 0u64),
    49,
    33,
    1,
    heap_costs[33],
    0
  );
  generic!(
    DeadlineIndexLen,
    DeadlineIndexLen::<Runtime>::hashed_key_for(WakeupClock::Block),
    49,
    49,
    1,
    0
  );
  generic!(
    GlobalCircuitBreaker,
    GlobalCircuitBreaker::<Runtime>::hashed_key().to_vec(),
    32,
    32,
    1,
    0
  );
  generic!(
    ActiveActorCount,
    ActiveActorCount::<Runtime>::hashed_key().to_vec(),
    32,
    32,
    1,
    0
  );
  generic!(
    ActiveActorLimit,
    ActiveActorLimit::<Runtime>::hashed_key().to_vec(),
    32,
    32,
    1,
    0
  );
  assert_eq!(owners.len(), 35);
  let sum = |field: &str| {
    owners
      .iter() // deos-bypass: bounded-iter -- 35 source-selected typed prefix roles
      .map(|owner| owner[field].as_u64().unwrap())
      .sum::<u64>()
  };
  let envelope = crate::signed_envelope::input(&stems);
  println!(
    "{}",
    json!({"scope": "conditional_signed_pair_actors_storage_input",
    "inputSha256": INPUT_SHA256,
    "actorsLibSha256": digest(include_bytes!("../../../pallets/actors/src/lib.rs")),
    "cargoLockSha256": digest(include_bytes!("../../../Cargo.lock")),
    "actorIdEndExclusive": ACTOR_ID_END, "sourceIds": [0, 1, 2, 3],
    "namespaceStemsSha256": digest(&stems.encode()), "typedPrefixRoles": owners,
    "namespaceStems": stems
      .iter() // deos-bypass: bounded-iter -- at most 512 guarded runtime namespace stems
      .map(|stem| stem
        .iter() // deos-bypass: bounded-iter -- at most 32 bytes per guarded namespace stem
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>())
      .collect::<Vec<_>>(),
    "sourcePageImages": page_images, "sourceIdBranchUnionBytes": source_costs,
    "joinedRegistrationImageSha256": digest(&registration_image.encode()),
    "joinedRegistrationBranchUnionBytes": registration_costs,
    "joinedPageImageSha256": digest(&page_image.encode()),
    "joinedPageBranchUnionBytes": page_costs,
    "eightAddedPageBranchUnionBytes": added_costs,
    "heapPageCount": heap_pages, "heapPageImageSha256": digest(&heap_image.encode()),
    "heapPageBranchUnionBytes": heap_costs,
    "actorsReadInputExcludingHoleTablesBytes": sum("readInputBytes"),
    "actorsReadAndNetDeleteInputExcludingHoleTablesBytes": sum("readAndNetDeleteInputBytes"),
    "currencyAndFrameInputs": envelope,
    "holeTables": {"scope": "separately_qualified_fixed_corpus_input",
      "forwardQueriesPerSource": 1, "reverseQueriesPerSource": 32,
      "prestateHoleDeletions": 0},
    "excluded": ["host commit/rollback context", "complete physical signed call", "host CPU/physical Weight",
      "initial chronology/repetition", "tariff", "production binding"]})
  );
}
