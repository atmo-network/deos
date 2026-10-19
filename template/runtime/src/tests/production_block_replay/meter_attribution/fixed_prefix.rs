//! Complete native caller capture, not a P/R price or a production-host transfer.

use super::*;
use pallet_deos_actors::{
  BlockResourceBudget,
  benchmark_observation::{self, Phase},
};
use polkadot_sdk::{
  sp_externalities::{
    Extensions, Externalities, set_and_run_with_externalities, with_externalities,
  },
  sp_runtime::StateVersion,
  sp_state_machine::{
    Backend, Ext, OverlayedChanges, TrieBackendBuilder, create_proof_check_backend,
  },
};
use std::panic::{AssertUnwindSafe, catch_unwind};

type Host = sc_client_db::BenchmarkingState<BlakeTwo256>;
type Db = (u32, u32, u32, u32);

struct Boundary {
  name: &'static str,
  proof: StorageProof,
  db: Db,
}

#[derive(Clone)]
struct EntryInput {
  staged_key: Vec<u8>,
  stored_extrinsic: Vec<u8>,
  declaration: Weight,
  retained_overhead: Weight,
  encoded_bytes: usize,
  quarter: Weight,
  authorship_key: Vec<u8>,
  timestamp_key: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
struct EntryReceipt {
  frame: Weight,
  provisional_prepass: Weight,
  retained_base_and_bytes: Weight,
  declared_prefix: Result<Weight, super::super::freeze_entry::Refusal>,
  authorship_cached: bool,
  timestamp_pending: bool,
}

struct Capture {
  recorder: Recorder<BlakeTwo256>,
  boundaries: Vec<Boundary>,
  active: Option<Phase>,
  budget: Option<BlockResourceBudget>,
  expected_block: crate::BlockNumber,
  entry_input: Option<EntryInput>,
  prepass_frame: Option<Weight>,
  receipt: Option<EntryReceipt>,
  regions: Vec<&'static str>,
  clock_span: Option<super::Span>,
  inclusive_clock_span: Option<super::Span>,
}

thread_local! {
  static CAPTURE: RefCell<Option<Capture>> = const { RefCell::new(None) };
}

fn region(name: &'static str) {
  CAPTURE.with_borrow_mut(|capture| {
    let Some(capture) = capture else { return };
    assert!(capture.regions.len() < 16);
    drop(capture.clock_span.take());
    capture.regions.push(name);
    capture.clock_span = super::span(name);
  });
}

fn boundary(name: &'static str) {
  CAPTURE.with_borrow_mut(|capture| {
    let Some(capture) = capture else { return };
    assert!(capture.boundaries.len() < 16);
    capture.boundaries.push(Boundary {
      name,
      proof: capture.recorder.to_storage_proof(),
      db: with_externalities(|ext| ext.read_write_count()).unwrap(),
    });
  });
}

pub(crate) fn observe_freeze(now: crate::BlockNumber, configured: BlockResourceBudget) {
  let (enabled, input) = CAPTURE.with_borrow_mut(|capture| {
    let Some(capture) = capture else {
      return (false, None);
    };
    assert_eq!(
      now, capture.expected_block,
      "capture is bound to its exact input block"
    );
    assert_eq!(capture.active, Some(Phase::PrepassBody));
    assert!(
      capture.budget.replace(configured).is_none(),
      "one freeze attempt"
    );
    (true, capture.entry_input.clone())
  });
  if let Some(input) = input {
    // Diagnostic receipt reads are a separate arm, never donated to the primary capture.
    let receipt = EntryReceipt {
      authorship_cached: polkadot_sdk::sp_io::storage::exists(&input.authorship_key),
      timestamp_pending: polkadot_sdk::sp_io::storage::exists(&input.timestamp_key),
      frame: System::block_weight().total(),
      provisional_prepass: input.declaration,
      retained_base_and_bytes: input.retained_overhead,
      declared_prefix: super::super::freeze_entry::normalized_provider_prefix(
        now,
        input.declaration,
        input.quarter,
        &input.staged_key,
        &input.stored_extrinsic,
      ),
    };
    CAPTURE.with_borrow_mut(|capture| {
      let capture = capture.as_mut().unwrap();
      assert_eq!(
        capture.prepass_frame,
        Some(receipt.frame),
        "observation-to-provider work must not be mistaken for a new settled charge"
      );
      assert!(capture.receipt.replace(receipt).is_none());
    });
  }
  if enabled {
    boundary("freeze_entry");
    region("actor_prepass_after_freeze");
  }
}

fn notice(phase: Phase, entering: bool) {
  if phase == Phase::PrepassBody
    && entering
    && CAPTURE.with_borrow(|capture| capture.as_ref().unwrap().entry_input.is_some())
  {
    let frame = System::block_weight().total();
    CAPTURE.with_borrow_mut(|capture| {
      assert!(
        capture
          .as_mut()
          .unwrap()
          .prepass_frame
          .replace(frame)
          .is_none()
      );
    });
  }
  let name = match (phase, entering) {
    (Phase::PrepassBody, true) => "prepass_body_enter",
    (Phase::PrepassBody, false) => "prepass_body_exit",
    (Phase::DrainBody, true) => "drain_body_enter",
    (Phase::DrainBody, false) => "drain_body_exit",
    _ => panic!("unexpected phase in the default-profile caller"),
  };
  CAPTURE.with_borrow_mut(|capture| {
    let capture = capture.as_mut().unwrap();
    if entering {
      assert!(capture.active.replace(phase).is_none());
    } else {
      assert_eq!(capture.active.take(), Some(phase));
    }
  });
  boundary(name);
  region(match (phase, entering) {
    (Phase::PrepassBody, true) => "prepass_preparation_to_freeze",
    (Phase::DrainBody, true) => "actor_drain",
    (_, false) => "remaining_fixed_and_common",
    _ => unreachable!("phase admission above is exhaustive"),
  });
}

struct ResultRow {
  top: BTreeMap<Vec<u8>, Vec<u8>>,
  proof: StorageProof,
  boundaries: Vec<Boundary>,
  before_commit: Db,
  after_commit: Db,
  calls: BTreeMap<String, u64>,
  accepted: bool,
  receipt: Option<EntryReceipt>,
  regions: Vec<&'static str>,
}

fn entry_input(block: &Block) -> EntryInput {
  use polkadot_sdk::frame_support::dispatch::DispatchClass;
  let mut prepasses = block.extrinsics.iter().enumerate().filter(|(_, xt)| {
    matches!(
      xt.function,
      RuntimeCall::Actors(pallet_deos_actors::Call::actor_prepass {})
    )
  });
  let (index, xt) = prepasses.next().unwrap();
  assert!(prepasses.next().is_none());
  assert_eq!(index, 2, "declared three-provider topology");
  let info = xt.get_dispatch_info();
  assert_eq!(info.class, DispatchClass::Mandatory);
  assert_eq!(info.extension_weight, Weight::zero());
  let maximum = BlockResourceBudgetValue::get().maximum_block();
  assert_eq!(
    info.total_weight(),
    maximum,
    "only this provisional call declaration is removed"
  );
  let encoded = xt.encode();
  let mut staged_key = System::storage_info()
    .into_iter()
    .find(|item| item.storage_name == b"ExtrinsicData")
    .unwrap()
    .prefix;
  let index = u32::try_from(index).unwrap().encode();
  staged_key.extend(polkadot_sdk::sp_io::hashing::twox_64(&index));
  staged_key.extend(index);
  let retained_overhead = crate::configs::RuntimeBlockWeights::get()
    .get(info.class)
    .base_extrinsic
    .checked_add(&Weight::from_parts(
      0,
      u64::try_from(encoded.len()).unwrap(),
    ))
    .unwrap();
  EntryInput {
    staged_key,
    stored_extrinsic: encoded.encode(),
    declaration: info.total_weight(),
    retained_overhead,
    encoded_bytes: encoded.len(),
    quarter: Weight::from_parts(maximum.ref_time() / 4, maximum.proof_size() / 4),
    authorship_key: crate::Authorship::storage_info()
      .into_iter()
      .find(|item| item.storage_name == b"Author")
      .unwrap()
      .prefix,
    timestamp_key: crate::Timestamp::storage_info()
      .into_iter()
      .find(|item| item.storage_name == b"DidUpdate")
      .unwrap()
      .prefix,
  }
}

fn execute(block: Block, original_root: H256) -> (bool, H256) {
  // The outer import transaction is distinct from FRAME's inner dispatch transactions.
  polkadot_sdk::sp_io::storage::start_transaction();
  boundary("executive_enter");
  let accepted = match catch_unwind(AssertUnwindSafe(|| Executive::execute_block(block.into()))) {
    Ok(()) => true,
    Err(error) => {
      let message = error
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| error.downcast_ref::<String>().map(String::as_str));
      assert_eq!(
        message,
        Some("Storage root must match that calculated."),
        "only the deliberate final-check refusal is an expected failure"
      );
      false
    }
  };
  boundary(if accepted {
    "executive_return"
  } else {
    "executive_refusal"
  });
  region("root_and_import_settlement");
  if accepted {
    polkadot_sdk::sp_io::storage::commit_transaction();
  } else {
    polkadot_sdk::sp_io::storage::rollback_transaction();
    boundary("import_rollback");
  }
  let root = H256::from_slice(&polkadot_sdk::sp_io::storage::root(StateVersion::V1));
  boundary("post_call_root");
  if !accepted {
    assert_eq!(
      root, original_root,
      "refused import retains no state mutation"
    );
  }
  region("witness_capture_and_sdk_commit");
  (accepted, root)
}

