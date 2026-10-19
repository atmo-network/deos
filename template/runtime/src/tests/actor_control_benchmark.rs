//! Reference bindings and native/caller-bound Wasm SDK conformance, not physical bounds.

use super::common;
use crate::{Actors, AllPalletsWithSystem, Runtime, System};
use polkadot_sdk::{
  frame_benchmarking::{BenchmarkParameter, BenchmarkResult, Benchmarking},
  frame_support::{storage::StoragePrefixedMap, traits::WhitelistedStorageKeys},
  sp_core::storage::Storage,
  sp_runtime::traits::BlakeTwo256,
  sp_state_machine::Backend,
};

#[test]
fn canonical_user_control_callers_match_reference_sdk_host() {
  let cases = [
    (
      b"scheduler_service_user_shared_refusal".as_slice(),
      BenchmarkParameter::c,
      3,
    ),
    (
      b"scheduler_service_user_opening_no_action".as_slice(),
      BenchmarkParameter::r,
      1,
    ),
  ];
  let ordinary = <Actors as Benchmarking>::benchmarks(false);
  let extras = <Actors as Benchmarking>::benchmarks(true);
  let parents = [
    (
      "synthetic-capacity",
      common::benchmarking_storage(common::seeded_synthetic_actor_test_ext()),
    ),
    (
      "development-preset",
      // Reuse the runtime-owned preset builder, without account/policy patches. Empty CODE
      // is a native-call placeholder, not the selected Wasm or a byte-equivalent proof input.
      super::production_block_replay::preset_genesis_storage(
        &[],
        &polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into(),
      ),
    ),
  ];
  for (parent_name, parent) in parents {
    for (method, parameter, maximum) in &cases {
      assert!(ordinary.iter().all(|entry| entry.name != *method));
      let metadata = extras.iter().find(|entry| entry.name == *method).unwrap();
      assert_eq!(metadata.components, vec![(*parameter, 0, *maximum)]);
      for value in 0..=*maximum {
        for verify in [true, false] {
          common::with_benchmarking_host(parent.clone(), |_| {
            assert_eq!(System::block_number(), 0);
            assert_eq!(pallet_deos_actors::ServiceHeader::<Runtime>::get().count, 0);
            let components = [(*parameter, value)];
            let rows = <Actors as Benchmarking>::run_benchmark(
              method,
              &components,
              &AllPalletsWithSystem::whitelisted_storage_keys(),
              verify,
              1,
            )
            .unwrap_or_else(|error| {
              panic!("{parent_name}: {method:?} {components:?} verify={verify}: {error:?}")
            });
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].components, components);
            // The SDK wipes back to this explicit parent. Do not mistake post-wipe state
            // for the successor verified inside the benchmark, or promote timing/proof rows.
            assert_eq!(System::block_number(), 0);
            assert_eq!(pallet_deos_actors::ServiceHeader::<Runtime>::get().count, 0);
          });
        }
      }
    }
  }
}

#[test]
fn mandatory_prepass_control_matches_reference_sdk_and_verification_boundary() {
  check_actor_control_source_sdk(b"mandatory_prepass_control", 2);
}

#[test]
fn mandatory_prepass_entry_control_matches_reference_sdk_and_verification_boundary() {
  check_actor_control_source_sdk(b"mandatory_prepass_entry_control", 1);
}

#[test]
fn mandatory_prepass_deadline_control_matches_reference_sdk_and_verification_boundary() {
  check_actor_control_source_sdk(b"mandatory_prepass_deadline_control", 1);
}

#[test]
fn mandatory_prepass_dependency_control_matches_reference_sdk_and_verification_boundary() {
  check_actor_control_source_sdk(b"mandatory_prepass_dependency_control", 2);
}

#[test]
fn resource_pass_control_matches_reference_sdk_and_verification_boundary() {
  check_actor_control_source_sdk(b"scheduler_resource_pass_control", 3);
}

