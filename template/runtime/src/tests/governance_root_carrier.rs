use super::common::{committed_fixture_storage, new_test_ext, with_benchmarking_host};
use crate::{Governance, Runtime};
use codec::{DecodeAll, Encode};
use pallet_governance::{
  CurrentEpochServicePhase, EpochServicePhase, FinalizedProposalIdsByDomain,
  FinalizedProposalOutcomeExpiryBuckets, FinalizedProposals, ProposalMetadataByItem,
  ProposalUrgentAuthorizedAt, ProposalWinningPrimaryOptionByItem,
};
use polkadot_sdk::{
  frame_benchmarking::benchmarking,
  frame_support::{assert_ok, traits::Hooks},
  sp_core::{Blake2Hasher, H256, Hasher},
  sp_runtime::StateVersion,
  sp_state_machine::Backend,
  sp_trie::{
    EMPTY_PREFIX, HashDBT, LayoutV1, MemoryDB, StorageProof, Trie, TrieConfiguration,
    TrieDBBuilder, delta_trie_root,
  },
};
use std::collections::BTreeSet;

#[test]
#[ignore = "opt-in Governance hook/root original-witness gate; no clock or physical pricing"]
fn hook_root_before_commit_preserves_original_witness() {
  type Layout = LayoutV1<Blake2Hasher>;
  assert_eq!(crate::VERSION.state_version(), StateVersion::V1);
  let mut source = new_test_ext();
  let (now, expiry, check_hook) = source.execute_with(|| {
    let prepared = Governance::prepare_independent_full_indices_value_contrast(true).unwrap();
    assert_ok!(<Governance as Hooks<crate::BlockNumber>>::try_state(
      prepared.0
    ));
    prepared
  });
  assert_eq!((now, expiry), (33, 17));
  let parent = committed_fixture_storage(source);
  assert!(parent.children_default.is_empty());
  let keys = [101, 102].map(FinalizedProposalIdsByDomain::<Runtime>::hashed_key_for);
  let values = keys
    .each_ref()
    .map(|key| parent.top.get(key).unwrap().clone());
  assert!(values.iter().all(|value| value.len() == 65540));
  assert_ne!(
    Blake2Hasher::hash(&values[0]),
    Blake2Hasher::hash(&values[1])
  );

  // Build the oracle from immutable data, never by probing the primary host or replaying its hook.
  let wrapper_inputs = [
    polkadot_sdk::frame_support::storage::storage_prefix(b"System", b"Number").to_vec(),
    pallet_governance::PendingEnactments::<Runtime>::hashed_key().to_vec(),
    pallet_governance::LastProcessedEpoch::<Runtime>::hashed_key().to_vec(),
    CurrentEpochServicePhase::<Runtime>::hashed_key().to_vec(),
  ]
  .map(|key| {
    let value = parent.top.get(&key).cloned();
    (key, value)
  });
  let mut expected = parent.top.clone();
  for ((domain, key), value) in [101, 102].into_iter().zip(&keys).zip(&values) {
    let ids = Vec::<u32>::decode_all(&mut &value[..]).unwrap();
    assert_eq!(ids.len(), 16384);
    assert_eq!(ids[0], 2000);
    assert!(
      expected
        .remove(&FinalizedProposals::<Runtime>::hashed_key_for(
          domain, ids[0]
        ))
        .is_some()
    );
    expected.remove(&ProposalMetadataByItem::<Runtime>::hashed_key_for(
      domain, ids[0],
    ));
    expected.remove(&ProposalWinningPrimaryOptionByItem::<Runtime>::hashed_key_for(domain, ids[0]));
    expected.remove(&ProposalUrgentAuthorizedAt::<Runtime>::hashed_key_for(
      domain, ids[0],
    ));
    expected.insert(key.clone(), ids[1..].encode());
  }
  assert!(
    expected
      .remove(&FinalizedProposalOutcomeExpiryBuckets::<Runtime>::hashed_key_for(expiry))
      .is_some()
  );
  expected.insert(
    CurrentEpochServicePhase::<Runtime>::hashed_key().to_vec(),
    EpochServicePhase::RewardExpiry.encode(),
  );
  let delta = parent
    .top
    .keys()
    .chain(expected.keys())
    .collect::<BTreeSet<_>>()
    .into_iter()
    .filter(|key| parent.top.get(*key) != expected.get(*key))
    .map(|key| (key.clone(), expected.get(key).cloned()))
    .collect::<Vec<_>>();
  assert_eq!(delta.len(), 6);
  let mut original = MemoryDB::<Blake2Hasher>::default();
  let root = Layout::trie_build(&mut original, parent.top.clone());
  let expected_root = Layout::trie_root(expected.clone());
  assert_ne!(
    root, expected_root,
    "an old-root fallback must not satisfy the oracle"
  );

  let verify = |claimed: H256, mut replay: MemoryDB<Blake2Hasher>| -> Result<(), &'static str> {
    if claimed != expected_root {
      return Err("post-state root mismatch");
    }
    {
      let trie = TrieDBBuilder::<Layout>::new(&replay, &root).build();
      for (key, value) in keys.iter().zip(&values) {
        if trie
          .get(key)
          .map_err(|_| "missing original index")?
          .as_ref()
          != Some(value)
        {
          return Err("wrong original index");
        }
      }
      for (key, value) in &wrapper_inputs {
        if trie
          .get(key)
          .map_err(|_| "missing original wrapper input")?
          != *value
        {
          return Err("wrong original wrapper input");
        }
      }
    }
    let replayed =
      delta_trie_root::<Layout, _, _, _, _, _>(&mut replay, root, delta.clone(), None, None)
        .map_err(|_| "missing original mutation/repair input")?;
    if replayed != expected_root {
      return Err("whole-delta replay mismatch");
    }
    Ok(())
  };

  with_benchmarking_host(parent, |host| {
    let recorder = host.recorder().unwrap();
    assert!(host.get_whitelist().is_empty());
    assert!(recorder.to_storage_proof().is_empty());
    assert_eq!(host.proof_size(), Some(0));

    // The only useful caller: no setup read, verification, timer or in-span commit.
    let charged = Governance::on_initialize(now);
    let body = recorder.to_storage_proof();
    let observed = H256::from_slice(&polkadot_sdk::sp_io::storage::root(StateVersion::V1));
    let witness = recorder.to_storage_proof();
    let absolute = host.proof_size().unwrap();
    // All remaining work is outside the captured caller/witness boundary.
    assert!(
      body
        .iter_nodes()
        .all(|node| witness.iter_nodes().any(|v| v == node))
    );
    assert!(
      witness.iter_nodes().all(|node| {
        HashDBT::get(&original, &Blake2Hasher::hash(node), EMPTY_PREFIX).as_ref() == Some(node)
      }),
      "newly computed nodes cannot be presented as original inputs"
    );
    for value in &values {
      assert_eq!(
        witness.iter_nodes().filter(|node| *node == value).count(),
        1
      );
    }
    assert_ok!(verify(observed, witness.to_memory_db::<Blake2Hasher>()));
    let compact = witness.to_compact_proof::<Blake2Hasher>(root).unwrap();
    assert_eq!(absolute as usize, compact.encoded_size());
    let (replayed, compact_root) = compact.to_memory_db::<Blake2Hasher>(&root).unwrap();
    assert_eq!(compact_root, root);
    assert_ok!(verify(observed, replayed));
    assert_eq!(
      verify(root, witness.to_memory_db::<Blake2Hasher>()),
      Err("post-state root mismatch")
    );
    let missing = StorageProof::new(
      witness
        .iter_nodes()
        .filter(|node| *node != &values[0])
        .cloned(),
    );
    assert_eq!(
      verify(observed, missing.to_memory_db::<Blake2Hasher>()),
      Err("missing original index")
    );

    benchmarking::commit_db();
    let reset_witness = recorder.to_storage_proof();
    assert!(reset_witness.is_empty());
    assert_eq!(
      verify(observed, reset_witness.to_memory_db::<Blake2Hasher>()),
      Err("missing original index")
    );
    let actual = host
      .pairs(Default::default())
      .unwrap()
      .map(Result::unwrap)
      .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(
      actual, expected,
      "complete TOP, not a Governance-only projection"
    );
    check_hook().unwrap();
    assert_ok!(<Governance as Hooks<crate::BlockNumber>>::try_state(now));
    type W = crate::weights::pallet_governance::SubstrateWeight<Runtime>;
    assert_eq!(
      charged,
      <W as pallet_governance::WeightInfo>::service_pending_enactments(0)
        .saturating_add(<W as pallet_governance::WeightInfo>::service_epoch_catch_up())
        .saturating_add(
          <W as pallet_governance::WeightInfo>::service_finalized_proposal_outcomes(2)
        )
    );
    println!(
      "GOVERNANCE_ROOT_CARRIER_V1 {}",
      serde_json::json!({
        "clock": now, "expiry": expiry, "domains": 2, "due": 2, "population": 32768,
        "requiredValueClasses": 2, "netDeltaKeys": delta.len(), "proofBaseline": 0,
        "bodyWitnessNodes": body.len(), "rootWitnessNodes": witness.len(),
        "rawWitnessBytes": witness.encoded_size(), "compactWitnessBytes": compact.encoded_size(),
        "originalRoot": root, "independentPostRoot": expected_root,
        "originalRawAndCompactDeltaReplay": true, "completeTopMatches": true,
        "staleRootRejected": true, "missingValueRejected": true, "resetWitnessRejected": true,
        "captureBeforeCommitAndVerification": true, "timerCalled": false,
        "preparedNativeNotPhysicalPricingHostMaximumHistoryOrWasm": true
      })
    );
  });
}
