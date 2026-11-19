//! Fixed native component correctness; no clock, block-admission or pricing evidence.

use super::{
  common, h1_storage_trace,
  production_block_replay::{current_top_storage, preset_genesis_storage},
};
use crate::{
  Authorship, Balances, CollatorSelection, Runtime, RuntimeCall, RuntimeOrigin, Session, Staking,
  System, configs::RuntimeSessionRotation,
};
use codec::Encode;
use pallet_session_rotation::SessionRotation;
use polkadot_sdk::{
  frame_executive::OnInitializeWithWeightRegistration,
  frame_support::{
    assert_ok,
    dispatch::DispatchClass,
    traits::{Hooks, StorageInfoTrait},
    weights::Weight,
  },
  pallet_balances, pallet_collator_selection, pallet_session,
  sp_core::{
    H256,
    storage::{Storage, well_known_keys},
  },
  sp_externalities::set_and_run_with_externalities,
  sp_io::TestExternalities,
  sp_keyring::Sr25519Keyring,
  sp_runtime::{
    Digest, DigestItem,
    traits::{BlakeTwo256, Dispatchable, Hash},
  },
  sp_state_machine::{Backend, Ext, OverlayedChanges, create_proof_check_backend},
  sp_trie::StorageProof,
};
use std::collections::{BTreeMap, BTreeSet};

type Top = BTreeMap<Vec<u8>, Vec<u8>>;
const ED: crate::Balance = crate::EXISTENTIAL_DEPOSIT;

fn alice() -> crate::AccountId {
  Sr25519Keyring::Alice.to_account_id()
}
fn bob() -> crate::AccountId {
  Sr25519Keyring::Bob.to_account_id()
}
fn root(top: &Top) -> H256 {
  BlakeTwo256::trie_root(
    top.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
    crate::VERSION.state_version(),
  )
}
fn cache_key() -> Vec<u8> {
  Authorship::storage_info()
    .into_iter()
    .find(|i| i.storage_name == b"Author")
    .unwrap()
    .prefix
}
fn validators_key() -> Vec<u8> {
  pallet_session::Validators::<Runtime>::hashed_key().to_vec()
}

fn parent(purge: bool) -> Storage {
  let preset = polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into();
  let storage = preset_genesis_storage(&[], &preset);
  assert!(storage.children_default.is_empty());
  assert_eq!(storage.top.get(well_known_keys::CODE), Some(&Vec::new()));
  let mut ext = TestExternalities::new(storage);
  ext.execute_with(|| {
    assert_eq!(
      Staking::native_security_mode(),
      pallet_staking::NativeSecurityMode::TrustedSet
    );
    assert_eq!(Session::validators(), vec![bob(), alice()]);
    assert_eq!(Session::queued_keys().len(), 2);
    assert!(Session::load_keys(&alice()).is_some());
    assert!(Session::load_keys(&bob()).is_some());
    assert!(Balances::free_balance(alice()) > 18 * ED);
    assert!(Balances::free_balance(bob()) >= ED);
    assert_ok!(Balances::transfer_keep_alive(
      RuntimeOrigin::signed(alice()),
      CollatorSelection::account_id().into(),
      16 * ED
    ));
    if purge {
      for who in [alice(), bob()] {
        assert_ok!(
          RuntimeCall::Session(pallet_session::Call::purge_keys {})
            .dispatch(RuntimeOrigin::signed(who))
        );
      }
    }
    <RuntimeSessionRotation as SessionRotation<crate::BlockNumber>>::rotate();
    <RuntimeSessionRotation as SessionRotation<crate::BlockNumber>>::rotate();
    assert_eq!(Session::current_index(), 2);
    assert_eq!(
      Session::validators(),
      if purge { vec![] } else { vec![bob(), alice()] }
    );
    assert_eq!(
      polkadot_sdk::pallet_aura::Authorities::<Runtime>::get().to_vec(),
      vec![
        Sr25519Keyring::Bob.public().into(),
        Sr25519Keyring::Alice.public().into()
      ]
    );
    assert!(!polkadot_sdk::sp_io::storage::exists(&cache_key()));
  });
  let storage = common::committed_fixture_storage(ext);
  assert_eq!(
    crate::VERSION.state_version(),
    polkadot_sdk::sp_runtime::StateVersion::default()
  );
  // Authenticate constructor/layout on a separate peer, never by probing the cold primary.
  common::with_benchmarking_host(storage.clone(), |_| {
    assert_eq!(
      polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
      root(&storage.top).as_bytes()
    );
  });
  storage
}

