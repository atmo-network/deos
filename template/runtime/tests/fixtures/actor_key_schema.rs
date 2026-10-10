use codec::{Decode, Encode};
use polkadot_sdk::frame_support::__private::metadata::{
  RuntimeMetadata, RuntimeMetadataPrefixed, v16::StorageEntryType,
};

/// Key declarations and their complete type registry, excluding runtime constants and values.
pub fn identity(encoded: &[u8]) -> [u8; 32] {
  let mut input = encoded;
  let RuntimeMetadataPrefixed(_, RuntimeMetadata::V16(metadata)) =
    RuntimeMetadataPrefixed::decode(&mut input).expect("native metadata v16")
  else {
    panic!("expected metadata v16")
  };
  assert!(input.is_empty());
  let actors = metadata
    .pallets
    .into_iter()
    .find(|pallet| pallet.name == "Actors")
    .expect("Actors metadata");
  let storage = actors.storage.expect("Actors storage metadata");
  let keys = storage
    .entries
    .into_iter()
    .map(|entry| {
      let key = match entry.ty {
        StorageEntryType::Plain(_) => None,
        StorageEntryType::Map { hashers, key, .. } => Some((hashers, key.id)),
      };
      (entry.name, key)
    })
    .collect::<Vec<_>>();
  (storage.prefix, keys, metadata.types).using_encoded(polkadot_sdk::sp_io::hashing::blake2_256)
}
