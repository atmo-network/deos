//! Native maximum-state Control diagnostic for the unchanged retention corpus.
//!
//! User identities, fees, holds, contracts and slots use public pallet calls. The
//! source pages/holes are a synthetic projection of the fixed recipe, prepared
//! outside the encoded chain. This is not a public construction-history certificate.

mod actor_retention_corpus;
mod actor_retention_fixture;

use actor_retention_corpus::{INPUT_SHA256, PAGES, WATCHES, digest, fixed_pages};
use actor_retention_fixture::{
  PREPARED_ACTIVE_ACTORS, PREPARED_IDENTITIES, Prepared, inherents, prepare, query_proof,
  recorded_keys, recorded_nodes, signed_lifecycle,
};
use codec::Encode;
use deos_runtime::{
  AccountId, Actors, Assets, Balances, Block, Executive, Header, Runtime, RuntimeCall,
  RuntimeEvent, System, TransactionPayment, UncheckedExtrinsic,
};
use pallet_deos_actors::{
  ActorContractOf, ActorId, DependencyPlans, DependencyRegistrationFreeIndices,
  DependencyRegistrationFreePositions, DependencyRegistrationHeaders, DependencyRegistrationPages,
  DependencyRegistrationPosition, DependencyRegistrationPositions, DependencySourceAllocatorState,
  Event, ParkedBalanceEpisodes, ProcessResidence, ProcessStatus, ServiceNodes,
  ServiceResidenceKind, TriggerFamily,
};
use polkadot_sdk::{
  frame_support::dispatch::GetDispatchInfo,
  sp_block_builder::runtime_decl_for_block_builder::BlockBuilder,
  sp_core::{Blake2Hasher, Pair},
  sp_genesis_builder::{PresetId, runtime_decl_for_genesis_builder::GenesisBuilder},
  sp_io::TestExternalities,
  sp_keyring::Sr25519Keyring,
  sp_runtime::{
    Digest, DigestItem,
    traits::{BlakeTwo256, Hash, Header as HeaderT},
  },
  sp_trie::{
    proof_size_extension::{ProofSizeExt, RecordingProofSizeProvider},
    recorder::Recorder,
  },
};
use std::collections::BTreeSet;

type EpochCall = (UncheckedExtrinsic, AccountId, deos_runtime::Nonce, u128);

fn epoch_script(
  importer: &mut TestExternalities,
  temporary: &[ActorId; 2],
  contract: &ActorContractOf<Runtime>,
) -> Vec<EpochCall> {
  // Signature/quote preparation belongs outside the author's proof recorder.
  // Otherwise wallet-side reads would prime the measured signed dispatch.
  importer.execute_with(|| {
    [
      (
        Sr25519Keyring::Alice.pair(),
        RuntimeCall::Actors(pallet_deos_actors::Call::deactivate_actor {
          actor_id: temporary[0],
        }),
      ),
      (
        Sr25519Keyring::Bob.pair(),
        RuntimeCall::Actors(pallet_deos_actors::Call::activate_actor {
          actor_id: temporary[1],
          contract: contract.clone(),
        }),
      ),
    ]
    .into_iter()
    .map(|(pair, call)| {
      let owner = AccountId::from(pair.public());
      let nonce = System::account_nonce(&owner);
      let extrinsic = signed_lifecycle(&pair, call);
      let quote = TransactionPayment::compute_fee(
        u32::try_from(extrinsic.encoded_size()).unwrap(),
        &extrinsic.get_dispatch_info(),
        0,
      );
      (extrinsic, owner, nonce, quote)
    })
    .collect()
  })
}

