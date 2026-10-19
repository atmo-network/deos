//! Finite native I/K reachability and opt-in proof-bound export, without physical timing.

use super::*;
use crate::{
  AssetConversion, DeosRouter, configs::address_event_ingress::AddressEventIngressExtension,
};
use pallet_deos_actors::{
  ActorContract, ActorContractHeads, ActorContractOf, ActorProcesses, ActorSemanticStates,
  AssetFilter, Config, FundingSourcePolicy, OnUnfunded, SourceFilter, TaskEffectWeightProvider,
};
use polkadot_sdk::{
  frame_support::storage::TransactionOutcome,
  sp_core::Blake2Hasher,
  sp_genesis_builder::{PresetId, runtime_decl_for_genesis_builder::GenesisBuilder},
  sp_runtime::traits::{Dispatchable, TransactionExtension},
  sp_trie::{
    proof_size_extension::{ProofSizeExt, RecordingProofSizeProvider},
    recorder::Recorder,
  },
};

#[derive(Clone, Copy, Debug)]
enum Case {
  Inspection,
  Complete,
}

impl Case {
  fn label(self) -> &'static str {
    match self {
      Self::Inspection => "inspection",
      Self::Complete => "complete",
    }
  }
}

fn contract(
  trigger: pallet_deos_actors::TriggerOf<Runtime>,
  tasks: Vec<pallet_deos_actors::TaskOf<Runtime>>,
) -> ActorContractOf<Runtime> {
  ActorContract {
    trigger,
    cooldown_blocks: 0,
    window: Some(ScheduleWindow {
      start: 1,
      end: <Runtime as Config>::MinWindowLength::get() + 10,
    }),
    steps: tasks
      .into_iter()
      .map(|task| StepOf::<Runtime> {
        precondition: None,
        task,
        on_error: StepErrorPolicy::AbortCycle,
      })
      .collect::<Vec<_>>()
      .try_into()
      .unwrap(),
    funding: FundingSourcePolicy::OwnerOnly,
    on_unfunded: OnUnfunded::Close,
    close_when: None,
    parked_balance_activation: None,
    auto_close_at_cycle_nonce: None,
  }
}

fn credit(
  owner: &crate::AccountId,
  target: &crate::AccountId,
  amount: crate::Balance,
  certified: bool,
) {
  let call = RuntimeCall::Balances(polkadot_sdk::pallet_balances::Call::transfer_keep_alive {
    dest: MultiAddress::Id(target.clone()),
    value: amount,
  });
  assert_ok!(polkadot_sdk::frame_support::storage::with_transaction(
    || {
      let origin = RuntimeOrigin::signed(owner.clone());
      let info = call.get_dispatch_info();
      let pre = certified.then(|| {
        AddressEventIngressExtension
          .prepare((), &origin, &call, &info, 0)
          .unwrap()
      });
      let (post, result) = match call.dispatch(origin) {
        Ok(post) => (post, Ok(())),
        Err(error) => (error.post_info, Err(error.error)),
      };
      if let Some(pre) = pre {
        AddressEventIngressExtension::post_dispatch_details(pre, &info, &post, 0, &result).unwrap();
      }
      if result.is_ok() {
        TransactionOutcome::Commit(result)
      } else {
        TransactionOutcome::Rollback(result)
      }
    }
  ));
}