fn control_source_parents() -> [(&'static str, Storage); 2] {
  [
    (
      "synthetic-capacity",
      common::benchmarking_storage(common::seeded_synthetic_actor_test_ext()),
    ),
    (
      "development-preset",
      super::production_block_replay::preset_genesis_storage(
        &[],
        &polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into(),
      ),
    ),
  ]
}

// Opt-in route: ci-local.sh --only tests --package deos-runtime --features runtime-benchmarks
// --ignored --test-filter tests::actor_control_benchmark::block_resource_idle_finalize_control_matches_reference_sdk_and_verification_boundary
#[cfg(not(feature = "benchmark-authority-scaffolding"))]
#[test]
#[ignore = "separate accepted four-dispatch ordinary SDK gate required; no timing or pricing"]
fn block_resource_idle_finalize_control_matches_reference_sdk_and_verification_boundary() {
  let method = b"block_resource_idle_finalize_control";
  assert!(
    <Actors as Benchmarking>::benchmarks(false)
      .iter()
      .all(|entry| entry.name != method)
  );
  let extras = <Actors as Benchmarking>::benchmarks(true);
  assert!(
    extras
      .iter()
      .find(|entry| entry.name == method)
      .unwrap()
      .components
      .is_empty()
  );
  for (parent_name, parent) in control_source_parents() {
    assert!(parent.children_default.is_empty());
    let mut proofs = Vec::new();
    for verify in [false, true] {
      common::with_benchmarking_host(parent.clone(), |host| {
        let rows = <Actors as Benchmarking>::run_benchmark(
          method,
          &[],
          &AllPalletsWithSystem::whitelisted_storage_keys(),
          verify,
          1,
        )
        .unwrap_or_else(|error| {
          panic!("{parent_name}: literal idle/finalize verify={verify}: {error:?}")
        });
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert!(row.components.is_empty());
        for required in [
          pallet_deos_actors::CurrentBlockResourceState::<Runtime>::hashed_key(),
          pallet_deos_actors::FinalizedBlockResourceTelemetry::<Runtime>::hashed_key(),
        ] {
          assert!(row.keys.iter().any(|(key, _, _, _)| key == &required));
        }
        proofs.push(row.proof_size);
        // SDK wipe is checked through fresh backend pairs, not cached outer Ext reads.
        let restored = host
          .pairs(Default::default())
          .unwrap()
          .map(Result::unwrap)
          .collect::<std::collections::BTreeMap<_, _>>();
        assert!(
          restored == parent.top,
          "{parent_name}: SDK wipe restores complete parent TOP"
        );
      });
    }
    assert_eq!(
      proofs[0], proofs[1],
      "verification is outside the SDK saved body proof"
    );
  }
}

// Opt-in: ci-local.sh --only tests --package deos-runtime --features runtime-benchmarks
// --ignored --test-filter tests::actor_control_benchmark::reference_idle_starvation_events_follow_halt_and_recovery
#[cfg(not(feature = "benchmark-authority-scaffolding"))]
#[test]
#[ignore = "source acceptance and separate native execution approval required"]
fn reference_idle_starvation_events_follow_halt_and_recovery() {
  use pallet_deos_actors::{
    BenchmarkHelper, BlockResourcePhase, Config, CurrentBlockResourceState, Event,
    FinalizedBlockResourceTelemetry, GlobalCircuitBreaker, IdleStarvationPhase,
    IdleStarvationState, WeightInfo,
  };
  use polkadot_sdk::{
    frame_support::{assert_ok, traits::Hooks, weights::Weight},
    frame_system::Phase,
  };
  type Clock = <Runtime as Config>::BenchmarkHelper;
  type W = <Runtime as Config>::WeightInfo;

  assert_eq!(<Runtime as Config>::MaxIdleStarvationBlocks::get(), 25);
  let tick_millis = <Runtime as Config>::CadenceTickMillis::get();
  assert_eq!(crate::SLOT_DURATION, 6_000);
  assert_eq!(tick_millis, 500);
  let tick_step = crate::SLOT_DURATION.checked_div(tick_millis).unwrap();
  assert!(tick_step > 0);
  assert_eq!(
    tick_step.checked_mul(tick_millis).unwrap(),
    crate::SLOT_DURATION
  );
  let reserve = <Runtime as Config>::ActorOnIdleReserve::get();
  assert!(reserve.all_lt(Weight::MAX));
  let housekeeping = W::scheduler_on_idle_base()
    .checked_add(&W::block_resource_finalize())
    .unwrap();
  assert!(housekeeping.ref_time() > 0 && housekeeping.proof_size() > 0);
  assert!(housekeeping.all_lte(reserve));

  for (parent_name, parent) in control_source_parents() {
    polkadot_sdk::sp_io::TestExternalities::new(parent).execute_with(|| {
      assert!(
        !CurrentBlockResourceState::<Runtime>::exists(),
        "{parent_name}"
      );
      assert!(!GlobalCircuitBreaker::<Runtime>::get(), "{parent_name}");
      assert!(!IdleStarvationState::<Runtime>::exists(), "{parent_name}");
      assert_eq!(
        IdleStarvationState::<Runtime>::get(),
        IdleStarvationPhase::Healthy
      );
      let initial_moment = crate::Timestamp::get();
      assert_eq!(initial_moment % tick_millis, 0);
      let mut tick = initial_moment.checked_div(tick_millis).unwrap();
      for observation in 1..=27u32 {
        let halt_only = observation <= 26;
        let next_tick = tick.checked_add(tick_step).unwrap();
        assert!(next_tick > tick);
        assert_ok!(Clock::advance_to_scheduler_tick(next_tick));
        tick = next_tick;
        assert_ok!(Clock::prepare_actor_prepass_context());
        let now = System::block_number();
        if halt_only {
          assert_ok!(Actors::set_global_circuit_breaker(
            crate::RuntimeOrigin::root(),
            true
          ));
        }
        assert_ok!(Actors::actor_prepass(crate::RuntimeOrigin::none()));
        let prepass = CurrentBlockResourceState::<Runtime>::get().unwrap();
        assert_eq!(prepass.block_number(), now);
        assert_eq!(prepass.phase(), BlockResourcePhase::ExternalPhase);
        assert_eq!(prepass.optional_actor_work_halted(), halt_only);
        System::note_finished_initialize();
        if halt_only {
          assert_ok!(Actors::set_global_circuit_breaker(
            crate::RuntimeOrigin::root(),
            false
          ));
        }
        assert!(!GlobalCircuitBreaker::<Runtime>::get());
        let baseline = CurrentBlockResourceState::<Runtime>::get().unwrap();
        assert_eq!(baseline.phase(), BlockResourcePhase::ExternalPhase);
        assert_eq!(baseline.optional_actor_work_halted(), halt_only);
        let remaining_control = baseline
          .budget()
          .unwrap()
          .limits()
          .actor_control()
          .checked_sub(&baseline.usage().actor_control_used())
          .unwrap();
        assert!(housekeeping.all_lte(remaining_control));
        System::note_finished_extrinsics();
        assert_eq!(System::execution_phase(), Some(Phase::Finalization));
        // System initialization retains inherited and earlier local-block events.
        let prefix = System::events();
        let prefix_count = System::event_count();
        assert_eq!(usize::try_from(prefix_count).unwrap(), prefix.len());
        let returned = <Actors as Hooks<crate::BlockNumber>>::on_idle(now, reserve);
        <Actors as Hooks<crate::BlockNumber>>::on_finalize(now);
        assert_ok!(Clock::finalize_scheduler_clock());
        assert!(!CurrentBlockResourceState::<Runtime>::exists());
        let telemetry = FinalizedBlockResourceTelemetry::<Runtime>::get().unwrap();
        assert_eq!(telemetry.block_number(), now);
        assert_eq!(telemetry.optional_actor_work_halted(), halt_only);
        let control_delta = telemetry
          .usage()
          .actor_control_used()
          .checked_sub(&baseline.usage().actor_control_used())
          .unwrap();
        let effect_delta = telemetry
          .usage()
          .actor_effect_used()
          .checked_sub(&baseline.usage().actor_effect_used())
          .unwrap();
        assert_eq!(
          telemetry.usage().user_dispatch_used(),
          baseline.usage().user_dispatch_used()
        );
        assert_eq!(control_delta.checked_add(&effect_delta).unwrap(), returned);
        if halt_only {
          assert_eq!(returned, housekeeping);
          assert_eq!(control_delta, housekeeping);
          assert_eq!(effect_delta, Weight::zero());
          let expected = if observation < 25 {
            IdleStarvationPhase::Starving {
              consecutive_blocks: observation,
            }
          } else {
            IdleStarvationPhase::Alerted {
              consecutive_blocks: observation,
            }
          };
          assert_eq!(IdleStarvationState::<Runtime>::get(), expected);
        } else {
          assert!(
            !IdleStarvationState::<Runtime>::exists(),
            "{parent_name}: recovery required"
          );
        }
        let events = System::events();
        assert_eq!(&events[..prefix.len()], prefix.as_slice());
        let suffix = &events[prefix.len()..];
        assert_eq!(
          System::event_count(),
          prefix_count
            .checked_add(u32::try_from(suffix.len()).unwrap())
            .unwrap()
        );
        let expected_event = match observation {
          25 => Some(Event::<Runtime>::IdleStarvationDetected {
            consecutive_blocks: 25,
          }),
          27 => Some(Event::<Runtime>::IdleStarvationRecovered {
            consecutive_blocks: 26,
          }),
          _ => None,
        };
        let starvation_events = suffix
          .iter()
          .filter(|record| {
            matches!(
              record.event,
              crate::RuntimeEvent::Actors(
                Event::IdleStarvationDetected { .. } | Event::IdleStarvationRecovered { .. }
              )
            )
          })
          .collect::<Vec<_>>();
        assert_eq!(
          starvation_events.len(),
          usize::from(expected_event.is_some())
        );
        if let Some(expected) = expected_event {
          let record = starvation_events[0];
          assert_eq!(record.event, crate::RuntimeEvent::Actors(expected));
          assert_eq!(record.phase, Phase::Finalization);
          assert!(record.topics.is_empty());
        }
        if halt_only {
          assert_eq!(suffix.len(), starvation_events.len());
        }
      }
    });
  }
}

#[cfg(not(feature = "benchmark-authority-scaffolding"))]
#[test]
#[ignore = "source acceptance and separate native execution approval required"]
fn idle_event_live_prefix_original_inputs_replay_delta() {
  use codec::Encode;
  use pallet_deos_actors::{
    BenchmarkHelper, BlockResourceBudgetProvider, BlockResourcePhase, Config,
    CurrentBlockResourceState, Event, FinalizedBlockResourceTelemetry, GlobalCircuitBreaker,
    IdleStarvationPhase, IdleStarvationState, WeightInfo,
  };
  use polkadot_sdk::{
    frame_benchmarking::benchmarking,
    frame_support::{assert_ok, traits::Hooks, weights::Weight},
    frame_system::Phase,
    sp_core::{
      Blake2Hasher, H256, Hasher,
      storage::{TrackedStorageKey, well_known_keys},
    },
    sp_trie::{
      EMPTY_PREFIX, HashDBT, LayoutV1, MemoryDB, StorageProof, Trie, TrieConfiguration,
      TrieDBBuilder, delta_trie_root,
    },
  };
  use std::collections::{BTreeMap, BTreeSet};
  type Clock = <Runtime as Config>::BenchmarkHelper;
  type W = <Runtime as Config>::WeightInfo;
  type Layout = LayoutV1<Blake2Hasher>;

  fn live_top() -> BTreeMap<Vec<u8>, Vec<u8>> {
    let mut top = BTreeMap::new();
    let mut key = Vec::new();
    if let Some(value) = polkadot_sdk::sp_io::storage::get(&key) {
      top.insert(key.clone(), value.to_vec());
    }
    while let Some(next) = polkadot_sdk::sp_io::storage::next_key(&key) {
      assert!(next > key);
      let value = polkadot_sdk::sp_io::storage::get(&next).expect("enumerated live key");
      top.insert(next.clone(), value.to_vec());
      key = next;
    }
    assert!(
      !top
        .keys()
        .any(|key| well_known_keys::is_child_storage_key(key))
    );
    top
  }

  fn prepare(
    tick_step: u64,
    housekeeping: Weight,
  ) -> pallet_deos_actors::BlockResourceState<crate::BlockNumber> {
    System::reset_events();
    let tick = crate::Timestamp::get() / <Runtime as Config>::CadenceTickMillis::get();
    assert_ok!(Clock::advance_to_scheduler_tick(
      tick.checked_add(tick_step).unwrap()
    ));
    assert_ok!(Clock::prepare_actor_prepass_context());
    assert_ok!(Actors::set_global_circuit_breaker(
      crate::RuntimeOrigin::root(),
      true
    ));
    assert_ok!(Actors::actor_prepass(crate::RuntimeOrigin::none()));
    System::note_finished_initialize();
    assert_ok!(Actors::set_global_circuit_breaker(
      crate::RuntimeOrigin::root(),
      false
    ));
    let state = CurrentBlockResourceState::<Runtime>::get().unwrap();
    assert_eq!(state.block_number(), System::block_number());
    assert_eq!(state.phase(), BlockResourcePhase::ExternalPhase);
    assert!(state.optional_actor_work_halted());
    assert!(!GlobalCircuitBreaker::<Runtime>::get());
    let configured = <Runtime as Config>::BlockResourceBudget::get();
    assert_eq!(
      <Runtime as Config>::BlockResourceBudget::for_prepass(System::block_number(), configured),
      Ok(configured)
    );
    assert!(
      housekeeping.all_lte(
        state
          .budget()
          .unwrap()
          .limits()
          .actor_control()
          .checked_sub(&state.usage().actor_control_used())
          .unwrap()
      )
    );
    System::note_finished_extrinsics();
    assert_eq!(System::execution_phase(), Some(Phase::Finalization));
    state
  }

  assert_eq!(<Runtime as Config>::MaxIdleStarvationBlocks::get(), 25);
  let tick_millis = <Runtime as Config>::CadenceTickMillis::get();
  assert_eq!(tick_millis, 500);
  assert_eq!(crate::SLOT_DURATION, 6_000);
  let tick_step = crate::SLOT_DURATION.checked_div(tick_millis).unwrap();
  let reserve = <Runtime as Config>::ActorOnIdleReserve::get();
  let housekeeping = W::scheduler_on_idle_base()
    .checked_add(&W::block_resource_finalize())
    .unwrap();
  assert!(housekeeping.ref_time() > 0 && housekeeping.proof_size() > 0);
  assert!(housekeeping.all_lte(reserve));
  let parent = super::production_block_replay::preset_genesis_storage(
    &[],
    &polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into(),
  );
  assert!(parent.children_default.is_empty());
  assert_eq!(parent.top.get(well_known_keys::CODE), Some(&Vec::new()));
  let mut fixture = polkadot_sdk::sp_io::TestExternalities::new(parent);
  for observation in 1..=24 {
    fixture.execute_with(|| {
      assert!(!CurrentBlockResourceState::<Runtime>::exists());
      assert!(!GlobalCircuitBreaker::<Runtime>::get());
      prepare(tick_step, housekeeping);
      let now = System::block_number();
      let returned = <Actors as Hooks<crate::BlockNumber>>::on_idle(now, reserve);
      <Actors as Hooks<crate::BlockNumber>>::on_finalize(now);
      assert_eq!(returned, housekeeping);
      assert_ok!(Clock::finalize_scheduler_clock());
      assert!(!CurrentBlockResourceState::<Runtime>::exists());
      assert_eq!(
        IdleStarvationState::<Runtime>::get(),
        IdleStarvationPhase::Starving {
          consecutive_blocks: observation,
        }
      );
    });
    fixture.commit_all().unwrap();
  }
  let source = common::committed_fixture_storage(fixture);
  assert!(source.children_default.is_empty());
  let mut original = MemoryDB::<Blake2Hasher>::default();
  let root = Layout::trie_build(&mut original, source.top.clone());
  assert_eq!(root, Layout::trie_root(source.top.clone()));
  let starvation_key = IdleStarvationState::<Runtime>::hashed_key();
  let starving = IdleStarvationPhase::Starving {
    consecutive_blocks: 24,
  }
  .encode();
  assert_eq!(source.top.get(starvation_key.as_slice()), Some(&starving));

  let (baseline, body, full, post_root, top) =
    common::with_benchmarking_host(source.clone(), |host| {
      benchmarking::set_whitelist(Vec::<TrackedStorageKey>::new());
      let recorder = host.recorder().expect("recording host required");
      let baseline = recorder.to_storage_proof();
      let before = prepare(tick_step, housekeeping);
      let now = System::block_number();
      assert_eq!(now, 25);
      let returned = <Actors as Hooks<crate::BlockNumber>>::on_idle(now, reserve);
      <Actors as Hooks<crate::BlockNumber>>::on_finalize(now);
      let body = recorder.to_storage_proof();
      let post_root = H256::from_slice(&polkadot_sdk::sp_io::storage::root(
        crate::VERSION.state_version(),
      ));
      let full = recorder.to_storage_proof();
      // All enumeration and semantic observers follow both retained snapshots.
      let top = live_top();
      assert_eq!(returned, housekeeping);
      assert!(!CurrentBlockResourceState::<Runtime>::exists());
      assert!(!GlobalCircuitBreaker::<Runtime>::get());
      assert_eq!(System::execution_phase(), Some(Phase::Finalization));
      assert_eq!(
        IdleStarvationState::<Runtime>::get(),
        IdleStarvationPhase::Alerted {
          consecutive_blocks: 25,
        }
      );
      let telemetry = FinalizedBlockResourceTelemetry::<Runtime>::get().unwrap();
      assert_eq!(telemetry.block_number(), now);
      assert!(telemetry.optional_actor_work_halted());
      assert_eq!(
        telemetry
          .usage()
          .actor_control_used()
          .checked_sub(&before.usage().actor_control_used())
          .unwrap(),
        housekeeping
      );
      assert_eq!(
        telemetry.usage().actor_effect_used(),
        before.usage().actor_effect_used()
      );
      assert_eq!(
        telemetry.usage().user_dispatch_used(),
        before.usage().user_dispatch_used()
      );
      let events = System::events();
      let (detection, prefix) = events.split_last().expect("detection suffix required");
      assert_eq!(
        detection.event,
        crate::RuntimeEvent::Actors(Event::<Runtime>::IdleStarvationDetected {
          consecutive_blocks: 25,
        })
      );
      assert_eq!(detection.phase, Phase::Finalization);
      assert!(detection.topics.is_empty());
      assert_eq!(
        usize::try_from(System::event_count()).unwrap(),
        prefix.len().checked_add(1).unwrap()
      );
      // Retain every actual prefix record in TOP/delta; assert only the public breaker anchors.
      let mut breakers = [
        (true, Phase::Initialization),
        (false, Phase::ApplyExtrinsic(0)),
      ]
      .into_iter();
      for record in prefix {
        assert!(matches!(
          record.phase,
          Phase::Initialization | Phase::ApplyExtrinsic(0)
        ));
        if let crate::RuntimeEvent::Actors(Event::GlobalCircuitBreakerSet { paused }) =
          &record.event
        {
          let (expected_paused, expected_phase) =
            breakers.next().expect("unexpected breaker producer");
          assert_eq!(*paused, expected_paused);
          assert_eq!(record.phase, expected_phase);
          assert!(record.topics.is_empty());
        }
      }
      assert!(breakers.next().is_none(), "public breaker prefix required");
      assert_ok!(Clock::finalize_scheduler_clock());
      (baseline, body, full, post_root, top)
    });
  let mut post_db = MemoryDB::<Blake2Hasher>::default();
  assert_eq!(Layout::trie_build(&mut post_db, top.clone()), post_root);
  assert_eq!(Layout::trie_root(top.clone()), post_root);
  let original_nodes = |proof: &StorageProof| {
    proof
      .iter_nodes()
      .filter(|node| {
        HashDBT::get(&original, &Blake2Hasher::hash(node), EMPTY_PREFIX).as_ref() == Some(*node)
      })
      .cloned()
      .collect::<BTreeSet<_>>()
  };
  let body_inputs = original_nodes(&body);
  let root_inputs = original_nodes(&full);
  assert!(
    baseline
      .iter_nodes()
      .all(|node| body.iter_nodes().any(|entry| entry == node))
  );
  assert!(
    body
      .iter_nodes()
      .all(|node| full.iter_nodes().any(|entry| entry == node))
  );
  let root_added_original = root_inputs
    .difference(&body_inputs)
    .cloned()
    .collect::<BTreeSet<_>>();
  let computed_only = full
    .iter_nodes()
    .filter(|node| !root_inputs.contains(*node))
    .cloned()
    .collect::<BTreeSet<_>>();
  assert!(root_added_original.is_disjoint(&computed_only));
  let body_db = StorageProof::new(body_inputs).into_memory_db::<Blake2Hasher>();
  assert_eq!(
    TrieDBBuilder::<Layout>::new(&body_db, &root)
      .build()
      .get(&starvation_key)
      .unwrap(),
    Some(starving)
  );
  let delta = source
    .top
    .keys()
    .chain(top.keys())
    .collect::<BTreeSet<_>>()
    .into_iter()
    .filter(|key| source.top.get(*key) != top.get(*key))
    .map(|key| (key.clone(), top.get(key).cloned()))
    .collect::<Vec<_>>();
  let mut replay = StorageProof::new(root_inputs).into_memory_db::<Blake2Hasher>();
  assert_eq!(
    delta_trie_root::<Layout, _, _, _, _, _>(&mut replay, root, delta, None, None,).unwrap(),
    post_root
  );
  // Named original read plus supplied complete net delta, not proof-only execution.
}

fn check_actor_control_source_sdk(method: &[u8], maximum: u32) {
  check_actor_control_source_sdk_with(method, maximum, 0..=maximum, |_, c, verify| {
    <Actors as Benchmarking>::run_benchmark(
      method,
      &[(BenchmarkParameter::c, c)],
      &AllPalletsWithSystem::whitelisted_storage_keys(),
      verify,
      1,
    )
    .unwrap_or_else(|error| panic!("native {method:?}: c={c} verify={verify}: {error:?}"))
  });
}

fn check_actor_control_source_sdk_with(
  method: &[u8],
  maximum: u32,
  cells: std::ops::RangeInclusive<u32>,
  mut run: impl FnMut(&sc_client_db::BenchmarkingState<BlakeTwo256>, u32, bool) -> Vec<BenchmarkResult>,
) {
  assert!(
    <Actors as Benchmarking>::benchmarks(false)
      .iter()
      .all(|entry| entry.name != method)
  );
  let extras = <Actors as Benchmarking>::benchmarks(true);
  assert_eq!(
    extras
      .iter()
      .find(|entry| entry.name == method)
      .unwrap()
      .components,
    vec![(BenchmarkParameter::c, 0, maximum)]
  );
  for (parent_name, parent) in control_source_parents() {
    assert!(parent.children_default.is_empty());
    for c in cells.clone() {
      let mut observed = Vec::new();
      for verify in [false, true] {
        common::with_benchmarking_host(parent.clone(), |host| {
          assert_eq!(System::block_number(), 0);
          let rows = run(host, c, verify);
          assert_eq!(rows.len(), 1);
          let row = &rows[0];
          assert_eq!(row.components, vec![(BenchmarkParameter::c, c)]);
          let legacy = [
            pallet_deos_actors::ActorReadyTail::<Runtime>::hashed_key(),
            pallet_deos_actors::PrepassExecutionCutoff::<Runtime>::hashed_key(),
          ];
          for key in legacy {
            assert_eq!(
              row.keys.iter().any(|(read, _, _, _)| read == &key),
              verify,
              "legacy reads are verification only, never the recorded source"
            );
          }
          // SDK key metadata aggregates storage prefixes, not full double-map keys.
          let required_key = if method == b"mandatory_prepass_deadline_control" {
            pallet_deos_actors::DeadlineIndexPages::<Runtime>::final_prefix().to_vec()
          } else if method == b"mandatory_prepass_dependency_control" {
            pallet_deos_actors::DependencyScanSourceListState::<Runtime>::hashed_key().to_vec()
          } else if method == b"scheduler_resource_pass_control" {
            pallet_deos_actors::ServiceHeader::<Runtime>::hashed_key().to_vec()
          } else {
            pallet_deos_actors::CurrentBlockResourceState::<Runtime>::hashed_key().to_vec()
          };
          let zero_input = method == b"scheduler_resource_pass_control" && c == 1;
          assert_eq!(
            row.keys.iter().any(|(key, _, _, _)| key == &required_key),
            !zero_input || verify,
            "zero-input pass has no recorded selector access; its verifier does"
          );
          observed.push((
            row.proof_size,
            u64::from(row.reads) + u64::from(row.repeat_reads),
          ));
          // A nested Wasm StateMachine does not clear the outer Ext's cached reads.
          let restored = host
            .pairs(Default::default())
            .unwrap()
            .map(Result::unwrap)
            .collect::<std::collections::BTreeMap<_, _>>();
          assert!(
            restored == parent.top,
            "{parent_name}: SDK wipe restores the complete parent"
          );
        });
      }
      assert_eq!(
        observed[0].0, observed[1].0,
        "verification is outside the saved body proof"
      );
      assert!(
        observed[1].1 > observed[0].1,
        "the positive verifier performs real reads"
      );
    }
  }
}

#[cfg(not(feature = "benchmark-authority-scaffolding"))]
#[test]
#[ignore = "one opt-in native matched Deadline witness pair; source review required, no timing or pricing"]
fn prepass_deadline_native_prefix_inputs_replay_delta() {
  use codec::Encode;
  use pallet_deos_actors::{AssetOps, BlockResourceBudgetProvider, Config, Task, WakeupKey};
  use polkadot_sdk::{
    frame_benchmarking::benchmarking,
    sp_core::{
      Blake2Hasher, H256, Hasher,
      storage::{TrackedStorageKey, well_known_keys},
    },
    sp_trie::{
      EMPTY_PREFIX, HashDBT, LayoutV1, MemoryDB, StorageProof, Trie, TrieConfiguration,
      TrieDBBuilder, delta_trie_root,
    },
  };
  use std::collections::{BTreeMap, BTreeSet};
  type Layout = LayoutV1<Blake2Hasher>;
  type Ops = <Runtime as Config>::AssetOps;

  fn assert_ordinary_budget() {
    let configured = <Runtime as Config>::BlockResourceBudget::get();
    assert_eq!(
      <Runtime as Config>::BlockResourceBudget::for_prepass(System::block_number(), configured),
      Ok(configured),
      "no thread-local isolated-budget authority"
    );
  }

  // Ext's live iterator merges insertions/deletions with the backend. Backend::pairs alone does not.
  fn live_top() -> BTreeMap<Vec<u8>, Vec<u8>> {
    let mut top = BTreeMap::new();
    let mut key = Vec::new();
    if let Some(value) = polkadot_sdk::sp_io::storage::get(&key) {
      top.insert(key.clone(), value.to_vec());
    }
    while let Some(next) = polkadot_sdk::sp_io::storage::next_key(&key) {
      assert!(next > key);
      let value = polkadot_sdk::sp_io::storage::get(&next).expect("enumerated live key");
      top.insert(next.clone(), value.to_vec());
      key = next;
    }
    assert!(
      !top
        .keys()
        .any(|key| well_known_keys::is_child_storage_key(key))
    );
    top
  }

  fn selected_events(actor: pallet_deos_actors::ActorId) -> Vec<Vec<u8>> {
    System::events()
      .into_iter()
      .filter_map(|record| {
        let crate::RuntimeEvent::Actors(event) = record.event else {
          return None;
        };
        let selected = match &event {
          pallet_deos_actors::Event::TriggerOccurrenceProcessed { actor_id, .. }
          | pallet_deos_actors::Event::PipelineFeeCharged { actor_id, .. }
          | pallet_deos_actors::Event::ActionFeeCharged { actor_id, .. }
          | pallet_deos_actors::Event::TransferExecuted { actor_id, .. }
          | pallet_deos_actors::Event::StepFailed { actor_id, .. } => *actor_id == actor,
          _ => false,
        };
        selected.then(|| event.encode())
      })
      .collect()
  }

  for variable in ["RUST_LOG", "RUNTIME_LOG"] {
    assert!(
      std::env::var_os(variable).is_none(),
      "{variable} must be absent"
    );
  }
  let parent = super::production_block_replay::preset_genesis_storage(
    &[],
    &polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into(),
  );
  assert!(parent.children_default.is_empty());
  assert_eq!(parent.top.get(well_known_keys::CODE), Some(&Vec::new()));
  // Reproduce the selected SDK v1 additions before setup; retain actual setup-added flags/entries.
  let (source, root, whitelist, actor, retry_key, peers, execute, verify, custody) =
    common::with_benchmarking_host(parent, |host| {
      let mut whitelist = AllPalletsWithSystem::whitelisted_storage_keys();
      whitelist.push(
        polkadot_sdk::frame_system::Account::<Runtime>::hashed_key_for(
          polkadot_sdk::frame_benchmarking::whitelisted_caller::<crate::AccountId>(),
        )
        .into(),
      );
      for key in [
        polkadot_sdk::frame_support::storage::transactional::TRANSACTION_LEVEL_KEY,
        well_known_keys::EXTRINSIC_INDEX,
        well_known_keys::INTRABLOCK_ENTROPY,
      ] {
        whitelist.push(TrackedStorageKey::new(key.to_vec()));
      }
      benchmarking::set_whitelist(whitelist);
      let (actor, execute, verify) = Actors::prepare_prepass_deadline_witness().unwrap();
      assert_ordinary_budget();
      let identity = Actors::actor_identity(actor).unwrap();
      let contract = Actors::actor_contract(actor).unwrap();
      let Task::Transfer { to, asset, .. } = &contract.steps[0].task else {
        panic!("selected retry must remain the producer's Transfer");
      };
      assert_ne!(*asset, <Runtime as Config>::FeeNativeAssetId::get());
      let accounts = [
        identity.sovereign_account.clone(),
        identity.owner.clone(),
        to.clone(),
      ];
      let asset = *asset;
      let native = <Runtime as Config>::FeeNativeAssetId::get();
      let snapshot = move || {
        accounts
          .iter()
          .map(|account| {
            (
              System::account(account).encode(),
              polkadot_sdk::pallet_balances::Holds::<Runtime>::get(account).encode(),
              polkadot_sdk::pallet_balances::Freezes::<Runtime>::get(account).encode(),
              polkadot_sdk::pallet_balances::Locks::<Runtime>::get(account).encode(),
              Ops::balance(account, native),
              Ops::total_balance(account, native),
              Ops::balance(account, asset),
              Ops::total_balance(account, asset),
            )
          })
          .collect::<Vec<_>>()
      };
      let balances = snapshot();
      let events = selected_events(actor);
      let custody = move || {
        assert_eq!(Actors::actor_identity(actor), Some(identity.clone()));
        assert_eq!(
          snapshot(),
          balances,
          "selected native fee custody and Transfer custody unchanged"
        );
        assert_eq!(
          selected_events(actor),
          events,
          "no selected fee or Action invocation"
        );
      };
      let handle = pallet_deos_actors::DeadlineHandles::<Runtime>::get(actor).unwrap();
      let retry_page =
        pallet_deos_actors::DeadlinePages::<Runtime>::get(handle.key, handle.page).unwrap();
      assert!(retry_page.entries.contains(&Some(handle.actor)));
      let retry_key =
        pallet_deos_actors::DeadlinePages::<Runtime>::hashed_key_for(handle.key, handle.page);
      let now_tick = crate::Timestamp::get() / <Runtime as Config>::CadenceTickMillis::get();
      let peers = pallet_deos_actors::DeadlinePages::<Runtime>::iter()
        .filter(|(key, _, _)| matches!(key, WakeupKey::Tick(tick) if *tick <= now_tick))
        .flat_map(|(_, _, page)| {
          page
            .entries
            .into_iter()
            .flatten()
            .map(|entry| entry.actor_id)
        })
        .collect::<BTreeSet<_>>();
      let whitelist = benchmarking::get_whitelist();
      assert!(whitelist.iter().all(|key| key.child_trie_key.is_none()));
      let root = H256::from_slice(&polkadot_sdk::sp_io::storage::root(
        crate::VERSION.state_version(),
      ));
      benchmarking::commit_db();
      let top = host
        .pairs(Default::default())
        .unwrap()
        .map(Result::unwrap)
        .collect();
      let source = Storage {
        top,
        ..Default::default()
      };
      assert!(source.children_default.is_empty());
      assert_eq!(source.top.get(well_known_keys::CODE), Some(&Vec::new()));
      (
        source, root, whitelist, actor, retry_key, peers, execute, verify, custody,
      )
    });
  let mut original = MemoryDB::<Blake2Hasher>::default();
  assert_eq!(Layout::trie_build(&mut original, source.top.clone()), root);
  assert_eq!(Layout::trie_root(source.top.clone()), root);
  let retry_value = source
    .top
    .get(&retry_key)
    .expect("original selected retry page");
  // The page payload, not the unaccessed paid Run, is the selected original retry read dependency.

  let mut observations = Vec::new();
  for parent_cut in [true, false] {
    let observed = common::with_benchmarking_host(source.clone(), |host| {
      benchmarking::set_whitelist(whitelist.clone());
      let recorder = host.recorder().expect("existing SDK recorder enabled");
      let mut baseline = None;
      let mut prefix = None;
      let mut ready = || {
        assert_ordinary_budget();
        assert!(pallet_deos_actors::CurrentBlockResourceState::<Runtime>::get().is_none());
        assert_eq!(
          live_top(),
          source.top,
          "entry neither mutates nor publishes S"
        );
        assert_eq!(
          H256::from_slice(&polkadot_sdk::sp_io::storage::root(
            crate::VERSION.state_version()
          )),
          root
        );
        benchmarking::commit_db();
        assert!(recorder.to_storage_proof().is_empty());
        assert_eq!(benchmarking::get_whitelist(), whitelist);
        for key in &whitelist {
          polkadot_sdk::sp_io::storage::get(&key.key);
        }
        baseline = Some((
          benchmarking::proof_size().unwrap(),
          recorder.to_storage_proof(),
        ));
        benchmarking::reset_read_write_count();
        assert_eq!(benchmarking::read_write_count(), (0, 0, 0, 0));
      };
      let mut save_prefix = || {
        // Retained recorder snapshot only: no storage query, commit or compact-proof host call.
        prefix = Some(recorder.to_storage_proof());
      };
      let settlement = execute(parent_cut, &mut ready, &mut save_prefix)
        .expect("Err stops this branch; no local-ledger reuse after storage rollback");
      let body_size = benchmarking::proof_size().unwrap();
      let body = recorder.to_storage_proof();
      let stop_counts = benchmarking::read_write_count();
      let post_root = H256::from_slice(&polkadot_sdk::sp_io::storage::root(
        crate::VERSION.state_version(),
      ));
      let full = recorder.to_storage_proof();
      // Root snapshot is final before any overlay enumeration or semantic oracle.
      let top = live_top();
      verify();
      custody();
      assert!(pallet_deos_actors::CurrentBlockResourceState::<Runtime>::get().is_none());
      let peer_outcomes = peers
        .iter()
        .map(|peer| {
          (
            *peer,
            pallet_deos_actors::ActorProcesses::<Runtime>::get(peer).encode(),
            pallet_deos_actors::ActorRunStateStore::<Runtime>::get(peer).encode(),
            pallet_deos_actors::ActorSemanticStates::<Runtime>::get(peer).encode(),
            pallet_deos_actors::TriggerDeadlineHandles::<Runtime>::get(peer).encode(),
            pallet_deos_actors::ServiceNodes::<Runtime>::get(peer).encode(),
          )
        })
        .collect::<Vec<_>>();
      let node = pallet_deos_actors::ServiceNodes::<Runtime>::get(actor).unwrap();
      assert_eq!(
        node.eligible_from,
        System::block_number().checked_add(1).unwrap()
      );
      assert!(!pallet_deos_actors::DeadlineHandles::<Runtime>::contains_key(actor));
      let ring = pallet_deos_actors::ServiceHeader::<Runtime>::get();
      let start = ring.cursor.expect("returned retry makes Service nonempty");
      let mut current = start;
      let mut visited = BTreeSet::new();
      let mut selected_returns = 0;
      for _ in 0..ring.count {
        assert!(
          visited.insert(current.actor_id),
          "one traversal, no duplicate member"
        );
        let member = pallet_deos_actors::ServiceNodes::<Runtime>::get(current.actor_id).unwrap();
        assert_eq!(member.generation, current.generation);
        selected_returns += u32::from(current.actor_id == actor);
        current = member.next;
      }
      assert_eq!(current, start);
      assert_eq!(selected_returns, 1, "exactly one canonical B+1 return");
      (
        baseline.unwrap(),
        prefix,
        body_size,
        body,
        stop_counts,
        post_root,
        full,
        top,
        settlement,
        peer_outcomes,
      )
    });
    observations.push(observed);
  }
  assert_eq!(observations.len(), 2, "exactly one matched pair; no retry");
  let a = &observations[0];
  let b = &observations[1];
  assert!(a.1.is_some() && b.1.is_none());
  assert_eq!(a.0, b.0, "same W-prefilled baseline on original R");
  assert_eq!(a.5, b.5, "post roots");
  assert_eq!(
    a.7, b.7,
    "complete live TOP, including actual peers and fee sink"
  );
  assert_eq!(
    a.8, b.8,
    "both-axis local settlement and actual Deadline Weight"
  );
  assert_eq!(
    a.9, b.9,
    "actual due Tick peers, not an invented empty clock"
  );
  let mixed_delta = |top: &BTreeMap<Vec<u8>, Vec<u8>>| {
    source
      .top
      .keys()
      .chain(top.keys())
      .collect::<BTreeSet<_>>()
      .into_iter()
      .filter(|key| source.top.get(*key) != top.get(*key))
      .map(|key| (key.clone(), top.get(key).cloned()))
      .collect::<Vec<_>>()
  };
  assert_eq!(
    mixed_delta(&a.7),
    mixed_delta(&b.7),
    "complete insert/update/delete delta"
  );

  for (parent_cut, observation) in [true, false].into_iter().zip(observations) {
    let (baseline, prefix, body_size, body, stop_counts, post_root, full, top, _, peer_outcomes) =
      observation;
    assert_eq!(Layout::trie_root(top.clone()), post_root);
    let mut post_db = MemoryDB::<Blake2Hasher>::default();
    assert_eq!(Layout::trie_build(&mut post_db, top.clone()), post_root);
    let delta = mixed_delta(&top);
    let original_nodes = |proof: &StorageProof| {
      proof
        .iter_nodes()
        .filter(|node| {
          HashDBT::get(&original, &Blake2Hasher::hash(node), EMPTY_PREFIX).as_ref() == Some(*node)
        })
        .cloned()
        .collect::<BTreeSet<_>>()
    };
    let baseline_inputs = original_nodes(&baseline.1);
    let prefix_inputs = prefix.as_ref().map(&original_nodes);
    let body_inputs = original_nodes(&body);
    let full_inputs = original_nodes(&full);
    let body_nodes = body.iter_nodes().collect::<BTreeSet<_>>();
    assert!(
      baseline
        .1
        .iter_nodes()
        .all(|node| body_nodes.contains(node))
    );
    if let Some(prefix) = &prefix {
      assert!(prefix.iter_nodes().all(|node| body_nodes.contains(node)));
    }
    assert!(
      body
        .iter_nodes()
        .all(|node| full.iter_nodes().any(|full_node| full_node == node))
    );
    let root_added_original = full_inputs
      .difference(&body_inputs)
      .cloned()
      .collect::<BTreeSet<_>>();
    let computed_outputs = full
      .iter_nodes()
      .filter(|node| !full_inputs.contains(*node))
      .count();
    let mut body_db =
      StorageProof::new(body_inputs.iter().cloned()).into_memory_db::<Blake2Hasher>();
    let body_only_sufficient =
      delta_trie_root::<Layout, _, _, _, _, _>(&mut body_db, root, delta.clone(), None, None)
        .is_ok_and(|replayed| replayed == post_root);
    let witness = StorageProof::new(full_inputs.iter().cloned());
    let compact = witness.to_compact_proof::<Blake2Hasher>(root).unwrap();
    let (mut replay, replay_root) = compact.to_memory_db::<Blake2Hasher>(&root).unwrap();
    assert_eq!(replay_root, root);
    assert_eq!(
      TrieDBBuilder::<Layout>::new(&replay, &root)
        .build()
        .get(&retry_key)
        .unwrap()
        .as_ref(),
      Some(retry_value)
    );
    // A targeted negative is valid only when the exact selected encoded payload is separable.
    let necessity = if full_inputs.contains(retry_value) {
      let missing = StorageProof::new(
        full_inputs
          .iter()
          .filter(|node| *node != retry_value)
          .cloned(),
      )
      .into_memory_db::<Blake2Hasher>();
      assert!(
        TrieDBBuilder::<Layout>::new(&missing, &root)
          .build()
          .get(&retry_key)
          .is_err()
      );
      "authenticatedSelectedPayloadRemovalFailsRead"
    } else {
      "necessityNotEstablished"
    };
    assert_eq!(
      delta_trie_root::<Layout, _, _, _, _, _>(&mut replay, root, delta.clone(), None, None)
        .unwrap(),
      post_root
    );
    println!(
      "PREPASS_DEADLINE_NATIVE_WITNESS_V1 {}",
      serde_json::json!({
        "branch": if parent_cut { "entryFalseBreakerDeadline" } else { "standaloneDeadline" },
        "originalRoot": format!("{root:?}"), "postRoot": format!("{post_root:?}"),
        "baselineSdkBytes": baseline.0, "baselineRawNodes": baseline.1.iter_nodes().count(),
        "prefixRawNodes": prefix.as_ref().map(|proof| proof.iter_nodes().count()),
        "bodySdkBytes": body_size, "sdkIncrement": body_size.saturating_sub(baseline.0),
        "signedScalarChange": i64::from(body_size) - i64::from(baseline.0),
        "bodyRawNodes": body.iter_nodes().count(), "fullRawNodes": full.iter_nodes().count(),
        "baselineOriginalInputs": baseline_inputs.len(),
        "baselineComputedOutputs": baseline.1.iter_nodes().count() - baseline_inputs.len(),
        "prefixOriginalInputs": prefix_inputs.as_ref().map(BTreeSet::len),
        "prefixComputedOutputs": prefix.as_ref().zip(prefix_inputs.as_ref()).map(|(raw, inputs)| raw.iter_nodes().count() - inputs.len()),
        "bodyOriginalInputs": body_inputs.len(), "bodyComputedOutputs": body.iter_nodes().count() - body_inputs.len(),
        "rootAddedOriginalRepair": root_added_original.len(),
        "excludedComputedOutputs": computed_outputs, "bodyOnlyDeltaSufficient": body_only_sufficient,
        "rawOriginalWitnessBytes": witness.encoded_size(), "compactOriginalWitnessBytes": compact.encoded_size(),
        "stopCounts": stop_counts, "completeMixedDeltaKeys": delta.len(),
        "actualDueTickPeers": peer_outcomes.len(), "selectedOriginalInput": "DeadlinePages retry page",
        "targetedNecessity": necessity, "completeDeltaReplayed": true,
        "nativeEmptyCodeSyntheticValidationDataNotWasmHistoryOrProductionAuthority": true,
        "zeroStopWritesDoesNotMeanZeroOverlay": true,
      })
    );
  }
}

#[cfg(not(feature = "benchmark-authority-scaffolding"))]
#[test]
#[ignore = "opt-in native SDK baseline; no timing, Wasm or pricing"]
fn deadline_c0_caller_recording_matches_sdk_baseline() {
  use codec::Encode;
  use polkadot_sdk::{
    frame_benchmarking::{Recording, benchmarking},
    sp_core::{H256, storage::TrackedStorageKey},
    sp_trie::{StorageProof, recorder::Recorder},
  };
  use std::collections::{BTreeMap, BTreeSet};

  #[derive(Debug, Eq, PartialEq)]
  struct Baseline {
    root: H256,
    size: u32,
    proof: StorageProof,
  }
  #[derive(Debug, Eq, PartialEq)]
  struct Body {
    size: u32,
    proof: StorageProof,
    stop_counts: (u32, u32, u32, u32),
  }
  struct Boundary {
    recorder: Recorder<BlakeTwo256>,
    whitelist: Vec<TrackedStorageKey>,
    baseline: Option<Baseline>,
    body: Option<Body>,
  }
  impl Recording for Boundary {
    fn start(&mut self) {
      assert!(self.baseline.is_none() && self.body.is_none());
      assert_ne!(System::block_number(), 0);
      assert_eq!(benchmarking::get_whitelist(), self.whitelist);
      benchmarking::commit_db();
      assert!(self.recorder.to_storage_proof().is_empty());
      let live = benchmarking::get_whitelist();
      assert_eq!(live, self.whitelist);
      for key in &live {
        assert!(key.child_trie_key.is_none());
        polkadot_sdk::frame_support::storage::unhashed::get_raw(&key.key);
      }
      let size = benchmarking::proof_size().unwrap();
      let proof = self.recorder.to_storage_proof();
      assert!(size > 0 && !proof.is_empty());
      let roots = self.recorder.recorded_keys();
      assert_eq!(roots.len(), 1);
      let root = *roots.keys().next().unwrap();
      benchmarking::reset_read_write_count();
      assert_eq!(benchmarking::read_write_count(), (0, 0, 0, 0));
      assert_eq!(benchmarking::proof_size(), Some(size));
      assert_eq!(self.recorder.to_storage_proof(), proof);
      self.baseline = Some(Baseline { root, size, proof });
    }

    fn stop(&mut self) {
      assert!(self.body.is_none());
      let baseline = self.baseline.as_ref().unwrap();
      assert_eq!(benchmarking::get_whitelist(), self.whitelist);
      let size = benchmarking::proof_size().unwrap();
      let proof = self.recorder.to_storage_proof();
      let stop_counts = benchmarking::read_write_count();
      assert!(size >= baseline.size);
      let nodes = proof.iter_nodes().collect::<BTreeSet<_>>();
      assert!(baseline.proof.iter_nodes().all(|node| nodes.contains(node)));
      let roots = self.recorder.recorded_keys();
      assert_eq!(roots.len(), 1);
      assert!(roots.contains_key(&baseline.root));
      self.body = Some(Body {
        size,
        proof,
        stop_counts,
      });
    }
  }

  fn assert_restored(host: &sc_client_db::BenchmarkingState<BlakeTwo256>, parent: &Storage) {
    let restored = host
      .pairs(Default::default())
      .unwrap()
      .map(Result::unwrap)
      .collect::<BTreeMap<_, _>>();
    assert!(
      restored == parent.top,
      "wipe restores the complete parent TOP"
    );
  }

  for variable in ["RUST_LOG", "RUNTIME_LOG"] {
    assert!(
      std::env::var_os(variable).is_none(),
      "{variable} must be absent"
    );
  }
  let method = b"mandatory_prepass_deadline_control".as_slice();
  assert!(
    <Actors as Benchmarking>::benchmarks(false)
      .iter()
      .all(|entry| entry.name != method)
  );
  let metadata = <Actors as Benchmarking>::benchmarks(true);
  assert_eq!(
    metadata
      .iter()
      .find(|entry| entry.name == method)
      .unwrap()
      .components,
    vec![(BenchmarkParameter::c, 0, 1)],
  );
  let parent = super::production_block_replay::preset_genesis_storage(
    &[],
    &polkadot_sdk::sp_genesis_builder::DEV_RUNTIME_PRESET.into(),
  );
  assert!(parent.children_default.is_empty());
  let components = [(BenchmarkParameter::c, 0)];
  let (sdk_increment, whitelist) = common::with_benchmarking_host(parent.clone(), |host| {
    let rows = <Actors as Benchmarking>::run_benchmark(
      method,
      &components,
      &AllPalletsWithSystem::whitelisted_storage_keys(),
      false,
      1,
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].components, components);
    let prefix = pallet_deos_actors::DeadlineIndexPages::<Runtime>::final_prefix().to_vec();
    assert!(rows[0].keys.iter().any(|(key, _, _, _)| key == &prefix));
    let whitelist = host.get_whitelist();
    assert!(whitelist.iter().all(|key| key.child_trie_key.is_none()));
    assert!(whitelist.iter().any(|key| {
      key.key == polkadot_sdk::frame_support::storage::transactional::TRANSACTION_LEVEL_KEY
    }));
    assert_restored(host, &parent);
    // No timing field or complete SDK result is read, formatted or retained.
    (rows[0].proof_size, whitelist)
  });

  let mut observations = Vec::new();
  for verify in [false, true] {
    let observed = common::with_benchmarking_host(parent.clone(), |host| {
      benchmarking::wipe_db();
      benchmarking::set_whitelist(whitelist.clone());
      let mut boundary = Boundary {
        recorder: host.recorder().unwrap(),
        whitelist: whitelist.clone(),
        baseline: None,
        body: None,
      };
      Actors::verify_prepass_deadline_c0_benchmark(&mut boundary, verify).unwrap();
      let baseline = boundary.baseline.take().unwrap();
      let body = boundary.body.take().unwrap();
      let increment = body.size.checked_sub(baseline.size).unwrap();
      assert_eq!(increment, sdk_increment);
      let after_verify = benchmarking::read_write_count();
      let reads = |counts: (u32, u32, u32, u32)| u64::from(counts.0) + u64::from(counts.1);
      if verify {
        assert!(reads(after_verify) > reads(body.stop_counts));
      }
      let later_nodes = boundary.recorder.to_storage_proof();
      let later_set = later_nodes.iter_nodes().collect::<BTreeSet<_>>();
      assert!(body.proof.iter_nodes().all(|node| later_set.contains(node)));
      let keys = boundary
        .recorder
        .recorded_keys()
        .into_values()
        .flat_map(|keys| keys.into_keys().map(|key| key.as_ref().to_vec()))
        .collect::<BTreeSet<_>>();
      for key in [
        pallet_deos_actors::ActorReadyTail::<Runtime>::hashed_key(),
        pallet_deos_actors::PrepassExecutionCutoff::<Runtime>::hashed_key(),
      ] {
        assert_eq!(keys.contains(key.as_slice()), verify);
      }
      assert_eq!(benchmarking::get_whitelist(), whitelist);
      eprintln!(
        "DEADLINE_NATIVE_SDK_BASELINE verify={verify} baseline={} absolute={} increment={increment} stop_before_commit={:?} after_verify_before_commit={after_verify:?} body_nodes={} native_only=true sdk_prepared_root_attested=false timings=excluded",
        baseline.size,
        body.size,
        body.stop_counts,
        body.proof.iter_nodes().count(),
      );
      // Commit/wipe are strictly after the saved body proof and generated verification.
      benchmarking::commit_db();
      benchmarking::wipe_db();
      assert_restored(host, &parent);
      (baseline, body, after_verify)
    });
    observations.push(observed);
  }
  assert_eq!(observations[0].0, observations[1].0);
  assert_eq!(observations[0].1, observations[1].1);
  assert!(
    u64::from(observations[1].2.0) + u64::from(observations[1].2.1)
      > u64::from(observations[0].2.0) + u64::from(observations[0].2.1)
  );
  let whitelist_digest = H256::from(polkadot_sdk::sp_io::hashing::blake2_256(
    &whitelist.encode(),
  ));
  eprintln!(
    "DEADLINE_NATIVE_SDK_BASELINE_COMPLETE bodies=3 sdk_increment={sdk_increment} whitelist_keys={} whitelist_blake2_256={whitelist_digest:x} native_only=true timings=excluded",
    whitelist.len(),
  );
}

