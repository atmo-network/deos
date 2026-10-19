//! Test-only thread-CPU attribution in the unchanged canonical full-block caller.

use super::*;
use cpu_time::ThreadTime;
use std::{cell::RefCell, time::Instant};

#[derive(Debug)]
struct Sample {
  phase: &'static str,
  cpu_ns: u128,
  elapsed_ns: u128,
}

thread_local! {
  static SAMPLES: RefCell<Option<Vec<Sample>>> = const { RefCell::new(None) };
}

pub(crate) struct Span {
  phase: &'static str,
  cpu: ThreadTime,
  elapsed: Instant,
}

pub(crate) fn span(phase: &'static str) -> Option<Span> {
  SAMPLES
    .with(|samples| samples.borrow().is_some())
    .then(|| Span {
      phase,
      cpu: ThreadTime::try_now().expect("thread CPU clock is required"),
      elapsed: Instant::now(),
    })
}

impl Drop for Span {
  fn drop(&mut self) {
    let cpu_ns = self
      .cpu
      .try_elapsed()
      .expect("thread CPU clock remains available")
      .as_nanos();
    let elapsed_ns = self.elapsed.elapsed().as_nanos();
    SAMPLES.with(|samples| {
      let mut samples = samples.borrow_mut();
      let samples = samples.as_mut().unwrap();
      assert!(samples.len() < 1024, "bounded diagnostic callback domain");
      samples.push(Sample {
        phase: self.phase,
        cpu_ns,
        elapsed_ns,
      });
    });
  }
}

fn observed<R>(enabled: bool, run: impl FnOnce() -> R) -> (R, Vec<Sample>) {
  assert!(SAMPLES.with(|samples| samples.borrow().is_none()));
  struct Reset;
  impl Drop for Reset {
    fn drop(&mut self) {
      SAMPLES.with(|samples| *samples.borrow_mut() = None);
    }
  }
  let _reset = Reset;
  if enabled {
    SAMPLES.with(|samples| *samples.borrow_mut() = Some(Vec::with_capacity(1024)));
  }
  let value = run();
  let samples = SAMPLES.with(|samples| samples.borrow_mut().take().unwrap_or_default());
  (value, samples)
}

fn imported(authored: &AuthoredBlock, enabled: bool) -> (serde_json::Value, BTreeSet<Vec<u8>>) {
  let block = authored.block.clone();
  let encoded = block.encode();
  assert_eq!(
    encoded,
    authored.block.encode(),
    "identical signed input bytes"
  );
  let mut ext = TestExternalities::new_with_code_and_state(
    &[],
    authored.pre_state.clone(),
    crate::VERSION.state_version(),
  );
  let recorder = Recorder::<polkadot_sdk::sp_core::Blake2Hasher>::default();
  ext.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
    recorder.clone(),
  )));
  let ((cpu_ns, elapsed_ns), samples) = ext.execute_with_recorder(recorder.clone(), || {
    crate::apis::validate_context_inherent_geometry(&authored.inherent_data).unwrap();
    assert!(
      authored
        .inherent_data
        .check_extrinsics(&authored.block.clone().into())
        .ok()
    );
    observed(enabled, || {
      let cpu = ThreadTime::try_now().expect("thread CPU clock is required");
      let elapsed = Instant::now();
      Executive::execute_block(block.into());
      (
        cpu.try_elapsed().unwrap().as_nanos(),
        elapsed.elapsed().as_nanos(),
      )
    })
  });
  let proof = recorder.estimate_encoded_size();
  // Copy proof contents after both timed intervals and before any diagnostic storage scan.
  let proof_nodes = recorder.to_storage_proof().into_nodes();
  assert_no_auxiliary_block_overlay(&ext);
  let actual = ext.execute_with(current_top_storage);
  assert_eq!(actual.top, authored.post_state.top, "complete TOP identity");
  assert_eq!(
    actual.children_default,
    authored.post_state.children_default
  );
  let mut phases = BTreeMap::<&str, (usize, u128, u128)>::new();
  for sample in &samples {
    let entry = phases.entry(sample.phase).or_default();
    entry.0 += 1;
    entry.1 += sample.cpu_ns;
    entry.2 += sample.elapsed_ns;
  }
  if enabled {
    for name in ["prepare", "post"] {
      assert_eq!(
        phases.get(name).unwrap().0,
        16,
        "one callback per signed call"
      );
    }
  } else {
    assert!(samples.is_empty());
  }
  let callback_cpu = samples.iter().map(|sample| sample.cpu_ns).sum::<u128>();
  (
    serde_json::json!({
      "enabled": enabled, "executiveCpuNs": cpu_ns, "executiveElapsedNs": elapsed_ns,
      "callbackCpuNs": callback_cpu, "phases": phases, "recordedProof": proof,
      "rawCallbacks": samples.iter().map(|sample| (sample.phase, sample.cpu_ns, sample.elapsed_ns)).collect::<Vec<_>>(),
      "observedShareNotUninstrumentedCostBound": true,
      "notProductionWasmOrGlobalCpuUpper": true,
    }),
    proof_nodes,
  )
}