fn capture(
  parent: &Storage,
  expected_post: &Storage,
  block: Block,
  observe: bool,
  probe_entry: bool,
) -> ResultRow {
  assert!(
    !probe_entry || observe,
    "diagnostic receipt requires the observer arm"
  );
  assert!(
    !probe_entry || super::SAMPLES.with(|samples| samples.borrow().is_none()),
    "diagnostic receipt reads are excluded from raw clock collection"
  );
  let entry_input = probe_entry.then(|| entry_input(&block));
  benchmark_observation::assert_unobserved();
  assert!(parent.children_default.is_empty(), "TOP-only host domain");
  let original_root = BlakeTwo256::trie_root(
    parent
      .top
      .iter()
      .map(|(k, v)| (k.clone(), v.clone()))
      .collect(),
    StateVersion::V1,
  );
  let host = Host::new(parent.clone(), None, true, true).unwrap();
  let recorder = host.recorder().unwrap();
  assert!(recorder.to_storage_proof().is_empty());
  assert_eq!(host.read_write_count(), (0, 0, 0, 0));
  struct Reset;
  impl Drop for Reset {
    fn drop(&mut self) {
      CAPTURE.with_borrow_mut(|capture| *capture = None);
    }
  }
  assert!(CAPTURE.with_borrow(|capture| capture.is_none()));
  let _reset = Reset;
  if observe {
    CAPTURE.with_borrow_mut(|capture| {
      *capture = Some(Capture {
        recorder: recorder.clone(),
        boundaries: Vec::new(),
        active: None,
        budget: None,
        expected_block: *block.header.number(),
        entry_input,
        prepass_frame: None,
        receipt: None,
        regions: vec!["prefix_initialize_and_context"],
        inclusive_clock_span: super::span("complete_obligation_capture"),
        clock_span: super::span("prefix_initialize_and_context"),
      })
    });
  }
  let mut overlay = OverlayedChanges::default();
  let mut extensions = Extensions::default();
  extensions.register(ProofSizeExt::new(RecordingProofSizeProvider::new(
    recorder.clone(),
  )));
  let mut ext = Ext::new(&mut overlay, &host, Some(&mut extensions));
  let run = || {
    set_and_run_with_externalities(&mut ext, || {
      if observe {
        benchmark_observation::with_observer(notice, || execute(block, original_root))
      } else {
        execute(block, original_root)
      }
    })
  };
  let ((accepted, root), calls) = if observe {
    crate::tests::h1_storage_trace::recorded(|| crate::tests::h1_storage_trace::logical_calls(run))
  } else {
    (run(), BTreeMap::new())
  };
  // The original-root witness must survive the commit/reopen boundary independently of the host.
  let proof = recorder.to_storage_proof();
  let before_commit = host.read_write_count();
  ext.commit();
  let after_commit = host.read_write_count();
  // Stop the complete capture before any independent TOP scan or reference verification.
  let (boundaries, receipt, regions) = CAPTURE
    .with_borrow_mut(|capture| capture.take())
    .map_or_else(
      || (Vec::new(), None, Vec::new()),
      |mut capture| {
        drop(capture.clock_span.take());
        drop(capture.inclusive_clock_span.take());
        assert!(capture.active.is_none());
        assert_eq!(capture.budget, Some(BlockResourceBudgetValue::get()));
        assert_eq!(capture.receipt.is_some(), capture.entry_input.is_some());
        (capture.boundaries, capture.receipt, capture.regions)
      },
    );
  assert_eq!(overlay.transaction_depth(), 0);
  let top = host
    .pairs(Default::default())
    .unwrap()
    .map(Result::unwrap)
    .collect::<BTreeMap<_, _>>();
  let expected = if accepted {
    &expected_post.top
  } else {
    &parent.top
  };
  assert_eq!(&top, expected, "complete committed TOP identity");
  assert_eq!(
    root,
    BlakeTwo256::trie_root(
      top.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
      StateVersion::V1
    )
  );
  ResultRow {
    top,
    proof,
    boundaries,
    before_commit,
    after_commit,
    calls,
    accepted,
    receipt,
    regions,
  }
}

