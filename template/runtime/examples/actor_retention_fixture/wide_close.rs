//! Current-domain signed wide Close through the real SDK caller and optional PVF export.
//! Public native preparation is outside the encoded block; no resource maximum is claimed.

#[path = "block.rs"]
mod actor_block_fixture;
#[path = "native_api.rs"]
mod native_api;
#[path = "pvf.rs"]
mod pvf_fixture;
#[path = "wide_contract.rs"]
mod wide_contract;
use deos_runtime as pvf_runtime;

use actor_block_fixture::{
  contract, create_user, inherents, native_transfer, preset_storage, signed_lifecycle,
};
use codec::Encode;
use deos_runtime::{
  AccountId, Actors, AllPalletsWithSystem, AssetConversion, Assets, Balances, Block, DeosRouter,
  Executive, Header, Runtime, RuntimeCall, RuntimeEvent, RuntimeHoldReason, RuntimeOrigin, System,
};
use pallet_deos_actors::{
  ActorContractTailChunks, ActorSemanticStates, BlockResourcePhase, Config,
  CurrentBlockResourceState, DependencyPlans, Event, HoldReason, ParkedBalanceActivationOf,
  ParkedBalanceEpisodes, ParkedBalanceRule, Trigger,
};
use polkadot_sdk::{
  frame_support::{
    assert_ok,
    dispatch::{DispatchClass, GetDispatchInfo},
    traits::{StorageInfoTrait, tokens::fungible::InspectHold},
    weights::Weight,
  },
  sp_block_builder::runtime_decl_for_block_builder::BlockBuilder,
  sp_core::{Blake2Hasher, Pair, storage::Storage},
  sp_genesis_builder::PresetId,
  sp_io::TestExternalities,
  sp_keyring::Sr25519Keyring,
  sp_runtime::{
    Digest, DigestItem, MultiAddress, StateVersion,
    traits::{BlakeTwo256, Hash, Header as HeaderT},
  },
  sp_state_machine::Backend,
  sp_trie::{
    LayoutV1, MemoryDB, StorageProof, Trie, TrieConfiguration, TrieDBBuilder, delta_trie_root,
    proof_size_extension::{ProofSizeExt, RecordingProofSizeProvider},
    read_trie_value,
    recorder::Recorder,
  },
};
use primitives::AssetKind;
use std::collections::{BTreeMap, BTreeSet};

type DirtyDelta = BTreeMap<Vec<u8>, Option<Vec<u8>>>;

fn dirty_delta(ext: &TestExternalities) -> DirtyDelta {
  assert!(ext.overlayed_changes().children().next().is_none());
  ext
    .overlayed_changes()
    .changes()
    .map(|(key, entry)| (key.clone(), entry.clone().value().cloned()))
    .collect()
}

fn top(ext: &TestExternalities) -> Storage {
  Storage {
    top: ext
      .backend
      .pairs(Default::default())
      .unwrap()
      .map(Result::unwrap)
      .collect(),
    children_default: Default::default(),
  }
}