#[cfg(not(feature = "benchmark-authority-scaffolding"))]
mod wasm_frozen {
  use super::*;
  use codec::{DecodeAll, Encode};
  use polkadot_sdk::{
    frame_benchmarking::{BenchmarkBatch, BenchmarkConfig, BenchmarkList},
    frame_support::traits::StorageInfo,
    sp_core::traits::{CallContext, RuntimeCode, WrappedRuntimeCode},
    sp_externalities::Extensions,
    sp_runtime::traits::Hash,
    sp_state_machine::{OverlayedChanges, StateMachine},
    sp_trie::proof_size_extension::ProofSizeExt,
    sp_version::RuntimeVersion,
  };
  use sc_executor::{HeapAllocStrategy, WasmExecutor};
  use std::{borrow::Cow, ffi::OsString};

  type HostFunctions = (
    polkadot_sdk::sp_io::SubstrateHostFunctions,
    polkadot_sdk::frame_benchmarking::benchmarking::HostFunctions,
    cumulus_primitives_proof_size_hostfunction::storage_proof_size::HostFunctions,
  );

  fn digest(bytes: &[u8]) -> String {
    polkadot_sdk::sp_io::hashing::sha2_256(bytes)
      .iter()
      .map(|byte| format!("{byte:02x}"))
      .collect()
  }

