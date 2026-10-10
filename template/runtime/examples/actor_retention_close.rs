//! Complete signed User Close on the unchanged fixed retention corpus.
//! Preparation is synthetic; this supplies no full-history or resource upper.

mod actor_retention_corpus;
mod actor_retention_fixture;
#[path = "actor_retention_fixture/wide_contract.rs"]
mod wide_contract;
#[cfg(test)]
use wide_contract::wide_close_steps;
#[path = "actor_retention_fixture/native_api.rs"]
mod actor_retention_native_api;
#[path = "actor_retention_fixture/pvf.rs"]
mod pvf_fixture;
use deos_runtime as pvf_runtime;

use actor_retention_corpus::{INPUT_SHA256, PAGES, WATCHES, digest, fixed_pages};
use actor_retention_fixture::{
  PREPARED_ACTIVE_ACTORS, PREPARED_IDENTITIES, Prepared, inherents, prepare, query_proof,
  recorded_keys, recorded_nodes, signed_lifecycle,
};
use codec::Encode;
#[cfg(test)]
use codec::MaxEncodedLen;
use deos_runtime::{
  AccountId, Actors, Assets, Balances, Block, Executive, Header, Runtime, RuntimeCall,
  RuntimeEvent, RuntimeHoldReason, System, TransactionPayment,
};
use pallet_deos_actors::{
  ActorContractOf, ActorContractTailChunks, AssetOps, Config, DependencyPlans,
  DependencyRegistrationFreeIndices, DependencyRegistrationFreePositions,
  DependencyRegistrationHeaders, DependencyRegistrationPages, DependencyRegistrationPosition,
  DependencyRegistrationPositions, DependencySourceAllocatorState, DependencyTimedReviews, Event,
  HoldReason, ParkedBalanceEpisodes,
};
#[cfg(test)]
use pallet_deos_actors::{
  AmountResolution, FundingSourcePolicyOf, Predicate, StepErrorPolicy, Task, TaskOf, Trigger,
  TriggerOf,
};
use polkadot_sdk::{
  frame_support::{
    assert_ok,
    dispatch::{DispatchClass, GetDispatchInfo},
    traits::tokens::fungible::InspectHold,
  },
  sp_block_builder::runtime_decl_for_block_builder::BlockBuilder,
  sp_core::{Blake2Hasher, storage::Storage},
  sp_genesis_builder::{PresetId, runtime_decl_for_genesis_builder::GenesisBuilder},
  sp_io::TestExternalities,
  sp_keyring::Sr25519Keyring,
  sp_runtime::{
    Digest, DigestItem,
    traits::{BlakeTwo256, Hash, Header as HeaderT},
  },
  sp_state_machine::Backend,
  sp_trie::{
    proof_size_extension::{ProofSizeExt, RecordingProofSizeProvider},
    recorder::Recorder,
  },
};
use primitives::AssetKind;
#[cfg(test)]
use primitives::OracleFeedId;
use std::collections::BTreeSet;
use std::time::Instant;

type Ops = <Runtime as Config>::AssetOps;

fn wide_close_contract(assets: &[u32], contract: &mut ActorContractOf<Runtime>) {
  wide_contract::wide_close_contract(assets, &actor_retention_fixture::owners(), contract);
}