fn replay(authored: &AuthoredBlock, block: Block, proof: &StorageProof) -> bool {
  benchmark_observation::assert_unobserved();
  let original_root = BlakeTwo256::trie_root(
    authored
      .pre_state
      .top
      .iter()
      .map(|(k, v)| (k.clone(), v.clone()))
      .collect(),
    StateVersion::V1,
  );
  let compact = proof
    .clone()
    .into_compact_proof::<BlakeTwo256>(original_root)
    .unwrap();
  let (decoded, decoded_root) = compact
    .to_storage_proof::<BlakeTwo256>(&original_root)
    .unwrap();
  assert_eq!(decoded_root, original_root);
  let backend = create_proof_check_backend::<BlakeTwo256>(original_root, decoded).unwrap();
  let recorder = Recorder::<BlakeTwo256>::default();
  let backend = TrieBackendBuilder::wrap(&backend)
    .with_recorder(recorder.clone())
    .build();
  let mut extensions = Extensions::default();
  extensions.register(ProofSizeExt::new(RecordingProofSizeProvider::new(recorder)));
  let mut overlay = OverlayedChanges::default();
  let mut ext = Ext::new(&mut overlay, &backend, Some(&mut extensions));
  set_and_run_with_externalities(&mut ext, || {
    let (accepted, root) = execute(block, original_root);
    let expected = if accepted {
      authored.block.header.state_root()
    } else {
      &original_root
    };
    assert_eq!(
      &root, expected,
      "proof-only replay checks the independent full-state root"
    );
    accepted
  })
}

