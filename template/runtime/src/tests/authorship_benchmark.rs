//! Source/recording-boundary checks only; no physical samples or production pricing.

mod host_backend;

use super::{common, h1_storage_trace};
use crate::benchmarks::authorship::{self, Config, Pallet, cases};
use crate::{AccountId, AllPalletsWithSystem, Authorship, CollatorSelection, Runtime, System};
use codec::Encode;
use polkadot_sdk::{
  frame_benchmarking::{BenchmarkParameter, Benchmarking, Recording},
  frame_support::traits::{Get, Hooks, StorageInfoTrait, WhitelistedStorageKeys},
  frame_system,
  pallet_authorship::EventHandler,
  sp_consensus_aura::{digests::CompatibleDigestItem, sr25519::AuthoritySignature},
  sp_runtime::{generic::DigestItem, traits::Header as _},
};

fn prepared_geometry(d: u32) -> serde_json::Value {
  let mut digest = System::digest();
  let digest_bytes = digest.encoded_size();
  let items = digest.logs.len();
  let bound: u32 = <Runtime as polkadot_sdk::pallet_aura::Config>::MaxAuthorities::get();
  assert_eq!(bound, 20);
  let (engine, slot) = digest.logs.last().unwrap().as_pre_runtime().unwrap();
  assert_eq!(engine, polkadot_sdk::sp_consensus_aura::AURA_ENGINE_ID);
  assert_eq!(slot, u64::from(bound - 1).encode());
  let empty_header_bytes = crate::Header::new(
    System::block_number(),
    Default::default(),
    Default::default(),
    Default::default(),
    Default::default(),
  )
  .encoded_size();
  let limit = crate::configs::RuntimeBlockLength::get()
    .max_header_size()
    .min(polkadot_sdk::cumulus_primitives_core::relay_chain::MAX_HEAD_DATA_SIZE)
    as usize;
  assert!(digest_bytes + empty_header_bytes <= limit);
  assert_eq!(
    System::block_size() as usize,
    digest_bytes + empty_header_bytes
  );
  let mut sealed_shape = crate::Header::new(
    System::block_number(),
    Default::default(),
    Default::default(),
    Default::default(),
    digest.clone(),
  );
  // Encoding only: no valid signature or complete-block reachability is claimed.
  sealed_shape
    .digest_mut()
    .push(DigestItem::aura_seal(AuthoritySignature::from(
      polkadot_sdk::sp_core::sr25519::Signature::from_raw([0; 64]),
    )));
  let minimum_sealed_header_bytes = sealed_shape.encoded_size();
  let mut foreign_payload_bytes = 0;
  for item in &digest.logs[..items - 1] {
    let (engine, payload) = item.as_pre_runtime().unwrap();
    assert_eq!(engine, *b"test");
    if d == 2 {
      assert!(payload.is_empty());
    }
    foreign_payload_bytes += payload.len();
  }
  match d {
    0 => assert_eq!(items, 1),
    1 => {
      assert_eq!(items, 2);
      if let DigestItem::PreRuntime(_, payload) = &mut digest.logs[0] {
        payload.push(7);
      }
    }
    2 => digest
      .logs
      .insert(items - 1, DigestItem::PreRuntime(*b"test", Vec::new())),
    _ => panic!("unknown native geometry"),
  }
  let next_initialization_header_bytes = empty_header_bytes + digest.encoded_size();
  if d != 0 {
    assert!(
      next_initialization_header_bytes > limit,
      "selected corner must be maximal within its class"
    );
    assert!(
      minimum_sealed_header_bytes
        > polkadot_sdk::cumulus_primitives_core::relay_chain::MAX_HEAD_DATA_SIZE as usize,
      "initialization superset is not a complete sealed-block witness"
    );
  }
  serde_json::json!({
    "digestBytes": digest_bytes, "digestItems": items, "foreignPayloadBytes": foreign_payload_bytes,
    "initializationHeaderBytes": empty_header_bytes + digest_bytes, "headerCeilingBytes": limit,
    "nextInitializationHeaderBytes": (d != 0).then_some(next_initialization_header_bytes),
    "minimumSealedHeaderBytes": minimum_sealed_header_bytes,
  })
}

fn with_observer_ext(run: impl FnOnce()) {
  h1_storage_trace::recorded(|| common::seeded_test_ext().execute_with(run));
}

fn assert_whitelist_coverage(trace: &serde_json::Value, record_initialize: bool) {
  let owners = trace["owners"].as_object().unwrap();
  let mut exclusions = authorship::owner_keys().to_vec();
  if record_initialize {
    exclusions.extend(authorship::initialization_owner_keys());
  }
  for info in AllPalletsWithSystem::storage_info() {
    let owner = format!(
      "{}.{}",
      String::from_utf8(info.pallet_name).unwrap(),
      String::from_utf8(info.storage_name).unwrap()
    );
    if owners.contains_key(&owner) {
      for entry in AllPalletsWithSystem::whitelisted_storage_keys()
        .into_iter()
        .filter(|key| key.key.starts_with(&info.prefix))
      {
        assert!(
          exclusions.contains(&entry.key),
          "observed owner {owner} inherits a global whitelist exclusion"
        );
      }
    }
  }
}