fn execute(preset: &PresetId, prepared: Prepared, pages: &[Vec<u64>], rotation: bool) {
  println!("admission execution begins: preset={preset} rotation={rotation}");
  let mut author = TestExternalities::new(prepared.storage.clone());
  let mut importer = TestExternalities::new(prepared.storage);
  let first_block = if rotation {
    use pallet_session_rotation::SessionRotation;
    use polkadot_sdk::frame_support::traits::EstimateNextSessionRotation;
    let period = <<Runtime as polkadot_sdk::pallet_session::Config>::NextSessionRotation as
      EstimateNextSessionRotation<u32>>::average_session_length();
    type Rotation = <Runtime as pallet_session_rotation::Config>::SessionRotation;
    assert!(period > 0 && Rotation::should_rotate(0) && Rotation::should_rotate(period));
    assert!(!Rotation::should_rotate(period - 1));
    period
  } else {
    2
  };
  // The constructed clock isolates the scheduled hook; no history to it is asserted.
  if rotation {
    for state in [&mut author, &mut importer] {
      state.execute_with(|| {
        System::set_block_number(first_block - 1);
        assert_eq!(deos_runtime::Session::current_index(), 0);
      });
      state.commit_all().unwrap();
    }
  }
  let mut parent = Header::new(
    first_block - 1,
    BlakeTwo256::ordered_trie_root(
      Vec::new(),
      deos_runtime::VERSION.extrinsics_root_state_version(),
    ),
    *author.as_backend().root(),
    Default::default(),
    Digest::default(),
  );
  let mut woke = false;
  for number in first_block..=first_block + 64 {
    assert_eq!(*author.as_backend().root(), *parent.state_root());
    assert_eq!(*importer.as_backend().root(), *parent.state_root());
    let rotation_source_next = (rotation && number == first_block).then(|| {
      importer.execute_with(|| {
        let list = pallet_deos_actors::DependencyScanSourceListState::<Runtime>::get();
        assert_eq!(list.count, WATCHES as u32);
        pallet_deos_actors::DependencyScanSourceNodes::<Runtime>::get(list.cursor.unwrap())
          .unwrap()
          .next
      })
    });
    let required_nodes = query_proof(&mut importer, &prepared.queries)
      .iter_nodes()
      .cloned()
      .collect::<BTreeSet<_>>();
    assert_eq!(*importer.as_backend().root(), *parent.state_root());
    let script = epoch_script(
      &mut importer,
      &prepared.temporary,
      &prepared.temporary_contract,
    );
    let data = inherents(&parent, number);
    let recorder = Recorder::<Blake2Hasher>::default();
    author.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
      recorder.clone(),
    )));
    let (block, phase, epoch_raw) = author.execute_with_recorder(recorder.clone(), || {
      let header = Header::new(
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
      );
      Executive::initialize_block(&header);
      let mut extrinsics = <Runtime as BlockBuilder<Block>>::inherent_extrinsics(data.clone());
      assert!(extrinsics.len() >= 3);
      let mut before = None;
      let mut after = None;
      for extrinsic in &extrinsics {
        let is_prepass = matches!(
          extrinsic.function,
          RuntimeCall::Actors(pallet_deos_actors::Call::actor_prepass {})
        );
        if is_prepass {
          assert!(before.is_none());
          before = Some((
            recorded_nodes(&recorder),
            recorded_keys(&recorder),
            recorder.estimate_encoded_size(),
          ));
        }
        Executive::apply_extrinsic(extrinsic.clone())
          .unwrap()
          .unwrap();
        if is_prepass {
          after = Some((
            recorded_nodes(&recorder),
            recorded_keys(&recorder),
            recorder.estimate_encoded_size(),
          ));
        }
      }
      let state = Actors::block_resource_state().unwrap();
      let usage = state.usage();
      let woke = !ParkedBalanceEpisodes::<Runtime>::contains_key(prepared.target);
      let mut epoch_raw = Vec::new();
      if woke {
        for (extrinsic, _, _, _) in &script {
          let raw_before = recorder.estimate_encoded_size();
          Executive::apply_extrinsic(extrinsic.clone()).unwrap().unwrap();
          epoch_raw.push(recorder.estimate_encoded_size() - raw_before);
          extrinsics.push(extrinsic.clone());
        }
      }

      let fee = System::events()
        .iter(/* deos-bypass: bounded-iter -- current-block events in the fixed Actor-only native fixture. */)
        .find_map(|record| match record.event {
          RuntimeEvent::Actors(Event::TriggerOccurrenceProcessed {
            actor_id,
            trigger_family: TriggerFamily::ParkedBalance,
            fee,
          }) if actor_id == prepared.target => Some(fee),
          _ => None,
        });
      let block = Block {
        header: Executive::finalize_block(),
        extrinsics,
      };
      (
        block,
        (
          before.unwrap(),
          after.unwrap(),
          usage.actor_control_used(),
          state.budget().unwrap().limits().actor_control(),
          woke,
          fee,
        ),
        epoch_raw,
      )
    });
    author.commit_all().unwrap();
    assert_eq!(*author.as_backend().root(), *block.header.state_root());
    println!("admission author root reconciled: preset={preset} block={number}");
    // Inherent validation is a separate runtime call. Its reads must not prime
    // the execution recorder or change signed storage-weight reclamation.
    importer.execute_with(|| {
      assert!(<Runtime as BlockBuilder<Block>>::check_inherents(block.clone().into(), data).ok());
    });
    let import_recorder = Recorder::<Blake2Hasher>::default();
    importer.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
      import_recorder.clone(),
    )));
    importer.execute_with_recorder(import_recorder, || {
      Executive::execute_block(block.clone().into());
    });
    importer.commit_all().unwrap();
    assert_eq!(*importer.as_backend().root(), *block.header.state_root());
    println!("admission import root reconciled: preset={preset} block={number}");
    if let Some(next) = rotation_source_next {
      importer.execute_with(|| {
        assert_eq!(deos_runtime::Session::current_index(), 1);
        assert_eq!(
          pallet_deos_actors::DependencyScanSourceListState::<Runtime>::get().cursor,
          Some(next)
        );
        for source in
          prepared.sources.iter(/* deos-bypass: bounded-iter -- four fixed corpus source scans. */)
        {
          assert!(
            pallet_deos_actors::DependencyRevisions::<Runtime>::get(source)
              .scan_target
              .is_some()
          );
        }
        assert!(
          !Actors::finalized_block_resource_telemetry()
            .unwrap()
            .optional_actor_work_halted()
        );
      });
      println!(
        "scope=native_synthetic_retention_rotation preset={preset} block={number} session_index=1 source_cursor_advanced_to={next} four_scans_active=true optional_work_halted=false"
      );
    }
    let (
      (before_nodes, before_keys, before_raw),
      (after_nodes, after_keys, after_raw),
      charged,
      limit,
      did_wake,
      fee,
    ) = phase;
    if did_wake {
      for (key, _) in &prepared.queries {
        assert!(!before_keys.contains(key));
        assert!(after_keys.contains(key));
      }
      assert!(required_nodes.is_subset(&after_nodes));
      let required_unprimed_bytes = required_nodes
        .difference(&before_nodes)
        .map(Encode::encoded_size)
        .sum::<usize>();
      let raw_increment = after_raw.checked_sub(before_raw).unwrap();
      assert!(raw_increment >= required_unprimed_bytes);
      assert_eq!(
        raw_increment,
        after_nodes
          .difference(&before_nodes)
          .map(Encode::encoded_size)
          .sum::<usize>()
      );
      author.execute_with(|| {
        assert_eq!(epoch_raw.len(), 2);
        for ((_, owner, nonce, quote), raw) in script.iter(/* deos-bypass: bounded-iter -- the two fixed signed calls. */).zip(&epoch_raw) {
          assert_eq!(System::account_nonce(owner), nonce + 1);
          let fee = System::events().iter(/* deos-bypass: bounded-iter -- the complete fixed block's events. */).find_map(|record| match &record.event {
            RuntimeEvent::TransactionPayment(polkadot_sdk::pallet_transaction_payment::Event::TransactionFeePaid { who, actual_fee, tip }) if who == owner => {
              assert_eq!(*tip, 0);
              Some(*actual_fee)
            },
            _ => None,
          }).unwrap();
          assert!(fee <= *quote, "actual coded tariff must stay within its declared reservation");
          println!("scope=native_prepared_signed_retention_epoch preset={preset} block={number} owner={owner:?} nonce={nonce} declared_fee={quote} actual_fee={fee} raw_call_increment={raw}");
        }
        assert!(Actors::active_actor_state(prepared.temporary[0]).is_none());
        assert_eq!(Actors::actor_state_hold(prepared.temporary[0]).unwrap().encode(), prepared.dormant_holds[0]);
        assert_eq!(Actors::actor_contract(prepared.temporary[1]), Some(prepared.temporary_contract.clone()));
        assert!(ParkedBalanceEpisodes::<Runtime>::contains_key(prepared.temporary[1]));
        for (index, source) in prepared.sources.iter(/* deos-bypass: bounded-iter -- four fixed sources. */).enumerate() {
          let frontier = (pages[index][PAGES - 1] + 1) * 32 + 1;
          assert_eq!(DependencyRegistrationPositions::<Runtime>::get(source, prepared.temporary[0]), None);
          assert_eq!(DependencyRegistrationPositions::<Runtime>::get(source, prepared.temporary[1]), Some(DependencyRegistrationPosition { page: frontier / 32, slot: (frontier % 32) as u8 }));
          assert_eq!(DependencyRegistrationHeaders::<Runtime>::get(source).next_index, frontier + 1);
        }
        let identity = Actors::actor_identity(prepared.target).unwrap();
        assert_eq!(identity, prepared.identity);
        assert_eq!(
          DependencySourceAllocatorState::<Runtime>::get().encode(),
          prepared.allocator
        );
        let process = Actors::actor_processes(prepared.target).unwrap();
        assert_eq!(process.status, ProcessStatus::Serving);
        assert_eq!(
          process.residence,
          Some(ProcessResidence::Service(ServiceResidenceKind::Pending))
        );
        assert_eq!(
          ServiceNodes::<Runtime>::get(prepared.target)
            .unwrap()
            .eligible_from,
          number + 1
        );
        assert_eq!(
          Actors::actor_contract(prepared.target),
          Some(prepared.contract.clone())
        );
        assert!(Actors::pending_signal(prepared.target));
        assert!(Actors::actor_run_state(prepared.target).is_none());
        assert!(DependencyPlans::<Runtime>::get(prepared.target).is_empty());
        assert_eq!(
          Balances::free_balance(&identity.sovereign_account) + fee.unwrap(),
          prepared.native_before
        );
        assert_eq!(
          Assets::balance(prepared.first_asset, &identity.sovereign_account),
          117
        );
        assert_eq!(
          (Actors::actor_identity_count(), Actors::active_actor_count()),
          (PREPARED_IDENTITIES, PREPARED_ACTIVE_ACTORS)
        );
        let count = 31 * PAGES as u64;
        for (index, source) in prepared.sources.iter(/* deos-bypass: bounded-iter -- four fixed watch sources. */).copied().enumerate() {
          let selected = &pages[index];
          let local = PAGES / 2;
          let header = DependencyRegistrationHeaders::<Runtime>::get(source);
          assert_eq!(
            (header.count, header.free_count),
            (PAGES as u32, count - 31)
          );
          assert!(!DependencyRegistrationPages::<Runtime>::contains_key(
            source,
            selected[local]
          ));
          assert_eq!(
            DependencyRegistrationPages::<Runtime>::get(source, selected[local - 1])
              .unwrap()
              .next,
            Some(selected[local + 1])
          );
          assert_eq!(
            DependencyRegistrationPages::<Runtime>::get(source, selected[local + 1])
              .unwrap()
              .previous,
            Some(selected[local - 1])
          );
          for offset in 0..31u64 {
            let destination = 31 * local as u64 + offset;
            let tail = DependencyRegistrationPosition {
              page: selected[PAGES - 1],
              slot: (31 - offset) as u8,
            };
            assert_eq!(
              DependencyRegistrationFreePositions::<Runtime>::get(source, destination),
              Some(tail)
            );
            assert_eq!(
              DependencyRegistrationFreeIndices::<Runtime>::get(source, tail),
              Some(destination)
            );
          }
          for tail in count - 31..=count {
            assert!(!DependencyRegistrationFreePositions::<Runtime>::contains_key(source, tail));
          }
          for slot in 0..32u8 {
            assert!(!DependencyRegistrationFreeIndices::<Runtime>::contains_key(
              source,
              DependencyRegistrationPosition {
                page: selected[local],
                slot
              }
            ));
          }
        }
      });
      println!(
        "scope=native_synthetic_retention_control preset={preset} block={number} watches={WATCHES} required_unprimed_bytes={required_unprimed_bytes} prepass_raw_increment={raw_increment} charged_control={charged:?} control_limit={limit:?} required_reads_exceed_control={}",
        required_unprimed_bytes as u64 > limit.proof_size()
      );
      woke = true;
    }
    #[cfg(feature = "try-runtime")]
    author.execute_with(|| {
      println!("admission author try-state begins: preset={preset} block={number}");
      <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(number).unwrap();
      println!("admission author try-state passed: preset={preset} block={number}");
    });
    #[cfg(feature = "try-runtime")]
    importer.execute_with(|| {
      println!("admission import try-state begins: preset={preset} block={number}");
      <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(number).unwrap();
      println!("admission import try-state passed: preset={preset} block={number}");
    });
    if woke {
      break;
    }
    parent = block.header;
  }
  assert!(
    woke,
    "the bounded due-review prefix must reach the first prepared target"
  );
}

