//! Proof-bound Aura-sealed PVF input from a prepared native block oracle.

use super::pvf_runtime::{self, Block, Executive, Header, Runtime};
use codec::Encode;
use polkadot_sdk::{
  cumulus_primitives_core::ParachainBlockData,
  frame_support::traits::ExecuteBlock,
  polkadot_parachain_primitives::primitives::{HeadData, ValidationParams, ValidationResult},
  sp_core::{Blake2Hasher, Pair, storage::Storage},
  sp_genesis_builder::PresetId,
  sp_inherents::InherentData,
  sp_io::TestExternalities,
  sp_runtime::{DigestItem, traits::Header as HeaderT},
  sp_state_machine::Backend,
  sp_trie::{
    StorageProof,
    proof_size_extension::{ProofSizeExt, RecordingProofSizeProvider},
    recorder::Recorder,
  },
};
use std::{io::Write, path::Path};

pub(super) fn export(
  output: &Path,
  preset: &PresetId,
  (storage, expected, parent, mut block, data): (Storage, Storage, &Header, Block, InherentData),
) -> Result<(), String> {
  use polkadot_sdk::sp_consensus_aura::{
    digests::CompatibleDigestItem,
    sr25519::{AuthorityPair, AuthoritySignature},
  };
  type Executor = polkadot_sdk::cumulus_pallet_aura_ext::BlockExecutor<Runtime, Executive>;
  let code = storage.top[polkadot_sdk::sp_core::storage::well_known_keys::CODE].clone();
  let mut oracle = TestExternalities::new_with_code_and_state(
    &code,
    storage,
    pvf_runtime::VERSION.state_version(),
  );
  let native_seal_reused = block.header.digest().logs.last().is_some_and(|item| {
    <DigestItem as CompatibleDigestItem<AuthoritySignature>>::as_aura_seal(item).is_some()
  });
  if !native_seal_reused {
    let signer = oracle.execute_with(|| {
      let authorities = polkadot_sdk::pallet_aura::Authorities::<Runtime>::get();
      let slot = <DigestItem as CompatibleDigestItem<AuthoritySignature>>::as_aura_pre_digest(
        &block.header.digest().logs[0],
      )
      .unwrap();
      let authority = &authorities[u64::from(slot) as usize % authorities.len()];
      ["//Alice", "//Bob"]
        .into_iter() // deos-bypass: bounded-iter -- two fixed preset authority seeds
        .map(|seed| AuthorityPair::from_string(seed, None).unwrap())
        .find(|pair| &pair.public() == authority)
        .expect("selected preset authority has a known diagnostic key")
    });
    let signature = signer.sign(block.header.hash().as_bytes());
    block
      .header
      .digest_mut()
      .push(DigestItem::aura_seal(signature));
  }
  let seal_recorder = Recorder::<Blake2Hasher>::default();
  let mut verified = block.clone().into();
  oracle.execute_with_recorder(seal_recorder.clone(), || {
    <Executor as ExecuteBlock<Block>>::verify_and_remove_seal(&mut verified);
  });
  let recorder = Recorder::<Blake2Hasher>::default();
  oracle.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
    recorder.clone(),
  )));
  oracle.execute_with_recorder(recorder.clone(), || {
    <Executor as ExecuteBlock<Block>>::execute_verified_block(verified);
  });
  let output_recorder = Recorder::<Blake2Hasher>::default();
  let (validation_data, result) = oracle.execute_with_recorder(output_recorder.clone(), || {
    use polkadot_sdk::cumulus_pallet_parachain_system as parachain;
    let upward_messages = parachain::UpwardMessages::<Runtime>::get();
    let horizontal_messages = parachain::HrmpOutboundMessages::<Runtime>::get();
    let new_validation_code = parachain::NewValidationCode::<Runtime>::get();
    let custom_head = parachain::CustomValidationHeadData::<Runtime>::get();
    let processed_downward_messages = parachain::ProcessedDownwardMessages::<Runtime>::get();
    assert!(upward_messages.is_empty() && horizontal_messages.is_empty());
    assert!(new_validation_code.is_none() && custom_head.is_none());
    assert_eq!(processed_downward_messages, 0);
    (
      parachain::ValidationData::<Runtime>::get().unwrap(),
      ValidationResult {
        head_data: HeadData(block.header.encode()),
        new_validation_code: None,
        upward_messages: Default::default(),
        horizontal_messages: Default::default(),
        processed_downward_messages,
        hrmp_watermark: parachain::HrmpWatermark::<Runtime>::get(),
      },
    )
  });
  oracle.commit_all().unwrap();
  assert_eq!(*oracle.as_backend().root(), *block.header.state_root());
  let actual = oracle
    .backend
    .pairs(Default::default())
    .unwrap()
    .map(Result::unwrap)
    .collect::<std::collections::BTreeMap<_, _>>();
  assert_eq!(
    actual, expected.top,
    "sealed native execution preserves the complete oracle"
  );
  // PVF excludes seal reads from execution proof-size accounting, but needs both sets of nodes.
  let proof = StorageProof::merge([
    seal_recorder.drain_storage_proof(),
    recorder.drain_storage_proof(),
    output_recorder.drain_storage_proof(),
  ])
  .to_compact_proof::<Blake2Hasher>(*parent.state_root())
  .unwrap();
  let inherent = data
    .get_data::<cumulus_primitives_parachain_inherent::ParachainInherentData>(
      &cumulus_primitives_parachain_inherent::INHERENT_IDENTIFIER,
    )
    .unwrap()
    .unwrap();
  let pvd = inherent.validation_data;
  assert_eq!(pvd, validation_data);
  assert_eq!(pvd.parent_head.0, parent.encode());
  let params = ValidationParams {
    parent_head: pvd.parent_head.clone(),
    block_data: polkadot_sdk::polkadot_parachain_primitives::primitives::BlockData(
      ParachainBlockData::new(vec![block.clone()], proof, None).encode(),
    ),
    relay_parent_number: pvd.relay_parent_number,
    relay_parent_storage_root: pvd.relay_parent_storage_root,
  };
  for (name, bytes) in [
    ("params.scale", params.encode()),
    ("pvd.scale", pvd.encode()),
    ("sealed-block.scale", block.encode()),
    ("expected-head.scale", block.header.encode()),
    ("expected-result.scale", result.encode()),
  ] {
    let path = output.join(format!("{preset}.{name}"));
    std::fs::OpenOptions::new()
      .write(true)
      .create_new(true)
      .open(path)
      .and_then(|mut file| file.write_all(&bytes))
      .map_err(|error| format!("cannot export PVF fixture: {error}"))?;
  }
  println!(
    "pvf_fixture preset={preset} aura_seal_verified=true complete_native_state_matched=true native_seal_reused={native_seal_reused} block_data_bytes={} max_pov_bytes={} relay_proof_authentication=false",
    params.block_data.0.len(),
    pvd.max_pov_size
  );
  Ok(())
}
