#[path = "../examples/actor_retention_fixture/wide_close.rs"]
mod wide_close_fixture;

#[path = "fixtures/actor_key_schema.rs"]
mod actor_key_schema;

use deos_runtime::{Block, Runtime};
use polkadot_sdk::sp_genesis_builder::runtime_decl_for_genesis_builder::GenesisBuilder;

#[test]
fn default_library_actor_key_schema_identity() {
  // Metadata evaluates runtime constants; an empty host is enough for this key-only projection.
  polkadot_sdk::sp_io::TestExternalities::default().execute_with(|| {
    let encoded = Runtime::metadata_at_version(16).expect("native metadata v16 exists");
    eprintln!(
      "actor_key_schema domain=default_library identity={:02x?}",
      actor_key_schema::identity(&encoded)
    );
  });
}

#[test]
fn close_hold_host_has_no_custom_account_callbacks() {
  use polkadot_sdk::{frame_system, pallet_balances};
  use std::any::TypeId;

  assert_eq!(
    TypeId::of::<<Runtime as pallet_deos_actors::Config>::StateHoldCurrency>(),
    TypeId::of::<deos_runtime::Balances>(),
  );
  assert_eq!(
    TypeId::of::<<Runtime as pallet_balances::Config>::AccountStore>(),
    TypeId::of::<deos_runtime::System>(),
  );
  assert_eq!(
    TypeId::of::<<Runtime as pallet_balances::Config>::DustRemoval>(),
    TypeId::of::<()>()
  );
  assert_eq!(
    TypeId::of::<<Runtime as pallet_balances::Config>::DoneSlashHandler>(),
    TypeId::of::<()>()
  );
  assert_eq!(
    TypeId::of::<<Runtime as frame_system::Config>::OnNewAccount>(),
    TypeId::of::<()>()
  );
  assert_eq!(
    TypeId::of::<<Runtime as frame_system::Config>::OnKilledAccount>(),
    TypeId::of::<()>()
  );
}

#[test]
fn signed_wide_close_retains_frozen_user_declaration_on_both_presets() {
  for preset in <Runtime as GenesisBuilder<Block>>::preset_names() {
    wide_close_fixture::run(&preset, None, None);
  }
}

#[test]
fn sr25519_signing_is_randomized_and_storage_proofs_are_canonical() {
  use polkadot_sdk::sp_core::{Pair, sr25519};
  // The fixture signs its caller and its Aura seal with sr25519. Schnorrkel draws a randomized
  // nonce, so one key signs the same message differently on every run and every emitted artifact
  // is instance-bound. This test pins that property: if signing ever becomes deterministic the
  // retained artifact identity loses its stated reason and this evidence must be requalified.
  let pair = sr25519::Pair::from_seed(&[7u8; 32]);
  let message = b"instance-bound fixture export";
  let first = pair.sign(message);
  let second = pair.sign(message);
  assert!(sr25519::Pair::verify(&first, message, &pair.public()));
  assert!(sr25519::Pair::verify(&second, message, &pair.public()));
  let randomized = first != second;
  // StorageProof keeps its nodes in a BTreeSet, so proof ordering is canonical and cannot
  // explain export variance; only signature randomness can.
  use polkadot_sdk::sp_trie::StorageProof;
  let nodes = vec![vec![3u8, 3, 3], vec![1u8, 1], vec![2u8, 2, 2]];
  let forward = StorageProof::new(nodes.clone());
  let reversed = StorageProof::new(nodes.into_iter().rev());
  let canonical = forward == reversed;
  assert!(canonical);
  eprintln!(
    "export_determinism sign=randomized signature_repeats={} proof_nodes_canonical={canonical} scope=fixture_artifact_identity",
    !randomized
  );
  assert!(
    randomized,
    "deterministic sr25519 signing would make fixture exports regenerable and must be requalified"
  );
}