#[test]
fn complete_fixed_obligation_caller_preserves_success_and_late_refusal() {
  let storage = preset_genesis_storage(
    &[],
    &polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into(),
  );
  let parent = parent_header_for(storage.clone(), &[], 0);
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let authored = author_complete_block_after(
    storage,
    &[],
    &parent,
    1,
    UserDemand::ActorOnly,
    &signer,
    0,
    &BTreeMap::new(),
  );
  // Reuse the existing meter driver opt-in. Never enable before host/profile admission.
  let collect_clocks = match std::env::var("DEOS_RESOURCE_METER_MEASURE").as_deref() {
    Ok("1") => true,
    Err(std::env::VarError::NotPresent) => false,
    _ => panic!("DEOS_RESOURCE_METER_MEASURE must be absent or 1"),
  };
  for valid in [true, false] {
    let mut block = authored.block.clone();
    if !valid {
      block.header.set_state_root(H256::repeat_byte(0xa5));
    }
    let plain = capture(
      &authored.pre_state,
      &authored.post_state,
      block.clone(),
      false,
      false,
    );
    let (observed, raw_clock_spans) = super::observed(collect_clocks, || {
      capture(
        &authored.pre_state,
        &authored.post_state,
        block.clone(),
        true,
        false,
      )
    });
    assert_eq!(
      observed.regions,
      [
        "prefix_initialize_and_context",
        "prepass_preparation_to_freeze",
        "actor_prepass_after_freeze",
        "remaining_fixed_and_common",
        "actor_drain",
        "remaining_fixed_and_common",
        "root_and_import_settlement",
        "witness_capture_and_sdk_commit",
      ]
    );
    let clock_accounting = if collect_clocks {
      let (inclusive, regions) = raw_clock_spans.split_last().unwrap();
      assert_eq!(inclusive.phase, "complete_obligation_capture");
      assert_eq!(
        regions
          .iter()
          .map(|sample| sample.phase)
          .collect::<Vec<_>>(),
        observed.regions
      );
      let region_cpu = regions.iter().map(|sample| sample.cpu_ns).sum::<u128>();
      let region_elapsed = regions.iter().map(|sample| sample.elapsed_ns).sum::<u128>();
      // Boundary/readout gaps are retained by the inclusive owner, never silently discarded.
      Some(serde_json::json!({
        "regionCpuNs": region_cpu, "regionElapsedNs": region_elapsed,
        "inclusiveCpuNs": inclusive.cpu_ns, "inclusiveElapsedNs": inclusive.elapsed_ns,
        "includedBoundaryGlueCpuNs": inclusive.cpu_ns.checked_sub(region_cpu).unwrap(),
        "includedBoundaryGlueElapsedNs": inclusive.elapsed_ns.checked_sub(region_elapsed).unwrap(),
        "inclusiveAndRegionsMustNotBeAdded": true,
      }))
    } else {
      assert!(raw_clock_spans.is_empty());
      None
    };
    let probed = capture(
      &authored.pre_state,
      &authored.post_state,
      block.clone(),
      true,
      true,
    );
    assert_eq!(probed.accepted, valid);
    assert_eq!(
      probed.top, observed.top,
      "receipt observation cannot alter imported state"
    );
    assert_eq!(
      probed.proof, observed.proof,
      "receipt may not donate original witness inputs"
    );
    let input = entry_input(&block);
    let receipt = probed.receipt.unwrap();
    let raw_prefix = receipt
      .frame
      .checked_sub(&receipt.provisional_prepass)
      .unwrap();
    assert!(
      raw_prefix.all_gte(input.retained_overhead),
      "Prepass base and bytes remain in the prefix"
    );
    let expected = if raw_prefix.any_gt(input.quarter) {
      Err(super::super::freeze_entry::Refusal::Ceiling)
    } else {
      Ok(raw_prefix)
    };
    assert_eq!(receipt.declared_prefix, expected);
    assert_eq!(receipt.retained_base_and_bytes, input.retained_overhead);
    assert_eq!(
      receipt.provisional_prepass,
      BlockResourceBudgetValue::get().maximum_block()
    );
    assert!(plain.receipt.is_none() && observed.receipt.is_none());
    assert!(receipt.authorship_cached && receipt.timestamp_pending);
    assert!(!probed.top.contains_key(&input.authorship_key));
    assert!(!probed.top.contains_key(&input.timestamp_key));
    let timestamp_prepaid =
      <polkadot_sdk::pallet_timestamp::weights::SubstrateWeight<Runtime> as
        polkadot_sdk::pallet_timestamp::WeightInfo>::on_finalize();
    assert!(raw_prefix.all_gte(timestamp_prepaid));
    // These are rejected accounting candidates, not alternative runtime allocations.
    // Arithmetic alone cannot detect an omitted owner or a duplicated prepayment.
    let arithmetic_only = |prefix, tail| {
      BlockResourceBudget::from_settled_prefix(
        BlockResourceBudgetValue::get().maximum_block(),
        input.quarter,
        prefix,
        tail,
        1,
        3,
      )
      .unwrap()
      .fixed_envelope()
    };
    let omitted_tail = arithmetic_only(raw_prefix, Weight::zero());
    let duplicated_prepayment = arithmetic_only(raw_prefix, timestamp_prepaid);
    let credited_prepayment = arithmetic_only(
      raw_prefix.checked_sub(&timestamp_prepaid).unwrap(),
      Weight::zero(),
    );
    assert_eq!(omitted_tail, raw_prefix);
    assert_eq!(
      duplicated_prepayment,
      raw_prefix.checked_add(&timestamp_prepaid).unwrap()
    );
    assert_ne!(duplicated_prepayment, raw_prefix);
    assert_ne!(credited_prepayment, raw_prefix);
    let mq_context = crate::configs::MessageQueueServiceWeight::get()
      .checked_add(&crate::configs::MaximumContextMeasuredWeight::get())
      .unwrap();
    assert!(mq_context.ref_time() > input.quarter.ref_time());
    println!(
      "FIXED_OWNER_COMPOSITION_REFUSAL_V1 {}",
      serde_json::json!({
        "acceptedImport": valid,
        "chargedPrefix": [raw_prefix.ref_time(), raw_prefix.proof_size()],
        "timestampPrepaid": [timestamp_prepaid.ref_time(), timestamp_prepaid.proof_size()],
        "authorshipPendingAtFreeze": receipt.authorship_cached,
        "timestampPendingAtFreeze": receipt.timestamp_pending,
        "bothCachesAbsentAfterImportOrRollback": true,
        "arithmeticAcceptsOmittedTailAndDuplicatePrepayment": true,
        "prepaidCreditChangesAuthenticatedPrefix": true,
        "messageServicePlusContextDeclaration": [mq_context.ref_time(), mq_context.proof_size()],
        "quarter": [input.quarter.ref_time(), input.quarter.proof_size()],
        "jointMaximumReachabilityEstablished": false,
        "completeAuthorshipAndHostReserve": null,
        "compositionQualified": false,
        "physicalCostQualified": false,
        "allocationInstalled": false,
      })
    );
    println!(
      "FIXED_PREFIX_ENTRY_RECEIPT_V1 {}",
      serde_json::json!({
        "accepted": valid,
        "declaredPrefixGuard": format!("{:?}", receipt.declared_prefix),
        "declaredPrefix": [raw_prefix.ref_time(), raw_prefix.proof_size()],
        "removedProvisionalPrepass": [receipt.provisional_prepass.ref_time(), receipt.provisional_prepass.proof_size()],
        "retainedPrepassBaseAndBytes": [input.retained_overhead.ref_time(), input.retained_overhead.proof_size()],
        "retainedEncodedBytes": input.encoded_bytes,
        "diagnosticLogicalCalls": probed.calls,
        "diagnosticSdkBeforeCommit": probed.before_commit,
        "configuredBudgetUnchanged": true, "completeRemainingFixedReserve": null,
        "physicalPrefixQualified": false, "allocationInstalled": false,
      })
    );
    assert_eq!(plain.accepted, valid);
    assert_eq!(observed.accepted, valid);
    assert_eq!(plain.top, observed.top);
    assert_eq!(
      plain.proof, observed.proof,
      "passive boundaries must not donate witness inputs"
    );
    assert_eq!(plain.before_commit, observed.before_commit);
    assert_eq!(plain.after_commit, observed.after_commit);
    // A missing backend input must not masquerade as the expected rejected block.
    assert!(replay(&authored, authored.block.clone(), &observed.proof));
    assert_eq!(replay(&authored, block, &observed.proof), valid);
    let original_root = BlakeTwo256::trie_root(
      authored
        .pre_state
        .top
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect(),
      StateVersion::V1,
    );
    assert!(
      create_proof_check_backend::<BlakeTwo256>(original_root, StorageProof::empty()).is_err()
    );
    assert!(
      create_proof_check_backend::<BlakeTwo256>(H256::repeat_byte(0xa5), observed.proof.clone())
        .is_err()
    );
    let names = observed
      .boundaries
      .iter()
      .map(|b| b.name)
      .collect::<Vec<_>>();
    assert_eq!(
      &names[..6],
      &[
        "executive_enter",
        "prepass_body_enter",
        "freeze_entry",
        "prepass_body_exit",
        "drain_body_enter",
        "drain_body_exit"
      ]
    );
    assert_eq!(
      names[6],
      if valid {
        "executive_return"
      } else {
        "executive_refusal"
      }
    );
    assert_eq!(names.last(), Some(&"post_call_root"));
    assert_eq!(names.contains(&"import_rollback"), !valid);
    let mut seen = BTreeSet::new();
    let mut increments = 0;
    for boundary in &observed.boundaries {
      for node in boundary.proof.iter_nodes() {
        if seen.insert(node.clone()) {
          increments += node.len();
        }
      }
      assert_eq!(
        boundary.db.2, 0,
        "SDK writes are commit counts, not provisional work"
      );
    }
    assert_eq!(
      seen,
      observed.proof.clone().into_nodes(),
      "one original-root union, never summed independent proof deltas"
    );
    assert_eq!(
      increments,
      observed.proof.iter_nodes().map(Vec::len).sum::<usize>()
    );
    assert!(
      observed.calls.get("Put").copied().unwrap_or(0) > 0,
      "incurred writes survive late import rollback in the operation journal"
    );
    assert!(observed.calls.get("StorageRoot").copied().unwrap_or(0) > 0);
    if !valid {
      assert_eq!(observed.after_commit.2, 0);
    }
    println!(
      "FIXED_OBLIGATION_CALLER_V1 {}",
      serde_json::json!({
        "accepted": valid, "boundaries": names, "logicalCalls": observed.calls,
        "sdkDbBeforeCommit": observed.before_commit, "sdkDbAfterCommit": observed.after_commit,
        "originalWitnessBytes": increments, "nativeOnly": true, "timingsCollected": collect_clocks,
        "disjointCallerRegions": observed.regions,
        "rawClockSpans": raw_clock_spans.iter().map(|sample|
          (sample.phase, sample.cpu_ns, sample.elapsed_ns)).collect::<Vec<_>>(),
        "rawClockSpansIncludeObserverGlue": true, "rawClocksAreNotCalibration": true,
        "clockAccounting": clock_accounting,
        "completeCallerNotFixedOnlyPrice": true, "outstandingFixedReserveQualified": false,
        "productionTransferQualified": false, "quarterCredit": false,
      })
    );
  }
}

