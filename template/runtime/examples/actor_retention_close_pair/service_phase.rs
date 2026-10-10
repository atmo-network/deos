//! A bounded native predecessor selecting the seven-member Service cohort.

use super::{
  Actors, Blake2Hasher, BlakeTwo256, Block, BlockBuilder, Digest, DigestItem, Encode, Executive,
  Hash, Header, HeaderT, Ops, Prepared, PresetId, ProofSizeExt, Recorder,
  RecordingProofSizeProvider, Runtime, RuntimeCall, Sr25519Keyring, TestExternalities,
  UncheckedExtrinsic, WATCHES, inherents, signed_lifecycle,
};
use pallet_deos_actors::{
  ActorId, ActorType, AssetOps, BalanceDependencySources, BlockResourcePhase, DeadlineHandles,
  DeadlineHeaders, DeadlineIndexLen, DeadlineIndexPositions, DeadlinePages, DependencyRevisions,
  DependencyScanSourceListState, DependencyTimedReviews, ServiceHeader, ServiceNodes, Task,
  Trigger, WakeupClock, WakeupKey,
};
use primitives::{AssetKind, ecosystem::actor_ids};

pub(super) const WARMUP_BLOCKS: u32 = 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Predecessor {
  Cold,
  Service,
  NormalizedReviews,
}

impl Predecessor {
  pub(super) fn blocks(self) -> u32 {
    match self {
      Self::Cold => 0,
      Self::Service => WARMUP_BLOCKS,
      Self::NormalizedReviews => super::PAGES as u32 + WARMUP_BLOCKS,
    }
  }
}

pub(super) fn verify_review_geometry(prepared: &Prepared, closed: bool) {
  let old = WakeupKey::Block(2);
  assert!(!DeadlineHeaders::<Runtime>::contains_key(old));
  assert!(!DeadlineIndexPositions::<Runtime>::contains_key(old));
  for page in 0..super::PAGES.div_ceil(32) as u64 {
    assert!(!DeadlinePages::<Runtime>::contains_key(old, page));
  }
  let expected = super::PAGES as u32 - if closed { 2 } else { 0 };
  let heap_entries = DeadlineIndexLen::<Runtime>::get(WakeupClock::Block);
  assert!(heap_entries <= expected && heap_entries >= expected.div_ceil(3));
  assert!(DeadlineIndexLen::<Runtime>::get(WakeupClock::Tick) <= 5);
  let mut buckets = std::collections::BTreeMap::<u32, u32>::new();
  // The fixed 9,968-identity diagnostic audit runs outside recorded block execution.
  for actor in prepared.target..u64::from(super::PREPARED_IDENTITIES) {
    if closed && (actor == prepared.target || actor == prepared.target + 1) {
      continue;
    }
    let review = DependencyTimedReviews::<Runtime>::get(actor).unwrap();
    let handle = DeadlineHandles::<Runtime>::get(actor).unwrap();
    assert_eq!(review.owner.actor, handle.actor);
    assert_eq!(handle.actor.actor_id, actor);
    assert_eq!(review.deadline, handle.key);
    let WakeupKey::Block(due) = handle.key else {
      panic!("every fixed permanent retains a Block review")
    };
    assert!(due <= super::BLOCK + if closed { 1 } else { 0 });
    *buckets.entry(due).or_default() += 1;
    assert_eq!(handle.page, 0);
    let header = DeadlineHeaders::<Runtime>::get(handle.key).unwrap();
    assert!((1..=3).contains(&header.count));
    assert_eq!(header.page_count, 1);
    assert_eq!(
      (header.first_page, header.last_page, header.next_page),
      (0, 0, 1)
    );
    assert_eq!(header.first_vacant_page, Some(0));
    assert!(DeadlineIndexPositions::<Runtime>::get(handle.key).unwrap() < heap_entries);
    let page = DeadlinePages::<Runtime>::get(handle.key, 0).unwrap();
    assert_eq!((page.previous_page, page.next_page), (None, None));
    assert_eq!(
      (page.previous_vacant_page, page.next_vacant_page),
      (None, None)
    );
    assert_eq!(u32::from(page.live_entries), header.count);
    assert_eq!(page.entries[usize::from(handle.slot)], Some(handle.actor));
    assert_eq!(
      page.entries.into_iter().flatten().count(),
      header.count as usize
    );
  }
  assert_eq!(buckets.len(), heap_entries as usize);
  for (due, count) in &buckets {
    assert_eq!(
      DeadlineHeaders::<Runtime>::get(WakeupKey::Block(*due))
        .unwrap()
        .count,
      *count
    );
  }
  println!(
    "{}",
    serde_json::json!({ "scope": "native_bounded_review_geometry", "closed": closed, "reviewActors": expected, "blockHeapEntries": heap_entries, "maximumBucketMembers": buckets.into_values().max().unwrap(), "oldReviewBucketAbsent": true, "singlePageZeroBuckets": true, "source": "Ordinary negative review quanta; no Deadline state injection." })
  );
}

