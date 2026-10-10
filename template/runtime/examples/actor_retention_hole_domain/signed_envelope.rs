//! Currency/FRAME inputs for the fixed signed pair after Executive initialization.
//! Overlay authority, fresh balance logic and funded accounts are recipe premises.
use codec::{Encode, MaxEncodedLen};
use deos_runtime::{AccountId, Balance, Runtime};
use polkadot_sdk::{
  frame_support::traits::{StorageInfo, StorageInfoTrait},
  sp_core::storage::well_known_keys,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub fn session_successor_interval(stems: &BTreeSet<Vec<u8>>) -> Value {
  use codec::Decode;
  use deos_runtime::Block;
  use polkadot_sdk::{
    frame_support::__private::{
      metadata::{
        RuntimeMetadata, RuntimeMetadataPrefixed,
        v16::{StorageEntryType, StorageHasher},
      },
      scale_info::{PortableRegistry, TypeDef, TypeDefPrimitive},
    },
    sp_genesis_builder::runtime_decl_for_genesis_builder::GenesisBuilder,
    sp_state_machine::Backend as _,
  };
  fn width(registry: &PortableRegistry, id: u32, ancestors: &BTreeSet<u32>) -> Option<usize> {
    if ancestors.contains(&id) {
      return None;
    }
    let mut ancestors = ancestors.clone();
    ancestors.insert(id);
    let child = |id| width(registry, id, &ancestors);
    match &registry.resolve(id)?.type_def {
      TypeDef::Primitive(primitive) => Some(match primitive {
        TypeDefPrimitive::Bool | TypeDefPrimitive::U8 | TypeDefPrimitive::I8 => 1,
        TypeDefPrimitive::U16 | TypeDefPrimitive::I16 => 2,
        TypeDefPrimitive::Char | TypeDefPrimitive::U32 | TypeDefPrimitive::I32 => 4,
        TypeDefPrimitive::U64 | TypeDefPrimitive::I64 => 8,
        TypeDefPrimitive::U128 | TypeDefPrimitive::I128 => 16,
        TypeDefPrimitive::U256 | TypeDefPrimitive::I256 => 32,
        TypeDefPrimitive::Str => return None,
      }),
      TypeDef::Composite(composite) => composite
        .fields
        .iter() // deos-bypass: bounded-iter -- fixed native metadata; recursive type IDs are rejected
        .try_fold(0usize, |n, field| n.checked_add(child(field.ty.id)?)),
      TypeDef::Tuple(tuple) => tuple
        .fields
        .iter() // deos-bypass: bounded-iter -- fixed native metadata; recursive type IDs are rejected
        .try_fold(0usize, |n, field| n.checked_add(child(field.id)?)),
      TypeDef::Variant(variants) => {
        let mut maximum = 0;
        for variant in &variants.variants {
          let fields = variant
            .fields
            .iter() // deos-bypass: bounded-iter -- fixed native metadata; recursive type IDs are rejected
            .try_fold(0usize, |n, field| n.checked_add(child(field.ty.id)?))?;
          maximum = maximum.max(fields);
        }
        maximum.checked_add(1)
      }
      TypeDef::Array(array) => child(array.type_param.id)?.checked_mul(array.len as usize),
      TypeDef::Compact(compact) => match &registry.resolve(compact.type_param.id)?.type_def {
        TypeDef::Primitive(TypeDefPrimitive::U8) => Some(2),
        TypeDef::Primitive(TypeDefPrimitive::U16) => Some(4),
        TypeDef::Primitive(TypeDefPrimitive::U32) => Some(5),
        TypeDef::Primitive(TypeDefPrimitive::U64) => Some(9),
        TypeDef::Primitive(TypeDefPrimitive::U128) => Some(17),
        TypeDef::Primitive(TypeDefPrimitive::U256) => Some(33),
        _ => None,
      },
      TypeDef::Sequence(_) | TypeDef::BitSequence(_) => None,
    }
  }
  fn merge_patch(target: &mut Value, patch: Value) {
    match (target, patch) {
      (Value::Object(target), Value::Object(patch)) => {
        for (key, value) in patch {
          merge_patch(target.entry(key).or_insert(Value::Null), value);
        }
      }
      (target, patch) => *target = patch,
    }
  }
  let cursor =
    polkadot_sdk::frame_support::storage::storage_prefix(b"Staking", b"NativeSecurityRewardPots")
      .to_vec();
  let guard = polkadot_sdk::pallet_assets::Asset::<Runtime>::hashed_key_for(0u32);
  assert_eq!((cursor.len(), guard.len()), (32, 52));
  assert!(cursor < guard);
  let encoded = Runtime::metadata_at_version(16).unwrap();
  let mut input = &encoded[..];
  let RuntimeMetadataPrefixed(_, RuntimeMetadata::V16(metadata)) =
    RuntimeMetadataPrefixed::decode(&mut input).unwrap()
  else {
    panic!("native metadata must be v16")
  };
  assert!(input.is_empty() && metadata.pallets.len() <= 64);
  let mut interval = Vec::new();
  for pallet in &metadata.pallets {
    let version = polkadot_sdk::frame_support::storage::storage_prefix(
      pallet.name.as_bytes(),
      polkadot_sdk::frame_support::traits::STORAGE_VERSION_STORAGE_KEY_POSTFIX,
    )
    .to_vec();
    if cursor <= version && version <= guard {
      interval.push(json!({"owner": format!("{}::StorageVersion", pallet.name),
        "prefixHex": hex(&version), "maxKeyBytes": 32}));
    }
    let Some(storage) = &pallet.storage else {
      continue;
    };
    assert!(storage.entries.len() <= 256);
    for entry in &storage.entries {
      let prefix = polkadot_sdk::frame_support::storage::storage_prefix(
        storage.prefix.as_bytes(),
        entry.name.as_bytes(),
      )
      .to_vec();
      if prefix < cursor || prefix > guard {
        continue;
      }
      let mut key_bytes = 32;
      if let StorageEntryType::Map { hashers, key, .. } = &entry.ty {
        let fields = if hashers.len() == 1 {
          vec![key.id]
        } else {
          let TypeDef::Tuple(tuple) = &metadata.types.resolve(key.id).unwrap().type_def else {
            panic!("map key must expose one tuple field per hasher")
          };
          tuple.fields.iter().map(|field| field.id).collect() // deos-bypass: bounded-iter -- fixed native map-key tuple
        };
        assert_eq!(fields.len(), hashers.len());
        // Fixed native map-key tuple, with matching hasher/field cardinalities checked above.
        for (hasher, id) in hashers
          .iter() /* deos-bypass: bounded-iter */
          .zip(fields)
        {
          key_bytes += match hasher {
            StorageHasher::Blake2_128 | StorageHasher::Twox128 => 16,
            StorageHasher::Blake2_256 | StorageHasher::Twox256 => 32,
            StorageHasher::Blake2_128Concat
            | StorageHasher::Twox64Concat
            | StorageHasher::Identity => {
              let payload = width(&metadata.types, id, &BTreeSet::new())
                .expect("successor interval needs a finite encoded key width");
              payload
                + match hasher {
                  StorageHasher::Blake2_128Concat => 16,
                  StorageHasher::Twox64Concat => 8,
                  StorageHasher::Identity => 0,
                  _ => unreachable!(),
                }
            }
          };
        }
      }
      interval.push(json!({"owner": format!("{}::{}", pallet.name, entry.name),
        "prefixHex": hex(&prefix), "maxKeyBytes": key_bytes}));
    }
  }
  interval.sort_by_key(|row| row["prefixHex"].as_str().unwrap().to_owned());
  let covered = interval
    .iter() // deos-bypass: bounded-iter -- at most 512 guarded namespace stems
    .map(|row| row["prefixHex"].as_str().unwrap().to_owned())
    .collect::<BTreeSet<_>>();
  let expected = stems
    .iter() // deos-bypass: bounded-iter -- at most 512 guarded namespace stems
    .filter(|key| cursor <= **key && **key <= guard)
    .map(|key| hex(key))
    .collect::<BTreeSet<_>>();
  assert_eq!(covered, expected);
  let maximum_key = interval
    .iter() // deos-bypass: bounded-iter -- at most 512 guarded interval entries
    .map(|row| row["maxKeyBytes"].as_u64().unwrap() as usize)
    .max()
    .unwrap();
  assert!(maximum_key <= 712);
  let session = session_input(stems);
  let tombstones = session["preIteratorDeletionSuperset"].as_array().unwrap();
  // Twelve declared pre-iterator tombstones.
  assert!(
    tombstones
      .iter() /* deos-bypass: bounded-iter */
      .all(|row| {
        let key = row["key"].as_str().unwrap();
        key <= hex(&cursor).as_str() || key > hex(&guard).as_str()
      })
  );
  let mut genesis = Vec::new();
  let presets = <Runtime as GenesisBuilder<Block>>::preset_names();
  assert_eq!(presets.len(), 2);
  for preset in presets {
    let default = <Runtime as GenesisBuilder<Block>>::get_preset(&None).unwrap();
    let patch = <Runtime as GenesisBuilder<Block>>::get_preset(&Some(preset.clone())).unwrap();
    let mut config = serde_json::from_slice(&default).unwrap();
    merge_patch(&mut config, serde_json::from_slice(&patch).unwrap());
    let mut ext = polkadot_sdk::sp_io::TestExternalities::new_empty();
    ext.execute_with(|| {
      <Runtime as GenesisBuilder<Block>>::build_state(serde_json::to_vec(&config).unwrap()).unwrap()
    });
    ext.commit_all().unwrap();
    let backend_successor = ext.as_backend().next_storage_key(&cursor).unwrap().unwrap();
    assert!(cursor < backend_successor && backend_successor <= guard);
    let next = ext.execute_with(|| {
      assert!(polkadot_sdk::pallet_assets::Asset::<Runtime>::contains_key(
        0u32
      ));
      assert!(
        pallet_staking::NativeSecurityRewardPots::<Runtime>::iter()
          .next()
          .is_none()
      );
      for row in tombstones {
        let key = decode_hex(row["key"].as_str().unwrap());
        polkadot_sdk::sp_io::storage::clear(&key);
      }
      let next = polkadot_sdk::sp_io::storage::next_key(&cursor).unwrap();
      assert!(cursor < next && next <= guard);
      assert!(polkadot_sdk::pallet_assets::Asset::<Runtime>::contains_key(
        0u32
      ));
      next
    });
    genesis.push(json!({"preset": preset, "assetZeroPresent": true,
      "backendSuccessor": hex(&backend_successor), "successorAfterTombstones": hex(&next),
      "guardPreserved": true}));
  }
  let cursor_bytes = 33;
  let path_nodes = 2 * cursor_bytes + 1 + 2 * maximum_key + 1;
  json!({"scope": "conditional_live_asset_successor_interval", "cursor": hex(&cursor),
    "guard": hex(&guard), "guardKeyBytes": guard.len(), "interval": interval,
    "maximumSuccessorKeyBytes": maximum_key, "maximumOverlaySkips": 0,
    "maximumBackendSearches": 1, "cursorBytes": cursor_bytes,
    "maximumPathNodesPerSearch": path_nodes, "encodedNodeCeilingBytes": 1284,
    "readInputBytes": path_nodes * 1284 + 5, "externalValueBlobAllowanceBytes": 0,
    "genesisWitnesses": genesis, "preIteratorDeletionSuperset": tombstones,
    "premises": ["Asset(0) stays present in the initial backend and is never deleted by pre-Session work",
      "Ordinary coherent namespace inventory and original empty-security closed-recipe conditions",
      "No extra ingress, asset destruction, Root/raw mutation, upgrade or child-storage producer"],
    "excluded": ["full public chronology or repeated history", "raw proof/physical Weight/CPU/PoV maximum", "complete Close/tariff/binding"]})
}

fn hex(key: &[u8]) -> String {
  key.iter().map(|byte| format!("{byte:02x}")).collect() // deos-bypass: bounded-iter -- current typed key/namespace width guards
}

fn decode_hex(key: &str) -> Vec<u8> {
  assert!(key.len().is_multiple_of(2) && key.len() <= 64);
  (0..key.len())
    .step_by(2)
    .map(|i| u8::from_str_radix(&key[i..i + 2], 16).unwrap())
    .collect() // deos-bypass: bounded-iter -- at most 32-byte pre-iterator tombstone keys
}

enum Payload {
  Typed,
  Overlay,
  Credit,
  EmptyPrefix,
  FixedVersion,
  ClosedValue { bytes: usize, model: &'static str },
}

struct Role {
  key_bytes: usize,
  queries: usize,
  deletions: usize,
  payload: Payload,
}

fn ancestors(key: &[u8], stems: &BTreeSet<Vec<u8>>) -> usize {
  assert!(key.len() <= 32 && stems.len() <= 512);
  let depths = stems
    .iter() // deos-bypass: bounded-iter -- at most 512 guarded runtime namespace stems
    .filter_map(|stem| {
      let matching = stem
        .iter() // deos-bypass: bounded-iter -- at most 32 bytes per guarded namespace stem
        .flat_map(|byte| [byte >> 4, byte & 15])
        .zip(key.iter().flat_map(|byte| [byte >> 4, byte & 15])) // deos-bypass: bounded-iter -- at most 32 bytes in this namespace/raw key
        .take_while(|(a, b)| a == b)
        .count();
      (matching < 64).then_some(matching)
    })
    .collect::<BTreeSet<_>>();
  // Short raw keys conservatively include their own terminal depth, as in the package guard.
  depths.len() + 1
}

fn row(info: &StorageInfo, stems: &BTreeSet<Vec<u8>>, role: Role) -> Value {
  let Role {
    key_bytes,
    queries,
    deletions,
    payload,
  } = role;
  // A queried ordinary node has <= key_bytes partial bytes, 528 reference bytes,
  // and <=7 header/bitmap/framing bytes. The existing 650 guard covers keys <=115;
  // a mismatching longer foreign node retains the separate 1284 allowance.
  assert!((32..=115).contains(&key_bytes));
  assert!(queries <= 63 && deletions <= queries);
  let (value_bytes, model) = match payload {
    Payload::Typed => {
      let maximum = info.max_size.unwrap();
      assert!(maximum < u32::MAX);
      (
        maximum as usize - (key_bytes - 32),
        "typed_max_encoded_value",
      )
    }
    Payload::Overlay => {
      assert!(info.max_size.is_none());
      (0, "initialized_overlay_no_old_external_blob")
    }
    Payload::Credit => {
      // The configured fungible credit serializes its Balance amount. OnFinalize also
      // clears a stray credit; the source-owned bound includes the amount conservatively.
      let maximum = Balance::max_encoded_len();
      if let Some(size) = info.max_size {
        assert_eq!(size as usize, maximum);
      }
      (maximum, "stored_fungible_credit_balance")
    }
    Payload::EmptyPrefix => {
      assert!(info.max_size.is_none());
      (0, "coherent_empty_namespace_absence")
    }
    Payload::FixedVersion => {
      assert!(info.max_size.is_none());
      let size = polkadot_sdk::frame_system::LastRuntimeUpgradeInfo::from(deos_runtime::VERSION)
        .encoded_size();
      assert!(size <= 64);
      (size, "fixed_no_upgrade_version_marker")
    }
    Payload::ClosedValue { bytes, model } => {
      if let Some(size) = info.max_size {
        assert!(size as usize >= key_bytes - 32 + bytes);
      }
      (bytes, model)
    }
  };
  let blob = if model == "initialized_overlay_no_old_external_blob"
    || model == "coherent_empty_namespace_absence"
    || model == "closed_recipe_absence"
    || model == "closed_overwrite_no_old_external_blob"
  {
    0
  } else {
    value_bytes + codec::Compact(value_bytes as u32).encoded_size()
  };
  let global_nodes = ancestors(&info.prefix, stems);
  let suffix_nodes = 2 * queries * (key_bytes - 32);
  let read = (global_nodes + suffix_nodes) * 650 + queries * (1284 + blob) + 5;
  json!({"owner": format!("{}::{}", String::from_utf8(info.pallet_name.clone()).unwrap(),
      String::from_utf8(info.storage_name.clone()).unwrap()),
    "keyBytes": key_bytes, "queries": queries, "deletions": deletions,
    "payloadModel": model, "valueAllowanceBytes": value_bytes, "valueBlobAllowanceBytes": blob,
    "namespacePrefix": info.prefix.iter().map(|byte| format!("{byte:02x}")).collect::<String>(), // deos-bypass: bounded-iter -- fixed 32-byte namespace prefix
    "globalAncestorNodes": global_nodes, "querySuffixNodes": suffix_nodes,
    "readInputBytes": read, "readAndNetDeleteInputBytes": read + deletions * 1284})
}

pub fn economic_effect_input(stems: &BTreeSet<Vec<u8>>) -> Value {
  use pallet_deos_actors::{ActorIdentityOf, DormantActorSemanticRecordOf};
  use polkadot_sdk::sp_trie::{LayoutV1, TrieLayout};
  use primitives::assets::AssetKind;
  type Layout = LayoutV1<polkadot_sdk::sp_core::Blake2Hasher>;
  assert!(!Layout::USE_EXTENSION);
  assert_eq!(Layout::MAX_INLINE_VALUE, Some(33));
  let account = AccountId::from([0u8; 32]);
  assert_eq!(
    pallet_deos_actors::SovereignIndex::<Runtime>::hashed_key_for(&account).len(),
    80
  );
  assert_eq!(
    pallet_deos_actors::BalanceDependencySources::<Runtime>::hashed_key_for(AssetKind::Local(0))
      .len(),
    53
  );
  assert_eq!(
    pallet_deos_actors::BalanceDependencySources::<Runtime>::hashed_key_for(AssetKind::Native)
      .len(),
    49
  );
  let dormant_value = 1 + DormantActorSemanticRecordOf::<Runtime>::max_encoded_len();
  assert_eq!(ActorIdentityOf::<Runtime>::max_encoded_len(), 86);
  assert_eq!(dormant_value, 1 + 86 + 8);
  let mut infos = <deos_runtime::System>::storage_info();
  infos.extend(<deos_runtime::Balances>::storage_info());
  infos.extend(<deos_runtime::Assets>::storage_info());
  infos.extend(<deos_runtime::Actors>::storage_info());
  assert!(infos.len() <= 256);
  let mut rows = Vec::new();
  let closed = |bytes, model| Payload::ClosedValue { bytes, model };
  macro_rules! owner {
    ($pallet:literal, $name:literal, $key:expr, $queries:expr, $payload:expr) => {{
      let info = infos
        .iter() // deos-bypass: bounded-iter -- at most 256 guarded four-pallet entries
        .find(|info| {
          info.pallet_name == $pallet.as_bytes() && info.storage_name == $name.as_bytes()
        })
        .unwrap();
      rows.push(row(
        info,
        stems,
        Role {
          key_bytes: $key,
          queries: $queries,
          deletions: 0,
          payload: $payload,
        },
      ));
    }};
  }
  // Native fee payers are the four producer sovereigns. Fee Sink and its two
  // destinations complete the seven-account set; existing LP credits need no System mutation.
  owner!("System", "Account", 80, 7, Payload::Typed);
  owner!("Balances", "TotalIssuance", 32, 1, Payload::Typed);
  owner!("Assets", "Asset", 52, 5, Payload::Typed);
  owner!("Assets", "Account", 100, 9, Payload::Typed);
  owner!("Actors", "SovereignIndex", 80, 3, Payload::Typed);
  owner!(
    "Actors",
    "ActorSemanticStates",
    56,
    1,
    closed(dormant_value, "closed_dormant_semantic_value")
  );
  owner!("Actors", "ActorIdentities", 56, 1, Payload::Typed);
  // Dormant ingress authenticates the semantic/identity pair and rejects legacy
  // authority. These absent reads are real work even though no trigger can latch.
  owner!(
    "Actors",
    "ActorProcesses",
    56,
    1,
    closed(0, "closed_recipe_absence")
  );
  owner!(
    "Actors",
    "ActorControlLocators",
    56,
    1,
    closed(0, "closed_recipe_absence")
  );
  owner!(
    "Actors",
    "ActorContractHead",
    56,
    1,
    closed(0, "closed_recipe_absence")
  );
  owner!(
    "Actors",
    "ActorRunState",
    56,
    1,
    closed(0, "closed_recipe_absence")
  );
  owner!(
    "Actors",
    "ActorUnsignaledControlCells",
    56,
    1,
    closed(0, "closed_recipe_absence")
  );
  owner!("Actors", "BalanceDependencySources", 53, 5, Payload::Typed);
  owner!("Actors", "DependencySourceBalances", 56, 4, Payload::Typed);
  owner!("Actors", "DependencyRevisions", 56, 4, Payload::Typed);
  owner!("Actors", "DependencyScanSourceNodes", 56, 4, Payload::Typed);
  owner!("Actors", "GlobalCircuitBreaker", 32, 1, Payload::Typed);
  owner!("System", "Events", 32, 1, Payload::Overlay);
  owner!("System", "EventCount", 32, 1, Payload::Typed);
  owner!("System", "ExecutionPhase", 32, 1, Payload::Typed);
  owner!("System", "Number", 32, 1, Payload::Typed);
  let raw = polkadot_sdk::frame_support::storage::transactional::TRANSACTION_LEVEL_KEY;
  let global_nodes = ancestors(raw, stems);
  let raw_read = global_nodes * 650 + 1284 + 5 + 5;
  rows.push(
    json!({"owner": "Raw::TransactionLevel", "keyBytes": raw.len(),
      "queries": 1, "deletions": 1, "payloadModel": "raw_u32", "valueAllowanceBytes": 4,
      "valueBlobAllowanceBytes": 5,
      "namespacePrefix": raw.iter().map(|byte| format!("{byte:02x}")).collect::<String>(), // deos-bypass: bounded-iter -- fixed nineteen-byte transaction-level key
      "globalAncestorNodes": global_nodes, "querySuffixNodes": 0,
      "readInputBytes": raw_read, "readAndNetDeleteInputBytes": raw_read + 1284}),
  );
  assert_eq!(rows.len(), 22);
  let read = rows
    .iter() // deos-bypass: bounded-iter -- exactly twenty-two scoped prefix roles
    .map(|r| r["readInputBytes"].as_u64().unwrap())
    .sum::<u64>();
  let net_delete = rows
    .iter() // deos-bypass: bounded-iter -- exactly twenty-two scoped prefix roles
    .map(|r| r["readAndNetDeleteInputBytes"].as_u64().unwrap())
    .sum::<u64>();
  json!({"scope": "conditional_post_bootstrap_economic_effect_input",
    "typedAndRawPrefixRoles": rows, "readInputBytes": read,
    "readAndNetDeleteInputBytes": net_delete,
    "queryNodeBytes": 650, "foreignOrCollapseNodeBytes": 1284,
    "participants": {"lpProducers": 4, "nativeAccounts": 7, "assetDefinitions": 5,
      "assetAccounts": 9, "ingressRecipients": 3, "dormantRecipientActors": 1,
      "activeScanningSources": 4},
    "premises": ["Four one-Step fixed-one LP Transfer producers, existing owner LP accounts and Native producer funding/floors",
      "TrustedSet Fee Sink two Native Transfer Steps with its authored ED guard; no security-reward leg",
      "Dormant Native-liquidity recipient retains canonical identity/semantic pair and no process/legacy/Contract/run authority",
      "Local(0) exists, is sufficient with minimum one and has no supply/recipient overflow; direct bridge mint has no Local ingress/publication",
      "Fresh Native accounts, preserved providers/floors, no dust/reaping/reserves/holds/freezes or extra calls",
      "Four coherent LP sources already retain active scan_target and scan-source nodes; revisions do not exhaust; Native source absent",
      "LP Holder is unit and Anchor freezer is pure for these non-anchor accounts; LP source balances remain above minimum",
      "Real initialized Events overlay and empty event topics, coherent qualified namespaces and ordinary keys"],
    "excluded": ["Actor service/trigger admission, hold reconciliation, Deadline and scan/review bodies",
      "Initial source bootstrap, complete host transaction/commit/rollback and CPU",
      "Physical Weight, whole-runtime repetition, complete Close/tariff and production binding"]})
}

pub fn session_input(stems: &BTreeSet<Vec<u8>>) -> Value {
  use polkadot_sdk::{
    frame_support::traits::Get,
    sp_trie::{LayoutV1, TrieLayout},
  };
  type Layout = LayoutV1<polkadot_sdk::sp_core::Blake2Hasher>;
  assert!(!Layout::USE_EXTENSION);
  assert_eq!(Layout::MAX_INLINE_VALUE, Some(33));
  let maximum_roster: u32 =
    <Runtime as polkadot_sdk::pallet_collator_selection::Config>::MaxInvulnerables::get();
  let roster = maximum_roster as usize;
  assert_eq!(roster, 20);
  let key_bytes = polkadot_sdk::sp_consensus_aura::sr25519::AuthorityId::max_encoded_len();
  let keys = deos_runtime::SessionKeys {
    aura: polkadot_sdk::sp_core::sr25519::Public::from_raw([0; 32]).into(),
  };
  assert_eq!(key_bytes, 32);
  assert_eq!(keys.encoded_size(), key_bytes);
  let account = AccountId::from([0u8; 32]);
  assert_eq!(
    polkadot_sdk::pallet_session::NextKeys::<Runtime>::hashed_key_for(&account).len(),
    72
  );
  assert_eq!(
    pallet_staking::NativeSecurityRewardPots::<Runtime>::hashed_key_for(0u32).len(),
    52
  );
  let mut infos = <deos_runtime::System>::storage_info();
  infos.extend(<deos_runtime::ParachainSystem>::storage_info());
  infos.extend(<deos_runtime::Session>::storage_info());
  infos.extend(<deos_runtime::CollatorSelection>::storage_info());
  infos.extend(<deos_runtime::Aura>::storage_info());
  infos.extend(<deos_runtime::Staking>::storage_info());
  assert!(infos.len() <= 256);
  let mut rows = Vec::new();
  let closed = |bytes, model| Payload::ClosedValue { bytes, model };
  macro_rules! owner {
    ($pallet:literal, $name:literal, $key:expr, $queries:expr, $deletes:expr, $payload:expr) => {{
      let info = infos
        .iter() // deos-bypass: bounded-iter -- at most 256 guarded six-pallet entries
        .find(|info| {
          info.pallet_name == $pallet.as_bytes() && info.storage_name == $name.as_bytes()
        })
        .unwrap();
      rows.push(row(
        info,
        stems,
        Role {
          key_bytes: $key,
          queries: $queries,
          deletions: $deletes,
          payload: $payload,
        },
      ));
    }};
  }
  let validators =
    codec::Compact(roster as u32).encoded_size() + roster * AccountId::max_encoded_len();
  let queued = codec::Compact(roster as u32).encoded_size()
    + roster * (AccountId::max_encoded_len() + key_bytes);
  owner!(
    "Session",
    "CurrentIndex",
    32,
    1,
    0,
    closed(4, "source_typed_scalar")
  );
  owner!(
    "Session",
    "QueuedChanged",
    32,
    1,
    0,
    closed(1, "source_typed_scalar")
  );
  owner!(
    "Session",
    "QueuedKeys",
    32,
    1,
    0,
    closed(queued, "closed_fixed_trusted_roster")
  );
  owner!(
    "Session",
    "Validators",
    32,
    1,
    0,
    closed(validators, "closed_fixed_trusted_roster")
  );
  owner!(
    "Session",
    "DisabledValidators",
    32,
    1,
    1,
    closed(1, "closed_empty_disabled_vector")
  );
  owner!(
    "Session",
    "NextKeys",
    72,
    roster,
    0,
    closed(32, "closed_fixed_aura_key")
  );
  owner!(
    "CollatorSelection",
    "Invulnerables",
    32,
    1,
    0,
    closed(validators, "closed_fixed_trusted_roster")
  );
  owner!(
    "Aura",
    "Authorities",
    32,
    1,
    0,
    closed(validators, "closed_fixed_trusted_roster")
  );
  owner!(
    "Staking",
    "ActiveNativeSecurityEpochSnapshot",
    32,
    1,
    0,
    closed(0, "closed_recipe_absence")
  );
  owner!(
    "Staking",
    "NativeSecurityRewardPots",
    52,
    1,
    0,
    closed(0, "closed_recipe_absence")
  );
  owner!("System", "BlockWeight", 32, 1, 0, Payload::Typed);
  owner!("System", "Events", 32, 1, 0, Payload::Overlay);
  owner!("System", "EventCount", 32, 1, 0, Payload::Typed);
  owner!("System", "ExecutionPhase", 32, 1, 0, Payload::Typed);
  owner!("System", "Number", 32, 1, 0, Payload::Typed);
  let raw = polkadot_sdk::frame_support::storage::transactional::TRANSACTION_LEVEL_KEY;
  let global_nodes = ancestors(raw, stems);
  let raw_read = global_nodes * 650 + 1284 + 5 + 5;
  rows.push(
    json!({"owner": "Raw::TransactionLevel", "keyBytes": raw.len(),
    "queries": 1, "deletions": 1, "payloadModel": "raw_u32", "valueAllowanceBytes": 4,
    "valueBlobAllowanceBytes": 5, "namespacePrefix": raw.iter().map(|byte| format!("{byte:02x}")).collect::<String>(), // deos-bypass: bounded-iter -- fixed nineteen-byte transaction-level key
    "globalAncestorNodes": global_nodes, "querySuffixNodes": 0,
    "readInputBytes": raw_read, "readAndNetDeleteInputBytes": raw_read + 1284}),
  );
  assert_eq!(rows.len(), 16);
  let reward_prefix = &infos
    .iter() // deos-bypass: bounded-iter -- at most 256 guarded six-pallet entries
    .find(|info| info.pallet_name == b"Staking" && info.storage_name == b"NativeSecurityRewardPots")
    .unwrap()
    .prefix;
  let mut tombstones = Vec::new();
  for (pallet, name) in [
    ("System", "Events"),
    ("System", "EventCount"),
    ("System", "BlockWeight"),
    ("ParachainSystem", "NewValidationCode"),
    ("ParachainSystem", "BlockWeightMode"),
    ("ParachainSystem", "ValidationData"),
    ("ParachainSystem", "ProcessedDownwardMessages"),
    ("ParachainSystem", "UpwardMessages"),
    ("ParachainSystem", "HrmpOutboundMessages"),
    ("ParachainSystem", "CustomValidationHeadData"),
    ("Session", "DisabledValidators"),
  ] {
    let info = infos
      .iter() // deos-bypass: bounded-iter -- at most 256 guarded six-pallet entries
      .find(|info| info.pallet_name == pallet.as_bytes() && info.storage_name == name.as_bytes())
      .unwrap();
    assert_eq!(info.prefix.len(), 32);
    tombstones.push(json!({"owner": format!("{pallet}::{name}"), "keyBytes": 32,
      "key": info.prefix.iter().map(|byte| format!("{byte:02x}")).collect::<String>(), // deos-bypass: bounded-iter -- fixed 32-byte singleton prefix
      "afterRewardPrefix": info.prefix > *reward_prefix}));
  }
  assert!(raw < reward_prefix.as_slice());
  tombstones.push(
    json!({"owner": "Raw::TransactionLevel", "keyBytes": raw.len(),
    "key": raw.iter().map(|byte| format!("{byte:02x}")).collect::<String>(), // deos-bypass: bounded-iter -- fixed nineteen-byte transaction-level key
    "afterRewardPrefix": false}),
  );
  let skip_bound = tombstones
    .iter() // deos-bypass: bounded-iter -- twelve-entry source-owned deletion superset
    .filter(|item| item["afterRewardPrefix"] == true)
    .count();
  assert_eq!(skip_bound, 1);
  // Seek follows one cursor path; next-key adds at most one terminal path. V1 has
  // no extension nodes. The extra cursor byte also covers the SDK debug crosscheck.
  let cursor_bytes = 33usize;
  let foreign_key_bytes = 712usize;
  let path_nodes = 2 * cursor_bytes + 1 + 2 * foreign_key_bytes + 1;
  let per_search = path_nodes * 1284 + 5;
  let searches = 1 + skip_bound;
  let successor_read = searches * per_search;
  let sum = |field: &str| {
    rows
      .iter() // deos-bypass: bounded-iter -- sixteen selected Session prefix roles
      .map(|item| item[field].as_u64().unwrap())
      .sum::<u64>()
  };
  json!({"scope": "conditional_scheduled_trusted_session_input", "typedAndRawPrefixRoles": rows,
    "prefixReadInputBytes": sum("readInputBytes"), "prefixReadAndNetDeleteInputBytes": sum("readAndNetDeleteInputBytes"),
    "preIteratorDeletionSuperset": tombstones,
    "emptyRewardSuccessorInput": {"rewardPrefix": reward_prefix.iter().map(|byte| format!("{byte:02x}")).collect::<String>(), // deos-bypass: bounded-iter -- fixed 32-byte reward-map prefix
      "maximumOverlaySkips": skip_bound, "maximumBackendSearches": searches,
      "cursorBytes": cursor_bytes, "ordinaryKeyCeilingBytes": foreign_key_bytes,
      "maximumPathNodesPerSearch": path_nodes, "encodedNodeCeilingBytes": 1284,
      "perSearchReadInputBytes": per_search, "readInputBytes": successor_read,
      "externalValueBlobAllowanceBytes": 0, "netDeletedRewardKeys": 0},
    "readInputBytes": sum("readInputBytes") + successor_read as u64,
    "readAndNetDeleteInputBytes": sum("readAndNetDeleteInputBytes") + successor_read as u64,
    "maximumRoster": roster,
    "premises": ["fixed TrustedSet invulnerables and complete Aura keys; no manual rotation, offence or roster mutation",
      "empty native security pots and active snapshot preserved from genesis",
      "scheduled hook occurs during real Executive initialization before extrinsics and later economic hooks",
      "current source-owned ordinary namespace/key inventory; no raw/Root/upgrade or child-storage producer",
      "empty EventTopics and funded fresh native author/pot accounts; no reaping callbacks",
      "production native-security mode; no runtime-benchmarks raw mode override",
      "unchanged Aura authorities; empty pre-session callbacks"],
    "excluded": ["whole hook CPU and SDK/host commit", "physical Weight or budget containment",
      "public birth/initial-state equivalence and joint horizon repetition", "complete Close/tariff/production binding"]})
}

pub fn session_successor_witness(stems: &BTreeSet<Vec<u8>>) -> Value {
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_trie::{
      LayoutV1, MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder,
      TrieDBRawIterator,
    },
  };
  type Layout = LayoutV1<Blake2Hasher>;
  let infos = <deos_runtime::Staking>::storage_info();
  assert!(infos.len() <= 128);
  let prefix = &infos
    .iter() // deos-bypass: bounded-iter -- at most 128 guarded Staking storage entries
    .find(|info| info.storage_name == b"NativeSecurityRewardPots")
    .unwrap()
    .prefix;
  let neighbor = stems
    .iter() // deos-bypass: bounded-iter -- at most 512 guarded namespace stems
    .find(|stem| stem.as_slice() > prefix.as_slice())
    .unwrap();
  let prefix_only_bound = (ancestors(prefix, stems) * 650 + 1284 + 5) as u64;
  let mut points = Vec::new();
  for spine in [0usize, 64] {
    let mut rows = Vec::new();
    for stem in stems.iter(/* deos-bypass: bounded-iter -- at most 512 guarded namespace stems. */)
    {
      if stem == prefix {
        continue;
      }
      let mut key = stem.clone();
      key.extend_from_slice(&[17; 68]);
      rows.push((key, vec![19; 200]));
    }
    let mut successor = neighbor.clone();
    successor.resize(712, 0);
    rows.push((successor.clone(), vec![29; 4096]));
    for digit in 0..spine {
      let mut key = successor.clone();
      key[32 + digit / 2] = if digit.is_multiple_of(2) { 16 } else { 1 };
      rows.push((key, vec![29; 4096]));
    }
    assert!(rows.len() <= 512);
    rows.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    assert!(rows.iter().all(|(key, _)| !key.starts_with(prefix))); // deos-bypass: bounded-iter -- at most 512 guarded synthetic rows
    let mut database = MemoryDB::<Blake2Hasher>::default();
    let root = Layout::trie_build(&mut database, rows);
    let mut observations = Vec::new();
    for global in [false, true] {
      let mut recorder = Recorder::<Layout>::new();
      {
        let trie = TrieDBBuilder::<Layout>::new(&database, &root)
          .with_recorder(&mut recorder)
          .build();
        if global {
          let mut iter = trie.key_iter().unwrap();
          iter.seek(prefix).unwrap();
          assert_eq!(iter.next().unwrap().unwrap(), successor);
          assert!(!successor.starts_with(prefix));
        } else {
          let mut iter = TrieDBRawIterator::new_prefixed(&trie, prefix).unwrap();
          assert!(iter.next_key(&trie).is_none());
        }
      }
      let records = recorder.drain();
      assert!(records.len() <= 128);
      assert!(records.iter().all(|record| record.data != vec![29; 4096])); // deos-bypass: bounded-iter -- at most 128 guarded key-path records
      let proof = StorageProof::new(records.into_iter().map(|record| record.data));
      let bytes = proof.encoded_size();
      if !global {
        assert!(bytes as u64 <= prefix_only_bound);
      }
      let replay = proof.into_memory_db::<Blake2Hasher>();
      let trie = TrieDBBuilder::<Layout>::new(&replay, &root).build();
      if global {
        let mut iter = trie.key_iter().unwrap();
        iter.seek(prefix).unwrap();
        assert_eq!(iter.next().unwrap().unwrap(), successor);
      } else {
        let mut iter = TrieDBRawIterator::new_prefixed(&trie, prefix).unwrap();
        assert!(iter.next_key(&trie).is_none());
      }
      observations.push(
        json!({"query": if global { "global_successor" } else { "prefixed_absence" },
        "recordedEncodedProofBytes": bytes, "keyProofReplay": true,
        "externalNeighborValueBlobRecorded": false}),
      );
    }
    if spine == 64 {
      assert!(
        observations[1]["recordedEncodedProofBytes"]
          .as_u64()
          .unwrap()
          > prefix_only_bound
      );
    }
    points.push(
      json!({"syntheticNeighborSpine": spine, "successorKeyBytes": successor.len(),
      "successorNamespace": neighbor.iter().map(|byte| format!("{byte:02x}")).collect::<String>(), // deos-bypass: bounded-iter -- fixed 32-byte neighbor namespace
      "observations": observations, "admittedRuntimeState": false}),
    );
  }
  json!({"scope": "synthetic_empty_session_reward_successor_geometry",
    "rewardPrefix": prefix.iter().map(|byte| format!("{byte:02x}")).collect::<String>(), // deos-bypass: bounded-iter -- fixed 32-byte reward namespace
    "prefixOnlyReadInputBytes": prefix_only_bound, "points": points,
    "result": "empty FRAME map iteration needs a distinct global-successor input; prefix-only bound is inapplicable",
    "excluded": ["admitted runtime storage values or public chronology", "complete SessionRotation input/physical Weight",
      "overlay tombstone skipping maximum", "joint horizon repeatability", "Close/tariff/production binding"]})
}

fn overwrite_namespace_witness(prefix: &[u8], stems: &BTreeSet<Vec<u8>>, bound: u64) -> Vec<Value> {
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_trie::{
      LayoutV1, MemoryDB, Recorder, StorageProof, TrieConfiguration, TrieDBMutBuilder, TrieMut,
    },
  };
  type Layout = LayoutV1<Blake2Hasher>;
  let mut points = Vec::new();
  for value_bytes in [32, 33, 4096] {
    for delete in [false, true] {
      let old_value = vec![29; value_bytes];
      let mut rows = Vec::new();
      for stem in
        stems.iter(/* deos-bypass: bounded-iter -- at most 512 guarded namespace peers. */)
      {
        if stem == prefix {
          continue;
        }
        let mut key = stem.clone();
        key.extend_from_slice(&[17; 68]);
        rows.push((key, vec![19; 200]));
      }
      rows.push((prefix.to_vec(), old_value.clone()));
      rows.sort_unstable_by(|left, right| left.0.cmp(&right.0));
      let mut database = MemoryDB::<Blake2Hasher>::default();
      let root = Layout::trie_build(&mut database, rows);
      let mut expected_database = database.clone();
      let mut expected_root = root;
      {
        let mut trie =
          TrieDBMutBuilder::<Layout>::from_existing(&mut expected_database, &mut expected_root)
            .build();
        if delete {
          trie.remove(prefix).unwrap();
        } else {
          trie.insert(prefix, &[23; 37]).unwrap();
        }
      }
      let mut recorder = Recorder::<Layout>::new();
      let mut recorded_root = root;
      {
        let mut trie = TrieDBMutBuilder::<Layout>::from_existing(&mut database, &mut recorded_root)
          .with_recorder(&mut recorder)
          .build();
        if delete {
          trie.remove(prefix).unwrap();
        } else {
          trie.insert(prefix, &[23; 37]).unwrap();
        }
      }
      assert_eq!(recorded_root, expected_root);
      let records = recorder.drain();
      assert!(records.len() <= 128);
      assert!(records.iter().all(|record| record.data != old_value)); // deos-bypass: bounded-iter -- at most 128 guarded old-path records
      let proof = StorageProof::new(records.into_iter().map(|record| record.data));
      let bytes = proof.encoded_size();
      let allowance = bound + if delete { 1284 } else { 0 };
      assert!(bytes as u64 <= allowance);
      let mut replay = proof.into_memory_db::<Blake2Hasher>();
      let mut replay_root = root;
      {
        let mut trie =
          TrieDBMutBuilder::<Layout>::from_existing(&mut replay, &mut replay_root).build();
        if delete {
          trie.remove(prefix).unwrap();
        } else {
          trie.insert(prefix, &[23; 37]).unwrap();
        }
      }
      assert_eq!(replay_root, expected_root);
      points.push(
        json!({"oldValueBytes": value_bytes, "operation": if delete { "delete" } else { "replace" },
        "recordedEncodedProofBytes": bytes, "conditionalMutationInputBytes": allowance,
        "externalOldValueBlobRecorded": false, "rootReplay": true, "admittedRuntimeState": false}),
      );
    }
  }
  points
}