pub(crate) fn run(preset: &PresetId, wasm: Option<&[u8]>, pvf_output: Option<&std::path::Path>) {
  let owner = Sr25519Keyring::Alice.to_account_id();
  let pair = Sr25519Keyring::Alice.pair();
  let code = if pvf_output.is_some() {
    wasm.expect("PVF export selects explicit Wasm")
  } else {
    &[]
  };
  let mut prepared = TestExternalities::new_with_code_and_state(
    code,
    preset_storage(preset),
    deos_runtime::VERSION.state_version(),
  );
  let (target, custody, lp, signed, held_before, released, native_before, counts) = prepared
    .execute_with(|| {
      System::set_block_number(1);
      assert!(!Actors::user_creation_enabled());
      assert_ok!(Actors::set_user_creation_enabled(
        RuntimeOrigin::root(),
        true
      ));
      let local = AssetKind::Local(0);
      assert_ok!(DeosRouter::create_pool(
        RuntimeOrigin::signed(owner.clone()),
        AssetKind::Native,
        local
      ));
      assert_ok!(AssetConversion::add_liquidity(
        RuntimeOrigin::signed(owner.clone()),
        Box::new(AssetKind::Native),
        Box::new(local),
        100 * deos_runtime::UNIT,
        100 * deos_runtime::UNIT,
        1,
        1,
        owner.clone()
      ));
      let lp =
        polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((AssetKind::Native, local))
          .unwrap()
          .lp_token;
      let mut plan = contract(Trigger::manual(), Vec::new());
      assert_eq!(<Runtime as Config>::MaxParkedBalanceWatches::get(), 1);
      plan.parked_balance_activation = Some(
        ParkedBalanceActivationOf::<Runtime>::try_from_rules(vec![ParkedBalanceRule {
          asset: AssetKind::Local(lp),
          authored_min_delta: 100,
        }])
        .unwrap(),
      );
      let owners = Sr25519Keyring::well_known()
        .map(|key| key.to_account_id())
        .chain((0..28).map(|index| AccountId::from(Sr25519Keyring::numeric(index).public())))
        .collect::<Vec<_>>();
      wide_contract::wide_close_contract(&[lp], &owners, &mut plan);
      assert!(plan.close_when.is_none());
      let target = create_user(&owner, Some(plan.clone()));
      let custody = Actors::actor_identity(target).unwrap().sovereign_account;
      native_transfer(&owner, &custody, 100 * deos_runtime::UNIT);
      assert_ok!(Assets::transfer(
        RuntimeOrigin::signed(owner.clone()),
        lp,
        MultiAddress::Id(custody.clone()),
        99
      ));
      assert_eq!(Actors::actor_contract(target), Some(plan));
      assert_eq!(DependencyPlans::<Runtime>::get(target).len(), 1);
      assert!(ParkedBalanceEpisodes::<Runtime>::contains_key(target));
      assert!(Actors::actor_run_state(target).is_none());
      for tail in 0..3u32 {
        assert!(ActorContractTailChunks::<Runtime>::contains_key(
          target, tail
        ));
      }
      let hold = Actors::actor_state_hold(target).unwrap().breakdown;
      let released =
        hold.identity + hold.contract_head + hold.contract_body + hold.detector + hold.run;
      let held_before = <Balances as InspectHold<AccountId>>::balance_on_hold(
        &RuntimeHoldReason::Actors(HoldReason::ActorState),
        &owner,
      );
      assert!(released > 0 && held_before >= released);
      let signed = signed_lifecycle(
        &pair,
        RuntimeCall::Actors(pallet_deos_actors::Call::close_actor { actor_id: target }),
      );
      (
        target,
        custody.clone(),
        lp,
        signed,
        held_before,
        released,
        Balances::free_balance(&custody),
        (Actors::actor_identity_count(), Actors::active_actor_count()),
      )
    });
  prepared.commit_all().unwrap();
  let storage = top(&prepared);
  let parent = Header::new(
    1,
    BlakeTwo256::ordered_trie_root(
      Vec::new(),
      deos_runtime::VERSION.extrinsics_root_state_version(),
    ),
    *prepared.as_backend().root(),
    Default::default(),
    Digest::default(),
  );
  let data = inherents(&parent, 2);
  let mut author = TestExternalities::new_with_code_and_state(
    code,
    storage.clone(),
    deos_runtime::VERSION.state_version(),
  );
  let recorder = Recorder::<Blake2Hasher>::default();
  author.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
    recorder.clone(),
  )));
  let mut extrinsics = author.execute_with_recorder(recorder.clone(), || {
    let header = Header::new(
      2,
      Default::default(),
      Default::default(),
      parent.hash(),
      Digest {
        logs: vec![DigestItem::PreRuntime(
          polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
          2u64.encode(),
        )],
      },
    );
    Executive::initialize_block(&header);
    let extrinsics = <Runtime as BlockBuilder<Block>>::inherent_extrinsics(data.clone());
    for xt in &extrinsics {
      Executive::apply_extrinsic(xt.clone()).unwrap().unwrap();
    }
    extrinsics
  });
  let snapshot = |author: &mut TestExternalities| {
    let proof = recorder.to_storage_proof();
    let estimate = recorder.estimate_encoded_size();
    let result = author.execute_with(|| {
      (
        CurrentBlockResourceState::<Runtime>::get().expect("real Prepass freezes resource state"),
        *System::block_weight().get(DispatchClass::Normal),
      )
    });
    assert_eq!(
      recorder.to_storage_proof(),
      proof,
      "diagnostics must not prime body proof"
    );
    assert_eq!(recorder.estimate_encoded_size(), estimate);
    result
  };
  let (before, frame_before) = snapshot(&mut author);
  let prefix_delta = dirty_delta(&author);
  assert_eq!(before.block_number(), 2);
  assert_eq!(before.phase(), BlockResourcePhase::ExternalPhase);
  assert_eq!(before.usage().user_dispatch_used(), Weight::zero());
  let frozen = before.budget().unwrap();
  let call_info = signed.get_dispatch_info();
  let declared = call_info.call_weight;
  // With no smaller post-dispatch weight, the SDK uses total signed DispatchInfo, including
  // extensions, plus FRAME base and actual encoded length. Outer proof reclaim is separate.
  let block_weights = <<Runtime as polkadot_sdk::frame_system::Config>::BlockWeights as
    polkadot_sdk::frame_support::traits::Get<polkadot_sdk::frame_system::limits::BlockWeights>>::get();
  let expected_inner = polkadot_sdk::frame_system::calculate_consumed_extrinsic_weight::<RuntimeCall>(
    &block_weights,
    &call_info,
    signed.encoded_size(),
  );
  author.execute_with_recorder(recorder.clone(), || {
    Executive::apply_extrinsic(signed.clone()).unwrap().unwrap();
  });
  extrinsics.push(signed);
  let (after, frame_after) = snapshot(&mut author);
  let body_proof = recorder.to_storage_proof();
  let body_keys = recorder
    .recorded_keys()
    .into_values()
    .flat_map(|keys| keys.into_keys().map(|key| key.as_ref().to_vec()))
    .collect::<BTreeSet<_>>();
  let body_delta = dirty_delta(&author);
  assert_eq!(recorder.to_storage_proof(), body_proof);
  assert_eq!(after.block_number(), before.block_number());
  assert_eq!(after.phase(), BlockResourcePhase::ExternalPhase);
  assert_eq!(
    after.budget().unwrap(),
    frozen,
    "one immutable freeze survives dispatch"
  );
  assert_eq!(
    after.usage().actor_control_used(),
    before.usage().actor_control_used()
  );
  assert_eq!(
    after.usage().actor_effect_used(),
    before.usage().actor_effect_used()
  );
  let charged = after.usage().user_dispatch_used();
  assert_eq!(
    charged, expected_inner,
    "settlement matches its exact SDK total-info/base/length owner"
  );
  assert!(
    declared.all_lte(charged),
    "inner User settlement retains the call declaration"
  );
  assert!(charged.ref_time() > 0 && charged.proof_size() > 0);
  println!(
    "signed_close_frozen preset={preset} block=2 phase=ExternalPhase budget_unchanged=true declared_ref={} declared_proof={} user_ref={} user_proof={} frame_normal_before_ref={} frame_normal_before_proof={} frame_normal_after_ref={} frame_normal_after_proof={} source=public_native_preparation physical_maximum=false",
    declared.ref_time(),
    declared.proof_size(),
    charged.ref_time(),
    charged.proof_size(),
    frame_before.ref_time(),
    frame_before.proof_size(),
    frame_after.ref_time(),
    frame_after.proof_size(),
  );
  let block = author.execute_with_recorder(recorder.clone(), || Block {
    header: Executive::finalize_block(),
    extrinsics,
  });
  let full_proof = recorder.to_storage_proof();
  let final_delta = dirty_delta(&author);
  let parent_root = *author.backend.root();
  assert_eq!(deos_runtime::VERSION.state_version(), StateVersion::V1);
  // The independent complete-backend calculation is off-chain, after all recorded execution.
  // It returns a transaction which is never applied; the live caller overlay remains untouched.
  let (body_root, _) = author.backend.storage_root(
    body_delta
      .iter(/* deos-bypass: bounded-iter -- off-chain one-block dirty snapshot. */)
      .map(|(key, value)| (key.as_slice(), value.as_deref())),
    StateVersion::V1,
  );
  let replay = |proof: &StorageProof, delta: &DirtyDelta| {
    delta_trie_root::<LayoutV1<Blake2Hasher>, _, _, _, _, _>(
      &mut proof.to_memory_db::<Blake2Hasher>(),
      parent_root,
      delta
        .iter(/* deos-bypass: bounded-iter -- off-chain one-block dirty snapshot. */)
        .map(|(key, value)| (key.clone(), value.clone())),
      None,
      None,
    )
  };
  let body_from_body = replay(&body_proof, &body_delta);
  if let Ok(root) = body_from_body.as_ref() {
    assert_eq!(*root, body_root);
  }
  assert_eq!(replay(&full_proof, &body_delta), Ok(body_root));
  assert_eq!(
    replay(&full_proof, &final_delta),
    Ok(*block.header.state_root())
  );
  let body_nodes = body_proof.iter_nodes().cloned().collect::<BTreeSet<_>>();
  let catalog = AllPalletsWithSystem::storage_info();
  let allocator_key = pallet_deos_actors::NextActorId::<Runtime>::hashed_key().to_vec();
  let allocator_value = storage
    .top
    .get(&allocator_key)
    .expect("committed Actor allocator marker");
  // Twox128(pallet name) is the common 16-byte containing prefix.
  let actor_prefix = &allocator_key[..16];
  for delta in [&prefix_delta, &body_delta, &final_delta] {
    assert_eq!(
      delta
        .get(&allocator_key)
        .map_or(Some(allocator_value), Option::as_ref),
      Some(allocator_value)
    );
  }
  let version_key =
    polkadot_sdk::frame_support::traits::StorageVersion::storage_key::<Actors>().to_vec();
  assert!(version_key.starts_with(actor_prefix));
  // Only finite prepared TOP and one-block cloned deltas; no consensus or live trie scan.
  let actor_keys = storage
    .top
    .keys()
    .chain(prefix_delta.keys())
    .chain(body_delta.keys())
    .chain(final_delta.keys())
    .filter(|key| key.starts_with(actor_prefix))
    .collect::<BTreeSet<_>>();
  let mut parent_keys = 0usize;
  let mut final_keys = 0usize;
  let mut version_keys = 0usize;
  let mut namespaces = BTreeSet::new();
  for key in actor_keys {
    parent_keys += usize::from(storage.top.contains_key(key));
    final_keys += usize::from(
      final_delta
        .get(key)
        .map_or_else(|| storage.top.contains_key(key), Option::is_some),
    );
    if key == &version_key {
      version_keys += 1;
      continue;
    }
    let info = catalog
      .iter(/* deos-bypass: bounded-iter -- static runtime storage metadata. */)
      .filter(|info| !info.prefix.is_empty() && key.starts_with(&info.prefix))
      .max_by_key(|info| info.prefix.len())
      .expect("Actor-prefix key must have a declared storage owner or FRAME version owner");
    assert_eq!(info.pallet_name.as_slice(), b"Actors");
    namespaces.insert(String::from_utf8_lossy(&info.storage_name).into_owned());
  }
  println!(
    "signed_close_actor_prefix preset={preset} anchor_unchanged=true pallet_prefix_bytes={} anchor_key_bytes={} anchor_value_bytes={} parent_actor_keys={parent_keys} final_actor_keys={final_keys} declared_namespaces={} frame_version_keys={version_keys} unclassified_union_keys=0 scope=native_source_only",
    actor_prefix.len(),
    allocator_key.len(),
    allocator_value.len(),
    namespaces.len(),
  );
  // This census uses only retained snapshots and isolated proof databases after recording.
  let body_database = body_proof.to_memory_db::<Blake2Hasher>();
  let final_database = full_proof.to_memory_db::<Blake2Hasher>();
  let body_view =
    TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&body_database, &parent_root).build();
  let final_view =
    TrieDBBuilder::<LayoutV1<Blake2Hasher>>::new(&final_database, &parent_root).build();
  let union_keys = prefix_delta
    .keys()
    .chain(body_delta.keys())
    .chain(final_delta.keys())
    .collect::<BTreeSet<_>>();
  let operation = |delta: &DirtyDelta, key: &[u8]| match delta.get(key) {
    None => "not_dirty",
    Some(None) => "delete",
    Some(Some(_)) => "put",
  };
  for key in union_keys {
    let parent_value = storage.top.get(key);
    let expected_hash = parent_value.map(|value| BlakeTwo256::hash(value));
    let body_hash = body_view
      .get_hash(key)
      .map(|hash| {
        assert_eq!(hash, expected_hash);
      })
      .is_ok();
    let final_hash = final_view
      .get_hash(key)
      .map(|hash| {
        assert_eq!(hash, expected_hash);
      })
      .is_ok();
    let body_value = body_view
      .get(key)
      .map(|value| {
        assert_eq!(value.as_ref(), parent_value);
      })
      .is_ok();
    let final_value = final_view
      .get(key)
      .map(|value| {
        assert_eq!(value.as_ref(), parent_value);
      })
      .is_ok();
    assert!(
      final_hash,
      "final proof supports the original key hash/absence path"
    );
    if parent_value.is_none_or(|value| value.len() < 33) {
      assert!(final_value);
    }
    let owner = catalog.iter(/* deos-bypass: bounded-iter -- static metadata labels for one off-chain dirty union. */)
      .filter(|info| !info.prefix.is_empty() && key.starts_with(&info.prefix))
      .max_by_key(|info| info.prefix.len())
      .map(|info| format!("{}::{}", String::from_utf8_lossy(&info.pallet_name), String::from_utf8_lossy(&info.storage_name)))
      .unwrap_or_else(|| "RawOrUnclassified".into());
    let final_same_parent =
      final_delta.get(key).map_or(parent_value, Option::as_ref) == parent_value;
    println!(
      "signed_close_original_key preset={preset} key={} owner={owner} key_bytes={} old_value_bytes={} parent_present={} prefix_op={} body_op={} final_op={} final_same_parent={final_same_parent} body_recorded_key={} body_hash_path={body_hash} body_value={body_value} final_hash_path={final_hash} final_value={final_value}",
      polkadot_sdk::sp_core::hexdisplay::HexDisplay::from(key),
      key.len(),
      parent_value.map_or(0, Vec::len),
      parent_value.is_some(),
      operation(&prefix_delta, key),
      operation(&body_delta, key),
      operation(&final_delta, key),
      body_keys.contains(key),
    );
  }
  for (cut, delta) in [
    ("prefix", &prefix_delta),
    ("body", &body_delta),
    ("final", &final_delta),
  ] {
    let mut classes = [0usize; 4];
    let mut same_parent = 0usize;
    for (key, value) in delta {
      let parent = storage.top.get(key);
      classes[2 * usize::from(parent.is_some()) + usize::from(value.is_some())] += 1;
      same_parent += usize::from(value.as_ref() == parent);
    }
    assert_eq!(classes.into_iter().sum::<usize>(), delta.len());
    println!(
      "signed_close_original_classes preset={preset} cut={cut} dirty={} absent_delete={} absent_put={} occupied_delete={} occupied_put={} effective_same_parent={same_parent} scope=original_parent_not_read_trace",
      delta.len(),
      classes[0],
      classes[1],
      classes[2],
      classes[3]
    );
  }
  // Necessary, not sufficient, condition for the selected mutator's strict-prefix absent-delete
  // defect: an absent removal key that is a strict byte-prefix of a present parent key. This is
  // a bounded scan of the finite prepared TOP, never a consensus storage scan.
  let parent_top = storage.top.keys().collect::<Vec<_>>();
  let mut union_prefix_descendant = 0usize;
  let mut absent_delete_prefix_descendant = 0usize;
  let mut union_prefix_extension = 0usize;
  let mut absent_delete_prefix_extension = 0usize;
  let mut absent_delete_union = 0usize;
  for key in prefix_delta
    .keys()
    .chain(body_delta.keys())
    .chain(final_delta.keys())
    .collect::<BTreeSet<_>>()
  {
    let present = storage.top.contains_key(key);
    let absent_delete = !present && final_delta.get(key).map_or(false, Option::is_none);
    let descendant = parent_top
      .iter(/* deos-bypass: bounded-iter -- finite prepared TOP crossed with one dirty union. */)
      .any(|present| present.len() > key.len() && present.starts_with(key));
    let extension = parent_top
      .iter(/* deos-bypass: bounded-iter -- finite prepared TOP crossed with one dirty union. */)
      .any(|present| key.len() > present.len() && key.starts_with(present));
    union_prefix_descendant += usize::from(descendant);
    union_prefix_extension += usize::from(extension);
    if absent_delete {
      absent_delete_union += 1;
      absent_delete_prefix_descendant += usize::from(descendant);
      absent_delete_prefix_extension += usize::from(extension);
    }
  }
  println!(
    "signed_close_absent_prefix preset={preset} parent_top_keys={} union_absent_delete={absent_delete_union} union_prefix_descendant={union_prefix_descendant} absent_delete_prefix_descendant={absent_delete_prefix_descendant} union_prefix_extension={union_prefix_extension} absent_delete_prefix_extension={absent_delete_prefix_extension} scope=necessary_condition_only",
    parent_top.len()
  );
  assert_eq!(
    absent_delete_prefix_descendant, 0,
    "an absent removal that is a strict prefix of a present key reaches the unqualified mutator branch"
  );
  // Real-state check of the synthetic survivor-encoding rule. A fresh memory trie built from the
  // prepared TOP must reproduce the parent root; each occupied deletion is then replayed in
  // isolation, so the nodes the mutation reads beyond that key's own lookup are exactly its
  // repair reads. This is one-block snapshot evidence, not a batch measurement or a tariff.
  {
    type Layout = LayoutV1<Blake2Hasher>;
    // The fixture's state-machine recorder is hasher-parameterised; trie mutation needs the
    // layout-parameterised trie-db recorder that exposes drain().
    use polkadot_sdk::sp_trie::Recorder as TrieDbRecorder;
    let mut isolated_db = MemoryDB::<Blake2Hasher>::default();
    let isolated_root = Layout::trie_build(&mut isolated_db, storage.top.clone());
    assert_eq!(
      isolated_root, parent_root,
      "the prepared TOP must rebuild the parent root"
    );
    let mut occupied_deletes = 0usize;
    let mut zero_extra = 0usize;
    let mut one_extra = 0usize;
    let mut more_extra = 0usize;
    let mut max_extra_bytes = 0usize;
    for (key, value) in &final_delta {
      if value.is_some() || !storage.top.contains_key(key) {
        continue;
      }
      occupied_deletes += 1;
      let mut lookup_recorder = TrieDbRecorder::<Layout>::new();
      assert!(
        TrieDBBuilder::<Layout>::new(&isolated_db, &isolated_root)
          .with_recorder(&mut lookup_recorder)
          .build()
          .get(key)
          .unwrap()
          .is_some(),
        "an occupied deletion must resolve before its repair is priced"
      );
      let lookup_nodes = lookup_recorder
        .drain()
        .into_iter()
        .map(|record| record.data)
        .collect::<BTreeSet<_>>();
      let mut mutation_recorder = TrieDbRecorder::<Layout>::new();
      let mut isolated_clone = isolated_db.clone();
      delta_trie_root::<Layout, _, _, _, _, _>(
        &mut isolated_clone,
        isolated_root,
        [(key.clone(), None::<Vec<u8>>)],
        Some(&mut mutation_recorder),
        None,
      )
      .unwrap();
      let extras = mutation_recorder
        .drain()
        .into_iter()
        .map(|record| record.data)
        .filter(|node| !lookup_nodes.contains(node))
        .collect::<Vec<_>>();
      match extras.len() {
        0 => zero_extra += 1,
        1 => one_extra += 1,
        _ => more_extra += 1,
      }
      max_extra_bytes = max_extra_bytes.max(extras
          .iter(/* deos-bypass: bounded-iter -- at most one repair node per occupied deletion. */)
          .map(Vec::len)
          .sum::<usize>());
    }
    assert_eq!(occupied_deletes, zero_extra + one_extra + more_extra);
    println!(
      "signed_close_isolated_delete_repair preset={preset} occupied_deletes={occupied_deletes} zero_extra={zero_extra} one_extra={one_extra} more_extra={more_extra} max_extra_bytes={max_extra_bytes} scope=isolated_snapshot_not_batch_or_tariff"
    );
  }
  let mut missing = 0usize;
  let mut new_signed = 0usize;
  let mut shared_changed = 0usize;
  let mut prefix_unchanged = 0usize;
  for (key, value) in &body_delta {
    let phase = match prefix_delta.get(key) {
      None => {
        new_signed += 1;
        "new_signed_dirty"
      }
      Some(old) if old != value => {
        shared_changed += 1;
        "shared_value_changed"
      }
      Some(_) => {
        prefix_unchanged += 1;
        "prefix_dirty_unchanged"
      }
    };
    let singleton = BTreeMap::from([(key.clone(), value.clone())]);
    assert!(replay(&full_proof, &singleton).is_ok());
    if replay(&body_proof, &singleton).is_err() {
      missing += 1;
      let owner_info = catalog
        .iter(/* deos-bypass: bounded-iter -- static runtime storage metadata. */)
        .filter(|info| !info.prefix.is_empty() && key.starts_with(&info.prefix))
        .max_by_key(|info| info.prefix.len());
      let owner = owner_info
        .map(|info| {
          format!(
            "{}::{}",
            String::from_utf8_lossy(&info.pallet_name),
            String::from_utf8_lossy(&info.storage_name)
          )
        })
        .unwrap_or_else(|| "RawOrUnclassified".to_owned());
      let parent_value = author.backend.storage(key).unwrap();
      let lookup_complete = read_trie_value::<LayoutV1<Blake2Hasher>, _>(
        &body_proof.to_memory_db::<Blake2Hasher>(),
        &parent_root,
        key,
        None,
        None,
      )
      .ok()
      .is_some_and(|observed| observed == parent_value);
      if owner.starts_with("Actors::") && lookup_complete && value.is_none() {
        let info = owner_info.expect("typed Actor mutation has a storage owner");
        // This is a finite prepared parent TOP snapshot, never a consensus storage scan.
        let namespace_entries = storage
          .top
          .keys()
          .filter(|key| key.starts_with(&info.prefix))
          .count();
        let expected_single = author
          .backend
          .storage_root(
            std::iter::once((key.as_slice(), value.as_deref())),
            StateVersion::V1,
          )
          .0;
        // Stop at the first sufficient one-node certificate; do not search larger subsets.
        let supplement = full_proof
          .iter_nodes()
          .filter(|node| !body_nodes.contains(*node))
          .find_map(|node| {
            let completed = StorageProof::new(
              body_proof
                .iter_nodes()
                .cloned()
                .chain(std::iter::once(node.clone())),
            );
            replay(&completed, &singleton).ok().map(|root| {
              assert_eq!(root, expected_single);
              (node.len(), BlakeTwo256::hash(node))
            })
          })
          .expect("canonical occupied single delete closes with one surviving-child node");
        println!(
          "signed_close_delete_completion preset={preset} owner={owner} namespace_entries={namespace_entries} key_bytes={} old_value_bytes={} supplemental_nodes=1 supplemental_encoded_bytes={} supplemental_hash={:?}",
          key.len(),
          parent_value.as_ref().unwrap().len(),
          supplement.0,
          supplement.1,
        );
      }
      println!(
        "signed_close_write_gap preset={preset} owner={owner} phase={phase} deletion={} parent_present={} recorded_read={} body_lookup_complete={lookup_complete}",
        value.is_none(),
        parent_value.is_some(),
        body_keys.contains(key),
      );
    }
  }
  let final_nodes = full_proof.iter_nodes().cloned().collect::<BTreeSet<_>>();
  assert!(body_nodes.is_subset(&final_nodes));
  assert_eq!(
    recorder.to_storage_proof(),
    full_proof,
    "off-chain checks must not record nodes"
  );
  assert_eq!(
    *author.backend.root(),
    parent_root,
    "verification must not commit a transaction"
  );
  println!(
    "signed_close_write_union preset={preset} prefix_dirty={} body_dirty={} final_dirty={} body_nodes={} final_nodes={} added_union_nodes={} body_delta_complete={} body_from_final_complete=true final_delta_complete=true missing_single_keys={missing} new_signed_dirty={new_signed} shared_value_changed={shared_changed} prefix_dirty_unchanged={prefix_unchanged}",
    prefix_delta.len(),
    body_delta.len(),
    final_delta.len(),
    body_nodes.len(),
    final_nodes.len(),
    final_nodes.difference(&body_nodes).count(),
    body_from_body.is_ok(),
  );
  author.commit_all().unwrap();
  assert_eq!(*author.as_backend().root(), *block.header.state_root());
  // Diagnostic queries stay outside recording; priming changes proof-reclaimed dispatch Weight.
  author.execute_with(|| {
    assert!(CurrentBlockResourceState::<Runtime>::get().is_none());
    let finalized = Actors::finalized_block_resource_telemetry().unwrap();
    assert_eq!(finalized.block_number(), 2);
    assert_eq!(finalized.budget(), frozen);
    assert_eq!(finalized.usage().user_dispatch_used(), charged);
    assert!(Actors::actor_identity(target).is_none());
    assert!(Actors::actor_state_hold(target).is_none());
    assert!(Actors::actor_contract(target).is_none());
    assert!(Actors::actor_run_state(target).is_none());
    assert!(!ParkedBalanceEpisodes::<Runtime>::contains_key(target));
    assert!(DependencyPlans::<Runtime>::get(target).is_empty());
    for tail in 0..3u32 { assert!(!ActorContractTailChunks::<Runtime>::contains_key(target, tail)); }
    assert_eq!(<Balances as InspectHold<AccountId>>::balance_on_hold(&RuntimeHoldReason::Actors(HoldReason::ActorState), &owner), held_before - released);
    assert_eq!((Balances::free_balance(&custody), Assets::balance(lp, &custody)), (native_before, 99));
    assert_eq!((Actors::actor_identity_count(), Actors::active_actor_count()), (counts.0 - 1, counts.1 - 1));
    assert!(System::events().into_iter(/* deos-bypass: bounded-iter -- one four-extrinsic diagnostic block. */).any(|record| matches!(record.event, RuntimeEvent::Actors(Event::ActorClosed { actor_id, reason: pallet_deos_actors::CloseReason::OwnerInitiated }) if actor_id == target)));
  });
  let expected = top(&author);
  native_api::assert_builder(
    storage.clone(),
    expected.clone(),
    &block,
    data.clone(),
    ActorSemanticStates::<Runtime>::hashed_key_for(target),
    None,
    wasm,
  );
  if let Some(output) = pvf_output {
    pvf_fixture::export(output, preset, (storage, expected, &parent, block, data)).unwrap();
  } else if wasm.is_none() {
    native_api::assert_import(storage, expected, &block);
  }
  println!(
    "current_wide_close preset={preset} target={target} watches=1 steps=12 tails=3 custody_preserved=true synthetic_setup=true physical_maximum=false"
  );
}
