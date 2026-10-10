//! Two signed permanent Close calls on the unchanged four-source retention corpus.
//! The topology and late parent clock are synthetic preparation, not public history.

mod actor_retention_corpus;
#[allow(
  dead_code,
  reason = "the shared fixture also supplies other retention diagnostic modes"
)]
mod actor_retention_fixture;
#[path = "actor_retention_close_pair/service_phase.rs"]
mod service_phase;
#[path = "actor_retention_close_pair/storage_observation.rs"]
mod storage_observation;

use actor_retention_corpus::{INPUT_SHA256, PAGES, WATCHES, digest, fixed_pages};
use actor_retention_fixture::{
  PREPARED_ACTIVE_ACTORS, PREPARED_IDENTITIES, Prepared, inherents, prepare_committed,
  recorded_keys, signed_lifecycle,
};
use codec::Encode;
use deos_runtime::{
  AccountId, Actors, Balances, Block, Executive, Header, Nonce, Runtime, RuntimeCall, RuntimeEvent,
  RuntimeHoldReason, RuntimeOrigin, System, TransactionPayment, UncheckedExtrinsic,
};
use pallet_deos_actors::{
  ActorContractTailChunks, ActorId, ActorProcesses, AssetOps, BlockResourcePhase, Config,
  DependencyPlans, DependencyRegistrationHeaders, DependencyRegistrationPages,
  DependencyRegistrationPositions, DependencyScanSourceListState, DependencySourceAllocatorState,
  DependencySourceBalances, DependencyTimedReviews, Event, HoldReason, OwnerSlotBitmaps,
  ParkedBalanceEpisodes, ServiceHeader, ServiceNodes, SovereignIndex, WeightInfo,
};
use polkadot_sdk::{
  frame_support::{
    assert_ok, dispatch::GetDispatchInfo, traits::tokens::fungible::InspectHold, weights::Weight,
  },
  sp_block_builder::runtime_decl_for_block_builder::BlockBuilder,
  sp_core::Blake2Hasher,
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
use primitives::AssetKind;
use service_phase::Predecessor;
use std::time::Instant;

type Ops = <Runtime as Config>::AssetOps;
type Weights = <Runtime as Config>::WeightInfo;

const BLOCK: u32 = 2_552_520_723;

struct CloseCall {
  actor: ActorId,
  owner: AccountId,
  custody: AccountId,
  nonce: Nonce,
  quote: u128,
  free_before: u128,
  held_before: u128,
  release: u128,
  owner_bitmap_after: [u8; 32],
  extrinsic: UncheckedExtrinsic,
}

fn prepare_boundary(state: &mut TestExternalities, prepared: &Prepared, parent: u32) {
  state.execute_with(|| {
    // A bounded preparation changes the parent clock and retires the active temporary.
    // It does not claim that the public recipe produced this late-height state.
    System::set_block_number(parent);
    pallet_governance::LastProcessedEpoch::<Runtime>::put(parent);
    assert_ok!(Actors::deactivate_actor(
      RuntimeOrigin::signed(Sr25519Keyring::Alice.to_account_id()),
      prepared.temporary[0],
    ));
    for temporary in prepared.temporary {
      assert!(Actors::actor_contract(temporary).is_none());
      assert!(!ParkedBalanceEpisodes::<Runtime>::contains_key(temporary));
      assert!(!ServiceNodes::<Runtime>::contains_key(temporary));
    }
    assert_eq!(
      (Actors::actor_identity_count(), Actors::active_actor_count()),
      (PREPARED_IDENTITIES, PREPARED_ACTIVE_ACTORS - 1)
    );
    assert_eq!(
      DependencyScanSourceListState::<Runtime>::get().count,
      WATCHES as u32
    );
  });
  state.commit_all().unwrap();
}

fn prepare_calls(state: &mut TestExternalities, prepared: &Prepared) -> [CloseCall; 2] {
  state.execute_with(|| {
    let reason = RuntimeHoldReason::Actors(HoldReason::ActorState);
    let calls = [prepared.target, prepared.target.checked_add(1).unwrap()].map(|actor| {
      let identity = Actors::actor_identity(actor).unwrap();
      assert_eq!(
        Actors::actor_contract(actor),
        Some(prepared.contract.clone())
      );
      assert!(Actors::actor_run_state(actor).is_none());
      assert!(ParkedBalanceEpisodes::<Runtime>::contains_key(actor));
      assert!(!ServiceNodes::<Runtime>::contains_key(actor));
      for tail in 0..3 {
        assert!(ActorContractTailChunks::<Runtime>::contains_key(
          actor, tail
        ));
      }
      assert_eq!(
        Ops::total_balance(&identity.sovereign_account, AssetKind::Native),
        0
      );
      for source in &prepared.sources {
        let asset = DependencySourceBalances::<Runtime>::get(source).unwrap();
        assert_eq!(Ops::total_balance(&identity.sovereign_account, asset), 0);
      }
      let hold = Actors::actor_state_hold(actor).unwrap().breakdown;
      let release =
        hold.identity + hold.contract_head + hold.contract_body + hold.detector + hold.run;
      let held_before =
        <Balances as InspectHold<AccountId>>::balance_on_hold(&reason, &identity.owner);
      assert!(release > 0 && held_before >= release);
      let mut owner_bitmap_after = OwnerSlotBitmaps::<Runtime>::get(&identity.owner);
      let slot = identity.actor_class.owner_slot().unwrap();
      let mask = 1u8 << (slot % 8);
      assert_ne!(owner_bitmap_after[usize::from(slot / 8)] & mask, 0);
      owner_bitmap_after[usize::from(slot / 8)] &= !mask;
      let pair = Sr25519Keyring::well_known()
        .find(|key| key.to_account_id() == identity.owner)
        .unwrap()
        .pair();
      let extrinsic = signed_lifecycle(
        &pair,
        RuntimeCall::Actors(pallet_deos_actors::Call::close_actor { actor_id: actor }),
      );
      let quote = TransactionPayment::compute_fee(
        u32::try_from(extrinsic.encoded_size()).unwrap(),
        &extrinsic.get_dispatch_info(),
        0,
      );
      CloseCall {
        actor,
        nonce: System::account_nonce(&identity.owner),
        quote,
        free_before: Balances::free_balance(&identity.owner),
        held_before,
        release,
        owner_bitmap_after,
        extrinsic,
        owner: identity.owner,
        custody: identity.sovereign_account,
      }
    });
    assert_ne!(calls[0].owner, calls[1].owner);
    assert!(Actors::block_resource_state().is_none());
    calls
  })
}

fn frame_json(weight: Weight) -> serde_json::Value {
  serde_json::json!({ "refTime": weight.ref_time(), "proofSize": weight.proof_size() })
}

fn verify_closed(calls: &[CloseCall; 2], prepared: &Prepared, pages: &[Vec<u64>]) {
  let reason = RuntimeHoldReason::Actors(HoldReason::ActorState);
  assert_eq!(
    (Actors::actor_identity_count(), Actors::active_actor_count()),
    (PREPARED_IDENTITIES - 2, PREPARED_ACTIVE_ACTORS - 3)
  );
  assert!(Actors::block_resource_state().is_none());
  let telemetry = Actors::finalized_block_resource_telemetry().unwrap();
  assert_eq!(telemetry.block_number(), BLOCK);
  assert!(!telemetry.optional_actor_work_halted());
  assert_eq!(ServiceHeader::<Runtime>::get().round_block, Some(BLOCK));
  assert!(ServiceHeader::<Runtime>::get().count <= 7);
  let events = System::events();
  for call in calls {
    assert!(Actors::actor_identity(call.actor).is_none());
    assert!(Actors::actor_state_hold(call.actor).is_none());
    assert!(Actors::actor_contract(call.actor).is_none());
    assert!(Actors::actor_run_state(call.actor).is_none());
    assert!(!ActorProcesses::<Runtime>::contains_key(call.actor));
    assert!(!ServiceNodes::<Runtime>::contains_key(call.actor));
    assert!(!SovereignIndex::<Runtime>::contains_key(&call.custody));
    assert!(!ParkedBalanceEpisodes::<Runtime>::contains_key(call.actor));
    assert!(!DependencyTimedReviews::<Runtime>::contains_key(call.actor));
    assert!(DependencyPlans::<Runtime>::get(call.actor).is_empty());
    for tail in 0..3 {
      assert!(!ActorContractTailChunks::<Runtime>::contains_key(
        call.actor, tail
      ));
    }
    assert_eq!(
      OwnerSlotBitmaps::<Runtime>::get(&call.owner),
      call.owner_bitmap_after
    );
    assert_eq!(
      <Balances as InspectHold<AccountId>>::balance_on_hold(&reason, &call.owner),
      call.held_before - call.release
    );
    assert_eq!(System::account_nonce(&call.owner), call.nonce + 1);
    assert_eq!(Ops::total_balance(&call.custody, AssetKind::Native), 0);
    for source in &prepared.sources {
      let asset = DependencySourceBalances::<Runtime>::get(source).unwrap();
      assert_eq!(Ops::total_balance(&call.custody, asset), 0);
    }
    assert!(events.iter(/* deos-bypass: bounded-iter -- the five-extrinsic diagnostic block. */).any(|record| {
      matches!(record.event, RuntimeEvent::Actors(Event::ActorClosed { actor_id, .. }) if actor_id == call.actor)
    }));
    let fees = events.iter(/* deos-bypass: bounded-iter -- two signed Close fee events. */).filter_map(|record| {
      match &record.event {
        RuntimeEvent::TransactionPayment(polkadot_sdk::pallet_transaction_payment::Event::TransactionFeePaid { who, actual_fee, tip }) if who == &call.owner => {
          assert_eq!(*tip, 0);
          Some(*actual_fee)
        },
        _ => None,
      }
    }).collect::<Vec<_>>();
    assert_eq!(fees.len(), 1);
    assert!(fees[0] <= call.quote);
    assert_eq!(
      Balances::free_balance(&call.owner),
      call.free_before + call.release - fees[0]
    );
  }
  assert_eq!(
    DependencySourceAllocatorState::<Runtime>::get().encode(),
    prepared.allocator
  );
  assert_eq!(
    DependencyScanSourceListState::<Runtime>::get().count,
    WATCHES as u32
  );
  for (index, source) in prepared.sources.iter(/* deos-bypass: bounded-iter -- four fixed watched sources. */).copied().enumerate()
  {
    let header = DependencyRegistrationHeaders::<Runtime>::get(source);
    assert_eq!(
      (header.count, header.free_count),
      (PAGES as u32 - 2, 31 * PAGES as u64 - 62)
    );
    for call in calls {
      assert!(!DependencyRegistrationPositions::<Runtime>::contains_key(
        source, call.actor
      ));
    }
    for cohort in [0, PAGES / 2] {
      assert!(!DependencyRegistrationPages::<Runtime>::contains_key(
        source,
        pages[index][cohort]
      ));
    }
    assert_eq!(
      DependencyRegistrationPages::<Runtime>::get(source, pages[index][1])
        .unwrap()
        .previous,
      None
    );
    assert_eq!(
      DependencyRegistrationPages::<Runtime>::get(source, pages[index][PAGES / 2 - 1])
        .unwrap()
        .next,
      Some(pages[index][PAGES / 2 + 1])
    );
    assert_eq!(
      DependencyRegistrationPages::<Runtime>::get(source, pages[index][PAGES / 2 + 1])
        .unwrap()
        .previous,
      Some(pages[index][PAGES / 2 - 1])
    );
  }
}

fn execute_pair(preset: &PresetId, mut prepared: Prepared, pages: &[Vec<u64>], mode: Predecessor) {
  let phase = mode != Predecessor::Cold;
  let mut state = prepared.committed.take().unwrap();
  prepared.storage.top.clear();
  prepare_boundary(&mut state, &prepared, BLOCK - 1 - mode.blocks());
  let observed_parent = phase.then(|| service_phase::advance(&mut state, &prepared, preset, mode));
  let calls = prepare_calls(&mut state, &prepared);
  println!(
    "pair prepared: preset={preset} block={BLOCK} actors={:?} owners={:?}",
    calls.each_ref().map(|call| call.actor),
    calls.each_ref().map(|call| &call.owner)
  );
  state.commit_all().unwrap();
  let mut author = TestExternalities::new_empty();
  let mut importer = TestExternalities::new_empty();
  author.state_version = state.state_version;
  importer.state_version = state.state_version;
  author.backend = state.backend;
  importer.backend = author.backend.clone();
  let parent_root = *author.backend.root();
  assert_eq!(*importer.backend.root(), parent_root);
  let parent = observed_parent.unwrap_or_else(|| {
    Header::new(
      BLOCK - 1,
      BlakeTwo256::ordered_trie_root(
        Vec::new(),
        deos_runtime::VERSION.extrinsics_root_state_version(),
      ),
      parent_root,
      Default::default(),
      Digest::default(),
    )
  });
  assert_eq!(*parent.state_root(), parent_root);
  let data = inherents(&parent, BLOCK);
  storage_observation::record_pre_state(&author, preset);
  let recorder = Recorder::<Blake2Hasher>::default();
  author.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
    recorder.clone(),
  )));
  println!("pair author begins: preset={preset} block={BLOCK}");
  let started = Instant::now();
  let (block, p0, drain_headroom) = author.execute_with_recorder(recorder.clone(), || {
    let header = Header::new(
      BLOCK,
      Default::default(),
      Default::default(),
      parent.hash(),
      Digest { logs: vec![DigestItem::PreRuntime(polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID, u64::from(BLOCK).encode())] },
    );
    Executive::initialize_block(&header);
    let mut extrinsics = <Runtime as BlockBuilder<Block>>::inherent_extrinsics(data.clone());
    assert_eq!(extrinsics.len(), 3);
    for inherent in &extrinsics {
      Executive::apply_extrinsic(inherent.clone()).unwrap().unwrap();
    }
    let initial_state = Actors::block_resource_state().unwrap();
    assert_eq!(initial_state.phase(), BlockResourcePhase::ExternalPhase);
    assert_eq!(initial_state.outstanding_reservations(), 0);
    assert!(!initial_state.optional_actor_work_halted());
    assert_eq!(initial_state.usage().user_dispatch_used(), Weight::zero());
    let budget = initial_state.budget().unwrap();
    let round = ServiceHeader::<Runtime>::get();
    assert_eq!(round.round_block, Some(BLOCK));
    assert!(round.count <= 7);
    if phase {
      assert_eq!(round.count, 7, "the Service-phase mode must witness the full cohort");
      assert_ne!(initial_state.usage().actor_effect_used(), Weight::zero(), "the phase must contain a real Actor Action");
    }
    let p0 = System::block_weight().total();
    println!("{}", serde_json::json!({ "scope": "prepared_two_owner_close_p0", "preset": preset, "block": BLOCK, "frame": frame_json(p0), "recordedProofBytes": recorder.to_storage_proof().encoded_size(), "recordedBackendKeys": storage_observation::key_image(&recorded_keys(&recorder)), "serviceRound": format!("{round:?}"), "resourceState": format!("{initial_state:?}") }));
    for (index, call) in calls.iter(/* deos-bypass: bounded-iter -- exactly two signed Close calls. */).enumerate() {
      let before_keys = recorded_keys(&recorder);
      let before_proof = recorder.to_storage_proof().encoded_size();
      let before_frame = System::block_weight().total();
      println!("pair signed Close begins: preset={preset} index={index} actor={}", call.actor);
      Executive::apply_extrinsic(call.extrinsic.clone()).unwrap().unwrap();
      extrinsics.push(call.extrinsic.clone());
      let resource = Actors::block_resource_state().unwrap();
      assert_eq!(resource.phase(), BlockResourcePhase::ExternalPhase);
      assert_eq!(resource.outstanding_reservations(), 0);
      assert!(!resource.optional_actor_work_halted());
      assert_eq!(resource.budget().unwrap(), budget);
      assert_eq!(resource.usage().actor_control_used(), initial_state.usage().actor_control_used());
      assert_eq!(resource.usage().actor_effect_used(), initial_state.usage().actor_effect_used());
      assert_eq!(ServiceHeader::<Runtime>::get(), round);
      let after_keys = recorded_keys(&recorder);
      println!("{}", serde_json::json!({ "scope": "prepared_two_owner_signed_close", "preset": preset, "block": BLOCK, "callIndex": index, "actor": call.actor, "encodedExtrinsicBytes": call.extrinsic.encoded_size(), "declaredWeight": frame_json(call.extrinsic.get_dispatch_info().total_weight()), "frameBefore": frame_json(before_frame), "frameAfter": frame_json(System::block_weight().total()), "recordedProofBeforeBytes": before_proof, "recordedProofAfterBytes": recorder.to_storage_proof().encoded_size(), "newRecordedBackendKeys": after_keys.difference(&before_keys).map(|key| polkadot_sdk::sp_core::hexdisplay::HexDisplay::from(key).to_string()).collect::<Vec<_>>(), "recordedBackendKeys": storage_observation::key_image(&after_keys), "resourceState": format!("{resource:?}") }));
    }
    let resource = Actors::block_resource_state().unwrap();
    let remaining = <Runtime as polkadot_sdk::frame_system::Config>::BlockWeights::get().max_block.checked_sub(&System::block_weight().total()).unwrap();
    let headroom = remaining.checked_sub(&deos_runtime::XcmpQueue::on_idle_weight()).unwrap()
      .min(<Runtime as Config>::ActorOnIdleReserve::get())
      .min(budget.limits().actor_control().checked_sub(&resource.usage().actor_control_used()).unwrap());
    let housekeeping = Weights::scheduler_on_idle_base().checked_add(&Weights::block_resource_finalize()).unwrap();
    assert!(housekeeping.all_lte(headroom));
    println!("pair finalization begins: preset={preset} drain_headroom={headroom:?}");
    let block = Block { header: Executive::finalize_block(), extrinsics };
    (block, p0, headroom)
  });
  let author_seconds = started.elapsed().as_secs_f64();
  let author_proof_bytes = recorder.to_storage_proof().encoded_size();
  let author_provider_bytes = recorder.estimate_encoded_size();
  assert!(author_proof_bytes as u64 <= deos_runtime::MAXIMUM_BLOCK_WEIGHT.proof_size());
  let author_delta = storage_observation::record_delta(&author, preset, "author", &recorder);
  println!("pair author commit begins: preset={preset} proof_bytes={author_proof_bytes}");
  author.commit_all().unwrap();
  assert_eq!(*author.backend.root(), *block.header.state_root());
  importer.execute_with(|| {
    assert!(<Runtime as BlockBuilder<Block>>::check_inherents(block.clone().into(), data).ok())
  });
  let import_recorder = Recorder::<Blake2Hasher>::default();
  importer.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
    import_recorder.clone(),
  )));
  println!("pair import begins: preset={preset} block={BLOCK}");
  let started = Instant::now();
  importer.execute_with_recorder(import_recorder.clone(), || {
    Executive::execute_block(block.clone().into())
  });
  let import_seconds = started.elapsed().as_secs_f64();
  let import_proof_bytes = import_recorder.to_storage_proof().encoded_size();
  assert!(import_proof_bytes as u64 <= deos_runtime::MAXIMUM_BLOCK_WEIGHT.proof_size());
  let import_delta =
    storage_observation::record_delta(&importer, preset, "import", &import_recorder);
  assert_eq!(author_delta, import_delta);
  println!("pair import commit begins: preset={preset} proof_bytes={import_proof_bytes}");
  importer.commit_all().unwrap();
  assert_eq!(*importer.backend.root(), *block.header.state_root());
  for state in [&mut author, &mut importer] {
    state.execute_with(|| verify_closed(&calls, &prepared, pages));
    if phase {
      state.execute_with(|| service_phase::verify_members(&prepared));
    }
    if mode == Predecessor::NormalizedReviews {
      state.execute_with(|| service_phase::verify_review_geometry(&prepared, true));
    }
  }
  println!(
    "{}",
    serde_json::json!({ "scope": "prepared_two_owner_full_corpus_close_complete", "preset": preset, "block": BLOCK, "servicePhase": phase, "reviewNormalized": mode == Predecessor::NormalizedReviews, "warmupBlocks": mode.blocks(), "inputSha256": INPUT_SHA256, "identitiesBefore": PREPARED_IDENTITIES, "permanentCohortsBefore": PAGES, "sources": WATCHES, "signedCloseCalls": 2, "authorProofBytes": author_proof_bytes, "authorProviderEstimateBytes": author_provider_bytes, "importProofBytes": import_proof_bytes, "p0": frame_json(p0), "drainHeadroom": frame_json(drain_headroom), "authorSeconds": author_seconds, "importSeconds": import_seconds, "stateRoot": format!("{:?}", block.header.state_root()), "rootsReconciled": true, "scopeLimit": "Finite native author/import suffix on synthetic initial topology and clocks. No authenticated public history, repetition invariant, physical maximum, tariff, host/PVF or production binding certificate." })
  );
}

