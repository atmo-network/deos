//! Runtime Integrity Tests
//!
//! FRAME collects every pallet `#[pallet::integrity_test]` block into `AllPalletsWithSystem`.
//! Those assertions encode configuration invariants that hold only for one specific combination
//! of runtime constants and generated weights: derived bound equalities, non-zero page sizes, and
//! the requirement that one maximum automatic actor cleanup fits the guaranteed `on_idle` service
//! envelope.
//!
//! A parameter change or a weight regeneration can invalidate any of them, and a node only
//! discovers that at startup. Nothing else in this workspace executes them, so this module is the
//! gate that keeps those invariants inside ordinary local validation.

use super::common::new_test_ext;
use crate::AllPalletsWithSystem;
use polkadot_sdk::frame_support::traits::IntegrityTest;

fn fixed_u64_key_nibble(key: &[u8; 24], depth: usize) -> u8 {
  if depth.is_multiple_of(2) {
    key[depth / 2] >> 4
  } else {
    key[depth / 2] & 15
  }
}

// For every query count, maximize the union of branch costs over the complete key domain.
// A smaller key domain only removes branches or reduces their child count.
fn fixed_u64_key_branch_union(keys: &[[u8; 24]], mut depth: usize, cap: usize) -> Vec<usize> {
  assert!(!keys.is_empty());
  if keys.len() == 1 {
    return vec![0, 0];
  }
  while fixed_u64_key_nibble(&keys[0], depth) == fixed_u64_key_nibble(keys.last().unwrap(), depth) {
    depth += 1;
    assert!(depth < 48, "fixed-width Concat keys must remain unique");
  }
  let mut costs = vec![0];
  let mut start = 0;
  let mut children = 0;
  while start < keys.len() {
    let digit = fixed_u64_key_nibble(&keys[start], depth);
    let end =
      start + keys[start..].partition_point(|key| fixed_u64_key_nibble(key, depth) == digit);
    let child = fixed_u64_key_branch_union(&keys[start..end], depth + 1, cap);
    let mut next = vec![0; (costs.len() + child.len() - 1).min(cap + 1)];
    for (left, left_cost) in costs.iter().enumerate() {
      for (right, right_cost) in child.iter().enumerate().take(next.len() - left) {
        next[left + right] = next[left + right].max(left_cost + right_cost);
      }
    }
    costs = next;
    start = end;
    children += 1;
  }
  assert!((2..=16).contains(&children));
  // Two header, two bitmap, two SCALE-vector bytes and one odd-nibble rounding byte.
  // Every child reference occupies at most 33 bytes; equal-width keys forbid branch values.
  for cost in &mut costs[1..] {
    *cost += 7 + 33 * children;
  }
  costs
}

#[test]
fn runtime_financial_wait_codec_matches_storage_catalog_bound() {
  use codec::{Decode, Encode, MaxEncodedLen};
  use pallet_deos_actors::{
    ActorFinancialWaits, ActorRef, ActorRunAuthority, ActorType, FinancialWaitEpisode,
    FinancialWaitRecord, FinancialWaitScope, UnpaidActorService,
  };
  use polkadot_sdk::frame_support::traits::StorageInfoTrait;
  type Episode = FinancialWaitEpisode<crate::Balance>;
  let scope = FinancialWaitScope {
    actor: ActorRef {
      actor_id: u64::MAX,
      generation: u64::MAX,
    },
    actor_type: ActorType::User,
    contract_authority: ActorRunAuthority {
      semantic_contract_id: [1; 32],
      body_commitment: [2; 32],
      admission_identity: [3; 32],
      pipeline_service_identity: [4; 32],
    },
    behavior_episode: Some([5; 32]),
  };
  let scope_bytes = 8 + 8 + 1 + 4 * 32 + 1 + 32;
  let clock_bytes = 3 * 8; // Entry time, originating Tick duration, absolute expiry Tick.
  let identity_bytes = 3 * 32;
  let value_ceiling = scope_bytes + 1 + 8 + 4 + 4 + 16 + clock_bytes + identity_bytes;
  assert_eq!(scope.encoded_size(), scope_bytes);
  assert_eq!(Episode::max_encoded_len(), value_ceiling);
  assert_eq!(value_ceiling, 331);
  for (boundary, boundary_bytes) in [
    (UnpaidActorService::AtTimeTrigger { due_tick: 1 }, 1 + 8),
    (
      UnpaidActorService::PipelineOpening { cycle_nonce: 1 },
      1 + 8,
    ),
    (
      UnpaidActorService::RunningAction {
        cycle_nonce: 1,
        cursor: u32::MAX,
      },
      1 + 8 + 4,
    ),
    (
      UnpaidActorService::SuspendedAction {
        cycle_nonce: 1,
        cursor: u32::MAX,
        unsuccessful_attempts_at_cursor: u32::MAX,
      },
      1 + 8 + 4 + 4,
    ),
  ] {
    let record =
      FinancialWaitRecord::new(scope, boundary, crate::Balance::MAX, 501, 500, u64::MAX).unwrap();
    let episode = Episode::new(record);
    let encoded = episode.encode();
    assert_eq!(boundary.encoded_size(), boundary_bytes);
    assert_eq!(
      encoded.len(),
      scope_bytes + boundary_bytes + 16 + clock_bytes + identity_bytes
    );
    assert!(encoded.len() <= value_ceiling);
    if matches!(boundary, UnpaidActorService::SuspendedAction { .. }) {
      assert_eq!(encoded.len(), value_ceiling);
    }
    let mut input = &encoded[..];
    assert_eq!(Episode::decode(&mut input).unwrap(), episode);
    assert!(input.is_empty());
    assert!(
      episode
        .bind(scope, boundary, crate::Balance::MAX, 500, u64::MAX)
        .is_ok()
    );
  }
  let key = ActorFinancialWaits::<crate::Runtime>::hashed_key_for(scope.actor.actor_id);
  let prefix_bytes = 2 * 16;
  let suffix_bytes = 16 + u64::max_encoded_len();
  assert_eq!(key.len(), prefix_bytes + suffix_bytes);
  let info = AllPalletsWithSystem::storage_info()
    .into_iter()
    .filter(|info| info.prefix == key[..prefix_bytes])
    .collect::<Vec<_>>();
  assert_eq!(info.len(), 1);
  assert_eq!(info[0].storage_name, b"ActorFinancialWaits");
  assert_eq!(
    info[0].max_values,
    Some(<crate::Runtime as pallet_deos_actors::Config>::MaxActorIdentities::get())
  );
  assert_eq!(
    info[0].max_size,
    Some(u32::try_from(suffix_bytes + value_ceiling).unwrap())
  );
  assert_eq!(suffix_bytes + value_ceiling, 355);
  assert_eq!(suffix_bytes + value_ceiling - u64::max_encoded_len(), 347);
}

/// Runs every pallet integrity assertion against the concrete DEOS runtime configuration.
#[test]
fn runtime_pallet_integrity_holds() {
  new_test_ext().execute_with(|| {
    AllPalletsWithSystem::integrity_test();
  });
}

/// Typed FRAME keys are `Twox128(pallet) || Twox128(item) || suffix`. An absent removal can only
/// reach the unqualified compact-branch merge in trie-db's remove path when a present key extends
/// the absent key, so the typed domain must not telescope. This asserts the metadata half:
/// every storage item prefix has one width and no prefix extends another, which rules out the
/// cross-item case because two items can never share thirty-two prefix bytes. The within-item
/// case reduces to a hasher collision, because a concat map key embeds `hash(key) || key` and an
/// equal-width hashed map has no length difference to exploit.
#[test]
fn runtime_typed_storage_prefixes_do_not_telescope() {
  use polkadot_sdk::frame_support::traits::StorageInfoTrait;
  let infos = AllPalletsWithSystem::storage_info();
  let prefixes: Vec<Vec<u8>> = infos
    .iter()
    .filter(|info| !info.prefix.is_empty())
    .map(|info| info.prefix.clone())
    .collect();
  assert!(
    prefixes.len() > 100,
    "the runtime exposes many storage items"
  );
  let distinct = prefixes.iter().collect::<std::collections::BTreeSet<_>>();
  assert_eq!(
    distinct.len(),
    prefixes.len(),
    "storage item prefixes must be distinct"
  );
  let widths = prefixes
    .iter()
    .map(Vec::len)
    .collect::<std::collections::BTreeSet<_>>();
  assert_eq!(widths, std::collections::BTreeSet::from([2 * 16]));
  for outer in &prefixes {
    for inner in &prefixes {
      if outer.len() < inner.len() {
        assert!(
          !inner.starts_with(outer),
          "no storage item prefix may extend another"
        );
      }
    }
  }
  eprintln!(
    "typed_storage_prefixes items={} widths={widths:?} telescoping_pairs=0 scope=metadata_prefix_half_only",
    prefixes.len()
  );
}

#[test]
fn runtime_governance_fixed_weight_covers_joint_hook_branches() {
  use crate::Runtime;
  use pallet_governance::WeightInfo;

  type W = <Runtime as pallet_governance::Config>::WeightInfo;
  let pending = W::service_pending_enactments(
    <Runtime as pallet_governance::Config>::MaxPendingEnactmentsPerBlock::get(),
  );
  let catch_up = W::service_epoch_catch_up();
  let phases = [
    (
      "maturing",
      W::service_maturing_proposals(
        <Runtime as pallet_governance::Config>::MaxMaturingProposalsPerBlock::get(),
      ),
    ),
    (
      "finalized",
      W::service_finalized_proposal_outcomes(
        <Runtime as pallet_governance::Config>::MaxFinalizedProposalOutcomesPerBlock::get(),
      ),
    ),
    (
      "reward_expiry",
      W::service_expiring_accounts(
        <Runtime as pallet_governance::Config>::MaxExpiringAccountsPerBlock::get(),
      ),
    ),
  ];
  new_test_ext().execute_with(|| {
    let fixed = crate::configs::governance_config::GovernanceFixedWeight::get();
    for (phase, epoch_phase) in phases {
      let joint = pending.saturating_add(catch_up).saturating_add(epoch_phase);
      println!("Governance fixed hook: phase={phase} pending={pending:?} catch_up={catch_up:?} epoch_phase={epoch_phase:?} joint={joint:?} fixed={fixed:?}");
      assert!(joint.all_lte(fixed), "the independent pending pass and epoch phase must fit together: {phase}");
    }
  });
}

#[path = "../../tests/fixtures/actor_key_schema.rs"]
mod actor_key_schema;