#[test]
fn authorship_complete_metadata_is_extra_and_categorical() {
  assert!(<Pallet<Runtime> as Benchmarking>::benchmarks(false).is_empty());
  let metadata = <Pallet<Runtime> as Benchmarking>::benchmarks(true);
  assert_eq!(metadata.len(), 3);
  for name in [
    b"authorship_complete".as_slice(),
    b"authorship_with_initialize",
    b"authorship_initialize_root",
  ] {
    let entry = metadata.iter().find(|entry| entry.name == name).unwrap();
    assert_eq!(
      entry.components,
      [(BenchmarkParameter::c, 0, 2), (BenchmarkParameter::d, 0, 2)]
    );
  }
  let whitelist = AllPalletsWithSystem::whitelisted_storage_keys();
  assert!(
    whitelist
      .iter()
      .any(|entry| entry.key == authorship::owner_keys()[0])
  );
  struct NoRecording;
  impl Recording for NoRecording {}
  with_observer_ext(|| {
    let root = polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version());
    for (c, d) in [(3, 0), (0, 3)] {
      assert!(cases::native_instance(c, d, true, &mut NoRecording).is_err());
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
        root
      );
      assert!(cases::native_initialize_instance(c, d, true, &mut NoRecording).is_err());
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
        root
      );
      assert!(cases::native_root_instance(c, d, true, &mut NoRecording).is_err());
      assert_eq!(
        polkadot_sdk::sp_io::storage::root(crate::VERSION.state_version()),
        root
      );
    }
  });
}

#[test]
fn authorship_complete_native_empty_and_funded() {
  for c in 0..=1 {
    for d in 0..=2 {
      with_observer_ext(|| {
        let mut recording = h1_storage_trace::BenchmarkRecording::new("authorship_complete");
        cases::native_instance(c, d, true, &mut recording).unwrap();
        let trace = recording.finish();
        let geometry = prepared_geometry(d);
        assert_eq!(
          trace["owners"]["System.Digest"]["get_value_bytes"],
          geometry["digestBytes"]
        );
        assert_whitelist_coverage(&trace, false);
        assert_eq!(
          trace["owners"]["System.Digest"]["calls"]["Put"],
          serde_json::Value::Null
        );
        let cache = &trace["owners"]["Authorship.Author"];
        assert_eq!(cache["calls"]["Get"], 1);
        assert_eq!(cache["calls"]["Put"], 1);
        assert_eq!(cache["calls"]["Delete"], 1);
        assert_eq!(cache["absent_reads"], 1);
        assert_eq!(cache["net_changed_keys"], 0);
        assert_eq!(
          trace["owners"]["CollatorSelection.LastAuthoredBlock"]["calls"]["Put"],
          1
        );
        for owner in ["System.Digest", "Aura.Authorities", "Session.Validators"] {
          assert_eq!(trace["owners"][owner]["calls"]["Get"], 1, "{owner}");
        }
        // One internal nonzero charge and the Executive wrapper's zero-return registration.
        assert_eq!(trace["owners"]["System.BlockWeight"]["calls"]["Put"], 2);
        assert_eq!(trace["owners"]["System.BlockWeight"]["calls"]["Get"], 2);
        println!(
          "AUTHORSHIP_NATIVE {}",
          serde_json::json!({"case": c, "geometry": d, "input": geometry, "trace": trace})
        );
      });
    }
  }
}