fn parachain_clock_codec_witness() -> Vec<Value> {
  use codec::DecodeAll;
  use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
  use deos_runtime::{Hash, Header};
  use polkadot_sdk::{
    cumulus_pallet_parachain_system::relay_state_snapshot::RelayChainStateProof,
    cumulus_primitives_core::{PersistedValidationData, relay_chain::HeadData, rpsr_digest},
    sp_runtime::{Digest, DigestItem, traits::Header as _},
  };
  let mut points = Vec::new();
  for parent_number in [
    0u32,
    1,
    13,
    63,
    64,
    16_383,
    16_384,
    187_203,
    1_073_741_823,
    1_073_741_824,
    2_552_520_737,
  ] {
    for sealed in [false, true] {
      if parent_number == 0 && sealed {
        continue;
      }
      let number = parent_number.checked_add(1).unwrap();
      let width = codec::Compact(parent_number).encoded_size();
      let mut logs = Vec::new();
      if parent_number > 0 {
        logs.push(DigestItem::PreRuntime(
          polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
          u64::from(parent_number).encode(),
        ));
        logs.push(rpsr_digest::relay_parent_storage_root_item(
          Hash::from([27u8; 32]),
          parent_number,
        ));
      }
      if sealed {
        // A fixed-size opaque signature exercises encoding; it is never authenticated.
        let signature = polkadot_sdk::sp_core::sr25519::Signature::from_raw([23u8; 64]);
        assert_eq!(signature.encoded_size(), 64);
        let seal = DigestItem::Seal(
          polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
          signature.encode(),
        );
        assert_eq!(seal.encoded_size(), 71);
        logs.push(seal);
      }
      assert!(logs.len() <= 3);
      let header = Header::new(
        parent_number,
        Hash::from([29u8; 32]),
        Hash::from([31u8; 32]),
        Hash::from([37u8; 32]),
        Digest { logs },
      );
      let header_bytes = header.encode();
      let expected_header = if parent_number == 0 {
        96 + width + 1
      } else {
        149 + 2 * width + usize::from(sealed) * 71
      };
      assert_eq!(header_bytes.len(), expected_header);
      assert_eq!(Header::decode_all(&mut &header_bytes[..]).unwrap(), header);
      let parent_head = HeadData(header_bytes.clone());
      let builder = RelayStateSproofBuilder {
        para_id: deos_runtime::PARACHAIN_ID.into(),
        current_slot: u64::from(number).into(),
        included_para_head: Some(parent_head.clone()),
        ..Default::default()
      };
      let host_bytes = builder.host_config.encode();
      let (root, proof) = builder.into_state_root_and_proof();
      let proof_bytes = proof.encode();
      assert!(proof_bytes.len() <= 4_096);
      let reader =
        RelayChainStateProof::new(deos_runtime::PARACHAIN_ID.into(), root, proof).unwrap();
      let read_head = reader.read_included_para_head().unwrap();
      assert_eq!(read_head, parent_head);
      assert_eq!(u64::from(reader.read_slot().unwrap()), u64::from(number));
      let host = reader.read_abridged_host_configuration().unwrap();
      assert_eq!(host.encode(), host_bytes);
      assert!(reader.read_upgrade_go_ahead_signal().unwrap().is_none());
      assert!(reader.read_upgrade_restriction_signal().unwrap().is_none());
      let messaging = reader.read_messaging_state_snapshot(&host).unwrap();
      assert!(messaging.ingress_channels.is_empty() && messaging.egress_channels.is_empty());
      assert_eq!(messaging.dmq_mqc_head, Hash::default());
      assert_eq!(
        messaging
          .relay_dispatch_queue_remaining_capacity
          .remaining_count,
        8
      );
      assert_eq!(
        messaging
          .relay_dispatch_queue_remaining_capacity
          .remaining_size,
        1_024
      );
      assert_eq!(messaging.encoded_size(), 42);
      let data = PersistedValidationData {
        parent_head,
        relay_parent_number: number,
        relay_parent_storage_root: root,
        max_pov_size: deos_runtime::MAXIMUM_BLOCK_WEIGHT.proof_size() as u32,
      };
      let data_bytes = data.encode();
      assert_eq!(
        data_bytes.len(),
        codec::Compact(header_bytes.len() as u32).encoded_size() + header_bytes.len() + 40
      );
      assert_eq!(
        PersistedValidationData::decode_all(&mut &data_bytes[..]).unwrap(),
        data
      );
      points.push(json!({
        "parentNumber": parent_number, "relayNumber": number,
        "parentCompactBytes": width, "rpsrCompactBytes": if parent_number == 0 { 0 } else { width },
        "sealedHeaderShape": sealed, "auraSealBytes": if sealed { 71 } else { 0 },
        "parentHeaderBytes": header_bytes.len(), "validationDataBytes": data_bytes.len(),
        "parentHeaderHex": hex(&header_bytes), "validationDataHex": hex(&data_bytes),
        "relayProofHex": hex(&proof_bytes), "relayProofBytes": proof_bytes.len(),
        "relayStorageRootHex": hex(root.as_bytes()), "messagingBytes": messaging.encoded_size(),
        "immediateParentProofRead": true, "emptyChannels": true, "absentUpgradeSignals": true,
        "syntheticOpaqueHashesAndSignature": true, "sealVerified": false,
        "admittedRuntimeState": false, "authenticatedRelayHistory": false,
        "literalHistory": false,
      }));
    }
  }
  assert_eq!(points.len(), 21);
  points
}

