//! Native SDK caller witness for a public terminal Close inside Executive finalization.
//! Root admission setup is outside the encoded block; no complete resource maximum is claimed.

#[path = "actor_retention_fixture/block.rs"]
mod actor_block_fixture;
#[path = "actor_retention_fixture/native_api.rs"]
mod native_api;
#[path = "actor_retention_fixture/pvf.rs"]
mod pvf_fixture;
use deos_runtime as pvf_runtime;

use actor_block_fixture::{
  contract, create_user, inherents, native_transfer, preset_storage, signed_lifecycle,
};
use codec::Encode;
use deos_runtime::{
  Actors, AssetConversion, Assets, Balances, Block, DeosRouter, Executive, Header, Runtime,
  RuntimeCall, RuntimeEvent, RuntimeOrigin, System,
};
use pallet_deos_actors::{
  ActorSemanticStates, ActorType, AmountResolution, CloseReason, Config, Event, Mutability, Task,
  TaskEffectWeightProvider, Trigger, WeightInfo,
};
use polkadot_sdk::{
  frame_support::{assert_ok, traits::Get},
  sp_block_builder::runtime_decl_for_block_builder::BlockBuilder,
  sp_core::{Blake2Hasher, storage::Storage},
  sp_genesis_builder::{PresetId, runtime_decl_for_genesis_builder::GenesisBuilder},
  sp_io::TestExternalities,
  sp_keyring::Sr25519Keyring,
  sp_runtime::{
    Digest, DigestItem, MultiAddress,
    traits::{BlakeTwo256, Hash, Header as HeaderT},
  },
  sp_state_machine::Backend,
  sp_trie::{
    proof_size_extension::{ProofSizeExt, RecordingProofSizeProvider},
    recorder::Recorder,
  },
  sp_weights::WeightToFee,
};
use primitives::AssetKind;

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