#[test]
fn runtime_typed_storage_key_widths_cover_empty_actor_prefix_proofs() {
  use codec::{Decode, Encode, MaxEncodedLen};
  use polkadot_sdk::frame_support::StorageHasher as _;
  use polkadot_sdk::{
    frame_support::__private::{
      metadata::{
        RuntimeMetadata, RuntimeMetadataPrefixed,
        v16::{StorageEntryType, StorageHasher},
      },
      scale_info::{PortableRegistry, TypeDef, TypeDefPrimitive},
    },
    sp_runtime::traits::OpaqueKeys,
    staging_xcm::VersionedAssetId,
  };
  use std::collections::{BTreeMap, BTreeSet};

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
        .iter()
        .try_fold(0usize, |n, field| n.checked_add(child(field.ty.id)?)),
      TypeDef::Tuple(tuple) => tuple
        .fields
        .iter()
        .try_fold(0usize, |n, field| n.checked_add(child(field.id)?)),
      TypeDef::Variant(variants) => {
        let mut maximum = 0;
        for variant in &variants.variants {
          let fields = variant
            .fields
            .iter()
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

  fn has_fixed_width(registry: &PortableRegistry, id: u32, ancestors: &BTreeSet<u32>) -> bool {
    if ancestors.contains(&id) {
      return false;
    }
    let mut ancestors = ancestors.clone();
    ancestors.insert(id);
    let child = |id| has_fixed_width(registry, id, &ancestors);
    match &registry.resolve(id).unwrap().type_def {
      TypeDef::Primitive(primitive) => !matches!(primitive, TypeDefPrimitive::Str),
      TypeDef::Composite(composite) => composite.fields.iter().all(|field| child(field.ty.id)),
      TypeDef::Tuple(tuple) => tuple.fields.iter().all(|field| child(field.id)),
      TypeDef::Array(array) => child(array.type_param.id),
      TypeDef::Variant(variants) => {
        let lengths = variants
          .variants
          .iter()
          .map(|variant| {
            variant.fields.iter().try_fold(0usize, |n, field| {
              child(field.ty.id).then_some(())?;
              n.checked_add(width(registry, field.ty.id, &ancestors)?)
            })
          })
          .collect::<Option<Vec<_>>>();
        lengths.is_some_and(|lengths| {
          !lengths.is_empty() && lengths.windows(2).all(|pair| pair[0] == pair[1])
        })
      }
      TypeDef::Compact(_) | TypeDef::Sequence(_) | TypeDef::BitSequence(_) => false,
    }
  }

  new_test_ext().execute_with(|| {
    let encoded = crate::Runtime::metadata_at_version(16).expect("native metadata v16 exists");
    eprintln!("actor_key_schema domain=unit identity={:02x?}", actor_key_schema::identity(&encoded));
    let mut input = &encoded[..];
    let RuntimeMetadataPrefixed(_, RuntimeMetadata::V16(metadata)) =
      RuntimeMetadataPrefixed::decode(&mut input).expect("native metadata decodes")
    else {
      panic!("native metadata must be v16")
    };
    assert!(input.is_empty());
    let keys = crate::SessionKeys {
      aura: polkadot_sdk::sp_core::sr25519::Public::from_raw([7; 32]).into(),
    };
    assert_eq!(crate::SessionKeys::key_ids().len(), 1);
    let key_id = crate::SessionKeys::key_ids()[0];
    let raw_key = keys.get_raw(key_id);
    assert_eq!(raw_key.len(), 32);
    assert_eq!(keys.encoded_size(), 32);
    assert_eq!(
      polkadot_sdk::sp_consensus_aura::sr25519::AuthorityId::max_encoded_len(),
      32
    );
    let mut owners = BTreeMap::new();
    let mut fixed_key_owners = BTreeSet::new();
    let mut typed_prefixes = BTreeMap::new();
    let mut service_keys = BTreeSet::new();
    for key in [
      polkadot_sdk::sp_core::storage::well_known_keys::CODE,
      polkadot_sdk::sp_core::storage::well_known_keys::PENDING_CODE,
      polkadot_sdk::sp_core::storage::well_known_keys::HEAP_PAGES,
      polkadot_sdk::sp_core::storage::well_known_keys::EXTRINSIC_INDEX,
      polkadot_sdk::sp_core::storage::well_known_keys::INTRABLOCK_ENTROPY,
      polkadot_sdk::frame_support::storage::transactional::TRANSACTION_LEVEL_KEY,
      // Cumulus writes this marker in its genesis builder.
      b":c",
    ] {
      assert!(key.len() <= 32);
      assert!(service_keys.insert(key.to_vec()));
    }
    for pallet in &metadata.pallets {
      let key = polkadot_sdk::frame_support::storage::storage_prefix(
        pallet.name.as_bytes(),
        polkadot_sdk::frame_support::traits::STORAGE_VERSION_STORAGE_KEY_POSTFIX,
      );
      assert_eq!(key.len(), 32);
      assert!(service_keys.insert(key.to_vec()));
    }
    let mut raw_key_owners = 0;
    for pallet in &metadata.pallets {
      let Some(storage) = &pallet.storage else {
        continue;
      };
      for entry in &storage.entries {
        let owner = format!("{}::{}", pallet.name, entry.name);
        let mut key_bytes = 32usize;
        if let StorageEntryType::Map { hashers, key, .. } = &entry.ty {
          let fields = if hashers.len() == 1 {
            vec![key.id]
          } else {
            let TypeDef::Tuple(tuple) = &metadata.types.resolve(key.id).unwrap().type_def else {
              panic!("{owner} must expose one tuple field per hasher")
            };
            tuple.fields.iter().map(|field| field.id).collect()
          };
          if owner == "Actors::ActorContractTailChunk" {
            assert_eq!(
              hashers,
              &[
                StorageHasher::Blake2_128Concat,
                StorageHasher::Blake2_128Concat
              ]
            );
            assert_eq!(
              fields
                .iter()
                .map(|id| width(&metadata.types, *id, &BTreeSet::new()))
                .collect::<Vec<_>>(),
              [Some(8), Some(4)]
            );
          }
          assert_eq!(fields.len(), hashers.len(), "{owner}");
          let fixed_key = hashers.iter().zip(&fields).all(|(hasher, id)| {
            matches!(hasher, StorageHasher::Blake2_128 | StorageHasher::Twox128
              | StorageHasher::Blake2_256 | StorageHasher::Twox256)
              || has_fixed_width(&metadata.types, *id, &BTreeSet::new())
          });
          if fixed_key {
            assert!(fixed_key_owners.insert(owner.clone()));
          }
          for (hasher, id) in hashers.iter().zip(fields) {
            key_bytes += match hasher {
              StorageHasher::Blake2_128 | StorageHasher::Twox128 => 16,
              StorageHasher::Blake2_256 | StorageHasher::Twox256 => 32,
              StorageHasher::Blake2_128Concat
              | StorageHasher::Twox64Concat
              | StorageHasher::Identity => {
                let payload = if owner == "Session::KeyOwner" {
                  // SDK stores a Vec, but this composition publishes one fixed-width Aura key.
                  raw_key_owners += 1;
                  let TypeDef::Tuple(tuple) = &metadata.types.resolve(id).unwrap().type_def else {
                    panic!("Session key ownership must expose its exact tuple")
                  };
                  assert_eq!(tuple.fields.len(), 2);
                  assert_eq!(
                    width(&metadata.types, tuple.fields[0].id, &BTreeSet::new()),
                    Some(4)
                  );
                  let TypeDef::Sequence(sequence) =
                    &metadata.types.resolve(tuple.fields[1].id).unwrap().type_def
                  else {
                    panic!("Session raw key must remain a byte sequence")
                  };
                  assert!(matches!(
                    metadata
                      .types
                      .resolve(sequence.type_param.id)
                      .unwrap()
                      .type_def,
                    TypeDef::Primitive(TypeDefPrimitive::U8)
                  ));
                  (key_id, raw_key.to_vec()).encoded_size()
                } else {
                  width(&metadata.types, id, &BTreeSet::new())
                    .unwrap_or_else(|| panic!("{owner} needs an explicit finite key bound"))
                };
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
        let prefix = polkadot_sdk::frame_support::storage::storage_prefix(
          storage.prefix.as_bytes(),
          entry.name.as_bytes(),
        );
        assert!(typed_prefixes.insert(prefix.to_vec(), key_bytes).is_none());
        assert!(owners.insert(owner, key_bytes).is_none());
      }
    }
    // Check every compiled Actor key, not only owners selected by the proof-model queries below.
    let mut actor_key_owners = 0usize;
    let mut actor_key_maximum = 0usize;
    for (owner, key_bytes) in &owners {
      if owner.starts_with("Actors::") {
        assert!((32..=81).contains(key_bytes), "{owner}: typed Actor key needs requalification");
        actor_key_owners += 1;
        actor_key_maximum = actor_key_maximum.max(*key_bytes);
      }
    }
    assert!(actor_key_owners > 0);
    eprintln!(
      "typed_key_catalogue all_owners={} actor_owners={actor_key_owners} actor_key_maximum={actor_key_maximum} domain=compiled_schema_not_raw_state",
      owners.len(),
    );
    // The selected SDK adds the per-pool fee key; denied administration does not remove its schema.
    assert_eq!(owners.len(), 240);
    assert_eq!(
      owners["AssetConversion::PoolFees"],
      32 + 16 + <crate::Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolId::max_encoded_len()
    );
    let xcmp_alias =
      polkadot_sdk::frame_support::storage::storage_prefix(b"XcmpQueue", b"InboundXcmpStatus")
        .to_vec();
    assert!(service_keys.insert(xcmp_alias.clone()));
    let mut private_aliases = 0;
    for info in super::production_block_replay::runtime_proof_storage_info() {
      if typed_prefixes.contains_key(&info.prefix) {
        continue;
      }
      assert!(
        service_keys.contains(&info.prefix),
        "unlisted storage owner: {info:?}"
      );
      assert_eq!(info.prefix, xcmp_alias);
      assert_eq!(info.max_values, Some(1));
      private_aliases += 1;
    }
    assert_eq!(private_aliases, 1);
    let namespace_stems = typed_prefixes
      .keys()
      .chain(service_keys.iter())
      .cloned()
      .collect::<BTreeSet<_>>();
    #[cfg(feature = "runtime-benchmarks")]
    let namespace_stems = namespace_stems
      .into_iter()
      .chain([crate::configs::staking_config::BENCHMARK_NATIVE_SECURITY_MODE_KEY.to_vec()])
      .collect::<BTreeSet<_>>();
    let watches = <<crate::Runtime as pallet_deos_actors::Config>::MaxParkedBalanceWatches
      as polkadot_sdk::frame_support::traits::Get<u32>>::get() as usize;
    assert_eq!(watches, 1);
    for storage_name in [
      b"DependencyRegistrationFreePositions".as_slice(),
      b"DependencyRegistrationFreeIndices".as_slice(),
    ] {
      let target = polkadot_sdk::frame_support::storage::storage_prefix(b"Actors", storage_name);
      assert!(typed_prefixes.contains_key(target.as_slice()));
      let mut branch_depths = BTreeSet::new();
      for stem in &namespace_stems {
        let matching = stem
          .iter()
          .flat_map(|byte| [byte >> 4, byte & 15])
          .zip(target.iter().flat_map(|byte| [byte >> 4, byte & 15]))
          .take_while(|(left, right)| left == right)
          .count();
        if matching < 64 {
          branch_depths.insert(matching);
        } else {
          assert_eq!(stem.as_slice(), target);
        }
      }
      // Four fixed namespace branches plus the map boundary; source tails remain separate.
      let global_nodes = branch_depths.len() + 1;
      assert_eq!(
        branch_depths.into_iter().collect::<Vec<_>>(),
        [0, 1, 32, 33]
      );
      let source_nodes = 2 * (56 - 32);
      let subtree_nodes = 2 * 32 - 1;
      let encoded_bound = (global_nodes + watches * (source_nodes + subtree_nodes)) * 650 + 5;
      assert_eq!(encoded_bound, 75_405);
    }
    for preset in crate::genesis_config_presets::preset_names() {
      let storage = super::production_block_replay::preset_genesis_storage(&[], &preset);
      assert!(storage.children_default.is_empty());
      assert!(!storage.top.contains_key(&xcmp_alias));
      for key in storage.top.keys() {
        if service_keys.contains(key) {
          assert!(key.len() <= 32);
          continue;
        }
        let bound = typed_prefixes
          .iter()
          .find_map(|(prefix, bound)| key.starts_with(prefix).then_some(*bound))
          .unwrap_or_else(|| panic!("{preset:?}: unlisted genesis key {key:?}"));
        assert!(
          key.len() <= bound,
          "{preset:?}: key exceeds its typed bound"
        );
        assert!(key.len() <= 712);
      }
    }
    assert_eq!(raw_key_owners, 1);
    assert!(fixed_key_owners.contains("Actors::ActorIdentities"));
    assert!(!fixed_key_owners.contains("Actors::ActorWaitingOccupancies"));
    assert_eq!(owners["Session::KeyOwner"], 77);
    let maximum = owners.iter().max_by_key(|(_, bytes)| *bytes).unwrap();
    assert_eq!(maximum.0, "PolkadotXcm::RemoteLockedFungibles");
    assert_eq!(*maximum.1, 712);
    let matching_nodes_for_prefix = |target: &[u8]| {
      let depths = namespace_stems
        .iter()
        .filter_map(|stem| {
          let matching = stem
            .iter()
            .flat_map(|byte| [byte >> 4, byte & 15])
            .zip(target.iter().flat_map(|byte| [byte >> 4, byte & 15]))
            .take_while(|(left, right)| left == right)
            .count();
          (matching < 64).then_some(matching)
        })
        .collect::<BTreeSet<_>>();
      depths.len() + 1
    };
    let matching_nodes = |storage_name: &str| {
      matching_nodes_for_prefix(&polkadot_sdk::frame_support::storage::storage_prefix(
        b"Actors",
        storage_name.as_bytes(),
      ))
    };
    let package_proof_with_prefixes =
      |queries: &[&str],
       deleted: usize,
       deleted_tails: usize,
       payload: usize,
       shared_prefixes: &[(&str, usize)]| {
        let unique_prefixes = queries.iter().copied().collect::<BTreeSet<_>>();
        let globals = unique_prefixes
          .iter()
          .map(|name| matching_nodes(name))
          .sum::<usize>();
        // Each entry reserves one key/clock group; repeated owners retain independent groups.
        let shared_ancestors = shared_prefixes
          .iter()
          .map(|(name, prefix)| {
            assert!(unique_prefixes.contains(name));
            assert!((32..=owners[&format!("Actors::{name}")]).contains(prefix));
            2 * (prefix - 32)
          })
          .sum::<usize>();
        let suffixes = queries
          .iter()
          .map(|name| {
            let width = owners[&format!("Actors::{name}")];
            assert!((32..=81).contains(&width));
            if *name == "ActorContractTailChunk" {
              assert_eq!(width, 76);
              0
            } else {
              let prefix = shared_prefixes
                .iter()
                .find_map(|(owner, prefix)| (owner == name).then_some(*prefix))
                .unwrap_or(32);
              2 * (width - prefix)
            }
          })
          .sum::<usize>();
        let tails = queries
          .iter()
          .filter(|name| **name == "ActorContractTailChunk")
          .count();
        // Read-only and removed coherent tails share the Actor's complete 56-byte prefix.
        let tail_nodes = if tails == 0 {
          0
        } else {
          2 * (56 - 32) + 2 * tails - 1
        };
        let foreign_reads = queries.len() - tails;
        assert!(deleted_tails == 0 || deleted_tails == tails);
        let collapse_nodes = deleted - deleted_tails + usize::from(deleted_tails > 0);
        let coarse = (globals + shared_ancestors + suffixes + tail_nodes) * 650
          + (foreign_reads + collapse_nodes) * 1284
          + payload
          + 5;
        // Fixed-length map keys forbid branch values below their own namespace. Limit this
        // refinement to one query per namespace, leaving grouped pages, complete tails and
        // all earlier namespace/foreign/collapse/value charges at their existing owners.
        let eligible = unique_prefixes.iter().filter_map(|name| {
          let owner = format!("Actors::{name}");
          let occurrences = queries.iter().filter(|query| *query == name).count();
          if occurrences != 1 || *name == "ActorContractTailChunk"
            || shared_prefixes.iter().any(|(group, _)| group == name)
            || !fixed_key_owners.contains(&owner) {
            return None;
          }
          let suffix = owners[&owner] - 32;
          assert!(suffix > 0);
          let old = 2 * suffix * 650;
          // At most two branch positions per suffix byte; partial bytes sum to suffix.
          // Seven branch framing bytes, 16 hashed child references and 37 leaf framing/hash.
          let refined = 2 * suffix * (7 + 16 * 33) + suffix + 37;
          assert!(refined <= old);
          Some((*name, suffix, old - refined))
        }).collect::<Vec<_>>();
        let reduction = eligible.iter().map(|(_, _, bytes)| bytes).sum::<usize>();
        eprintln!("Fixed-key Close input: {}", serde_json::json!({
          "coarsePackageBytes": coarse,
          "refinedPackageBytes": coarse - reduction,
          "singleQueryFixedKeyReadReductionBytes": reduction,
          "eligibleNamespaceSuffixSavings": eligible,
          "queriedRoles": queries,
          "payloadCeilingBytes": payload,
          "sharedPrefixGroups": shared_prefixes,
          "deletionAllowances": deleted,
          "completeDeletedTails": deleted_tails,
        }));
        coarse
      };
    let package_proof = |queries: &[&str], deleted, deleted_tails, payload| {
      package_proof_with_prefixes(queries, deleted, deleted_tails, payload, &[])
    };
    let blob = |value: usize| value + codec::Compact(u32::try_from(value).unwrap()).encoded_size();
    let dormant = [
      "ActorSemanticStates",
      "ActorIdentities",
      "ActorIdentityCount",
      "SovereignIndex",
      "ActorStateHolds",
      "ActorContractHead",
      "ActorRunState",
      "ActorProcesses",
      "ActorControlLocators",
      "ActorUnsignaledControlCells",
      "GlobalCircuitBreaker",
    ];
    let finalizer = [
      "ActorControlLocators",
      "ActorUnsignaledControlCells",
      "ActiveActorCount",
      "ActorIdentityCount",
      "SovereignIndex",
      "TriggerDeadlineHandles",
      "ActorContractHead",
      "ActorRunState",
      "ActorSemanticStates",
      "ActorStateHolds",
    ];
    let identity_bytes = pallet_deos_actors::ActorIdentityOf::<crate::Runtime>::max_encoded_len();
    let hold_bytes =
      pallet_deos_actors::ActorStateHoldRecordOf::<crate::Runtime>::max_encoded_len();
    assert_eq!(identity_bytes, 86);
    assert_eq!(hold_bytes, 112);
    let tail_count = <<crate::Runtime as pallet_deos_actors::Config>::MaxContractSteps
      as polkadot_sdk::frame_support::traits::Get<u32>>::get()
      .saturating_sub(1)
      .div_ceil(pallet_deos_actors::MAX_STEPS_PER_TAIL_CHUNK) as usize;
    assert_eq!(tail_count, 3);
    let body_payload =
      blob(pallet_deos_actors::ActorContractHeadOf::<crate::Runtime>::max_encoded_len())
        + tail_count
          * blob(pallet_deos_actors::ActorStepChunkOf::<crate::Runtime>::max_encoded_len());
    assert_eq!(pallet_deos_actors::ActorStepResourceEnvelope::max_encoded_len(), 36);
    let run_payload = blob(pallet_deos_actors::ActorRunStateOf::<crate::Runtime>::max_encoded_len());
    assert_eq!(run_payload, 216);
    let common_finalizer_payload = body_payload
      + blob(pallet_deos_actors::ActorSemanticStateOf::<crate::Runtime>::max_encoded_len())
      + run_payload;
    let policy_payload =
      blob(pallet_deos_actors::ActorBehaviorPolicyOf::<crate::Runtime>::max_encoded_len())
        + blob(pallet_deos_actors::ActorBehaviorMonitor::max_encoded_len());
    let financial_payload =
      blob(pallet_deos_actors::FinancialWaitEpisode::<crate::Balance>::max_encoded_len());
    assert_eq!((policy_payload, financial_payload), (297, 333));
    let mut close_proof_bounds = std::collections::BTreeMap::new();
    for (class_key, user) in [("OwnerSlotBitmaps", true), ("SystemSovereigns", false)] {
      let mut dormant_queries = dormant.to_vec();
      dormant_queries.push(class_key);
      assert_eq!(dormant_queries.len(), 12);
      let dormant_payload = blob(identity_bytes)
        + blob(1 + identity_bytes + 8)
        + if user { blob(hold_bytes) } else { 0 };
      let dormant_bound = package_proof(
        &dormant_queries,
        if user { 5 } else { 3 },
        0,
        dormant_payload,
      );
      let mut finalizer_queries = finalizer.to_vec();
      finalizer_queries.push(class_key);
      finalizer_queries.extend(std::iter::repeat_n("ActorContractTailChunk", tail_count));
      let finalizer_payload = common_finalizer_payload + if user { blob(hold_bytes) } else { 0 };
      let deleted = if user { 6 } else { 4 } + tail_count;
      let finalizer_bound =
        package_proof(&finalizer_queries, deleted, tail_count, finalizer_payload);
      finalizer_queries.extend(["ActorBehaviorPolicies", "ActorBehaviorMonitors"]);
      if user {
        finalizer_queries.push("ActorFinancialWaits");
      }
      let optional_payload = policy_payload + if user { financial_payload } else { 0 };
      let optional_bound = package_proof(
        &finalizer_queries,
        deleted + if user { 3 } else { 2 },
        tail_count,
        finalizer_payload + optional_payload,
      );
      let expected = if user {
        (438_132, 454_103, 566_437, 299, 11_736, 630)
      } else {
        (404_900, 420_871, 495_854, 185, 11_622, 297)
      };
      assert_eq!(
        (
          dormant_bound,
          finalizer_bound,
          optional_bound,
          dormant_payload,
          finalizer_payload,
          optional_payload
        ),
        expected,
        "{class_key}: Close point-proof inputs need requalification"
      );
      close_proof_bounds.insert((class_key, "D"), dormant_bound);
      close_proof_bounds.insert((class_key, "F"), finalizer_bound);
      close_proof_bounds.insert((class_key, "F optional"), optional_bound);
    }
    let loading = [
      "ActorSemanticStates",
      "ActorControlLocators",
      "ActorIdentities",
      "ActorContractHead",
      "ActorUnsignaledControlCells",
      "ActorProcesses",
      "ServiceNodes",
      "DeadlineHandles",
      "PendingCheckOwners",
      "ActorRunState",
      "TriggerDeadlineHandles",
      "GlobalCircuitBreaker",
    ];
    let loading_payload = common_finalizer_payload
      + blob(pallet_deos_actors::ActorProcessOf::<crate::Runtime>::max_encoded_len())
      + blob(pallet_deos_actors::ServiceNode::<crate::BlockNumber>::max_encoded_len())
      + 2 * blob(pallet_deos_actors::DeadlineHandleOf::<crate::Runtime>::max_encoded_len())
      + blob(pallet_deos_actors::PendingCheckOwner::max_encoded_len());
    let deadline_payload = blob(pallet_deos_actors::DeadlinePage::max_encoded_len());
    let parked_payload = blob(pallet_deos_actors::DependencyTimedReview::<
      crate::BlockNumber,
    >::max_encoded_len())
      + blob(pallet_deos_actors::ParkedBalanceEpisodeOf::<crate::Runtime>::max_encoded_len());
    let mut loading_queries = loading.to_vec();
    loading_queries.extend(std::iter::repeat_n("ActorContractTailChunk", tail_count));
    for (name, extra_queries, extra_payload, expected) in [
      ("L Service/Disabled", vec![], 0, (15, 11_829, 451_026)),
      (
        "L Deadline",
        vec!["DeadlinePages"],
        deadline_payload,
        (16, 12_413, 519_844),
      ),
      (
        "L Parked",
        vec![
          "DeadlinePages",
          "DependencyTimedReviews",
          "ParkedBalanceEpisodes",
        ],
        deadline_payload + parked_payload,
        (18, 12_535, 591_434),
      ),
      (
        "L policy/monitor superset",
        vec![
          "DeadlinePages",
          "DependencyTimedReviews",
          "ParkedBalanceEpisodes",
          "ActorBehaviorPolicies",
          "ActorBehaviorMonitors",
        ],
        deadline_payload + parked_payload + policy_payload,
        (20, 12_832, 663_849),
      ),
      (
        "L waiting Service",
        vec![
          "ActorBehaviorPolicies",
          "ActorBehaviorMonitors",
          "ActorFinancialWaits",
        ],
        policy_payload + financial_payload,
        (18, 12_459, 559_508),
      ),
      (
        "L waiting Deadline",
        vec![
          "DeadlinePages",
          "ActorBehaviorPolicies",
          "ActorBehaviorMonitors",
          "ActorFinancialWaits",
        ],
        deadline_payload + policy_payload + financial_payload,
        (19, 13_043, 628_326),
      ),
    ] {
      let mut queries = loading_queries.clone();
      queries.extend(extra_queries);
      let payload = loading_payload + extra_payload;
      let bound = package_proof(&queries, 1, 0, payload);
      assert_eq!(
        (queries.len(), payload, bound),
        expected,
        "{name}: Close loading proof inputs need requalification"
      );
      close_proof_bounds.insert(("shared", name), bound);
    }
    let service_queries = [
      "ActorControlLocators",
      "ActorUnsignaledControlCells",
      "ActorProcesses",
      "ServiceNodes",
      "ServiceNodes",
      "ServiceNodes",
      "ServiceHeader",
    ];
    let service_payload = 3
      * blob(pallet_deos_actors::ServiceNode::<crate::BlockNumber>::max_encoded_len())
      + blob(pallet_deos_actors::ServiceHeaderRecord::<crate::BlockNumber>::max_encoded_len())
      + blob(pallet_deos_actors::ActorProcessOf::<crate::Runtime>::max_encoded_len());
    let service_bound = package_proof(&service_queries, 1, 0, service_payload);
    assert_eq!((service_payload, service_bound), (239, 214_616));
    close_proof_bounds.insert(("shared", "S"), service_bound);
    let mut waiting_service_queries = service_queries.to_vec();
    waiting_service_queries.extend([
      "ActorSemanticStates",
      "ActorFinancialWaits",
      "ActorContractHead",
      "ActorRunState",
      "TriggerDeadlineHandles",
      "ActorBehaviorPolicies",
      "ActorBehaviorMonitors",
    ]);
    waiting_service_queries.extend(std::iter::repeat_n("ActorContractTailChunk", tail_count));
    let waiting_service_payload = service_payload
      + common_finalizer_payload
      + financial_payload
      + policy_payload
      + blob(pallet_deos_actors::DeadlineHandleOf::<crate::Runtime>::max_encoded_len());
    let waiting_service_bound =
      package_proof(&waiting_service_queries, 1, 0, waiting_service_payload);
    assert_eq!(
      (
        waiting_service_queries.len(),
        waiting_service_payload,
        waiting_service_bound
      ),
      (17, 12_526, 516_691),
    );
    close_proof_bounds.insert(("shared", "S waiting"), waiting_service_bound);
    // One member bucket and one clock index have independently shared first-key prefixes.
    // Selected pages are not complete subtrees: each deletion still owns a collapse sibling.
    use pallet_deos_actors::{WakeupClock, WakeupKey};
    let deadline_prefix = 32 + 16 + WakeupKey::<crate::BlockNumber>::max_encoded_len();
    let index_prefix = 32 + 16 + WakeupClock::max_encoded_len();
    assert_eq!((deadline_prefix, index_prefix), (57, 49));
    for (key, prefix) in [
      (WakeupKey::Block(u32::MAX), 53),
      (WakeupKey::Tick(u64::MAX), 57),
    ] {
      let first_page = pallet_deos_actors::DeadlinePages::<crate::Runtime>::hashed_key_for(key, 0);
      let second_page =
        pallet_deos_actors::DeadlinePages::<crate::Runtime>::hashed_key_for(key, u64::MAX);
      assert_eq!(first_page.len(), prefix + 24);
      assert_eq!(&first_page[..prefix], &second_page[..prefix]);
      let first_index =
        pallet_deos_actors::DeadlineIndexPages::<crate::Runtime>::hashed_key_for(key.clock(), 0);
      let second_index = pallet_deos_actors::DeadlineIndexPages::<crate::Runtime>::hashed_key_for(
        key.clock(),
        u64::MAX,
      );
      assert_eq!(first_index.len(), index_prefix + 24);
      assert_eq!(&first_index[..index_prefix], &second_index[..index_prefix]);
    }
    let shared_deadline_prefixes = [
      ("DeadlinePages", deadline_prefix),
      ("DeadlineIndexPages", index_prefix),
    ];
    let block_index = pallet_deos_actors::DeadlineIndexPages::<crate::Runtime>::hashed_key_for(
      WakeupClock::Block,
      0,
    );
    let tick_index = pallet_deos_actors::DeadlineIndexPages::<crate::Runtime>::hashed_key_for(
      WakeupClock::Tick,
      0,
    );
    assert_ne!(&block_index[..index_prefix], &tick_index[..index_prefix]);
    // Trigger and process removal may use different keys and clocks. Reserve both groups;
    // same-key/same-clock overlap can only reduce the proof, not authorize extra sharing.
    let two_deadline_prefix_groups = [
      ("DeadlinePages", deadline_prefix),
      ("DeadlinePages", deadline_prefix),
      ("DeadlineIndexPages", index_prefix),
      ("DeadlineIndexPages", index_prefix),
    ];
    let header_payload = blob(pallet_deos_actors::DeadlineHeader::max_encoded_len());
    let handle_payload =
      blob(pallet_deos_actors::DeadlineHandleOf::<crate::Runtime>::max_encoded_len());
    let process_payload =
      blob(pallet_deos_actors::ActorProcessOf::<crate::Runtime>::max_encoded_len());
    let semantic_payload =
      blob(pallet_deos_actors::ActorSemanticStateOf::<crate::Runtime>::max_encoded_len());
    assert_eq!((process_payload, semantic_payload), (62, 432));
    let index_page_payload =
      blob(pallet_deos_actors::DeadlineIndexPageOf::<crate::Runtime>::max_encoded_len());
    let scalar_payload = blob(u32::max_encoded_len());
    assert_eq!(
      (
        header_payload,
        handle_payload,
        deadline_payload,
        index_page_payload
      ),
      (42, 35, 584, 291),
    );
    assert_eq!(
      <crate::Runtime as pallet_deos_actors::Config>::MaxActiveActors::get(),
      10_000
    );
    let mut trigger_last_member_input = None;
    let mut primary_deadline_inputs = Vec::new();
    let mut parked_primary_input = None;
    for (owner, handle, authority, authority_payload) in [
      (
        "T",
        "TriggerDeadlineHandles",
        "ActorSemanticStates",
        semantic_payload,
      ),
      ("B", "DeadlineHandles", "ActorProcesses", process_payload),
    ] {
      for (branch, member_pages, index_pages, positions, deleted) in
        [("surviving key", 7, 4, 1, 2), ("last member", 1, 18, 16, 5)]
      {
        let mut queries = vec![
          "ActorControlLocators",
          "ActorUnsignaledControlCells",
          handle,
          authority,
          "DeadlineHeaders",
          "ActorWaitingOccupancies",
          "ActorWaitingCursorIndices",
          "DeadlineIndexLen",
        ];
        queries.extend(std::iter::repeat_n("DeadlinePages", member_pages));
        queries.extend(std::iter::repeat_n("DeadlineIndexPages", index_pages));
        queries.extend(std::iter::repeat_n("DeadlineIndexPositions", positions));
        let payload = handle_payload
          + authority_payload
          + header_payload
          + member_pages * deadline_payload
          + index_pages * index_page_payload
          + (positions + 1) * scalar_payload;
        let bound =
          package_proof_with_prefixes(&queries, deleted, 0, payload, &shared_deadline_prefixes);
        let expected = match (owner, branch) {
          ("T", "surviving key") => (20, 5_771, 745_124),
          ("T", "last member") => (43, 6_416, 1_516_253),
          ("B", "surviving key") => (20, 5_401, 744_754),
          ("B", "last member") => (43, 6_046, 1_515_883),
          _ => unreachable!(),
        };
        assert_eq!(
          (queries.len(), payload, bound),
          expected,
          "{owner} {branch}: Deadline proof inputs need requalification",
        );
        if branch == "last member" {
          close_proof_bounds.insert(("shared", owner), bound);
          if owner == "T" {
            trigger_last_member_input = Some((queries.clone(), deleted, payload));
          }
        }
        if owner == "B" {
          primary_deadline_inputs.push((branch, queries.clone(), deleted, payload));
          // Conservative Parked authentication includes the short-circuited review fallback.
          // AwaitingFunding's direct Tick residence does not enter this branch.
          queries.extend([
            "PendingCheckOwners",
            "DependencyTimedReviews",
            "PendingDependencyReviews",
          ]);
          let payload = payload
            + blob(pallet_deos_actors::PendingCheckOwner::max_encoded_len())
            + 2
              * blob(pallet_deos_actors::DependencyTimedReview::<
                crate::BlockNumber,
              >::max_encoded_len());
          let bound =
            package_proof_with_prefixes(&queries, deleted, 0, payload, &shared_deadline_prefixes);
          let expected = match branch {
            "surviving key" => (23, 5_494, 852_049),
            "last member" => (46, 6_139, 1_623_178),
            _ => unreachable!(),
          };
          assert_eq!(
            (queries.len(), payload, bound),
            expected,
            "B Parked {branch}: Deadline proof inputs need requalification",
          );
          if branch == "last member" {
            close_proof_bounds.insert(("shared", "B Parked"), bound);
            parked_primary_input = Some((queries.clone(), deleted, payload));
          }
        }
      }
    }
    // Park release owns this suffix independently of its optional primary Deadline removal.
    let park_common = [
      "ActorProcesses",
      "PendingCheckOwners",
      "ParkedBalanceEpisodes",
      "DependencyPlans",
      "DeadlineHandles",
      "DependencyTimedReviews",
      "PendingDependencyEvents",
      "PendingDependencyReviews",
    ];
    type Plan = polkadot_sdk::frame_support::BoundedVec<
      pallet_deos_actors::DependencyPlanRegistration,
      <crate::Runtime as pallet_deos_actors::Config>::MaxContractSteps,
    >;
    let park_common_payload = process_payload
      + handle_payload
      + blob(pallet_deos_actors::PendingCheckOwner::max_encoded_len())
      + blob(pallet_deos_actors::ParkedBalanceEpisodeOf::<crate::Runtime>::max_encoded_len())
      + blob(Plan::max_encoded_len())
      + 2
        * blob(pallet_deos_actors::DependencyTimedReview::<
          crate::BlockNumber,
        >::max_encoded_len())
      + blob(pallet_deos_actors::PendingDependencyEvent::max_encoded_len());
    let registration_payload =
      blob(pallet_deos_actors::DependencyRegistrationHandle::max_encoded_len());
    let position_payload =
      blob(pallet_deos_actors::DependencyRegistrationPosition::max_encoded_len());
    let registration_header_payload =
      blob(pallet_deos_actors::DependencyRegistrationHeader::max_encoded_len());
    let registration_page_payload =
      blob(pallet_deos_actors::DependencyRegistrationPage::max_encoded_len());
    let revision_payload = blob(pallet_deos_actors::DependencyRevisionState::max_encoded_len());
    let scan_node_payload = blob(pallet_deos_actors::DependencyScanSourceNode::max_encoded_len());
    let scan_list_payload = blob(pallet_deos_actors::DependencyScanSourceList::max_encoded_len());
    let source_asset_payload =
      blob(<crate::Runtime as pallet_deos_actors::Config>::AssetId::max_encoded_len());
    let source_id_payload = blob(pallet_deos_actors::DependencySourceId::max_encoded_len());
    assert_eq!(
      (
        pallet_deos_actors::DependencyRegistrationPosition::max_encoded_len(),
        u64::max_encoded_len(),
      ),
      (9, 8),
      "Hole values must remain inline under the pinned LayoutV1 model",
    );
    let source = u64::MAX;
    let first = pallet_deos_actors::DependencyRegistrationPosition { page: 0, slot: 0 };
    let last = pallet_deos_actors::DependencyRegistrationPosition {
      page: u64::MAX,
      slot: 31,
    };
    let forward =
      pallet_deos_actors::DependencyRegistrationFreePositions::<crate::Runtime>::hashed_key_for(
        source, 0,
      );
    let forward_last =
      pallet_deos_actors::DependencyRegistrationFreePositions::<crate::Runtime>::hashed_key_for(
        source,
        u64::MAX,
      );
    let reverse =
      pallet_deos_actors::DependencyRegistrationFreeIndices::<crate::Runtime>::hashed_key_for(
        source, first,
      );
    let reverse_last =
      pallet_deos_actors::DependencyRegistrationFreeIndices::<crate::Runtime>::hashed_key_for(
        source, last,
      );
    assert_eq!((forward.len(), reverse.len()), (80, 81));
    assert_eq!(&forward[..56], &forward_last[..56]);
    assert_eq!(&reverse[..56], &reverse_last[..56]);
    let mut parked_suffix_inputs = Vec::new();
    for (branch, pages, revisions, scan_nodes, per_source_deletions) in [
      ("retained page", 1, 0, 0, 2),
      ("interior page", 3, 1, 0, 3),
      ("last source", 1, 1, 3, 8),
      ("mixed-source superset", 3, 1, 3, 8),
    ] {
      let mut queries = park_common.to_vec();
      if scan_nodes > 0 {
        queries.push("DependencyScanSourceListState");
      }
      for _ in 0..watches {
        queries.extend([
          "DependencyRegistrations",
          "DependencyRegistrationPositions",
          "DependencyRegistrationHeaders",
          "DependencySourceBalances",
          "BalanceDependencySources",
        ]);
        queries.extend(std::iter::repeat_n("DependencyRegistrationPages", pages));
        queries.extend(std::iter::repeat_n("DependencyRevisions", revisions));
        queries.extend(std::iter::repeat_n("DependencyScanSourceNodes", scan_nodes));
      }
      let payload = park_common_payload
        + watches
          * (registration_payload
            + position_payload
            + registration_header_payload
            + source_asset_payload
            + source_id_payload
            + pages * registration_page_payload
            + revisions * revision_payload
            + scan_nodes * scan_node_payload)
        + if scan_nodes > 0 { scan_list_payload } else { 0 };
      let deleted = 6 + watches * per_source_deletions;
      let ordinary = package_proof(&queries, deleted, 0, payload);
      let holes = if branch == "retained page" {
        let mut hole_queries = Vec::new();
        for _ in 0..watches {
          hole_queries.extend([
            "DependencyRegistrationFreePositions",
            "DependencyRegistrationFreeIndices",
          ]);
        }
        // Both inserted positions are absent before retirement, independently of other holes.
        package_proof(&hole_queries, 0, 0, 0)
      } else if branch == "last source" {
        [
          "DependencyRegistrationFreePositions",
          "DependencyRegistrationFreeIndices",
        ]
        .iter()
        .map(|name| {
          assert_eq!(matching_nodes(name), 5);
          (matching_nodes(name) + watches * (48 + 63)) * 650 + watches * 1284 + 5
        })
        .sum()
      } else {
        // One new hole plus 31 old local holes: at most 31 pre-state tail relocations.
        // These selected keys are not a complete prefix when unrelated holes survive.
        [
          "DependencyRegistrationFreePositions",
          "DependencyRegistrationFreeIndices",
        ]
        .iter()
        .map(|name| {
          assert_eq!(matching_nodes(name), 5);
          let width = owners[&format!("Actors::{name}")];
          assert_eq!(width, if name.ends_with("Indices") { 81 } else { 80 });
          if name.ends_with("Positions") {
            // The dense stack's bounded index image shares actual branch paths and fanout.
            (matching_nodes(name) + watches * 48) * 650
              + watches * (DENSE_DEPENDENCY_HOLE_STACK_BRANCH_BOUND + 63 * 39 + 31 * 558 + 1284)
              + 5
          } else {
            // Monotone page IDs retain the full suffix domain, but every source row is typed.
            (matching_nodes(name) + watches * 48) * 650
              + watches * (PARTIAL_DEPENDENCY_HOLE_REVERSE_READ_BOUND + 31 * 559 + 1284)
              + 5
          }
        })
        .sum()
      };
      let expected = match branch {
        "retained page" => (14, 8, 1_976, 602_879, 135_173, 738_052),
        "interior page" => (17, 9, 4_165, 769_454, 1_901_256, 2_670_710),
        "last source" => (19, 14, 2_076, 751_653, 153_378, 905_031),
        "mixed-source superset" => (21, 14, 4_230, 881_175, 1_901_256, 2_782_431),
        _ => unreachable!(),
      };
      assert_eq!(
        (
          queries.len(),
          deleted,
          payload,
          ordinary,
          holes,
          ordinary + holes
        ),
        expected,
        "P {branch}: Park release proof inputs need requalification",
      );
      parked_suffix_inputs.push((branch, queries, deleted, payload, holes));
      if branch == "mixed-source superset" {
        close_proof_bounds.insert(("shared", "P suffix"), ordinary + holes);
      }
    }
    // Exact native hold release rewrites these two currency keys, including an empty Holds row.
    fn value_bound<T: MaxEncodedLen>(_: &T) -> usize {
      T::max_encoded_len()
    }
    let holds_bytes = value_bound(
      &polkadot_sdk::pallet_balances::Holds::<crate::Runtime>::get(super::common::ALICE),
    );
    let account_bytes = value_bound(&polkadot_sdk::frame_system::Account::<crate::Runtime>::get(
      super::common::ALICE,
    ));
    assert_eq!((holds_bytes, account_bytes), (73, 80));
    let mut currency_matching = 0;
    for (pallet, name) in [("Balances", "Holds"), ("System", "Account")] {
      assert_eq!(owners[&format!("{pallet}::{name}")], 80);
      let prefix =
        polkadot_sdk::frame_support::storage::storage_prefix(pallet.as_bytes(), name.as_bytes());
      currency_matching += matching_nodes_for_prefix(&prefix) + 2 * (80 - 32);
    }
    let currency_payload = blob(holds_bytes) + blob(account_bytes);
    let currency_bound = currency_matching * 650 + 2 * 1284 + currency_payload + 5;
    assert_eq!(
      (currency_matching, currency_payload, currency_bound),
      (200, 157, 132_730),
      "native currency release proof inputs need requalification",
    );
    let event_owners = ["Number", "ExecutionPhase", "EventCount", "Events"];
    let mut event_matching = 0;
    for name in event_owners {
      assert_eq!(owners[&format!("System::{name}")], 32);
      let key = polkadot_sdk::frame_support::storage::storage_prefix(b"System", name.as_bytes());
      event_matching += matching_nodes_for_prefix(&key);
    }
    let transaction_key =
      polkadot_sdk::frame_support::storage::transactional::TRANSACTION_LEVEL_KEY;
    assert_eq!(transaction_key.len(), 19);
    assert!(service_keys.contains(transaction_key));
    // This helper conservatively includes the raw key's own terminal depth.
    event_matching += matching_nodes_for_prefix(transaction_key);
    let event_payload = blob(crate::BlockNumber::max_encoded_len())
      + blob(polkadot_sdk::frame_system::Phase::max_encoded_len())
      + 2 * blob(u32::max_encoded_len());
    assert_eq!(event_payload, 21);
    // Executive's reset keeps Events append authority in the overlay. Its old external
    // value is never fetched; inline bytes or its hash fit the matching-node allowance.
    let event_bound = event_matching * 650 + 5 * 1284 + event_payload + 5;
    assert_eq!(
      (event_matching, event_payload, event_bound),
      (23, 21, 21_396),
      "FRAME event/transaction key proof inputs need requalification",
    );
    // Compose justified node/payload uppers, including overlap conservatively. This is
    // conditional read/net-root coverage, not CPU, callbacks or a budget-fit certificate.
    let proof = |class, owner| close_proof_bounds[&(class, owner)];
    let common_host = event_bound;
    for (class, user, expected) in [
      (
        "OwnerSlotBitmaps",
        true,
        [592_258, 2_790_124, 4_160_209, 7_121_525, 4_416_548, 1_796_762, 2_864_772],
      ),
      (
        "SystemSovereigns",
        false,
        [426_296, 2_624_162, 3_994_247, 6_955_563, 4_213_235, 0, 0],
      ),
    ] {
      let host = common_host + if user { currency_bound } else { 0 };
      let finalizer_bound = proof(class, "F");
      let trigger = proof("shared", "T");
      let dormant = proof(class, "D") + host;
      let service =
        proof("shared", "L Service/Disabled") + trigger + proof("shared", "S") + finalizer_bound + host;
      let deadline =
        proof("shared", "L Deadline") + trigger + proof("shared", "B") + finalizer_bound + host;
      // Park forbids policy and Run. Its suffix excludes the review Deadline owner.
      let parked = proof("shared", "L Parked")
        + trigger
        + proof("shared", "B Parked")
        + proof("shared", "P suffix")
        + finalizer_bound
        + host;
      let optional = proof("shared", "L policy/monitor superset")
        + trigger
        + proof("shared", "B").max(proof("shared", "S"))
        + proof(class, "F optional")
        + host;
      // Financial waiting is User-only and disarms the independent Trigger.
      let (waiting_service, waiting_deadline) = if user {
        (
          proof("shared", "L waiting Service")
            + proof("shared", "S waiting")
            + proof(class, "F optional")
            + host,
          proof("shared", "L waiting Deadline")
            + proof("shared", "B")
            + proof(class, "F optional")
            + host,
        )
      } else {
        (0, 0)
      };
      assert_eq!(
        [
          dormant,
          service,
          deadline,
          parked,
          optional,
          waiting_service,
          waiting_deadline
        ],
        expected,
        "{class}: composed conditional Close proof inputs need requalification",
      );
      assert_eq!(expected.into_iter().max().unwrap(), parked);
      // One canonical Actor shares its point keys across loading, Trigger removal,
      // Service removal and finalization. Keep other roles/multiplicities distinct.
      let (trigger_queries, trigger_deletions, trigger_payload) =
        trigger_last_member_input.as_ref().unwrap();
      let mut shared_service_queries = Vec::new();
      for name in loading.iter().chain(finalizer.iter()).chain([class].iter()) {
        if *name != "ServiceNodes" && !shared_service_queries.contains(name) {
          shared_service_queries.push(name);
        }
      }
      shared_service_queries.extend(std::iter::repeat_n("ActorContractTailChunk", tail_count));
      shared_service_queries.extend(std::iter::repeat_n("ServiceNodes", 3));
      shared_service_queries.push("ServiceHeader");
      let mut service_union_queries = trigger_queries.clone();
      for name in &shared_service_queries {
        if matches!(*name, "ActorContractTailChunk" | "ServiceNodes")
          || !service_union_queries.contains(name)
        {
          service_union_queries.push(name);
        }
      }
      let process_blob = blob(pallet_deos_actors::ActorProcessOf::<crate::Runtime>::max_encoded_len());
      let service_node_blob =
        blob(pallet_deos_actors::ServiceNode::<crate::BlockNumber>::max_encoded_len());
      let semantic_blob =
        blob(pallet_deos_actors::ActorSemanticStateOf::<crate::Runtime>::max_encoded_len());
      let shared_service_payload = loading_payload + service_payload - process_blob - service_node_blob
        + if user { blob(hold_bytes) } else { 0 };
      let service_union_payload = shared_service_payload + trigger_payload - semantic_blob;
      // Retain all component deletion allowances, including loading's extra sibling.
      let service_deletions = 1 + 1 + if user { 6 } else { 4 } + tail_count;
      let service_union = package_proof_with_prefixes(
        &service_union_queries,
        trigger_deletions + service_deletions,
        tail_count,
        service_union_payload,
        &shared_deadline_prefixes,
      ) + host;
      eprintln!(
        "Conditional Service Close union: {class}; queries={}, payload={}, bytes={service_union}; component sum={service}",
        service_union_queries.len(),
        service_union_payload,
      );
      assert_eq!(
        (service_union_queries.len(), service_union_payload, service_union),
        if user { (62, 18_054, 2_238_017) } else { (62, 17_940, 2_072_055) },
        "{class}: shared Service Close proof inputs need requalification",
      );
      assert!(service_union < service);
      let mut ordinary_deadline_maximum = 0;
      for (branch, primary_queries, primary_deletions, primary_payload) in &primary_deadline_inputs {
        let mut deadline_union_queries = trigger_queries.clone();
        for name in primary_queries {
          if !matches!(*name, "ActorControlLocators" | "ActorUnsignaledControlCells")
            || !deadline_union_queries.contains(name)
          {
            deadline_union_queries.push(name);
          }
        }
        for name in loading.iter().chain(finalizer.iter()).chain([class].iter()) {
          if !deadline_union_queries.contains(name) {
            deadline_union_queries.push(name);
          }
        }
        deadline_union_queries.extend(std::iter::repeat_n("ActorContractTailChunk", tail_count));
        // Both handles and their Actor-keyed authorities are already in the loading payload.
        // Its target-page check shares the primary extraction page, not the Trigger page.
        let deadline_union_payload = loading_payload + trigger_payload + primary_payload
          - semantic_payload - process_payload - 2 * handle_payload
          + if user { blob(hold_bytes) } else { 0 };
        let deadline_union_deletions =
          trigger_deletions + primary_deletions + 1 + if user { 6 } else { 4 } + tail_count;
        let deadline_union = package_proof_with_prefixes(
          &deadline_union_queries,
          deadline_union_deletions,
          tail_count,
          deadline_union_payload,
          &two_deadline_prefix_groups,
        ) + host;
        assert_eq!(
          deadline_union - package_proof_with_prefixes(
            &deadline_union_queries,
            deadline_union_deletions,
            tail_count,
            deadline_union_payload,
            &shared_deadline_prefixes,
          ) - host,
          (2 * (deadline_prefix - 32) + 2 * (index_prefix - 32)) * 650,
          "independent Deadline groups must not share their source/clock ancestors",
        );
        eprintln!(
          "Conditional ordinary Deadline Close union: {class}; branch={branch}, queries={}, payload={deadline_union_payload}, bytes={deadline_union}",
          deadline_union_queries.len(),
        );
        let expected_deadline_union = match (user, *branch) {
          (true, "last member") => (98, 23_841, 3_516_514),
          (true, _) => (75, 23_196, 2_745_385),
          (false, "last member") => (98, 23_727, 3_350_552),
          (false, _) => (75, 23_082, 2_579_423),
        };
        assert_eq!(
          (deadline_union_queries.len(), deadline_union_payload, deadline_union),
          expected_deadline_union,
          "{class} {branch}: shared ordinary Deadline inputs need requalification",
        );
        ordinary_deadline_maximum = ordinary_deadline_maximum.max(deadline_union);
        let mut policy_deadline_queries = deadline_union_queries;
        policy_deadline_queries.extend(["ActorBehaviorPolicies", "ActorBehaviorMonitors"]);
        let policy_deadline_payload = deadline_union_payload + policy_payload;
        let policy_deadline_union = package_proof_with_prefixes(
          &policy_deadline_queries,
          deadline_union_deletions + 2,
          tail_count,
          policy_deadline_payload,
          &two_deadline_prefix_groups,
        ) + host;
        eprintln!(
          "Conditional policy Deadline Close union: {class}; branch={branch}, queries={}, payload={policy_deadline_payload}, bytes={policy_deadline_union}",
          policy_deadline_queries.len(),
        );
        assert_eq!(
          (policy_deadline_queries.len(), policy_deadline_payload, policy_deadline_union),
          (expected_deadline_union.0 + 2, expected_deadline_union.1 + 297,
            expected_deadline_union.2 + 74_983),
          "{class} {branch}: policy Deadline proof inputs need requalification",
        );
        assert!(deadline_union < policy_deadline_union && policy_deadline_union < optional);
        if user {
          // Serving AwaitFunding retains both Deadline owners and no financial episode.
          policy_deadline_queries.push("ActorFinancialWaits");
          let financial_capable_deadline_union = package_proof_with_prefixes(
            &policy_deadline_queries,
            deadline_union_deletions + 3,
            tail_count,
            policy_deadline_payload,
            &two_deadline_prefix_groups,
          ) + host;
          eprintln!(
            "Conditional financial-capable Deadline Close union: {class}; branch={branch}, queries={}, payload={policy_deadline_payload}, bytes={financial_capable_deadline_union}",
            policy_deadline_queries.len(),
          );
          assert_eq!(
            (policy_deadline_queries.len(), policy_deadline_payload, financial_capable_deadline_union),
            (expected_deadline_union.0 + 3, expected_deadline_union.1 + 297,
              expected_deadline_union.2 + 74_983 + 37_018),
            "{class} {branch}: financial-capable Deadline proof inputs need requalification",
          );
          assert!(policy_deadline_union < financial_capable_deadline_union);
          assert!(financial_capable_deadline_union < optional);
        }
      }
      assert_eq!(ordinary_deadline_maximum, if user { 3_516_514 } else { 3_350_552 });
      assert!(ordinary_deadline_maximum < deadline);
      if !user {
        // System pause retains the independent Trigger, without a Service/primary residence.
        let mut disabled_queries = trigger_queries.clone();
        for name in loading.iter().chain(finalizer.iter()).chain([class].iter()) {
          if !disabled_queries.contains(name) {
            disabled_queries.push(name);
          }
        }
        disabled_queries.extend(std::iter::repeat_n("ActorContractTailChunk", tail_count));
        let disabled_payload = loading_payload + trigger_payload - semantic_payload - handle_payload;
        let disabled_deletions = trigger_deletions + 1 + 4 + tail_count;
        let disabled_union = package_proof_with_prefixes(
          &disabled_queries,
          disabled_deletions,
          tail_count,
          disabled_payload,
          &shared_deadline_prefixes,
        ) + host;
        eprintln!(
          "Conditional Disabled Close union: {class}; queries={}, payload={disabled_payload}, bytes={disabled_union}",
          disabled_queries.len(),
        );
        assert_eq!((disabled_queries.len(), disabled_payload, disabled_union), (59, 17_778, 2_001_107));
        assert!(disabled_union < service_union);
        disabled_queries.extend(["ActorBehaviorPolicies", "ActorBehaviorMonitors"]);
        let disabled_policy_payload = disabled_payload + policy_payload;
        let disabled_policy_union = package_proof_with_prefixes(
          &disabled_queries,
          disabled_deletions + 2,
          tail_count,
          disabled_policy_payload,
          &shared_deadline_prefixes,
        ) + host;
        eprintln!(
          "Conditional policy Disabled Close union: {class}; queries={}, payload={disabled_policy_payload}, bytes={disabled_policy_union}",
          disabled_queries.len(),
        );
        assert_eq!(
          (disabled_queries.len(), disabled_policy_payload, disabled_policy_union),
          (61, 18_075, 2_076_090),
        );
        assert_eq!(disabled_policy_union - disabled_union, 74_983);
      }
      let (park_primary_queries, park_primary_deletions, park_primary_payload) =
        parked_primary_input.as_ref().unwrap();
      let pending_owner_payload = blob(pallet_deos_actors::PendingCheckOwner::max_encoded_len());
      let timed_review_payload = blob(pallet_deos_actors::DependencyTimedReview::<
        crate::BlockNumber,
      >::max_encoded_len());
      assert_eq!((pending_owner_payload, timed_review_payload), (25, 34));
      let mut parked_union_maximum = 0;
      let mut parked_financial_capable_maximum = 0;
      for (branch, suffix_queries, suffix_deletions, suffix_payload, holes) in &parked_suffix_inputs {
        let mut parked_queries = trigger_queries.clone();
        for name in park_primary_queries {
          if !matches!(*name, "ActorControlLocators" | "ActorUnsignaledControlCells")
            || !parked_queries.contains(name)
          {
            parked_queries.push(name);
          }
        }
        for name in suffix_queries {
          // Only the named Actor-keyed common roles share identity. Every source/registration/
          // page/scan role retains its multiplicity even when its storage namespace repeats.
          if !park_common.contains(name) || !parked_queries.contains(name) {
            parked_queries.push(name);
          }
        }
        for name in loading.iter().chain(finalizer.iter()).chain([class].iter()) {
          if !parked_queries.contains(name) {
            parked_queries.push(name);
          }
        }
        parked_queries.extend(std::iter::repeat_n("ActorContractTailChunk", tail_count));
        // Coherent Park has no Run. Keep its absence query and every deletion allowance.
        let parked_payload = loading_payload + trigger_payload + park_primary_payload + suffix_payload
          - semantic_payload - 2 * process_payload - 3 * handle_payload
          - 2 * pending_owner_payload - 2 * timed_review_payload - run_payload
          + if user { blob(hold_bytes) } else { 0 };
        let parked_deletions = trigger_deletions + park_primary_deletions + suffix_deletions
          + 1 + if user { 6 } else { 4 } + tail_count;
        let parked_ordinary = package_proof_with_prefixes(
          &parked_queries,
          parked_deletions,
          tail_count,
          parked_payload,
          &two_deadline_prefix_groups,
        ) + host;
        let parked_union = parked_ordinary + holes;
        eprintln!(
          "Conditional Parked Close union: {class}; branch={branch}, queries={}, payload={parked_payload}, ordinary={parked_ordinary}, holes={holes}, bytes={parked_union}; absent Run blob={run_payload}",
          parked_queries.len(),
        );
        let (expected_queries, user_payload, user_ordinary, user_union) = match *branch {
          "retained page" => (109, 25_479, 4_011_848, 4_147_021),
          "interior page" => (112, 27_668, 4_178_423, 6_079_679),
          "last source" => (114, 25_579, 4_160_622, 4_314_000),
          "mixed-source superset" => (116, 27_733, 4_290_144, 6_191_400),
          _ => unreachable!(),
        };
        assert_eq!(
          (parked_queries.len(), parked_payload, parked_ordinary, parked_union),
          (expected_queries, user_payload - if user { 0 } else { 114 },
            user_ordinary - if user { 0 } else { 165_962 },
            user_union - if user { 0 } else { 165_962 }),
          "{class} {branch}: shared Parked proof inputs need requalification",
        );
        assert!(parked_union < parked);
        parked_union_maximum = parked_union_maximum.max(parked_union);
        if user {
          // Park forbids close_when but permits AwaitFunding. Serving has no financial episode;
          // destruction still clears that absent key. This is independent of wait residence.
          parked_queries.push("ActorFinancialWaits");
          let financial_capable_parked_union = package_proof_with_prefixes(
            &parked_queries,
            parked_deletions + 1,
            tail_count,
            parked_payload,
            &two_deadline_prefix_groups,
          ) + host + holes;
          eprintln!(
            "Conditional financial-capable Parked Close union: {class}; branch={branch}, queries={}, payload={parked_payload}, bytes={financial_capable_parked_union}",
            parked_queries.len(),
          );
          assert_eq!(financial_capable_parked_union - parked_union, 37_018);
          assert!(financial_capable_parked_union < parked + 37_018);
          parked_financial_capable_maximum =
            parked_financial_capable_maximum.max(financial_capable_parked_union);
        }
      }
      assert_eq!(parked_suffix_inputs.len(), 4);
      assert_eq!(parked_union_maximum, if user { 6_191_400 } else { 6_025_438 });
      assert_eq!(parked_financial_capable_maximum, if user { 6_228_418 } else { 0 });
      assert!(parked_union_maximum < parked);
      // Optional companions use the same Actor ID across authentication, cancellation and
      // destruction. Their bytes are counted once; every deletion sibling remains reserved.
      let mut policy_service_queries = service_union_queries;
      policy_service_queries.extend(["ActorBehaviorPolicies", "ActorBehaviorMonitors"]);
      let policy_service_payload = service_union_payload + policy_payload;
      let policy_service = package_proof_with_prefixes(
        &policy_service_queries,
        trigger_deletions + service_deletions + 2,
        tail_count,
        policy_service_payload,
        &shared_deadline_prefixes,
      ) + host;
      eprintln!(
        "Conditional policy Service Close union: {class}; queries={}, payload={policy_service_payload}, bytes={policy_service}",
        policy_service_queries.len(),
      );
      assert_eq!(
        (policy_service_queries.len(), policy_service_payload, policy_service),
        if user { (64, 18_351, 2_313_000) } else { (64, 18_237, 2_147_038) },
        "{class}: shared policy Service Close proof inputs need requalification",
      );
      assert!(policy_service < optional);
      if user {
        // A Serving AwaitFunding Contract retains its ordinary Trigger. Destruction still
        // clears the financial key, whose absent value contributes no blob payload.
        policy_service_queries.push("ActorFinancialWaits");
        let financial_capable_service = package_proof_with_prefixes(
          &policy_service_queries,
          trigger_deletions + service_deletions + 3,
          tail_count,
          policy_service_payload,
          &shared_deadline_prefixes,
        ) + host;
        eprintln!(
          "Conditional financial-capable Service Close union: {class}; queries={}, payload={policy_service_payload}, bytes={financial_capable_service}",
          policy_service_queries.len(),
        );
        assert_eq!(
          (policy_service_queries.len(), policy_service_payload, financial_capable_service),
          (65, 18_351, 2_350_018),
          "{class}: Serving financial-capable Close proof inputs need requalification",
        );
        assert!(policy_service < financial_capable_service && financial_capable_service < optional);
        // AwaitingFunding disarms the independent Trigger and has no primary Deadline while
        // tentatively on Service. Keep both absence probes, never its old extraction topology.
        shared_service_queries.extend([
          "ActorBehaviorPolicies",
          "ActorBehaviorMonitors",
          "ActorFinancialWaits",
        ]);
        let waiting_payload = shared_service_payload + policy_payload + financial_payload;
        let waiting_union = package_proof(
          &shared_service_queries,
          service_deletions + 3,
          tail_count,
          waiting_payload,
        ) + host;
        eprintln!(
          "Conditional waiting Service Close union: {class}; queries={}, payload={waiting_payload}, bytes={waiting_union}",
          shared_service_queries.len(),
        );
        assert_eq!(
          (shared_service_queries.len(), waiting_payload, waiting_union),
          (26, 12_700, 977_471),
          "{class}: shared waiting Service Close proof inputs need requalification",
        );
        assert!(waiting_union < waiting_service);
        // The waiting source and primary extraction refer to the same process, handle and
        // member page. Financial waiting has no independent Trigger or Parked review fallback.
        let mut waiting_deadline_maximum = 0;
        for (branch, primary_queries, primary_deletions, primary_payload) in &primary_deadline_inputs {
          let mut waiting_deadline_queries = primary_queries.clone();
          for name in loading.iter().chain(finalizer.iter()).chain([class].iter()) {
            if !waiting_deadline_queries.contains(name) {
              waiting_deadline_queries.push(name);
            }
          }
          waiting_deadline_queries.extend(std::iter::repeat_n("ActorContractTailChunk", tail_count));
          waiting_deadline_queries.extend([
            "ActorBehaviorPolicies",
            "ActorBehaviorMonitors",
            "ActorFinancialWaits",
          ]);
          let waiting_deadline_payload = loading_payload + primary_payload - process_payload
            - handle_payload + blob(hold_bytes) + policy_payload + financial_payload;
          let waiting_deadline_union = package_proof_with_prefixes(
            &waiting_deadline_queries,
            primary_deletions + 1 + 6 + tail_count + 3,
            tail_count,
            waiting_deadline_payload,
            &shared_deadline_prefixes,
          ) + host;
          eprintln!(
            "Conditional waiting Deadline Close union: {class}; branch={branch}, queries={}, payload={waiting_deadline_payload}, bytes={waiting_deadline_union}",
            waiting_deadline_queries.len(),
          );
          assert_eq!(
            (waiting_deadline_queries.len(), waiting_deadline_payload, waiting_deadline_union),
            if *branch == "last member" { (62, 18_522, 2_279_403) } else { (39, 17_877, 1_508_274) },
            "{branch}: shared waiting Deadline Close proof inputs need requalification",
          );
          waiting_deadline_maximum = waiting_deadline_maximum.max(waiting_deadline_union);
        }
        assert_eq!(primary_deadline_inputs.len(), 2);
        assert_eq!(waiting_deadline_maximum, 2_279_403);
        assert!(waiting_deadline_maximum < waiting_deadline);
      }
    }
    use pallet_deos_actors::{CancellationReason, CloseReason, CycleResult, OutcomeTotals};
    assert_eq!(
      (
        pallet_deos_actors::ActorId::max_encoded_len(),
        u64::max_encoded_len(),
        CloseReason::max_encoded_len(),
        CancellationReason::max_encoded_len(),
        CycleResult::max_encoded_len(),
        OutcomeTotals::max_encoded_len(),
        crate::AccountId::max_encoded_len(),
        crate::Balance::max_encoded_len(),
        crate::RuntimeHoldReason::max_encoded_len(),
        polkadot_sdk::frame_system::Phase::max_encoded_len(),
      ),
      (8, 8, 1, 2, 1, 24, 32, 16, 2, 5),
      "Close event field widths need requalification",
    );
    let reason = CloseReason::OwnerInitiated;
    let events = [
      crate::RuntimeEvent::Actors(pallet_deos_actors::Event::ActorClosed {
        actor_id: 0,
        reason,
      }),
      crate::RuntimeEvent::Actors(pallet_deos_actors::Event::CycleCancelled {
        actor_id: 0,
        cycle_nonce: 1,
        reason: CancellationReason::Closing(reason),
      }),
      crate::RuntimeEvent::Actors(pallet_deos_actors::Event::CycleSummary {
        actor_id: 0,
        cycle_nonce: 1,
        result: CycleResult::Cancelled,
        outcomes: OutcomeTotals::default(),
      }),
      crate::RuntimeEvent::Balances(polkadot_sdk::pallet_balances::Event::Endowed {
        account: super::common::ALICE,
        free_balance: crate::EXISTENTIAL_DEPOSIT,
      }),
      crate::RuntimeEvent::Balances(polkadot_sdk::pallet_balances::Event::Released {
        reason: crate::RuntimeHoldReason::Actors(pallet_deos_actors::HoldReason::ActorState),
        who: super::common::ALICE,
        amount: crate::EXISTENTIAL_DEPOSIT,
      }),
    ];
    let record_bytes = events.map(|event| {
      // These scalar-width witnesses qualify encoding only, not a dispatched event trace.
      let record = polkadot_sdk::frame_system::EventRecord::<_, crate::Hash> {
        phase: polkadot_sdk::frame_system::Phase::ApplyExtrinsic(0),
        event,
        topics: Vec::new(),
      };
      record.encoded_size()
    });
    assert_eq!(record_bytes, [17, 26, 49, 56, 58]);
    let [closed, cancelled, summary, endowed, released] = record_bytes;
    assert_eq!(
      (
        closed,
        cancelled + summary + closed,
        released + closed,
        cancelled + summary + released + closed,
        record_bytes.into_iter().sum::<usize>(),
      ),
      (17, 92, 75, 150, 206),
      "Close event suffix bounds need requalification",
    );
    // Compact width changes only at these three boundaries. Five Close records cannot cross
    // two boundaries; examine every possible crossing offset, including count saturation.
    let max_records = record_bytes.len() as u32;
    let mut vector_count_growth = 0;
    for boundary in [0u32, 1 << 6, 1 << 14, 1 << 30, u32::MAX] {
      for offset in 0..=max_records {
        let count = boundary.saturating_sub(offset);
        for appended in 0..=max_records {
          let Some(next) = count.checked_add(appended) else {
            continue;
          };
          let growth = codec::Compact(next).encoded_size() - codec::Compact(count).encoded_size();
          vector_count_growth = vector_count_growth.max(growth);
        }
      }
    }
    assert_eq!(
      (max_records, vector_count_growth, 206 + vector_count_growth),
      (5, 2, 208)
    );
    // An ordinary Append buffer can have no encoded count yet. Its first materialization owns
    // the entire count prefix, rather than just the logical vector-count width increase.
    let first_count_prefix = codec::Compact::<u32>::max_encoded_len();
    assert_eq!((first_count_prefix, 206 + first_count_prefix), (5, 211));
    assert_eq!(endowed + released + closed, 131);
    let asset_id_bytes = 1
      + polkadot_sdk::staging_xcm::v3::AssetId::max_encoded_len()
        .max(polkadot_sdk::staging_xcm::v4::AssetId::max_encoded_len())
        .max(polkadot_sdk::staging_xcm::v5::AssetId::max_encoded_len());
    assert_eq!(712, 32 + 8 + 4 + 16 + 32 + 16 + asset_id_bytes);
    assert_eq!(VersionedAssetId::max_encoded_len() + 1, asset_id_bytes);
    let junction = polkadot_sdk::staging_xcm::v3::Junction::AccountId32 {
      network: Some(polkadot_sdk::staging_xcm::v3::NetworkId::ByFork {
        block_number: u64::MAX,
        block_hash: [0xff; 32],
      }),
      id: [0xff; 32],
    };
    let asset_id = VersionedAssetId::V3(polkadot_sdk::staging_xcm::v3::AssetId::Concrete(
      polkadot_sdk::staging_xcm::v3::MultiLocation {
        parents: u8::MAX,
        interior: polkadot_sdk::staging_xcm::v3::Junctions::X8(
          junction, junction, junction, junction, junction, junction, junction, junction,
        ),
      },
    ));
    assert_eq!(asset_id.encoded_size(), asset_id_bytes);
    let mut key = polkadot_sdk::frame_support::storage::storage_prefix(
      b"PolkadotXcm",
      b"RemoteLockedFungibles",
    )
    .to_vec();
    key.extend(polkadot_sdk::frame_support::Twox64Concat::hash(
      &u32::MAX.encode(),
    ));
    key.extend(polkadot_sdk::frame_support::Blake2_128Concat::hash(
      &crate::AccountId::new([0xff; 32]).encode(),
    ));
    key.extend(polkadot_sdk::frame_support::Blake2_128Concat::hash(
      &asset_id.encode(),
    ));
    assert_eq!(key.len(), 712);
  });
}

#[test]
fn system_raw_storage_requires_root_and_can_exceed_the_ordinary_key_domain() {
  use crate::{RuntimeCall, RuntimeOrigin};
  use polkadot_sdk::{
    frame_support::{assert_noop, assert_ok, traits::Contains},
    sp_runtime::{DispatchError, traits::Dispatchable},
  };

  new_test_ext().execute_with(|| {
    let key = vec![0xfe; 713];
    let items = vec![(key.clone(), vec![7])];
    let calls = [
      RuntimeCall::System(polkadot_sdk::frame_system::Call::set_storage {
        items: items.clone(),
      }),
      RuntimeCall::System(polkadot_sdk::frame_system::Call::kill_storage {
        keys: vec![key.clone()],
      }),
      RuntimeCall::System(polkadot_sdk::frame_system::Call::kill_prefix {
        prefix: key.clone(),
        subkeys: 1,
      }),
    ];
    for call in calls {
      assert!(crate::configs::RuntimeCallFilter::contains(&call));
      for origin in [
        RuntimeOrigin::signed(super::common::ALICE),
        RuntimeOrigin::none(),
      ] {
        assert_noop!(
          call.clone().dispatch(origin).map_err(|error| error.error),
          DispatchError::BadOrigin
        );
      }
    }
    assert_ok!(crate::System::set_storage(RuntimeOrigin::root(), items));
    assert_eq!(
      polkadot_sdk::sp_io::storage::get(&key).as_deref(),
      Some(&[7][..])
    );
    assert_ok!(crate::System::kill_storage(
      RuntimeOrigin::root(),
      vec![key.clone()],
    ));
    assert!(!polkadot_sdk::sp_io::storage::exists(&key));
  });
}

#[test]
fn privileged_origin_inventory_rejects_signed_authority_escalation() {
  use crate::{Runtime, RuntimeOrigin};
  use polkadot_sdk::frame_support::traits::EnsureOrigin;

  macro_rules! assert_root_only {
    ($origin:ty, $name:literal) => {{
      assert!(
        <$origin as EnsureOrigin<RuntimeOrigin>>::try_origin(RuntimeOrigin::root()).is_ok(),
        "{} must accept Root",
        $name,
      );
      assert!(
        <$origin as EnsureOrigin<RuntimeOrigin>>::try_origin(RuntimeOrigin::signed(
          super::common::ALICE,
        ))
        .is_err(),
        "{} must reject signed authority",
        $name,
      );
    }};
  }

  assert_root_only!(
    <Runtime as pallet_deos_actors::Config>::SystemOrigin,
    "Actors System control"
  );
  assert_root_only!(
    <Runtime as pallet_deos_actors::Config>::GlobalBreakerOrigin,
    "Actors global breaker"
  );
  assert_root_only!(
    <Runtime as pallet_governance::Config>::AdminOrigin,
    "Governance administration"
  );
  assert_root_only!(
    <Runtime as pallet_deos_router::Config>::AdminOrigin,
    "DEOS Router administration"
  );
  assert_root_only!(
    <Runtime as pallet_staking::Config>::AdminOrigin,
    "DEOS Staking administration"
  );
  assert_root_only!(
    <Runtime as pallet_staking::Config>::SecurityRewardFundingOrigin,
    "security reward funding"
  );
  assert_root_only!(
    <Runtime as pallet_tmc::Config>::AdminOrigin,
    "TMC administration"
  );
  assert_root_only!(
    <Runtime as polkadot_sdk::pallet_assets::Config>::ForceOrigin,
    "pallet-assets force administration"
  );
  assert_root_only!(
    <Runtime as pallet_asset_registry::Config>::RegistryOrigin,
    "Asset Registry administration"
  );
  assert_root_only!(
    <Runtime as pallet_oracle::Config>::RegisterOrigin,
    "DEOS Oracle feed registration"
  );
  assert_root_only!(
    <Runtime as polkadot_sdk::pallet_preimage::Config>::ManagerOrigin,
    "preimage administration"
  );
  assert_root_only!(
    <Runtime as polkadot_sdk::pallet_xcm::Config>::AdminOrigin,
    "XCM administration"
  );
  assert_root_only!(
    <Runtime as polkadot_sdk::cumulus_pallet_xcmp_queue::Config>::ControllerOrigin,
    "XCMP queue control"
  );

  type AssetCreator = <Runtime as polkadot_sdk::pallet_assets::Config>::CreateOrigin;
  assert!(
    <AssetCreator as polkadot_sdk::frame_support::traits::EnsureOriginWithArg<
      RuntimeOrigin,
      u32,
    >>::try_origin(RuntimeOrigin::root(), &0)
    .is_ok(),
    "pallet-assets creation must accept Root",
  );
  assert!(
    <AssetCreator as polkadot_sdk::frame_support::traits::EnsureOriginWithArg<
      RuntimeOrigin,
      u32,
    >>::try_origin(RuntimeOrigin::signed(super::common::ALICE), &0)
    .is_err(),
    "pallet-assets creation must reject signed authority",
  );

  type CollatorUpdater = <Runtime as polkadot_sdk::pallet_collator_selection::Config>::UpdateOrigin;
  assert!(
    <CollatorUpdater as EnsureOrigin<RuntimeOrigin>>::try_origin(RuntimeOrigin::root()).is_ok(),
    "collator updates accept Root",
  );
  assert!(
    <CollatorUpdater as EnsureOrigin<RuntimeOrigin>>::try_origin(RuntimeOrigin::signed(
      super::common::ALICE,
    ))
    .is_err(),
    "collator updates reject signed authority outside the explicit relay StakingAdmin path",
  );

  type OraclePublisher = <Runtime as pallet_oracle::Config>::PublishOrigin;
  assert!(
    <OraclePublisher as EnsureOrigin<RuntimeOrigin>>::try_origin(RuntimeOrigin::signed(
      super::common::ALICE,
    ))
    .is_ok(),
    "Oracle publication is the explicitly bounded signed exception",
  );
  assert!(
    <OraclePublisher as EnsureOrigin<RuntimeOrigin>>::try_origin(RuntimeOrigin::root()).is_err(),
    "Oracle publication must still pass feed-level producer checks",
  );
}

// Bound by the complete current dense-index hash image, independently of the selected holes.
const DENSE_DEPENDENCY_HOLE_STACK_BRANCH_BOUND: usize = 104_987;

// Fixed-width reverse suffixes have no branch values or foreign rows below the source prefix.
const PARTIAL_DEPENDENCY_HOLE_REVERSE_READ_BOUND: usize = 63 * (50 * (7 + 16 * 33) + 25 + 14);

#[test]
fn runtime_partial_dependency_hole_reverse_proof_uses_typed_subtrees() {
  use codec::{Decode, Encode, MaxEncodedLen};
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_trie::{
      LayoutV1, MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder,
      TrieDBMutBuilder, TrieLayout, TrieMut, delta_trie_root,
    },
  };
  type Layout = LayoutV1<Blake2Hasher>;
  type Positions = pallet_deos_actors::DependencyRegistrationFreePositions<crate::Runtime>;
  type Indices = pallet_deos_actors::DependencyRegistrationFreeIndices<crate::Runtime>;
  type Position = pallet_deos_actors::DependencyRegistrationPosition;
  assert!(!Layout::USE_EXTENSION);
  assert_eq!(Layout::MAX_INLINE_VALUE, Some(33));
  assert_eq!(
    (Position::max_encoded_len(), u64::max_encoded_len()),
    (9, 8)
  );
  let source = u64::MAX;
  let positions = (0..95u64)
    .map(|index| Position {
      page: if index < 31 {
        u64::MAX
      } else {
        (1 << 63) + ((index - 31) / 31) * (1 << 40)
      },
      slot: ((index % 31) + 1) as u8,
    })
    .collect::<Vec<_>>();
  let newly_retired = Position {
    page: u64::MAX,
    slot: 0,
  };
  let key = Indices::hashed_key_for(source, newly_retired);
  assert_eq!(key.len(), 81);
  let prefix = &key[..56];
  for position in &positions {
    let key = Indices::hashed_key_for(source, *position);
    assert_eq!(key.len(), 81);
    assert_eq!(&key[..56], prefix);
  }
  // A 25-byte equal-width suffix has no branch value; all sixteen children can be hashed.
  let fanout = (0..=u8::MAX).map(|last| {
    let mut suffix = vec![0x11; 24];
    suffix.push(last);
    (suffix, vec![0x22; 8])
  });
  let branch = Layout::trie_root_unhashed(fanout);
  assert_eq!((branch.len(), branch.encoded_size()), (555, 557));
  assert!(branch.encoded_size() <= 2 + 25 + 2 + 16 * 33 + 2);
  let per_source = 48 * 650 + PARTIAL_DEPENDENCY_HOLE_REVERSE_READ_BOUND + 31 * 559 + 1284;
  let watches =
    <crate::Runtime as pallet_deos_actors::Config>::MaxParkedBalanceWatches::get() as usize;
  assert_eq!(watches, 1);
  assert_eq!(
    (
      PARTIAL_DEPENDENCY_HOLE_REVERSE_READ_BOUND,
      per_source,
      5 * 650 + watches * per_source + 5,
    ),
    (1_687_707, 1_737_520, 1_740_775),
  );

  // The new slot-zero hole is popped first; the 31 old local holes pull 31 outside tail rows.
  // This typed trie primitive preserves both hole maps, without publishing Actor topology.
  let mut before = std::collections::BTreeMap::new();
  for (index, position) in positions.iter().enumerate() {
    before.insert(
      Positions::hashed_key_for(source, index as u64),
      position.encode(),
    );
    before.insert(
      Indices::hashed_key_for(source, *position),
      (index as u64).encode(),
    );
  }
  before.insert(
    Positions::hashed_key_for(source - 1, 0),
    newly_retired.encode(),
  );
  before.insert(
    Indices::hashed_key_for(source - 1, newly_retired),
    0u64.encode(),
  );
  assert_eq!(before.len(), 192);
  let mut database = MemoryDB::<Blake2Hasher>::default();
  let mut root = Default::default();
  {
    let mut trie = TrieDBMutBuilder::<Layout>::new(&mut database, &mut root).build();
    for (key, value) in &before {
      trie.insert(key, value).unwrap();
    }
  }
  let mut recorder = Recorder::<Layout>::new();
  {
    let trie = TrieDBBuilder::<Layout>::new(&database, &root)
      .with_recorder(&mut recorder)
      .build();
    for index in (0..31u64).chain(64..=95) {
      let key = Positions::hashed_key_for(source, index);
      assert_eq!(trie.get(&key).unwrap(), before.get(&key).cloned());
      let position = positions
        .get(index as usize)
        .copied()
        .unwrap_or(newly_retired);
      let key = Indices::hashed_key_for(source, position);
      assert_eq!(trie.get(&key).unwrap(), before.get(&key).cloned());
    }
  }
  let read_proof = StorageProof::new(recorder.drain().into_iter().map(|record| record.data));
  let read_bound = 2 * (65 + 48) * 650
    + DENSE_DEPENDENCY_HOLE_STACK_BRANCH_BOUND
    + 63 * 39
    + PARTIAL_DEPENDENCY_HOLE_REVERSE_READ_BOUND
    + 2 * 1284
    + 5;
  assert!(read_proof.encoded_size() <= read_bound);
  let mut after = before.clone();
  let mut delta = Vec::new();
  for index in 0..31u64 {
    let target = Positions::hashed_key_for(source, index);
    let tail = Positions::hashed_key_for(source, 94 - index);
    let position = positions[(94 - index) as usize];
    let old_reverse = Indices::hashed_key_for(source, positions[index as usize]);
    let new_reverse = Indices::hashed_key_for(source, position);
    after.insert(target.clone(), position.encode());
    after.remove(&tail);
    after.remove(&old_reverse);
    after.insert(new_reverse.clone(), index.encode());
    delta.extend([
      (target, Some(position.encode())),
      (tail, None),
      (old_reverse, None),
      (new_reverse, Some(index.encode())),
    ]);
  }
  for index in 0..64u64 {
    let position = Position::decode(&mut &after[&Positions::hashed_key_for(source, index)][..])
      .expect("every retained dense offset has a typed position");
    assert_eq!(
      after[&Indices::hashed_key_for(source, position)],
      index.encode()
    );
  }
  assert_eq!(
    after.len(),
    130,
    "both maps retain exactly 64 rows plus the peer source"
  );
  assert!(!after.contains_key(&Indices::hashed_key_for(source, newly_retired)));
  let post_root = delta_trie_root::<Layout, _, _, _, _, _>(
    &mut database,
    root,
    delta.clone(),
    Some(&mut recorder),
    None,
  )
  .unwrap();
  assert_eq!(post_root, Layout::trie_root(after));
  let proof = StorageProof::new(
    read_proof
      .iter_nodes()
      .cloned()
      .chain(recorder.drain().into_iter().map(|record| record.data)),
  );
  assert!(proof.encoded_size() <= read_bound + 31 * (558 + 559));
  let mut replay = proof.into_memory_db::<Blake2Hasher>();
  assert_eq!(
    delta_trie_root::<Layout, _, _, _, _, _>(&mut replay, root, delta, None, None).unwrap(),
    post_root,
  );
}

#[test]
fn runtime_dense_dependency_hole_stack_bounds_partial_prefix_proof() {
  use codec::{Encode, MaxEncodedLen};
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_trie::{
      LayoutV1, MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder,
      TrieDBMutBuilder, TrieLayout, TrieMut, delta_trie_root,
    },
  };
  type Layout = LayoutV1<Blake2Hasher>;
  type Positions = pallet_deos_actors::DependencyRegistrationFreePositions<crate::Runtime>;

  assert!(!Layout::USE_EXTENSION);
  assert_eq!(Layout::MAX_INLINE_VALUE, Some(33));
  assert_eq!(
    pallet_deos_actors::DependencyRegistrationPosition::max_encoded_len(),
    9
  );
  let actors = <crate::Runtime as pallet_deos_actors::Config>::MaxActiveActors::get() as usize;
  assert_eq!(actors, 10_000);
  // The production append guard rejects free_count >= 32 * MaxActiveActors before key access.
  let domain = 32 * actors;
  let source = u64::MAX;
  let prefix = Positions::hashed_key_for(source, 0)[..56].to_vec();
  let mut keys = (0..domain)
    .map(|index| {
      let key = Positions::hashed_key_for(source, index as u64);
      assert_eq!(key.len(), 80);
      assert_eq!(&key[..56], prefix);
      <[u8; 24]>::try_from(&key[56..]).unwrap()
    })
    .collect::<Vec<_>>();
  keys.sort_unstable();
  assert!(keys.windows(2).all(|pair| pair[0] != pair[1]));
  eprintln!("Dense hole-stack proof: hashed/sorted {domain} guarded offsets; maximizing 63 paths");
  let costs = fixed_u64_key_branch_union(&keys, 0, 63);
  assert_eq!(costs.len(), 64);
  assert!(costs.windows(2).all(|pair| pair[0] <= pair[1]));
  // Shared partial-key bytes sum to at most 24 per queried path. Leaves add at most 15 bytes.
  // A selected-key deletion may load one typed sibling; complete-prefix loss adds one foreign node.
  let read = costs[63] + 63 * (24 + 15);
  let per_source = 48 * 650 + read + 31 * 558 + 1284;
  let watches =
    <crate::Runtime as pallet_deos_actors::Config>::MaxParkedBalanceWatches::get() as usize;
  assert_eq!(watches, 1);
  let table = 5 * 650 + watches * per_source + 5;
  assert_eq!(
    (costs[63], read, per_source, table),
    (
      DENSE_DEPENDENCY_HOLE_STACK_BRANCH_BOUND,
      107_444,
      157_226,
      160_481
    ),
    "the dense hole-stack key-domain proof needs requalification",
  );

  // A coherent retained C32 page has at least one registration. For N registrations,
  // P <= N and the committed hole count is <= 32P - N <= 31N. Empty-page retirement
  // may append one provisional hole before reclaiming that page's 32 slots. Include
  // its offset's absence proof as well as all pre-state dense offsets.
  let coherent_last_offset = 31 * actors;
  let coherent_keys = keys
    .iter()
    .copied()
    .filter(|key| {
      u64::from_le_bytes(key[16..24].try_into().unwrap()) <= coherent_last_offset as u64
    })
    .collect::<Vec<_>>();
  assert_eq!(coherent_keys.len(), coherent_last_offset + 1);
  eprintln!(
    "Dense hole-stack proof: coherent/transient domain {} offsets; maximizing 63 paths",
    coherent_keys.len(),
  );
  let coherent_costs = fixed_u64_key_branch_union(&coherent_keys, 0, 63);
  assert_eq!(coherent_costs.len(), 64);
  assert!(
    coherent_costs
      .iter()
      .zip(&costs)
      .all(|(tight, coarse)| tight <= coarse)
  );
  let coherent_read = coherent_costs[63] + 63 * (24 + 15);
  let coherent_per_source = 48 * 650 + coherent_read + 31 * 558 + 1284;
  let coherent_table = 5 * 650 + watches * coherent_per_source + 5;
  eprintln!(
    "Coherent dense hole-stack proof: branch={} read={coherent_read} per_source={coherent_per_source} watched_sources={watches} table={coherent_table}; coarse_table={table}",
    coherent_costs[63],
  );
  assert_eq!(
    (
      coherent_costs[63],
      coherent_read,
      coherent_per_source,
      coherent_table
    ),
    (104_430, 106_887, 156_669, 159_924),
    "the coherent/transient dense hole-stack proof needs requalification",
  );

  // Typed trie primitive: 31 swap-last deletions retain the source and another source's row.
  let mut before = (0..95u64)
    .map(|index| {
      (
        Positions::hashed_key_for(source, index),
        pallet_deos_actors::DependencyRegistrationPosition {
          // Leave a live slot in every possible retained C32 page.
          page: index / 31,
          slot: (index % 31) as u8,
        }
        .encode(),
      )
    })
    .collect::<std::collections::BTreeMap<_, _>>();
  let peer = Positions::hashed_key_for(source - 1, 0);
  before.insert(
    peer.clone(),
    pallet_deos_actors::DependencyRegistrationPosition { page: 9, slot: 0 }.encode(),
  );
  let mut database = MemoryDB::<Blake2Hasher>::default();
  let mut root = Default::default();
  {
    let mut trie = TrieDBMutBuilder::<Layout>::new(&mut database, &mut root).build();
    for (key, value) in &before {
      trie.insert(key, value).unwrap();
    }
  }
  let mut recorder = Recorder::<Layout>::new();
  {
    let trie = TrieDBBuilder::<Layout>::new(&database, &root)
      .with_recorder(&mut recorder)
      .build();
    for index in (0..31u64).chain(64..=95) {
      let key = Positions::hashed_key_for(source, index);
      assert_eq!(trie.get(&key).unwrap(), before.get(&key).cloned());
    }
  }
  let read_proof = StorageProof::new(recorder.drain().into_iter().map(|record| record.data));
  assert!(read_proof.encoded_size() <= (65 + 48) * 650 + read + 1284 + 5);
  let mut after = before.clone();
  let mut delta = Vec::new();
  for index in 0..31u64 {
    let target = Positions::hashed_key_for(source, index);
    let tail = Positions::hashed_key_for(source, 94 - index);
    let value = before[&tail].clone();
    after.insert(target.clone(), value.clone());
    after.remove(&tail);
    delta.extend([(target, Some(value)), (tail, None)]);
  }
  let post_root = delta_trie_root::<Layout, _, _, _, _, _>(
    &mut database,
    root,
    delta.clone(),
    Some(&mut recorder),
    None,
  )
  .unwrap();
  assert_eq!(post_root, Layout::trie_root(after));
  let proof = StorageProof::new(
    read_proof
      .iter_nodes()
      .cloned()
      .chain(recorder.drain().into_iter().map(|record| record.data)),
  );
  assert!(proof.encoded_size() <= (65 + 48) * 650 + read + 31 * 558 + 1284 + 5);
  let mut replay = proof.into_memory_db::<Blake2Hasher>();
  assert_eq!(
    delta_trie_root::<Layout, _, _, _, _, _>(&mut replay, root, delta, None, None).unwrap(),
    post_root
  );
}

#[test]
fn runtime_fixed_key_branches_exclude_values_before_read_refinement() {
  use codec::Encode;
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_trie::{LayoutV1, TrieConfiguration, TrieLayout},
  };
  type Layout = LayoutV1<Blake2Hasher>;
  assert!(!Layout::USE_EXTENSION);
  assert_eq!(Layout::MAX_INLINE_VALUE, Some(33));
  // Canonical V1: <=162 partial nibbles take at most two header bytes; hash refs are 32 bytes.
  // This is an independent node envelope, not a co-reachable maximum or a runtime Weight.
  let raw_actor_node_bound = 2 + 81 + 2 + 16 * (1 + 32) + (1 + 32);
  let framed_actor_node_bound =
    raw_actor_node_bound + codec::Compact(raw_actor_node_bound as u32).encoded_size();
  assert_eq!((raw_actor_node_bound, framed_actor_node_bound), (646, 648));
  assert!(framed_actor_node_bound <= 650); // Preserve the existing conservative structural pad.
  let mut encoded_cases = 0usize;
  let mut largest_raw = 0usize;
  for width in 0..=81usize {
    for value_len in [0usize, 32, 33, 64] {
      let value = vec![0x22; value_len];
      let leaf = Layout::trie_root_unhashed([(vec![0x11; width], value.clone())]);
      assert!(leaf.len() <= raw_actor_node_bound);
      assert!(leaf.encoded_size() <= framed_actor_node_bound);
      largest_raw = largest_raw.max(leaf.len());
      encoded_cases += 1;
      if width == 81 {
        continue;
      }
      let prefix = vec![0x11; width];
      let children = (0..16u8)
        .map(|index| {
          let mut key = prefix.clone();
          key.push(index << 4);
          (key, vec![0x33; 33])
        })
        .collect::<Vec<_>>();
      let mut entries = children;
      entries.push((prefix, value));
      let branch = Layout::trie_root_unhashed(entries);
      assert!(branch.len() <= raw_actor_node_bound);
      assert!(branch.encoded_size() <= framed_actor_node_bound);
      largest_raw = largest_raw.max(branch.len());
      encoded_cases += 1;
    }
    if width < 81 {
      // Valueless branches may end their common partial at either nibble of a byte.
      for odd_partial in [false, true] {
        let entries = (0..16u8).map(|index| {
          let mut key = vec![0x11; width];
          key.push(if odd_partial {
            0x10 | index
          } else {
            index << 4
          });
          (key, vec![0x33; 33])
        });
        let branch = Layout::trie_root_unhashed(entries);
        assert!(branch.len() <= raw_actor_node_bound);
        assert!(branch.encoded_size() <= framed_actor_node_bound);
        largest_raw = largest_raw.max(branch.len());
        encoded_cases += 1;
      }
    }
  }
  eprintln!(
    "actor_node_envelope cases={encoded_cases} largest_raw_observed={largest_raw} raw_bound={raw_actor_node_bound} vec_framed_bound={framed_actor_node_bound} scope=canonical_codec_not_weight"
  );
  // Check the terminal framing bound across inline/hash transition and all selected widths.
  for suffix in [17usize, 24, 48] {
    for value_len in [0usize, 32, 33, 64] {
      let leaf = Layout::trie_root_unhashed([(vec![0x11; suffix], vec![0x22; value_len])]);
      assert!(leaf.encoded_size() <= suffix + 37);
    }
  }
  let prefix = vec![0x11; 23];
  let mut fixed = (0..=u8::MAX)
    .map(|last| {
      let mut key = prefix.clone();
      key.push(last);
      (key, vec![last; 64])
    })
    .collect::<Vec<_>>();
  // A full hashed-child fork with a shared partial; each complete key is exactly 24 bytes.
  let no_value = Layout::trie_root_unhashed(fixed.clone()).encoded_size();
  assert!(no_value <= 7 + 16 * 33 + prefix.len());
  // A shorter complete key is a branch value. It invalidates the refinement's premise.
  fixed.push((prefix.clone(), vec![0x77; 64]));
  let with_value = Layout::trie_root_unhashed(fixed).encoded_size();
  assert!(with_value > 7 + 16 * 33 + prefix.len());
  eprintln!(
    "Fixed-key branch framing: no_value={no_value} with_value={with_value}; branch=535 partial={}",
    prefix.len()
  );
}

#[test]
fn runtime_occupied_lookup_path_bounds_ignore_foreign_key_width() {
  use codec::Encode;
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_trie::{LayoutV1, MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder},
  };
  type Layout = LayoutV1<Blake2Hasher>;
  let mut cases = 0usize;
  for width in [0usize, 1, 32, 56, 80, 81] {
    for value_len in [32usize, 33, 112, 4096] {
      let target = vec![0; width];
      let value = vec![0x55; value_len];
      let mut entries = std::collections::BTreeMap::new();
      entries.insert(Vec::new(), vec![0x99; 4096]);
      entries.insert(vec![0xff; 2048], vec![0x77; 4096]);
      // A fork at every query nibble saturates path depth. Siblings exceed the typed key domain.
      for nibble in 0..2 * width {
        let mut foreign = target.clone();
        foreign[nibble / 2] = if nibble % 2 == 0 { 0x10 } else { 0x01 };
        foreign.extend_from_slice(&[0xee; 2048]);
        entries.insert(foreign, vec![0x77; 4096]);
      }
      entries.insert(target.clone(), value.clone());
      let mut database = MemoryDB::<Blake2Hasher>::default();
      let root = Layout::trie_build(&mut database, entries);
      let mut recorder = Recorder::<Layout>::new();
      assert_eq!(
        TrieDBBuilder::<Layout>::new(&database, &root)
          .with_recorder(&mut recorder)
          .build()
          .get(&target)
          .unwrap(),
        Some(value.clone()),
      );
      let records = recorder.drain();
      let external_value = usize::from(value_len >= 33);
      let node_limit = 2 * width + 1;
      assert_eq!(records.len(), node_limit + external_value);
      assert_eq!(
        records.iter().filter(|record| record.data == value).count(),
        external_value
      );
      // Reuse canonical V1 codec accounting; only matching path partials are query-width bounded.
      let raw_node_bound = 2 + width + 2 + 16 * (1 + 32) + (1 + 32);
      assert!(
        records
          .iter()
          .filter(|record| record.data != value)
          .all(|record| record.data.len() <= raw_node_bound)
      );
      let proof = StorageProof::new(records.into_iter().map(|record| record.data));
      let input_bound = codec::Compact((node_limit + external_value) as u32).encoded_size()
        + node_limit * (raw_node_bound + codec::Compact(raw_node_bound as u32).encoded_size())
        + external_value * value.encoded_size();
      assert!(proof.encoded_size() <= input_bound);
      let replay = proof.into_memory_db::<Blake2Hasher>();
      assert_eq!(
        TrieDBBuilder::<Layout>::new(&replay, &root)
          .build()
          .get(&target)
          .unwrap(),
        Some(value)
      );
      cases += 1;
    }
  }
  eprintln!(
    "occupied_lookup cases={cases} max_key_bytes=81 max_path_nodes=163 external_value_separate=true foreign_key_bytes_above=2048 scope=canonical_single_positive_lookup_not_weight"
  );
}

#[test]
fn runtime_occupied_lookup_path_bound_excludes_absence_and_repair() {
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_trie::{
      LayoutV1, MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder,
      delta_trie_root,
    },
  };
  type Layout = LayoutV1<Blake2Hasher>;
  // Synthetic retained prefix, not an admitted Actor namespace or a runtime state.
  let mut target = vec![0x11; 16];
  target.extend_from_slice(&[0; 40]);
  let mut short_absent = target.clone();
  *short_absent.last_mut().unwrap() = 1;
  let mut foreign = short_absent.clone();
  foreign.extend_from_slice(&[0xee; 2048]);
  let mut marker = vec![0x11; 16];
  marker.extend_from_slice(&[0xff; 16]);
  let mut entries = std::collections::BTreeMap::from([
    (target.clone(), vec![0x55; 112]),
    (foreign, vec![0x77; 8]),
    (marker.clone(), vec![0x99; 8]),
  ]);
  let mut database = MemoryDB::<Blake2Hasher>::default();
  let root = Layout::trie_build(&mut database, entries.clone());
  let mut recorder = Recorder::<Layout>::new();
  assert_eq!(
    TrieDBBuilder::<Layout>::new(&database, &root)
      .with_recorder(&mut recorder)
      .build()
      .get(&target)
      .unwrap(),
    Some(vec![0x55; 112])
  );
  let read_nodes = recorder
    .drain()
    .into_iter()
    .map(|record| record.data)
    .collect::<std::collections::BTreeSet<_>>();
  let raw_node_bound = 2 + target.len() + 2 + 16 * (1 + 32) + (1 + 32);
  assert!(read_nodes.iter().all(|data| data.len() <= raw_node_bound));
  let read_proof = StorageProof::new(read_nodes.iter().cloned());
  let delta = [(target.clone(), None::<Vec<u8>>)];
  let mut incomplete = read_proof.into_memory_db::<Blake2Hasher>();
  assert!(
    delta_trie_root::<Layout, _, _, _, _, _>(&mut incomplete, root, delta.clone(), None, None)
      .is_err()
  );
  // A short absent query can reach a nonmatching long leaf, unlike an occupied path.
  assert_eq!(
    TrieDBBuilder::<Layout>::new(&database, &root)
      .with_recorder(&mut recorder)
      .build()
      .get(&short_absent)
      .unwrap(),
    None
  );
  assert!(
    recorder
      .drain()
      .iter()
      .any(|record| record.data.len() > raw_node_bound)
  );
  let post_root = delta_trie_root::<Layout, _, _, _, _, _>(
    &mut database,
    root,
    delta.clone(),
    Some(&mut recorder),
    None,
  )
  .unwrap();
  entries.remove(&target);
  assert_eq!(post_root, Layout::trie_root(entries));
  assert_eq!(
    TrieDBBuilder::<Layout>::new(&database, &post_root)
      .build()
      .get(&marker)
      .unwrap(),
    Some(vec![0x99; 8])
  );
  let extra = recorder
    .drain()
    .into_iter()
    .map(|record| record.data)
    .filter(|data| !read_nodes.contains(data))
    .collect::<std::collections::BTreeSet<_>>();
  assert_eq!(extra.len(), 1);
  let extra_bytes = extra.first().unwrap().len();
  assert!(extra_bytes > 646);
  let proof = StorageProof::new(read_nodes.into_iter().chain(extra));
  let mut replay = proof.into_memory_db::<Blake2Hasher>();
  assert_eq!(
    delta_trie_root::<Layout, _, _, _, _, _>(&mut replay, root, delta, None, None).unwrap(),
    post_root
  );
  eprintln!(
    "occupied_lookup_repair target_key_bytes=56 retained_prefix_bytes=16 extra_nodes=1 extra_bytes={extra_bytes} scope=foreign_width_premise_counterexample_not_actor_state"
  );
}

#[test]
fn runtime_absent_delete_is_a_noop_when_the_query_diverges_inside_the_branch() {
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_trie::{
      LayoutV1, MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder,
      delta_trie_root,
    },
  };
  type Layout = LayoutV1<Blake2Hasher>;
  // Synthetic keys, not a typed Actor state. Two present keys share a value-bearing compact
  // branch, and the absent query diverges inside that branch's partial.
  //
  // Deliberately excluded geometry: when an absent query ends exactly at a value-bearing
  // NibbledBranch whose compressed partial continues beyond the query, and that node has one
  // used child, the selected trie-db0.32.0 fix_inner merges the child after advancing past the
  // end of the query key. Only debug_assert guards that advance, so the following left() panics
  // with an out-of-range slice in release. The reproduction and panic text are retained under
  // sdk2609-absent-prefix-deletion; absent removal is therefore not yet a general no-op.
  let prefix = vec![0x11; 16];
  let stem = [prefix.as_slice(), &[0x00]].concat();
  let absent = [stem.as_slice(), &[0x21]].concat();
  let occupied = [stem.as_slice(), &[0x20]].concat();
  let child = [occupied.as_slice(), &[0x30]].concat();
  let marker = [prefix.as_slice(), &[0x0f]].concat();
  let entries = std::collections::BTreeMap::from([
    (occupied.clone(), vec![0x55; 33]),
    (child.clone(), vec![0x77; 8]),
    (marker.clone(), vec![0x99; 8]),
  ]);
  assert!(!entries.contains_key(&absent));
  assert!(entries.keys().all(|key| !key.starts_with(&absent)));
  let mut database = MemoryDB::<Blake2Hasher>::default();
  let root = Layout::trie_build(&mut database, entries.clone());
  assert_eq!(root, Layout::trie_root(entries));
  let mut recorder = Recorder::<Layout>::new();
  assert_eq!(
    TrieDBBuilder::<Layout>::new(&database, &root)
      .with_recorder(&mut recorder)
      .build()
      .get_hash(&absent)
      .unwrap(),
    None
  );
  let proof = StorageProof::new(recorder.drain().into_iter().map(|record| record.data));
  let absent_bytes = absent.len();
  let delta = [(absent, None::<Vec<u8>>)];
  let after =
    delta_trie_root::<Layout, _, _, _, _, _>(&mut database, root, delta.clone(), None, None)
      .unwrap();
  let replay = delta_trie_root::<Layout, _, _, _, _, _>(
    &mut proof.into_memory_db::<Blake2Hasher>(),
    root,
    delta,
    None,
    None,
  )
  .unwrap();
  assert_eq!(replay, after);
  assert_eq!(
    after, root,
    "a diverging absent delete must leave the root unchanged"
  );
  let view = TrieDBBuilder::<Layout>::new(&database, &after).build();
  assert_eq!(view.get(&occupied).unwrap(), Some(vec![0x55; 33]));
  assert_eq!(view.get(&child).unwrap(), Some(vec![0x77; 8]));
  assert_eq!(view.get(&marker).unwrap(), Some(vec![0x99; 8]));
  eprintln!(
    "absent_delete diverging_query_bytes={} root_unchanged=true proof_replay_matches=true strict_prefix_geometry=excluded_unqualified",
    absent_bytes
  );
}

