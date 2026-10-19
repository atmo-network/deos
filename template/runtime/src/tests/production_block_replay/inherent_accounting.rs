//! Registered encoded-length accounting, not physical proof or admission-cost measurement.

use super::*;
use polkadot_sdk::frame_support::dispatch::DispatchClass;

#[derive(Debug)]
struct ByteReceipt {
  inherent_bytes: u64,
  initial_size_counter: u32,
  emitted_digest_bytes: u64,
  initial_frame: Weight,
  settled_frame: Weight,
  fixed: Weight,
}

#[test]
#[allow(deprecated)] // Probe the exact bare prepare boundary used by Executive.
fn inherent_bytes_separate_frame_charges_from_header_and_digest_counters() {
  use crate::configs::unsigned::RuntimeUnsignedValidator;
  use polkadot_sdk::sp_runtime::traits::{TransactionExtension, ValidateUnsigned};
  let mut receipts = Vec::new();
  for (parent_padding, header_padding) in [(0, 0), (257, 0), (0, 257)] {
    let fixture = prepare_actor_fixture(&[], 0, WorkloadSchedule::ManualOnly);
    let mut parent = parent_header_for(fixture.storage.clone(), &[], 1);
    if parent_padding != 0 {
      parent
        .digest_mut()
        .push(DigestItem::Other(vec![0; parent_padding]));
    }
    let mut logs = vec![DigestItem::PreRuntime(
      polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
      2u64.encode(),
    )];
    if header_padding != 0 {
      // Executive forwards pre-runtime items, not arbitrary Other items, into initialization.
      logs.push(DigestItem::PreRuntime(*b"BYTE", vec![0; header_padding]));
    }
    let header = Header::new(
      2,
      Default::default(),
      Default::default(),
      parent.hash(),
      Digest { logs },
    );
    let mut ext = TestExternalities::new(fixture.storage);
    let receipt = ext.execute_with(|| {
      Executive::initialize_block(&header);
      let initial_size_counter = System::block_size();
      let initial_frame = System::block_weight().total();
      let mut inherent_bytes = 0u64;
      let mut emitted_digest_bytes = 0u64;
      let extrinsics = inherent_data_for(&parent, 2).create_extrinsics();
      assert_eq!(extrinsics.len(), 3);
      for extrinsic in extrinsics {
        let info = extrinsic.get_dispatch_info();
        let len = u32::try_from(extrinsic.encode().len()).unwrap();
        assert_eq!(info.class, DispatchClass::Mandatory);
        assert_eq!(info.extension_weight, Weight::zero());
        let frame_before = System::block_weight();
        let size_before = System::block_size();
        let digest_before = System::digest();
        let root_before = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
        // A pure guard's unchanged root/counters are not a CPU or no-read proof.
        assert_ok!(RuntimeUnsignedValidator::pre_dispatch(&extrinsic.function));
        assert_eq!(System::block_weight(), frame_before);
        assert_eq!(System::block_size(), size_before);
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
          root_before
        );
        // Expose preparation before executing the body, then restore all probe effects.
        // Prepass temporarily books its full maximum here; that is not settled fixed work.
        polkadot_sdk::sp_io::storage::start_transaction();
        assert_ok!(crate::TxExtension::bare_validate_and_prepare(
          &extrinsic.function,
          &info,
          usize::try_from(len).unwrap(),
        ));
        let base = crate::configs::RuntimeBlockWeights::get()
          .get(info.class)
          .base_extrinsic;
        let expected = frame_before
          .total()
          .checked_add(&info.total_weight())
          .and_then(|weight| weight.checked_add(&base))
          .and_then(|weight| weight.checked_add(&Weight::from_parts(0, u64::from(len))))
          .unwrap();
        assert_eq!(System::block_weight().total(), expected);
        assert_eq!(System::block_size(), size_before.checked_add(len).unwrap());
        polkadot_sdk::sp_io::storage::rollback_transaction();
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
          root_before
        );
        assert_ok!(Executive::apply_extrinsic(extrinsic));
        let digest_after = System::digest();
        assert!(digest_after.logs.starts_with(&digest_before.logs));
        let added_digest = digest_after.logs[digest_before.logs.len()..]
          .iter()
          .map(|item| u64::try_from(item.encode().len()).unwrap())
          .sum::<u64>();
        emitted_digest_bytes = emitted_digest_bytes.checked_add(added_digest).unwrap();
        inherent_bytes = inherent_bytes.checked_add(u64::from(len)).unwrap();
        assert_eq!(
          u64::from(System::block_size()),
          u64::from(size_before) + u64::from(len) + added_digest,
        );
        let retained = System::block_weight()
          .total()
          .checked_sub(&frame_before.total())
          .unwrap();
        assert!(retained.proof_size() >= base.proof_size() + u64::from(len));
        for class in [DispatchClass::Normal, DispatchClass::Operational] {
          assert_eq!(System::block_weight().get(class), frame_before.get(class));
        }
      }
      assert_eq!(
        u64::from(System::block_size()),
        u64::from(initial_size_counter) + inherent_bytes + emitted_digest_bytes,
      );
      assert!(emitted_digest_bytes > 0);
      let state = Actors::block_resource_state().unwrap();
      assert_eq!(
        state.phase(),
        pallet_deos_actors::BlockResourcePhase::ExternalPhase
      );
      let fixed = state.budget().unwrap().fixed_envelope();
      assert_eq!(fixed, BlockResourceBudgetValue::get().fixed_envelope());
      let receipt = ByteReceipt {
        inherent_bytes,
        initial_size_counter,
        emitted_digest_bytes,
        initial_frame,
        settled_frame: System::block_weight().total(),
        fixed,
      };
      println!(
        "INHERENT_BYTE_RECEIPT_V1 {}",
        serde_json::json!({
          "parentPadding": parent_padding,
          "headerPadding": header_padding,
          "chargedInherentEncodedBytes": receipt.inherent_bytes.to_string(),
          "initialBlockSizeCounter": receipt.initial_size_counter.to_string(),
          "emittedDigestCounterBytes": receipt.emitted_digest_bytes.to_string(),
          "registeredFrameProofSize": receipt.settled_frame.proof_size().to_string(),
          "configuredFixedProofSize": receipt.fixed.proof_size().to_string(),
        })
      );
      Executive::finalize_block();
      assert!(Actors::finalized_block_resource_telemetry().is_some());
      receipt
    });
    receipts.push(receipt);
  }
  let baseline = &receipts[0];
  let parent_payload = &receipts[1];
  let header_payload = &receipts[2];
  assert!(parent_payload.inherent_bytes > baseline.inherent_bytes);
  assert_eq!(
    parent_payload.initial_size_counter,
    baseline.initial_size_counter
  );
  assert_eq!(parent_payload.fixed, baseline.fixed);
  assert_eq!(header_payload.inherent_bytes, baseline.inherent_bytes);
  assert!(header_payload.initial_size_counter > baseline.initial_size_counter);
  assert_eq!(header_payload.initial_frame, baseline.initial_frame);
  assert_eq!(header_payload.settled_frame, baseline.settled_frame);
  assert_eq!(header_payload.fixed, baseline.fixed);
}