fn component_digest() -> Digest {
  Digest {
    logs: vec![DigestItem::PreRuntime(
      polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID,
      1u64.encode(),
    )],
  }
}
fn prefix(digest: &Digest) {
  System::initialize(&1, &System::block_hash(0), digest);
}
fn initialize() {
  assert_eq!(<(Authorship,) as OnInitializeWithWeightRegistration<Runtime>>::on_initialize_with_weight_registration(1), Weight::zero());
}
fn gap() {
  assert_ok!(
    RuntimeCall::Balances(pallet_balances::Call::transfer_keep_alive {
      dest: bob().into(),
      value: ED
    })
    .dispatch(RuntimeOrigin::signed(alice()))
  );
}
fn finalize() {
  Authorship::on_finalize(1);
}
fn unobserved() {
  {
    let digest = component_digest();
    prefix(&digest);
    initialize();
  }
  gap();
  finalize();
}

#[derive(Debug, PartialEq, Eq)]
struct Outputs {
  alice: crate::Balance,
  bob: crate::Balance,
  pot: crate::Balance,
  issuance: crate::Balance,
  last: crate::BlockNumber,
  weight: polkadot_sdk::frame_system::ConsumedWeight,
  cache: bool,
}
fn outputs() -> Outputs {
  Outputs {
    alice: Balances::free_balance(alice()),
    bob: Balances::free_balance(bob()),
    pot: Balances::free_balance(CollatorSelection::account_id()),
    issuance: Balances::total_issuance(),
    last: pallet_collator_selection::LastAuthoredBlock::<Runtime>::get(alice()),
    weight: System::block_weight(),
    cache: polkadot_sdk::sp_io::storage::exists(&cache_key()),
  }
}

#[derive(Clone, Copy, Debug)]
enum Fault {
  None,
  Reset,
  Commit,
  Observer,
}
struct Capture {
  boundaries: [Option<u32>; 4],
  end_i: StorageProof,
  read: StorageProof,
  transition: StorageProof,
  post: Top,
  outputs: Outputs,
  root: Vec<u8>,
}
fn capture(storage: Storage, fault: Fault) -> Capture {
  common::with_benchmarking_host(storage, |host| {
    let recorder = host.recorder().unwrap();
    let b_i;
    let e_i;
    let end_i;
    {
      let digest = component_digest();
      prefix(&digest);
      b_i = host.proof_size(); // I.begin; no storage verification in the retained scope.
      initialize();
      e_i = host.proof_size(); // I.end.
      end_i = recorder.to_storage_proof();
    }
    match fault {
      Fault::None => (),
      Fault::Reset => recorder.reset(),
      Fault::Commit => polkadot_sdk::frame_benchmarking::benchmarking::commit_db(),
      Fault::Observer => {
        let _ = System::account(Sr25519Keyring::Charlie.to_account_id());
      }
    }
    gap();
    let b_f = host.proof_size(); // F.begin.
    finalize();
    let e_f = host.proof_size(); // F.end; freeze an owned copy, not a live recorder clone.
    let read = recorder.to_storage_proof();
    // Separately labelled actual root-repair probe, after read-witness closure.
    let root = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
    let transition = recorder.to_storage_proof();
    Capture {
      boundaries: [b_i, e_i, b_f, e_f],
      end_i,
      read,
      transition,
      root,
      outputs: outputs(),
      post: current_top_storage().top,
    }
  })
}
fn nodes(proof: &StorageProof) -> BTreeSet<Vec<u8>> {
  proof.iter_nodes().cloned().collect()
}