fn retained_target_source() -> (Storage, Storage, Block, InherentData) {
  use codec::DecodeAll;
  use polkadot_sdk::{
    frame_support::traits::ExecuteBlock,
    sp_consensus_aura::{digests::CompatibleDigestItem, sr25519::AuthoritySignature},
  };
  assert!(
    std::env::var_os("DEOS_RESOURCE_METER_MEASURE").is_none(),
    "this boundary counterexample collects no timing samples"
  );
  let input = |variable: &str, expected: &str| {
    let bytes =
      std::fs::read(std::env::var(variable).expect("explicit retained source input")).unwrap();
    assert_eq!(
      format!(
        "{:x}",
        H256::from(polkadot_sdk::sp_io::hashing::sha2_256(&bytes))
      ),
      expected,
      "retained inputs must not be replaced or repaired"
    );
    bytes
  };
  let parent = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_TERMINAL_SOURCE_PARENT",
        "7397be5ba157a264ef34e891268cfffe4ab54500656fdfa2da2cca007ac72f7c",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let post = Storage {
    top: BTreeMap::<Vec<u8>, Vec<u8>>::decode_all(
      &mut input(
        "DEOS_TERMINAL_SOURCE_POST",
        "216baf777b9edaf3e9f10a1864daa8912815573d4dd5a183443d70cee907974e",
      )
      .as_slice(),
    )
    .unwrap(),
    ..Default::default()
  };
  let sealed = Block::decode_all(
    &mut input(
      "DEOS_TERMINAL_SOURCE_BLOCK",
      "aaa7d947521758407b93ebddab032bdba8bc97f6e1668b824cb0d792bee0e606",
    )
    .as_slice(),
  )
  .unwrap();
  assert_eq!(*sealed.header.number(), 187_465);
  let wasm = &parent.top[well_known_keys::CODE];
  assert_eq!(
    polkadot_sdk::sp_io::hashing::sha2_256(wasm),
    ACCEPTED_PRODUCTION_WASM_SHA256
  );
  // SDK authentication removes the real seal; no state/header repair or new block is authored.
  type Executor = polkadot_sdk::cumulus_pallet_aura_ext::BlockExecutor<Runtime, Executive>;
  let mut verified = sealed.clone().into();
  externalities_with_bound_code(parent.clone()).execute_with(|| {
    <Executor as ExecuteBlock<Block>>::verify_and_remove_seal(&mut verified);
  });
  let mut block = sealed;
  let seal = block.header.digest_mut().pop().unwrap();
  assert!(<DigestItem as CompatibleDigestItem<AuthoritySignature>>::as_aura_seal(&seal).is_some());
  assert_eq!(
    format!("{:x}", block.header.hash()),
    "1135c9bb8929e85fb24ebc9cad8a403174604dc5554d243a5947ddd4db3c726e"
  );
  // Recover context from the literal, already-authenticated inherent, not a synthetic new parent.
  let mut inherent_data = InherentData::new();
  for xt in &block.extrinsics {
    match &xt.function {
      RuntimeCall::Timestamp(polkadot_sdk::pallet_timestamp::Call::set { now }) => {
        inherent_data
          .put_data(polkadot_sdk::sp_timestamp::INHERENT_IDENTIFIER, now)
          .unwrap();
      }
      RuntimeCall::ParachainSystem(
        polkadot_sdk::cumulus_pallet_parachain_system::Call::set_validation_data {
          data,
          inbound_messages_data,
        },
      ) => {
        let (downward, downward_hashed) = inbound_messages_data.downward_messages.messages();
        let (horizontal, horizontal_hashed) = inbound_messages_data.horizontal_messages.messages();
        assert!(
          downward.is_empty()
            && downward_hashed.is_empty()
            && horizontal.is_empty()
            && horizontal_hashed.is_empty(),
          "retained empty-message source only"
        );
        let literal = ParachainInherentData {
          validation_data: data.validation_data.clone(),
          relay_chain_state: data.relay_chain_state.clone(),
          relay_parent_descendants: data.relay_parent_descendants.clone(),
          collator_peer_id: data.collator_peer_id.clone(),
          downward_messages: Default::default(),
          horizontal_messages: Default::default(),
        };
        inherent_data
          .put_data(INHERENT_IDENTIFIER, &literal)
          .unwrap();
      }
      _ => {}
    }
  }
  pallet_deos_actors::provide_actor_prepass_inherent_data(&mut inherent_data).unwrap();
  (parent, post, block, inherent_data)
}