fn producer_ids(prepared: &Prepared) -> [ActorId; WATCHES] {
  let first = prepared.target.checked_sub(WATCHES as u64 + 2).unwrap();
  std::array::from_fn(|index| first.checked_add(index as u64).unwrap())
}

pub(super) fn verify_members(prepared: &Prepared) {
  assert_eq!(ServiceHeader::<Runtime>::get().count, 7);
  for actor in [
    actor_ids::BURN_ACTOR_ID,
    actor_ids::FEE_SINK_ACTORS_ID,
    actor_ids::BLDR_SPLITTER_ACTORS_ID,
  ]
  .into_iter()
  .chain(producer_ids(prepared))
  {
    let identity = Actors::actor_identity(actor).unwrap();
    assert_eq!(
      identity.actor_class.actor_type(),
      if actor < prepared.target - WATCHES as u64 - 2 {
        ActorType::System
      } else {
        ActorType::User
      }
    );
    assert!(ServiceNodes::<Runtime>::contains_key(actor));
  }
  assert!(BalanceDependencySources::<Runtime>::get(AssetKind::Native).is_none());
}

fn seed_calls(state: &mut TestExternalities) -> [UncheckedExtrinsic; 2] {
  // Different wallets avoid changing either nonce while both payloads are prepared.
  state.execute_with(|| {
    [
      (Sr25519Keyring::Alice, actor_ids::BURN_ACTOR_ID),
      (Sr25519Keyring::Bob, actor_ids::BLDR_SPLITTER_ACTORS_ID),
    ]
    .map(|(signer, actor)| {
      let custody = Actors::actor_identity(actor).unwrap().sovereign_account;
      signed_lifecycle(
        &signer.pair(),
        RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
          dest: polkadot_sdk::sp_runtime::MultiAddress::Id(custody),
          value: 1,
        }),
      )
    })
  })
}

