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

/// Prepared relay proofs authenticate this fixture, not public relay ancestry or physical cost.
#[test]
#[allow(deprecated)] // Observe the common bare pre_dispatch seam as well as actual execution.
fn actual_dmp_geometry_binds_builder_core_and_aura_before_context_dispatch() {
  use crate::configs::unsigned::RuntimeUnsignedValidator;
  use cumulus_primitives_parachain_inherent::MessageQueueChain;
  use polkadot_sdk::{
    cumulus_pallet_aura_ext::BlockExecutor,
    cumulus_primitives_core::InboundDownwardMessage,
    frame_support::{storage::storage_prefix, traits::ExecuteBlock},
    sp_api::runtime_decl_for_core::Core,
    sp_block_builder::runtime_decl_for_block_builder::BlockBuilder,
    sp_consensus_aura::{digests::CompatibleDigestItem, sr25519::AuthorityPair},
    sp_runtime::traits::ValidateUnsigned,
  };
  type AuraExecutor = BlockExecutor<Runtime, Executive>;
  let storage = preset_genesis_storage(
    &[],
    &polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into(),
  );
  let parent = parent_header_for(storage.clone(), &[], 0);
  let data_for = |count| {
    let downward_messages = (0..count)
      .map(|_| InboundDownwardMessage {
        sent_at: 1,
        msg: vec![1],
      })
      .collect::<Vec<_>>();
    let mut chain = MessageQueueChain::default();
    for message in &downward_messages {
      chain.extend_downward(message);
    }
    let builder = RelayStateSproofBuilder {
      para_id: crate::PARACHAIN_ID.into(),
      current_slot: 1u64.into(),
      included_para_head: Some(HeadData(parent.encode())),
      dmq_mqc_head: Some(chain.head()),
      ..Default::default()
    };
    let (relay_parent_storage_root, relay_chain_state) = builder.into_state_root_and_proof();
    let mut data = inherent_data_for(&parent, 1);
    data.replace_data(
      INHERENT_IDENTIFIER,
      &ParachainInherentData {
        validation_data: PersistedValidationData {
          parent_head: HeadData(parent.encode()),
          relay_parent_number: 1,
          relay_parent_storage_root,
          max_pov_size: crate::MAXIMUM_BLOCK_WEIGHT.proof_size().try_into().unwrap(),
        },
        relay_chain_state,
        downward_messages,
        horizontal_messages: Default::default(),
        relay_parent_descendants: Vec::new(),
        collator_peer_id: None,
      },
    );
    data
  };
  let data512 = data_for(512);
  let data513 = data_for(513);
  // Deliberately incoherent provider output is checked but NEVER executed: its Basic/root
  // still authenticates all 513 actual messages, while its reconstructed call has only 512.
  let mut supplied512 = data513.clone();
  let mut supplied = supplied512
    .get_data::<ParachainInherentData>(&INHERENT_IDENTIFIER)
    .unwrap()
    .unwrap();
  supplied.downward_messages.truncate(512);
  supplied512.replace_data(INHERENT_IDENTIFIER, &supplied);
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let author = |data| {
    author_complete_block_after_with_expected_dispatch(
      storage.clone(),
      &[],
      &parent,
      1,
      UserDemand::ActorOnly,
      &signer,
      0,
      &BTreeMap::new(),
      &[],
      data,
      None,
    )
  };
  let control = author(data512.clone());
  let actual = TestExternalities::new(storage.clone()).execute_with(|| {
    assert_ok!(crate::apis::validate_context_inherent_geometry(&data512));
    assert_ok!(crate::apis::validate_context_inherent_geometry(
      &supplied512
    ));
    assert_eq!(
      crate::apis::validate_context_inherent_geometry(&data513),
      Err(pallet_deos_actors::BlockResourceError::ContextGeometryExceeded)
    );
    let extrinsics = data513.create_extrinsics();
    assert_eq!(extrinsics.len(), 3);
    let actual = extrinsics
      .into_iter()
      .find(|xt| matches!(xt.function, RuntimeCall::ParachainSystem(_)))
      .unwrap();
    let RuntimeCall::ParachainSystem(
      polkadot_sdk::cumulus_pallet_parachain_system::Call::set_validation_data {
        inbound_messages_data,
        ..
      },
    ) = &actual.function
    else {
      unreachable!("selected validation inherent")
    };
    let (full, hashed) = inbound_messages_data.downward_messages.messages();
    assert_eq!((full.len(), hashed.len()), (513, 0));
    actual
  });
  let mut candidate = control.block.clone();
  let context_index = candidate
    .extrinsics
    .iter()
    .position(|xt| matches!(xt.function, RuntimeCall::ParachainSystem(_)))
    .unwrap();
  assert_eq!(candidate.extrinsics.len(), 3);
  assert!(
    context_index < 2,
    "both context providers precede Actor Prepass"
  );
  assert!(matches!(
    candidate.extrinsics[1 - context_index].function,
    RuntimeCall::Timestamp(_)
  ));
  assert!(matches!(
    candidate.extrinsics[2].function,
    RuntimeCall::Actors(pallet_deos_actors::Call::actor_prepass {})
  ));
  candidate.extrinsics[context_index] = actual.clone();
  let checker = TestExternalities::new(storage.clone()).execute_with(|| {
    let root = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
    let result = <Runtime as BlockBuilder<Block>>::check_inherents(
      candidate.clone().into(),
      supplied512.clone(),
    );
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
      root
    );
    result
  });
  // These complete pallet namespaces include ValidationData, MQC, relay context roots,
  // parent queue state and Actor phase; System's staged encoded bytes are separate.
  let context_state = || {
    let prefixes = [
      b"ParachainSystem".as_slice(),
      b"MessageQueue",
      b"XcmpQueue",
      b"Actors",
    ]
    .map(|name| storage_prefix(name, b"")[..16].to_vec());
    current_top_storage()
      .top
      .into_iter()
      .filter(|(key, _)| prefixes.iter().any(|prefix| key.starts_with(prefix)))
      .collect::<BTreeMap<_, _>>()
  };
  let exhausted: TransactionValidityError = InvalidTransaction::ExhaustsResources.into();
  let (application, guard, before_context, before_digest) = TestExternalities::new(storage.clone())
    .execute_with(|| {
      Executive::initialize_block(&control.block.header);
      for xt in control.block.extrinsics.iter().take(context_index) {
        assert_ok!(Executive::apply_extrinsic(xt.clone()));
      }
      let before_context = context_state();
      let before_digest = System::digest();
      let root = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
      let guard = RuntimeUnsignedValidator::pre_dispatch(&actual.function);
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
        root
      );
      let weight = System::block_weight();
      let size = System::block_size();
      // Executive notes encoded bytes before validity checking. The builder caller owns rollback.
      polkadot_sdk::sp_io::storage::start_transaction();
      let application = <Runtime as BlockBuilder<Block>>::apply_extrinsic(actual.clone());
      if application.is_err() {
        assert_eq!(application, Err(exhausted));
        assert_eq!(guard, Err(exhausted));
        assert_eq!(context_state(), before_context);
        assert_eq!(System::digest(), before_digest);
        assert_eq!(System::block_weight(), weight);
        assert_eq!(System::block_size(), size);
      } else {
        assert_eq!(
          application,
          Ok(Ok(())),
          "baseline must actually dispatch successfully"
        );
        assert_ne!(context_state(), before_context);
      }
      polkadot_sdk::sp_io::storage::rollback_transaction();
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
        root
      );
      (application, guard, before_context, before_digest)
    });
  let authority_pairs = [
    AuthorityPair::from_string("//Alice", None).unwrap(),
    AuthorityPair::from_string("//Bob", None).unwrap(),
  ];
  let authority_index = TestExternalities::new(storage.clone()).execute_with(|| {
    let authorities = polkadot_sdk::pallet_aura::Authorities::<Runtime>::get();
    authority_pairs
      .iter()
      .position(|pair| pair.public() == authorities[1])
      .unwrap()
  });
  let seal = |mut block: Block| {
    let signature = authority_pairs[authority_index].sign(block.header.hash().as_bytes());
    block
      .header
      .digest_mut()
      .push(DigestItem::aura_seal(signature));
    block
  };
  let replay = |block: Block, aura: bool, expected: Option<&Storage>| {
    let mut ext = TestExternalities::new(storage.clone());
    let recorder = Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
    ext.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
      recorder.clone(),
    )));
    ext.execute_with_recorder(recorder, || {
      let root = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
      polkadot_sdk::sp_io::storage::start_transaction();
      let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if aura {
          <AuraExecutor as ExecuteBlock<Block>>::execute_block(block.clone().into());
        } else {
          <Runtime as Core<Block>>::execute_block(block.clone().into());
        }
      }));
      if let Some(expected) = expected {
        assert!(
          result.is_ok(),
          "coherent full import must succeed: aura={aura}"
        );
        assert_eq!(current_top_storage().top, expected.top);
        polkadot_sdk::sp_io::storage::commit_transaction();
      } else {
        let panic = result.expect_err("over-context must fail before root/finalization checks");
        let message = panic
          .downcast_ref::<String>()
          .map(String::as_str)
          .or_else(|| panic.downcast_ref::<&str>().copied())
          .unwrap_or("non-text panic");
        let expected = format!(
          "{:?}",
          polkadot_sdk::frame_executive::ExecutiveError::ApplyExtrinsic(exhausted)
        );
        assert_eq!(message, expected, "not a seal, MQC, or final-root failure");
        assert_eq!(context_state(), before_context);
        assert_eq!(System::digest(), before_digest);
        polkadot_sdk::sp_io::storage::rollback_transaction();
        assert_eq!(
          polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
          root
        );
      }
    });
    if expected.is_some() {
      ext.commit_all().unwrap();
      assert_eq!(ext.backend.root(), block.header.state_root());
    }
  };
  // Always retain complete, correctly rooted and Aura-sealed 512-message controls.
  replay(control.block.clone(), false, Some(&control.post_state));
  replay(seal(control.block.clone()), true, Some(&control.post_state));
  if application == Ok(Ok(())) {
    // On the unfixed runtime do not stop at a checker assertion: first demonstrate
    // complete real author/Core/Aura acceptance with all 513 messages authenticated.
    let over = author(data513);
    replay(over.block.clone(), false, Some(&over.post_state));
    replay(seal(over.block), true, Some(&over.post_state));
    eprintln!(
      "Q1 prepared DMP513 accepted by builder, complete Core and sealed Aura; no public ancestry claim"
    );
  } else {
    // The actual call has its coherent 513-message proof. The old control roots cannot
    // explain rejection: require the exact early validity error and untouched context.
    replay(candidate.clone(), false, None);
    replay(seal(candidate), true, None);
  }
  assert_eq!(
    application,
    Err(exhausted),
    "actual DMP513 must be refused before context dispatch"
  );
  assert_eq!(guard, Err(exhausted));
  assert_eq!(
    checker.encode(),
    crate::apis::context_geometry_fatal_result().encode(),
    "actual-call geometry must reject even while reconstructed supplied-data geometry passes"
  );
}
