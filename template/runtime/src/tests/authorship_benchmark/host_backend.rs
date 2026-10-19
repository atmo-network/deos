//! Native SDK host semantics; the real runner's timing fields are deliberately unused.

use super::{authorship, common};
use crate::{AllPalletsWithSystem, Runtime, System};
use codec::Encode;
use polkadot_sdk::{
  frame_benchmarking::{BenchmarkParameter, Benchmarking, benchmarking},
  frame_support::traits::WhitelistedStorageKeys,
  sp_core::storage::{Storage, TrackedStorageKey, well_known_keys},
  sp_externalities::set_and_run_with_externalities,
  sp_runtime::traits::BlakeTwo256,
  sp_state_machine::{Backend, Ext, OverlayedChanges},
};
use sc_client_db::BenchmarkingState;

type Host = BenchmarkingState<BlakeTwo256>;

fn genesis() -> Storage {
  let mut source = common::seeded_test_ext();
  source.execute_with(|| System::set_block_number(0));
  source.commit_all().unwrap();
  let top = source
    .backend
    .pairs(Default::default())
    .unwrap()
    .map(Result::unwrap)
    .collect::<std::collections::BTreeMap<_, _>>();
  assert!(
    !top
      .keys()
      .any(|key| well_known_keys::is_child_storage_key(key))
  );
  Storage {
    top,
    ..Default::default()
  }
}

fn with_host<R>(genesis: Storage, run: impl FnOnce(&Host) -> R) -> R {
  let host = Host::new(genesis, None, true, true).unwrap();
  let mut overlay = OverlayedChanges::default();
  let mut ext = Ext::new(&mut overlay, &host, None);
  set_and_run_with_externalities(&mut ext, || run(&host))
}

