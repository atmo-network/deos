//! Current-domain signed wide Close through the real SDK caller and optional PVF export.
//! Public native preparation is outside the encoded block; no resource maximum is claimed.

#[path = "actor_retention_fixture/block.rs"]
mod actor_block_fixture;
#[path = "actor_retention_fixture/native_api.rs"]
mod native_api;
#[path = "actor_retention_fixture/pvf.rs"]
mod pvf_fixture;
#[path = "actor_retention_fixture/wide_contract.rs"]
mod wide_contract;
use deos_runtime as pvf_runtime;

use actor_block_fixture::{
  contract, create_user, inherents, native_transfer, preset_storage, signed_lifecycle,
};
use codec::Encode;
use deos_runtime::{
  AccountId, Actors, AssetConversion, Assets, Balances, Block, DeosRouter, Executive, Header,
  Runtime, RuntimeCall, RuntimeEvent, RuntimeHoldReason, RuntimeOrigin, System,
};
use pallet_deos_actors::{
  ActorContractTailChunks, ActorSemanticStates, Config, DependencyPlans, Event, HoldReason,
  ParkedBalanceActivationOf, ParkedBalanceEpisodes, ParkedBalanceRule, Trigger,
};
use polkadot_sdk::{
  frame_support::{assert_ok, traits::tokens::fungible::InspectHold},
  sp_block_builder::runtime_decl_for_block_builder::BlockBuilder,
  sp_core::{Blake2Hasher, Pair, storage::Storage},
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
  let block = author.execute_with_recorder(recorder, || {
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
    for xt in &extrinsics {
      Executive::apply_extrinsic(xt.clone()).unwrap().unwrap();
    }
    Executive::apply_extrinsic(signed.clone()).unwrap().unwrap();
    extrinsics.push(signed);
    Block {
      header: Executive::finalize_block(),
      extrinsics,
    }
  });
  author.commit_all().unwrap();
  assert_eq!(*author.as_backend().root(), *block.header.state_root());
  // Diagnostic queries stay outside recording; priming changes proof-reclaimed dispatch Weight.
  author.execute_with(|| {
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

fn main() -> Result<(), String> {
  let arguments = std::env::args().skip(1).collect::<Vec<_>>();
  if arguments == ["--help"] || arguments == ["-h"] {
    println!(
      "Usage: actor_wide_close [--sdk-wasm PATH | --pvf-fixture PATH NEW_DIR]\n\nRuns both named presets through current one-watch, twelve-Step wide no-policy signed Close.\nNative public preparation is outside the encoded block; SDK builders and recording/replay\nCore imports compare complete TOP/root and push executor/decode/duplicate rollback.\n--sdk-wasm selects actual Wasm with full version checks and no native fallback.\n--pvf-fixture binds parent code and exports Aura-sealed candidates and complete expected\nresults to a new directory. Worker validation is separate. The old four-watch corpus is\nnot used or modified. This is finite schema/caller acceptance, not complete physical\nresource/tariff, population, public history, live node or relay provenance."
    );
    return Ok(());
  }
  let (wasm, output) = match arguments.as_slice() {
    [] => (None, None),
    [flag, path] if flag == "--sdk-wasm" => (
      Some(std::fs::read(path).map_err(|error| format!("cannot read Wasm: {error}"))?),
      None,
    ),
    [flag, path, directory] if flag == "--pvf-fixture" => {
      let code = std::fs::read(path).map_err(|error| format!("cannot read Wasm: {error}"))?;
      let output = std::path::PathBuf::from(directory);
      std::fs::create_dir(&output)
        .map_err(|error| format!("PVF output must be a new directory: {error}"))?;
      (Some(code), Some(output))
    }
    _ => return Err("unexpected arguments; use --help".into()),
  };
  for preset in <Runtime as GenesisBuilder<Block>>::preset_names() {
    run(&preset, wasm.as_deref(), output.as_deref());
  }
  Ok(())
}