fn main() {
  let args = std::env::args().collect::<Vec<_>>();
  if args.len() == 2 && args[1] == "--help" {
    println!(
      "Fixed-corpus two-owner signed Close diagnostic.\nUsage: actor_retention_close_pair FIXED_PAGE_JSON [--service-phase|--review-normalized] [--preset=NAME]\nRuns the final two-owner Close block at 2552520723 on the exact four-source corpus, 9968 permanent cohorts and twelve-Step Contracts. --service-phase runs twenty complete author/import predecessor blocks; --review-normalized runs 9988, requiring ordinary negative reviews to retire the old shared bucket and leave page-zero Block buckets with at most three members. Both modes seed two ordinary signed Native ingresses and require seven Service members plus a real Actor Action at P0. The default retains the cold one-block point. Initial topology and parent/Governance clocks are synthetic; preparation, signatures and quotes remain outside recording. Per-block logs expose progress. No literal construction history, repetition, physical maximum, tariffs or production-Wasm/host certificate is supplied."
    );
    return;
  }
  assert!(
    (2..=4).contains(&args.len()),
    "supply the already fixed page JSON; see --help"
  );
  let mode = match args.get(2).map(String::as_str) {
    Some("--service-phase") => Predecessor::Service,
    Some("--review-normalized") => Predecessor::NormalizedReviews,
    _ => Predecessor::Cold,
  };
  let phase = mode != Predecessor::Cold;
  assert!(
    phase || args.len() <= 3,
    "only a predecessor mode may precede a preset"
  );
  let selected = args.get(if phase { 3 } else { 2 }).map(|arg| {
    arg
      .strip_prefix("--preset=")
      .expect("only --preset=NAME may follow the corpus")
      .to_owned()
  });
  let presets = <Runtime as GenesisBuilder<Block>>::preset_names();
  if let Some(preset) = &selected {
    assert!(presets.contains(preset), "select a runtime-owned preset");
  }
  let pages = fixed_pages(&args[1]);
  println!(
    "scope=prepared_two_owner_close input_sha256={INPUT_SHA256} diagnostic_sha256={} cargo_lock_sha256={}",
    digest(include_bytes!("actor_retention_close_pair.rs")),
    digest(include_bytes!("../../Cargo.lock"))
  );
  for preset in presets {
    if selected.as_ref().is_some_and(|chosen| chosen != &preset) {
      continue;
    }
    println!("pair preparation begins: preset={preset}");
    execute_pair(&preset, prepare_committed(&preset, &pages), &pages, mode);
  }
}