#[test]
fn native_resource_meter_full_caller_preserves_execution() {
  let storage = preset_genesis_storage(
    &[],
    &polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into(),
  );
  let parent = parent_header_for(storage.clone(), &[], 0);
  let signer = sr25519::Pair::from_string("//Alice", None).unwrap();
  let calls = (0..16)
    .map(|_| RuntimeCall::System(polkadot_sdk::frame_system::Call::remark { remark: Vec::new() }))
    .collect::<Vec<_>>();
  let authored = author_complete_block_after_with_calls(
    storage,
    &[],
    &parent,
    1,
    UserDemand::ActorOnly,
    &signer,
    0,
    &BTreeMap::new(),
    &calls,
  );
  assert_complete_block_replays_natively(&authored, &[]);
  let measurement = match std::env::var("DEOS_RESOURCE_METER_MEASURE").as_deref() {
    Ok("1") => true,
    Err(std::env::VarError::NotPresent) => false,
    _ => panic!("DEOS_RESOURCE_METER_MEASURE must be absent or 1"),
  };
  let (warmup, warmup_nodes) = imported(&authored, true);
  let mut triplets = Vec::new();
  for _ in 0..if measurement { 3 } else { 1 } {
    let (before, before_nodes) = imported(&authored, false);
    let (measured, measured_nodes) = imported(&authored, true);
    let (after, after_nodes) = imported(&authored, false);
    assert_eq!(before["recordedProof"], measured["recordedProof"]);
    assert_eq!(before["recordedProof"], after["recordedProof"]);
    assert_eq!(warmup_nodes, before_nodes, "exact recorded proof nodes");
    assert_eq!(
      before_nodes, measured_nodes,
      "observer preserves proof contents"
    );
    assert_eq!(before_nodes, after_nodes);
    triplets.push(serde_json::json!({"before": before, "observed": measured, "after": after}));
  }
  if !measurement {
    return;
  }
  let (_, calibration) = observed(true, || {
    for _ in 0..256 {
      let _span = span("empty");
      core::hint::black_box(());
    }
  });
  println!(
    "RESOURCE_METER_FULL_CALLER_V1 {}",
    serde_json::json!({
      "nativeOnly": true, "signedRemarks": 16, "excludedWarmup": warmup,
      "triplets": triplets,
      "emptyProbeCalibration": calibration.iter().map(|sample| (sample.cpu_ns, sample.elapsed_ns)).collect::<Vec<_>>(),
      "clockOverheadNotSubtracted": true, "canonicalWireMetadataTopRootProofPreserved": true,
      "source": "canonical SDK Executive and TxExtension; cfg(test)-only callback spans",
      "notActorPerStepAttributionOrProductionPrice": true,
    })
  );
}
