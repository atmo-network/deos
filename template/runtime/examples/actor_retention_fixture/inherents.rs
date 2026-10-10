//! Consensus inherent data shared by retention block and encoding diagnostics.

use codec::Encode;
use cumulus_primitives_parachain_inherent::{INHERENT_IDENTIFIER, ParachainInherentData};
use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
use deos_runtime::Header;
use polkadot_sdk::{
  cumulus_primitives_core::PersistedValidationData,
  polkadot_parachain_primitives::primitives::HeadData, sp_inherents::InherentData,
};

pub(crate) fn inherents(parent: &Header, number: u32) -> InherentData {
  let parent_head = HeadData(parent.encode());
  let (relay_parent_storage_root, relay_chain_state) = RelayStateSproofBuilder {
    para_id: deos_runtime::PARACHAIN_ID.into(),
    current_slot: u64::from(number).into(),
    included_para_head: Some(parent_head.clone()),
    ..Default::default()
  }
  .into_state_root_and_proof();
  let mut data = InherentData::new();
  data
    .put_data(
      polkadot_sdk::sp_timestamp::INHERENT_IDENTIFIER,
      &(u64::from(number) * deos_runtime::SLOT_DURATION),
    )
    .unwrap();
  data
    .put_data(
      INHERENT_IDENTIFIER,
      &ParachainInherentData {
        validation_data: PersistedValidationData {
          parent_head,
          relay_parent_number: number,
          relay_parent_storage_root,
          max_pov_size: deos_runtime::MAXIMUM_BLOCK_WEIGHT.proof_size() as u32,
        },
        relay_chain_state,
        downward_messages: Default::default(),
        horizontal_messages: Default::default(),
        relay_parent_descendants: Default::default(),
        collator_peer_id: None,
      },
    )
    .unwrap();
  pallet_deos_actors::provide_actor_prepass_inherent_data(&mut data).unwrap();
  data
}