/// Multi-delete batches under the verified premises: canonical V1, no extension, typed keys that
/// cannot telescope. Each independent cluster reuses the proven single-delete shape, and one extra
/// cluster nests a key and its descendant to probe a cascading merge. The assertions cover replay
/// correctness and the per-delete repair ceiling; the printed extras count is a finite observation
/// about these shapes, never a tariff, additive price or whole-call bound.
#[test]
fn runtime_multi_delete_batch_repair_stays_within_the_single_delete_ceiling() {
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_trie::{
      LayoutV1, MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder,
      delta_trie_root,
    },
  };
  type Layout = LayoutV1<Blake2Hasher>;
  // A surviving sibling below the 32-byte child reference threshold is inlined into its parent
  // node, so the repair never needs a separate read; an out-of-line sibling does. Both variants
  // are exercised because the difference, not the key count, decides the repair read.
  for survivor_bytes in [8usize, 33] {
    let mut entries = std::collections::BTreeMap::new();
    let mut deleted = Vec::new();
    for cluster in 0..4u8 {
      let prefix = vec![0x10 + cluster; 16];
      let marker = [prefix.as_slice(), &[0x0f]].concat();
      let target = [prefix.as_slice(), &[0x00, 0x20]].concat();
      let child = [target.as_slice(), &[0x30]].concat();
      entries.insert(marker, vec![0x99; survivor_bytes]);
      entries.insert(target.clone(), vec![0x55; 33]);
      entries.insert(child, vec![0x77; survivor_bytes]);
      deleted.push(target);
    }
    let nested_prefix = vec![0x20; 16];
    let nested_marker = [nested_prefix.as_slice(), &[0x0f]].concat();
    let outer = [nested_prefix.as_slice(), &[0x00, 0x20]].concat();
    let inner = [outer.as_slice(), &[0x30]].concat();
    entries.insert(nested_marker, vec![0x99; survivor_bytes]);
    entries.insert(outer.clone(), vec![0x55; 33]);
    entries.insert(inner.clone(), vec![0x77; 33]);
    deleted.push(outer);
    deleted.push(inner);
    let mut database = MemoryDB::<Blake2Hasher>::default();
    let root = Layout::trie_build(&mut database, entries.clone());
    assert_eq!(root, Layout::trie_root(entries.clone()));
    let mut recorder = Recorder::<Layout>::new();
    {
      let trie = TrieDBBuilder::<Layout>::new(&database, &root)
        .with_recorder(&mut recorder)
        .build();
      for key in &deleted {
        assert!(trie.get(key).unwrap().is_some());
      }
    }
    let read_nodes = recorder
      .drain()
      .into_iter()
      .map(|record| record.data)
      .collect::<std::collections::BTreeSet<_>>();
    let delta = deleted
      .iter(/* deos-bypass: bounded-iter -- six fixed synthetic deletions. */)
      .map(|key| (key.clone(), None::<Vec<u8>>))
      .collect::<Vec<_>>();
    let read_only = StorageProof::new(read_nodes.iter().cloned());
    let mut read_only_db = read_only.into_memory_db::<Blake2Hasher>();
    let read_only_result =
      delta_trie_root::<Layout, _, _, _, _, _>(&mut read_only_db, root, delta.clone(), None, None);
    let mut mutation_recorder = Recorder::<Layout>::new();
    let expected_post = delta_trie_root::<Layout, _, _, _, _, _>(
      &mut database,
      root,
      delta.clone(),
      Some(&mut mutation_recorder),
      None,
    )
    .unwrap();
    let extras = mutation_recorder
      .drain()
      .into_iter()
      .map(|record| record.data)
      .filter(|node| !read_nodes.contains(node))
      .collect::<std::collections::BTreeSet<_>>();
    let mut after = entries.clone();
    for key in &deleted {
      after.remove(key);
    }
    assert_eq!(expected_post, Layout::trie_root(after.clone()));
    let batch_proof = StorageProof::new(read_nodes.iter().cloned().chain(extras.iter().cloned()));
    let mut batch_db = batch_proof.into_memory_db::<Blake2Hasher>();
    let replayed =
      delta_trie_root::<Layout, _, _, _, _, _>(&mut batch_db, root, delta, None, None).unwrap();
    assert_eq!(replayed, expected_post);
    let view = TrieDBBuilder::<Layout>::new(&database, &expected_post).build();
    for key in &deleted {
      assert_eq!(view.get(key).unwrap(), None);
    }
    let extras_bytes = extras.iter().map(Vec::len).sum::<usize>();
    eprintln!(
      "multi_delete_batch deletions={} survivor_bytes={survivor_bytes} read_only_replay_ok={} extras_nodes={} extras_bytes={extras_bytes} ceiling_nodes={} scope=finite_synthetic_shapes_not_tariff",
      deleted.len(),
      read_only_result.is_ok(),
      extras.len(),
      deleted.len()
    );
    assert!(
      extras.len() <= deleted.len(),
      "each deleted key may need at most one off-path repair node"
    );
    if survivor_bytes < 32 {
      assert!(
        read_only_result.is_ok(),
        "an inlined sibling needs no read beyond the recorded lookup proof"
      );
    } else {
      assert!(
        !read_only_result.is_ok() || !extras.is_empty(),
        "an out-of-line sibling needs its body from outside the recorded lookup proof"
      );
    }
  }
}