fn run(
  preset: &PresetId,
  case: Case,
  wasm: &[u8],
  output: Option<&std::path::Path>,
) -> serde_json::Value {
  let signer = polkadot_sdk::sp_keyring::Sr25519Keyring::Alice.pair();
  let owner = crate::AccountId::from(signer.public());
  let storage = preset_genesis_storage(wasm, preset);
  let mut prepared =
    TestExternalities::new_with_code_and_state(wasm, storage, crate::VERSION.state_version());
  let (prefix, target, custody, pipeline, effect) = prepared.execute_with(|| {
    System::set_block_number(1);
    super::super::common::set_consensus_timestamp(crate::SLOT_DURATION);
    assert_ok!(Actors::set_user_creation_enabled(
      RuntimeOrigin::root(),
      true
    ));
    let local = primitives::AssetKind::Local(0);
    assert_ok!(DeosRouter::create_pool(
      RuntimeOrigin::signed(owner.clone()),
      primitives::AssetKind::Native,
      local
    ));
    assert_ok!(AssetConversion::add_liquidity(
      RuntimeOrigin::signed(owner.clone()),
      Box::new(primitives::AssetKind::Native),
      Box::new(local),
      100 * crate::UNIT,
      100 * crate::UNIT,
      1,
      1,
      owner.clone()
    ));
    let task = Task::AddLiquidity {
      asset_a: primitives::AssetKind::Native,
      asset_b: local,
      amount_a: AmountResolution::Fixed(crate::UNIT),
      amount_b: AmountResolution::Fixed(crate::UNIT),
      min_lp_out: 1,
    };
    let effect = <Runtime as Config>::TaskEffectWeight::maximum_effect_weight(&task).unwrap();
    let base = BlockResourceBudgetValue::get().limits().actor_base_turn();
    let capacity =
      (base.ref_time() / effect.ref_time()).min(base.proof_size() / effect.proof_size());
    assert!(
      (1..128).contains(&capacity),
      "existing bounded effect-prefix geometry changed"
    );
    let mut prefix = Vec::new();
    for _ in 0..=capacity {
      let id = Actors::next_actor_id();
      assert_ok!(Actors::create_system_actor(
        RuntimeOrigin::root(),
        owner.clone(),
        Mutability::Mutable,
        Some(contract(
          Trigger::manual(),
          vec![task.clone(), Task::StopCycle]
        ))
      ));
      let account = Actors::actor_identity(id).unwrap().sovereign_account;
      credit(&owner, &account, 100 * crate::UNIT, false);
      assert_ok!(Assets::transfer(
        RuntimeOrigin::signed(owner.clone()),
        0,
        MultiAddress::Id(account),
        10 * crate::UNIT
      ));
      assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), id));
      prefix.push(id);
    }
    let tasks = match case {
      Case::Inspection => vec![task.clone(); <Runtime as Config>::MaxContractSteps::get() as usize],
      Case::Complete => Vec::new(),
    };
    let target = Actors::next_actor_id();
    assert_ok!(Actors::create_user_actor(
      RuntimeOrigin::signed(owner.clone()),
      Mutability::Mutable,
      Some(contract(
        Trigger::address_event(SourceFilter::Any, AssetFilter::Any),
        tasks
      ))
    ));
    let custody = Actors::actor_identity(target).unwrap().sovereign_account;
    let pipeline = Actors::actor_cost_quote(target)
      .unwrap()
      .prospective_pipeline_fee
      .unwrap()
      .total_fee;
    credit(&owner, &custody, 100 * crate::UNIT, true);
    let state = Actors::active_actor_state(target).unwrap();
    assert!(state.hot.pending_signal && state.run_state.is_none());
    assert_eq!(state.identity.cycle_nonce, 0);
    <crate::Timestamp as polkadot_sdk::frame_support::traits::Hooks<u32>>::on_finalize(1);
    (prefix, target, custody, pipeline, effect)
  });
  prepared.commit_all().unwrap();
  let storage = prepared.execute_with(current_top_storage);
  let parent = Header::new(
    1,
    Default::default(),
    *prepared.as_backend().root(),
    Default::default(),
    Digest::default(),
  );
  let data = inherent_data_for(&parent, 2);
  let mut author = TestExternalities::new_with_code_and_state(
    wasm,
    storage.clone(),
    crate::VERSION.state_version(),
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
    let extrinsics = data.create_extrinsics();
    for xt in &extrinsics {
      Executive::apply_extrinsic(xt.clone()).unwrap().unwrap();
    }
    extrinsics
  });
  let xt = author.execute_with(|| {
    assert_eq!(
      Actors::service_header().cursor.unwrap().actor_id,
      *prefix.last().unwrap()
    );
    let before = Actors::block_resource_state().unwrap();
    assert!(!before.optional_actor_work_halted());
    assert_eq!(
      before.usage().actor_effect_used(),
      effect.saturating_mul((prefix.len() - 1) as u64)
    );
    let close = RuntimeCall::Actors(pallet_deos_actors::Call::close_actor {
      actor_id: *prefix.last().unwrap(),
    });
    actors_integration_tests::signed_extrinsic(&signer, System::account_nonce(&owner), close)
  });
  author.execute_with_recorder(recorder.clone(), || {
    Executive::apply_extrinsic(xt.clone()).unwrap().unwrap();
  });
  extrinsics.push(xt);
  let nonce = author.execute_with(|| {
    assert_eq!(Actors::service_header().cursor.unwrap().actor_id, target);
    System::account_nonce(&owner)
  });
  let (remarks, next_weight, rejected) = author.execute_with_recorder(recorder.clone(), || {
    saturate_user_dispatch(&signer, 2, nonce, &mut extrinsics, true)
  });
  let body_proof = recorder.to_storage_proof();
  let (native_before, semantic_before, head_before, process_before) = author.execute_with(|| {
    assert!(
      rejected.is_some(),
      "the finite authoring filler reaches its public resource boundary"
    );
    let drain_source = Actors::block_resource_state().unwrap();
    assert!(!drain_source.optional_actor_work_halted());
    let limits = drain_source.budget().unwrap().limits();
    let shared_remaining = limits
      .shared_economic()
      .checked_sub(
        &drain_source
          .usage()
          .actor_effect_used()
          .saturating_add(drain_source.usage().user_dispatch_used()),
      )
      .unwrap();
    assert!(shared_remaining.proof_size() < effect.proof_size());
    assert!(shared_remaining.proof_size() < next_weight.proof_size());
    assert!(
      effect.ref_time() <= shared_remaining.ref_time(),
      "this is a declared ProofSize frontier, not RefTime saturation"
    );
    (
      Balances::free_balance(&custody),
      ActorSemanticStates::<Runtime>::get(target)
        .unwrap()
        .encode(),
      ActorContractHeads::<Runtime>::get(target).unwrap().encode(),
      ActorProcesses::<Runtime>::get(target).unwrap().encode(),
    )
  });
  let (header, trace) = super::super::h1_storage_trace::recorded(|| {
    // Observer snapshots must not prime the proof-size provider used by the actual block.
    let sample = author.execute_with(super::super::h1_storage_trace::Sample::new);
    assert_eq!(recorder.to_storage_proof(), body_proof);
    let header = author.execute_with_recorder(recorder.clone(), || {
      sample.phase("candidate-finalization", Executive::finalize_block)
    });
    let final_proof = recorder.to_storage_proof();
    let trace = author.execute_with(|| sample.finish().unwrap());
    assert_eq!(recorder.to_storage_proof(), final_proof);
    (header, trace)
  });
  author.execute_with(|| {
    let state = Actors::active_actor_state(target).unwrap();
    match case {
      Case::Inspection => {
        assert_eq!(Actors::actor_identity(target).unwrap().cycle_nonce, 0, "refusal does not open the Pipeline");
        assert!(state.hot.pending_signal && state.run_state.is_none());
        assert_eq!(ActorSemanticStates::<Runtime>::get(target).unwrap().encode(), semantic_before);
        assert_eq!(ActorContractHeads::<Runtime>::get(target).unwrap().encode(), head_before);
        assert_eq!(ActorProcesses::<Runtime>::get(target).unwrap().encode(), process_before);
        assert_eq!(Balances::free_balance(&custody), native_before);
        assert_eq!(Actors::service_header().cursor.unwrap().actor_id, target);
        assert!(trace["owners"]["System.Account"]["calls"]["Get"].as_u64().unwrap_or(0) >= 2, "I reaches actual Native spendability before refusal");
        assert!(trace["owners"]["Actors.ActorContractHead"]["calls"]["Get"].as_u64().unwrap_or(0) >= 2, "I loads the Step and capacity Head");
      }
      Case::Complete => {
        assert_eq!(Actors::actor_identity(target).unwrap().cycle_nonce, 1, "one complete zero-Step Cycle opens");
        assert!(!state.hot.pending_signal && state.run_state.is_none());
        assert_eq!(Balances::free_balance(&custody), native_before - pipeline);
        assert!(pallet_deos_actors::DeadlineHandles::<Runtime>::contains_key(target), "complete K includes window placement");
      }
    }
    assert!(!System::events().iter().any(|record| matches!(record.event, RuntimeEvent::Actors(Event::TransferExecuted { actor_id, .. } | Event::StepSkipped { actor_id, .. } | Event::StepFailed { actor_id, .. }) if actor_id == target)));
  });
  let block = Block { header, extrinsics };
  author.commit_all().unwrap();
  let expected = author.execute_with(current_top_storage);
  assert_eq!(*author.as_backend().root(), *block.header.state_root());
  if let Some(output) = output {
    let label: PresetId = format!("{preset}.{}", case.label()).into();
    pvf_fixture::export(output, &label, (storage, expected, &parent, block, data)).unwrap();
  }
  eprintln!(
    "pvf_first_native preset={preset} case={} prefix={} remarks={remarks} reachable=true exported={} physical_observation=false",
    case.label(),
    prefix.len(),
    output.is_some()
  );
  serde_json::json!({"preset":preset, "case":case.label(), "prefix_actors":prefix.len(), "signed_remarks":remarks, "target":target, "native_reachable":true, "exported":output.is_some()})
}

