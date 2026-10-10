//! Finite encoding witnesses and structural bounds for the selected constructor.

use codec::Encode;
use cumulus_primitives_parachain_inherent::{INHERENT_IDENTIFIER, ParachainInherentData};
use deos_runtime::{Block, Header, Runtime, RuntimeCall};
use polkadot_sdk::{
  cumulus_primitives_core::{relay_chain, rpsr_digest},
  sp_block_builder::runtime_decl_for_block_builder::BlockBuilder,
  sp_core::{Blake2Hasher, storage::StateVersion},
  sp_runtime::{Digest, DigestItem, traits::Header as _},
  sp_trie::{LayoutV1, read_trie_value},
};
use serde_json::{Value, json};

pub fn input() -> Value {
  let mut input = constructor_input(false);
  input["sealedParent"] = constructor_input(true);
  input
}

fn constructor_input(sealed: bool) -> Value {
  let keys = [
    relay_chain::well_known_keys::ACTIVE_CONFIG.to_vec(),
    relay_chain::well_known_keys::para_head(deos_runtime::PARACHAIN_ID.into()),
    relay_chain::well_known_keys::EPOCH_INDEX.to_vec(),
    relay_chain::well_known_keys::ONE_EPOCH_AGO_RANDOMNESS.to_vec(),
    relay_chain::well_known_keys::CURRENT_SLOT.to_vec(),
    relay_chain::well_known_keys::AUTHORITIES.to_vec(),
    relay_chain::well_known_keys::NEXT_AUTHORITIES.to_vec(),
  ];
  assert!(
    keys.iter(/* deos-bypass: bounded-iter -- seven fixed constructor keys */).all(|key| key.len() <= 64)
  );
  assert_eq!(StateVersion::default(), StateVersion::V1);
  assert!(u64::from(u32::MAX) * deos_runtime::SLOT_DURATION < 1u64 << 48);
  let header_upper = 3 * 32
    + 5
    + 1
    + (1 + 4 + 1 + 8)
    + (1 + 4 + 1 + 32 + 5)
    + if sealed { 1 + 4 + 2 + 64 } else { 0 };
  let value_upper = header_upper + 2;
  // A radix trie for seven values has at most thirteen ordinary nodes and seven
  // external values. References are at most a compact-framed 32-byte hash.
  let proof_nodes_upper = 2 * keys.len() - 1 + keys.len();
  let node_upper = 2 + 64 + 2 + 16 * 33 + 2 + value_upper;
  let proof_upper = 1 + proof_nodes_upper * (node_upper + 2);
  let validation_upper = value_upper + 4 + 32 + 4;
  let parachain_call_upper = 2 + validation_upper + proof_upper + 2 + 4;
  let parachain_extrinsic_upper = codec::Compact(u32::try_from(parachain_call_upper + 1).unwrap())
    .encoded_size()
    + 1
    + parachain_call_upper;
  let timestamp_extrinsic_upper = 1 + 1 + 2 + 7;
  let prepass_extrinsic_upper = 1 + 1 + 2;
  let total_upper = parachain_extrinsic_upper + timestamp_extrinsic_upper + prepass_extrinsic_upper;
  assert!(parachain_call_upper + 1 < 1 << 16);
  assert_eq!(
    (header_upper, node_upper, proof_upper, total_upper),
    if sealed {
      (230, 830, 16641, 16941)
    } else {
      (159, 759, 15221, 15448)
    }
  );
  let mut witnesses = Vec::new();
  for number in [
    1,
    63,
    64,
    16_383,
    16_384,
    (1 << 30) - 1,
    1 << 30,
    2_552_520_738,
    u32::MAX,
  ] {
    for logs in 0..=2 {
      if sealed && logs != 2 {
        continue;
      }
      let mut digest = Digest::default();
      if logs > 0 {
        digest.logs.push(DigestItem::PreRuntime(
          polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
          u64::from(number - 1).encode(),
        ));
      }
      if logs > 1 {
        digest
          .logs
          .push(rpsr_digest::relay_parent_storage_root_item(
            Default::default(),
            number - 1,
          ));
      }
      if sealed {
        use polkadot_sdk::sp_consensus_aura::{
          digests::CompatibleDigestItem, sr25519::AuthoritySignature,
        };
        digest
          .logs
          .push(DigestItem::aura_seal(AuthoritySignature::from(
            polkadot_sdk::sp_core::sr25519::Signature::from_raw([23; 64]),
          )));
      }
      let parent = Header::new(
        number - 1,
        Default::default(),
        Default::default(),
        Default::default(),
        digest,
      );
      assert!(parent.encoded_size() <= header_upper);
      let data = super::inherent_data::inherents(&parent, number);
      let para: ParachainInherentData = data.get_data(&INHERENT_IDENTIFIER).unwrap().unwrap();
      assert!(para.validation_data.encoded_size() <= validation_upper);
      let proof = &para.relay_chain_state;
      let proof_nodes = proof.iter_nodes().count();
      assert!(proof_nodes <= proof_nodes_upper && proof.encoded_size() <= proof_upper);
      assert!(proof.iter_nodes().all(|node| node.len() <= node_upper)); // deos-bypass: bounded-iter -- at most twenty constructor proof nodes, checked above
      let db = proof.to_memory_db::<Blake2Hasher>();
      for key in &keys {
        let value = read_trie_value::<LayoutV1<Blake2Hasher>, _>(
          &db,
          &para.validation_data.relay_parent_storage_root,
          key,
          None,
          None,
        )
        .unwrap()
        .unwrap();
        assert!(value.len() <= value_upper);
      }
      let extrinsics = <Runtime as BlockBuilder<Block>>::inherent_extrinsics(data);
      assert_eq!(extrinsics.len(), 3);
      let lengths = extrinsics
        .iter(/* deos-bypass: bounded-iter -- exactly three declared inherents, checked above */)
        .map(Encode::encoded_size)
        .collect::<Vec<_>>();
      assert!(matches!(
        extrinsics[0].function,
        RuntimeCall::ParachainSystem(_)
      ));
      assert!(matches!(extrinsics[1].function, RuntimeCall::Timestamp(_)));
      assert!(matches!(
        extrinsics[2].function,
        RuntimeCall::Actors(pallet_deos_actors::Call::actor_prepass {})
      ));
      assert!(
        lengths[0] <= parachain_extrinsic_upper
          && lengths[1] <= timestamp_extrinsic_upper
          && lengths[2] <= prepass_extrinsic_upper
      );
      let total = lengths.iter().sum::<usize>(); // deos-bypass: bounded-iter -- exactly three lengths
      assert!(total <= total_upper);
      witnesses.push(json!({"number": number, "parentDigestLogs": logs,
        "parentHeaderBytes": parent.encoded_size(), "relayProofNodes": proof_nodes,
        "relayProofEncodedBytes": proof.encoded_size(), "encodedExtrinsicBytes": lengths,
        "totalEncodedBytes": total}));
    }
  }
  json!({"scope": "source_bounded_selected_constructor_encoding",
    "keyCount": keys.len(), "maximumKeyBytes": keys.iter().map(Vec::len).max().unwrap(), // deos-bypass: bounded-iter -- seven fixed constructor keys
    "headerUpperBytes": header_upper, "maximumValueBytes": value_upper,
    "relayProofNodeUpper": proof_nodes_upper, "relayNodePayloadUpperBytes": node_upper,
    "relayProofEncodedUpperBytes": proof_upper, "extrinsicUpperBytes": [parachain_extrinsic_upper, timestamp_extrinsic_upper, prepass_extrinsic_upper],
    "totalEncodedUpperBytes": total_upper, "witnesses": witnesses,
    "premises": ["Unchanged selected constructor/default seven-key relay builder, para ID and 6000ms native slot", if sealed { "Parent digest has one eight-byte Aura pre-runtime, one RPSR root/u32 digest and one 64-byte Aura seal; codec signatures are not authenticated" } else { "Parent digest is empty or has one eight-byte Aura pre-runtime and one RPSR root/u32 digest" }, "Empty messages/descendants, absent peer and fixed default relay authority/configuration", "Timestamp input is monotone and at least existing Now; all numbers are u32", "This creates and encodes inherents only; it neither applies a block nor certifies public chronology or physical Weight"]})
}