#[test]
fn authorship_with_initialize_records_generated_digest_in_all_six_cells() {
  for c in 0..=1 {
    for d in 0..=2 {
      with_observer_ext(|| {
        if (c, d) == (0, 0) {
          // Exercise the physical SDK runner's minimum-number setup contract.
          System::set_block_number(0);
        }
        System::deposit_log(DigestItem::PreRuntime(*b"old!", vec![8; 3]));
        System::register_extra_weight_unchecked(
          polkadot_sdk::frame_support::weights::Weight::from_parts(9, 7),
          polkadot_sdk::frame_support::dispatch::DispatchClass::Normal,
        );
        let mut recording = h1_storage_trace::BenchmarkRecording::new("authorship_with_initialize");
        cases::native_initialize_instance(c, d, true, &mut recording).unwrap();
        let trace = recording.finish();
        let geometry = prepared_geometry(d);
        assert_eq!(System::block_number(), 2);
        assert_whitelist_coverage(&trace, true);
        let digest = &trace["owners"]["System.Digest"];
        assert_eq!(digest["calls"]["Put"], 1);
        assert_eq!(digest["calls"]["Get"], 1);
        assert_eq!(digest["get_value_bytes"], geometry["digestBytes"]);
        assert_eq!(digest["submitted_value_bytes"], geometry["digestBytes"]);
        for owner in [
          "System.BlockSize",
          "System.ParentHash",
          "System.BlockHash",
          "System.Number",
          "System.ExecutionPhase",
        ] {
          assert_eq!(trace["owners"][owner]["calls"]["Put"], 1, "{owner}");
        }
        for key in authorship::initialization_owner_keys().iter().skip(1) {
          let owner = format!(
            "raw:{}",
            polkadot_sdk::sp_core::hexdisplay::HexDisplay::from(key)
          );
          assert_eq!(trace["owners"][owner]["calls"]["Put"], 1);
        }
        assert_eq!(trace["owners"]["System.BlockWeight"]["calls"]["Delete"], 1);
        assert_eq!(trace["owners"]["System.BlockWeight"]["calls"]["Get"], 2);
        assert_eq!(trace["owners"]["System.BlockWeight"]["calls"]["Put"], 2);
        let cache = &trace["owners"]["Authorship.Author"];
        assert_eq!(cache["calls"]["Get"], 1);
        assert_eq!(cache["calls"]["Put"], 1);
        assert_eq!(cache["calls"]["Delete"], 1);
        assert_eq!(cache["net_changed_keys"], 0);
        println!(
          "AUTHORSHIP_INITIALIZE_NATIVE {}",
          serde_json::json!({"case": c, "geometry": d, "input": geometry, "trace": trace})
        );
      });
    }
  }
}

#[test]
fn authorship_existing_recipient_preserves_complete_owner_signature() {
  for boundary in 0..=2 {
    with_observer_ext(|| {
      let name = match boundary {
        0 => "authorship_complete",
        1 => "authorship_with_initialize",
        _ => "authorship_initialize_root",
      };
      let mut recording = h1_storage_trace::BenchmarkRecording::new(name);
      match boundary {
        0 => cases::native_instance(2, 0, true, &mut recording),
        1 => cases::native_initialize_instance(2, 0, true, &mut recording),
        _ => cases::native_root_instance(2, 0, true, &mut recording),
      }
      .unwrap();
      let trace = recording.finish();
      assert_whitelist_coverage(&trace, boundary != 0);
      let cache = &trace["owners"]["Authorship.Author"];
      assert_eq!(cache["calls"]["Get"], 1, "{name}");
      assert_eq!(cache["calls"]["Put"], 1, "{name}");
      assert_eq!(cache["calls"]["Delete"], 1, "{name}");
      assert_eq!(cache["net_changed_keys"], 0, "{name}");
      assert_eq!(
        trace["owners"]["System.Account"]["absent_reads"], 0,
        "{name}"
      );
      assert_eq!(
        trace["owners"]["System.BlockWeight"]["calls"]["Put"], 2,
        "{name}"
      );
      let author = polkadot_sdk::pallet_session::Validators::<Runtime>::get()
        .pop()
        .unwrap();
      assert!(frame_system::Account::<Runtime>::contains_key(&author));
      assert!(!System::events().iter().any(|record| matches!(
        &record.event,
        crate::RuntimeEvent::System(frame_system::Event::NewAccount { account })
          if account == &author
      )));
      println!(
        "AUTHORSHIP_EXISTING_RECIPIENT {}",
        serde_json::json!({"method": name, "c": 2, "d": 0, "trace": trace})
      );
    });
  }
}

#[test]
fn authorship_complete_callback_proxy_fails_cache_signature() {
  with_observer_ext(|| {
    let fixture = <Runtime as Config>::prepare(1, 0, false).unwrap();
    let author = polkadot_sdk::pallet_session::Validators::<Runtime>::get()
      .pop()
      .unwrap();
    let number = crate::System::block_number();
    let mut recording = h1_storage_trace::BenchmarkRecording::new("callback_proxy");
    recording.start();
    <CollatorSelection as EventHandler<AccountId, u32>>::note_author(author);
    Authorship::on_finalize(number);
    recording.stop();
    // Identical final economic/declaration assertions do not establish cache coverage.
    <Runtime as Config>::verify(fixture);
    let trace = recording.finish();
    let cache = &trace["owners"]["Authorship.Author"];
    assert_eq!(cache["calls"]["Get"], serde_json::Value::Null);
    assert_eq!(cache["calls"]["Put"], serde_json::Value::Null);
    assert_eq!(cache["calls"]["Delete"], 1);
    assert_eq!(trace["owners"]["System.BlockWeight"]["calls"]["Put"], 1);
  });
}