#[test]
fn pvf_first_service_scenarios_are_natively_reachable() {
  let wasm = std::env::var_os("DEOS_PVF_FIRST_WASM");
  let output = std::env::var_os("DEOS_PVF_FIRST_OUTPUT");
  assert_eq!(
    wasm.is_some(),
    output.is_some(),
    "PVF export requires both explicit Wasm and new output path"
  );
  let code = wasm
    .map(|path| std::fs::read(path).unwrap())
    .unwrap_or_default();
  let output = output.map(std::path::PathBuf::from);
  if let Some(path) = &output {
    std::fs::create_dir(path).expect("PVF export directory must be new");
  }
  let mut cases = Vec::new();
  for preset in <Runtime as GenesisBuilder<Block>>::preset_names() {
    for case in [Case::Inspection, Case::Complete] {
      cases.push(run(&preset, case, &code, output.as_deref()));
    }
  }
  assert_eq!(
    cases.len(),
    4,
    "the profile freezes two presets and two source cases"
  );
  if let Some(output) = output {
    let mut files = std::fs::read_dir(&output)
      .unwrap()
      .map(|entry| entry.unwrap().path())
      .collect::<Vec<_>>();
    files.sort();
    assert_eq!(
      files.len(),
      20,
      "four complete five-file candidate sets are required"
    );
    let digest = |bytes: &[u8]| {
      format!(
        "{}",
        polkadot_sdk::sp_core::hexdisplay::HexDisplay::from(
          &polkadot_sdk::sp_io::hashing::sha2_256(bytes)
        )
      )
    };
    let inputs = files.iter().map(|path| serde_json::json!({"file":path.file_name().unwrap().to_str().unwrap(), "sha256":digest(&std::fs::read(path).unwrap())})).collect::<Vec<_>>();
    let manifest = serde_json::json!({"profile":"pvf-first-service-v1", "stage":"NATIVE_VERIFIED_EXPORTED_NOT_PVF_ACCEPTED", "wasm_sha256":digest(&code), "para_id":crate::PARACHAIN_ID, "cases":cases, "inputs":inputs, "physical_observation":false, "physical_envelope_qualified":false});
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
      .write(true)
      .create_new(true)
      .open(output.join("profile.json"))
      .unwrap();
    file
      .write_all(serde_json::to_string_pretty(&manifest).unwrap().as_bytes())
      .unwrap();
  }
}