#[test]
fn runtime_mixed_delta_keeps_original_paths_and_delete_repair() {
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_trie::{
      LayoutV1, MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder,
      TrieLayout, delta_trie_root,
    },
  };
  type Layout = LayoutV1<Blake2Hasher>;
  assert!(!Layout::USE_EXTENSION);
  assert_eq!(Layout::MAX_INLINE_VALUE, Some(33));
  // Fixed-width synthetic keys, not a Governance history. The last-byte fork has
  // an out-of-line surviving leaf; another namespace supplies an independent overwrite.
  let key = |namespace: u8, tail: u8| [vec![namespace; 32], vec![0; 19], vec![tail]].concat();
  let deleted = key(0x11, 0x20);
  let survivor = key(0x11, 0x30);
  let inserted = key(0x11, 0x40);
  let updated = key(0x22, 0x60);
  let entries = std::collections::BTreeMap::from([
    (deleted.clone(), vec![0x55; 33]),
    (survivor.clone(), vec![0x77; 33]),
    (updated.clone(), vec![0x99; 33]),
  ]);
  // The SDK sorts this delta: insertion supplied first does not precede deletion
  // during trie mutation. Caller execution order cannot eliminate repair ownership.
  let delta = vec![
    (inserted.clone(), Some(vec![0x44; 33])),
    (updated.clone(), Some(vec![0x88; 33])),
    (deleted.clone(), None),
  ];
  assert!(deleted < inserted);
  assert!(
    entries
      .keys()
      .chain(delta.iter().map(|(key, _)| key))
      .all(|key| key.len() == 52)
  );
  let mut database = MemoryDB::<Blake2Hasher>::default();
  let root = Layout::trie_build(&mut database, entries.clone());
  assert_eq!(root, Layout::trie_root(entries.clone()));
  let mut recorder = Recorder::<Layout>::new();
  {
    let trie = TrieDBBuilder::<Layout>::new(&database, &root)
      .with_recorder(&mut recorder)
      .build();
    for (key, _) in &delta {
      assert_eq!(
        trie.get_hash(key).unwrap().is_some(),
        entries.contains_key(key)
      );
    }
  }
  let original_paths = recorder
    .drain()
    .into_iter()
    .map(|record| record.data)
    .collect::<std::collections::BTreeSet<_>>();
  let mut paths_only =
    StorageProof::new(original_paths.iter().cloned()).into_memory_db::<Blake2Hasher>();
  assert!(
    delta_trie_root::<Layout, _, _, _, _, _>(&mut paths_only, root, delta.clone(), None, None)
      .is_err()
  );
  let mut mutation_recorder = Recorder::<Layout>::new();
  let post_root = delta_trie_root::<Layout, _, _, _, _, _>(
    &mut database,
    root,
    delta.clone(),
    Some(&mut mutation_recorder),
    None,
  )
  .unwrap();
  let extras = mutation_recorder
    .drain()
    .into_iter()
    .map(|record| record.data)
    .filter(|node| !original_paths.contains(node))
    .collect::<std::collections::BTreeSet<_>>();
  assert_eq!(
    extras.len(),
    1,
    "this mixed shape still needs its surviving sibling node"
  );
  let mut after = entries.clone();
  for (key, value) in &delta {
    if let Some(value) = value {
      after.insert(key.clone(), value.clone());
    } else {
      after.remove(key);
    }
  }
  assert_eq!(post_root, Layout::trie_root(after));
  let nodes = original_paths
    .iter()
    .cloned()
    .chain(extras.iter().cloned())
    .collect::<std::collections::BTreeSet<_>>();
  assert!(
    !nodes.contains(&entries[&survivor]),
    "repair forwards a value hash, not its external blob"
  );
  let mut replay = StorageProof::new(nodes).into_memory_db::<Blake2Hasher>();
  assert!(
    TrieDBBuilder::<Layout>::new(&replay, &root)
      .build()
      .get(&survivor)
      .is_err()
  );
  assert_eq!(
    delta_trie_root::<Layout, _, _, _, _, _>(&mut replay, root, delta, None, None).unwrap(),
    post_root
  );
  let view = TrieDBBuilder::<Layout>::new(&database, &post_root).build();
  assert_eq!(view.get(&deleted).unwrap(), None);
  assert_eq!(view.get(&survivor).unwrap(), Some(vec![0x77; 33]));
  assert_eq!(view.get(&inserted).unwrap(), Some(vec![0x44; 33]));
  assert_eq!(view.get(&updated).unwrap(), Some(vec![0x88; 33]));
  println!(
    "MIXED_DELTA_REPAIR_INPUT_V1 {}",
    serde_json::json!({
      "keyBytes": 52, "deletes": 1, "inserts": 1, "updates": 1,
      "originalHashPathNodes": original_paths.len(), "extraRepairNodes": extras.len(),
      "extraNodeBytes": extras.first().unwrap().len(),
      "inputInsertBeforeDelete": true, "oldSurvivorValueUnavailable": true,
      "rootReplayMatchesIndependentRebuild": true,
      "syntheticNotRuntimeHistory": true, "notGeneralMixedDeltaBoundOrPrice": true
    })
  );
}