pub fn parachain_input(stems: &BTreeSet<Vec<u8>>) -> Value {
  use cumulus_primitives_parachain_inherent::MessageQueueChain;
  use polkadot_sdk::cumulus_pallet_parachain_system::{
    Ancestor, MessagingStateSnapshot, PoVMessages, UsedBandwidth,
  };
  let mut ancestor = Ancestor::new_unchecked(UsedBandwidth::default(), None);
  ancestor.replace_para_head_hash(deos_runtime::Hash::default());
  let segment_bytes = vec![ancestor].encoded_size();
  // SegmentTracker encodes UsedBandwidth, optional u32 watermark and absent signal; PhantomData is empty.
  let aggregate_bytes = (UsedBandwidth::default(), Some(u32::MAX), None::<u8>).encoded_size();
  let messaging_bytes = MessagingStateSnapshot {
    dmq_mqc_head: Default::default(),
    relay_dispatch_queue_remaining_capacity: Default::default(),
    ingress_channels: Vec::new(),
    egress_channels: Vec::new(),
  }
  .encoded_size();
  let host_bytes = cumulus_test_relay_sproof_builder::RelayStateSproofBuilder::default()
    .host_config
    .encoded_size();
  let tracker_bytes = PoVMessages::default().encoded_size();
  let dmq_bytes = MessageQueueChain::default().encoded_size();
  assert_eq!(
    (
      segment_bytes,
      aggregate_bytes,
      messaging_bytes,
      host_bytes,
      tracker_bytes,
      dmq_bytes
    ),
    (44, 15, 42, 44, 43, 32)
  );
  let mut infos = <deos_runtime::ParachainSystem>::storage_info();
  infos.extend(<deos_runtime::ParachainInfo>::storage_info());
  infos.extend(<deos_runtime::System>::storage_info());
  infos.extend(<deos_runtime::Aura>::storage_info());
  infos.extend(<deos_runtime::AuraExt>::storage_info());
  infos.extend(<deos_runtime::XcmpQueue>::storage_info());
  assert!(infos.len() <= 128);
  let mut rows = Vec::new();
  macro_rules! owner {
    ($pallet:literal, $name:literal, $deletes:expr, $payload:expr) => {{
      let info = infos
        .iter() // deos-bypass: bounded-iter -- at most 128 guarded six-pallet storage entries
        .find(|info| {
          info.pallet_name == $pallet.as_bytes() && info.storage_name == $name.as_bytes()
        })
        .unwrap();
      rows.push(row(
        info,
        stems,
        Role {
          key_bytes: 32,
          queries: 1,
          deletions: $deletes,
          payload: $payload,
        },
      ));
    }};
  }
  let empty = || Payload::ClosedValue {
    bytes: 1,
    model: "coherent_empty_vector",
  };
  let absent = || Payload::ClosedValue {
    bytes: 0,
    model: "closed_recipe_absence",
  };
  let overwrite = || Payload::ClosedValue {
    bytes: 0,
    model: "closed_overwrite_no_old_external_blob",
  };
  // ParachainSystem uses without_storage_info even for fixed-width scalar owners.
  let scalar = |bytes| Payload::ClosedValue {
    bytes,
    model: "source_typed_scalar",
  };
  owner!("System", "Digest", 0, Payload::Overlay);
  owner!("System", "ParentHash", 0, Payload::Typed);
  owner!("System", "BlockWeight", 0, Payload::Typed);
  owner!("ParachainInfo", "ParachainId", 0, Payload::Typed);
  owner!("Aura", "CurrentSlot", 0, Payload::Typed);
  owner!("AuraExt", "RelaySlotInfo", 0, Payload::Typed);
  owner!(
    "ParachainSystem",
    "DidSetValidationCode",
    0,
    scalar(bool::max_encoded_len())
  );
  owner!("ParachainSystem", "NewValidationCode", 0, absent());
  owner!(
    "ParachainSystem",
    "UnincludedSegment",
    0,
    Payload::ClosedValue {
      bytes: segment_bytes,
      model: "closed_single_parent_segment"
    }
  );
  owner!("ParachainSystem", "BlockWeightMode", 1, absent());
  owner!("ParachainSystem", "ValidationData", 0, overwrite());
  owner!(
    "ParachainSystem",
    "ProcessedDownwardMessages",
    0,
    scalar(u32::max_encoded_len())
  );
  owner!("ParachainSystem", "UpwardMessages", 0, empty());
  owner!("ParachainSystem", "HrmpOutboundMessages", 0, empty());
  owner!("ParachainSystem", "CustomValidationHeadData", 0, absent());
  owner!(
    "ParachainSystem",
    "HrmpWatermark",
    0,
    scalar(u32::max_encoded_len())
  );
  owner!(
    "ParachainSystem",
    "HostConfiguration",
    0,
    Payload::ClosedValue {
      bytes: host_bytes,
      model: "closed_fixed_host_configuration"
    }
  );
  owner!(
    "ParachainSystem",
    "AnnouncedHrmpMessagesPerCandidate",
    1,
    scalar(u32::max_encoded_len())
  );
  owner!("ParachainSystem", "UpgradeGoAhead", 1, absent());
  owner!(
    "ParachainSystem",
    "LastRelayChainBlockNumber",
    0,
    scalar(u32::max_encoded_len())
  );
  owner!(
    "ParachainSystem",
    "AggregatedUnincludedSegment",
    0,
    Payload::ClosedValue {
      bytes: aggregate_bytes,
      model: "closed_empty_bandwidth_tracker"
    }
  );
  owner!("ParachainSystem", "UpgradeRestrictionSignal", 1, absent());
  owner!("ParachainSystem", "RelayStateProof", 0, overwrite());
  owner!(
    "ParachainSystem",
    "RelevantMessagingState",
    0,
    Payload::ClosedValue {
      bytes: messaging_bytes,
      model: "closed_empty_messaging_state"
    }
  );
  owner!("ParachainSystem", "PendingApprovedPeer", 1, absent());
  owner!(
    "ParachainSystem",
    "LastDmqMqcHead",
    0,
    Payload::ClosedValue {
      bytes: dmq_bytes,
      model: "closed_message_queue_hash"
    }
  );
  owner!("ParachainSystem", "LastHrmpMqcHeads", 0, empty());
  owner!(
    "ParachainSystem",
    "LastProcessedHrmpMessage",
    0,
    scalar(2 * u32::max_encoded_len())
  );
  owner!(
    "ParachainSystem",
    "PoVMessagesTracker",
    0,
    Payload::ClosedValue {
      bytes: tracker_bytes,
      model: "closed_empty_pov_message_tracker"
    }
  );
  owner!("ParachainSystem", "PendingUpwardMessages", 0, empty());
  owner!(
    "ParachainSystem",
    "PreviousCoreCount",
    0,
    Payload::ClosedValue {
      bytes: 1,
      model: "closed_single_core_compact"
    }
  );
  owner!("ParachainSystem", "PendingUpwardSignals", 1, empty());
  owner!(
    "ParachainSystem",
    "UpwardDeliveryFeeFactor",
    0,
    scalar(u128::max_encoded_len())
  );
  owner!("XcmpQueue", "OutboundXcmpStatus", 0, empty());
  // The constructor reads this additional absent message cursor; import does not need its body.
  owner!(
    "ParachainSystem",
    "LastProcessedDownwardMessage",
    0,
    scalar(2 * u32::max_encoded_len())
  );
  assert_eq!(rows.len(), 35);
  let overwrite_info = infos
    .iter() // deos-bypass: bounded-iter -- at most 128 guarded storage entries
    .find(|info| info.pallet_name == b"ParachainSystem" && info.storage_name == b"RelayStateProof")
    .unwrap();
  let overwrite_bound = rows
    .iter() // deos-bypass: bounded-iter -- thirty-five selected Parachain query roles
    .find(|item| item["owner"] == "ParachainSystem::RelayStateProof")
    .unwrap()["readInputBytes"]
    .as_u64()
    .unwrap();
  let witnesses = overwrite_namespace_witness(&overwrite_info.prefix, stems, overwrite_bound);
  let sum = |field: &str| {
    rows
      .iter() // deos-bypass: bounded-iter -- thirty-five selected Parachain query roles
      .map(|item| item[field].as_u64().unwrap())
      .sum::<u64>()
  };
  json!({"scope": "conditional_recipe_parachain_input", "typedPrefixRoles": rows,
    "readInputBytes": sum("readInputBytes"), "readAndNetDeleteInputBytes": sum("readAndNetDeleteInputBytes"),
    "closedValueBytes": {"singleParentSegment": segment_bytes, "aggregate": aggregate_bytes,
      "messaging": messaging_bytes, "hostConfiguration": host_bytes, "povTracker": tracker_bytes, "dmqHash": dmq_bytes},
    "overwriteRootWitnesses": witnesses, "clockCodecWitnesses": parachain_clock_codec_witness(),
    "premises": ["each linked constructor includes the immediate parent; no extra segment mutation",
      "empty inbound/outbound channels, messages, signals and peer; no upgrade",
      "unit OnSystemEvent, zero RelayParentOffset and no CoreInfo/BlockBundleInfo digest",
      "only Aura pre-runtime and relay-parent-root digest effects", "real initialization precedes inherent application",
      "ValidationData is killed before exists/read and RelayStateProof is overwritten before any runtime read",
      "V1 mutations retain old hashed values as hashes; no caller converts returned old values into bytes"],
    "excluded": ["arbitrary relay input or populated message/upgrade state", "full PVF validator/relay proof CPU and encoded PoV",
      "scheduled SessionRotation", "Actors body/economic input", "complete SDK/host commit/rollback and physical Weight",
      "budget containment", "public chronology/joint horizon repetition", "tariff/production binding"]})
}