fn execute_construction(preset: &PresetId, prepared: Prepared, call_footprint: bool) {
  use pallet_deos_actors::{
    ActorContractHeads, ActorType, AmountResolution, AssetOps, BalanceDependencySources,
    DependencyRevisions, DependencyScanSourceListState, Task, Trigger, WeightInfo,
  };
  use polkadot_sdk::frame_support::{traits::Get, weights::Weight};
  use primitives::AssetKind;

  type Ops = <Runtime as pallet_deos_actors::Config>::AssetOps;
  type Weights = <Runtime as pallet_deos_actors::Config>::WeightInfo;
  let (mut author, mut importer) = match prepared.committed {
    Some(committed) => {
      assert!(call_footprint);
      let root = *committed.backend.root();
      let mut author = TestExternalities::new_empty();
      let mut importer = TestExternalities::new_empty();
      author.state_version = committed.state_version;
      importer.state_version = committed.state_version;
      author.backend = committed.backend;
      importer.backend = author.backend.clone();
      assert_eq!(*author.backend.root(), root);
      assert_eq!(*importer.backend.root(), root);
      println!("construction committed backends ready: preset={preset} root={root:?}");
      (author, importer)
    }
    None => (
      TestExternalities::new(prepared.storage.clone()),
      TestExternalities::new(prepared.storage),
    ),
  };
  let first = {
    use polkadot_sdk::frame_support::traits::EstimateNextSessionRotation;
    <<Runtime as polkadot_sdk::pallet_session::Config>::NextSessionRotation as
      EstimateNextSessionRotation<u32>>::average_session_length()
  };
  let last = first
    .checked_add(if call_footprint { 0 } else { 8 })
    .unwrap();
  let storage_owners = call_footprint.then(|| {
    use polkadot_sdk::frame_support::traits::StorageInfoTrait;
    deos_runtime::AllPalletsWithSystem::storage_info()
      .into_iter()
      .map(|info| {
        (
          info.prefix,
          format!(
            "{}::{}",
            String::from_utf8(info.pallet_name).unwrap(),
            String::from_utf8(info.storage_name).unwrap()
          ),
        )
      })
      .collect::<std::collections::BTreeMap<_, _>>()
  });
  let producers = importer.execute_with(|| {
    assert_eq!(prepared.native_before, 0);
    let producers = ActorContractHeads::<Runtime>::iter()
      .filter_map(|(actor, head)| {
        if head.header.trigger != Trigger::cadenced(1) {
          return None;
        }
        let state = Actors::active_actor_state(actor).unwrap();
        if state.identity.actor_class.actor_type() != ActorType::User {
          return None;
        }
        assert_eq!(state.contract.steps.len(), 1);
        let Task::Transfer {
          asset,
          amount: AmountResolution::Fixed(1),
          ..
        } = state.contract.steps[0].task
        else {
          panic!("fixed producers retain their one-unit Transfer");
        };
        let source = BalanceDependencySources::<Runtime>::get(asset).unwrap();
        Some((actor, asset, source, state.contract))
      })
      .collect::<Vec<_>>();
    assert_eq!(producers.len(), WATCHES);
    producers
  });
  let mut previous = importer.execute_with(|| {
    producers
      .iter() // deos-bypass: bounded-iter -- four fixed corpus producers
      .map(|(actor, asset, source, _)| {
        let custody = Actors::actor_identity(*actor).unwrap().sovereign_account;
        (
          Ops::total_balance(&custody, *asset),
          DependencyRevisions::<Runtime>::get(source).revision,
        )
      })
      .collect::<Vec<_>>()
  });
  let mut transfers = [0u32; WATCHES];
  for state in [&mut author, &mut importer] {
    state.execute_with(|| System::set_block_number(first - 1));
    state.commit_all().unwrap();
  }
  let mut parent = Header::new(
    first - 1,
    BlakeTwo256::ordered_trie_root(
      Vec::new(),
      deos_runtime::VERSION.extrinsics_root_state_version(),
    ),
    *author.as_backend().root(),
    Default::default(),
    Digest::default(),
  );
  let mut minimum_headroom = Weight::MAX;
  for number in first..=last {
    println!("construction block begins: preset={preset} block={number}");
    assert_eq!(*author.as_backend().root(), *parent.state_root());
    assert_eq!(*importer.as_backend().root(), *parent.state_root());
    let script = if number == first {
      epoch_script(
        &mut importer,
        &prepared.temporary,
        &prepared.temporary_contract,
      )
    } else {
      Vec::new()
    };
    let data = inherents(&parent, number);
    let recorder = Recorder::<Blake2Hasher>::default();
    author.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
      recorder.clone(),
    )));
    let (block, frame_used, headroom, raw) = author.execute_with_recorder(recorder.clone(), || {
      let header = Header::new(
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
      );
      Executive::initialize_block(&header);
      let mut extrinsics = <Runtime as BlockBuilder<Block>>::inherent_extrinsics(data.clone());
      assert!(extrinsics.len() >= 3);
      for extrinsic in &extrinsics {
        Executive::apply_extrinsic(extrinsic.clone())
          .unwrap()
          .unwrap();
      }
      for (index, (extrinsic, _, _, _)) in script.iter(/* deos-bypass: bounded-iter -- two fixed signed lifecycle calls. */).enumerate() {
        // Recorder snapshots inspect only already-recorded data, without priming reads.
        let before = call_footprint.then(|| {
          (actor_retention_fixture::recorded_keys(&recorder), recorder.to_storage_proof().encoded_size())
        });
        Executive::apply_extrinsic(extrinsic.clone())
          .unwrap()
          .unwrap();
        if let Some((before_keys, before_bytes)) = before {
          let after_keys = actor_retention_fixture::recorded_keys(&recorder);
          let mut new_keys = std::collections::BTreeMap::<String, Vec<String>>::new();
          for key in after_keys.difference(&before_keys) {
            let owner = storage_owners.as_ref().unwrap().get(key.get(..32).unwrap_or(key))
              .cloned().unwrap_or_else(|| "raw_or_unclassified".to_owned());
            new_keys.entry(owner).or_default().push(key.iter(/* deos-bypass: bounded-iter -- typed runtime keys are bounded by the 712-byte input contract. */).map(|byte| format!("{byte:02x}")).collect());
          }
          println!("{}", serde_json::json!({
            "scope": "native_prepared_signed_call_backend_footprint", "preset": preset,
            "block": number, "callIndex": index,
            "call": if index == 0 { "deactivate_actor" } else { "activate_actor" },
            "encodedExtrinsicBytes": extrinsic.encoded_size(),
            "declaredWeight": { "refTime": extrinsic.get_dispatch_info().total_weight().ref_time(), "proofSize": extrinsic.get_dispatch_info().total_weight().proof_size() },
            "recordedProofBeforeBytes": before_bytes,
            "recordedProofAfterBytes": recorder.to_storage_proof().encoded_size(),
            "newRecordedBackendKeyCount": after_keys.difference(&before_keys).count(),
            "newRecordedBackendKeys": new_keys,
            "scopeLimit": "New keys against the preceding block recorder; repeated and overlay-only reads are excluded. No maximum or physical coefficient."
          }));
        }
        extrinsics.push(extrinsic.clone());
      }
      let state = Actors::block_resource_state().unwrap();
      assert_eq!(state.outstanding_reservations(), 0);
      assert!(!state.optional_actor_work_halted());
      let frame_used = System::block_weight().total();
      let remaining = <Runtime as polkadot_sdk::frame_system::Config>::BlockWeights::get()
        .max_block
        .checked_sub(&frame_used)
        .unwrap();
      // Empty XCMP migration is the sole other configured OnIdle consumer. Reserve its
      // declaration even when tuple rotation puts it after Actors; this is a point check.
      let headroom = remaining
        .checked_sub(&deos_runtime::XcmpQueue::on_idle_weight())
        .unwrap()
        .min(<Runtime as pallet_deos_actors::Config>::ActorOnIdleReserve::get())
        .min(
          state
            .budget()
            .unwrap()
            .limits()
            .actor_control()
            .checked_sub(&state.usage().actor_control_used())
            .unwrap(),
        );
      let fixed = Weights::scheduler_on_idle_base()
        .checked_add(&Weights::block_resource_finalize())
        .unwrap();
      assert!(fixed.all_lte(headroom));
      let block = Block {
        header: Executive::finalize_block(),
        extrinsics,
      };
      (
        block,
        frame_used,
        headroom,
        recorder.estimate_encoded_size(),
      )
    });
    minimum_headroom = minimum_headroom.min(headroom);
    author.commit_all().unwrap();
    assert_eq!(*author.as_backend().root(), *block.header.state_root());
    importer.execute_with(|| {
      assert!(<Runtime as BlockBuilder<Block>>::check_inherents(block.clone().into(), data).ok());
    });
    let import_recorder = Recorder::<Blake2Hasher>::default();
    importer.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
      import_recorder.clone(),
    )));
    importer.execute_with_recorder(import_recorder, || {
      Executive::execute_block(block.clone().into());
    });
    importer.commit_all().unwrap();
    assert_eq!(*importer.as_backend().root(), *block.header.state_root());
    importer.execute_with(|| {
      assert_eq!(deos_runtime::Session::current_index(), 1);
      assert!(Actors::block_resource_state().is_none());
      let finalized = Actors::finalized_block_resource_telemetry().unwrap();
      assert_eq!(finalized.block_number(), number);
      assert!(!finalized.optional_actor_work_halted());
      assert!(ParkedBalanceEpisodes::<Runtime>::contains_key(
        prepared.target
      ));
      assert_eq!(
        Actors::actor_identity(prepared.target),
        Some(prepared.identity.clone())
      );
      assert_eq!(
        Actors::actor_contract(prepared.target),
        Some(prepared.contract.clone())
      );
      assert_eq!(
        DependencyScanSourceListState::<Runtime>::get().count,
        WATCHES as u32
      );
      assert!(pallet_deos_actors::ServiceHeader::<Runtime>::get().count <= 7);
      assert!(pallet_deos_actors::TriggerDeadlineHandles::<Runtime>::iter().count() <= 5);
      for (index, (actor, asset, source, contract)) in producers
        .iter() // deos-bypass: bounded-iter -- four fixed corpus producers
        .enumerate()
      {
        let state = Actors::active_actor_state(*actor).unwrap();
        assert_eq!(&state.contract, contract);
        let custody = &state.identity.sovereign_account;
        let balance = Ops::total_balance(custody, *asset);
        assert_eq!(Ops::balance(custody, *asset), balance);
        assert!(balance > 1);
        assert!(
          Ops::balance(custody, AssetKind::Native)
            >= <Runtime as pallet_deos_actors::Config>::MinUserBalance::get()
        );
        let revision = DependencyRevisions::<Runtime>::get(source).revision;
        let debit = previous[index].0.checked_sub(balance).unwrap();
        assert!(debit <= 1);
        if debit == 1 {
          transfers[index] += 1;
          assert!(revision > previous[index].1);
        }
        previous[index] = (balance, revision);
        assert_eq!(
          Ops::total_balance(&prepared.identity.sovereign_account, *asset),
          0
        );
      }
      assert_eq!(
        Balances::free_balance(&prepared.identity.sovereign_account),
        0
      );
    });
    for (name, state) in [("author", &mut author), ("import", &mut importer)] {
      if call_footprint {
        continue;
      }
      #[cfg(feature = "try-runtime")]
      state.execute_with(|| {
        println!("construction try-state begins: preset={preset} block={number} side={name}");
        <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(number).unwrap();
        println!("construction try-state passed: preset={preset} block={number} side={name}");
      });
      #[cfg(not(feature = "try-runtime"))]
      let _ = (name, state);
    }
    println!(
      "scope=native_prepared_negative_construction preset={preset} block={number} signed_calls={} frame_used={frame_used:?} drain_headroom={headroom:?} raw_block_bytes={raw} producer_transfers={transfers:?} parked_target=true roots_reconciled=true",
      script.len()
    );
    parent = block.header;
  }
  if call_footprint {
    println!(
      "scope=native_prepared_signed_call_footprint_complete preset={preset} blocks=1 minimum_drain_headroom={minimum_headroom:?} roots_reconciled=true; local role checks, no full try-state or maximum certificate"
    );
  } else {
    assert!(transfers.iter().all(|count| *count > 0)); // deos-bypass: bounded-iter -- four fixed corpus producers
    println!(
      "scope=native_prepared_negative_construction_complete preset={preset} blocks=9 minimum_drain_headroom={minimum_headroom:?} producer_transfers={transfers:?}; no maximum or public-history certificate"
    );
  }
}