#[test]
fn runtime_deadline_index_page_domain_bounds_partial_prefix_proof() {
  use codec::{Encode, MaxEncodedLen};
  use pallet_deos_actors::{DeadlineIndexPageOf, DeadlineIndexPages, WakeupClock, WakeupKey};
  use polkadot_sdk::{
    sp_core::Blake2Hasher,
    sp_trie::{
      LayoutV1, MemoryDB, Recorder, StorageProof, Trie, TrieConfiguration, TrieDBBuilder,
      TrieDBMutBuilder, TrieLayout, TrieMut, delta_trie_root,
    },
  };
  type Layout = LayoutV1<Blake2Hasher>;
  type Pages = DeadlineIndexPages<crate::Runtime>;
  type Page = DeadlineIndexPageOf<crate::Runtime>;

  assert!(!Layout::USE_EXTENSION);
  assert_eq!(Layout::MAX_INLINE_VALUE, Some(33));
  let actors = <crate::Runtime as pallet_deos_actors::Config>::MaxActiveActors::get();
  assert_eq!(actors, 10_000);
  // Insertion checks the cap before writing index len. Removal and repair only use indices
  // below the coherent clock-local length. The page ID is u64::from(index / 32).
  let pages = u64::from(actors.div_ceil(32));
  assert_eq!(pages, 313);
  let prefix_len = 32 + 16 + WakeupClock::max_encoded_len();
  assert_eq!(prefix_len, 49);
  let block = Pages::hashed_key_for(WakeupClock::Block, 0);
  let tick = Pages::hashed_key_for(WakeupClock::Tick, 0);
  assert_ne!(&block[..prefix_len], &tick[..prefix_len]);
  let mut keys = (0..pages)
    .map(|page| {
      let block_key = Pages::hashed_key_for(WakeupClock::Block, page);
      let tick_key = Pages::hashed_key_for(WakeupClock::Tick, page);
      assert_eq!((block_key.len(), tick_key.len()), (73, 73));
      assert_eq!(&block_key[..prefix_len], &block[..prefix_len]);
      assert_eq!(&tick_key[..prefix_len], &tick[..prefix_len]);
      assert_eq!(&block_key[prefix_len..], &tick_key[prefix_len..]);
      <[u8; 24]>::try_from(&tick_key[prefix_len..]).unwrap()
    })
    .collect::<Vec<_>>();
  keys.sort_unstable();
  assert!(keys.windows(2).all(|pair| pair[0] != pair[1]));
  let costs = fixed_u64_key_branch_union(&keys, 0, 63);
  assert_eq!(costs.len(), 64);
  assert!(costs.windows(2).all(|pair| pair[0] <= pair[1]));
  let image_hash = polkadot_sdk::sp_io::hashing::sha2_256(&keys.encode());
  assert_eq!(
    image_hash,
    [
      0xc1, 0x52, 0xe3, 0x7e, 0x51, 0xb0, 0x67, 0x53, 0xf3, 0xcf, 0x18, 0x7c, 0x58, 0xdd, 0xa2,
      0xa6, 0x55, 0xde, 0x69, 0xc7, 0x49, 0x69, 0x90, 0xde, 0x6c, 0x39, 0x54, 0x8b, 0x96, 0x15,
      0x12, 0xe5,
    ],
    "the Deadline index page hash domain needs requalification",
  );
  assert_eq!((costs[4], costs[18], costs[63]), (2_948, 9_025, 12_805));
  eprintln!(
    "Deadline index page domain: pages={pages} image_sha256={image_hash:?} branch_curve={costs:?}"
  );
  let raw_value = Page::max_encoded_len();
  let value_blob = raw_value + codec::Compact(raw_value as u32).encoded_size();
  assert_eq!((raw_value, value_blob), (289, 291));
  // Partial bytes sum to at most 24 on each selected suffix path. Each terminal adds at
  // most 37 framing/hash bytes and one max-sized value blob. A missing protected prefix
  // can load one foreign witness; namespace/clock ancestors and net deletion witnesses
  // remain separately charged by the caller. Aliasing two clocks never enlarges this sum.
  for queries in [4, 18] {
    let finite_read = costs[queries] + queries * (61 + value_blob) + 1284;
    let coarse_read = queries * (48 * 650 + 1284 + value_blob);
    assert!(finite_read < coarse_read);
    assert_eq!(
      (finite_read, coarse_read - finite_read),
      if queries == 4 {
        (5_640, 125_460)
      } else {
        (16_645, 573_305)
      },
      "the finite Deadline index page read bound needs requalification",
    );
    eprintln!(
      "Deadline index page reads: q={queries} branch={} finite={finite_read} coarse={coarse_read} saving={}",
      costs[queries],
      coarse_read - finite_read
    );
  }

  // Typed V1 trie primitives validate read/absence and net-root replay against the model.
  // Full/sparse/empty page maps are not complete admitted Actor states or a reachability proof.
  let value = |page: u64| {
    Page::try_from(
      (0..32)
        .map(|slot| WakeupKey::Tick(page * 32 + slot))
        .collect::<Vec<_>>(),
    )
    .unwrap()
    .encode()
  };
  assert_eq!(value(0).len(), raw_value);
  for retained_stride in [1, 17, 0] {
    let mut before = (0..pages)
      .filter(|page| retained_stride != 0 && page % retained_stride == 0)
      .map(|page| (Pages::hashed_key_for(WakeupClock::Tick, page), value(page)))
      .collect::<std::collections::BTreeMap<_, _>>();
    // Preserve a different protected clock even when the queried clock has no pages.
    before.insert(block.clone(), value(pages));
    let mut database = MemoryDB::<Blake2Hasher>::default();
    let mut root = Default::default();
    {
      let mut trie = TrieDBMutBuilder::<Layout>::new(&mut database, &mut root).build();
      for (key, value) in &before {
        trie.insert(key, value).unwrap();
      }
    }
    let mut recorder = Recorder::<Layout>::new();
    let queries = 18;
    {
      let trie = TrieDBBuilder::<Layout>::new(&database, &root)
        .with_recorder(&mut recorder)
        .build();
      for page in 0..queries as u64 {
        let key = Pages::hashed_key_for(WakeupClock::Tick, page);
        assert_eq!(trie.get(&key).unwrap(), before.get(&key).cloned());
      }
    }
    let read_proof = StorageProof::new(recorder.drain().into_iter().map(|record| record.data));
    let read_bound = (65 + 34) * 650 + costs[queries] + queries * (61 + value_blob) + 1284 + 5;
    assert!(read_proof.encoded_size() <= read_bound);
    let mut after = before.clone();
    let delta = (0..4u64)
      .map(|page| {
        let key = Pages::hashed_key_for(WakeupClock::Tick, page);
        let new_value = (page == 3).then(|| value(pages + page));
        if let Some(value) = &new_value {
          after.insert(key.clone(), value.clone());
        } else {
          after.remove(&key);
        }
        (key, new_value)
      })
      .collect::<Vec<_>>();
    let post_root = delta_trie_root::<Layout, _, _, _, _, _>(
      &mut database,
      root,
      delta.clone(),
      Some(&mut recorder),
      None,
    )
    .unwrap();
    assert_eq!(post_root, Layout::trie_root(after));
    let proof = StorageProof::new(
      read_proof
        .iter_nodes()
        .cloned()
        .chain(recorder.drain().into_iter().map(|record| record.data)),
    );
    let proof_bytes = proof.encoded_size();
    assert!(proof_bytes <= read_bound + 3 * 1284);
    let mut replay = proof.into_memory_db::<Blake2Hasher>();
    assert_eq!(
      delta_trie_root::<Layout, _, _, _, _, _>(&mut replay, root, delta, None, None).unwrap(),
      post_root,
    );
    eprintln!(
      "Deadline index page primitive: stride={retained_stride} read={} read_root={} bound={}",
      read_proof.encoded_size(),
      proof_bytes,
      read_bound + 3 * 1284
    );
  }
}