fn empty_frame_declaration_envelope() -> Value {
  use polkadot_sdk::frame_support::weights::Weight;
  type ParachainWeights =
    <Runtime as polkadot_sdk::cumulus_pallet_parachain_system::Config>::WeightInfo;
  type StakingWeights = <Runtime as pallet_staking::Config>::WeightInfo;
  let budget = <Runtime as pallet_deos_actors::Config>::BlockResourceBudget::get();
  let database = <Runtime as polkadot_sdk::frame_system::Config>::DbWeight::get();
  let host = cumulus_test_relay_sproof_builder::RelayStateSproofBuilder::default().host_config;
  let hrmp = u64::from(host.hrmp_max_message_num_per_candidate);
  assert_eq!(hrmp, 5);
  let fixed = budget.fixed_envelope();
  // These are returned/registered SDK declarations, including prepaid finalization work.
  let parachain_initialize = database.reads_writes(17 + hrmp, 18 + hrmp);
  let consensus = database.reads(1);
  let included_parent = database.reads_writes(3, 2);
  let downward = <ParachainWeights as polkadot_sdk::cumulus_pallet_parachain_system::WeightInfo>::enqueue_inbound_downward_messages(0);
  let horizontal = database.reads_writes(1, 2);
  let horizon = <Runtime as pallet_staking::Config>::SecurityRewardClaimHorizon::get()
    .checked_add(2)
    .unwrap();
  let session_settlement =
    <StakingWeights as pallet_staking::WeightInfo>::settle_due_native_security_reward(horizon);
  let session_contraction =
    <StakingWeights as pallet_staking::WeightInfo>::contract_native_security_obligations();
  let encoded = Weight::from_parts(0, 16_941);
  let components = [
    ("conservative_fixed_envelope", fixed),
    ("parachain_initialize", parachain_initialize),
    ("relay_consensus_callback", consensus),
    ("included_single_parent", included_parent),
    ("empty_downward_enqueue", downward),
    ("empty_horizontal_enqueue", horizontal),
    ("trusted_session_settlement", session_settlement),
    ("trusted_session_contraction", session_contraction),
    ("sealed_inherent_length_upper", encoded),
  ];
  let frame = components
    .iter() // deos-bypass: bounded-iter -- fixed array of nine declared Weight owners
    .fold(Weight::zero(), |sum, (_, weight)| {
      sum.checked_add(weight).unwrap()
    });
  let xcmp = deos_runtime::XcmpQueue::on_idle_weight();
  let frame_and_xcmp = frame.checked_add(&xcmp).unwrap();
  let control = budget.limits().actor_control();
  let ceiling = budget.maximum_block().checked_sub(&control).unwrap();
  let margin = ceiling.checked_sub(&frame_and_xcmp).unwrap();
  let pair = |weight: Weight| [weight.ref_time(), weight.proof_size()];
  json!({
    "scope": "conditional_bare_empty_block_registered_frame_envelope",
    "components": components.map(|(owner, weight)| json!({"owner": owner, "weight": pair(weight)})),
    "hrmpMessagesPerCandidate": hrmp, "sessionRewardInspectionBound": horizon,
    "frameUpper": pair(frame), "conservativePrecedingXcmp": pair(xcmp),
    "frameAndXcmpUpper": pair(frame_and_xcmp), "maximumBlockMinusControl": pair(ceiling),
    "sufficientGateMargin": pair(margin),
    "premises": ["three bare inherents only; no signed/general dispatch or extra input",
      "fixed current runtime/configuration/Weight and the declared relay constructor",
      "empty messages, one zero-bandwidth included parent, no signals/peer/Core/Bundle/upgrade",
      "fixed TrustedSet keys and empty security obligations; successful callbacks",
      "closed Governance phase/due inputs; no migration or additional scheduled work",
      "zero-effect reconciled Prepass and no external Control debit"],
    "excluded": ["physical proof/RefTime or root/signature/host cost", "signed boundary calls",
      "admitted coherent historical parent", "useful service quanta or joint horizon", "tariffs/production binding"]
  })
}