fn execute_close(
  preset: &PresetId,
  prepared: Prepared,
  pages: &[Vec<u64>],
  api_mode: Option<&str>,
  sdk_wasm: Option<&[u8]>,
  pvf_output: Option<&std::path::Path>,
) {
  assert_eq!(
    prepared.contract.steps.len(),
    <Runtime as Config>::MaxContractSteps::get() as usize
  );
  assert_eq!(prepared.sources.len(), WATCHES);
  let owner = prepared.identity.owner.clone();
  let custody = prepared.identity.sovereign_account.clone();
  let reason = RuntimeHoldReason::Actors(HoldReason::ActorState);
  let code = if pvf_output.is_some() {
    sdk_wasm.expect("PVF export selects explicit Wasm")
  } else {
    &[]
  };
  let mut state = TestExternalities::new_with_code_and_state(
    code,
    prepared.storage,
    deos_runtime::VERSION.state_version(),
  );
  let (extrinsic, nonce, quote, held_before, release) = state.execute_with(|| {
    assert_eq!(
      Actors::actor_contract(prepared.target),
      Some(prepared.contract.clone())
    );
    // Retain custody below readiness so Prepass leaves the original Park for Close.
    // This public adapter preparation remains outside the encoded block.
    assert_ok!(Ops::transfer(
      &custody,
      &Sr25519Keyring::Alice.to_account_id(),
      AssetKind::Local(prepared.first_asset),
      18
    ));
    assert_eq!(Assets::balance(prepared.first_asset, &custody), 99);
    assert!(ParkedBalanceEpisodes::<Runtime>::contains_key(
      prepared.target
    ));
    let hold = Actors::actor_state_hold(prepared.target).unwrap().breakdown;
    let release =
      hold.identity + hold.contract_head + hold.contract_body + hold.detector + hold.run;
    let held_before = <Balances as InspectHold<AccountId>>::balance_on_hold(&reason, &owner);
    assert!(release > 0 && held_before >= release);
    for tail in 0..3u32 {
      assert!(ActorContractTailChunks::<Runtime>::contains_key(
        prepared.target,
        tail
      ));
    }
    let pair = Sr25519Keyring::well_known()
      .find(|key| key.to_account_id() == owner)
      .unwrap()
      .pair();
    let nonce = System::account_nonce(&owner);
    let extrinsic = signed_lifecycle(
      &pair,
      RuntimeCall::Actors(pallet_deos_actors::Call::close_actor {
        actor_id: prepared.target,
      }),
    );
    let quote = TransactionPayment::compute_fee(
      u32::try_from(extrinsic.encoded_size()).unwrap(),
      &extrinsic.get_dispatch_info(),
      0,
    );
    #[cfg(feature = "try-runtime")]
    <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(1).unwrap();
    (extrinsic, nonce, quote, held_before, release)
  });
  state.commit_all().unwrap();
  let storage = Storage {
    top: state
      .backend
      .pairs(Default::default())
      .unwrap()
      .map(Result::unwrap)
      .collect(),
    children_default: Default::default(),
  };
  drop(state);
  println!("public Close pre-state collected: preset={preset}");
  let api_pre_state = api_mode.map(|_| storage.clone());
  let pvf_pre_state = pvf_output.map(|_| storage.clone());
  let mut author = TestExternalities::new_with_code_and_state(
    code,
    storage.clone(),
    deos_runtime::VERSION.state_version(),
  );
  let mut importer = TestExternalities::new_with_code_and_state(
    code,
    storage,
    deos_runtime::VERSION.state_version(),
  );
  let author_root = *author.as_backend().root();
  let importer_root = *importer.as_backend().root();
  if importer_root != author_root {
    let author_top = author
      .backend
      .pairs(Default::default())
      .unwrap()
      .map(Result::unwrap)
      .collect::<std::collections::BTreeMap<_, _>>();
    let importer_top = importer
      .backend
      .pairs(Default::default())
      .unwrap()
      .map(Result::unwrap)
      .collect::<std::collections::BTreeMap<_, _>>();
    let keys = author_top
      .keys()
      .chain(importer_top.keys())
      .collect::<BTreeSet<_>>();
    let differences = keys
      .into_iter()
      .filter(|key| author_top.get(*key) != importer_top.get(*key))
      .map(|key| {
        (
          key.clone(),
          author_top.get(key).cloned(),
          importer_top.get(key).cloned(),
        )
      })
      .collect::<Vec<_>>();
    std::fs::write(
      "/tmp/deos-wide-close-prestate-diagnostic.scale",
      (&author_top, &importer_top).encode(),
    )
    .unwrap();
    eprintln!(
      "prepared roots differ: preset={preset} author={author_root:?} importer={importer_root:?} author_keys={} importer_keys={} differing_keys={} differences={differences:?}",
      author_top.len(),
      importer_top.len(),
      differences.len()
    );
  }
  assert_eq!(
    importer_root, author_root,
    "prepared state copies must agree"
  );
  let parent = Header::new(
    1,
    BlakeTwo256::ordered_trie_root(
      Vec::new(),
      deos_runtime::VERSION.extrinsics_root_state_version(),
    ),
    author_root,
    Default::default(),
    Digest::default(),
  );
  assert_eq!(*importer.as_backend().root(), *parent.state_root());
  let required_nodes = query_proof(&mut importer, &prepared.queries)
    .iter_nodes()
    .cloned()
    .collect::<BTreeSet<_>>();
  println!("public Close required-node query complete: preset={preset}");
  let data = inherents(&parent, 2);
  let recorder = Recorder::<Blake2Hasher>::default();
  author.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
    recorder.clone(),
  )));
  let (
    block,
    before_nodes,
    before_keys,
    after_nodes,
    after_keys,
    raw_increment,
    call_elapsed,
    finalization_elapsed,
  ) = author.execute_with_recorder(recorder.clone(), || {
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
    let mut extrinsics = <Runtime as BlockBuilder<Block>>::inherent_extrinsics(data.clone());
    assert!(extrinsics.len() >= 3);
    for inherent in &extrinsics {
      Executive::apply_extrinsic(inherent.clone())
        .unwrap()
        .unwrap();
    }
    let before_nodes = recorded_nodes(&recorder);
    let before_keys = recorded_keys(&recorder);
    let before_raw = recorder.estimate_encoded_size();
    let signed_close = extrinsic.clone();
    let call_started = Instant::now();
    Executive::apply_extrinsic(signed_close).unwrap().unwrap();
    let call_elapsed = call_started.elapsed();
    let after_nodes = recorded_nodes(&recorder);
    let after_keys = recorded_keys(&recorder);
    let raw_increment = recorder
      .estimate_encoded_size()
      .checked_sub(before_raw)
      .unwrap();
    extrinsics.push(extrinsic);
    let finalization_started = Instant::now();
    let finalized_header = Executive::finalize_block();
    let finalization_elapsed = finalization_started.elapsed();
    (
      Block {
        header: finalized_header,
        extrinsics,
      },
      before_nodes,
      before_keys,
      after_nodes,
      after_keys,
      raw_increment,
      call_elapsed,
      finalization_elapsed,
    )
  });
  let author_commit_started = Instant::now();
  author.commit_all().unwrap();
  println!("public Close block authored: preset={preset}");
  let author_commit_elapsed = author_commit_started.elapsed();
  assert_eq!(*author.as_backend().root(), *block.header.state_root());
  importer.execute_with(|| {
    assert!(
      <Runtime as BlockBuilder<Block>>::check_inherents(block.clone().into(), data.clone()).ok()
    );
  });
  let import_recorder = Recorder::<Blake2Hasher>::default();
  importer.register_extension(ProofSizeExt::new(RecordingProofSizeProvider::new(
    import_recorder.clone(),
  )));
  let import_block = block.clone().into();
  let import_elapsed = importer.execute_with_recorder(import_recorder, || {
    let import_started = Instant::now();
    Executive::execute_block(import_block);
    import_started.elapsed()
  });
  let import_commit_started = Instant::now();
  importer.commit_all().unwrap();
  let import_commit_elapsed = import_commit_started.elapsed();
  assert_eq!(*importer.as_backend().root(), *block.header.state_root());
  if let Some(storage) = api_pre_state {
    println!("SDK API check begins: preset={preset} mode={api_mode:?}");
    let expected = Storage {
      top: importer
        .backend
        .pairs(Default::default())
        .unwrap()
        .map(Result::unwrap)
        .collect(),
      children_default: Default::default(),
    };
    match api_mode.unwrap() {
      "--api-import" => actor_retention_native_api::assert_import(storage, expected, &block),
      "--api-builder" => actor_retention_native_api::assert_builder(
        storage,
        expected,
        &block,
        data.clone(),
        pallet_deos_actors::ActorSemanticStates::<Runtime>::hashed_key_for(prepared.target),
        None,
        sdk_wasm,
      ),
      _ => unreachable!("CLI mode was validated"),
    }
  }
  for (key, _) in &prepared.queries {
    assert!(
      !before_keys.contains(key),
      "inherents must not consume the measured Parked cleanup"
    );
    assert!(after_keys.contains(key));
  }
  assert!(required_nodes.is_subset(&after_nodes));
  let required_unprimed_bytes = required_nodes
    .difference(&before_nodes)
    .map(Encode::encoded_size)
    .sum::<usize>();
  assert!(raw_increment >= required_unprimed_bytes);
  assert_eq!(
    raw_increment,
    after_nodes
      .difference(&before_nodes)
      .map(Encode::encoded_size)
      .sum::<usize>()
  );
  author.execute_with(|| {
    assert!(Actors::actor_identity(prepared.target).is_none());
    assert!(Actors::actor_state_hold(prepared.target).is_none());
    assert!(Actors::actor_contract(prepared.target).is_none());
    assert!(Actors::actor_run_state(prepared.target).is_none());
    assert!(!ParkedBalanceEpisodes::<Runtime>::contains_key(prepared.target));
    assert!(!DependencyTimedReviews::<Runtime>::contains_key(prepared.target));
    assert!(DependencyPlans::<Runtime>::get(prepared.target).is_empty());
    for tail in 0..3u32 {
      assert!(!ActorContractTailChunks::<Runtime>::contains_key(prepared.target, tail));
    }
    assert_eq!(<Balances as InspectHold<AccountId>>::balance_on_hold(&reason, &owner), held_before - release);
    assert_eq!(Balances::free_balance(&custody), prepared.native_before);
    assert_eq!(Assets::balance(prepared.first_asset, &custody), 99);
    assert_eq!(System::account_nonce(&owner), nonce + 1);
    assert_eq!(
      (Actors::actor_identity_count(), Actors::active_actor_count()),
      (PREPARED_IDENTITIES - 1, PREPARED_ACTIVE_ACTORS - 1)
    );
    assert_eq!(DependencySourceAllocatorState::<Runtime>::get().encode(), prepared.allocator);
    assert_eq!(Actors::actor_contract(prepared.temporary[0]), Some(prepared.temporary_contract.clone()));
    assert_eq!(Actors::actor_state_hold(prepared.temporary[1]).unwrap().encode(), prepared.dormant_holds[1]);
    let count = 31 * PAGES as u64;
    for (index, source) in prepared.sources.iter(/* deos-bypass: bounded-iter -- four fixed watch sources. */).copied().enumerate() {
      let selected = &pages[index];
      let local = PAGES / 2;
      let header = DependencyRegistrationHeaders::<Runtime>::get(source);
      assert_eq!((header.count, header.free_count), (PAGES as u32, count - 31));
      assert_eq!(DependencyRegistrationPositions::<Runtime>::get(source, prepared.target), None);
      assert!(!DependencyRegistrationPages::<Runtime>::contains_key(source, selected[local]));
      assert_eq!(DependencyRegistrationPages::<Runtime>::get(source, selected[local - 1]).unwrap().next, Some(selected[local + 1]));
      assert_eq!(DependencyRegistrationPages::<Runtime>::get(source, selected[local + 1]).unwrap().previous, Some(selected[local - 1]));
      for offset in 0..31u64 {
        let position = DependencyRegistrationPosition { page: selected[PAGES - 1], slot: (31 - offset) as u8 };
        let destination = 31 * local as u64 + offset;
        assert_eq!(DependencyRegistrationFreePositions::<Runtime>::get(source, destination), Some(position));
        assert_eq!(DependencyRegistrationFreeIndices::<Runtime>::get(source, position), Some(destination));
      }
      for tail in count - 31..=count {
        assert!(!DependencyRegistrationFreePositions::<Runtime>::contains_key(source, tail));
      }
      for slot in 0..32u8 {
        assert!(!DependencyRegistrationFreeIndices::<Runtime>::contains_key(source, DependencyRegistrationPosition { page: selected[local], slot }));
      }
    }
    assert!(System::events().iter(/* deos-bypass: bounded-iter -- this fixed block's events. */).any(|record| matches!(record.event, RuntimeEvent::Actors(Event::ActorClosed { actor_id, .. }) if actor_id == prepared.target)));
    let fee = System::events().iter(/* deos-bypass: bounded-iter -- one signed Close in this fixed block. */).find_map(|record| match &record.event {
      RuntimeEvent::TransactionPayment(polkadot_sdk::pallet_transaction_payment::Event::TransactionFeePaid { who, actual_fee, tip }) if who == &owner => { assert_eq!(*tip, 0); Some(*actual_fee) },
      _ => None,
    }).unwrap();
    assert!(fee <= quote);
    println!("scope=native_synthetic_retention_public_close preset={preset} block=2 watches={WATCHES} steps={} trigger_bytes={} funding_bytes={} contract_bytes={} required_unprimed_bytes={required_unprimed_bytes} raw_close_increment={raw_increment} declared_fee={quote} actual_fee={fee} released_hold={release} normal_weight={:?}", prepared.contract.steps.len(), prepared.contract.trigger.encoded_size(), prepared.contract.funding.encoded_size(), prepared.contract.encoded_size(), System::block_weight().get(DispatchClass::Normal));
  });
  #[cfg(feature = "try-runtime")]
  author.execute_with(|| {
    <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(2).unwrap()
  });
  #[cfg(feature = "try-runtime")]
  importer.execute_with(|| {
    <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(2).unwrap()
  });
  println!(
    "scope=native_synthetic_retention_public_close_timing preset={preset} signed_call_ns={} finalization_ns={} author_memory_commit_ns={} complete_import_ns={} import_memory_commit_ns={}",
    call_elapsed.as_nanos(),
    finalization_elapsed.as_nanos(),
    author_commit_elapsed.as_nanos(),
    import_elapsed.as_nanos(),
    import_commit_elapsed.as_nanos()
  );
  if let Some(output) = pvf_output {
    let expected = Storage {
      top: importer
        .backend
        .pairs(Default::default())
        .unwrap()
        .map(Result::unwrap)
        .collect(),
      children_default: Default::default(),
    };
    println!("wide public Close PVF export begins: preset={preset}");
    pvf_fixture::export(
      output,
      preset,
      (pvf_pre_state.unwrap(), expected, &parent, block, data),
    )
    .unwrap();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn single_watch_wide_contract(assets: &[u32]) -> ActorContractOf<Runtime> {
    assert_eq!(assets.len(), WATCHES);
    let mut contract = actor_retention_fixture::contract(Trigger::manual(), Vec::new());
    contract.parked_balance_activation = Some(
      pallet_deos_actors::ParkedBalanceActivationOf::<Runtime>::try_from_rules(
        assets
          .iter() // deos-bypass: bounded-iter -- exactly four Task assets; one configured watch.
          .take(<Runtime as Config>::MaxParkedBalanceWatches::get() as usize)
          .map(|asset| pallet_deos_actors::ParkedBalanceRule {
            asset: AssetKind::Local(*asset),
            authored_min_delta: 100,
          })
          .collect(),
      )
      .unwrap(),
    );
    wide_close_contract(assets, &mut contract);
    contract
  }

  #[test]
  fn single_watch_header_preserves_independent_allowlist_width() {
    let contract = single_watch_wide_contract(&(0..WATCHES as u32).collect::<Vec<_>>());
    assert_eq!(
      contract.steps.len(),
      <Runtime as Config>::MaxContractSteps::get() as usize
    );
    assert_eq!(
      contract.trigger.encoded_size(),
      TriggerOf::<Runtime>::max_encoded_len()
    );
    assert_eq!(
      contract.funding.encoded_size(),
      FundingSourcePolicyOf::<Runtime>::max_encoded_len()
    );
    assert_eq!(
      contract
        .parked_balance_activation
        .as_ref()
        .unwrap()
        .watches
        .len(),
      <Runtime as Config>::MaxParkedBalanceWatches::get() as usize
    );
    // Canonical admission bounds total predicates, independently of the raw DNF schema.
    // Four singleton clauses maximize the clause-prefix bytes for this reference domain.
    let predicates = <Runtime as Config>::MaxPredicatesPerStep::get() as usize;
    let clauses = <Runtime as Config>::MaxPreconditionClauses::get() as usize;
    assert_eq!((predicates, clauses), (4, 4));
    let predicate_bytes = Predicate::<
      AssetKind,
      deos_runtime::Balance,
      deos_runtime::BlockNumber,
      OracleFeedId,
    >::max_encoded_len();
    let precondition_bytes = codec::Compact(predicates as u32).encoded_size()
      + predicates * (codec::Compact(1u32).encoded_size() + predicate_bytes);
    let step_bytes = 1
      + precondition_bytes
      + TaskOf::<Runtime>::max_encoded_len()
      + StepErrorPolicy::max_encoded_len();
    for step in &contract.steps {
      assert_eq!(
        step.precondition.as_ref().unwrap().encoded_size(),
        precondition_bytes
      );
      assert_eq!(
        step.task.encoded_size(),
        TaskOf::<Runtime>::max_encoded_len()
      );
      assert_eq!(step.encoded_size(), step_bytes);
    }
    println!(
      "scope=reference_admitted_step_codec predicates={predicates} clauses={clauses} predicate_bytes={predicate_bytes} precondition_bytes={precondition_bytes} task_bytes={} error_policy_bytes={} step_bytes={step_bytes} steps={}",
      TaskOf::<Runtime>::max_encoded_len(),
      StepErrorPolicy::max_encoded_len(),
      contract.steps.len()
    );
  }

  #[test]
  fn admitted_no_policy_resource_codec_stays_within_bound() {
    use pallet_deos_actors::{
      ActorBodyAuthority, ActorContractHeaderOf, ActorContractHeads, ActorRunStateOf,
      ActorStepResourceEnvelope, ParkedBalanceActivationOf, StepControlWeightContext,
      StepControlWeightProvider, TaskEffectWeightProvider, WeightInfo,
    };

    // Admission bounds each bare control/effect coordinate by the idle reserve.
    // The actual no-policy Contract adds only the fixed Parked completion owner.
    // This is a coded-value/codec ceiling, independently of physical Weight soundness.
    let reserve = <Runtime as Config>::ActorOnIdleReserve::get();
    let parked = <Runtime as Config>::WeightInfo::complete_cycle_to_parked_balance();
    let upper = ActorStepResourceEnvelope {
      control: reserve.checked_add(&parked).unwrap(),
      effect: reserve,
    };
    assert!(upper.control.ref_time() < 1u64 << 40);
    assert!(upper.effect.ref_time() < 1u64 << 40);
    assert!(upper.control.proof_size() < 1u64 << 30);
    assert!(upper.effect.proof_size() < 1u64 << 30);
    assert_eq!(upper.encoded_size(), 20);
    assert_eq!(ActorStepResourceEnvelope::max_encoded_len(), 36);

    for preset in <Runtime as GenesisBuilder<Block>>::preset_names() {
      TestExternalities::new(actor_retention_fixture::preset_storage(&preset)).execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Actors::set_user_creation_enabled(deos_runtime::RuntimeOrigin::root(), true));
        let owner = actor_retention_fixture::owners()[0].clone();
        // Match the publicly created LP ledgers used by the retention fixture.
        let mut assets = Vec::new();
        let mut paired = AssetKind::Local(0);
        for _ in 0..WATCHES {
          assert_ok!(deos_runtime::DeosRouter::create_pool(
            deos_runtime::RuntimeOrigin::signed(owner.clone()),
            AssetKind::Native,
            paired,
          ));
          assert_ok!(deos_runtime::AssetConversion::add_liquidity(
            deos_runtime::RuntimeOrigin::signed(owner.clone()),
            Box::new(AssetKind::Native),
            Box::new(paired),
            deos_runtime::UNIT,
            deos_runtime::UNIT / 4,
            1,
            1,
            owner.clone(),
          ));
          let lp = polkadot_sdk::pallet_asset_conversion::Pools::<Runtime>::get((
            AssetKind::Native,
            paired,
          ))
          .unwrap()
          .lp_token;
          assets.push(lp);
          paired = AssetKind::Local(lp);
        }
        let contract = single_watch_wide_contract(&assets);
        let actor = actor_retention_fixture::create_user(&owner, Some(contract.clone()));
        assert_eq!(Actors::actor_contract(actor).as_ref(), Some(&contract));
        let head = ActorContractHeads::<Runtime>::get(actor).unwrap();
        let header_bytes = head.header.encoded_size();
        assert_eq!(
          head.header.parked_balance_activation.as_ref().unwrap().encoded_size(),
          ParkedBalanceActivationOf::<Runtime>::max_encoded_len()
        );
        assert_eq!(head.header.trigger.encoded_size(), TriggerOf::<Runtime>::max_encoded_len());
        assert_eq!(head.header.funding.encoded_size(), FundingSourcePolicyOf::<Runtime>::max_encoded_len());
        assert_eq!(header_bytes, ActorContractHeaderOf::<Runtime>::max_encoded_len() - 8);
        let step_bytes = contract.steps[0].encoded_size();
        assert_eq!(step_bytes, 228);
        assert_eq!(head.encoded_size(), header_bytes + 2 + step_bytes + head.first_step_resources.as_ref().unwrap().encoded_size());
        let chunk_fixed = ActorBodyAuthority::<u64, [u8; 32]>::max_encoded_len()
          + u32::max_encoded_len()
          + 2 * codec::Compact(4u32).encoded_size();
        let mut stored_contract_bytes = head.encoded_size();
        let mut resources = vec![head.first_step_resources.unwrap()];
        for index in 0..3 {
          let chunk = ActorContractTailChunks::<Runtime>::get(actor, index).unwrap();
          assert_eq!(chunk.encoded_size(), chunk_fixed + chunk.steps.len() * step_bytes + chunk.step_resources.iter(/* deos-bypass: bounded-iter -- each typed chunk holds at most MAX_STEPS_PER_TAIL_CHUNK resource records. */).map(Encode::encoded_size).sum::<usize>());
          stored_contract_bytes += chunk.encoded_size();
          resources.extend(chunk.step_resources);
        }
        assert_eq!(resources.len(), <Runtime as Config>::MaxContractSteps::get() as usize);
        for resource in &resources {
          assert!(resource.control.all_lte(upper.control));
          assert!(resource.effect.all_lte(upper.effect));
          assert!(resource.encoded_size() <= upper.encoded_size());
        }
        // Bridge the reached benchmark's operand/policy variants through the real host ports.
        // The compiled publication is checked first; this does not manufacture a Run.
        for (cursor, (step, resource)) in contract.steps.iter().zip(&resources).enumerate() { // deos-bypass: bounded-iter -- exactly twelve admitted Steps and resource slots.
          let cursor = u32::try_from(cursor).unwrap();
          let count = u32::try_from(resources.len()).unwrap();
          let context = StepControlWeightContext {
            cursor,
            steps_in_fragment: if cursor == 0 { 1 } else { (count - (1 + (cursor - 1) / 4 * 4)).min(4) },
            opening_tail_chunks: if cursor == 0 { count.saturating_sub(1).div_ceil(4) } else { 0 },
            predicate_evaluation_units: step.precondition.as_ref().unwrap().evaluation_units(),
          };
          let control = <Runtime as Config>::StepControlWeight::maximum_control_weight(context, step).unwrap();
          let effect = <Runtime as Config>::TaskEffectWeight::maximum_effect_weight(&step.task).unwrap();
          assert_eq!(resource.control, control.checked_add(&parked).unwrap());
          assert_eq!(resource.effect, effect);
          let mut equivalent = step.clone();
          equivalent.on_error = StepErrorPolicy::ContinueNextStep;
          let Task::RemoveLiquidity { asset_a, asset_b, lp_amount, .. } = &mut equivalent.task else {
            panic!("the wide resource witness uses RemoveLiquidity")
          };
          *asset_a = AssetKind::Foreign(u32::MAX);
          *asset_b = AssetKind::Foreign(u32::MAX - 1);
          *lp_amount = AmountResolution::Fixed(u128::MAX);
          assert_eq!(<Runtime as Config>::StepControlWeight::maximum_control_weight(context, &equivalent), Some(control));
          assert_eq!(<Runtime as Config>::TaskEffectWeight::maximum_effect_weight(&equivalent.task), Some(effect));
        }
        let fixed_body_bytes = 2 + 3 * chunk_fixed;
        let resource_bytes: usize = resources.iter(/* deos-bypass: bounded-iter -- the preceding assertion fixes this admitted Contract to MaxContractSteps records. */).map(Encode::encoded_size).sum();
        assert_eq!(stored_contract_bytes, header_bytes + fixed_body_bytes + resources.len() * step_bytes + resource_bytes);
        assert!(stored_contract_bytes <= header_bytes + fixed_body_bytes + resources.len() * (step_bytes + upper.encoded_size()));
        assert_eq!(ActorRunStateOf::<Runtime>::max_encoded_len(), 214);
        println!("scope=reference_admitted_close_codec preset={preset} resource_bytes_total={resource_bytes} header_bytes={header_bytes} chunk_fixed_bytes={chunk_fixed} fixed_body_bytes={fixed_body_bytes} resource_bytes={} resource_slots={} stored_contract_bytes={stored_contract_bytes} run_schema_bytes={} source_bridge=non_stop_remove_liquidity_same_context", upper.encoded_size(), resources.len(), ActorRunStateOf::<Runtime>::max_encoded_len());
        println!("scope=admitted_no_policy_resource_codec preset={preset} reserve={reserve:?} parked_completion={parked:?} control_upper={:?} effect_upper={:?} step_resource_bytes={} schema_maximum_bytes={} steps={}", upper.control, upper.effect, upper.encoded_size(), ActorStepResourceEnvelope::max_encoded_len(), resources.len());
        assert_ok!(Actors::close_actor(deos_runtime::RuntimeOrigin::signed(owner), actor));
        #[cfg(feature = "try-runtime")]
        <Actors as polkadot_sdk::frame_support::traits::Hooks<u32>>::try_state(1).unwrap();
      });
    }
  }

  #[test]
  fn wide_body_storage_copies_preserve_the_prepared_root() {
    let assets = (0..WATCHES as u32).collect::<Vec<_>>();
    let steps = wide_close_steps(&assets);
    let mut storage = Storage::default();
    for (index, chunk) in steps.chunks(4).enumerate() {
      storage.top.insert(
        ActorContractTailChunks::<Runtime>::hashed_key_for(27u64, index as u32),
        chunk.encode(),
      );
    }
    let mut author = TestExternalities::new(storage.clone());
    let mut importer = TestExternalities::new(storage);
    assert_eq!(*author.as_backend().root(), *importer.as_backend().root());
  }
}