pub(super) fn advance(
  author: &mut TestExternalities,
  prepared: &Prepared,
  preset: &PresetId,
  mode: Predecessor,
) -> Header {
  let blocks = mode.blocks();
  assert!(blocks > 0 && blocks <= super::PAGES as u32 + WARMUP_BLOCKS);
  let normalize = mode == Predecessor::NormalizedReviews;
  let first = super::BLOCK - blocks;
  if normalize {
    author.execute_with(|| {
      assert_eq!(
        DeadlineHeaders::<Runtime>::get(WakeupKey::Block(2))
          .unwrap()
          .count,
        super::PAGES as u32
      );
    });
  }
  let mut importer = TestExternalities::new_empty();
  importer.state_version = author.state_version;
  importer.backend = author.backend.clone();
  let mut parent = Header::new(
    first - 1,
    BlakeTwo256::ordered_trie_root(
      Vec::new(),
      deos_runtime::VERSION.extrinsics_root_state_version(),
    ),
    *author.backend.root(),
    Default::default(),
    Digest::default(),
  );
  let producers = producer_ids(prepared);
  let mut previous = author.execute_with(|| {
    producers.map(|actor| {
      let state = Actors::active_actor_state(actor).unwrap();
      assert_eq!(state.contract.trigger, Trigger::cadenced(1));
      assert_eq!(state.identity.actor_class.actor_type(), ActorType::User);
      assert_eq!(state.contract.steps.len(), 1);
      let Task::Transfer { asset, .. } = state.contract.steps[0].task else {
        panic!("the fixed producer must retain its Transfer")
      };
      assert_eq!(
        BalanceDependencySources::<Runtime>::get(asset),
        Some(prepared.sources[(actor - producers[0]) as usize])
      );
      let native = Ops::total_balance(&state.identity.sovereign_account, AssetKind::Native);
      let lp = Ops::total_balance(&state.identity.sovereign_account, asset);
      let revision =
        DependencyRevisions::<Runtime>::get(prepared.sources[(actor - producers[0]) as usize])
          .revision;
      (native, lp, revision)
    })
  });
  let mut transfers = [0u32; WATCHES];
  let mut maximum_service = 0u32;
  for number in first..super::BLOCK {
    let started = std::time::Instant::now();
    println!("Service predecessor begins: preset={preset} block={number}");
    assert_eq!(*author.backend.root(), *parent.state_root());
    assert_eq!(*importer.backend.root(), *parent.state_root());
    let script = if number == first {
      seed_calls(author).to_vec()
    } else {
      Vec::new()
    };
    let data = inherents(&parent, number);
    let recorder = Recorder::<Blake2Hasher>::default();
    author.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
      recorder.clone(),
    )));
    let block = author.execute_with_recorder(recorder.clone(), || {
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
      assert_eq!(extrinsics.len(), 3);
      for inherent in &extrinsics {
        Executive::apply_extrinsic(inherent.clone())
          .unwrap()
          .unwrap();
      }
      for extrinsic in &script {
        Executive::apply_extrinsic(extrinsic.clone())
          .unwrap()
          .unwrap();
        extrinsics.push(extrinsic.clone());
      }
      let resource = Actors::block_resource_state().unwrap();
      assert_eq!(resource.phase(), BlockResourcePhase::ExternalPhase);
      assert_eq!(resource.outstanding_reservations(), 0);
      assert!(!resource.optional_actor_work_halted());
      Block {
        header: Executive::finalize_block(),
        extrinsics,
      }
    });
    author.commit_all().unwrap();
    assert_eq!(*author.backend.root(), *block.header.state_root());
    let author_proof = recorder.to_storage_proof().encoded_size();
    assert!(author_proof as u64 <= deos_runtime::MAXIMUM_BLOCK_WEIGHT.proof_size());
    importer.execute_with(|| {
      assert!(<Runtime as BlockBuilder<Block>>::check_inherents(block.clone().into(), data).ok())
    });
    let import_recorder = Recorder::<Blake2Hasher>::default();
    importer.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
      import_recorder.clone(),
    )));
    importer.execute_with_recorder(import_recorder.clone(), || {
      Executive::execute_block(block.clone().into())
    });
    importer.commit_all().unwrap();
    assert_eq!(*importer.backend.root(), *block.header.state_root());
    let import_proof = import_recorder.to_storage_proof().encoded_size();
    assert!(import_proof as u64 <= deos_runtime::MAXIMUM_BLOCK_WEIGHT.proof_size());
    let (service, block_heap, tick_heap, old_review_count) = author.execute_with(|| {
      assert!(Actors::block_resource_state().is_none());
      assert!(
        !Actors::finalized_block_resource_telemetry()
          .unwrap()
          .optional_actor_work_halted()
      );
      assert_eq!(
        Actors::active_actor_count(),
        super::PREPARED_ACTIVE_ACTORS - 1
      );
      assert_eq!(
        DependencyScanSourceListState::<Runtime>::get().count,
        WATCHES as u32
      );
      for (index, actor) in producers.into_iter().enumerate() {
        let state = Actors::active_actor_state(actor).unwrap();
        let Task::Transfer { asset, .. } = state.contract.steps[0].task else {
          panic!("producer task changed")
        };
        let native = Ops::total_balance(&state.identity.sovereign_account, AssetKind::Native);
        let lp = Ops::total_balance(&state.identity.sovereign_account, asset);
        let revision = DependencyRevisions::<Runtime>::get(prepared.sources[index]).revision;
        let debit = previous[index].1.checked_sub(lp).unwrap();
        assert!(debit <= 1, "one committed Step at most per producer/block");
        if debit == 1 {
          assert!(native < previous[index].0, "the LP effect must be paid");
          assert!(
            revision > previous[index].2,
            "the LP transfer must publish its watched source"
          );
          transfers[index] = transfers[index].checked_add(1).unwrap();
        }
        assert!(lp > 1);
        previous[index] = (native, lp, revision);
      }
      let service = ServiceHeader::<Runtime>::get();
      assert!(service.count <= 7);
      (
        service,
        DeadlineIndexLen::<Runtime>::get(WakeupClock::Block),
        DeadlineIndexLen::<Runtime>::get(WakeupClock::Tick),
        DeadlineHeaders::<Runtime>::get(WakeupKey::Block(2)).map(|header| header.count),
      )
    });
    if normalize {
      assert_eq!(
        old_review_count.unwrap_or(0),
        (super::PAGES as u32).saturating_sub(number - first + 1),
        "one ordinary review must retire one old-bucket member per block"
      );
      assert!(block_heap <= super::PAGES as u32 + 1);
      assert!(tick_heap <= 5);
    }
    maximum_service = maximum_service.max(service.count);
    println!(
      "{}",
      serde_json::json!({ "scope": "native_service_predecessor_block", "preset": preset, "block": number, "signedCalls": script.len(), "authorProofBytes": author_proof, "importProofBytes": import_proof, "rootsReconciled": true, "service": format!("{service:?}"), "blockHeapEntries": block_heap, "tickHeapEntries": tick_heap, "oldReviewBucketCount": old_review_count, "producerTransfers": transfers, "elapsedSeconds": started.elapsed().as_secs_f64() })
    );
    parent = block.header;
  }
  assert!(transfers.into_iter().all(|count| count > 0));
  author.execute_with(|| verify_members(prepared));
  if normalize {
    author.execute_with(|| verify_review_geometry(prepared, false));
  }
  println!(
    "{}",
    serde_json::json!({ "scope": "native_service_predecessor_complete", "preset": preset, "blocks": blocks, "reviewNormalized": normalize, "maximumServiceMembers": maximum_service, "finalServiceMembers": 7, "producerTransfers": transfers, "rootsReconciled": true, "scopeLimit": "Finite executable suffix from a synthetic initial state with two preparatory Native ingresses. Review normalization uses ordinary quanta; no authenticated construction horizon or physical maximum." })
  );
  parent
}