pub fn quiet_hooks_input(stems: &BTreeSet<Vec<u8>>) -> Value {
  use polkadot_sdk::frame_support::traits::Get;
  let maximum: u32 = <Runtime as polkadot_sdk::pallet_aura::Config>::MaxAuthorities::get();
  assert_eq!(maximum, 20);
  let validators =
    codec::Compact(maximum).encoded_size() + maximum as usize * AccountId::max_encoded_len();
  let mut infos = <deos_runtime::System>::storage_info();
  infos.extend(<deos_runtime::Balances>::storage_info());
  infos.extend(<deos_runtime::TransactionPayment>::storage_info());
  infos.extend(<deos_runtime::Authorship>::storage_info());
  infos.extend(<deos_runtime::CollatorSelection>::storage_info());
  infos.extend(<deos_runtime::Session>::storage_info());
  infos.extend(<deos_runtime::Aura>::storage_info());
  infos.extend(<deos_runtime::AuraExt>::storage_info());
  infos.extend(<deos_runtime::Timestamp>::storage_info());
  infos.extend(<deos_runtime::Governance>::storage_info());
  infos.extend(<deos_runtime::MessageQueue>::storage_info());
  infos.extend(<deos_runtime::PolkadotXcm>::storage_info());
  assert!(infos.len() <= 512);
  let mut rows = Vec::new();
  macro_rules! owner {
    ($pallet:literal, $name:literal, $key:expr, $queries:expr, $deletes:expr, $payload:expr) => {{
      let info = infos
        .iter() // deos-bypass: bounded-iter -- at most 512 guarded twelve-pallet storage entries
        .find(|info| {
          info.pallet_name == $pallet.as_bytes() && info.storage_name == $name.as_bytes()
        })
        .unwrap();
      rows.push(row(
        info,
        stems,
        Role {
          key_bytes: $key,
          queries: $queries,
          deletions: $deletes,
          payload: $payload,
        },
      ));
    }};
  }
  owner!("System", "Account", 80, 2, 0, Payload::Typed);
  owner!("System", "Number", 32, 1, 0, Payload::Typed);
  owner!("System", "BlockWeight", 32, 1, 0, Payload::Typed);
  owner!("System", "Digest", 32, 1, 0, Payload::Overlay);
  owner!("System", "Events", 32, 1, 0, Payload::Overlay);
  owner!("System", "EventCount", 32, 1, 0, Payload::Typed);
  owner!("Balances", "TotalIssuance", 32, 1, 0, Payload::Typed);
  owner!("Authorship", "Author", 32, 1, 1, Payload::Typed);
  owner!(
    "CollatorSelection",
    "LastAuthoredBlock",
    72,
    1,
    0,
    Payload::Typed
  );
  owner!(
    "Session",
    "Validators",
    32,
    1,
    0,
    Payload::ClosedValue {
      bytes: validators,
      model: "closed_trusted_roster"
    }
  );
  owner!(
    "Session",
    "DisabledValidators",
    32,
    1,
    0,
    Payload::ClosedValue {
      bytes: 1,
      model: "coherent_empty_vector"
    }
  );
  owner!("Aura", "Authorities", 32, 1, 0, Payload::Typed);
  owner!("Aura", "CurrentSlot", 32, 1, 0, Payload::Typed);
  owner!("AuraExt", "Authorities", 32, 1, 0, Payload::Typed);
  owner!("Timestamp", "Now", 32, 1, 0, Payload::Typed);
  owner!("Timestamp", "DidUpdate", 32, 1, 1, Payload::Typed);
  owner!(
    "TransactionPayment",
    "NextFeeMultiplier",
    32,
    1,
    0,
    Payload::Typed
  );
  owner!(
    "TransactionPayment",
    "TxPaymentCredit",
    32,
    1,
    1,
    Payload::Credit
  );
  owner!(
    "Governance",
    "PendingEnactments",
    32,
    1,
    0,
    Payload::ClosedValue {
      bytes: 1,
      model: "coherent_empty_vector"
    }
  );
  owner!("Governance", "LastProcessedEpoch", 32, 1, 0, Payload::Typed);
  owner!(
    "Governance",
    "CurrentEpochServicePhase",
    32,
    1,
    0,
    Payload::Typed
  );
  owner!(
    "Governance",
    "ProposalMaturityBuckets",
    52,
    1,
    0,
    Payload::ClosedValue {
      bytes: 1,
      model: "coherent_empty_vector"
    }
  );
  owner!(
    "Governance",
    "FinalizedProposalOutcomeExpiryBuckets",
    52,
    1,
    0,
    Payload::ClosedValue {
      bytes: 1,
      model: "coherent_empty_vector"
    }
  );
  owner!(
    "Governance",
    "ExpiryBuckets",
    52,
    1,
    0,
    Payload::ClosedValue {
      bytes: 1,
      model: "coherent_empty_vector"
    }
  );
  owner!(
    "MessageQueue",
    "ServiceHead",
    32,
    1,
    0,
    Payload::ClosedValue {
      bytes: 0,
      model: "closed_recipe_absence"
    }
  );
  owner!(
    "PolkadotXcm",
    "CurrentMigration",
    32,
    1,
    0,
    Payload::ClosedValue {
      bytes: 0,
      model: "closed_recipe_absence"
    }
  );
  owner!(
    "PolkadotXcm",
    "VersionDiscoveryQueue",
    32,
    1,
    0,
    Payload::ClosedValue {
      bytes: 1,
      model: "coherent_empty_vector"
    }
  );
  // The source-owned legacy alias is absent from current runtime metadata.
  let legacy = StorageInfo {
    pallet_name: b"XcmpQueue".to_vec(),
    storage_name: b"InboundXcmpStatus".to_vec(),
    prefix: polkadot_sdk::frame_support::storage::storage_prefix(
      b"XcmpQueue",
      b"InboundXcmpStatus",
    )
    .to_vec(),
    max_values: Some(1),
    max_size: None,
  };
  assert!(stems.contains(&legacy.prefix));
  rows.push(row(
    &legacy,
    stems,
    Role {
      key_bytes: 32,
      queries: 1,
      deletions: 0,
      payload: Payload::ClosedValue {
        bytes: 0,
        model: "closed_recipe_absence",
      },
    },
  ));
  assert_eq!(rows.len(), 28);
  let sum = |field: &str| {
    rows
      .iter() // deos-bypass: bounded-iter -- twenty-eight selected ordinary quiet-hook prefix roles
      .map(|item| item[field].as_u64().unwrap())
      .sum::<u64>()
  };
  json!({"scope": "conditional_ordinary_quiet_hook_input", "typedPrefixRoles": rows,
    "emptyFrameDeclarationEnvelope": empty_frame_declaration_envelope(),
    "readInputBytes": sum("readInputBytes"), "readAndNetDeleteInputBytes": sum("readAndNetDeleteInputBytes"),
    "maximumTrustedValidators": maximum, "trustedValidatorVectorBytes": validators,
    "messageQueueMutex": "environmental token; no storage role",
    "premises": ["nonrotating block; fixed TrustedSet roster/keys within twenty", "no disabled validators or key/roster mutation",
      "coherent quiet Governance, Maturing phase and catch-up one", "empty message/discovery/migration state; no sender input",
      "plain unindexed events and initialized Digest/Events overlay", "live native author; pot absent or live with fresh flags; no dust/reaping",
      "real Timestamp inherent and matching Aura slot"],
    "excluded": ["ParachainSystem inherent/finalization and AuraExt relay callback", "scheduled SessionRotation complete-call input",
      "Actors prepass/Service/producer/economic inputs", "complete SDK/host commit/rollback", "CPU/physical Weight",
      "budget containment", "public chronology/repetition", "tariff", "production binding"]})
}