#[test]
fn sdk_runner_uses_real_host_for_minimal_control_and_generated_input() {
  let storage = genesis();
  for (method, initialized) in [
    (b"authorship_complete".as_slice(), false),
    (b"authorship_with_initialize".as_slice(), true),
  ] {
    // Match the CLI's separate verification and DB passes; do not use timing fields.
    for verify in [true, false] {
      with_host(storage.clone(), |host| {
        let rows = <authorship::Pallet<Runtime> as Benchmarking>::run_benchmark(
          method,
          &[(BenchmarkParameter::c, 0), (BenchmarkParameter::d, 0)],
          &AllPalletsWithSystem::whitelisted_storage_keys(),
          verify,
          1,
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        let mut exclusions = authorship::owner_keys().to_vec();
        if initialized {
          exclusions.extend(authorship::initialization_owner_keys());
        }
        for key in &exclusions {
          assert!(!host.get_whitelist().iter().any(|entry| &entry.key == key));
          for (_, _, _, whitelisted) in row.keys.iter().filter(|entry| &entry.0 == key) {
            assert!(
              !whitelisted,
              "removed owner must not retain a tracker discount"
            );
          }
        }
        if !verify {
          let digest = &authorship::owner_keys()[2];
          let (_, reads, writes, whitelisted) =
            row.keys.iter().find(|entry| &entry.0 == digest).unwrap();
          assert!(!whitelisted);
          assert_eq!(*reads, u32::from(!initialized));
          assert_eq!(*writes, u32::from(initialized));
          println!(
            "AUTHORSHIP_HOST_ORIGIN {}",
            serde_json::json!({
              "initialize": initialized, "digestBackendReads": reads,
              "digestNetWrites": writes, "timingFieldsDiscarded": true
            })
          );
        }
        // The runner's deferred wipe restores genesis, not the authored successor.
        assert_eq!(System::block_number(), 0);
        assert_eq!(
          polkadot_sdk::sp_io::storage::get(&authorship::owner_keys()[2]).map(|v| v.to_vec()),
          storage.top.get(&authorship::owner_keys()[2]).cloned()
        );
      });
    }
  }
}

#[test]
fn host_whitelist_recorder_reset_and_pending_root_have_distinct_owners() {
  use authorship::Config;
  with_host(genesis(), |host| {
    let number = authorship::owner_keys()[3].clone();
    let recorder = host.recorder().unwrap();
    assert!(polkadot_sdk::sp_io::storage::get(&number).is_some());
    assert!(!recorder.to_storage_proof().is_empty());
    System::set_block_number(1);
    benchmarking::commit_db();
    assert!(
      recorder.to_storage_proof().is_empty(),
      "reopen resets proof"
    );
    // Empty compact proof against an unchanged nonempty root takes the SDK's zero fallback.
    assert_eq!(host.proof_size(), Some(0));

    host.set_whitelist(vec![TrackedStorageKey::new(number.clone())]);
    benchmarking::reset_read_write_count();
    assert!(polkadot_sdk::sp_io::storage::get(&number).is_some());
    assert_eq!(host.read_write_count(), (0, 0, 0, 0));
    let prefetched = recorder.to_storage_proof();
    assert!(!prefetched.is_empty());
    let prefetched_size = host.proof_size();
    benchmarking::reset_read_write_count();
    assert_eq!(recorder.to_storage_proof(), prefetched);
    assert_eq!(host.proof_size(), prefetched_size);

    host.set_whitelist(Vec::new());
    benchmarking::reset_read_write_count();
    assert!(polkadot_sdk::sp_io::storage::get(&number).is_some());
    assert_eq!(host.read_write_count(), (1, 0, 0, 0));
    assert_eq!(recorder.to_storage_proof(), prefetched);

    // An unrecorded API probe, not a copied SDK runner or an alternative tariff body.
    let fixture = <Runtime as Config>::prepare(0, 0, true).unwrap();
    benchmarking::commit_db();
    benchmarking::reset_read_write_count();
    <Runtime as Config>::execute(&fixture);
    let after_hooks = recorder.to_storage_proof();
    let hook_size = host.proof_size();
    let _ = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
    let after_root = recorder.to_storage_proof();
    assert!(
      after_root.len() > after_hooks.len(),
      "pending root materialization must expose an additional witness in this fixture"
    );
    assert!(
      after_hooks
        .iter_nodes()
        .all(|node| after_root.iter_nodes().any(|v| v == node))
    );
    println!(
      "AUTHORSHIP_HOST_ROOT {}",
      serde_json::json!({
        "hookWitnessNodes": after_hooks.len(), "rootWitnessNodes": after_root.len(),
        "hookCompactBytes": hook_size, "rootCompactBytes": host.proof_size(),
        "physicalBound": false
      })
    );
    <Runtime as Config>::verify(fixture);
    benchmarking::commit_db();
    assert!(recorder.to_storage_proof().is_empty());
    assert_eq!(host.proof_size(), Some(0));
    assert_eq!(
      polkadot_sdk::sp_io::storage::get(&number).unwrap().to_vec(),
      2u32.encode()
    );
  });
}

#[test]
fn parent_digest_overwrite_records_nodes_without_fetching_hashed_payload() {
  use polkadot_sdk::{
    frame_benchmarking::{BenchmarkRecording, Recording},
    sp_core::storage::TRIE_VALUE_NODE_THRESHOLD,
    sp_runtime::{
      generic::{Digest, DigestItem},
      traits::Hash as _,
    },
  };
  use std::cell::{Cell, RefCell};

  struct CaptureAfterStop<'a> {
    inner: BenchmarkRecording<'a>,
    capture: &'a dyn Fn(),
  }

  impl Recording for CaptureAfterStop<'_> {
    fn start(&mut self) {
      self.inner.start();
    }

    fn stop(&mut self) {
      self.inner.stop();
      (self.capture)();
    }
  }

  let aura = DigestItem::PreRuntime(
    polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
    1u64.encode(),
  );
  let inline = Digest {
    logs: vec![aura.clone()],
  }
  .encode();
  let hashed = Digest {
    logs: vec![DigestItem::PreRuntime(*b"test", vec![7; 4096]), aura],
  }
  .encode();
  assert!(inline.len() < TRIE_VALUE_NODE_THRESHOLD as usize);
  assert!(hashed.len() >= TRIE_VALUE_NODE_THRESHOLD as usize);
  let storage = genesis();
  let mut expected_root = None;
  // Representation controls, not complete imported reference-block histories.
  for (label, prior) in [
    ("absent", None),
    ("inline", Some(inline)),
    ("hashed", Some(hashed)),
  ] {
    let root = with_host(storage.clone(), |host| {
      let key = authorship::owner_keys()[2].clone();
      let recorder = host.recorder().unwrap();
      let stop_proof = RefCell::new(None);
      let positive_read = Cell::new(false);
      let hashed_prior = prior
        .as_ref()
        .filter(|v| v.len() >= TRIE_VALUE_NODE_THRESHOLD as usize);
      let before_start = || {
        match &prior {
          Some(value) => polkadot_sdk::sp_io::storage::set(&key, value),
          None => polkadot_sdk::sp_io::storage::clear(&key),
        }
        benchmarking::commit_db();
        if let Some(value) = hashed_prior {
          assert_eq!(
            polkadot_sdk::sp_io::storage::get(&key).unwrap().to_vec(),
            *value
          );
          assert!(
            recorder
              .to_storage_proof()
              .iter_nodes()
              .any(|node| node == value)
          );
          positive_read.set(true);
          // The read control is setup-only; do not prefill the actual body witness.
          benchmarking::commit_db();
        }
        benchmarking::reset_read_write_count();
        assert!(host.get_whitelist().is_empty());
        assert!(recorder.to_storage_proof().is_empty());
        assert_eq!(host.proof_size(), Some(0));
      };
      let capture = || {
        assert!(stop_proof.borrow().is_none());
        *stop_proof.borrow_mut() = Some(recorder.to_storage_proof());
      };
      let mut recording = CaptureAfterStop {
        inner: BenchmarkRecording::new(&before_start),
        capture: &capture,
      };
      authorship::cases::native_root_instance(0, 0, true, &mut recording).unwrap();
      let proof = stop_proof
        .take()
        .expect("capture before post-body verification");
      assert!(!proof.is_empty());
      if let Some(value) = &prior {
        if hashed_prior.is_some() {
          assert!(positive_read.get());
          assert!(!proof.iter_nodes().any(|node| node == value));
          let old_hash = BlakeTwo256::hash(value);
          let hash_bytes: &[u8] = old_hash.as_ref();
          assert!(
            proof
              .iter_nodes()
              .any(|node| node.windows(hash_bytes.len()).any(|v| v == hash_bytes))
          );
        } else {
          assert!(
            proof
              .iter_nodes()
              .any(|node| node.windows(value.len()).any(|v| v == value.as_slice()))
          );
        }
      }
      println!(
        "AUTHORSHIP_PARENT_DIGEST {}",
        serde_json::json!({
          "case": label, "currentComponents": [["c", 0], ["d", 0]],
          "oldEncodedBytes": prior.as_ref().map(Vec::len), "witnessNonempty": true,
          "oldReadPositiveControl": positive_read.get(), "capturedAfterStopBeforeVerify": true,
          "timingFieldsDiscarded": true, "physicalBound": false
        })
      );
      polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version())
    });
    if let Some(expected) = &expected_root {
      assert_eq!(&root, expected);
    } else {
      expected_root = Some(root);
    }
  }
}