#[derive(Debug, PartialEq, Eq)]
enum Replay {
  Accepted,
  RootMismatch,
  BackendReadFault,
}
fn replay(r0: H256, proof: StorageProof, expected: H256, outputs_expected: &Outputs) -> Replay {
  let backend = create_proof_check_backend::<BlakeTwo256>(r0, proof).unwrap();
  // Authenticate this required original read explicitly before Ext (which panics on backend errors).
  if backend.storage(&validators_key()).is_err() {
    return Replay::BackendReadFault;
  }
  let mut overlay = OverlayedChanges::default();
  let mut ext = Ext::new(&mut overlay, &backend, None);
  set_and_run_with_externalities(&mut ext, || {
    unobserved();
    let actual = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
    if actual != expected.as_bytes() {
      return Replay::RootMismatch;
    }
    // Replay verifies touched outputs only; untouched None-path pot/last-authored
    // records are authenticated by the independent complete-state root, not extra proof reads.
    assert_eq!(Balances::free_balance(alice()), outputs_expected.alice);
    assert_eq!(Balances::free_balance(bob()), outputs_expected.bob);
    assert_eq!(System::block_weight(), outputs_expected.weight);
    assert!(!polkadot_sdk::sp_io::storage::exists(&cache_key()));
    Replay::Accepted
  })
}

fn reference(storage: &Storage) -> (Top, Outputs) {
  TestExternalities::new(storage.clone()).execute_with(|| {
    unobserved();
    (current_top_storage().top, outputs())
  })
}
fn logical(storage: &Storage, purge: bool, expected: &Top) {
  h1_storage_trace::recorded(|| {
    TestExternalities::new(storage.clone()).execute_with(|| {
      let sample;
      {
        let digest = component_digest();
        prefix(&digest);
        sample = h1_storage_trace::Sample::new();
        sample.phase("authorship_initialize", initialize);
      }
      sample.phase("authorship_gap", gap);
      sample.phase("authorship_finalize", finalize);
      let trace = sample.finish().unwrap();
      let cache = &trace["owners"]["Authorship.Author"]["calls"];
      assert_eq!(cache["Get"], 1);
      assert_eq!(cache["Delete"], 1);
      assert_eq!(
        cache["Put"],
        if purge {
          serde_json::Value::Null
        } else {
          1.into()
        }
      );
      assert_eq!(
        trace["owners"]["System.BlockWeight"]["calls"]["Put"],
        if purge { 1 } else { 2 }
      );
      assert_eq!(&current_top_storage().top, expected);
    })
  });
}

