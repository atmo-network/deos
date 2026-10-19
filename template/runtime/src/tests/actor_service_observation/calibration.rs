//! Functional capture-boundary qualification, not an admitted RefTime calibration series.

use super::*;

std::thread_local! {
  static REPLAY_CALLBACKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[test]
fn capture_clock_intervals_reject_invalid_coordinates() {
  let start = Instant::now();
  let end = start
    .checked_add(std::time::Duration::from_nanos(1))
    .unwrap();
  assert_eq!(checked_capture_interval_ns(start, end), Ok(1));
  assert_eq!(checked_capture_interval_ps(1), Ok(1000));
  assert_eq!(
    checked_capture_interval_ns(start, start),
    Err("zero clock interval")
  );
  assert_eq!(
    checked_capture_interval_ns(end, start),
    Err("monotonic clock moved backward")
  );
  assert_eq!(start.duration_since(end), std::time::Duration::ZERO);
  assert_eq!(checked_capture_interval_ps(0), Err("zero clock interval"));
  let largest_ns = u128::from(u64::MAX) / 1000;
  assert_eq!(
    checked_capture_interval_ps(largest_ns),
    Ok(u64::try_from(largest_ns * 1000).unwrap())
  );
  assert_eq!(
    checked_capture_interval_ps(largest_ns + 1),
    Err("clock interval exceeds RefTime range")
  );
  assert_eq!(
    checked_capture_interval_ps(u128::MAX),
    Err("clock interval ps overflow")
  );
  let too_long = start
    .checked_add(std::time::Duration::from_nanos(u64::MAX / 1000 + 1))
    .unwrap();
  assert_eq!(
    checked_capture_interval_ns(start, too_long),
    Err("clock interval exceeds RefTime range")
  );
}

#[test]
fn no_callback_capture_preserves_caller_and_commit_suffix() {
  let output = std::env::var_os("DEOS_SERVICE_OBSERVATION_OUTPUT").map(std::path::PathBuf::from);
  if let Some(output) = &output {
    std::fs::create_dir(output).expect("capture output must be a new directory");
  }
  let mut cases = Vec::new();
  for (label, (parent, fixture)) in [
    ("successful-lp", prepare()),
    ("paired-booking-refusal", prepare_inspection_refusal()),
  ] {
    let diagnostic = run(&parent, &fixture, true, false);
    let clean = run_call_captured(&parent, &fixture, false, false, None, false, || {
      assert_ok!(Actors::actor_prepass(RuntimeOrigin::none()));
    });
    assert_eq!(
      clean.top, diagnostic.top,
      "complete caller TOP, fees and resources"
    );
    assert_eq!(clean.root, diagnostic.root);
    assert!(!clean.diagnostic_callback);
    assert!(diagnostic.diagnostic_callback);
    assert!(!clean.observation.fault_applied);
    assert!(clean.observation.operations.is_empty());
    assert!(clean.observation.transactions.is_empty());
    assert!(clean.observation.markers.is_empty());
    assert!(clean.observation.wall_ns.is_empty());
    assert!(clean.observation.first_seen.is_empty());
    assert_eq!(clean.observation.refusal_roots, [None; 2]);
    assert!(clean.observation.after_task.is_none());
    assert!(clean.elapsed_ns > 0 && clean.root_ns > 0 && clean.commit_ns > 0);
    let complete_db_counts = clean.db_after_commit;
    assert_eq!(clean.db.2, 0, "original snapshot remains pre-commit");
    assert!(
      complete_db_counts.2 > 0,
      "{label}: complete caller must include commit's tracked writes, got {complete_db_counts:?}"
    );
    assert!(
      clean.db.0 <= complete_db_counts.0
        && clean.db.1 <= complete_db_counts.1
        && clean.db.2 <= complete_db_counts.2
        && clean.db.3 <= complete_db_counts.3,
      "cumulative SDK counts cannot decrease at commit"
    );
    assert_eq!(
      complete_db_counts.2, diagnostic.db_after_commit.2,
      "diagnostic callbacks must not add committed writes"
    );
    let changed_top_keys = parent
      .top
      .keys()
      .chain(clean.top.keys())
      .collect::<BTreeSet<_>>()
      .into_iter()
      .filter(|key| parent.top.get(*key) != clean.top.get(*key))
      .count();
    assert!(changed_top_keys > 0);
    assert!(
      usize::try_from(complete_db_counts.2).unwrap() >= changed_top_keys,
      "committed SDK writes must cover every net changed TOP key"
    );
    let disjoint = clean
      .elapsed_ns
      .checked_add(clean.root_ns)
      .unwrap()
      .checked_add(clean.commit_ns)
      .unwrap();
    assert!(
      clean.capture_ns >= disjoint,
      "inclusive span encloses body, root and commit"
    );

    let parent_root = Layout::trie_root(parent.top.clone());
    let mut original = MemoryDB::<Blake2Hasher>::default();
    assert_eq!(
      Layout::trie_build(&mut original, parent.top.clone()),
      parent_root
    );
    assert!(clean.proof.iter_nodes().all(|node| {
      HashDBT::get(&original, &Blake2Hasher::hash(node), EMPTY_PREFIX).as_ref() == Some(node)
    }));
    assert!(
      clean
        .body_proof
        .iter_nodes()
        .all(|node| { clean.proof.iter_nodes().any(|full| full == node) })
    );
    assert_eq!(
      replay(parent_root, &clean.proof, &parent.top, &clean.top, &[]),
      Ok(clean.root),
      "complete mixed delta requires original repair inputs"
    );
    assert!(
      replay(
        parent_root,
        &StorageProof::empty(),
        &parent.top,
        &clean.top,
        &[]
      )
      .is_err()
    );
    let compact = clean
      .proof
      .to_compact_proof::<Blake2Hasher>(parent_root)
      .unwrap();
    let (db, root) = compact.to_memory_db::<Blake2Hasher>(&parent_root).unwrap();
    assert_eq!(root, parent_root);
    REPLAY_CALLBACKS.set(0);
    let nested_replay = benchmark_observation::with_observer(
      |_, _| REPLAY_CALLBACKS.set(REPLAY_CALLBACKS.get() + 1),
      || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
          replay_caller_captured(
            parent_root,
            db.clone(),
            &fixture,
            false,
            clean.observation.recorder.clone(),
            [None; 2],
            None,
          )
        }))
      },
    );
    assert!(
      nested_replay.is_err(),
      "outer observer must refuse replay; observed {} phase callbacks",
      REPLAY_CALLBACKS.get()
    );
    assert_eq!(
      REPLAY_CALLBACKS.get(),
      0,
      "replay must refuse before invocation"
    );
    assert!(ACTIVE.with_borrow(Option::is_none));
    assert_eq!(
      replay_caller_captured(
        parent_root,
        db,
        &fixture,
        false,
        clean.observation.recorder.clone(),
        [None; 2],
        None,
      ),
      clean.root,
      "compact proof must execute the actual no-callback caller"
    );

    for (journal, fault) in [(true, false), (false, true)] {
      let invoked = std::cell::Cell::new(false);
      let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_call_captured(&parent, &fixture, journal, fault, None, false, || {
          invoked.set(true)
        })
      }));
      assert!(refused.is_err());
      assert!(
        !invoked.get(),
        "incompatible capture must refuse before invocation"
      );
      assert!(ACTIVE.with_borrow(Option::is_none));
    }
    let invoked = std::cell::Cell::new(false);
    let nested = benchmark_observation::with_observer(
      |_, _| {},
      || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
          run_call_captured(&parent, &fixture, false, false, None, false, || {
            invoked.set(true)
          })
        }))
      },
    );
    assert!(
      nested.is_err(),
      "an outer callback must refuse no-callback capture"
    );
    assert!(!invoked.get());
    assert!(ACTIVE.with_borrow(Option::is_none));
    if let Some(output) = &output {
      export_case(output, label, parent_root, &clean, &compact);
    }
    cases.push(serde_json::json!({
      "case": label, "parentRoot": format!("{parent_root:?}"),
      "postRoot": format!("{:?}", clean.root), "diagnosticTopEqual": true,
      "diagnosticCallbackInstalled": false, "journalOperations": 0,
      "originalCompactCallerReplay": true, "emptyDeltaWitnessRejected": true,
      "bodyRootCommitEnclosed": true,
      "sdkDbBeforeCommit": clean.db, "sdkDbAfterCommit": complete_db_counts,
      "netChangedTopKeys": changed_top_keys,
      "rawWitnessBytes": clean.proof.encode().len(), "compactBytes": compact.encode().len(),
      "physicalPricingQualified": false, "productionTransferQualified": false
    }));
  }
  if let Some(output) = &output {
    std::fs::write(
      output.join("report.json"),
      serde_json::to_vec_pretty(&serde_json::json!({
        "cases": cases, "captureBoundaryQualified": true,
        "inclusiveCaptureContainsProofSnapshotsAndGlue": true,
        "diagnosticLateFaultExcludedFromNoCallbackMode": true,
        "nestedObserverRejectedBeforeInvocation": true,
        "nestedReplayObserverRejectedBeforeInvocation": true,
        "dbAfterCommitCapturedBeforePostStateEnumeration": true,
        "originalWitnessFrozenBeforeCommit": true,
        "captureClockIntervalsChecked": true,
        "calibrationSeries": false, "physicalPricingQualified": false,
        "completeDomainMaximaQualified": false, "independentKQualified": false
      }))
      .unwrap(),
    )
    .unwrap();
  }
}