fn empty_namespace_witness(prefix: &[u8], stems: &BTreeSet<Vec<u8>>, bound: u64) -> Vec<Value> {
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_trie::{
      LayoutV1, MemoryDB, Recorder, StorageProof, TrieConfiguration, TrieDBBuilder,
      TrieDBRawIterator,
    },
  };
  type Layout = LayoutV1<Blake2Hasher>;
  let mut points = Vec::new();
  for case in ["empty_root", "one_peer", "namespace_forest"] {
    let mut rows = Vec::new();
    if case != "empty_root" {
      for stem in
        stems.iter(/* deos-bypass: bounded-iter -- at most 512 guarded runtime namespace stems. */)
      {
        if stem == prefix || (case == "one_peer" && !rows.is_empty()) {
          continue;
        }
        let mut key = stem.clone();
        key.extend_from_slice(&[17; 68]);
        // Synthetic namespace peers exercise node encoding, not admitted storage values.
        rows.push((key, vec![19; 200]));
      }
    }
    rows.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    assert!(rows.iter().all(|(key, _)| !key.starts_with(prefix))); // deos-bypass: bounded-iter -- at most 512 guarded synthetic peers
    let peers = rows.len();
    let mut database = MemoryDB::<Blake2Hasher>::default();
    let root = Layout::trie_build(&mut database, rows);
    let mut recorder = Recorder::<Layout>::new();
    {
      let trie = TrieDBBuilder::<Layout>::new(&database, &root)
        .with_recorder(&mut recorder)
        .build();
      let mut iter = TrieDBRawIterator::new_prefixed(&trie, prefix).unwrap();
      assert!(iter.next_key(&trie).is_none());
    }
    let proof = StorageProof::new(recorder.drain().into_iter().map(|record| record.data));
    let bytes = proof.encoded_size();
    assert!(bytes as u64 <= bound);
    let replay = proof.into_memory_db::<Blake2Hasher>();
    let trie = TrieDBBuilder::<Layout>::new(&replay, &root).build();
    let mut iter = TrieDBRawIterator::new_prefixed(&trie, prefix).unwrap();
    assert!(iter.next_key(&trie).is_none());
    points.push(json!({"case": case, "syntheticPeerCount": peers,
      "recordedEncodedProofBytes": bytes, "conditionalReadInputBytes": bound,
      "emptyPrefixReplay": true, "admittedRuntimeState": false}));
  }
  points
}