#[test]
fn root_inclusive_runner_matches_absolute_witness_without_whitelist_prefill() {
  use authorship::Config;
  let storage = genesis();
  for c in 0..=1 {
    // Independent, unrecorded host-API comparison; no diagnostic query enters the benchmark body.
    let (hook_bytes, root_bytes, root_nodes) = with_host(storage.clone(), |host| {
      let fixture = <Runtime as Config>::prepare(c, 0, true).unwrap();
      benchmarking::set_whitelist(Vec::new());
      benchmarking::commit_db();
      assert!(host.recorder().unwrap().to_storage_proof().is_empty());
      assert_eq!(host.proof_size(), Some(0));
      <Runtime as Config>::execute(&fixture);
      let hook_bytes = host.proof_size().unwrap();
      let hook_nodes = host.recorder().unwrap().to_storage_proof().len();
      let _ = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
      let root_bytes = host.proof_size().unwrap();
      let root_nodes = host.recorder().unwrap().to_storage_proof().len();
      assert!(root_nodes > hook_nodes);
      assert!(root_bytes > hook_bytes);
      <Runtime as Config>::verify(fixture);
      (hook_bytes, root_bytes, root_nodes)
    });
    assert!(!AllPalletsWithSystem::whitelisted_storage_keys().is_empty());
    for verify in [true, false] {
      with_host(storage.clone(), |host| {
        let rows = <authorship::Pallet<Runtime> as Benchmarking>::run_benchmark(
          b"authorship_initialize_root",
          &[(BenchmarkParameter::c, c), (BenchmarkParameter::d, 0)],
          &AllPalletsWithSystem::whitelisted_storage_keys(),
          verify,
          1,
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert!(host.get_whitelist().is_empty());
        assert!(rows[0].keys.iter().all(|entry| !entry.3));
        // Fails if root moves after stop or if a nonempty proof baseline is subtracted.
        assert_eq!(rows[0].proof_size, root_bytes);
        assert_ne!(rows[0].proof_size, hook_bytes);
        assert_eq!(System::block_number(), 0);
        if !verify {
          println!(
            "AUTHORSHIP_ROOT_INCLUSIVE_HOST {}",
            serde_json::json!({
              "case": c, "geometry": 0, "apiStartProof": 0,
              "hookCompactBytes": hook_bytes, "absoluteRootCompactBytes": root_bytes,
              "runnerProofBytes": rows[0].proof_size, "rootWitnessNodes": root_nodes,
              "timingFieldsDiscarded": true, "physicalBound": false
            })
          );
        }
      });
    }
  }
}