#[cfg(feature = "try-runtime")]
#[test]
fn router_try_state_reconciles_lp_pairs_with_pool_and_asset_truth() {
  use super::common::{ALICE, create_pool, seeded_test_ext};
  use crate::{DeosRouter, Runtime, RuntimeOrigin, configs::AssetKind};
  use polkadot_sdk::frame_support::{assert_ok, traits::Hooks};
  use polkadot_sdk::pallet_asset_conversion::PoolLocator;

  seeded_test_ext().execute_with(|| {
    let pair = (
      AssetKind::Native,
      AssetKind::Local(primitives::ecosystem::protocol_tokens::BLDR_ASSET_ID),
    );
    assert_ok!(create_pool(RuntimeOrigin::signed(ALICE), pair.0, pair.1,));
    let pool_id = <Runtime as polkadot_sdk::pallet_asset_conversion::Config>::PoolLocator::pool_id(
      &pair.0, &pair.1,
    )
    .expect("fixture pool identity resolves");
    let pool = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get(pool_id)
      .expect("fixture pool exists");
    assert_ok!(<DeosRouter as Hooks<crate::BlockNumber>>::try_state(1));

    pallet_deos_router::LpPairByTokenId::<Runtime>::mutate(|pairs| {
      pairs.remove(&pool.lp_token);
    });
    assert!(<DeosRouter as Hooks<crate::BlockNumber>>::try_state(1).is_err());
    assert_ok!(crate::configs::assets_config::register_pool_lp_pair(
      pair.0, pair.1,
    ));

    polkadot_sdk::pallet_assets::Asset::<Runtime>::remove(pool.lp_token);
    assert!(<DeosRouter as Hooks<crate::BlockNumber>>::try_state(1).is_err());
  });
}