  fn read_bound_wasm(
    path: Option<OsString>,
    expected: Option<OsString>,
  ) -> Result<Vec<u8>, String> {
    let path = path.ok_or("DEOS_ACTOR_CONTROL_WASM is required")?;
    let expected = expected
      .ok_or("DEOS_ACTOR_CONTROL_WASM_SHA256 is required")?
      .into_string()
      .map_err(|_| "Wasm digest must be lowercase ASCII hex")?;
    if expected.len() != 64
      || !expected
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
      return Err("Wasm digest must contain 64 lowercase hex digits".into());
    }
    let code = std::fs::read(path).map_err(|error| format!("Explicit Wasm input: {error}"))?;
    if digest(&code) != expected {
      return Err("Explicit Wasm SHA-256 mismatch".into());
    }
    Ok(code)
  }

  #[test]
  fn artifact_input_refuses_missing_malformed_and_mismatched_identity() {
    let source = OsString::from(concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/src/tests/actor_control_benchmark.rs"
    ));
    assert!(read_bound_wasm(None, Some("0".repeat(64).into())).is_err());
    assert!(read_bound_wasm(Some(source.clone()), None).is_err());
    for value in [
      "".into(),
      "0".repeat(63),
      "G".repeat(64),
      "A".repeat(64),
      format!("{}\n", "0".repeat(64)),
    ] {
      assert!(read_bound_wasm(Some(source.clone()), Some(value.into())).is_err());
    }
    let bytes = std::fs::read(&source).unwrap();
    let actual = digest(&bytes);
    let mut mismatch = actual.clone();
    mismatch.replace_range(..1, if actual.starts_with('0') { "1" } else { "0" });
    assert_eq!(
      read_bound_wasm(Some(source.clone()), Some(mismatch.into())).unwrap_err(),
      "Explicit Wasm SHA-256 mismatch"
    );
    assert_eq!(
      read_bound_wasm(Some(source), Some(actual.into())).unwrap(),
      bytes
    );
  }

  fn call_api<R: DecodeAll>(
    host: &sc_client_db::BenchmarkingState<BlakeTwo256>,
    executor: &WasmExecutor<HostFunctions>,
    code: &RuntimeCode<'_>,
    method: &str,
    input: &[u8],
  ) -> R {
    let mut extensions = Extensions::default();
    extensions.register(ProofSizeExt::new(
      host.recorder().expect("SDK recorder is required"),
    ));
    let mut overlay = OverlayedChanges::default();
    let bytes = StateMachine::new(
      host,
      &mut overlay,
      executor,
      method,
      input,
      &mut extensions,
      code,
      CallContext::Offchain,
    )
    .execute()
    .unwrap_or_else(|error| panic!("Wasm {method} execution: {error}"));
    R::decode_all(&mut bytes.as_slice())
      .unwrap_or_else(|error| panic!("Wasm {method} decode: {error}"))
  }

  #[test]
  #[ignore = "Requires an explicitly source-bound benchmark Wasm and SHA-256; no timing evidence"]
  fn caller_bound_public_prepass_c0_conforms() {
    assert!(
      std::env::var_os("RUST_LOG").is_none() && std::env::var_os("RUNTIME_LOG").is_none(),
      "Unset timing trace selectors for conformance"
    );
    let wasm = read_bound_wasm(
      std::env::var_os("DEOS_ACTOR_CONTROL_WASM"),
      std::env::var_os("DEOS_ACTOR_CONTROL_WASM_SHA256"),
    )
    .unwrap();
    let code_fetcher = WrappedRuntimeCode(Cow::Borrowed(&wasm));
    let code = RuntimeCode {
      code_fetcher: &code_fetcher,
      heap_pages: Some(8192),
      hash: BlakeTwo256::hash(&wasm).as_bytes().to_vec(),
    };
    // Offchain benchmark APIs use the executor's offchain allocator, not RuntimeCode.heap_pages.
    let executor = WasmExecutor::<HostFunctions>::builder()
      .with_onchain_heap_alloc_strategy(HeapAllocStrategy::Static { extra_pages: 8192 })
      .with_offchain_heap_alloc_strategy(HeapAllocStrategy::Static { extra_pages: 8192 })
      .build();
    let method = b"mandatory_prepass_control".as_slice();
    // Probes use a disposable parent, not a body cell's live backend/recorder.
    common::with_benchmarking_host(control_source_parents()[1].1.clone(), |host| {
      let mut version: RuntimeVersion = call_api(host, &executor, &code, "Core_version", &[]);
      let mut expected = crate::VERSION;
      version.apis.to_mut().sort_unstable();
      expected.apis.to_mut().sort_unstable();
      assert_eq!(version, expected, "actual Wasm Core/API coordinate");
      for extra in [false, true] {
        let (list, _): (Vec<BenchmarkList>, Vec<StorageInfo>) = call_api(
          host,
          &executor,
          &code,
          "Benchmark_benchmark_metadata",
          &extra.encode(),
        );
        let selected = list
          .iter()
          .filter(|entry| entry.pallet == b"pallet_deos_actors" && entry.instance == b"Actors")
          .collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        let matches = selected[0]
          .benchmarks
          .iter()
          .filter(|entry| entry.name == method)
          .collect::<Vec<_>>();
        assert_eq!(matches.len(), usize::from(extra));
        if extra {
          assert_eq!(matches[0].components, vec![(BenchmarkParameter::c, 0, 2)]);
        }
      }
    });
    check_actor_control_source_sdk_with(method, 2, 0..=0, |host, c, verify| {
      let config = BenchmarkConfig {
        pallet: b"pallet_deos_actors".to_vec(),
        instance: b"Actors".to_vec(),
        benchmark: method.to_vec(),
        selected_components: vec![(BenchmarkParameter::c, c)],
        verify,
        internal_repeats: 1,
      };
      let batches: Result<Vec<BenchmarkBatch>, String> = call_api(
        host,
        &executor,
        &code,
        "Benchmark_dispatch_benchmark",
        &config.encode(),
      );
      let mut batches = batches.unwrap_or_else(|error| panic!("Wasm benchmark API: {error}"));
      assert_eq!(batches.len(), 1);
      let batch = batches.pop().unwrap();
      assert_eq!(batch.pallet, config.pallet);
      assert_eq!(batch.instance, config.instance);
      assert_eq!(batch.benchmark, config.benchmark);
      for row in &batch.results {
        assert!(
          !row
            .keys
            .iter()
            .any(|(key, _, _, _)| key == b"Benchmark Override"),
          "overrides are not source conformance"
        );
        assert!(
          !row.keys.is_empty(),
          "weightless substitution is not source conformance"
        );
      }
      batch.results
    });
    println!(
      "WASM_FROZEN_CONTROL_CONFORMANCE sha256={} parents=2 cells=1 verify_modes=2 body_dispatches=4 timings_discarded=true",
      digest(&wasm)
    );
  }
}