fn main() {
  let args = std::env::args().collect::<Vec<_>>();
  if args.len() == 2 && args[1] == "--help" {
    println!(
      "Fixed-corpus native retention admission diagnostic.\nUsage: actor_retention_admission FIXED_PAGE_JSON [--rotation|--construction|--call-footprint [--preset=NAME]]\nRuns both runtime-owned presets unless one declared preset is selected. --rotation prepares the parent clock immediately before the scheduled session hook. --construction leaves watched custody unfunded and runs nine blocks from that hook, including a signed epoch and all four paid producers. --call-footprint separately records new backend keys of the signed deactivate/activate pair in one complete author/import block with local role checks, without the nine-block full try-state sweep. The positive suffix is bounded by 65 complete blocks per preset. Source topology and parent clock are synthetic preparation outside the encoded block suffix. No public construction history, physical maximum, compact PoV, RefTime or production-Wasm certificate is supplied."
    );
    return;
  }
  let rotation = args.get(2).is_some_and(|arg| arg == "--rotation");
  let construction = args.get(2).is_some_and(|arg| arg == "--construction");
  let call_footprint = args.get(2).is_some_and(|arg| arg == "--call-footprint");
  assert!(
    args.len() == 2
      || ((3..=4).contains(&args.len()) && (rotation || construction || call_footprint)),
    "supply the already fixed page JSON; see --help"
  );
  let selected = args.get(3).map(|arg| {
    arg
      .strip_prefix("--preset=")
      .expect("only --preset=NAME may follow the mode")
      .to_owned()
  });
  let presets = <Runtime as GenesisBuilder<Block>>::preset_names();
  if let Some(preset) = &selected {
    assert!(presets.contains(preset), "select a runtime-owned preset");
  }
  let pages = fixed_pages(&args[1]);
  println!(
    "scope=native_synthetic_retention_control input_sha256={INPUT_SHA256} actors_lib_sha256={} cargo_lock_sha256={}",
    digest(include_bytes!("../../pallets/actors/src/lib.rs")),
    digest(include_bytes!("../../Cargo.lock"))
  );
  for preset in presets {
    if selected.as_ref().is_some_and(|chosen| chosen != &preset) {
      continue;
    }
    println!("admission preparation starts: preset={preset}");
    if construction || call_footprint {
      let prepared = if call_footprint {
        actor_retention_fixture::prepare_committed(&preset, &pages)
      } else {
        actor_retention_fixture::prepare_with_target_funding(&preset, &pages, None, false)
      };
      execute_construction(&preset, prepared, call_footprint);
    } else {
      let prepared = prepare(&preset, &pages, None);
      execute(&preset, prepared, &pages, rotation);
    }
  }
  println!(
    "Selected native preset suffixes reconciled. Synthetic topology remains separate from public history and production resource certification."
  );
}