#[test]
fn parked_construction_declared_service_round_fits_static_prepass_budget() {
  use crate::{Actors, Runtime, configs::AssetKind};
  use pallet_deos_actors::{
    ActorAdmissionCertificate, ActorContractHeads, ActorContractTailChunks, ActorSemanticState,
    ActorSemanticStates, ActorStepResourceEnvelope, ActorType,
    AdmissionCertificateAuthorityProvider, AmountResolution, OnUnfunded, StepControlExecution,
    StepControlOutcome, StepControlPhase, StepControlPlacement, StepControlWeightContext,
    StepControlWeightProvider, StepErrorPolicy, StepOf, Task, TaskEffectExecution,
    TaskEffectWeightProvider, Trigger, WeightInfo,
  };
  use polkadot_sdk::{frame_support::weights::Weight, sp_io::TestExternalities};

  type Weights = <Runtime as pallet_deos_actors::Config>::WeightInfo;
  type Control = <Runtime as pallet_deos_actors::Config>::StepControlWeight;
  type Effect = <Runtime as pallet_deos_actors::Config>::TaskEffectWeight;
  type Authority = <Runtime as pallet_deos_actors::Config>::AdmissionCertificateAuthority;
  let authority = Authority::current().expect("fixed reference authority is available");
  let selector = Weights::service_round_begin_populated()
    .checked_add(&Weights::service_round_probe_eligible())
    .unwrap();
  let suffix = Weights::service_round_admit_eligible()
    .max(Weights::service_member_retire_interior())
    .max(Weights::service_member_retire_pair_cursor())
    .max(Weights::service_member_retire_singleton());
  let per_member = selector
    .checked_add(&Weights::scheduler_actor_state_probe())
    .and_then(|weight| weight.checked_add(&Actors::close_dispatch_weight_upper()))
    .and_then(|weight| weight.checked_add(&suffix))
    .unwrap();
  let budget = crate::configs::BlockResourceBudgetValue::get();
  let mandatory = Weights::scheduler_on_initialize_cutoff()
    .checked_add(&Actors::deadline_service_weight_upper())
    .and_then(|weight| weight.checked_add(&Weights::dependency_scan_source_probe()))
    .and_then(|weight| {
      weight.checked_add(
        &Weights::process_dependency_scan_unit()
          .max(Weights::process_dependency_scan_completion_unit()),
      )
    })
    .and_then(|weight| weight.checked_add(&Weights::scheduler_on_idle_base()))
    .and_then(|weight| weight.checked_add(&Weights::block_resource_finalize()))
    .unwrap();
  let available_control = budget
    .limits()
    .actor_control()
    .checked_sub(&mandatory)
    .unwrap();
  let available_effect = budget.limits().actor_base_turn();
  assert!(<Runtime as pallet_deos_actors::Config>::MaxExecutionsPerBlock::get() >= 7);
  assert!(<Runtime as pallet_deos_actors::Config>::MaxQueueEntriesScannedPerBlock::get() >= 7);
  let producer = StepOf::<Runtime> {
    precondition: None,
    task: Task::Transfer {
      to: super::common::ALICE,
      asset: AssetKind::Local(1),
      amount: AmountResolution::Fixed(1),
    },
    on_error: StepErrorPolicy::AbortCycle,
  };
  let producer_resources = ActorStepResourceEnvelope {
    control: Control::maximum_control_weight(
      StepControlWeightContext {
        cursor: 0,
        steps_in_fragment: 1,
        opening_tail_chunks: 0,
        predicate_evaluation_units: 0,
      },
      &producer,
    )
    .unwrap(),
    effect: Effect::maximum_effect_weight(&producer.task).unwrap(),
  };
  let check_current_control = |step: &StepOf<Runtime>,
                               resources: ActorStepResourceEnvelope,
                               cursor,
                               step_count: u32,
                               invoked,
                               fee_collected| {
    let actual = Control::actual_control_weight(
      StepControlWeightContext {
        cursor,
        steps_in_fragment: 1,
        opening_tail_chunks: if cursor == 0 {
          step_count.saturating_sub(1)
        } else {
          0
        },
        predicate_evaluation_units: step
          .precondition
          .as_ref()
          .map_or(0, |p| p.evaluation_units()),
      },
      step,
      resources.control,
      StepControlExecution {
        phase: if cursor == 0 {
          StepControlPhase::Opening
        } else {
          StepControlPhase::Running
        },
        outcome: if cursor + 1 == step_count {
          StepControlOutcome::Completed
        } else {
          StepControlOutcome::Continued
        },
        placement: if cursor + 1 == step_count {
          StepControlPlacement::None
        } else {
          StepControlPlacement::Queue
        },
        task_effect: if invoked {
          TaskEffectExecution::Invoked
        } else {
          TaskEffectExecution::NotInvoked
        },
        action_fee_collected: fee_collected,
      },
    )
    .expect("fixed reference current Step has a supported Control model");
    assert!(actual.all_lte(resources.control));
  };
  check_current_control(&producer, producer_resources, 0, 1, true, true);
  for preset in crate::genesis_config_presets::preset_names() {
    TestExternalities::new(super::production_block_replay::preset_genesis_storage(&[], &preset))
      .execute_with(|| {
        let mut round_control = selector;
        let mut round_effect = Weight::zero();
        let mut systems = 0;
        let mut temporal_systems = 0;
        for (actor, head) in ActorContractHeads::<Runtime>::iter() {
          let state = Actors::active_actor_state(actor).unwrap();
          let Some(ActorSemanticState::Active(semantic)) = ActorSemanticStates::<Runtime>::get(actor)
          else {
            panic!("genesis Service owner must retain its active semantic authority");
          };
          let expected = ActorAdmissionCertificate::new(
            state.contract.semantic_contract_id(),
            state.contract.body_commitment().unwrap(),
            state.contract.trigger.wake_qualification(&state.contract.window),
            authority.runtime_actor_semantics_version,
            authority.production_weight_identity,
            authority.body_geometry_version,
            authority.configured_bounds_commitment,
            authority.maximum_lifecycle_weight,
          );
          assert_eq!(semantic.admission, expected);
          assert_eq!(head.header.admission_identity, expected.admission_identity);
          assert_eq!(state.identity.actor_class.actor_type(), ActorType::System);
          assert!(!head.header.has_close_when);
          assert_eq!(head.header.on_unfunded, OnUnfunded::Close);
          assert!(head.header.parked_balance_activation.is_none());
          assert!(head.header.auto_close_at_cycle_nonce.is_none());
          match head.header.trigger {
            Trigger::Cadenced { every_ticks } => {
              assert!(every_ticks > 0);
              temporal_systems += 1;
            }
            Trigger::Manual | Trigger::AddressEvent { .. } => {}
            Trigger::AtTime { .. } => panic!("construction has no genesis AtTime owner"),
          }
          let resources = head
            .first_step_resources
            .into_iter()
            .chain(
              ActorContractTailChunks::<Runtime>::iter_prefix(actor)
                .flat_map(|(_, chunk)| chunk.step_resources.into_iter()),
            )
            .collect::<Vec<_>>();
          assert_eq!(resources.len(), head.header.step_count as usize);
          assert!((1..=2).contains(&head.header.step_count));
          for (cursor, (step, resource)) in state.contract.steps.iter().zip(&resources).enumerate() {
            for invoked in [false, true] {
              check_current_control(step, *resource, cursor as u32, head.header.step_count, invoked, false);
            }
          }
          let maximum = resources.iter().fold(
            ActorStepResourceEnvelope {
              control: Weight::zero(),
              effect: Weight::zero(),
            },
            |maximum, next| ActorStepResourceEnvelope {
              control: maximum.control.max(next.control),
              effect: maximum.effect.max(next.effect),
            },
          );
          round_control = round_control
            .checked_add(&per_member)
            .and_then(|weight| weight.checked_add(&maximum.control))
            .unwrap();
          round_effect = round_effect.checked_add(&maximum.effect).unwrap();
          systems += 1;
        }
        assert_eq!(systems, 3);
        assert_eq!(temporal_systems, 1);
        round_control = round_control
          .checked_add(
            &per_member
              .checked_add(&producer_resources.control)
              .unwrap()
              .saturating_mul(4),
          )
          .unwrap();
        round_effect = round_effect
          .checked_add(&producer_resources.effect.saturating_mul(4))
          .unwrap();
        println!(
          "{preset}: declared seven-member round Control {round_control:?} / {available_control:?}; effect {round_effect:?} / {available_effect:?}; mandatory {mandatory:?}; current genesis authorities {systems}"
        );
        // Current declarations only: complete physical owners and temporal latency remain open.
        assert!(round_control.all_lte(available_control));
        assert!(round_effect.all_lte(available_effect));
      });
  }
}
