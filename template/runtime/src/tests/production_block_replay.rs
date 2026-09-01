//! Full Executive block fixtures for production-Wasm replay.
//!
//! Unlike the Actor-only capacity harness, this includes consensus context,
//! all runtime hooks, inherent order, extrinsics roots and finalization.

use crate::{Block, Executive, Header, InherentDataExt, RuntimeGenesisConfig};
use codec::Encode;
use cumulus_primitives_parachain_inherent::{INHERENT_IDENTIFIER, ParachainInherentData};
use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
use polkadot_sdk::{
  cumulus_primitives_core::PersistedValidationData,
  polkadot_parachain_primitives::primitives::HeadData,
  sp_core::storage::{Storage, well_known_keys},
  sp_inherents::InherentData,
  sp_io::TestExternalities,
  sp_runtime::{
    BuildStorage, Digest, DigestItem,
    traits::{BlakeTwo256, Hash, Header as HeaderT},
  },
};

fn merge_genesis_patch(target: &mut serde_json::Value, patch: serde_json::Value) {
  match (target, patch) {
    (serde_json::Value::Object(target), serde_json::Value::Object(patch)) => {
      for (key, value) in patch {
        merge_genesis_patch(target.entry(key).or_insert(serde_json::Value::Null), value);
      }
    }
    (target, patch) => *target = patch,
  }
}

fn reference_genesis_storage(wasm: &[u8]) -> Storage {
  let preset = crate::genesis_config_presets::get_preset(
    &polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into(),
  )
  .expect("Development is a declared runtime preset");
  let mut config = serde_json::to_value(RuntimeGenesisConfig::default())
    .expect("default runtime genesis serializes");
  merge_genesis_patch(
    &mut config,
    serde_json::from_slice(&preset).expect("runtime-owned preset is JSON"),
  );
  let config: RuntimeGenesisConfig =
    serde_json::from_value(config).expect("complete runtime preset deserializes");
  let mut storage = config.build_storage().expect("reference genesis builds");
  storage
    .top
    .insert(well_known_keys::CODE.to_vec(), wasm.to_vec());
  storage
}

/// Author a first block from the complete reference preset, without synthetic
/// timestamp/validation/resource writes or direct Actor-hook invocation.
fn author_reference_first_block(wasm: &[u8]) -> (Storage, Block, InherentData) {
  let storage = reference_genesis_storage(wasm);
  let mut ext = TestExternalities::new_with_code_and_state(
    wasm,
    storage.clone(),
    crate::VERSION.state_version(),
  );
  let genesis_root = *ext.as_backend().root();
  let genesis_header = Header::new(
    0,
    BlakeTwo256::ordered_trie_root(Vec::new(), crate::VERSION.extrinsics_root_state_version()),
    genesis_root,
    Default::default(),
    Digest::default(),
  );
  let parent_head = HeadData(genesis_header.encode());
  let (block, inherent_data) = ext.execute_with(|| {
    let header = Header::new(
      1,
      Default::default(),
      Default::default(),
      genesis_header.hash(),
      Digest {
        logs: vec![DigestItem::PreRuntime(
          polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
          1u64.encode(),
        )],
      },
    );
    Executive::initialize_block(&header);
    let proof_builder = RelayStateSproofBuilder {
      para_id: crate::PARACHAIN_ID.into(),
      current_slot: 1u64.into(),
      included_para_head: Some(parent_head.clone()),
      ..Default::default()
    };
    let (relay_parent_storage_root, relay_chain_state) = proof_builder.into_state_root_and_proof();
    let mut data = InherentData::new();
    data
      .put_data(
        polkadot_sdk::sp_timestamp::INHERENT_IDENTIFIER,
        &crate::SLOT_DURATION,
      )
      .expect("timestamp inherent encodes");
    data
      .put_data(
        INHERENT_IDENTIFIER,
        &ParachainInherentData {
          validation_data: PersistedValidationData {
            parent_head,
            relay_parent_number: 1,
            relay_parent_storage_root,
            max_pov_size: 5_000_000,
          },
          relay_chain_state,
          downward_messages: Default::default(),
          horizontal_messages: Default::default(),
          relay_parent_descendants: Default::default(),
          collator_peer_id: None,
        },
      )
      .expect("parachain inherent encodes");
    pallet_deos_actors::provide_actor_prepass_inherent_data(&mut data)
      .expect("Actor Prepass inherent encodes");
    let extrinsics = data.create_extrinsics();
    for extrinsic in &extrinsics {
      Executive::apply_extrinsic(extrinsic.clone())
        .expect("inherent is valid")
        .expect("inherent dispatch succeeds");
    }
    (
      Block {
        header: Executive::finalize_block(),
        extrinsics,
      },
      data,
    )
  });
  (storage, block, inherent_data)
}

#[test]
fn reference_full_block_fixture_replays_through_executive() {
  let (storage, block, data) = author_reference_first_block(&[]);
  let mut replay =
    TestExternalities::new_with_code_and_state(&[], storage, crate::VERSION.state_version());
  replay.execute_with(|| {
    crate::apis::validate_context_inherent_geometry(&data)
      .expect("full block context geometry is bounded");
    assert!(data.check_extrinsics(&block.clone().into()).ok());
    Executive::execute_block(block.into());
  });
}

#[test]
#[ignore = "requires exact current production Wasm via DEOS_PRODUCTION_WASM"]
fn reference_full_block_replays_in_production_wasm_with_verified_storage_proof() {
  let path = std::env::var_os("DEOS_PRODUCTION_WASM")
    .expect("DEOS_PRODUCTION_WASM must explicitly select the current production artifact");
  let wasm = std::fs::read(path).expect("selected production Wasm is readable");
  let (storage, block, data) = author_reference_first_block(&wasm);
  let evidence = super::wasm_replay::replay_block(storage, &wasm, &block.encode(), &data)
    .expect("production Wasm and independent proof-only execution agree");
  assert_eq!(evidence.runtime_version, crate::VERSION);
  assert_eq!(evidence.block_hash, block.header.hash());
  assert_eq!(evidence.post_state_root, *block.header.state_root());
  assert!(evidence.execute_block_result.is_empty());
  for proof in [&evidence.execution_proof, &evidence.verification_proof] {
    assert_eq!(
      proof.storage_proof_scale_bytes,
      proof.storage_proof.encoded_size()
    );
    assert_eq!(
      proof.compact_proof_scale_bytes,
      proof.compact_proof.encoded_size()
    );
    assert_eq!(proof.trie_node_count, proof.storage_proof.len());
    assert!(proof.trie_node_bytes > 0);
  }
  println!(
    "WASM_BLOCK_REPLAY_V1 code={:?} block={:?} preRoot={:?} postRoot={:?} executionStorageProofBytes={} executionCompactProofBytes={} executionNodeBytes={} executionNodes={} verificationStorageProofBytes={} verificationCompactProofBytes={} proofSizeObservations={:?}",
    evidence.runtime_code_hash,
    evidence.block_hash,
    evidence.pre_state_root,
    evidence.post_state_root,
    evidence.execution_proof.storage_proof_scale_bytes,
    evidence.execution_proof.compact_proof_scale_bytes,
    evidence.execution_proof.trie_node_bytes,
    evidence.execution_proof.trie_node_count,
    evidence.verification_proof.storage_proof_scale_bytes,
    evidence.verification_proof.compact_proof_scale_bytes,
    evidence.proof_size_observations,
  );
}
