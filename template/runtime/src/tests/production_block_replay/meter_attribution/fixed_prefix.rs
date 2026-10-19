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
}

#[derive(Debug, PartialEq, Eq)]
struct EntryReceipt {
  frame: Weight,
  provisional_prepass: Weight,
  retained_base_and_bytes: Weight,
  declared_prefix: Result<Weight, super::super::freeze_entry::Refusal>,
}

struct Capture {
  recorder: Recorder<BlakeTwo256>,
  boundaries: Vec<Boundary>,
  active: Option<Phase>,
  budget: Option<BlockResourceBudget>,
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
      now, 1,
      "this constructor covers only the declared first block"
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

fn capture(authored: &AuthoredBlock, block: Block, observe: bool, probe_entry: bool) -> ResultRow {
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
  assert!(
    authored.pre_state.children_default.is_empty(),
    "TOP-only host domain"
  );
  let original_root = BlakeTwo256::trie_root(
    authored
      .pre_state
      .top
      .iter()
      .map(|(k, v)| (k.clone(), v.clone()))
      .collect(),
    StateVersion::V1,
  );
  let host = Host::new(authored.pre_state.clone(), None, true, true).unwrap();
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
    &authored.post_state.top
  } else {
    &authored.pre_state.top
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
    let plain = capture(&authored, block.clone(), false, false);
    let (observed, raw_clock_spans) = super::observed(collect_clocks, || {
      capture(&authored, block.clone(), true, false)
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
    let probed = capture(&authored, block.clone(), true, true);
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