#[test]
fn reference_context_helper_verifies_its_actual_abridged_operand() {
  use crate::configs::{
    MaxInboundDownwardMessagesPerContext, MaxInboundHorizontalChannelsPerContext,
    MaxInboundHorizontalMessagesPerContext, RuntimeBlockWeights,
    actor_config::RuntimeActorsBenchmarkHelper as Helper,
  };
  use codec::Encode;
  use pallet_deos_actors::BenchmarkHelper;
  use polkadot_sdk::frame_support::assert_ok;

  common::seeded_test_ext().execute_with(|| {
    let (data, inbound) = Helper::prepare_maximum_context_inherent();
    assert_eq!(
      data.encode().as_slice(),
      include_bytes!("../../fixtures/maximum-context-basic.scale").as_slice()
    );
    let expected = (RuntimeBlockWeights::get().max_block.proof_size() / 6 / 65_536)
      .min(u64::from(MaxInboundDownwardMessagesPerContext::get())) as u32;
    let (full, hashed) = inbound.downward_messages.messages();
    assert_eq!(full.len(), expected as usize);
    assert!(full.iter().all(|message| message.msg.len() == 65_536));
    assert_eq!(
      full.len() + hashed.len(),
      MaxInboundDownwardMessagesPerContext::get() as usize
    );
    let (full, hashed) = inbound.horizontal_messages.messages();
    assert_eq!(
      full.len(),
      MaxInboundHorizontalMessagesPerContext::get() as usize
    );
    assert!(full.iter().all(|(_, message)| message.data == [0]));
    assert!(hashed.is_empty());
    assert_eq!(
      full
        .iter()
        .map(|(sender, _)| *sender)
        .collect::<std::collections::BTreeSet<_>>()
        .len(),
      MaxInboundHorizontalChannelsPerContext::get() as usize
    );
    assert_ok!(crate::apis::validate_inbound_messages_geometry(&inbound));
    assert_ok!(Helper::execute_maximum_context_inherent((data, inbound)));
    assert_eq!(
      polkadot_sdk::cumulus_pallet_parachain_system::ProcessedDownwardMessages::<Runtime>::get(),
      expected
    );
    Helper::verify_maximum_context_inherent();
    // A deliberately inconsistent counter must not turn verification into a no-op.
    polkadot_sdk::cumulus_pallet_parachain_system::ProcessedDownwardMessages::<Runtime>::put(
      expected + 1,
    );
    assert!(std::panic::catch_unwind(Helper::verify_maximum_context_inherent).is_err());
  });
}