fn run(preset: &PresetId, wasm: Option<&[u8]>, pvf_output: Option<&std::path::Path>) {
  println!("native finalization Close preparation begins: preset={preset}");
  let pair = Sr25519Keyring::Alice.pair();
  let owner = Sr25519Keyring::Alice.to_account_id();
  let local = AssetKind::Local(0);
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
  let (prefix, effect, target, custody, signed, target_key) = prepared.execute_with(|| {
    System::set_block_number(1);
    assert_ok!(Actors::set_user_creation_enabled(
      RuntimeOrigin::root(),
      true
    ));
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
    let task = Task::AddLiquidity {
      asset_a: AssetKind::Native,
      asset_b: local,
      amount_a: AmountResolution::Fixed(deos_runtime::UNIT),
      amount_b: AmountResolution::Fixed(deos_runtime::UNIT),
      min_lp_out: 1,
    };
    let effect = <Runtime as Config>::TaskEffectWeight::maximum_effect_weight(&task).unwrap();
    let base = <Runtime as Config>::BlockResourceBudget::get()
      .limits()
      .actor_base_turn();
    let capacity =
      (base.ref_time() / effect.ref_time()).min(base.proof_size() / effect.proof_size());
    assert!((1..128).contains(&capacity));
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
      native_transfer(&owner, &account, 100 * deos_runtime::UNIT);
      assert_ok!(Assets::transfer(
        RuntimeOrigin::signed(owner.clone()),
        0,
        MultiAddress::Id(account),
        10 * deos_runtime::UNIT
      ));
      assert_ok!(Actors::manual_trigger(RuntimeOrigin::root(), id));
      prefix.push(id);
    }
    let plan = contract(
      Trigger::manual(),
      vec![Task::Transfer {
        to: owner.clone(),
        asset: local,
        amount: AmountResolution::Fixed(deos_runtime::UNIT),
      }],
    );
    let minimum = <Runtime as Config>::MinUserBalance::get();
    let trigger_fee = <Runtime as Config>::WeightToFee::weight_to_fee(
      &<Runtime as Config>::WeightInfo::manual_trigger(),
    );
    let target = create_user(&owner, Some(plan.clone()));
    let account = Actors::actor_identity(target).unwrap().sovereign_account;
    assert_eq!(Balances::free_balance(&account), 0);
    native_transfer(&owner, &account, minimum + trigger_fee);
    assert_ok!(Assets::transfer(
      RuntimeOrigin::signed(owner.clone()),
      0,
      MultiAddress::Id(account.clone()),
      deos_runtime::UNIT
    ));
    assert_ok!(Actors::manual_trigger(
      RuntimeOrigin::signed(owner.clone()),
      target
    ));
    assert_eq!(Balances::free_balance(&account), minimum);
    assert!(
      Actors::attempt_fee_envelope(ActorType::User, &plan.steps, 0)
        .unwrap()
        .total
        > 0
    );
    let signed = signed_lifecycle(
      &pair,
      RuntimeCall::Actors(pallet_deos_actors::Call::close_actor {
        actor_id: *prefix.last().unwrap(),
      }),
    );
    (
      prefix,
      effect,
      target,
      account,
      signed,
      ActorSemanticStates::<Runtime>::hashed_key_for(target),
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
  let block = author.execute_with_recorder(recorder, || {
    let header = Header::new(2, Default::default(), Default::default(), parent.hash(), Digest { logs: vec![DigestItem::PreRuntime(polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID, 2u64.encode())] });
    Executive::initialize_block(&header);
    let mut extrinsics = <Runtime as BlockBuilder<Block>>::inherent_extrinsics(data.clone());
    for xt in &extrinsics { Executive::apply_extrinsic(xt.clone()).unwrap().unwrap(); }
    assert_eq!(Actors::service_header().cursor.unwrap().actor_id, *prefix.last().unwrap());
    let before = Actors::block_resource_state().unwrap();
    assert!(!before.optional_actor_work_halted());
    assert_eq!(before.usage().actor_effect_used(), effect.saturating_mul((prefix.len() - 1) as u64));
    Executive::apply_extrinsic(signed.clone()).unwrap().unwrap();
    extrinsics.push(signed);
    assert_eq!(Actors::service_header().cursor.unwrap().actor_id, target);
    assert!(Actors::actor_identity(target).is_some());
    let native = Balances::free_balance(&custody);
    let local_balance = Assets::balance(0, &custody);
    let header = Executive::finalize_block();
    assert!(Actors::actor_identity(target).is_none());
    assert_eq!((Balances::free_balance(&custody), Assets::balance(0, &custody)), (native, local_balance));
    assert!(System::events().into_iter(/* deos-bypass: bounded-iter -- one finite block with a guarded prefix below 128 Actors and four extrinsics. */).any(|record| matches!(record.event, RuntimeEvent::Actors(Event::ActorClosed { actor_id, reason: CloseReason::CycleAdmissionInsufficient }) if actor_id == target)));
    Block { header, extrinsics }
  });
  author.commit_all().unwrap();
  assert_eq!(*author.as_backend().root(), *block.header.state_root());
  let expected = top(&author);
  let setup_key = ActorSemanticStates::<Runtime>::hashed_key_for(*prefix.last().unwrap());
  native_api::assert_builder(
    storage.clone(),
    expected.clone(),
    &block,
    data.clone(),
    setup_key,
    Some(target_key),
    wasm,
  );
  if let Some(output) = pvf_output {
    pvf_fixture::export(output, preset, (storage, expected, &parent, block, data)).unwrap();
  } else if wasm.is_none() {
    native_api::assert_import(storage, expected, &block);
  }
  println!(
    "native_finalization_close preset={preset} prefix_actors={} target={target} custody_preserved=true phase=finalization reason=CycleAdmissionInsufficient",
    prefix.len()
  );
}

fn main() -> Result<(), String> {
  let arguments = std::env::args().skip(1).collect::<Vec<_>>();
  if arguments == ["--help"] || arguments == ["-h"] {
    println!(
      "Usage: actor_finalization_close [--sdk-wasm PATH | --pvf-fixture PATH NEW_DIR]\n\nRuns both runtime-owned presets through a finite native-prepared finalization Close witness,\nactual SDK BlockBuilder with plain/recording proof-size providers, post-Close\nexecutor/response-decode errors in push and finalization, fresh Core recording/replay\nimports, and late invalid-root rollback.\nBy default the SDK executor uses compiled Rust. --sdk-wasm selects actual Wasm execution\nfor SDK authoring/import and checks the artifact's full runtime version; preparation\nand the independent expected block/state remain native. No native fallback is allowed.\n--pvf-fixture binds the selected code in parent storage and exports proof-bound,\nAura-sealed candidate inputs after an independent native seal/state check.\nPVF worker validation is a separate caller. This does not authenticate relay history,\nrun a live node, replay public setup history or establish a resource maximum."
    );
    return Ok(());
  }
  let (wasm, pvf_output) = match arguments.as_slice() {
    [] => (None, None),
    [flag, path] if flag == "--sdk-wasm" => {
      let code = std::fs::read(path).map_err(|error| format!("cannot read Wasm: {error}"))?;
      (Some(code), None)
    }
    [flag, path, output] if flag == "--pvf-fixture" => {
      let code = std::fs::read(path).map_err(|error| format!("cannot read Wasm: {error}"))?;
      let output = std::path::PathBuf::from(output);
      std::fs::create_dir(&output)
        .map_err(|error| format!("PVF output must be a new directory: {error}"))?;
      (Some(code), Some(output))
    }
    _ => return Err("unexpected arguments; use --help".into()),
  };
  for preset in <Runtime as GenesisBuilder<Block>>::preset_names() {
    run(&preset, wasm.as_deref(), pvf_output.as_deref());
  }
  Ok(())
}