#[test]
fn authorship_phase_driver_fixed_native_cohort() {
  for purge in [false, true] {
    let storage = parent(purge);
    let r0 = root(&storage.top);
    let before = TestExternalities::new(storage.clone()).execute_with(outputs);
    let (post, expected_outputs) = reference(&storage);
    let expected = root(&post);
    assert_ne!(r0, expected, "fixed root-changing fixture premise");
    let reward = if purge { 0 } else { (before.pot - ED) / 2 };
    assert_eq!(expected_outputs.alice, before.alice - ED + reward);
    assert_eq!(expected_outputs.bob, before.bob + ED);
    assert_eq!(expected_outputs.pot, before.pot - reward);
    assert_eq!(expected_outputs.issuance, before.issuance);
    assert_eq!(expected_outputs.last, if purge { before.last } else { 1 });
    assert!(!expected_outputs.cache);
    let mut weight = polkadot_sdk::frame_system::ConsumedWeight::default();
    if !purge {
      use pallet_collator_selection::WeightInfo;
      weight.accrue(
        <Runtime as pallet_collator_selection::Config>::WeightInfo::note_author(),
        DispatchClass::Mandatory,
      );
    }
    assert_eq!(expected_outputs.weight, weight);
    let primary = capture(storage.clone(), Fault::None);
    assert_eq!(primary.post, post);
    assert_eq!(primary.outputs, expected_outputs);
    assert_eq!(primary.root, expected.as_bytes());
    assert!(nodes(&primary.end_i).is_subset(&nodes(&primary.read)));
    assert_eq!(
      replay(r0, primary.transition.clone(), expected, &expected_outputs),
      Replay::Accepted
    );
    logical(&storage, purge, &post);
    println!(
      "AUTHORSHIP_PHASE_DRIVER purge={purge} B_I/E_I/B_F/E_F={:?} read_nodes={} transition_nodes={} root={expected:?}",
      primary.boundaries,
      primary.read.len(),
      primary.transition.len()
    );
    if purge {
      continue;
    }
    for fault in [Fault::Reset, Fault::Commit] {
      let tainted = capture(storage.clone(), fault);
      let lost = nodes(&tainted.end_i)
        .difference(&nodes(&tainted.read))
        .cloned()
        .collect::<Vec<_>>();
      assert!(
        !lost.is_empty(),
        "{fault:?} must lose actual retained original nodes"
      );
      assert_eq!(
        tainted.post, post,
        "final parity alone cannot detect lineage loss"
      );
      assert!(
        !nodes(&tainted.end_i).is_subset(&nodes(&tainted.transition)),
        "root probe cannot silently repair retention loss"
      );
      println!(
        "AUTHORSHIP_PHASE_DRIVER {fault:?} rejected: lost_original_nodes={}",
        lost.len()
      );
    }
    let intact = nodes(&primary.transition);
    let mut required = None;
    let mut repair = None;
    // Fixed Some proof only: discover a necessary original input, never remove R0 itself.
    for node in &intact {
      let proof = StorageProof::new(intact.iter().filter(|n| *n != node).cloned());
      let Ok(backend) = create_proof_check_backend::<BlakeTwo256>(r0, proof.clone()) else {
        continue;
      };
      if backend.storage(&validators_key()).is_err() {
        required = Some(proof);
        break;
      }
    }
    let reduced = required.expect("BLOCKED: no authenticated required original Validators input");
    assert_eq!(
      replay(r0, reduced, expected, &expected_outputs),
      Replay::BackendReadFault
    );
    println!("AUTHORSHIP_PHASE_DRIVER required original Validators input removal rejected");
    // Repair-only extraction is conditional: only nodes not used by the full read sequence qualify.
    for node in nodes(&primary.transition).difference(&nodes(&primary.read)) {
      let proof = StorageProof::new(intact.iter().filter(|n| *n != node).cloned());
      if create_proof_check_backend::<BlakeTwo256>(r0, proof.clone()).is_err() {
        continue;
      }
      // Any earlier missing read is unsupported, not a repair refusal.
      let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        replay(r0, proof, expected, &expected_outputs)
      }));
      if matches!(attempt, Ok(Replay::RootMismatch)) {
        repair = Some(());
        break;
      }
    }
    println!(
      "AUTHORSHIP_PHASE_DRIVER repair_only={}",
      if repair.is_some() {
        "rejected by independent root"
      } else {
        "unsupported: no authenticated repair-only refusal"
      }
    );
    let observer = capture(storage.clone(), Fault::Observer);
    assert_eq!(observer.post, post);
    let extra = nodes(&observer.read)
      .difference(&nodes(&primary.read))
      .cloned()
      .collect::<Vec<_>>();
    if extra.is_empty() {
      println!(
        "AUTHORSHIP_PHASE_DRIVER observer donation unsupported: Charlie input already covered"
      );
    } else {
      let key = polkadot_sdk::frame_system::Account::<Runtime>::hashed_key_for(
        Sr25519Keyring::Charlie.to_account_id(),
      );
      let clean = create_proof_check_backend::<BlakeTwo256>(r0, primary.read.clone()).unwrap();
      let tainted = create_proof_check_backend::<BlakeTwo256>(r0, observer.read).unwrap();
      assert!(clean.storage(&key).is_err());
      assert_eq!(
        tainted.storage(&key).unwrap(),
        storage.top.get(&key).cloned()
      );
      println!(
        "AUTHORSHIP_PHASE_DRIVER observer original-input donation authenticated nodes={} (not a general detector)",
        extra.len()
      );
    }
  }
}