fn block_hash_window_witness(
  stems: &BTreeSet<Vec<u8>>,
  read_bound: u64,
  net_bound: u64,
) -> Vec<Value> {
  use polkadot_sdk::{
    sp_core::{Blake2Hasher, H256},
    sp_trie::{
      LayoutV1, MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder,
      TrieDBMutBuilder, TrieMut,
    },
  };
  type Layout = LayoutV1<Blake2Hasher>;
  type Hashes = polkadot_sdk::frame_system::BlockHash<Runtime>;
  let window = <Runtime as polkadot_sdk::frame_system::Config>::BlockHashCount::get();
  assert_eq!(window, 4_096);
  assert_eq!(H256::max_encoded_len(), 32);
  let prefix = &Hashes::hashed_key_for(0u32)[..32];
  let mut points = Vec::new();
  for block in [4_096u32, 4_097, 4_098, 187_204, 2_552_520_738] {
    let parent = block.checked_sub(1).unwrap();
    let first = parent.saturating_sub(window).max(1);
    let prune = block.saturating_sub(window).saturating_sub(1);
    let value = |number: u32| polkadot_sdk::sp_io::hashing::blake2_256(&number.encode()).to_vec();
    let mut hashes = std::collections::BTreeMap::from([(Hashes::hashed_key_for(0u32), value(0))]);
    for number in first..parent {
      hashes.insert(Hashes::hashed_key_for(number), value(number));
    }
    assert_eq!(
      hashes.len(),
      usize::try_from(parent.min(window + 1)).unwrap()
    );
    let genesis_key = Hashes::hashed_key_for(0u32);
    let parent_key = Hashes::hashed_key_for(parent);
    let prune_key = Hashes::hashed_key_for(prune);
    for key in [&genesis_key, &parent_key, &prune_key] {
      assert_eq!(key.len(), 44);
    }
    let expected_genesis = hashes[&genesis_key].clone();
    let expected_pruned = (prune > 0).then(|| hashes[&prune_key].clone());
    assert!(!hashes.contains_key(&parent_key));
    let before_hashes = hashes.len();
    let mut rows = hashes.clone();
    for stem in stems {
      // deos-bypass: bounded-iter -- at most 512 guarded runtime namespace stems
      if stem != prefix {
        let mut key = stem.clone();
        key.extend_from_slice(&[17; 68]);
        assert!(rows.insert(key, vec![19; 200]).is_none());
      }
    }
    assert!(rows.len() <= 4_609);
    let mut database = MemoryDB::<Blake2Hasher>::default();
    let root = Layout::trie_build(&mut database, rows.clone());
    let mut recorder = Recorder::<Layout>::new();
    {
      let trie = TrieDBBuilder::<Layout>::new(&database, &root)
        .with_recorder(&mut recorder)
        .build();
      assert_eq!(
        trie.get(&genesis_key).unwrap(),
        Some(expected_genesis.clone())
      );
      assert!(trie.get(&parent_key).unwrap().is_none());
      if prune > 0 {
        assert_eq!(trie.get(&prune_key).unwrap(), expected_pruned);
      }
    }
    let read_records = recorder.drain();
    assert!(read_records.len() <= 512);
    let read_proof = StorageProof::new(
      read_records
        .iter() // deos-bypass: bounded-iter -- at most 512 guarded synthetic read-path records
        .map(|record| record.data.clone()),
    );
    let read_bytes = read_proof.encoded_size();
    assert!(read_bytes as u64 <= read_bound);
    let replay_read = read_proof.into_memory_db::<Blake2Hasher>();
    let trie = TrieDBBuilder::<Layout>::new(&replay_read, &root).build();
    assert_eq!(
      trie.get(&genesis_key).unwrap(),
      Some(expected_genesis.clone())
    );
    assert!(trie.get(&parent_key).unwrap().is_none());
    if prune > 0 {
      assert_eq!(trie.get(&prune_key).unwrap(), expected_pruned);
    }
    let new_parent_value = value(parent);
    hashes.insert(parent_key.clone(), new_parent_value.clone());
    let transient_hashes = hashes.len();
    if prune > 0 {
      assert!(hashes.remove(&prune_key).is_some());
    }
    assert_eq!(
      hashes.len(),
      usize::try_from(block.min(window + 1)).unwrap()
    );
    assert_eq!(hashes[&genesis_key], expected_genesis);
    rows.insert(parent_key.clone(), new_parent_value.clone());
    if prune > 0 {
      assert!(rows.remove(&prune_key).is_some());
    }
    let expected_root = Layout::trie_root(rows);
    let mut recorded_root = root;
    {
      let mut trie = TrieDBMutBuilder::<Layout>::from_existing(&mut database, &mut recorded_root)
        .with_recorder(&mut recorder)
        .build();
      trie.insert(&parent_key, &new_parent_value).unwrap();
      if prune > 0 {
        trie.remove(&prune_key).unwrap();
      }
    }
    assert_eq!(recorded_root, expected_root);
    let proof = StorageProof::new(
      read_records
        .into_iter()
        .chain(recorder.drain())
        .map(|record| record.data),
    );
    let net_bytes = proof.encoded_size();
    assert!(net_bytes as u64 <= net_bound);
    let mut replay = proof.into_memory_db::<Blake2Hasher>();
    let mut replay_root = root;
    {
      let mut trie =
        TrieDBMutBuilder::<Layout>::from_existing(&mut replay, &mut replay_root).build();
      trie.insert(&parent_key, &new_parent_value).unwrap();
      if prune > 0 {
        trie.remove(&prune_key).unwrap();
      }
    }
    assert_eq!(replay_root, expected_root);
    points.push(json!({"block": block, "firstRetainedParentKey": first, "prunedKey": if prune > 0 { Some(prune) } else { None },
      "beforeHashEntries": before_hashes, "transientHashEntries": transient_hashes, "afterHashEntries": hashes.len(),
      "keyBytes": 44, "valueBytes": 32, "recordedReadProofBytes": read_bytes, "recordedNetProofBytes": net_bytes,
      "conditionalReadInputBytes": read_bound, "conditionalNetInputBytes": net_bound, "genesisPreserved": true,
      "readReplay": true, "rootReplay": true, "syntheticOpaqueHashValues": true, "admittedRuntimeState": false, "literalHistory": false}));
  }
  points
}

