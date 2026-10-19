//! Current-block context ordering under native Executive, not PVF/host qualification.

use super::*;
use pallet_deos_actors::ActorPrepassContext;
use polkadot_sdk::sp_block_builder::runtime_decl_for_block_builder::BlockBuilder;

fn context_header(parent: &Header, number: u32) -> Header {
  Header::new(
    number,
    Default::default(),
    Default::default(),
    parent.hash(),
    Digest {
      logs: vec![DigestItem::PreRuntime(
        polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
        u64::from(number).encode(),
      )],
    },
  )
}

#[test]
fn prepass_timestamp_order_rejects_stale_context() {
  // Both valid context orders, then each required context missing at the Prepass boundary.
  for order in [[0usize, 1, 2], [1, 0, 2], [0, 2, 1], [1, 2, 0]] {
    let valid = order[2] == 2;
    let fixture = prepare_actor_fixture(&[], 0, WorkloadSchedule::ManualOnly);
    let seed_parent = parent_header_for(fixture.storage.clone(), &[], 1);
    let mut ext = TestExternalities::new(fixture.storage);
    let replay_input = ext.execute_with(|| {
      Executive::initialize_block(&context_header(&seed_parent, 2));
      for extrinsic in inherent_data_for(&seed_parent, 2).create_extrinsics() {
        assert_ok!(Executive::apply_extrinsic(extrinsic));
      }
      let parent = Executive::finalize_block();
      let old_timestamp = crate::Timestamp::get();
      assert_eq!(old_timestamp, 2 * crate::SLOT_DURATION);
      let parent_storage = current_top_storage();
      let header = context_header(&parent, 3);
      Executive::initialize_block(&header);
      assert_eq!(crate::Timestamp::get(), old_timestamp);
      assert!(!polkadot_sdk::cumulus_pallet_parachain_system::ValidationData::<Runtime>::exists());
      let data = inherent_data_for(&parent, 3);
      let generated = data.create_extrinsics();
      assert_eq!(generated.len(), 3);
      let candidate = Block { header, extrinsics: order.map(|index| generated[index].clone()).to_vec() };
      let result = <Runtime as BlockBuilder<Block>>::check_inherents(candidate.clone().into(), data.clone());
      println!("PREPASS_ORDER_CONFORMANCE_V1 order={order:?} check={} expected={valid}", result.ok());
      assert_eq!(result.ok(), valid, "inherent checking must reject incomplete current context");
      if valid {
        let mut duplicate = candidate.clone();
        duplicate.extrinsics.push(generated[2].clone());
        assert!(!<Runtime as BlockBuilder<Block>>::check_inherents(duplicate.into(), data).ok());
      }
      for extrinsic in candidate.extrinsics.iter().cloned() {
        let prepass = matches!(extrinsic.function, RuntimeCall::Actors(pallet_deos_actors::Call::actor_prepass {}));
        if prepass {
          let ready = <<Runtime as pallet_deos_actors::Config>::PrepassContext as ActorPrepassContext>::context_ready();
          assert_eq!(ready, valid);
          if !valid {
            let before = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
            let state = Actors::block_resource_state();
            polkadot_sdk::sp_io::storage::start_transaction();
            let applied = Executive::apply_extrinsic(extrinsic);
            assert_eq!(applied, Err(InvalidTransaction::BadMandatory.into()));
            assert_eq!(Actors::block_resource_state(), state, "refusal precedes new freeze/effects");
            polkadot_sdk::sp_io::storage::rollback_transaction();
            assert_eq!(polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()), before);
            return None;
          }
          assert_eq!(crate::Timestamp::get(), 3 * crate::SLOT_DURATION);
        }
        assert_ok!(Executive::apply_extrinsic(extrinsic));
      }
      let finalized = Executive::finalize_block();
      Some((parent_storage, Block { header: finalized, extrinsics: candidate.extrinsics }))
    });
    if let Some((parent_storage, block)) = replay_input {
      let mut replay = TestExternalities::new(parent_storage);
      replay.execute_with(|| Executive::execute_block(block.into()));
      println!("PREPASS_ORDER_REPLAY_V1 order={order:?} ok=true");
    }
  }
}

#[test]
fn timestamp_update_adapter_tracks_sdk_marker_lifecycle() {
  use polkadot_sdk::frame_support::traits::OnFinalize;
  let fixture = prepare_actor_fixture(&[], 0, WorkloadSchedule::ManualOnly);
  let parent = parent_header_for(fixture.storage.clone(), &[], 1);
  TestExternalities::new(fixture.storage).execute_with(|| {
    Executive::initialize_block(&context_header(&parent, 2));
    let extrinsics = inherent_data_for(&parent, 2).create_extrinsics();
    for extrinsic in extrinsics.into_iter().take(2) {
      assert_ok!(Executive::apply_extrinsic(extrinsic));
    }
    let ready = || <<Runtime as pallet_deos_actors::Config>::PrepassContext as ActorPrepassContext>::context_ready();
    assert!(ready());
    let marker = crate::Timestamp::storage_info().into_iter().find(|item| item.storage_name == b"DidUpdate").unwrap();
    assert_eq!(marker.max_size, Some(1));
    assert_eq!(marker.max_values, Some(1));
    assert_eq!(marker.prefix.len(), 32);
    let moment = crate::Timestamp::get();
    // Exercise the actual SDK consumer/producer with an unchanged numerical timestamp.
    <crate::Timestamp as OnFinalize<u32>>::on_finalize(2);
    assert_eq!(crate::Timestamp::get(), moment);
    assert!(!ready());
    crate::Timestamp::set_timestamp(moment);
    assert_eq!(crate::Timestamp::get(), moment);
    assert!(ready());
    for bytes in [None, Some(vec![0]), Some(vec![2]), Some(vec![1, 0])] {
      let root = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
      polkadot_sdk::sp_io::storage::start_transaction();
      match bytes {
        Some(bytes) => polkadot_sdk::sp_io::storage::set(&marker.prefix, &bytes),
        None => polkadot_sdk::sp_io::storage::clear(&marker.prefix),
      }
      assert!(!ready(), "missing/false/noncanonical/oversized marker fails closed");
      polkadot_sdk::sp_io::storage::rollback_transaction();
      assert_eq!(polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()), root);
    }
  });
}