fn main() {
  let args = std::env::args().collect::<Vec<_>>();
  if args.len() == 2 && args[1] == "--help" {
    println!(
      "Fixed-corpus native public Close.\nUsage: actor_retention_close FIXED_PAGE_JSON [--api-import|--api-builder|--sdk-wasm PATH|--pvf-fixture PATH NEW_DIR]\nRuns both runtime-owned presets with one signed User Close, the configured watch bound and three body tails. The target joins maximum-width AddressEvent/funding allowlists, a present window and maximum cooldown with twelve maximum-encoded RemoveLiquidity Tasks and four maximum-width predicates each; ParkedBalance excludes policy/nonce terminals. Other Actors retain prior bodies. Reports native elapsed nanoseconds for signed call, finalization, memory commits and complete import, with setup/proof diagnostics outside timing. --api-import uses a generated SDK Core client and StateMachine to verify full native import and late invalid-root rollback with proof recording. --api-builder uses the SDK BlockBuilder through generated Core/BlockBuilder APIs. It checks injected executor and response-decode errors after actual Close, dirty-overlay/proof/diagnostic-extension rollback, successful retry, rejected duplicate and reused finalization; it requires corrected SDK transaction-depth restoration. SDK modes use compiled Rust by default. --sdk-wasm PATH selects actual Wasm for the same builder/provider/error checks, forbids native fallback and checks the complete runtime version; code is supplied externally to SDK execution. --pvf-fixture PATH NEW_DIR binds explicit code in parent storage and exports compact-proof-bound Aura-sealed inputs after complete native seal/state reconciliation; real PVF workers are a separate caller. The output directory must be new. Preparation and the independent expected state remain native and outside the encoded chain. No mode runs a live node. No full-history, complete resource maximum, tariff, compact PoV, physical RefTime or production-Wasm certificate is supplied."
    );
    return;
  }
  let (api_mode, sdk_wasm, pvf_output) = match args.as_slice() {
    [_, _] => (None, None, None),
    [_, _, mode] if matches!(mode.as_str(), "--api-import" | "--api-builder") => {
      (Some(mode.as_str()), None, None)
    }
    [_, _, flag, path] if flag == "--sdk-wasm" => {
      let code = std::fs::read(path).expect("selected SDK diagnostic Wasm is readable");
      println!(
        "sdk_execution=wasm sdk_wasm_sha256={} sdk_wasm_bytes={} code_source=external",
        digest(&code),
        code.len()
      );
      (Some("--api-builder"), Some(code), None)
    }
    [_, _, flag, path, output] if flag == "--pvf-fixture" => {
      let code = std::fs::read(path).expect("selected PVF diagnostic Wasm is readable");
      let output = std::path::PathBuf::from(output);
      std::fs::create_dir(&output).expect("PVF output must be a new directory");
      println!(
        "pvf_wasm_sha256={} pvf_wasm_bytes={} code_source=parent_storage",
        digest(&code),
        code.len()
      );
      (None, Some(code), Some(output))
    }
    _ => panic!("supply the fixed page JSON and optional SDK mode; see --help"),
  };
  let pages = fixed_pages(&args[1]);
  println!(
    "scope=native_synthetic_retention_public_close input_sha256={INPUT_SHA256} actors_lib_sha256={} cargo_lock_sha256={}",
    digest(include_bytes!("../../pallets/actors/src/lib.rs")),
    digest(include_bytes!("../../Cargo.lock"))
  );
  for preset in <Runtime as GenesisBuilder<Block>>::preset_names() {
    println!("public Close preparation starts: preset={preset}");
    execute_close(
      &preset,
      prepare(&preset, &pages, Some(wide_close_contract)),
      &pages,
      api_mode,
      sdk_wasm.as_deref(),
      pvf_output.as_deref(),
    );
  }
  println!(
    "Both prepared native public Close blocks reconciled. Full-history and production resource qualification remain separate."
  );
}