pub fn frame_kernel_input(stems: &BTreeSet<Vec<u8>>) -> Value {
  let infos = <polkadot_sdk::frame_system::Pallet<Runtime>>::storage_info();
  assert!(infos.len() <= 64);
  let mut rows = Vec::new();
  macro_rules! owner {
    ($name:literal, $key:expr, $queries:expr, $deletes:expr, $payload:expr) => {{
      let info = infos
        .iter() // deos-bypass: bounded-iter -- at most 64 guarded System storage entries
        .find(|info| info.storage_name == $name.as_bytes())
        .unwrap();
      rows.push(row(
        info,
        stems,
        Role {
          key_bytes: $key,
          queries: $queries,
          deletions: $deletes,
          payload: $payload,
        },
      ));
    }};
  }
  owner!("LastRuntimeUpgrade", 32, 1, 0, Payload::FixedVersion);
  owner!("Number", 32, 1, 0, Payload::Typed);
  owner!("ParentHash", 32, 1, 0, Payload::Typed);
  owner!("Digest", 32, 1, 0, Payload::Overlay);
  // Genesis, the parent and the pruned hash can be three distinct keys.
  owner!("BlockHash", 44, 3, 1, Payload::Typed);
  owner!("BlockWeight", 32, 1, 0, Payload::Typed);
  owner!("BlockSize", 32, 1, 1, Payload::Typed);
  owner!("ExecutionPhase", 32, 1, 1, Payload::Typed);
  owner!("InherentsApplied", 32, 1, 1, Payload::Typed);
  owner!("Events", 32, 1, 0, Payload::Overlay);
  owner!("EventCount", 32, 1, 0, Payload::Typed);
  owner!("EventTopics", 32, 1, 0, Payload::EmptyPrefix);
  owner!("ExtrinsicCount", 32, 1, 1, Payload::Typed);
  // Three declared inherents plus the signed pair; all values are intra-block writes.
  owner!("ExtrinsicData", 44, 5, 0, Payload::Overlay);
  owner!("BlocksTillUpgrade", 32, 1, 0, Payload::Typed);
  owner!("ExtrinsicWeightReclaimed", 32, 1, 1, Payload::Typed);
  for (owner, key, value) in [
    ("Raw::ExtrinsicIndex", well_known_keys::EXTRINSIC_INDEX, 4),
    (
      "Raw::IntraBlockEntropy",
      well_known_keys::INTRABLOCK_ENTROPY,
      32,
    ),
  ] {
    let global_nodes = ancestors(key, stems);
    let blob = value + codec::Compact(value as u32).encoded_size();
    let read = global_nodes * 650 + 1284 + blob + 5;
    rows.push(
      json!({"owner": owner, "keyBytes": key.len(), "queries": 1, "deletions": 1,
      "payloadModel": "raw_fixed_width", "valueAllowanceBytes": value,
      "valueBlobAllowanceBytes": blob,
      "namespacePrefix": key.iter().map(|byte| format!("{byte:02x}")).collect::<String>(), // deos-bypass: bounded-iter -- at most 32-byte guarded raw key
      "globalAncestorNodes": global_nodes, "querySuffixNodes": 0,
      "readInputBytes": read, "readAndNetDeleteInputBytes": read + 1284}),
    );
  }
  assert_eq!(rows.len(), 18);
  let topics = rows
    .iter() // deos-bypass: bounded-iter -- eighteen selected kernel prefix roles
    .find(|item| item["owner"] == "System::EventTopics")
    .unwrap();
  let prefix = infos
    .iter() // deos-bypass: bounded-iter -- at most 64 guarded System storage entries
    .find(|info| info.storage_name == b"EventTopics")
    .unwrap()
    .prefix
    .clone();
  let witnesses =
    empty_namespace_witness(&prefix, stems, topics["readInputBytes"].as_u64().unwrap());
  let hashes = rows
    .iter() // deos-bypass: bounded-iter -- eighteen selected kernel prefix roles
    .find(|item| item["owner"] == "System::BlockHash")
    .unwrap();
  let hash_witnesses = block_hash_window_witness(
    stems,
    hashes["readInputBytes"].as_u64().unwrap(),
    hashes["readAndNetDeleteInputBytes"].as_u64().unwrap(),
  );
  let sum = |field: &str| {
    rows
      .iter() // deos-bypass: bounded-iter -- eighteen selected kernel prefix roles
      .map(|item| item[field].as_u64().unwrap())
      .sum::<u64>()
  };
  json!({"scope": "conditional_direct_system_executive_kernel_input",
    "typedAndRawPrefixRoles": rows,
    "readInputBytes": sum("readInputBytes"), "readAndNetDeleteInputBytes": sum("readAndNetDeleteInputBytes"),
    "emptyEventTopicsPrefixWitnesses": witnesses, "blockHashWindowWitnesses": hash_witnesses, "maximumExtrinsics": 5,
    "premises": ["coherent current namespace/key inventory", "fresh matching runtime version marker; no upgrade",
      "empty EventTopics through plain unindexed events", "real System initialization/reset and prior finalized ExtrinsicData empty",
      "at most three declared inherents and the two signed lifecycle calls", "unit migrator and default System pre/post callbacks"],
    "excluded": ["AllPallets/inherent/economic body inputs", "complete SDK/host commit/rollback",
      "CPU/physical Weight", "budget containment", "public chronology/repetition", "tariff", "production binding"]})
}

pub fn input(stems: &BTreeSet<Vec<u8>>) -> Value {
  assert_eq!(AccountId::max_encoded_len(), 32);
  assert_eq!(Balance::max_encoded_len(), 16);
  let mut infos = <polkadot_sdk::frame_system::Pallet<Runtime>>::storage_info();
  infos.extend(<polkadot_sdk::pallet_balances::Pallet<Runtime>>::storage_info());
  infos.extend(<polkadot_sdk::pallet_assets::Pallet<Runtime>>::storage_info());
  infos.extend(<polkadot_sdk::pallet_transaction_payment::Pallet<Runtime>>::storage_info());
  infos.extend(<pallet_deos_actors::Pallet<Runtime>>::storage_info());
  assert!(infos.len() <= 256);
  let mut rows = Vec::new();
  macro_rules! owner {
    ($pallet:literal, $name:literal, $key:expr, $queries:expr, $deletes:expr, $payload:expr) => {{
      let info = infos
        .iter() // deos-bypass: bounded-iter -- at most 256 guarded five-pallet storage entries
        .find(|info| {
          info.pallet_name == $pallet.as_bytes() && info.storage_name == $name.as_bytes()
        })
        .unwrap();
      rows.push(row(
        info,
        stems,
        Role {
          key_bytes: $key,
          queries: $queries,
          deletions: $deletes,
          payload: $payload,
        },
      ));
    }};
  }
  owner!("System", "Account", 80, 3, 0, Payload::Typed);
  owner!("Balances", "Holds", 80, 2, 0, Payload::Typed);
  owner!("Balances", "TotalIssuance", 32, 1, 0, Payload::Typed);
  owner!("Assets", "Asset", 52, 4, 0, Payload::Typed);
  owner!("Assets", "Account", 100, 4, 0, Payload::Typed);
  owner!(
    "TransactionPayment",
    "NextFeeMultiplier",
    32,
    1,
    0,
    Payload::Typed
  );
  owner!(
    "TransactionPayment",
    "TxPaymentCredit",
    32,
    1,
    1,
    Payload::Credit
  );
  owner!(
    "Actors",
    "CurrentBlockResourceState",
    32,
    1,
    0,
    Payload::Typed
  );
  owner!("System", "BlockHash", 44, 1, 0, Payload::Typed);
  owner!("System", "Number", 32, 1, 0, Payload::Typed);
  owner!("System", "InherentsApplied", 32, 1, 0, Payload::Typed);
  owner!("System", "BlockWeight", 32, 1, 0, Payload::Typed);
  owner!("System", "BlockSize", 32, 1, 0, Payload::Typed);
  owner!(
    "System",
    "ExtrinsicWeightReclaimed",
    32,
    1,
    1,
    Payload::Typed
  );
  owner!("System", "ExecutionPhase", 32, 1, 0, Payload::Typed);
  owner!("System", "EventCount", 32, 1, 0, Payload::Typed);
  owner!("System", "Events", 32, 1, 0, Payload::Overlay);
  owner!("System", "ExtrinsicData", 44, 2, 0, Payload::Overlay);
  for (owner, key) in [
    ("Raw::ExtrinsicIndex", well_known_keys::EXTRINSIC_INDEX),
    (
      "Raw::TransactionLevel",
      polkadot_sdk::frame_support::storage::transactional::TRANSACTION_LEVEL_KEY,
    ),
  ] {
    let global_nodes = ancestors(key, stems);
    let blob = 4 + codec::Compact(4u32).encoded_size();
    let read = global_nodes * 650 + 1284 + blob + 5;
    rows.push(
      json!({"owner": owner, "keyBytes": key.len(), "queries": 1, "deletions": 1,
      "payloadModel": "raw_u32", "valueAllowanceBytes": 4, "valueBlobAllowanceBytes": blob,
      "namespacePrefix": key.iter().map(|byte| format!("{byte:02x}")).collect::<String>(), // deos-bypass: bounded-iter -- at most 32-byte guarded raw key
      "globalAncestorNodes": global_nodes, "querySuffixNodes": 0,
      "readInputBytes": read, "readAndNetDeleteInputBytes": read + 1284}),
    );
  }
  assert_eq!(rows.len(), 20);
  let sum = |field: &str| {
    rows
      .iter() // deos-bypass: bounded-iter -- twenty selected currency/FRAME prefix roles
      .map(|row| row[field].as_u64().unwrap())
      .sum::<u64>()
  };
  json!({"scope": "conditional_signed_pair_currency_frame_input", "typedAndRawPrefixRoles": rows,
    "readInputBytes": sum("readInputBytes"), "readAndNetDeleteInputBytes": sum("readAndNetDeleteInputBytes"),
    "queryNodeBytes": 650, "foreignOrCollapseNodeBytes": 1284,
    "accountPremises": ["Alice/Bob and Fee Sink only", "fresh balance flags", "native floors/provider/holds retained", "no dust/reaping/new-account callbacks"],
    "framePremises": ["real Executive initialization and Events reset", "consensus inherents applied before the pair", "prior finalized ExtrinsicData empty", "plain unindexed events", "Immortal Era / BlockHash(0)", "zero tip and metadata hash disabled", "Actor lifecycle calls not tracked by ingress extension", "unit migrator/default callbacks; no upgrade"],
    "excluded": ["complete block system/finalization input", "host commit/rollback context", "CPU/physical Weight", "tariff", "public chronology/repetition", "production binding"]})
}