#[test]
#[ignore = "exact retained terminal packet; no history regeneration or timing collection"]
fn retained_target_import_does_not_emit_native_fixed_owner_boundaries() {
  let (parent, post, block, inherent_data) = retained_target_source();
  let wasm = &parent.top[well_known_keys::CODE];
  let native = capture(&parent, &post, block.clone(), true, false);
  assert!(native.accepted);
  assert_eq!(
    native
      .boundaries
      .iter()
      .filter(|b| b.name == "freeze_entry")
      .count(),
    1
  );
  assert!(
    native
      .boundaries
      .iter()
      .any(|b| b.name == "prepass_body_exit")
  );
  assert!(
    native
      .boundaries
      .iter()
      .any(|b| b.name == "drain_body_exit")
  );
  std::thread_local! { static TARGET_NOTICES: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
  let notice = |_: Phase, _: bool| TARGET_NOTICES.set(TARGET_NOTICES.get().checked_add(1).unwrap());
  let target = benchmark_observation::with_observer(notice, || {
    crate::tests::wasm_replay::replay_block(parent.clone(), wasm, &block.encode(), &inherent_data)
  })
  .expect("selected actual Wasm import and independent compact proof-only replay must succeed");
  assert_eq!(target.post_state_root, *block.header.state_root());
  assert!(!target.execution_proof.storage_proof.is_empty());
  assert!(
    !target.proof_size_observations.is_empty(),
    "real guest proof-size imports were executed"
  );
  assert_eq!(
    TARGET_NOTICES.get(),
    0,
    "native owner hooks do not cross the actual guest ABI"
  );
  let mut refused = block.clone();
  refused.header.set_state_root(H256::repeat_byte(0xa5));
  let native_refusal = capture(&parent, &post, refused.clone(), true, false);
  assert!(!native_refusal.accepted);
  assert!(native_refusal.calls.get("Put").copied().unwrap_or(0) > 0);
  let target_error = benchmark_observation::with_observer(notice, || {
    crate::tests::wasm_replay::replay_block(parent.clone(), wasm, &refused.encode(), &inherent_data)
  })
  .expect_err("the same target must reject the changed final root");
  assert!(
    target_error.starts_with("Full-state Core_execute_block:"),
    "{target_error}"
  );
  assert_eq!(
    TARGET_NOTICES.get(),
    0,
    "changed-root target rejection also supplies no native owner milestones"
  );
  println!(
    "FIXED_OWNER_TARGET_BOUNDARY_COUNTEREXAMPLE_V1 {}",
    serde_json::json!({
      "profile": "existing WasmExecutor/Core_execute_block/Onchain-import-with-proof-recording",
      "codeSha256": format!("{:x}", H256::from(ACCEPTED_PRODUCTION_WASM_SHA256)),
      "block": block.header.number(), "unsealedBlockHash": format!("{:x}", block.header.hash()),
      "nativeOwnerBoundaries": native.boundaries.iter().map(|b| b.name).collect::<Vec<_>>(),
      "targetOwnerNotices": TARGET_NOTICES.get(), "targetProofSizeImports": target.proof_size_observations.len(),
      "targetPostRoot": format!("{:x}", target.post_state_root), "targetChangedRootRejection": target_error,
      "targetFailureLocationVerified": false,
      "semanticTransferForSelectedPacket": true, "physicalTransferQualified": false,
      "nativeObserverVisibleInTarget": TARGET_NOTICES.get() != 0,
      "otherTargetObservationInterfacesRuledOut": false, "completeRemainingFixedReserve": null,
      "pvfProfileQualified": false, "timingsCollected": false,
    })
  );
}

#[test]
#[ignore = "retained CODE/packet and explicit new guest-import output directory; no timing"]
fn retained_target_guest_import_sites_preserve_bound_prefix_and_results() {
  use crate::tests::wasm_replay::{GuestImportTrace, replay_block_with_guest_imports};
  let (parent, _post, block, inherent_data) = retained_target_source();
  let output = std::path::PathBuf::from(
    std::env::var_os("DEOS_GUEST_IMPORT_OUTPUT").expect("explicit new output directory"),
  );
  std::fs::create_dir(&output).expect("do not overwrite retained trace evidence");
  let wasm = &parent.top[well_known_keys::CODE];
  let encoded = block.encode();
  let json_trace = |trace: &GuestImportTrace| {
    serde_json::json!({
      "codeSha256": trace.code_sha256.map(|h| format!("{h:x}")),
      "originalRoot": trace.original_root.map(|h| format!("{h:x}")),
      "callDataHash": trace.call_data_hash.map(|h| format!("{h:x}")),
      "compiledModuleSha256": trace.compiled_module_sha256.map(|h| format!("{h:x}")),
      "sites": trace.sites.iter().map(|site| serde_json::json!({
        "import": site.import, "ordinal": site.ordinal,
        "frames": site.frames.iter().map(|frame| serde_json::json!({
          "function": frame.function, "moduleOffset": frame.module_offset,
          "functionOffset": frame.function_offset, "name": frame.name,
        })).collect::<Vec<_>>(),
      })).collect::<Vec<_>>(),
      "seen": trace.seen, "incomplete": trace.incomplete,
      "completeImportPrefixOnly": trace.has_complete_import_prefix(),
      "injectedBeforeOriginalImport": trace.injected_at,
      "execution": trace.execution,
      "backendRootAfterExecution": trace.backend_root_after_execution.map(|h| format!("{h:x}")),
      "proofObservations": trace.proof_observations,
    })
  };
  let mut report = serde_json::json!({
    "scope": "actual guest call-sites at existing storage-set/proof-size imports, not complete owner boundaries",
    "offsetCoordinate": "SDK-prepared module; no raw CODE offset/owner map asserted",
    "checksComplete": false,
    "unobservedWork": ["guest work between import sites", "owners with no observed import", "host setup/verification/root/commit"],
    "completeFixedOwnerAttribution": false, "remainingFixedReserve": null,
    "observerOverheadQualified": false, "physicalTransferQualified": false,
    "timingsCollected": false, "productionCodeChanged": false,
  });
  let save = |report: &serde_json::Value| {
    std::fs::write(
      output.join("guest-imports.json"),
      serde_json::to_vec_pretty(report).unwrap(),
    )
    .unwrap();
  };
  let (observed, trace) =
    replay_block_with_guest_imports(parent.clone(), wasm, &encoded, &inherent_data, 16384, None);
  report["normal"] = json_trace(&trace);
  save(&report);
  let observed = observed.expect("observed import and ordinary compact proof-only replay succeed");
  assert!(
    trace.has_complete_import_prefix(),
    "seen={} retained={} incomplete={}",
    trace.seen,
    trace.sites.len(),
    trace.incomplete
  );
  assert_eq!(observed.post_state_root, *block.header.state_root());
  assert_eq!(
    trace.code_sha256,
    Some(H256::from(ACCEPTED_PRODUCTION_WASM_SHA256))
  );
  assert_eq!(trace.original_root, Some(observed.pre_state_root));
  assert_eq!(
    trace.call_data_hash,
    Some(H256::from(polkadot_sdk::sp_io::hashing::blake2_256(
      &encoded
    )))
  );
  assert_eq!(trace.backend_root_after_execution, trace.original_root);
  assert!(trace.compiled_module_sha256.is_some());
  assert_eq!(trace.execution, Some(Ok(())));
  let is_proof = |site: &&crate::tests::wasm_replay::GuestImportSite| {
    site.import == "ext_storage_proof_size_storage_proof_size_version_1"
  };
  assert_eq!(
    trace.sites.iter().filter(is_proof).count(),
    observed.proof_size_observations.len()
  );
  assert_eq!(
    trace.proof_observations,
    observed.proof_size_observations.len()
  );
  assert!(
    trace
      .sites
      .iter()
      .any(|site| site.import == "ext_storage_set_version_1")
  );
  assert!(trace.has_complete_import_prefix(), "{trace:?}");
  for (index, site) in trace.sites.iter().enumerate() {
    assert_eq!(site.ordinal as usize, index + 1);
    assert!(
      site.frames.iter().any(|frame| frame
        .name
        .as_ref()
        .is_some_and(|name| name.contains("Core_execute_block"))),
      "{site:?}"
    );
  }
  let interrupt_at = 17;
  let (interrupted, prefix) = replay_block_with_guest_imports(
    parent.clone(),
    wasm,
    &encoded,
    &inherent_data,
    16384,
    Some(interrupt_at),
  );
  report["interrupted"] = json_trace(&prefix);
  save(&report);
  assert!(
    interrupted
      .unwrap_err()
      .contains("DEV_GUEST_IMPORT_INTERRUPT")
  );
  assert_eq!(prefix.code_sha256, trace.code_sha256);
  assert_eq!(prefix.original_root, trace.original_root);
  assert_eq!(prefix.call_data_hash, trace.call_data_hash);
  assert_eq!(prefix.backend_root_after_execution, trace.original_root);
  assert_eq!(prefix.injected_at, Some(interrupt_at));
  assert!(prefix.compiled_module_sha256.is_some());
  assert_eq!(
    prefix.proof_observations,
    prefix.sites[..interrupt_at as usize - 1]
      .iter()
      .filter(is_proof)
      .count(),
    "interruption precedes the original import"
  );
  assert!(
    prefix
      .execution
      .as_ref()
      .unwrap()
      .as_ref()
      .unwrap_err()
      .contains("DEV_GUEST_IMPORT_INTERRUPT")
  );
  assert!(prefix.has_complete_import_prefix());
  assert_eq!(prefix.sites, trace.sites[..interrupt_at as usize]);
  let (capped, missing) =
    replay_block_with_guest_imports(parent.clone(), wasm, &encoded, &inherent_data, 1, None);
  report["capped"] = json_trace(&missing);
  save(&report);
  let capped = capped.expect("observer truncation changes evidence, not guest execution");
  assert_eq!(capped.post_state_root, observed.post_state_root);
  assert_eq!(
    capped.proof_size_observations,
    observed.proof_size_observations
  );
  assert_eq!(
    capped.execution_recorded_keys,
    observed.execution_recorded_keys
  );
  assert_eq!(
    capped.execution_proof.storage_proof,
    observed.execution_proof.storage_proof
  );
  assert_eq!(missing.sites, trace.sites[..1]);
  assert!(missing.incomplete);
  assert!(!missing.has_complete_import_prefix());
  let (unbound, rejected) =
    replay_block_with_guest_imports(parent.clone(), &[], &encoded, &inherent_data, 1, None);
  report["codeBindingRefusal"] = json_trace(&rejected);
  save(&report);
  assert_eq!(
    unbound.unwrap_err(),
    "Supplied Wasm does not equal pre-state :code"
  );
  assert!(
    rejected.sites.is_empty()
      && rejected.code_sha256.is_none()
      && !rejected.has_complete_import_prefix()
  );
  report["checksComplete"] = serde_json::json!(true);
  save(&report);
  let names: BTreeSet<_> = trace
    .sites
    .iter()
    .flat_map(|site| site.frames.iter())
    .filter_map(|frame| frame.name.clone())
    .collect();
  println!(
    "FIXED_GUEST_IMPORT_SITES_V1 {}",
    serde_json::json!({
      "codeSha256": report["normal"]["codeSha256"], "originalRoot": report["normal"]["originalRoot"],
      "sites": trace.seen, "interruptPrefix": prefix.seen, "frameNames": names,
      "postRoot": format!("{:x}", observed.post_state_root), "report": output.join("guest-imports.json"),
      "completeFixedOwnerAttribution": false, "remainingFixedReserve": null,
    })
  );
}
